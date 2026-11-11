use crate::error::AppError;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
/// Dictation history for the History screen (newest first, capped). Reads straight
/// from the HistoryManager's SQLite store (§6.1).
#[tauri::command]
pub fn list_history(
    app: AppHandle,
) -> Result<Vec<crate::managers::history::HistoryEntry>, AppError> {
    app.state::<Arc<crate::managers::history::HistoryManager>>()
        .list()
}

#[tauri::command]
pub fn delete_history_entry(app: AppHandle, id: i64) -> Result<(), AppError> {
    let history = app.state::<Arc<crate::managers::history::HistoryManager>>();
    history.delete_entry(&app, id)
}

#[tauri::command]
pub fn clear_history(app: AppHandle) -> Result<(), AppError> {
    let history = app.state::<Arc<crate::managers::history::HistoryManager>>();
    history.clear(&app)
}

/// All-time usage totals for the Home dashboard (§5.4 / release-v1 #6). The
/// frontend derives the four cards (time / words / time-saved / speed) from these.
#[tauri::command]
pub fn get_usage_stats(app: AppHandle) -> Result<crate::managers::history::UsageStats, AppError> {
    app.state::<Arc<crate::managers::history::HistoryManager>>()
        .usage_stats()
}

/// Per-day 口述/AI 产出 for the Home 近 N 天 chart. Only days with rows are
/// returned; the frontend zero-fills the rest of the window.
#[tauri::command]
pub fn get_daily_usage(
    app: AppHandle,
    days: u32,
) -> Result<Vec<crate::managers::history::DailyUsage>, AppError> {
    app.state::<Arc<crate::managers::history::HistoryManager>>()
        .daily_usage(days)
}

use crate::error::AppResult;
use crate::managers::{enhance::EnhanceManager, history::HistoryManager};
use crate::pipeline::reenhance_config;
/// History 重试 — re-run the current LLM on a stored entry's transcript and save the
/// enhanced version. No audio needed (re-enhance, not re-transcribe). Errors surface
/// to the caller so the History screen can toast them; the success path emits
/// `history-updated` so the list re-fetches and shows the new 润色 box.
#[tauri::command]
pub(crate) fn reenhance_history_entry(app: AppHandle, id: i64) -> AppResult<()> {
    let history = app.state::<Arc<HistoryManager>>();
    let raw = history
        .raw_text_of(id)?
        .ok_or_else(|| AppError::Internal("history entry not found".into()))?;
    if raw.trim().is_empty() {
        return Err(AppError::Internal("无可润色的原文".into()));
    }
    let enhance = app.state::<Arc<EnhanceManager>>();
    let enhanced = enhance.enhance(&raw, &reenhance_config(&app))?;
    history.set_enhanced(&app, id, &enhanced)
}
