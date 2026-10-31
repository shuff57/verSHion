//! Tests for `fuzzyhits.rs` on the real migrated schema: a served source whose stem is a near-rewording (another
//! match_hash) is a fuzzy hit when it is admissible and has the same body; otherwise it is dropped or only flagged.
use super::*;
use crate::db;
use std::path::Path;

const SECRET: &str = "test-secret-0123456789abcdef0123456789abcdef";
const TOKS: &[&str] = &["what", "is", "the", "capital", "of", "france"];
const BODY: &[&str] = &["kind:Mc", "paris", "rome"];

fn open() -> (Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    (conn, dir)
}
fn user(c: &Connection, id: &str) {
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?1, 'teacher', 1)", params![id, format!("{id}@x.org")]).unwrap();
}
fn item(c: &Connection, id: &str, author: &str, publish: &str) {
    c.execute("INSERT INTO items (id, author_id, kind, visibility, current_publish_id, public_publish_id, public_version, license, created_at, updated_at) VALUES (?1, ?2, 'bank', 'public', ?3, ?3, 1, 'CC-BY-4.0', 1, 1)", params![id, author, publish]).unwrap();
}
fn publish(c: &Connection, id: &str, item: &str, author: &str) {
    c.execute(
        "INSERT INTO item_publishes (publish_id, item_id, author_id, version, public_version, status, schema_version, hide_key, license, attested_at, attest_text_rev,
            acks_json, request_sha256, stage_token, full_key, full_bytes, title, subject, course, grade, kinds_mask, question_count, has_images, created_at)
         VALUES (?1, ?2, ?3, 1, 1, 'current', 2, 0, 'CC-BY-4.0', 1, 'r', '{}', 'x', ?1, 'k', 10, '', '', '', '', 0, 1, 1, 1)",
        params![id, item, author],
    )
    .unwrap();
}
fn question(c: &Connection, publish: &str, qid: &str, mh: &str, seq: i64, body: &[&str]) {
    let required: Vec<String> = {
        let mut st = c.prepare("SELECT name FROM pragma_table_info('item_questions') WHERE \"notnull\" = 1 AND dflt_value IS NULL").unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    let vals: Vec<String> = required.iter().map(|n| if n == "pos" { "0".to_string() } else { "''".to_string() }).collect();
    c.execute(&format!("INSERT INTO item_questions ({}) VALUES ({})", required.join(", "), vals.join(", ")), []).unwrap();
    c.execute(
        "UPDATE item_questions SET publish_id = ?1, qid = ?2, seq = ?3, match_hash = ?4, stem_tokens = ?5, body_tokens = ?6, full_content = 'FULL', near_full = 'NB', near_stripped = 'NB',
            near_kf = 'NKF', near_bare_full = 'NBARE', near_bare_stripped = 'NBARE', chain_json = '[]' WHERE rowid = last_insert_rowid()",
        params![publish, qid, seq, mh, serde_json::to_string(TOKS).unwrap(), serde_json::to_string(body).unwrap()],
    )
    .unwrap();
}
fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}
fn subject() -> FuzzyPos {
    FuzzyPos { pos: 0, mh: "mhS".into(), tokens: s(TOKS), bands: s(&["0:a:b", "1:c:d"]), first: None, nfull: Some("NS".into()), nbare: Some("NSBARE".into()), noexpl: false, body: Some(s(BODY)) }
}
fn fp() -> Fingerprinter {
    Fingerprinter::from_secret(SECRET).unwrap()
}
const NONE: &[String] = &[];

/// `me` was served b's item (an import of mhB), and b's stem bands are in the public index.
fn world(body: &[&str]) -> (Connection, tempfile::TempDir, String) {
    let (c, d) = open();
    user(&c, "me");
    user(&c, "b");
    let ib = "1".repeat(32);
    item(&c, &ib, "b", "pb");
    publish(&c, "pb", &ib, "b");
    question(&c, "pb", "q", "mhB", 5, body);
    c.execute("INSERT INTO served (reader_id, item_id, publish_id, kind, license, licenses_json, first_at, at) VALUES ('me', ?1, 'pb', 'import', 'CC-BY-4.0', '[\"CC-BY-4.0\"]', 1, 5)", [&ib]).unwrap();
    for b in ["0:a:b", "1:c:d"] {
        c.execute("INSERT INTO stem_bands (user_id, band, item_id, qid) VALUES ('pub', ?1, ?2, 'q')", params![b, ib]).unwrap();
    }
    (c, d, ib)
}

#[test]
fn the_sql_files_join_into_a_query_that_prepares_and_an_empty_pool_has_no_hits() {
    let (c, _d) = open();
    let r = fuzzy_hits(&c, &fp(), "me", &[subject()], NONE, NONE, &[]).unwrap();
    assert!(r.hits.is_empty() && r.cut.is_empty() && r.stem_only.is_empty() && !r.capped);
    let mut nb = subject();
    nb.bands = vec![];
    assert!(fuzzy_hits(&c, &fp(), "me", &[nb], NONE, NONE, &[]).unwrap().hits.is_empty());
}

#[test]
fn a_served_near_rewording_with_the_same_body_is_a_fuzzy_import_hit() {
    let (c, _d, ib) = world(BODY);
    let r = fuzzy_hits(&c, &fp(), "me", &[subject()], NONE, NONE, &[]).unwrap();
    assert_eq!(r.hits.len(), 1);
    let h = &r.hits[0];
    assert_eq!((h.item_id.as_str(), h.qid.as_str(), h.match_hash.as_str(), h.kind.as_str()), (ib.as_str(), "q", "mhB", "import"));
    assert!(h.fuzzy && h.by_match && !h.by_ref && !h.same);
    assert_eq!((h.author_name.as_str(), h.license.as_str(), h.source_first_seq, h.version), ("b", "CC-BY-4.0", 5, 1));
    assert_eq!(h.lic_all, s(&["CC-BY-4.0"]));
    assert!(!h.near, "the licence-sense content differs");
    assert!(r.stem_only.is_empty());
}

