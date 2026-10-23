//! The "My shared items" window (egui). All logic lives in [`MineModel`]; this only draws it.
//!
//! Entry points for the rest of the app:
//!
//! - [`request`]: ask for the window from any screen with a `Context` (the Account window's status
//!   page and the publish flow's Done screen use it).
//! - [`MineUi::show`]: once per frame next to `AccountUi::show`; it does nothing and touches no
//!   network while the window is closed.
//!
//! Every string that came from the server is drawn as a plain label after [`api::plain`], never as
//! markup or a link. The only links are the four legal pages, built from the app's own configured
//! server address.

use std::sync::Arc;

use egui::{Margin, Ui};
use web_time::Instant;

use super::api;
use super::browse::Problem;
use super::browse_text::SIGNED_OUT_LINE;
use super::mine::{Action, CounterForm, ExportState, MineModel, Panel, Screen, issue_text};
use super::mine_text as text;
use super::model::Page;
use super::pool::{ItemState, MineItem, Visibility};
use super::publish::{Gate, ShareSource, snapshot_of};
use super::publish_ui::{banner, body, heading, link, para, status_line, warn_box};
use super::ui::AccountUi;
use crate::model::QuestionBank;
use crate::{kit, theme};

/// What the app can ask the window for without owning it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MineRequest {
    List,
    /// The list, opened at one item.
    Item(String),
    /// The page with the data download, account deletion and the legal pages.
    Data,
}

fn pending_id() -> egui::Id {
    egui::Id::new("share_mine_request")
}

/// Asks for the window (it opens on the next frame).
pub fn request(ctx: &egui::Context, r: MineRequest) {
    ctx.data_mut(|d| d.insert_temp(pending_id(), Some(r)));
    ctx.request_repaint();
}

/// The App's handle on the window. Closed by default; does nothing until opened.
#[derive(Default)]
pub struct MineUi {
    pub model: MineModel,
    repaint_set: bool,
}

impl MineUi {
    pub fn is_open(&self) -> bool {
        self.model.open
    }

    pub fn open(&mut self) {
        self.model.open_list();
    }

    /// The "Shared" mark for a bank page: the item this device shared the bank as, if it did. Reads
    /// one small file the first time the window has been opened; no network.
    pub fn shared_as(&mut self, bank_id: &str) -> Option<String> {
        self.model.shared_record(&format!("bank:{bank_id}")).map(|r| r.item_id)
    }

    /// Draws the window when open; returns at once (no I/O at all) when closed. `banks` are the
    /// local banks, for "Share again...".
    pub fn show(&mut self, ctx: &egui::Context, account: &mut AccountUi, banks: &[QuestionBank]) {
        if let Some(Some(r)) = ctx.data_mut(|d| d.remove_temp::<Option<MineRequest>>(pending_id())) {
            match r {
                MineRequest::List => self.model.open_list(),
                MineRequest::Item(id) => self.model.open_item(&id),
                MineRequest::Data => self.model.open_account_page(),
            }
        }
        if !self.model.open {
            return;
        }
        if !self.repaint_set {
            let c = ctx.clone();
            self.model.set_repaint(Arc::new(move || c.request_repaint()));
            self.repaint_set = true;
        }
        let snap = snapshot_of(&account.model);
        self.model.set_base_url(&super::ShareSettings::legal_base());
        self.model.frame(Instant::now(), &snap);
        if self.model.wants_account_check() {
            account.open_quiet();
            self.model.account_check_asked();
        }
        if self.model.take_open_account() {
            account.reveal();
        }
        if self.model.take_delete_page() {
            account.reveal();
            account.model.go(Page::Delete);
        }
        if self.model.gate == Gate::Ready {
            account.close_quiet();
        }
        if let Some(key) = self.model.take_republish() {
            let src = key.strip_prefix("bank:").and_then(|id| banks.iter().find(|b| b.id == id)).map(ShareSource::from_bank);
            match src {
                Some(Ok(s)) => {
                    super::publish_ui::request(ctx, s);
                    self.model.close();
                    return;
                }
                Some(Err(e)) => self.model.notice = Some(super::model::Notice::Error(e)),
                None => {
                    self.model.notice = Some(super::model::Notice::Error("That bank is not on this device any more, so it can not be shared again from here.".into()))
                }
            }
        }
        if account.visible() {
            return;
        }
        if self.model.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::new("share_mine"))
            .frame(kit::card().inner_margin(Margin::same(22)))
            .show(ctx, |ui| {
                ui.set_width(680.0_f32.min(ctx.content_rect().width() - 60.0).max(280.0));
                egui::ScrollArea::vertical().max_height(ctx.content_rect().height() - 110.0).show(ui, |ui| {
                    close = page(ui, &mut self.model);
                });
            });
        if close || (modal.should_close() && !self.model.busy_action()) {
            self.model.close();
        }
    }
}

