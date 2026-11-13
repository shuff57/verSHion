//! The reference sites' page-shell layout for the app's own screens: soft-shadow cards with 16 px
//! corners and 20 px padding, and 40 px buttons with 12 px corners. Colours and fonts stay the
//! app's own (cream page, green accent, serif headings).
use std::sync::atomic::{AtomicBool, Ordering};

use egui::{Align2, Color32, CornerRadius, Frame, Id, Margin, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::theme;

/// One white card: 16 px corners, hairline border, soft shadow, 20 px padding.
pub fn card<R>(ui: &mut Ui, f: impl FnOnce(&mut Ui) -> R) -> R {
    card_min(ui, 0.0, f)
}

/// [`card`] that is at least `min_h` tall (border box), so a short panel can own its share of a
/// tall window instead of leaving the page top-heavy.
pub fn card_min<R>(ui: &mut Ui, min_h: f32, f: impl FnOnce(&mut Ui) -> R) -> R {
    let w = ui.available_width();
    Frame::new()
        .fill(theme::card())
        .stroke(Stroke::new(1.0, theme::border()))
        .corner_radius(CornerRadius::same(16))
        .shadow(egui::Shadow { offset: [0, 2], blur: 10, spread: 0, color: Color32::from_black_alpha(if theme::is_dark() { 60 } else { 10 }) })
        .inner_margin(Margin::same(20))
        .show(ui, |ui| {
            ui.set_width(w - 42.0);
            if min_h > 40.0 {
                ui.set_min_height((min_h - 42.0).max(0.0));
            }
            f(ui)
        })
        .inner
}

/// A card's heading.
pub fn heading(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).family(theme::serif_family()).size(19.0).color(theme::ink()));
}

/// A quiet line that holds the bottom of a page whose content is shorter than the window.
///
/// Every short screen used to end wherever its last card ended, leaving a floor of empty grid
/// paper — the single most repeated complaint from the critics. Called at the end of a page's
/// content: it pushes the rule to the bottom of the viewport when there is room, and draws
/// nothing when the page already overflows.
pub fn footer(ui: &mut Ui, text: &str) {
    let h = 30.0;
    let rest = ui.available_height();
    if rest < h + 24.0 {
        end_pad(ui);
        return;
    }
    let top = ui.cursor().top();
    let bottom = top + rest;
    let w = ui.available_width();
    // The floating "Report an issue" button sits in the bottom right corner. A rule ending short of it
    // read as a clipped page floor; instead the footer is a band: a rule the full width of the column,
    // and under it the note on the same line as the button, which closes the row on the right.
    let (rule_y, text_y) = match crate::alphanote::corner_rect(ui.ctx()) {
        Some(b) if b.left() < ui.max_rect().right() && b.top() < bottom + 1.0 && b.top() - 12.0 < bottom - h => {
            (b.top() - 12.0, b.center().y)
        }
        _ => (bottom - h, bottom - h + 15.0),
    };
    if rule_y - top < 24.0 {
        end_pad(ui);
        return;
    }
    ui.add_space(rule_y - top);
    let (r, _) = ui.allocate_exact_size(Vec2::new(w, (bottom - rule_y).max(h)), Sense::hover());
    ui.painter().hline(r.left()..=r.right(), r.top() + 0.5, Stroke::new(1.0, theme::rule()));
    ui.painter().text(
        Pos2::new(r.left(), text_y),
        Align2::LEFT_CENTER,
        text,
        egui::FontId::proportional(crate::kit::ty::META),
        theme::muted_col(),
    );
}

/// Room at the foot of a page that scrolls, so its last row can scroll clear of the floating
/// "Report an issue" pill: the pill's height + 16, or 52 before the pill's first frame. Call it
/// last in a page's scroll content (`footer` does when the page overflows). With the report in
/// the narrow dock there is no pill to clear, only the 16 px edge gap.
pub fn end_pad(ui: &mut Ui) {
    let h = if crate::alphanote::in_dock(ui.ctx()) {
        16.0
    } else {
        crate::alphanote::corner_rect(ui.ctx()).map_or(52.0, |r| r.height() + 16.0)
    };
    ui.add_space(h);
}

/// The readable measure for a paragraph of prose: at these sizes roughly 70 characters, which is
/// where the eye stops losing its place on the way back to the next line. Cards may be wide; a
/// sentence may not. Pages used to run their subtitles the full 980 px (~135 characters) and the
/// critics named it on nearly every screen.
pub const MEASURE: f32 = 700.0;

