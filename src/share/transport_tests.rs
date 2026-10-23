//! The native transport against a small in-process HTTP server (std::net only, 127.0.0.1, an OS
//! picked port). Each test starts its own server; nothing here touches a real service.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::api::{ApiError, ErrorDetail};
use super::secret::Secret;
use super::transport::{block_on, Method, NativeTransport, NetKind, Request, Response, Transport, DEFAULT_MAX_RESPONSE};

#[derive(Clone, Debug, Default)]
struct Seen {
    line: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Seen {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

struct Server {
    port: u16,
    seen: Arc<Mutex<Vec<Seen>>>,
}

impl Server {
    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }
    fn seen(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }
}

/// Serves connections until the process ends. `handler` gets what was read (head and, if the
/// request had a Content-Length, the whole body) and writes the reply on the stream itself.
fn serve(handler: impl Fn(&Seen, &mut TcpStream) + Send + Sync + 'static) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    let handler = Arc::new(handler);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { break };
            let (log, handler) = (log.clone(), handler.clone());
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut s = Seen::default();
                reader.read_line(&mut s.line).unwrap();
                loop {
                    let mut l = String::new();
                    if reader.read_line(&mut l).unwrap_or(0) == 0 || l == "\r\n" {
                        break;
                    }
                    if let Some((k, v)) = l.trim_end().split_once(':') {
                        s.headers.push((k.to_string(), v.trim().to_string()));
                    }
                }
                if let Some(n) = s.header("content-length").and_then(|v| v.parse::<usize>().ok()) {
                    s.body = vec![0; n];
                    reader.read_exact(&mut s.body).unwrap();
                }
                log.lock().unwrap().push(s.clone());
                let mut out = stream;
                handler(&s, &mut out);
            });
        }
    });
    Server { port, seen }
}

fn reply(out: &mut TcpStream, status: &str, headers: &[(&str, &str)], body: &[u8]) {
    let mut head = format!("HTTP/1.1 {status}\r\nConnection: close\r\n");
    for (k, v) in headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    if !headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("content-length")) {
        head.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    head.push_str("\r\n");
    let _ = out.write_all(head.as_bytes());
    let _ = out.write_all(body);
}

fn req(method: Method, url: String) -> Request {
    Request { method, url, ..Request::default() }
}

fn send(r: Request) -> Result<Response, super::transport::NetError> {
    block_on(NativeTransport::default().send(r))
}

fn noise(n: usize) -> Vec<u8> {
    // Every byte value, so UTF-8 assumptions would show.
    (0..n).map(|i| (i * 31 + i / 251) as u8).collect()
}

