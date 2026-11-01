//! What the Worker's pool `handle` does before a handler runs (worker/src/pool/index.ts `createPool`, shared.ts `poolKeyVerdict`): the method and origin
//! checks, the burst rate limit, the trust and invite gates, and the pool-key check on writes. HTTP-free: the route layer hands in what it read from the
//! request and turns the `PoolError` into the reply.
use super::core::Core;
use super::ctx::{PoolCtx, PoolError, Res};
use super::fingerprint::{Fingerprinter, KeyVerdict, assert_pool_key};
use super::invites::{INVITE_FREE_PATHS, require_invite};
use super::poolcontent::{awaiting_first_decision, pending_blob_path, pending_circle_path, pool_trust};
use super::poolio::rate_limit;
use super::routetable::Route;
use crate::auth::crypto::Key;
use crate::auth::guard::ip_bucket;
use crate::blobs::BlobStore;
use rusqlite::OptionalExtension;
use serde_json::Value;

/// Who is calling, as the auth package reports it (trust read from the database on every request).
#[derive(Debug, Clone, PartialEq)]
pub struct Caller {
    pub user_id: String,
    pub trust: String,
}

/// What the route layer read from the request.
pub struct Req<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub origin: Option<&'a str>,
    pub x_client: Option<&'a str>,
    /// The client address the route found (last `X-Forwarded-For` entry); None or blank is a 403.
    pub ip: Option<&'a str>,
}

/// Recomputes the keyed content hashes of up to five retained publishes and compares them with the stored ones: a pool made under another key is a mismatch.
/// No pool data at all is `Empty`; data with no readable body to check against is `Unknown`.
pub fn pool_key_verdict(x: &PoolCtx, store: &BlobStore, core: &Core) -> rusqlite::Result<KeyVerdict> {
    let mut st = x.conn.prepare(
        "SELECT p.publish_id, p.full_key FROM item_publishes p JOIN items i ON i.id = p.item_id AND i.fold_rev = 1
          WHERE p.full_key IS NOT NULL AND p.status IN ('current', 'previous')
            AND EXISTS (SELECT 1 FROM item_questions q WHERE q.publish_id = p.publish_id AND q.full_content IS NOT NULL)
          ORDER BY p.publish_id LIMIT 5",
    )?;
    let rows: Vec<(String, String)> = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
    if rows.is_empty() {
        let any = x.conn.query_row("SELECT 1 FROM items LIMIT 1", [], |_| Ok(())).optional()?.is_some();
        return Ok(if any { KeyVerdict::Unknown } else { KeyVerdict::Empty });
    }
    let (mut seen, mut all) = (false, true);
    for (publish_id, key) in rows {
        let Ok(Some(bytes)) = store.get(&key) else { continue };
        let Ok(p) = serde_json::from_slice::<Value>(&bytes) else { continue };
        let qs: Vec<Value> = match (p.get("bank").filter(|b| !b.is_null()), p.get("question").filter(|q| !q.is_null())) {
            (Some(b), _) => b.get("questions").and_then(Value::as_array).cloned().unwrap_or_default(),
            (None, Some(q)) => vec![q.clone()],
            _ => vec![],
        };
        if qs.is_empty() {
            continue;
        }
        let Ok(made) = core.content_hashes(&qs) else { continue };
        let mut q = x.conn.prepare("SELECT full_content FROM item_questions WHERE publish_id = ?1 ORDER BY pos")?;
        let stored: Vec<String> = q.query_map([&publish_id], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        seen = true;
        if stored != made {
            all = false;
        }
    }
    Ok(if !seen {
        KeyVerdict::Unknown
    } else if all {
        KeyVerdict::Match
    } else {
        KeyVerdict::Mismatch
    })
}

/// The pool key check: the key in use must be the one the data was made with (503 `pool_key_mismatch` otherwise).
pub fn check_key(x: &PoolCtx, store: &BlobStore, core: &Core, fp: &Fingerprinter) -> Res<()> {
    assert_pool_key(x, fp, || pool_key_verdict(x, store, core).unwrap_or(KeyVerdict::Unknown))?;
    Ok(())
}

pub struct Gate<'a> {
    pub allowed_origins: &'a [String],
    pub limit_key: &'a Key,
    pub ip_prefix: usize,
    pub store: &'a BlobStore,
    pub core: &'a Core,
    pub fp: &'a Fingerprinter,
}

/// Method, origin, rate limit, trust, invite and key: in the Worker's order. `caller` None is the 401 an unauthenticated request gets.
pub fn gate(g: &Gate, x: &PoolCtx, route: &Route, req: &Req, caller: Option<&Caller>) -> Res<Caller> {
    if !route.methods().contains(&req.method) {
        return Err(PoolError::new(405, "method_not_allowed").header("Allow", route.methods().join(", ")).into());
    }
    if let Some(o) = req.origin {
        if !g.allowed_origins.iter().any(|a| a == o) {
            return Err(PoolError::new(403, "origin_not_allowed").into());
        }
        if req.x_client.is_some() {
            return Err(PoolError::new(400, "native_client_with_origin").into());
        }
    }
    let Some(p) = caller else {
        return Err(PoolError::new(401, "unauthorized").header("WWW-Authenticate", r#"Bearer error="invalid_token""#).into());
    };
    // Every authenticated request is counted first, approved or not, so a pending account cannot send unlimited requests.
    let is_read = matches!(req.method, "GET" | "HEAD") || req.path.ends_with("/import") || req.path.ends_with("/versions") || req.path == "/circles/heads";
    rate_limit(x.conn, g.limit_key, x.now, req.ip, &|ip| ip_bucket(ip, g.ip_prefix), &p.user_id, is_read)?;
    // pending and unverified accounts (and anything else) cannot use the public pool. A CONFIRMED pending account may export its profile, redeem a pool
    // invite code and use circles, but only while its latest approval decision is still open.
    let upload = pending_blob_path(req.method, req.path);
    let pending_ok = p.trust == "pending" && (pending_circle_path(req.method, req.path) || upload) && awaiting_first_decision(x.conn, &p.user_id)?;
    let free = INVITE_FREE_PATHS.contains(&req.path);
    if !pool_trust(&p.trust) && !pending_ok && !(free && p.trust == "pending") {
        let msg = if p.trust == "pending" {
            "Your account is waiting for approval. The shared pool opens once an admin approves it; you can already join a circle with a circle code."
        } else {
            "This account cannot use the shared pool."
        };
        return Err(PoolError::new(403, "account_not_approved").with("message", msg.into()).into());
    }
    // The PUBLIC pool also needs a redeemed invite code (admins are exempt; the export and the redemption itself are open; circle paths and the image
    // uploads of a circle send are exempt too).
    if p.trust != "admin" && !pending_circle_path(req.method, req.path) && !upload && !free {
        require_invite(x, &p.user_id)?;
    }
    // No write under a key other than the pool's own (reads continue). POST /items/versions and POST /circles/heads are bulk reads.
    if !matches!(req.method, "GET" | "HEAD") && req.path != "/items/versions" && req.path != "/circles/heads" {
        check_key(x, g.store, g.core, g.fp)?;
    }
    Ok(p.clone())
}

#[cfg(test)]
#[path = "gate_tests.rs"]
mod tests;
