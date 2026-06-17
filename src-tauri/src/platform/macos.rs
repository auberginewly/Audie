// macOS implementation of trait Platform.
//
// P0.1: hotkey via tauri-plugin-global-shortcut. The callback is parked in the
// shared HotkeyRegistry — the plugin's `with_handler` (built in lib.rs) is the
// single entry that dispatches into the registry.
//
// P0.4 adds clipboard-method inject (save → write → Cmd+V → restore). P1 will
// add Keychain Services calls.

use std::sync::Arc;
use std::time::Duration;

use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;
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

    fn inject_text(&self, app: &AppHandle, text: &str) -> AppResult<()> {
        // Clipboard method: most compatible across apps. Save the user's current
        // clipboard, paste our text, then restore. `read_text` fails when the
        // clipboard holds non-text (e.g. an image) — treat that as "nothing to
        // restore" rather than an error.
        let original = app.clipboard().read_text().ok();

        app.clipboard()
            .write_text(text.to_string())
            .map_err(|err| AppError::Inject(format!("clipboard write failed: {err}")))?;

        // Give the pasteboard a beat to settle before the synthetic paste.
        std::thread::sleep(Duration::from_millis(20));
        simulate_cmd_v()?;

        // The frontmost app reads the pasteboard asynchronously on Cmd+V;
        // restoring too early clobbers our text before it lands.
        std::thread::sleep(Duration::from_millis(120));
        if let Some(prev) = original {
            if let Err(err) = app.clipboard().write_text(prev) {
                log::warn!("failed to restore clipboard after inject: {err}");
            }
        }

        Ok(())
    }

    fn store_secret(&self, _key: &str, _value: &str) -> AppResult<()> {
        // P1 will call macOS Keychain Services via `security-framework`.
        unimplemented!("store_secret — P1")
    }

    fn read_secret(&self, _key: &str) -> AppResult<String> {
        unimplemented!("read_secret — P1")
    }
}

/// Post a synthetic Cmd+V. Requires Accessibility permission — without it macOS
/// silently drops the events (post() still returns no error), so paste never
/// lands. Granting the permission is the P0.6 error-flow / P3 onboarding work.
fn simulate_cmd_v() -> AppResult<()> {
    const KEY_V: CGKeyCode = 9; // kVK_ANSI_V

    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .map_err(|()| AppError::Inject("CGEventSource creation failed".into()))?;

    let key_down = CGEvent::new_keyboard_event(source.clone(), KEY_V, true)
        .map_err(|()| AppError::Inject("CGEvent key-down failed".into()))?;
    key_down.set_flags(CGEventFlags::CGEventFlagCommand);
    key_down.post(CGEventTapLocation::HID);

    let key_up = CGEvent::new_keyboard_event(source, KEY_V, false)
        .map_err(|()| AppError::Inject("CGEvent key-up failed".into()))?;
    key_up.set_flags(CGEventFlags::CGEventFlagCommand);
    key_up.post(CGEventTapLocation::HID);

    Ok(())
}
