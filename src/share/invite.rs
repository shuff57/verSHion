//! The invite-code section of the Account window: type the code an admin gave you, press Redeem.
//! No egui: tests drive it with a scripted session.
//!
//! - Redeeming is `POST /me/invite`. It works for a pending account too, so the section is shown
//!   to pending and approved accounts alike.
//! - The code is typed into [`InviteModel::code`], sent once, and cleared the moment the pool
//!   accepts it. It is never written to disk and never printed (`Debug` is blank). A refused code
//!   stays in the box so a typo can be fixed, and goes when the Account window closes.
//! - A closed model (nothing pressed) does no I/O.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::Duration;

use web_time::Instant;

use super::api::ApiError;
use super::browse::Problem;
use super::model::Runner;
use super::pool::{InviteAck, Pool, PoolError, RefusalKind, RetryPolicy};
use super::publish::SharedSession;

#[derive(Clone, Debug, PartialEq)]
pub enum InviteState {
    /// Nothing known yet (an approved account from before invite codes is fine without one).
    Unknown,
    Redeeming,
    /// The pool holds a redemption for this account; `already` when it did before this press.
    Redeemed { already: bool },
    Failed(Problem),
}

pub struct InviteModel {
    /// What is typed in the box.
    pub code: String,
    pub state: InviteState,
    runner: Runner,
    retry: RetryPolicy,
    job: Option<Receiver<Result<InviteAck, PoolError>>>,
    now: Option<Instant>,
    repaint: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl std::fmt::Debug for InviteModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "InviteModel({:?})", self.state)
    }
}

impl Default for InviteModel {
    fn default() -> Self {
        InviteModel::new(Runner::Background)
    }
}

impl InviteModel {
    pub fn new(runner: Runner) -> InviteModel {
        InviteModel { code: String::new(), state: InviteState::Unknown, runner, retry: RetryPolicy::default(), job: None, now: None, repaint: None }
    }

    pub fn with_retry(mut self, retry: RetryPolicy) -> InviteModel {
        self.retry = retry;
        self
    }

    pub fn set_repaint(&mut self, f: Arc<dyn Fn() + Send + Sync>) {
        self.repaint = Some(f);
    }

    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    pub fn redeemed(&self) -> bool {
        matches!(self.state, InviteState::Redeemed { .. })
    }

    /// The code can be sent: something is typed, nothing is running, and a refusal's wait is over.
    pub fn can_redeem(&self) -> bool {
        !self.busy() && !self.redeemed() && !self.code.trim().is_empty() && self.wait().is_none()
    }

    /// How long until the pool will take another try (after a 429).
    pub fn wait(&self) -> Option<Duration> {
        let InviteState::Failed(p) = &self.state else { return None };
        let (at, now) = (p.retry_at?, self.now?);
        (at > now).then(|| at - now)
    }

    /// Forgets the box and the result (the Account window closed, or the person signed out).
    pub fn reset(&mut self) {
        self.job = None;
        self.code.clear();
        self.state = InviteState::Unknown;
    }

    /// The pool says an invite code is needed (from any pool call): mark it so the section can say so.
    pub fn note_required(&mut self) {
        if !self.redeemed() && !self.busy() {
            self.state = InviteState::Failed(Problem { message: super::pool::INVITE_REQUIRED_TEXT.into(), retry: false, retry_at: None, account: false });
        }
    }

    pub fn redeem(&mut self, session: Option<SharedSession>) {
        if !self.can_redeem() {
            return;
        }
        let Some(session) = session else {
            self.state = InviteState::Failed(Problem { message: "Sign in first; the code is tied to your account.".into(), retry: false, retry_at: None, account: false });
            return;
        };
        let (tx, rx) = channel();
        let (code, retry, repaint) = (self.code.trim().to_string(), self.retry, self.repaint.clone());
        async fn run(session: SharedSession, retry: RetryPolicy, code: String, tx: std::sync::mpsc::Sender<Result<InviteAck, PoolError>>, repaint: Option<Arc<dyn Fn() + Send + Sync>>) {
            let pool = Pool::new(&*session).with_retry(retry);
            let r = pool.redeem_invite(&code).await;
            let _ = tx.send(r);
            if let Some(f) = repaint {
                f();
            }
        }
        match self.runner {
            #[cfg(not(target_arch = "wasm32"))]
            Runner::Inline => super::transport::block_on(run(session, retry, code, tx, repaint)),
            #[cfg(target_arch = "wasm32")]
            Runner::Inline => wasm_bindgen_futures::spawn_local(run(session, retry, code, tx, repaint)),
            #[cfg(not(target_arch = "wasm32"))]
            Runner::Background => {
                std::thread::spawn(move || super::transport::block_on(run(session, retry, code, tx, repaint)));
            }
            #[cfg(target_arch = "wasm32")]
            Runner::Background => wasm_bindgen_futures::spawn_local(run(session, retry, code, tx, repaint)),
        }
        self.job = Some(rx);
        self.state = InviteState::Redeeming;
    }

    /// Applies a finished request. Does nothing while no request is running.
    pub fn frame(&mut self, now: Instant) {
        self.now = Some(now);
        let Some(rx) = &self.job else { return };
        let r = match rx.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err(PoolError::Api(ApiError::Offline("the request stopped".into()))),
        };
        self.job = None;
        match r {
            Ok(ack) => {
                self.code.clear();
                self.state = InviteState::Redeemed { already: ack.already };
            }
            Err(e) => self.state = InviteState::Failed(problem_of(&e, now)),
        }
    }
}

/// An error in words, with what can be done about it.
pub fn problem_of(e: &PoolError, now: Instant) -> Problem {
    let mut p = Problem { message: e.message(), retry: false, retry_at: None, account: false };
    match e {
        PoolError::Api(ApiError::SignedOut) => p.message = "Your session ended. Sign in again, then enter the code.".into(),
        PoolError::Api(_) => p.retry = true,
        PoolError::Local(_) => p.message = "That does not look like an invite code. It has 24 letters and digits, usually shown in groups of four.".into(),
        PoolError::Refused(r) => {
            if matches!(r.kind, RefusalKind::DailyLimit { .. } | RefusalKind::RateLimited | RefusalKind::Busy) {
                p.retry = true;
                p.retry_at = r.detail.retry_after.filter(|s| *s > 0).map(|s| now + Duration::from_secs(s));
            }
        }
    }
    p
}
