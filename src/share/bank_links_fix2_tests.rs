//! Phase D1, critics 65 and 66: the fixes, each test written first against the code it pins. Data safety first (a
//! links file that could not be read never lets a new link take the place of a copy with unsent work, joining again
//! checks the copy again, whether the person is the bank's author belongs to one account, "Add a full copy" read
//! without keys moves no link, the surviving mutants of critic 65 pinned), then the wording and the screens.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use super::bank_links::*;
use super::bank_links_fix_tests::*;
use super::bank_links_tests::*;
use super::browse::{BrowseModel, ImportLog, Problem};
use super::circle_edit::*;
use super::circle_links::*;
use super::circles::{AddState, CirclesModel};
use super::circles_api::*;
use super::circles_tests::*;
use super::merge::{BaseQuestion, RowKind, base_of};
use super::model::Runner;
use super::pool::*;
use crate::model::QuestionBank;

fn tag_of(who: &str) -> Option<String> {
    account_tag(Some(&format!("https://pool.test\n{who}@school.org")))
}

fn unreadable_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("vershion-c67-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    // a directory where the file should be: "there, but it can not be read"
    std::fs::create_dir_all(dir.join(LINKS_DOC)).unwrap();
    dir
}

fn dir_model(dir: &std::path::Path) -> CircleEditModel {
    let media: super::publish::SharedMedia = Arc::new(HashMap::<String, Vec<u8>>::new());
    let mut m = CircleEditModel::new(Runner::Inline, Arc::new(Mem::default()), media, Arc::new(NoDiskRestore), LinkLog::Dir(dir.to_path_buf())).with_retry(RetryPolicy::none());
    m.set_clock(NOW);
    m
}

// ---- c65 B1: a links file that could not be read ----------------------------------------------------------------------

