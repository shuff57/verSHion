//! Tests for `licensecheck.rs` and `chain.rs`: the pure marker rules, and the void-item stand-ins over a real schema.
use super::*;
use crate::db;
use std::path::Path;

fn link(item: &str, license: &str, modified: bool) -> ChainLink {
    ChainLink { author_name: "Ann".into(), item_id: item.into(), license: license.into(), modified, ..Default::default() }
}
fn st(allowed: &[&str], binders: &[&str]) -> Option<StoredAllowed> {
    Some(StoredAllowed { allowed: allowed.iter().map(|s| s.to_string()).collect(), binders: binders.iter().map(|s| s.to_string()).collect(), ..Default::default() })
}
const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn open() -> (Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    (conn, dir)
}

#[test]
fn a_chain_allows_what_every_link_allows() {
    assert_eq!(chain_allows(&[]), LICENSES.to_vec());
    assert_eq!(chain_allows(&[link(A, "CC-BY-4.0", false)]), vec!["CC-BY-4.0"], "a verbatim link keeps its own licence");
    assert_eq!(chain_allows(&[link(A, "CC-BY-4.0", true)]), vec!["CC-BY-4.0", "CC-BY-SA-4.0", "CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"]);
    assert_eq!(chain_allows(&[link(A, "CC0-1.0", false)]), LICENSES.to_vec(), "CC0 may be relabelled even verbatim");
    assert_eq!(chain_allows(&[link(A, "CC-BY-4.0", true), link(B, "CC-BY-NC-4.0", true)]), vec!["CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"]);
    let collapsed = ChainLink { collapsed_licenses: Some(vec!["CC-BY-SA-4.0".into(), "junk".into()]), ..Default::default() };
    assert_eq!(chain_allows(&[collapsed]), vec!["CC-BY-SA-4.0"], "a collapsed run still grants, and an unknown licence is ignored");
}

#[test]
fn the_marker_is_ok_mismatch_or_one_of_the_unverified_kinds_worst_first() {
    let none = HashMap::new();
    let ok = license_check("CC-BY-4.0", &[vec![]], &[st(&["CC-BY-4.0", "CC0-1.0"], &[])], &none);
    assert_eq!((ok.status, ok.positions), ("ok", None));
    let bad = license_check("CC-BY-4.0", &[vec![], vec![]], &[st(&["CC-BY-4.0"], &[]), st(&["CC-BY-NC-4.0"], &[])], &none);
    assert_eq!(bad.status, "mismatch");
    assert_eq!(bad.positions, Some(vec![Position { pos: 1, allowed: vec!["CC-BY-NC-4.0"] }]));
    // Without a record the displayed chain decides.
    let chain_only = license_check("CC0-1.0", &[vec![link(A, "CC-BY-4.0", true)]], &[None], &none);
    assert_eq!(chain_only.status, "mismatch");
    let mut soft = st(&["CC-BY-4.0"], &[]).unwrap();
    soft.soft = true;
    let mut claimed = soft.clone();
    claimed.claimed = true;
    let r = license_check("CC-BY-4.0", &[vec![], vec![]], &[None, Some(claimed)], &none);
    assert_eq!(r.status, "unverified", "soft outranks claimed");
    assert_eq!(r.positions, Some(vec![Position { pos: 1, allowed: vec![] }]));
    let mut earlier = st(&["CC-BY-4.0"], &[]).unwrap();
    earlier.earlier = true;
    assert_eq!(license_check("CC-BY-4.0", &[vec![]], &[Some(earlier)], &none).status, "unverified_earlier_match");
}

#[test]
fn a_void_binder_narrows_a_question_to_its_stand_in_licence() {
    let mut stand = HashMap::new();
    stand.insert(A.to_string(), ChainLink { item_id: B.into(), license: "CC-BY-NC-4.0".into(), modified: true, ..Default::default() });
    // Recorded as allowing CC-BY, but its deciding source A was found infringing and tied to B (NC): CC-BY no longer fits.
    let r = license_check("CC-BY-4.0", &[vec![link(B, "CC-BY-NC-4.0", true)]], &[st(&["CC-BY-4.0", "CC-BY-NC-4.0"], &[A])], &stand);
    assert_eq!(r.status, "mismatch");
    assert_eq!(r.positions.unwrap()[0].allowed, vec!["CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"][..1].to_vec());
    // A verbatim link to the stand-in in the chain keeps exactly its licence.
    let r = license_check("CC-BY-NC-SA-4.0", &[vec![link(B, "CC-BY-NC-4.0", false)]], &[st(&["CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"], &[A])], &stand);
    assert_eq!(r.status, "mismatch");
}

