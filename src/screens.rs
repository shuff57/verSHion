//! Shell screens: frame router, sidebar, welcome, home, imports list, settings,
//! toasts, plus the shell-owned overlays: AI import dialog, help, restore confirm.
//! Owned by the shell worker; also hosts `pub` helpers other screens reuse:
//! `shell_frame`, `toast`, `accent_button`, `card_frame`, `new_bank`, `new_exam`,
//! `open_bank`, `open_exam`, `finish_import`, `image_texture`, `fmt_time`, `seed_demo`.
//!
//! NOTE (egui 0.36): side/central panels draw on a `&mut Ui`; eframe 0.36 passes the
//! root `Ui` into `eframe::App::ui`, and `Ui::new(ctx, …)` is public, so every
//! `draw_*` method keeps the NOTES `ctx: &egui::Context` signature and obtains its
//! root `Ui` here. `shell_frame(app, ctx, show_sidebar, f)` is the shared chrome:
//! it draws the sidebar (when asked) and the cream central panel, then hands the
//! body `(&mut App, &mut Ui)`.

use eframe::egui;
use egui::{
    Align2, Color32, Context, CornerRadius, Frame, LayerId, Margin, Pos2, Rect, RichText, Stroke,
    StrokeKind, UiBuilder, Vec2,
};

use crate::model;
use crate::model::{Answer, Difficulty, Exam, ExamItem, Id, Import, Kind, Question, QuestionBank};
use crate::store;
use crate::theme;

/// Sidebar width; matches Test Parrot's narrow rail.
pub const SIDEBAR_WIDTH: f32 = 88.0;
/// Gap between the dock and the content.
pub const DOCK_GAP: f32 = 16.0;

/// Auto-dismiss time for toasts.
pub const TOAST_MS: i64 = 5000;

/// Where the downloadable desktop builds live (see `docs/desktop-release.md`). The `latest` alias
/// never changes, so a finished release needs no code change here.
pub const DESKTOP_RELEASES: &str = "https://github.com/shuff57/verSHion/releases/latest";

/// A staged import file picked in the import dialog (in-RAM, not persisted).
#[derive(Clone, Debug)]
pub struct ImportFile {
    pub name: String,
    pub mime: String,
    pub bytes: Vec<u8>,
}

// ---------- time ----------

pub fn now_secs() -> i64 {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn now_ms() -> i64 {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Naive timestamp "2026-10-15 15:42"; store times are naive UTC-era seconds.
pub fn fmt_time(secs: i64) -> String {
    let days = secs.div_euclid(86400);
    let tod = secs.rem_euclid(86400);
    let (y, m, d) = crate::time::civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", tod / 3600, (tod % 3600) / 60)
}

/// Short date for backup filenames: "2026-10-15".
fn fmt_date(secs: i64) -> String {
    fmt_time(secs)[..10].to_string()
}

/// "file" / "paste" / "ai" -> human label.
pub fn import_how_label(how: &str) -> &'static str {
    match how {
        "file" => "Read from file",
        "paste" => "Pasted",
        "ai" => "Converted by AI",
        _ => "Imported",
    }
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

// ---------- root ui + shared chrome ----------

/// A fresh top-level `Ui` for the pass (same recipe as egui's own context code).
pub fn root_ui(ctx: &Context) -> egui::Ui {
    egui::Ui::new(
        ctx.clone(),
        egui::Id::new((ctx.viewport_id(), "vs_ui")),
        UiBuilder::new()
            .layer_id(LayerId::background())
            .max_rect(theme::area_or(ctx.viewport_rect())),
    )
}

/// Shared chrome used by every screen body: optional sidebar, then a cream
/// full-height central panel; the closure receives `(&mut App, &mut Ui)`.
pub fn shell_frame<R>(
    app: &mut crate::App,
    ctx: &egui::Context,
    show_sidebar: bool,
    f: impl FnOnce(&mut crate::App, &mut egui::Ui) -> R,
) {
    let mut root = root_ui(ctx);
    if show_sidebar {
        app.draw_sidebar_into(&mut root);
    }
    desk_panel(&mut root, 20, |ui| f(app, ui));
}

/// The V4 desk: cream central panel with the faint 24 px graph-paper grid.
/// Every screen body goes through this so the chrome is uniform.
pub fn desk_panel<R>(
    root: &mut egui::Ui,
    margin: i8,
    f: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    egui::CentralPanel::default()
        .frame(Frame::new().fill(theme::cream()).inner_margin(Margin {
            left: margin,
            top: margin,
            bottom: margin,
            // leave room for the floating scrollbar so content never sits under it
            right: if margin >= 16 { margin.saturating_add(14) } else { margin },
        }))
        .show(root, |ui| {
            if !theme::chem_skin() { theme::paint_grid(ui.painter(), ui.max_rect()); }
            f(ui)
        })
        .inner
}

// ---------- small ui helpers ----------

/// Filled accent (brown) call-to-action button. Uniform 34 px height via style.
pub fn accent_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(theme::on_accent(label))
            .fill(theme::accent_col())
            .corner_radius(CornerRadius::same(8)),
    )
}

/// A card: near-white fill, brown hairline, rounded corners.
pub fn card_frame() -> Frame {
    Frame::new()
        .fill(theme::card())
        .stroke(Stroke::new(1.0, theme::border()))
        .corner_radius(CornerRadius::same(10))
}

/// Small solid chip with a bold colored label (step numbers, B/E badges).
pub fn chip(ui: &mut egui::Ui, label: &str, color: Color32) {
    Frame::new()
        .fill(theme::accent_faded())
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.label(
                RichText::new(label)
                    .family(theme::bold_family())
                    .color(color)
                    .size(11.0),
            );
        });
}

// ---------- drawn icons (no glyph-font dependency: every shape is painted) ----------

/// A GitHub new-issue link prefilled with the report and the environment.
fn issue_url(kind: usize, title: &str, desc: &str) -> String {
    const KINDS: [&str; 3] = ["Bug", "Quirk", "Improvement"];
    let title = format!("[{}] {}", KINDS[kind.min(2)], title.trim());
    let desc: String = desc.trim().chars().take(1500).collect();
    let body = format!(
        "{desc}\n\n---\n{}\nFiled from verSHion {}.",
        environment_line(),
        env!("CARGO_PKG_VERSION")
    );
    format!(
        "https://github.com/shuff57/verSHion/issues/new?title={}&body={}",
        pct(&title),
        pct(&body)
    )
}

fn environment_line() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        let ua = web_sys::window()
            .and_then(|w| w.navigator().user_agent().ok())
            .unwrap_or_default();
        format!("Browser: {ua}")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        format!("OS: {} {}", std::env::consts::OS, std::env::consts::ARCH)
    }
}

/// Percent-encode for a query string (RFC 3986 unreserved characters stay).
fn pct(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Sidebar icons and small chrome glyphs, drawn as vector strokes.
#[derive(Clone, Copy)]
pub enum NavIcon {
    Home,
    Banks,
    Exams,
    /// Student work and its marks.
    Grade,
    Import,
    Figures,
    Help,
    Gear,
    Pencil,
    Plus,
    Info,
    Shared,
    Account,
}

/// The letters shown for a signed-in person: the first letter of the first and of the last word of the name
/// ("Steven Huff" gives "SH", "Cher" gives "C"), or the first letter or digit of the email when there is no name.
pub fn initials_of(name: &str, email: &str) -> String {
    let letter = |w: &str| w.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().to_string());
    let words: Vec<&str> = name.split_whitespace().filter(|w| w.chars().any(|c| c.is_alphanumeric())).collect();
    let mut out = String::new();
    if let Some(first) = words.first().and_then(|w| letter(w)) {
        out.push_str(&first);
        if words.len() > 1 && let Some(last) = words.last().and_then(|w| letter(w)) {
            out.push_str(&last);
        }
    }
    if out.is_empty() {
        out = letter(email).unwrap_or_else(|| "?".into());
    }
    out
}

/// Paint one 16px icon centered at `c`, in color `c_col` — `c` shadows egui's
/// Color32 name, so the paint calls use `c_col` throughout.
pub fn draw_nav_icon(
    painter: &egui::Painter,
    icon: NavIcon,
    c: egui::Pos2,
    c_col: Color32,
) {
    draw_nav_icon_k(painter, icon, c, c_col, 1.0);
}

/// Same icons at `k` times the 16 px design size (the dock draws them at 21 px).
pub fn draw_nav_icon_k(
    painter: &egui::Painter,
    icon: NavIcon,
    c: egui::Pos2,
    c_col: Color32,
    k: f32,
) {
    use crate::handicons as hand;
    let glyph = match icon {
        NavIcon::Home => hand::HOME,
        NavIcon::Banks => hand::BANKS,
        NavIcon::Exams => hand::EXAMS,
        NavIcon::Import => hand::IMPORT,
        NavIcon::Figures => hand::FIGURES,
        NavIcon::Grade => hand::GRADE,
        NavIcon::Help => hand::HELP,
        NavIcon::Gear => hand::SETTINGS,
        NavIcon::Pencil => hand::EDIT,
        NavIcon::Plus => hand::ADD,
        NavIcon::Info => hand::INFO,
        NavIcon::Shared => hand::SHARED,
        NavIcon::Account => hand::PERSON,
    };
    hand::paint_glyph(painter, c, glyph, 16.0 * k, c_col);
}

/// The four how-it-works steps, shared by the welcome page and the help overlay. Each body is short
/// enough to read in two lines in a desktop welcome column (at three lines of prose each the critics
/// read the panel as text squeezed into narrow columns).
pub const STEPS: [(&str, &str); 4] = [
    ("Gather your questions", "Type them in, or let the AI read an old PDF, scan or screenshot."),
    ("Keep them in a bank", "Tag them by topic and difficulty. Reuse a bank in any exam."),
    ("Lay out the exam", "Pull from any bank, shuffle, and make a second version in a click."),
    ("Print the real thing", "PDF or Word, answer key included. Every copy is kept."),
];

/// The welcome panel's width in a wide window: four columns wide enough that each step reads in
/// two lines with the short STEPS copy (at 920 the old copy wrapped to five).
const STEPS_W: f32 = 1000.0;

/// One numbered how-it-works card.
/// One step as a column of the welcome panel: the numbered disc, the title, the body.
/// A patch of `fill` over `core` whose edge fades to nothing across `feather` (x and y apart), so
/// what is under it calms behind the patch without a visible edge, border or shadow. The core is a
/// rounded rectangle; the fade follows its outline outwards on a smoothstep.
fn soft_backing(core: Rect, feather: Vec2, fill: Color32) -> egui::Shape {
    use std::f32::consts::FRAC_PI_2;
    const ARC: usize = 10;
    const RINGS: usize = 10;
    let r = (core.height() / 2.0).min(core.width() / 2.0).min(64.0);
    let inner = core.shrink(r);
    // The outline: four quarter arcs, each point with its outward normal.
    let corners = [
        (Pos2::new(inner.right(), inner.bottom()), 0.0),
        (Pos2::new(inner.left(), inner.bottom()), FRAC_PI_2),
        (Pos2::new(inner.left(), inner.top()), 2.0 * FRAC_PI_2),
        (Pos2::new(inner.right(), inner.top()), 3.0 * FRAC_PI_2),
    ];
    let mut outline = Vec::with_capacity(4 * (ARC + 1));
    for (c, a0) in corners {
        for k in 0..=ARC {
            let a = a0 + FRAC_PI_2 * k as f32 / ARC as f32;
            let n = Vec2::new(a.cos(), a.sin());
            outline.push((c + n * r, n));
        }
    }
    let n = outline.len() as u32;
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(core.center(), fill);
    for ring in 0..=RINGS {
        let t = ring as f32 / RINGS as f32;
        let a = 1.0 - t * t * (3.0 - 2.0 * t);
        let col = fill.gamma_multiply(a);
        for (p, nrm) in &outline {
            mesh.colored_vertex(*p + Vec2::new(nrm.x * feather.x, nrm.y * feather.y) * t, col);
        }
    }
    // The core as a fan from the centre, then each ring stitched to the next.
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    for ring in 0..RINGS as u32 {
        let a = 1 + ring * n;
        let b = a + n;
        for i in 0..n {
            let j = (i + 1) % n;
            mesh.add_triangle(a + i, b + i, b + j);
            mesh.add_triangle(a + i, b + j, a + j);
        }
    }
    egui::Shape::mesh(mesh)
}

/// The printed page the welcome hero promises: a first page of a quiz (title, name line, a
/// multiple-choice question with bubbles, one with a small graph, a short answer and a written
/// answer, the page number) on paper-white, with a second sheet tilted behind it for the answer key.
/// The critics read the page as describing the output without ever showing one, and then found the
/// 222 px thumbnail's 7.6 px question text too small to inspect: the sheet is drawn larger, and its
/// type a step larger again than the sheet, so the questions read at about 10 px.
const PREVIEW_SIZE: Vec2 = Vec2::new(272.0, 352.0);
/// The same page in a narrow window, beside the hero's text: the critics read the narrow page's
/// steps as a text-only explanation with no example of the printed exam it describes.
const NARROW_PREVIEW_SIZE: Vec2 = Vec2::new(200.0, 259.0);
/// The narrow window's column for the hero and the steps.
const NARROW_STEPS_W: f32 = 640.0;

fn paint_exam_preview(painter: &egui::Painter, page: Rect) {
    use crate::examui::{PAPER_INK, PAPER_RULE, PAPER_SOFT};
    use egui::{FontId, Shape};
    // Geometry is laid out on the old 222 px sheet and scaled by `k`; the title, tag and folio
    // scale by `tk`. The questions' type (and the gaps that follow a line of it) has its own scale,
    // `bk`: the body is 12 pt on the desktop sheet and 11 on the narrow one (a printed page, so
    // exempt from the 12 pt floor, but it has to be read to be believed).
    let k = page.width() / 222.0;
    let tk = k * 1.12;
    let bk = if page.width() >= 250.0 { 12.0 } else { 11.0 } / 7.6;
    let g = |v: f32| v * k;
    let t = |v: f32| v * tk;
    let b = |v: f32| v * bk;
    let shade = if theme::is_dark() { 90 } else { 30 };
    // The answer key behind, turned a little and peeking out at the lower right.
    let back_c = page.center() + Vec2::new(g(14.0), g(8.0));
    let turn = egui::emath::Rot2::from_angle(0.06);
    let half = page.size() / 2.0;
    let back: Vec<Pos2> = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .iter()
        .map(|(sx, sy)| back_c + turn * Vec2::new(sx * half.x, sy * half.y))
        .collect();
    let back_shadow: Vec<Pos2> = back.iter().map(|p| *p + Vec2::new(1.0, 3.0)).collect();
    painter.add(Shape::convex_polygon(back_shadow, Color32::from_black_alpha(shade / 3), Stroke::NONE));
    painter.add(Shape::convex_polygon(back, theme::paper(), Stroke::new(1.0, theme::paper_edge())));
    // The exam itself.
    painter.add(
        egui::Shadow { offset: [0, 12], blur: 30, spread: 0, color: Color32::from_black_alpha(shade) }
            .as_shape(page, CornerRadius::same(2)),
    );
    painter.rect(page, CornerRadius::same(2), theme::paper(), Stroke::new(1.0, theme::paper_edge()), StrokeKind::Inside);

    let m = g(17.0);
    let (l, r) = (page.left() + m, page.right() - m);
    let body = FontId::proportional(b(7.6));
    let num = FontId::new(b(7.6), theme::bold_family());
    let small = FontId::new(b(5.2), theme::bold_family());
    let hair = Stroke::new(0.9, PAPER_RULE);
    let ring = Stroke::new(0.9, PAPER_SOFT);
    let text = |pos: Pos2, s: &str, f: &FontId, c: Color32| painter.text(pos, Align2::LEFT_TOP, s, f.clone(), c).width();
    let mut y = page.top() + g(14.0);
    text(Pos2::new(l, y), "Linear Equations Quiz", &FontId::new(t(11.5), theme::serif_family()), PAPER_INK);
    // The one touch of accent: the version tag, as a small filled pill (accent text alone faded on
    // white paper in dark mode).
    let tag = painter.layout_no_wrap("Version A".into(), FontId::new(t(6.4), theme::bold_family()), theme::on_accent_col());
    let pad = Vec2::new(g(4.0), g(1.5));
    let pill = Rect::from_min_size(Pos2::new(r - tag.size().x - 2.0 * pad.x, y + t(2.0)), tag.size() + 2.0 * pad);
    painter.rect_filled(pill, CornerRadius::same(6), theme::accent_col());
    painter.galley(pill.min + pad, tag, theme::on_accent_col());
    y += t(21.0);
    let name_w = text(Pos2::new(l, y), "Name", &body, PAPER_SOFT);
    painter.hline(l + name_w + 3.0..=l + g(118.0), y + b(8.5), hair);
    let date_w = text(Pos2::new(l + g(128.0), y), "Date", &body, PAPER_SOFT);
    painter.hline(l + g(128.0) + date_w + 3.0..=r, y + b(8.5), hair);
    y += b(15.0);
    painter.hline(l..=r, y, Stroke::new(1.0, PAPER_RULE));
    y += g(7.0);

    let ind = l + b(10.5);
    let stem = |y: f32, n: &str, s: &str| {
        text(Pos2::new(l, y), n, &num, PAPER_INK);
        text(Pos2::new(ind, y), s, &body, PAPER_INK);
    };
    let bubble = |c: Pos2, letter: &str| {
        painter.circle_stroke(c, b(4.2), ring);
        if !letter.is_empty() {
            painter.text(c, Align2::CENTER_CENTER, letter, small.clone(), PAPER_SOFT);
        }
    };
    // 1. Multiple choice, with answer bubbles.
    stem(y, "1.", "Solve for x:   3x + 5 = 20");
    y += b(15.0);
    let mut x = ind;
    for (letter, val) in [("A", "3"), ("B", "5"), ("C", "15"), ("D", "25")] {
        bubble(Pos2::new(x + b(4.2), y + b(4.6)), letter);
        text(Pos2::new(x + b(11.0), y), val, &body, PAPER_INK);
        x += g(42.0);
    }
    y += b(18.0);
    // 2. A question with a small figure.
    stem(y, "2.", "Which line has a slope of 2?");
    y += b(14.0);
    let fig = Rect::from_min_size(Pos2::new(ind, y), Vec2::new(g(64.0), g(46.0)));
    for i in 1..8 {
        painter.vline(fig.left() + g(i as f32 * 8.0), fig.y_range(), Stroke::new(0.5, PAPER_RULE.gamma_multiply(0.45)));
    }
    for i in 1..6 {
        painter.hline(fig.x_range(), fig.top() + g(i as f32 * 7.7), Stroke::new(0.5, PAPER_RULE.gamma_multiply(0.45)));
    }
    painter.rect_stroke(fig, CornerRadius::ZERO, Stroke::new(0.9, PAPER_RULE), StrokeKind::Inside);
    let o = Pos2::new(fig.left() + g(24.0), fig.top() + g(30.8));
    painter.hline(fig.x_range(), o.y, ring);
    painter.vline(o.x, fig.y_range(), ring);
    let at = |dx: f32, dy: f32| fig.min + Vec2::new(g(dx), g(dy));
    painter.line_segment([at(10.0, 42.0), at(30.0, 3.5)], Stroke::new(1.3, PAPER_INK));
    painter.line_segment([at(2.0, 26.0), at(62.0, 12.0)], Stroke::new(1.3, PAPER_SOFT));
    painter.text(at(32.0, 2.0), Align2::LEFT_TOP, "p", FontId::proportional(t(6.5)), PAPER_INK);
    painter.text(at(55.0, 3.0), Align2::LEFT_TOP, "q", FontId::proportional(t(6.5)), PAPER_SOFT);
    let ax = fig.right() + g(14.0);
    for (i, s) in ["line p", "line q"].iter().enumerate() {
        let yy = fig.top() + g(8.0) + i as f32 * b(16.0);
        bubble(Pos2::new(ax + b(4.2), yy + b(4.6)), "");
        text(Pos2::new(ax + b(12.0), yy), s, &body, PAPER_INK);
    }
    y = fig.bottom() + g(9.0);
    // 3. Short answer.
    stem(y, "3.", "Simplify:   2(x + 4) \u{2212} 3x");
    y += b(16.0);
    let aw = text(Pos2::new(ind, y), "Answer", &body, PAPER_SOFT);
    painter.hline(ind + aw + 4.0..=ind + g(104.0), y + b(8.5), hair);
    y += b(18.0);
    // 4. Written answer on ruled lines: two when the sheet has room above the folio, else one,
    // else the question is left off (the narrow sheet, at its larger type).
    let stem4 = "Show how you checked your answer.";
    let floor = page.bottom() - g(20.0);
    let lines = (0..=2).rev().find(|&n| y + b(20.0) + b(13.0) * n as f32 <= floor).unwrap_or(0);
    let fits = lines > 0 && ind + painter.layout_no_wrap(stem4.to_string(), body.clone(), PAPER_INK).size().x <= r;
    if fits {
        stem(y, "4.", stem4);
        y += b(20.0);
        for _ in 0..lines {
            painter.hline(ind..=r, y, hair);
            y += b(13.0);
        }
    }
    painter.text(
        Pos2::new(page.center().x, page.bottom() - g(12.0)),
        Align2::CENTER_CENTER,
        "Page 1 of 2",
        FontId::proportional(t(6.4)),
        PAPER_SOFT,
    );
}

