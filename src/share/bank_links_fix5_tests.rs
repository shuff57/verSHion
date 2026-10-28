//! Phase D1, round 5 (critics 71 and 72): each test written first against the code it pins. One list of results per
//! bank (`act_results`) replaces the single notice: a restore or trim the person did not see on its History is said
//! on its bank's strip and Bank settings, whatever screen was left behind; another bank's result never takes its
//! place; a result goes when dismissed or when the next outcome of its kind comes; nothing one account was told is
//! said to the next. Also: the strip's "up to date" after a restore conflict, a job kept to the account it was
//! started for, the adds on the Banks list, and the merge review's own words.

use std::sync::{Arc, Mutex};

use serde_json::json;

use super::bank_links_fix4_tests::*;
use super::bank_links_fix_tests::*;
use super::bank_links_tests::*;
use super::circle_edit::*;
use super::circle_links::*;
use super::circles::CirclesModel;
use super::circles_api::*;
use super::circles_tests::*;
use super::pool::*;
use crate::model::QuestionBank;

fn rollback_path(bid: &str) -> String {
    format!("/circles/{CID}/banks/{bid}/rollback")
}

fn trim_path(bid: &str) -> String {
    format!("/circles/{CID}/banks/{bid}/trim")
}

/// A linked bank's page, as the account in `snap` sees it.
fn strip_as(ctx: &egui::Context, m: &mut CirclesModel, banks: &mut [QuestionBank], acct: &mut FakeAccount, snap: &super::publish::AccountSnapshot, local: &str) -> String {
    let mut b = browse();
    frame_text(ctx, &mut |ui| {
        super::bank_links_ui::strip_with(ui, m, banks, &mut b, acct, snap, now(), NOW, local);
    })
}

/// A's History opened from Bank settings on the Shared page (no bank page hosts it), its versions read while the
/// person is on A's page in My Banks (the History is not drawn).
fn history_on_the_shared_page(ctx: &egui::Context, m: &mut CirclesModel, banks: &mut [QuestionBank], acct: &mut FakeAccount, s: &Arc<Script>) {
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    assert!(m.edit.open_history(CID, BID, "Unit 2 quiz", owner()));
    m.edit.host = None;
    strip(ctx, m, banks, acct, s, LOCAL);
    strip(ctx, m, banks, acct, s, LOCAL);
    assert!(loaded(m), "{:?}", m.edit.view);
}

/// Bank settings' History section of A on the Shared page.
fn settings_text(m: &mut CirclesModel, bid: &str, title: &str) -> String {
    text_of(1280.0, &mut |ui| super::circle_edit_ui::history_section(ui, m, CID, bid, title))
}

// ---- c71 I1a: a History left open on the Shared page, not drawn ----------------------------------------------------

#[test]
fn i1a_a_restore_from_bank_settings_that_failed_is_said_on_the_banks_strip_and_bank_settings() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    history_on_the_shared_page(&ctx, &mut m, &mut banks, &mut acct, &s);
    m.edit.ask_restore(1);
    s.on("Post", &rollback_path(BID), 503, json!({"error": "unavailable"}));
    m.edit.confirm_restore(false);
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("The restore of version 1 did not go through") && t.contains("Open History to try again"), "{t}");
    assert!(t.contains("Dismiss"), "{t}");
    assert!(settings_text(&mut m, BID, "Unit 2 quiz").contains("The restore of version 1 did not go through"));
    // Back on the Shared page: the History does not show the restore as still under way, and says how it went once.
    let page = text_of(1280.0, &mut |ui| super::circle_edit_ui::draw_page(ui, &mut m));
    assert_eq!(page.matches("did not go through").count(), 1, "{page}");
    assert!(!page.contains("Restoring"), "{page}");
}

#[test]
fn i1a_a_restore_from_bank_settings_refused_as_too_large_is_said_on_the_banks_strip() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    history_on_the_shared_page(&ctx, &mut m, &mut banks, &mut acct, &s);
    m.edit.ask_restore(1);
    s.on("Post", &rollback_path(BID), 428, json!({"error": "confirm_large_required", "removed": 2, "total": 3}));
    m.edit.confirm_restore(false);
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("Version 1 was not restored: it would remove 2 of the bank's 3 questions"), "{t}");
}

#[test]
fn i1a_the_strip_says_restoring_while_the_bank_settings_restore_is_on_its_way_and_bank_settings_does_too() {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    assert!(m.edit.open_history(CID, BID, "Unit 2 quiz", owner()));
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &loaded);
    m.edit.ask_restore(1);
    s.on("Post", &rollback_path(BID), 200, rolled(4, 1));
    s.on("Get", &versions(), 200, page(4, &[4, 3, 2, 1]));
    let release = s.hold();
    m.edit.confirm_restore(false);
    let t = settings_text(&mut m, BID, "Unit 2 quiz");
    assert!(t.contains("Restoring version 1 in the circle"), "{t}");
    release.send(()).unwrap();
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| !m.edit.acting());
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("Version 4 was restored from version 1") && t.contains("Version 4 is the latest"), "{t}");
}

// ---- c71 I1b: another bank's result never takes this one's place -----------------------------------------------------

#[test]
fn i1b_two_restores_in_a_row_each_say_how_they_went_on_their_own_strip() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    // A: restore, Back (the answer: refused)
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    m.edit.ask_restore(1);
    s.on("Post", &rollback_path(BID), 503, json!({"error": "unavailable"}));
    m.edit.confirm_restore(false);
    m.edit.back();
    // B: its History, a restore, Back (the answer: done)
    let b = hex(77);
    s.on("Get", &format!("/circles/{CID}/banks/{b}/versions"), 200, json!({"bank_id": b, "head_version": 2, "oldest_kept": 1, "pinned_versions": [], "versions": [{"version": 2, "version_id": hex(2), "by": "Mr Keesler", "at": NOW, "message": "", "added": 0, "changed": 1, "removed": 0, "question_count": 1, "license": "CC-BY-4.0", "kept": true, "withheld": false}, {"version": 1, "version_id": hex(1), "by": "Mr Keesler", "at": NOW - DAY, "message": "", "added": 1, "changed": 0, "removed": 0, "question_count": 1, "license": "CC-BY-4.0", "kept": true, "withheld": false}], "next_before": null}));
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B);
    assert!(m.edit.open_history(CID, &b, "Plants", owner()));
    m.edit.host = Some(EditHost { local_id: LOCAL_B.into(), back: "< Back to Plants".into() });
    m.edit.host_frame = None;
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B);
    m.edit.ask_restore(1);
    s.on("Post", &rollback_path(&b), 200, json!({"circle_id": CID, "bank_id": b, "version": 3, "version_id": hex(3), "qids": [QB], "added": 0, "changed": 1, "removed": 0, "rollback_of": 1}));
    m.edit.confirm_restore(false);
    m.edit.back();
    let tb = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B);
    assert!(tb.contains("Version 3 was restored from version 1") && !tb.contains("did not go through"), "{tb}");
    let ta = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(ta.contains("The restore of version 1 did not go through") && !ta.contains("Version 3 was restored"), "{ta}");
}

