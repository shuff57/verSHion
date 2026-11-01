//! The Account window (egui). All logic lives in [`AccountModel`]; this only draws it.
//!
//! Every string that came from the server (name, school, email, reasons) is drawn as a plain
//! label after [`super::api::plain`], never as markup or a link.

use std::sync::Arc;

use egui::{Margin, RichText, Ui};
use web_time::Instant;

use super::api::{self, Status};
use super::model::{AccountModel, MailKind, Notice, Page};
use crate::{kit, theme};

/// The App's handle on the Account window. Closed by default; does nothing until opened.
#[derive(Default)]
pub struct AccountUi {
    pub model: AccountModel,
    repaint_set: bool,
    /// Open but not drawn: the publish flow is only looking the sign-in up.
    quiet: bool,
    /// The "Invite code" section of the status page.
    pub invite: super::invite::InviteModel,
    /// The app shows the account as a section of the Shared page, not as a floating window: the
    /// window is never drawn, and asking for it (`open`, `reveal`) asks for that page instead.
    pub page_mode: bool,
    /// The Shared page is on screen this frame: a quiet lookup must not close the account.
    pinned: bool,
    wants_page: bool,
}

/// Field ids whose egui edit state is dropped once the field is empty, so no copy of a password
/// stays in egui's memory.
const SECRET_FIELDS: [&str; 3] = ["share_pw", "share_pw2", "share_paste"];

/// The width the account forms were made for (the window's), kept when they sit in a wider card.
const FORM_WIDTH: f32 = 460.0;

impl AccountUi {
    /// The account as the app uses it: a section of the Shared page.
    pub fn in_page() -> AccountUi {
        AccountUi { page_mode: true, ..AccountUi::default() }
    }

    /// The Shared page is drawn: keep the account logic open and let the section be drawn.
    pub fn pin(&mut self) {
        self.pinned = true;
        if !self.model.open {
            self.model.open();
        }
        self.quiet = false;
    }

    /// Somebody asked to see the account (a Settings button, a window's "Open account..."): the app
    /// then goes to the Shared page. Reads and clears the request.
    pub fn take_wants_page(&mut self) -> bool {
        std::mem::take(&mut self.wants_page)
    }

    pub fn open(&mut self) {
        self.quiet = false;
        self.wants_page = self.page_mode;
        self.model.open();
    }

    pub fn is_open(&self) -> bool {
        self.model.open
    }

    /// Opens the account logic without drawing the window, so the publish flow can find out whether
    /// someone is signed in (one request for a saved session, none otherwise). No-op when open.
    pub fn open_quiet(&mut self) {
        if !self.model.open {
            self.model.open();
            self.quiet = true;
        }
    }

    /// Shows the window if it was only open in the background (or opens it).
    pub fn reveal(&mut self) {
        self.quiet = false;
        self.wants_page = self.page_mode;
        self.model.open();
    }

    /// Closes a background-only window; a window the person can see is left alone.
    pub fn close_quiet(&mut self) {
        if self.quiet && !self.pinned {
            self.quiet = false;
            self.model.close();
        }
    }

    /// The window is on screen.
    pub fn visible(&self) -> bool {
        self.model.open && !self.quiet && !self.page_mode
    }

    pub fn open_link(&mut self, link: api::Link) {
        self.model.open_link(link);
    }

