//! The invite-code gate (worker/src/pool/invites.ts). Pool use needs BOTH the admin's approval (trust `teacher`) AND a redeemed
//! invite code an admin issued. Until an account holds a redemption every other pool route answers `403 invite_required`
//! (admins exempt; `POST /me/invite` and `GET /me/export` stay open).
//!
//! Codes: 120 random bits as 24 Crockford base32 characters (six groups of four), returned ONCE when issued; only the SHA-256
//! of the normalised form is stored. Single use by default, `max_uses` up to 1,000, an optional expiry, revocable (a revocation
//! stops further redemptions; it never undoes one). Redemption attempts are a daily allowance per account (`invite_tries`).
//!
//! HTTP-free cores: the route layer reads the body, answers with `(status, body)`, and writes the auth package's own admin audit
//! (`h.audit`) beside the pool audit written here.
use super::ctx::{AuditEntry, PoolCtx, PoolError, Res, audit, bad};
use super::sqlfrag::{DAY_MS, alive_sql, hex, is_id, new_id};
use crate::auth::validate::clean_text;
use rusqlite::{OptionalExtension, params};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

/// Crockford base32 (no I, L, O, U).
pub const INVITE_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// Random bytes per code: 15 bytes = 120 bits = 24 characters.
pub const INVITE_BYTES: usize = 15;
pub const INVITE_CHARS: usize = INVITE_BYTES * 8 / 5;
pub const INVITE_MAX_USES: i64 = 1000;
pub const INVITE_MAX_DAYS: i64 = 365;
/// Invites one page of `GET /admin/pool/invites` lists (newest first, cursor paged).
pub const INVITE_PAGE: usize = 100;
/// Routes an account without a redeemed invite may still call.
pub const INVITE_FREE_PATHS: [&str; 2] = ["/me/invite", "/me/export"];

const INVITE_REQUIRED_MESSAGE: &str = "The shared pool is invite-only. Enter the invite code an admin gave you.";
const INVALID_MESSAGE: &str = "This invite code is not valid: it may have expired, been used up or been withdrawn. Ask an admin for a new one.";

/// A new code, grouped for reading aloud: XXXX-XXXX-XXXX-XXXX-XXXX-XXXX.
pub fn new_invite_code() -> String {
    let mut b = [0u8; INVITE_BYTES];
    getrandom::getrandom(&mut b).expect("the system random source");
    encode_code(&b)
}

fn encode_code(b: &[u8]) -> String {
    let (mut bits, mut acc, mut out) = (0u32, 0u32, String::new());
    for x in b {
        acc = (acc << 8) | *x as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(INVITE_ALPHABET[((acc >> bits) & 31) as usize] as char);
        }
        acc &= (1 << bits) - 1;
    }
    out.as_bytes().chunks(4).map(|c| std::str::from_utf8(c).unwrap()).collect::<Vec<_>>().join("-")
}

/// The normalised form of a typed code (case, spaces and hyphens ignored; I/L read as 1, O as 0), or None.
pub fn normalise_invite_code(raw: &Value) -> Option<String> {
    let raw = raw.as_str()?;
    if raw.encode_utf16().count() > 100 {
        return None;
    }
    let s: String = raw.to_uppercase().chars().filter(|c| !c.is_whitespace() && *c != '-').map(|c| if c == 'I' || c == 'L' { '1' } else if c == 'O' { '0' } else { c }).collect();
    (s.chars().count() == INVITE_CHARS && s.bytes().all(|b| INVITE_ALPHABET.contains(&b))).then_some(s)
}

pub fn invite_hash(normalised: &str) -> String {
    hex(&Sha256::digest(normalised.as_bytes()))
}

/// 403 `invite_required` unless the account holds a redemption (an erased account: the 401 an unauthenticated request gets).
pub fn require_invite(x: &PoolCtx, user_id: &str) -> Res<()> {
    if x.conn.query_row("SELECT 1 FROM pool_invite_redemptions WHERE user_id = ?1", [user_id], |_| Ok(())).optional()?.is_some() {
        return Ok(());
    }
    x.throw_if_erased(user_id)?;
    Err(PoolError::new(403, "invite_required").with("message", INVITE_REQUIRED_MESSAGE.into()).into())
}

fn invalid() -> PoolError {
    PoolError::new(422, "invite_invalid").with("message", INVALID_MESSAGE.into())
}

