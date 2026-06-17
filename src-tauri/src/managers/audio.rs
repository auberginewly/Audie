// AudioManager — captures the default input device. It does two jobs:
//   1. emits `audio-level` events (~30 FPS peak) to drive the overlay waveform,
//   2. accumulates raw samples so `stop_capture` can hand the whole utterance
//      to the transcription pipeline. PROJECT_SPEC.md §3.6 / §6.1.
//
// cpal's `Stream` is `!Send`, so capture lives on a dedicated thread that owns
// the stream from creation to drop. A second emitter thread snapshots the
// running peak every ~33ms. Samples are pushed into a shared buffer from the
// cpal callback and drained on stop.
//
// Mirrors Handy's pattern: cpal default host, blocking thread, atomic shutdown.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::asr::AudioData;
use crate::error::{AppError, AppResult};

const AUDIO_LEVEL_EVENT: &str = "audio-level";
const EMIT_INTERVAL_MS: u64 = 33;
const SHUTDOWN_POLL_MS: u64 = 10;

#[derive(Serialize, Clone, Copy)]
struct AudioLevelPayload {
    level: f32,
}

#[derive(Default)]
struct LevelAccum {
    peak: f32,
}

/// Format of the active capture, learned once the stream opens. Needed to write
/// a correct WAV header at stop time.
#[derive(Clone, Copy, Default)]
struct AudioMeta {
    sample_rate: u32,
    channels: u16,
}

struct CaptureSession {
    shutdown: Arc<AtomicBool>,
    capture_thread: Option<JoinHandle<()>>,
    emit_thread: Option<JoinHandle<()>>,
    buffer: Arc<Mutex<Vec<f32>>>,
    meta: AudioMeta,
}

pub struct AudioManager {
    session: Mutex<Option<CaptureSession>>,
}

impl AudioManager {
    pub fn new() -> Self {
        Self {
            session: Mutex::new(None),
        }
    }

    /// Open the default input device and start streaming peak levels while
    /// buffering samples. Idempotent: a redundant call while a session is live
    /// logs a warn and returns Ok.
    pub fn start_capture(&self, app: AppHandle) -> AppResult<()> {
        let mut guard = self.session.lock();
        if guard.is_some() {
            log::warn!("start_capture called while a session is active; ignoring");
            return Ok(());
        }

        let shutdown = Arc::new(AtomicBool::new(false));
        let accum = Arc::new(Mutex::new(LevelAccum::default()));
        let buffer = Arc::new(Mutex::new(Vec::<f32>::new()));

        // Spawn the capture thread and wait for stream setup to either succeed
        // (reporting the stream's format) or fail. cpal's `Stream` is `!Send`,
        // so it must be created and parked on the same thread.
        let (ready_tx, ready_rx) = mpsc::channel::<AppResult<AudioMeta>>();
        let shutdown_cap = shutdown.clone();
        let accum_cap = accum.clone();
        let buffer_cap = buffer.clone();
        let capture_thread = thread::Builder::new()
            .name("audie-audio-capture".into())
            .spawn(move || {
                run_capture_thread(shutdown_cap, accum_cap, buffer_cap, ready_tx);
            })
            .map_err(|e| AppError::Device(format!("spawn capture thread: {e}")))?;

        let meta = match ready_rx.recv() {
            Ok(Ok(meta)) => meta,
            Ok(Err(err)) => {
                let _ = capture_thread.join();
                return Err(err);
            }
            Err(_) => {
                let _ = capture_thread.join();
                return Err(AppError::Internal(
                    "capture thread exited before reporting readiness".into(),
                ));
            }
        };

        let shutdown_emit = shutdown.clone();
        let accum_emit = accum.clone();
        let app_emit = app.clone();
        let emit_thread = match thread::Builder::new()
            .name("audie-audio-emit".into())
            .spawn(move || {
                run_emit_thread(app_emit, accum_emit, shutdown_emit);
            }) {
            Ok(t) => t,
            Err(e) => {
                // Roll back the capture thread we just started so we don't leak it.
                shutdown.store(true, Ordering::Relaxed);
                let _ = capture_thread.join();
                return Err(AppError::Internal(format!("spawn emit thread: {e}")));
            }
        };

        *guard = Some(CaptureSession {
            shutdown,
            capture_thread: Some(capture_thread),
            emit_thread: Some(emit_thread),
            buffer,
            meta,
        });

        log::info!(
            "audio capture started ({} Hz, {} ch)",
            meta.sample_rate,
            meta.channels
        );
        Ok(())
    }

