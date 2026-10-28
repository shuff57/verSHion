//! Phase D1, critics 63 and 64: the fixes, each test written first against the code it pins. Data safety first
//! (a full copy never takes the viewers' form, Get latest on a viewers'-form copy finishes, the join offer never
//! takes a link from a copy that has unsent work, one account's check never marks another account's link, a
//! signed-out answer does not start a check every frame, a promoted viewer's copy, the author's restore and trim
//! on links from before phase D), then the wording and the screens.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use super::bank_links::*;
use super::bank_links_tests::*;
use super::browse::{BrowseModel, ImportLog};
use super::circle_edit::*;
use super::circle_links::*;
use super::circles::CirclesModel;
use super::circles_api::*;
use super::circles_tests::*;
use super::merge::{Choice, RowKind, base_of};
use super::model::Runner;
use super::pool::*;
use super::publish::AccountSnapshot;
use crate::model::QuestionBank;

pub(super) const QC: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa3";

/// A full-form question with its key and explanation.
pub(super) fn qfull(id: &str, stem: &str) -> Value {
    json!({"id": id, "stem": stem, "kind": {"Mc": {"answers": [{"text": "A", "correct": true}, {"text": "B", "correct": false}]}}, "points": 1.0, "difficulty": null, "explanation": "Because A.", "created": 5, "modified": 6})
}

/// The viewers' form as the pool serves it: every `correct:false`, `explanation:null`, `keyless:true`.
pub(super) fn qstripped(id: &str, stem: &str) -> Value {
    json!({"id": id, "stem": stem, "kind": {"Mc": {"answers": [{"text": "A", "correct": false}, {"text": "B", "correct": false}]}}, "points": 1.0, "difficulty": null, "explanation": null, "keyless": true, "created": 5, "modified": 6})
}

pub(super) fn body_of(qs: Vec<Value>) -> Vec<u8> {
    json!({"schema_version": 2, "bank": {"id": BID, "name": "Cells", "questions": qs, "created": 1, "modified": 2}}).to_string().into_bytes()
}

pub(super) fn served_qs(version: u64, qs: Vec<Value>, keys_hidden: bool) -> Vec<u8> {
    let n = qs.len();
    let head = json!({"circle": {"id": CID, "name": "Biology team"}, "bank": {"id": BID, "title": "Cells", "subject": "Biology", "course": "", "grade": "", "license": "CC-BY-4.0", "version_license": "CC-BY-4.0", "author_name": "Mr Keesler", "is_author": false, "version": version, "version_id": hex(version), "question_count": n, "has_images": false, "committed_at": NOW - DAY, "hide_key": keys_hidden, "keys_hidden": keys_hidden}, "origins": null});
    let h = head.to_string();
    let p = String::from_utf8(body_of(qs)).unwrap();
    format!("{},\"payload\":{}}}", &h[..h.len() - 1], p).into_bytes()
}

/// A bank in step with version 1 in the full form (keys and explanations), and its link.
pub(super) fn linked_full(items: &[(&str, &str)]) -> (QuestionBank, CircleLink) {
    linked_from(items.iter().map(|(q, s)| qfull(q, s)).collect(), false)
}

/// A bank and its link made from version 1 of `qs`; `stripped`: read in the viewers' form.
pub(super) fn linked_from(qs: Vec<Value>, stripped: bool) -> (QuestionBank, CircleLink) {
    let theirs = circle_theirs_as(&body_of(qs), NOW, stripped);
    let bank = QuestionBank { id: LOCAL.into(), name: "Unit 2 quiz".into(), questions: theirs.iter().map(|t| t.q.clone()).collect(), created: NOW / 1000, modified: NOW / 1000, details: Default::default() };
    let head = BankHead { id: BID.into(), title: "Cells".into(), license: "CC-BY-4.0".into(), version: 1, version_id: hex(1), committed_at: NOW - DAY, ..BankHead::default() };
    let base = theirs.iter().map(|t| base_of(&t.q, &t.qid, &t.fp, t.sha.clone())).collect();
    let mut l = CircleLink::from_head(CID, "Algebra team", &head, LOCAL, base, NOW);
    l.mark_bank(&bank);
    if stripped {
        l.mode = LinkMode::Follow;
        l.viewers_form = true;
    }
    (bank, l)
}

pub(super) fn turn(m: &mut CircleEditModel, s: &Arc<Script>, banks: &mut [QuestionBank], n: usize) {
    for _ in 0..n {
        m.frame(now(), &ready(s));
        m.sync(banks);
    }
}

pub(super) fn phase(m: &CircleEditModel) -> String {
    match &m.view {
        EditView::Merge(v) => match &v.phase {
            MergePhase::Review(r) => format!("Review({:?}) {:?}", r.plan.rows.iter().map(|x| (x.kind, x.choice)).collect::<Vec<_>>(), r.problem),
            MergePhase::Applied(d) => format!("Applied({:?})", d.outcome),
            MergePhase::Failed(p) => format!("Failed({})", p.message),
            MergePhase::Blocked(s) => format!("Blocked({s})"),
            other => format!("{other:?}").chars().take(160).collect(),
        },
        other => format!("{other:?}").chars().take(160).collect(),
    }
}

pub(super) fn has_keys(bank: &QuestionBank) -> bool {
    bank.questions.iter().all(|q| !crate::model::is_keyless(q) && q.explanation.as_deref() == Some("Because A."))
}

// ---- B1: a full copy never takes the viewers' form ------------------------------------------------------------------

/// A full copy (keys and explanations, one unsent edit), the bank now hiding its keys from viewers, and the person
/// a viewer: the newest version is read in the viewers' form.
fn keys_hidden_case(mode: LinkMode) -> (CircleEditModel, Arc<Script>, Vec<QuestionBank>, CircleLink) {
    let (mut bank, mut l) = linked_full(&[(QA, "One"), (QB, "Two")]);
    l.mode = mode;
    l.role_seen = Some("viewer".into());
    bank.questions[1].stem = "Two (my wording)".into();
    bank.modified += 5;
    let s = Script::new();
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served_qs(2, vec![qstripped(QA, "One"), qstripped(QB, "Two"), qstripped(QC, "Three")], true));
    let mut m = edit_model();
    m.add_link(l.clone()).unwrap();
    (m, s, vec![bank], l)
}

#[test]
fn b1_get_latest_never_puts_the_viewers_form_into_a_copy_that_has_its_keys() {
    for mode in [LinkMode::Edit, LinkMode::Follow] {
        let (mut m, s, mut banks, l) = keys_hidden_case(mode);
        let before = m.link_for_local(LOCAL).unwrap();
        m.frame(now(), &ready(&s));
        m.open_merge(LOCAL, perm_of(&l));
        turn(&mut m, &s, &mut banks, 3);
        let p = phase(&m);
        assert!(p.starts_with("Blocked(") && p.contains("answer keys") && p.contains("nothing was changed"), "{mode:?}: {p}");
        // Nothing can be applied from here.
        m.apply_merge();
        turn(&mut m, &s, &mut banks, 3);
        assert!(has_keys(&banks[0]), "{mode:?}: every key and explanation stays");
        assert_eq!(banks[0].questions[1].stem, "Two (my wording)");
        assert_eq!(banks[0].questions.len(), 2);
        assert_eq!(m.link_for_local(LOCAL).unwrap(), before, "{mode:?}: the link is unchanged");
    }
}

#[test]
fn b1_the_strip_says_a_full_copy_of_a_bank_that_hides_its_keys_keeps_them_and_offers_no_get_latest() {
    let (bank, mut l) = linked_full(&[(QA, "One")]);
    l.role_seen = Some("viewer".into());
    l.head_seen = 2;
    let mut m = edit_model();
    m.add_link(l.clone()).unwrap();
    let st = m.status_of(&bank).unwrap();
    // Not known to hide its keys yet: Get latest is offered (the merge itself refuses, see above).
    assert!(strip_view(&input(&l, &st)).buttons.contains(&StripButton::GetLatest));
    // A check saw the bank hiding its keys: the mismatch is said, and Get latest is not offered.
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, json!({"banks": [{"bank_id": BID, "status": "current", "version": 2, "version_id": hex(2), "license": "CC-BY-4.0", "at": NOW - 1000, "by": "B. Teacher", "hide_key": true}]}));
    s.keep("Get", "/circles", 200, circles_list("viewer"));
    let mut c = checks();
    c.strip(&mut m, &bank, &ready(&s), now(), NOW);
    c.frame(&mut m, NOW);
    let l2 = m.link_for_local(LOCAL).unwrap();
    assert!(l2.seen.hide_key, "the check notes that the bank hides its keys");
    let v = strip_view(&input(&l2, &st));
    assert!(v.line.contains("hidden from viewers") && v.line.contains("your copy has them"), "{}", v.line);
    assert!(!v.buttons.contains(&StripButton::GetLatest), "{:?}", v.buttons);
    // An editor reads the full form: nothing to say.
    let mut ed = l2.clone();
    ed.role_seen = Some("editor".into());
    assert!(!strip_view(&input(&ed, &st)).line.contains("hidden from viewers"));
    // A viewers'-form copy keeps its own line.
    let mut vf = l2.clone();
    vf.viewers_form = true;
    vf.mode = LinkMode::Follow;
    assert!(strip_view(&input(&vf, &st)).line.contains("your copy has none"));
}

