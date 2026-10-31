//! The auth routes over HTTP (port of `handle` in auth/src/auth.ts): path and method lookup, CORS, client mode, body
//! parsing, the handler on a blocking thread (password hashing must not stall the runtime), and the mail the handler
//! queued, sent after the reply so a slow mail server can neither delay nor change an answer.
use super::admin_read::{self, DomainLabels};
use super::config::{AuthConfig, PATHS};
use super::crypto::{Keys, derive_keys};
use super::http::{ClientInfo, HttpError, MAX_BODY_BYTES, client_info, cors_headers, error_response, json, parse_json, preflight, with_headers};
use super::mail::{MailKind, MailMessage, Mailer, Outbox};
use super::notices::{AdminEvent, Notice, admin_alert, new_application_budget};
use super::session::Ctx;
use super::smtp::{MailEnv, SmtpMailer, send_to};
use super::{account, admin, admin_suspend, device, session, signup, staff_domains, tokens};
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::State;
use axum::http::{HeaderMap, Method, Request, Uri};
use axum::response::Response;
use axum::routing::any;
use rusqlite::{Connection, Transaction};
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub type TxHook = Box<dyn Fn(&Transaction, &str, i64) -> rusqlite::Result<()> + Send + Sync>;
pub type ExportExtra = Box<dyn Fn(&Connection, &str) -> Map<String, Value> + Send + Sync>;
pub type Labels = Box<dyn Fn(&Connection, &[String]) -> HashMap<String, String> + Send + Sync>;

pub struct AuthState {
    pub db: Mutex<Connection>,
    pub cfg: AuthConfig,
    pub keys: Keys,
    pub mail: MailEnv,
    pub mailer: Box<dyn Mailer>,
    pub clock: Box<dyn Fn() -> i64 + Send + Sync>,
    /// The pool's content removal when an account is deleted (phase 4).
    pub on_delete: TxHook,
    /// The pool's standing hook when an account is suspended or restored (phase 4).
    pub standing: TxHook,
    /// Extra sections of the data export (phase 4).
    pub export_extra: ExportExtra,
    pub domain_labels: Labels,
}

pub fn system_now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

impl AuthState {
    /// SMTP mail from `mail`, the system clock, and no pool hooks yet.
    pub fn new(db: Connection, cfg: AuthConfig, mail: MailEnv) -> Self {
        let keys = derive_keys(&cfg.secret);
        Self {
            db: Mutex::new(db),
            cfg,
            keys,
            mailer: Box::new(SmtpMailer { env: mail.clone() }),
            mail,
            clock: Box::new(system_now),
            on_delete: Box::new(|_, _, _| Ok(())),
            standing: Box::new(|_, _, _| Ok(())),
            export_extra: Box::new(|_, _| Map::new()),
            domain_labels: Box::new(staff_domains::labels),
        }
    }
}

const GET: &str = "GET";
const POST: &str = "POST";

/// The methods a path answers, or None when the path is not one of ours.
fn methods_for(path: &str) -> Option<&'static [&'static str]> {
    let a = PATHS.admin;
    let p = PATHS;
    let exact: [(&str, &'static [&'static str]); 14] = [
        (p.signup, &[POST]),
        (p.login, &[POST]),
        (p.refresh, &[POST]),
        (p.logout, &[POST]),
        (p.verify, &[POST]),
        (p.resend, &[POST]),
        (p.forgot, &[POST]),
        (p.reset, &[POST]),
        (p.me, &[GET]),
        (p.account, &["DELETE"]),
        (p.export, &[GET]),
        (p.enrol, &[POST]),
        (p.enrolment_code, &[POST]),
        (p.reauth, &[POST]),
    ];
    if let Some((_, m)) = exact.iter().find(|(x, _)| *x == path) {
        return Some(m);
    }
    if path == p.reapply {
        return Some(&[POST]);
    }
    let rest = path.strip_prefix(a)?.strip_prefix('/')?;
    let revoke = p.admin_revoke.trim_start_matches('/');
    Some(match rest {
        "queue" | "proposals" | "audit" => &[GET],
        "approve" | "reject" | "reject-bulk" | "confirm" | "cancel" | "suspend" | "restore" => &[POST],
        r if r == revoke => &[POST],
        "staff-domains" => &[GET, "PUT", "DELETE"],
        r if r.starts_with("users/") && r.len() > 6 && !r[6..].contains('/') => &[GET],
        _ => return None,
    })
}

fn required(h: &HeaderMap, bytes: &[u8]) -> Result<Map<String, Value>, HttpError> {
    parse_json(h, bytes, MAX_BODY_BYTES)
}

