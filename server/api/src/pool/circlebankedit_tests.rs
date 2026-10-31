//! Tests for `circlebankedit.rs` on the real migrated schema: who is served a bank image, and the licence setting.
use super::*;
use crate::db;
use crate::pool::circles::create_circle;
use crate::pool::ctx::Fail;
use std::path::Path;

fn open() -> (rusqlite::Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    (conn, dir)
}
fn user(c: &rusqlite::Connection, u: &str) {
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?3, 'teacher', 1)", params![u, format!("{u}@x.org"), format!("Teacher {u}")]).unwrap();
}
fn err(f: Fail) -> (u16, String, Value) {
    match f {
        Fail::Pool(e) => (e.status, e.code.clone(), e.body()),
        other => panic!("{other:?}"),
    }
}
fn m(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}
fn join(c: &rusqlite::Connection, cid: &str, u: &str, role: &str, seq: i64) {
    c.execute("INSERT INTO circle_members (circle_id, user_id, role, joined_at, join_seq) VALUES (?1, ?2, ?3, 10, ?4)", params![cid, u, role, seq]).unwrap();
}
const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn world() -> (rusqlite::Connection, tempfile::TempDir, String, String) {
    let (c, d) = open();
    for u in ["a", "b", "c", "z"] {
        user(&c, u);
    }
    let cid = create_circle(&PoolCtx::new(&c, 1_000), "a", &m(json!({ "name": "Team" }))).unwrap().1["circle"]["id"].as_str().unwrap().to_string();
    join(&c, &cid, "b", "editor", 5);
    join(&c, &cid, "c", "viewer", 6);
    let bid = format!("{:032x}", 1);
    c.execute("INSERT INTO circle_banks (id, circle_id, author_id, title, license, head_version, head_version_id, created_at, updated_at) VALUES (?1, ?2, 'b', 'Quiz', 'circle-only', 1, 'v1', 1, 77)", params![bid, cid]).unwrap();
    c.execute("INSERT INTO circle_versions (id, bank_id, circle_id, version, commit_id, request_sha256, committer_id, license, status, full_key, index_key, bytes, question_count, created_at, committed_at) VALUES ('v1', ?1, ?2, 1, 'c1', 'x', 'b', 'circle-only', 'current', 'k', 'ik', 1, 1, 600, 600)", params![bid, cid]).unwrap();
    c.execute("INSERT INTO blobs (sha256, size, content_type, width, height, first_uploaded_at) VALUES (?1, 10, 'image/png', 1, 1, 1)", [SHA]).unwrap();
    c.execute("INSERT INTO circle_version_blobs (version_id, sha256) VALUES ('v1', ?1)", [SHA]).unwrap();
    (c, d, cid, bid)
}

#[test]
fn a_member_gets_the_type_of_an_image_the_bank_uses_and_everyone_else_a_counted_miss() {
    let (c, _d, cid, bid) = world();
    let x = PoolCtx::new(&c, 5_000);
    assert_eq!(bank_blob_type(&x, "c", &cid, &bid, SHA).unwrap(), "image/png");
    let other = "b".repeat(64);
    for (who, sha) in [("z", SHA), ("c", other.as_str()), ("c", "short")] {
        assert_eq!(err(bank_blob_type(&x, who, &cid, &bid, sha).unwrap_err()).1, "not_found");
    }
    let n: i64 = c.query_row("SELECT SUM(count) FROM usage_daily WHERE user_id = 'c' AND metric = 'blob_miss'", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 2);
    c.execute("UPDATE circles SET closed_at = 1, close_reason = 'owner_closed'", []).unwrap();
    assert_eq!(err(bank_blob_type(&x, "c", &cid, &bid, SHA).unwrap_err()).1, "not_found", "a closed circle serves nothing");
}

#[test]
fn images_follow_holds_withholding_removal_blocks_and_key_hiding() {
    let (c, _d, cid, bid) = world();
    let x = PoolCtx::new(&c, 5_000);
    let ok = |who: &str| bank_blob_type(&x, who, &cid, &bid, SHA).is_ok();
    c.execute("INSERT INTO circle_holds (bank_id, reason, source, at) VALUES (?1, 'reports', '', 1)", [&bid]).unwrap();
    assert_eq!((ok("c"), ok("b"), ok("a")), (false, true, false), "held by a report: its author only");
    c.execute("DELETE FROM circle_holds", []).unwrap();
    c.execute("UPDATE circle_versions SET withheld_at = 9", []).unwrap();
    assert!(!ok("b"), "a withheld version serves no image");
    c.execute("UPDATE circle_versions SET withheld_at = NULL", []).unwrap();
    c.execute("UPDATE circle_banks SET removed_at = 5, removed_reason = 'author_removed'", []).unwrap();
    assert!(!ok("b"));
    c.execute("UPDATE circle_banks SET removed_at = NULL, removed_reason = NULL", []).unwrap();
    c.execute("INSERT INTO privacy_blocks (kind, hash, item_id, at) VALUES ('blob', ?1, 'i', 1)", [SHA]).unwrap();
    assert!(!ok("b"), "confirmed student data is served to nobody");
    c.execute("DELETE FROM privacy_blocks", []).unwrap();
    // A viewer of a key-hiding bank gets only the images the stripped form keeps.
    c.execute("UPDATE circle_banks SET hide_key = 1", []).unwrap();
    assert_eq!((ok("c"), ok("b")), (false, true));
    c.execute("UPDATE circle_versions SET strip_opts = ?1", [json!({ "image_shas": [SHA] }).to_string()]).unwrap();
    assert!(ok("c"));
}