#[test]
fn b1_a_review_read_in_full_whose_apply_comes_back_without_keys_applies_nothing() {
    // An editor's copy: the review is read in full. Before Apply the person is made a viewer of a bank that hides
    // its keys, so the second read is the viewers' form: nothing is applied.
    let (mut bank, l) = linked_full(&[(QA, "One"), (QB, "Two")]);
    bank.modified += 1;
    let s = Script::new();
    s.on("Post", &format!("/circles/{CID}/banks/{BID}/merge"), 200, json!({"base_version": 1, "base_kept": true, "head_version": 2, "head_version_id": hex(2), "head_at": NOW, "head_by": "B. Teacher", "merge_token": "t".repeat(40), "between": []}));
    s.on_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served_qs(2, vec![qfull(QA, "One, reworded"), qfull(QB, "Two")], false));
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served_qs(2, vec![qstripped(QA, "One, reworded"), qstripped(QB, "Two")], true));
    let mut m = edit_model();
    m.add_link(l.clone()).unwrap();
    let mut banks = vec![bank];
    m.frame(now(), &ready(&s));
    m.open_merge(LOCAL, Perm { role: Role::Editor, is_author: false, closed: false });
    turn(&mut m, &s, &mut banks, 3);
    assert!(phase(&m).starts_with("Review("), "{}", phase(&m));
    m.apply_merge();
    turn(&mut m, &s, &mut banks, 3);
    let p = phase(&m);
    assert!(p.starts_with("Review(") && p.contains("answer keys"), "{p}");
    assert!(has_keys(&banks[0]) && banks[0].questions[0].stem == "One");
    assert_eq!(m.link_for_local(LOCAL).unwrap().base_version, 1);
}

// ---- B2: Get latest on a viewers'-form copy finishes ----------------------------------------------------------------

/// The viewers' form of a 4-choice question, its choices in the order given (the pool shuffles them on every read).
pub(super) fn qshuffled(id: &str, stem: &str, order: [usize; 4]) -> Value {
    let names = ["Mitochondria", "Ribosome", "Nucleus", "Golgi body"];
    let answers: Vec<Value> = order.iter().map(|i| json!({"text": names[*i], "correct": false})).collect();
    json!({"id": id, "stem": stem, "kind": {"Mc": {"answers": answers}}, "points": 1.0, "difficulty": null, "explanation": null, "keyless": true, "created": 5, "modified": 6})
}

pub(super) fn shuffled_read(version: u64, stems: [&str; 3], k: usize) -> Value {
    let orders = [[0, 1, 2, 3], [2, 0, 3, 1], [3, 2, 1, 0], [1, 3, 0, 2], [2, 3, 0, 1]];
    let qs = vec![qshuffled(QA, stems[0], orders[k % 5]), qshuffled(QB, stems[1], orders[(k + 1) % 5]), qshuffled(QC, stems[2], orders[(k + 2) % 5])];
    serde_json::from_slice(&served_qs(version, qs, true)).unwrap()
}

#[test]
fn b2_get_latest_on_a_viewers_form_copy_lists_only_what_changed_and_applies() {
    let first: Value = shuffled_read(1, ["One", "Two", "Three"], 0);
    let (bank, mut l) = linked_from(first["payload"]["bank"]["questions"].as_array().unwrap().clone(), true);
    l.role_seen = Some("viewer".into());
    let s = Script::new();
    // Every read is shuffled afresh; the owner changed one stem in version 2.
    for k in 1..5 {
        s.on_raw("Get", &format!("/circles/{CID}/banks/{BID}"), shuffled_read(2, ["One", "Two, reworded", "Three"], k).to_string().into_bytes());
    }
    let mut m = edit_model();
    m.add_link(l.clone()).unwrap();
    let mut banks = vec![bank];
    m.frame(now(), &ready(&s));
    m.open_merge(LOCAL, perm_of(&l));
    turn(&mut m, &s, &mut banks, 3);
    let EditView::Merge(v) = &m.view else { panic!() };
    let MergePhase::Review(r) = &v.phase else { panic!("{}", phase(&m)) };
    let kinds: Vec<RowKind> = r.plan.rows.iter().map(|x| x.kind).collect();
    assert_eq!(kinds, vec![RowKind::Unchanged, RowKind::TheirsChanged, RowKind::Unchanged], "a fresh shuffle is not a change");
    m.apply_merge();
    turn(&mut m, &s, &mut banks, 3);
    assert!(phase(&m).starts_with("Applied"), "{}", phase(&m));
    assert_eq!(banks[0].questions[1].stem, "Two, reworded");
    assert!(banks[0].questions.iter().all(crate::model::is_keyless));
    let after = m.link_for_local(LOCAL).unwrap();
    assert_eq!(after.base_version, 2);
    assert!(after.viewers_form && !after.can_send());
    // The body reviewed is the body applied: one read of the viewers' form, not two (each counts against a daily limit).
    let reads = s.paths().iter().filter(|p| *p == &format!("Get /circles/{CID}/banks/{BID}")).count();
    assert_eq!(reads, 1, "{:?}", s.paths());
    // The next Get latest lists nothing for a version already had.
    let _ = Choice::Take;
}

#[test]
fn b2_a_real_change_to_a_choice_is_still_listed_in_the_viewers_form() {
    let first: Value = shuffled_read(1, ["One", "Two", "Three"], 0);
    let (bank, mut l) = linked_from(first["payload"]["bank"]["questions"].as_array().unwrap().clone(), true);
    l.role_seen = Some("viewer".into());
    let mut next = shuffled_read(2, ["One", "Two", "Three"], 3);
    next["payload"]["bank"]["questions"][2]["kind"]["Mc"]["answers"][0]["text"] = json!("Cell wall");
    let s = Script::new();
    s.on_raw("Get", &format!("/circles/{CID}/banks/{BID}"), next.to_string().into_bytes());
    let mut m = edit_model();
    m.add_link(l.clone()).unwrap();
    let mut banks = vec![bank];
    m.frame(now(), &ready(&s));
    m.open_merge(LOCAL, perm_of(&l));
    turn(&mut m, &s, &mut banks, 3);
    let EditView::Merge(v) = &m.view else { panic!() };
    let MergePhase::Review(r) = &v.phase else { panic!("{}", phase(&m)) };
    let kinds: Vec<RowKind> = r.plan.rows.iter().map(|x| x.kind).collect();
    assert_eq!(kinds, vec![RowKind::Unchanged, RowKind::Unchanged, RowKind::TheirsChanged]);
}

// ---- B3: the join offer never takes a link from a copy this computer has ------------------------------------------

/// Joins the circle (again) with a code; `role` is the role given.
pub(super) fn join_with(s: &Arc<Script>, m: &mut CirclesModel, role: &str) {
    s.keep("Get", "/circles", 200, json!({"circles": []}));
    s.on("Post", "/circles/join/preview", 200, json!({"circle": {"id": CID, "name": "Algebra team", "owner_name": "Ms Frizzle", "members": 3}, "role": role, "already_member": false}));
    s.on("Post", "/circles/join", 200, json!({"circle_id": CID, "role": role, "already": false}));
    s.keep("Get", &format!("/circles/{CID}"), 200, detail(role, json!({})));
    m.open_join("C-ABCDEFGHJKMNPQRSTVWXYZ01");
    m.frame(now(), &ready(s));
    m.look_up_code();
    m.frame(now(), &ready(s));
    m.confirm_join();
    for _ in 0..3 {
        m.frame(now(), &ready(s));
    }
}

