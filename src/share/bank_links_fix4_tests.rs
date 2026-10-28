//! Phase D1, round 4 (critics 69 and 70): each test written first against the code it pins. A restore or trim the
//! person was told to wait for, left by Back, the Circles tab or another bank's page, is kept and its outcome said on
//! the bank's strip; one bank's notice is never drawn on another bank's strip and never hides the wait note.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::json;

use super::bank_links_fix_tests::*;
use super::bank_links_tests::*;
use super::browse::{BrowseModel, ImportLog};
use super::circle_edit::*;
use super::circle_links::*;
use super::circles::CirclesModel;
use super::circles_api::*;
use super::circles_tests::*;
use super::model::Runner;
use super::pool::*;
use crate::model::QuestionBank;

pub(super) const LOCAL_B: &str = "local-bank-b";

pub(super) fn browse() -> BrowseModel {
    BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), ImportLog::memory())
}

pub(super) fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    ctx
}

/// One frame of `draw` on a context that lives across calls (its frame count goes on, as the app's does); the text.
pub(super) fn frame_text(ctx: &egui::Context, draw: &mut dyn FnMut(&mut egui::Ui)) -> String {
    let t0 = ctx.input(|i| i.time);
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 6000.0))), time: Some(t0 + 0.05), ..Default::default() };
    let out = ctx.run_ui(input, |ui| {
        egui::CentralPanel::default().show(ui, |ui| draw(ui));
    });
    let mut all = String::new();
    for s in &out.shapes {
        if let egui::epaint::Shape::Text(t) = &s.shape {
            // (round 18) a non-breaking space keeps "Easy -> Hard" on one line: read as a space
            all.push_str(&t.galley.text().replace('\u{a0}', " "));
            all.push('\n');
        }
    }
    out.drop_without_applying_deltas();
    all
}

/// One frame of a linked bank's page in My Banks (the strip, or the circle's screen drawn in place of the questions).
pub(super) fn strip(ctx: &egui::Context, m: &mut CirclesModel, banks: &mut [QuestionBank], acct: &mut FakeAccount, s: &Arc<Script>, local: &str) -> String {
    let mut b = browse();
    let snap = ready(s);
    frame_text(ctx, &mut |ui| {
        super::bank_links_ui::strip_with(ui, m, banks, &mut b, acct, &snap, now(), NOW, local);
    })
}

/// Frames of `local`'s page until `done` (a background job's answer came); the last frame's text.
pub(super) fn until(ctx: &egui::Context, m: &mut CirclesModel, banks: &mut [QuestionBank], acct: &mut FakeAccount, s: &Arc<Script>, local: &str, done: &dyn Fn(&CirclesModel) -> bool) -> String {
    for _ in 0..500 {
        let t = strip(ctx, m, banks, acct, s, local);
        if done(m) {
            return t;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("timed out: {:?}", m.edit.view);
}

pub(super) fn owner() -> Perm {
    Perm { role: Role::Owner, is_author: true, closed: false }
}

pub(super) fn editor() -> Perm {
    Perm { role: Role::Editor, is_author: false, closed: false }
}

pub(super) fn versions() -> String {
    format!("/circles/{CID}/banks/{BID}/versions")
}

pub(super) fn page(head: u64, lines: &[u64]) -> serde_json::Value {
    let ls: Vec<serde_json::Value> = lines.iter().map(|v| json!({"version": v, "version_id": hex(*v), "by": "Mr Keesler", "at": NOW - DAY * (*v as i64), "message": "", "added": 0, "changed": 1, "removed": 0, "question_count": 1, "license": "CC-BY-4.0", "kept": true, "withheld": false})).collect();
    json!({"bank_id": BID, "head_version": head, "oldest_kept": 1, "pinned_versions": [], "versions": ls, "next_before": null})
}

pub(super) fn rolled(version: u64, from: u64) -> serde_json::Value {
    json!({"circle_id": CID, "bank_id": BID, "version": version, "version_id": hex(version), "qids": [QA], "added": 0, "changed": 1, "removed": 0, "rollback_of": from})
}

/// Two linked banks: A ("Unit 2 quiz", the owner's, at version 3 and up to date) and B ("Plants").
pub(super) fn two_at_three() -> (CirclesModel, Arc<Script>, Vec<QuestionBank>) {
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, heads(3));
    s.keep("Get", "/circles", 200, circles_list("owner"));
    let mut m = model();
    let (a, mut la) = linked(&[(QA, "One")]);
    la.base_version = 3;
    la.head_seen = 3;
    la.role_seen = Some("owner".into());
    la.author_known = true;
    la.is_author = true;
    let (mut b, mut lb) = linked(&[(QB, "Two")]);
    b.id = LOCAL_B.into();
    b.name = "Plants".into();
    lb.local_bank_id = LOCAL_B.into();
    lb.circle_bank_id = hex(77);
    lb.role_seen = Some("owner".into());
    lb.author_known = true;
    lb.mark_bank(&b);
    m.edit.add_link(la).unwrap();
    m.edit.add_link(lb).unwrap();
    (m, s, vec![a, b])
}

/// The same links in a model whose jobs run on their own threads (a request can be held at the server).
pub(super) fn background(m: &mut CirclesModel) {
    let media: super::publish::SharedMedia = Arc::new(HashMap::<String, Vec<u8>>::new());
    let mut e = CircleEditModel::new(Runner::Background, Arc::new(Mem::default()), media, Arc::new(NoDiskRestore), LinkLog::memory()).with_retry(RetryPolicy::none());
    e.set_clock(NOW);
    for l in m.edit.links().to_vec() {
        e.add_link(l).unwrap();
    }
    m.edit = e;
}

/// A's History, asked for on A's page (drawn there in place of the questions).
pub(super) fn history_on_a(m: &mut CirclesModel) {
    assert!(m.edit.open_history(CID, BID, "Unit 2 quiz", owner()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    // (as the strip's History button does: drawn from this frame on, not a History left behind earlier)
    m.edit.host_frame = None;
}

pub(super) fn loaded(m: &CirclesModel) -> bool {
    matches!(&m.edit.view, EditView::History(h) if h.loaded)
}

// ---- c69 I1: a restore or trim the person was told to wait for ----------------------------------------------------------

#[test]
fn i1_a_restore_answered_while_the_person_waits_on_another_bank_is_said_on_its_strip() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("Version 3 is the latest. Your copy is up to date."), "{t}");
    history_on_a(&mut m);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("Restore this version..."), "{t}");
    m.edit.ask_restore(1);
    s.on("Post", &format!("/circles/{CID}/banks/{BID}/rollback"), 200, rolled(4, 1));
    s.on("Get", &versions(), 200, page(4, &[4, 3, 2, 1]));
    m.edit.confirm_restore(false);
    // The person goes to bank B and waits there while the answer comes.
    for _ in 0..3 {
        let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B);
        assert!(!t.contains("restored"), "B's strip says nothing of A: {t}");
    }
    // Back on A: the History left behind is not drawn again, and the strip says how the restore went.
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(!t.contains("History: \""), "{t}");
    assert!(t.contains("Version 4 was restored from version 1"), "{t}");
    assert!(t.contains("Version 4 is the latest") && !t.contains("up to date"), "{t}");
}

