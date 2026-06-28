// LLM provider abstraction + OpenAI-compatible adapter. PROJECT_SPEC.md §4.1.

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::managers::enhance::EnhanceConfig;

const CHAT_COMPLETIONS_PATH: &str = "/chat/completions";

pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &str;
    fn enhance(&self, text: &str, prompt: &str) -> AppResult<String>;
}

pub struct OpenAiCompatibleProvider {
    api_key: String,
    base_url: String,
    model: String,
}

impl OpenAiCompatibleProvider {
    pub fn new(api_key: String, base_url: String, model: String) -> Self {
        Self {
            api_key,
            base_url,
            model,
        }
    }
}

#[derive(Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
}

#[derive(Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatResponseMessage,
}

#[derive(Deserialize)]
struct ChatResponseMessage {
    content: String,
}

impl LlmProvider for OpenAiCompatibleProvider {
    fn name(&self) -> &str {
        "openai_compatible"
    }

    fn enhance(&self, text: &str, prompt: &str) -> AppResult<String> {
        let request = ChatCompletionRequest {
            model: self.model.clone(),
            temperature: 0.2,
            messages: vec![
                ChatMessage {
                    role: "system",
                    // Append /no_think so Qwen3 skips its <think> reasoning phase —
                    // that phase blows the hot-path latency budget ("松手即出"). Non-Qwen
                    // models ignore the unknown directive. Request-time only; the stored
                    // (user-editable) prompt is untouched.
                    content: format!("{prompt}\n\n/no_think"),
                },
                ChatMessage {
                    // Just the raw transcript — all polish instructions live in the
                    // (user-editable) system prompt, which already declares "the user
                    // message is the raw transcript". No instruction wrapper here, or
                    // it duplicates the system prompt and fights its anti-injection stance.
                    role: "user",
                    content: text.to_string(),
                },
            ],
        };

        let client = reqwest::blocking::Client::new();
        let resp = client
            .post(chat_completions_endpoint(&self.base_url))
            .bearer_auth(&self.api_key)
            .json(&request)
            .send()
            .map_err(classify_reqwest_error)?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            return Err(classify_http_status(status.as_u16(), &body));
        }

        let parsed: ChatCompletionResponse = resp
            .json()
            .map_err(|e| AppError::Provider(format!("解析 OpenAI-compatible 响应失败：{e}")))?;
        let content = parsed
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content)
            .ok_or_else(|| AppError::Provider("OpenAI-compatible 返回空润色结果".into()))?;
        // Strip <think> reasoning / preambles / code fences before the text reaches
        // the cursor; fall back to the raw transcript if only reasoning remains.
        let cleaned = sanitize_enhance_output(&content, text);
        if cleaned.is_empty() {
            return Err(AppError::Provider(
                "OpenAI-compatible 返回空润色结果".into(),
            ));
        }
        Ok(cleaned)
    }
}

pub(crate) fn build_provider(config: &EnhanceConfig) -> AppResult<Box<dyn LlmProvider>> {
    match config.llm_provider.as_str() {
        "openai_compatible" => {
            let base_url = required_setting(
                &config.openai_compatible_base_url,
                "OpenAI-compatible base URL 未配置，请先到设置页填写",
            )?;
            // Local providers (Ollama / LM Studio on localhost) accept an empty key;
            // a missing cloud key is still a configuration error so the user gets a
            // clear "key 未配置" instead of a request-time 401.
            let api_key = if is_local_endpoint(&base_url) {
                config.openai_compatible_api_key.clone().unwrap_or_default()
            } else {
                required_key(
                    &config.openai_compatible_api_key,
                    "OpenAI-compatible API key 未配置，请先到设置页填写",
                )?
            };
            Ok(Box::new(OpenAiCompatibleProvider::new(
                api_key,
                base_url,
                required_setting(
                    &config.openai_compatible_model,
                    "OpenAI-compatible model 未配置，请先到设置页填写",
                )?,
            )))
        }
        other => Err(AppError::Internal(format!(
            "unsupported LLM provider: {other}"
        ))),
    }
}

/// A localhost / 127.0.0.1 base URL means a local OpenAI-compatible server
/// (Ollama, LM Studio) where the API key is optional. Shared with provider_test
/// so the 测试 button doesn't demand a key for local endpoints either.
pub(crate) fn is_local_endpoint(base_url: &str) -> bool {
    let host = base_url
        .split("://")
        .nth(1)
        .unwrap_or(base_url)
        .split('/')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("");
    host == "localhost" || host == "127.0.0.1" || host == "[::1]"
}

fn required_key(value: &Option<String>, message: &str) -> AppResult<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| AppError::Provider(message.into()))
}

fn required_setting(value: &str, message: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(AppError::Provider(message.into()))
    } else {
        Ok(trimmed.to_string())
    }
}

