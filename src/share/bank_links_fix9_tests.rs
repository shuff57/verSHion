//! Phase D1, round 9 (critics 79 and 80): each test written first against the code it pins. A send or restore with no
//! clear answer is decided by the version it was based on, whatever read raised the newest version first; what such a
//! send carried is kept with the link (also across a restart), so Get latest matches the circle's questions to it even
//! after a correction; and the conflict that follows names a colleague's version as the colleague's.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::bank_links_fix4_tests::*;
use super::bank_links_fix8_tests::*;
use super::bank_links_fix_tests::*;
use super::bank_links_tests::*;
use super::circle_edit::*;
use super::circles::CirclesModel;
use super::circles_api::{HeadEntry, HeadState};
use super::circles_tests::*;
use super::merge::RowKind;
use super::publish::AccountSnapshot;
use crate::model::QuestionBank;

/// The added question as the teacher corrected it after sending it.
pub(super) const FIXED: &str = "Solve 5x = 25";

/// olga, as the Account window knows her: her display name is the one the circle shows for her versions.
pub(super) fn olga_named(s: &Arc<Script>) -> AccountSnapshot {
    AccountSnapshot { name: Some("Olga Teacher".into()), ..olga(s) }
}

pub(super) fn heads_by(version: u64, by: &str) -> Value {
    json!({"banks": [{"bank_id": BID, "status": "current", "version": version, "version_id": hex(version), "license": "CC-BY-4.0", "at": NOW - 1000, "by": by, "hide_key": false}]})
}

pub(super) fn kinds(m: &CirclesModel) -> Option<Vec<RowKind>> {
    match &m.edit.view {
        EditView::Merge(v) => match &v.phase {
            MergePhase::Review(r) => Some(r.plan.rows.iter().map(|r| r.kind).collect()),
            _ => None,
        },
        _ => None,
    }
}

pub(super) fn count(k: &[RowKind], of: RowKind) -> usize {
    k.iter().filter(|x| **x == of).count()
}

/// Frames of A's page as `who`.
pub(super) fn frames(m: &mut CirclesModel, banks: &mut [QuestionBank], ctx: &egui::Context, acct: &mut FakeAccount, who: &AccountSnapshot, n: usize) -> String {
    let mut t = String::new();
    for _ in 0..n {
        t = strip_as(ctx, m, banks, acct, who, LOCAL);
    }
    t
}

/// A (at version 3) changed by `edit`, sent by olga (`named`: the Account window knows her display name) with a note; the
/// send is answered `fail` or `status`/`body`, and the check asked for at once is answered `check`. The Send page is
/// then left.
pub(super) fn sent_with(edit: &dyn Fn(&mut QuestionBank), named: bool, fail: Option<super::api::ApiError>, status: u16, body: Value, check: (u16, Value)) -> Rig {
    let (mut m, s, mut banks) = two_at_three();
    let who = &if named { olga_named(&s) } else { olga(&s) };
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("owner", json!({})));
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip_as(&ctx, &mut m, &mut banks, &mut acct, who, LOCAL);
    edit(&mut banks[0]);
    banks[0].modified += 5;
    assert!(m.edit.open_send(LOCAL, owner()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    frames(&mut m, &mut banks, &ctx, &mut acct, who, 3);
    m.edit.set_message("Added a question");
    m.edit.set_attest(true);
    match fail {
        Some(e) => {
            s.on("Post", "/circles/heads", 200, heads(3));
            s.fail(e);
        }
        None => {
            s.on("Put", &put_path(), status, body);
        }
    }
    m.edit.send();
    m.edit.sync(&mut banks);
    assert!(s.paths().iter().any(|p| p.starts_with("Put ")), "the send went out: {:?}", s.paths());
    // the check asked for at once (and only it) is answered `check`
    s.on("Post", "/circles/heads", check.0, check.1);
    strip_as(&ctx, &mut m, &mut banks, &mut acct, who, LOCAL);
    m.edit.close();
    (m, s, banks, ctx, acct)
}

pub(super) fn add_question(b: &mut QuestionBank) {
    b.questions.push(serde_json::from_value(qjson("new-local", ADDED)).unwrap());
}

/// The answer to a send that went through but named no version this app can read.
pub(super) fn version_zero() -> Value {
    json!({"circle_id": CID, "bank_id": BID, "version": 0, "qids": [], "added": 0, "changed": 0, "removed": 0})
}

/// Get latest on A's page: the review of `served`, the circle's version 4 (sent by "Olga Teacher" from version 3).
pub(super) fn get_latest(m: &mut CirclesModel, s: &Arc<Script>, banks: &mut [QuestionBank], ctx: &egui::Context, acct: &mut FakeAccount, served: Vec<u8>) -> String {
    s.on("Post", &format!("{}/merge", put_path()), 200, merge_start(3, 4));
    s.keep_raw("Get", &put_path(), served);
    assert!(m.edit.open_merge(LOCAL, owner()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    until_as(ctx, m, banks, acct, s, LOCAL, &|m| kinds(m).is_some());
    strip_as(ctx, m, banks, acct, &olga(s), LOCAL)
}

pub(super) fn apply(m: &mut CirclesModel, s: &Arc<Script>, banks: &mut [QuestionBank], ctx: &egui::Context, acct: &mut FakeAccount) {
    m.edit.apply_merge();
    until_as(ctx, m, banks, acct, s, LOCAL, &|m| matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::Applied(_))));
    m.edit.cancel_merge();
}

pub(super) fn changes(m: &mut CirclesModel, bank: &QuestionBank) -> (usize, usize, usize) {
    let c = m.edit.status_of(bank).unwrap().changes;
    assert!(c.known);
    (c.added, c.changed, c.removed)
}

// ---- c79 I2: decided by the version the send or restore was based on ------------------------------------------------

#[test]
fn c79_i2_a_send_that_made_the_newest_version_is_not_said_to_have_failed_after_another_read_saw_it_first() {
    for read in ["shared page", "history"] {
        let (mut m, s, mut banks, ctx, mut acct) = unclear_send(timed_out(), 0, Value::Null, 4);
        // before the check asked for at once answers, another read learns version 4
        match read {
            "shared page" => m.edit.apply_heads(&[HeadEntry { bank_id: BID.into(), state: HeadState::Current { version: 4, license: None, at: Some(NOW), by: None, hide_key: None } }]),
            _ => {
                s.on("Get", &versions(), 200, page(4, &[4, 3, 2, 1]));
                history_on_a(&mut m);
            }
        }
        checked(&mut m, &s, &mut banks, &ctx, &mut acct);
        let r = said(&m, LOCAL);
        assert!(!r.contains("did not go through") && !r.contains("is checked again"), "{read}: {r}");
        assert!(r.contains("version 4") && r.contains("may be the one you sent"), "{read}: {r}");
    }
}

#[test]
fn c79_i2_a_restore_that_made_the_newest_version_is_not_said_to_have_failed_after_another_read_saw_it_first() {
    let (mut m, s, mut banks, ctx, mut acct) = restore_answered(timed_out(), 0, Value::Null, true);
    s.keep("Post", "/circles/heads", 200, heads(4));
    // the answer is read on A's page (the check is asked for at once)
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga(&s), LOCAL);
    m.edit.apply_heads(&[HeadEntry { bank_id: BID.into(), state: HeadState::Current { version: 4, license: None, at: Some(NOW), by: None, hide_key: None } }]);
    frames(&mut m, &mut banks, &ctx, &mut acct, &olga(&s), 3);
    let r = said(&m, LOCAL);
    assert!(!r.contains("did not go through") && !r.contains("may have gone through"), "{r}");
    assert!(r.contains("version 4") && r.contains("may be the restore of version 1"), "{r}");
}

