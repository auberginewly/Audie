use serde::Serialize;

#[derive(Serialize, Clone)]
pub(super) struct FinalTranscript {
    pub(super) text: String,
    pub(super) duration_ms: u64,
}

#[derive(Serialize, Clone)]
#[allow(dead_code)] // 保留 partial 事件合同；当前产品只显示最终结果。
pub(super) struct PartialTranscript {
    pub(super) text: String,
    pub(super) is_final: bool,
    pub(super) sequence: u64,
}

#[derive(Serialize, Clone)]
pub(super) struct EnhanceProgress {
    pub(super) phase: &'static str,
    pub(super) message: String,
}

// 前端只需稳定错误类别、可操作说明和可恢复标志，不依赖 Rust 错误类型。
#[derive(Serialize, Clone)]
pub(super) struct ErrorPayload {
    pub(super) code: &'static str,
    pub(super) message: String,
    pub(super) recoverable: bool,
}
