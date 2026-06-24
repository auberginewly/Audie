// AudioManager — captures the default input device. It does two jobs:
//   1. emits `audio-level` events (~30 FPS peak) to drive the overlay waveform,
//   2. accumulates raw samples so `stop_capture` can hand the whole utterance
//      to the transcription pipeline. PROJECT_SPEC.md §3.6 / §6.1.
// It deliberately stops there: ASR selection, LLM polish, and text injection
// live in later managers so the audio layer stays platform/provider agnostic.
//
// cpal's `Stream` is `!Send`, so capture lives on a dedicated thread that owns
// the stream from creation to drop. A second emitter thread snapshots the
// running peak every ~33ms. Samples are pushed into a shared buffer from the
// cpal callback and drained on stop.
//
// Mirrors Handy's pattern: cpal default host, blocking thread, atomic shutdown.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::asr::{AudioChunk, AudioData};
use crate::error::{AppError, AppResult};
use crate::platform::Platform;

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

/// Optional live PCM outlet (P2.5). When present, the cpal callback forwards
/// each buffer of samples as an `AudioChunk` in addition to the batch buffer —
/// the streaming ASR consumer drains the receiver. `None` (the default path) is
/// a no-op, so the batch flow keeps its current cost. Real consumer lands P2.6.
struct ChunkSink {
    tx: mpsc::Sender<AppResult<AudioChunk>>,
    sequence: AtomicU64,
    sample_rate: u32,
    channels: u16,
}

impl ChunkSink {
    fn forward(&self, samples: Vec<f32>) {
        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed);
        // A dropped receiver just means the streaming consumer went away; the
        // batch path is unaffected, so swallow the send error.
        let _ = self.tx.send(Ok(AudioChunk {
            samples,
            sample_rate: self.sample_rate,
            channels: self.channels,
            sequence,
            is_final: false,
        }));
    }

    /// Send an end-of-input sentinel so the streaming consumer's recv loop ends
    /// deterministically at stop. We must NOT rely on the Sender's Drop for this:
    /// the cpal callback holds a ChunkSink clone whose release can lag past stop,
    /// so the doubao loop would block on recv forever and never send its closing
    /// frame — the take then hangs until the 20s timeout. An explicit is_final
    /// chunk breaks the loop now.
    fn finish(&self) {
        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed);
        let _ = self.tx.send(Ok(AudioChunk {
            samples: Vec::new(),
            sample_rate: self.sample_rate,
            channels: self.channels,
            sequence,
            is_final: true,
        }));
    }
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
        self.start_capture_inner(app, None)
    }

    /// Like `start_capture` but also forwards live PCM chunks to `chunk_tx` for
    /// the streaming ASR path. Batch buffering is unchanged. Wired into the hot
    /// path at P2.6; defined here so P2.5 lands the outlet plumbing.
    #[allow(dead_code)]
    pub fn start_capture_streaming(
        &self,
        app: AppHandle,
        chunk_tx: mpsc::Sender<AppResult<AudioChunk>>,
    ) -> AppResult<()> {
        self.start_capture_inner(app, Some(chunk_tx))
    }

    fn start_capture_inner(
        &self,
        app: AppHandle,
        chunk_tx: Option<mpsc::Sender<AppResult<AudioChunk>>>,
    ) -> AppResult<()> {
        let mut guard = self.session.lock();
        if guard.is_some() {
            log::warn!("start_capture called while a session is active; ignoring");
            return Ok(());
        }

        let shutdown = Arc::new(AtomicBool::new(false));
        let accum = Arc::new(Mutex::new(LevelAccum::default()));
        let buffer = Arc::new(Mutex::new(Vec::<f32>::new()));

        // Ask the platform layer whether to override cpal's `default_input_device`.
        // macOS: if the system default is Bluetooth, this returns the name of a
        // wired alternative so we sidestep the AirPods A2DP/HFP gotcha.
        let preferred_name = app
            .try_state::<Arc<dyn Platform>>()
            .and_then(|p| p.inner().preferred_input_device_name());

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
                run_capture_thread(
                    shutdown_cap,
                    accum_cap,
                    buffer_cap,
                    chunk_tx,
                    preferred_name,
                    ready_tx,
                );
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
    chunk_tx: Option<mpsc::Sender<AppResult<AudioChunk>>>,
    preferred_name: Option<String>,
    ready_tx: mpsc::Sender<AppResult<AudioMeta>>,
) {
    let host = cpal::default_host();
    let device = match resolve_input_device(&host, preferred_name.as_deref()) {
        Some(d) => d,
        None => {
            let _ = ready_tx.send(Err(AppError::Device("no default input device".into())));
            return;
        }
    };

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
            sequence: AtomicU64::new(0),
            sample_rate: meta.sample_rate,
            channels: meta.channels,
        })
    });
    let err_fn = |err| log::error!("cpal stream error: {err}");

    let build_result = match sample_format {
        SampleFormat::F32 => {
            let (a, b, s) = (accum.clone(), buffer.clone(), chunk_sink.clone());
            device.build_input_stream(
                &config,
                move |data: &[f32], _| process_f32(&a, &b, s.as_deref(), data),
                err_fn,
                None,
            )
        }
        SampleFormat::I16 => {
            let (a, b, s) = (accum.clone(), buffer.clone(), chunk_sink.clone());
            device.build_input_stream(
                &config,
                move |data: &[i16], _| process_i16(&a, &b, s.as_deref(), data),
                err_fn,
                None,
            )
        }
        SampleFormat::U16 => {
            let (a, b, s) = (accum.clone(), buffer.clone(), chunk_sink.clone());
            device.build_input_stream(
                &config,
                move |data: &[u16], _| process_u16(&a, &b, s.as_deref(), data),
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
// append normalized f32 samples for transcription. When a streaming sink is
// present, the same normalized samples are also forwarded as an `AudioChunk`.
fn process_f32(
    accum: &Mutex<LevelAccum>,
    buffer: &Mutex<Vec<f32>>,
    sink: Option<&ChunkSink>,
    data: &[f32],
) {
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
    if let Some(sink) = sink {
        sink.forward(data.to_vec());
    }
}

fn process_i16(
    accum: &Mutex<LevelAccum>,
    buffer: &Mutex<Vec<f32>>,
    sink: Option<&ChunkSink>,
    data: &[i16],
) {
    let scale = i16::MAX as f32;
    let mut peak = 0.0f32;
    let mut chunk = sink.map(|_| Vec::with_capacity(data.len()));
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
            if let Some(chunk) = chunk.as_mut() {
                chunk.push(v);
            }
        }
    }
    merge_peak(accum, peak);
    if let (Some(sink), Some(chunk)) = (sink, chunk) {
        sink.forward(chunk);
    }
}

fn process_u16(
    accum: &Mutex<LevelAccum>,
    buffer: &Mutex<Vec<f32>>,
    sink: Option<&ChunkSink>,
    data: &[u16],
) {
    let mut peak = 0.0f32;
    let mut chunk = sink.map(|_| Vec::with_capacity(data.len()));
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
            if let Some(chunk) = chunk.as_mut() {
                chunk.push(v);
            }
        }
    }
    merge_peak(accum, peak);
    if let (Some(sink), Some(chunk)) = (sink, chunk) {
        sink.forward(chunk);
    }
}

