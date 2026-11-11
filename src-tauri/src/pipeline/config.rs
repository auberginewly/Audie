use super::context::DictationMode;
use crate::asr::doubao::config as doubao_config;
use crate::managers::enhance::EnhanceConfig;
use crate::managers::transcription::TranscriptionConfig;
use crate::platform::Platform;
use crate::settings;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

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
    // 只读当前 ASR 的钥匙串条目，切换 provider 不应触碰无关密钥。
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
    // 选中豆包才启用流式；仅有已保存 token 不能抢占用户选择的 ASR。
    if asr_provider != "doubao_stream" {
        return None;
    }
    let api_key_or_access_token = read_secret(doubao_config::SECRET_API_KEY_OR_ACCESS_TOKEN)?;
    let app_id = read_secret(doubao_config::SECRET_APP_ID);

    Some(TranscriptionConfig {
        asr_provider: "doubao_stream".into(),
        // 豆包通过 resource_id 选择服务，不使用通用的 asr_model 字段。
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

/// 把主要语言前置到提示词，用户不必在编辑器里维护占位符。
/// 未选择时跟随系统，读不到系统语言才回退中文。
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

/// 历史记录的重试属于显式操作，不受自动润色开关限制；缺 key 要显示失败。
pub(crate) fn reenhance_config(app: &AppHandle) -> EnhanceConfig {
    let settings = settings::load_settings(app);
    let enhance_prompt = prepend_language(app, &settings, &settings.enhance_prompt);
    let platform = app.state::<Arc<dyn Platform>>();

    enhance_config_from_settings(
        settings.llm_provider,
        true,
        // 显式重试已由 force_enabled 决定，不受润色开关影响。
        true,
        enhance_prompt,
        settings.openai_compatible_base_url,
        settings.openai_compatible_model,
        settings.llm_api_key_id,
        |key_id| read_optional_secret(platform.inner().as_ref(), key_id),
    )
}

// 参数与设置字段一一对应，并允许测试注入密钥读取函数；
// 单为减少参数再包装一层只会增加映射，保留这个局部例外。
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
    // 自动润色关闭时不访问钥匙串；写作、改写、历史重试则按显式意图读取。
    let want_enhance = force_enabled || polish_toggle;

    // 本地 Ollama / LM Studio 允许免 key，空 id 不触发钥匙串读取。
    let openai_compatible_api_key =
        if want_enhance && llm_provider == "openai_compatible" && !llm_api_key_id.is_empty() {
            read_secret(&llm_api_key_id)
        } else {
            None
        };

    // 显式调用即使缺配置也执行并反馈失败；自动润色仅在开关开启且配置可用时执行。
    // 关闭开关就保留纯转写，不应因已有 key 强制润色。
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
