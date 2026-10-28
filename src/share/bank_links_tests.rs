//! Phase D1 (docs/circles-phaseD-plan.md section 10): circle banks in My Banks. The link file with and without the
//! new fields, the badge and every state of the strip, no request outside a linked bank's page, the check's
//! 10-minute rule, "Add to My Banks" by role, the viewers' form never sending, "Make my own copy", a bank shared
//! into a circle becoming the linked bank, the join offer's ticks, and another account dropping what was read.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use super::bank_links::*;
use super::browse::{BrowseModel, ImportLog};
use super::circle_edit::*;
use super::circle_links::*;
use super::circles::{AddState, CirclesModel};
use super::circles_api::*;
use super::circles_tests::*;
use super::merge::base_of;
use super::model::Runner;
use super::pool::*;
use super::publish::{AccountSnapshot, Gate, LogIo, PublishModel, ShareSource, Step};
use crate::model::QuestionBank;

pub(super) const LOCAL: &str = "local-bank-1";
pub(super) const QA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1";
pub(super) const QB: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa2";
pub(super) const MIN: Duration = Duration::from_secs(60);

pub(super) fn hex(v: u64) -> String {
    format!("{v:032x}")
}

pub(super) fn qjson(id: &str, stem: &str) -> Value {
    json!({"id": id, "stem": stem, "kind": {"Mc": {"answers": [{"text": "A", "correct": true}, {"text": "B", "correct": false}]}}, "points": 1.0, "difficulty": null, "created": 5, "modified": 6})
}

pub(super) fn payload(items: &[(&str, &str)]) -> Vec<u8> {
    let qs: Vec<Value> = items.iter().map(|(q, s)| qjson(q, s)).collect();
    json!({"schema_version": 2, "bank": {"id": BID, "name": "Cells", "questions": qs, "created": 1, "modified": 2}}).to_string().into_bytes()
}

/// A circle bank as the pool serves it; `keys_hidden`: the viewers' form of a bank that hides its keys.
pub(super) fn served(version: u64, items: &[(&str, &str)], keys_hidden: bool) -> Vec<u8> {
    let head = json!({"circle": {"id": CID, "name": "Biology team"}, "bank": {"id": BID, "title": "Cells", "subject": "Biology", "course": "", "grade": "", "license": "CC-BY-4.0", "version_license": "CC-BY-4.0", "author_name": "Mr Keesler", "is_author": false, "version": version, "version_id": hex(version), "question_count": items.len(), "has_images": false, "committed_at": NOW - DAY, "hide_key": keys_hidden, "keys_hidden": keys_hidden}, "origins": null});
    let h = head.to_string();
    let p = String::from_utf8(payload(items)).unwrap();
    format!("{},\"payload\":{}}}", &h[..h.len() - 1], p).into_bytes()
}

/// A linked bank made from version 1, and its link.
pub(super) fn linked(items: &[(&str, &str)]) -> (QuestionBank, CircleLink) {
    let theirs = circle_theirs(&payload(items), NOW);
    let bank = QuestionBank { id: LOCAL.into(), name: "Unit 2 quiz".into(), questions: theirs.iter().map(|t| t.q.clone()).collect(), created: NOW / 1000, modified: NOW / 1000, details: Default::default() };
    let head = BankHead { id: BID.into(), title: "Cells".into(), license: "CC-BY-4.0".into(), version: 1, version_id: hex(1), committed_at: NOW - DAY, ..BankHead::default() };
    let base = theirs.iter().map(|t| base_of(&t.q, &t.qid, &t.fp, t.sha.clone())).collect();
    let mut l = CircleLink::from_head(CID, "Algebra team", &head, LOCAL, base, NOW);
    l.mark_bank(&bank);
    (bank, l)
}

pub(super) fn edit_model() -> CircleEditModel {
    let media: super::publish::SharedMedia = Arc::new(HashMap::<String, Vec<u8>>::new());
    let mut m = CircleEditModel::new(Runner::Inline, Arc::new(Mem::default()), media, Arc::new(NoDiskRestore), LinkLog::memory()).with_retry(RetryPolicy::none());
    m.set_clock(NOW);
    m
}

pub(super) fn checks() -> BankLinks {
    let mut c = BankLinks::new(Runner::Inline);
    c.set_retry(RetryPolicy::none());
    c
}

pub(super) fn heads(version: u64) -> Value {
    json!({"banks": [{"bank_id": BID, "status": "current", "version": version, "version_id": hex(version), "license": "CC-BY-4.0", "at": NOW - 1000, "by": "B. Teacher", "hide_key": false}]})
}

pub(super) fn circles_list(role: &str) -> Value {
    json!({"circles": [summary(role, json!({"name": "Algebra team"}))]})
}

pub(super) fn status(added: usize, changed: usize, known: bool, stale: bool) -> LinkStatus {
    // (built through the model so the private cache key stays private)
    let (mut bank, l) = linked(&[(QA, "One"), (QB, "Two")]);
    let mut m = edit_model();
    m.add_link(l).unwrap();
    for i in 0..added {
        let mut q = bank.questions[0].clone();
        q.id = format!("new-{i}");
        bank.questions.push(q);
    }
    for q in bank.questions.iter_mut().take(changed) {
        q.stem = format!("{} (changed)", q.stem);
    }
    if !known {
        m.update_link(LOCAL, |l| {
            l.base.clear();
            true
        });
    }
    if stale {
        m.update_link(LOCAL, |l| {
            l.bank_modified = bank.modified + 100;
            l.bank_print = "something else".into();
            true
        });
    }
    m.status_of(&bank).unwrap()
}

pub(super) fn input<'a>(l: &'a CircleLink, s: &'a LinkStatus) -> StripInput<'a> {
    StripInput { link: l, status: Some(s), check: Check::Done, sign_in: SignIn::Ready, now_ms: NOW, act: None, sent_unread: false, changed_since: false, unsure: false }
}

