// Tauri entry. `main.rs` only calls `run()` — all setup happens here.
//
// P0.1 wiring:
//   - global-shortcut plugin → dispatches into `HotkeyRegistry`
//   - Press → state Idle→Recording → show overlay window
//   - Release → state Recording→Idle → hide overlay window
//   - Overlay window positioned bottom-center, click-through

mod asr;
mod commands;
mod error;
mod llm;
mod managers;
mod platform;
mod provider_test;
mod state;

use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition};
use tauri_plugin_global_shortcut::ShortcutState;

use crate::asr::doubao::config as doubao_config;
use crate::asr::{AudioData, TranscriptStream};
use crate::error::{AppError, AppResult};
use crate::managers::audio::AudioManager;
use crate::managers::enhance::{fallback_after_enhance_failure, EnhanceConfig, EnhanceManager};
use crate::managers::inject::InjectManager;
use crate::managers::transcription::{TranscriptionConfig, TranscriptionManager};
use crate::platform::{current_platform, HotkeyCallback, HotkeyEvent, HotkeyRegistry, Platform};
use crate::state::{AppState, StateMachine};

const SUCCESS_HOLD_MS: u64 = 150;
const ERROR_HOLD_MS: u64 = 2500;

type ActiveStreamingSession = Arc<parking_lot::Mutex<Option<TranscriptStream>>>;

#[derive(Serialize, Clone)]
struct FinalTranscript {
    text: String,
    duration_ms: u64,
}

#[derive(Serialize, Clone)]
#[allow(dead_code)] // P2.2 defines the event payload; P2.5 will emit it to overlay.
struct PartialTranscript {
    text: String,
    is_final: bool,
    sequence: u64,
}

#[derive(Serialize, Clone)]
struct EnhanceProgress {
    phase: &'static str,
    message: String,
}

// §3.6 `error` event payload. Flattens AppError's category + message and adds
// the recoverable flag from the §3.7 table.
#[derive(Serialize, Clone)]
struct ErrorPayload {
    code: &'static str,
    message: String,
    recoverable: bool,
}

struct HotkeyContext<'a> {
    app: &'a AppHandle,
    state: &'a Arc<StateMachine>,
    audio: &'a Arc<AudioManager>,
    transcription: &'a Arc<TranscriptionManager>,
    enhance: &'a Arc<EnhanceManager>,
    inject: &'a Arc<InjectManager>,
    streaming: &'a ActiveStreamingSession,
}

const OVERLAY_WINDOW_LABEL: &str = "overlay";
const OVERLAY_BOTTOM_MARGIN_PX: f64 = 4.0;

