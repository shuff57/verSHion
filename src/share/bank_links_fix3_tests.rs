//! Phase D1, critics 67 and 68: the fixes, each test written first against the code it pins. Background adds and a
//! signed-out answer, a sign-out or another account in the middle of an add, a send under way on one bank while
//! another bank's strip is used; then the wording; then the surviving mutants of critic 67, pinned.

use std::sync::Arc;

use serde_json::json;

use super::api::ApiError;
use super::bank_links::*;
use super::bank_links_fix_tests::*;
use super::bank_links_tests::*;
use super::browse::{BrowseModel, ImportLog};
use super::circle_edit::*;
use super::circle_links::*;
use super::circles::{AddState, CirclesModel};
use super::circles_api::*;
use super::circles_tests::*;
use super::model::Runner;
use super::pool::*;
use super::publish::{AccountSnapshot, Gate};
use crate::model::QuestionBank;

fn bank_reads(s: &Arc<Script>) -> usize {
    s.paths().iter().filter(|p| p.starts_with(&format!("Get /circles/{CID}/banks/"))).count()
}

fn browse() -> BrowseModel {
    BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), ImportLog::memory())
}

fn rows(m: &mut CirclesModel) -> String {
    text_of(1280.0, &mut |ui| super::bank_links_ui::adding_rows_of(ui, m))
}

fn state(m: &CirclesModel, bid: &str) -> AddState {
    m.add_state(CID, bid).map(|j| j.state.clone()).expect("an add")
}

/// App frames away from the Shared page: the app's tick, then the browse window's frame (which has the banks).
fn ticks(m: &mut CirclesModel, snap: &AccountSnapshot, banks: &mut Vec<QuestionBank>, b: &mut BrowseModel, n: usize) -> usize {
    let mut busy = 0;
    for _ in 0..n {
        if m.tick_background(now(), snap) {
            busy += 1;
        }
        super::circles::land_waiting(banks, b);
    }
    busy
}

// ---- c67 I1: a "signed out" answer during a background add ------------------------------------------------------------

#[test]
fn i1_a_signed_out_answer_stops_the_add_and_the_banks_list_asks_to_sign_in() {
    let (mut m, s) = open_circle("editor", json!({}));
    let a = as_who(&s, "olga");
    m.note_account(&a);
    m.add_bank(BID, false);
    for _ in 0..40 {
        s.fail(ApiError::SignedOut);
    }
    let before = bank_reads(&s);
    let (mut banks, mut b) = (Vec::new(), browse());
    // The Account window still says signed in (it looks the account up again later).
    let busy = ticks(&mut m, &a, &mut banks, &mut b, 20);
    assert_eq!(bank_reads(&s) - before, 1, "asked once, not on every frame");
    assert!(busy <= 1, "the app is not kept awake: {busy}");
    assert_eq!(state(&m, BID), AddState::Waiting, "kept, to be made once signed in");
    let t = rows(&mut m);
    assert!(t.contains("Sign in to finish adding \"Cells\"") && t.contains("Sign in..."), "{t}");
    assert!(!t.contains("Adding \"Cells\""), "{t}");
    // The Shared page does not start it again either.
    for _ in 0..10 {
        m.frame(now(), &a);
        m.sync(&mut banks, &mut b);
    }
    assert_eq!(bank_reads(&s) - before, 1);
    // "Sign in...": the Account window opens, and the add is made once the read works.
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    assert!(super::circles_phasec_tests::click_text(&ctx, 1280.0, "Sign in...", 0, &mut |ui| super::bank_links_ui::adding_rows_of(ui, &mut m)));
    assert!(m.take_adds_sign_in(), "the Account window is asked for");
    assert!(!m.take_adds_sign_in(), "once");
    s.on_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(2, &[(QA, "One")], false));
    ticks(&mut m, &a, &mut banks, &mut b, 6);
    assert_eq!(banks.len(), 1);
    assert!(matches!(state(&m, BID), AddState::Done { linked: true, .. }), "{:?}", m.adds);
    assert!(!m.tick_background(now(), &a), "nothing left to do");
}

// ---- c67 I2: a sign-out, or another account, in the middle of an add -----------------------------------------------------

#[test]
fn i2_a_sign_out_keeps_the_waiting_add_and_the_same_account_finishes_it() {
    for at in ["waiting", "reading", "landing"] {
        let (mut m, s) = open_circle("editor", json!({}));
        s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(2, &[(QA, "One")], false));
        let a = as_who(&s, "olga");
        m.note_account(&a);
        m.add_bank(BID, false);
        let (mut banks, mut b) = (Vec::new(), browse());
        match at {
            // the read is done, its answer not yet taken
            "reading" => {
                m.tick_background(now(), &a);
            }
            // the copy was handed to the banks' side, not yet put in
            "landing" => {
                m.tick_background(now(), &a);
                m.tick_background(now(), &a);
            }
            _ => {}
        }
        let out = AccountSnapshot::of_gate(Gate::SignedOut);
        let reads = bank_reads(&s);
        let busy = ticks(&mut m, &out, &mut banks, &mut b, 5);
        assert_eq!(busy, 0, "{at}: nothing runs while signed out");
        assert!(banks.is_empty(), "{at}: nothing read for the account is put in while it is signed out");
        assert_eq!(bank_reads(&s), reads, "{at}");
        assert_eq!(state(&m, BID), AddState::Waiting, "{at}: kept");
        let t = rows(&mut m);
        assert!(t.contains("Sign in to finish adding \"Cells\""), "{at}: {t}");
        // The same account signs in again: the add is made, once.
        ticks(&mut m, &a, &mut banks, &mut b, 8);
        assert_eq!(banks.len(), 1, "{at}");
        assert!(matches!(state(&m, BID), AddState::Done { linked: true, .. }), "{at}: {:?}", m.adds);
        assert!(m.edit.link_for_local(&banks[0].id).is_some());
    }
}

#[test]
fn i2_another_account_runs_none_of_the_adds_and_the_banks_list_says_so() {
    let two = json!([
        {"id": hex(201), "title": "Unit 1", "head_version": 1, "question_count": 1, "created_at": NOW, "updated_at": NOW},
        {"id": hex(202), "title": "Unit 2", "head_version": 1, "question_count": 1, "created_at": NOW, "updated_at": NOW},
        {"id": hex(203), "title": "Unit 3", "head_version": 1, "question_count": 1, "created_at": NOW, "updated_at": NOW}
    ]);
    let (mut m, s) = open_circle("editor", json!({"banks": two}));
    for id in [hex(201), hex(202), hex(203)] {
        let mut doc: serde_json::Value = serde_json::from_slice(&served(1, &[(QA, "One")], false)).unwrap();
        doc["bank"]["id"] = json!(id);
        s.keep_raw("Get", &format!("/circles/{CID}/banks/{id}"), doc.to_string().into_bytes());
    }
    let (a, p) = (as_who(&s, "olga"), as_who(&s, "pete"));
    m.note_account(&a);
    for id in [hex(201), hex(202), hex(203)] {
        m.add_bank(&id, false);
    }
    let (mut banks, mut b) = (Vec::new(), browse());
    // The first is in My Banks; the second's read is done; then pete signs in.
    for _ in 0..8 {
        m.tick_background(now(), &a);
        super::circles::land_waiting(&mut banks, &mut b);
        if matches!(state(&m, &hex(201)), AddState::Done { .. }) {
            break;
        }
    }
    m.tick_background(now(), &a);
    assert_eq!(banks.len(), 1);
    let reads = bank_reads(&s);
    let busy = ticks(&mut m, &p, &mut banks, &mut b, 10);
    assert_eq!(bank_reads(&s), reads, "nothing is read for pete");
    assert_eq!(banks.len(), 1, "nothing lands for pete");
    assert_eq!(busy, 0);
    assert!(m.edit.link_for_local(&banks[0].id).is_some(), "olga's bank stays added and linked");
    assert!(m.add_state(CID, &hex(201)).is_none(), "what was said for olga goes");
    for id in [hex(202), hex(203)] {
        let AddState::Failed(pr) = state(&m, &id) else { panic!("{:?}", m.adds) };
        assert!(!pr.retry, "{pr:?}");
    }
    // (critic 69 minor 6) said to pete without the titles of olga's circle's banks
    let t = rows(&mut m);
    assert!(t.contains("2 banks that another account on this computer asked to add were not added."), "{t}");
    assert!(!t.contains("Unit 2") && !t.contains("Unit 3") && !t.contains("Adding") && !t.contains("Sign in to finish"), "{t}");
    // olga back: what failed stays failed (it is said, not made behind pete's back).
    ticks(&mut m, &a, &mut banks, &mut b, 6);
    assert_eq!(banks.len(), 1);
}

