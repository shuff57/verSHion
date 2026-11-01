//! The pool's axum layer (worker/src/pool/index.ts `createPool.handle`): finds the route, authenticates, runs the gate, reads the body the handler wants,
//! calls the HTTP-free core in the Worker's order and writes the reply. Handlers that are not wired yet answer 501 `not_ported`.
use super::core::Core;
use super::ctx::{Fail, PoolCtx, PoolError, PoolNotice, Res};
use super::fingerprint::Fingerprinter;
use super::gate::{Caller, Gate, Req, gate};
use super::list::{ListQuery, SEARCH_CANDIDATES, list};
use super::poolio::{JSON_BODY_MAX, read_json, read_json_max};
use super::poolkeys::PoolKeys;
use super::image::MAX_IMAGE_BYTES;
use super::export::{EXPORT_LIMITS, ExportWho, export_account};
use super::circlepurge::export_circles;
use super::read::{ReadOut, Reader};
use super::publishonce::{Env, MAX_META, MAX_PAYLOAD, publish};
use super::publishrun::HeavySlots;
use super::readslots::{OpenReads, ReadSlots};
use super::routetable::{Route, route_of};
use crate::auth::crypto::Key;
use crate::auth::guard::ip_bucket;
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
/// Who may call the admin routes: an admin account on a known device, and for writes a sudo token (the auth package's guard). Returns the admin's id.
pub trait PoolAdminGuard: Send + Sync {
    fn admin(&self, headers: &HeaderMap, write: bool) -> Result<String, PoolError>;
}

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
    pub admin: Box<dyn PoolAdminGuard>,
    pub clock: Arc<dyn Fn() -> i64 + Send + Sync>,
    pub heavy: HeavySlots,
    pub opens: OpenReads,
    pub heavy_wait: Duration,
    pub body_deadline: Duration,
    /// Delivers the notices a request queued (after the reply is decided).
    pub notify: Arc<dyn Fn(Vec<PoolNotice>) + Send + Sync>,
}

/// A handler's answer.
pub enum Reply {
    Json(u16, Value),
    /// Bytes with their own headers; `hold` keeps the open-read slots until the body has been sent or dropped.
    Raw { status: u16, headers: Vec<(String, String)>, body: Vec<u8>, hold: Option<ReadSlots> },
}

/// A body that owns the read slots of the stream it carries.
struct Held {
    inner: http_body_util::Full<axum::body::Bytes>,
    _hold: Option<ReadSlots>,
}
impl axum::body::HttpBody for Held {
    type Data = axum::body::Bytes;
    type Error = std::convert::Infallible;
    fn poll_frame(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        std::pin::Pin::new(&mut self.inner).poll_frame(cx)
    }
    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }
    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.size_hint()
    }
}

pub(super) fn raw_response(status: u16, headers: Vec<(String, String)>, body: Vec<u8>, hold: Option<ReadSlots>) -> Response {
    let mut r = Response::new(Body::new(Held { inner: http_body_util::Full::new(body.into()), _hold: hold }));
    *r.status_mut() = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    for (k, v) in headers {
        if let (Ok(k), Ok(v)) = (HeaderName::try_from(k.as_str()), HeaderValue::try_from(v.as_str())) {
            r.headers_mut().insert(k, v);
        }
    }
    r
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
pub(super) fn fail_response(f: Fail) -> Response {
    match f {
        Fail::Pool(e) => pool_err(e),
        Fail::Db(e) => {
            tracing::error!("pool: database error: {e}");
            json_response(500, &json!({ "error": "server_error" }))
        }
    }
}
pub(super) fn json_response(status: u16, v: &Value) -> Response {
    let mut r = Response::new(Body::from(v.to_string()));
    *r.status_mut() = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    r.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json"));
    r
}

pub(super) fn header_str<'a>(h: &'a HeaderMap, name: &str) -> Option<&'a str> {
    h.get(name).and_then(|v| v.to_str().ok())
}
/// The last `X-Forwarded-For` entry (the proxy's own); None when absent or blank.
pub(super) fn client_ip(h: &HeaderMap) -> Option<String> {
    header_str(h, "x-forwarded-for").and_then(|s| s.rsplit(',').next()).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

/// The request body: Content-Length required and exact, within `max` and the deadline.
pub(super) async fn read_body(st: &PoolState, h: &HeaderMap, body: Body, max: usize, optional: bool) -> Res<Vec<u8>> {
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

pub(super) fn query_of(q: Option<&str>) -> Vec<(String, String)> {
    url::form_urlencoded::parse(q.unwrap_or("").as_bytes()).map(|(k, v)| (k.into_owned(), v.into_owned())).collect()
}
pub(super) fn param(q: &[(String, String)], k: &str) -> Option<String> {
    q.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone())
}

/// What a handler needs of the request.
struct Call<'a> {
    st: &'a PoolState,
    trust: &'a str,
    family: Option<&'a str>,
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
    /// A body that may be absent (an import, a versions list).
    fn json_opt(&self) -> Res<Map<String, Value>> {
        read_json(header_str(self.headers, "content-type"), header_str(self.headers, "content-length"), &self.body, true)
    }
    fn addr(&self) -> String {
        client_ip(self.headers).map(|ip| ip_bucket(&ip, self.st.ip_prefix)).unwrap_or_default()
    }
}