#[test]
fn b1_while_the_links_file_cant_be_read_no_link_is_added_and_try_again_keeps_the_copy_with_unsent_work() {
    let dir = unreadable_dir("b1");
    let mut m = dir_model(&dir);
    assert!(m.unreadable().is_some());
    let (mut a, l1) = linked(&[(QA, "One")]);
    a.questions[0].stem = "One (my unsent fix)".into();
    a.modified += 5;
    // Add (or join, or a full copy) while the file can not be read: refused, and nothing is kept in memory.
    let mut l2 = l1.clone();
    l2.local_bank_id = "local-bank-2".into();
    let e = m.add_link(l2.clone()).unwrap_err();
    assert!(e.contains("couldn't be read"), "{e}");
    assert!(m.link_for_local("local-bank-2").is_none(), "not kept as if it were saved");
    assert!(m.add_link_replacing(l2.clone()).is_err());
    assert!(m.link_for_local("local-bank-2").is_none());
    // A bank shared into the circle meanwhile: not linked, said.
    let mut l3 = l1.clone();
    l3.local_bank_id = "local-bank-3".into();
    l3.circle_bank_id = hex(78);
    queue_link(l3);
    assert!(m.links().is_empty());
    assert!(matches!(outcome_of(&hex(78)), Some(LinkOutcome::NotSaved(m)) if m.contains("so no bank was linked")));
    // The file can be read again (it was a moment's lock), with the first copy's link in it.
    std::fs::remove_dir_all(dir.join(LINKS_DOC)).unwrap();
    std::fs::write(dir.join(LINKS_DOC), json!({"links": [l1]}).to_string()).unwrap();
    // (minor) not read again on every call while it could not be read: only when asked
    assert!(m.unreadable().is_some() && m.status_of(&a).is_none());
    m.read_links_again();
    assert!(m.unreadable().is_none());
    assert_eq!(m.link_for_local(LOCAL).map(|l| l.circle_bank_id), Some(BID.to_string()), "the copy with unsent work keeps its link");
    assert!(m.link_for_local("local-bank-2").is_none() && m.link_for_local("local-bank-3").is_none());
    assert_eq!(m.status_of(&a).unwrap().changes.total(), 1);
    m.update_link(LOCAL, |l| {
        l.head_seen += 1;
        true
    });
    let file = std::fs::read_to_string(dir.join(LINKS_DOC)).unwrap();
    assert!(file.contains(LOCAL) && !file.contains("local-bank-2"), "{file}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn b1_a_link_that_could_not_be_saved_is_not_kept_as_if_it_were() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("vershion-c67-b1w-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut m = dir_model(&dir);
    assert!(m.links().is_empty() && m.unreadable().is_none(), "no file yet: an empty list");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    let (_, l) = linked(&[(QA, "One")]);
    let r = m.add_link(l);
    let wrote = dir.join(LINKS_DOC).exists();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    if !wrote {
        assert!(r.is_err(), "{r:?}");
        assert!(m.link_for_local(LOCAL).is_none(), "the link the page said could not be saved is not used");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn b1_the_circle_page_offers_no_add_while_the_links_file_cant_be_read() {
    let (mut m, s) = open_circle("editor", json!({}));
    let dir = unreadable_dir("b1page");
    m.edit = dir_model(&dir);
    let n = s.seen().len();
    m.add_bank(BID, false);
    assert!(m.adds.is_empty(), "no linked add: this computer may already have the bank");
    m.add_full_copy(BID);
    assert!(m.adds.is_empty());
    for width in [1280.0, 420.0] {
        let t = text_of(width, &mut |ui| {
            super::circles_ui::body_of(ui, &mut m);
        });
        assert!(!t.to_lowercase().contains("add to my banks") && t.contains("Make my own copy"), "{t}");
        assert!(t.contains("couldn't be read"), "says why: {t}");
    }
    // An own copy is not linked: still made.
    m.add_bank(BID, true);
    assert_eq!(m.adds.len(), 1);
    assert_eq!(s.seen().len(), n);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn b1_the_join_offer_adds_nothing_while_the_links_file_cant_be_read() {
    let s = Script::new();
    let mut m = model();
    let dir = unreadable_dir("b1join");
    m.edit = dir_model(&dir);
    join_with(&s, &mut m, "editor");
    let o = m.join_offer.as_ref().expect("offer");
    assert!(o.problem.as_ref().is_some_and(|p| p.message.contains("couldn't be read")), "{o:?}");
    m.tick_offer(BID, true);
    m.add_ticked();
    assert!(m.adds.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- c65 I1: joining again checks the copy again ------------------------------------------------------------------------

#[test]
fn i1_after_joining_again_the_copy_is_checked_again_and_can_send() {
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, json!({"banks": [{"bank_id": BID, "status": "unavailable"}]}));
    s.on("Get", "/circles", 200, json!({"circles": []}));
    let mut cm = model();
    let (bank, mut l) = linked(&[(QA, "One")]);
    l.role_seen = Some("editor".into());
    l.role_seen_by = tag_of("olga");
    cm.edit.add_link(l).unwrap();
    let a = as_who(&s, "olga");
    cm.note_account(&a);
    let t0 = now();
    cm.links.strip(&mut cm.edit, &bank, &a, t0, NOW);
    cm.links.frame(&mut cm.edit, NOW);
    assert!(cm.edit.link_for_local(LOCAL).unwrap().gone(), "she left: her own check says so");
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
    let before = heads_count(&s);
    // A minute later she opens the copy: it is checked again at once.
    let v = cm.links.strip(&mut cm.edit, &bank, &a, t0 + MIN, NOW).unwrap();
    assert!(v.busy, "{v:?}");
    cm.links.frame(&mut cm.edit, NOW);
    assert_eq!(heads_count(&s), before + 1);
    let v = cm.links.strip(&mut cm.edit, &bank, &a, t0 + MIN, NOW).unwrap();
    assert!(!v.line.contains("no longer in"), "{v:?}");
    assert!(!cm.edit.link_for_local(LOCAL).unwrap().gone());
}

// ---- c65 I2: whether the person is the bank's author belongs to one account ----------------------------------------------

fn author_link() -> (QuestionBank, CircleLink) {
    let (bank, mut l) = linked(&[(QA, "One")]);
    l.is_author = true;
    l.author_known = true;
    l.role_seen = Some("editor".into());
    l.role_seen_by = tag_of("olga");
    (bank, l)
}

#[test]
fn i2_another_member_account_does_not_inherit_the_authors_restore_and_trim() {
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, heads(1));
    s.keep("Get", "/circles", 200, circles_list("editor"));
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("editor", json!({})));
    let mut cm = model();
    let (bank, l) = author_link();
    cm.edit.add_link(l).unwrap();
    let b = as_who(&s, "pete");
    cm.note_account(&b);
    cm.links.strip(&mut cm.edit, &bank, &b, now(), NOW);
    cm.links.frame(&mut cm.edit, NOW);
    let after = cm.edit.link_for_local(LOCAL).unwrap();
    assert_eq!(after.role_seen_by, tag_of("pete"));
    assert!(!after.is_author && !perm_of(&after).can_rewind(), "pete is not offered Restore and Trim");
    // Whether pete is the author was read in the same check.
    assert!(s.paths().iter().any(|p| p == &format!("Get /circles/{CID}")), "{:?}", s.paths());
    assert!(after.author_known);
}

#[test]
fn i2_when_the_circle_page_could_not_be_read_the_author_is_learnt_again_next_time() {
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, heads(1));
    s.keep("Get", "/circles", 200, circles_list("editor"));
    s.on("Get", &format!("/circles/{CID}"), 503, json!({"error": "unavailable"}));
    let mut d = detail("editor", json!({}));
    d["banks"][0]["is_author"] = json!(true);
    s.keep("Get", &format!("/circles/{CID}"), 200, d);
    let mut cm = model();
    let (bank, l) = author_link();
    cm.edit.add_link(l).unwrap();
    let b = as_who(&s, "pete");
    cm.note_account(&b);
    let t0 = now();
    cm.links.strip(&mut cm.edit, &bank, &b, t0, NOW);
    cm.links.frame(&mut cm.edit, NOW);
    let after = cm.edit.link_for_local(LOCAL).unwrap();
    assert!(!after.is_author && !after.author_known, "not known for pete yet: {after:?}");
    // The next check reads the circle page and learns that pete is the author too.
    cm.links.check_now(LOCAL);
    cm.links.strip(&mut cm.edit, &bank, &b, t0 + MIN, NOW);
    cm.links.frame(&mut cm.edit, NOW);
    let after = cm.edit.link_for_local(LOCAL).unwrap();
    assert!(after.is_author && after.author_known, "{:?}", s.paths());
}

#[test]
fn i2_another_accounts_visit_to_the_circle_page_does_not_change_the_authors_link() {
    let s = Script::new();
    s.keep("Get", "/circles", 200, json!({"circles": [summary("editor", json!({}))]}));
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("editor", json!({})));
    s.keep("Get", &format!("/circles/{CID}/activity"), 200, json!({"activity": [], "next_before": null}));
    let mut m = model();
    let (_, l) = author_link();
    m.edit.add_link(l).unwrap();
    let b = as_who(&s, "pete");
    m.note_account(&b);
    m.open_list();
    m.frame(now(), &b);
    m.open_circle(CID);
    for _ in 0..4 {
        m.frame(now(), &b);
    }
    assert!(m.circle.as_ref().is_some_and(|c| c.detail.is_some()));
    let after = m.edit.link_for_local(LOCAL).unwrap();
    assert!(after.is_author && after.author_known, "olga's link keeps her Restore and Trim");
    // olga's own visit teaches her link.
    let a = as_who(&s, "olga");
    let mut m2 = model();
    let (_, l) = author_link();
    m2.edit.add_link(l).unwrap();
    m2.note_account(&a);
    m2.open_list();
    m2.frame(now(), &a);
    m2.open_circle(CID);
    for _ in 0..4 {
        m2.frame(now(), &a);
    }
    assert!(!m2.edit.link_for_local(LOCAL).unwrap().is_author, "the circle says she is not the author");
}

// ---- c65 I3: "Add a full copy" read without keys moves no link -----------------------------------------------------------

#[test]
fn i3_a_full_copy_read_in_the_viewers_form_moves_no_link_and_says_why() {
    let (mut cm, s) = open_circle("editor", json!({}));
    let (bank, mut l) = linked(&[(QA, "One"), (QB, "Two")]);
    l.mode = LinkMode::Follow;
    l.role_seen = Some("editor".into());
    cm.edit.add_link(l).unwrap();
    cm.edit.status_of(&bank);
    // Between the press and the read the person was made a viewer of a bank that hides its keys.
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served_qs(2, vec![qstripped(QA, "One"), qstripped(QB, "Two")], true));
    let mut banks = vec![bank];
    let mut browse = BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), ImportLog::memory());
    cm.add_full_copy(BID);
    for _ in 0..3 {
        cm.frame(now(), &ready(&s));
        cm.sync(&mut banks, &mut browse);
    }
    assert_eq!(banks.len(), 1, "no keyless bank named as a full copy");
    assert!(cm.edit.link_for_local(LOCAL).is_some(), "the copy keeps its link");
    let Some(AddState::Failed(p)) = cm.add_state(CID, BID).map(|j| j.state.clone()) else { panic!("{:?}", cm.adds) };
    assert!(p.message.contains("answer keys") && p.message.contains("Nothing was changed"), "{}", p.message);
}

