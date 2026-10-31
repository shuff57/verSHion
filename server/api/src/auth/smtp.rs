//! Mail: the settings, the text of each message, and the SMTP submission transport (port of worker/src/mail.ts and
//! worker/src/mail_smtp.ts; SMTP is the decided path, Resend is not ported). Port 465 is implicit TLS, any other port
//! does STARTTLS first, and AUTH uses a mailbox login. `MAIL_MODE=log` prints links for local development only.
use super::mail::{MailKind, MailMessage, Mailer};
use lettre::message::{Mailbox, header::ContentType};
use lettre::transport::smtp::authentication::{Credentials, Mechanism};
use lettre::{Message, SmtpTransport, Transport};
use std::time::Duration;

pub const FROM_NAME_DEFAULT: &str = "verSHion";
const TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tls {
    Implicit,
    StartTls,
    /// Tests only: a plain local socket.
    None,
}

#[derive(Clone, Debug, Default)]
pub struct MailEnv {
    pub log_mode: bool,
    /// The bare envelope and header sender address.
    pub from: String,
    pub from_name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub tls: Option<Tls>,
    /// Base URL of the app page that handles the links (it reads the token from the fragment).
    pub app_url: String,
    pub admin_emails: Vec<String>,
    pub escalation_emails: Vec<String>,
    pub smtp: bool,
}

fn list(v: Option<String>) -> Vec<String> {
    v.unwrap_or_default().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

/// The bare address from `MAIL_FROM` (which may be written "Name <addr>"; SMTP's MAIL FROM wants only the address), else
/// the mailbox that signs in.
pub fn smtp_from(mail_from: Option<&str>, username: &str) -> String {
    let configured = mail_from.map(str::trim).unwrap_or("");
    if configured.is_empty() {
        return username.trim().to_string();
    }
    if let Some(rest) = configured.strip_suffix('>') {
        if let Some(i) = rest.rfind('<') {
            let inner = rest[i + 1..].trim();
            if !inner.is_empty() {
                return inner.to_string();
            }
        }
    }
    configured.to_string()
}

pub fn smtp_port(v: Option<&str>) -> u16 {
    v.and_then(|s| s.trim().parse::<u32>().ok()).filter(|n| (1..65536).contains(n)).map(|n| n as u16).unwrap_or(465)
}

pub fn implicit_tls(secure: Option<&str>, port: u16) -> bool {
    match secure.map(|s| s.trim().to_lowercase()).as_deref() {
        Some("on" | "1" | "true" | "implicit") => true,
        Some("off" | "0" | "false" | "starttls") => false,
        _ => port == 465,
    }
}

/// A header value may never carry a bare CR or LF: that ends the header line and injects another.
pub fn header_value(v: &str) -> String {
    v.split(['\r', '\n']).filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" ").trim().to_string()
}

const LOCAL_HOSTS: [&str; 3] = ["localhost", "127.0.0.1", "[::1]"];

fn host_of(url: &str) -> String {
    let rest = url.split("://").nth(1).unwrap_or("");
    let auth = rest.split(['/', '?', '#']).next().unwrap_or("");
    let auth = auth.rsplit('@').next().unwrap_or("");
    match auth.strip_prefix('[') {
        Some(v6) => format!("[{}]", v6.split(']').next().unwrap_or("")),
        None => auth.split(':').next().unwrap_or("").to_lowercase(),
    }
}

impl MailEnv {
    /// Reads the settings through `get` (the process environment in production). Log mode refuses to run unless the app
    /// is plainly a local dev build, because it prints live tokens.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<MailEnv, String> {
        let non_empty = |k: &str| get(k).map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        let host = non_empty("SMTP_HOSTNAME").unwrap_or_default();
        let username = non_empty("SMTP_USERNAME").unwrap_or_default();
        let password = get("SMTP_PASSWORD").unwrap_or_default();
        let port = smtp_port(get("SMTP_PORT").as_deref());
        let smtp = !host.is_empty() && !username.is_empty() && !password.is_empty();
        let env = MailEnv {
            log_mode: get("MAIL_MODE").as_deref() == Some("log"),
            from: smtp_from(get("MAIL_FROM").as_deref(), &username),
            from_name: non_empty("SMTP_FROM_NAME").unwrap_or_else(|| FROM_NAME_DEFAULT.to_string()),
            host,
            port,
            username,
            // The Worker stripped whitespace: app passwords are often pasted in groups.
            password: password.chars().filter(|c| !c.is_whitespace()).collect(),
            tls: Some(if implicit_tls(get("SMTP_SECURE").as_deref(), port) { Tls::Implicit } else { Tls::StartTls }),
            app_url: get("APP_URL").unwrap_or_default(),
            admin_emails: list(get("ADMIN_EMAIL")),
            escalation_emails: list(get("ESCALATION_EMAIL")),
            smtp,
        };
        env.assert_safe()?;
        Ok(env)
    }

    pub fn assert_safe(&self) -> Result<(), String> {
        if self.log_mode && !LOCAL_HOSTS.contains(&host_of(&self.app_url).as_str()) {
            return Err("MAIL_MODE=log is only allowed when APP_URL is on localhost or 127.0.0.1".into());
        }
        Ok(())
    }

    fn link(&self, path: &str, token: &str) -> String {
        // Fragment, not query: it never reaches a server log or a Referer header.
        format!("{}{path}#token={token}", self.app_url)
    }
}

