use super::capture::{run_capture_thread, CapturePlan, CaptureThreadArgs};
use super::monitor::run_emit_thread;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::sync::Arc;
use std::thread;

use parking_lot::Mutex;
use tauri::{AppHandle, Manager};

use super::{
    AudioManager, AudioMeta, CaptureSession, CapturedAudio, LevelAccum, RetainedSamples,
    AUDIO_LEVEL_EVENT, MAX_RETAINED_SAMPLES, RECORDING_MEMORY_LIMIT_MESSAGE,
};
use crate::asr::{AudioChunk, AudioData};
use crate::error::{AppError, AppResult};
use crate::platform::Platform;

impl AudioManager {
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
        let selected = crate::settings::load_settings(&app).input_device;
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
}
