//! Tests for `license.rs`: the table, and the cases of worker/test/pool/licence-cases.test.ts that are decided by the
//! rule alone (r7 s5, r7 s9, the earliest-wins resolution), at the level of `decide_licence`.
use super::*;

fn g(item: &str, license: &str, modified: bool) -> Grant {
    Grant { item: item.into(), license: license.into(), modified }
}
fn unit(item: &str, verbatim: bool, own: &[&str], chain: &[&str], contents: &[&str], seq: i64) -> LicenceUnit {
    LicenceUnit { item: item.into(), verbatim, chain: chain.iter().map(|s| s.to_string()).collect(), own: vec![own.iter().map(|s| s.to_string()).collect()], contents: contents.iter().map(|s| s.to_string()).collect(), seq }
}

#[test]
fn the_table_never_turns_one_condition_set_into_another() {
    assert!(may_republish("CC-BY-4.0", "CC-BY-NC-SA-4.0"));
    assert!(!may_republish("CC-BY-SA-4.0", "CC-BY-NC-SA-4.0"), "BY-SA cannot become BY-NC-SA");
    assert!(!may_republish("CC-BY-NC-4.0", "CC-BY-4.0"));
    assert!(may_republish("CC0-1.0", "circle-only"));
    assert!(republish_options("circle-only").is_empty() && republish_options("bogus").is_empty());
    assert!(!may_republish("bogus", "CC0-1.0") && !may_republish("CC0-1.0", "bogus"));
    assert_eq!(allowed_by_all(["CC-BY-4.0", "CC-BY-NC-4.0"]), vec!["CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"]);
    assert_eq!(allowed_by_all([]), LICENSES.to_vec());
    assert!(allowed_by_all(["circle-only"]).is_empty());
}

#[test]
fn a_verbatim_copy_keeps_its_licence_except_cc0_and_an_adaptation_follows_the_table() {
    assert_eq!(grant_options(&g("i", "CC-BY-4.0", false)), &["CC-BY-4.0"]);
    assert_eq!(grant_options(&g("i", "CC-BY-4.0", true)).len(), 4);
    assert_eq!(grant_options(&g("i", "CC0-1.0", false)).len(), 6, "CC0 may be relabelled freely");
    assert!(grant_options(&g("i", "circle-only", true)).is_empty());
    assert!(grant_options(&g("i", "nonsense", false)).is_empty());
}

#[test]
fn grants_within_one_item_unite_when_verbatim_and_intersect_when_adapted_across_items_they_intersect() {
    assert_eq!(allowed_for(&[]), LICENSES.to_vec());
    // v1 under CC-BY and v2 under CC-BY-NC, both verbatim: a copy of either may use either.
    let both = allowed_for(&[g("a", "CC-BY-4.0", false), g("a", "CC-BY-NC-4.0", false)]);
    assert_eq!(both, vec!["CC-BY-4.0", "CC-BY-NC-4.0"]);
    // An adaptation of both versions must satisfy both: only what NC allows.
    let adapted = allowed_for(&[g("a", "CC-BY-4.0", true), g("a", "CC-BY-NC-4.0", true)]);
    assert_eq!(adapted, vec!["CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"]);
    assert_eq!(allowed_for(&[g("a", "CC-BY-SA-4.0", true), g("b", "CC-BY-NC-4.0", true)]), Vec::<&str>::new(), "SA with NC leaves nothing");
    let bank = allowed_for_all(&[vec![g("a", "CC-BY-4.0", true)], vec![g("b", "CC-BY-NC-4.0", true)]]);
    assert_eq!(bank, vec!["CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"]);
    assert_eq!(allowed_for_bank(&[vec!["CC0-1.0", "CC-BY-4.0"], vec!["CC-BY-4.0"]]), vec!["CC-BY-4.0"]);
}

#[test]
fn a_stored_chain_counts_every_earlier_link_as_adapted_and_collapsed_runs_too() {
    let collapsed = vec!["CC-BY-NC-4.0".to_string()];
    let grants = chain_grants(&[
        ChainLinkLicense { item_id: "a", license: "CC-BY-4.0", collapsed_licenses: &[] },
        ChainLinkLicense { item_id: "", license: "", collapsed_licenses: &collapsed },
        ChainLinkLicense { item_id: "b", license: "", collapsed_licenses: &[] },
    ]);
    assert_eq!(grants, vec![g("a", "CC-BY-4.0", true), g("collapsed:CC-BY-NC-4.0", "CC-BY-NC-4.0", true)]);
}

