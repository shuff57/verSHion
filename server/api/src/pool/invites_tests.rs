//! Tests replaying worker/test/pool/round28.test.ts (the invite-code gate) on the real migrated schema.
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
fn user(c: &Connection, id: &str, trust: &str) {
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?1, ?3, 1)", params![id, format!("{id}@x.org"), trust]).unwrap();
}
fn obj(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}
fn err(r: Res<impl std::fmt::Debug>) -> (u16, String) {
    match r {
        Err(Fail::Pool(e)) => (e.status, e.code),
        other => panic!("{other:?}"),
    }
}
fn issue(c: &Connection, now: i64, body: Value) -> Value {
    let (s, v) = issue_invite(&PoolCtx::new(c, now), "adm", &obj(body)).unwrap();
    assert_eq!(s, 201);
    v
}
fn redeem(c: &Connection, now: i64, u: &str, code: &str) -> Res<Value> {
    redeem_invite(&PoolCtx::new(c, now), u, &obj(json!({ "code": code })))
}
const DAY: i64 = DAY_MS;

#[test]
fn a_code_has_24_crockford_characters_in_six_groups_and_normalises_loosely() {
    let c = new_invite_code();
    assert_eq!(c.len(), 29);
    assert!(c.split('-').count() == 6 && c.split('-').all(|g| g.len() == 4 && g.bytes().all(|b| INVITE_ALPHABET.contains(&b))), "{c}");
    assert_ne!(c, new_invite_code());
    let norm = normalise_invite_code(&json!(c)).unwrap();
    assert_eq!(norm.len(), 24);
    // Typed loosely: lower case, spaces, O for 0 and I/L for 1 are read as the code.
    let loose = c.to_lowercase().replace('-', " ").replace('0', "o").replace('1', "l");
    assert_eq!(normalise_invite_code(&json!(loose)), Some(norm.clone()));
    for bad in [json!("not a code"), json!(5), json!(null), json!("U".repeat(24)), json!("0".repeat(23)), json!("0".repeat(101))] {
        assert_eq!(normalise_invite_code(&bad), None, "{bad}");
    }
    assert_eq!(invite_hash(&norm).len(), 64);
    // A fixed vector for the encoder: 15 zero bytes, and 15 0xff bytes.
    assert_eq!(encode_code(&[0; 15]), "0000-0000-0000-0000-0000-0000");
    assert_eq!(encode_code(&[0xff; 15]), "ZZZZ-ZZZZ-ZZZZ-ZZZZ-ZZZZ-ZZZZ");
}

#[test]
fn the_gate_refuses_until_a_code_is_redeemed_then_opens_and_the_code_is_stored_only_hashed() {
    let (c, _d) = open();
    user(&c, "adm", "admin");
    user(&c, "nina", "teacher");
    let x = PoolCtx::new(&c, 1_000);
    assert_eq!(err(require_invite(&x, "nina")), (403, "invite_required".into()));
    let Err(Fail::Pool(e)) = require_invite(&x, "nina") else { panic!() };
    assert!(e.body()["message"].as_str().unwrap().contains("invite"));
    assert_eq!(err(redeem_invite(&x, "nina", &obj(json!({ "code": "not a code" })))), (400, "invalid_request".into()));
    let inv = issue(&c, 1_000, json!({ "note": "pilot: Chico High science" }));
    assert_eq!((inv["max_uses"].clone(), inv["expires_at"].clone(), inv["shown_once"].clone()), (json!(1), Value::Null, json!(true)));
    let code = inv["code"].as_str().unwrap();
    let norm = normalise_invite_code(&json!(code)).unwrap();
    // Stored nowhere in the clear, in any table.
    for t in ["pool_invites", "pool_audit", "pool_invite_redemptions"] {
        let dump: String = c.query_row(&format!("SELECT COALESCE(group_concat(json_object('r', t.rowid) || quote(t.rowid)), '') FROM {t} t"), [], |r| r.get(0)).unwrap();
        assert!(!dump.contains(&norm));
    }
    let rows: Vec<String> = c.prepare("SELECT code_hash FROM pool_invites").unwrap().query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect();
    assert_eq!(rows, vec![invite_hash(&norm)]);
    let audited: String = c.query_row("SELECT group_concat(COALESCE(detail, '') || COALESCE(note, '')) FROM pool_audit", [], |r| r.get(0)).unwrap();
    assert!(!audited.contains(&norm) && !audited.contains(code));
    let loose = code.to_lowercase().replace('-', " ").replace('0', "o").replace('1', "l");
    assert_eq!(redeem(&c, 1_000, "nina", &loose).unwrap(), json!({ "ok": true, "redeemed": true }));
    require_invite(&x, "nina").unwrap();
    assert_eq!(redeem(&c, 1_000, "nina", code).unwrap(), json!({ "ok": true, "already": true }), "a second redemption uses nothing");
    let uses: i64 = c.query_row("SELECT uses FROM pool_invites", [], |r| r.get(0)).unwrap();
    assert_eq!(uses, 1);
    let log: Vec<(String, Option<String>, Option<String>)> =
        c.prepare("SELECT action, actor_id, target_user_id FROM pool_audit WHERE action LIKE 'invite.%' ORDER BY at, action").unwrap().query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().map(Result::unwrap).collect();
    assert_eq!(log, vec![("invite.issue".into(), Some("adm".into()), None), ("invite.redeem".into(), Some("nina".into()), Some("nina".into()))]);
    // The admin sees who redeemed it, and an account's standing.
    let id = inv["invite_id"].as_str().unwrap();
    let (_, one) = list_invites(&x, &InviteQuery { id: Some(id), ..Default::default() }).unwrap();
    assert_eq!((one["invite"]["state"].clone(), one["invite"]["uses"].clone(), one["invite"]["redeemed_by"][0]["user_id"].clone()), (json!("used_up"), json!(1), json!("nina")));
}

