//! Circle banks in My Banks (phase D1), drawn: the badge on the Banks list and the status strip on a linked bank's
//! page. All logic is in [`super::bank_links`] and the circles model; this only draws and passes clicks on. The
//! hooks in `src/bankui.rs` (approved by the owner) are one call each: [`badge`], [`strip`] and [`adding_rows`].

use egui::{Margin, Ui};
use web_time::Instant;

use super::bank_links::{Badge, StripButton, StripView, perm_of, unreadable_view};
use super::browse::now_ms;
use super::circle_edit::EditHost;
use super::publish::snapshot_of;
use crate::{kit, theme};

/// The quiet line under a linked bank's name on the Banks list: "Circle", the circle's name, at most one state.
/// Reads this computer only; draws nothing for a bank with no link.
pub fn badge(ui: &mut Ui, app: &mut crate::App, bank_id: &str) {
    let crate::App { shared, store, .. } = app;
    let Some(bank) = store.banks.iter().find(|b| b.id == bank_id) else { return };
    let c = &mut shared.circles;
    if let Some(b) = c.links.badge(&mut c.edit, bank) {
        let warns = c.edit.with_link(bank_id, super::bank_links::badge_warns).unwrap_or(false);
        badge_line(ui, &b, warns);
    }
}

/// Owner decision (b): the banks being added in the background, one quiet row each, for the top of the Banks list;
/// (critic 66 I4) and how an add that finished away from the Shared page went when it did not go as asked: it
/// failed, or the bank was added but is not connected to the circle. Those stay until dismissed. Reads the circles
/// model only and never starts a request; draws nothing when there is nothing to say. Drawn by the one-line hook in
/// `bankui::banks_body` (approved by the owner).
pub fn adding_rows(ui: &mut Ui, app: &mut crate::App) {
    // (critic 68 minor 3) "Open it" on a bank that was added but not connected
    if let Some(id) = adding_rows_with(ui, &mut app.shared.circles, &mut app.account)
        && app.store.bank(&id).is_some()
    {
        app.open_bank(&id);
    }
}

/// [`adding_rows`] with its parts (tests give a stand-in account window): the rows, then what their buttons ask of
/// the page. Returns the bank "Open it" asked for.
pub(super) fn adding_rows_with(ui: &mut Ui, c: &mut super::circles::CirclesModel, account: &mut dyn AccountHooks) -> Option<String> {
    adding_rows_of(ui, c);
    // (critic 67 I1) "Sign in..." on a row: the Account window
    if c.take_adds_sign_in() {
        account.reveal();
    }
    c.take_open_bank()
}