#[test]
fn i1b_a_banks_second_result_after_its_first_was_seen_is_said_too() {
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
    s.on("Post", &trim_path(BID), 200, json!({"ok": true, "pruned": 1, "oldest_kept": 2, "pinned_versions": []}));
    s.on("Get", &versions(), 200, page(3, &[3, 2]));
    m.edit.confirm_trim();
    m.edit.back();
    assert!(strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL).contains("The trim deleted the content of 1 older version."));
    // seen; then B's page, then A gets a second result
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B);
    make_stale(&mut m, LOCAL);
    treat_in_step(&mut m, &s, &mut banks, LOCAL, BID, false);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B);
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("now used as it is"), "the second result is said: {t}");
    assert!(t.contains("The trim deleted"), "the first stays until dismissed: {t}");
}

// ---- c71 I1c: a later outcome replaces an earlier one --------------------------------------------------------------

#[test]
fn i1c_a_retry_that_went_through_replaces_the_earlier_failure() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    m.edit.ask_restore(1);
    s.on("Post", &rollback_path(BID), 503, json!({"error": "unavailable"}));
    m.edit.confirm_restore(false);
    m.edit.back();
    assert!(strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL).contains("did not go through"));
    // "Open History to try again": it goes through this time, while the History is shown.
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    m.edit.ask_restore(1);
    s.on("Post", &rollback_path(BID), 200, rolled(4, 1));
    s.on("Get", &versions(), 200, page(4, &[4, 3, 2, 1]));
    m.edit.confirm_restore(false);
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("Done. Version 4 is now the circle's newest") && !t.contains("did not go through"), "the History says it once, without the old failure: {t}");
    m.edit.close_restore_result();
    m.edit.back();
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(!t.contains("did not go through") && t.contains("Version 4 is the latest"), "{t}");
}

#[test]
fn i1c_dismiss_takes_a_result_off_the_strip() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    make_stale(&mut m, LOCAL);
    treat_in_step(&mut m, &s, &mut banks, LOCAL, BID, false);
    assert!(strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL).contains("now used as it is"));
    let mut b = browse();
    let snap = ready(&s);
    assert!(super::circles_phasec_tests::click_text(&ctx, 1280.0, "Dismiss", 0, &mut |ui| {
        super::bank_links_ui::strip_with(ui, &mut m, &mut banks, &mut b, &mut acct, &snap, now(), NOW, LOCAL);
    }));
    assert!(!strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL).contains("now used as it is"));
}

// ---- c71 I2: a result with no bank on this computer, and the save failures ---------------------------------------------

#[test]
fn i2_a_restore_of_a_circle_bank_with_no_copy_here_is_said_named_on_the_banks_list_and_its_bank_settings_only() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    let z = hex(99);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    s.on("Get", &format!("/circles/{CID}/banks/{z}/versions"), 200, json!({"bank_id": z, "head_version": 3, "oldest_kept": 1, "pinned_versions": [], "versions": [{"version": 3, "version_id": hex(3), "by": "Mr Keesler", "at": NOW, "message": "", "added": 0, "changed": 1, "removed": 0, "question_count": 1, "license": "CC-BY-4.0", "kept": true, "withheld": false}, {"version": 1, "version_id": hex(1), "by": "Mr Keesler", "at": NOW - DAY, "message": "", "added": 1, "changed": 0, "removed": 0, "question_count": 1, "license": "CC-BY-4.0", "kept": true, "withheld": false}], "next_before": null}));
    assert!(m.edit.open_history(CID, &z, "Unit 3 test", owner()));
    m.edit.frame(now(), &ready(&s));
    m.edit.ask_restore(1);
    s.on("Post", &rollback_path(&z), 200, json!({"circle_id": CID, "bank_id": z, "version": 4, "version_id": hex(4), "qids": [QA], "added": 0, "changed": 1, "removed": 0, "rollback_of": 1}));
    m.edit.confirm_restore(false);
    m.edit.back();
    m.edit.frame(now(), &ready(&s));
    // Not above another bank's screens or strips.
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    m.edit.frame(now(), &ready(&s));
    let t = text_of(1280.0, &mut |ui| super::circle_edit_ui::draw_page(ui, &mut m));
    assert!(t.contains("History: \"Unit 2 quiz\"") && !t.contains("Version 4 was restored"), "{t}");
    m.edit.back();
    assert!(!strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL).contains("Version 4 was restored"));
    assert!(!strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B).contains("Version 4 was restored"));
    // The top of the Banks list says it, named; and Z's Bank settings.
    let t = rows(&mut m);
    assert!(t.contains("\"Unit 3 test\"") && t.contains("Version 4 was restored from version 1") && t.contains("Dismiss"), "{t}");
    assert!(settings_text(&mut m, &z, "Unit 3 test").contains("Version 4 was restored from version 1"));
    assert!(!settings_text(&mut m, BID, "Unit 2 quiz").contains("Version 4 was restored"));
    let ctx2 = egui::Context::default();
    crate::theme::setup(&ctx2);
    assert!(super::circles_phasec_tests::click_text(&ctx2, 1280.0, "Dismiss", 0, &mut |ui| super::bank_links_ui::adding_rows_of(ui, &mut m)));
    assert!(!rows(&mut m).contains("Version 4 was restored"));
}

