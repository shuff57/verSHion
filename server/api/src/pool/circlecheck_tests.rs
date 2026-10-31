//! Tests for `circlecheck.rs` on the real migrated schema: who may publish circle material publicly, the independence statement, and
//! what a refusal names.
use super::*;
use crate::db;
use crate::pool::ctx::Fail;
use rusqlite::Connection;
use std::path::Path;

fn open() -> (Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    (conn, dir)
}
fn id(n: u8) -> String {
    format!("{:032x}", n)
}
fn user(c: &Connection, u: &str) {
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?1, 'teacher', 1)", params![u, format!("{u}@x.org")]).unwrap();
}
fn circle(c: &Connection, n: u8) {
    c.execute("INSERT INTO circles (id, name, created_by, created_at, updated_at, qkeys) VALUES (?1, ?2, 'bob', 1, 1, 1)", params![id(n), format!("Circle {n}")]).unwrap();
}
fn member(c: &Connection, circ: u8, u: &str, left: bool) {
    c.execute(
        "INSERT INTO circle_members (circle_id, user_id, role, joined_at, join_seq, left_at, left_reason) VALUES (?1, ?2, 'editor', 1, ?3, ?4, ?5)",
        params![id(circ), u, circ as i64 * 10 + u.len() as i64, left.then_some(5), left.then_some("left")],
    )
    .unwrap();
}
fn bank(c: &Connection, n: u8, circ: u8, author: &str, license: &str) {
    c.execute("INSERT INTO circle_banks (id, circle_id, author_id, title, license, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, 1, 1)", params![id(n), id(circ), author, format!("Bank {n}"), license]).unwrap();
}
fn qkey(c: &Connection, circ: u8, bank_n: u8, mh: &str, seq: i64, by: &str) {
    c.execute("INSERT INTO circle_qkeys (circle_id, match_hash, bank_id, first_version, first_seq, first_by) VALUES (?1, ?2, ?3, 1, ?4, ?5)", params![id(circ), mh, id(bank_n), seq, by]).unwrap();
}
fn hs(v: &[&[&str]]) -> Vec<Vec<String>> {
    v.iter().map(|p| p.iter().map(|s| s.to_string()).collect()).collect()
}
fn run(c: &Connection, me: &str, hashes: &[&[&str]], short: &[bool], claim: Option<&CircleClaim>) -> Res<Vec<String>> {
    check_circle_material(&PoolCtx::new(c, 1_000), me, &hs(hashes), short, claim)
}
fn refusal(r: Res<Vec<String>>) -> Value {
    match r {
        Err(Fail::Pool(e)) => {
            assert_eq!((e.status, e.code.as_str()), (422, "circle_material"));
            e.body()
        }
        other => panic!("{other:?}"),
    }
}

/// Alice (publisher) and Bob share circle 1; Bob first sent `bobq` in bank 1.
fn world(c: &Connection) {
    for u in ["alice", "bob"] {
        user(c, u);
    }
    circle(c, 1);
    member(c, 1, "alice", false);
    member(c, 1, "bob", false);
    bank(c, 1, 1, "bob", "circle-only");
    qkey(c, 1, 1, "bobq", 10, "bob");
    qkey(c, 1, 1, "aliceq", 11, "alice");
}

#[test]
fn a_claim_is_read_with_its_limits() {
    let m = |v: Value| v.as_object().unwrap().clone();
    assert_eq!(read_circle_claim(&m(json!({}))).unwrap(), None);
    assert_eq!(read_circle_claim(&m(json!({ "circle_independent_of": null }))).unwrap(), None);
    let ok = json!({ "accepted": true, "banks": [id(1), id(1), id(2)], "statement": "I wrote this myself first." });
    let c = read_circle_claim(&m(json!({ "circle_independent_of": ok }))).unwrap().unwrap();
    assert_eq!((c.banks, c.statement.as_str()), (vec![id(1), id(2)], "I wrote this myself first."));
    let field = |v: Value| match read_circle_claim(&m(json!({ "circle_independent_of": v }))) {
        Err(Fail::Pool(e)) => e.body()["field"].as_str().unwrap().to_string(),
        other => panic!("{other:?}"),
    };
    assert_eq!(field(json!({ "accepted": false, "banks": [id(1)], "statement": "long enough here" })), "circle_independent_of");
    assert_eq!(field(json!({ "accepted": true, "banks": [], "statement": "long enough here" })), "circle_independent_of");
    assert_eq!(field(json!({ "accepted": true, "banks": ["nope"], "statement": "long enough here" })), "circle_independent_of");
    assert_eq!(field(json!({ "accepted": true, "banks": vec![id(1); 51], "statement": "long enough here" })), "circle_independent_of");
    assert_eq!(field(json!([])), "circle_independent_of");
    assert_eq!(field(json!({ "accepted": true, "banks": [id(1)], "statement": "short" })), "circle_independent_of.statement");
    assert_eq!(field(json!({ "accepted": true, "banks": [id(1)] })), "circle_independent_of.statement");
}

#[test]
fn a_publisher_who_never_joined_a_circle_passes_and_unrelated_text_passes() {
    let (c, _d) = open();
    world(&c);
    user(&c, "carol");
    assert!(run(&c, "carol", &[&["bobq"]], &[false], None).unwrap().is_empty());
    assert!(run(&c, "alice", &[&["other"]], &[false], None).unwrap().is_empty());
}

