//! The "Share to pool..." window (egui). All logic lives in [`PublishModel`]; this only draws it.
//!
//! Entry points for the rest of the app, none of which needs the window to exist yet:
//!
//! - [`share_bank`] / [`share_exam`]: ask for the window from any screen with a `Context`.
//! - [`PublishUi::show`]: call once per frame (next to `AccountUi::show`); it does nothing and
//!   touches no network while the window is closed.
//!
//! Every string that came from the server is drawn as a plain label after [`api::plain`], never
//! as markup or a link.

use std::sync::Arc;

use egui::{Margin, RichText, Ui};
use web_time::Instant;

use super::api::{self, Status};
use super::model::Notice;
use super::pool::{ATTEST_STATEMENT, License, LabelChoice, LicenseRefusal, PayloadRefusal, Pos, PublishProgress, StudentBody, Visibility};
use super::publish::{Fix, Gate, ImageNote, PublishModel, Preview, ShareSource, Step, Target, snapshot_of};
use super::publish_text as text;
use super::ui::AccountUi;
use crate::model::{Exam, QuestionBank};
use crate::{kit, theme};

/// The App's handle on the publish window. Closed by default; does nothing until a source is started.
#[derive(Default)]
pub struct PublishUi {
    pub model: PublishModel,
    repaint_set: bool,
}

fn pending_id() -> egui::Id {
    egui::Id::new("share_publish_request")
}

/// Asks for the publish window for this source (it opens on the next frame).
pub fn request(ctx: &egui::Context, source: ShareSource) {
    ctx.data_mut(|d| d.insert_temp(pending_id(), Some(source)));
    ctx.request_repaint();
}

fn pending_circle_id() -> egui::Id {
    egui::Id::new("share_publish_request_circle")
}

/// Asks for the publish window for this source with a circle as the destination.
pub fn request_for_circle(ctx: &egui::Context, source: ShareSource, circle_id: &str) {
    ctx.data_mut(|d| d.insert_temp(pending_circle_id(), Some(circle_id.to_string())));
    request(ctx, source);
}

/// "Share to pool..." on a bank. Returns the sentence to show when there is nothing to share.
pub fn share_bank(ctx: &egui::Context, bank: &QuestionBank) -> Result<(), String> {
    request(ctx, ShareSource::from_bank(bank)?);
    Ok(())
}

/// "Share to pool..." on an exam (its questions are shared as a bank).
pub fn share_exam(ctx: &egui::Context, exam: &Exam, banks: &[QuestionBank]) -> Result<(), String> {
    request(ctx, ShareSource::from_exam(exam, banks)?);
    Ok(())
}

impl PublishUi {
    pub fn is_open(&self) -> bool {
        self.model.open
    }

    pub fn start(&mut self, source: ShareSource) {
        self.model.start(source);
    }

    /// Draws the window when open; returns at once (no I/O at all) when closed. `account` is the
    /// app's Account window: the sign-in is looked up through it, and "Open account" shows it.
    pub fn show(&mut self, ctx: &egui::Context, account: &mut AccountUi) {
        if let Some(Some(src)) = ctx.data_mut(|d| d.remove_temp::<Option<ShareSource>>(pending_id())) {
            self.model.start(src);
            if let Some(Some(cid)) = ctx.data_mut(|d| d.remove_temp::<Option<String>>(pending_circle_id())) {
                self.model.circle.activate(Some(cid));
            }
        }
        if !self.model.open {
            return;
        }
        if !self.repaint_set {
            let c = ctx.clone();
            self.model.set_repaint(Arc::new(move || c.request_repaint()));
            self.repaint_set = true;
        }
        let snap = snapshot_of(&account.model);
        self.model.frame(Instant::now(), &snap);
        if self.model.wants_account_check() {
            account.open_quiet();
            self.model.account_check_asked();
        }
        if self.model.take_open_account() {
            account.reveal();
        }
        if self.model.gate == Gate::Ready {
            account.close_quiet();
        }
        // The Account window is on top while it is open.
        if account.visible() {
            return;
        }
        if self.model.busy() || self.model.retry_wait().is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::new("share_publish"))
            .frame(kit::card().inner_margin(Margin::same(22)))
            .show(ctx, |ui| {
                ui.set_width(600.0_f32.min(ctx.content_rect().width() - 60.0).max(280.0));
                // A modal's area offers only last frame's size, so a plain scroll area keeps the height
                // of the first page it showed (one line while the account is checked) and the form
                // under it can not be seen. The minimum lets it grow to its content, up to the cap.
                let tall = ctx.content_rect().height() - 110.0;
                egui::ScrollArea::vertical().max_height(tall).min_scrolled_height(tall).show(ui, |ui| {
                    close = page(ui, &mut self.model);
                });
            });
        if close || (modal.should_close() && !self.model.busy()) {
            self.model.close();
        }
    }
}

// ---- small helpers -------------------------------------------------------------------------------

pub(super) fn para(ui: &mut Ui, s: &str) {
    ui.add(egui::Label::new(theme::muted(s).size(12.5)).wrap());
    ui.add_space(6.0);
}

pub(super) fn body(ui: &mut Ui, s: &str) {
    ui.add(egui::Label::new(theme::text(s).size(13.0)).wrap());
}

pub(super) fn heading(ui: &mut Ui, s: &str) {
    ui.add_space(8.0);
    ui.label(theme::strong(s).size(13.5));
}

pub(super) fn link(ui: &mut Ui, s: &str) -> bool {
    ui.add(egui::Button::new(theme::accent(s).size(12.5)).frame(false)).clicked()
}

pub(super) fn banner(ui: &mut Ui, n: &Notice) {
    let (t, fill, col) = match n {
        Notice::Info(t) => (t, theme::accent_faded(), theme::text_col()),
        Notice::Error(t) => (t, theme::error_wash(), theme::bad()),
    };
    egui::Frame::new().fill(fill).corner_radius(10).inner_margin(Margin::same(10)).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.add(egui::Label::new(RichText::new(t.as_str()).color(col).size(13.0)).wrap());
    });
    ui.add_space(8.0);
}

