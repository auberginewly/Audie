use super::*;
pub(crate) fn settings_from_patch(
    current: Settings,
    patch: SettingsPatch,
) -> Result<Settings, AppError> {
    let next = Settings {
        hotkey: patch.hotkey.unwrap_or(current.hotkey),
        asr_provider: patch.asr_provider.unwrap_or(current.asr_provider),
        // Empty is meaningful (= adapter built-in default), so like input_device we
        // keep an empty patch value rather than filtering it back to current.
        asr_model: patch
            .asr_model
            .map(|value| value.trim().to_string())
            .unwrap_or(current.asr_model),
        llm_provider: patch.llm_provider.unwrap_or(current.llm_provider),
        enhance_enabled: patch.enhance_enabled.unwrap_or(current.enhance_enabled),
        enhance_prompt: patch.enhance_prompt.unwrap_or(current.enhance_prompt),
        openai_compatible_base_url: patch
            .openai_compatible_base_url
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or(current.openai_compatible_base_url),
        // Empty is meaningful (= no model chosen yet; provider pick no longer seeds
        // a hardcoded one), so keep an explicit empty patch value rather than
        // filtering it back to current.
        openai_compatible_model: patch
            .openai_compatible_model
            .map(|value| value.trim().to_string())
            .unwrap_or(current.openai_compatible_model),
        // Empty is meaningful (= key-optional local provider), so keep an empty
        // patch value rather than filtering it back to current (like input_device).
        llm_api_key_id: patch
            .llm_api_key_id
            .map(|value| value.trim().to_string())
            .unwrap_or(current.llm_api_key_id),
        doubao_endpoint: patch
            .doubao_endpoint
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or(current.doubao_endpoint),
        doubao_resource_id: patch
            .doubao_resource_id
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or(current.doubao_resource_id),
        glm_endpoint: patch
            .glm_endpoint
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or(current.glm_endpoint),
        aliyun_endpoint: patch
            .aliyun_endpoint
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or(current.aliyun_endpoint),
        stepfun_endpoint: patch
            .stepfun_endpoint
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or(current.stepfun_endpoint),
        // Empty is meaningful here (= automatic), so unlike the others we keep an
        // empty patch value instead of filtering it back to the current value.
        input_device: patch
            .input_device
            .map(|value| value.trim().to_string())
            .unwrap_or(current.input_device),
        onboarding_completed: patch
            .onboarding_completed
            .unwrap_or(current.onboarding_completed),
        onboarding_test_completed: patch
            .onboarding_test_completed
            .unwrap_or(current.onboarding_test_completed),
        // Empty is meaningful (= follow system locale), so like input_device we keep
        // an empty patch value rather than filtering it back to current.
        primary_language: patch.primary_language.unwrap_or(current.primary_language),
        // Retention is an enum-ish id (never/day/week/month/forever) — a blank patch
        // keeps current; normalize_settings clamps anything unknown.
        history_retention: patch
            .history_retention
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or(current.history_retention),
        ui_language: patch
            .ui_language
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or(current.ui_language),
        show_in_dock: patch.show_in_dock.unwrap_or(current.show_in_dock),
        // 空串有意义（= 写作键未配置），故保留空 patch 值，不 filter 回 current（同 input_device）。
        compose_hotkey: patch
            .compose_hotkey
            .map(|value| value.trim().to_string())
            .unwrap_or(current.compose_hotkey),
        compose_prompt: patch.compose_prompt.unwrap_or(current.compose_prompt),
        rewrite_prompt: patch.rewrite_prompt.unwrap_or(current.rewrite_prompt),
        // Whole-map replace when provided (the front-end sends the merged map).
        llm_models: patch.llm_models.unwrap_or(current.llm_models),
    };

    validate_settings(&next)?;
    Ok(next)
}
