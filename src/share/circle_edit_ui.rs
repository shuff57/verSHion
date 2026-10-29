//! The working-copy screens of the Circles tab (egui): only draws. All logic is in
//! [`CircleEditModel`]. Every server string goes through [`api::plain`]; no raw code is shown.

use egui::{Margin, Ui};

use super::api;
use super::browse::Problem;
use super::circle_edit::*;
use super::circle_links::CircleLink;
use super::circles::CirclesModel;
use super::circles_text as text;
use super::circles_ui::{back_link, line, pill, problem_box, section, text_row};
use super::merge::{Choice, MergePlan, Row, RowKind};
use super::pool::PublishProgress;
use super::publish_ui::{body, para, warn_box};
use crate::{kit, theme};

/// Draws the screen that is open, if one is and it was opened from the Shared page. Returns true when it did (the
/// circle page is not drawn). A screen opened from a bank's page in My Banks is drawn there (`draw_page`).
pub(super) fn draw(ui: &mut Ui, m: &mut CirclesModel) -> bool {
    if !m.edit.active() || m.edit.host.is_some() {
        return false;
    }
    draw_page(ui, m);
    true
}

/// The open screen (send, get latest, history, a version), wherever it was asked for.
/// (round 20, critic 100 minor 8) The page title of "Use my bank as it is" and of "Get latest": the circle's title on both, never
/// her bank's own name on one page and the circle's on the next.
pub(super) fn merge_title(asking_as_is: bool, circle_title: &str) -> String {
    let title = if asking_as_is { text::AS_IS_TITLE } else { "Get latest" };
    format!("{title}: \"{circle_title}\"")
}

pub(super) fn draw_page(ui: &mut Ui, m: &mut CirclesModel) {
    let now = m.now_ms();
    // (critic 71 I1) the History, or one of its versions, is in front of the person this frame
    if matches!(m.edit.view, EditView::History(_) | EditView::Version(_) | EditView::Question(_)) {
        m.edit.mark_drawn(ui.ctx().cumulative_frame_nr());
    }
    // (critic 69 I2, critic 71) the results of this screen's own bank only, each with Dismiss
    let said = m.edit.page_results();
    if let Some(id) = super::bank_links_ui::result_lines(ui, &said) {
        m.edit.dismiss_result(id);
    }
    if !said.is_empty() {
        ui.add_space(6.0);
    }
    let e = &mut m.edit;
    match &e.view {
        EditView::Send(_) => send_page(ui, e, now),
        EditView::Merge(_) => merge_page(ui, e),
        EditView::History(_) => history_page(ui, e, now),
        EditView::Version(_) => version_page(ui, e, now),
        EditView::Question(_) => super::question_history_ui::draw(ui, m),
        EditView::Closed => {}
    }
}

/// The way back at the top of a screen: to the bank's page when it was asked for there (phase D1).
fn back_label(e: &CircleEditModel) -> String {
    e.host.as_ref().map(|h| h.back.clone()).unwrap_or_else(|| "< Back".into())
}

fn wait(p: &Problem) -> Option<std::time::Duration> {
    let at = p.retry_at?;
    let now = web_time::Instant::now();
    (at > now).then(|| at - now)
}

fn problem(ui: &mut Ui, p: &Problem) -> (bool, bool) {
    let (mut retry, mut account) = (false, false);
    problem_box(ui, p, wait(p), &mut retry, &mut account);
    (retry, account)
}

fn counts(a: u32, c: u32, r: u32) -> String {
    format!("{a} added, {c} changed, {r} removed")
}

// ---- send my changes ------------------------------------------------------------------------------------------

#[derive(Clone, PartialEq)]
enum SendAct {
    None,
    Back,
    Send,
    Retry,
    RetryPartial,
    Toggle(String, bool),
    TickAll(bool),
    SendAnyway,
    CancelLarge,
    Merge,
    Note,
    Licence,
    Decide(super::circle_hide_ui::DecideAct),
    Pictures(super::circle_hide_ui::PictureAct),
}

/// The send summary's words (plan 4.7): each change's kind, the row's first line, and the sentence above the rows.
const TICK_INTRO: &str = "Tick the changes to send. Unticked ones stay on this computer, still shown as unsent.";
/// A copy in step with its base has nothing to send and no list to tick. (The second sentence is the page's old wording,
/// kept so what it has said about an in-step copy stays recognisable.)
const IN_STEP: &str = "This copy is in step with the circle, so there is nothing to send. Nothing in your copy differs from the version it was made from.";

fn kind_word(k: super::partial_send::SendKind) -> &'static str {
    use super::partial_send::SendKind;
    match k {
        SendKind::Changed => "Changed",
        SendKind::Added => "Added",
        SendKind::Removed => "Removed",
    }
}

/// "Changed  Q3", or plain "Removed" for a removal a bounded link no longer has a question number for.
fn row_tag(row: &super::partial_send::SendRow) -> String {
    match row.number {
        Some(n) => format!("{}  Q{n}", kind_word(row.kind)),
        None => kind_word(row.kind).to_string(),
    }
}

/// The tick box's accessible name: its kind, the question number, and the stem when this copy still has it, so a
/// screen-reader user hears which question a row is (two bounded removals stay distinguishable by their number).
pub(super) fn row_name(row: &super::partial_send::SendRow) -> String {
    let kind = kind_word(row.kind);
    let number = row.number.map(|n| format!("question {n}"));
    match (&row.stem, number) {
        (Some(stem), Some(n)) => format!("{kind}, {n}: \"{}\"", api::plain(stem)),
        (None, Some(n)) => format!("{kind}, {n}"),
        (Some(stem), None) => format!("{kind}: \"{}\"", api::plain(stem)),
        (None, None) => kind.to_string(),
    }
}

/// The tick box of one row: the visible tag, and the full row as its accessible name.
fn tick_box(ui: &mut Ui, row: &super::partial_send::SendRow, on: &mut bool) -> bool {
    let resp = ui.checkbox(on, theme::text(row_tag(row)));
    let name = row_name(row);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *on, &name));
    resp.changed()
}

/// "Send 2 changes", "Send 1 change".
fn send_label(n: usize) -> String {
    if n == 1 { "Send 1 change".into() } else { format!("Send {n} changes") }
}

/// The send summary's first line: what is sent, where, and the version it is based on.
fn send_heading(title: &str, circle: Option<&str>, base: u64) -> String {
    let to = match circle {
        Some(c) => format!("Send my changes to \"{title}\" ({c})."),
        None => format!("Send my changes to \"{title}\"."),
    };
    format!("{to} Based on version {base}.")
}

/// (phase D3, plan 4.7) What a send that left changes behind says: what went, and what stays on this computer.
pub(super) fn partial_sent_line(version: u64, sent: usize, total: usize) -> String {
    let left = total - sent;
    let stays = if left == 1 { "1 change stays on this computer, not sent.".to_string() } else { format!("{left} changes stay on this computer, not sent.") };
    format!("Sent version {version} with {sent} of your {total} changes. {stays}")
}

/// One change's row: its tick box (kind and question number) and the stem in quotes, cut to the room it has. At a
/// narrow width the stem goes on its own line under the box (plan 4.8); otherwise it follows on the same line. Returns
/// the new tick state when the person changed it.
fn change_row(ui: &mut Ui, row: &super::partial_send::SendRow, ticked: bool) -> Option<bool> {
    let mut on = ticked;
    let mut changed = false;
    let stem = row.stem.as_deref().map(|s| format!("\"{}\"", api::plain(s)));
    let narrow = ui.available_width() < 520.0;
    if narrow {
        changed |= tick_box(ui, row, &mut on);
        if let Some(stem) = &stem {
            ui.indent(("send_row", &row.local), |ui| {
                ui.add(egui::Label::new(theme::text(stem)).truncate());
            });
        }
    } else {
        ui.horizontal(|ui| {
            changed |= tick_box(ui, row, &mut on);
            if let Some(stem) = &stem {
                ui.add(egui::Label::new(theme::text(stem)).truncate());
            }
        });
    }
    changed.then_some(on)
}