/// The subject and text of each message.
pub fn render(env: &MailEnv, m: &MailMessage) -> (String, String) {
    let token = m.token.as_deref().unwrap_or("");
    let reason = m.reason.as_deref().unwrap_or("");
    let (s, t): (&str, String) = match m.kind {
        MailKind::Verify => (
            "Confirm your verSHion address",
            format!("Confirm this address to finish signing up:\n\n{}\n\nThe link works once and expires in 24 hours. If you do not see this after a few minutes, look in your spam or junk folder. If you did not sign up, ignore this mail.", env.link("/account/verify", token)),
        ),
        MailKind::Reset => (
            "Reset your verSHion password",
            format!("Set a new password:\n\n{}\n\nThe link works once and expires in 1 hour. Using it signs you out everywhere. If you did not ask for this, ignore this mail.", env.link("/account/reset", token)),
        ),
        MailKind::SignupExisting => (
            "Someone tried to sign up with your address",
            if m.account_verified == Some(true) {
                "Someone tried to create a verSHion account with this address, which already has one. If it was you, sign in or reset your password. Nothing has changed.".into()
            } else {
                "Someone tried to create a verSHion account with this address. An account for it already exists but was never confirmed. If you did not create it, reset the password to take it over; that confirms the address as yours.".into()
            },
        ),
        MailKind::PasswordChanged => (
            "Your verSHion password was changed",
            "Your password was just reset and every session was signed out. If this was not you, reset it again now and contact the site owner.".into(),
        ),
        MailKind::ApplicationApproved => ("Your verSHion account was approved", "Your account has been approved. You can now publish to and browse the shared pool.".into()),
        MailKind::ApplicationRejected => (
            "Your verSHion application",
            format!("We could not approve your account.\n\n{reason}\n\nYou can update your details and apply again from the account screen after the waiting period."),
        ),
        MailKind::ApprovalRevoked => (
            "Your verSHion approval was withdrawn",
            format!("Your account's approval was withdrawn and you were signed out.\n\n{reason}\n\nYou can apply again from the account screen after the waiting period."),
        ),
        MailKind::AccountSuspended => (
            "Your verSHion account is suspended",
            format!("Your account has been suspended and signed out everywhere.\n\n{reason}\n\nReply to this mail to contact the site owner."),
        ),
        MailKind::AccountRestored => ("Your verSHion account is restored", "Your account's suspension has ended. You can sign in again.".into()),
        MailKind::DeviceEnrol => (
            "Add the verSHion app on your other devices",
            format!("To sign in to the verSHion app on another device (even while sign-ins are being blocked because of failed attempts), open this link on that device within 15 minutes and enter your password:\n\n{}\n\nThe link works once. If you did not just reset your password or confirm your address, ignore this mail.", env.link("/account/enrol", token)),
        ),
    };
    (s.to_string(), t)
}

/// Builds the message lettre will send. It folds, encodes and dot-stuffs; the values are still cleaned of CR/LF first.
pub fn build_message(env: &MailEnv, to: &str, subject: &str, text: &str) -> Result<Message, String> {
    let name = header_value(if env.from_name.is_empty() { FROM_NAME_DEFAULT } else { &env.from_name });
    let from = Mailbox::new(Some(name), header_value(&env.from).parse().map_err(|_| "mail: bad sender address".to_string())?);
    let to = Mailbox::new(None, header_value(to).replace(['<', '>'], "").parse().map_err(|_| "mail: bad recipient address".to_string())?);
    Message::builder().from(from).to(to).subject(header_value(subject)).header(ContentType::TEXT_PLAIN).body(text.replace("\r\n", "\n")).map_err(|_| "mail: could not build the message".to_string())
}

/// One plain-text mail to one address. Errors keep the server's class, never the address or the text.
pub fn send_raw(env: &MailEnv, to: &str, subject: &str, text: &str) -> Result<(), String> {
    if env.log_mode {
        env.assert_safe()?;
        tracing::info!("[dev mail] to={to} subject={subject}\n{text}");
        return Ok(());
    }
    if !env.smtp {
        return Err("mail not configured".into());
    }
    let msg = build_message(env, to, subject, text)?;
    let creds = Credentials::new(env.username.clone(), env.password.clone());
    let b = match env.tls.unwrap_or(Tls::Implicit) {
        Tls::Implicit => SmtpTransport::relay(&env.host),
        Tls::StartTls => SmtpTransport::starttls_relay(&env.host),
        Tls::None => Ok(SmtpTransport::builder_dangerous(&env.host)),
    }
    .map_err(|_| "SMTP: bad host".to_string())?;
    // AUTH LOGIN is what Stalwart and Gmail offer; PLAIN is the fallback.
    let transport = b.port(env.port).credentials(creds).authentication(vec![Mechanism::Login, Mechanism::Plain]).timeout(Some(TIMEOUT)).build();
    transport.send(&msg).map(|_| ()).map_err(|e| {
        let class = if e.is_permanent() { "refused" } else if e.is_transient() { "deferred" } else if e.is_timeout() { "timeout" } else { "failed" };
        format!("SMTP {class}")
    })
}

/// "admins": every admin; "everyone": every admin plus the escalation contact.
pub fn send_to(env: &MailEnv, everyone: bool, subject: &str, text: &str) -> Result<(), String> {
    let mut to = env.admin_emails.clone();
    if everyone {
        to.extend(env.escalation_emails.iter().cloned());
    }
    let mut first_err = None;
    for addr in to {
        if let Err(e) = send_raw(env, &addr, subject, text) {
            first_err.get_or_insert(e);
        }
    }
    first_err.map_or(Ok(()), Err)
}

pub struct SmtpMailer {
    pub env: MailEnv,
}

impl Mailer for SmtpMailer {
    fn send(&self, m: &MailMessage) -> Result<(), String> {
        let (subject, text) = render(&self.env, m);
        send_raw(&self.env, &m.to, &subject, &text)
    }
}

#[cfg(test)]
#[path = "smtp_tests.rs"]
mod tests;