/// POST /me/invite {code}: the account redeems an invite code. One redemption per account: an account that already holds one is
/// told so and nothing is used up. Every refusal of a code (unknown, expired, revoked, used up) is the same `422 invite_invalid`.
pub fn redeem_invite(x: &PoolCtx, me: &str, body: &Map<String, Value>) -> Res<Value> {
    let code = body.get("code").and_then(normalise_invite_code).ok_or_else(|| bad("code"))?;
    let held = || -> rusqlite::Result<bool> { Ok(x.conn.query_row("SELECT 1 FROM pool_invite_redemptions WHERE user_id = ?1", [me], |_| Ok(())).optional()?.is_some()) };
    if held()? {
        return Ok(json!({ "ok": true, "already": true }));
    }
    x.require_daily(me, "invite_tries", 1)?;
    let inv: Option<String> = x.conn.query_row("SELECT id FROM pool_invites WHERE code_hash = ?1", [invite_hash(&code)], |r| r.get(0)).optional()?;
    let inv = inv.ok_or_else(invalid)?;
    // One transaction: the use is taken only if the code is still good and the account holds no redemption and stands
    // (nothing is written for an erased account); the redemption and its audit follow it by changes().
    let tx = x.conn.unchecked_transaction()?;
    let used = tx.execute(
        &format!(
            "UPDATE pool_invites SET uses = uses + 1
              WHERE id = ?1 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > ?2) AND uses < max_uses
                AND NOT EXISTS (SELECT 1 FROM pool_invite_redemptions WHERE user_id = ?3) AND {}",
            alive_sql("?3")
        ),
        params![inv, x.now, me],
    )?;
    tx.execute("INSERT INTO pool_invite_redemptions (user_id, invite_id, at) SELECT ?1, ?2, ?3 WHERE changes() = 1", params![me, inv, x.now])?;
    audit(&tx, &AuditEntry { actor_id: Some(me), actor_kind: "user", action: "invite.redeem", target_user_id: Some(me), detail: Some(json!({ "invite_id": inv })), ..Default::default() }, x.now, Some("changes() = 1"))?;
    tx.commit()?;
    if used == 1 {
        return Ok(json!({ "ok": true, "redeemed": true }));
    }
    // Nothing taken: a concurrent redemption of this account won, the account was erased, or the code is spent.
    if held()? {
        return Ok(json!({ "ok": true, "already": true }));
    }
    x.throw_if_erased(me)?;
    Err(invalid().into())
}

fn note_of(v: Option<&Value>) -> Res<Option<String>> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.is_empty() => Ok(None),
        Some(v) => clean_text(v, 1000).map(Some).ok_or_else(|| bad("note").into()),
    }
}
fn int_in(v: &Value, lo: i64, hi: i64, field: &str) -> Res<i64> {
    // (a JS integer: a whole number, whatever its JSON spelling)
    let n = v.as_f64().filter(|f| f.fract() == 0.0).map(|f| f as i64);
    match n {
        Some(n) if (lo..=hi).contains(&n) => Ok(n),
        _ => Err(PoolError::new(400, "invalid_request").with("field", field.into()).with("min", lo.into()).with("max", hi.into()).into()),
    }
}
fn opt<'a>(b: &'a Map<String, Value>, k: &str) -> Option<&'a Value> {
    b.get(k).filter(|v| !v.is_null())
}

const STATE_SQL: &str = "CASE WHEN revoked_at IS NOT NULL THEN 'revoked' WHEN uses >= max_uses THEN 'used_up' WHEN expires_at IS NOT NULL AND expires_at <= ?1 THEN 'expired' ELSE 'active' END";

fn invite_cols() -> String {
    format!("id, created_at, created_by, max_uses, uses, expires_at, revoked_at, revoked_by, note, {STATE_SQL} AS state")
}

fn invite_json(r: &rusqlite::Row) -> rusqlite::Result<Value> {
    let g = |i: usize| r.get::<_, rusqlite::types::Value>(i);
    let v = |i: usize| match g(i).unwrap() {
        rusqlite::types::Value::Null => Value::Null,
        rusqlite::types::Value::Integer(n) => json!(n),
        rusqlite::types::Value::Text(s) => json!(s),
        other => json!(format!("{other:?}")),
    };
    let names = ["id", "created_at", "created_by", "max_uses", "uses", "expires_at", "revoked_at", "revoked_by", "note", "state"];
    Ok(Value::Object(names.iter().enumerate().map(|(i, n)| (n.to_string(), v(i))).collect()))
}

/// POST /admin/pool/invites {max_uses?: 1..1000 (default 1), expires_in_days?: 1..365, note?}: the code is in this answer only.
/// The code itself is never written anywhere: neither audit has it.
pub fn issue_invite(x: &PoolCtx, admin_id: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let max_uses = match opt(body, "max_uses") {
        None => 1,
        Some(v) => int_in(v, 1, INVITE_MAX_USES, "max_uses")?,
    };
    let days = opt(body, "expires_in_days").map(|v| int_in(v, 1, INVITE_MAX_DAYS, "expires_in_days")).transpose()?;
    let note = note_of(body.get("note"))?;
    let code = new_invite_code();
    let id = new_id();
    let expires_at = days.map(|d| x.now + d * DAY_MS);
    let tx = x.conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO pool_invites (id, code_hash, created_at, created_by, max_uses, uses, expires_at, note) VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7)",
        params![id, invite_hash(&normalise_invite_code(&json!(code)).unwrap()), x.now, admin_id, max_uses, expires_at, note],
    )?;
    audit(&tx, &AuditEntry { actor_id: Some(admin_id), actor_kind: "admin", action: "invite.issue", detail: Some(json!({ "invite_id": id, "max_uses": max_uses, "expires_at": expires_at })), note: note.as_deref(), ..Default::default() }, x.now, None)?;
    tx.commit()?;
    Ok((201, json!({ "invite_id": id, "code": code, "max_uses": max_uses, "expires_at": expires_at, "note": note, "shown_once": true })))
}