// ---- c67 I3: a send under way on one bank while another bank's strip is used ---------------------------------------------

fn editor() -> Perm {
    Perm { role: Role::Editor, is_author: false, closed: false }
}

const LOCAL_B: &str = "local-bank-b";

/// Two linked banks; A ("Unit 2 quiz") has one unsent change.
fn two_linked() -> (CirclesModel, Arc<Script>, Vec<QuestionBank>) {
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, heads(1));
    s.keep("Get", "/circles", 200, circles_list("editor"));
    let mut m = model();
    let (mut a, la) = linked(&[(QA, "One")]);
    let (mut b, mut lb) = linked(&[(QB, "Two")]);
    b.id = LOCAL_B.into();
    b.name = "Plants".into();
    lb.local_bank_id = LOCAL_B.into();
    lb.circle_bank_id = hex(77);
    lb.mark_bank(&b);
    m.edit.add_link(la).unwrap();
    m.edit.add_link(lb).unwrap();
    a.questions[0].stem = "One (my fix)".into();
    a.modified += 5;
    let banks = vec![a, b];
    m.edit.status_of(&banks[0]);
    m.edit.status_of(&banks[1]);
    (m, s, banks)
}

#[test]
fn i3_another_banks_page_does_not_open_while_a_send_is_on_its_way_and_the_send_lands() {
    let (mut m, s, mut banks) = two_linked();
    s.send_base_404(true);
    let e = &mut m.edit;
    e.frame(now(), &ready(&s));
    assert!(e.open_send(LOCAL, editor()));
    e.frame(now(), &ready(&s));
    e.set_attest(true);
    e.set_message("fix");
    e.frame(now(), &ready(&s));
    e.sync(&mut banks);
    e.frame(now(), &ready(&s));
    s.on("Put", &format!("/circles/{CID}/banks/{BID}"), 201, json!({"circle_id": CID, "bank_id": BID, "version": 2, "version_id": hex(2), "qids": [QA], "added": 0, "changed": 1, "removed": 0}));
    e.send();
    e.frame(now(), &ready(&s));
    e.sync(&mut banks);
    assert_eq!(s.paths().iter().filter(|p| p.starts_with("Put")).count(), 1, "{:?}", s.paths());
    assert!(e.acting());
    // The answer is on its way. On bank B the person presses Get latest, Send, History.
    let n = s.paths().len();
    assert!(!e.open_merge(LOCAL_B, editor()));
    assert!(!e.open_send(LOCAL_B, editor()));
    assert!(!e.open_history(CID, &hex(77), "Plants", editor()));
    assert!(matches!(&e.view, EditView::Send(v) if v.local_id == LOCAL), "A's send page stays");
    assert_eq!(s.paths().len(), n, "nothing is asked for B");
    let w = e.wait_note().expect("said why");
    assert!(w.contains("Wait for the send from \"Unit 2 quiz\" to finish"), "{w}");
    // The answer comes: A's link moves on and nothing is left unsent.
    e.frame(now(), &ready(&s));
    e.sync(&mut banks);
    assert_eq!(e.link_for_local(LOCAL).unwrap().base_version, 2);
    assert_eq!(e.status_of(&banks[0]).unwrap().changes.total(), 0);
    assert!(!e.acting() && e.wait_note().is_none());
    // Now B's page opens.
    s.keep("Post", &format!("/circles/{CID}/banks/{}/merge", hex(77)), 503, json!({"error": "unavailable"}));
    assert!(e.open_merge(LOCAL_B, editor()));
    assert!(matches!(&e.view, EditView::Merge(v) if v.local_id == LOCAL_B));
}

#[test]
fn n23_a_send_on_its_way_is_kept_when_the_screens_close_and_moves_the_link_on() {
    let (mut m, s, mut banks) = two_linked();
    s.send_base_404(true);
    let e = &mut m.edit;
    e.frame(now(), &ready(&s));
    assert!(e.open_send(LOCAL, editor()));
    e.frame(now(), &ready(&s));
    e.set_attest(true);
    e.frame(now(), &ready(&s));
    e.sync(&mut banks);
    e.frame(now(), &ready(&s));
    s.on("Put", &format!("/circles/{CID}/banks/{BID}"), 201, json!({"circle_id": CID, "bank_id": BID, "version": 2, "version_id": hex(2), "qids": [QA], "added": 0, "changed": 1, "removed": 0}));
    e.send();
    e.sync(&mut banks);
    assert!(e.acting());
    // (its answer not read yet) The Circles tab opens, or the person goes elsewhere: the screens close.
    e.close();
    assert!(e.acting(), "the send is kept");
    e.frame(now(), &ready(&s));
    e.sync(&mut banks);
    assert_eq!(e.link_for_local(LOCAL).unwrap().base_version, 2, "its answer moves the link on");
    assert_eq!(e.status_of(&banks[0]).unwrap().changes.total(), 0, "nothing shows as unsent");
}

#[test]
fn i3_a_restore_on_its_way_is_not_dropped_by_another_banks_page() {
    let (mut m, s, mut banks) = two_linked();
    let page = |head: u64, lines: &[u64]| {
        let ls: Vec<serde_json::Value> = lines.iter().map(|v| json!({"version": v, "version_id": hex(*v), "by": "Mr Keesler", "at": NOW - DAY * (*v as i64), "message": format!("note {v}"), "added": 0, "changed": 1, "removed": 0, "question_count": 1, "license": "CC-BY-4.0", "kept": true, "withheld": false})).collect();
        json!({"bank_id": BID, "head_version": head, "oldest_kept": 1, "pinned_versions": [], "versions": ls, "next_before": null})
    };
    s.on("Get", &format!("/circles/{CID}/banks/{BID}/versions"), 200, page(3, &[3, 2, 1]));
    let e = &mut m.edit;
    e.frame(now(), &ready(&s));
    let owner = Perm { role: Role::Owner, is_author: true, closed: false };
    assert!(e.open_history(CID, BID, "Cells", owner));
    e.frame(now(), &ready(&s));
    e.ask_restore(1);
    s.on("Post", &format!("/circles/{CID}/banks/{BID}/rollback"), 200, json!({"circle_id": CID, "bank_id": BID, "version": 4, "version_id": hex(4), "qids": [QA], "added": 0, "changed": 1, "removed": 0, "rollback_of": 1}));
    s.on("Get", &format!("/circles/{CID}/banks/{BID}/versions"), 200, page(4, &[4, 3, 2, 1]));
    e.confirm_restore(false);
    assert!(!e.open_merge(LOCAL_B, editor()));
    assert!(e.wait_note().is_some_and(|w| w.contains("Wait for the restore")), "{:?}", e.wait_note());
    e.frame(now(), &ready(&s));
    e.sync(&mut banks);
    let EditView::History(h) = &e.view else { panic!("{:?}", e.view) };
    assert!(matches!(h.restore.as_ref().map(|x| &x.stage), Some(RestoreStage::Done { version: 4, .. })), "the restore's answer is read");
}

