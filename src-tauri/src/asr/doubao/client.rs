// Doubao streaming ASR WebSocket client (volcengine bigmodel) — P2.5.
//
// Protocol source: Volcengine docs "大模型流式语音识别API" (doc 6561/1354869).
// Voxt was used only as an implementation reference; official docs are the
// protocol source of truth. Binary framing lives in `codec`; this file only
// drives WebSocket connection/auth/send/receive behavior.
//
// P2.5 consumes this from the dev-only `test_doubao_streaming` command (feeding
// a whole PCM16 buffer at once). The recording hot path wires in at P2.6.

#![allow(dead_code)] // transcribe_pcm16 is consumed by the dev command (P2.5) / hot path (P2.6).

use std::time::Duration;

use futures_util::{SinkExt, Stream, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Error as WsError;
use tokio_tungstenite::tungstenite::Message;

use super::{codec, config};
use crate::asr::{AsrProvider, AudioChunk, AudioChunkStream, AudioData, TranscriptStream};
use crate::error::{AppError, AppResult};

/// Pace successive audio frames so we don't flood the server. Doubao's async
/// bigmodel tolerates faster-than-realtime, so this is well under 200ms/frame.
const CHUNK_SEND_INTERVAL_MS: u64 = 20;
/// Upper bound on waiting for the final recognition after the input closes.
const FINAL_TIMEOUT_SECS: u64 = 20;

/// How long the connectivity test waits for the first server frame before treating
/// the already-upgraded session as accepted (auth is verified at the WS upgrade).
const CONNECT_TEST_TIMEOUT_SECS: u64 = 8;

use super::codec::pcm16_from_samples;
use super::config::{build_request, new_request_id};
pub use super::config::{DoubaoAuth, DoubaoStreamConfig};

pub struct DoubaoStreamingProvider {
    config: DoubaoStreamConfig,
}

impl DoubaoStreamingProvider {
    pub fn new(config: DoubaoStreamConfig) -> Self {
        Self { config }
    }
}

impl AsrProvider for DoubaoStreamingProvider {
    fn name(&self) -> &str {
        "doubao_stream"
    }

    fn transcribe(&self, audio: &AudioData) -> AppResult<String> {
        // Batch (whole-buffer) doubao for 撤销/重试 re-transcribe, so a doubao take
        // never falls back to a different ASR model (单模型不降级). Same ws protocol
        // as the live path via transcribe_pcm16, on a private current-thread runtime
        // like run_streaming_runtime (we're already off the main async loop).
        let pcm16 = pcm16_from_samples(&audio.samples, audio.sample_rate, audio.channels)?;
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| AppError::Internal("build doubao runtime".into()))?
            .block_on(transcribe_pcm16(&self.config, &pcm16))
    }

    fn transcribe_stream(&self, chunks: AudioChunkStream) -> AppResult<TranscriptStream> {
        let cfg = self.config.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("audie-doubao-stream".into())
            .spawn(move || {
                let result =
                    run_streaming_runtime(cfg, chunks).map(|text| crate::asr::TranscriptDelta {
                        text,
                        is_final: true,
                        sequence: 0,
                    });
                let _ = tx.send(result);
            })
            .map_err(|_| AppError::Internal("spawn doubao stream".into()))?;
        Ok(rx)
    }
}

/// Stream a whole 16k/mono/16-bit PCM buffer to Doubao and return the final text.
/// Errors are classified per PROJECT_SPEC.md §3.7 (no panics).
pub async fn transcribe_pcm16(cfg: &DoubaoStreamConfig, pcm16: &[u8]) -> AppResult<String> {
    let request = build_request(cfg)?;
    let (ws, _resp) = connect_async(request)
        .await
        .map_err(|err| classify_ws_error(&err, cfg))?;
    let (mut write, read) = ws.split();

    // 1. Negotiate the session with a full-client-request (sequence 1).
    let payload = build_full_request_payload(&new_request_id())?;
    write
        .send(Message::Binary(codec::build_full_client_request(
            1, &payload,
        )))
        .await
        .map_err(|err| classify_ws_error(&err, cfg))?;

    // 2. Receive concurrently while we keep sending audio.
    let receiver = tokio::spawn(receive_loop(read));

    // 3. Push audio frames (sequence ascending from 2).
    let mut sequence: i32 = 2;
    for chunk in pcm16.chunks(config::STREAMING_PACKET_BYTES) {
        write
            .send(Message::Binary(codec::build_audio_chunk(sequence, chunk)))
            .await
            .map_err(|err| classify_ws_error(&err, cfg))?;
        sequence += 1;
        tokio::time::sleep(Duration::from_millis(CHUNK_SEND_INTERVAL_MS)).await;
    }

    // 4. Close the input with the negative final frame.
    let final_sequence = codec::final_sequence_value(sequence);
    write
        .send(Message::Binary(codec::build_final_audio(final_sequence)))
        .await
        .map_err(|err| classify_ws_error(&err, cfg))?;

    // 5. Wait for the final recognition; fall back to a timeout error.
    match tokio::time::timeout(Duration::from_secs(FINAL_TIMEOUT_SECS), receiver).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err(AppError::Internal("doubao receive task failed".into())),
        Err(_) => Err(AppError::Network(
            "doubao stream timed out waiting for final result".into(),
        )),
    }
}

