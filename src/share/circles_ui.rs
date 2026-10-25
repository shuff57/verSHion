//! The Circles tab (egui): only draws. All logic is in [`CirclesModel`]. Called inside a page card
//! of the Shared page; every server string goes through [`api::plain`].

use egui::{Margin, Ui};

use super::api;
use super::browse::Problem;
use super::browse_text as btext;
use super::circles::*;
use super::circles_api::{Invite, Role, code_groups};
use super::circles_text as text;
use super::pool::License;
use super::publish_text::{license_info, license_name};
use super::publish_ui::{banner, body, body_lines, heading, link, para, warn_box};
use crate::{kit, theme};

/// What the page asks the rest of the app for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    None,
    /// "Add a bank": open the Share window with this circle chosen.
    ShareToCircle(String),
    /// "See it under Things I imported".
    ShowImported,
}

pub(super) fn line(ui: &mut Ui, s: impl Into<String>) {
    ui.add(egui::Label::new(theme::muted(s.into()).size(12.5)).wrap());
}

pub(super) fn back_link(ui: &mut Ui, label: &str) -> bool {
    ui.add(egui::Button::new(theme::accent(label).size(12.5)).frame(false)).clicked()
}

pub(super) fn problem_box(ui: &mut Ui, p: &Problem, wait: Option<std::time::Duration>, retry: &mut bool, account: &mut bool) {
    warn_box(ui, &p.message);
    if !p.retry && !p.account {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        if p.retry {
            ui.add_enabled_ui(wait.is_none(), |ui| *retry = kit::primary_button(ui, "Try again").clicked());
        }
        if p.account {
            *account = kit::ghost_button(ui, "Open account...").clicked();
        }
    });
    if let Some(w) = wait {
        para(ui, &format!("You can try again in about {} seconds.", w.as_secs().max(1)));
    }
}

pub(super) fn wait_of(m: &CirclesModel, p: &Problem) -> Option<std::time::Duration> {
    let at = p.retry_at?;
    let now = m.now_instant();
    (at > now).then(|| at - now)
}

pub(super) fn section<R>(ui: &mut Ui, title: &str, f: impl FnOnce(&mut Ui) -> R) -> R {
    let r = kit::card()
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(theme::strong(title).size(14.5));
            ui.add_space(4.0);
            f(ui)
        })
        .inner;
    ui.add_space(8.0);
    r
}

/// A pill that a wrapping row moves to the next line whole (a framed `kit::pill` would be squeezed
/// into the row's cursor and stretched to the row's height).
pub(super) fn pill(ui: &mut Ui, label: &str, fill: egui::Color32, fg: egui::Color32) {
    super::shared_ui::step_pill(ui, label, fill, fg);
}

fn role_pill(ui: &mut Ui, r: Role) {
    let (fill, fg) = match r {
        Role::Owner => (theme::accent_col(), theme::on_accent_col()),
        _ => (theme::accent_faded(), theme::text_col()),
    };
    pill(ui, text::role_title(r), fill, fg);
}

/// A wrapping row of text and pills at text height (a plain row is a button's height, 34 px).
pub(super) fn text_row<R>(ui: &mut Ui, f: impl FnOnce(&mut Ui) -> R) -> R {
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = 18.0;
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.horizontal_wrapped(f).inner
    })
    .inner
}

/// One line in two tones: `main` in the text colour, then `sep` and `quiet` muted. Wraps as one label.
pub(super) fn two_tone(ui: &mut Ui, main: &str, sep: &str, quiet: &str) {
    let mut job = egui::text::LayoutJob::default();
    let style = ui.style().clone();
    theme::text(main).size(12.5).append_to(&mut job, &style, egui::FontSelection::Default, egui::Align::Center);
    if !quiet.is_empty() {
        theme::muted(format!("{sep}{quiet}")).size(12.5).append_to(&mut job, &style, egui::FontSelection::Default, egui::Align::Center);
    }
    ui.add(egui::Label::new(job).wrap());
}

fn capitalised(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// The width a `kit` pill button with this label takes.
fn button_width(ui: &Ui, label: &str) -> f32 {
    let t = egui::RichText::new(label).family(theme::bold_family());
    let g = egui::WidgetText::from(t).into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Button);
    g.size().x + 2.0 * ui.spacing().button_padding.x
}

/// A card's content with one button on the right, the content wrapping in the width left over. In a
/// narrow card the button goes under the content instead, so the two never overlap. `action` is the
/// label and whether it is the primary button; true when it was clicked.
fn with_action(ui: &mut Ui, action: Option<(&str, bool)>, body: impl FnOnce(&mut Ui)) -> bool {
    let Some((label, primary)) = action else {
        body(ui);
        return false;
    };
    let press = |ui: &mut Ui| if primary { kit::primary_button(ui, label).clicked() } else { kit::ghost_button(ui, label).clicked() };
    let bw = button_width(ui, label);
    let avail = ui.available_width();
    if avail < bw + 240.0 {
        body(ui);
        ui.add_space(4.0);
        return press(ui);
    }
    let mut clicked = false;
    ui.horizontal_top(|ui| {
        let w = avail - bw - ui.spacing().item_spacing.x;
        let h = ui
            .allocate_ui_with_layout(egui::vec2(w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.set_width(w);
                body(ui);
            })
            .response
            .rect
            .height();
        // The button is centred on the body's height (not on what is left of the page).
        let size = egui::vec2(ui.available_width(), h.max(ui.spacing().interact_size.y));
        ui.allocate_ui_with_layout(size, egui::Layout::right_to_left(egui::Align::Center), |ui| clicked = press(ui));
    });
    clicked
}

// ---- the tab ----------------------------------------------------------------------------------------------------

/// The Circles tab body. `banks` are the person's own banks (not used yet; kept for "add a bank").
pub(super) fn body_of(ui: &mut Ui, m: &mut CirclesModel) -> Outcome {
    let out = body_inner(ui, m);
    forget_cleared_fields(ui.ctx(), m);
    out
}

/// The text fields that hold an address or an invitation code. Their egui edit state (with its undo
/// history) is dropped once the field is empty, so Ctrl+Z can not bring a cleared value back (critic 57 I3,
/// as `ui.rs` does for the password fields).
const SECRET_FIELDS: [&str; 2] = ["circle_email_address", "circle_code_text"];

/// Drops the edit state of an emptied (or closed) address or code field. Called every frame the Shared page
/// is drawn, whichever tab shows.
pub(super) fn forget_cleared_fields(ctx: &egui::Context, m: &CirclesModel) {
    for id in SECRET_FIELDS {
        let empty = match id {
            "circle_email_address" => !matches!(&m.panel, Panel::Email(f) if !f.address.is_empty()),
            _ => m.join.text.is_empty(),
        };
        if empty {
            ctx.data_mut(|d| d.remove::<egui::text_edit::TextEditState>(egui::Id::new(id)));
        }
    }
}

fn body_inner(ui: &mut Ui, m: &mut CirclesModel) -> Outcome {
    let mut out = Outcome::None;
    if let Some(n) = m.notice.clone() {
        banner(ui, &n);
    }
    if super::circle_edit_ui::draw(ui, m) {
        return out;
    }
    match m.screen {
        Screen::Gate => gate(ui, m),
        Screen::List => list_page(ui, m),
        Screen::Circle => circle_page(ui, m, &mut out),
        Screen::Bank => bank_page(ui, m, &mut out),
        Screen::Join => join_page(ui, m),
        Screen::Create => create_page(ui, m),
    }
    out
}

