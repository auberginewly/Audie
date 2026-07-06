use objc2::class;
use objc2::msg_send;
use objc2::runtime::{AnyObject, Bool};
use std::sync::mpsc;
use tauri::AppHandle;

use crate::error::{AppError, AppResult};

pub(super) fn set_visible(app: &AppHandle, visible: bool) -> AppResult<()> {
    let (tx, rx) = mpsc::channel();
    app.run_on_main_thread(move || {
        let applied = unsafe {
            let ns_app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
            if ns_app.is_null() {
                let _ = tx.send(false);
                return;
            }
            let policy = if visible { 0usize } else { 1usize };
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
