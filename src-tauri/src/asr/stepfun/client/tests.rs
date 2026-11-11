use super::*;

#[test]
fn new_falls_back_to_default_model_when_blank() {
    let provider = StepFunProvider::new(config::ENDPOINT.into(), "key".into(), "  ".into());
    assert_eq!(provider.model, config::DEFAULT_MODEL);
}

#[test]
fn new_keeps_explicit_endpoint() {
    let provider = StepFunProvider::new(
        "https://stepfun.example.test/audio/transcriptions".into(),
        "key".into(),
        config::DEFAULT_MODEL.into(),
    );
    assert_eq!(
        provider.endpoint,
        "https://stepfun.example.test/audio/transcriptions"
    );
}

#[test]
fn new_keeps_explicit_model() {
    let provider = StepFunProvider::new(
        config::ENDPOINT.into(),
        "key".into(),
        "stepaudio-2-asr-pro".into(),
    );
    assert_eq!(provider.model, "stepaudio-2-asr-pro");
}

#[test]
fn transcribe_rejects_empty_key_as_provider_error() {
    let provider = StepFunProvider::new(config::ENDPOINT.into(), "   ".into(), String::new());
    let audio = AudioData {
        samples: vec![0.0],
        sample_rate: 16_000,
        channels: 1,
    };
    let err = provider.transcribe(&audio).unwrap_err();
    assert!(matches!(err, AppError::Provider(_)));
}

// --- request body construction ---

#[test]
fn build_request_body_has_base64_audio_and_pcm_format() {
    let body = build_request_body(config::DEFAULT_MODEL, &[0x00, 0x01, 0xFF]);

    // base64("\x00\x01\xFF") == "AAH/"
    assert_eq!(body["audio"]["data"], "AAH/");

    let transcription = &body["input"]["transcription"];
    assert_eq!(transcription["model"], config::DEFAULT_MODEL);
    assert_eq!(transcription["language"], "zh");
    assert_eq!(transcription["enable_itn"], true);

    let format = &body["input"]["format"];
    assert_eq!(format["type"], "pcm");
    assert_eq!(format["codec"], "pcm_s16le");
    assert_eq!(format["rate"], config::SAMPLE_RATE);
    assert_eq!(format["bits"], 16);
    assert_eq!(format["channel"], 1);
}

#[test]
fn build_request_body_threads_explicit_model() {
    let body = build_request_body("stepaudio-2-asr-pro", &[]);
    assert_eq!(
        body["input"]["transcription"]["model"],
        "stepaudio-2-asr-pro"
    );
}

// --- base64 ---

#[test]
fn base64_encode_matches_rfc4648_vectors() {
    assert_eq!(base64_encode(b""), "");
    assert_eq!(base64_encode(b"f"), "Zg==");
    assert_eq!(base64_encode(b"fo"), "Zm8=");
    assert_eq!(base64_encode(b"foo"), "Zm9v");
    assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
    assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
    assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
}

// --- single-event parsing ---

