// StepFun ASR (SSE) configuration defaults.
//
// Field source: reverse-engineered from Voxt + StepFun SSE conventions, NOT yet
// confirmed against official docs. TODO: verify on real hardware / docs.

#![allow(dead_code)] // endpoint/format consts are consumed once `transcribe` lands.

/// StepFun streaming SSE transcription endpoint.
/// TODO: confirm the path + that it accepts base64 PCM with Accept: text/event-stream.
pub const ENDPOINT: &str = "https://api.stepfun.com/v1/audio/asr/sse";

/// Default model when settings leave `asr_model` empty.
pub const DEFAULT_MODEL: &str = "stepaudio-2.5-asr";

/// Keychain key id for the StepFun API key.
pub const SECRET_API_KEY: &str = "stepfun_api_key";

/// StepFun expects raw PCM16 16 kHz mono, base64-encoded (NO WAV header).
/// TODO: confirm format/sample_rate field names on the request body.
pub const SAMPLE_RATE: u32 = 16_000;
pub const AUDIO_FORMAT: &str = "pcm";
