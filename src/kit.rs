//! V4 widget kit: the shared pills, headers and toggles every screen uses so the
//! chrome stays uniform ("Study Studio in paper tones"). Pure egui, painter-drawn.

use egui::{Color32, CornerRadius, FontId, Margin, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::theme;

/// Cards/List view choice shared by Home and the Bank screens.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum ViewMode {
    #[default]
    Cards,
    List,
}

/// Card radius (cards, tiles, popovers).
pub const R_CARD: u8 = 16;
/// Field radius.
pub const R_FIELD: u8 = 12;

fn pill_radius(h: f32) -> CornerRadius {
    CornerRadius::same((h / 2.0) as u8)
}

/// The one primary button: filled accent pill.
pub fn primary_button(ui: &mut Ui, label: &str) -> egui::Response {
    let (rest, hover, press) = crate::pageshell::accent_states();
    crate::pageshell::styled_button(
        ui,
        RichText::new(label).family(theme::bold_family()).color(theme::on_accent_col()),
        rest,
        hover,
        press,
        Stroke::NONE,
        36.0,
    )
}

/// The primary at full strength in both themes. On the dark page the house accent is a pale mint
/// that read as a weak, half-disabled button beside an outlined secondary; this fills it a
/// full-strength green with white type there instead (the light theme's primary is unchanged).
pub fn solid_primary(ui: &mut Ui, label: &str) -> egui::Response {
    if !theme::is_dark() {
        return primary_button(ui, label);
    }
    let fill = Color32::from_rgb(0x2f, 0x7a, 0x4b);
    crate::pageshell::styled_button(
        ui,
        RichText::new(label).family(theme::bold_family()).color(Color32::WHITE),
        fill,
        fill.lerp_to_gamma(Color32::WHITE, 0.10),
        fill.lerp_to_gamma(Color32::BLACK, 0.15),
        Stroke::NONE,
        36.0,
    )
}

/// [`solid_primary`] with a visible waiting state: a gated action (one that cannot be taken until a
/// required field is filled) must look unmistakably off. The waiting face is a flat muted fill with
/// no outline and a low-contrast label: distinct from the enabled primary (no accent at all) and
/// from a secondary (which is outlined, on the card's own fill, in ink). Pair it with
/// [`gated_reason`] directly under it, which names the missing step.
pub fn primary_button_on(ui: &mut Ui, label: &str, on: bool) -> egui::Response {
    if on {
        return solid_primary(ui, label);
    }
    let fill = theme::field().lerp_to_gamma(theme::rule(), if theme::is_dark() { 0.35 } else { 0.55 });
    let text = RichText::new(label).family(theme::bold_family()).color(theme::muted_col().gamma_multiply(0.85));
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = Vec2::new(18.0, 4.0);
        let w = &mut ui.visuals_mut().widgets;
        // a hover-only button is styled as "noninteractive" by egui, so that state gets the face too
        for state in [&mut w.noninteractive, &mut w.inactive, &mut w.hovered, &mut w.active] {
            state.weak_bg_fill = fill;
            state.bg_fill = fill;
            state.bg_stroke = Stroke::NONE;
            state.expansion = 0.0;
            state.corner_radius = CornerRadius::same(12);
        }
        ui.add(egui::Button::new(text).min_size(Vec2::new(0.0, 36.0)).sense(Sense::hover()))
            .on_hover_cursor(egui::CursorIcon::NotAllowed)
    })
    .inner
}

/// Warning ink: an amber-brown that reads as "something is missing" without the alarm of red, at
/// AA contrast on the card in both themes.
pub fn warn_col() -> Color32 {
    if theme::is_dark() { Color32::from_rgb(0xe8, 0xb8, 0x62) } else { Color32::from_rgb(0x8a, 0x55, 0x00) }
}

/// The one line under a waiting [`primary_button_on`] that says what it waits for ("Type your
/// password to send"), in warning ink with a warning mark, so the gate reads as a step to take.
/// Shown only while the step is missing.
pub fn gated_reason(ui: &mut Ui, reason: &str) -> egui::Response {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.label(RichText::new(egui_phosphor::regular::INFO).font(theme::icon_font(15.0)).color(warn_col()));
        ui.add(egui::Label::new(RichText::new(reason).family(theme::bold_family()).size(ty::META).color(warn_col())).wrap_mode(egui::TextWrapMode::Extend));
    })
    .response
}

/// A field that gates a button and is still empty wears an accent ring, so the eye goes to the step
/// that is missing. Call with the field's response after adding it.
pub fn attention_ring(ui: &Ui, field: &egui::Response) {
    ui.painter().rect_stroke(field.rect.expand(1.5), CornerRadius::same(R_FIELD), Stroke::new(1.5, theme::accent_col()), StrokeKind::Outside);
}

