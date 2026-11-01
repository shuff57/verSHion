//! The pool admin routes over HTTP (worker/src/pool/admin.ts `poolAdminRoutes`). The guard (admin account, known device, sudo token for writes) is the
//! auth package's, behind `PoolAdminGuard`; every write then checks the pool key; bodies are read after the guard, up to `ADMIN_BODY_MAX`. The cores
//! write both audit lines themselves, so a handler only passes the admin's id. Paths are exact: ids travel in the body or the query string.
use super::ctx::{Fail, PoolCtx, PoolError, Res};
use super::gate::check_key;
use super::preread::ADMIN_BODY_MAX;
use super::routes::{PoolState, Reply, client_ip, fail_response, header_str, json_response, param, query_of, raw_response, read_body};
use super::circlesendprep::SendEnv;
use super::{adminblock, adminhold, adminpool, circleadmin, invites};
use axum::extract::{Request, State};
use axum::http::HeaderMap;
use axum::response::Response;
use serde_json::{Map, Value, json};
use std::sync::Arc;

/// (method, path, writes) of every admin route this module serves.
pub(super) const TABLE: &[(&str, &str, bool)] = &[
    ("GET", "/admin/pool/reports", false),
    ("GET", "/admin/pool/flags", false),
    ("GET", "/admin/pool/item", false),
    ("GET", "/admin/pool/key", false),
    ("POST", "/admin/pool/hold", true),
    ("POST", "/admin/pool/release", true),
    ("POST", "/admin/pool/release-hold", true),
    ("POST", "/admin/pool/dismiss", true),
    ("POST", "/admin/pool/remove", true),
    ("POST", "/admin/pool/unremove", true),
    ("POST", "/admin/pool/purge-preview", true),
    ("POST", "/admin/dmca/takedown-preview", true),
    ("POST", "/admin/pool/evidence", true),
    ("POST", "/admin/pool/unblock", true),
    ("POST", "/admin/dmca/unblock", true),
    ("POST", "/admin/pool/privacy-restore", true),
    ("POST", "/admin/pool/purge", true),
    ("POST", "/admin/pool/purge-held-copies", true),
    ("POST", "/admin/pool/appeal-decision", true),
    ("POST", "/admin/pool/resolve-flag", true),
    ("PUT", "/admin/pool/limits", true),
    ("GET", "/admin/dmca/notices", false),
    ("GET", "/admin/dmca/actions", false),
    ("GET", "/admin/dmca/counter-notices", false),
    ("POST", "/admin/dmca/notices", true),
    ("POST", "/admin/dmca/takedown", true),
    ("POST", "/admin/dmca/court-action", true),
    ("POST", "/admin/dmca/resolve", true),
    ("POST", "/admin/dmca/hold-copies-preview", true),
    ("POST", "/admin/dmca/hold-copies", true),
    ("GET", "/admin/dmca/copies", false),
    ("POST", "/admin/dmca/release-copies", true),
    ("POST", "/admin/dmca/exempt", true),
    ("POST", "/admin/dmca/withdraw", true),
    ("POST", "/admin/pool/invites", true),
    ("GET", "/admin/pool/invites", false),
    ("POST", "/admin/pool/invites/revoke", true),
    ("GET", "/admin/pool/circle-reports", false),
    ("GET", "/admin/pool/circles", false),
    ("GET", "/admin/pool/circle", false),
    ("GET", "/admin/pool/circle-bank", false),
    ("POST", "/admin/pool/circle-evidence-captured", true),
    ("POST", "/admin/pool/circle-hold", true),
    ("POST", "/admin/pool/circle-release", true),
    ("POST", "/admin/pool/circle-remove", true),
    ("POST", "/admin/pool/circle-close", true),
];

struct ACall<'a> {
    st: &'a PoolState,
    x: &'a PoolCtx<'a>,
    admin: &'a str,
    headers: &'a HeaderMap,
    query: Vec<(String, String)>,
    body: Map<String, Value>,
}

fn pair(r: (u16, Value)) -> Res<Reply> {
    Ok(Reply::Json(r.0, r.1))
}

