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
use super::publish_ui::{banner, body, link, para, warn_box};
use crate::{kit, theme};

/// What the page asks the rest of the app for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    None,
    /// "Add a bank": open the Share window with this circle chosen.
    ShareToCircle(String),
    /// "See it under Things I imported".
    ShowImported,
    /// "Open in My Banks" (phase D1): the Banks screen on this local bank.
    OpenBank(String),
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
        // (round 17, critic 96 minor 3) a daily limit's wait in hours, not tens of thousands of seconds
        let s = w.as_secs().max(1);
        para(ui, &if s < 90 { format!("You can try again in about {s} seconds.") } else { format!("You can try again {}.", super::api::wait_text(Some(s))) });
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

/// The Circles tab body (also drawn on its own by the QA rig).
pub fn body_of(ui: &mut Ui, m: &mut CirclesModel) -> Outcome {
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
        Screen::Join => join_page(ui, m, &mut out),
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

fn join_page(ui: &mut Ui, m: &mut CirclesModel, out: &mut Outcome) {
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
    let mut offer = OfferAct::None;
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
            // (phase D1) "Add its banks to My Banks?", a tick per bank.
            if let Some(o) = m.join_offer.clone() {
                offer = join_offer(ui, m, &o);
            }
            // (critic 64 polish 7, critic 66 minor 7) while the offer of its banks is up, adding them (then opening
            // them) is the next step, not the circle: one big button at a time
            let offering = m.join_offer.as_ref().is_some_and(|o| o.problem.is_none() && o.banks.as_ref().is_none_or(|b| !b.is_empty()));
            let label = "Open the circle";
            open = if offering { kit::ghost_button(ui, label) } else { kit::primary_button(ui, label) }.clicked();
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
    match offer {
        OfferAct::None => {}
        OfferAct::Tick(bid, on) => m.tick_offer(&bid, on),
        OfferAct::Add => m.add_ticked(),
        OfferAct::NotNow => m.dismiss_offer(),
        OfferAct::Retry(cid, bid) => m.retry_add(&cid, &bid),
        OfferAct::Open(local) => *out = Outcome::OpenBank(local),
    }
    if open {
        m.dismiss_offer();
        m.open_joined();
    }
}

#[derive(Clone, Debug, PartialEq)]
enum OfferAct {
    None,
    Tick(String, bool),
    Add,
    NotNow,
    Retry(String, String),
    Open(String),
}

/// After joining (phase D1, section 4.4): "Add its banks to My Banks?" with a tick per bank (all ticked when there
/// are 10 or fewer), then how each one went.
fn join_offer(ui: &mut Ui, m: &CirclesModel, o: &JoinOffer) -> OfferAct {
    let mut act = OfferAct::None;
    section(ui, "Add its banks to My Banks?", |ui| {
        if let Some(p) = &o.problem {
            warn_box(ui, &p.message);
            line(ui, "You can add its banks from the circle's page.");
            return;
        }
        let Some(banks) = &o.banks else {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(theme::muted("Looking at the circle's banks...").size(12.5));
            });
            return;
        };
        if banks.is_empty() {
            line(ui, "The circle has no banks yet.");
            return;
        }
        para(ui, if o.role.can_add_banks() {
            "Each one is linked to the circle: get newer versions when you choose, and send your changes back. Nothing changes by itself."
        } else {
            "Each one is linked to the circle: get newer versions when you choose. As a viewer you can't send changes back."
        });
        for b in banks {
            // (critic 63 B3) A bank this computer already has is not added again: its link and its changes stay.
            if let Some(name) = &b.in_banks {
                let name = api::plain(name);
                two_tone(ui, &b.title, " - ", &if name.is_empty() { "already in My Banks".to_string() } else { format!("already in My Banks as \"{name}\"") });
                continue;
            }
            if !o.started {
                let mut on = b.ticked;
                if ui.checkbox(&mut on, b.title.as_str()).changed() {
                    act = OfferAct::Tick(b.id.clone(), on);
                }
                continue;
            }
            if !b.ticked {
                // (critic 66 minor 7) said, not dropped
                two_tone(ui, &b.title, " - ", "not added");
                continue;
            }
            match m.add_state(&o.cid, &b.id).map(|j| &j.state) {
                Some(AddState::Done { bank_id, bank_name, .. }) => {
                    text_row(ui, |ui| {
                        two_tone(ui, &b.title, " - ", &format!("in My Banks as \"{}\"", api::plain(bank_name)));
                        if back_link(ui, "Open") {
                            act = OfferAct::Open(bank_id.clone());
                        }
                    });
                }
                Some(AddState::Failed(p)) => {
                    two_tone(ui, &b.title, " - ", &p.message);
                    if kit::ghost_button(ui, "Try again").clicked() {
                        act = OfferAct::Retry(o.cid.clone(), b.id.clone());
                    }
                }
                Some(AddState::Working) => {
                    ui.horizontal_wrapped(|ui| {
                        ui.add(egui::Spinner::new().size(14.0));
                        ui.label(theme::muted(format!("Adding \"{}\"...", b.title)).size(12.5));
                    });
                }
                _ => two_tone(ui, &b.title, " - ", "waiting"),
            }
        }
        if !o.started {
            let n = banks.iter().filter(|b| b.ticked && b.in_banks.is_none()).count();
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(n > 0, |ui: &mut Ui| kit::primary_button(ui, "Add the ticked banks")).clicked() {
                    act = OfferAct::Add;
                }
                if kit::ghost_button(ui, "Not now").clicked() {
                    act = OfferAct::NotNow;
                }
            });
        }
    });
    act
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

    // Banks (phase D1): each row opens the bank in My Banks or adds it there; nothing is edited on this page.
    if banks_section(ui, m, &d, out) {
        return;
    }

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

