use tauri_plugin_macos_permissions::{check_microphone_permission, request_microphone_permission};

// IOKit HID access for Input Monitoring (§5.8 option B: actively prompt).
#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOHIDCheckAccess(request: u32) -> u32;
    fn IOHIDRequestAccess(request: u32) -> bool;
}

// <IOKit/hid/IOHIDLib.h>: kIOHIDRequestTypeListenEvent = 1; kIOHIDAccessTypeGranted = 0.
const K_IOHID_REQUEST_TYPE_LISTEN_EVENT: u32 = 1;
const K_IOHID_ACCESS_TYPE_GRANTED: u32 = 0;

pub(super) fn input_monitoring_granted() -> bool {
    // SAFETY: C call from IOKit with a constant request type, returns an enum int.
    unsafe { IOHIDCheckAccess(K_IOHID_REQUEST_TYPE_LISTEN_EVENT) == K_IOHID_ACCESS_TYPE_GRANTED }
}

pub(super) fn request_input_monitoring_access() {
    // SAFETY: Shows the system prompt when undecided; no-op once decided.
    unsafe {
        IOHIDRequestAccess(K_IOHID_REQUEST_TYPE_LISTEN_EVENT);
    }
}

pub(super) fn ensure_microphone_permission() -> bool {
    // `request` triggers requestAccess(.audio): it shows the prompt only when
    // status is NotDetermined and is a no-op when already decided.
    if let Err(err) = tauri::async_runtime::block_on(request_microphone_permission()) {
        log::warn!("request microphone permission: {err}");
    }
    tauri::async_runtime::block_on(check_microphone_permission())
}

pub(super) fn microphone_status() -> bool {
    // Presence check only — no prompt.
    tauri::async_runtime::block_on(check_microphone_permission())
}

pub(super) fn request_microphone() {
    if let Err(err) = tauri::async_runtime::block_on(request_microphone_permission()) {
        log::warn!("request microphone permission: {err}");
    }
}
