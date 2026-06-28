// ASR provider abstraction. PROJECT_SPEC.md §4.1 / §6.4.
//
// One trait, one adapter file per engine. Adding an engine = adding an adapter,
// without touching anything else. P0 ships a single batch (non-streaming)
// provider — Groq. P2 will extend this with a streaming variant.

pub mod aliyun;
pub mod doubao;
pub mod glm;
pub mod groq;
pub mod openai;
pub mod stepfun;
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

/// Cloud streaming ASR (doubao/aliyun/stepfun) all want 16 kHz mono PCM, so this
/// is the rate the shared `pcm16_mono_16k_bytes` resamples to. Doubao keeps its own
/// copy of this constant (config.rs) so it isn't refactored by this scaffolding.
pub(crate) const PCM16_TARGET_RATE: u32 = 16_000;

/// Downmix interleaved samples to a single mono channel by averaging frames.
/// Shared by the WS/SSE batch adapters (aliyun/stepfun) — doubao keeps its own.
pub(crate) fn downmix_to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    let channels = channels.max(1) as usize;
    if channels == 1 {
        return samples.to_vec();
    }

    samples
        .chunks(channels)
        .map(|frame| frame.iter().copied().sum::<f32>() / frame.len() as f32)
        .collect()
}

/// Linear-interpolation resample from `from_rate` to `to_rate`. Errors (not panics)
/// on a zero source rate — a broken capture must surface as §3.7 Device, not crash.
pub(crate) fn resample_linear(
    samples: &[f32],
    from_rate: u32,
    to_rate: u32,
) -> AppResult<Vec<f32>> {
    if from_rate == 0 {
        return Err(AppError::Device("audio sample rate is zero".into()));
    }
    if samples.is_empty() || from_rate == to_rate {
        return Ok(samples.to_vec());
    }

    let output_len = ((samples.len() as u64 * to_rate as u64) / from_rate as u64) as usize;
    if output_len == 0 {
        return Ok(Vec::new());
    }

    let ratio = from_rate as f32 / to_rate as f32;
    let mut output = Vec::with_capacity(output_len);
    for index in 0..output_len {
        let source = index as f32 * ratio;
        let left = source.floor() as usize;
        let right = (left + 1).min(samples.len() - 1);
        let t = source - left as f32;
        output.push(samples[left] * (1.0 - t) + samples[right] * t);
    }
    Ok(output)
}

/// Downmix to mono, resample to 16 kHz, and encode as little-endian 16-bit PCM —
/// the raw (header-less) byte layout aliyun/stepfun send on the wire. Shared so
/// each new cloud adapter doesn't re-derive the same conversion.
#[allow(dead_code)] // consumed once the aliyun/stepfun adapters move past the stub.
pub(crate) fn pcm16_mono_16k_bytes(audio: &AudioData) -> AppResult<Vec<u8>> {
    let mono = downmix_to_mono(&audio.samples, audio.channels);
    let resampled = resample_linear(&mono, audio.sample_rate, PCM16_TARGET_RATE)?;
    let mut pcm = Vec::with_capacity(resampled.len() * 2);
    for sample in resampled {
        let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        pcm.extend_from_slice(&value.to_le_bytes());
    }
    Ok(pcm)
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

    #[test]
    fn downmix_to_mono_averages_stereo_frames() {
        let mono = downmix_to_mono(&[1.0, -1.0, 0.5, 0.5], 2);
        assert_eq!(mono, vec![0.0, 0.5]);
    }

    #[test]
    fn resample_linear_rejects_zero_source_rate() {
        let err = resample_linear(&[0.0, 1.0], 0, PCM16_TARGET_RATE).unwrap_err();
        assert!(matches!(err, AppError::Device(_)));
    }

    #[test]
    fn pcm16_mono_16k_bytes_downmixes_and_keeps_16k_passthrough() {
        // Already 16k mono → no resample, so the two samples map straight to
        // little-endian i16 (0.0 → 0, 1.0 → i16::MAX = 0x7FFF).
        let audio = AudioData {
            samples: vec![0.0, 1.0],
            sample_rate: PCM16_TARGET_RATE,
            channels: 1,
        };
        let pcm = pcm16_mono_16k_bytes(&audio).expect("encodes");
        assert_eq!(pcm, vec![0, 0, 255, 127]);
    }

    #[test]
    fn pcm16_mono_16k_bytes_resamples_8k_to_16k() {
        let audio = AudioData {
            samples: vec![0.0, 1.0, 0.0, -1.0],
            sample_rate: 8_000,
            channels: 1,
        };
        let pcm = pcm16_mono_16k_bytes(&audio).expect("encodes");
        // 4 samples @ 8k → 8 samples @ 16k → 16 bytes.
        assert_eq!(pcm.len(), 16);
    }
}