#[test]
fn i1_b_back_while_a_restore_is_on_its_way_keeps_it_and_the_strip_says_so_then_how_it_went() {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &loaded);
    m.edit.ask_restore(1);
    s.on("Post", &format!("/circles/{CID}/banks/{BID}/rollback"), 200, rolled(4, 1));
    s.on("Get", &versions(), 200, page(4, &[4, 3, 2, 1]));
    let release = s.hold();
    m.edit.confirm_restore(false);
    assert!(m.edit.acting());
    let mut b = browse();
    let snap = ready(&s);
    assert!(super::circles_phasec_tests::click_text(&ctx, 1280.0, "< Back to Unit 2 quiz", 0, &mut |ui| {
        super::bank_links_ui::strip_with(ui, &mut m, &mut banks, &mut b, &mut acct, &snap, now(), NOW, LOCAL);
    }));
    assert!(!m.edit.active(), "the History closed");
    assert!(m.edit.acting(), "the restore is kept");
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("Restoring version 1 in the circle") && !t.contains("up to date"), "{t}");
    release.send(()).unwrap();
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| !m.edit.acting());
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("Version 4 was restored from version 1") && t.contains("Version 4 is the latest") && !t.contains("up to date"), "{t}");
}

#[test]
fn i1_c_opening_the_circles_tab_while_a_restore_is_on_its_way_keeps_it() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    m.edit.ask_restore(1);
    s.on("Post", &format!("/circles/{CID}/banks/{BID}/rollback"), 200, rolled(4, 1));
    s.on("Get", &versions(), 200, page(4, &[4, 3, 2, 1]));
    m.edit.confirm_restore(false);
    // (its answer is not read yet) The Circles tab opens.
    m.open_list();
    assert!(!m.edit.active());
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("Version 4 was restored from version 1") && t.contains("Version 4 is the latest") && !t.contains("up to date"), "{t}");
}

#[test]
fn i1_d_a_trim_left_by_back_says_how_it_went() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    m.edit.open_trim();
    m.edit.set_trim_keep(2);
    m.edit.ask_trim();
    s.on("Post", &format!("/circles/{CID}/banks/{BID}/trim"), 200, json!({"ok": true, "pruned": 1, "oldest_kept": 2, "pinned_versions": []}));
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    m.edit.confirm_trim();
    m.edit.back();
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("The trim deleted the content of 1 older version."), "{t}");
}

#[test]
fn i1_e_a_restore_that_failed_while_the_person_was_away_says_so() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    m.edit.ask_restore(1);
    s.on("Post", &format!("/circles/{CID}/banks/{BID}/rollback"), 503, json!({"error": "unavailable"}));
    m.edit.confirm_restore(false);
    m.edit.back();
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("The restore of version 1 did not go through") && t.contains("Open History to try again"), "{t}");
    assert!(t.contains("Version 3 is the latest"), "{t}");
}

#[test]
fn i1_f_another_account_is_not_told_how_the_first_ones_restore_went() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    m.edit.ask_restore(1);
    s.on("Post", &format!("/circles/{CID}/banks/{BID}/rollback"), 200, rolled(4, 1));
    s.on("Get", &versions(), 200, page(4, &[4, 3, 2, 1]));
    m.edit.confirm_restore(false);
    m.edit.forget_account(true);
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(!t.contains("restored") && !t.contains("Restoring"), "{t}");
}