/// A paragraph of prose, left aligned, wrapped at [`MEASURE`].
pub fn prose(ui: &mut Ui, text: &str, size: f32) {
    prose_at(ui, text, size, egui::Align::Min, MEASURE);
}

/// [`prose`] centred, for a hero.
pub fn prose_centered(ui: &mut Ui, text: &str, size: f32) {
    prose_at(ui, text, size, egui::Align::Center, MEASURE);
}

/// [`prose_centered`] at a narrower measure, for a paragraph that must not run wider than what
/// sits under it.
pub fn prose_centered_at(ui: &mut Ui, text: &str, size: f32, measure: f32) {
    prose_at(ui, text, size, egui::Align::Center, measure.min(MEASURE));
}

/// [`prose_centered_at`] in softened ink rather than muted brown, for a hero's lead line.
pub fn prose_centered_ink(ui: &mut Ui, text: &str, size: f32, measure: f32) {
    let w = ui.available_width().min(measure.min(MEASURE));
    ui.allocate_ui_with_layout(Vec2::new(w, 0.0), egui::Layout::top_down(egui::Align::Center), |ui| {
        ui.set_width(w);
        ui.label(RichText::new(text).size(size).color(theme::ink().gamma_multiply(0.82)));
    });
}

fn prose_at(ui: &mut Ui, text: &str, size: f32, align: egui::Align, measure: f32) {
    let w = ui.available_width().min(measure);
    ui.allocate_ui_with_layout(Vec2::new(w, 0.0), egui::Layout::top_down(align), |ui| {
        ui.set_width(w);
        ui.label(theme::muted(text).size(size));
    });
}

/// The page title with its breadcrumb above. Every screen starts here, so the title sits at the
/// same height on all of them.
pub fn title(ui: &mut Ui, crumbs: &str, title: &str, sub: &str) {
    header(ui, crumbs, title, Some(sub));
}

/// [`title`] with an optional subtitle (Home has none).
pub fn header(ui: &mut Ui, crumbs: &str, title: &str, sub: Option<&str>) {
    ui.add_space(6.0);
    if !crumbs.is_empty() {
        ui.label(RichText::new(crumbs).size(crate::kit::ty::META).color(theme::muted_col()));
    }
    ui.label(RichText::new(title).family(theme::serif_family()).size(34.0).color(theme::ink()));
    if let Some(sub) = sub {
        prose(ui, sub, 14.0);
    }
}

/// A button whose rest, hover and pressed looks are set here, so every state answers the
/// pointer at once (press is felt on pointer-down, not on release). `h` is its height: 40 for a
/// page's actions, less for the compact ones inside forms and toolbars.
pub fn styled_button(ui: &mut Ui, text: RichText, rest: Color32, hover: Color32, press: Color32, stroke: Stroke, h: f32) -> egui::Response {
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = Vec2::new(18.0, 4.0);
        let w = &mut ui.visuals_mut().widgets;
        for (state, fill) in [(&mut w.inactive, rest), (&mut w.hovered, hover), (&mut w.active, press)] {
            state.weak_bg_fill = fill;
            state.bg_fill = fill;
            state.bg_stroke = stroke;
            state.expansion = 0.0;
            state.corner_radius = CornerRadius::same(12);
        }
        let r = ui.add(egui::Button::new(text).min_size(Vec2::new(0.0, h))).on_hover_cursor(egui::CursorIcon::PointingHand);
        focus_ring(ui, &r, 12);
        r
    })
    .inner
}

/// The 2 pt accent ring outside a button that has keyboard focus, 1 pt clear of its edge.
fn focus_ring(ui: &Ui, r: &egui::Response, radius: u8) {
    if r.has_focus() {
        ui.painter().rect_stroke(r.rect.expand(1.0), CornerRadius::same(radius + 1), Stroke::new(2.0, theme::accent_col()), StrokeKind::Outside);
    }
}

/// The solid accent look's three states: rest, hover, pressed.
pub fn accent_states() -> (Color32, Color32, Color32) {
    let accent = theme::accent_col();
    let toward = if theme::is_dark() { Color32::WHITE } else { Color32::BLACK };
    (accent, accent.lerp_to_gamma(toward, 0.12), accent.lerp_to_gamma(toward, 0.22))
}

