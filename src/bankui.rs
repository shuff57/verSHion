//! Question Banks screens (V4): the bank list and the bank editor. The editor is a
//! grid of question cards; clicking a card expands it in place into the two-column
//! editor from `qedit` (one open at a time: the open card is `App.qedit`'s draft).
//! Also hosts the shared `fmt_date` / kind-chip helpers other modules import.

use egui::{
    Align, Button, Color32, ComboBox, CornerRadius, Frame, Layout, Margin, Pos2, Rect, RichText,
    Sense, Stroke, TextEdit, Ui, Vec2,
};
// egui's widget id type vs the store's bank/question id (a String).
use egui::Id as UiId;

use crate::kit::{self, ViewMode};
use crate::model::{Difficulty, Id, QKind, Question};
use crate::screens::now_secs;
use crate::{qedit, screens, theme, time, App, Screen};

// ---------- shared helpers (pub: other workers may use these) ----------

/// Formats a unix-seconds timestamp as a short date, "Oct 15, 2026".
pub fn fmt_date(epoch: i64) -> String {
    if epoch <= 0 {
        return "—".to_string();
    }
    let (y, m, d) = time::civil_from_days(epoch.div_euclid(86_400));
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!("{} {}, {}", MONTHS[(m - 1) as usize], d, y)
}

/// "just now", "2 hours ago", ... falling back to a date after a month.
pub fn rel_time(epoch: i64) -> String {
    if epoch <= 0 {
        return "—".to_string();
    }
    let d = (now_secs() - epoch).max(0);
    let plural = |n: i64, unit: &str| format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" });
    if d < 60 {
        "just now".to_string()
    } else if d < 3600 {
        plural(d / 60, "minute")
    } else if d < 86_400 {
        plural(d / 3600, "hour")
    } else if d < 30 * 86_400 {
        plural(d / 86_400, "day")
    } else {
        fmt_date(epoch)
    }
}

/// Chip color for a question type.
pub fn kind_chip_color(kind: QKind) -> Color32 {
    match kind {
        QKind::Mc => Color32::from_rgb(0x35, 0x60, 0x7c),
        QKind::Tf => Color32::from_rgb(0x3e, 0x6b, 0x48),
        QKind::Matching => Color32::from_rgb(0x6b, 0x53, 0x79),
        QKind::Blank => Color32::from_rgb(0xb5, 0x8a, 0x3d),
        QKind::Short => Color32::from_rgb(0x7c, 0x5b, 0x3a),
        QKind::Multipart => Color32::from_rgb(0x8c, 0x44, 0x44),
    }
}

/// Short chip label for a question type.
pub fn kind_chip_label(kind: QKind) -> &'static str {
    match kind {
        QKind::Mc => "MC",
        QKind::Tf => "T/F",
        QKind::Matching => "Match",
        QKind::Blank => "Blank",
        QKind::Short => "Short",
        QKind::Multipart => "Multi",
    }
}

/// Loads (and caches in `App.textures`) the egui texture for a stored image.
pub fn texture_for(
    app: &mut App,
    ctx: &egui::Context,
    img: &crate::model::ImageRef,
) -> Option<egui::TextureHandle> {
    app.image_texture(ctx, &img.file)
}

fn diff_label(difficulty: Difficulty) -> &'static str {
    match difficulty {
        Difficulty::Low => "Easy",
        Difficulty::Medium => "Medium",
        Difficulty::High => "Hard",
    }
}

/// Points on human form ("2 pt", "2.5 pt"); None when the question has none.
pub fn points_label(points: Option<f64>) -> Option<String> {
    points.map(|p| {
        if p.fract() == 0.0 {
            format!("{} pt", p as i64)
        } else {
            format!("{p} pt")
        }
    })
}

/// Reads/edits per-frame UI state kept in egui context memory.
fn with_state<T, R>(ctx: &egui::Context, key: UiId, f: impl FnOnce(&mut T) -> R) -> R
where
    T: Default + Clone + Send + Sync + 'static,
{
    ctx.data_mut(|d| f(d.get_temp_mut_or_default::<T>(key)))
}

fn view_of(list: bool) -> ViewMode {
    if list { ViewMode::List } else { ViewMode::Cards }
}

fn save_settings(app: &App) {
    let _ = crate::store::save_json("settings.json", &app.settings);
}

/// The Banks screen's widest content: past this the tiles would be too wide to scan.
const BANKS_MAX_W: f32 = 1600.0;
/// The narrowest a Banks tile gets before the grid drops a column (inner 248: two lines of key
/// for five types, the footer's long form).
pub(crate) const BANK_TILE_MIN_W: f32 = 280.0;

/// Columns of the Banks tile grid in `avail` points with `gap` between tiles: as many tiles of at
/// least [`BANK_TILE_MIN_W`] as fit (at least one); the tiles then share the width equally.
fn banks_grid_cols(avail: f32, gap: f32) -> usize {
    (((avail + gap) / (BANK_TILE_MIN_W + gap)).floor() as usize).max(1)
}

/// Draws a clickable crumb "Banks" that returns to the list.
fn crumb_row(ui: &mut Ui, app: &mut App, tail: Option<&str>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let b = ui.add(
            Button::new(kit::meta_muted("Banks")).frame(false),
        );
        if tail.is_some() && b.clicked() {
            app.screen = Screen::Banks;
            app.bank = None;
            app.qedit = None;
        }
        if let Some(t) = tail {
            ui.label(kit::meta_muted("/"));
            // A long name is shortened to the row, never widening the page.
            ui.add(egui::Label::new(kit::meta_muted(t)).truncate());
        }
    });
}

/// The shared search box ([`kit::search_field`]); returns true when text changed.
fn search_pill(ui: &mut Ui, text: &mut String, w: f32, hint: &str) -> bool {
    kit::search_field(ui, text, w, hint)
}

// ---------- transient per-screen state ----------

/// State of the Question Banks list screen.
#[derive(Clone, Default)]
struct BanksState {
    search: String,
    /// Bank id awaiting delete confirmation.
    confirm_delete: Option<Id>,
    /// Bank id being renamed + the current name text.
    rename: Option<(Id, String)>,
    /// Archived banks are listed too (marked), so they can be restored or deleted.
    show_archived: bool,
}

/// Sort choices for the question card list.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub(crate) enum Sort {
    #[default]
    Newest,
    Oldest,
    Kind,
    Difficulty,
    Topic,
}

impl Sort {
    fn all() -> [Sort; 5] {
        [Sort::Newest, Sort::Oldest, Sort::Kind, Sort::Difficulty, Sort::Topic]
    }
    fn label(self) -> &'static str {
        match self {
            Sort::Newest => "Newest",
            Sort::Oldest => "Oldest",
            Sort::Kind => "Question type",
            Sort::Difficulty => "Difficulty",
            Sort::Topic => "Topic",
        }
    }
}

/// State of the bank editor screen.
#[derive(Clone, Default)]
struct BankEditState {
    search: String,
    /// Topic the card list is filtered to (None = all).
    topic: Option<String>,
    /// Difficulty filter (None = all).
    diff: Option<Difficulty>,
    /// Type filter chip (None = All).
    kind: Option<QKind>,
    sort: Sort,
    /// Last deleted question + its index, for the Undo strip.
    last_deleted: Option<(Question, usize)>,
    /// Open card seen last frame (scroll into view when it changes).
    last_open: Option<String>,
    /// Topic rename / merge / delete dialog.
    topic_edit: Option<TopicEdit>,
    /// "Question Bank details" dialog draft (None = closed).
    details: Option<crate::model::BankDetails>,
    /// "Tidy stems" dialog (None = closed).
    tidy: Option<TidyDialog>,
    /// The last applied tidy, for its Undo strip.
    last_tidy: Option<TidyUndo>,
}

/// The "Tidy stems" preview: every question tidying would change, and which are ticked.
#[derive(Clone)]
struct TidyDialog {
    items: Vec<crate::tidy::QuestionTidy>,
    picked: Vec<bool>,
}

/// Questions as they were before the last tidy, and where the backup went.
#[derive(Clone)]
struct TidyUndo {
    before: Vec<Question>,
    backup: String,
}

/// Opens the "Tidy stems" dialog on the bank's current questions.
fn open_tidy(app: &App, ctx: &egui::Context, key: UiId, bank_id: &Id) {
    let items = app.store.bank(bank_id).map(|b| crate::tidy::tidy_bank(&b.questions)).unwrap_or_default();
    let picked = vec![true; items.len()];
    with_state(ctx, key, |s: &mut BankEditState| s.tidy = Some(TidyDialog { items, picked }));
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum TopicMode {
    Rename,
    Merge,
    Delete,
}

/// The topic dialog: which topic, what to do, the typed / chosen target.
#[derive(Clone, Debug)]
struct TopicEdit {
    from: String,
    mode: TopicMode,
    text: String,
}

/// "Used in N exams" list dialog: title + exam ids (shared by banks list and editor).
#[derive(Clone, Default)]
struct UsageDialog {
    open: Option<(String, Vec<Id>)>,
}

fn usage_key() -> UiId {
    UiId::new("bank_usage_dialog")
}

pub(crate) fn open_usage(ctx: &egui::Context, title: String, exams: Vec<Id>) {
    with_state(ctx, usage_key(), |s: &mut UsageDialog| s.open = Some((title, exams)));
}

/// Lists the exams that use a bank / question; a row opens that exam.
pub(crate) fn draw_usage_dialog(app: &mut App, ctx: &egui::Context) {
    let Some((title, ids)) = with_state(ctx, usage_key(), |s: &mut UsageDialog| s.open.clone()) else {
        return;
    };
    let mut close = false;
    let mut go: Option<Id> = None;
    let modal = egui::Modal::new(UiId::new("bank_usage_modal")).show(ctx, |ui| {
        ui.set_min_width(380.0);
        ui.set_max_width(460.0);
        ui.label(theme::serif(title.clone()).size(20.0));
        ui.add_space(2.0);
        ui.label(theme::muted(crate::bankops::usage_label(ids.len())).size(12.5));
        ui.add_space(8.0);
        if ids.is_empty() {
            ui.label(theme::muted("No exam places these questions yet. Add them from an exam's question shelf.").size(12.5));
        }
        for id in &ids {
            let Some(e) = app.store.exam(id) else { continue };
            let resp = kit::card().inner_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(theme::strong(truncate(&e.name, 44)));
                        ui.label(
                            theme::muted(format!(
                                "{} question{} · edited {}",
                                e.items.len(),
                                if e.items.len() == 1 { "" } else { "s" },
                                rel_time(e.modified)
                            ))
                            .size(11.5),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(theme::accent("Open").size(12.5));
                    });
                });
            });
            let r = ui.interact(resp.response.rect, ui.id().with(("usage_row", id)), Sense::click());
            if r.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if r.clicked() {
                go = Some(id.clone());
            }
            ui.add_space(6.0);
        }
        ui.add_space(6.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if crate::pageshell::outline(ui, "Close").clicked() {
                close = true;
            }
        });
    });
    if let Some(id) = &go {
        app.qedit = None;
        app.open_exam(id);
    }
    if close || go.is_some() || modal.should_close() {
        with_state(ctx, usage_key(), |s: &mut UsageDialog| s.open = None);
    }
}

/// Accent "Used in N exams" link text; returns its response (hover sense only: the
/// caller's card owns the click and checks the pointer against this rect).
fn usage_link(ui: &mut Ui, n: usize, size: f32) -> egui::Response {
    let txt = crate::bankops::usage_label(n);
    let rt = if n > 0 { RichText::new(txt).size(size).color(theme::accent_col()).underline() } else { theme::muted(txt).size(size) };
    let r = ui.add(egui::Label::new(rt).extend().sense(Sense::hover()));
    if n > 0 && r.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    r
}

// ---------- screen 1: the Question Banks list ----------

/// One bank as its tile shows it, on Banks and on Home (spec section 3).
pub(crate) struct BankTile {
    pub id: Id,
    pub name: String,
    pub questions: usize,
    pub topics: usize,
    pub mix: [usize; 6],
    /// Exams placing at least one of its questions.
    pub used_in: Vec<Id>,
    pub edited: i64,
    pub archived: bool,
}

impl BankTile {
    /// The tile's facts for `b`, read from the store.
    pub(crate) fn of(store: &crate::store::Store, b: &crate::model::QuestionBank) -> BankTile {
        let mut mix = [0usize; 6];
        for q in &b.questions {
            mix[mix_index(q.kind.qkind())] += 1;
        }
        BankTile {
            id: b.id.clone(),
            name: b.name.clone(),
            questions: b.questions.len(),
            topics: unique_topics(&b.questions).len(),
            mix,
            used_in: crate::bankops::exams_using_bank(&store.exams, &b.id).iter().map(|e| e.id.clone()).collect(),
            edited: b.modified,
            archived: store.is_archived(&b.id),
        }
    }
}

pub(crate) fn mix_index(k: QKind) -> usize {
    match k {
        QKind::Mc => 0,
        QKind::Tf => 1,
        QKind::Multipart => 2,
        QKind::Matching => 3,
        QKind::Blank => 4,
        QKind::Short => 5,
    }
}

fn unique_topics(qs: &[Question]) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for q in qs {
        for t in &q.topics {
            let t = t.trim();
            if !t.is_empty() && !v.iter().any(|x| x == t) {
                v.push(t.to_string());
            }
        }
    }
    v.sort();
    v
}

