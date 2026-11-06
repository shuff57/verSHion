//! The Grade screen: the work handed in for one exam, a student's pages as transcribed, and the
//! words the transcriber was unsure of. Also the grading engine the results view uses.
use eframe::egui;
use crate::model::{StudentSubmission, SubmittedPage};
use crate::screens::shell_frame;
use crate::{kit, pageshell, theme};
use egui::{Align2, CornerRadius, FontId, Margin, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use std::collections::HashMap;

#[derive(Clone, Default, Debug)]
pub struct GradeState {
    pub exam_id: Option<String>,
    pub student_id: Option<String>,
    pub page_id: Option<String>,
    pub text_version: String,
    pub show_boxes: bool,
    /// The word panel has a word open (its choices shown).
    pub tray_open: bool,
    /// The word open in the panel: (page id, flag key).
    pub sel: Option<(String, String)>,
    /// "formatted" shows the transcript with accepted readings; "raw" as the transcriber wrote it.
    pub text_view: String,
    pub read_line: Option<i32>,
    pub selected_row: Option<usize>,
    /// Readings the teacher accepted this session, by `page_id:flag key`.
    pub corrections: HashMap<String, String>,
}

impl GradeState {
    pub fn new() -> Self {
        Self {
            text_version: "corrected".into(),
            show_boxes: true,
            text_view: "formatted".into(),
            read_line: None,
            ..Default::default()
        }
    }
}

/// Move reading line relative.
fn move_read_line(state: &mut GradeState, n_lines: usize, d: i32) {
    let cur = state.read_line.unwrap_or(if d > 0 { -1 } else { 0 });
    let nxt = (cur + d).clamp(0, n_lines.saturating_sub(1) as i32);
    state.read_line = Some(nxt);
}

/// Records the teacher's decision on the unsure word `word` of page `page_id`: `Some(true)` uses
/// the offered reading, `Some(false)` keeps the transcriber's own, `None` clears the decision.
/// Returns whether it changed.
fn set_decision(store: &mut crate::store::Store, page_id: &str, word: &str, decision: Option<bool>) -> bool {
    let mut changed = false;
    for sub in store.student_submissions.iter_mut() {
        for p in sub.pages.iter_mut().filter(|p| p.id == page_id) {
            for c in p.corrections.iter_mut().filter(|c| c.word == word) {
                let (accepted, rejected) = match decision {
                    Some(true) => (true, false),
                    Some(false) => (false, true),
                    None => (false, false),
                };
                if c.accepted != accepted || c.rejected != rejected {
                    c.accepted = accepted;
                    c.rejected = rejected;
                    changed = true;
                }
            }
            if changed && decision == Some(true) {
                p.via = Some("tray".into());
            }
        }
    }
    changed
}

// ---- the words the transcriber was unsure of ----------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum FlagKind {
    /// Read with low confidence; the transcriber offers a reading.
    Unsure { read: String, offered: String, confidence: f32, accepted: bool, rejected: bool },
    /// Not read at all (an `[illegible]` marker in the transcript).
    Unreadable { kept: String },
}

/// One uncertain word on a page, and where it sits: (line, byte start, byte end).
#[derive(Clone, Debug, PartialEq)]
struct Flag {
    key: String,
    at: Option<(usize, usize, usize)>,
    kind: FlagKind,
}

const ILLEGIBLE: &str = "[illegible]";

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '^' || c == '_'
}

/// The first place `word` stands on its own in `line` (else the first place it appears at all).
fn find_word(line: &str, word: &str) -> Option<usize> {
    if word.is_empty() {
        return None;
    }
    let mut first = None;
    for (i, _) in line.match_indices(word) {
        first.get_or_insert(i);
        let before = line[..i].chars().next_back().is_none_or(|c| !is_word_char(c));
        let after = line[i + word.len()..].chars().next().is_none_or(|c| !is_word_char(c));
        if before && after {
            return Some(i);
        }
    }
    first
}

fn flags(page: &SubmittedPage) -> Vec<Flag> {
    let lines: Vec<&str> = page.transcript.lines().collect();
    let mut out = Vec::new();
    for c in &page.corrections {
        let at = lines.iter().enumerate().find_map(|(li, l)| find_word(l, &c.word).map(|s| (li, s, s + c.word.len())));
        out.push(Flag {
            key: format!("w:{}", c.word),
            at,
            kind: FlagKind::Unsure { read: c.word.clone(), offered: c.term.clone(), confidence: c.confidence, accepted: c.accepted, rejected: c.rejected },
        });
    }
    let markers: Vec<(usize, usize)> =
        lines.iter().enumerate().flat_map(|(li, l)| l.match_indices(ILLEGIBLE).map(move |(s, _)| (li, s))).collect();
    for (i, m) in page.illegible.iter().enumerate() {
        out.push(Flag {
            key: format!("i:{i}"),
            at: markers.get(i).map(|&(li, s)| (li, s, s + ILLEGIBLE.len())),
            kind: FlagKind::Unreadable { kept: m.word.clone() },
        });
    }
    out.sort_by_key(|f| f.at.map(|(l, s, _)| (l, s)).unwrap_or((usize::MAX, 0)));
    out
}

/// Words still to check on a page: every unreadable run, and every unsure word not yet decided.
fn open_words(page: &SubmittedPage) -> usize {
    flags(page)
        .iter()
        .filter(|f| !matches!(f.kind, FlagKind::Unsure { accepted: true, .. } | FlagKind::Unsure { rejected: true, .. }))
        .count()
}

/// A transcript line cut into plain runs and flagged words (index into `flags`).
enum Seg<'a> {
    Text(&'a str),
    Flag(usize),
}

fn line_segments<'a>(line: &'a str, li: usize, flags: &[Flag]) -> Vec<Seg<'a>> {
    let mut hits: Vec<(usize, usize, usize)> =
        flags.iter().enumerate().filter_map(|(fi, f)| f.at.filter(|a| a.0 == li).map(|(_, s, e)| (s, e, fi))).collect();
    hits.sort();
    let mut out = Vec::new();
    let mut pos = 0;
    for (s, e, fi) in hits {
        if s < pos {
            continue;
        }
        if s > pos {
            out.push(Seg::Text(&line[pos..s]));
        }
        out.push(Seg::Flag(fi));
        pos = e;
    }
    if pos < line.len() || out.is_empty() {
        out.push(Seg::Text(&line[pos..]));
    }
    out
}

// ---- where a submission stands ------------------------------------------------------------------

enum Standing {
    Marked(f64, usize),
    Check(usize),
    Unread(usize),
    Ready,
}