// ---- c69 I2: one bank's notice, and the wait note ---------------------------------------------------------------------

/// The bank's copy was put back from a saved copy: "Use my bank as it is" is offered.
pub(super) fn make_stale(m: &mut CirclesModel, local: &str) {
    m.edit.update_link(local, |l| {
        l.bank_modified += 100;
        l.bank_print = "something else".into();
        true
    });
}

/// "Use my bank as it is" on `local`, answered and confirmed (the circle has nothing newer).
pub(super) fn treat_in_step(m: &mut CirclesModel, s: &Arc<Script>, banks: &mut [QuestionBank], local: &str, bid: &str, wait: bool) {
    s.on("Post", &format!("/circles/{CID}/banks/{bid}/merge"), 400, json!({"error": "invalid_request", "field": "base_version"}));
    m.edit.frame(now(), &ready(s));
    m.edit.sync(banks);
    assert!(m.edit.open_merge_to_rebase(local, owner()));
    m.edit.host = Some(EditHost { local_id: local.into(), back: "< Back".into() });
    for _ in 0..500 {
        m.edit.frame(now(), &ready(s));
        m.edit.sync(banks);
        if matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::StaleUpToDate { ask: Some(StaleAsk::Rebase) })) {
            break;
        }
        if wait {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    m.edit.rebaseline();
    m.edit.sync(banks);
    assert!(m.edit.results_of(local).iter().any(|(_, n)| n.contains("now used as it is")), "{:?}", m.edit.results_of(local));
    m.edit.close();
}

#[test]
fn i2_a_one_banks_notice_is_drawn_on_its_own_strip_only_and_goes_once_seen() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    make_stale(&mut m, LOCAL);
    treat_in_step(&mut m, &s, &mut banks, LOCAL, BID, false);
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B);
    assert!(!t.contains("now used as it is"), "not on B: {t}");
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("now used as it is"), "on A: {t}");
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B);
    // (round 5, critic 71 I1b) another bank's page never takes it away: it stays until dismissed
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("now used as it is") && t.contains("Dismiss"), "still said once another bank was opened: {t}");
    let mut b = browse();
    let snap = ready(&s);
    assert!(super::circles_phasec_tests::click_text(&ctx, 1280.0, "Dismiss", 0, &mut |ui| {
        super::bank_links_ui::strip_with(ui, &mut m, &mut banks, &mut b, &mut acct, &snap, now(), NOW, LOCAL);
    }));
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(!t.contains("now used as it is"), "gone once dismissed: {t}");
}

#[test]
fn i2_b_a_banks_own_notice_does_not_hide_the_wait_note() {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B);
    // B's own notice.
    make_stale(&mut m, LOCAL_B);
    treat_in_step(&mut m, &s, &mut banks, LOCAL_B, &hex(77), true);
    // A send from A on its way, held at the server.
    banks[0].questions[0].stem = "One (my fix)".into();
    banks[0].modified += 5;
    assert!(m.edit.open_send(LOCAL, editor()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| matches!(&m.edit.view, EditView::Send(v) if matches!(v.licence, LicenceState::Known(_)) && !matches!(v.partial, PartialSend::Reading) && v.changes.is_some()));
    m.edit.set_attest(true);
    s.on("Put", &format!("/circles/{CID}/banks/{BID}"), 201, json!({"circle_id": CID, "bank_id": BID, "version": 4, "version_id": hex(4), "qids": [QA], "added": 0, "changed": 1, "removed": 0}));
    let release = s.hold();
    m.edit.send();
    m.edit.sync(&mut banks);
    assert!(m.edit.acting());
    let mut b = browse();
    let snap = ready(&s);
    assert!(super::circles_phasec_tests::click_text(&ctx, 1280.0, "History", 0, &mut |ui| {
        super::bank_links_ui::strip_with(ui, &mut m, &mut banks, &mut b, &mut acct, &snap, now(), NOW, LOCAL_B);
    }));
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B);
    assert!(t.contains("Wait for the send from \"Unit 2 quiz\" to finish"), "{t}");
    assert!(t.contains("now used as it is"), "{t}");
    release.send(()).unwrap();
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| !m.edit.acting());
}

#[test]
fn i1_g_a_restore_answered_while_an_older_version_is_looked_at_is_shown_on_the_way_back() {
    let (mut m, s, mut banks) = two_at_three();
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    let e = &mut m.edit;
    e.frame(now(), &ready(&s));
    assert!(e.open_history(CID, BID, "Unit 2 quiz", owner()));
    e.frame(now(), &ready(&s));
    e.ask_restore(1);
    s.on("Post", &format!("/circles/{CID}/banks/{BID}/rollback"), 200, rolled(4, 1));
    e.confirm_restore(false);
    // "Look at it" on version 2 before the answer is read.
    s.on_raw("Get", &format!("/circles/{CID}/banks/{BID}/versions/2"), served(2, &[(QA, "One")], false));
    e.view_version(2);
    e.frame(now(), &ready(&s));
    e.sync(&mut banks);
    assert!(!e.acting());
    s.on("Get", &versions(), 200, page(4, &[4, 3, 2, 1]));
    e.back();
    e.frame(now(), &ready(&s));
    let EditView::History(h) = &e.view else { panic!("{:?}", e.view) };
    assert!(matches!(h.restore.as_ref().map(|r| &r.stage), Some(RestoreStage::Done { version: 4, .. })), "{:?}", h.restore);
    assert_eq!(h.head_version, 4, "read again");
    assert_eq!(e.link_for_local(LOCAL).unwrap().head_seen, 4);
}