    /// Draws the window when open; returns at once (no I/O at all) when closed.
    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.model.open {
            return;
        }
        if !self.repaint_set {
            let c = ctx.clone();
            self.model.set_repaint(Arc::new({
                let c = c.clone();
                move || c.request_repaint()
            }));
            self.invite.set_repaint(Arc::new(move || c.request_repaint()));
            self.repaint_set = true;
        }
        let now = Instant::now();
        self.model.frame(now);
        self.invite.frame(now);
        if self.invite.busy() || self.invite.wait().is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
        if let Some(at) = self.model.poller.next_at() {
            ctx.request_repaint_after(at.saturating_duration_since(now));
        }
        self.pinned = false;
        if self.quiet || self.page_mode {
            return;
        }
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::new("share_account"))
            .frame(kit::card().inner_margin(Margin::same(22)))
            .show(ctx, |ui| {
                ui.set_width(460.0_f32.min(ctx.content_rect().width() - 60.0).max(260.0));
                let tall = ctx.content_rect().height() - 120.0; // grows past last frame's size: see publish_ui
                egui::ScrollArea::vertical().max_height(tall).min_scrolled_height(tall).show(ui, |ui| {
                    close = self.page(ui);
                });
            });
        if close || modal.should_close() {
            self.quiet = false;
            self.model.close();
            self.invite.reset();
        }
        for id in SECRET_FIELDS {
            let empty = match id {
                "share_pw" => self.model.forms.password.is_empty(),
                "share_pw2" => self.model.forms.password2.is_empty(),
                _ => self.model.forms.paste.is_empty(),
            };
            if empty {
                ctx.data_mut(|d| d.remove::<egui::text_edit::TextEditState>(egui::Id::new(id)));
            }
        }
    }

    /// The forms as a section of the Shared page: the same pages as the window, without its title
    /// row and Close button (the page has its own heading). The window's `show` still handles the
    /// model's frame.
    pub fn draw_section(&mut self, ui: &mut Ui) {
        self.section(ui, true);
    }

    /// The section on the Shared page's Settings tab, which offers "Delete my account" itself
    /// (with what deleting does), so the status page's own link is left out.
    pub fn draw_settings_section(&mut self, ui: &mut Ui) {
        self.section(ui, false);
    }

    fn section(&mut self, ui: &mut Ui, delete_link: bool) {
        ui.scope(|ui| {
            // The forms keep the width they were made for in the window; the status page (account
            // details and the status box) fills the card like the rest of the Settings tab.
            if self.model.page != Page::Status {
                ui.set_max_width(ui.available_width().min(FORM_WIDTH));
            }
            self.page_body(ui, false, delete_link);
        });
    }

    /// The invite-code box on its own: the guided step "Enter your invite code", whose card
    /// already has the heading and the sentence about the code.
    pub fn draw_invite(&mut self, ui: &mut Ui) {
        let session: Option<super::publish::SharedSession> = self.model.client().map(|c| c.clone() as super::publish::SharedSession);
        let idle = !self.model.busy();
        invite_box(ui, &mut self.invite, session, idle, true);
    }

    /// The heading of the page the model is on ("Sign in", "Check your email"...).
    pub fn page_title(&self) -> &'static str {
        title(self.model.page)
    }

    /// Returns true when the window should close.
    fn page(&mut self, ui: &mut Ui) -> bool {
        self.page_body(ui, true, true)
    }

    fn page_body(&mut self, ui: &mut Ui, chrome: bool, delete_link: bool) -> bool {
        let m = &mut self.model;
        let mut close = false;
        if chrome {
            ui.horizontal(|ui| {
                ui.label(theme::serif(title(m.page)).size(24.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if kit::ghost_button(ui, "Close").clicked() {
                        close = true;
                    }
                    if m.busy() {
                        ui.spinner();
                    }
                });
            });
        } else if m.busy() {
            ui.spinner();
        }
        ui.add_space(6.0);
        let credentials = takes_password(m.page);
        if credentials && !m.base_url().is_empty() {
            server_header(ui, m);
        }
        notice(ui, m.notice.as_ref());
        if m.me.is_some() && matches!(m.page, Page::Confirm | Page::Reset | Page::Enrol) {
            let who = m.me.as_ref().map(|me| me.email.clone()).unwrap_or_default();
            para(
                ui,
                &format!(
                    "You are signed in as {who}. The email link does not say which address it was sent to; if it \
                     was for another address, using it changes that account only and leaves this session alone."
                ),
            );
        }
        // Nothing that takes a password works until the server shown above is confirmed.
        if credentials && m.custom_server() {
            dev_banner(ui);
        }
        ui.add_enabled_ui(!credentials || m.trusted, |ui| match m.page {
            Page::NotSetUp => para(ui, crate::share::NOT_SET_UP),
            Page::Opening => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(theme::muted("Opening your account…"));
                });
            }
            Page::NotConfigured | Page::Server => server_page(ui, m),
            Page::SignIn => sign_in_page(ui, m),
            Page::SignUp => sign_up_page(ui, m),
            Page::CheckEmail(kind) => check_email_page(ui, m, kind),
            Page::Confirm => confirm_page(ui, m),
            Page::Forgot => forgot_page(ui, m),
            Page::Reset => reset_page(ui, m),
            Page::Enrol => enrol_page(ui, m),
            Page::Status => status_page(ui, m, !chrome, delete_link),
            Page::Delete => delete_page(ui, m),
        });
        if m.page == Page::Status && m.me.as_ref().is_some_and(|me| matches!(me.status, Status::Pending | Status::Approved)) {
            let session: Option<super::publish::SharedSession> = m.client().map(|c| c.clone() as super::publish::SharedSession);
            invite_section(ui, &mut self.invite, session, !m.busy());
        }
        if credentials && !m.trusted && !m.base_url().is_empty() {
            trust_modal(ui.ctx(), m);
        }
        if !matches!(m.page, Page::NotConfigured | Page::Server | Page::NotSetUp) && !credentials && !m.base_url().is_empty() {
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(theme::muted(format!("Server: {}", api::server_label(m.base_url()))).size(12.0));
            });
        }
        if !m.server_locked()
            && !m.busy()
            && m.me.is_none()
            && !matches!(m.page, Page::NotConfigured | Page::Server)
            && link(ui, "Use a different server")
        {
            m.edit_server();
        }
        close
    }
}

