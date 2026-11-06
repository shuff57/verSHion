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
    // The refresh sits with the sentence it refreshes.
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        ui.label(theme::strong(line).size(13.5));
        if matches!(m.state, State::Reading) {
            ui.spinner();
        }
        if loaded && kit::chip_toggle(ui, "Read again", false).on_hover_text("Fetch the waiting applications again").clicked() {
            reload = true;
        }
    });
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
    // The open decision is brought into view once (and again when the password box opens), so its
    // buttons are not left below the window.
    let view_key = deciding.as_ref().map(|r| format!("{r}:{}", m.confirming));
    let seen_id = egui::Id::new("admin_decision_in_view");
    let bring = view_key.is_some() && ui.ctx().data(|d| d.get_temp::<Option<String>>(seen_id)) != Some(view_key.clone());
    ui.ctx().data_mut(|d| d.insert_temp(seen_id, view_key));
    for app in &items {
        let open = deciding.as_deref() == Some(app.request_id.as_str());
        let stroke = if open { egui::Stroke::new(1.5, theme::accent_col()) } else { egui::Stroke::new(1.0, theme::border()) };
        let card = kit::card().stroke(stroke).inner_margin(egui::Margin::same(14)).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            // while one decision is open the other applications wait: no stray Approve on the wrong person
            ui.add_enabled_ui(open || deciding.is_none(), |ui| application(ui, app, open, &mut ask));
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
        if open && bring {
            ui.scroll_to_rect(card.response.rect.expand(8.0), None);
        }
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
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if kit::ghost_button(ui, "Refuse...").clicked() {
                    *ask = Some(Ask::Open(app.clone(), false));
                }
                if kit::primary_button(ui, "Approve...").clicked() {
                    *ask = Some(Ask::Open(app.clone(), true));
                }
            });
        }
    });
    ui.add_space(6.0);
    // The same six facts on every card, in the same places, so cards compare at a glance.
    let school = if app.school.is_empty() { "Not given".to_string() } else { app.school.clone() };
    let label = app.domain_hint.clone().unwrap_or_else(|| "None".into());
    let domain_hist = match app.domain_history {
        Some((a, r)) if a > 0 || r > 0 => format!("{a} approved, {r} refused"),
        _ => "No decisions yet".into(),
    };
    let before = match app.prior_rejections {
        0 => "No".to_string(),
        1 => "Once, refused".to_string(),
        n => format!("{n} times, refused"),
    };
    let rows: [(&str, String, bool); 6] = [
        ("Email", app.email.clone(), false),
        ("Waiting", age_text(app.age_seconds), app.age_seconds >= 2 * 86_400),
        ("School", school, false),
        ("Domain", format!("{}  ·  {domain_hist}", app.email_domain), false),
        ("Your label", label, false),
        ("Applied before", before, app.prior_rejections > 0),
    ];
    let value = |ui: &mut Ui, v: &str, strong: bool| {
        let t = if strong { theme::strong(v).size(13.0) } else { theme::text(v).size(13.0) };
        ui.add(egui::Label::new(t).wrap_mode(egui::TextWrapMode::Extend));
    };
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = 18.0;
        let grid = egui::Grid::new(("admin_facts", &app.request_id)).spacing([14.0, 6.0]).min_col_width(0.0);
        if wide {
            grid.num_columns(4).show(ui, |ui| {
                for pair in rows.chunks(2) {
                    for (i, (k, v, st)) in pair.iter().enumerate() {
                        // fixed column widths: every card's facts line up with the card above it
                        let lw = if i == 0 { 96.0 } else { 128.0 };
                        ui.allocate_ui_with_layout(egui::vec2(lw, 18.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_min_width(lw);
                            ui.add_space(if i == 1 { 24.0 } else { 0.0 });
                            ui.label(kit::small_caps_text(k));
                        });
                        let vw: f32 = if i == 0 { 300.0 } else { 0.0 };
                        ui.allocate_ui_with_layout(egui::vec2(vw.max(1.0), 18.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_min_width(vw);
                            value(ui, v, *st);
                        });
                    }
                    ui.end_row();
                }
            });
        } else {
            grid.num_columns(2).show(ui, |ui| {
                for (k, v, st) in &rows {
                    ui.allocate_ui_with_layout(egui::vec2(104.0, 18.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_min_width(104.0);
                        ui.label(kit::small_caps_text(k));
                    });
                    ui.add(egui::Label::new(if *st { theme::strong(v.as_str()).size(13.0) } else { theme::text(v.as_str()).size(13.0) }).wrap());
                    ui.end_row();
                }
            });
        }
    });
    if !open && !wide {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if kit::primary_button(ui, "Approve...").clicked() {
                *ask = Some(Ask::Open(app.clone(), true));
            }
            if kit::ghost_button(ui, "Refuse...").clicked() {
                *ask = Some(Ask::Open(app.clone(), false));
            }
        });
    }
}

