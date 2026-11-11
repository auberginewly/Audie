use rusqlite::{Connection, OptionalExtension};

use super::HistoryEntry;

#[allow(clippy::too_many_arguments)] // flat column list; a struct here adds noise
pub(super) fn insert_entry(
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

pub(super) fn fetch_list(conn: &Connection, limit: i64) -> rusqlite::Result<Vec<HistoryEntry>> {
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

pub(super) fn delete_older_than(conn: &Connection, cutoff: i64) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM history WHERE created_at < ?1", [cutoff])
}

pub(super) fn fetch_raw_text(conn: &Connection, id: i64) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT raw_text FROM history WHERE id = ?1", [id], |row| {
        row.get(0)
    })
    .optional()
}

pub(super) fn update_enhanced(
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
