// Tauri entry. `main.rs` only calls `run()` — all setup happens here.
//
// Trigger wiring (P3.9):
//   - Platform CGEventTap → HotkeyEvent::Pressed → handle_hotkey toggle
//   - Press → state Idle→Recording → show overlay window
//   - Second press → Recording→Processing → transcribe/inject
//   - Overlay window positioned bottom-center, click-through

mod asr;
mod commands;
mod error;
mod llm;
mod managers;
mod platform;
mod provider_test;
mod state;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition};

use crate::asr::doubao::config as doubao_config;
use crate::asr::{AudioData, TranscriptStream};
use crate::error::{AppError, AppResult};
use crate::managers::audio::AudioManager;
use crate::managers::enhance::{fallback_after_enhance_failure, EnhanceConfig, EnhanceManager};
use crate::managers::history::HistoryManager;
use crate::managers::inject::InjectManager;
use crate::managers::transcription::{TranscriptionConfig, TranscriptionManager};
use crate::platform::{current_platform, HotkeyCallback, HotkeySlot, Platform};
use crate::state::{AppState, StateMachine};

const SUCCESS_HOLD_MS: u64 = 150;
// Terminal toasts (error / cancelled / polish-unavailable) carry actions
// (重试 / 撤销操作 / 去设置), so they linger long enough to read and click before
// the overlay auto-settles to Idle. A user action transitions the state first,
// which turns the pending settle into a no-op (see `settle_to_idle`).
const TERMINAL_HOLD_MS: u64 = 6000;

type ActiveStreamingSession = Arc<parking_lot::Mutex<Option<TranscriptStream>>>;

/// The last utterance, kept so a terminal toast can resume it: 撤销操作 (after a
/// cancel), 重试 (after an error), or 插入原文. Stored the moment recording
/// finishes; `transcript` fills in once ASR returns (None if it never got there).
#[derive(Clone)]
struct LastTake {
    audio: AudioData,
    transcript: Option<String>,
    duration_ms: u64,
}

type LastTakeSlot = Arc<parking_lot::Mutex<Option<LastTake>>>;

/// Monotonic id of the take that may still inject. A worker captures its id when
/// it spawns and only injects while the shared counter still equals it; tapping ✕
/// mid-Processing bumps the counter, superseding the worker without racing it.
type TakeGen = Arc<AtomicU64>;

/// What to do with the current take. `Polish` cleans the transcript (default fn key;
/// 「AI 润色」开关开且配了 LLM 才润色，否则纯转写); `Compose` generates prose from spoken points (写作键).
/// 片2 adds `Rewrite`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DictationMode {
    Polish,
    Rewrite,
    Compose,
}

impl DictationMode {
    /// History `mode` column value (片2 adds `Rewrite => "rewrite"`).
    fn as_str(self) -> &'static str {
        match self {
            DictationMode::Polish => "polish",
            DictationMode::Rewrite => "rewrite",
            DictationMode::Compose => "compose",
        }
    }
}

/// Which trigger fired — set on the press that starts a take. `start_recording`
/// resolves it to a `DictationMode`; 片2 will branch Primary on the selection state
/// (有选中 → 改写). `pub(crate)` so `commands::update_settings` can rebuild callbacks.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum HotkeyRole {
    Primary,
    Compose,
}

/// The in-flight take's mode, set when recording starts (which trigger fired) and
/// read by the pipeline tail. A toggle keeps it across the start→finish gap.
type ActiveModeSlot = Arc<parking_lot::Mutex<DictationMode>>;

/// 改写模式抓到的选中文字：start_recording 探选中时存，finish 拼进 LLM 输入后用。
type RewriteSourceSlot = Arc<parking_lot::Mutex<Option<String>>>;

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
    last_take: &'a LastTakeSlot,
    take_gen: &'a TakeGen,
    active_mode: &'a ActiveModeSlot,
}