#[test]
fn parse_delta_event() {
    let event = parse_sse_data(r#"{"type":"transcript.text.delta","delta":"你好"}"#)
        .unwrap()
        .unwrap();
    assert_eq!(
        event,
        SseEvent::Delta {
            text: "你好".into()
        }
    );
}

#[test]
fn parse_done_event_with_full_text() {
    let event = parse_sse_data(r#"{"type":"transcript.text.done","text":"你好世界"}"#)
        .unwrap()
        .unwrap();
    assert_eq!(
        event,
        SseEvent::Done {
            text: Some("你好世界".into())
        }
    );
}

#[test]
fn parse_stream_terminator_returns_none() {
    assert_eq!(parse_sse_data("[DONE]").unwrap(), None);
}

#[test]
fn parse_unknown_type_is_other() {
    let event = parse_sse_data(r#"{"type":"ping"}"#).unwrap().unwrap();
    assert_eq!(event, SseEvent::Other);
}

#[test]
fn parse_explicit_error_event_carries_message() {
    let event = parse_sse_data(r#"{"type":"error","error":{"message":"quota exceeded"}}"#)
        .unwrap()
        .unwrap();
    assert_eq!(
        event,
        SseEvent::Error {
            message: "quota exceeded".into()
        }
    );
}

#[test]
fn parse_embedded_error_object_without_type_is_error() {
    let event = parse_sse_data(r#"{"error":"bad request"}"#)
        .unwrap()
        .unwrap();
    assert_eq!(
        event,
        SseEvent::Error {
            message: "bad request".into()
        }
    );
}

#[test]
fn parse_invalid_json_is_internal_error() {
    let err = parse_sse_data("not json").unwrap_err();
    assert!(matches!(err, AppError::Internal(_)));
}

// --- full-stream parsing ---

#[test]
fn parse_sse_stream_prefers_done_text_over_deltas() {
    let body = concat!(
        "data: {\"type\":\"transcript.text.delta\",\"delta\":\"你\"}\n",
        "data: {\"type\":\"transcript.text.delta\",\"delta\":\"好\"}\n",
        "data: {\"type\":\"transcript.text.done\",\"text\":\"你好世界\"}\n",
        "data: [DONE]\n",
    );
    assert_eq!(parse_sse_stream(body).unwrap(), "你好世界");
}

#[test]
fn parse_sse_stream_falls_back_to_accumulated_deltas() {
    // `done` without a `text` field → return the concatenated deltas.
    let body = concat!(
        "data: {\"type\":\"transcript.text.delta\",\"delta\":\"hello \"}\n",
        "data: {\"type\":\"transcript.text.delta\",\"delta\":\"world\"}\n",
        "data: {\"type\":\"transcript.text.done\"}\n",
    );
    assert_eq!(parse_sse_stream(body).unwrap(), "hello world");
}

#[test]
fn parse_sse_stream_skips_event_and_blank_lines() {
    // `event:`/`id:`/comment lines and CRLF endings must not break accumulation.
    let body = "event: message\r\ndata: {\"type\":\"transcript.text.delta\",\"delta\":\"a\"}\r\n\r\ndata: {\"type\":\"transcript.text.done\",\"text\":\"abc\"}\r\n";
    assert_eq!(parse_sse_stream(body).unwrap(), "abc");
}

#[test]
fn parse_sse_stream_maps_error_event_to_provider() {
    let body = concat!(
        "data: {\"type\":\"transcript.text.delta\",\"delta\":\"x\"}\n",
        "data: {\"type\":\"error\",\"error\":{\"message\":\"rate limited\"}}\n",
    );
    let err = parse_sse_stream(body).unwrap_err();
    match err {
        AppError::Provider(msg) => assert!(msg.contains("rate limited")),
        other => panic!("expected Provider, got {other:?}"),
    }
}

#[test]
fn parse_sse_stream_without_text_is_internal_error() {
    let body = "data: {\"type\":\"ping\"}\ndata: [DONE]\n";
    let err = parse_sse_stream(body).unwrap_err();
    assert!(matches!(err, AppError::Internal(_)));
}

#[test]
fn parse_sse_stream_propagates_malformed_json() {
    let body = "data: not-json\n";
    let err = parse_sse_stream(body).unwrap_err();
    assert!(matches!(err, AppError::Internal(_)));
}

// --- error classification ---

#[test]
fn classify_http_status_maps_categories() {
    assert!(matches!(
        classify_http_status(401, ""),
        AppError::Provider(_)
    ));
    assert!(matches!(
        classify_http_status(403, ""),
        AppError::Provider(_)
    ));
    assert!(matches!(
        classify_http_status(429, ""),
        AppError::Network(_)
    ));
    assert!(matches!(
        classify_http_status(503, ""),
        AppError::Network(_)
    ));
    assert!(matches!(
        classify_http_status(400, "bad"),
        AppError::Provider(_)
    ));
}
