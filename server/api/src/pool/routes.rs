//! The pool's axum layer (worker/src/pool/index.ts `createPool.handle`): finds the route, authenticates, runs the gate, reads the body the handler wants,
//! calls the HTTP-free core in the Worker's order and writes the reply. Handlers that are not wired yet answer 501 `not_ported`.
use super::core::Core;
use super::ctx::{Fail, PoolCtx, PoolError, PoolNotice, Res};
use super::fingerprint::Fingerprinter;
use super::gate::{Caller, Gate, Req, gate};
use super::list::{ListQuery, SEARCH_CANDIDATES, list};
use super::poolio::{JSON_BODY_MAX, read_json};
use super::poolkeys::PoolKeys;
use super::publishonce::{Env, MAX_META, MAX_PAYLOAD, publish};
use super::publishrun::HeavySlots;
use super::routetable::{Route, route_of};
use crate::auth::crypto::Key;
use crate::blobs::BlobStore;
use axum::body::{Body, to_bytes};
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::response::Response;
use axum::routing::any;
use axum::Router;
use rusqlite::Connection;
use serde_json::{Map, Value, json};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Who is signed in: the caller with the trust level as it is now in the database.
pub trait PoolAuth: Send + Sync {
    fn caller(&self, headers: &HeaderMap) -> Option<Caller>;
}

pub struct PoolState {
    pub conn: Mutex<Connection>,
    pub store: BlobStore,
    pub core: Core,
    pub fp: Fingerprinter,
    pub keys: PoolKeys,
    pub limit_key: Key,
    pub ip_prefix: usize,
    pub allowed_origins: Vec<String>,
    pub auth: Box<dyn PoolAuth>,
    pub clock: Arc<dyn Fn() -> i64 + Send + Sync>,
    pub heavy: HeavySlots,
    pub heavy_wait: Duration,
    pub body_deadline: Duration,
    /// Delivers the notices a request queued (after the reply is decided).
    pub notify: Arc<dyn Fn(Vec<PoolNotice>) + Send + Sync>,
}

/// A handler's answer.
pub enum Reply {
    Json(u16, Value),
}

fn pool_err(e: PoolError) -> Response {
    let mut body = Map::new();
    body.insert("error".into(), json!(e.code));
    for (k, v) in &e.extra {
        body.insert(k.clone(), v.clone());
    }
    let mut r = json_response(e.status, &Value::Object(body));
    for (k, v) in &e.headers {
        if let (Ok(k), Ok(v)) = (HeaderName::try_from(k.as_str()), HeaderValue::try_from(v.as_str())) {
            r.headers_mut().insert(k, v);
        }
    }
    r
}
fn fail_response(f: Fail) -> Response {
    match f {
        Fail::Pool(e) => pool_err(e),
        Fail::Db(e) => {
            tracing::error!("pool: database error: {e}");
            json_response(500, &json!({ "error": "server_error" }))
        }
    }
}
fn json_response(status: u16, v: &Value) -> Response {
    let mut r = Response::new(Body::from(v.to_string()));
    *r.status_mut() = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    r.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json"));
    r
}

fn header_str<'a>(h: &'a HeaderMap, name: &str) -> Option<&'a str> {
    h.get(name).and_then(|v| v.to_str().ok())
}
/// The last `X-Forwarded-For` entry (the proxy's own); None when absent or blank.
fn client_ip(h: &HeaderMap) -> Option<String> {
    header_str(h, "x-forwarded-for").and_then(|s| s.rsplit(',').next()).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

/// The request body: Content-Length required and exact, within `max` and the deadline.
async fn read_body(st: &PoolState, h: &HeaderMap, body: Body, max: usize, optional: bool) -> Res<Vec<u8>> {
    let Some(declared) = header_str(h, "content-length") else {
        if optional {
            return Ok(vec![]);
        }
        return Err(PoolError::new(411, "length_required").with("message", json!("Send the body with a Content-Length.")).into());
    };
    if declared.is_empty() || declared.len() > 12 || !declared.bytes().all(|b| b.is_ascii_digit()) {
        return Err(PoolError::new(400, "invalid_request").with("field", json!("Content-Length")).into());
    }
    let n: usize = declared.parse().unwrap_or(usize::MAX);
    if n > max {
        return Err(PoolError::new(413, "body_too_large").with("limit", json!(max)).into());
    }
    if n == 0 {
        return Ok(vec![]);
    }
    let bytes = match tokio::time::timeout(st.body_deadline, to_bytes(body, max)).await {
        Err(_) => return Err(PoolError::new(408, "request_timeout").into()),
        Ok(Err(_)) => return Err(PoolError::new(400, "invalid_request").into()),
        Ok(Ok(b)) => b,
    };
    if bytes.len() != n {
        return Err(PoolError::new(400, "length_mismatch").into());
    }
    Ok(bytes.to_vec())
}

fn query_of(q: Option<&str>) -> Vec<(String, String)> {
    url::form_urlencoded::parse(q.unwrap_or("").as_bytes()).map(|(k, v)| (k.into_owned(), v.into_owned())).collect()
}
fn param(q: &[(String, String)], k: &str) -> Option<String> {
    q.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone())
}

