use parking_lot::Mutex;
use tauri::AppHandle;

use super::{HotkeyCallback, HotkeySlot, Platform};
use crate::error::AppResult;

mod clipboard;
mod credential;
mod hotkey;
mod language;

#[derive(Default)]
pub struct WindowsPlatform {
    primary_hotkey: Mutex<Option<hotkey::HotkeyHandle>>,
    compose_hotkey: Mutex<Option<hotkey::HotkeyHandle>>,
}

impl WindowsPlatform {
    pub fn new() -> Self {
        Self::default()
    }

    fn slot_handle(&self, slot: HotkeySlot) -> &Mutex<Option<hotkey::HotkeyHandle>> {
        match slot {
            HotkeySlot::Primary => &self.primary_hotkey,
            HotkeySlot::Compose => &self.compose_hotkey,
        }
    }
}

impl Platform for WindowsPlatform {
    fn register_hotkey(
        &self,
        _app: &AppHandle,
        slot: HotkeySlot,
        combo: &str,
        callback: HotkeyCallback,
    ) -> AppResult<()> {
        let slot_handle = self.slot_handle(slot);
        if slot_handle.lock().is_some() {
            return Ok(());
        }
        let handle = hotkey::register(slot, combo, callback)?;
        *slot_handle.lock() = Some(handle);
        Ok(())
    }

    fn unregister_hotkey(&self, _app: &AppHandle, slot: HotkeySlot) {
        let _ = self.slot_handle(slot).lock().take();
    }

    fn unregister_all_hotkeys(&self, _app: &AppHandle) -> AppResult<()> {
        let _ = self.primary_hotkey.lock().take();
        let _ = self.compose_hotkey.lock().take();
        Ok(())
    }

    fn inject_text(&self, app: &AppHandle, text: &str) -> AppResult<()> {
        clipboard::inject_text(app, text)
    }

    fn ensure_microphone_permission(&self) -> bool {
        true
    }

    fn microphone_status(&self) -> bool {
        true
    }

    fn accessibility_status(&self) -> bool {
        true
    }

    fn input_monitoring_status(&self) -> bool {
        true
    }

    fn system_language(&self) -> Option<String> {
        language::system_language_label()
    }

    fn store_secret(&self, key: &str, value: &str) -> AppResult<()> {
        credential::store_secret(key, value)
    }

    fn has_secret(&self, key: &str) -> AppResult<bool> {
        credential::has_secret(key)
    }

    fn read_secret(&self, key: &str) -> AppResult<String> {
        credential::read_secret(key)
    }

    fn delete_secret(&self, key: &str) -> AppResult<()> {
        credential::delete_secret(key)
    }
}
