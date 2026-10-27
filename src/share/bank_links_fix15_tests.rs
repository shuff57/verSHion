//! Phase D1, round 15 (critics 91 and 92, and the owner's simplification of 2026-10-26): each test written first against the
//! code it pins. A link an earlier build wrote reads the circle's version at its base version once; after that read the copy's
//! difficulty for each question the version has is the circle's (a later deliberate change of hers supersedes), and an open
//! question editor is told. Until the read has succeeded nothing is sent and nothing is said to be up to date.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use super::bank_links_fix10_tests::*;
use super::bank_links_fix11_tests::{rated, rig, version_path, version_reads};
use super::bank_links_fix4_tests::*;
use super::bank_links_fix8_tests::*;
use super::bank_links_fix9_tests::*;
use super::bank_links_fix_tests::*;
use super::bank_links_tests::*;
use super::circle_edit::*;
use super::circles::CirclesModel;
use super::circles_tests::*;
use super::merge::{Choice, RowKind};
use super::publish::AccountSnapshot;
use crate::model::{Difficulty, QuestionBank};

/// A's link and bank as an earlier build left them at `version`: questions One and Two rated `one` and `two`, a base that kept
/// no difficulty, the bank marked with the link, and no record of a read.
pub(super) fn phase_c(m: &mut CirclesModel, banks: &mut [QuestionBank], version: u64, one: Option<Difficulty>, two: Option<Difficulty>) {
    let (mut bank, mut link) = linked(&[(QA, "One"), (QB, "Two")]);
    bank.questions[0].difficulty = one;
    bank.questions[1].difficulty = two;
    for b in &mut link.base {
        b.snap.difficulty = None;
    }
    link.base_version = version;
    link.base_version_id = hex(version);
    link.head_seen = version;
    link.mark_bank(&bank);
    link.difficulty_read = None;
    m.edit.update_link(LOCAL, |l| {
        link.role_seen = l.role_seen.clone();
        link.role_seen_by = l.role_seen_by.clone();
        link.author_known = l.author_known;
        link.is_author = l.is_author;
        *l = link;
        true
    });
    banks[0] = bank;
}

/// The circle's version 3: One rated `one`, Two not rated.
fn v3(one: Option<&str>) -> Vec<u8> {
    served_qs(3, vec![rated(QA, "One", one), rated(QB, "Two", None)], false)
}

fn rows(m: &CirclesModel) -> Vec<super::merge::Row> {
    match &m.edit.view {
        EditView::Merge(v) => match &v.phase {
            MergePhase::Review(r) => r.plan.rows.clone(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// What A's next send carries as One's difficulty.
fn sent_one(m: &mut CirclesModel, banks: &[QuestionBank]) -> Value {
    let link = m.edit.link_for_local(LOCAL).unwrap();
    let sent: Value = serde_json::from_slice(&super::circle_links::payload_of(&link, &banks[0]).unwrap()).unwrap();
    sent["bank"]["questions"][0]["difficulty"].clone()
}

/// Bob's version 4: version 3 as served with a question added.
fn v4(one: Option<&str>) -> Vec<u8> {
    served_qs(4, vec![rated(QA, "One", one), rated(QB, "Two", None), rated(QC, "Three", None)], false)
}

fn bobs_four() -> Value {
    let mut start = merge_start_by(3, 4, "Bob Teacher");
    start["added"] = json!(1);
    start
}

// ---- the owner's rule: after the one read, the copy's difficulty is the circle's -------------------------------------------

/// (c91 P2) Bob re-rated One Low -> High in version 3; the phase C Get latest kept her old Low. After the read her copy has
/// High, nothing is hers to send, and Bob's version 4 is plain to take.
#[test]
fn c91_p2_a_colleagues_rerating_the_phase_c_merge_dropped_is_the_copys() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    s.keep_raw("Get", &version_path(3), v3(Some("High")));
    let who = olga_named(&s);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High), "{strip}");
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 0, 0), "{strip}");
    assert!(strip.contains("up to date") && !strip.contains("unsent"), "{strip}");
    assert_eq!(sent_one(&mut m, &banks), json!("High"));
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, bobs_four(), v4(Some("High")));
    assert_eq!(kinds(&m).unwrap(), vec![RowKind::Unchanged, RowKind::Unchanged, RowKind::NewInTheirs], "{page}");
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High));
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 0, 0));
}

/// (c92 I1) The phase C Get latest dropped Bob's "High" on One; she fixed a typo in One since. After the read One has his
/// rating and her fix: the fix is hers to send, the rating is not, no page says she removed it, and her send carries "High".
#[test]
fn c92_i1_a_typo_fixed_since_still_takes_the_circles_rating() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, None, None);
    banks[0].questions[0].stem = "One (typo fixed)".into();
    banks[0].questions[0].modified = NOW / 1000 + 60;
    banks[0].modified = NOW / 1000 + 60;
    s.keep_raw("Get", &version_path(3), v3(Some("High")));
    let who = olga_named(&s);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!((banks[0].questions[0].stem.as_str(), banks[0].questions[0].difficulty), ("One (typo fixed)", Some(Difficulty::High)), "{strip}");
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 1, 0), "only the typo: {strip}");
    let page = latest_as(&mut m, &s, &mut banks, &ctx, &mut acct, &who, bobs_four(), v4(Some("High")));
    let r = rows(&m);
    assert_eq!(r[0].kind, RowKind::OursChanged, "{page}");
    assert!(r[0].diffs.iter().all(|d| d.field != "Difficulty"), "no difficulty line: {:?}", r[0].diffs);
    assert!(!page.contains("Yours now: none"), "{page}");
    apply(&mut m, &s, &mut banks, &ctx, &mut acct);
    assert_eq!(sent_one(&mut m, &banks), json!("High"));
}

