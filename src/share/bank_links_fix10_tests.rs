//! Phase D1, round 10 (critics 81 and 82): each test written first against the code it pins. Text is compared as
//! verSHion's server stores it (Word's Symbol font mapped, blank and repeated topics dropped: checked against
//! share-core itself); a send with no clear answer is matched to the circle's questions only when the version right after
//! the one it was based on is not a colleague's; any answer from the circle ends "couldn't check".

use std::sync::Arc;

use serde_json::{Value, json};

use super::bank_links_fix4_tests::*;
use super::bank_links_fix8_tests::*;
use super::bank_links_fix9_tests::*;
use super::bank_links_fix_tests::*;
use super::bank_links_tests::*;
use super::canon;
use super::circle_edit::*;
use super::circles::CirclesModel;
use super::circles_tests::*;
use super::merge::RowKind;
use crate::model::QuestionBank;

// ---- c81 I1: compared with the server's own cleaning rules ---------------------------------------------------------

/// What verSHion's server stores of these questions: share-core's `publish_prepare`, as the Worker runs it for a circle
/// send. `None` when it refuses them.
fn stored(qs: &[Value]) -> Option<Vec<Value>> {
    let body = json!({"schema_version": 2, "bank": {"id": BID, "name": "Cells", "questions": qs, "created": 1, "modified": 2}}).to_string();
    let out: Value = serde_json::from_str(&share_core::wasm::publish_prepare_json(body.as_bytes(), "{}")).unwrap();
    if out["ok"] != true {
        return None;
    }
    Some(out["payload"]["bank"]["questions"].as_array().unwrap().clone())
}

fn question(v: &Value) -> crate::model::Question {
    serde_json::from_value(v.clone()).unwrap()
}

fn topics_of(list: &[&str]) -> Vec<String> {
    canon::topics(&list.iter().map(|s| s.to_string()).collect::<Vec<_>>())
}

#[test]
fn c81_i1_word_symbol_font_characters_compare_as_the_mapped_ones() {
    use canon::block;
    // (Word's Symbol font pasted as private-use U+F0xx: the server stores >=, !=, pi...)
    assert_eq!(block("x \u{F0B3} 0"), block("x \u{2265} 0"));
    assert_eq!(block("x \u{F0B9} y"), block("x \u{2260} y"));
    assert_eq!(block("\u{F070}r\u{b2}"), block("\u{3c0}r\u{b2}"));
    assert_eq!(block("\u{F044}x"), block("\u{394}x"));
    assert_ne!(block("\u{F0B3}"), block("\u{2264}"));
    // one the server does not map is not some other character
    assert_ne!(block("\u{F001}"), block(""));
    assert_eq!(topics_of(&["\u{F061} decay"]), topics_of(&["\u{3b1} decay"]));
}

#[test]
fn c81_i1_line_breaks_and_hangul_are_compared_as_stored() {
    use canon::block;
    // (k02) a vertical tab in ASCII text is a line break
    assert_eq!(block("a\u{B}b"), "a\nb");
    // (k03) in other text too, and the paragraph separator
    assert_eq!(block("\u{e9}\u{B}x\u{2029}y"), "e\u{301}\nx\ny");
    assert_eq!(block("\u{e9}\u{2028}x"), "e\u{301}\nx");
    // (k12) the last Hangul syllable
    assert_eq!(block("\u{d7a3}"), "\u{1112}\u{1175}\u{11c2}");
    assert_eq!(block("\u{ac00}\u{d7a3}"), "\u{1100}\u{1161}\u{1112}\u{1175}\u{11c2}");
}

#[test]
fn c81_i1_topics_compare_as_the_server_keeps_them() {
    // blank, repeated and invisible ones are dropped; white space is collapsed
    assert_eq!(topics_of(&["Algebra", "Algebra", " ", ""]), topics_of(&["Algebra"]));
    assert_eq!(topics_of(&[" Algebra  ", "Algebra"]), vec!["Algebra".to_string()]);
    assert_eq!(topics_of(&["Alg\u{200D}ebra", "Algebra"]), topics_of(&["Algebra"]));
    assert!(topics_of(&["\u{200E}", "\u{301}", "\u{200C}", "\u{3000}", "\u{301} \u{301}", "\u{200E} \u{200F}"]).is_empty());
    assert_eq!(topics_of(&["  Fran\u{e7}ais \t culture "]), topics_of(&["Franc\u{327}ais culture"]));
    // the order is kept, and a real difference is one
    assert_ne!(topics_of(&["a", "b"]), topics_of(&["b", "a"]));
    assert_ne!(topics_of(&["Algebra"]), topics_of(&["Algebra 2"]));
    assert_ne!(topics_of(&["Algebra"]), topics_of(&[]));
}