#[cfg(unix)]
#[test]
fn i2_an_unlink_that_could_not_be_saved_keeps_the_link_and_says_so_on_its_strip() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("vershion-c71-unlink-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, heads(1));
    s.keep("Get", "/circles", 200, circles_list("owner"));
    let mut m = model();
    let media: super::publish::SharedMedia = Arc::new(std::collections::HashMap::<String, Vec<u8>>::new());
    m.edit = CircleEditModel::new(super::model::Runner::Inline, Arc::new(Mem::default()), media, Arc::new(NoDiskRestore), LinkLog::Dir(dir.clone())).with_retry(RetryPolicy::none());
    m.edit.set_clock(NOW);
    let (a, l) = linked(&[(QA, "One")]);
    m.edit.add_link(l).unwrap();
    let mut banks = vec![a];
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    m.edit.ask_forget(LOCAL);
    m.edit.confirm_forget(LOCAL);
    let wrote = std::fs::read_dir(&dir).map(|d| d.count()).unwrap_or(0) > 1 || !std::fs::read_to_string(dir.join(LINKS_DOC)).unwrap_or_default().contains(LOCAL);
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    if !wrote {
        assert!(m.edit.link_for_local(LOCAL).is_some(), "the link the file still has is kept");
        let ctx = ctx();
        let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
        let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
        assert!(t.contains("could not be unlinked") && t.contains("could not be saved"), "{t}");
        assert!(!rows(&mut m).contains("could not be"), "not at the top of the Banks list: it has its strip");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn i2_a_check_that_could_not_be_saved_says_so_on_the_strip_of_each_bank_it_changed() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("vershion-c71-check-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let media: super::publish::SharedMedia = Arc::new(std::collections::HashMap::<String, Vec<u8>>::new());
    let mut e = CircleEditModel::new(super::model::Runner::Inline, Arc::new(Mem::default()), media, Arc::new(NoDiskRestore), LinkLog::Dir(dir.clone())).with_retry(RetryPolicy::none());
    let (_, l) = linked(&[(QA, "One")]);
    e.add_link(l).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    e.apply_heads(&[HeadEntry { bank_id: BID.into(), state: HeadState::Current { version: 5, license: None, at: Some(NOW), by: None, hide_key: None } }]);
    let saved = std::fs::read_to_string(dir.join(LINKS_DOC)).unwrap_or_default().contains("\"head_seen\":5");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    if !saved {
        let r = e.results_of(LOCAL);
        assert!(r.iter().any(|(_, t)| t.contains("The result of the check could not be saved")), "{r:?}");
        assert!(e.results_bankless().is_empty());
    }
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- c71 I3: a restore conflict ----------------------------------------------------------------------------------------

#[test]
fn i3_after_a_restore_conflict_left_behind_the_strip_is_not_up_to_date_and_says_nothing_was_reloaded() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    m.edit.ask_restore(1);
    s.on("Post", &rollback_path(BID), 409, json!({"error": "conflict", "head_version": 5, "head_at": NOW, "head_by": "Mr Keesler"}));
    m.edit.confirm_restore(false);
    m.edit.back();
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(!t.contains("up to date"), "{t}");
    assert!(t.contains("Version 5 is the latest. You have version 3."), "{t}");
    assert!(!t.contains("reloaded"), "{t}");
    assert!(t.contains("someone sent version 5 first") && t.contains("Open History to choose again"), "{t}");
}

#[test]
fn i3_after_a_restore_conflict_on_the_history_shown_it_is_reloaded_and_the_link_knows_the_newer_version() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    m.edit.ask_restore(1);
    s.on("Post", &rollback_path(BID), 409, json!({"error": "conflict", "head_version": 5}));
    s.on("Get", &versions(), 200, page(5, &[5, 4, 3, 2, 1]));
    m.edit.confirm_restore(false);
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("The history has been reloaded"), "{t}");
    assert_eq!(m.edit.link_for_local(LOCAL).unwrap().head_seen, 5);
}

// ---- c72 I2: a result does not outlive its truth ---------------------------------------------------------------------

#[test]
fn c72_i2_a_restore_result_never_says_it_is_the_newest_next_to_a_newer_version() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    m.edit.ask_restore(1);
    s.on("Post", &rollback_path(BID), 200, rolled(4, 1));
    m.edit.confirm_restore(false);
    m.edit.back();
    assert!(strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL).contains("Version 4 was restored from version 1"));
    // someone sends version 5; "Check now"
    m.edit.update_link(LOCAL, |l| {
        l.head_seen = 5;
        true
    });
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("Version 5 is the latest") && !t.contains("it is now the circle's newest"), "{t}");
}

#[test]
fn c72_i2_a_send_whose_answer_could_not_be_read_is_no_longer_said_once_get_latest_caught_up() {
    let (mut m, s, mut banks) = two_at_three();
    s.send_base_404(true);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    banks[0].questions[0].stem = "One (my fix)".into();
    banks[0].modified += 5;
    assert!(m.edit.open_send(LOCAL, editor()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    for _ in 0..3 {
        strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    }
    m.edit.set_attest(true);
    // the answer has no version the app can read
    s.on("Put", &format!("/circles/{CID}/banks/{BID}"), 201, json!({"circle_id": CID, "bank_id": BID, "version": 0, "qids": [], "added": 0, "changed": 0, "removed": 0}));
    m.edit.send();
    m.edit.sync(&mut banks);
    m.edit.close();
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("Press Get latest to catch up"), "{t}");
    // Get latest: nothing newer
    s.on("Post", &format!("/circles/{CID}/banks/{BID}/merge"), 400, json!({"error": "invalid_request", "field": "base_version"}));
    assert!(m.edit.open_merge(LOCAL, editor()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    for _ in 0..3 {
        strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    }
    m.edit.cancel_merge();
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(!t.contains("Press Get latest to catch up"), "{t}");
}

// ---- c71 minors: accounts ------------------------------------------------------------------------------------------

#[test]
fn m19_after_another_account_signed_in_its_own_restore_is_said_on_the_strip() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    let (olga, pete) = (as_who(&s, "olga"), as_who(&s, "pete"));
    // (a link checked by another account: whether this one is the bank's author is read from the circle's page)
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("owner", json!({})));
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL);
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &pete, LOCAL);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &pete, LOCAL);
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &pete, LOCAL);
    m.edit.ask_restore(1);
    s.on("Post", &rollback_path(BID), 200, rolled(4, 1));
    m.edit.confirm_restore(false);
    m.edit.back();
    let t = strip_as(&ctx, &mut m, &mut banks, &mut acct, &pete, LOCAL);
    assert!(t.contains("Version 4 was restored from version 1"), "{t}");
}

#[test]
fn m01_another_banks_strip_never_says_restoring() {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &loaded);
    m.edit.ask_restore(1);
    s.on("Post", &rollback_path(BID), 200, rolled(4, 1));
    s.on("Get", &versions(), 200, page(4, &[4, 3, 2, 1]));
    // (B's own check first: the request held is the restore's)
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B, &|m| m.links.check_of(LOCAL_B) != super::bank_links::Check::Checking);
    let release = s.hold();
    m.edit.confirm_restore(false);
    m.edit.back();
    let tb = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL_B);
    assert!(!tb.contains("Restoring"), "{tb}");
    let ta = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(ta.contains("Restoring version 1 in the circle"), "{ta}");
    release.send(()).unwrap();
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| !m.edit.acting());
}

