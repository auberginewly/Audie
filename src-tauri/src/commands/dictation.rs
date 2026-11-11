use tauri::{AppHandle, Emitter, Manager};
#[tauri::command]
pub(crate) fn confirm_recording(app: AppHandle) {
    crate::pipeline::confirm_recording(app)
}
#[tauri::command]
pub(crate) fn cancel_recording(app: AppHandle) {
    crate::pipeline::cancel_recording(app)
}
#[tauri::command]
pub(crate) fn undo_last(app: AppHandle) {
    crate::pipeline::undo_last(app)
}
#[tauri::command]
pub(crate) fn retry_last(app: AppHandle) {
    crate::pipeline::retry_last(app)
}
#[tauri::command]
pub(crate) fn insert_raw_last(app: AppHandle) {
    crate::pipeline::insert_raw_last(app)
}

/// 去设置 on the polish-unavailable toast — surface the main window and ask it to
/// open Settings (the overlay is a separate webview, so it signals via an event).
#[tauri::command]
pub(crate) fn open_main_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    let _ = app.emit("open-settings", ());
}