/// Hide a window's green zoom traffic light. The main window is fixed-size, so
/// the zoom button can never do anything; hiding it reads cleaner than a grayed
/// stub. Done via a direct NSWindow `standardWindowButton:` / `setHidden:` —
/// macOS has no per-button Tauri config.
#[cfg(target_os = "macos")]
fn hide_zoom_button(window: &tauri::WebviewWindow) {
    use objc2::msg_send;
    use objc2::runtime::{AnyObject, Bool};

    let Ok(ptr) = window.ns_window() else {
        return;
    };
    let ns_window = ptr.cast::<AnyObject>();
    if ns_window.is_null() {
        return;
    }
    // NSWindowButton::ZoomButton == 2.
    // SAFETY: `ns_window` is the live NSWindow for this webview window; the
    // selectors are standard AppKit and setup runs on the main thread.
    unsafe {
        let button: *mut AnyObject = msg_send![ns_window, standardWindowButton: 2usize];
        if !button.is_null() {
            let _: () = msg_send![button, setHidden: Bool::new(true)];
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .try_init();

    let registry = Arc::new(HotkeyRegistry::default());
    let registry_for_handler = registry.clone();
    let registry_for_setup = registry.clone();

    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |_app, shortcut, event| {
                    let hk = match event.state() {
                        ShortcutState::Pressed => HotkeyEvent::Pressed,
                        ShortcutState::Released => HotkeyEvent::Released,
                    };
                    registry_for_handler.dispatch(shortcut, hk);
                })
                .build(),
        )
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            // Frontend → Rust goes through commands. The hot path itself is not
            // command-driven: global-shortcut events below enter `handle_hotkey`,
            // while settings/keychain/provider-test stay here as explicit UI calls.
            commands::get_settings,
            commands::update_settings,
            commands::export_config,
            commands::import_config,
            commands::list_asr_providers,
            commands::list_llm_providers,
            commands::set_secret,
            commands::has_secret,
            commands::get_secret_for_settings,
            commands::delete_secret,
            #[cfg(debug_assertions)]
            commands::test_doubao_streaming,
            provider_test::test_provider,
            // Overlay capsule controls (fe.8b).
            confirm_recording,
            cancel_recording,
        ])
        .setup(move |app| {
            let app_handle = app.handle().clone();

            if let Err(err) = position_overlay(&app_handle) {
                log::error!("position overlay failed: {err:?}");
                return Err(Box::new(std::io::Error::other(format!("{err:?}"))));
            }

            // Convert the overlay into a non-activating NSPanel so clicking the
            // capsule buttons (✕/✓) never activates Audie / steals focus (fe.8b-2).
            #[cfg(target_os = "macos")]
            convert_overlay_to_panel(app);

            // The main window is fixed-size (resizable/maximizable off), so the
            // green zoom traffic light is dead — hide it instead of showing it
            // grayed out (macOS only).
            #[cfg(target_os = "macos")]
            if let Some(window) = app.get_webview_window("main") {
                hide_zoom_button(&window);
            }

            let state_machine = Arc::new(StateMachine::new());
            let audio = Arc::new(AudioManager::new());
            let transcription = Arc::new(TranscriptionManager::new());
            let enhance = Arc::new(EnhanceManager::new());
            let platform: Arc<dyn Platform> =
                Arc::from(current_platform(registry_for_setup.clone()));
            let inject = Arc::new(InjectManager::new(platform.clone()));

            // This is the backend object graph for the P1 pipeline:
            // StateMachine owns legal UI states; Audio captures samples; ASR and
            // LLM managers choose providers from settings; Inject delegates the
            // OS-specific paste/keychain work to Platform.
            // Manage first so `build_hotkey_callback` can resolve managers off
            // the app state — the same callback gets rebuilt when the hotkey
            // changes (commands::update_settings).
            app.manage(state_machine);
            app.manage(audio);
            app.manage(transcription);
            app.manage(enhance);
            app.manage(inject);
            app.manage(registry_for_setup.clone());
            app.manage(platform.clone());
            app.manage(Arc::new(parking_lot::Mutex::new(None::<TranscriptStream>)));

            let hotkey = commands::load_hotkey(&app_handle);
            if let Err(err) =
                platform.register_hotkey(&app_handle, &hotkey, build_hotkey_callback(&app_handle))
            {
                log::error!("register hotkey {hotkey}: {err:?}");
                return Err(Box::new(std::io::Error::other(format!("{err:?}"))));
            }

            log::info!("registered global hotkey {hotkey}");

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Build the press/release callback for the global hotkey. Resolves managers
/// off the app state instead of capturing clones, so it can be rebuilt verbatim
/// when the hotkey changes — see `commands::update_settings`.
pub(crate) fn build_hotkey_callback(app: &AppHandle) -> HotkeyCallback {
    let app = app.clone();
    Box::new(move |event| {
        let state = app.state::<Arc<StateMachine>>();
        let audio = app.state::<Arc<AudioManager>>();
        let transcription = app.state::<Arc<TranscriptionManager>>();
        let enhance = app.state::<Arc<EnhanceManager>>();
        let inject = app.state::<Arc<InjectManager>>();
        let streaming = app.state::<ActiveStreamingSession>();
        let ctx = HotkeyContext {
            app: &app,
            state: state.inner(),
            audio: audio.inner(),
            transcription: transcription.inner(),
            enhance: enhance.inner(),
            inject: inject.inner(),
            streaming: streaming.inner(),
        };
        handle_hotkey(&ctx, event);
    })
}

fn handle_hotkey(ctx: &HotkeyContext<'_>, event: HotkeyEvent) {
    // Toggle control model: each hotkey *press* starts a take (from Idle) or
    // finishes it (from Recording). Key-up is ignored — holding no longer
    // auto-stops; the user presses again to finish. A press mid-pipeline
    // (Processing/Success/Error/Cancel) is a no-op.
    if !matches!(event, HotkeyEvent::Pressed) {
        return;
    }
    match ctx.state.current() {
        AppState::Idle => start_recording(ctx),
        AppState::Recording => finish_recording(ctx),
        _ => {}
    }
}

/// Enter the front half of the pipeline: permission gate → open cpal stream →
/// Recording state → overlay. No ASR happens until finish, because P1 uses
/// batch transcription.
fn start_recording(ctx: &HotkeyContext<'_>) {
    // Gate on mic permission before recording: a denial otherwise captures
    // silence and the user only sees a Whisper hallucination. Flash red
    // instead (§3.7 Permission).
    let platform = ctx.app.state::<Arc<dyn Platform>>();
    if !platform.ensure_microphone_permission() {
        let _ = show_overlay(ctx.app);
        enter_error(
            ctx.app.clone(),
            ctx.state.clone(),
            AppError::Permission("请授予麦克风权限".into()),
        );
        return;
    }
    // Start capture BEFORE the Idle→Recording transition: a cpal failure
    // (no input device, build_input_stream blew up, etc.) needs to surface
    // as Idle→Error (§3.7 Device) which is only legal from Idle. Doing the
    // transition first would strand us in Recording with a dead stream.
    let streaming_start = start_streaming_session(ctx.app, ctx.transcription, ctx.streaming);
    let capture_result = match streaming_start {
        Some(chunk_tx) => ctx.audio.start_capture_streaming(ctx.app.clone(), chunk_tx),
        None => ctx.audio.start_capture(ctx.app.clone()),
    };
    if let Err(err) = capture_result {
        log::error!("start capture: {err:?}");
        clear_streaming_session(ctx.streaming);
        let _ = show_overlay(ctx.app);
        enter_error(ctx.app.clone(), ctx.state.clone(), err);
        return;
    }
    if ctx
        .state
        .transition(ctx.app, AppState::Recording, Some("toggle-start"))
    {
        if let Err(err) = show_overlay(ctx.app) {
            log::error!("show overlay: {err:?}");
        }
    } else {
        // Transition rejected (shouldn't happen — we just confirmed Idle).
        // Tear down the capture we just opened.
        clear_streaming_session(ctx.streaming);
        if let Err(err) = ctx.audio.stop_capture() {
            log::warn!("rollback stop_capture: {err:?}");
        }
    }
}

/// Close the audio session and hand one complete utterance to the pipeline
/// tail. The overlay stays visible through Processing/Success/Error so the
/// user sees the result; it's hidden only when we settle back to Idle.
fn finish_recording(ctx: &HotkeyContext<'_>) {
    if !ctx
        .state
        .transition(ctx.app, AppState::Processing, Some("toggle-finish"))
    {
        return;
    }
    let streaming_result = take_streaming_session(ctx.streaming);
    match ctx.audio.stop_capture() {
        Ok(recorded) => {
            spawn_transcription(
                ctx.app.clone(),
                ctx.state.clone(),
                ctx.transcription.clone(),
                ctx.enhance.clone(),
                ctx.inject.clone(),
                recorded,
                streaming_result,
            );
        }
        Err(err) => {
            log::error!("stop capture: {err:?}");
            enter_error(ctx.app.clone(), ctx.state.clone(), err);
        }
    }
}

/// Build a HotkeyContext from app state — the same wiring `build_hotkey_callback`
/// uses — so the overlay's cancel/confirm commands drive the exact same pipeline.
fn with_hotkey_ctx<R>(app: &AppHandle, f: impl FnOnce(&HotkeyContext<'_>) -> R) -> R {
    let state = app.state::<Arc<StateMachine>>();
    let audio = app.state::<Arc<AudioManager>>();
    let transcription = app.state::<Arc<TranscriptionManager>>();
    let enhance = app.state::<Arc<EnhanceManager>>();
    let inject = app.state::<Arc<InjectManager>>();
    let streaming = app.state::<ActiveStreamingSession>();
    let ctx = HotkeyContext {
        app,
        state: state.inner(),
        audio: audio.inner(),
        transcription: transcription.inner(),
        enhance: enhance.inner(),
        inject: inject.inner(),
        streaming: streaming.inner(),
    };
    f(&ctx)
}

/// ✓ on the capsule — finish the current take (same as a second hotkey press).
#[tauri::command]
fn confirm_recording(app: AppHandle) {
    with_hotkey_ctx(&app, |ctx| {
        if ctx.state.current() == AppState::Recording {
            finish_recording(ctx);
        }
    });
}

/// ✕ on the capsule — discard the current recording and return to Idle. fe.8b
/// only handles Recording; cancelling mid-pipeline (with undo) lands in fe.8c.
#[tauri::command]
fn cancel_recording(app: AppHandle) {
    with_hotkey_ctx(&app, |ctx| {
        if ctx.state.current() != AppState::Recording {
            return;
        }
        clear_streaming_session(ctx.streaming);
        if let Err(err) = ctx.audio.stop_capture() {
            log::warn!("cancel stop_capture: {err:?}");
        }
        ctx.state
            .transition(ctx.app, AppState::Cancel, Some("overlay-cancel"));
        settle_to_idle(ctx.app, ctx.state, "cancelled");
    });
}

/// Run the (blocking) transcription off the hotkey thread, then drive the
/// P1 pipeline tail: ASR → `final-transcript` event → optional LLM enhance →
/// inject at the caret → Success → Idle. Any failure is mapped to `error` and
/// recovers through the same Idle exit. PROJECT_SPEC.md §3.2 / §3.3 / §4.3.
fn spawn_transcription(
    app: AppHandle,
    state: Arc<StateMachine>,
    transcription: Arc<TranscriptionManager>,
    enhance: Arc<EnhanceManager>,
    inject: Arc<InjectManager>,
    audio: AudioData,
    streaming_result: Option<TranscriptStream>,
) {
    thread::spawn(move || {
        let duration_ms = duration_ms(&audio);

        // AirPods on A2DP (no HFP switch yet) and a few other broken-mic states
        // produce a buffer of literal zeros. Sending that to Whisper costs an API
        // call and gets back "Thank you." hallucinations. Detect digital silence
        // here and short-circuit to a friendly Device error. Threshold is "any
        // sample exceeds 1e-4" — real mic noise floor sits well above that, so a
        // healthy 200ms recording always passes.
        if duration_ms >= 200 && is_digital_silence(&audio) {
            enter_error(
                app,
                state,
                AppError::Device(
                    "麦克风没声音，请检查蓝牙耳机是否切到通话模式，或换默认输入设备".into(),
                ),
            );
            return;
        }

        let text = match resolve_transcript(&app, &transcription, &audio, streaming_result) {
            Ok(text) => text,
            Err(err) => {
                log::error!("transcription failed: {err:?}");
                enter_error(app, state, err);
                return;
            }
        };

        if let Err(err) = finish_pipeline_tail(&app, &enhance, &inject, &text, duration_ms) {
            log::error!("inject failed: {err:?}");
            enter_error(app, state, err);
            return;
        }

        state.transition(&app, AppState::Success, Some("injected"));
        thread::sleep(Duration::from_millis(SUCCESS_HOLD_MS));
        settle_to_idle(&app, &state, "done");
    });
}

fn resolve_transcript(
    app: &AppHandle,
    transcription: &TranscriptionManager,
    audio: &AudioData,
    streaming_result: Option<TranscriptStream>,
) -> AppResult<String> {
    if let Some(stream) = streaming_result {
        let delta = stream
            .recv()
            .map_err(|_| AppError::Network("doubao stream ended without result".into()))??;
        if delta.is_final {
            return Ok(delta.text);
        }
        return Err(AppError::Internal(
            "doubao stream returned non-final transcript".into(),
        ));
    }

    let config = transcription_config(app);
    transcription.transcribe(audio, &config)
}

fn finish_pipeline_tail(
    app: &AppHandle,
    enhance: &EnhanceManager,
    inject: &InjectManager,
    text: &str,
    duration_ms: u64,
) -> AppResult<()> {
    // P0.3 acceptance: the transcript shows up in the console.
    println!("[transcript] {text}");
    log::info!("transcript ({duration_ms} ms): {text}");
    let _ = app.emit(
        "final-transcript",
        FinalTranscript {
            text: text.to_string(),
            duration_ms,
        },
    );

    let text_to_inject = maybe_enhance_text(app, enhance, text);

    // P0.4: inject at the caret. On failure the text is still on the
    // clipboard (§3.7 fallback) — flash Error so the user knows to paste.
    inject.inject(app, &text_to_inject)
}

fn maybe_enhance_text(app: &AppHandle, enhance: &EnhanceManager, text: &str) -> String {
    let config = enhance_config(app);
    if !config.enhance_enabled {
        return text.to_string();
    }

    emit_enhance_progress(app, "started", "润色中…");
    match enhance.enhance(text, &config) {
        Ok(enhanced) => {
            emit_enhance_progress(app, "completed", "润色完成");
            enhanced
        }
        Err(err) => {
            log::warn!("enhance failed, injecting original transcript: {err:?}");
            let fallback = fallback_after_enhance_failure(text, &err);
            emit_enhance_progress(app, "failed", &fallback.message);
            fallback.text_to_inject
        }
    }
}

fn emit_enhance_progress(app: &AppHandle, phase: &'static str, message: &str) {
    let _ = app.emit(
        "enhance-progress",
        EnhanceProgress {
            phase,
            message: message.to_string(),
        },
    );
}

fn start_streaming_session(
    app: &AppHandle,
    transcription: &TranscriptionManager,
    streaming: &ActiveStreamingSession,
) -> Option<mpsc::Sender<AppResult<crate::asr::AudioChunk>>> {
    let config = doubao_streaming_config(app)?;
    let (chunks_tx, chunks_rx) = mpsc::channel();
    match transcription.transcribe_stream(chunks_rx, &config) {
        Ok(transcripts) => {
            *streaming.lock() = Some(transcripts);
            Some(chunks_tx)
        }
        Err(err) => {
            log::warn!("doubao streaming unavailable, falling back to batch: {err:?}");
            None
        }
    }
}

fn clear_streaming_session(streaming: &ActiveStreamingSession) {
    let _ = streaming.lock().take();
}

fn take_streaming_session(streaming: &ActiveStreamingSession) -> Option<TranscriptStream> {
    streaming.lock().take()
}

fn transcription_config(app: &AppHandle) -> TranscriptionConfig {
    let settings = commands::load_settings(app);
    let platform = app.state::<Arc<dyn Platform>>();

    transcription_config_from_settings(
        settings.asr_provider,
        settings.whisper_cpp_model_path,
        |key_id| read_optional_secret(platform.inner().as_ref(), key_id),
    )
}

fn transcription_config_from_settings(
    asr_provider: String,
    whisper_cpp_model_path: Option<String>,
    mut read_secret: impl FnMut(&str) -> Option<String>,
) -> TranscriptionConfig {
    let (groq_api_key, openai_api_key) = match asr_provider.as_str() {
        "groq" => (read_secret("groq_api_key"), None),
        "openai" => (None, read_secret("openai_api_key")),
        _ => (None, None),
    };

    TranscriptionConfig {
        asr_provider,
        groq_api_key,
        openai_api_key,
        whisper_cpp_model_path,
        doubao_endpoint: None,
        doubao_resource_id: None,
        doubao_app_id: None,
        doubao_api_key_or_access_token: None,
    }
}

fn doubao_streaming_config(app: &AppHandle) -> Option<TranscriptionConfig> {
    let settings = commands::load_settings(app);
    let platform = app.state::<Arc<dyn Platform>>();
    doubao_streaming_config_from_settings(
        settings.doubao_endpoint,
        settings.doubao_resource_id,
        |key_id| read_optional_secret(platform.inner().as_ref(), key_id),
    )
}

fn doubao_streaming_config_from_settings(
    endpoint: String,
    resource_id: String,
    mut read_secret: impl FnMut(&str) -> Option<String>,
) -> Option<TranscriptionConfig> {
    let api_key_or_access_token = read_secret(doubao_config::SECRET_API_KEY_OR_ACCESS_TOKEN)?;
    let app_id = read_secret(doubao_config::SECRET_APP_ID);

    Some(TranscriptionConfig {
        asr_provider: "doubao_stream".into(),
        groq_api_key: None,
        openai_api_key: None,
        whisper_cpp_model_path: None,
        doubao_endpoint: Some(endpoint),
        doubao_resource_id: Some(resource_id),
        doubao_app_id: app_id,
        doubao_api_key_or_access_token: Some(api_key_or_access_token),
    })
}

fn enhance_config(app: &AppHandle) -> EnhanceConfig {
    let settings = commands::load_settings(app);
    let platform = app.state::<Arc<dyn Platform>>();

    enhance_config_from_settings(
        settings.llm_provider,
        settings.enhance_enabled,
        settings.enhance_prompt,
        settings.openai_compatible_base_url,
        settings.openai_compatible_model,
        |key_id| read_optional_secret(platform.inner().as_ref(), key_id),
    )
}

fn enhance_config_from_settings(
    llm_provider: String,
    enhance_enabled: bool,
    enhance_prompt: String,
    openai_compatible_base_url: String,
    openai_compatible_model: String,
    mut read_secret: impl FnMut(&str) -> Option<String>,
) -> EnhanceConfig {
    let openai_compatible_api_key = if enhance_enabled && llm_provider == "openai_compatible" {
        read_secret("openai_compatible_api_key")
    } else {
        None
    };

    EnhanceConfig {
        llm_provider,
        enhance_enabled,
        enhance_prompt,
        openai_compatible_api_key,
        openai_compatible_base_url,
        openai_compatible_model,
    }
}

fn read_optional_secret(platform: &dyn Platform, key_id: &str) -> Option<String> {
    platform
        .read_secret(key_id)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Emit the error, flash Error, and recover to Idle after a hold. Spawns its own
/// thread so callers on the hotkey path don't block.
fn enter_error(app: AppHandle, state: Arc<StateMachine>, err: AppError) {
    let _ = app.emit(
        "error",
        ErrorPayload {
            code: err.code(),
            message: err.message().to_string(),
            recoverable: err.recoverable(),
        },
    );
    state.transition(&app, AppState::Error, Some("error"));
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(ERROR_HOLD_MS));
        settle_to_idle(&app, &state, "recovered");
    });
}

/// Transition to Idle and hide the overlay window. The single exit point for the
/// pipeline — overlay visibility mirrors "not Idle".
fn settle_to_idle(app: &AppHandle, state: &Arc<StateMachine>, reason: &str) {
    state.transition(app, AppState::Idle, Some(reason));
    if let Err(err) = hide_overlay(app) {
        log::error!("hide overlay: {err:?}");
    }
}

fn is_digital_silence(audio: &AudioData) -> bool {
    const SILENCE_EPS: f32 = 1e-4;
    !audio.samples.iter().any(|s| s.abs() > SILENCE_EPS)
}

fn duration_ms(audio: &AudioData) -> u64 {
    let denom = audio.sample_rate as u64 * audio.channels.max(1) as u64;
    if denom == 0 {
        0
    } else {
        audio.samples.len() as u64 * 1000 / denom
    }
}

/// Swizzle the overlay `NSWindow` into a non-activating `NSPanel`. Clicking the
/// capsule then never activates Audie, so clipboard injection keeps targeting
/// the user's frontmost app. `cocoa` is deprecated (→ objc2-app-kit) but the
/// crate's `set_collection_behaviour` still takes its type; the API works.
#[cfg(target_os = "macos")]
#[allow(deprecated)]
fn convert_overlay_to_panel(app: &tauri::App) {
    use tauri_nspanel::cocoa::appkit::NSWindowCollectionBehavior;
    use tauri_nspanel::{WebviewPanelManager, WebviewWindowExt};

    app.manage(WebviewPanelManager::default());
    let overlay = match app.get_webview_window(OVERLAY_WINDOW_LABEL) {
        Some(overlay) => overlay,
        None => {
            log::error!("overlay window missing for panel conversion");
            return;
        }
    };
    let panel = match overlay.to_panel() {
        Ok(panel) => panel,
        Err(err) => {
            log::error!("overlay to_panel failed: {err:?}");
            return;
        }
    };
    // NSWindowStyleMaskNonactivatingPanel = 1 << 7.
    panel.set_style_mask(1 << 7);
    // Above app windows; visible across spaces and over fullscreen apps.
    panel.set_level(25); // NSStatusWindowLevel
    panel.set_collection_behaviour(
        NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
            | NSWindowCollectionBehavior::NSWindowCollectionBehaviorStationary
            | NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary,
    );
}

fn position_overlay(app: &AppHandle) -> AppResult<()> {
    let overlay = app
        .get_webview_window(OVERLAY_WINDOW_LABEL)
        .ok_or_else(|| AppError::Internal("overlay window not found".into()))?;

    let monitor = overlay
        .primary_monitor()
        .map_err(|err| AppError::Internal(format!("primary_monitor: {err}")))?
        .ok_or_else(|| AppError::Internal("no primary monitor".into()))?;

    let monitor_size = monitor.size();
    let scale = monitor.scale_factor();

    let win_size = overlay
        .outer_size()
        .map_err(|err| AppError::Internal(format!("outer_size: {err}")))?;

    let bottom_margin_px = (OVERLAY_BOTTOM_MARGIN_PX * scale).round() as i32;
    let x = (monitor_size.width as i32 - win_size.width as i32) / 2;
    let y = monitor_size.height as i32 - win_size.height as i32 - bottom_margin_px;

    overlay
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|err| AppError::Internal(format!("set_position: {err}")))?;

    // Interactivity (clicks on ✕/✓ without activating Audie) is handled by the
    // non-activating NSPanel conversion in `setup`, not here.
    Ok(())
}

