//! The base of the circles module (worker/src/pool/circles.ts, constants, small helpers, membership lookups, codes and caps): what every
//! circle route builds on. The SQL fragments are in `circlesql.rs`.
use super::circlesql::{memberships_sql, membership_rows_sql, owned_sql};
use super::ctx::{Fail, PoolCtx, PoolError, Res, not_found};
use super::invites::{INVITE_ALPHABET, INVITE_CHARS};
use super::sqlfrag::{DAY_MS, hex, is_id};
use rusqlite::{OptionalExtension, params};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const CIRCLE_NAME_MAX: usize = 80;
pub const CIRCLE_DESC_MAX: usize = 1000;
pub const CIRCLE_NOTE_MAX: usize = 200;
pub const CIRCLE_CODE_MAX_USES: i64 = 50;
pub const CIRCLE_CODE_DAYS: i64 = 7;
/// An owner close, a bank deletion: hidden at once, undoable this long, then purged.
pub const CIRCLE_UNDO_MS: i64 = 14 * DAY_MS;
/// Banks one POST /circles/heads may ask about.
pub const HEADS_MAX: usize = 200;
pub const ACTIVITY_PAGE: usize = 50;
pub const CIRCLE_INVITE_PAGE: usize = 100;
/// Versions of a bank kept in full; older ones keep their history line only.
pub const KEPT_VERSIONS: i64 = 20;
/// Owner closes in 30 days past which the account is flagged for the admin.
pub const CIRCLE_CLOSES_FLAG: i64 = 5;
pub const ESTABLISHED_MEMBER_MS: i64 = DAY_MS;
pub const ESTABLISHED_ACCOUNT_MS: i64 = 7 * DAY_MS;
pub const CIRCLE_INSTANT_HOLDS_PER_DAY: i64 = 3;
pub const CIRCLE_THROTTLE_MS: i64 = 30 * DAY_MS;
pub const CIRCLE_REPORT_REASONS: [&str; 4] = ["student_data", "copyright", "inappropriate", "other"];
pub const DELETED_NAME: &str = "deleted teacher";
pub const TRANSFER_OFFER_MS: i64 = 7 * DAY_MS;

/// A member's display name: "deleted teacher" once erased.
pub fn name_of(display: Option<&str>, deleted_at: Option<i64>) -> String {
    if deleted_at.is_some() { DELETED_NAME.into() } else { display.unwrap_or(DELETED_NAME).into() }
}
/// A circle response answers missing text as "", never null.
pub fn text(v: Option<&str>) -> String {
    v.unwrap_or("").into()
}
/// Who closed a circle, from its close reason (None while open).
pub fn closed_by(reason: Option<&str>) -> Option<&'static str> {
    match reason {
        Some("owner_closed") => Some("owner"),
        Some("admin") => Some("admin"),
        Some("last_member_gone") => Some("last_member"),
        _ => None,
    }
}
/// SQL for `closed_by` on the column/expression `r`.
pub fn closed_by_sql(r: &str) -> String {
    format!("CASE {r} WHEN 'owner_closed' THEN 'owner' WHEN 'admin' THEN 'admin' WHEN 'last_member_gone' THEN 'last_member' END")
}

