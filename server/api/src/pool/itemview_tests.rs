//! Tests for `itemview.rs` on the real migrated schema: visibility, the served-form rule, what each view shows to the
//! author and to others, and the per-question provenance.
use super::*;
use crate::db;
use std::path::Path;

fn open() -> (Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    (conn, dir)
}
fn user(c: &Connection, id: &str) {
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?3, 'teacher', 1)", params![id, format!("{id}@x.org"), format!("Teacher {id}")]).unwrap();
}
fn item(c: &Connection, id: &str, author: &str, visibility: &str) {
    c.execute(
        "INSERT INTO items (id, author_id, kind, visibility, current_publish_id, public_publish_id, public_version, public_at, version, title, subject, course, grade, kinds_mask, question_count,
            license, has_images, committed_at, created_at, updated_at) VALUES (?1, ?2, 'bank', ?3, 'pc', 'pp', 2, 50, 3, 'Fractions', 'math', 'alg1', '8', 5, 2, 'CC-BY-4.0', 1, 100, 1, 1)",
        params![id, author, visibility],
    )
    .unwrap();
}
fn publish(c: &Connection, id: &str, item: &str, author: &str, hide: bool, version: i64) {
    c.execute(
        "INSERT INTO item_publishes (publish_id, item_id, author_id, version, public_version, status, schema_version, hide_key, license, attested_at, attest_text_rev,
            acks_json, request_sha256, stage_token, full_key, stripped_key, full_bytes, title, subject, course, grade, kinds_mask, question_count, has_images, created_at)
         VALUES (?1, ?2, ?3, ?4, 2, 'current', 2, ?5, 'CC-BY-4.0', 1, 'r', '{}', 'x', ?1, 'FK', 'SK', 10, '', '', '', '', 0, 1, 1, 1)",
        params![id, item, author, version, hide as i64],
    )
    .unwrap();
}
fn question(c: &Connection, publish: &str, qid: &str, pos: i64, chain: &str, allowed: Option<&str>) {
    let required: Vec<String> = {
        let mut st = c.prepare("SELECT name FROM pragma_table_info('item_questions') WHERE \"notnull\" = 1 AND dflt_value IS NULL").unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    let vals: Vec<String> = required.iter().map(|n| if n == "pos" { "0".to_string() } else { "''".to_string() }).collect();
    c.execute(&format!("INSERT INTO item_questions ({}) VALUES ({})", required.join(", "), vals.join(", ")), []).unwrap();
    c.execute(
        "UPDATE item_questions SET publish_id = ?1, qid = ?2, pos = ?3, match_hash = ?4, full_sha256 = 'FULLSHA', stripped_sha256 = 'STRIPSHA', chain_json = ?5, allowed_json = ?6 WHERE rowid = last_insert_rowid()",
        params![publish, qid, pos, format!("mh-{qid}"), chain, allowed],
    )
    .unwrap();
}
fn world(hide: bool) -> (Connection, tempfile::TempDir, String) {
    let (c, d) = open();
    user(&c, "a");
    user(&c, "r");
    let id = "1".repeat(32);
    item(&c, &id, "a", "public");
    publish(&c, "pc", &id, "a", hide, 3);
    publish(&c, "pp", &id, "a", false, 2);
    question(&c, "pc", "q1", 1, "[]", None);
    question(&c, "pc", "q0", 0, "[]", None);
    (c, d, id)
}

#[test]
fn only_those_who_may_see_an_item_load_it() {
    let (c, _d, id) = world(false);
    assert!(load_visible(&c, &id, "r").unwrap().is_some());
    assert!(load_visible(&c, &id, "a").unwrap().is_some());
    assert!(load_visible(&c, &"2".repeat(32), "r").unwrap().is_none());
    c.execute("UPDATE items SET visibility = 'unlisted'", []).unwrap();
    assert!(load_visible(&c, &id, "r").unwrap().is_none(), "not public: others do not see it");
    assert!(load_visible(&c, &id, "a").unwrap().is_some(), "its author does");
    c.execute("UPDATE items SET visibility = 'public'", []).unwrap();
    c.execute("INSERT INTO item_holds (item_id, reason, at) VALUES (?1, 'admin', 1)", [&id]).unwrap();
    assert!(load_visible(&c, &id, "r").unwrap().is_none(), "held");
}

#[test]
fn the_served_form_is_stripped_only_for_a_non_author_of_a_hidden_key_publish() {
    assert_eq!(served_form("a", 1, "a"), Form::Full);
    assert_eq!(served_form("a", 1, "r"), Form::Stripped);
    assert_eq!(served_form("a", 0, "r"), Form::Full);
    assert_eq!(served_form("a", 0, "a"), Form::Full);
}

#[test]
fn the_view_shows_the_author_the_current_publish_and_others_the_public_one() {
    let (c, _d, id) = world(true);
    let row = load_visible(&c, &id, "a").unwrap().unwrap();
    let mine = item_view(&row, "a", Form::Full);
    assert_eq!((mine["version"].clone(), mine["publish_id"].clone(), mine["committed_at"].clone()), (json!(3), json!("pc"), json!(100)));
    assert_eq!((mine["visibility"].clone(), mine["is_author"].clone(), mine["hide_key"].clone()), (json!("public"), json!(true), json!(true)));
    let theirs = item_view(&row, "r", Form::Stripped);
    assert_eq!((theirs["version"].clone(), theirs["publish_id"].clone(), theirs["committed_at"].clone()), (json!(2), json!("pp"), json!(50)));
    assert!(theirs.get("visibility").is_none());
    assert_eq!((theirs["form"].clone(), theirs["is_author"].clone()), (json!("stripped"), json!(false)));
    assert_eq!((theirs["kinds"].clone(), theirs["author_name"].clone(), theirs["has_images"].clone()), (json!(["Mc", "Matching"]), json!("Teacher a"), json!(true)));
    // Neither view carries a key or a hash of the full form.
    let text = theirs.to_string();
    assert!(!text.contains("FK") && !text.contains("full_key") && !text.contains("sha256"));
    assert_eq!(kinds_of(0), Vec::<&str>::new());
    assert_eq!(kinds_of(63).len(), 6);
}

#[test]
fn questions_show_the_served_hash_only_and_in_position_order() {
    let (c, _d, _id) = world(true);
    let (q, check) = questions_for(&c, "pc", Form::Stripped, "CC-BY-4.0").unwrap();
    assert_eq!(q.iter().map(|x| x["qid"].as_str().unwrap()).collect::<Vec<_>>(), vec!["q0", "q1"]);
    assert_eq!(q[0]["served_sha256"], json!("STRIPSHA"));
    assert!(!serde_json::to_string(&q).unwrap().contains("FULLSHA"), "a stripped serve never names the full hash");
    assert_eq!(questions_for(&c, "pc", Form::Full, "CC-BY-4.0").unwrap().0[0]["served_sha256"], json!("FULLSHA"));
    assert_eq!(q[0]["match_hash"], json!("mh-q0"));
    assert_eq!(check.status, "ok");
}

#[test]
fn a_long_chain_is_collapsed_and_a_stored_record_that_forbids_the_licence_marks_a_mismatch() {
    let (c, _d, _id) = world(false);
    let chain: Vec<ChainLink> = (0..40).map(|i| ChainLink { item_id: format!("{:032x}", i + 1), license: "CC-BY-4.0".into(), author_name: format!("A{i}"), ..Default::default() }).collect();
    c.execute("UPDATE item_questions SET chain_json = ?1 WHERE qid = 'q0'", [serde_json::to_string(&chain).unwrap()]).unwrap();
    c.execute("UPDATE item_questions SET allowed_json = ?1 WHERE qid = 'q1'", [r#"{"allowed":["CC-BY-NC-4.0"],"binders":[]}"#]).unwrap();
    let (q, check) = questions_for(&c, "pc", Form::Full, "CC0-1.0").unwrap();
    let shown = q[0]["chain"].as_array().unwrap();
    assert_eq!(shown.len(), CHAIN_MAX + 1);
    assert_eq!(shown[8]["author_name"], json!("and 8 contributors between"));
    assert_eq!(check.status, "mismatch");
}