pub(super) fn warn_box(ui: &mut Ui, s: &str) {
    egui::Frame::new().fill(theme::error_wash()).stroke(egui::Stroke::new(1.5, theme::bad())).corner_radius(10).inner_margin(Margin::same(10)).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.add(egui::Label::new(RichText::new(s).color(theme::bad()).size(13.0)).wrap());
    });
    ui.add_space(8.0);
}

pub(super) fn field(ui: &mut Ui, label: &str, value: &mut String, id: &str, hint: &str) {
    ui.label(theme::strong(label).size(12.5));
    ui.add(egui::TextEdit::singleline(value).id(egui::Id::new(id)).hint_text(hint).desired_width(f32::INFINITY));
    ui.add_space(4.0);
}

fn title(model: &PublishModel) -> &'static str {
    match model.step {
        Step::Gate => "Share to the pool",
        Step::Options if model.circle.active => "Share to a circle",
        Step::Options => "Share to the pool",
        Step::Checking => "Checking...",
        Step::Review => "This is what students will see",
        Step::Fix => "Needs your decision",
        Step::Publishing => "Publishing...",
        Step::Done => "Shared",
        Step::Failed => "It did not finish",
    }
}

/// Returns true when the window should close.
pub(super) fn page(ui: &mut Ui, m: &mut PublishModel) -> bool {
    let mut close = false;
    ui.horizontal(|ui| {
        ui.label(theme::serif(title(m)).size(24.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let finished = m.step == Step::Done || (m.circle.active && matches!(m.circle.stage, super::circle_send::SendStage::Done(_)));
            if m.step != Step::Publishing && !m.circle.busy() && kit::ghost_button(ui, if finished { "Done" } else { "Close" }).clicked() {
                close = true;
            }
            if m.busy() {
                ui.spinner();
            }
        });
    });
    if let Some(s) = &m.source {
        ui.label(theme::muted(format!("{} - {} question{}", api::plain(&s.label), s.questions.len(), if s.questions.len() == 1 { "" } else { "s" })).size(12.0));
    }
    ui.add_space(6.0);
    if let Some(n) = m.notice.clone() {
        banner(ui, &n);
    }
    match m.step {
        Step::Gate => gate_page(ui, m),
        Step::Options if m.circle.active => circle_page(ui, m),
        Step::Options => options_page(ui, m),
        Step::Checking | Step::Publishing => busy_page(ui, m),
        Step::Review => review_page(ui, m),
        Step::Fix => fix_page(ui, m),
        Step::Failed => failed_page(ui, m),
        Step::Done => done_page(ui, m),
    }
    close
}

// ---- gate ---------------------------------------------------------------------------------------------

pub(super) fn status_line(s: &Status) -> (&'static str, &'static str) {
    match s {
        Status::Pending => ("Your account is waiting for approval.", "An administrator checks every account by hand before it can share. You can keep working; come back when it is approved."),
        Status::Approved => ("Your account is approved.", ""),
        Status::Rejected { revoked: false, .. } => ("Your account was not approved.", "You can read the reason in the Account window, and apply again if it allows."),
        Status::Rejected { revoked: true, .. } => ("Your account's approval was withdrawn.", "You can read the reason in the Account window."),
        Status::Suspended { .. } => ("Your account is suspended.", "It can not share while it is suspended."),
        Status::Unverified => ("Your account is not finished.", "Finish it with the link in the confirmation email."),
        Status::Other(_) => ("Your account state is not one this version of verSHion knows.", "Open the Account window for details."),
    }
}

fn gate_page(ui: &mut Ui, m: &mut PublishModel) {
    let mut open_account = false;
    match m.gate.clone() {
        Gate::Checking => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(theme::muted("Checking your account..."));
            });
        }
        Gate::NotSetUp => para(ui, crate::share::NOT_SET_UP),
        Gate::NoServer => {
            para(ui, "No pool server is set up yet. Open the Account window to enter one.");
            open_account = kit::primary_button(ui, "Open account...").clicked();
        }
        Gate::SignedOut => {
            body(ui, "Sharing needs an account. Sign in, or create one, in the Account window.");
            para(ui, "Everything else in verSHion keeps working without one. What you enter here is kept while you sign in.");
            open_account = kit::primary_button(ui, "Open account...").clicked();
        }
        Gate::Unreachable => {
            body(ui, "Your sign-in could not be checked. You may be offline.");
            para(ui, "Open the Account window to try again. Nothing was shared.");
            open_account = kit::primary_button(ui, "Open account...").clicked();
        }
        Gate::NotApproved(status) => {
            let (head, more) = status_line(&status);
            body(ui, head);
            if !more.is_empty() {
                para(ui, more);
            }
            open_account = kit::primary_button(ui, "Open account...").clicked();
        }
        Gate::Ready => {}
    }
    if open_account {
        m.request_open_account();
    }
}

// ---- options ------------------------------------------------------------------------------------------

/// "Where to?": the public pool or one of the person's circles.
fn destination(ui: &mut Ui, m: &mut PublishModel) {
    heading(ui, "Where do you want to share it?");
    let mut to_circle = m.circle.active;
    ui.add_enabled_ui(!m.circle.busy(), |ui| {
        ui.radio_value(&mut to_circle, false, "The public pool: other teachers can find it and copy it");
        ui.radio_value(&mut to_circle, true, "One of my circles: only its members can see it");
    });
    if to_circle != m.circle.active {
        if to_circle {
            m.circle.activate(None);
        } else {
            m.circle.deactivate();
        }
    }
}

