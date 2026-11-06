//! The approval queue (egui). All logic is in [`super::admin`]; this file only draws it.
//!
//! The screen is the admin's, so it says what an approval means and asks for the password before any
//! change goes through. Everything a person typed is drawn as plain text (the model already made it
//! plain), never as markup.

use egui::Ui;

use super::admin::{age_text, summary, AdminModel, Application, Decision, State, CONFIRM_PROMPT, EVIDENCE};
use super::publish::SharedSession;
use crate::{kit, pageshell, theme};

/// The widest a typed field gets: a one-line note does not need the card's full width.
const FIELD_W: f32 = 520.0;

fn field(ui: &mut Ui, id: &str, value: &mut String, hint: &str, one_line: bool) {
    let w = ui.available_width().min(FIELD_W);
    if one_line {
        ui.add(egui::TextEdit::singleline(value).id(egui::Id::new(id)).hint_text(hint).desired_width(w));
    } else {
        ui.add(egui::TextEdit::multiline(value).id(egui::Id::new(id)).hint_text(hint).desired_rows(2).desired_width(w));
    }
}

/// One labelled fact of an application: a small label beside its value, in a facts grid.
fn fact(ui: &mut Ui, label: &str, value: &str, strong: bool) {
    ui.label(kit::small_caps_text(label));
    let t = if strong { theme::strong(value).size(13.0) } else { theme::text(value).size(13.0) };
    ui.add(egui::Label::new(t).wrap());
    ui.end_row();
}

/// A facts grid: labels in a narrow column, values beside them.
fn facts(ui: &mut Ui, salt: &str, f: impl FnOnce(&mut Ui)) {
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = 18.0;
        egui::Grid::new(salt).num_columns(2).spacing([14.0, 6.0]).min_col_width(0.0).show(ui, f);
    });
}

/// What a press in the list asks for, applied after the list is drawn.
enum Ask {
    Open(Application, bool),
    Send,
    Cancel,
    Confirm,
    CancelConfirm,
}

