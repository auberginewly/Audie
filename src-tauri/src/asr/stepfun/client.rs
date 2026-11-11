// StepFun ASR SSE client — request shape, SSE parsing, error classification.
//
// Whole-utterance (batch) provider: POST base64 PCM with Accept: text/event-stream,
// then read the SSE event stream line by line, accumulating `transcript.text.delta`
// fragments and taking `transcript.text.done`'s `text` as the final transcript.
// `[DONE]` ends the stream; an `error` event surfaces as §3.7 Provider.
//
// Field source: reverse-engineered from Voxt + StepFun SSE conventions, NOT
// confirmed against official docs. Fields we could not verify are marked TODO;
// unverified ones (hotwords, prompt) are left off to stay conservative.

use serde_json::json;

use super::config;
use crate::asr::{pcm16_mono_16k_bytes, AsrProvider, AudioData};
use crate::error::{AppError, AppResult};

/// HTTP request timeout. Voxt's WS path used 45s; SSE is whole-utterance so the
/// server may stream deltas for a few seconds after upload — keep it generous.
const REQUEST_TIMEOUT_SECS: u64 = 45;

pub struct StepFunProvider {
    endpoint: String,
    api_key: String,
    /// Resolved at construction: selected model, or DEFAULT_MODEL when blank.
    model: String,
}