/// A Line-safe and a Block-only pool of characters teachers' text has, for the comparisons with share-core below.
const LINE_POOL: &[&str] = &[
    "a", "B", "z", "7", " ", "  ", "\u{a0}", "\u{3000}", "\u{e9}", "e\u{301}", "\u{c5}", "\u{212b}", "\u{2126}", "\u{3a9}", "\u{1e09}", "c\u{327}\u{301}",
    "a\u{323}\u{301}", "a\u{301}\u{323}", "\u{d55c}", "\u{1112}\u{1161}\u{11ab}", "\u{ac00}", "\u{d7a3}", "\u{f900}", "\u{8c48}", "\u{F0B3}", "\u{F070}", "\u{F0B9}",
    "\u{F061}", "\u{2265}", "\u{3c0}", "\u{644}", "\u{627}", "\u{622}", "\u{627}\u{653}", "\u{200C}", "\u{200D}", "\u{200E}", "\u{200F}", "\u{1F468}", "\u{2764}\u{FE0F}",
    "\u{FEFF}", "\u{2028}", "\u{2029}", "\u{B}", "\r", "\u{301}", "\u{34F}", "\u{915}\u{94D}", "\u{5D0}", "x\u{302}", "\u{1D465}",
];
const BLOCK_ONLY: &[&str] = &["\n", "\r\n", "\t", "\u{200B}", "\u{AD}", "\u{2060}", "\u{2062}", "\u{F001}"];

/// A small deterministic generator (no dependency).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn text(&mut self, pool: &[&str], extra: &[&str], max: u64) -> String {
        let n = 1 + self.next() % max;
        (0..n)
            .map(|_| {
                let k = self.next() as usize;
                if !extra.is_empty() && k % 5 == 0 { extra[k / 5 % extra.len()] } else { pool[k % pool.len()] }
            })
            .collect()
    }
}

/// (c81 I1) Every question as the server stores it is the same question as the bank's: its texts (Symbol-font
/// characters, line breaks, composed or not) and its topics (blank, repeated, invisible ones dropped). Checked against
/// share-core's own `publish_prepare` on many questions.
#[test]
fn c81_i1_what_the_server_stores_is_the_same_question_on_many_inputs() {
    let mut r = Rng(0x9E37_79B9_7F4A_7C15);
    let (mut tried, mut kept) = (0, 0);
    for i in 0..1500 {
        let stem = format!("Q{i} {}", r.text(LINE_POOL, BLOCK_ONLY, 12));
        let topics: Vec<String> = (0..r.next() % 5).map(|_| if r.next() % 4 == 0 { "Algebra".to_string() } else { r.text(LINE_POOL, &[], 6) }).collect();
        let q = json!({"id": "q1", "stem": stem, "kind": {"Mc": {"answers": [{"text": r.text(LINE_POOL, BLOCK_ONLY, 6), "correct": true}, {"text": format!("B{}", r.text(LINE_POOL, &[], 4)), "correct": false}]}},
            "points": 1.0, "difficulty": null, "explanation": r.text(LINE_POOL, BLOCK_ONLY, 8), "topics": topics, "created": 5, "modified": 6});
        tried += 1;
        let Some(s) = stored(std::slice::from_ref(&q)) else { continue };
        kept += 1;
        let (local, theirs) = (question(&q), question(&s[0]));
        assert!(super::merge::same_question(&local, &theirs), "#{i}\nlocal  {}\nstored {}", q, s[0]);
        assert_eq!(canon::topics(&local.topics), canon::topics(&theirs.topics), "#{i}: {:?} / {:?}", local.topics, theirs.topics);
    }
    assert!(kept > tried / 3, "enough questions were accepted to mean something: {kept} of {tried}");
}

/// The same, one character at a time, over every code point the server accepts: a text (stem) and a topic.
#[test]
fn c81_i1_every_character_compares_as_the_server_stores_it() {
    use share_core::text::{Class, Max, check, clean, has_visible};
    let mut skipped = 0;
    for cp in 0..=0x10FFFFu32 {
        let Some(c) = char::from_u32(cp) else { continue };
        let s = c.to_string();
        if check(&s, Class::Block).is_ok() {
            let kept = clean(&s, Class::Block, Max::Bytes(64)).unwrap();
            assert_eq!(canon::block(&kept), canon::block(&s), "U+{cp:04X} as a text");
        }
        if check(&s, Class::Line).is_ok() {
            let t = share_core::format::served_text(&clean(&s, Class::Line, Max::Chars(64)).unwrap());
            let theirs: Vec<String> = if has_visible(&t) { vec![t] } else { Vec::new() };
            let ours = canon::topics(std::slice::from_ref(&s));
            // (c81 minor 3) a letter or digit Unicode 17 added: the server's character tables (Unicode 16) do not count it
            // as visible, this app's standard library (Unicode 17) knows it; accepted
            if theirs.is_empty() && !ours.is_empty() && c.is_alphanumeric() {
                skipped += 1;
                continue;
            }
            assert_eq!(ours, canon::topics(&theirs), "U+{cp:04X} as a topic");
        }
    }
    assert!(skipped < 6000, "only Unicode 17's new letters and digits: {skipped}");
}