/// The tab's body.
pub fn body(ui: &mut Ui, m: &mut AdminModel, session: Option<SharedSession>) {
    m.frame(web_time::Instant::now());
    pageshell::heading(ui, "Approval queue");
    ui.add_space(4.0);
    ui.scope(|ui| {
        ui.set_max_width(ui.available_width().min(pageshell::MEASURE));
        ui.add(
            egui::Label::new(
                theme::muted(
                    "Every teacher is checked by hand before the pool opens to them. Approve only someone you can vouch for: \
                     an entry in the school's staff directory, or a person at the school who confirmed it.",
                )
                .size(13.0),
            )
            .wrap(),
        );
    });
    ui.add_space(12.0);
    // The first frame the tab shows: read the queue (once).
    m.open_queue(session.clone());
    if m.busy() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(300));
    }

    // ---- where the queue stands, and reading it again ------------------------------------------
    let line = match &m.state {
        State::Loaded(q) => summary(q),
        State::Reading => "Reading the queue...".to_string(),
        State::Failed(p) => p.message.clone(),
        State::Closed => "The queue has not been read yet.".to_string(),
    };
    let loaded = matches!(m.state, State::Loaded(_));
    let failed = matches!(m.state, State::Failed(_));
    let mut reload = false;
    const AGAIN: &str = "Read the queue again";
    // The summary and the refresh share a row when both fit; otherwise the button goes under the sentence.
    let text_w = ui.painter().layout_no_wrap(line.clone(), egui::FontId::new(13.5, theme::bold_family()), theme::ink()).size().x;
    let one_row = ui.available_width() >= text_w + pageshell::button_width(ui, AGAIN) + 40.0;
    ui.horizontal(|ui| {
        ui.add(egui::Label::new(theme::strong(line).size(13.5)).wrap());
        if matches!(m.state, State::Reading) {
            ui.spinner();
        }
        if one_row {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if loaded && kit::ghost_button(ui, AGAIN).on_hover_text("Fetch the waiting applications again").clicked() {
                    reload = true;
                }
            });
        }
    });
    if !one_row && loaded {
        ui.add_space(6.0);
        if kit::ghost_button(ui, AGAIN).on_hover_text("Fetch the waiting applications again").clicked() {
            reload = true;
        }
    }
    if failed && kit::primary_button(ui, "Try again").clicked() {
        reload = true;
    }
    if reload {
        m.load(session.clone());
    }
    // A note from the last action (the password prompt is said by the password box itself).
    if let Some(n) = m.notice.clone().filter(|n| !(m.confirming && n == CONFIRM_PROMPT)) {
        ui.add_space(4.0);
        ui.add(egui::Label::new(theme::text(n).size(13.0)).wrap());
    }
    ui.add_space(10.0);

    let items: Vec<Application> = match &m.state {
        State::Loaded(q) => q.items.clone(),
        _ => Vec::new(),
    };
    if loaded && items.is_empty() {
        ui.label(theme::text("Nothing is waiting. Every application has been decided.").size(13.0));
    }
    let mut ask: Option<Ask> = None;
    // A password asked for with no decision open (not reached by the model today, kept safe).
    if m.confirming && m.decision.is_none() {
        kit::card().inner_margin(egui::Margin::same(14)).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            password_block(ui, m, &mut ask);
        });
        ui.add_space(8.0);
    }
    let deciding = m.decision.as_ref().map(|d| d.request_id.clone());
    for app in &items {
        let open = deciding.as_deref() == Some(app.request_id.as_str());
        let stroke = if open { egui::Stroke::new(1.5, theme::accent_col()) } else { egui::Stroke::new(1.0, theme::border()) };
        kit::card().stroke(stroke).inner_margin(egui::Margin::same(14)).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            application(ui, app, open, &mut ask);
            if open {
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(8.0);
                let confirming = m.confirming;
                if let Some(d) = m.decision.as_mut() {
                    decision_form(ui, d, confirming, &mut ask);
                }
                if confirming {
                    ui.add_space(10.0);
                    password_block(ui, m, &mut ask);
                }
            }
        });
        ui.add_space(8.0);
    }
    match ask {
        Some(Ask::Open(app, approve)) => m.open_decision(&app, approve),
        Some(Ask::Send) => m.decide(session.clone()),
        Some(Ask::Cancel) => m.cancel_decision(),
        Some(Ask::Confirm) => m.confirm_password(session.clone()),
        Some(Ask::CancelConfirm) => {
            m.password.clear();
            m.confirming = false;
        }
        None => {}
    }
    ui.add_space(4.0);
    ui.scope(|ui| {
        ui.set_max_width(ui.available_width().min(pageshell::MEASURE));
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
    });
}

/// One application: who, from where, how long it has waited, what is known about the domain.
fn application(ui: &mut Ui, app: &Application, open: bool, ask: &mut Option<Ask>) {
    let wide = ui.available_width() >= 620.0;
    let buttons = |ui: &mut Ui, ask: &mut Option<Ask>| {
        if kit::ghost_button(ui, "Refuse...").clicked() {
            *ask = Some(Ask::Open(app.clone(), false));
        }
        if kit::primary_button(ui, "Approve...").clicked() {
            *ask = Some(Ask::Open(app.clone(), true));
        }
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        ui.label(theme::strong(app.who()).size(15.0));
        if app.suspended {
            kit::pill_no_wrap(ui, "suspended", theme::error_wash(), theme::bad());
        }
        if app.profile_changed {
            kit::pill_no_wrap(ui, "profile changed since applying", theme::accent_faded(), theme::text_col());
        }
        if !open && wide {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| buttons(ui, ask));
        }
    });
    ui.add_space(6.0);
    let who = |ui: &mut Ui| {
        facts(ui, &format!("admin_who_{}", app.request_id), |ui| {
            fact(ui, "Email", &app.email, false);
            fact(ui, "School", if app.school.is_empty() { "Not given" } else { &app.school }, false);
        });
    };
    let known = |ui: &mut Ui| facts(ui, &format!("admin_known_{}", app.request_id), |ui| {
        fact(ui, "Waiting", &age_text(app.age_seconds), app.age_seconds >= 2 * 86_400);
        let mut domain = app.email_domain.clone();
        if let Some((approved, rejected)) = app.domain_history
            && (approved > 0 || rejected > 0)
        {
            domain.push_str(&format!("  ·  {approved} approved, {rejected} refused before"));
        }
        if let Some(h) = &app.domain_hint {
            domain.push_str(&format!("  ·  your label: {h}"));
        }
        fact(ui, "Domain", &domain, false);
        if app.prior_rejections > 0 {
            let n = match app.prior_rejections {
                1 => "1 earlier application was refused".to_string(),
                n => format!("{n} earlier applications were refused"),
            };
            fact(ui, "History", &n, true);
        }
    });
    if wide {
        ui.columns(2, |c| {
            who(&mut c[0]);
            known(&mut c[1]);
        });
    } else {
        who(ui);
        ui.add_space(6.0);
        known(ui);
        if !open {
            ui.add_space(8.0);
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                // Left to right here, so Approve comes first.
                if kit::primary_button(ui, "Approve...").clicked() {
                    *ask = Some(Ask::Open(app.clone(), true));
                }
                if kit::ghost_button(ui, "Refuse...").clicked() {
                    *ask = Some(Ask::Open(app.clone(), false));
                }
            });
        }
    }
}