/// Connectivity + auth probe for the model config dialog's 测试 button (Doubao has
/// no /models endpoint, so `test_provider` can't cover it). Opens the WS, sends the
/// full-client-request handshake, and inspects at most one server frame — without
/// sending audio. Doubao authenticates at the WS upgrade, so a failed upgrade
/// (401/403) maps to Provider; an error response frame maps via the codec; a valid
/// frame or short silence means the session was accepted. §3.7, no panics.
pub async fn test_connection(cfg: &DoubaoStreamConfig) -> AppResult<()> {
    let request = build_request(cfg)?;
    let (ws, _resp) = connect_async(request)
        .await
        .map_err(|err| classify_ws_error(&err, cfg))?;
    let (mut write, mut read) = ws.split();

    let payload = build_full_request_payload(&new_request_id())?;
    write
        .send(Message::Binary(codec::build_full_client_request(
            1, &payload,
        )))
        .await
        .map_err(|err| classify_ws_error(&err, cfg))?;

    match tokio::time::timeout(Duration::from_secs(CONNECT_TEST_TIMEOUT_SECS), read.next()).await {
        // An error frame (bad auth/config) surfaces as a codec error → Provider.
        Ok(Some(Ok(Message::Binary(bytes)))) => {
            codec::parse_server_packet(&bytes).map_err(classify_codec_error)?;
            Ok(())
        }
        Ok(Some(Ok(_))) => Ok(()),
        Ok(Some(Err(err))) => Err(classify_ws_error_without_context(&err)),
        // Clean close or no frame in the window: the upgrade (auth) already passed.
        Ok(None) | Err(_) => Ok(()),
    }
}

/// Stream live AudioManager chunks to Doubao and return the final text.
/// Partial server text is logged only; P2 intentionally does not emit
/// `partial-transcript` or alter the overlay.
pub async fn transcribe_audio_chunks(
    cfg: &DoubaoStreamConfig,
    chunks: AudioChunkStream,
) -> AppResult<String> {
    let request = build_request(cfg)?;
    let (ws, _resp) = connect_async(request)
        .await
        .map_err(|err| classify_ws_error(&err, cfg))?;
    let (mut write, read) = ws.split();

    let payload = build_full_request_payload(&new_request_id())?;
    write
        .send(Message::Binary(codec::build_full_client_request(
            1, &payload,
        )))
        .await
        .map_err(|err| classify_ws_error(&err, cfg))?;

    let receiver = tokio::spawn(receive_loop(read));
    let mut sequence: i32 = 2;

    for chunk_result in chunks {
        let chunk = chunk_result?;
        if chunk.is_final {
            break;
        }
        let pcm16 = audio_chunk_to_pcm16(&chunk)?;
        if pcm16.is_empty() {
            continue;
        }
        for packet in pcm16.chunks(config::STREAMING_PACKET_BYTES) {
            write
                .send(Message::Binary(codec::build_audio_chunk(sequence, packet)))
                .await
                .map_err(|err| classify_ws_error(&err, cfg))?;
            sequence += 1;
        }
    }

    // Input is done (is_final sentinel from the audio thread, or the channel
    // closed): tell Doubao no more audio is coming so it emits its final result.
    log::debug!("doubao: input closed after seq={sequence}, sending final frame");
    let final_sequence = codec::final_sequence_value(sequence);
    write
        .send(Message::Binary(codec::build_final_audio(final_sequence)))
        .await
        .map_err(|err| classify_ws_error(&err, cfg))?;

    match tokio::time::timeout(Duration::from_secs(FINAL_TIMEOUT_SECS), receiver).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err(AppError::Internal("doubao receive task failed".into())),
        Err(_) => Err(AppError::Network(
            "doubao stream timed out waiting for final result".into(),
        )),
    }
}