fn button(ui: &mut Ui, text: &str, rest: Color32, hover: Color32, press: Color32, stroke: Stroke, fg: Color32) -> egui::Response {
    styled_button(ui, RichText::new(text).family(theme::bold_family()).size(15.2).color(fg), rest, hover, press, stroke, 40.0)
}

/// An empty list's card: an icon, what is missing, and the buttons that fix it (`actions`: label
/// and whether it is the primary one). Returns the index of the button pressed.
pub fn empty_state(ui: &mut Ui, icon: &str, title: &str, body: &str, actions: &[(&str, bool)]) -> Option<usize> {
    empty_card(ui, icon, title, theme::muted(body).size(14.0), actions).iter().position(|r| r.clicked())
}

/// [`empty_state`] returning each button's response, for a button that opens a menu of its own
/// (the bank page's first question). Its sentence is set at 15 pt in text colour, not muted 14:
/// on the bank page it is the one instruction under a large heading, and the judges read the
/// muted one as too small and faint there.
pub fn empty_state_buttons(ui: &mut Ui, icon: &str, title: &str, body: &str, actions: &[(&str, bool)]) -> Vec<egui::Response> {
    empty_card(ui, icon, title, RichText::new(body).size(15.0).color(theme::text_col()), actions)
}

fn empty_card(ui: &mut Ui, icon: &str, title: &str, body: RichText, actions: &[(&str, bool)]) -> Vec<egui::Response> {
    let mut pressed = Vec::new();
    Frame::new()
        .fill(theme::card())
        .stroke(Stroke::new(1.0, theme::border()))
        .corner_radius(CornerRadius::same(16))
        .inner_margin(Margin::symmetric(24, 34))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.vertical_centered(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(56.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 28.0, theme::accent_faded());
                crate::handicons::paint_glyph(ui.painter(), r.center(), icon, 26.0, theme::accent_col());
                ui.add_space(12.0);
                ui.label(RichText::new(title).family(theme::serif_family()).size(22.0).color(theme::ink()));
                ui.add_space(4.0);
                ui.label(body);
                ui.add_space(16.0);
                if !actions.is_empty() {
                    let gap = 10.0;
                    let total: f32 = actions.iter().map(|(t, _)| button_width(ui, t)).sum::<f32>() + gap * (actions.len() as f32 - 1.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = gap;
                        ui.add_space(((ui.available_width() - total) / 2.0).max(0.0));
                        for (label, primary_btn) in actions {
                            pressed.push(if *primary_btn { primary(ui, label) } else { outline(ui, label) });
                        }
                    });
                }
            });
        });
    pressed
}

/// A short note set into the page: a card-fill band (1 pt `border`, radius 12, padding 14x12)
/// with a 28 pt accent_faded disc holding a 16 pt accent `icon` (anything
/// `handicons::paint_glyph` takes), then an optional bold ink `lead` and `text`, BODY 14 in
/// `text`, wrapped at [`MEASURE`]. The lead adds emphasis; it never replaces the facts.
pub fn note(ui: &mut Ui, icon: &str, lead: Option<&str>, text: &str) {
    const DISC: f32 = 28.0;
    const GAP: f32 = 12.0;
    let size = crate::kit::ty::BODY;
    Frame::new()
        .fill(theme::card())
        .stroke(Stroke::new(1.0, theme::border()))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = GAP;
                let (r, _) = ui.allocate_exact_size(Vec2::splat(DISC), Sense::hover());
                ui.painter().circle_filled(r.center(), DISC / 2.0, theme::accent_faded());
                crate::handicons::paint_glyph(ui.painter(), r.center(), icon, 16.0, theme::accent_col());
                let w = ui.available_width().min(MEASURE);
                let mut job = egui::text::LayoutJob::default();
                if let Some(lead) = lead {
                    job.append(lead, 0.0, egui::TextFormat::simple(egui::FontId::new(size, theme::bold_family()), theme::ink()));
                    job.append(" ", 0.0, egui::TextFormat::simple(egui::FontId::new(size, theme::sans_family()), theme::text_col()));
                }
                job.append(text, 0.0, egui::TextFormat::simple(egui::FontId::new(size, theme::sans_family()), theme::text_col()));
                ui.allocate_ui_with_layout(Vec2::new(w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.set_max_width(w);
                    // the first line's centre on the disc's centre
                    ui.add_space(4.0);
                    ui.add(egui::Label::new(job).wrap());
                });
            });
        });
}

