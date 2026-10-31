//! Tests for `authctx.rs` against the real migrated schema: the author's own shown work, first publishes, key-free
//! contents, the independent-roots "common" rule, and the first publish of a content.
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
fn user(c: &Connection, id: &str) {
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?1, 'teacher', 1)", params![id, format!("{id}@x.org")]).unwrap();
}
fn item(c: &Connection, id: &str, author: &str, publish: &str) {
    c.execute("INSERT INTO items (id, author_id, kind, visibility, current_publish_id, license, created_at, updated_at) VALUES (?1, ?2, 'bank', 'public', ?3, 'CC0-1.0', 1, 1)", params![id, author, publish]).unwrap();
}
fn publish(c: &Connection, id: &str, item: &str, author: &str, hide_key: bool, status: &str) {
    c.execute(
        "INSERT INTO item_publishes (publish_id, item_id, author_id, version, status, schema_version, hide_key, license, attested_at, attest_text_rev,
            acks_json, request_sha256, stage_token, full_key, full_bytes, title, subject, course, grade, kinds_mask, question_count, has_images, created_at)
         VALUES (?1, ?2, ?3, 1, ?4, 2, ?5, 'CC0-1.0', 1, 'r', '{}', 'x', ?1, 'k', 10, '', '', '', '', 0, 1, 1, 1)",
        params![id, item, author, status, hide_key as i64],
    )
    .unwrap();
}
/// A question row with every required column filled neutrally, then the ones the queries read set.
fn question(c: &Connection, publish: &str, qid: &str, mh: &str, seq: i64, full: &str, near_full: &str, chain: &str) {
    let required: Vec<String> = {
        let mut st = c.prepare("SELECT name FROM pragma_table_info('item_questions') WHERE \"notnull\" = 1 AND dflt_value IS NULL").unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    let vals: Vec<String> = required.iter().map(|n| if n == "pos" { "0".to_string() } else { "''".to_string() }).collect();
    c.execute(&format!("INSERT INTO item_questions ({}) VALUES ({})", required.join(", "), vals.join(", ")), []).unwrap_or_else(|e| panic!("{e}"));
    c.execute(
        "UPDATE item_questions SET publish_id = ?1, qid = ?2, match_hash = ?3, seq = ?4, full_content = ?5, near_full = ?6, near_stripped = ?6, chain_json = ?7 WHERE rowid = last_insert_rowid()",
        params![publish, qid, mh, seq, full, near_full, chain],
    )
    .unwrap();
}
fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

#[test]
fn no_hashes_means_only_the_authors_item_ids_and_the_default_threshold() {
    let (c, _d) = open();
    user(&c, "a");
    item(&c, "i1", "a", "p1");
    let x = PoolCtx::new(&c, 1);
    let a = author_context(&x, "a", &[]).unwrap();
    assert_eq!(a.mine, ["i1".to_string()].into_iter().collect());
    assert_eq!(a.k, 3);
    assert!(a.first_at.is_empty() && a.common.is_empty());
}

#[test]
fn own_work_counts_only_what_was_shown_and_the_first_publish_is_the_earliest_record() {
    let (c, _d) = open();
    user(&c, "a");
    item(&c, "i1", "a", "p1");
    publish(&c, "p1", "i1", "a", false, "current");
    publish(&c, "p2", "i1", "a", true, "previous");
    question(&c, "p1", "q1", "mh1", 5, "F1", "N1", "[]");
    question(&c, "p2", "q2", "mh2", 3, "F2", "N2", "[]");
    // A staged (uncommitted) publish is not retained evidence.
    publish(&c, "p3", "i1", "a", false, "staged");
    question(&c, "p3", "q3", "mh3", 1, "F3", "N3", "[]");
    c.execute("INSERT INTO author_firsts (author_id, match_hash, first_seq) VALUES ('a', 'mh2', 1)", []).unwrap();
    c.execute("INSERT INTO author_contents (author_id, match_hash, kind, hash, first_seq) VALUES ('a', 'mh1', 'shown', 'F9', 2), ('a', 'mh1', 'kf', 'KF1', 2), ('a', 'mh4', 'shown', 'F4', 2)", []).unwrap();
    let x = PoolCtx::new(&c, 1);
    let a = author_context(&x, "a", &s(&["mh1", "mh2", "mh3", "mh4", "mh1"])).unwrap();
    assert!(a.own["mh1"].contains("F1") && a.own["mh1"].contains("F9"), "the retained shown content and the durable one");
    assert!(a.own["mh2"].shown.is_empty(), "a hidden-key publish contributes no content");
    assert!(!a.own.contains_key("mh3"), "a staged publish is not retained");
    assert!(a.own["mh4"].contains("F4"), "a purged original is still the author's own");
    assert_eq!((a.first_at["mh1"], a.first_at["mh2"]), (5, 1), "the durable record lowers the retained one");
    assert!(!a.first_at.contains_key("mh3"));
    assert_eq!(a.own_kf, ["KF1".to_string()].into_iter().collect());
}

#[test]
fn a_question_is_common_only_when_enough_independent_roots_published_it_visibly() {
    let (c, _d) = open();
    for u in ["me", "b", "c", "d"] {
        user(&c, u);
    }
    c.execute("INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('*', 'block_common_k', 2, 1, 'me')", []).unwrap();
    let (ib, ic, id) = ("1".repeat(32), "2".repeat(32), "3".repeat(32));
    for (it, u, p) in [(&ib, "b", "pb"), (&ic, "c", "pc"), (&id, "d", "pd")] {
        item(&c, it, u, p);
        publish(&c, p, it, u, false, "current");
    }
    question(&c, "pb", "q", "mh", 10, "FB", "NB", "[]");
    // c copied from b: its root is b, so it adds no independent root.
    question(&c, "pc", "q", "mh", 20, "FC", "NC", &format!(r#"[{{"item_id":"{ib}"}}]"#));
    let x = PoolCtx::new(&c, 1);
    let a = author_context(&x, "me", &s(&["mh"])).unwrap();
    assert_eq!(a.k, 2);
    assert!(!a.common.contains("mh"), "b and its copy are one root");
    assert_eq!(a.roots["mh"].iter().map(|r| (r.r.as_str(), r.s)).collect::<Vec<_>>(), vec![("b", 10), ("b", 20)], "the copy's root is b: a row per served content, one root");
    // An "earlier identical publication" link is not lineage: c is its own root.
    c.execute("UPDATE item_questions SET chain_json = ?1 WHERE publish_id = 'pc'", [format!(r#"[{{"item_id":"{ib}","earlier_match":true}}]"#)]).unwrap();
    let a = author_context(&x, "me", &s(&["mh"])).unwrap();
    assert!(a.common.contains("mh"));
    // The publisher's own lineage never counts, nor does an item that others cannot see.
    c.execute("UPDATE items SET visibility = 'unlisted' WHERE id = ?1", [&ic]).unwrap();
    assert!(!author_context(&x, "me", &s(&["mh"])).unwrap().common.contains("mh"));
    question(&c, "pd", "q", "mh", 30, "FD", "ND", "[]");
    assert!(author_context(&x, "me", &s(&["mh"])).unwrap().common.contains("mh"), "b and d are independent");
    assert!(!author_context(&x, "b", &s(&["mh"])).unwrap().roots["mh"].iter().any(|r| r.r == "b"), "the asker's own root is excluded");
}

#[test]
fn the_first_publish_of_a_content_ignores_a_bare_key_free_form_when_the_key_is_a_work() {
    let (c, _d) = open();
    user(&c, "a");
    c.execute("INSERT INTO author_contents (author_id, match_hash, kind, hash, first_seq) VALUES ('a', 'm1', 'nshown', 'N1', 4), ('a', 'm2', 'kf', 'K1', 6)", []).unwrap();
    let x = PoolCtx::new(&c, 1);
    let subj = [
        ContentSubject { n: Some("N1"), kf: "other", keywork: false },
        ContentSubject { n: Some("zz"), kf: "K1", keywork: false },
        ContentSubject { n: Some("zz"), kf: "K1", keywork: true },
        ContentSubject { n: Some("N1"), kf: "K1", keywork: false },
        ContentSubject { n: None, kf: "none", keywork: false },
    ];
    let m = own_content_first(&x, "a", &subj).unwrap();
    assert_eq!(m.get(&0), Some(&4));
    assert_eq!(m.get(&1), Some(&6));
    assert_eq!(m.get(&2), None, "the key is a work: its key-free form stands for nothing");
    assert_eq!(m.get(&3), Some(&4), "the earlier of the two");
    assert_eq!(m.get(&4), None);
    assert!(own_content_first(&x, "a", &[]).unwrap().is_empty());
    assert!(own_content_first(&x, "someone", &subj).unwrap().is_empty());
}
