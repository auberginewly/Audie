use std::time::Duration;

use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::error::{AppError, AppResult};

pub(super) fn inject_text(
    app: &AppHandle,
    text: &str,
    focus_target_pid: Option<i32>,
) -> AppResult<()> {
    // Clipboard method: most compatible across apps. The transcript deliberately
    // remains on the pasteboard as a manual Cmd+V fallback if Accessibility is
    // missing or stale.
    app.clipboard()
        .write_text(text.to_string())
        .map_err(|err| AppError::Inject(format!("clipboard write failed: {err}")))?;

    if !preflight_post_event_access() {
        request_post_event_access();
        return Err(AppError::Permission(
            "辅助功能权限未授予，文字已复制到剪贴板，可手动粘贴；请到 系统设置 → 隐私与安全性 → 辅助功能 启用 Audie".into(),
        ));
    }

    let restored = match focus_target_pid {
        Some(pid) => restore_focus_if_stolen(pid),
        None => false,
    };
    std::thread::sleep(Duration::from_millis(if restored { 50 } else { 20 }));
    simulate_cmd_v()
}

pub(super) fn read_selection(app: &AppHandle) -> Option<String> {
    const SENTINEL: &str = "\u{2063}AUDIE_SEL_PROBE\u{2063}";
    let original = app.clipboard().read_text().ok();
    if app.clipboard().write_text(SENTINEL.to_string()).is_err() {
        return None;
    }
    if simulate_cmd_c().is_err() {
        restore_clipboard(app, original);
        return None;
    }

    std::thread::sleep(Duration::from_millis(120));
    match app.clipboard().read_text() {
        Ok(text) if text != SENTINEL => Some(text),
        _ => {
            restore_clipboard(app, original);
            None
        }
    }
}

#[allow(deprecated, unexpected_cfgs)]
pub(super) fn current_frontmost_pid() -> Option<i32> {
    use tauri_nspanel::cocoa::base::{id, nil};
    use tauri_nspanel::objc::{class, msg_send, sel, sel_impl};
    // SAFETY: read-only AppKit class accessors; NSWorkspace is process-wide and
    // safe to query off the main thread.
    unsafe {
        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace == nil {
            return None;
        }
        let app: id = msg_send![workspace, frontmostApplication];
        if app == nil {
            return None;
        }
        let pid: i32 = msg_send![app, processIdentifier];
        Some(pid)
    }
}

pub(super) fn preflight_post_event_access() -> bool {
    // SAFETY: parameterless C function from ApplicationServices.
    unsafe { CGPreflightPostEventAccess() }
}

pub(super) fn request_post_event_access() {
    // SAFETY: parameterless C function from ApplicationServices.
    unsafe {
        let _ = CGRequestPostEventAccess();
    }
}

fn simulate_cmd_v() -> AppResult<()> {
    post_cmd_key(9, "V")
}

fn simulate_cmd_c() -> AppResult<()> {
    post_cmd_key(8, "C")
}

fn post_cmd_key(keycode: CGKeyCode, label: &str) -> AppResult<()> {
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .map_err(|()| AppError::Inject("CGEventSource creation failed".into()))?;

    let key_down = CGEvent::new_keyboard_event(source.clone(), keycode, true)
        .map_err(|()| AppError::Inject(format!("CGEvent Cmd+{label} key-down failed")))?;
    key_down.set_flags(CGEventFlags::CGEventFlagCommand);
    key_down.post(CGEventTapLocation::HID);

    let key_up = CGEvent::new_keyboard_event(source, keycode, false)
        .map_err(|()| AppError::Inject(format!("CGEvent Cmd+{label} key-up failed")))?;
    key_up.set_flags(CGEventFlags::CGEventFlagCommand);
    key_up.post(CGEventTapLocation::HID);

    Ok(())
}

fn restore_clipboard(app: &AppHandle, original: Option<String>) {
    if let Some(text) = original {
        let _ = app.clipboard().write_text(text);
    }
}

#[allow(deprecated, unexpected_cfgs)]
fn restore_focus_if_stolen(target_pid: i32) -> bool {
    use tauri_nspanel::cocoa::base::{id, nil, BOOL};
    use tauri_nspanel::objc::{class, msg_send, sel, sel_impl};
    let own_pid = std::process::id() as i32;
    // SAFETY: read-only accessors + activateWithOptions:0 (a cross-process request,
    // == Voxt's activate(options: [])); none mutate our own view hierarchy.
    unsafe {
        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace == nil {
            return false;
        }
        let frontmost: id = msg_send![workspace, frontmostApplication];
        if frontmost == nil {
            return false;
        }
        let frontmost_pid: i32 = msg_send![frontmost, processIdentifier];
        if frontmost_pid != own_pid {
            return false;
        }
        let target: id = msg_send![
            class!(NSRunningApplication),
            runningApplicationWithProcessIdentifier: target_pid
        ];
        if target == nil {
            return false;
        }
        let activated: BOOL = msg_send![target, activateWithOptions: 0u64];
        activated
    }
}

extern "C" {
    fn CGPreflightPostEventAccess() -> bool;
    fn CGRequestPostEventAccess() -> bool;
}
