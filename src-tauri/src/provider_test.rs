use serde::{Deserialize, Serialize};

use crate::commands::{
    available_asr_providers, available_llm_providers, validate_secret_key_id, DEFAULT_ASR_PROVIDER,
    DEFAULT_LLM_PROVIDER,
};
use crate::error::{AppError, AppResult};

const GROQ_MODELS_ENDPOINT: &str = "https://api.groq.com/openai/v1/models";
const OPENAI_MODELS_ENDPOINT: &str = "https://api.openai.com/v1/models";
const OPENAI_COMPATIBLE_MODELS_PATH: &str = "/models";
const TEST_TIMEOUT_SECS: u64 = 8;

#[derive(Deserialize)]
pub struct ProviderTestRequest {
    pub kind: ProviderKind,
    pub provider_id: String,
    pub key_id: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Asr,
    Llm,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct ProviderTestResult {
    pub ok: bool,
    pub message: String,
}

#[tauri::command]
pub fn test_provider(request: ProviderTestRequest) -> Result<ProviderTestResult, AppError> {
    validate_provider_target(request.kind, &request.provider_id)?;
    validate_secret_key_id(&request.key_id)?;

    // Local endpoints (Ollama / LM Studio on localhost) need no key — don't demand one.
    let local = request
        .base_url
        .as_deref()
        .is_some_and(crate::llm::is_local_endpoint);
    let api_key = read_api_key_for_test(request.api_key.as_deref(), local)?;

    let endpoint = provider_test_endpoint(request.kind, &request.provider_id, request.base_url)?;
    test_models_endpoint(&endpoint, &api_key)?;

    Ok(ProviderTestResult {
        ok: true,
        message: "连接测试通过".into(),
    })
}

/// Models response from an OpenAI-compatible `/models` endpoint.
#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
}

/// Fetch the live model id list from an OpenAI-compatible provider's `/models`.
/// Every LLM card is OpenAI-compatible, so this works for all cloud cards and for
/// local servers (Ollama / LM Studio return exactly the pulled models, no key).
/// Driven by the 刷新 button in the model config dialog; the curated static list is
/// the offline fallback when this isn't run / fails.
#[tauri::command]
pub fn list_provider_models(
    base_url: String,
    api_key: Option<String>,
) -> Result<Vec<String>, AppError> {
    let base = base_url.trim();
    if base.is_empty() {
        return Err(AppError::Provider("请先填写 base URL".into()));
    }
    let endpoint = join_url(base, OPENAI_COMPATIBLE_MODELS_PATH);
    fetch_chat_model_ids(&endpoint, api_key.as_deref())
}

fn fetch_chat_model_ids(endpoint: &str, api_key: Option<&str>) -> AppResult<Vec<String>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(TEST_TIMEOUT_SECS))
        .build()
        .map_err(|err| AppError::Internal(format!("build model list client: {err}")))?;

    let mut request = client.get(endpoint);
    // Local servers need no key; cloud providers do (401 surfaces as Provider).
    if let Some(key) = api_key.map(str::trim).filter(|value| !value.is_empty()) {
        request = request.bearer_auth(key);
    }
    let resp = request.send().map_err(classify_reqwest_error)?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().unwrap_or_default();
        return Err(classify_http_status(status.as_u16(), &body));
    }

    let parsed: ModelsResponse = resp
        .json()
        .map_err(|err| AppError::Provider(format!("解析模型列表失败：{err}")))?;

    let mut ids: Vec<String> = parsed
        .data
        .into_iter()
        .map(|entry| entry.id)
        .filter(|id| is_chat_model(id))
        .collect();
    ids.sort();
    ids.dedup();
    Ok(ids)
}

/// Heuristic chat-model filter: the standard /models response has no capability
/// field, so we strip the obvious non-chat ids (embeddings / speech / image),
/// which OpenAI in particular mixes into one list. Imperfect but removes the noise.
fn is_chat_model(id: &str) -> bool {
    let lower = id.to_ascii_lowercase();
    const NON_CHAT: &[&str] = &[
        "embed",
        "tts",
        "whisper",
        "transcribe",
        "dall-e",
        "dalle",
        "rerank",
        "moderation",
        "image",
        "stable-diffusion",
        "flux",
        "speech",
        "realtime",
    ];
    !NON_CHAT.iter().any(|needle| lower.contains(needle))
}

fn read_api_key_for_test(inline_api_key: Option<&str>, local: bool) -> AppResult<String> {
    match inline_api_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(api_key) => Ok(api_key.to_string()),
        // Local providers run keyless against localhost; cloud must supply a key.
        None if local => Ok(String::new()),
        None => Err(AppError::Provider("请先填写 API key".into())),
    }
}

fn validate_provider_target(kind: ProviderKind, provider_id: &str) -> AppResult<()> {
    let providers = match kind {
        ProviderKind::Asr => available_asr_providers(),
        ProviderKind::Llm => available_llm_providers(),
    };

    if providers.iter().any(|provider| provider.id == provider_id) {
        Ok(())
    } else {
        Err(AppError::Internal(format!(
            "unsupported provider test target: {provider_id}"
        )))
    }
}

