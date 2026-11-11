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
