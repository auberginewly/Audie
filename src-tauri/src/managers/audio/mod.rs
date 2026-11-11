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

use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use std::thread::JoinHandle;

use parking_lot::Mutex;
use serde::Serialize;

use crate::asr::{AudioChunk, AudioData};
use crate::error::AppResult;

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
}

impl Default for AudioManager {
    fn default() -> Self {
        Self::new()
    }
}

mod buffer;
mod capture;
mod monitor;
mod recording;

pub use capture::{auto_input_device_name, list_input_devices, MicrophoneInfo};
