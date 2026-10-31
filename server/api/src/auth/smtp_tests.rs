//! Tests for `smtp.rs`: settings, rendering, header safety and a scripted SMTP server.
use super::*;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

fn env_of(pairs: &[(&str, &str)]) -> Result<MailEnv, String> {
    let m: HashMap<String, String> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    MailEnv::from_lookup(|k| m.get(k).cloned())
}

#[test]
fn the_sender_address_is_bare_and_the_port_and_tls_follow_the_settings() {
    assert_eq!(smtp_from(Some("verSHion <pool@x.org>"), "box@x.org"), "pool@x.org");
    assert_eq!(smtp_from(Some(" pool@x.org "), "box@x.org"), "pool@x.org");
    assert_eq!(smtp_from(None, " box@x.org "), "box@x.org");
    assert_eq!(smtp_from(Some("  "), "box@x.org"), "box@x.org");
    assert_eq!((smtp_port(None), smtp_port(Some("587")), smtp_port(Some("0")), smtp_port(Some("70000")), smtp_port(Some("x"))), (465, 587, 465, 465, 465));
    assert!(implicit_tls(None, 465) && !implicit_tls(None, 587));
    assert!(implicit_tls(Some("ON"), 587) && !implicit_tls(Some("starttls"), 465));
}

#[test]
fn smtp_is_chosen_only_when_all_three_settings_are_present() {
    let base = [("SMTP_HOSTNAME", "mail.x.org"), ("SMTP_USERNAME", "box@x.org"), ("SMTP_PASSWORD", "abcd efgh")];
    let e = env_of(&base).unwrap();
    assert!(e.smtp);
    assert_eq!((e.password.as_str(), e.from.as_str(), e.tls), ("abcdefgh", "box@x.org", Some(Tls::Implicit)), "whitespace is stripped from the password");
    assert!(!env_of(&base[..2]).unwrap().smtp);
    assert_eq!(send_raw(&env_of(&base[..2]).unwrap(), "a@b.org", "s", "t"), Err("mail not configured".into()));
    let e = env_of(&[("ADMIN_EMAIL", "a@x.org, b@x.org,,"), ("ESCALATION_EMAIL", "c@x.org")]).unwrap();
    assert_eq!((e.admin_emails.len(), e.escalation_emails.len()), (2, 1));
}

#[test]
fn log_mode_refuses_anything_but_a_local_app() {
    assert!(env_of(&[("MAIL_MODE", "log"), ("APP_URL", "http://localhost:5173")]).is_ok());
    assert!(env_of(&[("MAIL_MODE", "log"), ("APP_URL", "http://127.0.0.1:8080/x")]).is_ok());
    assert!(env_of(&[("MAIL_MODE", "log"), ("APP_URL", "http://[::1]:8080")]).is_ok());
    for url in ["https://versh.example", "", "http://localhost.evil.org", "http://localhost@evil.org"] {
        assert!(env_of(&[("MAIL_MODE", "log"), ("APP_URL", url)]).is_err(), "{url:?}");
    }
    let e = env_of(&[("MAIL_MODE", "log"), ("APP_URL", "http://localhost")]).unwrap();
    assert_eq!(send_raw(&e, "a@b.org", "s", "t"), Ok(()));
}

#[test]
fn links_carry_the_token_in_the_fragment_and_each_kind_has_its_text() {
    let e = env_of(&[("APP_URL", "https://app.example")]).unwrap();
    let (s, t) = render(&e, &MailMessage::new(MailKind::Verify, "a@b.org").with_token("TOK", 1));
    assert_eq!(s, "Confirm your verSHion address");
    assert!(t.contains("https://app.example/account/verify#token=TOK") && t.contains("expires in 24 hours"));
    let (_, t) = render(&e, &MailMessage::new(MailKind::Reset, "a@b.org").with_token("R", 1));
    assert!(t.contains("/account/reset#token=R") && t.contains("expires in 1 hour"));
    let (_, t) = render(&e, &MailMessage::new(MailKind::DeviceEnrol, "a@b.org").with_token("D", 1));
    assert!(t.contains("/account/enrol#token=D") && t.contains("within 15 minutes"));
    let mut m = MailMessage::new(MailKind::SignupExisting, "a@b.org");
    m.account_verified = Some(true);
    assert!(render(&e, &m).1.contains("already has one"));
    m.account_verified = Some(false);
    assert!(render(&e, &m).1.contains("never confirmed"));
    let mut m = MailMessage::new(MailKind::AccountSuspended, "a@b.org");
    m.reason = Some("spam".into());
    assert!(render(&e, &m).1.contains("\n\nspam\n\n"));
    for k in [MailKind::PasswordChanged, MailKind::ApplicationApproved, MailKind::ApplicationRejected, MailKind::ApprovalRevoked, MailKind::AccountRestored] {
        let (s, t) = render(&e, &MailMessage::new(k, "a@b.org"));
        assert!(!s.is_empty() && !t.is_empty());
    }
}