/// A restore or trim of `olga`'s held at the server; `pete` signs in; then it is answered with `reply`.
fn held_then_another_account(path: &str, reply: Option<(u16, serde_json::Value)>, trim: bool) -> (CirclesModel, Arc<Script>, Vec<QuestionBank>, egui::Context, FakeAccount, String, String) {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    let (olga, pete) = (as_who(&s, "olga"), as_who(&s, "pete"));
    // (a link checked by another account: whether this one is the bank's author is read from the circle's page)
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("owner", json!({})));
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    for _ in 0..500 {
        strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL);
        if loaded(&m) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    // (olga's check answered first: the request held is the restore's or the trim's)
    for _ in 0..500 {
        strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL);
        if m.links.check_of(LOCAL) != super::bank_links::Check::Checking {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let release = s.hold();
    if trim {
        m.edit.open_trim();
        m.edit.set_trim_keep(2);
        m.edit.ask_trim();
        m.edit.confirm_trim();
    } else {
        m.edit.ask_restore(1);
        m.edit.confirm_restore(false);
    }
    assert!(m.edit.acting());
    for _ in 0..500 {
        if s.paths().iter().any(|p| p.ends_with(path)) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    // pete signs in on this computer before the answer
    let during = strip_as(&ctx, &mut m, &mut banks, &mut acct, &pete, LOCAL);
    // "History of the bank" in "...": refused while it is on its way, and the strip says to wait
    assert!(!m.edit.open_history(CID, BID, "Unit 2 quiz", owner()));
    let during = during + &strip_as(&ctx, &mut m, &mut banks, &mut acct, &pete, LOCAL);
    // (pete's checks answered first: the answer queued is the restore's or the trim's)
    for _ in 0..500 {
        strip_as(&ctx, &mut m, &mut banks, &mut acct, &pete, LOCAL);
        if m.links.check_of(LOCAL) != super::bank_links::Check::Checking {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    match reply {
        Some((st, body)) => {
            s.on("Post", path, st, body);
        }
        None => {
            s.fail(super::api::ApiError::SignedOut);
        }
    }
    release.send(()).unwrap();
    // (the model's own frames only: no check takes the answer queued)
    for _ in 0..500 {
        m.edit.frame(now(), &pete);
        if !m.edit.acting() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let after = strip_as(&ctx, &mut m, &mut banks, &mut acct, &pete, LOCAL) + &strip_as(&ctx, &mut m, &mut banks, &mut acct, &pete, LOCAL);
    (m, s, banks, ctx, acct, during, after)
}

#[test]
fn m03_minor4_another_account_is_not_told_of_the_first_ones_restore_nor_that_it_is_up_to_date_meanwhile() {
    let (_, _, _, _, _, during, after) = held_then_another_account(&rollback_path(BID), Some((200, rolled(4, 1))), false);
    assert!(!during.contains("Restoring") && !during.contains("restore"), "{during}");
    assert!(!during.contains("up to date"), "{during}");
    assert!(during.contains("Waiting for an earlier request on this computer to finish"), "{during}");
    assert!(!after.contains("restored"), "{after}");
}

#[test]
fn m11_another_account_is_not_told_how_the_first_ones_trim_went() {
    let (_, _, _, _, _, during, after) = held_then_another_account(&trim_path(BID), Some((200, json!({"ok": true, "pruned": 1, "oldest_kept": 2, "pinned_versions": []}))), true);
    assert!(!during.contains("Trimming"), "{during}");
    assert!(!after.contains("trim"), "{after}");
}

#[test]
fn minor2_the_first_accounts_signed_out_answer_does_not_sign_the_next_one_out() {
    let (mut m, _, _, _, _, _, after) = held_then_another_account(&rollback_path(BID), None, false);
    assert!(!after.contains("Sign in to send") && !after.contains("Your session ended"), "{after}");
    assert!(!m.edit.take_signed_out());
}

#[test]
fn m10_a_restore_whose_session_ended_left_behind_says_so_on_the_strip() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    m.edit.ask_restore(1);
    s.fail(super::api::ApiError::SignedOut);
    m.edit.confirm_restore(false);
    m.edit.back();
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("Your session ended before the restore of version 1 was answered"), "{t}");
}

#[test]
fn m12_a_trim_whose_session_ended_left_behind_says_so_on_the_strip() {
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
    s.fail(super::api::ApiError::SignedOut);
    m.edit.confirm_trim();
    m.edit.back();
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("Your session ended before the trim was answered"), "{t}");
}

#[test]
fn minor7_a_trim_left_behind_keeps_the_line_about_versions_kept_for_review() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    m.edit.open_trim();
    m.edit.set_trim_keep(1);
    m.edit.ask_trim();
    s.on("Post", &trim_path(BID), 200, json!({"ok": true, "pruned": 1, "oldest_kept": 2, "pinned_versions": [2], "evidence_kept": 1}));
    m.edit.confirm_trim();
    m.edit.back();
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("The trim deleted the content of 1 older version.") && t.contains("kept while an administrator reviews it"), "{t}");
}

// ---- c71 minor 3: one client serves every account ---------------------------------------------------------------------

/// A session whose signed-in account changes during the first request (another account signs in while it waits), and
/// whose first answer asks to try again.
struct Switching {
    account: Mutex<Option<String>>,
    seen: Mutex<Vec<String>>,
}

impl PoolSession for Switching {
    fn base(&self) -> String {
        "https://pool.test".into()
    }
    fn account(&self) -> Option<String> {
        self.account.lock().unwrap().clone()
    }
    fn send<'a>(&'a self, build: &'a dyn Fn() -> super::transport::Request) -> super::transport::LocalFuture<'a, Result<super::transport::Response, super::api::ApiError>> {
        Box::pin(async move {
            let r = build();
            let who = self.account.lock().unwrap().clone().unwrap_or_default();
            let path = r.url.trim_start_matches("https://pool.test").to_string();
            self.seen.lock().unwrap().push(format!("{path} as {who}"));
            if path.ends_with("/versions") {
                return Ok(super::transport::Response::new(200, &page(3, &[3, 2, 1]).to_string()));
            }
            *self.account.lock().unwrap() = Some("pete".into());
            Ok(super::transport::Response::new(503, &json!({"error": "busy"}).to_string()))
        })
    }
}

