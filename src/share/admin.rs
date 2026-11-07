//! The approval queue: the admin's screen for the accounts waiting to be let into the pool.
//!
//! No egui here; [`super::admin_ui`] draws it and `admin_tests` drives it against a scripted session.
//!
//! - Reading is `GET /admin/queue?status=open` (oldest first, with the queue's age and the labels the
//!   server generated: the domain's history, earlier rejections, whether the profile changed since the
//!   application). Every applicant field is the snapshot submitted with that application and is shown
//!   as plain text, never as markup.
//! - Deciding is `POST /admin/approve` or `POST /admin/reject`. They need an admin on a device that has
//!   signed in before, and no password (the owner's choice, 2026-11-07). If a server does answer
//!   `sudo_required`, the password box opens: the person's own password goes to `POST /auth/reauth` for a
//!   **sudo token** good for five minutes, sent once on the caller's thread and cleared; the token lives in
//!   memory only.
//! - Nothing is decided by accident: a decision is a form the admin fills in and presses. Approving
//!   asks which evidence was checked (the server records it), and a rejection's reason is shown to the
//!   applicant, so it is asked for separately from the reviewer's private note.
//! - One job at a time. A refusal keeps the decision form open so nothing typed is lost; a `409` means
//!   somebody else decided it first, and the row leaves the list.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, Sender, channel};
use std::time::Duration;

use serde_json::{Value, json};
use web_time::Instant;

use super::api::{self, ApiError, plain};
use super::browse::Problem;
use super::model::Runner;
use super::pool::{PoolError, PoolSession, RetryPolicy};
use super::publish::{AccountSnapshot, AccountTurn, AccountWatch, SharedSession};
use super::secret::Secret;
use super::transport::{Method, Request, Response};

/// The evidence the server accepts for an approval, and the wording the admin picks it by. The first
/// is the gate: every application carries a link to the applicant's staff page, and the reviewer opens
/// it before approving. The other two stay for applications made before that rule.
pub const EVIDENCE: [(&str, &str); 3] = [
    ("staff_page", "Their staff page lists them (open the link)"),
    ("directory", "The school's staff directory lists them"),
    ("school_confirmed", "A person at the school confirmed it"),
];

/// The line a decision sent without a live token leaves on screen (the password box then opens).
pub const CONFIRM_PROMPT: &str = "Confirm your password to make a change. It is used once and not kept.";

/// How long the server's sudo token is good for when it does not say (the Worker's `expires_in` is 300).
const SUDO_SECONDS: u64 = 300;

// ---- what the queue says ---------------------------------------------------------------------------

/// One application, as the server listed it. Applicant-typed text is already made plain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Application {
    pub request_id: String,
    pub user_id: String,
    pub email: String,
    pub email_domain: String,
    /// The name, school and staff-page link submitted with this application (a snapshot, not the live profile).
    pub display_name: String,
    pub school: String,
    /// The page the applicant gave as proof they work at the school; empty for applications made before
    /// 2026-11-07, when it became required.
    pub staff_page: String,
    pub age_seconds: i64,
    /// Earlier applications of this account that were rejected or revoked.
    pub prior_rejections: i64,
    /// A label the admin gave this email domain (it never grants trust by itself).
    pub domain_hint: Option<String>,
    pub domain_history: Option<(i64, i64)>,
    /// The live profile differs from the one submitted with this application.
    pub profile_changed: bool,
    pub suspended: bool,
    pub trust: String,
}

impl Application {
    /// What to call this person in a sentence (their name, or their address if none was given).
    pub fn who(&self) -> String {
        if self.display_name.is_empty() { self.email.clone() } else { self.display_name.clone() }
    }
}

/// `GET /admin/queue?status=open`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Queue {
    pub open: i64,
    pub oldest_seconds: i64,
    pub items: Vec<Application>,
}