/// The read overwrites, also a rating of hers the circle does not have (the circle's "none" in version 3) and one she cleared:
/// after the read the copy is the circle's; a change she makes after it is hers, counted and kept.
#[test]
fn c91_the_read_overwrites_then_her_later_change_is_hers() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, None, Some(Difficulty::Low));
    s.keep_raw("Get", &version_path(3), v3(Some("High")));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!((banks[0].questions[0].difficulty, banks[0].questions[1].difficulty), (Some(Difficulty::High), None));
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 0, 0));
    // her deliberate change after the read
    banks[0].questions[0].difficulty = Some(Difficulty::Low);
    banks[0].modified += 5;
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::Low), "never overwritten again: {strip}");
    assert_eq!(changes(&mut m, &banks[0].clone()), (0, 1, 0), "{strip}");
    assert_eq!(version_reads(&s, 3), 1);
}

/// The bank changed by the read is saved, and its time moves on (an export preview made before is made again), while a send
/// whose answer could not be read is not said to have unsent changes after it.
#[test]
fn c91_i1_the_overwritten_bank_is_saved_and_its_time_moves_on() {
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    let before = banks[0].modified;
    m.edit.update_link(LOCAL, |l| {
        l.sent_unread = Some(super::circle_links::SentUnread { by: String::new(), modified: before });
        true
    });
    s.keep_raw("Get", &version_path(3), v3(Some("High")));
    let who = olga_named(&s);
    m.edit.frame(now(), &who);
    assert!(!m.edit.sync(&mut banks), "the read is taken at the next frame");
    m.edit.frame(now(), &who);
    assert!(m.edit.sync(&mut banks), "the banks are to be saved");
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High));
    assert!(banks[0].modified > before, "{} > {before}", banks[0].modified);
    assert_eq!(m.edit.sent_unread_of(&banks[0].clone()), Some(false), "nothing of hers changed after that send");
    m.edit.frame(now(), &who);
    assert!(!m.edit.sync(&mut banks), "once");
}

/// (c91 I1, P1) One's editor was open (its draft taken) when the read landed. The editor is told: Done writes the circle's
/// rating, never her old one back. A rating she chose in the editor herself is hers.
#[test]
fn c91_i1_an_open_question_editor_does_not_undo_the_circles_rating() {
    for chose in [None, Some(Difficulty::Medium)] {
        let (mut m, s, mut banks, ctx, mut acct) = rig();
        s.keep("Post", "/circles/heads", 200, heads(3));
        phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
        let mut draft = crate::qedit::Draft::from_question(LOCAL.into(), &banks[0].questions[0]);
        let mut other = crate::qedit::Draft::from_question(LOCAL.into(), &banks[0].questions[1]);
        if let Some(d) = chose {
            draft.difficulty = Some(d);
        }
        s.keep_raw("Get", &version_path(3), v3(Some("High")));
        let who = olga_named(&s);
        let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
        let moved = m.edit.take_inherited();
        refresh_draft(Some(&mut draft), &moved);
        refresh_draft(Some(&mut other), &moved);
        assert!(m.edit.take_inherited().is_empty(), "told once");
        // Done: the draft is written whole
        let q = draft.build_question(Some(&banks[0].questions[0]));
        banks[0].questions[0] = q;
        banks[0].modified += 5;
        let strip2 = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
        assert_eq!(banks[0].questions[0].difficulty, chose.or(Some(Difficulty::High)), "{strip} / {strip2}");
        assert_eq!(other.difficulty, None, "another question's draft is left alone");
        let mine = if chose.is_some() { 1 } else { 0 };
        assert_eq!(changes(&mut m, &banks[0].clone()), (0, mine, 0), "{strip2}");
    }
}

/// The editor of another bank, or a new question, is never touched.
#[test]
fn c91_i1_only_that_questions_draft_is_refreshed() {
    let (bank, _) = linked(&[(QA, "One")]);
    let q = bank.questions[0].clone();
    let moved = vec![Inherited { local: LOCAL.into(), qid: q.id.clone(), before: None, after: Some(Difficulty::High) }];
    let mut other_bank = crate::qedit::Draft::from_question("another".into(), &q);
    let mut fresh = crate::qedit::Draft::new(LOCAL.into());
    refresh_draft(Some(&mut other_bank), &moved);
    refresh_draft(Some(&mut fresh), &moved);
    assert_eq!((other_bank.difficulty, fresh.difficulty), (None, None));
    let mut same = crate::qedit::Draft::from_question(LOCAL.into(), &q);
    refresh_draft(Some(&mut same), &moved);
    assert_eq!(same.difficulty, Some(Difficulty::High));
    refresh_draft(None, &moved);
}