fn provider_test_endpoint(
    kind: ProviderKind,
    provider_id: &str,
    base_url: Option<String>,
) -> AppResult<String> {
    match (kind, provider_id) {
        (ProviderKind::Asr, DEFAULT_ASR_PROVIDER) => Ok(GROQ_MODELS_ENDPOINT.into()),
        (ProviderKind::Asr, "openai") => Ok(OPENAI_MODELS_ENDPOINT.into()),
        (ProviderKind::Llm, DEFAULT_LLM_PROVIDER) => {
            let base = base_url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| AppError::Provider("请填写 OpenAI-compatible base URL".into()))?;
            Ok(join_url(base, OPENAI_COMPATIBLE_MODELS_PATH))
        }
        (ProviderKind::Asr, "whisper_cpp") => Err(AppError::Device(
            "Whisper.cpp 本地模型测试放到 P1.4 ASR provider 切换".into(),
        )),
        _ => Err(AppError::Internal(format!(
            "unsupported provider test target: {provider_id}"
        ))),
    }
}

fn join_url(base: &str, path: &str) -> String {
    format!("{}{}", base.trim_end_matches('/'), path)
}

fn test_models_endpoint(endpoint: &str, api_key: &str) -> AppResult<()> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(TEST_TIMEOUT_SECS))
        .build()
        .map_err(|err| AppError::Internal(format!("build provider test client: {err}")))?;

    let mut request = client.get(endpoint);
    // No bearer for keyless local endpoints (empty key).
    if !api_key.is_empty() {
        request = request.bearer_auth(api_key);
    }
    let resp = request.send().map_err(classify_reqwest_error)?;

    let status = resp.status();
    if status.is_success() {
        Ok(())
    } else {
        let body = resp.text().unwrap_or_default();
        Err(classify_http_status(status.as_u16(), &body))
    }
}

fn classify_reqwest_error(e: reqwest::Error) -> AppError {
    if e.is_timeout() {
        AppError::Network("网络失败：provider 测试请求超时，请检查网络或代理".into())
    } else if e.is_connect() {
        AppError::Network("网络失败：无法连接 provider，请检查网络或代理".into())
    } else {
        AppError::Network(format!("网络失败：provider 测试请求失败：{e}"))
    }
}

fn classify_http_status(status: u16, body: &str) -> AppError {
    match status {
        401 => AppError::Provider("API key 无效，请检查设置".into()),
        403 => AppError::Network("provider 拒绝请求，可能是网络、代理或地区限制".into()),
        429 => AppError::Network("provider 请求过于频繁或额度受限，请稍后重试".into()),
        500..=599 => AppError::Network(format!("provider 服务端异常（{status}）")),
        _ => {
            let snippet: String = body.chars().take(200).collect();
            AppError::Provider(format!("provider 拒绝请求（{status}）：{snippet}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_chat_model_keeps_chat_drops_non_chat() {
        // Chat models kept.
        assert!(is_chat_model("gpt-5.2"));
        assert!(is_chat_model("deepseek-chat"));
        assert!(is_chat_model("glm-4.6"));
        assert!(is_chat_model("qwen-plus-latest"));
        // Non-chat noise dropped (OpenAI mixes these into /models).
        assert!(!is_chat_model("text-embedding-3-large"));
        assert!(!is_chat_model("whisper-1"));
        assert!(!is_chat_model("gpt-4o-transcribe"));
        assert!(!is_chat_model("tts-1"));
        assert!(!is_chat_model("dall-e-3"));
    }

    #[test]
    fn openai_compatible_endpoint_joins_base_url_and_models_path() {
        let endpoint = provider_test_endpoint(
            ProviderKind::Llm,
            "openai_compatible",
            Some("https://api.deepseek.com/v1/".into()),
        )
        .unwrap();

        assert_eq!(endpoint, "https://api.deepseek.com/v1/models");
    }

    #[test]
    fn openai_compatible_requires_base_url() {
        let err = provider_test_endpoint(ProviderKind::Llm, "openai_compatible", Some(" ".into()))
            .unwrap_err();

        assert!(matches!(err, AppError::Provider(_)));
        assert_eq!(err.message(), "请填写 OpenAI-compatible base URL");
    }

    #[test]
    fn inline_api_key_is_trimmed() {
        assert_eq!(
            read_api_key_for_test(Some("  inline-key  "), false).unwrap(),
            "inline-key"
        );
    }

    #[test]
    fn cloud_requires_a_key() {
        // Blank or missing key for a non-local provider is an error.
        assert_eq!(
            read_api_key_for_test(Some("  "), false)
                .unwrap_err()
                .message(),
            "请先填写 API key"
        );
        assert_eq!(
            read_api_key_for_test(None, false).unwrap_err().message(),
            "请先填写 API key"
        );
    }

    #[test]
    fn local_endpoint_needs_no_key() {
        // Ollama / LM Studio (localhost) test keyless.
        assert_eq!(read_api_key_for_test(None, true).unwrap(), "");
    }

    #[test]
    fn http_401_maps_to_provider_error() {
        let err = classify_http_status(401, "");

        assert!(matches!(err, AppError::Provider(_)));
        assert!(!err.recoverable());
    }

    #[test]
    fn http_403_maps_to_network_error() {
        let err = classify_http_status(403, "");

        assert!(matches!(err, AppError::Network(_)));
        assert!(err.recoverable());
    }
}
