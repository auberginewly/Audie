//! Dictation lifecycle. Commands and hotkeys enter here; managers do the work.
mod config;
mod context;
mod events;
mod processing;
mod recording;
mod recovery;
mod streaming;

pub(crate) use config::reenhance_config;
use config::*;
pub(crate) use context::HotkeyRole;
use context::*;
use events::*;
use processing::*;
pub(crate) use recording::confirm_recording;
use recording::*;
use recovery::*;
pub(crate) use recovery::{cancel_recording, insert_raw_last, retry_last, undo_last};
use streaming::*;

use crate::asr::doubao::config as doubao_config;
use crate::asr::{AudioData, TranscriptStream};
use crate::error::{AppError, AppResult};
use crate::managers::audio::AudioManager;
use crate::managers::enhance::{fallback_after_enhance_failure, EnhanceConfig, EnhanceManager};
use crate::managers::history::HistoryManager;
use crate::managers::inject::InjectManager;
use crate::managers::transcription::{TranscriptionConfig, TranscriptionManager};
use crate::platform::Platform;
use crate::settings;
use crate::state::{AppState, StateMachine};
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
const SUCCESS_HOLD_MS: u64 = 150;

// Terminal toasts (error / cancelled / polish-unavailable) carry actions
// (重试 / 撤销操作 / 去设置), so they linger long enough to read and click before
// the overlay auto-settles to Idle. A user action transitions the state first,
// which turns the pending settle into a no-op (see `settle_to_idle`).
const TERMINAL_HOLD_MS: u64 = 6000;

const STREAMING_AUDIO_QUEUE_CAPACITY: usize = 32;

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

pub(crate) fn manage_state(app: &tauri::App) {
    app.manage(Arc::new(parking_lot::Mutex::new(None::<TranscriptStream>)));
    app.manage(Arc::new(parking_lot::Mutex::new(None::<LastTake>)));
    app.manage(Arc::new(AtomicU64::new(0)));
    app.manage::<ActiveModeSlot>(Arc::new(parking_lot::Mutex::new(DictationMode::Polish)));
    app.manage(ActivePromptSlot(parking_lot::Mutex::new(None)));
    app.manage::<RewriteSourceSlot>(Arc::new(parking_lot::Mutex::new(None)));
}

pub(crate) fn handle_trigger(app: &AppHandle, role: HotkeyRole) {
    with_hotkey_ctx(app, |ctx| handle_hotkey(ctx, role));
}

fn show_overlay(app: &AppHandle) -> AppResult<()> {
    app.state::<Arc<dyn Platform>>().show_overlay(app)
}

fn hide_overlay(app: &AppHandle, generation: u64) -> AppResult<()> {
    let handle = app.clone();
    app.run_on_main_thread(move || {
        // Check at execution time: a new recording can supersede this queued hide.
        if handle.state::<TakeGen>().load(Ordering::SeqCst) != generation
            || handle.state::<Arc<StateMachine>>().current() != AppState::Idle
        {
            return;
        }
        if let Err(err) = handle.state::<Arc<dyn Platform>>().hide_overlay(&handle) {
            log::error!("hide overlay: {err:?}");
        }
    })
    .map_err(|err| AppError::Internal(format!("run_on_main_thread: {err}")))
}

#[cfg(test)]
mod tests;
