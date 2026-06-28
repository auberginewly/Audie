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
use crate::managers::model::ModelManager;
use crate::managers::transcription::{TranscriptionConfig, TranscriptionManager};
use crate::platform::{current_platform, HotkeyCallback, Platform};
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
            commands::export_config,
            commands::import_config,
            commands::list_asr_providers,
            commands::list_llm_providers,
            commands::get_available_models,
            commands::get_current_local_asr_model,
            commands::set_active_local_asr_model,
            commands::download_model,
            commands::cancel_download,
            commands::delete_model,
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
            // ModelManager scans app_data_dir/models at construction so any GGML on
            // disk is usable with zero clicks. A failure (e.g. unresolvable data dir)
            // must not abort startup — degrade to an empty catalog, like history.
            let model = match ModelManager::new(&app_handle) {
                Ok(model) => model,
                Err(err) => {
                    log::error!("init ModelManager: {err:?}");
                    ModelManager::empty(&app_handle)
                }
            };
            let model = Arc::new(model);

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
            app.manage(model);
            app.manage(platform.clone());
            app.manage(Arc::new(parking_lot::Mutex::new(None::<TranscriptStream>)));
            // fe.8c: last-take store (undo / retry / insert-raw) + take generation
            // counter (mid-Processing cancel supersedes the in-flight worker).
            app.manage(Arc::new(parking_lot::Mutex::new(None::<LastTake>)));
            app.manage(Arc::new(AtomicU64::new(0)));

            let hotkey = commands::load_hotkey(&app_handle);
            if let Err(err) =
                platform.register_hotkey(&app_handle, &hotkey, build_hotkey_callback(&app_handle))
            {
                // Don't abort startup: the default trigger is fn, which needs Input
                // Monitoring. A missing grant must still let the app launch so the
                // user can grant it in Settings and relaunch (P3.9 known caveat).
                log::warn!(
                    "register trigger {hotkey} failed (grant Input Monitoring then relaunch): {err:?}"
                );
            } else {
                log::info!("registered trigger {hotkey}");
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Build the trigger-tap callback. Resolves managers off the app state instead of
/// capturing clones, so it can be rebuilt verbatim when the trigger changes —
/// see `commands::update_settings`.
pub(crate) fn build_hotkey_callback(app: &AppHandle) -> HotkeyCallback {
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
        };
        handle_hotkey(&ctx);
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
    platform.register_hotkey(&app, &hotkey, build_hotkey_callback(&app))
}