// ---- c65 minors ------------------------------------------------------------------------------------------------------------

#[test]
fn minor_the_badge_says_newer_version_only_where_the_strip_offers_get_latest() {
    let (bank, mut l) = linked(&[(QA, "One")]);
    l.role_seen = Some("viewer".into());
    l.seen.hide_key = true;
    l.head_seen = 3;
    let mut m = edit_model();
    m.add_link(l.clone()).unwrap();
    let st = m.status_of(&bank).unwrap();
    assert!(!strip_view(&input(&l, &st)).buttons.contains(&StripButton::GetLatest));
    assert_ne!(badge_of(&l, Some(&st)).note.as_deref(), Some("newer version"));
    // An ordinary copy that is behind still says it.
    l.seen.hide_key = false;
    assert_eq!(badge_of(&l, Some(&st)).note.as_deref(), Some("newer version"));
}

#[test]
fn minor_another_accounts_check_never_marks_a_shared_or_older_link_gone() {
    let sb = Script::new();
    sb.keep("Post", "/circles/heads", 200, json!({"banks": [{"bank_id": BID, "status": "unavailable"}]}));
    sb.keep("Get", "/circles", 200, json!({"circles": []}));
    let bank: QuestionBank = serde_json::from_value(json!({"id": LOCAL, "name": "Unit 1", "questions": [qjson("q0", "Zero")], "created": 1, "modified": 1_700_000_000, "details": {}})).unwrap();
    let sent = SentBank { circle_id: CID.into(), bank_id: BID.into(), version: 1, version_id: hex(1), qids: vec![QA.into()], ..SentBank::default() };
    let l = link_from_share(CID, "Algebra team", Role::Owner, &bank, &sent, &ItemMeta { title: "Unit 1".into(), ..ItemMeta::default() }, License::CcBy, NOW).unwrap();
    assert!(l.role_seen_by.is_none());
    let mut cm = model();
    cm.edit.add_link(l).unwrap();
    let b = as_who(&sb, "pete");
    cm.note_account(&b);
    cm.links.strip(&mut cm.edit, &bank, &b, now(), NOW);
    cm.links.frame(&mut cm.edit, NOW);
    let after = cm.edit.link_for_local(LOCAL).unwrap();
    assert!(!after.gone() && after.role_seen.as_deref() == Some("owner"), "{:?}", after.role_seen);
    let v = cm.links.strip(&mut cm.edit, &bank, &b, now(), NOW).unwrap();
    assert!(!v.line.contains("no longer in") && v.line.contains("not in this circle"), "{}", v.line);
    assert!(!v.buttons.contains(&StripButton::Send) && !v.buttons.contains(&StripButton::GetLatest));
    // Without an account to tell them apart (the model alone), a check that does not list the circle still says so.
    let (_, mut old) = linked(&[(QA, "One")]);
    apply_check(&mut old, None, Some(&[]), NOW);
    assert!(old.gone());
}

