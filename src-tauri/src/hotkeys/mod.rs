//! Trigger wiring only: registration, settings capture and pipeline callbacks.
mod capture;
mod registration;
use crate::error::{AppError, AppResult};
use crate::pipeline::{self, HotkeyRole};
use crate::platform::{HotkeyCallback, HotkeySlot, Platform};
use crate::settings;
pub(crate) use capture::{begin_trigger_capture, end_trigger_capture};
pub(crate) use registration::apply_hotkeys_if_changed;
use registration::register_compose_hotkey;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

fn build_hotkey_callback(app: &AppHandle, role: HotkeyRole) -> HotkeyCallback {
    let app = app.clone();
    Box::new(move || pipeline::handle_trigger(&app, role))
}

pub(crate) fn register_configured(app: &AppHandle) {
    let platform = app.state::<Arc<dyn Platform>>();
    let hotkey = settings::load_hotkey(app);
    if let Err(err) = platform.register_hotkey(
        app,
        HotkeySlot::Primary,
        &hotkey,
        build_hotkey_callback(app, HotkeyRole::Primary),
    ) {
        log::warn!(
            "register trigger {hotkey} failed (grant Input Monitoring then relaunch): {err:?}"
        );
    } else {
        log::info!("registered trigger {hotkey}");
    }
    register_compose_hotkey(app);
}
