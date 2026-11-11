use super::*;
/// Clamp loaded settings to valid values — the validation the per-field store readers
/// used to do, applied once after deserialize. Unknown asr/llm providers reset to
/// default (but `doubao_stream` stays — streaming-only, absent from the batch list);
/// legacy doubao resource id migrates; empty required strings fall back to defaults;
/// a blank whisper model path becomes None.
pub(super) fn normalize_settings(mut settings: Settings) -> Settings {
    if settings.asr_provider != "doubao_stream"
        && !available_asr_providers()
            .iter()
            .any(|provider| provider.id == settings.asr_provider)
    {
        settings.asr_provider = DEFAULT_ASR_PROVIDER.to_string();
    }
    if !available_llm_providers()
        .iter()
        .any(|provider| provider.id == settings.llm_provider)
    {
        settings.llm_provider = DEFAULT_LLM_PROVIDER.to_string();
    }
    settings.doubao_resource_id = normalize_doubao_resource_id(settings.doubao_resource_id);
    // asr_model is free-form (each adapter validates / defaults at request time);
    // empty = built-in default. Only trim hand-edited whitespace, never reset.
    settings.asr_model = settings.asr_model.trim().to_string();

    if settings.hotkey.trim().is_empty() || should_migrate_windows_fn(&settings.hotkey) {
        settings.hotkey = DEFAULT_HOTKEY.to_string();
    }
    if settings.enhance_prompt.trim().is_empty() {
        settings.enhance_prompt = Settings::default().enhance_prompt;
    }
    if settings.openai_compatible_base_url.trim().is_empty() {
        settings.openai_compatible_base_url = DEFAULT_OPENAI_COMPATIBLE_BASE_URL.to_string();
    }
    // No empty→default reset for the model: an empty model is a meaningful "not yet
    // chosen" state (picking a provider no longer seeds a hardcoded/stale model id —
    // the user fetches the live list or types one). build_provider surfaces a clear
    // "model 未配置" error when enhance runs without one.
    settings.openai_compatible_model = settings.openai_compatible_model.trim().to_string();
    if settings.doubao_endpoint.trim().is_empty() {
        settings.doubao_endpoint = crate::asr::doubao::config::DEFAULT_ENDPOINT.to_string();
    }
    if settings.doubao_resource_id.trim().is_empty() {
        settings.doubao_resource_id = crate::asr::doubao::config::DEFAULT_RESOURCE_ID.to_string();
    }
    if settings.glm_endpoint.trim().is_empty() {
        settings.glm_endpoint = crate::asr::glm::ENDPOINT.to_string();
    }
    if settings.aliyun_endpoint.trim().is_empty() {
        settings.aliyun_endpoint = crate::asr::aliyun::config::DEFAULT_ENDPOINT.to_string();
    }
    if settings.stepfun_endpoint.trim().is_empty() {
        settings.stepfun_endpoint = crate::asr::stepfun::config::ENDPOINT.to_string();
    }

    if !HISTORY_RETENTION_IDS.contains(&settings.history_retention.as_str()) {
        settings.history_retention = DEFAULT_HISTORY_RETENTION.to_string();
    }
    if !UI_LANGUAGE_IDS.contains(&settings.ui_language.as_str()) {
        settings.ui_language = DEFAULT_UI_LANGUAGE.to_string();
    }
    if settings.compose_prompt.trim().is_empty() {
        settings.compose_prompt = Settings::default().compose_prompt;
    }
    if settings.rewrite_prompt.trim().is_empty() {
        settings.rewrite_prompt = Settings::default().rewrite_prompt;
    }

    settings
}

pub(super) fn normalize_doubao_resource_id(stored: String) -> String {
    if stored == crate::asr::doubao::config::LEGACY_RESOURCE_ID {
        crate::asr::doubao::config::DEFAULT_RESOURCE_ID.to_string()
    } else {
        stored
    }
}
