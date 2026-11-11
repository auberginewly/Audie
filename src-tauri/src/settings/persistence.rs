use super::*;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
/// Legacy tauri-plugin-store filename, read once to migrate into TOML (P3 cleanup).
const LEGACY_SETTINGS_JSON: &str = "settings.json";

/// Path to the human-editable settings file (TOML), in the platform config dir.
fn settings_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    app.path()
        .app_config_dir()
        .map(|dir| dir.join("settings.toml"))
        .map_err(|err| AppError::Internal(format!("resolve config dir: {err}")))
}

fn write_settings_toml(path: &Path, settings: &Settings) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| AppError::Internal(format!("create config dir: {err}")))?;
    }
    let text = toml::to_string_pretty(settings)
        .map_err(|err| AppError::Internal(format!("serialize settings: {err}")))?;
    std::fs::write(path, text)
        .map_err(|err| AppError::Internal(format!("write settings.toml: {err}")))
}

/// Load user settings from `settings.toml`. The first run after the TOML switch the
/// file is absent, so we migrate the legacy tauri-plugin-store JSON (which yields
/// defaults when empty) and write the TOML once. Parse failures degrade to defaults
/// rather than wedging settings. Always normalized so callers get valid values.
pub fn load_settings(app: &AppHandle) -> Settings {
    let path = match settings_path(app) {
        Ok(path) => path,
        Err(err) => {
            log::error!("{err}");
            return Settings::default();
        }
    };

    if path.exists() {
        match std::fs::read_to_string(&path) {
            Ok(text) => match toml::from_str::<Settings>(&text) {
                Ok(settings) => {
                    let migrate_windows_fn = should_migrate_windows_fn(&settings.hotkey);
                    let normalized = normalize_settings(settings);
                    if migrate_windows_fn {
                        if let Err(err) = write_settings_toml(&path, &normalized) {
                            log::error!(
                                "write settings.toml during Windows hotkey migration: {err}"
                            );
                        }
                    }
                    return normalized;
                }
                Err(err) => log::error!("parse settings.toml, using defaults: {err}"),
            },
            Err(err) => log::error!("read settings.toml: {err}"),
        }
        return normalize_settings(Settings::default());
    }

    let migrated = normalize_settings(migrate_from_legacy_json(app));
    if let Err(err) = write_settings_toml(&path, &migrated) {
        log::error!("write settings.toml during migration: {err}");
    }
    migrated
}

/// The persisted hotkey, for startup (lib.rs) + `hotkeys::apply_hotkeys_if_changed`. Derived
/// from the full settings load so there's a single source of truth.
pub fn load_hotkey(app: &AppHandle) -> String {
    load_settings(app).hotkey
}

/// One-time migration: read the legacy tauri-plugin-store JSON directly with serde
/// (a flat key→value object whose keys match Settings fields), so we no longer
/// depend on the store plugin. Unknown legacy keys are ignored and missing fields
/// use Default (container `serde(default)`); an absent/unreadable/corrupt file
/// yields Default.
fn migrate_from_legacy_json(app: &AppHandle) -> Settings {
    let Ok(dir) = app.path().app_config_dir() else {
        return Settings::default();
    };
    match std::fs::read_to_string(dir.join(LEGACY_SETTINGS_JSON)) {
        Ok(text) => serde_json::from_str::<Settings>(&text).unwrap_or_else(|err| {
            log::warn!("parse legacy settings.json, using defaults: {err}");
            Settings::default()
        }),
        Err(_) => Settings::default(),
    }
}

pub(crate) fn persist_settings(app: &AppHandle, settings: Settings) -> Result<(), AppError> {
    let path = settings_path(app)?;
    write_settings_toml(&path, &settings)
}