// ---- critic 67 and 68 minors ------------------------------------------------------------------------------------------------

#[test]
fn m1_an_add_under_way_when_the_circles_model_closes_is_made_once() {
    for at in ["reading", "landing"] {
        let (mut m, s) = open_circle("editor", json!({}));
        s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(2, &[(QA, "One")], false));
        let a = as_who(&s, "olga");
        m.note_account(&a);
        m.add_bank(BID, false);
        m.tick_background(now(), &a);
        if at == "landing" {
            m.tick_background(now(), &a);
        }
        // (an invite code was redeemed: the Shared page closes the circles model)
        m.close();
        let (mut banks, mut b) = (Vec::new(), browse());
        ticks(&mut m, &a, &mut banks, &mut b, 10);
        assert_eq!(banks.len(), 1, "{at}: made, and once");
        assert!(matches!(state(&m, BID), AddState::Done { linked: true, .. }), "{at}: {:?}", m.adds);
        assert!(!rows(&mut m).contains("Adding"), "{at}");
    }
}

#[test]
fn m2_a_copy_without_keys_is_not_told_it_has_its_answer_keys() {
    let (bank, mut l) = linked_from(vec![qstripped(QA, "One"), qstripped(QB, "Two")], true);
    l.mode = LinkMode::Follow;
    l.viewers_form = true;
    l.role_seen = Some("editor".into());
    let s = Script::new();
    s.on_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served_qs(2, vec![qfull(QA, "One changed"), qfull(QB, "Two")], false));
    s.on_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served_qs(2, vec![qstripped(QA, "One changed"), qstripped(QB, "Two")], true));
    let mut m = edit_model();
    m.add_link(l.clone()).unwrap();
    let mut banks = vec![bank];
    m.frame(now(), &ready(&s));
    m.open_merge(LOCAL, perm_of(&l));
    turn(&mut m, &s, &mut banks, 3);
    m.apply_merge();
    turn(&mut m, &s, &mut banks, 3);
    let p = phase(&m);
    assert!(!p.contains("Your copy has its answer keys") && p.contains("Nothing was changed"), "{p}");
    assert!(p.contains("can't be applied"), "{p}");
    assert_eq!(banks[0].questions[0].stem, "One", "nothing applied");
}

#[test]
fn m3_unlink_names_the_unsent_changes_it_strands() {
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, heads(1));
    s.keep("Get", "/circles", 200, circles_list("editor"));
    for (n, want) in [(2, Some("Your 2 unsent changes can then never be sent to the circle.")), (1, Some("Your 1 unsent change can then never be sent to the circle.")), (0, None)] {
        let mut m = model();
        let (mut bank, l) = linked(&[(QA, "One"), (QB, "Two")]);
        for q in bank.questions.iter_mut().take(n) {
            q.stem.push_str(" (my fix)");
        }
        bank.modified += 5;
        m.edit.add_link(l).unwrap();
        let mut banks = vec![bank];
        let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
        m.edit.ask_forget(LOCAL);
        let (_, t) = page_frame(&mut m, &mut banks, &mut acct, &ready(&s), now());
        assert!(t.contains("Unlink? The bank stays here as it is"), "{t}");
        match want {
            Some(w) => assert!(t.contains(w), "{n}: {t}"),
            None => assert!(!t.contains("unsent"), "{n}: {t}"),
        }
    }
    // A copy that can not send strands nothing.
    assert!(!unlink_question(Some(3), false).contains("unsent"));
    assert!(unlink_question(None, true).contains("Changes you have not sent can then never be sent"));
}

#[test]
fn m4_no_pool_on_the_banks_list_the_send_page_and_the_share_window() {
    use super::browse::Problem;
    use super::circles::AddJob;
    // The Banks list: a failed add says the circles model's words without "pool".
    let mut m = model();
    let p = Problem { message: "The pool can not take this as it is. Nothing was sent.".into(), retry: false, retry_at: None, account: false };
    m.adds.push(AddJob { cid: CID.into(), bid: BID.into(), own: false, title: "Cells".into(), role: Role::Editor, state: AddState::Failed(p), full: false, dismissed: false, account: None });
    let t = rows(&mut m);
    // (critic 70 I1) verSHion's server, never the circle, for what the server did
    assert!(t.contains("\"Cells\" couldn't be added: verSHion's server can not take this as it is.") && !t.to_lowercase().contains("pool"), "{t}");
    // The circles model's own words reach that row: no "pool" there either.
    let (mut cm, s) = open_circle("editor", json!({}));
    s.on("Get", &format!("/circles/{CID}/banks/{BID}"), 503, json!({"error": "pool_migrating"}));
    cm.add_bank(BID, false);
    for _ in 0..3 {
        cm.frame(now(), &ready(&s));
    }
    let AddState::Failed(p) = state(&cm, BID) else { panic!("{:?}", cm.adds) };
    assert!(!p.message.to_lowercase().contains("pool"), "{}", p.message);
    // What the checks say about a question sent to a circle.
    for code in ["forbidden_char", "bad_image_file", "bad_license", "bad_id", "bad_form", "unknown_kind", "something_new"] {
        let i: Issue = serde_json::from_value(json!({"code": code, "path": "bank.questions[0].stem"})).unwrap();
        let t = super::publish_text::issue_text_circle(&i, false);
        assert!(!t.to_lowercase().contains("pool"), "{code}: {t}");
    }
    // The send page's refusals that are not mapped to circle words.
    let mut e = edit_model();
    let (bank, l) = linked(&[(QA, "One")]);
    e.add_link(l).unwrap();
    let mut banks = vec![bank];
    let s = Script::new();
    s.on("Get", &format!("/circles/{CID}/banks/{BID}/versions"), 422, json!({"error": "image_refused"}));
    e.frame(now(), &ready(&s));
    e.open_history(CID, BID, "Cells", Perm { role: Role::Editor, is_author: false, closed: false });
    turn(&mut e, &s, &mut banks, 2);
    let EditView::History(h) = &e.view else { panic!() };
    let msg = h.problem.as_ref().map(|p| p.message.clone()).unwrap_or_default();
    assert!(!msg.is_empty() && !msg.to_lowercase().contains("pool"), "{msg}");
}

