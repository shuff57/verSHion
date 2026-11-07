//! The approval queue (egui). All logic is in [`super::admin`]; this file only draws it.
//!
//! The screen is the admin's, so it says what an approval means and asks for the password before any
//! change goes through. Everything a person typed is drawn as plain text (the model already made it
//! plain), never as markup.

use egui::{RichText, Ui};

use super::admin::{
    age_text, dollars, growth_summary, referrer_label, source_label, summary, visits_line, AdminModel, Application, Decision, GrowthState, SourceRow, State, View,
    CONFIRM_PROMPT, EVIDENCE, GROWTH_DAYS,
};
use super::publish::SharedSession;
use crate::{kit, pageshell, theme};

/// The widest a typed field gets: a one-line note does not need the card's full width.
const FIELD_W: f32 = 520.0;

fn field(ui: &mut Ui, id: &str, value: &mut String, hint: &str, one_line: bool) -> egui::Response {
    let w = ui.available_width().min(FIELD_W);
    if one_line {
        ui.add(egui::TextEdit::singleline(value).id(egui::Id::new(id)).hint_text(hint).desired_width(w))
    } else {
        ui.add(egui::TextEdit::multiline(value).id(egui::Id::new(id)).hint_text(hint).desired_rows(2).desired_width(w))
    }
}