fn gate(ui: &mut Ui, m: &mut CirclesModel) {
    // The Shared page shows the guided steps before the tabs; this only appears for the moment the
    // account is being looked up again, or after a sign-in ended.
    match m.gate.clone() {
        super::publish::Gate::Checking => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(theme::muted("Checking your account..."));
            });
        }
        _ => {
            body(ui, "Your sign-in ended or needs a look. The steps are at the top of this page.");
            if kit::primary_button(ui, "Open my sharing settings").clicked() {
                m.request_open_account();
            }
        }
    }
}

// ---- the list ------------------------------------------------------------------------------------------------------

fn list_page(ui: &mut Ui, m: &mut CirclesModel) {
    let (mut create, mut join) = (false, false);
    ui.horizontal_wrapped(|ui| {
        create = kit::primary_button(ui, "Create a circle...").clicked();
        join = kit::ghost_button(ui, "Enter a code...").clicked();
        if m.busy() {
            ui.spinner();
        }
    });
    ui.add_space(6.0);
    if create {
        m.start_create();
        return;
    }
    if join {
        m.start_join();
        return;
    }
    let (mut retry, mut account) = (false, false);
    if let Some(p) = m.list.problem.clone() {
        let w = wait_of(m, &p);
        problem_box(ui, &p, w, &mut retry, &mut account);
    }
    if retry {
        m.retry_list();
    }
    if account {
        m.request_open_account();
    }
    if !m.list.loaded {
        if m.list.problem.is_none() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(theme::muted("Loading your circles..."));
            });
        }
        return;
    }
    if m.list.items.is_empty() {
        ui.label(theme::strong(text::EMPTY_LIST_TITLE).size(14.5));
        para(ui, text::EMPTY_LIST_HELP);
        return;
    }
    para(ui, "A circle is a small private group of teachers who share banks with each other. Only its members can see what is in it.");
    let now = m.now_ms();
    let mut open: Option<String> = None;
    for c in m.list.items.clone() {
        kit::card().inner_margin(Margin::same(10)).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let clicked = with_action(ui, Some(("Open", true)), |ui| {
                text_row(ui, |ui| {
                    ui.add(egui::Label::new(theme::strong(api::plain(&c.name)).size(14.5)).wrap());
                    role_pill(ui, c.role());
                    if c.closed() {
                        pill(ui, "Closed", theme::error_wash(), theme::bad());
                    }
                    if c.ownership_offered_until.is_some_and(|u| u > now) {
                        pill(ui, "You were offered ownership", theme::accent_col(), theme::on_accent_col());
                    }
                });
                ui.add_space(2.0);
                line(ui, format!("{}, {}", text::members_text(c.members), text::banks_text(c.banks)));
                if c.closed() {
                    let who = if c.closed_by_owner() { "The owner closed it." } else { "It was closed by an administrator, or because nobody was left." };
                    line(ui, match c.undo_until {
                        Some(u) if u > now => format!("{who} Everyone loses access {}.", text::until(u, now)),
                        _ => format!("{who} It will be deleted soon."),
                    });
                }
            });
            if clicked {
                open = Some(c.id.clone());
            }
        });
        ui.add_space(4.0);
    }
    if let Some(id) = open {
        m.open_circle(&id);
    }
}

// ---- create and join ------------------------------------------------------------------------------------------

fn create_page(ui: &mut Ui, m: &mut CirclesModel) {
    if back_link(ui, "< All circles") {
        m.go_list();
        return;
    }
    ui.add_space(4.0);
    ui.label(theme::serif("Create a circle").size(20.0));
    para(ui, "A circle is a small private group of teachers who share banks with each other. You will be its owner and can invite other teachers with a code.");
    ui.label(theme::strong("Name").size(12.5));
    ui.add(egui::TextEdit::singleline(&mut m.create.name).id(egui::Id::new("circle_name")).hint_text("e.g. Algebra team").desired_width(f32::INFINITY));
    ui.add_space(4.0);
    ui.label(theme::strong("What it is for (optional)").size(12.5));
    ui.add(egui::TextEdit::multiline(&mut m.create.description).id(egui::Id::new("circle_description")).desired_rows(3).desired_width(f32::INFINITY));
    ui.add_space(6.0);
    let blocker = m.create_blocker();
    if let Some(b) = blocker
        && !m.create.name.trim().is_empty()
    {
        para(ui, b);
    }
    let (mut retry, mut account) = (false, false);
    if let Some(p) = m.create.problem.clone() {
        let w = wait_of(m, &p);
        problem_box(ui, &p, w, &mut retry, &mut account);
    }
    if account {
        m.request_open_account();
    }
    let mut go = retry;
    ui.horizontal(|ui| {
        ui.add_enabled_ui(blocker.is_none() && !m.create.working, |ui| go |= kit::primary_button(ui, "Create the circle").clicked());
        if kit::ghost_button(ui, "Cancel").clicked() {
            m.go_list();
        }
        if m.create.working {
            ui.spinner();
        }
    });
    if go {
        m.create_circle();
    }
}

fn join_page(ui: &mut Ui, m: &mut CirclesModel) {
    if back_link(ui, "< All circles") {
        m.go_list();
        return;
    }
    ui.add_space(4.0);
    ui.label(theme::serif("Join a circle with a code").size(20.0));
    let mut lookup = false;
    let mut change = false;
    let mut open = false;
    let mut join = false;
    let mut retry = false;
    let mut account = false;
    let working = matches!(m.join.stage, JoinStage::Looking | JoinStage::Joining);
    match &m.join.stage {
        JoinStage::Typing | JoinStage::Looking | JoinStage::Failed(_) => {
            para(ui, text::CODE_PASTE_HELP);
            ui.add_enabled_ui(!working, |ui| {
                ui.add(egui::TextEdit::multiline(&mut m.join.text).id(egui::Id::new("circle_code_text")).hint_text("C-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX").desired_rows(2).desired_width(f32::INFINITY));
            });
            let found = m.join_code();
            if let Some(c) = &found {
                line(ui, format!("Found a code: {}", code_groups(c)));
            } else if !m.join.text.trim().is_empty() {
                line(ui, "No circle code found yet. A code starts with C- and has 24 letters and digits.");
            }
            if let JoinStage::Failed(p) = &m.join.stage {
                let p = p.clone();
                let w = wait_of(m, &p);
                problem_box(ui, &p, w, &mut retry, &mut account);
            }
            ui.horizontal(|ui| {
                ui.add_enabled_ui(found.is_some() && !working, |ui| lookup = kit::primary_button(ui, "Look up").clicked() || retry);
                if working {
                    ui.spinner();
                }
            });
        }
        JoinStage::Preview(p, _) => {
            let role = p.role();
            let name = api::plain(&p.circle.name);
            let owner = btext::author(&p.circle.owner_name);
            if p.already_member {
                body(ui, &format!("You are already in \"{name}\"."));
            } else {
                body(ui, &format!("Join \"{name}\" as {}?", role_phrase(role)));
            }
            line(ui, format!("Owner: {owner}. {}.", text::members_text(p.circle.members)));
            if !p.already_member {
                para(ui, "Nothing is used up until you press Join.");
            }
            ui.horizontal(|ui| {
                join = kit::primary_button(ui, if p.already_member { "Open it" } else { "Join" }).clicked();
                change = kit::ghost_button(ui, "Use another code").clicked();
            });
        }
        JoinStage::Joining => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(theme::muted("Joining..."));
            });
        }
        JoinStage::Done { name, role, already, .. } => {
            let n = if name.is_empty() { "the circle".to_string() } else { format!("\"{name}\"") };
            body(ui, &if *already { format!("You were already in {n}.") } else { format!("You joined {n} as {}.", role_phrase(*role)) });
            ui.add_space(6.0);
            open = kit::primary_button(ui, "Open the circle").clicked();
        }
    }
    if account {
        m.request_open_account();
    }
    if lookup {
        m.look_up_code();
    }
    if join {
        let already = matches!(&m.join.stage, JoinStage::Preview(p, _) if p.already_member);
        if already {
            if let JoinStage::Preview(p, _) = &m.join.stage {
                let id = p.circle.id.clone();
                m.open_circle(&id);
            }
        } else {
            m.confirm_join();
        }
    }
    if change {
        m.change_code();
    }
    if open {
        m.open_joined();
    }
}