impl App {
    pub fn draw_banks(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let key = UiId::new("testparrot_banks_state");
        screens::desk_panel(ui, 20, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("banks_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.scope(|ui| {
                        // The grid page uses the window's width (to a wide cap), so the tiles reach
                        // the right edge rather than stopping at the reading width of a text page.
                        ui.set_max_width(ui.available_width().min(BANKS_MAX_W));
                        self.banks_body(ui, &ctx, key);
                    });
                });
        });
        draw_bank_overlays(self, &ctx, key);
        draw_usage_dialog(self, &ctx);
    }

    fn banks_body(&mut self, ui: &mut Ui, ctx: &egui::Context, key: UiId) {
        let total_q: usize = self.store.banks.iter().map(|b| b.questions.len()).sum();
        let sub = format!(
            "{} bank{} · {} question{}",
            self.store.banks.len(),
            if self.store.banks.len() == 1 { "" } else { "s" },
            total_q,
            if total_q == 1 { "" } else { "s" }
        );
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                kit::page_header(ui, &["Home", "Banks"], "Question banks", Some(&sub));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if crate::pageshell::primary(ui, "New bank").clicked() {
                    let id = self.new_bank("Untitled Question Bank");
                    self.open_bank(&id);
                }
                if crate::pageshell::outline(ui, "Import").clicked() {
                    self.screen = crate::Screen::Imports;
                }
            });
        });
        ui.add_space(12.0);

        // Toolbar.
        let mut search = with_state(ctx, key, |s: &mut BanksState| s.search.clone());
        let mut list = self.settings.banks_list_view;
        ui.horizontal(|ui| {
            if search_pill(ui, &mut search, 300.0, "Search banks") {
                with_state(ctx, key, |s: &mut BanksState| s.search = search.clone());
            }
            let archived_n = self.store.banks.iter().filter(|b| self.store.is_archived(&b.id)).count();
            if archived_n > 0 {
                let on = with_state(ctx, key, |s: &mut BanksState| s.show_archived);
                if kit::chip_toggle(ui, &format!("Show archived  {archived_n}"), on).clicked() {
                    with_state(ctx, key, |s: &mut BanksState| s.show_archived = !on);
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let mut mode = view_of(list);
                if kit::view_toggle(ui, &mut mode, "banks_list") {
                    list = mode == ViewMode::List;
                }
            });
        });
        if list != self.settings.banks_list_view {
            self.settings.banks_list_view = list;
            save_settings(self);
        }
        ui.add_space(12.0);
        crate::share::bank_links_ui::adding_rows(ui, self);

        if self.store.banks.is_empty() {
            let pressed = crate::pageshell::empty_state(
                ui,
                crate::handicons::BANKS,
                "Your first Question Bank",
                "A bank is a shelf of questions you can reuse in any exam. Start one by hand or hand a test you already have to the importer.",
                &[("New bank", true), ("Import a test", false)],
            );
            match pressed {
                Some(0) => {
                    let id = self.new_bank("Untitled Question Bank");
                    self.open_bank(&id);
                }
                Some(_) => self.screen = crate::Screen::Imports,
                None => {}
            }
            return;
        }

        let query = search.trim().to_lowercase();
        let show_archived = with_state(ctx, key, |s: &mut BanksState| s.show_archived);
        let rows: Vec<BankTile> = self
            .store
            .banks
            .iter()
            .filter(|b| show_archived || !self.store.is_archived(&b.id))
            .filter(|b| query.is_empty() || b.name.to_lowercase().contains(&query))
            .map(|b| BankTile::of(&self.store, b))
            .collect();
        if rows.is_empty() {
            if query.is_empty() {
                let r = crate::pageshell::empty_line(ui, crate::handicons::BANKS, "Every bank is archived.", Some("Show archived"));
                if r.is_some() {
                    with_state(ctx, key, |s: &mut BanksState| s.show_archived = true);
                }
            } else {
                let r = crate::pageshell::empty_line(ui, crate::handicons::BANKS, &format!("No banks match \"{search}\"."), Some("Clear search"));
                if r.is_some() {
                    with_state(ctx, key, |s: &mut BanksState| s.search.clear());
                }
            }
            crate::pageshell::end_pad(ui);
            return;
        }

        if list {
            // Header row.
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                kit::label(ui, "Bank");
            });
            ui.add_space(2.0);
            for r in &rows {
                bank_list_row(ui, self, ctx, key, r);
                ui.add_space(6.0);
            }
        } else {
            let avail = ui.available_width();
            let gap = kit::GUTTER;
            let cols = banks_grid_cols(avail, gap);
            // Fewer banks than columns still fills the row: one bank is a wide card, not a
            // quarter-width tile with three quarters of empty page beside it.
            let fit = rows.len().clamp(1, cols);
            // Each column on whole pixels, the last ending on the page's right edge (under the view
            // toggle and New bank).
            let ppp = ui.ctx().pixels_per_point();
            let cols_x = grid_columns(ui.cursor().left(), avail, fit, gap, ppp);
            // Every tile has the same slots, so every tile is BANK_TILE_H and a row ends on one line.
            for chunk in rows.chunks(cols) {
                let top = (ui.cursor().top() * ppp).round() / ppp;
                for (r, &(l, rt)) in chunk.iter().zip(&cols_x) {
                    let rect = Rect::from_min_max(Pos2::new(l, top), Pos2::new(rt, top + BANK_TILE_H));
                    let mut cell = ui.new_child(egui::UiBuilder::new().max_rect(rect).id_salt(("bank_tile_cell", &r.id)));
                    match bank_tile(&mut cell, self, rect.width(), r, TileVariant::Banks) {
                        Some(TileAct::Menu(a)) => apply_bank_menu(self, ctx, key, r, Some(a)),
                        Some(TileAct::Usage) => open_usage(ctx, r.name.clone(), r.used_in.clone()),
                        Some(TileAct::Open | TileAct::AddQuestions) => self.open_bank(&r.id),
                        None => {}
                    }
                }
                ui.advance_cursor_after_rect(Rect::from_min_size(Pos2::new(ui.cursor().left(), top), Vec2::new(avail, BANK_TILE_H)));
                ui.add_space(gap);
            }
        }
        crate::pageshell::end_pad(ui);
    }

    /// The bank editor: breadcrumb, serif editable name, subtitle, toolbar (search,
    /// type chips, topic chips, sort/difficulty, view toggle) and a grid of question
    /// cards that expand in place into the editor.
    pub fn draw_bank_edit(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let Some(bank_id) = self.bank.clone().or_else(|| match &self.screen {
            Screen::BankEdit(id) => Some(id.clone()),
            _ => None,
        }) else {
            return;
        };
        if self.store.bank(&bank_id).is_none() {
            self.screen = Screen::Banks;
            self.bank = None;
            self.qedit = None;
            return;
        }
        // A draft belonging to another bank never carries over.
        if self.qedit.as_ref().is_some_and(|d| d.bank_id != bank_id) {
            self.qedit = None;
        }

        let key = UiId::new("testparrot_bank_edit_state").with(&bank_id);
        // Headless test hook: VERSHION_OPEN_Q=<n> opens the n-th question once.
        if let Ok(v) = crate::env_var("VERSHION_OPEN_Q") {
            let done = ctx.data_mut(|d| d.get_temp::<bool>(UiId::new("qopen_hook")).unwrap_or(false));
            if !done {
                ctx.data_mut(|d| d.insert_temp(UiId::new("qopen_hook"), true));
                let n: usize = v.parse().unwrap_or(1);
                let q = self.store.bank(&bank_id).and_then(|b| b.questions.get(n.saturating_sub(1)).cloned());
                if let Some(q) = q {
                    self.qedit = Some(qedit::Draft::from_question(bank_id.clone(), &q));
                }
            }
        }
        screens::desk_panel(ui, 20, |ui| {
            // Whether the page scrolled last frame: then its content keeps clear of the bar.
            let scrolls_key = key.with("bank_page_scrolls");
            let scrolls = ctx.data(|d| d.get_temp::<bool>(scrolls_key)).unwrap_or(false);
            let out = egui::ScrollArea::vertical()
                .id_salt("bank_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.scope(|ui| {
                        // The same width as the Banks grid and the other pages, so the header's
                        // actions, the empty card and the list end on the page's right margin;
                        // less a gutter for the scroll bar while the page scrolls.
                        let w = bank_page_w(ui.available_width(), scrolls.then(|| scroll_bar_reach(ui)));
                        ui.set_max_width(w);
                        self.bank_edit_body(ui, &ctx, key, &bank_id);
                        crate::pageshell::end_pad(ui);
                    });
                });
            let now = out.content_size.y > out.inner_rect.height() + 0.5;
            if now != scrolls {
                ctx.data_mut(|d| d.insert_temp(scrolls_key, now));
                ctx.request_repaint();
            }
        });

        // Insert-figure modal stacks on top of the open editor.
        self.draw_figure_picker(&ctx);
        draw_topic_dialog(self, &ctx, key, &bank_id);
        draw_details_dialog(self, &ctx, key, &bank_id);
        // Headless/QA hook: VERSHION_TIDY=1 opens the bank's "Tidy stems" dialog once.
        if crate::env_var("VERSHION_TIDY").is_ok() {
            let hid = UiId::new("qs_tidy_hook");
            if !ctx.data(|d| d.get_temp::<bool>(hid).unwrap_or(false)) {
                ctx.data_mut(|d| d.insert_temp(hid, true));
                open_tidy(self, &ctx, key, &bank_id);
            }
        }
        draw_tidy_dialog(self, &ctx, key, &bank_id);
        draw_usage_dialog(self, &ctx);
        // Headless/QA hook: VERSHION_IMPORT_MODAL=1 opens the bank's Import modal once.
        if crate::env_var("VERSHION_IMPORT_MODAL").is_ok() {
            let hid = UiId::new("qs_import_modal_hook");
            if !ctx.data(|d| d.get_temp::<bool>(hid).unwrap_or(false)) {
                ctx.data_mut(|d| d.insert_temp(hid, true));
                crate::importui::open_import_modal(&ctx, &bank_id);
            }
        }
        self.draw_import_modal(&ctx);
        self.draw_resolve_images(&ctx, &bank_id);
        draw_bank_export(self, &ctx);
    }

    /// The bank's actions. `ltr`: drawn left to right in a wrapping row of their own (a narrow
    /// window), each moving to the next line whole; otherwise added to a right-to-left row beside
    /// the name. The pool's three actions share one "Share" menu. Returns the actions' width
    /// with all of them on one line.
    fn bank_toolbar(&mut self, ui: &mut Ui, ctx: &egui::Context, key: UiId, bank_id: &Id, questions: &[Question], ltr: bool) -> f32 {
        #[derive(Clone, Copy)]
        enum Tool {
            More,
            Details,
            Popover,
            Import,
            Share,
            Export,
            Add,
        }
        let update = self.browse.has_update(bank_id);
        let share_label = if update {
            format!("Share (update) {}", egui_phosphor::regular::CARET_DOWN)
        } else {
            format!("Share {}", egui_phosphor::regular::CARET_DOWN)
        };
        // Read left to right by rank: the bank's main action first, then the outlined actions
        // that bring questions in or send them out, then the quiet ones that only show the
        // bank (its details, its own window) and the overflow, as one segmented group.
        // An empty bank's card below has its own filled Add question with the type menu, the
        // page's one primary; the toolbar's would only repeat it, so it is hidden until the first
        // question is in (judged on the whole bank, never on a filter).
        // Import likewise: the empty card offers Import questions beside its Add question.
        // An empty bank shows only what works on it: Share and Preview have nothing to act on
        // yet, so the header keeps just the "…" menu (Details and From pool… inside it) and the
        // empty card's Add question / Import questions lead. They appear with the first question.
        let empty = questions.is_empty();
        let mut tools = if empty { vec![] } else { vec![Tool::Add, Tool::Import, Tool::Share, Tool::Export, Tool::Details, Tool::Popover] };
        tools.push(Tool::More);
        if !ltr {
            tools.reverse();
        }
        let gap = ui.spacing().item_spacing.x;
        // Details, Preview and the overflow are one segmented group: one outlined container on the
        // paper fill (quieter than Share's card-white box and bold label), hairlines between.
        let quiet = |t: Tool| matches!(t, Tool::Details | Tool::Popover | Tool::More);
        let mut group = Rect::NOTHING;
        let mut seams: Vec<f32> = Vec::new();
        // The container is painted under the segments once their rects are known.
        let group_bg = ui.painter().add(egui::Shape::Noop);
        let mut width = 0.0;
        let mut last_gap = 0.0;
        let mut first = true;
        for (i, &t) in tools.iter().enumerate() {
            // The space after this tool: none inside the segmented group, the usual gap elsewhere.
            let after = if quiet(t) && tools.get(i + 1).is_some_and(|&n| quiet(n)) { 0.0 } else { gap };
            ui.spacing_mut().item_spacing.x = after;
            if ltr && !first {
                // pageshell buttons are laid out where the cursor is: start the next line first.
                // The group moves to the next line whole: its first segment carries its width.
                let w = match t {
                    Tool::More | Tool::Popover => 0.0,
                    Tool::Details => quiet_tool_w(ui, "Details") + quiet_tool_w(ui, "Preview") + MORE_W,
                    Tool::Import => crate::pageshell::button_width(ui, "Import"),
                    Tool::Share => crate::pageshell::button_width(ui, share_label.as_str()),
                    Tool::Export => crate::pageshell::button_width(ui, "Export"),
                    Tool::Add => crate::pageshell::button_width(ui, "Add question"),
                };
                if w > ui.max_rect().right() - ui.cursor().min.x {
                    ui.end_row();
                }
            }
            first = false;
            let r = match t {
                Tool::Add => {
                    let r = crate::pageshell::primary(ui, "Add question");
                    self.add_question_menu(&r, bank_id);
                    r
                }
                Tool::Export => {
                    let r = kit::secondary(ui, "Export", TOOL_H).on_hover_text("Preview and download the bank file (PDF, answers included) or its JSON");
                    if r.clicked() {
                        open_bank_export(ctx, bank_id);
                    }
                    r
                }
                Tool::Share => {
                    let r = kit::secondary(ui, &share_label, TOOL_H).on_hover_text(if update {
                        "Share this bank with other teachers, or copy in questions they shared. The pool has a newer version of something imported here."
                    } else {
                        "Share this bank with other teachers, or copy in questions they shared (needs a sharing account)"
                    });
                    egui::Popup::menu(&r).show(|ui| {
                        ui.set_min_width(190.0);
                        // Nothing to share in an empty bank.
                        if !questions.is_empty() && ui.button(crate::share::shared::SHARE_ACTION).on_hover_text("Share this bank with other teachers (needs a sharing account)").clicked() {
                            let res = self.store.bank(bank_id).map(|b| crate::share::publish_ui::share_bank(ctx, b));
                            if let Some(Err(e)) = res {
                                self.toast_err(e);
                                // a draft in the bank stops the share: open the first one
                                let first = self.store.bank(bank_id).and_then(|b| b.questions.iter().find(|q| !crate::qcheck::gate_blockers(q).is_empty()).cloned());
                                if let Some(q) = first {
                                    self.qedit = Some(qedit::Draft::from_question(bank_id.to_string(), &q));
                                }
                            }
                            ui.close();
                        }
                        if ui.button("From pool…").on_hover_text("Find questions other teachers shared and copy them into this bank (needs a sharing account)").clicked() {
                            self.browse.open_for_bank(bank_id);
                            ui.close();
                        }
                        // Only when the pool has something newer.
                        if update && ui.button("Update from pool…").on_hover_text("The pool has a newer version of something imported into this bank. Nothing changes until you choose.").clicked() {
                            self.browse.open_update(bank_id);
                            ui.close();
                        }
                    });
                    r
                }
                Tool::Import => {
                    let r = kit::secondary(ui, "Import", TOOL_H).on_hover_text("Import files or pasted questions into this bank");
                    if r.clicked() {
                        crate::importui::open_import_modal(ctx, bank_id);
                    }
                    r
                }
                Tool::Popover => {
                    let r = quiet_tool(ui, Some(egui_phosphor::regular::EYE), Some("Preview"), "Preview", CornerRadius::ZERO).on_hover_text("Open this bank in its own window");
                    if r.clicked() {
                        open_popover(ctx, bank_id);
                    }
                    r
                }
                Tool::Details => {
                    let r = quiet_tool(ui, Some(crate::handicons::INFO), Some("Details"), "Details", CornerRadius { nw: QUIET_R, sw: QUIET_R, ne: 0, se: 0 }).on_hover_text("Description, author and license");
                    if r.clicked() {
                        let d = self.store.bank(bank_id).map(|b| b.details.clone()).unwrap_or_default();
                        with_state(ctx, key, |s: &mut BankEditState| s.details = Some(d));
                    }
                    r
                }
                // (Details has its own button beside this menu; the menu keeps what has none.)
                Tool::More => {
                    // Alone on an empty bank it is the whole group, rounded on every corner, and
                    // says what it is ("Bank actions", the tile menu's name) with a caret like
                    // Share's, since it is the header's only route to the bank's actions.
                    let r = if empty {
                        quiet_tool(ui, None, Some(&bank_actions_label()), "Bank actions", CornerRadius::same(QUIET_R)).on_hover_text("Details and copying questions from the pool")
                    } else {
                        quiet_tool(ui, None, None, "More bank actions", CornerRadius { nw: 0, sw: 0, ne: QUIET_R, se: QUIET_R })
                    };
                    egui::Popup::menu(&r).show(|ui| {
                        ui.set_min_width(170.0);
                        if empty {
                            // What an empty bank can use: its details, and bringing in questions
                            // other teachers shared (Share's menu holds these once it shows).
                            if ui.button("Details…").on_hover_text("Description, author and license").clicked() {
                                let d = self.store.bank(bank_id).map(|b| b.details.clone()).unwrap_or_default();
                                with_state(ctx, key, |s: &mut BankEditState| s.details = Some(d));
                                ui.close();
                            }
                            if ui.button("From pool…").on_hover_text("Find questions other teachers shared and copy them into this bank (needs a sharing account)").clicked() {
                                self.browse.open_for_bank(bank_id);
                                ui.close();
                            }
                            if update && ui.button("Update from pool…").on_hover_text("The pool has a newer version of something imported into this bank. Nothing changes until you choose.").clicked() {
                                self.browse.open_update(bank_id);
                                ui.close();
                            }
                            return;
                        }
                        let tidy = ui
                            .button("Tidy stems…")
                            .on_hover_text("Clean up stems from older imports: tables of blanks, line breaks, spacing. Shows a preview first.");
                        if tidy.clicked() {
                            open_tidy(self, ctx, key, bank_id);
                            ui.close();
                        }
                    });
                    r
                }
            };
            if quiet(t) {
                if group.is_positive() {
                    seams.push(if r.rect.left() >= group.right() { r.rect.left() } else { r.rect.right() } .round() + 0.5);
                }
                group = group.union(r.rect);
            }
            width += r.rect.width() + after;
            last_gap = after;
        }
        ui.spacing_mut().item_spacing.x = gap;
        if group.is_positive() {
            // Container: paper fill, 1 pt edge, Share's radius; a hairline at each inner seam
            // (every segment edge strictly inside the group), inset from top and bottom.
            ui.painter().set(group_bg, egui::Shape::rect_filled(group, CornerRadius::same(QUIET_R), theme::cream()));
            let p = ui.painter();
            p.rect_stroke(group, CornerRadius::same(QUIET_R), Stroke::new(1.0, theme::edge()), egui::StrokeKind::Inside);
            for x in seams {
                p.vline(x, (group.top() + 9.0)..=(group.bottom() - 9.0), Stroke::new(1.0, theme::edge()));
            }
        }
        width - last_gap
    }

    /// The type menu under an "Add question" button: the type is chosen first and the editor opens
    /// already set to it. Keyboard: Enter on the focused button opens the menu, arrows and Enter
    /// pick, 1-6 pick directly.
    fn add_question_menu(&mut self, r: &egui::Response, bank_id: &Id) {
        let opened_now = r.clicked();
        egui::Popup::menu(r).show(|ui| {
            ui.set_min_width(190.0);
            for (n, k) in crate::bankops::KINDS.into_iter().enumerate() {
                let hot = [egui::Key::Num1, egui::Key::Num2, egui::Key::Num3, egui::Key::Num4, egui::Key::Num5, egui::Key::Num6][n];
                let b = ui.button(format!("{}   {}", n + 1, k.label()));
                if n == 0 && opened_now {
                    b.request_focus();
                }
                if b.clicked() || ui.input(|i| i.key_pressed(hot)) {
                    self.qedit = Some(qedit::Draft::new_of(bank_id.clone(), k));
                    ui.close();
                }
            }
        });
    }

    fn bank_edit_body(&mut self, ui: &mut Ui, ctx: &egui::Context, key: UiId, bank_id: &Id) {
        let (bank_name, questions, modified) = match self.store.bank(bank_id) {
            Some(b) => (b.name.clone(), b.questions.clone(), b.modified),
            None => return,
        };
        let topics = unique_topics(&questions);
        // The page's width before anything is drawn: the circle's strip is kept to it.
        let page_w = ui.available_width();

        // Header: breadcrumb, serif name + subtitle, actions.
        crumb_row(ui, self, Some(&bank_name));
        ui.add_space(2.0);
        // The actions sit beside the name when both fit, else on their own row under it. Their
        // width is last frame's (all on one line), so a long name never runs under them.
        let tools_key = key.with("toolbar_w");
        let tools_w = ctx.data(|d| d.get_temp::<f32>(tools_key)).unwrap_or(640.0);
        let avail = ui.available_width();
        let beside = avail - tools_w - 24.0;
        // The whole name (and its rename pencil) must fit beside them, or it would be cut off.
        let name_need = ui.fonts_mut(|f| f.layout_no_wrap(bank_name.clone(), egui::FontId::new(34.0, theme::serif_family()), theme::ink()).size().x) + 12.0 + PENCIL_ROOM + 8.0;
        let one_row = beside >= name_need.max(300.0);
        let mut drawn_w = None;
        let mut name_row: Option<egui::Rect> = None;
        ui.horizontal(|ui| {
            // On a row of its own the name may use the whole width, less its rename pencil.
            let name_w = if one_row { beside.min(680.0) } else { avail - PENCIL_ROOM - 4.0 }.max(160.0);
            ui.vertical(|ui| {
                ui.set_max_width(if one_row { beside } else { avail });
                let mut name_buf = bank_name.clone();
                let name_font = egui::FontId::new(34.0, theme::serif_family());
                // The name field hugs its text so the rename pencil sits right after it.
                let text_w = ui.fonts_mut(|f| f.layout_no_wrap(name_buf.clone(), name_font.clone(), theme::ink()).size().x);
                let field_w = (text_w + 12.0).clamp(160.0, name_w);
                let field_id = ui.id().with(("bank_name_field", bank_id.as_str()));
                // A name too long for its line is shown on that one line ending in "…" (the whole
                // name on hover and in the breadcrumb), the pencil clear after it; the field comes
                // back while it is being renamed.
                let editing_key = field_id.with("editing");
                // 1: the pencil (or the name) was just pressed; 2: the field has had its focus.
                let editing = ui.data(|d| d.get_temp::<u8>(editing_key).unwrap_or(0));
                let editing = editing > 0 || ui.ctx().memory(|m| m.has_focus(field_id));
                let wraps = text_w + 12.0 > name_w && !editing;
                let row = ui
                    .horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        if wraps {
                            let mut job = egui::text::LayoutJob::single_section(name_buf.clone(), egui::TextFormat::simple(name_font.clone(), theme::ink()));
                            job.wrap = egui::text::TextWrapping { max_width: name_w, max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
                            let galley = ui.fonts_mut(|f| f.layout_job(job));
                            let label = ui.add(egui::Label::new(galley).sense(Sense::click())).on_hover_cursor(egui::CursorIcon::Text).on_hover_text(bank_name.as_str());
                            let pencil = pencil_button(ui);
                            if label.clicked() || pencil.clicked() {
                                ui.data_mut(|d| d.insert_temp(editing_key, 1u8));
                            }
                            None
                        } else {
                            let resp = ui.add(
                                TextEdit::singleline(&mut name_buf)
                                    .id(field_id)
                                    .font(name_font.clone())
                                    .text_color(theme::ink())
                                    .frame(Frame::new())
                                    .hint_text("Untitled bank")
                                    .desired_width(field_w),
                            );
                            match ui.data(|d| d.get_temp::<u8>(editing_key)) {
                                Some(1) => {
                                    resp.request_focus();
                                    ui.data_mut(|d| d.insert_temp(editing_key, 2u8));
                                }
                                Some(_) if !resp.has_focus() => ui.data_mut(|d| d.remove::<u8>(editing_key)),
                                _ => {}
                            }
                            if !resp.has_focus() && pencil_button(ui).clicked() {
                                resp.request_focus();
                            }
                            Some(resp)
                        }
                    });
                name_row = Some(row.response.rect);
                if let Some(resp) = row.inner
                    && resp.changed() && !name_buf.trim().is_empty() && name_buf.trim() != bank_name {
                    let t = now_secs();
                    if let Some(b) = self.store.bank_mut(bank_id) {
                        b.name = name_buf.trim().to_string();
                        b.modified = t;
                    }
                    self.store.save_banks();
                }
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    // Body size under the title: the page's facts, read before anything is pressed.
                    ui.label(RichText::new(format!(
                        "{} question{} · {} topic{} · edited {} · ",
                        questions.len(),
                        if questions.len() == 1 { "" } else { "s" },
                        topics.len(),
                        if topics.len() == 1 { "" } else { "s" },
                        rel_time(modified),
                    )).size(kit::ty::BODY).color(theme::text_col()));
                    let used: Vec<Id> = crate::bankops::exams_using_bank(&self.store.exams, bank_id).iter().map(|e| e.id.clone()).collect();
                    let r = usage_link(ui, used.len(), kit::ty::BODY);
                    if !used.is_empty() && ui.interact(r.rect, ui.id().with("bank_usage_link"), Sense::click()).clicked() {
                        open_usage(ctx, bank_name.clone(), used);
                    }
                    let pics: usize = questions.iter().map(crate::bankops::pictures_needed).sum();
                    if pics > 0 {
                        ui.label(RichText::new(" · ").size(kit::ty::BODY).color(theme::text_col()));
                        let link = ui
                            .add(
                                egui::Label::new(
                                    RichText::new(format!("{pics} picture{} needed", if pics == 1 { "" } else { "s" }))
                                        .size(kit::ty::BODY)
                                        .color(theme::bad())
                                        .underline(),
                                )
                                .sense(Sense::click()),
                            )
                            .on_hover_text("Add the missing pictures: upload, crop from the original, or describe them");
                        if link.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if link.clicked() {
                            crate::importui::open_resolve_images(self, bank_id);
                        }
                    }
                });
            });
            if one_row {
                // Centred on the name's line (not on name + meta), right-aligned to the page edge.
                let left = ui.cursor().min.x;
                let row = name_row.unwrap_or_else(|| ui.min_rect());
                let mid = row.center().y;
                let band = egui::Rect::from_min_max(
                    Pos2::new(left, mid - TOOL_H / 2.0 - 4.0),
                    Pos2::new(ui.max_rect().right(), mid + TOOL_H / 2.0 + 4.0),
                );
                let mut bar = ui.new_child(egui::UiBuilder::new().max_rect(band).id_salt("bank_toolbar").layout(Layout::right_to_left(Align::Center)));
                drawn_w = Some(self.bank_toolbar(&mut bar, ctx, key, bank_id, &questions, false));
            }
        });
        if !one_row {
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                drawn_w = Some(self.bank_toolbar(ui, ctx, key, bank_id, &questions, true));
            });
        }
        if let Some(w) = drawn_w
            && (w - tools_w).abs() > 0.5
        {
            ctx.data_mut(|d| d.insert_temp(tools_key, w));
            ctx.request_repaint();
        }
        let hosted = ui
            .scope(|ui| {
                ui.set_max_width(page_w);
                crate::share::bank_links_ui::strip(ui, self, bank_id)
            })
            .inner;
        if hosted { return; }
        let desc = self.store.bank(bank_id).map(|b| b.details.description.trim().to_string()).unwrap_or_default();
        if !desc.is_empty() {
            ui.add_space(2.0);
            ui.add(egui::Label::new(theme::text(desc).size(13.0)).wrap());
        }
        ui.add_space(12.0);
        // Usage counts per question for the cards (one pass per frame).
        let usage = crate::bankops::question_usage(&self.store.exams, bank_id);
        ctx.data_mut(|d| d.insert_temp(key.with("usage"), usage));
        // An empty bank has nothing to outline or filter: the empty card takes the whole width.
        let wide = ui.available_width() >= 860.0 && !questions.is_empty();
        if wide {
            let side_w = 200.0;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 18.0;
                ui.allocate_ui_with_layout(Vec2::new(side_w, 0.0), Layout::top_down(Align::Min), |ui| {
                    ui.set_width(side_w);
                    outline_panel(ui, ctx, key, &questions);
                });
                let rest = ui.available_width();
                ui.allocate_ui_with_layout(Vec2::new(rest, 0.0), Layout::top_down(Align::Min), |ui| {
                    ui.set_width(rest);
                    self.bank_edit_main(ui, ctx, key, bank_id, questions, topics, true);
                });
            });
        } else {
            self.bank_edit_main(ui, ctx, key, bank_id, questions, topics, false);
        }
    }

    /// Toolbar, filters and the question cards (right of the outline when wide).
    #[allow(clippy::too_many_arguments)]
    fn bank_edit_main(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        key: UiId,
        bank_id: &Id,
        questions: Vec<Question>,
        topics: Vec<String>,
        wide: bool,
    ) {
        // Toolbar row 1: search, sort, difficulty, view toggle.
        let st = with_state(ctx, key, |s: &mut BankEditState| s.clone());
        let mut search = st.search.clone();
        let mut list = self.settings.bank_edit_list_view;
        // An empty bank has nothing to search, sort or filter: its rows (and the lone "All 0"
        // chip) stay hidden until the first question is in.
        let filters = !questions.is_empty();
        if filters {
        ui.horizontal(|ui| {
            if search_pill(ui, &mut search, 280.0, "Search questions") {
                with_state(ctx, key, |s: &mut BankEditState| s.search = search.clone());
            }
            let mut sort_val = st.sort;
            ComboBox::from_id_salt("banks_sort")
                .selected_text(kit::meta(format!("Sort: {}", sort_val.label())))
                .show_ui(ui, |ui| {
                    for s in Sort::all() {
                        ui.selectable_value(&mut sort_val, s, s.label());
                    }
                });
            if sort_val != st.sort {
                with_state(ctx, key, |s: &mut BankEditState| s.sort = sort_val);
            }
            let mut diff_val = st.diff;
            ComboBox::from_id_salt("banks_diff")
                .selected_text(
                    theme::text(match diff_val {
                        Some(d) => format!("Difficulty: {}", diff_label(d)),
                        None => "Difficulty: All".to_string(),
                    })
                    .size(kit::ty::META),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut diff_val, None, "All");
                    for d in [Difficulty::Low, Difficulty::Medium, Difficulty::High] {
                        ui.selectable_value(&mut diff_val, Some(d), diff_label(d));
                    }
                });
            if diff_val != st.diff {
                with_state(ctx, key, |s: &mut BankEditState| s.diff = diff_val);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let mut mode = view_of(list);
                if kit::view_toggle(ui, &mut mode, "bank_edit") {
                    list = mode == ViewMode::List;
                }
            });
        });
        if list != self.settings.bank_edit_list_view {
            self.settings.bank_edit_list_view = list;
            save_settings(self);
        }
        ui.add_space(6.0);
        }

        // Narrow windows: type + topic chips stand in for the outline panel.
        if !wide && filters {
        // Toolbar row 2: type chips with counts.
        let mut counts = [0usize; 6];
        for q in &questions {
            counts[mix_index(q.kind.qkind())] += 1;
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            if kit::chip_toggle(ui, &format!("All  {}", questions.len()), st.kind.is_none()).clicked() {
                with_state(ctx, key, |s: &mut BankEditState| s.kind = None);
            }
            for k in [
                QKind::Mc,
                QKind::Tf,
                QKind::Multipart,
                QKind::Matching,
                QKind::Blank,
                QKind::Short,
            ] {
                let n = counts[mix_index(k)];
                if n == 0 && st.kind != Some(k) {
                    continue;
                }
                if kit::chip_toggle(ui, &format!("{}  {n}", qedit::type_tag(k)), st.kind == Some(k))
                    .clicked()
                {
                    let sel = if st.kind == Some(k) { None } else { Some(k) };
                    with_state(ctx, key, |s: &mut BankEditState| s.kind = sel);
                }
            }
        });
        // Row 3: topic chips (replaces the old outline panel).
        if !topics.is_empty() {
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                kit::label(ui, "Topics");
                for t in &topics {
                    let n = questions
                        .iter()
                        .filter(|q| q.topics.iter().any(|x| x.trim() == t))
                        .count();
                    let on = st.topic.as_deref() == Some(t.as_str());
                    let chip = kit::chip_toggle(ui, &format!("{t}  {n}"), on).on_hover_text("Right-click to rename, merge or delete");
                    if chip.clicked() {
                        let sel = if on { None } else { Some(t.clone()) };
                        with_state(ctx, key, |s: &mut BankEditState| s.topic = sel);
                    }
                    // Narrow layout has no outline panel: the topic menu lives on the chip.
                    chip.context_menu(|ui| {
                        let mut act = None;
                        if ui.button("Rename…").clicked() {
                            act = Some(TopicMode::Rename);
                        }
                        if topics.len() > 1 && ui.button("Merge into…").clicked() {
                            act = Some(TopicMode::Merge);
                        }
                        if ui.button("Delete…").clicked() {
                            act = Some(TopicMode::Delete);
                        }
                        if let Some(mode) = act {
                            let text = if mode == TopicMode::Rename { t.clone() } else { String::new() };
                            with_state(ctx, key, |s: &mut BankEditState| s.topic_edit = Some(TopicEdit { from: t.clone(), mode, text }));
                            ui.close();
                        }
                    });
                }
            });
        }
        }
        // Active filter note with a clear link (the outline shows the choice too).
        if wide && (st.topic.is_some() || st.kind.is_some()) {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let mut parts: Vec<String> = Vec::new();
                if let Some(k) = st.kind {
                    parts.push(k.label().to_string());
                }
                if let Some(t) = &st.topic {
                    parts.push(format!("topic “{t}”"));
                }
                ui.label(kit::meta_muted(format!("Showing {}", parts.join(" · "))));
                if kit::link(ui, "Clear", false).clicked() {
                    with_state(ctx, key, |s: &mut BankEditState| {
                        s.kind = None;
                        s.topic = None;
                    });
                }
            });
        }
        ui.add_space(10.0);

        // Undo strip for the last deleted question.
        if let Some((dq, idx)) = st.last_deleted.clone() {
            kit::card().inner_margin(Margin::symmetric(14, 8)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(theme::text(format!("Question deleted: {}", truncate(&crate::equation::stem_preview(&dq.stem), 60))).size(kit::ty::META));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(Button::new(RichText::new("Dismiss").color(theme::muted_col())).frame(false))
                            .clicked()
                        {
                            with_state(ctx, key, |s: &mut BankEditState| s.last_deleted = None);
                        }
                        if crate::pageshell::outline(ui, "Undo").clicked() {
                            if let Some(b) = self.store.bank_mut(bank_id) {
                                let at = idx.min(b.questions.len());
                                b.questions.insert(at, dq.clone());
                                b.modified = now_secs();
                            }
                            self.store.save_banks();
                            with_state(ctx, key, |s: &mut BankEditState| s.last_deleted = None);
                            self.toast("Question restored");
                        }
                    });
                });
            });
            ui.add_space(10.0);
        }

        // Undo strip for the last "Tidy stems".
        if let Some(u) = st.last_tidy.clone() {
            kit::card().inner_margin(Margin::symmetric(14, 8)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let file = std::path::Path::new(&u.backup).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
                    let n = u.before.len();
                    ui.label(theme::text(format!("Tidied {n} question{} · backup saved as {file}", if n == 1 { "" } else { "s" })).size(kit::ty::META))
                        .on_hover_text(u.backup.clone());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(Button::new(RichText::new("Dismiss").color(theme::muted_col())).frame(false)).clicked() {
                            with_state(ctx, key, |s: &mut BankEditState| s.last_tidy = None);
                        }
                        if crate::pageshell::outline(ui, "Undo").on_hover_text("Put these questions back as they were before tidying").clicked() {
                            let t = now_secs();
                            if let Some(b) = self.store.bank_mut(bank_id) {
                                crate::tidy::undo(&mut b.questions, &u.before);
                                b.modified = t;
                            }
                            self.store.save_banks();
                            with_state(ctx, key, |s: &mut BankEditState| s.last_tidy = None);
                            self.toast("Tidy undone");
                        }
                    });
                });
            });
            ui.add_space(10.0);
        }

        // Filter + order.
        let query = search.trim().to_lowercase();
        let mut order: Vec<usize> = questions
            .iter()
            .enumerate()
            .filter(|(_, q)| {
                (query.is_empty() || q.stem.to_lowercase().contains(&query) || crate::equation::stem_preview(&q.stem).to_lowercase().contains(&query))
                    && st.diff.is_none_or(|d| Some(d) == q.difficulty)
                    && st.kind.is_none_or(|k| k == q.kind.qkind())
                    && st
                        .topic
                        .as_deref()
                        .is_none_or(|t| q.topics.iter().any(|x| x.trim() == t))
            })
            .map(|(i, _)| i)
            .collect();
        order.sort_by(|&a, &b| {
            let (qa, qb) = (&questions[a], &questions[b]);
            match st.sort {
                Sort::Newest => qb.created.cmp(&qa.created),
                Sort::Oldest => qa.created.cmp(&qb.created),
                Sort::Kind => qa
                    .kind
                    .qkind()
                    .label()
                    .cmp(qb.kind.qkind().label())
                    .then_with(|| qb.created.cmp(&qa.created)),
                Sort::Difficulty => diff_rank(qa.difficulty)
                    .cmp(&diff_rank(qb.difficulty))
                    .then_with(|| qb.created.cmp(&qa.created)),
                Sort::Topic => first_topic(qa)
                    .cmp(&first_topic(qb))
                    .then_with(|| qb.created.cmp(&qa.created)),
            }
        });

        let open_qid: Option<String> = self.qedit.as_ref().and_then(|d| d.qid.clone());
        let new_open = self.qedit.as_ref().is_some_and(|d| d.qid.is_none());

        let open_marker = open_qid.clone().or(new_open.then(|| "<new>".to_string()));
        if st.last_open != open_marker {
            with_state(ctx, key, |s: &mut BankEditState| s.last_open = open_marker.clone());
        }

        let avail = ui.available_width();
        let gap = 10.0;
        let list = self.settings.bank_edit_list_view;
        let cols = if list { 1 } else if avail >= 820.0 { 3 } else if avail >= 540.0 { 2 } else { 1 };
        let cw = ((avail - gap * (cols as f32 - 1.0)) / cols as f32).floor();

        if questions.is_empty() && !new_open {
            // (the filter rows above are hidden for an empty bank, so the gap is the header's)
            ui.add_space(4.0);
            // The card's own buttons do what the toolbar's Add question and Import do. On a wide
            // page the card is one row across its width (the strip below is a full-width band too);
            // narrower, the shared stacked, centred card.
            let (title, body) = ("No questions yet", "Write the first question yourself, or import a test you already have.");
            let actions = [("Add question", true), ("Import questions", false)];
            let b = if ui.available_width() >= EMPTY_WIDE {
                empty_bank_card_row(ui, crate::handicons::EDIT, title, body, &actions)
            } else {
                crate::pageshell::empty_state_buttons(ui, crate::handicons::EDIT, title, body, &actions)
            };
            if let Some(add) = b.first() {
                self.add_question_menu(add, bank_id);
            }
            if b.get(1).cloned().is_some_and(|r| r.on_hover_text("Import files or pasted questions into this bank").clicked()) {
                crate::importui::open_import_modal(ctx, bank_id);
            }
            // What a bank gives you, in a quieter band attached under the card (the welcome page's
            // hero and "How it works" pairing); gone with the first question.
            ui.add_space(kit::GUTTER);
            empty_bank_strip(ui);
        } else if order.is_empty() {
            if crate::pageshell::empty_line(ui, crate::handicons::BANKS, "No questions match those filters.", Some("Clear filters")).is_some() {
                with_state(ctx, key, |s: &mut BankEditState| {
                    s.search.clear();
                    s.kind = None;
                    s.topic = None;
                    s.diff = None;
                });
            }
        }

        let mut pending: Vec<usize> = Vec::new();
        for &i in &order {
            // The question being edited keeps its place in the list; the editor is a modal.
            pending.push(i);
            if pending.len() == cols {
                flush_row(ui, self, ctx, key, bank_id, &questions, &mut pending, cw, gap, list, &topics);
            }
        }
        flush_row(ui, self, ctx, key, bank_id, &questions, &mut pending, cw, gap, list, &topics);

        if !questions.is_empty() {
            ui.add_space(6.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    theme::muted(format!(
                        "Showing {} of {} · click a card to edit it",
                        order.len(),
                        questions.len()
                    ))
                    .size(kit::ty::META),
                );
            });
        }

        self.question_modal(ctx, key, bank_id, &topics);
    }

    /// The question editor: a modal over the list. Applies Save, Cancel and Delete.
    fn question_modal(&mut self, ctx: &egui::Context, key: UiId, bank_id: &str, pool: &[String]) {
        let Some(mut d) = self.qedit.take() else { return };
        let mut textures = std::mem::take(&mut self.textures);
        let action = qedit::draw_modal(ctx, &mut d, pool, &mut textures);
        self.textures = textures;
        if !matches!(action, qedit::QAction::None) {
            crate::wysiwyg::new_session(ctx);
        }
        match action {
            qedit::QAction::Done => self.commit_draft(d),
            qedit::QAction::Cancel => {}
            qedit::QAction::Delete => {
                if let Some(qid) = d.qid.clone() {
                    delete_question(self, ctx, key, bank_id, &qid);
                }
            }
            qedit::QAction::None => self.qedit = Some(d),
        }
    }
}

/// Draws the pending collapsed cards as one grid row, then clears `pending`.
#[allow(clippy::too_many_arguments)]
fn flush_row(
    ui: &mut Ui,
    app: &mut App,
    ctx: &egui::Context,
    key: UiId,
    bank_id: &str,
    questions: &[Question],
    pending: &mut Vec<usize>,
    cw: f32,
    gap: f32,
    list: bool,
    pool: &[String],
) {
    if pending.is_empty() {
        return;
    }
    // Cards in a row share one height: the tallest of them. Measuring first (a sizing pass,
    // which takes no clicks and takes no room) keeps the row's bottom edge straight.
    let mut row_h = 0.0_f32;
    if !list && pending.len() > 1 {
        for &i in pending.iter() {
            let h = measure_card(ui, app, ctx, key, bank_id, cw, &questions[i], i + 1, pool);
            row_h = row_h.max(h);
        }
    }
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for &i in pending.iter() {
            ui.allocate_ui_with_layout(Vec2::new(cw, row_h), Layout::top_down(Align::Min), |ui| {
                ui.set_width(cw);
                draw_card(ui, app, ctx, key, bank_id, cw, Some(&questions[i]), i + 1, list, pool, false, row_h);
            });
        }
    });
    ui.add_space(if list { 6.0 } else { gap });
    pending.clear();
}

/// The height a question card would take at this width, measured without drawing anything
/// that can be clicked.
fn measure_card(
    ui: &mut Ui,
    app: &mut App,
    ctx: &egui::Context,
    key: UiId,
    bank_id: &str,
    cw: f32,
    q: &Question,
    n: usize,
    pool: &[String],
) -> f32 {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .sizing_pass()
            .max_rect(Rect::from_min_size(ui.cursor().left_top(), Vec2::new(cw, 6000.0)))
            .layout(Layout::top_down(Align::Min)),
    );
    child.set_width(cw);
    draw_card(&mut child, app, ctx, key, bank_id, cw, Some(q), n, false, pool, false, 0.0);
    (child.min_rect().height()).max(0.0)
}