/// A row's buttons (phase D1): the primary first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RowButton {
    Open,
    Add,
    Own,
    FullCopy,
    BringBack,
}

impl RowButton {
    fn label(self) -> &'static str {
        match self {
            RowButton::Open => "Open in My Banks",
            RowButton::Add => "Add to My Banks",
            RowButton::Own => "Make my own copy",
            RowButton::FullCopy => "Add a full copy",
            RowButton::BringBack => "Bring it back",
        }
    }

    /// What the button does, on hover (critic 64 I1).
    fn hint(self, can_send: bool) -> Option<&'static str> {
        match self {
            RowButton::Add => Some(if can_send { ADD_HINT } else { ADD_HINT_VIEWER }),
            RowButton::Own => Some(OWN_HINT),
            RowButton::FullCopy => Some("A new bank, read in full, that is connected to the circle so you can send from it. Your earlier copy stays, no longer connected."),
            _ => None,
        }
    }
}

const ADD_HINT: &str = "Add to My Banks: the bank stays connected to the circle. You get newer versions when you choose and send your changes back.";
const ADD_HINT_VIEWER: &str = "Add to My Banks: the bank stays connected to the circle, so you can get newer versions (viewers can't send changes).";
const LINKS_UNREADABLE_ROW: &str = "The list of shared banks on this computer couldn't be read just now, so banks can't be added to My Banks: this computer may have them already. Nothing was changed.";
const OWN_HINT: &str = "Make my own copy: a separate bank that is yours alone; nothing goes back and forth.";