/// The send page. (phase D3, plan 4.7) When the base version's body is read, the page lists the changes with a tick
/// box each: the kind, the bank's own question number and the stem, then how many are ticked and the Send button with
/// its count. The plan's per-row "Compare" links are not drawn: the D2 compare view is not built yet.
fn send_page(ui: &mut Ui, e: &mut CircleEditModel, now: i64) {
    let mut act = SendAct::None;
    let mut msg;
    let mut account = false;
    let mut attest;
    let mut act_attest: Option<bool> = None;
    {
        let EditView::Send(v) = &e.view else { return };
        msg = v.message.clone();
        attest = v.attest;
        let working = matches!(v.stage, SendStage::Working(_));
        let done = matches!(v.stage, SendStage::Done(_));
        // Greyed while sending rather than hidden, so the page does not jump.
        let back = back_label(e);
        if ui.add_enabled_ui(!working, |ui| back_link(ui, &back)).inner {
            act = SendAct::Back;
        }
        ui.add_space(4.0);
        // (phase D3, plan 4.7) Once the base version's body is read, the summary's own words say what is sent and
        // from which version; the summary itself is drawn while the page is the person's to act on.
        let editing = matches!(v.stage, SendStage::Editing);
        // (phase D3) A copy in step has nothing to send: no list, no Send, and no "this adds a new version" beside it.
        let in_step = matches!(&v.partial, PartialSend::Ready { changes, .. } if changes.ids().is_empty());
        let heading = match &v.partial {
            PartialSend::Ready { base, .. } if editing => send_heading(&v.title, e.send_circle_name().map(|n| api::plain(n)).as_deref(), *base),
            _ => format!("Send my changes: \"{}\"", v.title),
        };
        ui.add(egui::Label::new(theme::serif(heading).size(20.0)).wrap());
        ui.add_space(2.0);
        // (phase D3) The tick list, or what stands in its way. No per-row "Compare" links: the D2 compare view is not
        // built yet (see this page's own note).
        if editing {
            match &v.partial {
                PartialSend::Ready { changes, ticked, rows, .. } => {
                    if let Some(n) = &v.notice {
                        warn_box(ui, n);
                        ui.add_space(4.0);
                    }
                    // (phase D3) Nothing differs: there is no list to tick, and the page says plainly that there is
                    // nothing to send (no "you can still send it" beside a Send it does not offer).
                    if changes.ids().is_empty() {
                        line(ui, IN_STEP);
                    } else {
                        line(ui, TICK_INTRO);
                        ui.add_space(4.0);
                        for row in rows {
                            if let Some(on) = change_row(ui, row, ticked.contains(&row.local)) {
                                act = SendAct::Toggle(row.local.clone(), on);
                            }
                        }
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.label(theme::text(format!("{} of {} ticked.", ticked.len(), changes.ids().len())));
                            ui.add_space(8.0);
                            if kit::ghost_button(ui, "Tick all").clicked() {
                                act = SendAct::TickAll(true);
                            }
                            ui.label(theme::muted("·"));
                            if kit::ghost_button(ui, "Untick all").clicked() {
                                act = SendAct::TickAll(false);
                            }
                        });
                    }
                    ui.add_space(6.0);
                }
                PartialSend::Reading => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(theme::muted(PARTIAL_READING));
                    });
                    ui.add_space(6.0);
                }
                // (phase D3) A read that may yet work keeps the page and its note field; Send waits for Try again. The
                // generic line is followed by the reason the read kept (a session that ended, the daily limit, the
                // network) when it says more.
                PartialSend::ReadFailed(why) => {
                    warn_box(ui, PARTIAL_READ_FAILED);
                    if why != PARTIAL_READ_FAILED && !why.is_empty() {
                        line(ui, why);
                    }
                    if kit::ghost_button(ui, "Try again").clicked() {
                        act = SendAct::RetryPartial;
                    }
                    ui.add_space(6.0);
                }
                // (phase D3, plan 4.7) The base can not be had: the whole bank is sent, said plainly.
                PartialSend::Unavailable(why) => {
                    warn_box(ui, why);
                    ui.add_space(6.0);
                }
            }
        }
        // What is about to go: once sent, the "Sent" box says what went instead.
        if !done {
            // (phase D3) The summary's tick list replaces the counts line it is made of.
            if !matches!(v.partial, PartialSend::Ready { .. }) {
                match &v.changes {
                    Some(c) if c.known && c.total() == 0 => line(ui, IN_STEP),
                    Some(c) if c.known => line(ui, format!("Since your copy last matched the circle: {}.", counts(c.added as u32, c.changed as u32, c.removed as u32))),
                    // (critic 88 I2) counted but for a difficulty: what is counted, and why a difficulty may be missing (the
                    // strip says the same)
                    // (round 18, critic 98 minor 5) after "Use my bank as it is" the send includes the difficulties: nothing contradicts it
                    Some(c) if c.partial() && e.sends_as_is(&v.local_id) => line(ui, format!("Since your copy last matched the circle: {}.", counts(c.added as u32, c.changed as u32, c.removed as u32))),
                    Some(c) if c.partial() => line(ui, format!("Since your copy last matched the circle: {}. {}", counts(c.added as u32, c.changed as u32, c.removed as u32), text::difficulty_caveat(e.base_version_of(&v.local_id).unwrap_or(0)))),
                    Some(_) => line(ui, format!("{} The circle works them out when you send.", text::NO_RECORD)),
                    None => line(ui, "Counting what changed..."),
                }
            }
            if matches!(v.stage, SendStage::Editing) && !in_step {
                // (round 19, critic 100 minor 9) "restore one later" only to someone who can (the owner, the bank's author)
                line(ui, if v.perm.can_rewind() { "This adds a new version to the circle's bank. The older versions stay in the history, and you can restore one later." } else { "This adds a new version to the circle's bank. The older versions stay in the history." });
            }
            // (critic 78 I2) The earlier send went through but its answer could not be read: the counts still include it.
            if matches!(v.stage, SendStage::Editing) && e.earlier_send(&v.local_id) == Some(true) && !e.status(&v.local_id).is_some_and(|s| s.stale) {
                warn_box(ui, SEND_PAGE_EARLIER);
                if kit::primary_button(ui, "Get latest").clicked() {
                    act = SendAct::Merge;
                }
            }
            // A stale copy can not be sent: say so at the top, with the way on, not only under a greyed Send.
            if matches!(v.stage, SendStage::Editing) && e.status(&v.local_id).is_some_and(|s| s.stale) {
                warn_box(ui, "The bank on this computer is older than the record of it (a safety copy of your banks may have been put back), so it can't be sent yet. Get the latest first; the counts above may be wrong until then.");
                if kit::primary_button(ui, "Get latest").clicked() {
                    act = SendAct::Merge;
                }
            }
            if v.merged {
                para(ui, "This send follows the Get latest you just did, so the history shows that you took in the circle's newer changes.");
            }
            // (round 17, critic 96 I2) what "Use my bank as it is" said, in one line
            if matches!(v.stage, SendStage::Editing) && e.sends_as_is(&v.local_id) {
                para(ui, text::AS_IS_SEND_LINE);
            }
            if let Some(b) = v.base_override {
                warn_box(ui, &format!("The circle's newest version ({b}) is hidden while it is reviewed, so it could not be brought in. This sends your version on top of it and replaces what the circle has now. The hidden version, and anything you did not take in, stay in the history."));
                // The same heading as on the merge page, so the list reads as what is replaced.
                if !v.replaces.is_empty() {
                    ui.label(theme::strong("Versions that would be replaced").size(12.5));
                }
                for l in v.replaces.iter().take(10) {
                    let who = api::plain(&l.by);
                    line(ui, format!("Version {}{}", l.version, if who.is_empty() { String::new() } else { format!(" by {who}") }));
                }
            }
        }
        ui.add_space(6.0);
        match &v.stage {
            SendStage::Done(s) => {
                let mut lines = sent_lines(s, v.perm.can_rewind());
                // (phase D3, plan 4.7) A send that left changes behind says what went and what stayed here, in place
                // of the server's counts. The selection is the page's own (`partial` is not reset by the send), so it
                // is exactly what was sent.
                if s.version > 0
                    && let PartialSend::Ready { changes, ticked, .. } = &v.partial
                    && !ticked.is_empty()
                    && ticked.len() < changes.ids().len()
                {
                    lines[0] = partial_sent_line(s.version, ticked.len(), changes.ids().len());
                }
                section(ui, "Sent", |ui| {
                    for (i, l) in lines.iter().enumerate() {
                        if i == 0 {
                            body(ui, l);
                        } else {
                            para(ui, l);
                        }
                    }
                });
                if kit::primary_button(ui, "Done").clicked() {
                    act = SendAct::Back;
                }
                return finish_send(ui, e, act, msg, account);
            }
            SendStage::Working(p) => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(theme::text(match p {
                        Some(PublishProgress::UploadingImage { done, total, .. }) => format!("Sending picture {} of {total}...", done + 1),
                        Some(PublishProgress::ResendingImages { .. }) => "Sending the pictures again...".to_string(),
                        _ => "Sending your questions...".to_string(),
                    }));
                });
                para(ui, "Nothing is sent twice if you wait. Your changes are still on this computer.");
                return finish_send(ui, e, act, msg, account);
            }
            SendStage::Large { removed, total } => {
                warn_box(ui, &format!("This version removes {removed} of the bank's {total} questions. Nothing has been sent. Send it anyway only if that is what you meant."));
                ui.horizontal_wrapped(|ui| {
                    if kit::danger_text_button(ui, "Send anyway").clicked() {
                        act = SendAct::SendAnyway;
                    }
                    if kit::ghost_button(ui, "Do not send").clicked() {
                        act = SendAct::CancelLarge;
                    }
                });
                return finish_send(ui, e, act, msg, account);
            }
            SendStage::Payload(p) => {
                let p = p.clone();
                if p.issues.is_empty() {
                    // (critic 68 minor 2) one sentence: nothing is listed below to change
                    para(ui, "The circle did not accept this version, and verSHion's server did not say which question. Check the long texts and the pictures, then send again. Nothing was sent; your changes are still on this computer.");
                } else {
                    para(ui, "verSHion checked these questions for the circle and needs you to change the items below. Nothing was sent. Your changes are still on this computer.");
                }
                for i in p.issues.iter().take(30) {
                    let t = super::publish_text::issue_text_circle(i, false);
                    egui::Frame::new().fill(theme::error_wash()).corner_radius(8).inner_margin(Margin::same(8)).show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.add(egui::Label::new(egui::RichText::new(t).color(theme::bad()).size(12.5)).wrap());
                        if let Some(h) = &i.hint {
                            ui.add(egui::Label::new(theme::text(api::plain(h)).size(12.0)).wrap());
                        }
                    });
                    ui.add_space(3.0);
                }
                if p.issues.len() > 30 {
                    para(ui, &format!("... and {} more.", p.issues.len() - 30));
                }
                if !p.issues.is_empty() {
                    para(ui, "Change the questions in the bank, then come back and send again.");
                }
                if kit::ghost_button(ui, "Back to the note").clicked() {
                    act = SendAct::Note;
                }
                return finish_send(ui, e, act, msg, account);
            }
            SendStage::Keyless { positions, kinds } => {
                warn_box(ui, &super::pool::keyless_text(positions, kinds));
                para(ui, "Your changes are still on this computer.");
                ui.horizontal_wrapped(|ui| {
                    if kit::ghost_button(ui, "Back to the note").clicked() {
                        act = SendAct::Note;
                    }
                    if kit::ghost_button(ui, "Cancel").clicked() {
                        act = SendAct::Back;
                    }
                });
                return finish_send(ui, e, act, msg, account);
            }
            SendStage::Decide(d) => {
                let busy = e.acting();
                let a = super::circle_hide_ui::decisions(
                    ui,
                    d,
                    "This bank hides its answer keys from viewers, so verSHion makes a viewers' copy of what you send for the circle. It needs these decisions first. Nothing was sent yet; your changes are still on this computer.",
                    "Change these questions in your bank, then send again.",
                    "Send with these decisions",
                    busy,
                );
                match a {
                    super::circle_hide_ui::DecideAct::None => {}
                    super::circle_hide_ui::DecideAct::Cancel => act = SendAct::Note,
                    other => act = SendAct::Decide(other),
                }
                return finish_send(ui, e, act, msg, account);
            }
            SendStage::Pictures(p) => {
                let busy = e.acting();
                match super::circle_hide_ui::pictures(ui, p, text::PICTURES_SEND_INTRO, "Send", false, busy) {
                    super::circle_hide_ui::PictureAct::None => {}
                    super::circle_hide_ui::PictureAct::Cancel => act = SendAct::Note,
                    other => act = SendAct::Pictures(other),
                }
                return finish_send(ui, e, act, msg, account);
            }
            SendStage::Conflict(c) => {
                // (critic 78 I2) the circle may be ahead because of the person's own earlier send: never "someone else";
                // (critic 80 I2) and never "your earlier send" beside a colleague's name
                let (head, then) = match e.conflict_own(&v.local_id, c) {
                    Some(went) => own_conflict_page(went),
                    None => ("Someone else sent changes first, so nothing was sent and nothing was overwritten.", "Get latest to bring their changes in beside yours, then send. You decide what happens to every question you both changed."),
                };
                warn_box(ui, head);
                para(ui, &conflict_line(c, now));
                para(ui, then);
                ui.horizontal_wrapped(|ui| {
                    if kit::primary_button(ui, "Get latest").clicked() {
                        act = SendAct::Merge;
                    }
                    if kit::ghost_button(ui, "Back").clicked() {
                        act = SendAct::Back;
                    }
                });
                return finish_send(ui, e, act, msg, account);
            }
            SendStage::Refused { message, merge_again } => {
                warn_box(ui, message);
                ui.horizontal_wrapped(|ui| {
                    if *merge_again {
                        if kit::primary_button(ui, "Get latest").clicked() {
                            act = SendAct::Merge;
                        }
                    } else if kit::primary_button(ui, "Send again").clicked() {
                        act = SendAct::Note;
                    }
                    if kit::ghost_button(ui, "Back").clicked() {
                        act = SendAct::Back;
                    }
                });
                return finish_send(ui, e, act, msg, account);
            }
            SendStage::Failed(p) => {
                let (retry, acc) = problem(ui, p);
                if retry {
                    act = SendAct::Retry;
                }
                account = acc;
                ui.add_space(4.0);
                if kit::ghost_button(ui, "Change the note").clicked() {
                    act = SendAct::Note;
                }
                return finish_send(ui, e, act, msg, account);
            }
            SendStage::Editing => {}
        }
        match &v.licence {
            LicenceState::Checking => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(theme::muted("Checking the bank's licence..."));
                });
            }
            LicenceState::Known(l) => line(ui, licence_line(v.perm.is_author, *l)),
            LicenceState::Failed(p) => {
                // The box alone says only what went wrong; this says which check it was.
                ui.label(theme::strong("Checking the bank's licence did not work").size(12.5));
                let (retry, _) = problem(ui, p);
                if retry {
                    act = SendAct::Licence;
                }
            }
        }
        ui.add_space(4.0);
        ui.label(theme::strong("A note for the history (optional)").size(12.5));
        ui.add(egui::TextEdit::multiline(&mut msg).id(egui::Id::new("circle_send_note")).desired_rows(2).desired_width(f32::INFINITY));
        line(ui, "Say what you changed, so the others can tell versions apart. At most 500 characters.");
        ui.add_space(6.0);
        ui.label(theme::strong("Your confirmation").size(12.5));
        kit::card().inner_margin(Margin::same(10)).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let known = if let LicenceState::Known(l) = &v.licence { Some(*l) } else { None };
            ui.add(egui::Label::new(theme::text(attest_statement(v.perm.is_author, known)).size(12.5)).wrap());
            ui.add_space(4.0);
            if ui.checkbox(&mut attest, "Yes, I confirm this").changed() {
                act_attest = Some(attest);
            }
        });
        ui.add_space(6.0);
        let blocker = e.send_blocker();
        // (phase D3, plan 4.7 and 6) The Send button says how many changes go, and with nothing ticked it is disabled:
        // the page says what to do instead of relying on the send's own refusal. A copy with nothing differing has no
        // Send at all: there is nothing to send.
        let (send_text, no_tick) = match &v.partial {
            PartialSend::Ready { ticked, .. } => (send_label(ticked.len()), ticked.is_empty()),
            PartialSend::Unavailable(_) => ("Send everything".to_string(), false),
            _ => ("Send".to_string(), false),
        };
        let unavailable = matches!(v.partial, PartialSend::Unavailable(_));
        ui.horizontal_wrapped(|ui| {
            if !in_step && ui.add_enabled(blocker.is_none() && !no_tick, |ui: &mut Ui| kit::primary_button(ui, &send_text)).clicked() {
                act = SendAct::Send;
            }
            // (phase D3, plan 4.7) The plan's line offers Get latest; the way to it is here, as on a 409.
            if unavailable && kit::ghost_button(ui, "Get latest").clicked() {
                act = SendAct::Merge;
            }
            if kit::ghost_button(ui, "Cancel").clicked() {
                act = SendAct::Back;
            }
        });
        if no_tick && !in_step {
            line(ui, TICK_ONE);
        }
        if let Some(b) = blocker
            && !in_step
            && !matches!(v.partial, PartialSend::Reading | PartialSend::ReadFailed(_))
        {
            line(ui, b);
        }
    }
    if let Some(a) = act_attest {
        e.set_attest(a);
    }
    finish_send(ui, e, act, msg, account);
}