// ---------- banks list pieces ----------

fn paint_mix_bar(ui: &mut Ui, w: f32, mix: &[usize; 6]) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 6.0), Sense::hover());
    mix_bar_at(ui.painter(), rect, mix);
}

/// The type-mix bar painted into `rect` (6 px tall on the bank cards), one segment per type in
/// [`mix_index`] order, each in its [`theme::type_hue`] (the key's swatches), with a 2 px gap of
/// card between segments so neighbours stay apart.
pub(crate) fn mix_bar_at(p: &egui::Painter, rect: Rect, mix: &[usize; 6]) {
    mix_bar_folded(p, rect, mix, &[]);
}

/// The gap of card between two bar segments.
const BAR_GAP: f32 = 2.0;

/// Each bar segment as (its type, or None for the folded rest, its x span), `w` wide from 0:
/// shown types in [`mix_index`] order, then the `folded` types (the key's "+N more") as one
/// segment at the end, [`BAR_GAP`] between segments, none narrower than 4.
fn bar_segments(mix: &[usize; 6], folded: &[usize], w: f32) -> Vec<(Option<usize>, f32, f32)> {
    let total: usize = mix.iter().sum();
    if total == 0 {
        return Vec::new();
    }
    let mut parts: Vec<(Option<usize>, usize)> = (0..6).filter(|i| mix[*i] > 0 && !folded.contains(i)).map(|i| (Some(i), mix[i])).collect();
    let rest: usize = folded.iter().map(|&i| mix[i]).sum();
    if rest > 0 {
        parts.push((None, rest));
    }
    let k = parts.len() as f32;
    let room = (w - BAR_GAP * (k - 1.0)).max(0.0);
    // Widths in proportion, each at least 4; the widest gives back what the small ones took.
    let mut ws: Vec<f32> = parts.iter().map(|&(_, n)| (room * n as f32 / total as f32).max(4.0)).collect();
    let over: f32 = ws.iter().sum::<f32>() - room;
    if over > 0.0
        && let Some(m) = ws.iter_mut().max_by(|a, b| a.total_cmp(b))
    {
        *m = (*m - over).max(4.0);
    }
    let mut x = 0.0;
    parts
        .iter()
        .zip(ws)
        .map(|(&(t, _), sw)| {
            let seg = (t, x, x + sw);
            x += sw + BAR_GAP;
            seg
        })
        .collect()
}

/// [`mix_bar_at`] with the types the key folded into "+N more" drawn as one hollow segment at
/// the end, the same outline as the "+N more" swatch, so every segment matches a key entry.
fn mix_bar_folded(p: &egui::Painter, rect: Rect, mix: &[usize; 6], folded: &[usize]) {
    let segs = bar_segments(mix, folded, rect.width());
    if segs.is_empty() {
        p.rect_filled(rect, CornerRadius::same(3), theme::field());
        return;
    }
    for (t, a, b) in segs {
        let r = Rect::from_min_max(Pos2::new(rect.left() + a, rect.top()), Pos2::new(rect.left() + b, rect.bottom()));
        match t {
            Some(i) => {
                p.rect_filled(r, CornerRadius::same(3), theme::type_hue(i));
            }
            None => {
                p.rect(r, CornerRadius::same(3), theme::card(), Stroke::new(1.0, theme::muted_col()), egui::StrokeKind::Inside);
            }
        }
    }
}

/// What the bank "..." menu asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BankAct {
    Open,
    Popover,
    Export,
    Rename,
    Archive,
    Delete,
}

/// The bank "..." menu items in order (as on the reference: Open, Pop-over, Export, then Delete
/// after a separator; Rename is ours). `None` marks the separator; the bool says enabled
/// (Export needs at least one question).
fn bank_menu_items(nq: usize, archived: bool) -> Vec<Option<(BankAct, &'static str, bool)>> {
    vec![
        Some((BankAct::Open, "Open", true)),
        Some((BankAct::Popover, "Preview", true)),
        Some((BankAct::Export, "Export…", nq > 0)),
        Some((BankAct::Rename, "Rename", true)),
        Some((BankAct::Archive, if archived { "Restore" } else { "Archive" }, true)),
        None,
        Some((BankAct::Delete, "Delete…", true)),
    ]
}

/// The bank menu's items, drawn into an open popup; returns the one clicked.
fn bank_menu_body(ui: &mut Ui, nq: usize, archived: bool) -> Option<BankAct> {
    let mut act = None;
    ui.set_min_width(150.0);
    for it in bank_menu_items(nq, archived) {
        match it {
            None => {
                ui.separator();
            }
            Some((a, label, enabled)) => {
                let text = if a == BankAct::Delete { RichText::new(label).color(theme::bad()) } else { RichText::new(label) };
                let b = ui.add_enabled(enabled, egui::Button::new(text));
                let b = if a == BankAct::Export && !enabled { b.on_disabled_hover_text("Add a question first") } else { b };
                if b.clicked() {
                    act = Some(a);
                    ui.close();
                }
            }
        }
    }
    act
}

/// The list row's "…" (32 square, the icon tier) at `rect`'s centre; returns the item clicked.
fn bank_menu(ui: &mut Ui, rect: Rect, r: &BankTile) -> Option<BankAct> {
    bank_menu_with(ui, rect, r, kit::more_dots)
}

/// [`bank_menu`] with its "…" drawn by `dots` (a tile's is the quiet one: its Open leads).
fn bank_menu_with(ui: &mut Ui, rect: Rect, r: &BankTile, dots: fn(&mut Ui, f32, &str) -> egui::Response) -> Option<BankAct> {
    // (a child Ui: placing the button never moves the parent's cursor)
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).id_salt(("bank_menu", &r.id)));
    let b = dots(&mut child, MENU_SIDE, "Bank actions");
    egui::Popup::menu(&b).show(|ui| bank_menu_body(ui, r.questions, r.archived)).and_then(|p| p.inner)
}

/// A Ui in `r` that does not move `ui`'s cursor (a tile places its controls in its own rect).
fn placed(ui: &mut Ui, r: Rect, salt: impl std::hash::Hash + std::fmt::Debug) -> Ui {
    ui.new_child(egui::UiBuilder::new().max_rect(r).id_salt(salt).layout(Layout::left_to_right(Align::Center)))
}

/// The left and right edges of `n` grid columns `gap` apart across `avail` from `x0`, each on a
/// whole device pixel (`ppp` pixels per point) so 1 pt borders stay sharp; the last column ends on
/// `x0 + avail` (snapped), the page's right edge, absorbing the rounding.
fn grid_columns(x0: f32, avail: f32, n: usize, gap: f32, ppp: f32) -> Vec<(f32, f32)> {
    let snap = |v: f32| (v * ppp).round() / ppp;
    let n = n.max(1);
    let cw = (avail - gap * (n as f32 - 1.0)) / n as f32;
    (0..n)
        .map(|i| {
            let l = x0 + i as f32 * (cw + gap);
            let r = if i + 1 == n { x0 + avail } else { l + cw };
            (snap(l), snap(r))
        })
        .collect()
}

/// `ui` with its widgets' minimum height set to `h`, so a button asked for `h` is drawn `h` tall
/// (the theme's 34 would otherwise make it taller than the slot laid out for it).
fn fixed_h(mut ui: Ui, h: f32) -> Ui {
    ui.spacing_mut().interact_size.y = h;
    ui
}

pub(crate) fn apply_bank_menu(app: &mut App, ctx: &egui::Context, key: UiId, r: &BankTile, act: Option<BankAct>) {
    match act {
        Some(BankAct::Open) => app.open_bank(&r.id),
        Some(BankAct::Popover) => open_popover(ctx, &r.id),
        Some(BankAct::Export) => open_bank_export(ctx, &r.id),
        Some(BankAct::Rename) => with_state(ctx, key, |s: &mut BanksState| s.rename = Some((r.id.clone(), r.name.clone()))),
        Some(BankAct::Archive) => {
            app.store.set_archived(&r.id, !r.archived);
            app.toast(if r.archived { "Bank restored" } else { "Bank archived. Show archived brings it back." });
        }
        Some(BankAct::Delete) => with_state(ctx, key, |s: &mut BanksState| s.confirm_delete = Some(r.id.clone())),
        None => {}
    }
}

/// "11 questions · 4 topics" (with "Archived · " first on an archived bank); `topics` false
/// leaves the topics off (a circle badge needs the room).
fn counts_line(r: &BankTile, topics: bool) -> String {
    let mut s = format!("{}{} question{}", if r.archived { "Archived · " } else { "" }, r.questions, if r.questions == 1 { "" } else { "s" });
    if topics {
        s += &format!(" · {} topic{}", r.topics, if r.topics == 1 { "" } else { "s" });
    }
    s
}

// ---------- the bank tile, shared by Banks and Home (spec section 3) ----------

/// Every bank tile is this tall on both screens: each slot is filled on every tile, so a row's
/// tiles end on one line without being stretched. The slots, top down: pad, header (2 name
/// lines), gap, meta, gap, key (2 lines), gap, hairline, gap, the footer's Open button (the usage
/// link level with its middle), then the side pad again under it, so Open sits as far from the
/// tile's bottom edge as from its right. No type-mix bar: the key (count and name per swatch)
/// already says what the bar said (round 20).
pub(crate) const BANK_TILE_H: f32 = T_PAD + T_HEAD + 6.0 + T_META + T_MK + T_KEY + T_BR + 1.0 + T_RA + T_ACT + T_PAD_FOOT;
const T_PAD: f32 = 16.0;
const T_HEAD: f32 = 44.0;
const T_META: f32 = 18.0;
/// Meta line to key.
const T_MK: f32 = 8.0;
const T_KEY: f32 = 40.0;
/// Key to hairline.
const T_BR: f32 = 12.0;
/// Hairline to the footer's button.
const T_RA: f32 = 8.0;
/// An empty bank's Add questions: the compact button height (the Exams cards' Open), with this
/// clear space above and below it inside its dashed box, and between the box and the hairline.
const ADD_H: f32 = 28.0;
const ADD_PAD: f32 = 8.0;
/// The footer: Open, a tinted accent button this tall (drawn exactly this tall, the "…"'s
/// size); the usage link (18) sits level with it.
const T_ACT: f32 = 32.0;
const T_FOOT: f32 = 18.0;
/// Under Open: the tile's side pad, so the button keeps the same margin below as on its right.
const T_PAD_FOOT: f32 = T_PAD;
/// The "…" on a Banks tile (and the list row): a 32 square in the icon tier.
const MENU_SIDE: f32 = 32.0;
/// The glyph beside the name on a Home tile.
const T_GLYPH: f32 = 34.0;
/// The type key's two lines, 18 each with 4 between.
const KEY_LINE: f32 = 18.0;
const KEY_LGAP: f32 = 4.0;
const KEY_LABELS: [&str; 6] = ["multiple choice", "true/false", "multipart", "matching", "fill in the blank", "short answer"];

/// Where a tile is drawn: Home has a glyph and no menu; Banks has the "…" menu and the circle badge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TileVariant {
    Home,
    Banks,
}

/// What a click on a tile asks for; the caller applies it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TileAct {
    Open,
    Usage,
    AddQuestions,
    Menu(BankAct),
}

/// The type key laid out for a tile: each shown type's mix index, x and line, and the "+N more"
/// (its x on line 2 and the types it stands for) when two lines do not hold them all.
#[derive(Debug, PartialEq)]
struct TileKey {
    items: Vec<(usize, f32, usize)>,
    more: Option<(f32, Vec<usize>)>,
    /// How much closer than usual each swatch sits to its words (0, or 1 on the tightest key).
    snug: f32,
}

/// The "+N more" text.
fn more_label(n: usize) -> String {
    format!("+{n} more")
}

/// Lays out the type key in `w`: items in [`mix_index`] order, two lines at most, every type
/// named. The items sit 12 apart; when that needs a third line they close up to 8 apart, then
/// each swatch also sits 1 closer to its words; only when even that needs a third line does line 2
/// drop items from its end until "+N more" fits 12 after the last (the last resort: a tile too
/// narrow for all its types by name).
/// `item_w(i)` is item i's width (swatch, gap and words); `more_w(n)` the width of "+n more".
fn layout_tile_key(mix: &[usize; 6], w: f32, item_w: impl Fn(usize) -> f32, more_w: impl Fn(usize) -> f32) -> TileKey {
    let usual = layout_tile_key_gap(mix, w, &item_w, &more_w, 12.0, 0.0);
    if usual.more.is_none() {
        return usual;
    }
    for (gap, snug) in [(8.0, 0.0), (8.0, 1.0)] {
        let k = layout_tile_key_gap(mix, w, &item_w, &more_w, gap, snug);
        if k.more.is_none() {
            return k;
        }
    }
    usual
}

/// [`layout_tile_key`] with its items `IGAP` apart.
fn layout_tile_key_gap(mix: &[usize; 6], w: f32, item_w: &impl Fn(usize) -> f32, more_w: &impl Fn(usize) -> f32, igap: f32, snug: f32) -> TileKey {
    let item_w = |i: usize| item_w(i) - snug;
    let shown: Vec<usize> = (0..6).filter(|&i| mix[i] > 0).collect();
    let mut items: Vec<(usize, f32, usize)> = Vec::new();
    let mut rest: Vec<usize> = Vec::new();
    let (mut x, mut line) = (0.0_f32, 0usize);
    for &i in &shown {
        let iw = item_w(i);
        if x > 0.0 && x + iw > w {
            x = 0.0;
            line += 1;
        }
        if line > 1 {
            rest.push(i);
            continue;
        }
        items.push((i, x, line));
        x += iw + igap;
    }
    if rest.is_empty() {
        return TileKey { items, more: None, snug };
    }
    // Make room on line 2 for "+N more", dropping its last items as needed.
    loop {
        let end = items.iter().filter(|it| it.2 == 1).map(|&(i, x, _)| x + item_w(i)).fold(0.0, f32::max);
        let at = if end > 0.0 { end + igap } else { 0.0 };
        if at + more_w(rest.len()) <= w || !items.iter().any(|it| it.2 == 1) {
            return TileKey { items, more: Some((at, rest)), snug };
        }
        let (i, _, _) = items.pop().expect("line 2 has an item");
        rest.insert(0, i);
    }
}

/// The footer's "Edited …" in the form that fits `room`: "Edited 23 days ago", then the
/// month-day form "Edited Oct 16", then "Oct 16".
fn edited_forms(edited: i64) -> [String; 3] {
    let md = if edited > 0 {
        let (_, m, d) = time::civil_from_days(edited.div_euclid(86_400));
        const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
        format!("{} {d}", MONTHS[(m as usize).clamp(1, 12) - 1])
    } else {
        "—".to_string()
    };
    [format!("Edited {}", rel_time(edited)), format!("Edited {md}"), md]
}

/// One bank tile, `w` wide and [`BANK_TILE_H`] tall, the same on Banks and Home: the serif name
/// (two lines kept), the counts and when it was edited (or the circle badge on Banks), the type
/// key (or, for an empty bank, a dashed placeholder holding Add questions),
/// a hairline, then where it is used (an underlined link) and "Open →". One card interact: a
/// click on the usage link, Add questions or the "…" menu returns that act and never `Open`.
pub(crate) fn bank_tile(ui: &mut Ui, app: &mut App, w: f32, t: &BankTile, v: TileVariant) -> Option<TileAct> {
    let (rect, card) = ui.allocate_exact_size(Vec2::new(w, BANK_TILE_H), Sense::click());
    card.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &t.name));
    let mut act = None;
    let hot = card.hovered() || card.has_focus();
    {
        let p = ui.painter();
        if card.hovered() {
            let col = if theme::is_dark() { Color32::from_black_alpha(70) } else { Color32::from_black_alpha(22) };
            p.add(egui::Shadow { offset: [0, 3], blur: 12, spread: 0, color: col }.as_shape(rect, CornerRadius::same(kit::R_CARD)));
        }
        let edge = if hot { theme::border().lerp_to_gamma(theme::accent_col(), 0.45) } else { theme::border() };
        p.rect(rect, CornerRadius::same(kit::R_CARD), theme::card(), Stroke::new(1.0, edge), egui::StrokeKind::Inside);
        if card.has_focus() {
            p.rect_stroke(rect.expand(1.0), CornerRadius::same(kit::R_CARD + 1), Stroke::new(2.0, theme::accent_col()), egui::StrokeKind::Outside);
        }
    }
    if card.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let x0 = rect.left() + T_PAD;
    let iw = w - 2.0 * T_PAD;
    let top = rect.top() + T_PAD;

    // Header: the name, two lines kept, top-aligned; "…" top right (Banks) or the glyph (Home).
    let (name_x, name_w) = match v {
        TileVariant::Banks => (x0, iw - MENU_SIDE - 12.0),
        TileVariant::Home => (x0 + T_GLYPH + 11.0, iw - T_GLYPH - 11.0),
    };
    let mut job = egui::text::LayoutJob::default();
    job.append(&t.name, 0.0, egui::TextFormat { font_id: kit::card_title_font(), color: theme::ink(), line_height: Some(kit::ty::CARD_LINE), ..Default::default() });
    job.wrap = egui::text::TextWrapping { max_width: name_w.max(40.0), max_rows: 2, break_anywhere: false, overflow_character: Some('…') };
    let name = ui.painter().layout_job(job);
    let one_line = name.rows.len() <= 1;
    // On Home a one-line name sits level with the glyph's middle; it never moves the meta line.
    let name_y = if v == TileVariant::Home && one_line { top + (T_GLYPH - kit::ty::CARD_LINE) / 2.0 } else { top };
    ui.painter().galley(Pos2::new(name_x, name_y), name, theme::ink());
    if v == TileVariant::Home {
        let g = Rect::from_min_size(Pos2::new(x0, top), Vec2::splat(T_GLYPH));
        ui.painter().rect_filled(g, CornerRadius::same(10), theme::accent_faded());
        crate::handicons::paint_glyph(ui.painter(), g.center(), crate::handicons::BANKS, 16.8, theme::accent_col());
    }

    // Meta: the counts (or "No questions yet"), and when it was edited, right, in the first form
    // that fits; on Banks a circle badge takes the rest of the row instead of the edited text.
    let meta_y = top + T_HEAD + 6.0;
    let meta_c = meta_y + T_META / 2.0;
    let badged = v == TileVariant::Banks && app.shared.circles.edit.with_link(&t.id, |_| ()).is_some();
    let meta_font = egui::FontId::new(kit::ty::META, theme::sans_family());
    let measure = |s: &str| ui.painter().layout_no_wrap(s.to_string(), meta_font.clone(), theme::text_col()).size().x;
    let forms = edited_forms(t.edited);
    let edited_w = [measure(&forms[0]), measure(&forms[1]), measure(&forms[2])];
    let empty_text = format!("{}No questions yet", if t.archived { "Archived · " } else { "" });
    let (topics, edited) = if t.questions == 0 {
        let w = measure(&empty_text);
        (true, if badged { None } else { (0..3).find(|&k| w + 12.0 + edited_w[k] <= iw) })
    } else if badged {
        (false, None)
    } else {
        meta_fit(measure(&counts_line(t, true)), measure(&counts_line(t, false)), edited_w, iw)
    };
    let counts_text = if t.questions == 0 { empty_text } else { counts_line(t, topics) };
    let counts = ui.painter().layout_no_wrap(counts_text, meta_font.clone(), theme::text_col());
    let counts_w = counts.size().x;
    let counts_col = if t.questions == 0 { theme::muted_col() } else { theme::text_col() };
    ui.painter().galley(Pos2::new(x0, meta_c - counts.size().y / 2.0), counts, counts_col);
    if let Some(k) = edited {
        let g = ui.painter().layout_no_wrap(forms[k].clone(), meta_font.clone(), theme::muted_col());
        ui.painter().galley(Pos2::new(x0 + iw - g.size().x, meta_c - g.size().y / 2.0), g, theme::muted_col());
    }

    // Key: the type key, each type's count and name; an empty bank has no types to show, so the
    // key slot holds one dashed placeholder with its next step, Add questions, in the middle: the
    // compact button with ADD_PAD clear above and below it, the box ending ADD_PAD over the hairline.
    let key_y = meta_y + T_META + T_MK;
    let mut add_btn: Option<Rect> = None;
    let mut more_hit = false;
    if t.questions == 0 {
        let slot = Rect::from_min_size(Pos2::new(x0, key_y), Vec2::new(iw, ADD_H + 2.0 * ADD_PAD));
        dashed_round_rect(ui.painter(), slot, 8.0, Stroke::new(1.0, theme::edge()));
        let btn_w = ui.painter().layout_no_wrap("Add questions".to_string(), egui::FontId::new(kit::ty::BUTTON_SM, theme::bold_family()), theme::ink()).size().x + 28.0;
        add_btn = Some(Rect::from_center_size(slot.center(), Vec2::new(btn_w.ceil(), ADD_H)));
    } else {
        let (_, more_pressed) = paint_tile_key(ui, Pos2::new(x0, key_y), iw, &t.mix, &t.id);
        // "+N more" is a control of its own: pressing it never opens the bank.
        more_hit = more_pressed;
    }

    // Hairline, then the footer: where it is used (a link to those exams) left, Open right, the
    // tile's main action as a tinted accent button (the "…" beside the name stays quiet).
    let rule_y = key_y + T_KEY + T_BR;
    ui.painter().hline(x0..=x0 + iw, rule_y + 0.5, Stroke::new(1.0, theme::border()));
    let act_y = rule_y + 1.0 + T_RA;
    let foot_c = act_y + T_ACT / 2.0;
    if t.used_in.is_empty() {
        let g = ui.painter().layout_no_wrap(usage_text(t), meta_font.clone(), theme::muted_col());
        ui.painter().galley(Pos2::new(x0, foot_c - g.size().y / 2.0), g, theme::muted_col());
    }

    // The tile's own controls, added after its interact so each takes its own clicks.
    if t.questions == 0
        && let Some(r) = add_btn
        && kit::secondary(&mut placed(ui, r, ("tile_add", &t.id)), "Add questions", ADD_H).clicked()
    {
        act = Some(TileAct::AddQuestions);
    }
    let open_w = kit::tinted_accent_w(ui, "Open");
    let open_r = Rect::from_min_size(Pos2::new(x0 + iw - open_w, act_y), Vec2::new(open_w, T_ACT));
    if kit::tinted_accent(&mut fixed_h(placed(ui, open_r, ("tile_open", &t.id)), T_ACT), "Open", T_ACT).clicked() {
        act = Some(TileAct::Open);
    }
    if !t.used_in.is_empty() {
        let r = Rect::from_min_max(Pos2::new(x0, foot_c - T_FOOT / 2.0), Pos2::new(open_r.left() - 12.0, foot_c + T_FOOT / 2.0));
        if tile_usage_link(&mut placed(ui, r, ("tile_usage", &t.id)), &usage_text(t)).clicked() {
            act = Some(TileAct::Usage);
        }
    }
    if badged {
        let bx = x0 + counts_w + 12.0;
        let r = Rect::from_min_max(Pos2::new(bx, meta_c - 11.0), Pos2::new(x0 + iw, meta_c + 11.0));
        if r.width() > 24.0 {
            let mut b = placed(ui, r, ("tile_badge", &t.id));
            b.set_clip_rect(r.intersect(b.clip_rect()));
            crate::share::bank_links_ui::badge(&mut b, app, &t.id);
        }
    }
    if v == TileVariant::Banks {
        let r = Rect::from_min_size(Pos2::new(x0 + iw - MENU_SIDE, top - 4.0), Vec2::splat(MENU_SIDE));
        if let Some(a) = bank_menu_with(ui, r, t, kit::quiet_more_dots) {
            act = Some(TileAct::Menu(a));
        }
    }
    let card = if edited.is_none() { card.on_hover_text(forms[0].clone()) } else { card };
    if act.is_none() && !more_hit && card.clicked() {
        act = Some(TileAct::Open);
    }
    act
}

/// Which meta row fits `room`: the counts with topics (`full`) or without (`short`), then the
/// edited text in its first form that fits 12 after them. The long "Edited 24 days ago" wins over
/// the topics (so a row of narrow tiles reads alike: "11 questions", "Edited 24 days ago" right), then
/// the month-day forms; with no room for any edited form the counts stand alone (topics kept
/// when they fit).
fn meta_fit(full: f32, short: f32, edited: [f32; 3], room: f32) -> (bool, Option<usize>) {
    const GAP: f32 = 12.0;
    let order = [(true, 0), (false, 0), (true, 1), (false, 1), (true, 2), (false, 2)];
    order
        .iter()
        .find(|&&(t, k)| if t { full } else { short } + GAP + edited[k] <= room)
        .map(|&(t, k)| (t, Some(k)))
        .unwrap_or((full <= room, None))
}

/// A link that reads as one at rest: 13 pt accent, regular weight (a step below the semibold
/// "Open"), with a quiet underline that turns solid under the pointer or focus.
fn tile_usage_link(ui: &mut Ui, label: &str) -> egui::Response {
    let col = theme::accent_col();
    let g = ui.painter().layout_no_wrap(label.to_owned(), egui::FontId::new(kit::ty::META, theme::sans_family()), col);
    let text = g.size();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(text.x.ceil(), text.y.max(18.0)), Sense::click());
    let top = rect.center().y - text.y / 2.0;
    ui.painter().galley(Pos2::new(rect.left(), top), g, col);
    let hot = resp.hovered() || resp.has_focus();
    let line = if hot { col } else { col.gamma_multiply(0.45) };
    ui.painter().hline(rect.left()..=rect.left() + text.x, (top + text.y - 1.0).round() + 0.5, Stroke::new(1.0, line));
    if resp.has_focus() {
        ui.painter().rect_stroke(rect.expand(2.0), CornerRadius::same(4), Stroke::new(2.0, col), egui::StrokeKind::Outside);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Link, true, label));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A dashed outline round `r` with corners of radius `rad` (an intentional empty slot).
fn dashed_round_rect(p: &egui::Painter, r: Rect, rad: f32, stroke: Stroke) {
    let r = r.shrink(0.5);
    let mut pts = Vec::new();
    let corners = [
        (Pos2::new(r.right() - rad, r.top() + rad), -90.0_f32),
        (Pos2::new(r.right() - rad, r.bottom() - rad), 0.0),
        (Pos2::new(r.left() + rad, r.bottom() - rad), 90.0),
        (Pos2::new(r.left() + rad, r.top() + rad), 180.0),
    ];
    for (c, a0) in corners {
        for s in 0..=6 {
            let a = (a0 + 90.0 * s as f32 / 6.0).to_radians();
            pts.push(Pos2::new(c.x + rad * a.cos(), c.y + rad * a.sin()));
        }
    }
    pts.push(pts[0]);
    p.extend(egui::Shape::dashed_line(&pts, stroke, 5.0, 4.0));
}

/// The width from which the empty bank's card is one row and its strip three columns.
const EMPTY_WIDE: f32 = 820.0;