/// What a row needs from this computer's link to its bank.
struct RowLink {
    local_id: String,
    bank_name: String,
    missing: bool,
    /// "Add a full copy" is offered (critic 63 I3).
    full_copy: bool,
    lines: Vec<String>,
    /// (round 18, critic 98 minor 4) Sending is off for it (its lines in the warning colour).
    warn: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum RowAct {
    None,
    Open(String),
    Add(String, bool),
    FullCopy(String),
    RetryAdd(String),
    OpenNew(String),
    Restore(String),
    AskPurge(String),
    Purge(String),
    CancelPurge,
    Settings(String),
    Preview(String),
    Report(String),
    AskRemove(String),
    Remove,
    KeepIt,
}

/// The Banks card of the circle page (phase D1, section 4.3). Each bank is a row: "Open in My Banks" when this
/// computer has it, else "Add to My Banks" and "Make my own copy"; under it Bank settings (owner and author),
/// Report... and Remove.... Returns true when the page was left (a bank's settings opened).
fn banks_section(ui: &mut Ui, m: &mut CirclesModel, d: &super::circles_api::CircleDetail, out: &mut Outcome) -> bool {
    let role = d.circle.role();
    let closed = d.circle.closed();
    let cid = d.circle.id.clone();
    let now = m.now_ms();
    let confirming_purge = m.circle.as_ref().and_then(|c| c.confirm_purge.clone());
    let mut rows: std::collections::HashMap<String, RowLink> = Default::default();
    for b in &d.banks {
        let removed = b.removed.is_some();
        let st = m.edit.with_bank_link(&cid, &b.id, |l| l.local_bank_id.clone()).and_then(|local| m.edit.status(&local).cloned());
        if let Some(r) = m.edit.with_bank_link(&cid, &b.id, |l| RowLink {
            local_id: l.local_bank_id.clone(),
            bank_name: st.as_ref().map(|s| s.bank_name.clone()).unwrap_or_default(),
            missing: st.as_ref().is_some_and(|s| s.missing),
            full_copy: super::bank_links::full_copy_offered(l, role),
            lines: super::circle_edit_ui::standing(l, st.as_ref(), removed, role),
            warn: st.as_ref().is_some_and(|s| !s.missing && l.can_send() && role.can_add_banks() && super::bank_links::sending_off(s)),
        }) {
            rows.insert(b.id.clone(), r);
        }
    }
    // (round 16, critic 94 m6) removing a bank she has in My Banks with changes not sent: named under the question
    let remove_unsent = match m.row_bank().filter(|rb| rb.confirm_remove).map(|rb| rb.bid.clone()) {
        Some(bid) => m.edit.unsent_of_bank(&cid, &bid),
        None => Vec::new(),
    };
    let mut act = RowAct::None;
    let mut add = false;
    // (critic 65 B1) while this computer's list of shared banks can not be read, a bank it already has can not be
    // told apart: no "Add to My Banks"
    let links_unreadable = m.edit.unreadable().is_some();
    let mut read_again = false;
    section(ui, "Banks", |ui| {
        if m.allowed(Action::AddBank) {
            // (critic 66 I1) the other way round from "Add to My Banks": one of my banks goes into the circle
            add = kit::primary_button(ui, "Share one of my banks here...").on_hover_text("Shares one of your banks with this circle. It stays in My Banks, connected to the circle.").clicked();
            ui.add_space(4.0);
        }
        // (critic 64 I1) the two verbs, said once where they are offered
        let offers_add = d.banks.iter().any(|b| b.removed.is_none() && !b.hidden && bank_can(role, closed, b, BankAction::Copy) && !rows.get(&b.id).is_some_and(|l| !l.missing));
        if links_unreadable {
            warn_box(ui, LINKS_UNREADABLE_ROW);
            if kit::ghost_button(ui, "Try again").clicked() {
                read_again = true;
            }
        } else if offers_add {
            para(ui, &format!("{} {}", if role.can_add_banks() { ADD_HINT } else { ADD_HINT_VIEWER }, OWN_HINT));
        }
        if d.banks.is_empty() {
            ui.label(theme::strong(text::EMPTY_BANKS).size(13.5));
            para(ui, if role.can_add_banks() { text::EMPTY_BANKS_HELP_EDITOR } else { text::EMPTY_BANKS_HELP_VIEWER });
        }
        for b in &d.banks {
            let link = rows.get(&b.id);
            let present = link.is_some_and(|l| !l.missing);
            let job = m.add_state(&cid, &b.id).cloned();
            let adding = job.as_ref().is_some_and(|j| matches!(j.state, AddState::Waiting | AddState::Working));
            // (round 48: after a partial "Delete for good now" nothing can be brought back or deleted again from here)
            let claimed = m.purge_claimed.get(&b.id).copied();
            let can_restore = claimed.is_none() && bank_can(role, closed, b, BankAction::Restore) && b.removed.as_ref().and_then(|r| r.undo_until).is_some_and(|u| u > now);
            let can_purge = claimed.is_none() && bank_can(role, closed, b, BankAction::Purge);
            let can_copy = !b.hidden && bank_can(role, closed, b, BankAction::Copy);
            let buttons = row_buttons(b.removed.is_some(), can_restore, present, link.is_some_and(|l| l.full_copy), can_copy && !adding, links_unreadable);
            let card = kit::card().inner_margin(Margin::same(8)).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                // (critic 66 I5) after an add that was not connected, adding again is not the next step
                let unconnected = job.as_ref().is_some_and(|j| !j.own && matches!(j.state, AddState::Done { linked: false, .. }));
                // (and Make my own copy is not made the big button while Add can not be offered)
                let clicked = with_actions_hinted(ui, &buttons, role.can_add_banks(), !unconnected && (present || !links_unreadable), |ui| {
                    if b.hidden {
                        ui.add(egui::Label::new(theme::strong(text::HIDDEN_BANK_TITLE).size(13.5)).wrap());
                        line(ui, text::HIDDEN_BANK_LINE.to_string());
                        line(ui, format!("version {} - {}", b.head_version, text::questions_text(b.question_count)));
                    } else {
                        ui.add(egui::Label::new(theme::strong(btext_shown(&b.title)).size(13.5)).wrap());
                        let mut bits = vec![format!("by {}", btext::author(&b.author_name)), format!("version {}", b.head_version), text::questions_text(b.question_count)];
                        let ago = text::ago(b.updated_at, now);
                        if !ago.is_empty() {
                            bits.push(format!("updated {ago}"));
                        }
                        line(ui, bits.join(" - "));
                        // (critic 64 polish 16) only a promise the page keeps: "circle-only" stays inside the circle
                        line(ui, if b.license == "circle-only" { format!("{}. {}", license_name(&b.license), text::reshare_line(&b.license)) } else { format!("{}.", license_name(&b.license)) });
                        if let Some(l) = link {
                            if !l.missing {
                                let name = api::plain(&l.bank_name);
                                ui.add(egui::Label::new(theme::accent(if name.is_empty() { "In My Banks.".to_string() } else { format!("In My Banks as \"{name}\"") }).size(12.5)).wrap());
                            }
                            for s in &l.lines {
                                let t = if l.warn { egui::RichText::new(s.as_str()).color(theme::bad()) } else { theme::accent(s.as_str()) };
                                ui.add(egui::Label::new(t.size(12.5)).wrap());
                            }
                        } else if b.is_author && b.removed.is_none() && !links_unreadable {
                            // (section 11: a bank shared before phase D has no link on this computer)
                            line(ui, "Your original bank is not linked. Add the circle's bank to My Banks to work on it with the circle (this adds a second bank; you can delete your original afterwards).");
                        }
                        if b.hide_key && b.removed.is_none() {
                            line(ui, if role.can_add_banks() { "Answer keys hidden from viewers." } else { "Answer keys hidden for viewers: you get it without them." });
                        }
                        if b.is_held() {
                            ui.add(egui::Label::new(theme::accent(text::HELD_BANK).size(12.5)).wrap());
                        }
                    }
                    if let Some(n) = claimed {
                        line(ui, text::purge_pending_line(n));
                    } else if let Some(r) = &b.removed {
                        line(ui, format!("Removed from the circle. {}", text::undo_line(r.undo_until, now)));
                    }
                    match job.as_ref().map(|j| (&j.state, j.own)) {
                        Some((AddState::Waiting, _)) => line(ui, "Waiting for the copy before it..."),
                        Some((AddState::Working, own)) => {
                            ui.horizontal(|ui| {
                                ui.add(egui::Spinner::new().size(14.0));
                                ui.label(theme::muted(if own { "Making your own copy..." } else { "Adding it to My Banks..." }).size(12.5));
                            });
                        }
                        Some((AddState::Done { bank_id, bank_name, linked: false, why }, own)) => {
                            // (critic 64 I6, polish 8) where it is, and a way to open it; (critic 66 I5) why it is not
                            // connected and, when adding it again can help, what to do
                            let words = if own {
                                format!("Your own copy \"{}\" is in My Banks. It is not linked to the circle.", api::plain(bank_name))
                            } else {
                                let why = why.as_deref().map_or(String::new(), |w| format!(": {w}"));
                                let again = if present { "" } else { " Delete it in My Banks, then add it again." };
                                format!("Added as \"{}\", not connected{why}.{again}", api::plain(bank_name))
                            };
                            text_row(ui, |ui| {
                                ui.add(egui::Label::new(theme::accent(words).size(12.5)).wrap());
                                if back_link(ui, "Open it") {
                                    act = RowAct::OpenNew(bank_id.clone());
                                }
                            });
                        }
                        _ => {}
                    }
                });
                if let Some(i) = clicked {
                    act = match buttons[i] {
                        RowButton::Open => link.map_or(RowAct::None, |l| RowAct::Open(l.local_id.clone())),
                        RowButton::Add => RowAct::Add(b.id.clone(), false),
                        RowButton::FullCopy => RowAct::FullCopy(b.id.clone()),
                        RowButton::Own => RowAct::Add(b.id.clone(), true),
                        RowButton::BringBack => RowAct::Restore(b.id.clone()),
                    };
                }
                if let Some(AddState::Failed(p)) = job.as_ref().map(|j| &j.state) {
                    warn_box(ui, &p.message);
                    // (critic 69 minor 1) "Try again" only where trying again can help (not for another account's add)
                    if p.retry && kit::ghost_button(ui, "Try again").clicked() {
                        act = RowAct::RetryAdd(b.id.clone());
                    }
                }
                // More: what is not editing (Q1).
                let settings = b.removed.is_none() && !b.hidden && !closed && (role == Role::Owner || b.is_author);
                let report = bank_can(role, closed, b, BankAction::Report) && !b.hidden;
                let remove = bank_can(role, closed, b, BankAction::Remove);
                let asking_remove = m.row_bank().is_some_and(|rb| rb.bid == b.id && rb.confirm_remove);
                // (owner decision a) look at the questions before adding, read only
                let preview = b.removed.is_none() && can_copy;
                if settings || report || remove || preview {
                    text_row(ui, |ui| {
                        ui.label(theme::muted("More:").size(12.5));
                        if preview && back_link(ui, "Preview") {
                            act = RowAct::Preview(b.id.clone());
                        }
                        if settings && back_link(ui, "Bank settings") {
                            act = RowAct::Settings(b.id.clone());
                        }
                        if report && back_link(ui, "Report...") {
                            act = RowAct::Report(b.id.clone());
                        }
                        // (critic 64 polish 11) a destructive action in the danger colour; (round 19, critic 100 minor 10) not offered again
                        // while it is asked
                        if remove && !asking_remove && kit::danger_text_button(ui, "Remove...").clicked() {
                            act = RowAct::AskRemove(b.id.clone());
                        }
                    });
                }
                // Removing, asked under the row.
                if let Some(rb) = m.row_bank().filter(|rb| rb.bid == b.id) {
                    if let Some(p) = &rb.act_problem {
                        warn_box(ui, &p.message);
                    }
                    if rb.confirm_remove {
                        // (round 19, critic 100 minor 10) the question has its heading, as in Bank settings
                        ui.add(egui::Label::new(theme::strong(text::REMOVE_BANK_HEAD).size(13.5)).wrap());
                        body(ui, text::REMOVE_BANK);
                        for (name, n, waiting) in &remove_unsent {
                            line(ui, text::leave_unsent(name, *n, *waiting));
                        }
                        ui.horizontal_wrapped(|ui| {
                            if kit::danger_text_button(ui, "Remove it").clicked() {
                                act = RowAct::Remove;
                            }
                            if kit::ghost_button(ui, "Keep it").clicked() {
                                act = RowAct::KeepIt;
                            }
                        });
                    }
                }
                if can_purge {
                    if confirming_purge.as_deref() == Some(b.id.as_str()) {
                        body(ui, text::PURGE_BANK);
                        ui.horizontal_wrapped(|ui| {
                            if kit::danger_text_button(ui, "Delete it for good").clicked() {
                                act = RowAct::Purge(b.id.clone());
                            }
                            if kit::ghost_button(ui, "Keep it").clicked() {
                                act = RowAct::CancelPurge;
                            }
                        });
                    } else if kit::danger_text_button(ui, "Delete for good now...").clicked() {
                        act = RowAct::AskPurge(b.id.clone());
                    }
                }
            });
            if m.scroll_to.as_deref() == Some(b.id.as_str()) {
                ui.scroll_to_rect(card.response.rect, Some(egui::Align::Center));
            }
            // The report form, under its row.
            if m.row_bank().is_some_and(|rb| rb.bid == b.id && rb.report.is_some()) {
                report_form(ui, m);
            }
            ui.add_space(3.0);
        }
    });
    if let Some(bid) = d.banks.iter().find(|b| m.scroll_to.as_deref() == Some(b.id.as_str())).map(|b| b.id.clone()) {
        m.take_scroll_to(&bid);
    }
    if add {
        *out = Outcome::ShareToCircle(d.circle.id.clone());
    }
    if read_again {
        m.edit.read_links_again();
    }
    match act {
        RowAct::None => {}
        RowAct::Open(local) => *out = Outcome::OpenBank(local),
        RowAct::Add(bid, own) => m.add_bank(&bid, own),
        RowAct::FullCopy(bid) => m.add_full_copy(&bid),
        RowAct::OpenNew(local) => *out = Outcome::OpenBank(local),
        RowAct::RetryAdd(bid) => m.retry_add(&cid, &bid),
        RowAct::Restore(bid) => m.restore_bank(&bid),
        RowAct::AskPurge(bid) => m.ask_purge(&bid),
        RowAct::Purge(bid) => m.purge_bank(&bid),
        RowAct::CancelPurge => m.cancel_purge(),
        RowAct::Settings(bid) => {
            m.open_bank(&bid);
            return true;
        }
        RowAct::Preview(bid) => {
            m.open_preview(&bid);
            return true;
        }
        RowAct::Report(bid) => m.begin_report_on(&bid),
        RowAct::AskRemove(bid) => m.ask_remove_on(&bid),
        RowAct::Remove => m.remove_bank(),
        RowAct::KeepIt => m.cancel_remove_bank(),
    }
    false
}

