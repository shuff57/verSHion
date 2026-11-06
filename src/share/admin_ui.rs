//! The approval queue (egui). All logic is in [`super::admin`]; this file only draws it.
//!
//! The screen is the admin's, so it says what an approval means and asks for the password before any
//! change goes through. Everything a person typed is drawn as plain text (the model already made it
//! plain), never as markup.

use egui::{RichText, Ui};

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
    // An open decision owns the window: the tab's heading and blurb yield (the chosen tab already
    // names the page), so the applicant's card and its buttons fit together under the count line.
    if m.decision.is_none() && !m.confirming {
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
    }
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
    let deciding = m.decision.as_ref().map(|d| d.request_id.clone());
    // The open decision is brought into view once (and again when the password box opens). Its own
    // top is what is brought: a page scrolled to the bottom of a tall form leaves the buttons under
    // the pointer with no applicant above them.
    let view_key = deciding.as_ref().map(|r| format!("{r}:{}", m.confirming));
    let seen_id = egui::Id::new("admin_brought");
    let bring = view_key.is_some() && ui.ctx().data(|d| d.get_temp::<String>(seen_id)) != view_key;

    // The queue is a viewport of its own at every width: a visible track, a bottom edge above the
    // window rim rather than through the third card, and a cue when applications continue below.
    let room = (ui.clip_rect().bottom() - ui.cursor().top() - 34.0).max(200.0);
    // The viewport ends between two cards or under a card's name row, never through a name, a fact
    // or a button: last frame's card geometry says where those are, and the window keeps the most
    // that fits. A card taller than half
    // the room (an open decision on a small window) takes the room as it is.
    let ends_id = egui::Id::new("admin_card_ends");
    let ends: Vec<f32> = ui.ctx().data(|d| d.get_temp(ends_id)).unwrap_or_default();
    let vp_h = ends.iter().copied().filter(|e| *e <= room && *e >= room * 0.5).fold(None, |a: Option<f32>, e| Some(a.map_or(e, |a| a.max(e)))).map_or(room, |e| e + 2.0);
    let mut card_ends: Vec<f32> = Vec::new();
    let track = kit::visible_scroll(ui);
    let out = egui::ScrollArea::vertical()
        .id_salt("admin_queue")
        .max_height(vp_h)
        .auto_shrink([false, false])
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let content_top = ui.min_rect().top();
            // A password asked for with no decision open (not reached by the model today, kept safe).
            if m.confirming && m.decision.is_none() {
                kit::card().inner_margin(egui::Margin::same(14)).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    password_block(ui, m, &mut ask);
                });
                ui.add_space(8.0);
            }
            for app in &items {
                let open = deciding.as_deref() == Some(app.request_id.as_str());
                let busy = deciding.is_some() && !open;
                let stroke = if open { egui::Stroke::new(1.5, theme::accent_col()) } else { egui::Stroke::new(1.0, theme::border()) };
                let card = kit::card().stroke(stroke).inner_margin(egui::Margin::same(14)).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    // while one decision is open the other applications wait. Their facts stay fully
                    // legible; only their buttons are shown as not-interactive, so no one reads an open
                    // queue as a set of live controls.
                    application(ui, app, open, busy, &mut ask);
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
                // two clean places to stop: under the card, or under its name row (the next
                // applicant's name and buttons show, cut between rows, never through them)
                card_ends.push(card.response.rect.top() - content_top + 64.0);
                card_ends.push(card.response.rect.bottom() - content_top);
                if open && bring {
                    // The first passes lay out with the fallback font metrics and settle over two or
                    // three frames; scrolling to a rect from one of those would fling the queue past
                    // the applicant. Only scroll once the same target has held still for a frame.
                    let r = card.response.rect;
                    let last_id = egui::Id::new("admin_bring_rect");
                    let last: Option<(String, egui::Rect)> = ui.ctx().data(|d| d.get_temp(last_id));
                    let stable = last.as_ref().is_some_and(|(k, p)| Some(k) == view_key.as_ref() && (*p).min.distance(r.min) < 1.5);
                    ui.ctx().data_mut(|d| d.insert_temp(last_id, (view_key.clone().unwrap_or_default(), r)));
                    if stable {
                        let vp = ui.clip_rect();
                        // Bring the card's top in (never its bottom): the name, school and waiting
                        // time stay above the form in every state, password or not.
                        if r.top() > vp.top() + 6.0 || r.top() < vp.top() {
                            let target = egui::Rect::from_min_size(egui::pos2(r.left(), r.top()), egui::vec2(r.width(), 1.0));
                            ui.scroll_to_rect(target, Some(egui::Align::Min));
                        }
                        ui.ctx().data_mut(|d| d.insert_temp(seen_id, view_key.clone()));
                    }
                }
                ui.add_space(8.0);
            }
            // The audit note closes the queue, so it travels with the list rather than eating the
            // height the waiting cards need.
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
        });
    kit::restore_scroll(ui, track);
    ui.ctx().data_mut(|d| d.insert_temp(ends_id, card_ends));
    let (bar, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 24.0), egui::Sense::hover());
    if out.content_size.y > out.inner_rect.height() + 2.0 {
        kit::more_below(ui, bar, "More below");
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
}

