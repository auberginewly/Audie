// Doubao streaming ASR (volcengine bigmodel) — PROJECT_SPEC.md §5 / P2.3+.
//
// P2.3 lands the pure-function binary codec only: no WebSocket, no manager
// integration, no UI. Later slices wire it into AudioManager + hot path.
//
// Protocol reference: agent-project/voxt/Voxt/Transcription/RemoteASRTranscriber+DoubaoTypes.swift
// + DoubaoASRConfiguration.swift. We re-implement in Rust, not transliterate.

pub mod codec;