fn dispatch(c: &ACall, method: &str, path: &str) -> Res<Reply> {
    let (x, a, b, st) = (c.x, c.admin, &c.body, c.st);
    match (method, path) {
        ("GET", "/admin/pool/reports") => Ok(Reply::Json(200, adminpool::list_reports(x, param(&c.query, "after").as_deref())?)),
        ("GET", "/admin/pool/flags") => {
            let limit = param(&c.query, "limit").map(|s| s.parse::<i64>().unwrap_or(0));
            Ok(Reply::Json(200, adminpool::list_flags(x, limit, param(&c.query, "cursor").as_deref())?))
        }
        ("GET", "/admin/pool/key") => Ok(Reply::Json(200, adminhold::pool_key(x, &c.st.fp.pool_key_id())?)),
        ("GET", "/admin/pool/item") => item(c),
        ("POST", "/admin/pool/hold") => pair(adminpool::hold(x, a, b)?),
        ("POST", "/admin/pool/release") => pair(adminpool::release(x, a, b)?),
        ("POST", "/admin/pool/release-hold") => pair(adminpool::release_hold(x, a, b)?),
        ("POST", "/admin/pool/dismiss") => pair(adminpool::dismiss(x, a, b)?),
        ("POST", "/admin/pool/remove") => pair(adminpool::remove(x, a, b)?),
        ("POST", "/admin/pool/unremove") => pair(adminpool::unremove(x, a, b)?),
        ("POST", "/admin/pool/purge-preview") => Ok(Reply::Json(200, adminblock::purge_preview(x, &st.keys, a, b)?)),
        ("POST", "/admin/dmca/takedown-preview") => Ok(Reply::Json(200, adminblock::takedown_preview(x, &st.keys, a, b)?)),
        ("POST", "/admin/pool/evidence") => pair(adminblock::evidence(x, a, b)?),
        ("POST", "/admin/pool/unblock") => pair(adminblock::privacy_unblock(x, &st.store, a, b)?),
        ("POST", "/admin/dmca/unblock") => {
            let env = SendEnv { x, core: &st.core, fp: &st.fp, keys: &st.keys, store: &st.store };
            pair(adminblock::dmca_unblock(&env, a, b)?)
        }
        ("POST", "/admin/pool/privacy-restore") => pair(adminblock::privacy_restore_route(x, &st.store, a, b)?),
        ("POST", "/admin/pool/purge") => pair(adminblock::purge_route(x, &st.store, &st.keys, a, b)?),
        ("POST", "/admin/pool/purge-held-copies") => Ok(Reply::Json(200, adminblock::purge_held_copies_route(x, &st.store, a, b)?)),
        ("POST", "/admin/pool/appeal-decision") => pair(adminblock::appeal_decision(x, &st.store, a, b)?),
        ("POST", "/admin/pool/resolve-flag") => pair(adminblock::resolve_flag(x, a, b)?),
        ("PUT", "/admin/pool/limits") => pair(adminblock::set_limit(x, a, b)?),
        ("GET", "/admin/dmca/notices" | "/admin/dmca/actions" | "/admin/dmca/counter-notices") => {
            let which = path.rsplit('/').next().unwrap_or("");
            let limit = param(&c.query, "limit").map(|s| s.parse::<i64>().unwrap_or(0));
            Ok(Reply::Json(200, adminblock::list_dmca(x, which, limit, param(&c.query, "cursor").as_deref())?))
        }
        ("POST", "/admin/dmca/notices") => pair(adminblock::create_notice(x, a, b)?),
        ("POST", "/admin/dmca/takedown") => pair(adminblock::takedown_route(x, &st.store, &st.keys, a, b)?),
        ("POST", "/admin/dmca/court-action") => pair(adminhold::court_action(x, a, b)?),
        ("POST", "/admin/dmca/resolve") => pair(adminhold::resolve(x, &st.store, a, b)?),
        ("POST", "/admin/dmca/hold-copies-preview") => pair(adminhold::hold_copies_preview(x, &st.keys, a, b)?),
        ("POST", "/admin/dmca/hold-copies") => pair(adminhold::hold_copies(x, &st.keys, a, b)?),
        ("GET", "/admin/dmca/copies") => {
            let q = |k: &str| param(&c.query, k);
            let aid = match q("action_id") {
                Some(s) if super::sqlfrag::is_id(&s) => s,
                _ => return Err(PoolError::new(400, "invalid_request").with("field", json!("action_id")).into()),
            };
            pair(adminhold::copies_of_action(x, Some(&aid), q("held_after").as_deref(), q("candidates_after").as_deref(), q("after").as_deref())?)
        }
        ("POST", "/admin/dmca/release-copies") => pair(adminhold::release_copies(x, a, b)?),
        ("POST", "/admin/dmca/exempt") => pair(adminhold::exempt(x, a, b)?),
        ("POST", "/admin/dmca/withdraw") => pair(adminhold::withdraw(x, &st.store, a, b)?),
        ("POST", "/admin/pool/invites") => pair(invites::issue_invite(x, a, b)?),
        ("GET", "/admin/pool/invites") => {
            let (id, user_id, after) = (param(&c.query, "id"), param(&c.query, "user_id"), param(&c.query, "after"));
            pair(invites::list_invites(x, &invites::InviteQuery { id: id.as_deref(), user_id: user_id.as_deref(), after: after.as_deref() })?)
        }
        ("POST", "/admin/pool/invites/revoke") => pair(invites::revoke_invite(x, a, b)?),
        ("GET", "/admin/pool/circle-reports") => Ok(Reply::Json(200, circleadmin::list_reports(x, param(&c.query, "after").as_deref())?)),
        ("GET", "/admin/pool/circles") => Ok(Reply::Json(200, circleadmin::list_circles(x, param(&c.query, "user_id").as_deref(), param(&c.query, "after").as_deref())?)),
        ("GET", "/admin/pool/circle") => pair(circleadmin::circle_detail(x, a, param(&c.query, "id").as_deref())?),
        ("GET", "/admin/pool/circle-bank") => {
            let q: Map<String, Value> = c.query.iter().map(|(k, v)| (k.clone(), Value::String(v.clone()))).collect();
            pair(circleadmin::bank_view(x, &st.store, a, &q)?)
        }
        ("POST", "/admin/pool/circle-evidence-captured") => pair(circleadmin::evidence_captured(x, a, b)?),
        ("POST", "/admin/pool/circle-hold") => pair(circleadmin::hold(x, a, b)?),
        ("POST", "/admin/pool/circle-release") => pair(circleadmin::release(x, a, b)?),
        ("POST", "/admin/pool/circle-remove") => pair(circleadmin::remove_bank(x, a, b)?),
        ("POST", "/admin/pool/circle-close") => pair(circleadmin::close_circle(x, a, b)?),
        _ => Ok(Reply::Json(501, json!({ "error": "not_ported" }))),
    }
}