/// A quiet scroll viewport for a list that scrolls inside a page that also scrolls: the handle is
/// hidden until the pointer is over the region (or drags it), then floats thin over the content's
/// own right edge, inside the region. At rest the page shows one scroller per column, never a second
/// rail in the gutter; the region says it continues with a "more below" cue instead.
pub fn quiet_scroll_area<R>(ui: &mut Ui, area: egui::ScrollArea, add: impl FnOnce(&mut Ui) -> R) -> egui::scroll_area::ScrollAreaOutput<R> {
    ui.scope(|ui| {
        let saved = ui.style().clone();
        let s = &mut ui.spacing_mut().scroll;
        *s = egui::style::ScrollStyle::floating();
        s.bar_width = 6.0;
        s.floating_width = 4.0;
        s.bar_inner_margin = 3.0;
        s.foreground_color = false;
        s.bar_outer_margin = 5.0;
        // hidden at rest and while the pointer merely crosses the region: it shows on the bar's
        // own strip and while dragged
        s.dormant_handle_opacity = 0.0;
        s.active_handle_opacity = 0.0;
        s.dormant_background_opacity = 0.0;
        s.active_background_opacity = 0.0;
        s.interact_background_opacity = 0.0;
        let v = ui.visuals_mut();
        v.extreme_bg_color = Color32::TRANSPARENT;
        let handle = theme::muted_col().gamma_multiply(0.7);
        v.widgets.inactive.bg_fill = handle;
        v.widgets.hovered.bg_fill = theme::muted_col();
        v.widgets.active.bg_fill = theme::muted_col();
        for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active] {
            w.corner_radius = CornerRadius::same(3);
        }
        area.show(ui, |ui| {
            ui.set_style(saved);
            add(ui)
        })
    })
    .inner
}

/// A scroll bar the eye can find: a solid track (not the floating hairline that fades in only
/// under the pointer) with an ink handle, in both themes. Returns the colour to restore on
/// `extreme_bg_color` after the scroll area. Long-form panels used to look complete while half
/// their controls sat below the fold, because the bar was invisible until hovered.
pub fn visible_scroll(ui: &mut Ui) -> (Color32, egui::style::ScrollStyle) {
    let old = ui.visuals().extreme_bg_color;
    let old_style = ui.spacing().scroll.clone();
    ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
    ui.spacing_mut().scroll.bar_width = 10.0;
    // the handle takes the widget's text stroke, so it stays ink-dark on the light page and
    // cream on the dark one; the track is a tan step off the card so it reads as a channel
    ui.spacing_mut().scroll.foreground_color = true;
    ui.spacing_mut().scroll.fade.strength = 0.0;
    ui.spacing_mut().scroll.dormant_handle_opacity = 1.0;
    ui.visuals_mut().extreme_bg_color = theme::field().lerp_to_gamma(theme::rule(), 0.42);
    (old, old_style)
}

/// Put a scroll area's track colour and bar style back after [`visible_scroll`].
pub fn restore_scroll(ui: &mut Ui, old: (Color32, egui::style::ScrollStyle)) {
    ui.visuals_mut().extreme_bg_color = old.0;
    ui.spacing_mut().scroll = old.1;
}

/// "More below": the hairline and the quiet bar under a scroll viewport that really continues.
/// Drawn only when the content overflows, so it never claims hidden rows that are not there.
pub fn more_below(ui: &Ui, rect: Rect, label: &str) {
    let p = ui.painter();
    p.hline(rect.x_range(), rect.top() + 0.5, Stroke::new(1.0, theme::border()));
    let font = FontId::new(11.0, theme::bold_family());
    let g = p.layout_no_wrap(label.to_string(), font, theme::muted_col());
    let icon = p.layout_no_wrap(egui_phosphor::regular::CARET_DOWN.to_string(), theme::icon_font(14.0), theme::muted_col());
    let total = g.size().x + 7.0 + icon.size().x;
    let x = rect.center().x - total / 2.0;
    p.galley(Pos2::new(x, rect.center().y - g.size().y / 2.0 + 1.0), g, theme::muted_col());
    p.galley(Pos2::new(x + total - icon.size().x, rect.center().y - icon.size().y / 2.0 + 1.0), icon, theme::muted_col());
}

/// Ghost/outline button for every secondary action: the page buttons' look at the compact height.
pub fn ghost_button(ui: &mut Ui, label: &str) -> egui::Response {
    crate::pageshell::styled_button(
        ui,
        RichText::new(label).family(theme::bold_family()).color(theme::ink()),
        theme::card(),
        theme::hover_bg(),
        theme::accent_faded(),
        Stroke::new(1.0, theme::edge()),
        36.0,
    )
}

