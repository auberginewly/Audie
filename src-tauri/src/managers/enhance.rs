// EnhanceManager — P1.5 LLM polish orchestration. PROJECT_SPEC.md §4.3.
// Like TranscriptionManager, this is an adapter boundary: it calls the selected
// LLM provider and returns either polished text or an error for lib.rs to handle.

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

        // The caller owns fallback semantics. Here an LLM failure remains an
        // error; lib.rs converts it into "inject original + show warning".
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

pub(crate) fn fallback_after_enhance_failure(text: &str, _err: &AppError) -> EnhanceFallback {
    EnhanceFallback {
        text_to_inject: text.to_string(),
        message: "润色失败但已注入原文".into(),
    }
}

#[cfg(test)]
mod tests {
    use crate::error::AppError;

    #[test]
    fn enhance_failure_falls_back_to_original_text() {
        let original = "嗯那个今天我们开会讨论一下";
        let err = AppError::Network("LLM timeout".into());

        let outcome = super::fallback_after_enhance_failure(original, &err);

        assert_eq!(outcome.text_to_inject, original);
        assert_eq!(outcome.message, "润色失败但已注入原文");
    }
}