fn options_page(ui: &mut Ui, m: &mut PublishModel) {
    destination(ui, m);
    if let Some(s) = &m.source
        && s.missing > 0
    {
        para(ui, &format!("{} question(s) of this exam could not be found in any bank and are not shared.", s.missing));
    }
    let republish = !matches!(m.target(), Target::New);
    if let Some(r) = m.record().cloned() {
        heading(ui, "Shared before");
        para(ui, &format!("You shared this on this device before, as item {} (version {}).", api::plain(&r.item_id), r.version));
        let mut update = m.form.update_existing;
        ui.radio_value(&mut update, true, "Update that item with these questions");
        ui.radio_value(&mut update, false, "Share a new, separate copy");
        if update != m.form.update_existing {
            m.set_update_existing(update);
        }
    }
    let republish = republish || matches!(m.target(), Target::Republish { .. });
    heading(ui, "Listing");
    field(ui, "Title", &mut m.form.title, "pub_title", "What teachers will see in the list");
    field(ui, "Subject", &mut m.form.subject, "pub_subject", "For example Biology");
    // Two equal columns: in a plain row the first field took the whole width and squeezed Grade
    // into a column one letter wide.
    ui.columns(2, |c| {
        field(&mut c[0], "Course", &mut m.form.course, "pub_course", "Optional");
        field(&mut c[1], "Grade", &mut m.form.grade, "pub_grade", "Optional");
    });
    field(ui, "Tags (separated by commas)", &mut m.form.tags, "pub_tags", "cells, mitosis");
    field(ui, "Standards (separated by commas)", &mut m.form.standards, "pub_standards", "Optional");

    heading(ui, "Who can see it");
    let mut vis = m.form.visibility;
    ui.add_enabled_ui(!republish, |ui| {
        ui.radio_value(&mut vis, Visibility::Public, text::PUBLIC_LINE);
        ui.radio_value(&mut vis, Visibility::Unlisted, text::UNLISTED_LINE);
    });
    if republish {
        para(ui, "An item that is already shared keeps its visibility. Change it later from your shared items.");
    }
    if vis != m.form.visibility {
        m.set_visibility(vis);
    }

    heading(ui, "Licence (you choose; nothing is preselected)");
    for info in text::LICENSES.iter() {
        if info.license == License::CircleOnly && m.form.visibility == Visibility::Public {
            continue;
        }
        let mut cur = m.form.license;
        // Each licence's line sits under its own button, not halfway to the next one.
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.radio_value(&mut cur, Some(info.license), info.name);
            ui.indent(("lic", info.name), |ui| ui.add(egui::Label::new(theme::muted(info.line).size(12.0)).wrap()));
        });
        ui.add_space(4.0);
        m.form.license = cur;
    }
    para(ui, text::PERMANENT_LINE);

    heading(ui, "Answer key");
    ui.checkbox(&mut m.form.hide_key, text::HIDE_KEY_LINE);
    if !m.form.hide_key {
        warn_box(ui, text::SHOW_KEY_LINE);
    }

    heading(ui, "Your confirmation");
    kit::card().inner_margin(Margin::same(12)).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.add(egui::Label::new(theme::text(ATTEST_STATEMENT).size(12.5)).wrap());
        ui.add_space(6.0);
        ui.checkbox(&mut m.form.attest, "Yes, I confirm this");
    });
    ui.add_space(10.0);
    let mut go = false;
    ui.add_enabled_ui(m.form.attest, |ui| {
        go = kit::primary_button(ui, "Next: check what students will see").clicked();
    });
    if !m.form.attest {
        para(ui, "Tick the confirmation to continue. Nothing is sent until you do.");
    }
    if go {
        m.check();
    }
}

// ---- checking / publishing ---------------------------------------------------------------------------------

fn progress_text(p: &Option<PublishProgress>, publishing: bool) -> String {
    match p {
        Some(PublishProgress::UploadingImage { done, total, .. }) => format!("Uploading picture {} of {}...", done + 1, total),
        Some(PublishProgress::ResendingImages { count }) => format!("Sending {count} picture{} again...", if *count == 1 { "" } else { "s" }),
        Some(PublishProgress::Sending) | None => if publishing { "Sending to the pool...".into() } else { "Checking with the pool...".into() },
    }
}

fn busy_page(ui: &mut Ui, m: &mut PublishModel) {
    let publishing = m.step == Step::Publishing;
    ui.horizontal(|ui| {
        ui.spinner();
        ui.label(theme::text(progress_text(&m.progress, publishing)));
    });
    if !publishing && m.source.as_ref().is_some_and(|s| s.questions.iter().any(|q| q.image.is_some())) {
        para(ui, text::PICTURE_UPLOAD_LINE);
    }
    if publishing {
        para(ui, "Stopping only stops waiting. If the request already reached the server it may still be shared; sending it again later is safe.");
    }
    ui.add_space(6.0);
    if kit::ghost_button(ui, "Cancel").clicked() {
        m.cancel();
    }
}

// ---- review ----------------------------------------------------------------------------------------------------

pub(super) fn body_lines(ui: &mut Ui, b: &StudentBody) {
    match b {
        StudentBody::Mc { choices } => {
            for (i, c) in choices.iter().enumerate() {
                let lock = if c.locked { "  (stays in place)" } else { "" };
                ui.label(theme::text(format!("   {}. {}{}", i + 1, text::shown(&c.text), lock)).size(12.5));
            }
        }
        StudentBody::TrueFalse { .. } => {
            ui.label(theme::muted("   True / False").size(12.5));
        }
        StudentBody::Matching { left, word_bank, .. } => {
            for l in left {
                ui.label(theme::text(format!("   - {}", text::shown(l))).size(12.5));
            }
            if !word_bank.is_empty() {
                ui.label(theme::muted(format!("   Word bank: {}", word_bank.iter().map(|w| text::shown(w)).collect::<Vec<_>>().join(", "))).size(12.5));
            }
        }
        StudentBody::Blank { blanks, .. } => {
            ui.label(theme::muted(format!("   {blanks} blank{} to fill in", if *blanks == 1 { "" } else { "s" })).size(12.5));
        }
        StudentBody::Short { .. } => {
            ui.label(theme::muted("   Written answer").size(12.5));
        }
        StudentBody::Multipart { parts } => {
            for (i, p) in parts.iter().enumerate() {
                ui.label(theme::strong(format!("   Part {}: {}", i + 1, text::shown(&p.stem))).size(12.5));
                body_lines(ui, &p.body);
            }
        }
    }
}

fn picture_state(images: &[ImageNote], q: usize) -> Option<bool> {
    images.iter().find(|i| i.question == q).map(|i| i.kept)
}