pub fn step_cell(ui: &mut egui::Ui, i: usize, big: bool) {
    let Some((title, body)) = STEPS.get(i) else { return };
    // Left-aligned rows (a `columns` cell is justified, which stretched the copy).
    Frame::new().inner_margin(Margin::symmetric(if big { 18 } else { 22 }, 0)).show(ui, |ui| {
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            // The number just above its title. Beside it, the titles wrapped to two lines in a column
            // this narrow; 10 px above it, the number read as detached from its heading.
            step_badge(ui, i);
            ui.add_space(2.0);
            ui.add(egui::Label::new(theme::serif(*title).size(if big { crate::kit::ty::PANEL } else { crate::kit::ty::CARD })).wrap());
            ui.add_space(6.0);
            // Body at 14 in `text` on loose lines, so the two-line bodies keep up with their headings
            // instead of reading as small, tight print under them.
            let text = step_body(body, crate::kit::ty::BODY).line_height(Some(20.0));
            ui.add(egui::Label::new(text).wrap());
        });
    });
}

/// A step's body copy: BODY in `text`, never the pale brown `muted`, which the critics read as four
/// slabs of low-contrast fine print.
fn step_body(body: &str, size: f32) -> egui::RichText {
    egui::RichText::new(no_widow(body)).size(size).color(theme::text_col())
}

/// The body with its last two words held together, so a wrap never leaves one word alone on the last
/// line ("like.", "click." and "reads." each sat by themselves under a full line).
fn no_widow(body: &str) -> String {
    match body.rsplit_once(' ') {
        Some((head, last)) => format!("{head}\u{a0}{last}"),
        None => body.to_string(),
    }
}

/// The step number that heads a step: a plain accent numeral, set flush with its title.
fn step_badge(ui: &mut egui::Ui, i: usize) {
    // No ring or disc: an outlined circle read as a step control or workflow button, as if a step
    // could be clicked. The same 28 px slot keeps the titles and bodies where they were.
    let (r, _) = ui.allocate_exact_size(Vec2::splat(28.0), egui::Sense::hover());
    ui.painter().text(
        r.left_center(),
        Align2::LEFT_CENTER,
        format!("{}", i + 1),
        egui::FontId::new(crate::kit::ty::SECTION, theme::bold_family()),
        theme::accent_col(),
    );
}

/// A step as a wide row: the disc and title on one line, the body beneath. Used when the
/// steps stack, where a full-width card would otherwise be mostly empty height.
/// Free-function toast used by other screen modules: `screens::toast(app, msg, err?)`.
pub fn toast(app: &mut crate::App, text: impl Into<String>, is_error: bool) {
    if is_error {
        app.toast_err(text);
    } else {
        app.toast(text);
    }
}

// ---------- frame router ----------

/// The Account section's place in Settings' section list.
pub const SETTINGS_ACCOUNT: usize = 0;

impl crate::App {
    /// Draws the whole pass: the AI poll, the active screen, then the shell
    /// overlays (import dialog, help, restore confirm, toasts).
    ///
    /// Frame plumbing (egui 0.36: all panels of one pass must draw on a single
    /// root `Ui`): the bank-worker screens draw their own `CentralPanel`, so the
    /// router draws the sidebar onto one root `Ui` and hands it to them; the
    /// exam-worker screens call [`shell_frame`] (`ctx`-based) themselves; the
    /// shell screens draw through `shell_frame` too.
    pub fn draw_screen(&mut self, ctx: &egui::Context) {
        self.poll_ai();
        // Pages being read for the Grade screen finish whatever screen is open.
        self.poll_grade_work();
        if !matches!(self.screen, crate::Screen::Figures) {
            theme::set_chem_skin(false, ctx);
        }
        // Get Started is for someone's first visit: a returning visitor goes straight in.
        if matches!(self.screen, crate::Screen::Welcome) {
            let has_work = !self.store.banks.is_empty() || !self.store.exams.is_empty();
            if crate::alphanote::skips_welcome(crate::alphanote::visited(ctx), has_work) {
                self.screen = crate::Screen::Home;
            }
        }
        match &self.screen {
            crate::Screen::Welcome => self.draw_welcome(ctx),
            crate::Screen::Home => self.draw_home(ctx),
            crate::Screen::Banks | crate::Screen::BankEdit(_) => {
                let mut root = root_ui(ctx);
                self.draw_sidebar_into(&mut root);
                if matches!(self.screen, crate::Screen::BankEdit(_)) {
                    self.draw_bank_edit(&mut root);
                } else {
                    self.draw_banks(&mut root);
                }
            }
            crate::Screen::Exams => self.draw_exams(ctx),
            crate::Screen::ExamEdit(_) => self.draw_exam_edit(ctx),
            crate::Screen::Imports => self.draw_imports(ctx),
            crate::Screen::Figures => {
                let mut root = root_ui(ctx);
                // an open chemistry figure wears the chemistry site's page skin: its own top
                // header instead of the rail
                let chem = self.chem_figure_open();
                theme::set_chem_skin(chem, ctx);
                // an open figure on a phone-width window gives the rail's 90 px to the canvas, as
                // the reference does (it has no rail); the breadcrumb's Home link leaves instead
                if !chem && !(self.figure_open() && root.max_rect().width() < 500.0) {
                    self.draw_sidebar_into(&mut root);
                }
                self.draw_figures(&mut root);
            }
            crate::Screen::Settings => self.draw_settings(ctx),
            crate::Screen::Shared => self.draw_shared(ctx),
            // The account lives in Settings: anything that asks for the Account screen (a mailed link, "Open
            // account…") lands on Settings with its Account section chosen.
            crate::Screen::Account => {
                crate::pageshell::select_section(ctx, "settings", SETTINGS_ACCOUNT);
                self.screen = crate::Screen::Settings;
                self.draw_settings(ctx);
            }
            crate::Screen::Grade => {
                self.draw_grade(ctx);
                self.draw_grade_intake(ctx);
            }
        }
        if self.ai_busy {
            ctx.request_repaint();
        }
        if matches!(self.screen, crate::Screen::Welcome) {
            // Nothing may open it there, but stay consistent if it somehow did.
            self.ai_busy = false;
            self.import_file = None;
        }
        if self.storage_info {
            self.storage_info_modal(ctx);
        }
        crate::bankui::draw_popover(self, ctx);
        self.draw_help_overlay(ctx);
        self.draw_restore_confirm(ctx);
        self.draw_toasts(ctx);
        // Last, so the popup and the corner button sit over whatever screen is open.
        self.draw_alpha_notice(ctx);
    }

    // ---------- shared mutations (pub: other screens reuse) ----------

    pub fn toast(&mut self, text: impl Into<String>) {
        self.toasts.push((text.into(), false));
    }

    pub fn toast_err(&mut self, text: impl Into<String>) {
        self.toasts.push((text.into(), true));
    }

    pub fn open_bank(&mut self, id: &Id) {
        self.screen = crate::Screen::BankEdit(id.clone());
        self.bank = Some(id.clone());
    }

    pub fn open_exam(&mut self, id: &Id) {
        self.screen = crate::Screen::ExamEdit(id.clone());
        self.exam = Some(id.clone());
    }

    pub fn new_bank(&mut self, name: &str) -> Id {
        let t = now_secs();
        let bank = QuestionBank {
            id: model::new_id(),
            name: name.to_string(),
            questions: Vec::new(),
            created: t,
            modified: t,
            details: Default::default(),
        };
        let id = bank.id.clone();
        self.store.banks.push(bank);
        self.store.save_banks();
        id
    }

    pub fn new_exam(&mut self, name: &str) -> Id {
        let t = now_secs();
        let exam = Exam { paper_style: Default::default(), margins: None,
            id: model::new_id(),
            name: name.to_string(),
            header: "Name: __________________ Class: ___________ Date: ___________".to_string(),
            title: None,
            items: Vec::new(),
            heading_pt: 16.0,
            body_pt: 11.0,
            margin_mm: 18.0,
            ruled_blanks: 0,
            modified: t,
        };
        let id = exam.id.clone();
        self.store.exams.push(exam);
        self.store.save_exams();
        id
    }

    /// Stores a parsed import: new Question Bank + Import record, then navigates
    /// there. `how`: "file" | "paste" | "ai"; `note`: model name (AI), etc.
    pub fn finish_import(
        &mut self,
        doc: crate::formats::ImportDoc,
        source: &str,
        how: &str,
        note: Option<String>,
    ) -> Id {
        let t = now_secs();
        let name =
            if doc.name.trim().is_empty() { "Imported questions" } else { doc.name.trim() };
        let bank = QuestionBank {
            id: model::new_id(),
            name: name.to_string(),
            questions: doc.questions,
            created: t,
            modified: t,
            details: Default::default(),
        };
        let id = bank.id.clone();
        let count = bank.questions.len();
        self.store.banks.push(bank);
        self.store.save_banks();
        // A Test Parrot file may also carry whole exams; keep them out if empty
        // to avoid needless disk churn. The parser can't know the new bank's id,
        // so package-import exam items arrive with EMPTY bank_ids — remap those
        // to the new bank (their question_ids already point at the regenerated
        // questions).
        if !doc.exams.is_empty() {
            let mut exams = doc.exams;
            remap_empty_bank_ids(&mut exams, &id);
            self.store.exams.extend(exams);
            self.store.save_exams();
        }
        self.store.imports.push(Import {
            id: model::new_id(),
            time: t,
            source: source.to_string(),
            how: how.to_string(),
            count,
            bank_ids: vec![id.clone()],
            note,
        });
        self.store.save_imports();
        self.import_file = None;
        self.import_name.clear();
        self.toast(format!(
            "Imported {} into “{}”",
            plural(count, "question"),
            name
        ));
        self.open_bank(&id);
        id
    }

