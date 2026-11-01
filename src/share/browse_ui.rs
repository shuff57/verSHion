//! The pool browser window (egui). All logic lives in [`BrowseModel`]; this only draws it.
//!
//! Entry points for the rest of the app:
//!
//! - [`request`]: ask for the window from any screen with a `Context` (the Account window's
//!   "Browse shared questions" button uses it).
//! - [`BrowseUi::open_for_bank`] / [`BrowseUi::open_update`]: the "From pool..." and "Update from
//!   pool" buttons of a bank page.
//! - [`BrowseUi::show`]: once per frame next to `AccountUi::show`; it does nothing and touches no
//!   network while the window is closed.
//!
//! Every string that came from the server is drawn as a plain label after [`api::plain`], never as
//! markup or a link.

use std::sync::Arc;

use egui::{Margin, Ui};
use web_time::Instant;

use super::api;
use super::browse::{
    BrowseModel, Changes, Dest, ImportChoice, ImportDone, ImportPanel, ImportedRecord, Load, Loaded, MergeDone, MergePhase, Problem, ReportState, Screen, Tab,
};
use super::merge::{Choice, MergePlan, Row, RowKind};
use super::browse_text::{self as text, ReportChoice};
use super::pool::{ListItem, License, Served};
use super::publish::{Gate, snapshot_of};
use super::publish_text::license_info;
use super::publish_ui::{banner, body, body_lines, heading, para, status_line, warn_box};
use super::ui::AccountUi;
use crate::model::QuestionBank;
use crate::{kit, theme};

/// What the app can ask the window for without owning it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrowseRequest {
    Browse,
    ForBank(String),
    Imported,
    Update(String),
}

fn pending_id() -> egui::Id {
    egui::Id::new("share_browse_request")
}

/// Asks for the pool browser (it opens on the next frame).
pub fn request(ctx: &egui::Context, r: BrowseRequest) {
    ctx.data_mut(|d| d.insert_temp(pending_id(), Some(r)));
    ctx.request_repaint();
}

/// The App's handle on the browser window. Closed by default; does nothing until opened.
#[derive(Default)]
pub struct BrowseUi {
    pub model: BrowseModel,
    repaint_set: bool,
}

impl BrowseUi {
    pub fn is_open(&self) -> bool {
        self.model.open
    }

    pub fn open_for_bank(&mut self, bank_id: &str) {
        self.model.open_for_bank(bank_id);
    }

    pub fn open_update(&mut self, bank_id: &str) {
        self.model.open_update(bank_id);
    }

    /// True when the last update check found a newer version of something this bank was imported
    /// from. Reads one small file the first time; no network.
    pub fn has_update(&mut self, bank_id: &str) -> bool {
        self.model.update_for_bank(bank_id).is_some()
    }

    /// Draws the window when open; returns at once (no I/O at all) when closed. Returns true when
    /// `banks` changed (an import finished): the caller then saves them.
    pub fn show(&mut self, ctx: &egui::Context, account: &mut AccountUi, banks: &mut Vec<QuestionBank>) -> bool {
        let mut opened_now = false;
        if let Some(Some(r)) = ctx.data_mut(|d| d.remove_temp::<Option<BrowseRequest>>(pending_id())) {
            let tab = if matches!(r, BrowseRequest::Imported | BrowseRequest::Update(_)) { super::shared::Tab::Imported } else { super::shared::Tab::Browse };
            match r {
                BrowseRequest::Browse => self.model.open_browse(),
                BrowseRequest::ForBank(id) => self.model.open_for_bank(&id),
                BrowseRequest::Imported => self.model.open_imported(),
                BrowseRequest::Update(id) => self.model.open_update(&id),
            }
            // In the app the browser is a tab of the Shared page: go there.
            if account.page_mode {
                super::shared_ui::goto(ctx, tab);
                opened_now = true;
            }
        }
        if !self.model.open {
            return false;
        }
        if account.page_mode && !opened_now && !super::shared_ui::is_drawn(ctx) {
            // The Shared page was left: nothing keeps running behind it.
            self.model.close();
            return false;
        }
        if !self.repaint_set {
            let c = ctx.clone();
            self.model.set_repaint(Arc::new(move || c.request_repaint()));
            self.repaint_set = true;
        }
        let snap = snapshot_of(&account.model);
        self.model.frame(Instant::now(), &snap);
        let changed = self.model.sync(banks);
        if self.model.wants_account_check() {
            account.open_quiet();
            self.model.account_check_asked();
        }
        if self.model.take_open_account() {
            account.reveal();
        }
        if self.model.gate == Gate::Ready {
            account.close_quiet();
        }
        if account.visible() {
            return changed;
        }
        if self.model.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
        if account.page_mode {
            return changed;
        }
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::new("share_browse"))
            .frame(kit::card().inner_margin(Margin::same(22)))
            .show(ctx, |ui| {
                ui.set_width(760.0_f32.min(ctx.content_rect().width() - 60.0).max(280.0));
                let tall = ctx.content_rect().height() - 110.0; // grows past last frame's size: see publish_ui
                egui::ScrollArea::vertical().max_height(tall).min_scrolled_height(tall).show(ui, |ui| {
                    close = page(ui, &mut self.model);
                });
            });
        if close || (modal.should_close() && !self.model.busy()) {
            self.model.close();
        }
        changed
    }
}

// ---- the page ------------------------------------------------------------------------------------------------------

fn title(m: &BrowseModel) -> &'static str {
    match m.screen {
        Screen::Gate | Screen::Browse => "Shared questions",
        Screen::Item => "Shared item",
    }
}

