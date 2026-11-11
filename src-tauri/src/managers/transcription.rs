// TranscriptionManager — owns the active AsrProvider and turns buffered audio
// into text. PROJECT_SPEC.md §6.1. P1 makes the provider
// selectable from settings.
// This manager only chooses and calls an ASR adapter; it does not emit UI events,
// mutate app state, or decide fallback behavior. Those decisions stay in lib.rs.

use crate::asr::aliyun::client::AliyunProvider;
use crate::asr::doubao::client::{DoubaoAuth, DoubaoStreamConfig, DoubaoStreamingProvider};
use crate::asr::glm::GlmProvider;
use crate::asr::openai::OpenAiProvider;
use crate::asr::stepfun::client::StepFunProvider;
use crate::asr::{AsrProvider, AudioChunkStream, AudioData, TranscriptStream};
use crate::error::{AppError, AppResult};

pub struct TranscriptionManager;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TranscriptionConfig {
    pub asr_provider: String,
    /// Selected ASR model id; empty = adapter built-in default. Doubao ignores it.
    pub asr_model: String,
    pub openai_api_key: Option<String>,
    pub doubao_endpoint: Option<String>,
    pub doubao_resource_id: Option<String>,
    pub doubao_app_id: Option<String>,
    pub doubao_api_key_or_access_token: Option<String>,
    pub glm_endpoint: Option<String>,
    pub glm_api_key: Option<String>,
    pub aliyun_endpoint: Option<String>,
    pub aliyun_api_key: Option<String>,
    pub stepfun_endpoint: Option<String>,
    pub stepfun_api_key: Option<String>,
}

impl TranscriptionManager {
    pub fn new() -> Self {
        Self
    }

    pub fn transcribe(&self, audio: &AudioData, config: &TranscriptionConfig) -> AppResult<String> {
        // Provider choice is data-driven by Settings. Adding a new batch ASR
        // means adding an adapter plus one `build_provider` arm, not changing
        // the hotkey pipeline.
        let provider = build_provider(config)?;
        provider.transcribe(audio)
    }

    #[allow(dead_code)] // P2.2 exposes the contract; hotkey pipeline stays batch for now.
    pub fn transcribe_stream(
        &self,
        chunks: AudioChunkStream,
        config: &TranscriptionConfig,
    ) -> AppResult<TranscriptStream> {
        let provider = build_provider(config)?;
        provider.transcribe_stream(chunks)
    }
}

impl Default for TranscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}

fn required_key(value: &Option<String>, message: &str) -> AppResult<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| AppError::Provider(message.into()))
}

fn build_provider(config: &TranscriptionConfig) -> AppResult<Box<dyn AsrProvider>> {
    match config.asr_provider.as_str() {
        "openai" => Ok(Box::new(OpenAiProvider::new(
            required_key(
                &config.openai_api_key,
                "OpenAI API key 未配置，请先到设置页填写",
            )?,
            config.asr_model.clone(),
        ))),
        "doubao_stream" => Ok(Box::new(DoubaoStreamingProvider::new(
            doubao_stream_config(config)?,
        ))),
        "glm" => Ok(Box::new(GlmProvider::new(
            required_key(&config.glm_endpoint, "GLM endpoint 未配置")?,
            required_key(&config.glm_api_key, "GLM API key 未配置，请先到设置页填写")?,
            config.asr_model.clone(),
        ))),
        "aliyun_fun" => Ok(Box::new(AliyunProvider::new(
            required_key(&config.aliyun_endpoint, "通义 endpoint 未配置")?,
            required_key(
                &config.aliyun_api_key,
                "通义 DashScope API key 未配置，请先到设置页填写",
            )?,
            config.asr_model.clone(),
        ))),
        "stepfun" => Ok(Box::new(StepFunProvider::new(
            required_key(&config.stepfun_endpoint, "StepFun endpoint 未配置")?,
            required_key(
                &config.stepfun_api_key,
                "StepFun API key 未配置，请先到设置页填写",
            )?,
            config.asr_model.clone(),
        ))),
        other => Err(AppError::Internal(format!(
            "unsupported ASR provider: {other}"
        ))),
    }
}

fn doubao_stream_config(config: &TranscriptionConfig) -> AppResult<DoubaoStreamConfig> {
    let endpoint = required_key(&config.doubao_endpoint, "Doubao endpoint 未配置")?;
    let resource_id = required_key(&config.doubao_resource_id, "Doubao resource_id 未配置")?;
    let api_key_or_access_token = required_key(
        &config.doubao_api_key_or_access_token,
        "Doubao API Key / Access Token 未配置，请先到设置页填写",
    )?;
    let app_id = config.doubao_app_id.clone().unwrap_or_default();

    Ok(DoubaoStreamConfig {
        endpoint,
        auth: DoubaoAuth::from_settings(app_id, api_key_or_access_token),
        resource_id,
    })
}

#[cfg(test)]
mod tests;