/// Pages whose form sends a password.
fn takes_password(page: Page) -> bool {
    matches!(page, Page::SignIn | Page::SignUp | Page::Confirm | Page::Reset | Page::Enrol | Page::Delete)
}

/// Development builds only: typed server addresses are not something a release can do.
fn dev_banner(ui: &mut Ui) {
    egui::Frame::new().fill(theme::error_wash()).stroke(egui::Stroke::new(2.0, theme::bad())).corner_radius(10).inner_margin(Margin::same(10)).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new("DEVELOPMENT BUILD: custom server").family(theme::bold_family()).size(15.0).color(theme::bad()));
        ui.add(egui::Label::new(RichText::new("Release builds talk only to the server they were built for.").size(12.0).color(theme::bad())).wrap());
    });
    ui.add_space(8.0);
}

/// The server a password on this page goes to, large enough to read before typing.
fn server_header(ui: &mut Ui, m: &AccountModel) {
    egui::Frame::new().fill(theme::accent_faded()).corner_radius(10).inner_margin(Margin::same(12)).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        kit::small_caps(ui, "Your password goes to");
        ui.add(
            egui::Label::new(
                RichText::new(api::server_label(m.base_url())).size(17.0).family(egui::FontFamily::Monospace).color(theme::ink()),
            )
            .wrap(),
        );
    });
    ui.add_space(8.0);
}

