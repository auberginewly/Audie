// Doubao streaming ASR binary frame codec (volcengine bigmodel).
//
// Header layout (big-endian, 4 bytes), then optional sequence (4), optional
// event (4), payload size (4 for typed responses), then payload:
//
//   byte 0: (version << 4) | header_size_in_words
//   byte 1: (message_type << 4) | flags
//   byte 2: (serialization << 4) | compression
//   byte 3: reserved = 0x00
//
// version=0x1, header_size_in_words=0x1 (so 4-byte fixed header).
// message_type: 0x1 FullClientRequest, 0x2 AudioOnlyClientRequest,
//               0x9 FullServerResponse, 0xB ServerAck, 0xF ServerErrorResponse.
// flags bits:   0x1 positive_sequence, 0x2 last_audio, 0x3 negative_audio_packet,
//               0x4 event.
// serialization: 0x0 none, 0x1 JSON. compression: 0x0 none, 0x1 gzip.
//
// Wire reference: agent-project/voxt/Voxt/Transcription/RemoteASRTranscriber+DoubaoTypes.swift
// + RemoteASRTranscriber.swift `buildDoubaoPacket` / `parseDoubaoServerPacket`.
// We re-implement in Rust against the same on-wire protocol, not transliterate.

#![allow(dead_code)] // P2.3 lands the codec; P2.5+ wires the WebSocket client.

use std::io::Read;

use flate2::read::GzDecoder;

pub const VERSION: u8 = 0x1;
pub const HEADER_SIZE_WORDS: u8 = 0x1;

pub mod msg_type {
    pub const FULL_CLIENT_REQUEST: u8 = 0x1;
    pub const AUDIO_ONLY_CLIENT_REQUEST: u8 = 0x2;
    pub const FULL_SERVER_RESPONSE: u8 = 0x9;
    pub const SERVER_ACK: u8 = 0xB;
    pub const SERVER_ERROR_RESPONSE: u8 = 0xF;
}

pub mod flags {
    pub const POSITIVE_SEQUENCE: u8 = 0x1;
    pub const LAST_AUDIO_PACKET: u8 = 0x2;
    pub const NEGATIVE_AUDIO_PACKET: u8 = POSITIVE_SEQUENCE | LAST_AUDIO_PACKET; // 0x3
    pub const EVENT: u8 = 0x4;
}

pub mod serialization {
    pub const NONE: u8 = 0x0;
    pub const JSON: u8 = 0x1;
}

pub mod compression {
    pub const NONE: u8 = 0x0;
    pub const GZIP: u8 = 0x1;
}

#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("doubao frame too short: need {need} bytes, have {have}")]
    InsufficientData { need: usize, have: usize },
    #[error("doubao frame compression unsupported: {0:#x}")]
    UnsupportedCompression(u8),
    #[error("doubao frame message type unsupported: {0:#x}")]
    UnsupportedMessageType(u8),
    #[error("doubao gzip decode failed: {0}")]
    Gzip(String),
    #[error("doubao json decode failed: {0}")]
    Json(String),
    #[error("doubao server error: {message}")]
    ServerError { code: Option<u32>, message: String },
}

/// Build a full-client-request frame (`type=0x1`, JSON payload, seq positive).
/// Used as the first packet on a new WS connection to negotiate session.
pub fn build_full_client_request(sequence: i32, json_payload: &[u8]) -> Vec<u8> {
    build_frame(
        msg_type::FULL_CLIENT_REQUEST,
        flags::POSITIVE_SEQUENCE,
        serialization::JSON,
        compression::NONE,
        Some(sequence),
        json_payload,
    )
}

/// Build an audio-only chunk frame (`type=0x2`, raw PCM payload, seq positive).
/// `pcm` is little-endian 16-bit PCM mono @ 16 kHz (Doubao requirement).
pub fn build_audio_chunk(sequence: i32, pcm: &[u8]) -> Vec<u8> {
    build_frame(
        msg_type::AUDIO_ONLY_CLIENT_REQUEST,
        flags::POSITIVE_SEQUENCE,
        serialization::NONE,
        compression::NONE,
        Some(sequence),
        pcm,
    )
}