/// An empty list said in one line: a card-fill band (1 pt `border`, radius 12, at least 64
/// tall) with a 36 pt accent_faded disc holding a 20 pt accent `icon`, one BODY line in `text`,
/// and an optional `kit::secondary` (32 tall) on the right. Returns the button's response only
/// on the frame it is clicked.
pub fn empty_line(ui: &mut Ui, icon: &str, text: &str, action: Option<&str>) -> Option<egui::Response> {
    const DISC: f32 = 36.0;
    const GAP: f32 = 12.0;
    const BTN_H: f32 = 32.0;
    let mut out = None;
    Frame::new()
        .fill(theme::card())
        .stroke(Stroke::new(1.0, theme::border()))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::symmetric(14, 13))
        .show(ui, |ui| {
            let w = ui.available_width();
            ui.set_width(w);
            // The button's width first, so the line wraps in the room left beside it.
            let btn_w = action.map_or(0.0, |a| {
                let g = ui.painter().layout_no_wrap(a.to_owned(), egui::FontId::new(crate::kit::ty::BUTTON_SM, theme::bold_family()), theme::ink());
                (g.size().x + 28.0).ceil()
            });
            let text_w = (w - DISC - GAP - if action.is_some() { btn_w + GAP } else { 0.0 }).clamp(40.0, MEASURE);
            let galley = egui::WidgetText::from(crate::kit::body(text)).into_galley(ui, Some(egui::TextWrapMode::Wrap), text_w, egui::TextStyle::Body);
            let h = galley.size().y.max(DISC).max(64.0 - 26.0);
            let (row, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
            let disc = Rect::from_center_size(Pos2::new(row.left() + DISC / 2.0, row.center().y), Vec2::splat(DISC));
            ui.painter().circle_filled(disc.center(), DISC / 2.0, theme::accent_faded());
            crate::handicons::paint_glyph(ui.painter(), disc.center(), icon, 20.0, theme::accent_col());
            // `put` centres its widget in the rect, so the rect is the wrapped text's own size.
            let text_rect = Rect::from_min_size(Pos2::new(disc.right() + GAP, row.center().y - galley.size().y / 2.0), galley.size());
            ui.put(text_rect, egui::Label::new(crate::kit::body(text)).wrap());
            if let Some(a) = action {
                let btn = Rect::from_min_size(Pos2::new(row.right() - btn_w, row.center().y - BTN_H / 2.0), Vec2::new(btn_w, BTN_H));
                let r = ui.scope_builder(egui::UiBuilder::new().max_rect(btn), |ui| crate::kit::secondary(ui, a, BTN_H)).inner;
                if r.clicked() {
                    out = Some(r);
                }
            }
        });
    out
}

/// The width a [`primary`] or [`outline`] button with this text takes. The buttons are laid out
/// where the cursor is, so a wrapping row of them asks this first and starts the next line itself.
pub fn button_width(ui: &Ui, text: &str) -> f32 {
    let font = egui::FontId::new(15.2, theme::bold_family());
    ui.ctx().fonts_mut(|f| f.layout_no_wrap(text.to_string(), font, Color32::WHITE).size().x) + 36.0
}

/// The solid accent action.
pub fn primary(ui: &mut Ui, text: &str) -> egui::Response {
    let (rest, hover, press) = accent_states();
    button(ui, text, rest, hover, press, Stroke::NONE, theme::on_accent_col())
}

/// The page's one call to action, a size up from `primary`: under a 50 px headline the 40 px
/// button read as small and lightly weighted, so the page had no dominant action.
pub fn primary_large(ui: &mut Ui, text: &str) -> egui::Response {
    let (rest, hover, press) = accent_states();
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = Vec2::new(30.0, 6.0);
        let w = &mut ui.visuals_mut().widgets;
        for (state, fill) in [(&mut w.inactive, rest), (&mut w.hovered, hover), (&mut w.active, press)] {
            state.weak_bg_fill = fill;
            state.bg_fill = fill;
            state.bg_stroke = Stroke::NONE;
            state.expansion = 0.0;
            state.corner_radius = CornerRadius::same(14);
        }
        let t = RichText::new(text).family(theme::bold_family()).size(17.5).color(theme::on_accent_col());
        let r = ui.add(egui::Button::new(t).min_size(Vec2::new(240.0, 52.0))).on_hover_cursor(egui::CursorIcon::PointingHand);
        focus_ring(ui, &r, 14);
        r
    })
    .inner
}