/// The other way round: a read that saw no newer version does not make the check's "still version 3" a move.
#[test]
fn c79_i2_a_check_that_finds_the_version_the_send_was_based_on_says_it_has_not_arrived_after_another_read() {
    let (mut m, s, mut banks, ctx, mut acct) = unclear_send(timed_out(), 0, Value::Null, 3);
    m.edit.apply_heads(&[HeadEntry { bank_id: BID.into(), state: HeadState::Current { version: 3, license: None, at: Some(NOW - 5), by: None, hide_key: None } }]);
    checked(&mut m, &s, &mut banks, &ctx, &mut acct);
    let r = said(&m, LOCAL);
    // (critic 81 minor 5: not "did not go through")
    assert!(r.contains("may not have arrived yet") && r.contains("still has version 3"), "{r}");
}

// ---- c80 I2 (and c79 minor 1): who sent the newer version -------------------------------------------------------------

#[test]
fn c80_i2_the_check_names_a_colleagues_next_version_and_says_the_send_did_not_go_through() {
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, true, timed_out(), 0, Value::Null, (200, heads_by(4, "Bob Teacher")));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    let r = said(&m, LOCAL);
    assert!(r.contains("Bob Teacher sent version 4") && r.contains("did not go through"), "{r}");
    assert!(!r.contains("may be") && !r.contains("is checked again"), "{r}");
    // and a later conflict is the colleague's
    let page = conflict_by(&mut m, &s, &mut banks, &ctx, &mut acct, &who, 5, "Bob Teacher");
    assert!(page.contains("Someone else sent") && !page.contains("earlier send"), "{page}");
}

#[test]
fn c80_i2_the_check_finding_the_persons_own_next_version_says_the_send_went_through() {
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, true, timed_out(), 0, Value::Null, (200, heads_by(4, "Olga Teacher")));
    let who = olga_named(&s);
    let t = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    let r = said(&m, LOCAL);
    assert!(r.contains("version 4") && r.contains("sent by you") && r.contains("went through") && r.contains("Get latest"), "{r}");
    assert!(!r.contains("did not go through") && !r.contains("may be"), "{r}");
    assert!(t.contains("Get latest"), "{t}");
    // its changes went: the strip does not count them as unsent, and a conflict now is with the person's own send
    assert!(t.contains("Your changes were sent") && !t.contains("unsent"), "{t}");
    assert_eq!(m.edit.earlier_send(LOCAL), Some(true));
}