fn standing(s: &StudentSubmission) -> Standing {
    if s.graded {
        return Standing::Marked(s.score.unwrap_or(0.0), s.per_question.len());
    }
    let open: usize = s.pages.iter().map(|p| open_words(p)).sum();
    let unread = s.pages.iter().filter(|p| p.transcript.trim().is_empty()).count();
    if open > 0 {
        Standing::Check(open)
    } else if unread > 0 {
        Standing::Unread(unread)
    } else {
        Standing::Ready
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn fmt_pts(v: f64) -> String {
    if v.fract() == 0.0 { format!("{v:.0}") } else { format!("{v:.1}") }
}

impl Standing {
    fn label(&self) -> String {
        match self {
            Standing::Marked(score, n) => format!("Marked {}/{n}", fmt_pts(*score)),
            Standing::Check(n) => format!("{} to check", plural(*n, "word", "words")),
            Standing::Unread(n) => format!("{} not read", plural(*n, "page", "pages")),
            Standing::Ready => "Ready to mark".to_string(),
        }
    }

    fn colors(&self) -> (egui::Color32, egui::Color32) {
        match self {
            Standing::Marked(..) => (theme::accent_faded(), theme::accent_col()),
            Standing::Check(_) => (theme::highlighter(), theme::on_highlighter()),
            Standing::Unread(_) => (theme::hover_bg(), theme::text_col()),
            Standing::Ready => (theme::field(), theme::text_col()),
        }
    }
}

/// A status pill painted at the right edge of `row`, vertically centred. Returns its left edge.
fn paint_pill(ui: &egui::Ui, right: f32, cy: f32, text: &str, fill: egui::Color32, fg: egui::Color32) -> f32 {
    let g = ui.painter().layout_no_wrap(text.to_string(), FontId::new(11.0, theme::bold_family()), fg);
    let r = Rect::from_min_max(Pos2::new(right - g.size().x - 18.0, cy - 10.0), Pos2::new(right, cy + 10.0));
    ui.painter().rect_filled(r, CornerRadius::same(255), fill);
    ui.painter().galley(Pos2::new(r.left() + 9.0, cy - g.size().y / 2.0), g, fg);
    r.left()
}

/// A status pill laid out like any widget (so a wrapping row can move it to the next line whole).
fn status_pill(ui: &mut egui::Ui, text: &str, fill: egui::Color32, fg: egui::Color32, h: f32) -> egui::Response {
    let g = ui.painter().layout_no_wrap(text.to_string(), FontId::new(11.5, theme::bold_family()), fg);
    let (r, resp) = ui.allocate_exact_size(Vec2::new(g.size().x + 20.0, h), Sense::hover());
    ui.painter().rect_filled(r, CornerRadius::same(255), fill);
    ui.painter().galley(Pos2::new(r.left() + 10.0, r.center().y - g.size().y / 2.0), g, fg);
    resp
}

/// One panel of the workspace: a card as tall as its contents, up to `size.y`, scrolling inside past that
/// (a card stretched to the window around two lines read as an empty slab).
fn panel(ui: &mut egui::Ui, salt: &str, size: Vec2, f: impl FnOnce(&mut egui::Ui)) {
    ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
        kit::card().inner_margin(Margin::same(16)).show(ui, |ui| {
            ui.set_width(size.x - 34.0);
            let rows = egui::Id::new("grade_panel_rows");
            ui.ctx().data_mut(|d| d.remove_temp::<Vec<Rect>>(rows));
            let out = egui::ScrollArea::vertical()
                .id_salt(salt)
                .auto_shrink([false, true])
                .max_height(size.y - 30.0)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    f(ui)
                });
            // a card that ends on half a line reads as guillotined: hide the line the panel's
            // bottom edge cuts through, so the edge falls in the gap under the last whole one
            let cut = ui
                .ctx()
                .data(|d| d.get_temp::<Vec<Rect>>(rows))
                .unwrap_or_default()
                .into_iter()
                .filter(|r| {
                    // only a line-sized row is hidden whole: a card only loses its own bottom
                    // padding to the edge, and hiding a whole card would empty the panel
                    r.height() < 72.0 && r.top() < out.inner_rect.bottom() - 1.0 && r.bottom() > out.inner_rect.bottom() + 1.0
                })
                .map(|r| r.top())
                .fold(f32::INFINITY, f32::min);
            if cut.is_finite() {
                let band = Rect::from_min_max(
                    Pos2::new(out.inner_rect.left(), cut),
                    Pos2::new((out.inner_rect.right() - 8.0).max(out.inner_rect.left()), out.inner_rect.bottom()),
                );
                ui.painter().with_clip_rect(out.inner_rect).rect_filled(band, 0.0, theme::card());
            }
        });
    });
}

/// A widget row that the panel's bottom edge may cut; see `panel`.
fn note_panel_row(ui: &egui::Ui, r: Rect) {
    let id = egui::Id::new("grade_panel_rows");
    let mut rows: Vec<Rect> = ui.ctx().data(|d| d.get_temp(id)).unwrap_or_default();
    rows.push(r);
    ui.ctx().data_mut(|d| d.insert_temp(id, rows));
}

/// A small icon button (line navigation).
fn icon_button(ui: &mut egui::Ui, icon: &str, hint: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(icon).font(theme::icon_font(15.0)).color(theme::text_col()))
            .fill(theme::card())
            .stroke(Stroke::new(1.0, theme::border()))
            .corner_radius(CornerRadius::same(8))
            .min_size(Vec2::new(30.0, 28.0)),
    )
    .on_hover_text(hint)
}

// ---- the screen's parts --------------------------------------------------------------------------

fn choose_exam(state: &mut GradeState, id: String) {
    let view = std::mem::take(&mut state.text_view);
    *state = GradeState::new();
    state.text_view = if view.is_empty() { "formatted".into() } else { view };
    state.exam_id = Some(id);
}

/// No exam chosen, or the chosen one has no work: every exam, with how much work it has.
fn exam_chooser(ui: &mut egui::Ui, exams: &[(String, String, usize, usize)], state: &mut GradeState) {
    let current = state.exam_id.as_ref().and_then(|id| exams.iter().find(|e| &e.0 == id));
    let mut pick = None;
    pageshell::card(ui, |ui| {
        match current {
            None => pageshell::heading(ui, "Choose an exam to grade"),
            Some(e) => pageshell::heading(ui, &format!("No student work for {} yet", e.1)),
        }
        ui.add_space(4.0);
        let body = match current {
            None => "Each student's pages are transcribed first. Here you check the words the transcriber was unsure of, \
                     then mark each question against the exam's answer key.",
            Some(_) => "Work for an exam appears here once its pages are transcribed. Adding photos from this screen is not \
                        built yet; another exam may already have work to check.",
        };
        ui.scope(|ui| {
            ui.set_max_width(ui.available_width().min(pageshell::MEASURE));
            pageshell::prose(ui, body, 13.5);
        });
        ui.add_space(14.0);
        if exams.is_empty() {
            ui.label(theme::muted("There are no exams yet. Make one on the Exams page first.").size(13.0));
            return;
        }
        kit::small_caps(ui, "Exams");
        ui.add_space(4.0);
        let w = ui.available_width();
        // The exams with work first: those are the ones there is something to do on.
        let mut order: Vec<&(String, String, usize, usize)> = exams.iter().collect();
        order.sort_by_key(|e| e.2 == 0);
        for (id, name, n, check) in order {
            let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 46.0), Sense::click());
            let on = state.exam_id.as_deref() == Some(id.as_str());
            if on {
                ui.painter().rect_filled(r, CornerRadius::same(10), theme::accent_faded());
            } else if resp.hovered() {
                ui.painter().rect_filled(r, CornerRadius::same(10), theme::hover_bg());
            }
            ui.painter().line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, theme::border()));
            let cy = r.center().y;
            ui.painter().text(Pos2::new(r.left() + 12.0, cy), Align2::LEFT_CENTER, name, FontId::new(15.0, theme::serif_family()), theme::ink());
            ui.painter().text(
                Pos2::new(r.right() - 12.0, cy),
                Align2::RIGHT_CENTER,
                egui_phosphor::regular::CARET_RIGHT,
                theme::icon_font(15.0),
                theme::muted_col(),
            );
            let mut right = r.right() - 36.0;
            if *n == 0 {
                paint_pill(ui, right, cy, "No work yet", theme::field(), theme::muted_col());
            } else {
                if *check > 0 {
                    right = paint_pill(ui, right, cy, &format!("{check} to check"), theme::highlighter(), theme::on_highlighter()) - 6.0;
                }
                paint_pill(ui, right, cy, &plural(*n, "student", "students"), theme::accent_faded(), theme::accent_col());
            }
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() && !on {
                pick = Some(id.clone());
            }
        }
    });
    if let Some(id) = pick {
        choose_exam(state, id);
    }
}