/// "Send your password to <server>?", once per address, before any password field works.
fn trust_modal(ctx: &egui::Context, m: &mut AccountModel) {
    let (mut yes, mut change, mut no) = (false, false, false);
    egui::Modal::new(egui::Id::new("share_trust_server"))
        .frame(kit::card().inner_margin(Margin::same(22)))
        .show(ctx, |ui| {
            ui.set_width(420.0_f32.min(ctx.content_rect().width() - 80.0).max(240.0));
            // Only development builds ever ask (a built-in server needs no confirmation).
            dev_banner(ui);
            ui.label(theme::serif("Send your password to this server?").size(20.0));
            ui.add_space(8.0);
            ui.add(
                egui::Label::new(
                    RichText::new(api::server_label(m.base_url())).size(17.0).family(egui::FontFamily::Monospace).color(theme::ink()),
                )
                .wrap(),
            );
            ui.add_space(8.0);
            para(
                ui,
                "Only continue if this is your school's verSHion pool server. Your email and password will be sent to \
                 it. verSHion asks again whenever the address changes.",
            );
            ui.horizontal_wrapped(|ui| {
                yes = kit::primary_button(ui, "Yes, it is my school's server").clicked();
                if !m.server_locked() {
                    change = kit::ghost_button(ui, "Change server").clicked();
                }
                no = kit::ghost_button(ui, "Not now").clicked();
            });
        });
    if yes {
        m.trust_server();
    } else if change {
        m.edit_server();
    } else if no {
        m.close();
    }
}

fn title(page: Page) -> &'static str {
    match page {
        Page::NotConfigured | Page::Server | Page::NotSetUp | Page::Opening => "Sharing account",
        Page::SignIn => "Sign in",
        Page::SignUp => "Create an account",
        Page::CheckEmail(_) => "Check your email",
        Page::Confirm => "Finish your account",
        Page::Forgot => "Reset your password",
        Page::Reset => "Choose a new password",
        Page::Enrol => "Add this device",
        Page::Status => "Your account",
        Page::Delete => "Delete your account?",
    }
}

fn notice(ui: &mut Ui, n: Option<&Notice>) {
    let Some(n) = n else { return };
    let (text, fill, col) = match n {
        Notice::Info(t) => (t, theme::accent_faded(), theme::text_col()),
        Notice::Error(t) => (t, theme::error_wash(), theme::bad()),
    };
    egui::Frame::new().fill(fill).corner_radius(10).inner_margin(Margin::same(10)).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.add(egui::Label::new(RichText::new(text.as_str()).color(col).size(13.0)).wrap());
    });
    ui.add_space(8.0);
}

fn para(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(theme::muted(text).size(12.5)).wrap());
    ui.add_space(6.0);
}

fn link(ui: &mut Ui, text: &str) -> bool {
    ui.add(egui::Button::new(theme::accent(text).size(12.5)).frame(false)).clicked()
}

/// A labelled one-line field; true when Enter was pressed in it.
fn field(ui: &mut Ui, label: &str, value: &mut String, id: &str, secret: bool, hint: &str) -> bool {
    ui.label(theme::strong(label).size(12.5));
    let r = ui.add(
        egui::TextEdit::singleline(value)
            .id(egui::Id::new(id))
            .password(secret)
            .hint_text(hint)
            .desired_width(ui.available_width().min(FORM_WIDTH)),
    );
    ui.add_space(6.0);
    r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
}

fn profile_fields(ui: &mut Ui, m: &mut AccountModel) {
    field(ui, "Display name", &mut m.forms.display_name, "share_name", false, "How other teachers see you");
    field(ui, "School", &mut m.forms.school, "share_school", false, "School name and town");
    ui.checkbox(&mut m.forms.adult, "I am an adult employee of a school");
    ui.add_space(6.0);
}

fn password_pair(ui: &mut Ui, m: &mut AccountModel, label: &str) {
    field(ui, label, &mut m.forms.password, "share_pw", true, "");
    field(ui, "Repeat the password", &mut m.forms.password2, "share_pw2", true, "");
    para(ui, "At least 10 characters. Avoid common words, runs like 1234, and your email address.");
}

fn actions(ui: &mut Ui, busy: bool, add: impl FnOnce(&mut Ui)) {
    ui.add_space(4.0);
    ui.add_enabled_ui(!busy, |ui| {
        ui.horizontal_wrapped(|ui| {
            // A button that does not fit moves to the next row whole instead of wrapping its label.
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            add(ui)
        })
    });
}

