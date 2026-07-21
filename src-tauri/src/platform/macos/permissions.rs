use block2::RcBlock;
use objc2::{class, msg_send, runtime::Bool};
use objc2_foundation::NSString;
use tauri_plugin_macos_permissions::{
    check_accessibility_permission, check_microphone_permission, request_accessibility_permission,
};

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGPreflightListenEventAccess() -> bool;
    fn CGRequestListenEventAccess() -> bool;
}

pub(super) fn input_monitoring_granted() -> bool {
    // SAFETY: CoreGraphics exposes this no-argument preflight as a process-wide
    // TCC query. It returns a boolean and does not write through Rust memory.
    unsafe { CGPreflightListenEventAccess() }
}

pub(super) fn request_input_monitoring_access() {
    // SAFETY: CoreGraphics exposes this no-argument request to register the
    // current process with the ListenEvent TCC service and show its system UI.
    unsafe {
        CGRequestListenEventAccess();
    }
}

pub(super) fn ensure_microphone_permission() -> bool {
    if !microphone_status() {
        request_microphone();
    }
    microphone_status()
}

pub(super) fn microphone_status() -> bool {
    // Presence check only — no prompt.
    tauri::async_runtime::block_on(check_microphone_permission())
}

pub(super) fn request_microphone() {
    let media_type = NSString::from_str("soun");
    let completion = RcBlock::new(|granted: Bool| {
        log::info!(
            "microphone permission request completed: {}",
            granted.as_bool()
        );
    });

    // SAFETY: [Category 8 — FFI boundary] `AVCaptureDevice` owns the documented
    // class method, `media_type` is a live NSString containing AVMediaTypeAudio,
    // and `RcBlock` supplies a non-null Objective-C block with the required
    // `(BOOL) -> void` ABI. AVFoundation copies the asynchronous completion block.
    unsafe {
        let _: () = msg_send![
            class!(AVCaptureDevice),
            requestAccessForMediaType: &*media_type,
            completionHandler: &*completion
        ];
    }
}

pub(super) fn accessibility_status() -> bool {
    tauri::async_runtime::block_on(check_accessibility_permission())
}

pub(super) fn request_accessibility() {
    tauri::async_runtime::block_on(request_accessibility_permission());
}