/// A send from A as `who` answered 409 for version `head` sent by `by`; the page's text.
pub(super) fn conflict_by(m: &mut CirclesModel, s: &Arc<Script>, banks: &mut [QuestionBank], ctx: &egui::Context, acct: &mut FakeAccount, who: &AccountSnapshot, head: u64, by: &str) -> String {
    banks[0].questions[0].stem = format!("One (fix {head})");
    banks[0].modified += 5;
    assert!(m.edit.open_send(LOCAL, owner()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    frames(m, banks, ctx, acct, who, 3);
    m.edit.set_attest(true);
    s.on("Put", &put_path(), 409, json!({"error": "conflict", "head_version": head, "head_at": NOW - 60_000, "head_by": by}));
    m.edit.send();
    m.edit.sync(banks);
    let page = strip_as(ctx, m, banks, acct, who, LOCAL);
    assert!(matches!(&m.edit.view, EditView::Send(v) if matches!(v.stage, SendStage::Conflict(_))), "{page}");
    page
}

#[test]
fn c80_i2_after_the_check_said_the_send_had_not_arrived_a_colleagues_conflict_is_never_your_earlier_send() {
    let (mut m, s, mut banks, ctx, mut acct) = unclear_send(timed_out(), 0, Value::Null, 3);
    checked(&mut m, &s, &mut banks, &ctx, &mut acct);
    // (critic 81 minor 5: "may not have arrived yet", not "did not go through"; the colleague is told apart by name, which
    // the Account window always gives while signed in)
    assert!(said(&m, LOCAL).contains("may not have arrived yet"));
    let who = olga_named(&s);
    let page = conflict_by(&mut m, &s, &mut banks, &ctx, &mut acct, &who, 4, "Bob Teacher");
    assert!(!page.contains("earlier send") && page.contains("Someone else sent"), "{page}");
    m.edit.close();
    let r = said(&m, LOCAL);
    assert!(!r.contains("earlier send") && r.contains("someone sent version 4 first"), "{r}");
}

/// No check answered (offline until after the colleague's send): the 409's own `head_by` decides.
#[test]
fn c80_i2_with_no_check_a_conflict_naming_a_colleague_is_theirs_and_one_naming_the_person_is_their_own() {
    for (by, own) in [("Bob Teacher", false), ("Olga Teacher", true)] {
        let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, true, timed_out(), 0, Value::Null, (503, json!({"error": "unavailable"})));
        let who = olga_named(&s);
        frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
        assert!(said(&m, LOCAL).contains("is checked again"), "{by}: no check answered: {}", said(&m, LOCAL));
        let page = conflict_by(&mut m, &s, &mut banks, &ctx, &mut acct, &who, 4, by);
        m.edit.close();
        let r = said(&m, LOCAL);
        if own {
            assert!(page.contains("Your earlier send went through") && !page.contains("Someone else"), "{page}");
            assert!(r.contains("your earlier send went through") && !r.contains("someone"), "{r}");
        } else {
            assert!(page.contains("Someone else sent") && !page.contains("earlier send"), "{page}");
            assert!(!r.contains("earlier send") && r.contains("someone sent version 4 first"), "{r}");
        }
    }
}

/// (c79 minor 1) After a restart, a conflict with the person's own version (named by the 409) is never blamed on
/// someone else; one with a colleague's still is the colleague's.
#[test]
fn c79_m1_after_a_restart_a_conflict_with_the_persons_own_version_is_not_someone_elses() {
    for (by, own) in [("Olga Teacher", true), ("Bob Teacher", false)] {
        let (mut m0, s, mut banks, ctx, mut acct) = sent_with(&add_question, true, timed_out(), 0, Value::Null, (503, json!({"error": "unavailable"})));
        let log = Arc::new(Mutex::new(m0.edit.links().to_vec()));
        drop(m0);
        let mut m = restarted(log);
        let who = olga_named(&s);
        // the circle can not be reached for the check at first
        s.on("Post", "/circles/heads", 503, json!({"error": "unavailable"}));
        frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
        let page = conflict_by(&mut m, &s, &mut banks, &ctx, &mut acct, &who, 4, by);
        if own {
            assert!(page.contains("Your earlier send went through") && !page.contains("Someone else"), "{page}");
        } else {
            assert!(page.contains("Someone else sent") && !page.contains("earlier send"), "{page}");
        }
    }
}

// ---- c80 I1: what a send with no clear answer carried is matched, even after a correction -----------------------------

#[test]
fn c80_i1_a_question_added_by_an_unclear_send_then_corrected_is_linked_once_and_counts_as_changed() {
    for name in ["timeout", "version 0"] {
        let (mut m, s, mut banks, ctx, mut acct) = match name {
            "timeout" => sent_with(&add_question, false, timed_out(), 0, Value::Null, (200, heads(4))),
            _ => sent_with(&add_question, false, None, 201, version_zero(), (200, heads(4))),
        };
        // the teacher corrects the question just sent, before Get latest
        banks[0].questions.iter_mut().find(|q| q.stem == ADDED).unwrap().stem = FIXED.into();
        banks[0].modified += 5;
        checked(&mut m, &s, &mut banks, &ctx, &mut acct);
        let page = get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served(4, &[(QA, "One"), (QN, ADDED)], false));
        let k = kinds(&m).unwrap();
        assert_eq!((count(&k, RowKind::NewInTheirs), count(&k, RowKind::Conflict), count(&k, RowKind::OursChanged)), (0, 0, 1), "{name}: {page}");
        assert!(!page.contains("New in the circle") && !page.contains("yours alone") && !page.contains("CONFLICT"), "{name}: {page}");
        assert!(page.contains("Version 4 is your own send"), "{name}: {page}");
        assert!(!page.contains("added the same question"), "{name}: the corrected question is not the same: {page}");
        apply(&mut m, &s, &mut banks, &ctx, &mut acct);
        assert_eq!(banks[0].questions.iter().filter(|q| q.stem == FIXED).count(), 1, "{name}");
        assert!(!banks[0].questions.iter().any(|q| q.stem == ADDED), "{name}: the old wording is not added back");
        assert_eq!(banks[0].questions.len(), 2, "{name}");
        assert_eq!(m.edit.with_link(LOCAL, |l| (l.base_version, l.qids.len())), Some((4, 2)), "{name}");
        assert_eq!(changes(&mut m, &banks[0].clone()), (0, 1, 0), "{name}: the correction is the one unsent change");
        let t = strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga(&s), LOCAL);
        assert!(t.contains("1 unsent change"), "{name}: {t}");
    }
}