/// GET /admin/pool/item?id=: the full form in any state, the stored body streamed in as `payload_full` under a read reservation.
fn item(c: &ACall) -> Res<Reply> {
    let id = match param(&c.query, "id") {
        Some(s) if super::sqlfrag::is_id(&s) => s,
        _ => return Err(PoolError::new(400, "invalid_request").with("field", json!("id")).into()),
    };
    let ip = client_ip(c.headers).map(|ip| crate::auth::guard::ip_bucket(&ip, c.st.ip_prefix)).unwrap_or_default();
    let slots = c.st.opens.hold(c.admin, &ip)?;
    let Some((head, key)) = adminpool::item_head(c.x, &id)? else { return Err(PoolError::new(404, "not_found").into()) };
    let bytes = key.and_then(|k| c.st.store.get(&k).ok().flatten());
    let Some(bytes) = bytes else {
        let mut h = head;
        h["payload_full"] = Value::Null;
        return Ok(Reply::Json(200, h));
    };
    let mut body = super::read::payload_prefix(&head.to_string(), "payload_full");
    body.extend_from_slice(&bytes);
    body.push(b'}');
    Ok(Reply::Raw { status: 200, headers: vec![("Content-Type".into(), "application/json".into())], body, hold: Some(slots) })
}

pub async fn admin_handle(State(st): State<Arc<PoolState>>, req: Request) -> Response {
    let (parts, body) = req.into_parts();
    let (method, path) = (parts.method.as_str().to_string(), parts.uri.path().to_string());
    let Some(&(_, _, write)) = TABLE.iter().find(|(m, p, _)| *m == method && *p == path) else {
        let any = TABLE.iter().any(|(_, p, _)| *p == path);
        return json_response(if any { 405 } else { 404 }, &json!({ "error": if any { "method_not_allowed" } else { "not_found" } }));
    };
    let guarded = {
        let (st, headers) = (st.clone(), parts.headers.clone());
        tokio::task::spawn_blocking(move || st.admin.admin(&headers, write)).await
    };
    let admin = match guarded {
        Ok(Ok(a)) => a,
        Ok(Err(e)) => return fail_response(Fail::Pool(e)),
        Err(_) => return json_response(500, &json!({ "error": "server_error" })),
    };
    let bytes = if write {
        match read_body(&st, &parts.headers, body, ADMIN_BODY_MAX, false).await {
            Ok(b) => b,
            Err(f) => return fail_response(f),
        }
    } else {
        vec![]
    };
    let query = query_of(parts.uri.query());
    let done = {
        let st = st.clone();
        let headers = parts.headers.clone();
        tokio::task::spawn_blocking(move || -> Res<Reply> {
            let json_body = if write { super::poolio::read_json_max(header_str(&headers, "content-type"), header_str(&headers, "content-length"), &bytes, false, ADMIN_BODY_MAX)? } else { Map::new() };
            let conn = st.conn.lock().unwrap_or_else(|p| p.into_inner());
            let x = PoolCtx::new(&conn, (st.clock)());
            // No admin write under a key other than the pool's own.
            if write {
                check_key(&x, &st.store, &st.core, &st.fp)?;
            }
            let call = ACall { st: &st, x: &x, admin: &admin, headers: &headers, query, body: json_body };
            let r = dispatch(&call, &method, &path);
            (st.notify)(x.take_notices());
            r
        })
        .await
    };
    match done {
        Ok(Ok(Reply::Json(s, v))) => json_response(s, &v),
        Ok(Ok(Reply::Raw { status, headers, body, hold })) => raw_response(status, headers, body, hold),
        Ok(Err(f)) => fail_response(f),
        Err(_) => json_response(500, &json!({ "error": "server_error" })),
    }
}
