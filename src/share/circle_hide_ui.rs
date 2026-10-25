//! The decisions page for hiding a circle bank's answer keys from viewers (egui): only draws. The
//! model is [`Decisions`]; the bank page and a working copy's send both use this page. Every server
//! string goes through `api::plain` (via `publish_text`); no raw code is shown.

use egui::{Margin, RichText, Ui};

use super::api;
use super::circle_hide::{Decisions, Pictures};
use super::circles_text as ctext;
use super::pool::{LabelChoice, Pos};
use super::publish_text as text;
use super::publish_ui::{heading, link, para, warn_box};
use crate::{kit, theme};

/// What the person did on the page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum DecideAct {
    None,
    Label(Pos, LabelChoice),
    Order(Pos, bool),
    AllOrders,
    Formatting(usize, bool),
    Send,
    Cancel,
}

fn place(question: usize, part: Option<usize>) -> String {
    let mut name = text::question_label(question, false);
    if let Some(p) = part {
        name.push_str(&format!(", part {}", p + 1));
    }
    name
}

/// What is left, in one line ("Still to decide: 2 letter labels, 1 choice order.").
pub(super) fn todo_line(d: &Decisions) -> Option<String> {
    let t = d.todo();
    let plural = |n: usize, one: &str, many: &str| if n == 1 { format!("1 {one}") } else { format!("{n} {many}") };
    let mut left = Vec::new();
    if t.labels > 0 {
        left.push(plural(t.labels, "letter label decision", "letter label decisions"));
    }
    if t.orders > 0 {
        left.push(plural(t.orders, "choice order to confirm", "choice orders to confirm"));
    }
    if t.formatting > 0 {
        left.push(plural(t.formatting, "question to check", "questions to check"));
    }
    if t.hard > 0 {
        left.push(plural(t.hard, "thing to change in the questions", "things to change in the questions"));
    }
    (!left.is_empty()).then(|| format!("Still to do: {}.", left.join(", ")))
}

/// The page. `intro` says what this is for; `fix` where a question that must change is changed; `send`
/// names the button; `busy` greys it.
pub(super) fn decisions(ui: &mut Ui, d: &Decisions, intro: &str, fix: &str, send: &str, busy: bool) -> DecideAct {
    let mut act = DecideAct::None;
    let p = &d.refusal;
    para(ui, intro);
    if d.restarted {
        let pics = if d.pictures_dropped { " The pictures you chose to keep were dropped too, so viewers will not get them." } else { "" };
        warn_box(ui, &format!("The bank changed while you were deciding, so all your earlier decisions were dropped. Decide again below.{pics}"));
    }
    // 1. things that must change in the questions
    let hard = d.hard();
    if !hard.is_empty() {
        heading(ui, "Change these in the questions first");
        para(ui, &format!("These can not be acknowledged: they would show the answer to viewers. {fix}"));
        for i in hard.iter().take(30) {
            egui::Frame::new().fill(theme::error_wash()).corner_radius(8).inner_margin(Margin::same(8)).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.add(egui::Label::new(RichText::new(text::issue_text_circle(i, false)).color(theme::bad()).size(12.5)).wrap());
                if let Some(h) = &i.hint {
                    ui.add(egui::Label::new(theme::text(api::plain(h)).size(12.0)).wrap());
                }
            });
            ui.add_space(4.0);
        }
        if hard.len() > 30 {
            para(ui, &format!("... and {} more.", hard.len() - 30));
        }
    }
    // 2. letter labels
    if !p.label_findings.is_empty() {
        heading(ui, "Letter labels on choices");
        para(ui, "Some choices start with letters (A, B, C ...). Strip them, so viewers get the choices shuffled without letters, or keep them, so the letters and the order stay as written.");
        for f in &p.label_findings {
            let at = Pos { question: f.question, part: f.part };
            kit::card().inner_margin(Margin::same(8)).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(theme::strong(place(f.question, f.part)).size(12.5));
                for c in f.choices.iter().take(6) {
                    ui.add(egui::Label::new(theme::muted(format!("\"{}\" becomes \"{}\"", text::shown(&c.before), text::shown(&c.after))).size(12.0)).wrap());
                }
                if f.truncated || f.choices.len() > 6 {
                    ui.label(theme::muted("...").size(12.0));
                }
                if f.order_revealing {
                    ui.add(egui::Label::new(theme::muted("Keeping these letters shows the order, so the order would stay fixed.").size(12.0)).wrap());
                }
                let cur = d.label(at);
                ui.horizontal_wrapped(|ui| {
                    if d.allowed(at, LabelChoice::Strip) && ui.selectable_label(cur == Some(LabelChoice::Strip), "Strip the letters").clicked() {
                        act = DecideAct::Label(at, LabelChoice::Strip);
                    }
                    if d.allowed(at, LabelChoice::Keep) && ui.selectable_label(cur == Some(LabelChoice::Keep), "Keep the letters").clicked() {
                        act = DecideAct::Label(at, LabelChoice::Keep);
                    }
                });
                if !d.allowed(at, LabelChoice::Keep) {
                    ui.add(egui::Label::new(theme::muted("Keeping them would give the answer away, so they can only be stripped.").size(12.0)).wrap());
                }
            });
            ui.add_space(3.0);
        }
    }
    // 3. orders that stay as written
    if !p.needs_order_ack.is_empty() {
        heading(ui, "Choices that stay in your order");
        para(ui, "These choices will not be shuffled for viewers: they see them in the order you wrote them. Confirm that is fine.");
        for at in &p.needs_order_ack {
            let why = p.order_reasons.iter().find(|o| (o.question, o.part) == (at.question, at.part)).map(|o| text::order_reason_text(&o.reason));
            let mut yes = d.order_acked(*at);
            let label = format!("{} stays in this order{}", place(at.question, at.part), why.map(|w| format!(" ({w})")).unwrap_or_default());
            if ui.checkbox(&mut yes, label).changed() {
                act = DecideAct::Order(*at, yes);
            }
        }
        if p.needs_order_ack.len() > 1 && link(ui, "Confirm all of them") {
            act = DecideAct::AllOrders;
        }
    }
    // 4. low-risk formatting
    let ackable = d.ackable();
    if !ackable.is_empty() {
        heading(ui, "Check these choices");
        para(ui, "These might give the answer away to viewers, but may be fine. Look at each one, and tick it only if it does not show the answer.");
        for q in ackable {
            kit::card().inner_margin(Margin::same(8)).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(theme::strong(text::question_label(q, false)).size(12.5));
                for i in p.issues.iter().filter(|i| i.question_index() == Some(q)) {
                    ui.add(egui::Label::new(theme::text(text::issue_text(i, false)).size(12.0)).wrap());
                    if let Some(h) = &i.hint {
                        ui.add(egui::Label::new(theme::muted(api::plain(h)).size(11.5)).wrap());
                    }
                }
                for f in p.formatting_findings.iter().filter(|f| f.question == q) {
                    if let Some(s) = &f.served {
                        ui.add(egui::Label::new(theme::muted(format!("As viewers see it: \"{}\"", text::shown(s))).size(12.0)).wrap());
                    }
                }
                let mut yes = d.formatting_acked(q);
                if ui.checkbox(&mut yes, "I checked: this does not give the answer away").changed() {
                    act = DecideAct::Formatting(q, yes);
                }
            });
            ui.add_space(3.0);
        }
    }
    ui.add_space(6.0);
    if let Some(l) = todo_line(d) {
        para(ui, &l);
    }
    ui.add(egui::Label::new(theme::muted(ctext::HIDE_TRIES).size(12.0)).wrap());
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        if ui.add_enabled(d.ready() && !busy, |ui: &mut Ui| kit::primary_button(ui, send)).clicked() {
            act = DecideAct::Send;
        }
        if kit::ghost_button(ui, "Cancel").clicked() {
            act = DecideAct::Cancel;
        }
        if busy {
            ui.spinner();
        }
    });
    act
}

