//! "Share to a circle": the second destination of the publish window. A bank (or an exam) goes into
//! one of the person's circles as a new bank, where its members can look at it and copy it. No egui.
//!
//! Rules this module keeps:
//!
//! - Nothing is sent until the person chose a circle and a licence, ticked the own-work statement
//!   and pressed the button. The answer keys go with the questions: the screen says so.
//! - The same `commit_id` is kept while nothing the server reads has changed, so a retry after a lost
//!   answer is the same send (the pool answers the committed one again with `replayed: true`).
//! - Server text is kept raw and drawn through `api::plain`.
//! - A circle send has no preview step: the pool's checks (share-core, the licence rules) answer
//!   with the same refusals as a public publish, shown here with what to change.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::Duration;

use web_time::Instant;

use super::api::{self, ApiError};
use super::browse::Problem;
use super::circles_api::{CircleSend, CircleSummary, Role, SentBank};
use super::pool::{ItemMeta, License, LicenseRefusal, PayloadRefusal, Pool, PoolError, PublishProgress, RefusalKind, RetryPolicy, sha256_hex};
use super::publish::{ShareSource, SharedMedia, SharedSession};

/// Where the list of the person's circles stands.
#[derive(Debug, Default)]
pub enum CircleList {
    #[default]
    Idle,
    Loading,
    Ready(Vec<CircleSummary>),
    Failed(Problem),
}

/// What the pool refused that the person can fix.
#[derive(Debug)]
pub enum Refused {
    Payload(Box<PayloadRefusal>),
    License(Box<LicenseRefusal>),
}

#[derive(Debug, Default)]
pub enum SendStage {
    #[default]
    Editing,
    Sending(Option<PublishProgress>),
    Done(Box<SentInfo>),
    Failed(Problem),
    Refused(Refused),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SentInfo {
    pub sent: SentBank,
    pub circle_name: String,
    pub circle_id: String,
    pub title: String,
}

enum Msg {
    List(Result<Vec<CircleSummary>, PoolError>),
    Progress(PublishProgress),
    Sent(Result<SentBank, PoolError>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    List,
    Send,
}

struct Job {
    kind: Kind,
    rx: Receiver<Msg>,
}

#[derive(Default)]
pub struct CircleSendModel {
    /// The destination is a circle (otherwise the public pool).
    pub active: bool,
    pub list: CircleList,
    pub chosen: Option<String>,
    pub license: Option<License>,
    pub message: String,
    pub attest: bool,
    pub stage: SendStage,
    /// The sign-in ended during a call: the publish window goes back to its sign-in step.
    pub signed_out: bool,
    /// The pool answered `invite_required`.
    pub invite_required: bool,
    preselect: Option<String>,
    commit: Option<(String, String)>,
    sent_title: Option<String>,
    job: Option<Job>,
    now: Option<Instant>,
}

/// The fixed words of the circle destination.
pub const KEYS_NOTE: &str = "Everyone in this circle will see the answer keys.";
pub const NEW_BANK_NOTE: &str = "This adds a new bank to the circle. Sending a changed version of the same bank comes in a later update of verSHion.";
/// "All rights reserved" for a circle bank (the public list's line is about unlisted items).
pub const CIRCLE_ONLY_LINE: &str = "Members can open it and copy it inside the circle, but may not share it publicly.";
pub const RESHARE_NOTE: &str = "Not yet: sharing a circle bank's questions publicly is not available in this version, whatever the licence. A later version may let members do it for Creative Commons banks. \"All rights reserved\" always keeps them inside the circle.";

impl CircleSendModel {
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// The destination is switched to a circle. `preselect` is a circle id asked for by the circle page.
    pub fn activate(&mut self, preselect: Option<String>) {
        self.active = true;
        if preselect.is_some() {
            self.preselect = preselect;
        }
        self.stage = SendStage::Editing;
    }

    pub fn deactivate(&mut self) {
        if self.job.is_none() {
            self.active = false;
            self.stage = SendStage::Editing;
        }
    }

    /// Forgets everything for a new source.
    pub fn reset(&mut self) {
        *self = CircleSendModel::default();
    }

    pub fn take_signed_out(&mut self) -> bool {
        std::mem::take(&mut self.signed_out)
    }

    /// Circles the person may add a bank to: open, and owner or editor.
    pub fn choices(&self) -> Vec<&CircleSummary> {
        match &self.list {
            CircleList::Ready(l) => l.iter().filter(|c| !c.closed() && c.role().can_add_banks()).collect(),
            _ => Vec::new(),
        }
    }

    pub fn chosen_circle(&self) -> Option<&CircleSummary> {
        let id = self.chosen.as_ref()?;
        self.choices().into_iter().find(|c| c.id == *id)
    }

    pub fn choose(&mut self, id: &str) {
        if self.choices().iter().any(|c| c.id == id) {
            self.chosen = Some(id.to_string());
        }
    }