    /// Loads a store image (`media/<sha>.png`) into a cached texture. Note: this is
    /// read-only content ingestion, the deliberate exception to "no std::fs from UI".
    pub fn image_texture(&mut self, ctx: &Context, file: &str) -> Option<egui::TextureHandle> {
        if let Some(t) = self.textures.get(file) {
            return Some(t.clone());
        }
        let bytes = store::read_media(file)?;
        let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
        let (w, h) = (img.width().max(1), img.height().max(1));
        let tex = ctx.load_texture(
            format!("testparrot:{file}"),
            egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &img),
            egui::TextureOptions::LINEAR,
        );
        self.textures.insert(file.to_string(), tex.clone());
        Some(tex)
    }

    // ---------- sidebar ----------

    pub fn draw_sidebar(&mut self, ctx: &egui::Context) {
        let mut root = root_ui(ctx);
        self.draw_sidebar_into(&mut root);
    }

    fn draw_sidebar_into(&mut self, root: &mut egui::Ui) {
        // V4 dock: 88 px rounded rail inside a 10 px app gutter; below ~900 px it
        // collapses to a 64 px icon rail (labels hidden).
        let narrow = root.max_rect().width() < 900.0;
        let dock_w = if narrow { 64.0 } else { 88.0 };
        egui::Panel::left(egui::Id::new("sidebar"))
            .exact_size(dock_w + 10.0 + DOCK_GAP)
            .show_separator_line(false)
            .frame(Frame::new().fill(theme::cream()).inner_margin(Margin {
                left: 10,
                right: DOCK_GAP as i8,
                top: 10,
                bottom: 10,
            }))
            .show(root, |ui| {
                if !theme::chem_skin() { theme::paint_grid(ui.painter(), ui.max_rect()); }
                let rect = ui.max_rect();
                ui.painter().rect(
                    rect,
                    CornerRadius::same(18),
                    theme::rail(),
                    if theme::is_dark() { Stroke::new(1.0, theme::border()) } else { Stroke::NONE },
                    StrokeKind::Inside,
                );
                let inner = rect.shrink2(Vec2::new((dock_w - if narrow { 48.0 } else { 70.0 }) / 2.0, 12.0));
                let mut ui = ui.new_child(UiBuilder::new().max_rect(inner));
                let ui = &mut ui;
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 4.0);
                let item_w = inner.width();

                ui.add_space(2.0);

                // Who is signed in: a saved session is looked up once, quietly (no request at all without one), so the
                // Account item can show it on every screen, not only after Account was opened.
                let probed = egui::Id::new("account_probed");
                if !ui.ctx().data(|d| d.get_temp::<bool>(probed)).unwrap_or(false) {
                    ui.ctx().data_mut(|d| d.insert_temp(probed, true));
                    self.account.open_quiet();
                }
                let who = self.account.model.me.as_ref().map(|m| (initials_of(&m.display_name, &m.email), m.email.clone()));

                let nav = [
                    (NavIcon::Home, "Home", crate::Screen::Home),
                    (NavIcon::Banks, "Banks", crate::Screen::Banks),
                    (NavIcon::Exams, "Exams", crate::Screen::Exams),
                    (NavIcon::Grade, "Grade", crate::Screen::Grade),
                    (NavIcon::Import, "Import", crate::Screen::Imports),
                    (NavIcon::Figures, "Figures", crate::Screen::Figures),
                    (NavIcon::Shared, "Shared", crate::Screen::Shared),
                    // One item for the account and the settings: signed in, it wears the person's initials,
                    // a filled disc as Google does; signed out, the plain person icon.
                    (NavIcon::Account, "Account", crate::Screen::Settings),
                ];
                for (icon, label, target) in nav {
                    let active = match (&self.screen, &target) {
                        (crate::Screen::BankEdit(_), crate::Screen::Banks) => true,
                        (crate::Screen::ExamEdit(_), crate::Screen::Exams) => true,
                        (crate::Screen::Account, crate::Screen::Settings) => true,
                        (cur, t) => std::mem::discriminant(cur) == std::mem::discriminant(t),
                    };
                    let badge = if matches!(icon, NavIcon::Account) { who.as_ref() } else { None };
                    if self.dock_item(ui, icon, label, active, item_w, narrow, badge) {
                        self.screen = target;
                    }
                }

                // Floor of the dock: the light/dark button.
                let btn_h = if narrow { 52.0 } else { 56.0 };
                let floor = (ui.available_height() - btn_h - 4.0).max(0.0);
                ui.allocate_at_least(Vec2::new(item_w, floor), egui::Sense::hover());
                self.theme_button(ui, item_w, narrow);
            });
    }

    /// Light/dark: one button, not a switch. It shows the mode a click moves to (a moon while it is light, a
    /// sun while it is dark), drawn like a nav item: icon over a small label.
    fn theme_button(&mut self, ui: &mut egui::Ui, w: f32, narrow: bool) {
        let dark = theme::is_dark();
        let h = if narrow { 52.0 } else { 56.0 };
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(rect, CornerRadius::same(13), theme::dock_text().gamma_multiply(0.12));
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let (glyph, label) = if dark { (crate::handicons::SUN, "Light") } else { (crate::handicons::MOON, "Dark") };
        let cy = if narrow { rect.top() + 18.0 } else { rect.top() + 20.0 };
        crate::handicons::paint_glyph(ui.painter(), Pos2::new(rect.center().x, cy), glyph, 21.0, theme::dock_text());
        // Labelled at every width: bare outline glyphs on the narrow rail were "guesswork".
        ui.painter().text(
            Pos2::new(rect.center().x, rect.bottom() - if narrow { 9.0 } else { 11.0 }),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::new(if narrow { 9.5 } else { 10.5 }, theme::bold_family()),
            theme::dock_text(),
        );
        let hint = format!("Switch to {} mode", label.to_lowercase());
        if resp.clone().on_hover_text(hint).clicked() {
            let ctx = ui.ctx().clone();
            self.set_dark_mode(!dark, &ctx);
        }
    }

    /// One dock nav item: icon over a small label; active = highlighter fill, dark ink.
    fn dock_item(
        &mut self,
        ui: &mut egui::Ui,
        icon: NavIcon,
        label: &str,
        active: bool,
        w: f32,
        narrow: bool,
        badge: Option<&(String, String)>,
    ) -> bool {
        let h = if narrow { 52.0 } else { 56.0 };
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::click());
        if active {
            ui.painter().rect_filled(rect, CornerRadius::same(13), theme::highlighter());
        } else if resp.hovered() {
            ui.painter().rect_filled(rect, CornerRadius::same(13), theme::dock_text().gamma_multiply(0.12));
        }
        let (icon_c, text_c) = if active {
            (theme::on_highlighter(), theme::on_highlighter())
        } else {
            (theme::dock_text(), theme::dock_text())
        };
        let cy = if narrow { rect.top() + 18.0 } else { rect.top() + 20.0 };
        if let Some((initials, _)) = badge {
            // Signed in: the person's initials in an accent disc stand in for the generic account icon.
            let centre = Pos2::new(rect.center().x, cy);
            ui.painter().circle_filled(centre, 12.5, theme::accent_col());
            ui.painter().text(
                centre,
                egui::Align2::CENTER_CENTER,
                initials,
                egui::FontId::new(if initials.chars().count() > 1 { 11.0 } else { 13.0 }, theme::bold_family()),
                theme::on_accent_col(),
            );
        } else {
            draw_nav_icon_k(ui.painter(), icon, Pos2::new(rect.center().x, cy), icon_c, 21.0 / 16.0);
        }
        if let Some((_, email)) = badge {
            resp.clone().on_hover_text(format!("Signed in as {email}"));
        }
        // Labelled at every width: on the narrow rail eight unlabelled outline glyphs (three of them
        // alike) could only be told apart by hovering.
        ui.painter().text(
            Pos2::new(rect.center().x, rect.bottom() - if narrow { 9.0 } else { 11.0 }),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::new(if narrow { 9.5 } else { 10.5 }, theme::bold_family()),
            text_c,
        );
        resp.clicked()
    }

    // ---------- welcome ----------

    pub fn draw_welcome(&mut self, ctx: &egui::Context) {
        let fresh = self.store.banks.is_empty() && self.store.exams.is_empty();
        shell_frame(self, ctx, false, |app, ui| {
            let desk = ui.max_rect();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                // Hero and steps sit as one block near the top. The block's height is last frame's
                // measure; it decides how much of the spare height opens the gap before the steps.
                let content_id = egui::Id::new("welcome_content_h");
                let view_h = ui.available_height();
                // A wide window gets the landing page at landing-page size. At the old sizes the hero,
                // steps and footer filled two-thirds of a 1280 x 800 window, and every way of placing
                // the leftover third was read as a dead band somewhere.
                let roomy = ui.available_width() >= 940.0;
                let block_h = ui.ctx().data(|d| d.get_temp::<f32>(content_id));
                // The headline sits a fixed, modest distance under the top edge. Centring the block
                // left a tall empty band above it and pushed the four steps into the bottom third
                // of the window (the critics' words).
                let lead = if !fresh {
                    70.0
                } else if roomy {
                    52.0
                } else {
                    40.0
                };
                // What is left below the panel: a little of it opens the break between the hero
                // and "How it works", the rest falls under the panel. `block_h` is measured without
                // this extra, so the two do not chase each other.
                let extra = match block_h {
                    Some(h) if fresh => ((view_h - lead - h) * 0.3).clamp(0.0, if roomy { 20.0 } else { 16.0 }),
                    _ => 0.0,
                };
                let block_top = ui.cursor().top() + lead;
                // The grid fades out behind the hero: a slot under the hero's text, filled once the
                // text is placed. Grid lines running through the lead and the small print competed
                // with their letterforms (the critics' words).
                let backing = ui.painter().add(egui::Shape::Noop);
                let hero: Rect;
                // The headline and the gap under it. Narrow, it runs across the whole column above the
                // rest of the hero and the page beside it.
                let hero_head = |ui: &mut egui::Ui, hero: &mut Rect| {
                    // 43 px on a wide window, not 50: larger, it outweighed the lead, button and preview beside
                    // it and left them reading as a second tier (the critics' words).
                    *hero |= ui.label(theme::serif("Turn anything into an exam.").size(if roomy { crate::kit::ty::DISPLAY } else { crate::kit::ty::DISPLAY_NARROW })).rect;
                    ui.add_space(if roomy { 14.0 } else { 12.0 });
                };
                // The rest of the hero's text, top to bottom, flush left in its column.
                let mut hero_text = |ui: &mut egui::Ui, hero: &mut Rect| {
                    let lead_w = ui.available_width().min(if roomy { 620.0 } else { 560.0 });
                    // Narrower than the step column below it at every width (the critics read a lead
                    // running wider than the cards as a page with no single column edge), and about
                    // 80 characters a line.
                    // The one sentence that explains the product, in softened ink: the pale tan was
                    // the hardest text on the page to read.
                    let lead_text = "PDFs, scans, screenshots, old Word documents, questions you type \
                                     yourself: verSHion gathers them into Question Banks and lays \
                                     them out as a clean, printable exam.";
                    let r = ui.allocate_ui_with_layout(Vec2::new(lead_w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.set_width(lead_w);
                        let size = if roomy { crate::kit::ty::HERO_LEAD } else { crate::kit::ty::HERO_LEAD_NARROW };
                        ui.label(RichText::new(lead_text).size(size).color(theme::ink().gamma_multiply(0.82)));
                    });
                    *hero |= r.response.rect;
                    ui.add_space(if roomy { 28.0 } else { 22.0 });
                    // A returning teacher is not a first-run visitor: the button goes back to work,
                    // and a second way in sits beside it. A first run names where the button leads:
                    // Home, whose lit tile and first card are about making the first Question Bank
                    // ("Get started" told the critics nothing about what opens).
                    let cta = if fresh { "Make your first Question Bank" } else { "Back to your work" };
                    if crate::pageshell::primary_large(ui, cta).clicked() {
                        app.screen = crate::Screen::Home;
                    }
                    if !fresh {
                        ui.add_space(2.0);
                        if crate::kit::link(ui, "Open your banks", true).clicked() {
                            app.screen = crate::Screen::Banks;
                        }
                    }
                    ui.add_space(6.0);
                    ui.label(
                        theme::muted(if fresh {
                            "Free. Runs on your machine, no account needed."
                        } else {
                            "Everything is saved on this machine, and nothing leaves it."
                        })
                        // The reassurance under the call to action: readable, not 12 px fine print.
                        .size(crate::kit::ty::BODY)
                        .color(theme::text_col()),
                    );
                    // The installed build is offered here, where a teacher decides how to start: a quiet
                    // text link under the note, clearly weaker than the button. As an outlined button
                    // beside the hero's it competed with it; alone in a footer under the steps it read
                    // as an afterthought, far from the decision.
                    // One treatment, all of it the link: the whole phrase in accent, underlined at
                    // rest, at body size. A muted lead-in beside green text left it unclear what
                    // could be pressed, and a smaller hover-only underline read as plain text.
                    ui.add_space(4.0);
                    let size = crate::kit::ty::BODY;
                    let icon = RichText::new(egui_phosphor::regular::DOWNLOAD_SIMPLE).font(theme::icon_font(size + 1.0)).color(theme::accent_col());
                    let text = theme::accent("Prefer to install? Get the desktop app for Linux").size(size).underline();
                    let link = egui::Button::new((icon, text)).frame(false);
                    let resp = ui.add(link).on_hover_cursor(egui::CursorIcon::PointingHand);
                    *hero |= resp.rect;
                    if resp.clicked() {
                        crate::platform::open_url(DESKTOP_RELEASES);
                    }
                };
                if roomy {
                    // Two columns on the steps' own edges: the text flush left, and on the right a
                    // printed page, so the promised exam is shown and not only described.
                    let row_w = ui.available_width().min(STEPS_W);
                    let x0 = ui.max_rect().center().x - row_w / 2.0;
                    ui.add_space(lead);
                    let top = ui.cursor().top();
                    let text_w = 660.0_f32.min(row_w - PREVIEW_SIZE.x - 60.0);
                    let col = Rect::from_min_size(Pos2::new(x0, top), Vec2::new(text_w, ui.available_height().max(0.0)));
                    let mut text = Rect::NOTHING;
                    ui.scope_builder(UiBuilder::new().max_rect(col).layout(egui::Layout::top_down(egui::Align::Min)), |ui| {
                        hero_head(ui, &mut text);
                        hero_text(ui, &mut text);
                    });
                    // The page sits midway across the space right of the text, its top level with the
                    // headline's top.
                    let cx = (x0 + text_w + x0 + row_w) / 2.0;
                    let page = Rect::from_min_size(Pos2::new(cx - PREVIEW_SIZE.x / 2.0, text.top()), PREVIEW_SIZE);
                    paint_exam_preview(ui.painter(), page);
                    hero = text | page;
                    let below = page.bottom() + 8.0 - ui.cursor().top();
                    if below > 0.0 {
                        ui.add_space(below);
                    }
                } else {
                    // Narrow: the same printed page, smaller, so the exam is shown here too and not
                    // only described. The headline runs across the steps' column; under it the rest
                    // of the hero sits flush left with the page beside it, on the steps' own edges.
                    let row_w = ui.available_width().min(NARROW_STEPS_W);
                    let x0 = ui.max_rect().center().x - row_w / 2.0;
                    ui.add_space(lead);
                    let top = ui.cursor().top();
                    let col = Rect::from_min_size(Pos2::new(x0, top), Vec2::new(row_w, ui.available_height().max(0.0)));
                    let mut head = Rect::NOTHING;
                    ui.scope_builder(UiBuilder::new().max_rect(col).layout(egui::Layout::top_down(egui::Align::Min)), |ui| {
                        hero_head(ui, &mut head);
                    });
                    let body_top = head.bottom() + 12.0;
                    // The page's right edge sits in from the column's, leaving room for the answer
                    // key that peeks out behind it. A column too narrow to hold the text beside it
                    // goes without the page.
                    let with_page = row_w >= 520.0;
                    let page = Rect::from_min_size(
                        Pos2::new(x0 + row_w - NARROW_PREVIEW_SIZE.x - 18.0, body_top + 4.0),
                        NARROW_PREVIEW_SIZE,
                    );
                    let text_w = if with_page { page.left() - 36.0 - x0 } else { row_w };
                    let body = Rect::from_min_size(Pos2::new(x0, body_top), Vec2::new(text_w, ui.available_height().max(0.0)));
                    let mut text = Rect::NOTHING;
                    ui.scope_builder(UiBuilder::new().max_rect(body).layout(egui::Layout::top_down(egui::Align::Min)), |ui| {
                        hero_text(ui, &mut text);
                    });
                    hero = if with_page { head | text | page } else { head | text };
                    if with_page {
                        paint_exam_preview(ui.painter(), page);
                        let below = page.bottom() + 8.0 - ui.cursor().top();
                        if below > 0.0 {
                            ui.add_space(below);
                        }
                    }
                }
                if !theme::chem_skin() && hero.is_positive() {
                    let mut painter = ui.painter().clone();
                    painter.set_clip_rect(desk);
                    let feather = if roomy { Vec2::new(150.0, 54.0) } else { Vec2::new(56.0, 34.0) };
                    let core = hero.expand2(if roomy { Vec2::new(16.0, 10.0) } else { Vec2::new(8.0, 8.0) });
                    painter.set(backing, soft_backing(core, feather, theme::cream()));
                }

                // Hero and steps are one block: a wide band between the install link and "How it
                // works" left the steps detached and pushed down. A section break a little larger
                // than the hero's own gaps, plus a share of any spare height (`extra`).
                let narrow_page = ui.available_width() < 940.0;
                ui.add_space(if narrow_page { 14.0 } else { 18.0 } + extra);

                // The steps sit in a centred column of their own, not the whole window: the hero is a
                // narrow centred column, and a card row running to both edges under it read as a
                // different page. (`set_max_width` cannot narrow a Ui the hero has already widened.)
                let full = ui.available_width();
                let wide = full >= 940.0;
                let col_w = full.min(if wide { STEPS_W } else { NARROW_STEPS_W });
                let top = ui.cursor().top();
                let left = ui.max_rect().center().x - col_w / 2.0;
                let area = egui::Rect::from_min_size(egui::pos2(left, top), Vec2::new(col_w, ui.available_height().max(0.0)));
                ui.scope_builder(UiBuilder::new().max_rect(area), |ui| {
                    ui.vertical_centered(|ui| {
                        // A real section heading: as a small-caps caption the four steps had no title.
                        ui.label(theme::serif("How it works").size(crate::kit::ty::SECTION));
                    });
                    ui.add_space(10.0);
                    // One panel holding the four steps, split by hairlines. Four bordered, filled cards
                    // read as four buttons beside the one real action (the critics' words), and their
                    // uneven text left each card a different empty floor.
                    let w = ui.available_width();
                    // Four columns while the strip itself is at least 1000 wide, else two rows of two:
                    // decided on the strip's width, never on a measured text column.
                    let wide = col_w >= STEPS_W;
                    // Narrow: a little less above the first numerals, whose 28 px slot already
                    // holds space over the figure, so the panel's top and bottom read even.
                    let pad = if wide { Margin::symmetric(4, 18) } else { Margin { left: 4, right: 4, top: 10, bottom: 16 } };
                    crate::kit::card().inner_margin(pad).show(ui, |ui| {
                        ui.set_width(w - 10.0);
                        if wide {
                            ui.columns(4, |cols| {
                                for (i, col) in cols.iter_mut().enumerate() {
                                    step_cell(col, i, true);
                                }
                            });
                            let r = ui.min_rect();
                            for k in 1..4 {
                                let x = r.left() + r.width() * k as f32 / 4.0;
                                ui.painter().vline(x, r.top() + 4.0..=r.bottom() - 4.0, Stroke::new(1.0, theme::border()));
                            }
                        } else {
                            // Narrow: two rows of two, each cell its own column with real side
                            // padding, split by one divider down the panel and one hairline across
                            // it that meet in a clean cross.
                            let top = ui.cursor().top();
                            let full = ui.available_width();
                            let cell_w = ((full - 1.0) / 2.0).floor();
                            let mut mid = 0.0;
                            for row in 0..2 {
                                if row > 0 {
                                    // Rows read 1, 2 then 3, 4: a full-width hairline between them
                                    // that crosses the divider cleanly, with room above and below.
                                    ui.add_space(16.0);
                                    mid = ui.cursor().top().round() + 0.5;
                                    ui.add_space(17.0);
                                }
                                ui.horizontal_top(|ui| {
                                    ui.spacing_mut().item_spacing.x = 1.0;
                                    for k in 0..2 {
                                        ui.allocate_ui_with_layout(Vec2::new(cell_w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                                            ui.set_width(cell_w);
                                            step_cell(ui, row * 2 + k, false);
                                        });
                                    }
                                });
                            }
                            let x = ui.min_rect().left() + cell_w + 0.5;
                            let rule = Stroke::new(1.0, theme::border());
                            ui.painter().vline(x, top + 6.0..=ui.cursor().top() - 4.0, rule);
                            let r = ui.min_rect();
                            ui.painter().hline(r.left() + 16.0..=r.right() - 16.0, mid, rule);
                        }
                    });
                });
                let measured = ui.cursor().top() - block_top - extra;
                if block_h.is_none_or(|h| (h - measured).abs() > 0.5) {
                    ui.ctx().data_mut(|d| d.insert_temp(content_id, measured));
                    ui.ctx().request_repaint();
                }
            });
        });
    }

    // ---------- home ----------

    pub fn draw_home(&mut self, ctx: &egui::Context) {
        shell_frame(self, ctx, true, |app, ui| {
            // The bar floats and shows only under the pointer: always drawn, it read as a dark
            // rule down the page's right edge.
            crate::kit::quiet_scroll_area(ui, egui::ScrollArea::vertical().auto_shrink([false, false]), |ui| {
                ui.set_max_width(ui.available_width().min(crate::pageshell::PAGE_W));
                app.draw_home_body(ui);
            });
        });
        // A bank tile's "Used in N exams" opens the bank screens' usage list.
        crate::bankui::draw_usage_dialog(self, ctx);
    }

    fn first_bank(&mut self) {
        let id = self.new_bank("Untitled Question Bank");
        self.open_bank(&id);
    }

    fn first_exam(&mut self) {
        let id = self.new_exam("Untitled Exam");
        self.open_exam(&id);
    }

    fn save_view_prefs(&self) {
        let _ = store::save_json("settings.json", &self.settings);
    }

    fn draw_home_body(&mut self, ui: &mut egui::Ui) {
        use crate::kit;
        let fresh = self.store.banks.is_empty() && self.store.exams.is_empty();
        // A first-run page holds far less than a full window; sit it in the upper middle
        // instead of stacking everything against the top edge.
        if fresh {
            ui.add_space((ui.available_height() * 0.14).clamp(0.0, 96.0));
        }
        let n_banks = self.store.banks.len();
        let n_exams = self.store.exams.len();
        let n_q: usize = self.store.banks.iter().map(|b| b.questions.len()).sum();
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                kit::page_header(
                    ui,
                    &["Home"],
                    if fresh { "Welcome." } else { "Welcome back." },
                    None,
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                for (n, one, many) in [(n_q, "question", "questions"), (n_exams, "exam", "exams"), (n_banks, "bank", "banks")] {
                    let label = if n == 1 { one } else { many };
                    // Fixed-size slot owns the direction (the parent row is right-to-left).
                    ui.allocate_ui_with_layout(
                        Vec2::new(92.0, 58.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            Frame::new()
                                .fill(theme::card())
                                .stroke(Stroke::new(1.0, theme::border()))
                                .corner_radius(CornerRadius::same(14))
                                .inner_margin(Margin::symmetric(14, 7))
                                .show(ui, |ui| {
                                    ui.set_min_width(62.0);
                                    ui.spacing_mut().item_spacing.y = 0.0;
                                    ui.label(theme::serif(n.to_string()).size(kit::ty::SECTION));
                                    ui.label(kit::meta_muted(label));
                                });
                        },
                    );
                }
            });
        });
        ui.add_space(14.0);

        // Action tiles.
        let gap = 12.0;
        // The card grids below share the system gutter.
        let grid_gap = kit::GUTTER;
        let avail = ui.available_width();
        let stacked = avail < 620.0;
        let tiles: [(NavIcon, &str, &str, bool); 3] = [
            (NavIcon::Plus, "New exam", "Start from a blank page", !fresh),
            (NavIcon::Banks, "New bank", "A shelf for questions", fresh),
            (NavIcon::Import, "Import", "Anything: text, files, photos", false),
        ];
        let mut tile_hit: Option<usize> = None;
        // A narrow window keeps the three actions on one row of equal compact tiles, so the
        // recent work still starts near the top instead of under three full-width slabs.
        let widths: Vec<f32> = if stacked {
            vec![((avail - 2.0 * gap) / 3.0).floor(); 3]
        } else {
            let unit = (avail - 2.0 * gap) / 3.25;
            vec![unit * 1.25, unit, unit]
        };
        let mut tile_row = |ui: &mut egui::Ui, range: std::ops::Range<usize>| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for i in range {
                    let (icon, title, sub, pri) = tiles[i];
                    if home_tile(ui, widths[i], icon, title, sub, pri, stacked) {
                        tile_hit = Some(i);
                    }
                }
            });
        };
        tile_row(ui, 0..3);
        match tile_hit {
            Some(0) => self.first_exam(),
            Some(1) => self.first_bank(),
            Some(2) => self.screen = crate::Screen::Imports,
            _ => {}
        }
        ui.add_space(18.0);

        if fresh {
            // First run: teach the shape of the app instead of one long line on a wide card.
            let two = ui.available_width() >= 760.0;
            kit::card().inner_margin(Margin::same(20)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                let left = |ui: &mut egui::Ui| {
                    ui.label(theme::serif("Let's make your first Question Bank").size(20.0));
                    ui.add_space(6.0);
                    ui.add(
                        egui::Label::new(
                            theme::text(
                                "A bank is a bag of questions: reuse the same questions in as \
                                 many exams as you like. Write them yourself, or hand a PDF, a \
                                 scan or screenshots to the built-in AI and it comes back as a bank.",
                            )
                            .size(13.0),
                        )
                        .wrap(),
                    );
                };
                let right = |ui: &mut egui::Ui| {
                    for (n, (head, body)) in [
                        ("Gather", "A bank of questions, or a PDF, a scan, a screenshot, a Word file."),
                        ("Lay out", "Drag questions from any bank onto an exam page and reorder them."),
                        ("Print", "Four paper styles, a PDF or a DOCX, with an answer key."),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        ui.horizontal_top(|ui| {
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(20.0), egui::Sense::hover());
                            ui.painter().circle_filled(r.center(), 10.0, theme::accent_faded());
                            ui.painter().text(
                                r.center(),
                                Align2::CENTER_CENTER,
                                format!("{}", n + 1),
                                egui::FontId::new(10.5, theme::bold_family()),
                                theme::accent_col(),
                            );
                            ui.add_space(4.0);
                            ui.vertical(|ui| {
                                ui.label(theme::text(head).size(13.0).strong());
                                ui.add(egui::Label::new(theme::muted(body).size(11.5)).wrap());
                            });
                        });
                        if n < 2 {
                            ui.add_space(6.0);
                        }
                    }
                };
                if two {
                    ui.columns(2, |cols| {
                        cols[0].set_width(cols[0].available_width() - 12.0);
                        left(&mut cols[0]);
                        right(&mut cols[1]);
                    });
                } else {
                    left(ui);
                    ui.add_space(14.0);
                    right(ui);
                }
            });
            return;
        }

        // ---- Recent exams ----
        let mut exams: Vec<ExamRow> = self
            .store
            .exams
            .iter()
            .map(|e| {
                let mut pts = 0.0;
                for it in &e.items {
                    if let Some(q) = self
                        .store
                        .banks
                        .iter()
                        .find(|b| b.id == it.bank_id)
                        .and_then(|b| b.questions.iter().find(|q| q.id == it.question_id))
                    {
                        pts += q.points.unwrap_or(1.0);
                    }
                }
                let bank = e
                    .items
                    .first()
                    .and_then(|it| self.store.banks.iter().find(|b| b.id == it.bank_id))
                    .map(|b| b.name.clone())
                    .unwrap_or_else(|| "No bank yet".to_string());
                ExamRow {
                    id: e.id.clone(),
                    name: e.name.clone(),
                    qs: e.items.len(),
                    pts,
                    bank,
                    edited: e.modified,
                }
            })
            .collect();
        exams.sort_by(|a, b| b.edited.cmp(&a.edited));

        let mut exam_mode = if self.settings.home_exams_list { kit::ViewMode::List } else { kit::ViewMode::Cards };
        let mut link_exams = false;
        // A narrow page has one view of its recent exams, the compact rows, so it shows no
        // cards/list switch (a switch there named a view that was not the one shown).
        let compact_exams = ui.available_width() < HOME_COMPACT_W;
        ui.horizontal(|ui| {
            kit::label(ui, "Recent exams");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !compact_exams {
                    if kit::view_toggle(ui, &mut exam_mode, "home_exams") {
                        self.settings.home_exams_list = exam_mode == kit::ViewMode::List;
                        self.save_view_prefs();
                    }
                    ui.add_space(4.0);
                }
                if kit::link(ui, "All exams", true).clicked() {
                    link_exams = true;
                }
            });
        });
        ui.add_space(6.0);
        if link_exams {
            self.screen = crate::Screen::Exams;
        }
        let mut open_ex: Option<Id> = None;
        if exams.is_empty() {
            if crate::pageshell::empty_line(ui, crate::handicons::EXAMS, "No exams yet. An exam you start is listed here.", Some("New exam")).is_some() {
                self.first_exam();
            }
        } else if compact_exams {
            let rows: Vec<&ExamRow> = exams.iter().take(3).collect();
            if let Some(id) = exam_rows_compact(ui, self, &rows) {
                open_ex = Some(id);
            }
        } else if exam_mode == kit::ViewMode::Cards {
            let mut cols = grid_cols(ui.available_width(), 230.0, grid_gap, 3);
            // Fewer exams than columns: the cards take the row (up to a readable width) rather
            // than leaving the page's right side empty.
            cols = exams.len().clamp(1, cols);
            let cw = ((ui.available_width() - grid_gap * (cols as f32 - 1.0)) / cols as f32).floor();
            for chunk in exams.iter().take(cols * 2).collect::<Vec<_>>().chunks(cols) {
                // Every card has the same slots, so every card is HOME_EXAM_H.
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = grid_gap;
                    for (k, e) in chunk.iter().enumerate() {
                        if exam_card(ui, self, cw, e, k % 2 == 1) {
                            open_ex = Some(e.id.clone());
                        }
                    }
                });
                ui.add_space(grid_gap);
            }
        } else {
            let rows: Vec<&ExamRow> = exams.iter().take(8).collect();
            if let Some(id) = exam_list(ui, &rows) {
                open_ex = Some(id);
            }
        }
        if let Some(id) = open_ex {
            self.open_exam(&id);
        }
        ui.add_space(22.0);

        // ---- Your banks ----
        let mut banks: Vec<crate::bankui::BankTile> = self.store.banks.iter().map(|b| crate::bankui::BankTile::of(&self.store, b)).collect();
        banks.sort_by(|a, b| b.edited.cmp(&a.edited));
        let mut bank_mode = if self.settings.home_banks_list { kit::ViewMode::List } else { kit::ViewMode::Cards };
        let mut link_banks = false;
        ui.horizontal(|ui| {
            kit::label(ui, "Your banks");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if kit::view_toggle(ui, &mut bank_mode, "home_banks") {
                    self.settings.home_banks_list = bank_mode == kit::ViewMode::List;
                    self.save_view_prefs();
                }
                ui.add_space(4.0);
                if kit::link(ui, "All banks", true).clicked() {
                    link_banks = true;
                }
            });
        });
        ui.add_space(6.0);
        if link_banks {
            self.screen = crate::Screen::Banks;
        }
        let mut open_bk: Option<Id> = None;
        if banks.is_empty() {
            if crate::pageshell::empty_line(ui, crate::handicons::BANKS, "No banks yet. A bank you start is listed here.", Some("New bank")).is_some() {
                self.first_bank();
            }
        } else if bank_mode == kit::ViewMode::Cards {
            use crate::bankui::{bank_tile, TileAct, TileVariant};
            let mut cols = grid_cols(ui.available_width(), 226.0, grid_gap, 4);
            cols = banks.len().clamp(1, cols);
            let cw = ((ui.available_width() - grid_gap * (cols as f32 - 1.0)) / cols as f32).floor();
            for chunk in banks.iter().take(cols * 2).collect::<Vec<_>>().chunks(cols) {
                // Every tile is BANK_TILE_H (the Banks screen's tile), so a row ends on one line.
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = grid_gap;
                    for b in chunk {
                        match bank_tile(ui, self, cw, b, TileVariant::Home) {
                            Some(TileAct::Usage) => crate::bankui::open_usage(ui.ctx(), b.name.clone(), b.used_in.clone()),
                            Some(_) => open_bk = Some(b.id.clone()),
                            None => {}
                        }
                    }
                });
                ui.add_space(grid_gap);
            }
        } else {
            let rows: Vec<&crate::bankui::BankTile> = banks.iter().take(10).collect();
            if let Some(id) = bank_list(ui, &rows) {
                open_bk = Some(id);
            }
        }
        if let Some(id) = open_bk {
            self.open_bank(&id);
        }

        // ---- Last export strip ----
        if let Some(rec) = self.store.exports.iter().max_by_key(|r| r.time).cloned() {
            ui.add_space(20.0);
            let mut go = false;
            Frame::new()
                .fill(theme::card())
                .stroke(Stroke::new(1.0, theme::border()))
                .corner_radius(CornerRadius::same(255))
                .inner_margin(Margin::symmetric(16, 9))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width().max(0.0));
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::hover());
                        draw_printer_icon(ui.painter(), r.center(), theme::accent_col());
                        let what = if rec.what.is_empty() { String::new() } else { format!(" with {}", rec.what.iter().skip(1).cloned().collect::<Vec<_>>().join(", ")) };
                        let what = if what == " with " { String::new() } else { what };
                        ui.label(theme::strong("Last export").size(crate::kit::ty::META));
                        ui.label(
                            theme::text(format!(
                                "{} · {} · {}{}, {}",
                                fmt_when(rec.time),
                                rec.exam_name,
                                rec.format.to_uppercase(),
                                what,
                                plural(rec.versions.max(1) as usize, "version")
                            ))
                            .size(crate::kit::ty::META),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if crate::kit::link(ui, "Export history", false).clicked() {
                                go = true;
                            }
                        });
                    });
                });
            if go {
                if self.store.exams.iter().any(|e| e.id == rec.exam_id) {
                    self.history_open = true;
                    self.open_exam(&rec.exam_id);
                } else {
                    self.toast("That exam no longer exists, so its history is not available.");
                }
            }
        }
        ui.add_space(16.0);
        // Hold the bottom of the window: the dashboard is shorter than a maximised screen and
        // used to end on a floor of empty grid paper.
        crate::pageshell::footer(
            ui,
            "Saved on this machine · banks, exams and figures all live in one folder",
        );
    }

    // ---------- settings ----------

    fn save_settings(&mut self) {
        let _ = store::save_json("settings.json", &self.settings);
    }

    fn settings_changed(&self, before: &crate::Settings) -> bool {
        before.ai_provider != self.settings.ai_provider
            || before.ai_model != self.settings.ai_model
            || before.ai_base != self.settings.ai_base
            || before.ollama_base != self.settings.ollama_base
    }

    /// Help line under the AI settings: where the key comes from for the provider.
    pub fn ai_help_line(&self) -> String {
        let cfg = crate::ai::AiCfg::from(&self.settings);
        match cfg.provider.as_str() {
            "cloud" => format!("Ollama Cloud runs gemma4:31b on Ollama's servers. {}", cfg.key_source_help()),
            "ollama" => "Ollama runs on your machine: install it, pull a vision model (the \
                  default is gemma4:31b), and verSHion finds it at 127.0.0.1:11434."
                .to_string(),
            _ => cfg.key_source_help(),
        }
    }

    /// Settings > AI: masked key entry, save/remove, status and a connection test for the
    /// current keyed provider. The key lives only in the text buffer until saved (then it is
    /// cleared), and in `ai-keys.json`; it is never shown, logged or put in a toast.
    /// Model picker: a dropdown filled by "Find models", plus a free-text name for models the
    /// server does not list. Find models keeps only the models this account can use to read a
    /// picture (`ai::check_models`) and remembers them in settings, so the list survives a restart.
    fn draw_model_picker(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        use std::sync::{Arc, Mutex};
        #[derive(Default)]
        struct Job {
            busy: bool,
            progress: (usize, usize),
            result: Option<Result<crate::ai::Checked, String>>,
        }
        let cfg = crate::ai::AiCfg::from(&self.settings);
        let id_job = egui::Id::new("ai_models_job");
        let id_msg = egui::Id::new("ai_models_msg");
        let job: Arc<Mutex<Job>> = ctx.data_mut(|d| d.get_temp(id_job)).unwrap_or_default();
        let mut msg: Option<(String, bool)> = ctx.data_mut(|d| d.get_temp(id_msg));
        let target = format!("{}|{}", cfg.provider, cfg.base);
        let finished = job.lock().ok().and_then(|mut g| g.result.take());
        if let Some(r) = finished {
            match r {
                Ok(c) => {
                    msg = Some((crate::ai::checked_note(&c), false));
                    self.settings.ai_models_ok = c.ok;
                    self.settings.ai_models_for = target.clone();
                    self.save_settings();
                }
                Err(e) => msg = Some((e, true)),
            }
        }
        let (busy, progress) = job.lock().map(|g| (g.busy, g.progress)).unwrap_or((false, (0, 0)));
        let models: Vec<String> =
            if self.settings.ai_models_for == target { self.settings.ai_models_ok.clone() } else { Vec::new() };
        ui.horizontal(|ui| {
            ui.label(theme::text("Model"));
            let mut chosen = self.settings.ai_model.trim().to_string();
            let shown = if chosen.is_empty() { format!("{} (default)", cfg.model) } else { chosen.clone() };
            egui::ComboBox::from_id_salt("ai_model_combo")
                .width(300.0)
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut chosen, String::new(), format!("Provider default ({})", cfg.model));
                    for m in &models {
                        ui.selectable_value(&mut chosen, m.clone(), m.as_str());
                    }
                    if models.is_empty() {
                        ui.label(
                            theme::muted("Press Find models to list the models your account can use to read images.")
                                .size(11.5),
                        );
                    }
                });
            if chosen.as_str() != self.settings.ai_model.trim() {
                self.settings.ai_model = chosen;
            }
            if ui.add_enabled_ui(!busy, |ui| crate::pageshell::outline(ui, "Find models").clicked()).inner {
                msg = None;
                if let Ok(mut g) = job.lock() {
                    g.busy = true;
                    g.progress = (0, 0);
                }
                let job2 = job.clone();
                let ctx2 = ctx.clone();
                let cfg2 = cfg.clone();
                #[cfg(not(target_arch = "wasm32"))]
                std::thread::spawn(move || {
                    let (job3, ctx3) = (job2.clone(), ctx2.clone());
                    let r = crate::ai::check_models(&cfg2, &move |done, total| {
                        if let Ok(mut g) = job3.lock() {
                            g.progress = (done, total);
                        }
                        ctx3.request_repaint();
                    });
                    if let Ok(mut g) = job2.lock() {
                        g.busy = false;
                        g.result = Some(r);
                    }
                    ctx2.request_repaint();
                });
                #[cfg(target_arch = "wasm32")]
                wasm_bindgen_futures::spawn_local(async move {
                    let (job3, ctx3) = (job2.clone(), ctx2.clone());
                    let r = crate::ai::check_models_async(&cfg2, &move |done, total| {
                        if let Ok(mut g) = job3.lock() {
                            g.progress = (done, total);
                        }
                        ctx3.request_repaint();
                    })
                    .await;
                    if let Ok(mut g) = job2.lock() {
                        g.busy = false;
                        g.result = Some(r);
                    }
                    ctx2.request_repaint();
                });
            }
        });
        ui.horizontal(|ui| {
            ui.label(theme::muted("or type a model name").size(12.0));
            ui.add(
                egui::TextEdit::singleline(&mut self.settings.ai_model)
                    .margin(egui::Margin::symmetric(12, 9))
                    .desired_width(300.0)
                    .hint_text(cfg.model.as_str()),
            );
        });
        if busy {
            let (done, total) = progress;
            let text = if total == 0 { "Checking models…".to_string() } else { format!("Checking models… {done} of {total}") };
            ui.label(theme::muted(text).size(12.0));
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        } else if let Some((m, bad)) = &msg {
            let t = if *bad { theme::muted(m.clone()).color(theme::bad()) } else { theme::muted(m.clone()) };
            ui.label(t.size(12.0));
        }
        ctx.data_mut(|d| {
            d.insert_temp(id_job, job);
            match msg {
                Some(m) => {
                    d.insert_temp(id_msg, m);
                }
                None => {
                    d.remove::<(String, bool)>(id_msg);
                }
            }
        });
    }

    /// Coffee, the issue list and the report form. The report opens a prefilled GitHub issue;
    /// nothing is sent from the app and no server secret is involved.
    fn draw_community(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        use crate::kit;
        const COFFEE: &str = "https://www.buymeacoffee.com/shuff57";
        const ISSUES: &str = "https://github.com/shuff57/verSHion/issues";
        kit::card().inner_margin(Margin::same(18)).show(ui, |ui| {
            ui.set_min_width(ui.available_width().max(0.0));
            ui.label(theme::serif("Community").size(18.0));
            ui.add_space(4.0);
            ui.label(theme::muted(
                "verSHion is free. A coffee helps keep it going; bugs and ideas live on GitHub.",
            ));
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                if crate::pageshell::outline(ui, &format!("{}  Buy me a coffee", egui_phosphor::regular::COFFEE)).clicked() {
                    crate::platform::open_url(COFFEE);
                }
                if crate::pageshell::outline(ui, &format!("{}  Reported issues", egui_phosphor::regular::LIST_BULLETS)).clicked() {
                    crate::platform::open_url(ISSUES);
                }
                if crate::pageshell::outline(ui, &format!("{}  Desktop app", egui_phosphor::regular::DOWNLOAD_SIMPLE)).clicked() {
                    crate::platform::open_url(DESKTOP_RELEASES);
                }
                if crate::pageshell::outline(ui, &format!("{}  Report a bug", egui_phosphor::regular::BUG)).clicked() {
                    ctx.data_mut(|d| d.insert_temp(egui::Id::new("report_open"), true));
                }
            });
        });
        ui.add_space(12.0);
    }

    /// The alpha popup (first open) or the corner "Report an issue" button, and the report dialog
    /// they open. Drawn by the app on every screen, so a report can start anywhere.
    pub(crate) fn draw_alpha_notice(&mut self, ctx: &egui::Context) {
        use crate::alphanote::{self, Outcome};
        // Leaving Get Started is what counts as having visited.
        if !matches!(self.screen, crate::Screen::Welcome) {
            alphanote::mark_visited(ctx);
        }
        let first = !alphanote::seen(ctx);
        // The alpha popup opens after Get Started, not over it.
        if first && matches!(self.screen, crate::Screen::Welcome) {
            return;
        }
        let out = alphanote::show(ctx, first);
        if first && matches!(out, Outcome::Closed | Outcome::Report) {
            alphanote::mark_seen(ctx);
        }
        if out == Outcome::Report {
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("report_open"), true));
        }
        self.draw_report_dialog(ctx);
    }

    fn draw_report_dialog(&mut self, ctx: &egui::Context) {
        let id_open = egui::Id::new("report_open");
        let id_kind = egui::Id::new("report_kind");
        let id_title = egui::Id::new("report_title");
        let id_desc = egui::Id::new("report_desc");
        let mut open: bool = ctx.data_mut(|d| d.get_temp(id_open)).unwrap_or(false);
        if !open {
            return;
        }
        let mut kind: usize = ctx.data_mut(|d| d.get_temp(id_kind)).unwrap_or(0);
        let mut title: String = ctx.data_mut(|d| d.get_temp(id_title)).unwrap_or_default();
        let mut desc: String = ctx.data_mut(|d| d.get_temp(id_desc)).unwrap_or_default();
        const KINDS: [(&str, &str); 3] = [
            ("Bug", "doesn't work"),
            ("Quirk", "works, but picky"),
            ("Improve", "wish it had"),
        ];
        let mut close_now = false;
        egui::Window::new("Report an issue")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_min_width(380.0);
                ui.horizontal(|ui| {
                    for (i, (label, hint)) in KINDS.iter().enumerate() {
                        if ui.selectable_label(kind == i, *label).on_hover_text(*hint).clicked() {
                            kind = i;
                        }
                    }
                });
                ui.add_space(6.0);
                ui.label(theme::text("Title"));
                ui.add(
                    egui::TextEdit::singleline(&mut title)
                        .desired_width(f32::INFINITY)
                        .hint_text("One line: what went wrong?"),
                );
                ui.add_space(6.0);
                ui.label(theme::text("What happened?"));
                ui.add(
                    egui::TextEdit::multiline(&mut desc)
                        .desired_width(f32::INFINITY)
                        .desired_rows(4)
                        .hint_text("What did you try, and what happened instead?"),
                );
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    let ready = !title.trim().is_empty() && !desc.trim().is_empty();
                    if crate::pageshell::primary(ui, "Open a GitHub issue").clicked() && ready {
                        crate::platform::open_url(&issue_url(kind, &title, &desc));
                        close_now = true;
                        title.clear();
                        desc.clear();
                    }
                    if crate::pageshell::outline(ui, "Cancel").clicked() {
                        close_now = true;
                    }
                });
                ui.label(
                    theme::muted("This opens a prefilled issue on GitHub; nothing is sent from verSHion.")
                        .size(11.5),
                );
            });
        if close_now {
            open = false;
        }
        ctx.data_mut(|d| {
            d.insert_temp(id_open, open);
            d.insert_temp(id_kind, kind);
            d.insert_temp(id_title, title);
            d.insert_temp(id_desc, desc);
        });
    }

    fn draw_ai_key_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        use crate::kit;
        let cfg = crate::ai::AiCfg::from(&self.settings);
        if !cfg.uses_key() {
            return;
        }
        type Probe = std::sync::Arc<std::sync::Mutex<Option<Result<(), String>>>>;
        let id_buf = egui::Id::new("ai_key_buf");
        let id_probe = egui::Id::new("ai_key_probe");
        let id_msg = egui::Id::new("ai_key_msg");
        let mut buf: String = ctx.data_mut(|d| d.get_temp(id_buf)).unwrap_or_default();
        let probe: Probe = ctx.data_mut(|d| d.get_temp(id_probe)).unwrap_or_default();
        let mut msg: Option<(String, bool)> = ctx.data_mut(|d| d.get_temp(id_msg));
        if let Some(r) = probe.lock().ok().and_then(|mut g| g.take()) {
            msg = Some(match r {
                Ok(()) => ("Connection OK".to_string(), false),
                Err(e) => (e, true),
            });
        }
        let testing = msg.as_ref().is_some_and(|m| m.0 == "Testing…");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(theme::text(if cfg.provider == "workers" { "Relay key" } else { "API key" }));
            ui.add(
                egui::TextEdit::singleline(&mut buf)
                    .margin(egui::Margin::symmetric(12, 9))
                    .password(true)
                    .desired_width(300.0)
                    .hint_text("paste a key"),
            );
            if crate::pageshell::primary(ui, "Save key").clicked() && !buf.trim().is_empty() {
                match crate::ai::set_key(&cfg.provider, Some(&buf)) {
                    Ok(()) => msg = Some(("Key saved on this computer".to_string(), false)),
                    Err(e) => msg = Some((e, true)),
                }
                buf.clear();
            }
        });
        ui.horizontal(|ui| {
            let status = cfg.key_status();
            ui.label(theme::muted(status.describe()).size(12.0));
            if crate::ai::load_keys().get(&cfg.provider).is_some() && kit::danger_text_button(ui, "Remove key").clicked() {
                match crate::ai::set_key(&cfg.provider, None) {
                    Ok(()) => msg = Some(("Key removed".to_string(), false)),
                    Err(e) => msg = Some((e, true)),
                }
            }
            if !testing && crate::pageshell::outline(ui, "Test connection").clicked() {
                msg = Some(("Testing…".to_string(), false));
                let probe2 = probe.clone();
                let ctx2 = ctx.clone();
                let cfg2 = cfg.clone();
                #[cfg(not(target_arch = "wasm32"))]
                std::thread::spawn(move || {
                    let r = crate::ai::test_connection(&cfg2);
                    if let Ok(mut g) = probe2.lock() {
                        *g = Some(r);
                    }
                    ctx2.request_repaint();
                });
                #[cfg(target_arch = "wasm32")]
                wasm_bindgen_futures::spawn_local(async move {
                    let r = crate::ai::test_connection_async(&cfg2).await;
                    if let Ok(mut g) = probe2.lock() {
                        *g = Some(r);
                    }
                    ctx2.request_repaint();
                });
            }
        });
        if crate::platform::help_clip_name(&cfg.provider).is_some() {
            ui.horizontal(|ui| {
                let label =
                    format!("{}  Watch how to get a key", egui_phosphor::regular::PLAY_CIRCLE);
                if crate::pageshell::outline(ui, &label).clicked() {
                    crate::platform::open_help_video(&cfg.provider);
                }
            });
        }
        if let Some((m, bad)) = &msg {
            let t = if *bad { theme::muted(m.clone()).color(theme::bad()) } else { theme::muted(m.clone()) };
            ui.label(t.size(12.0));
        }
        ctx.data_mut(|d| {
            d.insert_temp(id_buf, buf);
            d.insert_temp(id_probe, probe);
            match msg {
                Some(m) => {
                    d.insert_temp(id_msg, m);
                }
                None => {
                    d.remove::<(String, bool)>(id_msg);
                }
            }
        });
    }

    pub fn draw_settings(&mut self, ctx: &egui::Context) {
        use crate::{kit, pageshell};
        shell_frame(self, ctx, true, |app, ui| {
          {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                // Wider than the other pages' 980: at 980 the page left a dead strip of grid right of
                // its column on a desktop window.
                ui.set_max_width(ui.available_width().min(1160.0));
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        pageshell::title(ui, "Home  /  Account", "Account", "Your account, backups and AI settings. Your work is saved on this machine; keep a copy by exporting it.");
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // A quiet help action, the tertiary tier: as a full outlined button it
                        // outweighed Connect, the card's real action.
                        if kit::tertiary(ui, egui_phosphor::regular::QUESTION, "How it works", 34.0).clicked() {
                            app.help_open = true;
                        }
                    });
                });
                ui.add_space(18.0);

                let items = [
                    pageshell::NavItem { icon: crate::handicons::PERSON, label: "Account" },
                    pageshell::NavItem { icon: crate::handicons::DOWNLOAD, label: "Backup & storage" },
                    pageshell::NavItem { icon: crate::handicons::SPARKLE, label: "AI conversion" },
                    pageshell::NavItem { icon: crate::handicons::HELP, label: "Help & community" },
                ];
                // A full-width closing row keeps local storage separate from section navigation.
                let (n_banks, n_exams) = (app.store.banks.len(), app.store.exams.len());
                let n_q: usize = app.store.banks.iter().map(|b| b.questions.len()).sum();
                let mut backup = false;
                let aside = |ui: &mut egui::Ui| {
                    let plural = |n: usize, one: &str| format!("{n} {one}{}", if n == 1 { "" } else { "s" });
                    let counts = [plural(n_banks, "bank"), plural(n_exams, "exam"), plural(n_q, "question")];
                    let w = ui.available_width();
                    kit::card().inner_margin(Margin::symmetric(20, 14)).show(ui, |ui| {
                        ui.set_width(w - 42.0);
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(theme::strong("On this machine").size(14.0));
                                ui.label(theme::muted(counts.join(" · ")).size(13.0));
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let label = format!("{}  Download backup", egui_phosphor::regular::FILE_ARROW_DOWN);
                                backup = pageshell::outline(ui, &label).clicked();
                            });
                        });
                    });
                };
                // On the account page the always-available local backup comes first and the optional
                // sharing setup follows it.
                pageshell::sections_aside(ui, "settings", &items, &[SETTINGS_ACCOUNT], aside, |ui, section| {
                // fields and menus share the buttons' 12 px corners
                let w = &mut ui.visuals_mut().widgets;
                for state in [&mut w.inactive, &mut w.hovered, &mut w.active] {
                    state.corner_radius = CornerRadius::same(12);
                }
                match section {
                SETTINGS_ACCOUNT => {
                // --- Account (optional): the sign-in forms, or the signed-in account's status ---
                app.account.pin();
                pageshell::card(ui, |ui| {
                    let narrow_window = ui.ctx().content_rect().width() < 900.0;
                    // A desktop window: the form and what an account adds side by side, split by a
                    // hairline. Stacked, both sat in the left part of the wide card and left its right
                    // half empty; the status page (signed in) fills the card on its own.
                    if !narrow_window && !matches!(app.account.model.page, crate::share::model::Page::Status) {
                        let inner = ui.available_width();
                        let gutter = 24.0;
                        // narrow enough that the address field gets more room than it had stacked;
                        // the descriptions are cut short to fit one line each here (a wider column
                        // squeezed the form and lost 3 of 4 side-by-side verdicts)
                        let right_w = (inner * 0.27).clamp(200.0, 260.0);
                        let left_w = (inner - right_w - 2.0 * gutter - 1.0).max(320.0);
                        let top = ui.cursor().top();
                        let row = ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            ui.allocate_ui_with_layout(Vec2::new(left_w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                                ui.set_width(left_w);
                                ui.spacing_mut().item_spacing.x = 8.0;
                                ui.add_space(4.0);
                                pageshell::heading(ui, app.account.page_title());
                                ui.add_space(6.0);
                                // The server form says where an address comes from under its
                                // helper line, not in a separate inset box below it.
                                app.account.draw_account_section(ui);
                            });
                            ui.add_space(2.0 * gutter + 1.0);
                            ui.allocate_ui_with_layout(Vec2::new(right_w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                                ui.set_width(right_w);
                                ui.spacing_mut().item_spacing.x = 8.0;
                                // level with the card's title across the hairline
                                ui.add_space(10.0);
                                // Sized so the three end about level with the compact form beside
                                // them (at 40 px the column ran well below it).
                                crate::share::shared_ui::account_reasons_inline(ui, 20.0);
                                ui.add_space(6.0);
                            });
                        });
                        let x = ui.min_rect().left() + left_w + gutter + 0.5;
                        ui.painter().vline(x, top + 2.0..=row.response.rect.bottom() - 2.0, Stroke::new(1.0, theme::border()));
                        return;
                    }
                    // The card used to open on a plain grey sentence; say what it is first.
                    pageshell::heading(ui, app.account.page_title());
                    ui.add_space(2.0);
                    app.account.draw_account_section(ui);
                    // What an account adds belongs to this card, under a hairline, not in a row of
                    // tile-like cards of its own below it.
                    ui.add_space(if narrow_window { 4.0 } else { 12.0 });
                    let (rule, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), egui::Sense::hover());
                    ui.painter().hline(rule.x_range(), rule.center().y, Stroke::new(1.0, theme::border()));
                    ui.add_space(if narrow_window { 4.0 } else { 12.0 });
                    crate::share::shared_ui::account_reasons_inline(ui, if narrow_window { 3.0 } else { 20.0 });
                });
                }
                1 => {
                // --- Back up / restore ---
                pageshell::card(ui, |ui| {
                    pageshell::heading(ui, "Back up");
                    ui.add_space(4.0);
                    ui.label(theme::muted(
                        "Zips up every bank, exam, import, export, setting and picture into one file.",
                    ));
                    ui.add_space(2.0);
                    ui.label(
                        theme::muted(
                            "Restoring replaces everything in verSHion with that file's contents, so keep the backup somewhere else too.",
                        )
                        .size(crate::kit::ty::META),
                    );
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if pageshell::primary(ui, "Download backup").clicked() {
                            app.backup_now();
                        }
                        if pageshell::outline(ui, "Restore from backup…").clicked() {
                            app.start_restore();
                        }
                    });
                });
                ui.add_space(12.0);

                // --- Storage location ---
                pageshell::card(ui, |ui| {
                    pageshell::heading(ui, "Where your work is stored");
                    ui.add_space(6.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(store::backend_label())
                                .color(theme::text_col())
                                .size(crate::kit::ty::META)
                                .family(egui::FontFamily::Monospace),
                        )
                        .wrap(),
                    );
                    ui.add_space(6.0);
                    let n_banks = app.store.banks.len();
                    let n_exams = app.store.exams.len();
                    let n_q: usize = app.store.banks.iter().map(|b| b.questions.len()).sum();
                    ui.label(
                        theme::muted(format!(
                            "{n_banks} bank{} · {n_exams} exam{} · {n_q} question{} on this computer",
                            if n_banks == 1 { "" } else { "s" },
                            if n_exams == 1 { "" } else { "s" },
                            if n_q == 1 { "" } else { "s" },
                        ))
                        .size(crate::kit::ty::META),
                    );
                    ui.add_space(8.0);
                    if pageshell::outline(ui, "Open folder").clicked() {
                        let _ = crate::platform::open_path(&store::store_dir());
                    }
                });
                ui.add_space(12.0);

                // --- What is in that folder ---
                pageshell::card(ui, |ui| {
                    pageshell::heading(ui, "What is in that folder");
                    ui.add_space(6.0);
                    for (name, what) in [
                        ("banks.json", "every question bank, with its topics and points"),
                        ("exams.json", "every exam, its paper style and its page order"),
                        ("settings.json", "the theme, the window size and your view choices"),
                        ("exports", "each PDF and DOCX you have downloaded, with its date"),
                        ("figures", "the pictures and figure sheets you have made"),
                    ] {
                        ui.horizontal_wrapped(|ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(name)
                                        .size(crate::kit::ty::META)
                                        .family(egui::FontFamily::Monospace)
                                        .color(theme::text_col()),
                                ),
                            );
                            ui.add(
                                egui::Label::new(theme::muted(format!("— {what}")).size(crate::kit::ty::META)).wrap(),
                            );
                        });
                    }
                });
                }
                2 => {
                // --- AI conversion ---
                pageshell::card(ui, |ui| {
                    pageshell::heading(ui, "AI conversion");
                    ui.add_space(4.0);
                    ui.label(theme::muted(
                        "Used when a test is converted for you. Runs where you point it, by \
                         default entirely on your machine.",
                    ));
                    ui.add_space(10.0);
                    let before = app.settings.clone();
                    ui.horizontal_wrapped(|ui| {
                        for (val, label) in [
                            ("account", "verSHion AI (credits)"),
                            ("cloud", "Ollama Cloud"),
                            ("workers", "verSHion relay"),
                            ("ollama", "Ollama (local)"),
                            ("openrouter", "OpenRouter"),
                            ("custom", "Custom endpoint"),
                        ] {
                            let on = app.settings.ai_provider == val;
                            if kit::chip_toggle(ui, label, on).clicked() {
                                app.settings.ai_provider = val.to_string();
                            }
                        }
                    });
                    ui.add_space(6.0);
                    if app.settings.ai_provider == "custom" || app.settings.ai_provider == "workers" {
                        ui.horizontal(|ui| {
                            ui.label(theme::text("Base URL"));
                            let hint = if app.settings.ai_provider == "workers" {
                                "https://your-relay.example/ai"
                            } else {
                                "http://127.0.0.1:8000/v1"
                            };
                            ui.add(
                                egui::TextEdit::singleline(&mut app.settings.ai_base)
                                    .margin(egui::Margin::symmetric(12, 9))
                                    .desired_width(360.0)
                                    .hint_text(hint),
                            );
                        });
                    }
                    if app.settings.ai_provider == "custom" {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(theme::muted("Presets").size(11.5));
                            for (label, url) in [
                                ("LM Studio", "http://127.0.0.1:1234/v1"),
                                ("llama.cpp", "http://127.0.0.1:8080/v1"),
                                ("vLLM", "http://127.0.0.1:8000/v1"),
                                ("Ollama /v1", "http://127.0.0.1:11434/v1"),
                            ] {
                                if kit::chip_toggle(ui, label, app.settings.ai_base == url).clicked() {
                                    app.settings.ai_base = url.to_string();
                                }
                            }
                        });
                    }
                    if app.settings.ai_provider == "ollama" {
                        ui.horizontal(|ui| {
                            ui.label(theme::text("Ollama URL"));
                            ui.add(
                                egui::TextEdit::singleline(&mut app.settings.ollama_base)
                                    .margin(egui::Margin::symmetric(12, 9))
                                    .desired_width(360.0)
                                    .hint_text("http://127.0.0.1:11434 (this machine)"),
                            );
                        });
                    }
                    if app.settings.ai_provider == "account" {
                        // Charged to the signed-in account: say whether there is one.
                        if crate::ai::account_ai_ready() {
                            ui.label(theme::text("Signed in. Each conversion uses credits from your account."));
                        } else {
                            ui.label(theme::muted("Sign in on the Account screen to use it."));
                        }
                        if pageshell::outline(ui, "Open Account…").clicked() {
                            app.account.open();
                        }
                        ui.add_space(6.0);
                    }
                    if app.settings.ai_provider == "account" {
                        // The server chooses the model and holds the key; the picker would show another provider's default.
                        ui.label(theme::muted("The verSHion server chooses the model and keeps the key, so there is nothing to pick here.").size(12.0));
                    } else {
                        app.draw_model_picker(ui, ctx);
                        ui.add_space(6.0);
                        app.draw_ai_key_ui(ui, ctx);
                    }
                    ui.add_space(6.0);
                    ui.label(theme::muted(app.ai_help_line()).size(11.5));
                    if app.settings_changed(&before) {
                        app.save_settings();
                    }
                });
                }
                _ => {
                    app.draw_community(ui, ctx);
                }
                }
                });
                if backup {
                    app.backup_now();
                }
                // A short section ends two-thirds down and leaves a floor of empty grid paper; the
                // critics named that on settings and account more than anything else. A quiet rule
                // holds the bottom of the viewport instead.
                crate::pageshell::footer(
                    ui,
                    if ui.ctx().content_rect().width() < 900.0 {
                        "verSHion · saved on this machine"
                    } else {
                        "verSHion · your work is saved on this machine, and nothing leaves it unless you share it"
                    },
                );
            });
          }
        });
    }

    // ---------- toasts ----------

    pub fn draw_toasts(&mut self, ctx: &egui::Context) {
        let now = now_ms();
        // Keep toast_times parallel to toasts; other screens may push without times.
        while self.toast_times.len() < self.toasts.len() {
            self.toast_times.push(now);
        }
        self.toast_times.truncate(self.toasts.len());
        // Expire old entries (any order; front entries are the oldest).
        let mut i = self.toasts.len();
        let mut expired: Vec<usize> = Vec::new();
        while i > 0 {
            i -= 1;
            if now - self.toast_times[i] >= TOAST_MS {
                expired.push(i);
            }
        }
        for i in expired {
            self.toasts.remove(i);
            self.toast_times.remove(i);
        }
        if self.toasts.is_empty() {
            return;
        }
        // the share toast sits in the figure stage as the reference's dark pill
        let stage = ctx.data(|d| d.get_temp::<(Rect, u64)>(egui::Id::new("share_stage"))).filter(|(_, f)| ctx.cumulative_frame_nr().saturating_sub(*f) < 3).map(|(r, _)| r);
        if let Some(stage) = stage {
            if let Some((text, _)) = self.toasts.iter().find(|(t, e)| !e && t.starts_with("Link copied")) {
                let w = ((stage.width() * 0.5).floor() - 1.0).max(160.0);
                egui::Area::new(egui::Id::new("share_pill"))
                    .order(egui::Order::Foreground)
                    .interactable(false)
                    .fixed_pos(Pos2::new(stage.center().x, stage.bottom() - if stage.width() < 500.0 { 16.0 } else { 15.0 }))
                    .pivot(Align2::CENTER_BOTTOM)
                    .show(ctx, |ui| {
                        let one = ctx.fonts_mut(|f| f.layout_no_wrap(text.clone(), egui::FontId::new(13.76, theme::semibold_family()), Color32::WHITE).size().x);
                        // a wrapped pill is 3 px taller in the reference
                        let vm = if one > w - 28.0 { Margin { left: 14, right: 14, top: 8, bottom: if one > 2.0 * (w - 28.0) { 10 } else { 9 } } } else { Margin { left: 14, right: 14, top: 7, bottom: 9 } };
                        Frame::new().fill(Color32::from_rgb(0x1f, 0x29, 0x37)).corner_radius(CornerRadius::same(27)).inner_margin(vm).show(ui, |ui| {
                            ui.set_max_width(w - 28.0);
                            // like the reference, a wrapped pill fills its maximum width
                            if one > w - 28.0 {
                                ui.set_min_width(w - 28.0);
                            }
                            if one > w - 28.0 {
                                ui.vertical_centered(|ui| {
                                    ui.add(egui::Label::new(RichText::new(text.clone()).family(theme::semibold_family()).size(13.76).color(Color32::WHITE)).halign(egui::Align::Center));
                                });
                            } else {
                                // a one-line pill hugs its text, as the reference's does
                                ui.add(egui::Label::new(RichText::new(text.clone()).family(theme::semibold_family()).size(13.76).color(Color32::WHITE)).wrap_mode(egui::TextWrapMode::Extend));
                            }
                        });
                    });
            }
        }
        let items: Vec<(String, bool)> = self.toasts.iter().filter(|(t, e)| *e || stage.is_none() || !t.starts_with("Link copied")).cloned().collect();
        // indices below refer to `self.toasts`, so map them back
        let map: Vec<usize> = (0..self.toasts.len()).filter(|&i| self.toasts[i].1 || stage.is_none() || !self.toasts[i].0.starts_with("Link copied")).collect();
        let mut dismissed: Vec<usize> = Vec::new();
        egui::Area::new(egui::Id::new("toasts"))
            .order(egui::Order::Foreground)
            .anchor(Align2::RIGHT_BOTTOM, [-16.0, -16.0 - crate::alphanote::TOAST_LIFT])
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                for (i, (text, is_error)) in items.iter().enumerate() {
                    let tint = if *is_error {
                        theme::error_wash()
                    } else {
                        theme::card()
                    };
                    let stroke = if *is_error { theme::red() } else { theme::border() };
                    Frame::new()
                        .fill(tint)
                        .stroke(Stroke::new(1.0, stroke))
                        .corner_radius(CornerRadius::same(crate::kit::R_CARD))
                        .shadow(egui::epaint::Shadow {
                            offset: [0, 4],
                            blur: 14,
                            spread: 0,
                            color: Color32::from_black_alpha(40),
                        })
                        .inner_margin(Margin::symmetric(16, 10))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(if *is_error {
                                    RichText::new(text.clone()).color(theme::red())
                                } else {
                                    theme::text(text.clone())
                                });
                                if ui
                                    .add(
                                        egui::Button::new(RichText::new(egui_phosphor::regular::X).size(12.0).color(theme::muted_col()))
                                            .fill(Color32::TRANSPARENT)
                                            .corner_radius(CornerRadius::same(255)),
                                    )
                                    .clicked()
                                {
                                    dismissed.push(i);
                                }
                            });
                        });
                }
            });
        // Click-to-dismiss, descending order so earlier indices stay valid.
        dismissed.sort_unstable_by(|a, b| b.cmp(a));
        for i in dismissed {
            let i = map[i];
            self.toasts.remove(i);
            self.toast_times.remove(i);
        }
    }

    // ---------- help overlay ----------

    pub fn draw_help_overlay(&mut self, ctx: &egui::Context) {
        if !self.help_open {
            return;
        }
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::new("vershion_help"))
            .frame(
                crate::kit::card()
                    .inner_margin(Margin::same(22))
                    .shadow(egui::epaint::Shadow { offset: [0, 8], blur: 28, spread: 0, color: Color32::from_black_alpha(60) }),
            )
            .show(ctx, |ui| {
                ui.set_width(540.0);
                ui.label(theme::serif("How verSHion works").size(26.0));
                ui.add_space(4.0);
                ui.label(theme::muted(
                    "Two ideas: Question Banks hold questions; Exams pick and arrange them for printing.",
                ));
                ui.add_space(14.0);
                for (i, (title, body)) in STEPS.iter().enumerate() {
                    ui.horizontal_top(|ui| {
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(26.0), egui::Sense::hover());
                        ui.painter().circle_filled(r.center(), 13.0, theme::accent_faded());
                        ui.painter().text(
                            r.center(),
                            Align2::CENTER_CENTER,
                            format!("{}", i + 1),
                            egui::FontId::new(12.0, theme::bold_family()),
                            theme::accent_col(),
                        );
                        ui.vertical(|ui| {
                            ui.set_max_width(480.0);
                            ui.label(theme::strong(*title).size(13.5));
                            ui.label(theme::muted(*body).size(12.0));
                        });
                    });
                    ui.add_space(8.0);
                }
                ui.add_space(4.0);
                ui.label(theme::strong("Keyboard").size(13.5));
                ui.label(theme::muted(
                    "Ctrl/Cmd+N — new exam (Home, Exams) or new bank (Banks) · Ctrl/Cmd+Enter — save a question · Esc — cancel a question · Ctrl/Cmd+Z / Ctrl/Cmd+Shift+Z — undo/redo an exam · Ctrl/Cmd+E — export an exam",
                ).size(12.0));
                ui.add_space(8.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if crate::pageshell::primary(ui, "Got it").clicked() {
                        close = true;
                    }
                });
            });
        if close || modal.should_close() {
            self.help_open = false;
        }
    }

    // ---------- backup / restore ----------

    pub fn backup_now(&mut self) {
        let dest = crate::platform::save_path(
            &format!("vershion-backup-{}.zip", fmt_date(now_secs())),
            "verSHion backups",
            &["zip"],
        );
        let Some(dest) = dest else { return };
        match write_backup(&dest) {
            Ok(()) => self.toast(format!("Backup saved: {}", dest.display())),
            Err(e) => self.toast_err(format!("Backup failed: {e}")),
        }
    }

    pub fn start_restore(&mut self) {
        let Some(path) = crate::platform::pick_file("verSHion backups", &["zip"])
        else {
            return;
        };
        self.confirm_restore = Some(path);
    }

    pub fn draw_restore_confirm(&mut self, ctx: &egui::Context) {
        let Some(path) = self.confirm_restore.clone() else { return };
        let mut action: u8 = 0; // 1 = dismissed, 2 = replace
        let modal = egui::Modal::new(egui::Id::new("vershion_restore"))
            .frame(crate::kit::card().inner_margin(Margin::same(22)))
            .show(ctx, |ui| {
                ui.set_width(440.0);
                ui.label(theme::serif("Restore from backup?").size(22.0));
                ui.add_space(6.0);
                ui.label(theme::text(
                    "This replaces ALL your banks, exams, imports and exports with the backup. Continue?",
                ));
                ui.add_space(14.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(theme::on_accent("Replace").family(theme::bold_family()))
                                .fill(theme::red())
                                .corner_radius(CornerRadius::same(255))
                                .min_size(Vec2::new(0.0, 34.0)),
                        )
                        .clicked()
                    {
                        action = 2;
                    }
                    if crate::pageshell::outline(ui, "Cancel").clicked() {
                        action = 1;
                    }
                });
            });
        if modal.should_close() && action == 0 {
            action = 1;
        }
        if action != 0 {
            self.confirm_restore = None;
            if action == 2 {
                self.restore_from(&path);
            }
        }
    }

    fn restore_from(&mut self, path: &std::path::Path) {
        match read_backup(path) {
            Ok(count) => {
                self.store = store::Store::load();
                if let Some(s) = store::load_json::<crate::Settings>("settings.json") {
                    self.settings = s;
                }
                self.toast(format!("Restored {count} files from the backup"));
            }
            Err(e) => self.toast_err(format!("Restore failed: {e}")),
        }
    }

    // ---------- demo data (VERSHION_SEED) ----------

    /// One demo bank (MC / TF / FIB) and a two-item demo exam; persisted to disk.
    pub fn seed_demo(&mut self) {
        let t = now_secs();
        let q1 = Question {
            id: model::new_id(),
            stem: "Which planet is closest to the Sun?".to_string(),
            kind: Kind::Mc {
                answers: vec![
                    Answer { text: "Mercury".into(), correct: true, locked: false },
                    Answer { text: "Venus".into(), correct: false, locked: false },
                    Answer { text: "Mars".into(), correct: false, locked: false },
                    Answer { text: "Earth".into(), correct: false, locked: false },
                ],
            },
            points: Some(1.0),
            difficulty: Some(Difficulty::Medium),
            topics: vec!["Solar system".into()],
            image: None,
            explanation: Some("Mercury is the innermost planet of the eight.".into()),
            figure: None,
            created: t,
            modified: t,
        };
        let q2 = Question {
            id: model::new_id(),
            stem: "The Sun is a star.".to_string(),
            kind: Kind::Tf { correct: true },
            points: Some(1.0),
            difficulty: Some(Difficulty::Low),
            topics: vec!["Solar system".into()],
            image: None,
            explanation: None,
            figure: None,
            created: t,
            modified: t,
        };
        let q3 = Question {
            id: model::new_id(),
            stem: "The largest planet is ___ in our solar system.".to_string(),
            kind: Kind::Blank {
                accept: vec![vec!["Jupiter".to_string()]],
            },
            points: Some(2.0),
            difficulty: Some(Difficulty::High),
            topics: vec!["Solar system".into()],
            image: None,
            explanation: None,
            figure: None,
            created: t,
            modified: t,
        };
        let (q1_id, q3_id) = (q1.id.clone(), q3.id.clone());
        let bank = QuestionBank {
            id: model::new_id(),
            name: "Solar System — Practice".into(),
            questions: vec![q1, q2, q3],
            created: t,
            modified: t,
            details: Default::default(),
        };
        let bank_id = bank.id.clone();
        self.store.banks.push(bank);
        let exam = Exam { paper_style: Default::default(), margins: None,
            id: model::new_id(),
            name: "Solar System — Quiz".into(),
            header: "Name: __________________ Class: ___________ Date: ___________".into(),
            title: Some("Solar System Quiz".into()),
            items: vec![
                ExamItem::new(bank_id.clone(), q1_id),
                ExamItem::new(bank_id, q3_id),
            ],
            heading_pt: 16.0,
            body_pt: 11.0,
            margin_mm: 18.0,
            ruled_blanks: 0,
            modified: t,
        };
        self.store.exams.push(exam);
        self.store.save_banks();
        self.store.save_exams();
    }
} // impl crate::App

