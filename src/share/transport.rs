//! The HTTP seam of the shared-pool client.
//!
//! The client builds a [`Request`] and awaits a [`Response`] through [`Transport`]; nothing else in
//! `share` touches the network, so every flow runs against a mock in tests. Two real transports:
//! - [`NativeTransport`] (ureq): sends `X-Client: native` and never an `Origin` header, which is
//!   what makes the server return refresh and device tokens in bodies (and what it requires: a
//!   native request with an `Origin` is refused).
//! - [`WebTransport`] (`fetch`): `credentials: "include"` so the HttpOnly refresh and device cookies
//!   travel, and no `X-Client` or `X-Device-Token` (the browser path is cookie-only).
//!
//! Requests and responses redact their bodies and credential headers in `Debug`.
//!
//! What a request can carry (the Worker's pool routes need all of it):
//! - any of GET, HEAD, POST, PUT, PATCH, DELETE;
//! - a JSON text body ([`Request::body`], sent as `application/json`) or raw bytes
//!   ([`Request::bytes`], a blob or a publish, with its own [`Request::content_type`]); either way
//!   `Content-Length` is sent (a bodyless POST sends `Content-Length: 0`), because the Worker
//!   refuses a body without one;
//! - a response cap ([`Request::max_response`], 8 MB unless set): the read stops at the cap, never
//!   buffering past it, and the call fails with [`NetKind::TooLarge`] instead of returning a cut-off
//!   body. `binary` asks for [`Response::bytes`] instead of text;
//! - `If-None-Match` as an ordinary header: a 304 comes back as a response (empty body) with the
//!   server's [`Response::etag`]; HEAD returns the headers and no body.
//!
//! Redirects are never followed (native: `max_redirects(0)`; browser: `redirect: "error"`), so a
//! `Location` can't move a body or a token to another host. A request whose URL is not plain
//! http(s), has a user name in it, or carries credential headers to a URL with one, is refused
//! before anything is sent.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use super::secret::Secret;

/// A boxed future that need not be `Send` (browser futures are not).
pub type LocalFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Head => "HEAD",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Patch => "PATCH",
            Method::Delete => "DELETE",
        }
    }
}

/// Headers whose values are credentials. Never printed.
const SECRET_HEADERS: [&str; 3] = ["authorization", "x-device-token", "x-sudo-token"];

pub struct Request {
    pub method: Method,
    /// Absolute URL: the configured base plus the route path.
    pub url: String,
    pub headers: Vec<(String, Secret)>,
    /// JSON text. It can hold a password or token, so it is a [`Secret`] too.
    pub body: Option<Secret>,
    /// A different time limit than [`DEFAULT_TIMEOUT`]: shorter for refresh (see
    /// `client::REFRESH_TIMEOUT`), longer for a big upload or download. It covers the whole call,
    /// body included.
    pub timeout: Option<Duration>,
    /// Raw request body (a blob, a publish). Wins over `body`. Its type is `content_type`, or
    /// `application/octet-stream`.
    pub bytes: Option<Vec<u8>>,
    /// Overrides the body's type (JSON text defaults to `application/json`).
    pub content_type: Option<String>,
    /// Most response bytes read; `None` is [`DEFAULT_MAX_RESPONSE`]. Past it the call fails with
    /// [`NetKind::TooLarge`].
    pub max_response: Option<usize>,
    /// Keep the response as bytes ([`Response::bytes`]) rather than text ([`Response::body`]); a
    /// read that fails part way is then an error, never a short success.
    pub binary: bool,
}

impl Default for Request {
    fn default() -> Self {
        Request {
            method: Method::Get,
            url: String::new(),
            headers: Vec::new(),
            body: None,
            timeout: None,
            bytes: None,
            content_type: None,
            max_response: None,
            binary: false,
        }
    }
}

/// Every route but refresh.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Response cap when a request names none: covers the pool's JSON routes and an item (a few MB).
pub const DEFAULT_MAX_RESPONSE: usize = 8 * 1024 * 1024;