fn line(ui: &mut Ui, s: impl Into<String>) {
    ui.add(egui::Label::new(theme::muted(s.into()).size(12.5)).wrap());
}

fn problem_box(ui: &mut Ui, m: &BrowseModel, p: &Problem, retry: &mut bool, account: &mut bool) {
    warn_box(ui, &p.message);
    let wait = m.retry_wait(p);
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

/// Returns true when the window should close.
pub(super) fn page(ui: &mut Ui, m: &mut BrowseModel) -> bool {
    let mut close = false;
    ui.horizontal(|ui| {
        ui.label(theme::serif(title(m)).size(24.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !m.busy_import() && kit::ghost_button(ui, "Close").clicked() {
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
        Screen::Browse => browse_page(ui, m),
        Screen::Item => item_page(ui, m),
    }
    close
}

fn gate_page(ui: &mut Ui, m: &mut BrowseModel) {
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
            body(ui, text::SIGNED_OUT_LINE);
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

// ---- browse ------------------------------------------------------------------------------------------------------------

const KINDS: [&str; 6] = ["Mc", "Tf", "Matching", "Blank", "Short", "Multipart"];

fn combo(ui: &mut Ui, id: &str, label: &str, value: &mut String, options: &[(&str, String)]) {
    let shown = options.iter().find(|(v, _)| v == value).map_or("Any".to_string(), |(_, n)| n.clone());
    ui.label(theme::strong(label).size(12.5));
    egui::ComboBox::from_id_salt(id).selected_text(shown).width(190.0).show_ui(ui, |ui| {
        ui.selectable_value(value, String::new(), "Any");
        for (v, n) in options {
            ui.selectable_value(value, (*v).to_string(), n.as_str());
        }
    });
}

fn text_filter(ui: &mut Ui, id: &str, label: &str, value: &mut String, hint: &str) {
    ui.label(theme::strong(label).size(12.5));
    ui.add(egui::TextEdit::singleline(value).id(egui::Id::new(id)).hint_text(hint).desired_width(190.0));
}

fn browse_page(ui: &mut Ui, m: &mut BrowseModel) {
    ui.horizontal(|ui| {
        for (t, name) in [(Tab::Search, "Search the pool"), (Tab::Imported, "What I imported")] {
            let on = m.tab == t;
            let b = ui.add(egui::Button::new(if on { theme::strong(name).size(13.5) } else { theme::muted(name).size(13.5) }).frame(false));
            if b.clicked() {
                m.tab = t;
            }
        }
        if m.update_count() > 0 {
            kit::pill(ui, &format!("{} update{}", m.update_count(), if m.update_count() == 1 { "" } else { "s" }), theme::accent_faded(), theme::text_col());
        }
    });
    ui.separator();
    match m.tab {
        Tab::Search => search_tab(ui, m),
        Tab::Imported => imported_tab(ui, m),
    }
}

/// The browser as a tab of the Shared page: `tab` is the page's tab (search or imported). An open
/// item (or its update, merge and report pages) covers the tab until "Back".
pub(super) fn tab_body(ui: &mut Ui, m: &mut BrowseModel, tab: Tab) {
    m.tab = tab;
    if let Some(n) = m.notice.clone() {
        banner(ui, &n);
    }
    match m.screen {
        Screen::Gate => gate_page(ui, m),
        Screen::Item => item_page(ui, m),
        Screen::Browse => match tab {
            Tab::Search => search_tab(ui, m),
            Tab::Imported => imported_tab(ui, m),
        },
    }
}

fn search_tab(ui: &mut Ui, m: &mut BrowseModel) {
    let (mut go, mut clear) = (false, false);
    ui.horizontal(|ui| {
        let r = ui.add(egui::TextEdit::singleline(&mut m.form.q).id(egui::Id::new("browse_q")).hint_text("Search titles, subjects, tags and question text").desired_width(ui.available_width() - 190.0));
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        go = kit::primary_button(ui, "Search").clicked() || enter;
        clear = kit::ghost_button(ui, "Clear").clicked();
    });
    egui::CollapsingHeader::new(theme::muted("Filters").size(12.5)).id_salt("browse_filters").show(ui, |ui| {
        egui::Grid::new("browse_filter_grid").num_columns(4).spacing([12.0, 6.0]).show(ui, |ui| {
            text_filter(ui, "browse_subject", "Subject", &mut m.form.subject, "e.g. Algebra");
            let kinds: Vec<(&str, String)> = KINDS.iter().map(|k| (*k, text::kind_name(k).to_string())).collect();
            combo(ui, "browse_kind", "Kind of question", &mut m.form.kind, &kinds);
            ui.end_row();
            text_filter(ui, "browse_tag", "Tag", &mut m.form.tag, "");
            text_filter(ui, "browse_standard", "Standard", &mut m.form.standard, "e.g. CCSS.MATH.8.EE");
            ui.end_row();
            let lic: Vec<(&str, String)> = License::ALL.iter().filter(|l| **l != License::CircleOnly).map(|l| (l.as_str(), license_info(*l).name.to_string())).collect();
            combo(ui, "browse_license", "Licence", &mut m.form.license, &lic);
            text_filter(ui, "browse_grade", "Grade (this page)", &mut m.form.grade, "e.g. 8");
            ui.end_row();
        });
        para(ui, text::LICENSE_FILTER_NOTE);
    });
    if clear {
        m.clear_search();
    } else if go {
        m.search();
    }
    ui.add_space(6.0);
    results(ui, m);
}

fn results(ui: &mut Ui, m: &mut BrowseModel) {
    let (mut retry, mut account) = (false, false);
    if let Some(p) = m.results.problem.clone() {
        problem_box(ui, m, &p, &mut retry, &mut account);
    }
    if retry {
        m.retry_search();
    }
    if account {
        m.request_open_account();
    }
    if !m.results.loaded {
        if m.results.problem.is_none() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(theme::muted("Searching the pool..."));
            });
        }
        return;
    }
    let visible: Vec<ListItem> = m.visible().into_iter().cloned().collect();
    if m.results.items.is_empty() {
        if m.results.page == 0 {
            body(ui, if m.form.is_empty() { "Nothing has been shared publicly yet." } else { "Nothing matches that search." });
            para(ui, "Try fewer words, or clear the filters. Only public items appear here.");
        } else {
            body(ui, "There are no more results.");
        }
    } else if visible.is_empty() {
        body(ui, "Nothing on this page matches that grade.");
        para(ui, "The grade box only narrows the page you are looking at. Go to the next page, or clear it.");
    }
    let mut open: Option<ListItem> = None;
    for it in &visible {
        if card(ui, it) {
            open = Some(it.clone());
        }
        ui.add_space(4.0);
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_enabled_ui(m.can_prev(), |ui| {
            if kit::ghost_button(ui, "Previous page").clicked() {
                m.prev_page();
            }
        });
        ui.label(theme::muted(format!("Page {}", m.results.page + 1)).size(12.5));
        ui.add_enabled_ui(m.can_next(), |ui| {
            if kit::ghost_button(ui, "Next page").clicked() {
                m.next_page();
            }
        });
    });
    if let Some(it) = open {
        let id = it.id.clone();
        m.open_item(&id, Some(it));
    }
}

/// One result. True when it was opened.
fn card(ui: &mut Ui, it: &ListItem) -> bool {
    let mut opened = false;
    kit::card().inner_margin(Margin::same(10)).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.add(egui::Label::new(theme::strong(text::shown(&it.title)).size(14.0)).wrap());
                let mut bits = Vec::new();
                for p in [api::plain(&it.subject), api::plain(&it.course), super::circles_text::grade_text(&it.grade)] {
                    if !p.is_empty() {
                        bits.push(p);
                    }
                }
                bits.push(text::count(it.question_count));
                if !it.kinds.is_empty() {
                    bits.push(it.kinds.iter().map(|k| text::kind_name(k)).collect::<Vec<_>>().join(", "));
                }
                line(ui, bits.join(" - "));
                let mut meta = format!("{} - by {} - version {}", text::license_name(&it.license), text::author(&it.author_name), it.version);
                let d = text::date(it.committed_at);
                if !d.is_empty() {
                    meta.push_str(&format!(" - updated {d}"));
                }
                line(ui, meta);
                line(ui, if it.hide_key { "Answer key hidden" } else { "Answer key shared" });
                if let Some(mark) = it.license_check.as_ref().and_then(text::check_marker) {
                    ui.add(egui::Label::new(theme::accent(mark).size(12.0)).wrap());
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                opened = kit::primary_button(ui, "Open").clicked();
            });
        });
    });
    opened
}