fn share_into_circle(s: &Arc<Script>, snap: &AccountSnapshot, status: u16, reply: serde_json::Value) -> super::publish::PublishModel {
    use super::publish::{LogIo, PublishModel, ShareSource, Step};
    let b = json!({"id": "b1", "name": "Unit 1", "questions": [qjson("q0", "Zero"), qjson("q1", "One")], "created": 1, "modified": 2, "details": {}});
    let src = ShareSource::from_bank_json("bank:b1", "Bank: Unit 1", "Unit 1", &b.to_string());
    let mut p = PublishModel::new(Runner::Inline, Arc::new(std::collections::HashMap::<String, Vec<u8>>::new()), LogIo::memory()).with_retry(RetryPolicy::none());
    p.start(src);
    p.frame(now(), snap);
    assert_eq!(p.step, Step::Options);
    s.keep("Get", "/circles", 200, json!({"circles": [summary("editor", json!({"name": "Algebra team"}))]}));
    p.circle.activate(None);
    p.frame(now(), snap);
    p.frame(now(), snap);
    p.circle.choose(CID);
    p.circle.license = Some(License::CcBy);
    p.circle.attest = true;
    s.act("Post", "/banks", status, reply);
    p.send_to_circle();
    p.frame(now(), snap);
    p
}

fn publish_text(p: &mut super::publish::PublishModel) -> String {
    text_of(1280.0, &mut |ui| {
        super::publish_ui::page(ui, p);
    })
}

#[test]
fn m4_m7_the_share_window_speaks_of_the_circle_and_of_adding_to_my_banks() {
    let s = Script::new();
    let mut p = share_into_circle(&s, &ready(&s), 201, json!({"circle_id": CID, "bank_id": BID, "version": 1, "version_id": hex(1), "qids": [QA, QB], "added": 2, "changed": 0, "removed": 0, "replayed": true}));
    let t = publish_text(&mut p);
    assert!(t.contains("The circle already had this send, so nothing was added twice."), "{t}");
    assert!(t.contains("Its members can add it to My Banks or make their own copy"), "{t}");
    assert!(!t.contains("open it and get a copy"), "{t}");
    let s = Script::new();
    let mut p = share_into_circle(&s, &ready(&s), 422, json!({"error": "invalid_payload", "errors": [{"code": "forbidden_char", "path": "bank.questions[0].stem"}]}));
    let t = publish_text(&mut p);
    assert!(t.contains("verSHion checked these questions for the circle and needs you to change the items below. Nothing was shared."), "{t}");
    assert!(!t.contains("The pool checked") && !t.contains("the pool does not take"), "{t}");
}

#[test]
fn r42_sharing_into_a_circle_says_a_server_fault_as_every_circle_screen_does() {
    let s = Script::new();
    let mut p = share_into_circle(&s, &ready(&s), 500, json!({"error": "internal"}));
    let t = publish_text(&mut p);
    assert!(t.contains("verSHion's server had a problem of its own."), "{t}");
    assert!(!t.contains("The pool had") && !t.contains("The circle had"), "{t}");
}

#[test]
fn m5_the_send_page_does_not_argue_with_itself_when_no_question_is_named() {
    let mut m = model();
    let (bank, l) = linked(&[(QA, "One")]);
    m.edit.add_link(l).unwrap();
    let mut banks = vec![bank];
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, heads(1));
    s.keep("Get", "/circles", 200, circles_list("editor"));
    m.edit.view = EditView::Send(Box::new(send_view(SendStage::Payload(Box::default()))));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    let (_, t) = page_frame(&mut m, &mut banks, &mut acct, &ready(&s), now());
    assert!(t.contains("The circle did not accept this version, and verSHion's server did not say which question."), "{t}");
    assert!(!t.contains("the items below") && !t.contains("Change the questions in the bank"), "{t}");
    assert!(t.contains("Nothing was sent"), "{t}");
    // With questions named, the list and its lead stay.
    let i: Issue = serde_json::from_value(json!({"code": "too_long", "path": "bank.questions[0].stem", "limit": 8192})).unwrap();
    let pr = PayloadRefusal { issues: vec![i], ..PayloadRefusal::default() };
    m.edit.view = EditView::Send(Box::new(send_view(SendStage::Payload(Box::new(pr)))));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    let (_, t) = page_frame(&mut m, &mut banks, &mut acct, &ready(&s), now());
    assert!(t.contains("the items below") && !t.contains("did not say which question"), "{t}");
}

fn goto_asked(ctx: &egui::Context) -> Option<(String, Option<String>)> {
    ctx.data(|d| d.get_temp::<Option<(String, Option<String>)>>(egui::Id::new("share_shared_circle_goto"))).flatten()
}

#[test]
fn m6_the_banks_list_offers_a_remedy_that_works() {
    use super::browse::Problem;
    use super::circles::AddJob;
    let job = |bid: String, title: &str, state: AddState| AddJob { cid: CID.into(), bid, own: false, title: title.into(), role: Role::Editor, state, full: false, dismissed: false, account: None };
    let mut m = model();
    let gone = Problem { message: "That circle or bank is not available to you any more.".into(), retry: false, retry_at: None, account: false };
    let offline = Problem { message: "No connection.".into(), retry: true, retry_at: None, account: false };
    m.adds.push(job(hex(1), "Gone bank", AddState::Failed(gone)));
    m.adds.push(job(hex(2), "Offline bank", AddState::Failed(offline)));
    m.adds.push(job(hex(3), "Pictures bank", AddState::Done { bank_id: "b3".into(), bank_name: "Pictures bank".into(), linked: false, why: Some("1 of 2 pictures couldn't be fetched".into()) }));
    let t = rows(&mut m);
    assert!(t.contains("\"Gone bank\" couldn't be added: That circle or bank is not available to you any more.") && !t.contains("any more. Try again"), "no useless Try again: {t}");
    assert!(t.contains("\"Offline bank\" couldn't be added: No connection. Try again from the circle's page."), "{t}");
    assert_eq!(t.matches("Open the circle").count(), 1, "only where trying again can help: {t}");
    assert!(t.contains("\"Pictures bank\" was added, but it is not connected to the circle: 1 of 2 pictures couldn't be fetched. Delete it in My Banks, then add it again."), "{t}");
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    assert!(super::circles_phasec_tests::click_text(&ctx, 1280.0, "Open the circle", 0, &mut |ui| super::bank_links_ui::adding_rows_of(ui, &mut m)));
    assert_eq!(goto_asked(&ctx), Some((CID.to_string(), Some(hex(2)))));
    assert!(super::circles_phasec_tests::click_text(&ctx, 1280.0, "Open it", 0, &mut |ui| super::bank_links_ui::adding_rows_of(ui, &mut m)));
    assert_eq!(m.take_open_bank().as_deref(), Some("b3"));
    // A bank of this computer that still has the link: adding again would not help, so that is not said.
    let mut m2 = model();
    let (_, l) = linked(&[(QA, "One")]);
    m2.edit.add_link(l).unwrap();
    m2.adds.push(AddJob { cid: CID.into(), bid: BID.into(), own: false, title: "Cells".into(), role: Role::Editor, state: AddState::Done { bank_id: "b9".into(), bank_name: "Cells".into(), linked: false, why: Some("\"Unit 2 quiz\" is already connected to it".into()) }, full: false, dismissed: false, account: None });
    let t = rows(&mut m2);
    assert!(t.contains("was added, but it is not connected") && !t.contains("Delete it in My Banks"), "{t}");
}

#[test]
fn m7_the_chooser_asks_which_bank_to_share_with_the_circle() {
    let mut s = super::shared_ui::SharedUi::default();
    s.share_target = Some(CID.into());
    s.model.toggle_chooser();
    let mut a = super::ui::AccountUi::in_page();
    a.model = super::model::AccountModel::new(Arc::new(|_| panic!("no client may be built")), Runner::Inline, super::model::SettingsIo::memory()).with_server_mode(super::model::server_mode(None, false));
    let (mut b, mut mi): (super::BrowseUi, super::MineUi) = Default::default();
    let (bank, _) = linked(&[(QA, "One")]);
    let mut banks = vec![bank];
    let t = text_of(1280.0, &mut |ui| {
        s.tabs(ui, &mut a, &mut b, &mut mi, &mut banks, &[]);
    });
    assert!(t.contains("Which bank do you want to share with the circle?") && !t.contains("add to the circle"), "{t}");
}