#[test]
fn b3_joining_again_never_unlinks_the_copy_this_computer_has() {
    let s = Script::new();
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(2, &[(QA, "One")], false));
    let mut m = model();
    // The copy this computer has: linked, one unsent change; the person had left (or was removed).
    let (mut old, mut l) = linked(&[(QA, "One")]);
    l.role_seen = Some(ROLE_NONE.into());
    old.questions[0].stem = "One (my unsent fix)".into();
    old.modified += 5;
    m.edit.add_link(l).unwrap();
    let mut banks: Vec<QuestionBank> = vec![old];
    assert_eq!(m.edit.status_of(&banks[0]).unwrap().changes.total(), 1);
    join_with(&s, &mut m, "editor");
    let o = m.join_offer.as_ref().expect("offer");
    let b = &o.banks.as_ref().expect("its banks")[0];
    assert!(!b.ticked, "a bank this computer has is not ticked");
    assert_eq!(b.in_banks.as_deref(), Some("Unit 2 quiz"), "it says where it is");
    let t = text_of(1280.0, &mut |ui| {
        super::circles_ui::body_of(ui, &mut m);
    });
    assert!(t.contains("already in My Banks as \"Unit 2 quiz\""), "{t}");
    let mut browse = BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), ImportLog::memory());
    // Ticking it by hand does nothing either: it is not added a second time.
    m.tick_offer(BID, true);
    m.add_ticked();
    for _ in 0..4 {
        m.frame(now(), &ready(&s));
        m.sync(&mut banks, &mut browse);
    }
    assert_eq!(banks.len(), 1, "no second copy");
    assert_eq!(m.edit.link_for_local(LOCAL).map(|l| l.circle_bank_id), Some(BID.to_string()), "the first copy keeps its link");
    assert_eq!(banks[0].questions[0].stem, "One (my unsent fix)");
}

#[test]
fn b3_a_link_is_never_moved_to_another_bank_except_by_add_a_full_copy() {
    let (bank, l) = linked(&[(QA, "One")]);
    let mut m = edit_model();
    m.add_link(l.clone()).unwrap();
    m.status_of(&bank);
    let mut other = l.clone();
    other.local_bank_id = "local-bank-2".into();
    let e = m.add_link(other.clone()).unwrap_err();
    assert!(e.contains("Unit 2 quiz") && e.contains("already"), "{e}");
    assert_eq!(m.link_for_local(LOCAL).map(|x| x.local_bank_id), Some(LOCAL.to_string()));
    assert!(m.link_for_local("local-bank-2").is_none());
    // The bank of the old link is gone from this computer: a new one may take the link.
    let mut m2 = edit_model();
    m2.add_link(l.clone()).unwrap();
    m2.sync(&mut []);
    assert!(m2.status(LOCAL).is_some_and(|s| s.missing));
    m2.add_link(other.clone()).unwrap();
    assert!(m2.link_for_local("local-bank-2").is_some());
    // "Add a full copy" replaces the link on purpose.
    let mut m3 = edit_model();
    m3.add_link(l).unwrap();
    m3.status_of(&bank);
    m3.add_link_replacing(other).unwrap();
    assert!(m3.link_for_local("local-bank-2").is_some() && m3.link_for_local(LOCAL).is_none());
}

#[test]
fn b3_add_to_my_banks_is_refused_for_a_bank_this_computer_already_has() {
    let (mut m, s) = open_circle("editor", json!({}));
    let (bank, l) = linked(&[(QA, "One")]);
    m.edit.add_link(l).unwrap();
    m.edit.status_of(&bank);
    let n = s.seen().len();
    m.add_bank(BID, false);
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
    }
    assert!(m.adds.is_empty() && s.seen().len() == n, "{:?}", m.adds);
    // Make my own copy is still offered and made.
    m.add_bank(BID, true);
    assert_eq!(m.adds.len(), 1);
}

// ---- I1: one account's check never marks another account's link -------------------------------------------------

pub(super) fn as_who(s: &Arc<Script>, who: &str) -> AccountSnapshot {
    AccountSnapshot { account: Some(format!("https://pool.test\n{who}@school.org")), ..ready(s) }
}

pub(super) fn heads_count(s: &Arc<Script>) -> usize {
    s.paths().iter().filter(|p| *p == "Post /circles/heads").count()
}

#[test]
fn i1_another_accounts_check_does_not_mark_the_link_gone() {
    let sa = Script::new();
    sa.keep("Post", "/circles/heads", 200, heads(1));
    sa.keep("Get", "/circles", 200, circles_list("editor"));
    let sb = Script::new();
    sb.keep("Post", "/circles/heads", 200, json!({"banks": [{"bank_id": BID, "status": "unavailable"}]}));
    sb.keep("Get", "/circles", 200, json!({"circles": []}));
    let mut cm = model();
    let (bank, l) = linked(&[(QA, "One")]);
    cm.edit.add_link(l).unwrap();
    let (a, b) = (as_who(&sa, "olga"), as_who(&sb, "pete"));
    let t0 = now();
    // olga's check: she is an editor; the link remembers whose role that is.
    cm.note_account(&a);
    cm.links.strip(&mut cm.edit, &bank, &a, t0, NOW);
    cm.links.frame(&mut cm.edit, NOW);
    let saved = cm.edit.link_for_local(LOCAL).unwrap();
    assert_eq!(saved.role_seen.as_deref(), Some("editor"));
    assert!(saved.role_seen_by.is_some());
    // pete signs in on the same computer and opens the bank: his check does not write "not in the circle".
    cm.note_account(&b);
    cm.links.strip(&mut cm.edit, &bank, &b, t0 + MIN, NOW);
    cm.links.frame(&mut cm.edit, NOW);
    let saved = cm.edit.link_for_local(LOCAL).unwrap();
    assert!(!saved.gone() && saved.role_seen.as_deref() == Some("editor"), "{:?}", saved.role_seen);
    let v = cm.links.strip(&mut cm.edit, &bank, &b, t0 + MIN, NOW).unwrap();
    assert!(v.line.contains("another account") && !v.buttons.contains(&StripButton::Send) && !v.buttons.contains(&StripButton::GetLatest), "{v:?}");
    // olga again: her strip is as before.
    cm.note_account(&a);
    let v = cm.links.strip(&mut cm.edit, &bank, &a, t0 + MIN * 2, NOW).unwrap();
    assert!(!v.line.contains("another account") && !v.line.contains("no longer in"), "{v:?}");
}

#[test]
fn i1_a_link_marked_gone_is_checked_again_once_after_each_sign_in() {
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, heads(1));
    s.keep("Get", "/circles", 200, circles_list("editor"));
    let mut cm = model();
    let (bank, mut l) = linked(&[(QA, "One")]);
    // Marked "not in the circle" earlier (a check of another account, before the account was remembered).
    l.role_seen = Some(ROLE_NONE.into());
    cm.edit.add_link(l).unwrap();
    let a = as_who(&s, "olga");
    cm.note_account(&a);
    let t0 = now();
    let v = cm.links.strip(&mut cm.edit, &bank, &a, t0, NOW).unwrap();
    assert!(v.busy, "checked once by itself: {v:?}");
    cm.links.frame(&mut cm.edit, NOW);
    assert!(!cm.edit.link_for_local(LOCAL).unwrap().gone(), "back in the circle");
    assert_eq!(heads_count(&s), 1);
    // ... and not every frame.
    for k in 0..30 {
        cm.links.strip(&mut cm.edit, &bank, &a, t0 + MIN * (k % 9), NOW);
        cm.links.frame(&mut cm.edit, NOW);
    }
    assert_eq!(heads_count(&s), 1);
    assert!(!cm.links.will_check(LOCAL, false, t0 + MIN));
    // Still gone after its check: not asked again by itself in this sign-in, so the page asks nothing of the account.
    let s2 = Script::new();
    s2.keep("Post", "/circles/heads", 200, json!({"banks": [{"bank_id": BID, "status": "unavailable"}]}));
    s2.keep("Get", "/circles", 200, json!({"circles": []}));
    let mut cm2 = model();
    let (bank2, mut l2) = linked(&[(QA, "One")]);
    l2.role_seen = Some(ROLE_NONE.into());
    cm2.edit.add_link(l2).unwrap();
    let a2 = as_who(&s2, "olga");
    cm2.note_account(&a2);
    for k in 0..30 {
        cm2.links.strip(&mut cm2.edit, &bank2, &a2, t0 + MIN * (11 * k), NOW);
        cm2.links.frame(&mut cm2.edit, NOW);
    }
    assert_eq!(heads_count(&s2), 1, "once per sign-in");
    assert!(!cm2.links.will_check(LOCAL, true, t0 + MIN * 400));
}

