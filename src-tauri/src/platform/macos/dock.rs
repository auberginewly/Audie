use objc2::class;
use objc2::msg_send;
use objc2::runtime::{AnyObject, Bool};
use std::ffi::c_void;
use std::sync::mpsc;
use tauri::AppHandle;

use crate::error::{AppError, AppResult};

const APP_ICON_PNG: &[u8] = include_bytes!("../../../icons/icon.png");

pub(super) fn set_visible(app: &AppHandle, visible: bool) -> AppResult<()> {
    let (tx, rx) = mpsc::channel();
    app.run_on_main_thread(move || {
        let applied = unsafe {
            let ns_app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
            if ns_app.is_null() {
                let _ = tx.send(false);
                return;
            }
            let policy = if visible { 0isize } else { 1isize };
            let ok: Bool = msg_send![ns_app, setActivationPolicy: policy];
            ok.as_bool()
        };
        let _ = tx.send(applied);
    })
    .map_err(|err| AppError::Internal(format!("run_on_main_thread: {err}")))?;

    match rx.recv() {
        Ok(true) => Ok(()),
        Ok(false) => Err(AppError::Internal("set Dock visibility failed".into())),
        Err(err) => Err(AppError::Internal(format!("set Dock visibility: {err}"))),
    }
}

pub(super) fn apply_app_icon(app: &AppHandle) -> AppResult<()> {
    let (tx, rx) = mpsc::channel();
    app.run_on_main_thread(move || {
        // SAFETY: Category 8 - FFI boundary. This runs on Tauri's main thread,
        // calls standard AppKit/Foundation constructors, checks every nullable
        // Objective-C object before use, and passes a pointer/length pair backed
        // by a static byte slice that outlives the copied NSData.
        let applied = unsafe {
            let data: *mut AnyObject = msg_send![
                class!(NSData),
                dataWithBytes: APP_ICON_PNG.as_ptr().cast::<c_void>(),
                length: APP_ICON_PNG.len()
            ];
            if data.is_null() {
                let _ = tx.send(false);
                return;
            }

            let image_alloc: *mut AnyObject = msg_send![class!(NSImage), alloc];
            if image_alloc.is_null() {
                let _ = tx.send(false);
                return;
            }

            let image: *mut AnyObject = msg_send![image_alloc, initWithData: data];
            if image.is_null() {
                let _ = tx.send(false);
                return;
            }

            let ns_app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
            if ns_app.is_null() {
                let _: () = msg_send![image, release];
                let _ = tx.send(false);
                return;
            }

            let _: () = msg_send![ns_app, setApplicationIconImage: image];
            let _: () = msg_send![image, release];
            true
        };
        let _ = tx.send(applied);
    })
    .map_err(|err| AppError::Internal(format!("run_on_main_thread: {err}")))?;

    match rx.recv() {
        Ok(true) => Ok(()),
        Ok(false) => Err(AppError::Internal("set app icon failed".into())),
        Err(err) => Err(AppError::Internal(format!("set app icon: {err}"))),
    }
}