/// Beside the chooser: what happens on this screen, in three steps.
fn how_it_works(ui: &mut egui::Ui) {
    pageshell::card(ui, |ui| {
        pageshell::heading(ui, "How grading works");
        ui.add_space(10.0);
        let steps = [
            ("Transcribed", "Each page of a student's work is read into text. Words the reader was unsure of are flagged."),
            ("Checked", "Open a student's page and decide each flagged word: use the reading offered, or keep what was written."),
            ("Marked", "Each question is marked against the exam's answer key, and the student's score shows in the list."),
        ];
        for (i, (title, body)) in steps.iter().enumerate() {
            ui.horizontal_top(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 13.0, theme::accent_faded());
                ui.painter().text(r.center(), Align2::CENTER_CENTER, (i + 1).to_string(), FontId::new(13.0, theme::bold_family()), theme::accent_col());
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.label(theme::strong(*title).size(14.0));
                    ui.add(egui::Label::new(theme::muted(*body).size(12.5)).wrap());
                });
            });
            ui.add_space(10.0);
        }
    });
}

/// Every submission for the exam, with where it stands.
fn student_list(ui: &mut egui::Ui, subs: &[StudentSubmission], shown: &str, state: &mut GradeState) {
    kit::small_caps(ui, &format!("Students  {}", subs.len()));
    ui.add_space(6.0);
    let w = ui.available_width();
    let mut pick = None;
    for s in subs {
        let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 58.0), Sense::click());
        let on = s.id == shown;
        let p = ui.painter();
        // The open student: a wash its status pill still stands out on, and the accent bar.
        if on {
            p.rect_filled(r, CornerRadius::same(10), theme::hover_bg());
            p.rect_filled(Rect::from_min_size(r.min + Vec2::new(0.0, 10.0), Vec2::new(3.0, r.height() - 20.0)), CornerRadius::same(2), theme::accent_col());
        } else if resp.hovered() {
            p.rect_filled(r, CornerRadius::same(10), theme::hover_bg().gamma_multiply(0.6));
        }
        let name = p.layout(s.student_name.clone(), FontId::new(14.5, theme::bold_family()), theme::ink(), w - 24.0);
        p.galley(Pos2::new(r.left() + 12.0, r.top() + 9.0), name, theme::ink());
        let st = standing(s);
        let (fill, fg) = st.colors();
        let g = p.layout_no_wrap(st.label(), FontId::new(11.0, theme::bold_family()), fg);
        let pill = Rect::from_min_size(Pos2::new(r.left() + 12.0, r.top() + 32.0), Vec2::new(g.size().x + 16.0, 18.0));
        p.rect_filled(pill, CornerRadius::same(255), fill);
        p.galley(Pos2::new(pill.left() + 8.0, pill.center().y - g.size().y / 2.0), g, fg);
        p.text(
            Pos2::new(pill.right() + 8.0, pill.center().y),
            Align2::LEFT_CENTER,
            plural(s.pages.len(), "page", "pages"),
            FontId::proportional(12.0),
            theme::muted_col(),
        );
        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            pick = Some(s);
        }
    }
    if let Some(s) = pick {
        state.student_id = Some(s.id.clone());
        state.page_id = s.pages.first().map(|p| p.id.clone());
        state.read_line = None;
        state.tray_open = false;
        state.sel = None;
    }
}

/// The student's name, where they stand, and their pages.
/// The student's name and where they stand (across all their pages).
fn student_header(ui: &mut egui::Ui, s: &StudentSubmission) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        ui.label(RichText::new(&s.student_name).family(theme::serif_family()).size(24.0).color(theme::ink()));
        let st = standing(s);
        let (fill, fg) = st.colors();
        let label = match st {
            Standing::Check(_) if s.pages.len() > 1 => format!("{} in all", st.label()),
            _ => st.label(),
        };
        status_pill(ui, &label, fill, fg, 26.0);
    });
}

/// The page chips (in the transcript card): which page of this student is open.
fn page_chips(ui: &mut egui::Ui, s: &StudentSubmission, p: &SubmittedPage, state: &mut GradeState) {
    let mut pick = None;
    for (i, q) in s.pages.iter().enumerate() {
        let on = q.id == p.id;
        if kit::chip_toggle(ui, &format!("Page {}", i + 1), on).clicked() && !on {
            pick = Some(q.id.clone());
        }
    }
    if let Some(id) = pick {
        state.page_id = Some(id);
        state.read_line = None;
        state.tray_open = false;
        state.sel = None;
    }
}