// ---------- files: import mime, pdf, backup ----------

/// Rewires parsed-exam items whose bank_id is blank (the parser regenerates ids
/// and cannot know the bank id being created right now) onto `bank_id`.
/// Non-empty bank_ids pass through untouched.
// ---------- V4 home helpers (painter-drawn cards, tiles, lists) ----------

struct ExamRow {
    id: Id,
    name: String,
    qs: usize,
    pts: f64,
    bank: String,
    edited: i64,
}

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const WEEKDAYS: [&str; 7] = ["Thursday", "Friday", "Saturday", "Sunday", "Monday", "Tuesday", "Wednesday"];

/// "2 hours ago" / "Yesterday" / "Oct 9".
fn rel_time(secs: i64) -> String {
    if secs <= 0 {
        return "Not yet".to_string();
    }
    let d = (now_secs() - secs).max(0);
    if d < 60 {
        "Just now".to_string()
    } else if d < 3600 {
        format!("{} min ago", d / 60)
    } else if d < 86400 {
        plural((d / 3600) as usize, "hour") + " ago"
    } else if d < 2 * 86400 {
        "Yesterday".to_string()
    } else {
        let (_, m, day) = crate::time::civil_from_days(secs.div_euclid(86400));
        format!("{} {}", MONTHS[(m as usize).clamp(1, 12) - 1], day)
    }
}