/// 0..=1, easing toward 1 while `on` (a card under the pointer) and back to 0 after, in 0.14 s;
/// instant when motion is reduced.
pub fn hover_t(ctx: &egui::Context, id: egui::Id, on: bool) -> f32 {
    let secs = if crate::pageshell::reduced_motion() { 0.0 } else { 0.14 };
    crate::pageshell::ease_out(ctx.animate_bool_with_time(id, on, secs))
}

/// The one search box: a 40 px field with a magnifier, 12 px corners like the buttons. True when
/// the text changed.
pub fn search_field(ui: &mut Ui, text: &mut String, w: f32, hint: &str) -> bool {
    search_field_with(ui, text, w, hint, None).changed()
}

/// `search_field` for a box that needs its text field's response (Enter, focus) or a fixed id.
pub fn search_field_with(ui: &mut Ui, text: &mut String, w: f32, hint: &str, id: Option<egui::Id>) -> egui::Response {
    let mut out = None;
    egui::Frame::new()
        .fill(theme::card())
        .stroke(Stroke::new(1.0, theme::border()))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::symmetric(13, 10))
        .show(ui, |ui| {
            ui.set_width(w - 28.0);
            // The row takes its height from the theme's 34 px control size, which would make the
            // box 54 px tall; the frame's padding supplies the height instead.
            ui.spacing_mut().interact_size.y = 20.0;
            // The placeholder in the muted tone, as the magnifier (5.5:1 on the light card, 5.2:1
            // on the dark), not egui's 60% text (3.0:1 and 4.1:1); typed text stays ink.
            ui.visuals_mut().weak_text_color = Some(theme::muted_col());
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(15.0), Sense::hover());
                let s = Stroke::new(1.6, theme::muted_col());
                ui.painter().circle_stroke(r.center() + Vec2::new(-1.0, -1.0), 5.0, s);
                ui.painter().line_segment([r.center() + Vec2::new(2.8, 2.8), r.center() + Vec2::new(6.0, 6.0)], s);
                let mut edit = egui::TextEdit::singleline(text)
                    .frame(egui::Frame::new())
                    .margin(Margin::ZERO)
                    .hint_text(hint)
                    .desired_width(ui.available_width());
                if let Some(id) = id {
                    edit = edit.id(id);
                }
                out = Some(ui.add(edit));
            });
        });
    out.expect("the search row always draws its field")
}

/// Danger text button (Delete).
pub fn danger_text_button(ui: &mut Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).family(theme::bold_family()).color(theme::bad()))
            .frame(false),
    )
}

/// Small-caps label: 10–12 px, bold, uppercase, letter-spaced look, muted.
pub fn small_caps(ui: &mut Ui, text: &str) {
    ui.label(small_caps_text(text));
}

/// [`small_caps`]'s text at 10.5 (ungraded screens keep it; graded ones use [`label_text`]).
pub fn small_caps_text(text: &str) -> RichText {
    caps_text(text, 10.5)
}

fn caps_text(text: &str, size: f32) -> RichText {
    // egui has no letter-spacing; thin spaces between letters approximate .09em.
    let spaced: String = text
        .to_uppercase()
        .chars()
        .flat_map(|c| [c, '\u{200A}'])
        .collect();
    RichText::new(spaced).family(theme::bold_family()).size(size).color(theme::muted_col())
}

/// Filled chip: pill with a wash fill and bold small label.
pub fn pill(ui: &mut Ui, label: &str, fill: Color32, fg: Color32) -> egui::Response {
    pill_with(ui, label, fill, fg, egui::TextWrapMode::Wrap)
}

/// The same chip with a label that never wraps. For a row whose line breaks are chosen by the
/// caller by measurement (`importui::chip_rows`): there a wrapping label would report its
/// squeezed width back to the frame and the chip would come out one letter wide.
pub fn pill_no_wrap(ui: &mut Ui, label: &str, fill: Color32, fg: Color32) -> egui::Response {
    pill_with(ui, label, fill, fg, egui::TextWrapMode::Extend)
}

fn pill_with(
    ui: &mut Ui,
    label: &str,
    fill: Color32,
    fg: Color32,
    wrap: egui::TextWrapMode,
) -> egui::Response {
    egui::Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(255))
        .inner_margin(Margin::symmetric(9, 2))
        .show(ui, |ui| {
            ui.add(
                egui::Label::new(RichText::new(label).family(theme::bold_family()).size(11.0).color(fg))
                    .wrap_mode(wrap),
            );
        })
        .response
}