// ---- the page ------------------------------------------------------------------------------------------------------

fn title(m: &MineModel) -> &'static str {
    match m.screen {
        Screen::Gate | Screen::List => "My shared items",
        Screen::Detail => "Shared item",
        Screen::Account => "My data and account",
    }
}

fn line(ui: &mut Ui, s: impl Into<String>) {
    ui.add(egui::Label::new(theme::muted(s.into()).size(12.5)).wrap());
}

/// Draws a failure. Returns (retry, open account).
fn problem_box(ui: &mut Ui, m: &MineModel, p: &Problem) -> (bool, bool) {
    warn_box(ui, &p.message);
    let wait = m.retry_wait(p);
    let (mut retry, mut account) = (false, false);
    ui.horizontal_wrapped(|ui| {
        if p.retry {
            ui.add_enabled_ui(wait.is_none(), |ui| retry = kit::primary_button(ui, "Try again").clicked());
        }
        if p.account {
            account = kit::ghost_button(ui, "Open account...").clicked();
        }
    });
    if let Some(w) = wait {
        para(ui, &format!("You can try again in about {} seconds.", w.as_secs().max(1)));
    }
    (retry, account)
}

/// Returns true when the window should close.
pub(super) fn page(ui: &mut Ui, m: &mut MineModel) -> bool {
    let mut close = false;
    ui.horizontal(|ui| {
        ui.label(theme::serif(title(m)).size(24.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !m.busy_action() && kit::ghost_button(ui, "Close").clicked() {
                close = true;
            }
            if m.busy() {
                ui.spinner();
            }
        });
    });
    ui.add_space(4.0);
    if let Some(n) = m.notice.clone() {
        banner(ui, &n);
    }
    match m.screen {
        Screen::Gate => gate_page(ui, m),
        Screen::List => list_page(ui, m),
        Screen::Detail => detail_page(ui, m),
        Screen::Account => account_page(ui, m),
    }
    close
}

fn gate_page(ui: &mut Ui, m: &mut MineModel) {
    let mut open_account = false;
    match m.gate.clone() {
        Gate::Checking => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(theme::muted("Checking your account..."));
            });
        }
        Gate::NotSetUp => para(ui, crate::share::NOT_SET_UP),
        Gate::NoServer => {
            para(ui, "No pool server is set up yet. Open the Account window to enter one.");
            open_account = kit::primary_button(ui, "Open account...").clicked();
        }
        Gate::SignedOut => {
            body(ui, SIGNED_OUT_LINE);
            ui.add_space(6.0);
            open_account = kit::primary_button(ui, "Open account...").clicked();
        }
        Gate::Unreachable => {
            body(ui, "Your sign-in could not be checked. You may be offline.");
            para(ui, "Open the Account window to try again. You can still use everything else in verSHion.");
            open_account = kit::primary_button(ui, "Open account...").clicked();
        }
        Gate::NotApproved(status) => {
            let (head, more) = status_line(&status);
            body(ui, head);
            if !more.is_empty() {
                para(ui, more);
            }
            open_account = kit::primary_button(ui, "Open account...").clicked();
        }
        Gate::Ready => {}
    }
    if open_account {
        m.request_open_account();
    }
}

// ---- the list ----------------------------------------------------------------------------------------------------------

fn state_pill(ui: &mut Ui, it: &MineItem) {
    let (fill, fg) = match it.state {
        ItemState::Live => (theme::accent_faded(), theme::text_col()),
        _ => (theme::error_wash(), theme::bad()),
    };
    kit::pill(ui, text::state_label(it), fill, fg);
}