/// What the person did on the picture step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum PictureAct {
    None,
    /// "Keep it for viewers" (`true`) or "Drop it" for the picture of a question.
    Keep(usize, bool),
    Send,
    Cancel,
}

/// The picture step: each picture of the bank (by its question: the share screens draw no thumbnails), kept for
/// viewers or dropped, dropped until the person keeps it. `intro` says what this is for; `send` names the button;
/// `tries` adds the line about the day's tries (the bank page); `busy` greys the button.
pub(super) fn pictures(ui: &mut Ui, p: &Pictures, intro: &str, send: &str, tries: bool, busy: bool) -> PictureAct {
    let mut act = PictureAct::None;
    para(ui, intro);
    heading(ui, ctext::PICTURES_HEADING);
    para(ui, ctext::PICTURES_HELP);
    for pic in &p.list {
        kit::card().inner_margin(Margin::same(8)).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(theme::strong(text::question_label(pic.question, false)).size(12.5));
            if !pic.stem.is_empty() {
                ui.add(egui::Label::new(theme::muted(text::shown(&pic.stem)).size(12.0)).wrap());
            }
            let kept = p.kept(pic.question);
            ui.horizontal_wrapped(|ui| {
                if ui.selectable_label(kept, ctext::PICTURE_KEEP).clicked() && !kept {
                    act = PictureAct::Keep(pic.question, true);
                }
                if ui.selectable_label(!kept, ctext::PICTURE_DROP).clicked() && kept {
                    act = PictureAct::Keep(pic.question, false);
                }
            });
        });
        ui.add_space(3.0);
    }
    ui.add_space(6.0);
    let n = p.image_ok().len();
    let total = p.list.len();
    para(ui, &match (n, total) {
        (0, 1) => "The picture will be left out for viewers.".to_string(),
        (0, _) => format!("All {total} pictures will be left out for viewers."),
        (1, 1) => "Viewers will get the picture.".to_string(),
        _ if n == total => format!("Viewers will get all {total} pictures."),
        _ => format!("Viewers will get {n} of the {total} pictures; the others are left out."),
    });
    if tries {
        ui.add(egui::Label::new(theme::muted(ctext::HIDE_TRIES).size(12.0)).wrap());
    }
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        if ui.add_enabled(!busy, |ui: &mut Ui| kit::primary_button(ui, send)).clicked() {
            act = PictureAct::Send;
        }
        if kit::ghost_button(ui, "Cancel").clicked() {
            act = PictureAct::Cancel;
        }
        if busy {
            ui.spinner();
        }
    });
    act
}