#[test]
fn minor3_a_retry_after_another_account_signed_in_is_not_sent_with_that_accounts_sign_in() {
    let session = Arc::new(Switching { account: Mutex::new(Some("olga".into())), seen: Mutex::new(Vec::new()) });
    let snap = super::publish::AccountSnapshot::ready(session.clone());
    let media: super::publish::SharedMedia = Arc::new(std::collections::HashMap::<String, Vec<u8>>::new());
    let mut e = CircleEditModel::new(super::model::Runner::Inline, Arc::new(Mem::default()), media, Arc::new(NoDiskRestore), LinkLog::memory()).with_retry(RetryPolicy::instant(4));
    e.set_clock(NOW);
    e.frame(now(), &snap);
    assert!(e.open_history(CID, BID, "Unit 2 quiz", owner()));
    e.frame(now(), &snap);
    e.ask_restore(1);
    e.confirm_restore(false);
    e.frame(now(), &snap);
    let seen = session.seen.lock().unwrap().clone();
    assert_eq!(seen.iter().filter(|p| p.contains("/rollback")).count(), 1, "{seen:?}");
    assert!(!seen.iter().any(|p| p.ends_with("as pete")), "{seen:?}");
}

#[test]
fn minor3_a_pinned_session_sends_for_its_own_account_only() {
    let inner = Arc::new(Switching { account: Mutex::new(Some("olga".into())), seen: Mutex::new(Vec::new()) });
    let p = pinned(inner.clone());
    assert_eq!(p.account().as_deref(), Some("olga"));
    let req = || super::transport::Request { url: "https://pool.test/circles/x/banks/y/versions".into(), ..Default::default() };
    let r = super::transport::block_on(p.send(&req));
    assert!(r.is_ok());
    *inner.account.lock().unwrap() = Some("pete".into());
    let r = super::transport::block_on(p.send(&req));
    assert!(matches!(r, Err(super::api::ApiError::SignedOut)), "{r:?}");
    assert_eq!(inner.seen.lock().unwrap().len(), 1);
    // a session that does not know its account is never held back
    *inner.account.lock().unwrap() = None;
    let q = pinned(inner.clone());
    *inner.account.lock().unwrap() = Some("vera".into());
    assert!(super::transport::block_on(q.send(&req)).is_ok());
}

// ---- c71 minors: adds ------------------------------------------------------------------------------------------------

#[test]
fn m33_an_add_asked_for_is_counted_at_once_so_the_next_start_says_it() {
    let log = super::circles::AddsLog::memory();
    let (mut m, s) = open_circle("editor", json!({}));
    m.set_adds_log(log.clone());
    let a = as_who(&s, "olga");
    m.note_account(&a);
    m.add_bank(BID, false);
    drop(m);
    let mut m = model();
    m.set_adds_log(log);
    assert!(rows(&mut m).contains("verSHion closed before 1 bank you asked to add was finished"));
}

#[test]
fn minor1_adds_finished_on_the_shared_page_are_not_said_as_unfinished_on_the_next_start() {
    let log = super::circles::AddsLog::memory();
    let (mut m, s) = open_circle("editor", json!({}));
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(2, &[(QA, "One")], false));
    m.set_adds_log(log.clone());
    let a = as_who(&s, "olga");
    m.note_account(&a);
    m.add_bank(BID, false);
    let (mut banks, mut b) = (Vec::new(), browse());
    for _ in 0..6 {
        m.frame(now(), &a);
        m.sync(&mut banks, &mut b);
    }
    assert_eq!(banks.len(), 1, "made on the Shared page");
    drop(m);
    let mut m = model();
    m.set_adds_log(log);
    let t = rows(&mut m);
    assert!(!t.contains("closed before"), "{t}");
}

#[test]
fn m34_try_again_adds_with_the_role_the_person_has_now() {
    let (mut m, s) = open_circle("editor", json!({}));
    let a = as_who(&s, "olga");
    m.note_account(&a);
    let failed = super::browse::Problem { message: "verSHion's server did not answer.".into(), retry: true, retry_at: None, account: false };
    m.adds.push(super::circles::AddJob { cid: CID.into(), bid: BID.into(), own: false, title: "Cells".into(), role: Role::Viewer, state: super::circles::AddState::Failed(failed), full: false, dismissed: false, account: m.account_tag() });
    m.retry_add(CID, BID);
    assert_eq!(m.adds[0].role, Role::Editor);
    assert_eq!(m.adds[0].state, super::circles::AddState::Waiting);
}

#[test]
fn minor5_the_first_account_signed_in_again_is_told_its_add_was_not_made() {
    let (mut m, s) = open_circle("editor", json!({}));
    let (olga, pete) = (as_who(&s, "olga"), as_who(&s, "pete"));
    m.note_account(&olga);
    m.add_bank(BID, false);
    m.note_account(&pete);
    assert!(rows(&mut m).contains("1 bank that another account on this computer asked to add was not added"));
    m.note_account(&olga);
    let t = rows(&mut m);
    assert!(t.contains("\"Cells\" was not added: another account signed in before it was finished. Add it again from the circle's page."), "{t}");
    assert!(!t.contains("another account on this computer asked"), "{t}");
}

// ---- c72 I1, I3: words ---------------------------------------------------------------------------------------------

#[test]
fn c72_i1_the_merge_reviews_notes_say_the_circle_changed_the_question_never_the_server() {
    use super::circles_text::merge_words;
    let cases = [
        ("The pool dropped this question. It is kept unless you remove it. Kept questions are no longer linked to the pool.", "The circle removed this question. It is kept unless you remove it. Kept questions are no longer linked to the circle."),
        ("The pool dropped this question. You changed it, so it is kept unless you remove it.", "The circle removed this question. You changed it, so it is kept unless you remove it."),
        ("You and the pool made the same change.", "You and the circle made the same change."),
        ("The pool changed this question and you entered its answer key. The pool's wording is taken and your key is kept.", "The circle changed this question and you entered its answer key. The circle's wording is taken and your key is kept."),
        ("The pool's version changed while you were reviewing. Nothing was applied; start the merge again.", "The circle's version changed while you were reviewing. Nothing was applied; press Get latest again."),
        ("The pool's version is missing a question that was reviewed.", "The circle's version is missing a question that was reviewed."),
    ];
    for (from, want) in cases {
        assert_eq!(merge_words(from), want);
    }
    // Every word merge.rs can put on a circle's review, once mapped: no "pool", never verSHion or its server.
    for (line, lit) in literals(include_str!("merge.rs")) {
        if lit.starts_with("From the pool: ") || lit.starts_with("From the shared pool:") {
            continue; // a topic written into the bank (renamed for a circle when applied), never drawn here
        }
        let c = merge_words(&lit);
        assert!(!says_pool(&c) && !c.contains("verSHion"), "merge.rs:{line}: {lit} -> {c}");
    }
}

