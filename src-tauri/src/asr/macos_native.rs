// macOS native on-device dictation via SFSpeechRecognizer (decision B1).
//
// Keyless, fully offline: the OS owns the model, so there is nothing to download
// and no API key. We feed the recorded utterance to the recognizer as a single
// AVAudioPCMBuffer (batch, not live streaming) with requiresOnDeviceRecognition,
// then collect the final bestTranscription.formattedString.
//
// Threading: `transcribe` already runs on a dedicated std::thread (lib.rs
// spawn_transcription). SFSpeechRecognizer delivers results to the result-handler
// block on its own background queue, so we hand the final text back over an mpsc
// channel and block this thread on recv_timeout — no run loop to pump here.
//
// This module is macOS-only (objc2-speech). The provider id "macos_native" is
// gated to the macOS build in managers/transcription.rs build_provider.

use std::sync::mpsc;
use std::time::Duration;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::AllocAnyThread;
use objc2_avf_audio::{AVAudioCommonFormat, AVAudioFormat, AVAudioPCMBuffer};
use objc2_foundation::NSError;
use objc2_speech::{
    SFSpeechAudioBufferRecognitionRequest, SFSpeechRecognitionResult, SFSpeechRecognizer,
    SFSpeechRecognizerAuthorizationStatus,
};

use crate::asr::{downmix_to_mono, resample_linear, AsrProvider, AudioData, PCM16_TARGET_RATE};
use crate::error::{AppError, AppResult};

/// Upper bound on how long we wait for the recognizer's final result. On-device
/// recognition of a short utterance returns well within this; the cap just stops a
/// stuck task from hanging the transcription thread forever.
const RECOGNITION_TIMEOUT: Duration = Duration::from_secs(30);

pub struct MacosNativeProvider;

impl MacosNativeProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MacosNativeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AsrProvider for MacosNativeProvider {
    fn name(&self) -> &str {
        "macos_native"
    }

    fn transcribe(&self, audio: &AudioData) -> AppResult<String> {
        // Gate on Speech authorization first: an unauthorized recognizer would fail
        // opaquely. A denial is recoverable by the user (§3.7 Permission).
        // SAFETY: parameterless class method, returns a plain enum.
        let status = unsafe { SFSpeechRecognizer::authorizationStatus() };
        if status != SFSpeechRecognizerAuthorizationStatus::Authorized {
            return Err(AppError::Permission(
                "语音识别权限未授予，请到 系统设置 → 隐私与安全性 → 语音识别 启用 Audie".into(),
            ));
        }

        // SFSpeechRecognizer wants audio buffers; Audie hands us f32 mono-or-stereo.
        // Downmix + resample to 16 kHz mono (the rate the cloud adapters also target).
        let mono = downmix_to_mono(&audio.samples, audio.channels);
        let samples = resample_linear(&mono, audio.sample_rate, PCM16_TARGET_RATE)?;
        if samples.is_empty() {
            return Ok(String::new());
        }

        // SFSpeechRecognizer() with the default locale; init returns nil when the
        // system has no recognizer for the current language at all (§3.7 Device —
        // unrecoverable without changing language).
        // SAFETY: init consumes a fresh allocation and returns the recognizer or nil.
        let recognizer = unsafe { SFSpeechRecognizer::init(SFSpeechRecognizer::alloc()) };
        let recognizer =
            recognizer.ok_or_else(|| AppError::Device("本机听写对当前语言不可用".into()))?;

        // On-device is the whole point (offline, private). If the installed locale
        // can't run on-device, surface Device with the exact wording the task asks for.
        // SAFETY: instance method on a live recognizer.
        if !unsafe { recognizer.supportsOnDeviceRecognition() } {
            return Err(AppError::Device("本机听写对该语言不可用".into()));
        }

        let text = run_recognition(&recognizer, &samples)?;
        Ok(text.trim().to_string())
    }
}

