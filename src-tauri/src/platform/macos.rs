// macOS implementation of trait Platform.
//
// Platform-facing behavior stays here; OS-specific implementation details live in
// private child modules so this file remains the public macOS surface.

use parking_lot::Mutex;
use tauri::AppHandle;

use super::{HotkeyCallback, HotkeySlot, Platform};
use crate::error::AppResult;

mod audio_device;
mod capture;
mod clipboard;
mod dock;
mod hotkey;
mod keychain;
mod language;
mod permissions;

#[derive(Default)]
pub struct MacosPlatform {
    // The app that was frontmost when recording started. Clicking an overlay
    // button makes Audie frontmost, so inject restores this app before Cmd+V.
    focus_target_pid: Mutex<Option<i32>>,
    // P3.8 dev-only trigger-key probe.
    probe: Mutex<Option<hotkey::EventTapHandle>>,
    // P3.9 production trigger (HotkeySlot::Primary).
    trigger: Mutex<Option<hotkey::EventTapHandle>>,
    // Compose mode's second independent trigger.
    compose_trigger: Mutex<Option<hotkey::EventTapHandle>>,
    // P3.10 settings recorder capture tap.
    capture: Mutex<Option<hotkey::EventTapHandle>>,
}

impl MacosPlatform {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Platform for MacosPlatform {
    fn register_hotkey(
        &self,
        _app: &AppHandle,
        slot: HotkeySlot,
        combo: &str,
        callback: HotkeyCallback,
    ) -> AppResult<()> {
        let spec = hotkey::parse_trigger(combo)?;
        self.start_trigger(slot, spec, callback)
    }

    fn unregister_hotkey(&self, _app: &AppHandle, slot: HotkeySlot) {
        self.stop_trigger(slot);
    }

    fn unregister_all_hotkeys(&self, _app: &AppHandle) -> AppResult<()> {
        self.stop_trigger(HotkeySlot::Primary);
        self.stop_trigger(HotkeySlot::Compose);
        Ok(())
    }

    fn inject_text(&self, app: &AppHandle, text: &str) -> AppResult<()> {
        clipboard::inject_text(app, text, *self.focus_target_pid.lock())
    }

    fn read_selection(&self, app: &AppHandle) -> Option<String> {
        clipboard::read_selection(app)
    }

    fn capture_focus_target(&self) {
        let pid = clipboard::current_frontmost_pid();
        *self.focus_target_pid.lock() = pid;
        log::debug!("capture_focus_target: frontmost pid = {pid:?}");
    }

    fn preferred_input_device_name(&self) -> Option<String> {
        audio_device::pick_reliable_input()
    }

    fn system_language(&self) -> Option<String> {
        language::system_language_label()
    }

    fn set_dock_visible(&self, app: &AppHandle, visible: bool) -> AppResult<()> {
        dock::set_visible(app, visible)
    }

    fn apply_app_icon(&self, app: &AppHandle) -> AppResult<()> {
        dock::apply_app_icon(app)
    }

    fn ensure_microphone_permission(&self) -> bool {
        permissions::ensure_microphone_permission()
    }

    fn store_secret(&self, key: &str, value: &str) -> AppResult<()> {
        keychain::store_secret(key, value)
    }

    fn has_secret(&self, key: &str) -> AppResult<bool> {
        keychain::has_secret(key)
    }

    fn read_secret(&self, key: &str) -> AppResult<String> {
        keychain::read_secret(key)
    }

    fn delete_secret(&self, key: &str) -> AppResult<()> {
        keychain::delete_secret(key)
    }

    fn input_monitoring_status(&self) -> bool {
        permissions::input_monitoring_granted()
    }

    fn request_input_monitoring(&self) {
        permissions::request_input_monitoring_access();
    }

    fn microphone_status(&self) -> bool {
        permissions::microphone_status()
    }

    fn request_microphone(&self) {
        permissions::request_microphone();
    }

    fn accessibility_status(&self) -> bool {
        permissions::accessibility_status()
    }

    fn request_accessibility(&self) {
        permissions::request_accessibility();
    }

    fn start_trigger_capture(&self, app: &AppHandle) -> AppResult<()> {
        self.start_trigger_capture(app)
    }

    fn stop_trigger_capture(&self) {
        self.stop_trigger_capture();
    }

    fn start_trigger_probe(&self, app: &AppHandle) -> AppResult<()> {
        self.start_trigger_probe(app)
    }

    fn stop_trigger_probe(&self) -> AppResult<()> {
        self.stop_trigger_probe()
    }
}