pub(super) fn adding_rows_of(ui: &mut Ui, c: &mut super::circles::CirclesModel) {
    use super::circles::AddState;
    /// What a row offers besides its words.
    #[derive(Clone, Copy, PartialEq)]
    enum Remedy {
        None,
        SignIn,
        OpenCircle,
        OpenBank,
    }
    /// One row: the adds it stands for (none: the last run's unfinished adds), its words, whether it is a result (it
    /// can be dismissed, unless it waits for a sign-in), what it offers, and a spinner.
    struct Row {
        adds: Vec<usize>,
        line: String,
        result: bool,
        remedy: Remedy,
        spinner: bool,
        /// (critic 71 I2) a restore, trim or send result of a circle bank with no copy on this computer
        said: Option<u64>,
    }
    let mut rows: Vec<Row> = Vec::new();
    // (critic 71 I2) how something went for a circle bank with no copy here: said here, named (never above another
    // bank), until dismissed
    for (id, line) in c.edit.results_bankless() {
        rows.push(Row { adds: Vec::new(), line, result: true, remedy: Remedy::None, spinner: false, said: Some(id) });
    }
    // (critic 69 minor 3) adds the last run did not finish: said once, without titles (another account may be here)
    let unfinished = c.unfinished_at_start();
    if unfinished > 0 {
        let line = if unfinished == 1 { "verSHion closed before 1 bank you asked to add was finished. Add it again from the circle's page.".to_string() } else { format!("verSHion closed before {unfinished} banks you asked to add were finished. Add them again from the circle's page.") };
        rows.push(Row { adds: Vec::new(), line, result: true, remedy: Remedy::None, spinner: false, said: None });
    }
    let need_sign_in = c.adds_need_sign_in();
    // (critic 69 minor 6) another account's adds are said in one line, without the titles of its circle's banks
    let now_tag = c.account_tag();
    let others: Vec<usize> = (0..c.adds.len()).filter(|i| { let a = &c.adds[*i]; a.account.is_some() && a.account != now_tag && !a.dismissed && !matches!(a.state, AddState::Done { .. }) }).collect();
    if !others.is_empty() {
        let line = if others.len() == 1 { "1 bank that another account on this computer asked to add was not added. Sign in with that account, then add it again from the circle's page.".to_string() } else { format!("{} banks that another account on this computer asked to add were not added. Sign in with that account, then add them again from the circle's page.", others.len()) };
        rows.push(Row { adds: others.clone(), line, result: true, remedy: Remedy::None, spinner: false, said: None });
    }
    for (i, a) in c.adds.iter().enumerate() {
        if others.contains(&i) {
            continue;
        }
        let title = super::api::plain(&a.title);
        let row = |line: String, result: bool, remedy: Remedy| Some(Row { adds: vec![i], line, result, remedy, spinner: a.state == AddState::Working, said: None });
        let r = match &a.state {
            AddState::Waiting | AddState::Working if need_sign_in => {
                // (critic 67 I1, I2) the session ended: the add waits for the same account to sign in again
                let circle = c.circle_name_of(&a.cid);
                let from = if circle.is_empty() { String::new() } else { format!(" from {circle}") };
                let what = if a.own { "making your own copy of" } else { "adding" };
                row(format!("Sign in to finish {what} \"{title}\"{from}."), true, Remedy::SignIn)
            }
            AddState::Waiting | AddState::Working => {
                let circle = c.circle_name_of(&a.cid);
                let from = if circle.is_empty() { String::new() } else { format!(" from {circle}") };
                let what = if a.own { "Making your own copy of" } else { "Adding" };
                row(format!("{what} \"{title}\"{from}..."), false, Remedy::None)
            }
            AddState::Failed(_) if !a.dismissed && a.account.is_some() && a.account == now_tag && super::circles::stopped_by_switch(&a.state) => {
                // (critic 71 minor 5) the account that asked for it, signed in again: told it was not made
                row(format!("\"{title}\" was not added: another account signed in before it was finished. Add it again from the circle's page."), true, Remedy::OpenCircle)
            }
            AddState::Failed(p) if !a.dismissed => {
                // (critic 68 minors 1, 3) the circle's words; "try again" and the circle's page only where trying
                // again can help
                let msg = super::circles_text::circle_words(p.message.trim_end_matches(' '));
                if p.retry {
                    row(format!("\"{title}\" couldn't be added: {msg} Try again from the circle's page."), true, Remedy::OpenCircle)
                } else {
                    row(format!("\"{title}\" couldn't be added: {msg}"), true, Remedy::None)
                }
            }
            AddState::Done { linked: false, why, bank_id, .. } if !a.dismissed && !a.own => {
                // (critic 68 minor 3) adding a bank that was not connected again helps only when no other bank of this
                // computer has the circle bank's link
                let elsewhere = c.edit.linked_elsewhere(&a.cid, &a.bid, bank_id).is_some();
                let why = why.as_deref().map_or(String::new(), |w| format!(": {w}"));
                let again = if elsewhere { "" } else { " Delete it in My Banks, then add it again." };
                row(format!("\"{title}\" was added, but it is not connected to the circle{why}.{again}"), true, Remedy::OpenBank)
            }
            _ => None,
        };
        rows.extend(r);
    }
    let mut dismiss: Option<usize> = None;
    let mut chosen: Option<(Remedy, usize)> = None;
    for (k, r) in rows.iter().enumerate() {
        kit::card().inner_margin(Margin::symmetric(14, 9)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            super::circles_ui::text_row(ui, |ui| {
                if r.spinner {
                    ui.add(egui::Spinner::new().size(14.0));
                }
                let text = if r.result { theme::text(r.line.as_str()).size(13.0) } else { theme::muted(r.line.as_str()).size(13.0) };
                ui.add(egui::Label::new(text).wrap());
                let label = match r.remedy {
                    Remedy::SignIn => Some("Sign in..."),
                    Remedy::OpenCircle => Some("Open the circle"),
                    Remedy::OpenBank => Some("Open it"),
                    Remedy::None => None,
                };
                if let Some(l) = label
                    && kit::ghost_button(ui, l).clicked()
                {
                    chosen = Some((r.remedy, k));
                }
                // (a row that waits for a sign-in is not a result to dismiss: it goes once the add is made)
                if r.result && r.remedy != Remedy::SignIn && kit::ghost_button(ui, "Dismiss").clicked() {
                    dismiss = Some(k);
                }
            });
        });
        ui.add_space(6.0);
    }
    if let Some(r) = dismiss.and_then(|k| rows.get(k)) {
        if let Some(id) = r.said {
            c.edit.dismiss_result(id);
        } else if r.adds.is_empty() {
            c.dismiss_unfinished();
        }
        for i in &r.adds {
            if let Some(a) = c.adds.get_mut(*i) {
                a.dismissed = true;
            }
        }
    }
    let Some((remedy, k)) = chosen else { return };
    let Some(&i) = rows.get(k).and_then(|r| r.adds.first()) else { return };
    match remedy {
        Remedy::SignIn => c.sign_in_for_adds(),
        Remedy::OpenCircle => {
            let a = &c.adds[i];
            super::shared_ui::open_circle(ui.ctx(), &a.cid, Some(&a.bid));
        }
        Remedy::OpenBank => {
            if let AddState::Done { bank_id, .. } = &c.adds[i].state {
                let id = bank_id.clone();
                c.ask_open_bank(&id);
            }
        }
        Remedy::None => {}
    }
}

