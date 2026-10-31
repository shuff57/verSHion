//! Tests for `blocking.rs` on the real migrated schema: the preview token (bound, expiring, once), the selection reader, the
//! plan digest, the SQL fragments, and the two hold sweeps.
use super::*;
use crate::db;
use crate::pool::ctx::Fail;
use rusqlite::Connection;
use std::path::Path;

fn open() -> (Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    (conn, dir)
}
fn keys() -> PoolKeys {
    PoolKeys::from_secret("test-secret-0123456789abcdef0123456789abcdef")
}
fn plan(sel_keys: &[&str], sel_blobs: &[&str], affected: &[&str]) -> BlockPlan {
    BlockPlan {
        item_id: "i".repeat(1),
        matches: vec![MatchPlan {
            pos: 0,
            hash: "mh0".into(),
            keys: sel_keys.iter().map(|k| k.to_string()).collect(),
            scope: "stem".into(),
            stem_wide: false,
            generic_stem: None,
            generic_authors: None,
            suggest_scope: None,
            affects: None,
            other_authors: 0,
            introduced: true,
            selected: true,
            suggested: None,
            first: None,
        }],
        blob: sel_blobs.iter().map(|b| BlobPlan { sha256: b.to_string(), other_authors: 0, introduced: true, selected: true, suggested: None }).collect(),
        affected: affected.iter().map(|a| Affected { item_id: a.to_string(), author_id: "au".into(), title: "T".into(), via: "question".into() }).collect(),
        affected_total: affected.len() as i64,
        ..Default::default()
    }
}
fn bind(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}
fn refused(r: Res<()>) -> (u16, String) {
    match r {
        Err(Fail::Pool(e)) => (e.status, e.code),
        other => panic!("{other:?}"),
    }
}

#[test]
fn scopes_are_the_six_names_and_part_numbers_without_leading_zeros() {
    for ok in ["default", "skeleton", "stem", "full", "explanation", "answer", "part:0", "part:7", "part:999"] {
        assert!(is_scope(&json!(ok)), "{ok}");
    }
    for bad in ["part:", "part:01", "part:1000", "part:-1", "Stem", "all", ""] {
        assert!(!is_scope(&json!(bad)), "{bad}");
    }
    assert!(!is_scope(&json!(5)));
}

#[test]
fn the_selected_keys_blobs_and_pairs_come_from_the_selected_positions_only() {
    let mut p = plan(&["k1", "k2", "k1"], &["b1"], &["x"]);
    p.matches.push(MatchPlan { selected: false, keys: vec!["unselected".into()], hash: "mh1".into(), ..p.matches[0].clone() });
    p.blob.push(BlobPlan { selected: false, ..p.blob[0].clone() });
    assert_eq!(selected_match(&p), vec!["k1", "k2"]);
    assert_eq!(selected_blobs(&p), vec!["b1"]);
    assert_eq!(selected_key_pairs(&p), vec![("k1".to_string(), "mh0".to_string()), ("k2".to_string(), "mh0".to_string())]);
}