#[test]
fn another_body_is_only_a_stem_flag_and_a_different_stem_is_no_hit() {
    let (c, _d, _ib) = world(&["kind:Mc", "berlin", "madrid"]);
    let r = fuzzy_hits(&c, &fp(), "me", &[subject()], NONE, NONE, &[]).unwrap();
    assert!(r.hits.is_empty());
    assert_eq!(r.stem_only, [0usize].into_iter().collect());
    let mut other = subject();
    other.tokens = s(&["totally", "unrelated", "words", "go", "here", "now"]);
    let r = fuzzy_hits(&c, &fp(), "me", &[other], NONE, NONE, &[]).unwrap();
    assert!(r.hits.is_empty() && r.stem_only.is_empty());
}

#[test]
fn own_void_independent_and_later_sources_are_not_candidates() {
    let (c, _d, ib) = world(BODY);
    // The publisher's own item, and an item whose chain runs through one of hers.
    assert!(fuzzy_hits(&c, &fp(), "me", &[subject()], &[ib.clone()], NONE, &[]).unwrap().hits.is_empty());
    // An item the publisher attests independence of is no near candidate.
    assert!(fuzzy_hits(&c, &fp(), "me", &[subject()], NONE, &[ib.clone()], &[]).unwrap().hits.is_empty());
    // Not later than the publisher's own first publish (the source is at seq 5).
    let mut early = subject();
    early.first = Some(5);
    assert!(fuzzy_hits(&c, &fp(), "me", &[early.clone()], NONE, NONE, &[]).unwrap().hits.is_empty());
    early.first = Some(6);
    assert_eq!(fuzzy_hits(&c, &fp(), "me", &[early], NONE, NONE, &[]).unwrap().hits.len(), 1);
    // A source that is the same match_hash is the exact evidence's business, not a fuzzy one.
    let mut same = subject();
    same.mh = "mhB".into();
    assert!(fuzzy_hits(&c, &fp(), "me", &[same], NONE, NONE, &[]).unwrap().hits.is_empty());
    // A void item is never a candidate.
    c.execute("INSERT INTO void_items (item_id, at, reason) VALUES (?1, 1, 'dmca')", [&ib]).unwrap();
    assert!(fuzzy_hits(&c, &fp(), "me", &[subject()], NONE, NONE, &[]).unwrap().hits.is_empty());
}

#[test]
fn a_chain_through_the_publishers_items_is_dropped_and_more_than_the_keep_cap_is_cut() {
    let (c, _d, _ib) = world(BODY);
    // Chain through one of her items: dropped after the query.
    let mine = "9".repeat(32);
    c.execute("UPDATE item_questions SET chain_json = ?1 WHERE publish_id = 'pb'", [format!(r#"[{{"item_id":"{mine}","license":"CC0-1.0"}}]"#)]).unwrap();
    assert!(fuzzy_hits(&c, &fp(), "me", &[subject()], &[mine.clone()], NONE, &[]).unwrap().hits.is_empty());
    c.execute("UPDATE item_questions SET chain_json = '[]' WHERE publish_id = 'pb'", []).unwrap();
    // FUZZY_KEEP + 3 imported candidates: the earliest FUZZY_KEEP are kept, the rest cut with their licences.
    for n in 0..(FUZZY_KEEP as usize + 3) {
        let (it, p) = (format!("{:032x}", n + 100), format!("px{n}"));
        item(&c, &it, "b", &p);
        publish(&c, &p, &it, "b");
        question(&c, &p, "q", &format!("mhx{n}"), 50 + n as i64, BODY);
        c.execute("INSERT INTO served (reader_id, item_id, publish_id, kind, license, licenses_json, first_at, at) VALUES ('me', ?1, ?2, 'import', 'CC-BY-4.0', '[\"CC-BY-4.0\"]', 1, 5)", params![it, p]).unwrap();
        for b in ["0:a:b", "1:c:d"] {
            c.execute("INSERT INTO stem_bands (user_id, band, item_id, qid) VALUES ('pub', ?1, ?2, 'q')", params![b, it]).unwrap();
        }
    }
    let r = fuzzy_hits(&c, &fp(), "me", &[subject()], NONE, NONE, &[]).unwrap();
    assert_eq!(r.hits.len() as i64, FUZZY_KEEP);
    assert_eq!(r.hits[0].source_first_seq, 5, "the earliest is kept first");
    let cut = &r.cut[&0];
    assert_eq!((cut.n, cut.lics.clone()), (4, s(&["CC-BY-4.0"])));
    // Beside an exact source the cut never counts.
    let r = fuzzy_hits(&c, &fp(), "me", &[subject()], NONE, NONE, &[0]).unwrap();
    assert!(!r.cut.contains_key(&0));
}

#[test]
fn cuts_merge_by_union_and_the_earlier_cuts_named_items_win() {
    let mut cut = HashMap::new();
    let by = |pairs: &[(&str, &str)]| pairs.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect::<HashMap<_, _>>();
    add_cut(&mut cut, 0, &s(&["A", "B"]), "i1", 2, &by(&[("A", "i1"), ("B", "i1")]));
    add_cut(&mut cut, 0, &s(&["B", "C"]), "i2", 3, &by(&[("B", "i2"), ("C", "i2")]));
    let c = &cut[&0];
    assert_eq!((c.lics.clone(), c.item.as_str(), c.n), (s(&["A", "B", "C"]), "i1", 5));
    assert_eq!(c.by["B"], "i1");
    assert_eq!(c.by["C"], "i2");
}