/// (critic 71) One result: its words, then a small Dismiss. True when Dismiss was pressed.
pub(super) fn result_line(ui: &mut Ui, text: &str) -> bool {
    super::circles_ui::text_row(ui, |ui| {
        // (the same words as the strip's other notes; Dismiss beside them)
        ui.add(egui::Label::new(theme::muted(text).size(12.5)).wrap());
        kit::ghost_button(ui, "Dismiss").clicked()
    })
}

/// Results drawn one under another; the id of the one dismissed.
pub(super) fn result_lines(ui: &mut Ui, items: &[(u64, String)]) -> Option<u64> {
    let mut out = None;
    for (id, t) in items {
        if result_line(ui, t) {
            out = Some(*id);
        }
    }
    out
}

fn galley_width(ui: &Ui, t: egui::RichText) -> f32 {
    egui::WidgetText::from(t).into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Body).size().x
}

/// One line: the pill, the circle's name (cut with an ellipsis when narrow), then the state, which is never cut.
pub(super) fn badge_line(ui: &mut Ui, b: &Badge, warns: bool) {
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = 16.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            super::shared_ui::step_pill(ui, "Circle", theme::accent_faded(), theme::text_col());
            let name = theme::muted(b.circle.as_str()).size(12.0);
            // (critic 64 polish 11) a copy that can no longer be sent says so in the warning colour
            let note = b.note.as_ref().map(|n| if warns { egui::RichText::new(format!("· {n}")).color(theme::bad()) } else { theme::accent(format!("· {n}")) }.size(12.0));
            let note_w = note.clone().map_or(0.0, |n| galley_width(ui, n) + 6.0);
            let name_w = galley_width(ui, name.clone()).min((ui.available_width() - note_w).max(24.0));
            ui.allocate_ui_with_layout(egui::vec2(name_w, 16.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.add(egui::Label::new(name).truncate());
            });
            if let Some(n) = note {
                ui.add(egui::Label::new(n).extend());
            }
        });
    });
}

/// The status strip of a linked bank's page, under its name. Returns true when one of the circle's screens (send,
/// get latest, history) is open for this bank: it was drawn here in place of the questions. Draws nothing, and
/// asks nothing, for a bank with no link. Saves the banks itself when a merge changed them.
pub fn strip(ui: &mut Ui, app: &mut crate::App, bank_id: &str) -> bool {
    let snap = snapshot_of(&app.account.model);
    let crate::App { shared, store, browse, account, .. } = app;
    let out = strip_with(ui, &mut shared.circles, &mut store.banks, &mut browse.model, account, &snap, Instant::now(), now_ms(), bank_id);
    if out.save {
        store.save_banks();
    }
    out.page
}

