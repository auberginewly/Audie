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
use std::sync::mpsc::{self, SyncSender, TrySendError};
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
// Settings mic-preview level. A separate event from `audio-level` so the preview
// meter and the overlay waveform never cross-drive each other.
const MIC_MONITOR_LEVEL_EVENT: &str = "mic-monitor-level";
const EMIT_INTERVAL_MS: u64 = 33;
const SHUTDOWN_POLL_MS: u64 = 10;
const MAX_RETAINED_AUDIO_BYTES: usize = 512 * 1024 * 1024;
const MAX_RETAINED_SAMPLES: usize = MAX_RETAINED_AUDIO_BYTES / std::mem::size_of::<f32>();
const RECORDING_MEMORY_LIMIT_MESSAGE: &str =
    "录音占用内存超过 512 MiB，已丢弃这次音频，请重新录一段较短内容";

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

struct RetainedSamples {
    max_samples: usize,
    exceeded: AtomicBool,
}

impl RetainedSamples {
    fn new(max_samples: usize) -> Self {
        Self {
            max_samples,
            exceeded: AtomicBool::new(false),
        }
    }

    fn accepted_len(&self, current_len: usize, incoming_len: usize) -> usize {
        let remaining = self.max_samples.saturating_sub(current_len);
        let accepted = remaining.min(incoming_len);
        if accepted < incoming_len {
            self.exceeded.store(true, Ordering::Relaxed);
        }
        accepted
    }

    fn exceeded(&self) -> bool {
        self.exceeded.load(Ordering::Relaxed)
    }
}

/// Optional live PCM outlet (P2.5). When present, the cpal callback forwards
/// each buffer of samples as an `AudioChunk` in addition to the batch buffer —
/// the streaming ASR consumer drains the receiver. `None` (the default path) is
/// a no-op, so the batch flow keeps its current cost. Real consumer lands P2.6.
struct ChunkSink {
    tx: SyncSender<AppResult<AudioChunk>>,
    overflowed: Arc<AtomicBool>,
    sequence: AtomicU64,
    sample_rate: u32,
    channels: u16,
}

impl ChunkSink {
    fn forward(&self, samples: Vec<f32>) {
        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed);
        let result = self.tx.try_send(Ok(AudioChunk {
            samples,
            sample_rate: self.sample_rate,
            channels: self.channels,
            sequence,
            is_final: false,
        }));
        match result {
            Ok(()) | Err(TrySendError::Disconnected(_)) => {}
            Err(TrySendError::Full(_)) => {
                self.overflowed.store(true, Ordering::Relaxed);
            }
        }
    }

    /// Send an end-of-input sentinel so the streaming consumer's recv loop ends
    /// deterministically at stop. We must NOT rely on the Sender's Drop for this:
    /// the cpal callback holds a ChunkSink clone whose release can lag past stop,
    /// so the doubao loop would block on recv forever and never send its closing
    /// frame — the take then hangs until the 20s timeout. An explicit is_final
    /// chunk breaks the loop now.
    fn finish(&self) {
        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed);
        let result = self.tx.try_send(Ok(AudioChunk {
            samples: Vec::new(),
            sample_rate: self.sample_rate,
            channels: self.channels,
            sequence,
            is_final: true,
        }));
        match result {
            Ok(()) | Err(TrySendError::Disconnected(_)) => {}
            Err(TrySendError::Full(_)) => {
                self.overflowed.store(true, Ordering::Relaxed);
                log::warn!("streaming audio queue full at final sentinel; receiver will finish on channel close");
            }
        }
    }
}

struct CaptureSession {
    shutdown: Arc<AtomicBool>,
    capture_thread: Option<JoinHandle<()>>,
    emit_thread: Option<JoinHandle<()>>,
    buffer: Arc<Mutex<Vec<f32>>>,
    retained: Arc<RetainedSamples>,
    streaming_queue_overflowed: Option<Arc<AtomicBool>>,
    meta: AudioMeta,
}

pub struct CapturedAudio {
    pub audio: AudioData,
    pub streaming_queue_overflowed: bool,
}