/// A question as a teacher pastes it from Word: Symbol-font characters, a topic twice and a blank one, an explanation
/// of nothing but a byte-order mark.
fn pasted(id: &str) -> Value {
    json!({"id": id, "stem": "If x \u{F0B3} 0 and y \u{F0B9} 0, find \u{F070}r\u{b2}", "kind": {"Mc": {"answers": [{"text": "\u{F061} = 2", "correct": true}, {"text": "\u{F062}", "correct": false}]}},
        "points": 1.0, "difficulty": null, "explanation": "\u{FEFF}", "topics": ["Algebra", "Algebra ", " ", "Geometry"], "created": 5, "modified": 6})
}

fn add_pasted(b: &mut QuestionBank) {
    b.questions.push(question(&pasted("new-local")));
}

/// Get latest on A's page from `base` to `head`: the review of `qs` (the circle's newest version).
fn latest(m: &mut CirclesModel, s: &Arc<Script>, banks: &mut [QuestionBank], ctx: &egui::Context, acct: &mut FakeAccount, base: u64, head: u64, qs: Vec<Value>) -> String {
    s.on("Post", &format!("{}/merge", put_path()), 200, merge_start(base, head));
    s.keep_raw("Get", &put_path(), served_qs(head, qs, false));
    assert!(m.edit.open_merge(LOCAL, owner()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    until_as(ctx, m, banks, acct, s, LOCAL, &|m| kinds(m).is_some());
    strip_as(ctx, m, banks, acct, &olga(s), LOCAL)
}

/// (c81 I1) Sent with a readable answer, then read back as the server stored it: the pasted question is not "changed in
/// the circle", and a correction made since is the teacher's own change, never a conflict with her own send.
#[test]
fn c81_i1_a_pasted_question_sent_and_read_back_is_unchanged() {
    for corrected in [false, true] {
        let sent = json!({"circle_id": CID, "bank_id": BID, "version": 4, "version_id": hex(4), "qids": [QA, QN], "added": 1, "changed": 0, "removed": 0});
        let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_pasted, false, None, 201, sent, (200, heads(4)));
        assert_eq!(m.edit.with_link(LOCAL, |l| l.base_version), Some(4));
        if corrected {
            banks[0].questions[1].stem.push_str(" (exact)");
            banks[0].modified += 5;
        }
        // the circle's version 5: a colleague changed question One; the pasted one as the server stored it
        let qs = stored(&[qjson(QA, "One (changed)"), pasted(QN)]).unwrap();
        assert!(qs[1]["stem"].as_str().unwrap().contains('\u{2265}') && qs[1]["topics"].as_array().unwrap().len() == 2, "{}", qs[1]);
        let page = latest(&mut m, &s, &mut banks, &ctx, &mut acct, 4, 5, qs);
        let k = kinds(&m).unwrap();
        let ours = usize::from(corrected);
        assert_eq!((count(&k, RowKind::TheirsChanged), count(&k, RowKind::Conflict), count(&k, RowKind::NewInTheirs), count(&k, RowKind::OursChanged)), (1, 0, 0, ours), "corrected {corrected}: {page}");
        assert!(!page.contains("CONFLICT"), "{page}");
    }
}

/// (c81 I1) Sent with no clear answer, then read back as the server stored it: matched to the bank's question, never
/// "New in the circle" with Add ticked (a second copy).
#[test]
fn c81_i1_a_pasted_question_sent_without_a_clear_answer_is_not_new_in_the_circle() {
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_pasted, false, timed_out(), 0, Value::Null, (200, heads(4)));
    checked(&mut m, &s, &mut banks, &ctx, &mut acct);
    let qs = stored(&[qjson(QA, "One"), pasted(QN)]).unwrap();
    let page = latest(&mut m, &s, &mut banks, &ctx, &mut acct, 3, 4, qs);
    let k = kinds(&m).unwrap();
    assert_eq!((count(&k, RowKind::NewInTheirs), count(&k, RowKind::Conflict)), (0, 0), "{page}");
    assert!(!page.contains("New in the circle") && page.contains("Version 4 is your own send"), "{page}");
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!(banks[0].questions.len(), 2, "not added twice");
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 0, 0));
}

// ---- the comparisons' own rules (mutation survivors of round 9) -----------------------------------------------------

fn q_of(kind: Value) -> crate::model::Question {
    question(&json!({"id": "q", "stem": "Stem", "kind": kind, "points": 1.0, "difficulty": null, "created": 5, "modified": 6}))
}