/// What the bank's page needs of the account window (tests give a stand-in: the real one may read the keychain).
pub(super) trait AccountHooks {
    fn is_open(&self) -> bool;
    fn open_quiet(&mut self);
    fn reveal(&mut self);
}

impl AccountHooks for super::ui::AccountUi {
    fn is_open(&self) -> bool {
        super::ui::AccountUi::is_open(self)
    }
    fn open_quiet(&mut self) {
        super::ui::AccountUi::open_quiet(self)
    }
    fn reveal(&mut self) {
        super::ui::AccountUi::reveal(self)
    }
}

/// What [`strip_with`] did: a circle's screen was drawn in place of the questions (`page`), the banks changed (`save`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct StripOut {
    pub page: bool,
    pub save: bool,
}

/// [`strip`] with its parts (tests drive it with a scripted session and a stand-in account window).
#[allow(clippy::too_many_arguments)]
pub(super) fn strip_with(ui: &mut Ui, c: &mut super::circles::CirclesModel, banks: &mut [crate::model::QuestionBank], browse: &mut super::browse::BrowseModel, account: &mut dyn AccountHooks, snap: &super::publish::AccountSnapshot, now: Instant, ms: i64, bank_id: &str) -> StripOut {
    let ctx = ui.ctx().clone();
    let mut out = StripOut::default();
    let unreadable = c.edit.unreadable();
    if unreadable.is_none() && c.edit.with_link(bank_id, |_| ()).is_none() {
        return out;
    }
    if !c.has_repaint() {
        let r = ctx.clone();
        c.set_repaint(std::sync::Arc::new(move || r.request_repaint()));
    }
    // The same account rule as the Shared page: what was read for one account is never shown to another.
    c.note_account(snap);
    // (critic 71 I1) which frame this is: a History not drawn since the last one is not in front of the person
    c.edit.set_pass(ctx.cumulative_frame_nr());
    c.edit.frame(now, snap);
    c.links.frame(&mut c.edit, ms);
    // (critic 73 I1) a bank whose newest version is not known after an answer: checked at once
    for local in c.edit.take_check_asks() {
        c.links.check_now(&local);
    }
    // (critic 82 I1) and one the circle answered for since a check could not reach it
    for local in c.edit.take_answered() {
        c.links.answered(&local);
    }
    // (critic 65 minor, critic 66 I7) a send or a Get latest that met an ended session: the strip says "Sign in..."
    if c.edit.take_signed_out() {
        c.links.session_ended();
    }
    // (critic 63 I2: a signed-out answer stops the checks in `links` itself, until the account changes or the person asks)
    let _ = c.links.take_signed_out();
    // Counts, a send the person asked for, a review to build, a merge to apply: the bank changes only here.
    out.save |= c.edit.sync(banks);
    for a in c.edit.take_advanced() {
        browse.set_circle_version(&a.circle_bank_id, &a.local_bank_id, a.version);
    }
    if c.links.busy() || c.edit.busy() {
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    }
    if unreadable.is_some() {
        let mut again = false;
        draw_strip(ui, &unreadable_view(), &mut |b| again |= b == StripButton::TryAgain, &mut |_| {}, None);
        if again {
            c.edit.read_links_again();
        }
        return out;
    }
    let Some(bank) = banks.iter().find(|b| b.id == bank_id) else { return out };
    let back = format!("< Back to {}", super::api::plain(&bank.name));
    // A check would start: the account is looked up quietly (once; a saved sign-in is used, nothing is asked).
    let gone = c.edit.with_link(bank_id, |l| l.gone()).unwrap_or(false);
    if !account.is_open() && c.links.will_check(bank_id, gone, now) {
        account.open_quiet();
    }
    let Some(view) = c.links.strip(&mut c.edit, bank, snap, now, ms) else { return out };
    // (critic 64 polish 13) A History or a version the person left by going elsewhere is not shown again in place of
    // the questions; a send or a Get latest under way stays.
    let frame = ui.ctx().cumulative_frame_nr();
    let hosted = c.edit.host.as_ref().is_some_and(|h| h.local_id == bank_id);
    if hosted && matches!(c.edit.view, super::circle_edit::EditView::History(_) | super::circle_edit::EditView::Version(_)) && c.edit.host_frame.is_some_and(|f| f + 1 < frame) {
        c.edit.close();
    }
    if c.edit.active() && c.edit.host.as_ref().is_some_and(|h| h.local_id == bank_id) {
        c.edit.host_frame = Some(frame);
        super::circle_edit_ui::draw_page(ui, c);
        // (critic 66 I7) "Open account..." on the send page opens the Account window here too
        if c.edit.take_account_ask() {
            account.reveal();
        }
        if !c.edit.active() {
            c.edit.host = None;
        }
        out.page = true;
        return out;
    }
    let Some((cid, bid, title, perm)) = c.edit.with_link(bank_id, |l| (l.circle_id.clone(), l.circle_bank_id.clone(), l.title.clone(), perm_of(l))) else { return out };
    let mut pressed: Option<StripButton> = None;
    let mut menu: Option<Menu> = None;
    // (critic 67 M3) "Unlink?" names the unsent changes it would strand; (critic 69 minor 2) and a send from this bank
    // on its way, which still reaches the circle
    let asked = c.edit.forget_asked(bank_id).then(|| {
        let unsent = c.edit.status(bank_id).filter(|s| s.changes.known).map(|s| s.changes.total());
        super::bank_links::unlink_ask(unsent, c.edit.with_link(bank_id, |l| l.can_send()).unwrap_or(false), c.edit.sending_from(bank_id))
    });
    // (critic 71) this bank's own results, each with Dismiss, and (critic 67 I3) the wait note after them
    let notes: Vec<(Option<u64>, String)> = c.edit.results_of(bank_id).into_iter().map(|(id, t)| (Some(id), t)).chain(c.edit.wait_note().map(|w| (None, w))).collect();
    draw_strip(ui, &view, &mut |b| pressed = Some(b), &mut |m| menu = Some(m), Some((asked, notes)));
    let needs_account = matches!(pressed, Some(StripButton::Send | StripButton::GetLatest | StripButton::History | StripButton::TreatInStep | StripButton::TryAgain)) || matches!(menu, Some(Menu::History | Menu::CheckNow));
    if needs_account && !account.is_open() {
        account.open_quiet();
    }
    // The screen is drawn here in place of the questions only when it opened (critic 67 I3: another bank's screen
    // under way is never taken over by this page).
    let hosted_now = |c: &mut super::circles::CirclesModel, opened: bool| {
        if opened {
            c.edit.host = Some(EditHost { local_id: bank_id.to_string(), back: back.clone() });
            c.edit.host_frame = Some(frame);
        }
    };
    match pressed {
        None => {}
        Some(StripButton::Send) => {
            let opened = c.edit.open_send(bank_id, perm);
            hosted_now(c, opened);
        }
        Some(StripButton::GetLatest) => {
            let opened = c.edit.open_merge(bank_id, perm);
            hosted_now(c, opened);
        }
        Some(StripButton::TreatInStep) => {
            let opened = c.edit.open_merge_to_rebase(bank_id, perm);
            hosted_now(c, opened);
        }
        Some(StripButton::History) => {
            let opened = c.edit.open_history(&cid, &bid, &title, perm);
            hosted_now(c, opened);
        }
        Some(StripButton::OpenAccount) => {
            // (critic 63 I2) once signed in again, the next frame checks again
            c.links.check_now(bank_id);
            account.reveal();
        }
        Some(StripButton::TryAgain) => c.links.check_now(bank_id),
        Some(StripButton::Unlink) => c.edit.ask_forget(bank_id),
        Some(StripButton::AddFullCopy) => super::shared_ui::open_circle(&ctx, &cid, Some(&bid)),
    }
    match menu {
        None => {}
        Some(Menu::History) => {
            let opened = c.edit.open_history(&cid, &bid, &title, perm);
            hosted_now(c, opened);
        }
        Some(Menu::CirclePage) => super::shared_ui::open_circle(&ctx, &cid, Some(&bid)),
        Some(Menu::CheckNow) => c.links.check_now(bank_id),
        Some(Menu::Unlink) => c.edit.ask_forget(bank_id),
        Some(Menu::ConfirmUnlink) => c.edit.confirm_forget(bank_id),
        Some(Menu::KeepLink) => c.edit.cancel_forget(),
        Some(Menu::Dismiss(id)) => c.edit.dismiss_result(id),
    }
    out
}