fn server_page(ui: &mut Ui, m: &mut AccountModel) {
    para(
        ui,
        "Sharing questions with other teachers needs an account on a verSHion pool server. It is optional: \
         everything else in verSHion works without an account, on this device.",
    );
    let enter = field(ui, "Server address", &mut m.forms.server, "share_server", false, "https://pool.example.org");
    let mut save = enter;
    actions(ui, m.busy(), |ui| {
        save |= kit::primary_button(ui, "Save").clicked();
    });
    if m.page == Page::Server && !m.base_url().is_empty() && link(ui, "Cancel") {
        m.go(Page::SignIn);
    }
    if save {
        m.save_server();
    }
}

fn keychain_banner(ui: &mut Ui, m: &AccountModel) {
    let note = m.keychain_note.or_else(|| m.client().and_then(|c| c.persist_note()));
    if let Some(n) = note {
        para(ui, n);
    }
}

fn sign_in_page(ui: &mut Ui, m: &mut AccountModel) {
    keychain_banner(ui, m);
    let mut go = field(ui, "Email", &mut m.forms.email, "share_email", false, "you@school.org");
    go |= field(ui, "Password", &mut m.forms.password, "share_pw", true, "");
    actions(ui, m.busy(), |ui| {
        go |= kit::primary_button(ui, "Sign in").clicked();
    });
    if go {
        m.submit_sign_in();
    }
    if m.lockdown {
        ui.add_space(8.0);
        kit::card().inner_margin(Margin::same(12)).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            para(
                ui,
                "A device you have signed in on before still gets in during a pause. To get in on this one now, \
                 reset your password, or use the device link from a reset or confirmation email.",
            );
            ui.horizontal_wrapped(|ui| {
                if kit::ghost_button(ui, "Use a device code").clicked() {
                    m.go(Page::Enrol);
                }
                if kit::ghost_button(ui, "Reset password").clicked() {
                    m.go(Page::Forgot);
                }
            });
        });
    }
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        if link(ui, "Create an account") {
            m.go(Page::SignUp);
        }
        if link(ui, "Forgot your password?") {
            m.go(Page::Forgot);
        }
        if link(ui, "I have a link from an email") {
            m.go(Page::CheckEmail(MailKind::Verify));
        }
    });
}

fn sign_up_page(ui: &mut Ui, m: &mut AccountModel) {
    field(ui, "Email", &mut m.forms.email, "share_email", false, "Your school address");
    field(ui, "Password", &mut m.forms.password, "share_pw", true, "");
    para(ui, "At least 10 characters. Avoid common words, runs like 1234, and your email address.");
    profile_fields(ui, m);
    para(
        ui,
        "We email you a link. When you open it you set your password, name and school again: anyone can type \
         someone else's address into this form, so only the person who receives the email decides them.",
    );
    let mut go = false;
    actions(ui, m.busy(), |ui| {
        go = kit::primary_button(ui, "Create account").clicked();
    });
    if go {
        m.submit_sign_up();
    }
    if link(ui, "Back to sign in") {
        m.go(Page::SignIn);
    }
}

/// Masked like a password (the link is a credential): no clear text in screenshots, no copy out.
fn paste_box(ui: &mut Ui, m: &mut AccountModel) -> bool {
    let enter = field(ui, "Link or code from the email", &mut m.forms.paste, "share_paste", !m.forms.show_paste, "Paste it here");
    ui.checkbox(&mut m.forms.show_paste, "Show what I pasted");
    enter
}

fn check_email_page(ui: &mut Ui, m: &mut AccountModel, kind: MailKind) {
    para(ui, "Open the link from the email on this device, or copy the whole link (or just the long code in it) and paste it here.");
    let mut go = paste_box(ui, m);
    let mut resend = false;
    actions(ui, m.busy(), |ui| {
        go |= kit::primary_button(ui, "Continue").clicked();
        if kind == MailKind::Verify && !m.forms.email.trim().is_empty() {
            resend = kit::ghost_button(ui, "Send the link again").clicked();
        }
    });
    if go {
        m.submit_paste();
    }
    if resend {
        m.submit_resend();
    }
    if link(ui, "Back to sign in") {
        m.go(Page::SignIn);
    }
}

