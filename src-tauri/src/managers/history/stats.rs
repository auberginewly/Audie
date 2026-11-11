use rusqlite::Connection;

use super::{DailyUsage, UsageStats};

pub(super) fn fetch_stats(conn: &Connection) -> rusqlite::Result<UsageStats> {
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
           COALESCE(SUM(CASE WHEN mode IN ('compose', 'rewrite') AND enhanced_text IS NOT NULL THEN word_count ELSE 0 END), 0)
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

pub(super) fn fetch_daily(conn: &Connection, days: u32) -> rusqlite::Result<Vec<DailyUsage>> {
    // Same 口径 as fetch_stats (口述 = LENGTH(raw_text) of polish rows; AI 产出 =
    // word_count of compose/rewrite rows that have enhanced text), bucketed by
    // LOCAL calendar day — the generated `-(days - 1) days` modifier keeps the
    // requested window on the user's wall clock without a date library.
    // days = 0 means 全部 (all time): the empty `since` short-circuits the predicate.
    let since = if days == 0 {
        String::new()
    } else {
        format!("-{} days", days - 1)
    };
    let mut stmt = conn.prepare(
        "SELECT
           date(created_at, 'unixepoch', 'localtime') AS d,
           COALESCE(SUM(CASE WHEN mode = 'polish' THEN LENGTH(raw_text) ELSE 0 END), 0),
           COALESCE(SUM(CASE WHEN mode IN ('compose', 'rewrite') AND enhanced_text IS NOT NULL THEN word_count ELSE 0 END), 0)
         FROM history
         WHERE kind = 'success'
           AND (?1 = '' OR date(created_at, 'unixepoch', 'localtime') >= date('now', 'localtime', ?1))
         GROUP BY d
         ORDER BY d",
    )?;
    let rows = stmt.query_map([since], |row| {
        Ok(DailyUsage {
            day: row.get(0)?,
            spoken_words: row.get(1)?,
            ai_output_words: row.get(2)?,
        })
    })?;
    rows.collect()
}
