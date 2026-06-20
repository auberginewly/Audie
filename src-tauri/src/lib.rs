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

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition};
use tauri_plugin_global_shortcut::ShortcutState;

use crate::asr::AudioData;
use crate::error::{AppError, AppResult};
use crate::managers::audio::AudioManager;
use crate::managers::enhance::{fallback_after_enhance_failure, EnhanceConfig, EnhanceManager};
use crate::managers::inject::InjectManager;
use crate::managers::transcription::{TranscriptionConfig, TranscriptionManager};
use crate::platform::{current_platform, HotkeyCallback, HotkeyEvent, HotkeyRegistry, Platform};
use crate::state::{AppState, StateMachine};

const SUCCESS_HOLD_MS: u64 = 150;
const ERROR_HOLD_MS: u64 = 2500;

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

const OVERLAY_WINDOW_LABEL: &str = "overlay";
const OVERLAY_BOTTOM_MARGIN_PX: f64 = 16.0;

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
            commands::delete_secret,
            provider_test::test_provider,
        ])
        .setup(move |app| {
            let app_handle = app.handle().clone();

            if let Err(err) = position_overlay(&app_handle) {
                log::error!("position overlay failed: {err:?}");
                return Err(Box::new(std::io::Error::other(format!("{err:?}"))));
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
        handle_hotkey(
            &app,
            state.inner(),
            audio.inner(),
            transcription.inner(),
            enhance.inner(),
            inject.inner(),
            event,
        );
    })
}

fn handle_hotkey(
    app: &AppHandle,
    state: &Arc<StateMachine>,
    audio: &Arc<AudioManager>,
    transcription: &Arc<TranscriptionManager>,
    enhance: &Arc<EnhanceManager>,
    inject: &Arc<InjectManager>,
    event: HotkeyEvent,
) {
    match event {
        HotkeyEvent::Pressed => {
            // Press enters the front half of the pipeline: permission gate →
            // open cpal stream → Recording state → overlay. No ASR happens until
            // Release, because P1 still uses batch transcription.
            // Gate on mic permission before recording: a denial otherwise
            // captures silence and the user only sees a Whisper hallucination.
            // Flash red instead (§3.7 Permission).
            let platform = app.state::<Arc<dyn Platform>>();
            if !platform.ensure_microphone_permission() {
                let _ = show_overlay(app);
                enter_error(
                    app.clone(),
                    state.clone(),
                    AppError::Permission("请授予麦克风权限".into()),
                );
                return;
            }
            // Start capture BEFORE the Idle→Recording transition: a cpal failure
            // (no input device, build_input_stream blew up, etc.) needs to surface
            // as Idle→Error (§3.7 Device) which is only legal from Idle. Doing the
            // transition first would strand us in Recording with a dead stream.
            if let Err(err) = audio.start_capture(app.clone()) {
                log::error!("start capture: {err:?}");
                let _ = show_overlay(app);
                enter_error(app.clone(), state.clone(), err);
                return;
            }
            if state.transition(app, AppState::Recording, Some("hotkey-down")) {
                if let Err(err) = show_overlay(app) {
                    log::error!("show overlay: {err:?}");
                }
            } else {
                // Transition rejected (shouldn't happen — we just confirmed Idle
                // implicitly by reaching here). Tear down the capture we just opened.
                if let Err(err) = audio.stop_capture() {
                    log::warn!("rollback stop_capture: {err:?}");
                }
            }
        }
        HotkeyEvent::Released => {
            // Release closes the audio session and hands one complete utterance
            // to the pipeline tail. From here on the overlay remains visible so
            // the user sees Processing/Success/Error instead of a blink.
            if !state.transition(app, AppState::Processing, Some("hotkey-up")) {
                return;
            }
            // Overlay stays up through Processing/Success/Error so the user can
            // see the result; it's hidden only when we settle back to Idle.
            match audio.stop_capture() {
                Ok(recorded) => {
                    spawn_transcription(
                        app.clone(),
                        state.clone(),
                        transcription.clone(),
                        enhance.clone(),
                        inject.clone(),
                        recorded,
                    );
                }
                Err(err) => {
                    log::error!("stop capture: {err:?}");
                    enter_error(app.clone(), state.clone(), err);
                }
            }
        }
    }
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

        let config = transcription_config(&app);
        let text = match transcription.transcribe(&audio, &config) {
            Ok(text) => text,
            Err(err) => {
                log::error!("transcription failed: {err:?}");
                enter_error(app, state, err);
                return;
            }
        };

        // P0.3 acceptance: the transcript shows up in the console.
        println!("[transcript] {text}");
        log::info!("transcript ({duration_ms} ms): {text}");
        let _ = app.emit(
            "final-transcript",
            FinalTranscript {
                text: text.clone(),
                duration_ms,
            },
        );

        let text_to_inject = maybe_enhance_text(&app, &enhance, &text);

        // P0.4: inject at the caret. On failure the text is still on the
        // clipboard (§3.7 fallback) — flash Error so the user knows to paste.
        if let Err(err) = inject.inject(&app, &text_to_inject) {
            log::error!("inject failed: {err:?}");
            enter_error(app, state, err);
            return;
        }

        state.transition(&app, AppState::Success, Some("injected"));
        thread::sleep(Duration::from_millis(SUCCESS_HOLD_MS));
        settle_to_idle(&app, &state, "done");
    });
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
    }
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

    // Click-through — the capsule must not steal events from the underlying app.
    overlay
        .set_ignore_cursor_events(true)
        .map_err(|err| AppError::Internal(format!("set_ignore_cursor_events: {err}")))?;

    Ok(())
}

fn show_overlay(app: &AppHandle) -> AppResult<()> {
    let overlay = app
        .get_webview_window(OVERLAY_WINDOW_LABEL)
        .ok_or_else(|| AppError::Internal("overlay window not found".into()))?;
    overlay
        .show()
        .map_err(|err| AppError::Internal(format!("show overlay: {err}")))?;
    Ok(())
}

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
