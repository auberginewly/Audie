use super::*;
pub(super) fn transcription_config(app: &AppHandle) -> TranscriptionConfig {
    let settings = settings::load_settings(app);
    let platform = app.state::<Arc<dyn Platform>>();

    transcription_config_from_settings(&settings, |key_id| {
        read_optional_secret(platform.inner().as_ref(), key_id)
    })
}

pub(super) fn transcription_config_from_settings(
    settings: &settings::Settings,
    mut read_secret: impl FnMut(&str) -> Option<String>,
) -> TranscriptionConfig {
    let asr_provider = settings.asr_provider.clone();
    let openai_api_key = (asr_provider == "openai")
        .then(|| read_secret("openai_api_key"))
        .flatten();
    let doubao_api_key_or_access_token = (asr_provider == "doubao_stream")
        .then(|| read_secret(doubao_config::SECRET_API_KEY_OR_ACCESS_TOKEN))
        .flatten();
    let doubao_app_id = (asr_provider == "doubao_stream")
        .then(|| read_secret(doubao_config::SECRET_APP_ID))
        .flatten();
    // Each new cloud ASR reads only its own keychain key (and only when selected),
    // so picking another provider never touches an unrelated secret.
    let glm_api_key = (asr_provider == "glm")
        .then(|| read_secret(crate::asr::glm::SECRET_API_KEY))
        .flatten();
    let aliyun_api_key = (asr_provider == "aliyun_fun")
        .then(|| read_secret(crate::asr::aliyun::config::SECRET_API_KEY))
        .flatten();
    let stepfun_api_key = (asr_provider == "stepfun")
        .then(|| read_secret(crate::asr::stepfun::config::SECRET_API_KEY))
        .flatten();

    TranscriptionConfig {
        asr_provider,
        asr_model: settings.asr_model.clone(),
        openai_api_key,
        doubao_endpoint: Some(settings.doubao_endpoint.clone()),
        doubao_resource_id: Some(settings.doubao_resource_id.clone()),
        doubao_app_id,
        doubao_api_key_or_access_token,
        glm_endpoint: Some(settings.glm_endpoint.clone()),
        glm_api_key,
        aliyun_endpoint: Some(settings.aliyun_endpoint.clone()),
        aliyun_api_key,
        stepfun_endpoint: Some(settings.stepfun_endpoint.clone()),
        stepfun_api_key,
    }
}

pub(super) fn doubao_streaming_config(app: &AppHandle) -> Option<TranscriptionConfig> {
    let settings = settings::load_settings(app);
    let platform = app.state::<Arc<dyn Platform>>();
    doubao_streaming_config_from_settings(
        &settings.asr_provider,
        settings.doubao_endpoint,
        settings.doubao_resource_id,
        |key_id| read_optional_secret(platform.inner().as_ref(), key_id),
    )
}

pub(super) fn doubao_streaming_config_from_settings(
    asr_provider: &str,
    endpoint: String,
    resource_id: String,
    mut read_secret: impl FnMut(&str) -> Option<String>,
) -> Option<TranscriptionConfig> {
    // Doubao streaming is opt-in: it only kicks in when the user explicitly picks
    // doubao in the model selector (asr_provider == "doubao_stream"). A saved token
    // alone must NOT hijack every recording — the explicit provider choice wins.
    if asr_provider != "doubao_stream" {
        return None;
    }
    let api_key_or_access_token = read_secret(doubao_config::SECRET_API_KEY_OR_ACCESS_TOKEN)?;
    let app_id = read_secret(doubao_config::SECRET_APP_ID);

    Some(TranscriptionConfig {
        asr_provider: "doubao_stream".into(),
        // Doubao uses resource_id, not model; asr_model stays blank for it (left
        // for a future provider-specific variant mapping if豆包 ever needs one).
        asr_model: String::new(),
        openai_api_key: None,
        doubao_endpoint: Some(endpoint),
        doubao_resource_id: Some(resource_id),
        doubao_app_id: app_id,
        doubao_api_key_or_access_token: Some(api_key_or_access_token),
        glm_endpoint: None,
        glm_api_key: None,
        aliyun_endpoint: None,
        aliyun_api_key: None,
        stepfun_endpoint: None,
        stepfun_api_key: None,
    })
}

/// Prepend the main language as one line so the LLM knows it, without exposing a
/// {{placeholder}} in the editable prompt. The dropdown wins; empty = follow the
/// system locale; 中文 as a last resort. Shared by 润色 and 写作.
pub(super) fn prepend_language(
    app: &AppHandle,
    settings: &settings::Settings,
    prompt: &str,
) -> String {
    let language = if settings.primary_language.trim().is_empty() {
        app.state::<Arc<dyn Platform>>()
            .inner()
            .system_language()
            .unwrap_or_else(|| "中文".to_string())
    } else {
        settings.primary_language.clone()
    };
    format!("用户主要语言：{language}\n\n{prompt}")
}