    /// Applies finished jobs. Called every frame by the publish window while it is open.
    pub fn frame(&mut self, now: Instant, session: Option<&SharedSession>, runner: super::model::Runner, retry: RetryPolicy, media: &SharedMedia, repaint: Option<Arc<dyn Fn() + Send + Sync>>) {
        self.now = Some(now);
        self.drain();
        if self.active
            && matches!(self.list, CircleList::Idle)
            && let Some(s) = session
        {
            self.list = CircleList::Loading;
            self.spawn(Kind::List, s.clone(), runner, retry, media.clone(), repaint, Work::List);
        }
    }

    fn drain(&mut self) {
        let mut msgs = Vec::new();
        let mut lost = false;
        if let Some(j) = &self.job {
            loop {
                match j.rx.try_recv() {
                    Ok(m) => msgs.push(m),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        lost = true;
                        break;
                    }
                }
            }
        }
        for m in msgs {
            match m {
                Msg::Progress(p) => self.stage = SendStage::Sending(Some(p)),
                Msg::List(r) => {
                    self.job = None;
                    lost = false;
                    self.on_list(r);
                }
                Msg::Sent(r) => {
                    self.job = None;
                    lost = false;
                    self.on_sent(r);
                }
            }
        }
        if lost && let Some(j) = self.job.take() {
            let p = Problem { message: "The request stopped unexpectedly.".into(), retry: true, retry_at: None, account: false };
            match j.kind {
                Kind::List => self.list = CircleList::Failed(p),
                Kind::Send => self.stage = SendStage::Failed(p),
            }
        }
    }

    fn problem(&mut self, e: &PoolError) -> Option<Problem> {
        let now = self.now.unwrap_or_else(Instant::now);
        let mut p = Problem { message: e.message(), retry: false, retry_at: None, account: false };
        match e {
            PoolError::Api(ApiError::SignedOut) => {
                self.signed_out = true;
                return None;
            }
            PoolError::Api(_) => p.retry = true,
            PoolError::Local(_) => {}
            PoolError::Refused(r) => {
                let wait = r.detail.retry_after.filter(|s| *s > 0).map(|s| now + Duration::from_secs(s));
                match &r.kind {
                    RefusalKind::InviteRequired => {
                        self.invite_required = true;
                        p.account = true;
                    }
                    RefusalKind::NotApproved { .. } => p.account = true,
                    RefusalKind::DailyLimit { .. } | RefusalKind::RateLimited | RefusalKind::Busy | RefusalKind::PublishInProgress | RefusalKind::TooManyUploads | RefusalKind::TooManyOpenReads => {
                        p.retry = true;
                        p.retry_at = wait;
                    }
                    RefusalKind::NotFound => p.message = "That circle is not available to you any more. It may have been closed, or you may have left it. Choose another.".into(),
                    RefusalKind::CircleClosed { .. } => p.message = "That circle is closed. Choose another.".into(),
                    RefusalKind::Conflict { .. } => {
                        p.message = "Something changed at the same moment. Try again.".into();
                        p.retry = true;
                    }
                    RefusalKind::BlobMissing { .. } | RefusalKind::HashMismatch => p.retry = true,
                    _ => {}
                }
            }
        }
        Some(p)
    }

    fn on_list(&mut self, r: Result<Vec<CircleSummary>, PoolError>) {
        match r {
            Ok(l) => {
                self.list = CircleList::Ready(l);
                if let Some(p) = self.preselect.take() {
                    self.choose(&p);
                }
                if self.chosen.is_none() {
                    let only: Vec<String> = self.choices().iter().map(|c| c.id.clone()).collect();
                    if only.len() == 1 {
                        self.chosen = Some(only[0].clone());
                    }
                }
            }
            Err(e) => {
                if let Some(p) = self.problem(&e) {
                    self.list = CircleList::Failed(p);
                } else {
                    self.list = CircleList::Idle;
                }
            }
        }
    }

    fn on_sent(&mut self, r: Result<SentBank, PoolError>) {
        match r {
            Ok(sent) => {
                let (circle_name, circle_id) = self.chosen_circle().map(|c| (api::plain(&c.name), c.id.clone())).unwrap_or_default();
                let title = self.sent_title.take().unwrap_or_default();
                self.commit = None;
                self.stage = SendStage::Done(Box::new(SentInfo { sent, circle_name, circle_id, title }));
            }
            Err(e) => match e.kind() {
                Some(RefusalKind::InvalidPayload(p)) | Some(RefusalKind::LabelsNeedDecision(p)) | Some(RefusalKind::OrderAckRequired(p)) => {
                    self.commit = None;
                    self.stage = SendStage::Refused(Refused::Payload(p.clone()));
                }
                Some(RefusalKind::LicenseIncompatible(l)) => {
                    self.commit = None;
                    self.stage = SendStage::Refused(Refused::License(l.clone()));
                }
                Some(RefusalKind::AttestationRequired) => {
                    self.attest = false;
                    self.stage = SendStage::Failed(Problem { message: e.message(), retry: false, retry_at: None, account: false });
                }
                Some(RefusalKind::CommitIdConflict) => {
                    self.commit = None;
                    self.stage = SendStage::Failed(Problem { message: e.message(), retry: true, retry_at: None, account: false });
                }
                _ => match self.problem(&e) {
                    Some(p) => self.stage = SendStage::Failed(p),
                    None => self.stage = SendStage::Editing,
                },
            },
        }
    }