// ---- until the read has succeeded: nothing is sent, nothing is up to date ---------------------------------------------------

/// Frames of the model alone (no strip, so no check of the circle is asked) as `who` at `at`.
fn quiet(m: &mut CirclesModel, banks: &mut [QuestionBank], who: &AccountSnapshot, at: std::time::Instant, n: usize) {
    for _ in 0..n {
        m.edit.frame(at, who);
        m.edit.sync(banks);
    }
}

fn wait_reads(s: &Arc<Script>, n: u64, want: usize) {
    for _ in 0..200 {
        if version_reads(s, n) >= want {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("not read: {:?}", s.paths());
}

fn press(ctx: &egui::Context, m: &mut CirclesModel, banks: &mut [QuestionBank], acct: &mut FakeAccount, who: &AccountSnapshot, label: &str) {
    let mut b = browse();
    assert!(super::circles_phasec_tests::click_text(ctx, 1280.0, label, 0, &mut |ui| {
        super::bank_links_ui::strip_with(ui, m, banks, &mut b, acct, who, now(), NOW, LOCAL);
    }), "no {label}");
}

/// (c91 P4) The circle had a problem a moment ago: version 3 could not be read. The strip says so with Try again, Send is
/// neither offered nor opened, and the badge does not count. Try again reads it: the copy then has Bob's rating, and Send is
/// offered for her own change, carrying his rating.
#[test]
fn c91_p4_nothing_is_sent_before_the_read() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, None, None);
    banks[0].questions[1].stem = "Two (mine)".into();
    banks[0].modified += 30;
    s.on("Get", &version_path(3), 503, json!({"error": "unavailable"}));
    s.on("Get", &version_path(3), 503, json!({"error": "unavailable"}));
    s.keep_raw("Get", &version_path(3), v3(Some("High")));
    let who = olga_named(&s);
    // (round 16, critic 94 m2) a first failure is still "checking", asked again within seconds; the second is said
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(version_reads(&s, 3), 1, "{strip}");
    assert!(strip.contains("Checking the circle...") && !strip.contains("Couldn't check"), "{strip}");
    let strip = (0..3).map(|_| page_frame(&mut m, &mut banks, &mut acct, &who, now() + Duration::from_secs(6)).1).last().unwrap();
    assert_eq!(version_reads(&s, 3), 2, "{strip}");
    assert!(strip.contains(super::circles_text::CANT_CHECK) && strip.contains("Try again"), "{strip}");
    assert!(!strip.contains("Send my changes") && !strip.contains("up to date") && !strip.contains("unsent change"), "{strip}");
    // (round 18, critic 98 minor 8) said as what it is
    assert_eq!(badge(&mut m, &banks[0].clone()), super::bank_links::WAITING_NOTE);
    assert!(!m.edit.open_send(LOCAL, owner()), "no Send page before the read");
    press(&ctx, &mut m, &mut banks, &mut acct, &who, "Try again");
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(version_reads(&s, 3), 3, "{strip}");
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High), "{strip}");
    assert!(strip.contains("You have 1 unsent change") && strip.contains("Send my changes"), "{strip}");
    // (round 17, critic 96 I1) with the news of the circle's difficulty beside it
    assert_eq!(badge(&mut m, &banks[0].clone()), "difficulty updated · 1 unsent");
    assert!(m.edit.open_send(LOCAL, owner()));
    assert_eq!(sent_one(&mut m, &banks), json!("High"));
}

/// (c91 P3, I4) A link the round 12 build wrote (its base filled from the bank), its version 4 no longer kept: never "up to
/// date", the difficulties said to be unknown, Check now offered and no Send; the same half an hour later, when it is asked
/// again and still can't be read.
#[test]
fn c91_p3_a_version_no_longer_kept_is_never_up_to_date() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(4));
    phase_c(&mut m, &mut banks, 4, Some(Difficulty::High), None);
    let b = banks[0].clone();
    m.edit.update_link(LOCAL, |l| {
        for (bq, q) in l.base.iter_mut().zip(&b.questions) {
            bq.snap.difficulty = Some(q.difficulty);
        }
        true
    });
    super::bank_links_fix12_tests::not_kept(&s, 4);
    let who = olga_named(&s);
    let mut reads = 0;
    for later in [0, 31 * 60] {
        let at = now() + Duration::from_secs(later);
        let mut strip = String::new();
        for _ in 0..3 {
            strip = page_frame(&mut m, &mut banks, &mut acct, &who, at).1;
        }
        assert!(!strip.contains("up to date") && !strip.contains("Send my changes"), "{later}: {strip}");
        // (round 17, critic 96 minor 1) why sending is off, and what she can do, that first
        // (round 18, critic 98 minor 3) said once, as the circle's newest version
        assert!(strip.contains(&super::circles_text::cant_read_line(4, Unread::NotKept, true)) && strip.contains("Use my bank as it is..."), "{later}: {strip}");
        assert_eq!(badge(&mut m, &banks[0].clone()), "changes not counted");
        assert!(!m.edit.open_send(LOCAL, owner()));
        assert!(version_reads(&s, 4) > reads, "asked again after a while");
        reads = version_reads(&s, 4);
    }
    let _ = ctx;
}

