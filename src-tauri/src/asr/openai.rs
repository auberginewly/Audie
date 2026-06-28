// OpenAI ASR adapter — `/audio/transcriptions`, model selectable from settings
// (whisper-1 default; gpt-4o-transcribe / -mini variants). Key from keychain.

use serde::Deserialize;

use crate::asr::{encode_wav, AsrProvider, AudioData};
use crate::error::{AppError, AppResult};

const ENDPOINT: &str = "https://api.openai.com/v1/audio/transcriptions";
const MODEL: &str = "whisper-1";

pub struct OpenAiProvider {
    api_key: String,
    /// Resolved at construction: the selected model, or the built-in default when
    /// settings left asr_model empty. Kept owned so transcribe() needs no fallback.
    model: String,
}

impl OpenAiProvider {
    pub fn new(api_key: String, model: String) -> Self {
        let model = if model.trim().is_empty() {
            MODEL.to_string()
        } else {
            model
        };
        Self { api_key, model }
    }
}

#[derive(Deserialize)]
struct OpenAiResponse {
    text: String,
}

impl AsrProvider for OpenAiProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn transcribe(&self, audio: &AudioData) -> AppResult<String> {
        let wav = encode_wav(audio);

        let file_part = reqwest::blocking::multipart::Part::bytes(wav)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| AppError::Internal(format!("build multipart part: {e}")))?;
        let form = reqwest::blocking::multipart::Form::new()
            .text("model", self.model.clone())
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

        let parsed: OpenAiResponse = resp
            .json()
            .map_err(|e| AppError::Provider(format!("解析 OpenAI 响应失败：{e}")))?;
        Ok(parsed.text.trim().to_string())
    }
}

fn classify_reqwest_error(e: reqwest::Error) -> AppError {
    if e.is_timeout() {
        AppError::Network("请求 OpenAI 超时，请检查网络或代理".into())
    } else if e.is_connect() {
        AppError::Network("无法连接 OpenAI，请检查网络或代理".into())
    } else {
        AppError::Network(format!("OpenAI 请求失败：{e}"))
    }
}

fn classify_http_status(status: u16, body: &str) -> AppError {
    match status {
        401 => AppError::Provider("OpenAI API key 无效，请检查设置".into()),
        403 => AppError::Network("OpenAI 拒绝请求，可能是网络、代理或地区限制".into()),
        429 => AppError::Network("OpenAI 请求过于频繁或额度受限，请稍后重试".into()),
        500..=599 => AppError::Network(format!("OpenAI 服务端异常（{status}）")),
        _ => {
            let snippet: String = body.chars().take(200).collect();
            AppError::Provider(format!("OpenAI 拒绝请求（{status}）：{snippet}"))
        }
    }
}