/// Parses the server's answer. `None` when it is not a queue at all.
pub fn parse_queue(v: &Value) -> Option<Queue> {
    let items = v.get("items")?.as_array()?;
    let text = |x: Option<&Value>| x.and_then(Value::as_str).map(plain).unwrap_or_default();
    let num = |x: Option<&Value>| x.and_then(Value::as_i64).unwrap_or(0);
    let flag = |x: Option<&Value>| x.and_then(Value::as_bool).unwrap_or(false);
    let mut out = Vec::with_capacity(items.len());
    for it in items {
        let labels = it.get("server_labels");
        let applicant = it.get("applicant");
        let history = labels.and_then(|l| l.get("domain_history"));
        let request_id = text(it.get("request_id"));
        let user_id = text(it.get("user_id"));
        if request_id.is_empty() || user_id.is_empty() {
            continue;
        }
        let email = text(it.get("email"));
        let domain = text(it.get("email_domain"));
        out.push(Application {
            request_id,
            user_id,
            email: email.clone(),
            email_domain: if domain.is_empty() { email.rsplit('@').next().unwrap_or("").to_string() } else { domain },
            display_name: text(applicant.and_then(|a| a.get("display_name"))),
            school: text(applicant.and_then(|a| a.pointer("/profile/school"))),
            staff_page: text(applicant.and_then(|a| a.pointer("/profile/staff_page"))),
            age_seconds: num(it.get("age_seconds")),
            prior_rejections: num(labels.and_then(|l| l.get("prior_rejections"))),
            domain_hint: labels.and_then(|l| l.get("domain_hint")).and_then(Value::as_str).map(plain).filter(|h| !h.is_empty()),
            domain_history: history.map(|h| (num(h.get("approved")), num(h.get("rejected")))),
            profile_changed: flag(labels.and_then(|l| l.get("profile_changed"))),
            suspended: flag(labels.and_then(|l| l.get("suspended"))),
            trust: text(it.get("trust")),
        });
    }
    Some(Queue {
        open: v.get("open").and_then(Value::as_i64).unwrap_or(out.len() as i64),
        oldest_seconds: num(v.get("oldestAgeSeconds")),
        items: out,
    })
}

// ---- the growth report -----------------------------------------------------------------------------
//
// `GET /admin/growth?days=N`: per ad / link tag (`?src=` on the link, kept on the account at sign-up) the sign-ups in
// the window, how many are approved, and their purchases, gross revenue and refunds. Counts only: the server never
// names an account here. Visits come from Cloudflare Web Analytics when the server has it connected; Cloudflare does
// not see the `?src=` tag, so visits are listed by the site that linked here, apart from the per-tag table.

/// The windows the screen offers, in days.
pub const GROWTH_DAYS: [i64; 3] = [7, 30, 90];
/// The window the screen opens on.
pub const GROWTH_DEFAULT_DAYS: i64 = 30;
/// What the screen says when the server has no visit counts connected.
pub const VISITS_OFF: &str = "Visit counts are not connected yet.";
/// What it says when they are connected but could not be read just now.
pub const VISITS_UNAVAILABLE: &str = "Visit counts could not be read from Cloudflare just now. Read again in a few minutes.";

