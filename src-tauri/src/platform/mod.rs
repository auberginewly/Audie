// Platform abstraction (PROJECT_SPEC.md §3.4 / §6.3).
//
// All `#[cfg(target_os)]` lives behind this trait. Managers MUST go through
// `current_platform()` — they must never import macos.rs / windows.rs directly.
// Think of Platform as the border around OS side effects: hotkeys, paste
// injection, permission checks, device preferences, and secrets.
//
// P0.1 only uses `register_hotkey` / `unregister_all_hotkeys`. inject_text lands
// in P0.4; keychain methods are filled in during P1.

use tauri::AppHandle;

use crate::error::AppResult;

/// Callback fired on a trigger tap (fn / single / combo). Tap-toggle only — there
/// is no separate release event since the control model is press-to-toggle.
pub type HotkeyCallback = Box<dyn Fn() + Send + Sync + 'static>;

#[allow(dead_code)] // Trait surface defined whole per SPEC §3.4; later slices fill in callers.
pub trait Platform: Send + Sync {
    /// Register the trigger key (e.g. "Fn", "F13", "Ctrl+Shift+Space"). The
    /// callback fires once per tap (press-to-toggle).
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

    /// Return the device name to prefer over the system default input. Used to
    /// dodge Bluetooth-mic gotchas (e.g. AirPods on A2DP read literal zeros until
    /// macOS deigns to flip to HFP, which also nukes system audio quality).
    /// `None` means "no opinion — caller should use the system default".
    /// The caller is expected to match cpal's `Device::name()` against this.
    /// P3 设备选择切片会把这条逻辑搬到设置页。
    fn preferred_input_device_name(&self) -> Option<String> {
        None
    }

    /// Remember the frontmost app at recording start so injection can restore
    /// focus to it if an overlay-button click later steals key focus to us. macOS
    /// overrides this; other platforms don't fight a non-activating panel, so the
    /// default no-op keeps their impl unchanged (fe.8c).
    fn capture_focus_target(&self) {}

    /// P3.8 dev-only trigger-key probe: start a listen-only `CGEventTap` and emit
    /// `trigger-probe-key` for every key/flags event, so we can verify fn + custom
    /// single/combo keys reach us before P3.9 swaps the real trigger. Default no-op
    /// keeps non-macOS impls unchanged (it's a macOS-only dev probe). SPEC §5.8.
    fn start_trigger_probe(&self, _app: &AppHandle) -> AppResult<()> {
        Ok(())
    }

    fn stop_trigger_probe(&self) -> AppResult<()> {
        Ok(())
    }

    /// P3.9 — Input Monitoring (macOS): the default trigger (fn) and the CGEventTap
    /// need it. Default impls treat it as granted/no-op for platforms without the
    /// concept, so non-macOS never gates on it.
    fn input_monitoring_status(&self) -> bool {
        true
    }
    fn request_input_monitoring(&self) {}

    /// P3.12 — Microphone (macOS TCC). `status` reads without prompting (onboarding
    /// polls it); `request` shows the system prompt. Default granted/no-op so
    /// non-macOS never gates here.
    fn microphone_status(&self) -> bool {
        true
    }
    fn request_microphone(&self) {}

    /// P3.12 — Accessibility / post-event access (macOS). Injection's synthetic Cmd+V
    /// needs it. `status` preflights without prompting; `request` shows the prompt.
    /// Default granted/no-op for non-macOS.
    fn accessibility_status(&self) -> bool {
        true
    }
    fn request_accessibility(&self) {}

    /// P3.10 — start/stop a listen-only capture tap for the Settings recorder. macOS
    /// emits `trigger-captured` / `trigger-capture-rejected`; default no-op elsewhere.
    fn start_trigger_capture(&self, _app: &AppHandle) -> AppResult<()> {
        Ok(())
    }
    fn stop_trigger_capture(&self) {}

    /// P1 — system keychain (macOS Keychain Services / Windows Credential Manager).
    fn store_secret(&self, key: &str, value: &str) -> AppResult<()>;
    fn has_secret(&self, key: &str) -> AppResult<bool>;
    fn read_secret(&self, key: &str) -> AppResult<String>;
    fn delete_secret(&self, key: &str) -> AppResult<()>;
}

#[cfg(target_os = "macos")]
pub fn current_platform() -> Box<dyn Platform> {
    Box::new(macos::MacosPlatform::new())
}

#[cfg(target_os = "windows")]
pub fn current_platform() -> Box<dyn Platform> {
    // Windows keeps the same trait shape so the Rust pipeline remains portable;
    // the concrete Win32/Credential Manager calls are intentionally deferred to P4.
    Box::new(windows::WindowsPlatform::new())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn current_platform() -> Box<dyn Platform> {
    panic!("unsupported platform — Audie targets macOS (P0–P3) and Windows (P4).")
}

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;