/// The empty bank's card on a wide page: the shared empty card's parts (56 pt accent_faded disc,
/// 22 pt serif title, the 15 pt sentence in `text`, 40 tall primary and outline buttons) in one
/// row across the card's width, all vertically centred: disc and text left, the buttons ending on
/// the card's right padding. Returns each button's response in `actions` order.
fn empty_bank_card_row(ui: &mut Ui, icon: &str, title: &str, body: &str, actions: &[(&str, bool)]) -> Vec<egui::Response> {
    const DISC: f32 = 56.0;
    const GAP: f32 = 20.0;
    const BTN_GAP: f32 = 10.0;
    let mut pressed = Vec::new();
    Frame::new()
        .fill(theme::card())
        .stroke(Stroke::new(1.0, theme::border()))
        .corner_radius(CornerRadius::same(kit::R_CARD))
        .inner_margin(Margin::symmetric(28, 26))
        .show(ui, |ui| {
            let w = ui.available_width();
            ui.set_width(w);
            let btns: f32 = actions.iter().map(|(t, _)| crate::pageshell::button_width(ui, t)).sum::<f32>() + BTN_GAP * (actions.len() as f32 - 1.0);
            let text_w = (w - DISC - GAP - GAP - btns).max(120.0);
            let title = egui::WidgetText::from(theme::serif(title).size(22.0).color(theme::ink()));
            let body = egui::WidgetText::from(RichText::new(body).size(15.0).color(theme::text_col()));
            let th = title.clone().into_galley(ui, Some(egui::TextWrapMode::Wrap), text_w, egui::TextStyle::Body).size().y;
            let bh = body.clone().into_galley(ui, Some(egui::TextWrapMode::Wrap), text_w, egui::TextStyle::Body).size().y;
            const TGAP: f32 = 4.0;
            const BTN_H: f32 = 40.0;
            let h = DISC.max(th + TGAP + bh).max(BTN_H);
            let (row, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
            let disc = Rect::from_center_size(Pos2::new(row.left() + DISC / 2.0, row.center().y), Vec2::splat(DISC));
            ui.painter().circle_filled(disc.center(), DISC / 2.0, theme::accent_faded());
            crate::handicons::paint_glyph(ui.painter(), disc.center(), icon, 26.0, theme::accent_col());
            // Each part in its own column of the row, its block centred on the row's middle.
            let text_x = disc.right() + GAP;
            let text_top = (row.center().y - (th + TGAP + bh) / 2.0).round();
            let text = Rect::from_min_size(Pos2::new(text_x, text_top), Vec2::new(text_w, th + TGAP + bh));
            ui.scope_builder(egui::UiBuilder::new().max_rect(text).layout(Layout::top_down(Align::Min)), |ui| {
                ui.spacing_mut().item_spacing.y = TGAP;
                ui.add(egui::Label::new(title).wrap());
                ui.add(egui::Label::new(body).wrap());
            });
            let btn_rect = Rect::from_min_size(Pos2::new(row.right() - btns, (row.center().y - BTN_H / 2.0).round()), Vec2::new(btns, BTN_H));
            ui.scope_builder(egui::UiBuilder::new().max_rect(btn_rect).layout(Layout::left_to_right(Align::Center)), |ui| {
                ui.spacing_mut().item_spacing.x = BTN_GAP;
                for (label, primary_btn) in actions {
                    pressed.push(if *primary_btn { crate::pageshell::primary(ui, label) } else { crate::pageshell::outline(ui, label) });
                }
            });
        });
    pressed
}

/// The empty bank's three facts: (icon, title, body). The first part's body is the list of types.
const EMPTY_STRIP: [(&str, &str, &str); 3] = [
    (egui_phosphor::regular::LIST_BULLETS, "Six question types", ""),
    (egui_phosphor::regular::TAG, "Topics and difficulty", "Tag each question with topics and Easy, Medium or Hard, then filter the bank by them."),
    (crate::handicons::EXAMS, "Use it in any exam", "On an exam's page, open this bank in the Question bank panel and add its questions."),
];

/// The strip under an empty bank's card: what a bank gives you, in three parts. A quiet companion
/// to the card above it: the page's own `cream` inside a hairline (the card is the brighter `card`),
/// no buttons. Three columns split by hairlines while the strip is at least 820 wide; below that,
/// three rows, each title beside its text.
fn empty_bank_strip(ui: &mut Ui) {
    let w = ui.available_width();
    let wide = w >= EMPTY_WIDE;
    let pad = if wide { Margin::symmetric(4, 18) } else { Margin::symmetric(20, 16) };
    Frame::new()
        .fill(theme::cream())
        .stroke(Stroke::new(1.0, theme::border()))
        .corner_radius(CornerRadius::same(kit::R_CARD))
        .inner_margin(pad)
        .show(ui, |ui| {
            // The card's own width rule (the frame's margin and hairline come off the page's width),
            // so both bands end on one edge.
            ui.set_width(ui.available_width());
            // Rows as tall as their text: the default 24 pt widget row sat the type names and titles
            // lower than the bodies beside them.
            ui.spacing_mut().interact_size.y = 20.0;
            let rule = Stroke::new(1.0, theme::border());
            if wide {
                ui.columns(3, |cols| {
                    for (i, col) in cols.iter_mut().enumerate() {
                        Frame::new().inner_margin(Margin::symmetric(20, 0)).show(col, |ui| {
                            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                                strip_title(ui, i);
                                ui.add_space(6.0);
                                strip_body(ui, i);
                            });
                        });
                    }
                });
                let r = ui.min_rect();
                for k in 1..3 {
                    let x = (r.left() + r.width() * k as f32 / 3.0).round() + 0.5;
                    ui.painter().vline(x, r.top() + 2.0..=r.bottom() - 2.0, rule);
                }
            } else {
                // Rows: the title in a fixed column, its text beside it, a hairline between rows.
                let title_w = 210.0_f32.min(ui.available_width() * 0.4);
                for i in 0..3 {
                    if i > 0 {
                        ui.add_space(12.0);
                        let y = ui.cursor().top().round() + 0.5;
                        ui.painter().hline(ui.max_rect().x_range(), y, rule);
                        ui.add_space(13.0);
                    }
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 12.0;
                        ui.allocate_ui_with_layout(Vec2::new(title_w, 0.0), Layout::top_down(Align::Min), |ui| {
                            ui.set_width(title_w);
                            strip_title(ui, i);
                        });
                        let bw = ui.available_width();
                        ui.allocate_ui_with_layout(Vec2::new(bw, 0.0), Layout::top_down(Align::Min), |ui| {
                            ui.set_width(bw);
                            // the body's first line on the title's (22 tall at 17, 20 at 14)
                            ui.add_space(1.0);
                            strip_body(ui, i);
                        });
                    });
                }
            }
        });
}

/// A strip part's title: its accent glyph, then the serif card title in ink.
fn strip_title(ui: &mut Ui, i: usize) {
    let (icon, title, _) = EMPTY_STRIP[i];
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let (r, _) = ui.allocate_exact_size(Vec2::new(20.0, kit::ty::CARD_LINE), Sense::hover());
        crate::handicons::paint_glyph(ui.painter(), r.center(), icon, 18.0, theme::accent_col());
        ui.add(egui::Label::new(theme::serif(title).size(kit::ty::CARD).color(theme::ink())).wrap());
    });
}

/// A strip part's text: BODY in `text`; the first part lists the six types, each with the swatch
/// its tiles' key and bar use, in the bar's order.
fn strip_body(ui: &mut Ui, i: usize) {
    if i > 0 {
        ui.add(egui::Label::new(RichText::new(EMPTY_STRIP[i].2).size(kit::ty::BODY).color(theme::text_col()).line_height(Some(20.0))).wrap());
        return;
    }
    let mut kinds = crate::bankops::KINDS;
    kinds.sort_by_key(|k| mix_index(*k));
    // An aligned grid, two columns of three in the bar's order read across, every name starting on
    // its column's x (one column if two will not fit); the same grid at every width.
    const SW: f32 = 9.0;
    const SGAP: f32 = 6.0;
    const COL_GAP: f32 = 24.0;
    const ROW_H: f32 = 20.0;
    const ROW_GAP: f32 = 6.0;
    let texts: Vec<RichText> = kinds.iter().map(|k| RichText::new(k.label()).size(kit::ty::BODY).color(theme::text_col())).collect();
    let name_w: Vec<f32> = texts
        .iter()
        .map(|t| egui::WidgetText::from(t.clone()).into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Body).size().x.ceil())
        .collect();
    let item_w = SW + SGAP + name_w.iter().copied().fold(0.0_f32, f32::max);
    let cols = if 2.0 * item_w + COL_GAP <= ui.available_width() { 2 } else { 1 };
    let rows = kinds.len().div_ceil(cols);
    let (area, _) = ui.allocate_exact_size(Vec2::new(cols as f32 * item_w + (cols as f32 - 1.0) * COL_GAP, rows as f32 * ROW_H + (rows as f32 - 1.0) * ROW_GAP), Sense::hover());
    for (n, (k, text)) in kinds.iter().zip(texts).enumerate() {
        let (row, col) = (n / cols, n % cols);
        let at = Pos2::new(area.left() + col as f32 * (item_w + COL_GAP), area.top() + row as f32 * (ROW_H + ROW_GAP));
        let sw = Rect::from_center_size(Pos2::new(at.x + SW / 2.0, at.y + ROW_H / 2.0), Vec2::splat(SW));
        ui.painter().rect_filled(sw, CornerRadius::same(3), theme::type_hue(mix_index(*k)));
        // the label's own width, so it starts at its column's x (put centres a label in its rect)
        let tr = Rect::from_min_size(Pos2::new(at.x + SW + SGAP, at.y), Vec2::new(name_w[n], ROW_H));
        ui.put(tr, egui::Label::new(text).extend());
    }
}

/// The usage link's words ("Used in 1 exam").
fn usage_text(t: &BankTile) -> String {
    crate::bankops::usage_label(t.used_in.len())
}

/// The tile's type key at `at`, `w` wide: two 13 pt lines, each item a swatch in its type's
/// [`theme::type_hue`] (the bar segment's colour), the count in semibold ink and the type in
/// `text`. Returns the types folded into "+N more" (its swatch is hollow), and whether "+N more"
/// was pressed this frame.
fn paint_tile_key(ui: &mut Ui, at: Pos2, w: f32, mix: &[usize; 6], salt: &str) -> (Vec<usize>, bool) {
    const SW: f32 = 9.0;
    const SGAP: f32 = 5.0;
    let p = ui.painter().clone();
    let galleys: Vec<_> = (0..6).map(|i| p.layout_job(key_item_job(mix[i], KEY_LABELS[i]))).collect();
    // "+N more" is a link-style reveal: accent semibold, underlined under the pointer or focus, a
    // button that opens a list of the types it stands for; its hollow swatch is the bar's last,
    // hollow segment.
    let more_font = egui::FontId::new(kit::ty::META, theme::semibold());
    let more_g = |n: usize| p.layout_no_wrap(more_label(n), more_font.clone(), theme::accent_col());
    let key = layout_tile_key(mix, w, |i| SW + SGAP + galleys[i].size().x, |n| SW + SGAP + more_g(n).size().x);
    for &(i, x, l) in &key.items {
        let ly = at.y + l as f32 * (KEY_LINE + KEY_LGAP);
        let sr = Rect::from_center_size(Pos2::new(at.x + x + SW / 2.0, ly + KEY_LINE / 2.0), Vec2::splat(SW));
        p.rect_filled(sr, CornerRadius::same(3), theme::type_hue(i));
        let g = galleys[i].clone();
        p.galley(Pos2::new(at.x + x + SW + SGAP - key.snug, ly + (KEY_LINE - g.size().y) / 2.0), g, theme::text_col());
    }
    let Some((x, rest)) = key.more else {
        return (Vec::new(), false);
    };
    let g = more_g(rest.len());
    let ly = at.y + KEY_LINE + KEY_LGAP;
    let sr = Rect::from_center_size(Pos2::new(at.x + x + SW / 2.0, ly + KEY_LINE / 2.0), Vec2::splat(SW));
    p.rect(sr, CornerRadius::same(3), theme::card(), Stroke::new(1.0, theme::muted_col()), egui::StrokeKind::Inside);
    let r = Rect::from_min_size(Pos2::new(at.x + x + SW + SGAP, ly + (KEY_LINE - g.size().y) / 2.0), g.size());
    p.galley(r.min, g, theme::accent_col());
    let name = more_name(rest.len());
    let h = ui.interact(r.expand(2.0), ui.id().with(("tile_more", salt)), Sense::click());
    h.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
    let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&h));
    if h.hovered() || h.has_focus() || open {
        p.hline(r.x_range(), (r.bottom() - 2.0).round() + 0.5, Stroke::new(1.0, theme::accent_col()));
    }
    if h.has_focus() {
        p.rect_stroke(r.expand(3.0), CornerRadius::same(4), Stroke::new(2.0, theme::accent_col()), egui::StrokeKind::Outside);
    }
    let pressed = h.clicked();
    let h = h.on_hover_cursor(egui::CursorIcon::PointingHand);
    let h = if open { h } else { h.on_hover_text(name) };
    // The folded types, each with its swatch and count, in a small list under "+N more"; Escape or
    // a click anywhere closes it.
    egui::Popup::menu(&h).show(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.label(kit::meta_muted("Also in this bank"));
        for &i in &rest {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = SGAP;
                let (sr, _) = ui.allocate_exact_size(Vec2::new(SW, KEY_LINE), Sense::hover());
                ui.painter().rect_filled(Rect::from_center_size(sr.center(), Vec2::splat(SW)), CornerRadius::same(3), theme::type_hue(i));
                ui.label(key_item_job(mix[i], KEY_LABELS[i]));
            });
        }
    });
    (rest, pressed)
}

/// The accessible name (and hover text) of a tile key's "+N more".
fn more_name(n: usize) -> String {
    format!("Show the other {n} question type{}", if n == 1 { "" } else { "s" })
}

/// One key item's words: the count in semibold ink, then the type in `text`, both 13 pt.
fn key_item_job(n: usize, label: &str) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.append(&n.to_string(), 0.0, egui::TextFormat { font_id: egui::FontId::new(kit::ty::META, theme::semibold()), color: theme::ink(), ..Default::default() });
    job.append(&format!(" {label}"), 0.0, egui::TextFormat { font_id: egui::FontId::new(kit::ty::META, theme::sans_family()), color: theme::text_col(), ..Default::default() });
    job
}

fn bank_list_row(ui: &mut Ui, app: &mut App, ctx: &egui::Context, key: UiId, r: &BankTile) {
    let mut link: Option<Rect> = None;
    let mut menu_rect = Rect::NOTHING;
    let resp = kit::card().inner_margin(Margin::symmetric(14, 9)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let total = ui.available_width();
            ui.allocate_ui(Vec2::new((total * 0.38).floor(), 22.0), |ui| {
                ui.label(theme::serif(truncate(&r.name, 40)).size(kit::ty::CARD));
            });
            ui.allocate_ui(Vec2::new((total * 0.27).floor(), 22.0), |ui| {
                ui.label(kit::meta(counts_line(r, true)));
            });
            ui.allocate_ui(Vec2::new(90.0, 22.0), |ui| {
                paint_mix_bar(ui, 80.0, &r.mix);
            });
            ui.allocate_ui(Vec2::new(118.0, 22.0), |ui| {
                link = Some(usage_link(ui, r.used_in.len(), kit::ty::META).rect);
            });
            ui.label(kit::meta_muted(format!("Edited {}", rel_time(r.edited))));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                menu_rect = ui.allocate_exact_size(Vec2::splat(MENU_SIDE), Sense::hover()).0;
            });
        });
        crate::share::bank_links_ui::badge(ui, app, &r.id);
    });
    // The "..." sits centred on the row, also when a circle line makes it taller.
    menu_rect = Rect::from_center_size(egui::pos2(menu_rect.center().x, resp.response.rect.center().y), Vec2::splat(MENU_SIDE));
    let click = ui.interact(resp.response.rect, ui.id().with(("bank_row", &r.id)), Sense::click());
    let menu_act = bank_menu(ui, menu_rect, r);
    // (the menu is placed inside the row: the next row still starts below this one)
    ui.advance_cursor_after_rect(resp.response.rect);
    if menu_act.is_some() {
        apply_bank_menu(app, ctx, key, r, menu_act);
        return;
    }
    let on_link = click.clicked() && !r.used_in.is_empty() && link.zip(click.interact_pointer_pos()).is_some_and(|(l, p)| l.expand(2.0).contains(p));
    if on_link {
        open_usage(ctx, r.name.clone(), r.used_in.clone());
        return;
    }
    if click.hovered() {
        ui.painter().rect_stroke(
            resp.response.rect,
            CornerRadius::same(kit::R_CARD),
            Stroke::new(1.0, theme::accent_col()),
            egui::StrokeKind::Inside,
        );
    }
    if click.clicked() {
        app.open_bank(&r.id);
    }
}

/// Rename and delete-confirmation modals for the banks list.
fn draw_bank_overlays(app: &mut App, ctx: &egui::Context, key: UiId) {
    draw_bank_export(app, ctx);
    let st = with_state(ctx, key, |s: &mut BanksState| s.clone());

    // Rename.
    if let Some((id, ref rename_text)) = st.rename {
        let mut close = false;
        let mut commit = false;
        let mut commit_text = rename_text.clone();
        let modal = egui::Modal::new(UiId::new("testparrot_rename_bank")).show(ctx, |ui| {
            ui.set_min_width(360.0);
            ui.label(theme::serif("Rename bank").size(20.0));
            ui.add_space(6.0);
            let r = ui.add(TextEdit::singleline(&mut commit_text).desired_width(ui.available_width()));
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                commit = true;
            }
            ui.add_space(10.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if crate::pageshell::primary(ui, "Rename").clicked() {
                    commit = true;
                }
                if crate::pageshell::outline(ui, "Cancel").clicked() {
                    close = true;
                }
            });
        });
        if commit {
            let t = commit_text.trim().to_string();
            if !t.is_empty() {
                if let Some(b) = app.store.bank_mut(&id) {
                    b.name = t;
                    b.modified = now_secs();
                }
                app.store.save_banks();
            }
        }
        if commit || close || modal.should_close() {
            with_state(ctx, key, |s: &mut BanksState| s.rename = None);
        }
    }

    // Delete confirmation.
    if let Some(id) = st.confirm_delete.clone() {
        let bank = app.store.bank(&id).cloned();
        let mut close = false;
        let mut confirmed = false;
        let modal = egui::Modal::new(UiId::new("testparrot_delete_bank")).show(ctx, |ui| {
            ui.set_min_width(380.0);
            ui.label(theme::serif("Delete this bank?").size(20.0));
            ui.add_space(6.0);
            if let Some(b) = &bank {
                ui.label(
                    theme::text(format!(
                        "This permanently deletes \"{}\" and its {} question{}. It is removed from your \
                         machine, and exams stop being able to pull new copies of its questions.",
                        b.name,
                        b.questions.len(),
                        if b.questions.len() == 1 { "" } else { "s" },
                    ))
                    .size(12.5),
                );
            }
            ui.add_space(10.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add(
                        Button::new(RichText::new("Delete").family(theme::bold_family()).color(theme::on_accent_col()))
                            .fill(theme::bad())
                            .corner_radius(CornerRadius::same(255))
                            .min_size(Vec2::new(0.0, 34.0)),
                    )
                    .clicked()
                {
                    confirmed = true;
                }
                if crate::pageshell::outline(ui, "Cancel").clicked() {
                    close = true;
                }
            });
        });
        if confirmed {
            app.store.banks.retain(|b| b.id != id);
            app.store.forget_archived(&id);
            app.store.save_banks();
            app.toast("Question bank deleted");
            if app.bank.as_deref() == Some(id.as_str()) {
                app.screen = Screen::Banks;
                app.bank = None;
                app.qedit = None;
                app.panel_bank = None;
            }
        }
        if close || confirmed || modal.should_close() {
            with_state(ctx, key, |s: &mut BanksState| s.confirm_delete = None);
        }
    }
}

/// Saves a bank as the JSON question-bank file (asks where; the write goes through `rfd` + fs).
pub fn export_bank(app: &mut App, bank_id: &Id) {
    let Some(bank) = app.store.bank(bank_id).cloned() else {
        app.toast_err("That bank is no longer there.");
        return;
    };
    let path = crate::platform::save_path(
        &format!("{}.json", crate::bankexport::file_stem(&bank.name)),
        "Question bank JSON",
        &["json"],
    );
    let Some(path) = path else { return };
    let doc = crate::bankexport::bank_data(&bank);
    let written = serde_json::to_string_pretty(&doc)
        .map_err(|e| e.to_string())
        .and_then(|s| crate::platform::write(&path, s.as_bytes()).map_err(|e| e.to_string()));
    match written {
        Ok(()) => app.toast(format!("Exported to {}", path.display())),
        Err(e) => app.toast_err(format!("Export failed: {e}")),
    }
}

fn bank_export_key() -> UiId {
    UiId::new("qs_bank_export_dialog")
}

/// Opens the "Export question bank" dialog (preview, then PDF or JSON).
pub fn open_bank_export(ctx: &egui::Context, bank_id: &Id) {
    ctx.data_mut(|d| d.insert_temp(bank_export_key(), Some(bank_id.clone())));
}

/// The rendered bank file for the open dialog: which bank / revision, the PDF, its pages.
struct BankExportCache {
    bank: Id,
    modified: i64,
    pdf: Result<Vec<u8>, String>,
    pages: Vec<egui::TextureHandle>,
}

thread_local! {
    static BANK_EXPORT: std::cell::RefCell<Option<BankExportCache>> = const { std::cell::RefCell::new(None) };
}

/// Saves one of the plain-text interchange formats, reporting questions the format skipped.
fn save_bank_text(
    app: &mut App,
    bank: &crate::model::QuestionBank,
    suffix: &str,
    label: &str,
    make: fn(&crate::model::QuestionBank) -> (String, usize),
    close: &mut bool,
) {
    let name = format!("{}{suffix}.txt", crate::bankexport::file_stem(&bank.name));
    let Some(path) = crate::platform::save_path(&name, label, &["txt"]) else { return };
    let (text, skipped) = make(bank);
    match crate::platform::write(&path, text.as_bytes()) {
        Ok(()) => {
            let note = if skipped > 0 {
                format!(" ({skipped} question{} skipped)", if skipped == 1 { "" } else { "s" })
            } else {
                String::new()
            };
            app.toast(format!("Saved {}{note}", path.display()));
            *close = true;
        }
        Err(e) => app.toast_err(format!("Couldn't save: {e}")),
    }
}

/// Test Parrot's "Export Question Bank": a preview of the printable bank file (answers
/// included), then Download PDF, the JSON file, a spreadsheet-friendly CSV, or the Aiken /
/// Blackboard text formats.
fn draw_bank_export(app: &mut App, ctx: &egui::Context) {
    if let Ok(v) = crate::env_var("VERSHION_BANK_EXPORT") {
        let hid = UiId::new("qs_bank_export_hook");
        if !ctx.data(|d| d.get_temp::<bool>(hid).unwrap_or(false)) {
            ctx.data_mut(|d| d.insert_temp(hid, true));
            let n: usize = v.trim().parse().unwrap_or(1);
            if let Some(b) = app.store.banks.get(n.saturating_sub(1)) {
                open_bank_export(ctx, &b.id.clone());
            }
        }
    }
    let Some(bank_id) = ctx.data(|d| d.get_temp::<Option<Id>>(bank_export_key())).flatten() else { return };
    let Some(bank) = app.store.bank(&bank_id).cloned() else {
        ctx.data_mut(|d| d.insert_temp::<Option<Id>>(bank_export_key(), None));
        return;
    };
    let fresh = BANK_EXPORT.with(|c| c.borrow().as_ref().is_some_and(|c| c.bank == bank_id && c.modified == bank.modified));
    if !fresh {
        let pdf = crate::bankexport::bank_pdf(&bank);
        let mut pages = Vec::new();
        if let Ok(bytes) = &pdf {
            if let Ok(pngs) = crate::platform::render_pdf_pages(bytes) {
                for (i, png) in pngs.iter().enumerate() {
                    if let Ok(img) = image::load_from_memory(png) {
                        let img = img.thumbnail(900, 1200).to_rgba8();
                        let ci = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
                        pages.push(ctx.load_texture(format!("bank_export_page_{i}"), ci, egui::TextureOptions::LINEAR));
                    }
                }
            }
        }
        // QA hook: VERSHION_BANK_EXPORT_OUT=<file> also writes the PDF there.
        if let (Ok(out), Ok(bytes)) = (crate::env_var("VERSHION_BANK_EXPORT_OUT"), &pdf) {
            let _ = std::fs::write(out, bytes);
        }
        BANK_EXPORT.with(|c| *c.borrow_mut() = Some(BankExportCache { bank: bank_id.clone(), modified: bank.modified, pdf, pages }));
    }
    let screen = crate::theme::area_or(ctx.content_rect());
    let w = 700.0_f32.min(screen.width() - 40.0);
    let mut close = false;
    let mut save_pdf = false;
    let mut save_json = false;
    let mut save_csv = false;
    let mut save_aiken = false;
    let mut save_blackboard = false;
    let modal = egui::Modal::new(UiId::new("qs_bank_export_modal")).show(ctx, |ui| {
        ui.set_width(w);
        ui.label(theme::serif("Export question bank").size(22.0).color(theme::ink()));
        ui.add(egui::Label::new(theme::muted("For teachers: this file contains correct answers and suggested answers. It is separate from printing an exam.").size(12.5)).wrap());
        ui.add_space(8.0);
        BANK_EXPORT.with(|c| {
            let c = c.borrow();
            let Some(c) = c.as_ref() else { return };
            match &c.pdf {
                Err(e) => {
                    ui.label(RichText::new(format!("The PDF couldn't be made: {e}")).color(theme::bad()));
                }
                Ok(_) if c.pages.is_empty() => {
                    ui.label(theme::muted("Preview unavailable (pdftoppm is not installed); the PDF itself is ready.").size(12.5));
                }
                Ok(_) => {
                    Frame::new().fill(theme::field()).corner_radius(CornerRadius::same(8)).inner_margin(Margin::same(10)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        egui::ScrollArea::vertical().id_salt("bank_export_preview").max_height((screen.height() - 320.0).max(240.0)).auto_shrink([false, false]).show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                for t in &c.pages {
                                    let sz = t.size_vec2();
                                    let k = ((ui.available_width() - 8.0) / sz.x).min(1.0);
                                    ui.add(egui::Image::new(t).fit_to_exact_size(sz * k));
                                    ui.add_space(8.0);
                                }
                            });
                        });
                    });
                    ui.add_space(4.0);
                    ui.label(theme::muted(format!(
                        "{} · {} question{} · the bank's data rides along inside the PDF, so it imports back as it is",
                        plural_pages(c.pages.len()),
                        bank.questions.len(),
                        if bank.questions.len() == 1 { "" } else { "s" }
                    )).size(11.5));
                }
            }
        });
        ui.add_space(8.0);
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            if crate::pageshell::outline(ui, "Save CSV file…").on_hover_text("One row per question, for Excel, Sheets or Numbers").clicked() {
                save_csv = true;
            }
            if crate::pageshell::outline(ui, "Save Aiken file…").on_hover_text("Aiken text: single-answer multiple choice").clicked() {
                save_aiken = true;
            }
            if crate::pageshell::outline(ui, "Save Blackboard file…").on_hover_text("Tab-delimited text for Blackboard Learn's Upload Questions").clicked() {
                save_blackboard = true;
            }
            if crate::pageshell::outline(ui, "Save JSON file…").on_hover_text("The bank as a question-bank JSON file (for other apps and backups)").clicked() {
                save_json = true;
            }
        });
        // (its own row: with the four file buttons beside them the two ended up on top of each other)
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let ok = BANK_EXPORT.with(|c| c.borrow().as_ref().is_some_and(|c| c.pdf.is_ok()));
                if ui.add_enabled_ui(ok, |ui| crate::pageshell::primary(ui, "Download PDF")).inner.clicked() {
                    save_pdf = true;
                }
                if crate::pageshell::outline(ui, "Cancel").clicked() {
                    close = true;
                }
            });
        });
        ui.label(theme::muted("Aiken holds single-answer multiple choice; Blackboard holds multiple choice, true/false, fill in the blank and short answer.").size(11.0));
    });
    if save_aiken {
        save_bank_text(app, &bank, ".aiken", "Aiken text", crate::bankexport::bank_aiken, &mut close);
    }
    if save_blackboard {
        save_bank_text(app, &bank, ".blackboard", "Blackboard text", crate::bankexport::bank_blackboard, &mut close);
    }
    if save_csv {
        let path = crate::platform::save_path(
            &format!("{}.csv", crate::bankexport::file_stem(&bank.name)),
            "CSV",
            &["csv"],
        );
        if let Some(path) = path {
            match crate::platform::write(&path, crate::bankexport::bank_csv(&bank).as_bytes()) {
                Ok(()) => {
                    app.toast(format!("Saved {}", path.display()));
                    close = true;
                }
                Err(e) => app.toast_err(format!("Couldn't save: {e}")),
            }
        }
    }
    if save_json {
        export_bank(app, &bank_id);
    }
    if save_pdf {
        let path = crate::platform::save_path(
            &format!("{}.pdf", crate::bankexport::file_stem(&bank.name)),
            "PDF",
            &["pdf"],
        );
        if let Some(path) = path {
            let bytes = BANK_EXPORT.with(|c| c.borrow().as_ref().and_then(|c| c.pdf.as_ref().ok().cloned()));
            match bytes.map(|b| crate::platform::write(&path, &b)) {
                Some(Ok(())) => {
                    app.toast(format!("Saved {}", path.display()));
                    close = true;
                }
                Some(Err(e)) => app.toast_err(format!("Couldn't save: {e}")),
                None => {}
            }
        }
    }
    if close || modal.should_close() {
        ctx.data_mut(|d| d.insert_temp::<Option<Id>>(bank_export_key(), None));
        BANK_EXPORT.with(|c| *c.borrow_mut() = None);
    }
}

fn plural_pages(n: usize) -> String {
    format!("{n} page{}", if n == 1 { "" } else { "s" })
}

/// The exported bank document. Details travel only when the teacher entered some.
pub(crate) fn bank_export_json(bank: &crate::model::QuestionBank) -> serde_json::Value {
    let mut doc = serde_json::json!({
        "kind": "test-parrot-question-bank",
        "version": 1,
        "name": bank.name,
        "questions": bank.questions,
    });
    if !bank.details.is_empty() {
        doc["details"] = serde_json::to_value(bank.details.trimmed()).unwrap_or_default();
    }
    doc
}

