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
const STREAMING_AUDIO_QUEUE_CAPACITY: usize = 32;

type ActiveStreamingSession = Arc<parking_lot::Mutex<Option<TranscriptStream>>>;

/// The last utterance, kept so a terminal toast can resume it: 撤销操作 (after a
/// cancel), 重试 (after an error), or 插入原文. Stored the moment recording
/// finishes; `transcript` fills in once ASR returns (None if it never got there).
#[derive(Clone)]
struct LastTake {
    audio: Arc<AudioData>,
    transcript: Option<String>,
    duration_ms: u64,
    generation: u64,
    mode: DictationMode,
    prompt: Option<String>,
    rewrite_source: Option<String>,
}

impl LastTake {
    fn history_raw(&self, text: &str) -> String {
        match (self.mode, self.rewrite_source.as_deref()) {
            (DictationMode::Rewrite, Some(source)) => rewrite_history_raw(text, source),
            _ => text.to_string(),
        }
    }
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

/// Keep the mode's original prompt even if settings change before a retry.
// Tauri keys state by concrete type: a type alias of RewriteSourceSlot would
// share its storage, so the prompt needs a distinct wrapper.
struct ActivePromptSlot(parking_lot::Mutex<Option<String>>);

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut log_targets = vec![tauri_plugin_log::Target::new(
        tauri_plugin_log::TargetKind::LogDir {
            file_name: Some("audie".into()),
        },
    )];
    #[cfg(debug_assertions)]
    log_targets.push(tauri_plugin_log::Target::new(
        tauri_plugin_log::TargetKind::Stdout,
    ));

    let log_level = if cfg!(debug_assertions) {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets(log_targets)
                .level(log_level)
                .max_file_size(5 * 1024 * 1024)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(5))
                .timezone_strategy(tauri_plugin_log::TimezoneStrategy::UseLocal)
                .build(),
        )
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
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
            commands::get_daily_usage,
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
            commands::restart_app,
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
            app.manage(ActivePromptSlot(parking_lot::Mutex::new(None)));
            app.manage::<RewriteSourceSlot>(Arc::new(parking_lot::Mutex::new(None)));

            let presentation_app = app_handle.clone();
            let presentation_platform = platform.clone();
            let show_in_dock = commands::load_settings(&app_handle).show_in_dock;
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(150));
                if let Err(err) = presentation_platform.apply_app_icon(&presentation_app) {
                    log::warn!("apply app icon failed: {err:?}");
                }
                if !show_in_dock {
                    if let Err(err) =
                        presentation_platform.set_dock_visible(&presentation_app, false)
                    {
                        log::error!("apply Dock visibility failed: {err:?}");
                    }
                }
            });

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
/// trigger and start the platform capture session. macOS runs a native listen-only
/// tap because the webview cannot see fn; Windows reads keydown in the focused
/// recorder after this command has suspended the global shortcuts.
#[tauri::command]
fn begin_trigger_capture(app: AppHandle) -> AppResult<()> {
    let platform = app.state::<Arc<dyn Platform>>();
    platform.unregister_all_hotkeys(&app)?;
    if let Err(err) = platform.start_trigger_capture(&app) {
        if let Err(restore_err) = restore_configured_hotkeys(&app) {
            log::error!("restore hotkeys after capture start failure: {restore_err}");
        }
        return Err(err);
    }
    Ok(())
}

#[tauri::command]
fn end_trigger_capture(app: AppHandle) -> AppResult<()> {
    let platform = app.state::<Arc<dyn Platform>>();
    platform.stop_trigger_capture();
    restore_configured_hotkeys(&app)
}

