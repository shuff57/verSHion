//! Tests for `admin_read.rs`: queue, user detail, audit log, stats and the review reminder.
use super::super::admin::tests::{World, approve_body, body_of, code, native, world};
use super::*;
use crate::auth::admin::approve;
use crate::auth::crypto::uuid_v4;
use crate::auth::mail::Outbox;

fn q(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}
fn no_labels(_: &Connection, _: &[String]) -> HashMap<String, String> {
    HashMap::new()
}
fn queue_of(w: &World, h: &HeaderMap, pairs: &[(&str, &str)]) -> Result<Value, HttpError> {
    queue(&w.ctx(), h, &native(), &q(pairs), &no_labels).map(body_of)
}

#[test]
fn the_queue_lists_open_applications_oldest_first_with_snapshots_and_labels() {
    let w = world();
    let a = w.admin("root@x.org");
    let (u1, r1) = w.applicant("a@school.org");
    let (_u2, r2) = w.applicant("b@other.org");
    w.db.lock().unwrap().execute("UPDATE approval_requests SET requested_at = 1000 WHERE id = ?1", [&r1]).unwrap();
    w.db.lock().unwrap().execute("UPDATE approval_requests SET requested_at = 2000, display_name_snapshot = 'Snap Name', profile_snapshot = '{\"school\":\"S\"}' WHERE id = ?1", [&r2]).unwrap();
    w.db.lock().unwrap().execute("UPDATE users SET display_name = 'Live Name' WHERE email = 'b@other.org'", []).unwrap();
    w.db.lock().unwrap().execute("INSERT INTO domain_decisions (domain, approved, rejected) VALUES ('school.org', 4, 1)", []).unwrap();
    let labels = |_: &Connection, d: &[String]| -> HashMap<String, String> { d.iter().filter(|x| *x == "school.org").map(|x| (x.clone(), "Pleasant Valley staff domain".to_string())).collect() };
    let b = body_of(queue(&w.ctx(), &a.read_only, &native(), &q(&[]), &labels).unwrap());
    assert_eq!(b["open"], 2);
    let items = b["items"].as_array().unwrap();
    assert_eq!((items[0]["request_id"].as_str(), items[1]["request_id"].as_str()), (Some(r1.as_str()), Some(r2.as_str())));
    assert_eq!((items[0]["user_id"].as_str(), items[0]["status"].as_str(), items[0]["email_domain"].as_str()), (Some(u1.as_str()), Some("open"), Some("school.org")));
    assert_eq!(items[0]["server_labels"]["domain_hint"], "Pleasant Valley staff domain");
    assert_eq!(items[0]["server_labels"]["domain_history"], json!({ "approved": 4, "rejected": 1 }));
    assert_eq!(items[1]["server_labels"]["domain_history"], json!({ "approved": 0, "rejected": 0 }));
    assert_eq!(items[1]["server_labels"]["profile_changed"], true, "the live name differs from the submitted snapshot");
    assert_eq!((items[1]["applicant"]["display_name"].as_str(), items[1]["applicant"]["profile"]["school"].as_str()), (Some("Snap Name"), Some("S")));
    assert_eq!(items[0]["server_labels"]["profile_changed"], false);
}

#[test]
fn the_queue_filters_pages_and_validates_its_parameters() {
    let w = world();
    let a = w.admin("root@x.org");
    for i in 0..5 {
        let (_, r) = w.applicant(&format!("t{i}@d{}.org", i % 2));
        w.db.lock().unwrap().execute("UPDATE approval_requests SET requested_at = ?1 WHERE id = ?2", params![1000 + i, r]).unwrap();
    }
    let page1 = queue_of(&w, &a.read_only, &[("limit", "2")]).unwrap();
    assert_eq!(page1["items"].as_array().unwrap().len(), 2);
    let cur = page1["next_cursor"].as_str().unwrap().to_string();
    let page2 = queue_of(&w, &a.read_only, &[("limit", "2"), ("cursor", &cur)]).unwrap();
    assert_eq!(page2["items"].as_array().unwrap().len(), 2);
    let page3 = queue_of(&w, &a.read_only, &[("limit", "2"), ("cursor", page2["next_cursor"].as_str().unwrap())]).unwrap();
    assert_eq!((page3["items"].as_array().unwrap().len(), page3["next_cursor"].clone()), (1, Value::Null));
    assert_eq!(queue_of(&w, &a.read_only, &[("domain", "d0.org")]).unwrap()["items"].as_array().unwrap().len(), 3);
    assert_eq!(queue_of(&w, &a.read_only, &[("min_age_seconds", "1000000000")]).unwrap()["items"].as_array().unwrap().len(), 5, "all are old enough");
    assert_eq!(queue_of(&w, &a.read_only, &[("max_age_seconds", "1")]).unwrap()["items"].as_array().unwrap().len(), 0);
    for (k, v) in [("status", "bogus"), ("domain", "Bad Domain!"), ("cursor", "nope"), ("min_age_seconds", "-1")] {
        let e = queue_of(&w, &a.read_only, &[(k, v)]).unwrap_err();
        assert_eq!((e.status, e.extra.get("field")), (400, Some(&json!(k))), "{k}");
    }
    // Decided applications leave the open list and appear under their status.
    let (u, r) = w.applicant("z@x.org");
    approve(&w.ctx(), &a.full, &native(), &approve_body(&u, &r), &mut Outbox::new()).unwrap();
    assert_eq!(queue_of(&w, &a.read_only, &[("status", "approved")]).unwrap()["items"][0]["request_id"].as_str(), Some(r.as_str()));
    assert_eq!(queue_of(&w, &a.read_only, &[("status", "all")]).unwrap()["items"].as_array().unwrap().len(), 6);
    assert_eq!(code(queue(&w.ctx(), &HeaderMap::new(), &native(), &q(&[]), &no_labels).map(|_| Response::default())).0, 401);
}

