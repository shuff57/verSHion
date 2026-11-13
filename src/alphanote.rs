//! The alpha notice: a popup the first time the app opens, and after it is closed a small
//! "Report an issue" button that stays in the bottom right corner of every screen. The report
//! itself is the app's existing issue dialog (`screens`); this module only draws the two pieces
//! and says what was pressed.
use egui::{Align2, Color32, CornerRadius, Margin, RichText, Stroke, Ui, Vec2};

use crate::{kit, pageshell, theme};

/// The notice, worded once: the popup and the sign-in card both show it.
pub const TEXT: &str = "verSHion is in alpha and under active development, so things may be slow, change, or break.";

/// What the person did this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Nothing,
    /// Closed the popup (Got it, Esc, or a click outside it).
    Closed,
    /// Pressed "Report an issue", in the popup or on the corner button.
    Report,
}

/// Draws the popup while `popup` is true, else the corner button. A press of "Report an issue" in
/// the popup also closes it, so the caller gets `Report` once and the popup does not come back.
///
/// When the narrow dock drew the report item this frame ([`dock_item`]), the corner button is not
/// drawn: a press of the dock item comes back here as `Report`.
pub fn show(ctx: &egui::Context, popup: bool) -> Outcome {
    if popup {
        popup_modal(ctx)
    } else if in_dock(ctx) {
        if ctx.data_mut(|d| d.remove_temp::<bool>(egui::Id::new(DOCK_PRESSED_ID))).unwrap_or(false) {
            Outcome::Report
        } else {
            Outcome::Nothing
        }
    } else if corner_button(ctx) {
        Outcome::Report
    } else {
        Outcome::Nothing
    }
}

fn popup_modal(ctx: &egui::Context) -> Outcome {
    let mut out = Outcome::Nothing;
    let modal = egui::Modal::new(egui::Id::new("alpha_note_popup"))
        .frame(kit::card().inner_margin(Margin::same(22)))
        .show(ctx, |ui| {
            ui.set_width(440.0_f32.min(ctx.content_rect().width() - 80.0).max(240.0));
            kit::small_caps(ui, "Alpha");
            ui.add_space(2.0);
            ui.label(theme::serif("verSHion is still being built").size(22.0));
            ui.add_space(8.0);
            ui.add(egui::Label::new(theme::text(TEXT)).wrap());
            ui.add_space(14.0);
            ui.horizontal_wrapped(|ui| {
                if pageshell::primary(ui, "Got it").clicked() {
                    out = Outcome::Closed;
                }
                if pageshell::outline(ui, &format!("{}  Report an issue", egui_phosphor::regular::BUG)).clicked() {
                    out = Outcome::Report;
                }
            });
        });
    if out == Outcome::Nothing && modal.should_close() {
        out = Outcome::Closed;
    }
    out
}

/// Where the corner button was drawn last frame, if it is on screen. A footer rule along the bottom of
/// a page stops short of it instead of running underneath. `None` while the report lives in the
/// narrow dock instead (the dock is drawn before the page, so this frame's answer is known).
pub fn corner_rect(ctx: &egui::Context) -> Option<egui::Rect> {
    if in_dock(ctx) {
        return None;
    }
    ctx.memory(|m| m.area_rect(egui::Id::new(CORNER_ID)))
}

const DOCK_PASS_ID: &str = "alpha_report_in_dock";
const DOCK_PRESSED_ID: &str = "alpha_report_dock_pressed";

/// The report is a dock item this frame, so the corner pill is not drawn.
pub fn in_dock(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<u64>(egui::Id::new(DOCK_PASS_ID))) == Some(ctx.cumulative_pass_nr())
}

/// Whether the dock should carry the report: the same moments the corner pill would show (after
/// the first-open popup was closed).
pub fn wants_dock(ctx: &egui::Context) -> bool {
    seen(ctx)
}