/// A bank row's buttons, primary first (phase D1, section 4.3): the same on the circle page and the preview. A
/// removed bank: Bring it back. A bank this computer has: Open in My Banks (and Add a full copy where offered, and
/// Make my own copy). Else Add to My Banks (not while the links file can not be read, critic 65 B1) and Make my
/// own copy. `can_copy`: the person may copy it and no add of it is under way.
fn row_buttons(removed: bool, can_restore: bool, present: bool, full_copy: bool, can_copy: bool, links_unreadable: bool) -> Vec<RowButton> {
    let mut buttons = Vec::new();
    if removed {
        if can_restore {
            buttons.push(RowButton::BringBack);
        }
    } else if present {
        buttons.push(RowButton::Open);
        if full_copy && can_copy {
            buttons.push(RowButton::FullCopy);
        }
        if can_copy {
            buttons.push(RowButton::Own);
        }
    } else if can_copy {
        if !links_unreadable {
            buttons.push(RowButton::Add);
        }
        buttons.push(RowButton::Own);
    }
    buttons
}

/// A card's content with its buttons on the right, the content wrapping in the width left over (primary first,
/// left to right). In a narrow card the buttons go under the content. Each button says what it does on hover
/// (critic 64 I1). Which one was clicked.
fn with_actions_hinted(ui: &mut Ui, actions: &[RowButton], can_send: bool, primary: bool, body: impl FnOnce(&mut Ui)) -> Option<usize> {
    if actions.is_empty() {
        body(ui);
        return None;
    }
    let gap = ui.spacing().item_spacing.x;
    let bw: f32 = actions.iter().map(|a| button_width(ui, a.label()) + gap).sum::<f32>() - gap;
    let avail = ui.available_width();
    let mut clicked = None;
    let press = |ui: &mut Ui, i: usize, clicked: &mut Option<usize>| {
        let mut r = if i == 0 && primary { kit::primary_button(ui, actions[i].label()) } else { kit::ghost_button(ui, actions[i].label()) };
        if let Some(h) = actions[i].hint(can_send) {
            r = r.on_hover_text(h);
        }
        if r.clicked() {
            *clicked = Some(i);
        }
    };
    if avail < bw + 240.0 {
        body(ui);
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            for i in 0..actions.len() {
                press(ui, i, &mut clicked);
            }
        });
        return clicked;
    }
    ui.horizontal_top(|ui| {
        let w = avail - bw - gap;
        let h = ui
            .allocate_ui_with_layout(egui::vec2(w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.set_width(w);
                body(ui);
            })
            .response
            .rect
            .height();
        let size = egui::vec2(ui.available_width(), h.max(ui.spacing().interact_size.y));
        ui.allocate_ui_with_layout(size, egui::Layout::right_to_left(egui::Align::Center), |ui| {
            for i in (0..actions.len()).rev() {
                press(ui, i, &mut clicked);
            }
        });
    });
    clicked
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
    // (critic 92 I2) the banks of this circle in My Banks with changes not sent yet; (round 16, critic 94 m6) also when closing it
    let unsent = if matches!(m.panel, Panel::ConfirmLeave | Panel::ConfirmClose) { m.cid().map(|c| m.edit.unsent_in(&c)).unwrap_or_default() } else { Vec::new() };
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
                        // (round 18, critic 98 minor 10) as every circle screen says dates
                        line(ui, text::code_until(x));
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
                body(ui, text::LEAVE_MY_BANKS);
                for (name, n, waiting) in &unsent {
                    line(ui, text::leave_unsent(name, *n, *waiting));
                }
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
                for (name, n, waiting) in &unsent {
                    line(ui, text::leave_unsent(name, *n, *waiting));
                }
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
                body(ui, text::REMOVE_MEMBER);
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
            // (round 17, critic 96 minor 8) dates in one form ("Oct 8, 2025")
            let joined = text::date(mem.joined_at);
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