// ---- the link file ---------------------------------------------------------------------------------------------

#[test]
fn a_link_file_with_and_without_the_new_fields_loads_the_same() {
    let (_, l) = linked(&[(QA, "One")]);
    let mut v = serde_json::to_value(&l).unwrap();
    // A file written before phase D: none of the new fields.
    for k in ["mode", "viewers_form", "role_seen", "role_seen_at", "is_author", "seen"] {
        assert!(v.as_object_mut().unwrap().remove(k).is_some(), "{k} is written");
    }
    let old: CircleLink = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(old.mode, LinkMode::Edit, "a link from before phase D is a working copy");
    assert!(!old.viewers_form && old.role_seen.is_none() && old.seen == BankSeen::default());
    assert_eq!(old, l, "it loads the same as the link itself");
    // Written again and read back: the same.
    let again: CircleLink = serde_json::from_value(serde_json::to_value(&old).unwrap()).unwrap();
    assert_eq!(again, old);
    // A follow link round-trips; a mode this build does not know never sends.
    let mut f = l.clone();
    f.mode = LinkMode::Follow;
    f.viewers_form = true;
    let back: CircleLink = serde_json::from_value(serde_json::to_value(&f).unwrap()).unwrap();
    assert_eq!(back, f);
    let mut odd = v;
    odd["mode"] = json!("co-edit");
    odd["seen"] = json!({"removed": true, "something_new": 1});
    let odd: CircleLink = serde_json::from_value(odd).unwrap();
    assert_eq!(odd.mode, LinkMode::Follow);
    assert!(!odd.can_send() && odd.seen.removed);
    // Through the file the app keeps.
    let dir = std::env::temp_dir().join(format!("vershion-d1-links-{}-a", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut file = serde_json::to_value(&l).unwrap();
    file.as_object_mut().unwrap().remove("mode");
    std::fs::write(dir.join(LINKS_DOC), json!({"links": [file]}).to_string()).unwrap();
    let got = LinkLog::Dir(dir.clone()).load();
    assert_eq!(got, vec![l]);
    assert!(!dir.join(format!("{LINKS_DOC}.bad")).exists(), "nothing is set aside");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unreadable_links_file_is_not_written_over_and_every_bank_page_says_so() {
    let dir = std::env::temp_dir().join(format!("vershion-d1-links-{}-b", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(LINKS_DOC)).unwrap();
    let media: super::publish::SharedMedia = Arc::new(HashMap::<String, Vec<u8>>::new());
    let mut m = CircleEditModel::new(Runner::Inline, Arc::new(Mem::default()), media, Arc::new(NoDiskRestore), LinkLog::Dir(dir.clone()));
    assert!(m.unreadable().is_some());
    // A bank shared into a circle meanwhile: not linked (critic 65 B1: no bank can be told apart), and said.
    let (bank, mut l) = linked(&[(QA, "One")]);
    l.circle_bank_id = hex(77);
    queue_link(l);
    assert!(m.links().is_empty());
    assert!(matches!(outcome_of(&hex(77)), Some(LinkOutcome::NotSaved(_))));
    assert!(m.status_of(&bank).is_none());
    assert!(dir.join(LINKS_DOC).is_dir(), "nothing was written over it");
    assert_eq!(unreadable_view().line, "The list of shared banks couldn't be read. Nothing was changed.");
    assert_eq!(unreadable_view().buttons, vec![StripButton::TryAgain]);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- the badge ------------------------------------------------------------------------------------------------------

#[test]
fn the_badge_says_the_circle_and_at_most_one_state() {
    let (_, l) = linked(&[(QA, "One"), (QB, "Two")]);
    let none = status(0, 0, true, false);
    assert_eq!(badge_of(&l, Some(&none)), Badge { circle: "Algebra team".into(), note: None });
    let unsent = status(1, 2, true, false);
    assert_eq!(badge_of(&l, Some(&unsent)).note.as_deref(), Some("3 unsent changes"));
    assert_eq!(badge_of(&l, Some(&status(1, 0, true, false))).note.as_deref(), Some("1 unsent change"));
    let mut behind = l.clone();
    behind.head_seen = 3;
    assert_eq!(badge_of(&behind, Some(&unsent)).note.as_deref(), Some("newer version · 3 unsent"), "newer version first, the unsent work beside it");
    assert_eq!(badge_of(&behind, Some(&none)).note.as_deref(), Some("newer version"));
    assert_eq!(badge_of(&l, Some(&status(0, 0, false, false))).note.as_deref(), Some("changes not counted"));
    let mut gone = behind.clone();
    gone.role_seen = Some(ROLE_NONE.into());
    assert_eq!(badge_of(&gone, Some(&unsent)).note.as_deref(), Some("no longer in the circle"));
    let mut closed = l.clone();
    closed.seen.circle_closed = true;
    assert_eq!(badge_of(&closed, None).note.as_deref(), Some("circle closed"));
    let mut removed = l.clone();
    removed.seen.removed = true;
    assert_eq!(badge_of(&removed, None).note.as_deref(), Some("removed from the circle"));
    // A copy that can not send has no unsent changes.
    let mut follow = l.clone();
    follow.mode = LinkMode::Follow;
    assert_eq!(badge_of(&follow, Some(&unsent)).note, None);
    // Server text in the name is drawn plain.
    let mut odd = l;
    odd.circle_name = "<b>Algebra</b>\u{202e} team".into();
    assert!(!badge_of(&odd, None).circle.contains('\u{202e}'));
}

#[test]
fn no_request_on_app_start_on_the_banks_list_or_on_an_unlinked_bank() {
    let s = Script::new();
    // App start: the models are made and do nothing.
    let mut c = checks();
    let mut m = edit_model();
    let cm = model();
    assert!(!cm.links.busy());
    let (bank, l) = linked(&[(QA, "One")]);
    m.add_link(l).unwrap();
    // The Banks list: every badge, many frames, signed in.
    for _ in 0..5 {
        assert!(c.badge(&mut m, &bank).is_some());
        c.frame(&mut m, NOW);
    }
    // An unlinked bank's page, signed in: no strip, no request.
    let other = QuestionBank { id: "other".into(), name: "Mine".into(), questions: Vec::new(), created: 1, modified: 1, details: Default::default() };
    assert!(c.strip(&mut m, &other, &ready(&s), now(), NOW).is_none());
    assert!(c.badge(&mut m, &other).is_none());
    assert!(s.seen().is_empty(), "{:?}", s.paths());
}

// ---- the strip ------------------------------------------------------------------------------------------------------

#[test]
fn the_strip_says_every_state_of_the_table() {
    use StripButton::*;
    let (_, l) = linked(&[(QA, "One"), (QB, "Two")]);
    let in_step = status(0, 0, true, false);
    let unsent = status(1, 2, true, false);
    let v = strip_view(&input(&l, &in_step));
    assert_eq!((v.line.as_str(), v.buttons.clone()), ("Shared with Algebra team. Version 1 is the latest. Your copy is up to date.", vec![History]));
    let v = strip_view(&input(&l, &unsent));
    assert_eq!((v.line.as_str(), v.buttons.clone()), ("Shared with Algebra team. Version 1 is the latest. You have 3 unsent changes.", vec![Send, History]));
    let mut behind = l.clone();
    behind.head_seen = 3;
    let v = strip_view(&input(&behind, &in_step));
    assert_eq!((v.line.as_str(), v.buttons.clone()), ("Shared with Algebra team. Version 3 is the latest. You have version 1.", vec![GetLatest, History]));
    let v = strip_view(&input(&behind, &unsent));
    assert_eq!(v.line, "Shared with Algebra team. Version 3 is the latest. You have version 1 and 3 unsent changes. Get the latest first, then send.");
    assert_eq!(v.buttons, vec![GetLatest, Send, History], "Q10: Get latest first, Send still offered");
    let v = strip_view(&input(&l, &status(0, 0, false, false)));
    // (critic 66 minor 5) Get latest only when the copy is behind
    // (critic 88 I2: words the Send page bears out)
    assert_eq!((v.line.as_str(), v.buttons.clone()), ("Shared with Algebra team. Version 1 is the latest. Your copy has no record of what it looked like when it last matched the circle, so the changes can't be counted here.", vec![Send, History]));
    // A viewer's follow link.
    let mut f = behind.clone();
    f.mode = LinkMode::Follow;
    f.role_seen = Some("viewer".into());
    let v = strip_view(&input(&f, &in_step));
    assert_eq!((v.line.as_str(), v.buttons.clone()), ("Shared with Algebra team (you can view). Version 3 is the latest. You have version 1.", vec![GetLatest, History]));
    let v = strip_view(&input(&f, &status(0, 2, true, false)));
    assert_eq!((v.line.as_str(), v.buttons.clone()), ("Shared with Algebra team (you can view). Version 3 is the latest. You changed 2 questions here. They stay on this computer; viewers can't send.", vec![GetLatest]));
    // The viewers' form.
    let mut vf = f.clone();
    vf.viewers_form = true;
    let v = strip_view(&input(&vf, &in_step));
    assert_eq!((v.line.as_str(), v.buttons.clone()), ("Shared with Algebra team. Answer keys are hidden from viewers in this bank, so your copy has none.", vec![GetLatest]));
    // Checking, signed out, offline.
    let v = strip_view(&StripInput { check: Check::Checking, ..input(&l, &in_step) });
    assert!(v.busy && v.line == "Shared with Algebra team. Checking for a newer version..." && v.buttons.is_empty());
    let mut seen = l.clone();
    seen.role_seen_at = Some(NOW - 2 * DAY);
    let v = strip_view(&StripInput { sign_in: SignIn::Out, ..input(&seen, &unsent) });
    assert_eq!((v.line.as_str(), v.buttons.clone()), ("Shared with Algebra team. Version 1 when last checked, 2 days ago. You have 3 unsent changes. Sign in to send or get newer versions.", vec![OpenAccount]));
    let v = strip_view(&StripInput { check: Check::Offline, ..input(&l, &unsent) });
    assert_eq!((v.line.as_str(), v.buttons.clone()), ("Couldn't check the circle just now. Your bank works as usual.", vec![TryAgain]));
    // Put back from a safety copy.
    let v = strip_view(&input(&l, &status(0, 0, true, true)));
    assert_eq!((v.line.as_str(), v.buttons.clone()), ("This bank was put back from a safety copy, so verSHion can't tell what you changed since version 1.", vec![GetLatest, TreatInStep, Unlink]));
    assert!(v.warn);
    // Held back for review.
    let mut held = l.clone();
    held.seen.under_review = true;
    let v = strip_view(&input(&held, &unsent));
    assert_eq!((v.line.as_str(), v.buttons.clone()), ("Shared with Algebra team. The circle's newest version is held back for review. You can keep working here.", vec![Send]));
    // Removed from the circle, with and without a time to bring it back.
    let mut removed = l.clone();
    removed.seen.removed = true;
    removed.seen.removed_until = Some(NOW + 9 * DAY);
    let v = strip_view(&input(&removed, &unsent));
    assert!(v.line.starts_with("The bank was removed from the circle (it can be brought back until ") && v.line.ends_with("). Your copy stays as it is. Ask the circle's owner if it should come back. You can keep using it as your own bank."), "{}", v.line);
    assert_eq!(v.buttons, vec![Unlink], "(critic 66 I6) Unlink is a small button, never the big one");
    removed.seen.removed_until = None;
    assert_eq!(strip_view(&input(&removed, &unsent)).line, "The bank was removed from the circle. Your copy stays as it is. You can keep using it as your own bank.");
    // No longer in the circle, or it closed.
    let mut gone = l.clone();
    gone.role_seen = Some(ROLE_NONE.into());
    let v = strip_view(&input(&gone, &unsent));
    assert_eq!((v.line.as_str(), v.buttons.clone()), ("You are no longer in Algebra team. Your copy stays; it can't be sent. You can keep using it as your own bank.", vec![Unlink]));
    let mut closed = l.clone();
    closed.seen.circle_closed = true;
    assert_eq!(strip_view(&input(&closed, &unsent)).line, "The circle Algebra team was closed. Your copy stays; it can't be sent. You can keep using it as your own bank.");
    // No raw codes, no student or legal claims anywhere in the strip.
    for s in [&in_step, &unsent] {
        for link in [&l, &behind, &f, &vf, &held, &removed, &gone, &closed] {
            let line = strip_view(&input(link, s)).line;
            assert!(!line.contains('_') && !line.to_lowercase().contains("student") && !line.contains("copyright"), "{line}");
        }
    }
}

#[test]
fn a_viewers_form_link_never_offers_or_makes_a_send_even_after_a_promotion() {
    let (bank, mut l) = linked(&[(QA, "One")]);
    l.mode = LinkMode::Follow;
    l.viewers_form = true;
    l.role_seen = Some("editor".into());
    l.head_seen = 2;
    let st = status(0, 1, true, false);
    for check in [Check::Done, Check::NotYet] {
        let v = strip_view(&StripInput { check, ..input(&l, &st) });
        assert!(!v.buttons.contains(&StripButton::Send), "{v:?}");
        assert!(v.buttons.contains(&StripButton::AddFullCopy), "a promoted viewer is offered a full copy: {v:?}");
    }
    assert_eq!(perm_of(&l).role, Role::Viewer, "the screens get a viewer's permission");
    // A follow link made from the full form, its person now an editor: still no send from this copy.
    let mut f = l.clone();
    f.viewers_form = false;
    for st in [status(0, 0, true, false), status(1, 1, true, false), status(0, 0, false, false)] {
        let v = strip_view(&input(&f, &st));
        assert!(!v.buttons.contains(&StripButton::Send), "{v:?}");
    }
    // The model refuses too: no send page, and a forced one sends nothing.
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, heads(1));
    let mut m = edit_model();
    m.add_link(l).unwrap();
    m.open_send(LOCAL, Perm { role: Role::Editor, is_author: false, closed: false });
    assert!(!m.active(), "no send page for a copy that follows the circle");
    m.view = EditView::Send(Box::new(SendView {
        local_id: LOCAL.into(),
        cid: CID.into(),
        bid: BID.into(),
        title: "Cells".into(),
        perm: Perm { role: Role::Editor, is_author: false, closed: false },
        message: String::new(),
        changes: None,
        stage: SendStage::Editing,
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
    }));
    assert_eq!(m.send_blocker(), Some(FOLLOW_NO_SEND));
    m.send();
    let mut banks = vec![bank];
    m.frame(now(), &ready(&s));
    m.sync(&mut banks);
    m.frame(now(), &ready(&s));
    assert!(!s.paths().iter().any(|p| p.starts_with("Put ")), "{:?}", s.paths());
}

#[test]
fn a_follow_link_gets_the_latest_without_merge_initiate() {
    let (bank, mut l) = linked(&[(QA, "One")]);
    l.mode = LinkMode::Follow;
    let s = Script::new();
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(2, &[(QA, "One, reworded"), (QB, "Two")], false));
    let mut m = edit_model();
    m.add_link(l.clone()).unwrap();
    let mut banks = vec![bank];
    m.frame(now(), &ready(&s));
    m.open_merge(LOCAL, perm_of(&l));
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
        m.sync(&mut banks);
    }
    assert!(!s.paths().iter().any(|p| p.ends_with("/merge")), "{:?}", s.paths());
    assert!(matches!(&m.view, EditView::Merge(v) if matches!(v.phase, MergePhase::Review(_))), "the same review as a working copy: {:?}", m.view);
    m.apply_merge();
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
        m.sync(&mut banks);
    }
    assert!(matches!(&m.view, EditView::Merge(v) if matches!(v.phase, MergePhase::Applied(_))));
    assert_eq!(banks[0].questions.len(), 2);
    assert_eq!(m.link_for_local(LOCAL).unwrap().base_version, 2);
    // Nothing newer: "you already have it", still without merge-initiate.
    m.cancel_merge();
    m.open_merge(LOCAL, perm_of(&l));
    m.frame(now(), &ready(&s));
    assert!(matches!(&m.view, EditView::Merge(v) if matches!(v.phase, MergePhase::UpToDate)));
}

// ---- the check ------------------------------------------------------------------------------------------------------

#[test]
fn the_check_runs_only_signed_in_at_most_every_ten_minutes_and_on_check_now() {
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, heads(3));
    s.keep("Get", "/circles", 200, circles_list("editor"));
    let mut c = checks();
    let mut m = edit_model();
    let (bank, l) = linked(&[(QA, "One")]);
    m.add_link(l).unwrap();
    let count = |s: &Arc<Script>| s.paths().iter().filter(|p| *p == "Post /circles/heads").count();
    // Signed out: nothing.
    let t0 = now();
    for g in [Gate::SignedOut, Gate::Checking, Gate::NoServer, Gate::Unreachable] {
        let v = c.strip(&mut m, &bank, &AccountSnapshot::of_gate(g.clone()), t0, NOW).unwrap();
        assert!(!v.busy);
        // (a session left over while the account is being looked up again, or signed out, is not used)
        let v = c.strip(&mut m, &bank, &AccountSnapshot { gate: g, ..ready(&s) }, t0, NOW).unwrap();
        assert!(!v.busy);
    }
    assert!(s.seen().is_empty());
    // Signed in: one check, applied on the next frame.
    let v = c.strip(&mut m, &bank, &ready(&s), t0, NOW).unwrap();
    assert!(v.busy, "{v:?}");
    c.frame(&mut m, NOW);
    assert_eq!(count(&s), 1);
    assert_eq!(s.last().path, "/circles", "and the circles list, for the role");
    let v = c.strip(&mut m, &bank, &ready(&s), t0, NOW).unwrap();
    assert_eq!(v.line, "Shared with Algebra team. Version 3 is the latest. You have version 1.");
    let saved = m.link_for_local(LOCAL).unwrap();
    assert_eq!((saved.head_seen, saved.role_seen.as_deref()), (3, Some("editor")));
    // Within 10 minutes: no second request, however many frames.
    for k in 0..30 {
        c.strip(&mut m, &bank, &ready(&s), t0 + MIN * (k % 9), NOW);
        c.frame(&mut m, NOW);
    }
    assert_eq!(count(&s), 1);
    // After 10 minutes: once more.
    c.strip(&mut m, &bank, &ready(&s), t0 + MIN * 10, NOW);
    c.frame(&mut m, NOW);
    assert_eq!(count(&s), 2);
    // "Check now": at once.
    c.check_now(LOCAL);
    c.strip(&mut m, &bank, &ready(&s), t0 + MIN * 11, NOW);
    c.frame(&mut m, NOW);
    assert_eq!(count(&s), 3);
}