// ---- I2: a signed-out answer does not start a check every frame --------------------------------------------------

#[test]
fn i2_a_signed_out_answer_stops_the_checks_and_the_strip_says_sign_in() {
    let s = Script::new();
    for _ in 0..60 {
        s.fail(super::api::ApiError::SignedOut);
    }
    let mut c = checks();
    let mut m = edit_model();
    let (bank, l) = linked(&[(QA, "One")]);
    m.add_link(l).unwrap();
    let t0 = now();
    let mut last = None;
    for k in 0..20u64 {
        c.frame(&mut m, NOW);
        last = c.strip(&mut m, &bank, &ready(&s), t0 + Duration::from_millis(16 * k), NOW);
        c.frame(&mut m, NOW);
    }
    assert_eq!(heads_count(&s), 1, "{:?}", s.paths());
    let v = last.unwrap();
    assert!(v.line.contains("Sign in") && v.buttons == vec![StripButton::OpenAccount] && !v.busy, "{v:?}");
    // Pressing "Sign in..." (or Check now) asks once more.
    c.check_now(LOCAL);
    for k in 0..5u64 {
        c.strip(&mut m, &bank, &ready(&s), t0 + Duration::from_secs(1 + k), NOW);
        c.frame(&mut m, NOW);
    }
    assert_eq!(heads_count(&s), 2);
}

// ---- critic 64 I9: a check does not collapse the strip --------------------------------------------------------------

#[test]
fn i9_a_check_keeps_the_strip_and_its_buttons_and_adds_the_spinner() {
    let st = status(0, 1, true, false);
    let (_, mut l) = linked(&[(QA, "One"), (QB, "Two")]);
    l.role_seen = Some("editor".into());
    l.role_seen_at = Some(NOW - DAY);
    let done = strip_view(&input(&l, &st));
    let checking = strip_view(&StripInput { check: Check::Checking, ..input(&l, &st) });
    assert!(checking.busy && !done.busy);
    assert_eq!((checking.line.as_str(), &checking.buttons), (done.line.as_str(), &done.buttons));
    assert!(checking.buttons.contains(&StripButton::Send));
}

// ---- I3: a promoted viewer's copy ------------------------------------------------------------------------------------

#[test]
fn i3_a_promoted_viewers_copy_is_offered_a_full_copy_on_the_strip_and_the_circle_page() {
    let (mut bank, mut l) = linked(&[(QA, "One"), (QB, "Two")]);
    l.mode = LinkMode::Follow;
    l.role_seen = Some("editor".into());
    bank.questions[0].stem = "One, fixed".into();
    bank.modified += 5;
    let mut m = edit_model();
    m.add_link(l.clone()).unwrap();
    let st = m.status_of(&bank).unwrap();
    let v = strip_view(&input(&l, &st));
    assert!(v.line.contains("You can send changes now") && !v.line.contains("(you can view)"), "{}", v.line);
    assert_eq!(v.buttons.first(), Some(&StripButton::AddFullCopy));
    assert!(!v.buttons.contains(&StripButton::Send));
    assert_eq!(StripButton::AddFullCopy.label(), "Add a full copy...");
    // Still a viewer: no such offer.
    let mut viewer = l.clone();
    viewer.role_seen = Some("viewer".into());
    assert!(!strip_view(&input(&viewer, &st)).buttons.contains(&StripButton::AddFullCopy));
    // The circle page's row offers it too.
    let (mut cm, s) = open_circle("editor", json!({}));
    cm.edit.add_link(l.clone()).unwrap();
    cm.edit.status_of(&bank);
    let t = text_of(1280.0, &mut |ui| {
        super::circles_ui::body_of(ui, &mut cm);
    });
    assert!(t.contains("Open in My Banks") && t.contains("Add a full copy") && !t.contains("Add to My Banks"), "{t}");
    // Pressing it adds a new bank that can send; the earlier copy stays, no longer connected, and the page says so.
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(2, &[(QA, "One"), (QB, "Two")], false));
    let mut banks = vec![bank];
    let mut browse = BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), ImportLog::memory());
    cm.add_full_copy(BID);
    for _ in 0..3 {
        cm.frame(now(), &ready(&s));
        cm.sync(&mut banks, &mut browse);
    }
    assert_eq!(banks.len(), 2);
    assert_eq!(banks[0].questions[0].stem, "One, fixed", "the earlier copy is untouched");
    assert!(cm.edit.link_for_local(LOCAL).is_none());
    let nl = cm.edit.link_for_local(&banks[1].id).expect("the full copy is linked");
    assert!(nl.can_send() && nl.mode == LinkMode::Edit);
    assert_eq!(banks[1].name, "Cells (full copy)");
    let n = format!("{:?}", cm.notice);
    assert!(n.contains("Unit 2 quiz") && n.contains("no longer connected"), "{n}");
}

#[test]
fn i3_keys_arriving_in_a_viewers_form_copy_clear_its_mark() {
    let (bank, mut l) = linked_from(vec![qstripped(QA, "One")], true);
    l.role_seen = Some("editor".into());
    let s = Script::new();
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served_qs(2, vec![qfull(QA, "One"), qfull(QB, "Two")], false));
    let mut m = edit_model();
    m.add_link(l.clone()).unwrap();
    let mut banks = vec![bank];
    m.frame(now(), &ready(&s));
    m.open_merge(LOCAL, perm_of(&l));
    turn(&mut m, &s, &mut banks, 3);
    m.apply_merge();
    turn(&mut m, &s, &mut banks, 3);
    assert!(phase(&m).starts_with("Applied"), "{}", phase(&m));
    assert!(!crate::model::is_keyless(&banks[0].questions[0]), "the keys arrived");
    let after = m.link_for_local(LOCAL).unwrap();
    assert!(!after.viewers_form && !after.can_send(), "no longer the viewers' form, still a copy that follows");
    let st = m.status_of(&banks[0]).unwrap();
    let v = strip_view(&input(&after, &st));
    assert!(!v.line.contains("your copy has none"), "{}", v.line);
    assert!(v.buttons.contains(&StripButton::AddFullCopy));
}

// ---- I5: links from before phase D keep the author's restore and trim ------------------------------------------------

#[test]
fn i5_a_link_from_before_phase_d_learns_the_author_on_its_first_check() {
    // A link file written before phase D: no mode, no role, no author.
    let (bank, l) = linked(&[(QA, "One")]);
    let mut v = serde_json::to_value(&l).unwrap();
    for k in ["mode", "viewers_form", "role_seen", "role_seen_at", "role_seen_by", "is_author", "author_known", "seen"] {
        v.as_object_mut().unwrap().remove(k);
    }
    let old: CircleLink = serde_json::from_value(v).unwrap();
    assert!(!old.author_known, "not known: it is learnt, not taken as false");
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, heads(1));
    s.keep("Get", "/circles", 200, circles_list("editor"));
    let mut d = detail("editor", json!({}));
    d["banks"][0]["is_author"] = json!(true);
    s.keep("Get", &format!("/circles/{CID}"), 200, d);
    let mut m = edit_model();
    m.add_link(old).unwrap();
    let mut c = checks();
    let t0 = now();
    c.strip(&mut m, &bank, &ready(&s), t0, NOW);
    c.frame(&mut m, NOW);
    let after = m.link_for_local(LOCAL).unwrap();
    assert!(after.is_author && after.author_known, "{:?}", s.paths());
    let p = perm_of(&after);
    assert!(p.can_rewind(), "the author, an editor, may restore and trim from the bank page");
    // Known now: the next check does not read the circle again.
    let n = s.paths().iter().filter(|p| *p == &format!("Get /circles/{CID}")).count();
    c.check_now(LOCAL);
    c.strip(&mut m, &bank, &ready(&s), t0 + MIN, NOW);
    c.frame(&mut m, NOW);
    assert_eq!(s.paths().iter().filter(|p| *p == &format!("Get /circles/{CID}")).count(), n);
    // A link made in phase D knows it from the read it was made from.
    assert!(linked(&[(QA, "One")]).1.author_known);
}