#[test]
fn m8_a_linked_import_opens_in_my_banks() {
    let rec = |item: &str, bank: &str| -> super::browse::ImportedRecord {
        serde_json::from_value(json!({"item_id": item, "bank_id": bank, "bank_name": "Unit 2 quiz", "version": 1, "license": "CC-BY-4.0", "title": "Cells", "author": "Mr Keesler", "imported_at": NOW, "newer": null, "circle": {"circle_id": CID, "circle_name": "Algebra team"}})).unwrap()
    };
    let recs = vec![rec(BID, LOCAL), rec(&hex(5), "b-plain")];
    let mut b = BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), ImportLog::Memory(Arc::new(std::sync::Mutex::new(recs))));
    let mut e = edit_model();
    e.add_link(linked(&[(QA, "One")]).1).unwrap();
    b.screen = super::browse::Screen::Browse;
    let t = text_of(1280.0, &mut |ui| {
        super::browse_ui::tab_body(ui, &mut b, super::browse::Tab::Imported);
    });
    assert_eq!(t.matches("Open in My Banks").count(), 1, "the linked one: {t}");
    assert_eq!(t.matches("Open in the circle").count(), 1, "the copy that is not linked: {t}");
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    assert!(super::circles_phasec_tests::click_text(&ctx, 1280.0, "Open in My Banks", 0, &mut |ui| {
        super::browse_ui::tab_body(ui, &mut b, super::browse::Tab::Imported);
    }));
    assert_eq!(b.open_bank.take().as_deref(), Some(LOCAL));
    assert_eq!(goto_asked(&ctx), None, "not the circle's page");
}

#[test]
fn m9_the_authors_licence_line_points_to_a_page_that_exists() {
    let t = licence_line(true, License::CcBy);
    assert!(t.contains("from Bank settings on the circle's page") && !t.contains("bank's page in the circle"), "{t}");
}

#[test]
fn m10_a_bank_shared_into_a_circle_keeps_restore_and_trim_when_the_circle_page_cannot_be_read() {
    let s = Script::new();
    let olga = as_who(&s, "olga");
    let p = share_into_circle(&s, &olga, 201, json!({"circle_id": CID, "bank_id": BID, "version": 1, "version_id": hex(1), "qids": [QA, QB], "added": 2, "changed": 0, "removed": 0, "replayed": false}));
    assert!(matches!(p.circle.stage, super::circle_send::SendStage::Done(_)));
    let mut cm = model();
    let l = cm.edit.link_for_local("b1").expect("linked");
    assert_eq!(l.role_seen_by, tag_of("olga"), "made for the account that shared it");
    // Her first check: the circle's page can not be read.
    s.keep("Post", "/circles/heads", 200, heads(1));
    s.keep("Get", "/circles", 200, circles_list("owner"));
    s.keep("Get", &format!("/circles/{CID}"), 503, json!({"error": "unavailable"}));
    let bank: QuestionBank = serde_json::from_value(json!({"id": "b1", "name": "Unit 1", "questions": [qjson("q0", "Zero"), qjson("q1", "One")], "created": 1, "modified": 2, "details": {}})).unwrap();
    cm.note_account(&olga);
    cm.links.strip(&mut cm.edit, &bank, &olga, now(), NOW);
    cm.links.frame(&mut cm.edit, NOW);
    let after = cm.edit.link_for_local("b1").unwrap();
    assert!(after.is_author && perm_of(&after).can_rewind(), "Restore and Trim stay: {after:?}");
    assert!(!s.paths().iter().any(|p| p == &format!("Get /circles/{CID}")), "not asked again: {:?}", s.paths());
}

fn tag_of(who: &str) -> Option<String> {
    account_tag(Some(&format!("https://pool.test\n{who}@school.org")))
}

fn send_view(stage: SendStage) -> SendView {
    SendView {
        local_id: LOCAL.into(),
        cid: CID.into(),
        bid: BID.into(),
        title: "Cells".into(),
        perm: Perm { role: Role::Editor, is_author: false, closed: false },
        message: String::new(),
        changes: None,
        stage,
        partial: PartialSend::Unavailable(PARTIAL_UNAVAILABLE.into()),
        base_body: None,
        notice: None,
        confirm_large: false,
        base_override: None,
        merged: false,
        attest: true,
        licence: LicenceState::Known(License::CcBy),
        replaces: Vec::new(),
        decisions: Default::default(),
        decisions_for: None,
        sent_payload: None,
        picked: None,
    }
}

// ---- critic 67: the surviving mutants, pinned ---------------------------------------------------------------------------------

fn unreadable_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("vershion-c68-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    // a directory where the file should be: "there, but it can not be read"
    std::fs::create_dir_all(dir.join(LINKS_DOC)).unwrap();
    dir
}

fn dir_model(dir: &std::path::Path) -> CircleEditModel {
    let media: super::publish::SharedMedia = Arc::new(std::collections::HashMap::<String, Vec<u8>>::new());
    let mut m = CircleEditModel::new(Runner::Inline, Arc::new(Mem::default()), media, Arc::new(NoDiskRestore), LinkLog::Dir(dir.to_path_buf())).with_retry(RetryPolicy::none());
    m.set_clock(NOW);
    m
}

/// A link made by sharing the bank into the circle before the sharing account was remembered (or by an older
/// build): its author is known, the account is not.
fn share_link() -> (QuestionBank, CircleLink) {
    let (bank, mut l) = linked(&[(QA, "One")]);
    l.is_author = true;
    l.author_known = true;
    l.role_seen = Some("owner".into());
    l.role_seen_by = None;
    (bank, l)
}