/// The narrow window's dock item for the report, drawn by the dock above its light/dark button in
/// place of the floating pill, which sat over page content at narrow widths. `w` x `h` is the dock's
/// item size; `paint` draws the icon and the short visible label "Report" in the dock's own style.
/// Its accessible name is the pill's, "Report an issue". A press is handed to [`show`] this frame.
pub fn dock_item(ui: &mut Ui, w: f32, h: f32, paint: impl FnOnce(&Ui, egui::Rect, &egui::Response)) {
    let ctx = ui.ctx().clone();
    // The pass number is read before the data lock is taken: egui's context lock is not re-entrant.
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(DOCK_PASS_ID), pass));
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), "Report an issue"));
    paint(ui, rect, &resp);
    if resp.on_hover_text("Report an issue: something broken or confusing? Tell us.").clicked() {
        ctx.data_mut(|d| d.insert_temp(egui::Id::new(DOCK_PRESSED_ID), true));
    }
}

const CORNER_ID: &str = "alpha_report_button";

/// The corner button's inset from the window's bottom right corner: the page's own margins
/// (`screens::desk_panel` keeps 20 at the foot and 20 + 14 scrollbar room at the right), so the
/// button sits inside the page, its right edge on the content's right edge and its foot on the
/// page's bottom margin, not across the margin band where the page grid stops.
const CORNER_INSET: [f32; 2] = [-34.0, -20.0];

/// The corner button; true when pressed.
fn corner_button(ctx: &egui::Context) -> bool {
    let mut pressed = false;
    egui::Area::new(egui::Id::new(CORNER_ID))
        .order(egui::Order::Foreground)
        .anchor(Align2::RIGHT_BOTTOM, CORNER_INSET)
        .show(ctx, |ui| {
            egui::Frame::new()
                .corner_radius(CornerRadius::same(255))
                .shadow(egui::epaint::Shadow { offset: [0, 2], blur: 8, spread: 0, color: Color32::from_black_alpha(22) })
                .show(ui, |ui| pressed = pill(ui));
        });
    pressed
}

fn pill(ui: &mut Ui) -> bool {
    // A real action on every screen, in the quiet outlined tier: card fill, the 1 pt control edge,
    // ink icon and label at the compact button size, semibold (the tertiary tier's weight). Muted
    // 12 pt text on a 1.45:1 hairline read as disabled; bold ink read as a second primary action.
    ui.add(
        egui::Button::new(
            RichText::new(format!("{}  Report an issue", egui_phosphor::regular::BUG))
                .size(kit::ty::BUTTON_SM)
                .family(theme::semibold())
                .color(theme::ink()),
        )
        .fill(theme::card())
        .stroke(Stroke::new(1.0, theme::edge()))
        .corner_radius(CornerRadius::same(255))
        .min_size(Vec2::new(0.0, 32.0)),
    )
    .on_hover_text("Something broken or confusing? Tell us.")
    .clicked()
}

/// Kept as its own small document, not a field of `Settings`.
const SEEN_DOC: &str = "alpha_note_seen.json";
const SEEN_ID: &str = "alpha_note_seen";

/// The popup was closed once on this device (read once, then remembered for the session).
pub fn seen(ctx: &egui::Context) -> bool {
    let id = egui::Id::new(SEEN_ID);
    if let Some(v) = ctx.data(|d| d.get_temp::<bool>(id)) {
        return v;
    }
    let v = crate::store::load_json::<bool>(SEEN_DOC).unwrap_or(false);
    ctx.data_mut(|d| d.insert_temp(id, v));
    v
}

/// Remembers the popup was closed, so it does not open again on the next start.
pub fn mark_seen(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(SEEN_ID), true));
    let _ = crate::store::save_json(SEEN_DOC, &true);
}

const VISITED_DOC: &str = "visited.json";
const VISITED_ID: &str = "visited_before";

/// This device has been past the Get Started page before (read once per session).
pub fn visited(ctx: &egui::Context) -> bool {
    let id = egui::Id::new(VISITED_ID);
    if let Some(v) = ctx.data(|d| d.get_temp::<bool>(id)) {
        return v;
    }
    let v = crate::store::load_json::<bool>(VISITED_DOC).unwrap_or(false);
    ctx.data_mut(|d| d.insert_temp(id, v));
    v
}

