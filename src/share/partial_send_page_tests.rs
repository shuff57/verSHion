//! Circles phase D3, piece 4 (docs/circles-phaseD-plan.md 4.7, 5.5 and 6): the Send page's summary of changes
//! against the base version's body. Each test pins one rule: the body is read once when the page opens, the
//! defaults are every change ticked except `held_back`, an unticked change goes as the base's raw question, an
//! unticked removal is put back and keeps its pair, a base that can not be had sends the whole bank said plainly,
//! nothing ticked refuses the send, and an edit made while the send was in flight shows as unsent afterwards.
//!
//! The drawing is the next piece: these tests drive the model only. (phase D3, piece 5) The tests at the end draw
//! the page itself, at 1280 and 420, with the existing `drawn` / `click_text` helpers.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::api::ApiError;
use super::bank_links_fix11_tests::version_path;
use super::bank_links_fix4_tests::{background, ctx, frame_text, owner};
use super::bank_links_fix_tests::{QC, linked_from, qfull, served_qs};
use super::bank_links_tests::{LOCAL, QA, QB, heads, hex};
use super::browse::ImageSink;
use super::circle_edit::*;
use super::circle_hide::{Decisions, Pictures};
use super::circle_links::{CircleLink, LinkLog, changes_since, payload_of};
use super::circles::CirclesModel;
use super::circles_phasec_tests::click_text;
use super::circles_tests::{BID, NoDiskRestore, Script, model, now, ready};
use super::merge::{Choice, RowKind};
use super::model::Runner;
use super::partial_send::BodyChanges;
use super::pool::{LabelChoice, LocalProblem, MediaSource, PayloadRefusal, Pos, RetryPolicy, sha256_hex};
use super::transport::Method;
use crate::model::{ImageRef, QuestionBank};

/// The question only this bank has (added: no circle id).
const QD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa4";
/// The circle id the answer gives the added question.
const QNEW: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa9";

fn base_qs() -> Vec<Value> {
    vec![qfull(QA, "A"), qfull(QB, "B"), qfull(QC, "C")]
}

fn added_question() -> crate::model::Question {
    let mut q = super::circle_links::circle_theirs(&super::bank_links_fix_tests::body_of(vec![qfull(QD, "D")]), super::circles_tests::NOW).remove(0).q;
    q.id = crate::model::new_id();
    q
}

/// The local id this copy has for a circle question.
fn local(link: &CircleLink, qid: &str) -> String {
    link.qids.iter().find(|q| q.qid == qid).expect("a linked question").local.clone()
}

/// A question as the base body has it, with a picture (`media/<sha256>.<ext>`).
fn qfull_pic(id: &str, stem: &str, file: &str) -> Value {
    let mut q = qfull(id, stem);
    q["image"] = json!({"file": file, "width": 4, "height": 3});
    q
}

/// The circle's blob route for a picture (the fetch the merge already makes).
fn blob_path(sha: &str) -> String {
    format!("/circles/{}/banks/{}/blobs/{sha}", super::circles_tests::CID, super::circles_tests::BID)
}

/// Where a send's pictures go: every `images.put` is kept so a test can say the fetched bytes were stored.
#[derive(Default)]
struct Pics(Mutex<Vec<Vec<u8>>>);

impl ImageSink for Pics {
    fn put(&self, bytes: &[u8]) -> Result<ImageRef, String> {
        self.0.lock().unwrap().push(bytes.to_vec());
        Ok(ImageRef { file: format!("media/{}.webp", sha256_hex(bytes)), width: 2, height: 2 })
    }
}

/// What a test's `MediaSource::read` answers: pictures on this computer, by file name.
#[derive(Default)]
struct MediaOnDisk(Mutex<HashMap<String, Vec<u8>>>);

impl MediaSource for MediaOnDisk {
    fn read(&self, file: &str) -> Option<Vec<u8>> {
        self.0.lock().unwrap().get(file).cloned()
    }
    fn keep(&self, file: &str, bytes: &[u8]) -> bool {
        self.put(file, bytes);
        true
    }
}

impl MediaOnDisk {
    fn put(&self, file: &str, bytes: &[u8]) {
        self.0.lock().unwrap().insert(file.to_string(), bytes.to_vec());
    }
}

/// The circles model with a test's picture sink and media store (the send fetches a base picture through the one
/// and reads this computer's through the other).
fn model_with(images: Arc<Pics>, media: Arc<MediaOnDisk>) -> CirclesModel {
    let mut m = CirclesModel::new(Runner::Inline, images.clone()).with_retry(RetryPolicy::none());
    m.edit = CircleEditModel::new(Runner::Inline, images, media, Arc::new(NoDiskRestore), LinkLog::memory()).with_retry(RetryPolicy::none());
    m.set_clock(super::circles_tests::NOW);
    m
}

/// A working copy in step with version 3 (the base body is `base_qs`), `edit` applied, and the Send page opened
/// with its base body read once. The links file lives in memory.
fn open_page(edit: &dyn Fn(&mut QuestionBank), held_back: &[&str]) -> (CirclesModel, Arc<Script>, Vec<QuestionBank>) {
    open_page_in(base_qs(), edit, held_back, model())
}

/// The same, with the pictures of a test's own model.
fn open_page_in(base: Vec<Value>, edit: &dyn Fn(&mut QuestionBank), held_back: &[&str], mut m: CirclesModel) -> (CirclesModel, Arc<Script>, Vec<QuestionBank>) {
    let (mut bank, mut link) = linked_from(base.clone(), false);
    link.base_version = 3;
    link.head_seen = 3;
    link.role_seen = Some("owner".into());
    link.author_known = true;
    link.is_author = true;
    link.held_back = held_back.iter().map(|qid| local(&link, qid)).collect();
    edit(&mut bank);
    bank.modified += 5;
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, heads(3));
    s.keep_raw("Get", &version_path(3), served_qs(3, base, false));
    let mut banks = vec![bank];
    m.edit.add_link(link).unwrap();
    m.edit.frame(now(), &ready(&s));
    m.edit.sync(&mut banks);
    assert!(m.edit.open_send(LOCAL, owner()));
    m.edit.frame(now(), &ready(&s));
    m.edit.sync(&mut banks);
    m.edit.set_attest(true);
    (m, s, banks)
}

/// The same, with the circle answering the base read `status`/`body` instead.
fn open_page_base_error(status: u16, body: Value, edit: &dyn Fn(&mut QuestionBank)) -> (CirclesModel, Arc<Script>, Vec<QuestionBank>) {
    open_page_base_error_in(base_qs(), status, body, edit, model())
}

/// The same, with a test's model and base body (for a base question with a picture).
fn open_page_base_error_in(base: Vec<Value>, status: u16, body: Value, edit: &dyn Fn(&mut QuestionBank), mut m: CirclesModel) -> (CirclesModel, Arc<Script>, Vec<QuestionBank>) {
    let (mut bank, mut link) = linked_from(base, false);
    link.base_version = 3;
    link.head_seen = 3;
    link.role_seen = Some("owner".into());
    link.author_known = true;
    link.is_author = true;
    edit(&mut bank);
    bank.modified += 5;
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, heads(3));
    s.on("Get", &version_path(3), status, body);
    let mut banks = vec![bank];
    m.edit.add_link(link).unwrap();
    m.edit.frame(now(), &ready(&s));
    m.edit.sync(&mut banks);
    assert!(m.edit.open_send(LOCAL, owner()));
    m.edit.frame(now(), &ready(&s));
    m.edit.sync(&mut banks);
    m.edit.set_attest(true);
    (m, s, banks)
}

/// The same, for a base body in another order (the circle's own order).
fn open_page_with_base(base: Vec<Value>, edit: &dyn Fn(&mut QuestionBank), held_back: &[&str]) -> (CirclesModel, Arc<Script>, Vec<QuestionBank>) {
    open_page_in(base, edit, held_back, model())
}

