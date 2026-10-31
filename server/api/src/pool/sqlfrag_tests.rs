//! Tests for `sqlfrag.rs`. Every SQL fragment is prepared against the real migrated schema, and the rules that matter
//! (visibility, quota, block keys) are run over small data.
use super::*;
use crate::db;
use rusqlite::{Connection, params};
use std::path::Path;

fn open() -> (Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    (conn, dir)
}

#[test]
fn ids_have_the_patterns_the_routes_check() {
    let id = new_id();
    assert!(is_id(&id) && id.len() == 32);
    assert_ne!(id, new_id());
    assert!(!is_id(&id.to_uppercase()) && !is_id("abc") && !is_id(&format!("{id}0")));
    assert!(is_publish_id(&"A".repeat(22)) && is_publish_id(&"a-_".repeat(21)) && !is_publish_id(&"a".repeat(21)) && !is_publish_id(&"a".repeat(65)) && !is_publish_id(&format!("{}!", "a".repeat(22))));
    assert!(is_sha(&"0".repeat(64)) && !is_sha(&"g".repeat(64)) && !is_sha(&"0".repeat(63)));
    assert_eq!(day_of(DAY_MS * 3 + 5), 3);
    assert_eq!(hex(&[0, 15, 255]), "000fff");
}

#[test]
fn the_limits_table_has_every_metric_once_with_the_documented_defaults() {
    let mut names: Vec<&str> = DEFAULT_LIMITS.iter().map(|(n, _)| *n).collect();
    let total = names.len();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), total, "no metric twice");
    assert_eq!(total, 52);
    assert_eq!((default_limit("publishes"), default_limit("storage_bytes"), default_limit("circle_bytes"), default_limit("nope")), (Some(50), Some(200 * MB), Some(200 * MB), None));
    assert_eq!(default_limit("circle_email_domain_hourly"), Some(30));
}

#[test]
fn flag_severity_defaults_to_three_and_the_sql_agrees_with_the_table() {
    assert_eq!((severity_of("privacy_text_match"), severity_of("origin_unverified"), severity_of("anything else")), (0, 4, 3));
    let (c, _d) = open();
    c.execute_batch("CREATE TEMP TABLE f (kind TEXT)").unwrap();
    for (k, _) in FLAG_SEVERITY {
        c.execute("INSERT INTO f VALUES (?1)", [k]).unwrap();
    }
    c.execute("INSERT INTO f VALUES ('mystery')", []).unwrap();
    let mut st = c.prepare(&format!("SELECT kind, {} FROM f", flag_severity_sql())).unwrap();
    for row in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))).unwrap() {
        let (k, s) = row.unwrap();
        assert_eq!(s, severity_of(&k), "{k}");
    }
    assert!(INTEGRITY_FLAGS.iter().all(|k| severity_of(k) <= 4) && COLLAPSED_FLAGS == ["origin_unverified"]);
}

fn user(c: &Connection, id: &str, trust: &str, suspended: bool, deleted: bool) {
    // An erased account keeps no email (a CHECK in the schema).
    let email = (!deleted).then(|| format!("{id}@x.org"));
    c.execute("INSERT INTO users (id, email, trust, created_at, suspended_at, deleted_at) VALUES (?1, ?2, ?3, 1, ?4, ?5)", params![id, email, trust, suspended.then_some(5), deleted.then_some(5)]).unwrap();
}

fn item(c: &Connection, id: &str, author: &str, questions: i64) {
    c.execute("INSERT INTO items (id, author_id, kind, visibility, current_publish_id, license, created_at, updated_at, question_count) VALUES (?1, ?2, 'bank', 'public', 'p', 'CC0-1.0', 1, 1, ?3)", params![id, author, questions]).unwrap();
}

