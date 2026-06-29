// Dictation history (Home/History screens) — PROJECT_SPEC.md §6.1 manager.
//
// SQLite-backed, text-only (audio never lands here per §6.6 privacy rule). The
// schema is fresh, so init is a single CREATE TABLE IF NOT EXISTS — no migration
// crate. Stats are aggregated on the fly from the same table (no separate counter
// to drift out of sync). Pattern adapted from cjpais/Handy's history.rs, stripped
// of its WAV files / saved-flag / specta event typing.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::error::{AppError, AppResult};

const HISTORY_UPDATED_EVENT: &str = "history-updated";
const LIST_LIMIT: i64 = 500;

/// One dictation outcome. Field names are snake_case so the frontend Zod schema
/// (src/types/history.ts) mirrors them verbatim, like Settings does.
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
        let retention = crate::commands::load_settings(app).history_retention;
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

fn create_table(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS history (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at    INTEGER NOT NULL,
            kind          TEXT    NOT NULL,
            mode          TEXT    NOT NULL DEFAULT 'polish',
            raw_text      TEXT    NOT NULL DEFAULT '',
            enhanced_text TEXT,
            word_count    INTEGER NOT NULL DEFAULT 0,
            duration_ms   INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_history_created_at ON history(created_at);",
    )
}

/// Older DBs predate the `mode` column. Add it idempotently — a fresh `create_table`
/// already includes it, so this ALTER then fails "duplicate column name", which we
/// swallow (no migration framework, mirroring this module's simple-init philosophy).
fn add_mode_column_if_missing(conn: &Connection) {
    let _ = conn.execute(
        "ALTER TABLE history ADD COLUMN mode TEXT NOT NULL DEFAULT 'polish'",
        [],
    );
}

#[allow(clippy::too_many_arguments)] // flat column list; a struct here adds noise
fn insert_entry(
    conn: &Connection,
    created_at: i64,
    kind: &str,
    mode: &str,
    raw_text: &str,
    enhanced_text: Option<&str>,
    word_count: i64,
    duration_ms: i64,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO history (created_at, kind, mode, raw_text, enhanced_text, word_count, duration_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            created_at,
            kind,
            mode,
            raw_text,
            enhanced_text,
            word_count,
            duration_ms
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

fn fetch_list(conn: &Connection, limit: i64) -> rusqlite::Result<Vec<HistoryEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, created_at, kind, mode, raw_text, enhanced_text, word_count, duration_ms
         FROM history
         ORDER BY created_at DESC, id DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], |row| {
        Ok(HistoryEntry {
            id: row.get(0)?,
            created_at: row.get(1)?,
            kind: row.get(2)?,
            mode: row.get(3)?,
            raw_text: row.get(4)?,
            enhanced_text: row.get(5)?,
            word_count: row.get(6)?,
            duration_ms: row.get(7)?,
        })
    })?;
    rows.collect()
}

fn fetch_stats(conn: &Connection) -> rusqlite::Result<UsageStats> {
    // Split spoken (口述听写, mode='polish') from AI output (写作/改写) so the Home
    // 口述 cards aren't inflated by generated text. 口述字数/速度 count the ASR transcript
    // itself —— LENGTH(raw_text) (SQLite returns char count for TEXT), NOT the polished
    // word_count —— so 速度 reflects what was actually spoken. AI 产出 keeps word_count
    // (the generated / rewritten text). Over successful rows only.
    conn.query_row(
        "SELECT
           COALESCE(SUM(CASE WHEN mode = 'polish' THEN LENGTH(raw_text) ELSE 0 END), 0),
           COALESCE(SUM(CASE WHEN mode = 'polish' THEN duration_ms ELSE 0 END), 0),
           COALESCE(SUM(CASE WHEN mode = 'polish' THEN 1 ELSE 0 END), 0),
           COALESCE(SUM(CASE WHEN mode IN ('compose', 'rewrite') THEN word_count ELSE 0 END), 0)
         FROM history
         WHERE kind = 'success'",
        [],
        |row| {
            Ok(UsageStats {
                spoken_words: row.get(0)?,
                spoken_duration_ms: row.get(1)?,
                spoken_count: row.get(2)?,
                ai_output_words: row.get(3)?,
            })
        },
    )
}

fn delete_older_than(conn: &Connection, cutoff: i64) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM history WHERE created_at < ?1", [cutoff])
}

fn fetch_raw_text(conn: &Connection, id: i64) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT raw_text FROM history WHERE id = ?1", [id], |row| {
        row.get(0)
    })
    .optional()
}

