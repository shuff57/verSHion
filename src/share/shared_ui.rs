//! The Shared page (egui): a top-level screen of the app, drawn in the app's own page shell. All
//! logic is in [`SharedModel`] (which tab, which guided step) and in the models it hosts:
//! [`BrowseModel`] (Browse and Things I imported), [`MineModel`] (Items I shared, Reports,
//! Settings), the account forms and the invite box. This file only draws.
//!
//! Hooks for the rest of the app: [`goto`] (a contextual button asks for a tab), [`take_goto`]
//! (called once per frame by the app: returns true when the screen should become Shared),
//! [`mark_drawn`] / [`is_drawn`] (the hosted models stop when the page is left).
//!
//! Every server string is drawn as plain text through the hosted pages.

use egui::Ui;

use super::browse_ui::BrowseUi;
use super::circles::CirclesModel;
use super::circles_ui::Outcome;
use super::mine_ui::MineUi;
use super::publish::{Gate, ShareSource, snapshot_of};
use super::shared::{Guide, SHARE_ACTION, STEPS, SharedModel, Tab};
use super::ui::AccountUi;
use crate::model::{Exam, QuestionBank};
use crate::{kit, pageshell, theme};

/// The App's handle on the Shared page.
#[derive(Default)]
pub struct SharedUi {
    pub model: SharedModel,
    /// A line shown under the tabs (why the "Share to pool…" chooser could not start).
    pub notice: Option<String>,
    /// Circles (the tab's own model: it does no I/O until the tab or a circle copy needs it).
    pub circles: CirclesModel,
    /// "Add a bank" on a circle's page: the chooser then sends to this circle.
    pub share_target: Option<String>,
    /// "Open in My Banks" on a circle's page (phase D1): the app opens this bank's page.
    pub open_bank: Option<String>,
    repaint_set: bool,
}

fn goto_id() -> egui::Id {
    egui::Id::new("share_shared_goto")
}

fn circle_goto_id() -> egui::Id {
    egui::Id::new("share_shared_circle_goto")
}

/// Asks for the Shared page on the Circles tab, at a circle (and one of its banks).
pub fn open_circle(ctx: &egui::Context, circle_id: &str, bank_id: Option<&str>) {
    ctx.data_mut(|d| d.insert_temp(circle_goto_id(), Some((circle_id.to_string(), bank_id.map(str::to_string)))));
    ctx.request_repaint();
}

fn drawn_id() -> egui::Id {
    egui::Id::new("share_shared_drawn")
}

/// Asks for the Shared page on this tab (it opens on the next frame).
pub fn goto(ctx: &egui::Context, tab: Tab) {
    ctx.data_mut(|d| d.insert_temp(goto_id(), Some(tab)));
    ctx.request_repaint();
}

/// True when something asked for the Shared page this frame (a contextual button, or the account
/// being asked for): the app then switches the screen. Reads and clears the request.
pub fn take_goto(ctx: &egui::Context, shared: &mut SharedUi, account: &mut AccountUi) -> bool {
    // (owner decision b) Banks the person asked to add finish after the Shared page is left: the circles model's
    // adds advance here, and the finished copy goes into the banks in the browse window's frame (`land_waiting`).
    // Nothing runs unless an add is waiting, under way or landing.
    if !is_drawn(ctx) && shared.circles.tick_background_in(web_time::Instant::now(), &snapshot_of(&account.model), ctx.cumulative_frame_nr()) {
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }
    let mut go = false;
    if ctx.data(|d| d.get_temp::<Option<(String, Option<String>)>>(circle_goto_id())).flatten().is_some() {
        shared.model.goto(Tab::Circles);
        go = true;
    }
    if let Some(Some(tab)) = ctx.data_mut(|d| d.remove_temp::<Option<Tab>>(goto_id())) {
        shared.model.goto(tab);
        go = true;
    }
    if account.take_wants_page() {
        shared.model.goto(Tab::Settings);
        go = true;
    }
    go
}

/// The page was drawn in this frame.
pub fn mark_drawn(ctx: &egui::Context) {
    let n = ctx.cumulative_frame_nr();
    ctx.data_mut(|d| d.insert_temp(drawn_id(), n));
}

/// The page was drawn in this very frame (the hosted models close when it was not).
pub fn is_drawn(ctx: &egui::Context) -> bool {
    let n = ctx.cumulative_frame_nr();
    ctx.data(|d| d.get_temp::<u64>(drawn_id())) == Some(n)
}