/// Frames and syncs until the send summary is ready (and the licence is known).
fn wait_ready(m: &mut CirclesModel, s: &Arc<Script>, banks: &mut [QuestionBank]) {
    for _ in 0..500 {
        m.edit.frame(now(), &ready(s));
        m.edit.sync(banks);
        if matches!(m.edit.partial_send(), Some(PartialSend::Ready { .. })) && matches!(&m.edit.view, EditView::Send(v) if matches!(v.licence, LicenceState::Known(_))) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("not ready: {:?}", m.edit.view);
}

fn ticked(m: &CirclesModel) -> BTreeSet<String> {
    m.edit.send_ticked().expect("the summary is ready").clone()
}

fn changes(m: &CirclesModel) -> BodyChanges {
    m.edit.send_changes().expect("the summary is ready").clone()
}

fn puts(s: &Arc<Script>) -> Vec<super::circles_tests::Sent> {
    s.seen().into_iter().filter(|x| x.method == Method::Put).collect()
}

/// The payload of the last PUT (the body is the meta line, a newline, then the payload).
fn put_payload(x: &super::circles_tests::Sent) -> Value {
    let text = String::from_utf8(x.body.clone()).expect("a UTF-8 send body");
    let (_, payload) = text.split_once('\n').expect("a meta line and a payload");
    serde_json::from_str(payload).expect("the payload is JSON")
}

/// The meta line of the last PUT (the decisions and the commit id).
fn put_meta(x: &super::circles_tests::Sent) -> Value {
    let text = String::from_utf8(x.body.clone()).expect("a UTF-8 send body");
    serde_json::from_str(text.split_once('\n').expect("a meta line and a payload").0).expect("the meta is JSON")
}

fn question_stems(v: &Value) -> Vec<String> {
    v.pointer("/bank/questions").and_then(Value::as_array).expect("bank.questions").iter().map(|q| q["stem"].as_str().unwrap().to_string()).collect()
}

fn qid_of(q: &Value) -> Option<&str> {
    q.pointer("/published/qid").and_then(Value::as_str)
}

/// The answer to a send that made `version` from the composed questions, in payload order.
fn sent_with(version: u64, qids: &[&str]) -> Value {
    json!({"circle_id": super::circles_tests::CID, "bank_id": super::circles_tests::BID, "version": version, "version_id": hex(version), "qids": qids, "added": 0, "changed": 0, "removed": 0})
}

/// Runs the send to its answer.
fn send(m: &mut CirclesModel, s: &Arc<Script>, banks: &mut [QuestionBank], answer: Value) {
    s.on("Put", &super::bank_links_fix8_tests::put_path(), 201, answer);
    m.edit.send();
    m.edit.sync(banks);
    m.edit.frame(now(), &ready(s));
}

// ---- opening the page: one read, every change ticked but held_back ---------------------------------------------------

#[test]
fn d3_the_send_page_reads_the_base_body_once_and_starts_every_change_ticked_but_held_back() {
    // base [A, B, C]; this bank [A', B, D] (A changed, B unchanged, C removed, D added)
    let (m, s, _banks) = open_page(
        &|b| {
            b.questions[0].stem = "A (changed)".into();
            b.questions.pop();
            b.questions.push(added_question());
        },
        &[],
    );
    let read = s.paths().iter().filter(|p| **p == format!("Get {}", version_path(3))).count();
    assert_eq!(read, 1, "the base body is read once: {:?}", s.paths());
    let c = changes(&m);
    assert_eq!((c.changed.len(), c.added.len(), c.removed.len()), (1, 1, 1), "{c:?}");
    assert_eq!(ticked(&m), c.ids(), "all changes start ticked");
    assert!(matches!(m.edit.partial_send(), Some(PartialSend::Ready { .. })));
}

#[test]
fn d3_a_held_back_id_starts_unticked_and_one_no_longer_changed_is_not_ticked() {
    // base [A, B, C]; this bank [A', B] (A changed, C removed); held back: B (unchanged now) and C (still changed)
    let (mut m, s, mut banks) = open_page(
        &|b| {
            b.questions[0].stem = "A (changed)".into();
            b.questions.pop();
        },
        &[QB, QC],
    );
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let (a, b, c) = (local(&link, QA), local(&link, QB), local(&link, QC));
    let t = ticked(&m);
    assert_eq!(t, BTreeSet::from([a]), "only the change not held back is ticked: {t:?}");
    assert!(!t.contains(&b), "an id no longer changed is never ticked");
    assert!(!t.contains(&c), "a held back change starts unticked");
    // The send keeps only the unticked id that still differs: B is dropped from `held_back`.
    send(&mut m, &s, &mut banks, sent_with(4, &[QA, QB, QC]));
    assert_eq!(m.edit.link_for_local(LOCAL).unwrap().held_back, vec![c], "a held back id no longer changed is dropped");
    assert_eq!(changes_since(&m.edit.link_for_local(LOCAL).unwrap(), &banks[0]).removed, 1);
}

// ---- unticking a change: the base's raw question, and the link afterwards -------------------------------------------

#[test]
fn d3_an_unticked_change_goes_as_the_bases_raw_question_and_stays_unsent_after_the_send() {
    let (mut m, s, mut banks) = open_page(
        &|b| {
            b.questions[0].stem = "A (changed)".into();
            b.questions.pop();
            b.questions.push(added_question());
        },
        &[],
    );
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let (a, x, d) = (local(&link, QA), local(&link, QC), banks[0].questions[2].id.clone());
    m.edit.toggle_change(&a, false);
    assert!(!m.edit.send_ticked().unwrap().contains(&a));
    // composed: [the base's raw A, B, D]; C is a ticked removal, so it is omitted
    send(&mut m, &s, &mut banks, sent_with(4, &[QA, QB, QNEW]));
    let payload = put_payload(&puts(&s).last().expect("the send went out"));
    assert_eq!(question_stems(&payload), ["A", "B", "D"], "{payload}");
    let qs = payload.pointer("/bank/questions").and_then(Value::as_array).unwrap();
    assert_eq!(qs[0]["id"], json!(QA), "the base's raw question, its own circle id");
    assert_eq!(qid_of(&qs[0]), Some(QA), "and the block that keeps it unchanged");
    assert_eq!(qid_of(&qs[2]), None, "the added question is new to the circle");

    let link = m.edit.link_for_local(LOCAL).unwrap();
    assert_eq!(link.base_version, 4, "the link is at the version the composed payload made");
    assert_eq!(link.held_back, vec![a.clone()], "the unticked change is kept for the next summary");
    let count = changes_since(&link, &banks[0]);
    assert_eq!((count.changed, count.added, count.removed), (1, 0, 0), "the unticked change stays unsent: {count:?}");
    assert_eq!(link.qid_of(&d), Some(QNEW), "the ticked addition is in step");
    assert!(link.base.iter().any(|b| b.local_id == d && b.snap.stem == "D"), "the added question's base is what was sent");
    assert!(link.base.iter().any(|b| b.local_id == a && b.snap.stem == "A"), "the unticked change's base is the base's raw question");
    assert_eq!(link.qid_of(&x), None, "the ticked removal's pair is dropped");
}

// ---- removals: unticked restored with its pair, ticked omitted ---------------------------------------------------------

#[test]
fn d3_an_unticked_removal_is_put_back_with_its_pair_and_a_ticked_one_is_omitted() {
    for untick in [true, false] {
        let (mut m, s, mut banks) = open_page(
            &|b| {
                b.questions[0].stem = "A (changed)".into();
                b.questions.pop();
            },
            &[],
        );
        let link = m.edit.link_for_local(LOCAL).unwrap();
        let x = local(&link, QC);
        if untick {
            m.edit.toggle_change(&x, false);
        }
        let qids: &[&str] = if untick { &[QA, QB, QC] } else { &[QA, QB] };
        send(&mut m, &s, &mut banks, sent_with(4, qids));
        let payload = put_payload(&puts(&s).last().expect("the send went out"));
        if untick {
            assert_eq!(question_stems(&payload), ["A (changed)", "B", "C"], "the removal is put back: {payload}");
            let qs = payload.pointer("/bank/questions").and_then(Value::as_array).unwrap();
            assert_eq!(qs[2]["id"], json!(QC), "as the base has it");
            assert_eq!(qid_of(&qs[2]), Some(QC), "and keeps its identity");
        } else {
            assert_eq!(question_stems(&payload), ["A (changed)", "B"], "the ticked removal is omitted: {payload}");
        }
        let link = m.edit.link_for_local(LOCAL).unwrap();
        assert_eq!(link.base_version, 4);
        if untick {
            assert_eq!(link.qid_of(&x), Some(QC), "the restored removal keeps its pair");
            assert_eq!(link.held_back, vec![x.clone()], "and is unticked for the next summary");
            assert_eq!(changes_since(&link, &banks[0]).removed, 1, "still a removal here");
        } else {
            assert_eq!(link.qid_of(&x), None, "the sent removal's pair is dropped");
            assert!(link.held_back.is_empty());
            assert_eq!(changes_since(&link, &banks[0]).removed, 0);
        }
    }
}

// ---- pictures: a question sent as the base has it keeps the base's picture ---------------------------------------------

/// (plan section 6, "Pictures") The plain line when a picture this send needs can not be fetched.
const BASE_PICTURE_NOT_FETCHED: &str =
    "A picture this send needs could not be fetched from the circle, so nothing was sent. Try again.";

/// The stage the page shows after a send attempt.
fn send_stage(m: &CirclesModel) -> &SendStage {
    match &m.edit.view {
        EditView::Send(v) => &v.stage,
        other => panic!("not the send page: {other:?}"),
    }
}

#[test]
fn d3_an_unticked_changes_base_picture_missing_here_is_fetched_and_stored_before_the_send() {
    // base [A with a picture, B, C]; this bank changes A and the person unticks it: it goes as the base has it,
    // picture included, and that file is not on this computer.
    let pic = b"the base's picture".to_vec();
    let sha = sha256_hex(&pic);
    let file = format!("media/{sha}.webp");
    let base = vec![qfull_pic(QA, "A", &file), qfull(QB, "B"), qfull(QC, "C")];
    let images = Arc::new(Pics::default());
    let media = Arc::new(MediaOnDisk::default());
    let (mut m, s, mut banks) = open_page_in(base, &|b| {
        b.questions[0].stem = "A (changed)".into();
        b.questions[1].stem = "B (changed)".into();
    }, &[], model_with(images.clone(), media));
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let a = local(&link, QA);
    m.edit.toggle_change(&a, false);
    s.on_raw("Get", &blob_path(&sha), pic.clone());
    s.on("Put", &format!("/blobs/{sha}"), 200, json!({"sha256": sha, "size": pic.len(), "width": 4, "height": 3}));
    send(&mut m, &s, &mut banks, sent_with(4, &[QA, QB, QC]));
    // The base body is still read once; the picture adds one blob GET, not another version read.
    assert_eq!(s.paths().iter().filter(|p| **p == format!("Get {}", version_path(3))).count(), 1, "{:?}", s.paths());
    assert_eq!(s.paths().iter().filter(|p| **p == format!("Get {}", blob_path(&sha))).count(), 1, "one blob GET: {:?}", s.paths());
    assert_eq!(*images.0.lock().unwrap(), vec![pic], "the fetched bytes go through the send's image sink");
    let payload = put_payload(&puts(&s).last().expect("the send went out"));
    let qs = payload.pointer("/bank/questions").and_then(Value::as_array).unwrap();
    assert_eq!(qs[0]["id"], json!(QA), "the base's raw question");
    assert_eq!(qs[0]["image"]["file"], json!(file), "with the base's own picture");
    assert!(matches!(send_stage(&m), SendStage::Done(_)), "the send went: {:?}", send_stage(&m));
}

#[test]
fn d3_a_restored_removals_picture_is_fetched_too() {
    // base [A, B, C with a picture]; this bank removed C and the person unticks the removal: C goes back as the base
    // has it, picture included, and that file is not on this computer.
    let pic = b"the removed question's picture".to_vec();
    let sha = sha256_hex(&pic);
    let file = format!("media/{sha}.webp");
    let base = vec![qfull(QA, "A"), qfull(QB, "B"), qfull_pic(QC, "C", &file)];
    let images = Arc::new(Pics::default());
    let media = Arc::new(MediaOnDisk::default());
    let (mut m, s, mut banks) = open_page_in(base, &|b| {
        b.questions[0].stem = "A (changed)".into();
        b.questions.pop();
    }, &[], model_with(images.clone(), media));
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let x = local(&link, QC);
    m.edit.toggle_change(&x, false);
    s.on_raw("Get", &blob_path(&sha), pic.clone());
    s.on("Put", &format!("/blobs/{sha}"), 200, json!({"sha256": sha, "size": pic.len(), "width": 4, "height": 3}));
    send(&mut m, &s, &mut banks, sent_with(4, &[QA, QB, QC]));
    assert_eq!(s.paths().iter().filter(|p| **p == format!("Get {}", blob_path(&sha))).count(), 1, "{:?}", s.paths());
    assert_eq!(*images.0.lock().unwrap(), vec![pic], "the restored question's picture is stored");
    let payload = put_payload(&puts(&s).last().expect("the send went out"));
    let qs = payload.pointer("/bank/questions").and_then(Value::as_array).unwrap();
    assert_eq!(qs[2]["id"], json!(QC), "the removal is put back as the base has it");
    assert_eq!(qs[2]["image"]["file"], json!(file), "with its picture");
    assert!(matches!(send_stage(&m), SendStage::Done(_)), "the send went: {:?}", send_stage(&m));
}

#[test]
fn d3_a_blob_404_or_a_hash_mismatch_fetches_nothing_sends_nothing_and_says_the_plain_line() {
    for mismatch in [false, true] {
        let pic = b"the base's picture".to_vec();
        let sha = sha256_hex(&pic);
        let file = format!("media/{sha}.webp");
        let base = vec![qfull_pic(QA, "A", &file), qfull(QB, "B"), qfull(QC, "C")];
        let images = Arc::new(Pics::default());
        let media = Arc::new(MediaOnDisk::default());
        let (mut m, s, mut banks) = open_page_in(base, &|b| {
            b.questions[0].stem = "A (changed)".into();
            b.questions[1].stem = "B (changed)".into();
        }, &[], model_with(images.clone(), media));
        let link = m.edit.link_for_local(LOCAL).unwrap();
        let a = local(&link, QA);
        m.edit.toggle_change(&a, false);
        if mismatch {
            // 200 with bytes that do not hash to the name asked for: `circle_blob` refuses it.
            s.on_raw("Get", &blob_path(&sha), b"some other picture".to_vec());
        } else {
            s.on("Get", &blob_path(&sha), 404, json!({"error": "not_found"}));
        }
        send(&mut m, &s, &mut banks, sent_with(4, &[QA, QB, QC]));
        assert!(puts(&s).is_empty(), "nothing is uploaded or sent: {:?}", s.paths());
        assert!(images.0.lock().unwrap().is_empty(), "the bytes are never stored, whatever came back");
        match send_stage(&m) {
            SendStage::Failed(p) => assert_eq!(p.message, BASE_PICTURE_NOT_FETCHED, "mismatch: {mismatch}"),
            other => panic!("not the plain line (mismatch: {mismatch}): {other:?}"),
        }
    }
}

#[test]
fn d3_a_base_picture_already_here_is_not_fetched() {
    let pic = b"the base's picture".to_vec();
    let sha = sha256_hex(&pic);
    let file = format!("media/{sha}.webp");
    let base = vec![qfull_pic(QA, "A", &file), qfull(QB, "B"), qfull(QC, "C")];
    let images = Arc::new(Pics::default());
    let media = Arc::new(MediaOnDisk::default());
    media.put(&file, &pic);
    let (mut m, s, mut banks) = open_page_in(base, &|b| {
        b.questions[0].stem = "A (changed)".into();
        b.questions[1].stem = "B (changed)".into();
    }, &[], model_with(images.clone(), media));
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let a = local(&link, QA);
    m.edit.toggle_change(&a, false);
    // No blob GET is scripted: a fetch here would panic in the scripted session.
    s.on("Put", &format!("/blobs/{sha}"), 200, json!({"sha256": sha, "size": pic.len(), "width": 4, "height": 3}));
    send(&mut m, &s, &mut banks, sent_with(4, &[QA, QB, QC]));
    assert!(!s.paths().iter().any(|p| p.contains("/blobs/") && p.starts_with("Get ")), "no blob GET: {:?}", s.paths());
    assert!(images.0.lock().unwrap().is_empty(), "nothing is fetched or re-stored");
    assert!(matches!(send_stage(&m), SendStage::Done(_)), "{:?}", send_stage(&m));
}

#[test]
fn d3_a_whole_bank_send_does_not_fetch_a_picture() {
    // The base can not be had, so the whole bank goes: the picture comes from this computer, and no blob is fetched.
    let pic = b"the base's picture".to_vec();
    let sha = sha256_hex(&pic);
    let file = format!("media/{sha}.webp");
    let base = vec![qfull_pic(QA, "A", &file), qfull(QB, "B"), qfull(QC, "C")];
    let images = Arc::new(Pics::default());
    let media = Arc::new(MediaOnDisk::default());
    media.put(&file, &pic);
    let (mut m, s, mut banks) = open_page_base_error_in(base, 404, json!({"error": "version_not_retained"}), &|b| b.questions[0].stem = "A (changed)".into(), model_with(images.clone(), media));
    assert!(matches!(m.edit.partial_send(), Some(PartialSend::Unavailable(_))), "the whole bank: {:?}", m.edit.partial_send());
    // No blob GET is scripted: a fetch here would panic in the scripted session.
    s.on("Put", &format!("/blobs/{sha}"), 200, json!({"sha256": sha, "size": pic.len(), "width": 4, "height": 3}));
    send(&mut m, &s, &mut banks, sent_with(4, &[QA, QB, QC]));
    assert!(!s.paths().iter().any(|p| p.contains("/blobs/") && p.starts_with("Get ")), "no blob GET: {:?}", s.paths());
    let payload = put_payload(&puts(&s).last().expect("the send went out"));
    assert_eq!(question_stems(&payload), ["A (changed)", "B", "C"], "the whole bank, as before: {payload}");
    assert!(matches!(send_stage(&m), SendStage::Done(_)), "{:?}", send_stage(&m));
}

#[test]
fn d3_a_missing_picture_on_our_own_question_keeps_the_missing_media_refusal_and_fetches_nothing() {
    // The added question is the person's own (ticked): its picture is not fetched from the circle. Today's refusal
    // still says so, and nothing is sent.
    let pic = b"our own picture".to_vec();
    let sha = sha256_hex(&pic);
    let file = format!("media/{sha}.webp");
    let images = Arc::new(Pics::default());
    let media = Arc::new(MediaOnDisk::default());
    let (mut m, s, mut banks) = open_page_in(base_qs(), &|b| {
        let mut q = added_question();
        q.image = Some(ImageRef { file: file.clone(), width: 4, height: 3 });
        b.questions.push(q);
    }, &[], model_with(images.clone(), media));
    // No blob GET is scripted: a fetch here would panic in the scripted session.
    send(&mut m, &s, &mut banks, sent_with(4, &[QA, QB, QC, QNEW]));
    assert!(!s.paths().iter().any(|p| p.contains("/blobs/") && p.starts_with("Get ")), "no blob GET: {:?}", s.paths());
    assert!(puts(&s).is_empty(), "nothing is sent: {:?}", s.paths());
    let want = LocalProblem::MissingMedia { file }.message();
    match send_stage(&m) {
        SendStage::Failed(p) => assert_eq!(p.message, want),
        other => panic!("not the missing-picture refusal: {other:?}"),
    }
}

#[test]
fn d3_two_base_questions_sharing_one_picture_make_one_blob_get() {
    // base [A with a picture, B, C with the same picture]; A and C are changed and unticked, B is ticked. The one
    // file is fetched once, though two base raw questions name it.
    let pic = b"one picture, two questions".to_vec();
    let sha = sha256_hex(&pic);
    let file = format!("media/{sha}.webp");
    let base = vec![qfull_pic(QA, "A", &file), qfull(QB, "B"), qfull_pic(QC, "C", &file)];
    let images = Arc::new(Pics::default());
    let media = Arc::new(MediaOnDisk::default());
    let (mut m, s, mut banks) = open_page_in(base, &|b| {
        b.questions[0].stem = "A (changed)".into();
        b.questions[1].stem = "B (changed)".into();
        b.questions[2].stem = "C (changed)".into();
    }, &[], model_with(images.clone(), media));
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let (a, c) = (local(&link, QA), local(&link, QC));
    m.edit.toggle_change(&a, false);
    m.edit.toggle_change(&c, false);
    s.on_raw("Get", &blob_path(&sha), pic.clone());
    s.on("Put", &format!("/blobs/{sha}"), 200, json!({"sha256": sha, "size": pic.len(), "width": 4, "height": 3}));
    send(&mut m, &s, &mut banks, sent_with(4, &[QA, QB, QC]));
    assert_eq!(s.paths().iter().filter(|p| **p == format!("Get {}", blob_path(&sha))).count(), 1, "one GET for one file: {:?}", s.paths());
    let payload = put_payload(&puts(&s).last().expect("the send went out"));
    let qs = payload.pointer("/bank/questions").and_then(Value::as_array).unwrap();
    assert_eq!(qs[0]["image"]["file"], json!(file));
    assert_eq!(qs[2]["image"]["file"], json!(file));
    assert!(matches!(send_stage(&m), SendStage::Done(_)), "{:?}", send_stage(&m));
}

#[test]
fn d3_a_base_picture_named_png_is_sent_under_its_own_name() {
    // The base names a PNG (an imported picture); the store would keep a fetched copy as WebP under another name. The
    // base raw question must keep the base's name, or the server would hash it as changed.
    let pic = b"an imported picture".to_vec();
    let sha = sha256_hex(&pic);
    let file = format!("media/{sha}.png");
    let base = vec![qfull_pic(QA, "A", &file), qfull(QB, "B"), qfull(QC, "C")];
    let images = Arc::new(Pics::default());
    let media = Arc::new(MediaOnDisk::default());
    let (mut m, s, mut banks) = open_page_in(base, &|b| {
        b.questions[0].stem = "A (changed)".into();
        b.questions[1].stem = "B (changed)".into();
    }, &[], model_with(images.clone(), media.clone()));
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let a = local(&link, QA);
    m.edit.toggle_change(&a, false);
    s.on_raw("Get", &blob_path(&sha), pic.clone());
    s.on("Put", &format!("/blobs/{sha}"), 200, json!({"sha256": sha, "size": pic.len(), "width": 4, "height": 3}));
    send(&mut m, &s, &mut banks, sent_with(4, &[QA, QB, QC]));
    assert_eq!(s.paths().iter().filter(|p| **p == format!("Get {}", blob_path(&sha))).count(), 1, "{:?}", s.paths());
    let payload = put_payload(&puts(&s).last().expect("the send went out"));
    let qs = payload.pointer("/bank/questions").and_then(Value::as_array).unwrap();
    assert_eq!(qs[0]["image"]["file"], json!(file), "the base's PNG name, not the store's WebP name");
    assert!(puts(&s).iter().any(|p| p.path == format!("/blobs/{sha}")), "the fetched PNG bytes go up: {:?}", s.paths());
    assert_eq!(media.read(&file).as_deref(), Some(pic.as_slice()), "the fetched original is kept under the payload's name, so the next send does not download it again");
}

#[test]
fn d3_a_signed_out_fetch_goes_to_the_sign_in_path_and_an_offline_one_to_the_plain_line() {
    for signed_out in [true, false] {
        let pic = b"the base's picture".to_vec();
        let sha = sha256_hex(&pic);
        let file = format!("media/{sha}.webp");
        let base = vec![qfull_pic(QA, "A", &file), qfull(QB, "B"), qfull(QC, "C")];
        let images = Arc::new(Pics::default());
        let media = Arc::new(MediaOnDisk::default());
        let (mut m, s, mut banks) = open_page_in(base, &|b| {
            b.questions[0].stem = "A (changed)".into();
            b.questions[1].stem = "B (changed)".into();
        }, &[], model_with(images.clone(), media));
        let link = m.edit.link_for_local(LOCAL).unwrap();
        let a = local(&link, QA);
        m.edit.toggle_change(&a, false);
        // The send's licence check (a POST) answers first; the blob GET then meets the failure.
        s.say(200, heads(3));
        if signed_out {
            s.fail(ApiError::SignedOut);
        } else {
            s.fail(ApiError::Offline("no network".into()));
        }
        send(&mut m, &s, &mut banks, sent_with(4, &[QA, QB, QC]));
        assert!(puts(&s).is_empty(), "nothing is uploaded or sent: {:?}", s.paths());
        assert!(images.0.lock().unwrap().is_empty(), "nothing is stored");
        if signed_out {
            assert!(m.edit.take_signed_out(), "the sign-in path, as for any signed-out answer");
            assert!(matches!(send_stage(&m), SendStage::Editing), "back to the note, not a picture refusal: {:?}", send_stage(&m));
        } else {
            match send_stage(&m) {
                SendStage::Failed(p) => {
                    assert_eq!(p.message, BASE_PICTURE_NOT_FETCHED);
                    assert!(p.retry, "the line says Try again, and that control is drawn");
                }
                other => panic!("not the plain line: {other:?}"),
            }
        }
    }
}

#[test]
fn d3_a_picture_the_circle_will_not_serve_says_why_and_offers_a_way_forward() {
    // (product critic 13 minor 2) A 404 from the blob route will not change by trying again: it may mean the picture
    // is no longer kept, or that the person may no longer read it. Say so, name the picture, and point at Get latest
    // or removing it, instead of an endless "Try again".
    let pic = b"the base's picture".to_vec();
    let sha = sha256_hex(&pic);
    let file = format!("media/{sha}.webp");
    let base = vec![qfull_pic(QA, "A", &file), qfull(QB, "B"), qfull(QC, "C")];
    let images = Arc::new(Pics::default());
    let media = Arc::new(MediaOnDisk::default());
    let (mut m, s, mut banks) = open_page_in(base, &|b| {
        b.questions[0].stem = "A (changed)".into();
        b.questions[1].stem = "B (changed)".into();
    }, &[], model_with(images.clone(), media));
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let a = local(&link, QA);
    m.edit.toggle_change(&a, false);
    s.say(200, heads(3));
    s.fail(ApiError::Http { status: 404, code: "not_found".into(), field: None, reason: None, retry_after: None });
    send(&mut m, &s, &mut banks, sent_with(4, &[QA, QB, QC]));
    assert!(puts(&s).is_empty(), "nothing is uploaded or sent: {:?}", s.paths());
    match send_stage(&m) {
        SendStage::Failed(p) => {
            assert!(!p.retry, "a 404 is not transient: no Try again");
            assert!(p.message.contains("no longer be kept"), "{}", p.message);
            assert!(p.message.contains(&file), "names the picture: {}", p.message);
        }
        other => panic!("not the 404 line: {other:?}"),
    }
}

#[test]
fn d3_ui_a_failed_picture_fetch_draws_try_again_and_pressing_it_fetches_and_sends() {
    let pic = b"the base's picture".to_vec();
    let sha = sha256_hex(&pic);
    let file = format!("media/{sha}.webp");
    let base = vec![qfull_pic(QA, "A", &file), qfull(QB, "B"), qfull(QC, "C")];
    let images = Arc::new(Pics::default());
    let media = Arc::new(MediaOnDisk::default());
    let (mut m, s, mut banks) = open_page_in(base, &|b| {
        b.questions[0].stem = "A (changed)".into();
        b.questions[1].stem = "B (changed)".into();
    }, &[], model_with(images.clone(), media));
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let a = local(&link, QA);
    m.edit.toggle_change(&a, false);
    s.on("Get", &blob_path(&sha), 404, json!({"error": "not_found"}));
    send(&mut m, &s, &mut banks, sent_with(4, &[QA, QB, QC]));
    match send_stage(&m) {
        SendStage::Failed(p) => assert!(p.retry, "the Failed page offers to try again"),
        other => panic!("not the plain line: {other:?}"),
    }
    let shapes = page(1280.0, &mut m);
    assert!(has(&shapes, "Try again"), "the control the line names is drawn: {:?}", texts(&shapes));
    // The second attempt: the blob now answers, then the picture upload and the send.
    s.on_raw("Get", &blob_path(&sha), pic.clone());
    s.on("Put", &format!("/blobs/{sha}"), 200, json!({"sha256": sha, "size": pic.len(), "width": 4, "height": 3}));
    assert!(click_text(&ctx(), 1280.0, "Try again", 0, &mut |ui| super::circle_edit_ui::draw_page(ui, &mut m)), "Try again is clickable");
    m.edit.sync(&mut banks);
    m.edit.frame(now(), &ready(&s));
    assert_eq!(s.paths().iter().filter(|p| **p == format!("Get {}", blob_path(&sha))).count(), 2, "the retry fetches again: {:?}", s.paths());
    assert!(matches!(send_stage(&m), SendStage::Done(_)), "and the send goes: {:?}", send_stage(&m));
}

// ---- the base can not be had: the whole bank, said plainly ---------------------------------------------------------------

#[test]
fn d3_a_base_no_longer_kept_or_withheld_sends_the_whole_bank_and_the_plain_line_is_there() {
    for body in [json!({"error": "version_not_retained"}), json!({"error": "version_withheld"})] {
        let (mut m, s, mut banks) = open_page_base_error(404, body.clone(), &|b| b.questions[0].stem = "A (changed)".into());
        let shown = match m.edit.partial_send().unwrap() {
            PartialSend::Unavailable(why) => why.clone(),
            other => panic!("{body}: {other:?}"),
        };
        assert_eq!(shown, PARTIAL_UNAVAILABLE, "the plan's plain line: {body}");
        assert!(shown.contains("needs the version your copy is based on") && shown.contains("no longer keeps it"), "{shown}");
        assert!(m.edit.send_blocker().is_none(), "nothing is disabled: the whole bank is sent");
        let link = m.edit.link_for_local(LOCAL).unwrap();
        let bank = banks[0].clone();
        let expected: Value = serde_json::from_slice(&payload_of(&link, &bank).unwrap()).unwrap();
        send(&mut m, &s, &mut banks, sent_with(4, &[QA]));
        let payload = put_payload(&puts(&s).last().expect("the send went out"));
        assert_eq!(payload, expected, "the whole bank, as before: {body}");
        assert!(matches!(&m.edit.view, EditView::Send(v) if matches!(v.stage, SendStage::Done(_))), "{body}");
        assert_eq!(m.edit.link_for_local(LOCAL).unwrap().base_version, 4);
    }
}

// ---- nothing ticked ------------------------------------------------------------------------------------------------------

#[test]
fn d3_nothing_ticked_refuses_with_tick_at_least_one_change_and_sends_nothing() {
    let (mut m, s, mut banks) = open_page(&|b| b.questions[0].stem = "A (changed)".into(), &[]);
    m.edit.tick_all(false);
    assert!(m.edit.send_ticked().unwrap().is_empty());
    assert!(m.edit.send_blocker().is_none(), "the button is not disabled for having no tick yet");
    m.edit.send();
    let stage = match &m.edit.view {
        EditView::Send(v) => &v.stage,
        other => panic!("not on the send page: {other:?}"),
    };
    assert!(matches!(stage, SendStage::Refused { message, merge_again: false } if message == TICK_ONE), "{stage:?}");
    m.edit.sync(&mut banks);
    m.edit.frame(now(), &ready(&s));
    assert!(puts(&s).is_empty(), "nothing was sent");
    assert!(matches!(m.edit.partial_send(), Some(PartialSend::Ready { ticked, .. }) if ticked.is_empty()), "the ticks stay as they were");
}

// ---- an edit while the send was in flight ---------------------------------------------------------------------------------

#[test]
fn d3_an_edit_made_while_the_send_was_in_flight_is_not_stale_and_shows_as_unsent_after_the_answer() {
    let base = base_qs();
    let (mut bank, mut link) = linked_from(base.clone(), false);
    link.base_version = 3;
    link.head_seen = 3;
    link.role_seen = Some("owner".into());
    link.author_known = true;
    link.is_author = true;
    bank.questions[0].stem = "A (changed)".into();
    bank.modified += 5;
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, heads(3));
    s.keep_raw("Get", &version_path(3), served_qs(3, base, false));
    let mut banks = vec![bank];
    let mut m = model();
    m.edit.add_link(link).unwrap();
    m.edit.frame(now(), &ready(&s));
    m.edit.sync(&mut banks);
    // the base read runs on its own thread
    background(&mut m);
    m.edit.frame(now(), &ready(&s));
    assert!(m.edit.open_send(LOCAL, owner()));
    let mut reads = 0;
    for _ in 0..500 {
        m.edit.frame(now(), &ready(&s));
        m.edit.sync(&mut banks);
        reads = s.paths().iter().filter(|p| **p == format!("Get {}", version_path(3))).count();
        if matches!(m.edit.partial_send(), Some(PartialSend::Ready { .. })) && matches!(&m.edit.view, EditView::Send(v) if matches!(v.licence, LicenceState::Known(_))) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(matches!(m.edit.partial_send(), Some(PartialSend::Ready { .. })), "the base read answered: {:?}", s.paths());
    assert_eq!(reads, 1, "one read per page: {:?}", s.paths());
    m.edit.set_attest(true);
    s.on("Put", &super::bank_links_fix8_tests::put_path(), 201, sent_with(4, &[QA]));
    // the send is held at the server (its first request, the licence check, stops there)
    let n = s.paths().len();
    let release = s.hold();
    m.edit.send();
    m.edit.sync(&mut banks);
    for _ in 0..500 {
        if s.paths().len() > n {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(s.paths().len() > n, "the send went out: {:?}", s.paths());
    // she edits the same question again while it is on its way
    banks[0].questions[0].stem = "A (changed again)".into();
    banks[0].modified += 5;
    release.send(()).unwrap();
    for _ in 0..500 {
        m.edit.frame(now(), &ready(&s));
        m.edit.sync(&mut banks);
        if matches!(&m.edit.view, EditView::Send(v) if matches!(v.stage, SendStage::Done(_))) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let link = m.edit.link_for_local(LOCAL).unwrap();
    assert!(!link.stale_for(&banks[0]), "newer than the link: never put back");
    assert!(!m.edit.status_of(&banks[0]).unwrap().stale);
    let count = changes_since(&link, &banks[0]);
    assert_eq!(count.changed, 1, "the edit made in flight is unsent: {count:?}");
    assert_eq!(link.base_version, 4);
}

// ---- the whole-bank path unchanged -----------------------------------------------------------------------------------------

#[test]
fn d3_the_whole_bank_path_is_unchanged_when_the_base_can_not_be_had() {
    // The same shape as the existing send tests: one change, no base body read scripted (the circle no longer
    // keeps it). The payload is the bank exactly as `payload_of` makes it today.
    let (mut m, s, mut banks) = open_page_base_error(404, json!({"error": "version_not_retained"}), &|b| b.questions[0].stem = "A (changed)".into());
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let bank = banks[0].clone();
    let expected: Value = serde_json::from_slice(&payload_of(&link, &bank).unwrap()).unwrap();
    send(&mut m, &s, &mut banks, sent_with(4, &[QA]));
    let payload = put_payload(&puts(&s).last().expect("the send went out"));
    assert_eq!(payload, expected);
}

// ---- the summary is a snapshot: a bank changed since is listed again, nothing is sent yet -------------------------------

#[test]
fn d3_a_bank_changed_after_the_body_arrived_is_listed_again_and_nothing_is_sent() {
    let (mut m, s, mut banks) = open_page(&|b| b.questions[0].stem = "A (changed)".into(), &[]);
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let (a, b) = (local(&link, QA), local(&link, QB));
    // A question that was unchanged when the body was read is edited after it.
    banks[0].questions[1].stem = "B (changed later)".into();
    banks[0].modified += 5;
    m.edit.send();
    m.edit.sync(&mut banks);
    assert!(puts(&s).is_empty(), "nothing is sent while the list is out of date: {:?}", s.paths());
    let (stage, notice) = match &m.edit.view {
        EditView::Send(v) => (format!("{:?}", v.stage), v.notice.clone()),
        other => panic!("not on the send page: {other:?}"),
    };
    assert!(stage.contains("Editing"), "{stage}");
    assert_eq!(notice.as_deref(), Some(SEND_LIST_AGAIN));
    let c = changes(&m);
    assert!(c.changed.contains(&a) && c.changed.contains(&b), "the list is worked out again: {c:?}");
    let t = ticked(&m);
    assert!(t.contains(&a) && t.contains(&b), "a change not seen yet starts ticked: {t:?}");
    // The next press goes, with the new change.
    send(&mut m, &s, &mut banks, sent_with(4, &[QA, QB, QC]));
    let payload = put_payload(&puts(&s).last().expect("the send went out"));
    assert_eq!(question_stems(&payload), ["A (changed)", "B (changed later)", "C"]);
}

#[test]
fn d3_deleting_a_ticked_changed_question_makes_it_a_removal_whose_tick_decides() {
    for keep_tick in [true, false] {
        // A is changed (ticked, then deleted while the page is open); B is changed and stays ticked, so the send
        // still has something to send either way.
        let (mut m, s, mut banks) = open_page(
            &|b| {
                b.questions[0].stem = "A (changed)".into();
                b.questions[1].stem = "B (changed)".into();
            },
            &[],
        );
        let link = m.edit.link_for_local(LOCAL).unwrap();
        let a = local(&link, QA);
        banks[0].questions.retain(|q| q.id != a);
        banks[0].modified += 5;
        m.edit.send();
        m.edit.sync(&mut banks);
        assert!(puts(&s).is_empty(), "nothing is sent while the list is out of date: {keep_tick} {:?}", s.paths());
        let c = changes(&m);
        assert!(c.removed.contains(&a) && !c.changed.contains(&a), "the deleted change is a removal now: {c:?}");
        assert!(ticked(&m).contains(&a), "the tick carries by local id: {:?}", ticked(&m));
        if !keep_tick {
            m.edit.toggle_change(&a, false);
            assert!(!ticked(&m).contains(&a));
        }
        let qids: &[&str] = if keep_tick { &[QB, QC] } else { &[QA, QB, QC] };
        send(&mut m, &s, &mut banks, sent_with(4, qids));
        let payload = put_payload(&puts(&s).last().expect("the send went out"));
        if keep_tick {
            assert_eq!(question_stems(&payload), ["B (changed)", "C"], "a ticked removal goes: {payload}");
        } else {
            assert_eq!(question_stems(&payload), ["A", "B (changed)", "C"], "an unticked removal is put back: {payload}");
        }
    }
}

// ---- a 409 then Get latest keeps the ticks by local id --------------------------------------------------------------------

#[test]
fn d3_after_a_conflict_and_get_latest_an_unticked_change_is_still_unticked() {
    // A is unticked; D is a new question and stays ticked, so the send has something to send.
    let (mut m, s, mut banks) = open_page(
        &|b| {
            b.questions[0].stem = "A (changed)".into();
            b.questions.push(added_question());
        },
        &[],
    );
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let (a, d) = (local(&link, QA), banks[0].questions[3].id.clone());
    m.edit.toggle_change(&a, false);
    s.on("Put", &super::bank_links_fix8_tests::put_path(), 409, json!({"error": "conflict", "head_version": 4, "head_at": super::circles_tests::NOW - 1000, "head_by": "Bob Teacher"}));
    m.edit.send();
    m.edit.sync(&mut banks);
    m.edit.frame(now(), &ready(&s));
    assert!(matches!(&m.edit.view, EditView::Send(v) if matches!(v.stage, SendStage::Conflict(_))), "{:?}", m.edit.view);
    // Get latest: Bob changed A; she keeps hers by default, so A is still an unsent change.
    s.on("Post", &format!("{}/merge", super::bank_links_fix8_tests::put_path()), 200, super::bank_links_fix8_tests::merge_start(3, 4));
    s.keep_raw("Get", &super::bank_links_fix8_tests::put_path(), served_qs(4, vec![qfull(QA, "A (Bob)"), qfull(QB, "B"), qfull(QC, "C")], false));
    assert!(m.edit.open_merge(LOCAL, owner()));
    for _ in 0..500 {
        m.edit.frame(now(), &ready(&s));
        m.edit.sync(&mut banks);
        if matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::Review(_))) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::Review(_))), "{:?}", m.edit.view);
    let conflict = match &m.edit.view {
        EditView::Merge(v) => match &v.phase {
            MergePhase::Review(r) => r.plan.rows.iter().position(|x| x.kind == RowKind::Conflict),
            _ => None,
        },
        _ => None,
    };
    let conflict = conflict.expect("A was changed by both");
    m.edit.set_merge_choice(conflict, Choice::Ours);
    m.edit.apply_merge();
    for _ in 0..500 {
        m.edit.frame(now(), &ready(&s));
        m.edit.sync(&mut banks);
        if matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::Applied(_))) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::Applied(_))), "{:?}", m.edit.view);
    m.edit.cancel_merge();
    // The next Send page reads the new base body; the unticked mark is kept by local id.
    s.keep_raw("Get", &version_path(4), served_qs(4, vec![qfull(QA, "A (Bob)"), qfull(QB, "B"), qfull(QC, "C")], false));
    assert!(m.edit.open_send(LOCAL, owner()));
    wait_ready(&mut m, &s, &mut banks);
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let a = local(&link, QA);
    let d = banks[0].questions.iter().find(|q| link.qid_of(&q.id).is_none()).expect("the added question").id.clone();
    let c = changes(&m);
    assert!(c.changed.contains(&a) && c.added.contains(&d), "A is still unsent and D is still new: {c:?}");
    assert!(!m.edit.send_ticked().unwrap().contains(&a), "A is still unticked after the conflict and Get latest");
    assert!(m.edit.send_ticked().unwrap().contains(&d), "a change not seen before the conflict is ticked");
    let read4 = s.paths().iter().filter(|p| **p == format!("Get {}", version_path(4))).count();
    assert_eq!(read4, 1, "the rebuilt list reads the new base body once: {:?}", s.paths());
}