/// An error answer (non-2xx) is read only up to this much; a longer one comes back with an empty
/// body (the status still says what happened).
pub const ERROR_BODY_LIMIT: usize = 64 * 1024;

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.expose())
    }

    /// The route part of the URL (`/auth/login`), for mocks and messages.
    pub fn path(&self) -> &str {
        let after_scheme = self.url.split_once("://").map_or(self.url.as_str(), |(_, r)| r);
        let p = after_scheme.find('/').map_or("", |i| &after_scheme[i..]);
        p.split(['?', '#']).next().unwrap_or("")
    }

    pub fn json(&self) -> Option<serde_json::Value> {
        self.body.as_ref().and_then(|b| serde_json::from_str(b.expose()).ok())
    }

    /// The body that will go on the wire and its type, or `None` for no body.
    fn wire_body(&self) -> Option<(Vec<u8>, String)> {
        if let Some(b) = &self.bytes {
            return Some((b.clone(), self.content_type.clone().unwrap_or_else(|| "application/octet-stream".into())));
        }
        self.body
            .as_ref()
            .map(|b| (b.expose().as_bytes().to_vec(), self.content_type.clone().unwrap_or_else(|| "application/json".into())))
    }

    /// Refuses a request that could leak: not http(s), a user name in the URL, or credentials
    /// aimed at a URL that carries its own. Run by both transports before sending.
    pub fn check_target(&self) -> Result<(), NetError> {
        let bad = |why: &str| NetError::offline(format!("Request refused: {why}."));
        let u = url::Url::parse(&self.url).map_err(|_| bad("the address is not valid"))?;
        if !matches!(u.scheme(), "http" | "https") {
            return Err(bad("only http and https addresses are used"));
        }
        if u.host_str().is_none() {
            return Err(bad("the address has no host"));
        }
        if !u.username().is_empty() || u.password().is_some() {
            return Err(bad("the address must not contain a user name"));
        }
        Ok(())
    }
}

impl std::fmt::Debug for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let headers: Vec<String> = self
            .headers
            .iter()
            .map(|(k, v)| {
                if SECRET_HEADERS.contains(&k.to_ascii_lowercase().as_str()) {
                    format!("{k}: <redacted>")
                } else {
                    format!("{k}: {}", v.expose())
                }
            })
            .collect();
        f.debug_struct("Request")
            .field("method", &self.method)
            .field("url", &self.url)
            .field("headers", &headers)
            .field("body", &self.body.as_ref().map(|b| format!("<{} bytes redacted>", b.expose().len())))
            .field("timeout", &self.timeout)
            .field("bytes", &self.bytes.as_ref().map(|b| format!("<{} bytes redacted>", b.len())))
            .field("content_type", &self.content_type)
            .field("max_response", &self.max_response)
            .field("binary", &self.binary)
            .finish()
    }
}

pub struct Response {
    pub status: u16,
    /// `Retry-After` in seconds, when the server sent one (429s).
    pub retry_after: Option<u64>,
    /// JSON text (or empty for 204, 304 and HEAD). It can hold tokens, so it is a [`Secret`].
    /// Empty when the request asked for `binary`.
    pub body: Secret,
    /// The body as bytes, only for a `binary` request.
    pub bytes: Vec<u8>,
    /// The `ETag` header, as the server sent it (quotes and `W/` included), to send back as
    /// `If-None-Match`.
    pub etag: Option<String>,
    /// The `Content-Type` header.
    pub content_type: Option<String>,
}

impl Response {
    pub fn new(status: u16, body: &str) -> Response {
        Response { status, retry_after: None, body: Secret::new(body), bytes: Vec::new(), etag: None, content_type: None }
    }

    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// 304 Not Modified: the copy sent with `If-None-Match` is still current.
    pub fn not_modified(&self) -> bool {
        self.status == 304
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(self.body.expose()).unwrap_or(serde_json::Value::Null)
    }
}

impl std::fmt::Debug for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Response")
            .field("status", &self.status)
            .field("retry_after", &self.retry_after)
            .field("body", &format!("<{} bytes redacted>", self.body.expose().len() + self.bytes.len()))
            .field("etag", &self.etag)
            .finish()
    }
}

/// The request never got an HTTP answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetError {
    pub kind: NetKind,
    /// A sentence naming the host and the cause; never a token.
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetKind {
    /// No connection: offline, DNS, refused, reset, timeout, or (in a browser) a CORS refusal,
    /// which a page cannot tell apart.
    Offline,
    /// A connection, but no secure one: a certificate this computer does not trust (school
    /// networks that inspect traffic install their own) or a broken TLS handshake.
    Tls,
    /// The answer is bigger than the request allowed (`max_response`). Nothing past the cap was
    /// read.
    TooLarge,
}

impl NetError {
    pub fn too_large(text: impl Into<String>) -> NetError {
        NetError { kind: NetKind::TooLarge, text: text.into() }
    }
    pub fn offline(text: impl Into<String>) -> NetError {
        NetError { kind: NetKind::Offline, text: text.into() }
    }
    pub fn tls(text: impl Into<String>) -> NetError {
        NetError { kind: NetKind::Tls, text: text.into() }
    }
}