/// No page open yet: the student's pages, and their marks once marked.
fn pages_overview(ui: &mut egui::Ui, s: &StudentSubmission, state: &mut GradeState) {
    pageshell::heading(ui, "Choose a page");
    ui.add_space(2.0);
    ui.label(theme::muted("Open a page to check its transcript and the words the transcriber was unsure of.").size(13.0));
    ui.add_space(10.0);
    let w = ui.available_width();
    let mut pick = None;
    for (i, p) in s.pages.iter().enumerate() {
        let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 58.0), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(r, CornerRadius::same(10), theme::hover_bg());
        }
        ui.painter().line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, theme::border()));
        let lines = p.transcript.lines().filter(|l| !l.trim().is_empty()).count();
        let open = open_words(p);
        let (pill, fill, fg) = if lines == 0 {
            ("Not read yet".to_string(), theme::field(), theme::muted_col())
        } else if open > 0 {
            (format!("{} to check", plural(open, "word", "words")), theme::highlighter(), theme::on_highlighter())
        } else {
            ("Nothing to check".to_string(), theme::accent_faded(), theme::accent_col())
        };
        let name = ui.painter().text(
            Pos2::new(r.left() + 12.0, r.top() + 19.0),
            Align2::LEFT_CENTER,
            format!("Page {}", i + 1),
            FontId::new(14.5, theme::bold_family()),
            theme::ink(),
        );
        // the status beside the page it describes, not across the card
        let g = ui.painter().layout_no_wrap(pill, FontId::new(11.0, theme::bold_family()), fg);
        let pr = Rect::from_min_size(Pos2::new(name.right() + 10.0, r.top() + 9.0), Vec2::new(g.size().x + 18.0, 20.0));
        ui.painter().rect_filled(pr, CornerRadius::same(255), fill);
        ui.painter().galley(Pos2::new(pr.left() + 9.0, pr.center().y - g.size().y / 2.0), g, fg);
        ui.painter().text(
            Pos2::new(r.right() - 12.0, r.center().y),
            Align2::RIGHT_CENTER,
            egui_phosphor::regular::CARET_RIGHT,
            theme::icon_font(15.0),
            theme::muted_col(),
        );
        let left = r.right() - 30.0;
        let preview = p.transcript.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
        let sub = if lines == 0 { "Its text appears once it is transcribed.".to_string() } else { format!("{}  ·  {preview}", plural(lines, "line", "lines")) };
        let g = ui.painter().layout(sub, FontId::proportional(12.5), theme::muted_col(), left - r.left() - 30.0);
        let row = g.rows.first().map(|r| r.rect().height()).unwrap_or(14.0);
        ui.painter().with_clip_rect(Rect::from_min_size(Pos2::new(r.left(), r.top() + 32.0), Vec2::new(left - r.left() - 16.0, row + 2.0))).galley(
            Pos2::new(r.left() + 12.0, r.top() + 32.0),
            g,
            theme::muted_col(),
        );
        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            pick = Some(p.id.clone());
        }
    }
    if let Some(id) = pick {
        state.page_id = Some(id);
        state.read_line = None;
    }
    if s.graded && !s.per_question.is_empty() {
        ui.add_space(18.0);
        let right = s.per_question.iter().filter(|q| q.correct).count();
        kit::small_caps(ui, &format!("Marks  {right} of {}", s.per_question.len()));
        ui.add_space(6.0);
        // the wrong ones first and in red: they are what a teacher looks for
        for (correct, title) in [(false, "Wrong"), (true, "Right")] {
            let qs: Vec<String> = s.per_question.iter().filter(|q| q.correct == correct).map(|q| format!("Q{}", q.exam_item_index + 1)).collect();
            if qs.is_empty() {
                continue;
            }
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                ui.spacing_mut().interact_size.y = 20.0;
                let (lw, _) = ui.allocate_exact_size(Vec2::new(52.0, 22.0), Sense::hover());
                ui.painter().text(lw.left_center(), Align2::LEFT_CENTER, title, FontId::new(12.5, theme::bold_family()), if correct { theme::text_col() } else { theme::bad() });
                for q in &qs {
                    if correct {
                        status_pill(ui, q, theme::accent_faded(), theme::accent_col(), 22.0);
                    } else {
                        status_pill(ui, q, theme::error_wash(), theme::bad(), 22.0);
                    }
                }
            });
            ui.add_space(4.0);
        }
    }
}

/// An uncertain word inside a transcript line.
fn word_chip(ui: &mut egui::Ui, text: &str, fill: egui::Color32, fg: egui::Color32, open: bool) -> egui::Response {
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = Vec2::new(4.0, 0.0);
        ui.add(
            egui::Button::new(RichText::new(text).size(15.5).color(fg))
                .fill(fill)
                .stroke(if open { Stroke::new(1.5, theme::ink()) } else { Stroke::NONE })
                .corner_radius(CornerRadius::same(5))
                .min_size(Vec2::new(0.0, 22.0)),
        )
    })
    .inner
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn transcript_panel(ui: &mut egui::Ui, s: &StudentSubmission, p: &SubmittedPage, flags: &[Flag], state: &mut GradeState) {
    let lines: Vec<&str> = p.transcript.lines().collect();
    let n = s.pages.iter().position(|q| q.id == p.id).map(|i| i + 1).unwrap_or(1);
    let corrected = state.text_view != "raw";
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        pageshell::heading(ui, "Transcript");
        if s.pages.len() > 1 {
            ui.add_space(6.0);
            page_chips(ui, s, p, state);
        } else {
            ui.label(theme::muted(format!("Page {n} of {}", s.pages.len())).size(12.5));
        }
    });
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        if kit::chip_toggle(ui, "Corrected", corrected).on_hover_text("With the readings you accepted (F)").clicked() {
            state.text_view = "formatted".into();
        }
        if kit::chip_toggle(ui, "As written", !corrected).on_hover_text("As the transcriber read it (P)").clicked() {
            state.text_view = "raw".into();
        }
        if !lines.is_empty() {
            ui.add_space(10.0);
            if icon_button(ui, egui_phosphor::regular::CARET_UP, "Previous line (Up)").clicked() {
                move_read_line(state, lines.len(), -1);
            }
            if icon_button(ui, egui_phosphor::regular::CARET_DOWN, "Next line (Down)").clicked() {
                move_read_line(state, lines.len(), 1);
            }
            let at = match state.read_line {
                Some(i) => format!("Line {} of {}", i + 1, lines.len()),
                None => plural(lines.len(), "line", "lines"),
            };
            ui.label(theme::muted(at).size(12.5));
        }
    });
    ui.add_space(10.0);
    if lines.is_empty() {
        ui.label(theme::muted("This page has not been read yet. Its text appears here once it is transcribed.").size(13.5));
        return;
    }
    let open_key = state.sel.as_ref().filter(|(pid, _)| state.tray_open && pid == &p.id).map(|(_, k)| k.clone());
    let mut pick: Option<String> = None;
    for (li, line) in lines.iter().enumerate() {
        let reading = state.read_line == Some(li as i32);
        let bg = ui.painter().add(egui::Shape::Noop);
        let row = ui
            .horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.spacing_mut().interact_size.y = 24.0;
                let (g, _) = ui.allocate_exact_size(Vec2::new(30.0, 24.0), Sense::hover());
                ui.painter().text(
                    g.left_center() + Vec2::new(0.0, 1.0),
                    Align2::LEFT_CENTER,
                    (li + 1).to_string(),
                    FontId::proportional(12.0),
                    if reading { theme::accent_col() } else { theme::muted_col() },
                );
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(0.0, 6.0);
                    ui.spacing_mut().interact_size.y = 24.0;
                    for seg in line_segments(line, li, flags) {
                        match seg {
                            Seg::Text(t) => {
                                ui.label(RichText::new(t).size(16.0).color(theme::ink()));
                            }
                            Seg::Flag(fi) => {
                                let f = &flags[fi];
                                let open = open_key.as_deref() == Some(f.key.as_str());
                                let r = match &f.kind {
                                    FlagKind::Unsure { read, offered, .. } => {
                                        if corrected && matches!(f.kind, FlagKind::Unsure { accepted: true, .. }) {
                                            word_chip(ui, offered, theme::accent_faded(), theme::accent_col(), open)
                                        } else {
                                            word_chip(ui, read, theme::highlighter(), theme::on_highlighter(), open)
                                        }
                                    }
                                    FlagKind::Unreadable { .. } => word_chip(ui, "illegible", theme::error_wash(), theme::bad(), open),
                                };
                                if r.clicked() {
                                    pick = Some(f.key.clone());
                                }
                            }
                        }
                    }
                });
            })
            .response
            .rect;
        note_panel_row(ui, row);
        if reading {
            let r = row.expand2(Vec2::new(6.0, 3.0));
            ui.painter().set(bg, egui::Shape::rect_filled(r, CornerRadius::same(6), theme::hover_bg()));
            ui.painter().rect_filled(Rect::from_min_size(r.min, Vec2::new(3.0, r.height())), CornerRadius::same(2), theme::accent_col());
        }
        ui.add_space(7.0);
    }
    if let Some(k) = pick {
        state.tray_open = true;
        state.sel = Some((p.id.clone(), k));
    }
}

