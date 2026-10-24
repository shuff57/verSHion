//! The working-copy screens of the Circles tab (egui): only draws. All logic is in
//! [`CircleEditModel`]. Every server string goes through [`api::plain`]; no raw code is shown.

use egui::{Margin, Ui};

use super::api;
use super::browse::Problem;
use super::circle_edit::*;
use super::circle_links::CircleLink;
use super::circles::CirclesModel;
use super::circles_api::Role;
use super::circles_text as text;
use super::circles_ui::{back_link, line, pill, problem_box, section, text_row};
use super::merge::{Choice, MergePlan, Row, RowKind};
use super::pool::PublishProgress;
use super::publish_ui::{body, para, warn_box};
use crate::{kit, theme};

/// Draws the screen that is open, if one is. Returns true when it did (the circle page is not drawn).
pub(super) fn draw(ui: &mut Ui, m: &mut CirclesModel) -> bool {
    if !m.edit.active() {
        return false;
    }
    let now = m.now_ms();
    if let Some(n) = m.edit.notice.clone() {
        para(ui, &n);
    }
    let e = &mut m.edit;
    match &e.view {
        EditView::Send(_) => send_page(ui, e, now),
        EditView::Merge(_) => merge_page(ui, e),
        EditView::History(_) => history_page(ui, e, now),
        EditView::Version(_) => version_page(ui, e, now),
        EditView::Closed => {}
    }
    true
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum SendAct {
    None,
    Back,
    Send,
    Retry,
    SendAnyway,
    CancelLarge,
    Merge,
    Note,
    Licence,
}

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
        if ui.add_enabled_ui(!working, |ui| back_link(ui, "< Back")).inner {
            act = SendAct::Back;
        }
        ui.add_space(4.0);
        ui.add(egui::Label::new(theme::serif(format!("Send my changes: \"{}\"", v.title)).size(20.0)).wrap());
        ui.add_space(2.0);
        // What is about to go: once sent, the "Sent" box says what went instead.
        if !done {
            match &v.changes {
                Some(c) if c.known && c.total() == 0 => line(ui, "Nothing in your copy differs from the version it was made from. You can still send it, but there is nothing new to say."),
                Some(c) if c.known => line(ui, format!("Since your copy was last in step with the circle: {}.", counts(c.added as u32, c.changed as u32, c.removed as u32))),
                Some(_) => line(ui, "Your copy has no record of what it looked like when it was last in step, so the changes can not be counted here. The circle works them out when you send."),
                None => line(ui, "Counting what changed..."),
            }
            if matches!(v.stage, SendStage::Editing) {
                line(ui, "This adds a new version to the circle's bank. The older versions stay in the history, and you can restore one later.");
            }
            // A stale copy can not be sent: say so at the top, with the way on, not only under a greyed Send.
            if matches!(v.stage, SendStage::Editing) && e.status(&v.local_id).is_some_and(|s| s.stale) {
                warn_box(ui, "The bank on this computer is older than the record of it (a saved copy of your banks may have been put back), so it can not be sent yet. Get the latest changes and merge first; the counts above may be wrong until then.");
                if kit::primary_button(ui, "Get latest and merge").clicked() {
                    act = SendAct::Merge;
                }
            }
            if v.merged {
                para(ui, "This send goes with the merge you just made, so the history shows that you merged in the circle's newer changes.");
            }
            if let Some(b) = v.base_override {
                warn_box(ui, &format!("The circle's newest version ({b}) is hidden while it is reviewed, so it could not be merged. This sends your version on top of it and REPLACES what the circle has now. The hidden version, and anything you did not merge, stay in the history."));
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
                section(ui, "Sent", |ui| {
                    for (i, l) in sent_lines(s, v.perm.can_rewind()).iter().enumerate() {
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
                para(ui, "The pool checked these questions and needs you to change the items below. Nothing was sent. Your changes are still on this computer.");
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
                if p.issues.is_empty() {
                    para(ui, "The pool did not say which question. Check the long texts and the pictures, then send again.");
                }
                para(ui, "Change the questions in the bank, then come back and send again.");
                if kit::ghost_button(ui, "Back to the note").clicked() {
                    act = SendAct::Note;
                }
                return finish_send(ui, e, act, msg, account);
            }
            SendStage::Conflict(c) => {
                warn_box(ui, "Someone else sent changes first, so nothing was sent and nothing was overwritten.");
                para(ui, &conflict_line(c, now));
                para(ui, "Get their changes and merge them with yours, then send. You decide what happens to every question you both changed.");
                ui.horizontal_wrapped(|ui| {
                    if kit::primary_button(ui, "Get latest and merge").clicked() {
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
                        if kit::primary_button(ui, "Get latest and merge").clicked() {
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
        ui.horizontal_wrapped(|ui| {
            if ui.add_enabled(blocker.is_none(), |ui: &mut Ui| kit::primary_button(ui, "Send")).clicked() {
                act = SendAct::Send;
            }
            if kit::ghost_button(ui, "Cancel").clicked() {
                act = SendAct::Back;
            }
        });
        if let Some(b) = blocker {
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
        e.notice = Some("Open my sharing settings from the top of this page to sign in or fix your account.".into());
    }
    match act {
        SendAct::None => {}
        SendAct::Back => e.close(),
        SendAct::Send => e.send(),
        SendAct::Retry => e.retry_act(),
        SendAct::SendAnyway => e.confirm_large_send(),
        SendAct::CancelLarge => e.cancel_large_send(),
        SendAct::Note => e.back_to_note(),
        SendAct::Licence => e.retry_licence(),
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
        (RowKind::Conflict, Choice::Ours) => "Keep mine",
        (RowKind::Conflict, Choice::Theirs) if row.carry_key => "Use the circle's version, keeping my answer key",
        (RowKind::Conflict, Choice::Theirs) => "Use the circle's version",
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
fn circle_words(note: &str) -> String {
    note.replace("The pool", "The circle").replace("the pool", "the circle")
}

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
        if ui.add_enabled_ui(!applying, |ui| back_link(ui, "< Back")).inner {
            act = MergeAct::Cancel;
        }
        ui.add_space(4.0);
        ui.add(egui::Label::new(theme::serif(format!("Get latest and merge: \"{}\"", v.title)).size(20.0)).wrap());
        ui.add_space(4.0);
        let leave = |ui: &mut Ui, act: &mut MergeAct, label: &str| {
            if kit::ghost_button(ui, label).clicked() {
                *act = MergeAct::Cancel;
            }
        };
        match &v.phase {
            MergePhase::Fetching | MergePhase::Ready(_) => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(theme::muted("Looking at what changed in the circle..."));
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
                para(ui, "Your bank is changed only when this has arrived and a copy of your banks has been saved. If it fails, nothing changes.");
                leave(ui, &mut act, "Cancel");
            }
            MergePhase::Applied(d) => {
                kit::card().inner_margin(Margin::same(12)).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label(theme::strong(format!("Your copy is now in step with version {}", d.version)).size(14.5));
                    let o = &d.outcome;
                    line(ui, format!("{} taken from the circle, {} kept as yours, {} added, {} removed, {} kept as a second copy.", o.taken, o.kept, o.added, o.removed, o.both));
                    ui.add_space(4.0);
                    ui.label(theme::strong("Your way back").size(12.5));
                    ui.add(egui::Label::new(theme::text(format!("A copy of all your banks from just before the merge is saved at {}", api::plain(&d.restore))).size(12.0)).wrap());
                    line(ui, "To go back, close verSHion and copy that file over banks.json in the same folder.");
                });
                ui.add_space(6.0);
                if d.changes.total() > 0 || !d.changes.known {
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
            MergePhase::UpToDate => {
                body(ui, "You already have the circle's newest version. There is nothing to get.");
                leave(ui, &mut act, "Back");
            }
            MergePhase::StaleUpToDate { ask } => {
                body(ui, "The circle has nothing newer than the version your copy was in step with.");
                warn_box(ui, "But the bank on this computer is older than the record of it: a saved copy of your banks may have been put back. Sending is blocked until you decide.");
                match ask {
                    None => {
                        para(ui, "If the bank as it is now is what you want the circle to have, treat it as in step: nothing in the bank changes, and the next send sends it as it is, replacing the circle's questions with yours. Or let go of the link: the bank stays, but it is no longer connected to the circle's bank.");
                        ui.horizontal_wrapped(|ui| {
                            if ui.add_enabled(can_send, |ui: &mut Ui| kit::primary_button(ui, "Treat this copy as in step...")).clicked() {
                                act = MergeAct::AskRebase;
                            }
                            if kit::ghost_button(ui, "Forget this link...").clicked() {
                                act = MergeAct::AskForget;
                            }
                            leave(ui, &mut act, "Back");
                        });
                    }
                    Some(StaleAsk::Rebase) => {
                        warn_box(ui, "Are you sure? The next send will make the circle's newest version match THIS bank. If this bank is older than the circle's work (for example a saved copy was put back), other people's changes will no longer be in the newest version. They stay in the history, where they can be restored.");
                        ui.horizontal_wrapped(|ui| {
                            if ui.add_enabled(can_send, |ui: &mut Ui| kit::danger_text_button(ui, "Yes, treat it as in step")).clicked() {
                                act = MergeAct::Rebase;
                            }
                            if kit::ghost_button(ui, "No, go back").clicked() {
                                act = MergeAct::CancelAsk;
                            }
                        });
                    }
                    Some(StaleAsk::Forget) => {
                        warn_box(ui, "Are you sure? The bank stays on this computer, but it is no longer connected to the circle's bank: you can not send its changes or get the circle's newer versions. You can make a new working copy from the circle later.");
                        ui.horizontal_wrapped(|ui| {
                            if kit::danger_text_button(ui, "Yes, forget the link").clicked() {
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
                    Some(h) => warn_box(ui, &format!("The circle's newest version ({h}) is hidden while it is reviewed, so it can not be merged. It stays in the history.")),
                    None => warn_box(ui, "The circle's newest version is hidden while it is reviewed, so it can not be merged. It stays in the history."),
                }
                para(ui, "You can still send your version on top of it. That REPLACES what the circle has now with your bank: changes other people sent, and that you have not merged, will no longer be in the newest version. They stay in the history, where they can be restored. Only do it if your bank is what the circle should have.");
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
        MergeAct::SendNow => e.open_send(&local, perm),
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
    para(ui, &format!("Your copy is in step with version {}. The circle has version {}. Nothing has been changed yet. Go through the questions below, then press Apply merge. Cancel leaves everything as it is. A copy of your banks is saved just before anything is written.", plan.old_version, plan.new_version));
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
    if r.fallback {
        warn_box(ui, "The version your copy was made from is no longer kept, and this copy has no record of it, so this merge can only compare yours with the circle's newest version. Every question where the two differ is shown as a conflict for you to decide. Nothing is decided for you.");
    }
    let counts_row = [RowKind::Conflict, RowKind::TheirsChanged, RowKind::OursChanged, RowKind::NewInTheirs, RowKind::RemovedInTheirs, RowKind::RemovedLocally, RowKind::Unchanged];
    let summary: Vec<String> = counts_row.iter().filter(|k| plan.count(**k) > 0).map(|k| summary_label(*k, plan.count(*k))).collect();
    line(ui, summary.join(" - "));
    if plan.unlinked > 0 {
        para(ui, &format!("{} question{} in this bank {} yours alone (not in the circle's bank yet), so the merge leaves {} as {} {}.", plan.unlinked, if plan.unlinked == 1 { "" } else { "s" }, if plan.unlinked == 1 { "is" } else { "are" }, if plan.unlinked == 1 { "it" } else { "them" }, if plan.unlinked == 1 { "it" } else { "they" }, if plan.unlinked == 1 { "is" } else { "are" }));
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
                let (fill, fg) = if row.kind == RowKind::Conflict { (theme::error_wash(), theme::bad()) } else { (theme::accent_faded(), theme::text_col()) };
                pill(ui, kind_label(row.kind), fill, fg);
            });
            ui.add(egui::Label::new(theme::strong(&row.title).size(13.0)).wrap());
            if let Some(n) = &row.note {
                line(ui, circle_words(&api::plain(n)));
            }
            let (base_who, theirs_who) = (format!("In version {}", plan.old_version), format!("In version {} (the circle's newest)", plan.new_version));
            for d in &row.diffs {
                // Two-way: every field counts as changed (the earlier one is not known); only the ones
                // that really differ are worth reading.
                if r.fallback && d.ours == d.theirs {
                    continue;
                }
                ui.label(theme::strong(d.field).size(12.0));
                let rows: Vec<(&str, &String)> = if r.fallback { vec![("Yours now", &d.ours), (&theirs_who, &d.theirs)] } else { vec![(&base_who, &d.base), ("Yours now", &d.ours), (&theirs_who, &d.theirs)] };
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
    let unchanged = plan.rows.iter().filter(|r| r.kind == RowKind::Unchanged).count();
    if unchanged > 0 {
        line(ui, format!("{unchanged} unchanged"));
    }
    ui.add_space(8.0);
    let undecided = plan.undecided();
    if undecided > 0 {
        para(ui, &format!("{} conflict{} still need{} a decision before the merge can be applied.", undecided, if undecided == 1 { "" } else { "s" }, if undecided == 1 { "s" } else { "" }));
    }
    // Why Apply merge did not go through: next to the button that was pressed, not above a long review.
    if let Some(p) = &r.problem {
        warn_box(ui, p);
    }
    ui.horizontal(|ui| {
        if ui.add_enabled(e.can_apply_merge(), |ui: &mut Ui| kit::primary_button(ui, "Apply merge")).clicked() {
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
        let EditView::History(h) = &e.view else { return };
        if back_link(ui, "< Back") {
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
                    line(ui, format!("Includes a merge of the changes made since version {m}."));
                }
                let can_look = l.kept && !l.withheld;
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
    });
}

fn trim_panel(ui: &mut Ui, t: &TrimAsk, act: &mut HistAct) {
    section(ui, "Trim old versions", |ui| match &t.stage {
        TrimStage::Editing => {
            para(ui, "Keep only the newest versions in full. The older ones stay listed in the history, but their content is deleted, so they can not be opened or restored. This frees room if the circle's storage is full.");
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
            body(ui, &if d.pruned == 0 { "Nothing was deleted: no older version could be trimmed (one may be kept for a review).".to_string() } else { format!("Done. The content of {} older version{} was deleted.", d.pruned, if d.pruned == 1 { "" } else { "s" }) });
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
                para(ui, text::ANSWER_KEYS_NOTE);
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

#[derive(Clone, PartialEq)]
enum CardAct {
    None,
    Send(String),
    Merge(String),
    History(String, String, String),
    Forget(String),
    ConfirmForget(String),
    CancelForget,
}

/// One line about a working copy: how it stands against the circle.
fn standing(l: &CircleLink, st: Option<&LinkStatus>, removed: bool) -> Vec<String> {
    let mut v = Vec::new();
    if removed {
        v.push("This bank was removed from the circle, so nothing can be sent to it or merged from it. The owner or the bank's author may be able to bring it back.".to_string());
    }
    match st {
        Some(s) if s.missing => v.push("This bank is not on this computer any more.".to_string()),
        Some(s) if s.stale => v.push("The bank on this computer is older than the record of it (a saved copy may have been put back). Sending is blocked: press Get latest and merge, or forget the link.".to_string()),
        Some(s) if s.changes.total() > 0 => v.push(format!("Not sent yet: {}.", counts(s.changes.added as u32, s.changes.changed as u32, s.changes.removed as u32))),
        Some(s) if !s.changes.known => v.push("Unsent changes can not be counted for this copy.".to_string()),
        Some(_) => v.push("Nothing to send.".to_string()),
        None => {}
    }
    if l.behind() > 0 {
        v.push(behind_line(l.behind()));
    }
    v
}

/// The bank was removed from the circle (the owner's or author's choice, or a review).
fn bank_removed(m: &CirclesModel, bid: &str) -> bool {
    m.circle.as_ref().and_then(|c| c.detail.as_ref()).is_some_and(|d| d.banks.iter().any(|b| b.id == bid && b.removed.is_some()))
}

/// "Forget this link..." and, after one click, the question with its two answers.
fn forget_buttons(ui: &mut Ui, asked: bool, offer: bool, local: &str, act: &mut CardAct) {
    if asked {
        ui.add(egui::Label::new(theme::accent("Forget the link? The bank stays here, but it is no longer connected to the circle's bank.").size(12.5)).wrap());
        if kit::danger_text_button(ui, "Yes, forget it").clicked() {
            *act = CardAct::ConfirmForget(local.to_string());
        }
        if kit::ghost_button(ui, "Keep it").clicked() {
            *act = CardAct::CancelForget;
        }
    } else if offer && kit::ghost_button(ui, "Forget this link...").clicked() {
        *act = CardAct::Forget(local.to_string());
    }
}

/// "My working copies" for one circle: each with what to do next. Nothing is drawn without any.
pub(super) fn working_copies(ui: &mut Ui, m: &mut CirclesModel, cid: &str) {
    let links = m.edit.links_in(cid);
    if links.is_empty() {
        return;
    }
    let mut act = CardAct::None;
    let (role, closed) = m.circle.as_ref().map(|c| (c.role(), c.closed())).unwrap_or((Role::Viewer, false));
    section(ui, "My working copies", |ui| {
        para(ui, "Banks on this computer that are linked to a bank in this circle. Nothing goes to the circle until you press Send.");
        for l in &links {
            let st = m.edit.status(&l.local_bank_id).cloned();
            let missing = st.as_ref().is_some_and(|s| s.missing);
            kit::card().inner_margin(Margin::same(8)).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.add(egui::Label::new(theme::strong(api::plain(&l.title)).size(13.5)).wrap());
                line(ui, format!("Based on version {}.", l.base_version));
                let removed = bank_removed(m, &l.circle_bank_id);
                for s in standing(l, st.as_ref(), removed) {
                    ui.add(egui::Label::new(theme::accent(s).size(12.5)).wrap());
                }
                ui.horizontal_wrapped(|ui| {
                    let can_send = role.can_add_banks() && !closed && !missing && !removed;
                    let stale = st.as_ref().is_some_and(|s| s.stale);
                    // In place, not in a nested Ui: a nested one in a wrapping row squeezes the label.
                    // Greyed for a stale copy: the line above says sending is blocked until a merge.
                    if ui.add_enabled(can_send && !stale, |ui: &mut Ui| kit::primary_button(ui, "Send my changes...")).clicked() {
                        act = CardAct::Send(l.local_bank_id.clone());
                    }
                    if ui.add_enabled(can_send, |ui: &mut Ui| kit::ghost_button(ui, "Get latest and merge")).clicked() {
                        act = CardAct::Merge(l.local_bank_id.clone());
                    }
                    if kit::ghost_button(ui, "History of versions").clicked() {
                        act = CardAct::History(l.circle_id.clone(), l.circle_bank_id.clone(), l.title.clone());
                    }
                    forget_buttons(ui, m.edit.forget_asked(&l.local_bank_id), missing || st.as_ref().is_some_and(|s| s.stale), &l.local_bank_id, &mut act);
                });
            });
            ui.add_space(3.0);
        }
    });
    apply_card(m, act);
}

fn apply_card(m: &mut CirclesModel, act: CardAct) {
    match act {
        CardAct::None => {}
        CardAct::Send(local) => {
            if let Some(l) = m.edit.link_for_local(&local) {
                let p = m.perm_for(&l.circle_id, &l.circle_bank_id);
                m.edit.open_send(&local, p);
            }
        }
        CardAct::Merge(local) => {
            if let Some(l) = m.edit.link_for_local(&local) {
                let p = m.perm_for(&l.circle_id, &l.circle_bank_id);
                m.edit.open_merge(&local, p);
            }
        }
        CardAct::History(cid, bid, title) => {
            let p = m.perm_for(&cid, &bid);
            m.edit.open_history(&cid, &bid, &title, p);
        }
        CardAct::Forget(local) => m.edit.ask_forget(&local),
        CardAct::ConfirmForget(local) => m.edit.confirm_forget(&local),
        CardAct::CancelForget => m.edit.cancel_forget(),
    }
}

/// On a bank's page: where this computer's copy stands (when it has a working copy) and the history.
pub(super) fn bank_section(ui: &mut Ui, m: &mut CirclesModel, cid: &str, bid: &str, title: &str) {
    let link = m.edit.link_for_bank(cid, bid);
    let mut act = CardAct::None;
    let (role, closed) = m.circle.as_ref().map(|c| (c.role(), c.closed())).unwrap_or((Role::Viewer, false));
    section(ui, "Versions", |ui| {
        if let Some(l) = &link {
            let st = m.edit.status(&l.local_bank_id).cloned();
            let missing = st.as_ref().is_some_and(|s| s.missing);
            ui.label(theme::strong("You have a working copy of this bank").size(12.5));
            line(ui, format!("Based on version {}.", l.base_version));
            let removed = bank_removed(m, bid);
            for s in standing(l, st.as_ref(), removed) {
                ui.add(egui::Label::new(theme::accent(s).size(12.5)).wrap());
            }
            ui.horizontal_wrapped(|ui| {
                let can = role.can_add_banks() && !closed && !missing && !removed;
                let stale = st.as_ref().is_some_and(|s| s.stale);
                if ui.add_enabled(can && !stale, |ui: &mut Ui| kit::primary_button(ui, "Send my changes...")).clicked() {
                    act = CardAct::Send(l.local_bank_id.clone());
                }
                if ui.add_enabled(can, |ui: &mut Ui| kit::ghost_button(ui, "Get latest and merge")).clicked() {
                    act = CardAct::Merge(l.local_bank_id.clone());
                }
                forget_buttons(ui, m.edit.forget_asked(&l.local_bank_id), missing || st.as_ref().is_some_and(|s| s.stale), &l.local_bank_id, &mut act);
            });
            ui.add_space(4.0);
        }
        para(ui, "See every version that was sent, who sent it and when, look at an older one, and (for the owner or the bank's author) restore it.");
        if kit::ghost_button(ui, "History of versions").clicked() {
            act = CardAct::History(cid.to_string(), bid.to_string(), title.to_string());
        }
    });
    apply_card(m, act);
}

/// The "N new versions" line for a bank card on the circle page, when this computer has a working copy of it.
pub(super) fn bank_badge(m: &mut CirclesModel, cid: &str, bid: &str) -> Option<String> {
    let l = m.edit.link_for_bank(cid, bid)?;
    let st = m.edit.status(&l.local_bank_id);
    let mut bits = vec!["You have a working copy".to_string()];
    if l.behind() > 0 {
        bits.push(behind_line(l.behind()));
    }
    if st.is_some_and(|s| s.changes.total() > 0) {
        bits.push("unsent changes".into());
    }
    Some(bits.join(" - "))
}
