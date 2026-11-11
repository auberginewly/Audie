use super::Settings;
pub use crate::platform::DEFAULT_HOTKEY;
use std::collections::HashMap;
pub const DEFAULT_ASR_PROVIDER: &str = "doubao_stream";

pub const DEFAULT_LLM_PROVIDER: &str = "openai_compatible";

pub const DEFAULT_OPENAI_COMPATIBLE_BASE_URL: &str = "https://api.openai.com/v1";

pub const DEFAULT_OPENAI_COMPATIBLE_MODEL: &str = "gpt-4o-mini";

// Legacy shared LLM key id — the default so pre-4b installs keep reading their key
// until the user re-picks a provider (which writes a per-provider key id).
pub const DEFAULT_LLM_API_KEY_ID: &str = "openai_compatible_api_key";

pub const DEFAULT_HISTORY_RETENTION: &str = "forever";

pub const DEFAULT_UI_LANGUAGE: &str = "zh-Hans";

pub(super) const HISTORY_RETENTION_IDS: &[&str] = &["never", "day", "week", "month", "forever"];

pub(super) const UI_LANGUAGE_IDS: &[&str] = &["zh-Hans", "zh-Hant", "en"];

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: DEFAULT_HOTKEY.to_string(),
            asr_provider: DEFAULT_ASR_PROVIDER.to_string(),
            asr_model: String::new(),
            llm_provider: DEFAULT_LLM_PROVIDER.to_string(),
            // 默认开：配了 LLM 就自动润色（沿用 f87b551 后的行为）；用户可关掉只要纯转写。
            enhance_enabled: true,
            enhance_prompt: include_str!("../../prompts/enhance_default.md")
                .trim_end()
                .to_string(),
            openai_compatible_base_url: DEFAULT_OPENAI_COMPATIBLE_BASE_URL.to_string(),
            openai_compatible_model: DEFAULT_OPENAI_COMPATIBLE_MODEL.to_string(),
            llm_api_key_id: DEFAULT_LLM_API_KEY_ID.to_string(),
            doubao_endpoint: crate::asr::doubao::config::DEFAULT_ENDPOINT.to_string(),
            doubao_resource_id: crate::asr::doubao::config::DEFAULT_RESOURCE_ID.to_string(),
            glm_endpoint: crate::asr::glm::ENDPOINT.to_string(),
            aliyun_endpoint: crate::asr::aliyun::config::DEFAULT_ENDPOINT.to_string(),
            stepfun_endpoint: crate::asr::stepfun::config::ENDPOINT.to_string(),
            input_device: String::new(),
            onboarding_completed: false,
            onboarding_test_completed: false,
            primary_language: String::new(),
            history_retention: DEFAULT_HISTORY_RETENTION.to_string(),
            ui_language: DEFAULT_UI_LANGUAGE.to_string(),
            show_in_dock: true,
            compose_hotkey: String::new(),
            compose_prompt: include_str!("../../prompts/compose_default.md")
                .trim_end()
                .to_string(),
            rewrite_prompt: include_str!("../../prompts/rewrite_default.md")
                .trim_end()
                .to_string(),
            llm_models: HashMap::new(),
        }
    }
}