#[test]
fn single_use_a_use_cap_an_expiry_and_revocation_all_answer_the_same_invite_invalid() {
    let (c, _d) = open();
    for u in ["adm", "u0", "u1", "u2", "u3", "u4", "u5"] {
        user(&c, u, "teacher");
    }
    let t0 = 1_000_000;
    let once = issue(&c, t0, json!({}));
    redeem(&c, t0, "u0", once["code"].as_str().unwrap()).unwrap();
    assert_eq!(err(redeem(&c, t0, "u1", once["code"].as_str().unwrap())), (422, "invite_invalid".into()), "single use");
    let two = issue(&c, t0, json!({ "max_uses": 2, "expires_in_days": 3 }));
    assert_eq!(two["expires_at"], json!(t0 + 3 * DAY));
    redeem(&c, t0, "u1", two["code"].as_str().unwrap()).unwrap();
    redeem(&c, t0, "u2", two["code"].as_str().unwrap()).unwrap();
    assert_eq!(err(redeem(&c, t0, "u3", two["code"].as_str().unwrap())).1, "invite_invalid", "the cap");
    let exp = issue(&c, t0, json!({ "max_uses": 5, "expires_in_days": 1 }));
    assert_eq!(err(redeem(&c, t0 + DAY, "u3", exp["code"].as_str().unwrap())).1, "invite_invalid", "expired at the instant it ends");
    redeem(&c, t0 + DAY - 1, "u3", exp["code"].as_str().unwrap()).unwrap();
    let rev = issue(&c, t0, json!({ "max_uses": 5 }));
    redeem(&c, t0, "u4", rev["code"].as_str().unwrap()).unwrap();
    let x = PoolCtx::new(&c, t0);
    let id = rev["invite_id"].as_str().unwrap();
    assert_eq!(revoke_invite(&x, "adm", &obj(json!({ "invite_id": id, "note": "posted in a staff chat" }))).unwrap().0, 200);
    assert_eq!(err(redeem(&c, t0, "u5", rev["code"].as_str().unwrap())).1, "invite_invalid", "revoked");
    require_invite(&x, "u4").unwrap();
    assert_eq!(revoke_invite(&x, "adm", &obj(json!({ "invite_id": id }))).unwrap().0, 409);
    assert_eq!(revoke_invite(&x, "adm", &obj(json!({ "invite_id": "f".repeat(32) }))).unwrap().0, 404);
    let revokes: i64 = c.query_row("SELECT COUNT(*) FROM pool_audit WHERE action = 'invite.revoke'", [], |r| r.get(0)).unwrap();
    assert_eq!(revokes, 1);
    let unknown = "0".repeat(24);
    assert_eq!(err(redeem(&c, t0, "u5", &unknown)).1, "invite_invalid", "an unknown code: the same answer");
    // Validation of the admin's input.
    for body in [json!({ "max_uses": 0 }), json!({ "max_uses": 1001 }), json!({ "max_uses": 1.5 }), json!({ "expires_in_days": 0 }), json!({ "expires_in_days": 366 }), json!({ "max_uses": "3" })] {
        assert_eq!(err(issue_invite(&x, "adm", &obj(body.clone()))), (400, "invalid_request".into()), "{body}");
    }
    let (_, all) = list_invites(&PoolCtx::new(&c, t0 + DAY), &InviteQuery::default()).unwrap();
    let mut states: Vec<_> = all["invites"].as_array().unwrap().iter().map(|i| i["state"].as_str().unwrap().to_string()).collect();
    states.sort();
    assert_eq!(states, vec!["expired", "revoked", "used_up", "used_up"]);
}

