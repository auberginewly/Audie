use super::*;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::io::Write;

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut enc = GzEncoder::new(Vec::new(), Compression::default());
    enc.write_all(bytes).unwrap();
    enc.finish().unwrap()
}

#[test]
fn full_client_request_layout_matches_wire_format() {
    let payload = br#"{"hello":1}"#;
    let frame = build_full_client_request(1, payload);

    // Header: version=1, header_size=1 → 0x11; type=1, flags=1 → 0x11;
    // serialization=JSON(1), compression=none(0) → 0x10; reserved 0x00.
    assert_eq!(&frame[0..4], &[0x11, 0x11, 0x10, 0x00]);
    // BE i32 sequence = 1
    assert_eq!(&frame[4..8], &1i32.to_be_bytes());
    // BE u32 payload size + payload
    assert_eq!(&frame[8..12], &(payload.len() as u32).to_be_bytes());
    assert_eq!(&frame[12..], payload);
}

#[test]
fn audio_chunk_carries_raw_pcm_with_positive_sequence() {
    let pcm = vec![0x00u8, 0x01, 0xFF, 0x7F];
    let frame = build_audio_chunk(7, &pcm);
    // type=2, flags=1 → 0x21; serialization=none, compression=none → 0x00
    assert_eq!(&frame[0..4], &[0x11, 0x21, 0x00, 0x00]);
    assert_eq!(&frame[4..8], &7i32.to_be_bytes());
    assert_eq!(&frame[8..12], &(pcm.len() as u32).to_be_bytes());
    assert_eq!(&frame[12..], pcm.as_slice());
}

#[test]
fn final_audio_packet_uses_negative_sequence_and_empty_payload() {
    let seq = final_sequence_value(5);
    assert_eq!(seq, -5);
    let frame = build_final_audio(seq);
    // type=2, flags=3 (negative=positive|last) → 0x23
    assert_eq!(&frame[0..4], &[0x11, 0x23, 0x00, 0x00]);
    assert_eq!(&frame[4..8], &(-5i32).to_be_bytes());
    assert_eq!(&frame[8..12], &0u32.to_be_bytes());
    assert_eq!(frame.len(), 12);
}

#[test]
fn final_sequence_clamps_to_minus_two() {
    assert_eq!(final_sequence_value(0), -2);
    assert_eq!(final_sequence_value(1), -2);
    assert_eq!(final_sequence_value(2), -2);
    assert_eq!(final_sequence_value(10), -10);
}

#[test]
fn parse_uncompressed_response_extracts_result_text() {
    let json = "{\"result\":{\"text\":\"你好世界\"}}".as_bytes();
    let frame = build_synthetic_response(json, flags::POSITIVE_SEQUENCE, 3, compression::NONE);

    let parsed = parse_server_packet(&frame).unwrap();
    match parsed {
        ServerPacket::Response {
            text,
            is_final,
            sequence,
        } => {
            assert_eq!(text.as_deref(), Some("你好世界"));
            assert!(!is_final);
            assert_eq!(sequence, Some(3));
        }
        other => panic!("expected Response, got {other:?}"),
    }
}

#[test]
fn parse_gzip_response_decompresses_then_extracts_text() {
    let json = br#"{"result":{"text":"hello"}}"#;
    let compressed = gzip(json);
    let frame =
        build_synthetic_response(&compressed, flags::POSITIVE_SEQUENCE, 7, compression::GZIP);

    let parsed = parse_server_packet(&frame).unwrap();
    match parsed {
        ServerPacket::Response { text, .. } => assert_eq!(text.as_deref(), Some("hello")),
        other => panic!("expected Response, got {other:?}"),
    }
}

#[test]
fn parse_response_marks_final_when_last_audio_flag_is_set() {
    let json = br#"{"result":{"text":"done"}}"#;
    let frame = build_synthetic_response(
        json,
        flags::POSITIVE_SEQUENCE | flags::LAST_AUDIO_PACKET,
        12,
        compression::NONE,
    );
    let parsed = parse_server_packet(&frame).unwrap();
    match parsed {
        ServerPacket::Response { is_final, .. } => assert!(is_final),
        other => panic!("expected Response, got {other:?}"),
    }
}

#[test]
fn parse_response_marks_final_when_sequence_is_negative() {
    let json = br#"{"result":{"text":"done"}}"#;
    let frame = build_synthetic_response(json, flags::POSITIVE_SEQUENCE, -4, compression::NONE);
    let parsed = parse_server_packet(&frame).unwrap();
    match parsed {
        ServerPacket::Response {
            is_final, sequence, ..
        } => {
            assert!(is_final);
            assert_eq!(sequence, Some(-4));
        }
        other => panic!("expected Response, got {other:?}"),
    }
}

#[test]
fn parse_response_marks_final_when_is_last_package_nested() {
    // Doubao signals the closing recognition with `is_last_package: true` nested
    // under `result`, on a *positive* sequence with no last-audio flag — the exact
    // shape that used to hang the receive loop until the 20s timeout.
    let json = br#"{"result":{"text":"done","is_last_package":true}}"#;
    let frame = build_synthetic_response(json, flags::POSITIVE_SEQUENCE, 9, compression::NONE);
    let parsed = parse_server_packet(&frame).unwrap();
    match parsed {
        ServerPacket::Response { text, is_final, .. } => {
            assert!(is_final, "is_last_package must mark the response final");
            assert_eq!(text.as_deref(), Some("done"));
        }
        other => panic!("expected Response, got {other:?}"),
    }
}

