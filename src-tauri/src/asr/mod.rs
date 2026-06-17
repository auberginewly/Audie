// ASR provider abstraction. PROJECT_SPEC.md §4.1 / §6.4.
//
// One trait, one adapter file per engine. Adding an engine = adding an adapter,
// without touching anything else. P0 ships a single batch (non-streaming)
// provider — Groq. P2 will extend this with a streaming variant.

pub mod groq;

use crate::error::AppResult;

/// One captured utterance, as interleaved f32 samples in [-1.0, 1.0].
/// Providers are responsible for encoding this into whatever wire format their
/// API expects (Groq wants a WAV/MP3 file part).
pub struct AudioData {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

/// Speech → text. Synchronous on purpose: the caller runs it on a dedicated
/// thread (no async runtime in P0). Streaming lands in P2 as a separate method.
pub trait AsrProvider: Send + Sync {
    #[allow(dead_code)] // surfaced in settings UI from P1 onward.
    fn name(&self) -> &str;

    fn transcribe(&self, audio: &AudioData) -> AppResult<String>;
}