#[test]
fn i5_the_circle_page_teaches_the_links_of_its_banks_who_the_author_is() {
    let mut d = json!({});
    d["banks"] = json!([{"id": BID, "title": "Cells", "subject": "Biology", "license": "CC-BY-4.0", "author_name": "Mr Keesler", "is_author": true, "head_version": 1, "question_count": 2, "created_at": NOW - DAY, "updated_at": NOW - DAY}]);
    let (_, l) = linked(&[(QA, "One")]);
    let mut old = l.clone();
    old.is_author = false;
    old.author_known = false;
    let s = Script::new();
    s.keep("Get", "/circles", 200, json!({"circles": [summary("editor", json!({}))]}));
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("editor", d));
    s.keep("Get", &format!("/circles/{CID}/activity"), 200, json!({"activity": [], "next_before": null}));
    let mut m = model();
    m.edit.add_link(old).unwrap();
    m.open_list();
    m.frame(now(), &ready(&s));
    m.open_circle(CID);
    for _ in 0..4 {
        m.frame(now(), &ready(&s));
    }
    let after = m.edit.link_for_local(LOCAL).unwrap();
    assert!(after.is_author && after.author_known);
}

// ---- I4: the behaviour critic 63's surviving mutants changed, pinned ------------------------------------------------

/// The account window as the bank's page uses it, without touching the keychain.
#[derive(Default)]
pub(super) struct FakeAccount {
    pub(super) open: bool,
    pub(super) quiet_opens: usize,
    pub(super) reveals: usize,
}

impl super::bank_links_ui::AccountHooks for FakeAccount {
    fn is_open(&self) -> bool {
        self.open
    }
    fn open_quiet(&mut self) {
        self.quiet_opens += 1;
    }
    fn reveal(&mut self) {
        self.reveals += 1;
    }
}

/// One frame of a linked bank's page (the strip, as `bankui` draws it); the text drawn.
pub(super) fn page_frame(m: &mut CirclesModel, banks: &mut [QuestionBank], acct: &mut FakeAccount, snap: &AccountSnapshot, at: web_time::Instant) -> (super::bank_links_ui::StripOut, String) {
    let mut browse = BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), ImportLog::memory());
    let mut out = Default::default();
    let t = text_of(1280.0, &mut |ui| {
        out = super::bank_links_ui::strip_with(ui, m, banks, &mut browse, acct, snap, at, NOW, LOCAL);
    });
    (out, t)
}

#[test]
fn i4_m1_the_bank_page_keeps_the_account_rule() {
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, heads(3));
    s.keep("Get", "/circles", 200, circles_list("editor"));
    // (critic 65 I2) pete's check learns whether pete is the bank's author
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("editor", json!({})));
    let mut m = model();
    let (bank, l) = linked(&[(QA, "One")]);
    m.edit.add_link(l).unwrap();
    let mut banks = vec![bank];
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    let t0 = now();
    let (_, t) = page_frame(&mut m, &mut banks, &mut acct, &as_who(&s, "olga"), t0);
    assert!(t.contains("Shared with Algebra team"), "{t}");
    page_frame(&mut m, &mut banks, &mut acct, &as_who(&s, "olga"), t0);
    assert_eq!(heads_count(&s), 1);
    assert_eq!(m.links.check_of(LOCAL), Check::Done);
    // Another account opens the same bank a minute later: what olga's check read is not used for pete.
    page_frame(&mut m, &mut banks, &mut acct, &as_who(&s, "pete"), t0 + MIN);
    assert_eq!(heads_count(&s), 2, "asked again for the other account");
    // Signed out on the page: nothing read stays.
    page_frame(&mut m, &mut banks, &mut acct, &AccountSnapshot::of_gate(super::publish::Gate::SignedOut), t0 + MIN * 2);
    assert_eq!(m.links.check_of(LOCAL), Check::NotYet);
}

#[test]
fn i4_the_bank_page_looks_the_account_up_once_not_every_frame() {
    let s2 = Script::new();
    s2.keep("Post", "/circles/heads", 200, json!({"banks": [{"bank_id": BID, "status": "unavailable"}]}));
    s2.keep("Get", "/circles", 200, json!({"circles": []}));
    let mut m = model();
    let (bank, mut l) = linked(&[(QA, "One")]);
    l.role_seen = Some(ROLE_NONE.into());
    m.edit.add_link(l).unwrap();
    let mut banks = vec![bank];
    let mut acct = FakeAccount::default();
    let t0 = now();
    for k in 0..30 {
        page_frame(&mut m, &mut banks, &mut acct, &as_who(&s2, "olga"), t0 + MIN * (11 * k));
    }
    assert_eq!(heads_count(&s2), 1);
    assert!(acct.quiet_opens <= 1, "{}", acct.quiet_opens);
}

#[test]
fn i4_m6_treat_as_in_step_from_the_strip_asks_at_once() {
    let (mut bank, l) = linked(&[(QA, "One")]);
    let s = Script::new();
    s.on("Post", &format!("/circles/{CID}/banks/{BID}/merge"), 400, json!({"error": "invalid_request", "field": "base_version"}));
    let mut m = edit_model();
    m.add_link(l).unwrap();
    m.update_link(LOCAL, |l| {
        l.bank_modified = bank.modified + 100;
        l.bank_print = "something else".into();
        true
    });
    bank.questions[0].stem = "One (put back)".into();
    let mut banks = vec![bank];
    m.sync(&mut banks);
    m.frame(now(), &ready(&s));
    m.open_merge_to_rebase(LOCAL, Perm { role: Role::Editor, is_author: false, closed: false });
    turn(&mut m, &s, &mut banks, 2);
    let EditView::Merge(v) = &m.view else { panic!() };
    assert!(matches!(v.phase, MergePhase::StaleUpToDate { ask: Some(StaleAsk::Rebase) }), "{:?}", v.phase);
}

#[test]
fn i4_m7_a_shared_bank_put_back_from_a_saved_copy_is_stale() {
    let b = json!({"id": "b1", "name": "Unit 1", "questions": [qjson("q0", "Zero")], "created": 1, "modified": 1_700_000_000, "details": {}});
    let bank: QuestionBank = serde_json::from_value(b).unwrap();
    let sent = SentBank { circle_id: CID.into(), bank_id: BID.into(), version: 1, version_id: hex(1), qids: vec![QA.into()], ..SentBank::default() };
    let l = link_from_share(CID, "Algebra team", Role::Owner, &bank, &sent, &ItemMeta { title: "Unit 1".into(), ..ItemMeta::default() }, License::CcBy, NOW).unwrap();
    assert!(!l.stale_for(&bank));
    let mut older = bank.clone();
    older.modified -= 100;
    older.questions[0].stem = "Zero, as it was".into();
    assert!(l.stale_for(&older), "a bank put back from before the share");
}

#[test]
fn i4_m2_m14_what_a_check_says_about_a_bank_it_can_not_read_and_a_review_that_ended() {
    let (_, l) = linked(&[(QA, "One")]);
    let list = |role: &str| -> Vec<CircleSummary> { serde_json::from_value(json!([summary(role, json!({}))])).unwrap() };
    let entry = |state: HeadState| vec![HeadEntry { bank_id: BID.into(), state }];
    // Unavailable while the circle lists the person (purged, or hidden): removed, not "you left".
    let mut a = l.clone();
    apply_check(&mut a, Some(&entry(HeadState::Unavailable)), Some(&list("editor")), NOW);
    assert!(a.seen.removed && !a.gone());
    // Held back for review, then current again: no longer said to be held back.
    let mut b = l.clone();
    apply_check(&mut b, Some(&entry(HeadState::UnderReview)), Some(&list("editor")), NOW);
    assert!(b.seen.under_review);
    apply_check(&mut b, Some(&entry(HeadState::Current { version: 2, license: None, at: None, by: None, hide_key: None })), Some(&list("editor")), NOW);
    assert!(!b.seen.under_review);
}