fn list_page(ui: &mut Ui, m: &mut MineModel) {
    para(ui, "Everything you have shared to the pool, and what you can do with each item.");
    let mut reload = false;
    if let Some(p) = m.list.problem.clone() {
        let (retry, account) = problem_box(ui, m, &p);
        reload = retry;
        if account {
            m.request_open_account();
        }
    }
    if m.list.items.is_empty() && m.list.loaded {
        body(ui, text::NO_ITEMS);
    } else if !m.list.loaded && m.list.problem.is_none() {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(theme::muted("Loading your items..."));
        });
    }
    let mut pick: Option<String> = None;
    let now = m.now_ms();
    for it in &m.list.items {
        if it.state == ItemState::TakedownPending {
            // A pending notice carries no content: the id is all there is.
            kit::card().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(theme::strong("An item you had deleted").size(13.5));
                    state_pill(ui, it);
                });
                line(ui, text::explain(it, now));
                if kit::ghost_button(ui, "Open").clicked() {
                    pick = Some(it.id.clone());
                }
            });
            ui.add_space(6.0);
            continue;
        }
        kit::card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(theme::strong(text::shown(&it.title)).size(13.5));
                state_pill(ui, it);
            });
            line(
                ui,
                format!(
                    "{} - {} - version {} - {}",
                    text::kind_name(&it.kind),
                    text::count(it.question_count),
                    it.version,
                    text::license_name(&it.license)
                ),
            );
            if kit::ghost_button(ui, "Open").clicked() {
                pick = Some(it.id.clone());
            }
        });
        ui.add_space(6.0);
    }
    if m.can_load_more() && kit::ghost_button(ui, "Show more").clicked() {
        m.load_more();
    }
    if let Some(id) = pick {
        m.select(&id);
    }
    if reload {
        m.reload();
    }
    ui.add_space(8.0);
    ui.separator();
    if m.list.storage.limit > 0 {
        line(ui, format!("Space used on the pool: {} of {}.", text::bytes_text(m.list.storage.used as usize), text::bytes_text(m.list.storage.limit as usize)));
    }
    if kit::ghost_button(ui, "My data, account and legal pages").clicked() {
        m.open_account_screen();
    }
}

// ---- one item ----------------------------------------------------------------------------------------------------------

fn detail_page(ui: &mut Ui, m: &mut MineModel) {
    if ui.add(egui::Button::new(theme::accent("< All my items").size(12.5)).frame(false)).clicked() {
        m.back();
        return;
    }
    let Some(it) = m.selected_item().cloned() else {
        body(ui, "That item is not in your list any more.");
        return;
    };
    let now = m.now_ms();
    if it.state == ItemState::TakedownPending {
        ui.label(theme::serif("An item you had deleted").size(20.0));
    } else {
        ui.label(theme::serif(text::shown(&it.title)).size(20.0));
        line(
            ui,
            format!(
                "{} - {} - version {} - {} - {}",
                text::kind_name(&it.kind),
                text::count(it.question_count),
                it.version,
                text::license_name(&it.license),
                if it.is_unlisted() { "private" } else { "public" }
            ),
        );
        if it.import_count > 0 {
            line(ui, format!("Copied by {} other teacher{}.", it.import_count, if it.import_count == 1 { "" } else { "s" }));
        }
    }
    ui.add_space(6.0);
    state_pill(ui, &it);
    ui.add_space(4.0);
    body(ui, &text::explain(&it, now));
    if let Some(a) = m.appeal_answer(&it.id) {
        ui.add_space(4.0);
        para(ui, &format!("Your appeal: {a}"));
    }
    if it.state == ItemState::HeldForReview && !it.positions.is_empty() {
        let nums: Vec<String> = it.positions.iter().take(30).map(|p| (p + 1).to_string()).collect();
        para(ui, &format!("Questions named: {}.", nums.join(", ")));
    }
    ui.add_space(8.0);
    if let Some(p) = m.problem.clone() {
        let (retry, account) = problem_box(ui, m, &p);
        if account {
            m.request_open_account();
        }
        if retry {
            m.problem = None;
        }
    }
    match m.panel.clone() {
        Panel::None => {}
        Panel::ConfirmDelete => return confirm_delete(ui, m),
        Panel::Appeal => return appeal_panel(ui, m),
        Panel::Counter => return counter_panel(ui, m),
        Panel::CounterSent(ack) => return counter_sent(ui, m, &ack),
    }
    heading(ui, "What you can do");
    let actions = m.actions();
    if actions.is_empty() {
        para(ui, "Nothing can be done with this item from here.");
    }
    let busy = m.busy_action();
    ui.add_enabled_ui(!busy, |ui| {
        ui.horizontal_wrapped(|ui| {
            for a in actions {
                let clicked = if a == Action::Delete { kit::danger_text_button(ui, a.label()).clicked() } else { kit::ghost_button(ui, a.label()).clicked() };
                if !clicked {
                    continue;
                }
                match a {
                    Action::MakePublic => m.set_visibility(Visibility::Public),
                    Action::MakePrivate => m.set_visibility(Visibility::Unlisted),
                    Action::Delete => m.ask_delete(),
                    Action::Undo => m.undo_delete(),
                    Action::Republish => m.share_again(),
                    Action::Appeal => m.start_appeal(),
                    Action::CounterNotice => m.start_counter(),
                }
            }
        });
    });
}