// ---- a transient base-read failure: the page waits, Try again reads, never the whole bank ---------------------------------

#[test]
fn d3_an_offline_or_limited_base_read_keeps_the_page_and_retry_reads_again() {
    for (status, body) in [(503, json!({"error": "unavailable"})), (429, json!({"error": "rate_limited"}))] {
        let (mut m, s, mut banks) = open_page_base_error(status, body.clone(), &|b| b.questions[0].stem = "A (changed)".into());
        let why = match m.edit.partial_send().unwrap() {
            PartialSend::ReadFailed(why) => why.clone(),
            other => panic!("{body}: {other:?}"),
        };
        assert_ne!(why, PARTIAL_UNAVAILABLE, "{body}");
        assert!(!why.contains("no longer keeps it"), "a read that may work is not said to be gone: {why}");
        assert_eq!(m.edit.send_blocker(), Some(PARTIAL_READ_FAILED), "{body}");
        m.edit.send();
        m.edit.sync(&mut banks);
        m.edit.frame(now(), &ready(&s));
        assert!(puts(&s).is_empty(), "no whole-bank send for a read that may pass: {status} {:?}", s.paths());
        // The circle answers the one read when it is asked again.
        s.keep_raw("Get", &version_path(3), served_qs(3, base_qs(), false));
        m.edit.retry_partial();
        wait_ready(&mut m, &s, &mut banks);
        let reads = s.paths().iter().filter(|p| **p == format!("Get {}", version_path(3))).count();
        assert_eq!(reads, 2, "Try again runs the one read again: {status} {:?}", s.paths());
    }
}

