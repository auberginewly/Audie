use std::sync::Arc;

use tauri::{AppHandle, Manager};

use super::{
    classify_http_status, classify_reqwest_error, join_url, resolve_api_key_for_provider_action,
    ProviderKind, ProviderTestRequest, ProviderTestResult, OPENAI_COMPATIBLE_MODELS_PATH,
    TEST_TIMEOUT_SECS,
};
use crate::error::{AppError, AppResult};
use crate::platform::Platform;
use crate::settings::{
    available_asr_providers, available_llm_providers, validate_secret_key_id, DEFAULT_LLM_PROVIDER,
};

const OPENAI_MODELS_ENDPOINT: &str = "https://api.openai.com/v1/models";

#[tauri::command]
pub fn test_provider(
    app: AppHandle,
    request: ProviderTestRequest,
) -> Result<ProviderTestResult, AppError> {
    validate_provider_target(request.kind, &request.provider_id)?;
    validate_secret_key_id(&request.key_id)?;

    // Local endpoints (Ollama / LM Studio on localhost) need no key — don't demand one.
    let local = request
        .base_url
        .as_deref()
        .is_some_and(crate::llm::is_local_endpoint);
    let platform = app.state::<Arc<dyn Platform>>();
    let api_key = resolve_api_key_for_provider_action(
        request.api_key.as_deref(),
        Some(&request.key_id),
        local,
        |key_id| platform.has_secret(key_id),
        |key_id| platform.read_secret(key_id),
    )?;

    let endpoint = provider_test_endpoint(request.kind, &request.provider_id, request.base_url)?;
    test_models_endpoint(&endpoint, &api_key)?;

    Ok(ProviderTestResult {
        ok: true,
        message: "连接测试通过".into(),
    })
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
        (ProviderKind::Asr, "openai") => Ok(OPENAI_MODELS_ENDPOINT.into()),
        (ProviderKind::Llm, DEFAULT_LLM_PROVIDER) => {
            let base = base_url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| AppError::Provider("请填写 OpenAI-compatible base URL".into()))?;
            Ok(join_url(base, OPENAI_COMPATIBLE_MODELS_PATH))
        }
        _ => Err(AppError::Internal(format!(
            "unsupported provider test target: {provider_id}"
        ))),
    }
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

#[cfg(test)]
mod tests;