/// "Tuesday, 4:12 pm" within the last week, else "Oct 9, 4:12 pm".
fn fmt_when(secs: i64) -> String {
    let days = secs.div_euclid(86400);
    let tod = secs.rem_euclid(86400);
    let (h, mi) = (tod / 3600, (tod % 3600) / 60);
    let (h12, ap) = (if h % 12 == 0 { 12 } else { h % 12 }, if h < 12 { "am" } else { "pm" });
    let clock = format!("{h12}:{mi:02} {ap}");
    if now_secs() - secs < 7 * 86400 {
        format!("{}, {clock}", WEEKDAYS[days.rem_euclid(7) as usize])
    } else {
        let (_, m, d) = crate::time::civil_from_days(days);
        format!("{} {d}, {clock}", MONTHS[(m as usize).clamp(1, 12) - 1])
    }
}

fn grid_cols(avail: f32, min_w: f32, gap: f32, max: usize) -> usize {
    (((avail + gap) / (min_w + gap)).floor() as usize).clamp(1, max)
}

fn fmt_pts(p: f64) -> String {
    if (p - p.round()).abs() < 0.05 { format!("{}", p.round() as i64) } else { format!("{p:.1}") }
}

/// Single-row, ellipsised text painted with its top-left at `pos`.
fn paint_fit(p: &egui::Painter, pos: Pos2, text: &str, font: egui::FontId, col: Color32, width: f32) {
    let mut job = egui::text::LayoutJob::single_section(
        text.to_string(),
        egui::text::TextFormat { font_id: font, color: col, ..Default::default() },
    );
    job.wrap.max_width = width.max(1.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let g = p.layout_job(job);
    p.galley(pos, g, col);
}

fn sans(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Proportional)
}
fn bold(size: f32) -> egui::FontId {
    egui::FontId::new(size, theme::bold_family())
}
fn serif_font(size: f32) -> egui::FontId {
    egui::FontId::new(size, theme::serif_family())
}

