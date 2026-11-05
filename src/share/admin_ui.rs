//! The approval queue (egui). All logic is in [`super::admin`]; this file only draws it.
//!
//! The screen is the admin's, so it says what an approval means and asks for the password before any
//! change goes through. Everything a person typed is drawn as plain text (the model already made it
//! plain), never as markup.

use egui::Ui;

use super::admin::{age_text, summary, AdminModel, Application, State, EVIDENCE};
use super::publish::SharedSession;
use crate::{kit, pageshell, theme};

fn field(ui: &mut Ui, id: &str, value: &mut String, hint: &str, one_line: bool) {
    if one_line {
        ui.add(egui::TextEdit::singleline(value).id(egui::Id::new(id)).hint_text(hint).desired_width(f32::INFINITY));
    } else {
        ui.add(
            egui::TextEdit::multiline(value)
                .id(egui::Id::new(id))
                .hint_text(hint)
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        );
    }
}

/// The tab's body.
pub fn body(ui: &mut Ui, m: &mut AdminModel, session: Option<SharedSession>) {
    m.frame(web_time::Instant::now());
    pageshell::heading(ui, "Approval queue");
    ui.add_space(4.0);
    ui.add(
        egui::Label::new(
            theme::muted(
                "Every teacher is checked by hand before the pool opens to them. Approve only someone you can vouch for: \
                 the pool's rule is an entry in the school's staff directory, or a person at the school who confirmed it. \
                 Your password is asked once and is good for five minutes of changes.",
            )
            .size(13.0),
        )
        .wrap(),
    );
    ui.add_space(10.0);
    // The first frame the tab shows: read the queue (once).
    m.open_queue(session.clone());
    if m.busy() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(300));
    }

    if let Some(n) = m.notice.clone() {
        ui.add_space(2.0);
        ui.add(egui::Label::new(theme::text(n).size(13.0)).wrap());
        ui.add_space(6.0);
    }

    // ---- the password, when a change needs one ------------------------------------------------
    if m.confirming {
        let mut confirm = false;
        let mut cancel = false;
        kit::card().inner_margin(egui::Margin::same(12)).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(theme::strong("Confirm your password").size(13.5));
            ui.add_space(2.0);
            ui.add(
                egui::Label::new(
                    theme::muted(
                        "Used once, sent over the secure connection, and never stored. It makes a five-minute token for changes.",
                    )
                    .size(12.5),
                )
                .wrap(),
            );
            ui.add_space(6.0);
            let r = ui.add(
                egui::TextEdit::singleline(&mut m.password)
                    .id(egui::Id::new("admin_password"))
                    .password(true)
                    .hint_text("Your password")
                    .desired_width(280.0),
            );
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if kit::primary_button(ui, "Confirm").clicked() || enter {
                    confirm = true;
                }
                if kit::ghost_button(ui, "Cancel").clicked() {
                    cancel = true;
                }
            });
        });
        if confirm {
            m.confirm_password(session.clone());
        }
        if cancel {
            m.password.clear();
            m.confirming = false;
        }
        ui.add_space(8.0);
    }

    // ---- the decision being filled in ---------------------------------------------------------
    let mut send = false;
    let mut cancel = false;
    if let Some(d) = m.decision.as_mut() {
        let title = if d.approve { format!("Approve {}", d.who) } else { format!("Refuse {}", d.who) };
        kit::card().inner_margin(egui::Margin::same(12)).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(theme::strong(title.as_str()).size(14.0));
            ui.add_space(6.0);
            if d.approve {
                ui.label(theme::muted("What did you check?").size(12.5));
                for (i, (_, label)) in EVIDENCE.iter().enumerate() {
                    ui.radio_value(&mut d.evidence, i, *label);
                }
                ui.add_space(4.0);
                ui.label(theme::muted("Where you checked it (kept in the audit trail)").size(12.5));
                field(ui, "admin_evidence_note", &mut d.evidence_note, "e.g. the staff directory page, or who confirmed it", true);
            } else {
                ui.label(theme::muted("Why (the applicant reads this)").size(12.5));
                field(
                    ui,
                    "admin_reason",
                    &mut d.reason,
                    "e.g. We could not find you in the school's staff list. Write again from your school address.",
                    false,
                );
            }
            ui.add_space(6.0);
            ui.label(theme::muted("Your note (never shown to the applicant)").size(12.5));
            field(ui, "admin_note", &mut d.note, "optional", true);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let label = if d.approve { "Approve" } else { "Refuse" };
                if kit::primary_button(ui, label).clicked() {
                    send = true;
                }
                if kit::ghost_button(ui, "Cancel").clicked() {
                    cancel = true;
                }
            });
        });
        ui.add_space(8.0);
    }
    if send {
        m.decide(session.clone());
    }
    if cancel {
        m.cancel_decision();
    }

    // ---- the list -----------------------------------------------------------------------------
    let line = match &m.state {
        State::Loaded(q) => summary(q),
        State::Reading => "Reading the queue...".to_string(),
        State::Failed(p) => p.message.clone(),
        State::Closed => "The queue has not been read yet.".to_string(),
    };
    ui.add(egui::Label::new(theme::muted(line).size(13.0)).wrap());
    ui.add_space(6.0);
    if matches!(m.state, State::Reading) {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(theme::muted("One moment..."));
        });
    }
    let failed = matches!(m.state, State::Failed(_));
    if failed && kit::primary_button(ui, "Try again").clicked() {
        m.load(session.clone());
    }
    let loaded = matches!(m.state, State::Loaded(_));
    // A refresh keeps the list on screen while it runs; this always offers one.
    if loaded && kit::ghost_button(ui, "Read again").clicked() {
        m.load(session.clone());
    }
    ui.add_space(8.0);

    let items: Vec<Application> = match &m.state {
        State::Loaded(q) => q.items.clone(),
        _ => Vec::new(),
    };
    if loaded && items.is_empty() {
        ui.label(theme::text("Nothing is waiting. Every application has been decided.").size(13.0));
    }
    let mut pick: Option<(Application, bool)> = None;
    for app in &items {
        kit::card().inner_margin(egui::Margin::same(12)).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                ui.label(theme::strong(app.who()).size(14.0));
                if app.suspended {
                    kit::pill(ui, "suspended", theme::error_wash(), theme::bad());
                }
                if app.profile_changed {
                    kit::pill(ui, "profile changed since applying", theme::accent_faded(), theme::text_col());
                }
            });
            ui.add_space(2.0);
            ui.add(egui::Label::new(theme::muted(format!("{}  ·  {}", app.email, app.email_domain)).size(12.5)).wrap());
            if !app.school.is_empty() {
                ui.add(egui::Label::new(theme::text(format!("School: {}", app.school)).size(12.5)).wrap());
            }
            let mut facts = vec![format!("Waiting {}", age_text(app.age_seconds))];
            if app.prior_rejections > 0 {
                facts.push(match app.prior_rejections {
                    1 => "1 earlier application was refused".to_string(),
                    n => format!("{n} earlier applications were refused"),
                });
            }
            if let Some((approved, rejected)) = app.domain_history
                && (approved > 0 || rejected > 0)
            {
                facts.push(format!("This domain: {approved} approved, {rejected} refused"));
            }
            if let Some(h) = &app.domain_hint {
                facts.push(format!("Your label for it: {h}"));
            }
            ui.add(egui::Label::new(theme::muted(facts.join("  ·  ")).size(12.0)).wrap());
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if kit::primary_button(ui, "Approve...").clicked() {
                    pick = Some((app.clone(), true));
                }
                if kit::ghost_button(ui, "Refuse...").clicked() {
                    pick = Some((app.clone(), false));
                }
            });
        });
        ui.add_space(6.0);
    }
    if let Some((app, approve)) = pick {
        m.open_decision(&app, approve);
    }
    ui.add_space(4.0);
    ui.add(
        egui::Label::new(
            theme::muted(
                "A label on an email domain never grants trust by itself: students often hold addresses on the staff domain. \
                 Every approval and refusal is written to the audit log with your name.",
            )
            .size(12.0),
        )
        .wrap(),
    );
}
