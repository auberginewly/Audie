use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::settings::validate_secret_key_id;

pub(crate) mod connection;
pub(crate) mod models;

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

fn resolve_api_key_for_provider_action(
    inline_api_key: Option<&str>,
    key_id: Option<&str>,
    local: bool,
    has_secret: impl FnOnce(&str) -> AppResult<bool>,
    read_secret: impl FnOnce(&str) -> AppResult<String>,
) -> AppResult<String> {
    if let Some(api_key) = inline_api_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(api_key.to_string());
    }
    if local {
        return Ok(String::new());
    }

    let key_id = key_id.ok_or_else(|| AppError::Provider("请先填写 API key".into()))?;
    validate_secret_key_id(key_id)?;
    if !has_secret(key_id)? {
        return Err(AppError::Provider("请先填写 API key".into()));
    }

    let api_key = read_secret(key_id)?;
    let api_key = api_key.trim();
    if api_key.is_empty() {
        Err(AppError::Provider("请先填写 API key".into()))
    } else {
        Ok(api_key.to_string())
    }
}

fn join_url(base: &str, path: &str) -> String {
    format!("{}{}", base.trim_end_matches('/'), path)
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
mod tests;
