// 调用所选 LLM；pipeline 决定失败时如何兜底和发布反馈。

use crate::error::{AppError, AppResult};
use crate::llm::build_provider;

pub struct EnhanceManager;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EnhanceConfig {
    pub llm_provider: String,
    pub enhance_enabled: bool,
    pub enhance_prompt: String,
    pub openai_compatible_api_key: Option<String>,
    pub openai_compatible_base_url: String,
    pub openai_compatible_model: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnhanceFallback {
    pub text_to_inject: String,
    pub message: String,
}

impl EnhanceManager {
    pub fn new() -> Self {
        Self
    }

    pub fn enhance(&self, text: &str, config: &EnhanceConfig) -> AppResult<String> {
        if !config.enhance_enabled {
            return Ok(text.to_string());
        }

        // 失败仍返回错误，由 pipeline 保留原因并决定是否注入原文。
        let provider = build_provider(config)?;
        log::info!("enhancing transcript with {}", provider.name());
        provider.enhance(text, &config.enhance_prompt)
    }
}

impl Default for EnhanceManager {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn fallback_after_enhance_failure(text: &str, err: &AppError) -> EnhanceFallback {
    EnhanceFallback {
        text_to_inject: text.to_string(),
        message: err.message().to_string(),
    }
}

#[cfg(test)]
mod tests;