/// "Where your work is stored" info popup; clears its flag on Close/backdrop/Esc.
impl App {
    pub(crate) fn storage_info_modal(&mut self, ctx: &egui::Context) {
        let mut close = false;
        let modal = egui::Modal::new(UiId::new("testparrot_storage_info")).show(ctx, |ui| {
            ui.set_min_width(420.0);
            ui.label(theme::serif("Where your work is stored").size(20.0));
            ui.add_space(6.0);
            ui.label(
                theme::text(
                    "Every bank, exam and export lives on your machine, in plain files you can back up:",
                )
                .size(12.5),
            );
            ui.add_space(6.0);
            Frame::new()
                .fill(theme::field())
                .corner_radius(CornerRadius::same(kit::R_FIELD))
                .inner_margin(Margin::same(8))
                .show(ui, |ui| {
                    ui.monospace(crate::store::backend_label());
                });
            ui.add_space(6.0);
            ui.label(
                theme::muted(
                    "banks.json and exams.json hold your data; media/ holds question images; \
                     exports/ holds every exam PDF and Word file ever produced.",
                )
                .size(11.5),
            );
            ui.add_space(10.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if crate::pageshell::primary(ui, "Close").clicked() {
                    close = true;
                }
            });
        });
        if close || modal.should_close() {
            self.storage_info = false;
        }
    }
}

// ---------- bank editor: outline panel (types + topics) ----------

/// One outline row: label left, count right; filled when `on`. Leaves `right_pad` free
/// at the right edge (for a menu button). Returns the row's click response.
fn side_row(ui: &mut Ui, label: &str, n: usize, on: bool, right_pad: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::click());
    let p = ui.painter();
    if on {
        p.rect_filled(rect, CornerRadius::same(8), theme::accent_faded());
        p.rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.0, theme::accent_col().gamma_multiply(0.45)), egui::StrokeKind::Inside);
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(8), theme::hover_bg());
    }
    let fg = if on { theme::accent_col() } else { theme::text_col() };
    let font = egui::FontId::new(kit::ty::META, if on { theme::bold_family() } else { egui::FontFamily::Proportional });
    let cg = p.layout_no_wrap(n.to_string(), egui::FontId::new(kit::ty::LABEL, theme::bold_family()), theme::muted_col());
    let cx = rect.right() - 10.0 - right_pad;
    let mut job = egui::text::LayoutJob::simple_singleline(label.to_string(), font, fg);
    job.wrap.max_width = (cx - cg.size().x - 8.0 - (rect.left() + 10.0)).max(20.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('…');
    let lg = p.layout_job(job);
    p.galley(Pos2::new(rect.left() + 10.0, rect.center().y - lg.size().y / 2.0), lg, fg);
    p.galley(Pos2::new(cx - cg.size().x, rect.center().y - cg.size().y / 2.0), cg, theme::muted_col());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Left panel of the bank editor: Outline (all + per type, counts) and Topics (counts,
/// click to filter, "…" menu to rename / merge / delete across the bank).
fn outline_panel(ui: &mut Ui, ctx: &egui::Context, key: UiId, questions: &[Question]) {
    let st = with_state(ctx, key, |s: &mut BankEditState| s.clone());
    ui.spacing_mut().item_spacing.y = 2.0;
    kit::label(ui, "Outline");
    ui.add_space(4.0);
    if side_row(ui, "All questions", questions.len(), st.kind.is_none() && st.topic.is_none(), 0.0).clicked() {
        with_state(ctx, key, |s: &mut BankEditState| {
            s.kind = None;
            s.topic = None;
        });
    }
    let counts = crate::bankops::type_counts(questions);
    for (k, n) in crate::bankops::KINDS.iter().copied().zip(counts) {
        if n == 0 && st.kind != Some(k) {
            continue;
        }
        let on = st.kind == Some(k);
        if side_row(ui, k.label(), n, on, 0.0).clicked() {
            with_state(ctx, key, |s: &mut BankEditState| s.kind = if on { None } else { Some(k) });
        }
    }
    ui.add_space(16.0);
    kit::label(ui, "Topics");
    ui.add_space(4.0);
    let tc = crate::bankops::topic_counts(questions);
    if tc.is_empty() {
        ui.add(
            egui::Label::new(kit::meta_muted("No topics yet. Topics added to a question are listed here.")).wrap(),
        );
        return;
    }
    for (t, n) in &tc {
        let on = st.topic.as_deref() == Some(t.as_str());
        let resp = side_row(ui, t, *n, on, 18.0);
        // "…" menu over the row's right edge — only while the row is hovered or selected, so
        // the list is not a column of stray ellipses.
        let mut act: Option<TopicMode> = None;
        let mut over_menu = false;
        if on || resp.hovered() {
            let mrect = Rect::from_min_size(
                Pos2::new(resp.rect.right() - 26.0, resp.rect.top()),
                Vec2::new(24.0, resp.rect.height()),
            );
            // A child ui that takes no room in the list (the row already has it).
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(mrect).layout(Layout::left_to_right(Align::Center)));
            child.style_mut().visuals.button_frame = false;
            over_menu = {
                let ui = &mut child;
                {
                    ui.menu_button(RichText::new("…").size(14.0).color(if on { theme::accent_col() } else { theme::muted_col() }), |ui| {
                        if ui.button("Rename…").clicked() {
                            act = Some(TopicMode::Rename);
                            ui.close();
                        }
                        if tc.len() > 1 && ui.button("Merge into…").clicked() {
                            act = Some(TopicMode::Merge);
                            ui.close();
                        }
                        if ui.button("Delete…").clicked() {
                            act = Some(TopicMode::Delete);
                            ui.close();
                        }
                    })
                    .response
                    .contains_pointer()
                }
            };
        }
        if let Some(mode) = act {
            let text = if mode == TopicMode::Rename { t.clone() } else { String::new() };
            with_state(ctx, key, |s: &mut BankEditState| s.topic_edit = Some(TopicEdit { from: t.clone(), mode, text }));
        } else if resp.clicked() && !over_menu {
            let sel = if on { None } else { Some(t.clone()) };
            with_state(ctx, key, |s: &mut BankEditState| s.topic = sel);
        }
    }
}

/// Rename / merge / delete a topic on every question of the bank.
fn draw_topic_dialog(app: &mut App, ctx: &egui::Context, key: UiId, bank_id: &Id) {
    let Some(mut te) = with_state(ctx, key, |s: &mut BankEditState| s.topic_edit.clone()) else {
        return;
    };
    let Some(bank) = app.store.bank(bank_id) else { return };
    let tc = crate::bankops::topic_counts(&bank.questions);
    let n_from = tc.iter().find(|(t, _)| *t == te.from).map(|(_, n)| *n).unwrap_or(0);
    let others: Vec<String> = tc.iter().map(|(t, _)| t.clone()).filter(|t| *t != te.from).collect();
    let qs = |n: usize| format!("{n} question{}", if n == 1 { "" } else { "s" });

    let mut close = false;
    let mut apply = false;
    let target = crate::bankops::canonical_topic(&others, &te.text);
    let merging = others.contains(&target);
    let modal = egui::Modal::new(UiId::new("bank_topic_dialog")).show(ctx, |ui| {
        ui.set_min_width(400.0);
        ui.set_max_width(440.0);
        match te.mode {
            TopicMode::Rename => {
                ui.label(theme::serif("Rename topic").size(20.0));
                ui.add_space(4.0);
                ui.add(
                    egui::Label::new(
                        theme::text(format!("“{}” is on {}. The new name replaces it on all of them.", te.from, qs(n_from))).size(12.5),
                    )
                    .wrap(),
                );
                ui.add_space(8.0);
                // Read Enter before the field runs (it may consume the key when it hands back focus).
                let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                let r = ui.add(TextEdit::singleline(&mut te.text).desired_width(ui.available_width()).hint_text("New topic name"));
                // Focus the field once when the dialog opens.
                let fid = UiId::new("bank_topic_dialog_focused");
                if !ui.ctx().data(|d| d.get_temp::<bool>(fid).unwrap_or(false)) {
                    ui.ctx().data_mut(|d| d.insert_temp(fid, true));
                    r.request_focus();
                }
                if enter && (r.lost_focus() || r.has_focus()) {
                    apply = true;
                }
                let sugg = crate::bankops::suggest_topics(&others, &te.text, &[], 6);
                if !sugg.is_empty() && te.text.trim() != te.from {
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                        ui.label(theme::muted("Existing:").size(11.5));
                        for t in sugg {
                            if kit::chip_toggle(ui, &t, false).clicked() {
                                te.text = t.clone();
                            }
                        }
                    });
                }
                if merging {
                    ui.add_space(6.0);
                    ui.add(
                        egui::Label::new(
                            theme::muted(format!("“{target}” already exists, so the two topics merge into one.")).size(12.0),
                        )
                        .wrap(),
                    );
                }
            }
            TopicMode::Merge => {
                ui.label(theme::serif(format!("Merge “{}” into…", te.from)).size(20.0));
                ui.add_space(4.0);
                ui.add(
                    egui::Label::new(
                        theme::text(format!("Its {} {} to the topic you pick; “{}” goes away.", qs(n_from), if n_from == 1 { "moves" } else { "move" }, te.from)).size(12.5),
                    )
                    .wrap(),
                );
                ui.add_space(8.0);
                egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                    for (t, n) in tc.iter().filter(|(t, _)| *t != te.from) {
                        let on = te.text == *t;
                        if side_row(ui, t, *n, on, 0.0).clicked() {
                            te.text = t.clone();
                        }
                    }
                });
            }
            TopicMode::Delete => {
                ui.label(theme::serif(format!("Delete topic “{}”?", te.from)).size(20.0));
                ui.add_space(4.0);
                ui.add(
                    egui::Label::new(
                        theme::text(format!(
                            "It is removed from {}. {}",
                            qs(n_from),
                            if n_from == 1 { "The question itself stays." } else { "The questions themselves stay." }
                        )).size(12.5),
                    )
                    .wrap(),
                );
            }
        }
        ui.add_space(12.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let ok = match te.mode {
                TopicMode::Rename => !te.text.trim().is_empty() && target != te.from,
                TopicMode::Merge => others.contains(&te.text),
                TopicMode::Delete => true,
            };
            let label = match te.mode {
                TopicMode::Rename if merging => "Merge",
                TopicMode::Rename => "Rename",
                TopicMode::Merge => "Merge",
                TopicMode::Delete => "Delete topic",
            };
            let clicked = if te.mode == TopicMode::Delete {
                ui.add(
                    Button::new(RichText::new(label).family(theme::bold_family()).color(theme::on_accent_col()))
                        .fill(theme::bad())
                        .corner_radius(CornerRadius::same(255))
                        .min_size(Vec2::new(0.0, 34.0)),
                )
                .clicked()
            } else {
                ui.add_enabled_ui(ok, |ui| crate::pageshell::primary(ui, label)).inner.clicked()
            };
            if clicked && ok {
                apply = true;
            }
            if crate::pageshell::outline(ui, "Cancel").clicked() {
                close = true;
            }
        });
    });

    if apply {
        let to = if te.mode == TopicMode::Merge { te.text.clone() } else { target.clone() };
        let valid = match te.mode {
            TopicMode::Rename => !to.trim().is_empty() && to != te.from,
            TopicMode::Merge => others.contains(&to),
            TopicMode::Delete => true,
        };
        if valid {
            let t = now_secs();
            let mut n = 0;
            if let Some(b) = app.store.bank_mut(bank_id) {
                n = match te.mode {
                    TopicMode::Delete => crate::bankops::delete_topic(&mut b.questions, &te.from),
                    _ => crate::bankops::rename_topic(&mut b.questions, &te.from, &to),
                };
                b.modified = t;
            }
            // An open draft carries its own topic list: keep it in step.
            if let Some(d) = app.qedit.as_mut().filter(|d| &d.bank_id == bank_id) {
                let mut one = vec![Question { topics: d.topics.clone(), ..placeholder_question() }];
                match te.mode {
                    TopicMode::Delete => crate::bankops::delete_topic(&mut one, &te.from),
                    _ => crate::bankops::rename_topic(&mut one, &te.from, &to),
                };
                d.topics = one.remove(0).topics;
            }
            app.store.save_banks();
            let from = te.from.clone();
            with_state(ctx, key, |s: &mut BankEditState| {
                if s.topic.as_deref() == Some(from.as_str()) {
                    s.topic = if te.mode == TopicMode::Delete { None } else { Some(to.clone()) };
                }
            });
            let qn = qs(n);
            app.toast(match te.mode {
                TopicMode::Delete => format!("Removed “{from}” from {qn}"),
                TopicMode::Merge => format!("Merged “{from}” into “{to}” on {qn}"),
                TopicMode::Rename if merging => format!("Merged “{from}” into “{to}” on {qn}"),
                TopicMode::Rename => format!("Renamed “{from}” to “{to}” on {qn}"),
            });
            close = true;
        }
    }
    if close || modal.should_close() {
        ctx.data_mut(|d| d.remove_temp::<bool>(UiId::new("bank_topic_dialog_focused")));
        with_state(ctx, key, |s: &mut BankEditState| s.topic_edit = None);
    } else {
        with_state(ctx, key, |s: &mut BankEditState| s.topic_edit = Some(te));
    }
}

/// A throwaway question (only `topics` matters) for running bankops on a draft's list.
fn placeholder_question() -> Question {
    Question {
        id: String::new(),
        stem: String::new(),
        kind: crate::model::Kind::Short { sample: String::new() },
        points: None,
        difficulty: None,
        topics: Vec::new(),
        image: None,
        explanation: None,
        figure: None,
        created: 0,
        modified: 0,
    }
}

// ---------- question cards ----------

fn diff_rank(d: Option<Difficulty>) -> u8 {
    match d {
        Some(Difficulty::Low) => 0,
        Some(Difficulty::Medium) => 1,
        Some(Difficulty::High) => 2,
        None => 3,
    }
}

fn first_topic(q: &Question) -> String {
    q.topics
        .iter()
        .map(|t| t.trim().to_lowercase())
        .min()
        .unwrap_or_else(|| "~unsorted".to_string())
}

fn paint_chevron(ui: &mut Ui, open: bool) {
    let (r, _) = ui.allocate_exact_size(Vec2::splat(15.0), Sense::hover());
    let s = Stroke::new(1.8, theme::muted_col());
    let c = r.center();
    let (a, b) = if open { (2.0, -2.0) } else { (-2.0, 2.0) };
    ui.painter().line_segment([c + Vec2::new(-4.5, a), c + Vec2::new(0.0, b)], s);
    ui.painter().line_segment([c + Vec2::new(0.0, b), c + Vec2::new(4.5, a)], s);
}

/// "Draft" / "No correct answer" on a card whose question is not finished (see `qcheck`):
/// it can be edited and saved, but not put on an exam or shared. The tooltip says what is missing.
fn draft_chip(ui: &mut Ui, q: &Question) {
    if let Some((label, tip)) = crate::qcheck::chip(q) {
        let (fg, bg) = if ui.visuals().dark_mode {
            (Color32::from_rgb(235, 175, 90), Color32::from_rgba_unmultiplied(235, 175, 90, 36))
        } else {
            (Color32::from_rgb(140, 78, 0), Color32::from_rgba_unmultiplied(196, 120, 20, 40))
        };
        chip(ui, label, bg, fg).on_hover_text(tip);
    }
}

fn meta_chip(ui: &mut Ui, text: &str) {
    chip(ui, text, theme::field(), theme::text_col());
}

/// A bank card's chip (type, topic, draft): `kit::pill`'s look with its label at the 12 pt floor.
fn chip(ui: &mut Ui, label: &str, fill: Color32, fg: Color32) -> egui::Response {
    Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(255))
        .inner_margin(Margin::symmetric(9, 1))
        .show(ui, |ui| {
            ui.add(egui::Label::new(RichText::new(label).family(theme::bold_family()).size(kit::ty::LABEL).color(fg)).wrap_mode(egui::TextWrapMode::Extend));
        })
        .response
}

/// A bank card's type chip: accent wash, accent text.
fn type_chip(ui: &mut Ui, label: &str) {
    chip(ui, label, theme::accent_faded(), theme::accent_col());
}

/// A fill-in-the-blank key as one line: each blank's accepted answers joined by " / ",
/// blanks separated by "; " (Test Parrot prints "14; 10").
pub(crate) fn blank_answer_text(accept: &[Vec<String>]) -> String {
    accept
        .iter()
        .map(|alts| alts.iter().map(|a| a.trim()).filter(|a| !a.is_empty()).collect::<Vec<_>>().join(" / "))
        .map(|t| if t.is_empty() { "—".to_string() } else { t })
        .collect::<Vec<_>>()
        .join("; ")
}

/// Dashed "Picture needed" stand-in for a `[picture]` marker on a bank card.
fn card_picture_box(ui: &mut Ui, alt: Option<&str>, size: f32) {
    let w = ui.available_width().clamp(120.0, 520.0);
    let h = size * 3.4;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    let p = ui.painter();
    let red = theme::red();
    p.rect_filled(rect, CornerRadius::same(6), red.gamma_multiply(0.08));
    p.extend(egui::Shape::dashed_line(
        &[rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom(), rect.left_top()],
        Stroke::new(1.0, red),
        4.0,
        3.0,
    ));
    let alt = alt.unwrap_or("").trim();
    let cy = if alt.is_empty() { rect.center().y } else { rect.center().y - size * 0.55 };
    p.text(Pos2::new(rect.center().x, cy), egui::Align2::CENTER_CENTER, "Picture needed", egui::FontId::new(size * 0.9, theme::bold_family()), red);
    if !alt.is_empty() {
        let max = ((w / (size * 0.42)) as usize).max(12);
        p.text(
            Pos2::new(rect.center().x, cy + size * 1.15),
            egui::Align2::CENTER_CENTER,
            truncate(alt, max),
            egui::FontId::new(size * 0.75, theme::serif_family()),
            theme::muted_col(),
        );
    }
    ui.add_space(4.0);
}

/// Rich text with its `[picture]` markers drawn as "Picture needed" boxes; the question's
/// attached picture stands in for the first marker (`picture` is taken once used).
fn card_rich(ui: &mut Ui, app: &mut App, text: &str, rich: &crate::equation::Rich, picture: &mut Option<crate::model::ImageRef>) {
    use crate::bankops::{split_pictures, StemPart};
    for part in split_pictures(text) {
        match part {
            StemPart::Text(t) => {
                crate::equation::rich_ui(ui, &t, rich);
            }
            StemPart::Picture(alt) => match picture.take() {
                Some(img) => card_image(ui, app, &img),
                None => card_picture_box(ui, alt.as_deref(), rich.size),
            },
        }
    }
}

fn card_image(ui: &mut Ui, app: &mut App, img: &crate::model::ImageRef) {
    let ctx = ui.ctx().clone();
    if let Some(tex) = texture_for(app, &ctx, img) {
        let max_w = ui.available_width().min(360.0);
        let sz = tex.size_vec2();
        let k = (max_w / sz.x.max(1.0)).min(1.0).min(220.0 / sz.y.max(1.0));
        ui.add_space(3.0);
        ui.add(egui::Image::new(&tex).fit_to_exact_size(sz * k));
        ui.add_space(3.0);
    } else {
        card_picture_box(ui, Some("picture file missing"), 13.0);
    }
}

/// "SUGGESTED ANSWER"-style block: small caps title, left rule, content.
fn answer_block(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    ui.add_space(5.0);
    let out = Frame::new().inner_margin(Margin { left: 12, right: 0, top: 1, bottom: 2 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(kit::label_text(title));
        ui.add_space(2.0);
        add(ui);
    });
    let r = out.response.rect;
    ui.painter().vline(r.left() + 1.0, r.y_range(), Stroke::new(2.0, theme::border()));
}

/// Choices with the correct ones marked (green, bold letter, check mark), as Test Parrot's
/// bank list shows them.
fn card_choices(ui: &mut Ui, app: &mut App, answers: &[crate::model::Answer], size: f32, indent: f32) {
    use crate::equation::Rich;
    for (i, a) in answers.iter().enumerate() {
        let letter = format!("{}. ", char::from(b'A' + (i % 26) as u8));
        ui.horizontal_top(|ui| {
            ui.add_space(indent);
            ui.vertical(|ui| {
                let col = if a.correct { theme::green() } else { theme::text_col() };
                let mut rich = Rich::new(size, col).lead(letter.clone(), size, if a.correct { theme::green() } else { theme::ink() });
                if a.correct {
                    rich.family = rich.bold_family.clone();
                }
                // The check mark follows the choice's last line ("A. Ca ✓").
                let text = if a.correct { format!("{} ✓", a.text.trim_end()) } else { a.text.clone() };
                let mut none = None;
                card_rich(ui, app, &text, &rich, &mut none);
            });
        });
    }
}

/// The whole question on a collapsed bank card (Test Parrot's bank list): the stem as it
/// prints (rich, pictures), choices with the correct ones marked, the fill-in answers, the
/// suggested answer, matching pairs, every multipart part with its own answers, and the
/// teacher's explanation.
fn card_full_question(ui: &mut Ui, app: &mut App, q: &Question, size: f32) {
    use crate::equation::Rich;
    use crate::model::{Kind, PartKind};
    let rich = Rich::new(size, theme::ink());
    let mut picture = q.image.clone();
    if q.stem.trim().is_empty() {
        // a draft saved before its question was written
        ui.label(theme::muted("(no question text)").size(size));
    }
    card_rich(ui, app, &q.stem, &rich, &mut picture);
    if let Some(img) = picture.take() {
        card_image(ui, app, &img);
    }
    let small = size - 1.0;
    let keyless = crate::model::is_keyless(q);
    let tf = |ui: &mut Ui, correct: bool, indent: f32| {
        ui.horizontal(|ui| {
            ui.add_space(indent);
            for (v, label) in [(true, "True"), (false, "False")] {
                // (an imported question without its key has no right answer to tick)
                let on = !keyless && v == correct;
                let t = RichText::new(if on { format!("{label} ✓") } else { label.to_string() }).size(size);
                ui.label(if on { t.color(theme::green()).family(theme::bold_family()) } else { t.color(theme::text_col()) });
                ui.add_space(10.0);
            }
        });
    };
    match &q.kind {
        Kind::Mc { answers } => {
            ui.add_space(3.0);
            card_choices(ui, app, answers, size, 2.0);
        }
        Kind::Tf { correct } => {
            ui.add_space(3.0);
            tf(ui, *correct, 2.0);
        }
        Kind::Matching { pairs, distractors } => {
            answer_block(ui, "MATCHES", |ui| {
                for (i, p) in pairs.iter().enumerate() {
                    let left = crate::equation::stem_preview(&p.left);
                    let right = crate::equation::stem_preview(&p.right);
                    ui.add(egui::Label::new(theme::text(format!("{}. {left}  →  {right}", i + 1)).size(small)).wrap());
                }
                if !distractors.is_empty() {
                    let d: Vec<String> = distractors.iter().map(|d| crate::equation::stem_preview(d)).collect();
                    ui.add(egui::Label::new(theme::muted(format!("Also listed: {}", d.join(", "))).size(small - 1.0)).wrap());
                }
            });
        }
        Kind::Blank { accept } => {
            if !accept.is_empty() {
                answer_block(ui, "ANSWERS", |ui| {
                    ui.add(egui::Label::new(theme::text(blank_answer_text(accept)).size(small)).wrap());
                });
            }
        }
        Kind::Short { sample } => {
            if !sample.trim().is_empty() {
                answer_block(ui, "SUGGESTED ANSWER", |ui| {
                    let mut none = None;
                    card_rich(ui, app, sample, &Rich::new(small, theme::text_col()), &mut none);
                });
            }
        }
        Kind::Multipart { parts } => {
            for (pi, p) in parts.iter().enumerate() {
                ui.add_space(5.0);
                let label = part_label(pi, &p.stem);
                let r = Rich::new(size, theme::ink());
                let r = if label.is_empty() { r } else { r.lead(label, size, theme::ink()) };
                ui.horizontal_top(|ui| {
                    ui.add_space(4.0);
                    ui.vertical(|ui| {
                        let mut none = None;
                        card_rich(ui, app, &p.stem, &r, &mut none);
                        match &p.kind {
                            PartKind::Mc { answers } => card_choices(ui, app, answers, small, 14.0),
                            PartKind::Tf { correct } => tf(ui, *correct, 14.0),
                            PartKind::Short { sample } => {
                                if !sample.trim().is_empty() {
                                    answer_block(ui, "SUGGESTED ANSWER", |ui| {
                                        let mut none = None;
                                        card_rich(ui, app, sample, &Rich::new(small, theme::text_col()), &mut none);
                                    });
                                }
                            }
                        }
                    });
                });
            }
        }
    }
    if let Some(e) = q.explanation.as_deref().filter(|e| !e.trim().is_empty()) {
        answer_block(ui, "EXPLANATION", |ui| {
            let mut none = None;
            card_rich(ui, app, e, &Rich::new(small, theme::text_col()), &mut none);
        });
    }
}

