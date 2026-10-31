//! Mail to the admins: the new-application notice with an hourly budget (past it, sign-ups are counted and the hourly
//! timer sends one summary), the alert for any action against an admin account, and the queue reminder (port of
//! worker/src/notices.ts and the tail of worker/src/mail.ts).
use super::admin_read::{Reminder, ReminderKind};
use super::smtp::{MailEnv, send_raw, send_to};
use rusqlite::Connection;

pub const NOTICES_PER_HOUR: i64 = 5;
const HOUR_MS: i64 = 3_600_000;

#[derive(Debug, PartialEq, Eq)]
pub enum Notice {
    Sent,
    Deferred,
}

/// "New application" mail, within the hourly budget. Returns what to do; the caller sends after releasing the database.
pub fn new_application_budget(conn: &Connection, now: i64) -> rusqlite::Result<Notice> {
    let sent: i64 = conn.query_row(
        "INSERT INTO notice_budget (hour, sent) VALUES (?1, 1) ON CONFLICT (hour) DO UPDATE SET sent = sent + 1 RETURNING sent",
        [now / HOUR_MS],
        |r| r.get(0),
    )?;
    if sent <= NOTICES_PER_HOUR {
        return Ok(Notice::Sent);
    }
    conn.execute("UPDATE notice_backlog SET pending = pending + 1 WHERE id = 1", [])?;
    Ok(Notice::Deferred)
}

pub fn notice_new_application(conn: &Connection, env: &MailEnv, user_id: &str, now: i64) -> Result<Notice, String> {
    let n = new_application_budget(conn, now).map_err(|_| "db".to_string())?;
    if n == Notice::Sent {
        send_to(env, false, "verSHion: an account is awaiting approval", &format!("Account {user_id} confirmed its address and is in the approval queue."))?;
    }
    Ok(n)
}

/// Hourly: one summary mail for the sign-ups that were over the budget, and old budget rows dropped.
pub fn flush_notice_backlog(conn: &mut Connection, env: &MailEnv, now: i64) -> Result<i64, String> {
    let tx = conn.transaction().map_err(|_| "db".to_string())?;
    let n: i64 = tx.query_row("SELECT pending FROM notice_backlog WHERE id = 1", [], |r| r.get(0)).unwrap_or(0);
    tx.execute("UPDATE notice_backlog SET pending = pending - ?1 WHERE id = 1", [n]).map_err(|_| "db".to_string())?;
    tx.execute("DELETE FROM notice_budget WHERE hour < ?1", [now / HOUR_MS - 24]).map_err(|_| "db".to_string())?;
    tx.commit().map_err(|_| "db".to_string())?;
    if n > 0 {
        send_to(
            env,
            false,
            &format!("verSHion: {n} more account(s) awaiting approval"),
            &format!("{n} more account(s) joined the approval queue in the last hour (not mailed one by one). Bulk-reject from the queue if they are spam."),
        )?;
    }
    Ok(n)
}

pub struct AdminEvent<'a> {
    pub action: &'a str,
    pub actor_id: Option<&'a str>,
    pub target_id: &'a str,
    pub at: i64,
    pub proposal_id: Option<&'a str>,
}

fn iso(ms: i64) -> String {
    super::signup::iso8601(ms)
}

/// Any action against an admin-level account, at once, to every admin AND the escalation contact: a compromised admin
/// cannot quietly suspend or remove the others.
pub fn admin_alert(env: &MailEnv, e: &AdminEvent) -> Result<(), String> {
    let what = match e.action {
        "admin_suspension_proposed" => "proposed suspending the admin account",
        "admin_suspension_cancelled" => "cancelled a proposal to suspend the admin account",
        "admin_suspended" => "suspended the admin account",
        "admin_account_deleted" => "deleted their own admin account",
        other => other,
    };
    let mut text = format!("Admin {} {what} {} at {}.", e.actor_id.unwrap_or("(unknown)"), e.target_id, iso(e.at));
    if let Some(p) = e.proposal_id {
        text.push_str(&format!(" Proposal {p}: another admin must confirm it, or it can be cancelled from the admin screen."));
    }
    text.push_str(" If this was not expected, check the audit log now; scripts/make-admin.sh can restore a suspended admin.");
    send_to(env, true, &format!("verSHion ALERT: admin action ({})", e.action), &text)
}

/// The timer's queue reminder. An overdue queue goes to every admin AND the escalation contact, so a single admin going
/// silent cannot leave applicants waiting unnoticed. The digest goes to both too: the last day's decisions per admin are
/// what makes one admin approving a wave of accounts visible to someone else.
pub fn queue_reminder(env: &MailEnv, r: &Reminder) -> Result<(), String> {
    let hours = r.oldest_age_seconds / 3600;
    let (subject, text) = match r.kind {
        ReminderKind::Escalation => (
            format!("verSHion: {} application(s) waiting, the oldest for {hours} h", r.open),
            format!("The approval queue has {} open application(s); the oldest has waited {hours} hours. If the admin is unavailable, a second admin can be promoted with the make-admin script.", r.open),
        ),
        ReminderKind::Digest => {
            let mut t = format!("The approval queue has {} open application(s); the oldest has waited {hours} hours.\nLast 24 h: {} approved, {} rejected.", r.open, r.approved_last_day, r.rejected_last_day);
            for (admin, a, rej) in &r.by_admin {
                t.push_str(&format!("\n  admin {admin}: {a} approved, {rej} rejected"));
            }
            (format!("verSHion: daily approval digest ({} waiting)", r.open), t)
        }
    };
    let mut first = None;
    for addr in env.admin_emails.iter().chain(env.escalation_emails.iter()) {
        if let Err(e) = send_raw(env, addr, &subject, &text) {
            first.get_or_insert(e);
        }
    }
    first.map_or(Ok(()), Err)
}

#[cfg(test)]
#[path = "notices_tests.rs"]
mod tests;
