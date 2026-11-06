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
pub fn show(ctx: &egui::Context, popup: bool) -> Outcome {
    if popup {
        popup_modal(ctx)
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

/// The corner button; true when pressed.
fn corner_button(ctx: &egui::Context) -> bool {
    let mut pressed = false;
    egui::Area::new(egui::Id::new("alpha_report_button"))
        .order(egui::Order::Foreground)
        .anchor(Align2::RIGHT_BOTTOM, [-16.0, -16.0])
        .show(ctx, |ui| {
            egui::Frame::new()
                .corner_radius(CornerRadius::same(255))
                .shadow(egui::epaint::Shadow { offset: [0, 4], blur: 14, spread: 0, color: Color32::from_black_alpha(40) })
                .show(ui, |ui| pressed = pill(ui));
        });
    pressed
}

fn pill(ui: &mut Ui) -> bool {
    ui.add(
        egui::Button::new(
            RichText::new(format!("{}  Report an issue", egui_phosphor::regular::BUG))
                .family(theme::bold_family())
                .size(13.5)
                .color(theme::ink()),
        )
        .fill(theme::card())
        .stroke(Stroke::new(1.0, theme::border()))
        .corner_radius(CornerRadius::same(255))
        .min_size(Vec2::new(0.0, 36.0)),
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
}