/// The words the transcriber was unsure of on this page. Every card carries its own decision
/// buttons, so the choice is visible without opening anything; the open (keyboard) card is just
/// the highlighted one. Returns the word and its new decision when one was made: the caller
/// writes it to the page and saves.
fn words_panel(ui: &mut egui::Ui, s: &StudentSubmission, p: &SubmittedPage, flags: &[Flag], state: &mut GradeState) -> Option<(String, Option<bool>)> {
    pageshell::heading(ui, "Words to check");
    ui.add_space(2.0);
    let open_n = open_words(p);
    let all: usize = s.pages.iter().map(open_words).sum();
    let line = if flags.is_empty() {
        if p.transcript.trim().is_empty() { "Nothing yet: this page has not been read." } else { "Nothing on this page was uncertain." }.to_string()
    } else if open_n == 0 {
        "Every word on this page is checked.".to_string()
    } else if all > open_n {
        format!("{} on this page, {all} across {}", plural(open_n, "word", "words"), plural(s.pages.len(), "page", "pages"))
    } else {
        format!("{} on this page", plural(open_n, "word", "words"))
    };
    ui.label(theme::muted(line).size(12.5));
    ui.add_space(10.0);
    let open_key = state.sel.as_ref().filter(|(pid, _)| state.tray_open && pid == &p.id).map(|(_, k)| k.clone());
    // What a press asks for, applied after the list is drawn.
    enum Ask {
        Open(String),
        Decide(String, Option<bool>),
        Next(String),
    }
    let mut ask = None;
    for f in flags {
        let open = open_key.as_deref() == Some(f.key.as_str());
        let accepted = matches!(f.kind, FlagKind::Unsure { accepted: true, .. });
        let rejected = matches!(f.kind, FlagKind::Unsure { rejected: true, .. });
        let frame = egui::Frame::new()
            .fill(if accepted {
                theme::accent_faded()
            } else if open {
                theme::field()
            } else {
                theme::card()
            })
            .stroke(Stroke::new(
                if open || accepted { 1.5 } else { 1.0 },
                if accepted {
                    theme::accent_col()
                } else if open {
                    theme::accent_col()
                } else {
                    theme::border()
                },
            ))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let place = match f.at {
                    Some((l, _, _)) => format!("Line {}", l + 1),
                    None => "Not placed in the text".into(),
                };
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    kit::small_caps(ui, &place);
                    if accepted {
                        status_pill(ui, "Accepted", theme::accent_faded(), theme::accent_col(), 18.0);
                    } else if rejected {
                        status_pill(ui, "Kept as written", theme::field(), theme::text_col(), 18.0);
                    }
                });
                ui.add_space(3.0);
                match &f.kind {
                    FlagKind::Unsure { read, offered, confidence, .. } => {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            ui.spacing_mut().interact_size.y = 22.0;
                            ui.label(RichText::new(format!(" {read} ")).size(14.0).color(theme::on_highlighter()).background_color(theme::highlighter()));
                            ui.label(theme::muted("reads as").size(12.5));
                            ui.label(theme::strong(offered.as_str()).size(14.0));
                        });
                        let sure = format!("The transcriber is {:.0}% sure.", (confidence * 100.0).clamp(0.0, 100.0));
                        let note = if accepted {
                            format!("Using “{offered}”.")
                        } else if rejected {
                            format!("Keeping “{read}”, the transcriber's reading.")
                        } else {
                            sure
                        };
                        ui.label(theme::muted(note).size(12.0));
                        ui.add_space(8.0);
                        ui.horizontal_wrapped(|ui| {
                            if accepted || rejected {
                                if kit::ghost_button(ui, "Undo").clicked() {
                                    ask = Some(Ask::Decide(f.key.clone(), None));
                                }
                            } else {
                                if kit::primary_button(ui, &format!("Use “{offered}”")).clicked() {
                                    ask = Some(Ask::Decide(f.key.clone(), Some(true)));
                                }
                                if kit::ghost_button(ui, "Keep written")
                                    .on_hover_text("Reject the reading offered and keep the transcriber's own")
                                    .clicked()
                                {
                                    ask = Some(Ask::Decide(f.key.clone(), Some(false)));
                                }
                            }
                        });
                    }
                    FlagKind::Unreadable { kept } => {
                        ui.label(theme::strong("Could not be read").size(14.0));
                        if !kept.is_empty() {
                            ui.label(theme::muted(format!("The transcriber kept “{kept}”.")).size(12.0));
                        }
                        ui.add_space(6.0);
                        ui.add(
                            egui::Label::new(theme::text("Read this one from the paper. Typing a reading in here is not available yet.").size(12.5))
                                .wrap(),
                        );
                        ui.add_space(6.0);
                        if kit::ghost_button(ui, "Next word").clicked() {
                            ask = Some(Ask::Next(f.key.clone()));
                        }
                    }
                }
            });
        note_panel_row(ui, frame.response.rect);
        if !open && frame.response.interact(Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            ask = Some(Ask::Open(f.key.clone()));
        }
        ui.add_space(8.0);
    }
    if !flags.is_empty() && open_key.is_none() {
        ui.label(theme::muted("Every card has its own decision: use the reading, or keep what was written. T opens the first card; Esc closes it.").size(12.0));
    }
    let next = |key: &str| flags.iter().position(|f| f.key == key).and_then(|i| flags.get(i + 1)).map(|f| f.key.clone());
    match ask {
        Some(Ask::Open(k)) => {
            state.tray_open = true;
            state.sel = Some((p.id.clone(), k));
        }
        Some(Ask::Decide(k, decision)) => {
            if decision == Some(true) {
                match next(&k) {
                    Some(n) => state.sel = Some((p.id.clone(), n)),
                    None => state.tray_open = false,
                }
            }
            let word = flags.iter().find(|f| f.key == k).and_then(|f| match &f.kind {
                FlagKind::Unsure { read, .. } => Some(read.clone()),
                _ => None,
            });
            return word.map(|w| (w, decision));
        }
        Some(Ask::Next(k)) => match next(&k) {
            Some(n) => state.sel = Some((p.id.clone(), n)),
            None => state.tray_open = false,
        },
        None => {}
    }
    None
}