/// One question card. `q == None` is the unsaved new-question draft (always open).
#[allow(clippy::too_many_arguments)]
fn draw_card(
    ui: &mut Ui,
    app: &mut App,
    ctx: &egui::Context,
    key: UiId,
    bank_id: &str,
    width: f32,
    q: Option<&Question>,
    n: usize,
    list: bool,
    _pool: &[String],
    scroll_to: bool,
    min_h: f32,
) {
    // Editing happens in a modal over the list, so a card never expands in place; the one being
    // edited keeps its shape and gets an accent outline.
    let open = false;
    let editing = match q {
        None => false,
        Some(q) => app.qedit.as_ref().and_then(|d| d.qid.as_deref()) == Some(q.id.as_str()),
    };
    let mut toggled = false;
    let used = q
        .and_then(|q| ctx.data(|d| d.get_temp::<std::collections::HashMap<Id, usize>>(key.with("usage"))).and_then(|m| m.get(&q.id).copied()))
        .unwrap_or(0);
    let mut usage_rect: Option<Rect> = None;
    let mut copy_rect: Option<Rect> = None;

    let frame = kit::card()
        .stroke(Stroke::new(1.0, if editing { theme::accent_col() } else { theme::border() }));
    let out = frame.show(ui, |ui| {
        ui.set_width(width - 2.0);
        if min_h > 0.0 {
            ui.set_min_height(min_h);
        }

        // ----- header (the click target) -----
        let hdr = Frame::new().inner_margin(Margin::symmetric(13, 10)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let kind = q
                .map(|q| q.kind.qkind())
                .or_else(|| app.qedit.as_ref().map(|d| d.qkind()))
                .unwrap_or(QKind::Mc);
            if list && !open {
                let q = q.unwrap();
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    ui.allocate_ui(Vec2::new(54.0, 18.0), |ui| type_chip(ui, qedit::type_tag(kind)));
                    ui.label(RichText::new(format!("Q{n}")).family(theme::bold_family()).size(kit::ty::LABEL).color(theme::muted_col()));
                    draft_chip(ui, q);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        paint_chevron(ui, false);
                        copy_rect = Some(copy_icon(ui));
                        if let Some(p) = points_label(q.points) {
                            ui.label(RichText::new(p).family(theme::bold_family()).size(12.0).color(theme::ink()));
                        }
                        if used > 0 {
                            let r = ui.add(
                                egui::Label::new(
                                    RichText::new(format!("in {used} exam{}", if used == 1 { "" } else { "s" }))
                                        .size(kit::ty::LABEL)
                                        .color(theme::accent_col())
                                        .underline(),
                                )
                                .sense(Sense::hover()),
                            );
                            usage_rect = Some(r.rect);
                        }
                        if let Some(d) = q.difficulty {
                            ui.label(theme::muted(diff_label(d)).size(kit::ty::LABEL));
                        }
                        if let Some(t) = q.topics.iter().map(|t| t.trim()).find(|t| !t.is_empty()) {
                            meta_chip(ui, &truncate(t, 16));
                        }
                    });
                });
                ui.add_space(4.0);
                card_full_question(ui, app, q, 14.0);
            } else {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    type_chip(ui, qedit::type_tag(kind));
                    match q {
                        Some(q) => {
                            ui.label(RichText::new(format!("Q{n}")).family(theme::bold_family()).size(kit::ty::LABEL).color(theme::muted_col()));
                            draft_chip(ui, q);
                        }
                        None => {
                            ui.label(theme::serif("New question").size(15.0));
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        paint_chevron(ui, open);
                        if q.is_some() && !open {
                            copy_rect = Some(copy_icon(ui));
                        }
                        if let Some(p) = q.and_then(|q| points_label(q.points)) {
                            ui.label(RichText::new(p).family(theme::bold_family()).size(12.0).color(theme::ink()));
                        }
                    });
                });
                if let (false, Some(q)) = (open, q) {
                    ui.add_space(5.0);
                    card_full_question(ui, app, q, 14.0);
                    // Keep the topic chips on the card's bottom edge so a row of cards lines up.
                    if min_h > 0.0 {
                        let used = ui.min_rect().height();
                        let pad = (min_h - used - 24.0).max(0.0);
                        ui.add_space(pad);
                    } else {
                        ui.add_space(6.0);
                    }
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);
                        for t in q.topics.iter().map(|t| t.trim()).filter(|t| !t.is_empty()).take(2) {
                            meta_chip(ui, &truncate(t, 18));
                        }
                        if let Some(d) = q.difficulty {
                            ui.label(theme::muted(diff_label(d)).size(kit::ty::LABEL));
                        }
                        if used > 0 {
                            usage_rect = Some(usage_link(ui, used, kit::ty::LABEL).rect);
                        }
                    });
                }
            }
        });
        let hid = ui.id().with(("qhdr", q.map(|q| q.id.as_str()).unwrap_or("<new>")));
        // (round 7, critic M7; round 8, critic P; round 9, critic M5) the card's menu route is said on the card, where the
        // person looks, and it promises only what this card's menu holds: an unlinked bank's has Duplicate and Delete, and a
        // new-question card has no menu at all
        let hint = match q {
            None => None,
            Some(_) if app.shared.circles.edit.with_link(bank_id, |_| ()).is_some() => {
                Some("Right-click for Duplicate, Delete and History in the circle; Shift+F10 works from the keyboard.")
            }
            Some(_) => Some("Right-click for Duplicate and Delete; Shift+F10 works from the keyboard."),
        };
        let hr = ui.interact(hdr.response.rect, hid, Sense::click());
        let hr = if let Some(h) = hint { hr.on_hover_text(h) } else { hr };
        if hr.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if let Some(q) = q {
            // (round 6, critic) The card's menu is not mouse-only: Shift+F10 with the card focused (Tab reaches it) or under
            // the pointer opens it at the card's corner, where Tab and Enter reach "History in the circle" (hook H3).
            let keyed = ui.input(|i| i.modifiers.shift && i.key_pressed(egui::Key::F10)) && (hr.has_focus() || hr.hovered());
            let menu = if keyed { egui::Popup::context_menu(&hr).at_position(hr.rect.left_bottom()).open_memory(egui::SetOpenCommand::Bool(true)) } else { egui::Popup::context_menu(&hr) };
            menu.show(|ui| {
                if ui.button("Duplicate").clicked() {
                    duplicate_question(app, bank_id, &q.id);
                    ui.close();
                }
                if ui.button("Delete").clicked() {
                    if open {
                        app.qedit = None;
                    }
                    delete_question(app, ctx, key, bank_id, &q.id);
                    ui.close();
                }
                crate::share::question_history_ui::question_menu(ui, app, bank_id, &q.id);
            });
        }
        if hr.clicked() {
            let on_usage = usage_rect.zip(hr.interact_pointer_pos()).is_some_and(|(r, p)| r.expand(2.0).contains(p));
            let on_copy = copy_rect.zip(hr.interact_pointer_pos()).is_some_and(|(r, p)| r.contains(p));
            match q {
                Some(q) if on_copy => duplicate_question(app, bank_id, &q.id),
                Some(q) if on_usage => {
                    let ids: Vec<Id> = crate::bankops::exams_using_question(&app.store.exams, bank_id, &q.id).iter().map(|e| e.id.clone()).collect();
                    open_usage(ctx, format!("Q{n} · {}", truncate(&crate::equation::stem_preview(&q.stem), 40)), ids);
                }
                _ => toggled = true,
            }
        }
        if open && scroll_to {
            hr.scroll_to_me(Some(Align::TOP));
        }
    });

    if !open && out.response.hovered() {
        ui.painter().rect_stroke(
            out.response.rect,
            CornerRadius::same(kit::R_CARD),
            Stroke::new(1.0, theme::accent_col()),
            egui::StrokeKind::Inside,
        );
    }

    if toggled {
        if open {
            // Collapse = Cancel.
            app.qedit = None;
        } else if let Some(q) = q {
            app.qedit = Some(qedit::Draft::from_question(bank_id.to_string(), q));
        }
    }
}

/// Copies question `qid` of `bank` (Test Parrot's Copy): same content under a fresh id
/// and fresh timestamps, inserted right after the original. Returns the copy's id.
pub(crate) fn duplicate_in_bank(bank: &mut crate::model::QuestionBank, qid: &str) -> Option<Id> {
    let idx = bank.questions.iter().position(|x| x.id == qid)?;
    let now = now_secs();
    let copy = Question { id: crate::model::new_id(), created: now, modified: now, ..bank.questions[idx].clone() };
    let id = copy.id.clone();
    bank.questions.insert(idx + 1, copy);
    bank.modified = now;
    Some(id)
}

/// Duplicates a stored question in place (bank card Copy button / right-click menu).
fn duplicate_question(app: &mut App, bank_id: &str, qid: &str) {
    let done = app.store.bank_mut(&bank_id.to_string()).and_then(|b| duplicate_in_bank(b, qid)).is_some();
    if done {
        app.store.save_banks();
    }
}

/// The card's Copy affordance: a small two-sheets icon. Only paints and reports its rect;
/// the header's click handler routes a click inside it (the header owns the clicks).
fn copy_icon(ui: &mut Ui) -> Rect {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(20.0, 20.0), Sense::hover());
    let hot = resp.hovered();
    let col = if hot { theme::accent_col() } else { theme::muted_col() };
    if hot {
        ui.painter().rect_filled(r, CornerRadius::same(5), theme::accent_faded());
    }
    ui.painter().text(
        r.center(),
        egui::Align2::CENTER_CENTER,
        egui_phosphor::regular::COPY_SIMPLE,
        theme::icon_font(14.0),
        col,
    );
    resp.on_hover_text("Copy question");
    r
}

/// Removes a stored question and parks it in the Undo strip.
fn delete_question(app: &mut App, ctx: &egui::Context, key: UiId, bank_id: &str, qid: &str) {
    let mut removed: Option<(Question, usize)> = None;
    if let Some(bank) = app.store.bank_mut(&bank_id.to_string()) {
        if let Some(idx) = bank.questions.iter().position(|x| x.id == qid) {
            removed = Some((bank.questions.remove(idx), idx));
            bank.modified = now_secs();
        }
    }
    if let Some(r) = removed {
        app.store.save_banks();
        with_state(ctx, key, |s: &mut BankEditState| s.last_deleted = Some(r));
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max_chars).collect();
        out.push('…');
        out
    }
}

// ---------- tests ----------

// ---------- bank details, rename pencil, Pop-over window ----------

/// The rename affordance after the bank name: the kit's outlined icon button (the header's
/// button family, a size down from its 40 tall actions), "Rename this bank" its tip and name.
fn pencil_button(ui: &mut Ui) -> egui::Response {
    kit::icon_button(ui, crate::handicons::EDIT, PENCIL_SIDE, "Rename this bank")
}

/// The rename button's side.
const PENCIL_SIDE: f32 = 32.0;
/// Room after the name for the rename button: the gap before it and its side.
const PENCIL_ROOM: f32 = 10.0 + PENCIL_SIDE;

/// Width of the bank toolbar's overflow button.
const MORE_W: f32 = 40.0;
/// The bank toolbar's button height.
const TOOL_H: f32 = 40.0;
/// Side padding of a quiet toolbar button (Details, Preview): its hover wash reaches this far
/// past the icon and the label.
const QUIET_PAD: f32 = 12.0;
/// Corner radius of the segmented Details / Preview / … group (Share's 12).
const QUIET_R: u8 = 12;

/// How far the page's scroll bar reaches in from the right at its widest, plus a visible gap.
fn scroll_bar_reach(ui: &Ui) -> f32 {
    let s = &ui.spacing().scroll;
    s.bar_inner_margin + s.bar_width + s.bar_outer_margin + 8.0
}

/// The Banks grid's content width in `avail` (its wide cap), for a page that ends on its edge.
pub(crate) fn banks_page_w(avail: f32) -> f32 {
    avail.min(BANKS_MAX_W)
}

/// The bank page's content width in `avail`: the Banks grid's cap, and while the page scrolls
/// (`bar` the scroll bar's reach) short of the bar, so cards, the view toggle and the header's
/// actions end on one edge clear of it. Never narrower than the reading width the page had
/// before it widened (or `avail`), so a narrow window is unchanged.
fn bank_page_w(avail: f32, bar: Option<f32>) -> f32 {
    let w = avail.min(BANKS_MAX_W);
    match bar {
        Some(b) => w.min((avail - b).max(crate::pageshell::PAGE_W.min(avail))),
        None => w,
    }
}

/// The empty bank header's labelled menu button: "Bank actions" and a caret.
fn bank_actions_label() -> String {
    format!("Bank actions {}", egui_phosphor::regular::CARET_DOWN)
}

/// Width of a quiet toolbar button with a 16 pt icon and `label`.
fn quiet_tool_w(ui: &Ui, label: &str) -> f32 {
    let text = ui.fonts_mut(|f| f.layout_no_wrap(label.to_string(), egui::FontId::new(kit::ty::BUTTON, theme::semibold()), Color32::WHITE).size().x);
    (QUIET_PAD * 2.0 + 16.0 + 7.0 + text).ceil()
}

/// One segment of the bank toolbar's quiet group (Details, Preview, the overflow). The toolbar
/// draws the group's container (paper fill, 1 pt `edge`, radius [`QUIET_R`]) and the hairlines
/// between segments; the segment adds, inside its corners `r`, a hover_bg wash under the pointer
/// or keyboard focus, accent_faded while pressed or while its menu is open, and the 2 pt focus
/// ring. `icon`: a 16 pt ink glyph before `label`, in ink semibold (Share is bold on card white,
/// so the group stays a step below it); `label` None draws the three overflow dots in a `TOOL_H`
/// square. `name` is the accessible name (and, for the dots, the hover text).
fn quiet_tool(ui: &mut Ui, icon: Option<&str>, label: Option<&str>, name: &str, r: CornerRadius) -> egui::Response {
    let w = match label {
        // Without an icon the label alone sits between the side pads.
        Some(l) => quiet_tool_w(ui, l) - if icon.is_none() { 16.0 + 7.0 } else { 0.0 },
        None => MORE_W,
    };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, TOOL_H), Sense::click());
    let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp));
    let p = ui.painter();
    // The group's outline is drawn over the segments afterwards, so a wash fills to it.
    if resp.is_pointer_button_down_on() || open {
        p.rect_filled(rect, r, theme::accent_faded());
    } else if resp.hovered() || resp.has_focus() {
        p.rect_filled(rect, r, theme::hover_bg());
    }
    let fg = theme::ink();
    match label {
        Some(l) => {
            let galley = p.layout_no_wrap(l.to_owned(), egui::FontId::new(kit::ty::BUTTON, theme::semibold()), fg);
            let mut x = rect.left() + QUIET_PAD;
            if let Some(icon) = icon {
                crate::handicons::paint_glyph(p, Pos2::new(x + 8.0, rect.center().y), icon, 16.0, fg);
                x += 16.0 + 7.0;
            }
            p.galley(Pos2::new(x, rect.center().y - galley.size().y / 2.0), galley, fg);
        }
        None => {
            for dx in [-6.5, 0.0, 6.5] {
                p.circle_filled(rect.center() + Vec2::new(dx, 0.0), 2.0, fg);
            }
        }
    }
    if resp.has_focus() {
        let ring = CornerRadius { nw: r.nw.saturating_add(1), ne: r.ne.saturating_add(1), sw: r.sw.saturating_add(1), se: r.se.saturating_add(1) };
        ui.painter().rect_stroke(rect.expand(1.0), ring, Stroke::new(2.0, theme::accent_col()), egui::StrokeKind::Outside);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    if label.is_none() { resp.on_hover_text(name) } else { resp }
}

/// Writes edited details onto a bank; returns true when anything changed.
pub(crate) fn apply_details(bank: &mut crate::model::QuestionBank, draft: &crate::model::BankDetails, now: i64) -> bool {
    let d = draft.trimmed();
    if bank.details == d {
        return false;
    }
    bank.details = d;
    bank.modified = now;
    true
}

/// Test Parrot's "Question Bank details" dialog: description, declared author, license.
fn draw_details_dialog(app: &mut App, ctx: &egui::Context, key: UiId, bank_id: &Id) {
    let Some(mut d) = with_state(ctx, key, |s: &mut BankEditState| s.details.clone()) else {
        return;
    };
    let mut close = false;
    let mut save = false;
    let modal = egui::Modal::new(UiId::new("bank_details_dialog")).show(ctx, |ui| {
      Frame::new().inner_margin(Margin::symmetric(14, 10)).show(ui, |ui| {
        ui.set_min_width(520.0);
        ui.set_max_width(560.0);
        ui.label(theme::serif("Question bank details").size(22.0));
        ui.add_space(4.0);
        ui.label(theme::text("Only details you enter are included when this bank is exported and shared.").size(12.5));
        ui.add_space(10.0);
        let w = ui.available_width();
        let field = |ui: &mut Ui, label: &str| {
            ui.label(theme::strong(label).size(13.0));
            ui.add_space(2.0);
        };
        field(ui, "Description");
        ui.add(TextEdit::multiline(&mut d.description).desired_rows(4).desired_width(w).hint_text("What these questions cover, grade level, …"));
        ui.add_space(8.0);
        field(ui, "Declared author");
        ui.add(TextEdit::singleline(&mut d.author).desired_width(w));
        ui.add_space(8.0);
        field(ui, "License name");
        ui.add(TextEdit::singleline(&mut d.license_name).desired_width(w).hint_text("e.g. CC BY 4.0"));
        ui.add_space(8.0);
        field(ui, "License URL");
        ui.add(TextEdit::singleline(&mut d.license_url).desired_width(w).hint_text("https://"));
        ui.add_space(12.0);
        ui.separator();
        ui.add_space(6.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if crate::pageshell::primary(ui, "Save details").clicked() {
                save = true;
            }
            if crate::pageshell::outline(ui, "Cancel").clicked() {
                close = true;
            }
        });
      });
    });
    if save {
        let changed = app.store.bank_mut(bank_id).is_some_and(|b| apply_details(b, &d, now_secs()));
        if changed {
            app.store.save_banks();
            app.toast("Details saved");
        }
    }
    if save || close || modal.should_close() {
        with_state(ctx, key, |s: &mut BankEditState| s.details = None);
    } else {
        with_state(ctx, key, |s: &mut BankEditState| s.details = Some(d));
    }
}

/// "Tidy stems": before / after of every question tidying would change, a tick per question,
/// and Apply (after a backup of banks.json; the bank page then offers Undo).
fn draw_tidy_dialog(app: &mut App, ctx: &egui::Context, key: UiId, bank_id: &Id) {
    use crate::equation::{rich_ui, Rich};
    use crate::model::Kind;
    let Some(mut td) = with_state(ctx, key, |s: &mut BankEditState| s.tidy.clone()) else {
        return;
    };
    let mut close = false;
    let mut apply = false;
    let screen = crate::theme::area_or(ctx.content_rect());
    let w = (screen.width() - 120.0).clamp(420.0, 940.0);
    let modal = egui::Modal::new(UiId::new("bank_tidy_dialog")).show(ctx, |ui| {
        Frame::new().inner_margin(Margin::symmetric(14, 10)).show(ui, |ui| {
            ui.set_width(w);
            ui.label(theme::serif("Tidy stems").size(22.0));
            ui.add_space(4.0);
            let n = td.items.len();
            if n == 0 {
                ui.add(egui::Label::new(theme::text("Every stem in this bank is already tidy. Nothing to change.").size(12.5)).wrap());
            } else {
                ui.add(
                    egui::Label::new(
                        theme::text(format!(
                            "{n} question{} from older imports can be cleaned up: tables of blanks, line breaks, wrapped lines and spacing. Nothing changes until you apply, and fill-in-the-blank answers stay with their blanks.",
                            if n == 1 { "" } else { "s" }
                        ))
                        .size(12.5),
                    )
                    .wrap(),
                );
            }
            ui.add_space(10.0);
            egui::ScrollArea::vertical().max_height((screen.height() - 260.0).max(200.0)).auto_shrink([false, true]).show(ui, |ui| {
                // Keep the cards clear of the scroll bar.
                ui.set_max_width(ui.available_width() - 12.0);
                for (i, t) in td.items.iter().enumerate() {
                    kit::card().inner_margin(Margin::symmetric(12, 9)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut td.picked[i], theme::strong(format!("Question {}", t.index + 1)).size(13.5));
                            ui.label(kit::small_caps_text(&t.before.kind.qkind().label().to_uppercase()));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let fixes: Vec<&str> = t.fixes.iter().map(|f| f.label()).collect();
                                ui.label(theme::muted(fixes.join(" · ")).size(11.5));
                            });
                        });
                        for f in &t.fields {
                            ui.add_space(6.0);
                            if t.fields.len() > 1 || f.label != "Stem" {
                                ui.label(kit::small_caps_text(&f.label.to_uppercase()));
                                ui.add_space(2.0);
                            }
                            ui.columns(2, |cols| {
                                for (c, (head, text)) in cols.iter_mut().zip([("Before", &f.before), ("After", &f.after)]) {
                                    c.label(theme::muted(head).size(11.0));
                                    Frame::new()
                                        .fill(if head == "After" { theme::accent_col().gamma_multiply(0.06) } else { Color32::TRANSPARENT })
                                        .stroke(Stroke::new(1.0, theme::border()))
                                        .corner_radius(CornerRadius::same(6))
                                        .inner_margin(Margin::symmetric(8, 6))
                                        .show(c, |ui| {
                                            ui.set_width(ui.available_width());
                                            rich_ui(ui, text, &Rich::new(13.0, theme::text_col()));
                                        });
                                }
                            });
                        }
                        if let (Kind::Blank { accept: a }, Kind::Blank { accept: b }) = (&t.before.kind, &t.after.kind)
                            && a != b
                        {
                            ui.add_space(4.0);
                            ui.label(
                                theme::muted(format!(
                                    "Answers: {} entries for {} blanks → {} for {} (the header row's stray blanks go; every answer stays with its blank)",
                                    a.len(),
                                    t.before.stem.matches("___").count(),
                                    b.len(),
                                    t.after.stem.matches("___").count(),
                                ))
                                .size(11.5),
                            );
                        }
                        if let Some(note) = &t.note {
                            ui.add_space(4.0);
                            ui.add(egui::Label::new(RichText::new(note).size(11.5).color(theme::bad())).wrap());
                        }
                    });
                    ui.add_space(8.0);
                }
            });
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);
            let k = td.picked.iter().filter(|p| **p).count();
            ui.horizontal(|ui| {
                if n > 0 {
                    ui.label(theme::muted(format!("{k} of {n} selected")).size(12.0));
                    if ui.add(Button::new(RichText::new("All").size(12.0).color(theme::accent_col())).frame(false)).clicked() {
                        td.picked.iter_mut().for_each(|p| *p = true);
                    }
                    if ui.add(Button::new(RichText::new("None").size(12.0).color(theme::accent_col())).frame(false)).clicked() {
                        td.picked.iter_mut().for_each(|p| *p = false);
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if n > 0 {
                        let label = format!("Apply to {k} question{}", if k == 1 { "" } else { "s" });
                        if ui.add_enabled_ui(k > 0, |ui| crate::pageshell::primary(ui, &label)).inner.clicked() && k > 0 {
                            apply = true;
                        }
                    }
                    if crate::pageshell::outline(ui, if n == 0 { "Close" } else { "Cancel" }).clicked() {
                        close = true;
                    }
                });
            });
            if n > 0 {
                ui.add_space(4.0);
                ui.add(
                    egui::Label::new(
                        theme::muted("Your banks are backed up first (backups folder in the store). Undo then appears on the bank page.").size(11.5),
                    )
                    .wrap(),
                );
            }
        });
    });
    if apply {
        let picked: Vec<&crate::tidy::QuestionTidy> = td.items.iter().zip(&td.picked).filter(|(_, p)| **p).map(|(t, _)| t).collect();
        match crate::store::backup_banks(&app.store.banks) {
            Err(e) => app.toast_err(format!("Couldn't back up the banks, so nothing was changed: {e}")),
            Ok(path) => {
                let t = now_secs();
                let before = app
                    .store
                    .bank_mut(bank_id)
                    .map(|b| {
                        let u = crate::tidy::apply(&mut b.questions, &picked, t);
                        if !u.is_empty() {
                            b.modified = t;
                        }
                        u
                    })
                    .unwrap_or_default();
                let n = before.len();
                if n > 0 {
                    app.store.save_banks();
                    let backup = path.display().to_string();
                    with_state(ctx, key, |s: &mut BankEditState| s.last_tidy = Some(TidyUndo { before, backup }));
                    app.toast(format!("Tidied {n} question{}", if n == 1 { "" } else { "s" }));
                } else {
                    app.toast_err("Nothing was tidied: those questions changed since the preview");
                }
            }
        }
    }
    if apply || close || modal.should_close() {
        with_state(ctx, key, |s: &mut BankEditState| s.tidy = None);
    } else {
        with_state(ctx, key, |s: &mut BankEditState| s.tidy = Some(td));
    }
}

fn popover_key() -> UiId {
    UiId::new("qs_bank_popover")
}

/// Opens (or re-targets) the bank Pop-over window.
pub fn open_popover(ctx: &egui::Context, bank_id: &Id) {
    ctx.data_mut(|d| d.insert_temp(popover_key(), Some(bank_id.clone())));
}

#[derive(Clone, Default)]
struct PopoverState {
    search: String,
    /// Open banks (tabs), in order; the active one is the Pop-over key's bank.
    tabs: Vec<Id>,
    /// Filters row shown.
    filters: bool,
    /// Type filter (empty = every type).
    kinds: Vec<QKind>,
    diff: Option<Difficulty>,
    topic: Option<String>,
    sort: Sort,
    /// Answer lines chosen per question (copied with it and used when it is added to an exam).
    lines: std::collections::HashMap<String, u8>,
    /// Copy math as LaTeX `$…$` instead of plain Unicode text.
    latex: bool,
    settings: bool,
    /// Answer columns chosen per multiple-choice question (1, 2 or 4; shown, copied, and
    /// kept when it is added to an exam).
    cols: std::collections::HashMap<String, u8>,
}

/// Answer-line choices on the Pop-over cards (as on the reference).
pub(crate) const ANSWER_LINES: [u8; 5] = [0, 2, 4, 6, 10];

fn lines_label(n: u8) -> String {
    if n == 0 { "No lines".to_string() } else { format!("{n} lines") }
}

/// Indices of `qs` the Pop-over lists: search over the stem text, type / difficulty / topic
/// filters, then the sort.
pub(crate) fn popover_order(qs: &[Question], search: &str, kinds: &[QKind], diff: Option<Difficulty>, topic: Option<&str>, sort: Sort) -> Vec<usize> {
    let query = search.trim().to_lowercase();
    let mut order: Vec<usize> = qs
        .iter()
        .enumerate()
        .filter(|(_, q)| {
            (query.is_empty() || q.stem.to_lowercase().contains(&query) || crate::equation::stem_preview(&q.stem).to_lowercase().contains(&query))
                && (kinds.is_empty() || kinds.contains(&q.kind.qkind()))
                && diff.is_none_or(|d| Some(d) == q.difficulty)
                && topic.is_none_or(|t| q.topics.iter().any(|x| x.trim() == t))
        })
        .map(|(i, _)| i)
        .collect();
    order.sort_by(|&a, &b| {
        let (qa, qb) = (&qs[a], &qs[b]);
        match sort {
            Sort::Newest => qb.created.cmp(&qa.created),
            Sort::Oldest => qa.created.cmp(&qb.created),
            Sort::Kind => mix_index(qa.kind.qkind()).cmp(&mix_index(qb.kind.qkind())).then_with(|| qb.created.cmp(&qa.created)),
            Sort::Difficulty => diff_rank(qa.difficulty).cmp(&diff_rank(qb.difficulty)).then_with(|| qb.created.cmp(&qa.created)),
            Sort::Topic => first_topic(qa).cmp(&first_topic(qb)).then_with(|| qb.created.cmp(&qa.created)),
        }
    });
    order
}