fn confirm_delete(ui: &mut Ui, m: &mut MineModel) {
    heading(ui, "Delete this item?");
    para(
        ui,
        "It is hidden from everyone at once. You can undo this for 14 days; after that it is removed for good. \
         Copies other teachers already made stay with them.",
    );
    ui.horizontal(|ui| {
        ui.add_enabled_ui(!m.busy_action(), |ui| {
            if kit::danger_text_button(ui, "Delete it").clicked() {
                m.confirm_delete();
            }
            if kit::ghost_button(ui, "Keep it").clicked() {
                m.cancel_panel();
            }
        });
    });
}

fn appeal_panel(ui: &mut Ui, m: &mut MineModel) {
    heading(ui, "Appeal");
    para(ui, "Tell the admin why this should be put back, in 500 characters or fewer. You can appeal once; an admin reads it and decides.");
    ui.add(egui::TextEdit::multiline(&mut m.appeal_note).id(egui::Id::new("share_mine_appeal")).desired_rows(4).desired_width(f32::INFINITY));
    let n = m.appeal_note.trim().chars().count();
    line(ui, format!("{n} of 500 characters"));
    ui.horizontal(|ui| {
        ui.add_enabled_ui(m.appeal_ready() && !m.busy_action(), |ui| {
            if kit::primary_button(ui, "Send appeal").clicked() {
                m.send_appeal();
            }
        });
        ui.add_enabled_ui(!m.busy_action(), |ui| {
            if kit::ghost_button(ui, "Cancel").clicked() {
                m.cancel_panel();
            }
        });
    });
}

fn counter_field(ui: &mut Ui, label: &str, value: &mut String, id: &str, multiline: bool) {
    ui.label(theme::strong(label).size(12.5));
    if multiline {
        ui.add(egui::TextEdit::multiline(value).id(egui::Id::new(id)).desired_rows(3).desired_width(f32::INFINITY));
    } else {
        ui.add(egui::TextEdit::singleline(value).id(egui::Id::new(id)).desired_width(f32::INFINITY));
    }
    ui.add_space(4.0);
}

fn counter_panel(ui: &mut Ui, m: &mut MineModel) {
    heading(ui, "Counter-notice");
    warn_box(ui, text::COUNTER_DRAFT);
    body(ui, text::COUNTER_INTRO);
    ui.add_space(4.0);
    para(ui, text::COUNTER_NOT_KEPT);
    let f: &mut CounterForm = &mut m.counter;
    counter_field(ui, "Full name", &mut f.name, "share_mine_cn_name", false);
    counter_field(ui, "Address (shared with the person who complained)", &mut f.address, "share_mine_cn_addr", true);
    counter_field(ui, "Phone number (shared with the person who complained)", &mut f.phone, "share_mine_cn_phone", false);
    counter_field(ui, "Which material, and why you believe it was taken down by mistake", &mut f.material_desc, "share_mine_cn_desc", true);
    ui.checkbox(&mut f.perjury_stmt, text::STATEMENT_PERJURY);
    ui.checkbox(&mut f.consent_jurisdiction, text::STATEMENT_JURISDICTION);
    ui.checkbox(&mut f.accept_service, text::STATEMENT_SERVICE);
    ui.add_space(6.0);
    counter_field(ui, "Signature: type your full name again", &mut f.signature, "share_mine_cn_sig", false);
    // What is still missing, once the person has started.
    let started = !(f.name.is_empty() && f.address.is_empty() && f.phone.is_empty() && f.material_desc.is_empty() && f.signature.is_empty());
    if started {
        for (field, issue) in m.counter.issues() {
            line(ui, issue_text(field, issue));
        }
    }
    para(ui, "Sending shares the details above with the person who complained, and with the pool's admins.");
    ui.horizontal(|ui| {
        ui.add_enabled_ui(m.counter.ready() && !m.busy_action(), |ui| {
            if kit::primary_button(ui, "Send counter-notice").clicked() {
                m.send_counter();
            }
        });
        ui.add_enabled_ui(!m.busy_action(), |ui| {
            if kit::ghost_button(ui, "Cancel").clicked() {
                m.cancel_panel();
            }
        });
    });
}