fn finish_send(_ui: &mut Ui, e: &mut CircleEditModel, act: SendAct, msg: String, account: bool) {
    if let EditView::Send(v) = &e.view
        && msg != v.message
    {
        e.set_message(&msg);
    }
    if account {
        // (critic 66 I7) the Account window opens, on the bank's page as on the Shared page
        e.ask_account();
    }
    match act {
        SendAct::None => {}
        SendAct::Back => e.close(),
        SendAct::Send => e.send(),
        SendAct::Retry => e.retry_act(),
        SendAct::RetryPartial => e.retry_partial(),
        SendAct::Toggle(local, on) => e.toggle_change(&local, on),
        SendAct::TickAll(on) => e.tick_all(on),
        SendAct::SendAnyway => e.confirm_large_send(),
        SendAct::CancelLarge => e.cancel_large_send(),
        SendAct::Note => e.back_to_note(),
        SendAct::Licence => e.retry_licence(),
        SendAct::Decide(a) => {
            use super::circle_hide_ui::DecideAct;
            match a {
                DecideAct::Label(at, c) => e.send_decide_label(at, c),
                DecideAct::Order(at, y) => e.send_set_order(at, y),
                DecideAct::AllOrders => e.send_all_orders(),
                DecideAct::Formatting(q, y) => e.send_set_formatting(q, y),
                DecideAct::Send => e.send_with_decisions(),
                DecideAct::Cancel | DecideAct::None => {}
            }
        }
        SendAct::Pictures(a) => {
            use super::circle_hide_ui::PictureAct;
            match a {
                PictureAct::Keep(q, yes) => e.send_keep_picture(q, yes),
                PictureAct::Send => e.send_with_pictures(),
                PictureAct::Cancel | PictureAct::None => {}
            }
        }
        SendAct::Merge => {
            let (local, perm) = match &e.view {
                EditView::Send(v) => (v.local_id.clone(), v.perm),
                _ => return,
            };
            e.open_merge(&local, perm);
        }
    }
}