#[test]
fn c80_i1_a_question_changed_before_and_after_an_unclear_send_is_changed_only_by_you_never_a_conflict() {
    for restart in [false, true] {
        let edit = |b: &mut QuestionBank| b.questions[0].stem = "One (13)".into();
        let (mut m0, s, mut banks, ctx, mut acct) = sent_with(&edit, false, timed_out(), 0, Value::Null, (200, heads(4)));
        let mut m = if restart {
            let log = Arc::new(Mutex::new(m0.edit.links().to_vec()));
            restarted(log)
        } else {
            m0
        };
        banks[0].questions[0].stem = "One (15)".into();
        banks[0].modified += 5;
        checked(&mut m, &s, &mut banks, &ctx, &mut acct);
        let page = get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served(4, &[(QA, "One (13)")], false));
        let k = kinds(&m).unwrap();
        assert_eq!((count(&k, RowKind::Conflict), count(&k, RowKind::TheirsChanged), count(&k, RowKind::OursChanged)), (0, 0, 1), "restart {restart}: {page}");
        assert!(!page.contains("CONFLICT") && page.contains("Version 4 is your own send"), "restart {restart}: {page}");
        assert!(page.contains("You sent this in version 4 and changed it since"), "restart {restart}: {page}");
        // what was sent is the circle's newest, never shown as version 3
        assert!(!page.contains("In version 3") && page.contains("Yours now"), "restart {restart}: {page}");
        apply(&mut m, &s, &mut banks, &ctx, &mut acct);
        assert_eq!(banks[0].questions[0].stem, "One (15)", "restart {restart}: the later edit stays");
        assert_eq!(changes(&mut m, &banks[0].clone()), (0, 1, 0), "restart {restart}");
    }
}

/// What the send carried is matched only where the circle has it as it was sent: a question the circle changed after
/// it (a colleague's version on top of the person's) is still both changed.
#[test]
fn c80_i1_a_question_a_colleague_changed_after_the_send_is_still_a_conflict() {
    let edit = |b: &mut QuestionBank| b.questions[0].stem = "One (13)".into();
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&edit, false, timed_out(), 0, Value::Null, (200, heads(4)));
    banks[0].questions[0].stem = "One (15)".into();
    banks[0].modified += 5;
    checked(&mut m, &s, &mut banks, &ctx, &mut acct);
    let page = get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served(4, &[(QA, "One (17)")], false));
    let k = kinds(&m).unwrap();
    assert_eq!(count(&k, RowKind::Conflict), 1, "{page}");
    assert!(!page.contains("is your own send"), "nothing the circle has is what was sent: {page}");
}