/// Remembers the Get Started page was left, so the next visit goes straight in.
pub fn mark_visited(ctx: &egui::Context) {
    if visited(ctx) {
        return;
    }
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(VISITED_ID), true));
    let _ = crate::store::save_json(VISITED_DOC, &true);
}

/// Someone who has been here before skips Get Started: they left it once, or they already have
/// banks or exams (they used the app before this flag existed).
pub fn skips_welcome(visited: bool, has_work: bool) -> bool {
    visited || has_work
}

/// How far up the toasts sit so they stay clear of the corner button.
pub const TOAST_LIFT: f32 = 56.0;

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::{kittest::Queryable, Harness};

    struct State {
        popup: bool,
        last: Outcome,
    }

    /// The app's fonts are installed on the first frame, which draws nothing.
    fn harness<'a>(popup: bool) -> Harness<'a, State> {
        let mut first = true;
        Harness::builder().build_ui_state(
            move |ui, s: &mut State| {
                if std::mem::take(&mut first) {
                    theme::setup(ui.ctx());
                    ui.ctx().request_repaint();
                    return;
                }
                let o = show(ui.ctx(), s.popup);
                if o != Outcome::Nothing {
                    s.last = o;
                }
            },
            State { popup, last: Outcome::Nothing },
        )
    }

    #[test]
    fn a_returning_visitor_skips_get_started() {
        assert!(!skips_welcome(false, false), "brand new");
        assert!(skips_welcome(true, false), "left the page before");
        assert!(skips_welcome(false, true), "already has banks or exams");
    }

    #[test]
    fn the_popup_says_alpha_and_got_it_closes_it() {
        let mut h = harness(true);
        h.run();
        assert!(h.query_by_label_contains("is in alpha and under active development").is_some());
        assert!(h.query_by_label_contains("Report an issue").is_some());
        h.get_by_label("Got it").click();
        h.run();
        assert_eq!(h.state().last, Outcome::Closed);
    }

    #[test]
    fn report_in_the_popup_reports() {
        let mut h = harness(true);
        h.run();
        h.get_by_label_contains("Report an issue").click();
        h.run();
        assert_eq!(h.state().last, Outcome::Report);
    }

    #[test]
    fn with_the_popup_closed_only_the_corner_button_shows_and_it_reports() {
        let mut h = harness(false);
        h.run();
        assert!(h.query_by_label_contains("is in alpha and under active development").is_none());
        assert!(h.query_by_label("Got it").is_none());
        h.get_by_label_contains("Report an issue").click();
        h.run();
        assert_eq!(h.state().last, Outcome::Report);
    }

    /// Narrow: the dock carries the report, the corner pill is not drawn, and the dock item (named
    /// "Report an issue" though it reads "Report") reports through `show` the same way.
    #[test]
    fn in_the_narrow_dock_the_item_reports_and_the_pill_is_not_drawn() {
        let mut first = true;
        let mut h = Harness::builder().build_ui_state(
            move |ui, s: &mut State| {
                if std::mem::take(&mut first) {
                    theme::setup(ui.ctx());
                    ui.ctx().request_repaint();
                    return;
                }
                dock_item(ui, 48.0, 52.0, |_, _, _| {});
                assert!(in_dock(ui.ctx()) && corner_rect(ui.ctx()).is_none());
                let o = show(ui.ctx(), s.popup);
                if o != Outcome::Nothing {
                    s.last = o;
                }
            },
            State { popup: false, last: Outcome::Nothing },
        );
        h.run();
        assert_eq!(h.query_all_by_label_contains("Report an issue").count(), 1, "the dock item only, no pill");
        h.get_by_label("Report an issue").click();
        h.run();
        assert_eq!(h.state().last, Outcome::Report);

        // From the keyboard: Tab reaches it (the only focusable control here), Enter presses it.
        h.state_mut().last = Outcome::Nothing;
        h.key_press(egui::Key::Tab);
        h.run();
        assert!(h.get_by_label("Report an issue").is_focused(), "Tab focuses the dock item");
        h.key_press(egui::Key::Enter);
        h.run();
        assert_eq!(h.state().last, Outcome::Report, "Enter on the focused item reports");
    }
}
