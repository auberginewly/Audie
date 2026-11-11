use super::buffer::{process_f32, process_i16, process_u16};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use parking_lot::Mutex;
use serde::Serialize;

use super::{AudioMeta, ChunkSink, LevelAccum, RetainedSamples, SHUTDOWN_POLL_MS};
use crate::asr::AudioChunk;
use crate::error::{AppError, AppResult};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};

/// How a capture thread opens and consumes the input device for one session.
pub(super) struct CapturePlan {
    /// Explicit device name to use, or `None` to fall back to the host default.
    pub(super) preferred_name: Option<String>,
    /// The name came from an explicit user pick, so a miss is a Device error
    /// rather than a silent fall back to the default.
    pub(super) explicit: bool,
    /// Track levels only, don't retain samples (the Settings mic preview).
    pub(super) level_only: bool,
}

pub(super) struct CaptureThreadArgs {
    pub(super) shutdown: Arc<AtomicBool>,
    pub(super) accum: Arc<Mutex<LevelAccum>>,
    pub(super) buffer: Arc<Mutex<Vec<f32>>>,
    pub(super) retained: Arc<RetainedSamples>,
    pub(super) chunk_tx: Option<SyncSender<AppResult<AudioChunk>>>,
    pub(super) streaming_queue_overflowed: Option<Arc<AtomicBool>>,
    pub(super) plan: CapturePlan,
    pub(super) ready_tx: mpsc::Sender<AppResult<AudioMeta>>,
}

pub(super) fn run_capture_thread(args: CaptureThreadArgs) {
    let CaptureThreadArgs {
        shutdown,
        accum,
        buffer,
        retained,
        chunk_tx,
        streaming_queue_overflowed,
        plan,
        ready_tx,
    } = args;

    let host = cpal::default_host();
    let device = match resolve_input_device(&host, plan.preferred_name.as_deref(), plan.explicit) {
        Some(d) => d,
        None => {
            // An explicit user pick that can't be resolved gets a specific message
            // (SPEC §3.7 Device); the automatic path keeps the generic one.
            let msg = match (&plan.preferred_name, plan.explicit) {
                (Some(name), true) => {
                    format!("所选麦克风「{name}」不可用，请到设置重新选择")
                }
                _ => "no default input device".into(),
            };
            let _ = ready_tx.send(Err(AppError::Device(msg)));
            return;
        }
    };
    let level_only = plan.level_only;

    let supported = match device.default_input_config() {
        Ok(c) => c,
        Err(e) => {
            // TCC permission is gated upstream by `ensure_microphone_permission`
            // before we ever reach here, so anything cpal surfaces at this point
            // is a real device problem (busy, disconnected, unsupported format).
            let _ = ready_tx.send(Err(AppError::Device(format!("default_input_config: {e}"))));
            return;
        }
    };

    let sample_format = supported.sample_format();
    let config: StreamConfig = supported.into();
    let meta = AudioMeta {
        sample_rate: config.sample_rate.0,
        channels: config.channels,
    };
    // Build the optional live PCM outlet now that we know the stream format.
    let chunk_sink = chunk_tx.map(|tx| {
        Arc::new(ChunkSink {
            tx,
            overflowed: streaming_queue_overflowed
                .unwrap_or_else(|| Arc::new(AtomicBool::new(false))),
            sequence: AtomicU64::new(0),
            sample_rate: meta.sample_rate,
            channels: meta.channels,
        })
    });
    let err_fn = |err| log::error!("cpal stream error: {err}");

    let build_result = match sample_format {
        SampleFormat::F32 => {
            let (a, b, r, s) = (
                accum.clone(),
                buffer.clone(),
                retained.clone(),
                chunk_sink.clone(),
            );
            device.build_input_stream(
                &config,
                move |data: &[f32], _| process_f32(&a, &b, s.as_deref(), &r, data, level_only),
                err_fn,
                None,
            )
        }
        SampleFormat::I16 => {
            let (a, b, r, s) = (
                accum.clone(),
                buffer.clone(),
                retained.clone(),
                chunk_sink.clone(),
            );
            device.build_input_stream(
                &config,
                move |data: &[i16], _| process_i16(&a, &b, s.as_deref(), &r, data, level_only),
                err_fn,
                None,
            )
        }
        SampleFormat::U16 => {
            let (a, b, r, s) = (
                accum.clone(),
                buffer.clone(),
                retained.clone(),
                chunk_sink.clone(),
            );
            device.build_input_stream(
                &config,
                move |data: &[u16], _| process_u16(&a, &b, s.as_deref(), &r, data, level_only),
                err_fn,
                None,
            )
        }
        other => {
            let _ = ready_tx.send(Err(AppError::Device(format!(
                "unsupported sample format: {other:?}"
            ))));
            return;
        }
    };

    let stream = match build_result {
        Ok(s) => s,
        Err(e) => {
            let _ = ready_tx.send(Err(AppError::Device(format!("build_input_stream: {e}"))));
            return;
        }
    };

    if let Err(e) = stream.play() {
        let _ = ready_tx.send(Err(AppError::Device(format!("stream.play: {e}"))));
        return;
    }

    let _ = ready_tx.send(Ok(meta));

    while !shutdown.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(SHUTDOWN_POLL_MS));
    }
    drop(stream);
    // Stream is stopped — no more `forward` calls. Tell the streaming consumer the
    // input is done so its recv loop ends now (see `ChunkSink::finish`). No-op on
    // the batch path, where `chunk_sink` is None.
    if let Some(sink) = chunk_sink.as_deref() {
        sink.finish();
    }
}

