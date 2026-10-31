//! Tests for `list.rs` on the real migrated schema: who sees what, the filters, the word and tag shortlists, and paging.
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
fn user(c: &Connection, id: &str) {
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?1, 'teacher', 1)", params![id, format!("{id}@x.org")]).unwrap();
}
fn id(n: u8) -> String {
    format!("{:032x}", n)
}
/// A public item by `author`, created at `at`, with its publish id `p<n>`.
fn item(c: &Connection, n: u8, author: &str, at: i64) {
    c.execute(
        "INSERT INTO items (id, author_id, kind, visibility, current_publish_id, public_publish_id, public_version, version, title, subject, subject_fold, course, grade, kinds_mask,
            question_count, license, has_images, committed_at, created_at, updated_at, fold_rev)
         VALUES (?1, ?2, 'bank', 'public', ?3, ?3, 2, 3, ?4, 'Math', 'math', 'alg', '8', 1, 1, 'CC-BY-4.0', 0, 100, ?5, ?5, 1)",
        params![id(n), author, format!("p{n}"), format!("Item {n}"), at],
    )
    .unwrap();
}
fn words(c: &Connection, n: u8, ws: &[&str]) {
    for w in ws {
        c.execute("INSERT INTO served_words (publish_id, word) VALUES (?1, ?2)", params![format!("p{n}"), w]).unwrap();
    }
}
fn keys() -> PoolKeys {
    PoolKeys::from_secret("test-secret-0123456789abcdef0123456789abcdef")
}
fn q() -> ListQuery {
    ListQuery::default()
}
fn run(c: &Connection, me: &str, query: &ListQuery) -> Value {
    list(&PoolCtx::new(c, 1_000), &keys(), me, query, SEARCH_CANDIDATES).unwrap()
}
fn ids(v: &Value) -> Vec<String> {
    v["items"].as_array().unwrap().iter().map(|i| i["id"].as_str().unwrap().to_string()).collect()
}
fn field(r: Res<Value>) -> String {
    match r {
        Err(Fail::Pool(e)) => {
            assert_eq!((e.status, e.code.as_str()), (400, "invalid_request"));
            e.extra["field"].as_str().unwrap().to_string()
        }
        other => panic!("{other:?}"),
    }
}
fn setup() -> (Connection, tempfile::TempDir) {
    let (c, d) = open();
    for u in ["a", "r"] {
        user(&c, u);
    }
    (c, d)
}

#[test]
fn the_public_pool_is_listed_newest_first_and_hides_what_others_may_not_see() {
    let (c, _d) = setup();
    item(&c, 1, "a", 10);
    item(&c, 2, "a", 30);
    item(&c, 3, "a", 20);
    item(&c, 4, "a", 40);
    item(&c, 5, "a", 50);
    c.execute("UPDATE items SET visibility = 'unlisted' WHERE id = ?1", [id(4)]).unwrap();
    c.execute("INSERT INTO item_holds (item_id, reason, at) VALUES (?1, 'admin', 1)", [id(5)]).unwrap();
    let out = run(&c, "r", &q());
    assert_eq!(ids(&out), vec![id(2), id(3), id(1)]);
    let first = &out["items"][0];
    assert_eq!((first["version"].clone(), first["committed_at"].clone(), first["kinds"].clone()), (json!(2), json!(100), json!(["Mc"])));
    assert!(first.get("is_author").is_none() && first.get("visibility").is_none());
    assert_eq!(out["next_cursor"], Value::Null);
    // In the public scope the author is also shown their own held item (visibleTo lets an author see their own), but not
    // their unlisted one; scope=mine lists all of their live items.
    assert_eq!(ids(&run(&c, "a", &q())), vec![id(5), id(2), id(3), id(1)]);
    let mine = run(&c, "a", &ListQuery { scope: Some("mine".into()), ..q() });
    assert_eq!(ids(&mine), vec![id(5), id(4), id(2), id(3), id(1)]);
    assert_eq!((mine["items"][0]["is_author"].clone(), mine["items"][1]["visibility"].clone()), (json!(true), json!("unlisted")));
    assert_eq!(mine["items"][0]["version"], json!(3), "the author sees the current version");
    assert!(ids(&run(&c, "r", &ListQuery { scope: Some("mine".into()), ..q() })).is_empty());
}

