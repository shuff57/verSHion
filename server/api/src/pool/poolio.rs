//! The HTTP-free rest of worker/src/pool/shared.ts: the burst rate limits on every pool route, the notice outbox, the
//! held-copy notices, and the pure parts of the body readers (`parsePublish`, `readJson`, the content-type rules). The
//! streaming itself (deadlines, byte reservations, per-key read slots) belongs to the route layer, which hands these
//! functions the bytes it read.
use super::ctx::{Fail, PoolCtx, PoolError, PoolNotice, Res, bad};
use super::jsonscan::scan_json;
use crate::auth::crypto::{Key, hmac};
use crate::ratelimit::{Rule, hit, undo};
use rusqlite::{Connection, params};
use serde_json::{Map, Value};

/// Burst limits on every pool route, per account and per client IP (the daily caps come on top).
pub const RATE_ACCOUNT: Rule = Rule { limit: 300, window_seconds: 300 };
/// Per address: 20 account-shares, so a school's NAT with many teachers syncing is not cut off; only requests that passed
/// their account's limit are counted against it.
pub const RATE_IP: Rule = Rule { limit: 6000, window_seconds: 300 };
/// Read routes, per client IP per day: generous, because a whole school can share one address.
pub const RATE_IP_READ_DAILY: Rule = Rule { limit: 50_000, window_seconds: 86_400 };
/// One account's share of an address's daily read budget: 20%.
pub const RATE_ACCOUNT_IP_READ_DAILY: Rule = Rule { limit: 10_000, window_seconds: 86_400 };

/// Charges one request to the account's limits, then (only if it passed them) the address's. `ip` is the client address the
/// route found (None or blank: 403 `client_ip_required`); `bucket` groups it (IPv6 by prefix). The key scheme is the auth
/// package's (`a:<hmac(user:<id>)>:`), so account deletion purges these rows too.
pub fn rate_limit(conn: &Connection, limit_key: &Key, now: i64, ip: Option<&str>, bucket: &dyn Fn(&str) -> String, user_id: &str, read: bool) -> Res<()> {
    let ip = ip.map(str::trim).filter(|i| !i.is_empty()).ok_or_else(|| PoolError::new(403, "client_ip_required"))?;
    let acct = format!("a:{}:pool:", hmac(limit_key, &format!("user:{user_id}")));
    let ip_h = format!("i:{}:pool:", hmac(limit_key, &bucket(ip)));
    let mut account: Vec<(String, Rule)> = vec![(format!("{acct}{}", RATE_ACCOUNT.window_seconds), RATE_ACCOUNT)];
    let mut addr: Vec<(String, Rule)> = vec![(format!("{ip_h}{}", RATE_IP.window_seconds), RATE_IP)];
    if read {
        account.push((format!("{acct}{ip_h}read:{}", RATE_ACCOUNT_IP_READ_DAILY.window_seconds), RATE_ACCOUNT_IP_READ_DAILY));
        addr.push((format!("{ip_h}read:{}", RATE_IP_READ_DAILY.window_seconds), RATE_IP_READ_DAILY));
    }
    let mut charged: Vec<(String, Rule)> = Vec::new();
    for stage in [account, addr] {
        let mut denied = 0;
        for (k, r) in &stage {
            let v = hit(conn, k, *r, now)?;
            if !v.allowed {
                denied = denied.max(v.retry_after);
            }
        }
        charged.extend(stage);
        if denied > 0 {
            for (k, r) in &charged {
                undo(conn, k, *r, now)?;
            }
            return Err(PoolError::new(429, "rate_limited").header("Retry-After", denied.to_string()).into());
        }
    }
    Ok(())
}

/// A notice that must not be lost (the author's takedown notice, whose counter-notice window runs from it) is written to
/// `pool_outbox` with the action it reports (`id` deterministic: a retry writes it once), then sent by `flush_outbox`.
/// `cond`: SQL that must hold for it to be written. Returns whether a row was written.
pub fn outbox_insert(conn: &Connection, id: &str, n: &PoolNotice, now: i64, cond: &str) -> rusqlite::Result<bool> {
    let payload = serde_json::to_string(n).unwrap();
    Ok(conn.execute(&format!("INSERT OR IGNORE INTO pool_outbox (id, kind, payload, at) SELECT ?1, ?2, ?3, ?4 WHERE {cond}"), params![id, n.kind(), payload, now])? == 1)
}