fn counter_sent(ui: &mut Ui, m: &mut MineModel, ack: &super::pool::CounterNoticeAck) {
    heading(ui, "Counter-notice sent");
    body(ui, "It was passed on to the person who complained. verSHion did not keep what you typed.");
    if ack.restore_not_before > 0 {
        para(
            ui,
            &format!(
                "If they do not take the matter to court, the item can be restored from {} and by {}.",
                text::date(ack.restore_not_before),
                text::date(ack.restore_by)
            ),
        );
    }
    if kit::primary_button(ui, "Done").clicked() {
        m.cancel_panel();
    }
}

// ---- data, account, legal ----------------------------------------------------------------------------------------------

fn account_page(ui: &mut Ui, m: &mut MineModel) {
    if ui.add(egui::Button::new(theme::accent("< All my items").size(12.5)).frame(false)).clicked() {
        m.close_account_screen();
        return;
    }
    heading(ui, "Download my data");
    body(ui, text::EXPORT_BLURB);
    ui.add_space(4.0);
    match m.export.clone() {
        ExportState::Idle => {
            if kit::primary_button(ui, "Download my data").clicked() {
                m.start_export();
            }
        }
        ExportState::Running { done, total } => {
            ui.horizontal(|ui| {
                ui.spinner();
                if total == 0 {
                    ui.label(theme::muted("Asking the pool for your data..."));
                } else {
                    ui.label(theme::muted(format!("Fetching part {} of {}...", done + 1, total)));
                }
            });
            if total > 0 {
                ui.add(egui::ProgressBar::new(done as f32 / total as f32).desired_width(260.0));
            }
            if kit::ghost_button(ui, "Cancel").clicked() {
                m.cancel_export();
            }
        }
        ExportState::Ready { size, parts } => {
            body(ui, &format!("Ready: {}{}.", text::bytes_text(size), if parts > 0 { format!(" in {} parts", parts + 1) } else { String::new() }));
            if parts > 0 {
                para(ui, "It is large, so it is saved as one main file and extra files beside it (-part-2, -part-3, ...). Keep them together.");
            }
            ui.horizontal(|ui| {
                if kit::primary_button(ui, "Save to a file...").clicked() {
                    m.save_export();
                }
                if kit::ghost_button(ui, "Discard").clicked() {
                    m.cancel_export();
                }
            });
        }
        ExportState::Saved { place, parts } => {
            body(ui, &format!("Saved to {}{}.", api::plain(&place), if parts > 0 { format!(", with {parts} more file(s) beside it") } else { String::new() }));
            para(ui, "The file holds personal information and your answer keys. Keep it somewhere private. verSHion did not keep a copy.");
            if kit::ghost_button(ui, "Download again").clicked() {
                m.start_export();
            }
        }
        ExportState::Failed(p) => {
            let (retry, account) = problem_box(ui, m, &p);
            if retry {
                m.start_export();
            }
            if account {
                m.request_open_account();
            }
        }
    }
    para(ui, "The pool allows three downloads a day.");

    heading(ui, "Delete my account");
    for l in text::DELETION_LINES {
        line(ui, format!("- {l}"));
    }
    ui.add_space(4.0);
    if kit::danger_text_button(ui, "Delete my account...").clicked() {
        m.request_delete_page();
    }

    heading(ui, "Reports and strikes");
    line(ui, text::STRIKES_LINE);

    heading(ui, "Legal pages (drafts)");
    legal_links(ui, m);
}

/// The four legal pages: links when a server is configured, the names alone when not.
pub(super) fn legal_links(ui: &mut Ui, m: &MineModel) {
    for (name, url) in m.legal_links() {
        match url {
            Some(u) => {
                if link(ui, name) {
                    crate::platform::open_url(&u);
                }
            }
            None => line(ui, name),
        }
    }
    para(ui, text::DRAFT_LEGAL);
}