// ---- c69 minors -------------------------------------------------------------------------------------------------------

pub(super) fn bank_reads(s: &Arc<Script>) -> usize {
    s.paths().iter().filter(|p| p.starts_with(&format!("Get /circles/{CID}/banks/"))).count()
}

pub(super) fn rows(m: &mut CirclesModel) -> String {
    text_of(1280.0, &mut |ui| super::bank_links_ui::adding_rows_of(ui, m))
}

pub(super) fn add_state(m: &CirclesModel, bid: &str) -> AddState {
    m.add_state(CID, bid).map(|j| j.state.clone()).expect("an add")
}

/// App frames away from the Shared page: the app's tick, then the browse window's frame (which has the banks).
pub(super) fn ticks(m: &mut CirclesModel, snap: &super::publish::AccountSnapshot, banks: &mut Vec<QuestionBank>, b: &mut BrowseModel, n: usize) -> usize {
    let mut busy = 0;
    for _ in 0..n {
        if m.tick_background(now(), snap) {
            busy += 1;
        }
        super::circles::land_waiting(banks, b);
    }
    busy
}

use super::circles::AddState;

#[test]
fn m1_another_accounts_try_again_does_not_run_the_first_accounts_add() {
    let (mut m, s) = open_circle("editor", json!({}));
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(2, &[(QA, "One")], false));
    let (olga, vera) = (as_who(&s, "olga"), as_who(&s, "vera"));
    m.note_account(&olga);
    m.add_bank(BID, false);
    // vera, a viewer of the same circle, signs in on this computer before it was made, and opens the circle.
    m.note_account(&vera);
    assert!(matches!(add_state(&m, BID), AddState::Failed(_)));
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("viewer", json!({})));
    m.open_circle(CID);
    for _ in 0..4 {
        m.frame(now(), &vera);
    }
    let t = text_of(1280.0, &mut |ui| {
        super::circles_ui::body_of(ui, &mut m);
    });
    assert!(t.contains("Another account signed in before it was finished"), "{t}");
    assert!(!t.contains("Try again"), "no Try again where it can not help: {t}");
    // Asked anyway: nothing runs for vera, and nothing is made with olga's role.
    m.retry_add(CID, BID);
    let reads = bank_reads(&s);
    let (mut banks, mut b) = (Vec::new(), browse());
    ticks(&mut m, &vera, &mut banks, &mut b, 6);
    for _ in 0..3 {
        m.frame(now(), &vera);
        m.sync(&mut banks, &mut b);
    }
    assert_eq!(bank_reads(&s), reads, "{:?}", s.paths());
    assert!(banks.is_empty());
    assert!(matches!(add_state(&m, BID), AddState::Failed(_)));
}

#[test]
fn m2_unlink_while_a_send_from_the_bank_is_on_its_way_does_not_say_its_changes_are_stranded() {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    banks[0].questions[0].stem = "One (my fix)".into();
    banks[0].modified += 5;
    assert!(m.edit.open_send(LOCAL, editor()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| matches!(&m.edit.view, EditView::Send(v) if matches!(v.licence, LicenceState::Known(_)) && !matches!(v.partial, PartialSend::Reading) && v.changes.is_some()));
    m.edit.set_attest(true);
    s.on("Put", &format!("/circles/{CID}/banks/{BID}"), 201, json!({"circle_id": CID, "bank_id": BID, "version": 4, "version_id": hex(4), "qids": [QA], "added": 0, "changed": 1, "removed": 0}));
    let release = s.hold();
    m.edit.send();
    m.edit.sync(&mut banks);
    m.edit.close();
    assert!(m.edit.acting());
    m.edit.ask_forget(LOCAL);
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("Unlink? The bank stays here as it is") && t.contains("A send from this bank is on its way"), "{t}");
    assert!(!t.contains("can then never be sent"), "{t}");
    release.send(()).unwrap();
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| !m.edit.acting());
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("Unlink? The bank stays here as it is") && !t.contains("A send from this bank is on its way"), "{t}");
}

#[test]
fn m3_a_copy_put_into_the_banks_is_linked_by_the_shared_page_whatever_it_shows() {
    let (mut m, s) = open_circle("editor", json!({}));
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(2, &[(QA, "One")], false));
    let a = as_who(&s, "olga");
    m.note_account(&a);
    m.add_bank(BID, false);
    let (mut banks, mut b) = (Vec::new(), browse());
    m.tick_background(now(), &a);
    m.tick_background(now(), &a);
    assert!(super::circles::land_waiting(&mut banks, &mut b), "in the banks (the app saves them)");
    // The Shared page is drawn next, while it shows a step of the sign-in (not its tabs).
    let mut sh = super::shared_ui::SharedUi::default();
    sh.circles = m;
    let mut acct = super::ui::AccountUi::in_page();
    acct.model = super::model::AccountModel::new(Arc::new(|_| panic!("no client may be built")), Runner::Inline, super::model::SettingsIo::memory()).with_server_mode(super::model::server_mode(None, false));
    let (mut bu, mut mi): (super::BrowseUi, super::MineUi) = Default::default();
    text_of(1280.0, &mut |ui| {
        sh.draw(ui, &mut acct, &mut bu, &mut mi, &mut banks, &[]);
    });
    assert!(sh.circles.edit.link_for_local(&banks[0].id).is_some(), "the bank is not left without its link");
}

