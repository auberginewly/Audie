use super::*;
/// Run the (blocking) transcription off the hotkey thread, then drive the
/// P1 pipeline tail: ASR → `final-transcript` event → optional LLM enhance →
/// inject at the caret → Success → Idle. Any failure is mapped to `error` and
/// recovers through the same Idle exit. PROJECT_SPEC.md §3.2 / §3.3 / §4.3.
#[allow(clippy::too_many_arguments)] // pipeline tail wiring; splitting it would
                                     // just shuffle the same handles around.
pub(super) fn spawn_transcription(
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

pub(super) fn cache_transcript(last_take: &LastTakeSlot, generation: u64, text: &str) -> bool {
    let mut slot = last_take.lock();
    match slot.as_mut() {
        Some(take) if take.generation == generation => {
            take.transcript = Some(text.to_string());
            true
        }
        _ => false,
    }
}

pub(super) fn resolve_transcript(
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
pub(super) fn finish_pipeline_tail(
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
pub(super) fn record_history(
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

/// "No content recognized" outcome: record a `kind=empty` history entry and surface
/// it on the overlay. Reuses the ERROR toast (which renders as a neutral card with no
/// action buttons for this category — not the scary red device-error treatment), so
/// the user gets immediate feedback plus a history row.
pub(super) fn enter_no_content(
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

/// 改写历史的「原文」展示文本：口述指令 + 被改写的引用内容，分两段带小标题。History 原文框
/// 用 `whitespace-pre-wrap` 渲染、换行保留，这样历史里看得出「对哪段内容、下了什么指令」。
/// 与喂给 LLM 的 input（`原文/指令` 格式）不同——那个迁就 rewrite prompt，这个给人看。
pub(super) fn rewrite_history_raw(instruction: &str, source: &str) -> String {
    format!("指令：{instruction}\n\n引用：\n{source}")
}

pub(super) fn emit_enhance_progress(app: &AppHandle, phase: &'static str, message: &str) {
    let _ = app.emit(
        "enhance-progress",
        EnhanceProgress {
            phase,
            message: message.to_string(),
        },
    );
}