/// One row of the report (or the totals, whose `source` is always none).
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
pub struct SourceRow {
    #[serde(default)]
    pub source: Option<String>,
    pub signups: i64,
    pub approved: i64,
    pub purchases: i64,
    /// Gross: what was paid, before refunds and fees.
    pub revenue_cents: i64,
    pub refunds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
pub struct Referrer {
    /// The referring host; none for a direct visit (or one whose browser sent no referrer).
    #[serde(default)]
    pub referrer: Option<String>,
    pub visits: i64,
}

/// The report's `visits`: `available` with counts, or not, with a reason (`not_configured` / `unavailable`).
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
pub struct VisitsReport {
    pub available: bool,
    #[serde(default)]
    pub reason: Option<String>,
    /// The window the counts cover (at most 90 days, even in a longer report).
    #[serde(default)]
    pub days: Option<i64>,
    #[serde(default)]
    pub total: i64,
    #[serde(default)]
    pub by_referrer: Vec<Referrer>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
pub struct Growth {
    pub days: i64,
    pub sources: Vec<SourceRow>,
    pub totals: SourceRow,
    #[serde(default)]
    pub visits: VisitsReport,
}

/// Parses the server's answer, making every server-sent string plain. `None` when it is not a report.
pub fn parse_growth(v: &Value) -> Option<Growth> {
    let mut g: Growth = serde_json::from_value(v.clone()).ok()?;
    let clean = |s: &mut Option<String>| *s = s.as_deref().map(plain).filter(|t| !t.is_empty());
    g.sources.iter_mut().for_each(|r| clean(&mut r.source));
    g.totals.source = None;
    g.visits.by_referrer.iter_mut().for_each(|r| clean(&mut r.referrer));
    Some(g)
}

/// A tag as the table shows it.
pub fn source_label(source: &Option<String>) -> String {
    source.clone().unwrap_or_else(|| "(no tag)".into())
}

/// A referring host as the list shows it.
pub fn referrer_label(r: &Option<String>) -> String {
    r.clone().unwrap_or_else(|| "(direct or not sent)".into())
}

/// US cents as dollars: `$0.00`, `$12.50`, `$1,234.00`.
pub fn dollars(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let c = cents.unsigned_abs();
    let whole = (c / 100).to_string();
    let mut grouped = String::new();
    for (i, ch) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    format!("{sign}${grouped}.{:02}", c % 100)
}

/// One sentence over the table.
pub fn growth_summary(g: &Growth) -> String {
    let n = g.totals.signups;
    let who = if n == 1 { "1 sign-up".to_string() } else { format!("{n} sign-ups") };
    let tags = g.sources.iter().filter(|r| r.source.is_some()).count();
    let from = match tags {
        0 => String::new(),
        1 => ", from 1 tagged link".to_string(),
        t => format!(", from {t} tagged links"),
    };
    format!("{who} in the last {} days{from}.", g.days)
}

/// The line about visits when the counts are not shown.
pub fn visits_line(v: &VisitsReport) -> Option<&'static str> {
    if v.available {
        None
    } else if v.reason.as_deref() == Some("unavailable") {
        Some(VISITS_UNAVAILABLE)
    } else {
        Some(VISITS_OFF)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum GrowthState {
    Closed,
    Reading,
    Loaded(Growth),
    Failed(Problem),
}

/// Which of the admin tab's two screens is showing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    Queue,
    Growth,
}

// ---- the screen's state ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    /// Nothing read yet (no request).
    Closed,
    Reading,
    Loaded(Queue),
    Failed(Problem),
}

/// A decision being filled in, for one application.
#[derive(Clone, Debug, PartialEq)]
pub struct Decision {
    pub request_id: String,
    pub user_id: String,
    /// The person's name, for the form's heading.
    pub who: String,
    pub approve: bool,
    /// Which of [`EVIDENCE`] was checked (approve only).
    pub evidence: usize,
    /// Where the admin found it (sent with an approval; the server keeps it in the audit trail).
    pub evidence_note: String,
    /// The reviewer's note. Never shown to the applicant.
    pub note: String,
    /// Why the application was refused. Shown to the applicant, so it is a separate field.
    pub reason: String,
}

impl Decision {
    fn of(app: &Application, approve: bool) -> Decision {
        Decision {
            request_id: app.request_id.clone(),
            user_id: app.user_id.clone(),
            who: app.who(),
            approve,
            // An application with a staff page defaults to checking that page; the older evidence
            // choices are still there for an application made before the link was required.
            evidence: if app.staff_page.is_empty() { 1 } else { 0 },
            evidence_note: String::new(),
            note: String::new(),
            reason: String::new(),
        }
    }

    /// What is missing before it can be sent, in words.
    pub fn problem(&self) -> Option<&'static str> {
        if !self.approve && self.reason.trim().is_empty() {
            Some("Write why, in the words the applicant will read. A refusal without a reason leaves them guessing.")
        } else if self.approve && self.evidence >= EVIDENCE.len() {
            Some("Choose what you checked.")
        } else if self.approve && self.evidence_note.trim().is_empty() {
            Some("Say where you checked it: the staff directory page, or who confirmed it. The audit trail keeps it.")
        } else if self.note.trim().is_empty() {
            // The server refuses a decision without a private note ("Check the form."), so it is asked for here.
            Some("Add a private note for the audit trail. It is never shown to the applicant.")
        } else {
            None
        }
    }

    /// The JSON body the server documents for this decision. The private note never reaches the applicant.
    pub fn body(&self) -> Value {
        if self.approve {
            json!({
                "user_id": self.user_id,
                "request_id": self.request_id,
                "evidence": EVIDENCE[self.evidence.min(EVIDENCE.len() - 1)].0,
                "evidence_note": self.evidence_note.trim(),
                "note": self.note.trim(),
            })
        } else {
            json!({
                "user_id": self.user_id,
                "request_id": self.request_id,
                "reason": self.reason.trim(),
                "note": self.note.trim(),
            })
        }
    }
}

enum Done {
    Queue(Result<Queue, PoolError>),
    Sudo(Result<Secret, PoolError>),
    Decided { request_id: String, approve: bool, result: Result<(), PoolError> },
}