// ---- an answer that could not be read: reopening keeps the selection (and the commit id) --------------------------------

#[test]
fn d3_after_an_unread_answer_reopening_keeps_the_unticked_selection_and_the_commit_id() {
    for mode in ["timeout", "version 0"] {
        // A is unticked; D is new and ticked, so the send has something to send.
        let (mut m, s, mut banks) = open_page(
            &|b| {
                b.questions[0].stem = "A (changed)".into();
                b.questions.push(added_question());
            },
            &[],
        );
        let link = m.edit.link_for_local(LOCAL).unwrap();
        let (a, d) = (local(&link, QA), banks[0].questions[3].id.clone());
        m.edit.toggle_change(&a, false);
        // (the licence check's own answer is queued first, so the answer or failure lands on the send itself)
        s.on("Post", "/circles/heads", 200, heads(3));
        match mode {
            "timeout" => {
                s.fail(super::bank_links_fix8_tests::timed_out().unwrap());
            }
            _ => {
                s.on("Put", &super::bank_links_fix8_tests::put_path(), 201, super::bank_links_fix9_tests::version_zero());
            }
        }
        m.edit.send();
        m.edit.sync(&mut banks);
        m.edit.frame(now(), &ready(&s));
        let first = puts(&s).pop().expect("the send went out");
        let first: Value = serde_json::from_str(String::from_utf8(first.body).unwrap().split_once('\n').unwrap().0).unwrap();
        m.edit.close();
        // Reopening starts from the retry record: the same selection, and the same commit id once sent again.
        assert!(m.edit.open_send(LOCAL, owner()));
        wait_ready(&mut m, &s, &mut banks);
        assert!(!m.edit.send_ticked().unwrap().contains(&a), "{mode}: the unticked change is still unticked after reopening");
        assert!(m.edit.send_ticked().unwrap().contains(&d), "{mode}: and the other change is still ticked");
        m.edit.set_attest(true);
        send(&mut m, &s, &mut banks, sent_with(4, &[QA, QB, QC, QNEW]));
        let second = puts(&s).pop().expect("the send went out again");
        let second: Value = serde_json::from_str(String::from_utf8(second.body).unwrap().split_once('\n').unwrap().0).unwrap();
        assert_eq!(second["commit_id"], first["commit_id"], "{mode}: the same selection reuses the commit id: nothing is sent twice");
    }
}