fn confirm_page(ui: &mut Ui, m: &mut AccountModel) {
    if !m.has_link_token() {
        return check_email_page(ui, m, MailKind::Verify);
    }
    para(
        ui,
        "Choose your password and enter your details. This replaces what was typed at sign-up, including the \
         password, so only the person who received this email sets them.",
    );
    password_pair(ui, m, "New password");
    profile_fields(ui, m);
    let mut go = false;
    actions(ui, m.busy(), |ui| {
        go = kit::primary_button(ui, "Confirm").clicked();
    });
    if go {
        m.submit_confirm();
    }
}

fn forgot_page(ui: &mut Ui, m: &mut AccountModel) {
    para(ui, "Enter your address. If it has an account, we email a link to choose a new password.");
    let mut go = field(ui, "Email", &mut m.forms.email, "share_email", false, "you@school.org");
    actions(ui, m.busy(), |ui| {
        go |= kit::primary_button(ui, "Send reset link").clicked();
    });
    if go {
        m.submit_forgot();
    }
    ui.horizontal_wrapped(|ui| {
        if link(ui, "I already have the link") {
            m.go(Page::CheckEmail(MailKind::Reset));
        }
        if link(ui, "Back to sign in") {
            m.go(Page::SignIn);
        }
    });
}

fn reset_page(ui: &mut Ui, m: &mut AccountModel) {
    if !m.has_link_token() {
        return check_email_page(ui, m, MailKind::Reset);
    }
    para(ui, "Setting a new password signs you out on every device.");
    password_pair(ui, m, "New password");
    if m.reset_needs_profile {
        profile_fields(ui, m);
    }
    let mut go = false;
    actions(ui, m.busy(), |ui| {
        go = kit::primary_button(ui, "Set password").clicked();
    });
    if go {
        m.submit_reset();
    }
}

fn enrol_page(ui: &mut Ui, m: &mut AccountModel) {
    para(
        ui,
        "After a password reset or a confirmation, we email a link for adding the app on your other devices \
         (it works once, for 15 minutes). With it, this device gets in even while sign-in is paused.",
    );
    if m.has_link_token() {
        para(ui, "The code from your link is ready.");
    } else {
        paste_box(ui, m);
    }
    field(ui, "Email", &mut m.forms.email, "share_email", false, "you@school.org");
    let mut go = field(ui, "Password", &mut m.forms.password, "share_pw", true, "");
    actions(ui, m.busy(), |ui| {
        go |= kit::primary_button(ui, "Add this device and sign in").clicked();
    });
    if go {
        m.submit_enrol();
    }
    if link(ui, "Back to sign in") {
        m.go(Page::SignIn);
    }
}