/// The admin tab. One request at a time; a closed model does no I/O.
pub struct AdminModel {
    pub state: State,
    /// The password typed to confirm a change (wiped as soon as the token comes back).
    pub password: String,
    /// The password box is open.
    pub confirming: bool,
    /// The decision being filled in.
    pub decision: Option<Decision>,
    /// A line under the list: what just happened, or what a refusal said.
    pub notice: Option<String>,
    /// The sudo token and when it stops working.
    sudo: Option<(Secret, Instant)>,
    /// A decision was waiting on the password: send it as soon as a token arrives.
    send_after_confirm: bool,
    /// The session a waiting decision belongs to.
    pending_session: Option<SharedSession>,
    watch: AccountWatch,
    runner: Runner,
    retry: RetryPolicy,
    job: Option<Receiver<Done>>,
    now: Option<Instant>,
    repaint: Option<Arc<dyn Fn() + Send + Sync>>,
    /// The queue or the growth report.
    pub view: View,
    pub growth: GrowthState,
    /// The window the report is read for (one of [`GROWTH_DAYS`]).
    pub growth_days: i64,
    /// A line under the report: a refused re-read (the last report stays on screen).
    pub growth_notice: Option<String>,
    /// The report's own request, apart from the queue's so neither waits on the other.
    growth_job: Option<Receiver<(i64, Result<Growth, PoolError>)>>,
}

impl std::fmt::Debug for AdminModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdminModel")
            .field("state", &self.state)
            .field("confirming", &self.confirming)
            .field("decision", &self.decision.as_ref().map(|d| (d.request_id.clone(), d.approve)))
            .finish_non_exhaustive()
    }
}

impl Default for AdminModel {
    fn default() -> Self {
        AdminModel::new(Runner::Background)
    }
}

impl AdminModel {
    pub fn new(runner: Runner) -> AdminModel {
        AdminModel {
            state: State::Closed,
            password: String::new(),
            confirming: false,
            decision: None,
            notice: None,
            sudo: None,
            send_after_confirm: false,
            pending_session: None,
            watch: AccountWatch::default(),
            runner,
            retry: RetryPolicy::default(),
            job: None,
            now: None,
            repaint: None,
            view: View::Queue,
            growth: GrowthState::Closed,
            growth_days: GROWTH_DEFAULT_DAYS,
            growth_notice: None,
            growth_job: None,
        }
    }

    pub fn with_retry(mut self, retry: RetryPolicy) -> AdminModel {
        self.retry = retry;
        self
    }

    pub fn set_repaint(&mut self, f: Arc<dyn Fn() + Send + Sync>) {
        self.repaint = Some(f);
    }

    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// Another account signed in (or this one signed out): nothing one admin read is shown to the next,
    /// and a typed password does not outlive the account it was typed for.
    pub fn note_account(&mut self, snap: &AccountSnapshot) {
        if self.watch.note(snap) != AccountTurn::Same {
            self.reset();
        }
    }

    /// Forgets everything: the list, the form, the password and the token.
    pub fn reset(&mut self) {
        self.job = None;
        self.state = State::Closed;
        self.password.clear();
        self.confirming = false;
        self.decision = None;
        self.notice = None;
        self.sudo = None;
        self.send_after_confirm = false;
        self.pending_session = None;
        self.growth = GrowthState::Closed;
        self.growth_notice = None;
        self.growth_job = None;
    }

    /// The growth report's request is running.
    pub fn growth_busy(&self) -> bool {
        self.growth_job.is_some()
    }

    /// The report was opened: read it, once, unless it is already read or being read.
    pub fn open_growth(&mut self, session: Option<SharedSession>) {
        self.view = View::Growth;
        if matches!(self.growth, GrowthState::Closed | GrowthState::Failed(_)) {
            self.load_growth(session);
        }
    }

    /// Another window: read the report for it (the table on screen is for another window, so it gives way).
    pub fn set_growth_days(&mut self, days: i64, session: Option<SharedSession>) {
        if days == self.growth_days || !GROWTH_DAYS.contains(&days) || self.growth_busy() {
            return;
        }
        self.growth_days = days;
        self.growth = GrowthState::Closed;
        self.load_growth(session);
    }

    /// (Re)reads the report for [`Self::growth_days`]. A report for the same window stays on screen until the answer.
    pub fn load_growth(&mut self, session: Option<SharedSession>) {
        if self.growth_busy() {
            return;
        }
        let Some(session) = session else {
            self.growth = GrowthState::Failed(Problem { message: "Sign in first. The growth report belongs to an admin account.".into(), retry: false, retry_at: None, account: true });
            return;
        };
        if !matches!(&self.growth, GrowthState::Loaded(g) if g.days == self.growth_days) {
            self.growth = GrowthState::Reading;
        }
        self.growth_notice = None;
        let (tx, rx) = channel();
        let (days, repaint) = (self.growth_days, self.repaint.clone());
        let account = session.account();
        async fn run(session: SharedSession, account: Option<String>, days: i64, tx: Sender<(i64, Result<Growth, PoolError>)>, repaint: Option<Arc<dyn Fn() + Send + Sync>>) {
            let r = read_growth(&*session, account.as_deref(), days).await;
            let _ = tx.send((days, r));
            if let Some(f) = repaint {
                f();
            }
        }
        self.growth_job = Some(rx);
        self.spawn(move || run(session, account, days, tx, repaint));
    }