#[test]
fn minor_a_session_that_ended_during_a_send_shows_sign_in_on_the_strip() {
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, heads(1));
    s.keep("Get", "/circles", 200, circles_list("editor"));
    let mut m = model();
    let (bank, l) = linked(&[(QA, "One")]);
    m.edit.add_link(l).unwrap();
    let mut banks = vec![bank];
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    let t0 = now();
    page_frame(&mut m, &mut banks, &mut acct, &ready(&s), t0);
    page_frame(&mut m, &mut banks, &mut acct, &ready(&s), t0);
    // Get latest, from the strip, meets an ended session.
    s.fail(super::api::ApiError::SignedOut);
    m.edit.open_merge(LOCAL, Perm { role: Role::Editor, is_author: false, closed: false });
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    let (out, _) = page_frame(&mut m, &mut banks, &mut acct, &ready(&s), t0 + Duration::from_secs(1));
    assert!(out.page);
    // Back to the bank: the strip says to sign in, with its button.
    m.edit.close();
    let (_, t) = page_frame(&mut m, &mut banks, &mut acct, &ready(&s), t0 + Duration::from_secs(2));
    assert!(t.contains("Sign in..."), "{t}");
}

// ---- c65 I4: the surviving mutants, pinned ------------------------------------------------------------------------------

#[test]
fn m07_a_link_whose_bank_is_not_counted_yet_is_never_taken() {
    let (_, l) = linked(&[(QA, "One")]);
    let mut m = edit_model();
    m.add_link(l.clone()).unwrap();
    // (no badge or page has counted LOCAL yet: it is taken as here, with its changes)
    let mut other = l;
    other.local_bank_id = "local-bank-2".into();
    assert!(m.add_link(other).is_err());
    assert!(m.link_for_local(LOCAL).is_some() && m.link_for_local("local-bank-2").is_none());
}

#[test]
fn m08_a_bank_added_while_another_took_the_link_lands_unlinked_and_the_link_stays() {
    let (mut cm, s) = open_circle("editor", json!({}));
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(2, &[(QA, "One"), (QB, "Two")], false));
    cm.add_bank(BID, false);
    assert_eq!(cm.adds.len(), 1);
    // Meanwhile another bank of this computer was linked to it (a share from the publish window, say).
    let (mut other, mut l) = linked(&[(QA, "One")]);
    other.id = "local-bank-9".into();
    l.local_bank_id = "local-bank-9".into();
    l.mark_bank(&other);
    cm.edit.add_link(l).unwrap();
    let mut banks: Vec<QuestionBank> = vec![other];
    let mut browse = BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), ImportLog::memory());
    for _ in 0..3 {
        cm.frame(now(), &ready(&s));
        cm.sync(&mut banks, &mut browse);
    }
    assert_eq!(banks.len(), 2, "the bank was added");
    assert!(cm.edit.link_for_local(&banks[1].id).is_none(), "but not linked");
    assert!(cm.edit.link_for_local("local-bank-9").is_some(), "the other bank keeps the link");
    assert!(matches!(&cm.add_state(CID, BID).unwrap().state, AddState::Done { linked: false, .. }));
    let n = format!("{:?}", cm.notice);
    assert!(n.contains("already connected"), "{n}");
}

#[test]
fn m09_add_a_full_copy_is_refused_for_a_copy_that_can_send() {
    let (mut cm, s) = open_circle("editor", json!({}));
    let (mut bank, l) = linked(&[(QA, "One")]);
    bank.questions[0].stem = "One (my unsent fix)".into();
    bank.modified += 5;
    cm.edit.add_link(l).unwrap();
    cm.edit.status_of(&bank);
    let n = s.seen().len();
    cm.add_full_copy(BID);
    for _ in 0..3 {
        cm.frame(now(), &ready(&s));
    }
    assert!(cm.adds.is_empty() && s.seen().len() == n, "{:?}", cm.adds);
    assert_eq!(cm.edit.link_for_local(LOCAL).map(|l| l.circle_bank_id), Some(BID.to_string()));
}

#[test]
fn m10_m11_a_second_member_account_is_not_another_account_and_learns_the_author_again() {
    // (m10) pete is in the circle too: his check is his, not "another account's".
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, heads(2));
    s.keep("Get", "/circles", 200, circles_list("viewer"));
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("viewer", json!({})));
    let mut cm = model();
    let (bank, l) = author_link();
    cm.edit.add_link(l).unwrap();
    let b = as_who(&s, "pete");
    cm.note_account(&b);
    cm.links.strip(&mut cm.edit, &bank, &b, now(), NOW);
    cm.links.frame(&mut cm.edit, NOW);
    assert_eq!(cm.links.check_of(LOCAL), Check::Done);
    let after = cm.edit.link_for_local(LOCAL).unwrap();
    assert_eq!((after.role_seen.as_deref(), after.role_seen_by.clone(), after.head_seen), (Some("viewer"), tag_of("pete"), 2));
    let v = cm.links.strip(&mut cm.edit, &bank, &b, now(), NOW).unwrap();
    assert!(v.line.contains("(you can view)") && !v.line.contains("another account"), "{v:?}");
}