/// Runs one route. Everything here is synchronous; the caller runs it on a blocking thread.
#[allow(clippy::too_many_arguments)]
fn run(s: &AuthState, method: &str, path: &str, h: &HeaderMap, query: &HashMap<String, String>, bytes: &[u8], client: &ClientInfo, out: &mut Outbox) -> Result<Response, HttpError> {
    let c = Ctx { db: &s.db, cfg: &s.cfg, keys: &s.keys, now: (s.clock)() };
    let p = PATHS;
    let a = p.admin;
    let hook = &*s.standing;
    match (method, path) {
        (POST, x) if x == p.signup => signup::signup(&c, h, &required(h, bytes)?, out),
        (POST, x) if x == p.login => session::login(&c, h, client, &required(h, bytes)?),
        (POST, x) if x == p.refresh => {
            let body = if client.mode == super::http::ClientMode::Native { Some(required(h, bytes)?) } else { None };
            session::refresh(&c, h, client, body.as_ref())
        }
        (POST, x) if x == p.logout => {
            let body = if h.contains_key("content-type") { Some(required(h, bytes)?) } else { None };
            session::logout(&c, h, client, body.as_ref())
        }
        (POST, x) if x == p.verify => tokens::verify(&c, h, client, &required(h, bytes)?, out),
        (POST, x) if x == p.resend => signup::resend(&c, h, &required(h, bytes)?, out),
        (POST, x) if x == p.forgot => signup::forgot(&c, h, &required(h, bytes)?, out),
        (POST, x) if x == p.reset => tokens::reset(&c, h, client, &required(h, bytes)?, out),
        (GET, x) if x == p.me => account::me(&c, h),
        ("DELETE", x) if x == p.account => account::delete_account(&c, h, client, &required(h, bytes)?, &*s.on_delete, out),
        (GET, x) if x == p.export => account::export(&c, h, &*s.export_extra),
        (POST, x) if x == p.enrol => device::enrol(&c, h, client, &required(h, bytes)?),
        (POST, x) if x == p.enrolment_code => device::enrolment_code(&c, h, client, out),
        (POST, x) if x == p.reauth => device::reauth(&c, h, client, &required(h, bytes)?),
        (POST, x) if x == p.reapply => account::reapply(&c, h, &required(h, bytes)?),
        _ => {
            let rest = path.strip_prefix(a).and_then(|r| r.strip_prefix('/')).unwrap_or("");
            let labels: DomainLabels = &*s.domain_labels;
            match (method, rest) {
                (GET, "queue") => admin_read::queue(&c, h, client, query, labels),
                (GET, "proposals") => admin_suspend::proposals(&c, h, client),
                (GET, "audit") => admin_read::audit_log(&c, h, client, query),
                (GET, "staff-domains") => staff_domains::list(&c, h, client),
                (GET, u) if u.starts_with("users/") => admin_read::user_detail(&c, h, client, &u[6..]),
                (POST, "approve") => admin::approve(&c, h, client, &required(h, bytes)?, out),
                (POST, "reject") => admin::reject(&c, h, client, &required(h, bytes)?, out),
                (POST, "reject-bulk") => admin::reject_bulk(&c, h, client, &required(h, bytes)?, out),
                (POST, "confirm") => admin_suspend::confirm(&c, h, client, &required(h, bytes)?, hook, out),
                (POST, "cancel") => admin_suspend::cancel(&c, h, client, &required(h, bytes)?, out),
                (POST, "suspend") => admin_suspend::suspend(&c, h, client, &required(h, bytes)?, hook, out),
                (POST, "restore") => admin_suspend::restore(&c, h, client, &required(h, bytes)?, hook, out),
                (POST, r) if r == p.admin_revoke.trim_start_matches('/') => admin::revoke(&c, h, client, &required(h, bytes)?, out),
                ("PUT", "staff-domains") => staff_domains::put(&c, h, client, &required(h, bytes)?),
                ("DELETE", "staff-domains") => staff_domains::delete(&c, h, client, &required(h, bytes)?),
                _ => Err(HttpError::new(404, "not_found")),
            }
        }
    }
}

fn dispatch_blocking(s: &AuthState, method: &str, path: &str, h: &HeaderMap, query: &HashMap<String, String>, bytes: &[u8], allowed: &[&str]) -> (Response, Outbox) {
    let mut out = Outbox::new();
    let origin = h.get("origin").and_then(|v| v.to_str().ok());
    let cors = cors_headers(origin, &s.cfg.allowed_origins);
    if !allowed.contains(&method) {
        let allow = allowed.join(", ");
        let res = json(405, &json!({ "error": "method_not_allowed" }), &[("Allow", &allow)]);
        return (with_headers(res, &cors), out);
    }
    let res = match client_info(h, &s.cfg.allowed_origins).and_then(|client| run(s, method, path, h, query, bytes, &client, &mut out)) {
        Ok(r) => r,
        Err(e) => error_response(&e),
    };
    (with_headers(res, &cors), out)
}