/// Type pill (MC, TF, FILL…): accent wash with accent text.
pub fn type_pill(ui: &mut Ui, label: &str) {
    pill(ui, label, theme::accent_faded(), theme::accent_col());
}

/// Marker pill ("Needs AI", "Just added"): highlighter fill, dark ink text.
pub fn marker_pill(ui: &mut Ui, label: &str) {
    pill(ui, label, theme::highlighter(), theme::on_highlighter());
}

/// Outline chip used for filters/topics; `on` fills it with the accent wash.
pub fn chip_toggle(ui: &mut Ui, label: &str, on: bool) -> egui::Response {
    let fill = if on { theme::accent_faded() } else { Color32::TRANSPARENT };
    let stroke = if on { Stroke::new(1.0, theme::accent_col()) } else { Stroke::new(1.0, theme::border()) };
    let fg = if on { theme::accent_col() } else { theme::text_col() };
    ui.add(
        egui::Button::new(RichText::new(label).family(theme::bold_family()).size(12.0).color(fg))
            .fill(fill)
            .stroke(stroke)
            .corner_radius(CornerRadius::same(255))
            .min_size(Vec2::new(0.0, 26.0)),
    )
}

/// Cards/List two-segment pill. Returns true when the mode changed.
pub fn view_toggle(ui: &mut Ui, mode: &mut ViewMode, salt: &str) -> bool {
    let size = Vec2::new(72.0, 30.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect(rect, pill_radius(30.0), theme::card(), Stroke::new(1.0, theme::border()), StrokeKind::Inside);
    let mut changed = false;
    for (i, m) in [ViewMode::Cards, ViewMode::List].into_iter().enumerate() {
        let seg = Rect::from_min_size(rect.min + Vec2::new(i as f32 * 36.0, 0.0), Vec2::new(36.0, 30.0)).shrink(2.0);
        let resp = ui.interact(seg, ui.id().with((salt, "view_seg", i)), Sense::click());
        let on = *mode == m;
        if on {
            ui.painter().rect_filled(seg, pill_radius(26.0), theme::accent_faded());
        } else if resp.hovered() {
            ui.painter().rect_filled(seg, pill_radius(26.0), theme::hover_bg());
        }
        let col = if on { theme::accent_col() } else { theme::muted_col() };
        let c = seg.center();
        if i == 0 {
            for (dx, dy) in [(-3.5, -3.5), (3.5, -3.5), (-3.5, 3.5), (3.5, 3.5)] {
                ui.painter().rect_filled(Rect::from_center_size(c + Vec2::new(dx, dy), Vec2::splat(5.0)), CornerRadius::same(1), col);
            }
        } else {
            for dy in [-4.5, 0.0, 4.5] {
                ui.painter().line_segment([c + Vec2::new(-6.0, dy), c + Vec2::new(6.0, dy)], Stroke::new(1.8, col));
            }
        }
        if resp.clicked() && !on {
            *mode = m;
            changed = true;
        }
    }
    changed
}

/// Screen header: small breadcrumb line, then a serif title (34–40 px) and an
/// optional muted subtitle. Replaces the old top bar.
pub fn page_header(ui: &mut Ui, crumbs: &[&str], title: &str, subtitle: Option<&str>) {
    crate::pageshell::header(ui, &crumbs.join("  /  "), title, subtitle);
}

/// Card frame: radius 16, 1 px border, no heavy shadow.
pub fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(theme::card())
        .stroke(Stroke::new(1.0, theme::border()))
        .corner_radius(CornerRadius::same(R_CARD))
}

/// Round answer-correct circle: filled accent with a check when `on`.
pub fn check_circle(ui: &mut Ui, on: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
    let c = rect.center();
    if on {
        ui.painter().circle_filled(c, 10.0, theme::accent_col());
        let s = Stroke::new(2.0, theme::on_accent_col());
        ui.painter().line_segment([c + Vec2::new(-4.5, 0.5), c + Vec2::new(-1.5, 3.5)], s);
        ui.painter().line_segment([c + Vec2::new(-1.5, 3.5), c + Vec2::new(4.5, -3.0)], s);
    } else {
        ui.painter().circle_stroke(c, 10.0, Stroke::new(1.5, if resp.hovered() { theme::accent_col() } else { theme::rule() }));
    }
    resp
}

/// Text at a pt-size on the exam page etc. is the caller's business; this just
/// builds a UI-font id used by dock/labels.
pub fn ui_font(size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Proportional)
}

