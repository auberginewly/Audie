use crate::error::AppError;
use crate::platform::Platform;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
/// Enumerate input devices for the Settings device picker. A cheap CoreAudio/cpal
/// query (not the hot path), so it stays a plain sync command like the provider
/// lists. Returns names that `input_device` / device resolution match on.
#[tauri::command]
pub fn list_microphones() -> Vec<crate::managers::audio::MicrophoneInfo> {
    crate::managers::audio::list_input_devices()
}

/// Name of the device the automatic path would open (P0.7's override or the
/// system default), so the picker's "自动" row can show what it resolves to.
#[tauri::command]
pub fn auto_input_device(app: AppHandle) -> Option<String> {
    let platform = app.state::<Arc<dyn Platform>>();
    let preferred = platform.preferred_input_device_name();
    crate::managers::audio::auto_input_device_name(preferred)
}

/// Start the Settings mic-preview monitor on `device` (None / "" = automatic).
/// Emits `mic-monitor-level` until `stop_mic_monitor`, so the picker can show the
/// chosen mic is actually picking up sound. Called when the device section mounts
/// and on every selection change.
#[tauri::command]
pub fn start_mic_monitor(app: AppHandle, device: Option<String>) -> Result<(), AppError> {
    let audio = app.state::<Arc<crate::managers::audio::AudioManager>>();
    audio.start_monitor(app.clone(), device)
}

/// Stop the Settings mic-preview monitor (section unmounted / dialog closed).
#[tauri::command]
pub fn stop_mic_monitor(app: AppHandle) -> Result<(), AppError> {
    let audio = app.state::<Arc<crate::managers::audio::AudioManager>>();
    audio.stop_monitor();
    Ok(())
}
