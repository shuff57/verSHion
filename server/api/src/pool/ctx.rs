//! The pool's request context and its shared effects (the effectful parts of worker/src/pool/shared.ts): the error every
//! pool route answers with, notices to a human, the audit insert, the daily allowances and the miss guard.
//!
//! `PoolCtx` is synchronous like the rest of the port: a handler holds the database connection for the whole of its work
//! (it runs in `spawn_blocking`), and notices queue on `notices` to be delivered after the reply.
use super::sqlfrag::{DAY_MS, HOUR_MS, alive_sql, day_of, default_limit, new_id};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::cell::RefCell;

/// What a pool route answers with when it refuses: `{error: code, ...extra}` at `status`, plus any headers.
#[derive(Debug, Clone, PartialEq)]
pub struct PoolError {
    pub status: u16,
    pub code: String,
    pub extra: Map<String, Value>,
    pub headers: Vec<(String, String)>,
}

impl PoolError {
    pub fn new(status: u16, code: &str) -> Self {
        PoolError { status, code: code.into(), extra: Map::new(), headers: vec![] }
    }
    pub fn with(mut self, k: &str, v: Value) -> Self {
        self.extra.insert(k.into(), v);
        self
    }
    pub fn header(mut self, k: &str, v: impl Into<String>) -> Self {
        self.headers.push((k.into(), v.into()));
        self
    }
    /// The JSON body.
    pub fn body(&self) -> Value {
        let mut m = Map::new();
        m.insert("error".into(), Value::String(self.code.clone()));
        for (k, v) in &self.extra {
            m.insert(k.clone(), v.clone());
        }
        Value::Object(m)
    }
}

pub fn not_found() -> PoolError {
    PoolError::new(404, "not_found")
}
pub fn bad(field: &str) -> PoolError {
    PoolError::new(400, "invalid_request").with("field", json!(field))
}
/// What an unauthenticated request gets (the answer for a write skipped because the account is erased).
pub fn unauthorized() -> PoolError {
    PoolError::new(401, "unauthorized").header("WWW-Authenticate", "Bearer error=\"invalid_token\"")
}

/// A pool failure: a refusal to answer with, or the database failing underneath.
#[derive(Debug)]
pub enum Fail {
    Pool(PoolError),
    Db(rusqlite::Error),
}
impl From<PoolError> for Fail {
    fn from(e: PoolError) -> Self {
        Fail::Pool(e)
    }
}
impl From<rusqlite::Error> for Fail {
    fn from(e: rusqlite::Error) -> Self {
        Fail::Db(e)
    }
}
pub type Res<T> = Result<T, Fail>;

/// Something the pool wants a human to hear about. The JSON form is what `pool_outbox.payload` holds (the Worker's field
/// names, so rows written there are read here).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum PoolNotice {
    StudentDataReport { item_id: String, report_id: String },
    TakedownAuthor { item_id: String, author_id: String, action_id: String },
    KeyMismatch { key_id: String, stored_key_id: String },
    CounterNotice { action_id: String, complainant_contact: String, restore_not_before: i64 },
    Cap { user_id: String, metric: String },
    RepeatInfringer { user_id: String, strikes: i64 },
    /// `circle_id` set: a circle send or rollback; `item_id` is then the circle bank's id.
    PrivacyTextMatch {
        item_id: String,
        held: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        circle_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        version: Option<i64>,
    },
    CronFailure { stage: String },
    /// share-core trapped more than a few times in an hour (kept so stored rows still parse).
    WasmTrap { traps: i64 },
    D1Size { percent: i64, bytes: i64 },
    /// The author of a copy an admin action held (ids only; the reason class, never the matched text).
    CopyHeld { item_id: String, author_id: String, cause: String },
    KeyLeakReport { item_id: String, author_id: String },
    CircleReport { circle_id: String, bank_id: String, report_id: String, reason: String, held: bool },
    /// An email invite: the ONLY place the address and the code exist; never stored, never logged.
    CircleEmailInvite { to: String, code: String, role: String, expires_at: i64 },
    CircleCopyrightMatch { item_id: String, circle_id: String, version: i64 },
}

pub const ALERTS_PER_HOUR: i64 = 5;
/// Held-copy notices to authors an hour (their own channel; the rest see it in "My shared items").
pub const AUTHOR_NOTICES_PER_HOUR: i64 = 200;
/// Student-data report mails an hour (their own channel).
pub const PRIVACY_ALERTS_PER_HOUR: i64 = 20;

impl PoolNotice {
    /// The budget channel: no flood of one kind swallows another.
    fn channel(&self) -> String {
        match self {
            PoolNotice::StudentDataReport { .. } => "privacy_report".into(),
            PoolNotice::CircleReport { reason, .. } if reason == "student_data" => "privacy_report".into(),
            PoolNotice::PrivacyTextMatch { circle_id, .. } => if circle_id.is_some() { "circle_privacy_match" } else { "privacy_match" }.into(),
            PoolNotice::CopyHeld { .. } => "authors".into(),
            PoolNotice::KeyLeakReport { .. } => "authors_key_leak".into(),
            other => other.kind(),
        }
    }
    pub fn kind(&self) -> String {
        serde_json::to_value(self).ok().and_then(|v| v.get("kind").and_then(|k| k.as_str().map(String::from))).unwrap_or_default()
    }
}