fn role_phrase(r: Role) -> &'static str {
    match r {
        Role::Owner => "the owner",
        Role::Editor => "an editor (can add banks)",
        Role::Viewer => "a viewer (can look at and copy banks)",
    }
}

// ---- one circle ---------------------------------------------------------------------------------------------------------

fn circle_page(ui: &mut Ui, m: &mut CirclesModel, out: &mut Outcome) {
    if back_link(ui, "< All circles") {
        m.back();
        return;
    }
    ui.add_space(4.0);
    let Some(c) = m.circle.as_ref() else { return };
    let (mut retry, mut account) = (false, false);
    if let Some(p) = c.problem.clone() {
        let w = wait_of(m, &p);
        problem_box(ui, &p, w, &mut retry, &mut account);
        if retry {
            m.retry_circle();
        }
        if account {
            m.request_open_account();
        }
        return;
    }
    let Some(c) = m.circle.as_ref() else { return };
    let Some(d) = c.detail.clone() else {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(theme::muted("Loading the circle..."));
        });
        return;
    };
    let now = m.now_ms();
    let role = d.circle.role();
    let closed = d.circle.closed();

    ui.add(egui::Label::new(theme::serif(api::plain(&d.circle.name)).size(22.0)).wrap());
    ui.add_space(2.0);
    text_row(ui, |ui| {
        role_pill(ui, role);
        line(ui, format!("{}, {}", text::members_text(d.members.len() as u32), text::banks_text(d.banks.iter().filter(|b| b.removed.is_none()).count() as u32)));
    });
    let desc = api::plain(&d.circle.description);
    if !desc.is_empty() {
        ui.add(egui::Label::new(theme::text(desc).size(13.0)).wrap());
    }
    ui.add_space(6.0);
    if closed {
        let when = match d.circle.undo_until {
            Some(u) if u > now => format!("Everyone loses access {}.", text::until(u, now)),
            _ => "It will be deleted soon.".to_string(),
        };
        let who = if d.circle.closed_by_owner() { "The owner closed this circle." } else { "This circle was closed by an administrator, or because nobody was left in it. It can not be reopened here." };
        warn_box(ui, &format!("{who} {when} Copies on computers stay."));
        if m.allowed(Action::Reopen) {
            if kit::primary_button(ui, "Reopen the circle").clicked() {
                m.reopen();
            }
            ui.add_space(8.0);
        }
    }
    offer_banner(ui, m, now);
    if let Some(p) = m.circle.as_ref().and_then(|c| c.act_problem.clone()) {
        let w = wait_of(m, &p);
        let (mut r, mut a) = (false, false);
        problem_box(ui, &p, w, &mut r, &mut a);
        if a {
            m.request_open_account();
        }
        if r {
            m.retry_act();
        }
    }
    panels(ui, m);

    // Banks.
    let mut open_bank: Option<String> = None;
    let mut restore: Option<String> = None;
    let mut ask_purge: Option<String> = None;
    let mut purge: Option<String> = None;
    let mut cancel_purge = false;
    let confirming_purge = m.circle.as_ref().and_then(|c| c.confirm_purge.clone());
    let mut add = false;
    let badges: std::collections::HashMap<String, String> = d.banks.iter().filter_map(|b| super::circle_edit_ui::bank_badge(m, &d.circle.id, &b.id).map(|t| (b.id.clone(), t))).collect();
    section(ui, "Banks", |ui| {
        if m.allowed(Action::AddBank) {
            add = kit::primary_button(ui, "Add a bank...").clicked();
            ui.add_space(4.0);
        }
        if d.banks.is_empty() {
            ui.label(theme::strong(text::EMPTY_BANKS).size(13.5));
            para(ui, if role.can_add_banks() { text::EMPTY_BANKS_HELP_EDITOR } else { text::EMPTY_BANKS_HELP_VIEWER });
        }
        for b in &d.banks {
            kit::card().inner_margin(Margin::same(8)).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                let now_ms = m.now_ms();
                // (round 48: after a partial "Delete for good now" nothing can be brought back or deleted again from here)
                let claimed = m.purge_claimed.get(&b.id).copied();
                let can_restore = claimed.is_none() && bank_can(role, closed, b, BankAction::Restore) && b.removed.as_ref().and_then(|r| r.undo_until).is_some_and(|u| u > now_ms);
                let can_purge = claimed.is_none() && bank_can(role, closed, b, BankAction::Purge);
                let action = match &b.removed {
                    Some(_) if can_restore => Some(("Bring it back", false)),
                    Some(_) => None,
                    None => Some(("Open", true)),
                };
                let clicked = with_action(ui, action, |ui| {
                    if b.hidden {
                        ui.add(egui::Label::new(theme::strong(text::HIDDEN_BANK_TITLE).size(13.5)).wrap());
                        line(ui, text::HIDDEN_BANK_LINE.to_string());
                        line(ui, format!("version {} - {}", b.head_version, text::questions_text(b.question_count)));
                        if let Some(n) = claimed {
                            line(ui, text::purge_pending_line(n));
                        } else if let Some(r) = &b.removed {
                            line(ui, format!("Removed from the circle. {}", text::undo_line(r.undo_until, now)));
                        }
                        return;
                    }
                    ui.add(egui::Label::new(theme::strong(btext_shown(&b.title)).size(13.5)).wrap());
                    let mut bits = vec![format!("by {}", btext::author(&b.author_name)), format!("version {}", b.head_version), text::questions_text(b.question_count)];
                    let ago = text::ago(b.updated_at, now);
                    if !ago.is_empty() {
                        bits.push(format!("updated {ago}"));
                    }
                    line(ui, bits.join(" - "));
                    line(ui, format!("{}. {}", license_name(&b.license), text::reshare_line(&b.license)));
                    if let Some(bd) = badges.get(&b.id) {
                        ui.add(egui::Label::new(theme::accent(bd).size(12.5)).wrap());
                    }
                    if b.hide_key && b.removed.is_none() {
                        line(ui, if role.can_add_banks() { "Answer keys hidden from viewers." } else { "Answer keys hidden for viewers: you get it without them." });
                    }
                    if b.is_held() {
                        ui.add(egui::Label::new(theme::accent(text::HELD_BANK).size(12.5)).wrap());
                    }
                    if let Some(n) = claimed {
                        line(ui, text::purge_pending_line(n));
                    } else if let Some(r) = &b.removed {
                        line(ui, format!("Removed from the circle. {}", text::undo_line(r.undo_until, now)));
                    }
                });
                if clicked {
                    if b.removed.is_some() {
                        restore = Some(b.id.clone());
                    } else {
                        open_bank = Some(b.id.clone());
                    }
                }
                if can_purge {
                    if confirming_purge.as_deref() == Some(b.id.as_str()) {
                        body(ui, text::PURGE_BANK);
                        ui.horizontal_wrapped(|ui| {
                            if kit::danger_text_button(ui, "Delete it for good").clicked() {
                                purge = Some(b.id.clone());
                            }
                            if kit::ghost_button(ui, "Keep it").clicked() {
                                cancel_purge = true;
                            }
                        });
                    } else if kit::danger_text_button(ui, "Delete for good now...").clicked() {
                        ask_purge = Some(b.id.clone());
                    }
                }
            });
            ui.add_space(3.0);
        }
    });
    if add {
        *out = Outcome::ShareToCircle(d.circle.id.clone());
    }
    if let Some(b) = restore {
        m.restore_bank(&b);
    }
    if let Some(b) = ask_purge {
        m.ask_purge(&b);
    }
    if let Some(b) = purge {
        m.purge_bank(&b);
    }
    if cancel_purge {
        m.cancel_purge();
    }
    if let Some(b) = open_bank {
        m.open_bank(&b);
        return;
    }

    super::circle_edit_ui::working_copies(ui, m, &d.circle.id);
    members_section(ui, m, &d);
    if role == Role::Owner && !closed {
        codes_section(ui, m);
    }
    activity_section(ui, m);

    // Leave and close.
    section(ui, "This circle", |ui| {
        ui.horizontal_wrapped(|ui| {
            if m.allowed(Action::Rename) && kit::ghost_button(ui, "Rename...").clicked() {
                m.begin_rename();
            }
            if kit::ghost_button(ui, "Leave this circle...").clicked() {
                m.ask_leave();
            }
            if m.allowed(Action::Close) && kit::danger_text_button(ui, "Close this circle...").clicked() {
                m.ask_close();
            }
        });
        if role == Role::Owner {
            para(ui, "As the owner you can rename the circle, make codes, change roles, remove members, hand the circle to someone else, or close it.");
        }
    });
}

