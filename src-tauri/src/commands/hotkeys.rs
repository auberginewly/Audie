use crate::error::AppResult;
use tauri::AppHandle;
#[tauri::command]
pub(crate) fn begin_trigger_capture(app: AppHandle) -> AppResult<()> {
    crate::hotkeys::begin_trigger_capture(app)
}
#[tauri::command]
pub(crate) fn end_trigger_capture(app: AppHandle) -> AppResult<()> {
    crate::hotkeys::end_trigger_capture(app)
}