#[test]
fn m52_sign_in_on_the_strip_checks_again_once_signed_in() {
    let s = Script::new();
    s.fail(super::api::ApiError::SignedOut);
    s.keep("Post", "/circles/heads", 200, heads(1));
    s.keep("Get", "/circles", 200, circles_list("editor"));
    let mut m = model();
    let (bank, l) = linked(&[(QA, "One")]);
    m.edit.add_link(l).unwrap();
    let mut banks = vec![bank];
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    let t0 = now();
    page_frame(&mut m, &mut banks, &mut acct, &ready(&s), t0);
    let (_, t) = page_frame(&mut m, &mut banks, &mut acct, &ready(&s), t0 + Duration::from_secs(1));
    assert!(t.contains("Sign in..."), "{t}");
    assert_eq!(heads_count(&s), 1);
    // A minute later the account is signed in again and the person presses "Sign in...".
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    let mut browse = BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), ImportLog::memory());
    assert!(super::circles_phasec_tests::click_text(&ctx, 1280.0, "Sign in...", 0, &mut |ui| {
        super::bank_links_ui::strip_with(ui, &mut m, &mut banks, &mut browse, &mut acct, &ready(&s), t0 + MIN, NOW, LOCAL);
    }));
    assert!(acct.reveals >= 1);
    page_frame(&mut m, &mut banks, &mut acct, &ready(&s), t0 + MIN);
    page_frame(&mut m, &mut banks, &mut acct, &ready(&s), t0 + MIN);
    assert_eq!(heads_count(&s), 2, "{:?}", s.paths());
    let (_, t) = page_frame(&mut m, &mut banks, &mut acct, &ready(&s), t0 + MIN);
    assert!(!t.contains("Sign in...") && t.contains("Your copy is up to date"), "{t}");
}

// ---- the viewers' form: a fresh shuffle is not a change, a real change is (m01-m05) -------------------------------------

/// The viewers' form of one question, `kind` as the pool serves it.
fn qkind(id: &str, stem: &str, kind: Value) -> Value {
    json!({"id": id, "stem": stem, "kind": kind, "points": 1.0, "difficulty": null, "explanation": null, "keyless": true, "created": 5, "modified": 6})
}

fn mc(choices: &[(&str, bool)]) -> Value {
    json!({"Mc": {"answers": choices.iter().map(|(t, locked)| json!({"text": t, "correct": false, "locked": locked})).collect::<Vec<_>>()}})
}

/// For each question of `next`: whether it took the form this copy had (`true`: only shuffled).
fn settled(first: Vec<Value>, next: Vec<Value>) -> Vec<bool> {
    let a = circle_theirs_as(&body_of(first), NOW, true);
    let base: Vec<BaseQuestion> = a.iter().map(|t| base_of(&t.q, &t.qid, &t.fp, t.sha.clone())).collect();
    let mut b = circle_theirs_as(&body_of(next), NOW, true);
    settle_shuffle(&mut b, &base);
    b.iter().zip(&base).map(|(t, bq)| serde_json::to_value(&t.q.kind).unwrap() == serde_json::to_value(&bq.snap.kind).unwrap()).collect()
}

#[test]
fn m03_m04_m05_a_shuffle_is_settled_and_a_real_change_is_not() {
    // Matching: both columns shuffled is not a change; a left item reworded is (m03).
    let m1 = json!({"Matching": {"pairs": [{"left": "Cell wall", "right": ""}, {"left": "Nucleus", "right": ""}], "distractors": ["Holds DNA", "Keeps shape", "Makes food"]}});
    let shuffled = json!({"Matching": {"pairs": [{"left": "Nucleus", "right": ""}, {"left": "Cell wall", "right": ""}], "distractors": ["Makes food", "Holds DNA", "Keeps shape"]}});
    let reworded = json!({"Matching": {"pairs": [{"left": "Cell membrane", "right": ""}, {"left": "Nucleus", "right": ""}], "distractors": ["Holds DNA", "Keeps shape", "Makes food"]}});
    assert_eq!(settled(vec![qkind(QA, "Match", m1.clone()), qkind(QB, "Match 2", m1.clone())], vec![qkind(QA, "Match", shuffled), qkind(QB, "Match 2", reworded)]), vec![true, false]);
    // Mc with a locked "I don't know" last: the free choices shuffled is not a change; the locked one moved is (m04).
    let first = mc(&[("Mitochondria", false), ("Ribosome", false), ("Nucleus", false), ("I don't know", true)]);
    let free_moved = mc(&[("Nucleus", false), ("Mitochondria", false), ("Ribosome", false), ("I don't know", true)]);
    let locked_moved = mc(&[("I don't know", true), ("Mitochondria", false), ("Ribosome", false), ("Nucleus", false)]);
    assert_eq!(settled(vec![qkind(QA, "Which?", first.clone()), qkind(QB, "Which 2?", first)], vec![qkind(QA, "Which?", free_moved), qkind(QB, "Which 2?", locked_moved)]), vec![true, false]);
    // Multipart with an Mc part: the part's choices shuffled is not a change (m05); a part's choice reworded is.
    let part = |choices: &[&str]| json!({"Multipart": {"parts": [{"stem": "Part one", "kind": {"Mc": {"answers": choices.iter().map(|t| json!({"text": t, "correct": false})).collect::<Vec<_>>()}}}, {"stem": "Part two", "kind": {"Short": {"sample": ""}}}]}});
    let p1 = part(&["Two", "Three", "Four"]);
    assert_eq!(settled(vec![qkind(QA, "Parts", p1.clone()), qkind(QB, "Parts 2", p1)], vec![qkind(QA, "Parts", part(&["Four", "Two", "Three"])), qkind(QB, "Parts 2", part(&["Four", "Two", "Five"]))]), vec![true, false]);
}