    /// Applies a finished report request.
    fn growth_frame(&mut self, now: Instant) {
        let Some(rx) = &self.growth_job else { return };
        let (days, result) = match rx.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => (self.growth_days, Err(PoolError::Api(ApiError::Offline("the request stopped".into())))),
        };
        self.growth_job = None;
        if days != self.growth_days {
            return;
        }
        match result {
            Ok(g) => {
                self.growth_notice = None;
                self.growth = GrowthState::Loaded(g);
            }
            Err(e) => {
                let p = growth_problem(&e, now);
                match &self.growth {
                    GrowthState::Loaded(_) => self.growth_notice = Some(p.message),
                    _ => self.growth = GrowthState::Failed(p),
                }
            }
        }
    }

    /// The tab was opened: read the queue, once, unless it is already read or being read.
    pub fn open_queue(&mut self, session: Option<SharedSession>) {
        if matches!(self.state, State::Closed | State::Failed(_)) {
            self.load(session);
        }
    }

    /// (Re)reads the queue. The list on screen is replaced only by an answer, never cleared first.
    pub fn load(&mut self, session: Option<SharedSession>) {
        if self.busy() {
            return;
        }
        let Some(session) = session else {
            self.state = State::Failed(Problem { message: "Sign in first. The queue belongs to an admin account.".into(), retry: false, retry_at: None, account: true });
            return;
        };
        self.reading();
        let (tx, rx) = channel();
        let (retry, repaint) = (self.retry, self.repaint.clone());
        let account = session.account();
        async fn run(session: SharedSession, account: Option<String>, retry: RetryPolicy, tx: Sender<Done>, repaint: Option<Arc<dyn Fn() + Send + Sync>>) {
            let r = read_queue(&*session, account.as_deref(), retry).await;
            let _ = tx.send(Done::Queue(r));
            if let Some(f) = repaint {
                f();
            }
        }
        self.spawn(move || run(session, account, retry, tx, repaint));
        self.job = Some(rx);
    }

    fn reading(&mut self) {
        if !matches!(self.state, State::Loaded(_)) {
            self.state = State::Reading;
        }
        self.notice = None;
    }

    /// A decision is allowed right now (a live token, nothing running).
    pub fn sudo_ready(&self) -> bool {
        match (&self.sudo, self.now) {
            (Some((_, until)), Some(now)) => *until > now,
            (Some(_), None) => true,
            _ => false,
        }
    }

    /// Capture hook only (`VERSHION_SCREEN=shared:admin...`): shows `queue` as if it had just been read, with no
    /// request. `step` "approve" / "refuse" opens the form for the first application; "confirm" is an approval
    /// sent without a live token, waiting for the password (what [`Self::decide`] leaves on screen).
    pub fn qa_show(&mut self, queue: Queue, step: &str) {
        self.reset();
        let first = queue.items.first().cloned();
        self.state = State::Loaded(queue);
        let Some(app) = first else { return };
        match step {
            "approve" | "confirm" => {
                let mut d = Decision::of(&app, true);
                if step == "confirm" {
                    d.evidence_note = "Listed on the staff page of the school's website, science department".into();
                    self.confirming = true;
                    self.notice = Some(CONFIRM_PROMPT.into());
                }
                self.decision = Some(d);
            }
            "refuse" => self.decision = Some(Decision::of(&app, false)),
            _ => {}
        }
    }

    /// Capture hook only (`VERSHION_SCREEN=shared:admin:growth`): shows `g` as if it had just been read, with no request.
    pub fn qa_show_growth(&mut self, g: Growth) {
        self.reset();
        self.view = View::Growth;
        self.growth_days = g.days;
        self.growth = GrowthState::Loaded(g);
    }

    /// Opens the form for one application.
    pub fn open_decision(&mut self, app: &Application, approve: bool) {
        if self.busy() {
            return;
        }
        self.notice = None;
        self.decision = Some(Decision::of(app, approve));
    }

    pub fn cancel_decision(&mut self) {
        self.decision = None;
        self.send_after_confirm = false;
        self.notice = None;
    }

    /// Sends the form. No password is asked for first (see the body); only a server that refuses with
    /// `sudo_required` opens the password box, and the decision then goes by itself once it is confirmed.
    pub fn decide(&mut self, session: Option<SharedSession>) {
        if self.busy() {
            return;
        }
        let Some(d) = self.decision.clone() else { return };
        if let Some(why) = d.problem() {
            self.notice = Some(why.into());
            return;
        }
        let Some(session) = session else {
            self.notice = Some("You are signed out. Sign in again to decide.".into());
            return;
        };
        // Approving and refusing go straight out: the server wants an admin on a known device, not a fresh password.
        // A live sudo token still goes along. A server that does answer `sudo_required` opens the password box
        // (the session is kept for the retry that follows the confirmation).
        self.pending_session = Some(session.clone());
        let token = if self.sudo_ready() { self.sudo.clone().map(|(t, _)| t) } else { None };
        self.sending(&d, session, token);
    }

    /// `POST /auth/reauth`: the password becomes a five-minute token. Then a waiting decision goes.
    pub fn confirm_password(&mut self, session: Option<SharedSession>) {
        if self.busy() {
            return;
        }
        if self.password.is_empty() {
            self.notice = Some("Type your password first.".into());
            return;
        }
        let Some(session) = session else {
            self.notice = Some("You are signed out. Sign in again to confirm.".into());
            return;
        };
        let (tx, rx) = channel();
        let (retry, repaint) = (self.retry, self.repaint.clone());
        let account = session.account();
        let password = Secret::take(&mut self.password);
        async fn run(session: SharedSession, account: Option<String>, password: Secret, retry: RetryPolicy, tx: Sender<Done>, repaint: Option<Arc<dyn Fn() + Send + Sync>>) {
            let r = reauth(&*session, account.as_deref(), password, retry).await;
            let _ = tx.send(Done::Sudo(r));
            if let Some(f) = repaint {
                f();
            }
        }
        self.notice = Some("Confirming...".into());
        self.confirming = false;
        self.spawn(move || run(session, account, password, retry, tx, repaint));
        self.job = Some(rx);
    }

    fn sending(&mut self, d: &Decision, session: SharedSession, token: Option<Secret>) {
        let (tx, rx) = channel();
        let (retry, repaint) = (self.retry, self.repaint.clone());
        let account = session.account();
        let decision = d.clone();
        async fn run(session: SharedSession, account: Option<String>, decision: Decision, token: Option<Secret>, retry: RetryPolicy, tx: Sender<Done>, repaint: Option<Arc<dyn Fn() + Send + Sync>>) {
            let request_id = decision.request_id.clone();
            let approve = decision.approve;
            let r = send_decision(&*session, account.as_deref(), &decision, token, retry).await;
            let _ = tx.send(Done::Decided { request_id, approve, result: r });
            if let Some(f) = repaint {
                f();
            }
        }
        self.notice = Some(if d.approve { "Approving...".into() } else { "Refusing...".into() });
        self.send_after_confirm = false;
        self.spawn(move || run(session, account, decision, token, retry, tx, repaint));
        self.job = Some(rx);
    }

    /// Spawns one job. `Inline` runs it to completion (tests). The future is made inside the thread,
    /// because a browser future is not `Send` and the native transport's future need not be either.
    fn spawn<F, Fut>(&self, make: F)
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + 'static,
    {
        match self.runner {
            #[cfg(not(target_arch = "wasm32"))]
            Runner::Inline => super::transport::block_on(make()),
            #[cfg(target_arch = "wasm32")]
            Runner::Inline => wasm_bindgen_futures::spawn_local(make()),
            #[cfg(not(target_arch = "wasm32"))]
            Runner::Background => {
                std::thread::spawn(move || super::transport::block_on(make()));
            }
            #[cfg(target_arch = "wasm32")]
            Runner::Background => wasm_bindgen_futures::spawn_local(make()),
        }
    }

    /// Applies a finished request. Does nothing while no request is running.
    pub fn frame(&mut self, now: Instant) {
        self.now = Some(now);
        self.growth_frame(now);
        let Some(rx) = &self.job else { return };
        let done = match rx.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Done::Queue(Err(PoolError::Api(ApiError::Offline("the request stopped".into())))),
        };
        self.job = None;
        match done {
            Done::Queue(Ok(q)) => {
                self.notice = None;
                self.state = State::Loaded(q);
            }
            Done::Queue(Err(e)) => {
                let p = problem_of(&e, now);
                // A refused read keeps whatever was on screen (a failed refresh is not a lost list).
                match &mut self.state {
                    State::Loaded(_) => self.notice = Some(p.message.clone()),
                    _ => self.state = State::Failed(p),
                }
            }
            Done::Sudo(Ok(token)) => {
                self.sudo = Some((token, now + Duration::from_secs(SUDO_SECONDS)));
                self.confirming = false;
                self.notice = Some("Your password is confirmed. Changes go through for the next five minutes.".into());
                if self.send_after_confirm && self.decision.is_some() {
                    self.after_confirm_pending();
                } else {
                    self.send_after_confirm = false;
                }
            }
            Done::Sudo(Err(e)) => {
                let p = problem_of(&e, now);
                self.confirming = true;
                self.send_after_confirm = self.decision.is_some();
                self.notice = Some(p.message);
            }
            Done::Decided { request_id, approve, result } => match result {
                Ok(()) => {
                    let who = self.who_of(&request_id);
                    self.dropped(&request_id);
                    self.decision = None;
                    // The growth report counts approvals and sign-ups: mark it unread so it is read again the
                    // next time the Growth tab opens (a read still in flight is dropped, its answer is stale).
                    self.growth = GrowthState::Closed;
                    self.growth_job = None;
                    self.notice = Some(match (approve, who) {
                        (true, Some(w)) => format!("Approved {w}. They can use the pool now."),
                        (true, None) => "Approved. They can use the pool now.".into(),
                        (false, Some(w)) => format!("Refused {w}. They were given the reason you wrote."),
                        (false, None) => "Refused. They were given the reason you wrote.".into(),
                    });
                }
                Err(e) => {
                    let code = match &e {
                        PoolError::Api(ae) => ae.code().to_string(),
                        _ => String::new(),
                    };
                    let p = problem_of(&e, now);
                    match code.as_str() {
                        // The token lapsed or was for another session: ask again, keep the form.
                        "sudo_required" => {
                            self.sudo = None;
                            self.confirming = true;
                            self.send_after_confirm = self.decision.is_some();
                        }
                        // Somebody else decided it: the row leaves the list and the form closes.
                        "not_pending" => {
                            self.dropped(&request_id);
                            self.decision = None;
                        }
                        _ => {}
                    }
                    self.notice = Some(p.message);
                }
            },
        }
    }

    /// A decision made after the password was confirmed, for a form that was already filled in.
    fn after_confirm_pending(&mut self) {
        // The token is live now; the same call decides with it.
        let Some(session) = self.pending_session.take() else {
            self.send_after_confirm = false;
            return;
        };
        self.send_after_confirm = false;
        self.decide(Some(session));
    }

    fn dropped(&mut self, request_id: &str) {
        if let State::Loaded(q) = &mut self.state {
            let before = q.items.len();
            q.items.retain(|a| a.request_id != request_id);
            if q.items.len() != before {
                q.open = (q.open - 1).max(0);
            }
        }
    }

    fn who_of(&self, request_id: &str) -> Option<String> {
        match &self.state {
            State::Loaded(q) => q.items.iter().find(|a| a.request_id == request_id).map(Application::who),
            _ => None,
        }
    }
}

