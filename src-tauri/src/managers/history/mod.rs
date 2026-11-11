// Dictation history (Home/History screens) — PROJECT_SPEC.md §6.1 manager.
//
// SQLite-backed, text-only (audio never lands here per §6.6 privacy rule). The
// schema is fresh, so init is a single CREATE TABLE IF NOT EXISTS — no migration
// crate. Stats are aggregated on the fly from the same table (no separate counter
// to drift out of sync). Pattern adapted from cjpais/Handy's history.rs, stripped
// of its WAV files / saved-flag / specta event typing.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::error::{AppError, AppResult};

const HISTORY_UPDATED_EVENT: &str = "history-updated";
const LIST_LIMIT: i64 = 500;

/// One dictation outcome. Field names are snake_case so the frontend Zod schema
/// (src/features/history/schema.ts) mirrors them verbatim, like Settings does.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct HistoryEntry {
    pub id: i64,
    pub created_at: i64, // UTC unix seconds
    pub kind: String,    // "success" (has text) | "empty" (没有识别到内容)
    pub mode: String,    // "polish" | "rewrite" | "compose"
    pub raw_text: String,
    pub enhanced_text: Option<String>,
    pub word_count: i64,
    pub duration_ms: i64,
}

/// All-time usage aggregate for the Home dashboard, split by dictation kind so
/// 写作/改写 (AI output) don't inflate the 口述 (spoken) cards. Frontend: 口述
/// 时间/字数/速度 from `spoken_*`, plus an 「AI 产出」 card from `ai_output_words`.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct UsageStats {
    /// 纯口述听写（mode = "polish"）：字数（= ASR 转写原文 raw_text 字符数，非润色后）/ 时长 / 次数。
    pub spoken_words: i64,
    pub spoken_duration_ms: i64,
    pub spoken_count: i64,
    /// 写作 + 改写（mode in compose/rewrite）产出的字数。
    pub ai_output_words: i64,
}

/// One local-calendar-day bucket for the Home 近 N 天 chart. Same 口述/AI 产出
/// split as UsageStats so the curve and the all-time cards agree (see fetch_daily).
/// Only days that have rows are returned — the frontend zero-fills the gaps.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct DailyUsage {
    /// Local date, `YYYY-MM-DD` (SQLite `date(..., 'localtime')`).
    pub day: String,
    pub spoken_words: i64,
    pub ai_output_words: i64,
}

/// Upper bound for `daily_usage` range requests — a defensive clamp, not a product limit.
const MAX_DAILY_DAYS: u32 = 90;

pub struct HistoryManager {
    db_path: PathBuf,
}

impl HistoryManager {
    /// Resolve `<app_data_dir>/history.db` and create the table. A failure only logs
    /// (history degrades to unavailable) — it must not abort startup, mirroring the
    /// hotkey-registration tolerance in lib.rs.
    pub fn new(app: &AppHandle) -> Self {
        let db_path = match app.path().app_data_dir() {
            Ok(dir) => dir.join("history.db"),
            Err(err) => {
                log::error!("resolve app data dir for history db: {err}");
                PathBuf::new()
            }
        };

        let manager = Self { db_path };
        if let Err(err) = manager.init() {
            log::error!("init history db: {err}");
        }
        manager
    }

    fn init(&self) -> AppResult<()> {
        let conn = self.open()?;
        create_table(&conn).map_err(map_sqlite)?;
        add_mode_column_if_missing(&conn);
        Ok(())
    }

