//! Tests for `circlemail.rs`, replaying the Worker's round 46 / 48 cases: one mailbox is one address, plain refusals, the hour's caps,
//! rotation of an open invite and the masked label.
use super::*;
use crate::db;
use crate::pool::circles::create_circle;
use rusqlite::Connection;
use std::path::Path;

fn world() -> (Connection, tempfile::TempDir, String, PoolKeys) {
    let d = tempfile::tempdir().unwrap();
    let mut c = db::open(&d.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    for u in ["a", "b", "c"] {
        c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?3, 'teacher', 1)", params![u, format!("{u}@x.org"), format!("Teacher {u}")]).unwrap();
    }
    let body = json!({ "name": "Team" }).as_object().unwrap().clone();
    let cid = create_circle(&PoolCtx::new(&c, 1_000), "a", &body).unwrap().1["circle"]["id"].as_str().unwrap().to_string();
    (c, d, cid, PoolKeys::from_secret("test-secret-0123456789abcdef0123456789abcdef"))
}
fn limit(c: &Connection, metric: &str, v: i64, user: &str) {
    c.execute("INSERT OR REPLACE INTO pool_limits (metric, user_id, value, updated_at, updated_by) VALUES (?1, ?2, ?3, 1, 'test')", params![metric, user, v]).unwrap();
}
fn invite(c: &Connection, k: &PoolKeys, now: i64, me: &str, cid: &str, email: &str, role: &str) -> Result<Value, (u16, String, Value)> {
    let x = PoolCtx::new(c, now);
    let b = json!({ "email": email, "role": role }).as_object().unwrap().clone();
    email_invite(&x, k, me, cid, &b).map(|r| r.1).map_err(|f| match f {
        Fail::Pool(p) => (p.status, p.code.clone(), p.body()),
        o => panic!("{o:?}"),
    })
}
fn n(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn addresses_are_normalised_and_one_mailbox_is_one_address() {
    let s = |v: &str| normalise_invite_email(&json!(v));
    assert_eq!(s(" Vic.Tim@GMAIL.com. ").as_deref(), Some("vic.tim@gmail.com"));
    assert_eq!(s("ｖｉｃｔｉｍ@gmail.com").as_deref(), Some("victim@gmail.com"));
    assert_eq!(s("kim@bücher.example").as_deref(), Some("kim@xn--bcher-kva.example"));
    for bad in ["nobody", "@x.org", "a b@x.org", "a@x", "a@@x.org", "a@-x.org", "a(b)@x.org"] {
        assert_eq!(s(bad), None, "{bad}");
    }
    assert_eq!(normalise_invite_email(&json!(5)), None);
    assert_eq!(normalise_invite_email(&json!("a".repeat(321))), None);
    assert!(is_deliverable("kim@school.example") && !is_deliverable("kim@host.123") && !is_deliverable("kim@x"));
    for f in ["victim@gmail.com", "vic.tim@gmail.com", "v.i.c.t.i.m@googlemail.com", "victim+x@gmail.com"] {
        assert_eq!(email_cap_key(f), "victim@gmail.com", "{f}");
    }
    assert_eq!(email_cap_key("a.b+c@school.example"), "a.b@school.example");
    assert_eq!((mask_email("jane.doe@gmail.com"), mask_email("j@gmail.com")), ("j***@gmail.com".to_string(), "***@gmail.com".to_string()));
}

#[test]
fn one_mailbox_gets_three_mails_a_day_and_the_inviter_is_told_plainly() {
    let (c, _d, cid, k) = world();
    let forms = ["victim@gmail.com", "Vic.Tim@GMAIL.com", "v.i.c.t.i.m@googlemail.com", "victim+x@gmail.com", "victim@gmail.com.", " victim@gmail.com ", "ｖｉｃｔｉｍ@gmail.com", "VICTIM@googlemail.com."];
    limit(&c, "circle_email_invites_hourly", 100, "a");
    let out: Vec<_> = forms.iter().map(|e| invite(&c, &k, 5_000, "a", &cid, e, "viewer").map(|b| b["emailed"].clone()).map_err(|e| (e.0, e.1, e.2["reason"].clone(), e.2["emailed"].clone()))).collect();
    // The first goes; the next two ROTATE its open invite (one live code per address), and the rest are refused.
    assert_eq!(out[..3].iter().map(|r| r.clone().unwrap()).collect::<Vec<_>>(), vec![json!(true); 3]);
    for r in &out[3..] {
        assert_eq!(r.clone().unwrap_err(), (422, "invite_not_sent".to_string(), json!("address_limit"), json!(false)));
    }
    assert_eq!(n(&c, "SELECT COUNT(*) FROM circle_invites WHERE kind = 'email'"), 1, "nothing made for a mail that does not go; rotation reuses the invite");
    assert_eq!(n(&c, "SELECT COUNT(*) FROM circle_email_sends"), 1);
    let nd = invite(&c, &k, 5_000, "a", &cid, "kim@host.123", "viewer").unwrap_err();
    assert_eq!((nd.0, nd.1.as_str(), nd.2["reason"].clone()), (422, "invite_not_sent", json!("not_deliverable")));
    // The day's counts were taken once per mail that went, none for the ones refused.
    assert_eq!(n(&c, "SELECT count FROM usage_daily WHERE user_id = 'a' AND metric = 'circle_email_invites'"), 3);
}

#[test]
fn a_second_invite_rotates_the_open_one_and_the_label_is_masked() {
    let (c, _d, cid, k) = world();
    let r = invite(&c, &k, 5_000, "a", &cid, "Jane.Doe@Gmail.com", "viewer").unwrap();
    assert_eq!((r["address_label"].clone(), r["kind"].clone(), r["max_uses"].clone(), r.get("resent").cloned()), (json!("j***@gmail.com"), json!("email"), json!(1), None));
    let h1: String = c.query_row("SELECT code_hash FROM circle_invites", [], |r| r.get(0)).unwrap();
    let r2 = invite(&c, &k, 6_000, "a", &cid, "janedoe+x@gmail.com", "editor").unwrap();
    assert_eq!((r2["resent"].clone(), r2["invite_id"].clone(), r2["role"].clone(), r2["address_label"].clone()), (json!(true), r["invite_id"].clone(), json!("editor"), json!("j***@gmail.com")));
    let (h2, role): (String, String) = c.query_row("SELECT code_hash, role FROM circle_invites", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert!(h1 != h2 && role == "editor");
    // A different mailbox with the same label gets " #2".
    let r3 = invite(&c, &k, 7_000, "a", &cid, "j.else@gmail.com", "viewer").unwrap();
    assert_eq!(r3["address_label"], json!("j***@gmail.com #2"));
    // The rotated invite is no new code: two codes counted, three mails.
    assert_eq!(n(&c, "SELECT count FROM usage_daily WHERE metric = 'circle_invites'"), 2);
    assert_eq!(n(&c, "SELECT count FROM usage_daily WHERE metric = 'circle_email_invites'"), 3);
    // The address is in the notice only, never in a row.
    assert_eq!(n(&c, "SELECT COUNT(*) FROM circle_invites WHERE email_key LIKE '%gmail%' OR email_label LIKE '%jane%'"), 0);
}

#[test]
fn the_hour_has_per_inviter_per_domain_and_service_caps() {
    let (c, _d, cid, k) = world();
    limit(&c, "circle_email_invites_hourly", 10, "a");
    let mut st = vec![];
    for i in 0..11 {
        st.push(invite(&c, &k, 5_000, "a", &cid, &format!("person{i}@school{i}.example"), "viewer").map_err(|e| (e.0, e.1, e.2["metric"].clone())));
    }
    assert!(st[..10].iter().all(Result::is_ok));
    assert_eq!(st[10].clone().unwrap_err(), (429, "hourly_limit".to_string(), json!("circle_email_invites_hourly")));
    // The refused one gave its day count back; the next hour goes.
    assert_eq!(n(&c, "SELECT count FROM usage_daily WHERE user_id = 'a' AND metric = 'circle_email_invites'"), 10);
    assert!(invite(&c, &k, 5_000 + 3_600_000, "a", &cid, "late@school.example", "viewer").is_ok());
    // One domain: at most circle_email_domain_hourly an hour from everyone.
    let (c2, _d2, cid2, k2) = world();
    limit(&c2, "circle_email_domain_hourly", 2, "*");
    assert!(invite(&c2, &k2, 5_000, "a", &cid2, "a1@gmail.com", "viewer").is_ok());
    // (one inviter has her own share of a domain, so the second and third mails come from other owners, as in the Worker)
    let cb = create_circle(&PoolCtx::new(&c2, 1_000), "b", &json!({ "name": "B" }).as_object().unwrap().clone()).unwrap().1["circle"]["id"].as_str().unwrap().to_string();
    let cc = create_circle(&PoolCtx::new(&c2, 1_000), "c", &json!({ "name": "C" }).as_object().unwrap().clone()).unwrap().1["circle"]["id"].as_str().unwrap().to_string();
    assert!(invite(&c2, &k2, 5_000, "b", &cb, "a2@googlemail.com", "viewer").is_ok());
    let dom = invite(&c2, &k2, 5_000, "c", &cc, "a3@gmail.com", "viewer").unwrap_err();
    assert_eq!((dom.0, dom.1.as_str(), dom.2["metric"].clone()), (429, "hourly_limit", json!("circle_email_domain_hourly")));
    assert!(invite(&c2, &k2, 5_000, "c", &cc, "a3@school.example", "viewer").is_ok());
    // The service's hour: the last fifth is kept for inviters who sent little.
    let (c3, _d3, cid3, k3) = world();
    limit(&c3, "circle_email_hourly", 5, "*");
    for i in 0..5 {
        let _ = invite(&c3, &k3, 5_000, "a", &cid3, &format!("h{i}@d{i}.example"), "viewer");
    }
    assert_eq!(n(&c3, "SELECT COALESCE(SUM(sent), 0) FROM pool_alert_budget WHERE channel = 'circle_email_invite'"), 4);
    let busy = invite(&c3, &k3, 5_000, "a", &cid3, "h5@d5.example", "viewer").unwrap_err();
    assert_eq!((busy.0, busy.1.as_str()), (503, "busy"));
}

#[test]
fn only_the_owner_asks_and_the_request_is_checked() {
    let (c, _d, cid, k) = world();
    c.execute("INSERT INTO circle_members (circle_id, user_id, role, joined_at, join_seq) VALUES (?1, 'b', 'editor', 10, 5)", [&cid]).unwrap();
    assert_eq!(invite(&c, &k, 5_000, "b", &cid, "x@school.example", "viewer").unwrap_err().1, "circle_role_required");
    assert_eq!(invite(&c, &k, 5_000, "c", &cid, "x@school.example", "viewer").unwrap_err().0, 404);
    assert_eq!(invite(&c, &k, 5_000, "a", &cid, "not an address", "viewer").unwrap_err().1, "invalid_request");
    assert_eq!(invite(&c, &k, 5_000, "a", &cid, "x@school.example", "owner").unwrap_err().1, "invalid_request");
    let x = PoolCtx::new(&c, 5_000);
    let extra = json!({ "email": "x@school.example", "role": "viewer", "note": "hi" }).as_object().unwrap().clone();
    assert!(email_invite(&x, &k, "a", &cid, &extra).is_err());
    assert_eq!(n(&c, "SELECT COUNT(*) FROM usage_daily WHERE metric LIKE 'circle_%invites'"), 0);
}