#[test]
fn headers_and_extensions() {
    let h = blob_headers("image/jpeg", SHA);
    assert!(h.contains(&("Content-Disposition", format!("attachment; filename=\"{SHA}.jpg\""))));
    assert!(h.contains(&("Content-Security-Policy", "sandbox; default-src 'none'".to_string())));
    assert_eq!((image_ext("image/gif"), image_ext("image/webp"), image_ext("x")), ("gif", "webp", "bin"));
}

#[test]
fn only_the_owner_or_the_author_reach_the_patch() {
    let (c, _d, cid, bid) = world();
    let x = PoolCtx::new(&c, 5_000);
    assert!(patch_gate(&x, "b", &cid, &bid).is_ok());
    assert!(patch_gate(&x, "a", &cid, &bid).is_ok());
    let (s, code, body) = err(patch_gate(&x, "c", &cid, &bid).unwrap_err());
    assert_eq!((s, code.as_str(), body["needed"].clone()), (403, "circle_role_required", json!("owner_or_author")));
}

#[test]
fn the_author_changes_the_licence_once_and_it_is_audited() {
    let (c, _d, cid, bid) = world();
    let x = PoolCtx::new(&c, 5_000);
    let go = |who: &str, v: Value| {
        let (mm, b) = patch_gate(&x, who, &cid, &bid)?;
        patch_license(&x, who, &cid, &mm, &b, &m(v))
    };
    assert_eq!(go("b", json!({ "license": "CC-BY-4.0" })).unwrap(), json!({ "ok": true, "license": "CC-BY-4.0" }));
    let l: String = c.query_row("SELECT license FROM circle_banks", [], |r| r.get(0)).unwrap();
    assert_eq!(l, "CC-BY-4.0");
    let n: i64 = c.query_row("SELECT COUNT(*) FROM pool_audit WHERE action = 'circle.bank_license' AND item_id = ?1", [&cid], |r| r.get(0)).unwrap();
    assert_eq!(n, 1);
    // The owner is not the author; a bad or missing licence and stray fields are 400s.
    assert_eq!(err(go("a", json!({ "license": "CC0-1.0" })).unwrap_err()).2["needed"], json!("author"));
    for body in [json!({}), json!({ "license": "MIT" }), json!({ "license": 3 }), json!({ "license": "CC0-1.0", "x": 1 })] {
        assert_eq!(err(go("b", body).unwrap_err()).1, "invalid_request");
    }
    assert_eq!(err(go("b", json!({ "base_version": 1 })).unwrap_err()).2["field"], json!("base_version"));
    // A demoted author, a removed bank and a closed circle are refused.
    c.execute("UPDATE circle_banks SET removed_at = 5, removed_reason = 'author_removed'", []).unwrap();
    assert_eq!(err(go("b", json!({ "license": "CC0-1.0" })).unwrap_err()).1, "bank_removed");
}

#[test]
fn public_origins_limit_the_licences_the_bank_may_take() {
    let (c, _d, cid, bid) = world();
    let x = PoolCtx::new(&c, 5_000);
    c.execute("UPDATE circle_versions SET origins = ?1", [json!([{ "pos": 0, "item_id": "i", "license": "CC-BY-NC-4.0" }]).to_string()]).unwrap();
    let (mm, b) = patch_gate(&x, "b", &cid, &bid).unwrap();
    let (s, code, body) = err(patch_license(&x, "b", &cid, &mm, &b, &m(json!({ "license": "CC0-1.0" }))).unwrap_err());
    assert_eq!((s, code.as_str()), (422, "license_incompatible"));
    assert!(!body["allowed"].as_array().unwrap().contains(&json!("CC0-1.0")));
    assert!(patch_license(&x, "b", &cid, &mm, &b, &m(json!({ "license": "CC-BY-NC-4.0" }))).is_ok());
}