/// The decision being filled in, inside the application's own card.
fn decision_form(ui: &mut Ui, d: &mut Decision, confirming: bool, ask: &mut Option<Ask>) {
    let title = if d.approve { format!("Approve {}", d.who) } else { format!("Refuse {}", d.who) };
    ui.label(theme::strong(title.as_str()).size(14.0));
    ui.add_space(6.0);
    // Waiting for the password: what is about to be sent, in a line or two, so the password box stays in view.
    if confirming {
        let what = if d.approve {
            let mut w = format!("Checked: {}", EVIDENCE.get(d.evidence).map(|e| e.1).unwrap_or("nothing chosen"));
            if !d.evidence_note.trim().is_empty() {
                w.push_str(&format!("  ·  “{}”", d.evidence_note.trim()));
            }
            w
        } else {
            format!("Reason the applicant reads: “{}”", d.reason.trim())
        };
        ui.add(egui::Label::new(theme::text(what).size(13.0)).wrap());
        if !d.note.trim().is_empty() {
            ui.add(egui::Label::new(theme::muted(format!("Your note: {}", d.note.trim())).size(12.5)).wrap());
        }
        return;
    }
    ui.scope(|ui| {
        if d.approve {
            ui.label(theme::muted("What did you check?").size(12.5));
            ui.add_space(2.0);
            ui.scope(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.spacing_mut().interact_size.y = 24.0;
                for (i, (_, label)) in EVIDENCE.iter().enumerate() {
                    ui.radio_value(&mut d.evidence, i, *label);
                }
            });
            ui.add_space(6.0);
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
    });
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        let label = if d.approve { "Approve" } else { "Refuse" };
        if kit::primary_button(ui, label).clicked() {
            *ask = Some(Ask::Send);
        }
        if kit::ghost_button(ui, "Cancel").clicked() {
            *ask = Some(Ask::Cancel);
        }
    });
}

/// The password a change needs, asked once for five minutes of changes.
fn password_block(ui: &mut Ui, m: &mut AdminModel, ask: &mut Option<Ask>) {
    egui::Frame::new()
        .fill(theme::field())
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(theme::strong("Confirm your password to send this").size(13.5));
            ui.add_space(2.0);
            ui.add(
                egui::Label::new(
                    theme::muted("Sent once over the secure connection and never stored. It is good for five minutes of changes.")
                        .size(12.5),
                )
                .wrap(),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let r = ui.add(
                    egui::TextEdit::singleline(&mut m.password)
                        .id(egui::Id::new("admin_password"))
                        .password(true)
                        .hint_text("Your password")
                        .desired_width(260.0),
                );
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if kit::primary_button(ui, "Confirm").clicked() || enter {
                    *ask = Some(Ask::Confirm);
                }
                if kit::ghost_button(ui, "Cancel").clicked() {
                    *ask = Some(Ask::CancelConfirm);
                }
            });
        });
}