fn btext_shown(s: &str) -> String {
    super::publish_text::shown(s)
}

fn panels(ui: &mut Ui, m: &mut CirclesModel) {
    let busy = m.busy_action();
    enum Do {
        None,
        Cancel,
        MakeCode,
        DoneCode,
        Leave,
        Close,
        Remove,
        Transfer,
        Rename,
        SendEmail,
        AnotherEmail,
    }
    let mut d = Do::None;
    let email_blocker = m.email_blocker();
    let now_i = m.now_instant();
    let mut account = false;
    let kind = std::mem::discriminant(&m.panel);
    let _ = kind;
    let names: (String, String) = match &m.panel {
        Panel::ConfirmRemove(u) | Panel::ConfirmTransfer(u) => (m.member_name(u), String::new()),
        _ => (String::new(), String::new()),
    };
    let owner_leaving = m.my_role_public() == Role::Owner;
    match &mut m.panel {
        Panel::None => {}
        Panel::NewCode(f) => {
            section(ui, "Make a code", |ui| {
                ui.label(theme::strong("What can they do?").size(12.5));
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(&mut f.role, Role::Editor, "Can add banks");
                    ui.selectable_value(&mut f.role, Role::Viewer, "Can only look and copy");
                });
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(theme::strong("For how many people?").size(12.5));
                    ui.add(egui::DragValue::new(&mut f.people).range(1..=50).speed(0.2));
                    ui.label(theme::strong("Works for how many days?").size(12.5));
                    ui.add(egui::DragValue::new(&mut f.days).range(1..=30).speed(0.2));
                });
                ui.add_space(4.0);
                ui.label(theme::strong("A note for yourself (optional)").size(12.5));
                ui.add(egui::TextEdit::singleline(&mut f.note).id(egui::Id::new("circle_code_note")).hint_text("e.g. for Ms. Rivera").desired_width(f32::INFINITY));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(!busy, |ui| {
                        if kit::primary_button(ui, "Make the code").clicked() {
                            d = Do::MakeCode;
                        }
                    });
                    if kit::ghost_button(ui, "Cancel").clicked() {
                        d = Do::Cancel;
                    }
                    if busy {
                        ui.spinner();
                    }
                });
            });
        }
        Panel::Email(f) => {
            section(ui, "Invite a teacher by email", |ui| {
                if let Some(sent) = &f.sent {
                    let label = api::plain(&sent.address_label);
                    let who = if label.is_empty() { "the address".to_string() } else { label };
                    body(ui, &format!("Invitation sent to {who}, as {}.", role_phrase(sent.role())));
                    if sent.resent {
                        para(ui, text::EMAIL_RESENT);
                    }
                    if let Some(x) = sent.expires_at.filter(|x| *x > 0) {
                        // (the date, as the mail gives it: "expires in 6 days" reads wrong for a 7-day code)
                        line(ui, format!("The code in it works once, until {} (UTC).", api::date_text(x)));
                    }
                    line(ui, text::EMAIL_NEEDS_ACCOUNT);
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        if kit::primary_button(ui, "Invite someone else").clicked() {
                            d = Do::AnotherEmail;
                        }
                        if kit::ghost_button(ui, "Done").clicked() {
                            d = Do::Cancel;
                        }
                    });
                    return;
                }
                para(ui, text::EMAIL_HELP);
                ui.label(theme::strong("Their email address").size(12.5));
                ui.add_enabled_ui(!f.working, |ui| {
                    ui.add(egui::TextEdit::singleline(&mut f.address).id(egui::Id::new("circle_email_address")).hint_text("name@school.org").desired_width(f32::INFINITY));
                });
                ui.add_space(4.0);
                ui.label(theme::strong("What can they do?").size(12.5));
                ui.add_enabled_ui(!f.working, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.selectable_value(&mut f.role, Role::Editor, "Can add banks");
                        ui.selectable_value(&mut f.role, Role::Viewer, "Can only look and copy");
                    });
                });
                ui.add_space(4.0);
                para(ui, text::EMAIL_NEEDS_ACCOUNT);
                line(ui, text::EMAIL_PRIVACY);
                line(ui, text::EMAIL_PACE);
                ui.add_space(4.0);
                // (the form's own button is the way to try again: no second "Try again" next to it; whether it waits
                // is the model's `email_blocker`, which holds a per-domain wait to that domain only)
                if let Some(p) = &f.problem {
                    warn_box(ui, &p.message);
                    if let Some(w) = p.retry_at.filter(|at| *at > now_i).map(|at| at - now_i) {
                        let mins = w.as_secs().div_ceil(60);
                        let when = if mins <= 1 { "in about a minute".to_string() } else { format!("in about {mins} minutes") };
                        line(ui, if f.problem_domain.is_some() { format!("Addresses at that mail domain can be invited again {when}.") } else { format!("You can send it again {when}.") });
                    }
                    if p.account && kit::ghost_button(ui, "Open account...").clicked() {
                        account = true;
                    }
                }
                if let Some(b) = email_blocker.filter(|_| !f.address.trim().is_empty() && !f.working) {
                    line(ui, b);
                }
                ui.horizontal_wrapped(|ui| {
                    ui.add_enabled_ui(email_blocker.is_none() && !busy, |ui| {
                        if kit::primary_button(ui, "Send the invitation").clicked() {
                            d = Do::SendEmail;
                        }
                    });
                    if ui.add_enabled(!f.working, |ui: &mut Ui| kit::ghost_button(ui, "Cancel")).clicked() {
                        d = Do::Cancel;
                    }
                    if f.working {
                        ui.spinner();
                        ui.label(theme::muted("Sending the invitation..."));
                    }
                });
            });
        }
        Panel::Code(c) => {
            section(ui, "Your new code", |ui| {
                let shown = code_groups(c.code.expose());
                ui.label(egui::RichText::new(&shown).size(18.0).monospace().color(theme::ink()));
                ui.add_space(4.0);
                ui.add(egui::Label::new(theme::accent(text::CODE_SHOWN_ONCE).size(12.5)).wrap());
                line(ui, format!("For {} {}. It expires {}.", c.max_uses, if c.max_uses == 1 { "person" } else { "people" }, text::until(c.expires_at, 0.max(super::browse::now_ms()))));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if kit::primary_button(ui, "Copy the code").clicked() {
                        ui.ctx().copy_text(c.code.expose().to_string());
                    }
                    if kit::ghost_button(ui, "I have copied it").clicked() {
                        d = Do::DoneCode;
                    }
                });
            });
        }
        Panel::ConfirmLeave => {
            section(ui, "Leave this circle?", |ui| {
                body(ui, if owner_leaving { text::LEAVE_OWNER } else { text::LEAVE_MEMBER });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(!busy, |ui| {
                        if kit::primary_button(ui, "Leave").clicked() {
                            d = Do::Leave;
                        }
                    });
                    if kit::ghost_button(ui, "Stay").clicked() {
                        d = Do::Cancel;
                    }
                });
            });
        }
        Panel::ConfirmClose => {
            section(ui, "Close this circle?", |ui| {
                body(ui, text::CLOSE_CIRCLE);
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(!busy, |ui| {
                        if kit::primary_button(ui, "Close the circle").clicked() {
                            d = Do::Close;
                        }
                    });
                    if kit::ghost_button(ui, "Keep it open").clicked() {
                        d = Do::Cancel;
                    }
                });
            });
        }
        Panel::ConfirmRemove(_) => {
            section(ui, &format!("Remove {} from the circle?", names.0), |ui| {
                body(ui, "They lose access at once. Copies they made stay on their computer. They can only come back with a code you make after this.");
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(!busy, |ui| {
                        if kit::primary_button(ui, "Remove").clicked() {
                            d = Do::Remove;
                        }
                    });
                    if kit::ghost_button(ui, "Cancel").clicked() {
                        d = Do::Cancel;
                    }
                });
            });
        }
        Panel::ConfirmTransfer(_) => {
            section(ui, &format!("Offer the circle to {}?", names.0), |ui| {
                body(ui, text::TRANSFER_NOTE);
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(!busy, |ui| {
                        if kit::primary_button(ui, "Send the offer").clicked() {
                            d = Do::Transfer;
                        }
                    });
                    if kit::ghost_button(ui, "Cancel").clicked() {
                        d = Do::Cancel;
                    }
                });
            });
        }
        Panel::Rename { name } => {
            section(ui, "Rename the circle", |ui| {
                ui.add(egui::TextEdit::singleline(name).id(egui::Id::new("circle_rename")).desired_width(f32::INFINITY));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(!busy && !name.trim().is_empty(), |ui| {
                        if kit::primary_button(ui, "Save").clicked() {
                            d = Do::Rename;
                        }
                    });
                    if kit::ghost_button(ui, "Cancel").clicked() {
                        d = Do::Cancel;
                    }
                });
            });
        }
    }
    match d {
        Do::None => {}
        Do::Cancel => m.cancel_panel(),
        Do::MakeCode => m.make_code(),
        Do::DoneCode => m.done_with_code(),
        Do::Leave => m.leave(),
        Do::Close => m.close_the_circle(),
        Do::Remove => m.remove_member(),
        Do::Transfer => m.offer_ownership(),
        Do::Rename => m.rename(),
        Do::SendEmail => m.send_email_invite(),
        Do::AnotherEmail => m.another_email(),
    }
    if account {
        m.request_open_account();
    }
}

