//! Tests for `publishunits.rs`: an image unit found in a publish the account was served binds as served; the account's own earlier use exempts it; an
//! occurrence in an item never served is the "relay" path; unit-key and preview limits.
use super::*;
use crate::db;
use crate::pool::publishfacts::Facts;
use rusqlite::Connection;
use rusqlite::types::Value as Sql;
use std::path::Path;

const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const S_ITEM: &str = "11111111111111111111111111111111";
const PS: &str = "SSSSSSSSSSSSSSSSSSSSSS";

fn world() -> (Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut c = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    for (id, name) in [("me", "me"), ("src", "Source Teacher")] {
        c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?3, 'teacher', 1)", params![id, format!("{id}@x.org"), name]).unwrap();
    }
    (c, dir)
}
fn row(c: &Connection, table: &str, set: &[(&str, Sql)]) {
    let cols: Vec<(String, String)> = c
        .prepare(&format!("SELECT name, type FROM pragma_table_info('{table}') WHERE \"notnull\" = 1 AND dflt_value IS NULL"))
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(|x| x.unwrap())
        .collect();
    let (mut names, mut vals): (Vec<String>, Vec<Sql>) = (vec![], vec![]);
    for (n, t) in cols {
        if set.iter().any(|(k, _)| *k == n) {
            continue;
        }
        names.push(n);
        vals.push(if t.to_uppercase().contains("INT") { 0.into() } else { String::new().into() });
    }
    for (k, v) in set {
        names.push(k.to_string());
        vals.push(v.clone());
    }
    let q = vec!["?"; names.len()].join(",");
    c.execute(&format!("INSERT INTO {table} ({}) VALUES ({q})", names.join(",")), rusqlite::params_from_iter(vals)).unwrap();
}
fn s(v: &str) -> Sql {
    v.to_string().into()
}

/// A public item by `src` whose current publish carries the image; optionally served to `me`.
fn source(c: &Connection, served: bool) {
    row(c, "items", &[("id", s(S_ITEM)), ("author_id", s("src")), ("kind", s("bank")), ("visibility", s("public")), ("current_publish_id", s(PS)), ("public_publish_id", s(PS)), ("license", s("CC-BY-4.0")), ("public_version", 1.into())]);
    row(c, "item_publishes", &[("publish_id", s(PS)), ("item_id", s(S_ITEM)), ("author_id", s("src")), ("status", s("current")), ("hide_key", 0.into()), ("committed_at", 5.into())]);
    row(c, "blobs", &[("sha256", s(SHA)), ("content_type", s("image/png"))]);
    row(c, "item_blobs", &[("publish_id", s(PS)), ("form", s("full")), ("sha256", s(SHA)), ("seq", 3.into())]);
    if served {
        row(c, "served", &[("reader_id", s("me")), ("item_id", s(S_ITEM)), ("publish_id", s(PS)), ("kind", s("view")), ("license", s("CC-BY-4.0")), ("licenses_json", s("[\"CC-BY-4.0\"]"))]);
    }
}
fn facts_with_image() -> Facts {
    Facts { img_at: vec![Some(SHA.into())], served_text: vec![vec![]], hidden_text: vec![vec![]], unit_keys: vec![vec![]], ..Default::default() }
}
fn look(c: &Connection, f: &Facts, dry: bool, max: usize) -> Res<Units> {
    lookup(&PoolCtx::new(c, 1000), "me", dry, &AuthorCtx::default(), f, max)
}

#[test]
fn no_units_means_no_lookups() {
    let (c, _d) = world();
    let f = Facts { img_at: vec![None], served_text: vec![vec![]], hidden_text: vec![vec![]], unit_keys: vec![vec![]], ..Default::default() };
    let u = look(&c, &f, false, 10).unwrap();
    assert!(u.lifts.is_empty() && u.units_at == vec![vec![]]);
}

#[test]
fn an_image_in_a_publish_the_account_was_served_binds_as_served() {
    let (c, _d) = world();
    source(&c, true);
    let u = look(&c, &facts_with_image(), false, 10).unwrap();
    let l = &u.lifts[&0];
    assert_eq!(l.len(), 1);
    let l = &l[0];
    assert_eq!((l.item_id.as_str(), l.kind, l.served, l.ev_kind, l.seq, l.stock, l.own_first, l.visible), (S_ITEM, UnitKind::Image, true, Some("view"), 3, false, false, true));
    assert!(l.received.contains(&"CC-BY-4.0".to_string()));
    assert_eq!((l.author_name.as_str(), l.license.as_str(), l.version), ("Source Teacher", "CC-BY-4.0", 1));
}

#[test]
fn the_accounts_own_earlier_use_of_the_unit_exempts_it() {
    let (c, _d) = world();
    source(&c, true);
    row(&c, "author_contents", &[("author_id", s("me")), ("kind", s("unit")), ("hash", s(SHA)), ("first_seq", 1.into())]);
    assert!(look(&c, &facts_with_image(), false, 10).unwrap().lifts.is_empty());
}

#[test]
fn an_occurrence_in_an_item_never_served_is_found_but_not_as_served() {
    let (c, _d) = world();
    source(&c, false);
    let u = look(&c, &facts_with_image(), false, 10).unwrap();
    let l = &u.lifts[&0][0];
    assert_eq!((l.served, l.ev_kind, l.received.len()), (false, None, 0));
}

#[test]
fn the_accounts_own_items_are_never_sources() {
    let (c, _d) = world();
    source(&c, true);
    c.execute("UPDATE items SET author_id = 'me'", []).unwrap();
    let f = facts_with_image();
    let x = PoolCtx::new(&c, 1000);
    let mut a = AuthorCtx::default();
    a.mine.insert(S_ITEM.into());
    assert!(lookup(&x, "me", false, &a, &f, 10).unwrap().lifts.is_empty());
}

#[test]
fn too_many_distinct_units_are_refused() {
    let (c, _d) = world();
    let f = Facts { img_at: vec![None, None], served_text: vec![vec!["a".into(), "b".into()], vec!["b".into(), "c".into()]], hidden_text: vec![vec![], vec![]], unit_keys: vec![vec![], vec![]], ..Default::default() };
    let e = match look(&c, &f, false, 2) {
        Err(crate::pool::ctx::Fail::Pool(p)) => p,
        other => panic!("{other:?}"),
    };
    assert_eq!((e.status, e.code.as_str(), e.extra["limit"].clone()), (422, "too_many_units", json!(2)));
    assert!(look(&c, &f, false, 3).is_ok(), "b is counted once");
}

#[test]
fn a_hidden_text_is_a_unit_too_and_a_previews_lookups_are_charged() {
    let (c, _d) = world();
    let f = Facts { img_at: vec![None], served_text: vec![vec!["t1".into()]], hidden_text: vec![vec!["t2".into()]], unit_keys: vec![vec![]], ..Default::default() };
    let u = look(&c, &f, true, 10).unwrap();
    assert_eq!(u.units_at[0].iter().map(|x| (x.key.as_str(), x.hidden)).collect::<Vec<_>>(), vec![("t1", false), ("t2", true)]);
    let used: i64 = c.query_row("SELECT count FROM usage_daily WHERE user_id = 'me' AND metric = 'preview_units'", [], |r| r.get(0)).unwrap();
    assert_eq!(used, 2);
}