/// Look up `preferred_name` in the host's input device list and return it; fall
/// back to the host default if the name doesn't resolve (device may have been
/// unplugged between platform-layer query and capture). cpal's `name()` is
/// fallible per device — skip the ones that error.
fn resolve_input_device(host: &cpal::Host, preferred_name: Option<&str>) -> Option<cpal::Device> {
    if let Some(name) = preferred_name {
        if let Ok(devices) = host.input_devices() {
            for d in devices {
                if d.name().ok().as_deref() == Some(name) {
                    log::info!("using preferred input device: {name}");
                    return Some(d);
                }
            }
            log::warn!("preferred input device {name:?} not found via cpal; using default");
        }
    }
    host.default_input_device()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_f32_forwards_chunk_when_sink_present() {
        let accum = Mutex::new(LevelAccum::default());
        let buffer = Mutex::new(Vec::new());
        let (tx, rx) = mpsc::channel();
        let sink = ChunkSink {
            tx,
            sequence: AtomicU64::new(0),
            sample_rate: 16_000,
            channels: 1,
        };

        process_f32(&accum, &buffer, Some(&sink), &[0.5, -0.25]);

        // Batch buffer still gets the samples …
        assert_eq!(*buffer.lock(), vec![0.5, -0.25]);
        // … and the same samples arrive as an AudioChunk.
        let chunk = rx.recv().unwrap().unwrap();
        assert_eq!(chunk.samples, vec![0.5, -0.25]);
        assert_eq!(chunk.sequence, 0);
        assert!(!chunk.is_final);
    }

    #[test]
    fn process_f32_without_sink_only_fills_buffer() {
        let accum = Mutex::new(LevelAccum::default());
        let buffer = Mutex::new(Vec::new());

        process_f32(&accum, &buffer, None, &[0.1, 0.2]);

        assert_eq!(*buffer.lock(), vec![0.1, 0.2]);
    }

    #[test]
    fn process_i16_normalizes_and_forwards() {
        let accum = Mutex::new(LevelAccum::default());
        let buffer = Mutex::new(Vec::new());
        let (tx, rx) = mpsc::channel();
        let sink = ChunkSink {
            tx,
            sequence: AtomicU64::new(0),
            sample_rate: 16_000,
            channels: 1,
        };

        process_i16(&accum, &buffer, Some(&sink), &[i16::MAX, 0]);

        let chunk = rx.recv().unwrap().unwrap();
        assert_eq!(chunk.samples, vec![1.0, 0.0]);
        assert_eq!(*buffer.lock(), vec![1.0, 0.0]);
    }
}