fn restore_configured_hotkeys(app: &AppHandle) -> AppResult<()> {
    let platform = app.state::<Arc<dyn Platform>>();
    let hotkey = commands::load_hotkey(app);
    platform.register_hotkey(
        app,
        HotkeySlot::Primary,
        &hotkey,
        build_hotkey_callback(app, HotkeyRole::Primary),
    )?;
    register_compose_hotkey(app);
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
    // Invalidate delayed workers/timers before they can touch a new recording.
    let my_gen = {
        let mut cached = ctx.last_take.lock();
        let generation = ctx.take_gen.fetch_add(1, Ordering::SeqCst) + 1;
        *cached = None;
        generation
    };
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
    let settings = commands::load_settings(ctx.app);
    *ctx.app.state::<ActivePromptSlot>().0.lock() =
        Some(prompt_from_snapshot(None, &settings, mode));

    // Gate on mic permission before recording: a denial otherwise captures silence
    // and the user only sees a Whisper hallucination. Flash red instead (§3.7).

    if !platform.ensure_microphone_permission() {
        let _ = show_overlay(ctx.app);
        enter_error(
            ctx.app.clone(),
            ctx.state.clone(),
            AppError::Permission("请授予麦克风权限".into()),
            my_gen,
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
        enter_error(ctx.app.clone(), ctx.state.clone(), err, my_gen);
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
    let (my_gen, stopped) = {
        // Serialize this handoff with cancel/resume/terminal cleanup. In
        // particular, a cancel cannot observe Processing before its cache exists.
        // Only stopping capture happens here; provider calls always run unlocked.
        let mut cached = ctx.last_take.lock();
        if ctx.state.current() != AppState::Recording {
            return;
        }
        let generation = ctx.take_gen.load(Ordering::SeqCst);
        if !ctx
            .state
            .transition(ctx.app, AppState::Processing, Some("toggle-finish"))
        {
            return;
        }
        let streaming = take_streaming_session(ctx.streaming);
        let stopped = ctx.audio.stop_capture().map(|recorded| {
            let streaming =
                streaming_result_after_capture(streaming, recorded.streaming_queue_overflowed);
            let take = recorded_take(ctx, recorded.audio, generation);
            *cached = Some(take.clone());
            (take, streaming)
        });
        if stopped.is_err() {
            *cached = None;
        }
        (generation, stopped)
    };
    match stopped {
        Ok((take, streaming)) => spawn_transcription(
            ctx.app.clone(),
            ctx.state.clone(),
            ctx.transcription.clone(),
            ctx.enhance.clone(),
            ctx.inject.clone(),
            ctx.last_take.clone(),
            ctx.take_gen.clone(),
            my_gen,
            take,
            streaming,
            false,
        ),
        Err(err) => {
            log::error!("stop capture: {err:?}");
            enter_error(ctx.app.clone(), ctx.state.clone(), err, my_gen);
        }
    }
}

fn recorded_take(ctx: &HotkeyContext<'_>, audio: AudioData, generation: u64) -> LastTake {
    let audio = Arc::new(audio);
    LastTake {
        duration_ms: duration_ms(&audio),
        audio,
        transcript: None,
        generation,
        mode: *ctx.active_mode.lock(),
        prompt: ctx.app.state::<ActivePromptSlot>().0.lock().take(),
        rewrite_source: ctx.app.state::<RewriteSourceSlot>().lock().take(),
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
    with_hotkey_ctx(&app, |ctx| {
        let generation = {
            let mut cached = ctx.last_take.lock();
            let phase = ctx.state.current();
            if !matches!(phase, AppState::Recording | AppState::Processing) {
                return;
            }
            let generation = ctx.take_gen.fetch_add(1, Ordering::SeqCst) + 1;
            if phase == AppState::Recording {
                // Lazy cancel: keep the audio, without paying for another ASR call.
                clear_streaming_session(ctx.streaming);
                match ctx.audio.stop_capture() {
                    Ok(recorded) => *cached = Some(recorded_take(ctx, recorded.audio, generation)),
                    Err(err) => {
                        log::warn!("cancel stop_capture: {err:?}");
                        *cached = None;
                    }
                }
            }
            if !ctx
                .state
                .transition(ctx.app, AppState::Cancel, Some("overlay-cancel"))
            {
                return;
            }
            generation
        };
        record_cancelled_transcript(ctx.app, ctx.last_take);
        spawn_settle_after(
            ctx.app.clone(),
            ctx.state.clone(),
            TERMINAL_HOLD_MS,
            generation,
        );
    });
}

/// Resume the kept take from a terminal toast: 撤销操作 / 重试 run the full pipeline
/// (transcribe if needed → enhance → inject); `raw_only` (插入原文) injects the
/// transcript verbatim, skipping enhance. Re-enters via Cancel/Error → Processing.
fn resume_from_last_take(app: &AppHandle, raw_only: bool) {
    with_hotkey_ctx(app, |ctx| {
        let (take, my_gen) = {
            let mut cached = ctx.last_take.lock();
            if !matches!(ctx.state.current(), AppState::Cancel | AppState::Error) {
                return;
            }
            let Some(mut take) = cached.clone() else {
                return;
            };
            if !ctx
                .state
                .transition(ctx.app, AppState::Processing, Some("resume"))
            {
                return;
            }
            let generation = ctx.take_gen.fetch_add(1, Ordering::SeqCst) + 1;
            take.generation = generation;
            *cached = Some(take.clone());
            (take, generation)
        };
        spawn_transcription(
            ctx.app.clone(),
            ctx.state.clone(),
            ctx.transcription.clone(),
            ctx.enhance.clone(),
            ctx.inject.clone(),
            ctx.last_take.clone(),
            ctx.take_gen.clone(),
            my_gen,
            take,
            None,
            raw_only,
        );
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
    take: LastTake,
    streaming_result: Option<TranscriptStream>,
    raw_only: bool,
) {
    thread::spawn(move || {
        if take_gen.load(Ordering::SeqCst) != my_gen {
            return;
        }
        let duration_ms = take.duration_ms;
        let text = match take.transcript.as_ref() {
            Some(text) => text.clone(),
            None => {
                // Literal zeros indicate a broken/silent device; do not send them
                // to ASR, which may hallucinate speech from digital silence.
                if duration_ms >= 200 && is_digital_silence(&take.audio) {
                    enter_no_content(app, state, duration_ms, take.mode, my_gen);
                    return;
                }
                match resolve_transcript(&app, &transcription, &take.audio, streaming_result) {
                    Ok(text) => text,
                    Err(err) => {
                        log::error!("transcription failed: {err:?}");
                        enter_error(app, state, err, my_gen);
                        return;
                    }
                }
            }
        };

        // A cancelled take may keep its own late transcript for undo, but an old
        // worker must never fill a resumed/new take or recreate a released one.
        cache_transcript(&last_take, my_gen, &text);
        if take_gen.load(Ordering::SeqCst) != my_gen {
            return;
        }
        if text.trim().is_empty() {
            enter_no_content(app, state, duration_ms, take.mode, my_gen);
            return;
        }
        log::info!(
            "transcription completed (duration_ms={duration_ms}, chars={})",
            text.chars().count()
        );
        let _ = app.emit(
            "final-transcript",
            FinalTranscript {
                text: text.clone(),
                duration_ms,
            },
        );
        // Connection settings remain current so a corrected endpoint/key can be
        // retried. Only the take's template text and rewrite source stay frozen.
        let config = if raw_only {
            EnhanceConfig::default()
        } else {
            enhance_config(&app, take.mode, take.prompt.clone())
        };
        let (started, completed) = match take.mode {
            DictationMode::Polish => ("润色中…", "润色完成"),
            DictationMode::Rewrite => ("改写中…", "改写完成"),
            DictationMode::Compose => ("写作中…", "写作完成"),
        };
        if config.enhance_enabled && take_gen.load(Ordering::SeqCst) == my_gen {
            emit_enhance_progress(&app, "started", started);
        }
        let outcome = match finish_pipeline_tail(
            &take,
            &text,
            raw_only,
            &config,
            &take_gen,
            my_gen,
            |input, config| enhance.enhance(input, config),
            |output| inject.inject(&app, output),
            |mode, raw, enhanced, duration| {
                record_history(&app, "success", mode, raw, enhanced, duration);
            },
        ) {
            Ok(Some(outcome)) => outcome,
            Ok(None) => return,
            Err(err) => {
                log::error!("inject failed: {err:?}");
                enter_error(app, state, err, my_gen);
                return;
            }
        };
        let _cached = last_take.lock();
        if take_gen.load(Ordering::SeqCst) != my_gen {
            return;
        }
        match outcome {
            EnhanceOutcome::Enhanced => emit_enhance_progress(&app, "completed", completed),
            EnhanceOutcome::Failed => {
                emit_enhance_progress(&app, "failed", "润色失败但已注入原文");
            }
            EnhanceOutcome::Disabled => {}
        }
        state.transition(&app, AppState::Success, Some("injected"));
        let hold = if matches!(outcome, EnhanceOutcome::Failed) {
            TERMINAL_HOLD_MS
        } else {
            SUCCESS_HOLD_MS
        };
        spawn_settle_after(app, state, hold, my_gen);
    });
}

fn cache_transcript(last_take: &LastTakeSlot, generation: u64, text: &str) -> bool {
    let mut slot = last_take.lock();
    match slot.as_mut() {
        Some(take) if take.generation == generation => {
            take.transcript = Some(text.to_string());
            true
        }
        _ => false,
    }
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

/// Shared by first processing and every resume. Effects are passed in like the
/// settings mapper's key reader, so tests can cancel inside a real LLM/inject
/// boundary without a WebView, credentials, or keystrokes in the user's apps.
/// None means cancelled; callers must not publish a terminal state in that case.
#[allow(clippy::too_many_arguments)]
fn finish_pipeline_tail(
    take: &LastTake,
    text: &str,
    raw_only: bool,
    config: &EnhanceConfig,
    take_gen: &AtomicU64,
    my_gen: u64,
    enhance: impl FnOnce(&str, &EnhanceConfig) -> AppResult<String>,
    inject: impl FnOnce(&str) -> AppResult<()>,
    record: impl FnOnce(&str, &str, Option<String>, u64),
) -> AppResult<Option<EnhanceOutcome>> {
    if take_gen.load(Ordering::SeqCst) != my_gen {
        return Ok(None);
    }
    let original = if take.mode == DictationMode::Rewrite {
        take.rewrite_source.as_deref().unwrap_or_default()
    } else {
        text
    };
    let (output, outcome) = if raw_only || !config.enhance_enabled {
        (original.to_string(), EnhanceOutcome::Disabled)
    } else {
        let input = if take.mode == DictationMode::Rewrite {
            format!("原文：\n{original}\n\n指令：\n{text}")
        } else {
            text.to_string()
        };
        let enhanced = enhance(&input, config);
        // Cancellation while the blocking provider runs invalidates both its
        // success and its failure; neither is allowed to initiate a paste.
        if take_gen.load(Ordering::SeqCst) != my_gen {
            return Ok(None);
        }
        match enhanced {
            Ok(output) => (output, EnhanceOutcome::Enhanced),
            Err(err) => {
                log::warn!(
                    "{} failed, using original text: {err:?}",
                    take.mode.as_str()
                );
                let fallback = fallback_after_enhance_failure(original, &err);
                (fallback.text_to_inject, EnhanceOutcome::Failed)
            }
        }
    };
    if take_gen.load(Ordering::SeqCst) != my_gen {
        return Ok(None);
    }
    let injected = inject(&output);
    if take_gen.load(Ordering::SeqCst) != my_gen {
        return Ok(None);
    }
    injected?;
    let enhanced = matches!(outcome, EnhanceOutcome::Enhanced).then_some(output);
    record(
        take.mode.as_str(),
        &take.history_raw(text),
        enhanced,
        take.duration_ms,
    );
    Ok(Some(outcome))
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
        Some(take)
            if take
                .transcript
                .as_ref()
                .is_some_and(|text| !text.trim().is_empty()) =>
        {
            take.clone()
        }
        _ => return,
    };
    if let Some(text) = take.transcript.as_deref() {
        record_history(
            app,
            "success",
            take.mode.as_str(),
            &take.history_raw(text),
            None,
            take.duration_ms,
        );
    }
}

/// "No content recognized" outcome: record a `kind=empty` history entry and surface
/// it on the overlay. Reuses the ERROR toast (which renders as a neutral card with no
/// action buttons for this category — not the scary red device-error treatment), so
/// the user gets immediate feedback plus a history row.
fn enter_no_content(
    app: AppHandle,
    state: Arc<StateMachine>,
    duration_ms: u64,
    mode: DictationMode,
    generation: u64,
) {
    if app.state::<TakeGen>().load(Ordering::SeqCst) != generation {
        return;
    }
    record_history(&app, "empty", mode.as_str(), "", None, duration_ms);
    enter_error(
        app,
        state,
        AppError::Device("没有识别到内容".into()),
        generation,
    );
}

/// What happened to the transcript on the enhance step — distinguishes "polishing
/// was off" from "polished" (both inject the text, but only the latter has an
/// enhanced version worth storing) from "polish failed → raw fallback".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EnhanceOutcome {
    Disabled,
    Enhanced,
    Failed,
}

/// 改写历史的「原文」展示文本：口述指令 + 被改写的引用内容，分两段带小标题。History 原文框
/// 用 `whitespace-pre-wrap` 渲染、换行保留，这样历史里看得出「对哪段内容、下了什么指令」。
/// 与喂给 LLM 的 input（`原文/指令` 格式）不同——那个迁就 rewrite prompt，这个给人看。
fn rewrite_history_raw(instruction: &str, source: &str) -> String {
    format!("指令：{instruction}\n\n引用：\n{source}")
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
) -> Option<mpsc::SyncSender<AppResult<crate::asr::AudioChunk>>> {
    let config = doubao_streaming_config(app)?;
    let (chunks_tx, chunks_rx) = mpsc::sync_channel(STREAMING_AUDIO_QUEUE_CAPACITY);
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

fn streaming_result_after_capture(
    streaming_result: Option<TranscriptStream>,
    streaming_queue_overflowed: bool,
) -> Option<TranscriptStream> {
    if streaming_queue_overflowed {
        log::warn!("streaming audio queue overflowed; falling back to batch ASR");
        None
    } else {
        streaming_result
    }
}

fn transcription_config(app: &AppHandle) -> TranscriptionConfig {
    let settings = commands::load_settings(app);
    let platform = app.state::<Arc<dyn Platform>>();

    transcription_config_from_settings(&settings, |key_id| {
        read_optional_secret(platform.inner().as_ref(), key_id)
    })
}

fn transcription_config_from_settings(
    settings: &commands::Settings,
    mut read_secret: impl FnMut(&str) -> Option<String>,
) -> TranscriptionConfig {
    let asr_provider = settings.asr_provider.clone();
    let openai_api_key = (asr_provider == "openai")
        .then(|| read_secret("openai_api_key"))
        .flatten();
    let doubao_api_key_or_access_token = (asr_provider == "doubao_stream")
        .then(|| read_secret(doubao_config::SECRET_API_KEY_OR_ACCESS_TOKEN))
        .flatten();
    let doubao_app_id = (asr_provider == "doubao_stream")
        .then(|| read_secret(doubao_config::SECRET_APP_ID))
        .flatten();
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
        asr_model: settings.asr_model.clone(),
        openai_api_key,
        doubao_endpoint: Some(settings.doubao_endpoint.clone()),
        doubao_resource_id: Some(settings.doubao_resource_id.clone()),
        doubao_app_id,
        doubao_api_key_or_access_token,
        glm_endpoint: Some(settings.glm_endpoint.clone()),
        glm_api_key,
        aliyun_endpoint: Some(settings.aliyun_endpoint.clone()),
        aliyun_api_key,
        stepfun_endpoint: Some(settings.stepfun_endpoint.clone()),
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
    // alone must NOT hijack every recording — the explicit provider choice wins.
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
        openai_api_key: None,
        doubao_endpoint: Some(endpoint),
        doubao_resource_id: Some(resource_id),
        doubao_app_id: app_id,
        doubao_api_key_or_access_token: Some(api_key_or_access_token),
        glm_endpoint: None,
        glm_api_key: None,
        aliyun_endpoint: None,
        aliyun_api_key: None,
        stepfun_endpoint: None,
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

fn prompt_from_snapshot(
    snapshot: Option<String>,
    settings: &commands::Settings,
    mode: DictationMode,
) -> String {
    snapshot.unwrap_or_else(|| match mode {
        DictationMode::Polish => settings.enhance_prompt.clone(),
        DictationMode::Rewrite => settings.rewrite_prompt.clone(),
        DictationMode::Compose => settings.compose_prompt.clone(),
    })
}

fn enhance_config(app: &AppHandle, mode: DictationMode, snapshot: Option<String>) -> EnhanceConfig {
    let settings = commands::load_settings(app);
    let prompt = prompt_from_snapshot(snapshot, &settings, mode);
    let enhance_prompt = prepend_language(app, &settings, &prompt);
    let platform = app.state::<Arc<dyn Platform>>();
    enhance_config_from_settings(
        settings.llm_provider,
        mode != DictationMode::Polish,
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
    let enhance_prompt = prepend_language(app, &settings, &settings.enhance_prompt);
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
fn enter_error(app: AppHandle, state: Arc<StateMachine>, err: AppError, generation: u64) {
    {
        let last_take = app.state::<LastTakeSlot>();
        let _cached = last_take.lock();
        if app.state::<TakeGen>().load(Ordering::SeqCst) != generation {
            return;
        }
        let _ = app.emit(
            "error",
            ErrorPayload {
                code: err.code(),
                message: err.message().to_string(),
                recoverable: err.recoverable(),
            },
        );
        state.transition(&app, AppState::Error, Some("error"));
    }
    spawn_settle_after(app, state, TERMINAL_HOLD_MS, generation);
}

/// Auto-settle a terminal state to Idle after a hold. If the user acts first
/// (撤销 / 重试 → Processing), `settle_to_idle` finds a non-terminal state and
/// the timer is a no-op, so the live capsule isn't yanked away mid-resume.
fn spawn_settle_after(app: AppHandle, state: Arc<StateMachine>, hold_ms: u64, generation: u64) {
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(hold_ms));
        settle_to_idle(&app, &state, "recovered", generation);
    });
}

/// Transition to Idle and hide the overlay window. The single exit point for the
/// pipeline — overlay visibility mirrors "not Idle". Hides ONLY when the Idle
/// transition actually applied: a stale settle timer firing after the user tapped
/// 撤销 / 重试 (now Processing) must not hide the capsule out from under them.
fn settle_to_idle(app: &AppHandle, state: &Arc<StateMachine>, reason: &str, generation: u64) {
    let last_take = app.state::<LastTakeSlot>();
    let take_gen = app.state::<TakeGen>();
    if settle_take(&last_take, &take_gen, generation, || {
        state.transition(app, AppState::Idle, Some(reason))
    }) {
        if let Err(err) = hide_overlay(app, generation) {
            log::error!("hide overlay: {err:?}");
        }
    }
}

fn settle_take(
    last_take: &LastTakeSlot,
    take_gen: &AtomicU64,
    generation: u64,
    transition: impl FnOnce() -> bool,
) -> bool {
    let mut cached = last_take.lock();
    if take_gen.load(Ordering::SeqCst) != generation || !transition() {
        return false;
    }
    *cached = None;
    true
}

#[cfg(test)]
fn clear_last_take(last_take: &LastTakeSlot) {
    let _ = last_take.lock().take();
}

fn is_digital_silence(audio: &AudioData) -> bool {
    const SILENCE_EPS: f32 = 1e-4;
    !audio.samples.iter().any(|s| s.abs() > SILENCE_EPS)
}

fn duration_ms(audio: &AudioData) -> u64 {
    let denom = audio.sample_rate as u64 * audio.channels.max(1) as u64;
    (audio.samples.len() as u64 * 1000)
        .checked_div(denom)
        .unwrap_or_default()
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
fn hide_overlay(app: &AppHandle, generation: u64) -> AppResult<()> {
    let handle = app.clone();
    app.run_on_main_thread(move || {
        // A new recording can start before this queued AppKit operation runs.
        if handle.state::<TakeGen>().load(Ordering::SeqCst) != generation
            || handle.state::<Arc<StateMachine>>().current() != AppState::Idle
        {
            return;
        }
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
fn hide_overlay(app: &AppHandle, generation: u64) -> AppResult<()> {
    if app.state::<TakeGen>().load(Ordering::SeqCst) != generation
        || app.state::<Arc<StateMachine>>().current() != AppState::Idle
    {
        return Ok(());
    }
    let overlay = app
        .get_webview_window(OVERLAY_WINDOW_LABEL)
        .ok_or_else(|| AppError::Internal("overlay window not found".into()))?;
    overlay
        .hide()
        .map_err(|err| AppError::Internal(format!("hide overlay: {err}")))?;
    Ok(())
}

#[cfg(test)]
#[path = "tests/lib_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/recovery/mod.rs"]
mod recovery_tests;
