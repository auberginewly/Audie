// Doubao streaming ASR configuration defaults (volcengine bigmodel) — P2.5.
//
// Protocol source: Volcengine docs "大模型流式语音识别API" (doc 6561/1354869).
// Endpoint + resource id are non-secret store fields. Old-console AppID and the
// new-console API key / old-console Access Token are keychain secrets.

/// Default async bigmodel streaming endpoint.
pub const DEFAULT_ENDPOINT: &str = "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async";

/// Legacy ASR 1.0 duration resource id kept for settings migration.
pub const LEGACY_RESOURCE_ID: &str = "volc.bigasr.sauc.duration";

/// Default resource id for the duration-billed seed ASR 2.0 SAUC stream.
pub const DEFAULT_RESOURCE_ID: &str = "volc.seedasr.sauc.duration";

/// Keychain key id for the Doubao AppID (sensitive per Voxt's model).
pub const SECRET_APP_ID: &str = "doubao_app_id";

/// Keychain key id for the new-console API Key or old-console Access Token.
/// Keep the stored id stable for users who already saved `doubao_access_token`.
pub const SECRET_API_KEY_OR_ACCESS_TOKEN: &str = "doubao_access_token";

// Streaming PCM parameters (P2.5). Doubao bigmodel SAUC expects 16 kHz / mono /
// 16-bit PCM.
pub const STREAMING_SAMPLE_RATE: u32 = 16_000;
pub const STREAMING_BITS_PER_SAMPLE: u16 = 16;
pub const STREAMING_CHANNELS: u16 = 1;
pub const STREAMING_AUDIO_FORMAT: &str = "pcm";
pub const STREAMING_AUDIO_CODEC: &str = "raw";

/// Recommended per-frame PCM payload: 200ms of 16k/mono/16-bit audio.
/// = 16000 * 16 * 1 / 8 / 5 = 6400 bytes (Voxt `recommendedStreamingPacketBytes`).
pub const STREAMING_PACKET_BYTES: usize = 6_400;

use tokio_tungstenite::tungstenite::handshake::client::Request;
use tokio_tungstenite::tungstenite::http::{HeaderMap, HeaderValue};

use crate::error::{AppError, AppResult};

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

/// Build the WS handshake request carrying Doubao's auth headers. Old console
/// uses AppID + Access Token; new console uses X-Api-Key only.
pub(super) fn build_request(cfg: &DoubaoStreamConfig) -> AppResult<Request> {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    let mut request = cfg
        .endpoint
        .as_str()
        .into_client_request()
        .map_err(|_| AppError::Network("doubao endpoint invalid".into()))?;
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
    let header_value = HeaderValue::from_str(value).map_err(|_| {
        AppError::Internal("doubao authentication header invalid; check credentials".into())
    })?;
    headers.insert(name, header_value);
    Ok(())
}

pub(super) fn new_request_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("audie-{nanos:032x}")
}

#[cfg(test)]
mod tests;