fn review_page(ui: &mut Ui, m: &mut PublishModel) {
    let current = m.preview_is_current();
    let Some(p) = m.preview.clone() else {
        if kit::ghost_button(ui, "Back").clicked() {
            m.back_to_options();
        }
        return;
    };
    if let Some(b) = &p.blocked {
        warn_box(ui, b);
    }
    if !current {
        warn_box(ui, "You changed something after this preview was made. Check again before publishing.");
    }
    summary(ui, m, &p);
    if p.options.hide_key {
        para(ui, text::SHUFFLE_LINE);
    } else {
        warn_box(ui, "The answer key is NOT hidden. The page below shows what is shared, correct answers included.");
    }
    changes_made(ui, m, &p);
    heading(ui, "The questions");
    egui::Frame::new().stroke(egui::Stroke::new(1.0, theme::muted_col())).corner_radius(8).inner_margin(Margin::same(8)).show(ui, |ui| {
        egui::ScrollArea::vertical().id_salt("pub_review_q").max_height(300.0).show(ui, |ui| {
            ui.set_min_width(ui.available_width() - 16.0);
            for q in &p.view.questions {
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(theme::strong(format!("{}.", q.number)).size(13.0));
                    ui.label(theme::text(text::shown(&q.stem)).size(13.0));
                });
                match picture_state(&p.images, q.number - 1) {
                    Some(true) => {
                        ui.label(theme::muted("   [a picture is shown here]").size(12.0));
                    }
                    Some(false) => {
                        ui.label(theme::muted("   [the picture is removed]").size(12.0));
                    }
                    None => {}
                }
                body_lines(ui, &q.body);
                if !p.options.hide_key
                    && let StudentBody::Mc { choices } = &q.body
                {
                    let right: Vec<String> = choices.iter().enumerate().filter(|(_, c)| c.correct == Some(true)).map(|(i, _)| (i + 1).to_string()).collect();
                    if !right.is_empty() {
                        ui.label(theme::muted(format!("   Correct: {}", right.join(", "))).size(12.0));
                    }
                }
            }
        });
    });
    heading(ui, "Before you publish");
    if let Some(lc) = &p.result.license_check
        && lc.status != super::pool::LicenseStatus::Ok
        && let Some(r) = &lc.reason
    {
        para(ui, &format!("Licence check: {}", text::shown(r)));
    }
    let blocker = m.publish_blocker();
    ui.add_enabled_ui(p.blocked.is_none() && current, |ui| {
        ui.checkbox(&mut m.form.reviewed, if p.options.hide_key { "I looked at this. This is what students will see." } else { "I looked at this. I understand the answers are shown." });
    });
    ui.add_space(8.0);
    let (mut go, mut back, mut again) = (false, false, false);
    ui.horizontal_wrapped(|ui| {
        ui.add_enabled_ui(blocker.is_none(), |ui| {
            let label = if matches!(p.target, Target::New) { "Publish to the pool" } else { "Update the shared item" };
            let r = kit::primary_button(ui, label);
            go = r.clicked();
            if let Some(b) = blocker {
                r.on_disabled_hover_text(b);
            }
        });
        if !current {
            again = kit::ghost_button(ui, "Check again").clicked();
        }
        back = kit::ghost_button(ui, "Back to options").clicked();
    });
    if let Some(b) = blocker {
        para(ui, b);
    }
    if go {
        m.publish();
    }
    if again {
        m.check();
    }
    if back {
        m.back_to_options();
    }
}

fn summary(ui: &mut Ui, m: &PublishModel, p: &Preview) {
    let vis = match p.options.visibility {
        Visibility::Public => "Public",
        Visibility::Unlisted => "Unlisted (only you)",
    };
    let target = match &p.target {
        Target::New => "A new item".to_string(),
        Target::Republish { version, .. } => format!("An update of your item (now version {version})"),
    };
    let _ = m;
    ui.horizontal_wrapped(|ui| {
        kit::pill(ui, vis, theme::accent_faded(), theme::text_col());
        kit::pill(ui, &text::license_name(p.options.license.as_str()), theme::accent_faded(), theme::text_col());
        kit::pill(ui, &target, theme::accent_faded(), theme::text_col());
        kit::pill(ui, if p.options.hide_key { "Answer key hidden" } else { "Answer key shown" }, theme::accent_faded(), theme::text_col());
    });
}

fn changes_made(ui: &mut Ui, m: &mut PublishModel, p: &Preview) {
    let r = &p.result;
    let single = false;
    let mut lines: Vec<String> = Vec::new();
    for at in &r.labels_removed {
        lines.push(format!("Letter labels (A, B, C ...) are removed from the choices of {}.", text::question_label(at.question, single).to_lowercase()));
    }
    for o in &r.order_reasons {
        lines.push(format!("{}: the choices keep your order, because {}.", text::question_label(o.question, single), text::order_reason_text(&o.reason)));
    }
    for q in &r.figures_removed {
        lines.push(format!("{}: the editable figure is not shared (only its picture, if you keep it).", text::question_label(*q, single)));
    }
    for n in &r.notes {
        lines.push(text::note_text(n));
    }
    if !lines.is_empty() {
        heading(ui, "What was changed in the shared copy");
        for l in lines {
            ui.add(egui::Label::new(theme::text(format!("- {l}")).size(12.5)).wrap());
        }
    }
    if !p.images.is_empty() {
        heading(ui, "Pictures");
        let mut toggles: Vec<(usize, bool)> = Vec::new();
        for img in &p.images {
            let name = m.question_title(img.question);
            if p.options.hide_key {
                let mut keep = m.form.image_kept(img.question);
                let r = ui.checkbox(&mut keep, format!("{name}: keep the picture for students (it does not show the answer)"));
                if r.changed() {
                    toggles.push((img.question, keep));
                }
                ui.label(theme::muted(if img.kept { "   Students will see it." } else { "   Students will not see it (removed by default)." }).size(12.0));
            } else {
                ui.label(theme::text(format!("{name}: the picture is shared.")).size(12.5));
            }
        }
        for (q, keep) in toggles {
            m.set_keep_image(q, keep);
        }
    }
}

// ---- the refusal loop ---------------------------------------------------------------------------------------------

