// AudioManager — captures the default input device and emits `audio-level`
// events to drive the overlay waveform. PROJECT_SPEC.md §3.6 / §6.1.
//
// cpal's `Stream` is `!Send`, so capture lives on a dedicated thread that
// owns the stream from creation to drop. A second emitter thread snapshots
// the running peak every ~33ms (≈30 FPS, matching the SPEC).
//
// Mirrors Handy's pattern: cpal default host, blocking thread, atomic
// shutdown flag. No VAD here — that's P0.3.

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

struct CaptureSession {
    shutdown: Arc<AtomicBool>,
    capture_thread: Option<JoinHandle<()>>,
    emit_thread: Option<JoinHandle<()>>,
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

    /// Open the default input device and start streaming peak levels.
    /// Idempotent: a redundant call while a session is live logs a warn and returns Ok.
    pub fn start_capture(&self, app: AppHandle) -> AppResult<()> {
        let mut guard = self.session.lock();
        if guard.is_some() {
            log::warn!("start_capture called while a session is active; ignoring");
            return Ok(());
        }

        let shutdown = Arc::new(AtomicBool::new(false));
        let accum = Arc::new(Mutex::new(LevelAccum::default()));

        // Spawn the capture thread and wait for stream setup to either succeed
        // or fail. cpal's `Stream` is `!Send`, so it must be created and parked
        // on the same thread; the oneshot reports the outcome back.
        let (ready_tx, ready_rx) = mpsc::channel::<AppResult<()>>();
        let shutdown_cap = shutdown.clone();
        let accum_cap = accum.clone();
        let capture_thread = thread::Builder::new()
            .name("audie-audio-capture".into())
            .spawn(move || {
                run_capture_thread(shutdown_cap, accum_cap, ready_tx);
            })
            .map_err(|e| AppError::Device(format!("spawn capture thread: {e}")))?;

        match ready_rx.recv() {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                // Capture thread already returned; nothing to join on success path.
                let _ = capture_thread.join();
                return Err(err);
            }
            Err(_) => {
                let _ = capture_thread.join();
                return Err(AppError::Internal(
                    "capture thread exited before reporting readiness".into(),
                ));
            }
        }

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
        });

        log::info!("audio capture started");
        Ok(())
    }

    /// Signal both threads to exit and join them. Bounded at roughly EMIT_INTERVAL_MS.
    pub fn stop_capture(&self) -> AppResult<()> {
        let mut session = match self.session.lock().take() {
            Some(s) => s,
            None => return Ok(()),
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

        log::info!("audio capture stopped");
        Ok(())
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
    ready_tx: mpsc::Sender<AppResult<()>>,
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
            // P0.7 will classify this into AppError::Permission via the system API.
            let _ = ready_tx.send(Err(AppError::Device(format!("default_input_config: {e}"))));
            return;
        }
    };

    let sample_format = supported.sample_format();
    let config: StreamConfig = supported.into();
    let err_fn = |err| log::error!("cpal stream error: {err}");
    let accum_cb = accum.clone();

    let build_result = match sample_format {
        SampleFormat::F32 => device.build_input_stream(
            &config,
            move |data: &[f32], _| update_peak_f32(&accum_cb, data),
            err_fn,
            None,
        ),
        SampleFormat::I16 => device.build_input_stream(
            &config,
            move |data: &[i16], _| update_peak_i16(&accum_cb, data),
            err_fn,
            None,
        ),
        SampleFormat::U16 => device.build_input_stream(
            &config,
            move |data: &[u16], _| update_peak_u16(&accum_cb, data),
            err_fn,
            None,
        ),
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

    let _ = ready_tx.send(Ok(()));

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

fn update_peak_f32(accum: &Mutex<LevelAccum>, data: &[f32]) {
    let mut local = 0.0f32;
    for &s in data {
        let a = s.abs();
        if a > local {
            local = a;
        }
    }
    merge_peak(accum, local);
}

fn update_peak_i16(accum: &Mutex<LevelAccum>, data: &[i16]) {
    let mut local = 0.0f32;
    let scale = i16::MAX as f32;
    for &s in data {
        let a = (s as f32 / scale).abs();
        if a > local {
            local = a;
        }
    }
    merge_peak(accum, local);
}

fn update_peak_u16(accum: &Mutex<LevelAccum>, data: &[u16]) {
    let mut local = 0.0f32;
    // u16 silence is centered at 32768.
    for &s in data {
        let v = (s as f32 - 32768.0) / 32768.0;
        let a = v.abs();
        if a > local {
            local = a;
        }
    }
    merge_peak(accum, local);
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
