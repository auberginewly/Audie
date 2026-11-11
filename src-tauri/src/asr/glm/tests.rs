use super::*;

#[test]
fn new_falls_back_to_default_model_when_blank() {
    let provider = GlmProvider::new(ENDPOINT.into(), "key".into(), "  ".into());
    assert_eq!(provider.model, DEFAULT_MODEL);
}

#[test]
fn new_keeps_explicit_endpoint() {
    let provider = GlmProvider::new(
        "https://glm.example.test/audio/transcriptions".into(),
        "key".into(),
        DEFAULT_MODEL.into(),
    );
    assert_eq!(
        provider.endpoint,
        "https://glm.example.test/audio/transcriptions"
    );
}

#[test]
fn new_keeps_explicit_model() {
    let provider = GlmProvider::new(ENDPOINT.into(), "key".into(), "glm-asr-2512".into());
    assert_eq!(provider.model, "glm-asr-2512");
}

#[test]
fn build_form_succeeds_with_wav_bytes() {
    // The multipart part construction can fail on a bad MIME; assert the happy
    // path builds so transcribe()'s `?` never trips on a well-formed call.
    let form = build_form("glm-asr-1", vec![0u8; 44]).expect("form builds");
    // Form has no public field accessors; constructing without error is the check.
    drop(form);
}

#[test]
fn parse_transcript_reads_non_stream_json() {
    let body = r#"{"text":"  你好世界  "}"#;
    assert_eq!(parse_transcript(body).unwrap(), "你好世界");
}

#[test]
fn parse_transcript_accumulates_sse_text_fragments() {
    // Server ignored stream=false and replied SSE; we append fragments in order.
    let body = "data: {\"delta\":\"你好\"}\n\ndata: {\"delta\":\"世界\"}\n\ndata: [DONE]\n";
    assert_eq!(parse_transcript(body).unwrap(), "你好世界");
}

#[test]
fn parse_transcript_accepts_text_field_in_sse() {
    let body = "data: {\"text\":\"hello\"}\n\ndata: [DONE]\n";
    assert_eq!(parse_transcript(body).unwrap(), "hello");
}

#[test]
fn parse_transcript_errors_on_unparseable_body() {
    let err = parse_transcript("<html>gateway timeout</html>").unwrap_err();
    assert!(matches!(err, AppError::Internal(_)));
}

#[test]
fn classify_http_401_is_provider_error() {
    let err = classify_http_status(401, "unauthorized");
    assert!(matches!(err, AppError::Provider(_)));
}

#[test]
fn classify_http_403_is_provider_error() {
    let err = classify_http_status(403, "forbidden");
    assert!(matches!(err, AppError::Provider(_)));
}

#[test]
fn classify_http_429_is_recoverable_network() {
    let err = classify_http_status(429, "rate limited");
    assert!(matches!(err, AppError::Network(_)));
    assert!(err.recoverable());
}

#[test]
fn classify_http_5xx_is_network() {
    let err = classify_http_status(503, "unavailable");
    assert!(matches!(err, AppError::Network(_)));
}

#[test]
fn classify_http_unknown_status_is_provider_with_status() {
    let err = classify_http_status(418, "teapot");
    match err {
        AppError::Provider(msg) => assert!(msg.contains("418")),
        other => panic!("expected Provider, got {other:?}"),
    }
}

#[test]
fn privacy_http_diagnostics_keep_status_without_body() {
    for (status, category) in [
        (400, "provider"),
        (418, "provider"),
        (401, "provider"),
        (429, "network"),
        (503, "network"),
    ] {
        let error = classify_http_status(status, "PRIVATE_BODY_KEY_PROMPT_SELECTION");
        assert_eq!(error.code(), category);
        assert!(!error.to_string().contains("PRIVATE_BODY"));
        if status == 418 || status == 503 {
            assert!(error.message().contains(&status.to_string()));
        }
    }
}

#[test]
fn privacy_transport_diagnostics_omit_url_credentials_and_query() {
    let raw = reqwest::blocking::Client::new()
        .get("https://private-user:PRIVATE_PASSWORD@example.test/path?token=PRIVATE_QUERY")
        .header("Authorization", "PRIVATE_AUTH\ninvalid")
        .build()
        .unwrap_err();
    let raw = raw.with_url(
        reqwest::Url::parse(
            "https://private-user:PRIVATE_PASSWORD@example.test/?token=PRIVATE_QUERY",
        )
        .unwrap(),
    );
    let error = classify_reqwest_error(raw);
    assert_eq!(error.code(), "network");
    assert!(error.recoverable());
    assert!(!error.to_string().contains("PRIVATE_"));
    assert!(!error.to_string().contains("example.test"));
}

#[test]
fn privacy_malformed_transcript_does_not_echo_body() {
    let error = parse_transcript("PRIVATE_TRANSCRIPT_PROMPT").unwrap_err();
    assert_eq!(error.code(), "internal");
    assert!(!error.to_string().contains("PRIVATE_"));
}

#[test]
fn privacy_success_still_returns_transcript_content() {
    assert_eq!(
        parse_transcript(r#"{"text":"PRIVATE_TRANSCRIPT"}"#).unwrap(),
        "PRIVATE_TRANSCRIPT"
    );
}