#[cfg(unix)]
#[test]
fn k02_a_link_that_could_not_be_saved_is_not_noted_as_linked() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("vershion-c68-k02-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut m = dir_model(&dir);
    m.add_link(linked(&[(QA, "One")]).1).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    let (_, mut l) = linked(&[(QB, "Two")]);
    l.local_bank_id = "local-k02".into();
    l.circle_bank_id = hex(55);
    let r = m.add_link(l);
    let wrote = std::fs::read_to_string(dir.join(LINKS_DOC)).unwrap_or_default().contains("local-k02");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    if !wrote {
        assert!(r.is_err(), "{r:?}");
        // (the publish window and the Imported tab read this: a bank the list could not keep is not "linked")
        assert!(circles_of_bank("local-k02").is_empty());
        assert_eq!(circles_of_bank(LOCAL), vec![CID.to_string()], "the saved one still is");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn k06_add_pressed_again_while_it_is_under_way_reads_the_bank_once() {
    let (mut m, s) = open_circle("editor", json!({}));
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(2, &[(QA, "One")], false));
    let a = as_who(&s, "olga");
    m.note_account(&a);
    m.add_bank(BID, false);
    m.add_bank(BID, false);
    assert_eq!(m.adds.len(), 1);
    let (mut banks, mut b) = (Vec::new(), browse());
    // The read is under way (its answer not taken yet) when Add, and then Make my own copy, are pressed again.
    m.tick_background(now(), &a);
    assert_eq!(state(&m, BID), AddState::Working);
    m.add_bank(BID, false);
    m.add_bank(BID, true);
    assert_eq!(m.adds.len(), 1, "{:?}", m.adds);
    ticks(&mut m, &a, &mut banks, &mut b, 8);
    assert_eq!(bank_reads(&s), 1, "read once: {:?}", s.paths());
    assert_eq!(banks.len(), 1);
    assert!(matches!(state(&m, BID), AddState::Done { linked: true, .. }));
}

#[test]
fn k08_k10_joining_again_checks_a_copy_that_was_already_gone_at_start_and_forgets_the_last_check() {
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, json!({"banks": [{"bank_id": BID, "status": "unavailable"}]}));
    s.keep("Get", "/circles", 200, json!({"circles": []}));
    let mut cm = model();
    // (k08) The link said "no longer in" when the app started; its one check this sign-in said so again.
    let (bank, mut l) = linked(&[(QA, "One")]);
    l.role_seen = Some(ROLE_NONE.into());
    l.role_seen_by = tag_of("olga");
    cm.edit.add_link(l).unwrap();
    let a = as_who(&s, "olga");
    cm.note_account(&a);
    let t0 = now();
    cm.links.strip(&mut cm.edit, &bank, &a, t0, NOW);
    cm.links.frame(&mut cm.edit, NOW);
    cm.links.strip(&mut cm.edit, &bank, &a, t0 + 11 * MIN, NOW);
    cm.links.frame(&mut cm.edit, NOW);
    assert_eq!(heads_count(&s), 1, "a gone link is checked once a sign-in");
    // (k10) Another linked bank of the same circle: its last check could not reach the server.
    let (mut other, mut lo) = linked(&[(QB, "Two")]);
    other.id = "local-bank-o".into();
    lo.local_bank_id = other.id.clone();
    lo.circle_bank_id = hex(66);
    lo.role_seen = Some("editor".into());
    lo.role_seen_by = tag_of("olga");
    lo.author_known = true;
    lo.mark_bank(&other);
    cm.edit.add_link(lo).unwrap();
    // (both of its reads)
    s.fail(ApiError::Offline("no network".into()));
    s.fail(ApiError::Offline("no network".into()));
    cm.links.strip(&mut cm.edit, &other, &a, t0, NOW);
    cm.links.frame(&mut cm.edit, NOW);
    assert_eq!(cm.links.check_of("local-bank-o"), Check::Offline);
    let looking = AccountSnapshot { account: a.account.clone(), ..AccountSnapshot::of_gate(Gate::Checking) };
    let v = cm.links.strip(&mut cm.edit, &other, &looking, t0, NOW).unwrap();
    assert!(v.line.contains("Couldn't check"), "{v:?}");
    // She joins again.
    s.keep("Get", "/circles", 200, json!({"circles": [summary("editor", json!({"name": "Algebra team"}))]}));
    s.keep("Post", "/circles/heads", 200, heads(1));
    s.on("Post", "/circles/join/preview", 200, json!({"circle": {"id": CID, "name": "Algebra team", "owner_name": "Ms Frizzle", "members": 3}, "role": "editor", "already_member": false}));
    s.on("Post", "/circles/join", 200, json!({"circle_id": CID, "role": "editor", "already": false}));
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("editor", json!({})));
    cm.open_join("C-ABCDEFGHJKMNPQRSTVWXYZ01");
    cm.frame(t0, &a);
    cm.look_up_code();
    cm.frame(t0, &a);
    cm.confirm_join();
    for _ in 0..3 {
        cm.frame(t0, &a);
    }
    // (k10) While the account is looked up again, the other bank does not say its last check failed.
    let v = cm.links.strip(&mut cm.edit, &other, &looking, t0 + 12 * MIN, NOW).unwrap();
    assert!(!v.line.contains("Couldn't check"), "{v:?}");
    // (k08) The gone one is checked again at once, and is in the circle again.
    let before = heads_count(&s);
    cm.links.strip(&mut cm.edit, &bank, &a, t0 + 12 * MIN, NOW);
    cm.links.frame(&mut cm.edit, NOW);
    assert_eq!(heads_count(&s), before + 1, "{:?}", s.paths());
    let v = cm.links.strip(&mut cm.edit, &bank, &a, t0 + 12 * MIN, NOW).unwrap();
    assert!(!v.line.contains("no longer in"), "{v:?}");
}

#[test]
fn k12_another_member_does_not_get_restore_and_trim_from_a_shared_banks_link() {
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, heads(1));
    s.keep("Get", "/circles", 200, circles_list("editor"));
    // pete's circle page can not be read: whether he is the author stays unknown
    s.keep("Get", &format!("/circles/{CID}"), 503, json!({"error": "unavailable"}));
    let mut cm = model();
    let (bank, l) = share_link();
    cm.edit.add_link(l).unwrap();
    let p = as_who(&s, "pete");
    cm.note_account(&p);
    cm.links.strip(&mut cm.edit, &bank, &p, now(), NOW);
    cm.links.frame(&mut cm.edit, NOW);
    let after = cm.edit.link_for_local(LOCAL).unwrap();
    assert!(!after.is_author && !after.author_known && !perm_of(&after).can_rewind(), "{after:?}");
}

#[test]
fn k13_the_author_keeps_restore_and_trim_after_the_first_check_of_a_shared_banks_link() {
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, heads(1));
    s.keep("Get", "/circles", 200, circles_list("owner"));
    let mut d = detail("owner", json!({}));
    d["banks"][0]["is_author"] = json!(true);
    s.keep("Get", &format!("/circles/{CID}"), 200, d);
    let mut cm = model();
    let (bank, l) = share_link();
    cm.edit.add_link(l).unwrap();
    let o = as_who(&s, "olga");
    cm.note_account(&o);
    cm.links.strip(&mut cm.edit, &bank, &o, now(), NOW);
    cm.links.frame(&mut cm.edit, NOW);
    let after = cm.edit.link_for_local(LOCAL).unwrap();
    assert!(after.is_author && after.author_known && perm_of(&after).can_rewind(), "{after:?}");
    assert_eq!(after.role_seen_by, tag_of("olga"));
    assert!(s.paths().iter().any(|p| p == &format!("Get /circles/{CID}")), "learnt in the same check: {:?}", s.paths());
}

#[test]
fn k15_another_accounts_visit_to_the_circle_page_does_not_change_a_shared_banks_link() {
    let s = Script::new();
    s.keep("Get", "/circles", 200, json!({"circles": [summary("editor", json!({}))]}));
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("editor", json!({})));
    s.keep("Get", &format!("/circles/{CID}/activity"), 200, json!({"activity": [], "next_before": null}));
    let mut m = model();
    m.edit.add_link(share_link().1).unwrap();
    let p = as_who(&s, "pete");
    m.note_account(&p);
    m.open_list();
    m.frame(now(), &p);
    m.open_circle(CID);
    for _ in 0..4 {
        m.frame(now(), &p);
    }
    assert!(m.circle.as_ref().is_some_and(|c| c.detail.is_some()));
    let after = m.edit.link_for_local(LOCAL).unwrap();
    assert!(after.is_author && after.author_known, "pete's page does not speak for the author's link: {after:?}");
}