/// A level-only capture for the Settings mic preview. Like `CaptureSession` but
/// with no sample buffer — it exists purely to emit `mic-monitor-level` so the
/// user can confirm the picked mic is actually hearing them.
struct MonitorSession {
    shutdown: Arc<AtomicBool>,
    capture_thread: Option<JoinHandle<()>>,
    emit_thread: Option<JoinHandle<()>>,
}

pub struct AudioManager {
    session: Mutex<Option<CaptureSession>>,
    monitor: Mutex<Option<MonitorSession>>,
}

impl AudioManager {
    pub fn new() -> Self {
        Self {
            session: Mutex::new(None),
            monitor: Mutex::new(None),
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
        chunk_tx: SyncSender<AppResult<AudioChunk>>,
    ) -> AppResult<()> {
        self.start_capture_inner(app, Some(chunk_tx))
    }

    fn start_capture_inner(
        &self,
        app: AppHandle,
        chunk_tx: Option<SyncSender<AppResult<AudioChunk>>>,
    ) -> AppResult<()> {
        let mut guard = self.session.lock();
        if guard.is_some() {
            log::warn!("start_capture called while a session is active; ignoring");
            return Ok(());
        }

        let shutdown = Arc::new(AtomicBool::new(false));
        let accum = Arc::new(Mutex::new(LevelAccum::default()));
        let buffer = Arc::new(Mutex::new(Vec::<f32>::new()));
        let retained = Arc::new(RetainedSamples::new(MAX_RETAINED_SAMPLES));
        let streaming_queue_overflowed =
            chunk_tx.as_ref().map(|_| Arc::new(AtomicBool::new(false)));

        // Device resolution: an explicit pick in Settings (`input_device`, empty =
        // automatic) wins; otherwise fall back to the platform's P0.7 auto-pick
        // (returns a wired alternative when the system default is Bluetooth, to
        // sidestep the AirPods A2DP/HFP gotcha). `explicit` rides along so an
        // explicit pick that's gone errors instead of silently using the default.
        let selected = crate::commands::load_settings(&app).input_device;
        let (preferred_name, explicit) = if selected.trim().is_empty() {
            let auto = app
                .try_state::<Arc<dyn Platform>>()
                .and_then(|p| p.inner().preferred_input_device_name());
            (auto, false)
        } else {
            (Some(selected), true)
        };

        // Spawn the capture thread and wait for stream setup to either succeed
        // (reporting the stream's format) or fail. cpal's `Stream` is `!Send`,
        // so it must be created and parked on the same thread.
        let (ready_tx, ready_rx) = mpsc::channel::<AppResult<AudioMeta>>();
        let shutdown_cap = shutdown.clone();
        let accum_cap = accum.clone();
        let buffer_cap = buffer.clone();
        let retained_cap = retained.clone();
        let overflowed_cap = streaming_queue_overflowed.clone();
        let capture_thread = thread::Builder::new()
            .name("audie-audio-capture".into())
            .spawn(move || {
                run_capture_thread(CaptureThreadArgs {
                    shutdown: shutdown_cap,
                    accum: accum_cap,
                    buffer: buffer_cap,
                    retained: retained_cap,
                    chunk_tx,
                    streaming_queue_overflowed: overflowed_cap,
                    plan: CapturePlan {
                        preferred_name,
                        explicit,
                        level_only: false,
                    },
                    ready_tx,
                });
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
                run_emit_thread(app_emit, accum_emit, shutdown_emit, AUDIO_LEVEL_EVENT);
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
            retained,
            streaming_queue_overflowed,
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
    pub fn stop_capture(&self) -> AppResult<CapturedAudio> {
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
        if session.retained.exceeded() {
            log::warn!(
                "audio capture exceeded 512 MiB retained-memory fuse; dropped {} retained samples",
                samples.len()
            );
            return Err(AppError::Device(RECORDING_MEMORY_LIMIT_MESSAGE.into()));
        }
        log::info!("audio capture stopped ({} samples)", samples.len());
        let streaming_queue_overflowed = session
            .streaming_queue_overflowed
            .as_deref()
            .is_some_and(|overflowed| overflowed.load(Ordering::Relaxed));

        Ok(CapturedAudio {
            audio: AudioData {
                samples,
                sample_rate: session.meta.sample_rate,
                channels: session.meta.channels,
            },
            streaming_queue_overflowed,
        })
    }

    /// Open `device` (or P0.7's auto-pick when `None`/empty) level-only and emit
    /// `mic-monitor-level` so the Settings picker shows the mic is live. No sample
    /// buffer — purely a preview meter. Restarts cleanly if already running (the
    /// picker re-calls this when the selection changes). An explicit pick that
    /// can't open errors instead of falling back, so the meter just stays flat.
    pub fn start_monitor(&self, app: AppHandle, device: Option<String>) -> AppResult<()> {
        self.stop_monitor();

        let shutdown = Arc::new(AtomicBool::new(false));
        let accum = Arc::new(Mutex::new(LevelAccum::default()));
        // `level_only` keeps this empty; it exists only to satisfy the shared
        // capture-thread signature.
        let buffer = Arc::new(Mutex::new(Vec::<f32>::new()));

        let (preferred_name, explicit) = match device {
            Some(name) if !name.trim().is_empty() => (Some(name), true),
            _ => {
                let auto = app
                    .try_state::<Arc<dyn Platform>>()
                    .and_then(|p| p.inner().preferred_input_device_name());
                (auto, false)
            }
        };

        let (ready_tx, ready_rx) = mpsc::channel::<AppResult<AudioMeta>>();
        let shutdown_cap = shutdown.clone();
        let accum_cap = accum.clone();
        let retained = Arc::new(RetainedSamples::new(usize::MAX));
        let capture_thread = thread::Builder::new()
            .name("audie-monitor-capture".into())
            .spawn(move || {
                run_capture_thread(CaptureThreadArgs {
                    shutdown: shutdown_cap,
                    accum: accum_cap,
                    buffer,
                    retained,
                    chunk_tx: None,
                    streaming_queue_overflowed: None,
                    plan: CapturePlan {
                        preferred_name,
                        explicit,
                        level_only: true,
                    },
                    ready_tx,
                });
            })
            .map_err(|e| AppError::Device(format!("spawn monitor capture thread: {e}")))?;

        match ready_rx.recv() {
            Ok(Ok(_)) => {}
            Ok(Err(err)) => {
                let _ = capture_thread.join();
                return Err(err);
            }
            Err(_) => {
                let _ = capture_thread.join();
                return Err(AppError::Internal(
                    "monitor capture thread exited before reporting readiness".into(),
                ));
            }
        }

        let shutdown_emit = shutdown.clone();
        let accum_emit = accum.clone();
        let app_emit = app.clone();
        let emit_thread = match thread::Builder::new()
            .name("audie-monitor-emit".into())
            .spawn(move || {
                run_emit_thread(app_emit, accum_emit, shutdown_emit, MIC_MONITOR_LEVEL_EVENT);
            }) {
            Ok(t) => t,
            Err(e) => {
                shutdown.store(true, Ordering::Relaxed);
                let _ = capture_thread.join();
                return Err(AppError::Internal(format!(
                    "spawn monitor emit thread: {e}"
                )));
            }
        };

        *self.monitor.lock() = Some(MonitorSession {
            shutdown,
            capture_thread: Some(capture_thread),
            emit_thread: Some(emit_thread),
        });
        Ok(())
    }

    /// Stop the preview monitor if running (Settings closed / device changed /
    /// recording starting). No-op when idle.
    pub fn stop_monitor(&self) {
        let mut session = match self.monitor.lock().take() {
            Some(s) => s,
            None => return,
        };
        session.shutdown.store(true, Ordering::Relaxed);
        if let Some(t) = session.capture_thread.take() {
            if let Err(e) = t.join() {
                log::warn!("monitor capture thread panicked: {e:?}");
            }
        }
        if let Some(t) = session.emit_thread.take() {
            if let Err(e) = t.join() {
                log::warn!("monitor emit thread panicked: {e:?}");
            }
        }
    }
}

impl Default for AudioManager {
    fn default() -> Self {
        Self::new()
    }
}

/// How a capture thread opens and consumes the input device for one session.
struct CapturePlan {
    /// Explicit device name to use, or `None` to fall back to the host default.
    preferred_name: Option<String>,
    /// The name came from an explicit user pick, so a miss is a Device error
    /// rather than a silent fall back to the default.
    explicit: bool,
    /// Track levels only, don't retain samples (the Settings mic preview).
    level_only: bool,
}

struct CaptureThreadArgs {
    shutdown: Arc<AtomicBool>,
    accum: Arc<Mutex<LevelAccum>>,
    buffer: Arc<Mutex<Vec<f32>>>,
    retained: Arc<RetainedSamples>,
    chunk_tx: Option<SyncSender<AppResult<AudioChunk>>>,
    streaming_queue_overflowed: Option<Arc<AtomicBool>>,
    plan: CapturePlan,
    ready_tx: mpsc::Sender<AppResult<AudioMeta>>,
}

fn run_capture_thread(args: CaptureThreadArgs) {
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

fn run_emit_thread(
    app: AppHandle,
    accum: Arc<Mutex<LevelAccum>>,
    shutdown: Arc<AtomicBool>,
    event: &'static str,
) {
    while !shutdown.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(EMIT_INTERVAL_MS));
        let level = {
            let mut g = accum.lock();
            let l = g.peak;
            g.peak = 0.0;
            l
        };
        if let Err(e) = app.emit(event, AudioLevelPayload { level }) {
            log::warn!("emit {event} failed: {e}");
        }
    }
    // Flush a final zero so the UI bars drop instead of freezing on last peak.
    let _ = app.emit(event, AudioLevelPayload { level: 0.0 });
}

// Each `process_*` does one locked pass: track the peak for the waveform and
// append normalized f32 samples for transcription. When a streaming sink is
// present, the same normalized samples are also forwarded as an `AudioChunk`.
fn process_f32(
    accum: &Mutex<LevelAccum>,
    buffer: &Mutex<Vec<f32>>,
    sink: Option<&ChunkSink>,
    retained: &RetainedSamples,
    data: &[f32],
    level_only: bool,
) {
    let mut peak = 0.0f32;
    let mut accepted = data.len();
    {
        // `level_only` (Settings mic preview) tracks the peak only — no buffer
        // lock, no sample retention — so a long preview can't grow memory.
        let mut buf = if level_only {
            None
        } else {
            Some(buffer.lock())
        };
        if let Some(buf) = buf.as_mut() {
            accepted = retained.accepted_len(buf.len(), data.len());
            buf.reserve(accepted);
        }
        for (index, &s) in data.iter().enumerate() {
            let a = s.abs();
            if a > peak {
                peak = a;
            }
            if index < accepted {
                if let Some(buf) = buf.as_mut() {
                    buf.push(s);
                }
            }
        }
    }
    merge_peak(accum, peak);
    if let Some(sink) = sink.filter(|_| accepted > 0) {
        sink.forward(data[..accepted].to_vec());
    }
}

fn process_i16(
    accum: &Mutex<LevelAccum>,
    buffer: &Mutex<Vec<f32>>,
    sink: Option<&ChunkSink>,
    retained: &RetainedSamples,
    data: &[i16],
    level_only: bool,
) {
    let scale = i16::MAX as f32;
    let mut peak = 0.0f32;
    let mut chunk: Option<Vec<f32>> = None;
    {
        let mut buf = if level_only {
            None
        } else {
            Some(buffer.lock())
        };
        let accepted = buf.as_ref().map_or(data.len(), |buf| {
            retained.accepted_len(buf.len(), data.len())
        });
        if let Some(buf) = buf.as_mut() {
            buf.reserve(accepted);
        }
        if sink.is_some() && accepted > 0 {
            chunk = Some(Vec::with_capacity(accepted));
        }
        for (index, &s) in data.iter().enumerate() {
            let v = s as f32 / scale;
            let a = v.abs();
            if a > peak {
                peak = a;
            }
            if index < accepted {
                if let Some(buf) = buf.as_mut() {
                    buf.push(v);
                }
                if let Some(chunk) = chunk.as_mut() {
                    chunk.push(v);
                }
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
    retained: &RetainedSamples,
    data: &[u16],
    level_only: bool,
) {
    let mut peak = 0.0f32;
    let mut chunk: Option<Vec<f32>> = None;
    {
        let mut buf = if level_only {
            None
        } else {
            Some(buffer.lock())
        };
        let accepted = buf.as_ref().map_or(data.len(), |buf| {
            retained.accepted_len(buf.len(), data.len())
        });
        if let Some(buf) = buf.as_mut() {
            buf.reserve(accepted);
        }
        if sink.is_some() && accepted > 0 {
            chunk = Some(Vec::with_capacity(accepted));
        }
        for (index, &s) in data.iter().enumerate() {
            // u16 silence is centered at 32768.
            let v = (s as f32 - 32768.0) / 32768.0;
            let a = v.abs();
            if a > peak {
                peak = a;
            }
            if index < accepted {
                if let Some(buf) = buf.as_mut() {
                    buf.push(v);
                }
                if let Some(chunk) = chunk.as_mut() {
                    chunk.push(v);
                }
            }
        }
    }
    merge_peak(accum, peak);
    if let (Some(sink), Some(chunk)) = (sink, chunk) {
        sink.forward(chunk);
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

fn merge_peak(accum: &Mutex<LevelAccum>, local: f32) {
    if local <= 0.0 {
        return;
    }
    let mut g = accum.lock();
    if local > g.peak {
        g.peak = local;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_f32_forwards_chunk_when_sink_present() {
        let accum = Mutex::new(LevelAccum::default());
        let buffer = Mutex::new(Vec::new());
        let retained = RetainedSamples::new(usize::MAX);
        let (tx, rx) = mpsc::sync_channel(1);
        let overflowed = Arc::new(AtomicBool::new(false));
        let sink = ChunkSink {
            tx,
            overflowed,
            sequence: AtomicU64::new(0),
            sample_rate: 16_000,
            channels: 1,
        };

        process_f32(
            &accum,
            &buffer,
            Some(&sink),
            &retained,
            &[0.5, -0.25],
            false,
        );

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
        let retained = RetainedSamples::new(usize::MAX);

        process_f32(&accum, &buffer, None, &retained, &[0.1, 0.2], false);

        assert_eq!(*buffer.lock(), vec![0.1, 0.2]);
    }

    #[test]
    fn process_i16_normalizes_and_forwards() {
        let accum = Mutex::new(LevelAccum::default());
        let buffer = Mutex::new(Vec::new());
        let retained = RetainedSamples::new(usize::MAX);
        let (tx, rx) = mpsc::sync_channel(1);
        let overflowed = Arc::new(AtomicBool::new(false));
        let sink = ChunkSink {
            tx,
            overflowed,
            sequence: AtomicU64::new(0),
            sample_rate: 16_000,
            channels: 1,
        };

        process_i16(
            &accum,
            &buffer,
            Some(&sink),
            &retained,
            &[i16::MAX, 0],
            false,
        );

        let chunk = rx.recv().unwrap().unwrap();
        assert_eq!(chunk.samples, vec![1.0, 0.0]);
        assert_eq!(*buffer.lock(), vec![1.0, 0.0]);
    }

    #[test]
    fn process_f32_level_only_tracks_peak_without_buffering() {
        let accum = Mutex::new(LevelAccum::default());
        let buffer = Mutex::new(Vec::new());
        let retained = RetainedSamples::new(usize::MAX);

        // The Settings mic preview can run for minutes, so it must not retain
        // samples — but it must still update the peak that drives the meter.
        process_f32(&accum, &buffer, None, &retained, &[0.5, -0.25], true);

        assert!(buffer.lock().is_empty());
        assert_eq!(accum.lock().peak, 0.5);
    }

    #[test]
    fn recording_longer_than_120_seconds_is_still_retained() {
        let meta = AudioMeta {
            sample_rate: 48_000,
            channels: 2,
        };
        let samples_at_125_seconds = meta.sample_rate as usize * meta.channels as usize * 125;
        let retained = RetainedSamples::new(MAX_RETAINED_SAMPLES);

        assert_eq!(retained.accepted_len(samples_at_125_seconds, 1_024), 1_024);
        assert!(!retained.exceeded());
    }

    #[test]
    fn retained_sample_limit_equals_512_mib_of_f32_audio() {
        assert_eq!(
            MAX_RETAINED_SAMPLES * std::mem::size_of::<f32>(),
            MAX_RETAINED_AUDIO_BYTES
        );
    }

    #[test]
    fn process_f32_below_memory_fuse_keeps_all_samples() {
        let accum = Mutex::new(LevelAccum::default());
        let buffer = Mutex::new(vec![0.1]);
        let retained = RetainedSamples::new(4);

        process_f32(&accum, &buffer, None, &retained, &[0.2, 0.3], false);

        assert_eq!(*buffer.lock(), vec![0.1, 0.2, 0.3]);
        assert!(!retained.exceeded());
    }

    #[test]
    fn process_f32_caps_batch_buffer_and_sets_overflow() {
        let accum = Mutex::new(LevelAccum::default());
        let buffer = Mutex::new(vec![0.1, 0.2]);
        let retained = RetainedSamples::new(3);

        process_f32(&accum, &buffer, None, &retained, &[0.3, 0.4], false);

        assert_eq!(*buffer.lock(), vec![0.1, 0.2, 0.3]);
        assert!(retained.exceeded());
        assert_eq!(accum.lock().peak, 0.4);
    }

    #[test]
    fn process_i16_caps_forwarded_chunk_to_retained_samples() {
        let accum = Mutex::new(LevelAccum::default());
        let buffer = Mutex::new(vec![0.0]);
        let retained = RetainedSamples::new(2);
        let (tx, rx) = mpsc::sync_channel(1);
        let overflowed = Arc::new(AtomicBool::new(false));
        let sink = ChunkSink {
            tx,
            overflowed,
            sequence: AtomicU64::new(0),
            sample_rate: 16_000,
            channels: 1,
        };

        process_i16(
            &accum,
            &buffer,
            Some(&sink),
            &retained,
            &[i16::MAX, i16::MIN],
            false,
        );

        assert_eq!(*buffer.lock(), vec![0.0, 1.0]);
        assert!(retained.exceeded());
        let chunk = rx.try_recv().unwrap().unwrap();
        assert_eq!(chunk.samples, vec![1.0]);
    }

    #[test]
    fn process_u16_after_cap_tracks_peak_without_growing_buffer() {
        let accum = Mutex::new(LevelAccum::default());
        let buffer = Mutex::new(vec![0.1]);
        let retained = RetainedSamples::new(1);

        process_u16(&accum, &buffer, None, &retained, &[u16::MAX], false);

        assert_eq!(*buffer.lock(), vec![0.1]);
        assert!(retained.exceeded());
        assert!(accum.lock().peak > 0.9);
    }

    #[test]
    fn chunk_sink_sets_overflow_when_stream_queue_is_full() {
        let (tx, rx) = mpsc::sync_channel(1);
        let overflowed = Arc::new(AtomicBool::new(false));
        let sink = ChunkSink {
            tx,
            overflowed: overflowed.clone(),
            sequence: AtomicU64::new(0),
            sample_rate: 16_000,
            channels: 1,
        };

        sink.forward(vec![0.1]);
        sink.forward(vec![0.2]);

        assert!(overflowed.load(Ordering::Relaxed));
        let chunk = rx.try_recv().unwrap().unwrap();
        assert_eq!(chunk.samples, vec![0.1]);
        assert_eq!(chunk.sequence, 0);
    }

    #[test]
    fn chunk_sink_finish_does_not_block_when_stream_queue_is_full() {
        let (tx, _rx) = mpsc::sync_channel(1);
        let overflowed = Arc::new(AtomicBool::new(false));
        let sink = ChunkSink {
            tx,
            overflowed: overflowed.clone(),
            sequence: AtomicU64::new(0),
            sample_rate: 16_000,
            channels: 1,
        };

        sink.forward(vec![0.1]);
        sink.finish();

        assert!(overflowed.load(Ordering::Relaxed));
    }
}