pub(crate) fn chat_completions_endpoint(base_url: &str) -> String {
    format!(
        "{}{}",
        base_url.trim().trim_end_matches('/'),
        CHAT_COMPLETIONS_PATH
    )
}

fn classify_reqwest_error(e: reqwest::Error) -> AppError {
    if e.is_timeout() {
        AppError::Network("请求 OpenAI-compatible 超时，请检查网络或代理".into())
    } else if e.is_connect() {
        AppError::Network("无法连接 OpenAI-compatible，请检查网络或代理".into())
    } else {
        AppError::Network(format!("OpenAI-compatible 请求失败：{e}"))
    }
}

pub(crate) fn classify_http_status(status: u16, body: &str) -> AppError {
    match status {
        401 => AppError::Provider("OpenAI-compatible API key 无效，请检查设置".into()),
        403 => AppError::Network("OpenAI-compatible 拒绝请求，可能是网络、代理或地区限制".into()),
        429 => AppError::Network("OpenAI-compatible 请求过于频繁或额度受限，请稍后重试".into()),
        500..=599 => AppError::Network(format!("OpenAI-compatible 服务端异常（{status}）")),
        _ => {
            let snippet: String = body.chars().take(200).collect();
            AppError::Provider(format!("OpenAI-compatible 拒绝请求（{status}）：{snippet}"))
        }
    }
}

// ── Output sanitizer ─────────────────────────────────────────────────────────
// Local thinking models (Qwen3) emit <think>…</think>, and some models wrap the
// answer in a preamble or code fence. Left in, they'd be injected at the cursor.
// We strip them and, when only reasoning is left, fall back to the raw transcript
// rather than inject garbage.

const PREAMBLE_MARKERS: [&str; 5] = [
    "Final Output:",
    "Result:",
    "Cleaned Text:",
    "输出：",
    "以下是",
];

fn sanitize_enhance_output(raw: &str, fallback: &str) -> String {
    let cleaned = strip_think_blocks(raw);
    let cleaned = unwrap_code_fence(&cleaned);
    let cleaned = extract_after_preamble(&cleaned);
    let cleaned = cleaned.trim();
    if cleaned.is_empty() || looks_like_reasoning(cleaned) {
        return fallback.trim().to_string();
    }
    cleaned.to_string()
}

/// Case-insensitive byte position of an ASCII `needle`, scanning on char boundaries
/// so the returned index is always valid for slicing `haystack` (never mid-char).
/// Deliberately avoids the classic bug of indexing the original string with offsets
/// taken from a `to_lowercase()` copy, whose byte length can differ (e.g. 'ẞ', 'İ').
fn find_ci(haystack: &str, needle: &str) -> Option<usize> {
    let nlen = needle.len();
    haystack.char_indices().find_map(|(i, _)| {
        haystack
            .get(i..i + nlen)
            .filter(|slice| slice.eq_ignore_ascii_case(needle))
            .map(|_| i)
    })
}

fn strip_think_blocks(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = find_ci(rest, "<think>") {
        out.push_str(&rest[..open]);
        let after_open = &rest[open + "<think>".len()..];
        match find_ci(after_open, "</think>") {
            Some(close) => rest = &after_open[close + "</think>".len()..],
            None => rest = "", // unclosed tag: drop the remainder
        }
    }
    out.push_str(rest);
    // Drop any stray tags left behind.
    let out = strip_all(&out, "<think>");
    strip_all(&out, "</think>")
}

fn strip_all(text: &str, tag: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = find_ci(rest, tag) {
        out.push_str(&rest[..at]);
        rest = &rest[at + tag.len()..];
    }
    out.push_str(rest);
    out
}

fn unwrap_code_fence(text: &str) -> String {
    let trimmed = text.trim();
    if let Some(inner) = trimmed
        .strip_prefix("```")
        .and_then(|s| s.strip_suffix("```"))
    {
        // Drop an optional language tag on the fence's first line.
        let body = match inner.split_once('\n') {
            Some((first, rest)) if !first.contains(' ') && !first.contains('`') => rest,
            _ => inner,
        };
        return body.trim().to_string();
    }
    text.to_string()
}

fn extract_after_preamble(text: &str) -> String {
    let body = PREAMBLE_MARKERS
        .iter()
        .filter_map(|marker| rfind_ci_end(text, marker))
        .max()
        .map_or(text, |end| &text[end..]);
    body.trim()
        .trim_matches(|c| matches!(c, '"' | '\'' | '`' | '“' | '”' | '「' | '」'))
        .trim()
        .to_string()
}

