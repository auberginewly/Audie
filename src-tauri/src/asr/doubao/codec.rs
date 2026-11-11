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

use super::config;
use crate::error::{AppError, AppResult};

/// Downmix to mono, resample to Doubao's 16 kHz, and encode as little-endian
/// 16-bit PCM. Shared by the live chunk path and the whole-buffer batch path.
pub(super) fn pcm16_from_samples(
    samples: &[f32],
    sample_rate: u32,
    channels: u16,
) -> AppResult<Vec<u8>> {
    let mono = downmix_to_mono(samples, channels);
    let resampled = resample_linear(&mono, sample_rate, config::STREAMING_SAMPLE_RATE)?;
    let mut pcm = Vec::with_capacity(resampled.len() * 2);
    for sample in resampled {
        let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        pcm.extend_from_slice(&value.to_le_bytes());
    }
    Ok(pcm)
}

fn downmix_to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    let channels = channels.max(1) as usize;
    if channels == 1 {
        return samples.to_vec();
    }

    samples
        .chunks(channels)
        .map(|frame| frame.iter().copied().sum::<f32>() / frame.len() as f32)
        .collect()
}

fn resample_linear(samples: &[f32], from_rate: u32, to_rate: u32) -> AppResult<Vec<f32>> {
    if from_rate == 0 {
        return Err(AppError::Device("audio chunk sample rate is zero".into()));
    }
    if samples.is_empty() || from_rate == to_rate {
        return Ok(samples.to_vec());
    }

    let output_len = ((samples.len() as u64 * to_rate as u64) / from_rate as u64) as usize;
    if output_len == 0 {
        return Ok(Vec::new());
    }

    let ratio = from_rate as f32 / to_rate as f32;
    let mut output = Vec::with_capacity(output_len);
    for index in 0..output_len {
        let source = index as f32 * ratio;
        let left = source.floor() as usize;
        let right = (left + 1).min(samples.len() - 1);
        let t = source - left as f32;
        output.push(samples[left] * (1.0 - t) + samples[right] * t);
    }
    Ok(output)
}

#[cfg(test)]
#[path = "codec/tests.rs"]
mod tests;