/// Build a 16 kHz mono float32 AVAudioPCMBuffer, append it as the entire utterance,
/// run the recognition task, and block until the final transcription arrives.
fn run_recognition(recognizer: &SFSpeechRecognizer, samples: &[f32]) -> AppResult<String> {
    let buffer = make_pcm_buffer(samples)?;

    // SAFETY: `new` returns a retained request object (the designated way to make
    // an audio-buffer request).
    let request = unsafe { SFSpeechAudioBufferRecognitionRequest::new() };
    // SAFETY: setters on the live request; requiresOnDeviceRecognition keeps audio
    // off Apple's servers. Partial results are off — we only want the final string.
    unsafe {
        request.setRequiresOnDeviceRecognition(true);
        request.setShouldReportPartialResults(false);
        request.appendAudioPCMBuffer(&buffer);
        request.endAudio();
    }

    // The result handler fires on the recognizer's background queue; forward the
    // final text (or an error) over a channel so this thread can block on it.
    let (tx, rx) = mpsc::channel::<AppResult<String>>();
    let handler = RcBlock::new(
        move |result: *mut SFSpeechRecognitionResult, error: *mut NSError| {
            // A non-null error ends the task: report it once. Ignore later callbacks.
            if !error.is_null() {
                // SAFETY: non-null NSError owned by the callback for its duration.
                let message = unsafe { &*error }.localizedDescription().to_string();
                let _ = tx.send(Err(AppError::Provider(format!("本机听写失败：{message}"))));
                return;
            }
            if result.is_null() {
                return;
            }
            // SAFETY: non-null result owned by the callback for its duration.
            let result = unsafe { &*result };
            // Only the final result is committable; partials are disabled anyway.
            if unsafe { result.isFinal() } {
                let text = unsafe { result.bestTranscription().formattedString() }.to_string();
                let _ = tx.send(Ok(text));
            }
        },
    );

    // Keep the task alive for the duration of recognition (dropping it cancels).
    // SAFETY: request + handler outlive the call; the returned task is retained.
    let _task = unsafe { recognizer.recognitionTaskWithRequest_resultHandler(&request, &handler) };

    match rx.recv_timeout(RECOGNITION_TIMEOUT) {
        Ok(result) => result,
        Err(_) => Err(AppError::Provider("本机听写超时，未返回结果".into())),
    }
}

/// Wrap 16 kHz mono float32 `samples` in an AVAudioPCMBuffer the recognizer accepts.
fn make_pcm_buffer(samples: &[f32]) -> AppResult<Retained<AVAudioPCMBuffer>> {
    // A standard (deinterleaved) float32 format at 16 kHz, 1 channel.
    // SAFETY: documented initializer; non-zero sample rate and channel count. Returns
    // nil only on an invalid format spec, which this constant one isn't.
    let format = unsafe {
        AVAudioFormat::initWithCommonFormat_sampleRate_channels_interleaved(
            AVAudioFormat::alloc(),
            AVAudioCommonFormat::PCMFormatFloat32,
            f64::from(PCM16_TARGET_RATE),
            1,
            false,
        )
    }
    .ok_or_else(|| AppError::Internal("无法创建本机听写音频格式".into()))?;

    let frame_count = u32::try_from(samples.len())
        .map_err(|_| AppError::Internal("音频样本数超出本机听写缓冲上限".into()))?;

    // SAFETY: PCM format + a frame capacity that fits the samples; the initializer
    // returns a buffer with capacity for `frame_count` frames (or nil on failure).
    let buffer = unsafe {
        AVAudioPCMBuffer::initWithPCMFormat_frameCapacity(
            AVAudioPCMBuffer::alloc(),
            &format,
            frame_count,
        )
    }
    .ok_or_else(|| AppError::Internal("无法创建本机听写音频缓冲".into()))?;

    // floatChannelData yields one pointer per channel (mono → channel 0). Copy our
    // samples in, then set frameLength so the buffer reports the right length.
    // SAFETY: float32 format guarantees floatChannelData is non-null; we write exactly
    // `frame_count` contiguous samples into channel 0's `frame_count`-capacity region.
    unsafe {
        let channels = buffer.floatChannelData();
        if channels.is_null() {
            return Err(AppError::Internal("无法访问本机听写音频缓冲".into()));
        }
        let channel0 = (*channels).as_ptr();
        std::ptr::copy_nonoverlapping(samples.as_ptr(), channel0, samples.len());
        buffer.setFrameLength(frame_count);
    }

    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Provider construction must never touch the system (no recognizer, no auth) —
    // building it is cheap and side-effect-free; the system calls happen in transcribe.
    #[test]
    fn provider_constructs_without_system_calls() {
        let provider = MacosNativeProvider::new();
        assert_eq!(provider.name(), "macos_native");
    }

    // The unavailable path is exercised through transcribe's first gate: in CI the
    // Speech framework is unauthorized (no NSSpeechRecognitionUsageDescription
    // prompt was answered), so transcribe returns a recoverable error (Permission
    // when unauthorized, or Device when on-device recognition is unsupported) rather
    // than panicking. Either recoverable outcome is acceptable; a non-recoverable
    // Internal/Provider crash-equivalent is not.
    #[test]
    fn transcribe_unavailable_is_recoverable_not_panic() {
        let provider = MacosNativeProvider::new();
        let audio = AudioData {
            samples: vec![0.1, -0.1, 0.2, -0.2],
            sample_rate: 16_000,
            channels: 1,
        };

        match provider.transcribe(&audio) {
            // Authorized + on-device available on the dev box: an empty/garbage clip
            // can still return Ok("") — that's fine, it didn't crash.
            Ok(_) => {}
            Err(err) => assert!(
                matches!(err, AppError::Permission(_) | AppError::Device(_)),
                "unavailable native ASR must be recoverable, got: {err:?}"
            ),
        }
    }
}
