use crate::error::AppError;
use crate::platform::Platform;
use crate::settings::{
    load_settings, persist_settings, settings_from_patch, Settings, SettingsPatch,
};
use std::sync::Arc;
use tauri::Emitter;
use tauri::{AppHandle, Manager};
#[tauri::command]
pub fn get_settings(app: AppHandle) -> Result<Settings, AppError> {
    Ok(load_settings(&app))
}

/// Persist a new hotkey and apply it live: unregister the old combo, register
/// the new one (rebuilding the press/release callback), then write the store.
/// Re-register before persist so a registration failure leaves the store
/// untouched (no "saved but not active" mismatch).
#[tauri::command]
pub fn update_settings(app: AppHandle, patch: SettingsPatch) -> Result<Settings, AppError> {
    let current = load_settings(&app);
    let next = settings_from_patch(current.clone(), patch)?;

    crate::hotkeys::apply_hotkeys_if_changed(&app, &current, &next)?;
    apply_dock_visibility_if_changed(&app, &current, &next)?;
    persist_settings(&app, next)?;

    let updated = load_settings(&app);
    if let Err(err) = app.emit("settings-updated", &updated) {
        log::warn!("emit settings-updated failed: {err}");
    }
    Ok(updated)
}

fn apply_dock_visibility_if_changed(
    app: &AppHandle,
    current: &Settings,
    next: &Settings,
) -> Result<(), AppError> {
    if next.show_in_dock == current.show_in_dock {
        return Ok(());
    }
    let platform = app.state::<Arc<dyn Platform>>();
    platform.set_dock_visible(app, next.show_in_dock)
}
