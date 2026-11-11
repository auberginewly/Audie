use super::config::{enhance_config, transcription_config};
use super::context::{DictationMode, EnhanceOutcome, LastTake, LastTakeSlot, TakeGen};
use super::events::{EnhanceProgress, FinalTranscript};
use super::recording::is_digital_silence;
use super::recovery::{enter_error, spawn_settle_after};
use super::{SUCCESS_HOLD_MS, TERMINAL_HOLD_MS};
use crate::asr::{AudioData, TranscriptStream};
use crate::error::{AppError, AppResult};
use crate::managers::enhance::{fallback_after_enhance_failure, EnhanceConfig, EnhanceManager};
use crate::managers::history::HistoryManager;
use crate::managers::inject::InjectManager;
use crate::managers::transcription::TranscriptionManager;
use crate::state::{AppState, StateMachine};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use tauri::{AppHandle, Emitter, Manager};

/// 转写和润色会阻塞，放到工作线程以免卡住快捷键和系统 UI。
/// 首次处理与恢复共用尾部：ASR → final 事件 → 可选润色 → 注入 → 终态。
#[allow(clippy::too_many_arguments)] // 这些句柄来自同一条链路，另加容器只会转移接线。
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
                // 持续数字静音通常意味着设备异常；送进 ASR 可能产生幻觉文字。
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

        // 取消的轮次可以缓存晚到的转写用于撤销，但不能填入新轮次或复活已清理的缓存。
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
        // 连接配置取当前值，用户修好 endpoint/key 后才能重试；
        // 提示词和改写选区仍保持录音开始时的快照。
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
        match &outcome {
            EnhanceOutcome::Enhanced => emit_enhance_progress(&app, "completed", completed),
            EnhanceOutcome::Failed(message) => {
                emit_enhance_progress(&app, "failed", message);
            }
            EnhanceOutcome::Disabled => {}
        }
        state.transition(&app, AppState::Success, Some("injected"));
        let hold = if matches!(&outcome, EnhanceOutcome::Failed(_)) {
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

/// 首次处理和恢复共用的注入尾部；副作用由调用方传入，测试可在边界模拟取消，
/// 不需要 WebView、真实凭据或向用户应用发送按键。None 表示已取消，调用方不能发布终态。
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
        // 阻塞的 provider 返回后再核对轮次；取消后不论成功或失败都不能开始粘贴。
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
                (
                    fallback.text_to_inject,
                    EnhanceOutcome::Failed(fallback.message),
                )
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
    let enhanced = matches!(&outcome, EnhanceOutcome::Enhanced).then_some(output);
    record(
        take.mode.as_str(),
        &take.history_raw(text),
        enhanced,
        take.duration_ms,
    );
    Ok(Some(outcome))
}

/// 历史是附属能力，写库失败只记录诊断，不打断已完成的注入。
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

/// 空结果复用 Error 事件并记录 empty 历史；前端按类别展示中性提示，
/// 让用户知道这次确实执行过，同时不把无内容误报成设备故障。
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

/// 历史用「指令 + 引用」让用户看懂改写了什么；与发送给 LLM 的格式分开维护。
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
