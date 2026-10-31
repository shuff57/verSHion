//! Tests for `modtakedown.rs`: a takedown disables at once, opens the window, strikes pending, blocks re-uploads and quarantines; a retry resumes; the rightful
//! check wants an earlier copy by another author.
use super::*;
use crate::db;
use crate::pool::blocking::{BlobPlan, MatchPlan};
use rusqlite::Connection;
use std::path::Path;

fn world() -> (Connection, tempfile::TempDir, BlobStore) {
    let d = tempfile::tempdir().unwrap();
    let mut c = db::open(&d.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    for u in ["a", "b", "c"] {
        c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?3, 'teacher', 1)", params![u, format!("{u}@x.org"), u]).unwrap();
    }
    c.execute("INSERT INTO dmca_notices (id, received_at, complainant_name, complainant_contact, work_desc, material_desc, has_signature, has_good_faith, has_perjury_stmt, status, recorded_by) VALUES ('n', 1, 'C', 'c@x', 'w', 'm', 1, 1, 1, 'open', 'admin')", []).unwrap();
    let s = BlobStore::new(d.path().join("blobs")).unwrap();
    (c, d, s)
}
fn item(c: &Connection, id: &str, author: &str, publish: &str, hashes: &[(&str, i64)], origin: Option<&str>, committed: i64) {
    c.execute("INSERT INTO items (id, author_id, kind, visibility, current_publish_id, license, title, created_at, updated_at) VALUES (?1, ?2, 'bank', 'public', ?3, 'cc-by', ?1, 1, 1)", params![id, author, publish]).unwrap();
    c.execute(
        "INSERT INTO item_publishes (publish_id, item_id, author_id, version, status, schema_version, hide_key, license, attested_at, attest_text_rev, acks_json, request_sha256, stage_token,
           full_key, full_bytes, title, subject, course, grade, kinds_mask, question_count, has_images, created_at, committed_at)
         VALUES (?1, ?2, ?3, 1, 'current', 1, 0, 'cc-by', 1, 'r', '{}', 'x', 't', ?4, 1, 'T', '', '', '', 0, 0, 0, 1, ?5)",
        params![publish, id, author, format!("items/{id}/{publish}.json"), committed],
    )
    .unwrap();
    for (i, (h, seq)) in hashes.iter().enumerate() {
        c.execute("INSERT INTO item_questions (publish_id, qid, pos, qkind, match_hash, full_sha256, full_content, keyfree_content, origin_item_id, seq) VALUES (?1, ?2, ?3, 'mc', ?4, 'f', 'fc', 'kc', ?5, ?6)", params![publish, format!("{publish}q{i}"), i as i64, h, origin, seq]).unwrap();
    }
}
fn n(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}
fn plan(hashes: &[&str]) -> BlockPlan {
    BlockPlan {
        item_id: "src".into(),
        matches: hashes
            .iter()
            .enumerate()
            .map(|(i, h)| MatchPlan {
                pos: i as i64,
                hash: h.to_string(),
                keys: vec![h.to_string()],
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
            })
            .collect(),
        blob: vec![BlobPlan { sha256: "img".into(), other_authors: 0, introduced: true, selected: true, suggested: None }],
        ..Default::default()
    }
}
fn code(r: Res<TakedownOut>) -> (u16, String) {
    match r {
        Err(Fail::Pool(p)) => (p.status, p.code.clone()),
        Err(e) => panic!("{e:?}"),
        Ok(_) => panic!("ok"),
    }
}

#[test]
fn a_takedown_disables_strikes_pending_blocks_and_quarantines() {
    let (c, _d, s) = world();
    item(&c, "src", "a", "ps", &[("h1", 1)], None, 5);
    item(&c, "cp", "b", "pc", &[("h1", 2)], Some("src"), 6);
    s.put("items/src/ps.json", b"body").unwrap();
    let x = PoolCtx::new(&c, 1_000);
    let out = takedown(&x, &s, "adm", "n", "src", &plan(&["h1"]), None, &["c".to_string()], None).unwrap();
    assert_eq!(out.copies.iter().map(|c| c.item_id.as_str()).collect::<Vec<_>>(), vec!["cp"]);
    assert_eq!((c.query_row::<String, _, _>("SELECT removed_reason FROM items WHERE id = 'src'", [], |r| r.get(0)).unwrap().as_str(), n(&c, "SELECT quarantined_at FROM items WHERE id = 'src'")), ("takedown", 1_000));
    assert_eq!(n(&c, "SELECT COUNT(*) FROM strikes WHERE upheld_at IS NULL AND action_id = (SELECT id FROM dmca_actions)"), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM blocked_hashes WHERE kind = 'match' AND hash = 'h1'") + n(&c, "SELECT COUNT(*) FROM blocked_hashes WHERE kind = 'blob' AND hash = 'img'"), 2);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM dmca_exempt WHERE user_id = 'c'"), 1);
    assert_eq!(c.query_row::<String, _, _>("SELECT status FROM dmca_notices", [], |r| r.get(0)).unwrap(), "actioned");
    assert_eq!(n(&c, "SELECT COUNT(*) FROM pool_outbox WHERE id LIKE 'takedown_author:%'"), 1);
    assert!(c.query_row::<String, _, _>("SELECT full_key FROM item_publishes WHERE publish_id = 'ps'", [], |r| r.get(0)).unwrap().starts_with("quarantine/"));
    assert_eq!(n(&c, "SELECT COUNT(*) FROM pool_audit WHERE action = 'dmca.takedown'"), 1);
    // A retry of the same notice resumes it; another notice is refused.
    let again = takedown(&PoolCtx::new(&c, 2_000), &s, "adm", "n", "src", &plan(&["h1"]), None, &[], None).unwrap();
    assert_eq!(again.action_id, out.action_id);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM dmca_actions"), 1);
    c.execute("INSERT INTO dmca_notices (id, received_at, complainant_name, complainant_contact, work_desc, material_desc, has_signature, has_good_faith, has_perjury_stmt, status, recorded_by) VALUES ('n2', 1, 'C', 'c@x', 'w', 'm', 1, 1, 1, 'open', 'admin')", []).unwrap();
    assert_eq!(code(takedown(&PoolCtx::new(&c, 3_000), &s, "adm", "n2", "src", &plan(&["h1"]), None, &[], None)), (409, "already_removed".into()));
}

