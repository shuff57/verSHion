//! The Shared page: the tab and guided-step state machine, and a headless frame of every page and
//! state. Nothing here touches the network; nobody has seen these pages on a real screen.

use std::sync::Arc;

use super::api::Status;
use super::model::{AccountModel, MailKind, Page, Runner, SettingsIo, server_mode};
use super::publish::Gate;
use super::shared::{Guide, STEPS, SharedModel, Tab, guide_of, tabs_for};
use super::shared_ui::{SharedUi, goto, take_goto};
use super::ui::AccountUi;

fn rejected() -> Status {
    Status::Rejected { revoked: false, reason: Some("Not a teacher".into()), reapply_after_ms: None, can_reapply: false }
}

#[test]
fn every_account_state_maps_to_one_guided_step_or_note() {
    let g = |gate: Gate, page: Page, needed: bool, redeemed: bool| guide_of(&gate, page, needed, redeemed);
    assert_eq!(g(Gate::Checking, Page::Opening, false, false), Guide::Checking);
    assert_eq!(g(Gate::NotSetUp, Page::NotSetUp, false, false), Guide::NotSetUp);
    assert_eq!(g(Gate::NoServer, Page::Server, false, false), Guide::NoServer);
    assert_eq!(g(Gate::Unreachable, Page::Status, false, false), Guide::Offline);
    assert_eq!(g(Gate::SignedOut, Page::SignIn, false, false), Guide::SignIn);
    assert_eq!(g(Gate::SignedOut, Page::SignUp, false, false), Guide::SignIn);
    assert_eq!(g(Gate::SignedOut, Page::Forgot, false, false), Guide::SignIn);
    assert_eq!(g(Gate::SignedOut, Page::CheckEmail(MailKind::Verify), false, false), Guide::ConfirmEmail);
    assert_eq!(g(Gate::SignedOut, Page::Confirm, false, false), Guide::ConfirmEmail);
    assert_eq!(g(Gate::NotApproved(Status::Unverified), Page::Status, false, false), Guide::ConfirmEmail);
    assert_eq!(g(Gate::NotApproved(Status::Pending), Page::Status, false, false), Guide::Waiting);
    assert_eq!(g(Gate::NotApproved(rejected()), Page::Status, false, false), Guide::Blocked(rejected()));
    assert_eq!(g(Gate::Ready, Page::Status, true, false), Guide::NeedsInvite);
    assert_eq!(g(Gate::Ready, Page::Status, true, true), Guide::Ready, "a redeemed code lets the tabs come back");
    assert_eq!(g(Gate::Ready, Page::Status, false, false), Guide::Ready);
}

#[test]
fn a_confirmed_pending_account_gets_circles_and_settings_only() {
    // Round 54: a confirmed account waiting for approval may use circles, not the public pool's tabs.
    let pending = Gate::NotApproved(Status::Pending);
    assert_eq!(tabs_for(&pending), &[Tab::Circles, Tab::Settings]);
    for t in [Tab::Browse, Tab::Mine, Tab::Imported, Tab::Reports] {
        assert!(!tabs_for(&pending).contains(&t), "{t:?} waits for approval");
    }
    assert_eq!(tabs_for(&Gate::Ready), &Tab::ALL);
}

#[test]
fn the_waiting_copy_says_a_circle_code_works_now() {
    let help = Guide::Waiting.help();
    assert!(help.contains("circle code"), "{help}");
    let (_, body) = super::ui::status_text(&Status::Pending);
    assert!(body.contains("circle code"), "{body}");
    let (_, more) = super::publish_ui::status_line(&Status::Pending);
    assert!(more.contains("circle"), "{more}");
}

#[test]
fn the_steps_run_in_order_and_notes_have_none() {
    let order = [Guide::SignIn, Guide::ConfirmEmail, Guide::Waiting, Guide::NeedsInvite, Guide::Ready];
    for (i, s) in order.iter().enumerate() {
        assert_eq!(s.step(), Some(i as u8 + 1));
        assert!(!s.title().is_empty());
    }
    assert_eq!(order.len() as u8, STEPS);
    for n in [Guide::Checking, Guide::NotSetUp, Guide::NoServer, Guide::Offline, Guide::Blocked(Status::Pending)] {
        assert_eq!(n.step(), None);
        assert!(!n.help().is_empty() && !n.title().is_empty());
    }
}

#[test]
fn tabs_switch_and_the_invite_flag_follows_the_pool() {
    let mut m = SharedModel::default();
    assert_eq!(m.tab, Tab::Browse);
    m.toggle_chooser();
    assert!(m.choosing);
    m.goto(Tab::Imported);
    assert_eq!(m.tab, Tab::Imported);
    assert!(!m.choosing, "changing tab closes the chooser");
    assert_eq!(m.guide(&Gate::Ready, Page::Status, false), Guide::Ready);
    m.note_invite_required();
    assert_eq!(m.guide(&Gate::Ready, Page::Status, false), Guide::NeedsInvite);
    m.note_redeemed();
    assert_eq!(m.guide(&Gate::Ready, Page::Status, false), Guide::Ready);
    assert_eq!(Tab::ALL.len(), 6);
    assert!(Tab::ALL.iter().all(|t| !t.label().is_empty()));
}

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    ctx
}

fn account(configured: bool) -> AccountUi {
    let mut a = AccountUi::in_page();
    a.model = AccountModel::new(Arc::new(|_| panic!("no client may be built")), Runner::Inline, SettingsIo::memory()).with_server_mode(server_mode(None, !configured));
    a
}