#[test]
fn text_first_sent_by_someone_else_is_refused_naming_what_the_publisher_may_see() {
    let (c, _d) = open();
    world(&c);
    let b = refusal(run(&c, "alice", &[&["other"], &["bobq", "x"]], &[false, false], None));
    assert_eq!(b["positions"], json!([1]));
    assert_eq!(b["hits"], json!([{ "pos": 1, "circle_id": id(1), "circle_name": "Circle 1", "bank_id": id(1), "bank_title": "Bank 1", "may_claim_independence": false }]));
    assert!(b["message"].as_str().unwrap().contains("first shared by another member"));
    // Her own first send passes; so does a short statement.
    assert!(run(&c, "alice", &[&["aliceq"]], &[false], None).unwrap().is_empty());
    assert!(run(&c, "alice", &[&["bobq"]], &[true], None).unwrap().is_empty());
}

#[test]
fn text_she_published_publicly_before_the_circle_first_sender_is_hers() {
    let (c, _d) = open();
    world(&c);
    c.execute("INSERT INTO author_firsts (author_id, match_hash, first_seq) VALUES ('alice', 'bobq', 9)", []).unwrap();
    assert!(run(&c, "alice", &[&["bobq"]], &[false], None).unwrap().is_empty(), "hers at seq 9, the circle's first send was 10");
    c.execute("UPDATE author_firsts SET first_seq = 10", []).unwrap();
    refusal(run(&c, "alice", &[&["bobq"]], &[false], None));
}

#[test]
fn an_independence_statement_covers_a_named_bank_unless_she_can_still_read_the_text_elsewhere() {
    let (c, _d) = open();
    world(&c);
    let claim = CircleClaim { banks: vec![id(1)], statement: "I wrote these before joining.".into() };
    // Bank 1 is circle-only and she is an active member who can read it: the statement never covers an exact copy of something she
    // can read, and says so.
    let b = refusal(run(&c, "alice", &[&["bobq"]], &[false], Some(&claim)));
    assert_eq!(b["hits"][0]["may_claim_independence"], json!(false));
    // After she leaves, the text is no longer readable to her: the statement covers it and the bank is reported as claimed.
    c.execute("UPDATE circle_members SET left_at = 5, left_reason = 'left' WHERE user_id = 'alice'", []).unwrap();
    assert_eq!(run(&c, "alice", &[&["bobq"]], &[false], Some(&claim)).unwrap(), vec![id(1)]);
    // Without naming the bank it is refused, with ids but no names (a former member).
    let b = refusal(run(&c, "alice", &[&["bobq"]], &[false], Some(&CircleClaim { banks: vec![id(9)], ..claim.clone() })));
    assert_eq!(b["hits"][0], json!({ "pos": 0, "circle_id": id(1), "circle_name": null, "bank_id": id(1), "bank_title": null, "may_claim_independence": true }));
}

#[test]
fn a_held_bank_or_a_suspended_author_gives_a_generic_hit() {
    let (c, _d) = open();
    world(&c);
    let generic = json!({ "pos": 0, "circle_id": null, "circle_name": null, "bank_id": null, "bank_title": null, "may_claim_independence": false });
    c.execute("INSERT INTO circle_holds (bank_id, reason, source, at) VALUES (?1, 'reports', '', 1)", [id(1)]).unwrap();
    assert_eq!(refusal(run(&c, "alice", &[&["bobq"]], &[false], None))["hits"][0], generic);
    c.execute("DELETE FROM circle_holds", []).unwrap();
    c.execute("UPDATE users SET suspended_at = 3 WHERE id = 'bob'", []).unwrap();
    assert_eq!(refusal(run(&c, "alice", &[&["bobq"]], &[false], None))["hits"][0], generic);
}

#[test]
fn the_earliest_hit_across_circles_wins_and_former_membership_still_counts() {
    let (c, _d) = open();
    world(&c);
    user(&c, "dan");
    circle(&c, 2);
    member(&c, 2, "alice", true);
    member(&c, 2, "dan", false);
    bank(&c, 2, 2, "dan", "circle-only");
    qkey(&c, 2, 2, "bobq", 4, "dan");
    qkey(&c, 2, 2, "danq", 3, "dan");
    // Circle 2 (where she is a former member) first saw bobq at seq 4: that is the earliest hit.
    let b = refusal(run(&c, "alice", &[&["bobq"]], &[false], None));
    assert_eq!(b["hits"][0]["bank_id"], json!(id(2)));
    assert_eq!(b["hits"][0]["circle_id"], json!(id(2)));
    assert_eq!(b["hits"][0]["bank_title"], json!(null), "a former member sees ids, not names");
    // A circle with no records (qkeys = 0) is not consulted.
    c.execute("UPDATE circles SET qkeys = 0 WHERE id = ?1", [id(2)]).unwrap();
    assert_eq!(refusal(run(&c, "alice", &[&["bobq"]], &[false], None))["hits"][0]["bank_id"], json!(id(1)));
}

#[test]
fn the_circle_sql_fragments_count_and_match_membership() {
    use crate::pool::circlesql::*;
    let (c, _d) = open();
    world(&c);
    let one = |sql: String| -> i64 { c.query_row(&format!("SELECT {sql}"), [], |r| r.get(0)).unwrap() };
    let cid = format!("'{}'", id(1));
    assert_eq!(one(memberships_sql("'alice'")), 1);
    assert_eq!(one(members_sql(&cid)), 2);
    assert_eq!(one(owned_sql("'alice'")), 0);
    assert_eq!(one(membership_rows_sql("'alice'")), 1);
    assert_eq!(one(active_sql(&cid, "'alice'", &["editor"])), 1);
    assert_eq!(one(active_sql(&cid, "'alice'", &["owner"])), 0);
    assert_eq!(one(open_sql(&cid)), 1);
    assert_eq!(one(in_standing_sql("'bob'")), 1);
    c.execute("UPDATE users SET suspended_at = 1 WHERE id = 'bob'", []).unwrap();
    assert_eq!(one(in_standing_sql("'bob'")), 0);
    assert_eq!(one(open_invites_sql(&cid, "1000", false)), 0);
}