/// The read on its way: "Checking the circle...", no Send, the badge does not count; nothing is sent from the Send page either.
#[test]
fn c91_the_strip_says_it_is_checking_while_the_read_is_on_its_way() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, None, None);
    background(&mut m);
    s.keep_raw("Get", &version_path(3), v3(Some("High")));
    let who = olga_named(&s);
    let release = s.hold();
    quiet(&mut m, &mut banks, &who, now(), 1);
    wait_reads(&s, 3, 1);
    let strip = strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
    assert!(strip.contains("Checking the circle...") && !strip.contains("Send my changes") && !strip.contains("up to date"), "{strip}");
    // (round 18, critic 98 minor 8) said as what it is
    assert_eq!(badge(&mut m, &banks[0].clone()), super::bank_links::WAITING_NOTE);
    assert!(!m.edit.open_send(LOCAL, owner()));
    release.send(()).unwrap();
    for _ in 0..200 {
        quiet(&mut m, &mut banks, &who, now(), 1);
        if !m.edit.busy() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let strip = strip_as(&ctx, &mut m, &mut banks, &mut acct, &who, LOCAL);
    assert!(strip.contains("up to date"), "{strip}");
}

/// (critic 91 minor) The quiet read asks by this account's role: a role another account on this computer saw is not this one's.
/// An editor's role seen by another account, this account a viewer: never read.
#[test]
fn c91_the_quiet_read_goes_by_this_accounts_role() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    phase_c(&mut m, &mut banks, 3, None, None);
    m.edit.update_link(LOCAL, |l| {
        l.role_seen = Some("editor".into());
        l.role_seen_by = Some("another account".into());
        true
    });
    s.keep_raw("Get", &version_path(3), v3(Some("High")));
    let who = olga_named(&s);
    quiet(&mut m, &mut banks, &who, now(), 3);
    assert_eq!(version_reads(&s, 3), 0, "{:?}", s.paths());
    // (nothing asked yet: the copy still waits for it, so nothing is sent from it)
    assert_eq!(m.edit.status_of(&banks[0].clone()).unwrap().learning, Some(Learning::Checking));
    // this account's check: a viewer here
    s.keep("Post", "/circles/heads", 200, heads(3));
    s.keep("Get", "/circles", 200, circles_list("viewer"));
    m.links.check_now(LOCAL);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 4);
    assert_eq!(m.edit.with_link(LOCAL, |l| l.role_seen.clone()).unwrap().as_deref(), Some("viewer"));
    assert_eq!(version_reads(&s, 3), 0, "{:?}", s.paths());
}

/// (critic 91 minor) Get latest waiting for the quiet read on its way says so, and says when the circle can't be reached.
#[test]
fn c91_get_latest_waiting_for_the_read_says_why() {
    let (mut m, s, mut banks, _ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(4));
    phase_c(&mut m, &mut banks, 3, None, None);
    background(&mut m);
    s.keep_raw("Get", &version_path(3), v3(Some("High")));
    let who = olga_named(&s);
    let t0 = now();
    let release = s.hold();
    quiet(&mut m, &mut banks, &who, t0, 1);
    wait_reads(&s, 3, 1);
    s.on("Post", &format!("{}/merge", put_path()), 200, bobs_four());
    s.keep_raw("Get", &put_path(), v4(Some("High")));
    assert!(m.edit.open_merge(LOCAL, owner()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    for _ in 0..200 {
        quiet(&mut m, &mut banks, &who, t0, 1);
        if matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::Ready(_))) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let page = page_frame(&mut m, &mut banks, &mut acct, &who, t0 + Duration::from_secs(1)).1;
    assert!(page.contains("Checking the circle...") && !page.contains("Couldn't check") && !page.contains("Still checking"), "{page}");
    // (round 16, critic 94 m2) a read that is only slow is still checking; one that failed says so (see fix16)
    let page = page_frame(&mut m, &mut banks, &mut acct, &who, t0 + Duration::from_secs(20)).1;
    assert!(page.contains(super::circles_text::STILL_CHECKING) && !page.contains("Couldn't check"), "{page}");
    release.send(()).unwrap();
}

// ---- critic 92 I3: one wording and one order on every card ---------------------------------------------------------------------

/// The lines of `page` that read exactly `label` (a radio button's), by line number.
fn at_each(page: &str, label: &str) -> Vec<usize> {
    page.lines().enumerate().filter(|(_, l)| l.trim() == label).map(|(i, _)| i).collect()
}

/// Every card offers "Keep mine" first, then the other side's version under one label, `n` cards.
fn keep_then_take(page: &str, take: &str, n: usize) {
    let (keep, take_at) = (at_each(page, "Keep mine"), at_each(page, take));
    assert_eq!((keep.len(), take_at.len()), (n, n), "{page}");
    for i in 0..n {
        assert!(keep[i] < take_at[i] && take_at.get(i).is_some_and(|t| keep.get(i + 1).is_none_or(|k| t < k)), "card {i}: {page}");
    }
}

/// (c92 I3) A review with an undecided "Changed in the circle" card and a "Difficulty not recorded" card: both list Keep mine
/// first, then "Take the circle's version" (never "Use ..."), and neither has anything chosen.
#[test]
fn c92_i3_every_undecided_card_has_the_same_options_in_the_same_order() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    phase_c(&mut m, &mut banks, 3, None, None);
    super::bank_links_fix12_tests::not_kept(&s, 3);
    let page = get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served_qs(4, vec![rated(QA, "One (b)", Some("High")), rated(QB, "Two", Some("Low"))], false));
    let r = rows(&m);
    assert_eq!((r[0].kind, r[0].choice, r[0].options.clone()), (RowKind::TheirsChanged, Choice::Undecided, vec![Choice::Keep, Choice::Take]), "{page}");
    assert_eq!((r[1].kind, r[1].unrecorded, r[1].choice, r[1].options.clone()), (RowKind::Conflict, true, Choice::Undecided, vec![Choice::Ours, Choice::Theirs]), "{page}");
    assert!(!page.contains("Use the circle's"), "{page}");
    keep_then_take(&page, "Take the circle's version", 2);
}

