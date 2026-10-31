//! Tests for `hit.rs`: the bearing rules and licence units as provenance.ts reads them, the chain rewrite, and the two
//! small queries prepared against the real migrated schema.
use super::*;
use crate::db;
use std::path::Path;

fn hit() -> ImportHit {
    ImportHit { item_id: "src".into(), kind: "view".into(), lic_all: vec!["CC-BY-4.0".into(), "CC0-1.0".into()], source_first_seq: 7, ..Default::default() }
}
fn link(id: &str, lic: &str, modified: bool) -> ChainLink {
    ChainLink { item_id: id.into(), license: lic.into(), modified, author_name: "A".into(), ..Default::default() }
}
fn set(v: &[&str]) -> HashSet<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn bearing_follows_near_then_import_or_declared_and_a_claimed_read_binds_nothing() {
    let mut h = hit();
    assert_eq!(bearing_of(&h, false), Bearing::Soft);
    assert!(!is_hard(&h, false));
    assert_eq!(bearing_of(&h, true), Bearing::Adapted);
    h.kind = "import".into();
    assert_eq!(bearing_of(&h, false), Bearing::Adapted);
    h.near = true;
    assert_eq!(bearing_of(&h, false), Bearing::Verbatim);
    assert!(is_hard(&h, false));
    h.claimed = true;
    assert_eq!(bearing_of(&h, true), Bearing::Soft, "an attested-independent read binds nothing, even near and declared");
}

#[test]
fn a_unit_reads_near_licences_for_a_verbatim_source_and_per_content_groups_otherwise() {
    let mut h = hit();
    h.chain_lics = vec!["CC-BY-4.0".into()];
    h.contents = vec!["c1".into()];
    let u = unit_of(&h);
    assert!(!u.verbatim);
    assert_eq!((u.item.as_str(), u.seq), ("src", 7));
    assert_eq!(u.own, vec![h.lic_all.clone()], "no groups: every licence shown");
    h.lic_groups = vec![vec!["CC0-1.0".into()], vec!["CC-BY-4.0".into()]];
    assert_eq!(unit_of(&h).own, h.lic_groups);
    h.near = true;
    assert_eq!(unit_of(&h).own, vec![h.lic_all.clone()], "verbatim without near licences falls back to all");
    h.lic_near = vec!["CC-BY-4.0".into()];
    let u = unit_of(&h);
    assert!(u.verbatim);
    assert_eq!(u.own, vec![vec!["CC-BY-4.0".to_string()]]);
    assert_eq!((u.chain, u.contents), (vec!["CC-BY-4.0".to_string()], vec!["c1".to_string()]));
}

#[test]
fn a_chain_drops_void_links_swaps_stand_ins_and_keeps_each_item_once() {
    let links = vec![link("a", "CC0-1.0", false), link("v", "CC-BY-4.0", true), link("a", "CC0-1.0", true), link("w", "CC-BY-4.0", false), link("", "CC0-1.0", false), link("", "CC0-1.0", false)];
    let mut replaced = HashMap::new();
    replaced.insert("v".to_string(), link("rightful", "CC-BY-NC-4.0", true));
    let out = chain_of_links(&links, &set(&["v", "w"]), &replaced);
    let ids: Vec<&str> = out.iter().map(|l| l.item_id.as_str()).collect();
    assert_eq!(ids, vec!["a", "rightful", "", ""], "w dropped, v swapped, the second a dropped, blank ids are never deduplicated");
    assert!(out[1].modified);
    // The stand-in takes the replaced link's `modified`, not its own.
    let out = chain_of_links(&[link("v", "CC-BY-4.0", false)], &set(&["v"]), &replaced);
    assert!(!out[0].modified);
    assert_eq!(out[0].license, "CC-BY-NC-4.0");
    // A stand-in that is itself void is dropped.
    assert!(chain_of_links(&[link("v", "CC-BY-4.0", false)], &set(&["v", "rightful"]), &replaced).is_empty());
    assert!(chain_of("not json", &set(&[]), &HashMap::new()).is_empty());
    let json = serde_json::to_string(&vec![link("a", "CC0-1.0", false)]).unwrap();
    assert_eq!(chain_of(&json, &set(&[]), &HashMap::new()).len(), 1);
}

#[test]
fn link_grants_count_earlier_links_as_adaptations_and_the_immediate_source_as_given() {
    let mut collapsed = link("", "", false);
    collapsed.collapsed_licenses = Some(vec!["CC-BY-4.0".into()]);
    let earlier = vec![link("a", "CC0-1.0", false), collapsed];
    let imm = Grant { item: "src".into(), license: "CC-BY-4.0".into(), modified: false };
    let g = link_grants(&earlier, Some(&imm));
    assert_eq!(g.first(), Some(&Grant { item: "a".into(), license: "CC0-1.0".into(), modified: true }));
    assert_eq!(g.last(), Some(&imm));
    assert!(g.len() >= 3, "the collapsed run still grants");
    assert_eq!(link_grants(&[], Some(&Grant { item: String::new(), license: "CC0-1.0".into(), modified: false })), vec![]);
    assert_eq!(link_grants(&[], None), vec![]);
}

fn open() -> (rusqlite::Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    (conn, dir)
}

#[test]
fn stem_hits_prepare_against_the_real_view_and_an_empty_pool_has_none() {
    let (c, _d) = open();
    assert!(stem_hits(&c, "me", &[]).unwrap().is_empty());
    assert!(stem_hits(&c, "me", &["h1".into(), "h1".into()]).unwrap().is_empty());
}

#[test]
fn a_recorded_origin_of_a_void_item_without_a_replacement_is_void() {
    let (c, _d) = open();
    assert!(voided_refs(&c, "me", &[]).unwrap().is_empty());
    let item = "a".repeat(32);
    let other = "b".repeat(32);
    let refs = vec![(item.clone(), "q1".to_string()), (other.clone(), "q2".to_string())];
    assert!(voided_refs(&c, "me", &refs).unwrap().is_empty());
    c.execute("INSERT INTO void_items (item_id, at, reason) VALUES (?1, 1, 'dmca')", [&item]).unwrap();
    assert_eq!(voided_refs(&c, "me", &refs).unwrap(), set(&[&format!("{item}/q1")]));
    // A replaced item is not void for the reader: the chain follows the rightful item instead.
    c.execute("UPDATE void_items SET replaced_by = ?1, replaced_license = 'CC0-1.0'", [&other]).unwrap();
    assert!(voided_refs(&c, "me", &refs).unwrap().is_empty());
    // A voided serve counts only while the reader's publish still holds the question (none here).
    c.execute("DELETE FROM void_items", []).unwrap();
    c.execute("INSERT INTO served (reader_id, item_id, publish_id, kind, license, licenses_json, first_at, at, void_at) VALUES ('me', ?1, 'p1', 'view', 'CC0-1.0', '[]', 1, 1, 2)", [&item]).unwrap();
    assert!(voided_refs(&c, "me", &refs).unwrap().is_empty());
}