#[test]
fn paging_by_a_signed_cursor_walks_every_item_once() {
    let (c, _d) = setup();
    for n in 1..=5u8 {
        item(&c, n, "a", n as i64 * 10);
    }
    let p1 = run(&c, "r", &ListQuery { limit: Some("2".into()), ..q() });
    assert_eq!(ids(&p1), vec![id(5), id(4)]);
    let cur = p1["next_cursor"].as_str().unwrap().to_string();
    let p2 = run(&c, "r", &ListQuery { limit: Some("2".into()), cursor: Some(cur), ..q() });
    assert_eq!(ids(&p2), vec![id(3), id(2)]);
    let p3 = run(&c, "r", &ListQuery { limit: Some("2".into()), cursor: p2["next_cursor"].as_str().map(String::from), ..q() });
    assert_eq!(ids(&p3), vec![id(1)]);
    assert_eq!(p3["next_cursor"], Value::Null);
    // Equal creation times page on the id.
    c.execute("UPDATE items SET created_at = 7", []).unwrap();
    let a = run(&c, "r", &ListQuery { limit: Some("2".into()), ..q() });
    let b = run(&c, "r", &ListQuery { limit: Some("2".into()), cursor: a["next_cursor"].as_str().map(String::from), ..q() });
    let mut all = ids(&a);
    all.extend(ids(&b));
    assert_eq!(all, vec![id(5), id(4), id(3), id(2)]);
}

#[test]
fn a_limit_above_fifty_is_fifty_and_a_forged_cursor_is_a_400() {
    let (c, _d) = setup();
    for n in 1..=3u8 {
        item(&c, n, "a", n as i64);
    }
    assert_eq!(ids(&run(&c, "r", &ListQuery { limit: Some("500".into()), ..q() })).len(), 3);
    assert_eq!(field(list(&PoolCtx::new(&c, 1), &keys(), "r", &ListQuery { cursor: Some("forged.cursor".into()), ..q() }, 10)), "cursor");
}

#[test]
fn subject_kind_license_standard_and_tag_filters_narrow_the_list() {
    let (c, _d) = setup();
    for n in 1..=4u8 {
        item(&c, n, "a", n as i64 * 10);
    }
    c.execute("UPDATE items SET subject = 'Science', subject_fold = 'science' WHERE id = ?1", [id(1)]).unwrap();
    c.execute("UPDATE items SET kinds_mask = 4 WHERE id = ?1", [id(2)]).unwrap();
    c.execute("UPDATE items SET license = 'CC0-1.0' WHERE id = ?1", [id(3)]).unwrap();
    c.execute("INSERT INTO item_tags (publish_id, tag, fold) VALUES ('p4', 'Photo-synthesis', 'photo synthesis')", []).unwrap();
    c.execute("INSERT INTO item_tags (publish_id, tag, fold) VALUES ('p4', 'plain', 'plain')", []).unwrap();
    c.execute("INSERT INTO item_standards (publish_id, code) VALUES ('p3', 'CCSS.8.EE')", []).unwrap();
    let with = |f: fn(&mut ListQuery)| {
        let mut x = q();
        f(&mut x);
        ids(&run(&c, "r", &x))
    };
    assert_eq!(with(|x| x.subject = Some("SCIENCE".into())), vec![id(1)]);
    assert_eq!(with(|x| x.kind = Some("Matching".into())), vec![id(2)]);
    assert_eq!(with(|x| x.license = Some("CC0-1.0".into())), vec![id(3)]);
    assert_eq!(with(|x| x.standard = Some("  ccss.8.ee ".into())), vec![id(3)]);
    assert_eq!(with(|x| x.tag = Some("PLAIN".into())), vec![id(4)]);
    assert_eq!(with(|x| x.tag = Some("photo synthesis".into())), vec![id(4)], "a tag also matches by its folded form");
    assert_eq!(with(|x| x.tag = Some("missing".into())), Vec::<String>::new());
}

