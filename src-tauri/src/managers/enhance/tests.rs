use crate::error::AppError;

#[test]
fn enhance_failure_falls_back_to_original_text() {
    let original = "嗯那个今天我们开会讨论一下";
    let err = AppError::Network("LLM timeout".into());

    let outcome = super::fallback_after_enhance_failure(original, &err);

    assert_eq!(outcome.text_to_inject, original);
    assert_eq!(outcome.message, "LLM timeout");
}

#[test]
fn enhance_auth_failure_keeps_the_credential_reason() {
    let err = AppError::Provider("API key 无效，请检查设置".into());
    let outcome = super::fallback_after_enhance_failure("原文", &err);

    assert_eq!(outcome.text_to_inject, "原文");
    assert_eq!(outcome.message, err.message());
}