/// Every text of every kind of question is compared as the server stores it: composed or not is the same question.
#[test]
fn c81_every_kind_of_question_is_compared_as_stored() {
    use super::merge::same_question;
    let (c, d) = ("Caf\u{e9}", "Cafe\u{301}");
    // (m03) matching items and distractors
    let matching = |e: &str| q_of(json!({"Matching": {"pairs": [{"left": format!("L {e}"), "right": format!("R {e}")}], "distractors": [format!("D {e}")]}}));
    assert!(same_question(&matching(c), &matching(d)));
    assert!(!same_question(&matching(c), &matching("Cafe")));
    // (m04) accepted blanks
    let blank = |e: &str| q_of(json!({"Blank": {"accept": [[e, "x"]]}}));
    assert!(same_question(&blank(c), &blank(d)));
    assert!(!same_question(&blank(c), &blank("Cafe")));
    // (m05, m16) a part's stem and its choices
    let parts = |stem: &str, choice: &str| q_of(json!({"Multipart": {"parts": [{"stem": stem, "kind": {"Mc": {"answers": [{"text": choice, "correct": true}, {"text": "B", "correct": false}]}}}]}}));
    assert!(same_question(&parts(c, "x"), &parts(d, "x")), "part stem");
    assert!(same_question(&parts("x", c), &parts("x", d)), "part choice");
    assert!(!same_question(&parts("x", c), &parts("x", "Cafe")));
    // and the points
    let mut two = blank(c);
    two.points = Some(2.0);
    assert!(!same_question(&blank(c), &two));
}

/// (m08) A figure with a value the other does not have is another figure.
#[test]
fn c81_a_figure_with_one_more_value_is_not_the_same() {
    use super::merge::same_question;
    let fig = |params: Value| {
        let mut q = q_of(json!({"Tf": {"correct": true}}));
        q.figure = Some(serde_json::from_value(json!({"id": "number-line", "params": params})).unwrap());
        q
    };
    assert!(same_question(&fig(json!({"min": 1})), &fig(json!({"min": 1.0}))));
    assert!(!same_question(&fig(json!({"min": 1})), &fig(json!({"min": 1, "max": 9}))));
    assert!(!same_question(&fig(json!({"min": 1, "max": 9})), &fig(json!({"min": 1}))));
}

/// What an unclear send carried, matched to the circle's new questions: each sent question once (m11), and only to a
/// question with the same picture (m12).
#[test]
fn c81_what_a_send_carried_is_matched_once_and_with_its_picture() {
    // (round 11: matched by `rebase_own_send`, which links them through the base)
    use super::merge::{build_plan, rebase_own_send};
    let (mut bank, link) = linked(&[(QA, "One")]);
    let mut mine = q_with("mine-two", "Two", COMPOSED);
    mine.image = Some(serde_json::from_value(json!({"file": "media/aaaa.png", "width": 4, "height": 4})).unwrap());
    bank.questions.push(mine.clone());
    let pic = |q: &crate::model::Question, t: &super::merge::Theirs| q.image.as_ref().and_then(|i| super::circle_links::sha_of_file(&i.file)) == t.sha;
    // the circle has it twice (a colleague added the same question too), with the same picture
    let mut theirs = super::circle_links::circle_theirs(&payload(&[(QA, "One"), (QB, "x"), (QC, "y")]), NOW);
    for t in &mut theirs[1..] {
        t.q = q_with("t", "Two", COMPOSED);
        t.sha = Some("aaaa".into());
    }
    let linked_ids = [QA];
    let mut base = link.base.clone();
    let n = rebase_own_send(&mut base, &linked_ids, &[("", &mine)], &theirs, &pic, true).added.len();
    let plan = build_plan(BID, &base, &bank, &theirs, 1, 2, None, String::new());
    assert_eq!((n, plan.count(RowKind::NewInTheirs), plan.count(RowKind::Unchanged)), (1, 1, 2), "linked once; the second is new");
    // with another picture it is not what was sent
    for t in &mut theirs[1..] {
        t.sha = Some("bbbb".into());
    }
    let mut base = link.base.clone();
    let n = rebase_own_send(&mut base, &linked_ids, &[("", &mine)], &theirs, &pic, true).added.len();
    let plan = build_plan(BID, &base, &bank, &theirs, 1, 2, None, String::new());
    assert_eq!((n, plan.count(RowKind::NewInTheirs)), (0, 2));
}

// ---- what an unclear send carried, in the links file ------------------------------------------------------------------

/// A link as the links file keeps it, with `record` as its `sent_record`.
fn link_file_with(record: Value) -> Vec<u8> {
    let (_, l) = linked(&[(QA, "One")]);
    let mut v = serde_json::to_value(&l).unwrap();
    v["sent_record"] = record;
    json!({"links": [v]}).to_string().into_bytes()
}