/// "Bank settings" (phase D1, Q1): what is not editing, for the owner and the bank's author: answer keys for
/// viewers, the licence, the history, remove and report. No question list, no send, no merge: the bank is worked
/// on in My Banks.
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
    if b.preview {
        return preview_page(ui, m, out);
    }
    match &b.load {
        BankLoad::Loading => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(theme::muted("Opening the bank's settings..."));
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
            super::circle_edit_ui::history_section(ui, m, &cid, &bid, &api::plain(&title));
            return;
        }
        BankLoad::Ready(_) => {}
    }
    let Some(BankView { load: BankLoad::Ready(l), .. }) = m.bank.as_ref() else { return };
    let head = l.doc.bank.clone();
    let origins_note = l.doc.origins.as_array().is_some_and(|a| !a.is_empty());

    ui.label(theme::muted("Bank settings").size(12.5));
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
        para(ui, text::SETTINGS_KEYS_HIDDEN_NOTE);
    } else if !m.hide_allowed() {
        // (critic 66 minor 10) where the switch is shown, its card says it: not twice
        para(ui, text::SETTINGS_KEYS_NOTE);
    }
    if head.is_author && !matches!(head.held, serde_json::Value::Null | serde_json::Value::Bool(false)) {
        warn_box(ui, text::HELD_BANK);
    }
    if origins_note {
        para(ui, "Some questions in this bank were built on items from the public pool; their credits are kept with it.");
    }
    // (critic 64 I8) the way there, on this page
    para(ui, "The questions are worked on in My Banks.");
    let cid_here = m.bank.as_ref().map(|b| b.cid.clone()).unwrap_or_default();
    let local = m.edit.with_bank_link(&cid_here, &head.id, |l| l.local_bank_id.clone()).filter(|l| !m.edit.status(l).is_some_and(|s| s.missing));
    let adding = m.add_state(&cid_here, &head.id).is_some_and(|j| matches!(j.state, AddState::Waiting | AddState::Working));
    match local {
        Some(l) => {
            if kit::primary_button(ui, "Open in My Banks").clicked() {
                *out = Outcome::OpenBank(l);
            }
        }
        None if adding => {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new().size(14.0));
                ui.label(theme::muted("Adding it to My Banks...").size(12.5));
            });
        }
        // (critic 65 B1) not while this computer's list of shared banks can not be read
        None if m.edit.unreadable().is_some() => warn_box(ui, LINKS_UNREADABLE_ROW),
        None => {
            if kit::primary_button(ui, "Add to My Banks").clicked() {
                let bid = head.id.clone();
                m.add_bank(&bid, false);
            }
        }
    }

    hide_section(ui, m);

    let cid_now = m.bank.as_ref().map(|b| b.cid.clone()).unwrap_or_default();
    super::circle_edit_ui::history_section(ui, m, &cid_now, &head.id, &api::plain(&head.title));

    // Author's setting, remove, report.
    bank_admin(ui, m, &head.license);
}