fn imported_tab(ui: &mut Ui, m: &mut BrowseModel) {
    let recs: Vec<ImportedRecord> = m.imported().to_vec();
    ui.horizontal(|ui| {
        ui.add_enabled_ui(!m.checking() && !recs.is_empty(), |ui| {
            if kit::ghost_button(ui, "Check for updates").clicked() {
                m.check_updates();
            }
        });
        if m.checking() {
            ui.spinner();
        }
    });
    let (mut retry, mut account) = (false, false);
    if let Some(p) = m.check_problem.clone() {
        problem_box(ui, m, &p, &mut retry, &mut account);
    }
    if retry {
        m.check_updates();
    }
    if account {
        m.request_open_account();
    }
    if recs.is_empty() {
        body(ui, "You have not imported anything from the pool yet.");
        para(ui, "Search the pool, open an item and choose Import. The pool is then asked from time to time whether the author has published a newer version.");
        return;
    }
    para(ui, "A newer version is never put in your bank automatically. Choosing Update adds it as a new bank; your own bank is not touched.");
    let mut update: Option<ImportedRecord> = None;
    let mut view: Option<ImportedRecord> = None;
    let mut in_circle: Option<(String, String)> = None;
    for r in recs.iter().rev() {
        kit::card().inner_margin(Margin::same(10)).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.add(egui::Label::new(theme::strong(text::shown(&r.title)).size(14.0)).wrap());
                    line(ui, format!("In the bank \"{}\" - version {} - {} - by {}", text::shown(&r.bank_name), r.version, text::license_name(&r.license), text::author(&r.author)));
                    if let Some(c) = &r.circle {
                        let name = api::plain(&c.circle_name);
                        line(ui, if name.is_empty() { "A copy from one of your circles. Nothing you change here goes back to the circle.".to_string() } else { format!("A copy from the circle \"{name}\". Nothing you change here goes back to the circle.") });
                    }
                    if let Some(n) = &r.newer {
                        let lic = match &n.license {
                            Some(l) if *l != r.license => format!(" under {}", text::license_name(l)),
                            _ => String::new(),
                        };
                        ui.add(egui::Label::new(theme::accent(format!("Version {} is available{lic}.", n.version)).size(12.5)).wrap());
                    }
                    match r.status.as_deref() {
                        Some("under_review") => line(ui, "The pool is reviewing this item. Keep your copy; nothing needs doing."),
                        Some("removed") => line(ui, "The author or the pool removed this item. Your copy is yours to keep."),
                        Some("unavailable") => line(ui, "The pool can not show this item to you now. Your copy is not affected."),
                        _ => {}
                    }
                    if let Some(n) = &r.note {
                        line(ui, api::plain(n));
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(c) = &r.circle {
                        let label = if r.has_update() { "See the new version" } else { "Open in the circle" };
                        if kit::ghost_button(ui, label).clicked() {
                            in_circle = Some((c.circle_id.clone(), r.item_id.clone()));
                        }
                    } else if r.has_update() {
                        if kit::primary_button(ui, "Update...").clicked() {
                            update = Some(r.clone());
                        }
                    } else if kit::ghost_button(ui, "View").clicked() {
                        view = Some(r.clone());
                    }
                });
            });
        });
        ui.add_space(4.0);
    }
    if let Some((cid, bid)) = in_circle {
        super::shared_ui::open_circle(ui.ctx(), &cid, Some(&bid));
    }
    if let Some(r) = update {
        m.start_update(&r);
    } else if let Some(r) = view {
        let id = r.item_id.clone();
        m.open_item(&id, None);
    }
}