fn load(bytes: Vec<u8>) -> (Vec<super::circle_links::CircleLink>, bool) {
    let dir = std::env::temp_dir().join(format!("vershion-c81-{}-{}", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(super::circle_links::LINKS_DOC), bytes).unwrap();
    let log = super::circle_links::LinkLog::Dir(dir.clone());
    let links = log.load_checked().unwrap();
    let bad = dir.join(format!("{}.bad", super::circle_links::LINKS_DOC)).exists();
    let _ = std::fs::remove_dir_all(&dir);
    (links, bad)
}

static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// (c81 minor 2) A send record this build can not read (a field of the wrong type, a question of a kind it does not know,
/// one missing its times) is dropped alone: the link itself is kept, with its base and question ids.
#[test]
fn c81_m2_a_send_record_that_can_not_be_read_does_not_drop_the_link() {
    let q = |kind: Value| json!({"id": "x", "stem": "S", "kind": kind, "points": 1.0, "difficulty": null, "created": 5, "modified": 6});
    let mut no_created = q(json!({"Tf": {"correct": true}}));
    no_created.as_object_mut().unwrap().remove("created");
    for (what, record) in [
        ("wrong type", json!({"by": "", "base": "three", "questions": []})),
        ("not an object", json!("record")),
        ("unknown kind", json!({"by": "", "base": 3, "questions": [{"qid": QA, "q": q(json!({"Essay": {"words": 300}}))}]})),
        ("missing created", json!({"by": "", "base": 3, "questions": [{"qid": QA, "q": no_created}]})),
    ] {
        let (links, _) = load(link_file_with(record));
        assert_eq!(links.len(), 1, "{what}: the link is kept");
        assert!(links[0].sent_record.is_none(), "{what}: only the record goes");
        assert_eq!((links[0].base.len(), links[0].qids.len()), (1, 1), "{what}");
    }
    // (l04) one written before a field existed is read, its missing fields as their defaults
    let (links, bad) = load(link_file_with(json!({"by": "", "base": 3, "questions": [{"qid": QA, "q": q(json!({"Tf": {"correct": true}}))}]})));
    let r = links[0].sent_record.as_ref().expect("an older record is read");
    assert_eq!((r.base, r.not_through, r.modified, r.questions.len()), (3, false, 0, 1));
    assert!(!bad);
}

/// (l01) A send too big to keep keeps no questions (like a base): only the version it was based on.
#[test]
fn c81_a_send_record_too_big_to_keep_keeps_no_questions() {
    let (mut bank, l) = linked(&[(QA, "One")]);
    let one = bank.questions[0].clone();
    for i in 0..super::merge::BASE_MAX_QUESTIONS {
        let mut q = one.clone();
        q.id = format!("n{i}");
        bank.questions.push(q);
    }
    let r = super::circle_links::sent_record(&l, &bank, 3, String::new());
    assert!(r.questions.is_empty() && r.base == 3);
    bank.questions.truncate(2);
    bank.questions[1].stem = "x".repeat(super::merge::BASE_MAX_BYTES);
    assert!(super::circle_links::sent_record(&l, &bank, 3, String::new()).questions.is_empty(), "too many bytes");
    bank.questions[1].stem = "x".into();
    assert_eq!(super::circle_links::sent_record(&l, &bank, 3, String::new()).questions.len(), 2);
}

// ---- c81 I2 and minor 1: a send with no clear answer, and the version right after the one it was based on ----------------

/// The merge-initiate answer from `base` to `head`, every version in between sent by `by`.
pub(super) fn merge_start_by(base: u64, head: u64, by: &str) -> Value {
    let mut v = merge_start(base, head);
    v["head_by"] = json!(by);
    for l in v["between"].as_array_mut().unwrap() {
        l["by"] = json!(by);
    }
    v
}

/// The merge-initiate answer from 3 to 5: Bob Teacher's version 4, then Olga Teacher's 5.
pub(super) fn two_since() -> Value {
    let mut v = merge_start_by(3, 5, "Olga Teacher");
    let mut four = v["between"][0].clone();
    four["version"] = json!(4);
    four["version_id"] = json!(hex(4));
    four["by"] = json!("Bob Teacher");
    v["between"] = json!([four, v["between"][0].clone()]);
    v
}

/// Get latest on A's page as `who`: the review of `served`, `start` the merge-initiate answer.
pub(super) fn latest_as(m: &mut CirclesModel, s: &Arc<Script>, banks: &mut [QuestionBank], ctx: &egui::Context, acct: &mut FakeAccount, who: &super::publish::AccountSnapshot, start: Value, served: Vec<u8>) -> String {
    s.on("Post", &format!("{}/merge", put_path()), 200, start);
    s.keep_raw("Get", &put_path(), served);
    assert!(m.edit.open_merge(LOCAL, owner()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    for _ in 0..500 {
        let t = strip_as(ctx, m, banks, acct, who, LOCAL);
        if kinds(m).is_some() {
            return t;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("timed out: {:?}", m.edit.view);
}

/// (c81 I2, the critic's P1) The teacher's send got no clear answer; a colleague added the same question in the version
/// right after; the teacher corrected hers since. Get latest never takes the colleague's question for hers (which would
/// link it to the corrected one, and the next Send would overwrite it): whether the check named the colleague first or
/// could not reach the circle.
#[test]
fn c81_i2_a_colleagues_same_question_is_never_taken_for_the_persons_own_send() {
    for check in ["colleague first", "offline", "two versions since", "colleague first, name not known since"] {
        let answer = if check.starts_with("colleague first") { (200, super::bank_links_fix9_tests::heads_by(4, "Bob Teacher")) } else { (503, json!({"error": "unavailable"})) };
        let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, true, timed_out(), 0, Value::Null, answer);
        let who = olga_named(&s);
        frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
        banks[0].questions.iter_mut().find(|q| q.stem == ADDED).unwrap().stem = FIXED.into();
        banks[0].modified += 5;
        // (two versions since: Bob's 4, then the teacher's own 5, sent from another computer)
        let (start, head) = if check == "two versions since" { (two_since(), 5) } else { (merge_start_by(3, 4, "Bob Teacher"), 4) };
        // (the check named the colleague; the Account window has not given the name again since: what the check found
        // still holds)
        let at_get = if check.ends_with("not known since") { olga(&s) } else { who.clone() };
        let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &at_get, start, served(head, &[(QA, "One"), (QN, ADDED)], false));
        let k = kinds(&m).unwrap();
        assert_eq!((count(&k, RowKind::NewInTheirs), count(&k, RowKind::OursChanged)), (1, 0), "{check}: {page}");
        assert!(!page.contains("is your own send") && !page.contains("You sent this"), "{check}: {page}");
        apply(&mut m, &s, &mut banks, &ctx, &mut acct);
        assert_eq!(banks[0].questions.iter().filter(|q| q.stem == ADDED || q.stem == FIXED).count(), 2, "{check}: both kept");
        assert_eq!(changes(&mut m, &banks[0].clone()), (1, 0, 0), "{check}: the teacher's question is still hers to send, as new");
    }
}

/// The version right after it is the person's own (by name): the send is matched, also after the check said the circle
/// still had the version it was based on (a slow commit landed after the check).
#[test]
fn c81_i2_the_persons_own_next_version_is_their_send() {
    for check in [(200, heads(3)), (503, json!({"error": "unavailable"}))] {
        let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, true, timed_out(), 0, Value::Null, check.clone());
        let who = olga_named(&s);
        frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
        banks[0].questions.iter_mut().find(|q| q.stem == ADDED).unwrap().stem = FIXED.into();
        banks[0].modified += 5;
        let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, merge_start_by(3, 4, "Olga Teacher"), served(4, &[(QA, "One"), (QN, ADDED)], false));
        let k = kinds(&m).unwrap();
        assert_eq!((count(&k, RowKind::NewInTheirs), count(&k, RowKind::OursChanged)), (0, 1), "{}: {page}", check.0);
        assert!(page.contains("Version 4 is your own send"), "{}: {page}", check.0);
    }
}

/// (c81 minor 5) A check right after a send with no clear answer that still finds the version it was based on is not
/// proof: the send may still land. It says so, and keeps what the send carried.
#[test]
fn c81_m5_a_check_that_still_finds_the_old_version_says_the_send_may_not_have_arrived_yet() {
    let (mut m, s, mut banks, ctx, mut acct) = unclear_send(timed_out(), 0, Value::Null, 3);
    checked(&mut m, &s, &mut banks, &ctx, &mut acct);
    let r = said(&m, LOCAL);
    assert!(r.contains("may not have arrived yet") && r.contains("still has version 3") && r.contains("Send my changes"), "{r}");
    assert!(!r.contains("did not go through"), "{r}");
    assert!(m.edit.with_link(LOCAL, |l| l.sent_record.as_ref().is_some_and(|r| !r.not_through)).unwrap(), "not settled");
    // a restore's check says the same
    let (mut m, s, mut banks, ctx, mut acct) = restore_answered(timed_out(), 0, Value::Null, true);
    s.keep("Post", "/circles/heads", 200, heads(3));
    frames(&mut m, &mut banks, &ctx, &mut acct, &olga(&s), 3);
    let r = said(&m, LOCAL);
    assert!(r.contains("may not have arrived yet") && !r.contains("did not go through"), "{r}");
}

/// (c81 minor 1, e03, e04) Who sent a version is told by the display name the circle gives (it gives no account id):
/// exactly the person's own, white space aside.
#[test]
fn c81_m1_a_version_is_the_persons_own_only_under_exactly_their_name() {
    for (by, own) in [("Olga Teacher", true), (" Olga  Teacher ", true), ("olga teacher", false), ("Olga Teacher2", false)] {
        let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, true, timed_out(), 0, Value::Null, (200, super::bank_links_fix9_tests::heads_by(4, by)));
        frames(&mut m, &mut banks, &ctx, &mut acct, &olga_named(&s), 3);
        let r = said(&m, LOCAL);
        assert_eq!(r.contains("sent by you"), own, "{by:?}: {r}");
        assert_eq!(r.contains("did not go through"), !own, "{by:?}: {r}");
    }
}