/// A choice in the decision form: a filled accent dot when chosen, a ring when not (the stock radio's
/// dark-on-dark dot was hard to tell from the empty one in the dark theme).
fn choice(ui: &mut Ui, on: bool, label: &str) -> bool {
    let r = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let (c, resp) = ui.allocate_exact_size(egui::Vec2::splat(18.0), egui::Sense::click());
            let p = ui.painter();
            if on {
                p.circle_filled(c.center(), 8.0, theme::accent_col());
                p.circle_filled(c.center(), 3.2, theme::on_accent_col());
            } else {
                p.circle_stroke(c.center(), 7.5, egui::Stroke::new(1.5, if resp.hovered() { theme::accent_col() } else { theme::muted_col() }));
            }
            let t = ui.add(egui::Label::new(theme::text(label).size(13.5)).sense(egui::Sense::click()));
            resp.clicked() || t.clicked()
        })
        .inner;
    r
}

/// The decision being filled in, inside the application's own card.
fn decision_form(ui: &mut Ui, d: &mut Decision, confirming: bool, ask: &mut Option<Ask>) {
    let title = if d.approve { format!("Approve {}", d.who) } else { format!("Refuse {}", d.who) };
    ui.label(theme::strong(title.as_str()).size(14.0));
    ui.add_space(8.0);
    // Waiting for the password: what is about to be sent, as labelled rows, so the password box stays in view.
    if confirming {
        ui.scope(|ui| {
            ui.spacing_mut().interact_size.y = 18.0;
            egui::Grid::new("admin_sending").num_columns(2).spacing([14.0, 6.0]).show(ui, |ui| {
                let row = |ui: &mut Ui, k: &str, v: String| {
                    ui.label(kit::small_caps_text(k));
                    ui.add(egui::Label::new(theme::text(v).size(13.0)).wrap());
                    ui.end_row();
                };
                if d.approve {
                    row(ui, "Checked", EVIDENCE.get(d.evidence).map(|e| e.1).unwrap_or("Nothing chosen").to_string());
                    if !d.evidence_note.trim().is_empty() {
                        row(ui, "Where", format!("“{}”", d.evidence_note.trim()));
                    }
                } else {
                    row(ui, "Reason", format!("“{}”", d.reason.trim()));
                }
                if !d.note.trim().is_empty() {
                    row(ui, "Your note", d.note.trim().to_string());
                }
            });
        });
        return;
    }
    let left = |ui: &mut Ui, d: &mut Decision| {
        if d.approve {
            ui.label(theme::muted("What did you check?").size(12.5));
            ui.add_space(4.0);
            for (i, (_, label)) in EVIDENCE.iter().enumerate() {
                if choice(ui, d.evidence == i, label) {
                    d.evidence = i;
                }
                ui.add_space(2.0);
            }
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
    };
    let right = |ui: &mut Ui, d: &mut Decision| {
        if d.approve {
            ui.label(theme::muted("Where you checked it (kept in the audit trail)").size(12.5));
            field(ui, "admin_evidence_note", &mut d.evidence_note, "e.g. the staff directory page, or who confirmed it", true);
            ui.add_space(6.0);
        }
        ui.label(theme::muted("Your note (never shown to the applicant)").size(12.5));
        field(ui, "admin_note", &mut d.note, "optional", true);
    };
    if ui.available_width() >= 620.0 {
        ui.columns(2, |c| {
            left(&mut c[0], d);
            right(&mut c[1], d);
        });
    } else {
        left(ui, d);
        ui.add_space(6.0);
        right(ui, d);
    }
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
            ui.horizontal_wrapped(|ui| {
                let r = ui.add(
                    egui::TextEdit::singleline(&mut m.password)
                        .id(egui::Id::new("admin_password"))
                        .password(true)
                        .hint_text("Your password")
                        .desired_width(ui.available_width().min(FIELD_W) - 200.0),
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