// ---- one item ----------------------------------------------------------------------------------------------------------------

fn item_page(ui: &mut Ui, m: &mut BrowseModel) {
    let Some(state) = m.item.as_ref() else {
        m.back_to_list();
        return;
    };
    let (reporting, updating) = (state.report.is_some(), state.update.is_some());
    ui.horizontal(|ui| {
        let label = if reporting { "Back to the item" } else if updating { "Back to my imports" } else { "Back to results" };
        ui.add_enabled_ui(!m.busy_import(), |ui| {
            if kit::ghost_button(ui, label).clicked() {
                if reporting {
                    m.close_report();
                } else {
                    if updating {
                        m.tab = Tab::Imported;
                    }
                    m.back_to_list();
                }
            }
        });
    });
    ui.add_space(6.0);
    if reporting {
        report_page(ui, m);
        return;
    }
    let (mut retry, mut account) = (false, false);
    let load = m.item.as_ref().map(|i| i.load.clone()).unwrap_or(Load::Loading);
    match load {
        Load::Loading => {
            if let Some(s) = m.item.as_ref().and_then(|i| i.summary.clone()) {
                ui.add(egui::Label::new(theme::strong(text::shown(&s.title)).size(16.0)).wrap());
            }
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(theme::muted("Opening the item..."));
            });
        }
        Load::Failed(p) => {
            problem_box(ui, m, &p, &mut retry, &mut account);
        }
        Load::Ready(l) => loaded_page(ui, m, &l),
    }
    if retry {
        m.reload_item();
    }
    if account {
        m.request_open_account();
    }
}