/// A bank PATCH's size at most (strip decisions; the size of a send's meta).
const PATCH_BODY_MAX: usize = 1024 * 1024;

/// How much of the body the route reads.
fn body_limit(route: &Route, method: &str) -> Option<(usize, bool)> {
    use Route::*;
    match (route, method) {
        (Items | ItemsPreview | ItemPreview(_) | Banks(_), "POST") | (Item(_), "PUT" ) | (Bank(..), "PUT") => Some((MAX_PAYLOAD + MAX_META + 1, false)),
        (BlobPut(_), "PUT") => Some((MAX_IMAGE_BYTES, false)),
        (_, "GET" | "HEAD" | "DELETE") => None,
        (ItemImport(_), "POST") => Some((JSON_BODY_MAX, true)),
        (MeInvite, "POST") => Some((JSON_BODY_MAX, false)),
        (CircleHeads, "POST") => Some((JSON_BODY_MAX, true)),
        (Bank(..), "PATCH") => Some((PATCH_BODY_MAX, false)),
        (BankReport(..) | BankMerge(..) | BankRollback(..) | BankTrim(..), "POST") => Some((JSON_BODY_MAX, false)),
        (Circles | CircleJoinPreview | CircleJoin | Circle(_) | CircleInvites(_) | CircleInvitesRevoke(_) | CircleInvitesEmail(_) | CircleMembersRemove(_) | CircleMembersRole(_) | CircleTransfer(_), "POST" | "PATCH") => Some((JSON_BODY_MAX, false)),
        (Prefs | ItemVersions | Item(_) | ItemReport(_) | ItemAppeal(_) | ItemCounterNotice(_), _) => Some((JSON_BODY_MAX, false)),
        _ => None,
    }
}

fn dispatch(c: &Call, route: &Route, enter_heavy: &mut dyn FnMut() -> Res<()>) -> Res<Reply> {
    use super::{blobs, moderation as m, poolcontent as pc, readmisc};
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
        (Route::Item(id), "GET" | "HEAD") => {
            let rd = Reader { x, store: &c.st.store, opens: &c.st.opens, me, addr: &c.addr() };
            let inm = header_str(c.headers, "if-none-match");
            read_reply(rd.get_item(id, c.method == "HEAD", inm)?)
        }
        (Route::ItemImport(id), "POST") => {
            c.json_opt()?;
            let rd = Reader { x, store: &c.st.store, opens: &c.st.opens, me, addr: &c.addr() };
            read_reply(rd.import_item(id)?)
        }
        (Route::ItemBlob(id, sha), "GET") => {
            let b = readmisc::get_blob(x, &c.st.store, me, id, sha)?;
            let headers = readmisc::blob_headers(&b).into_iter().map(|(k, v)| (k.to_string(), v)).collect();
            Ok(Reply::Raw { status: 200, headers, body: b.bytes, hold: None })
        }
        (Route::BlobPut(sha), "PUT") => {
            let s = blobs::store_blob(x, &c.st.store, me, sha, &c.body)?;
            Ok(Reply::Json(201, json!({ "sha256": s.sha256, "size": s.size, "width": s.width, "height": s.height })))
        }
        (Route::MeInvite, "POST") => Ok(Reply::Json(200, super::invites::redeem_invite(x, me, &c.json()?)?)),
        (Route::MeExport, "GET") => {
            let who = ExportWho { user_id: me, trust: c.trust, session_family: c.family };
            let part = param(&c.query, "part");
            let out = export_account(x, &c.st.store, &who, part.as_deref(), EXPORT_LIMITS, &|x, me| Ok(export_circles(x, me)?))?;
            let headers = vec![
                ("Content-Type".to_string(), "application/json".to_string()),
                ("Content-Disposition".to_string(), format!("attachment; filename=\"{}\"", out.filename)),
                ("Cache-Control".to_string(), "no-store".to_string()),
                ("X-Content-Type-Options".to_string(), "nosniff".to_string()),
            ];
            Ok(Reply::Raw { status: 200, headers, body: out.body.to_string().into_bytes(), hold: None })
        }
        (r, _) if circle_route(r) => circles_dispatch(c, r),
        (r, _) if bank_route(r) => banks_dispatch(c, r, enter_heavy),
        _ => not_ported(),
    }
}