#[test]
fn k20_another_account_is_named_only_when_the_link_knows_its_account() {
    let st = status(0, 0, true, false);
    let (_, shared) = share_link();
    let v = strip_view(&StripInput { check: Check::OtherAccount, ..input(&shared, &st) });
    assert!(v.line.starts_with("Shared with Algebra team. The account signed in now is not in this circle"), "{}", v.line);
    let (_, mut known) = linked(&[(QA, "One")]);
    known.role_seen_by = tag_of("olga");
    let v = strip_view(&StripInput { check: Check::OtherAccount, ..input(&known, &st) });
    assert!(v.line.starts_with("Shared with Algebra team by another account on this computer."), "{}", v.line);
}

/// The viewers' form of one question, `kind` as the pool serves it.
fn qkind(id: &str, stem: &str, kind: serde_json::Value) -> serde_json::Value {
    json!({"id": id, "stem": stem, "kind": kind, "points": 1.0, "difficulty": null, "explanation": null, "keyless": true, "created": 5, "modified": 6})
}

/// For each question of `next`: whether it took the form this copy had (`true`: only shuffled).
fn settled(first: Vec<serde_json::Value>, next: Vec<serde_json::Value>) -> Vec<bool> {
    use super::merge::{BaseQuestion, base_of};
    let a = circle_theirs_as(&body_of(first), NOW, true);
    let base: Vec<BaseQuestion> = a.iter().map(|t| base_of(&t.q, &t.qid, &t.fp, t.sha.clone())).collect();
    let mut b = circle_theirs_as(&body_of(next), NOW, true);
    settle_shuffle(&mut b, &base);
    b.iter().zip(&base).map(|(t, bq)| serde_json::to_value(&t.q.kind).unwrap() == serde_json::to_value(&bq.snap.kind).unwrap()).collect()
}

#[test]
fn k25_k26_a_reworded_part_or_locked_choice_is_a_change_not_a_shuffle() {
    // (k25) A Multipart part reworded, its choices also shuffled.
    let part = |stem: &str, choices: &[&str]| json!({"Multipart": {"parts": [{"stem": stem, "kind": {"Mc": {"answers": choices.iter().map(|t| json!({"text": t, "correct": false})).collect::<Vec<_>>()}}}]}});
    let first = part("Part one", &["Two", "Three", "Four"]);
    assert_eq!(
        settled(vec![qkind(QA, "Parts", first.clone()), qkind(QB, "Parts 2", first)], vec![qkind(QA, "Parts", part("Part one", &["Four", "Two", "Three"])), qkind(QB, "Parts 2", part("Part one, reworded", &["Four", "Two", "Three"]))]),
        vec![true, false]
    );
    // (k26) A locked choice reworded in its place.
    let mc = |choices: &[(&str, bool)]| json!({"Mc": {"answers": choices.iter().map(|(t, locked)| json!({"text": t, "correct": false, "locked": locked})).collect::<Vec<_>>()}});
    let first = mc(&[("Mitochondria", false), ("Ribosome", false), ("I don't know", true)]);
    assert_eq!(
        settled(vec![qkind(QA, "Which?", first.clone()), qkind(QB, "Which 2?", first)], vec![qkind(QA, "Which?", mc(&[("Ribosome", false), ("Mitochondria", false), ("I don't know", true)])), qkind(QB, "Which 2?", mc(&[("Ribosome", false), ("Mitochondria", false), ("Not sure", true)]))]),
        vec![true, false]
    );
}

#[test]
fn k28_an_authors_real_reorder_of_choices_is_applied_to_a_full_copy() {
    let (bank, l) = linked_full(&[(QA, "One"), (QB, "Two")]);
    let s = Script::new();
    s.on("Post", &format!("/circles/{CID}/banks/{BID}/merge"), 200, json!({"base_version": 1, "base_kept": true, "head_version": 2, "head_version_id": hex(2), "head_at": NOW, "head_by": "B. Teacher", "merge_token": "t".repeat(40), "between": []}));
    let mut reordered = qfull(QA, "One");
    reordered["kind"]["Mc"]["answers"] = json!([{"text": "B", "correct": false}, {"text": "A", "correct": true}]);
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served_qs(2, vec![reordered, qfull(QB, "Two")], false));
    let mut m = edit_model();
    m.add_link(l).unwrap();
    let mut banks = vec![bank];
    m.frame(now(), &ready(&s));
    m.open_merge(LOCAL, Perm { role: Role::Editor, is_author: false, closed: false });
    turn(&mut m, &s, &mut banks, 3);
    m.apply_merge();
    turn(&mut m, &s, &mut banks, 3);
    assert!(phase(&m).starts_with("Applied"), "{}", phase(&m));
    let crate::model::Kind::Mc { answers } = &banks[0].questions[0].kind else { panic!() };
    assert_eq!(answers.iter().map(|a| a.text.as_str()).collect::<Vec<_>>(), vec!["B", "A"], "the author put B first on purpose");
}

