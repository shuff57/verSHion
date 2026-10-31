//! What the auth flows ask the mailer to send (port of `MailKind` / `MailMessage` in auth/src/types.ts).
//! Handlers never send: they push messages onto an outbox that the router hands to the mailer after the reply is
//! built, so a slow or failing mail server can neither delay nor change an answer (the Worker used `waitUntil`).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MailKind {
    /// Confirm a newly registered address.
    Verify,
    /// Reset a password.
    Reset,
    /// Someone signed up with an address that already has an account.
    SignupExisting,
    PasswordChanged,
    DeviceEnrol,
    ApplicationApproved,
    ApplicationRejected,
    ApprovalRevoked,
    AccountSuspended,
    AccountRestored,
    /// Internal, never rendered: an account just became active; `to` is its id. The router turns it into the admins' notice.
    NewApplication,
    /// Internal, never rendered: an action against an admin account. `reason` is the action, `to` the target id, `actor` the
    /// acting admin, `token` the proposal id and `expires_at` the time. The router mails the admins and the escalation contact.
    AdminAlert,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MailMessage {
    pub kind: MailKind,
    pub to: String,
    /// The one-time token or code, for the kinds that carry one.
    pub token: Option<String>,
    pub expires_at: Option<i64>,
    pub account_verified: Option<bool>,
    pub reason: Option<String>,
    pub actor: Option<String>,
}

impl MailMessage {
    /// An admin alert; see `MailKind::AdminAlert`.
    pub fn alert(action: &str, actor: &str, target: &str, proposal: Option<&str>, at: i64) -> Self {
        let mut m = Self::new(MailKind::AdminAlert, target);
        m.reason = Some(action.to_string());
        m.actor = Some(actor.to_string());
        m.token = proposal.map(str::to_string);
        m.expires_at = Some(at);
        m
    }

    pub fn new(kind: MailKind, to: &str) -> Self {
        Self { kind, to: to.to_string(), token: None, expires_at: None, account_verified: None, reason: None, actor: None }
    }
    pub fn with_token(mut self, token: &str, expires_at: i64) -> Self {
        self.token = Some(token.to_string());
        self.expires_at = Some(expires_at);
        self
    }
}

/// Messages queued by a handler, sent after the response.
pub type Outbox = Vec<MailMessage>;

/// Sends one message. Errors are reduced to a class by the implementation: a mail provider's error can echo the
/// address or token, so it is never logged.
pub trait Mailer: Send + Sync {
    fn send(&self, msg: &MailMessage) -> Result<(), String>;
}
