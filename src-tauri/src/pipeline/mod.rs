//! 听写生命周期入口：快捷键与 command 共用这里的状态，具体工作交给 manager。
mod config;
mod context;
mod events;
mod processing;
mod recording;
mod recovery;
mod streaming;

pub(crate) use config::reenhance_config;
pub(crate) use context::HotkeyRole;
use context::{
    ActiveModeSlot, ActivePromptSlot, ActiveStreamingSession, DictationMode, HotkeyContext,
    LastTake, LastTakeSlot, RewriteSourceSlot, TakeGen,
};
pub(crate) use recording::confirm_recording;
use recording::handle_hotkey;
pub(crate) use recovery::{cancel_recording, insert_raw_last, retry_last, undo_last};

use crate::asr::TranscriptStream;
use crate::error::{AppError, AppResult};
use crate::managers::audio::AudioManager;
use crate::managers::enhance::EnhanceManager;
use crate::managers::inject::InjectManager;
use crate::managers::transcription::TranscriptionManager;
use crate::platform::Platform;
use crate::state::{AppState, StateMachine};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

const SUCCESS_HOLD_MS: u64 = 150;

// 终态带重试、撤销、去设置等操作，保留六秒供阅读和点击。
// 操作会先改变状态，让旧定时器在 settle_to_idle 中失效。
const TERMINAL_HOLD_MS: u64 = 6000;

const STREAMING_AUDIO_QUEUE_CAPACITY: usize = 32;

/// 快捷键和胶囊按钮从同一份 Tauri state 组装上下文，避免两条录音流程各自持有状态。
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
        // 到主线程执行时再核对轮次；排队期间可能已经开始新录音。
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