// ---- refusal positions are the bank's question numbers --------------------------------------------------------------------

#[test]
fn d3_a_keyless_refusal_names_the_banks_question_not_the_payload_position() {
    // base order A, C, B; this bank A, B (C removed and unticked, so it goes back between A and B); B has no key.
    let base = vec![qfull(QA, "A"), qfull(QC, "C"), qfull(QB, "B")];
    let (mut m, _s, mut banks) = open_page_with_base(
        base,
        &|b| {
            b.questions.remove(1); // C
            b.questions[1].explanation = Some(crate::model::KEYLESS_SENTINEL.into()); // B
        },
        &[QC],
    );
    m.edit.send();
    m.edit.sync(&mut banks);
    let stage = match &m.edit.view {
        EditView::Send(v) => &v.stage,
        other => panic!("not on the send page: {other:?}"),
    };
    // The keyless question is payload position 2 (after the restored C), the bank's question 1.
    assert!(matches!(stage, SendStage::Keyless { positions, .. } if positions == &vec![1]), "{stage:?}");
}

// ---- send-on-top never composes against another base ----------------------------------------------------------------------

#[test]
fn d3_a_send_over_a_held_version_goes_as_the_whole_bank() {
    let (mut m, s, mut banks) = open_page(
        &|b| {
            b.questions[0].stem = "A (changed)".into();
            b.questions.push(added_question());
        },
        &[],
    );
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let d = banks[0].questions[3].id.clone();
    m.edit.toggle_change(&d, false);
    // A hidden newest version (what `send_on_top` sets): the whole bank goes, never a selection.
    if let EditView::Send(v) = &mut m.edit.view {
        v.base_override = Some(6);
    }
    let bank = banks[0].clone();
    let expected: Value = serde_json::from_slice(&payload_of(&link, &bank).unwrap()).unwrap();
    send(&mut m, &s, &mut banks, sent_with(4, &[QA, QB, QC, QNEW]));
    let payload = put_payload(&puts(&s).last().expect("the send went out"));
    assert_eq!(payload, expected, "the whole bank goes over the held version");
    assert_eq!(question_stems(&payload).len(), 4, "the unticked addition is still in the whole bank: {payload}");
}