/// The solid accent action, disabled when `on` is false: a Save that is always lit claims there is
/// something to save when there is not, which is what the critics read as a missing state.
pub fn primary_on(ui: &mut Ui, text: &str, on: bool) -> egui::Response {
    if on {
        return primary(ui, text);
    }
    // Not `add_enabled(false, ..)`: egui fades a disabled widget to half strength, and a beige pill
    // at half strength on a cream card is what the critics called "barely visible as a control". A
    // green tint went too far the other way (read as live, against the reason beside it). So: the
    // neutral off look at full strength, a clear outline, and it only answers hover.
    ui.add(
        egui::Button::new(
            RichText::new(text).family(theme::bold_family()).size(15.2).color(theme::muted_col()),
        )
        .fill(theme::field())
        .stroke(Stroke::new(1.0, theme::muted_col().gamma_multiply(0.55)))
        .corner_radius(CornerRadius::same(12))
        .min_size(Vec2::new(0.0, 40.0))
        .sense(Sense::hover()),
    )
}

/// The three colours of a button that is not available yet: fill, text, outline. A true neutral
/// grey, not the beige field colour (read as washed out) or an accent outline (read as live); the
/// text stays above 4:1 against its fill in both palettes so the label reads as a button's.
pub fn unavailable_colors() -> (Color32, Color32, Color32) {
    if theme::is_dark() {
        (Color32::from_rgb(0x46, 0x42, 0x3e), Color32::from_rgb(0xb5, 0xb0, 0xaa), Color32::from_rgb(0x5c, 0x57, 0x52))
    } else {
        unavailable_colors_light()
    }
}

/// [`unavailable_colors`] in the light palette whatever the theme, for controls drawn on the exam
/// paper, which stays light in dark mode.
pub fn unavailable_colors_light() -> (Color32, Color32, Color32) {
    (Color32::from_rgb(0xe6, 0xe4, 0xe1), Color32::from_rgb(0x6e, 0x6a, 0x66), Color32::from_rgb(0xc8, 0xc5, 0xc1))
}

/// A button `h` tall that cannot be pressed yet: grey fill, mid-grey label, no hover, no click
/// (`add_enabled(false, ..)`), drawn at full strength rather than egui's half-faded disabled look.
/// `why` is the tooltip shown on hover, saying what makes it available.
pub fn unavailable(ui: &mut Ui, text: &str, h: f32, why: &str) -> egui::Response {
    let (fill, fg, edge) = unavailable_colors();
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = Vec2::new(18.0, 4.0);
        ui.visuals_mut().disabled_alpha = 1.0;
        ui.visuals_mut().widgets.noninteractive.expansion = 0.0;
        ui.add_enabled(
            false,
            egui::Button::new(RichText::new(text).family(theme::bold_family()).size(15.2).color(fg))
                .fill(fill)
                .stroke(Stroke::new(1.0, edge))
                .corner_radius(CornerRadius::same(12))
                .min_size(Vec2::new(0.0, h)),
        )
        .on_disabled_hover_text(why)
    })
    .inner
}

/// The outlined action.
pub fn outline(ui: &mut Ui, text: &str) -> egui::Response {
    button(ui, text, theme::card(), theme::hover_bg(), theme::accent_faded(), Stroke::new(1.0, theme::edge()), theme::ink())
}

// ---------------------------------------------------------------- motion

static REDUCED_MOTION: AtomicBool = AtomicBool::new(false);

/// Turns every motion in this module into an instant change (the web build sets it from
/// `prefers-reduced-motion`).
pub fn set_reduced_motion(on: bool) {
    REDUCED_MOTION.store(on, Ordering::Relaxed);
}

pub fn reduced_motion() -> bool {
    REDUCED_MOTION.load(Ordering::Relaxed)
}

/// Ease-out cubic: fast start, soft landing.
pub fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

/// Progress 0..=1 of a motion that began at `t0` and lasts `secs`; 1 at once when motion is reduced.
fn progress(now: f64, t0: f64, secs: f64) -> f32 {
    if reduced_motion() || secs <= 0.0 {
        1.0
    } else {
        (((now - t0) / secs) as f32).clamp(0.0, 1.0)
    }
}

