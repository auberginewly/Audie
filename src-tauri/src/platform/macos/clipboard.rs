use std::time::Duration;

use core_foundation::base::{CFType, CFTypeID, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::string::{CFString, CFStringRef};

use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::error::{AppError, AppResult};

use super::permissions;

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
    selection_or_copy(focused_selection(), || read_selection_by_copy(app))
}

fn read_selection_by_copy(app: &AppHandle) -> Option<String> {
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
    permissions::accessibility_status()
}

pub(super) fn request_post_event_access() {
    permissions::request_accessibility();
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

// 系统已确认的空选区也有意义：不能继续 Cmd+C，把编辑器复制整行误当成选区。
fn selection_or_copy(
    known: Option<String>,
    copy: impl FnOnce() -> Option<String>,
) -> Option<String> {
    known.or_else(copy)
}

// 只读取焦点控件的选中文字，不读取整篇文档或其他应用内容。
fn focused_selection() -> Option<String> {
    let pid = current_frontmost_pid()?;
    // SAFETY: Create 返回 +1 引用，由 CFType 负责释放；以下 API 只在 macOS 编译。
    let app = unsafe { AXUIElementCreateApplication(pid) };
    if app.is_null() {
        return None;
    }
    let app = unsafe { CFType::wrap_under_create_rule(app) };
    // 热键回调不能长时间等待无响应的其他应用。
    unsafe { AXUIElementSetMessagingTimeout(app.as_CFTypeRef(), 0.05) };
    if let Some(text) = ax_selected_text(&app) {
        return Some(text);
    }
    let manual = CFString::new("AXManualAccessibility");
    let enabled = CFBoolean::true_value();
    // Electron 按需建立 AX 树；这是其官方辅助功能开关，不更改编辑器偏好设置。
    let electron = unsafe {
        AXUIElementSetAttributeValue(
            app.as_CFTypeRef(),
            manual.as_concrete_TypeRef(),
            enabled.as_CFTypeRef(),
        ) == 0
    };
    if electron {
        // 首次建树可能尚未完成。无法确认选区时保守听写，不能回退 Cmd+C 复制整行。
        Some(ax_selected_text(&app).unwrap_or_default())
    } else {
        None
    }
}

fn ax_selected_text(app: &CFType) -> Option<String> {
    let focused = ax_attribute(app, "AXFocusedUIElement")?;
    // SAFETY: 只有确认类型为 AXUIElement 后，才能向该对象发送 AX 请求。
    if focused.type_of() != unsafe { AXUIElementGetTypeID() } {
        return None;
    }
    unsafe { AXUIElementSetMessagingTimeout(focused.as_CFTypeRef(), 0.05) };
    ax_attribute(&focused, "AXSelectedText")?
        .downcast::<CFString>()
        .map(|text| text.to_string())
}

fn ax_attribute(element: &CFType, name: &str) -> Option<CFType> {
    let name = CFString::new(name);
    let mut value = std::ptr::null();
    // SAFETY: element/name 在调用期间存活，out 指针有效；成功的 Copy 引用交给 CFType 释放。
    let status = unsafe {
        AXUIElementCopyAttributeValue(
            element.as_CFTypeRef(),
            name.as_concrete_TypeRef(),
            &mut value,
        )
    };
    if status != 0 || value.is_null() {
        return None;
    }
    Some(unsafe { CFType::wrap_under_create_rule(value) })
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> CFTypeRef;
    fn AXUIElementGetTypeID() -> CFTypeID;
    fn AXUIElementSetMessagingTimeout(element: CFTypeRef, seconds: f32) -> i32;
    fn AXUIElementSetAttributeValue(element: CFTypeRef, name: CFStringRef, value: CFTypeRef)
        -> i32;
    fn AXUIElementCopyAttributeValue(
        element: CFTypeRef,
        name: CFStringRef,
        value: *mut CFTypeRef,
    ) -> i32;
}

#[cfg(test)]
mod tests;