/// A normal send whose answer is read, Get latest, and Unlink drop what an unclear send carried.
#[test]
fn c80_i1_what_an_unclear_send_carried_is_dropped_once_a_send_is_read_or_the_copy_catches_up() {
    let (mut m, s, mut banks, ctx, mut acct) = unclear_send(timed_out(), 0, Value::Null, 4);
    assert!(m.edit.with_link(LOCAL, |l| l.sent_record.is_some()).unwrap(), "kept with the link");
    checked(&mut m, &s, &mut banks, &ctx, &mut acct);
    get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served(4, &[(QA, "One"), (QN, ADDED)], false));
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert!(m.edit.with_link(LOCAL, |l| l.sent_record.is_none()).unwrap(), "dropped once the copy caught up");
    let (mut m, s, mut banks, ctx, mut acct) = unclear_send(timed_out(), 0, Value::Null, 4);
    checked(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert!(m.edit.open_send(LOCAL, owner()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    frames(&mut m, &mut banks, &ctx, &mut acct, &olga(&s), 3);
    m.edit.set_attest(true);
    s.on("Put", &put_path(), 201, json!({"circle_id": CID, "bank_id": BID, "version": 4, "version_id": hex(4), "qids": [QA, QN], "added": 1, "changed": 0, "removed": 0, "replayed": true}));
    m.edit.send();
    m.edit.sync(&mut banks);
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga(&s), LOCAL);
    assert!(m.edit.with_link(LOCAL, |l| l.sent_record.is_none()).unwrap(), "dropped once a send's answer is read");
}

/// Another account on this computer is never told that the newer version may be its own earlier send.
#[test]
fn c80_i2_another_account_after_a_restart_is_never_told_it_may_be_its_earlier_send() {
    let (mut m0, s, mut banks, ctx, mut acct) = sent_with(&add_question, false, timed_out(), 0, Value::Null, (503, json!({"error": "unavailable"})));
    let log = Arc::new(Mutex::new(m0.edit.links().to_vec()));
    let mut m = restarted(log);
    let pete = as_who(&s, "pete");
    s.on("Post", "/circles/heads", 503, json!({"error": "unavailable"}));
    frames(&mut m, &mut banks, &ctx, &mut acct, &pete, 2);
    let page = conflict_by(&mut m, &s, &mut banks, &ctx, &mut acct, &pete, 4, "Olga Teacher");
    assert!(page.contains("Someone else sent") && !page.contains("earlier send"), "{page}");
}

// ---- c79 I3 and minor 5: the same question, as verSHion's server stores it ---------------------------------------------

/// A question of `stem` whose choice, explanation and topic are written with `e` (decomposed or not).
pub(super) fn q_with(id: &str, stem: &str, e: &str) -> crate::model::Question {
    let mut q: crate::model::Question = serde_json::from_value(qjson(id, stem)).unwrap();
    if let crate::model::Kind::Mc { answers } = &mut q.kind {
        answers[0].text = format!("Caf{e}");
    }
    q.explanation = Some(format!("R{e}sum{e}"));
    q.topics = vec![format!("Fran{e}ais")];
    q
}

pub(super) const COMPOSED: &str = "\u{e9}";
pub(super) const DECOMPOSED: &str = "e\u{301}";

#[test]
fn c79_i3_canonical_text_is_what_the_server_stores() {
    use super::canon::{block, line};
    assert_eq!(block("Cafe\u{301}"), block("Caf\u{e9}"));
    assert_ne!(block("Cafe"), block("Caf\u{e9}"));
    // marks in either order (different classes) are the same; the same class in another order is not
    assert_eq!(block("a\u{323}\u{301}"), block("a\u{301}\u{323}"));
    assert_ne!(block("a\u{301}\u{300}"), block("a\u{300}\u{301}"));
    assert_eq!(block("\u{1e09}"), block("c\u{327}\u{301}"), "two marks, precomposed");
    assert_eq!(block("\u{1e09}"), block("\u{e7}\u{301}"));
    // Hangul by rule, with and without a final consonant
    assert_eq!(block("\u{d55c}"), block("\u{1112}\u{1161}\u{11ab}"));
    assert_eq!(block("\u{ac00}"), block("\u{1100}\u{1161}"));
    assert_ne!(block("\u{ac00}"), block("\u{ac01}"));
    // a singleton and a compatibility ideograph
    assert_eq!(block("\u{212b}"), block("\u{c5}"));
    assert_eq!(block("\u{f900}"), block("\u{8c48}"));
    // line breaks, the byte-order mark
    assert_eq!(block("a\r\nb\rc\u{2028}d"), "a\nb\nc\nd");
    assert_eq!(block("\u{feff}ab"), "ab");
    assert_eq!(block("plain ascii"), "plain ascii");
    assert_eq!(block("a\r\nb\rc"), "a\nb\nc", "ASCII too");
    // one-line text: white space collapsed and trimmed
    assert_eq!(line("  Fran\u{e7}ais \t culture "), line("Franc\u{327}ais culture"));
    assert_ne!(line("a b"), line("ab"));
}

#[test]
fn c79_i3_a_question_the_server_stored_composed_is_matched_to_the_banks_decomposed_one() {
    use super::merge::{build_plan, match_new_to_unlinked};
    let (mut bank, link) = linked(&[(QA, "One")]);
    bank.questions.push(q_with("mine-two", &format!("Caf{DECOMPOSED}?"), DECOMPOSED));
    let mut theirs = super::circle_links::circle_theirs(&payload(&[(QA, "One"), (QB, "x")]), NOW);
    theirs[1].q = q_with("t", &format!("Caf{COMPOSED}?"), COMPOSED);
    let mut plan = build_plan(BID, &link.base, &bank, &theirs, 1, 2, None, String::new());
    match_new_to_unlinked(&mut plan, &bank, &theirs, &same_picture, "same");
    assert_eq!((plan.count(RowKind::NewInTheirs), plan.matched, plan.unlinked), (0, 1, 0), "matched, not new in the circle");
}

#[test]
fn c79_m5_matching_also_compares_difficulty_and_figure() {
    use super::merge::{build_plan, match_new_to_unlinked};
    use crate::model::{Difficulty, FigureSpec};
    let fig = |v: Value| Some(FigureSpec { id: "number-line".into(), params: [("min".to_string(), v), ("label".to_string(), json!(format!("x{DECOMPOSED}")))].into_iter().collect() });
    let matched = |mine: &dyn Fn(&mut crate::model::Question), circle: &dyn Fn(&mut crate::model::Question)| {
        let (mut bank, link) = linked(&[(QA, "One")]);
        let mut q = q_with("mine-two", "Two", COMPOSED);
        mine(&mut q);
        bank.questions.push(q);
        let mut theirs = super::circle_links::circle_theirs(&payload(&[(QA, "One"), (QB, "x")]), NOW);
        let mut t = q_with("t", "Two", COMPOSED);
        circle(&mut t);
        theirs[1].q = t;
        let mut plan = build_plan(BID, &link.base, &bank, &theirs, 1, 2, None, String::new());
        match_new_to_unlinked(&mut plan, &bank, &theirs, &same_picture, "same");
        plan.matched == 1
    };
    assert!(matched(&|_| {}, &|_| {}));
    assert!(!matched(&|q| q.difficulty = Some(Difficulty::High), &|q| q.difficulty = Some(Difficulty::Low)), "another difficulty");
    assert!(!matched(&|q| q.difficulty = Some(Difficulty::High), &|_| {}));
    assert!(!matched(&|q| q.figure = fig(json!(1)), &|_| {}), "a figure on one side only");
    assert!(!matched(&|q| q.figure = fig(json!(1)), &|q| q.figure = fig(json!(2))), "another value");
    assert!(matched(&|q| q.figure = fig(json!(1.0)), &|q| q.figure = fig(json!(1))), "1.0 and 1 are the same number");
    assert!(matched(&|q| q.figure = fig(json!(1)), &|q| {
        q.figure = fig(json!(1));
        if let Some(f) = &mut q.figure {
            f.params.insert("label".into(), json!(format!("x{COMPOSED}")));
        }
    }), "a text value the server stored composed");
    assert!(!matched(&|q| q.figure = fig(json!(1)), &|q| {
        q.figure = fig(json!(1));
        if let Some(f) = &mut q.figure {
            f.id = "coordinate-plane".into();
        }
    }), "another figure");
}

/// A question this bank sent and the circle stored composed is not "changed in the circle" on the next Get latest, and a
/// bank question the circle has composed is not an unsent change.
#[test]
fn c79_i3_the_servers_composed_form_is_no_change_on_either_side() {
    use super::merge::build_plan;
    let (mut bank, mut link) = linked(&[(QA, "One")]);
    bank.questions[0] = q_with(&bank.questions[0].id.clone(), &format!("Caf{DECOMPOSED}"), DECOMPOSED);
    // the copy's base is what it sent (decomposed); the circle has it composed
    link.base[0].snap = super::merge::snap_of(&bank.questions[0], None);
    let mut theirs = super::circle_links::circle_theirs(&payload(&[(QA, "x")]), NOW);
    theirs[0].q = q_with("t", &format!("Caf{COMPOSED}"), COMPOSED);
    let plan = build_plan(BID, &link.base, &bank, &theirs, 1, 2, None, String::new());
    assert_eq!(plan.rows[0].kind, RowKind::Unchanged, "{:?}", plan.rows[0].diffs);
    // the other way round: a base that is the circle's (composed), the bank decomposed
    link.base[0].snap = super::merge::snap_of(&theirs[0].q, None);
    assert_eq!(super::circle_links::changes_since(&link, &bank).total(), 0);
    assert!(!super::merge::ours_changed(&link.base[0], &bank.questions[0]));
    // a real change still is one
    bank.questions[0].stem = "Tea".into();
    assert!(super::merge::ours_changed(&link.base[0], &bank.questions[0]));
}

/// End to end: an unclear send of a question typed decomposed, which the circle stores composed: Get latest links it.
#[test]
fn c79_i3_get_latest_after_an_unclear_send_matches_a_question_the_circle_stored_composed() {
    let stem = format!("Caf{DECOMPOSED} au lait?");
    let add = |b: &mut QuestionBank| b.questions.push(q_with("new-local", &format!("Caf{DECOMPOSED} au lait?"), DECOMPOSED));
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add, false, timed_out(), 0, Value::Null, (200, heads(4)));
    checked(&mut m, &s, &mut banks, &ctx, &mut acct);
    let mut circle = String::from_utf8(served(4, &[(QA, "One"), (QN, "PLACEHOLDER")], false)).unwrap();
    let q = serde_json::to_value(q_with(QN, &format!("Caf{COMPOSED} au lait?"), COMPOSED)).unwrap();
    let placeholder = serde_json::to_string(&qjson(QN, "PLACEHOLDER")).unwrap();
    assert!(circle.contains(&placeholder));
    circle = circle.replace(&placeholder, &q.to_string());
    let page = get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, circle.into_bytes());
    let k = kinds(&m).unwrap();
    assert_eq!((count(&k, RowKind::NewInTheirs), count(&k, RowKind::OursChanged)), (0, 0), "{page}");
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!(banks[0].questions.iter().filter(|q| q.stem.starts_with("Caf")).count(), 1);
    assert!(banks[0].questions.iter().any(|q| q.stem == stem), "the bank keeps its own text");
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 0, 0), "and it is no unsent change");
}