fn handle_hotkey(ctx: &HotkeyContext<'_>) {
    // Toggle control model: each trigger tap starts a take (from Idle) or finishes
    // it (from Recording). A tap mid-pipeline (Processing/Success/Error/Cancel) is
    // a no-op.
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
    // Snapshot the frontmost app NOW — before the permission gate (whose first-run
    // TCC prompt changes frontmost) and before the overlay shows. The ✓ / 撤销 /
    // 重试 button paths later make Audie frontmost, so inject needs this
    // pre-recording target to restore focus and paste at the user's caret.
    platform.capture_focus_target();

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
            record_history(&app, "success", &text, enhanced, take.duration_ms);
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
            match finish_pipeline_tail(&app, &enhance, &inject, &text, duration_ms) {
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

    let (text_to_inject, outcome) = maybe_enhance_text(app, enhance, text);

    // P0.4: inject at the caret. On failure the text is still on the
    // clipboard (§3.7 fallback) — flash Error so the user knows to paste.
    inject.inject(app, &text_to_inject)?;

    // Record the dictation (Home/History): raw transcript always, enhanced kept
    // whenever polishing actually ran (so both versions show). A history failure
    // must not break injection.
    let enhanced = matches!(outcome, EnhanceOutcome::Enhanced).then(|| text_to_inject.clone());
    record_history(app, "success", text, enhanced, duration_ms);
    Ok(matches!(outcome, EnhanceOutcome::Failed))
}

/// Persist one dictation outcome to the History store. Best-effort: a DB error only
/// logs, never propagates — history is peripheral to the inject hot path.
fn record_history(
    app: &AppHandle,
    kind: &str,
    raw_text: &str,
    enhanced_text: Option<String>,
    duration_ms: u64,
) {
    let history = app.state::<Arc<HistoryManager>>();
    if let Err(err) = history.record(app, kind, raw_text, enhanced_text, duration_ms as i64) {
        log::warn!("record history ({kind}): {err:?}");
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
    record_history(app, "success", &take.0, None, take.1);
}

/// "No content recognized" outcome: record a `kind=empty` history entry and surface
/// it on the overlay. Reuses the ERROR toast (which renders as a neutral card with no
/// action buttons for this category — not the scary red device-error treatment), so
/// the user gets immediate feedback plus a history row.
fn enter_no_content(app: AppHandle, state: Arc<StateMachine>, duration_ms: u64) {
    record_history(&app, "empty", "", None, duration_ms);
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

    // Resolve the local-ASR path: a selected catalog/custom model wins (its file in
    // app_data_dir/models), else fall back to the manually-typed path. Both stay
    // working — picking a downloaded model doesn't erase the manual escape hatch.
    let whisper_cpp_model_path = resolve_whisper_cpp_path(
        app,
        &settings.selected_local_asr_model,
        settings.whisper_cpp_model_path,
    );

    transcription_config_from_settings(
        settings.asr_provider,
        settings.asr_model,
        whisper_cpp_model_path,
        |key_id| read_optional_secret(platform.inner().as_ref(), key_id),
    )
}

/// Resolve the whisper.cpp model path: a non-empty `selected_local_asr_model` that
/// the ModelManager can map to a downloaded file takes priority; otherwise fall back
/// to the manual `whisper_cpp_model_path` (a stale/un-downloaded selection silently
/// degrades to the manual path rather than failing here).
fn resolve_whisper_cpp_path(
    app: &AppHandle,
    selected_local_asr_model: &str,
    manual_path: Option<String>,
) -> Option<String> {
    if !selected_local_asr_model.is_empty() {
        let model = app.state::<Arc<ModelManager>>();
        if let Some(path) = model.downloaded_model_path(selected_local_asr_model) {
            return Some(path.to_string_lossy().to_string());
        }
    }
    manual_path
}

fn transcription_config_from_settings(
    asr_provider: String,
    asr_model: String,
    whisper_cpp_model_path: Option<String>,
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
        whisper_cpp_model_path,
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
        whisper_cpp_model_path: None,
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
/// system locale; 中文 as a last resort.
fn enhance_prompt_with_language(app: &AppHandle, settings: &commands::Settings) -> String {
    let language = if settings.primary_language.trim().is_empty() {
        app.state::<Arc<dyn Platform>>()
            .inner()
            .system_language()
            .unwrap_or_else(|| "中文".to_string())
    } else {
        settings.primary_language.clone()
    };
    format!("用户主要语言：{language}\n\n{}", settings.enhance_prompt)
}

fn enhance_config(app: &AppHandle) -> EnhanceConfig {
    let settings = commands::load_settings(app);
    let enhance_prompt = enhance_prompt_with_language(app, &settings);
    let platform = app.state::<Arc<dyn Platform>>();

    enhance_config_from_settings(
        settings.llm_provider,
        settings.enhance_enabled,
        enhance_prompt,
        settings.openai_compatible_base_url,
        settings.openai_compatible_model,
        settings.llm_api_key_id,
        |key_id| read_optional_secret(platform.inner().as_ref(), key_id),
    )
}

/// Enhance config for the History 重试 — same prompt/provider, but the LLM key is
/// read regardless of the auto-enhance toggle (the user explicitly asked to polish
/// this entry). `enhance_enabled` is forced true so the key is fetched.
fn reenhance_config(app: &AppHandle) -> EnhanceConfig {
    let settings = commands::load_settings(app);
    let enhance_prompt = enhance_prompt_with_language(app, &settings);
    let platform = app.state::<Arc<dyn Platform>>();

    enhance_config_from_settings(
        settings.llm_provider,
        true,
        enhance_prompt,
        settings.openai_compatible_base_url,
        settings.openai_compatible_model,
        settings.llm_api_key_id,
        |key_id| read_optional_secret(platform.inner().as_ref(), key_id),
    )
}

fn enhance_config_from_settings(
    llm_provider: String,
    enhance_enabled: bool,
    enhance_prompt: String,
    openai_compatible_base_url: String,
    openai_compatible_model: String,
    llm_api_key_id: String,
    mut read_secret: impl FnMut(&str) -> Option<String>,
) -> EnhanceConfig {
    // Read the active provider's own key (4b). Empty id = key-optional local
    // provider (Ollama / LM Studio) → no key; build_provider allows that for
    // localhost endpoints.
    let openai_compatible_api_key =
        if enhance_enabled && llm_provider == "openai_compatible" && !llm_api_key_id.is_empty() {
            read_secret(&llm_api_key_id)
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

        let config =
            transcription_config_from_settings("groq".into(), String::new(), None, |key_id| {
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

        let config =
            transcription_config_from_settings("openai".into(), String::new(), None, |key_id| {
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

        let config =
            transcription_config_from_settings("glm".into(), String::new(), None, |key_id| {
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

        let config = transcription_config_from_settings(
            "aliyun_fun".into(),
            String::new(),
            None,
            |key_id| {
                requested.push(key_id.to_string());
                Some(format!("{key_id}-value"))
            },
        );

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
            transcription_config_from_settings("stepfun".into(), String::new(), None, |key_id| {
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
    fn whisper_cpp_transcription_config_reads_no_api_keys() {
        let mut requested = Vec::new();

        let config = transcription_config_from_settings(
            "whisper_cpp".into(),
            String::new(),
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
    fn disabled_enhance_config_reads_no_llm_key() {
        let mut requested = Vec::new();

        let config = enhance_config_from_settings(
            "openai_compatible".into(),
            false,
            "prompt".into(),
            "https://api.example.com/v1".into(),
            "model".into(),
            "deepseek_api_key".into(),
            |key_id| {
                requested.push(key_id.to_string());
                Some(format!("{key_id}-value"))
            },
        );

        assert!(requested.is_empty());
        assert_eq!(config.openai_compatible_api_key, None);
    }

    #[test]
    fn enabled_openai_compatible_enhance_config_reads_active_provider_key() {
        let mut requested = Vec::new();

        // 4b: reads the per-provider key id from settings, not a hardcoded one.
        let config = enhance_config_from_settings(
            "openai_compatible".into(),
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
        assert_eq!(
            config.openai_compatible_api_key.as_deref(),
            Some("deepseek_api_key-value")
        );
    }

    #[test]
    fn empty_llm_key_id_reads_no_key_for_local_provider() {
        let mut requested = Vec::new();

        // Ollama / LM Studio: empty key id = key-optional local provider, no read.
        let config = enhance_config_from_settings(
            "openai_compatible".into(),
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
        assert_eq!(config.openai_compatible_api_key, None);
    }
}