#[test]
fn m3_adds_not_finished_when_verhsion_closed_are_said_on_the_next_start() {
    let log = super::circles::AddsLog::memory();
    let (mut m, s) = open_circle("editor", json!({}));
    m.set_adds_log(log.clone());
    let a = as_who(&s, "olga");
    m.note_account(&a);
    m.add_bank(BID, false);
    assert!(!rows(&mut m).contains("closed before"), "not on this start");
    drop(m);
    // The next start.
    let mut m = model();
    m.set_adds_log(log.clone());
    let t = rows(&mut m);
    assert!(t.contains("verSHion closed before 1 bank you asked to add was finished. Add it again from the circle's page."), "{t}");
    assert!(!t.contains("Cells"), "no title: another account may be using this computer now: {t}");
    let ctx = ctx();
    assert!(super::circles_phasec_tests::click_text(&ctx, 1280.0, "Dismiss", 0, &mut |ui| super::bank_links_ui::adding_rows_of(ui, &mut m)));
    assert!(!rows(&mut m).contains("closed before"));
    // And the start after that does not say it again.
    let mut m = model();
    m.set_adds_log(log);
    assert!(!rows(&mut m).contains("closed before"));
}

#[test]
fn m4_a_sign_out_after_the_copy_was_read_does_not_read_it_again() {
    let (mut m, s) = open_circle("editor", json!({}));
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(2, &[(QA, "One")], false));
    let a = as_who(&s, "olga");
    m.note_account(&a);
    m.add_bank(BID, false);
    let (mut banks, mut b) = (Vec::new(), browse());
    // read, and handed to the banks' side
    m.tick_background(now(), &a);
    m.tick_background(now(), &a);
    let out = super::publish::AccountSnapshot::of_gate(super::publish::Gate::SignedOut);
    assert_eq!(ticks(&mut m, &out, &mut banks, &mut b, 5), 0, "nothing keeps the app awake while signed out");
    assert!(banks.is_empty(), "nothing put in while signed out");
    ticks(&mut m, &a, &mut banks, &mut b, 8);
    assert_eq!(banks.len(), 1);
    assert!(matches!(add_state(&m, BID), AddState::Done { linked: true, .. }), "{:?}", m.adds);
    assert_eq!(bank_reads(&s), 1, "read once: {:?}", s.paths());
}

#[test]
fn m5_the_sign_in_row_goes_once_the_same_account_signs_in_again() {
    let (mut m, s) = open_circle("editor", json!({}));
    let a = as_who(&s, "olga");
    m.note_account(&a);
    m.add_bank(BID, false);
    s.fail(super::api::ApiError::SignedOut);
    let (mut banks, mut b) = (Vec::new(), browse());
    ticks(&mut m, &a, &mut banks, &mut b, 4);
    assert!(rows(&mut m).contains("Sign in to finish adding"));
    // Signed in again in the Account window: for a moment the account is looked up, then the same one is back.
    let looking = super::publish::AccountSnapshot::of_gate(super::publish::Gate::Checking);
    ticks(&mut m, &looking, &mut banks, &mut b, 1);
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(2, &[(QA, "One")], false));
    ticks(&mut m, &a, &mut banks, &mut b, 6);
    assert_eq!(banks.len(), 1);
    assert!(!rows(&mut m).contains("Sign in to finish"));
}

#[test]
fn m6_the_failed_adds_of_another_account_show_no_titles() {
    let two = json!([
        {"id": hex(201), "title": "Unit 1", "head_version": 1, "question_count": 1, "created_at": NOW, "updated_at": NOW},
        {"id": hex(202), "title": "Unit 2", "head_version": 1, "question_count": 1, "created_at": NOW, "updated_at": NOW}
    ]);
    let (mut m, s) = open_circle("editor", json!({"banks": two}));
    let (a, p) = (as_who(&s, "olga"), as_who(&s, "pete"));
    m.note_account(&a);
    m.add_bank(&hex(201), false);
    m.add_bank(&hex(202), false);
    m.note_account(&p);
    let t = rows(&mut m);
    assert!(!t.contains("Unit 1") && !t.contains("Unit 2"), "{t}");
    assert!(t.contains("2 banks that another account on this computer asked to add were not added."), "{t}");
    let ctx = ctx();
    assert!(super::circles_phasec_tests::click_text(&ctx, 1280.0, "Dismiss", 0, &mut |ui| super::bank_links_ui::adding_rows_of(ui, &mut m)));
    assert!(!rows(&mut m).contains("were not added"));
}

// ---- critic 69: the surviving mutants, pinned -------------------------------------------------------------------------

