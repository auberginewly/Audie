use crate::error::AppError;
use crate::platform::Platform;
use crate::settings::{
    available_asr_providers, available_llm_providers, load_settings, ProviderMetadata,
};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
#[tauri::command]
pub fn list_asr_providers() -> Vec<ProviderMetadata> {
    available_asr_providers()
}

#[tauri::command]
pub fn list_llm_providers() -> Vec<ProviderMetadata> {
    available_llm_providers()
}

/// Assemble the Doubao streaming config from settings (endpoint / resource_id) +
/// keychain (app_id optional — blank = new-console single-key mode; access token
/// required). Reads secrets synchronously and returns an owned config, so callers
/// never hold the Tauri State across an await. Shared by the dev streaming probe
/// and the production connection test.
fn doubao_config_from_settings(
    app: &AppHandle,
) -> Result<crate::asr::doubao::client::DoubaoStreamConfig, AppError> {
    use crate::asr::doubao::{client, config};

    // endpoint / resource_id come from the (normalized) TOML settings; secrets from
    // keychain. Read both before any await so no Tauri State is held across it.
    let settings = load_settings(app);
    let platform = app.state::<Arc<dyn Platform>>();
    let app_id = platform
        .read_secret(config::SECRET_APP_ID)
        .unwrap_or_default();
    let api_key_or_access_token = platform
        .read_secret(config::SECRET_API_KEY_OR_ACCESS_TOKEN)
        .map_err(|_| AppError::Provider("豆包 API Key / Access Token 未配置".into()))?;

    Ok(client::DoubaoStreamConfig {
        endpoint: settings.doubao_endpoint,
        auth: client::DoubaoAuth::from_settings(app_id, api_key_or_access_token),
        resource_id: settings.doubao_resource_id,
    })
}

/// Production connectivity test for Doubao streaming ASR — drives the model config
/// dialog's 测试 button. Doubao is WebSocket-only (no /models endpoint), so this
/// can't go through `test_provider`; it opens the WS + handshake and checks one
/// frame for an auth/config error (no audio sent). See `client::test_connection`.
#[tauri::command]
pub async fn test_doubao_connection(
    app: AppHandle,
) -> Result<crate::provider_test::ProviderTestResult, AppError> {
    let cfg = doubao_config_from_settings(&app)?;
    crate::asr::doubao::client::test_connection(&cfg).await?;
    Ok(crate::provider_test::ProviderTestResult {
        ok: true,
        message: "连接测试通过".into(),
    })
}

/// Dev-only streaming probe for the Doubao ASR (P2.5). Reads a local 16k/mono/16-bit
/// wav and streams it to Doubao, logging partial/final text. Not registered in
/// release builds — the recording hot path lands P2.6.
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn test_doubao_streaming(app: AppHandle, wav_path: String) -> Result<String, AppError> {
    use crate::asr::doubao::client;

    let pcm16 = read_wav_pcm16(&wav_path)?;
    let cfg = doubao_config_from_settings(&app)?;
    log::info!("test_doubao_streaming: {} PCM bytes", pcm16.len());
    let text = client::transcribe_pcm16(&cfg, &pcm16).await?;
    log::info!(
        "test_doubao_streaming completed (chars={})",
        text.chars().count()
    );
    Ok(text)
}

/// Decode a 16k/mono/16-bit wav into little-endian PCM16 bytes (hound decoder).
#[cfg(debug_assertions)]
fn read_wav_pcm16(path: &str) -> Result<Vec<u8>, AppError> {
    use crate::asr::doubao::config;

    let mut reader =
        hound::WavReader::open(path).map_err(|err| AppError::Device(format!("open wav: {err}")))?;
    let spec = reader.spec();
    if spec.sample_rate != config::STREAMING_SAMPLE_RATE
        || spec.channels != config::STREAMING_CHANNELS
        || spec.bits_per_sample != config::STREAMING_BITS_PER_SAMPLE
    {
        return Err(AppError::Device(format!(
            "expect 16k mono 16-bit wav, got {} Hz / {} ch / {} bit",
            spec.sample_rate, spec.channels, spec.bits_per_sample
        )));
    }

    let mut bytes = Vec::new();
    for sample in reader.samples::<i16>() {
        let sample = sample.map_err(|err| AppError::Device(format!("read wav sample: {err}")))?;
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    Ok(bytes)
}
