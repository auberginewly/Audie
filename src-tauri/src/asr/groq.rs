// Groq adapter — whisper-large-v3-turbo over Groq's OpenAI-compatible
// `/audio/transcriptions` endpoint. PROJECT_SPEC.md §4.1.
//
use serde::Deserialize;

use crate::asr::{encode_wav, AsrProvider, AudioData};
use crate::error::{AppError, AppResult};

const ENDPOINT: &str = "https://api.groq.com/openai/v1/audio/transcriptions";
const MODEL: &str = "whisper-large-v3-turbo";

pub struct GroqProvider {
    api_key: String,
}

impl GroqProvider {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }
}

#[derive(Deserialize)]
struct GroqResponse {
    text: String,
}

impl AsrProvider for GroqProvider {
    fn name(&self) -> &str {
        "groq"
    }

    fn transcribe(&self, audio: &AudioData) -> AppResult<String> {
        let wav = encode_wav(audio);

        let file_part = reqwest::blocking::multipart::Part::bytes(wav)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| AppError::Internal(format!("build multipart part: {e}")))?;
        let form = reqwest::blocking::multipart::Form::new()
            .text("model", MODEL)
            .part("file", file_part);

        let client = reqwest::blocking::Client::new();
        let resp = client
            .post(ENDPOINT)
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .map_err(classify_reqwest_error)?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            return Err(classify_http_status(status.as_u16(), &body));
        }

        let parsed: GroqResponse = resp
            .json()
            .map_err(|e| AppError::Provider(format!("解析 Groq 响应失败：{e}")))?;
        Ok(parsed.text.trim().to_string())
    }
}

/// Classify reqwest transport errors into §3.7 friendly categories. Bare
/// reqwest Display is noisy and uninterpretable to end users.
fn classify_reqwest_error(e: reqwest::Error) -> AppError {
    if e.is_timeout() {
        AppError::Network("请求 Groq 超时，请检查网络或代理".into())
    } else if e.is_connect() {
        AppError::Network("无法连接 Groq，请检查网络或代理（中国大陆需走代理）".into())
    } else {
        AppError::Network(format!("Groq 请求失败：{e}"))
    }
}

/// Classify HTTP failure status into §3.7 categories. 401/invalid key is the
/// only non-recoverable case — user has to fix the key. 403 is treated as
/// Network because in practice it's Groq's region-block on mainland China,
/// recoverable by enabling a proxy.
fn classify_http_status(status: u16, body: &str) -> AppError {
    match status {
        401 => AppError::Provider("Groq API key 无效，请检查设置".into()),
        403 => AppError::Network("Groq 拒绝请求（可能是地区限制，中国大陆需走代理）".into()),
        429 => AppError::Network("Groq 请求过于频繁，请稍后重试".into()),
        500..=599 => AppError::Network(format!("Groq 服务端异常（{status}）")),
        _ => {
            let snippet: String = body.chars().take(200).collect();
            AppError::Provider(format!("Groq 拒绝请求（{status}）：{snippet}"))
        }
    }
}
