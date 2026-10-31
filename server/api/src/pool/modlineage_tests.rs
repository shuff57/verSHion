//! Tests for `modlineage.rs`: copies need lineage, the rightful author is never a copy, bodies move to quarantine and back, the erased author's evidence stays.
use super::*;
use crate::db;
use rusqlite::Connection;
use std::path::Path;

fn world() -> (Connection, tempfile::TempDir, BlobStore) {
    let d = tempfile::tempdir().unwrap();
    let mut c = db::open(&d.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    for u in ["a", "b", "c"] {
        c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?3, 'teacher', 1)", params![u, format!("{u}@x.org"), u]).unwrap();
    }
    let s = BlobStore::new(d.path().join("blobs")).unwrap();
    (c, d, s)
}
fn item(c: &Connection, id: &str, author: &str, publish: &str, hashes: &[&str], origin: Option<&str>) {
    c.execute("INSERT INTO items (id, author_id, kind, visibility, current_publish_id, license, title, created_at, updated_at) VALUES (?1, ?2, 'bank', 'public', ?3, 'cc-by', ?1, 1, 1)", params![id, author, publish]).unwrap();
    c.execute(
        "INSERT INTO item_publishes (publish_id, item_id, author_id, version, status, schema_version, hide_key, license, attested_at, attest_text_rev, acks_json, request_sha256, stage_token,
           full_key, stripped_key, full_bytes, title, subject, course, grade, kinds_mask, question_count, has_images, created_at, committed_at)
         VALUES (?1, ?2, ?3, 1, 'current', 1, 0, 'cc-by', 1, 'r', '{}', 'x', 't', ?4, ?5, 1, 'T', '', '', '', 0, 0, 0, 1, 1)",
        params![publish, id, author, format!("items/{id}/{publish}.json"), format!("items/{id}/{publish}.s.json")],
    )
    .unwrap();
    for (i, h) in hashes.iter().enumerate() {
        c.execute(
            "INSERT INTO item_questions (publish_id, qid, pos, qkind, match_hash, full_sha256, full_content, keyfree_content, origin_item_id) VALUES (?1, ?2, ?3, 'mc', ?4, 'f', 'fc', 'kc', ?5)",
            params![publish, format!("{publish}q{i}"), i as i64, h, origin],
        )
        .unwrap();
    }
}
fn ids(v: &[Copy]) -> Vec<&str> {
    v.iter().map(|c| c.item_id.as_str()).collect()
}

#[test]
fn a_copy_needs_lineage_and_the_whole_item() {
    let (c, _d, _s) = world();
    item(&c, "src", "a", "ps", &["h1", "h2"], None);
    item(&c, "cp", "b", "pc", &["h1", "h2", "h3"], Some("src"));
    // Shares the questions but has no lineage (no import, no origin): not a copy.
    item(&c, "ind", "c", "pi", &["h1", "h2"], None);
    let x = PoolCtx::new(&c, 100);
    let got = lineage_copies(&x, "src", None, false, None, &LineageOpts::default()).unwrap();
    assert_eq!(ids(&got), vec!["cp"]);
    assert!(got[0].visible);
    // Widened by the admin: the independent one too.
    let wide = lineage_copies(&x, "src", None, true, None, &LineageOpts::default()).unwrap();
    assert_eq!(ids(&wide), vec!["cp", "ind"]);
    // An import by the independent author gives it lineage.
    c.execute("INSERT INTO imports (user_id, item_id, kind, publish_id, at) VALUES ('c', 'src', 'import', 'ps', 1)", []).ok();
    // A designated question: copies by content.
    let des = lineage_copies(&x, "src", Some(&["h2".to_string()]), false, None, &LineageOpts::default()).unwrap();
    assert!(ids(&des).contains(&"cp"));
}

#[test]
fn the_rightful_author_and_exempt_authors_are_never_copies() {
    let (c, _d, _s) = world();
    item(&c, "src", "a", "ps", &["h1"], None);
    item(&c, "orig", "b", "po", &["h1"], Some("src"));
    item(&c, "cp", "c", "pc", &["h1"], Some("src"));
    c.execute("INSERT INTO dmca_notices (id, received_at, complainant_name, complainant_contact, work_desc, material_desc, has_signature, has_good_faith, has_perjury_stmt, status, recorded_by) VALUES ('n', 1, 'C', 'c@x', 'w', 'm', 1, 1, 1, 'actioned', 'admin')", []).unwrap();
    c.execute("INSERT INTO dmca_actions (id, notice_id, item_id, author_id, disabled_at, author_notified_at, rightful_item_id) VALUES ('act', 'n', 'src', 'a', 1, 1, 'orig')", []).unwrap();
    let x = PoolCtx::new(&c, 100);
    assert_eq!(ids(&lineage_copies(&x, "src", None, false, None, &LineageOpts::default()).unwrap()), vec!["cp"]);
    c.execute("INSERT INTO dmca_exempt (action_id, user_id, at) VALUES ('act', 'c', 1)", []).unwrap();
    assert!(lineage_copies(&x, "src", None, false, None, &LineageOpts::default()).unwrap().is_empty());
    // An item already held by the given source is left out.
    c.execute("DELETE FROM dmca_exempt", []).unwrap();
    c.execute("INSERT INTO item_holds (item_id, reason, at, by, source) VALUES ('cp', 'admin', 1, NULL, 'dmca:act')", []).unwrap();
    assert!(lineage_copies(&x, "src", None, false, None, &LineageOpts { held_by: "dmca:act", ..Default::default() }).unwrap().is_empty());
    assert_eq!(ids(&all_lineage_copies(&x, "src", None, false, None, &LineageOpts::default()).unwrap()), vec!["cp"]);
}