#[test]
fn a_check_writes_what_it_saw_into_the_link() {
    let (_, l) = linked(&[(QA, "One")]);
    let list = |role: &str, closed: bool| -> Vec<CircleSummary> { serde_json::from_value(json!([summary(role, json!({"closed_at": if closed { json!(NOW) } else { Value::Null }}))])).unwrap() };
    let entry = |state: HeadState| vec![HeadEntry { bank_id: BID.into(), state }];
    let mut a = l.clone();
    assert!(apply_check(&mut a, Some(&entry(HeadState::Current { version: 4, license: None, at: Some(NOW), by: Some("B".into()), hide_key: None })), Some(&list("viewer", false)), NOW));
    assert_eq!((a.head_seen, a.role(), a.gone()), (4, Some(Role::Viewer), false));
    let mut b = l.clone();
    apply_check(&mut b, Some(&entry(HeadState::Removed { until: Some(NOW + DAY) })), Some(&list("editor", false)), NOW);
    assert!(b.seen.removed && b.seen.removed_until == Some(NOW + DAY));
    let mut c = l.clone();
    apply_check(&mut c, Some(&entry(HeadState::Unavailable)), Some(&[]), NOW);
    assert!(c.gone() && c.role_seen.as_deref() == Some(ROLE_NONE), "left or removed");
    let mut d = l.clone();
    apply_check(&mut d, Some(&entry(HeadState::Unavailable)), Some(&list("editor", true)), NOW);
    assert!(d.gone() && d.seen.circle_closed, "the circle closed");
    let mut e = l.clone();
    apply_check(&mut e, Some(&entry(HeadState::UnderReview)), Some(&list("editor", false)), NOW);
    assert!(e.seen.under_review);
    // Back in the circle, the bank current again: every mark goes.
    apply_check(&mut c, Some(&entry(HeadState::Current { version: 1, license: None, at: None, by: None, hide_key: None })), Some(&list("editor", false)), NOW);
    assert!(!c.gone() && !c.seen.removed);
}