fn members_section(ui: &mut Ui, m: &mut CirclesModel, d: &super::circles_api::CircleDetail) {
    let mine = d.circle.role();
    let closed = d.circle.closed();
    let mut remove: Option<String> = None;
    let mut transfer: Option<String> = None;
    let mut set: Option<(String, Role)> = None;
    section(ui, "Members", |ui| {
        for (i, mem) in d.members.iter().enumerate() {
            if i > 0 {
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);
            }
            text_row(ui, |ui| {
                let name = btext::author(&mem.display_name);
                ui.add(egui::Label::new(theme::strong(if mem.is_me { format!("{name} (you)") } else { name }).size(13.0)).wrap());
                role_pill(ui, mem.role());
            });
            let joined = btext::date(mem.joined_at);
            line(ui, if joined.is_empty() { capitalised(mem.role().can()) } else { format!("Joined {joined} - {}", mem.role().can()) });
            if can(mine, closed, Action::ChangeRole) && !mem.is_me && mem.role() != Role::Owner {
                ui.add_space(2.0);
                ui.horizontal_wrapped(|ui| {
                    let (label, to) = if mem.role() == Role::Editor { ("Make viewer", Role::Viewer) } else { ("Make editor", Role::Editor) };
                    if kit::ghost_button(ui, label).clicked() {
                        set = Some((mem.user_id.clone(), to));
                    }
                    if kit::ghost_button(ui, "Offer ownership...").clicked() {
                        transfer = Some(mem.user_id.clone());
                    }
                    if kit::danger_text_button(ui, "Remove...").clicked() {
                        remove = Some(mem.user_id.clone());
                    }
                });
            }
        }
    });
    if let Some(u) = remove {
        m.ask_remove(&u);
    }
    if let Some(u) = transfer {
        m.ask_transfer(&u);
    }
    if let Some((u, r)) = set {
        m.set_role(&u, r);
    }
}

/// One code or emailed invitation, without its state (shown first, in the text colour):
/// "code - can add banks - 0 of 1 used - ..." or "emailed to j***@school.org - can add banks - sent 2 hours ago - ...".
pub(super) fn invite_line(i: &Invite, now: i64) -> String {
    let mut bits = Vec::new();
    if i.emailed() {
        let l = api::plain(&i.address_label);
        bits.push(if l.is_empty() { "emailed".to_string() } else { format!("emailed to {l}") });
    } else {
        bits.push("code".to_string());
    }
    bits.push(text::role_choice_name(i.role()).to_lowercase());
    if !i.emailed() {
        bits.push(format!("{} of {} used", i.uses, i.max_uses));
    }
    let ago = text::ago(i.created_at, now);
    if !ago.is_empty() {
        bits.push(format!("{} {ago}", if i.emailed() { "sent" } else { "made" }));
    }
    if i.active() {
        bits.push(format!("expires {}", text::until(i.expires_at, now)));
    }
    let by = api::plain(&i.created_by_name);
    if !by.is_empty() {
        bits.push(format!("made by {by}"));
    }
    if let Some(n) = i.note.as_deref().map(api::plain).filter(|n| !n.is_empty()) {
        bits.push(format!("note: {n}"));
    }
    bits.join(" - ")
}