/// One application: who, from where, how long it has waited, what is known about the domain.
fn application(ui: &mut Ui, app: &Application, open: bool, busy: bool, ask: &mut Option<Ask>) {
    let wide = ui.available_width() >= 620.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        ui.label(theme::strong(app.who()).size(15.0));
        if app.suspended {
            kit::pill_no_wrap(ui, "suspended", theme::error_wash(), theme::bad());
        }
        if app.profile_changed {
            // a trust warning, not a success tag: the accent wash read as "fine" on a card whose
            // whole job is to make the reviewer stop and look
            kit::pill_no_wrap(ui, "profile changed since applying", theme::highlighter(), theme::on_highlighter());
        }
        if busy {
            // why this card's controls are inert: one decision is open above
            kit::pill_no_wrap(ui, "waiting", theme::field(), theme::muted_col()).on_hover_text("Finish the open decision to act on this one");
        }
        if !open && wide {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let live = |ui: &mut egui::Ui, ask: &mut Option<Ask>| {
                    if kit::ghost_button(ui, "Refuse").clicked() {
                        *ask = Some(Ask::Open(app.clone(), false));
                    }
                    if kit::primary_button(ui, "Approve").clicked() {
                        *ask = Some(Ask::Open(app.clone(), true));
                    }
                };
                if busy {
                    ui.add_enabled_ui(false, |ui| live(ui, ask));
                } else {
                    live(ui, ask);
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
    // The labels are read before the values, so they cannot be the faintest type on the card: no
    // thin-space letterspacing at 9 px, just a tight bold caps at a size that holds up.
    let fact_label = |k: &str| RichText::new(k.to_uppercase()).family(theme::bold_family()).size(11.5).color(theme::text_col());
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = 18.0;
        let grid = egui::Grid::new(("admin_facts", &app.request_id)).spacing([14.0, 6.0]).min_col_width(0.0);
        if wide {
            // One table, two pairs a row, both pairs ending on the same right edge as the form
            // fields below (they used to stop ~50 px short of them).
            let lw1 = 96.0;
            let lw2 = 112.0;
            let gap = 14.0;
            let vw = ((ui.available_width() - lw1 - lw2 - 3.0 * gap) / 2.0).max(120.0);
            grid.num_columns(4).show(ui, |ui| {
                for pair in rows.chunks(2) {
                    for (i, (k, v, st)) in pair.iter().enumerate() {
                        let lw = if i == 0 { lw1 } else { lw2 };
                        ui.allocate_ui_with_layout(egui::vec2(lw, 18.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_min_width(lw);
                            ui.label(fact_label(k));
                        });
                        ui.allocate_ui_with_layout(egui::vec2(vw, 18.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
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
                    ui.allocate_ui_with_layout(egui::vec2(96.0, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_min_width(96.0);
                        ui.label(fact_label(k));
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
            let live = |ui: &mut egui::Ui, ask: &mut Option<Ask>| {
                if kit::primary_button(ui, "Approve").clicked() {
                    *ask = Some(Ask::Open(app.clone(), true));
                }
                if kit::ghost_button(ui, "Refuse").clicked() {
                    *ask = Some(Ask::Open(app.clone(), false));
                }
            };
            if busy {
                ui.add_enabled_ui(false, |ui| live(ui, ask));
            } else {
                live(ui, ask);
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

/// A fact the approval rests on: a label set at reading size in ink, the value beside it. These
/// used to be 9 px muted caps, the least legible text on the screen.
fn fact_row(ui: &mut Ui, label: &str, value: &str) {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        let w = 132.0_f32.min(ui.available_width() * 0.4);
        ui.allocate_ui_with_layout(egui::vec2(w, 20.0), egui::Layout::left_to_right(egui::Align::Min), |ui| {
            ui.set_min_width(w);
            ui.label(RichText::new(label).family(theme::bold_family()).size(12.5).color(theme::text_col()));
        });
        ui.add(egui::Label::new(theme::text(value).size(13.5)).wrap());
    });
}

/// The decision being filled in, inside the application's own card.
fn decision_form(ui: &mut Ui, d: &mut Decision, confirming: bool, ask: &mut Option<Ask>) {
    let title = if d.approve { format!("Approve {}", d.who) } else { format!("Refuse {}", d.who) };
    ui.label(theme::strong(title.as_str()).size(14.0));
    ui.add_space(8.0);
    // Waiting for the password: the form stays a form. The evidence is a choice, so it keeps its
    // radios; only the send buttons give way to the password strip below.
    if confirming {
        if d.approve {
            ui.label(theme::muted("What did you check?").size(12.5));
            ui.add_space(4.0);
            ui.scope(|ui| {
                ui.spacing_mut().interact_size.y = 22.0;
                ui.spacing_mut().item_spacing.y = 6.0;
                for (i, (_, label)) in EVIDENCE.iter().enumerate() {
                    if choice(ui, d.evidence == i, label) {
                        d.evidence = i;
                    }
                }
            });
            ui.add_space(8.0);
            let where_ = if d.evidence_note.trim().is_empty() { "Not given".to_string() } else { format!("“{}”", d.evidence_note.trim()) };
            fact_row(ui, "Where you checked it", &where_);
        } else {
            fact_row(ui, "Reason (the applicant reads this)", &format!("“{}”", d.reason.trim()));
        }
        if !d.note.trim().is_empty() {
            fact_row(ui, "Your note", d.note.trim());
        }
        return;
    }
    let left = |ui: &mut Ui, d: &mut Decision| {
        if d.approve {
            ui.label(theme::muted("What did you check?").size(12.5));
            ui.add_space(4.0);
            ui.scope(|ui| {
                ui.spacing_mut().interact_size.y = 22.0;
                ui.spacing_mut().item_spacing.y = 6.0;
                for (i, (_, label)) in EVIDENCE.iter().enumerate() {
                    if choice(ui, d.evidence == i, label) {
                        d.evidence = i;
                    }
                }
            });
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
        // A gated action must look gated: with the evidence note (or the refusal's reason) empty
        // the button stays off, so the step that is still missing is visible before it is pressed.
        if kit::primary_button_on(ui, label, d.problem().is_none()).clicked() {
            *ask = Some(Ask::Send);
        }
        if kit::ghost_button(ui, "Cancel").clicked() {
            *ask = Some(Ask::Cancel);
        }
    });
}

/// The password a change needs, asked once for five minutes of changes. Returns the block's rect so
/// the queue can bring it into view without scrolling the page's title away.
fn password_block(ui: &mut Ui, m: &mut AdminModel, ask: &mut Option<Ask>) -> egui::Rect {
    egui::Frame::new()
        .fill(theme::field())
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(12, 9))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let mut enter = false;
            let confirm_on = !m.password.is_empty();
            let button = |ui: &mut Ui, ask: &mut Option<Ask>, enter: bool| {
                if kit::primary_button_on(ui, "Confirm", confirm_on).clicked() || (enter && confirm_on) {
                    *ask = Some(Ask::Confirm);
                }
                if kit::ghost_button(ui, "Close")
                    .on_hover_text("Close this password box and keep the approval open")
                    .clicked()
                {
                    *ask = Some(Ask::CancelConfirm);
                }
            };
            let mut field = |ui: &mut Ui, width: f32| {
                let r = ui.add(
                    egui::TextEdit::singleline(&mut m.password)
                        .id(egui::Id::new("admin_password"))
                        .password(true)
                        .hint_text("Your password")
                        .desired_width(width),
                );
                r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
            };
            // The strip is one compact row: a short secret gets a short field and the buttons sit
            // beside it, so the waiting applications below stay on screen. On a phone the label
            // takes its own line and the field keeps the width the buttons leave.
            if ui.available_width() >= 760.0 {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    ui.label(theme::strong("Confirm your password to send").size(13.5));
                    let w = (ui.available_width() - 240.0).clamp(360.0, 420.0);
                    enter = field(ui, w);
                    button(ui, ask, enter);
                });
            } else {
                ui.label(theme::strong("Confirm your password to send").size(13.5));
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let w = (ui.available_width() - 236.0).clamp(200.0, 400.0);
                    enter = field(ui, w);
                    button(ui, ask, enter);
                });
            }
            ui.add_space(3.0);
            ui.add(
                egui::Label::new(
                    theme::muted("Used once over the secure connection and never stored; good for five minutes of changes.")
                        .size(12.0),
                )
                .wrap(),
            );
        })
        .response
        .rect
}
