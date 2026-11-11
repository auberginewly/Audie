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
    let raw =
        r#"{"header":{"event":"task-finished"},"payload":{"output":{"sentence":{"text":"done"}}}}"#;
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
    assert!(err.message().contains("API key"));
}

#[test]
fn parse_error_event_is_provider_error() {
    // 裸 error 仍属于 Provider；正文可能回显用户内容，因此使用固定说明。
    let raw = r#"{"header":{"event":"error"},"payload":{"message":"model not found"}}"#;
    let err = parse_server_event(raw).unwrap_err();
    assert!(matches!(err, AppError::Provider(_)));
    assert!(err.message().contains("模型"));
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

#[test]
fn privacy_server_failures_and_unknown_events_do_not_echo_payload() {
    for raw in [
        r#"{"header":{"event":"task-failed","error_message":"PRIVATE_AUTH_PROMPT"}}"#,
        r#"{"header":{"event":"error"},"payload":{"message":"PRIVATE_SELECTION"}}"#,
        r#"{"header":{"event":"PRIVATE_EVENT"}}"#,
    ] {
        let error = parse_server_event(raw).unwrap_err();
        assert!(!error.to_string().contains("PRIVATE_"));
    }
}

#[test]
fn privacy_success_still_returns_transcript_content() {
    let raw = r#"{"header":{"event":"task-finished"},"payload":{"output":{"text":"PRIVATE_TRANSCRIPT"}}}"#;
    assert_eq!(
        parse_server_event(raw).unwrap(),
        ServerEvent::Finished {
            text: Some("PRIVATE_TRANSCRIPT".into())
        }
    );
}