#[test]
fn a_text_query_needs_every_word_to_start_a_served_word_and_uses_the_served_publish() {
    let (c, _d) = setup();
    for n in 1..=3u8 {
        item(&c, n, "a", n as i64 * 10);
    }
    words(&c, 1, &["fractions", "decimals"]);
    words(&c, 2, &["fractions", "geometry"]);
    words(&c, 3, &["decimals"]);
    let find = |text: &str| ids(&run(&c, "r", &ListQuery { q: Some(text.into()), ..q() }));
    assert_eq!(find("frac"), vec![id(2), id(1)]);
    assert_eq!(find("Fractions DECIMALS"), vec![id(1)], "every word must match");
    assert_eq!(find("deci frac"), vec![id(1)]);
    assert_eq!(find("zzz"), Vec::<String>::new());
    // An unlisted item never makes the shortlist for others, but does for its author in scope=mine.
    c.execute("UPDATE items SET visibility = 'unlisted' WHERE id = ?1", [id(1)]).unwrap();
    assert_eq!(find("decimals"), vec![id(3)]);
    let mine = ids(&run(&c, "a", &ListQuery { q: Some("decimals".into()), scope: Some("mine".into()), ..q() }));
    assert_eq!(mine, vec![id(3), id(1)]);
    // A hidden-key item is indexed on the publish others are served, not the author's newer one.
    c.execute("UPDATE items SET public_publish_id = 'old', current_publish_id = 'p3' WHERE id = ?1", [id(3)]).unwrap();
    assert!(!find("decimals").contains(&id(3)));
}

#[test]
fn the_shortlist_is_bounded_by_the_candidate_limit_newest_first() {
    let (c, _d) = setup();
    for n in 1..=4u8 {
        item(&c, n, "a", n as i64 * 10);
        words(&c, n, &["alpha"]);
    }
    let out = list(&PoolCtx::new(&c, 1), &keys(), "r", &ListQuery { q: Some("alpha".into()), ..q() }, 2).unwrap();
    assert_eq!(ids(&out), vec![id(4), id(3)]);
}

#[test]
fn bad_parameters_name_the_field_in_the_workers_order() {
    let (c, _d) = setup();
    let bad = |f: fn(&mut ListQuery)| {
        let mut x = q();
        f(&mut x);
        field(list(&PoolCtx::new(&c, 1), &keys(), "r", &x, 10))
    };
    assert_eq!(bad(|x| x.scope = Some("all".into())), "scope");
    for l in ["0", "-1", "1.5", "abc", ""] {
        let mut x = q();
        x.limit = Some(l.into());
        assert_eq!(field(list(&PoolCtx::new(&c, 1), &keys(), "r", &x, 10)), "limit", "{l:?}");
    }
    assert_eq!(bad(|x| x.subject = Some(String::new())), "subject");
    assert_eq!(bad(|x| x.subject = Some("x".repeat(101))), "subject");
    assert_eq!(bad(|x| x.kind = Some("Essay".into())), "kind");
    assert_eq!(bad(|x| x.tag = Some("t".repeat(65))), "tag");
    assert_eq!(bad(|x| x.standard = Some(String::new())), "standard");
    assert_eq!(bad(|x| x.q = Some("!!! ---".into())), "q", "a query with no letters or digits");
    assert_eq!(bad(|x| x.q = Some(String::new())), "q");
    assert_eq!(bad(|x| x.license = Some("GPL".into())), "license");
}

#[test]
fn a_browse_is_charged_to_the_daily_allowance_and_refused_when_it_is_used_up() {
    let (c, _d) = setup();
    c.execute("INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('r', 'browse', 1, 1, 'a')", []).unwrap();
    let x = PoolCtx::new(&c, 1_000);
    list(&x, &keys(), "r", &q(), 10).unwrap();
    match list(&x, &keys(), "r", &q(), 10) {
        Err(Fail::Pool(e)) => assert_eq!(e.status, 429),
        other => panic!("{other:?}"),
    }
}
