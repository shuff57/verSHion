//! Tests for `republish.rs` on the real migrated schema: what an account may republish a just-served copy as depends on
//! what it holds of that question, under which licences.
use super::*;
use crate::db;
use crate::pool::license::LICENSES;
use rusqlite::Connection;
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
fn source(c: &Connection, item: &str, publish: &str, mh: &str, licence: &str, short: i64) {
    c.execute("INSERT INTO items (id, author_id, kind, visibility, current_publish_id, license, created_at, updated_at) VALUES (?1, 'b', 'bank', 'public', ?2, ?3, 1, 1)", params![item, publish, licence]).unwrap();
    c.execute(
        "INSERT INTO item_publishes (publish_id, item_id, author_id, version, public_version, status, schema_version, hide_key, license, attested_at, attest_text_rev,
            acks_json, request_sha256, stage_token, full_key, stripped_key, full_bytes, title, subject, course, grade, kinds_mask, question_count, has_images, created_at)
         VALUES (?1, ?2, 'b', 1, 1, 'current', 2, 0, ?3, 1, 'r', '{}', 'x', ?1, 'k', 's', 10, '', '', '', '', 0, 1, 1, 1)",
        params![publish, item, licence],
    )
    .unwrap();
    let required: Vec<String> = {
        let mut st = c.prepare("SELECT name FROM pragma_table_info('item_questions') WHERE \"notnull\" = 1 AND dflt_value IS NULL").unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    let vals: Vec<String> = required.iter().map(|n| if n == "pos" { "0".to_string() } else { "''".to_string() }).collect();
    c.execute(&format!("INSERT INTO item_questions ({}) VALUES ({})", required.join(", "), vals.join(", ")), []).unwrap();
    c.execute(
        "UPDATE item_questions SET publish_id = ?1, qid = 'q', match_hash = ?2, seq = 5, full_content = 'FULL', stripped_content = 'STRIP', keyfree_content = 'KF',
            near_full = 'NEAR', near_stripped = 'NEARS', near_kf = 'NKF', short = ?3, chain_json = '[]' WHERE rowid = last_insert_rowid()",
        params![publish, mh, short],
    )
    .unwrap();
}
fn serve(c: &Connection, reader: &str, item: &str, publish: &str, kind: &str, licence: &str) {
    c.execute(
        "INSERT INTO served (reader_id, item_id, publish_id, kind, license, licenses_json, first_at, at) VALUES (?1, ?2, ?3, ?4, ?5, json_array(?5), 1, 5)",
        params![reader, item, publish, kind, licence],
    )
    .unwrap();
}
fn world() -> (Connection, tempfile::TempDir) {
    let (c, d) = open();
    user(&c, "b");
    user(&c, "me");
    (c, d)
}

#[test]
fn nothing_held_means_every_licence_is_open() {
    let (c, _d) = world();
    source(&c, &"1".repeat(32), "pb", "M", "CC-BY-4.0", 0);
    let x = PoolCtx::new(&c, 1_000);
    assert_eq!(may_republish_as(&x, "me", "pb", Form::Full).unwrap(), LICENSES.to_vec());
}

#[test]
fn an_import_under_a_noncommercial_licence_narrows_what_a_verbatim_copy_may_be() {
    let (c, _d) = world();
    let ib = "1".repeat(32);
    source(&c, &ib, "pb", "M", "CC-BY-NC-4.0", 0);
    serve(&c, "me", &ib, "pb", "import", "CC-BY-NC-4.0");
    let x = PoolCtx::new(&c, 1_000);
    let allowed = may_republish_as(&x, "me", "pb", Form::Full).unwrap();
    assert!(allowed.contains(&"CC-BY-NC-4.0"), "{allowed:?}");
    assert!(!allowed.contains(&"CC0-1.0") && allowed.len() < LICENSES.len(), "{allowed:?}");
}

#[test]
fn a_plain_read_of_a_verbatim_copy_still_binds() {
    let (c, _d) = world();
    let ib = "1".repeat(32);
    source(&c, &ib, "pb", "M", "CC-BY-NC-4.0", 0);
    serve(&c, "me", &ib, "pb", "view", "CC-BY-NC-4.0");
    let x = PoolCtx::new(&c, 1_000);
    // The copy is verbatim (same near content as what was served): a read of it still binds.
    let verbatim = may_republish_as(&x, "me", "pb", Form::Full).unwrap();
    assert!(!verbatim.contains(&"CC0-1.0"), "{verbatim:?}");
}

#[test]
fn a_hidden_key_serve_is_judged_on_the_stripped_content() {
    let (c, _d) = world();
    let ib = "1".repeat(32);
    source(&c, &ib, "pb", "M", "CC-BY-NC-4.0", 0);
    serve(&c, "me", &ib, "pb", "import", "CC-BY-NC-4.0");
    let x = PoolCtx::new(&c, 1_000);
    // Stripped: the subject is hidden, compared on key-free facts only, and is still the same question.
    let allowed = may_republish_as(&x, "me", "pb", Form::Stripped).unwrap();
    assert!(!allowed.contains(&"CC0-1.0"), "{allowed:?}");
}

#[test]
fn an_unknown_publish_has_no_questions_and_so_allows_every_licence() {
    let (c, _d) = world();
    let x = PoolCtx::new(&c, 1_000);
    assert_eq!(may_republish_as(&x, "me", "nope", Form::Full).unwrap(), LICENSES.to_vec());
}
