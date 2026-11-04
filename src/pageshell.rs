//! The reference sites' page-shell layout for the app's own screens: soft-shadow cards with 16 px
//! corners and 20 px padding, and 40 px buttons with 12 px corners. Colours and fonts stay the
//! app's own (cream page, green accent, serif headings).
use std::sync::atomic::{AtomicBool, Ordering};

use egui::{Align2, Color32, CornerRadius, Frame, Id, Margin, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::theme;

/// One white card: 16 px corners, hairline border, soft shadow, 20 px padding.
pub fn card<R>(ui: &mut Ui, f: impl FnOnce(&mut Ui) -> R) -> R {
    let w = ui.available_width();
    Frame::new()
        .fill(theme::card())
        .stroke(Stroke::new(1.0, theme::border()))
        .corner_radius(CornerRadius::same(16))
        .shadow(egui::Shadow { offset: [0, 2], blur: 10, spread: 0, color: Color32::from_black_alpha(if theme::is_dark() { 60 } else { 10 }) })
        .inner_margin(Margin::same(20))
        .show(ui, |ui| {
            ui.set_width(w - 42.0);
            f(ui)
        })
        .inner
}

/// A card's heading.
pub fn heading(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).family(theme::serif_family()).size(19.0).color(theme::ink()));
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
        ui.label(RichText::new(crumbs).size(12.8).color(theme::muted_col()));
    }
    ui.label(RichText::new(title).family(theme::serif_family()).size(34.0).color(theme::ink()));
    if let Some(sub) = sub {
        ui.label(theme::muted(sub).size(14.0));
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
        ui.add(egui::Button::new(text).min_size(Vec2::new(0.0, h))).on_hover_cursor(egui::CursorIcon::PointingHand)
    })
    .inner
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
    let mut pressed = None;
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
                ui.painter().text(r.center(), Align2::CENTER_CENTER, icon, theme::icon_font(26.0), theme::accent_col());
                ui.add_space(12.0);
                ui.label(RichText::new(title).family(theme::serif_family()).size(22.0).color(theme::ink()));
                ui.add_space(4.0);
                ui.label(theme::muted(body).size(14.0));
                ui.add_space(16.0);
                if !actions.is_empty() {
                    let gap = 10.0;
                    let total: f32 = actions.iter().map(|(t, _)| button_width(ui, t)).sum::<f32>() + gap * (actions.len() as f32 - 1.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = gap;
                        ui.add_space(((ui.available_width() - total) / 2.0).max(0.0));
                        for (i, (label, primary_btn)) in actions.iter().enumerate() {
                            let r = if *primary_btn { primary(ui, label) } else { outline(ui, label) };
                            if r.clicked() {
                                pressed = Some(i);
                            }
                        }
                    });
                }
            });
        });
    pressed
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

/// The outlined action.
pub fn outline(ui: &mut Ui, text: &str) -> egui::Response {
    button(ui, text, theme::card(), theme::hover_bg(), theme::accent_faded(), Stroke::new(1.0, theme::rule()), theme::ink())
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
pub const PAGE_W: f32 = 1060.0;

const NAV_W: f32 = 236.0;
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
pub fn sections(ui: &mut Ui, salt: &str, items: &[NavItem], content: impl FnOnce(&mut Ui, usize)) {
    if items.is_empty() {
        return;
    }
    let ctx = ui.ctx().clone();
    let id = Id::new(("pageshell_sections", salt));
    let mut selected = ctx.data(|d| d.get_temp::<usize>(id.with("sel"))).unwrap_or(0).min(items.len() - 1);
    if ui.available_width() >= NAV_COLLAPSE_W {
        let content_w = (ui.available_width() - NAV_W - 28.0).clamp(280.0, 800.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 28.0;
            ui.vertical(|ui| {
                ui.set_width(NAV_W);
                nav_column(ui, id, items, &mut selected);
            });
            ui.vertical(|ui| {
                ui.set_width(content_w);
                ease_in_content(ui, id, selected, content);
            });
        });
    } else {
        ui.horizontal_wrapped(|ui| {
            for (i, it) in items.iter().enumerate() {
                if crate::kit::chip_toggle(ui, it.label, i == selected).clicked() {
                    selected = i;
                }
            }
        });
        ui.add_space(12.0);
        ease_in_content(ui, id, selected, content);
    }
    ctx.data_mut(|d| d.insert_temp(id.with("sel"), selected));
}

fn nav_column(ui: &mut Ui, id: Id, items: &[NavItem], selected: &mut usize) {
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
        ui.painter().text(mid + Vec2::new(18.0, 0.0), Align2::LEFT_CENTER, it.icon, theme::icon_font(18.0), col);
        ui.painter().text(mid + Vec2::new(46.0, 0.0), Align2::LEFT_CENTER, it.label, egui::FontId::new(15.2, theme::bold_family()), col);
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
