//! Tests for `evidence.rs` against the real migrated schema: the long query runs, and the admission rules it carries
//! (own items, void items, independence, imports vs reads) decide what a publish sees.
use super::*;
use crate::db;
use crate::pool::visible::SourceRef;
use std::path::Path;

fn open() -> (Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut conn, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    (conn, dir)
}
fn user(c: &Connection, id: &str, name: &str) {
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES (?1, ?2, ?3, 'teacher', 1)", params![id, format!("{id}@x.org"), name]).unwrap();
}
fn item(c: &Connection, id: &str, author: &str, publish: &str) {
    c.execute("INSERT INTO items (id, author_id, kind, visibility, current_publish_id, license, created_at, updated_at) VALUES (?1, ?2, 'bank', 'public', ?3, 'CC-BY-4.0', 1, 1)", params![id, author, publish]).unwrap();
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
fn question(c: &Connection, publish: &str, qid: &str, mh: &str, seq: i64) {
    let required: Vec<String> = {
        let mut st = c.prepare("SELECT name FROM pragma_table_info('item_questions') WHERE \"notnull\" = 1 AND dflt_value IS NULL").unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    let vals: Vec<String> = required.iter().map(|n| if n == "pos" { "0".to_string() } else { "''".to_string() }).collect();
    c.execute(&format!("INSERT INTO item_questions ({}) VALUES ({})", required.join(", "), vals.join(", ")), []).unwrap();
    c.execute(
        "UPDATE item_questions SET publish_id = ?1, qid = ?2, match_hash = ?3, seq = ?4, full_content = 'FULL', stripped_content = 'STRIP', keyfree_content = 'KF',
            near_full = 'NEAR', near_stripped = 'NEARS', near_kf = 'NKF', chain_json = '[]' WHERE rowid = last_insert_rowid()",
        params![publish, qid, mh, seq],
    )
    .unwrap();
}
fn served(c: &Connection, reader: &str, item: &str, publish: &str, kind: &str) {
    c.execute("INSERT INTO served (reader_id, item_id, publish_id, kind, license, licenses_json, first_at, at) VALUES (?1, ?2, ?3, ?4, 'CC-BY-4.0', '[\"CC-BY-4.0\"]', 1, 5)", params![reader, item, publish, kind]).unwrap();
}
fn facts(mh: &str, full: &str) -> PositionFacts {
    PositionFacts {
        subject: SubjectParam { mh: mh.into(), full: Some(full.into()), kf: "KF".into(), nfull: Some("NEAR".into()), nkf: "NKF".into(), nbare: None, noexpl: 0, refs: vec![] },
        first: None,
        ownshown: false,
        common: false,
        keywork: 0,
        short40: 0,
        croots: vec![],
        ck: 3,
        ownkf: false,
    }
}
fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}
const NONE: &[String] = &[];
fn run(c: &Connection, subjects: &[PositionFacts], mine: &[String], indep: &[String]) -> Evidence {
    evidence(c, "me", subjects, &EvidenceOpts { mine, independent: indep }).unwrap()
}

/// `me` was served b's item (publish pb, question mh/qid q).
fn world(kind: &str) -> (Connection, tempfile::TempDir, String) {
    let (c, d) = open();
    user(&c, "me", "Me");
    user(&c, "b", "Bea");
    let ib = "1".repeat(32);
    item(&c, &ib, "b", "pb");
    publish(&c, "pb", &ib, "b");
    question(&c, "pb", "q", "mh", 7);
    served(&c, "me", &ib, "pb", kind);
    (c, d, ib)
}

#[test]
fn an_empty_publish_has_no_evidence_and_the_query_runs_on_an_empty_pool() {
    let (c, _d) = open();
    assert!(run(&c, &[], NONE, NONE).by_pos.is_empty());
    let e = run(&c, &[facts("mh", "FULL")], NONE, NONE);
    assert!(e.by_pos.is_empty() && e.overflow.is_empty() && e.credits.is_empty() && e.cut.is_empty());
}

#[test]
fn an_import_of_the_same_question_is_a_verbatim_hit_with_its_licence_and_credit() {
    let (c, _d, ib) = world("import");
    let e = run(&c, &[facts("mh", "FULL")], NONE, NONE);
    let h = &e.by_pos[&0][0];
    assert_eq!((h.item_id.as_str(), h.qid.as_str(), h.kind.as_str()), (ib.as_str(), "q", "import"));
    assert!(h.same && h.near && h.same_full && h.same_imported && h.kf_same && h.by_match && !h.by_ref && !h.claimed);
    assert_eq!((h.author_name.as_str(), h.license.as_str(), h.source_first_seq, h.version), ("Bea", "CC-BY-4.0", 7, 1));
    assert_eq!(h.lic_all, s(&["CC-BY-4.0"]));
    assert_eq!(h.lic_near, s(&["CC-BY-4.0"]));
    assert_eq!(h.contents, s(&["FULL"]));
    assert_eq!(h.versions, vec![1]);
    let cr = &e.credits[&0];
    assert_eq!((cr.links.len(), cr.total, cr.missing), (1, 1, 0));
    assert_eq!((cr.links[0].item_id.as_str(), cr.links[0].author_name.as_str(), cr.links[0].modified), (ib.as_str(), "Bea", false));
    assert!(e.overflow.is_empty());
}

#[test]
fn a_changed_copy_is_adapted_not_same_and_a_second_position_is_numbered_apart() {
    let (c, _d, _ib) = world("import");
    let mut changed = facts("mh", "OTHER");
    changed.subject.nfull = Some("OTHERNEAR".into());
    changed.subject.kf = "OTHERKF".into();
    changed.subject.nkf = "OTHERNKF".into();
    let e = run(&c, &[facts("none", "x"), changed], NONE, NONE);
    assert!(!e.by_pos.contains_key(&0));
    let h = &e.by_pos[&1][0];
    assert!(!h.same && !h.near && !h.kf_same);
    assert_eq!(h.pos, 1);
    assert_eq!(e.credits[&1].links[0].modified, true, "an adaptation credits the source as modified");
}

#[test]
fn own_items_void_items_and_attested_independent_reads_are_not_evidence() {
    let (c, _d, ib) = world("view");
    // A read of a distinctive (uncommon) question is evidence; declared as an origin it also counts by reference.
    let e = run(&c, &[facts("mh", "FULL")], NONE, NONE);
    assert_eq!((e.by_pos[&0][0].kind.as_str(), e.by_pos[&0][0].by_ref), ("view", false));
    let mut declared = facts("mh", "FULL");
    declared.subject.refs = vec![SourceRef { item: ib.clone(), qid: "q".into() }];
    let e = run(&c, &[declared.clone()], NONE, NONE);
    assert!(e.by_pos[&0][0].by_ref);
    // The publisher's own item is never evidence.
    assert!(run(&c, &[declared.clone()], &[ib.clone()], NONE).by_pos.is_empty());
    // Attested independent: an exact, uncommon copy still counts (it is the original that was read)...
    assert!(!run(&c, &[declared.clone()], NONE, &[ib.clone()]).by_pos.is_empty());
    // ...but a CHANGED copy of an item the publisher attests independence of is dropped.
    let mut changed = declared.clone();
    changed.subject.nfull = Some("OTHERNEAR".into());
    changed.subject.nkf = "OTHERNKF".into();
    assert!(run(&c, &[changed], NONE, &[ib.clone()]).by_pos.is_empty());
    // A void item is never evidence.
    c.execute("INSERT INTO void_items (item_id, at, reason) VALUES (?1, 1, 'dmca')", [&ib]).unwrap();
    assert!(run(&c, &[declared], NONE, NONE).by_pos.is_empty());
}

#[test]
fn a_short_statement_makes_a_read_common_so_it_is_not_evidence_unless_attested_and_claimed() {
    let (c, _d, ib) = world("view");
    let mut short = facts("mh", "FULL");
    short.common = true;
    assert!(run(&c, &[short.clone()], NONE, NONE).by_pos.is_empty(), "a read of a common question is not evidence");
    // Attested independent, exact copy of a short read: a possible source only (claimed).
    short.short40 = 1;
    short.common = false;
    short.subject.refs = vec![SourceRef { item: ib.clone(), qid: "q".into() }];
    let e = run(&c, &[short], NONE, &[ib]);
    assert!(e.by_pos.is_empty() || e.by_pos[&0][0].claimed);
}

#[test]
fn past_the_allowance_the_overflow_is_counted_and_the_cut_keeps_the_left_out_licences() {
    let (c, _d, _ib) = world("import");
    for n in 0..(EVIDENCE_PER_POSITION as usize + 2) {
        let (it, p) = (format!("{:032x}", n + 100), format!("px{n}"));
        item(&c, &it, "b", &p);
        publish(&c, &p, &it, "b");
        question(&c, &p, "q", "mh", 50 + n as i64);
        served(&c, "me", &it, &p, "import");
    }
    let e = run(&c, &[facts("mh", "FULL")], NONE, NONE);
    assert_eq!(e.by_pos[&0].len() as i64, EVIDENCE_PER_POSITION);
    assert_eq!(e.overflow[&0], EVIDENCE_PER_POSITION + 3);
    let cut = &e.cut[&0];
    assert_eq!(cut.n, 3);
    assert_eq!(cut.lics, s(&["CC-BY-4.0"]));
    assert!(cut.by.contains_key("CC-BY-4.0"));
    assert_eq!(e.by_pos[&0][0].source_first_seq, 7, "the earliest copy ranks first");
}