fn codes_section(ui: &mut Ui, m: &mut CirclesModel) {
    let now = m.now_ms();
    let Some(c) = m.circle.as_ref() else { return };
    let invites = c.invites.clone();
    let problem = c.invites_problem.clone();
    let more = c.invites_next.is_some();
    let mut make = false;
    let mut email = false;
    let mut withdraw: Option<String> = None;
    let mut more_clicked = false;
    section(ui, "Codes and invitations", |ui| {
        ui.horizontal_wrapped(|ui| {
            make = kit::primary_button(ui, "Make a code...").clicked();
            email = kit::ghost_button(ui, "Invite by email...").clicked();
        });
        ui.add_space(4.0);
        if let Some(p) = &problem {
            warn_box(ui, &p.message);
        }
        match &invites {
            None if problem.is_none() => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(theme::muted("Loading codes..."));
                });
            }
            None => {}
            Some(l) if l.is_empty() => para(ui, text::NO_CODES),
            Some(l) => {
                para(ui, "The codes themselves are never shown again, and the addresses of emailed invitations are never kept. Withdraw one to stop it working.");
                for i in l {
                    text_row(ui, |ui| {
                        two_tone(ui, text::state_label(&i.state), " - ", &invite_line(i, now));
                        if i.active() && kit::danger_text_button(ui, "Withdraw").clicked() {
                            withdraw = Some(i.invite_id.clone());
                        }
                    });
                    ui.add_space(2.0);
                }
                if more {
                    more_clicked = link(ui, "Show more codes");
                }
            }
        }
    });
    if make {
        m.begin_code();
    }
    if email {
        m.begin_email();
    }
    if let Some(i) = withdraw {
        m.withdraw_code(&i);
    }
    if more_clicked {
        m.more_invites();
    }
}

fn activity_section(ui: &mut Ui, m: &mut CirclesModel) {
    let now = m.now_ms();
    let Some(c) = m.circle.as_ref() else { return };
    let (entries, loaded, problem, more) = (c.activity.clone(), c.activity_loaded, c.activity_problem.clone(), c.activity_next.is_some());
    let mut more_clicked = false;
    section(ui, "What happened lately", |ui| {
        if let Some(p) = &problem {
            warn_box(ui, &p.message);
        }
        if !loaded && problem.is_none() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(theme::muted("Loading..."));
            });
        } else if loaded && entries.is_empty() {
            para(ui, text::EMPTY_ACTIVITY);
        }
        for e in &entries {
            two_tone(ui, &text::activity_line(e), "   ", &text::ago(e.at, now));
            ui.add_space(2.0);
        }
        if more {
            more_clicked = link(ui, "Show older");
        }
    });
    if more_clicked {
        m.more_activity();
    }
}

// ---- one bank -------------------------------------------------------------------------------------------------------

fn bank_page(ui: &mut Ui, m: &mut CirclesModel, out: &mut Outcome) {
    let circle_name = m.circle.as_ref().and_then(|c| c.detail.as_ref()).map(|d| api::plain(&d.circle.name)).unwrap_or_else(|| "the circle".into());
    if back_link(ui, &format!("< {circle_name}")) {
        m.back();
        return;
    }
    ui.add_space(4.0);
    let now = m.now_ms();
    let (mut retry, mut account) = (false, false);
    let Some((bank_cid, bank_bid)) = m.bank.as_ref().map(|b| (b.cid.clone(), b.bid.clone())) else { return };
    let Some(b) = m.bank.as_ref() else { return };
    match &b.load {
        BankLoad::Loading => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(theme::muted("Opening the bank..."));
            });
            return;
        }
        BankLoad::Failed(p) => {
            if m.hide_off_offered() {
                // (critic 58 B2) Hiding the keys held the newest version back: the switch stays, with the way out.
                warn_box(ui, text::HIDE_HELD_HEAD);
                ui.add_space(4.0);
                hide_section(ui, m);
            } else {
                let p = p.clone();
                let w = wait_of(m, &p);
                problem_box(ui, &p, w, &mut retry, &mut account);
                // (critic 59 B1) Held back for another reason: the switch stays, both ways, and nothing says
                // turning hiding off would help.
                if m.hide_switch_on_held_head() {
                    ui.add_space(4.0);
                    hide_section(ui, m);
                }
            }
            if retry {
                m.retry_bank();
            }
            if account {
                m.request_open_account();
            }
            let (cid, bid) = (bank_cid.clone(), bank_bid.clone());
            let title = m.circle.as_ref().and_then(|c| c.detail.as_ref()).and_then(|d| d.banks.iter().find(|x| x.id == bid).map(|x| x.title.clone())).unwrap_or_default();
            super::circle_edit_ui::bank_section(ui, m, &cid, &bid, &api::plain(&title));
            return;
        }
        BankLoad::Ready(_) => {}
    }
    let Some(BankView { load: BankLoad::Ready(l), .. }) = m.bank.as_ref() else { return };
    let head = l.doc.bank.clone();
    let questions = l.view.clone();
    let origins_note = l.doc.origins.as_array().is_some_and(|a| !a.is_empty());

    ui.add(egui::Label::new(theme::serif(btext_shown(&head.title)).size(20.0)).wrap());
    let mut bits = vec![format!("by {}", btext::author(&head.author_name)), format!("version {}", head.version), text::questions_text(head.question_count)];
    let ago = text::ago(head.committed_at, now);
    if !ago.is_empty() {
        bits.push(format!("sent {ago}"));
    }
    for p in [api::plain(&head.subject), api::plain(&head.course), text::grade_text(&head.grade)] {
        if !p.is_empty() {
            bits.push(p);
        }
    }
    line(ui, bits.join(" - "));
    ui.add_space(4.0);
    if head.keys_hidden {
        // A viewer of a bank that hides its keys: said plainly, before anything else.
        keys_banner(ui, text::KEYS_HIDDEN_VIEWER);
    } else if head.hide_key {
        para(ui, text::ANSWER_KEYS_HIDDEN_NOTE);
    } else {
        para(ui, text::ANSWER_KEYS_NOTE);
    }
    if head.is_author && !matches!(head.held, serde_json::Value::Null | serde_json::Value::Bool(false)) {
        warn_box(ui, text::HELD_BANK);
    }
    if origins_note {
        para(ui, "Some questions in this bank were built on items from the public pool; their credits are kept with it.");
    }

    // The actions.
    let can_copy = m.bank_allowed(BankAction::Copy);
    // A second working copy would take the link from the first one: while this computer has a
    // working copy of the bank (still in the banks), only a plain copy is offered here.
    let linked = m.edit.link_for_bank(&bank_cid, &bank_bid).map(|l| !m.edit.status(&l.local_bank_id).is_some_and(|s| s.missing));
    let has_working = linked == Some(true);
    let can_work = m.can_make_working_copy() && !has_working;
    let (mut begin_copy, mut get_copy, mut cancel_copy, mut go_imported, mut work_copy) = (false, false, false, false, false);
    section(ui, "Get a copy", |ui| {
        let Some(b) = m.bank.as_mut() else { return };
        match &mut b.copy {
            CopyPanel::Closed => {
                para(ui, if can_work { text::WORKING_COPY_LINE } else { text::COPY_LINE });
                if head.keys_hidden {
                    para(ui, text::COPY_NO_KEYS);
                }
                if has_working {
                    line(ui, text::HAS_WORKING_COPY);
                }
                ui.add_enabled_ui(can_copy, |ui| begin_copy = kit::primary_button(ui, if can_work { "Make a working copy..." } else { "Get a copy..." }).clicked());
            }
            CopyPanel::Choosing { name } => {
                para(ui, if can_work { text::WORKING_COPY_LINE } else { text::COPY_LINE });
                if head.keys_hidden {
                    para(ui, text::COPY_NO_KEYS);
                }
                if has_working {
                    line(ui, text::HAS_WORKING_COPY);
                }
                ui.label(theme::strong("Name of your copy").size(12.5));
                ui.add(egui::TextEdit::singleline(name).id(egui::Id::new("circle_copy_name")).desired_width(f32::INFINITY));
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    if can_work {
                        work_copy = kit::primary_button(ui, "Make the working copy").clicked();
                        get_copy = kit::ghost_button(ui, "Get a plain copy instead").clicked();
                    } else {
                        get_copy = kit::primary_button(ui, "Get the copy").clicked();
                    }
                    cancel_copy = kit::ghost_button(ui, "Cancel").clicked();
                });
            }
            CopyPanel::Working => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(theme::muted("Making your copy..."));
                });
            }
            CopyPanel::Done(done) => {
                body(ui, &format!("Done. \"{}\" is now in your banks with {}.", api::plain(&done.bank_name), text::questions_text(done.added as u32)));
                if done.pictures_missing > 0 {
                    para(ui, &format!("{} of {} pictures could not be fetched; those questions are shown without them.", done.pictures_missing, done.pictures));
                }
                if head.keys_hidden {
                    para(ui, text::COPY_NO_KEYS_DONE);
                }
                para(ui, "It also appears under \"Things I imported\", where verSHion tells you if the circle sends a newer version. Your copy is never changed for you.");
                // (a copy of the viewers' form is never the working copy, whatever this computer holds)
                if linked.is_some() && !head.keys_hidden {
                    line(ui, text::WORKING_COPY_DONE);
                }
                go_imported = link(ui, "See it under Things I imported");
            }
            CopyPanel::Failed(p) => {
                warn_box(ui, &p.message);
                begin_copy = kit::ghost_button(ui, "Try again").clicked();
            }
        }
    });
    if get_copy {
        m.get_copy();
    }
    if work_copy {
        m.make_working_copy();
    }
    if cancel_copy {
        m.cancel_copy();
    }
    if begin_copy {
        m.begin_copy();
    }
    if go_imported {
        *out = Outcome::ShowImported;
    }

    hide_section(ui, m);

    let cid_now = m.bank.as_ref().map(|b| b.cid.clone()).unwrap_or_default();
    super::circle_edit_ui::bank_section(ui, m, &cid_now, &head.id, &api::plain(&head.title));

    // Author's setting, remove, report.
    bank_admin(ui, m, &head.license);

    // The questions.
    heading(ui, "Questions");
    match &questions {
        Err(e) => warn_box(ui, &format!("The questions could not be shown: {e}")),
        Ok(v) => {
            for q in v.questions.iter().take(100) {
                kit::card().inner_margin(Margin::same(8)).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    let pts = q.points.map(|p| format!(" ({} point{})", p, if p == 1.0 { "" } else { "s" })).unwrap_or_default();
                    ui.add(egui::Label::new(theme::strong(format!("{}.{pts} {}", q.number, btext_shown(&q.stem))).size(13.0)).wrap());
                    if q.has_image || q.has_figure {
                        line(ui, "Has a picture. Pictures are copied with the bank.");
                    }
                    body_lines(ui, &q.body);
                });
                ui.add_space(3.0);
            }
            if v.questions.len() > 100 {
                para(ui, &format!("... and {} more questions.", v.questions.len() - 100));
            }
        }
    }
}