/// The label over a field that gates the send button: while the field is empty it is set in ink
/// with "required", so the step still missing is the one the eye lands on.
fn gate_label(ui: &mut Ui, label: &str, missing: bool) {
    if missing {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.label(theme::strong(label).size(12.5));
            ui.label(theme::accent("required").size(11.5));
        });
    } else {
        ui.label(theme::muted(label).size(12.5));
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
    // The tab's two screens: the queue and the growth report. An open decision owns the window, so
    // the switch yields with the heading while one is being filled in.
    if m.decision.is_none() && !m.confirming {
        let mut pick = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            if kit::chip_toggle(ui, "Applications", m.view == View::Queue).clicked() {
                pick = Some(View::Queue);
            }
            if kit::chip_toggle(ui, "Growth", m.view == View::Growth).on_hover_text("Sign-ups and purchases by the link they came from").clicked() {
                pick = Some(View::Growth);
            }
        });
        ui.add_space(10.0);
        match pick {
            Some(View::Growth) => m.open_growth(session.clone()),
            Some(View::Queue) => m.view = View::Queue,
            None => {}
        }
    }
    if m.view == View::Growth && m.decision.is_none() && !m.confirming {
        growth_body(ui, m, session);
        return;
    }
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
                        "Every teacher is checked by hand before the pool opens to them. Each application carries a link to \
                         the applicant's staff page: open it, find them on it, and say where you checked. Approve only \
                         someone you can vouch for.",
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
    let ends: Vec<(usize, f32)> = ui.ctx().data(|d| d.get_temp(ends_id)).unwrap_or_default();
    // (measured from the current scroll offset, so a queue scrolled to an open form still ends
    // between two cards rather than through the next applicant's facts)
    let off_id = egui::Id::new("admin_queue_off");
    let off: f32 = ui.ctx().data(|d| d.get_temp(off_id)).unwrap_or(0.0);
    let vp_h = ends.iter().map(|(_, e)| *e - off).filter(|e| *e <= room && *e >= room * 0.5).fold(None, |a: Option<f32>, e| Some(a.map_or(e, |a| a.max(e)))).map_or(room, |e| e + 2.0);
    // (stop offset, cards fully above it) for this frame, and each card's bottom for the count
    let mut card_ends: Vec<(usize, f32)> = Vec::new();
    // each card's top, for counting the applications still below
    let mut bottoms: Vec<f32> = Vec::new();
    // The viewport stops between whole cards at every width: a card begun at the fold showed a
    // name and buttons over a blank body, as if its facts were missing.
    // The cards are as wide as their facts need (the widest card's, so all share one right edge),
    // not the window: a fact grid that stopped two thirds across left the buttons alone in the far
    // corner over an empty band.
    let full = ui.available_width();
    let wide_q = full - 28.0 >= 620.0;
    let nat = items.iter().map(|a| facts_natural_w(ui, a, wide_q)).fold(0.0_f32, f32::max);
    let col_w = (nat + 28.0 + 6.0).max(if wide_q { 668.0 } else { 468.0 }).min(full);
    let area = egui::ScrollArea::vertical()
        .id_salt("admin_queue")
        .max_height(vp_h)
        .max_width(col_w)
        .auto_shrink([false, false])
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded);
    // The queue scrolls inside a page that also scrolls, so its handle is the quiet one: thin,
    // muted, against the cards' edge, with no track to compete with the page's bar.
    let out = kit::quiet_scroll_area(ui, area, |ui| {
            ui.set_width(col_w);
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
                let mut form_top = None;
                let card = kit::card().stroke(stroke).inner_margin(egui::Margin::same(14)).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    // while one decision is open the other applications wait. Their facts stay fully
                    // legible; only their buttons are shown as not-interactive, so no one reads an open
                    // queue as a set of live controls.
                    application(ui, app, open, busy, &mut ask);
                    if open {
                        ui.add_space(10.0);
                        form_top = Some(ui.cursor().top());
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
                // the clean place to stop: under the card
                let n = bottoms.len();
                bottoms.push(card.response.rect.top() - content_top);
                card_ends.push((n + 1, card.response.rect.bottom() - content_top));
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
                        // Bring the card's top in, so the name, school and waiting time stay above
                        // the form; a card taller than the viewport brings its foot in instead, so
                        // the action row (Approve, or the password's Confirm) is never cut by the
                        // window's bottom edge.
                        if r.height() > vp.height() + 1.0 {
                            // the form starts at its own rule and names the applicant ("Approve
                            // Rosa Delgado"), so the viewport's top edge lands there rather than
                            // through a fact row; failing that, the card's foot is brought in
                            let ft = form_top.unwrap_or(r.top());
                            if r.bottom() - ft <= vp.height() - 8.0 {
                                if (ft - vp.top()).abs() > 6.0 {
                                    let target = egui::Rect::from_min_size(egui::pos2(r.left(), ft - 4.0), egui::vec2(r.width(), 1.0));
                                    ui.scroll_to_rect(target, Some(egui::Align::Min));
                                }
                            } else if r.bottom() > vp.bottom() || r.bottom() < vp.bottom() - 6.0 {
                                let target = egui::Rect::from_min_size(egui::pos2(r.left(), r.bottom() - 1.0), egui::vec2(r.width(), 1.0));
                                ui.scroll_to_rect(target, Some(egui::Align::Max));
                            }
                        } else if r.top() > vp.top() + 6.0 || r.top() < vp.top() {
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
    ui.ctx().data_mut(|d| {
        d.insert_temp(ends_id, card_ends);
        d.insert_temp(off_id, out.state.offset.y);
    });
    let (bar, _) = ui.allocate_exact_size(egui::vec2(col_w, 24.0), egui::Sense::hover());
    let seen_to = out.state.offset.y + out.inner_rect.height() + 2.0;
    if out.content_size.y > seen_to {
        // name what waits: the applications whose name row is below the viewport's edge (a card
        // already begun on screen is not "below")
        let hidden = bottoms.iter().filter(|top| **top + 48.0 > seen_to).count();
        let cue = match hidden {
            0 => "More below".to_string(),
            1 => "1 more application below".to_string(),
            n => format!("{n} more applications below"),
        };
        kit::more_below(ui, bar, &cue);
    }
    // The page's footer starts under the window's edge, never as a sliver beneath the queue: the
    // queue's viewport and cue own the window down to its rim.
    let rest = ui.clip_rect().bottom() - ui.cursor().top();
    if rest > 0.0 {
        ui.add_space(rest);
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
fn application(ui: &mut Ui, app: &Application, open: bool, _busy: bool, ask: &mut Option<Ask>) {
    let wide = ui.available_width() >= 620.0;
    // the buttons ride on the name row whenever it has room for them (a phone too), which keeps a
    // card short enough that the next applicant shows under it
    let buttons_beside = ui.available_width() >= 420.0;
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
        // While one decision is open the other cards' buttons stay live: pressing one opens that
        // applicant's decision in place of the open one (a faded "disabled" pair read as broken).
        if !open && buttons_beside {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                if kit::ghost_button(ui, "Refuse").clicked() {
                    *ask = Some(Ask::Open(app.clone(), false));
                }
                if kit::solid_primary(ui, "Approve").clicked() {
                    *ask = Some(Ask::Open(app.clone(), true));
                }
            });
        }
    });
    ui.add_space(if wide { 6.0 } else { 4.0 });
    let rows = fact_rows(app);
    application_facts(ui, app, &rows, wide);
    staff_page_row(ui, app);
    if !open && !buttons_beside {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            if kit::solid_primary(ui, "Approve").clicked() {
                *ask = Some(Ask::Open(app.clone(), true));
            }
            if kit::ghost_button(ui, "Refuse").clicked() {
                *ask = Some(Ask::Open(app.clone(), false));
            }
        });
    }
}