fn fix_page(ui: &mut Ui, m: &mut PublishModel) {
    match m.fix.clone() {
        Some(Fix::Payload(p)) => payload_fix(ui, m, &p),
        Some(Fix::License(l)) => license_fix(ui, m, &l),
        Some(Fix::Conflict { current_version, current_publish_id }) => {
            body(ui, "This item was updated from another device or tab since you last loaded it.");
            if let Some(v) = current_version {
                para(ui, &format!("The version on the server is now {v}."));
            }
            para(ui, "If you continue, your version replaces that newer one. Choose only if that is what you want.");
            let can = current_publish_id.is_some();
            let (mut replace, mut back) = (false, false);
            ui.horizontal_wrapped(|ui| {
                ui.add_enabled_ui(can, |ui| replace = kit::primary_button(ui, "Replace it with my version").clicked());
                back = kit::ghost_button(ui, "Cancel").clicked();
            });
            if replace {
                m.replace_newer_version();
            }
            if back {
                m.back_to_options();
            }
        }
        Some(Fix::CircleMaterial(c)) => circle_material_fix(ui, m, &c),
        Some(Fix::Gone { message }) => {
            body(ui, &message);
            para(ui, "You can share these questions again as a new item.");
            let (mut new, mut back) = (false, false);
            ui.horizontal_wrapped(|ui| {
                new = kit::primary_button(ui, "Share as a new item").clicked();
                back = kit::ghost_button(ui, "Back to options").clicked();
            });
            if new {
                m.publish_as_new();
            }
            if back {
                m.back_to_options();
            }
        }
        None => {
            if kit::ghost_button(ui, "Back to options").clicked() {
                m.back_to_options();
            }
        }
    }
}

fn payload_fix(ui: &mut Ui, m: &mut PublishModel, p: &PayloadRefusal) {
    let todo = m.todo();
    para(ui, "The pool checked these questions and needs you to look at the items below. Nothing was shared.");
    // 1. things that must change in the question
    let hard: Vec<(String, Option<String>, bool)> =
        m.hard_issues().into_iter().map(|i| (text::issue_text(i, false), i.hint.clone(), i.author_only)).collect();
    if !hard.is_empty() {
        heading(ui, "Fix these in the question");
        para(ui, "These can not be acknowledged. Close this window, change the question, then share again. Or go back and turn off \"hide the answer key\" if you mean to show the answers.");
        for (t, hint, author_only) in &hard {
            egui::Frame::new().fill(theme::error_wash()).corner_radius(8).inner_margin(Margin::same(8)).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.add(egui::Label::new(RichText::new(t.as_str()).color(theme::bad()).size(12.5)).wrap());
                if let Some(h) = hint {
                    ui.add(egui::Label::new(theme::text(api::plain(h)).size(12.0)).wrap());
                }
                if *author_only {
                    ui.label(theme::muted("Only you see this.").size(11.5));
                }
            });
            ui.add_space(4.0);
        }
    }
    // 2. low-risk formatting: one acknowledgement per question
    let ackable = m.ackable_questions();
    if !ackable.is_empty() {
        heading(ui, "Check these choices");
        para(ui, "These might give the answer away, but may be fine. Look at each one. Tick it only if it does not show the answer.");
        let total = p.formatting_findings_total;
        if total > p.formatting_findings.len() {
            para(ui, &format!("Showing {} of {} findings.", p.formatting_findings.len(), total));
        }
        for q in ackable {
            let mut yes = m.form.formatting_acked(q);
            ui.label(theme::strong(m.question_title(q)).size(12.5));
            for i in p.issues.iter().filter(|i| i.question_index() == Some(q)) {
                ui.add(egui::Label::new(theme::text(format!("   {}", text::issue_text(i, false))).size(12.0)).wrap());
                if let Some(h) = &i.hint {
                    ui.add(egui::Label::new(theme::muted(format!("   {}", api::plain(h))).size(11.5)).wrap());
                }
            }
            for f in p.formatting_findings.iter().filter(|f| f.question == q) {
                if let Some(s) = &f.served {
                    ui.label(theme::muted(format!("   As students see it: \"{}\"", text::shown(s))).size(12.0));
                }
            }
            if ui.checkbox(&mut yes, "I checked: this does not give the answer away").changed() {
                m.set_formatting_ack(q, yes);
            }
            ui.add_space(4.0);
        }
    }
    // 3. letter labels
    if !p.label_findings.is_empty() {
        heading(ui, "Letter labels on choices");
        para(ui, "Some choices start with letters (A, B, C ...). Strip them, or keep them. Kept labels can freeze the order of the choices.");
        for f in &p.label_findings {
            let at = Pos { question: f.question, part: f.part };
            let mut name = m.question_title(f.question);
            if let Some(part) = f.part {
                name.push_str(&format!(", part {}", part + 1));
            }
            ui.label(theme::strong(name).size(12.5));
            for c in f.choices.iter().take(6) {
                ui.label(theme::muted(format!("   \"{}\"  becomes  \"{}\"", text::shown(&c.before), text::shown(&c.after))).size(12.0));
            }
            if f.truncated || f.choices.len() > 6 {
                ui.label(theme::muted("   ...").size(12.0));
            }
            if f.order_revealing {
                para(ui, "   Keeping these letters would show the order, so the order stays fixed.");
            }
            let cur = m.form.label(at).or(f.decision);
            ui.horizontal(|ui| {
                if f.strip_allowed && ui.selectable_label(cur == Some(LabelChoice::Strip), "Strip the letters").clicked() {
                    m.decide_label(at, LabelChoice::Strip);
                }
                if ui.selectable_label(cur == Some(LabelChoice::Keep), "Keep the letters").clicked() {
                    m.decide_label(at, LabelChoice::Keep);
                }
            });
            ui.add_space(4.0);
        }
    }
    // 4. order acknowledgements
    if !p.needs_order_ack.is_empty() {
        heading(ui, "Choices that stay in your order");
        para(ui, "These choices will not be shuffled. Students will see them in the order you wrote them. Confirm that is fine.");
        for at in &p.needs_order_ack {
            let mut name = m.question_title(at.question);
            if let Some(part) = at.part {
                name.push_str(&format!(", part {}", part + 1));
            }
            let why = p.order_reasons.iter().find(|o| (o.question, o.part) == (at.question, at.part)).map(|o| text::order_reason_text(&o.reason));
            let mut yes = m.form.order_acked(*at);
            if ui.checkbox(&mut yes, format!("{name} stays in this order{}", why.map(|w| format!(" ({w})")).unwrap_or_default())).changed() {
                m.set_order_ack(*at, yes);
            }
        }
        if p.needs_order_ack.len() > 1 && link(ui, "Confirm all of them") {
            m.ack_all_orders();
        }
    }
    // 5. anything else the server said
    // (every issue is either shown above or is a decision/ack item)
    ui.add_space(8.0);
    let left = [(todo.hard, "to fix in the question"), (todo.formatting, "to check"), (todo.labels, "letter decisions"), (todo.orders, "order confirmations")]
        .into_iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, w)| format!("{n} {w}"))
        .collect::<Vec<_>>();
    if !left.is_empty() {
        para(ui, &format!("Still to do: {}.", left.join(", ")));
    }
    let (mut again, mut back) = (false, false);
    ui.horizontal_wrapped(|ui| {
        ui.add_enabled_ui(todo.done(), |ui| again = kit::primary_button(ui, "Check again").clicked());
        back = kit::ghost_button(ui, "Back to options").clicked();
    });
    if again {
        m.recheck();
    }
    if back {
        m.back_to_options();
    }
}