/// The listing's query parameters.
#[derive(Default)]
pub struct InviteQuery<'a> {
    pub id: Option<&'a str>,
    pub user_id: Option<&'a str>,
    pub after: Option<&'a str>,
}

/// GET /admin/pool/invites. `id`: that invite with every account that redeemed it. `user_id`: that account's redemption (null:
/// none; invite_id null: approved before the gate). Otherwise the invites, newest first, paged (`after` = created_at:id).
pub fn list_invites(x: &PoolCtx, q: &InviteQuery) -> Res<(u16, Value)> {
    if let Some(one) = q.id {
        if !is_id(one) {
            return Err(bad("id").into());
        }
        let inv = x.conn.query_row(&format!("SELECT {} FROM pool_invites WHERE id = ?2", invite_cols()), params![x.now, one], invite_json).optional()?;
        let Some(mut inv) = inv else { return Ok((404, json!({ "error": "not_found" }))) };
        let red: Vec<Value> = x
            .conn
            .prepare("SELECT user_id, at FROM pool_invite_redemptions WHERE invite_id = ?1 ORDER BY at, user_id")?
            .query_map([one], |r| Ok(json!({ "user_id": r.get::<_, String>(0)?, "at": r.get::<_, i64>(1)? })))?
            .collect::<Result<_, _>>()?;
        inv["redeemed_by"] = Value::Array(red);
        return Ok((200, json!({ "invite": inv })));
    }
    if let Some(who) = q.user_id {
        // `^[0-9a-f-]{36}$`
        if who.len() != 36 || !who.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b) || b == b'-') {
            return Err(bad("user_id").into());
        }
        let r: Option<(Option<String>, i64)> = x.conn.query_row("SELECT invite_id, at FROM pool_invite_redemptions WHERE user_id = ?1", [who], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
        let redemption = r.map(|(inv, at)| json!({ "invite_id": inv, "at": at, "grandfathered": inv.is_none() }));
        return Ok((200, json!({ "user_id": who, "redemption": redemption })));
    }
    // `^(\d{1,16}):([0-9a-f]{32})$`
    let cur = q.after.and_then(|a| a.split_once(':')).filter(|(n, id)| (1..=16).contains(&n.len()) && n.bytes().all(|b| b.is_ascii_digit()) && is_id(id)).map(|(n, id)| (n.parse::<i64>().unwrap(), id));
    let rows: Vec<Value> = x
        .conn
        .prepare(&format!(
            "SELECT {} FROM pool_invites WHERE ?2 IS NULL OR created_at < ?2 OR (created_at = ?2 AND id < ?3) ORDER BY created_at DESC, id DESC LIMIT ?4",
            invite_cols()
        ))?
        .query_map(params![x.now, cur.map(|c| c.0), cur.map(|c| c.1).unwrap_or(""), (INVITE_PAGE + 1) as i64], invite_json)?
        .collect::<Result<_, _>>()?;
    let page = &rows[..rows.len().min(INVITE_PAGE)];
    // (one more than shown: the cursor says there is more)
    let next = if rows.len() > INVITE_PAGE { page.last().map(|l| format!("{}:{}", l["created_at"], l["id"].as_str().unwrap())) } else { None };
    Ok((200, json!({ "invites": page, "next_cursor": next })))
}

/// POST /admin/pool/invites/revoke {invite_id, note?}: no further redemptions. Accounts that already redeemed it keep access.
pub fn revoke_invite(x: &PoolCtx, admin_id: &str, body: &Map<String, Value>) -> Res<(u16, Value)> {
    let id = body.get("invite_id").and_then(Value::as_str).filter(|i| is_id(i)).ok_or_else(|| bad("invite_id"))?;
    let note = note_of(body.get("note"))?;
    let there: Option<Option<i64>> = x.conn.query_row("SELECT revoked_at FROM pool_invites WHERE id = ?1", [id], |r| r.get(0)).optional()?;
    match there {
        None => return Ok((404, json!({ "error": "not_found" }))),
        Some(Some(at)) => return Ok((409, json!({ "error": "already_revoked", "revoked_at": at }))),
        Some(None) => {}
    }
    let tx = x.conn.unchecked_transaction()?;
    let n = tx.execute("UPDATE pool_invites SET revoked_at = ?2, revoked_by = ?3 WHERE id = ?1 AND revoked_at IS NULL", params![id, x.now, admin_id])?;
    audit(&tx, &AuditEntry { actor_id: Some(admin_id), actor_kind: "admin", action: "invite.revoke", detail: Some(json!({ "invite_id": id })), note: note.as_deref(), ..Default::default() }, x.now, Some("changes() = 1"))?;
    tx.commit()?;
    // (a concurrent revoke won: same answer as above)
    if n != 1 {
        return Ok((409, json!({ "error": "already_revoked" })));
    }
    Ok((200, json!({ "ok": true, "invite_id": id })))
}

#[cfg(test)]
#[path = "invites_tests.rs"]
mod tests;