/// The same six facts on every card, in the same places, so cards compare at a glance.
fn fact_rows(app: &Application) -> [(&'static str, String, bool); 6] {
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
    [
        ("Email", app.email.clone(), false),
        ("Waiting", age_text(app.age_seconds), app.age_seconds >= 2 * 86_400),
        ("School", school, false),
        ("Domain", format!("{}  ·  {domain_hist}", app.email_domain), false),
        ("Your label", label, false),
        ("Applied before", before, app.prior_rejections > 0),
    ]
}

fn label_width(ui: &Ui, ks: &[&str]) -> f32 {
    ks.iter()
        .map(|k| ui.fonts_mut(|f| f.layout_no_wrap(k.to_uppercase(), egui::FontId::new(11.5, theme::bold_family()), theme::text_col()).size().x))
        .fold(0.0_f32, f32::max)
        .ceil()
}

fn value_width(ui: &Ui, v: &str) -> f32 {
    ui.fonts_mut(|f| f.layout_no_wrap(v.to_string(), egui::FontId::new(13.0, theme::bold_family()), theme::text_col()).size().x).ceil()
}

/// How wide a card's facts (and its name row with the buttons) need to be, inside the card's margin.
fn facts_natural_w(ui: &Ui, app: &Application, wide: bool) -> f32 {
    let rows = fact_rows(app);
    let name_row = value_width(ui, &app.who()) + 260.0;
    let facts = if wide {
        let v1 = rows.iter().step_by(2).map(|r| value_width(ui, &r.1)).fold(0.0_f32, f32::max);
        let v2 = rows.iter().skip(1).step_by(2).map(|r| value_width(ui, &r.1)).fold(0.0_f32, f32::max);
        label_width(ui, &["Email", "School", "Your label"]) + label_width(ui, &["Waiting", "Domain", "Applied before"]) + v1 + v2 + 4.0 * 12.0 + 24.0
    } else {
        let v = rows.iter().map(|r| value_width(ui, &r.1)).fold(0.0_f32, f32::max);
        label_width(ui, &["Email", "Waiting", "School", "Domain", "Your label", "Applied before"]) + 12.0 + v + 8.0
    };
    facts.max(name_row)
}

fn application_facts(ui: &mut Ui, app: &Application, rows: &[(&'static str, String, bool); 6], wide: bool) {
    let value = |ui: &mut Ui, v: &str, strong: bool| {
        let t = if strong { theme::strong(v).size(13.0) } else { theme::text(v).size(13.0) };
        ui.add(egui::Label::new(t).wrap_mode(egui::TextWrapMode::Extend));
    };
    // The labels are read before the values, so they cannot be the faintest type on the card: no
    // thin-space letterspacing at 9 px, just a tight bold caps at a size that holds up.
    let fact_label = |k: &str| RichText::new(k.to_uppercase()).family(theme::bold_family()).size(11.5).color(theme::text_col());
    // Each label column is as wide as its longest label, so a value sits a short gap from its label
    // instead of across a dead band.
    let label_w = |ui: &Ui, ks: &[&str]| {
        ks.iter()
            .map(|k| ui.fonts_mut(|f| f.layout_no_wrap(k.to_uppercase(), egui::FontId::new(11.5, theme::bold_family()), theme::text_col()).size().x))
            .fold(0.0_f32, f32::max)
            .ceil()
    };
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = 18.0;
        let grid = egui::Grid::new(("admin_facts", &app.request_id)).spacing([12.0, if wide { 6.0 } else { 3.0 }]).min_col_width(0.0);
        if wide {
            // One table, two pairs a row, both pairs ending on the same right edge as the form
            // fields below (they used to stop ~50 px short of them).
            let lw1 = label_w(ui, &["Email", "School", "Your label"]);
            let lw2 = label_w(ui, &["Waiting", "Domain", "Applied before"]);
            let gap = 12.0;
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
            let lw = label_w(ui, &["Email", "Waiting", "School", "Domain", "Your label", "Applied before"]);
            grid.num_columns(2).show(ui, |ui| {
                for (k, v, st) in rows {
                    ui.allocate_ui_with_layout(egui::vec2(lw, 18.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_min_width(lw);
                        ui.label(fact_label(k));
                    });
                    ui.add(egui::Label::new(if *st { theme::strong(v.as_str()).size(13.0) } else { theme::text(v.as_str()).size(13.0) }).wrap());
                    ui.end_row();
                }
            });
        }
    });
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
/// The value sits on its own line in a quiet field-coloured box under its label, so the label and
/// the quoted answer no longer run together as one sentence.
fn fact_row(ui: &mut Ui, label: &str, value: &str) {
    ui.label(RichText::new(label).family(theme::bold_family()).size(12.5).color(theme::text_col()));
    ui.add_space(3.0);
    egui::Frame::new()
        .fill(theme::field())
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.set_max_width(ui.available_width().min(FIELD_W + 120.0));
            ui.add(egui::Label::new(theme::text(value).size(13.5)).wrap());
        });
    ui.add_space(4.0);
}