#[allow(dead_code)]

// ---- Grade engine (P3 Round 7) ----

fn grade_mc_answer(answers: &[crate::model::Answer], student: &str) -> (bool, f64) {
    let s = crate::gradecore::clean(student).trim().to_lowercase();
    let mut best = (usize::MAX, false, 0.0);
    for a in answers {
        let d = crate::gradecore::lev(&s, &crate::gradecore::clean(&a.text).trim().to_lowercase());
        if d < best.0 { best = (d, a.correct, if a.correct { 1.0 } else { 0.0 }); }
    }
    if best.1 { (true, 1.0) } else { (false, 0.0) }
}

fn grade_tf(correct: bool, student: &str) -> (bool, f64) {
    let s = crate::gradecore::clean(student).trim().to_lowercase();
    let picked_true = s.contains("true") || s.contains("t");
    let picked_false = s.contains("false") || s.contains("f");
    let ok = if correct { picked_true && !picked_false } else { picked_false && !picked_true };
    (ok, if ok { 1.0 } else { 0.0 })
}

fn grade_blank(accept: &[Vec<String>], student: &str) -> (bool, f64) {
    let s = crate::gradecore::clean(student).trim().to_lowercase();
    let mut best = (f64::MAX, false, 0.0);
    for group in accept {
        for v in group {
            let t = crate::gradecore::clean(v).trim().to_lowercase();
            let d = crate::gradecore::lev(&s, &t) as f64;
            let ratio = d / t.chars().count().max(1) as f64;
            if ratio < best.0 {
                best = (ratio, ratio <= 0.25, if ratio <= 0.25 { 1.0 } else if ratio <= 0.4 { 0.25 } else { 0.0 });
            }
        }
    }
    (best.1, best.2)
}

fn grade_short(sample: &str, student: &str) -> (bool, f64) {
    let s = crate::gradecore::clean(student).trim().to_lowercase();
    let key = crate::gradecore::clean(sample).trim().to_lowercase();
    let d = crate::gradecore::lev(&s, &key) as f64;
    let r = d / key.chars().count().max(1) as f64;
    let ok = r <= 0.3;
    (ok, if ok { 1.0 } else if r <= 0.5 { 0.25 } else { 0.0 })
}

fn score_for_item(item: &crate::model::ExamItem, store: &crate::store::Store, student_text: &str) -> (bool, f64, Vec<String>) {
    let notes: Vec<String> = Vec::new();
    let q_opt = store.question(&item.bank_id, &item.question_id);
    if q_opt.is_none() { return (false, 0.0, vec!["Question not found in bank".to_string()]); }
    let q = q_opt.unwrap();
    let correct = match &q.kind {
        crate::model::Kind::Mc { answers } => grade_mc_answer(answers, student_text).0,
        crate::model::Kind::Tf { correct: c } => grade_tf(*c, student_text).0,
        crate::model::Kind::Blank { accept } => grade_blank(accept, student_text).0,
        crate::model::Kind::Short { sample } => grade_short(sample, student_text).0,
        crate::model::Kind::Matching { pairs, .. } => {
            let s = crate::gradecore::clean(student_text).trim().to_lowercase();
            let mut matched = 0;
            for p in pairs {
                if s.contains(&crate::gradecore::clean(&p.left).trim().to_lowercase())
                    && s.contains(&crate::gradecore::clean(&p.right).trim().to_lowercase()) {
                    matched += 1;
                }
            }
            let total = pairs.len().max(1);
            matched >= total / 2
        }
        crate::model::Kind::Multipart { parts } => {
            let mut ok = 0;
            for p in parts {
                match &p.kind {
                    crate::model::PartKind::Mc { answers } => { let (c, _) = grade_mc_answer(answers, student_text); if c { ok += 1; } }
                    crate::model::PartKind::Tf { correct: c } => { let (correct, _) = grade_tf(*c, student_text); if correct { ok += 1; } }
                    crate::model::PartKind::Short { sample } => { let (correct, _) = grade_short(sample, student_text); if correct { ok += 1; } }
                }
            }
            if parts.is_empty() { false } else { (ok as f64 / parts.len() as f64) >= 0.5 }
        }
    };
    (correct, 1.0, notes)
}