/// The strip's button: the first is the big one only when it is a real next step (never History or Unlink).
fn strip_button(ui: &mut Ui, i: usize, b: StripButton) -> egui::Response {
    if i == 0 && !matches!(b, StripButton::History | StripButton::Unlink) { kit::primary_button(ui, b.label()) } else { kit::ghost_button(ui, b.label()) }
}

/// Under the strip's line: the "Unlink?" question when it was asked, and the notes (a result with the id its Dismiss
/// gives back, or the wait note with none).
pub(super) type StripMore = (Option<String>, Vec<(Option<u64>, String)>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Menu {
    History,
    CirclePage,
    CheckNow,
    Unlink,
    ConfirmUnlink,
    KeepLink,
    /// "Dismiss" on one of the bank's results.
    Dismiss(u64),
}

/// The strip: the pill and the sentence, then the buttons (primary first) and "..." (History, Circle page, Check
/// now, Unlink...). `more`: the "Unlink?" question when it was asked, and the notes of the circles model (this bank's
/// results, each with the id its Dismiss gives back, then the wait note); `None` for the unreadable-file strip, which
/// has no menu.
pub(super) fn draw_strip(ui: &mut Ui, v: &StripView, press: &mut dyn FnMut(StripButton), menu: &mut dyn FnMut(Menu), more: Option<StripMore>) {
    let fill = if v.warn { theme::error_wash() } else { theme::accent_faded() };
    egui::Frame::new().fill(fill).corner_radius(8).inner_margin(Margin::symmetric(12, 10)).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        super::circles_ui::text_row(ui, |ui| {
            super::shared_ui::step_pill(ui, "Circle", theme::accent_col(), theme::on_accent_col());
            if v.busy {
                ui.add(egui::Spinner::new().size(14.0));
            }
            ui.add(egui::Label::new(theme::text(v.line.as_str()).size(13.0)).wrap());
        });
        let Some((asked, notes)) = more else {
            if !v.buttons.is_empty() {
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    for (i, b) in v.buttons.iter().enumerate() {
                        if strip_button(ui, i, *b).clicked() {
                            press(*b);
                        }
                    }
                });
            }
            return;
        };
        for (id, n) in notes {
            match id {
                Some(id) => {
                    if result_line(ui, &n) {
                        menu(Menu::Dismiss(id));
                    }
                }
                None => super::circles_ui::line(ui, n),
            }
        }
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            for (i, b) in v.buttons.iter().enumerate() {
                if strip_button(ui, i, *b).clicked() {
                    press(*b);
                }
            }
            let r = kit::ghost_button(ui, "...").on_hover_text("More: history, the circle's page, check now, unlink");
            egui::Popup::menu(&r).show(|ui| {
                ui.set_min_width(170.0);
                if v.menu_history && !v.buttons.contains(&StripButton::History) && ui.button("History of the bank").clicked() {
                    menu(Menu::History);
                    ui.close();
                }
                if v.menu_circle_page && ui.button("Circle page").clicked() {
                    menu(Menu::CirclePage);
                    ui.close();
                }
                if !v.busy && ui.button("Check now").clicked() {
                    menu(Menu::CheckNow);
                    ui.close();
                }
                if !v.buttons.contains(&StripButton::Unlink) && ui.button("Unlink...").clicked() {
                    menu(Menu::Unlink);
                    ui.close();
                }
            });
        });
        if let Some(q) = asked {
            ui.add_space(4.0);
            ui.add(egui::Label::new(theme::text(q).size(12.5)).wrap());
            ui.horizontal_wrapped(|ui| {
                if kit::danger_text_button(ui, "Yes, unlink").clicked() {
                    menu(Menu::ConfirmUnlink);
                }
                if kit::ghost_button(ui, "Keep it").clicked() {
                    menu(Menu::KeepLink);
                }
            });
        }
    });
    ui.add_space(10.0);
}