fn run_streaming_runtime(cfg: DoubaoStreamConfig, chunks: AudioChunkStream) -> AppResult<String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| AppError::Internal("build doubao runtime".into()))?
        .block_on(transcribe_audio_chunks(&cfg, chunks))
}

/// 读取至 final 或连接关闭并返回最新转写；诊断仅记录 final 标记与字符数。
async fn receive_loop<S>(mut read: S) -> AppResult<String>
where
    S: Stream<Item = Result<Message, WsError>> + Unpin + Send + 'static,
{
    let mut latest = String::new();
    while let Some(message) = read.next().await {
        let bytes = match message.map_err(|err| classify_ws_error_without_context(&err))? {
            Message::Binary(bytes) => bytes,
            Message::Close(_) => break,
            _ => continue,
        };
        match codec::parse_server_packet(&bytes) {
            Ok(codec::ServerPacket::Response { text, is_final, .. }) => {
                if let Some(text) = text {
                    if !text.is_empty() {
                        latest = text;
                        log::debug!(
                            "doubao recognition update (final={is_final}, chars={})",
                            latest.chars().count()
                        );
                    }
                }
                if is_final {
                    break;
                }
            }
            Ok(codec::ServerPacket::Ack { .. }) => {}
            Err(err) => return Err(classify_codec_error(err)),
        }
    }
    Ok(latest)
}

/// Construct the full-client-request JSON. Streaming PCM uses pcm/raw @ 16k.
/// No corpus/hotwords here — the personal dictionary is P3.
fn build_full_request_payload(request_id: &str) -> AppResult<Vec<u8>> {
    let payload = serde_json::json!({
        "user": { "uid": "audie" },
        "audio": {
            "format": config::STREAMING_AUDIO_FORMAT,
            "codec": config::STREAMING_AUDIO_CODEC,
            "rate": config::STREAMING_SAMPLE_RATE,
            "bits": config::STREAMING_BITS_PER_SAMPLE,
            "channel": config::STREAMING_CHANNELS,
        },
        "request": {
            "reqid": request_id,
            "model_name": "bigmodel",
            "enable_itn": true,
            "enable_punc": true,
            "enable_ddc": true,
            "show_utterances": true,
            "enable_nonstream": false,
        }
    });
    serde_json::to_vec(&payload)
        .map_err(|_| AppError::Internal("doubao request payload encode failed".into()))
}

fn audio_chunk_to_pcm16(chunk: &AudioChunk) -> AppResult<Vec<u8>> {
    pcm16_from_samples(&chunk.samples, chunk.sample_rate, chunk.channels)
}

fn classify_ws_error(err: &WsError, _cfg: &DoubaoStreamConfig) -> AppError {
    // endpoint/resource_id 可由用户输入，不能作为错误上下文回显。
    classify_ws_error_without_context(err)
}

fn classify_ws_error_without_context(err: &WsError) -> AppError {
    match err {
        WsError::Http(response) => {
            let code = response.status().as_u16();
            if code == 401 || code == 403 {
                AppError::Provider(format!("doubao auth rejected (HTTP {code})"))
            } else {
                AppError::Network(format!("doubao handshake failed (HTTP {code})"))
            }
        }
        WsError::Io(_) => {
            AppError::Network("doubao connection failed; check network or proxy".into())
        }
        WsError::ConnectionClosed | WsError::AlreadyClosed => {
            AppError::Network("doubao connection closed".into())
        }
        _ => AppError::Network("doubao websocket failed; check endpoint, network or proxy".into()),
    }
}

fn classify_codec_error(err: codec::CodecError) -> AppError {
    match err {
        // Server-side rejection (e.g. quota, bad request) → Provider.
        codec::CodecError::ServerError { code, .. } => {
            let code = code.map(|c| format!(" {c}")).unwrap_or_default();
            AppError::Provider(format!(
                "doubao server error{code}; check credentials, model and quota"
            ))
        }
        // Anything else is a protocol/invariant violation, not the user's fault.
        _ => AppError::Internal("doubao protocol error; check endpoint or retry".into()),
    }
}

#[cfg(test)]
#[path = "client/tests.rs"]
mod tests;