pub trait Transport: Send + Sync {
    fn send<'a>(&'a self, req: Request) -> LocalFuture<'a, Result<Response, NetError>>;
}

/// Which delivery the server uses for this client (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Native,
    Web,
}

impl Mode {
    /// The build's own mode.
    pub fn current() -> Mode {
        if cfg!(target_arch = "wasm32") { Mode::Web } else { Mode::Native }
    }
}

/// The header policy both real transports apply on top of what the client set: native always
/// identifies itself and never claims an origin; the browser never sends the native-only headers
/// (the server's preflight would refuse them anyway).
pub fn finalize_headers(mode: Mode, headers: &mut Vec<(String, Secret)>) {
    headers.retain(|(k, _)| {
        let k = k.to_ascii_lowercase();
        match mode {
            Mode::Native => k != "origin" && k != "x-client",
            Mode::Web => k != "x-client" && k != "x-device-token" && k != "origin",
        }
    });
    if mode == Mode::Native {
        headers.push(("X-Client".into(), Secret::new("native")));
    }
}

fn parse_retry_after(v: Option<&str>) -> Option<u64> {
    v.and_then(|s| s.trim().parse::<u64>().ok())
}

/// Native: one blocking ureq call per request. The future is ready when polled, which is fine
/// because native jobs run on their own thread (see `model::Runner`).
#[cfg(not(target_arch = "wasm32"))]
pub struct NativeTransport {
    agent: ureq::Agent,
}

#[cfg(not(target_arch = "wasm32"))]
fn native_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        // Each request sets its own limit (`Request::timeout`); this one covers the call from the
        // first connect to the last body byte.
        .timeout_global(Some(DEFAULT_TIMEOUT))
        .http_status_as_error(false)
        // An auth API never redirects; following one could carry a body to another host.
        .max_redirects(0)
        // The OS trust store, not a bundled root list: school networks that inspect TLS install
        // their own root certificate there, and a bundled list would refuse every request.
        .tls_config(ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build())
        .build()
        .into()
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for NativeTransport {
    fn default() -> Self {
        NativeTransport { agent: native_agent() }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl NativeTransport {
    fn send_blocking(&self, mut req: Request) -> Result<Response, NetError> {
        req.check_target()?;
        finalize_headers(Mode::Native, &mut req.headers);
        let host = host_of(&req.url);
        let net = |e: ureq::Error| classify(&host, &e);
        let cap = req.max_response.unwrap_or(DEFAULT_MAX_RESPONSE);
        let timeout = req.timeout.unwrap_or(DEFAULT_TIMEOUT);

        let mut b = ureq::http::Request::builder().method(req.method.as_str()).uri(req.url.as_str());
        for (k, v) in &req.headers {
            // The type and length are set below from the body; a caller's copy would conflict.
            if k.eq_ignore_ascii_case("content-length") || k.eq_ignore_ascii_case("content-type") {
                continue;
            }
            b = b.header(k.as_str(), v.expose());
        }
        let wire = req.wire_body();
        // GET and HEAD carry no body; every other method always sends Content-Length (0 for none).
        let has_body_slot = !matches!(req.method, Method::Get | Method::Head);
        if let Some((_, ct)) = &wire {
            b = b.header("Content-Type", ct.as_str());
        }
        let invalid = || NetError::offline("Request refused: a header is not valid.".to_string());
        let mut resp = if has_body_slot {
            let data = wire.map(|(d, _)| d).unwrap_or_default();
            b = b.header("Content-Length", data.len());
            let r = b.body(data).map_err(|_| invalid())?;
            let r = self.agent.configure_request(r).timeout_global(Some(timeout)).build();
            self.agent.run(r)
        } else {
            let r = b.body(()).map_err(|_| invalid())?;
            let r = self.agent.configure_request(r).timeout_global(Some(timeout)).build();
            self.agent.run(r)
        }
        .map_err(net)?;
        let status = resp.status().as_u16();
        let hdr = |name: &str| resp.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_string);
        let retry_after = parse_retry_after(hdr("retry-after").as_deref());
        let etag = hdr("etag");
        let content_type = hdr("content-type");
        let declared = hdr("content-length").and_then(|v| v.trim().parse::<u64>().ok());
        let ok = (200..300).contains(&status);
        let limit = if ok { cap } else { ERROR_BODY_LIMIT.min(cap) };
        let too_big = || NetError::too_large(format!("The answer from {host} is larger than this app accepts ({}).", size_text(cap)));
        let empty = status == 204 || status == 304 || status == 205 || (100..200).contains(&status) || req.method == Method::Head;
        // The status is the answer; a body that can't be read (a 204 labelled gzip with nothing in
        // it, a truncated error page) is an empty body, not "offline": the request did arrive.
        // A binary 2xx is the exception: a cut-off blob must never look like a whole one.
        let mut data = Vec::new();
        if !empty {
            if ok && declared.is_some_and(|n| n > cap as u64) {
                return Err(too_big());
            }
            // One byte past the limit, so a body of exactly `limit` bytes is told from a longer one.
            match resp.body_mut().with_config().limit(limit as u64 + 1).read_to_vec() {
                Ok(v) if v.len() > limit => {
                    if ok {
                        return Err(too_big());
                    }
                }
                Ok(v) => data = v,
                Err(ureq::Error::BodyExceedsLimit(_)) if ok => return Err(too_big()),
                Err(e) if ok && req.binary => return Err(classify(&host, &e)),
                Err(_) => {}
            }
        }
        let (body, bytes) = if req.binary && ok {
            (String::new(), data)
        } else {
            (String::from_utf8(data).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned()), Vec::new())
        };
        Ok(Response { status, retry_after, body: Secret::new(body), bytes, etag, content_type })
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Transport for NativeTransport {
    fn send<'a>(&'a self, req: Request) -> LocalFuture<'a, Result<Response, NetError>> {
        let out = self.send_blocking(req);
        Box::pin(std::future::ready(out))
    }
}