pub(super) fn prompt_from_snapshot(
    snapshot: Option<String>,
    settings: &settings::Settings,
    mode: DictationMode,
) -> String {
    snapshot.unwrap_or_else(|| match mode {
        DictationMode::Polish => settings.enhance_prompt.clone(),
        DictationMode::Rewrite => settings.rewrite_prompt.clone(),
        DictationMode::Compose => settings.compose_prompt.clone(),
    })
}

pub(super) fn enhance_config(
    app: &AppHandle,
    mode: DictationMode,
    snapshot: Option<String>,
) -> EnhanceConfig {
    let settings = settings::load_settings(app);
    let prompt = prompt_from_snapshot(snapshot, &settings, mode);
    let enhance_prompt = prepend_language(app, &settings, &prompt);
    let platform = app.state::<Arc<dyn Platform>>();
    enhance_config_from_settings(
        settings.llm_provider,
        mode != DictationMode::Polish,
        settings.enhance_enabled,
        enhance_prompt,
        settings.openai_compatible_base_url,
        settings.openai_compatible_model,
        settings.llm_api_key_id,
        |key_id| read_optional_secret(platform.inner().as_ref(), key_id),
    )
}

/// Enhance config for the History 重试 — same prompt/provider. `force_enabled` is true
/// (the user explicitly asked to polish this entry), so it runs even when 润色 wouldn't
/// auto-trigger; a missing key then surfaces as a failure rather than silence.
pub(crate) fn reenhance_config(app: &AppHandle) -> EnhanceConfig {
    let settings = settings::load_settings(app);
    let enhance_prompt = prepend_language(app, &settings, &settings.enhance_prompt);
    let platform = app.state::<Arc<dyn Platform>>();

    enhance_config_from_settings(
        settings.llm_provider,
        true,
        // 显式触发：force 已 short-circuit，polish_toggle 不参与（不看「AI 润色」开关）。
        true,
        enhance_prompt,
        settings.openai_compatible_base_url,
        settings.openai_compatible_model,
        settings.llm_api_key_id,
        |key_id| read_optional_secret(platform.inner().as_ref(), key_id),
    )
}

// 8 args = a flat 1:1 map of the settings fields driving enhance, plus a read_secret
// injection point for tests. Grouping into a struct would shuffle the same fields for
// no clarity gain on a pure mapper, so allow the extra arg here.
#[allow(clippy::too_many_arguments)]
pub(super) fn enhance_config_from_settings(
    llm_provider: String,
    force_enabled: bool,
    polish_toggle: bool,
    enhance_prompt: String,
    openai_compatible_base_url: String,
    openai_compatible_model: String,
    llm_api_key_id: String,
    mut read_secret: impl FnMut(&str) -> Option<String>,
) -> EnhanceConfig {
    // Skip the keychain read entirely when 润色 can't run anyway (toggle off and not
    // forced) — only touch the secret store when the key is actually needed (CLAUDE.md
    // keychain note). force_enabled (compose / rewrite / History 重试) always wants it.
    let want_enhance = force_enabled || polish_toggle;

    // Read the active provider's own key (4b). Empty id = key-optional local
    // provider (Ollama / LM Studio) → no key; build_provider allows that for
    // localhost endpoints.
    let openai_compatible_api_key =
        if want_enhance && llm_provider == "openai_compatible" && !llm_api_key_id.is_empty() {
            read_secret(&llm_api_key_id)
        } else {
            None
        };

    // 润色 enablement: explicit triggers (force_enabled — compose / rewrite / History
    // 重试) always run, so a missing key surfaces as a failure instead of silence.
    // Otherwise 润色 runs only when the user's 「AI 润色」 toggle is on AND the LLM is
    // configured (a key present, or a local endpoint that needs none). Toggle off =
    // 纯转写 even when a key exists (Disabled, no 去设置 toast) — 给想要原文的人选择权.
    let enhance_enabled = force_enabled
        || (polish_toggle
            && (openai_compatible_api_key.is_some()
                || crate::llm::is_local_endpoint(&openai_compatible_base_url)));

    EnhanceConfig {
        llm_provider,
        enhance_enabled,
        enhance_prompt,
        openai_compatible_api_key,
        openai_compatible_base_url,
        openai_compatible_model,
    }
}

pub(super) fn read_optional_secret(platform: &dyn Platform, key_id: &str) -> Option<String> {
    platform
        .read_secret(key_id)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