#[test]
fn another_account_drops_what_was_read_and_asks_again() {
    let (mut cm, s) = open_circle("editor", json!({}));
    s.keep("Post", "/circles/heads", 200, heads(2));
    let (bank, l) = linked(&[(QA, "One")]);
    cm.edit.add_link(l).unwrap();
    let as_ = |who: &str| AccountSnapshot { account: Some(format!("https://pool.test\n{who}@school.org")), ..ready(&s) };
    cm.note_account(&as_("olga"));
    let t0 = now();
    let n = s.seen().len();
    cm.links.strip(&mut cm.edit, &bank, &as_("olga"), t0, NOW);
    cm.links.frame(&mut cm.edit, NOW);
    assert_eq!(cm.links.check_of(LOCAL), Check::Done);
    assert!(!cm.links.due(LOCAL, t0 + MIN));
    // Another account signs in: the check's state and the 10-minute rule go.
    cm.note_account(&as_("pete"));
    assert_eq!(cm.links.check_of(LOCAL), Check::NotYet);
    assert!(cm.links.due(LOCAL, t0 + MIN));
    cm.links.strip(&mut cm.edit, &bank, &as_("pete"), t0 + MIN, NOW);
    assert_eq!(s.paths()[n..].iter().filter(|p| *p == "Post /circles/heads").count(), 2, "asked again for the new account");
}