#[test]
fn user_detail_shows_the_account_its_applications_and_its_audit_trail() {
    let w = world();
    let a = w.admin("root@x.org");
    let (u, r) = w.applicant("t@x.org");
    approve(&w.ctx(), &a.full, &native(), &approve_body(&u, &r), &mut Outbox::new()).unwrap();
    let b = body_of(user_detail(&w.ctx(), &a.read_only, &native(), &u).unwrap());
    assert_eq!((b["user"]["email"].as_str(), b["user"]["trust"].as_str()), (Some("t@x.org"), Some("teacher")));
    assert_eq!((b["requests"][0]["decision"].as_str(), b["requests"][0]["note"].as_str()), (Some("approved"), Some("checked")), "the admin sees reviewer notes");
    assert_eq!((b["audit"][0]["action"].as_str(), b["audit"][0]["detail"]["request_id"].as_str()), (Some("approve"), Some(r.as_str())));
    assert!(b["user"].get("pw_hash").is_none());
    assert_eq!(code(user_detail(&w.ctx(), &a.read_only, &native(), "not-an-id").map(|_| Response::default())), (404, "not_found".into()));
    assert_eq!(code(user_detail(&w.ctx(), &a.read_only, &native(), &uuid_v4()).map(|_| Response::default())), (404, "not_found".into()));
}

#[test]
fn the_audit_log_pages_newest_first_with_a_stable_cursor() {
    let w = world();
    let a = w.admin("root@x.org");
    for i in 0..5 {
        w.db.lock().unwrap().execute("INSERT INTO auth_audit (id, at, actor_kind, action, detail) VALUES (?1, ?2, 'system', 'tick', ?3)", params![uuid_v4(), 5000, format!("{{\"n\":{i}}}")]).unwrap();
    }
    let page = |before: Option<&str>| {
        let mut pairs = vec![("limit", "2")];
        if let Some(b) = before {
            pairs.push(("before", b));
        }
        body_of(audit_log(&w.ctx(), &a.read_only, &native(), &q(&pairs)).unwrap())
    };
    let p1 = page(None);
    assert_eq!(p1["entries"].as_array().unwrap().len(), 2);
    assert_eq!(p1["entries"][0]["detail"]["n"], 4, "newest first, ties by insertion order");
    let p2 = page(p1["next_before"].as_str());
    assert_eq!((p2["entries"][0]["detail"]["n"].clone(), p2["entries"][1]["detail"]["n"].clone()), (json!(2), json!(1)));
    let p3 = page(p2["next_before"].as_str());
    assert!(p3["entries"].as_array().unwrap().iter().any(|e| e["detail"]["n"] == 0) && p3["next_before"].is_null() == (p3["entries"].as_array().unwrap().len() <= 2));
    let e = audit_log(&w.ctx(), &a.read_only, &native(), &q(&[("before", "x")])).unwrap_err();
    assert_eq!((e.status, e.extra.get("field")), (400, Some(&json!("before"))));
}

#[test]
fn the_review_reminder_escalates_when_overdue_and_otherwise_sends_a_digest_only_if_enabled() {
    let w = world();
    let a = w.admin("root@x.org");
    let conn = w.db.lock().unwrap();
    assert!(review_reminder(&conn, 1_800_000_000_000, 48 * 3600, false).unwrap().is_none(), "nothing open, no digest");
    let d = review_reminder(&conn, 1_800_000_000_000, 48 * 3600, true).unwrap().unwrap();
    assert_eq!((d.kind, d.open), (ReminderKind::Digest, 0));
    drop(conn);
    let (_u, r) = w.applicant("t@x.org");
    w.db.lock().unwrap().execute("UPDATE approval_requests SET requested_at = ?1 WHERE id = ?2", params![1_800_000_000_000i64 - 49 * 3_600_000, r]).unwrap();
    let conn = w.db.lock().unwrap();
    let e = review_reminder(&conn, 1_800_000_000_000, 48 * 3600, false).unwrap().unwrap();
    assert_eq!((e.kind, e.open, e.oldest_age_seconds), (ReminderKind::Escalation, 1, 49 * 3600));
    conn.execute("INSERT INTO approval_requests (id, user_id, requested_at, decided_at, decided_by, decision) VALUES ('x1', ?1, 1, ?2, ?3, 'approved'), ('x2', ?1, 2, ?2, ?3, 'rejected')", params![a.id, 1_800_000_000_000i64 - 1000, a.id]).unwrap();
    let e = review_reminder(&conn, 1_800_000_000_000, 48 * 3600, false).unwrap().unwrap();
    assert_eq!((e.approved_last_day, e.rejected_last_day, e.by_admin.len()), (1, 1, 1));
    let s = stats(&conn, 1_800_000_000_000).unwrap();
    assert_eq!((s.open, s.oldest_age_seconds), (1, 49 * 3600));
}
