//! Tests for `notices.rs`: budget, backlog flush, alert and reminder texts (mail captured through log mode is not
//! inspectable, so the senders are checked against a scripted SMTP server).
use super::*;
use crate::db;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{Arc, Mutex};

const NOW: i64 = 1_800_000_000_000;

fn open() -> (Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    (conn, dir)
}

/// Accepts connections until dropped; records each DATA payload's Subject and recipients.
fn sink() -> (MailEnv, Arc<Mutex<Vec<String>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for conn in l.incoming() {
            let Ok(mut s) = conn else { return };
            let mut r = BufReader::new(s.try_clone().unwrap());
            let _ = s.write_all(b"220 t\r\n");
            let (mut data, mut line, mut users) = (false, String::new(), 0);
            loop {
                line.clear();
                if r.read_line(&mut line).unwrap_or(0) == 0 {
                    break;
                }
                let l = line.trim_end().to_string();
                if data {
                    if l == "." {
                        data = false;
                        let _ = s.write_all(b"250 ok\r\n");
                    } else {
                        log.lock().unwrap().push(l);
                    }
                    continue;
                }
                let up = l.to_uppercase();
                let rep: &[u8] = if up.starts_with("EHLO") {
                    b"250-t\r\n250 AUTH LOGIN\r\n"
                } else if up.starts_with("AUTH") {
                    b"334 VXNlcm5hbWU6\r\n"
                } else if up.starts_with("RCPT") {
                    log.lock().unwrap().push(l.clone());
                    b"250 ok\r\n"
                } else if up.starts_with("MAIL") {
                    b"250 ok\r\n"
                } else if up == "DATA" {
                    data = true;
                    b"354 go\r\n"
                } else if up == "QUIT" {
                    let _ = s.write_all(b"221 bye\r\n");
                    break;
                } else {
                    users += 1;
                    if users == 1 { b"334 UGFzc3dvcmQ6\r\n" } else { b"235 ok\r\n" }
                };
                let _ = s.write_all(rep);
            }
        }
    });
    let mut e = MailEnv::from_lookup(|k| {
        [("SMTP_HOSTNAME", "127.0.0.1"), ("SMTP_USERNAME", "box@x.org"), ("SMTP_PASSWORD", "p"), ("ADMIN_EMAIL", "a1@x.org,a2@x.org"), ("ESCALATION_EMAIL", "esc@x.org")].iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
    })
    .unwrap();
    e.port = port;
    e.tls = Some(super::super::smtp::Tls::None);
    (e, seen)
}

fn rcpts(seen: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
    seen.lock().unwrap().iter().filter(|l| l.to_uppercase().starts_with("RCPT")).cloned().collect()
}

#[test]
fn the_first_five_notices_in_an_hour_are_sent_and_the_rest_are_counted() {
    let (c, _d) = open();
    for _ in 0..5 {
        assert_eq!(new_application_budget(&c, NOW).unwrap(), Notice::Sent);
    }
    assert_eq!(new_application_budget(&c, NOW).unwrap(), Notice::Deferred);
    assert_eq!(new_application_budget(&c, NOW + 10).unwrap(), Notice::Deferred);
    assert_eq!(c.query_row("SELECT pending FROM notice_backlog", [], |r| r.get::<_, i64>(0)).unwrap(), 2);
    assert_eq!(new_application_budget(&c, NOW + 3_600_000).unwrap(), Notice::Sent, "a new hour, a new budget");
}

#[test]
fn a_notice_goes_to_the_admins_only_and_the_flush_sends_one_summary() {
    let (mut c, _d) = open();
    let (env, seen) = sink();
    assert_eq!(notice_new_application(&c, &env, "u1", NOW).unwrap(), Notice::Sent);
    let r = rcpts(&seen);
    assert_eq!(r.len(), 2);
    assert!(r.iter().all(|x| !x.contains("esc@x.org")));
    assert!(seen.lock().unwrap().iter().any(|l| l.contains("Account u1 confirmed")));
    for _ in 0..7 {
        notice_new_application(&c, &env, "u", NOW).unwrap();
    }
    seen.lock().unwrap().clear();
    assert_eq!(flush_notice_backlog(&mut c, &env, NOW).unwrap(), 3);
    assert!(seen.lock().unwrap().iter().any(|l| l.contains("3 more account(s) joined")));
    seen.lock().unwrap().clear();
    assert_eq!(flush_notice_backlog(&mut c, &env, NOW).unwrap(), 0);
    assert!(seen.lock().unwrap().is_empty(), "nothing pending, nothing sent");
    c.execute("INSERT INTO notice_budget (hour, sent) VALUES (1, 1)", []).unwrap();
    flush_notice_backlog(&mut c, &env, NOW).unwrap();
    assert_eq!(c.query_row("SELECT COUNT(*) FROM notice_budget WHERE hour = 1", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
}

#[test]
fn an_admin_alert_reaches_everyone_including_the_escalation_contact() {
    let (env, seen) = sink();
    admin_alert(&env, &AdminEvent { action: "admin_suspension_proposed", actor_id: Some("adm1"), target_id: "adm2", at: NOW, proposal_id: Some("p9") }).unwrap();
    assert_eq!(rcpts(&seen).len(), 3);
    let all = seen.lock().unwrap().join("\n");
    assert!(all.contains("Admin adm1 proposed suspending the admin account adm2"), "{all}");
    assert!(all.contains("Proposal p9"), "{all}");
    assert!(all.contains("ALERT: admin action (admin_suspension_proposed)"), "{all}");
}

#[test]
fn escalation_and_digest_texts_name_the_hours_and_the_decisions() {
    let (env, seen) = sink();
    let esc = Reminder { open: 3, oldest_age_seconds: 50 * 3600 + 5, kind: ReminderKind::Escalation, approved_last_day: 0, rejected_last_day: 0, by_admin: vec![] };
    queue_reminder(&env, &esc).unwrap();
    assert_eq!(rcpts(&seen).len(), 3);
    let all = seen.lock().unwrap().join("\n");
    assert!(all.contains("3 application(s) waiting, the oldest for 50 h"), "{all}");
    seen.lock().unwrap().clear();
    let dig = Reminder { open: 1, oldest_age_seconds: 7200, kind: ReminderKind::Digest, approved_last_day: 4, rejected_last_day: 1, by_admin: vec![("adm1".into(), 4, 1)] };
    queue_reminder(&env, &dig).unwrap();
    let all = seen.lock().unwrap().join("\n");
    assert!(all.contains("Last 24 h: 4 approved, 1 rejected."), "{all}");
    assert!(all.contains("admin adm1: 4 approved, 1 rejected"), "{all}");
}
