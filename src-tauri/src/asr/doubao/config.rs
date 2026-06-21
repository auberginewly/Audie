// Doubao streaming ASR configuration defaults (volcengine bigmodel) — P2.5.
//
// Protocol source: Volcengine docs "大模型流式语音识别API" (doc 6561/1354869).
// Endpoint + resource id are non-secret store fields. Old-console AppID and the
// new-console API key / old-console Access Token are keychain secrets.

/// Default async bigmodel streaming endpoint.
pub const DEFAULT_ENDPOINT: &str = "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async";

/// Legacy ASR 1.0 duration resource id kept for settings migration.
pub const LEGACY_RESOURCE_ID: &str = "volc.bigasr.sauc.duration";

/// Default resource id for the duration-billed seed ASR 2.0 SAUC stream.
pub const DEFAULT_RESOURCE_ID: &str = "volc.seedasr.sauc.duration";

/// Keychain key id for the Doubao AppID (sensitive per Voxt's model).
pub const SECRET_APP_ID: &str = "doubao_app_id";

/// Keychain key id for the new-console API Key or old-console Access Token.
/// Keep the stored id stable for users who already saved `doubao_access_token`.
pub const SECRET_API_KEY_OR_ACCESS_TOKEN: &str = "doubao_access_token";

// Streaming PCM parameters (P2.5). Doubao bigmodel SAUC expects 16 kHz / mono /
// 16-bit PCM.
pub const STREAMING_SAMPLE_RATE: u32 = 16_000;
pub const STREAMING_BITS_PER_SAMPLE: u16 = 16;
pub const STREAMING_CHANNELS: u16 = 1;
pub const STREAMING_AUDIO_FORMAT: &str = "pcm";
pub const STREAMING_AUDIO_CODEC: &str = "raw";

/// Recommended per-frame PCM payload: 200ms of 16k/mono/16-bit audio.
/// = 16000 * 16 * 1 / 8 / 5 = 6400 bytes (Voxt `recommendedStreamingPacketBytes`).
pub const STREAMING_PACKET_BYTES: usize = 6_400;
