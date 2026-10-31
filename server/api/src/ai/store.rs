//! SQLite side of the AI relay (port of worker/src/ai/store.ts): usage rows, the credit ledger and service
//! sessions. Every statement is the Worker's own; a D1 `batch([...])` is one transaction here.
use rusqlite::{Connection, OptionalExtension, Transaction, params};

pub const USAGE_KEEP_MS: i64 = 90 * super::credits::DAY_MS;

/// One answered model call, as booked for the quota and for pricing.
pub struct Usage<'a> {
    pub kind: &'a str, // "chat" | "import_page"
    pub in_tokens: f64,
    pub out_tokens: f64,
    /// US dollars the call cost (OpenRouter's `usage.cost`, or a fallback turn's neurons at $0.000011).
    pub cost_usd: f64,
    /// The credit session the call ran in, which links a cost to its service.
    pub session: Option<&'a str>,
}

pub struct NewSession<'a> {
    pub id: &'a str,
    pub principal: &'a str,
    pub service: &'a str,
    pub credits: i64,
    /// The part of `credits` the purchased balance pays; the weekly allowance pays the rest.
    pub paid: i64,
    pub calls_allowed: i64,
    pub pages: i64,
    pub now: i64,
    pub expires_at: i64,
    pub week_start: i64,
    pub allowance: i64,
}

#[derive(Debug, PartialEq)]
pub struct ServiceCost {
    pub service: String,
    pub sessions: i64,
    pub usd: f64,
    pub avg_usd: f64,
    pub calls: i64,
}

/// A result above this lives in the blob store instead of the job row.
pub const RESULT_D1_MAX: usize = 256 * 1024;

#[derive(Debug, PartialEq)]
pub struct JobRow {
    pub id: String,
    pub principal: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub status: String, // queued | running | done | failed
    pub pages_total: i64,
    pub pages_done: i64,
    pub hint: Option<String>,
    pub error: Option<String>,
    pub result: Option<String>,
}

#[derive(Debug, PartialEq)]
pub struct Purchase {
    pub principal: String,
    pub credits: i64,
    pub cents: i64,
}

pub struct AiStore<'a> {
    conn: &'a mut Connection,
}

impl<'a> AiStore<'a> {
    pub fn new(conn: &'a mut Connection) -> Self {
        Self { conn }
    }

    #[cfg(test)]
    pub fn exec_for_test(&self, sql: &str) {
        self.conn.execute_batch(sql).unwrap();
    }

    /// 32 random hex characters, from SQLite's own source of randomness.
    pub fn new_id(&self) -> rusqlite::Result<String> {
        self.conn.query_row("SELECT lower(hex(randomblob(16)))", [], |r| r.get(0))
    }