#[test]
fn every_verb_goes_out_as_itself_and_comes_back() {
    let srv = serve(|s, out| {
        let verb = s.line.split(' ').next().unwrap().to_string();
        if verb == "HEAD" {
            reply(out, "200 OK", &[("Content-Length", "11"), ("X-Verb", "HEAD")], b"");
        } else {
            reply(out, "200 OK", &[("Content-Type", "application/json")], format!(r#"{{"verb":"{verb}"}}"#).as_bytes());
        }
    });
    for m in [Method::Get, Method::Head, Method::Post, Method::Put, Method::Patch, Method::Delete] {
        let r = send(req(m, srv.url("/v"))).unwrap_or_else(|e| panic!("{m:?}: {}", e.text));
        assert_eq!(r.status, 200, "{m:?}");
        if m == Method::Head {
            assert_eq!(r.body.expose(), "", "HEAD has no body");
        } else {
            assert_eq!(r.json()["verb"], m.as_str());
            assert_eq!(r.content_type.as_deref(), Some("application/json"));
        }
    }
    let seen = srv.seen();
    assert_eq!(seen.len(), 6);
    let mut verbs: Vec<_> = seen.iter().map(|s| s.line.split(' ').next().unwrap().to_string()).collect();
    verbs.sort();
    assert_eq!(verbs, ["DELETE", "GET", "HEAD", "PATCH", "POST", "PUT"]);
}

#[test]
fn content_length_is_always_sent() {
    let srv = serve(|_, out| reply(out, "204 No Content", &[], b""));
    // Bodyless writes: Content-Length 0, no Content-Type.
    for m in [Method::Post, Method::Put, Method::Patch, Method::Delete] {
        send(req(m, srv.url("/x"))).unwrap();
    }
    // JSON text body.
    let mut j = req(Method::Post, srv.url("/j"));
    j.body = Some(Secret::new(r#"{"a":"é"}"#));
    send(j).unwrap();
    // Raw bytes with a custom type.
    let blob = noise(70_000);
    let mut p = req(Method::Put, srv.url("/blob"));
    p.bytes = Some(blob.clone());
    p.content_type = Some("application/vnd.vershion.publish".into());
    send(p).unwrap();
    // Raw bytes, no type given.
    let mut o = req(Method::Put, srv.url("/o"));
    o.bytes = Some(vec![1, 2, 3]);
    send(o).unwrap();
    // GET carries none.
    send(req(Method::Get, srv.url("/g"))).unwrap();

    let seen = srv.seen();
    let by = |p: &str| seen.iter().find(|s| s.line.contains(&format!(" {p} "))).unwrap().clone();
    let mut bodyless = seen.iter().filter(|s| ["/x"].iter().any(|p| s.line.contains(&format!(" {p} "))));
    assert_eq!(bodyless.clone().count(), 4);
    assert!(bodyless.all(|s| s.header("content-length") == Some("0") && s.header("content-type").is_none() && s.body.is_empty()));
    let j = by("/j");
    assert_eq!(j.header("content-length"), Some("10")); // "é" is two bytes
    assert_eq!(j.header("content-type"), Some("application/json"));
    assert_eq!(j.body, r#"{"a":"é"}"#.as_bytes());
    let b = by("/blob");
    assert_eq!(b.header("content-length"), Some("70000"));
    assert_eq!(b.header("content-type"), Some("application/vnd.vershion.publish"));
    assert_eq!(b.body, blob, "raw bytes arrive untouched");
    let o = by("/o");
    assert_eq!(o.header("content-type"), Some("application/octet-stream"));
    let g = by("/g");
    assert!(g.header("content-length").is_none() && g.body.is_empty());
}

#[test]
fn a_caller_cannot_override_length_or_type_through_headers() {
    let srv = serve(|_, out| reply(out, "204 No Content", &[], b""));
    let mut r = req(Method::Post, srv.url("/x"));
    r.bytes = Some(vec![9; 5]);
    r.headers = vec![("Content-Length".into(), Secret::new("999")), ("content-type".into(), Secret::new("text/evil"))];
    send(r).unwrap();
    let s = &srv.seen()[0];
    assert_eq!(s.header("content-length"), Some("5"));
    assert_eq!(s.header("content-type"), Some("application/octet-stream"));
}

#[test]
fn big_bodies_go_both_ways() {
    // 6 MB up (a blob PUT), 6 MB down (binary), 200 KB of JSON text down (text, past the old 64 KB cut).
    let up = noise(6 * 1024 * 1024);
    let down = noise(6 * 1024 * 1024 + 17);
    let big_json = format!(r#"{{"items":"{}"}}"#, "x".repeat(200_000));
    let (d2, j2) = (down.clone(), big_json.clone());
    let srv = serve(move |s, out| {
        if s.line.starts_with("PUT") {
            reply(out, "200 OK", &[("ETag", "\"abc\"")], format!("{}", s.body.len()).as_bytes());
        } else if s.line.contains("/blob") {
            reply(out, "200 OK", &[("Content-Type", "image/webp"), ("ETag", "W/\"h\"")], &d2);
        } else {
            reply(out, "200 OK", &[("Content-Type", "application/json")], j2.as_bytes());
        }
    });
    let mut p = req(Method::Put, srv.url("/blobs/x"));
    p.bytes = Some(up.clone());
    p.timeout = Some(Duration::from_secs(60));
    let r = send(p).unwrap();
    assert_eq!(r.body.expose(), (6 * 1024 * 1024).to_string());
    assert_eq!(r.etag.as_deref(), Some("\"abc\""));
    assert_eq!(srv.seen()[0].body, up);

    let mut g = req(Method::Get, srv.url("/blob"));
    g.binary = true;
    let r = send(g).unwrap();
    assert_eq!(r.bytes, down);
    assert_eq!(r.body.expose(), "");
    assert_eq!(r.etag.as_deref(), Some("W/\"h\""));
    assert_eq!(r.content_type.as_deref(), Some("image/webp"));

    let r = send(req(Method::Get, srv.url("/items"))).unwrap();
    assert_eq!(r.body.expose().len(), big_json.len());
    assert_eq!(r.body.expose(), big_json);
}

#[test]
fn the_response_cap_is_enforced_without_buffering_past_it() {
    let sent = Arc::new(Mutex::new(0usize));
    let counter = sent.clone();
    let srv = serve(move |s, out| {
        if s.line.contains("/declared") {
            // Says it is 1 GB; sends nothing: a client that checks the header never reads.
            reply(out, "200 OK", &[("Content-Length", "1073741824")], b"");
        } else if s.line.contains("/exact") {
            reply(out, "200 OK", &[], &vec![b'a'; 1000]);
        } else if s.line.contains("/over") {
            reply(out, "200 OK", &[], &vec![b'a'; 1001]);
        } else {
            // No Content-Length, endless body (connection close delimits it): only a streaming
            // limit can stop the client.
            let _ = out.write_all(b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n");
            let chunk = vec![b'z'; 16 * 1024];
            while out.write_all(&chunk).is_ok() {
                *counter.lock().unwrap() += chunk.len();
                if *counter.lock().unwrap() > 64 * 1024 * 1024 {
                    break;
                }
            }
        }
    });
    let capped = |path: &str, cap: usize, binary: bool| {
        let mut r = req(Method::Get, srv.url(path));
        r.max_response = Some(cap);
        r.binary = binary;
        send(r)
    };
    assert_eq!(capped("/exact", 1000, false).unwrap().body.expose().len(), 1000);
    assert_eq!(capped("/exact", 1000, true).unwrap().bytes.len(), 1000);
    for binary in [false, true] {
        let e = capped("/over", 1000, binary).unwrap_err();
        assert_eq!(e.kind, NetKind::TooLarge, "{}", e.text);
    }
    let started = Instant::now();
    let e = capped("/declared", 1_000_000, true).unwrap_err();
    assert_eq!(e.kind, NetKind::TooLarge);
    assert!(started.elapsed() < Duration::from_secs(5));
    let e = capped("/endless", 100_000, true).unwrap_err();
    assert_eq!(e.kind, NetKind::TooLarge);
    assert!(matches!(ApiError::from(e), ApiError::TooLarge(_)));
    std::thread::sleep(Duration::from_millis(300));
    assert!(*sent.lock().unwrap() < 8 * 1024 * 1024, "the server was drained far past the cap: {}", sent.lock().unwrap());
    // Default cap is 8 MB.
    assert_eq!(DEFAULT_MAX_RESPONSE, 8 * 1024 * 1024);
    let e = send(req(Method::Get, srv.url("/endless"))).unwrap_err();
    assert_eq!(e.kind, NetKind::TooLarge);
}

#[test]
fn a_cut_off_binary_download_is_an_error_not_a_short_file() {
    let srv = serve(|_, out| {
        let _ = out.write_all(b"HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 100000\r\n\r\n");
        let _ = out.write_all(&[7u8; 1000]);
        // closes with 99000 bytes owed
    });
    let mut r = req(Method::Get, srv.url("/blob"));
    r.binary = true;
    let e = send(r).unwrap_err();
    assert_eq!(e.kind, NetKind::Offline, "{}", e.text);
}

#[test]
fn etag_and_if_none_match_give_a_304_with_no_body() {
    let srv = serve(|s, out| {
        let inm = s.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("if-none-match")).map(|(_, v)| v.clone());
        if inm.as_deref() == Some("\"v1\"") {
            // A 304 may carry a Content-Length that describes the 200; it has no body.
            reply(out, "304 Not Modified", &[("ETag", "\"v1\""), ("Content-Length", "5000")], b"");
        } else {
            reply(out, "200 OK", &[("ETag", "\"v1\""), ("Content-Type", "application/json")], br#"{"a":1}"#);
        }
    });
    let first = send(req(Method::Get, srv.url("/items/1"))).unwrap();
    assert_eq!((first.status, first.etag.as_deref()), (200, Some("\"v1\"")));
    assert!(first.is_success() && !first.not_modified());
    let mut again = req(Method::Get, srv.url("/items/1"));
    again.headers.push(("If-None-Match".into(), Secret::new(first.etag.clone().unwrap())));
    let t = Instant::now();
    let second = send(again).unwrap();
    assert!(second.not_modified() && !second.is_success());
    assert_eq!(second.etag.as_deref(), Some("\"v1\""));
    assert_eq!(second.body.expose(), "");
    assert!(t.elapsed() < Duration::from_secs(5), "waited for a body a 304 never has");
    // Same on a binary request.
    let mut b = req(Method::Get, srv.url("/items/1"));
    b.binary = true;
    b.headers.push(("If-None-Match".into(), Secret::new("\"v1\"")));
    let r = send(b).unwrap();
    assert!(r.not_modified() && r.bytes.is_empty());
}

#[test]
fn head_reports_headers_and_reads_no_body() {
    let srv = serve(|_, out| reply(out, "200 OK", &[("Content-Length", "123456"), ("ETag", "\"h\""), ("Content-Type", "image/webp")], b""));
    let mut r = req(Method::Head, srv.url("/blobs/x"));
    r.max_response = Some(10); // a HEAD says how big the body would be; it must not trip the cap
    let r = send(r).unwrap();
    assert_eq!((r.status, r.etag.as_deref(), r.content_type.as_deref()), (200, Some("\"h\""), Some("image/webp")));
    assert!(r.body.expose().is_empty() && r.bytes.is_empty());
}

#[test]
fn redirects_are_not_followed_and_nothing_reaches_the_other_host() {
    let elsewhere = serve(|_, out| reply(out, "200 OK", &[], b"you should not be here"));
    let target = elsewhere.url("/stolen");
    let srv = serve(move |_, out| reply(out, "302 Found", &[("Location", &target)], b""));
    for m in [Method::Get, Method::Post, Method::Put] {
        let mut r = req(m, srv.url("/go"));
        r.headers = vec![("Authorization".into(), Secret::new("Bearer secret-token")), ("X-Device-Token".into(), Secret::new("dev"))];
        if m != Method::Get {
            r.bytes = Some(b"payload".to_vec());
        }
        let resp = send(r).unwrap();
        assert_eq!(resp.status, 302, "{m:?}: the redirect is the answer");
    }
    std::thread::sleep(Duration::from_millis(200));
    assert!(elsewhere.seen().is_empty(), "a request went to the redirect target: {:?}", elsewhere.seen());
}

#[test]
fn the_workers_json_error_body_survives() {
    let body = r#"{"error":"formatting_refused","errors":[{"code":"invalid","path":"questions/3","reason":"hidden_text","ack_allowed":true,"message":"Some text is hidden."}],"formatting_findings":[{"q":3}]}"#;
    let srv = serve(move |_, out| reply(out, "422 Unprocessable Entity", &[("Content-Type", "application/json"), ("Retry-After", "7")], body.as_bytes()));
    let mut r = req(Method::Post, srv.url("/items"));
    r.bytes = Some(vec![1]);
    r.content_type = Some("application/vnd.vershion.publish".into());
    let resp = send(r).unwrap();
    assert_eq!(resp.status, 422);
    let d = ErrorDetail::from_response(&resp);
    assert_eq!((d.status, d.code.as_str(), d.reason.as_deref(), d.ack_allowed, d.retry_after), (422, "formatting_refused", Some("hidden_text"), Some(true), Some(7)));
    assert_eq!(d.json["formatting_findings"][0]["q"], 3);
    assert_eq!(d.json["errors"][0]["message"], "Some text is hidden.");
    assert!(!format!("{d:?}").contains("Some text is hidden"));
    assert_eq!(ApiError::from_response(&resp).code(), "formatting_refused");
    // A top-level body works the same.
    let d = ErrorDetail::from_response(&Response::new(409, r#"{"error":"publish_in_progress","reason":"busy","ack_allowed":false,"author_only":true}"#));
    assert_eq!((d.code.as_str(), d.ack_allowed, d.author_only), ("publish_in_progress", Some(false), true));
}

#[test]
fn a_huge_error_page_keeps_its_status_and_drops_its_body() {
    let srv = serve(|_, out| reply(out, "503 Service Unavailable", &[("Retry-After", "3")], &vec![b'x'; 300_000]));
    let r = send(req(Method::Get, srv.url("/x"))).unwrap();
    assert_eq!((r.status, r.retry_after), (503, Some(3)));
    assert!(r.body.expose().is_empty());
    // A small error body still comes through on a binary request.
    let srv = serve(|_, out| reply(out, "404 Not Found", &[], br#"{"error":"not_found"}"#));
    let mut b = req(Method::Get, srv.url("/x"));
    b.binary = true;
    let r = send(b).unwrap();
    assert_eq!(r.status, 404);
    assert_eq!(ApiError::from_response(&r).code(), "not_found");
}

#[test]
fn per_request_timeouts_cover_a_stalled_body() {
    let srv = serve(|_, out| {
        let _ = out.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\npartial");
        std::thread::sleep(Duration::from_secs(10));
    });
    let mut r = req(Method::Get, srv.url("/slow"));
    r.binary = true;
    r.timeout = Some(Duration::from_millis(600));
    let t = Instant::now();
    let e = send(r).unwrap_err();
    assert_eq!(e.kind, NetKind::Offline);
    assert!(t.elapsed() < Duration::from_secs(5), "{:?}", t.elapsed());
    // A longer limit than the 30 s default is accepted (not capped to it).
    let srv = serve(|_, out| reply(out, "200 OK", &[], b"ok"));
    let mut r = req(Method::Get, srv.url("/x"));
    r.timeout = Some(Duration::from_secs(120));
    assert_eq!(send(r).unwrap().body.expose(), "ok");
}

#[test]
fn unsafe_targets_are_refused_before_anything_is_sent() {
    let srv = serve(|_, out| reply(out, "200 OK", &[], b"hi"));
    let creds = vec![("Authorization".to_string(), Secret::new("Bearer t"))];
    let port = srv.port;
    for url in [
        format!("http://user:pw@127.0.0.1:{port}/x"),
        format!("http://user@127.0.0.1:{port}/x"),
        format!("ftp://127.0.0.1:{port}/x"),
        "file:///etc/passwd".into(),
        "not a url".into(),
        String::new(),
    ] {
        let mut r = req(Method::Get, url.clone());
        r.headers = creds.clone();
        let e = send(r).unwrap_err();
        assert!(e.text.starts_with("Request refused"), "{url}: {}", e.text);
    }
    assert!(srv.seen().is_empty());
}

#[test]
fn debug_output_hides_raw_bodies() {
    let mut r = req(Method::Put, "http://127.0.0.1:1/x".into());
    r.bytes = Some(b"super-secret-bytes".to_vec());
    let d = format!("{r:?}");
    assert!(!d.contains("super-secret"), "{d}");
    let mut resp = Response::new(200, "");
    resp.bytes = b"super-secret-bytes".to_vec();
    assert!(!format!("{resp:?}").contains("super-secret"));
}

#[test]
fn route_paths_cannot_name_another_host() {
    use super::client::route_url;
    assert_eq!(route_url("https://api.test", "/me").as_deref(), Some("https://api.test/me"));
    assert_eq!(route_url("https://api.test", "/blobs/ab?x=1").as_deref(), Some("https://api.test/blobs/ab?x=1"));
    for bad in ["@evil.test/x", "//evil.test/x", "evil.test", "", ":8080/x", "/a b", "/a\\b", "/a\r\nHost: x"] {
        assert_eq!(route_url("https://api.test", bad), None, "{bad:?}");
    }
}