/// `in_page`: drawn on the Shared page, which has its own tabs, its own legal card and, while the
/// account waits, a step card saying so; none of those is repeated here. `delete_link`: false
/// where the page offers "Delete my account" itself.
fn status_page(ui: &mut Ui, m: &mut AccountModel, in_page: bool, delete_link: bool) {
    keychain_banner(ui, m);
    let Some(me) = m.me.clone() else {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(theme::muted("Checking your account…"));
        });
        if !m.busy() {
            actions(ui, m.busy(), |ui| {
                if kit::ghost_button(ui, "Try again").clicked() {
                    m.check_now();
                }
            });
        }
        return;
    };
    if !me.display_name.is_empty() {
        ui.label(theme::strong(me.display_name.as_str()).size(15.0));
    }
    ui.label(theme::text(me.email.as_str()).size(12.5));
    if !me.school.is_empty() {
        ui.label(theme::muted(me.school.as_str()).size(12.5));
    }
    ui.add_space(10.0);
    // On the page a pending account is under the "Waiting for approval" step, which says it all.
    if !(in_page && matches!(me.status, Status::Pending)) {
        status_box(ui, m, &me);
        ui.add_space(8.0);
    }
    actions(ui, m.busy(), |ui| {
        if kit::ghost_button(ui, "Check now").clicked() {
            m.check_now();
        }
        // The page's tabs are these two places.
        if !in_page && matches!(me.status, Status::Approved) && kit::primary_button(ui, "Browse shared questions").clicked() {
            super::browse_ui::request(ui.ctx(), super::browse_ui::BrowseRequest::Browse);
        }
        if !in_page && matches!(me.status, Status::Approved) && kit::ghost_button(ui, "My shared items").clicked() {
            super::mine_ui::request(ui.ctx(), super::mine_ui::MineRequest::List);
        }
    });
    if !in_page {
        legal_row(ui, &super::ShareSettings::legal_base());
    }
    ui.add_space(10.0);
    ui.separator();
    ui.checkbox(&mut m.forms.forget_device, "Forget this device too (on a shared computer)");
    let (mut out, mut everywhere) = (false, false);
    // Never disabled: signing out must work even while a request hangs.
    actions(ui, false, |ui| {
        out = kit::ghost_button(ui, "Sign out").clicked();
        everywhere = kit::ghost_button(ui, "Sign out everywhere").clicked();
    });
    if out {
        m.submit_sign_out(false);
    }
    if everywhere {
        m.submit_sign_out(true);
    }
    para(ui, "Signing out everywhere ends the sessions on all your devices and forgets them all.");
    if delete_link && kit::danger_text_button(ui, "Delete account…").clicked() {
        m.go(Page::Delete);
    }
}

/// The approval state with its explanation, the reviewer's reason and, when allowed, re-applying.
fn status_box(ui: &mut Ui, m: &mut AccountModel, me: &api::Me) {
    kit::card().inner_margin(Margin::same(14)).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        let (head, body) = status_text(&me.status);
        ui.label(theme::serif(head).size(17.0));
        ui.add(egui::Label::new(theme::muted(body).size(12.5)).wrap());
        match &me.status {
            Status::Rejected { reason, .. } | Status::Suspended { reason } => {
                if let Some(r) = reason {
                    ui.add_space(4.0);
                    ui.label(theme::strong("Reason given").size(12.0));
                    ui.add(egui::Label::new(theme::text(api::plain(r))).wrap());
                }
            }
            _ => {}
        }
        if let Status::Rejected { reapply_after_ms, can_reapply, .. } = &me.status {
            ui.add_space(6.0);
            if *can_reapply {
                para(ui, "You can apply again now with updated details.");
                if m.forms.display_name.is_empty() && m.forms.school.is_empty() {
                    m.forms.display_name = me.display_name.clone();
                    m.forms.school = me.school.clone();
                }
                profile_fields(ui, m);
                let mut go = false;
                actions(ui, m.busy(), |ui| {
                    go = kit::primary_button(ui, "Apply again").clicked();
                });
                if go {
                    m.submit_reapply();
                }
            } else if let Some(at) = reapply_after_ms {
                para(ui, &format!("You can apply again from {} (UTC).", api::date_text(*at)));
            }
        }
    });
}

/// The draft legal pages, as links when a server address is configured and as names when not.
fn legal_row(ui: &mut Ui, base: &str) {
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(theme::muted("Legal (drafts):").size(12.0));
        for (name, file) in super::mine_text::LEGAL_PAGES {
            match super::mine_text::legal_url(base, file) {
                Some(u) => {
                    if ui.add(egui::Button::new(theme::accent(name).size(12.0)).frame(false)).clicked() {
                        crate::platform::open_url(&u);
                    }
                }
                None => {
                    ui.label(theme::muted(name).size(12.0));
                }
            }
        }
    });
}

/// Type the code an admin gave you, press Redeem. Open to a pending account too.
pub(super) fn invite_section(ui: &mut Ui, inv: &mut super::invite::InviteModel, session: Option<super::publish::SharedSession>, idle: bool) {
    invite_box(ui, inv, session, idle, false);
}