#[test]
fn i4_m12_m9_m13_the_check_notes_hiding_says_offline_and_forgets_the_last_accounts_answer() {
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, json!({"banks": [{"bank_id": BID, "status": "current", "version": 1, "version_id": hex(1), "license": "CC-BY-4.0", "at": NOW, "by": "B", "hide_key": true}]}));
    s.keep("Get", "/circles", 200, circles_list("editor"));
    let mut c = checks();
    let mut m = edit_model();
    let (bank, l) = linked(&[(QA, "One")]);
    m.add_link(l).unwrap();
    let t0 = now();
    c.strip(&mut m, &bank, &ready(&s), t0, NOW);
    c.frame(&mut m, NOW);
    assert!(m.hides_keys(BID), "the send asks about pictures first");
    // The account can not be reached: the strip says so (and that the bank works as usual).
    let v = c.strip(&mut m, &bank, &AccountSnapshot { gate: super::publish::Gate::Unreachable, ..ready(&s) }, t0 + MIN, NOW).unwrap();
    assert!(v.line.contains("Couldn't check the circle"), "{}", v.line);
    // An answer read for one account and not yet applied goes when another signs in.
    let mut c2 = checks();
    let mut m2 = edit_model();
    let (bank2, l2) = linked(&[(QA, "One")]);
    m2.add_link(l2).unwrap();
    c2.strip(&mut m2, &bank2, &ready(&s), t0, NOW);
    c2.forget();
    c2.frame(&mut m2, NOW);
    assert_eq!(m2.link_for_local(LOCAL).unwrap().role_seen, None, "not applied after the account changed");
}

#[test]
fn i4_m11_a_closed_circle_gives_no_send() {
    let (_, mut l) = linked(&[(QA, "One")]);
    l.role_seen = Some("editor".into());
    assert!(perm_of(&l).can_send());
    l.seen.circle_closed = true;
    assert!(!perm_of(&l).can_send() && perm_of(&l).closed);
}

#[test]
fn i4_m10_the_offer_lists_no_removed_or_hidden_bank() {
    let s = Script::new();
    let banks = json!([
        {"id": hex(101), "title": "Kept", "head_version": 1, "question_count": 1, "created_at": NOW, "updated_at": NOW},
        {"id": hex(102), "title": "", "hidden": true, "head_version": 1, "question_count": 1, "created_at": NOW, "updated_at": NOW},
        {"id": hex(103), "title": "Gone", "head_version": 1, "question_count": 1, "created_at": NOW, "updated_at": NOW, "removed": {"at": NOW, "reason": "owner_removed", "undo_until": NOW + DAY}}
    ]);
    let mut m = model();
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("viewer", json!({"banks": banks})));
    join_with_detail(&s, &mut m, "viewer");
    let o = m.join_offer.as_ref().unwrap();
    let ids: Vec<String> = o.banks.as_ref().unwrap().iter().map(|b| b.id.clone()).collect();
    assert_eq!(ids, vec![hex(101)]);
}

/// Joins with the circle's page already scripted.
fn join_with_detail(s: &Arc<Script>, m: &mut CirclesModel, role: &str) {
    s.keep("Get", "/circles", 200, json!({"circles": []}));
    s.on("Post", "/circles/join/preview", 200, json!({"circle": {"id": CID, "name": "Algebra team", "owner_name": "Ms Frizzle", "members": 3}, "role": role, "already_member": false}));
    s.on("Post", "/circles/join", 200, json!({"circle_id": CID, "role": role, "already": false}));
    m.open_join("C-ABCDEFGHJKMNPQRSTVWXYZ01");
    m.frame(now(), &ready(s));
    m.look_up_code();
    m.frame(now(), &ready(s));
    m.confirm_join();
    for _ in 0..3 {
        m.frame(now(), &ready(s));
    }
}

#[test]
fn i4_m8_a_viewers_copy_missing_pictures_is_still_linked() {
    // A follow link never sends, so pictures it could not fetch can never be removed from the circle by it.
    let (mut m, s) = open_circle("viewer", json!({}));
    let sha = "ab".repeat(32);
    let mut q = qjson(QA, "One");
    q["image"] = json!({"file": format!("media/{sha}.webp"), "width": 2, "height": 2});
    let doc = served_qs(2, vec![q], false);
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), doc);
    s.keep("Get", &format!("/circles/{CID}/banks/{BID}/blobs/{sha}"), 404, json!({"error": "not_found"}));
    let mut banks: Vec<QuestionBank> = Vec::new();
    let mut browse = BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), ImportLog::memory());
    m.add_bank(BID, false);
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
        m.sync(&mut banks, &mut browse);
    }
    assert_eq!(banks.len(), 1);
    let l = m.edit.link_for_local(&banks[0].id).expect("linked");
    assert_eq!(l.mode, LinkMode::Follow);
}

#[test]
fn i4_m5_a_get_latest_with_no_merge_token_never_marks_the_next_send_as_merged() {
    // An editor's working copy read as a viewer (a demotion): Get latest goes without merge-initiate, so no token.
    let (mut bank, mut l) = linked_full(&[(QA, "One"), (QB, "Two")]);
    l.role_seen = Some("viewer".into());
    bank.questions[1].stem = "Two (mine)".into();
    bank.modified += 5;
    let s = Script::new();
    // (phase D3) the Send page opened below reads its base version body; this test leaves it unscripted
    s.send_base_404(true);
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served_qs(2, vec![qfull(QA, "One, reworded"), qfull(QB, "Two")], false));
    s.keep("Post", "/circles/heads", 200, heads(2));
    let mut m = edit_model();
    m.add_link(l.clone()).unwrap();
    let mut banks = vec![bank];
    m.frame(now(), &ready(&s));
    m.open_merge(LOCAL, perm_of(&l));
    turn(&mut m, &s, &mut banks, 3);
    m.apply_merge();
    turn(&mut m, &s, &mut banks, 3);
    assert!(phase(&m).starts_with("Applied"), "{}", phase(&m));
    // Made an editor again, the person sends: the send is not marked as carrying a merge.
    m.open_send(LOCAL, Perm { role: Role::Editor, is_author: false, closed: false });
    let EditView::Send(v) = &m.view else { panic!("{:?}", phase(&m)) };
    assert!(!v.merged);
}

#[test]
fn i4_m4_the_viewers_form_is_keyless_even_where_a_question_does_not_say_so() {
    let (bank, mut l) = linked_from(vec![qstripped(QA, "One")], true);
    l.role_seen = Some("viewer".into());
    let mut q = qstripped(QA, "One, reworded");
    q.as_object_mut().unwrap().remove("keyless");
    q["explanation"] = json!("Leaked? No: the viewers' form is keyless whatever a question says.");
    let s = Script::new();
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served_qs(2, vec![q], true));
    let mut m = edit_model();
    m.add_link(l.clone()).unwrap();
    let mut banks = vec![bank];
    m.frame(now(), &ready(&s));
    m.open_merge(LOCAL, perm_of(&l));
    turn(&mut m, &s, &mut banks, 3);
    {
        let EditView::Merge(v) = &m.view else { panic!() };
        let MergePhase::Review(r) = &v.phase else { panic!("{}", phase(&m)) };
        let fields: Vec<&str> = r.plan.rows[0].diffs.iter().map(|d| d.field).collect();
        assert_eq!(fields, vec!["Question text"], "no key or explanation is shown as changed");
    }
    m.apply_merge();
    turn(&mut m, &s, &mut banks, 3);
    assert!(phase(&m).starts_with("Applied"), "{}", phase(&m));
    assert_eq!(banks[0].questions[0].stem, "One, reworded");
    assert!(crate::model::is_keyless(&banks[0].questions[0]));
}

// ---- critic 64: wording and the badge ---------------------------------------------------------------------------------

#[test]
fn c64_the_badge_says_no_unsent_changes_for_a_copy_the_person_can_not_send_from() {
    let (_, mut l) = linked(&[(QA, "One"), (QB, "Two")]);
    let unsent = status(1, 2, true, false);
    assert_eq!(badge_of(&l, Some(&unsent)).note.as_deref(), Some("3 unsent changes"));
    // An editor made a viewer: the strip says viewers can't send, so the badge does not count "unsent" changes.
    l.role_seen = Some("viewer".into());
    assert_eq!(badge_of(&l, Some(&unsent)).note, None);
    l.head_seen = 3;
    assert_eq!(badge_of(&l, Some(&unsent)).note.as_deref(), Some("newer version"));
}

fn circle_text(m: &mut CirclesModel, width: f32) -> String {
    text_of(width, &mut |ui| {
        super::circles_ui::body_of(ui, m);
    })
}

#[test]
fn c64_i1_the_two_verbs_are_explained_where_they_are_offered() {
    for (role, can_send) in [("editor", true), ("viewer", false)] {
        let (mut m, _s) = open_circle(role, json!({}));
        let t = circle_text(&mut m, 1280.0);
        assert!(t.contains("Add to My Banks: the bank stays connected to the circle"), "{role}: {t}");
        assert!(t.contains("Make my own copy: a separate bank that is yours alone"), "{role}: {t}");
        assert_eq!(t.contains("send your changes back"), can_send, "{role}");
        assert_eq!(t.contains("viewers can't send changes"), !can_send, "{role}");
    }
    // Nothing to explain when no row offers Add.
    let (mut m, _s) = open_circle("editor", json!({}));
    let (bank, l) = linked(&[(QA, "One")]);
    m.edit.add_link(l).unwrap();
    m.edit.status_of(&bank);
    assert!(!circle_text(&mut m, 1280.0).contains("Add to My Banks: the bank stays connected"));
}

