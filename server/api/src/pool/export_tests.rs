//! Tests replaying worker/test/pool/export.test.ts on the real migrated schema, with fixtures written directly (publishing is not
//! ported yet).
use super::*;
use crate::db;
use crate::pool::ctx::Fail;
use rusqlite::Connection;
use std::path::Path;

const DAY: i64 = 86_400_000;
const NOW: i64 = 100 * DAY;

fn open() -> (Connection, tempfile::TempDir, BlobStore) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    let store = BlobStore::new(dir.path().join("blobs")).unwrap();
    (conn, dir, store)
}
fn user(c: &Connection, id: &str, trust: &str, name: &str) {
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?3, ?4, 1)", params![id, format!("{id}@x.org"), name, trust]).unwrap();
}
fn login(c: &Connection, u: &str, auth_at: i64) {
    c.execute(
        "INSERT INTO sessions (id, family_id, user_id, refresh_hash, created_at, auth_at, expires_at, user_agent) VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6, 'TestBrowser/1')",
        params![format!("s-{u}-{auth_at}"), format!("fam-{u}"), u, format!("h-{u}-{auth_at}"), auth_at, auth_at + 30 * DAY],
    )
    .unwrap();
}
fn item(c: &Connection, store: &BlobStore, id: &str, author: &str, n: i64, body: &str, removed: Option<&str>) {
    let pc = format!("pc-{id}");
    c.execute(
        "INSERT INTO items (id, author_id, kind, visibility, current_publish_id, public_publish_id, public_version, version, title, subject, course, grade, kinds_mask, question_count,
            license, has_images, committed_at, created_at, updated_at, fold_rev, removed_at, removed_reason)
         VALUES (?1, ?2, 'bank', 'public', ?3, ?3, 1, 1, ?4, 'Geo', '', '', 1, 1, 'CC-BY-4.0', 0, 100, ?5, ?5, 1, ?6, ?7)",
        params![id, author, pc, format!("Title {id}"), n, removed.map(|_| 50), removed],
    )
    .unwrap();
    let key = format!("items/{id}/full");
    c.execute(
        "INSERT INTO item_publishes (publish_id, item_id, author_id, version, public_version, status, schema_version, hide_key, license, attested_at, attest_text_rev,
            acks_json, request_sha256, stage_token, full_key, stripped_key, full_bytes, title, subject, course, grade, kinds_mask, question_count, has_images, created_at)
         VALUES (?1, ?2, ?3, 1, 1, 'current', 2, 1, 'CC-BY-4.0', 1, 'r', '{}', 'x', ?1, ?4, 'k2', ?5, '', '', '', '', 0, 1, 1, 1)",
        params![pc, id, author, key, body.len() as i64],
    )
    .unwrap();
    store.put(&key, body.as_bytes()).unwrap();
}
fn who<'a>(u: &'a str, trust: &'a str) -> ExportWho<'a> {
    ExportWho { user_id: u, trust, session_family: Some(Box::leak(format!("fam-{u}").into_boxed_str())) }
}
fn circles(_: &PoolCtx, _: &str) -> Res<Value> {
    Ok(json!({ "memberships": [] }))
}
fn run(c: &Connection, s: &BlobStore, u: &str, trust: &str, part: Option<&str>, lim: ExportLimits) -> Res<ExportOut> {
    export_account(&PoolCtx::new(c, NOW), s, &who(u, trust), part, lim, &circles)
}
fn code(r: Res<ExportOut>) -> (u16, String) {
    match r {
        Err(Fail::Pool(e)) => (e.status, e.code),
        Ok(_) => panic!("expected a refusal"),
        Err(other) => panic!("{other:?}"),
    }
}
const HID: &str = r#"{"hidden":"Ann's hidden worked explanation"}"#;