/// Build the closing audio packet that tells Doubao the input stream is done.
/// `final_sequence` should be the negative form of the next audio sequence —
/// callers compute it via [`final_sequence_value`].
pub fn build_final_audio(final_sequence: i32) -> Vec<u8> {
    build_frame(
        msg_type::AUDIO_ONLY_CLIENT_REQUEST,
        flags::NEGATIVE_AUDIO_PACKET,
        serialization::NONE,
        compression::NONE,
        Some(final_sequence),
        &[],
    )
}

/// Compute the closing sequence given the next-audio-sequence counter.
/// Matches Voxt's `DoubaoASRConfiguration.finalStreamingSequence`.
pub fn final_sequence_value(next_audio_sequence: i32) -> i32 {
    -next_audio_sequence.max(2)
}

fn build_frame(
    message_type: u8,
    message_flags: u8,
    serialization: u8,
    compression: u8,
    sequence: Option<i32>,
    payload: &[u8],
) -> Vec<u8> {
    let has_sequence = (message_flags & flags::POSITIVE_SEQUENCE) != 0
        || (message_flags & flags::LAST_AUDIO_PACKET) != 0;

    let mut buf = Vec::with_capacity(8 + payload.len() + if has_sequence { 4 } else { 0 });
    buf.push((VERSION << 4) | HEADER_SIZE_WORDS);
    buf.push((message_type << 4) | (message_flags & 0x0F));
    buf.push((serialization << 4) | (compression & 0x0F));
    buf.push(0x00); // reserved

    if has_sequence {
        let seq = sequence.unwrap_or(0);
        buf.extend_from_slice(&seq.to_be_bytes());
    }
    buf.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    buf.extend_from_slice(payload);
    buf
}

/// Parsed server frame. Higher layers decide how to surface text + finality;
/// the codec only normalizes the on-wire shape and decompresses payloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerPacket {
    /// `type=0x9` recognition response with JSON payload (may be empty).
    Response {
        text: Option<String>,
        is_final: bool,
        sequence: Option<i32>,
    },
    /// `type=0xB` server acknowledgement (no recognition content).
    Ack { sequence: Option<i32> },
}

/// Parse one server frame. Returns `Ok(None)` if the buffer is a known but
/// content-free frame (e.g. empty response that only signals end-of-stream
/// via flags). `type=0xF` server errors are returned as `Err(ServerError)`
/// so the caller can route them into `AppError::Provider`.
pub fn parse_server_packet(data: &[u8]) -> Result<ServerPacket, CodecError> {
    if data.len() < 4 {
        return Err(CodecError::InsufficientData {
            need: 4,
            have: data.len(),
        });
    }
    let byte0 = data[0];
    let byte1 = data[1];
    let byte2 = data[2];
    let header_size_words = (byte0 & 0x0F) as usize;
    let header_size_bytes = std::cmp::max(4, header_size_words * 4);
    if data.len() < header_size_bytes {
        return Err(CodecError::InsufficientData {
            need: header_size_bytes,
            have: data.len(),
        });
    }
    let message_type = (byte1 >> 4) & 0x0F;
    let message_flags = byte1 & 0x0F;
    let compression = byte2 & 0x0F;

    let mut cursor = header_size_bytes;

    let has_sequence = (message_flags & flags::POSITIVE_SEQUENCE) != 0
        || (message_flags & flags::LAST_AUDIO_PACKET) != 0;
    let mut header_sequence: Option<i32> = None;
    if has_sequence {
        let seq = read_be_i32(data, cursor)?;
        header_sequence = Some(seq);
        cursor += 4;
    }
    if (message_flags & flags::EVENT) != 0 {
        // event id; we don't consume it but must advance past it.
        let _ = read_be_u32(data, cursor)?;
        cursor += 4;
    }

    match message_type {
        msg_type::FULL_SERVER_RESPONSE => {
            let payload_size = read_be_u32(data, cursor)? as usize;
            cursor += 4;
            if data.len() < cursor + payload_size {
                return Err(CodecError::InsufficientData {
                    need: cursor + payload_size,
                    have: data.len(),
                });
            }
            let raw = &data[cursor..cursor + payload_size];
            let payload = decompress(raw, compression)?;
            // Doubao flags its closing recognition with LAST_AUDIO_PACKET (frame
            // flags 0x3) or a negative sequence; both are checked here.
            let is_final = (message_flags & flags::LAST_AUDIO_PACKET) != 0
                || header_sequence.map(|s| s < 0).unwrap_or(false);
            if payload.is_empty() {
                return Ok(ServerPacket::Response {
                    text: None,
                    is_final,
                    sequence: header_sequence,
                });
            }
            let value: serde_json::Value =
                serde_json::from_slice(&payload).map_err(|e| CodecError::Json(e.to_string()))?;
            let text = extract_text(&value);
            // Doubao marks the closing recognition with `is_last_package: true`
            // (often nested under `result`/`audio_info`), NOT `is_final` — so search
            // the whole JSON tree for it. Without this the receive loop never breaks
            // and the take hangs until the 20s timeout (matches Voxt's `isLastPackage`).
            // Some payloads still carry `is_final`, so honor both.
            let json_final = json_contains_true_flag(&value, "is_last_package")
                || json_contains_true_flag(&value, "is_final");
            Ok(ServerPacket::Response {
                text,
                is_final: is_final || json_final,
                sequence: header_sequence,
            })
        }
        msg_type::SERVER_ACK => Ok(ServerPacket::Ack {
            sequence: header_sequence,
        }),
        msg_type::SERVER_ERROR_RESPONSE => {
            // Error code (BE u32) then payload (BE u32 size + bytes).
            let code = read_be_u32(data, cursor)?;
            cursor += 4;
            let payload_size = read_be_u32(data, cursor)? as usize;
            cursor += 4;
            if data.len() < cursor + payload_size {
                return Err(CodecError::InsufficientData {
                    need: cursor + payload_size,
                    have: data.len(),
                });
            }
            let raw = &data[cursor..cursor + payload_size];
            let payload = decompress(raw, compression)?;
            let message = String::from_utf8_lossy(&payload).into_owned();
            Err(CodecError::ServerError {
                code: Some(code),
                message,
            })
        }
        other => Err(CodecError::UnsupportedMessageType(other)),
    }
}