#[test]
fn no_sources_allow_anything_and_circle_only_material_allows_nothing() {
    assert_eq!(decide_licence(&[]), LicenceDecision { allowed: LICENSES.to_vec(), conflict: false, binders: vec![] });
    let d = decide_licence(&[unit("c", false, &["circle-only"], &[], &["h"], 1)]);
    assert!(d.allowed.is_empty());
}

#[test]
fn r7_s5_an_adaptation_of_contradicting_imports_follows_the_earlier_source() {
    // Sam (SA) published first, Nia (NC) second: Tess's own version of the common question is bound by Sam.
    let sam = unit("sam", false, &["CC-BY-SA-4.0"], &[], &["capital-sam"], 1);
    let nia = unit("nia", false, &["CC-BY-NC-4.0"], &[], &["capital-nia"], 2);
    let d = decide_licence(&[nia.clone(), sam.clone()]);
    assert_eq!(d.allowed, vec!["CC-BY-SA-4.0"]);
    assert!(d.conflict, "SA and NC contradict: flagged license_conflict");
    assert_eq!(d.binders, vec!["sam".to_string(), "nia".to_string()]);
    // The same in the other import order: it never depends on it.
    assert_eq!(decide_licence(&[sam, nia]).allowed, vec!["CC-BY-SA-4.0"]);
}

#[test]
fn a_verbatim_copy_is_decided_by_the_earliest_original_whatever_relabels_follow() {
    // r7 s9: a copy of an imported CC0 question stays CC0 beside later adaptations under other licences.
    let zero = unit("zero", true, &["CC0-1.0"], &[], &["h"], 1);
    let relabel = unit("nc", true, &["CC-BY-NC-SA-4.0"], &[], &["h"], 2);
    let adapt = unit("adapt", false, &["CC-BY-SA-4.0"], &[], &["other"], 3);
    let d = decide_licence(&[adapt, relabel, zero]);
    assert_eq!(d.allowed, LICENSES.to_vec());
    assert_eq!((d.conflict, d.binders), (false, vec!["zero".to_string()]));
}

#[test]
fn sources_that_served_the_same_content_are_one_material_and_the_earliest_decides_it() {
    // Two sources of the same served content: the earlier one (CC-BY) decides the material; the later NC relabel does not narrow it.
    let a = unit("a", false, &["CC-BY-4.0"], &[], &["same"], 1);
    let b = unit("b", false, &["CC-BY-NC-4.0"], &[], &["same"], 2);
    let d = decide_licence(&[b, a]);
    assert_eq!(d.allowed, vec!["CC-BY-4.0", "CC-BY-SA-4.0", "CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"]);
    assert_eq!(d.binders, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn a_units_own_contradiction_lets_the_older_chain_terms_win() {
    // Its chain says SA; it relabelled itself CC-BY-NC: the chain (older) wins and it is flagged.
    let u = unit("u", false, &["CC-BY-NC-4.0"], &["CC-BY-SA-4.0"], &["h"], 1);
    let (allowed, conflict) = unit_allows(&u);
    assert!(conflict);
    assert_eq!(allowed.into_iter().collect::<Vec<_>>(), vec!["CC-BY-SA-4.0"]);
}

#[test]
fn the_relied_licence_is_the_most_permissive_per_content_and_the_most_restrictive_across_contents() {
    let groups = vec![vec!["CC-BY-4.0".to_string(), "CC0-1.0".to_string()], vec!["CC-BY-4.0".to_string()]];
    assert_eq!(relied_licence(&groups, "CC-BY-SA-4.0", true), Some("CC-BY-4.0"), "CC0 reaches most for the first, BY is all the second has: the more restrictive of the two bests");
    assert_eq!(relied_licence(&[vec!["CC-BY-NC-4.0".to_string()]], "CC-BY-4.0", true), None, "NC does not permit BY");
    assert_eq!(relied_licence(&[vec!["CC0-1.0".to_string()]], "CC-BY-NC-4.0", false), Some("CC0-1.0"));
    assert_eq!(relied_licence(&[], "CC0-1.0", true), None);
    assert_eq!(relied_licence(&[vec!["junk".to_string()]], "CC0-1.0", true), None);
}