/// Owner decision (a): a circle bank's questions, read only, before adding it, with the row's own buttons (critic
/// 66 I3). A bank read in full shows its answer key, as the bank page does: the person gets it on adding the bank.
/// A viewer of a bank that hides its keys reads the viewers' form, and is told so. Nothing here edits.
fn preview_page(ui: &mut Ui, m: &mut CirclesModel, out: &mut Outcome) {
    let (mut retry, mut account) = (false, false);
    let mut act = None;
    let Some((bank_cid, bank_bid)) = m.bank.as_ref().map(|b| (b.cid.clone(), b.bid.clone())) else { return };
    let Some(b) = m.bank.as_ref() else { return };
    match &b.load {
        BankLoad::Loading => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(theme::muted("Opening the preview..."));
            });
        }
        BankLoad::Failed(p) => {
            let p = p.clone();
            let w = wait_of(m, &p);
            problem_box(ui, &p, w, &mut retry, &mut account);
        }
        BankLoad::Ready(l) => {
            let head = &l.doc.bank;
            ui.label(theme::muted("Preview").size(12.5));
            ui.add(egui::Label::new(theme::serif(btext_shown(&head.title)).size(20.0)).wrap());
            line(ui, [format!("by {}", btext::author(&head.author_name)), format!("version {}", head.version), text::questions_text(head.question_count)].join(" - "));
            ui.add_space(4.0);
            let full = l.view.as_ref().is_ok_and(|v| v.form == super::pool::Served::Full);
            act = preview_actions(ui, m, &bank_cid, &bank_bid);
            let Some(BankView { load: BankLoad::Ready(l), .. }) = m.bank.as_ref() else { return };
            if l.doc.bank.keys_hidden {
                keys_banner(ui, text::KEYS_HIDDEN_VIEWER);
            }
            para(ui, if full { "Read only: nothing here changes the bank." } else { "Read only: nothing here changes the bank. The answers are not shown." });
            match &l.view {
                Err(e) => warn_box(ui, &format!("The questions could not be shown: {e}")),
                Ok(v) => {
                    for q in v.questions.iter().take(100) {
                        kit::card().inner_margin(Margin::same(8)).show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            let pts = q.points.map(|p| format!(" ({} point{})", p, if p == 1.0 { "" } else { "s" })).unwrap_or_default();
                            ui.add(egui::Label::new(theme::strong(format!("{}.{pts} {}", q.number, btext_shown(&q.stem))).size(13.0)).wrap());
                            if q.has_image || q.has_figure {
                                line(ui, "Has a picture.");
                            }
                            preview_body(ui, &q.body);
                            if let Some(e) = q.explanation.as_deref().map(btext_shown).filter(|e| !e.is_empty()) {
                                line(ui, format!("Explanation: {e}"));
                            }
                        });
                        ui.add_space(3.0);
                    }
                    if v.questions.len() > 100 {
                        para(ui, &format!("... and {} more questions.", v.questions.len() - 100));
                    }
                }
            }
        }
    }
    if retry {
        m.retry_bank();
    }
    if account {
        m.request_open_account();
    }
    match act {
        Some((RowButton::Open, Some(local))) => *out = Outcome::OpenBank(local),
        Some((RowButton::Add, _)) => m.add_bank(&bank_bid, false),
        Some((RowButton::Own, _)) => m.add_bank(&bank_bid, true),
        Some((RowButton::FullCopy, _)) => m.add_full_copy(&bank_bid),
        _ => {}
    }
}

