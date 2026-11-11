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

#[allow(clippy::too_many_arguments)] // Keep fixture arguments aligned with stored columns.
fn insert_enhanced(
    conn: &Connection,
    created_at: i64,
    kind: &str,
    mode: &str,
    raw: &str,
    enhanced: &str,
    words: i64,
    dur: i64,
) {
    insert_entry(
        conn,
        created_at,
        kind,
        mode,
        raw,
        Some(enhanced),
        words,
        dur,
    )
    .expect("insert");
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
    insert_enhanced(
        &conn,
        200,
        "success",
        "compose",
        "写作指令",
        "AI 生成的一长段",
        200,
        1000,
    );
    insert_enhanced(
        &conn,
        250,
        "success",
        "rewrite",
        "改写指令和引用",
        "改写结果",
        50,
        800,
    );
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
fn stats_ai_output_excludes_rewrite_without_enhanced_text() {
    let conn = setup();
    insert(
        &conn,
        100,
        "success",
        "rewrite",
        "指令：改短\n\n引用：一大段被选中的原文",
        18,
        1000,
    );

    let stats = fetch_stats(&conn).expect("stats");
    assert_eq!(stats.ai_output_words, 0);
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
fn daily_buckets_by_local_day_and_split() {
    let conn = setup();
    // Day strings come from SQLite itself so the test is timezone-independent.
    let today: String = conn
        .query_row("SELECT date('now', 'localtime')", [], |r| r.get(0))
        .expect("today");
    let yesterday: String = conn
        .query_row("SELECT date('now', 'localtime', '-1 day')", [], |r| {
            r.get(0)
        })
        .expect("yesterday");
    let now = now_unix();

    // 今天：口述 4 字（LENGTH(raw)，不是 word_count 999）+ compose 产出 200；
    // rewrite 无 enhanced 不计 AI 产出；error 行排除。
    insert(&conn, now, "success", "polish", "口述听写", 999, 3000);
    insert_enhanced(
        &conn,
        now,
        "success",
        "compose",
        "写作指令",
        "产出",
        200,
        1000,
    );
    insert(&conn, now, "success", "rewrite", "指令+引用", 50, 800);
    insert(&conn, now, "error", "polish", "cancelled", 4, 0);
    // 昨天：只有口述 2 字。
    insert(&conn, now - 86_400, "success", "polish", "昨天", 2, 1000);
    // 10 天前：超出 7 天窗口。
    insert(
        &conn,
        now - 10 * 86_400,
        "success",
        "polish",
        "很久以前",
        4,
        1000,
    );

    let rows = fetch_daily(&conn, 7).expect("daily");
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[0],
        DailyUsage {
            day: yesterday,
            spoken_words: 2,
            ai_output_words: 0,
        }
    );
    assert_eq!(
        rows[1],
        DailyUsage {
            day: today,
            spoken_words: 4,
            ai_output_words: 200,
        }
    );
}

#[test]
fn daily_empty_returns_no_rows() {
    let conn = setup();
    assert!(fetch_daily(&conn, 7).expect("daily").is_empty());
}

#[test]
fn daily_zero_days_returns_all_time() {
    let conn = setup();
    let now = now_unix();
    insert(&conn, now, "success", "polish", "今天", 2, 1000);
    insert(
        &conn,
        now - 10 * 86_400,
        "success",
        "polish",
        "很久以前",
        4,
        1000,
    );

    // 7 天窗口看不到 10 天前；days = 0（全部）两天都在。
    assert_eq!(fetch_daily(&conn, 7).expect("windowed").len(), 1);
    let all = fetch_daily(&conn, 0).expect("all");
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].spoken_words, 4); // 按天升序，老的在先
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
    let id =
        insert_entry(&conn, 100, "success", "polish", "raw text", None, 8, 1000).expect("insert");
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