// ---- get latest and merge -----------------------------------------------------------------------------------

#[derive(Clone, PartialEq)]
enum MergeAct {
    None,
    Cancel,
    Retry,
    Apply,
    Pick(usize, Choice),
    SendNow,
    OnTop(u64),
    AskRebase,
    AskForget,
    CancelAsk,
    Rebase,
    Forget,
}

fn kind_label(k: RowKind) -> &'static str {
    match k {
        RowKind::Unchanged => "Unchanged",
        RowKind::TheirsChanged => "Changed in the circle",
        RowKind::OursChanged => "Changed by you",
        RowKind::Conflict => "CONFLICT: both changed",
        RowKind::NewInTheirs => "New in the circle",
        RowKind::RemovedInTheirs => "Removed from the circle",
        RowKind::RemovedLocally => "Deleted by you",
    }
}

/// "2 conflicts (both changed)", "1 new in the circle": the counts line above the review.
fn summary_label(k: RowKind, n: usize) -> String {
    match k {
        RowKind::Conflict if n == 1 => "1 conflict (both changed)".into(),
        RowKind::Conflict => format!("{n} conflicts (both changed)"),
        _ => format!("{n} {}", kind_label(k).to_lowercase()),
    }
}

fn choice_label(row: &Row, c: Choice) -> &'static str {
    match (row.kind, c) {
        (RowKind::TheirsChanged, Choice::Take) if row.carry_key => "Take the circle's wording and keep my answer key",
        (RowKind::TheirsChanged, Choice::Take) => "Take the circle's version",
        (RowKind::TheirsChanged, _) => "Keep mine",
        // (critic 88 I3) a change the person sent that may have been changed in the circle since
        (RowKind::OursChanged, Choice::Take) => "Take the circle's version",
        (RowKind::OursChanged, _) => "Keep mine",
        (RowKind::Conflict, Choice::Ours) => "Keep mine",
        // (critic 92 I3) one wording on every card
        (RowKind::Conflict, Choice::Theirs) if row.carry_key => "Take the circle's version, keeping my answer key",
        (RowKind::Conflict, Choice::Theirs) => "Take the circle's version",
        (RowKind::Conflict, _) => "Keep both (the circle's is added as a second question)",
        (RowKind::NewInTheirs, Choice::Add) => "Add it to my bank",
        (RowKind::NewInTheirs, _) => "Do not add it",
        (RowKind::RemovedInTheirs, Choice::Keep) => "Keep it (it will be sent as a new question)",
        (RowKind::RemovedInTheirs, _) => "Remove it from my bank",
        _ => "",
    }
}

/// "In version 1: <text>" in a review card: who muted, the text itself in the text colour; wraps as one label.
fn who_line(ui: &mut Ui, who: &str, value: &str) {
    let mut job = egui::text::LayoutJob::default();
    let style = ui.style().clone();
    theme::muted(format!("{who}: ")).size(12.0).append_to(&mut job, &style, egui::FontSelection::Default, egui::Align::Center);
    theme::text(value).size(12.0).append_to(&mut job, &style, egui::FontSelection::Default, egui::Align::Center);
    ui.add(egui::Label::new(job).wrap());
}

