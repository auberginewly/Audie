// 通义 / DashScope Fun-ASR realtime ASR (WebSocket) — PROJECT_SPEC.md §5.2.1.
//
// Scaffolding slice: `config` holds the official endpoint/model/key-id constants,
// `codec` builds the run-task / finish-task JSON frames and parses the
// result-generated / task-finished events (pure functions, unit-tested offline),
// and `client` owns the WS session + auth + error classification (stub transcribe
// until the live session lands).
//
// Field source: DashScope realtime WS path, reverse-engineered from Voxt — NOT yet
// confirmed against the official 通义 docs. Unverified fields are marked TODO in
// `codec` / `config`.

pub mod client;
pub mod codec;
pub mod config;