#[test]
fn c64_i6_a_picture_failure_on_add_says_so_plainly_and_the_row_says_where_it_is() {
    let (mut m, s) = open_circle("editor", json!({}));
    let sha = "cd".repeat(32);
    let mut q = qjson(QA, "One");
    q["image"] = json!({"file": format!("media/{sha}.webp"), "width": 2, "height": 2});
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served_qs(2, vec![q], false));
    s.keep("Get", &format!("/circles/{CID}/banks/{BID}/blobs/{sha}"), 404, json!({"error": "not_found"}));
    let mut banks: Vec<QuestionBank> = Vec::new();
    let mut browse = BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), ImportLog::memory());
    m.add_bank(BID, false);
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
        m.sync(&mut banks, &mut browse);
    }
    assert_eq!(banks.len(), 1);
    assert!(m.edit.link_for_local(&banks[0].id).is_none(), "an editor's copy missing pictures is not connected");
    let n = format!("{:?}", m.notice);
    assert!(n.contains("1 of 1 pictures couldn't be fetched") && n.contains("is not linked to the circle") && !n.contains("working copy") && !n.contains("plain copy"), "{n}");
    let t = circle_text(&mut m, 1280.0);
    assert!(t.contains("Added as \"Cells\", not connected: 1 of 1 pictures couldn't be fetched.") && t.contains("Open it"), "{t}");
}

#[test]
fn c64_i7_the_merge_and_send_pages_use_the_new_words() {
    let (bank, l) = linked(&[(QA, "One")]);
    let s = Script::new();
    s.on("Post", &format!("/circles/{CID}/banks/{BID}/merge"), 400, json!({"error": "invalid_request", "field": "base_version"}));
    let mut cm = model();
    cm.edit.add_link(l).unwrap();
    cm.edit.update_link(LOCAL, |l| {
        l.bank_modified = bank.modified + 100;
        l.bank_print = "x".into();
        true
    });
    let mut banks = vec![bank];
    cm.edit.sync(&mut banks);
    cm.edit.frame(now(), &ready(&s));
    cm.edit.open_merge(LOCAL, Perm { role: Role::Editor, is_author: false, closed: false });
    turn(&mut cm.edit, &s, &mut banks, 2);
    let t = text_of(1280.0, &mut |ui| {
        super::circle_edit_ui::draw_page(ui, &mut cm);
    });
    assert!(t.contains("Get latest: \"Cells\"") && t.contains("Use my bank as it is...") && t.contains("Unlink..."), "{t}");
    for old in ["in step", "Forget", "working copy", "Get latest and merge"] {
        assert!(!t.contains(old), "{old}: {t}");
    }
}

#[test]
fn c64_i8_bank_settings_shows_no_question_promise_and_opens_or_adds_the_bank() {
    let (mut m, s) = open_circle("owner", json!({}));
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(1, &[(QA, "One")], false));
    s.keep("Get", &format!("/circles/{CID}/banks/{BID}/versions"), 200, json!({"bank_id": BID, "head_version": 1, "oldest_kept": 1, "pinned_versions": [], "versions": [], "next_before": null}));
    m.open_bank(BID);
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
    }
    let t = circle_text(&mut m, 1280.0);
    assert!(t.contains("Bank settings") && !t.contains("The questions are shown here"), "{t}");
    assert!(t.contains("Add to My Banks"), "{t}");
    let (bank, l) = linked(&[(QA, "One")]);
    m.edit.add_link(l).unwrap();
    m.edit.status_of(&bank);
    let t = circle_text(&mut m, 1280.0);
    assert!(t.contains("Open in My Banks") && !t.contains("Add to My Banks"), "{t}");
}

#[test]
fn c64_polish_rows_and_lineage() {
    // An own copy is named as one, says where it is with a way to open it; a linked add's note has no version number.
    let (mut m, _s, banks, _b) = add_as("editor", false, true);
    assert_eq!(banks[0].name, "Cells (my copy)");
    let t = circle_text(&mut m, 1280.0);
    assert!(t.contains("Your own copy \"Cells (my copy)\" is in My Banks.") && t.contains("Open it"), "{t}");
    let (_m, _s, banks, _b) = add_as("editor", false, false);
    let d = &banks[0].details.description;
    assert!(d.starts_with("From the circle \"Biology team\": \"Cells\" by Mr Keesler") && !d.contains("version"), "{d}");
    // No row says "Not shared publicly from here yet".
    let (mut m, _s) = open_circle("editor", json!({}));
    assert!(!circle_text(&mut m, 1280.0).contains("Not shared publicly"));
    // An editor made a viewer: the row does not say "Nothing to send." beside a strip that says viewers can't send.
    let (bank, mut l) = linked(&[(QA, "One")]);
    l.role_seen = Some("viewer".into());
    let st = {
        let mut e = edit_model();
        e.add_link(l.clone()).unwrap();
        e.status_of(&bank).unwrap()
    };
    assert!(super::circle_edit_ui::standing(&l, Some(&st), false, Role::Viewer).is_empty());
}

#[test]
fn c64_polish_7_after_joining_each_added_bank_can_be_opened() {
    let s = Script::new();
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(2, &[(QA, "One")], false));
    let mut m = model();
    join_with(&s, &mut m, "editor");
    let mut banks: Vec<QuestionBank> = Vec::new();
    let mut browse = BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), ImportLog::memory());
    m.add_ticked();
    for _ in 0..4 {
        m.frame(now(), &ready(&s));
        m.sync(&mut banks, &mut browse);
    }
    assert_eq!(banks.len(), 1);
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    let mut got = super::circles_ui::Outcome::None;
    assert!(super::circles_phasec_tests::click_text(&ctx, 1280.0, "Open", 0, &mut |ui| {
        let o = super::circles_ui::body_of(ui, &mut m);
        if o != super::circles_ui::Outcome::None {
            got = o;
        }
    }));
    assert_eq!(got, super::circles_ui::Outcome::OpenBank(banks[0].id.clone()));
}

#[test]
fn c64_i11_sharing_a_bank_into_the_circle_it_is_linked_to_is_not_offered() {
    let s = Script::new();
    let b = json!({"id": LOCAL, "name": "Unit 2 quiz", "questions": [qjson("q0", "Zero")], "created": 1, "modified": 2, "details": {}});
    let src = super::publish::ShareSource::from_bank_json(&format!("bank:{LOCAL}"), "Bank: Unit 2 quiz", "Unit 2 quiz", &b.to_string());
    // This computer has the bank linked to the circle (the edit model read its links: the Banks list or its page).
    let (_, l) = linked(&[(QA, "One")]);
    let mut e = edit_model();
    e.add_link(l).unwrap();
    let mut p = super::publish::PublishModel::new(Runner::Inline, Arc::new(HashMap::<String, Vec<u8>>::new()), super::publish::LogIo::memory()).with_retry(RetryPolicy::none());
    p.start(src);
    p.frame(now(), &ready(&s));
    s.keep("Get", "/circles", 200, json!({"circles": [summary("editor", json!({"name": "Algebra team"}))]}));
    p.circle.activate(None);
    p.frame(now(), &ready(&s));
    p.frame(now(), &ready(&s));
    let t = text_of(1280.0, &mut |ui| {
        super::publish_ui::which_circle(ui, &mut p);
    });
    assert!(t.contains("This bank is already in it: use Send my changes on the bank's page."), "{t}");
    p.circle.choose(CID);
    assert_ne!(p.circle.chosen.as_deref(), Some(CID), "the circle it is linked to can not be chosen");
}