fn world() -> (Connection, tempfile::TempDir, BlobStore) {
    let (c, d, s) = open();
    user(&c, "ann", "teacher", "Ann Exporter");
    user(&c, "bob", "teacher", "Bob Other");
    user(&c, "eve", "teacher", "Eve Reporter");
    let (a1, a2, b1) = ("a".repeat(32), "b".repeat(32), "c".repeat(32));
    item(&c, &s, &a1, "ann", 1, HID, Some("takedown"));
    item(&c, &s, &a2, "ann", 2, r#"{"q":"second"}"#, None);
    item(&c, &s, &b1, "bob", 3, r#"{"secret":"BOBSECRETANSWER"}"#, None);
    // a retained previous version of a1, with its own body
    c.execute(
        "INSERT INTO item_publishes (publish_id, item_id, author_id, version, public_version, status, schema_version, hide_key, license, attested_at, attest_text_rev,
            acks_json, request_sha256, stage_token, full_key, stripped_key, full_bytes, title, subject, course, grade, kinds_mask, question_count, has_images, created_at)
         VALUES ('prev-a1', ?1, 'ann', 0, 0, 'previous', 2, 1, 'CC-BY-4.0', 1, 'r', '{}', 'x', 'prev-a1', 'items/a1/prev', 'k3', 5, '', '', '', '', 0, 1, 1, 1)",
        [&a1],
    )
    .unwrap();
    s.put("items/a1/prev", br#"{"old":1}"#).unwrap();
    c.execute("INSERT INTO imports (user_id, item_id, publish_id, version, form, license, author_id, at, kind, real_publish_id, licenses_json, first_at) VALUES ('ann', ?1, 'pc-x', 1, 'stripped', 'CC-BY-4.0', 'bob', 7, 'import', 'pc-x', '[]', 7)", [&b1]).unwrap();
    c.execute("INSERT INTO reports (id, item_id, reporter_id, reason, note, at) VALUES ('r1', ?1, 'ann', 'wrong_answer', 'Ann''s own report note', 8)", [&b1]).unwrap();
    c.execute("INSERT INTO reports (id, item_id, reporter_id, reason, note, at) VALUES ('r2', ?1, 'eve', 'other', 'Eve''s note about Ann', 9)", [&a1]).unwrap();
    c.execute("INSERT INTO attestations (publish_id, item_id, user_id, at, terms_version) VALUES ('pc-x', ?1, 'ann', 2, 'v1')", [&a1]).unwrap();
    c.execute("INSERT INTO dmca_actions (id, notice_id, item_id, author_id, disabled_at) VALUES ('act', 'n', ?1, 'ann', 40)", [&a1]).unwrap();
    c.execute(
        "INSERT INTO pool_outbox (id, kind, payload, at) VALUES ('o1', 'takedown_author', ?1, 41), ('o2', 'cap', ?2, 42)",
        params![json!({ "kind": "takedown_author", "itemId": a1, "authorId": "ann", "actionId": "act" }).to_string(), json!({ "kind": "cap", "userId": "ann", "metric": "x" }).to_string()],
    )
    .unwrap();
    c.execute("INSERT INTO admin_flags (id, kind, item_id, user_id, detail, dedup, at) VALUES ('f1', 'appeal', ?1, 'ann', ?2, 'd1', 43)", params![a2, json!({ "note": "Ann's appeal note" }).to_string()]).unwrap();
    c.execute("INSERT INTO admin_flags (id, kind, item_id, user_id, detail, dedup, at) VALUES ('f2', 'cap', NULL, 'ann', '{\"metric\":\"ADMINNOTE\"}', 'd2', 44)", []).unwrap();
    login(&c, "ann", NOW);
    (c, d, s)
}

#[test]
fn every_section_of_the_accounts_own_data_the_full_form_and_an_audit_row_without_content() {
    let (c, _d, s) = world();
    let out = run(&c, &s, "ann", "teacher", None, EXPORT_LIMITS).unwrap();
    assert_eq!(out.filename, "vershion-export.json");
    let x = &out.body;
    assert_eq!((x["format"].clone(), x["profile"]["id"].clone(), x["profile"]["display_name"].clone(), x["profile"]["approval"].clone()), (json!(EXPORT_FORMAT), json!("ann"), json!("Ann Exporter"), json!("teacher")));
    let ids: Vec<_> = x["items"].as_array().unwrap().iter().map(|i| i["id"].as_str().unwrap().to_string()).collect();
    assert_eq!(ids, vec!["a".repeat(32), "b".repeat(32)]);
    let i1 = &x["items"][0];
    assert_eq!(i1["state"], "takedown");
    assert_eq!(x["items"][1]["state"], "live");
    let versions: Vec<_> = i1["versions"].as_array().unwrap().iter().map(|v| (v["publish_id"].as_str().unwrap().to_string(), v["version"].as_i64().unwrap())).collect();
    assert_eq!(versions, vec![("prev-a1".to_string(), 0), (format!("pc-{}", "a".repeat(32)), 1)]);
    assert!(i1["versions"][0].get("full_key").is_none() && i1["versions"][0].get("hide_key").is_none());
    assert_eq!(x["bodies"]["a".repeat(32).as_str()], serde_json::from_str::<Value>(HID).unwrap(), "her hidden explanation: she is the author");
    assert_eq!(x["version_bodies"]["prev-a1"], json!({ "old": 1 }), "the retained previous version has its own body");
    assert!(x["version_bodies"].get(format!("pc-{}", "a".repeat(32)).as_str()).is_none(), "the current body is not repeated");
    let imp = x["imports"].as_array().unwrap();
    assert_eq!((imp.len(), imp[0]["item_id"].clone(), imp[0]["title"].clone()), (1, json!("c".repeat(32)), json!(format!("Title {}", "c".repeat(32)))));
    assert_eq!(x["reports"].as_array().unwrap().iter().map(|r| r["note"].clone()).collect::<Vec<_>>(), vec![json!("Ann's own report note")]);
    assert_eq!(x["takedowns"][0]["action_id"], "act");
    assert_eq!(x["takedowns"][0]["countered"], 0);
    assert_eq!(x["notices"], json!([{ "kind": "takedown_author", "at": 41, "item_id": "a".repeat(32), "action_id": "act" }]));
    assert_eq!(x["attestations"][0]["publish_id"], "pc-x");
    assert_eq!(x["appeals"], json!([{ "item_id": "b".repeat(32), "at": 43, "note": "Ann's appeal note", "resolved_at": null }]));
    assert_eq!(x["sessions"].as_array().unwrap().len(), 1);
    assert_eq!(x["sessions"][0]["user_agent"], "TestBrowser/1");
    let sess = x["sessions"][0].as_object().unwrap();
    assert!(!sess.contains_key("refresh_hash") && !sess.contains_key("id") && !sess.contains_key("family_id"), "no tokens or token hashes");
    assert_eq!(x["circles"], json!({ "memberships": [] }));
    let (detail, note): (String, Option<String>) = c.query_row("SELECT detail, note FROM pool_audit WHERE action = 'account.export' AND actor_id = 'ann'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert!(!detail.contains("hidden") && note.is_none(), "no content in the audit");
    assert_eq!(serde_json::from_str::<Value>(&detail).unwrap(), json!({ "items": 2, "parts": 1 }));
}

#[test]
fn nothing_of_anyone_else_no_admin_notes_flags_or_internal_columns() {
    let (c, _d, s) = world();
    let text = run(&c, &s, "ann", "teacher", None, EXPORT_LIMITS).unwrap().body.to_string();
    for leak in ["bob", "eve", "Eve's note about Ann", "Eve Reporter", "ADMINNOTE", "BOBSECRETANSWER", "x.org"] {
        // (Ann's own address is `ann@x.org`, her profile section; nobody else's address appears.)
        let n = text.matches(leak).count();
        let allowed = if leak == "x.org" { 1 } else { 0 };
        assert!(n == allowed || (leak == "bob" && text.contains("Title")), "leaks {leak}: {n}");
    }
    for key in ["\"refresh_hash\"", "\"pw_hash\"", "\"stage_token\"", "\"admin_flags\"", "\"dedup\""] {
        assert!(!text.contains(key), "{key}");
    }
}

#[test]
fn three_a_day_a_pending_account_gets_its_profile_only_and_a_stale_session_must_sign_in_again() {
    let (c, _d, s) = world();
    for _ in 0..3 {
        run(&c, &s, "ann", "teacher", None, EXPORT_LIMITS).unwrap();
    }
    assert_eq!(code(run(&c, &s, "ann", "teacher", None, EXPORT_LIMITS)), (429, "daily_limit".into()));
    user(&c, "pen", "pending", "Pen Ding");
    login(&c, "pen", NOW);
    let rp = run(&c, &s, "pen", "pending", None, EXPORT_LIMITS).unwrap();
    assert_eq!((rp.body["profile_only"].clone(), rp.body.get("items").cloned(), rp.body["profile"]["approval"].clone()), (json!(true), None, json!("pending")));
    assert_eq!(code(run(&c, &s, "pen", "pending", Some("1"), EXPORT_LIMITS)).0, 404);
    user(&c, "unv", "unverified", "Un Verified");
    login(&c, "unv", NOW);
    assert_eq!(code(run(&c, &s, "unv", "unverified", None, EXPORT_LIMITS)), (403, "account_not_approved".into()));
    user(&c, "stale", "teacher", "Stale");
    login(&c, "stale", NOW - 25 * 3_600_000);
    assert_eq!(code(run(&c, &s, "stale", "teacher", None, EXPORT_LIMITS)), (403, "reauth_required".into()));
    user(&c, "nosess", "teacher", "No Session");
    assert_eq!(code(run(&c, &s, "nosess", "teacher", None, EXPORT_LIMITS)).1, "reauth_required");
    c.execute("UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, pw_params = NULL, profile = NULL, deleted_at = 5 WHERE id = 'ann'", []).unwrap();
    assert_eq!(code(run(&c, &s, "ann", "teacher", None, EXPORT_LIMITS)).0, 404, "an erased account has no export");
    assert_eq!(code(run(&c, &s, "nobody", "teacher", None, EXPORT_LIMITS)).0, 404);
}

#[test]
fn a_malformed_part_number_is_a_400_before_anything_is_charged() {
    let (c, _d, s) = world();
    for p in ["0", "01", "abc", "123456", "-1", "1.5", ""] {
        assert_eq!(code(run(&c, &s, "ann", "teacher", Some(p), EXPORT_LIMITS)), (400, "invalid_request".into()), "{p:?}");
    }
    let used: Option<i64> = c.query_row("SELECT count FROM usage_daily WHERE user_id = 'ann' AND metric = 'exports'", [], |r| r.get(0)).optional().unwrap();
    assert_eq!(used, None);
}

#[test]
fn past_the_size_cap_the_export_is_a_manifest_of_parts_fetched_by_number_and_parts_do_not_count_again() {
    let (c, _d, s) = open();
    user(&c, "ann", "teacher", "Ann");
    let ids: Vec<String> = (0..4).map(|k| format!("{k:032x}")).collect();
    for (k, id) in ids.iter().enumerate() {
        item(&c, &s, id, "ann", k as i64, &format!("{{\"q\":\"{}\"}}", "delta ".repeat(40)), None);
    }
    login(&c, "ann", NOW);
    let lim = ExportLimits { whole: 600, part: 400 };
    let r = run(&c, &s, "ann", "teacher", None, lim).unwrap().body;
    assert_eq!(r["export_too_large"], json!(true));
    assert!(r.get("bodies").is_none());
    let parts = r["manifest"]["parts"].as_array().unwrap();
    assert!(parts.len() >= 2);
    let mut all: Vec<String> = parts.iter().flat_map(|p| p["items"].as_array().unwrap().iter().map(|i| i.as_str().unwrap().to_string())).collect();
    all.sort();
    assert_eq!(all, ids);
    assert_eq!(parts[0]["url"], "/me/export?part=1");
    let mut got = Vec::new();
    for p in parts {
        let n = p["part"].as_i64().unwrap().to_string();
        let out = run(&c, &s, "ann", "teacher", Some(&n), lim).unwrap();
        assert_eq!(out.filename, format!("vershion-export-part-{n}.json"));
        let mut keys: Vec<String> = out.body["bodies"].as_object().unwrap().keys().cloned().collect();
        keys.sort();
        let mut want: Vec<String> = p["items"].as_array().unwrap().iter().map(|i| i.as_str().unwrap().to_string()).collect();
        want.sort();
        assert_eq!(keys, want);
        assert_eq!(out.body["of"], json!(parts.len()));
        got.extend(keys);
    }
    got.sort();
    assert_eq!(got, ids);
    assert_eq!(code(run(&c, &s, "ann", "teacher", Some(&(parts.len() + 1).to_string()), lim)).0, 404);
    let used: i64 = c.query_row("SELECT count FROM usage_daily WHERE user_id = 'ann' AND metric = 'exports'", [], |r| r.get(0)).unwrap();
    assert_eq!(used, 1, "parts do not count again");
}