/// A Home card under the pointer: its border warms a little toward the accent and the card lifts
/// on a soft shadow. A full accent outline read as a selected item on a page with no selection.
fn hover_border(resp: &egui::Response) -> Stroke {
    if resp.hovered() {
        Stroke::new(1.0, theme::border().lerp_to_gamma(theme::accent_col(), 0.3))
    } else {
        Stroke::new(1.0, theme::border())
    }
}

/// The soft shadow under a hovered Home card; paint it before the card.
fn hover_lift(p: &egui::Painter, rect: Rect, resp: &egui::Response, radius: u8) {
    if resp.hovered() {
        let col = if theme::is_dark() { Color32::from_black_alpha(70) } else { Color32::from_black_alpha(22) };
        p.add(egui::Shadow { offset: [0, 3], blur: 12, spread: 0, color: col }.as_shape(rect, CornerRadius::same(radius)));
    }
}

/// One of the three action tiles. Returns true when clicked. `compact`: the narrow-window form,
/// one third of the row, a smaller glyph and the title alone.
fn home_tile(ui: &mut egui::Ui, w: f32, icon: NavIcon, title: &str, sub: &str, primary: bool, compact: bool) -> bool {
    let h = if compact { 58.0 } else { 72.0 };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let p = ui.painter();
    let (fill, stroke, fg, glyph_bg, glyph_fg) = if primary {
        let gbg = if theme::is_dark() { Color32::from_black_alpha(36) } else { Color32::from_white_alpha(46) };
        (theme::accent_col(), Stroke::new(1.0, theme::accent_col()), theme::on_accent_col(), gbg, theme::on_accent_col())
    } else {
        (theme::card(), hover_border(&resp), theme::ink(), theme::accent_faded(), theme::accent_col())
    };
    if !primary {
        hover_lift(p, rect, &resp, if compact { 16 } else { 18 });
    }
    if compact {
        p.rect(rect, CornerRadius::same(16), fill, stroke, StrokeKind::Inside);
        let g = Rect::from_min_size(Pos2::new(rect.min.x + 12.0, rect.center().y - 17.0), Vec2::splat(34.0));
        p.rect_filled(g, CornerRadius::same(11), glyph_bg);
        draw_nav_icon_k(p, icon, g.center(), glyph_fg, 1.1);
        let tx = g.right() + 11.0;
        let tw = rect.right() - tx - 8.0;
        let font = serif_font(17.0);
        let gh = p.layout_no_wrap(title.to_string(), font.clone(), fg).size().y;
        paint_fit(p, Pos2::new(tx, rect.center().y - gh / 2.0), title, font, fg, tw);
        return resp.clicked();
    }
    p.rect(rect, CornerRadius::same(18), fill, stroke, StrokeKind::Inside);
    let g = Rect::from_min_size(rect.min + Vec2::new(18.0, 15.0), Vec2::splat(42.0));
    p.rect_filled(g, CornerRadius::same(13), glyph_bg);
    draw_nav_icon_k(p, icon, g.center(), glyph_fg, 1.3);
    let tx = rect.min.x + 18.0 + 42.0 + 14.0;
    let tw = rect.right() - tx - 10.0;
    paint_fit(p, Pos2::new(tx, rect.min.y + 15.0), title, serif_font(19.0), fg, tw);
    let subc = if primary { fg.gamma_multiply(0.92) } else { theme::text_col() };
    paint_fit(p, Pos2::new(tx, rect.min.y + 40.0), sub, sans(crate::kit::ty::META), subc, tw);
    resp.clicked()
}