fn loaded_page(ui: &mut Ui, m: &mut BrowseModel, l: &Loaded) {
    let it = &l.doc.head.item;
    ui.add(egui::Label::new(theme::serif(text::shown(&it.title)).size(20.0)).wrap());
    let mut bits = vec![format!("by {}", text::author(&it.author_name)), format!("version {}", it.version), text::count(it.question_count)];
    let d = text::date(it.committed_at);
    if !d.is_empty() {
        bits.push(format!("updated {d}"));
    }
    for p in [api::plain(&it.subject), api::plain(&it.course), super::circles_text::grade_text(&it.grade)] {
        if !p.is_empty() {
            bits.push(p);
        }
    }
    line(ui, bits.join(" - "));
    if it.is_author {
        para(ui, "This is your own item.");
    }

    // Updating: what changed, first.
    if let Some(u) = m.item.as_ref().and_then(|i| i.update.as_ref()) {
        match &u.changes {
            Some(c) => changes_card(ui, c, &u.record),
            None => {
                para(ui, "Working out what changed...");
            }
        }
    }
    prior_imports(ui, m, &it.id);

    // Licence.
    heading(ui, "Licence");
    let lic = text::license_name(&it.license);
    body(ui, &lic);
    if let Some(info) = License::parse(&it.license).map(license_info) {
        para(ui, info.line);
    }
    let mark = l.doc.head.license_check.as_ref().and_then(text::check_marker);
    if mark.is_some() {
        warn_box(ui, &text::check_text(l.doc.head.license_check.as_ref()));
    } else {
        para(ui, &text::check_text(l.doc.head.license_check.as_ref()));
    }
    para(ui, if it.hide_key { text::KEYLESS_LINE } else { "The author shares the answer key with this item." });
    credits(ui, l);

    // Questions.
    heading(ui, "Questions");
    match &l.view {
        Err(e) => warn_box(ui, &format!("The questions could not be shown: {e}")),
        Ok(v) => {
            if !v.is_clean() && v.form == Served::Stripped {
                warn_box(ui, "This item says its answer key is hidden, but the key seems to be in it. It can not be imported; please report it.");
            }
            for q in v.questions.iter().take(100) {
                kit::card().inner_margin(Margin::same(8)).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    let pts = q.points.map(|p| format!(" ({} point{})", p, if p == 1.0 { "" } else { "s" })).unwrap_or_default();
                    ui.add(egui::Label::new(theme::strong(format!("{}.{pts} {}", q.number, text::shown(&q.stem))).size(13.0)).wrap());
                    if q.has_image || q.has_figure {
                        line(ui, "Has a picture. Pictures are copied when you import.");
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

    ui.add_space(8.0);
    import_section(ui, m, l);
}

fn changes_card(ui: &mut Ui, c: &Changes, rec: &ImportedRecord) {
    kit::card().inner_margin(Margin::same(10)).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(theme::strong(format!("What changed: version {} to version {}", c.old_version, c.new_version)).size(13.5));
        if c.nothing_new {
            body(ui, "Nothing in this version is different from what you imported. There is nothing to update.");
            return;
        }
        let mut lines = vec![
            format!("{} question{} new or changed in the new version", c.added, if c.added == 1 { "" } else { "s" }),
            format!("{} question{} of the old version are not in the new one", c.removed, if c.removed == 1 { "" } else { "s" }),
            format!("{} unchanged", c.unchanged),
        ];
        if let Some((a, b)) = &c.license {
            lines.push(format!("The licence changed from {} to {}", text::license_name(a), text::license_name(b)));
        }
        for l in lines {
            ui.label(theme::text(format!("- {l}")).size(12.5));
        }
        if c.bank_missing {
            line(ui, format!("The bank \"{}\" is not there any more; the new version can still be imported as a new bank.", text::shown(&rec.bank_name)));
        } else {
            if c.edited_locally > 0 {
                ui.add(egui::Label::new(theme::accent(format!("You changed {} question{} in your bank since you imported it. Those edits stay in your bank.", c.edited_locally, if c.edited_locally == 1 { "" } else { "s" })).size(12.5)).wrap());
            }
            if c.deleted_locally > 0 {
                line(ui, format!("You deleted {} imported question{} from your bank.", c.deleted_locally, if c.deleted_locally == 1 { "" } else { "s" }));
            }
        }
        para(ui, text::UPDATE_SAFE_LINE);
    });
    ui.add_space(4.0);
}

fn prior_imports(ui: &mut Ui, m: &mut BrowseModel, id: &str) {
    let updating = m.item.as_ref().is_some_and(|i| i.update.is_some());
    if updating {
        return;
    }
    let prior = m.imports_of(id);
    if prior.is_empty() {
        return;
    }
    let mut s = String::from("You already imported this: ");
    s.push_str(&prior.iter().map(|r| format!("version {} into \"{}\" on {}", r.version, text::shown(&r.bank_name), text::date(r.imported_at))).collect::<Vec<_>>().join("; "));
    s.push_str(". Importing again adds another copy; nothing you have is replaced.");
    para(ui, &s);
}

fn credits(ui: &mut Ui, l: &Loaded) {
    let with_chain: Vec<_> = l.doc.head.questions.iter().filter(|q| !q.chain.is_empty()).collect();
    heading(ui, "Where these questions come from");
    if with_chain.is_empty() {
        para(ui, "The pool has no earlier author on record for these questions.");
        return;
    }
    para(ui, &format!("{} of {} questions credit earlier authors. Their licences still apply to your copy.", with_chain.len(), l.doc.head.questions.len().max(with_chain.len())));
    egui::CollapsingHeader::new(theme::text("Show the credits").size(12.5)).id_salt("browse_credits").show(ui, |ui| {
        for q in with_chain {
            ui.label(theme::strong(format!("Question {}", q.pos + 1)).size(12.5));
            for link in &q.chain {
                ui.add(egui::Label::new(theme::text(format!("   - {}", text::chain_line(link))).size(12.5)).wrap());
            }
        }
    });
}

// ---- import --------------------------------------------------------------------------------------------------------------------

/// Set by Import... and Merge...: the panel they open starts below the item's questions, out of
/// sight, so the next frame scrolls it to the top of the view.
fn reveal_import_id() -> egui::Id {
    egui::Id::new("browse_reveal_import")
}

fn import_section(ui: &mut Ui, m: &mut BrowseModel, l: &Loaded) {
    if ui.ctx().data_mut(|d| d.remove_temp::<bool>(reveal_import_id())).unwrap_or(false) {
        ui.scroll_to_cursor(Some(egui::Align::Min));
    }
    if m.item.as_ref().is_some_and(|i| i.merge.is_some()) {
        merge_section(ui, m);
        return;
    }
    let panel = m.item.as_ref().map(|i| i.import.clone()).unwrap_or(ImportPanel::Closed);
    match panel {
        ImportPanel::Closed => {
            if let Some(why) = m.import_blocker() {
                para(ui, &why);
            }
            let own = l.doc.head.item.is_author;
            let updating = m.item.as_ref().is_some_and(|i| i.update.is_some());
            let merge_block = if updating { m.merge_blocker() } else { None };
            if updating && merge_block.is_some() && m.import_blocker().is_none() {
                para(ui, &format!("Merging is not possible here. {}", merge_block.clone().unwrap_or_default()));
            }
            if updating && merge_block.is_none() {
                para(ui, "Add as a new bank is the safe way: your bank is not touched. Merge into this bank works like version control: you review every question first, and nothing is written until you press Apply merge.");
            }
            ui.horizontal_wrapped(|ui| {
                let can = m.import_blocker().is_none();
                ui.add_enabled_ui(can, |ui| {
                    if kit::primary_button(ui, if updating { "Add as a new bank..." } else { "Import..." }).clicked() {
                        m.begin_import();
                        ui.ctx().data_mut(|d| d.insert_temp(reveal_import_id(), true));
                    }
                });
                if updating {
                    ui.add_enabled_ui(merge_block.is_none(), |ui| {
                        if kit::ghost_button(ui, "Merge into this bank...").clicked() {
                            m.begin_merge();
                            ui.ctx().data_mut(|d| d.insert_temp(reveal_import_id(), true));
                        }
                    });
                }
                if !own && kit::ghost_button(ui, "Report...").clicked() {
                    m.open_report();
                }
            });
        }
        ImportPanel::Choosing(c) => choose_panel(ui, m, l, c),
        ImportPanel::Working => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(theme::text("Copying the questions and their pictures..."));
            });
            para(ui, "Please keep this window open until it is done.");
        }
        ImportPanel::Failed(p) => {
            let (mut retry, mut account) = (false, false);
            problem_box(ui, m, &p, &mut retry, &mut account);
            if kit::ghost_button(ui, "Back").clicked() {
                m.cancel_import();
            }
            if retry {
                m.retry_import();
            }
            if account {
                m.request_open_account();
            }
        }
        ImportPanel::Done(d) => done_panel(ui, m, &d),
    }
}

