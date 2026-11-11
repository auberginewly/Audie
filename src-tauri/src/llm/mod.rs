// LLM provider abstraction + OpenAI-compatible adapter. PROJECT_SPEC.md §4.1.

mod openai_compatible;
mod sanitize;

pub use openai_compatible::OpenAiCompatibleProvider;

use crate::error::{AppError, AppResult};
use crate::managers::enhance::EnhanceConfig;

pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &str;
    fn enhance(&self, text: &str, prompt: &str) -> AppResult<String>;
}

pub(crate) fn build_provider(config: &EnhanceConfig) -> AppResult<Box<dyn LlmProvider>> {
    match config.llm_provider.as_str() {
        "openai_compatible" => {
            let base_url = required_setting(
                &config.openai_compatible_base_url,
                "OpenAI-compatible base URL 未配置，请先到设置页填写",
            )?;
            // Local providers (Ollama / LM Studio on localhost) accept an empty key;
            // a missing cloud key is still a configuration error so the user gets a
            // clear "key 未配置" instead of a request-time 401.
            let api_key = if is_local_endpoint(&base_url) {
                config.openai_compatible_api_key.clone().unwrap_or_default()
            } else {
                required_key(
                    &config.openai_compatible_api_key,
                    "OpenAI-compatible API key 未配置，请先到设置页填写",
                )?
            };
            Ok(Box::new(OpenAiCompatibleProvider::new(
                api_key,
                base_url,
                required_setting(
                    &config.openai_compatible_model,
                    "OpenAI-compatible model 未配置，请先到设置页填写",
                )?,
            )))
        }
        _ => Err(AppError::Internal("unsupported LLM provider".into())),
    }
}

/// A localhost / 127.0.0.1 base URL means a local OpenAI-compatible server
/// (Ollama, LM Studio) where the API key is optional. Shared with provider_test
/// so the 测试 button doesn't demand a key for local endpoints either.
pub(crate) fn is_local_endpoint(base_url: &str) -> bool {
    let host = base_url
        .split("://")
        .nth(1)
        .unwrap_or(base_url)
        .split('/')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("");
    host == "localhost" || host == "127.0.0.1" || host == "[::1]"
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

#[cfg(test)]
mod tests;