#[test]
fn a_line_break_in_a_value_cannot_start_another_header() {
    assert_eq!(header_value("a\r\nBcc: x@y.org"), "a Bcc: x@y.org");
    let e = env_of(&[("SMTP_HOSTNAME", "h"), ("SMTP_USERNAME", "box@x.org"), ("SMTP_PASSWORD", "p"), ("SMTP_FROM_NAME", "Ver\r\nBcc: z@y.org")]).unwrap();
    let raw = String::from_utf8(build_message(&e, "to@x.org", "Hi\r\nBcc: evil@y.org", ".dot first\r\nline").unwrap().formatted()).unwrap();
    let head = raw.split("\r\n\r\n").next().unwrap();
    assert!(!head.lines().any(|l| l.to_lowercase().starts_with("bcc:")), "{head}");
    assert!(build_message(&e, "not an address", "s", "t").is_err());
}

/// A scripted SMTP server that accepts one message with AUTH LOGIN, and records what it was told.
fn serve(final_reply: &'static str) -> (u16, Arc<Mutex<Vec<String>>>, std::thread::JoinHandle<()>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    let h = std::thread::spawn(move || {
        let (mut s, _) = l.accept().unwrap();
        let mut r = BufReader::new(s.try_clone().unwrap());
        s.write_all(b"220 test ESMTP\r\n").unwrap();
        let mut in_data = false;
        let mut line = String::new();
        loop {
            line.clear();
            if r.read_line(&mut line).unwrap() == 0 {
                return;
            }
            let l = line.trim_end().to_string();
            log.lock().unwrap().push(l.clone());
            if in_data {
                if l == "." {
                    in_data = false;
                    s.write_all(final_reply.as_bytes()).unwrap();
                }
                continue;
            }
            let up = l.to_uppercase();
            let reply: &[u8] = if up.starts_with("EHLO") {
                b"250-test\r\n250 AUTH LOGIN PLAIN\r\n"
            } else if up.starts_with("AUTH LOGIN") {
                b"334 VXNlcm5hbWU6\r\n"
            } else if up.starts_with("MAIL") || up.starts_with("RCPT") {
                b"250 ok\r\n"
            } else if up == "DATA" {
                in_data = true;
                b"354 go\r\n"
            } else if up == "QUIT" {
                s.write_all(b"221 bye\r\n").unwrap();
                return;
            } else if l.len() > 3 && !up.contains(' ') {
                // The base64 username, then the password.
                if log.lock().unwrap().iter().filter(|x| !x.contains(' ') && x.len() > 3).count() == 1 { b"334 UGFzc3dvcmQ6\r\n" } else { b"235 ok\r\n" }
            } else {
                b"500 what\r\n"
            };
            s.write_all(reply).unwrap();
        }
    });
    (port, seen, h)
}

fn local_env(port: u16) -> MailEnv {
    let mut e = env_of(&[("SMTP_HOSTNAME", "127.0.0.1"), ("SMTP_USERNAME", "box@x.org"), ("SMTP_PASSWORD", "secret"), ("MAIL_FROM", "verSHion <pool@x.org>"), ("APP_URL", "https://app.example")]).unwrap();
    e.port = port;
    e.tls = Some(Tls::None);
    e
}

#[test]
fn a_message_goes_through_submission_with_a_login_and_the_bare_sender() {
    let (port, seen, h) = serve("250 queued\r\n");
    let mailer = SmtpMailer { env: local_env(port) };
    mailer.send(&MailMessage::new(MailKind::Verify, "teacher@school.org").with_token("TOK", 1)).unwrap();
    h.join().unwrap();
    let log = seen.lock().unwrap().join("\n");
    assert!(log.contains("MAIL FROM:<pool@x.org>"), "{log}");
    assert!(log.contains("RCPT TO:<teacher@school.org>"), "{log}");
    assert!(log.contains("To: teacher@school.org") || log.contains("To: <teacher@school.org>"), "{log}");
    // The body is transfer-encoded (quoted-printable turns "=" into "=3D"), so look for either spelling.
    assert!(log.contains("#token=TOK") || log.contains("#token=3DTOK"), "{log}");
}

#[test]
fn a_refusal_is_reported_as_a_class_without_the_address_or_the_text() {
    let (port, _seen, h) = serve("550 mailbox unavailable teacher@school.org\r\n");
    let e = SmtpMailer { env: local_env(port) }.send(&MailMessage::new(MailKind::PasswordChanged, "teacher@school.org")).unwrap_err();
    h.join().unwrap();
    assert_eq!(e, "SMTP refused");
    assert!(!e.contains("school.org"));
}