pub struct PoolCtx<'a> {
    pub conn: &'a Connection,
    pub now: i64,
    /// Notices queued by this request, delivered after the reply.
    pub notices: RefCell<Vec<PoolNotice>>,
}

impl<'a> PoolCtx<'a> {
    pub fn new(conn: &'a Connection, now: i64) -> Self {
        PoolCtx { conn, now, notices: RefCell::new(Vec::new()) }
    }
    pub fn send(&self, n: PoolNotice) {
        self.notices.borrow_mut().push(n);
    }
    pub fn take_notices(&self) -> Vec<PoolNotice> {
        std::mem::take(&mut *self.notices.borrow_mut())
    }

    /// Admin alert mails (cap crossings and the like), at most ALERTS_PER_HOUR a channel; the rest stay in admin_flags.
    pub fn alert_budgeted(&self, n: PoolNotice) -> Res<()> {
        let channel = n.channel();
        let budget = match channel.as_str() {
            "privacy_report" => PRIVACY_ALERTS_PER_HOUR,
            "authors" | "authors_key_leak" => AUTHOR_NOTICES_PER_HOUR,
            _ => ALERTS_PER_HOUR,
        };
        let sent: i64 = self.conn.query_row(
            "INSERT INTO pool_alert_budget (hour, channel, sent) VALUES (?1, ?2, 1) ON CONFLICT (hour, channel) DO UPDATE SET sent = sent + 1 RETURNING sent",
            params![self.now.div_euclid(HOUR_MS), channel],
            |r| r.get(0),
        )?;
        if sent <= budget {
            self.send(n);
        }
        Ok(())
    }

    /// One admin flag (and at most a few mails an hour) per user, day and metric.
    pub fn cap_alert(&self, user_id: &str, metric: &str) -> Res<()> {
        let dedup = format!("cap:{user_id}:{}:{metric}", day_of(self.now));
        let tx = self.conn.unchecked_transaction()?;
        let ins = tx.execute(
            "INSERT OR IGNORE INTO admin_flags (id, kind, user_id, detail, dedup, at) VALUES (?1, 'cap', ?2, ?3, ?4, ?5)",
            params![new_id(), user_id, json!({ "metric": metric }).to_string(), dedup, self.now],
        )?;
        // The audit row is written only with a fresh flag: the previous statement's `changes() = 1`.
        audit(&tx, &AuditEntry { actor_kind: "system", action: "cap_reached", target_user_id: Some(user_id), detail: Some(json!({ "metric": metric })), ..Default::default() }, self.now, Some("changes() = 1"))?;
        tx.commit()?;
        if ins == 1 {
            self.alert_budgeted(PoolNotice::Cap { user_id: user_id.into(), metric: metric.into() })?;
        }
        Ok(())
    }

    /// True when the account's tombstone has committed.
    pub fn is_erased(&self, user_id: &str) -> Res<bool> {
        Ok(self.conn.query_row("SELECT 1 FROM users WHERE id = ?1 AND deleted_at IS NOT NULL", [user_id], |_| Ok(())).optional()?.is_some())
    }
    /// A write whose guarded statement changed nothing: 401 when the reason is that the account is erased.
    pub fn throw_if_erased(&self, user_id: &str) -> Res<()> {
        if self.is_erased(user_id)? { Err(unauthorized().into()) } else { Ok(()) }
    }

    /// The cap of `metric` for the user: their own row, else the site-wide one, else the default.
    pub fn limit_of(&self, user_id: &str, metric: &str) -> Res<i64> {
        let v: Option<i64> = self
            .conn
            .query_row("SELECT value FROM pool_limits WHERE metric = ?1 AND user_id IN (?2, '*') ORDER BY user_id = '*' LIMIT 1", params![metric, user_id], |r| r.get(0))
            .optional()?;
        Ok(v.or_else(|| default_limit(metric)).unwrap_or(0))
    }

    /// Takes `n` from today's allowance of `metric`; false (and nothing taken) when it would cross the cap. One atomic
    /// UPSERT, so concurrent requests cannot overshoot.
    pub fn take_daily(&self, user_id: &str, metric: &str, n: i64) -> Res<bool> {
        let cap = self.limit_of(user_id, metric)?;
        let r: Option<i64> = self
            .conn
            .query_row(
                &format!(
                    "INSERT INTO usage_daily (user_id, day, metric, count) SELECT ?1, ?2, ?3, ?4 WHERE ?4 <= ?5 AND {}
       ON CONFLICT (user_id, day, metric) DO UPDATE SET count = count + excluded.count WHERE count + excluded.count <= ?5
       RETURNING count",
                    alive_sql("?1")
                ),
                params![user_id, day_of(self.now), metric, n, cap],
                |r| r.get(0),
            )
            .optional()?;
        if r.is_some() {
            return Ok(true);
        }
        self.throw_if_erased(user_id)?; // not a cap hit: the account is gone
        self.cap_alert(user_id, metric)?;
        Ok(false)
    }

