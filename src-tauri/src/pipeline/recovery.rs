use super::*;
/// ✕ on the capsule — cancel into a "已取消" toast that offers 撤销操作. The take
/// is always kept (lazy: from Recording we just stash the raw audio without
/// transcribing; mid-Processing the worker already stored it), so undo can
/// resume it. The toast auto-settles to Idle after `TERMINAL_HOLD_MS`.
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

/// 撤销操作 on the cancelled toast — resume the kept take through the full pipeline.
pub(crate) fn undo_last(app: AppHandle) {
    resume_from_last_take(&app, false);
}

/// 重试 on the error toast — re-run the full pipeline on the kept take.
pub(crate) fn retry_last(app: AppHandle) {
    resume_from_last_take(&app, false);
}

/// 插入原文 on the error toast — inject the raw transcript, skipping enhance.
pub(crate) fn insert_raw_last(app: AppHandle) {
    resume_from_last_take(&app, true);
}

/// On cancel, preserve the transcript if one already exists (a mid-Processing cancel
/// where ASR returned, or streaming had a final) as a normal history entry — the user
/// keeps the text even though it wasn't injected. A cancel with no transcript (the
/// usual mid-recording ✕) records nothing. Best-effort; no change to the cancel path.
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

/// Emit the error, flash the Error toast, and auto-settle to Idle after the
/// terminal hold (the toast's 重试 / 插入原文 can act first). Spawns its own thread
/// so callers on the hotkey path don't block.
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

/// Auto-settle a terminal state to Idle after a hold. If the user acts first
/// (撤销 / 重试 → Processing), `settle_to_idle` finds a non-terminal state and
/// the timer is a no-op, so the live capsule isn't yanked away mid-resume.
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

/// Transition to Idle and hide the overlay window. The single exit point for the
/// pipeline — overlay visibility mirrors "not Idle". Hides ONLY when the Idle
/// transition actually applied: a stale settle timer firing after the user tapped
/// 撤销 / 重试 (now Processing) must not hide the capsule out from under them.
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
