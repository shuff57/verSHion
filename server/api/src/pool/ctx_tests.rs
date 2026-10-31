//! Tests for `ctx.rs`: limits and daily allowances, the miss guard, cap alerts and their hourly budget, the audit scrub,
//! and the notice JSON the outbox keeps.
use super::*;
use crate::db;
use std::path::Path;

fn open() -> (Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    (conn, dir)
}
fn user(c: &Connection, id: &str, deleted: bool) {
    if deleted {
        c.execute("INSERT INTO users (id, trust, created_at, deleted_at) VALUES (?1, 'teacher', 1, 5)", [id]).unwrap();
    } else {
        c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?3, 'teacher', 1)", params![id, format!("{id}@school.org"), format!("Name {id}")]).unwrap();
    }
}
fn pool_err(f: Fail) -> PoolError {
    match f {
        Fail::Pool(p) => p,
        Fail::Db(e) => panic!("database error: {e}"),
    }
}

#[test]
fn a_limit_is_the_users_row_then_the_site_row_then_the_default() {
    let (c, _d) = open();
    let x = PoolCtx::new(&c, 1_000);
    assert_eq!(x.limit_of("u1", "publishes").unwrap(), 50);
    c.execute("INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('*', 'publishes', 7, 1, 'a')", []).unwrap();
    assert_eq!(x.limit_of("u1", "publishes").unwrap(), 7, "the site-wide value");
    c.execute("INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('u1', 'publishes', 99, 1, 'a')", []).unwrap();
    assert_eq!(x.limit_of("u1", "publishes").unwrap(), 99, "their own row wins");
    assert_eq!(x.limit_of("u2", "publishes").unwrap(), 7);
}

#[test]
fn daily_allowance_is_taken_until_the_cap_then_refused_with_retry_after_and_one_cap_alert() {
    let (c, _d) = open();
    user(&c, "u1", false);
    c.execute("INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('u1', 'previews', 3, 1, 'a')", []).unwrap();
    let now = 5 * DAY_MS + 3_600_000; // an hour into day 5
    let x = PoolCtx::new(&c, now);
    assert!(x.take_daily("u1", "previews", 2).unwrap());
    assert!(x.take_daily("u1", "previews", 1).unwrap());
    assert!(!x.take_daily("u1", "previews", 1).unwrap(), "the cap is 3");
    assert!(!x.take_daily("u1", "previews", 1).unwrap());
    let used: i64 = c.query_row("SELECT count FROM usage_daily WHERE user_id = 'u1' AND metric = 'previews'", [], |r| r.get(0)).unwrap();
    assert_eq!(used, 3, "a refused request takes nothing");
    let flags: i64 = c.query_row("SELECT COUNT(*) FROM admin_flags WHERE kind = 'cap' AND user_id = 'u1'", [], |r| r.get(0)).unwrap();
    let audits: i64 = c.query_row("SELECT COUNT(*) FROM pool_audit WHERE action = 'cap_reached'", [], |r| r.get(0)).unwrap();
    assert_eq!((flags, audits), (1, 1), "one flag and one audit row a user, day and metric");
    assert_eq!(x.take_notices(), vec![PoolNotice::Cap { user_id: "u1".into(), metric: "previews".into() }], "and one mail");
    let e = pool_err(x.require_daily("u1", "previews", 1).unwrap_err());
    assert_eq!((e.status, e.code.as_str()), (429, "daily_limit"));
    assert_eq!(e.body(), json!({ "error": "daily_limit", "metric": "previews" }));
    assert_eq!(e.headers, vec![("Retry-After".to_string(), "82800".to_string())], "23 hours to the next UTC midnight");
    assert!(x.take_notices().is_empty(), "the same day's repeat raises nothing more");
    // The next day starts fresh.
    let next = PoolCtx::new(&c, 6 * DAY_MS);
    assert!(next.take_daily("u1", "previews", 3).unwrap());
    // give_daily returns what was not used and never goes below zero.
    next.give_daily("u1", "previews", 10).unwrap();
    assert!(next.take_daily("u1", "previews", 3).unwrap());
}

#[test]
fn an_erased_account_gets_401_not_a_cap_answer() {
    let (c, _d) = open();
    user(&c, "gone", true);
    let x = PoolCtx::new(&c, 1_000);
    let e = pool_err(x.take_daily("gone", "previews", 1).unwrap_err());
    assert_eq!((e.status, e.code.as_str()), (401, "unauthorized"));
    assert_eq!(e.headers, vec![("WWW-Authenticate".to_string(), "Bearer error=\"invalid_token\"".to_string())]);
    assert_eq!(pool_err(x.count_miss("gone", "item_miss").unwrap_err()).status, 401);
    assert!(x.is_erased("gone").unwrap());
    let none: i64 = c.query_row("SELECT COUNT(*) FROM usage_daily WHERE user_id = 'gone'", [], |r| r.get(0)).unwrap();
    assert_eq!(none, 0, "nothing is written under an erased account");
}

