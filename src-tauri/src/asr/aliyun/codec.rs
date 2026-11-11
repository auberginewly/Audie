// DashScope Fun-ASR realtime JSON frame codec (pure functions, no IO / no settings).
//
// DashScope's realtime WS protocol wraps every message in a `header` + `payload`.
// The client sends a `run-task` (to start), streams raw PCM as binary frames, then
// sends `finish-task`. The server replies with `task-started`, repeated
// `result-generated` (carrying recognized text), and a terminal `task-finished` (or
// `task-failed`). This module only encodes the two client frames and classifies the
// server events; the WS session lives in `client`.
//
// Field source: reverse-engineered from Voxt + DashScope realtime conventions, NOT
// confirmed against official docs. TODO: verify header/payload field names, the
// `result-generated` text path, and the task-failed error shape on real hardware.

#![allow(dead_code)] // consumed once the client drives a live session.

use super::config;
use crate::error::{AppError, AppResult};

/// Build the `run-task` start frame as a JSON string. `task_id` correlates every
/// subsequent server event back to this task.
/// TODO: confirm `parameters` keys (sample_rate / format / model placement).
pub fn build_run_task(task_id: &str, model: &str) -> AppResult<String> {
    let model = if model.trim().is_empty() {
        config::DEFAULT_MODEL
    } else {
        model
    };
    let frame = serde_json::json!({
        "header": {
            "action": "run-task",
            "task_id": task_id,
            "streaming": "duplex",
        },
        "payload": {
            "task_group": "audio",
            "task": "asr",
            "function": "recognition",
            "model": model,
            "parameters": {
                "format": config::AUDIO_FORMAT,
                "sample_rate": config::SAMPLE_RATE,
            },
            "input": {},
        }
    });
    serde_json::to_string(&frame)
        .map_err(|_| AppError::Internal("aliyun run-task encode failed".into()))
}

/// Build the `finish-task` frame telling DashScope the input stream is done so it
/// emits the terminal `task-finished` (with the final recognition).
pub fn build_finish_task(task_id: &str) -> AppResult<String> {
    let frame = serde_json::json!({
        "header": {
            "action": "finish-task",
            "task_id": task_id,
            "streaming": "duplex",
        },
        "payload": { "input": {} }
    });
    serde_json::to_string(&frame)
        .map_err(|_| AppError::Internal("aliyun finish-task encode failed".into()))
}

/// A classified server event. `text` carries the latest recognition (may be empty);
/// `is_final` marks the terminal `task-finished`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerEvent {
    /// `task-started` acknowledgement (no recognition content yet).
    Started,
    /// `result-generated` partial/incremental recognition.
    Result { text: Option<String> },
    /// `task-finished` terminal event.
    Finished { text: Option<String> },
}

/// Parse one server text frame. `task-failed` (or an error header) maps to a
/// `Provider` error so the caller routes it per §3.7; an unparseable frame is
/// `Internal` (protocol violation, not the user's fault).
pub fn parse_server_event(raw: &str) -> AppResult<ServerEvent> {
    let value: serde_json::Value = serde_json::from_str(raw)
        .map_err(|_| AppError::Internal("aliyun event decode failed".into()))?;

    let event = value
        .get("header")
        .and_then(|h| h.get("event"))
        .and_then(|e| e.as_str())
        .ok_or_else(|| AppError::Internal("aliyun event missing header.event".into()))?;

    match event {
        "task-started" => Ok(ServerEvent::Started),
        "result-generated" => Ok(ServerEvent::Result {
            text: extract_text(&value),
        }),
        "task-finished" => Ok(ServerEvent::Finished {
            text: extract_text(&value),
        }),
        // Both `task-failed` and a bare `error` event carry a server-side rejection
        // (bad key / quota / model-not-found) → Provider per §3.7.
        "task-failed" | "error" => Err(AppError::Provider(
            "通义 ASR 任务失败，请检查 API key、模型与额度".into(),
        )),
        _ => Err(AppError::Internal("aliyun unexpected event".into())),
    }
}

/// Pull recognized text from `payload.output.sentence.text` (the documented Fun-ASR
/// shape), falling back to `payload.output.text` for robustness.
/// TODO: confirm the canonical path against official docs.
fn extract_text(value: &serde_json::Value) -> Option<String> {
    let output = value.get("payload").and_then(|p| p.get("output"))?;
    if let Some(text) = output
        .get("sentence")
        .and_then(|s| s.get("text"))
        .and_then(|t| t.as_str())
    {
        return Some(text.to_string());
    }
    output
        .get("text")
        .and_then(|t| t.as_str())
        .map(str::to_string)
}

#[cfg(test)]
#[path = "codec/tests.rs"]
mod tests;
