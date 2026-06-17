// Whisper.cpp adapter skeleton. P1.4 deliberately does not link whisper.cpp or
// whisper-rs; local inference/model management is deferred to P3.

use crate::asr::{AsrProvider, AudioData};
use crate::error::{AppError, AppResult};

pub struct WhisperCppProvider {
    model_path: String,
}

impl WhisperCppProvider {
    pub fn new(model_path: String) -> Self {
        Self { model_path }
    }
}

impl AsrProvider for WhisperCppProvider {
    fn name(&self) -> &str {
        "whisper_cpp"
    }

    fn transcribe(&self, _audio: &AudioData) -> AppResult<String> {
        let _ = &self.model_path;
        Err(AppError::Device(
            "本地 Whisper 推理将在 P3 模型管理中接入".into(),
        ))
    }
}