/// (c92 I3) A plain change in the circle still starts from the circle's version (nothing for the teacher to decide), with the
/// options in the same order; a conflict's "Keep both" stays last.
#[test]
fn c92_i3_a_plain_change_keeps_its_default_in_the_same_order() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    let page = get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served_qs(4, vec![rated(QA, "One (b)", None)], false));
    let r = rows(&m);
    assert_eq!((r[0].kind, r[0].choice, r[0].options.clone()), (RowKind::TheirsChanged, Choice::Take, vec![Choice::Keep, Choice::Take]), "{page}");
    keep_then_take(&page, "Take the circle's version", 1);
    // both changed: Keep mine, Take the circle's version, Keep both
    banks[0].questions[0].stem = "One (a)".into();
    banks[0].modified += 5;
    m.edit.cancel_merge();
    let page = get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served_qs(4, vec![rated(QA, "One (b)", None)], false));
    assert_eq!(rows(&m)[0].kind, RowKind::Conflict, "{page}");
    keep_then_take(&page, "Take the circle's version", 1);
    assert!(at_each(&page, "Take the circle's version")[0] < at_each(&page, "Keep both (the circle's is added as a second question)")[0], "{page}");
}

// ---- critic 92 I2: what leaving does to the banks in My Banks ------------------------------------------------------------------

/// (c92 I2) "Leave this circle?" says what happens to the banks in My Banks, in My Banks words, and names each bank of this
/// circle with unsent changes, with its count; with nothing unsent there is no such line. Another circle's bank is not named.
#[test]
fn c92_i2_leaving_says_what_happens_to_my_banks_and_names_the_unsent() {
    for (unsent, role) in [(1, "editor"), (0, "editor"), (2, "owner"), (1, "viewer")] {
        let (mut m, _s) = open_circle(role, json!({}));
        let (mut bank, mut l) = linked(&[(QA, "One"), (QB, "Two")]);
        l.role_seen = Some(role.into());
        m.edit.add_link(l).unwrap();
        for q in bank.questions.iter_mut().take(unsent) {
            q.stem.push_str(" (mine)");
        }
        let (mut other, mut lo) = linked(&[(QC, "Three")]);
        other.id = "other-bank".into();
        other.name = "Plants".into();
        other.questions[0].stem.push_str(" (mine)");
        lo.local_bank_id = other.id.clone();
        lo.circle_id = hex(99);
        lo.circle_bank_id = hex(98);
        lo.role_seen = Some("editor".into());
        m.edit.add_link(lo).unwrap();
        let mut banks = vec![bank, other];
        m.edit.sync(&mut banks);
        m.ask_leave();
        let t = text_of(760.0, &mut |ui| {
            super::circles_ui::body_of(ui, &mut m);
        });
        assert!(t.contains("Leave this circle?"), "{t}");
        assert!(t.contains("Banks you added to My Banks stay there") && !t.contains("Copies you already made"), "{t}");
        let named = match (unsent, role) {
            (_, "viewer") | (0, _) => None,
            (1, _) => Some("\"Unit 2 quiz\" has 1 unsent change. Send it first if the circle should have it."),
            _ => Some("\"Unit 2 quiz\" has 2 unsent changes. Send them first if the circle should have them."),
        };
        match named {
            Some(n) => assert!(t.contains(n), "{unsent} {role}: {t}"),
            None => assert!(!t.contains("unsent change"), "{unsent} {role}: {t}"),
        }
        assert!(!t.contains("Plants"), "another circle's bank: {t}");
    }
}

// ---- critic 91's survivors ------------------------------------------------------------------------------------------------------