/// The link the applicant gave as proof, clickable: the reviewer opens it in a browser. Plain text in a
/// link widget (never fetched by the app), so a hostile URL cannot do anything until the admin clicks it.
fn staff_page_row(ui: &mut Ui, app: &Application) {
    if app.staff_page.is_empty() {
        return;
    }
    ui.add_space(7.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 7.0;
        ui.label(kit::small_caps_text("Staff page"));
        let link = ui.add(egui::Link::new(theme::accent(app.staff_page.as_str()).size(12.5)));
        if link.clicked() {
            crate::platform::open_url(&app.staff_page);
        }
        link.on_hover_text("Open it in your browser and check they are on it");
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
            let missing = d.reason.trim().is_empty();
            gate_label(ui, "Why (the applicant reads this)", missing);
            let r = field(
                ui,
                "admin_reason",
                &mut d.reason,
                "e.g. We could not find you in the school's staff list. Write again from your school address.",
                false,
            );
            if missing {
                kit::attention_ring(ui, &r);
            }
        }
    };
    let right = |ui: &mut Ui, d: &mut Decision| {
        if d.approve {
            let missing = d.evidence_note.trim().is_empty();
            gate_label(ui, "Where you checked it (kept in the audit trail)", missing);
            let r = field(ui, "admin_evidence_note", &mut d.evidence_note, "e.g. their staff page, which subject it lists, or who confirmed it", true);
            if missing {
                kit::attention_ring(ui, &r);
            }
            ui.add_space(6.0);
        }
        // The server needs a private note on every decision (it is in the audit trail), so it is gated like the rest.
        let note_missing = d.note.trim().is_empty();
        gate_label(ui, "Your note (never shown to the applicant)", note_missing);
        let r = field(ui, "admin_note", &mut d.note, "a private reminder for the audit trail", true);
        if note_missing {
            kit::attention_ring(ui, &r);
        }
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
    // A gated action must look gated, and say why: with the evidence note (or the refusal's
    // reason) empty the button waits in the muted face, and the warning line directly under it
    // names the missing step (the password step below uses the same pair).
    let ready = d.problem().is_none();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        let label = if d.approve { "Approve" } else { "Refuse" };
        if kit::primary_button_on(ui, label, ready).clicked() && ready {
            *ask = Some(Ask::Send);
        }
        if kit::ghost_button(ui, "Cancel").clicked() {
            *ask = Some(Ask::Cancel);
        }
    });
    if !ready {
        ui.add_space(2.0);
        kit::gated_reason(ui, gate_reason(d));
    }
}

/// The missing step a waiting send button names, in a few words.
fn gate_reason(d: &Decision) -> &'static str {
    if !d.approve {
        "Write why to refuse"
    } else if d.evidence >= EVIDENCE.len() {
        "Choose what you checked to approve"
    } else {
        "Add where you checked it to approve"
    }
}