/// The circle routes that are not about banks.
fn circle_route(r: &Route) -> bool {
    use Route::*;
    matches!(
        r,
        Circles | CircleJoinPreview | CircleJoin | CircleHeads | Circle(_) | CircleReopen(_) | CircleActivity(_) | CircleInvites(_) | CircleInvitesRevoke(_) | CircleInvitesEmail(_) | CircleLeave(_) | CircleMembersRemove(_)
            | CircleMembersRole(_) | CircleTransfer(_) | CircleTransferAccept(_) | CircleTransferCancel(_)
    )
}

fn circles_dispatch(c: &Call, route: &Route) -> Res<Reply> {
    use super::{circleactivity as act, circleinvites as inv, circlemail, circlemembers as mem, circles as cir};
    let (x, me) = (c.x, c.me);
    let pair = |r: (u16, Value)| Ok(Reply::Json(r.0, r.1));
    let ok = |v: Value| Ok(Reply::Json(200, v));
    match (route, c.method) {
        (Route::Circles, "GET") => ok(cir::list_circles(x, me)?),
        (Route::Circles, "POST") => pair(cir::create_circle(x, me, &c.json()?)?),
        (Route::CircleJoinPreview, "POST") => ok(inv::join_preview(x, me, &c.json()?)?),
        (Route::CircleJoin, "POST") => ok(inv::join_circle(x, me, &c.json()?)?),
        (Route::CircleHeads, "POST") => ok(act::heads(x, me, &c.json_opt()?)?),
        (Route::Circle(id), "GET") => ok(cir::get_circle(x, me, id)?),
        (Route::Circle(id), "PATCH") => ok(cir::patch_circle(x, me, id, &c.json()?)?),
        (Route::Circle(id), "DELETE") => ok(cir::close_circle(x, me, id)?),
        (Route::CircleReopen(id), "POST") => ok(cir::reopen_circle(x, me, id)?),
        (Route::CircleActivity(id), "GET") => ok(act::activity(x, me, id, param(&c.query, "before").as_deref())?),
        (Route::CircleInvites(id), "GET") => ok(inv::list_invites(x, me, id, param(&c.query, "after").as_deref())?),
        (Route::CircleInvites(id), "POST") => pair(inv::create_invite(x, me, id, &c.json()?)?),
        (Route::CircleInvitesRevoke(id), "POST") => ok(inv::revoke_invite(x, me, id, &c.json()?)?),
        (Route::CircleInvitesEmail(id), "POST") => pair(circlemail::email_invite(x, &c.st.keys, me, id, &c.json()?)?),
        (Route::CircleLeave(id), "POST") => ok(mem::leave_circle(x, me, id)?),
        (Route::CircleMembersRemove(id), "POST") => ok(mem::remove_member(x, me, id, &c.json()?)?),
        (Route::CircleMembersRole(id), "POST") => ok(mem::set_role(x, me, id, &c.json()?)?),
        (Route::CircleTransfer(id), "POST") => pair(mem::offer_transfer(x, me, id, &c.json()?)?),
        (Route::CircleTransferAccept(id), "POST") => ok(mem::accept_transfer(x, me, id)?),
        (Route::CircleTransferCancel(id), "POST") => ok(mem::cancel_transfer(x, me, id)?),
        _ => Ok(Reply::Json(501, json!({ "error": "not_ported" }))),
    }
}

