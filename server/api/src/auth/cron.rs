//! The Worker's scheduled stages for auth, on a timer (port of `scheduled` in worker/src/index.ts). The Worker ran at
//! minute 17 of every hour; the hourly stage sends the summary for sign-up notices over the budget, and the daily stage
//! (04:xx UTC) purges expired rows and escalates an overdue queue or sends the digest.
use super::admin_read::review_reminder;
use super::maintenance::{PurgeReport, purge_expired};
use super::notices::{flush_notice_backlog, queue_reminder};
use super::router::AuthState;
use std::sync::Arc;

const HOUR_MS: i64 = 3_600_000;
const MINUTE: i64 = 17;
const ESCALATE_AFTER_S: i64 = 48 * 3600;

/// Milliseconds from `now` until the next HH:17:00 UTC (a full hour if it is exactly that instant).
pub fn until_next_run(now: i64) -> i64 {
    let into_hour = now.rem_euclid(HOUR_MS);
    let target = MINUTE * 60_000;
    if into_hour < target { target - into_hour } else { HOUR_MS - into_hour + target }
}

pub fn is_daily_hour(now: i64) -> bool {
    now.div_euclid(HOUR_MS).rem_euclid(24) == 4
}

#[derive(Debug, Default)]
pub struct Tick {
    pub backlog: i64,
    pub purged: Option<PurgeReport>,
    pub reminder_sent: bool,
}

/// One scheduled run at `now`. Each stage's failure is logged and does not stop the next.
pub fn tick(s: &AuthState, now: i64) -> Tick {
    let mut t = Tick::default();
    {
        let mut conn = s.db.lock().unwrap_or_else(|p| p.into_inner());
        match flush_notice_backlog(&mut conn, &s.mail, now) {
            Ok(n) => t.backlog = n,
            Err(e) => tracing::warn!("cron: notice backlog: {e}"),
        }
    }
    if is_daily_hour(now) {
        let mut conn = s.db.lock().unwrap_or_else(|p| p.into_inner());
        match purge_expired(&mut conn, &s.cfg, now) {
            Ok(r) => t.purged = Some(r),
            Err(e) => tracing::warn!("cron: purge: {e}"),
        }
        let reminder = review_reminder(&conn, now, ESCALATE_AFTER_S, s.approval_digest);
        drop(conn);
        match reminder {
            Ok(Some(r)) => match queue_reminder(&s.mail, &r) {
                Ok(()) => t.reminder_sent = true,
                Err(e) => tracing::warn!("cron: queue reminder: {e}"),
            },
            Ok(None) => {}
            Err(_) => tracing::warn!("cron: review reminder: database error"),
        }
    }
    t
}

/// Runs forever: sleeps to each HH:17, then the stages on a blocking thread.
pub async fn run(s: Arc<AuthState>) {
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(until_next_run((s.clock)()) as u64)).await;
        let st = s.clone();
        let now = (s.clock)();
        match tokio::task::spawn_blocking(move || tick(&st, now)).await {
            Ok(t) => tracing::info!(backlog = t.backlog, daily = t.purged.is_some(), reminder = t.reminder_sent, "auth cron"),
            Err(_) => tracing::error!("auth cron panicked"),
        }
    }
}

#[cfg(test)]
#[path = "cron_tests.rs"]
mod tests;