#[test]
fn parse_response_returns_none_text_for_empty_payload() {
    let frame = build_synthetic_response(&[], flags::POSITIVE_SEQUENCE, 1, compression::NONE);
    let parsed = parse_server_packet(&frame).unwrap();
    match parsed {
        ServerPacket::Response { text, .. } => assert!(text.is_none()),
        other => panic!("expected Response, got {other:?}"),
    }
}

#[test]
fn parse_server_error_is_routed_as_provider_error() {
    let body = b"invalid token";
    let mut frame = vec![
        (VERSION << 4) | HEADER_SIZE_WORDS,
        (msg_type::SERVER_ERROR_RESPONSE << 4) | flags::POSITIVE_SEQUENCE,
        (serialization::NONE << 4) | compression::NONE,
        0x00,
    ];
    frame.extend_from_slice(&1i32.to_be_bytes()); // sequence
    frame.extend_from_slice(&45_000_001u32.to_be_bytes()); // error code
    frame.extend_from_slice(&(body.len() as u32).to_be_bytes());
    frame.extend_from_slice(body);

    let err = parse_server_packet(&frame).unwrap_err();
    match err {
        CodecError::ServerError { code, message } => {
            assert_eq!(code, Some(45_000_001));
            assert_eq!(message, "provider rejected request");
        }
        other => panic!("expected ServerError, got {other:?}"),
    }
}

#[test]
fn parse_rejects_truncated_frame_with_insufficient_data() {
    let err = parse_server_packet(&[0x11, 0x91]).unwrap_err();
    assert!(matches!(err, CodecError::InsufficientData { .. }));
}

#[test]
fn parse_rejects_unknown_message_type() {
    let mut frame = vec![(VERSION << 4) | HEADER_SIZE_WORDS, 0x70, 0x00, 0x00];
    frame.extend_from_slice(&0u32.to_be_bytes());
    let err = parse_server_packet(&frame).unwrap_err();
    assert!(matches!(err, CodecError::UnsupportedMessageType(0x7)));
}

#[test]
fn parse_rejects_unsupported_compression() {
    let mut frame = vec![
        (VERSION << 4) | HEADER_SIZE_WORDS,
        (msg_type::FULL_SERVER_RESPONSE << 4) | flags::POSITIVE_SEQUENCE,
        0x0F, // serialization=0, compression=0xF (unknown)
        0x00,
    ];
    frame.extend_from_slice(&1i32.to_be_bytes());
    frame.extend_from_slice(&3u32.to_be_bytes());
    frame.extend_from_slice(b"abc");
    let err = parse_server_packet(&frame).unwrap_err();
    assert!(matches!(err, CodecError::UnsupportedCompression(0xF)));
}

#[test]
fn parse_ack_returns_sequence_only() {
    let mut frame = vec![
        (VERSION << 4) | HEADER_SIZE_WORDS,
        (msg_type::SERVER_ACK << 4) | flags::POSITIVE_SEQUENCE,
        0x00,
        0x00,
    ];
    frame.extend_from_slice(&42i32.to_be_bytes());
    let parsed = parse_server_packet(&frame).unwrap();
    assert_eq!(parsed, ServerPacket::Ack { sequence: Some(42) });
}

/// Helper: build a synthetic FullServerResponse frame for parser tests.
fn build_synthetic_response(
    body: &[u8],
    message_flags: u8,
    sequence: i32,
    compression_kind: u8,
) -> Vec<u8> {
    let mut frame = vec![
        (VERSION << 4) | HEADER_SIZE_WORDS,
        (msg_type::FULL_SERVER_RESPONSE << 4) | (message_flags & 0x0F),
        (serialization::JSON << 4) | (compression_kind & 0x0F),
        0x00,
    ];
    let has_sequence = (message_flags & flags::POSITIVE_SEQUENCE) != 0
        || (message_flags & flags::LAST_AUDIO_PACKET) != 0;
    if has_sequence {
        frame.extend_from_slice(&sequence.to_be_bytes());
    }
    frame.extend_from_slice(&(body.len() as u32).to_be_bytes());
    frame.extend_from_slice(body);
    frame
}

#[test]
fn privacy_codec_error_display_and_debug_omit_server_body() {
    let body = b"PRIVATE_TRANSCRIPT_KEY";
    let mut frame = vec![0x11, msg_type::SERVER_ERROR_RESPONSE << 4, 0x00, 0x00];
    frame.extend_from_slice(&123u32.to_be_bytes());
    frame.extend_from_slice(&(body.len() as u32).to_be_bytes());
    frame.extend_from_slice(body);
    let error = parse_server_packet(&frame).unwrap_err();
    assert!(!error.to_string().contains("PRIVATE_"));
    assert!(!format!("{error:?}").contains("PRIVATE_"));
}
