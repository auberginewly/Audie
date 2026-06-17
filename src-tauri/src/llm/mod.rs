// LLM provider abstraction + OpenAI-compatible adapter. PROJECT_SPEC.md §4.1.

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::managers::enhance::EnhanceConfig;

const CHAT_COMPLETIONS_PATH: &str = "/chat/completions";

pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &str;
    fn enhance(&self, text: &str, prompt: &str) -> AppResult<String>;
}

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
                    content: prompt.to_string(),
                },
                ChatMessage {
                    role: "user",
                    content: format!("请润色以下语音转写文本，只输出润色后的文本：\n\n{text}"),
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

        let parsed: ChatCompletionResponse = resp
            .json()
            .map_err(|e| AppError::Provider(format!("解析 OpenAI-compatible 响应失败：{e}")))?;
        parsed
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content.trim().to_string())
            .filter(|content| !content.is_empty())
            .ok_or_else(|| AppError::Provider("OpenAI-compatible 返回空润色结果".into()))
    }
}

pub(crate) fn build_provider(config: &EnhanceConfig) -> AppResult<Box<dyn LlmProvider>> {
    match config.llm_provider.as_str() {
        "openai_compatible" => Ok(Box::new(OpenAiCompatibleProvider::new(
            required_key(
                &config.openai_compatible_api_key,
                "OpenAI-compatible API key 未配置，请先到设置页填写",
            )?,
            required_setting(
                &config.openai_compatible_base_url,
                "OpenAI-compatible base URL 未配置，请先到设置页填写",
            )?,
            required_setting(
                &config.openai_compatible_model,
                "OpenAI-compatible model 未配置，请先到设置页填写",
            )?,
        ))),
        other => Err(AppError::Internal(format!(
            "unsupported LLM provider: {other}"
        ))),
    }
}

fn required_key(value: &Option<String>, message: &str) -> AppResult<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| AppError::Provider(message.into()))
}

fn required_setting(value: &str, message: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(AppError::Provider(message.into()))
    } else {
        Ok(trimmed.to_string())
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
        AppError::Network(format!("OpenAI-compatible 请求失败：{e}"))
    }
}

pub(crate) fn classify_http_status(status: u16, body: &str) -> AppError {
    match status {
        401 => AppError::Provider("OpenAI-compatible API key 无效，请检查设置".into()),
        403 => AppError::Network("OpenAI-compatible 拒绝请求，可能是网络、代理或地区限制".into()),
        429 => AppError::Network("OpenAI-compatible 请求过于频繁或额度受限，请稍后重试".into()),
        500..=599 => AppError::Network(format!("OpenAI-compatible 服务端异常（{status}）")),
        _ => {
            let snippet: String = body.chars().take(200).collect();
            AppError::Provider(format!(
                "OpenAI-compatible 拒绝请求（{status}）：{snippet}"
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppError;
    use crate::managers::enhance::EnhanceConfig;

    #[test]
    fn openai_compatible_requires_keychain_secret() {
        let config = EnhanceConfig {
            llm_provider: "openai_compatible".into(),
            enhance_enabled: true,
            enhance_prompt: "去口水话".into(),
            openai_compatible_api_key: None,
            openai_compatible_base_url: "https://api.openai.com/v1".into(),
            openai_compatible_model: "gpt-4o-mini".into(),
        };

        let err = match build_provider(&config) {
            Ok(_) => panic!("expected OpenAI-compatible without key to fail"),
            Err(err) => err,
        };

        assert!(matches!(err, AppError::Provider(_)));
        assert_eq!(
            err.message(),
            "OpenAI-compatible API key 未配置，请先到设置页填写"
        );
    }

    #[test]
    fn joins_chat_completions_endpoint() {
        assert_eq!(
            chat_completions_endpoint("https://api.deepseek.com/v1/"),
            "https://api.deepseek.com/v1/chat/completions"
        );
    }

    #[test]
    fn classifies_openai_compatible_http_errors() {
        assert!(matches!(
            classify_http_status(401, ""),
            AppError::Provider(_)
        ));
        assert!(matches!(
            classify_http_status(403, ""),
            AppError::Network(_)
        ));
        assert!(matches!(
            classify_http_status(429, ""),
            AppError::Network(_)
        ));
        assert!(matches!(
            classify_http_status(500, ""),
            AppError::Network(_)
        ));
    }
}
