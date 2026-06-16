// macOS implementation of trait Platform.
//
// P0.1: hotkey via tauri-plugin-global-shortcut. The callback is parked in the
// shared HotkeyRegistry — the plugin's `with_handler` (built in lib.rs) is the
// single entry that dispatches into the registry.
//
// P0.5 will add clipboard-based inject. P1 will add Keychain Services calls.

use std::sync::Arc;

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

use super::{HotkeyCallback, HotkeyRegistry, Platform};
use crate::error::{AppError, AppResult};

pub struct MacosPlatform {
    registry: Arc<HotkeyRegistry>,
}

impl MacosPlatform {
    pub fn new(registry: Arc<HotkeyRegistry>) -> Self {
        Self { registry }
    }
}

impl Platform for MacosPlatform {
    fn register_hotkey(
        &self,
        app: &AppHandle,
        combo: &str,
        callback: HotkeyCallback,
    ) -> AppResult<()> {
        let shortcut: Shortcut = combo
            .parse()
            .map_err(|err| AppError::Internal(format!("invalid hotkey combo {combo:?}: {err}")))?;

        self.registry.insert(shortcut, callback);

        app.global_shortcut()
            .register(shortcut)
            .map_err(|err| AppError::Internal(format!("failed to register hotkey: {err}")))?;

        Ok(())
    }

    fn unregister_all_hotkeys(&self, app: &AppHandle) -> AppResult<()> {
        if let Err(err) = app.global_shortcut().unregister_all() {
            log::warn!("failed to unregister all shortcuts: {err}");
        }
        self.registry.clear();
        Ok(())
    }

    fn inject_text(&self, _text: &str) -> AppResult<()> {
        // P0.5 will implement clipboard save → write → simulate Cmd+V → restore.
        unimplemented!("inject_text — P0.5")
    }

    fn store_secret(&self, _key: &str, _value: &str) -> AppResult<()> {
        // P1 will call macOS Keychain Services via `security-framework`.
        unimplemented!("store_secret — P1")
    }

    fn read_secret(&self, _key: &str) -> AppResult<String> {
        unimplemented!("read_secret — P1")
    }
}