// ---- add to my banks, make my own copy --------------------------------------------------------------------------------

pub(super) fn add_as(role: &str, keys_hidden: bool, own: bool) -> (CirclesModel, Arc<Script>, Vec<QuestionBank>, BrowseModel) {
    let extra = if keys_hidden { json!({"banks": [{"id": BID, "title": "Cells", "subject": "Biology", "license": "CC-BY-4.0", "author_name": "Mr Keesler", "is_author": false, "head_version": 2, "question_count": 2, "created_at": NOW - DAY, "updated_at": NOW - DAY, "hide_key": true}]}) } else { json!({}) };
    let (mut m, s) = open_circle(role, extra);
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(2, &[(QA, "One"), (QB, "Two")], keys_hidden));
    let mut banks: Vec<QuestionBank> = Vec::new();
    let mut browse = BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), ImportLog::memory());
    m.add_bank(BID, own);
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
        m.sync(&mut banks, &mut browse);
    }
    (m, s, banks, browse)
}

#[test]
fn add_to_my_banks_makes_an_edit_link_for_editors_and_a_follow_link_for_viewers() {
    for (role, hidden, mode, form) in [("owner", false, LinkMode::Edit, false), ("editor", false, LinkMode::Edit, false), ("viewer", false, LinkMode::Follow, false), ("viewer", true, LinkMode::Follow, true)] {
        let (mut m, _s, banks, mut browse) = add_as(role, hidden, false);
        assert_eq!(banks.len(), 1, "{role}");
        let l = m.edit.link_for_local(&banks[0].id).expect("linked");
        assert_eq!((l.mode, l.viewers_form), (mode, form), "{role} {hidden}");
        assert_eq!(l.can_send(), mode == LinkMode::Edit && !form);
        assert_eq!(l.qids.len(), 2, "the circle's ids are kept");
        assert_eq!(l.role_seen.as_deref(), Some(role));
        assert!(matches!(&m.add_state(CID, BID).unwrap().state, AddState::Done { linked: true, .. }));
        // (critic 66 I2) not listed under Things I imported: its strip says where it stands
        assert!(browse.circle_bank_ids().is_empty());
        // A viewers' form copy has no keys: it is marked so.
        assert_eq!(banks[0].questions.iter().all(crate::model::is_keyless), hidden);
    }
}