#[test]
fn a_deficient_notice_or_a_missing_item_is_refused() {
    let (c, _d, s) = world();
    item(&c, "src", "a", "ps", &[("h1", 1)], None, 5);
    let x = PoolCtx::new(&c, 1_000);
    assert_eq!(code(takedown(&x, &s, "adm", "nope", "src", &plan(&[]), None, &[], None)), (404, "notice_not_found".into()));
    assert_eq!(code(takedown(&x, &s, "adm", "n", "ghostly", &plan(&[]), None, &[], None)), (404, "not_found".into()));
    c.execute("UPDATE dmca_notices SET status = 'deficient'", []).unwrap();
    assert_eq!(code(takedown(&x, &s, "adm", "n", "src", &plan(&[]), None, &[], None)), (409, "notice_not_actionable".into()));
}

#[test]
fn an_item_purged_before_the_notice_is_taken_down_as_a_ghost() {
    let (c, _d, s) = world();
    item(&c, "src", "a", "ps", &[("h1", 1)], None, 5);
    c.execute("UPDATE items SET purged_at = 50, removed_at = 50, removed_reason = 'author_deleted', current_publish_id = NULL WHERE id = 'src'", []).unwrap();
    let mut p = plan(&["h1"]);
    p.selection_required = Some(true);
    assert_eq!(code(takedown(&PoolCtx::new(&c, 1_000), &s, "adm", "n", "src", &p, None, &[], None)), (409, "selection_required".into()));
    p.selection_required = None;
    let out = takedown(&PoolCtx::new(&c, 1_000), &s, "adm", "n", "src", &p, None, &[], None).unwrap();
    assert_eq!(c.query_row::<String, _, _>("SELECT removed_reason FROM items", [], |r| r.get(0)).unwrap(), "takedown");
    assert_eq!(c.query_row::<String, _, _>("SELECT prev_removed_reason FROM dmca_actions WHERE id = ?1", [&out.action_id], |r| r.get(0)).unwrap(), "author_deleted");
    assert_eq!(n(&c, "SELECT COUNT(*) FROM strikes WHERE upheld_at IS NULL"), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM blocked_hashes"), 2);
}

#[test]
fn the_rightful_item_must_be_visible_by_another_author_and_earlier() {
    let (c, _d, _s) = world();
    // X (a) has the question at seq 5; R (b) had it at seq 2 and did not derive it from X.
    item(&c, "x", "a", "px", &[("h1", 5)], None, 50);
    item(&c, "r", "b", "pr", &[("h1", 2)], None, 20);
    let x = PoolCtx::new(&c, 100);
    let ok = rightful_check(&x, "r", "x", &["h1".to_string()], &[]).unwrap();
    assert!(ok.ok, "{ok:?}");
    assert_eq!((ok.hashes[0].rightful_seq, ok.hashes[0].item_seq), (Some(2), Some(5)));
    // With no block it falls back to the shared questions.
    assert!(rightful_check(&x, "r", "x", &[], &[]).unwrap().ok);
    // Later than X: not the rightful one.
    c.execute("UPDATE item_questions SET seq = 9 WHERE publish_id = 'pr'", []).unwrap();
    let late = rightful_check(&x, "r", "x", &["h1".to_string()], &[]).unwrap();
    assert_eq!((late.ok, late.reasons.clone()), (false, vec!["not_earlier_in_rightful"]));
    // Derived from X by its origin.
    c.execute("UPDATE item_questions SET origin_item_id = 'x', seq = 2 WHERE publish_id = 'pr'", []).unwrap();
    assert!(rightful_check(&x, "r", "x", &["h1".to_string()], &[]).unwrap().reasons.contains(&"rightful_derived_from_item"));
    // Same author and unknown items.
    assert!(rightful_check(&x, "x", "x", &[], &[]).unwrap().reasons.contains(&"same_author"));
    assert_eq!(rightful_check(&x, "nope", "x", &[], &[]).unwrap().reasons, vec!["rightful_not_found"]);
}
