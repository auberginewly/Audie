// Platform abstraction (PROJECT_SPEC.md §3.4 / §6.3).
//
// All `#[cfg(target_os)]` lives behind this trait. Managers MUST go through
// `current_platform()` — they must never import macos.rs / windows.rs directly.
//
// P0.1 only uses `register_hotkey` / `unregister_all_hotkeys`. inject_text lands
// in P0.4; keychain methods stay stubbed until P1.

use std::{collections::HashMap, sync::Arc};

use parking_lot::Mutex;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::Shortcut;

use crate::error::AppResult;

/// Callback fired on hotkey press / release.
pub type HotkeyCallback = Box<dyn Fn(HotkeyEvent) + Send + Sync + 'static>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyEvent {
    Pressed,
    Released,
}

/// Shared dispatch table — the global-shortcut plugin's single `with_handler`
/// closure dispatches into this registry by looking up the parsed `Shortcut`.
/// `MacosPlatform::register_hotkey` inserts; the plugin handler reads.
#[derive(Default)]
pub struct HotkeyRegistry {
    callbacks: Mutex<HashMap<Shortcut, HotkeyCallback>>,
}

impl HotkeyRegistry {
    pub fn insert(&self, shortcut: Shortcut, callback: HotkeyCallback) {
        self.callbacks.lock().insert(shortcut, callback);
    }

    pub fn dispatch(&self, shortcut: &Shortcut, event: HotkeyEvent) {
        if let Some(callback) = self.callbacks.lock().get(shortcut) {
            callback(event);
        }
    }

    #[allow(dead_code)] // Used by Platform::unregister_all_hotkeys in later slices.
    pub fn clear(&self) {
        self.callbacks.lock().clear();
    }
}

#[allow(dead_code)] // Trait surface defined whole per SPEC §3.4; later slices fill in callers.
pub trait Platform: Send + Sync {
    /// Register a global hotkey combo (e.g. "Ctrl+Shift+Space"). The callback
    /// fires on both press and release so press-to-talk works.
    fn register_hotkey(
        &self,
        app: &AppHandle,
        combo: &str,
        callback: HotkeyCallback,
    ) -> AppResult<()>;

    fn unregister_all_hotkeys(&self, app: &AppHandle) -> AppResult<()>;

    /// P0.4 — clipboard-method injection at the current caret: save the current
    /// clipboard, write `text`, simulate Cmd+V, restore. Needs `app` for the
    /// clipboard plugin. Per §6.3 the OS-specific keystroke stays in this layer.
    fn inject_text(&self, app: &AppHandle, text: &str) -> AppResult<()>;

    /// Ensure microphone (TCC) access before recording, returning whether it's
    /// granted. On first run this shows the system prompt; if already denied it
    /// returns false without a dialog (macOS only asks once). Gating here means a
    /// denial flashes the capsule red instead of silently capturing zeros
    /// (§3.7 Permission). SPEC §3.5 `request_permission`.
    fn ensure_microphone_permission(&self) -> bool;

    /// P1 — system keychain (macOS Keychain Services / Windows Credential Manager).
    fn store_secret(&self, key: &str, value: &str) -> AppResult<()>;
    fn read_secret(&self, key: &str) -> AppResult<String>;
}

#[cfg(target_os = "macos")]
pub fn current_platform(registry: Arc<HotkeyRegistry>) -> Box<dyn Platform> {
    Box::new(macos::MacosPlatform::new(registry))
}

#[cfg(target_os = "windows")]
pub fn current_platform(_registry: Arc<HotkeyRegistry>) -> Box<dyn Platform> {
    Box::new(windows::WindowsPlatform::new())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn current_platform(_registry: Arc<HotkeyRegistry>) -> Box<dyn Platform> {
    panic!("unsupported platform — Audie targets macOS (P0–P3) and Windows (P4).")
}

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;