/// The merge's notes are worded for the public pool; here the other side is the circle.
fn merge_page(ui: &mut Ui, e: &mut CircleEditModel) {
    let mut act = MergeAct::None;
    let mut can_send = true;
    let (local, perm);
    {
        let EditView::Merge(v) = &e.view else { return };
        local = v.local_id.clone();
        perm = v.perm;
        can_send = can_send && v.perm.can_send();
        // The same way out at the top as the other pages (greyed while the bank is being changed).
        let applying = matches!(v.phase, MergePhase::Applying(_));
        let back = back_label(e);
        if ui.add_enabled_ui(!applying, |ui| back_link(ui, &back)).inner {
            act = MergeAct::Cancel;
        }
        ui.add_space(4.0);
        ui.add(egui::Label::new(theme::serif(merge_title(e.asking_as_is(), &v.title)).size(20.0)).wrap());
        ui.add_space(4.0);
        let leave = |ui: &mut Ui, act: &mut MergeAct, label: &str| {
            if kit::ghost_button(ui, label).clicked() {
                *act = MergeAct::Cancel;
            }
        };
        match &v.phase {
            MergePhase::Fetching | MergePhase::Ready(_) => {
                // (critic 91 minor) waiting for the read of the version the copy last matched: said, and why when it takes long
                let wait = e.merge_wait_note();
                text_row(ui, |ui| {
                    ui.spinner();
                    ui.add(egui::Label::new(theme::muted(wait.as_deref().unwrap_or("Looking at what changed in the circle..."))).wrap());
                });
                para(ui, "Nothing in your bank changes yet.");
                leave(ui, &mut act, "Cancel");
            }
            MergePhase::Review(r) => review(ui, e, r, &mut act),
            MergePhase::Applying(_) => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(theme::text("Fetching the circle's newest version and its pictures..."));
                });
                para(ui, "Your bank is changed only when this has arrived and a safety copy of your banks has been saved. If it fails, nothing changes.");
                leave(ui, &mut act, "Cancel");
            }
            MergePhase::Applied(d) => {
                kit::card().inner_margin(Margin::same(12)).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label(theme::strong(format!("Your copy now matches version {}", d.version)).size(14.5));
                    let o = &d.outcome;
                    line(ui, format!("{} taken from the circle, {} kept as yours, {} added, {} removed, {} kept as a second copy.", o.taken, o.kept, o.added, o.removed, o.both));
                    ui.add_space(4.0);
                    ui.label(theme::strong("Your way back").size(12.5));
                    ui.add(egui::Label::new(theme::text(format!("A safety copy of all your banks from just before this change is saved at {}", api::plain(&d.restore))).size(12.0)).wrap());
                    line(ui, "If you need to go back, that file has all your banks as they were just before.");
                });
                ui.add_space(6.0);
                if (d.changes.total() > 0 || !d.changes.known) && !can_send {
                    // (phase D1: a copy that follows the circle keeps its own changes; it never sends)
                    para(ui, "Your copy keeps changes the circle does not have. They stay on this computer; viewers can't send.");
                    if kit::primary_button(ui, "Done").clicked() {
                        act = MergeAct::Cancel;
                    }
                } else if d.changes.total() > 0 || !d.changes.known {
                    para(ui, "Your copy still has changes the circle does not have. Nothing is sent until you press Send.");
                    ui.horizontal_wrapped(|ui| {
                        if ui.add_enabled(can_send, |ui: &mut Ui| kit::primary_button(ui, "Send my changes...")).clicked() {
                            act = MergeAct::SendNow;
                        }
                        leave(ui, &mut act, "Not now");
                    });
                } else {
                    para(ui, "Your copy now matches the circle's newest version. There is nothing to send.");
                    if kit::primary_button(ui, "Done").clicked() {
                        act = MergeAct::Cancel;
                    }
                }
            }
            // (round 17, critic 96 I2) after "Use my bank as it is", what was done (the line above says what the send does)
            MergePhase::UpToDate if v.as_is => {
                body(ui, text::AS_IS_DONE);
                leave(ui, &mut act, "Back");
            }
            MergePhase::UpToDate => {
                body(ui, "You already have the circle's newest version. There is nothing to get.");
                leave(ui, &mut act, "Back");
            }
            // (round 17, critic 96 I2) the question of "Use my bank as it is", in its own words: what the next send does, and the
            // difficulties it overwrites where they were never read
            MergePhase::StaleUpToDate { ask: Some(StaleAsk::Rebase) } => {
                para(ui, text::AS_IS_TEXT);
                if e.never_read(&v.local_id) {
                    para(ui, text::AS_IS_DROPPED);
                }
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(can_send, |ui: &mut Ui| kit::danger_text_button(ui, "Yes, use it as it is")).clicked() {
                        act = MergeAct::Rebase;
                    }
                    if kit::ghost_button(ui, "No, go back").clicked() {
                        act = MergeAct::CancelAsk;
                    }
                });
            }
            MergePhase::StaleUpToDate { ask } => {
                body(ui, "The circle has nothing newer than the version your copy last matched.");
                // (round 16, critic 94 m1) or that version can't be read any more (the copy was linked by an earlier build)
                if e.cant_be_read(&v.local_id) {
                    warn_box(ui, "But that version can't be read any more, so verSHion can't tell which difficulties you changed since. Sending is blocked until you decide.");
                } else {
                    warn_box(ui, "But the bank on this computer is older than the record of it: a safety copy of your banks may have been put back. Sending is blocked until you decide.");
                }
                match ask {
                    None => {
                        para(ui, "If the bank as it is now is what you want the circle to have, use it as it is: nothing in the bank changes, and the next send sends it as it is, replacing the circle's questions with yours. Or unlink it: the bank stays, but it is no longer connected to the circle.");
                        ui.horizontal_wrapped(|ui| {
                            if ui.add_enabled(can_send, |ui: &mut Ui| kit::primary_button(ui, "Use my bank as it is...")).clicked() {
                                act = MergeAct::AskRebase;
                            }
                            if kit::ghost_button(ui, "Unlink...").clicked() {
                                act = MergeAct::AskForget;
                            }
                            leave(ui, &mut act, "Back");
                        });
                    }
                    // (asked on its own page, above)
                    Some(StaleAsk::Rebase) => {}
                    Some(StaleAsk::Forget) => {
                        warn_box(ui, "Are you sure? The bank stays on this computer as it is, but it is no longer connected to the circle. Adding the circle's bank again later makes a new bank; this one stays unconnected.");
                        ui.horizontal_wrapped(|ui| {
                            if kit::danger_text_button(ui, "Yes, unlink").clicked() {
                                act = MergeAct::Forget;
                            }
                            if kit::ghost_button(ui, "No, go back").clicked() {
                                act = MergeAct::CancelAsk;
                            }
                        });
                    }
                }
            }
            MergePhase::HeadWithheld { head_version, replaces } => {
                match head_version {
                    Some(h) => warn_box(ui, &format!("The circle's newest version ({h}) is hidden while it is reviewed, so it can't be brought in. It stays in the history.")),
                    None => warn_box(ui, "The circle's newest version is hidden while it is reviewed, so it can't be brought in. It stays in the history."),
                }
                para(ui, "You can still send your version on top of it. That replaces what the circle has now with your bank: changes other people sent that your bank doesn't have will no longer be in the newest version. They stay in the history, where they can be restored. Only do it if your bank is what the circle should have.");
                if !replaces.is_empty() {
                    ui.label(theme::strong("Versions that would be replaced").size(12.5));
                    for l in replaces.iter().take(10) {
                        let who = api::plain(&l.by);
                        let note = api::plain(&l.message);
                        line(ui, format!("Version {}{}{}", l.version, if who.is_empty() { String::new() } else { format!(" by {who}") }, if note.is_empty() { String::new() } else { format!(": \"{note}\"") }));
                    }
                    ui.add_space(6.0);
                }
                ui.horizontal_wrapped(|ui| {
                    if let Some(h) = head_version
                        && ui.add_enabled(can_send, |ui: &mut Ui| kit::primary_button(ui, "Send my version on top...")).clicked()
                    {
                        act = MergeAct::OnTop(*h);
                    }
                    leave(ui, &mut act, "Back");
                });
            }
            MergePhase::Blocked(why) => {
                warn_box(ui, why);
                leave(ui, &mut act, "Back");
            }
            MergePhase::Failed(p) => {
                let (retry, _) = problem(ui, p);
                if retry {
                    act = MergeAct::Retry;
                }
                ui.add_space(4.0);
                leave(ui, &mut act, "Back");
            }
        }
    }
    match act {
        MergeAct::None => {}
        MergeAct::Cancel => e.cancel_merge(),
        MergeAct::Retry => e.retry_merge(),
        MergeAct::Apply => e.apply_merge(),
        MergeAct::Pick(i, c) => e.set_merge_choice(i, c),
        MergeAct::SendNow => {
            e.open_send(&local, perm);
        }
        MergeAct::OnTop(h) => e.send_on_top(h, perm),
        MergeAct::AskRebase => e.ask_stale(StaleAsk::Rebase),
        MergeAct::AskForget => e.ask_stale(StaleAsk::Forget),
        MergeAct::CancelAsk => e.cancel_stale_ask(),
        MergeAct::Rebase => e.rebaseline(),
        MergeAct::Forget => e.forget_stale(),
    }
}