/// (e08) A conflict with the person's own version that is not the one right after the send's: it may be that send.
#[test]
fn c81_a_conflict_with_the_persons_version_two_above_may_be_the_earlier_send() {
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, true, timed_out(), 0, Value::Null, (503, json!({"error": "unavailable"})));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    let page = conflict_by(&mut m, &s, &mut banks, &ctx, &mut acct, &who, 5, "Olga Teacher");
    assert!(page.contains("It may be your earlier send") && !page.contains("Your earlier send went through"), "{page}");
}

/// (c81 minor 6) A record whose version is the largest there is does not overflow anything.
#[test]
fn c81_m6_the_largest_version_number_does_not_overflow() {
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, true, timed_out(), 0, Value::Null, (503, json!({"error": "unavailable"})));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    m.edit.update_link(LOCAL, |l| l.sent_record.as_mut().map(|r| r.base = u64::MAX).is_some());
    let page = conflict_by(&mut m, &s, &mut banks, &ctx, &mut acct, &who, 5, "Olga Teacher");
    assert!(!page.contains("Your earlier send went through"), "{page}");
    m.edit.close();
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, merge_start_by(3, 4, "Olga Teacher"), served(4, &[(QA, "One"), (QN, ADDED)], false));
    assert!(!page.contains("is your own send"), "{page}");
}