/// What a handler needs of the request.
struct Call<'a> {
    st: &'a PoolState,
    x: &'a PoolCtx<'a>,
    me: &'a str,
    method: &'a str,
    query: Vec<(String, String)>,
    headers: &'a HeaderMap,
    body: Vec<u8>,
}

impl Call<'_> {
    fn json(&self) -> Res<Map<String, Value>> {
        read_json(header_str(self.headers, "content-type"), header_str(self.headers, "content-length"), &self.body, false)
    }
}

/// How much of the body the route reads.
fn body_limit(route: &Route, method: &str) -> Option<(usize, bool)> {
    use Route::*;
    match (route, method) {
        (Items | ItemsPreview | ItemPreview(_), "POST") | (Item(_), "PUT") => Some((MAX_PAYLOAD + MAX_META + 1, false)),
        (_, "GET" | "HEAD" | "DELETE") => None,
        (Prefs | ItemVersions | Item(_) | ItemReport(_) | ItemAppeal(_) | ItemCounterNotice(_), _) => Some((JSON_BODY_MAX, false)),
        _ => None,
    }
}

fn dispatch(c: &Call, route: &Route, enter_heavy: &mut dyn FnMut() -> Res<()>) -> Res<Reply> {
    use super::{moderation as m, poolcontent as pc, readmisc};
    let (x, me) = (c.x, c.me);
    let not_ported = || Ok(Reply::Json(501, json!({ "error": "not_ported" })));
    match (route, c.method) {
        (Route::Items, "GET") => {
            let q = ListQuery {
                scope: param(&c.query, "scope"),
                limit: param(&c.query, "limit"),
                subject: param(&c.query, "subject"),
                kind: param(&c.query, "kind"),
                tag: param(&c.query, "tag"),
                standard: param(&c.query, "standard"),
                q: param(&c.query, "q"),
                license: param(&c.query, "license"),
                cursor: param(&c.query, "cursor"),
            };
            Ok(Reply::Json(200, list(x, &c.st.keys, me, &q, SEARCH_CANDIDATES)?))
        }
        (Route::Items, "POST") | (Route::ItemsPreview, "POST") | (Route::ItemPreview(_), "POST") | (Route::Item(_), "PUT") => {
            let (path_id, dry) = match route {
                Route::ItemPreview(id) => (Some(id.as_str()), true),
                Route::Item(id) => (Some(id.as_str()), false),
                Route::ItemsPreview => (None, true),
                _ => (None, false),
            };
            let env = Env { store: &c.st.store, core: &c.st.core, fp: &c.st.fp };
            let o = publish(x, &env, me, path_id, header_str(c.headers, "content-type"), &c.body, dry, enter_heavy)?;
            Ok(Reply::Json(o.status, o.body))
        }
        (Route::Mine, "GET") => Ok(Reply::Json(200, super::mine::mine(x, me, param(&c.query, "after").as_deref())?)),
        (Route::Prefs, "GET") => Ok(Reply::Json(200, pc::get_prefs(x.conn, me)?)),
        (Route::Prefs, "PUT") => Ok(Reply::Json(200, pc::put_prefs(x.conn, me, x.now, &c.json()?)?)),
        (Route::ItemVersions, "POST") => {
            let b = c.json()?;
            let ids = readmisc::check_ids(b.get("ids"))?;
            Ok(Reply::Json(200, readmisc::versions(x, me, &ids)?))
        }
        (Route::Item(id), "PATCH") => {
            let (s, v) = m::patch_item(x, me, id, &c.json()?)?;
            Ok(Reply::Json(s, v))
        }
        (Route::Item(id), "DELETE") => {
            let (s, v) = m::delete_item(x, me, id)?;
            Ok(Reply::Json(s, v))
        }
        (Route::ItemUndelete(id), "POST") => {
            let (s, v) = m::undelete_item(x, me, id)?;
            Ok(Reply::Json(s, v))
        }
        (Route::ItemReport(id), "POST") => {
            let (s, v) = m::report(x, &c.st.store, me, id, &c.json()?)?;
            Ok(Reply::Json(s, v))
        }
        (Route::ItemAppeal(id), "POST") => {
            let (s, v) = m::appeal(x, me, id, &c.json()?)?;
            Ok(Reply::Json(s, v))
        }
        (Route::ItemCounterNotice(id), "POST") => {
            let (s, v) = m::counter_notice(x, me, id, &c.json()?)?;
            Ok(Reply::Json(s, v))
        }
        _ => not_ported(),
    }
}