/// The password a change needs, asked once for five minutes of changes. A plain part of the open
/// card under a top rule (no tinted, bordered box of its own inside the card), with its label on its
/// own line above the field and the same waiting button and warning line as the decision form.
/// Returns the block's rect.
fn password_block(ui: &mut Ui, m: &mut AdminModel, ask: &mut Option<Ask>) -> egui::Rect {
    let top = ui.cursor().top();
    ui.separator();
    ui.add_space(8.0);
    let mut enter = false;
    let confirm_on = !m.password.is_empty();
    ui.label(theme::strong("Confirm your password to send").size(13.5));
    ui.add_space(6.0);
    let mut confirm_left = ui.cursor().left();
    let row_left = ui.cursor().left();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        let w = (ui.available_width() - 250.0).clamp(200.0, 420.0);
        let r = ui.add(
            egui::TextEdit::singleline(&mut m.password)
                .id(egui::Id::new("admin_password"))
                .password(true)
                .hint_text("Your password")
                .desired_width(w),
        );
        if !confirm_on {
            kit::attention_ring(ui, &r);
        }
        enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        ui.add_space(4.0);
        let b = kit::primary_button_on(ui, "Confirm", confirm_on);
        confirm_left = b.rect.left();
        if (b.clicked() || enter) && confirm_on {
            *ask = Some(Ask::Confirm);
        }
        if kit::ghost_button(ui, "Close").on_hover_text("Close this password box and keep the approval open").clicked() {
            *ask = Some(Ask::CancelConfirm);
        }
    });
    if !confirm_on {
        // directly under the waiting Confirm
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.add_space((confirm_left - row_left).max(0.0));
            kit::gated_reason(ui, "Type your password to send");
        });
    }
    ui.add_space(6.0);
    ui.add(egui::Label::new(theme::muted("Used once over the secure connection and never stored; good for five minutes of changes.").size(12.0)).wrap());
    egui::Rect::from_min_max(egui::pos2(row_left, top), egui::pos2(ui.max_rect().right(), ui.cursor().top()))
}

// ---- the growth report ---------------------------------------------------------------------------

/// The table's columns after the tag.
const GROWTH_COLS: [&str; 5] = ["Signups", "Approved", "Purchases", "Revenue", "Refunds"];
/// The width of a number column (wide enough for "$12,345.00" at 13 px).
const NUM_W: f32 = 76.0;

/// A row's values, in [`GROWTH_COLS`] order.
pub fn growth_cells(r: &SourceRow) -> [String; 5] {
    [r.signups.to_string(), r.approved.to_string(), r.purchases.to_string(), dollars(r.revenue_cents), r.refunds.to_string()]
}

/// A table heading, in the fact grid's caps.
fn col_head(k: &str) -> RichText {
    RichText::new(k.to_uppercase()).family(theme::bold_family()).size(11.5).color(theme::text_col())
}

/// A number, right-aligned in its column so the digits line up.
fn num_cell(ui: &mut Ui, t: RichText) {
    ui.allocate_ui_with_layout(egui::vec2(NUM_W, 18.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.set_min_width(NUM_W);
        ui.add(egui::Label::new(t).wrap_mode(egui::TextWrapMode::Extend));
    });
}