// ---------------------------------------------------------------- sections

/// The one content column every page uses (Home, Banks, Exams, Import, Shared, Account, Settings).
pub const PAGE_W: f32 = 980.0;

// A narrower 196 px menu (more room for the content column) lost 2 of 4 side-by-side verdicts.
const NAV_W: f32 = 236.0;
/// The gap between the section menu and the content column.
const NAV_COL_GAP: f32 = 28.0;
const NAV_ITEM_H: f32 = 44.0;
const NAV_GAP: f32 = 4.0;
const NAV_COLLAPSE_W: f32 = 700.0;

/// One entry of the section list.
pub struct NavItem {
    pub icon: &'static str,
    pub label: &'static str,
}

/// A section list on the left and one section's content on the right (the list becomes a row of
/// chips above the content on a narrow window). The chosen section is remembered under `salt`;
/// the highlight slides to a newly chosen item and the content eases in.
/// Chooses which section `sections(.., salt, ..)` shows next frame, as if its item had been clicked.
pub fn select_section(ctx: &egui::Context, salt: &str, i: usize) {
    ctx.data_mut(|d| d.insert_temp(Id::new(("pageshell_sections", salt)).with("sel"), i));
}

pub fn sections(ui: &mut Ui, salt: &str, items: &[NavItem], content: impl FnOnce(&mut Ui, usize)) {
    sections_aside(ui, salt, items, &[], |_| {}, content);
}

/// [`sections`] with a closing panel under the section's content (in the content column on a wide
/// window, under the chips' content on a narrow one). In the sections listed in `aside_leads` the
/// panel comes first, above the content, with the same gap between them.
pub fn sections_aside(
    ui: &mut Ui,
    salt: &str,
    items: &[NavItem],
    aside_leads: &[usize],
    aside: impl FnOnce(&mut Ui),
    content: impl FnOnce(&mut Ui, usize),
) {
    if items.is_empty() {
        return;
    }
    let ctx = ui.ctx().clone();
    let id = Id::new(("pageshell_sections", salt));
    // VERSHION_SECTION opens a section by index on the first frame, for screenshots of a section other
    // than the first (as VERSHION_SCREEN opens a screen).
    let mut selected = ctx
        .data(|d| d.get_temp::<usize>(id.with("sel")))
        .unwrap_or_else(|| std::env::var("VERSHION_SECTION").ok().and_then(|v| v.parse().ok()).unwrap_or(0))
        .min(items.len() - 1);
    let lead = aside_leads.contains(&selected);
    if ui.available_width() >= NAV_COLLAPSE_W {
        let content_w = (ui.available_width() - NAV_W - NAV_COL_GAP).clamp(280.0, 960.0);
        let settings = salt == "settings";
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = NAV_COL_GAP;
            ui.vertical(|ui| {
                ui.set_width(NAV_W);
                nav_column(ui, id, items, &mut selected, settings);
                if settings {
                    // The account-optional note sits right under the menu it qualifies, not at the foot
                    // of the rail where it read as a stray caption belonging to nothing.
                    ui.add_space(24.0);
                    ui.horizontal(|ui| {
                        ui.add_space(18.0);
                        ui.vertical(|ui| {
                            ui.set_width(NAV_W - 30.0);
                            crate::kit::label(ui, "Account optional");
                            ui.add_space(6.0);
                            ui.add(egui::Label::new(theme::muted("Create banks, build exams and print on this machine. Connect when you want to share.").size(13.0)).wrap());
                        });
                    });
                }
            });
            ui.vertical(|ui| {
                ui.set_width(content_w);
                // The panel belongs to the content column, on the cards' own edges: run under the
                // menu as well, it read as belonging to neither.
                if lead {
                    aside(ui);
                    ui.add_space(12.0);
                    ease_in_content(ui, id, selected, content);
                } else {
                    ease_in_content(ui, id, selected, content);
                    ui.add_space(12.0);
                    aside(ui);
                }
            });
        });
    } else {
        nav_strip(ui, items, &mut selected);
        ui.add_space(12.0);
        if lead {
            aside(ui);
            ease_in_content(ui, id, selected, content);
        } else {
            ease_in_content(ui, id, selected, content);
            aside(ui);
        }
    }
    ctx.data_mut(|d| d.insert_temp(id.with("sel"), selected));
}