#[test]
fn the_digest_binds_the_kind_selection_affected_items_and_parameters_in_a_stable_order() {
    let p = plan(&["k2", "k1"], &["b"], &["y", "x"]);
    let a = plan_digest("dmca", &p, &bind(json!({ "b": 1, "a": 2 })));
    assert_eq!(a, plan_digest("dmca", &p, &bind(json!({ "a": 2, "b": 1 }))), "parameter order does not matter");
    assert_eq!(a, r#"["dmca","i",["k1","k2"],["b"],["x","y"],2,[["a",2],["b",1]]]"#);
    assert_ne!(a, plan_digest("privacy", &p, &bind(json!({ "b": 1, "a": 2 }))));
    assert_ne!(a, plan_digest("dmca", &p, &bind(json!({ "b": 1, "a": 3 }))));
    assert_ne!(a, plan_digest("dmca", &plan(&["k1"], &["b"], &["y", "x"]), &bind(json!({ "b": 1, "a": 2 }))));
    assert_ne!(a, plan_digest("dmca", &plan(&["k2", "k1"], &["b"], &["x"]), &bind(json!({ "b": 1, "a": 2 }))));
}

#[test]
fn a_preview_token_confirms_exactly_what_was_previewed_once() {
    let (c, _d) = open();
    let k = keys();
    let p = plan(&["k1"], &[], &["x"]);
    let b = bind(json!({ "admin": "adm" }));
    let x = PoolCtx::new(&c, 1_000);
    let t = preview_token(&x, &k, "dmca", &p, &b);
    // A different selection, kind or parameter does not accept it (and does not consume it).
    assert_eq!(refused(check_preview(&x, &k, "dmca", &plan(&["k9"], &[], &["x"]), &json!(t), &b)), (428, "preview_required".into()));
    assert_eq!(refused(check_preview(&x, &k, "privacy", &p, &json!(t), &b)), (428, "preview_required".into()));
    assert_eq!(refused(check_preview(&x, &k, "dmca", &p, &json!(t), &bind(json!({ "admin": "other" })))).1, "preview_required");
    check_preview(&x, &k, "dmca", &p, &json!(t), &b).unwrap();
    assert_eq!(refused(check_preview(&x, &k, "dmca", &p, &json!(t), &b)).1, "preview_required", "once");
    // Expired at PREVIEW_MS; and not valid under another key.
    let t2 = preview_token(&x, &k, "dmca", &p, &b);
    let late = PoolCtx::new(&c, 1_000 + PREVIEW_MS + 1);
    assert_eq!(refused(check_preview(&late, &k, "dmca", &p, &json!(t2), &b)).1, "preview_required");
    let t3 = preview_token(&x, &k, "dmca", &p, &b);
    assert_eq!(refused(check_preview(&x, &PoolKeys::from_secret("another-secret-0123456789abcdef012345"), "dmca", &p, &json!(t3), &b)).1, "preview_required");
    check_preview(&PoolCtx::new(&c, 1_000 + PREVIEW_MS), &k, "dmca", &p, &json!(t3), &b).unwrap();
}

#[test]
fn malformed_tokens_are_all_the_same_refusal() {
    let (c, _d) = open();
    let k = keys();
    let p = plan(&["k1"], &[], &[]);
    let x = PoolCtx::new(&c, 1_000);
    let good = preview_token(&x, &k, "dmca", &p, &Map::new());
    let (body, mac) = good.split_once('.').unwrap();
    for t in [json!(null), json!(5), json!(""), json!("nodot"), json!(format!("{body}.")), json!(format!(".{mac}")), json!(format!("{body}.{}", "A".repeat(43))), json!(format!("{body}.{mac}x")), json!(format!("{}.{mac}", "A".repeat(601)))] {
        assert_eq!(refused(check_preview(&x, &k, "dmca", &p, &t, &Map::new())), (428, "preview_required".into()), "{t}");
    }
    let r = match check_preview(&x, &k, "dmca", &p, &json!("x"), &Map::new()) {
        Err(Fail::Pool(e)) => e.body(),
        other => panic!("{other:?}"),
    };
    assert!(r["message"].as_str().unwrap().contains("Preview first"));
    check_preview(&x, &k, "dmca", &p, &json!(good), &Map::new()).unwrap();
}

#[test]
fn a_selection_is_read_from_the_body_with_its_limits() {
    assert!(read_selection(None).unwrap().questions.is_none());
    let s = read_selection(Some(&bind(json!({ "questions": "all", "images": ["a"], "scopes": { "0": "full" } })))).unwrap();
    assert!(s.all && s.questions.is_none());
    assert_eq!(s.images, Some(vec!["a".to_string()]));
    assert_eq!(s.scopes.unwrap()["0"], json!("full"));
    assert_eq!(read_selection(Some(&bind(json!({ "questions": [0, 2] })))).unwrap().questions, Some(vec![0, 2]));
    let field = |b: Value| match read_selection(Some(&bind(b))) {
        Err(Fail::Pool(e)) => e.body()["field"].as_str().unwrap().to_string(),
        other => panic!("{other:?}"),
    };
    assert_eq!(field(json!({ "questions": "some" })), "questions");
    assert_eq!(field(json!({ "questions": 3 })), "questions");
    assert_eq!(field(json!({ "questions": vec![0; 1001] })), "questions");
    assert_eq!(field(json!({ "scopes": [] })), "scopes");
    assert_eq!(field(json!({ "scopes": null })), "scopes");
    assert_eq!(field(json!({ "images": [1] })), "images");
    assert_eq!(field(json!({ "images": "a" })), "images");
}

fn user(c: &Connection, id: &str) {
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?1, 'teacher', 1)", params![id, format!("{id}@x.org")]).unwrap();
}
fn item(c: &Connection, id: &str, publish: &str) {
    c.execute(
        "INSERT INTO items (id, author_id, kind, visibility, current_publish_id, public_publish_id, public_version, version, title, license, committed_at, created_at, updated_at, fold_rev)
         VALUES (?1, 'a', 'bank', 'public', ?2, ?2, 1, 1, 'T', 'CC-BY-4.0', 1, 1, 1, 1)",
        params![id, publish],
    )
    .unwrap();
    c.execute(
        "INSERT INTO item_publishes (publish_id, item_id, author_id, version, public_version, status, schema_version, hide_key, license, attested_at, attest_text_rev,
            acks_json, request_sha256, stage_token, full_key, stripped_key, full_bytes, title, subject, course, grade, kinds_mask, question_count, has_images, created_at)
         VALUES (?1, ?2, 'a', 1, 1, 'current', 2, 0, 'CC-BY-4.0', 1, 'r', '{}', 'x', ?1, 'FK', 'SK', 10, '', '', '', '', 0, 1, 1, 1)",
        params![publish, id],
    )
    .unwrap();
}
fn question(c: &Connection, publish: &str, pos: i64, match_hash: &str) {
    let required: Vec<String> = {
        let mut st = c.prepare("SELECT name FROM pragma_table_info('item_questions') WHERE \"notnull\" = 1 AND dflt_value IS NULL").unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    let vals: Vec<String> = required.iter().map(|n| if n == "pos" { "0".to_string() } else { "''".to_string() }).collect();
    c.execute(&format!("INSERT INTO item_questions ({}) VALUES ({})", required.join(", "), vals.join(", ")), []).unwrap();
    c.execute("UPDATE item_questions SET publish_id = ?1, qid = ?2, pos = ?3, match_hash = ?4 WHERE rowid = last_insert_rowid()", params![publish, format!("q{pos}{publish}"), pos, match_hash]).unwrap();
}
fn id(n: u8) -> String {
    format!("{:032x}", n)
}

#[test]
fn a_privacy_hold_sweep_holds_users_of_blocked_text_in_batches_and_snapshots_the_positions() {
    let (c, _d) = open();
    user(&c, "a");
    for n in 1..=5u8 {
        item(&c, &id(n), &format!("p{n}"));
        question(&c, &format!("p{n}"), 0, "other");
        question(&c, &format!("p{n}"), 1, if n == 1 { "src" } else { "bad" });
    }
    item(&c, &id(6), "p6");
    question(&c, "p6", 0, "clean");
    let x = PoolCtx::new(&c, 7_000);
    // Item 1 is the source (excepted); 2..=5 use the blocked text; batch size 2.
    assert!(hold_privacy_batch(&x, 2, &id(1), &["bad".into()], &[], Some("adm"), 1).unwrap(), "a full batch: some may remain");
    let held = |c: &Connection| -> Vec<String> { c.prepare("SELECT item_id FROM item_holds ORDER BY item_id").unwrap().query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect() };
    assert_eq!(held(&c).len(), 2);
    assert!(!hold_privacy_batch(&x, 2, &id(1), &["bad".into()], &[], Some("adm"), 5).unwrap(), "all done within the batches");
    assert_eq!(held(&c), vec![id(2), id(3), id(4), id(5)]);
    let (reason, source, by, detail): (String, String, String, String) = c.query_row("SELECT reason, source, by, detail FROM item_holds WHERE item_id = ?1", [id(2)], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).unwrap();
    assert_eq!((reason.as_str(), source.as_str(), by.as_str()), ("student_data", "privacy", "adm"));
    assert_eq!(serde_json::from_str::<Value>(&detail).unwrap(), json!({ "publish_id": "p2", "positions": [1] }));
    // The authors were told (one notice per held item, within the hourly budget).
    assert!(!x.take_notices().is_empty());
}

#[test]
fn orphan_holds_are_released_when_no_active_block_is_used_any_more() {
    let (c, _d) = open();
    user(&c, "a");
    for n in 1..=3u8 {
        item(&c, &id(n), &format!("p{n}"));
        question(&c, &format!("p{n}"), 0, if n == 3 { "free" } else { "bad" });
        c.execute("INSERT INTO item_holds (item_id, reason, at, by, source) VALUES (?1, 'student_data', 1, 'adm', 'privacy')", [id(n)]).unwrap();
    }
    c.execute("INSERT INTO item_holds (item_id, reason, at, source) VALUES (?1, 'reports', 1, '')", [id(3)]).unwrap();
    let x = PoolCtx::new(&c, 9_000);
    c.execute("INSERT INTO privacy_blocks (kind, hash, item_id, at) VALUES ('match', 'bad', ?1, 1)", [id(1)]).unwrap();
    assert_eq!(release_orphan_holds(&x, "privacy", "student_data").unwrap(), 1, "only item 3 no longer uses a block");
    let left: Vec<(String, String)> = c.prepare("SELECT item_id, reason FROM item_holds ORDER BY item_id, reason").unwrap().query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(Result::unwrap).collect();
    assert_eq!(left, vec![(id(1), "student_data".into()), (id(2), "student_data".into()), (id(3), "reports".into())], "another reason's hold is untouched");
    c.execute("DELETE FROM privacy_blocks", []).unwrap();
    assert_eq!(release_orphan_holds(&x, "privacy", "student_data").unwrap(), 2);
    // A DMCA source counts only its active (unlifted, unexpired) blocks.
    let act = "ab12";
    c.execute("INSERT INTO item_holds (item_id, reason, at, source) VALUES (?1, 'blocklist', 1, ?2)", params![id(1), format!("dmca:{act}")]).unwrap();
    c.execute("INSERT INTO blocked_hashes (kind, hash, action_id, at, expires_at) VALUES ('match', 'bad', ?1, 1, 9_001)", [act]).unwrap();
    assert_eq!(release_orphan_holds(&x, &format!("dmca:{act}"), "blocklist").unwrap(), 0, "still active");
    assert_eq!(release_orphan_holds(&PoolCtx::new(&c, 9_001), &format!("dmca:{act}"), "blocklist").unwrap(), 1, "expired at its instant");
}
