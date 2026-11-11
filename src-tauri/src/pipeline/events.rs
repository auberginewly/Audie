use super::*;
#[derive(Serialize, Clone)]
pub(super) struct FinalTranscript {
    pub(super) text: String,
    pub(super) duration_ms: u64,
}

#[derive(Serialize, Clone)]
#[allow(dead_code)] // P2.2 defines the event payload; P2.5 will emit it to overlay.
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

// §3.6 `error` event payload. Flattens AppError's category + message and adds
// the recoverable flag from the §3.7 table.
#[derive(Serialize, Clone)]
pub(super) struct ErrorPayload {
    pub(super) code: &'static str,
    pub(super) message: String,
    pub(super) recoverable: bool,
}
