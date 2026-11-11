use serde::{Deserialize, Serialize};
use std::collections::HashMap;
// The factory-default enhance prompt is data, not source: it lives in
// prompts/enhance_default.md and is pulled into Settings::default() via include_str!
// (no prompt string in .rs). Edit that file to change the default; the user owns the
// value after (settings.toml / 润色提示词 box). Sent + main language at enhance time.

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(default)] // missing TOML keys fall back to Default (hand-edited / old files)
pub struct Settings {
    pub hotkey: String,
    pub asr_provider: String,
    /// ASR model id within the chosen provider. Empty = use each adapter's built-in default const, so old TOML
    /// files and untouched installs keep working. Doubao ignores it (豆包 uses
    /// resource_id, not model — left blank for it, see normalize note).
    pub asr_model: String,
    pub llm_provider: String,
    /// 「AI 润色」总开关。默认 true（配了 LLM 即自动润色）；关掉 = 即使配了 key 也只插入
    /// 语音转写原文（纯转写），给只想要原始文字的人选择权。compose / rewrite 不受它管。
    pub enhance_enabled: bool,
    pub enhance_prompt: String,
    pub openai_compatible_base_url: String,
    pub openai_compatible_model: String,
    /// Keychain key id holding the active LLM provider's API key. All cloud LLM
    /// cards drive one backend slot (openai_compatible) but each stores its own
    /// key (deepseek_api_key / kimi_api_key / …), so switching providers doesn't
    /// reuse another's key. Empty = key-optional local provider (Ollama / LM
    /// Studio). Defaults to the legacy shared id so existing installs keep working
    /// until the user re-picks a provider.
    pub llm_api_key_id: String,
    pub doubao_endpoint: String,
    pub doubao_resource_id: String,
    pub glm_endpoint: String,
    pub aliyun_endpoint: String,
    pub stepfun_endpoint: String,
    /// Manually selected input device name (matches `cpal` device.name()). Empty
    /// string = automatic (P0.7 picks a reliable mic). Not `Option` so the patch
    /// can express "clear back to auto" via an empty string.
    pub input_device: String,
    /// Whether first-run onboarding has been completed (P3.12). Default false so a
    /// fresh install auto-opens the SetupWizard; set true when the user finishes it.
    pub onboarding_completed: bool,
    /// Whether a real dictation has succeeded at least once. This makes the
    /// onboarding "Try it" verification survive restarting the app.
    pub onboarding_test_completed: bool,
    /// User's main language; pipeline/config.rs prepends it as a line to the enhance prompt at
    /// send time. Empty string = follow the system locale (like `input_device`'s
    /// empty = automatic). Resolved at enhance time in the pipeline.
    pub primary_language: String,
    /// How long dictation history is kept on disk (History screen). One of
    /// `never | day | week | month | forever`; `never` skips recording entirely,
    /// the rest prune older rows. `normalize_settings` clamps anything else.
    pub history_retention: String,
    pub ui_language: String,
    pub show_in_dock: bool,
    /// 写作模式（compose）独立触发键。空串 = 未配置 = 写作不启用（配了键即启用）。文法同主 hotkey。
    pub compose_hotkey: String,
    /// 写作模式提示词出厂默认（数据文件，源码零 prompt，同 enhance_prompt 经 include_str! 读）。
    pub compose_prompt: String,
    /// 改写模式（rewrite）提示词出厂默认（数据文件）。改写按口述指令改写选中文字（逻辑见片2）。
    pub rewrite_prompt: String,
    /// Per-provider LLM model the user chose, keyed by the front-end card id
    /// (deepseek / lmstudio / …). All LLM cards share one backend slot, so this
    /// lets 选用 restore each provider's own model instead of clearing it. Backend
    /// treats it as an opaque string→string map. MUST stay the last field: a TOML
    /// table has to follow all scalar keys at the same level.
    #[serde(default)]
    pub llm_models: HashMap<String, String>,
}

#[derive(Deserialize)]
pub struct SettingsPatch {
    pub hotkey: Option<String>,
    pub asr_provider: Option<String>,
    pub asr_model: Option<String>,
    pub llm_provider: Option<String>,
    pub enhance_enabled: Option<bool>,
    pub enhance_prompt: Option<String>,
    pub openai_compatible_base_url: Option<String>,
    pub openai_compatible_model: Option<String>,
    pub llm_api_key_id: Option<String>,
    pub doubao_endpoint: Option<String>,
    pub doubao_resource_id: Option<String>,
    pub glm_endpoint: Option<String>,
    pub aliyun_endpoint: Option<String>,
    pub stepfun_endpoint: Option<String>,
    pub input_device: Option<String>,
    pub onboarding_completed: Option<bool>,
    pub onboarding_test_completed: Option<bool>,
    pub primary_language: Option<String>,
    pub history_retention: Option<String>,
    pub ui_language: Option<String>,
    pub show_in_dock: Option<bool>,
    pub compose_hotkey: Option<String>,
    pub compose_prompt: Option<String>,
    pub rewrite_prompt: Option<String>,
    pub llm_models: Option<HashMap<String, String>>,
}