#[test]
fn make_my_own_copy_leaves_no_link_and_says_where_it_came_from() {
    let (mut m, _s, banks, mut browse) = add_as("editor", false, true);
    assert_eq!(banks.len(), 1);
    assert!(m.edit.link_for_local(&banks[0].id).is_none(), "not linked");
    assert!(m.edit.links().is_empty());
    let d = &banks[0].details.description;
    assert!(d.starts_with("From the circle \"Biology team\": \"Cells\" by Mr Keesler, version 2, ") && d.ends_with(" 2025."), "{d}");
    assert!(browse.circle_bank_ids().is_empty(), "Q7: no update notices for an own copy");
    assert!(matches!(&m.add_state(CID, BID).unwrap().state, AddState::Done { linked: false, .. }));
}

#[test]
fn a_viewer_may_not_add_a_hidden_or_removed_bank_and_one_add_runs_at_a_time() {
    let (mut m, s) = open_circle("viewer", json!({"banks": [{"id": BID, "title": "", "hidden": true, "head_version": 1, "question_count": 1, "created_at": NOW, "updated_at": NOW, "removed": {"at": NOW, "reason": "owner_removed", "undo_until": NOW + DAY}}]}));
    let n = s.seen().len();
    m.add_bank(BID, false);
    m.add_bank(BID, true);
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
    }
    assert!(m.adds.is_empty() && s.seen().len() == n);
}

// ---- joining --------------------------------------------------------------------------------------------------------

#[test]
fn the_join_offer_ticks_every_bank_up_to_ten_and_none_above() {
    assert!(join_ticks(0) && join_ticks(3) && join_ticks(10));
    assert!(!join_ticks(11));
    for (n, ticked) in [(3usize, true), (11, false)] {
        let s = Script::new();
        s.keep("Get", "/circles", 200, json!({"circles": []}));
        s.on("Post", "/circles/join", 200, json!({"circle_id": CID, "role": "viewer", "already": false}));
        s.on("Post", "/circles/join/preview", 200, json!({"circle": {"id": CID, "name": "Algebra team", "owner_name": "Ms Frizzle", "members": 3}, "role": "viewer", "already_member": false}));
        let banks: Vec<Value> = (0..n).map(|i| json!({"id": hex(100 + i as u64), "title": format!("Bank {i}"), "head_version": 1, "question_count": 1, "created_at": NOW, "updated_at": NOW})).collect();
        s.keep("Get", &format!("/circles/{CID}"), 200, detail("viewer", json!({"banks": banks})));
        let mut m = model();
        m.open_join("C-ABCDEFGHJKMNPQRSTVWXYZ01");
        m.frame(now(), &ready(&s));
        m.look_up_code();
        m.frame(now(), &ready(&s));
        m.confirm_join();
        for _ in 0..3 {
            m.frame(now(), &ready(&s));
        }
        let o = m.join_offer.as_ref().expect("the offer");
        let b = o.banks.as_ref().expect("its banks");
        assert_eq!(b.len(), n);
        assert!(b.iter().all(|x| x.ticked == ticked), "{n}");
        assert_eq!(o.role, Role::Viewer);
    }
}

#[test]
fn the_ticked_banks_are_added_one_after_another() {
    let s = Script::new();
    s.keep("Get", "/circles", 200, json!({"circles": []}));
    s.on("Post", "/circles/join/preview", 200, json!({"circle": {"id": CID, "name": "Algebra team", "owner_name": "Ms Frizzle", "members": 3}, "role": "editor", "already_member": false}));
    s.on("Post", "/circles/join", 200, json!({"circle_id": CID, "role": "editor", "already": false}));
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("editor", json!({})));
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(2, &[(QA, "One")], false));
    let mut m = model();
    m.open_join("C-ABCDEFGHJKMNPQRSTVWXYZ01");
    m.frame(now(), &ready(&s));
    m.look_up_code();
    m.frame(now(), &ready(&s));
    m.confirm_join();
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
    }
    let mut banks: Vec<QuestionBank> = Vec::new();
    let mut browse = BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), ImportLog::memory());
    m.add_ticked();
    for _ in 0..4 {
        m.frame(now(), &ready(&s));
        m.sync(&mut banks, &mut browse);
    }
    assert_eq!(banks.len(), 1);
    assert_eq!(m.edit.link_for_local(&banks[0].id).unwrap().mode, LinkMode::Edit);
}

// ---- a bank shared into a circle --------------------------------------------------------------------------------------