// ---- the requests ----------------------------------------------------------------------------------

fn request(base: &str, method: Method, path: &str, session: &dyn PoolSession, account: Option<&str>) -> Request {
    let mut r = Request { method, url: format!("{}{path}", base.trim_end_matches('/')), ..Request::default() };
    if let Some(d) = session.device_token(account) {
        r.headers.push(("X-Device-Token".into(), d));
    }
    r
}

fn refusal(r: &Response) -> PoolError {
    PoolError::Api(ApiError::from_response(r))
}

async fn read_queue(session: &dyn PoolSession, account: Option<&str>, _retry: RetryPolicy) -> Result<Queue, PoolError> {
    let base = session.base();
    let build = || request(&base, Method::Get, "/admin/queue?status=open", session, account);
    let resp = session.send_as(account, &build).await.map_err(PoolError::Api)?;
    if !resp.is_success() {
        return Err(refusal(&resp));
    }
    parse_queue(&resp.json()).ok_or_else(|| PoolError::Api(ApiError::BadReply("the approval queue".into())))
}

async fn read_growth(session: &dyn PoolSession, account: Option<&str>, days: i64) -> Result<Growth, PoolError> {
    let base = session.base();
    let path = format!("/admin/growth?days={days}");
    let build = || request(&base, Method::Get, &path, session, account);
    let resp = session.send_as(account, &build).await.map_err(PoolError::Api)?;
    if !resp.is_success() {
        return Err(refusal(&resp));
    }
    parse_growth(&resp.json()).ok_or_else(|| PoolError::Api(ApiError::BadReply("the growth report".into())))
}

