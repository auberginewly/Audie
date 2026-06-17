// TranscriptionManager — owns the active AsrProvider and turns buffered audio
// into text. PROJECT_SPEC.md §6.1. P0 hard-wires Groq; P1 makes the provider
// selectable from settings.

use crate::asr::groq::GroqProvider;
use crate::asr::{AsrProvider, AudioData};
use crate::error::AppResult;

pub struct TranscriptionManager {
    provider: Box<dyn AsrProvider>,
}

impl TranscriptionManager {
    pub fn new() -> Self {
        Self {
            provider: Box::new(GroqProvider::new()),
        }
    }

    pub fn transcribe(&self, audio: &AudioData) -> AppResult<String> {
        self.provider.transcribe(audio)
    }
}

impl Default for TranscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}