#[test]
fn m01_get_latest_on_a_viewers_form_copy_keeps_the_order_of_choices_it_had() {
    let first: Value = shuffled_read(1, ["One", "Two", "Three"], 0);
    let (bank, mut l) = linked_from(first["payload"]["bank"]["questions"].as_array().unwrap().clone(), true);
    l.role_seen = Some("viewer".into());
    let before: Vec<Vec<String>> = bank.questions.iter().map(|q| match &q.kind {
        crate::model::Kind::Mc { answers } => answers.iter().map(|a| a.text.clone()).collect(),
        _ => Vec::new(),
    }).collect();
    let s = Script::new();
    s.on_raw("Get", &format!("/circles/{CID}/banks/{BID}"), shuffled_read(2, ["One", "Two, reworded", "Three"], 1).to_string().into_bytes());
    let mut m = edit_model();
    m.add_link(l.clone()).unwrap();
    let mut banks = vec![bank];
    m.frame(now(), &ready(&s));
    m.open_merge(LOCAL, perm_of(&l));
    turn(&mut m, &s, &mut banks, 3);
    m.apply_merge();
    turn(&mut m, &s, &mut banks, 3);
    assert!(phase(&m).starts_with("Applied"), "{}", phase(&m));
    assert_eq!(banks[0].questions[1].stem, "Two, reworded", "the change was taken");
    let after: Vec<Vec<String>> = banks[0].questions.iter().map(|q| match &q.kind {
        crate::model::Kind::Mc { answers } => answers.iter().map(|a| a.text.clone()).collect(),
        _ => Vec::new(),
    }).collect();
    assert_eq!(after, before, "no question's choices moved, the taken one's neither");
}

#[test]
fn m02_an_authors_real_reorder_of_choices_reaches_a_full_copy() {
    let (mut bank, l) = linked_full(&[(QA, "One"), (QB, "Two")]);
    bank.modified += 1;
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
    let EditView::Merge(v) = &m.view else { panic!() };
    let MergePhase::Review(r) = &v.phase else { panic!("{}", phase(&m)) };
    let kinds: Vec<RowKind> = r.plan.rows.iter().map(|x| x.kind).collect();
    assert_eq!(kinds, vec![RowKind::TheirsChanged, RowKind::Unchanged], "the author put B first on purpose");
}

// ---- critic 66: the words and the screens ---------------------------------------------------------------------------------

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

#[test]
fn c66_i1_the_card_button_shares_one_of_my_banks_and_the_rows_add_to_my_banks() {
    for width in [1280.0, 420.0] {
        let (mut m, _s) = open_circle("editor", json!({}));
        let t = text_of(width, &mut |ui| {
            super::circles_ui::body_of(ui, &mut m);
        });
        assert!(t.contains("Share one of my banks here...") && !t.contains("Add a bank..."), "{t}");
        assert!(t.contains("Add to My Banks") && !t.contains("Add to my banks"), "{t}");
    }
}

#[test]
fn c66_i2_a_bank_added_to_my_banks_is_not_recorded_as_an_import() {
    let (mut m, _s, banks, mut browse) = add_as("editor", false, false);
    assert!(m.edit.link_for_local(&banks[0].id).is_some());
    assert!(browse.imported().is_empty(), "its strip says where it stands: not Things I imported");
    assert!(browse.update_for_bank(&banks[0].id).is_none());
}

fn import_rec(item: &str, bank: &str, circle: bool) -> super::browse::ImportedRecord {
    serde_json::from_value(json!({"item_id": item, "bank_id": bank, "bank_name": "Unit 2 quiz", "version": 1, "license": "CC-BY-4.0", "title": "Cells", "author": "Mr Keesler", "imported_at": NOW,
        "newer": {"version": 3, "license": null, "license_version": null},
        "circle": if circle { json!({"circle_id": CID, "circle_name": "Algebra team"}) } else { Value::Null }}))
    .unwrap()
}

#[test]
fn c66_i2_update_from_pool_is_never_offered_for_a_circle_bank_and_a_linked_one_says_so() {
    let recs = vec![import_rec(BID, LOCAL, true), import_rec(&hex(5), "b-plain", true), import_rec(&hex(6), "b-pool", false)];
    let mut b = BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), ImportLog::Memory(Arc::new(std::sync::Mutex::new(recs.clone()))));
    // LOCAL is linked to the circle on this computer (a copy made before phase D, recorded then).
    let mut e = edit_model();
    e.add_link(linked(&[(QA, "One")]).1).unwrap();
    assert!(b.update_for_bank(LOCAL).is_none(), "linked: its strip offers Get latest");
    assert!(b.update_for_bank("b-plain").is_none(), "a circle bank is not a public pool item");
    assert!(b.update_for_bank("b-pool").is_some(), "a pool import still is");
    assert_eq!(b.update_count(), 2, "the linked copy is not counted on the Imported tab");
    b.start_update(&recs[1]);
    assert!(b.item.is_none(), "a circle bank is never opened as a pool item");
    b.screen = super::browse::Screen::Browse;
    let t = text_of(1280.0, &mut |ui| {
        super::browse_ui::tab_body(ui, &mut b, super::browse::Tab::Imported);
    });
    assert!(t.contains("Linked to the circle \"Algebra team\": open it in My Banks to send or get the latest."), "{t}");
    assert_eq!(t.matches("Nothing you change here goes back to the circle").count(), 1, "only the copy that is not linked: {t}");
}