/// GET / HEAD of a bank's head version, or of version `n`: the head JSON with the stored payload spliced in (stripped for a viewer of a bank that hides
/// its keys), an ETag, and the HEAD and 304 forms.
fn bank_get(c: &Call, env: &super::circlesendprep::SendEnv, cid: &str, bid: &str, version: Option<i64>) -> Res<Reply> {
    use super::{circlebanks, circlehide, circlehidepatch};
    let (x, me) = (c.x, c.me);
    let head_only = c.method == "HEAD";
    let slots = c.st.opens.hold(me, &c.addr())?;
    let r = circlebanks::bank_read(x, me, cid, bid, version, &|me, bid, vid| circlehidepatch::hide_off_releases(env, me, bid, vid))?;
    let prefix = super::read::payload_prefix(&r.head, "payload");
    let inm = header_str(c.headers, "if-none-match");
    if head_only || inm.is_some_and(|h| super::read::if_none_match_hits(h, &r.etag)) {
        let Some(size) = c.st.store.len(&r.full_key).ok().flatten() else { return Err(PoolError::new(404, "not_found").into()) };
        let mut headers = vec![("ETag".to_string(), r.etag.clone())];
        // A stripped body's length is not known without stripping it: a viewer's HEAD has none.
        if head_only && !r.hidden {
            headers.push(("Content-Length".into(), (prefix.len() as u64 + size + 1).to_string()));
        }
        return Ok(Reply::Raw { status: if head_only { 200 } else { 304 }, headers, body: vec![], hold: None });
    }
    let payload = if r.hidden {
        circlehide::stripped_for_viewer(x, &c.st.core, &c.st.store, me, &r.full_key, r.strip_opts.as_deref())?
    } else {
        match c.st.store.get(&r.full_key).ok().flatten() {
            Some(b) => b,
            None => return Err(PoolError::new(404, "not_found").into()),
        }
    };
    let mut body = prefix;
    body.extend_from_slice(&payload);
    body.push(b'}');
    let headers = vec![("Content-Type".to_string(), "application/json".to_string()), ("ETag".into(), r.etag)];
    Ok(Reply::Raw { status: 200, headers, body, hold: Some(slots) })
}

/// The bank routes that carry no payload to serve or send.
fn bank_route(r: &Route) -> bool {
    use Route::*;
    matches!(r, Banks(..) | BankVersion(..) | Bank(..) | BankUndelete(..) | BankReport(..) | BankPurge(..) | BankBlob(..) | BankVersions(..) | BankChanges(..) | BankMerge(..) | BankRollback(..) | BankTrim(..))
}