#[test]
fn c72_i1_a_review_with_a_question_the_circle_removed_reads_the_circle_removed_it() {
    let s = Script::new();
    s.keep("Post", "/circles/heads", 200, heads(2));
    let mut m = model();
    let (a, l) = linked(&[(QA, "One"), (QB, "Two")]);
    m.edit.add_link(l).unwrap();
    let mut banks = vec![a];
    s.on("Post", &format!("/circles/{CID}/banks/{BID}/merge"), 200, json!({"bank_id": BID, "base_version": 1, "base_kept": true, "oldest_kept": 1, "head_version": 2, "head_version_id": hex(2), "head_at": NOW - 1000, "head_by": "Ms Frizzle", "between": [], "between_total": 1, "merge_token": "tok.en", "merge_token_expires_at": NOW + 1_800_000}));
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(2, &[(QA, "One")], false));
    m.edit.frame(now(), &ready(&s));
    assert!(m.edit.open_merge(LOCAL, editor()));
    for _ in 0..3 {
        m.edit.frame(now(), &ready(&s));
        m.edit.sync(&mut banks);
    }
    assert!(matches!(&m.edit.view, EditView::Merge(v) if matches!(v.phase, MergePhase::Review(_))), "{:?}", m.edit.view);
    let t = text_of(1280.0, &mut |ui| super::circle_edit_ui::draw_page(ui, &mut m));
    assert!(t.contains("The circle removed this question") && t.contains("no longer linked to the circle"), "{t}");
    assert!(!t.contains("verSHion") && !says_pool(&t), "{t}");
}

#[test]
fn c72_i3_sharing_into_a_circle_says_later_changes_are_sent_from_the_banks_page() {
    let n = super::circle_send::NEW_BANK_NOTE;
    assert!(!n.contains("later update"), "{n}");
    assert!(n.contains("links this bank to it") && n.contains("My Banks"), "{n}");
}

// ---- c71 minor 6 (critic 69 minor 7): a restore with no answer holds every bank for a minute at most ----------------

#[test]
fn minor6_a_restore_with_no_answer_is_waited_for_a_minute_then_the_other_banks_open_again() {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &loaded);
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| m.links.check_of(LOCAL) != super::bank_links::Check::Checking);
    m.edit.ask_restore(1);
    s.on("Post", &rollback_path(BID), 200, rolled(4, 1));
    let release = s.hold();
    m.edit.confirm_restore(false);
    m.edit.back();
    // 59 seconds: still waited for; B's History is refused
    m.edit.frame(now() + std::time::Duration::from_secs(59), &ready(&s));
    assert!(m.edit.acting());
    assert!(!m.edit.open_history(CID, &hex(77), "Plants", owner()));
    // a minute: no longer
    m.edit.frame(now() + super::circle_edit::ACT_PATIENCE, &ready(&s));
    assert!(!m.edit.acting());
    assert!(m.edit.wait_note().is_none());
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("No answer came for the restore of version 1 within a minute. It may still go through"), "{t}");
    assert!(!t.contains("Restoring"), "{t}");
    s.on("Get", &format!("/circles/{CID}/banks/{}/versions", hex(77)), 200, page(1, &[1]));
    assert!(m.edit.open_history(CID, &hex(77), "Plants", owner()), "B's History opens again");
    let _ = release.send(());
}

#[test]
fn a_sign_out_and_the_same_account_signing_in_again_keeps_the_results_another_account_never_sees_them() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    let (olga, pete) = (as_who(&s, "olga"), as_who(&s, "pete"));
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("owner", json!({})));
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL);
    make_stale(&mut m, LOCAL);
    treat_in_step(&mut m, &s, &mut banks, LOCAL, BID, false);
    assert!(strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL).contains("now used as it is"));
    let out = super::publish::AccountSnapshot::of_gate(super::publish::Gate::SignedOut);
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &out, LOCAL);
    assert!(strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL).contains("now used as it is"), "the same account, back");
    assert!(!strip_as(&ctx, &mut m, &mut banks, &mut acct, &pete, LOCAL).contains("now used as it is"), "another account");
}

// ---- critic 71's other survivors, pinned --------------------------------------------------------------------------

#[test]
fn m04_a_trim_on_its_way_says_trimming_on_the_strip_and_never_up_to_date() {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &loaded);
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| m.links.check_of(LOCAL) != super::bank_links::Check::Checking);
    m.edit.open_trim();
    m.edit.set_trim_keep(2);
    m.edit.ask_trim();
    s.on("Post", &trim_path(BID), 200, json!({"ok": true, "pruned": 1, "oldest_kept": 2, "pinned_versions": []}));
    let release = s.hold();
    m.edit.confirm_trim();
    m.edit.back();
    let t = strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("Trimming the bank's old versions in the circle...") && !t.contains("up to date"), "{t}");
    release.send(()).unwrap();
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| !m.edit.acting());
}

#[test]
fn m05_a_restore_whose_job_died_left_behind_says_it_did_not_go_through() {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &loaded);
    until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| m.links.check_of(LOCAL) != super::bank_links::Check::Checking);
    m.edit.ask_restore(1);
    // (no answer scripted: the job's thread ends without one)
    m.edit.confirm_restore(false);
    m.edit.back();
    let t = until(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL, &|m| !m.edit.acting());
    let t = t + &strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL);
    assert!(t.contains("The restore of version 1 did not go through: verSHion's server did not answer."), "{t}");
}

#[test]
fn m15_a_circle_bank_with_no_copy_here_has_its_results_above_its_own_history() {
    let (mut m, s, _) = two_at_three();
    let z = hex(99);
    let zpage = json!({"bank_id": z, "head_version": 3, "oldest_kept": 1, "pinned_versions": [], "versions": [{"version": 3, "version_id": hex(3), "by": "Mr Keesler", "at": NOW, "message": "", "added": 0, "changed": 1, "removed": 0, "question_count": 1, "license": "CC-BY-4.0", "kept": true, "withheld": false}, {"version": 1, "version_id": hex(1), "by": "Mr Keesler", "at": NOW - DAY, "message": "", "added": 1, "changed": 0, "removed": 0, "question_count": 1, "license": "CC-BY-4.0", "kept": true, "withheld": false}], "next_before": null});
    s.on("Get", &format!("/circles/{CID}/banks/{z}/versions"), 200, zpage.clone());
    m.edit.frame(now(), &ready(&s));
    assert!(m.edit.open_history(CID, &z, "Unit 3 test", owner()));
    m.edit.frame(now(), &ready(&s));
    m.edit.ask_restore(1);
    s.on("Post", &rollback_path(&z), 503, json!({"error": "unavailable"}));
    m.edit.confirm_restore(false);
    m.edit.back();
    m.edit.frame(now(), &ready(&s));
    s.on("Get", &format!("/circles/{CID}/banks/{z}/versions"), 200, zpage);
    assert!(m.edit.open_history(CID, &z, "Unit 3 test", owner()));
    m.edit.frame(now(), &ready(&s));
    let t = text_of(1280.0, &mut |ui| super::circle_edit_ui::draw_page(ui, &mut m));
    assert!(t.contains("The restore of version 1 did not go through") && t.contains("History: \"Unit 3 test\""), "{t}");
}