#[test]
fn c66_i3_preview_offers_add_and_copy_and_shows_the_key_of_a_bank_read_in_full() {
    let (mut m, s) = open_circle("editor", json!({}));
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(2, &[(QA, "One"), (QB, "Two")], false));
    m.open_preview(BID);
    for _ in 0..2 {
        m.frame(now(), &ready(&s));
    }
    for width in [1280.0, 420.0] {
        let t = text_of(width, &mut |ui| {
            super::circles_ui::body_of(ui, &mut m);
        });
        assert!(t.contains("Add to My Banks") && t.contains("Make my own copy"), "{t}");
        assert!(t.contains("A. A ✓") && t.contains("B. B") && !t.contains("1. A"), "letters, not numbers that read as decimals; the key: {t}");
        assert!(t.contains("Read only: nothing here changes the bank.") && !t.contains("The answers are not shown"), "{t}");
    }
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    assert!(super::circles_phasec_tests::click_text(&ctx, 1280.0, "Add to My Banks", 0, &mut |ui| {
        super::circles_ui::body_of(ui, &mut m);
    }));
    assert_eq!(m.adds.len(), 1);
    assert!(!m.adds[0].own);
    // A viewer of a bank that hides its keys: no key, said so.
    let (mut v, s2) = open_circle("viewer", json!({}));
    s2.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served_qs(2, vec![qstripped(QA, "One")], true));
    v.open_preview(BID);
    for _ in 0..2 {
        v.frame(now(), &ready(&s2));
    }
    let t = text_of(1280.0, &mut |ui| {
        super::circles_ui::body_of(ui, &mut v);
    });
    assert!(t.contains("A. A") && !t.contains('✓') && t.contains("The answers are not shown"), "{t}");
}

#[test]
fn c66_i4_the_banks_list_says_when_an_add_failed_or_was_not_connected_until_dismissed() {
    use super::circles::AddJob;
    let mut m = model();
    let p = Problem { message: "Could not reach the pool.".into(), retry: true, retry_at: None, account: false };
    m.adds.push(AddJob { cid: CID.into(), bid: BID.into(), own: false, title: "Unit 3 test".into(), role: Role::Editor, state: AddState::Failed(p), full: false, dismissed: false, account: None });
    m.adds.push(AddJob { cid: CID.into(), bid: hex(9), own: false, title: "Linear equations starter".into(), role: Role::Editor, state: AddState::Done { bank_id: "b9".into(), bank_name: "Linear equations starter".into(), linked: false, why: Some("2 of 3 pictures couldn't be fetched".into()) }, full: false, dismissed: false, account: None });
    m.adds.push(AddJob { cid: CID.into(), bid: hex(10), own: false, title: "Fine".into(), role: Role::Editor, state: AddState::Done { bank_id: "b10".into(), bank_name: "Fine".into(), linked: true, why: None }, full: false, dismissed: false, account: None });
    let draw = |m: &mut CirclesModel| {
        text_of(1280.0, &mut |ui| {
            super::bank_links_ui::adding_rows_of(ui, m);
        })
    };
    let t = draw(&mut m);
    assert!(t.contains("\"Unit 3 test\" couldn't be added: Could not reach verSHion's server. Try again from the circle's page."), "{t}");
    assert!(t.contains("\"Linear equations starter\" was added, but it is not connected to the circle: 2 of 3 pictures couldn't be fetched."), "{t}");
    assert!(!t.contains("Fine"), "a bank added and linked needs no line: {t}");
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    assert!(super::circles_phasec_tests::click_text(&ctx, 1280.0, "Dismiss", 0, &mut |ui| {
        super::bank_links_ui::adding_rows_of(ui, &mut m);
    }));
    let t = draw(&mut m);
    assert!(!t.contains("Unit 3 test") && t.contains("Linear equations starter"), "{t}");
}

#[test]
fn c66_i5_a_bank_added_but_not_connected_says_what_to_do_and_another_add_gets_its_own_name() {
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
    let t = text_of(1280.0, &mut |ui| {
        super::circles_ui::body_of(ui, &mut m);
    });
    assert!(t.contains("Added as \"Cells\", not connected: 1 of 1 pictures couldn't be fetched. Delete it in My Banks, then add it again."), "{t}");
    m.add_bank(BID, false);
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
        m.sync(&mut banks, &mut browse);
    }
    assert_eq!(banks.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), vec!["Cells", "Cells (2)"]);
}

#[test]
fn c66_i6_left_removed_and_closed_are_said_calmly_with_unlink_and_no_menu_item_that_fails() {
    use StripButton::*;
    let (_, l) = linked(&[(QA, "One"), (QB, "Two")]);
    let st = status(0, 1, true, false);
    let mut gone = l.clone();
    gone.role_seen = Some(ROLE_NONE.into());
    let mut closed = l.clone();
    closed.seen.circle_closed = true;
    let mut removed = l.clone();
    removed.seen.removed = true;
    for (x, circle_page) in [(&gone, false), (&closed, true), (&removed, true)] {
        let v = strip_view(&input(x, &st));
        assert!(!v.warn, "the bank works as a normal bank: {v:?}");
        assert_eq!(v.buttons, vec![Unlink]);
        assert!(v.line.ends_with("You can keep using it as your own bank."), "{}", v.line);
        assert!(!v.menu_history, "its history can not be read now: {v:?}");
        assert_eq!(v.menu_circle_page, circle_page, "{v:?}");
    }
    let v = strip_view(&input(&l, &st));
    assert!(v.menu_history && v.menu_circle_page);
}