#[test]
fn c64_polish_13_history_closes_when_the_bank_page_was_left_but_a_send_stays() {
    let s = Script::new();
    // (phase D3) the Send page opened below reads its base version body; this test leaves it unscripted
    s.send_base_404(true);
    s.keep("Post", "/circles/heads", 200, heads(1));
    s.keep("Get", "/circles", 200, circles_list("editor"));
    s.keep("Get", &format!("/circles/{CID}/banks/{BID}/versions"), 200, json!({"bank_id": BID, "head_version": 1, "oldest_kept": 1, "pinned_versions": [], "versions": [], "next_before": null}));
    let mut m = model();
    let (bank, l) = linked(&[(QA, "One")]);
    m.edit.add_link(l).unwrap();
    let mut banks = vec![bank];
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    let snap = as_who(&s, "olga");
    let t0 = now();
    let mut frame = |ctx: &egui::Context, m: &mut CirclesModel, draw: bool| {
        let mut browse = BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), ImportLog::memory());
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 4000.0))), ..Default::default() };
        let out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                if draw {
                    super::bank_links_ui::strip_with(ui, m, &mut banks, &mut browse, &mut acct, &snap, t0, NOW, LOCAL);
                }
            });
        });
        out.drop_without_applying_deltas();
    };
    frame(&ctx, &mut m, true);
    m.edit.open_history(CID, BID, "Cells", Perm { role: Role::Editor, is_author: false, closed: false });
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back".into() });
    frame(&ctx, &mut m, true);
    frame(&ctx, &mut m, true);
    assert!(matches!(m.edit.view, EditView::History(_)), "open while the page is on screen");
    // Another screen for a frame, then back: the page shows the questions again.
    frame(&ctx, &mut m, false);
    frame(&ctx, &mut m, true);
    assert!(matches!(m.edit.view, EditView::Closed), "{:?}", m.edit.view);
    // A send page stays (the person may be in the middle of it).
    m.edit.open_send(LOCAL, Perm { role: Role::Editor, is_author: false, closed: false });
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back".into() });
    frame(&ctx, &mut m, true);
    frame(&ctx, &mut m, false);
    frame(&ctx, &mut m, true);
    assert!(matches!(m.edit.view, EditView::Send(_)));
}

// ---- owner decision (b): "Add the ticked banks" finishes after the Shared page is left ----------------------------------

#[test]
fn b_owner_the_ticked_banks_finish_in_the_background_from_the_app_tick() {
    let s = Script::new();
    let two = json!([
        {"id": hex(201), "title": "Unit 1", "head_version": 1, "question_count": 1, "created_at": NOW, "updated_at": NOW},
        {"id": hex(202), "title": "Unit 2", "head_version": 1, "question_count": 1, "created_at": NOW, "updated_at": NOW}
    ]);
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("editor", json!({"banks": two})));
    for id in [hex(201), hex(202)] {
        let mut doc: Value = serde_json::from_slice(&served(1, &[(QA, "One")], false)).unwrap();
        doc["bank"]["id"] = json!(id);
        s.keep_raw("Get", &format!("/circles/{CID}/banks/{id}"), doc.to_string().into_bytes());
    }
    let mut m = model();
    join_with_detail(&s, &mut m, "editor");
    m.add_ticked();
    // (the list of circles is to be read again on the Shared page's next frame: never in the background)
    m.list = Default::default();
    let before = s.paths().len();
    // The person leaves the Shared page at once: only the app's tick (and the browse window's frame, which has the
    // banks) run from here.
    let mut banks: Vec<QuestionBank> = Vec::new();
    let mut browse = BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), ImportLog::memory());
    let snap = ready(&s);
    let mut saves = 0;
    for _ in 0..12 {
        m.tick_background(now(), &snap);
        if super::circles::land_waiting(&mut banks, &mut browse) {
            saves += 1;
        }
    }
    assert_eq!(banks.len(), 2, "both banks are in My Banks");
    assert!(saves >= 2, "the banks are saved");
    for b in &banks {
        assert!(m.edit.link_for_local(&b.id).is_some_and(|l| l.can_send()), "{} is linked", b.name);
    }
    assert!(m.adds.iter().all(|a| matches!(a.state, super::circles::AddState::Done { linked: true, .. })), "{:?}", m.adds);
    // Nothing else is read in the background: only the two banks (no list, no circle page, no activity).
    let paths = s.paths()[before..].to_vec();
    assert!(paths.iter().all(|p| p.starts_with(&format!("Get /circles/{CID}/banks/"))), "{paths:?}");
    assert_eq!(paths.len(), 2, "{paths:?}");
}

#[test]
fn a_owner_no_preview_of_a_removed_or_hidden_bank() {
    let (mut m, _s) = open_circle("editor", json!({"banks": [
        {"id": hex(301), "title": "Gone", "head_version": 1, "question_count": 1, "created_at": NOW, "updated_at": NOW, "removed": {"at": NOW, "reason": "owner_removed", "undo_until": NOW + DAY}},
        {"id": hex(302), "title": "", "hidden": true, "head_version": 1, "question_count": 1, "created_at": NOW, "updated_at": NOW}
    ]}));
    for bid in [hex(301), hex(302)] {
        m.open_preview(&bid);
        assert_eq!(m.screen, super::circles::Screen::Circle, "{bid}");
        assert!(m.bank.is_none());
    }
}

#[test]
fn b_owner_nothing_runs_in_the_background_when_nothing_was_asked() {
    let s = Script::new();
    let mut m = model();
    let mut banks: Vec<QuestionBank> = Vec::new();
    let mut browse = BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), ImportLog::memory());
    for _ in 0..5 {
        assert!(!m.tick_background(now(), &ready(&s)));
        assert!(!super::circles::land_waiting(&mut banks, &mut browse));
    }
    assert!(s.seen().is_empty());
}

// ---- owner decision (a): a read-only Preview before adding --------------------------------------------------------------

#[test]
fn a_owner_preview_shows_the_questions_read_only_from_the_circle_page() {
    for (role, hidden) in [("editor", false), ("viewer", true)] {
        let extra = if hidden { json!({"banks": [{"id": BID, "title": "Cells", "subject": "Biology", "license": "CC-BY-4.0", "author_name": "Mr Keesler", "is_author": false, "head_version": 2, "question_count": 2, "created_at": NOW - DAY, "updated_at": NOW - DAY, "hide_key": true}]}) } else { json!({}) };
        let (mut m, s) = open_circle(role, extra);
        s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(2, &[(QA, "Which organelle makes ATP?"), (QB, "Name the cell's control centre.")], hidden));
        let ctx = egui::Context::default();
        crate::theme::setup(&ctx);
        assert!(super::circles_phasec_tests::click_text(&ctx, 1280.0, "Preview", 0, &mut |ui| {
            super::circles_ui::body_of(ui, &mut m);
        }), "{role}: Preview is in the row");
        let reads = || s.paths().iter().filter(|p| *p == &format!("Get /circles/{CID}/banks/{BID}")).count();
        for _ in 0..3 {
            m.frame(now(), &ready(&s));
        }
        assert_eq!(reads(), 1, "{role}: one read of the bank");
        let t = circle_text(&mut m, 1280.0);
        assert!(t.contains("Preview") && t.contains("Which organelle makes ATP?") && t.contains("Name the cell's control centre."), "{role}: {t}");
        assert!(t.contains("Read only"), "{t}");
        for not in ["Bank settings", "Answer keys for viewers", "Send my changes", "Get latest", "History of versions", "Remove", "Licence"] {
            assert!(!t.contains(not), "{role}: {not} in {t}");
        }
        assert_eq!(t.contains("without its answer keys") || t.contains("without their correct answers"), hidden, "{role}: {t}");
        // Back to the circle: the circle's page is not read again.
        let n = s.paths().len();
        assert!(super::circles_phasec_tests::click_text(&ctx, 1280.0, "< Biology team", 0, &mut |ui| {
            super::circles_ui::body_of(ui, &mut m);
        }));
        for _ in 0..3 {
            m.frame(now(), &ready(&s));
        }
        assert_eq!(m.screen, super::circles::Screen::Circle);
        assert!(!s.paths()[n..].iter().any(|p| p == &format!("Get /circles/{CID}")), "{:?}", &s.paths()[n..]);
    }
}

#[test]
fn b_owner_the_banks_being_added_can_be_drawn_on_the_banks_list() {
    let s = Script::new();
    let mut m = model();
    join_with(&s, &mut m, "editor");
    m.add_ticked();
    let t = text_of(1280.0, &mut |ui| super::bank_links_ui::adding_rows_of(ui, &mut m));
    assert!(t.contains("Adding \"Cells\" from Algebra team..."), "{t}");
    // Nothing when nothing is being added.
    let mut m2 = model();
    assert!(!text_of(420.0, &mut |ui| super::bank_links_ui::adding_rows_of(ui, &mut m2)).contains("Adding"));
}
