// Groq adapter — whisper-large-v3-turbo over Groq's OpenAI-compatible
// `/audio/transcriptions` endpoint. PROJECT_SPEC.md §4.1.
//
// P0 BYOK shortcut: the key is read from the GROQ_API_KEY env var at call time
// (P1 moves it into the system keychain, §4.2). Nothing is persisted here.

use serde::Deserialize;

use crate::asr::{AsrProvider, AudioData};
use crate::error::{AppError, AppResult};

const ENDPOINT: &str = "https://api.groq.com/openai/v1/audio/transcriptions";
const MODEL: &str = "whisper-large-v3-turbo";
const API_KEY_ENV: &str = "GROQ_API_KEY";

#[derive(Default)]
pub struct GroqProvider;

impl GroqProvider {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Deserialize)]
struct GroqResponse {
    text: String,
}

impl AsrProvider for GroqProvider {
    fn name(&self) -> &str {
        "groq"
    }

    fn transcribe(&self, audio: &AudioData) -> AppResult<String> {
        let api_key = std::env::var(API_KEY_ENV)
            .map_err(|_| AppError::Provider(format!("{API_KEY_ENV} not set")))?;

        let wav = encode_wav(audio);

        let file_part = reqwest::blocking::multipart::Part::bytes(wav)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| AppError::Internal(format!("build multipart part: {e}")))?;
        let form = reqwest::blocking::multipart::Form::new()
            .text("model", MODEL)
            .part("file", file_part);

        let client = reqwest::blocking::Client::new();
        let resp = client
            .post(ENDPOINT)
            .bearer_auth(api_key)
            .multipart(form)
            .send()
            .map_err(|e| AppError::Network(format!("groq request failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            // 401/403 mean the key is bad — not recoverable, user must fix it.
            return if status == 401 || status == 403 {
                Err(AppError::Provider(format!(
                    "groq auth failed ({status}): {body}"
                )))
            } else {
                Err(AppError::Network(format!("groq returned {status}: {body}")))
            };
        }

        let parsed: GroqResponse = resp
            .json()
            .map_err(|e| AppError::Provider(format!("parse groq response: {e}")))?;
        Ok(parsed.text.trim().to_string())
    }
}

/// Encode f32 samples into a 16-bit PCM WAV (44-byte header + data).
/// Hand-rolled to avoid pulling in a WAV crate for ~20 lines.
fn encode_wav(audio: &AudioData) -> Vec<u8> {
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