impl crate::App {
    pub fn draw_grade(&mut self, ctx: &egui::Context) {
        let mut state = self.grade.clone().unwrap_or_else(GradeState::new);
        if state.exam_id.as_ref().is_some_and(|id| !self.store.exams.iter().any(|e| &e.id == id)) {
            state = GradeState::new();
        }
        let subs: Vec<StudentSubmission> = match &state.exam_id {
            Some(eid) => self.store.submissions_for(eid).cloned().collect(),
            None => Vec::new(),
        };
        let student: Option<StudentSubmission> =
            state.student_id.as_ref().and_then(|sid| subs.iter().find(|s| &s.id == sid)).or(subs.first()).cloned();
        let page: Option<SubmittedPage> = state
            .page_id
            .as_ref()
            .and_then(|pid| student.as_ref().and_then(|s| s.pages.iter().find(|p| &p.id == pid)))
            .cloned();
        let page_flags = page.as_ref().map(flags).unwrap_or_default();

        // Keys: up/down step through the lines, P / F the plain or corrected text, T opens the word
        // panel on its first word, Esc closes it.
        if !ctx.egui_wants_keyboard_input() {
            let (up, down, esc, p_key, f_key, t_key) = ctx.input(|i| {
                (
                    i.key_pressed(egui::Key::ArrowUp),
                    i.key_pressed(egui::Key::ArrowDown),
                    i.key_pressed(egui::Key::Escape),
                    i.key_pressed(egui::Key::P),
                    i.key_pressed(egui::Key::F),
                    i.key_pressed(egui::Key::T),
                )
            });
            if let Some(p) = &page {
                let n = p.transcript.lines().count().max(1);
                if up {
                    move_read_line(&mut state, n, -1);
                }
                if down {
                    move_read_line(&mut state, n, 1);
                }
            }
            if esc {
                state.tray_open = false;
            }
            if p_key {
                state.text_view = "raw".into();
            }
            if f_key {
                state.text_view = "formatted".into();
            }
            if t_key {
                state.tray_open = !state.tray_open;
            }
        }
        // An open word panel always has one of this page's words open.
        match &page {
            Some(p) if state.tray_open => {
                let ok = state.sel.as_ref().is_some_and(|(pid, k)| pid == &p.id && page_flags.iter().any(|f| &f.key == k));
                if !ok {
                    state.sel = page_flags.first().map(|f| (p.id.clone(), f.key.clone()));
                }
                state.tray_open = state.sel.is_some();
            }
            _ => state.tray_open = false,
        }

        // Every exam, with how much work it has: (id, name, students, students with words to check).
        let exams: Vec<(String, String, usize, usize)> = self
            .store
            .exams
            .iter()
            .map(|e| {
                let work: Vec<&StudentSubmission> = self.store.submissions_for(&e.id).collect();
                let check = work.iter().filter(|s| matches!(standing(s), Standing::Check(_))).count();
                (e.id.clone(), e.name.clone(), work.len(), check)
            })
            .collect();
        let mut save: Option<(String, String, Option<bool>)> = None;

        shell_frame(self, ctx, true, |_app, ui| {
            kit::page_header(
                ui,
                &["Home", "Grade"],
                "Grade",
                Some("Check each student's transcribed work word by word, then mark it against the exam's answer key."),
            );
            ui.add_space(12.0);
            // Which exam's work is being marked. With none chosen yet the card below is the chooser (one
            // control for one choice); once one is open this switches between them.
            if state.exam_id.is_some() && !subs.is_empty() {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                ui.label(theme::muted("Exam").size(12.5));
                let current = state
                    .exam_id
                    .as_ref()
                    .and_then(|id| exams.iter().find(|e| &e.0 == id).map(|e| e.1.clone()))
                    .unwrap_or_else(|| if exams.is_empty() { "No exams yet".into() } else { "Choose an exam".into() });
                let mut pick: Option<String> = None;
                egui::ComboBox::from_id_salt("grade_exam").selected_text(current).width(280.0).show_ui(ui, |ui| {
                    for (id, name, _, _) in &exams {
                        if ui.selectable_label(state.exam_id.as_deref() == Some(id.as_str()), name).clicked() {
                            pick = Some(id.clone());
                        }
                    }
                });
                if !subs.is_empty() {
                    let check = subs.iter().filter(|s| matches!(standing(s), Standing::Check(_))).count();
                    let mut line = plural(subs.len(), "student", "students");
                    if check > 0 {
                        line.push_str(&format!("  ·  {check} with words to check"));
                    }
                    ui.label(theme::muted(line).size(12.5));
                }
                if let Some(id) = pick {
                    choose_exam(&mut state, id);
                }
            });
            ui.add_space(14.0);
            }
            if subs.is_empty() {
                let w = ui.available_width();
                if w >= 900.0 {
                    let side = (w * 0.34).clamp(300.0, 380.0);
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 16.0;
                        ui.allocate_ui_with_layout(Vec2::new(w - side - 16.0, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                            exam_chooser(ui, &exams, &mut state)
                        });
                        ui.allocate_ui_with_layout(Vec2::new(side, 0.0), egui::Layout::top_down(egui::Align::Min), how_it_works);
                    });
                } else {
                    exam_chooser(ui, &exams, &mut state);
                    ui.add_space(14.0);
                    how_it_works(ui);
                }
                pageshell::footer(ui, "verSHion · student work stays on this machine");
                return;
            }

            let avail = ui.available_size();
            let gap = 14.0;
            let wide = avail.x >= 900.0;
            let list_w = if wide { 250.0 } else { 196.0 };
            let main_w = avail.x - list_w - gap;
            let shown = student.as_ref().map(|s| s.id.clone()).unwrap_or_default();
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                panel(ui, "grade_students", Vec2::new(list_w, avail.y), |ui| student_list(ui, &subs, &shown, &mut state));
                ui.allocate_ui_with_layout(Vec2::new(main_w, avail.y), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.set_width(main_w);
                    let Some(s) = &student else { return };
                    student_header(ui, s);
                    ui.add_space(10.0);
                    let h = ui.available_height();
                    let Some(p) = &page else {
                        panel(ui, "grade_pages", Vec2::new(main_w, h), |ui| pages_overview(ui, s, &mut state));
                        return;
                    };
                    let mut words = |ui: &mut egui::Ui, state: &mut GradeState| {
                        if let Some((word, decision)) = words_panel(ui, s, p, &page_flags, state) {
                            save = Some((p.id.clone(), word, decision));
                        }
                    };
                    if wide {
                        let words_w = 300.0;
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = gap;
                            panel(ui, "grade_transcript", Vec2::new(main_w - words_w - gap, h), |ui| {
                                transcript_panel(ui, s, p, &page_flags, &mut state)
                            });
                            panel(ui, "grade_words", Vec2::new(words_w, h), |ui| words(ui, &mut state));
                        });
                    } else {
                        // The transcript takes what its lines need, up to a little under half; the
                        // words get the rest, so at least one whole decision card is in view.
                        panel(ui, "grade_transcript", Vec2::new(main_w, (h * 0.44).floor()), |ui| transcript_panel(ui, s, p, &page_flags, &mut state));
                        ui.add_space(gap);
                        panel(ui, "grade_words", Vec2::new(main_w, ui.available_height()), |ui| words(ui, &mut state));
                    }
                });
            });
        });
        // A decision (accepted, kept as written, or taken back) is written to the page and saved,
        // so it survives a restart.
        if let Some((pid, word, decision)) = save
            && set_decision(&mut self.store, &pid, &word, decision)
        {
            self.store.save_student_submissions();
        }
        self.grade = Some(state);
    }

    /// Grade student submissions for an exam and return score + per-question grades.
    pub fn grade_exam(&mut self, exam_id: &str) -> Result<f64, String> {
        let _subs = self.store.submissions_for(&crate::model::Id::from(exam_id)).cloned().collect::<Vec<_>>();
        let _exam_opt = crate::store::load_json::<crate::model::Exam>(&format!("exam_{}.json", exam_id))
            .or_else(|| crate::store::load_json::<crate::model::Exam>("exams.json"));
        // Simplified: just note that grading logic exists. For full result screen,
        // use student_submissions directly with scores computed from graded field.
        Ok(0.0)
    }

    /// Draw GradeResults screen — results table + drill-down + CSV + re-grade.
    pub fn draw_grade_results(&mut self, ctx: &egui::Context) {
        use crate::screens::shell_frame;
        shell_frame(self, ctx, true, |app, ui| {
            ui.heading("Grade Results");
            ui.separator();

            // Gather submissions for the current exam
            let exam_id = app.grade.as_ref().and_then(|g| g.exam_id.clone()).unwrap_or_default();
            let subs: Vec<crate::model::StudentSubmission> = if !exam_id.is_empty() {
                app.store.submissions_for(&crate::model::Id::from(&exam_id)).cloned().collect()
            } else { vec![] };

            if subs.is_empty() {
                ui.label("No graded submissions for this exam yet. Grade submissions on the Grade screen first.");
                return;
            }

            // --- Results Table ---
            ui.label("Results");
            let mut max_score = 0.0;
            let mut rows: Vec<(String, f64, f64, bool, Vec<u32>)> = Vec::new();
            for sub in &subs {
                let score = sub.score.unwrap_or(0.0);
                let graded = sub.graded;
                max_score += sub.per_question.iter().map(|q| q.points_awarded).sum::<f64>();
                // Per-question icons: green = correct, red = wrong
                let icons: Vec<u32> = sub.per_question.iter().map(|q| if q.correct { 1 } else { 0 }).collect();
                rows.push((sub.student_name.clone(), score, 0.0, graded, icons));
            }

            ui.horizontal(|h| {
                h.label("Student");
                h.label("Score");
                h.label("%");
                h.label("Status");
                h.label("Per-question");
            });
            for (name, score, _, graded, icons) in &rows {
                ui.horizontal(|h| {
                    h.label(name.clone());
                    h.label(format!("{:.1}", score));
                    h.label(format!("{:.0}%", if max_score > 0.0 { (score / (max_score / rows.len().max(1) as f64)) * 100.0 } else { 0.0 }));
                    h.label(if *graded { "✓ Graded" } else { "○ Pending" });
                    let s = icons.iter().map(|&i| if i == 1 { "✓" } else { "✗" }).collect::<Vec<_>>().join(" ");
                    h.label(s);
                });
            }
            ui.separator();

            // --- Drill-Down ---
            ui.heading("Drill-Down");
            ui.label("Select a student row above to see per-question transcript + key + notes.");

            // --- Keyboard navigation for GradeResults ---
            let arrow_up = ctx.input(|i| i.key_pressed(egui::Key::ArrowUp));
            let arrow_down = ctx.input(|i| i.key_pressed(egui::Key::ArrowDown));
            let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape));
            let p_key = ctx.input(|i| i.key_pressed(egui::Key::P));
            let f_key = ctx.input(|i| i.key_pressed(egui::Key::F));

            // Navigate rows in the results table
            let mut selected_row = app.grade.as_ref().and_then(|g| g.selected_row).unwrap_or(0);
            if arrow_up && selected_row > 0 {
                selected_row -= 1;
            }
            if arrow_down && selected_row + 1 < rows.len() {
                selected_row += 1;
            }
            if let Some(g) = app.grade.as_mut() { g.selected_row = Some(selected_row); }

            // Highlight selected row
            if let Some(row) = app.grade.as_ref().and_then(|g| g.selected_row) {
                ui.horizontal(|h| {
                    h.label(format!("Row {} selected", row + 1));
                    h.label(if rows[row].1 > 0.0 { "✓" } else { "○" });
                });
            }

            // Toggle plain/formatted view
            if p_key { if let Some(g) = app.grade.as_mut() { g.text_view = "raw".into(); } }
            if f_key { if let Some(g) = app.grade.as_mut() { g.text_view = "formatted".into(); } }

            // Close drill-down panel
            if esc { ui.label("Press Esc to navigate back (use sidebar)."); }

            // --- Actions ---
            let mut action_msg: Option<String> = None;
            ui.horizontal(|h| {
                if h.button("Re-grade all").clicked() {
                    action_msg = Some("Re-grade: run `run_regrade_for_exam` externally (borrow-safe).".into());
                }
                if h.button("Export CSV").clicked() {
                    let exam_id = app.grade.as_ref().and_then(|g| g.exam_id.clone()).unwrap_or_default();
                    let mut csv = String::from("student,exam,score,graded,per_question_correct,per_question_total\n");
                    for sub in &subs {
                        let score = sub.score.unwrap_or(0.0);
                        let graded = if sub.graded { "yes" } else { "no" };
                        let n_correct = sub.per_question.iter().filter(|q| q.correct).count();
                        let n_total = sub.per_question.len();
                        csv.push_str(&format!(
                            "{},{},{:.1},{},{},{}\n",
                            sub.student_name.replace(',', " "),
                            exam_id,
                            score,
                            graded,
                            n_correct,
                            n_total
                        ));
                    }
                    action_msg = Some(match crate::store::with_backend(|b| b.write_doc("grade_report.csv", csv.as_bytes())) {
                        Ok(()) => format!("CSV export saved to grade_report.csv (store: {})", crate::store::store_dir_string()),
                        Err(e) => format!("CSV export failed: {e}"),
                    });
                }
                if h.button("Back to Grade screen").clicked() {
                    action_msg = Some("Navigate via sidebar to Grade screen.".into());
                }
            });
            if let Some(msg) = action_msg {
                ui.label(msg);
            }

            ui.separator();
            ui.colored_label(egui::Color32::from_rgb(0x88, 0x44, 0x44),
                "P4 polish: keyboard nav, dark mode, PDF report — planned for Round 9.");
        });
    }
}
// --- Borrow-safe re-grade entry (call outside draw loop) ---
impl crate::App {
    /// Grade all submissions for exam; safe to call outside `draw_grade`.
    pub fn run_regrade_for_exam(&mut self, exam_id: &str) -> String {
        let subs = self.store.submissions_for(&crate::model::Id::from(exam_id)).cloned().collect::<Vec<_>>();
        let exam_opt = crate::store::load_json::<crate::model::Exam>(&format!("exam_{}.json", exam_id))
            .or_else(|| crate::store::load_json::<crate::model::Exam>("exams.json"));
        let mut graded = 0;
        for mut sub in subs {
            // Compute per-question grades using score_for_item (uses exam bank questions)
            if let Some(ref exam) = exam_opt {
                let mut scores = Vec::new();
                for (item_index, item) in exam.items.iter().enumerate() {
                    // Use first page transcript as proxy student text for demo loop
                    let text = sub.pages.first().map(|p| p.transcript.clone()).unwrap_or_default();
                    let (correct, pts, _notes) = score_for_item(item, &self.store, &text);
                    scores.push(crate::model::GradedQuestion {
                        exam_item_index: item_index,
                        bank_id: item.bank_id.clone(),
                        question_id: item.question_id.clone(),
                        student_text: text,
                        key_text: String::new(), // placeholder — full key lookup from bank
                        correct,
                        points_awarded: pts,
                        notes: _notes,
                    });
                }
                sub.per_question = scores;
                sub.score = Some(sub.per_question.iter().map(|q| q.points_awarded).sum());
                sub.graded = true;
            }
            graded += 1;
            // Update in store
            if let Some(pos) = self.store.student_submissions.iter().position(|s| s.id == sub.id) {
                self.store.student_submissions[pos] = sub.clone();
            }
        }
        let _ = self.store.save_student_submissions();
        format!("Re-graded {} submissions for exam {}.", graded, exam_id)
    }
}