/// (survivor f_unreadable_always) The quiet read on its way when Get latest was pressed met a problem of the circle's own (503):
/// the review made after it does not take that for a version that can't be read. Once the page is left the read is asked again
/// a minute later, not half an hour later.
#[test]
fn s15_a_503_on_the_read_get_latest_waited_for_is_asked_again_after_a_minute() {
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    phase_c(&mut m, &mut banks, 3, None, None);
    background(&mut m);
    s.on("Get", &version_path(3), 503, json!({"error": "unavailable"}));
    s.keep_raw("Get", &version_path(3), v3(Some("High")));
    let who = olga_named(&s);
    let t0 = now();
    let release = s.hold();
    quiet(&mut m, &mut banks, &who, t0, 1);
    wait_reads(&s, 3, 1);
    s.on("Post", &format!("{}/merge", put_path()), 200, bobs_four());
    s.keep_raw("Get", &put_path(), v4(None));
    assert!(m.edit.open_merge(LOCAL, owner()));
    release.send(()).unwrap();
    for _ in 0..200 {
        quiet(&mut m, &mut banks, &who, t0, 1);
        if kinds(&m).is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(kinds(&m).is_some(), "{:?}", m.edit.view);
    m.edit.cancel_merge();
    quiet(&mut m, &mut banks, &who, t0 + Duration::from_secs(30), 2);
    assert_eq!(version_reads(&s, 3), 1, "not at once");
    quiet(&mut m, &mut banks, &who, t0 + Duration::from_secs(61), 1);
    wait_reads(&s, 3, 2);
}

/// A version the circle would not show (no longer kept) whose bank's state then changes: asked again at once, whether it is
/// the newest version known (survivor b_state_no_head: the time it was sent unchanged) or the person's role (b_state_no_role).
#[test]
fn s15_an_unreadable_version_is_asked_again_when_the_newest_version_or_the_role_changes() {
    for what in ["head", "role"] {
        let (mut m, s, mut banks, _ctx, _acct) = rig();
        phase_c(&mut m, &mut banks, 3, None, None);
        super::bank_links_fix12_tests::not_kept(&s, 3);
        let who = olga_named(&s);
        quiet(&mut m, &mut banks, &who, now(), 3);
        assert_eq!(version_reads(&s, 3), 1, "{what}");
        quiet(&mut m, &mut banks, &who, now(), 2);
        assert_eq!(version_reads(&s, 3), 1, "{what}: not again while nothing changed");
        m.edit.update_link(LOCAL, |l| {
            match what {
                "head" => l.head_seen = 4,
                _ => l.role_seen = Some(if l.role_seen.as_deref() == Some("owner") { "editor" } else { "owner" }.into()),
            }
            true
        });
        quiet(&mut m, &mut banks, &who, now(), 2);
        assert_eq!(version_reads(&s, 3), 2, "{what}");
    }
}

/// (survivor s_inherit_after_status) The pass that gives the copy the circle's rating counts after it: what the circle page shows
/// (the count as last worked out) is already the new one.
#[test]
fn s15_the_count_after_the_pass_is_the_new_one() {
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    phase_c(&mut m, &mut banks, 3, Some(Difficulty::Low), None);
    s.keep_raw("Get", &version_path(3), v3(Some("High")));
    let who = olga_named(&s);
    quiet(&mut m, &mut banks, &who, now(), 2);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High));
    let c = m.edit.status(LOCAL).unwrap().changes;
    assert!(c.known && c.total() == 0, "{c:?}");
}

/// (survivor s_merge_not_saved) Bank B takes the circle's rating in the same pass as A's Get latest is applied, and A's apply is
/// held back (A changed under the review): the pass still says the banks changed, so B is saved.
#[test]
fn s15_a_pass_that_gives_one_bank_its_rating_beside_another_banks_merge_saves() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    get_latest(&mut m, &s, &mut banks, &ctx, &mut acct, served_qs(4, vec![rated(QA, "One (b)", None)], false));
    // B: as an earlier build left it, its version 1 rating Two "High"
    let bid_b = hex(77);
    m.edit.update_link(super::bank_links_fix4_tests::LOCAL_B, |l| {
        for b in &mut l.base {
            b.snap.difficulty = None;
        }
        l.difficulty_read = None;
        true
    });
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{bid_b}/versions/1"), served_qs(1, vec![rated(QB, "Two", Some("High"))], false));
    s.keep_raw("Get", &put_path(), served_qs(4, vec![rated(QA, "One (b)", None)], false));
    let who = olga_named(&s);
    // the frame asks for B's version (its answer is taken at the next one), then Apply merge (the same)
    m.edit.frame(now(), &who);
    assert!(!m.edit.sync(&mut banks));
    m.edit.apply_merge();
    banks[0].questions[0].stem = "One (a)".into();
    banks[0].modified += 5;
    m.edit.frame(now(), &who);
    assert!(m.edit.sync(&mut banks), "B changed: the banks are to be saved");
    assert_eq!(banks[1].questions[0].difficulty, Some(Difficulty::High));
    assert_eq!(banks[0].questions[0].stem, "One (a)", "A's merge was not applied: its review is made again");
    assert!(matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::Ready(_))));
}