fn review(ui: &mut Ui, e: &CircleEditModel, r: &Review, act: &mut MergeAct) {
    let plan: &MergePlan = &r.plan;
    para(ui, &format!("Your copy matches version {}. The circle has version {}. Nothing has been changed yet. Go through the questions below, then press Apply changes. Cancel leaves everything as it is. A copy of your banks is saved just before anything is written.", plan.old_version, plan.new_version));
    if !r.start.between.is_empty() {
        ui.label(theme::strong("What was sent since").size(12.5));
        for l in &r.start.between {
            let who = api::plain(&l.by);
            let mut bits = vec![format!("Version {}", l.version)];
            if !who.is_empty() {
                bits.push(format!("by {who}"));
            }
            bits.push(counts(l.added, l.changed, l.removed));
            let note = api::plain(&l.message);
            line(ui, if note.is_empty() { bits.join(", ") } else { format!("{}: \"{note}\"", bits.join(", ")) });
        }
        if r.start.between_total > r.start.between.len() as u64 {
            line(ui, format!("... and {} more versions after those.", r.start.between_total - r.start.between.len() as u64));
        }
        ui.add_space(4.0);
    }
    // (critic 80 I1) the person's own send whose answer did not come back, said plainly
    if let Some(n) = r.own_version {
        para(ui, &text::own_send_line(n));
    }
    // (critic 85 m1) that send's version can not be read now: said plainly
    if let Some(n) = r.own_unread {
        para(ui, &text::own_unreadable_line(n));
    }
    if r.fallback {
        warn_box(ui, "The version your copy was made from is no longer kept, and this copy has no record of it, so this can only compare yours with the circle's newest version. Every question where the two differ is shown as a conflict for you to decide. Nothing is decided for you.");
    }
    let counts_row = [RowKind::Conflict, RowKind::TheirsChanged, RowKind::OursChanged, RowKind::NewInTheirs, RowKind::RemovedInTheirs, RowKind::RemovedLocally, RowKind::Unchanged];
    // (critic 86 I1) a decision only because this computer has no record of a difficulty is counted apart, never as a
    // conflict where both changed
    let unrecorded = plan.rows.iter().filter(|r| r.unrecorded).count();
    let mut summary: Vec<String> = counts_row.iter().map(|k| (*k, plan.count(*k) - if *k == RowKind::Conflict { unrecorded } else { 0 })).filter(|(_, n)| *n > 0).map(|(k, n)| summary_label(k, n)).collect();
    if unrecorded > 0 {
        let at = usize::from(plan.count(RowKind::Conflict) > unrecorded);
        summary.insert(at, text::unrecorded_count(unrecorded));
    }
    line(ui, summary.join(" - "));
    if plan.unlinked > 0 {
        para(ui, &format!("{} question{} in this bank {} yours alone (not in the circle's bank yet), so Get latest leaves {} as {} {}.", plan.unlinked, if plan.unlinked == 1 { "" } else { "s" }, if plan.unlinked == 1 { "is" } else { "are" }, if plan.unlinked == 1 { "it" } else { "them" }, if plan.unlinked == 1 { "it" } else { "they" }, if plan.unlinked == 1 { "is" } else { "are" }));
    }
    if plan.matched > 0 {
        para(ui, &text::same_added_line(plan.matched));
    }
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
            // The kind on its own row, the question under it: a long question then wraps from the
            // card's edge instead of from beside the pill.
            text_row(ui, |ui| {
                // (critic 86 I1) a decision only because a difficulty is not recorded here: its own words, and the
                // conflict colour only while it waits for a choice; (critic 90 I2) the same for a change of the circle's
                // left undecided beside such a difficulty
                let urgent = if row.unrecorded || row.kind == RowKind::TheirsChanged { row.needs_decision() } else { row.kind == RowKind::Conflict };
                let (fill, fg) = if urgent { (theme::error_wash(), theme::bad()) } else { (theme::accent_faded(), theme::text_col()) };
                pill(ui, if row.unrecorded { text::UNRECORDED_LABEL } else { kind_label(row.kind) }, fill, fg);
            });
            ui.add(egui::Label::new(theme::strong(&row.title).size(13.0)).wrap());
            if let Some(n) = &row.note {
                // (critic 72 I1) the merge's own words: the circle changed the question, not verSHion's server
                line(ui, text::merge_words(&api::plain(n)));
            }
            let (base_who, theirs_who) = (format!("In version {}", plan.old_version), format!("In version {} (the circle's newest)", plan.new_version));
            for d in &row.diffs {
                // Two-way: every field counts as changed (the earlier one is not known); only the ones
                // that really differ are worth reading.
                if r.fallback && d.ours == d.theirs {
                    continue;
                }
                ui.label(theme::strong(d.field).size(12.0));
                // (critic 80 I1) a question as the person's own send carried it: when that is the circle's newest, not the old
                // version, it is not shown under the old version's number; (critic 86 I2) when colleagues sent more since,
                // it is the version both started from, shown as the person's send
                let resent = r.resent.contains(&row.base_id);
                let own_who = format!("In version {} (your send)", r.resent_at);
                let older_own = resent && r.resent_at != plan.new_version;
                let two_way = r.fallback || (resent && !older_own);
                let base_line = if older_own { &own_who } else { &base_who };
                let rows: Vec<(&str, &String)> = if two_way { vec![("Yours now", &d.ours), (&theirs_who, &d.theirs)] } else { vec![(base_line, &d.base), ("Yours now", &d.ours), (&theirs_who, &d.theirs)] };
                ui.indent(("merge_diff", i, d.field), |ui| {
                    for (who, v) in rows {
                        let v = api::plain(v);
                        who_line(ui, who, if v.trim().is_empty() { "none" } else { &v });
                    }
                });
            }
            for c in &row.options {
                if ui.radio(row.choice == *c, choice_label(row, *c)).clicked() {
                    *act = MergeAct::Pick(i, *c);
                }
            }
            if row.needs_decision() {
                ui.add(egui::Label::new(theme::accent("Choose one to continue.").size(12.0)).wrap());
            }
        });
        ui.add_space(4.0);
    }
    // (critic 80 polish) how many are unchanged is said once, in the counts above the review
    ui.add_space(8.0);
    let undecided = plan.undecided();
    if undecided > 0 {
        // (critic 86 I1, critic 85 m1) a question whose difficulty is not recorded here, or one new in the circle that may
        // be the person's own, is not a conflict where both changed
        let what = if plan.rows.iter().filter(|r| r.needs_decision()).all(|r| r.kind == RowKind::Conflict && !r.unrecorded) { "conflict" } else { "question" };
        para(ui, &format!("{} {what}{} still need{} a decision before the changes can be applied.", undecided, if undecided == 1 { "" } else { "s" }, if undecided == 1 { "s" } else { "" }));
    }
    // Why Apply merge did not go through: next to the button that was pressed, not above a long review.
    if let Some(p) = &r.problem {
        warn_box(ui, p);
    }
    ui.horizontal(|ui| {
        if ui.add_enabled(e.can_apply_merge(), |ui: &mut Ui| kit::primary_button(ui, "Apply changes")).clicked() {
            *act = MergeAct::Apply;
        }
        if kit::ghost_button(ui, "Cancel").clicked() {
            *act = MergeAct::Cancel;
        }
    });
}

// ---- the history ------------------------------------------------------------------------------------------------------

#[derive(Clone, PartialEq)]
enum HistAct {
    None,
    Back,
    Reload,
    More,
    Look(u64),
    AskRestore(u64),
    CancelRestore,
    Restore(bool),
    RestoreDone,
    Retry,
    OpenTrim,
    TrimKeep(u32),
    AskTrim,
    CancelTrim,
    Trim,
    TrimDone,
}

fn version_line(ui: &mut Ui, l: &VersionLineView, now: i64) {
    let mut bits = vec![];
    let who = api::plain(&l.by);
    if !who.is_empty() {
        bits.push(format!("by {who}"));
    }
    let ago = text::ago(l.at, now);
    if !ago.is_empty() {
        bits.push(ago);
    }
    bits.push(counts(l.added, l.changed, l.removed));
    bits.push(text::questions_text(l.question_count));
    line(ui, bits.join(" - "));
}

type VersionLineView = super::circle_versions_api::VersionLine;