fn choose_panel(ui: &mut Ui, m: &mut BrowseModel, l: &Loaded, c: ImportChoice) {
    let it = &l.doc.head.item;
    let updating = m.item.as_ref().is_some_and(|i| i.update.is_some());
    kit::card().inner_margin(Margin::same(12)).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(theme::strong("Import these questions").size(14.0));
        para(ui, text::IMPORT_COPY_LINE);
        ui.label(theme::strong("Your copy").size(12.5));
        ui.label(theme::text(format!("- Licence: {}. It travels with the copy.", text::license_name(&it.license))).size(12.5));
        ui.label(theme::text(format!("- Credit: by {}. It is written into the bank's details.", text::author(&it.author_name))).size(12.5));
        if it.hide_key {
            ui.add(egui::Label::new(theme::text("- Without the answer key. The questions are marked so, and nothing prints a key for them.").size(12.5)).wrap());
        }
        ui.add_space(6.0);
        if updating {
            para(ui, text::UPDATE_SAFE_LINE);
        } else {
            ui.label(theme::strong("Where should they go?").size(12.5));
            let mut dest = c.dest.clone();
            egui::ScrollArea::vertical().max_height(150.0).id_salt("browse_dest").show(ui, |ui| {
                for b in &m.banks {
                    let sel = dest == Dest::Existing(b.id.clone());
                    if ui.radio(sel, format!("Add to \"{}\" ({} questions)", text::shown(&b.name), b.count)).clicked() {
                        dest = Dest::Existing(b.id.clone());
                    }
                }
                if ui.radio(dest == Dest::New, "Make a new bank").clicked() {
                    dest = Dest::New;
                }
            });
            if dest != c.dest {
                m.set_dest(dest);
            }
        }
        if (updating || c.dest == Dest::New) && let Some(ImportPanel::Choosing(cur)) = m.item.as_ref().map(|i| i.import.clone()) {
            let mut name = cur.new_name.clone();
            ui.label(theme::strong("Name of the new bank").size(12.5));
            ui.add(egui::TextEdit::singleline(&mut name).id(egui::Id::new("browse_new_bank")).desired_width(f32::INFINITY));
            if name != cur.new_name {
                m.set_new_name(&name);
            }
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if kit::primary_button(ui, "Import").clicked() {
                m.confirm_import();
            }
            if kit::ghost_button(ui, "Cancel").clicked() {
                m.cancel_import();
            }
        });
    });
}

fn done_panel(ui: &mut Ui, m: &mut BrowseModel, d: &ImportDone) {
    kit::card().inner_margin(Margin::same(12)).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(theme::strong(format!("Imported {} into \"{}\"", text::count(d.added as u32), text::shown(&d.bank_name))).size(14.5));
        line(ui, if d.new_bank { "A new bank was made. Nothing else in verSHion was changed." } else { "They were added at the end of the bank. Nothing that was there was changed." });
        ui.add_space(4.0);
        ui.label(theme::text(format!("Licence of your copy: {}", text::license_name(&d.license))).size(12.5));
        ui.label(theme::text(format!("Credit that travels with it: by {}, version {}", d.author, d.version)).size(12.5));
        if !d.may_republish_as.is_empty() {
            line(ui, format!("If you share it again you may use: {}.", d.may_republish_as.iter().map(|l| text::license_name(l)).collect::<Vec<_>>().join(", ")));
        } else {
            line(ui, "The pool says you may not publish an unchanged copy of this again.");
        }
        if d.keyless {
            para(ui, text::KEYLESS_LINE);
        }
        if d.pictures > 0 {
            if d.pictures_missing == 0 {
                line(ui, format!("{} picture{} copied.", d.pictures, if d.pictures == 1 { "" } else { "s" }));
            } else {
                warn_box(ui, &format!("{} of {} pictures could not be copied (the pool keeps them for 30 days{}). Those questions came without them.", d.pictures_missing, d.pictures, if d.held_pictures_quota { "; yours did not fit your allowance" } else { "" }));
            }
        }
        if !d.previously.is_empty() {
            line(ui, format!("You had imported version{} {} of this before.", if d.previously.len() == 1 { "" } else { "s" }, d.previously.iter().map(u64::to_string).collect::<Vec<_>>().join(", ")));
        }
        if !d.recorded {
            line(ui, "The pool did not record this as an import, because it is your own item.");
        }
    });
    ui.add_space(6.0);
    if kit::primary_button(ui, "Back to results").clicked() {
        m.tab = if m.item.as_ref().is_some_and(|i| i.update.is_some()) { Tab::Imported } else { Tab::Search };
        m.back_to_list();
    }
}

// ---- report --------------------------------------------------------------------------------------------------------------------