impl StepFunProvider {
    pub fn new(endpoint: String, api_key: String, model: String) -> Self {
        let model = if model.trim().is_empty() {
            config::DEFAULT_MODEL.to_string()
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

impl AsrProvider for StepFunProvider {
    fn name(&self) -> &str {
        "stepfun"
    }

    fn transcribe(&self, audio: &AudioData) -> AppResult<String> {
        if self.api_key.trim().is_empty() {
            return Err(AppError::Provider("StepFun API key 未配置".into()));
        }

        // StepFun wants raw PCM16-16k-mono base64, NOT a WAV file — see asr/mod.rs.
        let pcm = pcm16_mono_16k_bytes(audio)?;
        let body = build_request_body(&self.model, &pcm);

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .map_err(|_| AppError::Internal("build stepfun http client".into()))?;

        let resp = client
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .header("Accept", "text/event-stream")
            .json(&body)
            .send()
            .map_err(classify_reqwest_error)?;

        let status = resp.status();
        if !status.is_success() {
            // 消费响应但不将 provider 正文放入诊断；正文可能回显用户内容或凭据。
            let body = resp
                .text()
                .unwrap_or_else(|_| "<无法读取响应体>".to_string());
            return Err(classify_http_status(status.as_u16(), &body));
        }

        // SSE arrives as a text body; the parser walks `data:` lines, accumulates
        // deltas, and returns the final transcript. We read it whole rather than
        // streaming because the product surfaces only the松手后 final (no partials).
        let raw = resp
            .text()
            .map_err(|_| AppError::Network("StepFun 读取响应失败，请检查响应格式或重试".into()))?;
        parse_sse_stream(&raw)
    }
}

/// Build the SSE request body. Pure so the wire shape is unit-tested offline.
///
/// Shape (spec §3): root has sibling `audio` + `input`; `audio.data` is base64 PCM,
/// `input.transcription` carries model/language/enable_itn, `input.format` describes
/// the raw PCM. `language` defaults to "zh"; `enable_itn` enables inverse text
/// normalization (digits/punctuation).
///
/// TODO(official docs): `hotwords` (contextual phrases) and `prompt` (only the
/// `stepaudio-2-asr-pro` model supports it, per Voxt `supportsSSEPrompt`) are NOT
/// sent yet — adding unverified fields risks 4xx rejection. Wire them once the
/// payload keys are confirmed against StepFun's official ASR docs.
fn build_request_body(model: &str, pcm: &[u8]) -> serde_json::Value {
    json!({
        "audio": { "data": base64_encode(pcm) },
        "input": {
            "transcription": {
                "model": model,
                "language": "zh",
                "enable_itn": true,
            },
            "format": {
                "type": "pcm",
                "codec": "pcm_s16le",
                "rate": config::SAMPLE_RATE,
                "bits": 16,
                "channel": 1,
            },
        },
    })
}

/// One parsed SSE event. `Delta` carries an incremental text fragment; `Done`
/// marks the final event (its text, if present, is the complete transcript);
/// `Error` carries a server-reported failure message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SseEvent {
    Delta {
        text: String,
    },
    Done {
        text: Option<String>,
    },
    Error {
        message: String,
    },
    /// A keep-alive / unknown event the reader can skip.
    Other,
}

/// Parse one SSE `data:` line's JSON payload. Returns `Ok(None)` for the stream
/// terminator (`data: [DONE]`); an unparseable payload is an Internal protocol
/// error. The reader accumulates `Delta` fragments and stops on `Done`/`Error`.
/// TODO: confirm event `type` strings and text paths against official docs.
pub fn parse_sse_data(data: &str) -> AppResult<Option<SseEvent>> {
    let data = data.trim();
    if data.is_empty() {
        return Ok(Some(SseEvent::Other));
    }
    if data == "[DONE]" {
        return Ok(None);
    }

    let value: serde_json::Value = serde_json::from_str(data)
        .map_err(|_| AppError::Internal("stepfun SSE decode failed".into()))?;

    // Some StepFun errors arrive as a data-frame `error` object rather than a typed
    // event; check that first so a malformed `type` doesn't mask a real failure.
    if let Some(message) = extract_error_message(&value) {
        return Ok(Some(SseEvent::Error { message }));
    }

    let event_type = value.get("type").and_then(|t| t.as_str()).unwrap_or("");
    match event_type {
        "transcript.text.delta" => Ok(Some(SseEvent::Delta {
            text: value
                .get("delta")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_string(),
        })),
        "transcript.text.done" => Ok(Some(SseEvent::Done {
            text: value
                .get("text")
                .and_then(|t| t.as_str())
                .map(str::to_string),
        })),
        // TODO(official docs): confirm StepFun's error event `type` name. We treat
        // any explicit `error` type or an embedded `error` object as a failure.
        "error" => Ok(Some(SseEvent::Error {
            message: extract_error_message(&value).unwrap_or_else(|| "StepFun ASR 返回错误".into()),
        })),
        _ => Ok(Some(SseEvent::Other)),
    }
}

/// 只判断服务端是否报告错误；provider 的 message 可能回显原文或凭据。
fn extract_error_message(value: &serde_json::Value) -> Option<String> {
    value
        .get("error")
        .map(|_| "StepFun ASR 返回错误，请检查模型、额度或连接设置".into())
}

/// Walk a full SSE response body line by line, accumulating `delta` fragments and
/// taking the `done` text as the final transcript. Pure (no network) so the
/// end-to-end accumulation is unit-tested offline.
///
/// Precedence: a `done` text wins; otherwise the concatenated deltas are returned.
/// An `error` event short-circuits to §3.7 Provider. An empty/transcript-less
/// stream is an Internal protocol error (we expected at least one text event).
pub fn parse_sse_stream(body: &str) -> AppResult<String> {
    let mut accumulated = String::new();
    let mut final_text: Option<String> = None;
    let mut saw_text_event = false;

    for line in body.lines() {
        let line = line.trim_end_matches('\r');
        // SSE field lines look like `data: {...}` / `event: error`. We only act on
        // `data:`; `event:`/`id:`/comments are skipped (the payload `type` drives us).
        let Some(payload) = line.strip_prefix("data:") else {
            continue;
        };

        match parse_sse_data(payload)? {
            None => break, // [DONE]
            Some(SseEvent::Delta { text }) => {
                saw_text_event = true;
                accumulated.push_str(&text);
            }
            Some(SseEvent::Done { text }) => {
                saw_text_event = true;
                if let Some(text) = text {
                    final_text = Some(text);
                }
                break;
            }
            Some(SseEvent::Error { message }) => {
                return Err(AppError::Provider(format!("StepFun ASR 错误：{message}")));
            }
            Some(SseEvent::Other) => {}
        }
    }

    match final_text {
        Some(text) => Ok(text.trim().to_string()),
        None if saw_text_event => Ok(accumulated.trim().to_string()),
        None => Err(AppError::Internal("StepFun SSE 未返回任何转写结果".into())),
    }
}

/// Standard (RFC 4648) base64 encoder. Hand-rolled to avoid pulling a new crate
/// for a ~20-line, well-defined transform — keeps the change inside asr/stepfun/.
fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);

    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((triple >> 12) & 0x3F) as usize] as char);
        // Pad the last group with '=' for the bytes that don't exist.
        out.push(if chunk.len() > 1 {
            ALPHABET[((triple >> 6) & 0x3F) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(triple & 0x3F) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Classify reqwest transport errors into §3.7 Network (timeout / connect / other).
fn classify_reqwest_error(e: reqwest::Error) -> AppError {
    if e.is_timeout() {
        AppError::Network("请求 StepFun 超时，请检查网络或代理".into())
    } else if e.is_connect() {
        AppError::Network("无法连接 StepFun，请检查网络或代理".into())
    } else {
        AppError::Network("StepFun 请求失败，请检查连接设置、网络或代理".into())
    }
}

/// Classify a StepFun HTTP status per §3.7: 401/403 = bad key (Provider), 429 /
/// 5xx = transient (Network), other 4xx = Provider (rejected request).
fn classify_http_status(status: u16, _body: &str) -> AppError {
    match status {
        401 | 403 => AppError::Provider("StepFun API key 无效，请检查设置".into()),
        429 => AppError::Network("StepFun 请求过于频繁或额度受限，请稍后重试".into()),
        500..=599 => AppError::Network(format!("StepFun 服务端异常（{status}）")),
        _ => AppError::Provider(format!(
            "StepFun 拒绝请求（{status}），请检查模型与连接设置"
        )),
    }
}

#[cfg(test)]
#[path = "client/tests.rs"]
mod tests;
