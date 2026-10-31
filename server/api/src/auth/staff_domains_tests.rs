//! Tests for `staff_domains.rs`.
use super::super::admin::tests::{body_of, code, native, obj, world};
use super::*;

#[test]
fn only_a_real_registrable_domain_is_accepted() {
    let ok = |s: &str| registrable_domain(&json!(s));
    assert_eq!(ok(" Chicousd.NET. ").as_deref(), Some("chicousd.net"));
    assert_eq!(ok("pvhs.k12.example.org").as_deref(), Some("pvhs.k12.example.org"));
    assert_eq!(ok("xn--bcher-kva.example").as_deref(), Some("xn--bcher-kva.example"));
    for bad in ["", "com", "a.b", "gmail.com", "k12.ca.us", "co.uk", "proton.me", "-a.example.org", "a-.example.org", "a b.org", "a..org", "x.123", "x.o", "under_score.org"] {
        assert_eq!(ok(bad), None, "{bad:?}");
    }
    assert_eq!(registrable_domain(&json!(7)), None);
    assert_eq!(ok(&format!("{}.org", "a".repeat(64))), None, "a label is at most 63");
}

#[test]
fn admins_put_list_and_delete_hints_with_an_audit_row_and_the_queue_labels_use_them() {
    let w = world();
    let a = w.admin("root@x.org");
    let put_body = obj(json!({ "domain": "School.org", "note": "  Pleasant  Valley  USD " }));
    assert_eq!(code(put(&w.ctx(), &a.read_only, &native(), &put_body)), (403, "sudo_required".into()));
    assert_eq!(body_of(put(&w.ctx(), &a.full, &native(), &put_body).unwrap())["domain"], "school.org");
    put(&w.ctx(), &a.full, &native(), &obj(json!({ "domain": "school.org", "note": "renamed" }))).unwrap();
    let l = body_of(list(&w.ctx(), &a.read_only, &native()).unwrap());
    assert_eq!((l["domains"].as_array().unwrap().len(), l["domains"][0]["note"].as_str()), (1, Some("renamed")));
    assert_eq!(w.count("SELECT COUNT(*) FROM auth_audit WHERE action = 'staff_domain.put'"), 2);
    let conn = w.db.lock().unwrap();
    let got = labels(&conn, &["school.org".to_string(), "other.org".to_string()]);
    assert_eq!(got.get("school.org").map(String::as_str), Some("Staff-domain hint (not proof): renamed"));
    assert!(!got.contains_key("other.org"));
    drop(conn);
    let del = obj(json!({ "domain": "school.org" }));
    assert_eq!(delete(&w.ctx(), &a.full, &native(), &del).unwrap().status(), 200);
    assert_eq!(delete(&w.ctx(), &a.full, &native(), &del).unwrap().status(), 404);
    assert_eq!(w.count("SELECT COUNT(*) FROM auth_audit WHERE action = 'staff_domain.delete'"), 2);
}

#[test]
fn bad_domains_and_notes_are_refused() {
    let w = world();
    let a = w.admin("root@x.org");
    for (body, field) in [
        (json!({ "domain": "gmail.com", "note": "n" }), "domain"),
        (json!({ "note": "n" }), "domain"),
        (json!({ "domain": "school.org" }), "note"),
        (json!({ "domain": "school.org", "note": "\u{202e}x" }), "note"),
    ] {
        let e = put(&w.ctx(), &a.full, &native(), &obj(body)).unwrap_err();
        assert_eq!((e.status, e.extra.get("field")), (400, Some(&json!(field))), "{field}");
    }
    assert_eq!(delete(&w.ctx(), &a.full, &native(), &obj(json!({ "domain": "com" }))).unwrap_err().status, 400);
    assert_eq!(w.count("SELECT COUNT(*) FROM staff_domain_hint"), 0);
}
