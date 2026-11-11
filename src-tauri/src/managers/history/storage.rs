use rusqlite::Connection;

pub(super) fn create_table(conn: &Connection) -> rusqlite::Result<()> {
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
pub(super) fn add_mode_column_if_missing(conn: &Connection) {
    let _ = conn.execute(
        "ALTER TABLE history ADD COLUMN mode TEXT NOT NULL DEFAULT 'polish'",
        [],
    );
}