/// Look up `preferred_name` in the host's input device list. With
/// `require_exact = false` (the P0.7 auto path) a miss falls back to the host
/// default; with `require_exact = true` (an explicit Settings pick) a miss
/// returns `None` so the caller can raise a Device error. cpal's `name()` is
/// fallible per device — skip the ones that error.
fn resolve_input_device(
    host: &cpal::Host,
    preferred_name: Option<&str>,
    require_exact: bool,
) -> Option<cpal::Device> {
    if let Some(name) = preferred_name {
        if let Ok(devices) = host.input_devices() {
            for d in devices {
                if d.name().ok().as_deref() == Some(name) {
                    log::info!("using preferred input device: {name}");
                    return Some(d);
                }
            }
            log::warn!("preferred input device {name:?} not found via cpal");
        }
        // An explicit pick (Settings) that doesn't resolve must error rather than
        // silently fall back to the default. The P0.7 auto path passes
        // `require_exact = false` and keeps falling back.
        if require_exact {
            return None;
        }
    }
    host.default_input_device()
}

/// One enumerable input device for the Settings picker. `id` doubles as the
/// match key — it's the cpal `device.name()` that `resolve_input_device` looks
/// up later, so the picked value round-trips back to the same device.
#[derive(Serialize, Clone)]
pub struct MicrophoneInfo {
    pub id: String,
    pub label: String,
}

/// Enumerate input devices for the Settings picker. Names come from the same
/// cpal host `resolve_input_device` uses, so a picked name resolves back. Run
/// off the event loop by the caller (cpal enumeration can block).
pub fn list_input_devices() -> Vec<MicrophoneInfo> {
    let host = cpal::default_host();
    let Ok(devices) = host.input_devices() else {
        return Vec::new();
    };
    devices
        .filter_map(|d| d.name().ok())
        .map(|name| MicrophoneInfo {
            id: name.clone(),
            label: name,
        })
        .collect()
}

/// Name of the device the automatic path would open right now: P0.7's override
/// when present (`preferred`), else cpal's default input device. Lets the picker
/// name the "自动" row instead of leaving it opaque.
pub fn auto_input_device_name(preferred: Option<String>) -> Option<String> {
    preferred.or_else(|| {
        cpal::default_host()
            .default_input_device()
            .and_then(|d| d.name().ok())
    })
}