#[test]
fn stored_allowed_reads_only_a_complete_record() {
    assert_eq!(stored_allowed(None), None);
    assert_eq!(stored_allowed(Some("")), None);
    assert_eq!(stored_allowed(Some("not json")), None);
    assert_eq!(stored_allowed(Some(r#"{"allowed":["CC0-1.0"]}"#)), None, "binders missing");
    let s = stored_allowed(Some(r#"{"allowed":["CC0-1.0"],"binders":["x"],"soft":true}"#)).unwrap();
    assert!(s.soft && !s.claimed && s.binders == ["x"]);
}

fn user(c: &Connection, id: &str, name: &str, deleted: bool) {
    if deleted {
        c.execute("INSERT INTO users (id, trust, created_at, deleted_at) VALUES (?1, 'teacher', 1, 5)", [id]).unwrap();
    } else {
        c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?3, 'teacher', 1)", params![id, format!("{id}@x.org"), name]).unwrap();
    }
}
fn item(c: &Connection, id: &str, author: &str, license: &str) {
    c.execute("INSERT INTO items (id, author_id, kind, visibility, current_publish_id, license, public_version, created_at, updated_at) VALUES (?1, ?2, 'bank', 'public', 'p', ?3, 2, 1, 1)", params![id, author, license]).unwrap();
}

#[test]
fn void_links_are_dropped_or_replaced_by_the_rightful_item_and_deleted_names_are_hidden() {
    let (c, _d) = open();
    user(&c, "ua", "Ann", false);
    user(&c, "ub", "Bob", false);
    user(&c, "ud", "", true);
    let (r, v, d, plain) = ("1".repeat(32), "2".repeat(32), "3".repeat(32), "4".repeat(32));
    item(&c, &r, "ub", "CC-BY-NC-4.0");
    item(&c, &v, "ua", "CC0-1.0");
    item(&c, &d, "ud", "CC-BY-4.0");
    item(&c, &plain, "ua", "CC-BY-4.0");
    c.execute("INSERT INTO void_items (item_id, at, reason, replaced_by, replaced_license, replaced_author_id) VALUES (?1, 1, 'dmca', ?2, 'CC-BY-NC-4.0', 'ub')", params![v, r]).unwrap();
    let chain = vec![link(&plain, "CC-BY-4.0", true), link(&v, "CC0-1.0", false), link(&d, "CC-BY-4.0", true), link(&plain, "CC-BY-4.0", true)];
    let shown = display_chains(&c, &[chain]).unwrap().remove(0);
    let ids: Vec<&str> = shown.iter().map(|l| l.item_id.as_str()).collect();
    assert_eq!(ids, vec![plain.as_str(), r.as_str(), d.as_str()], "the void link shows as the rightful item, the repeat is dropped");
    assert_eq!((shown[1].author_name.as_str(), shown[1].license.as_str(), shown[1].modified, shown[1].version), ("Bob", "CC-BY-NC-4.0", false, 2), "licence at void time, modified as the chain recorded it");
    assert_eq!(shown[2].author_name, "deleted teacher");
    // A void item with no stand-in is simply dropped.
    c.execute("INSERT INTO void_items (item_id, at, reason) VALUES (?1, 1, 'dmca')", [&plain]).unwrap();
    let shown = display_chains(&c, &[vec![link(&plain, "CC-BY-4.0", true), link("", "", true)]]).unwrap().remove(0);
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].item_id, "", "a placeholder without an item stays");
    let info = void_info(&c, &[v.clone(), plain.clone(), "nope".into()]).unwrap();
    assert_eq!(info.voided.len(), 2);
    assert!(info.replaced.contains_key(&v) && !info.replaced.contains_key(&plain));
}

#[test]
fn a_stand_in_that_is_not_visible_is_a_neutral_label_with_the_same_licence() {
    let (c, _d) = open();
    user(&c, "ub", "Bob", false);
    let (r, v) = ("1".repeat(32), "2".repeat(32));
    item(&c, &r, "ub", "CC-BY-NC-4.0");
    c.execute("UPDATE items SET visibility = 'unlisted' WHERE id = ?1", [&r]).unwrap();
    c.execute("INSERT INTO void_items (item_id, at, reason, replaced_by, replaced_license, replaced_author_id) VALUES (?1, 1, 'dmca', ?2, 'CC-BY-NC-4.0', 'ub')", params![v, r]).unwrap();
    let l = void_info(&c, &[v.clone()]).unwrap().replaced.remove(&v).unwrap();
    assert_eq!((l.author_name.as_str(), l.item_id.as_str(), l.license.as_str(), l.version), ("the rightful author", "", "CC-BY-NC-4.0", 0));
}

#[test]
fn the_check_of_a_stored_item_reads_its_current_publish() {
    let (c, _d) = open();
    user(&c, "ua", "Ann", false);
    let it = "5".repeat(32);
    item(&c, &it, "ua", "CC0-1.0");
    c.execute("UPDATE items SET current_publish_id = 'cur' WHERE id = ?1", [&it]).unwrap();
    let cols = c.prepare("SELECT * FROM item_questions LIMIT 0").unwrap();
    let names: Vec<String> = cols.column_names().iter().map(|s| s.to_string()).collect();
    drop(cols);
    assert!(names.iter().any(|n| n == "allowed_json") && names.iter().any(|n| n == "chain_json"));
    let required: Vec<String> = {
        let mut st = c.prepare("SELECT name FROM pragma_table_info('item_questions') WHERE \"notnull\" = 1 AND dflt_value IS NULL").unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    // Fill every required column with a neutral value, then set the ones the check reads.
    let cols_sql = required.join(", ");
    let vals: Vec<String> = required.iter().map(|n| if n == "publish_id" { "'cur'".to_string() } else if n == "pos" { "0".to_string() } else { "''".to_string() }).collect();
    // Integer-typed required columns get 0 (SQLite is lax about '' in INTEGER NOT NULL, but be explicit when it matters).
    c.execute(&format!("INSERT INTO item_questions ({cols_sql}) VALUES ({})", vals.join(", ")), []).unwrap();
    c.execute("UPDATE item_questions SET chain_json = ?1, allowed_json = ?2 WHERE publish_id = 'cur'", params![serde_json::to_string(&vec![link(&"6".repeat(32), "CC-BY-4.0", false)]).unwrap(), r#"{"allowed":["CC-BY-4.0"],"binders":[]}"#]).unwrap();
    let m = license_check_of(&c, &[it.clone()]).unwrap();
    assert_eq!(m[&it].status, "mismatch", "the item is CC0 but its recorded sources allow only CC-BY");
    c.execute("UPDATE items SET license = 'CC-BY-4.0' WHERE id = ?1", [&it]).unwrap();
    assert_eq!(license_check_of(&c, &[it.clone()]).unwrap()[&it].status, "ok");
    assert!(license_check_of(&c, &[]).unwrap().is_empty());
}