/// Lines of stem markup as plain text: lists keep their markers, tables their ` | ` cells,
/// side-by-side panels follow each other; math as `$…$` (`latex`) or its Unicode text.
fn markup_lines(text: &str, latex: bool) -> Vec<String> {
    use crate::richtext::{line_text, parse, Block};
    fn walk(blocks: &[Block], out: &mut Vec<String>) {
        for (k, b) in blocks.iter().enumerate() {
            if k > 0 && !out.last().is_some_and(|l| l.is_empty()) && matches!(b, Block::Heading(..) | Block::Table { .. }) {
                out.push(String::new());
            }
            match b {
                Block::Para(ls) => out.extend(ls.iter().map(line_text)),
                Block::Bullets(ls) => out.extend(ls.iter().map(|l| format!("• {}", line_text(l)))),
                Block::Numbered(ls) => out.extend(ls.iter().enumerate().map(|(i, l)| format!("{}. {}", i + 1, line_text(l)))),
                Block::Quote(ls) => out.extend(ls.iter().map(|l| format!("> {}", line_text(l)))),
                Block::Table { rows, .. } => out.extend(rows.iter().map(|r| r.iter().map(line_text).collect::<Vec<_>>().join(" | "))),
                Block::Heading(_, l) => out.push(line_text(l)),
                Block::Divider => out.push("———".to_string()),
                Block::Side(panels) => {
                    for p in panels {
                        walk(p, out);
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(&parse(text), &mut out);
    if latex { out } else { out.into_iter().map(|l| crate::equation_text::stem_to_plain(&l)).collect() }
}

/// A question as text for the clipboard: stem, choices (A. B. ...), matching columns, parts,
/// and `lines` answer lines under a short answer (or each short part). Test-only convenience
/// wrapper; the UI calls [`popover_copy_text_cols`] with the chosen column count.
#[cfg(test)]
pub(crate) fn popover_copy_text(q: &Question, lines: u8, latex: bool) -> String {
    popover_copy_text_cols(q, lines, latex, 1)
}

/// Choice texts laid out in `cols` columns (column-major, as the exam prints them): one line
/// per row, each column padded to its widest entry.
fn choice_rows(items: &[String], cols: u8) -> Vec<String> {
    if cols <= 1 || items.len() < 2 {
        return items.to_vec();
    }
    let grid = crate::layout::choice_grid(items.len(), cols);
    let ncols = grid.iter().map(|r| r.len()).max().unwrap_or(1);
    let width: Vec<usize> = (0..ncols).map(|c| grid.iter().filter_map(|r| r.get(c)).map(|&i| items[i].chars().count()).max().unwrap_or(0)).collect();
    grid.iter()
        .map(|r| {
            let cells: Vec<String> = r.iter().enumerate().map(|(c, &i)| format!("{:<w$}", items[i], w = width[c])).collect();
            cells.join("    ").trim_end().to_string()
        })
        .collect()
}

/// [`popover_copy_text`] with the multiple-choice answers in `cols` columns.
pub(crate) fn popover_copy_text_cols(q: &Question, lines: u8, latex: bool, cols: u8) -> String {
    use crate::model::{Kind, PartKind};
    let one = |t: &str| markup_lines(t, latex).join(" ");
    let rule = "_".repeat(48);
    let mut out = markup_lines(&q.stem, latex);
    let push_lines = |out: &mut Vec<String>| {
        for _ in 0..lines {
            out.push(rule.clone());
        }
    };
    let abc = |i: usize| char::from(b'A' + (i % 26) as u8);
    match &q.kind {
        Kind::Mc { answers } => {
            let items: Vec<String> = answers.iter().enumerate().map(|(i, a)| format!("{}. {}", abc(i), one(&a.text))).collect();
            out.extend(choice_rows(&items, cols));
        }
        Kind::Tf { .. } => out.push("True / False".to_string()),
        Kind::Matching { pairs, distractors } => {
            out.extend(pairs.iter().enumerate().map(|(i, p)| format!("{}. {}", i + 1, one(&p.left))));
            let rights: Vec<&String> = pairs.iter().map(|p| &p.right).chain(distractors.iter()).collect();
            out.extend(rights.iter().enumerate().map(|(i, r)| format!("{}. {}", abc(i), one(r))));
        }
        Kind::Blank { .. } => {}
        Kind::Short { .. } => push_lines(&mut out),
        Kind::Multipart { parts } => {
            for (pi, p) in parts.iter().enumerate() {
                let pl = markup_lines(&p.stem, latex);
                out.push(format!("{}{}", part_label(pi, &p.stem), pl.first().cloned().unwrap_or_default()));
                out.extend(pl.into_iter().skip(1));
                match &p.kind {
                    PartKind::Mc { answers } => out.extend(answers.iter().enumerate().map(|(i, a)| format!("   {}. {}", abc(i), one(&a.text)))),
                    PartKind::Tf { .. } => out.push("   True / False".to_string()),
                    PartKind::Short { .. } => push_lines(&mut out),
                }
            }
        }
    }
    out.join("\n")
}

/// "a. " for part `pi`, or nothing when the part's text already starts with its own label
/// ("a. NH₃", "b) ...").
fn part_label(pi: usize, stem: &str) -> String {
    let t = stem.trim_start();
    let mut c = t.chars();
    let own = matches!((c.next(), c.next()), (Some(l), Some('.' | ')')) if l.is_ascii_alphabetic());
    if own { String::new() } else { format!("{}. ", char::from(b'a' + (pi % 26) as u8)) }
}

/// Does the question take answer lines (a short answer, or a multipart with a short part)?
fn takes_lines(q: &Question) -> bool {
    use crate::model::{Kind, PartKind};
    match &q.kind {
        Kind::Short { .. } => true,
        Kind::Multipart { parts } => parts.iter().any(|p| matches!(p.kind, PartKind::Short { .. })),
        _ => false,
    }
}

/// One Pop-over card: type label, answer-lines choice, the stem as it prints (rich), choices
/// or parts, and the actions.
#[allow(clippy::too_many_arguments)]
fn popover_card(ui: &mut Ui, q: &Question, n: usize, ps: &mut PopoverState, exam_id: Option<&Id>, in_exam: bool, add: &mut Option<Id>, edit: &mut Option<Id>) {
    use crate::equation::{rich_ui, Rich};
    use crate::model::{Kind, PartKind};
    let size = 13.0;
    kit::card().inner_margin(Margin::symmetric(12, 9)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let mut lines = ps.lines.get(&q.id).copied().unwrap_or(0);
        ui.horizontal(|ui| {
            ui.label(theme::muted(format!("{n}.")).size(11.5));
            ui.label(kit::small_caps_text(&q.kind.qkind().label().to_uppercase()));
            if let Some(p) = points_label(q.points) {
                ui.label(theme::muted(p).size(11.0));
            }
            if let Kind::Mc { .. } = &q.kind {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let mut c = ps.cols.get(&q.id).copied().unwrap_or(1);
                    let r = crate::examui::answer_columns_switch(ui, &mut c).on_hover_text("Answer columns: how the choices are laid out, copied and printed when added to an exam");
                    if r.changed() || c != ps.cols.get(&q.id).copied().unwrap_or(1) {
                        if c <= 1 {
                            ps.cols.remove(&q.id);
                        } else {
                            ps.cols.insert(q.id.clone(), c);
                        }
                    }
                });
            }
            if takes_lines(q) {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ComboBox::from_id_salt(("pop_lines", &q.id)).width(84.0).selected_text(RichText::new(lines_label(lines)).size(11.5)).show_ui(ui, |ui| {
                        for v in ANSWER_LINES {
                            ui.selectable_value(&mut lines, v, lines_label(v));
                        }
                    })
                    .response
                    .on_hover_text("Answer lines: copied under the question, and the work space it gets when added to an exam");
                });
            }
        });
        if lines == 0 {
            ps.lines.remove(&q.id);
        } else {
            ps.lines.insert(q.id.clone(), lines);
        }
        ui.add_space(2.0);
        let rich = Rich::new(size, theme::text_col());
        rich_ui(ui, &q.stem, &rich);
        let abc = |i: usize| format!("{}. ", char::from(b'A' + (i % 26) as u8));
        let sep = |ui: &mut Ui| {
            ui.add_space(4.0);
            let r = ui.available_rect_before_wrap();
            ui.painter().extend(egui::Shape::dashed_line(&[Pos2::new(r.left(), r.top()), Pos2::new(r.right(), r.top())], Stroke::new(1.0, theme::border()), 3.0, 3.0));
            ui.add_space(4.0);
        };
        match &q.kind {
            Kind::Mc { answers } => {
                sep(ui);
                let cols = ps.cols.get(&q.id).copied().unwrap_or(1);
                if cols <= 1 {
                    for (i, a) in answers.iter().enumerate() {
                        rich_ui(ui, &a.text, &Rich::new(size, theme::text_col()).lead(abc(i), size, theme::ink()));
                    }
                } else {
                    // Column-major grid of equal-width columns (A B down the first column).
                    let grid = crate::layout::choice_grid(answers.len(), cols);
                    let ncols = grid.iter().map(|r| r.len()).max().unwrap_or(1).max(1);
                    let cw = (ui.available_width() / ncols as f32).floor();
                    for row in grid {
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            for i in row {
                                ui.allocate_ui_with_layout(Vec2::new(cw, 0.0), Layout::top_down(Align::Min), |ui| {
                                    ui.set_width(cw - 6.0);
                                    rich_ui(ui, &answers[i].text, &Rich::new(size, theme::text_col()).lead(abc(i), size, theme::ink()));
                                });
                            }
                        });
                    }
                }
            }
            Kind::Tf { .. } => {
                sep(ui);
                ui.label(theme::text("True  /  False").size(size));
            }
            Kind::Matching { pairs, distractors } => {
                sep(ui);
                for (i, p) in pairs.iter().enumerate() {
                    rich_ui(ui, &p.left, &Rich::new(size, theme::text_col()).lead(format!("{}. ", i + 1), size, theme::ink()));
                }
                ui.add_space(3.0);
                for (i, r) in pairs.iter().map(|p| &p.right).chain(distractors.iter()).enumerate() {
                    rich_ui(ui, r, &Rich::new(size, theme::text_col()).lead(abc(i), size, theme::ink()));
                }
            }
            Kind::Blank { .. } | Kind::Short { .. } => {}
            Kind::Multipart { parts } => {
                sep(ui);
                for (pi, p) in parts.iter().enumerate() {
                    let label = part_label(pi, &p.stem);
                    let r = Rich::new(size, theme::text_col());
                    rich_ui(ui, &p.stem, &if label.is_empty() { r } else { r.lead(label, size, theme::ink()) });
                    if let PartKind::Mc { answers } = &p.kind {
                        ui.horizontal_wrapped(|ui| {
                            ui.add_space(16.0);
                            for (i, a) in answers.iter().enumerate() {
                                ui.label(theme::text(format!("{}{}", abc(i), crate::equation::stem_preview(&a.text))).size(size - 1.0));
                            }
                        });
                    }
                    ui.add_space(3.0);
                }
            }
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if exam_id.is_some() {
                let fixes = crate::qcheck::gate_blockers(q);
                if fixes.is_empty() {
                    let label = if in_exam { "Add again" } else { "Add to exam" };
                    if crate::pageshell::outline(ui, label).clicked() {
                        *add = Some(q.id.clone());
                    }
                } else {
                    // a draft can not go on an exam: say what is missing, and the button opens it
                    if crate::pageshell::outline(ui, "Finish this draft").on_hover_text("It can not be added to an exam until it is finished").clicked() {
                        *edit = Some(q.id.clone());
                    }
                    ui.label(RichText::new(format!("Draft: {}", crate::qcheck::describe(&fixes))).size(11.5).color(if ui.visuals().dark_mode { Color32::from_rgb(235, 175, 90) } else { Color32::from_rgb(140, 78, 0) }));
                }
            }
            if ui.add(Button::new(theme::accent("Copy text").size(12.0)).frame(false)).on_hover_text("Copy the question (with its answer lines) to paste anywhere").clicked() {
                ui.ctx().copy_text(popover_copy_text_cols(q, lines, ps.latex, ps.cols.get(&q.id).copied().unwrap_or(1)));
            }
            if ui.add(Button::new(theme::accent("Edit").size(12.0)).frame(false)).clicked() {
                *edit = Some(q.id.clone());
            }
        });
    });
}