#[test]
fn n33_sign_in_on_the_banks_list_opens_the_account_window() {
    let (mut m, s) = open_circle("editor", json!({}));
    let a = as_who(&s, "olga");
    m.note_account(&a);
    m.add_bank(BID, false);
    s.fail(super::api::ApiError::SignedOut);
    let (mut banks, mut b) = (Vec::new(), browse());
    ticks(&mut m, &a, &mut banks, &mut b, 4);
    let mut acct = FakeAccount::default();
    let ctx = ctx();
    assert!(super::circles_phasec_tests::click_text(&ctx, 1280.0, "Sign in...", 0, &mut |ui| {
        super::bank_links_ui::adding_rows_with(ui, &mut m, &mut acct);
    }));
    assert_eq!(acct.reveals, 1, "the Account window is shown");
}

#[test]
fn n34_unlink_a_copy_whose_changes_can_not_be_counted_says_they_can_not_be_sent() {
    let (mut m, s, mut banks) = two_at_three();
    m.edit.update_link(LOCAL, |l| {
        l.base.clear();
        true
    });
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    m.edit.ask_forget(LOCAL);
    let (_, t) = page_frame(&mut m, &mut banks, &mut acct, &ready(&s), now());
    assert!(t.contains("Unlink? The bank stays here as it is") && t.contains("Changes you have not sent can then never be sent to the circle."), "{t}");
}

#[test]
fn n31_n32_a_row_waiting_for_a_sign_in_has_no_dismiss_and_a_failed_row_keeps_its_own() {
    use super::browse::Problem;
    let (mut m, s) = open_circle("editor", json!({}));
    let a = as_who(&s, "olga");
    m.note_account(&a);
    m.add_bank(BID, false);
    s.fail(super::api::ApiError::SignedOut);
    let (mut banks, mut b) = (Vec::new(), browse());
    ticks(&mut m, &a, &mut banks, &mut b, 4);
    let t = rows(&mut m);
    assert!(t.contains("Sign in to finish adding") && !t.contains("Dismiss"), "{t}");
    let gone = Problem { message: "That circle or bank is not available to you any more.".into(), retry: false, retry_at: None, account: false };
    m.adds.push(super::circles::AddJob { cid: CID.into(), bid: hex(9), own: false, title: "Gone bank".into(), role: Role::Editor, state: AddState::Failed(gone), full: false, dismissed: false, account: None });
    let t = rows(&mut m);
    assert!(t.contains("Sign in to finish adding") && t.contains("\"Gone bank\" couldn't be added"), "{t}");
    assert_eq!(t.matches("Dismiss").count(), 1, "the failed row only: {t}");
}

#[test]
fn n07_make_my_own_copy_while_an_add_of_the_same_bank_waits_is_not_queued_too() {
    let (mut m, s) = open_circle("editor", json!({}));
    let a = as_who(&s, "olga");
    m.note_account(&a);
    m.add_bank(BID, false);
    assert_eq!(add_state(&m, BID), AddState::Waiting);
    m.add_bank(BID, true);
    assert_eq!(m.adds.len(), 1, "one add of a circle bank at a time: {:?}", m.adds);
}

// ---- c70 I1: no "pool" on circle screens ------------------------------------------------------------------------------

/// "pool" as a word ("pool_migrating" is a code, not a word).
pub(super) fn says_pool(t: &str) -> bool {
    let l = t.to_lowercase();
    l.match_indices("pool").any(|(i, _)| {
        let before = l[..i].chars().next_back();
        let after = l[i + 4..].chars().next();
        !before.is_some_and(|c| c.is_alphanumeric() || c == '_') && !after.is_some_and(|c| c.is_alphanumeric() || c == '_')
    })
}

#[test]
fn w1_the_pools_words_read_as_verhsions_and_never_as_the_circles() {
    use super::circles_text::circle_words;
    let cases = [
        ("The pool had a problem of its own. Nothing was changed; try again in a minute.", "verSHion's server had a problem of its own. Nothing was changed; try again in a minute."),
        ("The pool does not allow that for this account.", "verSHion's server does not allow that for this account."),
        ("You have changed answer-key settings as many times as the pool allows today. Try again tomorrow.", "You have changed answer-key settings as many times as verSHion allows today. Try again tomorrow."),
        ("this question is in a form the pool does not take", "this question is in a form verSHion does not take"),
        ("That step is for the pool's administrators.", "That step is for verSHion's administrators."),
        ("The pool server is being set up and can not take this yet. Try again later.", "verSHion's server is being set up and can not take this yet. Try again later."),
        ("The pool is being upgraded. Try again in a few minutes.", "verSHion's server is being upgraded. Try again in a few minutes."),
        ("The pool can not take this as it is. Nothing was sent.", "verSHion's server can not take this as it is. Nothing was sent."),
    ];
    for (from, want) in cases {
        assert_eq!(circle_words(from), want);
    }
    assert_eq!(circle_words(&circle_words(cases[0].0)), cases[0].1, "said once, not mapped twice");
}

