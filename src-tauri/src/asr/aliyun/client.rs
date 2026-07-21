// DashScope Fun-ASR realtime WebSocket client — auth + session driver.
//
// Batch shape (like OpenAI): `transcribe(&AudioData) -> String` runs one full
// WS round-trip on a private current-thread runtime (we're already off the main
// async loop, on the transcription worker thread). Flow:
//   connect (Bearer header) → run-task (JSON text) → wait task-started
//   → stream raw PCM binary frames (200ms slices) → finish-task (JSON text)
//   → accumulate result-generated text → task-finished returns the final.
// We do NOT emit partials or touch the overlay — the streaming hot path stays on
// Doubao; these 4 providers are松手后整段转写.
//
// Field source: DashScope realtime conventions reverse-engineered from Voxt, NOT
// confirmed against official docs. TODO: verify the bearer-token header name, the
// WS upgrade auth, and the run-task/event field names on real hardware.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::handshake::client::Request;
use tokio_tungstenite::tungstenite::http::{HeaderMap, HeaderValue};
use tokio_tungstenite::tungstenite::Error as WsError;
use tokio_tungstenite::tungstenite::Message;

use super::{codec, config};
use crate::asr::{pcm16_mono_16k_bytes, AsrProvider, AudioData};
use crate::error::{AppError, AppResult};

/// Per-frame PCM payload: 200ms of 16k/mono/16-bit = 16000 * 2 * 0.2 = 6400 bytes.
/// Matches the Doubao streaming cadence (spec §3 recommends reusing it).
const STREAMING_PACKET_BYTES: usize = 6_400;

/// Pace successive audio frames so we don't flood the duplex socket. DashScope's
/// realtime path tolerates faster-than-realtime; well under 200ms/frame.
const CHUNK_SEND_INTERVAL_MS: u64 = 20;

/// Upper bound on waiting for `task-started` after run-task.
const START_TIMEOUT_SECS: u64 = 10;

/// Upper bound on waiting for `task-finished` after finish-task.
const FINAL_TIMEOUT_SECS: u64 = 20;

/// WS connection parameters. Endpoint defaults to the official const; the model
/// flows from settings (`asr_model`, blank = adapter default).
#[derive(Clone)]
pub struct AliyunStreamConfig {
    pub endpoint: String,
    pub api_key: String,
    pub model: String,
}

pub struct AliyunProvider {
    config: AliyunStreamConfig,
}

impl AliyunProvider {
    pub fn new(endpoint: String, api_key: String, model: String) -> Self {
        let model = if model.trim().is_empty() {
            config::DEFAULT_MODEL.to_string()
        } else {
            model
        };
        Self {
            config: AliyunStreamConfig {
                endpoint,
                api_key,
                model,
            },
        }
    }
}

impl AsrProvider for AliyunProvider {
    fn name(&self) -> &str {
        "aliyun_fun"
    }

    fn transcribe(&self, audio: &AudioData) -> AppResult<String> {
        // Encode to 16k/mono/16-bit raw PCM once, then run the whole WS round-trip
        // on a private current-thread runtime — the same pattern Doubao's batch path
        // uses (we're already on the transcription worker thread, not the UI loop).
        let pcm16 = pcm16_mono_16k_bytes(audio)?;
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| AppError::Internal(format!("build aliyun runtime: {err}")))?
            .block_on(transcribe_pcm16(&self.config, &pcm16))
    }
}

/// Stream a whole 16k/mono/16-bit PCM buffer to DashScope and return the final text.
/// Errors are classified per PROJECT_SPEC.md §3.7 (no panics, no unwrap on the hot path).
async fn transcribe_pcm16(cfg: &AliyunStreamConfig, pcm16: &[u8]) -> AppResult<String> {
    let request = build_request(cfg)?;
    let (ws, _resp) = connect_async(request).await.map_err(classify_ws_error)?;
    let (mut write, mut read) = ws.split();

    let task_id = new_task_id();

    // 1. Open the task and wait for the server to acknowledge it's ready. DashScope's
    //    duplex protocol requires task-started before any audio is sent.
    write
        .send(Message::Text(codec::build_run_task(&task_id, &cfg.model)?))
        .await
        .map_err(classify_ws_error)?;
    wait_for_started(&mut read).await?;

    // 2. Push raw PCM as binary frames (no base64 — that's the D/Qwen path).
    for chunk in pcm16.chunks(STREAMING_PACKET_BYTES) {
        write
            .send(Message::Binary(chunk.to_vec()))
            .await
            .map_err(classify_ws_error)?;
        tokio::time::sleep(Duration::from_millis(CHUNK_SEND_INTERVAL_MS)).await;
    }

    // 3. Close the input so the server emits its terminal task-finished.
    write
        .send(Message::Text(codec::build_finish_task(&task_id)?))
        .await
        .map_err(classify_ws_error)?;

    // 4. Drain result-generated events and return the text from task-finished.
    //    A finished task with no recognition (pure silence) is an empty string, not
    //    an error (spec §5: no有效语音 → 空串正常返回).
    match tokio::time::timeout(Duration::from_secs(FINAL_TIMEOUT_SECS), collect_final(read)).await {
        Ok(result) => result,
        Err(_) => Err(AppError::Network("通义 ASR 等待最终结果超时".into())),
    }
}

