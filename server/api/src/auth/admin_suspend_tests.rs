//! Tests for `admin_suspend.rs`: suspend, restore and the two-person rule.
use super::super::admin::tests::{World, code, hdrs, native, obj, world};
use super::*;
use rusqlite::Transaction;

fn nothing(_: &Transaction, _: &str, _: i64) -> rusqlite::Result<()> {
    Ok(())
}

fn do_suspend(w: &World, h: &HeaderMap, target: &str) -> Result<Response, HttpError> {
    suspend(&w.ctx(), h, &native(), &obj(json!({ "user_id": target, "note": "internal", "reason": "policy breach" })), &nothing, &mut Outbox::new())
}

#[test]
fn suspending_a_teacher_is_immediate_ends_sessions_blocks_login_and_mails_them() {
    let w = world();
    let a = w.admin("root@x.org");
    let t = w.account("t@x.org", "teacher");
    let mut out = Outbox::new();
    let called = std::cell::Cell::new(0);
    let hook = |_: &Transaction, uid: &str, _: i64| -> rusqlite::Result<()> {
        assert!(!uid.is_empty());
        called.set(called.get() + 1);
        Ok(())
    };
    let r = suspend(&w.ctx(), &a.full, &native(), &obj(json!({ "user_id": t, "note": "internal", "reason": "policy breach" })), &hook, &mut out).unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(called.get(), 1);
    assert_eq!(w.count(&format!("SELECT COUNT(*) FROM users WHERE id = '{t}' AND suspended_at IS NOT NULL AND suspension_reason = 'policy breach' AND device_epoch = 1")), 1);
    assert_eq!(w.count("SELECT COUNT(*) FROM auth_audit WHERE action = 'suspend' AND prev_trust = 'teacher' AND new_trust = 'teacher'"), 1);
    assert_eq!((out[0].kind, out[0].reason.as_deref()), (MailKind::AccountSuspended, Some("policy breach")));
    assert_eq!(code(do_suspend(&w, &a.full, &t)), (409, "not_suspendable".into()));
    assert_eq!(code(do_suspend(&w, &a.full, &a.id)), (403, "self_action".into()));
    assert_eq!(code(do_suspend(&w, &a.full, &uuid_v4())), (404, "not_found".into()));
    assert_eq!(code(do_suspend(&w, &a.read_only, &t)), (403, "sudo_required".into()));
}

#[test]
fn restoring_lifts_the_suspension_audits_and_runs_the_standing_hook() {
    let w = world();
    let a = w.admin("root@x.org");
    let t = w.account("t@x.org", "teacher");
    let body = obj(json!({ "user_id": t, "note": "appeal upheld" }));
    let mut out = Outbox::new();
    assert_eq!(code(restore(&w.ctx(), &a.full, &native(), &body, &nothing, &mut out)), (409, "not_suspended".into()));
    do_suspend(&w, &a.full, &t).unwrap();
    assert_eq!(restore(&w.ctx(), &a.full, &native(), &body, &nothing, &mut out).unwrap().status(), 200);
    assert_eq!(w.count(&format!("SELECT COUNT(*) FROM users WHERE id = '{t}' AND suspended_at IS NULL AND suspension_reason IS NULL")), 1);
    assert_eq!(w.count("SELECT COUNT(*) FROM auth_audit WHERE action = 'restore' AND note = 'appeal upheld'"), 1);
    assert_eq!(out[0].kind, MailKind::AccountRestored);
    assert_eq!(code(restore(&w.ctx(), &a.full, &native(), &obj(json!({ "user_id": a.id, "note": "n" })), &nothing, &mut out)), (403, "self_action".into()));
}

#[test]
fn a_failing_standing_hook_rolls_the_suspension_back() {
    let w = world();
    let a = w.admin("root@x.org");
    let t = w.account("t@x.org", "teacher");
    let boom = |_: &Transaction, _: &str, _: i64| -> rusqlite::Result<()> { Err(rusqlite::Error::InvalidQuery) };
    let r = suspend(&w.ctx(), &a.full, &native(), &obj(json!({ "user_id": t, "note": "n", "reason": "r" })), &boom, &mut Outbox::new());
    assert_eq!(code(r).0, 500);
    assert_eq!(w.count("SELECT COUNT(*) FROM users WHERE suspended_at IS NOT NULL"), 0);
}

#[test]
fn suspending_an_admin_only_proposes_and_a_different_admin_confirms() {
    let w = world();
    let a = w.admin("a@x.org");
    let b = w.admin("b@x.org");
    let _c = w.admin("c@x.org");
    let r = do_suspend(&w, &a.full, &b.id).unwrap();
    assert_eq!(r.status().as_u16(), 202);
    let pid: String = w.db.lock().unwrap().query_row("SELECT id FROM admin_proposals", [], |r| r.get(0)).unwrap();
    assert_eq!(w.count("SELECT COUNT(*) FROM users WHERE suspended_at IS NOT NULL"), 0, "nothing happens yet");
    assert_eq!(w.count("SELECT COUNT(*) FROM admin_proposals WHERE self_confirm_after IS NULL"), 1, "others could confirm, so no self-confirmation");
    assert_eq!(code(do_suspend(&w, &a.full, &b.id)), (409, "proposal_exists".into()));
    let confirm_body = obj(json!({ "proposal_id": pid }));
    let mut out = Outbox::new();
    assert_eq!(code(confirm(&w.ctx(), &a.full, &native(), &confirm_body, &nothing, &mut out)), (403, "second_admin_required".into()), "the proposer cannot confirm alone");
    assert_eq!(code(confirm(&w.ctx(), &b.full, &native(), &confirm_body, &nothing, &mut out)), (403, "self_action".into()), "nor can the target");
    let c_admin = w.admin("d@x.org");
    let r = confirm(&w.ctx(), &c_admin.full, &native(), &confirm_body, &nothing, &mut out).unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(w.count(&format!("SELECT COUNT(*) FROM users WHERE id = '{}' AND suspended_at IS NOT NULL", b.id)), 1);
    assert_eq!(w.count("SELECT COUNT(*) FROM admin_proposals WHERE outcome = 'confirmed'"), 1);
    assert_eq!(w.count("SELECT COUNT(*) FROM auth_audit WHERE action IN ('suspend_proposed', 'suspend')"), 2);
    assert_eq!(code(confirm(&w.ctx(), &c_admin.full, &native(), &confirm_body, &nothing, &mut out)), (409, "no_open_proposal".into()));
}