/// Every refusal the server can give, in the circle's words: no "pool", and never the circle blamed for the server.
#[test]
fn w2_no_server_refusal_says_pool_on_a_circle_screen() {
    use super::api::ErrorDetail;
    let mut texts: Vec<String> = Vec::new();
    let codes = ["server_error", "unavailable", "forbidden", "rate_limited", "daily_limit", "hourly_limit", "too_large", "invalid_payload", "image_refused", "pool_key_mismatch", "pool_migrating", "too_many_units", "author_records_exceeded", "imports_quota", "reads_quota", "evidence_quota_exceeded", "origin_not_allowed", "client_ip_required", "method_not_allowed", "admin_only", "invalid_request", "not_found", "conflict", "invite_required", "pool_not_configured", "something_new", ""];
    let metrics = [None, Some("circle_join_tries"), Some("hide_key_changes"), Some("hide_key_tries"), Some("keys_hidden_reads"), Some("circle_email_invites"), Some("invite_redeem")];
    for status in [400u16, 403, 404, 409, 413, 422, 429, 500, 502, 503] {
        for code in codes {
            for metric in metrics {
                let json = json!({"error": code, "metric": metric, "field": "email"});
                let detail = ErrorDetail { status, code: code.into(), field: Some("email".into()), reason: None, ack_allowed: None, author_only: false, retry_after: None, json };
                let kind = RefusalKind::classify(&detail);
                texts.push(PoolError::Refused(Box::new(Refusal { kind, detail })).message());
            }
            texts.push(other_message(status, code, String::new()));
        }
    }
    for t in texts {
        let c = super::circles_text::circle_words(&t);
        // (owner decision, round 7: "shared pool" too, the invite-code and not-set-up refusals included)
        assert!(!says_pool(&c), "{t} -> {c}");
        assert!(!c.contains("The circle had") && !c.contains("The circle does not allow") && !c.contains("the circle does not take"), "{t} -> {c}");
    }
}

/// The Rust string literals of `src` (comments and character literals skipped), with their line numbers.
pub(super) fn literals(src: &str) -> Vec<(usize, String)> {
    let b = src.as_bytes();
    let (mut i, mut line, mut out) = (0, 1, Vec::new());
    while i < b.len() {
        match b[i] {
            b'\n' => {
                line += 1;
                i += 1;
            }
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'\'' if b.get(i + 1) == Some(&b'\\') => {
                i += 3;
                while i < b.len() && b[i] != b'\'' {
                    i += 1;
                }
                i += 1;
            }
            b'\'' if b.get(i + 2) == Some(&b'\'') => i += 3,
            b'"' => {
                let at = line;
                let mut s = Vec::new();
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    if b[i] == b'\\' {
                        s.push(b[i]);
                        i += 1;
                    }
                    if b[i] == b'\n' {
                        line += 1;
                    }
                    s.push(b[i]);
                    i += 1;
                }
                i += 1;
                out.push((at, String::from_utf8_lossy(&s).into_owned()));
            }
            _ => i += 1,
        }
    }
    out
}

/// Words of the circle screens (the Circles tab, its bank pages and send, history and settings, the strip and badge
/// in My Banks, sharing a bank into a circle) never say "pool" (critic 70 I1: a teacher reads it as their private
/// circle bank going public). The public pool's own screens (browse, publish, mine, the Account window) say it on
/// purpose and are not scanned. Allowed here, each for a reason:
const POOL_ALLOWED: &[(&str, &str)] = &[
    // the circle bank's credits of questions first taken from the public pool: it names the real public pool
    ("circles_ui.rs", "items from the public pool"),
    // a topic written by a public-pool import, renamed for the circle when a merge applies it (data, never shown)
    ("circle_edit.rs", "From the pool: "),
];

#[test]
fn w3_no_circle_screen_says_pool() {
    let files: [(&str, &str); 13] = [
        ("circles.rs", include_str!("circles.rs")),
        ("circles_ui.rs", include_str!("circles_ui.rs")),
        ("circles_text.rs", include_str!("circles_text.rs")),
        ("circle_edit.rs", include_str!("circle_edit.rs")),
        ("circle_edit_ui.rs", include_str!("circle_edit_ui.rs")),
        ("circle_hide.rs", include_str!("circle_hide.rs")),
        ("circle_hide_ui.rs", include_str!("circle_hide_ui.rs")),
        ("circle_send.rs", include_str!("circle_send.rs")),
        ("circle_links.rs", include_str!("circle_links.rs")),
        ("bank_links.rs", include_str!("bank_links.rs")),
        ("bank_links_ui.rs", include_str!("bank_links_ui.rs")),
        ("circles_api.rs", include_str!("circles_api.rs")),
        ("circle_versions_api.rs", include_str!("circle_versions_api.rs")),
    ];
    let mut bad = Vec::new();
    for (name, src) in files {
        for (line, lit) in literals(src) {
            // (the mapping's own patterns, in `circle_words`)
            // (critic 78 I3: and the two whole sentences it says differently on a circle screen)
            let mapping = name == "circles_text.rs"
                && (["The shared pool", "the shared pool", "The pool server", "the pool server", "reach the pool", "The pool's", "the pool's", "The pool", "the pool", "The pool dropped this question", "The shared pool is not available on this server yet."].contains(&lit.as_str())
                    || lit.starts_with("The shared pool needs an invite code."));
            let allowed = mapping || POOL_ALLOWED.iter().any(|(f, a)| *f == name && lit.contains(a));
            if says_pool(&lit) && !allowed {
                bad.push(format!("{name}:{line}: {lit}"));
            }
        }
    }
    // (critic 72 I1) The merge review's words come from merge.rs, shared with the public pool's updates: on a circle's
    // review each goes through `merge_words` first, and then says no "pool" either (the topic a merge writes into the
    // bank is data, renamed for a circle when applied, and never drawn).
    for (line, lit) in literals(include_str!("merge.rs")) {
        let shown = super::circles_text::merge_words(&lit);
        let data = lit.starts_with("From the pool: ") || lit.starts_with("From the shared pool:");
        if says_pool(&shown) && !data {
            bad.push(format!("merge.rs:{line}: {lit} -> {shown}"));
        }
    }
    assert!(bad.is_empty(), "\"pool\" on a circle screen:\n{}", bad.join("\n"));
    // The scanner finds what it should.
    assert_eq!(literals("let a = \"the pool\"; // \"not this\"\nlet c = '\"'; let d = \"x\\\"pool\\\"\";").len(), 2);
}