// ---- c80 polish ---------------------------------------------------------------------------------------------------------

#[test]
fn c80_polish_the_merge_review_says_how_many_are_unchanged_once() {
    let (mut m, s, mut banks, ctx, mut acct) = unclear_send(timed_out(), 0, Value::Null, 4);
    checked(&mut m, &s, &mut banks, &ctx, &mut acct);
    let page = get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served(4, &[(QA, "One"), (QN, ADDED)], false));
    assert_eq!(page.matches("2 unchanged").count(), 1, "{page}");
}

/// The account's invite code as every circle screen says it (the join page, the Circles list, Create a circle): never
/// "join circles" where nobody is joining, nor "the circle's code" where none was entered.
#[test]
fn c80_polish_the_invite_sentence_fits_every_circle_screen() {
    let t = super::circles_text::circle_words(super::pool::INVITE_REQUIRED_TEXT);
    assert_eq!(t, CIRCLE_INVITE_REQUIRED);
    assert!(t.contains("before it can use circles") && t.contains("The code it needs is not a circle's code"), "{t}");
    assert!(!t.contains("join circles") && !t.contains("This is not the circle's code") && !t.contains("pool"), "{t}");
}

// ---- critic 79 survivors e26, g02, g06 -------------------------------------------------------------------------------

/// (e26) Once Get latest caught the copy up, the next Send page does not start from the earlier send with no clear
/// answer (its note): that send is settled.
#[test]
fn c79_e26_after_get_latest_the_next_send_page_does_not_start_from_the_earlier_unclear_send() {
    let (mut m, s, mut banks, ctx, mut acct) = unclear_send(timed_out(), 0, Value::Null, 4);
    checked(&mut m, &s, &mut banks, &ctx, &mut acct);
    get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served(4, &[(QA, "One"), (QN, ADDED)], false));
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    banks[0].questions[0].stem = "One, later".into();
    banks[0].modified += 5;
    assert!(m.edit.open_send(LOCAL, owner()));
    assert!(matches!(&m.edit.view, EditView::Send(v) if v.message.is_empty()), "the old note is gone");
}