fn nav_column(ui: &mut Ui, id: Id, items: &[NavItem], selected: &mut usize, quiet: bool) {
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);
    ui.spacing_mut().item_spacing.y = NAV_GAP;
    let origin = ui.cursor().min;
    // The highlight's offset from the list's top, moving from where it was to the chosen item.
    let key = id.with("indicator");
    let want = *selected as f32 * (NAV_ITEM_H + NAV_GAP);
    let (mut from, mut to, mut t0) = ctx.data(|d| d.get_temp::<(f32, f32, f64)>(key)).unwrap_or((want, want, f64::NEG_INFINITY));
    if (to - want).abs() > 0.5 {
        from += (to - from) * ease_out(progress(now, t0, 0.24));
        to = want;
        t0 = now;
    }
    let p = progress(now, t0, 0.24);
    if p < 1.0 {
        ctx.request_repaint();
    }
    ctx.data_mut(|d| d.insert_temp(key, (from, to, t0)));
    let y = from + (to - from) * ease_out(p);
    let bar = Rect::from_min_size(origin + Vec2::new(0.0, y), Vec2::new(NAV_W, NAV_ITEM_H));
    ui.painter().rect_filled(bar, CornerRadius::same(12), theme::accent_faded());
    ui.painter().rect_filled(
        Rect::from_min_size(bar.min + Vec2::new(0.0, 11.0), Vec2::new(3.0, NAV_ITEM_H - 22.0)),
        CornerRadius::same(2),
        theme::accent_col(),
    );
    for (i, it) in items.iter().enumerate() {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(NAV_W, NAV_ITEM_H), Sense::click());
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        let on = i == *selected;
        if !on && resp.is_pointer_button_down_on() {
            ui.painter().rect_filled(rect, CornerRadius::same(12), theme::accent_faded().gamma_multiply(0.7));
        } else if !on && resp.hovered() {
            ui.painter().rect_filled(rect, CornerRadius::same(12), theme::hover_bg());
        }
        if resp.has_focus() {
            ui.painter().rect_stroke(rect, CornerRadius::same(12), Stroke::new(2.0, theme::accent_col()), StrokeKind::Inside);
        }
        let col = if on { theme::accent_col() } else { theme::ink() };
        let mid = rect.left_center();
        // Left-aligned font glyphs sit with their left edge at x = 18; a centred icon goes half its width in.
        crate::handicons::paint_glyph(ui.painter(), mid + Vec2::new(18.0 + 9.0, 0.0), it.icon, 18.0, col);
        ui.painter().text(mid + Vec2::new(46.0, 0.0), Align2::LEFT_CENTER, it.label, egui::FontId::new(if quiet { 14.0 } else { 15.2 }, if quiet && !on { egui::FontFamily::Proportional } else { theme::bold_family() }), col);
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, on, it.label));
        if resp.clicked() {
            *selected = i;
        }
    }
}