fn frame(ctx: &egui::Context, width: f32, f: &mut dyn FnMut(&mut egui::Ui)) {
    for i in 0..2 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 1400.0))),
            time: Some(i as f64 * 0.1),
            ..Default::default()
        };
        let out = ctx.run_ui(input, |ui| f(ui));
        out.drop_without_applying_deltas();
    }
}

#[test]
fn the_page_asks_for_the_account_instead_of_opening_a_window() {
    let ctx = ctx();
    let mut a = account(false);
    let mut s = SharedUi::default();
    assert!(!take_goto(&ctx, &mut s, &mut a));
    a.open();
    assert!(a.is_open() && !a.visible(), "in the app the floating window is never drawn");
    assert!(take_goto(&ctx, &mut s, &mut a));
    assert_eq!(s.model.tab, Tab::Settings);
    assert!(!take_goto(&ctx, &mut s, &mut a), "a request is read once");
    goto(&ctx, Tab::Mine);
    assert!(take_goto(&ctx, &mut s, &mut a));
    assert_eq!(s.model.tab, Tab::Mine);
    // a quiet lookup is not closed under the page
    a.pin();
    a.open_quiet();
    a.close_quiet();
    assert!(a.is_open());
    // the old window behaviour is untouched
    let mut w = AccountUi::default();
    w.open();
    assert!(w.visible() && !w.take_wants_page());
}

#[test]
fn smoke_frames_of_every_guided_state_at_wide_and_narrow_widths() {
    let ctx = ctx();
    for width in [1000.0, 420.0] {
        // sharing off in this build
        let mut a = account(false);
        let (mut s, mut b, mut m) = (SharedUi::default(), Default::default(), Default::default());
        frame(&ctx, width, &mut |ui| {
            s.draw(ui, &mut a, &mut b, &mut m, &mut Vec::new(), &[]);
        });
        // a configured account that has not signed in
        let mut a = account(true);
        let (mut s, mut b, mut m): (SharedUi, super::BrowseUi, super::MineUi) = (SharedUi::default(), Default::default(), Default::default());
        for page in [Page::SignIn, Page::SignUp, Page::Forgot, Page::CheckEmail(MailKind::Verify), Page::Confirm] {
            a.model.page = page;
            frame(&ctx, width, &mut |ui| {
            s.draw(ui, &mut a, &mut b, &mut m, &mut Vec::new(), &[]);
        });
        }
        // the cards for every note and step, directly
        for g in [Guide::Checking, Guide::NotSetUp, Guide::Offline, Guide::Waiting, Guide::Blocked(rejected()), Guide::NeedsInvite] {
            frame(&ctx, width, &mut |ui| s.guide_card(ui, &g, &mut a));
        }
    }
}

#[test]
fn the_tab_row_and_the_step_pills_wrap_inside_a_narrow_page() {
    // Seen on screen: a tab or step pill that did not fit was squeezed to one letter per line and
    // pushed the page past the window's right edge.
    let ctx = ctx();
    let mut a = account(true);
    for width in [420.0, 620.0, 760.0] {
        let (mut b, mut m): (super::BrowseUi, super::MineUi) = Default::default();
        let mut s = SharedUi::default();
        let mut right = 0.0f32;
        frame(&ctx, width, &mut |ui| {
            s.tabs(ui, &mut a, &mut b, &mut m, &mut Vec::new(), &[], false);
            right = right.max(ui.min_rect().right());
        });
        assert!(right <= width + 0.5, "tabs reach {right} in a {width} wide page");
        let mut right = 0.0f32;
        frame(&ctx, width, &mut |ui| {
            s.guide_card(ui, &Guide::NeedsInvite, &mut a);
            right = right.max(ui.min_rect().right());
        });
        assert!(right <= width + 0.5, "step pills reach {right} in a {width} wide page");
    }
}

#[test]
fn smoke_frames_of_every_tab_body() {
    let ctx = ctx();
    let mut a = account(true);
    let mut banks = vec![];
    for width in [1000.0, 420.0] {
        let (mut b, mut m): (super::BrowseUi, super::MineUi) = Default::default();
        let mut s = SharedUi::default();
        for t in Tab::ALL {
            s.model.goto(t);
            frame(&ctx, width, &mut |ui| {
                s.tabs(ui, &mut a, &mut b, &mut m, &mut banks, &[], false);
            });
        }
        s.model.toggle_chooser();
        frame(&ctx, width, &mut |ui| {
                s.tabs(ui, &mut a, &mut b, &mut m, &mut banks, &[], false);
            });
    }
}

// ---- critic 61 I3: the invite step and another account ----------------------------------------------------------

#[test]
fn c61_the_invite_box_and_its_result_do_not_carry_over_to_another_account() {
    use super::invite::{InviteModel, InviteState};
    use super::publish::AccountSnapshot;
    let snap = |email: &str| AccountSnapshot { account: Some(format!("https://pool.test\n{email}")), ..AccountSnapshot::of_gate(Gate::Ready) };
    // A redeemed a code; the pool asked A for one before.
    let mut m = SharedModel::default();
    let mut inv = InviteModel::new(Runner::Inline);
    m.note_account(&snap("ada@school.org"), &mut inv);
    inv.state = InviteState::Redeemed { already: false };
    m.note_invite_required();
    m.note_account(&snap("ada@school.org"), &mut inv);
    assert!(inv.redeemed() && m.invite_needed, "the same account keeps it");
    // B signs in straight after: A's redemption does not count for B, and B is not told it needs a code.
    m.note_account(&snap("bob@school.org"), &mut inv);
    assert!(!inv.redeemed() && !m.invite_needed);
    // A code typed and not sent goes with a sign-out.
    inv.code = "ABCD-EFGH".into();
    m.note_account(&AccountSnapshot::of_gate(Gate::SignedOut), &mut inv);
    assert!(inv.code.is_empty());
}
