// Doubao streaming ASR configuration defaults (volcengine bigmodel) — P2.4.
//
// Endpoint + resource id are non-secret and live in tauri-plugin-store; AppID and
// Access Token are sensitive and go to the system keychain (key ids below) —
// aligned with Voxt, which treats appID as a sensitive field too. Wiring these
// into the WebSocket client lands in P2.5.

/// Default async bigmodel streaming endpoint.
/// Reference: Voxt `DoubaoASRConfiguration.swift`.
pub const DEFAULT_ENDPOINT: &str = "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async";

/// Default resource id for the duration-billed bigmodel SAUC stream.
pub const DEFAULT_RESOURCE_ID: &str = "volc.bigasr.sauc.duration";

/// Keychain key id for the Doubao AppID (sensitive per Voxt's model).
pub const SECRET_APP_ID: &str = "doubao_app_id";

/// Keychain key id for the Doubao Access Token.
pub const SECRET_ACCESS_TOKEN: &str = "doubao_access_token";