/// A plain, quiet banner in the accent wash (not an error).
fn keys_banner(ui: &mut Ui, s: &str) {
    egui::Frame::new().fill(theme::accent_faded()).corner_radius(8).inner_margin(Margin::same(10)).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.add(egui::Label::new(theme::strong("Answer keys hidden").size(13.0)).wrap());
        ui.add(egui::Label::new(theme::text(s).size(12.5)).wrap());
    });
    ui.add_space(6.0);
}

/// "Answer keys for viewers": the switch for the bank's author (while an editor) or the circle's owner, the
/// confirmation, the decisions the pool asks for, and what it did.
fn hide_section(ui: &mut Ui, m: &mut CirclesModel) {
    // (on a bank whose newest version hiding held back, turning it off is offered as the way out: critic 58 B2;
    // only when the pool said it releases the version: critic 59 B1)
    let offer_off = m.hide_off_offered();
    let allowed = m.hide_allowed() || m.hide_switch_on_held_head();
    let Some(b) = m.bank.as_ref() else { return };
    // (the bank read again after the change: its newest version still held back, critic 60 B2)
    let head_held = b.withheld_head && matches!(b.load, BankLoad::Failed(_));
    if !allowed && matches!(b.hide, HideStage::Idle) {
        return;
    }
    let on = m.hide_key_on();
    let now_i = m.now_instant();
    #[derive(PartialEq)]
    enum Do {
        None,
        Ask(bool),
        Confirm,
        Cancel,
        Retry,
        Decide(super::circle_hide_ui::DecideAct),
    }
    let mut d = Do::None;
    let mut account = false;
    section(ui, text::HIDE_SECTION, |ui| {
        let Some(b) = m.bank.as_ref() else { return };
        let idle = matches!(b.hide, HideStage::Idle | HideStage::Done(_) | HideStage::Failed { .. });
        let mut want = on;
        ui.add_enabled_ui(idle && allowed, |ui| {
            if ui.checkbox(&mut want, "Hide answer keys from viewers").changed() {
                d = Do::Ask(want);
            }
        });
        line(ui, if on { text::HIDE_ON_LINE } else { text::HIDE_OFF_LINE });
        ui.add_space(4.0);
        if offer_off && idle && on && kit::primary_button(ui, "Turn hiding off").clicked() {
            d = Do::Ask(false);
        }
        match &b.hide {
            HideStage::Idle => {}
            HideStage::Confirm { on: to } => {
                body(ui, if *to { text::HIDE_ON_ASK } else { text::HIDE_OFF_ASK });
                line(ui, text::HIDE_TRIES);
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    if kit::primary_button(ui, if *to { "Hide the keys" } else { "Show the keys again" }).clicked() {
                        d = Do::Confirm;
                    }
                    if kit::ghost_button(ui, "Cancel").clicked() {
                        d = Do::Cancel;
                    }
                });
            }
            HideStage::Working { on: to, back } => {
                if let Some(dd) = back {
                    let _ = super::circle_hide_ui::decisions(ui, dd, "Sending your decisions...", "", "Hide the keys", true);
                } else {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(theme::muted(if *to { "Checking the viewers' copy..." } else { "Showing the keys again..." }));
                    });
                }
            }
            HideStage::Decide(dd) => {
                let a = super::circle_hide_ui::decisions(
                    ui,
                    dd,
                    "Before the keys can be hidden, the pool needs these decisions about the copy viewers will get. Nothing has changed yet.",
                    "Change the questions in a working copy and send a new version, then try again.",
                    "Hide the keys",
                    false,
                );
                if a != super::circle_hide_ui::DecideAct::None {
                    d = Do::Decide(a);
                }
            }
            HideStage::Done(r) => {
                for (i, l) in text::hide_result_lines(r, head_held).iter().enumerate() {
                    if i == 0 {
                        body(ui, l);
                    } else {
                        para(ui, l);
                    }
                }
                if kit::ghost_button(ui, "Close").clicked() {
                    d = Do::Cancel;
                }
            }
            HideStage::Failed { problem, .. } => {
                // (the buttons in one row: "Try again" next to "Close", not stacked)
                let w = problem.retry_at.filter(|at| *at > now_i).map(|at| at - now_i);
                warn_box(ui, &problem.message);
                ui.horizontal_wrapped(|ui| {
                    if problem.retry && ui.add_enabled(w.is_none(), |ui: &mut Ui| kit::primary_button(ui, "Try again")).clicked() {
                        d = Do::Retry;
                    }
                    if problem.account && kit::ghost_button(ui, "Open account...").clicked() {
                        account = true;
                    }
                    if kit::ghost_button(ui, "Close").clicked() {
                        d = Do::Cancel;
                    }
                });
                if let Some(w) = w {
                    let mins = w.as_secs().div_ceil(60);
                    line(ui, if mins <= 90 { format!("You can try again in about {mins} minutes.") } else { format!("You can try again in about {} hours.", mins.div_ceil(60)) });
                }
            }
        }
    });
    if account {
        m.request_open_account();
    }
    use super::circle_hide_ui::DecideAct;
    match d {
        Do::None => {}
        Do::Ask(to) => m.ask_hide(to),
        Do::Confirm => m.confirm_hide(),
        Do::Cancel => m.cancel_hide(),
        Do::Retry => m.retry_hide(),
        Do::Decide(DecideAct::Label(at, c)) => m.hide_decide_label(at, c),
        Do::Decide(DecideAct::Order(at, y)) => m.hide_set_order(at, y),
        Do::Decide(DecideAct::AllOrders) => m.hide_all_orders(),
        Do::Decide(DecideAct::Formatting(q, y)) => m.hide_set_formatting(q, y),
        Do::Decide(DecideAct::Send) => m.send_hide_decisions(),
        Do::Decide(DecideAct::Cancel) => m.cancel_hide(),
        Do::Decide(DecideAct::None) => {}
    }
}