/// (g02) Two questions new in the circle, the same as each other and as one question of the bank: only one is linked
/// to it; the other is still new (the circle has it twice).
#[test]
fn c79_g02_one_bank_question_is_matched_to_one_new_circle_question_only() {
    use super::merge::{build_plan, match_new_to_unlinked};
    const QC: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa3";
    let (mut bank, link) = linked(&[(QA, "One")]);
    bank.questions.push(serde_json::from_value(qjson("mine-two", "Two")).unwrap());
    let theirs = super::circle_links::circle_theirs(&payload(&[(QA, "One"), (QB, "Two"), (QC, "Two")]), NOW);
    let mut plan = build_plan(BID, &link.base, &bank, &theirs, 1, 2, None, String::new());
    match_new_to_unlinked(&mut plan, &bank, &theirs, &same_picture, "same");
    assert_eq!((plan.count(RowKind::NewInTheirs), plan.matched, plan.unlinked), (1, 1, 0));
    let to: Vec<&str> = plan.rows.iter().filter(|r| r.local_id.as_deref() == Some("mine-two")).map(|r| r.theirs_qid.as_str()).collect();
    assert_eq!(to, vec![QB], "linked once");
    // (g06) a linked question changes nothing in the bank: it is not added, and offers no choice
    let (mut bank, link) = linked(&[(QA, "One")]);
    bank.questions.push(serde_json::from_value(qjson("mine-two", "Two")).unwrap());
    let theirs = super::circle_links::circle_theirs(&payload(&[(QA, "One"), (QB, "Two")]), NOW);
    let mut plan = build_plan(BID, &link.base, &bank, &theirs, 1, 2, None, String::new());
    assert!(plan.changes_anything(), "before: Add is ticked");
    match_new_to_unlinked(&mut plan, &bank, &theirs, &same_picture, "same");
    assert!(!plan.changes_anything(), "a matched question changes nothing");
    assert!(plan.rows.iter().all(|r| r.local_id.as_deref() != Some("mine-two") || r.options.is_empty()));
}

/// A newer version two above the one the send was based on, the newest by a colleague: the person's send may be the one
/// in between, so it is not said to have failed.
#[test]
fn c80_i2_a_colleagues_version_two_above_the_send_does_not_say_it_failed() {
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, true, timed_out(), 0, Value::Null, (200, heads_by(5, "Bob Teacher")));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    let r = said(&m, LOCAL);
    assert!(!r.contains("did not go through") && r.contains("version 5"), "{r}");
}

/// Once a colleague's version is known to have come right after the one the send was based on (a 409 naming them), a
/// later conflict that names nobody is not "your earlier send" either.
#[test]
fn c80_i2_a_colleagues_conflict_settles_the_earlier_send_for_later_conflicts() {
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, true, timed_out(), 0, Value::Null, (503, json!({"error": "unavailable"})));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    conflict_by(&mut m, &s, &mut banks, &ctx, &mut acct, &who, 4, "Bob Teacher");
    m.edit.close();
    let page = conflict_by(&mut m, &s, &mut banks, &ctx, &mut acct, &who, 5, "");
    assert!(page.contains("Someone else sent") && !page.contains("earlier send"), "{page}");
}