#[test]
fn an_item_is_visible_to_others_only_when_public_committed_unremoved_unheld_and_the_author_stands() {
    let (c, _d) = open();
    user(&c, "good", "teacher", false, false);
    user(&c, "susp", "teacher", true, false);
    user(&c, "pend", "pending", false, false);
    user(&c, "gone", "teacher", false, true);
    for (id, a) in [("i_good", "good"), ("i_susp", "susp"), ("i_pend", "pend"), ("i_gone", "gone"), ("i_removed", "good"), ("i_held", "good"), ("i_anon", "gone")] {
        item(&c, id, a, 1);
    }
    c.execute("UPDATE items SET removed_at = 9, removed_reason = 'admin' WHERE id = 'i_removed'", []).unwrap();
    c.execute("UPDATE items SET anonymized = 1 WHERE id = 'i_anon'", []).unwrap();
    c.execute("INSERT INTO item_holds (item_id, reason, source, at) VALUES ('i_held', 'student_data', 'x', 1)", []).unwrap();
    let sql = format!("SELECT i.id FROM items i JOIN users u ON u.id = i.author_id WHERE {} ORDER BY i.id", visible_to_others());
    let mut st = c.prepare(&sql).unwrap();
    let ids: Vec<String> = st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect();
    // A deleted author's item stays visible only when it was anonymised and the author stood (the content outlives the
    // account); here the deleted author is a teacher, so i_anon shows and i_gone does not.
    assert!(ids.contains(&"i_good".to_string()) && ids.contains(&"i_anon".to_string()), "{ids:?}");
    for hidden in ["i_susp", "i_pend", "i_gone", "i_removed", "i_held"] {
        assert!(!ids.contains(&hidden.to_string()), "{hidden}");
    }
    // visible_to: the author sees their own unremoved item whatever their standing.
    let mine = format!("SELECT i.id FROM items i JOIN users u ON u.id = i.author_id WHERE {} ORDER BY i.id", visible_to("?1"));
    let mut st = c.prepare(&mine).unwrap();
    let ids: Vec<String> = st.query_map(["susp"], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect();
    assert!(ids.contains(&"i_susp".to_string()) && ids.contains(&"i_good".to_string()));
    assert!(!ids.contains(&"i_pend".to_string()));
}

#[test]
fn the_quota_and_sharing_fragments_prepare_and_charge_what_the_comment_says() {
    let (c, _d) = open();
    user(&c, "a", "teacher", false, false);
    item(&c, "i1", "a", 4);
    item(&c, "i2", "a", 6);
    c.execute("UPDATE items SET removed_at = 5, removed_reason = 'admin' WHERE id = 'i2'", []).unwrap();
    let used: i64 = c.query_row(&format!("SELECT {}", usage_sql(None)), params!["a", 10], |r| r.get(0)).unwrap();
    assert_eq!(used, 1024, "1 KB per item not removed");
    let with_removed: i64 = c.query_row(&format!("SELECT {}", usage_sql(Some("?3"))), params!["a", 10, "i2"], |r| r.get(0)).unwrap();
    assert_eq!(with_removed, 2048, "the undelete check counts the removed item as live");
    c.execute("INSERT INTO pending_blobs (sha256, uploader_id, size, until) VALUES (?1, 'a', 500, 100), (?2, 'a', 700, 5)", params!["a".repeat(64), "b".repeat(64)]).unwrap();
    let used: i64 = c.query_row(&format!("SELECT {}", usage_sql(None)), params!["a", 10], |r| r.get(0)).unwrap();
    assert_eq!(used, 1024 + 500, "a pending upload counts until it lapses");
    let removed: i64 = c.query_row(&format!("SELECT {REMOVED_SQL}"), params!["a"], |r| r.get(0)).unwrap();
    assert_eq!(removed, 0);
    let shared: i64 = c.query_row(&format!("SELECT {}", shared_sql("?1", "?2")), params!["a", "none"], |r| r.get(0)).unwrap();
    assert_eq!(shared, 4, "the removed item's questions are not shared");
    let shared_but_i1: i64 = c.query_row(&format!("SELECT {}", shared_sql("?1", "?2")), params!["a", "i1"], |r| r.get(0)).unwrap();
    assert_eq!(shared_but_i1, 0);
    assert!(alive_sql("?1").contains("deleted_at IS NOT NULL"));
    let alive: i64 = c.query_row(&format!("SELECT {}", alive_sql("?1")), ["a"], |r| r.get(0)).unwrap();
    assert_eq!(alive, 1);
}

#[test]
fn block_key_comparisons_prepare_and_match_question_and_part_keys() {
    let (c, _d) = open();
    let cols = c.prepare("SELECT * FROM item_questions LIMIT 0").unwrap();
    let names: Vec<String> = cols.column_names().iter().map(|s| s.to_string()).collect();
    for k in QKEY_COLS {
        assert!(names.iter().any(|n| n == k), "item_questions has {k}");
    }
    drop(cols);
    // The generated SQL must at least compile against the schema.
    for sql in [
        format!("SELECT {}", in_q_keys_of("'x'", "'p'")),
        format!("SELECT 1 FROM item_questions iq WHERE {}", q_key_eq("iq", "'x'")),
        format!("SELECT 1 FROM item_questions iq WHERE {}", q_key_in("iq", "(SELECT hash FROM privacy_blocks)")),
        format!("SELECT {}", anon_chain("'[]'")),
    ] {
        c.prepare(&sql).unwrap_or_else(|e| panic!("{e}: {sql}"));
    }
    assert_eq!(q_keys_of("?1"), "(SELECT qk.value FROM item_questions qkq, json_each(json_array(qkq.match_hash, qkq.bk_norm, qkq.bk_stem, qkq.bk_full, qkq.bk_expl, qkq.bk_ans)) qk WHERE qkq.publish_id = ?1 AND qk.value IS NOT NULL)");
    let chain: String = c.query_row(&format!("SELECT {}", anon_chain("?1")), [r#"[{"item_id":"zz","author_name":"Ann"}]"#], |r| r.get(0)).unwrap();
    assert_eq!(chain, r#"[{"item_id":"zz","author_name":"Ann"}]"#, "a link to an unknown item is kept as it was");
}