    fn open(&self) -> AppResult<Connection> {
        if self.db_path.as_os_str().is_empty() {
            return Err(AppError::Internal("history db path unavailable".into()));
        }
        if let Some(parent) = self.db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| AppError::Internal(format!("create history db dir: {err}")))?;
        }
        Connection::open(&self.db_path).map_err(map_sqlite)
    }

    /// Persist one outcome. `never` retention skips recording entirely; otherwise we
    /// insert, prune to the retention window, and notify the frontend. `enhanced_text`
    /// is the polished version when polishing ran, else None (caller decides).
    pub fn record(
        &self,
        app: &AppHandle,
        kind: &str,
        mode: &str,
        raw_text: &str,
        enhanced_text: Option<String>,
        duration_ms: i64,
    ) -> AppResult<()> {
        let retention = crate::settings::load_settings(app).history_retention;
        if retention == "never" {
            return Ok(());
        }

        let conn = self.open()?;
        let displayed = enhanced_text.as_deref().unwrap_or(raw_text);
        let word_count = displayed.chars().count() as i64;
        insert_entry(
            &conn,
            now_unix(),
            kind,
            mode,
            raw_text,
            enhanced_text.as_deref(),
            word_count,
            duration_ms,
        )
        .map_err(map_sqlite)?;

        if let Some(window) = retention_window_secs(&retention) {
            delete_older_than(&conn, now_unix() - window).map_err(map_sqlite)?;
        }

        emit_updated(app);
        Ok(())
    }

    pub fn list(&self) -> AppResult<Vec<HistoryEntry>> {
        let conn = self.open()?;
        fetch_list(&conn, LIST_LIMIT).map_err(map_sqlite)
    }

    pub fn delete_entry(&self, app: &AppHandle, id: i64) -> AppResult<()> {
        let conn = self.open()?;
        conn.execute("DELETE FROM history WHERE id = ?1", [id])
            .map_err(map_sqlite)?;
        emit_updated(app);
        Ok(())
    }

    pub fn clear(&self, app: &AppHandle) -> AppResult<()> {
        let conn = self.open()?;
        conn.execute("DELETE FROM history", [])
            .map_err(map_sqlite)?;
        emit_updated(app);
        Ok(())
    }

    /// All-time totals over successful dictations only (errors don't count).
    pub fn usage_stats(&self) -> AppResult<UsageStats> {
        let conn = self.open()?;
        fetch_stats(&conn).map_err(map_sqlite)
    }

    /// Per-day 口述/AI 产出 for the Home chart, over the last `days` local days
    /// (`days` = 0 means 全部/all time; otherwise clamped to MAX_DAILY_DAYS).
    /// Days without rows are absent, not zero.
    pub fn daily_usage(&self, days: u32) -> AppResult<Vec<DailyUsage>> {
        let conn = self.open()?;
        fetch_daily(&conn, days.min(MAX_DAILY_DAYS)).map_err(map_sqlite)
    }

    /// The stored transcript for an entry — drives the History 重试 (re-enhance).
    pub fn raw_text_of(&self, id: i64) -> AppResult<Option<String>> {
        let conn = self.open()?;
        fetch_raw_text(&conn, id).map_err(map_sqlite)
    }

    /// Overwrite an entry's enhanced text (History 重试 result) + word_count, then
    /// notify so the list shows the new 润色 box.
    pub fn set_enhanced(&self, app: &AppHandle, id: i64, enhanced: &str) -> AppResult<()> {
        let conn = self.open()?;
        update_enhanced(&conn, id, enhanced, enhanced.chars().count() as i64)
            .map_err(map_sqlite)?;
        emit_updated(app);
        Ok(())
    }
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn emit_updated(app: &AppHandle) {
    if let Err(err) = app.emit(HISTORY_UPDATED_EVENT, ()) {
        log::warn!("emit history-updated: {err}");
    }
}

fn map_sqlite(err: rusqlite::Error) -> AppError {
    AppError::Internal(format!("history db: {err}"))
}

/// Cleanup window for a retention id. `forever` / `never` return None (no time-based
/// delete — `never` is handled earlier by skipping the insert).
fn retention_window_secs(retention: &str) -> Option<i64> {
    match retention {
        "day" => Some(24 * 60 * 60),
        "week" => Some(7 * 24 * 60 * 60),
        "month" => Some(30 * 24 * 60 * 60),
        _ => None,
    }
}

// --- pure SQL helpers (take &Connection so tests can use an in-memory db) ---

mod queries;
mod stats;
mod storage;

use queries::{delete_older_than, fetch_list, fetch_raw_text, insert_entry, update_enhanced};
use stats::{fetch_daily, fetch_stats};
use storage::{add_mode_column_if_missing, create_table};

#[cfg(test)]
mod tests;
