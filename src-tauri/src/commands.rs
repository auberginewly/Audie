// Tauri commands for settings persistence (PROJECT_SPEC.md §3.5).
//
// P0.5 scope: hotkey only (microphone selection deferred to the future Settings
// page). Settings live in a tauri-plugin-store JSON file — NO manager owns them;
// the store plugin is the persistence layer per the P0.5 plan. Secrets never go
// here (those are P1 keychain, §6.6).

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;

use crate::error::AppError;
use crate::platform::Platform;

const STORE_FILE: &str = "settings.json";
const KEY_HOTKEY: &str = "hotkey";

pub const DEFAULT_HOTKEY: &str = "Ctrl+Shift+Space";

/// The only hotkeys the UI lets you pick in P0.5. A free-form key recorder is
/// P3 Settings-page work; presets keep this slice small and parseable by
/// `tauri-plugin-global-shortcut`.
pub const HOTKEY_PRESETS: &[&str] = &["Ctrl+Shift+Space", "Alt+Space", "Ctrl+Alt+Space"];

#[derive(Serialize, Deserialize, Clone)]
pub struct Settings {
    pub hotkey: String,
}

/// Read the persisted hotkey, falling back to the default when the store is
/// empty or holds something unexpected. Called at startup (lib.rs) and by
/// `get_settings`.
pub fn load_hotkey(app: &AppHandle) -> String {
    let stored = app
        .store(STORE_FILE)
        .ok()
        .and_then(|store| store.get(KEY_HOTKEY))
        .and_then(|value| value.as_str().map(str::to_string));

    match stored {
        Some(hotkey) if HOTKEY_PRESETS.contains(&hotkey.as_str()) => hotkey,
        _ => DEFAULT_HOTKEY.to_string(),
    }
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Result<Settings, AppError> {
    Ok(Settings {
        hotkey: load_hotkey(&app),
    })
}

/// Persist a new hotkey and apply it live: unregister the old combo, register
/// the new one (rebuilding the press/release callback), then write the store.
/// Re-register before persist so a registration failure leaves the store
/// untouched (no "saved but not active" mismatch).
#[tauri::command]
pub fn update_settings(app: AppHandle, hotkey: String) -> Result<Settings, AppError> {
    if !HOTKEY_PRESETS.contains(&hotkey.as_str()) {
        return Err(AppError::Internal(format!("unsupported hotkey: {hotkey}")));
    }

    let platform = app.state::<Arc<dyn Platform>>();
    platform.unregister_all_hotkeys(&app)?;
    platform.register_hotkey(&app, &hotkey, crate::build_hotkey_callback(&app))?;

    let store = app
        .store(STORE_FILE)
        .map_err(|err| AppError::Internal(format!("open store: {err}")))?;
    store.set(KEY_HOTKEY, hotkey.clone());
    store
        .save()
        .map_err(|err| AppError::Internal(format!("save store: {err}")))?;

    Ok(Settings { hotkey })
}
