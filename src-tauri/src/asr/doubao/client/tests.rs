use super::*;
use crate::asr::AudioChunk;

#[test]
fn full_request_payload_has_expected_audio_fields() {
    let bytes = build_full_request_payload("req-123").expect("payload encodes");
    let value: serde_json::Value = serde_json::from_slice(&bytes).expect("valid json");

    assert_eq!(value["audio"]["format"], config::STREAMING_AUDIO_FORMAT);
    assert_eq!(value["audio"]["codec"], config::STREAMING_AUDIO_CODEC);
    assert_eq!(value["audio"]["rate"], config::STREAMING_SAMPLE_RATE);
    assert_eq!(value["request"]["reqid"], "req-123");
    assert_eq!(value["request"]["model_name"], "bigmodel");
    assert_eq!(value["request"]["enable_nonstream"], false);
}

#[test]
fn audio_chunk_to_pcm16_downmixes_stereo() {
    let chunk = AudioChunk {
        samples: vec![1.0, -1.0, 0.5, 0.5],
        sample_rate: config::STREAMING_SAMPLE_RATE,
        channels: 2,
        sequence: 1,
        is_final: false,
    };

    let pcm = audio_chunk_to_pcm16(&chunk).expect("chunk converts");

    assert_eq!(pcm, vec![0, 0, 255, 63]);
}

#[test]
fn audio_chunk_to_pcm16_resamples_to_16k_mono() {
    let chunk = AudioChunk {
        samples: vec![0.0, 1.0, 0.0, -1.0],
        sample_rate: 8_000,
        channels: 1,
        sequence: 1,
        is_final: false,
    };

    let pcm = audio_chunk_to_pcm16(&chunk).expect("chunk converts");

    assert_eq!(pcm.len(), 16);
    assert_eq!(&pcm[0..2], &[0, 0]);
    assert_eq!(&pcm[4..6], &[255, 127]);
    assert_eq!(&pcm[12..14], &[1, 128]);
}

#[test]
fn privacy_websocket_and_codec_failures_do_not_echo_private_context() {
    let cfg = DoubaoStreamConfig {
        endpoint: "wss://PRIVATE_USER:PRIVATE_KEY@example.test/?token=PRIVATE_QUERY".into(),
        resource_id: "PRIVATE_RESOURCE".into(),
        auth: config::DoubaoAuth::NewConsole {
            api_key: "PRIVATE_KEY".into(),
        },
    };
    let response = tokio_tungstenite::tungstenite::http::Response::builder()
        .status(403)
        .body(None)
        .unwrap();
    let error = classify_ws_error(&WsError::Http(response), &cfg);
    assert_eq!(error.code(), "provider");
    assert!(!error.to_string().contains("PRIVATE_"));
    let error =
        classify_ws_error_without_context(&WsError::Io(std::io::Error::other("PRIVATE_URL")));
    assert_eq!(error.code(), "network");
    assert!(!error.to_string().contains("PRIVATE_"));
    for raw in [
        codec::CodecError::ServerError {
            code: Some(123),
            message: "PRIVATE_BODY".into(),
        },
        codec::CodecError::Json("PRIVATE_JSON".into()),
    ] {
        let error = classify_codec_error(raw);
        assert!(!error.to_string().contains("PRIVATE_"));
    }
}
