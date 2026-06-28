// TranscriptionManager — owns the active AsrProvider and turns buffered audio
// into text. PROJECT_SPEC.md §6.1. P0 hard-wires Groq; P1 makes the provider
// selectable from settings.
// This manager only chooses and calls an ASR adapter; it does not emit UI events,
// mutate app state, or decide fallback behavior. Those decisions stay in lib.rs.

use crate::asr::aliyun::client::AliyunProvider;
use crate::asr::doubao::client::{DoubaoAuth, DoubaoStreamConfig, DoubaoStreamingProvider};
use crate::asr::glm::GlmProvider;
use crate::asr::groq::GroqProvider;
#[cfg(target_os = "macos")]
use crate::asr::macos_native::MacosNativeProvider;
use crate::asr::openai::OpenAiProvider;
use crate::asr::stepfun::client::StepFunProvider;
use crate::asr::whisper_cpp::WhisperCppProvider;
use crate::asr::{AsrProvider, AudioChunkStream, AudioData, TranscriptStream};
use crate::error::{AppError, AppResult};

pub struct TranscriptionManager;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TranscriptionConfig {
    pub asr_provider: String,
    /// Selected ASR model id; empty = adapter built-in default. Doubao ignores it.
    pub asr_model: String,
    pub groq_api_key: Option<String>,
    pub openai_api_key: Option<String>,
    pub whisper_cpp_model_path: Option<String>,
    pub doubao_endpoint: Option<String>,
    pub doubao_resource_id: Option<String>,
    pub doubao_app_id: Option<String>,
    pub doubao_api_key_or_access_token: Option<String>,
    pub glm_api_key: Option<String>,
    pub aliyun_api_key: Option<String>,
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
        "groq" => Ok(Box::new(GroqProvider::new(
            required_key(
                &config.groq_api_key,
                "Groq API key 未配置，请先到设置页填写",
            )?,
            config.asr_model.clone(),
        ))),
        "openai" => Ok(Box::new(OpenAiProvider::new(
            required_key(
                &config.openai_api_key,
                "OpenAI API key 未配置，请先到设置页填写",
            )?,
            config.asr_model.clone(),
        ))),
        "whisper_cpp" => match config
            .whisper_cpp_model_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(model_path) => Ok(Box::new(WhisperCppProvider::new(model_path.to_string()))),
            None => Err(AppError::Device("未配置本地 Whisper 模型".into())),
        },
        "doubao_stream" => Ok(Box::new(DoubaoStreamingProvider::new(
            doubao_stream_config(config)?,
        ))),
        "glm" => Ok(Box::new(GlmProvider::new(
            required_key(&config.glm_api_key, "GLM API key 未配置，请先到设置页填写")?,
            config.asr_model.clone(),
        ))),
        "aliyun_fun" => Ok(Box::new(AliyunProvider::new(
            required_key(
                &config.aliyun_api_key,
                "通义 DashScope API key 未配置，请先到设置页填写",
            )?,
            config.asr_model.clone(),
        ))),
        "stepfun" => Ok(Box::new(StepFunProvider::new(
            required_key(
                &config.stepfun_api_key,
                "StepFun API key 未配置，请先到设置页填写",
            )?,
            config.asr_model.clone(),
        ))),
        // macOS on-device dictation: keyless, OS-managed model. The authorization /
        // on-device-availability guards live in the provider's transcribe (they need
        // a live recognizer), so construction here is unconditional on macOS.
        #[cfg(target_os = "macos")]
        "macos_native" => Ok(Box::new(MacosNativeProvider::new())),
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
mod tests {
    use super::*;

    #[test]
    fn openai_requires_keychain_secret() {
        let config = TranscriptionConfig {
            asr_provider: "openai".into(),
            asr_model: String::new(),
            groq_api_key: Some("groq-key".into()),
            openai_api_key: None,
            whisper_cpp_model_path: None,
            doubao_endpoint: None,
            doubao_resource_id: None,
            doubao_app_id: None,
            doubao_api_key_or_access_token: None,
            glm_api_key: None,
            aliyun_api_key: None,
            stepfun_api_key: None,
        };

        let err = match build_provider(&config) {
            Ok(_) => panic!("expected OpenAI without key to fail"),
            Err(err) => err,
        };

        assert!(matches!(err, AppError::Provider(_)));
        assert_eq!(err.message(), "OpenAI API key 未配置，请先到设置页填写");
    }

    #[test]
    fn groq_requires_keychain_secret() {
        let config = TranscriptionConfig {
            asr_provider: "groq".into(),
            asr_model: String::new(),
            groq_api_key: None,
            openai_api_key: Some("openai-key".into()),
            whisper_cpp_model_path: None,
            doubao_endpoint: None,
            doubao_resource_id: None,
            doubao_app_id: None,
            doubao_api_key_or_access_token: None,
            glm_api_key: None,
            aliyun_api_key: None,
            stepfun_api_key: None,
        };

        let err = match build_provider(&config) {
            Ok(_) => panic!("expected Groq without key to fail"),
            Err(err) => err,
        };

        assert!(matches!(err, AppError::Provider(_)));
        assert_eq!(err.message(), "Groq API key 未配置，请先到设置页填写");
    }