// ---- the drawing: the summary and its tick boxes (plan 4.7 and 4.8) --------------------------------------------------------

/// The raw text, box and colour of each drawn text, `width` wide (the same helper the other page tests use).
fn drawn(width: f32, draw: &mut dyn FnMut(&mut egui::Ui)) -> Vec<(String, egui::Rect, egui::Color32)> {
    let ctx = ctx();
    let mut all = Vec::new();
    for _ in 0..2 {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 6000.0))), ..Default::default() };
        let out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| draw(ui));
        });
        all = out
            .shapes
            .iter()
            .filter_map(|s| if let egui::epaint::Shape::Text(t) = &s.shape { Some((t.galley.text().to_string(), t.galley.rect.translate(t.pos.to_vec2()), t.galley.job.sections.first().map_or(t.fallback_color, |x| x.format.color))) } else { None })
            .collect();
        out.drop_without_applying_deltas();
    }
    all
}

/// The Send page as the Circles tab draws it.
fn page(width: f32, m: &mut CirclesModel) -> Vec<(String, egui::Rect, egui::Color32)> {
    drawn(width, &mut |ui| super::circle_edit_ui::draw_page(ui, m))
}

/// Every drawn text with whether egui cut it. `galley.text()` is the full job text, so a cut is seen only through
/// `elided`.
fn elided(width: f32, draw: &mut dyn FnMut(&mut egui::Ui)) -> Vec<(String, bool)> {
    let ctx = ctx();
    let mut all = Vec::new();
    for _ in 0..2 {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 6000.0))), ..Default::default() };
        let out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| draw(ui));
        });
        all = out
            .shapes
            .iter()
            .filter_map(|s| if let egui::epaint::Shape::Text(t) = &s.shape { Some((t.galley.text().to_string(), t.galley.elided)) } else { None })
            .collect();
        out.drop_without_applying_deltas();
    }
    all
}

/// Every drawn text stays inside `width` (nothing spills out of the page).
fn within(shapes: &[(String, egui::Rect, egui::Color32)], width: f32) {
    for (t, r, _) in shapes {
        assert!(r.min.x >= -0.5 && r.max.x <= width + 0.5, "{width}: {t:?} leaves the page: {r:?}");
    }
}

fn texts(shapes: &[(String, egui::Rect, egui::Color32)]) -> Vec<&str> {
    shapes.iter().map(|(t, ..)| t.as_str()).collect()
}

fn has(shapes: &[(String, egui::Rect, egui::Color32)], want: &str) -> bool {
    shapes.iter().any(|(t, ..)| t == want)
}

/// A Send page whose base version body has arrived but whose summary is worked out by `sync`, which has the bank:
/// the page is still Reading.
fn reading_page() -> (CirclesModel, Arc<Script>, Vec<QuestionBank>) {
    let base = base_qs();
    let (mut bank, mut link) = linked_from(base.clone(), false);
    link.base_version = 3;
    link.head_seen = 3;
    link.role_seen = Some("owner".into());
    link.author_known = true;
    link.is_author = true;
    bank.questions[0].stem = "A (changed)".into();
    bank.modified += 5;
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, heads(3));
    s.keep_raw("Get", &version_path(3), served_qs(3, base, false));
    let mut banks = vec![bank];
    let mut m = model();
    m.edit.add_link(link).unwrap();
    m.edit.frame(now(), &ready(&s));
    m.edit.sync(&mut banks);
    assert!(m.edit.open_send(LOCAL, owner()));
    // the read's answer is drained; the summary waits for `sync`, which has the bank
    m.edit.frame(now(), &ready(&s));
    (m, s, banks)
}

#[test]
fn d3_ui_the_ready_page_lists_each_change_with_kind_number_stem_and_tick_state() {
    // base [A, B, C]; this bank [A', B, D] (A changed, B unchanged, C removed, D added), C held back (unticked)
    let (mut m, _s, _banks) = open_page(
        &|b| {
            b.questions[0].stem = "A (changed)".into();
            b.questions.pop();
            b.questions.push(added_question());
        },
        &[QC],
    );
    for width in [1280.0, 420.0] {
        let shapes = page(width, &mut m);
        for want in [
            "Send my changes to \"Cells\" (Algebra team). Based on version 3.",
            "Tick the changes to send. Unticked ones stay on this computer, still shown as unsent.",
            "Changed  Q1",
            "\"A (changed)\"",
            "Added  Q3",
            "\"D\"",
            "Removed  Q3",
            "\"C\"",
            "2 of 3 ticked.",
            "Tick all",
            "Untick all",
            "Send 2 changes",
            "Cancel",
            "A note for the history (optional)",
        ] {
            assert!(has(&shapes, want), "{width}: {want:?} is not drawn: {:?}", texts(&shapes));
        }
        let at = |want: &str| shapes.iter().find(|(t, ..)| t == want).map(|(_, r, _)| *r).unwrap_or_else(|| panic!("{width}: {want:?} is not drawn"));
        let (tag, stem) = (at("Changed  Q1"), at("\"A (changed)\""));
        if width > 520.0 {
            assert!((tag.center().y - stem.center().y).abs() < 2.0, "{width}: a row is one line: {tag:?} {stem:?}");
        } else {
            assert!(stem.top() >= tag.bottom() - 1.0, "{width}: the stem is on its own line under the tag: {tag:?} {stem:?}");
        }
    }
}

#[test]
fn d3_ui_clicking_a_rows_box_sends_toggle_change_and_the_count_follows() {
    let (mut m, _s, _banks) = open_page(
        &|b| {
            b.questions[0].stem = "A (changed)".into();
            b.questions.pop();
            b.questions.push(added_question());
        },
        &[],
    );
    let ctx = ctx();
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let a = local(&link, QA);
    assert_eq!(m.edit.send_ticked().unwrap().len(), 3, "every change starts ticked");
    assert!(click_text(&ctx, 1280.0, "Changed  Q1", 0, &mut |ui| super::circle_edit_ui::draw_page(ui, &mut m)), "the row's box is drawn with its tag");
    assert!(!m.edit.send_ticked().unwrap().contains(&a), "clicking the box calls toggle_change");
    let shapes = page(1280.0, &mut m);
    assert!(has(&shapes, "2 of 3 ticked.") && has(&shapes, "Send 2 changes"), "{:?}", texts(&shapes));
    assert!(click_text(&ctx, 1280.0, "Tick all", 0, &mut |ui| super::circle_edit_ui::draw_page(ui, &mut m)));
    assert_eq!(m.edit.send_ticked().unwrap().len(), 3, "Tick all ticks every change");
    assert!(has(&page(1280.0, &mut m), "3 of 3 ticked."));
    assert!(click_text(&ctx, 1280.0, "Untick all", 0, &mut |ui| super::circle_edit_ui::draw_page(ui, &mut m)));
    assert!(m.edit.send_ticked().unwrap().is_empty(), "Untick all ticks none");
    let shapes = page(1280.0, &mut m);
    for want in ["0 of 3 ticked.", TICK_ONE, "Send 0 changes"] {
        assert!(has(&shapes, want), "{want:?} is not drawn: {:?}", texts(&shapes));
    }
}

#[test]
fn d3_ui_with_nothing_ticked_the_send_is_disabled_and_nothing_goes() {
    let (mut m, s, mut banks) = open_page(&|b| b.questions[0].stem = "A (changed)".into(), &[]);
    m.edit.tick_all(false);
    let shapes = page(1280.0, &mut m);
    assert!(has(&shapes, TICK_ONE), "nothing ticked: the line is there: {:?}", texts(&shapes));
    assert!(has(&shapes, "Send 0 changes"), "{:?}", texts(&shapes));
    assert!(click_text(&ctx(), 1280.0, "Send 0 changes", 0, &mut |ui| super::circle_edit_ui::draw_page(ui, &mut m)));
    m.edit.sync(&mut banks);
    m.edit.frame(now(), &ready(&s));
    assert!(puts(&s).is_empty(), "a disabled Send sends nothing: {:?}", s.paths());
    assert!(matches!(&m.edit.view, EditView::Send(v) if matches!(v.stage, SendStage::Editing)), "the page stays: no refusal stage");
}

#[test]
fn d3_ui_while_the_base_is_read_the_page_says_so_and_send_waits() {
    let (mut m, s, mut banks) = reading_page();
    assert!(matches!(m.edit.partial_send(), Some(PartialSend::Reading)));
    let shapes = page(1280.0, &mut m);
    assert!(has(&shapes, PARTIAL_READING), "{:?}", texts(&shapes));
    assert!(!shapes.iter().any(|(t, ..)| t.ends_with(" changes")), "no tick list yet: {:?}", texts(&shapes));
    assert!(click_text(&ctx(), 1280.0, "Send", 0, &mut |ui| super::circle_edit_ui::draw_page(ui, &mut m)));
    m.edit.sync(&mut banks);
    m.edit.frame(now(), &ready(&s));
    assert!(puts(&s).is_empty(), "Send waits for the read: {:?}", s.paths());
}

