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
use tokio_tungstenite::tungstenite::handshake::client::Request;
use tokio_tungstenite::tungstenite::http::{HeaderMap, HeaderValue};
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

/// Doubao auth mode. New console uses one API key; old console uses AppID plus
/// Access Token.
#[derive(Clone)]
pub enum DoubaoAuth {
    NewConsole {
        api_key: String,
    },
    OldConsole {
        app_id: String,
        access_token: String,
    },
}

impl DoubaoAuth {
    pub fn from_settings(app_id: String, api_key_or_access_token: String) -> Self {
        if app_id.trim().is_empty() {
            Self::NewConsole {
                api_key: api_key_or_access_token,
            }
        } else {
            Self::OldConsole {
                app_id,
                access_token: api_key_or_access_token,
            }
        }
    }
}

/// Connection parameters. Endpoint + resource id come from settings (non-secret).
#[derive(Clone)]
pub struct DoubaoStreamConfig {
    pub endpoint: String,
    pub auth: DoubaoAuth,
    pub resource_id: String,
}

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
            .map_err(|err| AppError::Internal(format!("build doubao runtime: {err}")))?
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
            .map_err(|err| AppError::Internal(format!("spawn doubao stream: {err}")))?;
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
        Ok(Err(join_err)) => Err(AppError::Internal(format!(
            "doubao receive task failed: {join_err}"
        ))),
        Err(_) => Err(AppError::Network(
            "doubao stream timed out waiting for final result".into(),
        )),
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
        Ok(Err(join_err)) => Err(AppError::Internal(format!(
            "doubao receive task failed: {join_err}"
        ))),
        Err(_) => Err(AppError::Network(
            "doubao stream timed out waiting for final result".into(),
        )),
    }
}

fn run_streaming_runtime(cfg: DoubaoStreamConfig, chunks: AudioChunkStream) -> AppResult<String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| AppError::Internal(format!("build doubao runtime: {err}")))?
        .block_on(transcribe_audio_chunks(&cfg, chunks))
}

/// Read frames until the server marks the result final (or the socket closes),
/// logging each non-empty text. Returns the latest recognized text.
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
                        log::info!("doubao transcript (final={is_final}): {latest}");
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

/// Build the WS handshake request carrying Doubao's auth headers. Old console
/// uses AppID + Access Token; new console uses X-Api-Key only.
fn build_request(cfg: &DoubaoStreamConfig) -> AppResult<Request> {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    let mut request = cfg
        .endpoint
        .as_str()
        .into_client_request()
        .map_err(|err| AppError::Network(format!("doubao endpoint invalid: {err}")))?;
    let request_id = new_request_id();
    let headers = request.headers_mut();
    match &cfg.auth {
        DoubaoAuth::NewConsole { api_key } => {
            insert_header(headers, "X-Api-Key", api_key.trim())?;
        }
        DoubaoAuth::OldConsole {
            app_id,
            access_token,
        } => {
            insert_header(headers, "X-Api-App-Key", app_id.trim())?;
            insert_header(headers, "X-Api-Access-Key", access_token.trim())?;
        }
    }
    insert_header(headers, "X-Api-Resource-Id", &cfg.resource_id)?;
    insert_header(headers, "X-Api-Request-Id", &request_id)?;
    insert_header(headers, "X-Api-Connect-Id", &request_id)?;
    insert_header(headers, "X-Api-Sequence", "-1")?;
    Ok(request)
}

fn insert_header(headers: &mut HeaderMap, name: &'static str, value: &str) -> AppResult<()> {
    let header_value = HeaderValue::from_str(value)
        .map_err(|err| AppError::Internal(format!("doubao header {name} invalid: {err}")))?;
    headers.insert(name, header_value);
    Ok(())
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
        .map_err(|err| AppError::Internal(format!("doubao request payload encode failed: {err}")))
}

fn audio_chunk_to_pcm16(chunk: &AudioChunk) -> AppResult<Vec<u8>> {
    pcm16_from_samples(&chunk.samples, chunk.sample_rate, chunk.channels)
}

/// Downmix to mono, resample to Doubao's 16 kHz, and encode as little-endian
/// 16-bit PCM. Shared by the live chunk path and the whole-buffer batch path.
fn pcm16_from_samples(samples: &[f32], sample_rate: u32, channels: u16) -> AppResult<Vec<u8>> {
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

fn new_request_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("audie-{nanos:032x}")
}

fn classify_ws_error(err: &WsError, cfg: &DoubaoStreamConfig) -> AppError {
    match err {
        // Handshake rejected: 401/403 means bad token/permissions (Provider),
        // anything else is a transport-level failure (Network).
        WsError::Http(response) => {
            let code = response.status().as_u16();
            if code == 401 || code == 403 {
                AppError::Provider(format!(
                    "doubao auth rejected (HTTP {code}); endpoint={}, resource_id={}",
                    cfg.endpoint, cfg.resource_id
                ))
            } else {
                AppError::Network(format!(
                    "doubao handshake failed (HTTP {code}); endpoint={}, resource_id={}",
                    cfg.endpoint, cfg.resource_id
                ))
            }
        }
        other => classify_ws_error_without_context(other),
    }
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
        WsError::Io(io_err) => AppError::Network(format!("doubao connection io: {io_err}")),
        WsError::ConnectionClosed | WsError::AlreadyClosed => {
            AppError::Network("doubao connection closed".into())
        }
        other => AppError::Network(format!("doubao websocket error: {other}")),
    }
}

fn classify_codec_error(err: codec::CodecError) -> AppError {
    match err {
        // Server-side rejection (e.g. quota, bad request) → Provider.
        codec::CodecError::ServerError { code, message } => {
            let code = code.map(|c| format!(" {c}")).unwrap_or_default();
            AppError::Provider(format!("doubao server error{code}: {message}"))
        }
        // Anything else is a protocol/invariant violation, not the user's fault.
        other => AppError::Internal(format!("doubao protocol error: {other}")),
    }
}

#[cfg(test)]
mod tests {
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
    fn build_request_uses_new_console_api_key_when_app_id_is_blank() {
        let cfg = DoubaoStreamConfig {
            endpoint: config::DEFAULT_ENDPOINT.into(),
            auth: DoubaoAuth::from_settings(" ".into(), "api-key".into()),
            resource_id: config::DEFAULT_RESOURCE_ID.into(),
        };

        let request = build_request(&cfg).expect("request builds");
        let headers = request.headers();

        assert_eq!(headers["X-Api-Key"], "api-key");
        assert!(!headers.contains_key("X-Api-App-Key"));
        assert!(!headers.contains_key("X-Api-Access-Key"));
        assert_eq!(headers["X-Api-Sequence"], "-1");
    }

    #[test]
    fn build_request_uses_old_console_app_id_and_access_token_when_app_id_is_set() {
        let cfg = DoubaoStreamConfig {
            endpoint: config::DEFAULT_ENDPOINT.into(),
            auth: DoubaoAuth::from_settings("app-id".into(), "access-token".into()),
            resource_id: config::DEFAULT_RESOURCE_ID.into(),
        };

        let request = build_request(&cfg).expect("request builds");
        let headers = request.headers();

        assert_eq!(headers["X-Api-App-Key"], "app-id");
        assert_eq!(headers["X-Api-Access-Key"], "access-token");
        assert!(!headers.contains_key("X-Api-Key"));
        assert_eq!(headers["X-Api-Sequence"], "-1");
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
}
