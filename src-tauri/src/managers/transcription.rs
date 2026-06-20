// TranscriptionManager — owns the active AsrProvider and turns buffered audio
// into text. PROJECT_SPEC.md §6.1. P0 hard-wires Groq; P1 makes the provider
// selectable from settings.
// This manager only chooses and calls an ASR adapter; it does not emit UI events,
// mutate app state, or decide fallback behavior. Those decisions stay in lib.rs.

use crate::asr::groq::GroqProvider;
use crate::asr::openai::OpenAiProvider;
use crate::asr::whisper_cpp::WhisperCppProvider;
use crate::asr::{AsrProvider, AudioChunkStream, AudioData, TranscriptStream};
use crate::error::{AppError, AppResult};

pub struct TranscriptionManager;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TranscriptionConfig {
    pub asr_provider: String,
    pub groq_api_key: Option<String>,
    pub openai_api_key: Option<String>,
    pub whisper_cpp_model_path: Option<String>,
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
        "groq" => Ok(Box::new(GroqProvider::new(required_key(
            &config.groq_api_key,
            "Groq API key 未配置，请先到设置页填写",
        )?))),
        "openai" => Ok(Box::new(OpenAiProvider::new(required_key(
            &config.openai_api_key,
            "OpenAI API key 未配置，请先到设置页填写",
        )?))),
        "whisper_cpp" => match config
            .whisper_cpp_model_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(model_path) => Ok(Box::new(WhisperCppProvider::new(model_path.to_string()))),
            None => Err(AppError::Device("未配置本地 Whisper 模型".into())),
        },
        other => Err(AppError::Internal(format!(
            "unsupported ASR provider: {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_requires_keychain_secret() {
        let config = TranscriptionConfig {
            asr_provider: "openai".into(),
            groq_api_key: Some("groq-key".into()),
            openai_api_key: None,
            whisper_cpp_model_path: None,
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
            groq_api_key: None,
            openai_api_key: Some("openai-key".into()),
            whisper_cpp_model_path: None,
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
            groq_api_key: None,
            openai_api_key: None,
            whisper_cpp_model_path: None,
        };

        let err = match build_provider(&config) {
            Ok(_) => panic!("expected WhisperCpp without model path to fail"),
            Err(err) => err,
        };

        assert!(matches!(err, AppError::Device(_)));
        assert_eq!(err.message(), "未配置本地 Whisper 模型");
    }

    #[test]
    fn whisper_cpp_with_model_path_still_defers_local_inference_to_p3() {
        let config = TranscriptionConfig {
            asr_provider: "whisper_cpp".into(),
            groq_api_key: None,
            openai_api_key: None,
            whisper_cpp_model_path: Some("/tmp/ggml.bin".into()),
        };

        let err = match build_provider(&config) {
            Ok(provider) => provider.transcribe(&AudioData {
                samples: vec![0.0],
                sample_rate: 16_000,
                channels: 1,
            }),
            Err(err) => Err(err),
        }
        .unwrap_err();

        assert!(matches!(err, AppError::Device(_)));
        assert_eq!(err.message(), "本地 Whisper 推理将在 P3 模型管理中接入");
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
            groq_api_key: None,
            openai_api_key: None,
            whisper_cpp_model_path: None,
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
            groq_api_key: Some("groq-key".into()),
            openai_api_key: None,
            whisper_cpp_model_path: None,
        };

        let err = manager.transcribe_stream(chunks_rx, &config).unwrap_err();

        assert!(matches!(err, AppError::Internal(_)));
        assert_eq!(
            err.message(),
            "streaming ASR is not implemented for this provider"
        );
    }
}