#[test]
fn d3_ui_a_failed_base_read_keeps_the_page_says_so_and_try_again_reads_again() {
    let (mut m, s, mut banks) = open_page_base_error(503, json!({"error": "unavailable"}), &|b| b.questions[0].stem = "A (changed)".into());
    let why = match m.edit.partial_send() {
        Some(PartialSend::ReadFailed(why)) => why.clone(),
        other => panic!("{other:?}"),
    };
    let shapes = page(1280.0, &mut m);
    for want in [PARTIAL_READ_FAILED, "Try again", "A note for the history (optional)", "Your confirmation", "Cancel"] {
        assert!(has(&shapes, want), "{want:?} is not drawn: {:?}", texts(&shapes));
    }
    // (phase D3) The generic line, then the reason the read kept when it says more (the network, the limit, the session).
    assert!(why == PARTIAL_READ_FAILED || has(&shapes, &why), "the stored reason {why:?} is drawn: {:?}", texts(&shapes));
    // Send is there but disabled: pressing it sends nothing.
    assert!(click_text(&ctx(), 1280.0, "Send", 0, &mut |ui| super::circle_edit_ui::draw_page(ui, &mut m)));
    m.edit.sync(&mut banks);
    m.edit.frame(now(), &ready(&s));
    assert!(puts(&s).is_empty(), "a failed read never sends the whole bank: {:?}", s.paths());
    // the circle answers the one read when it is asked again
    s.on_raw("Get", &version_path(3), served_qs(3, base_qs(), false));
    assert!(click_text(&ctx(), 1280.0, "Try again", 0, &mut |ui| super::circle_edit_ui::draw_page(ui, &mut m)));
    m.edit.frame(now(), &ready(&s));
    m.edit.sync(&mut banks);
    assert!(matches!(m.edit.partial_send(), Some(PartialSend::Ready { .. })), "Try again reads again: {:?}", s.paths());
    assert_eq!(s.paths().iter().filter(|p| **p == format!("Get {}", version_path(3))).count(), 2, "one more read, not none: {:?}", s.paths());
}

#[test]
fn d3_ui_the_unavailable_page_says_so_and_send_everything_sends_the_whole_bank() {
    let (mut m, s, mut banks) = open_page_base_error(404, json!({"error": "version_not_retained"}), &|b| b.questions[0].stem = "A (changed)".into());
    let shapes = page(1280.0, &mut m);
    assert!(has(&shapes, PARTIAL_UNAVAILABLE) && has(&shapes, "Send everything"), "{:?}", texts(&shapes));
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let expected: Value = serde_json::from_slice(&payload_of(&link, &banks[0]).unwrap()).unwrap();
    s.on("Put", &super::bank_links_fix8_tests::put_path(), 201, sent_with(4, &[QA]));
    assert!(click_text(&ctx(), 1280.0, "Send everything", 0, &mut |ui| super::circle_edit_ui::draw_page(ui, &mut m)));
    m.edit.sync(&mut banks);
    m.edit.frame(now(), &ready(&s));
    let payload = put_payload(&puts(&s).last().expect("the whole bank went out"));
    assert_eq!(payload, expected, "the whole-bank path is unchanged");
}

#[test]
fn d3_ui_a_send_over_a_held_version_says_the_whole_bank_goes() {
    let (mut m, _s, _banks) = open_page(&|b| b.questions[0].stem = "A (changed)".into(), &[]);
    if let EditView::Send(v) = &mut m.edit.view {
        v.base_override = Some(6);
        v.partial = PartialSend::Unavailable(SEND_ON_TOP_WHOLE.into());
    }
    let shapes = page(1280.0, &mut m);
    assert!(has(&shapes, SEND_ON_TOP_WHOLE) && has(&shapes, "Send everything"), "{:?}", texts(&shapes));
    assert!(!has(&shapes, "Changed  Q1"), "no tick list for a whole-bank send: {:?}", texts(&shapes));
}

#[test]
fn d3_ui_the_list_again_notice_is_drawn_above_the_rows() {
    let (mut m, _s, _banks) = open_page(&|b| b.questions[0].stem = "A (changed)".into(), &[]);
    if let EditView::Send(v) = &mut m.edit.view {
        v.notice = Some(SEND_LIST_AGAIN.into());
    }
    let shapes = page(1280.0, &mut m);
    let at = |want: &str| shapes.iter().find(|(t, ..)| t == want).map(|(_, r, _)| *r).unwrap_or_else(|| panic!("{want:?} is not drawn: {:?}", texts(&shapes)));
    let notice = at(SEND_LIST_AGAIN);
    assert!(notice.top() < at("Tick the changes to send. Unticked ones stay on this computer, still shown as unsent.").top(), "above the instructions");
    assert!(notice.top() < at("Changed  Q1").top(), "above the rows");
}

#[test]
fn d3_ui_at_420_each_row_is_two_lines_in_order_and_nothing_leaves_the_page() {
    let long = "Which of these numbers is prime, and why is it prime rather than a product of two smaller numbers? ".repeat(6);
    let (mut m, _s, _banks) = open_page(
        &move |b| {
            b.questions[0].stem = long.clone();
            b.questions.pop();
            b.questions.push(added_question());
        },
        &[QC],
    );
    for width in [1280.0, 420.0] {
        let shapes = page(width, &mut m);
        within(&shapes, width);
        let stem = shapes.iter().find(|(t, ..)| t.starts_with("\"Which of these numbers")).map(|(t, r, _)| (t.clone(), *r)).unwrap_or_else(|| panic!("{width}: the long stem is not drawn: {:?}", texts(&shapes)));
        assert!(stem.0.starts_with("\"Which of these numbers"), "{width}: the stem is drawn in quotes");
        assert!(stem.1.height() < 24.0, "{width}: the stem is cut to one line: {:?}", stem.1);
        // (phase D3) The cut itself, through the galley's own `elided`: a long stem is elided, a short one is not.
        let cuts = elided(width, &mut |ui| super::circle_edit_ui::draw_page(ui, &mut m));
        let cut = |want: &str| cuts.iter().find(|(t, _)| t.starts_with(want)).map(|(_, e)| *e);
        assert_eq!(cut("\"Which of these numbers"), Some(true), "{width}: the long stem is elided: {cuts:?}");
        assert_eq!(cut("\"D\""), Some(false), "{width}: a short stem is not cut: {cuts:?}");
        let tag = shapes.iter().find(|(t, ..)| t == "Changed  Q1").map(|(_, r, _)| *r).expect("the changed question's tag");
        if width > 520.0 {
            assert!((tag.center().y - stem.1.center().y).abs() < 2.0, "a row is one line at 1280: {tag:?} {:?}", stem.1);
        } else {
            assert!(stem.1.top() >= tag.bottom() - 1.0, "the stem is under its tag at 420: {tag:?} {:?}", stem.1);
        }
    }
    // (plan 4.8) two lines a row at 420, the rows in the order the list has them: changed, added, removed
    let shapes = page(420.0, &mut m);
    let at = |want: &str| shapes.iter().find(|(t, ..)| t == want).map(|(_, r, _)| *r).unwrap_or_else(|| panic!("{want:?} is not drawn: {:?}", texts(&shapes)));
    let (a_tag, d_tag, c_tag) = (at("Changed  Q1"), at("Added  Q3"), at("Removed  Q3"));
    let a_stem = shapes.iter().find(|(t, ..)| t.starts_with("\"Which of these numbers")).map(|(_, r, _)| *r).expect("the changed row's stem");
    let (d_stem, c_stem) = (at("\"D\""), at("\"C\""));
    assert!(a_stem.top() >= a_tag.bottom() - 1.0, "changed: stem under tag");
    assert!(d_tag.top() >= a_stem.bottom() - 1.0, "added: after the changed row");
    assert!(d_stem.top() >= d_tag.bottom() - 1.0, "added: stem under tag");
    assert!(c_tag.top() >= d_stem.bottom() - 1.0, "removed: after the added row");
    assert!(c_stem.top() >= c_tag.bottom() - 1.0, "removed: stem under tag");
}

#[test]
fn d3_ui_a_copy_in_step_lists_no_rows_and_says_so() {
    let (mut m, _s, _banks) = open_page(&|_| {}, &[]);
    for width in [1280.0, 420.0] {
        let shapes = page(width, &mut m);
        let t = texts(&shapes);
        assert!(has(&shapes, "This copy is in step with the circle, so there is nothing to send. Nothing in your copy differs from the version it was made from."), "{width}: {t:?}");
        assert!(!t.iter().any(|t| t.contains("You can still send it")), "{width}: no promise beside a Send that is not offered: {t:?}");
        assert!(!has(&shapes, "Tick the changes to send. Unticked ones stay on this computer, still shown as unsent.") && !has(&shapes, TICK_ONE), "{width}: nothing to tick: {t:?}");
        assert!(!shapes.iter().any(|(t, ..)| t.starts_with("Changed")), "{width}: no rows: {t:?}");
        assert!(!has(&shapes, "Send") && !has(&shapes, "Send 0 changes"), "{width}: no Send button: {t:?}");
        assert!(!t.iter().any(|t| t.starts_with("This adds a new version")), "{width}: nothing is added: {t:?}");
        assert!(has(&shapes, "Cancel"), "{width}: {t:?}");
    }
}

#[test]
fn d3_ui_a_removal_whose_base_snapshot_is_not_kept_is_a_plain_removed_row() {
    // A bounded link keeps its local id -> circle id pairs but no base snapshots: the removal is still listed, with no
    // stem and with the number its pair's place gives (so two of them can be told apart), plan 4.7.
    let base = base_qs();
    let (mut bank, mut link) = linked_from(base.clone(), false);
    link.base_version = 3;
    link.head_seen = 3;
    link.role_seen = Some("owner".into());
    link.author_known = true;
    link.is_author = true;
    link.base.clear();
    bank.questions.pop(); // C removed
    bank.modified += 5;
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, heads(3));
    s.keep_raw("Get", &version_path(3), served_qs(3, base, false));
    let mut banks = vec![bank];
    let mut m = model();
    m.edit.add_link(link).unwrap();
    m.edit.frame(now(), &ready(&s));
    m.edit.sync(&mut banks);
    assert!(m.edit.open_send(LOCAL, owner()));
    m.edit.frame(now(), &ready(&s));
    m.edit.sync(&mut banks);
    assert!(matches!(m.edit.partial_send(), Some(PartialSend::Ready { .. })), "the removal is still listed: {:?}", m.edit.partial_send());
    let shapes = page(1280.0, &mut m);
    let t = texts(&shapes);
    assert!(has(&shapes, "Removed  Q3"), "the plain row, numbered by its pair's place: {t:?}");
    assert!(!has(&shapes, "\"C\""), "no stem without a snapshot: {t:?}");
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let c = local(&link, QC);
    assert!(m.edit.send_ticked().unwrap().contains(&c));
    assert!(click_text(&ctx(), 1280.0, "Removed  Q3", 0, &mut |ui| super::circle_edit_ui::draw_page(ui, &mut m)));
    assert!(!m.edit.send_ticked().unwrap().contains(&c), "the plain row's box toggles it");
    assert_eq!(super::circle_edit_ui::row_name(&super::partial_send::SendRow { local: c, kind: super::partial_send::SendKind::Removed, number: Some(3), stem: None }), "Removed, question 3");
}