/// Used by painter code that needs a centered label.
pub fn center_text(ui: &Ui, pos: Pos2, text: &str, size: f32, col: Color32) {
    ui.painter().text(pos, egui::Align2::CENTER_CENTER, text, ui_font(size), col);
}

// ---------------------------------------------------------------- type scale

/// The type scale of the graded screens, in egui points (spec section 2). Call sites name a role
/// here instead of typing a size. Theme `Body` stays 13 for egui's own widgets in dialogs.
pub mod ty {
    /// Welcome headline (serif, ink).
    pub const DISPLAY: f32 = 43.0;
    /// Welcome headline below the narrow breakpoint.
    pub const DISPLAY_NARROW: f32 = 40.0;
    /// Welcome lead line (sans, ink x0.82).
    pub const HERO_LEAD: f32 = 16.5;
    /// Welcome lead line, narrow.
    pub const HERO_LEAD_NARROW: f32 = 15.0;
    /// Page title (serif, ink).
    pub const TITLE: f32 = 34.0;
    /// Section heading, stat values, step numerals (serif, ink).
    pub const SECTION: f32 = 22.0;
    /// Panel / tile heading (serif, ink).
    pub const PANEL: f32 = 19.0;
    /// Card name (serif, ink), on a [`CARD_LINE`] line.
    pub const CARD: f32 = 17.0;
    /// Line height reserved per line of a [`CARD`] name.
    pub const CARD_LINE: f32 = 22.0;
    /// Lead / subtitle prose (sans, muted).
    pub const LEAD: f32 = 14.0;
    /// Body prose (sans, text).
    pub const BODY: f32 = 14.0;
    /// Meta lines, status words, links (sans).
    pub const META: f32 = 13.0;
    /// Section labels, badges, pills: bold caps, muted. The only role at the floor.
    pub const LABEL: f32 = 12.0;
    /// Nothing on a graded screen is smaller (dock and printed content are exempt).
    pub const FLOOR: f32 = 12.0;
    /// Button text on a 36-40 tall button.
    pub const BUTTON: f32 = 15.2;
    /// Button text on a compact (under 38 tall) button.
    pub const BUTTON_SM: f32 = 13.5;
    /// The page's one large call to action (`pageshell::primary_large`).
    pub const BUTTON_LG: f32 = 17.5;
}

/// The gap between cards on the grids this pass touches (Banks, Home banks).
pub const GUTTER: f32 = 16.0;
/// A card's inner padding on those grids.
pub const CARD_PAD: f32 = 16.0;

/// Body prose: 14 pt, `text` colour.
pub fn body(t: impl Into<String>) -> RichText {
    RichText::new(t).size(ty::BODY).color(theme::text_col())
}

/// Lead / subtitle prose: 14 pt, muted (on card or cream only).
pub fn lead(t: impl Into<String>) -> RichText {
    RichText::new(t).size(ty::LEAD).color(theme::muted_col())
}

/// A meta line ("11 questions · 4 topics") or status word ("Saved"): 13 pt, `text` colour.
pub fn meta(t: impl Into<String>) -> RichText {
    RichText::new(t).size(ty::META).color(theme::text_col())
}

/// The quiet meta line ("Edited 23 days ago"): 13 pt, muted (on card or cream only).
pub fn meta_muted(t: impl Into<String>) -> RichText {
    RichText::new(t).size(ty::META).color(theme::muted_col())
}

/// The stressed meta word (a bank name after "From"): 13 pt semibold, `text` colour.
pub fn meta_strong(t: impl Into<String>) -> RichText {
    RichText::new(t).size(ty::META).family(theme::semibold()).color(theme::text_col())
}

/// A section label: 12 pt bold caps, hair-spaced, muted. Names a section, never a fact on a card.
pub fn label_text(t: &str) -> RichText {
    caps_text(t, ty::LABEL)
}

/// [`label_text`] as a label (the graded screens' replacement for [`small_caps`]).
pub fn label(ui: &mut Ui, t: &str) {
    ui.label(label_text(t));
}

/// A card name's font: serif 17 (lay it out on [`ty::CARD_LINE`] lines).
pub fn card_title_font() -> FontId {
    FontId::new(ty::CARD, theme::serif_family())
}

// ---------------------------------------------------------------- button tiers

/// One tier's rest look: fill, outline, label colour and face.
struct Tier {
    fill: Color32,
    stroke: Stroke,
    fg: Color32,
    family: egui::FontFamily,
}

/// The 2 pt accent ring outside a focused control, 1 pt clear of its edge.
fn focus_ring(ui: &Ui, rect: Rect, radius: u8) {
    ui.painter().rect_stroke(rect.expand(1.0), CornerRadius::same(radius.saturating_add(1)), Stroke::new(2.0, theme::accent_col()), StrokeKind::Outside);
}