/// A's History on its page, a restore answered while it is shown (its result is on the History, not closed).
fn a_result_on_as_history(ctx: &egui::Context, m: &mut CirclesModel, banks: &mut [QuestionBank], acct: &mut FakeAccount, s: &Arc<Script>) {
    strip(ctx, m, banks, acct, s, LOCAL);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(m);
    strip(ctx, m, banks, acct, s, LOCAL);
    strip(ctx, m, banks, acct, s, LOCAL);
    m.edit.ask_restore(1);
    s.on("Post", &rollback_path(BID), 200, rolled(4, 1));
    s.on("Get", &versions(), 200, page(4, &[4, 3, 2, 1]));
    m.edit.confirm_restore(false);
    let t = strip(ctx, m, banks, acct, s, LOCAL);
    assert!(t.contains("Done. Version 4 is now the circle's newest"), "{t}");
    assert!(m.edit.results_of(LOCAL).is_empty(), "said on the History, once");
}

fn result_on_strip(t: &str) -> bool {
    t.contains("Version 4 was restored from version 1.")
}

#[test]
fn m21_the_circles_tab_opened_over_a_result_not_closed_says_it_on_the_strip() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    a_result_on_as_history(&ctx, &mut m, &mut banks, &mut acct, &s);
    m.open_list();
    assert!(result_on_strip(&strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL)));
}

#[test]
fn m22_back_over_a_result_not_closed_says_it_on_the_strip() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    a_result_on_as_history(&ctx, &mut m, &mut banks, &mut acct, &s);
    m.edit.back();
    assert!(result_on_strip(&strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL)));
}

#[test]
fn m23_m24_another_banks_history_or_send_opened_over_a_result_not_closed_says_it_on_its_strip() {
    for send in [false, true] {
        let (mut m, s, mut banks) = two_at_three();
        let ctx = ctx();
        let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
        a_result_on_as_history(&ctx, &mut m, &mut banks, &mut acct, &s);
        if send {
            s.send_base_404(true);
            assert!(m.edit.open_send(LOCAL_B, owner()));
        } else {
            s.on("Get", &format!("/circles/{CID}/banks/{}/versions", hex(77)), 200, page(1, &[1]));
            assert!(m.edit.open_history(CID, &hex(77), "Plants", owner()));
        }
        m.edit.host = Some(EditHost { local_id: LOCAL_B.into(), back: "< Back to Plants".into() });
        m.edit.host_frame = None;
        assert!(m.edit.results_of(LOCAL).iter().any(|(_, t)| result_on_strip(t)), "send: {send}");
        assert!(result_on_strip(&strip(&ctx, &mut m, &mut banks, &mut acct, &s, LOCAL)), "send: {send}");
    }
}

#[test]
fn n02_a_result_left_on_a_history_no_longer_drawn_is_said_on_the_strip() {
    let (mut m, s, _) = two_at_three();
    let snap = ready(&s);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    m.edit.set_pass(10);
    m.edit.frame(now(), &snap);
    // Bank settings on the Shared page: drawn every frame
    assert!(m.edit.open_history(CID, BID, "Unit 2 quiz", owner()));
    m.edit.mark_drawn(10);
    m.edit.set_pass(11);
    m.edit.frame(now(), &snap);
    m.edit.mark_drawn(11);
    m.edit.ask_restore(1);
    s.on("Post", &rollback_path(BID), 200, rolled(4, 1));
    s.on("Get", &versions(), 200, page(4, &[4, 3, 2, 1]));
    m.edit.confirm_restore(false);
    m.edit.set_pass(12);
    m.edit.frame(now(), &snap);
    m.edit.mark_drawn(12);
    assert!(matches!(&m.edit.view, EditView::History(h) if matches!(h.restore.as_ref().map(|r| &r.stage), Some(RestoreStage::Done { .. }))), "shown on the History");
    assert!(m.edit.results_of(LOCAL).is_empty());
    // The person goes to A's page in My Banks: the History is no longer drawn.
    m.edit.set_pass(13);
    m.edit.frame(now(), &snap);
    m.edit.set_pass(14);
    m.edit.frame(now(), &snap);
    assert!(m.edit.results_of(LOCAL).iter().any(|(_, t)| result_on_strip(t)), "{:?}", m.edit.results_of(LOCAL));
    assert!(matches!(&m.edit.view, EditView::History(h) if h.restore.is_none()), "said once: the History no longer holds it");
}

#[test]
fn m25_m26_the_strip_of_a_restore_on_its_way_offers_nothing_and_shows_it_is_busy() {
    use super::bank_links::{Check, SignIn, StripInput, strip_view};
    let (_, mut l) = linked(&[(QA, "One")]);
    l.role_seen = Some("owner".into());
    let v = strip_view(&StripInput { link: &l, status: Some(&status(0, 0, true, false)), check: Check::Done, sign_in: SignIn::Ready, now_ms: NOW, act: Some("Restoring version 1 in the circle...".into()), sent_unread: false, changed_since: false, unsure: false });
    assert!(v.line.ends_with("Restoring version 1 in the circle...") && !v.line.contains("up to date"), "{}", v.line);
    assert!(v.buttons.is_empty() && v.busy, "{v:?}");
}

#[test]
fn m35_a_copy_read_for_bank_settings_before_a_sign_out_is_not_put_into_the_banks_after() {
    let (mut m, s) = open_circle("editor", json!({}));
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(2, &[(QA, "One")], false));
    let olga = as_who(&s, "olga");
    m.note_account(&olga);
    m.open_bank(BID);
    for _ in 0..3 {
        m.frame(now(), &olga);
    }
    m.begin_copy();
    m.get_copy();
    m.frame(now(), &olga);
    // signed out before the copy was put into the banks (only an add's copy is kept for the same account)
    m.note_account(&super::publish::AccountSnapshot::of_gate(super::publish::Gate::SignedOut));
    m.note_account(&olga);
    let (mut banks, mut b) = (Vec::new(), browse());
    for _ in 0..3 {
        m.frame(now(), &olga);
        m.sync(&mut banks, &mut b);
    }
    assert!(banks.is_empty(), "{}", banks.len());
}