/// The preview's buttons, as the bank's row has them (critic 66 I3), and how an add of it stands. The button
/// pressed, with the bank in My Banks for "Open in My Banks".
fn preview_actions(ui: &mut Ui, m: &mut CirclesModel, cid: &str, bid: &str) -> Option<(RowButton, Option<String>)> {
    let d = m.circle.as_ref().filter(|c| c.id == cid).and_then(|c| c.detail.clone())?;
    let e = d.banks.iter().find(|b| b.id == bid)?;
    let (role, closed) = (d.circle.role(), d.circle.closed());
    let local = m.edit.with_bank_link(cid, bid, |l| (l.local_bank_id.clone(), super::bank_links::full_copy_offered(l, role)));
    let present = local.as_ref().filter(|(l, _)| !m.edit.status(l).is_some_and(|s| s.missing)).cloned();
    let job = m.add_state(cid, bid).cloned();
    let adding = job.as_ref().is_some_and(|j| matches!(j.state, AddState::Waiting | AddState::Working));
    let can_copy = !e.hidden && e.removed.is_none() && bank_can(role, closed, e, BankAction::Copy);
    let links_unreadable = m.edit.unreadable().is_some();
    let buttons = row_buttons(false, false, present.is_some(), present.as_ref().is_some_and(|p| p.1), can_copy && !adding, links_unreadable);
    let unconnected = job.as_ref().is_some_and(|j| !j.own && matches!(j.state, AddState::Done { linked: false, .. }));
    let mut pressed = None;
    if !buttons.is_empty() {
        ui.horizontal_wrapped(|ui| {
            for (i, b) in buttons.iter().enumerate() {
                let mut r = if i == 0 && !unconnected { kit::primary_button(ui, b.label()) } else { kit::ghost_button(ui, b.label()) };
                if let Some(h) = b.hint(role.can_add_banks()) {
                    r = r.on_hover_text(h);
                }
                if r.clicked() {
                    pressed = Some(*b);
                }
            }
        });
    }
    match job.as_ref().map(|j| (&j.state, j.own)) {
        Some((AddState::Waiting | AddState::Working, own)) => {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new().size(14.0));
                ui.label(theme::muted(if own { "Making your own copy..." } else { "Adding it to My Banks..." }).size(12.5));
            });
        }
        Some((AddState::Failed(p), _)) => warn_box(ui, &p.message),
        Some((AddState::Done { bank_name, linked, .. }, own)) => {
            let name = api::plain(bank_name);
            line(ui, if own { format!("Your own copy \"{name}\" is in My Banks.") } else if *linked { format!("In My Banks as \"{name}\".") } else { format!("Added as \"{name}\", not connected to the circle.") });
        }
        None => {}
    }
    ui.add_space(6.0);
    pressed.map(|b| (b, present.map(|p| p.0)))
}