/// Sorts a ureq failure into offline vs. TLS, with a short reason (its full text can include the
/// URL, never anything secret, but a short reason reads better in a dialog).
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn classify(host: &str, e: &ureq::Error) -> NetError {
    let tls_text = |why: &str| NetError::tls(format!("No secure connection to {host} could be made ({why})."));
    match e {
        ureq::Error::Rustls(r) => tls_text(&short(&r.to_string())),
        ureq::Error::Tls(why) => tls_text(why),
        ureq::Error::Pem(_) => tls_text("certificate"),
        ureq::Error::Timeout(_) => NetError::offline(format!("Can't reach {host}: the connection timed out.")),
        ureq::Error::HostNotFound => NetError::offline(format!("Can't reach {host}: the server name was not found.")),
        ureq::Error::ConnectionFailed => NetError::offline(format!("Can't reach {host}: the connection failed.")),
        // rustls reports handshake and certificate failures through the stream as I/O errors.
        ureq::Error::Io(io) if is_tls_io(io) => tls_text(&short(&io.to_string())),
        ureq::Error::Io(io) => NetError::offline(format!("Can't reach {host}: {}.", io.kind())),
        other => NetError::offline(format!("Can't reach {host}: {}.", short(&other.to_string()))),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn is_tls_io(io: &std::io::Error) -> bool {
    let inner = io.get_ref().map(|e| e.to_string().to_ascii_lowercase()).unwrap_or_default();
    io.kind() == std::io::ErrorKind::InvalidData
        || ["certificate", "handshake", "tls", "alert"].iter().any(|w| inner.contains(w))
}

#[cfg(not(target_arch = "wasm32"))]
fn short(s: &str) -> String {
    s.chars().take(120).collect()
}

/// "8 MB", "64 KB", "500 bytes", for messages.
pub(crate) fn size_text(n: usize) -> String {
    match n {
        n if n >= 1024 * 1024 => format!("{} MB", n / 1024 / 1024),
        n if n >= 1024 => format!("{} KB", n / 1024),
        n => format!("{n} bytes"),
    }
}

pub(crate) fn host_of(url: &str) -> String {
    url::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_else(|| "the server".into())
}

/// Browser: `fetch` with CORS and credentials. The access token is only ever in the
/// `Authorization` header of a request; nothing is written to any browser storage. The browser
/// sets `Content-Length` itself from the body (0 for a bodyless POST) and never follows a redirect
/// here. A response is read chunk by chunk and cancelled at the cap.
#[cfg(target_arch = "wasm32")]
#[derive(Default)]
pub struct WebTransport;

#[cfg(target_arch = "wasm32")]
impl Transport for WebTransport {
    fn send<'a>(&'a self, mut req: Request) -> LocalFuture<'a, Result<Response, NetError>> {
        Box::pin(async move {
            use wasm_bindgen::JsCast as _;
            req.check_target()?;
            finalize_headers(Mode::Web, &mut req.headers);
            let host = host_of(&req.url);
            let unreachable = || {
                NetError::offline(format!(
                    "Can't reach {host}. You may be offline, or the server does not allow requests from this page."
                ))
            };
            let cap = req.max_response.unwrap_or(DEFAULT_MAX_RESPONSE);
            let too_big = || NetError::too_large(format!("The answer from {host} is larger than this app accepts ({}).", size_text(cap)));
            let headers = web_sys::Headers::new().map_err(|_| unreachable())?;
            for (k, v) in &req.headers {
                if k.eq_ignore_ascii_case("content-length") || k.eq_ignore_ascii_case("content-type") {
                    continue;
                }
                headers.set(k, v.expose()).map_err(|_| unreachable())?;
            }
            let init = web_sys::RequestInit::new();
            init.set_method(req.method.as_str());
            init.set_mode(web_sys::RequestMode::Cors);
            init.set_credentials(web_sys::RequestCredentials::Include);
            // Like the native agent (`max_redirects(0)`): an auth API never redirects, and
            // following one could carry a body to another host.
            init.set_redirect(web_sys::RequestRedirect::Error);
            if !matches!(req.method, Method::Get | Method::Head) {
                if let Some((data, ct)) = req.wire_body() {
                    headers.set("Content-Type", &ct).map_err(|_| unreachable())?;
                    let arr = js_sys::Uint8Array::from(data.as_slice());
                    init.set_body(&arr);
                }
            }
            init.set_headers(&headers);
            let limit = req.timeout.unwrap_or(DEFAULT_TIMEOUT).as_millis().min(u32::MAX as u128) as u32;
            let signal = web_sys::AbortSignal::timeout_with_u32(limit);
            init.set_signal(Some(&signal));
            let window = web_sys::window().ok_or_else(unreachable)?;
            let resp = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str_and_init(&req.url, &init))
                .await
                .map_err(|_| unreachable())?;
            let resp: web_sys::Response = resp.dyn_into().map_err(|_| unreachable())?;
            let status = resp.status();
            let h = resp.headers();
            let get = |n: &str| h.get(n).ok().flatten();
            let retry_after = parse_retry_after(get("Retry-After").as_deref());
            let etag = get("ETag");
            let content_type = get("Content-Type");
            let ok = (200..300).contains(&status);
            let limit = if ok { cap } else { ERROR_BODY_LIMIT.min(cap) };
            let empty = status == 204 || status == 304 || status == 205 || req.method == Method::Head;
            if ok && !empty && get("Content-Length").and_then(|v| v.trim().parse::<u64>().ok()).is_some_and(|n| n > cap as u64) {
                if let Some(body) = resp.body() {
                    let _ = body.cancel();
                }
                return Err(too_big());
            }
            let mut data: Vec<u8> = Vec::new();
            if !empty {
                if let Some(stream) = resp.body() {
                    let reader: web_sys::ReadableStreamDefaultReader = stream.get_reader().unchecked_into();
                    loop {
                        let step = match wasm_bindgen_futures::JsFuture::from(reader.read()).await {
                            Ok(v) => v,
                            Err(_) if ok && req.binary => return Err(unreachable()),
                            Err(_) => {
                                data.clear();
                                break;
                            }
                        };
                        let done = js_sys::Reflect::get(&step, &"done".into()).ok().and_then(|d| d.as_bool()).unwrap_or(true);
                        if done {
                            break;
                        }
                        let chunk = js_sys::Reflect::get(&step, &"value".into()).map(|v| js_sys::Uint8Array::new(&v)).unwrap_or_else(|_| js_sys::Uint8Array::new_with_length(0));
                        if data.len() + chunk.length() as usize > limit {
                            let _ = wasm_bindgen_futures::JsFuture::from(reader.cancel()).await;
                            if ok {
                                return Err(too_big());
                            }
                            data.clear();
                            break;
                        }
                        data.extend_from_slice(&chunk.to_vec());
                    }
                }
            }
            let (body, bytes) = if req.binary && ok {
                (String::new(), data)
            } else {
                (String::from_utf8_lossy(&data).into_owned(), Vec::new())
            };
            Ok(Response { status, retry_after, body: Secret::new(body), bytes, etag, content_type })
        })
    }
}

/// Runs a future to completion on the current thread, parking between polls. Native jobs use
/// it on their worker thread; tests use it to drive the client. (The browser uses
/// `spawn_local` instead and never blocks.)
#[cfg(not(target_arch = "wasm32"))]
pub fn block_on<F: Future>(fut: F) -> F::Output {
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake, Waker};
    struct Unpark(std::thread::Thread);
    impl Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut fut = std::pin::pin!(fut);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::thread::park(),
        }
    }
}