#[test]
fn a_bank_shared_into_a_circle_becomes_the_linked_bank_at_version_1() {
    let s = Script::new();
    let b = json!({"id": "b1", "name": "Unit 1", "questions": [qjson("q0", "Zero"), qjson("q1", "One")], "created": 1, "modified": 2, "details": {}});
    let src = ShareSource::from_bank_json("bank:b1", "Bank: Unit 1", "Unit 1", &b.to_string());
    let mut p = PublishModel::new(Runner::Inline, Arc::new(HashMap::<String, Vec<u8>>::new()), LogIo::memory()).with_retry(RetryPolicy::none());
    p.start(src);
    p.frame(now(), &ready(&s));
    assert_eq!(p.step, Step::Options);
    s.keep("Get", "/circles", 200, json!({"circles": [summary("editor", json!({"name": "Algebra team"}))]}));
    p.circle.activate(None);
    p.frame(now(), &ready(&s));
    p.frame(now(), &ready(&s));
    p.circle.choose(CID);
    p.circle.license = Some(License::CcBy);
    p.circle.attest = true;
    s.act("Post", "/banks", 201, json!({"circle_id": CID, "bank_id": BID, "version": 1, "version_id": hex(1), "qids": [QA, QB], "added": 2, "changed": 0, "removed": 0, "replayed": false}));
    p.send_to_circle();
    p.frame(now(), &ready(&s));
    let info = match &p.circle.stage {
        super::circle_send::SendStage::Done(i) => (**i).clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(info.linked.as_deref(), Some("Unit 1"));
    // The circles model takes it on its next read (the Banks list, a bank's page, the Shared page).
    let mut m = edit_model();
    let l = m.link_for_local("b1").expect("the shared bank is the linked bank");
    assert_eq!((l.circle_id.as_str(), l.circle_bank_id.as_str(), l.base_version, l.mode), (CID, BID, 1, LinkMode::Edit));
    assert_eq!(l.qid_of("q0"), Some(QA));
    assert_eq!(l.qid_of("q1"), Some(QB));
    assert!(l.is_author && l.can_send());
    assert_eq!(outcome_of(BID), Some(LinkOutcome::Linked));
    // The bank, unchanged since, is in step: nothing to send.
    let bank: QuestionBank = serde_json::from_value(b).unwrap();
    let st = m.status_of(&bank).unwrap();
    assert!(st.changes.known && st.changes.total() == 0 && !st.stale);
    // An exam shared into a circle links nothing.
    let mut q = super::circle_send::CircleSendModel::default();
    q.activate(None);
    let exam_src = ShareSource::from_bank_json("exam:e1", "Exam: E", "E", &json!({"id": "e1", "name": "E", "questions": [qjson("q0", "Zero")], "created": 1, "modified": 2}).to_string());
    assert!(exam_src.bank().is_none());
}

#[test]
fn a_bank_already_linked_keeps_its_link() {
    let (bank, l) = linked(&[(QA, "One")]);
    let mut m = edit_model();
    m.add_link(l.clone()).unwrap();
    let sent = SentBank { circle_id: CID.into(), bank_id: hex(9), version: 1, version_id: hex(1), qids: vec![QB.into()], ..SentBank::default() };
    let other = link_from_share(CID, "Algebra team", Role::Owner, &bank, &sent, &ItemMeta { title: "X".into(), ..ItemMeta::default() }, License::CcBy, NOW).unwrap();
    queue_link(other);
    assert_eq!(m.links().len(), 1);
    assert_eq!(m.link_for_local(LOCAL).unwrap().circle_bank_id, BID, "the first link stays");
    assert_eq!(outcome_of(&hex(9)), Some(LinkOutcome::AlreadyLinked));
    // Not matched to the bank's questions: no link.
    let short = SentBank { qids: vec![], ..sent };
    assert!(link_from_share(CID, "Algebra team", Role::Owner, &bank, &short, &ItemMeta::default(), License::CcBy, NOW).is_none());
}

// ---- the screens --------------------------------------------------------------------------------------------------

pub(super) fn text_of(width: f32, draw: &mut dyn FnMut(&mut egui::Ui)) -> String {
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 6000.0))), time: Some(0.0), ..Default::default() };
    let mut all = String::new();
    for _ in 0..2 {
        let out = ctx.run_ui(input.clone(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| draw(ui));
        });
        all.clear();
        for s in &out.shapes {
            if let egui::epaint::Shape::Text(t) = &s.shape {
                // (round 18) a non-breaking space keeps "Easy -> Hard" on one line: read as a space
                all.push_str(&t.galley.text().replace('\u{a0}', " "));
                all.push('\n');
            }
        }
        out.drop_without_applying_deltas();
    }
    all
}

#[test]
fn the_circle_page_is_a_doorway_with_add_and_copy_and_nothing_to_edit() {
    for width in [1280.0, 760.0, 420.0] {
        let (mut m, _s) = open_circle("viewer", json!({}));
        let t = text_of(width, &mut |ui| {
            super::circles_ui::body_of(ui, &mut m);
        });
        assert!(t.contains("Add to My Banks") && t.contains("Make my own copy"), "{t}");
        for gone in ["Send my changes", "Get latest", "My working copies", "Get a copy", "Bank settings", "Remove..."] {
            assert!(!t.contains(gone), "{gone} at {width}: {t}");
        }
        assert!(t.contains("Report..."), "a member who is not the author may report: {t}");
    }
}