// The overlay is an NSPanel on macOS (see setup). Show/hide go through the
// panel's order-front/order-out — AppKit calls that must run on the main thread,
// while show/hide are invoked from the hotkey + pipeline worker threads.
#[cfg(target_os = "macos")]
fn show_overlay(app: &AppHandle) -> AppResult<()> {
    let handle = app.clone();
    app.run_on_main_thread(move || {
        use tauri_nspanel::ManagerExt;
        match handle.get_webview_panel(OVERLAY_WINDOW_LABEL) {
            // order_front_regardless shows it WITHOUT making it key, so the
            // user's app stays frontmost for injection.
            Ok(panel) => panel.order_front_regardless(),
            Err(err) => log::error!("show_overlay: panel not found: {err:?}"),
        }
    })
    .map_err(|err| AppError::Internal(format!("run_on_main_thread: {err}")))
}

#[cfg(target_os = "macos")]
fn hide_overlay(app: &AppHandle) -> AppResult<()> {
    let handle = app.clone();
    app.run_on_main_thread(move || {
        use tauri_nspanel::ManagerExt;
        match handle.get_webview_panel(OVERLAY_WINDOW_LABEL) {
            Ok(panel) => panel.order_out(None),
            Err(err) => log::error!("hide_overlay: panel not found: {err:?}"),
        }
    })
    .map_err(|err| AppError::Internal(format!("run_on_main_thread: {err}")))
}

