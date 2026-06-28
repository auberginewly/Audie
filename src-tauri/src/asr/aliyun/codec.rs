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
        .map_err(|err| AppError::Internal(format!("aliyun run-task encode failed: {err}")))
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
        .map_err(|err| AppError::Internal(format!("aliyun finish-task encode failed: {err}")))
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
        .map_err(|err| AppError::Internal(format!("aliyun event decode failed: {err}")))?;

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
        "task-failed" | "error" => Err(AppError::Provider(format!(
            "aliyun {event}: {}",
            error_message(&value)
        ))),
        other => Err(AppError::Internal(format!(
            "aliyun unexpected event: {other}"
        ))),
    }
}

/// Pull a human-readable error string from a failure frame. DashScope buries the
/// message under different keys depending on the failure path, so try the known
/// spots before falling back to a generic message.
/// TODO: confirm the canonical error field(s) against official docs.
fn error_message(value: &serde_json::Value) -> String {
    let header = value.get("header");
    for key in ["error_message", "message"] {
        if let Some(msg) = header.and_then(|h| h.get(key)).and_then(|m| m.as_str()) {
            return msg.to_string();
        }
    }
    if let Some(msg) = value
        .get("payload")
        .and_then(|p| p.get("message"))
        .and_then(|m| m.as_str())
    {
        return msg.to_string();
    }
    "通义 ASR 任务失败".to_string()
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
mod tests {
    use super::*;

    #[test]
    fn run_task_carries_action_task_id_and_default_model() {
        let json = build_run_task("task-1", "").expect("encodes");
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert_eq!(value["header"]["action"], "run-task");
        assert_eq!(value["header"]["task_id"], "task-1");
        assert_eq!(value["payload"]["model"], config::DEFAULT_MODEL);
        assert_eq!(
            value["payload"]["parameters"]["sample_rate"],
            config::SAMPLE_RATE
        );
    }

    #[test]
    fn run_task_keeps_explicit_model() {
        let json = build_run_task("task-2", "paraformer-realtime-v2").expect("encodes");
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert_eq!(value["payload"]["model"], "paraformer-realtime-v2");
    }

    #[test]
    fn finish_task_carries_action_and_task_id() {
        let json = build_finish_task("task-3").expect("encodes");
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert_eq!(value["header"]["action"], "finish-task");
        assert_eq!(value["header"]["task_id"], "task-3");
    }

    #[test]
    fn parse_task_started() {
        let raw = r#"{"header":{"event":"task-started","task_id":"t"},"payload":{}}"#;
        assert_eq!(parse_server_event(raw).unwrap(), ServerEvent::Started);
    }

    #[test]
    fn parse_result_generated_extracts_sentence_text() {
        let raw = r#"{"header":{"event":"result-generated"},"payload":{"output":{"sentence":{"text":"你好世界"}}}}"#;
        match parse_server_event(raw).unwrap() {
            ServerEvent::Result { text } => assert_eq!(text.as_deref(), Some("你好世界")),
            other => panic!("expected Result, got {other:?}"),
        }
    }

    #[test]
    fn parse_task_finished_extracts_text() {
        let raw = r#"{"header":{"event":"task-finished"},"payload":{"output":{"sentence":{"text":"done"}}}}"#;
        match parse_server_event(raw).unwrap() {
            ServerEvent::Finished { text } => assert_eq!(text.as_deref(), Some("done")),
            other => panic!("expected Finished, got {other:?}"),
        }
    }

    #[test]
    fn parse_task_failed_is_provider_error() {
        let raw = r#"{"header":{"event":"task-failed","error_message":"invalid api key"}}"#;
        let err = parse_server_event(raw).unwrap_err();
        assert!(matches!(err, AppError::Provider(_)));
        assert!(err.message().contains("invalid api key"));
    }

    #[test]
    fn parse_error_event_is_provider_error() {
        // A bare `error` event (vs `task-failed`) still maps to Provider, with the
        // message pulled from `payload.message`.
        let raw = r#"{"header":{"event":"error"},"payload":{"message":"model not found"}}"#;
        let err = parse_server_event(raw).unwrap_err();
        assert!(matches!(err, AppError::Provider(_)));
        assert!(err.message().contains("model not found"));
    }

    #[test]
    fn parse_task_failed_without_message_falls_back() {
        let raw = r#"{"header":{"event":"task-failed"}}"#;
        let err = parse_server_event(raw).unwrap_err();
        assert!(matches!(err, AppError::Provider(_)));
        assert!(err.message().contains("通义 ASR 任务失败"));
    }

    #[test]
    fn parse_missing_event_is_internal_error() {
        let raw = r#"{"header":{},"payload":{}}"#;
        let err = parse_server_event(raw).unwrap_err();
        assert!(matches!(err, AppError::Internal(_)));
    }

    #[test]
    fn parse_invalid_json_is_internal_error() {
        let err = parse_server_event("not json").unwrap_err();
        assert!(matches!(err, AppError::Internal(_)));
    }
}