#[test]
fn add_to_my_banks_from_the_row_and_open_in_my_banks() {
    let (mut m, s) = open_circle("viewer", json!({}));
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(2, &[(QA, "One")], false));
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    assert!(super::circles_phasec_tests::click_text(&ctx, 1280.0, "Add to My Banks", 0, &mut |ui| {
        super::circles_ui::body_of(ui, &mut m);
    }));
    let mut banks: Vec<QuestionBank> = Vec::new();
    let mut browse = BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), ImportLog::memory());
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
        m.sync(&mut banks, &mut browse);
    }
    let local = banks[0].id.clone();
    assert_eq!(m.edit.link_for_local(&local).unwrap().mode, LinkMode::Follow, "a viewer's link follows");
    let t = text_of(1280.0, &mut |ui| {
        super::circles_ui::body_of(ui, &mut m);
    });
    assert!(t.contains("In My Banks as \"Cells\"") && t.contains("Open in My Banks") && !t.contains("Add to My Banks"), "{t}");
    let mut got = super::circles_ui::Outcome::None;
    assert!(super::circles_phasec_tests::click_text(&ctx, 420.0, "Open in My Banks", 0, &mut |ui| {
        let o = super::circles_ui::body_of(ui, &mut m);
        if o != super::circles_ui::Outcome::None {
            got = o;
        }
    }));
    assert_eq!(got, super::circles_ui::Outcome::OpenBank(local));
}

#[test]
fn bank_settings_holds_no_questions_and_no_send_or_merge() {
    let (mut m, s) = open_circle("owner", json!({}));
    let t = text_of(1280.0, &mut |ui| {
        super::circles_ui::body_of(ui, &mut m);
    });
    assert!(t.contains("Bank settings") && t.contains("Remove..."), "the owner's row: {t}");
    s.say_bytes(bank_body(&["First stem", "Second stem"]));
    m.open_bank(BID);
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    for width in [1280.0, 760.0, 420.0] {
        let t = text_of(width, &mut |ui| {
            super::circles_ui::body_of(ui, &mut m);
        });
        assert!(t.contains("Bank settings") && t.contains("History of versions") && t.contains("Hide answer keys from viewers"), "{t}");
        for gone in ["First stem", "Send my changes", "Get latest", "Get a copy", "Make a working copy"] {
            assert!(!t.contains(gone), "{gone}: {t}");
        }
    }
}

#[test]
fn report_from_the_row_opens_the_form_under_it() {
    let (mut m, s) = open_circle("editor", json!({}));
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    let n = s.seen().len();
    assert!(super::circles_phasec_tests::click_text(&ctx, 760.0, "Report...", 0, &mut |ui| {
        super::circles_ui::body_of(ui, &mut m);
    }));
    let t = text_of(760.0, &mut |ui| {
        super::circles_ui::body_of(ui, &mut m);
    });
    assert!(t.contains("What is wrong?"), "{t}");
    m.frame(now(), &ready(&s));
    assert_eq!(s.seen().len(), n, "the bank is not read for a report");
}

#[test]
fn a_deep_link_to_a_bank_lands_on_its_circle() {
    let s = Script::new();
    s.keep("Get", "/circles", 200, json!({"circles": [summary("editor", json!({}))]}));
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("editor", json!({})));
    s.keep("Get", &format!("/circles/{CID}/activity"), 200, json!({"activity": [], "next_before": null}));
    let mut m = model();
    m.open_bank_at(CID, BID);
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
    }
    assert_eq!(m.screen, super::circles::Screen::Circle);
    assert_eq!(m.scroll_to.as_deref(), Some(BID));
    m.open_bank_at(CID, &format!("{BID}:settings"));
    assert!(m.bank.is_some());
}

#[test]
fn the_join_offer_draws_its_ticks_and_buttons() {
    let s = Script::new();
    s.keep("Get", "/circles", 200, json!({"circles": []}));
    s.on("Post", "/circles/join/preview", 200, json!({"circle": {"id": CID, "name": "Algebra team", "owner_name": "Ms Frizzle", "members": 3}, "role": "editor", "already_member": false}));
    s.on("Post", "/circles/join", 200, json!({"circle_id": CID, "role": "editor", "already": false}));
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("editor", json!({})));
    let mut m = model();
    m.open_join("C-ABCDEFGHJKMNPQRSTVWXYZ01");
    m.frame(now(), &ready(&s));
    m.look_up_code();
    m.frame(now(), &ready(&s));
    m.confirm_join();
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
    }
    for width in [1280.0, 760.0, 420.0] {
        let t = text_of(width, &mut |ui| {
            super::circles_ui::body_of(ui, &mut m);
        });
        assert!(t.contains("Add its banks to My Banks?") && t.contains("Cells") && t.contains("Add the ticked banks") && t.contains("Not now"), "{t}");
    }
}

#[test]
fn every_strip_and_badge_draws_at_every_width() {
    let (_, l) = linked(&[(QA, "One"), (QB, "Two")]);
    let st = status(1, 1, true, false);
    let mut links = vec![l.clone()];
    for f in [
        |x: &mut CircleLink| x.head_seen = 4,
        |x: &mut CircleLink| x.mode = LinkMode::Follow,
        |x: &mut CircleLink| x.viewers_form = true,
        |x: &mut CircleLink| x.seen.removed = true,
        |x: &mut CircleLink| x.seen.under_review = true,
        |x: &mut CircleLink| x.role_seen = Some(ROLE_NONE.into()),
    ] {
        let mut x = l.clone();
        f(&mut x);
        links.push(x);
    }
    for width in [1280.0, 760.0, 420.0] {
        for x in &links {
            for check in [Check::Done, Check::Checking, Check::Offline] {
                let v = strip_view(&StripInput { check, ..input(x, &st) });
                let t = text_of(width, &mut |ui| {
                    super::bank_links_ui::draw_strip(ui, &v, &mut |_| {}, &mut |_| {}, Some((Some(unlink_question(Some(0), true)), Vec::new())));
                    super::bank_links_ui::badge_line(ui, &badge_of(x, Some(&st)), badge_warns(x));
                });
                assert!(t.contains("Circle") && v.buttons.iter().all(|b| t.contains(b.label())), "{width}: {t}");
            }
        }
        let t = text_of(width, &mut |ui| super::bank_links_ui::draw_strip(ui, &unreadable_view(), &mut |_| {}, &mut |_| {}, None));
        assert!(t.contains("couldn't be read") && t.contains("Try again"));
    }
}