    /// Signal both threads to exit, join them, and return the buffered utterance.
    /// Errors if no capture is active.
    pub fn stop_capture(&self) -> AppResult<AudioData> {
        let mut session = match self.session.lock().take() {
            Some(s) => s,
            None => {
                return Err(AppError::Device(
                    "stop_capture with no active session".into(),
                ))
            }
        };

        session.shutdown.store(true, Ordering::Relaxed);

        if let Some(t) = session.capture_thread.take() {
            if let Err(e) = t.join() {
                log::warn!("capture thread panicked: {e:?}");
            }
        }
        if let Some(t) = session.emit_thread.take() {
            if let Err(e) = t.join() {
                log::warn!("emit thread panicked: {e:?}");
            }
        }

        // Threads are joined, so the cpal callback can no longer touch the buffer.
        let samples = std::mem::take(&mut *session.buffer.lock());
        log::info!("audio capture stopped ({} samples)", samples.len());

        Ok(AudioData {
            samples,
            sample_rate: session.meta.sample_rate,
            channels: session.meta.channels,
        })
    }
}

impl Default for AudioManager {
    fn default() -> Self {
        Self::new()
    }
}

fn run_capture_thread(
    shutdown: Arc<AtomicBool>,
    accum: Arc<Mutex<LevelAccum>>,
    buffer: Arc<Mutex<Vec<f32>>>,
    ready_tx: mpsc::Sender<AppResult<AudioMeta>>,
) {
    let host = cpal::default_host();
    let device = match host.default_input_device() {
        Some(d) => d,
        None => {
            let _ = ready_tx.send(Err(AppError::Device("no default input device".into())));
            return;
        }
    };

    let supported = match device.default_input_config() {
        Ok(c) => c,
        Err(e) => {
            // cpal surfaces the macOS permission denial here as a device error.
            // P0.6 will classify this into AppError::Permission via the system API.
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
    let err_fn = |err| log::error!("cpal stream error: {err}");

    let build_result = match sample_format {
        SampleFormat::F32 => {
            let (a, b) = (accum.clone(), buffer.clone());
            device.build_input_stream(
                &config,
                move |data: &[f32], _| process_f32(&a, &b, data),
                err_fn,
                None,
            )
        }
        SampleFormat::I16 => {
            let (a, b) = (accum.clone(), buffer.clone());
            device.build_input_stream(
                &config,
                move |data: &[i16], _| process_i16(&a, &b, data),
                err_fn,
                None,
            )
        }
        SampleFormat::U16 => {
            let (a, b) = (accum.clone(), buffer.clone());
            device.build_input_stream(
                &config,
                move |data: &[u16], _| process_u16(&a, &b, data),
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
}

fn run_emit_thread(app: AppHandle, accum: Arc<Mutex<LevelAccum>>, shutdown: Arc<AtomicBool>) {
    while !shutdown.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(EMIT_INTERVAL_MS));
        let level = {
            let mut g = accum.lock();
            let l = g.peak;
            g.peak = 0.0;
            l
        };
        if let Err(e) = app.emit(AUDIO_LEVEL_EVENT, AudioLevelPayload { level }) {
            log::warn!("emit audio-level failed: {e}");
        }
    }
    // Flush a final zero so the UI bars drop instead of freezing on last peak.
    let _ = app.emit(AUDIO_LEVEL_EVENT, AudioLevelPayload { level: 0.0 });
}

// Each `process_*` does one locked pass: track the peak for the waveform and
// append normalized f32 samples for transcription.
fn process_f32(accum: &Mutex<LevelAccum>, buffer: &Mutex<Vec<f32>>, data: &[f32]) {
    let mut peak = 0.0f32;
    {
        let mut buf = buffer.lock();
        buf.reserve(data.len());
        for &s in data {
            let a = s.abs();
            if a > peak {
                peak = a;
            }
            buf.push(s);
        }
    }
    merge_peak(accum, peak);
}

fn process_i16(accum: &Mutex<LevelAccum>, buffer: &Mutex<Vec<f32>>, data: &[i16]) {
    let scale = i16::MAX as f32;
    let mut peak = 0.0f32;
    {
        let mut buf = buffer.lock();
        buf.reserve(data.len());
        for &s in data {
            let v = s as f32 / scale;
            let a = v.abs();
            if a > peak {
                peak = a;
            }
            buf.push(v);
        }
    }
    merge_peak(accum, peak);
}

fn process_u16(accum: &Mutex<LevelAccum>, buffer: &Mutex<Vec<f32>>, data: &[u16]) {
    let mut peak = 0.0f32;
    {
        let mut buf = buffer.lock();
        buf.reserve(data.len());
        for &s in data {
            // u16 silence is centered at 32768.
            let v = (s as f32 - 32768.0) / 32768.0;
            let a = v.abs();
            if a > peak {
                peak = a;
            }
            buf.push(v);
        }
    }
    merge_peak(accum, peak);
}

fn merge_peak(accum: &Mutex<LevelAccum>, local: f32) {
    if local <= 0.0 {
        return;
    }
    let mut g = accum.lock();
    if local > g.peak {
        g.peak = local;
    }
}
