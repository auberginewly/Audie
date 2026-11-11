// 智谱 GLM ASR adapter — HTTP, OpenAI-compatible `/audio/transcriptions`.
//
// Batch (non-streaming) transcribe: encode the take as a WAV, POST it as
// multipart, parse the final text out of the response. Key from keychain
// (glm_api_key). No partial events — this is the 松手即出 final path.
//
// Field source: the endpoint + OpenAI-compatible shape are reverse-engineered from
// Voxt, NOT confirmed against 智谱 official docs (SPEC §5.2.1). Where the wire shape
// is unconfirmed we implement conservatively and mark TODO; see inline notes on
// the `stream` field and response parsing.

use serde::Deserialize;

use crate::asr::{encode_wav, AsrProvider, AudioData};
use crate::error::{AppError, AppResult};

/// OpenAI-compatible transcription endpoint (BigModel open platform).
/// TODO: confirm path + whether a non-stream `stream=false` form is accepted.
pub const ENDPOINT: &str = "https://open.bigmodel.cn/api/paas/v4/audio/transcriptions";

/// Default model when settings leave `asr_model` empty.
pub const DEFAULT_MODEL: &str = "glm-asr-1";

/// Keychain key id for the GLM API key.
pub const SECRET_API_KEY: &str = "glm_api_key";

pub struct GlmProvider {
    endpoint: String,
    api_key: String,
    /// Resolved at construction: selected model, or DEFAULT_MODEL when blank.
    model: String,
}

impl GlmProvider {
    pub fn new(endpoint: String, api_key: String, model: String) -> Self {
        let model = if model.trim().is_empty() {
            DEFAULT_MODEL.to_string()
        } else {
            model
        };
        Self {
            endpoint,
            api_key,
            model,
        }
    }
}

/// Non-stream JSON response shape (OpenAI-compatible `{ "text": "..." }`).
/// TODO: confirm GLM returns this exact field for `stream=false` (Voxt only ever
/// drove the SSE path, so the non-stream JSON field name is unverified).
#[derive(Deserialize)]
struct GlmResponse {
    text: String,
}

impl AsrProvider for GlmProvider {
    fn name(&self) -> &str {
        "glm"
    }

    fn transcribe(&self, audio: &AudioData) -> AppResult<String> {
        let wav = encode_wav(audio);

        let form = build_form(&self.model, wav)?;

        let client = reqwest::blocking::Client::new();
        let resp = client
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            // Voxt lists SSE first, but we ask for a single JSON so the batch path
            // can parse one body; the parser still tolerates SSE if the server
            // ignores `stream=false`.
            .header("Accept", "application/json, text/event-stream, text/plain")
            .multipart(form)
            .send()
            .map_err(classify_reqwest_error)?;

        let status = resp.status();
        if !status.is_success() {
            // Distinguish "server sent an empty error body" from "we failed to read
            // the body" so logs/diagnostics aren't both rendered as an empty snippet.
            let body = resp
                .text()
                .unwrap_or_else(|_| "<无法读取响应体>".to_string());
            return Err(classify_http_status(status.as_u16(), &body));
        }

        let body = resp
            .text()
            .map_err(|e| AppError::Network(format!("读取 GLM 响应失败：{e}")))?;
        parse_transcript(&body)
    }
}

/// Build the multipart body. Separated from `transcribe` so the field layout is
/// unit-testable without a network round-trip.
fn build_form(model: &str, wav: Vec<u8>) -> AppResult<reqwest::blocking::multipart::Form> {
    let file_part = reqwest::blocking::multipart::Part::bytes(wav)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| AppError::Internal(format!("build multipart part: {e}")))?;

    Ok(reqwest::blocking::multipart::Form::new()
        .text("model", model.to_string())
        // Request a single non-stream JSON. TODO: confirm GLM honors `stream=false`;
        // Voxt always sent "true". The parser below copes with either reply.
        .text("stream", "false")
        .part("file", file_part))
}

/// Extract the final transcript from a GLM response body.
///
/// GLM may answer either way and the wire shape is unconfirmed (SPEC §5.2.1), so
/// this stays conservative:
///   1. whole-body JSON `{ "text": ... }` (the non-stream OpenAI shape we ask for);
///   2. an SSE stream we collect into one final string by accumulating the text
///      fragments across `data:` lines (server ignored `stream=false`).
///
/// A body we can't read text out of is a protocol surprise → §3.7 Internal.
fn parse_transcript(body: &str) -> AppResult<String> {
    // Fast path: a single JSON object (non-stream).
    if let Ok(parsed) = serde_json::from_str::<GlmResponse>(body) {
        return Ok(parsed.text.trim().to_string());
    }

    // SSE fallback: accumulate text fragments. TODO: confirm GLM's SSE event schema
    // (event types, whether deltas are incremental or cumulative) on real hardware;
    // here we append fragments in order, which is correct for pure-incremental deltas.
    let mut aggregate = String::new();
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let payload = line.strip_prefix("data:").map(str::trim).unwrap_or(line);
        if payload == "[DONE]" {
            break;
        }
        if let Some(fragment) = extract_text_fragment(payload) {
            aggregate.push_str(&fragment);
        }
    }

    if aggregate.trim().is_empty() {
        let snippet: String = body.chars().take(200).collect();
        return Err(AppError::Internal(format!(
            "无法从 GLM 响应解析文本：{snippet}"
        )));
    }
    Ok(aggregate.trim().to_string())
}

/// Pull a text fragment out of one SSE payload line. Tries the JSON fields GLM is
/// likely to use (OpenAI-compatible). TODO: lock to the real field once docs are
/// available — we deliberately do NOT replicate Voxt's "try every field + loose
/// regex" parser (that's盲码 per CLAUDE.md); we only accept structured JSON here.
fn extract_text_fragment(payload: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(payload).ok()?;
    if let Some(t) = value.get("text").and_then(|v| v.as_str()) {
        return Some(t.to_string());
    }
    if let Some(t) = value.get("delta").and_then(|v| v.as_str()) {
        return Some(t.to_string());
    }
    None
}

/// Classify reqwest transport errors into §3.7 friendly categories.
fn classify_reqwest_error(e: reqwest::Error) -> AppError {
    if e.is_timeout() {
        AppError::Network("请求 GLM 超时，请检查网络或代理".into())
    } else if e.is_connect() {
        AppError::Network("无法连接 GLM，请检查网络或代理".into())
    } else {
        AppError::Network(format!("GLM 请求失败：{e}"))
    }
}

/// Classify HTTP failure status into §3.7 categories. 401/403 invalid-or-rejected
/// key is a Provider error (user must fix the key); 429/5xx are recoverable Network.
/// TODO: cross-check against 智谱 official error-code table — these mappings mirror
/// the OpenAI-compatible convention, not a confirmed GLM table.
fn classify_http_status(status: u16, body: &str) -> AppError {
    match status {
        401 | 403 => AppError::Provider("GLM API key 无效或被拒绝，请检查设置".into()),
        429 => AppError::Network("GLM 请求过于频繁或额度受限，请稍后重试".into()),
        500..=599 => AppError::Network(format!("GLM 服务端异常（{status}）")),
        _ => {
            let snippet: String = body.chars().take(200).collect();
            AppError::Provider(format!("GLM 拒绝请求（{status}）：{snippet}"))
        }
    }
}

#[cfg(test)]
#[path = "glm/tests.rs"]
mod tests;