fn history_page(ui: &mut Ui, e: &mut CircleEditModel, now: i64) {
    let mut act = HistAct::None;
    {
        let back = back_label(e);
        let EditView::History(h) = &e.view else { return };
        if back_link(ui, &back) {
            act = HistAct::Back;
        }
        ui.add_space(4.0);
        ui.add(egui::Label::new(theme::serif(format!("History: \"{}\"", h.title)).size(20.0)).wrap());
        line(ui, "Every version that was sent, newest first. A bank keeps its last 20 versions in full; older ones stay listed but can not be opened.");
        // Only worth saying once something is no longer kept.
        if let Some(o) = h.oldest_kept.filter(|o| h.loaded && *o > 1) {
            line(ui, format!("The oldest version still kept is {o}."));
        }
        ui.add_space(4.0);
        // A failed first load is said at the top; a failed "Older versions" at the bottom, where it was pressed.
        if let Some(p) = h.problem.as_ref().filter(|_| h.lines.is_empty()) {
            let (retry, _) = problem(ui, p);
            if retry {
                act = HistAct::Reload;
            }
        }
        // Restore and trim panels.
        if let Some(r) = &h.restore {
            restore_panel(ui, r, &mut act);
        }
        if let Some(t) = &h.trim {
            trim_panel(ui, t, &mut act);
        }
        if !h.loaded && h.problem.is_none() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(theme::muted("Loading the history..."));
            });
            return finish_history(e, act);
        }
        if h.loaded && h.perm.can_rewind() && h.trim.is_none() && h.restore.is_none() {
            if kit::ghost_button(ui, "Trim old versions...").clicked() {
                act = HistAct::OpenTrim;
            }
            ui.add_space(4.0);
        }
        if h.loaded && h.lines.is_empty() {
            body(ui, "No versions yet.");
        }
        for l in &h.lines {
            kit::card().inner_margin(Margin::same(8)).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                text_row(ui, |ui| {
                    ui.label(theme::strong(format!("Version {}", l.version)).size(13.5));
                    if l.version == h.head_version {
                        pill(ui, "Newest", theme::accent_col(), theme::on_accent_col());
                    }
                    for f in version_flags(l, &h.pinned) {
                        pill(ui, f, theme::accent_faded(), theme::text_col());
                    }
                });
                version_line(ui, l, now);
                let note = api::plain(&l.message);
                if !note.is_empty() {
                    ui.add(egui::Label::new(theme::text(format!("\"{note}\"")).size(12.5)).wrap());
                }
                if let Some(r) = l.rollback_of {
                    line(ui, format!("Restored the content of version {r}."));
                }
                if let Some(m) = l.merged_from {
                    line(ui, format!("Includes the circle's changes made since version {m}."));
                }
                if h.closed_to_me(l) {
                    line(ui, "This version has no copy without answer keys, so you can not open it.");
                }
                let can_look = h.can_look(l);
                // One panel at a time: no restore offered while the trim panel is open.
                let can_restore = h.perm.can_rewind() && can_look && l.version < h.head_version && h.restore.is_none() && h.trim.is_none();
                if can_look || can_restore {
                    ui.horizontal_wrapped(|ui| {
                        if can_look && kit::ghost_button(ui, "Look at it").clicked() {
                            act = HistAct::Look(l.version);
                        }
                        if can_restore && kit::ghost_button(ui, "Restore this version...").clicked() {
                            act = HistAct::AskRestore(l.version);
                        }
                    });
                }
            });
            ui.add_space(3.0);
        }
        if let Some(p) = h.problem.as_ref().filter(|_| !h.lines.is_empty()) {
            let (retry, _) = problem(ui, p);
            if retry {
                act = HistAct::Reload;
            }
        }
        if h.loading_more {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(theme::muted("Loading older versions..."));
            });
        } else if h.next_before.is_some() && h.problem.is_none() && kit::ghost_button(ui, "Older versions").clicked() {
            act = HistAct::More;
        }
        if h.loaded && !h.perm.can_rewind() && !h.perm.closed {
            line(ui, "The circle's owner, or the person who sent the bank first, can restore an older version or trim the history.");
        }
    }
    finish_history(e, act);
}

fn restore_panel(ui: &mut Ui, r: &RestoreAsk, act: &mut HistAct) {
    section(ui, &format!("Restore version {}", r.to), |ui| match &r.stage {
        RestoreStage::Confirm => {
            para(ui, &format!("This makes the content of version {} the circle's newest version, as a new version. Nothing is lost: the versions in between stay in the history, and you can restore any of them again.", r.to));
            ui.horizontal_wrapped(|ui| {
                if kit::primary_button(ui, "Restore it").clicked() {
                    *act = HistAct::Restore(false);
                }
                if kit::ghost_button(ui, "Cancel").clicked() {
                    *act = HistAct::CancelRestore;
                }
            });
        }
        RestoreStage::Working => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(theme::text("Restoring..."));
            });
        }
        RestoreStage::Large { removed, total } => {
            warn_box(ui, &format!("Restoring version {} removes {removed} of the bank's {total} questions. Nothing has been changed. Restore it anyway only if that is what you meant.", r.to));
            ui.horizontal_wrapped(|ui| {
                if kit::danger_text_button(ui, "Restore anyway").clicked() {
                    *act = HistAct::Restore(true);
                }
                if kit::ghost_button(ui, "Cancel").clicked() {
                    *act = HistAct::CancelRestore;
                }
            });
        }
        RestoreStage::Done { version, held, withheld } => {
            for (i, l) in restore_lines(*version, r.to, *held, *withheld, true).iter().enumerate() {
                if i == 0 && !*held && !*withheld {
                    body(ui, l);
                } else {
                    para(ui, l);
                }
            }
            if kit::primary_button(ui, "Close").clicked() {
                *act = HistAct::RestoreDone;
            }
        }
        RestoreStage::Failed(p) => {
            let (retry, _) = problem(ui, p);
            if retry {
                *act = HistAct::Retry;
            }
            if kit::ghost_button(ui, "Close").clicked() {
                *act = HistAct::RestoreDone;
            }
        }
        // (critic 77 I2) it may have gone through: never "did not go through"
        RestoreStage::Unclear(why) => {
            let (retry, _) = problem(ui, &Problem { message: unclear_restore_page(why), retry: true, retry_at: None, account: false });
            if retry {
                *act = HistAct::Retry;
            }
            if kit::ghost_button(ui, "Close").clicked() {
                *act = HistAct::RestoreDone;
            }
        }
    });
}

fn trim_panel(ui: &mut Ui, t: &TrimAsk, act: &mut HistAct) {
    section(ui, "Trim old versions", |ui| match &t.stage {
        TrimStage::Editing => {
            para(ui, "Keep only the newest versions in full. The older ones stay listed in the history, but their content is deleted, so they can not be opened or restored. This frees room if the circle's storage is full. Versions kept for an administrator's review are not deleted by a trim.");
            let mut keep = t.keep;
            ui.horizontal(|ui| {
                ui.label(theme::text("Keep the newest"));
                ui.add(egui::DragValue::new(&mut keep).range(1..=19));
                ui.label(theme::text("versions"));
            });
            if keep != t.keep {
                *act = HistAct::TrimKeep(keep);
            }
            ui.horizontal_wrapped(|ui| {
                if kit::primary_button(ui, "Trim...").clicked() {
                    *act = HistAct::AskTrim;
                }
                if kit::ghost_button(ui, "Cancel").clicked() {
                    *act = HistAct::CancelTrim;
                }
            });
        }
        TrimStage::Confirm => {
            warn_box(ui, &format!("This deletes the content of every version except the newest {}. It can not be undone.", t.keep));
            ui.horizontal_wrapped(|ui| {
                if kit::danger_text_button(ui, "Delete the older versions").clicked() {
                    *act = HistAct::Trim;
                }
                if kit::ghost_button(ui, "Cancel").clicked() {
                    *act = HistAct::CancelTrim;
                }
            });
        }
        TrimStage::Working => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(theme::text("Trimming..."));
            });
        }
        TrimStage::Done(d) => {
            for (i, l) in trim_lines(d).iter().enumerate() {
                if i == 0 {
                    body(ui, l);
                } else {
                    para(ui, l);
                }
            }
            if kit::primary_button(ui, "Close").clicked() {
                *act = HistAct::TrimDone;
            }
        }
        TrimStage::Failed(p) => {
            let (retry, _) = problem(ui, p);
            if retry {
                *act = HistAct::Retry;
            }
            if kit::ghost_button(ui, "Close").clicked() {
                *act = HistAct::TrimDone;
            }
        }
        TrimStage::Unclear(why) => {
            let (retry, _) = problem(ui, &Problem { message: unclear_trim_page(why), retry: true, retry_at: None, account: false });
            if retry {
                *act = HistAct::Retry;
            }
            if kit::ghost_button(ui, "Close").clicked() {
                *act = HistAct::TrimDone;
            }
        }
    });
}