    #[test]
    fn whisper_cpp_without_model_path_is_device_error() {
        let config = TranscriptionConfig {
            asr_provider: "whisper_cpp".into(),
            asr_model: String::new(),
            groq_api_key: None,
            openai_api_key: None,
            whisper_cpp_model_path: None,
            doubao_endpoint: None,
            doubao_resource_id: None,
            doubao_app_id: None,
            doubao_api_key_or_access_token: None,
            glm_api_key: None,
            aliyun_api_key: None,
            stepfun_api_key: None,
        };

        let err = match build_provider(&config) {
            Ok(_) => panic!("expected WhisperCpp without model path to fail"),
            Err(err) => err,
        };

        assert!(matches!(err, AppError::Device(_)));
        assert_eq!(err.message(), "未配置本地 Whisper 模型");
    }

    #[test]
    fn whisper_cpp_with_missing_model_path_is_device_error() {
        // whisper_cpp now does real inference; a configured-but-missing model file
        // is a recoverable user mistake (Device), not an engine fault. build_provider
        // succeeds (path is non-empty); the missing file surfaces at transcribe time.
        let config = TranscriptionConfig {
            asr_provider: "whisper_cpp".into(),
            asr_model: String::new(),
            groq_api_key: None,
            openai_api_key: None,
            whisper_cpp_model_path: Some("/nonexistent/ggml-base.bin".into()),
            doubao_endpoint: None,
            doubao_resource_id: None,
            doubao_app_id: None,
            doubao_api_key_or_access_token: None,
            glm_api_key: None,
            aliyun_api_key: None,
            stepfun_api_key: None,
        };

        let provider = build_provider(&config).expect("non-empty model path must build");
        let err = provider
            .transcribe(&AudioData {
                samples: vec![0.0],
                sample_rate: 16_000,
                channels: 1,
            })
            .expect_err("missing model file must fail");

        assert!(matches!(err, AppError::Device(_)));
        assert!(err.message().contains("模型文件不存在"));
    }

    #[test]
    fn groq_with_selected_model_builds_provider() {
        // A non-empty asr_model must not break construction (model flows into the
        // adapter); with a key present, build_provider succeeds.
        let config = TranscriptionConfig {
            asr_provider: "groq".into(),
            asr_model: "whisper-large-v3".into(),
            groq_api_key: Some("groq-key".into()),
            openai_api_key: None,
            whisper_cpp_model_path: None,
            doubao_endpoint: None,
            doubao_resource_id: None,
            doubao_app_id: None,
            doubao_api_key_or_access_token: None,
            glm_api_key: None,
            aliyun_api_key: None,
            stepfun_api_key: None,
        };

        assert!(build_provider(&config).is_ok());
    }

    #[test]
    fn manager_batch_transcribe_still_uses_existing_provider_errors() {
        let manager = TranscriptionManager::new();
        let audio = AudioData {
            samples: vec![0.0],
            sample_rate: 16_000,
            channels: 1,
        };
        let config = TranscriptionConfig {
            asr_provider: "groq".into(),
            asr_model: String::new(),
            groq_api_key: None,
            openai_api_key: None,
            whisper_cpp_model_path: None,
            doubao_endpoint: None,
            doubao_resource_id: None,
            doubao_app_id: None,
            doubao_api_key_or_access_token: None,
            glm_api_key: None,
            aliyun_api_key: None,
            stepfun_api_key: None,
        };

        let err = manager.transcribe(&audio, &config).unwrap_err();

        assert!(matches!(err, AppError::Provider(_)));
        assert_eq!(err.message(), "Groq API key 未配置，请先到设置页填写");
    }

    #[test]
    fn manager_stream_transcribe_exposes_p2_contract_without_provider_impl() {
        let manager = TranscriptionManager::new();
        let (chunks_tx, chunks_rx) = std::sync::mpsc::channel();
        chunks_tx
            .send(Ok(crate::asr::AudioChunk {
                samples: vec![0.0],
                sample_rate: 16_000,
                channels: 1,
                sequence: 1,
                is_final: true,
            }))
            .unwrap();
        drop(chunks_tx);
        let config = TranscriptionConfig {
            asr_provider: "groq".into(),
            asr_model: String::new(),
            groq_api_key: Some("groq-key".into()),
            openai_api_key: None,
            whisper_cpp_model_path: None,
            doubao_endpoint: None,
            doubao_resource_id: None,
            doubao_app_id: None,
            doubao_api_key_or_access_token: None,
            glm_api_key: None,
            aliyun_api_key: None,
            stepfun_api_key: None,
        };

        let err = manager.transcribe_stream(chunks_rx, &config).unwrap_err();

        assert!(matches!(err, AppError::Internal(_)));
        assert_eq!(
            err.message(),
            "streaming ASR is not implemented for this provider"
        );
    }

    #[test]
    fn doubao_stream_requires_token_for_streaming_provider() {
        let config = TranscriptionConfig {
            asr_provider: "doubao_stream".into(),
            asr_model: String::new(),
            groq_api_key: None,
            openai_api_key: None,
            whisper_cpp_model_path: None,
            doubao_endpoint: Some("wss://example.test".into()),
            doubao_resource_id: Some("resource".into()),
            doubao_app_id: None,
            doubao_api_key_or_access_token: None,
            glm_api_key: None,
            aliyun_api_key: None,
            stepfun_api_key: None,
        };

        let err = match build_provider(&config) {
            Ok(_) => panic!("expected Doubao stream without token to fail"),
            Err(err) => err,
        };

        assert!(matches!(err, AppError::Provider(_)));
        assert_eq!(
            err.message(),
            "Doubao API Key / Access Token 未配置，请先到设置页填写"
        );
    }
}