/// True while the popup anchored on `resp` (`egui::Popup::menu(&resp)`) is open.
fn popup_open(ui: &Ui, resp: &egui::Response) -> bool {
    egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(resp))
}

/// A painted text button of one tier, `h` tall: hover_bg under the pointer, accent_faded while
/// pressed, a focus ring, PointingHand, Enter/Space (egui's click sense), and its label as the
/// accessible name. Inside a disabled `Ui` it takes the `unavailable` look and is not focusable.
fn tier_button(ui: &mut Ui, icon: Option<&str>, label: &str, h: f32, tier: Tier) -> egui::Response {
    let enabled = ui.is_enabled();
    let (size, pad) = if h >= 38.0 { (ty::BUTTON, 18.0) } else { (ty::BUTTON_SM, 14.0) };
    let (fill, stroke, fg) = if enabled {
        (tier.fill, tier.stroke, tier.fg)
    } else {
        let (f, t, e) = crate::pageshell::unavailable_colors();
        (f, Stroke::new(1.0, e), t)
    };
    let family = if enabled { tier.family } else { theme::bold_family() };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), FontId::new(size, family), fg);
    let (icon_w, icon_gap) = if icon.is_some() { (16.0, 7.0) } else { (0.0, 0.0) };
    let w = pad * 2.0 + icon_w + icon_gap + galley.size().x;
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w.ceil(), h), sense);
    let bg = if !enabled {
        fill
    } else if resp.is_pointer_button_down_on() || popup_open(ui, &resp) {
        theme::accent_faded()
    } else if resp.hovered() {
        theme::hover_bg()
    } else {
        fill
    };
    let r = CornerRadius::same(12);
    ui.painter().rect(rect, r, bg, stroke, StrokeKind::Inside);
    let mut x = rect.left() + pad;
    if let Some(icon) = icon {
        crate::handicons::paint_glyph(ui.painter(), Pos2::new(x + icon_w / 2.0, rect.center().y), icon, 16.0, fg);
        x += icon_w + icon_gap;
    }
    ui.painter().galley(Pos2::new(x, rect.center().y - galley.size().y / 2.0), galley, fg);
    if resp.has_focus() {
        focus_ring(ui, rect, 12);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}

/// Secondary tier, `h` tall (32 on cards, 40 in toolbars): card fill, 1 pt [`theme::edge`], ink
/// bold label (15.2 at 38+ tall, else 13.5).
pub fn secondary(ui: &mut Ui, label: &str, h: f32) -> egui::Response {
    tier_button(ui, None, label, h, Tier { fill: theme::card(), stroke: Stroke::new(1.0, theme::edge()), fg: theme::ink(), family: theme::bold_family() })
}

/// A card's main action: card fill, 1.5 pt accent outline, accent bold label. Never filled; the
/// page region's one filled primary stays `pageshell::primary`. (The Exams grid's Open is one step
/// stronger, a tinted fill: `examui::exams_open_button`.)
pub fn secondary_accent(ui: &mut Ui, label: &str, h: f32) -> egui::Response {
    tier_button(ui, None, label, h, Tier { fill: theme::card(), stroke: Stroke::new(1.5, theme::accent_col()), fg: theme::accent_col(), family: theme::bold_family() })
}

/// Tertiary tier (Details, Preview, How it works): no fill, 1 pt [`theme::edge`], a 16 pt ink
/// `icon` (anything `handicons::paint_glyph` takes), then the label in ink semibold.
pub fn tertiary(ui: &mut Ui, icon: &str, label: &str, h: f32) -> egui::Response {
    tier_button(ui, Some(icon), label, h, Tier { fill: Color32::TRANSPARENT, stroke: Stroke::new(1.0, theme::edge()), fg: theme::ink(), family: theme::semibold() })
}

/// The square shared by [`icon_button`] and [`more_dots`]: a `side` square in the secondary look
/// at rest (card fill, 1 pt [`theme::edge`], radius 12, ink glyph), hover_bg / accent_faded (also
/// while its popup is open), focus ring, `tip` as the hover text and the accessible name. Returns
/// the response and the colour to draw the glyph in.
fn icon_well(ui: &mut Ui, side: f32, tip: &str) -> (egui::Response, Color32) {
    let enabled = ui.is_enabled();
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(side), sense);
    let r = CornerRadius::same(12);
    let (fill, edge, fg) = if !enabled {
        let (f, t, e) = crate::pageshell::unavailable_colors();
        (f, e, t)
    } else if resp.is_pointer_button_down_on() || popup_open(ui, &resp) {
        (theme::accent_faded(), theme::edge(), theme::ink())
    } else if resp.hovered() {
        (theme::hover_bg(), theme::edge(), theme::ink())
    } else {
        (theme::card(), theme::edge(), theme::ink())
    };
    ui.painter().rect(rect, r, fill, Stroke::new(1.0, edge), StrokeKind::Inside);
    if resp.has_focus() {
        focus_ring(ui, rect, 12);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, tip));
    let resp = if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp };
    (resp.on_hover_text(tip), fg)
}