/// Sends what a handler queued. Failures are logged as a class, never with the address or the text.
pub fn deliver(s: &AuthState, out: Outbox) {
    for m in out {
        match m.kind {
            MailKind::NewApplication => {
                let verdict = new_application_budget(&s.db.lock().unwrap_or_else(|p| p.into_inner()), (s.clock)());
                match verdict {
                    Ok(Notice::Sent) => {
                        let text = format!("Account {} confirmed its address and is in the approval queue.", m.to);
                        if let Err(e) = send_to(&s.mail, false, "verSHion: an account is awaiting approval", &text) {
                            tracing::warn!("mail: new-application notice: {e}");
                        }
                    }
                    Ok(Notice::Deferred) => {}
                    Err(_) => tracing::warn!("mail: new-application budget: database error"),
                }
            }
            MailKind::AdminAlert => {
                let ev = AdminEvent { action: m.reason.as_deref().unwrap_or(""), actor_id: m.actor.as_deref(), target_id: &m.to, at: m.expires_at.unwrap_or(0), proposal_id: m.token.as_deref() };
                if let Err(e) = admin_alert(&s.mail, &ev) {
                    tracing::warn!("mail: admin alert: {e}");
                }
            }
            _ => {
                if let Err(e) = s.mailer.send(&m) {
                    tracing::warn!(kind = ?m.kind, "mail: {e}");
                }
            }
        }
    }
}

async fn handle(State(s): State<Arc<AuthState>>, req: Request<Body>) -> Response {
    let (parts, body) = req.into_parts();
    let path = parts.uri.path().to_string();
    let Some(allowed) = methods_for(&path) else { return json(404, &json!({ "error": "not_found" }), &[]) };
    let method = parts.method.as_str().to_string();
    if parts.method == Method::OPTIONS {
        return preflight(&parts.headers, &s.cfg.allowed_origins, &allowed.join(", "));
    }
    let origin = parts.headers.get("origin").and_then(|v| v.to_str().ok()).map(str::to_string);
    let cors = cors_headers(origin.as_deref(), &s.cfg.allowed_origins);
    let bytes = match to_bytes(body, MAX_BODY_BYTES + 1).await {
        Ok(b) => b,
        Err(_) => return with_headers(error_response(&HttpError::new(413, "body_too_large")), &cors),
    };
    let query = query_of(&parts.uri);
    let headers = parts.headers;
    let st = s.clone();
    let joined = tokio::task::spawn_blocking(move || dispatch_blocking(&st, &method, &path, &headers, &query, &bytes, allowed)).await;
    let Ok((res, out)) = joined else {
        tracing::error!("auth: a handler panicked");
        return with_headers(json(500, &json!({ "error": "server_error" }), &[]), &cors);
    };
    if !out.is_empty() {
        let st = s.clone();
        tokio::task::spawn_blocking(move || deliver(&st, out));
    }
    res
}

fn query_of(uri: &Uri) -> HashMap<String, String> {
    uri.query().and_then(|q| serde_urlencoded::from_str(q).ok()).unwrap_or_default()
}

/// Every auth route. Unknown paths fall through to the other routers (404 there).
pub fn router(state: Arc<AuthState>) -> Router {
    let a = PATHS.admin;
    let mut r = Router::new();
    let mut paths: Vec<String> = [PATHS.signup, PATHS.login, PATHS.refresh, PATHS.logout, PATHS.verify, PATHS.resend, PATHS.forgot, PATHS.reset, PATHS.me, PATHS.account, PATHS.export, PATHS.enrol, PATHS.enrolment_code, PATHS.reauth, PATHS.reapply]
        .iter()
        .map(|s| s.to_string())
        .collect();
    for rest in ["queue", "proposals", "audit", "approve", "reject", "reject-bulk", "confirm", "cancel", "suspend", "restore", "staff-domains"] {
        paths.push(format!("{a}/{rest}"));
    }
    paths.push(format!("{a}{}", PATHS.admin_revoke));
    for p in paths {
        r = r.route(&p, any(handle));
    }
    r.route(&format!("{a}/users/{{id}}"), any(handle)).with_state(state)
}

#[cfg(test)]
#[path = "router_tests.rs"]
mod tests;