/// Read frames until `task-started`. Any failure/error event short-circuits to
/// Provider; a closed socket before task-started is a Network failure.
async fn wait_for_started<S>(read: &mut S) -> AppResult<()>
where
    S: futures_util::Stream<Item = Result<Message, WsError>> + Unpin,
{
    let deadline = tokio::time::timeout(Duration::from_secs(START_TIMEOUT_SECS), async {
        while let Some(message) = read.next().await {
            let raw = match next_text_frame(message)? {
                Frame::Text(text) => text,
                Frame::Skip => continue,
                Frame::Closed => break,
            };
            match codec::parse_server_event(&raw)? {
                codec::ServerEvent::Started => return Ok(()),
                // Some servers stream a result before/without an explicit started ack;
                // treat any non-error event as "session is live" and proceed.
                codec::ServerEvent::Result { .. } | codec::ServerEvent::Finished { .. } => {
                    return Ok(())
                }
            }
        }
        Err(AppError::Network("通义 ASR 连接在任务开始前关闭".into()))
    })
    .await;

    match deadline {
        Ok(result) => result,
        Err(_) => Err(AppError::Network("通义 ASR 等待任务开始超时".into())),
    }
}

/// Accumulate recognition text across result-generated events, returning the text
/// once task-finished arrives (or the socket closes). The final event's text wins
/// when present; otherwise we keep the latest non-empty incremental result.
async fn collect_final<S>(mut read: S) -> AppResult<String>
where
    S: futures_util::Stream<Item = Result<Message, WsError>> + Unpin,
{
    let mut latest = String::new();
    while let Some(message) = read.next().await {
        let raw = match next_text_frame(message)? {
            Frame::Text(text) => text,
            Frame::Skip => continue,
            Frame::Closed => break,
        };
        match codec::parse_server_event(&raw)? {
            codec::ServerEvent::Started => {}
            codec::ServerEvent::Result { text } => {
                if let Some(text) = text {
                    if !text.is_empty() {
                        latest = text;
                    }
                }
            }
            codec::ServerEvent::Finished { text } => {
                if let Some(text) = text {
                    if !text.is_empty() {
                        latest = text;
                    }
                }
                return Ok(latest);
            }
        }
    }
    // Socket closed without task-finished: return whatever we accumulated rather than
    // erroring, so a clean-but-early close still yields the recognized text.
    Ok(latest)
}

/// One classified received WS frame: a JSON text payload to parse, a frame to skip
/// (ping/pong), or a socket close that ends the read loop.
enum Frame {
    Text(String),
    Skip,
    Closed,
}

/// Normalize one received WS message. A WS transport error maps to Network (§3.7).
fn next_text_frame(message: Result<Message, WsError>) -> AppResult<Frame> {
    match message.map_err(classify_ws_error)? {
        Message::Text(text) => Ok(Frame::Text(text)),
        // DashScope control/result frames are JSON text; tolerate a binary-wrapped
        // JSON payload just in case the server frames it that way.
        Message::Binary(bytes) => Ok(Frame::Text(String::from_utf8_lossy(&bytes).into_owned())),
        Message::Close(_) => Ok(Frame::Closed),
        _ => Ok(Frame::Skip), // ping / pong / frame — keep reading.
    }
}

/// Build the WS handshake request carrying the DashScope Bearer auth header.
fn build_request(cfg: &AliyunStreamConfig) -> AppResult<Request> {
    let mut request = cfg
        .endpoint
        .as_str()
        .into_client_request()
        .map_err(|err| AppError::Network(format!("通义 endpoint 无效: {err}")))?;
    let (name, value) = auth_header(cfg);
    insert_header(request.headers_mut(), name, &value)?;
    Ok(request)
}

fn insert_header(headers: &mut HeaderMap, name: &'static str, value: &str) -> AppResult<()> {
    let header_value = HeaderValue::from_str(value)
        .map_err(|err| AppError::Internal(format!("通义 header {name} 非法: {err}")))?;
    headers.insert(name, header_value);
    Ok(())
}