/// Icon tier: a `side` square in the secondary look holding one ink glyph (`icon`: anything
/// `handicons::paint_glyph` takes). `tip` is both the hover text and the accessible name.
pub fn icon_button(ui: &mut Ui, icon: &str, side: f32, tip: &str) -> egui::Response {
    let (resp, fg) = icon_well(ui, side, tip);
    let glyph = (side * 0.45).clamp(14.0, 20.0);
    crate::handicons::paint_glyph(ui.painter(), resp.rect.center(), icon, glyph, fg);
    resp
}

/// The overflow "…": three painted ink dots in a `side` square in the secondary look. Anchor its
/// menu on the returned response (`egui::Popup::menu(&r)`); it stays pressed while open.
pub fn more_dots(ui: &mut Ui, side: f32, tip: &str) -> egui::Response {
    let (resp, fg) = icon_well(ui, side, tip);
    let step = if side >= 36.0 { 6.5 } else { 5.5 };
    for dx in [-step, 0.0, step] {
        ui.painter().circle_filled(resp.rect.center() + Vec2::new(dx, 0.0), 2.0, fg);
    }
    resp
}

/// A card's main action one step stronger than [`secondary_accent`]: a tinted accent fill, a
/// 1.5 pt accent outline and an accent bold label, `h` tall (the look the Exams grid's Open won
/// with). For a card whose other actions are quiet; the page's one filled primary stays filled.
pub fn tinted_accent(ui: &mut Ui, label: &str, h: f32) -> egui::Response {
    let tint = theme::accent_faded();
    let toward = theme::accent_col();
    let text = RichText::new(label).family(theme::bold_family()).size(ty::BUTTON_SM).color(theme::accent_col());
    crate::pageshell::styled_button(ui, text, tint, tint.lerp_to_gamma(toward, 0.12), tint.lerp_to_gamma(toward, 0.22), Stroke::new(1.5, theme::accent_col()), h)
}

/// The width [`tinted_accent`] takes for `label` (its 18 pt side pads and the 13.5 bold label).
pub fn tinted_accent_w(ui: &Ui, label: &str) -> f32 {
    let g = ui.painter().layout_no_wrap(label.to_owned(), FontId::new(ty::BUTTON_SM, theme::bold_family()), theme::accent_col());
    (g.size().x + 36.0).ceil()
}

/// A quiet overflow "…" beside a card's stronger main action: no fill and a 1 pt [`theme::edge`]
/// outline at rest (the quiet outlined icon-button tier; the dots in `text`), the same `side`
/// square hit area; under the pointer it takes the
/// secondary look (hover_bg, 1 pt [`theme::edge`], ink dots), accent_faded while pressed or while
/// its menu is open, the focus ring, PointingHand, and `tip` as hover text and accessible name.
pub fn quiet_more_dots(ui: &mut Ui, side: f32, tip: &str) -> egui::Response {
    let enabled = ui.is_enabled();
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(side), sense);
    let (fill, edge, fg) = if !enabled {
        (Color32::TRANSPARENT, Color32::TRANSPARENT, theme::muted_col())
    } else if resp.is_pointer_button_down_on() || popup_open(ui, &resp) {
        (theme::accent_faded(), theme::edge(), theme::ink())
    } else if resp.hovered() {
        (theme::hover_bg(), theme::edge(), theme::ink())
    } else {
        (Color32::TRANSPARENT, theme::edge(), theme::text_col())
    };
    ui.painter().rect(rect, CornerRadius::same(12), fill, Stroke::new(1.0, edge), StrokeKind::Inside);
    let step = if side >= 36.0 { 6.5 } else { 5.5 };
    for dx in [-step, 0.0, step] {
        ui.painter().circle_filled(rect.center() + Vec2::new(dx, 0.0), 2.0, fg);
    }
    if resp.has_focus() {
        focus_ring(ui, rect, 12);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, tip));
    let resp = if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp };
    resp.on_hover_text(tip)
}