fn update_enhanced(
    conn: &Connection,
    id: i64,
    enhanced: &str,
    word_count: i64,
) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE history SET enhanced_text = ?1, word_count = ?2 WHERE id = ?3",
        rusqlite::params![enhanced, word_count, id],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        create_table(&conn).expect("create table");
        conn
    }

    fn insert(
        conn: &Connection,
        created_at: i64,
        kind: &str,
        mode: &str,
        raw: &str,
        words: i64,
        dur: i64,
    ) {
        insert_entry(conn, created_at, kind, mode, raw, None, words, dur).expect("insert");
    }

    #[test]
    fn list_returns_newest_first() {
        let conn = setup();
        insert(&conn, 100, "success", "polish", "first", 5, 1000);
        insert(&conn, 200, "success", "polish", "second", 6, 2000);

        let entries = fetch_list(&conn, 500).expect("list");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].raw_text, "second");
        assert_eq!(entries[1].raw_text, "first");
    }

    #[test]
    fn stats_split_spoken_vs_ai_output() {
        let conn = setup();
        insert(&conn, 100, "success", "polish", "口述听写", 999, 3000); // word_count 故意≠raw 字数
        insert(
            &conn,
            200,
            "success",
            "compose",
            "AI 生成的一长段",
            200,
            1000,
        );
        insert(&conn, 250, "success", "rewrite", "改写结果", 50, 800);
        insert(&conn, 300, "error", "polish", "cancelled", 4, 0); // 非 success，排除

        let stats = fetch_stats(&conn).expect("stats");
        // 口述字数 = ASR 转写原文(raw_text)字符数 = LENGTH("口述听写") = 4，不是 word_count(999)
        assert_eq!(stats.spoken_words, 4);
        assert_eq!(stats.spoken_duration_ms, 3000);
        assert_eq!(stats.spoken_count, 1);
        // AI 产出 = compose + rewrite 的字数
        assert_eq!(stats.ai_output_words, 250);
    }

    #[test]
    fn stats_are_zero_when_empty() {
        let conn = setup();
        let stats = fetch_stats(&conn).expect("stats");
        assert_eq!(stats.spoken_words, 0);
        assert_eq!(stats.spoken_duration_ms, 0);
        assert_eq!(stats.spoken_count, 0);
        assert_eq!(stats.ai_output_words, 0);
    }

    #[test]
    fn mode_round_trips() {
        let conn = setup();
        insert(&conn, 100, "success", "compose", "raw", 5, 1000);
        let entries = fetch_list(&conn, 500).expect("list");
        assert_eq!(entries[0].mode, "compose");
    }

    #[test]
    fn migration_adds_mode_to_legacy_table() {
        // Simulate a pre-mode DB: old schema with no `mode` column.
        let conn = Connection::open_in_memory().expect("open");
        conn.execute_batch(
            "CREATE TABLE history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at INTEGER NOT NULL,
                kind TEXT NOT NULL,
                raw_text TEXT NOT NULL DEFAULT '',
                enhanced_text TEXT,
                word_count INTEGER NOT NULL DEFAULT 0,
                duration_ms INTEGER NOT NULL DEFAULT 0
            );",
        )
        .expect("legacy table");
        add_mode_column_if_missing(&conn);
        insert(&conn, 100, "success", "compose", "raw", 5, 1000);
        assert_eq!(fetch_list(&conn, 500).expect("list")[0].mode, "compose");
    }

    #[test]
    fn migration_is_idempotent_on_fresh_table() {
        let conn = setup(); // create_table already added the column
        add_mode_column_if_missing(&conn); // duplicate-column error swallowed, no panic
        insert(&conn, 100, "success", "polish", "raw", 5, 1000);
        assert_eq!(fetch_list(&conn, 500).expect("list").len(), 1);
    }

    #[test]
    fn cleanup_deletes_only_older_than_cutoff() {
        let conn = setup();
        insert(&conn, 100, "success", "polish", "old", 1, 0);
        insert(&conn, 300, "success", "polish", "new", 1, 0);

        let deleted = delete_older_than(&conn, 200).expect("cleanup");
        assert_eq!(deleted, 1);
        let entries = fetch_list(&conn, 500).expect("list");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].raw_text, "new");
    }

    #[test]
    fn enhanced_text_round_trips() {
        let conn = setup();
        insert_entry(
            &conn,
            100,
            "success",
            "polish",
            "raw",
            Some("polished"),
            8,
            1500,
        )
        .expect("insert");
        let entries = fetch_list(&conn, 500).expect("list");
        assert_eq!(entries[0].enhanced_text.as_deref(), Some("polished"));
    }

    #[test]
    fn reenhance_updates_enhanced_text_and_word_count() {
        let conn = setup();
        let id = insert_entry(&conn, 100, "success", "polish", "raw text", None, 8, 1000)
            .expect("insert");
        assert_eq!(
            fetch_raw_text(&conn, id).expect("raw"),
            Some("raw text".to_string())
        );

        update_enhanced(&conn, id, "polished output", 15).expect("update");
        let entries = fetch_list(&conn, 500).expect("list");
        assert_eq!(entries[0].enhanced_text.as_deref(), Some("polished output"));
        assert_eq!(entries[0].word_count, 15);
    }

    #[test]
    fn retention_window_maps_known_ids_only() {
        assert_eq!(retention_window_secs("day"), Some(86_400));
        assert_eq!(retention_window_secs("week"), Some(604_800));
        assert_eq!(retention_window_secs("month"), Some(2_592_000));
        assert_eq!(retention_window_secs("forever"), None);
        assert_eq!(retention_window_secs("never"), None);
    }
}