async fn reauth(session: &dyn PoolSession, account: Option<&str>, password: Secret, _retry: RetryPolicy) -> Result<Secret, PoolError> {
    let base = session.base();
    let body = json!({ "password": password.expose() }).to_string();
    let build = || {
        let mut r = request(&base, Method::Post, "/auth/reauth", session, account);
        r.body = Some(Secret::new(body.clone()));
        r
    };
    let resp = session.send_as(account, &build).await.map_err(PoolError::Api)?;
    if !resp.is_success() {
        return Err(refusal(&resp));
    }
    let v = resp.json();
    match v.get("sudo_token").and_then(Value::as_str).filter(|t| !t.is_empty()) {
        Some(t) => Ok(Secret::new(t.to_string())),
        None => Err(PoolError::Api(ApiError::BadReply("no sudo token".into()))),
    }
}

async fn send_decision(session: &dyn PoolSession, account: Option<&str>, d: &Decision, token: Option<Secret>, _retry: RetryPolicy) -> Result<(), PoolError> {
    let base = session.base();
    let path = if d.approve { "/admin/approve" } else { "/admin/reject" };
    let body = d.body().to_string();
    let build = || {
        let mut r = request(&base, Method::Post, path, session, account);
        r.body = Some(Secret::new(body.clone()));
        if let Some(t) = &token {
            r.headers.push(("X-Sudo-Token".into(), t.clone()));
        }
        r
    };
    let resp = session.send_as(account, &build).await.map_err(PoolError::Api)?;
    if resp.is_success() { Ok(()) } else { Err(refusal(&resp)) }
}

