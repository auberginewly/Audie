use super::*;

#[test]
fn polish_enabled_when_cloud_key_present() {
    let mut requested = Vec::new();

    // 润色 (force=false, 开关 on): reads the active provider's key to derive
    // enablement; a present cloud key → enabled.
    let config = enhance_config_from_settings(
        "openai_compatible".into(),
        false,
        true,
        "prompt".into(),
        "https://api.deepseek.com/v1".into(),
        "model".into(),
        "deepseek_api_key".into(),
        |key_id| {
            requested.push(key_id.to_string());
            Some(format!("{key_id}-value"))
        },
    );

    assert_eq!(requested, vec!["deepseek_api_key"]);
    assert!(config.enhance_enabled);
    assert_eq!(
        config.openai_compatible_api_key.as_deref(),
        Some("deepseek_api_key-value")
    );
}

#[test]
fn polish_disabled_when_cloud_key_missing() {
    // 润色 (force=false, 开关 on): no cloud key configured → disabled = 静默纯转写.
    let config = enhance_config_from_settings(
        "openai_compatible".into(),
        false,
        true,
        "prompt".into(),
        "https://api.deepseek.com/v1".into(),
        "model".into(),
        "deepseek_api_key".into(),
        |_| None,
    );

    assert!(!config.enhance_enabled);
    assert_eq!(config.openai_compatible_api_key, None);
}

#[test]
fn polish_enabled_for_local_endpoint_without_key() {
    let mut requested = Vec::new();

    // Ollama / LM Studio: a local endpoint needs no key, so 润色 is enabled even
    // with force=false (开关 on) and an empty key id (no read happens).
    let config = enhance_config_from_settings(
        "openai_compatible".into(),
        false,
        true,
        "prompt".into(),
        "http://localhost:11434/v1".into(),
        "qwen2.5".into(),
        String::new(),
        |key_id| {
            requested.push(key_id.to_string());
            Some(format!("{key_id}-value"))
        },
    );

    assert!(requested.is_empty());
    assert!(config.enhance_enabled);
    assert_eq!(config.openai_compatible_api_key, None);
}

#[test]
fn forced_enable_stays_on_without_key() {
    // compose / rewrite / History 重试 (force=true): enabled even unconfigured AND
    // even with the 「AI 润色」 toggle off — force overrides it. A missing key then
    // surfaces as a failure (fallback) instead of silent passthrough.
    let config = enhance_config_from_settings(
        "openai_compatible".into(),
        true,
        false,
        "prompt".into(),
        "https://api.deepseek.com/v1".into(),
        "model".into(),
        "deepseek_api_key".into(),
        |_| None,
    );

    assert!(config.enhance_enabled);
    assert_eq!(config.openai_compatible_api_key, None);
}

#[test]
fn polish_toggle_off_skips_key_read_despite_cloud_key() {
    let mut requested = Vec::new();

    // 「AI 润色」开关 off (force=false): 纯转写 even with a cloud key configured, and
    // the keychain is never touched (只在真要用 key 时才读). 给想要原文的人选择权.
    let config = enhance_config_from_settings(
        "openai_compatible".into(),
        false,
        false,
        "prompt".into(),
        "https://api.deepseek.com/v1".into(),
        "model".into(),
        "deepseek_api_key".into(),
        |key_id| {
            requested.push(key_id.to_string());
            Some(format!("{key_id}-value"))
        },
    );

    assert!(requested.is_empty());
    assert!(!config.enhance_enabled);
    assert_eq!(config.openai_compatible_api_key, None);
}
