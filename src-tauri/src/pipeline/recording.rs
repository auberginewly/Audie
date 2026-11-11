use super::*;
pub(super) fn handle_hotkey(ctx: &HotkeyContext<'_>, role: HotkeyRole) {
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
pub(super) fn start_recording(ctx: &HotkeyContext<'_>, role: HotkeyRole) {
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
    let settings = settings::load_settings(ctx.app);
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
pub(super) fn finish_recording(ctx: &HotkeyContext<'_>) {
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

pub(super) fn recorded_take(
    ctx: &HotkeyContext<'_>,
    audio: AudioData,
    generation: u64,
) -> LastTake {
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

pub(super) fn is_digital_silence(audio: &AudioData) -> bool {
    const SILENCE_EPS: f32 = 1e-4;
    !audio.samples.iter().any(|s| s.abs() > SILENCE_EPS)
}

pub(super) fn duration_ms(audio: &AudioData) -> u64 {
    let denom = audio.sample_rate as u64 * audio.channels.max(1) as u64;
    (audio.samples.len() as u64 * 1000)
        .checked_div(denom)
        .unwrap_or_default()
}

/// ✓ on the capsule — finish the current take (same as a second hotkey press).
pub(crate) fn confirm_recording(app: AppHandle) {
    with_hotkey_ctx(&app, |ctx| {
        if ctx.state.current() == AppState::Recording {
            finish_recording(ctx);
        }
    });
}