fn finish_history(e: &mut CircleEditModel, act: HistAct) {
    match act {
        HistAct::None => {}
        HistAct::Back => e.back(),
        HistAct::Reload => e.reload_history(),
        HistAct::More => e.more_history(),
        HistAct::Look(n) => e.view_version(n),
        HistAct::AskRestore(n) => e.ask_restore(n),
        HistAct::CancelRestore => e.cancel_restore(),
        HistAct::Restore(large) => e.confirm_restore(large),
        HistAct::RestoreDone => e.close_restore_result(),
        HistAct::Retry => e.retry_act(),
        HistAct::OpenTrim => e.open_trim(),
        HistAct::TrimKeep(k) => e.set_trim_keep(k),
        HistAct::AskTrim => e.ask_trim(),
        HistAct::CancelTrim => e.cancel_trim(),
        HistAct::Trim => e.confirm_trim(),
        HistAct::TrimDone => e.close_trim_result(),
    }
}

// ---- looking at a version ------------------------------------------------------------------------------------------

fn version_page(ui: &mut Ui, e: &mut CircleEditModel, now: i64) {
    let (mut back, mut retry) = (false, false);
    {
        let EditView::Version(v) = &e.view else { return };
        if back_link(ui, "< Back to the history") {
            back = true;
        }
        ui.add_space(4.0);
        ui.add(egui::Label::new(theme::serif(format!("Version {}", v.n)).size(20.0)).wrap());
        if let Some(l) = &v.line {
            version_line(ui, l, now);
            let note = api::plain(&l.message);
            if !note.is_empty() {
                ui.add(egui::Label::new(theme::text(format!("\"{note}\"")).size(12.5)).wrap());
            }
        }
        ui.add_space(4.0);
        match &v.load {
            VersionLoad::Loading => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(theme::muted("Opening the version..."));
                });
            }
            VersionLoad::Failed(p) => {
                let (r, _) = problem(ui, p);
                retry = r;
            }
            VersionLoad::Ready(b) => {
                ui.add(egui::Label::new(theme::strong(api::plain(&b.title)).size(14.0)).wrap());
                para(ui, if b.keys_hidden { text::KEYS_HIDDEN_VIEWER } else if b.hide_key { text::ANSWER_KEYS_HIDDEN_NOTE } else { text::ANSWER_KEYS_NOTE });
                if b.questions.is_empty() {
                    body(ui, "No questions could be shown for this version.");
                }
                for q in b.questions.iter().take(200) {
                    kit::card().inner_margin(Margin::same(8)).show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.add(egui::Label::new(theme::strong(format!("{}. {}", q.number, q.stem)).size(13.0)).wrap());
                        if q.has_image {
                            line(ui, "Has a picture.");
                        }
                    });
                    ui.add_space(3.0);
                }
                if b.questions.len() > 200 {
                    para(ui, &format!("... and {} more questions.", b.questions.len() - 200));
                }
            }
        }
    }
    if retry {
        e.retry_version();
    }
    if back {
        e.back();
    }
}

// ---- on the circle page --------------------------------------------------------------------------------------------

/// One line about a linked copy: how it stands against the circle (the circle page's bank rows, phase D1).
/// `role`: the person's role as the circle's page says it now (critic 66 minor 8), not the link's remembered one.
pub(super) fn standing(l: &CircleLink, st: Option<&LinkStatus>, removed: bool, role: super::circles_api::Role) -> Vec<String> {
    let mut v = Vec::new();
    if removed {
        v.push("This bank was removed from the circle, so nothing can be sent to it or got from it. The owner or the bank's author may be able to bring it back.".to_string());
    }
    match st {
        Some(s) if s.missing => v.push("This bank is not on this computer any more.".to_string()),
        Some(s) if s.stale => v.push("The bank on this computer is older than the record of it (a safety copy may have been put back). Sending is blocked until you get the latest from its page in My Banks.".to_string()),
        // (a copy that follows the circle, or that the person can not send from now, has nothing to send: critic 64)
        Some(_) if !(l.can_send() && role.can_add_banks()) => {}
        // (round 15, critic 91 I2) a copy an earlier build linked, before the circle's version it last matched is read
        Some(s) if s.learning == Some(Learning::Checking) => v.push(text::CHECKING.to_string()),
        Some(s) if s.learning == Some(Learning::CantReach) => v.push(text::CANT_CHECK.to_string()),
        // (round 16) as the strip says them
        Some(s) if s.learning == Some(Learning::Limited) => v.push(text::READ_LIMIT.to_string()),
        Some(s) if s.learning == Some(Learning::Held) => v.push(text::DIFFICULTY_HELD.to_string()),
        // (round 16, q93_4) "not kept": counted as the strip counts it, and sending is said to be off
        // (round 17, critic 96 minor 1) with where she can do something about it
        // (round 18, critic 98 minor 3) by why the version can't be read
        Some(s) if s.learning == Some(Learning::NotKept) => v.push(if s.changes.total() > 0 {
            format!("Not sent yet: {}. A change to a difficulty can't be counted for this copy. {}", counts(s.changes.added as u32, s.changes.changed as u32, s.changes.removed as u32), text::cant_read_row(s.unread.unwrap_or(Unread::NotKept)))
        } else {
            format!("A change to a difficulty can't be counted for this copy. {}", text::cant_read_row(s.unread.unwrap_or(Unread::NotKept)))
        }),
        // (critic 88 I2) with why a change to a difficulty may be missing, as the strip says (round 18, critic 98 minor 5: not after
        // "Use my bank as it is", whose send includes the difficulties)
        Some(s) if s.changes.total() > 0 => v.push(format!("Not sent yet: {}.{}", counts(s.changes.added as u32, s.changes.changed as u32, s.changes.removed as u32), if s.changes.partial() && !l.as_is { " A change to a difficulty can't be counted for this copy." } else { "" })),
        Some(s) if s.changes.partial() && !l.as_is => v.push("A change to a difficulty can't be counted for this copy.".to_string()),
        Some(s) if s.changes.partial() => v.push("Your next send makes the circle's newest version match this bank, difficulties included.".to_string()),
        Some(s) if !s.changes.known => v.push("Unsent changes can not be counted for this copy.".to_string()),
        Some(_) => v.push("Nothing to send.".to_string()),
        None => {}
    }
    if l.behind() > 0 {
        v.push(behind_line(l.behind()));
    }
    v
}

/// The bank settings page's history (phase D1, Q1): every version, who sent it and when; restore and trim for
/// the owner and the bank's author. Sending and getting the latest are on the bank's page in My Banks.
pub(super) fn history_section(ui: &mut Ui, m: &mut CirclesModel, cid: &str, bid: &str, title: &str) {
    let mut open = false;
    section(ui, "History", |ui| {
        para(ui, "See every version that was sent, who sent it and when, look at an older one, and (for the owner or the bank's author) restore it.");
        open = kit::ghost_button(ui, "History of versions").clicked();
    });
    if open {
        let p = m.perm_for(cid, bid);
        if m.edit.open_history(cid, bid, title, p) {
            m.edit.host = None;
        }
    }
    // (critic 71 I1) the same as the bank's strip: a restore or trim of this bank on its way, and how the last ones
    // went (also for a circle bank with no copy here), each until dismissed
    if let Some(a) = m.edit.act_line_of(cid, bid) {
        text_row(ui, |ui| {
            ui.add(egui::Spinner::new().size(14.0));
            ui.add(egui::Label::new(theme::muted(a.as_str()).size(12.5)).wrap());
        });
    }
    if let Some(id) = super::bank_links_ui::result_lines(ui, &m.edit.results_of_circle_bank(cid, bid)) {
        m.edit.dismiss_result(id);
    }
    // (critic 67 I3) refused while a send, restore or trim is on its way
    if let Some(w) = m.edit.wait_note() {
        para(ui, &w);
    }
}