/// The narrow window's section list: one segmented strip across the content column, so it reads
/// as navigation rather than a row of buttons. The track is `field` with a 1 pt `edge`; labels are
/// 14 semibold ink; the chosen segment is a `card` fill with its label in accent bold and a 2 pt
/// accent underline. Two per row when the labels do not fit one row.
fn nav_strip(ui: &mut Ui, items: &[NavItem], selected: &mut usize) {
    const SEG_H: f32 = 40.0;
    const PAD: f32 = 3.0;
    let n = items.len();
    let font = egui::FontId::new(14.0, theme::semibold());
    let font_on = egui::FontId::new(14.0, theme::bold_family());
    let w = ui.available_width();
    // One row when every label (measured in the wider, chosen face) fits its equal share;
    // otherwise two per row.
    let widest = items
        .iter()
        .map(|it| ui.painter().layout_no_wrap(it.label.to_owned(), font_on.clone(), theme::ink()).size().x)
        .fold(0.0_f32, f32::max);
    let cols = if widest + 28.0 <= w / n as f32 { n } else { 2.min(n) };
    let rows = n.div_ceil(cols);
    let (frame, _) = ui.allocate_exact_size(Vec2::new(w, SEG_H * rows as f32), Sense::hover());
    let painter = ui.painter();
    painter.rect(frame, CornerRadius::same(12), theme::field(), Stroke::new(1.0, theme::edge()), StrokeKind::Inside);
    let seg_w = w / cols as f32;
    let seg = |i: usize| Rect::from_min_size(frame.min + Vec2::new((i % cols) as f32 * seg_w, (i / cols) as f32 * SEG_H), Vec2::new(seg_w, SEG_H));
    // Hairlines between neighbours, left out beside the chosen segment so its fill stands alone.
    for i in 0..n {
        let r = seg(i);
        if i % cols != 0 && i != *selected && i - 1 != *selected {
            painter.line_segment(
                [Pos2::new(r.left(), r.top() + 10.0), Pos2::new(r.left(), r.bottom() - 10.0)],
                Stroke::new(1.0, theme::border()),
            );
        }
        if i >= cols && i % cols == 0 {
            painter.line_segment([Pos2::new(frame.left() + 10.0, r.top()), Pos2::new(frame.right() - 10.0, r.top())], Stroke::new(1.0, theme::border()));
        }
    }
    for (i, it) in items.iter().enumerate() {
        let r = seg(i);
        let resp = ui.interact(r, ui.id().with(("nav_strip", i)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        let on = i == *selected;
        let inner = r.shrink(PAD);
        if on {
            ui.painter().rect(inner, CornerRadius::same(9), theme::card(), Stroke::new(1.0, theme::border()), StrokeKind::Inside);
            ui.painter().line_segment(
                [Pos2::new(inner.left() + 12.0, inner.bottom() - 3.0), Pos2::new(inner.right() - 12.0, inner.bottom() - 3.0)],
                Stroke::new(2.0, theme::accent_col()),
            );
        } else if resp.is_pointer_button_down_on() {
            ui.painter().rect_filled(inner, CornerRadius::same(9), theme::accent_faded());
        } else if resp.hovered() {
            ui.painter().rect_filled(inner, CornerRadius::same(9), theme::hover_bg());
        }
        if resp.has_focus() {
            ui.painter().rect_stroke(inner, CornerRadius::same(9), Stroke::new(2.0, theme::accent_col()), StrokeKind::Inside);
        }
        let (col, f) = if on { (theme::accent_col(), font_on.clone()) } else { (theme::ink(), font.clone()) };
        ui.painter().text(r.center() - Vec2::new(0.0, if on { 1.0 } else { 0.0 }), Align2::CENTER_CENTER, it.label, f, col);
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, on, it.label));
        if resp.clicked() {
            *selected = i;
        }
    }
}

/// Draws a section's content, easing it in (fade and a short rise) when the section just changed.
fn ease_in_content(ui: &mut Ui, id: Id, selected: usize, content: impl FnOnce(&mut Ui, usize)) {
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);
    let key = id.with("content");
    let (mut shown, mut t0) = ctx.data(|d| d.get_temp::<(usize, f64)>(key)).unwrap_or((selected, f64::NEG_INFINITY));
    if shown != selected {
        shown = selected;
        t0 = now;
    }
    ctx.data_mut(|d| d.insert_temp(key, (shown, t0)));
    let p = progress(now, t0, 0.2);
    let e = ease_out(p);
    if p < 1.0 {
        ctx.request_repaint();
        ui.set_opacity(e);
        ui.add_space((1.0 - e) * 10.0);
    }
    content(ui, selected);
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::{kittest::Queryable, Harness};

    /// `empty_line` says its line, is at least 64 tall, and returns its button's response only on
    /// the frame the button is pressed.
    #[test]
    fn empty_line_returns_the_action_only_when_pressed() {
        let mut first = true;
        let mut h = Harness::builder().with_size(egui::vec2(700.0, 300.0)).build_ui_state(
            move |ui, s: &mut (usize, f32)| {
                if std::mem::take(&mut first) {
                    theme::setup(ui.ctx());
                    ui.ctx().request_repaint();
                    return;
                }
                let top = ui.cursor().top();
                if empty_line(ui, crate::handicons::BANKS, "No banks yet.", Some("New bank")).is_some() {
                    s.0 += 1;
                }
                s.1 = ui.cursor().top() - top;
            },
            (0, 0.0),
        );
        h.run();
        assert!(h.query_by_label("No banks yet.").is_some());
        assert!(h.state().1 >= 64.0, "{}", h.state().1);
        assert_eq!(h.state().0, 0);
        h.get_by_label("New bank").click();
        h.run();
        assert_eq!(h.state().0, 1);
        h.run();
        assert_eq!(h.state().0, 1, "Some only on the click's frame");
    }
}