/// The growth report: a window choice, the per-tag table with its totals, and the visit counts.
fn growth_body(ui: &mut Ui, m: &mut AdminModel, session: Option<SharedSession>) {
    pageshell::heading(ui, "Growth");
    ui.add_space(4.0);
    ui.scope(|ui| {
        ui.set_max_width(ui.available_width().min(pageshell::MEASURE));
        ui.add(
            egui::Label::new(
                theme::muted(
                    "Sign-ups by the link they came from: a link to the app ending in ?src=a-tag (for example ?src=fb-math) counts \
                     under that tag. Counts only, no cookies, and no account is named here. Revenue is gross, before refunds and fees.",
                )
                .size(13.0),
            )
            .wrap(),
        );
    });
    ui.add_space(12.0);
    if m.growth_busy() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(300));
    }

    // ---- the window, and reading it again ---------------------------------------------------------
    let mut days = None;
    let mut reload = false;
    let loaded = matches!(m.growth, GrowthState::Loaded(_));
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        for d in GROWTH_DAYS {
            if kit::chip_toggle(ui, &format!("{d} days"), m.growth_days == d).clicked() {
                days = Some(d);
            }
        }
        ui.add_space(4.0);
        if m.growth_busy() {
            ui.spinner();
        } else if loaded && kit::chip_toggle(ui, "Read again", false).on_hover_text("Fetch the report again").clicked() {
            reload = true;
        }
    });
    ui.add_space(8.0);
    let line = match &m.growth {
        GrowthState::Loaded(g) => growth_summary(g),
        GrowthState::Reading => "Reading the report...".to_string(),
        GrowthState::Failed(p) => p.message.clone(),
        GrowthState::Closed => "The report has not been read yet.".to_string(),
    };
    ui.add(egui::Label::new(theme::strong(line).size(13.5)).wrap());
    if matches!(m.growth, GrowthState::Failed(_)) && kit::primary_button(ui, "Try again").clicked() {
        reload = true;
    }
    if let Some(n) = m.growth_notice.clone() {
        ui.add_space(4.0);
        ui.add(egui::Label::new(theme::text(n).size(13.0)).wrap());
    }
    if let Some(d) = days {
        m.set_growth_days(d, session.clone());
    } else if reload {
        m.load_growth(session.clone());
    }
    let GrowthState::Loaded(g) = &m.growth else { return };
    let g = g.clone();
    ui.add_space(10.0);

    // ---- the table (scrolls sideways on a narrow window rather than squeezing the numbers) ---------
    egui::ScrollArea::horizontal().id_salt("growth_table").auto_shrink([false, true]).show(ui, |ui| {
        kit::card().inner_margin(egui::Margin::same(14)).show(ui, |ui| {
            egui::Grid::new("growth_grid").num_columns(6).spacing([16.0, 7.0]).min_col_width(0.0).show(ui, |ui| {
                ui.label(col_head("Source"));
                for k in GROWTH_COLS {
                    num_cell(ui, col_head(k));
                }
                ui.end_row();
                if g.sources.is_empty() {
                    ui.label(theme::muted("No sign-ups in this window.").size(13.0));
                    ui.end_row();
                }
                for r in &g.sources {
                    let name = source_label(&r.source);
                    let t = if r.source.is_some() { theme::text(name).size(13.0) } else { theme::muted(name).size(13.0) };
                    ui.add(egui::Label::new(t).wrap_mode(egui::TextWrapMode::Extend));
                    for v in growth_cells(r) {
                        num_cell(ui, theme::text(v).size(13.0));
                    }
                    ui.end_row();
                }
                ui.label(theme::strong("Total").size(13.0));
                for v in growth_cells(&g.totals) {
                    num_cell(ui, theme::strong(v).size(13.0));
                }
                ui.end_row();
            });
        });
    });
    ui.add_space(14.0);

    // ---- visits -----------------------------------------------------------------------------------
    if let Some(line) = visits_line(&g.visits) {
        ui.add(egui::Label::new(theme::muted(line).size(13.0)).wrap());
        return;
    }
    let v = &g.visits;
    ui.label(theme::strong("Visits by referrer").size(14.0));
    ui.add_space(2.0);
    let window = v.days.unwrap_or(g.days);
    let total = if v.total == 1 { "1 visit".to_string() } else { format!("{} visits", v.total) };
    ui.scope(|ui| {
        ui.set_max_width(ui.available_width().min(pageshell::MEASURE));
        ui.add(
            egui::Label::new(
                theme::muted(format!(
                    "{total} in the last {window} days, from Cloudflare Web Analytics (sampled estimates). Cloudflare does not see the \
                     ?src= tag, so these are by the site that linked here, not by tag."
                ))
                .size(12.5),
            )
            .wrap(),
        );
    });
    ui.add_space(6.0);
    if v.by_referrer.is_empty() {
        ui.label(theme::muted("No visits in this window.").size(13.0));
        return;
    }
    egui::Grid::new("growth_referrers").num_columns(2).spacing([16.0, 5.0]).min_col_width(0.0).show(ui, |ui| {
        ui.label(col_head("Referrer"));
        num_cell(ui, col_head("Visits"));
        ui.end_row();
        for r in &v.by_referrer {
            let name = referrer_label(&r.referrer);
            let t = if r.referrer.is_some() { theme::text(name).size(13.0) } else { theme::muted(name).size(13.0) };
            ui.add(egui::Label::new(t).wrap_mode(egui::TextWrapMode::Extend));
            num_cell(ui, theme::text(r.visits.to_string()).size(13.0));
            ui.end_row();
        }
    });
}