#[cfg(not(target_os = "macos"))]
fn show_overlay(app: &AppHandle) -> AppResult<()> {
    let overlay = app
        .get_webview_window(OVERLAY_WINDOW_LABEL)
        .ok_or_else(|| AppError::Internal("overlay window not found".into()))?;
    overlay
        .show()
        .map_err(|err| AppError::Internal(format!("show overlay: {err}")))?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn hide_overlay(app: &AppHandle) -> AppResult<()> {
    let overlay = app
        .get_webview_window(OVERLAY_WINDOW_LABEL)
        .ok_or_else(|| AppError::Internal("overlay window not found".into()))?;
    overlay
        .hide()
        .map_err(|err| AppError::Internal(format!("hide overlay: {err}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groq_transcription_config_reads_only_groq_key() {
        let mut requested = Vec::new();

        let config = transcription_config_from_settings("groq".into(), None, |key_id| {
            requested.push(key_id.to_string());
            Some(format!("{key_id}-value"))
        });

        assert_eq!(requested, vec!["groq_api_key"]);
        assert_eq!(config.groq_api_key.as_deref(), Some("groq_api_key-value"));
        assert_eq!(config.openai_api_key, None);
    }

    #[test]
    fn openai_transcription_config_reads_only_openai_key() {
        let mut requested = Vec::new();

        let config = transcription_config_from_settings("openai".into(), None, |key_id| {
            requested.push(key_id.to_string());
            Some(format!("{key_id}-value"))
        });

        assert_eq!(requested, vec!["openai_api_key"]);
        assert_eq!(config.groq_api_key, None);
        assert_eq!(
            config.openai_api_key.as_deref(),
            Some("openai_api_key-value")
        );
    }

    #[test]
    fn whisper_cpp_transcription_config_reads_no_api_keys() {
        let mut requested = Vec::new();

        let config = transcription_config_from_settings(
            "whisper_cpp".into(),
            Some("/tmp/ggml.bin".into()),
            |key_id| {
                requested.push(key_id.to_string());
                Some(format!("{key_id}-value"))
            },
        );

        assert!(requested.is_empty());
        assert_eq!(config.groq_api_key, None);
        assert_eq!(config.openai_api_key, None);
        assert_eq!(
            config.whisper_cpp_model_path.as_deref(),
            Some("/tmp/ggml.bin")
        );
    }

    #[test]
    fn doubao_streaming_config_without_token_returns_none_for_batch_fallback() {
        let mut requested = Vec::new();

        let config = doubao_streaming_config_from_settings(
            "wss://example.test".into(),
            "resource".into(),
            |key_id| {
                requested.push(key_id.to_string());
                None
            },
        );

        assert!(config.is_none());
        assert_eq!(
            requested,
            vec![doubao_config::SECRET_API_KEY_OR_ACCESS_TOKEN]
        );
    }

    #[test]
    fn doubao_streaming_config_reads_token_then_optional_app_id_by_default() {
        let mut requested = Vec::new();

        let config = doubao_streaming_config_from_settings(
            "wss://example.test".into(),
            "resource".into(),
            |key_id| {
                requested.push(key_id.to_string());
                Some(format!("{key_id}-value"))
            },
        )
        .expect("token enables streaming config");

        assert_eq!(
            requested,
            vec![
                doubao_config::SECRET_API_KEY_OR_ACCESS_TOKEN,
                doubao_config::SECRET_APP_ID
            ]
        );
        assert_eq!(config.asr_provider, "doubao_stream");
        assert_eq!(
            config.doubao_endpoint.as_deref(),
            Some("wss://example.test")
        );
        assert_eq!(config.doubao_resource_id.as_deref(), Some("resource"));
        assert_eq!(
            config.doubao_api_key_or_access_token.as_deref(),
            Some("doubao_access_token-value")
        );
    }

    #[test]
    fn disabled_enhance_config_reads_no_llm_key() {
        let mut requested = Vec::new();

        let config = enhance_config_from_settings(
            "openai_compatible".into(),
            false,
            "prompt".into(),
            "https://api.example.com/v1".into(),
            "model".into(),
            |key_id| {
                requested.push(key_id.to_string());
                Some(format!("{key_id}-value"))
            },
        );

        assert!(requested.is_empty());
        assert_eq!(config.openai_compatible_api_key, None);
    }

    #[test]
    fn enabled_openai_compatible_enhance_config_reads_llm_key() {
        let mut requested = Vec::new();

        let config = enhance_config_from_settings(
            "openai_compatible".into(),
            true,
            "prompt".into(),
            "https://api.example.com/v1".into(),
            "model".into(),
            |key_id| {
                requested.push(key_id.to_string());
                Some(format!("{key_id}-value"))
            },
        );

        assert_eq!(requested, vec!["openai_compatible_api_key"]);
        assert_eq!(
            config.openai_compatible_api_key.as_deref(),
            Some("openai_compatible_api_key-value")
        );
    }
}