#[test]
fn d3_ui_a_rows_accessible_name_carries_its_question_and_stem() {
    use super::partial_send::{SendKind, SendRow};
    let changed = SendRow { local: "l".into(), kind: SendKind::Changed, number: Some(3), stem: Some("Which of these is prime?".into()) };
    assert_eq!(super::circle_edit_ui::row_name(&changed), "Changed, question 3: \"Which of these is prime?\"");
    let added = SendRow { local: "l".into(), kind: SendKind::Added, number: Some(12), stem: Some("Solve 2x + 3 = 7".into()) };
    assert_eq!(super::circle_edit_ui::row_name(&added), "Added, question 12: \"Solve 2x + 3 = 7\"");
    // Two bounded removals stay distinguishable even with no stem: their numbers are their own.
    let bounded = SendRow { local: "l".into(), kind: SendKind::Removed, number: Some(8), stem: None };
    assert_eq!(super::circle_edit_ui::row_name(&bounded), "Removed, question 8");
    let plain = SendRow { local: "l".into(), kind: SendKind::Removed, number: None, stem: None };
    assert_eq!(super::circle_edit_ui::row_name(&plain), "Removed");
}

#[test]
fn d3_ui_the_unavailable_page_offers_get_latest_and_it_opens_the_merge() {
    let (mut m, s, _banks) = open_page_base_error(404, json!({"error": "version_not_retained"}), &|b| b.questions[0].stem = "A (changed)".into());
    let shapes = page(1280.0, &mut m);
    assert!(has(&shapes, PARTIAL_UNAVAILABLE) && has(&shapes, "Send everything") && has(&shapes, "Get latest"), "{:?}", texts(&shapes));
    s.on("Post", &format!("{}/merge", super::bank_links_fix8_tests::put_path()), 200, super::bank_links_fix8_tests::merge_start(3, 4));
    s.keep_raw("Get", &super::bank_links_fix8_tests::put_path(), served_qs(3, base_qs(), false));
    assert!(click_text(&ctx(), 1280.0, "Get latest", 0, &mut |ui| super::circle_edit_ui::draw_page(ui, &mut m)));
    assert!(matches!(&m.edit.view, EditView::Merge(_)), "Get latest from the send page opens the merge: {:?}", m.edit.view);
}

#[test]
fn d3_ui_every_summary_state_draws_at_1280_and_420_within_the_page() {
    // (plan 4.8) Every new state gets a frame at both widths, not only the summary.
    let states: [(&str, fn() -> (CirclesModel, Arc<Script>, Vec<QuestionBank>), &[&str]); 4] = [
        ("reading", reading_page, &[PARTIAL_READING]),
        ("read failed", || open_page_base_error(503, json!({"error": "unavailable"}), &|b| b.questions[0].stem = "A (changed)".into()), &[PARTIAL_READ_FAILED, "Try again"]),
        ("unavailable", || open_page_base_error(404, json!({"error": "version_not_retained"}), &|b| b.questions[0].stem = "A (changed)".into()), &[PARTIAL_UNAVAILABLE, "Send everything", "Get latest"]),
        ("notice", || open_page(&|b| b.questions[0].stem = "A (changed)".into(), &[]), &[]),
    ];
    for (state, make, wants) in states {
        let (mut m, _s, _banks) = make();
        if state == "notice" {
            if let EditView::Send(v) = &mut m.edit.view {
                v.notice = Some(SEND_LIST_AGAIN.into());
            }
        }
        for width in [1280.0, 420.0] {
            let shapes = page(width, &mut m);
            within(&shapes, width);
            for want in wants.iter().copied().chain(if state == "notice" { Some(SEND_LIST_AGAIN) } else { None }) {
                assert!(has(&shapes, want), "{state} at {width}: {want:?} is not drawn: {:?}", texts(&shapes));
            }
        }
    }
}

#[test]
fn d3_ui_a_partial_sends_result_says_what_went_and_what_stayed() {
    // A change, C removed and D added; the removal is unticked, so two of the three changes go.
    let (mut m, s, mut banks) = open_page(
        &|b| {
            b.questions[0].stem = "A (changed)".into();
            b.questions.pop();
            b.questions.push(added_question());
        },
        &[],
    );
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let c = local(&link, QC);
    m.edit.toggle_change(&c, false);
    send(&mut m, &s, &mut banks, sent_with(4, &[QA, QB, QC]));
    let shapes = page(1280.0, &mut m);
    let t = texts(&shapes);
    assert!(t.contains(&"Sent version 4 with 2 of your 3 changes. 1 change stays on this computer, not sent."), "the plan 4.7 result: {t:?}");
    assert!(!t.iter().any(|t| t.starts_with("Version 4 is now the circle's newest")), "the server's counts are replaced: {t:?}");
    // The one that stayed is still on this computer, and the note says which version was made.
    assert_eq!(m.edit.link_for_local(LOCAL).unwrap().held_back, vec![c], "the unticked change stays held back");
    // Singular and plurals of the same sentence.
    assert_eq!(super::circle_edit_ui::partial_sent_line(8, 2, 3), "Sent version 8 with 2 of your 3 changes. 1 change stays on this computer, not sent.");
    assert_eq!(super::circle_edit_ui::partial_sent_line(8, 1, 2), "Sent version 8 with 1 of your 2 changes. 1 change stays on this computer, not sent.");
    assert_eq!(super::circle_edit_ui::partial_sent_line(8, 1, 3), "Sent version 8 with 1 of your 3 changes. 2 changes stay on this computer, not sent.");
}

// ---- decision/picture rows name the bank's question, the request keeps the payload position ---------------------------------

#[test]
fn d3_a_picture_row_names_the_banks_question_and_the_request_keeps_the_payload_position() {
    // base order A, C, B; this bank A, B (C removed and unticked, so the payload is A, C, B); B has a picture.
    let base = vec![qfull(QA, "A"), qfull(QC, "C"), qfull(QB, "B")];
    let pic = b"the B picture".to_vec();
    let sha = sha256_hex(&pic);
    let file = format!("media/{sha}.png");
    let images = Arc::new(Pics::default());
    let media = Arc::new(MediaOnDisk::default());
    media.put(&file, &pic);
    let (mut m, s, mut banks) = open_page_in(
        base,
        &|b| {
            b.questions.remove(1); // C
            b.questions[1].image = Some(ImageRef { file: file.clone(), width: 4, height: 3 }); // B
        },
        &[QC],
        model_with(images, media),
    );
    m.edit.note_hiding(BID, true);
    m.edit.send();
    m.edit.sync(&mut banks);
    let ctx = ctx();
    let text = {
        let EditView::Send(v) = &m.edit.view else { panic!("not on the send page") };
        let SendStage::Pictures(p) = &v.stage else { panic!("not the picture step: {:?}", v.stage) };
        assert_eq!(p.list.iter().map(|x| x.question).collect::<Vec<_>>(), vec![2], "the picture is payload position 2");
        frame_text(&ctx, &mut |ui| {
            let _ = super::circle_hide_ui::pictures(ui, p, "intro", "Send", false, false);
        })
    };
    assert!(text.contains("Question 2"), "the bank's question 2 (B), not the payload's 3: {text}");
    assert!(!text.contains("Question 3"), "{text}");
    m.edit.send_keep_picture(2, true);
    m.edit.send_with_pictures();
    s.on("Put", &format!("/blobs/{sha}"), 200, json!({"sha256": sha, "size": pic.len(), "width": 4, "height": 3}));
    s.on("Put", &super::bank_links_fix8_tests::put_path(), 201, sent_with(4, &[QA, QC, QB]));
    m.edit.sync(&mut banks);
    m.edit.frame(now(), &ready(&s));
    let send = puts(&s).into_iter().rev().find(|x| x.path == super::bank_links_fix8_tests::put_path()).expect("the send went out");
    assert_eq!(put_meta(&send)["image_ok"], json!([2]), "the request names the payload position, not the bank's");
}

#[test]
fn d3_a_decision_refusal_names_the_banks_question_and_the_request_keeps_the_payload_position() {
    let base = vec![qfull(QA, "A"), qfull(QC, "C"), qfull(QB, "B")];
    let (mut m, s, mut banks) = open_page_with_base(
        base,
        &|b| {
            b.questions.remove(1); // C
            b.questions[1].stem = "B (changed)".into(); // so the send has a ticked change besides the held removal
        },
        &[QC],
    );
    s.on(
        "Put",
        &super::bank_links_fix8_tests::put_path(),
        422,
        json!({"error": "labels_need_decision", "errors": [], "label_findings": [{"question": 2, "strip_allowed": true, "choices": [{"before": "A) x", "after": "x"}]}]}),
    );
    m.edit.send();
    m.edit.sync(&mut banks);
    m.edit.frame(now(), &ready(&s));
    let ctx = ctx();
    let text = {
        let EditView::Send(v) = &m.edit.view else { panic!("not on the send page") };
        let SendStage::Decide(d) = &v.stage else { panic!("not the decisions page: {:?}", v.stage) };
        assert_eq!(d.refusal.label_findings[0].question, 2, "the refusal itself stays at the payload position");
        frame_text(&ctx, &mut |ui| {
            let _ = super::circle_hide_ui::decisions(ui, d, "intro", "fix", "Send", false);
        })
    };
    assert!(text.contains("Question 2"), "the bank's question 2 (B), not the payload's 3: {text}");
    assert!(!text.contains("Question 3"), "{text}");
    m.edit.send_decide_label(Pos { question: 2, part: None }, LabelChoice::Strip);
    m.edit.send_with_decisions();
    s.on("Put", &super::bank_links_fix8_tests::put_path(), 201, sent_with(4, &[QA, QC, QB]));
    m.edit.sync(&mut banks);
    m.edit.frame(now(), &ready(&s));
    let meta = put_meta(&puts(&s).last().expect("the send went out"));
    assert_eq!(meta["labels"][0]["question"], json!(2), "the request names the payload position, not the bank's: {meta}");
}

#[test]
fn d3_without_a_restored_removal_the_display_numbers_are_the_payload_numbers() {
    // No map: the page names each payload position as it is (the bank page, a whole-bank send).
    let payload = json!({"schema_version": 2, "bank": {"questions": [{"stem": "A"}, {"stem": "B", "image": {"file": format!("media/{}.png", "d".repeat(64))}}]}}).to_string();
    let p = Pictures::of_payload(payload.as_bytes()).expect("one picture");
    assert_eq!(p.display_question(1), 1);
    let d = Decisions::new(PayloadRefusal::default(), Default::default());
    assert_eq!(d.display_question(1), 1);
    // A payload position with no bank question (a restored removal) keeps its payload number, never a wrong bank one.
    let mut p = p;
    p.set_bank_map(vec![Some(0), Some(1)]);
    assert_eq!(p.display_question(1), 1, "no removal: the payload number is the bank's");
    p.set_bank_map(vec![Some(0), None, Some(2)]);
    assert_eq!(p.display_question(1), 1);
    let mut d = Decisions::new(PayloadRefusal::default(), Default::default());
    d.set_bank_map(vec![Some(0), None, Some(2)]);
    assert_eq!(d.display_question(2), 2);
}