#[test]
fn w4_one_server_error_reads_the_same_on_every_circle_screen() {
    let want = "verSHion's server had a problem of its own. Nothing was changed; try again in a minute.";
    // The Circles tab (here its list, as on Bank settings: the circles model's own problems).
    let s = Script::new();
    s.on("Get", "/circles", 500, json!({"error": "internal"}));
    let mut m = model();
    m.open_list();
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
    }
    let t = text_of(1280.0, &mut |ui| {
        super::circles_ui::body_of(ui, &mut m);
    });
    assert!(t.contains(want) && !says_pool(&t), "{t}");
    // A bank's History (the send, get latest and history screens).
    let (mut m, s, mut banks) = two_at_three();
    s.on("Get", &versions(), 500, json!({"error": "internal"}));
    m.edit.frame(now(), &ready(&s));
    assert!(m.edit.open_history(CID, BID, "Unit 2 quiz", owner()));
    m.edit.frame(now(), &ready(&s));
    m.edit.sync(&mut banks);
    let t = text_of(1280.0, &mut |ui| super::circle_edit_ui::draw_page(ui, &mut m));
    assert!(t.contains(want) && !says_pool(&t), "{t}");
    // The Banks list's row of an add that failed.
    let (mut m, s) = open_circle("editor", json!({}));
    s.on("Get", &format!("/circles/{CID}/banks/{BID}"), 500, json!({"error": "internal"}));
    m.add_bank(BID, false);
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
    }
    let t = rows(&mut m);
    assert!(t.contains(want) && !says_pool(&t), "{t}");
}

#[test]
fn w5_the_hide_key_question_on_bank_settings_does_not_say_pool() {
    use super::circles_text as text;
    for t in [text::HIDE_ON_ASK, text::HIDE_TRIES, text::PICTURES_CHANGE_DECIDE, text::HELD_BANK, text::REPORT_THANKS, text::EMPTY_BANKS_HELP_EDITOR, text::NOT_REACHED] {
        assert!(!says_pool(t), "{t}");
    }
    assert!(text::HIDE_ON_ASK.contains("verSHion first checks the viewers' copy"), "{}", text::HIDE_ON_ASK);
    assert!(text::HIDE_TRIES.contains("also when verSHion asks for decisions"), "{}", text::HIDE_TRIES);
}

// ---- round 4: the surviving mutants of this round's own code, pinned -------------------------------------------------

#[test]
fn r12_another_banks_notice_is_not_drawn_above_this_banks_screen() {
    let (mut m, s, mut banks) = two_at_three();
    make_stale(&mut m, LOCAL);
    treat_in_step(&mut m, &s, &mut banks, LOCAL, BID, false);
    // B's History, on B's page.
    s.on("Get", &format!("/circles/{CID}/banks/{}/versions", hex(77)), 200, page(1, &[1]));
    m.edit.frame(now(), &ready(&s));
    assert!(m.edit.open_history(CID, &hex(77), "Plants", owner()));
    m.edit.host = Some(EditHost { local_id: LOCAL_B.into(), back: "< Back to Plants".into() });
    m.edit.frame(now(), &ready(&s));
    let t = text_of(1280.0, &mut |ui| super::circle_edit_ui::draw_page(ui, &mut m));
    assert!(t.contains("History: \"Plants\"") && !t.contains("now used as it is"), "{t}");
}

#[test]
fn r23_an_add_another_account_asked_for_is_not_made_for_the_one_signed_in() {
    let (mut m, s) = open_circle("editor", json!({}));
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(2, &[(QA, "One")], false));
    let a = as_who(&s, "olga");
    m.note_account(&a);
    m.adds.push(super::circles::AddJob { cid: CID.into(), bid: BID.into(), own: false, title: "Cells".into(), role: Role::Editor, state: AddState::Waiting, full: false, dismissed: false, account: super::bank_links::account_tag(Some("https://pool.test\npete@school.org")) });
    let (mut banks, mut b) = (Vec::new(), browse());
    ticks(&mut m, &a, &mut banks, &mut b, 6);
    assert_eq!(bank_reads(&s), 0, "{:?}", s.paths());
    assert!(banks.is_empty());
}
