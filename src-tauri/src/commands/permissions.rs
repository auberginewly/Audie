use crate::error::AppError;
use crate::platform::Platform;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
/// Dev-only trigger-key probe (P3.8). Starts a listen-only CGEventTap so we can
/// verify fn + custom single/combo keys reach us before P3.9 swaps the real
/// trigger; key events surface as the `trigger-probe-key` event. SPEC §5.8.
#[cfg(debug_assertions)]
#[tauri::command]
pub fn start_trigger_probe(app: AppHandle) -> Result<(), AppError> {
    app.state::<Arc<dyn Platform>>().start_trigger_probe(&app)
}

#[cfg(debug_assertions)]
#[tauri::command]
pub fn stop_trigger_probe(app: AppHandle) -> Result<(), AppError> {
    app.state::<Arc<dyn Platform>>().stop_trigger_probe()
}

/// P3.9 — Input Monitoring permission (macOS). The default trigger (fn) needs it.
/// `get` reads status without prompting; `request` shows the system prompt then
/// returns the (possibly still-false) status — a fresh grant only applies after
/// relaunch (SPEC §5.8 P3.9).
#[tauri::command]
pub fn get_input_monitoring_status(app: AppHandle) -> bool {
    app.state::<Arc<dyn Platform>>().input_monitoring_status()
}

#[tauri::command]
pub fn request_input_monitoring_permission(app: AppHandle) -> bool {
    let platform = app.state::<Arc<dyn Platform>>();
    platform.request_input_monitoring();
    platform.input_monitoring_status()
}

/// P3.12 — Microphone permission (macOS TCC). `get` reads status without prompting
/// (the onboarding wizard polls it on focus); `request` shows the system prompt then
/// returns the (possibly still-false) status.
#[tauri::command]
pub fn get_microphone_permission_status(app: AppHandle) -> bool {
    app.state::<Arc<dyn Platform>>().microphone_status()
}

#[tauri::command]
pub fn request_microphone_permission(app: AppHandle) -> bool {
    let platform = app.state::<Arc<dyn Platform>>();
    platform.request_microphone();
    platform.microphone_status()
}

/// P3.12 — Accessibility permission (macOS, post-event access). Injection's synthetic
/// Cmd+V needs it. `get` preflights without prompting; `request` shows the prompt.
#[tauri::command]
pub fn get_accessibility_permission_status(app: AppHandle) -> bool {
    app.state::<Arc<dyn Platform>>().accessibility_status()
}

#[tauri::command]
pub fn request_accessibility_permission(app: AppHandle) -> bool {
    let platform = app.state::<Arc<dyn Platform>>();
    platform.request_accessibility();
    platform.accessibility_status()
}

#[tauri::command]
pub fn restart_app(app: AppHandle) {
    app.request_restart();
}