#[test]
fn c66_i7_the_send_page_speaks_of_the_circle_and_open_account_opens_it() {
    assert!(!super::circles_text::PICTURES_SEND_INTRO.contains("pool"));
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
    assert!(t.contains("did not say which question") && !t.to_lowercase().contains("pool"), "{t}");
    let p = Problem { message: "Your account is waiting for approval.".into(), retry: false, retry_at: None, account: true };
    m.edit.view = EditView::Send(Box::new(send_view(SendStage::Failed(p))));
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    let mut browse = BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), ImportLog::memory());
    assert!(super::circles_phasec_tests::click_text(&ctx, 1280.0, "Open account...", 0, &mut |ui| {
        super::bank_links_ui::strip_with(ui, &mut m, &mut banks, &mut browse, &mut acct, &ready(&s), now(), NOW, LOCAL);
    }));
    page_frame(&mut m, &mut banks, &mut acct, &ready(&s), now());
    assert!(acct.reveals >= 1, "the Account window opens");
    assert!(!format!("{:?}", m.edit.results_of(LOCAL)).contains("top of this page"));
}

#[test]
fn c66_minors_the_strip_lines() {
    use StripButton::*;
    let (_, l) = linked(&[(QA, "One"), (QB, "Two")]);
    let none = status(0, 0, true, false);
    let unsent = status(0, 1, true, false);
    // Held back for review: no big Send when there is nothing to send (it would replace the circle's version).
    let mut held = l.clone();
    held.seen.under_review = true;
    let v = strip_view(&input(&held, &none));
    assert!(v.line.starts_with("Shared with Algebra team. ") && !v.buttons.contains(&Send), "{v:?}");
    assert_eq!(strip_view(&input(&held, &unsent)).buttons.first(), Some(&Send));
    // The viewers' form says which circle; promoted, why a full copy is offered.
    let mut vf = l.clone();
    vf.mode = LinkMode::Follow;
    vf.viewers_form = true;
    vf.role_seen = Some("viewer".into());
    assert!(strip_view(&input(&vf, &none)).line.starts_with("Shared with Algebra team"));
    vf.role_seen = Some("editor".into());
    let v = strip_view(&input(&vf, &none));
    assert!(v.line.contains("You can edit now: add a full copy to get the answer keys and send changes."), "{}", v.line);
    // Changes not counted, the copy not behind: no Get latest that has nothing to get.
    let v = strip_view(&input(&l, &status(0, 0, false, false)));
    assert!(!v.buttons.contains(&GetLatest), "{v:?}");
    let mut behind = l.clone();
    behind.head_seen = 3;
    assert!(strip_view(&input(&behind, &status(0, 0, false, false))).buttons.contains(&GetLatest));
    // A copy that keeps its keys.
    let mut kept = l.clone();
    kept.role_seen = Some("viewer".into());
    kept.seen.hide_key = true;
    assert!(strip_view(&input(&kept, &none)).line.contains("Get latest is turned off here so that you keep them."));
}

#[test]
fn c66_minors_the_circle_page_rows_and_the_join_offer() {
    // The row reads the role the page has just loaded: a viewer now has nothing to send.
    let (mut m, _s) = open_circle("viewer", json!({}));
    let (bank, mut l) = linked(&[(QA, "One")]);
    l.role_seen = Some("editor".into());
    m.edit.add_link(l).unwrap();
    m.edit.status_of(&bank);
    let t = text_of(1280.0, &mut |ui| {
        super::circles_ui::body_of(ui, &mut m);
    });
    assert!(t.contains("In My Banks as") && !t.contains("Nothing to send."), "{t}");
    // The author's original bank, not linked: Add makes a second bank, said.
    let mut d = json!({});
    d["banks"] = json!([{"id": BID, "title": "Cells", "subject": "Biology", "license": "CC-BY-4.0", "author_name": "Mr Keesler", "is_author": true, "head_version": 1, "question_count": 2, "created_at": NOW - DAY, "updated_at": NOW - DAY}]);
    let (mut a, _s) = open_circle("editor", d);
    let t = text_of(1280.0, &mut |ui| {
        super::circles_ui::body_of(ui, &mut a);
    });
    assert!(t.contains("this adds a second bank; you can delete your original afterwards"), "{t}");
    // The join offer: a bank left unticked is said, not dropped.
    let s = Script::new();
    let mut two = json!({});
    two["banks"] = json!([
        {"id": BID, "title": "Cells", "subject": "Biology", "license": "CC-BY-4.0", "author_name": "Mr Keesler", "is_author": false, "head_version": 1, "question_count": 2, "created_at": NOW - DAY, "updated_at": NOW - DAY},
        {"id": hex(9), "title": "Plants", "subject": "Biology", "license": "CC-BY-4.0", "author_name": "Mr Keesler", "is_author": false, "head_version": 1, "question_count": 2, "created_at": NOW - DAY, "updated_at": NOW - DAY}
    ]);
    s.keep("Get", "/circles", 200, json!({"circles": []}));
    s.on("Post", "/circles/join/preview", 200, json!({"circle": {"id": CID, "name": "Algebra team", "owner_name": "Ms Frizzle", "members": 3}, "role": "editor", "already_member": false}));
    s.on("Post", "/circles/join", 200, json!({"circle_id": CID, "role": "editor", "already": false}));
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("editor", two));
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(1, &[(QA, "One")], false));
    let mut j = model();
    j.open_join("C-ABCDEFGHJKMNPQRSTVWXYZ01");
    j.frame(now(), &ready(&s));
    j.look_up_code();
    j.frame(now(), &ready(&s));
    j.confirm_join();
    for _ in 0..3 {
        j.frame(now(), &ready(&s));
    }
    j.tick_offer(&hex(9), false);
    j.add_ticked();
    let t = text_of(1280.0, &mut |ui| {
        super::circles_ui::body_of(ui, &mut j);
    });
    assert!(t.contains("Plants") && t.contains("not added"), "{t}");
}