/// Where nothing the circle has is what the send carried, the review never says the version is the person's own: a
/// question the send did not change is not evidence.
#[test]
fn c80_i1_an_unchanged_question_is_no_evidence_of_the_persons_own_send() {
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, false, timed_out(), 0, Value::Null, (200, heads(4)));
    checked(&mut m, &s, &mut banks, &ctx, &mut acct);
    let page = get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served(4, &[(QA, "One"), (QB, "A colleague's question")], false));
    assert!(!page.contains("is your own send"), "{page}");
    assert_eq!(count(&kinds(&m).unwrap(), RowKind::NewInTheirs), 1, "{page}");
}

/// The answer key's own texts (a sample answer) are compared as the server stores them too.
#[test]
fn c79_i3_a_written_answers_sample_is_compared_as_stored() {
    use super::merge::{build_plan, match_new_to_unlinked};
    let short = |id: &str, sample: &str| -> crate::model::Question {
        let mut q: crate::model::Question = serde_json::from_value(qjson(id, "Name the drink")).unwrap();
        q.kind = crate::model::Kind::Short { sample: sample.into() };
        q
    };
    let matched = |mine: &str, circle: &str| {
        let (mut bank, link) = linked(&[(QA, "One")]);
        bank.questions.push(short("mine-two", mine));
        let mut theirs = super::circle_links::circle_theirs(&payload(&[(QA, "One"), (QB, "x")]), NOW);
        theirs[1].q = short("t", circle);
        let mut plan = build_plan(BID, &link.base, &bank, &theirs, 1, 2, None, String::new());
        match_new_to_unlinked(&mut plan, &bank, &theirs, &same_picture, "same");
        plan.matched == 1
    };
    assert!(matched(&format!("Caf{DECOMPOSED}"), &format!("Caf{COMPOSED}")));
    assert!(!matched("Tea", "Coffee"));
}

/// (c80 I2) A colleague's version two above the one the send was based on, named by the 409: theirs, never "your
/// earlier send" (the person's may be the one in between, which Get latest shows).
#[test]
fn c80_i2_a_conflict_with_a_colleagues_version_two_above_is_theirs() {
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, true, timed_out(), 0, Value::Null, (503, json!({"error": "unavailable"})));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    let page = conflict_by(&mut m, &s, &mut banks, &ctx, &mut acct, &who, 5, "Bob Teacher");
    assert!(page.contains("Someone else sent") && !page.contains("earlier send"), "{page}");
}

/// Once the check found a colleague's version right after the one the send was based on, a later conflict that names
/// nobody is not "your earlier send".
#[test]
fn c80_i2_after_the_check_named_a_colleague_a_later_unnamed_conflict_is_not_your_earlier_send() {
    let (mut m, s, mut banks, ctx, mut acct) = sent_with(&add_question, true, timed_out(), 0, Value::Null, (200, heads_by(4, "Bob Teacher")));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    let page = conflict_by(&mut m, &s, &mut banks, &ctx, &mut acct, &who, 5, "");
    assert!(page.contains("Someone else sent") && !page.contains("earlier send"), "{page}");
}

/// A question sent with its picture (its answer lost), which the circle has as sent: matched with its picture, not
/// "changed by you".
#[test]
fn c80_i1_a_sent_question_with_its_picture_is_unchanged_once_the_circle_has_it() {
    let file = format!("media/{}.webp", "b".repeat(64));
    let (mut m, s, mut banks) = two_at_three();
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("owner", json!({})));
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    frames(&mut m, &mut banks, &ctx, &mut acct, &olga(&s), 2);
    banks[0].questions[0].stem = "One (13)".into();
    banks[0].questions[0].image = Some(crate::model::ImageRef { file: file.clone(), width: 2, height: 2 });
    banks[0].modified += 5;
    let sent = banks[0].clone();
    let record = m.edit.with_link(LOCAL, |l| super::circle_links::sent_record(l, &sent, 3, String::new())).unwrap();
    m.edit.update_link(LOCAL, |l| {
        l.sent_record = Some(record);
        true
    });
    s.keep("Post", "/circles/heads", 200, heads(4));
    m.links.check_now(LOCAL);
    frames(&mut m, &mut banks, &ctx, &mut acct, &olga(&s), 3);
    let plain = serde_json::to_string(&qjson(QA, "One (13)")).unwrap();
    let mut pictured = qjson(QA, "One (13)");
    pictured["image"] = json!({"file": file, "width": 2, "height": 2});
    let circle = String::from_utf8(served(4, &[(QA, "One (13)")], false)).unwrap().replace(&plain, &pictured.to_string());
    let page = get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, circle.into_bytes());
    let k = kinds(&m).unwrap();
    assert_eq!((count(&k, RowKind::Unchanged), count(&k, RowKind::OursChanged), count(&k, RowKind::Conflict)), (1, 0, 0), "{page}");
    assert!(page.contains("Version 4 is your own send"), "{page}");
}
