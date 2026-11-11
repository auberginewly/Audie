use super::context::{LastTakeSlot, TakeGen};
use super::events::ErrorPayload;
use super::processing::{record_history, spawn_transcription};
use super::recording::recorded_take;
use super::streaming::clear_streaming_session;
use super::{hide_overlay, with_hotkey_ctx, TERMINAL_HOLD_MS};
use crate::error::AppError;
use crate::state::{AppState, StateMachine};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// 录音中取消只缓存音频，撤销时再转写；处理中取消保留已有结果，
/// 未完成的转写可能在恢复时重做。
/// 终态保留窗口供用户操作，超时后统一清理。
pub(crate) fn cancel_recording(app: AppHandle) {
    with_hotkey_ctx(&app, |ctx| {
        let generation = {
            let mut cached = ctx.last_take.lock();
            let phase = ctx.state.current();
            if !matches!(phase, AppState::Recording | AppState::Processing) {
                return;
            }
            let generation = ctx.take_gen.fetch_add(1, Ordering::SeqCst) + 1;
            if phase == AppState::Recording {
                // 只缓存音频，用户没有撤销时不额外支付一次 ASR 请求。
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

/// 撤销与重试共用恢复入口，复用已完成的转写；插入原文则跳过润色。
/// 只允许从 Cancel/Error 恢复，避免处理中重复提交。
pub(super) fn resume_from_last_take(app: &AppHandle, raw_only: bool) {
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

/// 撤销取消操作，沿原模式恢复完整链路。
pub(crate) fn undo_last(app: AppHandle) {
    resume_from_last_take(&app, false);
}

/// 失败后重试保留的音频与上下文。
pub(crate) fn retry_last(app: AppHandle) {
    resume_from_last_take(&app, false);
}

/// 跳过润色；改写模式保留原选区，其他模式使用转写原文。
pub(crate) fn insert_raw_last(app: AppHandle) {
    resume_from_last_take(&app, true);
}

/// 已取消但已有转写时仍存历史，让用户找回文字；没有转写则不建空记录。
/// 历史失败只记诊断，不改变取消状态。
pub(super) fn record_cancelled_transcript(app: &AppHandle, last_take: &LastTakeSlot) {
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

/// 错误统一通过事件发布并定时回到 Idle；重试或插入原文可以先接管状态。
pub(super) fn enter_error(
    app: AppHandle,
    state: Arc<StateMachine>,
    err: AppError,
    generation: u64,
) {
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

/// 操作后状态已变成 Processing，旧定时器会被 settle_to_idle 拒绝，不能隐藏正在恢复的胶囊。
pub(super) fn spawn_settle_after(
    app: AppHandle,
    state: Arc<StateMachine>,
    hold_ms: u64,
    generation: u64,
) {
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(hold_ms));
        settle_to_idle(&app, &state, "recovered", generation);
    });
}

/// 只有本轮确实切到 Idle 才隐藏胶囊；旧轮次或旧定时器不能打断新录音与恢复。
pub(super) fn settle_to_idle(
    app: &AppHandle,
    state: &Arc<StateMachine>,
    reason: &str,
    generation: u64,
) {
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

pub(super) fn settle_take(
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
pub(super) fn clear_last_take(last_take: &LastTakeSlot) {
    let _ = last_take.lock().take();
}