/// The bank Pop-over: questions of one or more banks (tabs) in a separate native window (an
/// embedded egui window where the platform has no multi-viewport support) so a teacher can
/// keep banks in view while building an exam: search and filter, set answer lines, copy a
/// question as text, add it to the open exam, or jump back to edit it. Called every frame from
/// the screen router.
pub fn draw_popover(app: &mut App, ctx: &egui::Context) {
    // Headless/QA hook: VERSHION_POPOVER=<n> opens the n-th bank's Pop-over once.
    if let Ok(v) = crate::env_var("VERSHION_POPOVER") {
        let hid = UiId::new("qs_popover_hook");
        if !ctx.data(|d| d.get_temp::<bool>(hid).unwrap_or(false)) {
            ctx.data_mut(|d| d.insert_temp(hid, true));
            let n: usize = v.trim().parse().unwrap_or(1);
            if let Some(b) = app.store.banks.get(n.saturating_sub(1)) {
                open_popover(ctx, &b.id.clone());
            }
            if crate::env_var("VERSHION_EMBED_VIEWPORTS").as_deref() == Ok("1") {
                ctx.set_embed_viewports(true);
            }
            // VERSHION_POPOVER_FILTERS=1 shows the filters row, =settings the copy settings
            match crate::env_var("VERSHION_POPOVER_FILTERS").as_deref() {
                Ok("1") => ctx.data_mut(|d| d.get_temp_mut_or_default::<PopoverState>(UiId::new("qs_popover_state")).filters = true),
                Ok("settings") => ctx.data_mut(|d| d.get_temp_mut_or_default::<PopoverState>(UiId::new("qs_popover_state")).settings = true),
                _ => {}
            }
        }
    }
    let Some(bank_id) = ctx.data(|d| d.get_temp::<Option<Id>>(popover_key())).flatten() else { return };
    let sk = UiId::new("qs_popover_state");
    let mut ps: PopoverState = ctx.data(|d| d.get_temp(sk)).unwrap_or_default();
    ps.tabs.retain(|t| app.store.bank(t).is_some());
    if app.store.bank(&bank_id).is_none() {
        // the active bank went away: the next tab, or close
        match ps.tabs.first().cloned() {
            Some(t) => open_popover(ctx, &t),
            None => {
                ctx.data_mut(|d| d.insert_temp::<Option<Id>>(popover_key(), None));
            }
        }
        ctx.data_mut(|d| d.insert_temp(sk, ps));
        return;
    }
    if !ps.tabs.contains(&bank_id) {
        ps.tabs.push(bank_id.clone());
    }
    let bank = app.store.bank(&bank_id).cloned().unwrap();
    let names: Vec<(Id, String)> = app.store.banks.iter().map(|b| (b.id.clone(), b.name.clone())).collect();
    let exam_id = match &app.screen {
        Screen::ExamEdit(id) => Some(id.clone()),
        _ => None,
    };
    let exam_items: Vec<Id> = exam_id.as_ref().and_then(|id| app.store.exam(id)).map(|e| e.items.iter().map(|it| it.question_id.clone()).collect()).unwrap_or_default();
    let exam_name = exam_id.as_ref().and_then(|id| app.store.exam(id)).map(|e| e.name.clone());
    let vid = egui::ViewportId::from_hash_of("qs_bank_popover_window");
    let builder = egui::ViewportBuilder::default()
        .with_title(format!("{} · verSHion", bank.name))
        .with_inner_size([460.0, 720.0])
        .with_min_inner_size([340.0, 380.0]);
    let mut close = false;
    let mut add: Option<Id> = None;
    let mut edit: Option<Id> = None;
    let mut switch: Option<Id> = None;
    let mut close_tab: Option<Id> = None;
    ctx.show_viewport_immediate(vid, builder, |ui, class| {
        let pctx = ui.ctx().clone();
        if pctx.input(|i| i.viewport().close_requested()) {
            close = true;
        }
        let embedded = class == egui::ViewportClass::EmbeddedWindow;
        let body = |ui: &mut Ui, ps: &mut PopoverState, close: &mut bool, add: &mut Option<Id>, edit: &mut Option<Id>, switch: &mut Option<Id>, close_tab: &mut Option<Id>| {
            // tabs: open banks, + to open another, copy settings
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for t in ps.tabs.clone() {
                    let name = names.iter().find(|(id, _)| *id == t).map(|(_, n)| n.clone()).unwrap_or_default();
                    let on = t == bank_id;
                    Frame::new()
                        .fill(if on { theme::card() } else { Color32::TRANSPARENT })
                        .stroke(Stroke::new(1.0, if on { theme::border() } else { Color32::TRANSPARENT }))
                        .corner_radius(CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 })
                        .inner_margin(Margin::symmetric(8, 4))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                let label = RichText::new(truncate(&name, 22)).size(12.5).color(if on { theme::ink() } else { theme::muted_col() });
                                if ui.add(Button::new(if on { label.strong() } else { label }).frame(false)).on_hover_text(&name).clicked() && !on {
                                    *switch = Some(t.clone());
                                }
                                if ui.add(Button::new(RichText::new(egui_phosphor::regular::X).size(12.0).color(theme::muted_col())).frame(false)).on_hover_text(format!("Close {name}")).clicked() {
                                    *close_tab = Some(t.clone());
                                }
                            });
                        });
                }
                let others: Vec<&(Id, String)> = names.iter().filter(|(id, _)| !ps.tabs.contains(id)).collect();
                if !others.is_empty() {
                    ui.menu_button(RichText::new("+").size(15.0), |ui| {
                        ui.label(theme::muted("Open a question bank").size(11.5));
                        for (id, n) in others {
                            if ui.button(n).clicked() {
                                *switch = Some(id.clone());
                                ui.close();
                            }
                        }
                    })
                    .response
                    .on_hover_text("Open another question bank here");
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if embedded && crate::pageshell::outline(ui, "Close").clicked() {
                        *close = true;
                    }
                    if ui.add(Button::new(RichText::new("⚙").size(15.0)).frame(false).selected(ps.settings)).on_hover_text("Copy settings").clicked() {
                        ps.settings = !ps.settings;
                    }
                });
            });
            if ps.settings {
                kit::card().inner_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(theme::strong("Copy settings").size(13.5));
                    ui.label(kit::small_caps_text("MATHEMATICS"));
                    ui.radio_value(&mut ps.latex, false, RichText::new("Plain text").strong()).on_hover_text("x² , ½ , √ … pastes anywhere (Google Docs, email)");
                    ui.label(theme::muted("Equations as Unicode text. For Google Docs, and anywhere else.").size(11.0));
                    ui.radio_value(&mut ps.latex, true, RichText::new("LaTeX").strong()).on_hover_text("$x^2$ … type it into Word's equation editor (Alt+=) or a LaTeX document");
                    ui.label(theme::muted("Equations as $…$ source, for Word's equation editor (Alt+=) and LaTeX.").size(11.0));
                });
                ui.add_space(4.0);
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let w = (ui.available_width() - 92.0).max(160.0);
                search_pill(ui, &mut ps.search, w, "Search question stems");
                let n_on = ps.kinds.len() + ps.diff.is_some() as usize + ps.topic.is_some() as usize;
                let label = if n_on > 0 { format!("Filters · {n_on}") } else { "Filters".to_string() };
                if ui.add(Button::new(RichText::new(label).size(12.5)).selected(ps.filters).corner_radius(CornerRadius::same(8))).clicked() {
                    ps.filters = !ps.filters;
                }
            });
            if ps.filters {
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    let kinds_label = if ps.kinds.is_empty() { "Question type".to_string() } else { format!("Type · {}", ps.kinds.len()) };
                    ui.menu_button(RichText::new(kinds_label).size(12.0), |ui| {
                        for k in [QKind::Mc, QKind::Tf, QKind::Matching, QKind::Blank, QKind::Short, QKind::Multipart] {
                            let mut on = ps.kinds.contains(&k);
                            if ui.checkbox(&mut on, k.label()).changed() {
                                if on {
                                    ps.kinds.push(k);
                                } else {
                                    ps.kinds.retain(|x| *x != k);
                                }
                            }
                        }
                    });
                    let dl = ps.diff.map(|d| format!("Difficulty · {}", diff_label(d))).unwrap_or_else(|| "Difficulty".to_string());
                    ui.menu_button(RichText::new(dl).size(12.0), |ui| {
                        ui.selectable_value(&mut ps.diff, None, "All");
                        for d in [Difficulty::Low, Difficulty::Medium, Difficulty::High] {
                            ui.selectable_value(&mut ps.diff, Some(d), diff_label(d));
                        }
                    });
                    let topics = unique_topics(&bank.questions);
                    if !topics.is_empty() || ps.topic.is_some() {
                        let tl = ps.topic.as_deref().map(|t| format!("Topic · {}", truncate(t, 16))).unwrap_or_else(|| "Topic".to_string());
                        ui.menu_button(RichText::new(tl).size(12.0), |ui| {
                            ui.selectable_value(&mut ps.topic, None, "All topics");
                            for t in topics {
                                ui.selectable_value(&mut ps.topic, Some(t.clone()), t);
                            }
                        });
                    }
                    ui.menu_button(RichText::new(format!("Sort: {}", ps.sort.label())).size(12.0), |ui| {
                        for so in Sort::all() {
                            ui.selectable_value(&mut ps.sort, so, so.label());
                        }
                    });
                    if (!ps.kinds.is_empty() || ps.diff.is_some() || ps.topic.is_some()) && ui.add(Button::new(theme::accent("Clear").size(12.0)).frame(false)).clicked() {
                        ps.kinds.clear();
                        ps.diff = None;
                        ps.topic = None;
                    }
                });
            }
            ui.add_space(4.0);
            ui.label(
                theme::muted(match &exam_name {
                    Some(n) => format!("Add sends a question to “{n}”."),
                    None => "Open an exam in the main window to add questions to it.".to_string(),
                })
                .size(11.5),
            );
            ui.add_space(6.0);
            let order = popover_order(&bank.questions, &ps.search, &ps.kinds, ps.diff, ps.topic.as_deref(), ps.sort);
            egui::ScrollArea::vertical().id_salt(("popover_scroll", &bank_id)).auto_shrink([false, false]).show(ui, |ui| {
                for (k, &i) in order.iter().enumerate() {
                    let q = &bank.questions[i];
                    popover_card(ui, q, k + 1, ps, exam_id.as_ref(), exam_items.contains(&q.id), add, edit);
                    ui.add_space(6.0);
                }
                if order.is_empty() {
                    ui.label(theme::muted(if bank.questions.is_empty() { "This bank has no questions yet." } else { "No questions match." }).size(12.5));
                }
            });
        };
        if embedded {
            // Embedded fallback: the viewport is already an egui window; just pad it.
            ui.set_min_size(Vec2::new(420.0, 540.0));
            body(ui, &mut ps, &mut close, &mut add, &mut edit, &mut switch, &mut close_tab);
        } else {
            egui::CentralPanel::default()
                .frame(Frame::new().fill(theme::cream()).inner_margin(Margin::same(12)))
                .show(ui, |ui| body(ui, &mut ps, &mut close, &mut add, &mut edit, &mut switch, &mut close_tab));
        }
    });
    if let Some(t) = switch {
        if !ps.tabs.contains(&t) {
            ps.tabs.push(t.clone());
        }
        open_popover(ctx, &t);
    }
    if let Some(t) = close_tab {
        let at = ps.tabs.iter().position(|x| *x == t).unwrap_or(0);
        ps.tabs.retain(|x| *x != t);
        if t == bank_id {
            match ps.tabs.get(at.min(ps.tabs.len().saturating_sub(1))).cloned() {
                Some(n) => open_popover(ctx, &n),
                None => close = true,
            }
        }
    }
    // (the card offers no Add for a draft; this keeps any other route from adding one)
    let add = add.filter(|qid| bank.questions.iter().find(|q| q.id == *qid).is_some_and(|q| crate::qcheck::gate_blockers(q).is_empty()));
    if let (Some(qid), Some(eid)) = (add, exam_id.as_ref()) {
        if let Some(before) = app.store.exam(eid).map(|e| e.items.clone()) {
            app.undo.push(before);
            app.redo.clear();
            let lines = ps.lines.get(&qid).copied().unwrap_or(0);
            if let Some(ex) = app.store.exam_mut(eid) {
                let cols = ps.cols.get(&qid).copied().unwrap_or(1);
                let mut item = crate::model::ExamItem::new(bank_id.clone(), qid);
                item.work_lines = lines.min(23);
                item.answer_columns = cols;
                ex.items.push(item);
                ex.modified = now_secs();
            }
            app.store.save_exams();
            app.toast("Added to the exam");
        }
    }
    if let Some(qid) = edit {
        if let Some(q) = app.store.question(&bank_id, &qid).cloned() {
            app.open_bank(&bank_id);
            app.qedit = Some(qedit::Draft::from_question(bank_id.clone(), &q));
            if app.allow_viewports {
                ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Focus);
            }
        }
    }
    if close {
        ps.tabs.clear();
        ctx.data_mut(|d| d.insert_temp::<Option<Id>>(popover_key(), None));
    }
    ctx.data_mut(|d| d.insert_temp(sk, ps));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_bank() -> crate::model::QuestionBank {
        crate::model::QuestionBank { id: "b".into(), name: "B".into(), questions: Vec::new(), created: 1, modified: 1, details: Default::default() }
    }

    fn q(stem: &str, kind: crate::model::Kind, created: i64) -> Question {
        let mut q = crate::formats::make_question(stem.to_string(), kind);
        q.created = created;
        q
    }

    fn mc(texts: &[&str]) -> crate::model::Kind {
        crate::model::Kind::Mc { answers: texts.iter().enumerate().map(|(i, t)| crate::model::Answer { text: t.to_string(), correct: i == 0, locked: false }).collect() }
    }

    /// The Banks grid fills its width: 1280 (about 1110 of page) is three wide tiles, 760 (about
    /// 614) two, a phone one; a wide window gains columns instead of a dead strip.
    #[test]
    fn the_banks_grid_takes_as_many_tiles_as_fit() {
        let g = kit::GUTTER;
        assert_eq!(banks_grid_cols(1110.0, g), 3);
        assert_eq!(banks_grid_cols(614.0, g), 2);
        assert_eq!(banks_grid_cols(400.0, g), 1);
        assert_eq!(banks_grid_cols(100.0, g), 1);
        assert_eq!(banks_grid_cols(1600.0, g), 5);
        // Every tile stays at least the minimum wide.
        for avail in [300.0f32, 575.0, 576.0, 900.0, 1110.0, 1600.0] {
            let n = banks_grid_cols(avail, g) as f32;
            assert!((avail - g * (n - 1.0)) / n >= BANK_TILE_MIN_W, "{avail}");
        }
    }

    #[test]
    fn popover_filters_and_sorts() {
        let mut a = q("Which gas is $CO_2$?", mc(&["carbon dioxide", "oxygen"]), 1);
        a.difficulty = Some(Difficulty::Low);
        a.topics = vec!["Gases".into()];
        let b = q("Explain **why**.", crate::model::Kind::Short { sample: String::new() }, 2);
        let mut c = q("The sky is blue.", crate::model::Kind::Tf { correct: true }, 3);
        c.difficulty = Some(Difficulty::High);
        let qs = vec![a, b, c];
        assert_eq!(popover_order(&qs, "", &[], None, None, Sort::Newest), vec![2, 1, 0]);
        assert_eq!(popover_order(&qs, "", &[], None, None, Sort::Oldest), vec![0, 1, 2]);
        assert_eq!(popover_order(&qs, "", &[QKind::Mc, QKind::Tf], None, None, Sort::Oldest), vec![0, 2]);
        assert_eq!(popover_order(&qs, "", &[], Some(Difficulty::High), None, Sort::Newest), vec![2]);
        assert_eq!(popover_order(&qs, "", &[], None, Some("Gases"), Sort::Newest), vec![0]);
        assert_eq!(popover_order(&qs, "why", &[], None, None, Sort::Newest), vec![1]);
        assert_eq!(popover_order(&qs, "co₂", &[], None, None, Sort::Newest), vec![0], "search reads the shown text");
    }

    #[test]
    fn popover_copy_text_carries_choices_lines_and_math_setting() {
        let a = q("Which gas is $CO_2$?\n• gas", mc(&["carbon dioxide", "*oxygen*"]), 1);
        assert_eq!(popover_copy_text(&a, 0, false), "Which gas is CO₂?\n• gas\nA. carbon dioxide\nB. oxygen");
        assert_eq!(popover_copy_text(&a, 0, true), "Which gas is $CO_2$?\n• gas\nA. carbon dioxide\nB. oxygen");
        let b = q("Explain.", crate::model::Kind::Short { sample: String::new() }, 2);
        let t = popover_copy_text(&b, 2, false);
        assert_eq!(t.lines().count(), 3);
        assert!(t.lines().skip(1).all(|l| l.chars().all(|c| c == '_') && l.len() > 20));
        let parts = crate::model::Kind::Multipart {
            parts: vec![
                crate::model::Part { stem: "a. NH_{3}".into(), kind: crate::model::PartKind::Short { sample: String::new() } },
                crate::model::Part { stem: "Second".into(), kind: crate::model::PartKind::Tf { correct: true } },
            ],
        };
        let m = q("Do all:", parts, 3);
        let t = popover_copy_text(&m, 1, false);
        assert_eq!(t.lines().next(), Some("Do all:"));
        assert!(t.contains("\na. NH₃\n____") && t.contains("\nb. Second\n   True / False"), "{t}");
        assert!(takes_lines(&m) && takes_lines(&b) && !takes_lines(&a));
        // headings, dividers and side-by-side panels flatten to lines
        let h = q("# Part A\n---\n::: side\nleft\n::: next\nright\n:::", crate::model::Kind::Short { sample: String::new() }, 4);
        assert_eq!(popover_copy_text(&h, 0, false), "Part A\n———\nleft\nright");
    }

    #[test]
    fn details_save_trimmed_and_only_on_change() {
        let mut b = empty_bank();
        let mut d = crate::model::BankDetails::default();
        assert!(!apply_details(&mut b, &d, 5), "nothing entered, nothing changes");
        assert_eq!(b.modified, 1);
        d.author = "  Ms. Rivera ".into();
        d.license_name = "CC BY 4.0".into();
        assert!(apply_details(&mut b, &d, 7));
        assert_eq!(b.details.author, "Ms. Rivera");
        assert_eq!(b.modified, 7);
        assert!(!apply_details(&mut b, &d, 9), "same details again is a no-op");
    }

    #[test]
    fn bank_menu_matches_the_reference_order() {
        let labels: Vec<&str> = bank_menu_items(3, false).iter().map(|i| i.map_or("---", |(_, l, _)| l)).collect();
        assert_eq!(labels, vec!["Open", "Preview", "Export…", "Rename", "Archive", "---", "Delete…"]);
        let back: Vec<&str> = bank_menu_items(3, true).iter().flatten().map(|(_, l, _)| *l).collect();
        assert!(back.contains(&"Restore") && !back.contains(&"Archive"));
        assert!(bank_menu_items(3, false).iter().flatten().all(|(_, _, en)| *en));
        // an empty bank cannot be exported (as on the reference)
        let export = |n| bank_menu_items(n, false).into_iter().flatten().find(|(a, _, _)| *a == BankAct::Export).unwrap().2;
        assert!(!export(0) && export(1));
    }

    #[test]
    fn export_carries_details_only_when_entered() {
        let mut b = empty_bank();
        assert!(bank_export_json(&b).get("details").is_none());
        b.details.description = "Unit 4 bonding".into();
        b.details.license_url = "https://creativecommons.org/licenses/by/4.0/".into();
        let v = bank_export_json(&b);
        assert_eq!(v["details"]["description"], "Unit 4 bonding");
        assert_eq!(v["details"]["license_url"], "https://creativecommons.org/licenses/by/4.0/");
        assert!(v["details"].get("author").is_none(), "empty fields stay out");
    }

    #[test]
    fn duplicate_inserts_a_fresh_copy_after_the_original() {
        use crate::formats::make_question;
        use crate::model::{Kind, Part, PartKind, QuestionBank};
        let mut a = make_question("First".into(), Kind::Tf { correct: true });
        a.topics = vec!["Atoms".into()];
        a.points = Some(2.0);
        a.created = 5;
        let b = make_question(
            "Second".into(),
            Kind::Multipart { parts: vec![Part { stem: "a.".into(), kind: PartKind::Short { sample: "x".into() } }] },
        );
        let mut bank: QuestionBank = serde_json::from_value(serde_json::json!({
            "id": "b1", "name": "B", "questions": [], "created": 0, "modified": 0
        }))
        .unwrap();
        bank.questions = vec![a.clone(), b.clone()];
        let id = duplicate_in_bank(&mut bank, &a.id).expect("copied");
        assert_eq!(bank.questions.len(), 3);
        let c = &bank.questions[1];
        assert_eq!(c.id, id);
        assert_ne!(c.id, a.id);
        assert_eq!(c.stem, "First");
        assert_eq!(c.topics, a.topics);
        assert_eq!(c.points, Some(2.0));
        assert!(matches!(c.kind, Kind::Tf { correct: true }));
        assert!(c.created > 5, "fresh timestamps");
        assert_eq!(bank.questions[2].id, b.id, "inserted right after the original");
        assert!(bank.modified > 0);
        assert!(duplicate_in_bank(&mut bank, "missing").is_none());
    }

    #[test]
    fn fmt_date_formats_epoch_days() {
        assert_eq!(fmt_date(0), "—");
        assert_eq!(fmt_date(86_400), "Jan 2, 1970");
        // 1970-01-01 + 56*365 + 14 leap days + day-of-year offset of Oct 15 (287).
        let secs: i64 = ((2026 - 1970) as i64 * 365 + 14 + 287) * 86_400;
        assert_eq!(fmt_date(secs), "Oct 15, 2026");
    }

    #[test]
    fn rel_time_buckets() {
        let now = now_secs();
        assert_eq!(rel_time(now - 5), "just now");
        assert_eq!(rel_time(now - 2 * 3600 - 10), "2 hours ago");
        assert_eq!(rel_time(now - 86_400 - 10), "1 day ago");
    }

    #[test]
    fn chip_colors_are_distinct() {
        let kinds = [
            QKind::Mc,
            QKind::Tf,
            QKind::Matching,
            QKind::Blank,
            QKind::Short,
            QKind::Multipart,
        ];
        let colors: Vec<Color32> = kinds.iter().map(|k| kind_chip_color(*k)).collect();
        assert_eq!(colors.len(), 6);
        for i in 0..colors.len() {
            for j in i + 1..colors.len() {
                assert_ne!(colors[i], colors[j], "kinds share chip colors");
            }
        }
    }

    #[test]
    fn popover_copies_choices_in_the_chosen_columns() {
        let q = q("Which is least electronegative?", mc(&["Ca", "O", "Ge", "C"]), 1);
        assert_eq!(popover_copy_text_cols(&q, 0, false, 1), popover_copy_text(&q, 0, false));
        assert_eq!(popover_copy_text_cols(&q, 0, false, 2), "Which is least electronegative?\nA. Ca    C. Ge\nB. O     D. C");
        assert_eq!(popover_copy_text_cols(&q, 0, false, 4), "Which is least electronegative?\nA. Ca    B. O    C. Ge    D. C");
        // three choices in two columns: A C / B
        assert_eq!(choice_rows(&["A. x".into(), "B. yy".into(), "C. z".into()], 2), vec!["A. x     C. z".to_string(), "B. yy".to_string()]);
    }

    #[test]
    fn blank_answer_text_reads_like_the_reference_key() {
        let acc = |v: &[&[&str]]| v.iter().map(|a| a.iter().map(|s| s.to_string()).collect()).collect::<Vec<Vec<String>>>();
        assert_eq!(blank_answer_text(&acc(&[&["14"], &["10"]])), "14; 10");
        assert_eq!(blank_answer_text(&acc(&[&["linear", " Linear "], &[]])), "linear / Linear; —");
        assert_eq!(blank_answer_text(&[]), "");
    }

    #[test]
    fn points_label_works() {
        assert_eq!(points_label(None), None);
        assert_eq!(points_label(Some(2.0)), Some("2 pt".to_string()));
        assert_eq!(points_label(Some(2.5)), Some("2.5 pt".to_string()));
    }

    /// Banks grid columns sit on whole device pixels at 1.7, keep their gap within a pixel, and
    /// the last one ends on the page's right edge.
    #[test]
    fn banks_grid_columns_land_on_whole_pixels() {
        for (avail, n) in [(1066.3_f32, 3usize), (603.9, 2), (1000.0, 3), (777.7, 1)] {
            let x0 = 187.4;
            let c = grid_columns(x0, avail, n, 16.0, 1.7);
            assert_eq!(c.len(), n);
            for &(l, r) in &c {
                for v in [l, r] {
                    assert!(((v * 1.7) - (v * 1.7).round()).abs() < 1e-3, "{v} is not on a pixel");
                }
                assert!(r > l);
            }
            assert!((c[n - 1].1 - (x0 + avail)).abs() <= 0.5 / 1.7 + 1e-4, "ends on the right edge");
            for w in c.windows(2) {
                assert!((w[1].0 - w[0].1 - 16.0).abs() <= 1.0 / 1.7 + 1e-4, "gap kept: {w:?}");
            }
        }
    }

    /// Every bank tile is the sum of its slots, 201, on Banks and on Home (spec section 3; 211
    /// until round 20 dropped the type-mix bar and its 8 + 6, the key's gap to the hairline 8 -> 12).
    #[test]
    fn a_bank_tile_is_201_tall() {
        // pad, name, meta, key, hairline, the footer's 32 tall Open, foot pad
        assert_eq!(BANK_TILE_H, 16.0 + 44.0 + 6.0 + 18.0 + 8.0 + 40.0 + 12.0 + 1.0 + 8.0 + 32.0 + 16.0);
        assert_eq!(BANK_TILE_H, 201.0);
        // An empty bank's dashed box: its button ADD_PAD clear above and below, the box ending
        // ADD_PAD over the hairline (it takes 4 of the key's 12 to the hairline).
        assert_eq!(ADD_H + 2.0 * ADD_PAD + ADD_PAD, T_KEY + T_BR);
        // Open keeps the tile's side pad below it as on its right.
        assert_eq!(T_PAD_FOOT, T_PAD);
    }

    /// The bank header's toolbar, drawn as the page draws it (`ltr` false: a right-to-left row
    /// beside the name; true: the narrow window's wrapping row), for a bank whose questions are
    /// the harness state's. The store is a scratch one: nothing here saves.
    fn toolbar_harness<'a>(ltr: bool) -> egui_kittest::Harness<'a, (App, Vec<Question>)> {
        let tmp = std::env::temp_dir().join(format!("vs-toolbar-test-{}", std::process::id()));
        crate::store::init(Box::new(crate::store::FsBackend::new(tmp)));
        let mut app = App::new_from(&egui::Context::default());
        app.store = Default::default();
        app.store.banks.push(empty_bank());
        let mut first = true;
        egui_kittest::Harness::builder().with_size(egui::vec2(1200.0, 400.0)).build_ui_state(
            move |ui, st: &mut (App, Vec<Question>)| {
                if std::mem::take(&mut first) {
                    theme::setup(ui.ctx());
                    ui.ctx().request_repaint();
                    return;
                }
                let ctx = ui.ctx().clone();
                let (app, qs) = st;
                let id: Id = "b".into();
                let key = UiId::new("toolbar_test");
                if ltr {
                    ui.horizontal_wrapped(|ui| {
                        app.bank_toolbar(ui, &ctx, key, &id, qs, true);
                    });
                } else {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        app.bank_toolbar(ui, &ctx, key, &id, qs, false);
                    });
                }
            },
            (app, Vec::new()),
        )
    }

    /// The toolbar's buttons, by accessible name, left to right as drawn.
    const FULL_TOOLBAR: [&str; 7] = ["Add question", "Import", "Share", "Export", "Details", "Preview", "More bank actions"];

    fn assert_full_toolbar(h: &egui_kittest::Harness<'_, (App, Vec<Question>)>, how: &str) {
        use egui_kittest::kittest::Queryable;
        let xs: Vec<f32> = FULL_TOOLBAR
            .iter()
            .map(|n| {
                let node = if *n == "Share" { h.get_by_label_contains("Share") } else { h.get_by_label(n) };
                node.rect().left()
            })
            .collect();
        assert!(xs.windows(2).all(|w| w[0] < w[1]), "{how}: toolbar in the old order {FULL_TOOLBAR:?}, got x {xs:?}");
    }

    /// An empty bank's header has only its labelled "Bank actions" menu (no Share, Preview or
    /// Details button), which reaches Details… and From pool…; with a question the whole toolbar is there in the old
    /// order and From pool… is in Share's menu; the first question brings the full toolbar back.
    #[test]
    fn an_empty_bank_header_is_the_more_menu_and_the_first_question_brings_the_toolbar_back() {
        use egui_kittest::kittest::Queryable;
        for ltr in [false, true] {
            let how = if ltr { "narrow" } else { "desktop" };
            let mut h = toolbar_harness(ltr);
            h.run();
            // Empty: the labelled "Bank actions" menu alone.
            assert!(h.query_by_label("Bank actions").is_some(), "{how}: empty bank keeps its menu, named Bank actions");
            for gone in ["Share", "Preview", "Details", "Add question", "Import", "Export", "More bank actions"] {
                assert!(h.query_by_label_contains(gone).is_none(), "{how}: empty bank header shows no {gone}");
            }
            h.get_by_label("Bank actions").click();
            h.run();
            assert!(h.query_by_label("Details…").is_some(), "{how}: … reaches Details… on an empty bank");
            assert!(h.query_by_label("From pool…").is_some(), "{how}: … reaches From pool… on an empty bank");
            assert!(h.query_by_label("Tidy stems…").is_none(), "{how}: no greyed Tidy stems on an empty bank");
            // Details… opens the details dialog's draft.
            h.get_by_label("Details…").click();
            h.run();
            let key = UiId::new("toolbar_test");
            assert!(with_state(&h.ctx, key, |s: &mut BankEditState| s.details.is_some()), "{how}: Details… opens the bank details");
            h.key_press(egui::Key::Escape);
            h.run();

            // The first question: the full toolbar is back, in the old order.
            h.state_mut().1.push(q("What is 2 + 2?", mc(&["4", "5"]), 1));
            h.run();
            h.run();
            assert_full_toolbar(&h, how);
            assert!(h.query_by_label("Details…").is_none(), "{how}: Details is its own button again, not in …");
            // From pool… is in Share's menu.
            h.get_by_label_contains("Share").click();
            h.run();
            assert!(h.query_by_label("From pool…").is_some(), "{how}: From pool… is in Share's menu");
            h.key_press(egui::Key::Escape);
            h.run();
            // "…" holds Tidy stems, not Details or From pool.
            h.get_by_label("More bank actions").click();
            h.run();
            assert!(h.query_by_label("Tidy stems…").is_some(), "{how}: … holds Tidy stems on a bank with questions");
            assert!(h.query_by_label("From pool…").is_none() && h.query_by_label("Details…").is_none(), "{how}: … holds neither Details… nor From pool… once Share and Details show");
        }
    }

    /// The key keeps two lines: what a third would hold folds into "+N more" on line 2, dropping
    /// line 2's last items until it fits 12 after them.
    #[test]
    fn the_tile_key_folds_a_third_line_into_more() {
        let widths = [110.0, 80.0, 80.0, 80.0, 115.0, 100.0];
        let item_w = |i: usize| widths[i];
        let more_w = |_n: usize| 44.0;
        // Everything in two lines: no "+N more".
        let k = layout_tile_key(&[1, 1, 0, 0, 0, 0], 284.0, item_w, more_w);
        assert_eq!(k.items, vec![(0, 0.0, 0), (1, 122.0, 0)]);
        assert_eq!(k.more, None);
        // A third line's items become "+2 more" after line 2's last item.
        let k = layout_tile_key(&[5, 2, 1, 1, 1, 1], 284.0, item_w, more_w);
        assert_eq!(k.items.iter().map(|i| (i.0, i.2)).collect::<Vec<_>>(), vec![(0, 0), (1, 0), (2, 1), (3, 1)]);
        assert_eq!(k.more, Some((184.0, vec![4, 5])));
        // No room after line 2's items: its last item gives way, and is counted in "+N more".
        let k = layout_tile_key(&[5, 2, 1, 1, 1, 1], 250.0, item_w, more_w);
        let (x, rest) = k.more.clone().unwrap();
        assert!(x + 44.0 <= 250.0, "+N more ends inside the key");
        assert_eq!(rest.len() + k.items.len(), 6, "every type is shown or counted");
        assert!(k.items.iter().all(|it| it.2 <= 1), "two lines at most");
        // Types stay in mix order: shown ones first, the folded ones after.
        let mut order: Vec<usize> = k.items.iter().map(|i| i.0).collect();
        order.extend(rest);
        assert_eq!(order, vec![0, 1, 2, 3, 4, 5]);
    }

    /// Before anything folds the key closes up: items 8 apart instead of 12, then each swatch 1
    /// closer to its words; it folds only when even the closest key needs a third line, and a key
    /// that fits at 12 keeps 12.
    #[test]
    fn the_tile_key_closes_up_before_it_folds() {
        // The measured key items (swatch 9, gap 5, "1 <type>" at 13 pt).
        let widths = [106.9, 75.5, 74.5, 74.5, 103.5, 93.7];
        let item_w = |i: usize| widths[i];
        let more_w = |_n: usize| 50.0;
        let science = [4, 1, 0, 1, 1, 1];
        // Banks at 760 (inside 271): 12 apart folds, 8 apart and a snug swatch names all five.
        let k = layout_tile_key(&science, 271.0, item_w, more_w);
        assert_eq!(k.more, None, "every type named");
        assert_eq!(k.snug, 1.0);
        assert_eq!(k.items.iter().map(|i| (i.0, i.2)).collect::<Vec<_>>(), vec![(0, 0), (1, 0), (3, 0), (4, 1), (5, 1)]);
        let end = k.items.iter().map(|&(i, x, _)| x + widths[i] - k.snug).fold(0.0, f32::max);
        assert!(end <= 271.0, "the key ends inside the tile: {end}");
        // Wide (inside 327): the usual 12 apart, nothing snug.
        let k = layout_tile_key(&[5, 2, 1, 1, 1, 1], 327.0, item_w, more_w);
        assert_eq!((k.more, k.snug), (None, 0.0));
        assert_eq!(k.items[1].1, widths[0] + 12.0);
        // Six types in 266 cannot be named in two lines: the fold is the last resort.
        let k = layout_tile_key(&[5, 2, 1, 1, 1, 1], 266.0, item_w, more_w);
        assert!(k.more.is_some());
    }

    /// A six-type tile at the narrow width folds into "+N more", and that is a real control: a
    /// named button that, clicked or pressed with Enter, lists the folded types with their counts
    /// (and never opens the bank); Escape closes the list.
    #[test]
    fn plus_more_reveals_the_folded_types_and_never_opens_the_bank() {
        use egui_kittest::kittest::{NodeT, Queryable};
        let tmp = std::env::temp_dir().join(format!("vs-more-test-{}", std::process::id()));
        crate::store::init(Box::new(crate::store::FsBackend::new(tmp)));
        let mut app = App::new_from(&egui::Context::default());
        app.store = Default::default();
        let tile = BankTile { id: "b".into(), name: "Six types".into(), questions: 11, topics: 4, mix: [5, 2, 1, 1, 1, 1], used_in: vec![], edited: 0, archived: false };
        let mut first = true;
        let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(400.0, 300.0)).build_ui_state(
            move |ui, st: &mut (App, Vec<TileAct>)| {
                if std::mem::take(&mut first) {
                    theme::setup(ui.ctx());
                    ui.ctx().request_repaint();
                    return;
                }
                // The narrow Banks tile (inside 271): six types cannot be named in two lines.
                if let Some(a) = bank_tile(ui, &mut st.0, 303.0, &tile, TileVariant::Banks) {
                    st.1.push(a);
                }
            },
            (app, Vec::new()),
        );
        h.run();
        let folded_names = |h: &egui_kittest::Harness<'_, (App, Vec<TileAct>)>| h.query_all_by_label_contains("short answer").count();
        assert_eq!(folded_names(&h), 0, "the folded names are not shown at rest");
        h.get_by_label_contains("Show the other").click();
        h.run();
        assert!(folded_names(&h) > 0, "pressing +N more lists the folded types");
        assert!(h.query_by_label_contains("Also in this bank").is_some());
        assert!(h.state().1.is_empty(), "pressing +N more does not open the bank: {:?}", h.state().1);
        h.key_press(egui::Key::Escape);
        h.run();
        assert_eq!(folded_names(&h), 0, "Escape closes the list");
        // From the keyboard: focus it, Enter opens the list.
        h.get_by_label_contains("Show the other").focus();
        h.run();
        h.key_press(egui::Key::Enter);
        h.run();
        assert!(folded_names(&h) > 0, "Enter on +N more lists the folded types");
        assert!(h.state().1.is_empty(), "Enter on +N more does not open the bank");
    }

    /// The footer's edited text has a long, a month-day and a bare form.
    #[test]
    fn edited_forms_shorten_to_the_month_day() {
        let f = edited_forms(1_760_659_200); // Oct 17, 2025 (UTC)
        assert!(f[0].starts_with("Edited "));
        assert_eq!(f[1], "Edited Oct 17");
        assert_eq!(f[2], "Oct 17");
    }

    /// The meta row keeps the long edited form before the topics, then shortens the date.
    #[test]
    fn the_meta_row_drops_topics_before_the_long_edited_form() {
        let ed = [101.0, 75.0, 38.0];
        // Banks (283 inside): everything.
        assert_eq!(meta_fit(119.0, 70.0, ed, 283.0), (true, Some(0)));
        // Home (200 inside): the topics go first, the long form stays.
        assert_eq!(meta_fit(119.0, 70.0, ed, 200.0), (false, Some(0)));
        // Tighter: the month-day form, then the bare date.
        assert_eq!(meta_fit(119.0, 70.0, ed, 160.0), (false, Some(1)));
        assert_eq!(meta_fit(119.0, 70.0, ed, 125.0), (false, Some(2)));
        // No room for any form: the counts alone.
        assert_eq!(meta_fit(119.0, 70.0, ed, 119.0), (true, None));
        assert_eq!(meta_fit(119.0, 70.0, ed, 100.0), (false, None));
    }

    /// While the bank page scrolls its content ends clear of the scroll bar (desktop), the page
    /// that does not scroll keeps the full width, and a narrow window is unchanged.
    #[test]
    fn the_bank_page_keeps_clear_of_its_scroll_bar() {
        let bar = 22.0;
        for avail in [1240.0_f32, 1500.0, 1700.0] {
            let w = bank_page_w(avail, Some(bar));
            assert!(w <= avail - bar, "{avail}: content right edge {w} reaches the bar at {}", avail - bar);
            assert_eq!(bank_page_w(avail, None), avail.min(BANKS_MAX_W), "{avail}: no bar, full width");
        }
        for avail in [700.0_f32, 900.0] {
            assert_eq!(bank_page_w(avail, Some(bar)), avail, "{avail}: narrow page unchanged");
        }
    }

    /// A badge's room comes from dropping the topics, never from a taller row.
    #[test]
    fn counts_line_drops_topics_for_a_badge() {
        let t = BankTile { id: "b".into(), name: "B".into(), questions: 11, topics: 4, mix: [11, 0, 0, 0, 0, 0], used_in: vec![], edited: 0, archived: false };
        assert_eq!(counts_line(&t, true), "11 questions · 4 topics");
        assert_eq!(counts_line(&t, false), "11 questions");
        let a = BankTile { archived: true, questions: 1, topics: 1, ..t };
        assert_eq!(counts_line(&a, true), "Archived · 1 question · 1 topic");
    }

    /// The bar's segments: shown types in mix order with a 2 px gap between them, the folded types
    /// as one segment at the end, none under 4, and the whole bar inside its width.
    #[test]
    fn bar_segments_have_gaps_and_fold_the_rest_last() {
        let mix = [5, 2, 1, 1, 1, 1];
        let s = bar_segments(&mix, &[], 284.0);
        assert_eq!(s.iter().map(|x| x.0).collect::<Vec<_>>(), (0..6).map(Some).collect::<Vec<_>>());
        for w in s.windows(2) {
            assert!((w[1].1 - w[0].2 - BAR_GAP).abs() < 0.01, "a 2 px gap between neighbours");
        }
        assert!(s.iter().all(|x| x.2 - x.1 >= 4.0 - 0.01));
        assert!(s.last().unwrap().2 <= 284.0 + 0.01);
        // MC takes 5/11 of what the gaps leave.
        assert!(((s[0].2 - s[0].1) - (284.0 - 5.0 * BAR_GAP) * 5.0 / 11.0).abs() < 0.01);
        // Folded types (a suffix in mix order) are one segment, last.
        let s = bar_segments(&mix, &[3, 4, 5], 201.0);
        assert_eq!(s.iter().map(|x| x.0).collect::<Vec<_>>(), vec![Some(0), Some(1), Some(2), None]);
        assert!(s.last().unwrap().2 <= 201.0 + 0.01);
        // Absent types draw nothing; an empty bank has no segments.
        assert_eq!(bar_segments(&[0, 3, 0, 0, 0, 0], &[], 80.0), vec![(Some(1), 0.0, 80.0)]);
        assert!(bar_segments(&[0; 6], &[], 80.0).is_empty());
        // A sliver is at least 4 and the bar still fits.
        let s = bar_segments(&[100, 1, 0, 0, 0, 0], &[], 80.0);
        assert!(s[1].2 - s[1].1 >= 4.0 - 0.01 && s[1].2 <= 80.0 + 0.01);
    }


    /// The empty bank page as the app draws it, `w` wide, for a bank holding the harness's questions.
    fn bank_page_harness<'a>(w: f32) -> egui_kittest::Harness<'a, App> {
        let tmp = std::env::temp_dir().join(format!("vs-bank-page-test-{}", std::process::id()));
        crate::store::init(Box::new(crate::store::FsBackend::new(tmp)));
        let mut app = App::new_from(&egui::Context::default());
        app.store = Default::default();
        app.store.banks.push(empty_bank());
        app.bank = Some("b".into());
        app.screen = Screen::BankEdit("b".into());
        let mut first = true;
        egui_kittest::Harness::builder().with_size(egui::vec2(w, 800.0)).build_ui_state(
            move |ui, app: &mut App| {
                if std::mem::take(&mut first) {
                    theme::setup(ui.ctx());
                    ui.ctx().request_repaint();
                    return;
                }
                app.draw_bank_edit(ui);
            },
            app,
        )
    }

    /// An empty bank shows the "what a bank gives you" strip under its card, inside the card's
    /// width and above the window's bottom at 1280 and 760, its six types and three titles real
    /// labels; the first question takes the strip away.
    #[test]
    fn the_empty_bank_strip_shows_until_the_first_question() {
        use egui_kittest::kittest::Queryable;
        for w in [1110.0, 615.0] {
            let mut h = bank_page_harness(w);
            h.run();
            let card = h.get_by_label("Add question").rect();
            for t in ["Six question types", "Topics and difficulty", "Use it in any exam"] {
                let r = h.get_by_label(t).rect();
                assert!(r.top() > card.bottom(), "{w}: {t} sits under the card");
                assert!(r.right() <= w + 0.5 && r.bottom() <= 800.0, "{w}: {t} inside the window, got {r:?}");
            }
            for k in crate::bankops::KINDS {
                let r = h.get_by_label(k.label()).rect();
                assert!(r.right() <= w + 0.5, "{w}: {} inside the window, got {r:?}", k.label());
            }
            for body in [EMPTY_STRIP[1].2, EMPTY_STRIP[2].2] {
                let r = h.get_by_label(body).rect();
                assert!(r.right() <= w + 0.5 && r.bottom() <= 800.0, "{w}: body wraps inside the window, got {r:?}");
            }
            h.state_mut().store.banks[0].questions.push(q("2 + 2 = ?", mc(&["4", "5"]), 1));
            h.run();
            for t in ["Six question types", "Topics and difficulty", "Use it in any exam"] {
                assert!(h.query_by_label(t).is_none(), "{w}: {t} gone once the bank has a question");
            }
        }
    }

    const EMPTY_SENTENCE: &str = "Write the first question yourself, or import a test you already have.";

    /// On a wide window the empty bank's card is one row: disc, "No questions yet" and its
    /// sentence on the left, Add question then Import questions on the right ending inside the
    /// card's right padding, all centred on one line and none overlapping; below EMPTY_WIDE the
    /// shared stacked, centred card. Both buttons still act: Add question opens the add menu,
    /// Import questions the import window.
    #[test]
    fn the_empty_bank_card_is_one_row_on_a_wide_window() {
        use egui_kittest::kittest::Queryable;
        let mut h = bank_page_harness(1110.0);
        h.run();
        let title = h.get_by_label("No questions yet").rect();
        let body = h.get_by_label(EMPTY_SENTENCE).rect();
        let add = h.get_by_label("Add question").rect();
        let imp = h.get_by_label("Import questions").rect();
        // The page's right edge is where the header's Bank actions ends; the card ends there too,
        // its 28 pt padding inside.
        let page_right = h.get_by_label("Bank actions").rect().right();
        assert!(title.left() < add.left() && body.right() < add.left(), "text left of the buttons: {body:?} {add:?}");
        assert!(add.right() < imp.left(), "Add question before Import questions, apart: {add:?} {imp:?}");
        assert!(imp.right() <= page_right - 28.0 + 0.5 && imp.right() >= page_right - 28.0 - 2.0, "buttons end on the card's right padding: {imp:?} page right {page_right}");
        assert!(title.bottom() <= body.top() + 0.5, "title above its sentence: {title:?} {body:?}");
        // One row: the text block and the buttons share a vertical centre.
        let text_mid = (title.top() + body.bottom()) / 2.0;
        assert!((add.center().y - text_mid).abs() <= 2.0 && (imp.center().y - add.center().y).abs() <= 0.5, "centred on one row: text {text_mid}, {add:?} {imp:?}");
        // The strip (and so the card's bottom) is close under the row, not under a stacked card.
        let strip = h.get_by_label("Six question types").rect();
        assert!(strip.top() > add.bottom() && strip.top() - add.bottom() < 120.0, "strip under a one-row card: {strip:?} {add:?}");

        // Add question opens the menu of the six types.
        assert!(h.query_by_label("1   Multiple choice").is_none());
        h.get_by_label("Add question").click();
        h.run();
        assert!(h.query_by_label("1   Multiple choice").is_some(), "Add question opens its type menu");
        h.key_press(egui::Key::Escape);
        h.run();
        // Import questions opens the import window.
        assert!(!crate::importui::import_modal_open(&h.ctx));
        h.get_by_label("Import questions").click();
        h.run();
        assert!(crate::importui::import_modal_open(&h.ctx), "Import questions opens the import window");

        // Narrow: the stacked card, title, sentence and buttons centred one under another.
        let mut h = bank_page_harness(615.0);
        h.run();
        let title = h.get_by_label("No questions yet").rect();
        let body = h.get_by_label(EMPTY_SENTENCE).rect();
        let add = h.get_by_label("Add question").rect();
        let imp = h.get_by_label("Import questions").rect();
        assert!(615.0 < EMPTY_WIDE);
        assert!(title.bottom() <= body.top() + 0.5 && body.bottom() < add.top(), "stacked: {title:?} {body:?} {add:?}");
        assert!((title.center().x - body.center().x).abs() <= 1.0, "title and sentence centred together");
        let pair_mid = (add.left() + imp.right()) / 2.0;
        assert!((pair_mid - body.center().x).abs() <= 2.0, "the buttons centred under the sentence: {pair_mid} vs {}", body.center().x);
    }

    /// "Six question types": the six names in two columns of three, in the bar's order read
    /// across, the names of a column starting on one x and a row's names on one line, each name
    /// its own label, all inside the first part of the strip (under its title, before the next
    /// part), at both widths.
    #[test]
    fn the_six_types_form_an_aligned_grid() {
        use egui_kittest::kittest::Queryable;
        let mut kinds = crate::bankops::KINDS;
        kinds.sort_by_key(|k| mix_index(*k));
        for w in [1110.0, 615.0] {
            let mut h = bank_page_harness(w);
            h.run();
            let rects: Vec<Rect> = kinds.iter().map(|k| h.get_by_label(k.label()).rect()).collect();
            for row in 0..3 {
                let (a, b) = (rects[row * 2], rects[row * 2 + 1]);
                assert!((a.center().y - b.center().y).abs() <= 0.5, "{w}: row {row} on one line: {a:?} {b:?}");
                assert!(a.right() < b.left(), "{w}: row {row} left then right, apart: {a:?} {b:?}");
                if row > 0 {
                    assert!(rects[row * 2 - 2].bottom() <= a.top() + 0.5, "{w}: row {row} under the row before");
                }
            }
            for col in 0..2 {
                let x = rects[col].left();
                for row in 1..3 {
                    assert!((rects[row * 2 + col].left() - x).abs() <= 0.5, "{w}: column {col} shares its x at row {row}: {:?} vs {x}", rects[row * 2 + col]);
                }
            }
            let title = h.get_by_label("Six question types").rect();
            let next = h.get_by_label("Topics and difficulty").rect();
            for (k, r) in kinds.iter().zip(&rects) {
                assert!(r.top() >= title.bottom() - 0.5 || r.left() >= title.right(), "{w}: {} under or beside its title", k.label());
                if w >= EMPTY_WIDE {
                    assert!(r.right() < next.left() && r.top() > title.bottom(), "{w}: {} inside the first column", k.label());
                } else {
                    assert!(r.bottom() < next.top() && r.right() <= w + 0.5, "{w}: {} inside the first row", k.label());
                }
            }
        }
    }

}