#[test]
fn k32_k33_try_again_on_the_strip_and_opening_the_shared_page_read_the_list_again() {
    for how in ["strip", "shared page"] {
        let dir = unreadable_dir(&format!("k32-{}", how.len()));
        let mut m = model();
        m.edit = dir_model(&dir);
        let (bank, l) = linked(&[(QA, "One")]);
        let mut banks = vec![bank];
        let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
        let s = Script::new();
        s.keep("Post", "/circles/heads", 200, heads(1));
        s.keep("Get", "/circles", 200, circles_list("editor"));
        let (_, t) = page_frame(&mut m, &mut banks, &mut acct, &ready(&s), now());
        assert!(t.contains("couldn't be read"), "{t}");
        // The file can be read again (it was a moment's lock), with the link in it.
        std::fs::remove_dir_all(dir.join(LINKS_DOC)).unwrap();
        std::fs::write(dir.join(LINKS_DOC), json!({"links": [l]}).to_string()).unwrap();
        if how == "strip" {
            let ctx = egui::Context::default();
            crate::theme::setup(&ctx);
            let mut b = browse();
            assert!(super::circles_phasec_tests::click_text(&ctx, 1280.0, "Try again", 0, &mut |ui| {
                super::bank_links_ui::strip_with(ui, &mut m, &mut banks, &mut b, &mut acct, &ready(&s), now(), NOW, LOCAL);
            }));
        } else {
            m.open_list();
        }
        assert!(m.edit.unreadable().is_none(), "{how}: read again");
        let (_, t) = page_frame(&mut m, &mut banks, &mut acct, &ready(&s), now());
        assert!(t.contains("Shared with Algebra team") && !t.contains("couldn't be read"), "{how}: {t}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

// ---- the new code's own variants, pinned -------------------------------------------------------------------------------------

#[test]
fn i1_opening_the_circles_tab_tries_a_stopped_add_once_more() {
    let (mut m, s) = open_circle("editor", json!({}));
    let a = as_who(&s, "olga");
    m.note_account(&a);
    m.add_bank(BID, false);
    s.fail(ApiError::SignedOut);
    let (mut banks, mut b) = (Vec::new(), browse());
    ticks(&mut m, &a, &mut banks, &mut b, 4);
    assert!(m.adds_need_sign_in());
    let reads = bank_reads(&s);
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(2, &[(QA, "One")], false));
    m.open_list();
    for _ in 0..3 {
        m.frame(now(), &a);
        m.sync(&mut banks, &mut b);
    }
    assert_eq!(bank_reads(&s), reads + 1, "{:?}", s.paths());
    assert_eq!(banks.len(), 1);
}

#[test]
fn i2_a_copy_already_put_into_the_banks_when_another_account_signs_in_is_linked_and_not_said_to_have_failed() {
    let (mut m, s) = open_circle("editor", json!({}));
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(2, &[(QA, "One")], false));
    let (a, p) = (as_who(&s, "olga"), as_who(&s, "pete"));
    m.note_account(&a);
    m.add_bank(BID, false);
    let (mut banks, mut b) = (Vec::new(), browse());
    // Read, handed to the banks' side, and put into the banks by the browse window's frame; not linked yet.
    m.tick_background(now(), &a);
    m.tick_background(now(), &a);
    assert!(super::circles::land_waiting(&mut banks, &mut b));
    assert_eq!(banks.len(), 1);
    // The Shared page's next frame is the first to see pete.
    m.frame(now(), &p);
    m.sync(&mut banks, &mut b);
    assert!(m.edit.link_for_local(&banks[0].id).is_some_and(|l| l.role_seen_by == tag_of("olga")), "linked for the account that asked");
    let t = rows(&mut m);
    assert!(!t.contains("couldn't be added"), "{t}");
}

#[test]
fn i3_a_retried_send_does_not_say_wait_until_another_page_is_asked_for() {
    let (mut m, s, mut banks) = two_linked();
    s.send_base_404(true);
    let e = &mut m.edit;
    e.frame(now(), &ready(&s));
    assert!(e.open_send(LOCAL, editor()));
    e.frame(now(), &ready(&s));
    e.set_attest(true);
    e.frame(now(), &ready(&s));
    e.sync(&mut banks);
    e.frame(now(), &ready(&s));
    s.on("Put", &format!("/circles/{CID}/banks/{BID}"), 503, json!({"error": "unavailable"}));
    e.send();
    e.frame(now(), &ready(&s));
    e.sync(&mut banks);
    assert!(!e.open_history(CID, &hex(77), "Plants", editor()));
    assert!(e.wait_note().is_some());
    e.frame(now(), &ready(&s));
    e.sync(&mut banks);
    assert!(matches!(&e.view, EditView::Send(v) if matches!(v.stage, SendStage::Failed(_))), "{:?}", e.view);
    assert!(e.wait_note().is_none());
    // "Try again" on A's page: on its way again, and nobody asked for another page meanwhile.
    s.on("Put", &format!("/circles/{CID}/banks/{BID}"), 201, json!({"circle_id": CID, "bank_id": BID, "version": 2, "version_id": hex(2), "qids": [QA], "added": 0, "changed": 1, "removed": 0}));
    e.retry_act();
    e.sync(&mut banks);
    assert!(e.acting());
    assert!(e.wait_note().is_none(), "{:?}", e.wait_note());
}

#[test]
fn i3_the_bank_settings_history_says_to_wait_too() {
    let (mut m, s, _banks) = two_linked();
    let lines: Vec<serde_json::Value> = [3u64, 2, 1].iter().map(|v| json!({"version": v, "version_id": hex(*v), "by": "Mr Keesler", "at": NOW - DAY, "message": "", "added": 0, "changed": 1, "removed": 0, "question_count": 1, "license": "CC-BY-4.0", "kept": true, "withheld": false})).collect();
    s.on("Get", &format!("/circles/{CID}/banks/{BID}/versions"), 200, json!({"bank_id": BID, "head_version": 3, "oldest_kept": 1, "pinned_versions": [], "versions": lines, "next_before": null}));
    let owner = Perm { role: Role::Owner, is_author: true, closed: false };
    m.edit.frame(now(), &ready(&s));
    assert!(m.edit.open_history(CID, BID, "Cells", owner));
    m.edit.frame(now(), &ready(&s));
    m.edit.ask_restore(1);
    s.on("Post", &format!("/circles/{CID}/banks/{BID}/rollback"), 200, json!({"circle_id": CID, "bank_id": BID, "version": 4, "version_id": hex(4), "qids": [QA], "added": 0, "changed": 1, "removed": 0, "rollback_of": 1}));
    m.edit.confirm_restore(false);
    assert!(!m.edit.open_history(CID, &hex(77), "Plants", owner));
    let t = text_of(1280.0, &mut |ui| super::circle_edit_ui::history_section(ui, &mut m, CID, &hex(77), "Plants"));
    assert!(t.contains("Wait for the restore of \"Unit 2 quiz\" to finish"), "{t}");
}

#[test]
fn i3_another_banks_strip_does_not_take_over_a_send_under_way_and_says_to_wait() {
    // A send on its way for real (a background thread held at the server), while the person uses bank B's strip.
    let (mut m, s, mut banks) = two_linked();
    s.send_base_404(true);
    let media: super::publish::SharedMedia = Arc::new(std::collections::HashMap::<String, Vec<u8>>::new());
    let mut e = CircleEditModel::new(Runner::Background, Arc::new(Mem::default()), media, Arc::new(NoDiskRestore), LinkLog::memory()).with_retry(RetryPolicy::none());
    e.set_clock(NOW);
    for l in m.edit.links().to_vec() {
        e.add_link(l).unwrap();
    }
    m.edit = e;
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    let wait_for = |m: &mut CirclesModel, banks: &mut Vec<QuestionBank>, done: &dyn Fn(&CirclesModel) -> bool| {
        for _ in 0..500 {
            m.edit.frame(now(), &ready(&s));
            m.edit.sync(banks);
            if done(m) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("timed out: {:?}", m.edit.view);
    };
    let strip_b = |m: &mut CirclesModel, banks: &mut Vec<QuestionBank>, acct: &mut FakeAccount| {
        let mut b = browse();
        text_of(1280.0, &mut |ui| {
            super::bank_links_ui::strip_with(ui, m, banks, &mut b, acct, &ready(&s), now(), NOW, LOCAL_B);
        })
    };
    // B's strip once, so its own check is done before the send.
    let t = strip_b(&mut m, &mut banks, &mut acct);
    assert!(t.contains("Shared with Algebra team"), "{t}");
    // A's send page, hosted on A's page.
    assert!(m.edit.open_send(LOCAL, editor()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    wait_for(&mut m, &mut banks, &|m| matches!(&m.edit.view, EditView::Send(v) if matches!(v.licence, LicenceState::Known(_)) && !matches!(v.partial, PartialSend::Reading) && v.changes.is_some()));
    m.edit.set_attest(true);
    s.on("Put", &format!("/circles/{CID}/banks/{BID}"), 201, json!({"circle_id": CID, "bank_id": BID, "version": 2, "version_id": hex(2), "qids": [QA], "added": 0, "changed": 1, "removed": 0}));
    let release = s.hold();
    m.edit.send();
    m.edit.sync(&mut banks);
    assert!(m.edit.acting());
    // On B's page: History.
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    let mut b = browse();
    assert!(super::circles_phasec_tests::click_text(&ctx, 1280.0, "History", 0, &mut |ui| {
        super::bank_links_ui::strip_with(ui, &mut m, &mut banks, &mut b, &mut acct, &ready(&s), now(), NOW, LOCAL_B);
    }));
    let t = strip_b(&mut m, &mut banks, &mut acct);
    assert!(!t.contains("Send my changes: \"Cells\""), "A's send page is not drawn on B's page: {t}");
    assert!(t.contains("Wait for the send from \"Unit 2 quiz\" to finish"), "{t}");
    assert_eq!(m.edit.host.as_ref().map(|h| h.local_id.as_str()), Some(LOCAL), "A's page keeps it");
    release.send(()).unwrap();
    wait_for(&mut m, &mut banks, &|m| !m.edit.acting());
    assert_eq!(m.edit.link_for_local(LOCAL).unwrap().base_version, 2);
}
