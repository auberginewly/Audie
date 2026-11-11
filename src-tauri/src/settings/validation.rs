use super::*;
pub(super) fn validate_settings(settings: &Settings) -> Result<(), AppError> {
    // The trigger string's real gate is parse_trigger at register time (platform
    // layer); here we only reject an empty value, so the recorder can pick fn /
    // function keys / combos freely (SPEC §5.8 P3.9).
    if settings.hotkey.trim().is_empty() {
        return Err(AppError::Internal("trigger key must not be empty".into()));
    }
    if should_migrate_windows_fn(&settings.hotkey) {
        return Err(AppError::Internal(
            "Windows does not expose Fn as a reliable global hotkey; use Ctrl+Shift+Space".into(),
        ));
    }
    if should_migrate_windows_fn(&settings.compose_hotkey) {
        return Err(AppError::Internal(
            "Windows does not expose Fn as a reliable global hotkey".into(),
        ));
    }
    // 润色/改写键与写作键不能相同（前端 HotkeyRecorder 已实时拦，这里兜底防手改 toml）。
    let compose = settings.compose_hotkey.trim();
    if !compose.is_empty() && compose == settings.hotkey.trim() {
        return Err(AppError::Internal(
            "写作触发键不能和润色/改写触发键相同".into(),
        ));
    }
    // `doubao_stream` is a real, selectable ASR choice (the model picker writes it)
    // but it's streaming-only, not a batch provider, so it stays out of
    // `available_asr_providers` / `list_asr_providers`. Accept it explicitly here.
    if settings.asr_provider != "doubao_stream"
        && !available_asr_providers()
            .iter()
            .any(|provider| provider.id == settings.asr_provider)
    {
        return Err(AppError::Internal(format!(
            "unsupported ASR provider: {}",
            settings.asr_provider
        )));
    }
    if !available_llm_providers()
        .iter()
        .any(|provider| provider.id == settings.llm_provider)
    {
        return Err(AppError::Internal(format!(
            "unsupported LLM provider: {}",
            settings.llm_provider
        )));
    }
    if settings.enhance_prompt.trim().is_empty() {
        return Err(AppError::Internal("enhance prompt cannot be empty".into()));
    }
    if settings.openai_compatible_base_url.trim().is_empty() {
        return Err(AppError::Internal(
            "OpenAI-compatible base URL cannot be empty".into(),
        ));
    }
    // openai_compatible_model may be empty: picking a provider seeds no model (ids go
    // stale), the user fetches/types one. build_provider errors only when enhance runs.
    if settings.doubao_endpoint.trim().is_empty() {
        return Err(AppError::Internal("Doubao endpoint cannot be empty".into()));
    }
    if settings.doubao_resource_id.trim().is_empty() {
        return Err(AppError::Internal(
            "Doubao resource id cannot be empty".into(),
        ));
    }
    if settings.glm_endpoint.trim().is_empty() {
        return Err(AppError::Internal("GLM endpoint cannot be empty".into()));
    }
    if settings.aliyun_endpoint.trim().is_empty() {
        return Err(AppError::Internal("Aliyun endpoint cannot be empty".into()));
    }
    if settings.stepfun_endpoint.trim().is_empty() {
        return Err(AppError::Internal(
            "StepFun endpoint cannot be empty".into(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_secret_key_id(key_id: &str) -> Result<(), AppError> {
    if key_id.is_empty()
        || !key_id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(AppError::Internal(format!(
            "unsupported secret key id: {key_id}"
        )));
    }
    Ok(())
}