#[test]
fn redemption_attempts_are_a_daily_allowance_of_ten() {
    let (c, _d) = open();
    user(&c, "adm", "admin");
    user(&c, "gus", "teacher");
    let t0 = 5 * DAY;
    for i in 0..10 {
        assert_eq!(err(redeem(&c, t0, "gus", &format!("{}{i:04}", "0123456789ABCDEFGHJK"))).1, "invite_invalid");
    }
    let inv = issue(&c, t0, json!({}));
    assert_eq!(err(redeem(&c, t0, "gus", inv["code"].as_str().unwrap())).1, "daily_limit", "the 11th try of the day");
    assert_eq!(redeem(&c, t0 + DAY, "gus", inv["code"].as_str().unwrap()).unwrap()["redeemed"], json!(true), "the next day");
}

#[test]
fn an_erased_account_redeems_nothing_and_a_racing_second_use_of_the_last_slot_loses() {
    let (c, _d) = open();
    for u in ["a", "b", "g"] {
        user(&c, u, "teacher");
    }
    c.execute("UPDATE users SET deleted_at = 5, email = NULL, pw_hash = NULL, pw_params = NULL, display_name = NULL, profile = NULL WHERE id = 'g'", []).unwrap();
    let last = issue(&c, 1_000, json!({}));
    let code = last["code"].as_str().unwrap();
    assert_eq!(err(redeem(&c, 1_000, "g", code)).0, 401, "the erased get the unauthenticated answer");
    let uses: i64 = c.query_row("SELECT uses FROM pool_invites", [], |r| r.get(0)).unwrap();
    assert_eq!(uses, 0, "nothing was taken");
    redeem(&c, 1_000, "a", code).unwrap();
    assert_eq!(err(redeem(&c, 1_000, "b", code)).1, "invite_invalid");
}

#[test]
fn the_listing_pages_newest_first_and_answers_one_account_or_one_invite() {
    let (c, _d) = open();
    user(&c, "adm", "admin");
    user(&c, "old", "teacher");
    c.execute("INSERT INTO pool_invite_redemptions (user_id, invite_id, at) VALUES ('old', NULL, 1)", []).unwrap();
    let mut ids = Vec::new();
    for i in 0..(INVITE_PAGE as i64 + 2) {
        ids.push(issue(&c, 1_000 + i, json!({}))["invite_id"].as_str().unwrap().to_string());
    }
    let x = PoolCtx::new(&c, 5_000);
    let (_, p1) = list_invites(&x, &InviteQuery::default()).unwrap();
    assert_eq!(p1["invites"].as_array().unwrap().len(), INVITE_PAGE);
    assert_eq!(p1["invites"][0]["id"], json!(ids.last().unwrap()));
    let after = p1["next_cursor"].as_str().unwrap().to_string();
    let (_, p2) = list_invites(&x, &InviteQuery { after: Some(&after), ..Default::default() }).unwrap();
    assert_eq!(p2["invites"].as_array().unwrap().len(), 2);
    assert_eq!(p2["next_cursor"], Value::Null);
    let (_, g) = list_invites(&x, &InviteQuery { user_id: Some("11111111-1111-1111-1111-111111111111"), ..Default::default() }).unwrap();
    assert_eq!(g["redemption"], Value::Null);
    c.execute("UPDATE users SET id = '11111111-1111-1111-1111-111111111111' WHERE id = 'old'", []).ok();
    c.execute("UPDATE pool_invite_redemptions SET user_id = '11111111-1111-1111-1111-111111111111'", []).unwrap();
    let (_, g) = list_invites(&x, &InviteQuery { user_id: Some("11111111-1111-1111-1111-111111111111"), ..Default::default() }).unwrap();
    assert_eq!(g["redemption"]["grandfathered"], json!(true));
    assert_eq!(err(list_invites(&x, &InviteQuery { user_id: Some("nope"), ..Default::default() })).1, "invalid_request");
    assert_eq!(err(list_invites(&x, &InviteQuery { id: Some("nope"), ..Default::default() })).1, "invalid_request");
    assert_eq!(list_invites(&x, &InviteQuery { id: Some(&"f".repeat(32)), ..Default::default() }).unwrap().0, 404);
}