fn rot(c: Pos2, v: Vec2, a: f32) -> Pos2 {
    let (s, co) = a.sin_cos();
    c + Vec2::new(v.x * co - v.y * s, v.x * s + v.y * co)
}

/// Small tilted sheet of paper with faint lines (and an optional highlighter bar).
fn paper_thumb(p: &egui::Painter, c: Pos2, size: Vec2, deg: f32, hl: bool, shadow: bool) {
    let a = deg.to_radians();
    let h = size / 2.0;
    let corners = |off: Vec2| -> Vec<Pos2> {
        [Vec2::new(-h.x, -h.y), Vec2::new(h.x, -h.y), Vec2::new(h.x, h.y), Vec2::new(-h.x, h.y)]
            .iter()
            .map(|v| rot(c + off, *v, a))
            .collect()
    };
    let s = size.y / 90.0;
    if shadow {
        p.add(egui::Shape::convex_polygon(corners(Vec2::new(0.0, 6.0 * s.max(0.5))), Color32::from_black_alpha(34), Stroke::NONE));
    }
    p.add(egui::Shape::convex_polygon(
        corners(Vec2::ZERO),
        Color32::from_rgb(0xff, 0xfd, 0xf8),
        Stroke::new(1.0, Color32::from_rgb(0xcf, 0xc9, 0xb6)),
    ));
    let pad = 8.0 * s.max(0.5);
    let inner_w = size.x - pad * 1.6;
    let step = 8.0 * s;
    let thin = Stroke::new((2.0 * s).max(1.0), Color32::from_rgb(0xd3, 0xcb, 0xb8));
    for i in 0..7usize {
        let y = -h.y + pad + 2.0 + i as f32 * step;
        let (w, stroke) = match i {
            0 => (inner_w * 0.58, Stroke::new((2.0 * s).max(1.0), Color32::from_rgb(0x9d, 0x8c, 0x7b))),
            2 if hl => (inner_w * 0.78, Stroke::new((4.0 * s).max(1.5), theme::highlighter())),
            _ => (inner_w, thin),
        };
        let x0 = if i == 0 { -w / 2.0 } else { -inner_w / 2.0 };
        p.line_segment([rot(c, Vec2::new(x0, y), a), rot(c, Vec2::new(x0 + w, y), a)], stroke);
    }
}

fn paint_preview_bg(p: &egui::Painter, r: Rect) {
    p.rect_filled(r, CornerRadius { nw: 16, ne: 16, sw: 0, se: 0 }, theme::accent_faded());
    let clip = p.with_clip_rect(r);
    let mut x = r.left() + 8.0;
    while x < r.right() {
        clip.line_segment([Pos2::new(x, r.top()), Pos2::new(x, r.bottom())], Stroke::new(1.0, theme::grid_line()));
        x += 16.0;
    }
    let mut y = r.top() + 8.0;
    while y < r.bottom() {
        clip.line_segment([Pos2::new(r.left(), y), Pos2::new(r.right(), y)], Stroke::new(1.0, theme::grid_line()));
        y += 16.0;
    }
    p.line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, theme::border()));
}

/// The first page on a Home exam card, the size the Exams screen used, centred in its strip.
const HOME_THUMB: Vec2 = Vec2::new(88.0, 114.0);
/// Every Home exam card is this tall. The text column flows from the top: pad 13, the name (one or
/// two lines of 22), gap 4, meta 18, gap 2, "From <bank>" (one or two lines of 18, so a bank's
/// name is read whole); the footer is pinned: gap 7, hairline 1, gap 7, footer 18, pad 13. The
/// longest case (44 + 36) ends 7 above the hairline.
const HOME_EXAM_H: f32 = 164.0;

/// Where a Home exam card's text starts (from the card's left edge) and how wide it may run.
fn exam_card_text_x(w: f32) -> (f32, f32) {
    // card edge, 6 px, the preview strip (8 + page + 8), 14 px, the text, 16 px.
    let x = 6.0 + 8.0 + HOME_THUMB.x + 8.0 + 14.0;
    (x, (w - x - 16.0).max(60.0))
}

/// "Edited 2 hours ago", "Edited yesterday", "Edited Oct 9".
fn edited_label(secs: i64) -> String {
    let r = rel_time(secs);
    let r = if MONTHS.iter().any(|m| r.starts_with(m)) || r.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        r
    } else {
        let mut c = r.chars();
        c.next().map(|f| f.to_lowercase().collect::<String>() + c.as_str()).unwrap_or_default()
    };
    format!("Edited {r}")
}

/// A Home exam card, `w` wide and [`HOME_EXAM_H`] tall: the first page on the left, beside it the
/// name over two lines, "11 questions · 26 points", "From <bank>", a hairline, then when it was
/// edited and an Open link. The whole card opens the exam; so does the link. Returns true on either.
fn exam_card(ui: &mut egui::Ui, app: &mut crate::App, w: f32, e: &ExamRow, tilt_right: bool) -> bool {
    use crate::kit;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, HOME_EXAM_H), egui::Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &e.name));
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let p = ui.painter();
    hover_lift(p, rect, &resp, 16);
    p.rect(rect, CornerRadius::same(16), theme::card(), hover_border(&resp), StrokeKind::Inside);
    if resp.has_focus() {
        p.rect_stroke(rect.expand(1.0), CornerRadius::same(17), Stroke::new(2.0, theme::accent_col()), StrokeKind::Outside);
    }
    let pv = Rect::from_min_max(
        Pos2::new(rect.min.x + 6.0, rect.min.y + 12.0),
        Pos2::new(rect.min.x + 6.0 + 8.0 + HOME_THUMB.x + 8.0, rect.max.y - 12.0),
    );
    let thumb_area = Rect::from_center_size(pv.center(), HOME_THUMB);
    paint_preview_bg(p, pv);
    let ctx = ui.ctx().clone();
    let page = crate::examui::paint_exam_thumb(app, &ctx, p, thumb_area, &e.id);
    let p = ui.painter();
    if page.is_none() {
        paper_thumb(p, thumb_area.center(), Vec2::new(60.0, 78.0), if tilt_right { 3.0 } else { -3.0 }, e.id.len() % 2 == 0, true);
    }
    let (dx, iw) = exam_card_text_x(w);
    let x = rect.min.x + dx;
    let mut y = rect.min.y + 13.0;
    // Name: two lines at most, then the facts right under it.
    let mut job = egui::text::LayoutJob::default();
    job.append(&e.name, 0.0, egui::TextFormat { font_id: kit::card_title_font(), color: theme::ink(), line_height: Some(kit::ty::CARD_LINE), ..Default::default() });
    job.wrap = egui::text::TextWrapping { max_width: iw, max_rows: 2, break_anywhere: false, overflow_character: Some('…') };
    let name = p.layout_job(job);
    let name_h = name.rows.len().max(1) as f32 * kit::ty::CARD_LINE;
    p.galley(Pos2::new(x, y), name, theme::ink());
    y += name_h + 4.0;
    let line = |p: &egui::Painter, y: f32, text: &str, font: egui::FontId, col: Color32| {
        let g = p.layout_no_wrap(text.to_string(), font.clone(), col);
        paint_fit(p, Pos2::new(x, y + (18.0 - g.size().y) / 2.0), text, font, col, iw);
    };
    let meta = format!("{} · {} point{}", plural(e.qs, "question"), fmt_pts(e.pts), if (e.pts - 1.0).abs() < 0.05 { "" } else { "s" });
    line(p, y, &meta, sans(kit::ty::META), theme::text_col());
    y += 18.0 + 2.0;
    // "From" quiet, then the bank in the stressed weight, wrapped to two lines so it reads whole.
    let mut job = egui::text::LayoutJob::default();
    job.append("From ", 0.0, egui::TextFormat { font_id: sans(kit::ty::META), color: theme::muted_col(), line_height: Some(18.0), ..Default::default() });
    job.append(&e.bank, 0.0, egui::TextFormat { font_id: egui::FontId::new(kit::ty::META, theme::semibold()), color: theme::text_col(), line_height: Some(18.0), ..Default::default() });
    job.wrap = egui::text::TextWrapping { max_width: iw, max_rows: 2, break_anywhere: false, overflow_character: Some('…') };
    p.galley(Pos2::new(x, y), p.layout_job(job), theme::text_col());
    // The footer, pinned to the card's foot.
    let y = rect.max.y - 13.0 - 18.0;
    p.hline(x..=x + iw, y - 7.0 - 0.5, Stroke::new(1.0, theme::border()));
    line(p, y, &edited_label(e.edited), sans(kit::ty::META), theme::muted_col());
    // The Open link, at the footer's right, added after the card so it takes its own click.
    let link_w = ui.painter().layout_no_wrap("Open".to_string(), egui::FontId::new(kit::ty::META, theme::semibold()), theme::accent_col()).size().x + 17.0;
    let lr = Rect::from_min_size(Pos2::new(x + iw - link_w.ceil(), y), Vec2::new(link_w.ceil(), 18.0));
    // (a child Ui: the link never moves the row's cursor)
    let open = kit::link(&mut ui.new_child(UiBuilder::new().max_rect(lr).id_salt(("home_exam_open", &e.id))), "Open", true).clicked();
    resp.clicked() || open
}

/// Below this page width Home's recent exams are compact rows, not the card grid: two columns of
/// 164-tall cards left a half-empty second row and pushed the banks below the window.
const HOME_COMPACT_W: f32 = 700.0;
/// Each compact recent-exam row is this tall: pad 10, the name (one line of 22), the facts and
/// when it was edited (18), "From <bank>" (18), pad 8.
const HOME_ROW_H: f32 = 76.0;
/// The page thumbnail in a compact row, and the green strip it sits on.
const HOME_ROW_THUMB: Vec2 = Vec2::new(40.0, 52.0);
const HOME_ROW_STRIP: f32 = 60.0;

/// One line of `text` in `font`, cut with an ellipsis at `width`.
fn line_job(text: &str, font: egui::FontId, col: Color32, width: f32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::single_section(text.to_string(), egui::text::TextFormat { font_id: font, color: col, ..Default::default() });
    job.wrap = egui::text::TextWrapping { max_width: width.max(1.0), max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
    job
}

/// Home's recent exams in a narrow window: one card of rows, each [`HOME_ROW_H`] tall, a small
/// first page on its green strip, then the name, "11 questions · 26 points · Edited Oct 25" and
/// "From <bank>", with Open on the right. A row (or its Open) opens the exam; returns which.
fn exam_rows_compact(ui: &mut egui::Ui, app: &mut crate::App, rows: &[&ExamRow]) -> Option<Id> {
    use crate::kit;
    let w = ui.available_width();
    let (outer, _) = ui.allocate_exact_size(Vec2::new(w, HOME_ROW_H * rows.len() as f32 + 2.0), egui::Sense::hover());
    ui.painter().rect(outer, CornerRadius::same(16), theme::card(), Stroke::new(1.0, theme::border()), StrokeKind::Inside);
    let open_font = egui::FontId::new(kit::ty::META, theme::semibold());
    let open_w = (ui.painter().layout_no_wrap("Open".to_string(), open_font, theme::accent_col()).size().x + 17.0).ceil();
    let ctx = ui.ctx().clone();
    let mut hit = None;
    for (i, e) in rows.iter().enumerate() {
        let r = Rect::from_min_size(outer.min + Vec2::new(1.0, 1.0 + HOME_ROW_H * i as f32), Vec2::new(w - 2.0, HOME_ROW_H));
        let resp = ui.interact(r, ui.id().with(("home_exam_row", &e.id)), egui::Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &e.name));
        let p = ui.painter().with_clip_rect(outer.shrink(1.0).intersect(ui.clip_rect()));
        if resp.hovered() {
            // The first and last rows keep the card's rounded corners.
            let cr = CornerRadius { nw: if i == 0 { 15 } else { 0 }, ne: if i == 0 { 15 } else { 0 }, sw: if i + 1 == rows.len() { 15 } else { 0 }, se: if i + 1 == rows.len() { 15 } else { 0 } };
            p.rect_filled(r, cr, theme::hover_bg());
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if resp.has_focus() {
            p.rect_stroke(r.shrink(2.0), CornerRadius::same(12), Stroke::new(2.0, theme::accent_col()), StrokeKind::Inside);
        }
        if i + 1 < rows.len() {
            p.hline(r.left() + 16.0..=r.right() - 16.0, r.bottom() - 0.5, Stroke::new(1.0, theme::border()));
        }
        let strip = Rect::from_min_size(Pos2::new(r.left() + 8.0, r.top() + 8.0), Vec2::new(HOME_ROW_STRIP, HOME_ROW_H - 16.0));
        p.rect_filled(strip, CornerRadius::same(8), theme::accent_faded().gamma_multiply(0.7));
        let thumb = Rect::from_center_size(strip.center(), HOME_ROW_THUMB);
        if crate::examui::paint_exam_thumb(app, &ctx, &p, thumb, &e.id).is_none() {
            paper_thumb(&p, thumb.center(), Vec2::new(34.0, 44.0), 0.0, false, true);
        }
        let x = strip.right() + 14.0;
        let tw = (r.right() - 16.0 - open_w - 12.0 - x).max(60.0);
        let mut y = r.top() + 10.0;
        p.galley(Pos2::new(x, y), p.layout_job(line_job(&e.name, kit::card_title_font(), theme::ink(), tw)), theme::ink());
        y += kit::ty::CARD_LINE;
        let facts = format!("{} · {} point{} · {}", plural(e.qs, "question"), fmt_pts(e.pts), if (e.pts - 1.0).abs() < 0.05 { "" } else { "s" }, edited_label(e.edited));
        let g = p.layout_job(line_job(&facts, sans(kit::ty::META), theme::text_col(), tw));
        p.galley(Pos2::new(x, y + (18.0 - g.size().y) / 2.0), g, theme::text_col());
        y += 18.0;
        let mut job = egui::text::LayoutJob::default();
        job.append("From ", 0.0, egui::TextFormat { font_id: sans(kit::ty::META), color: theme::muted_col(), ..Default::default() });
        job.append(&e.bank, 0.0, egui::TextFormat { font_id: egui::FontId::new(kit::ty::META, theme::semibold()), color: theme::text_col(), ..Default::default() });
        job.wrap = egui::text::TextWrapping { max_width: tw, max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
        let g = p.layout_job(job);
        p.galley(Pos2::new(x, y + (18.0 - g.size().y) / 2.0), g, theme::text_col());
        // Open, right, level with the row's middle; a child Ui so it takes its own click.
        let lr = Rect::from_min_size(Pos2::new(r.right() - 16.0 - open_w, r.center().y - 9.0), Vec2::new(open_w, 18.0));
        let open = kit::link(&mut ui.new_child(UiBuilder::new().max_rect(lr).id_salt(("home_exam_row_open", &e.id))), "Open", true).clicked();
        if resp.clicked() || open {
            hit = Some(e.id.clone());
        }
    }
    hit
}

fn list_columns(w: f32, fr: &[f32]) -> Vec<(f32, f32)> {
    // First column is the fixed 56 px thumb/icon column; the rest share the space.
    let gap = 12.0;
    let fixed_end = 8.0 + 56.0;
    let total: f32 = fr.iter().sum();
    let avail = (w - fixed_end - 14.0 - gap * fr.len() as f32).max(50.0);
    let mut x = fixed_end + gap;
    let mut out = vec![(8.0, 56.0)];
    for f in fr {
        let cw = avail * f / total;
        out.push((x, cw));
        x += cw + gap;
    }
    out
}

fn list_header(p: &egui::Painter, rect: Rect, cols: &[(f32, f32)], names: &[&str]) {
    for ((x, w), n) in cols.iter().skip(1).zip(names) {
        paint_fit(p, Pos2::new(rect.min.x + x, rect.min.y + 7.0), &n.to_uppercase(), bold(crate::kit::ty::LABEL), theme::muted_col(), *w);
    }
    p.line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, theme::border()));
}

