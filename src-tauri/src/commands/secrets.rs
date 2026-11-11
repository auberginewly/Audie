use crate::error::AppError;
use crate::platform::Platform;
use crate::settings::validate_secret_key_id;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
#[tauri::command]
pub fn set_secret(app: AppHandle, key_id: String, value: String) -> Result<(), AppError> {
    validate_secret_key_id(&key_id)?;
    if value.is_empty() {
        return Err(AppError::Provider("secret value cannot be empty".into()));
    }

    let platform = app.state::<Arc<dyn Platform>>();
    platform.store_secret(&key_id, &value)
}

#[tauri::command]
pub fn has_secret(app: AppHandle, key_id: String) -> Result<bool, AppError> {
    validate_secret_key_id(&key_id)?;

    let platform = app.state::<Arc<dyn Platform>>();
    platform_has_secret(platform.inner().as_ref(), &key_id)
}

/// Return a saved secret only for the Settings eye-button Reveal action.
/// Opening the dialog uses `has_secret` instead, so it never requests Keychain data.
#[tauri::command]
pub fn get_secret_for_settings(app: AppHandle, key_id: String) -> Result<Option<String>, AppError> {
    validate_secret_key_id(&key_id)?;

    let platform = app.state::<Arc<dyn Platform>>();
    secret_for_explicit_reveal(
        &key_id,
        |key_id| platform.has_secret(key_id),
        |key_id| platform.read_secret(key_id),
    )
}

#[tauri::command]
pub fn delete_secret(app: AppHandle, key_id: String) -> Result<(), AppError> {
    validate_secret_key_id(&key_id)?;

    let platform = app.state::<Arc<dyn Platform>>();
    platform.delete_secret(&key_id)
}

fn platform_has_secret(platform: &dyn Platform, key_id: &str) -> Result<bool, AppError> {
    platform.has_secret(key_id)
}

fn secret_for_explicit_reveal(
    key_id: &str,
    has_secret: impl FnOnce(&str) -> Result<bool, AppError>,
    read_secret: impl FnOnce(&str) -> Result<String, AppError>,
) -> Result<Option<String>, AppError> {
    if has_secret(key_id)? {
        read_secret(key_id).map(Some)
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests;