/// (e14) What one account's send carried is never matched for another account signed in on this computer.
#[test]
fn c81_another_accounts_unclear_send_is_not_this_ones() {
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, false, timed_out(), 0, Value::Null, (503, json!({"error": "unavailable"})));
    let pete = as_who(&s, "pete");
    frames(&mut m, &mut banks, &ctx, &mut acct, &pete, 2);
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &pete, merge_start(3, 4), served(4, &[(QA, "One"), (QN, ADDED)], false));
    assert!(!page.contains("is your own send") && !page.contains("You sent this"), "{page}");
}

/// The record of a send with no clear answer, set straight into A's link (based on version 3, by no known account).
fn with_record(m: &mut CirclesModel, questions: Vec<super::circle_links::SentQuestion>) {
    m.edit.update_link(LOCAL, |l| {
        l.sent_record = Some(super::circle_links::SentRecord { by: String::new(), base: 3, not_through: false, modified: 0, questions });
        true
    });
}

fn a_rig() -> (CirclesModel, Arc<Script>, Vec<QuestionBank>, egui::Context, FakeAccount) {
    let (mut m, s, mut banks) = two_at_three();
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("owner", json!({})));
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga(&s), LOCAL);
    (m, s, banks, ctx, acct)
}

/// (e15) A question the send carried with a picture is not what the circle has without it: the difference stays seen.
#[test]
fn c81_a_sent_question_is_matched_only_with_its_picture() {
    let (mut m, s, mut banks, ctx, mut acct) = a_rig();
    let mut sent = banks[0].questions[0].clone();
    sent.stem = "One (13)".into();
    sent.image = Some(serde_json::from_value(json!({"file": "media/aaaa.png", "width": 4, "height": 4})).unwrap());
    banks[0].questions[0] = sent.clone();
    banks[0].modified += 5;
    with_record(&mut m, vec![super::circle_links::SentQuestion { qid: QA.into(), q: sent }]);
    let page = get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served(4, &[(QA, "One (13)")], false));
    let k = kinds(&m).unwrap();
    assert_eq!(count(&k, RowKind::Unchanged), 0, "the circle's has no picture: {page}");
    assert!(!page.contains("is your own send"), "{page}");
}

/// (e19) A sent question is matched only to the bank question it was sent from (its id here), not to another linked to
/// the same circle question.
#[test]
fn c81_a_sent_question_is_matched_only_to_the_bank_question_it_was_sent_from() {
    let (mut m, s, mut banks, ctx, mut acct) = a_rig();
    let mut sent = banks[0].questions[0].clone();
    sent.id = "another-local".into();
    sent.stem = "One (13)".into();
    with_record(&mut m, vec![super::circle_links::SentQuestion { qid: QA.into(), q: sent }]);
    let page = get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served(4, &[(QA, "One (13)")], false));
    let k = kinds(&m).unwrap();
    assert_eq!(count(&k, RowKind::TheirsChanged), 1, "the circle changed the bank's question: {page}");
    assert!(!page.contains("is your own send"), "{page}");
}

