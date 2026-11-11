use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::{
    classify_http_status, classify_reqwest_error, join_url, resolve_api_key_for_provider_action,
    OPENAI_COMPATIBLE_MODELS_PATH, TEST_TIMEOUT_SECS,
};
use crate::error::{AppError, AppResult};
use crate::platform::Platform;

/// Short per-endpoint timeout for the zero-click local-LLM probe — three sequential
/// probes must stay snappy on model-section mount, so a down server fails fast
/// instead of stalling the list.
const DISCOVER_TIMEOUT_MS: u64 = 600;

/// Known local-LLM servers to auto-probe, paired with the picker card id they map
/// to (matches models.ts llmPresetForModelId). llama.cpp / llamafile has no card —
/// it surfaces under the generic "llamacpp" id so its live models still list.
const LOCAL_LLM_TARGETS: &[(&str, &str)] = &[
    ("ollama", "http://localhost:11434/v1"),
    ("lmstudio", "http://localhost:1234/v1"),
    ("llamacpp", "http://localhost:8080/v1"),
];

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
    app: AppHandle,
    base_url: String,
    api_key: Option<String>,
    key_id: Option<String>,
) -> Result<Vec<String>, AppError> {
    let base = base_url.trim();
    if base.is_empty() {
        return Err(AppError::Provider("请先填写 base URL".into()));
    }
    let local = crate::llm::is_local_endpoint(base);
    let platform = app.state::<Arc<dyn Platform>>();
    let api_key = resolve_api_key_for_provider_action(
        api_key.as_deref(),
        key_id.as_deref(),
        local,
        |key_id| platform.has_secret(key_id),
        |key_id| platform.read_secret(key_id),
    )?;
    let endpoint = join_url(base, OPENAI_COMPATIBLE_MODELS_PATH);
    fetch_chat_model_ids(
        &endpoint,
        (!api_key.is_empty()).then_some(api_key.as_str()),
        std::time::Duration::from_secs(TEST_TIMEOUT_SECS),
    )
}

/// One auto-detected local-LLM server. Hand-written Zod mirror in
/// src/features/providers/schema.ts (DiscoveredLocalLlmSchema) — keep field names in sync
/// (Audie hand-writes Zod, no specta).
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct DiscoveredLocalLlm {
    /// Picker card id this maps to (ollama / lmstudio / llamacpp).
    pub provider: String,
    pub base_url: String,
    /// Live chat models the server reports (empty when alive but none pulled).
    pub models: Vec<String>,
}

/// Zero-click local-LLM auto-detect (A2): probe the known local servers and return
/// only the ones that answered with at least one chat model. Invoked automatically
/// when the model section mounts — NOT a button — so the 本地 list fills itself.
/// Down/absent servers fail fast (short timeout) and are simply omitted.
#[tauri::command]
pub fn discover_local_llm() -> Vec<DiscoveredLocalLlm> {
    let timeout = std::time::Duration::from_millis(DISCOVER_TIMEOUT_MS);
    LOCAL_LLM_TARGETS
        .iter()
        .filter_map(|(provider, base_url)| {
            let endpoint = join_url(base_url, OPENAI_COMPATIBLE_MODELS_PATH);
            // Keyless localhost probe; any error (connection refused / timeout) drops
            // the server from the list. Empty model lists are dropped too — an alive
            // server with nothing pulled gives the user nothing to select.
            match fetch_chat_model_ids(&endpoint, None, timeout) {
                Ok(models) if !models.is_empty() => Some(DiscoveredLocalLlm {
                    provider: provider.to_string(),
                    base_url: base_url.to_string(),
                    models,
                }),
                _ => None,
            }
        })
        .collect()
}

fn fetch_chat_model_ids(
    endpoint: &str,
    api_key: Option<&str>,
    timeout: std::time::Duration,
) -> AppResult<Vec<String>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|_| AppError::Internal("build model list client".into()))?;

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
        .map_err(|_| AppError::Provider("解析模型列表失败，请检查响应格式或重试".into()))?;

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

#[cfg(test)]
mod tests;
