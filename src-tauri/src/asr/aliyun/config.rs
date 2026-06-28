// 通义 / DashScope Fun-ASR realtime configuration defaults.
//
// Field source: reverse-engineered from Voxt + DashScope realtime conventions,
// NOT yet confirmed against official docs. TODO: verify on real hardware / docs.

/// DashScope realtime inference WebSocket endpoint.
/// TODO: confirm the api-ws path + whether a query/header carries the model.
pub const DEFAULT_ENDPOINT: &str = "wss://dashscope.aliyuncs.com/api-ws/v1/inference";

/// Default Fun-ASR realtime model when settings leave `asr_model` empty.
pub const DEFAULT_MODEL: &str = "fun-asr-realtime";

/// Keychain key id for the DashScope API key.
pub const SECRET_API_KEY: &str = "aliyun_dashscope_api_key";

/// DashScope realtime PCM parameters: 16 kHz / mono / 16-bit.
/// TODO: confirm the `format` string DashScope expects (pcm vs wav vs raw).
pub const SAMPLE_RATE: u32 = 16_000;
pub const AUDIO_FORMAT: &str = "pcm";