/// (survivor g4) The read asked again a minute after the circle could not be reached is on its way: "Checking the circle...", not
/// "Can't check" from the last answer.
#[test]
fn s15_a_read_asked_again_says_it_is_checking() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, None, None);
    background(&mut m);
    s.on("Get", &version_path(3), 503, json!({"error": "unavailable"}));
    s.on("Get", &version_path(3), 503, json!({"error": "unavailable"}));
    s.keep_raw("Get", &version_path(3), v3(Some("High")));
    let who = olga_named(&s);
    let t0 = now();
    // (round 16, critic 94 m2: the second failure in a row is the one said)
    for (i, at) in [t0, t0 + Duration::from_secs(6)].into_iter().enumerate() {
        quiet(&mut m, &mut banks, &who, at, 1);
        wait_reads(&s, 3, i + 1);
        for _ in 0..200 {
            quiet(&mut m, &mut banks, &who, at, 1);
            if !m.edit.busy() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    assert_eq!(m.edit.status_of(&banks[0].clone()).unwrap().learning, Some(Learning::CantReach));
    let release = s.hold();
    quiet(&mut m, &mut banks, &who, t0 + Duration::from_secs(6 + 61), 1);
    wait_reads(&s, 3, 3);
    assert_eq!(m.edit.status_of(&banks[0].clone()).unwrap().learning, Some(Learning::Checking));
    let _ = (&ctx, &mut acct);
    release.send(()).unwrap();
}

/// The count is said again as it is when the copy stops waiting for the read for another reason than the read (the person is
/// now a viewer): a round 12 build's copy (its difficulties filled from the bank) counts what she changed, as before.
#[test]
fn s15_a_copy_no_longer_waiting_is_counted_again() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, None, None);
    m.edit.update_link(LOCAL, |l| {
        for b in &mut l.base {
            b.snap.difficulty = Some(None);
        }
        true
    });
    super::bank_links_fix12_tests::not_kept(&s, 3);
    edited_two(&mut banks);
    let who = olga_named(&s);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert!(strip.contains("Use my bank as it is..."), "{strip}");
    m.edit.update_link(LOCAL, |l| {
        l.role_seen = Some("viewer".into());
        true
    });
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 2);
    assert!(strip.contains("You changed 1 question here") && !strip.contains("up to date"), "{strip}");
}

fn edited_two(banks: &mut [QuestionBank]) {
    banks[0].questions[1].stem = "Two (mine)".into();
    banks[0].modified += 30;
}

/// (survivor o6) A send from an earlier build's copy whose answer did not say what it made; the read then gives the copy the
/// circle's ratings; the check finds the person's own version after it. Nothing she changed after that send is said to be
/// unsent (the read is not her change), so Send is not offered over her own version.
#[test]
fn s15_the_read_after_a_send_with_no_clear_answer_is_not_a_change_since() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    // (round 16: the send carried none for One, so One takes the circle's rating; one it carried rated is kept, see fix16)
    phase_c(&mut m, &mut banks, 3, None, None);
    let b = banks[0].clone();
    m.edit.update_link(LOCAL, |l| {
        l.sent_record = Some(super::circle_links::sent_record(l, &b, 3, String::new()));
        true
    });
    s.keep_raw("Get", &version_path(3), v3(Some("High")));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High));
    s.keep("Post", "/circles/heads", 200, heads_by(4, "Olga Teacher"));
    m.links.check_now(LOCAL);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert!(m.edit.sent_unread(LOCAL), "{strip}");
    assert!(strip.contains("Your changes were sent") && !strip.contains("Changes you made since") && !strip.contains("Send my changes"), "{strip}");
}

/// (survivor g12b) "Check now" in the strip's "..." menu asks for the circle's version again at once, like Try again.
#[test]
fn s15_check_now_in_the_menu_reads_again() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    s.keep("Post", "/circles/heads", 200, heads(3));
    phase_c(&mut m, &mut banks, 3, None, None);
    s.on("Get", &version_path(3), 503, json!({"error": "unavailable"}));
    s.on("Get", &version_path(3), 503, json!({"error": "unavailable"}));
    s.keep_raw("Get", &version_path(3), v3(Some("High")));
    let who = olga_named(&s);
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    let strip = (0..3).map(|_| page_frame(&mut m, &mut banks, &mut acct, &who, now() + Duration::from_secs(6)).1).last().unwrap();
    assert!(strip.contains("Try again"), "{strip}");
    press(&ctx, &mut m, &mut banks, &mut acct, &who, "...");
    press(&ctx, &mut m, &mut banks, &mut acct, &who, "Check now");
    frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
    assert_eq!(version_reads(&s, 3), 3, "{:?}", s.paths());
    assert_eq!(banks[0].questions[0].difficulty, Some(Difficulty::High));
}