fn exam_list(ui: &mut egui::Ui, rows: &[&ExamRow]) -> Option<Id> {
    let w = ui.available_width();
    let cols = list_columns(w, &[2.2, 0.8, 0.8, 1.4, 1.0]);
    let (outer, _) = ui.allocate_exact_size(Vec2::new(w, 30.0 + 54.0 * rows.len() as f32), egui::Sense::hover());
    ui.painter().rect(outer, CornerRadius::same(16), theme::card(), Stroke::new(1.0, theme::border()), StrokeKind::Inside);
    let head = Rect::from_min_size(outer.min, Vec2::new(w, 30.0));
    list_header(ui.painter(), head, &cols, &["Name", "Questions", "Points", "Bank", "Edited"]);
    let mut hit = None;
    for (i, e) in rows.iter().enumerate() {
        let r = Rect::from_min_size(outer.min + Vec2::new(1.0, 30.0 + 54.0 * i as f32), Vec2::new(w - 2.0, 54.0));
        let resp = ui.interact(r, ui.id().with(("exl", i)), egui::Sense::click());
        let p = ui.painter();
        if resp.hovered() {
            p.rect_filled(r, CornerRadius::ZERO, theme::hover_bg());
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if i + 1 < rows.len() {
            p.line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, theme::border()));
        }
        paper_thumb(p, Pos2::new(r.min.x + 8.0 + 28.0, r.center().y), Vec2::new(30.0, 38.0), 0.0, false, true);
        let cy = r.center().y;
        let at = |c: usize| Pos2::new(outer.min.x + cols[c].0, cy);
        paint_fit(p, at(1) - Vec2::new(0.0, 9.0), &e.name, serif_font(15.0), theme::ink(), cols[1].1);
        paint_fit(p, at(2) - Vec2::new(0.0, 8.0), &e.qs.to_string(), bold(13.0), theme::ink(), cols[2].1);
        paint_fit(p, at(3) - Vec2::new(0.0, 8.0), &fmt_pts(e.pts), bold(13.0), theme::ink(), cols[3].1);
        paint_fit(p, at(4) - Vec2::new(0.0, 8.0), &e.bank, sans(13.0), theme::text_col(), cols[4].1);
        paint_fit(p, at(5) - Vec2::new(0.0, 8.0), &rel_time(e.edited), sans(13.0), theme::text_col(), cols[5].1);
        if resp.clicked() {
            hit = Some(e.id.clone());
        }
    }
    hit
}

fn paint_bank_glyph(p: &egui::Painter, r: Rect) {
    p.rect_filled(r, CornerRadius::same(10), theme::accent_faded());
    draw_nav_icon_k(p, NavIcon::Banks, r.center(), theme::accent_col(), 1.05);
}

fn bank_list(ui: &mut egui::Ui, rows: &[&crate::bankui::BankTile]) -> Option<Id> {
    let w = ui.available_width();
    let mut cols = list_columns(w - 90.0 - 12.0, &[2.2, 0.9, 0.8, 1.0]);
    cols.push((w - 14.0 - 90.0 - 0.0, 90.0));
    let (outer, _) = ui.allocate_exact_size(Vec2::new(w, 30.0 + 54.0 * rows.len() as f32), egui::Sense::hover());
    ui.painter().rect(outer, CornerRadius::same(16), theme::card(), Stroke::new(1.0, theme::border()), StrokeKind::Inside);
    let head = Rect::from_min_size(outer.min, Vec2::new(w, 30.0));
    list_header(ui.painter(), head, &cols, &["Name", "Questions", "Topics", "Edited", "Types"]);
    let mut hit = None;
    for (i, b) in rows.iter().enumerate() {
        let r = Rect::from_min_size(outer.min + Vec2::new(1.0, 30.0 + 54.0 * i as f32), Vec2::new(w - 2.0, 54.0));
        let resp = ui.interact(r, ui.id().with(("bkl", i)), egui::Sense::click());
        let p = ui.painter();
        if resp.hovered() {
            p.rect_filled(r, CornerRadius::ZERO, theme::hover_bg());
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if i + 1 < rows.len() {
            p.line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, theme::border()));
        }
        let cy = r.center().y;
        paint_bank_glyph(p, Rect::from_center_size(Pos2::new(r.min.x + 8.0 + 28.0, cy), Vec2::splat(34.0)));
        let at = |c: usize| Pos2::new(outer.min.x + cols[c].0, cy);
        paint_fit(p, at(1) - Vec2::new(0.0, 9.0), &b.name, serif_font(15.0), theme::ink(), cols[1].1);
        paint_fit(p, at(2) - Vec2::new(0.0, 8.0), &b.questions.to_string(), bold(13.0), theme::ink(), cols[2].1);
        paint_fit(p, at(3) - Vec2::new(0.0, 8.0), &b.topics.to_string(), sans(13.0), theme::text_col(), cols[3].1);
        paint_fit(p, at(4) - Vec2::new(0.0, 8.0), &rel_time(b.edited), sans(13.0), theme::text_col(), cols[4].1);
        crate::bankui::mix_bar_at(p, Rect::from_min_size(Pos2::new(outer.min.x + cols[5].0, cy - 2.5), Vec2::new(cols[5].1, 5.0)), &b.mix);
        if resp.clicked() {
            hit = Some(b.id.clone());
        }
    }
    hit
}

/// Printer glyph for the last-export strip.
fn draw_printer_icon(p: &egui::Painter, c: Pos2, col: Color32) {
    p.text(
        c,
        egui::Align2::CENTER_CENTER,
        egui_phosphor::regular::PRINTER,
        theme::icon_font(17.0),
        col,
    );
}

pub(crate) fn remap_empty_bank_ids(exams: &mut [Exam], bank_id: &str) {
    for exam in exams.iter_mut() {
        for item in exam.items.iter_mut() {
            if item.bank_id.is_empty() {
                item.bank_id = bank_id.to_string();
            }
        }
    }
}

pub(crate) fn mime_of(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        _ => "image/png",
    }
}

/// Rasterizes every page of a PDF (up to `MAX_PDF_PAGES`) to PNG with `pdftoppm`.
pub(crate) fn pdf_pages_png(data: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    use std::process::{Command, Stdio};
    const MAX_PDF_PAGES: usize = 30;

    let dir = std::env::temp_dir().join(format!("vershion-pdf-{}", now_ms()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let src = dir.join("in.pdf");
    std::fs::write(&src, data).map_err(|e| e.to_string())?;
    let out = Command::new("pdftoppm")
        .args(["-png", "-r", "150", "-l", &MAX_PDF_PAGES.to_string()])
        .arg(&src)
        .arg(dir.join("page"))
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .output();
    let result = match out {
        Ok(o) if o.status.success() => {
            let mut files: Vec<_> = std::fs::read_dir(&dir)
                .map(|d| d.flatten().map(|e| e.path()).collect())
                .unwrap_or_default();
            files.retain(|f| f.extension().is_some_and(|e| e == "png"));
            files.sort(); // pdftoppm zero-pads the page number, so name order is page order
            let pages: Vec<Vec<u8>> = files.iter().filter_map(|f| std::fs::read(f).ok()).collect();
            if pages.is_empty() { Err("pdftoppm produced no pages".to_string()) } else { Ok(pages) }
        }
        Ok(o) => Err(format!("pdftoppm failed: {}", String::from_utf8_lossy(&o.stderr).trim())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(
            "PDF rasterizer (pdftoppm) not found — export the pages as images instead".to_string(),
        ),
        Err(e) => Err(e.to_string()),
    };
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// Guard for restore entries: only the five JSON docs plus exports/ and media/.
fn sanitize_backup_entry(name: &str) -> bool {
    if name.contains("..") || name.starts_with('/') || name.contains('\\') {
        return false;
    }
    matches!(
        name,
        "banks.json" | "exams.json" | "imports.json" | "exports.json" | "settings.json"
    ) || name.starts_with("exports/")
        || name.starts_with("media/")
}

/// Zips the whole store dir into `dest`.
fn write_backup(dest: &std::path::Path) -> Result<(), String> {
    use std::io::Write as _;

    let dir = store::store_dir();
    if !dir.exists() {
        return Err(format!("{} does not exist", dir.display()));
    }
    let file = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    let writer = std::io::BufWriter::new(file);
    let mut zip = zip::ZipWriter::new(writer);
    let options = zip::write::SimpleFileOptions::default();
    let mut wrote_any = false;
    for name in
        ["banks.json", "exams.json", "imports.json", "exports.json", "settings.json"]
    {
        if let Ok(data) = std::fs::read(dir.join(name)) {
            zip.start_file(name, options).map_err(|e| e.to_string())?;
            zip.write_all(&data).map_err(|e| e.to_string())?;
            wrote_any = true;
        }
    }
    for sub in ["exports", "media"] {
        let Ok(entries) = std::fs::read_dir(dir.join(sub)) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if path.is_file() {
                if let Ok(data) = std::fs::read(&path) {
                    zip.start_file(format!("{sub}/{name}"), options).map_err(|e| e.to_string())?;
                    zip.write_all(&data).map_err(|e| e.to_string())?;
                    wrote_any = true;
                }
            }
        }
    }
    zip.finish().map_err(|e| e.to_string())?;
    if !wrote_any {
        return Err("the store is empty — nothing to back up".to_string());
    }
    Ok(())
}

/// Replaces the store dir contents with a backup zip's files; returns file count.
fn read_backup(path: &std::path::Path) -> Result<usize, String> {
    use std::io::Read as _;

    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut archive =
        zip::ZipArchive::new(std::io::BufReader::new(file)).map_err(|e| e.to_string())?;
    let dir = store::store_dir();
    let mut count = 0;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_string();
        if entry.is_dir() || !sanitize_backup_entry(&name) {
            continue;
        }
        let mut data = Vec::new();
        entry.read_to_end(&mut data).map_err(|e| e.to_string())?;
        let dest = dir.join(&name);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(dest, data).map_err(|e| e.to_string())?;
        count += 1;
    }
    if count == 0 {
        return Err("nothing usable found in the backup".to_string());
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every Home exam card is the sum of its slots (spec section 3), whatever its text.
    #[test]
    fn a_home_exam_card_holds_two_name_and_two_bank_lines() {
        // The longest text (two name lines, two bank lines) still clears the pinned footer.
        let text = 13.0 + 44.0 + 4.0 + 18.0 + 2.0 + 36.0;
        let footer = 7.0 + 1.0 + 7.0 + 18.0 + 13.0;
        assert!(text + footer <= HOME_EXAM_H + 0.01);
        assert_eq!(HOME_EXAM_H, 164.0);
        // At 1280 the text column is 316 - 124 - 16 = 176 wide.
        assert_eq!(exam_card_text_x(316.0), (124.0, 176.0));
    }

    /// A compact recent-exam row (narrow Home) is the sum of its lines, and three of them are
    /// shorter than one row of the two-column cards they replace.
    #[test]
    fn a_compact_home_exam_row_is_its_lines() {
        assert_eq!(10.0 + crate::kit::ty::CARD_LINE + 18.0 + 18.0 + 8.0, HOME_ROW_H);
        assert!(HOME_ROW_THUMB.y + 8.0 <= HOME_ROW_H - 16.0, "the thumb sits inside its strip");
        assert!(3.0 * HOME_ROW_H + 2.0 < 2.0 * HOME_EXAM_H + 16.0);
    }

    #[test]
    fn edited_label_reads_as_a_sentence() {
        assert_eq!(edited_label(0), "Edited not yet");
        assert_eq!(edited_label(now_secs()), "Edited just now");
        assert!(edited_label(now_secs() - 2 * 3600).starts_with("Edited 2 hours"));
        let old = edited_label(1_760_659_200);
        assert_eq!(old, "Edited Oct 17");
    }

    #[test]
    fn epoch_start_formats() {
        assert_eq!(fmt_time(0), "1970-01-01 00:00");
    }

    #[test]
    fn known_timestamps() {
        // 1,000,000,001 s epoch — the famous billion-second bash.
        assert_eq!(fmt_time(1_000_000_001), "2001-09-09 01:46");
        // 2009-02-13 23:31:30 UTC.
        assert_eq!(fmt_time(1_234_567_890), "2009-02-13 23:31");
        // Leap day 2024.
        assert_eq!(fmt_time(1_709_164_800), "2024-02-29 00:00");
    }

    #[test]
    fn backup_entry_guard() {
        assert!(sanitize_backup_entry("banks.json"));
        assert!(sanitize_backup_entry("exports/abc.json"));
        assert!(sanitize_backup_entry("media/12ab.png"));
        assert!(!sanitize_backup_entry("../etc/passwd"));
        assert!(!sanitize_backup_entry("/etc/passwd"));
        assert!(!sanitize_backup_entry("C:\\Windows"));
        assert!(!sanitize_backup_entry("random.txt"));
    }

    #[test]
    fn backup_never_includes_or_restores_the_key_file() {
        // Restore guard refuses it, so a malicious/old zip can't clobber saved keys.
        assert!(!sanitize_backup_entry("ai-keys.json"));
        assert!(!sanitize_backup_entry("ai-keys.json.tmp"));
        // Backup writer works from a fixed file list that must not name it.
        let src = include_str!("screens.rs");
        let a = src.find("fn write_backup").unwrap();
        let body = &src[a..a + 900];
        assert!(!body.contains(concat!("ai-", "keys")));
    }

    #[test]
    fn package_exam_items_remap_onto_the_new_bank() {
        let bank_id = "b123".to_string();
        let mut exams = vec![
            Exam { paper_style: Default::default(), margins: None,
                id: "e1".into(),
                name: "Quiz".into(),
                header: String::new(),
                title: None,
                items: vec![
                    ExamItem::new("", "q1"),
                    ExamItem::new("other-bank", "q2"),
                ],
                heading_pt: 16.0,
                body_pt: 11.0,
                margin_mm: 18.0,
                ruled_blanks: 0,
                modified: 0,
            },
        ];
        remap_empty_bank_ids(&mut exams, &bank_id);
        assert_eq!(exams[0].items[0].bank_id, "b123");
        // A non-empty bank_id is never touched.
        assert_eq!(exams[0].items[1].bank_id, "other-bank");
        assert_eq!(exams[0].items[1].question_id, "q2"); // ids pass through verbatim
    }

    #[test]
    fn issue_links_are_percent_encoded() {
        let u = issue_url(0, "Caret loss & tabs", "Line one\nLine two");
        assert!(u.starts_with("https://github.com/shuff57/verSHion/issues/new?title="));
        assert!(u.contains("%20") && u.contains("%0A") && !u.contains(' '));
    }
}
/// Padlock glyph for the dock's "Saved on this computer" note.

#[cfg(test)]
mod initials_tests {
    use super::initials_of;

    #[test]
    fn a_two_word_name_gives_two_letters() {
        assert_eq!(initials_of("Steven Huff", "shuff57@gmail.com"), "SH");
        assert_eq!(initials_of("  maria  de la cruz ", "m@x.org"), "MC");
    }

    #[test]
    fn one_word_gives_one_letter_and_no_name_falls_back_to_the_email() {
        assert_eq!(initials_of("Deycilopez", "lopezdeyci60@gmail.com"), "D");
        assert_eq!(initials_of("", "lopezdeyci60@gmail.com"), "L");
        assert_eq!(initials_of("   ", "9lives@x.org"), "9");
        assert_eq!(initials_of("", ""), "?");
    }

    #[test]
    fn punctuation_only_words_are_skipped() {
        assert_eq!(initials_of("- Ana -", "a@b.c"), "A");
        assert_eq!(initials_of("Ana - Ruiz", "a@b.c"), "AR");
    }
}