fn report_page(ui: &mut Ui, m: &mut BrowseModel) {
    let (choice, note, state, summary) = match m.item.as_ref() {
        Some(i) => match &i.report {
            Some(r) => (r.choice, r.note.clone(), r.state.clone(), i.summary.clone()),
            None => return,
        },
        None => return,
    };
    ui.label(theme::serif("Report this item").size(20.0));
    if let Some(s) = summary {
        line(ui, text::shown(&s.title));
    }
    match state {
        ReportState::Sent(msg) => {
            body(ui, &msg);
            ui.add_space(6.0);
            if kit::primary_button(ui, "Back to the item").clicked() {
                m.close_report();
            }
        }
        state => {
            let sending = matches!(state, ReportState::Sending);
            para(ui, "What is wrong with it? The pool's reviewers read every report.");
            let mut pick = choice;
            ui.add_enabled_ui(!sending, |ui| {
                for c in ReportChoice::ALL.into_iter().filter(|c| *c != ReportChoice::KeyLeak || m.key_leak_offered()) {
                    let (name, help) = text::report_reason(c);
                    // Each reason's line sits under its own button, not halfway to the next one.
                    ui.scope(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        if ui.radio(pick == Some(c), name).clicked() {
                            pick = Some(c);
                        }
                        ui.label(theme::muted(format!("      {help}")).size(11.5));
                    });
                    ui.add_space(4.0);
                }
            });
            if pick != choice && let Some(c) = pick {
                m.set_report_choice(c);
            }
            let mut n = note.clone();
            ui.label(theme::strong("Note (optional, except for Something else)").size(12.5));
            ui.add_enabled(!sending, egui::TextEdit::multiline(&mut n).id(egui::Id::new("browse_report_note")).desired_rows(3).desired_width(f32::INFINITY));
            if n != note {
                m.set_report_note(&n);
            }
            para(ui, "Do not put students' names or other personal details in the note.");
            if let ReportState::Failed(p) = &state {
                let (mut retry, mut account) = (false, false);
                problem_box(ui, m, p, &mut retry, &mut account);
                if account {
                    m.request_open_account();
                }
            }
            if let Some(why) = m.report_blocker() {
                para(ui, why);
            }
            ui.horizontal(|ui| {
                ui.add_enabled_ui(!sending && m.report_blocker().is_none(), |ui| {
                    if kit::primary_button(ui, "Send report").clicked() {
                        m.send_report();
                    }
                });
                if sending {
                    ui.spinner();
                }
                if kit::ghost_button(ui, "Cancel").clicked() {
                    m.close_report();
                }
            });
        }
    }
}


// ---- merge -----------------------------------------------------------------------------------------------------------------------

fn kind_label(k: RowKind) -> &'static str {
    match k {
        RowKind::Unchanged => "Unchanged",
        RowKind::TheirsChanged => "Changed in the pool",
        RowKind::OursChanged => "Changed by you",
        RowKind::Conflict => "CONFLICT: both changed",
        RowKind::NewInTheirs => "New in the pool",
        RowKind::RemovedInTheirs => "Removed from the pool",
        RowKind::RemovedLocally => "Deleted by you",
    }
}

fn choice_label(row: &Row, c: Choice) -> &'static str {
    match (row.kind, c) {
        (RowKind::TheirsChanged, Choice::Take) if row.carry_key => "Take the pool's wording and keep my answer key",
        (RowKind::TheirsChanged, Choice::Take) => "Take the pool's version",
        (RowKind::TheirsChanged, _) => "Keep mine",
        (RowKind::Conflict, Choice::Ours) => "Keep mine",
        (RowKind::Conflict, Choice::Theirs) if row.carry_key => "Use the pool's version, keeping my answer key",
        (RowKind::Conflict, Choice::Theirs) => "Use the pool's version",
        (RowKind::Conflict, _) => "Keep both (the pool's is added as a second question, marked as from the pool)",
        (RowKind::NewInTheirs, Choice::Add) => "Add it to my bank",
        (RowKind::NewInTheirs, _) => "Do not add it",
        (RowKind::RemovedInTheirs, Choice::Keep) => "Keep it (it will no longer be linked to the pool)",
        (RowKind::RemovedInTheirs, _) => "Remove it from my bank",
        _ => "",
    }
}

fn merge_section(ui: &mut Ui, m: &mut BrowseModel) {
    let Some(ms) = m.item.as_ref().and_then(|i| i.merge.clone()) else { return };
    match ms.phase {
        MergePhase::Wanted => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(theme::muted("Preparing the review..."));
            });
        }
        MergePhase::Blocked(why) => {
            warn_box(ui, &why);
            if kit::ghost_button(ui, "Back").clicked() {
                m.cancel_merge();
            }
        }
        MergePhase::Applying => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(theme::text("Fetching the pool's version and its pictures..."));
            });
            para(ui, "Your bank is changed only when this has arrived and a copy of your banks has been saved. If it fails, nothing changes.");
            if kit::ghost_button(ui, "Cancel").clicked() {
                m.cancel_merge();
            }
        }
        MergePhase::Done(d) => merge_done(ui, m, &d),
        MergePhase::Review => {
            let Some(plan) = ms.plan.clone() else { return };
            review_page(ui, m, &plan, ms.problem.as_ref());
        }
    }
}