/// The next pool_seq value (orders joins and versions without clock ties).
pub fn next_seq(x: &PoolCtx) -> rusqlite::Result<i64> {
    x.conn.query_row("UPDATE pool_seq SET n = n + 1 WHERE id = 1 RETURNING n", [], |r| r.get(0))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Membership {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub closed_at: Option<i64>,
    pub close_reason: Option<String>,
    pub undo_until: Option<i64>,
    pub purge_after: Option<i64>,
    /// "owner" | "editor" | "viewer"
    pub role: String,
    pub joined_at: i64,
}

/// The circle and the caller's ACTIVE membership of it, or None (not a member, former member, purged).
pub fn member_of(x: &PoolCtx, cid: &str, me: &str) -> rusqlite::Result<Option<Membership>> {
    x.conn
        .query_row(
            "SELECT ci.id, ci.name, ci.description, ci.created_at, ci.updated_at, ci.closed_at, ci.close_reason, ci.undo_until, ci.purge_after, m.role, m.joined_at
         FROM circles ci JOIN circle_members m ON m.circle_id = ci.id AND m.user_id = ?2 AND m.left_at IS NULL
        WHERE ci.id = ?1 AND ci.purged_at IS NULL",
            params![cid, me],
            |r| {
                Ok(Membership {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    description: r.get(2)?,
                    created_at: r.get(3)?,
                    updated_at: r.get(4)?,
                    closed_at: r.get(5)?,
                    close_reason: r.get(6)?,
                    undo_until: r.get(7)?,
                    purge_after: r.get(8)?,
                    role: r.get(9)?,
                    joined_at: r.get(10)?,
                })
            },
        )
        .optional()
}

/// The caller's membership, or a counted 404 (the same for a circle that does not exist).
pub fn circle_or_miss(x: &PoolCtx, cid: &str, me: &str) -> Res<Membership> {
    x.miss_guard(me, "item_miss")?;
    let m = if is_id(cid) { member_of(x, cid, me)? } else { None };
    if let Some(m) = m {
        return Ok(m);
    }
    x.count_miss(me, "item_miss")?;
    Err(not_found().into())
}

pub fn closed_error(closed_reason: Option<&str>, undo_until: Option<i64>) -> PoolError {
    let message = match closed_reason {
        Some("owner_closed") => "The owner closed this circle.",
        Some("admin") => "An administrator closed this circle.",
        _ => "This circle closed when its last member left.",
    };
    PoolError::new(409, "circle_closed").with("reason", closed_reason.map(Value::from).unwrap_or(Value::Null)).with("undo_until", undo_until.map(Value::from).unwrap_or(Value::Null)).with("message", message.into())
}
pub fn require_open(m: &Membership) -> Res<()> {
    if m.closed_at.is_some() { Err(closed_error(m.close_reason.as_deref(), m.undo_until).into()) } else { Ok(()) }
}
pub fn require_role(m: &Membership, roles: &[&str]) -> Res<()> {
    if roles.contains(&m.role.as_str()) {
        return Ok(());
    }
    let needed = if roles.contains(&"editor") { "editor" } else { roles[0] };
    Err(PoolError::new(403, "circle_role_required").with("needed", needed.into()).with("role", m.role.clone().into()).into())
}

/// After a guarded write changed nothing: 401 if the account is erased, else what the state says now.
pub fn explain(x: &PoolCtx, cid: &str, me: &str, roles: Option<&[&str]>) -> Fail {
    let go = || -> Res<()> {
        x.throw_if_erased(me)?;
        let m = member_of(x, cid, me)?.ok_or_else(not_found)?;
        require_open(&m)?;
        if let Some(r) = roles {
            require_role(&m, r)?;
        }
        Ok(())
    };
    match go() {
        Err(e) => e,
        Ok(()) => PoolError::new(409, "conflict").with("message", "Something changed meanwhile; reload the circle and try again.".into()).into(),
    }
}

/// A circle code: "C" and 24 Crockford base32 characters, shown as C-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX.
pub fn new_circle_code() -> String {
    format!("C-{}", super::invites::new_invite_code())
}
/// The normalised form of a typed circle code (case, spaces and hyphens ignored; I/L read as 1, O as 0), or None.
pub fn normalise_circle_code(raw: &Value) -> Option<String> {
    let raw = raw.as_str()?;
    if raw.encode_utf16().count() > 100 {
        return None;
    }
    let s: String = raw.to_uppercase().chars().filter(|c| !c.is_whitespace() && *c != '-').collect();
    if s.chars().count() != INVITE_CHARS + 1 || !s.starts_with('C') {
        return None;
    }
    let body: String = s[1..].chars().map(|c| if c == 'I' || c == 'L' { '1' } else if c == 'O' { '0' } else { c }).collect();
    body.bytes().all(|b| INVITE_ALPHABET.contains(&b)).then(|| format!("C{body}"))
}
pub fn circle_code_hash(normalised: &str) -> String {
    hex(&Sha256::digest(format!("circle:{normalised}").as_bytes()))
}

/// The circles_owned refusal says a closed circle still counts for 14 days.
pub fn owned_message(cap: i64, mine: bool) -> String {
    format!(
        "{} {}{}{} circles, the most one account may own. A closed circle still counts for 14 days, until it is purged; you can also pass one on.",
        if mine { "You own" } else { "That would make" },
        cap,
        if mine { "" } else { "+" },
        ""
    )
}
#[derive(Debug, Clone, Copy)]
pub struct Caps {
    pub owned: i64,
    pub memberships: i64,
    pub rows: i64,
}
pub fn caps_of(x: &PoolCtx, me: &str) -> Res<Caps> {
    Ok(Caps { owned: x.limit_of(me, "circles_owned")?, memberships: x.limit_of(me, "circle_memberships")?, rows: x.limit_of(me, "circle_memberships_total")? })
}
/// The 422 `circles_limit` that names the cap actually reached.
pub fn limit_error(x: &PoolCtx, me: &str, caps: Caps, owning: bool) -> Res<PoolError> {
    let (owned, memberships, rows): (i64, i64, i64) = x.conn.query_row(
        &format!("SELECT {} AS owned, {} AS memberships, {} AS rows_", owned_sql("?1"), memberships_sql("?1"), membership_rows_sql("?1")),
        [me],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let e = PoolError::new(422, "circles_limit");
    Ok(if owning && owned >= caps.owned {
        e.with("metric", "circles_owned".into()).with("used", owned.into()).with("limit", caps.owned.into()).with("message", owned_message(caps.owned, true).into())
    } else if memberships >= caps.memberships {
        e.with("metric", "circle_memberships".into())
            .with("used", memberships.into())
            .with("limit", caps.memberships.into())
            .with("message", format!("You are in {} circles, the most allowed. Leave one first.", caps.memberships).into())
    } else {
        e.with("metric", "circle_memberships_total".into())
            .with("used", rows.into())
            .with("limit", caps.rows.into())
            .with("message", format!("You have been in {} circles that still exist. You can join another once some of them close.", caps.rows).into())
    })
}

#[cfg(test)]
#[path = "circlebase_tests.rs"]
mod tests;
