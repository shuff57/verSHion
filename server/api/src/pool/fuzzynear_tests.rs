//! Tests for `fuzzynear.rs` on the real migrated schema: a near-rewording by another author is found only when it is the
//! same kind and body and earlier than the publisher's own first; the publisher's own earlier versions are found
//! through retained rows and through the durable keyed record, the earlier of the two winning.
use super::*;
use crate::db;
use std::path::Path;

const SECRET: &str = "test-secret-0123456789abcdef0123456789abcdef";

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
/// A question with the stem tokens, body tokens and body hash the near lookups read.
fn question(c: &Connection, publish: &str, qid: &str, seq: i64, qkind: &str, tokens: &[&str], body: &[&str], body_hash: &str) {
    let required: Vec<String> = {
        let mut st = c.prepare("SELECT name FROM pragma_table_info('item_questions') WHERE \"notnull\" = 1 AND dflt_value IS NULL").unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    let vals: Vec<String> = required.iter().map(|n| if n == "pos" { "0".to_string() } else { "''".to_string() }).collect();
    c.execute(&format!("INSERT INTO item_questions ({}) VALUES ({})", required.join(", "), vals.join(", ")), []).unwrap();
    c.execute(
        "UPDATE item_questions SET publish_id = ?1, qid = ?2, seq = ?3, qkind = ?4, stem_tokens = ?5, body_tokens = ?6, body_hash = ?7, match_hash = ?8, chain_json = '[]' WHERE rowid = last_insert_rowid()",
        params![publish, qid, seq, qkind, serde_json::to_string(tokens).unwrap(), serde_json::to_string(body).unwrap(), body_hash, format!("mh-{qid}")],
    )
    .unwrap();
}
fn bands(c: &Connection, user: &str, item: &str, qid: &str, b: &[&str]) {
    for x in b {
        c.execute("INSERT INTO stem_bands (user_id, band, item_id, qid) VALUES (?1, ?2, ?3, ?4)", params![user, x, item, qid]).unwrap();
    }
}
fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

const TOKS: &[&str] = &["what", "is", "the", "capital", "of", "france"];
const BODY: &[&str] = &["kind:Mc", "paris", "rome"];

fn pubpos(first: Option<i64>) -> PubPos {
    PubPos { pos: 0, tokens: s(TOKS), bands: s(&["0:a:b", "1:c:d"]), qkind: "Mc".into(), first, body: Some(s(BODY)), loose_first: None, loose_expl: None, loose_ans: None, expl: None, ans: None }
}
fn public_world() -> (Connection, tempfile::TempDir) {
    let (c, d) = open();
    user(&c, "me");
    user(&c, "b");
    let ib = "1".repeat(32);
    item(&c, &ib, "b", "pb");
    publish(&c, "pb", &ib, "b");
    question(&c, "pb", "q", 5, "Mc", TOKS, BODY, "BH");
    bands(&c, "pub", &ib, "q", &["0:a:b", "1:c:d"]);
    (c, d)
}

#[test]
fn another_authors_earlier_near_rewording_of_the_same_kind_and_body_is_found() {
    let (c, _d) = public_world();
    let (near, cut) = public_near_earlier(&c, "me", &[pubpos(None)]).unwrap();
    let n = &near[&0];
    assert_eq!((n.qid.as_str(), n.seq, n.version, n.author_name.as_str(), n.license.as_str()), ("q", 5, 1, "b", "CC-BY-4.0"));
    assert!(cut.is_empty());
    // Only strictly before the publisher's own first publish.
    assert!(public_near_earlier(&c, "me", &[pubpos(Some(5))]).unwrap().0.is_empty());
    assert!(public_near_earlier(&c, "me", &[pubpos(Some(6))]).unwrap().0.contains_key(&0));
    // The same kind and the same body, or it is another question.
    let mut other_kind = pubpos(None);
    other_kind.qkind = "Tf".into();
    assert!(public_near_earlier(&c, "me", &[other_kind]).unwrap().0.is_empty());
    let mut other_body = pubpos(None);
    other_body.body = Some(s(&["kind:Mc", "berlin", "madrid"]));
    assert!(public_near_earlier(&c, "me", &[other_body]).unwrap().0.is_empty());
    // A different stem is not a rewording.
    let mut other_stem = pubpos(None);
    other_stem.tokens = s(&["completely", "different", "words", "here", "now", "ok"]);
    assert!(public_near_earlier(&c, "me", &[other_stem]).unwrap().0.is_empty());
    // The author's own items, and positions without bands, find nothing.
    assert!(public_near_earlier(&c, "b", &[pubpos(None)]).unwrap().0.is_empty());
    let mut nobands = pubpos(None);
    nobands.bands = vec![];
    assert!(public_near_earlier(&c, "me", &[nobands]).unwrap().0.is_empty());
}

#[test]
fn the_sql_window_is_the_earlier_of_the_two_firsts_as_the_worker_has_it() {
    // The Worker fetches candidates published before minFirst(first, looseFirst) and only then judges each against
    // `first` or the loose one. So a candidate between the two is never fetched, and the "lacks what she added" branch can
    // only ever see rows already earlier than both. Kept as the Worker has it: changing it would change outcomes.
    let (c, _d) = public_world();
    let mk = || {
        let mut p = pubpos(Some(8));
        p.expl = Some("E".into());
        p.loose_first = Some(3);
        p
    };
    assert!(public_near_earlier(&c, "me", &[mk()]).unwrap().0.is_empty(), "the source at 5 is outside the window (< 3)");
    c.execute("UPDATE item_questions SET seq = 2", []).unwrap();
    assert!(public_near_earlier(&c, "me", &[mk()]).unwrap().0.contains_key(&0), "a source at 2 is inside it");
}

#[test]
fn a_publishers_own_earlier_copy_is_never_the_public_source() {
    let (c, _d) = public_world();
    c.execute("INSERT INTO author_firsts (author_id, match_hash, first_seq) VALUES ('me', 'mh-q', 3)", []).unwrap();
    assert!(public_near_earlier(&c, "me", &[pubpos(None)]).unwrap().0.is_empty(), "her content was hers first");
}

fn own(first: Option<&[&str]>) -> OwnPos {
    OwnPos { pos: 0, tokens: s(TOKS), bands: s(&["0:a:b"]), body: "BH".into(), body_toks: first.map(s), expl: None, ans: None }
}

#[test]
fn her_own_retained_earlier_version_is_found_by_body_hash_or_by_choices() {
    let (c, _d) = open();
    user(&c, "me");
    let it = "2".repeat(32);
    item(&c, &it, "me", "pm");
    publish(&c, "pm", &it, "me");
    question(&c, "pm", "q", 4, "Mc", TOKS, BODY, "BH");
    bands(&c, "own:me", &it, "q", &["0:a:b"]);
    let fp = Fingerprinter::from_secret(SECRET).unwrap();
    let (mut cut, mut loose) = (HashSet::new(), HashMap::new());
    let r = own_fuzzy_first(&c, &fp, "me", &[own(Some(BODY))], &mut cut, &mut loose).unwrap();
    assert_eq!(r[&0], 4);
    assert_eq!(loose[&0], LooseFirst { seq: 4, expl: None, ans: None });
    // A different body hash with the same choices still counts when the later version adds nothing.
    let mut p = own(Some(BODY));
    p.body = "OTHER".into();
    assert_eq!(own_fuzzy_first(&c, &fp, "me", &[p], &mut cut, &mut HashMap::new()).unwrap()[&0], 4);
    // Different choices and a different body hash: not her version.
    let mut p = own(Some(&["kind:Mc", "berlin", "madrid"]));
    p.body = "OTHER".into();
    assert!(own_fuzzy_first(&c, &fp, "me", &[p], &mut cut, &mut HashMap::new()).unwrap().is_empty());
    // A different stem: not a rewording.
    let mut p = own(Some(BODY));
    p.tokens = s(&["totally", "unrelated", "words", "go", "here", "now"]);
    assert!(own_fuzzy_first(&c, &fp, "me", &[p], &mut cut, &mut HashMap::new()).unwrap().is_empty());
    assert!(cut.is_empty());
}

#[test]
fn the_durable_keyed_record_is_consulted_too_and_the_earlier_of_the_two_wins() {
    let (c, _d) = open();
    user(&c, "me");
    let fp = Fingerprinter::from_secret(SECRET).unwrap();
    let mut memo = HashMap::new();
    let band = fp.keyed_one("b:", "0:a:b", &mut memo);
    let tok_sk = serde_json::to_string(&fp.keyed_set(&s(TOKS), &mut memo, "t:")).unwrap();
    let body_sk = serde_json::to_string(&fp.keyed_body(&s(BODY), &mut memo)).unwrap();
    c.execute("INSERT INTO author_stems (author_id, band, qhash, tok_sk, body_sk, first_seq) VALUES ('me', ?1, 'qh', ?2, ?3, 2)", params![band, tok_sk, body_sk]).unwrap();
    let (mut cut, mut loose) = (HashSet::new(), HashMap::new());
    assert_eq!(own_fuzzy_first(&c, &fp, "me", &[own(Some(BODY))], &mut cut, &mut loose).unwrap()[&0], 2, "a purged original is still hers");
    assert_eq!(loose[&0].seq, 2);
    // With a retained version at 4 as well, the earlier durable record (2) still wins.
    let it = "2".repeat(32);
    item(&c, &it, "me", "pm");
    publish(&c, "pm", &it, "me");
    question(&c, "pm", "q", 4, "Mc", TOKS, BODY, "BH");
    bands(&c, "own:me", &it, "q", &["0:a:b"]);
    assert_eq!(own_fuzzy_first(&c, &fp, "me", &[own(Some(BODY))], &mut cut, &mut HashMap::new()).unwrap()[&0], 2);
    // Without body tokens the durable record cannot be compared.
    assert_eq!(own_fuzzy_first(&c, &fp, "me", &[own(None)], &mut cut, &mut HashMap::new()).unwrap()[&0], 4);
    // Nothing to look up.
    let mut nb = own(Some(BODY));
    nb.bands = vec![];
    assert!(own_fuzzy_first(&c, &fp, "me", &[nb], &mut cut, &mut loose).unwrap().is_empty());
}
