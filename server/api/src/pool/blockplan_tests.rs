//! Tests for `blockplan.rs` on the real migrated schema: default selections for a takedown and a privacy block, scopes, copies
//! (who had the content first), affected items, ghost items and selection validation.
use super::*;
use crate::db;
use serde_json::json;
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
fn item(c: &Connection, n: u8, author: &str, visibility: &str) {
    let (i, p) = (id(n), format!("p{n}"));
    c.execute(
        "INSERT INTO items (id, author_id, kind, visibility, current_publish_id, public_publish_id, public_version, version, title, license, committed_at, created_at, updated_at, fold_rev)
         VALUES (?1, ?2, 'bank', ?3, ?4, ?4, 1, 1, ?5, 'CC-BY-4.0', 1, 1, 1, 1)",
        params![i, author, visibility, p, format!("Item {n}")],
    )
    .unwrap();
    c.execute(
        "INSERT INTO item_publishes (publish_id, item_id, author_id, version, public_version, status, schema_version, hide_key, license, attested_at, attest_text_rev,
            acks_json, request_sha256, stage_token, full_key, stripped_key, full_bytes, title, subject, course, grade, kinds_mask, question_count, has_images, created_at)
         VALUES (?1, ?2, ?3, 1, 1, 'current', 2, 0, 'CC-BY-4.0', 1, 'r', '{}', 'x', ?1, 'FK', 'SK', 10, '', '', '', '', 0, 1, 1, 1)",
        params![p, i, author],
    )
    .unwrap();
}
/// A question at `pos` with block keys derived from `h` (norm `n:h`, stem `s:h`, full `f:h`, expl `e:h`, answer `a:h`).
fn question(c: &Connection, n: u8, pos: i64, h: &str, seq: i64) {
    let p = format!("p{n}");
    let required: Vec<String> = {
        let mut st = c.prepare("SELECT name FROM pragma_table_info('item_questions') WHERE \"notnull\" = 1 AND dflt_value IS NULL").unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    let vals: Vec<String> = required.iter().map(|n| if n == "pos" { "0".to_string() } else { "''".to_string() }).collect();
    c.execute(&format!("INSERT INTO item_questions ({}) VALUES ({})", required.join(", "), vals.join(", ")), []).unwrap();
    c.execute(
        "UPDATE item_questions SET publish_id = ?1, qid = ?2, pos = ?3, qkind = 'Mc', match_hash = ?4, seq = ?5, bk_norm = ?6, bk_stem = ?7, bk_full = ?8, bk_expl = ?9, bk_ans = ?10
          WHERE rowid = last_insert_rowid()",
        params![p, format!("q{pos}-{p}"), pos, format!("m:{h}"), seq, format!("n:{h}"), format!("s:{h}"), format!("f:{h}"), format!("e:{h}"), format!("a:{h}")],
    )
    .unwrap();
}
fn blob(c: &Connection, n: u8, sha: &str, form: &str, seq: i64) {
    c.execute("INSERT INTO item_blobs (publish_id, form, sha256, seq) VALUES (?1, ?2, ?3, ?4)", params![format!("p{n}"), form, sha, seq]).unwrap();
}
fn sel() -> BlockSelection {
    BlockSelection::default()
}
fn plan(c: &Connection, n: u8, s: &BlockSelection, kind: &str, rightful: Option<&str>) -> Res<BlockPlan> {
    block_plan(&PoolCtx::new(c, 1_000), 500, &id(n), s, kind, rightful)
}
fn field(r: Res<BlockPlan>) -> String {
    match r {
        Err(Fail::Pool(e)) => e.body()["field"].as_str().unwrap_or(&e.code).to_string(),
        other => panic!("{other:?}"),
    }
}

/// Item 1 (author a) has two questions and one image; item 2 (author b) copies question `x` and the image later.
fn world(c: &Connection) {
    for u in ["a", "b", "c"] {
        user(c, u);
    }
    item(c, 1, "a", "public");
    question(c, 1, 0, "x", 5);
    question(c, 1, 1, "own", 6);
    blob(c, 1, "img1", "full", 5);
    item(c, 2, "b", "public");
    question(c, 2, 0, "x", 9);
    blob(c, 2, "img1", "full", 9);
}

#[test]
fn a_takedown_selects_what_no_other_author_uses_and_leaves_shared_material_for_the_admin() {
    let (c, _d) = open();
    world(&c);
    // With K=3 a single other author is "uncommon": both questions and the image are selected, and the copy is the affected item.
    let p = plan(&c, 1, &sel(), "dmca", None).unwrap();
    assert_eq!(p.matches.len(), 2);
    assert!(p.matches.iter().all(|m| m.selected && m.scope == "skeleton"));
    assert_eq!(p.matches[0].keys, vec!["m:x", "n:x"]);
    assert_eq!((p.matches[0].other_authors, p.matches[0].introduced), (1, true), "item 1 had it first (seq 5 < 9)");
    assert_eq!(p.matches[1].other_authors, 0);
    assert_eq!(p.blob.len(), 1);
    assert!(p.blob[0].selected && p.blob[0].introduced);
    assert_eq!(p.affected.iter().map(|a| (a.item_id.clone(), a.via.as_str())).collect::<Vec<_>>(), vec![(id(2), "question")]);
    assert_eq!(p.affected_total, 1);
    assert_eq!(p.matches[0].affects.as_ref().unwrap(), &vec![Affects { item_id: id(2), title: "Item 2".into() }]);
    assert!(p.ghost.is_none());
    // Three other authors using the text make it common: not selected by default.
    user(&c, "d");
    for (n, a) in [(3u8, "c"), (4, "d")] {
        item(&c, n, a, "public");
        question(&c, n, 0, "x", 10);
    }
    let p = plan(&c, 1, &sel(), "dmca", None).unwrap();
    assert!(!p.matches[0].selected, "common among 3 other authors");
    assert!(p.matches[1].selected);
}

#[test]
fn a_copy_that_was_there_first_is_not_selected_unless_it_traces_to_the_rightful_item() {
    let (c, _d) = open();
    world(&c);
    // Item 2's own view: its copy of `x` came AFTER item 1's, so item 1 introduced it. Plan item 2 (the later copy).
    let p = plan(&c, 2, &sel(), "dmca", None).unwrap();
    assert!(!p.matches[0].introduced && !p.matches[0].selected, "another author had it first");
    assert!(!p.blob[0].introduced && !p.blob[0].selected);
    // Naming item 1 as the rightful item selects the material that is item 1's own.
    let p = plan(&c, 2, &sel(), "dmca", Some(&id(1))).unwrap();
    assert!(p.matches[0].selected && p.blob[0].selected);
}

#[test]
fn a_privacy_block_selects_nothing_by_default_and_suggests_what_only_this_item_has() {
    let (c, _d) = open();
    world(&c);
    let p = plan(&c, 1, &sel(), "privacy", None).unwrap();
    assert!(p.matches.iter().all(|m| !m.selected) && !p.blob[0].selected);
    assert_eq!(p.matches[0].scope, "default");
    // "x" is also pasted by another author who came later (not a copy of item 1 by import): still suggested (item 1 introduced it),
    // with the count of other authors; the own question has none.
    assert_eq!((p.matches[0].suggested, p.matches[0].other_authors), (Some(true), 1));
    assert_eq!((p.matches[1].suggested, p.matches[1].other_authors), (Some(true), 0));
    assert_eq!(p.blob[0].suggested, Some(false), "another author serves this image");
    // Seen from item 2: item 1 had it first, who is named.
    let p2 = plan(&c, 2, &sel(), "privacy", None).unwrap();
    assert_eq!(p2.matches[0].suggested, Some(false));
    assert_eq!(p2.matches[0].first, Some(First { item_id: id(1), author_id: "a".into() }));
    // The default privacy scope also holds the full key; ticking a question selects it and lists who it would hold.
    let s = BlockSelection { questions: Some(vec![0]), ..Default::default() };
    let p = plan(&c, 1, &s, "privacy", None).unwrap();
    assert_eq!(p.matches[0].keys, vec!["m:x", "n:x", "s:x", "f:x"]);
    assert!(p.matches[0].selected && !p.matches[1].selected);
    assert_eq!(p.affected.len(), 1);
    assert!(p.matches[0].stem_wide, "a stem key is among the keys");
}

#[test]
fn scopes_choose_the_keys_and_unknown_or_out_of_range_scopes_are_refused() {
    let (c, _d) = open();
    world(&c);
    let scoped = |pairs: &[(&str, &str)]| BlockSelection { scopes: Some(pairs.iter().map(|(k, v)| (k.to_string(), json!(v))).collect()), ..Default::default() };
    for (scope, want) in [("skeleton", vec!["m:x", "n:x"]), ("stem", vec!["m:x", "n:x", "s:x"]), ("full", vec!["f:x"]), ("explanation", vec!["e:x"]), ("answer", vec!["a:x"])] {
        let p = plan(&c, 1, &scoped(&[("0", scope)]), "dmca", None).unwrap();
        assert_eq!(p.matches[0].keys, want, "{scope}");
        assert_eq!(p.matches[0].scope, scope);
    }
    assert_eq!(field(plan(&c, 1, &scoped(&[("0", "bogus")]), "dmca", None)), "scopes");
    assert_eq!(field(plan(&c, 1, &scoped(&[("7", "full")]), "dmca", None)), "scopes");
    assert_eq!(field(plan(&c, 1, &scoped(&[("-1", "full")]), "dmca", None)), "scopes");
    // A part scope on a question with no parts has nothing to block.
    assert_eq!(field(plan(&c, 1, &scoped(&[("0", "part:1")]), "dmca", None)), "scopes");
    // Part keys: s:/e:/a: prefixes feed the matching scopes.
    c.execute("INSERT INTO item_part_keys (publish_id, pos, part, key) VALUES ('p1', 0, 1, 's:part1'), ('p1', 0, 1, 'e:part1'), ('p1', 0, 2, 'a:part2')", []).unwrap();
    assert_eq!(plan(&c, 1, &scoped(&[("0", "part:1")]), "dmca", None).unwrap().matches[0].keys, vec!["e:part1", "s:part1"]);
    assert_eq!(plan(&c, 1, &scoped(&[("0", "answer")]), "dmca", None).unwrap().matches[0].keys, vec!["a:x", "a:part2"]);
}

#[test]
fn an_explicit_selection_is_validated_and_overrides_the_default() {
    let (c, _d) = open();
    world(&c);
    let p = plan(&c, 1, &BlockSelection { questions: Some(vec![1]), images: Some(vec![]), ..Default::default() }, "dmca", None).unwrap();
    assert!(!p.matches[0].selected && p.matches[1].selected && !p.blob[0].selected);
    assert_eq!(p.affected_total, 0, "item 2 uses only the unselected text and image");
    assert_eq!(field(plan(&c, 1, &BlockSelection { questions: Some(vec![2]), ..Default::default() }, "dmca", None)), "questions");
    assert_eq!(field(plan(&c, 1, &BlockSelection { questions: Some(vec![-1]), ..Default::default() }, "dmca", None)), "questions");
    assert_eq!(field(plan(&c, 1, &BlockSelection { images: Some(vec!["nope".into()]), ..Default::default() }, "dmca", None)), "images");
}

#[test]
fn only_the_images_non_authors_are_served_are_planned() {
    let (c, _d) = open();
    world(&c);
    // A hidden-key publish serves its stripped form: an image only in the full form is not in the plan.
    c.execute("UPDATE item_publishes SET hide_key = 1 WHERE publish_id = 'p1'", []).unwrap();
    blob(&c, 1, "img1", "stripped", 5);
    blob(&c, 1, "secret", "full", 5);
    let p = plan(&c, 1, &sel(), "dmca", None).unwrap();
    assert_eq!(p.blob.iter().map(|b| b.sha256.as_str()).collect::<Vec<_>>(), vec!["img1"]);
}

#[test]
fn a_purged_item_is_planned_from_its_ghost_for_a_takedown_only_and_selects_nothing_by_default() {
    let (c, _d) = open();
    world(&c);
    // Not purged: unknown id and privacy-on-purged are 404.
    assert_eq!(field(plan(&c, 9, &sel(), "dmca", None)), "not_found");
    c.execute("UPDATE items SET purged_at = 50, removed_at = 50, removed_reason = 'account_deleted' WHERE id = ?1", [id(1)]).unwrap();
    assert_eq!(field(plan(&c, 1, &sel(), "privacy", None)), "not_found");
    // A purged item with no ghost publish has an empty plan.
    let p = plan(&c, 1, &sel(), "dmca", None).unwrap();
    assert_eq!((p.ghost, p.selection_required, p.matches.len()), (Some(true), Some(false), 0));
    // With a ghost: planned from ghost_questions / ghost_block_keys / ghost_units.
    c.execute("INSERT INTO ghost_publishes (publish_id, item_id, author_id, hide_key, public_version, license, at) VALUES ('gp', ?1, 'a', 0, 1, 'CC-BY-4.0', 40)", [id(1)]).unwrap();
    let required: Vec<String> = {
        let mut st = c.prepare("SELECT name FROM pragma_table_info('ghost_questions') WHERE \"notnull\" = 1 AND dflt_value IS NULL").unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    let vals: Vec<String> = required.iter().map(|n| if n == "pos" || n == "seq" { "0".to_string() } else { "''".to_string() }).collect();
    c.execute(&format!("INSERT INTO ghost_questions ({}) VALUES ({})", required.join(", "), vals.join(", ")), []).unwrap();
    c.execute("UPDATE ghost_questions SET publish_id = 'gp', qid = 'g0', pos = 0, match_hash = 'm:x', seq = 5, qkind = 'Mc', bk_norm = 'n:x', bk_stem = 's:x', bk_full = 'f:x', bk_expl = 'e:x', bk_ans = 'a:x' WHERE rowid = last_insert_rowid()", []).unwrap();
    c.execute("INSERT INTO ghost_units (publish_id, pos, part, key, kind, seq) VALUES ('gp', 0, 0, 'img1', 'image', 5), ('gp', 0, 0, 'px:abc', 'image', 5)", []).unwrap();
    let p = plan(&c, 1, &sel(), "dmca", None).unwrap();
    assert_eq!((p.ghost, p.selection_required), (Some(true), Some(true)));
    assert!(!p.matches[0].selected && !p.blob[0].selected, "nothing selected until the admin chooses");
    assert_eq!(p.blob.iter().map(|b| b.sha256.as_str()).collect::<Vec<_>>(), vec!["img1"], "pixel keys are not content hashes");
    assert_eq!(p.matches[0].keys, vec!["m:x", "n:x"]);
    // `all` applies the live default; an explicit pick selects that pick.
    let all = plan(&c, 1, &BlockSelection { all: true, ..Default::default() }, "dmca", None).unwrap();
    assert!(all.matches[0].selected && all.blob[0].selected);
    assert_eq!(all.selection_required, Some(false));
    assert_eq!(all.affected.iter().map(|a| a.item_id.clone()).collect::<Vec<_>>(), vec![id(2)]);
    let one = plan(&c, 1, &BlockSelection { questions: Some(vec![0]), ..Default::default() }, "dmca", None).unwrap();
    assert!(one.matches[0].selected && !one.blob[0].selected);
}

#[test]
fn the_digest_of_a_plan_changes_with_the_selection() {
    let (c, _d) = open();
    world(&c);
    let a = plan(&c, 1, &sel(), "dmca", None).unwrap();
    let b = plan(&c, 1, &BlockSelection { questions: Some(vec![1]), ..Default::default() }, "dmca", None).unwrap();
    let bind = serde_json::Map::new();
    assert_ne!(crate::pool::blocking::plan_digest("dmca", &a, &bind), crate::pool::blocking::plan_digest("dmca", &b, &bind));
    assert_eq!(crate::pool::blocking::plan_digest("dmca", &a, &bind), crate::pool::blocking::plan_digest("dmca", &plan(&c, 1, &sel(), "dmca", None).unwrap(), &bind));
}