impl crate::App {
    /// The Shared screen.
    pub fn draw_shared(&mut self, ctx: &egui::Context) {
        crate::screens::shell_frame(self, ctx, true, |app, ui| {
            let crate::App { shared, account, browse, mine, store, qedit, .. } = app;
            if shared.draw(ui, account, browse, mine, &mut store.banks, &store.exams) {
                store.save_banks();
            }
            // (round 15, critic 91 I1) a difficulty just taken from the circle reaches the open question editor
            super::circle_edit::refresh_draft(qedit.as_mut(), &shared.circles.edit.take_inherited());
        });
        // (phase D1) "Open in My Banks": the bank's page, where its strip says where it stands.
        if let Some(id) = self.shared.open_bank.take()
            && self.store.bank(&id).is_some()
        {
            self.open_bank(&id);
        }
    }
}

impl SharedUi {
    /// The page body inside the app's shell. Models the page hosts are opened here when a tab
    /// needs them and left alone otherwise.
    /// Returns true when the banks changed (a copy was made): the caller saves them.
    pub fn draw(&mut self, ui: &mut Ui, account: &mut AccountUi, browse: &mut BrowseUi, mine: &mut MineUi, banks: &mut Vec<QuestionBank>, exams: &[Exam]) -> bool {
        let mut changed = false;
        mark_drawn(ui.ctx());
        // (phase D1) a bank just shared into a circle is linked here too (the circles model takes it on a read)
        let _ = self.circles.edit.links();
        // (critic 69 minor 3) a circle bank the browse window put into the banks gets its link whatever this page shows
        self.circles.finish_landed();
        // (an address or code field the Circles tab emptied or closed leaves no undo history, whichever tab shows)
        super::circles_ui::forget_cleared_fields(ui.ctx(), &self.circles);
        account.pin();
        let snap = snapshot_of(&account.model);
        // (critic 60 I1: the tabs are drawn only while signed in, so the Circles page is told of a sign-out, or of
        // another account, here; what it read for one account is never shown to the next)
        self.circles.note_account(&snap);
        // (critic 61 I3: the same for the other tabs and the invite step, before anything of theirs is drawn)
        mine.model.note_account(&snap);
        browse.model.note_account(&snap);
        self.model.note_account(&snap, &mut account.invite);
        let guide = self.model.guide(&snap.gate, account.model.page, account.invite.redeemed());
        // An invite code taken: the tabs ask again.
        if account.invite.redeemed() && self.model.invite_needed {
            self.model.note_redeemed();
            browse.model.invite_required = false;
            mine.model.invite_required = false;
            browse.model.close();
            mine.model.close();
            self.circles.invite_required = false;
            self.circles.close();
        }
        if browse.model.invite_required || mine.model.invite_required || self.circles.invite_required {
            self.model.note_invite_required();
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            // The same column as Settings, the other page built from a stack of cards.
            ui.set_max_width(ui.available_width().min(820.0));
            ui.add_space(6.0);
            // The page's one action beside the title, as Banks has "New bank" (once the tabs show).
            let ready = guide == Guide::Ready;
            let avail = ui.available_width();
            let action_w = pill_width(ui, SHARE_ACTION) + 24.0;
            let beside = ready && avail - action_w >= 320.0;
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    if beside {
                        ui.set_max_width(avail - action_w);
                    }
                    pageshell::title(ui, "Home  /  Shared", "Shared", "Find questions other teachers shared, and share your own. Optional: everything else works without it.");
                });
                if beside {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| self.share_action(ui));
                }
            });
            if ready && !beside {
                ui.add_space(10.0);
                self.share_action(ui);
            }
            ui.add_space(14.0);
            match guide {
                Guide::Ready => changed |= self.tabs(ui, account, browse, mine, banks, exams),
                g => self.guide_card(ui, &g, account),
            }
            ui.add_space(12.0);
            pageshell::card(ui, |ui| {
                pageshell::heading(ui, "Legal pages (drafts)");
                ui.add_space(4.0);
                super::mine_ui::legal_links_for(ui, &super::ShareSettings::legal_base());
            });
            ui.add_space(20.0);
        });
        changed
    }

    // ---- guided steps ----------------------------------------------------------------------------------

    pub(super) fn guide_card(&mut self, ui: &mut Ui, g: &Guide, account: &mut AccountUi) {
        pageshell::card(ui, |ui| {
            if let Some(n) = g.step() {
                steps_strip(ui, n);
                ui.add_space(8.0);
            }
            pageshell::heading(ui, g.title());
            ui.add_space(4.0);
            ui.add(egui::Label::new(theme::muted(g.help()).size(13.5)).wrap());
            if !matches!(g, Guide::NotSetUp | Guide::Ready) {
                ui.add_space(10.0);
            }
            match g {
                Guide::Checking => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(theme::muted("One moment..."));
                    });
                }
                // The help line above already says it (NOT_SET_UP would repeat it).
                Guide::NotSetUp => {}
                Guide::Offline => {
                    if kit::primary_button(ui, "Try again").clicked() {
                        account.model.check_now();
                    }
                }
                Guide::NeedsInvite => account.draw_invite(ui),
                Guide::Blocked(_) | Guide::Waiting | Guide::SignIn | Guide::ConfirmEmail | Guide::NoServer => account.draw_section(ui),
                Guide::Ready => {}
            }
        });
    }

    // ---- the tabs ------------------------------------------------------------------------------------------

    /// The tabs. Returns true when a circle copy changed the banks.
    pub(super) fn tabs(&mut self, ui: &mut Ui, account: &mut AccountUi, browse: &mut BrowseUi, mine: &mut MineUi, banks: &mut Vec<QuestionBank>, exams: &[Exam]) -> bool {
        // Circles first: a request to open a circle or bank, the model's frame, copies and badges.
        let snap = snapshot_of(&account.model);
        if !self.repaint_set {
            let c = ui.ctx().clone();
            self.circles.set_repaint(std::sync::Arc::new(move || c.request_repaint()));
            self.repaint_set = true;
        }
        if let Some(Some((cid, bid))) = ui.ctx().data_mut(|d| d.remove_temp::<Option<(String, Option<String>)>>(circle_goto_id())) {
            self.model.goto(Tab::Circles);
            match bid {
                Some(b) => self.circles.open_bank_at(&cid, &b),
                None => self.circles.open_circle_at(&cid),
            }
        }
        let copies = browse.model.circle_bank_ids();
        if !self.circles.open && (self.model.tab == Tab::Circles || !copies.is_empty()) {
            self.circles.open_list();
        }
        // (critic 71 I1) which frame this is: a History left open here and not drawn is not in front of the person
        self.circles.edit.set_pass(ui.ctx().cumulative_frame_nr());
        self.circles.frame(web_time::Instant::now(), &snap);
        if self.circles.open {
            self.circles.want_heads(copies);
            if let Some(h) = self.circles.take_heads() {
                browse.model.apply_circle_heads(&h);
            }
            if self.circles.busy() {
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
            }
        }
        if self.circles.take_open_account() {
            account.reveal();
        }
        let changed = self.circles.sync(banks, &mut browse.model);
        // Keep the hosted models running only for the tab that needs them.
        match self.model.tab {
            Tab::Circles => {}
            Tab::Browse => {
                if !browse.model.open {
                    browse.model.open_browse();
                }
            }
            Tab::Imported => {
                if !browse.model.open {
                    browse.model.open_imported();
                }
            }
            Tab::Mine | Tab::Reports | Tab::Settings => {
                if !mine.model.open {
                    mine.model.open_list();
                }
            }
        }
        ui.horizontal_wrapped(|ui| {
            // A tab that does not fit moves to the next row whole: its label never wraps.
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            for t in Tab::ALL {
                let badge = match t {
                    Tab::Imported => browse.model.update_count(),
                    _ => 0,
                };
                let label = if badge > 0 { format!("{} ({badge})", t.label()) } else { t.label().to_string() };
                // The same 34 pt pills as every button in the cards below.
                let clicked = if self.model.tab == t { kit::primary_button(ui, &label) } else { tab_button(ui, &label) }.clicked();
                if clicked {
                    self.model.goto(t);
                }
            }
        });
        if let Some(n) = &self.notice {
            ui.add_space(4.0);
            ui.label(theme::muted(n.as_str()).size(13.0));
        }
        if self.model.choosing {
            ui.add_space(8.0);
            self.chooser(ui, banks, exams);
        }
        ui.add_space(10.0);
        let tab = self.model.tab;
        pageshell::card(ui, |ui| match tab {
            Tab::Browse => super::browse_ui::tab_body(ui, &mut browse.model, super::browse::Tab::Search),
            Tab::Imported => {
                super::browse_ui::tab_body(ui, &mut browse.model, super::browse::Tab::Imported);
                // (critic 68 minor 6) "Open in My Banks" on a linked copy
                if let Some(local) = browse.model.open_bank.take() {
                    self.open_bank = Some(local);
                }
            }
            Tab::Mine => {
                pageshell::heading(ui, "Items I shared");
                ui.add_space(6.0);
                super::mine_ui::items_body(ui, &mut mine.model);
                if mine.model.list.loaded && mine.model.list.items.is_empty() && mine.model.screen == super::mine::Screen::List {
                    ui.add_space(6.0);
                    if kit::primary_button(ui, SHARE_ACTION).clicked() {
                        self.model.choosing = true;
                    }
                }
            }
            Tab::Circles => match super::circles_ui::body_of(ui, &mut self.circles) {
                Outcome::None => {}
                Outcome::ShareToCircle(cid) => {
                    self.share_target = Some(cid);
                    self.model.choosing = true;
                }
                Outcome::ShowImported => self.model.goto(Tab::Imported),
                Outcome::OpenBank(local) => self.open_bank = Some(local),
            },
            Tab::Reports => reports_body(ui, mine, &mut self.model),
            Tab::Settings => {
                pageshell::heading(ui, "My sharing settings");
                ui.add_space(6.0);
                // "Delete my account" is in the part below, with what deleting does.
                account.draw_settings_section(ui);
                ui.add_space(8.0);
                ui.separator();
                // The page's own card lists the legal pages.
                super::mine_ui::settings_body(ui, &mut mine.model, false);
            }
        });
        changed
    }

    /// The page's action: opens (or closes) the "What do you want to share?" chooser.
    fn share_action(&mut self, ui: &mut Ui) {
        if kit::primary_button(ui, SHARE_ACTION).on_hover_text("Share a bank or an exam with other teachers").clicked() {
            self.model.toggle_chooser();
        }
    }

    /// Pick a bank or an exam: the publish window opens for it.
    fn chooser(&mut self, ui: &mut Ui, banks: &[QuestionBank], exams: &[Exam]) {
        pageshell::card(ui, |ui| {
            pageshell::heading(ui, if self.share_target.is_some() { "Which bank do you want to share with the circle?" } else { "What do you want to share?" });
            ui.add_space(4.0);
            if banks.is_empty() && exams.is_empty() {
                ui.add(egui::Label::new(theme::muted("You have no banks or exams yet. Make a bank on the Banks page, add some questions, then come back.").size(13.0)).wrap());
                return;
            }
            ui.label(theme::strong("Banks").size(13.0));
            if banks.is_empty() {
                ui.label(theme::muted("None yet.").size(12.5));
            }
            for b in banks {
                ui.horizontal(|ui| {
                    ui.label(theme::text(format!("{} ({} question{})", b.name, b.questions.len(), if b.questions.len() == 1 { "" } else { "s" })).size(13.0));
                    if kit::ghost_button(ui, "Share").clicked() {
                        self.start(ui, ShareSource::from_bank(b));
                    }
                });
            }
            ui.add_space(6.0);
            ui.label(theme::strong("Exams").size(13.0));
            if exams.is_empty() {
                ui.label(theme::muted("None yet.").size(12.5));
            }
            for e in exams {
                ui.horizontal(|ui| {
                    ui.label(theme::text(e.name.as_str()).size(13.0));
                    if kit::ghost_button(ui, "Share").clicked() {
                        self.start(ui, ShareSource::from_exam(e, banks));
                    }
                });
            }
        });
    }

    fn start(&mut self, ui: &Ui, src: Result<ShareSource, String>) {
        match src {
            Ok(s) => {
                match self.share_target.take() {
                    Some(cid) => super::publish_ui::request_for_circle(ui.ctx(), s, &cid),
                    None => super::publish_ui::request(ui.ctx(), s),
                }
                self.model.choosing = false;
                self.notice = None;
            }
            Err(e) => self.notice = Some(e),
        }
    }
}

