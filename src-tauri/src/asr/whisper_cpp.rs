// Local whisper.cpp inference over a user-supplied GGML model (whisper-rs 0.16,
// metal backend on macOS). Batch-only: the caller already runs `transcribe` on a
// dedicated std::thread (lib.rs spawn_transcription), so synchronous inference is
// correct here — no spawn_blocking / tokio runtime needed.
//
// This slice loads-and-releases the context on every call (no resident cache);
// context caching / idle unload is deferred to P3 model management.

use std::path::Path;

use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperError,
};

use crate::asr::{downmix_to_mono, resample_linear, AsrProvider, AudioData, PCM16_TARGET_RATE};
use crate::error::{AppError, AppResult};

pub struct WhisperCppProvider {
    /// Absolute path to a GGML model file (e.g. ggml-base.bin). User-supplied via
    /// Settings.whisper_cpp_model_path; build_provider already rejects an empty path.
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

    fn transcribe(&self, audio: &AudioData) -> AppResult<String> {
        // Pre-check the file before handing it to whisper.cpp: a missing path is a
        // recoverable user mistake (§3.7 Device), not an engine fault. Gives a clear
        // Chinese message instead of an opaque init failure.
        if !Path::new(&self.model_path).exists() {
            return Err(AppError::Device(format!(
                "模型文件不存在：{}",
                self.model_path
            )));
        }

        // whisper.cpp only accepts 16 kHz mono f32 in [-1, 1]. Reuse the shared
        // downmix + linear resample (asr/mod.rs) — same "→16k mono" need as the
        // cloud adapters. No PCM16 encode step: whisper consumes f32 directly.
        let mono = downmix_to_mono(&audio.samples, audio.channels);
        let samples = resample_linear(&mono, audio.sample_rate, PCM16_TARGET_RATE)?;

        // Empty input → empty transcript; upstream silence detection (lib.rs) already
        // intercepts digital silence before we get here.
        if samples.is_empty() {
            return Ok(String::new());
        }

        // Bad / non-GGML / version-mismatched model surfaces as Provider: the file
        // itself is wrong, the user must re-download (not recoverable by retry).
        let ctx =
            WhisperContext::new_with_params(&self.model_path, WhisperContextParameters::default())
                .map_err(provider_err("加载 Whisper 模型失败"))?;

        let mut state = ctx
            .create_state()
            .map_err(provider_err("创建 Whisper 推理状态失败"))?;

        // Greedy/best_of=1 is plenty for dictation; beam search is slower for no
        // meaningful gain on short utterances.
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(inference_threads());
        // Audie Non-Goal: never translate; language=None auto-detects.
        params.set_translate(false);
        params.set_language(None);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        state
            .full(params, &samples)
            .map_err(provider_err("Whisper 推理失败"))?;

        // full_n_segments returns a plain count in 0.16; concatenate each segment's
        // text. to_str_lossy avoids hard-failing on the rare non-UTF8 byte from the
        // decoder (better a slightly mangled char than a dropped transcript).
        let segment_count = state.full_n_segments();
        let mut text = String::new();
        for index in 0..segment_count {
            let Some(segment) = state.get_segment(index) else {
                continue;
            };
            let chunk = segment
                .to_str_lossy()
                .map_err(provider_err("读取 Whisper 转写结果失败"))?;
            text.push_str(&chunk);
        }

        Ok(text.trim().to_string())
    }
}

/// Map a whisper-rs error to a §3.7 Provider error with a Chinese context prefix.
fn provider_err(context: &'static str) -> impl Fn(WhisperError) -> AppError {
    move |err| AppError::Provider(format!("{context}：{err}"))
}

/// Conservative thread count: available parallelism capped at 8. Under the metal
/// backend the GPU does the heavy lifting so n_threads has little effect; this
/// just avoids over-subscribing on CPU-fallback paths.
fn inference_threads() -> std::os::raw::c_int {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    cores.clamp(1, 8) as std::os::raw::c_int
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_model_path_is_device_error() {
        let provider = WhisperCppProvider::new("/nonexistent/ggml-base.bin".into());
        let audio = AudioData {
            samples: vec![0.1, -0.1, 0.2],
            sample_rate: 16_000,
            channels: 1,
        };

        let err = provider
            .transcribe(&audio)
            .expect_err("missing model must fail");
        assert!(matches!(err, AppError::Device(_)));
        assert!(err.message().contains("模型文件不存在"));
    }

    #[test]
    fn empty_model_path_is_device_error() {
        // build_provider normally rejects an empty path, but defend here too: an
        // empty path can't exist, so it must be a Device error, never a crash.
        let provider = WhisperCppProvider::new(String::new());
        let audio = AudioData {
            samples: vec![0.1, -0.1],
            sample_rate: 16_000,
            channels: 1,
        };

        let err = provider
            .transcribe(&audio)
            .expect_err("empty path must fail");
        assert!(matches!(err, AppError::Device(_)));
    }

    #[test]
    fn inference_threads_within_bounds() {
        let n = inference_threads();
        assert!((1..=8).contains(&n));
    }
}