/// The WS handshake auth header pair (name, value). DashScope authenticates the WS
/// upgrade with a bearer token. Kept as a pure builder so it's testable without a
/// live socket. TODO: confirm whether DashScope also needs an `X-DashScope-*` header.
fn auth_header(cfg: &AliyunStreamConfig) -> (&'static str, String) {
    ("Authorization", format!("Bearer {}", cfg.api_key.trim()))
}

/// 32-char lowercase hex task_id with no hyphens (spec §4.1). Derived from the
/// nanosecond clock split across two halves — DashScope only requires uniqueness
/// per connection, not cryptographic randomness.
fn new_task_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    // u128 → 32 hex digits exactly.
    format!("{nanos:032x}")
}

/// Classify a DashScope WS error per §3.7: handshake 401/403 = bad key (Provider);
/// any other handshake/transport failure = Network. No panics.
fn classify_ws_error(err: WsError) -> AppError {
    match err {
        WsError::Http(response) => classify_upgrade_status(response.status().as_u16()),
        WsError::Io(io_err) => AppError::Network(format!("通义连接 io 错误: {io_err}")),
        WsError::ConnectionClosed | WsError::AlreadyClosed => {
            AppError::Network("通义连接已关闭".into())
        }
        other => AppError::Network(format!("通义 websocket 错误: {other}")),
    }
}

/// Classify a DashScope HTTP/WS upgrade failure per §3.7: 401/403 = bad key
/// (Provider); anything else transport-level = Network.
fn classify_upgrade_status(code: u16) -> AppError {
    if code == 401 || code == 403 {
        AppError::Provider(format!("通义鉴权失败（HTTP {code}），请检查 API Key"))
    } else {
        AppError::Network(format!("通义连接失败（HTTP {code}）"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_falls_back_to_default_model_when_blank() {
        let provider =
            AliyunProvider::new(config::DEFAULT_ENDPOINT.into(), "key".into(), "  ".into());
        assert_eq!(provider.config.model, config::DEFAULT_MODEL);
        assert_eq!(provider.config.endpoint, config::DEFAULT_ENDPOINT);
    }

    #[test]
    fn new_keeps_explicit_endpoint() {
        let provider = AliyunProvider::new(
            "wss://aliyun.example.test/inference".into(),
            "key".into(),
            config::DEFAULT_MODEL.into(),
        );
        assert_eq!(
            provider.config.endpoint,
            "wss://aliyun.example.test/inference"
        );
    }

    #[test]
    fn new_keeps_explicit_model() {
        let provider = AliyunProvider::new(
            config::DEFAULT_ENDPOINT.into(),
            "key".into(),
            "paraformer-realtime-v2".into(),
        );
        assert_eq!(provider.config.model, "paraformer-realtime-v2");
    }

    #[test]
    fn auth_header_is_bearer_token() {
        let cfg = AliyunStreamConfig {
            endpoint: config::DEFAULT_ENDPOINT.into(),
            api_key: " sk-abc ".into(),
            model: config::DEFAULT_MODEL.into(),
        };
        let (name, value) = auth_header(&cfg);
        assert_eq!(name, "Authorization");
        assert_eq!(value, "Bearer sk-abc");
    }

    #[test]
    fn build_request_sets_authorization_header() {
        let cfg = AliyunStreamConfig {
            endpoint: config::DEFAULT_ENDPOINT.into(),
            api_key: "sk-xyz".into(),
            model: config::DEFAULT_MODEL.into(),
        };
        let request = build_request(&cfg).expect("request builds");
        assert_eq!(request.headers()["Authorization"], "Bearer sk-xyz");
    }

    #[test]
    fn new_task_id_is_32_hex_chars() {
        let id = new_task_id();
        assert_eq!(id.len(), 32);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(!id.contains('-'));
    }

    #[test]
    fn classify_upgrade_status_maps_auth_vs_network() {
        assert!(matches!(
            classify_upgrade_status(401),
            AppError::Provider(_)
        ));
        assert!(matches!(
            classify_upgrade_status(403),
            AppError::Provider(_)
        ));
        assert!(matches!(classify_upgrade_status(500), AppError::Network(_)));
    }

    #[test]
    fn next_text_frame_unwraps_text_and_ends_on_close() {
        assert!(matches!(
            next_text_frame(Ok(Message::Text("{}".into()))).unwrap(),
            Frame::Text(t) if t == "{}"
        ));
        assert!(matches!(
            next_text_frame(Ok(Message::Close(None))).unwrap(),
            Frame::Closed
        ));
        assert!(matches!(
            next_text_frame(Ok(Message::Ping(vec![]))).unwrap(),
            Frame::Skip
        ));
    }

    #[test]
    fn next_text_frame_maps_transport_error_to_network() {
        match next_text_frame(Err(WsError::ConnectionClosed)) {
            Err(AppError::Network(_)) => {}
            other => panic!("expected Network error, got {:?}", other.map(|_| "frame")),
        }
    }
}