/// The invite box. `standalone`: it is a whole guided step whose card already has the heading
/// and the invite-only sentence, so neither is repeated here.
fn invite_box(ui: &mut Ui, inv: &mut super::invite::InviteModel, session: Option<super::publish::SharedSession>, idle: bool, standalone: bool) {
    use super::invite::InviteState;
    if !standalone {
        ui.add_space(10.0);
        ui.separator();
        ui.label(theme::strong("Invite code").size(13.5));
    }
    if let InviteState::Redeemed { already } = &inv.state {
        ui.label(theme::text(if *already { "Redeemed (this account already had a code)." } else { "Redeemed. You can use the shared pool." }).size(12.5));
        return;
    }
    if !standalone {
        para(ui, "The shared pool is invite-only. Enter the code an admin gave you (24 letters and digits, usually in groups of four). It is sent once and not kept.");
    }
    let busy = inv.busy();
    ui.add_enabled_ui(!busy && idle, |ui| {
        let w = ui.available_width().min(FORM_WIDTH);
        ui.add(egui::TextEdit::singleline(&mut inv.code).id(egui::Id::new("share_invite")).hint_text("XXXX-XXXX-XXXX-XXXX-XXXX-XXXX").desired_width(w));
    });
    if let InviteState::Failed(p) = &inv.state {
        ui.add(egui::Label::new(RichText::new(api::plain(&p.message)).color(theme::bad()).size(12.5)).wrap());
        if let Some(w) = inv.wait() {
            para(ui, &format!("You can try again in about {} seconds.", w.as_secs().max(1)));
        }
    }
    ui.horizontal(|ui| {
        ui.add_enabled_ui(inv.can_redeem() && idle, |ui| {
            if kit::primary_button(ui, "Redeem").clicked() {
                inv.redeem(session);
            }
        });
        if busy {
            ui.spinner();
        }
    });
    if inv.code.is_empty() {
        ui.ctx().data_mut(|d| d.remove::<egui::text_edit::TextEditState>(egui::Id::new("share_invite")));
    }
}

fn status_text(s: &Status) -> (&'static str, &'static str) {
    match s {
        Status::Pending => (
            "Waiting for approval",
            "An administrator checks every account by hand before it can share; this keeps answer keys away \
             from students. This window checks again every minute while it is open.",
        ),
        Status::Approved => ("Approved", "You can share questions and use the shared pool."),
        Status::Rejected { revoked: false, .. } => ("Not approved", "Your application was not approved."),
        Status::Rejected { revoked: true, .. } => ("Approval withdrawn", "Your account's approval was withdrawn."),
        Status::Suspended { .. } => ("Suspended", "This account is suspended and cannot share."),
        Status::Unverified => ("Not finished", "Finish the account with the link in the confirmation email."),
        Status::Other(_) => ("Status unknown", "This version of verSHion does not know this account state."),
    }
}

fn delete_page(ui: &mut Ui, m: &mut AccountModel) {
    para(
        ui,
        "Your account stops working at once: your email, name, school and password are erased and every \
         device is signed out. Questions you shared are hidden at once and removed after 14 days. Your work \
         on this device is not touched.",
    );
    ui.checkbox(&mut m.forms.delete_ack, "Yes, delete my account");
    field(ui, "Password", &mut m.forms.password, "share_pw", true, "");
    let (mut go, mut back) = (false, false);
    actions(ui, m.busy(), |ui| {
        go = ui
            .add(
                egui::Button::new(theme::on_accent("Delete account").family(theme::bold_family()))
                    .fill(theme::red())
                    .corner_radius(255)
                    .min_size(egui::vec2(0.0, 34.0)),
            )
            .clicked();
        back = kit::ghost_button(ui, "Cancel").clicked();
    });
    if go {
        m.submit_delete();
    }
    if back {
        m.go(Page::Status);
    }
}