/// A text link: 13 pt accent semibold, no underline at rest, underlined under the pointer and
/// when focused (plus the ring); `arrow` adds a trailing →. At least 18 tall. Returns its own click.
pub fn link(ui: &mut Ui, label: &str, arrow: bool) -> egui::Response {
    let enabled = ui.is_enabled();
    let col = if enabled { theme::accent_col() } else { theme::muted_col() };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), FontId::new(ty::META, theme::semibold()), col);
    let text = galley.size();
    let arrow_w = if arrow { 4.0 + 13.0 } else { 0.0 };
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new((text.x + arrow_w).ceil(), text.y.max(18.0)), sense);
    let top = rect.center().y - text.y / 2.0;
    ui.painter().galley(Pos2::new(rect.left(), top), galley, col);
    if arrow {
        crate::handicons::paint_glyph(ui.painter(), Pos2::new(rect.right() - 6.5, rect.center().y + 0.5), egui_phosphor::regular::ARROW_RIGHT, 13.0, col);
    }
    if enabled && (resp.hovered() || resp.has_focus()) {
        let y = (top + text.y - 2.0).round() + 0.5;
        ui.painter().hline(rect.left()..=rect.left() + text.x, y, Stroke::new(1.0, col));
    }
    if resp.has_focus() {
        focus_ring(ui, rect.expand(2.0), 4);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::{kittest::{NodeT, Queryable}, Harness};

    const NAMES: [&str; 6] = ["Import", "Open", "Details", "Preview bank", "Exam actions", "All exams"];

    /// One row of every tier; each click pushes its name. `enabled` false draws them disabled.
    fn tiers(enabled: bool) -> Harness<'static, Vec<&'static str>> {
        let mut first = true;
        Harness::builder().build_ui_state(
            move |ui, clicks: &mut Vec<&'static str>| {
                if std::mem::take(&mut first) {
                    theme::setup(ui.ctx());
                    ui.ctx().request_repaint();
                    return;
                }
                ui.add_enabled_ui(enabled, |ui| {
                    ui.horizontal(|ui| {
                        let rs = [
                            secondary(ui, "Import", 40.0),
                            secondary_accent(ui, "Open", 32.0),
                            tertiary(ui, crate::handicons::INFO, "Details", 40.0),
                            icon_button(ui, egui_phosphor::regular::EYE, 40.0, "Preview bank"),
                            more_dots(ui, 32.0, "Exam actions"),
                            link(ui, "All exams", true),
                        ];
                        for (r, n) in rs.iter().zip(NAMES) {
                            if r.clicked() {
                                clicks.push(n);
                            }
                        }
                    });
                });
            },
            Vec::new(),
        )
    }

    /// Every tier is a button named by its label (icon tiers by their tip), Tab reaches each in
    /// order, and Enter presses the focused one (spec section 4, "Every tier").
    #[test]
    fn every_tier_is_a_named_button_that_tab_reaches_and_enter_presses() {
        let mut h = tiers(true);
        h.run();
        for n in NAMES {
            let node = h.get_by_label(n);
            assert_eq!(node.accesskit_node().role(), egui::accesskit::Role::Button, "{n}");
            assert!(!node.accesskit_node().is_disabled(), "{n}");
        }
        for n in NAMES {
            h.key_press(egui::Key::Tab);
            h.run();
            assert!(h.get_by_label(n).is_focused(), "Tab reaches {n}");
            h.key_press(egui::Key::Enter);
            h.run();
            assert_eq!(h.state().last(), Some(&n), "Enter presses {n}");
        }
        assert_eq!(h.state().len(), NAMES.len());
    }

    /// Inside a disabled Ui every tier is reported disabled and Tab skips it.
    #[test]
    fn disabled_tiers_are_marked_and_not_focusable() {
        let mut h = tiers(false);
        h.run();
        for n in NAMES {
            assert!(h.get_by_label(n).accesskit_node().is_disabled(), "{n}");
        }
        h.key_press(egui::Key::Tab);
        h.run();
        for n in NAMES {
            assert!(!h.get_by_label(n).is_focused(), "{n} took focus while disabled");
        }
    }

    /// The type roles sit at or above the floor, and only the label role sits on it.
    #[test]
    fn the_type_scale_keeps_the_floor() {
        let all = [ty::DISPLAY, ty::DISPLAY_NARROW, ty::HERO_LEAD, ty::HERO_LEAD_NARROW, ty::TITLE, ty::SECTION, ty::PANEL, ty::CARD, ty::LEAD, ty::BODY, ty::META, ty::LABEL, ty::BUTTON, ty::BUTTON_SM, ty::BUTTON_LG];
        assert!(all.iter().all(|&s| s >= ty::FLOOR));
        assert_eq!(all.iter().filter(|&&s| s == ty::FLOOR).count(), 1);
    }
}