fn license_fix(ui: &mut Ui, m: &mut PublishModel, l: &LicenseRefusal) {
    let message = l.message.as_deref().map(text::shown).unwrap_or_else(|| "The licence you chose is not allowed for material this is built on.".into());
    body(ui, &message);
    for r in &l.relicensed {
        para(
            ui,
            &format!(
                "Item {} was relicensed after you received it: it was {}, and is now {}.",
                api::plain(&r.item_id),
                r.earlier.iter().map(|e| text::license_name(e)).collect::<Vec<_>>().join(" or "),
                text::license_name(&r.now)
            ),
        );
    }
    heading(ui, "Which questions, and whose terms bind them");
    for q in &l.per_question {
        ui.label(theme::strong(m.question_title(q.pos)).size(12.5));
        if q.bound_by.is_empty() {
            ui.label(theme::muted("   Built on something that limits the licence.").size(12.0));
        }
        for b in &q.bound_by {
            let who = if b.author_name.is_empty() { "another teacher".to_string() } else { text::shown(&b.author_name) };
            let what = if b.title.is_empty() { "an item".to_string() } else { format!("\"{}\"", text::shown(&b.title)) };
            let lic = b.license.as_deref().map(text::license_name).map(|n| format!(", licensed {n}")).unwrap_or_default();
            let id = b.item_id.as_deref().map(|i| format!(" (item {})", api::plain(i))).unwrap_or_default();
            ui.add(egui::Label::new(theme::text(format!("   Bound by {what} from {who}{lic}{id}.")).size(12.0)).wrap());
            if let Some(n) = &b.note {
                ui.label(theme::muted(format!("   {}", text::shown(n))).size(12.0));
            }
            if let Some(v) = &b.via {
                ui.label(theme::muted(format!("   Through a copy that was relabelled: {}", text::shown(v))).size(12.0));
            }
        }
        if q.evidence_overflow {
            ui.label(theme::muted("   More items bind this question than are listed.").size(12.0));
        }
        if !q.allowed.is_empty() {
            ui.label(theme::muted(format!("   Allowed for it: {}", q.allowed.iter().map(|a| text::license_name(a)).collect::<Vec<_>>().join("; "))).size(12.0));
        }
    }
    heading(ui, "What you can do");
    let usable: Vec<License> = l.allowed.iter().filter_map(|a| License::parse(a)).filter(|a| !(*a == License::CircleOnly && m.form.visibility == Visibility::Public)).collect();
    if usable.is_empty() {
        para(ui, "No single licence works for all of these questions. Split the bank into separate items, or, if you wrote these questions yourself and only looked at the items above, say so below.");
    } else {
        para(ui, "Pick a licence that every question allows:");
        ui.horizontal_wrapped(|ui| {
            for lic in usable {
                if kit::ghost_button(ui, text::license_info(lic).name).clicked() {
                    m.choose_allowed_license(lic);
                }
            }
        });
    }
    heading(ui, "Or: I wrote these myself");
    para(ui, "Only if the questions are really your own work, written without copying the items above. Your words are kept and an administrator may read them.");
    field(ui, "Items you only looked at (ids or links)", &mut m.form.independent_items, "pub_ind_items", "Paste the item ids");
    field(ui, "In a sentence: why these are your own work", &mut m.form.independent_statement, "pub_ind_stmt", "At least 10 characters");
    let (mut claim, mut back) = (false, false);
    ui.horizontal_wrapped(|ui| {
        claim = kit::ghost_button(ui, "Send this statement and check again").clicked();
        back = kit::ghost_button(ui, "Back to options").clicked();
    });
    if claim {
        m.claim_independence();
    }
    if back {
        m.back_to_options();
    }
}

// ---- failure and success --------------------------------------------------------------------------------------------

fn failed_page(ui: &mut Ui, m: &mut PublishModel) {
    let Some(f) = m.failure.clone() else {
        if kit::ghost_button(ui, "Back to options").clicked() {
            m.back_to_options();
        }
        return;
    };
    warn_box(ui, &f.message);
    let wait = m.retry_wait();
    let (mut retry, mut back, mut account) = (false, false, false);
    ui.horizontal_wrapped(|ui| {
        if f.retry.is_some() {
            ui.add_enabled_ui(wait.is_none(), |ui| retry = kit::primary_button(ui, "Try again").clicked());
        }
        if f.account {
            account = kit::ghost_button(ui, "Open account...").clicked();
        }
        back = kit::ghost_button(ui, "Back to options").clicked();
    });
    if let Some(w) = wait {
        para(ui, &format!("You can try again in about {} seconds.", w.as_secs().max(1)));
    }
    if retry {
        m.retry();
    }
    if account {
        m.request_open_account();
    }
    if back {
        m.back_to_options();
    }
}