    /// Gives back what a request took and did not use (an import that failed).
    pub fn give_daily(&self, user_id: &str, metric: &str, n: i64) -> Res<()> {
        self.conn.execute("UPDATE usage_daily SET count = MAX(0, count - ?4) WHERE user_id = ?1 AND day = ?2 AND metric = ?3", params![user_id, day_of(self.now), metric, n])?;
        Ok(())
    }

    /// 429 when today's allowance of `metric` is used up.
    pub fn require_daily(&self, user_id: &str, metric: &str, n: i64) -> Res<()> {
        if self.take_daily(user_id, metric, n)? {
            Ok(())
        } else {
            Err(self.daily_limit(metric).into())
        }
    }

    fn daily_limit(&self, metric: &str) -> PoolError {
        let secs = (((day_of(self.now) + 1) * DAY_MS - self.now) as f64 / 1000.0).ceil().max(1.0) as i64;
        PoolError::new(429, "daily_limit").with("metric", json!(metric)).header("Retry-After", secs.to_string())
    }

    /// Counts a miss (404) of `metric` (item_miss / blob_miss / publish_refusals).
    pub fn count_miss(&self, user_id: &str, metric: &str) -> Res<()> {
        let changed = self.conn.execute(
            &format!(
                "INSERT INTO usage_daily (user_id, day, metric, count) SELECT ?1, ?2, ?3, 1 WHERE {}
       ON CONFLICT (user_id, day, metric) DO UPDATE SET count = count + 1",
                alive_sql("?1")
            ),
            params![user_id, day_of(self.now), metric],
        )?;
        if changed == 0 {
            self.throw_if_erased(user_id)?;
        }
        Ok(())
    }

    /// Takes back one counted miss (a reserved refusal that turned out not to be one).
    pub fn refund_miss(&self, user_id: &str, metric: &str) -> Res<()> {
        self.conn.execute("UPDATE usage_daily SET count = count - 1 WHERE user_id = ?1 AND day = ?2 AND metric = ?3 AND count > 0", params![user_id, day_of(self.now), metric])?;
        Ok(())
    }

    /// Past the day's miss allowance, every id-taking route answers 429 for the rest of the day.
    pub fn miss_guard(&self, user_id: &str, metric: &str) -> Res<()> {
        let count: Option<i64> = self.conn.query_row("SELECT count FROM usage_daily WHERE user_id = ?1 AND day = ?2 AND metric = ?3", params![user_id, day_of(self.now), metric], |r| r.get(0)).optional()?;
        if let Some(c) = count {
            if c >= self.limit_of(user_id, metric)? {
                self.cap_alert(user_id, metric)?;
                return Err(self.daily_limit(metric).into());
            }
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct AuditEntry<'a> {
    pub actor_id: Option<&'a str>,
    /// "user", "admin" or "system".
    pub actor_kind: &'a str,
    pub action: &'a str,
    pub item_id: Option<&'a str>,
    pub target_user_id: Option<&'a str>,
    pub detail: Option<Value>,
    pub note: Option<&'a str>,
}

/// A pool_audit insert; `when` adds a SQL condition (for "only if this batch won"). Returns the rows written (0 or 1).
/// An ADMIN's note is kept as evidence after the subject's erasure, so the subject's personal data is scrubbed from it when
/// written: the display name and email of the target account (or of the item's author) become the account id.
pub fn audit(conn: &Connection, e: &AuditEntry, now: i64, when: Option<&str>) -> rusqlite::Result<usize> {
    conn.execute(
        &format!(
            "INSERT INTO pool_audit (id, at, actor_id, actor_kind, action, item_id, target_user_id, detail, note)
       SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
              CASE WHEN ?9 IS NULL OR ?4 <> 'admin' THEN ?9 ELSE (
                SELECT COALESCE(
                         (SELECT replace(replace(?9, CASE WHEN length(COALESCE(su.display_name, '')) >= 3 THEN su.display_name ELSE char(1) END, '[user ' || su.id || ']'),
                                         CASE WHEN length(COALESCE(su.email, '')) >= 3 THEN su.email ELSE char(1) END, '[user ' || su.id || ']')
                            FROM users su WHERE su.id = COALESCE(?7, (SELECT author_id FROM items WHERE id = ?6))), ?9)) END
       {}",
            when.map(|w| format!("WHERE {w}")).unwrap_or_default()
        ),
        params![new_id(), now, e.actor_id, e.actor_kind, e.action, e.item_id, e.target_user_id, e.detail.as_ref().map(|d| d.to_string()), e.note],
    )
}

/// Splits `xs` into chunks of `n`.
pub fn chunks<T: Clone>(xs: &[T], n: usize) -> Vec<Vec<T>> {
    xs.chunks(n).map(|c| c.to_vec()).collect()
}

#[cfg(test)]
#[path = "ctx_tests.rs"]
mod tests;