async fn handle(State(st): State<Arc<PoolState>>, req: Request) -> Response {
    let (parts, body) = req.into_parts();
    let (method, path) = (parts.method.as_str().to_string(), parts.uri.path().to_string());
    let Some(route) = route_of(&path) else {
        return json_response(404, &json!({ "error": "not_found" }));
    };
    // The gate, on a blocking thread (the database is synchronous).
    let gated = {
        let (st, route, method, path, headers) = (st.clone(), route.clone(), method.clone(), path.clone(), parts.headers.clone());
        tokio::task::spawn_blocking(move || {
            let conn = st.conn.lock().unwrap_or_else(|p| p.into_inner());
            let x = PoolCtx::new(&conn, (st.clock)());
            let g = Gate { allowed_origins: &st.allowed_origins, limit_key: &st.limit_key, ip_prefix: st.ip_prefix, store: &st.store, core: &st.core, fp: &st.fp };
            let ip = client_ip(&headers);
            let req = Req { method: &method, path: &path, origin: header_str(&headers, "origin"), x_client: header_str(&headers, "x-client"), ip: ip.as_deref() };
            let caller = st.auth.caller(&headers);
            let r = gate(&g, &x, &route, &req, caller.as_ref());
            (r, x.take_notices())
        })
        .await
    };
    let me = match gated {
        Ok((Ok(c), n)) => {
            (st.notify)(n);
            c
        }
        Ok((Err(f), n)) => {
            (st.notify)(n);
            return fail_response(f);
        }
        Err(_) => return json_response(500, &json!({ "error": "server_error" })),
    };
    let bytes = match body_limit(&route, &method) {
        Some((max, optional)) => match read_body(&st, &parts.headers, body, max, optional).await {
            Ok(b) => b,
            Err(f) => return fail_response(f),
        },
        None => vec![],
    };
    let handle = tokio::runtime::Handle::current();
    let done = {
        let (st, headers, query) = (st.clone(), parts.headers.clone(), query_of(parts.uri.query()));
        tokio::task::spawn_blocking(move || {
            let conn = st.conn.lock().unwrap_or_else(|p| p.into_inner());
            let x = PoolCtx::new(&conn, (st.clock)());
            let mut held = None;
            let call = Call { st: &st, x: &x, me: &me.user_id, method: &method, query, headers: &headers, body: bytes };
            let mut enter = || -> Res<()> {
                held = Some(handle.block_on(st.heavy.acquire(st.heavy_wait, || {}))?);
                Ok(())
            };
            let r = dispatch(&call, &route, &mut enter);
            drop(held);
            (r, x.take_notices())
        })
        .await
    };
    match done {
        Ok((r, n)) => {
            (st.notify)(n);
            match r {
                Ok(Reply::Json(s, v)) => json_response(s, &v),
                Err(f) => fail_response(f),
            }
        }
        Err(_) => json_response(500, &json!({ "error": "server_error" })),
    }
}

pub fn router(st: Arc<PoolState>) -> Router {
    Router::new()
        .route("/items", any(handle))
        .route("/items/{*rest}", any(handle))
        .route("/blobs/{*rest}", any(handle))
        .route("/me/export", any(handle))
        .route("/me/invite", any(handle))
        .route("/circles", any(handle))
        .route("/circles/{*rest}", any(handle))
        .with_state(st)
}

#[cfg(test)]
#[path = "routes_tests.rs"]
mod tests;