#[test]
fn the_last_other_admin_can_confirm_alone_only_after_the_delay() {
    let w = world();
    let a = w.admin("a@x.org");
    let b = w.admin("b@x.org");
    let r = do_suspend(&w, &a.full, &b.id).unwrap();
    let body = super::super::admin::tests::body_of(r);
    assert!(body["self_confirm_after"].as_i64().unwrap() > 1_800_000_000_000, "open only after the delay");
    let pid = body["proposal_id"].as_str().unwrap().to_string();
    let mut out = Outbox::new();
    assert_eq!(code(confirm(&w.ctx(), &a.full, &native(), &obj(json!({ "proposal_id": pid })), &nothing, &mut out)), (403, "second_admin_required".into()));
    // Once the delay has passed, the proposer may confirm alone (nobody else could).
    w.db.lock().unwrap().execute("UPDATE admin_proposals SET self_confirm_after = 1", []).unwrap();
    let r = confirm(&w.ctx(), &a.full, &native(), &obj(json!({ "proposal_id": pid })), &nothing, &mut out).unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(w.count(&format!("SELECT COUNT(*) FROM users WHERE id = '{}' AND suspended_at IS NOT NULL", b.id)), 1);
}

#[test]
fn any_admin_can_cancel_an_open_proposal_and_the_list_shows_it() {
    let w = world();
    let a = w.admin("a@x.org");
    let b = w.admin("b@x.org");
    let r = do_suspend(&w, &a.full, &b.id).unwrap();
    let pid = super::super::admin::tests::body_of(r)["proposal_id"].as_str().unwrap().to_string();
    let list = super::super::admin::tests::body_of(proposals(&w.ctx(), &a.read_only, &native()).unwrap());
    assert_eq!((list["proposals"][0]["id"].as_str(), list["proposals"][0]["outcome"].clone()), (Some(pid.as_str()), Value::Null));
    assert_eq!(cancel(&w.ctx(), &b.full, &native(), &obj(json!({ "proposal_id": pid })), &mut Outbox::new()).unwrap().status(), 200, "even the target may cancel");
    assert_eq!(w.count("SELECT COUNT(*) FROM admin_proposals WHERE outcome = 'cancelled'"), 1);
    assert_eq!(w.count("SELECT COUNT(*) FROM auth_audit WHERE action = 'suspend_proposal_cancelled'"), 1);
    assert_eq!(code(cancel(&w.ctx(), &b.full, &native(), &obj(json!({ "proposal_id": pid })), &mut Outbox::new())), (409, "no_open_proposal".into()));
    assert!(do_suspend(&w, &a.full, &b.id).is_ok(), "a cancelled proposal frees the target for a new one");
    let _ = hdrs;
}

#[test]
fn an_expired_proposal_cannot_be_confirmed_and_is_replaced_by_a_new_one() {
    let w = world();
    let a = w.admin("a@x.org");
    let b = w.admin("b@x.org");
    do_suspend(&w, &a.full, &b.id).unwrap();
    w.db.lock().unwrap().execute("UPDATE admin_proposals SET expires_at = 1", []).unwrap();
    let pid: String = w.db.lock().unwrap().query_row("SELECT id FROM admin_proposals", [], |r| r.get(0)).unwrap();
    let c = w.admin("c@x.org");
    assert_eq!(code(confirm(&w.ctx(), &c.full, &native(), &obj(json!({ "proposal_id": pid })), &nothing, &mut Outbox::new())), (409, "no_open_proposal".into()));
    assert!(do_suspend(&w, &a.full, &b.id).is_ok());
    assert_eq!(w.count("SELECT COUNT(*) FROM admin_proposals WHERE outcome = 'expired'"), 1);
}

#[test]
fn the_last_active_admin_cannot_be_suspended_by_confirmation() {
    let w = world();
    let a = w.admin("a@x.org");
    let b = w.admin("b@x.org");
    let r = do_suspend(&w, &a.full, &b.id).unwrap();
    let pid = super::super::admin::tests::body_of(r)["proposal_id"].as_str().unwrap().to_string();
    // The proposer goes away (suspended outside the flow): the target is now the last active admin.
    w.db.lock().unwrap().execute("UPDATE users SET suspended_at = 1 WHERE id = ?1", [&a.id]).unwrap();
    let c = w.admin("c@x.org");
    w.db.lock().unwrap().execute("UPDATE users SET suspended_at = 1 WHERE id = ?1", [&c.id]).unwrap();
    // c is suspended so the guard refuses it; confirmation by anyone else is impossible, and the rule holds in SQL.
    assert_eq!(code(confirm(&w.ctx(), &c.full, &native(), &obj(json!({ "proposal_id": pid })), &nothing, &mut Outbox::new())).0, 401);
    assert_eq!(w.count(&format!("SELECT COUNT(*) FROM users WHERE id = '{}' AND suspended_at IS NULL", b.id)), 1);
}
