// Windows implementation of trait Platform.
// PROJECT_SPEC.md §3.4 — macOS first, Windows in P4. Everything `unimplemented!()`.

use tauri::AppHandle;

use super::{HotkeyCallback, Platform};
use crate::error::AppResult;

pub struct WindowsPlatform;

impl WindowsPlatform {
    pub fn new() -> Self {
        Self
    }
}

impl Platform for WindowsPlatform {
    fn register_hotkey(
        &self,
        _app: &AppHandle,
        _combo: &str,
        _callback: HotkeyCallback,
    ) -> AppResult<()> {
        unimplemented!("Windows hotkey — P4")
    }

    fn unregister_all_hotkeys(&self, _app: &AppHandle) -> AppResult<()> {
        unimplemented!("Windows unregister — P4")
    }

    fn inject_text(&self, _text: &str) -> AppResult<()> {
        unimplemented!("Windows inject — P4")
    }

    fn store_secret(&self, _key: &str, _value: &str) -> AppResult<()> {
        unimplemented!("Windows credential manager — P4")
    }

    fn read_secret(&self, _key: &str) -> AppResult<String> {
        unimplemented!("Windows credential manager — P4")
    }
}