fn done_page(ui: &mut Ui, m: &mut PublishModel) {
    let Some(d) = m.done.clone() else { return };
    let r = &d.result;
    body(ui, if d.republish { "Your shared item was updated." } else { "Your questions are shared." });
    if r.replayed {
        para(ui, "The server had already received this very publish, so nothing new was made.");
    }
    ui.add_space(6.0);
    kit::card().inner_margin(Margin::same(12)).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(theme::strong(text::shown(&d.title)).size(14.0));
        ui.horizontal_wrapped(|ui| {
            ui.label(theme::muted("Item id:").size(12.5));
            ui.add(egui::Label::new(RichText::new(api::plain(&r.item_id)).family(egui::FontFamily::Monospace).size(12.0)).selectable(true));
        });
        ui.label(theme::muted(format!("Version {} - {} - {}", r.version, if d.public() { "Public" } else { "Unlisted: only you can see it" }, text::license_name(&r.license))).size(12.5));
        ui.label(theme::muted(if r.hide_key { "The answer key is hidden from other teachers." } else { "The answer key is shown." }).size(12.5));
    });
    if let Some(dup) = &r.possible_duplicate_of {
        ui.add_space(6.0);
        para(ui, &format!("The pool thinks this may duplicate item {}. It was shared anyway; an administrator may look.", api::plain(dup)));
    }
    for n in &r.dropped {
        para(ui, &text::note_text(n));
    }
    ui.add_space(8.0);
    if kit::ghost_button(ui, "My shared items").clicked() {
        super::mine_ui::request(ui.ctx(), super::mine_ui::MineRequest::Item(r.item_id.clone()));
        m.close();
    }
    para(ui, "You can update this item later by choosing Share to pool… in the same bank's Share menu.");
}

// ---- the circle destination ------------------------------------------------------------------------------------

fn circle_page(ui: &mut Ui, m: &mut PublishModel) {
    use super::circle_send::{CIRCLE_ONLY_LINE, CircleList, KEYS_NOTE, NEW_BANK_NOTE, RESHARE_NOTE, Refused, SendStage};
    // (once it is in the circle there is no destination left to choose)
    if !matches!(m.circle.stage, SendStage::Done(_)) {
        destination(ui, m);
    }
    if m.circle.busy() || matches!(m.circle.stage, SendStage::Sending(_)) {
        let t = match &m.circle.stage {
            SendStage::Sending(Some(p)) => progress_text(&Some(p.clone()), true).replace("the pool", "the circle"),
            _ => "Sending to the circle...".into(),
        };
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(theme::text(t));
        });
        return;
    }
    match &m.circle.stage {
        SendStage::Done(info) => {
            let info = (**info).clone();
            body(ui, &format!("\"{}\" is now in the circle \"{}\".", text::shown(&info.title), api::plain(&info.circle_name)));
            para(ui, &format!("{} question{} added. Its members can open it and get a copy; the answer keys are visible to them.", info.sent.added, if info.sent.added == 1 { "" } else { "s" }));
            if info.sent.replayed {
                para(ui, "The pool already had this send, so nothing was added twice.");
            }
            if kit::primary_button(ui, "Open the circle").clicked() {
                crate::share::shared_ui::open_circle(ui.ctx(), &info.circle_id, Some(&info.sent.bank_id));
                m.close();
            }
            return;
        }
        SendStage::Failed(p) => {
            let p = p.clone();
            warn_box(ui, &p.message);
            let (mut retry, mut back, mut account) = (false, false, false);
            ui.horizontal_wrapped(|ui| {
                if p.retry {
                    ui.add_enabled_ui(p.retry_at.is_none_or(|t| t <= Instant::now()), |ui| retry = kit::primary_button(ui, "Try again").clicked());
                }
                if p.account {
                    account = kit::ghost_button(ui, "Open account...").clicked();
                }
                back = kit::ghost_button(ui, "Back to options").clicked();
            });
            if account {
                m.request_open_account();
            }
            if retry {
                m.circle.back_to_editing();
                m.send_to_circle();
            }
            if back {
                m.circle.back_to_editing();
            }
            return;
        }
        SendStage::Refused(Refused::Payload(p)) => {
            let p = p.clone();
            para(ui, "The pool checked these questions and needs you to change the items below. Nothing was shared.");
            for i in p.issues.iter().take(30) {
                let t = text::issue_text(i, false);
                egui::Frame::new().fill(theme::error_wash()).corner_radius(8).inner_margin(Margin::same(8)).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.add(egui::Label::new(RichText::new(t).color(theme::bad()).size(12.5)).wrap());
                    if let Some(h) = &i.hint {
                        ui.add(egui::Label::new(theme::text(api::plain(h)).size(12.0)).wrap());
                    }
                });
                ui.add_space(3.0);
            }
            if p.issues.len() > 30 {
                para(ui, &format!("... and {} more.", p.issues.len() - 30));
            }
            para(ui, "Close this window, change the questions, then share again.");
            if kit::ghost_button(ui, "Back to options").clicked() {
                m.circle.back_to_editing();
            }
            return;
        }
        SendStage::Refused(Refused::License(l)) => {
            let l = l.clone();
            body(ui, &l.message.as_deref().map(text::shown).unwrap_or_else(|| "The licence you chose is not allowed for material these questions are built on.".into()));
            let usable: Vec<License> = l.allowed.iter().filter_map(|a| License::parse(a)).collect();
            if usable.is_empty() {
                para(ui, "No single licence works for all of these questions. Split the bank into separate banks.");
            } else {
                para(ui, "Pick a licence that every question allows:");
                ui.horizontal_wrapped(|ui| {
                    for lic in usable {
                        if kit::ghost_button(ui, text::license_info(lic).name).clicked() {
                            m.circle.choose_allowed(lic);
                        }
                    }
                });
            }
            if kit::ghost_button(ui, "Back to options").clicked() {
                m.circle.back_to_editing();
            }
            return;
        }
        SendStage::Editing | SendStage::Sending(_) => {}
    }

    // Which circle.
    heading(ui, "Which circle?");
    let mut retry_list = false;
    match &m.circle.list {
        CircleList::Idle | CircleList::Loading => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(theme::muted("Looking up your circles..."));
            });
        }
        CircleList::Failed(p) => {
            warn_box(ui, &p.message);
            retry_list = kit::ghost_button(ui, "Try again").clicked();
        }
        CircleList::Ready(_) => {
            let choices: Vec<(String, String, super::circles_api::Role)> = m.circle.choices().iter().map(|c| (c.id.clone(), api::plain(&c.name), c.role())).collect();
            if choices.is_empty() {
                body(ui, "You are not in a circle you can add banks to.");
                para(ui, "Make a circle, or ask its owner to make you an editor. Circles are in Shared, under Circles.");
                if kit::ghost_button(ui, "Open Circles").clicked() {
                    crate::share::shared_ui::goto(ui.ctx(), crate::share::shared::Tab::Circles);
                    m.close();
                }
            } else {
                let mut cur = m.circle.chosen.clone();
                for (id, name, role) in &choices {
                    ui.radio_value(&mut cur, Some(id.clone()), format!("{name}  ({})", role.name()));
                }
                if cur != m.circle.chosen
                    && let Some(id) = cur
                {
                    m.circle.choose(&id);
                }
            }
        }
    }
    if retry_list {
        m.circle.retry_list();
    }
    if m.circle.choices().is_empty() && matches!(m.circle.list, CircleList::Ready(_)) {
        return;
    }

    heading(ui, "Listing");
    field(ui, "Title", &mut m.form.title, "pub_title", "What the members will see in the list");
    field(ui, "Subject", &mut m.form.subject, "pub_subject", "For example Biology");
    ui.columns(2, |c| {
        field(&mut c[0], "Course", &mut m.form.course, "pub_course", "Optional");
        field(&mut c[1], "Grade", &mut m.form.grade, "pub_grade", "Optional");
    });
    field(ui, "Tags (separated by commas)", &mut m.form.tags, "pub_tags", "cells, mitosis");

    heading(ui, "Licence (you choose; nothing is preselected)");
    for info in text::LICENSES.iter() {
        let mut cur = m.circle.license;
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.radio_value(&mut cur, Some(info.license), info.name);
            // (the public list's line for "All rights reserved" is about unlisted items, not circles)
            let line = if info.license == License::CircleOnly { CIRCLE_ONLY_LINE } else { info.line };
            ui.indent(("circ_lic", info.name), |ui| ui.add(egui::Label::new(theme::muted(line).size(12.0)).wrap()));
        });
        ui.add_space(4.0);
        m.circle.license = cur;
    }
    para(ui, RESHARE_NOTE);

    heading(ui, "Answer keys");
    warn_box(ui, KEYS_NOTE);

    heading(ui, "A note for the circle (optional)");
    ui.add(egui::TextEdit::multiline(&mut m.circle.message).id(egui::Id::new("circle_send_message")).desired_rows(2).desired_width(f32::INFINITY));
    para(ui, NEW_BANK_NOTE);

    heading(ui, "Your confirmation");
    kit::card().inner_margin(Margin::same(12)).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.add(egui::Label::new(theme::text(ATTEST_STATEMENT).size(12.5)).wrap());
        ui.add_space(6.0);
        ui.checkbox(&mut m.circle.attest, "Yes, I confirm this");
    });
    ui.add_space(10.0);
    let why = m.circle.blocker();
    let mut go = false;
    ui.add_enabled_ui(why.is_none(), |ui| go = kit::primary_button(ui, "Share to the circle").clicked());
    if let Some(w) = why {
        para(ui, w);
    }
    if go {
        m.send_to_circle();
    }
}