#[test]
fn bodies_move_to_quarantine_and_back() {
    let (c, _d, s) = world();
    item(&c, "i1", "a", "p1", &["h1"], None);
    s.put("items/i1/p1.json", b"full").unwrap();
    s.put("items/i1/p1.s.json", b"stripped").unwrap();
    let x = PoolCtx::new(&c, 100);
    let w = move_bodies(&x, &s, "i1", true).unwrap();
    assert_eq!(w.len(), 2);
    let (fk, sk): (String, String) = c.query_row("SELECT full_key, stripped_key FROM item_publishes", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!((fk.as_str(), sk.as_str()), ("quarantine/i1/p1/full.json", "quarantine/i1/p1/stripped.json"));
    assert_eq!(s.get(&fk).unwrap().unwrap(), b"full");
    assert!(s.get("items/i1/p1.json").unwrap().is_none());
    assert_eq!(c.query_row::<i64, _, _>("SELECT COUNT(*) FROM r2_orphans", [], |r| r.get(0)).unwrap(), 0);
    // Already there: nothing moves.
    assert!(move_bodies(&x, &s, "i1", true).unwrap().is_empty());
    // And back out.
    let back = move_bodies(&x, &s, "i1", false).unwrap();
    assert_eq!(back.len(), 2);
    let fk2: String = c.query_row("SELECT full_key FROM item_publishes", [], |r| r.get(0)).unwrap();
    assert!(fk2.starts_with("items/i1/") && s.get(&fk2).unwrap().unwrap() == b"full");
}

#[test]
fn finish_quarantines_completes_a_half_done_takedown() {
    let (c, _d, s) = world();
    item(&c, "i1", "a", "p1", &["h1"], None);
    s.put("items/i1/p1.json", b"full").unwrap();
    s.put("items/i1/p1.s.json", b"st").unwrap();
    c.execute("UPDATE items SET removed_at = 1, removed_reason = 'takedown'", []).unwrap();
    c.execute("INSERT INTO dmca_notices (id, received_at, complainant_name, complainant_contact, work_desc, material_desc, has_signature, has_good_faith, has_perjury_stmt, status, recorded_by) VALUES ('n', 1, 'C', 'c@x', 'w', 'm', 1, 1, 1, 'actioned', 'admin')", []).unwrap();
    c.execute("INSERT INTO dmca_actions (id, notice_id, item_id, author_id, disabled_at, author_notified_at) VALUES ('act', 'n', 'i1', 'a', 1, 1)", []).unwrap();
    assert_eq!(finish_quarantines(&PoolCtx::new(&c, 100), &s).unwrap(), 1);
    assert!(c.query_row::<String, _, _>("SELECT full_key FROM item_publishes", [], |r| r.get(0)).unwrap().starts_with("quarantine/"));
    assert_eq!(finish_quarantines(&PoolCtx::new(&c, 100), &s).unwrap(), 0);
}

#[test]
fn evidence_is_voided_only_if_the_author_could_have_countered() {
    let (c, _d, _s) = world();
    let x = PoolCtx::new(&c, 100);
    assert!(should_void_evidence(&x, "a", None).unwrap());
    c.execute("UPDATE users SET deleted_at = 5, email = NULL, pw_hash = NULL, pw_params = NULL, display_name = NULL, profile = NULL WHERE id = 'a'", []).unwrap();
    assert!(!should_void_evidence(&x, "a", None).unwrap());
    assert!(should_void_evidence(&x, "a", Some("orig")).unwrap());
}

#[test]
fn derivation_lists_origin_and_chain_sources_but_not_earlier_matches() {
    let (c, _d, _s) = world();
    item(&c, "cp", "b", "pc", &["h1", "h2"], Some("src"));
    c.execute("UPDATE item_questions SET chain_json = '[{\"item_id\":\"mid\"},{\"item_id\":\"skip\",\"earlier_match\":1},{\"item_id\":\"poss\",\"possible_source\":1}]' WHERE qid = 'pcq1'", []).unwrap();
    let x = PoolCtx::new(&c, 100);
    let mut got: Vec<String> = serde_json::from_str(&derivation_of(&x, Some("pc"), false).unwrap()).unwrap();
    got.sort();
    assert_eq!(got, vec!["mid", "src"]);
    assert_eq!(derivation_of(&x, None, true).unwrap(), "[]");
    assert_eq!(latest_ghost(&x, "cp").unwrap(), None);
}