/// "Step 3 of 5" with the five steps as small marks.
fn steps_strip(ui: &mut Ui, now: u8) {
    const NAMES: [&str; 5] = ["Sign in", "Confirm email", "Approval", "Invite code", "Browse"];
    ui.horizontal_wrapped(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        ui.label(theme::muted(format!("Step {now} of {STEPS}")).size(12.5));
        for (i, n) in NAMES.iter().enumerate() {
            let k = i as u8 + 1;
            let (fill, fg) = if k == now {
                (theme::accent_col(), theme::on_accent_col())
            } else if k < now {
                (theme::accent_faded(), theme::text_col())
            } else {
                (egui::Color32::TRANSPARENT, theme::muted_col())
            };
            step_pill(ui, n, fill, fg);
        }
    });
}

/// `kit::pill`'s look, allocated in one piece so a wrapping row moves it to the next line whole
/// (a framed pill is laid out where the row's cursor is and squeezed there instead).
pub(super) fn step_pill(ui: &mut Ui, label: &str, fill: egui::Color32, fg: egui::Color32) {
    let text = egui::RichText::new(label).family(theme::bold_family()).size(11.0).color(fg);
    let galley = egui::WidgetText::from(text).into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Body);
    let (rect, _) = ui.allocate_exact_size(galley.size() + egui::vec2(18.0, 4.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, egui::CornerRadius::same(255), fill);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, fg);
}