fn banks_dispatch(c: &Call, route: &Route, enter_heavy: &mut dyn FnMut() -> Res<()>) -> Res<Reply> {
    use super::circlesendprep::SendEnv;
    use super::{circlebankedit as edit, circlebankstate as state, circlehidepatch, circlepurge, circlerollback, circleversions as ver};
    let (x, me) = (c.x, c.me);
    let ok = |v: Value| Ok(Reply::Json(200, v));
    let pair = |r: (u16, Value)| Ok(Reply::Json(r.0, r.1));
    let env = || SendEnv { x, core: &c.st.core, fp: &c.st.fp, keys: &c.st.keys, store: &c.st.store };
    match (route, c.method) {
        (Route::Banks(cid), "POST") | (Route::Bank(cid, _), "PUT") => {
            let bid = if let Route::Bank(_, b) = route { Some(b.as_str()) } else { None };
            
            pair(super::circlesend::send_version(&env(), me, cid, bid, header_str(c.headers, "content-type"), &c.body, enter_heavy)?)
        }
        (Route::Bank(cid, bid), "GET" | "HEAD") => bank_get(c, &env(), cid, bid, None),
        (Route::BankVersion(cid, bid, n), "GET") => bank_get(c, &env(), cid, bid, n.parse().ok().or(Some(0))),
        (Route::Bank(cid, bid), "DELETE") => ok(state::delete_bank(x, me, cid, bid)?),
        (Route::BankUndelete(cid, bid), "POST") => ok(state::undelete_bank(x, me, cid, bid)?),
        (Route::BankReport(cid, bid), "POST") => pair(state::report_bank(x, me, cid, bid, &c.json()?)?),
        (Route::BankPurge(cid, bid), "POST") => ok(circlepurge::purge_bank_now(x, &c.st.store, me, cid, bid)?),
        (Route::Bank(cid, bid), "PATCH") => {
            let (m, b) = edit::patch_gate(x, me, cid, bid)?;
            let body = read_json_max(header_str(c.headers, "content-type"), header_str(c.headers, "content-length"), &c.body, false, PATCH_BODY_MAX)?;
            ok(circlehidepatch::patch_bank(&env(), me, cid, &m, &b, &body)?)
        }
        (Route::BankBlob(cid, bid, sha), "GET") => {
            let ct = edit::bank_blob_type(x, me, cid, bid, sha)?;
            let Some(bytes) = c.st.store.get(&format!("blobs/{sha}")).ok().flatten() else { return Err(PoolError::new(404, "not_found").into()) };
            let headers = edit::blob_headers(&ct, sha).into_iter().map(|(k, v)| (k.to_string(), v)).collect();
            Ok(Reply::Raw { status: 200, headers, body: bytes, hold: None })
        }
        (Route::BankVersions(cid, bid), "GET") => ok(ver::history(x, me, cid, bid, param(&c.query, "before").as_deref())?),
        (Route::BankChanges(cid, bid), "GET") => match ver::changes(x, &c.st.store, me, cid, bid, header_str(c.headers, "if-none-match"))? {
            ver::Changes::NotModified(etag) => Ok(Reply::Raw { status: 304, headers: vec![("ETag".into(), etag)], body: vec![], hold: None }),
            ver::Changes::Body(v, etag) => {
                let mut headers = vec![("Content-Type".to_string(), "application/json".to_string())];
                if !etag.is_empty() {
                    headers.push(("ETag".into(), etag));
                }
                Ok(Reply::Raw { status: 200, headers, body: v.to_string().into_bytes(), hold: None })
            }
        },
        (Route::BankMerge(cid, bid), "POST") => ok(ver::merge_initiate(x, &c.st.keys, me, cid, bid, &c.json()?)?),
        (Route::BankTrim(cid, bid), "POST") => ok(ver::trim_versions(x, me, cid, bid, &c.json()?)?),
        (Route::BankRollback(cid, bid), "POST") => pair(circlerollback::rollback_bank(&env(), me, cid, bid, &c.json()?)?),
        _ => Ok(Reply::Json(501, json!({ "error": "not_ported" }))),
    }
}

fn read_reply(out: ReadOut) -> Res<Reply> {
    Ok(match out {
        ReadOut::Head { etag, content_length } => Reply::Raw { status: 200, headers: vec![("ETag".into(), etag), ("Content-Length".into(), content_length.to_string())], body: vec![], hold: None },
        ReadOut::NotModified { etag } => Reply::Raw { status: 304, headers: vec![("ETag".into(), etag)], body: vec![], hold: None },
        ReadOut::Body(s) => {
            let mut headers = vec![("Content-Type".to_string(), "application/json".to_string())];
            if let Some(e) = &s.etag {
                headers.push(("ETag".into(), e.clone()));
            }
            let body = s.bytes();
            Reply::Raw { status: 200, headers, body, hold: Some(s.slots) }
        }
    })
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
    if let (Route::BlobPut(sha), "PUT") = (&route, method.as_str()) {
        // Charged BEFORE the body is read: the day's uploads and the declared bytes.
        let (st2, sha, cl, me2) = (st.clone(), sha.clone(), header_str(&parts.headers, "content-length").map(str::to_string), me.user_id.clone());
        let pre = tokio::task::spawn_blocking(move || {
            let conn = st2.conn.lock().unwrap_or_else(|p| p.into_inner());
            let x = PoolCtx::new(&conn, (st2.clock)());
            super::blobs::preflight(&x, &me2, &sha, cl.as_deref()).map(|_| ())
        })
        .await;
        match pre {
            Ok(Ok(())) => {}
            Ok(Err(f)) => return fail_response(f),
            Err(_) => return json_response(500, &json!({ "error": "server_error" })),
        }
    }
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
            let call = Call { st: &st, trust: &me.trust, family: me.family.as_deref(), x: &x, me: &me.user_id, method: &method, query, headers: &headers, body: bytes };
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
                Ok(Reply::Raw { status, headers, body, hold }) => raw_response(status, headers, body, hold),
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
        .route("/admin/pool/{*rest}", any(super::adminroutes::admin_handle))
        .route("/admin/dmca/{*rest}", any(super::adminroutes::admin_handle))
        .with_state(st)
}

#[cfg(test)]
#[path = "routes_tests.rs"]
mod tests;
