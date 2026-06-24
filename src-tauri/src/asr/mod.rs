// ASR provider abstraction. PROJECT_SPEC.md §4.1 / §6.4.
//
// One trait, one adapter file per engine. Adding an engine = adding an adapter,
// without touching anything else. P0 ships a single batch (non-streaming)
// provider — Groq. P2 will extend this with a streaming variant.

pub mod doubao;
pub mod groq;
pub mod openai;
pub mod whisper_cpp;

use std::sync::mpsc::Receiver;

use crate::error::{AppError, AppResult};

/// One captured utterance, as interleaved f32 samples in [-1.0, 1.0].
/// Providers are responsible for encoding this into whatever wire format their
/// API expects (Groq wants a WAV/MP3 file part).
// Clone so the pipeline can keep a copy in `LastTake` for undo / retry while
// still transcribing the original (fe.8c).
#[derive(Clone)]
pub struct AudioData {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

/// One streaming audio packet. `sequence` is assigned by the stream producer so
/// adapters can preserve chunk order; `is_final` marks the input-side close.
#[allow(dead_code)] // P2.2 defines the streaming contract; P2.3+ wires providers.
pub struct AudioChunk {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub sequence: u64,
    pub is_final: bool,
}

/// A provider-normalized ASR update for the overlay. Partial updates may be
/// revised by later deltas; final updates are safe for the pipeline to commit.
#[allow(dead_code)] // P2.2 defines the streaming contract; P2.3+ emits deltas.
pub struct TranscriptDelta {
    pub text: String,
    pub is_final: bool,
    pub sequence: u64,
}

#[allow(dead_code)] // P2.2 contract type; consumed once streaming pipeline lands.
pub type AudioChunkStream = Receiver<AppResult<AudioChunk>>;
#[allow(dead_code)] // P2.2 contract type; consumed once streaming pipeline lands.
pub type TranscriptStream = Receiver<AppResult<TranscriptDelta>>;

/// Speech → text. Synchronous on purpose: the caller runs it on a dedicated
/// thread (no async runtime in P0). Streaming lands in P2 as a separate method.
pub trait AsrProvider: Send + Sync {
    #[allow(dead_code)] // surfaced in settings UI from P1 onward.
    fn name(&self) -> &str;

    fn transcribe(&self, audio: &AudioData) -> AppResult<String>;

    #[allow(dead_code)] // Default stub until each streaming adapter implements it.
    fn transcribe_stream(&self, _chunks: AudioChunkStream) -> AppResult<TranscriptStream> {
        Err(AppError::Internal(
            "streaming ASR is not implemented for this provider".into(),
        ))
    }
}

/// Encode f32 samples into a 16-bit PCM WAV (44-byte header + data).
/// Kept shared so remote batch ASR adapters don't each hand-roll their own.
pub(crate) fn encode_wav(audio: &AudioData) -> Vec<u8> {
    const BITS_PER_SAMPLE: u16 = 16;
    let channels = audio.channels.max(1);
    let sample_rate = audio.sample_rate;
    let byte_rate = sample_rate * channels as u32 * (BITS_PER_SAMPLE / 8) as u32;
    let block_align = channels * (BITS_PER_SAMPLE / 8);
    let data_len = (audio.samples.len() * 2) as u32;

    let mut buf = Vec::with_capacity(44 + data_len as usize);
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&(36 + data_len).to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
    buf.extend_from_slice(&1u16.to_le_bytes()); // audio format = PCM
    buf.extend_from_slice(&channels.to_le_bytes());
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    buf.extend_from_slice(&block_align.to_le_bytes());
    buf.extend_from_slice(&BITS_PER_SAMPLE.to_le_bytes());
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_len.to_le_bytes());
    for &s in &audio.samples {
        let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        buf.extend_from_slice(&v.to_le_bytes());
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_delta_can_represent_partial_text() {
        let delta = TranscriptDelta {
            text: "hel".into(),
            is_final: false,
            sequence: 1,
        };

        assert_eq!(delta.text, "hel");
        assert!(!delta.is_final);
        assert_eq!(delta.sequence, 1);
    }

    #[test]
    fn transcript_delta_can_represent_final_text() {
        let delta = TranscriptDelta {
            text: "hello".into(),
            is_final: true,
            sequence: 2,
        };

        assert_eq!(delta.text, "hello");
        assert!(delta.is_final);
        assert_eq!(delta.sequence, 2);
    }
}