    /// Why the button can not be pressed, or `None`.
    pub fn blocker(&self) -> Option<&'static str> {
        if self.busy() {
            return Some("Wait for the current step to finish.");
        }
        if self.chosen_circle().is_none() {
            return Some("Choose a circle.");
        }
        if self.license.is_none() {
            return Some("Choose a licence. Nothing is chosen for you.");
        }
        if !self.attest {
            return Some("Tick the box that says this is your own work.");
        }
        None
    }

    pub fn back_to_editing(&mut self) {
        if self.job.is_none() {
            self.stage = SendStage::Editing;
        }
    }

    /// Takes one of the licences the pool said every question allows.
    pub fn choose_allowed(&mut self, l: License) {
        if let SendStage::Refused(Refused::License(r)) = &self.stage
            && r.allowed.iter().any(|a| a == l.as_str())
        {
            self.license = Some(l);
            self.stage = SendStage::Editing;
        }
    }

    /// Sends the bank. `meta` is the listing (checked by the caller).
    #[allow(clippy::too_many_arguments)]
    pub fn send(&mut self, source: &ShareSource, meta: ItemMeta, session: &SharedSession, runner: super::model::Runner, retry: RetryPolicy, media: &SharedMedia, repaint: Option<Arc<dyn Fn() + Send + Sync>>) {
        if self.blocker().is_some() {
            return;
        }
        let (Some(cid), Some(license)) = (self.chosen.clone(), self.license) else { return };
        let payload = source.payload(1);
        let mut req = CircleSend::new(payload, license, meta.clone());
        req.message = self.message.trim().to_string();
        // The same commit id while nothing the server reads changed.
        let print = format!("{cid}|{}|{}|{:?}|{}", license.as_str(), req.message, meta, sha256_hex(&req.payload));
        match &self.commit {
            Some((p, id)) if *p == print => req.commit_id = id.clone(),
            _ => self.commit = Some((print, req.commit_id.clone())),
        }
        self.sent_title = Some(meta.title.clone());
        self.stage = SendStage::Sending(None);
        self.spawn(Kind::Send, session.clone(), runner, retry, media.clone(), repaint, Work::Send(cid, Box::new(req)));
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn(&mut self, kind: Kind, session: SharedSession, runner: super::model::Runner, retry: RetryPolicy, media: SharedMedia, repaint: Option<Arc<dyn Fn() + Send + Sync>>, work: Work) {
        let (tx, rx) = channel::<Msg>();
        match runner {
            #[cfg(not(target_arch = "wasm32"))]
            super::model::Runner::Inline => run(session, media, retry, work, tx, repaint),
            #[cfg(target_arch = "wasm32")]
            super::model::Runner::Inline => wasm_bindgen_futures::spawn_local(async move { run_async(session, media, retry, work, tx, repaint).await }),
            #[cfg(not(target_arch = "wasm32"))]
            super::model::Runner::Background => {
                std::thread::spawn(move || run(session, media, retry, work, tx, repaint));
            }
            #[cfg(target_arch = "wasm32")]
            super::model::Runner::Background => wasm_bindgen_futures::spawn_local(async move { run_async(session, media, retry, work, tx, repaint).await }),
        }
        self.job = Some(Job { kind, rx });
    }

    /// "Try again" after a list failure.
    pub fn retry_list(&mut self) {
        if matches!(self.list, CircleList::Failed(_)) {
            self.list = CircleList::Idle;
        }
    }

    /// Roles of the circles shown, for the pill.
    pub fn role_of(&self, id: &str) -> Role {
        self.choices().into_iter().find(|c| c.id == id).map(|c| c.role()).unwrap_or(Role::Viewer)
    }
}

enum Work {
    List,
    Send(String, Box<CircleSend>),
}

async fn run_async(session: SharedSession, media: SharedMedia, retry: RetryPolicy, work: Work, tx: std::sync::mpsc::Sender<Msg>, repaint: Option<Arc<dyn Fn() + Send + Sync>>) {
    let pool = Pool::new(&*session).with_retry(retry);
    let msg = match work {
        Work::List => Msg::List(pool.circles().await),
        Work::Send(cid, req) => {
            let (ptx, prepaint) = (tx.clone(), repaint.clone());
            let mut progress = move |p: PublishProgress| {
                let _ = ptx.send(Msg::Progress(p));
                if let Some(r) = &prepaint {
                    r();
                }
            };
            Msg::Sent(pool.send_new_circle_bank(&cid, &req, &*media, &mut progress).await)
        }
    };
    let _ = tx.send(msg);
    if let Some(r) = repaint {
        r();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn run(session: SharedSession, media: SharedMedia, retry: RetryPolicy, work: Work, tx: std::sync::mpsc::Sender<Msg>, repaint: Option<Arc<dyn Fn() + Send + Sync>>) {
    super::transport::block_on(run_async(session, media, retry, work, tx, repaint));
}