const OVERLAY_WINDOW_LABEL: &str = "overlay";
const OVERLAY_BOTTOM_MARGIN_PX: f64 = 4.0;
// Last bottom-center origin (x, y) the overlay panel was placed at. The follow
// loop compares the cursor-screen *target* to this — not the panel's live frame,
// which macOS animates during Space switches — so it only moves on a real screen
// change and never fights the swipe animation.
#[cfg(target_os = "macos")]
static OVERLAY_LAST_TARGET: parking_lot::Mutex<Option<(f64, f64)>> = parking_lot::Mutex::new(None);

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

    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            // Frontend → Rust goes through commands. The hot path itself is not
            // command-driven: global-shortcut events below enter `handle_hotkey`,
            // while settings/keychain/provider-test stay here as explicit UI calls.
            commands::get_settings,
            commands::update_settings,
            commands::list_asr_providers,
            commands::list_llm_providers,
            commands::list_microphones,
            commands::auto_input_device,
            commands::list_history,
            commands::delete_history_entry,
            commands::clear_history,
            commands::get_usage_stats,
            commands::start_mic_monitor,
            commands::stop_mic_monitor,
            commands::set_secret,
            commands::has_secret,
            commands::get_secret_for_settings,
            commands::delete_secret,
            #[cfg(debug_assertions)]
            commands::test_doubao_streaming,
            #[cfg(debug_assertions)]
            commands::start_trigger_probe,
            #[cfg(debug_assertions)]
            commands::stop_trigger_probe,
            commands::get_input_monitoring_status,
            commands::request_input_monitoring_permission,
            commands::get_microphone_permission_status,
            commands::request_microphone_permission,
            commands::get_accessibility_permission_status,
            commands::request_accessibility_permission,
            begin_trigger_capture,
            end_trigger_capture,
            provider_test::test_provider,
            provider_test::list_provider_models,
            provider_test::discover_local_llm,
            commands::test_doubao_connection,
            // Overlay capsule controls (fe.8b / fe.8c).
            confirm_recording,
            cancel_recording,
            undo_last,
            retry_last,
            insert_raw_last,
            open_main_window,
            reenhance_history_entry,
        ])
        .setup(move |app| {
            let app_handle = app.handle().clone();

            if let Err(err) = position_overlay(&app_handle) {
                log::error!("position overlay failed: {err:?}");
                return Err(Box::new(std::io::Error::other(format!("{err:?}"))));
            }

            // Convert the overlay into a non-activating NSPanel so clicking the
            // capsule buttons (✕/✓) never activates Audie / steals focus (fe.8b-2),
            // then start the thread that keeps it on the cursor's display.
            #[cfg(target_os = "macos")]
            {
                convert_overlay_to_panel(app);
                spawn_overlay_follow_thread(app.handle().clone());
            }

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
            let history = Arc::new(HistoryManager::new(&app_handle));
            let platform: Arc<dyn Platform> = Arc::from(current_platform());
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
            app.manage(history);
            app.manage(inject);
            app.manage(platform.clone());
            app.manage(Arc::new(parking_lot::Mutex::new(None::<TranscriptStream>)));
            // fe.8c: last-take store (undo / retry / insert-raw) + take generation
            // counter (mid-Processing cancel supersedes the in-flight worker).
            app.manage(Arc::new(parking_lot::Mutex::new(None::<LastTake>)));
            app.manage(Arc::new(AtomicU64::new(0)));
            // 写作/润色 mode of the in-flight take (set on the press that starts it).
            app.manage::<ActiveModeSlot>(Arc::new(parking_lot::Mutex::new(DictationMode::Polish)));
            app.manage::<RewriteSourceSlot>(Arc::new(parking_lot::Mutex::new(None)));

            let hotkey = commands::load_hotkey(&app_handle);
            if let Err(err) = platform.register_hotkey(
                &app_handle,
                HotkeySlot::Primary,
                &hotkey,
                build_hotkey_callback(&app_handle, HotkeyRole::Primary),
            ) {
                // Don't abort startup: the default trigger is fn, which needs Input
                // Monitoring. A missing grant must still let the app launch so the
                // user can grant it in Settings and relaunch (P3.9 known caveat).
                log::warn!(
                    "register trigger {hotkey} failed (grant Input Monitoring then relaunch): {err:?}"
                );
            } else {
                log::info!("registered trigger {hotkey}");
            }
            // 写作键（HotkeySlot::Compose）— only registered when enabled + configured.
            register_compose_hotkey(&app_handle);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Build the trigger-tap callback. Resolves managers off the app state instead of
/// capturing clones, so it can be rebuilt verbatim when the trigger changes —
/// see `commands::update_settings`.
pub(crate) fn build_hotkey_callback(app: &AppHandle, role: HotkeyRole) -> HotkeyCallback {
    let app = app.clone();
    Box::new(move || {
        let state = app.state::<Arc<StateMachine>>();
        let audio = app.state::<Arc<AudioManager>>();
        let transcription = app.state::<Arc<TranscriptionManager>>();
        let enhance = app.state::<Arc<EnhanceManager>>();
        let inject = app.state::<Arc<InjectManager>>();
        let streaming = app.state::<ActiveStreamingSession>();
        let last_take = app.state::<LastTakeSlot>();
        let take_gen = app.state::<TakeGen>();
        let active_mode = app.state::<ActiveModeSlot>();
        let ctx = HotkeyContext {
            app: &app,
            state: state.inner(),
            audio: audio.inner(),
            transcription: transcription.inner(),
            enhance: enhance.inner(),
            inject: inject.inner(),
            streaming: streaming.inner(),
            last_take: last_take.inner(),
            take_gen: take_gen.inner(),
            active_mode: active_mode.inner(),
        };
        handle_hotkey(&ctx, role);
    })
}

/// P3.10 trigger recorder: while the Settings recorder is open, stop the live
/// trigger and run a listen-only capture tap (macOS) that emits `trigger-captured`
/// / `trigger-capture-rejected` for whatever key / combo the user presses — the
/// webview can't see fn, so all capture is native. `end_trigger_capture` stops the
/// capture tap and restores the real trigger.
#[tauri::command]
fn begin_trigger_capture(app: AppHandle) -> AppResult<()> {
    let platform = app.state::<Arc<dyn Platform>>();
    platform.unregister_all_hotkeys(&app)?;
    platform.start_trigger_capture(&app)
}

#[tauri::command]
fn end_trigger_capture(app: AppHandle) -> AppResult<()> {
    let platform = app.state::<Arc<dyn Platform>>();
    platform.stop_trigger_capture();
    let hotkey = commands::load_hotkey(&app);
    platform.register_hotkey(
        &app,
        HotkeySlot::Primary,
        &hotkey,
        build_hotkey_callback(&app, HotkeyRole::Primary),
    )?;
    register_compose_hotkey(&app);
    Ok(())
}

/// Register the 写作 (compose) trigger when enabled + configured. A missing Input
/// Monitoring grant only logs (like the primary trigger) so startup / capture-restore
/// never abort. Reads the persisted settings, so callers must persist before calling
/// (startup + capture-restore both do).
fn register_compose_hotkey(app: &AppHandle) {
    let settings = commands::load_settings(app);
    if settings.compose_hotkey.trim().is_empty() {
        return;
    }
    let platform = app.state::<Arc<dyn Platform>>();
    match platform.register_hotkey(
        app,
        HotkeySlot::Compose,
        &settings.compose_hotkey,
        build_hotkey_callback(app, HotkeyRole::Compose),
    ) {
        Ok(()) => log::info!("registered 写作键 {}", settings.compose_hotkey),
        Err(err) => log::warn!(
            "register 写作键 {} failed (grant Input Monitoring then relaunch): {err:?}",
            settings.compose_hotkey
        ),
    }
}

fn handle_hotkey(ctx: &HotkeyContext<'_>, role: HotkeyRole) {
    // Toggle control model: each trigger tap starts a take (from Idle) or finishes
    // it (from Recording). A tap mid-pipeline (Processing/Success/Error/Cancel) is
    // a no-op. `role` (which key fired) only matters when starting — it picks the mode.
    match ctx.state.current() {
        AppState::Idle => start_recording(ctx, role),
        AppState::Recording => finish_recording(ctx),
        _ => {}
    }
}

/// Enter the front half of the pipeline: permission gate → open cpal stream →
/// Recording state → overlay. No ASR happens until finish, because P1 uses
/// batch transcription.
fn start_recording(ctx: &HotkeyContext<'_>, role: HotkeyRole) {
    let platform = ctx.app.state::<Arc<dyn Platform>>();
    // Snapshot the frontmost app NOW — before probing the selection (read_selection's
    // synthetic Cmd+C targets it), before the permission gate (whose first-run TCC
    // prompt changes frontmost), and before the overlay. inject restores focus here.
    platform.capture_focus_target();

    // Which trigger fired + the selection state decide the mode. 写作键 → 写作; fn with a
    // selection → 改写 (grab the selected text now, replace it on finish); fn with no
    // selection → 润色. read_selection probes via the clipboard (片2).
    let mode = match role {
        HotkeyRole::Compose => DictationMode::Compose,
        HotkeyRole::Primary => match platform.read_selection(ctx.app) {
            Some(sel) if !sel.trim().is_empty() => {
                *ctx.app.state::<RewriteSourceSlot>().lock() = Some(sel);
                DictationMode::Rewrite
            }
            _ => DictationMode::Polish,
        },
    };
    if !matches!(mode, DictationMode::Rewrite) {
        *ctx.app.state::<RewriteSourceSlot>().lock() = None;
    }
    *ctx.active_mode.lock() = mode;

    // Gate on mic permission before recording: a denial otherwise captures silence
    // and the user only sees a Whisper hallucination. Flash red instead (§3.7).

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
    // Free the mic if the Settings preview monitor is running — recording owns it.
    ctx.audio.stop_monitor();
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
    // Claim a fresh take id: this worker injects only while the counter still
    // equals it, so a mid-Processing ✕ (which bumps the counter) supersedes it.
    let my_gen = ctx.take_gen.fetch_add(1, Ordering::SeqCst) + 1;
    let mode = *ctx.active_mode.lock();
    match ctx.audio.stop_capture() {
        Ok(recorded) => {
            spawn_transcription(
                ctx.app.clone(),
                ctx.state.clone(),
                ctx.transcription.clone(),
                ctx.enhance.clone(),
                ctx.inject.clone(),
                ctx.last_take.clone(),
                ctx.take_gen.clone(),
                my_gen,
                recorded,
                streaming_result,
                mode,
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
    let last_take = app.state::<LastTakeSlot>();
    let take_gen = app.state::<TakeGen>();
    let active_mode = app.state::<ActiveModeSlot>();
    let ctx = HotkeyContext {
        app,
        state: state.inner(),
        audio: audio.inner(),
        transcription: transcription.inner(),
        enhance: enhance.inner(),
        inject: inject.inner(),
        streaming: streaming.inner(),
        last_take: last_take.inner(),
        take_gen: take_gen.inner(),
        active_mode: active_mode.inner(),
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

/// ✕ on the capsule — cancel into a "已取消" toast that offers 撤销操作. The take
/// is always kept (lazy: from Recording we just stash the raw audio without
/// transcribing; mid-Processing the worker already stored it), so undo can
/// resume it. The toast auto-settles to Idle after `TERMINAL_HOLD_MS`.
#[tauri::command]
fn cancel_recording(app: AppHandle) {
    with_hotkey_ctx(&app, |ctx| match ctx.state.current() {
        AppState::Recording => {
            // Lazy cancel: stop capture and keep the raw buffer; undo re-runs batch
            // ASR on it, so we don't burn a transcription the user may not want.
            clear_streaming_session(ctx.streaming);
            match ctx.audio.stop_capture() {
                Ok(recorded) => {
                    let duration_ms = duration_ms(&recorded);
                    *ctx.last_take.lock() = Some(LastTake {
                        audio: recorded,
                        transcript: None,
                        duration_ms,
                    });
                }
                Err(err) => log::warn!("cancel stop_capture: {err:?}"),
            }
            if ctx
                .state
                .transition(ctx.app, AppState::Cancel, Some("overlay-cancel"))
            {
                record_cancelled_transcript(ctx.app, ctx.last_take);
                spawn_settle_after(ctx.app.clone(), ctx.state.clone(), TERMINAL_HOLD_MS);
            }
        }
        AppState::Processing => {
            // Mid-pipeline: bump the take id so the in-flight worker skips injection
            // (it already stored the take), then show the cancelled toast.
            ctx.take_gen.fetch_add(1, Ordering::SeqCst);
            if ctx
                .state
                .transition(ctx.app, AppState::Cancel, Some("overlay-cancel-processing"))
            {
                record_cancelled_transcript(ctx.app, ctx.last_take);
                spawn_settle_after(ctx.app.clone(), ctx.state.clone(), TERMINAL_HOLD_MS);
            }
        }
        _ => {}
    });
}

/// Resume the kept take from a terminal toast: 撤销操作 / 重试 run the full pipeline
/// (transcribe if needed → enhance → inject); `raw_only` (插入原文) injects the
/// transcript verbatim, skipping enhance. Re-enters via Cancel/Error → Processing.
fn resume_from_last_take(app: &AppHandle, raw_only: bool) {
    with_hotkey_ctx(app, |ctx| {
        let take = match ctx.last_take.lock().clone() {
            Some(take) => take,
            None => return,
        };
        if !ctx
            .state
            .transition(ctx.app, AppState::Processing, Some("resume"))
        {
            return;
        }
        // Fresh take id so a ✕ during this resume can supersede it too.
        let my_gen = ctx.take_gen.fetch_add(1, Ordering::SeqCst) + 1;
        let app = ctx.app.clone();
        let state = ctx.state.clone();
        let transcription = ctx.transcription.clone();
        let enhance = ctx.enhance.clone();
        let inject = ctx.inject.clone();
        let take_gen = ctx.take_gen.clone();
        thread::spawn(move || {
            let text = match take.transcript {
                Some(text) => text,
                None => {
                    // Re-transcribe with the SAME model the live path picks: doubao
                    // when configured, else the user's batch asr_provider. Never
                    // silently swap to a different model (单模型不降级).
                    let config =
                        doubao_streaming_config(&app).unwrap_or_else(|| transcription_config(&app));
                    match transcription.transcribe(&take.audio, &config) {
                        Ok(text) => text,
                        Err(err) => {
                            log::error!("resume transcription failed: {err:?}");
                            enter_error(app, state, err);
                            return;
                        }
                    }
                }
            };
            if take_gen.load(Ordering::SeqCst) != my_gen {
                return; // superseded by a ✕ during the resume
            }
            // Mirror a normal finish: surface the raw transcript before enhance, so
            // undo / retry log + emit it the same way the first pass would have.
            let _ = app.emit(
                "final-transcript",
                FinalTranscript {
                    text: text.clone(),
                    duration_ms: take.duration_ms,
                },
            );
            let (to_inject, outcome) = if raw_only {
                (text.clone(), EnhanceOutcome::Disabled)
            } else {
                maybe_enhance_text(&app, &enhance, &text)
            };
            if let Err(err) = inject.inject(&app, &to_inject) {
                log::error!("resume inject failed: {err:?}");
                enter_error(app, state, err);
                return;
            }
            let enhanced = matches!(outcome, EnhanceOutcome::Enhanced).then(|| to_inject.clone());
            // resume (撤销/重试) re-runs the polish pipeline, so it records as polish.
            record_history(&app, "success", "polish", &text, enhanced, take.duration_ms);
            state.transition(&app, AppState::Success, Some("resumed"));
            let hold = if matches!(outcome, EnhanceOutcome::Failed) {
                TERMINAL_HOLD_MS
            } else {
                SUCCESS_HOLD_MS
            };
            thread::sleep(Duration::from_millis(hold));
            settle_to_idle(&app, &state, "done");
        });
    });
}

/// 撤销操作 on the cancelled toast — resume the kept take through the full pipeline.
#[tauri::command]
fn undo_last(app: AppHandle) {
    resume_from_last_take(&app, false);
}

/// 重试 on the error toast — re-run the full pipeline on the kept take.
#[tauri::command]
fn retry_last(app: AppHandle) {
    resume_from_last_take(&app, false);
}

/// 插入原文 on the error toast — inject the raw transcript, skipping enhance.
#[tauri::command]
fn insert_raw_last(app: AppHandle) {
    resume_from_last_take(&app, true);
}

/// 去设置 on the polish-unavailable toast — surface the main window and ask it to
/// open Settings (the overlay is a separate webview, so it signals via an event).
#[tauri::command]
fn open_main_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    let _ = app.emit("open-settings", ());
}

/// History 重试 — re-run the current LLM on a stored entry's transcript and save the
/// enhanced version. No audio needed (re-enhance, not re-transcribe). Errors surface
/// to the caller so the History screen can toast them; the success path emits
/// `history-updated` so the list re-fetches and shows the new 润色 box.
#[tauri::command]
fn reenhance_history_entry(app: AppHandle, id: i64) -> AppResult<()> {
    let history = app.state::<Arc<HistoryManager>>();
    let raw = history
        .raw_text_of(id)?
        .ok_or_else(|| AppError::Internal("history entry not found".into()))?;
    if raw.trim().is_empty() {
        return Err(AppError::Internal("无可润色的原文".into()));
    }
    let enhance = app.state::<Arc<EnhanceManager>>();
    let enhanced = enhance.enhance(&raw, &reenhance_config(&app))?;
    history.set_enhanced(&app, id, &enhanced)
}

/// Run the (blocking) transcription off the hotkey thread, then drive the
/// P1 pipeline tail: ASR → `final-transcript` event → optional LLM enhance →
/// inject at the caret → Success → Idle. Any failure is mapped to `error` and
/// recovers through the same Idle exit. PROJECT_SPEC.md §3.2 / §3.3 / §4.3.
#[allow(clippy::too_many_arguments)] // pipeline tail wiring; splitting it would
                                     // just shuffle the same handles around.
fn spawn_transcription(
    app: AppHandle,
    state: Arc<StateMachine>,
    transcription: Arc<TranscriptionManager>,
    enhance: Arc<EnhanceManager>,
    inject: Arc<InjectManager>,
    last_take: LastTakeSlot,
    take_gen: TakeGen,
    my_gen: u64,
    audio: AudioData,
    streaming_result: Option<TranscriptStream>,
    mode: DictationMode,
) {
    thread::spawn(move || {
        let duration_ms = duration_ms(&audio);

        // Keep the take from the start so a terminal toast can always resume it,
        // even if ASR fails (重试 re-transcribes) or the user cancels mid-flight.
        *last_take.lock() = Some(LastTake {
            audio: audio.clone(),
            transcript: None,
            duration_ms,
        });

        // AirPods on A2DP (no HFP switch yet) and a few other broken-mic states
        // produce a buffer of literal zeros — nothing recognizable. Detect digital
        // silence here (avoids an API call that returns "Thank you." hallucinations)
        // and treat it as "no content recognized". Threshold is "any sample exceeds
        // 1e-4" — real mic noise floor sits well above that, so a healthy 200ms
        // recording always passes.
        if duration_ms >= 200 && is_digital_silence(&audio) {
            enter_no_content(app, state, duration_ms);
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

        // ASR ran but found no speech — record a "没有识别到内容" entry, don't inject.
        if text.trim().is_empty() {
            enter_no_content(app, state, duration_ms);
            return;
        }

        // Fill the transcript in so 插入原文 / 重试 (after an inject failure) and
        // 撤销操作 (after a mid-Processing cancel) resume without re-transcribing.
        if let Some(take) = last_take.lock().as_mut() {
            take.transcript = Some(text.clone());
        }

        // Superseded by a ✕ during Processing — keep the take, skip injection, and
        // stay in the cancelled toast (which offers 撤销操作).
        if take_gen.load(Ordering::SeqCst) != my_gen {
            return;
        }

        let polish_unavailable =
            match finish_pipeline_tail(&app, &enhance, &inject, &text, duration_ms, mode) {
                Ok(failed) => failed,
                Err(err) => {
                    log::error!("inject failed: {err:?}");
                    enter_error(app, state, err);
                    return;
                }
            };

        state.transition(&app, AppState::Success, Some("injected"));
        // polish-unavailable is a Success that shows the amber 去设置 toast, so it
        // needs the longer hold; a clean success just flashes ✓.
        let hold = if polish_unavailable {
            TERMINAL_HOLD_MS
        } else {
            SUCCESS_HOLD_MS
        };
        thread::sleep(Duration::from_millis(hold));
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

/// Returns `true` when enhance fell back to the raw transcript (polish-unavailable
/// → the amber 去设置 toast); `false` for a clean inject.
fn finish_pipeline_tail(
    app: &AppHandle,
    enhance: &EnhanceManager,
    inject: &InjectManager,
    text: &str,
    duration_ms: u64,
    mode: DictationMode,
) -> AppResult<bool> {
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

    // 润色 cleans the transcript (配了 LLM 才润色，没配静默纯转写); 写作 generates from it
    // (写作键按下即生成). On LLM failure both fall back to raw text + the amber "去设置"
    // toast (Failed). 没配 LLM 的润色走 Disabled —— 静默注入原文、无 toast。
    let (text_to_inject, outcome) = match mode {
        DictationMode::Polish => maybe_enhance_text(app, enhance, text),
        DictationMode::Rewrite => rewrite_text(app, enhance, text),
        DictationMode::Compose => compose_text(app, enhance, text),
    };

    // P0.4: inject at the caret. On failure the text is still on the
    // clipboard (§3.7 fallback) — flash Error so the user knows to paste.
    inject.inject(app, &text_to_inject)?;

    // Record the dictation (Home/History): raw transcript always, enhanced kept
    // whenever polishing actually ran (so both versions show). A history failure
    // must not break injection.
    let enhanced = matches!(outcome, EnhanceOutcome::Enhanced).then(|| text_to_inject.clone());
    record_history(app, "success", mode.as_str(), text, enhanced, duration_ms);
    Ok(matches!(outcome, EnhanceOutcome::Failed))
}

/// Persist one dictation outcome to the History store. Best-effort: a DB error only
/// logs, never propagates — history is peripheral to the inject hot path.
fn record_history(
    app: &AppHandle,
    kind: &str,
    mode: &str,
    raw_text: &str,
    enhanced_text: Option<String>,
    duration_ms: u64,
) {
    let history = app.state::<Arc<HistoryManager>>();
    if let Err(err) = history.record(app, kind, mode, raw_text, enhanced_text, duration_ms as i64) {
        log::warn!("record history ({kind}/{mode}): {err:?}");
    }
}

/// On cancel, preserve the transcript if one already exists (a mid-Processing cancel
/// where ASR returned, or streaming had a final) as a normal history entry — the user
/// keeps the text even though it wasn't injected. A cancel with no transcript (the
/// usual mid-recording ✕) records nothing. Best-effort; no change to the cancel path.
fn record_cancelled_transcript(app: &AppHandle, last_take: &LastTakeSlot) {
    let take = match last_take.lock().as_ref() {
        Some(take) => match &take.transcript {
            Some(text) if !text.trim().is_empty() => (text.clone(), take.duration_ms),
            _ => return,
        },
        None => return,
    };
    record_history(app, "success", "polish", &take.0, None, take.1);
}

/// "No content recognized" outcome: record a `kind=empty` history entry and surface
/// it on the overlay. Reuses the ERROR toast (which renders as a neutral card with no
/// action buttons for this category — not the scary red device-error treatment), so
/// the user gets immediate feedback plus a history row.
fn enter_no_content(app: AppHandle, state: Arc<StateMachine>, duration_ms: u64) {
    record_history(&app, "empty", "polish", "", None, duration_ms);
    enter_error(app, state, AppError::Device("没有识别到内容".into()));
}

/// What happened to the transcript on the enhance step — distinguishes "polishing
/// was off" from "polished" (both inject the text, but only the latter has an
/// enhanced version worth storing) from "polish failed → raw fallback".
enum EnhanceOutcome {
    Disabled,
    Enhanced,
    Failed,
}

/// Returns the text to inject and the enhance outcome.
fn maybe_enhance_text(
    app: &AppHandle,
    enhance: &EnhanceManager,
    text: &str,
) -> (String, EnhanceOutcome) {
    let config = enhance_config(app);
    if !config.enhance_enabled {
        return (text.to_string(), EnhanceOutcome::Disabled);
    }

    emit_enhance_progress(app, "started", "润色中…");
    match enhance.enhance(text, &config) {
        Ok(enhanced) => {
            emit_enhance_progress(app, "completed", "润色完成");
            (enhanced, EnhanceOutcome::Enhanced)
        }
        Err(err) => {
            log::warn!("enhance failed, injecting original transcript: {err:?}");
            let fallback = fallback_after_enhance_failure(text, &err);
            emit_enhance_progress(app, "failed", &fallback.message);
            (fallback.text_to_inject, EnhanceOutcome::Failed)
        }
    }
}

/// 写作 compose: generate prose from the spoken points via the compose prompt
/// (生成立场; compose_config sets force_enabled=true). Mirrors `maybe_enhance_text` but
/// always runs the LLM (写作键按下即生成). On failure, inject the raw points + Failed so
/// the "去设置" toast surfaces (片1 reuses 润色's fallback text).
fn compose_text(app: &AppHandle, enhance: &EnhanceManager, text: &str) -> (String, EnhanceOutcome) {
    let config = compose_config(app);
    emit_enhance_progress(app, "started", "写作中…");
    match enhance.enhance(text, &config) {
        Ok(generated) => {
            emit_enhance_progress(app, "completed", "写作完成");
            (generated, EnhanceOutcome::Enhanced)
        }
        Err(err) => {
            log::warn!("compose failed, injecting raw points: {err:?}");
            let fallback = fallback_after_enhance_failure(text, &err);
            emit_enhance_progress(app, "failed", &fallback.message);
            (fallback.text_to_inject, EnhanceOutcome::Failed)
        }
    }
}

/// 改写 rewrite: 把 start 探到的选中文字（RewriteSource）+ 口述指令拼成一段喂 rewrite
/// prompt，结果替换选中（inject 的 Cmd+V 落在仍选中的文本上）。失败兜底注入选中原文本身
/// （替换成它自己、无变化），而不是把指令插进去。
fn rewrite_text(
    app: &AppHandle,
    enhance: &EnhanceManager,
    instruction: &str,
) -> (String, EnhanceOutcome) {
    let source = app
        .state::<RewriteSourceSlot>()
        .lock()
        .take()
        .unwrap_or_default();
    let input = format!("原文：\n{source}\n\n指令：\n{instruction}");
    let config = rewrite_config(app);
    emit_enhance_progress(app, "started", "改写中…");
    match enhance.enhance(&input, &config) {
        Ok(result) => {
            emit_enhance_progress(app, "completed", "改写完成");
            (result, EnhanceOutcome::Enhanced)
        }
        Err(err) => {
            log::warn!("rewrite failed, re-injecting the original selection: {err:?}");
            let fallback = fallback_after_enhance_failure(&source, &err);
            emit_enhance_progress(app, "failed", &fallback.message);
            (fallback.text_to_inject, EnhanceOutcome::Failed)
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

    transcription_config_from_settings(settings.asr_provider, settings.asr_model, |key_id| {
        read_optional_secret(platform.inner().as_ref(), key_id)
    })
}

fn transcription_config_from_settings(
    asr_provider: String,
    asr_model: String,
    mut read_secret: impl FnMut(&str) -> Option<String>,
) -> TranscriptionConfig {
    let (groq_api_key, openai_api_key) = match asr_provider.as_str() {
        "groq" => (read_secret("groq_api_key"), None),
        "openai" => (None, read_secret("openai_api_key")),
        _ => (None, None),
    };
    // Each new cloud ASR reads only its own keychain key (and only when selected),
    // so picking another provider never touches an unrelated secret.
    let glm_api_key = (asr_provider == "glm")
        .then(|| read_secret(crate::asr::glm::SECRET_API_KEY))
        .flatten();
    let aliyun_api_key = (asr_provider == "aliyun_fun")
        .then(|| read_secret(crate::asr::aliyun::config::SECRET_API_KEY))
        .flatten();
    let stepfun_api_key = (asr_provider == "stepfun")
        .then(|| read_secret(crate::asr::stepfun::config::SECRET_API_KEY))
        .flatten();

    TranscriptionConfig {
        asr_provider,
        asr_model,
        groq_api_key,
        openai_api_key,
        doubao_endpoint: None,
        doubao_resource_id: None,
        doubao_app_id: None,
        doubao_api_key_or_access_token: None,
        glm_api_key,
        aliyun_api_key,
        stepfun_api_key,
    }
}

fn doubao_streaming_config(app: &AppHandle) -> Option<TranscriptionConfig> {
    let settings = commands::load_settings(app);
    let platform = app.state::<Arc<dyn Platform>>();
    doubao_streaming_config_from_settings(
        &settings.asr_provider,
        settings.doubao_endpoint,
        settings.doubao_resource_id,
        |key_id| read_optional_secret(platform.inner().as_ref(), key_id),
    )
}

fn doubao_streaming_config_from_settings(
    asr_provider: &str,
    endpoint: String,
    resource_id: String,
    mut read_secret: impl FnMut(&str) -> Option<String>,
) -> Option<TranscriptionConfig> {
    // Doubao streaming is opt-in: it only kicks in when the user explicitly picks
    // doubao in the model selector (asr_provider == "doubao_stream"). A saved token
    // alone must NOT hijack every recording — picking Groq/Whisper has to win.
    if asr_provider != "doubao_stream" {
        return None;
    }
    let api_key_or_access_token = read_secret(doubao_config::SECRET_API_KEY_OR_ACCESS_TOKEN)?;
    let app_id = read_secret(doubao_config::SECRET_APP_ID);

    Some(TranscriptionConfig {
        asr_provider: "doubao_stream".into(),
        // Doubao uses resource_id, not model; asr_model stays blank for it (left
        // for a future provider-specific variant mapping if豆包 ever needs one).
        asr_model: String::new(),
        groq_api_key: None,
        openai_api_key: None,
        doubao_endpoint: Some(endpoint),
        doubao_resource_id: Some(resource_id),
        doubao_app_id: app_id,
        doubao_api_key_or_access_token: Some(api_key_or_access_token),
        glm_api_key: None,
        aliyun_api_key: None,
        stepfun_api_key: None,
    })
}

/// Prepend the main language as one line so the LLM knows it, without exposing a
/// {{placeholder}} in the editable prompt. The dropdown wins; empty = follow the
/// system locale; 中文 as a last resort. Shared by 润色 and 写作.
fn prepend_language(app: &AppHandle, settings: &commands::Settings, prompt: &str) -> String {
    let language = if settings.primary_language.trim().is_empty() {
        app.state::<Arc<dyn Platform>>()
            .inner()
            .system_language()
            .unwrap_or_else(|| "中文".to_string())
    } else {
        settings.primary_language.clone()
    };
    format!("用户主要语言：{language}\n\n{prompt}")
}

fn enhance_prompt_with_language(app: &AppHandle, settings: &commands::Settings) -> String {
    prepend_language(app, settings, &settings.enhance_prompt)
}

fn enhance_config(app: &AppHandle) -> EnhanceConfig {
    let settings = commands::load_settings(app);
    let enhance_prompt = enhance_prompt_with_language(app, &settings);
    let platform = app.state::<Arc<dyn Platform>>();

    enhance_config_from_settings(
        settings.llm_provider,
        // polish：不强制；启用 = 「AI 润色」开关开 且 配了 LLM（见 enhance_config_from_settings）。
        false,
        settings.enhance_enabled,
        enhance_prompt,
        settings.openai_compatible_base_url,
        settings.openai_compatible_model,
        settings.llm_api_key_id,
        |key_id| read_optional_secret(platform.inner().as_ref(), key_id),
    )
}

/// Enhance config for the History 重试 — same prompt/provider. `force_enabled` is true
/// (the user explicitly asked to polish this entry), so it runs even when 润色 wouldn't
/// auto-trigger; a missing key then surfaces as a failure rather than silence.
fn reenhance_config(app: &AppHandle) -> EnhanceConfig {
    let settings = commands::load_settings(app);
    let enhance_prompt = enhance_prompt_with_language(app, &settings);
    let platform = app.state::<Arc<dyn Platform>>();

    enhance_config_from_settings(
        settings.llm_provider,
        true,
        // 显式触发：force 已 short-circuit，polish_toggle 不参与（不看「AI 润色」开关）。
        true,
        enhance_prompt,
        settings.openai_compatible_base_url,
        settings.openai_compatible_model,
        settings.llm_api_key_id,
        |key_id| read_optional_secret(platform.inner().as_ref(), key_id),
    )
}

/// 写作 config: same LLM provider/key as 润色, but the compose prompt (生成立场) and
/// `force_enabled` true (写作键按下即生成；没配 key 则失败兜底，不静默).
fn compose_config(app: &AppHandle) -> EnhanceConfig {
    let settings = commands::load_settings(app);
    let compose_prompt = prepend_language(app, &settings, &settings.compose_prompt);
    let platform = app.state::<Arc<dyn Platform>>();

    enhance_config_from_settings(
        settings.llm_provider,
        true,
        // 显式触发：force 已 short-circuit，polish_toggle 不参与（不看「AI 润色」开关）。
        true,
        compose_prompt,
        settings.openai_compatible_base_url,
        settings.openai_compatible_model,
        settings.llm_api_key_id,
        |key_id| read_optional_secret(platform.inner().as_ref(), key_id),
    )
}

/// 改写 config: 同 LLM provider/key，用 rewrite prompt（改写立场）+ force_enabled true
/// （改写键触发即执行；没配 key 则失败兜底，不静默）。
fn rewrite_config(app: &AppHandle) -> EnhanceConfig {
    let settings = commands::load_settings(app);
    let rewrite_prompt = prepend_language(app, &settings, &settings.rewrite_prompt);
    let platform = app.state::<Arc<dyn Platform>>();

    enhance_config_from_settings(
        settings.llm_provider,
        true,
        // 显式触发：force 已 short-circuit，polish_toggle 不参与（不看「AI 润色」开关）。
        true,
        rewrite_prompt,
        settings.openai_compatible_base_url,
        settings.openai_compatible_model,
        settings.llm_api_key_id,
        |key_id| read_optional_secret(platform.inner().as_ref(), key_id),
    )
}

// 8 args = a flat 1:1 map of the settings fields driving enhance, plus a read_secret
// injection point for tests. Grouping into a struct would shuffle the same fields for
// no clarity gain on a pure mapper, so allow the extra arg here.
#[allow(clippy::too_many_arguments)]
fn enhance_config_from_settings(
    llm_provider: String,
    force_enabled: bool,
    polish_toggle: bool,
    enhance_prompt: String,
    openai_compatible_base_url: String,
    openai_compatible_model: String,
    llm_api_key_id: String,
    mut read_secret: impl FnMut(&str) -> Option<String>,
) -> EnhanceConfig {
    // Skip the keychain read entirely when 润色 can't run anyway (toggle off and not
    // forced) — only touch the secret store when the key is actually needed (CLAUDE.md
    // keychain note). force_enabled (compose / rewrite / History 重试) always wants it.
    let want_enhance = force_enabled || polish_toggle;

    // Read the active provider's own key (4b). Empty id = key-optional local
    // provider (Ollama / LM Studio) → no key; build_provider allows that for
    // localhost endpoints.
    let openai_compatible_api_key =
        if want_enhance && llm_provider == "openai_compatible" && !llm_api_key_id.is_empty() {
            read_secret(&llm_api_key_id)
        } else {
            None
        };

    // 润色 enablement: explicit triggers (force_enabled — compose / rewrite / History
    // 重试) always run, so a missing key surfaces as a failure instead of silence.
    // Otherwise 润色 runs only when the user's 「AI 润色」 toggle is on AND the LLM is
    // configured (a key present, or a local endpoint that needs none). Toggle off =
    // 纯转写 even when a key exists (Disabled, no 去设置 toast) — 给想要原文的人选择权.
    let enhance_enabled = force_enabled
        || (polish_toggle
            && (openai_compatible_api_key.is_some()
                || crate::llm::is_local_endpoint(&openai_compatible_base_url)));

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

/// Emit the error, flash the Error toast, and auto-settle to Idle after the
/// terminal hold (the toast's 重试 / 插入原文 can act first). Spawns its own thread
/// so callers on the hotkey path don't block.
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
    spawn_settle_after(app, state, TERMINAL_HOLD_MS);
}

/// Auto-settle a terminal state to Idle after a hold. If the user acts first
/// (撤销 / 重试 → Processing), `settle_to_idle` finds a non-terminal state and
/// the timer is a no-op, so the live capsule isn't yanked away mid-resume.
fn spawn_settle_after(app: AppHandle, state: Arc<StateMachine>, hold_ms: u64) {
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(hold_ms));
        settle_to_idle(&app, &state, "recovered");
    });
}

/// Transition to Idle and hide the overlay window. The single exit point for the
/// pipeline — overlay visibility mirrors "not Idle". Hides ONLY when the Idle
/// transition actually applied: a stale settle timer firing after the user tapped
/// 撤销 / 重试 (now Processing) must not hide the capsule out from under them.
fn settle_to_idle(app: &AppHandle, state: &Arc<StateMachine>, reason: &str) {
    if state.transition(app, AppState::Idle, Some(reason)) {
        if let Err(err) = hide_overlay(app) {
            log::error!("hide overlay: {err:?}");
        }
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
    // RawNSPanel forces canBecomeKeyWindow = YES, so a button click would grab
    // keyboard focus and the synthesized Cmd+V would paste into the panel, not
    // the user's field. becomesKeyOnlyIfNeeded keeps key focus on their app.
    panel.set_becomes_key_only_if_needed(true);
    // Receive mouse-moved events so the ✕/✓ buttons get :hover states.
    panel.set_accepts_mouse_moved_events(true);
    // NSPanel hides on app deactivation by default. Our overlay is non-activating
    // so Audie is never the active app — without this it would hide+show (flicker)
    // on every Space/app switch.
    panel.set_hides_on_deactivate(false);
    // Above app windows; visible across spaces and over fullscreen apps.
    panel.set_level(25); // NSStatusWindowLevel
    panel.set_collection_behaviour(
        NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
            | NSWindowCollectionBehavior::NSWindowCollectionBehaviorStationary
            | NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary,
    );
}

/// Move the overlay panel to the bottom-center of the screen the cursor is on,
/// so on a multi-display setup the capsule flies to the active screen. Done
/// natively (NSEvent.mouseLocation + NSScreen, all in points / one coordinate
/// space) to dodge winit's multi-scale physical-pixel pitfalls. Main thread only.
#[cfg(target_os = "macos")]
// `deprecated`: cocoa types. `unexpected_cfgs`: the old `objc` crate's msg_send!
// expands a `cargo-clippy` cfg that newer rustc flags.
#[allow(deprecated, unexpected_cfgs)]
fn reposition_overlay_to_cursor_screen(panel: &tauri_nspanel::raw_nspanel::RawNSPanel) {
    use tauri_nspanel::cocoa::appkit::{NSEvent, NSScreen};
    use tauri_nspanel::cocoa::base::{id, nil};
    use tauri_nspanel::cocoa::foundation::{NSPoint, NSRect};
    use tauri_nspanel::objc::{msg_send, sel, sel_impl};

    unsafe {
        let mouse: NSPoint = NSEvent::mouseLocation(nil);
        let screens: id = NSScreen::screens(nil);
        if screens == nil {
            return;
        }
        let count: usize = msg_send![screens, count];
        // Find the screen whose frame contains the cursor; fall back to main.
        let mut target: id = nil;
        for i in 0..count {
            let screen: id = msg_send![screens, objectAtIndex: i];
            let f: NSRect = NSScreen::frame(screen);
            if mouse.x >= f.origin.x
                && mouse.x <= f.origin.x + f.size.width
                && mouse.y >= f.origin.y
                && mouse.y <= f.origin.y + f.size.height
            {
                target = screen;
                break;
            }
        }
        if target == nil {
            target = NSScreen::mainScreen(nil);
        }
        if target == nil {
            return;
        }
        // visibleFrame excludes the Dock/menu bar, so "贴底" sits above the Dock.
        let vf: NSRect = NSScreen::visibleFrame(target);
        let frame: NSRect = msg_send![panel, frame];
        let x = vf.origin.x + (vf.size.width - frame.size.width) / 2.0;
        let y = vf.origin.y + OVERLAY_BOTTOM_MARGIN_PX;
        // Move only when the *target* (cursor's screen) changed — never react to
        // the panel's live frame, which macOS animates mid-swipe. Comparing the
        // target keeps the follow loop from snapping the panel back during the
        // Space-switch animation (the flicker the user saw).
        let mut last = OVERLAY_LAST_TARGET.lock();
        let changed = last.is_none_or(|(lx, ly)| (lx - x).abs() > 1.0 || (ly - y).abs() > 1.0);
        if changed {
            *last = Some((x, y));
            drop(last);
            let _: () = msg_send![panel, setFrameOrigin: NSPoint { x, y }];
        }
    }
}

/// Live multi-display follow: while the capsule is visible, poll the cursor and
/// re-place the panel on its screen, so it flies to whatever display the user
/// moves to mid-recording. Re-placing to the same spot is a no-op, so the panel
/// only actually moves when the cursor crosses to another screen.
#[cfg(target_os = "macos")]
fn spawn_overlay_follow_thread(app: AppHandle) {
    use tauri_nspanel::ManagerExt;
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(150));
        let app2 = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Ok(panel) = app2.get_webview_panel(OVERLAY_WINDOW_LABEL) {
                if panel.is_visible() {
                    reposition_overlay_to_cursor_screen(&panel);
                }
            }
        });
    });
}

/// Place the capsule at the bottom-center of whichever monitor the cursor is on,
/// so on a multi-display setup it follows the user to the active screen. Called
/// at setup and again on every show. Interactivity is handled by the NSPanel
/// conversion in `setup`, not here.
fn position_overlay(app: &AppHandle) -> AppResult<()> {
    let overlay = app
        .get_webview_window(OVERLAY_WINDOW_LABEL)
        .ok_or_else(|| AppError::Internal("overlay window not found".into()))?;

    let monitors = overlay
        .available_monitors()
        .map_err(|err| AppError::Internal(format!("available_monitors: {err}")))?;
    let cursor = app.cursor_position().ok();
    let monitor = cursor
        .and_then(|c| {
            monitors.iter().find(|m| {
                let p = m.position();
                let s = m.size();
                c.x >= p.x as f64
                    && c.x < (p.x + s.width as i32) as f64
                    && c.y >= p.y as f64
                    && c.y < (p.y + s.height as i32) as f64
            })
        })
        .or_else(|| monitors.first())
        .ok_or_else(|| AppError::Internal("no monitor available".into()))?;

    let m_pos = monitor.position();
    let m_size = monitor.size();
    let scale = monitor.scale_factor();
    let win_size = overlay
        .outer_size()
        .map_err(|err| AppError::Internal(format!("outer_size: {err}")))?;

    // Offset by the monitor origin so the position is correct in the global
    // multi-display coordinate space.
    let bottom_margin_px = (OVERLAY_BOTTOM_MARGIN_PX * scale).round() as i32;
    let x = m_pos.x + (m_size.width as i32 - win_size.width as i32) / 2;
    let y = m_pos.y + m_size.height as i32 - win_size.height as i32 - bottom_margin_px;

    overlay
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|err| AppError::Internal(format!("set_position: {err}")))?;
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
            Ok(panel) => {
                // Fly to the screen the cursor is on, then show WITHOUT making it
                // key (so the user's app stays frontmost for injection).
                reposition_overlay_to_cursor_screen(&panel);
                panel.order_front_regardless();
            }
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

        let config = transcription_config_from_settings("groq".into(), String::new(), |key_id| {
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

        let config = transcription_config_from_settings("openai".into(), String::new(), |key_id| {
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
    fn glm_transcription_config_reads_only_glm_key() {
        let mut requested = Vec::new();

        let config = transcription_config_from_settings("glm".into(), String::new(), |key_id| {
            requested.push(key_id.to_string());
            Some(format!("{key_id}-value"))
        });

        assert_eq!(requested, vec![crate::asr::glm::SECRET_API_KEY]);
        assert_eq!(config.glm_api_key.as_deref(), Some("glm_api_key-value"));
        assert_eq!(config.aliyun_api_key, None);
        assert_eq!(config.stepfun_api_key, None);
    }

    #[test]
    fn aliyun_transcription_config_reads_only_aliyun_key() {
        let mut requested = Vec::new();

        let config =
            transcription_config_from_settings("aliyun_fun".into(), String::new(), |key_id| {
                requested.push(key_id.to_string());
                Some(format!("{key_id}-value"))
            });

        assert_eq!(requested, vec![crate::asr::aliyun::config::SECRET_API_KEY]);
        assert_eq!(
            config.aliyun_api_key.as_deref(),
            Some("aliyun_dashscope_api_key-value")
        );
        assert_eq!(config.glm_api_key, None);
    }

    #[test]
    fn stepfun_transcription_config_reads_only_stepfun_key() {
        let mut requested = Vec::new();

        let config =
            transcription_config_from_settings("stepfun".into(), String::new(), |key_id| {
                requested.push(key_id.to_string());
                Some(format!("{key_id}-value"))
            });

        assert_eq!(requested, vec![crate::asr::stepfun::config::SECRET_API_KEY]);
        assert_eq!(
            config.stepfun_api_key.as_deref(),
            Some("stepfun_api_key-value")
        );
        assert_eq!(config.glm_api_key, None);
    }

    #[test]
    fn doubao_streaming_config_without_token_returns_none_for_batch_fallback() {
        let mut requested = Vec::new();

        let config = doubao_streaming_config_from_settings(
            "doubao_stream",
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
    fn doubao_streaming_config_skips_when_provider_not_doubao_stream() {
        let mut requested = Vec::new();

        // A saved token must not activate streaming when the user picked another
        // provider — and we must not even touch the keychain in that case.
        let config = doubao_streaming_config_from_settings(
            "groq",
            "wss://example.test".into(),
            "resource".into(),
            |key_id| {
                requested.push(key_id.to_string());
                Some(format!("{key_id}-value"))
            },
        );

        assert!(config.is_none());
        assert!(requested.is_empty());
    }

    #[test]
    fn doubao_streaming_config_reads_token_then_optional_app_id_by_default() {
        let mut requested = Vec::new();

        let config = doubao_streaming_config_from_settings(
            "doubao_stream",
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
    fn polish_enabled_when_cloud_key_present() {
        let mut requested = Vec::new();

        // 润色 (force=false, 开关 on): reads the active provider's key to derive
        // enablement; a present cloud key → enabled.
        let config = enhance_config_from_settings(
            "openai_compatible".into(),
            false,
            true,
            "prompt".into(),
            "https://api.deepseek.com/v1".into(),
            "model".into(),
            "deepseek_api_key".into(),
            |key_id| {
                requested.push(key_id.to_string());
                Some(format!("{key_id}-value"))
            },
        );

        assert_eq!(requested, vec!["deepseek_api_key"]);
        assert!(config.enhance_enabled);
        assert_eq!(
            config.openai_compatible_api_key.as_deref(),
            Some("deepseek_api_key-value")
        );
    }

    #[test]
    fn polish_disabled_when_cloud_key_missing() {
        // 润色 (force=false, 开关 on): no cloud key configured → disabled = 静默纯转写.
        let config = enhance_config_from_settings(
            "openai_compatible".into(),
            false,
            true,
            "prompt".into(),
            "https://api.deepseek.com/v1".into(),
            "model".into(),
            "deepseek_api_key".into(),
            |_| None,
        );

        assert!(!config.enhance_enabled);
        assert_eq!(config.openai_compatible_api_key, None);
    }

    #[test]
    fn polish_enabled_for_local_endpoint_without_key() {
        let mut requested = Vec::new();

        // Ollama / LM Studio: a local endpoint needs no key, so 润色 is enabled even
        // with force=false (开关 on) and an empty key id (no read happens).
        let config = enhance_config_from_settings(
            "openai_compatible".into(),
            false,
            true,
            "prompt".into(),
            "http://localhost:11434/v1".into(),
            "qwen2.5".into(),
            String::new(),
            |key_id| {
                requested.push(key_id.to_string());
                Some(format!("{key_id}-value"))
            },
        );

        assert!(requested.is_empty());
        assert!(config.enhance_enabled);
        assert_eq!(config.openai_compatible_api_key, None);
    }

    #[test]
    fn forced_enable_stays_on_without_key() {
        // compose / rewrite / History 重试 (force=true): enabled even unconfigured AND
        // even with the 「AI 润色」 toggle off — force overrides it. A missing key then
        // surfaces as a failure (fallback) instead of silent passthrough.
        let config = enhance_config_from_settings(
            "openai_compatible".into(),
            true,
            false,
            "prompt".into(),
            "https://api.deepseek.com/v1".into(),
            "model".into(),
            "deepseek_api_key".into(),
            |_| None,
        );

        assert!(config.enhance_enabled);
        assert_eq!(config.openai_compatible_api_key, None);
    }

    #[test]
    fn polish_toggle_off_skips_key_read_despite_cloud_key() {
        let mut requested = Vec::new();

        // 「AI 润色」开关 off (force=false): 纯转写 even with a cloud key configured, and
        // the keychain is never touched (只在真要用 key 时才读). 给想要原文的人选择权.
        let config = enhance_config_from_settings(
            "openai_compatible".into(),
            false,
            false,
            "prompt".into(),
            "https://api.deepseek.com/v1".into(),
            "model".into(),
            "deepseek_api_key".into(),
            |key_id| {
                requested.push(key_id.to_string());
                Some(format!("{key_id}-value"))
            },
        );

        assert!(requested.is_empty());
        assert!(!config.enhance_enabled);
        assert_eq!(config.openai_compatible_api_key, None);
    }
}
