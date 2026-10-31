//! Tests for `cron.rs`.
use super::*;
use crate::auth::smtp::MailEnv;
use crate::db;
use std::path::Path;

const DAY: i64 = 86_400_000;
// A UTC midnight near 2027-01-15.
const MIDNIGHT: i64 = 1_800_000_000_000 - 1_800_000_000_000 % DAY;

fn state(digest: bool) -> (AuthState, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    let mail = MailEnv::from_lookup(|k| [("MAIL_MODE", "log"), ("APP_URL", "http://localhost"), ("ADMIN_EMAIL", "a@x.org")].iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())).unwrap();
    let mut s = AuthState::new(conn, super::super::config::AuthConfig::new(&"s".repeat(32), vec![]).unwrap(), mail);
    s.approval_digest = digest;
    (s, dir)
}

#[test]
fn the_next_run_is_always_minute_seventeen() {
    assert_eq!(until_next_run(MIDNIGHT), 17 * 60_000);
    assert_eq!(until_next_run(MIDNIGHT + 10 * 60_000), 7 * 60_000);
    assert_eq!(until_next_run(MIDNIGHT + 17 * 60_000), HOUR_MS, "exactly on time waits a full hour");
    assert_eq!(until_next_run(MIDNIGHT + 59 * 60_000), 18 * 60_000);
    assert!(is_daily_hour(MIDNIGHT + 4 * HOUR_MS + 17 * 60_000));
    assert!(!is_daily_hour(MIDNIGHT + 5 * HOUR_MS) && !is_daily_hour(MIDNIGHT + 3 * HOUR_MS));
}

#[test]
fn the_hourly_stage_flushes_the_backlog_and_the_daily_stage_purges_and_reminds() {
    let (s, _d) = state(true);
    s.db.lock().unwrap().execute("UPDATE notice_backlog SET pending = 4", []).unwrap();
    s.db.lock().unwrap().execute("INSERT INTO rate_limits (key, window_start, count, expires_at) VALUES ('k', 0, 1, 5)", []).unwrap();
    let hourly = tick(&s, MIDNIGHT + 2 * HOUR_MS + 17 * 60_000);
    assert_eq!((hourly.backlog, hourly.purged.is_none(), hourly.reminder_sent), (4, true, false));
    let n: i64 = s.db.lock().unwrap().query_row("SELECT COUNT(*) FROM rate_limits", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1, "the purge waits for the daily hour");
    let daily = tick(&s, MIDNIGHT + 4 * HOUR_MS + 17 * 60_000);
    assert_eq!(daily.purged.unwrap().rate_limits, 1);
    assert!(daily.reminder_sent, "the digest is on, so a reminder goes out even with nothing waiting");
}

#[test]
fn without_the_digest_an_empty_queue_sends_nothing() {
    let (s, _d) = state(false);
    let t = tick(&s, MIDNIGHT + 4 * HOUR_MS + 17 * 60_000);
    assert!(t.purged.is_some() && !t.reminder_sent);
}