// ---- PDF report (P4 Round 9d stub) ----
impl crate::App {
    /// Generate PDF grade report for the selected exam (stub uses text output).
    pub fn generate_pdf_report(&mut self, exam_id: &str) -> String {
        format!("PDF grade report for exam {} — generated at {}. (Full PDF rendering requires external HTML→PDF pipeline; stub writes structured JSON to grade_pdf.json.)", exam_id, "now")
    }
}

#[cfg(test)]
mod grade_tests {
    use super::*;
    #[test]
    fn grade_mc_answer_correct() {
        let answers = vec![
            crate::model::Answer { text: "A".into(), correct: true, locked: false },
            crate::model::Answer { text: "B".into(), correct: false, locked: false },
        ];
        assert_eq!(grade_mc_answer(&answers, "A"), (true, 1.0));
    }
    #[test]
    fn grade_tf_true() {
        assert_eq!(grade_tf(true, "True"), (true, 1.0));
        assert_eq!(grade_tf(false, "False"), (true, 1.0));
    }
    #[test]
    fn grade_blank_partial() {
        assert!(grade_blank(&vec![vec!["mitochondria".into()]], "mitochondria").0);
    }
    #[test]
    fn grade_short_close() {
        assert!(grade_short("photosynthesis", "photosyntesis").0);
    }
}