/// Recursively search a JSON value for `key` set to boolean true. Doubao buries
/// `is_last_package` at varying depths depending on the response shape, so a flat
/// `.get(key)` misses it.
fn json_contains_true_flag(value: &serde_json::Value, key: &str) -> bool {
    match value {
        serde_json::Value::Object(map) => {
            if map.get(key).and_then(|v| v.as_bool()) == Some(true) {
                return true;
            }
            map.values().any(|v| json_contains_true_flag(v, key))
        }
        serde_json::Value::Array(items) => items.iter().any(|v| json_contains_true_flag(v, key)),
        _ => false,
    }
}

fn decompress(raw: &[u8], compression: u8) -> Result<Vec<u8>, CodecError> {
    match compression {
        compression::NONE => Ok(raw.to_vec()),
        compression::GZIP => {
            let mut decoder = GzDecoder::new(raw);
            let mut out = Vec::new();
            decoder
                .read_to_end(&mut out)
                .map_err(|e| CodecError::Gzip(e.to_string()))?;
            Ok(out)
        }
        other => Err(CodecError::UnsupportedCompression(other)),
    }
}

fn read_be_u32(data: &[u8], offset: usize) -> Result<u32, CodecError> {
    if data.len() < offset + 4 {
        return Err(CodecError::InsufficientData {
            need: offset + 4,
            have: data.len(),
        });
    }
    let bytes: [u8; 4] = data[offset..offset + 4].try_into().unwrap();
    Ok(u32::from_be_bytes(bytes))
}

fn read_be_i32(data: &[u8], offset: usize) -> Result<i32, CodecError> {
    read_be_u32(data, offset).map(|v| v as i32)
}

/// Extract recognised text from Doubao's JSON. Doubao puts the canonical text
/// at `result.text`; we also accept a top-level `text` for robustness.
fn extract_text(value: &serde_json::Value) -> Option<String> {
    if let Some(t) = value
        .get("result")
        .and_then(|r| r.get("text"))
        .and_then(|v| v.as_str())
    {
        return Some(t.to_string());
    }
    if let Some(t) = value.get("text").and_then(|v| v.as_str()) {
        return Some(t.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
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
                assert_eq!(message, "invalid token");
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
}
