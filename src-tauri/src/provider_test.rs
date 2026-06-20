use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::commands::{
    available_asr_providers, available_llm_providers, validate_secret_key_id, DEFAULT_ASR_PROVIDER,
    DEFAULT_LLM_PROVIDER,
};
use crate::error::{AppError, AppResult};
use crate::platform::Platform;

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
pub fn test_provider(
    app: AppHandle,
    request: ProviderTestRequest,
) -> Result<ProviderTestResult, AppError> {
    validate_provider_target(request.kind, &request.provider_id)?;
    validate_secret_key_id(&request.key_id)?;

    let platform = app.state::<Arc<dyn Platform>>();
    let api_key = read_api_key_for_test(
        platform.inner().as_ref(),
        &request.key_id,
        request.api_key.as_deref(),
    )?;

    let endpoint = provider_test_endpoint(request.kind, &request.provider_id, request.base_url)?;
    test_models_endpoint(&endpoint, &api_key)?;

    Ok(ProviderTestResult {
        ok: true,
        message: "连接测试通过".into(),
    })
}

fn read_api_key_for_test(
    _platform: &dyn Platform,
    _key_id: &str,
    inline_api_key: Option<&str>,
) -> AppResult<String> {
    if let Some(api_key) = inline_api_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Ok(api_key.to_string())
    } else {
        Err(AppError::Provider("请先填写 API key".into()))
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

    let resp = client
        .get(endpoint)
        .bearer_auth(api_key)
        .send()
        .map_err(classify_reqwest_error)?;

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
    fn inline_api_key_is_used_without_reading_keychain() {
        struct PanicOnReadPlatform;

        impl Platform for PanicOnReadPlatform {
            fn register_hotkey(
                &self,
                _app: &AppHandle,
                _combo: &str,
                _callback: crate::platform::HotkeyCallback,
            ) -> crate::error::AppResult<()> {
                unreachable!()
            }

            fn unregister_all_hotkeys(&self, _app: &AppHandle) -> crate::error::AppResult<()> {
                unreachable!()
            }

            fn inject_text(&self, _app: &AppHandle, _text: &str) -> crate::error::AppResult<()> {
                unreachable!()
            }

            fn ensure_microphone_permission(&self) -> bool {
                unreachable!()
            }

            fn store_secret(&self, _key: &str, _value: &str) -> crate::error::AppResult<()> {
                unreachable!()
            }

            fn has_secret(&self, _key: &str) -> crate::error::AppResult<bool> {
                unreachable!()
            }

            fn read_secret(&self, _key: &str) -> crate::error::AppResult<String> {
                panic!("inline provider test key must not read keychain")
            }

            fn delete_secret(&self, _key: &str) -> crate::error::AppResult<()> {
                unreachable!()
            }
        }

        let api_key =
            read_api_key_for_test(&PanicOnReadPlatform, "groq_api_key", Some("  inline-key  "))
                .unwrap();

        assert_eq!(api_key, "inline-key");
    }

    #[test]
    fn blank_inline_api_key_is_provider_error_without_reading_keychain() {
        struct PanicOnReadPlatform;

        impl Platform for PanicOnReadPlatform {
            fn register_hotkey(
                &self,
                _app: &AppHandle,
                _combo: &str,
                _callback: crate::platform::HotkeyCallback,
            ) -> crate::error::AppResult<()> {
                unreachable!()
            }

            fn unregister_all_hotkeys(&self, _app: &AppHandle) -> crate::error::AppResult<()> {
                unreachable!()
            }

            fn inject_text(&self, _app: &AppHandle, _text: &str) -> crate::error::AppResult<()> {
                unreachable!()
            }

            fn ensure_microphone_permission(&self) -> bool {
                unreachable!()
            }

            fn store_secret(&self, _key: &str, _value: &str) -> crate::error::AppResult<()> {
                unreachable!()
            }

            fn has_secret(&self, _key: &str) -> crate::error::AppResult<bool> {
                unreachable!()
            }

            fn read_secret(&self, _key: &str) -> crate::error::AppResult<String> {
                panic!("blank provider test key must not read keychain")
            }

            fn delete_secret(&self, _key: &str) -> crate::error::AppResult<()> {
                unreachable!()
            }
        }

        let err =
            read_api_key_for_test(&PanicOnReadPlatform, "groq_api_key", Some("  ")).unwrap_err();

        assert!(matches!(err, AppError::Provider(_)));
        assert_eq!(err.message(), "请先填写 API key");
    }

    #[test]
    fn missing_inline_api_key_is_provider_error_without_reading_keychain() {
        struct PanicOnReadPlatform;

        impl Platform for PanicOnReadPlatform {
            fn register_hotkey(
                &self,
                _app: &AppHandle,
                _combo: &str,
                _callback: crate::platform::HotkeyCallback,
            ) -> crate::error::AppResult<()> {
                unreachable!()
            }

            fn unregister_all_hotkeys(&self, _app: &AppHandle) -> crate::error::AppResult<()> {
                unreachable!()
            }

            fn inject_text(&self, _app: &AppHandle, _text: &str) -> crate::error::AppResult<()> {
                unreachable!()
            }

            fn ensure_microphone_permission(&self) -> bool {
                unreachable!()
            }

            fn store_secret(&self, _key: &str, _value: &str) -> crate::error::AppResult<()> {
                unreachable!()
            }

            fn has_secret(&self, _key: &str) -> crate::error::AppResult<bool> {
                unreachable!()
            }

            fn read_secret(&self, _key: &str) -> crate::error::AppResult<String> {
                panic!("missing provider test key must not read keychain")
            }

            fn delete_secret(&self, _key: &str) -> crate::error::AppResult<()> {
                unreachable!()
            }
        }

        let err = read_api_key_for_test(&PanicOnReadPlatform, "groq_api_key", None).unwrap_err();

        assert!(matches!(err, AppError::Provider(_)));
        assert_eq!(err.message(), "请先填写 API key");
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