    pub fn record(&self, principal: &str, u: &Usage, now: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO ai_usage (id, principal, at, kind, in_tokens, out_tokens, neurons, cost_usd, session)
             VALUES (lower(hex(randomblob(16))), ?1, ?2, ?3, ?4, ?5, 0, ?6, ?7)",
            params![principal, now, u.kind, u.in_tokens.floor().max(0.0) as i64, u.out_tokens.floor().max(0.0) as i64, u.cost_usd.max(0.0), u.session],
        )?;
        Ok(())
    }

    /// Chat turns one principal made since `since`.
    pub fn chat_turns_since(&self, principal: &str, since: i64) -> rusqlite::Result<i64> {
        self.conn.query_row("SELECT COUNT(*) FROM ai_usage WHERE principal = ?1 AND at >= ?2 AND kind = 'chat'", params![principal, since], |r| r.get(0))
    }

    /// US dollars spent since `since` (one key pays for every account): the daily budget guard.
    pub fn cost_since(&self, since: i64) -> rusqlite::Result<f64> {
        self.conn.query_row("SELECT COALESCE(SUM(cost_usd), 0) FROM ai_usage WHERE at >= ?1", [since], |r| r.get(0))
    }

    /// What each service has cost per session since `since`: the measurement that prices credits.
    pub fn cost_by_service(&self, since: i64) -> rusqlite::Result<Vec<ServiceCost>> {
        let mut st = self.conn.prepare(
            "SELECT s.service, COUNT(DISTINCT s.id), COALESCE(SUM(u.cost_usd), 0), COUNT(u.id)
               FROM ai_sessions s JOIN ai_usage u ON u.session = s.id WHERE u.at >= ?1 GROUP BY s.service ORDER BY s.service",
        )?;
        st.query_map([since], |r| {
            let (sessions, usd): (i64, f64) = (r.get(1)?, r.get(2)?);
            Ok(ServiceCost { service: r.get(0)?, sessions, usd, avg_usd: if sessions > 0 { usd / sessions as f64 } else { 0.0 }, calls: r.get(3)? })
        })?
        .collect()
    }

    /// Weekly-allowance credits used since `week_start`: the part of each session the purchased balance did not
    /// pay. An expired session with no answered call is not counted.
    pub fn credits_spent(&self, principal: &str, week_start: i64, now: i64) -> rusqlite::Result<i64> {
        self.conn.query_row(
            "SELECT COALESCE(SUM(credits - paid_credits), 0) FROM ai_sessions WHERE principal = ?1 AND created_at >= ?2 AND NOT (calls_ok = 0 AND expires_at < ?3)",
            params![principal, week_start, now],
            |r| r.get(0),
        )
    }

    /// Purchased credits: the sum of the ledger. Negative after a refund or dispute took back spent credits.
    pub fn purchased_balance(&self, principal: &str) -> rusqlite::Result<i64> {
        self.conn.query_row("SELECT COALESCE(SUM(delta), 0) FROM credit_ledger WHERE principal = ?1", [principal], |r| r.get(0))
    }

    /// Adds a ledger row once; false when (kind, ref) is already there (a replayed Stripe event).
    pub fn ledger(&self, principal: &str, delta: i64, kind: &str, reference: &str, now: i64, cents: Option<i64>) -> rusqlite::Result<bool> {
        let n = self.conn.execute(
            "INSERT OR IGNORE INTO credit_ledger (id, principal, delta, kind, ref, at, cents) VALUES (lower(hex(randomblob(16))), ?1, ?2, ?3, ?4, ?5, ?6)",
            params![principal, delta, kind, reference, now, cents],
        )?;
        Ok(n == 1)
    }

    /// The purchase a payment id credited, or None if it is not one of ours.
    pub fn purchase(&self, reference: &str) -> rusqlite::Result<Option<Purchase>> {
        let row: Option<(String, i64, Option<i64>)> = self
            .conn
            .query_row("SELECT principal, delta, cents FROM credit_ledger WHERE kind = 'purchase' AND ref = ?1", [reference], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()?;
        Ok(row.and_then(|(principal, credits, cents)| cents.filter(|c| *c != 0).map(|cents| Purchase { principal, credits, cents })))
    }

    /// True once the account has been deleted. An id with no account row is not "deleted".
    pub fn account_deleted(&self, user_id: &str) -> rusqlite::Result<bool> {
        Ok(self.conn.query_row("SELECT 1 FROM users WHERE id = ?1 AND deleted_at IS NOT NULL", [user_id], |_| Ok(())).optional()?.is_some())
    }

    /// Credits already taken back for a payment (reversal refs start with `<payment id>#`).
    pub fn reversed_for(&self, prefix: &str) -> rusqlite::Result<i64> {
        self.conn.query_row(
            "SELECT COALESCE(-SUM(delta), 0) FROM credit_ledger WHERE kind = 'reversal' AND substr(ref, 1, ?2) = ?1",
            params![prefix, prefix.chars().count() as i64],
            |r| r.get(0),
        )
    }

    /// Gives back the purchased part of sessions that expired without an answered call. Idempotent per session.
    pub fn refund_expired(&self, principal: &str, now: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO credit_ledger (id, principal, delta, kind, ref, at)
             SELECT lower(hex(randomblob(16))), principal, paid_credits, 'refund', id, ?2 FROM ai_sessions
             WHERE principal = ?1 AND paid_credits > 0 AND calls_ok = 0 AND expires_at < ?2",
            params![principal, now],
        )?;
        Ok(())
    }

    /// Charges and opens a session atomically. The session row goes in only if the weekly cap and the ledger
    /// balance still hold, and the ledger spend is written only if that row exists, in one transaction.
    pub fn start_session(&mut self, s: &NewSession) -> rusqlite::Result<bool> {
        let tx: Transaction = self.conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let inserted = tx.execute(
            "INSERT INTO ai_sessions (id, principal, service, credits, paid_credits, calls_allowed, calls_used, calls_ok, pages, created_at, expires_at)
             SELECT ?1, ?2, ?3, ?4, ?11, ?5, 0, 0, ?6, ?7, ?8
             WHERE (SELECT COALESCE(SUM(credits - paid_credits), 0) FROM ai_sessions WHERE principal = ?2 AND created_at >= ?9 AND NOT (calls_ok = 0 AND expires_at < ?7)) + ?4 - ?11 <= ?10
               AND (?11 = 0 OR (SELECT COALESCE(SUM(delta), 0) FROM credit_ledger WHERE principal = ?2) >= ?11)",
            params![s.id, s.principal, s.service, s.credits, s.calls_allowed, s.pages, s.now, s.expires_at, s.week_start, s.allowance, s.paid],
        )?;
        tx.execute(
            "INSERT INTO credit_ledger (id, principal, delta, kind, ref, at)
             SELECT lower(hex(randomblob(16))), ?2, -?3, 'spend', ?1, ?4 WHERE ?3 > 0 AND EXISTS (SELECT 1 FROM ai_sessions WHERE id = ?1)",
            params![s.id, s.principal, s.paid, s.now],
        )?;
        tx.commit()?;
        Ok(inserted == 1)
    }

    /// Takes one model call from the session's budget; false when it is not this principal's, expired or used up.
    pub fn take_call(&self, id: &str, principal: &str, now: i64) -> rusqlite::Result<bool> {
        let n = self.conn.execute(
            "UPDATE ai_sessions SET calls_used = calls_used + 1 WHERE id = ?1 AND principal = ?2 AND expires_at > ?3 AND calls_used < calls_allowed",
            params![id, principal, now],
        )?;
        Ok(n == 1)
    }

    pub fn call_answered(&self, id: &str) -> rusqlite::Result<()> {
        self.conn.execute("UPDATE ai_sessions SET calls_ok = calls_ok + 1 WHERE id = ?1", [id])?;
        Ok(())
    }
    pub fn jobs_since(&self, principal: &str, since: i64) -> rusqlite::Result<i64> {
        self.conn.query_row("SELECT COUNT(*) FROM ai_jobs WHERE principal = ?1 AND created_at >= ?2", params![principal, since], |r| r.get(0))
    }

    pub fn create_job(&self, id: &str, principal: &str, pages_total: i64, hint: &str, now: i64) -> rusqlite::Result<()> {
        let hint = if hint.is_empty() { None } else { Some(hint) };
        self.conn.execute(
            "INSERT INTO ai_jobs (id, principal, created_at, updated_at, status, pages_total, pages_done, hint) VALUES (?1, ?2, ?3, ?3, 'queued', ?4, 0, ?5)",
            params![id, principal, now, pages_total, hint],
        )?;
        Ok(())
    }

    /// A job, only for the principal that made it: anyone else gets the same None as for a missing id.
    pub fn job(&self, id: &str, principal: &str) -> rusqlite::Result<Option<JobRow>> {
        self.conn
            .query_row(
                "SELECT id, principal, created_at, updated_at, status, pages_total, pages_done, hint, error, result FROM ai_jobs WHERE id = ?1 AND principal = ?2",
                params![id, principal],
                |r| {
                    Ok(JobRow {
                        id: r.get(0)?,
                        principal: r.get(1)?,
                        created_at: r.get(2)?,
                        updated_at: r.get(3)?,
                        status: r.get(4)?,
                        pages_total: r.get(5)?,
                        pages_done: r.get(6)?,
                        hint: r.get(7)?,
                        error: r.get(8)?,
                        result: r.get(9)?,
                    })
                },
            )
            .optional()
    }

    pub fn progress(&self, id: &str, status: &str, pages_done: i64, result: Option<&str>, error: Option<&str>, now: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE ai_jobs SET status = ?2, pages_done = ?3, result = ?4, error = ?5, updated_at = ?6 WHERE id = ?1",
            params![id, status, pages_done, result, error, now],
        )?;
        Ok(())
    }

    /// Drops expired jobs, usage and sessions past their retention; returns the expired job ids so the caller
    /// can delete their blob results (`ai/jobs/<id>/result.json`).
    pub fn prune(&self, now: i64, job_ttl_ms: i64) -> rusqlite::Result<Vec<String>> {
        let cutoff = now - job_ttl_ms;
        let ids = self.conn.prepare("SELECT id FROM ai_jobs WHERE created_at < ?1")?.query_map([cutoff], |r| r.get::<_, String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
        self.conn.execute("DELETE FROM ai_jobs WHERE created_at < ?1", [cutoff])?;
        self.conn.execute("DELETE FROM ai_usage WHERE at < ?1", [now - USAGE_KEEP_MS])?;
        self.conn.execute("DELETE FROM ai_sessions WHERE created_at < ?1", [now - USAGE_KEEP_MS])?;
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use std::path::Path;

    const MON: i64 = 1_792_972_800_000;

    fn conn() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
        c
    }

    fn session<'a>(id: &'a str, credits: i64, paid: i64, now: i64) -> NewSession<'a> {
        NewSession { id, principal: "u1", service: "import", credits, paid, calls_allowed: 3, pages: 1, now, expires_at: now + 3_600_000, week_start: MON, allowance: 10 }
    }

    #[test]
    fn the_weekly_allowance_is_a_hard_cap_that_a_refused_session_leaves_untouched() {
        let mut c = conn();
        let mut s = AiStore::new(&mut c);
        assert!(s.start_session(&session("a", 6, 0, MON)).unwrap());
        assert!(!s.start_session(&session("b", 6, 0, MON + 1)).unwrap(), "6 + 6 > 10");
        assert!(s.start_session(&session("c", 4, 0, MON + 2)).unwrap(), "6 + 4 = 10 fits exactly");
        assert_eq!(s.credits_spent("u1", MON, MON + 3).unwrap(), 10);
        assert_eq!(s.purchased_balance("u1").unwrap(), 0, "no spend row for free credits");
    }

    #[test]
    fn a_paid_session_needs_the_balance_and_books_the_spend_in_the_same_step() {
        let mut c = conn();
        let mut s = AiStore::new(&mut c);
        assert!(!s.start_session(&session("a", 2, 2, MON)).unwrap(), "no balance yet");
        assert!(s.ledger("u1", 5, "purchase", "pi_1", MON, Some(125)).unwrap());
        assert!(!s.ledger("u1", 5, "purchase", "pi_1", MON, Some(125)).unwrap(), "a replayed event adds nothing");
        assert!(s.start_session(&session("a", 2, 2, MON)).unwrap());
        assert_eq!(s.purchased_balance("u1").unwrap(), 3);
        assert_eq!(s.credits_spent("u1", MON, MON).unwrap(), 0, "the paid part is not the allowance's");
        assert!(!s.start_session(&session("b", 4, 4, MON)).unwrap(), "4 > 3 left");
        assert_eq!(s.purchased_balance("u1").unwrap(), 3, "a refused session spends nothing");
    }

    #[test]
    fn an_expired_session_with_no_answer_is_refunded_once_and_leaves_the_week() {
        let mut c = conn();
        let mut s = AiStore::new(&mut c);
        s.ledger("u1", 5, "purchase", "pi_1", MON, Some(125)).unwrap();
        s.start_session(&session("a", 2, 2, MON)).unwrap();
        s.start_session(&session("free", 3, 0, MON)).unwrap();
        let later = MON + 2 * 3_600_000;
        s.refund_expired("u1", later).unwrap();
        s.refund_expired("u1", later).unwrap();
        assert_eq!(s.purchased_balance("u1").unwrap(), 5, "refunded exactly once");
        assert_eq!(s.credits_spent("u1", MON, later).unwrap(), 0, "unanswered sessions no longer count");
        s.call_answered("free").unwrap();
        assert_eq!(s.credits_spent("u1", MON, later).unwrap(), 3, "an answered one does");
    }

    #[test]
    fn calls_are_taken_from_the_budget_only_by_the_owner_before_expiry() {
        let mut c = conn();
        let mut s = AiStore::new(&mut c);
        s.start_session(&session("a", 1, 0, MON)).unwrap();
        assert!(!s.take_call("a", "someone-else", MON).unwrap());
        for _ in 0..3 {
            assert!(s.take_call("a", "u1", MON).unwrap());
        }
        assert!(!s.take_call("a", "u1", MON).unwrap(), "budget of 3 used up");
        s.start_session(&session("b", 1, 0, MON)).unwrap();
        assert!(!s.take_call("b", "u1", MON + 3_600_000).unwrap(), "expired");
    }

    #[test]
    fn usage_rows_feed_the_daily_cap_the_hourly_cap_and_the_cost_report() {
        let mut c = conn();
        let mut s = AiStore::new(&mut c);
        s.start_session(&session("imp", 1, 0, MON)).unwrap();
        let mut fig = session("fig", 1, 0, MON);
        fig.service = "figure";
        s.start_session(&fig).unwrap();
        for _ in 0..3 {
            s.record("u1", &Usage { kind: "chat", in_tokens: 100.9, out_tokens: 20.0, cost_usd: 0.001, session: Some("imp") }, MON + 5).unwrap();
        }
        s.record("u1", &Usage { kind: "chat", in_tokens: 1.0, out_tokens: 1.0, cost_usd: 0.002, session: Some("fig") }, MON + 5).unwrap();
        s.record("u2", &Usage { kind: "import_page", in_tokens: 1.0, out_tokens: 1.0, cost_usd: -1.0, session: None }, MON + 5).unwrap();
        assert!((s.cost_since(MON).unwrap() - 0.005).abs() < 1e-9, "a negative cost is booked as zero");
        assert_eq!(s.cost_since(MON + 6).unwrap(), 0.0);
        assert_eq!(s.chat_turns_since("u1", MON).unwrap(), 4);
        assert_eq!(s.chat_turns_since("u2", MON).unwrap(), 0, "import pages are not chat turns");
        let r = s.cost_by_service(0).unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!((r[0].service.as_str(), r[0].sessions, r[0].calls), ("figure", 1, 1));
        assert_eq!((r[1].service.as_str(), r[1].sessions, r[1].calls), ("import", 1, 3));
        assert!((r[1].avg_usd - 0.003).abs() < 1e-9);
    }

    #[test]
    fn purchases_reversals_and_deleted_accounts_read_back() {
        let mut c = conn();
        c.execute("INSERT INTO users (id, trust, created_at, deleted_at) VALUES ('gone', 'basic', 1, 5), ('live', 'basic', 1, NULL)", []).unwrap();
        let s = AiStore::new(&mut c);
        s.ledger("u1", 25, "purchase", "pi_9", MON, Some(1000)).unwrap();
        s.ledger("u1", -5, "reversal", "pi_9#refund:200", MON, None).unwrap();
        s.ledger("u1", -3, "reversal", "pi_9#dispute:dp_1", MON, None).unwrap();
        s.ledger("u1", -1, "reversal", "pi_90#refund:1", MON, None).unwrap();
        assert_eq!(s.purchase("pi_9").unwrap(), Some(Purchase { principal: "u1".into(), credits: 25, cents: 1000 }));
        assert_eq!(s.purchase("pi_missing").unwrap(), None);
        assert_eq!(s.reversed_for("pi_9#").unwrap(), 8, "pi_90 is a different payment");
        assert!(!s.account_deleted("never-existed").unwrap());
        assert!(s.account_deleted("gone").unwrap());
        assert!(!s.account_deleted("live").unwrap());
    }

    #[test]
    fn a_job_is_private_to_its_owner_and_progress_is_stored() {
        let mut c = conn();
        let s = AiStore::new(&mut c);
        s.create_job("j1", "u1", 3, "Quiz 3", MON).unwrap();
        s.create_job("j2", "u1", 1, "", MON + 5).unwrap();
        let j = s.job("j1", "u1").unwrap().unwrap();
        assert_eq!((j.status.as_str(), j.pages_total, j.pages_done, j.hint.as_deref()), ("queued", 3, 0, Some("Quiz 3")));
        assert_eq!(s.job("j2", "u1").unwrap().unwrap().hint, None, "an empty hint is stored as NULL");
        assert_eq!(s.job("j1", "u2").unwrap(), None, "another user sees the same None as a missing job");
        assert_eq!(s.job("nope", "u1").unwrap(), None);
        s.progress("j1", "running", 2, Some("{\"pages\":[]}"), None, MON + 10).unwrap();
        s.progress("j2", "failed", 0, None, Some("No page could be read."), MON + 11).unwrap();
        let j = s.job("j1", "u1").unwrap().unwrap();
        assert_eq!((j.status.as_str(), j.pages_done, j.updated_at, j.result.as_deref()), ("running", 2, MON + 10, Some("{\"pages\":[]}")));
        assert_eq!(s.job("j2", "u1").unwrap().unwrap().error.as_deref(), Some("No page could be read."));
        assert_eq!(s.jobs_since("u1", MON).unwrap(), 2);
        assert_eq!(s.jobs_since("u1", MON + 1).unwrap(), 1);
        assert_eq!(s.jobs_since("u2", MON).unwrap(), 0);
        assert!(c_status_check_rejects(&s), "status is constrained to the four known values");
    }

    fn c_status_check_rejects(s: &AiStore) -> bool {
        s.create_job("j3", "u1", 1, "", MON).unwrap();
        s.progress("j3", "weird", 0, None, None, MON).is_err()
    }

    #[test]
    fn the_prune_drops_expired_jobs_and_old_usage_and_names_the_jobs() {
        let mut c = conn();
        let s = AiStore::new(&mut c);
        let ttl = 72 * 3_600_000;
        s.create_job("old", "u1", 1, "", MON).unwrap();
        s.record("u1", &Usage { kind: "chat", in_tokens: 1.0, out_tokens: 1.0, cost_usd: 0.001, session: None }, MON).unwrap();
        assert!(s.prune(MON + 71 * 3_600_000, ttl).unwrap().is_empty(), "inside the ttl nothing goes");
        assert_eq!(s.prune(MON + 73 * 3_600_000, ttl).unwrap(), vec!["old".to_string()]);
        assert_eq!(s.job("old", "u1").unwrap(), None);
        assert!(s.cost_since(0).unwrap() > 0.0, "usage is kept for 90 days");
        s.prune(MON + 91 * 86_400_000, ttl).unwrap();
        assert_eq!(s.cost_since(0).unwrap(), 0.0);
    }
}
