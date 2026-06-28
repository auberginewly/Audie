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
    api_key: String,
    /// Resolved at construction: selected model, or DEFAULT_MODEL when blank.
    model: String,
}

impl StepFunProvider {
    pub fn new(api_key: String, model: String) -> Self {
        let model = if model.trim().is_empty() {
            config::DEFAULT_MODEL.to_string()
        } else {
            model
        };
        Self { api_key, model }
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
            .map_err(|e| AppError::Internal(format!("build stepfun http client: {e}")))?;

        let resp = client
            .post(config::ENDPOINT)
            .bearer_auth(&self.api_key)
            .header("Accept", "text/event-stream")
            .json(&body)
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

        // SSE arrives as a text body; the parser walks `data:` lines, accumulates
        // deltas, and returns the final transcript. We read it whole rather than
        // streaming because the product surfaces only the松手后 final (no partials).
        let raw = resp
            .text()
            .map_err(|e| AppError::Network(format!("StepFun 读取响应失败：{e}")))?;
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
        .map_err(|err| AppError::Internal(format!("stepfun SSE decode failed: {err}")))?;

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

/// Pull a human-readable message out of an `error` shape. StepFun's exact error
/// schema is unverified, so accept both `{"error":{"message":...}}` and a flat
/// `{"error":"..."}`/`{"message":...}`. TODO: confirm against official docs.
fn extract_error_message(value: &serde_json::Value) -> Option<String> {
    if let Some(err) = value.get("error") {
        if let Some(message) = err.get("message").and_then(|m| m.as_str()) {
            return Some(message.to_string());
        }
        if let Some(message) = err.as_str() {
            return Some(message.to_string());
        }
        // An `error` object with no readable message still signals failure.
        return Some(err.to_string());
    }
    None
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
        AppError::Network(format!("StepFun 请求失败：{e}"))
    }
}

/// Classify a StepFun HTTP status per §3.7: 401/403 = bad key (Provider), 429 /
/// 5xx = transient (Network), other 4xx = Provider (rejected request).
fn classify_http_status(status: u16, body: &str) -> AppError {
    match status {
        401 | 403 => AppError::Provider("StepFun API key 无效，请检查设置".into()),
        429 => AppError::Network("StepFun 请求过于频繁或额度受限，请稍后重试".into()),
        500..=599 => AppError::Network(format!("StepFun 服务端异常（{status}）")),
        _ => {
            let snippet: String = body.chars().take(200).collect();
            AppError::Provider(format!("StepFun 拒绝请求（{status}）：{snippet}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_falls_back_to_default_model_when_blank() {
        let provider = StepFunProvider::new("key".into(), "  ".into());
        assert_eq!(provider.model, config::DEFAULT_MODEL);
    }

    #[test]
    fn new_keeps_explicit_model() {
        let provider = StepFunProvider::new("key".into(), "stepaudio-2-asr-pro".into());
        assert_eq!(provider.model, "stepaudio-2-asr-pro");
    }

    #[test]
    fn transcribe_rejects_empty_key_as_provider_error() {
        let provider = StepFunProvider::new("   ".into(), String::new());
        let audio = AudioData {
            samples: vec![0.0],
            sample_rate: 16_000,
            channels: 1,
        };
        let err = provider.transcribe(&audio).unwrap_err();
        assert!(matches!(err, AppError::Provider(_)));
    }

    // --- request body construction ---

    #[test]
    fn build_request_body_has_base64_audio_and_pcm_format() {
        let body = build_request_body(config::DEFAULT_MODEL, &[0x00, 0x01, 0xFF]);

        // base64("\x00\x01\xFF") == "AAH/"
        assert_eq!(body["audio"]["data"], "AAH/");

        let transcription = &body["input"]["transcription"];
        assert_eq!(transcription["model"], config::DEFAULT_MODEL);
        assert_eq!(transcription["language"], "zh");
        assert_eq!(transcription["enable_itn"], true);

        let format = &body["input"]["format"];
        assert_eq!(format["type"], "pcm");
        assert_eq!(format["codec"], "pcm_s16le");
        assert_eq!(format["rate"], config::SAMPLE_RATE);
        assert_eq!(format["bits"], 16);
        assert_eq!(format["channel"], 1);
    }

    #[test]
    fn build_request_body_threads_explicit_model() {
        let body = build_request_body("stepaudio-2-asr-pro", &[]);
        assert_eq!(
            body["input"]["transcription"]["model"],
            "stepaudio-2-asr-pro"
        );
    }

    // --- base64 ---

    #[test]
    fn base64_encode_matches_rfc4648_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    // --- single-event parsing ---

    #[test]
    fn parse_delta_event() {
        let event = parse_sse_data(r#"{"type":"transcript.text.delta","delta":"你好"}"#)
            .unwrap()
            .unwrap();
        assert_eq!(
            event,
            SseEvent::Delta {
                text: "你好".into()
            }
        );
    }

    #[test]
    fn parse_done_event_with_full_text() {
        let event = parse_sse_data(r#"{"type":"transcript.text.done","text":"你好世界"}"#)
            .unwrap()
            .unwrap();
        assert_eq!(
            event,
            SseEvent::Done {
                text: Some("你好世界".into())
            }
        );
    }

    #[test]
    fn parse_stream_terminator_returns_none() {
        assert_eq!(parse_sse_data("[DONE]").unwrap(), None);
    }

    #[test]
    fn parse_unknown_type_is_other() {
        let event = parse_sse_data(r#"{"type":"ping"}"#).unwrap().unwrap();
        assert_eq!(event, SseEvent::Other);
    }

    #[test]
    fn parse_explicit_error_event_carries_message() {
        let event = parse_sse_data(r#"{"type":"error","error":{"message":"quota exceeded"}}"#)
            .unwrap()
            .unwrap();
        assert_eq!(
            event,
            SseEvent::Error {
                message: "quota exceeded".into()
            }
        );
    }

    #[test]
    fn parse_embedded_error_object_without_type_is_error() {
        let event = parse_sse_data(r#"{"error":"bad request"}"#)
            .unwrap()
            .unwrap();
        assert_eq!(
            event,
            SseEvent::Error {
                message: "bad request".into()
            }
        );
    }

    #[test]
    fn parse_invalid_json_is_internal_error() {
        let err = parse_sse_data("not json").unwrap_err();
        assert!(matches!(err, AppError::Internal(_)));
    }

    // --- full-stream parsing ---

    #[test]
    fn parse_sse_stream_prefers_done_text_over_deltas() {
        let body = concat!(
            "data: {\"type\":\"transcript.text.delta\",\"delta\":\"你\"}\n",
            "data: {\"type\":\"transcript.text.delta\",\"delta\":\"好\"}\n",
            "data: {\"type\":\"transcript.text.done\",\"text\":\"你好世界\"}\n",
            "data: [DONE]\n",
        );
        assert_eq!(parse_sse_stream(body).unwrap(), "你好世界");
    }

    #[test]
    fn parse_sse_stream_falls_back_to_accumulated_deltas() {
        // `done` without a `text` field → return the concatenated deltas.
        let body = concat!(
            "data: {\"type\":\"transcript.text.delta\",\"delta\":\"hello \"}\n",
            "data: {\"type\":\"transcript.text.delta\",\"delta\":\"world\"}\n",
            "data: {\"type\":\"transcript.text.done\"}\n",
        );
        assert_eq!(parse_sse_stream(body).unwrap(), "hello world");
    }

    #[test]
    fn parse_sse_stream_skips_event_and_blank_lines() {
        // `event:`/`id:`/comment lines and CRLF endings must not break accumulation.
        let body = "event: message\r\ndata: {\"type\":\"transcript.text.delta\",\"delta\":\"a\"}\r\n\r\ndata: {\"type\":\"transcript.text.done\",\"text\":\"abc\"}\r\n";
        assert_eq!(parse_sse_stream(body).unwrap(), "abc");
    }

    #[test]
    fn parse_sse_stream_maps_error_event_to_provider() {
        let body = concat!(
            "data: {\"type\":\"transcript.text.delta\",\"delta\":\"x\"}\n",
            "data: {\"type\":\"error\",\"error\":{\"message\":\"rate limited\"}}\n",
        );
        let err = parse_sse_stream(body).unwrap_err();
        match err {
            AppError::Provider(msg) => assert!(msg.contains("rate limited")),
            other => panic!("expected Provider, got {other:?}"),
        }
    }

    #[test]
    fn parse_sse_stream_without_text_is_internal_error() {
        let body = "data: {\"type\":\"ping\"}\ndata: [DONE]\n";
        let err = parse_sse_stream(body).unwrap_err();
        assert!(matches!(err, AppError::Internal(_)));
    }

    #[test]
    fn parse_sse_stream_propagates_malformed_json() {
        let body = "data: not-json\n";
        let err = parse_sse_stream(body).unwrap_err();
        assert!(matches!(err, AppError::Internal(_)));
    }

    // --- error classification ---

    #[test]
    fn classify_http_status_maps_categories() {
        assert!(matches!(
            classify_http_status(401, ""),
            AppError::Provider(_)
        ));
        assert!(matches!(
            classify_http_status(403, ""),
            AppError::Provider(_)
        ));
        assert!(matches!(
            classify_http_status(429, ""),
            AppError::Network(_)
        ));
        assert!(matches!(
            classify_http_status(503, ""),
            AppError::Network(_)
        ));
        assert!(matches!(
            classify_http_status(400, "bad"),
            AppError::Provider(_)
        ));
    }
}