/// "Question 3 was", "Questions 1 and 2 were", "Questions 1, 2 and 5 were" (1-based numbers).
fn questions_were(positions: &[usize]) -> String {
    match positions {
        [one] => format!("Question {one} was"),
        [rest @ .., last] => format!("Questions {} and {last} were", rest.iter().map(usize::to_string).collect::<Vec<_>>().join(", ")),
        [] => "Some questions were".to_string(),
    }
}

/// D13: questions first shared by another member of a circle.
fn circle_material_fix(ui: &mut Ui, m: &mut PublishModel, c: &super::pool::CircleMaterial) {
    body(ui, "Some of these questions can not be shared publicly from here.");
    para(ui, "They were first shared by another member of a circle you are in, or were in. Nothing was shared.");
    for b in c.banks() {
        let positions: Vec<usize> = c.hits.iter().filter(|h| h.bank_id == b.bank_id).map(|h| h.pos + 1).collect();
        let list = questions_were(&positions);
        if b.bank_id.is_empty() {
            ui.add(egui::Label::new(theme::text(format!("{list} first shared by another member of a circle. The pool does not say which bank.")).size(13.0)).wrap());
            ui.add_space(3.0);
            continue;
        }
        let bank = b.bank_title.as_deref().map(api::plain).filter(|t| !t.is_empty()).map(|t| format!("the circle bank \"{t}\"")).unwrap_or_else(|| "a circle bank".into());
        let circle = b.circle_name.as_deref().map(api::plain).filter(|t| !t.is_empty()).map(|t| format!(" ({t})")).unwrap_or_default();
        ui.add(egui::Label::new(theme::text(format!("{list} first shared by another member, in {bank}{circle}.")).size(13.0)).wrap());
        ui.add_space(3.0);
    }
    if c.positions.len() > c.hits.len() {
        let more = c.positions.len() - c.hits.len();
        para(ui, &if more == 1 { "1 more question is affected but not listed.".to_string() } else { format!("{more} more questions are affected but not listed.") });
    }
    heading(ui, "What you can do");
    para(ui, "Take those questions out of what you share, or share the bank to the circle instead of the public pool.");
    let may = c.hits.iter().any(|h| h.may_claim_independence);
    heading(ui, "Or: I wrote these myself");
    if may {
        para(ui, "Only if the questions are really your own work, written without copying the circle's bank. Your words are kept and an administrator may read them. If the circle's bank is private to the circle and you can still read it, this will not be accepted for an exact copy of it.");
        field(ui, "In a sentence: why these are your own work", &mut m.form.circle_independent_statement, "pub_circle_ind_stmt", "At least 10 characters");
    } else {
        para(ui, "A written statement can not lift this for these questions: they match a circle bank you can still read. Remove them, or ask the circle's owner to share the bank publicly under a Creative Commons licence.");
    }
    let (mut claim, mut back) = (false, false);
    ui.horizontal_wrapped(|ui| {
        ui.add_enabled_ui(may, |ui| claim = kit::ghost_button(ui, "Send this statement and check again").clicked());
        back = kit::ghost_button(ui, "Back to options").clicked();
    });
    if claim {
        m.claim_circle_independence();
    }
    if back {
        m.back_to_options();
    }
}
