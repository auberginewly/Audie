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
use crate::error::{AppError, AppResult};

/// Pace successive audio frames so we don't flood the server. Doubao's async
/// bigmodel tolerates faster-than-realtime, so this is well under 200ms/frame.
const CHUNK_SEND_INTERVAL_MS: u64 = 20;
/// Upper bound on waiting for the final recognition after the input closes.
const FINAL_TIMEOUT_SECS: u64 = 20;

/// Doubao auth mode. New console uses one API key; old console uses AppID plus
/// Access Token.
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
pub struct DoubaoStreamConfig {
    pub endpoint: String,
    pub auth: DoubaoAuth,
    pub resource_id: String,
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
}
