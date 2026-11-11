use serde::{Deserialize, Serialize};

use super::{sanitize::sanitize_enhance_output, LlmProvider};
use crate::error::{AppError, AppResult};

const CHAT_COMPLETIONS_PATH: &str = "/chat/completions";

pub struct OpenAiCompatibleProvider {
    api_key: String,
    base_url: String,
    model: String,
}

impl OpenAiCompatibleProvider {
    pub fn new(api_key: String, base_url: String, model: String) -> Self {
        Self {
            api_key,
            base_url,
            model,
        }
    }
}

#[derive(Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
}

#[derive(Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatResponseMessage,
}

#[derive(Deserialize)]
struct ChatResponseMessage {
    content: String,
}

impl LlmProvider for OpenAiCompatibleProvider {
    fn name(&self) -> &str {
        "openai_compatible"
    }

    fn enhance(&self, text: &str, prompt: &str) -> AppResult<String> {
        let request = ChatCompletionRequest {
            model: self.model.clone(),
            temperature: 0.2,
            messages: vec![
                ChatMessage {
                    role: "system",
                    // Append /no_think so Qwen3 skips its <think> reasoning phase —
                    // that phase blows the hot-path latency budget ("松手即出"). Non-Qwen
                    // models ignore the unknown directive. Request-time only; the stored
                    // (user-editable) prompt is untouched.
                    content: format!("{prompt}\n\n/no_think"),
                },
                ChatMessage {
                    // Just the raw transcript — all polish instructions live in the
                    // (user-editable) system prompt, which already declares "the user
                    // message is the raw transcript". No instruction wrapper here, or
                    // it duplicates the system prompt and fights its anti-injection stance.
                    role: "user",
                    content: text.to_string(),
                },
            ],
        };

        let client = reqwest::blocking::Client::new();
        let resp = client
            .post(chat_completions_endpoint(&self.base_url))
            .bearer_auth(&self.api_key)
            .json(&request)
            .send()
            .map_err(classify_reqwest_error)?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            return Err(classify_http_status(status.as_u16(), &body));
        }

        let parsed: ChatCompletionResponse = resp.json().map_err(|_| {
            AppError::Provider("解析 OpenAI-compatible 响应失败，请检查响应格式或重试".into())
        })?;
        let content = parsed
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content)
            .ok_or_else(|| AppError::Provider("OpenAI-compatible 返回空润色结果".into()))?;
        // Strip <think> reasoning / preambles / code fences before the text reaches
        // the cursor; fall back to the raw transcript if only reasoning remains.
        let cleaned = sanitize_enhance_output(&content, text);
        if cleaned.is_empty() {
            return Err(AppError::Provider(
                "OpenAI-compatible 返回空润色结果".into(),
            ));
        }
        Ok(cleaned)
    }
}
pub(crate) fn chat_completions_endpoint(base_url: &str) -> String {
    format!(
        "{}{}",
        base_url.trim().trim_end_matches('/'),
        CHAT_COMPLETIONS_PATH
    )
}

fn classify_reqwest_error(e: reqwest::Error) -> AppError {
    if e.is_timeout() {
        AppError::Network("请求 OpenAI-compatible 超时，请检查网络或代理".into())
    } else if e.is_connect() {
        AppError::Network("无法连接 OpenAI-compatible，请检查网络或代理".into())
    } else {
        AppError::Network("OpenAI-compatible 请求失败，请检查连接设置、网络或代理".into())
    }
}

pub(crate) fn classify_http_status(status: u16, _body: &str) -> AppError {
    match status {
        401 => AppError::Provider("OpenAI-compatible API key 无效，请检查设置".into()),
        403 => AppError::Network("OpenAI-compatible 拒绝请求，可能是网络、代理或地区限制".into()),
        429 => AppError::Network("OpenAI-compatible 请求过于频繁或额度受限，请稍后重试".into()),
        500..=599 => AppError::Network(format!("OpenAI-compatible 服务端异常（{status}）")),
        _ => AppError::Provider(format!(
            "OpenAI-compatible 拒绝请求（{status}），请检查模型与连接设置"
        )),
    }
}

#[cfg(test)]
mod tests;