// ---- words -----------------------------------------------------------------------------------------

/// "under an hour" / "3 hours" / "2 days 4 hours" for a queue's age.
pub fn age_text(seconds: i64) -> String {
    let s = seconds.max(0);
    let hours = s / 3600;
    if hours < 1 {
        return "under an hour".into();
    }
    let days = hours / 24;
    let rest = hours % 24;
    let plural = |n: i64, one: &str| format!("{n} {}{}", one, if n == 1 { "" } else { "s" });
    match (days, rest) {
        (0, _) => plural(hours, "hour"),
        (_, 0) => plural(days, "day"),
        _ => format!("{} {}", plural(days, "day"), plural(rest, "hour")),
    }
}

/// One sentence about the queue as a whole.
pub fn summary(q: &Queue) -> String {
    if q.open == 0 {
        return "Nothing is waiting. Every application has been decided.".into();
    }
    let n = if q.open == 1 { "1 application is waiting".to_string() } else { format!("{} applications are waiting", q.open) };
    format!("{n}. The oldest has waited {}.", age_text(q.oldest_seconds))
}

/// [`problem_of`], in the growth report's words.
pub fn growth_problem(e: &PoolError, now: Instant) -> Problem {
    let mut p = problem_of(e, now);
    match e {
        PoolError::Api(ApiError::SignedOut) => p.message = "You are signed out. Sign in again to read the growth report.".into(),
        PoolError::Api(ApiError::Http { code, .. }) if code == "forbidden" || code == "admin_only" => {
            p.message = "This account is not an admin, so it can not see the growth report.".into();
        }
        _ => {}
    }
    p
}

/// An error in words, with what can be done about it.
pub fn problem_of(e: &PoolError, now: Instant) -> Problem {
    let mut p = Problem { message: e.message(), retry: false, retry_at: None, account: false };
    match e {
        PoolError::Api(ApiError::SignedOut) => {
            p.message = "You are signed out. Sign in again to use the queue.".into();
            p.account = true;
        }
        PoolError::Api(ApiError::Http { code, retry_after, .. }) => match code.as_str() {
            "sudo_required" => p.message = "Confirm your password again, then the change goes through.".into(),
            "known_device_required" => {
                p.message = "This needs a device you have signed in on before. Sign in again on this computer. If you keep seeing this, sign out and back in.".into();
                p.account = true;
            }
            "forbidden" | "admin_only" => {
                p.message = "This account is not an admin, so it can not see the queue. An admin is made with the make-admin script.".into();
            }
            "not_pending" => p.message = "Someone else decided this application first. It has left the queue.".into(),
            "rate_limited" => {
                p.message = format!("Too many tries from this computer just now. Try again {}.", api::wait_text(*retry_after));
                p.retry = true;
                p.retry_at = retry_after.filter(|s| *s > 0).map(|s| api::later(now, s));
            }
            _ => p.retry = true,
        },
        PoolError::Api(_) => p.retry = true,
        PoolError::Local(_) => p.retry = false,
        PoolError::Refused(_) => p.retry = false,
    }
    p
}