/// Sends unsent outbox notices (these ids, or the oldest 200), marking each sent once `notify` took it. A failure leaves the
/// row for the next flush (the hourly cron). Returns how many went.
pub fn flush_outbox(conn: &Connection, now: i64, ids: Option<&[String]>, notify: &mut dyn FnMut(&PoolNotice) -> Result<(), String>) -> Res<usize> {
    let rows: Vec<(String, String)> = match ids {
        Some(ids) => conn
            .prepare("SELECT id, payload FROM pool_outbox WHERE sent_at IS NULL AND id IN (SELECT value FROM json_each(?1))")?
            .query_map([serde_json::to_string(ids).unwrap()], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?,
        None => conn
            .prepare("SELECT id, payload FROM pool_outbox WHERE sent_at IS NULL ORDER BY at LIMIT 200")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?,
    };
    let mut sent = 0;
    for (id, payload) in rows {
        let ok = serde_json::from_str::<PoolNotice>(&payload).map_err(|e| e.to_string()).and_then(|n| notify(&n));
        match ok {
            Ok(()) => {
                conn.execute("UPDATE pool_outbox SET sent_at = ?2 WHERE id = ?1 AND sent_at IS NULL", params![id, now])?;
                sent += 1;
            }
            Err(e) => eprintln!("pool: outbox {} failed: {e}", id.split(':').next().unwrap_or("")),
        }
    }
    Ok(sent)
}

/// Tells the authors of these items that an admin action held them. Every item, in chunks; the hourly budget decides what is
/// mailed, the rest is listed in "My shared items".
pub fn notify_held(x: &PoolCtx, item_ids: &[String], cause: &str) -> Res<()> {
    for ch in item_ids.chunks(500) {
        let rows: Vec<(String, String)> = x
            .conn
            .prepare("SELECT i.id, i.author_id FROM items i JOIN users u ON u.id = i.author_id WHERE i.id IN (SELECT value FROM json_each(?1)) AND u.deleted_at IS NULL")?
            .query_map([serde_json::to_string(ch).unwrap()], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        for (item_id, author_id) in rows {
            x.alert_budgeted(PoolNotice::CopyHeld { item_id, author_id, cause: cause.into() })?;
        }
    }
    Ok(())
}

pub const PUBLISH_CONTENT_TYPE: &str = "application/vnd.vershion.publish";
/// JSON route bodies at most (reports, appeals, prefs, versions, counter-notices).
pub const JSON_BODY_MAX: usize = 64 * 1024;

/// The media type of a Content-Type header: before any `;`, trimmed, lower case.
pub fn content_type_of(header: Option<&str>) -> String {
    header.unwrap_or("").split(';').next().unwrap_or("").trim().to_lowercase()
}
pub fn is_publish_type(header: Option<&str>) -> bool {
    content_type_of(header) == PUBLISH_CONTENT_TYPE
}

fn utf8_object(b: &[u8]) -> Option<Map<String, Value>> {
    // (A BOM is refused by the scan before this; `from_slice` rejects invalid UTF-8.)
    match serde_json::from_slice::<Value>(b) {
        Ok(Value::Object(m)) => Some(m),
        _ => None,
    }
}

/// A publish request body `<meta JSON>\n<payload bytes>`: the meta (held to share-core's structural rules by the scan before
/// the parse) and the offset where the payload starts.
pub fn parse_publish(raw: &[u8], max_payload: usize, max_meta: usize) -> Res<(Map<String, Value>, usize)> {
    let nl = raw.iter().position(|b| *b == b'\n');
    let Some(nl) = nl.filter(|n| *n <= max_meta) else { return Err(bad("meta").into()) };
    if raw.len() - nl - 1 > max_payload {
        return Err(PoolError::new(413, "body_too_large").with("limit", max_payload.into()).into());
    }
    let sc = scan_json(&raw[..nl]);
    if !sc.ok {
        return Err(PoolError::new(400, "invalid_json").with("field", "meta".into()).with("reason", sc.code.into()).into());
    }
    let meta = utf8_object(&raw[..nl]).ok_or_else(|| PoolError::new(400, "invalid_json").with("field", "meta".into()))?;
    Ok((meta, nl + 1))
}

/// A small JSON object body (`application/json` only, so an HTML form cannot send it cross-site), already read by the route.
/// With `optional`, no body is no body: Content-Length 0 (whatever Content-Type a runtime added), or neither a length nor a
/// type (a bare POST) is `{}`; 415 only when a body is really sent.
pub fn read_json(content_type: Option<&str>, content_length: Option<&str>, body: &[u8], optional: bool) -> Res<Map<String, Value>> {
    read_json_max(content_type, content_length, body, optional, JSON_BODY_MAX)
}
/// `read_json` with its own size cap (a bank PATCH carries strip decisions: up to a megabyte).
pub fn read_json_max(content_type: Option<&str>, content_length: Option<&str>, body: &[u8], optional: bool, max: usize) -> Res<Map<String, Value>> {
    if optional && (content_length == Some("0") || (content_length.is_none() && (content_type.is_none() || body.is_empty()))) {
        return Ok(Map::new());
    }
    if content_type_of(content_type) != "application/json" {
        return Err(PoolError::new(415, "unsupported_media_type").into());
    }
    if body.len() > max {
        return Err(PoolError::new(413, "body_too_large").with("limit", max.into()).into());
    }
    // Duplicate keys (a parse keeps the last) and the other structural limits are refused before the parse.
    let sc = scan_json(body);
    if !sc.ok {
        return Err(PoolError::new(400, "invalid_json").with("reason", sc.code.into()).into());
    }
    utf8_object(body).ok_or_else(|| Fail::from(PoolError::new(400, "invalid_json")))
}

#[cfg(test)]
#[path = "poolio_tests.rs"]
mod tests;