#[test]
fn misses_count_up_to_the_guard_and_a_refund_takes_one_back() {
    let (c, _d) = open();
    user(&c, "u1", false);
    c.execute("INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('u1', 'item_miss', 2, 1, 'a')", []).unwrap();
    let x = PoolCtx::new(&c, DAY_MS);
    x.miss_guard("u1", "item_miss").unwrap();
    x.count_miss("u1", "item_miss").unwrap();
    x.miss_guard("u1", "item_miss").unwrap();
    x.count_miss("u1", "item_miss").unwrap();
    assert_eq!(pool_err(x.miss_guard("u1", "item_miss").unwrap_err()).code, "daily_limit");
    x.refund_miss("u1", "item_miss").unwrap();
    x.miss_guard("u1", "item_miss").unwrap();
    x.refund_miss("u1", "item_miss").unwrap();
    x.refund_miss("u1", "item_miss").unwrap();
    let n: i64 = c.query_row("SELECT count FROM usage_daily WHERE user_id = 'u1' AND metric = 'item_miss'", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0, "a refund never goes below zero");
}

#[test]
fn alert_mails_are_budgeted_an_hour_per_channel() {
    let (c, _d) = open();
    let x = PoolCtx::new(&c, 10 * HOUR_MS);
    for i in 0..8 {
        x.alert_budgeted(PoolNotice::Cap { user_id: format!("u{i}"), metric: "m".into() }).unwrap();
    }
    assert_eq!(x.take_notices().len() as i64, ALERTS_PER_HOUR, "five an hour for general alerts");
    // A different channel has its own budget, so a flood of caps cannot swallow a privacy report.
    x.alert_budgeted(PoolNotice::StudentDataReport { item_id: "i".into(), report_id: "r".into() }).unwrap();
    x.alert_budgeted(PoolNotice::PrivacyTextMatch { item_id: "i".into(), held: true, circle_id: None, version: None }).unwrap();
    x.alert_budgeted(PoolNotice::PrivacyTextMatch { item_id: "i".into(), held: true, circle_id: Some("c".into()), version: Some(2) }).unwrap();
    assert_eq!(x.take_notices().len(), 3);
    // The next hour starts a new budget.
    let later = PoolCtx::new(&c, 11 * HOUR_MS);
    later.alert_budgeted(PoolNotice::Cap { user_id: "u".into(), metric: "m".into() }).unwrap();
    assert_eq!(later.take_notices().len(), 1);
}

#[test]
fn notices_serialise_with_the_workers_field_names() {
    let n = PoolNotice::TakedownAuthor { item_id: "i".into(), author_id: "a".into(), action_id: "x".into() };
    assert_eq!(serde_json::to_value(&n).unwrap(), json!({ "kind": "takedown_author", "itemId": "i", "authorId": "a", "actionId": "x" }));
    let stored = r#"{"kind":"counter_notice","actionId":"x","complainantContact":"c","restoreNotBefore":9}"#;
    assert_eq!(serde_json::from_str::<PoolNotice>(stored).unwrap(), PoolNotice::CounterNotice { action_id: "x".into(), complainant_contact: "c".into(), restore_not_before: 9 });
    let m = PoolNotice::PrivacyTextMatch { item_id: "i".into(), held: false, circle_id: None, version: None };
    assert_eq!(serde_json::to_value(&m).unwrap(), json!({ "kind": "privacy_text_match", "itemId": "i", "held": false }), "absent options are left out");
    assert_eq!(PoolNotice::D1Size { percent: 80, bytes: 1 }.kind(), "d1_size");
    assert_eq!(PoolNotice::CircleEmailInvite { to: "a@b.c".into(), code: "k".into(), role: "editor".into(), expires_at: 1 }.kind(), "circle_email_invite");
}

#[test]
fn an_admin_note_is_scrubbed_of_the_subjects_name_and_email_when_written() {
    let (c, _d) = open();
    user(&c, "t1", false);
    let note = "Suspended Name t1 (t1@school.org) after review";
    audit(&c, &AuditEntry { actor_id: Some("adm"), actor_kind: "admin", action: "suspend", target_user_id: Some("t1"), note: Some(note), ..Default::default() }, 5, None).unwrap();
    audit(&c, &AuditEntry { actor_id: Some("t1"), actor_kind: "user", action: "other", target_user_id: Some("t1"), note: Some(note), ..Default::default() }, 5, None).unwrap();
    let rows: Vec<(String, String)> = {
        let mut st = c.prepare("SELECT actor_kind, note FROM pool_audit ORDER BY actor_kind").unwrap();
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(|x| x.unwrap()).collect()
    };
    assert_eq!(rows[0], ("admin".into(), "Suspended [user t1] ([user t1]) after review".into()));
    assert_eq!(rows[1].1, note, "only an admin's note is scrubbed");
    assert_eq!(audit(&c, &AuditEntry { actor_kind: "system", action: "never", ..Default::default() }, 5, Some("0")).unwrap(), 0, "a false condition writes nothing");
    assert_eq!(chunks(&[1, 2, 3, 4, 5], 2), vec![vec![1, 2], vec![3, 4], vec![5]]);
}