// ---- round 5's own survivors, pinned --------------------------------------------------------------------------------

/// A's Send page on its page, its send answered "signed out" while it is shown.
fn a_send_whose_session_ended(ctx: &egui::Context, m: &mut CirclesModel, banks: &mut [QuestionBank], acct: &mut FakeAccount, s: &Arc<Script>) -> String {
    s.send_base_404(true);
    strip(ctx, m, banks, acct, s, LOCAL);
    banks[0].questions[0].stem = "One (my fix)".into();
    banks[0].modified += 5;
    assert!(m.edit.open_send(LOCAL, editor()));
    m.edit.host = Some(EditHost { local_id: LOCAL.into(), back: "< Back to Unit 2 quiz".into() });
    m.edit.host_frame = None;
    for _ in 0..3 {
        strip(ctx, m, banks, acct, s, LOCAL);
    }
    m.edit.set_attest(true);
    s.fail(super::api::ApiError::SignedOut);
    m.edit.send();
    m.edit.sync(banks);
    strip(ctx, m, banks, acct, s, LOCAL)
}

#[test]
fn n21_a_send_whose_session_ended_says_so_above_its_page() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    let t = a_send_whose_session_ended(&ctx, &mut m, &mut banks, &mut acct, &s);
    assert!(t.contains("Your session ended. Sign in again; where you were is kept, and nothing was lost.") && t.contains("Send my changes"), "{t}");
}

#[test]
fn n27_your_session_ended_goes_once_the_account_is_signed_in_again() {
    let (mut m, s, mut banks) = two_at_three();
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    a_send_whose_session_ended(&ctx, &mut m, &mut banks, &mut acct, &s);
    assert!(m.edit.results_of(LOCAL).iter().any(|(_, t)| t.starts_with("Your session ended")));
    m.edit.frame(now(), &super::publish::AccountSnapshot::of_gate(super::publish::Gate::SignedOut));
    assert!(m.edit.results_of(LOCAL).iter().any(|(_, t)| t.starts_with("Your session ended")), "kept while signed out");
    m.edit.frame(now(), &ready(&s));
    assert!(!m.edit.results_of(LOCAL).iter().any(|(_, t)| t.starts_with("Your session ended")), "{:?}", m.edit.results_of(LOCAL));
}

#[test]
fn n22_a_copy_put_into_the_banks_while_the_shared_page_shows_a_sign_in_step_is_no_longer_counted_unfinished() {
    let log = super::circles::AddsLog::memory();
    let (mut m, s) = open_circle("editor", json!({}));
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(2, &[(QA, "One")], false));
    m.set_adds_log(log.clone());
    let a = as_who(&s, "olga");
    m.note_account(&a);
    m.add_bank(BID, false);
    let (mut banks, mut b) = (Vec::new(), browse());
    m.tick_background(now(), &a);
    m.tick_background(now(), &a);
    assert!(super::circles::land_waiting(&mut banks, &mut b), "in the banks (the app saves them)");
    // The Shared page is drawn next, on a step of the sign-in (not its tabs): the copy gets its link there.
    let mut sh = super::shared_ui::SharedUi::default();
    sh.circles = m;
    let mut acct = super::ui::AccountUi::in_page();
    acct.model = super::model::AccountModel::new(Arc::new(|_| panic!("no client may be built")), super::model::Runner::Inline, super::model::SettingsIo::memory()).with_server_mode(super::model::server_mode(None, false));
    let (mut bu, mut mi): (super::BrowseUi, super::MineUi) = Default::default();
    text_of(1280.0, &mut |ui| {
        sh.draw(ui, &mut acct, &mut bu, &mut mi, &mut banks, &[]);
    });
    drop(sh);
    let mut m = model();
    m.set_adds_log(log);
    let t = rows(&mut m);
    assert!(!t.contains("closed before"), "{t}");
}

#[cfg(unix)]
#[test]
fn n32_a_check_that_could_not_be_saved_is_no_longer_said_once_the_list_is_saved() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("vershion-c71-resave-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let media: super::publish::SharedMedia = Arc::new(std::collections::HashMap::<String, Vec<u8>>::new());
    let mut e = CircleEditModel::new(super::model::Runner::Inline, Arc::new(Mem::default()), media, Arc::new(NoDiskRestore), LinkLog::Dir(dir.clone())).with_retry(RetryPolicy::none());
    let (_, l) = linked(&[(QA, "One")]);
    e.add_link(l).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    e.apply_heads(&[HeadEntry { bank_id: BID.into(), state: HeadState::Current { version: 5, license: None, at: Some(NOW), by: None, hide_key: None } }]);
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    let said = e.results_of(LOCAL).iter().any(|(_, t)| t.contains("could not be saved"));
    if said {
        e.update_link(LOCAL, |l| {
            l.head_seen = 6;
            true
        });
        assert!(std::fs::read_to_string(dir.join(LINKS_DOC)).unwrap().contains("\"head_seen\":6"));
        assert!(!e.results_of(LOCAL).iter().any(|(_, t)| t.contains("could not be saved")), "{:?}", e.results_of(LOCAL));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn n36_the_first_accounts_restore_whose_job_died_is_not_said_to_the_next_account() {
    let (mut m, s, mut banks) = two_at_three();
    background(&mut m);
    let ctx = ctx();
    let mut acct = FakeAccount { open: true, ..FakeAccount::default() };
    let (olga, pete) = (as_who(&s, "olga"), as_who(&s, "pete"));
    s.keep("Get", &format!("/circles/{CID}"), 200, detail("owner", json!({})));
    strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL);
    s.on("Get", &versions(), 200, page(3, &[3, 2, 1]));
    history_on_a(&mut m);
    for _ in 0..500 {
        strip_as(&ctx, &mut m, &mut banks, &mut acct, &olga, LOCAL);
        if loaded(&m) && m.links.check_of(LOCAL) != super::bank_links::Check::Checking {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let release = s.hold();
    m.edit.ask_restore(1);
    // (no answer scripted: once let go, the job's thread ends without one)
    m.edit.confirm_restore(false);
    for _ in 0..500 {
        if s.paths().iter().any(|p| p.ends_with("/rollback")) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    m.note_account(&pete);
    release.send(()).unwrap();
    for _ in 0..500 {
        m.edit.frame(now(), &pete);
        if !m.edit.acting() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(!m.edit.acting());
    assert!(m.edit.results_of(LOCAL).is_empty(), "{:?}", m.edit.results_of(LOCAL));
}