/// Byte index just past the LAST case-insensitive match of an ASCII `needle`.
fn rfind_ci_end(haystack: &str, needle: &str) -> Option<usize> {
    let nlen = needle.len();
    haystack.char_indices().rev().find_map(|(i, _)| {
        haystack
            .get(i..i + nlen)
            .filter(|slice| slice.eq_ignore_ascii_case(needle))
            .map(|_| i + nlen)
    })
}

fn looks_like_reasoning(text: &str) -> bool {
    text.lines().take(3).any(|line| {
        let line = line.trim_start();
        // to_ascii_lowercase is length-preserving and only feeds starts_with, so no
        // char-boundary hazard (unlike full Unicode to_lowercase).
        let lower = line.to_ascii_lowercase();
        lower.starts_with("reasoning")
            || lower.starts_with("thinking")
            || starts_with_numbered(&lower, "step ")
            || starts_with_numbered(&lower, "rule ")
            || line.starts_with("思考过程")
            || line.starts_with("推理")
            || line.starts_with("分析：")
    })
}

fn starts_with_numbered(lower: &str, kw: &str) -> bool {
    lower
        .strip_prefix(kw)
        .and_then(|rest| rest.chars().next())
        .is_some_and(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppError;
    use crate::managers::enhance::EnhanceConfig;

    #[test]
    fn openai_compatible_requires_keychain_secret() {
        let config = EnhanceConfig {
            llm_provider: "openai_compatible".into(),
            enhance_enabled: true,
            enhance_prompt: "去口水话".into(),
            openai_compatible_api_key: None,
            openai_compatible_base_url: "https://api.openai.com/v1".into(),
            openai_compatible_model: "gpt-4o-mini".into(),
        };

        let err = match build_provider(&config) {
            Ok(_) => panic!("expected OpenAI-compatible without key to fail"),
            Err(err) => err,
        };

        assert!(matches!(err, AppError::Provider(_)));
        assert_eq!(
            err.message(),
            "OpenAI-compatible API key 未配置，请先到设置页填写"
        );
    }

    #[test]
    fn local_endpoint_allows_missing_key() {
        // Ollama / LM Studio on localhost: no key is fine (4b key-optional local).
        let config = EnhanceConfig {
            llm_provider: "openai_compatible".into(),
            enhance_enabled: true,
            enhance_prompt: "去口水话".into(),
            openai_compatible_api_key: None,
            openai_compatible_base_url: "http://localhost:11434/v1".into(),
            openai_compatible_model: "qwen2.5".into(),
        };

        assert!(build_provider(&config).is_ok());
    }

    #[test]
    fn detects_local_endpoints() {
        assert!(is_local_endpoint("http://localhost:11434/v1"));
        assert!(is_local_endpoint("http://127.0.0.1:1234/v1"));
        assert!(!is_local_endpoint("https://api.deepseek.com/v1"));
    }

    #[test]
    fn joins_chat_completions_endpoint() {
        assert_eq!(
            chat_completions_endpoint("https://api.deepseek.com/v1/"),
            "https://api.deepseek.com/v1/chat/completions"
        );
    }

    #[test]
    fn classifies_openai_compatible_http_errors() {
        assert!(matches!(
            classify_http_status(401, ""),
            AppError::Provider(_)
        ));
        assert!(matches!(
            classify_http_status(403, ""),
            AppError::Network(_)
        ));
        assert!(matches!(
            classify_http_status(429, ""),
            AppError::Network(_)
        ));
        assert!(matches!(
            classify_http_status(500, ""),
            AppError::Network(_)
        ));
    }

    #[test]
    fn sanitize_strips_think_block() {
        assert_eq!(
            sanitize_enhance_output("<think>权衡了很多</think>最终文本", "原始"),
            "最终文本"
        );
    }

    #[test]
    fn sanitize_extracts_after_preamble() {
        assert_eq!(
            sanitize_enhance_output("输出：清理后的句子", "原始"),
            "清理后的句子"
        );
    }

    #[test]
    fn sanitize_unwraps_code_fence() {
        assert_eq!(
            sanitize_enhance_output("```\n清理后的句子\n```", "原始"),
            "清理后的句子"
        );
    }

    #[test]
    fn sanitize_passes_clean_text() {
        assert_eq!(
            sanitize_enhance_output("就是一句普通的话。", "原始"),
            "就是一句普通的话。"
        );
    }

    #[test]
    fn sanitize_falls_back_on_pure_reasoning() {
        assert_eq!(
            sanitize_enhance_output("<think>只有推理没有结论</think>", "原始转写"),
            "原始转写"
        );
    }

    #[test]
    fn sanitize_handles_length_changing_unicode_without_panic() {
        // Regression: full Unicode to_lowercase() can change byte length (ẞ, İ),
        // which once panicked when its offsets sliced the original string.
        assert_eq!(
            sanitize_enhance_output("ẞİ<think>x</think>clean", "fb"),
            "ẞİclean"
        );
    }
}
