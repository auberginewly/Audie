// StepFun ASR (SSE, whole-utterance) — PROJECT_SPEC.md §5.2.1.
//
// `config` holds the official endpoint/model/key-id constants; `client` owns the
// HTTP-SSE request shape, the SSE line parser (pure, unit-tested offline), the
// whole-stream accumulator, and the §3.7 error classifier. `transcribe` POSTs
// base64 PCM with Accept: text/event-stream and reads the SSE event stream to a
// final transcript. StepFun is SSE-over-HTTP, not a custom binary protocol, so no
// `codec` module is needed (the SSE parsing lives in `client`).
//
// Field source: reverse-engineered from Voxt + StepFun SSE conventions, NOT yet
// confirmed against official docs. Unverified fields are marked TODO in `client` /
// `config` (notably the SSE event `type` names and the omitted hotwords/prompt).

pub mod client;
pub mod config;