fn bank_admin(ui: &mut Ui, m: &mut CirclesModel, current_license: &str) {
    let can_license = m.bank_allowed(BankAction::SetLicense);
    let can_remove = m.bank_allowed(BankAction::Remove);
    let can_report = m.bank_allowed(BankAction::Report);
    let Some(b) = m.bank.as_ref() else { return };
    let problem = b.act_problem.clone();
    let confirming = b.confirm_remove;
    let report_open = b.report.is_some();
    let mut set: Option<License> = None;
    let mut ask_remove = false;
    let mut do_remove = false;
    let mut cancel_remove = false;
    let mut begin_report = false;
    // (an open report form is drawn below: the section is left out when it would be empty)
    if can_license || can_remove || (can_report && !report_open) {
        section(ui, "More", |ui| {
            if let Some(p) = &problem {
                warn_box(ui, &p.message);
            }
            if can_license {
                ui.label(theme::strong("Licence of this bank").size(12.5));
                ui.horizontal_wrapped(|ui| {
                    for l in [License::CircleOnly, License::CcBy, License::CcBySa, License::Cc0] {
                        let on = License::parse(current_license) == Some(l);
                        let label = license_info(l).name;
                        if ui.selectable_label(on, label).clicked() && !on {
                            set = Some(l);
                        }
                    }
                });
                para(ui, text::LICENSE_SETTING_NOTE);
            }
            ui.horizontal_wrapped(|ui| {
                if can_report && !report_open {
                    begin_report = kit::ghost_button(ui, "Report this bank...").clicked();
                }
                if can_remove {
                    if confirming {
                        body(ui, text::REMOVE_BANK);
                    } else {
                        ask_remove = kit::danger_text_button(ui, "Remove from the circle...").clicked();
                    }
                }
            });
            if confirming {
                ui.horizontal(|ui| {
                    do_remove = kit::danger_text_button(ui, "Remove it").clicked();
                    cancel_remove = kit::ghost_button(ui, "Keep it").clicked();
                });
            }
        });
    }
    report_form(ui, m);
    if let Some(l) = set {
        m.set_bank_license(l);
    }
    if ask_remove {
        m.ask_remove_bank();
    }
    if do_remove {
        m.remove_bank();
    }
    if cancel_remove {
        m.cancel_remove_bank();
    }
    if begin_report {
        m.begin_report();
    }
}

fn report_form(ui: &mut Ui, m: &mut CirclesModel) {
    let (mut send, mut cancel) = (false, false);
    let Some(b) = m.bank.as_mut() else { return };
    let Some(f) = b.report.as_mut() else { return };
    section(ui, "Report this bank", |ui| match &f.state {
        ReportState::Sent => {
            body(ui, text::REPORT_THANKS);
            cancel = kit::ghost_button(ui, "Close").clicked();
        }
        _ => {
            ui.label(theme::strong("What is wrong?").size(12.5));
            for r in ReportReasonChoice::ALL {
                ui.radio_value(&mut f.reason, r, r.label());
            }
            ui.add_space(4.0);
            ui.label(theme::strong("A note (optional)").size(12.5));
            ui.add(egui::TextEdit::multiline(&mut f.note).id(egui::Id::new("circle_report_note")).desired_rows(2).desired_width(f32::INFINITY));
            if let ReportState::Failed(p) = &f.state {
                warn_box(ui, &p.message);
            }
            ui.horizontal(|ui| {
                ui.add_enabled_ui(!matches!(f.state, ReportState::Sending), |ui| send = kit::primary_button(ui, "Send the report").clicked());
                cancel = kit::ghost_button(ui, "Cancel").clicked();
                if matches!(f.state, ReportState::Sending) {
                    ui.spinner();
                }
            });
        }
    });
    if send {
        m.send_report();
    }
    if cancel {
        m.cancel_report();
    }
}

/// "You were offered ownership" (Accept / Decline) for the member, "Offer pending" (Withdraw) for the owner.
fn offer_banner(ui: &mut Ui, m: &mut CirclesModel, now: i64) {
    let Some(p) = m.pending_offer().cloned() else { return };
    let (mine, owner) = (m.offered_to_me(), m.can_withdraw_offer());
    if !mine && !owner {
        return;
    }
    let (mut accept, mut cancel) = (false, false);
    let busy = m.busy_action();
    section(ui, if mine { "You were offered ownership of this circle" } else { "Ownership offer pending" }, |ui| {
        if mine {
            body(ui, "The owner would like you to take this circle over. You would become the owner and they would become an editor.");
            line(ui, format!("The offer ends {}.", text::until(p.expires_at, now)));
        } else {
            body(ui, &format!("You offered this circle to {}. You stay the owner until they accept.", api::plain(&p.to_name)));
            line(ui, format!("The offer ends {}.", text::until(p.expires_at, now)));
        }
        ui.horizontal(|ui| {
            ui.add_enabled_ui(!busy, |ui| {
                if mine {
                    accept = kit::primary_button(ui, "Accept ownership").clicked();
                    cancel = kit::ghost_button(ui, "Decline").clicked();
                } else {
                    cancel = kit::ghost_button(ui, "Withdraw the offer").clicked();
                }
            });
        });
    });
    if accept {
        m.accept_offer();
    }
    if cancel {
        m.cancel_offer();
    }
}
