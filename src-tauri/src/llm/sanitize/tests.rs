use super::*;

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