fn review_page(ui: &mut Ui, m: &mut BrowseModel, plan: &MergePlan, problem: Option<&Problem>) {
    ui.label(theme::serif(format!("Merge version {} into \"{}\"", plan.new_version, text::shown(&plan.bank_name))).size(18.0));
    para(ui, &format!("You have version {}. Nothing has been changed yet. Go through the questions below, then press Apply merge. Cancel leaves everything as it is. A copy of your banks is saved just before anything is written.", plan.old_version));
    let counts = [RowKind::Conflict, RowKind::TheirsChanged, RowKind::OursChanged, RowKind::NewInTheirs, RowKind::RemovedInTheirs, RowKind::RemovedLocally, RowKind::Unchanged];
    let summary: Vec<String> = counts.iter().filter(|k| plan.count(**k) > 0).map(|k| format!("{} {}", plan.count(*k), kind_label(*k).to_lowercase())).collect();
    line(ui, summary.join(" - "));
    if plan.unlinked > 0 {
        para(ui, &format!("{} question{} in this bank {} not linked to the pool (you added {} or copied {} in), so the merge leaves {} alone.", plan.unlinked, if plan.unlinked == 1 { "" } else { "s" }, if plan.unlinked == 1 { "is" } else { "are" }, if plan.unlinked == 1 { "it" } else { "them" }, if plan.unlinked == 1 { "it" } else { "them" }, if plan.unlinked == 1 { "it" } else { "them" }));
    }
    if let Some((a, b)) = &plan.license {
        kit::card().inner_margin(Margin::same(10)).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(theme::strong("The licence changed").size(13.0));
            ui.label(theme::text(format!("Before: {}", text::license_name(a))).size(12.5));
            ui.label(theme::text(format!("Now: {}", text::license_name(b))).size(12.5));
            line(ui, "The credit line in the bank's description is updated when you apply the merge, not before.");
            ui.add(egui::Label::new(theme::muted(api::plain(&plan.new_credit)).size(12.0)).wrap());
        });
        ui.add_space(4.0);
    }
    if let Some(p) = problem {
        warn_box(ui, &p.message);
    }
    let mut pick: Option<(usize, Choice)> = None;
    let mut order: Vec<usize> = (0..plan.rows.len()).filter(|i| plan.rows[*i].kind != RowKind::Unchanged).collect();
    order.sort_by_key(|i| match plan.rows[*i].kind {
        RowKind::Conflict => 0,
        RowKind::TheirsChanged => 1,
        RowKind::OursChanged => 2,
        RowKind::NewInTheirs => 3,
        RowKind::RemovedInTheirs => 4,
        _ => 5,
    });
    for i in order {
        let row = &plan.rows[i];
        kit::card().inner_margin(Margin::same(10)).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                let (fill, fg) = if row.kind == RowKind::Conflict { (theme::error_wash(), theme::bad()) } else { (theme::accent_faded(), theme::text_col()) };
                kit::pill(ui, kind_label(row.kind), fill, fg);
                ui.add(egui::Label::new(theme::strong(&row.title).size(13.0)).wrap());
            });
            if let Some(n) = &row.note {
                line(ui, api::plain(n));
            }
            for d in &row.diffs {
                ui.label(theme::strong(d.field).size(12.0));
                for (who, v) in [("When imported", &d.base), ("Yours now", &d.ours), ("The pool's new version", &d.theirs)] {
                    ui.add(egui::Label::new(theme::text(format!("   {who}: {}", api::plain(v))).size(12.0)).wrap());
                }
            }
            for c in &row.options {
                if ui.radio(row.choice == *c, choice_label(row, *c)).clicked() {
                    pick = Some((i, *c));
                }
            }
            if row.needs_decision() {
                ui.add(egui::Label::new(theme::accent("Choose one to continue.").size(12.0)).wrap());
            }
        });
        ui.add_space(4.0);
    }
    let unchanged: Vec<&Row> = plan.rows.iter().filter(|r| r.kind == RowKind::Unchanged).collect();
    if !unchanged.is_empty() {
        egui::CollapsingHeader::new(theme::muted(format!("{} unchanged", unchanged.len())).size(12.5)).id_salt("merge_unchanged").show(ui, |ui| {
            for r in unchanged {
                ui.label(theme::muted(&r.title).size(12.0));
            }
        });
    }
    if let Some((i, c)) = pick {
        m.set_merge_choice(i, c);
    }
    ui.add_space(8.0);
    let undecided = plan.undecided();
    if undecided > 0 {
        para(ui, &format!("{} conflict{} still need{} a decision before the merge can be applied.", undecided, if undecided == 1 { "" } else { "s" }, if undecided == 1 { "s" } else { "" }));
    }
    ui.horizontal(|ui| {
        ui.add_enabled_ui(m.can_apply_merge(), |ui| {
            if kit::primary_button(ui, "Apply merge").clicked() {
                m.apply_merge();
            }
        });
        if kit::ghost_button(ui, "Cancel").clicked() {
            m.cancel_merge();
        }
    });
}

fn merge_done(ui: &mut Ui, m: &mut BrowseModel, d: &MergeDone) {
    kit::card().inner_margin(Margin::same(12)).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(theme::strong(format!("Version {} was merged into \"{}\"", d.new_version, text::shown(&d.bank_name))).size(14.5));
        let o = &d.outcome;
        line(ui, format!("{} taken from the pool, {} kept as yours, {} added, {} removed, {} kept as a second copy.", o.taken, o.kept, o.added, o.removed, o.both));
        if d.license_changed {
            line(ui, "The bank's credit line and licence note were updated.");
        }
        ui.add_space(4.0);
        ui.label(theme::strong("Your way back").size(12.5));
        ui.add(egui::Label::new(theme::text(format!("A copy of all your banks from just before the merge is saved at {}", api::plain(&d.restore))).size(12.0)).wrap());
        line(ui, "To go back, close verSHion and copy that file over banks.json in the same folder.");
    });
    ui.add_space(6.0);
    if kit::primary_button(ui, "Back to my imports").clicked() {
        m.cancel_merge();
        m.tab = Tab::Imported;
        m.back_to_list();
    }
}
