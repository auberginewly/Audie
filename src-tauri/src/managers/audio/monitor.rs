use super::capture::{run_capture_thread, CapturePlan, CaptureThreadArgs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use parking_lot::Mutex;
use tauri::{AppHandle, Emitter, Manager};

use super::{
    AudioLevelPayload, AudioManager, AudioMeta, LevelAccum, MonitorSession, RetainedSamples,
    EMIT_INTERVAL_MS, MIC_MONITOR_LEVEL_EVENT,
};
use crate::error::{AppError, AppResult};
use crate::platform::Platform;

impl AudioManager {
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

pub(super) fn run_emit_thread(
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