/// A tab that is not showing: `kit::ghost_button`'s pill on the card colour, since the tabs sit on
/// the page's grid rather than in a card.
fn tab_button(ui: &mut Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(label).family(theme::bold_family()).color(theme::ink()))
            .fill(theme::card())
            .stroke(egui::Stroke::new(1.0, theme::rule()))
            .corner_radius(egui::CornerRadius::same(17))
            .min_size(egui::vec2(0.0, 34.0)),
    )
}

/// The width of a `kit` pill button with this label.
fn pill_width(ui: &Ui, label: &str) -> f32 {
    let text = egui::RichText::new(label).family(theme::bold_family());
    let galley = egui::WidgetText::from(text).into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Button);
    galley.size().x + 2.0 * ui.spacing().button_padding.x
}

/// Reports and appeals. The pool keeps no list of the reports a person sent, so this shows what it
/// does know: items of mine that need attention, and appeals sent in this session.
fn reports_body(ui: &mut Ui, mine: &mut MineUi, model: &mut SharedModel) {
    use super::mine_text as text;
    use super::pool::ItemState;
    pageshell::heading(ui, "Reports and appeals");
    ui.add_space(4.0);
    ui.add(
        egui::Label::new(
            theme::muted("A report you send about someone else's item is read by the pool's reviewers. The pool does not list reports back to the person who sent them, so none are shown here. Below are your own items that need attention.")
                .size(13.0),
        )
        .wrap(),
    );
    ui.add_space(8.0);
    let m = &mut mine.model;
    if !m.list.loaded {
        if let Some(p) = &m.list.problem {
            ui.add(egui::Label::new(theme::text(super::api::plain(&p.message)).size(13.0)).wrap());
        } else {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(theme::muted("Loading your items..."));
            });
        }
        return;
    }
    let needs: Vec<(String, String, String)> = m
        .list
        .items
        .iter()
        .filter(|i| !matches!(i.state, ItemState::Live | ItemState::Deleted))
        .map(|i| (i.id.clone(), if i.title.is_empty() { "An item you had deleted".to_string() } else { text::shown(&i.title) }, text::state_label(i).to_string()))
        .collect();
    if needs.is_empty() {
        ui.label(theme::text("Nothing needs your attention. None of your items is held, removed or taken down.").size(13.0));
    }
    let mut open: Option<String> = None;
    for (id, title, state) in &needs {
        kit::card().inner_margin(egui::Margin::same(10)).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(theme::strong(title.as_str()).size(13.5));
                kit::pill(ui, state, theme::error_wash(), theme::bad());
            });
            if let Some(a) = m.appeal_answer(id) {
                ui.label(theme::muted(format!("Your appeal: {a}")).size(12.5));
            }
            if kit::ghost_button(ui, "Open").clicked() {
                open = Some(id.clone());
            }
        });
        ui.add_space(4.0);
    }
    if let Some(id) = open {
        m.select(&id);
        model.goto(Tab::Mine);
    }
    ui.add_space(6.0);
    ui.label(theme::muted("An appeal is sent from the item's own page, once.").size(12.5));
}

/// Gate state used by the page's tests.
pub fn gate_of(account: &AccountUi) -> Gate {
    snapshot_of(&account.model).gate
}