/// A previewed question's body (critic 66 I3, minor 2): choices lettered A., B., C. as on the bank page (a number
/// would read as a decimal in a maths bank), and the answer key when the bank was read in full.
fn preview_body(ui: &mut Ui, b: &super::pool::StudentBody) {
    use super::pool::StudentBody;
    let say = |ui: &mut Ui, s: String| {
        ui.add(egui::Label::new(theme::text(s).size(12.5)).wrap());
    };
    match b {
        StudentBody::Mc { choices } => {
            for (i, c) in choices.iter().enumerate() {
                let lock = if c.locked { "  (stays in place)" } else { "" };
                let tick = if c.correct == Some(true) { " ✓" } else { "" };
                say(ui, format!("   {}. {}{tick}{lock}", char::from(b'A' + (i % 26) as u8), btext_shown(&c.text)));
            }
        }
        StudentBody::TrueFalse { answer } => match answer {
            Some(a) => say(ui, format!("   True{}   False{}", if *a { " ✓" } else { "" }, if *a { "" } else { " ✓" })),
            None => line(ui, "   True / False"),
        },
        StudentBody::Matching { left, word_bank, pairs } => {
            if pairs.is_empty() {
                for l in left {
                    say(ui, format!("   - {}", btext_shown(l)));
                }
            } else {
                for (l, r) in pairs {
                    say(ui, format!("   - {}: {} ✓", btext_shown(l), btext_shown(r)));
                }
            }
            if !word_bank.is_empty() {
                let label = if pairs.is_empty() { "Word bank" } else { "Also in the word bank" };
                line(ui, format!("   {label}: {}", word_bank.iter().map(|w| btext_shown(w)).collect::<Vec<_>>().join(", ")));
            }
        }
        StudentBody::Blank { blanks, accepted } => {
            if accepted.iter().any(|a| !a.is_empty()) {
                for (i, a) in accepted.iter().enumerate() {
                    say(ui, format!("   Blank {}: {} ✓", i + 1, a.iter().map(|w| btext_shown(w)).collect::<Vec<_>>().join(" / ")));
                }
            } else {
                line(ui, format!("   {blanks} blank{} to fill in", if *blanks == 1 { "" } else { "s" }));
            }
        }
        StudentBody::Short { sample } => match sample.as_deref().map(btext_shown).filter(|s| !s.is_empty()) {
            Some(s) => say(ui, format!("   Sample answer: {s}")),
            None => line(ui, "   Written answer"),
        },
        StudentBody::Multipart { parts } => {
            for (i, p) in parts.iter().enumerate() {
                ui.label(theme::strong(format!("   Part {}: {}", i + 1, btext_shown(&p.stem))).size(12.5));
                preview_body(ui, &p.body);
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
    // (critic 62 I2: with the keys hidden, which pictures viewers get can be chosen again; minor: when the picture
    // step follows the confirmation, its button only leads there and costs no try)
    let offer_pictures = m.can_change_pictures();
    let pictures_next = m.hide_pictures_follow();
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
        Pictures(super::circle_hide_ui::PictureAct),
        ChangePictures,
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
        if offer_pictures && kit::ghost_button(ui, text::PICTURES_CHANGE).clicked() {
            d = Do::ChangePictures;
        }
        let change = b.pictures_change;
        match &b.hide {
            HideStage::Idle => {}
            HideStage::Confirm { on: to } => {
                body(ui, if *to { text::HIDE_ON_ASK } else { text::HIDE_OFF_ASK });
                if !pictures_next {
                    line(ui, text::HIDE_TRIES);
                }
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    let label = if pictures_next {
                        text::HIDE_NEXT
                    } else if *to {
                        "Hide the keys"
                    } else {
                        "Show the keys again"
                    };
                    if kit::primary_button(ui, label).clicked() {
                        d = Do::Confirm;
                    }
                    if kit::ghost_button(ui, "Cancel").clicked() {
                        d = Do::Cancel;
                    }
                });
            }
            HideStage::Working { on: to, back } => {
                if let Some(dd) = back {
                    let _ = super::circle_hide_ui::decisions(ui, dd, "Sending your decisions...", "", if change { text::PICTURES_SAVE } else { "Hide the keys" }, true);
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
                    if change { text::PICTURES_CHANGE_DECIDE } else { "Before the keys can be hidden, verSHion needs these decisions about the copy viewers will get. Nothing has changed yet." },
                    "Change the questions in the bank in My Banks and send a new version, then try again.",
                    if change { text::PICTURES_SAVE } else { "Hide the keys" },
                    false,
                );
                if a != super::circle_hide_ui::DecideAct::None {
                    d = Do::Decide(a);
                }
            }
            HideStage::Pictures(p) => {
                let (intro, send) = if change { (text::PICTURES_CHANGE_INTRO, text::PICTURES_SAVE) } else { (text::PICTURES_BANK_INTRO, "Hide the keys") };
                let a = super::circle_hide_ui::pictures(ui, p, intro, send, true, false);
                if a != super::circle_hide_ui::PictureAct::None {
                    d = Do::Pictures(a);
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
    use super::circle_hide_ui::{DecideAct, PictureAct};
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
        Do::Pictures(PictureAct::Keep(q, yes)) => m.hide_keep_picture(q, yes),
        Do::Pictures(PictureAct::Send) => m.send_hide_pictures(),
        Do::Pictures(PictureAct::Cancel) => m.cancel_hide(),
        Do::Pictures(PictureAct::None) => {}
        Do::ChangePictures => m.change_pictures(),
    }
}

fn bank_admin(ui: &mut Ui, m: &mut CirclesModel, current_license: &str) {
    let can_license = m.bank_allowed(BankAction::SetLicense);
    let can_remove = m.bank_allowed(BankAction::Remove);
    let can_report = m.bank_allowed(BankAction::Report);
    let Some(b) = m.bank.as_ref() else { return };
    let problem = b.act_problem.clone();
    let confirming = b.confirm_remove;
    let (cid, bid) = (b.cid.clone(), b.bid.clone());
    // (round 16, critic 94 m6) as under the circle page's row
    let remove_unsent = if confirming { m.edit.unsent_of_bank(&cid, &bid) } else { Vec::new() };
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
                if can_remove && !confirming {
                    ask_remove = kit::danger_text_button(ui, "Remove from the circle...").clicked();
                }
            });
            // (round 17, critic 96 minor 7) the question and its lines each on their own line, under the buttons; (round 18, critic
            // 98 minor 9) under a heading of its own, set apart from "Report this bank..."
            if can_remove && confirming {
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
                ui.add(egui::Label::new(theme::strong(text::REMOVE_BANK_HEAD).size(13.5)).wrap());
                body(ui, text::REMOVE_BANK);
                for (name, n, waiting) in &remove_unsent {
                    line(ui, text::leave_unsent(name, *n, *waiting));
                }
            }
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
