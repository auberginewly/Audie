use crate::error::AppError;

#[test]
fn enhance_failure_falls_back_to_original_text() {
    let original = "嗯那个今天我们开会讨论一下";
    let err = AppError::Network("LLM timeout".into());

    let outcome = super::fallback_after_enhance_failure(original, &err);

    assert_eq!(outcome.text_to_inject, original);
    assert_eq!(outcome.message, "润色失败但已注入原文");
}