/// (e20) A send asked for while no account was known, answered after another account signed in: what it carried is not
/// kept for the account signed in now.
#[test]
fn c81_a_send_by_an_unknown_account_answered_after_another_signs_in_is_not_kept_for_it() {
    let (mut m, s, mut banks, ctx, mut acct) = a_rig();
    let unknown = ready(&s);
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &unknown, LOCAL);
    banks[0].questions[0].stem = "One (13)".into();
    banks[0].modified += 5;
    assert!(m.edit.open_send(LOCAL, owner()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    frames(&mut m, &mut banks, &ctx, &mut acct, &unknown, 3);
    m.edit.set_attest(true);
    s.on("Post", "/circles/heads", 200, heads(3));
    s.fail(timed_out().unwrap());
    m.edit.send();
    m.edit.sync(&mut banks);
    assert!(s.paths().iter().any(|p| p.starts_with("Put ")), "{:?}", s.paths());
    frames(&mut m, &mut banks, &ctx, &mut acct, &as_who(&s, "pete"), 2);
    assert!(m.edit.with_link(LOCAL, |l| l.sent_record.is_none()).unwrap());
}

// ---- c82 I1: any answer from the circle ends "Couldn't check the circle just now" ---------------------------------------

/// (c82 I1) The send times out and the check asked for at once can not reach the circle either (the same wifi); the Send
/// page, still open, says "Press Try again". Pressed a minute later, the circle answers: the strip under the page that
/// closes no longer says it could not check, and after a conflict it offers Get latest beside the note that says to.
#[test]
fn c82_i1_an_answer_from_the_circle_ends_couldnt_check() {
    for answer in ["went through", "conflict"] {
        let (mut m, s, mut banks) = two_at_three();
        s.keep("Get", &format!("/circles/{CID}"), 200, detail("owner", json!({})));
        let ctx = ctx();
        let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
        let who = olga_named(&s);
        strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
        add_question(&mut banks[0]);
        banks[0].modified += 5;
        assert!(m.edit.open_send(LOCAL, owner()));
        m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
        frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
        m.edit.set_attest(true);
        s.on("Post", "/circles/heads", 200, heads(3));
        s.fail(timed_out().unwrap());
        m.edit.send();
        m.edit.sync(&mut banks);
        // the check asked for at once fails too
        s.on("Post", "/circles/heads", 503, json!({"error": "unavailable"}));
        let page = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
        assert!(page.contains("may have gone through") && page.contains("Try again"), "{page}");
        assert_eq!(m.links.check_of(LOCAL), super::bank_links::Check::Offline);
        // a minute later the page's Try again is answered
        match answer {
            "went through" => {
                s.on("Put", &put_path(), 201, json!({"circle_id": CID, "bank_id": BID, "version": 4, "version_id": hex(4), "qids": [QA, QN], "added": 1, "changed": 0, "removed": 0, "replayed": true}));
                s.keep("Post", "/circles/heads", 200, super::bank_links_fix9_tests::heads_by(4, "Olga Teacher"));
            }
            _ => {
                s.on("Put", &put_path(), 409, json!({"error": "conflict", "head_version": 4, "head_at": NOW - 60_000, "head_by": "Bob Teacher"}));
                s.keep("Post", "/circles/heads", 200, super::bank_links_fix9_tests::heads_by(4, "Bob Teacher"));
            }
        }
        m.edit.retry_act();
        m.edit.sync(&mut banks);
        let page = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
        assert!(page.contains(if answer == "conflict" { "Bob Teacher sent version 4" } else { "Version 4 is now the circle's newest" }), "{answer}: {page}");
        m.edit.close();
        let t = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
        assert!(!t.contains("Couldn't check the circle"), "{answer}: {t}");
        if answer == "conflict" {
            assert!(t.contains("Get latest") && t.contains("sent version 4 first") && !t.contains("Try again"), "{t}");
        } else {
            assert!(t.contains("Version 4 is the latest") && t.contains("up to date"), "{t}");
        }
    }
}

/// (c82 I1) The Send page opened again after "Couldn't check": its licence read reaches the circle, so the strip it leaves
/// checks again instead of going on saying it could not.
#[test]
fn c82_i1_the_send_pages_licence_read_ends_couldnt_check() {
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, true, timed_out(), 0, Value::Null, (503, json!({"error": "unavailable"})));
    let who = olga_named(&s);
    let t = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert!(t.contains("Couldn't check the circle"), "{t}");
    s.keep("Post", "/circles/heads", 200, super::bank_links_fix9_tests::heads_by(3, "Olga Teacher"));
    assert!(m.edit.open_send(LOCAL, owner()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    m.edit.close();
    let t = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert!(!t.contains("Couldn't check the circle") && t.contains("Send my changes"), "{t}");
}