/// (survivor g15) Whatever else the strip says before the read (the circle's newest version held back for review, a send whose
/// answer could not be read), it offers no Send.
#[test]
fn s15_no_send_is_offered_from_any_strip_before_the_read() {
    for how in ["held back", "sent, changed since"] {
        let (mut m, s, mut banks, ctx, mut acct) = rig();
        phase_c(&mut m, &mut banks, 3, None, None);
        super::bank_links_fix12_tests::not_kept(&s, 3);
        edited_two(&mut banks);
        let who = olga_named(&s);
        if how == "held back" {
            s.keep("Post", "/circles/heads", 200, json!({"banks": [{"bank_id": BID, "status": "under_review"}]}));
            m.links.check_now(LOCAL);
        } else {
            s.keep("Post", "/circles/heads", 200, heads(3));
            m.edit.update_link(LOCAL, |l| {
                l.sent_unread = Some(super::circle_links::SentUnread { by: String::new(), modified: 1 });
                true
            });
        }
        let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 3);
        assert!(strip.contains(if how == "held back" { "held back for review" } else { "Changes you made since" }), "{how}: {strip}");
        assert!(!strip.contains("Send my changes"), "{how}: {strip}");
    }
}

/// (survivor g17) While the circle is checked the strip shows it is working.
#[test]
fn s15_the_checking_strip_is_busy() {
    use super::bank_links::{Check, SignIn, StripInput, strip_view};
    let (mut m, _s, mut banks, _ctx, _acct) = rig();
    phase_c(&mut m, &mut banks, 3, None, None);
    let st = m.edit.status_of(&banks[0].clone()).unwrap();
    assert_eq!(st.learning, Some(Learning::Checking));
    let l = m.edit.link_for_local(LOCAL).unwrap();
    let v = strip_view(&StripInput { link: &l, status: Some(&st), check: Check::Done, sign_in: SignIn::Ready, now_ms: NOW, act: None, sent_unread: false, changed_since: false, unsure: false });
    assert!(v.busy && v.line.contains("Checking the circle...") && !v.buttons.contains(&super::bank_links::StripButton::Send), "{v:?}");
}

/// (survivors g20, g20b) The circle page's row of such a copy says the same as its strip: checking, or the circle can't be
/// reached (nothing sendable), never "Nothing to send.".
#[test]
fn c91_the_circle_page_row_says_the_same_as_the_strip() {
    for answer in [None, Some(503)] {
        let (mut m, s) = open_circle("editor", json!({}));
        let (bank, mut l) = linked(&[(QA, "One")]);
        l.role_seen = Some("editor".into());
        for b in &mut l.base {
            b.snap.difficulty = None;
        }
        l.difficulty_read = None;
        m.edit.add_link(l).unwrap();
        let mut banks = vec![bank];
        let mut b = browse();
        // (not asked yet: no frame of a signed-in account; or asked, and the circle answered 503)
        if let Some(st) = answer {
            s.keep("Get", &format!("/circles/{CID}/banks/{BID}/versions/1"), st, json!({"error": "unavailable"}));
        }
        // (round 16, critic 94 m2: the second failure in a row is the one said)
        for k in 0..6u64 {
            if answer.is_some() {
                m.frame(now() + Duration::from_secs(if k < 3 { 0 } else { 6 }), &ready(&s));
            }
            m.sync(&mut banks, &mut b);
        }
        let t = text_of(1280.0, &mut |ui| {
            super::circles_ui::body_of(ui, &mut m);
        });
        let want = if answer.is_some() { super::circles_text::CANT_CHECK } else { "Checking the circle..." };
        assert!(t.contains(want) && !t.contains("Nothing to send."), "{answer:?}: {t}");
    }
}

/// (survivor g25) The version it last matched no longer kept, and the circle has a newer one: Get latest first (it settles the
/// copy), no Send, and the strip says so.
#[test]
fn s15_not_kept_and_behind_offers_get_latest_first() {
    let (mut m, s, mut banks, ctx, mut acct) = rig();
    phase_c(&mut m, &mut banks, 3, None, None);
    super::bank_links_fix12_tests::not_kept(&s, 3);
    edited_two(&mut banks);
    s.keep("Post", "/circles/heads", 200, heads(4));
    m.links.check_now(LOCAL);
    let who = olga_named(&s);
    // (the newer version, once known, has the version 3 asked for again: then still not kept)
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 5);
    assert!(strip.contains("You have version 3 and 1 unsent change.") && strip.contains("Get the latest first, then send."), "{strip}");
    assert!(strip.contains("Get latest") && !strip.contains("Send my changes") && !strip.contains("Sending is turned off"), "{strip}");
}

/// (survivor g24) A Get latest that waits for nothing of the kind (its answers in, its review not made yet) does not say it is
/// checking the circle.
#[test]
fn s15_a_merge_not_waiting_for_the_read_says_nothing_of_it() {
    let (mut m, s, mut banks, _ctx, _acct) = rig();
    s.on("Post", &format!("{}/merge", put_path()), 200, merge_start(3, 4));
    s.keep_raw("Get", &put_path(), served_qs(4, vec![rated(QA, "One (b)", None)], false));
    assert!(m.edit.open_merge(LOCAL, owner()));
    let who = olga_named(&s);
    for _ in 0..5 {
        m.edit.frame(now(), &who);
        if matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::Ready(_))) {
            break;
        }
    }
    assert!(matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::Ready(_))), "{:?}", m.edit.view);
    assert_eq!(m.edit.merge_wait_note(), None);
    m.edit.sync(&mut banks);
    assert!(kinds(&m).is_some());
}
