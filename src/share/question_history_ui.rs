//! Phase D2 (plan 4.5, 4.6, 4.8): one question's history in its circle, drawn on the bank's page in place of the
//! questions, as the bank's History is (`EditView::Question`, hosted by `bank_links_ui::strip`). It opens only from the
//! question card's menu ("History in the circle", hook H3 in `bankui::draw_card`) and reads nothing before.
//!
//! The page runs its own reads (history lines, change lists, version bodies, a picture) on the circle model's runner and
//! keeps them in the session's [`HistoryCache`] (`CircleEditModel::question_cache`), each under the account, role and
//! hiding state it was read with (round 3, critic B1). The bank changes only in [`sync_page`], called from
//! `CircleEditModel::sync`: a restore of one question (or its undo) goes through [`restore_question`] / [`undo_restore`],
//! a restore point first. Server text is drawn through `api::plain`.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use egui::{Margin, Ui};

use super::api::{self, ApiError};
use super::browse::{SharedImages, SharedRestore};
use super::circle_edit::EditView;
use super::circle_links::CircleLink;
use super::circle_versions_api::{ChangesPage, VersionLine, VersionsPage};
use super::circles::CirclesModel;
use super::circles_api::{BankDoc, Role};
use super::circles_text as words;
use super::circles_ui::{back_link, line, pill, text_row};
use super::model::Runner;
use super::pool::{Blob, LocalProblem, Pool, PoolError, RefusalKind, RetryPolicy};
use super::publish::SharedSession;
use super::publish_ui::{para, warn_box};
use super::question_history::*;
use crate::model::{Kind, QuestionBank};
use crate::{kit, theme};

/// What the page's reads run with: the circle model's session, runner and stores (given when the page opens).
pub struct Env {
    pub session: Option<SharedSession>,
    pub runner: Runner,
    pub retry: RetryPolicy,
    pub repaint: Option<Arc<dyn Fn() + Send + Sync>>,
    pub images: SharedImages,
    pub restore: SharedRestore,
}

/// Where the page's first read stands.
#[derive(Clone, Debug, PartialEq)]
pub enum Load {
    /// The circle does not have the question (new in this copy): nothing to read.
    Unknown,
    SignedOut,
    Loading,
    Ready,
    /// What went wrong, and whether Try again can help.
    Failed(String, bool),
}

#[derive(Clone, Debug, PartialEq)]
pub enum CompareState {
    Reading,
    Ready(Comparison),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompareView {
    /// The older (or chosen) side, drawn first.
    pub left: Side,
    pub right: Side,
    pub state: CompareState,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RestoreState {
    /// The question of plan 4.5 is asked; nothing is read or changed yet.
    Asking,
    /// Yes was pressed: the version (and its picture) is being read; the bank changes in the next `sync`.
    Reading,
    Done(String),
    Failed(String),
    /// (round 5, critic I4) "Undo restore" did not go through: the undo's own failure (its Try again undoes).
    UndoFailed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RestoreView {
    pub version: u64,
    pub state: RestoreState,
}

enum Work {
    /// `known`: the change lists just read (only the lines are read again).
    Load { cid: String, bid: String, cached: Option<Box<ChangesPage>>, lines: bool, known: Option<Box<ChangesPage>> },
    Body { cid: String, bid: String, n: u64 },
    Picture { cid: String, bid: String, sha: String },
}

enum Msg {
    Load(Box<Result<(Option<VersionsPage>, ChangesPage), PoolError>>),
    Body(u64, Box<Result<BankDoc, PoolError>>),
    Picture(String, Result<Blob, PoolError>),
}

/// What [`sync_page`] did: the bank changed (and the link to keep, when it changed too), or the page is to close.
#[derive(Debug, Default)]
pub(super) struct SyncOut {
    pub changed: Option<(String, Option<CircleLink>)>,
    pub close: bool,
}

/// What the timeline was last worked out from (round 3, critic I2): it is worked out again only when one of these moves.
#[derive(Clone, Debug, PartialEq)]
struct Inputs {
    bank_modified: i64,
    questions: usize,
    base_version: u64,
    bank_print_at: i64,
    pairs: usize,
    head_seen: u64,
    /// The link's record of the question (its base entry), as JSON.
    base: String,
    scope: String,
    show_all: bool,
    data: u64,
    local_id: String,
}

pub struct QuestionPage {
    pub cid: String,
    pub bid: String,
    pub local_bank_id: String,
    /// The question in the bank (a restored question that came back new has a new one).
    pub local_id: String,
    /// The circle's question the page opened on; kept when a restore unpairs the local question (critic P3).
    pub qid: Option<String>,
    /// "Question 5 · Multiple choice · "Solve 2x + 3 = 7"".
    pub heading: String,
    /// The question's place in the bank (1-based); 0 when it is no longer in the copy.
    pub number: usize,
    pub load: Load,
    pub lines: Vec<VersionLine>,
    pub page: Option<ChangesPage>,
    pub show_all: bool,
    pub compare: Option<CompareView>,
    pub restore: Option<RestoreView>,
    /// Worked out in [`sync_page`] from the link and the bank, for drawing.
    pub timeline: Option<Timeline>,
    pub your_copy: YourCopy,
    pub restore_ok: bool,
    /// The link's base version (the default compare for "Your copy").
    pub base_version: u64,
    /// (round 3, critic I2) How often the timeline was worked out (only when what it shows changed).
    pub timeline_builds: usize,
    /// What the reads are made under (see [`scope_of`]).
    scope: String,
    /// (round 3, critic I3) The link says a viewer of a bank that hides its keys: bank lines, never a tagged read.
    hiding_viewer: bool,
    copy: Option<Shown>,
    /// The newest version the link knew when the lines were read (the lines are kept per it).
    head_key: u64,
    reading: Vec<u64>,
    /// (round 3, critic M2) the picture a restore needs, held in memory until the restore goes through.
    picture: Option<(String, Result<Vec<u8>, String>)>,
    picture_asked: bool,
    lines_again: bool,
    /// The change lists just read, when only the lines are read again.
    known: Option<ChangesPage>,
    /// (round 3, critic P11) the first read waited for a session: it starts once there is one.
    waiting_session: bool,
    /// (round 3, critic I4) the session's account last seen.
    account: Option<Option<String>>,
    /// (round 3, critic P2) the last restore, for "Undo restore", and whether undo was pressed.
    undo: Option<Restored>,
    undo_asked: bool,
    /// Bumped whenever the page's reads change what it shows.
    data: u64,
    inputs: Option<Inputs>,
    /// (round 4, critic S-I4) Each read carries the generation it was asked in; a reset bumps it and older answers are
    /// dropped (never applied to the next account, role or bank state).
    round: u64,
    /// (round 4, critic P-6) A version shown whole ("Show").
    pub preview: Option<u64>,
    /// Worked out in `sync_page` for drawing: the shown version's question (and whether it is the viewers' form), and the
    /// stem of the question a restore asked about would bring in.
    preview_view: Option<(u64, Option<Shown>, bool)>,
    incoming: Option<(u64, String)>,
    /// (round 5, critic I2) The version a restore asked about was read for it (once per ask), or the read failed: why, and
    /// whether Try again can help.
    incoming_asked: Option<u64>,
    incoming_err: Option<(u64, String, bool)>,
    /// (round 5, critic I6) A Show whose read failed.
    preview_err: Option<(u64, String, bool)>,
    /// (round 5, critic I4) Whether Try again can help the restore's (or the undo's) failure.
    fail_retry: bool,
    /// (round 5, critic I3) A question of the bank is open in the app's editor: restore and undo wait.
    pub draft_open: bool,
    /// (round 5, critic S3) The held-back versions, sorted (for `held_between`).
    held: Vec<u64>,
    /// (round 4, critic P-4) The last restore's words, put back when a second restore is cancelled.
    undo_text: Option<String>,
    env: Env,
    jobs: Vec<(u64, Receiver<Msg>)>,
}

impl std::fmt::Debug for QuestionPage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuestionPage").field("local_id", &self.local_id).field("qid", &self.qid).field("load", &self.load).field("compare", &self.compare).field("restore", &self.restore).finish()
    }
}

fn kind_tag(k: &Kind) -> &'static str {
    match k {
        Kind::Mc { .. } => "Mc",
        Kind::Tf { .. } => "Tf",
        Kind::Matching { .. } => "Matching",
        Kind::Blank { .. } => "Blank",
        Kind::Short { .. } => "Short",
        Kind::Multipart { .. } => "Multipart",
    }
}

fn heading_of(bank: &QuestionBank, local_id: &str) -> (usize, String) {
    match bank.questions.iter().position(|q| q.id == local_id) {
        Some(i) => {
            let q = &bank.questions[i];
            (i + 1, format!("Question {} · {} · \"{}\"", i + 1, super::publish_text::kind_name(kind_tag(&q.kind)), super::merge::short_title(&q.stem, 60)))
        }
        None => (0, "A question no longer in your copy".into()),
    }
}

fn problem_words(e: &PoolError) -> (String, bool) {
    let retry = matches!(e, PoolError::Api(_) | PoolError::Refused(_));
    (words::circle_words(&e.message()), retry)
}

fn side_name(s: Side) -> String {
    match s {
        Side::Version(n) => format!("Version {n}"),
        Side::YourCopy => "Your copy".into(),
    }
}

/// "question 5", or "this question" for one no longer in the copy (critic P11).
fn question_words(n: usize) -> String {
    if n == 0 { "this question".into() } else { format!("question {n}") }
}

const SIGNED_OUT: &str = "Not signed in. Sign in, then try again.";
const BACK_NEW: &str = "It comes back as a new question when you send.";

fn hiding_viewer_of(link: &CircleLink) -> bool {
    link.viewers_form || (link.seen.hide_key && link.role() == Some(Role::Viewer))
}

impl QuestionPage {
    /// The page for the bank's question `question_id`; its first read starts at once (none for a question the circle
    /// does not have, and none signed out).
    pub fn open(link: &CircleLink, bank: &QuestionBank, question_id: &str, env: Env, cache: &mut HistoryCache) -> QuestionPage {
        let (number, heading) = heading_of(bank, question_id);
        let qid = link.qid_of(question_id).map(str::to_string);
        let mut p = QuestionPage {
            cid: link.circle_id.clone(),
            bid: link.circle_bank_id.clone(),
            local_bank_id: bank.id.clone(),
            local_id: question_id.to_string(),
            qid,
            heading,
            number,
            load: Load::Loading,
            lines: Vec::new(),
            page: None,
            show_all: false,
            compare: None,
            restore: None,
            timeline: None,
            your_copy: your_copy(link, bank, question_id),
            restore_ok: restore_offered(link),
            base_version: link.base_version,
            timeline_builds: 0,
            scope: scope_of(link),
            hiding_viewer: hiding_viewer_of(link),
            copy: side_of_copy(link, bank, question_id),
            head_key: link.head_seen.max(link.base_version),
            reading: Vec::new(),
            picture: None,
            picture_asked: false,
            lines_again: false,
            known: None,
            waiting_session: false,
            account: None,
            undo: None,
            undo_asked: false,
            data: 0,
            inputs: None,
            round: 0,
            preview: None,
            preview_view: None,
            incoming: None,
            incoming_asked: None,
            incoming_err: None,
            preview_err: None,
            fail_retry: false,
            draft_open: false,
            held: Vec::new(),
            undo_text: None,
            env,
            jobs: Vec::new(),
        };
        if p.qid.is_none() {
            p.load = Load::Unknown;
        } else {
            p.reload(cache);
        }
        p
    }

    fn launch(&mut self, work: Work) -> bool {
        let Some(session) = self.env.session.clone().map(super::pool::pinned) else { return false };
        let (tx, rx) = channel::<Msg>();
        let (retry, repaint) = (self.env.retry, self.env.repaint.clone());
        match self.env.runner {
            #[cfg(not(target_arch = "wasm32"))]
            Runner::Inline => run(session, retry, work, tx, repaint),
            #[cfg(not(target_arch = "wasm32"))]
            Runner::Background => {
                std::thread::spawn(move || run(session, retry, work, tx, repaint));
            }
            #[cfg(target_arch = "wasm32")]
            _ => wasm_bindgen_futures::spawn_local(async move { run_async(session, retry, work, tx, repaint).await }),
        }
        self.jobs.push((self.round, rx));
        true
    }

    /// The first read, or Try again: the change lists (with the tag of the ones read before under the same account, role
    /// and hiding state: a 304 is free; never for a viewer of a hiding bank, critic I3) and, when this session has none for
    /// the bank's newest version, the history lines.
    pub fn reload(&mut self, cache: &mut HistoryCache) {
        if self.qid.is_none() {
            return;
        }
        let lines = cache.lines(&self.cid, &self.bid, &self.scope, self.head_key).map(<[VersionLine]>::to_vec);
        let need_lines = lines.is_none() || self.lines_again;
        if let Some(l) = lines.filter(|_| !self.lines_again) {
            self.lines = l;
        }
        let cached = if self.hiding_viewer || self.lines_again { None } else { cache.page(&self.cid, &self.bid, &self.scope).cloned() };
        self.load = Load::Loading;
        self.waiting_session = false;
        let known = self.known.take();
        if !self.launch(Work::Load { cid: self.cid.clone(), bid: self.bid.clone(), cached: cached.map(Box::new), lines: need_lines, known: known.map(Box::new) }) {
            self.load = Load::SignedOut;
            self.waiting_session = true;
        }
    }

    /// (round 3, critic B1) Everything read for the bank is dropped and read again. (round 6, critic N4) `keep_result`:
    /// the page stays open for the same person (a role or hiding change), so the last restore's Undo and its words stay;
    /// another account's page drops them (they are not its to see). A restore still being read is dropped either way: its
    /// version was read under the old scope.
    fn forget_reads(&mut self, cache: &mut HistoryCache, keep_result: bool) {
        cache.drop_bank(&self.cid, &self.bid);
        self.lines.clear();
        self.page = None;
        self.compare = None;
        self.preview = None;
        self.reading.clear();
        // (round 4, critic S-I4, S-m2, S-m7) answers on their way are dropped by their round when they arrive
        self.round += 1;
        self.picture = None;
        self.picture_asked = false;
        if keep_result {
            if self.restore.as_ref().is_some_and(|r| matches!(r.state, RestoreState::Asking | RestoreState::Reading)) {
                self.restore = None;
            }
        } else {
            self.undo = None;
            self.undo_text = None;
            self.undo_asked = false;
            self.restore = None;
        }
        self.known = None;
        self.data += 1;
    }

    fn on_load(&mut self, r: Result<(Option<VersionsPage>, ChangesPage), PoolError>, cache: &mut HistoryCache) {
        match r {
            Ok((vp, page)) => {
                let read_lines = vp.is_some();
                // (round 3, critic B1) something moved at the pool without a new head (hiding turned on, a version held
                // back): a new or missing tag, or lists where there were none (or the other way). Nothing read for the
                // bank before is used again, and its lines are read again.
                let prior = cache.page(&self.cid, &self.bid, &self.scope).cloned();
                let moved = !page.from_cache && prior.as_ref().is_some_and(|p| page.etag.is_none() || page.etag != p.etag || page.per_question != p.per_question);
                // (round 4, critic S-m5) a first answer for the bank, with nothing kept of an earlier one (its page was
                // evicted): bodies and lines kept from before it are not trusted either
                let moved = moved || (!page.from_cache && prior.is_none() && cache.has_bank(&self.cid, &self.bid) && !read_lines);
                if !page.from_cache && prior.is_none() && read_lines {
                    cache.drop_bank(&self.cid, &self.bid);
                }
                if moved {
                    cache.drop_bank(&self.cid, &self.bid);
                    self.compare = None;
                    if !read_lines {
                        self.lines_again = true;
                        self.known = Some(page);
                        self.reload(cache);
                        return;
                    }
                }
                if let Some(vp) = vp {
                    self.lines = vp.versions;
                    cache.put_lines(&self.cid, &self.bid, &self.scope, self.head_key, self.lines.clone());
                }
                cache.put_page(&self.cid, &self.bid, &self.scope, page.clone());
                // lines kept from earlier this session that end below the circle's newest version: read once more
                let newest = self.lines.iter().map(|l| l.version).max().unwrap_or(0);
                if !read_lines && newest < page.head_version && !self.lines_again {
                    self.lines_again = true;
                    self.known = Some(page);
                    self.reload(cache);
                    return;
                }
                self.lines_again = false;
                self.page = Some(page);
                self.load = Load::Ready;
                self.data += 1;
            }
            Err(PoolError::Api(ApiError::SignedOut)) => self.load = Load::SignedOut,
            Err(e) => {
                self.lines_again = false;
                let (m, retry) = problem_words(&e);
                self.load = Load::Failed(m, retry);
            }
        }
    }

    /// (round 3, critic B1) The page says this person sees the viewers' form: a body read in full is never used.
    fn viewers_only(&self) -> bool {
        self.hiding_viewer || self.page.as_ref().is_some_and(|p| !p.per_question)
    }

    fn on_body(&mut self, n: u64, r: Result<BankDoc, PoolError>, cache: &mut HistoryCache, now: i64) {
        self.reading.retain(|v| *v != n);
        // (what went wrong, and whether Try again can help)
        let failed: Option<(String, bool)> = match r {
            // (critic M4) the answer is that version of this bank, or nothing of it is kept
            Ok(doc) if doc.bank.version != n || (!doc.bank.id.is_empty() && doc.bank.id != self.bid) => Some((format!("The circle answered with something other than version {n} of this bank, so nothing of it is shown."), true)),
            Ok(doc) if self.viewers_only() && !doc.bank.keys_hidden => Some((format!("Version {n} could not be read in the viewers' form."), true)),
            Ok(doc) => {
                let body = VersionBody::from_payload(n, &doc.payload, doc.bank.keys_hidden, now);
                if super::circle_links::payload_question_count(&doc.payload) > body.questions.len() {
                    Some((PoolError::Local(LocalProblem::NewerQuestions).message(), false))
                } else {
                    cache.put_body(&self.cid, &self.bid, &self.scope, body);
                    self.data += 1;
                    None
                }
            }
            Err(PoolError::Api(ApiError::SignedOut)) => Some((SIGNED_OUT.into(), true)),
            Err(e) => {
                // (critic B1) a version the pool no longer serves: whatever this session held of it goes
                let gone = matches!(&e, PoolError::Refused(r) if matches!(r.kind, RefusalKind::VersionWithheld | RefusalKind::VersionNotRetained));
                if gone {
                    cache.drop_body(&self.cid, &self.bid, n);
                }
                let (m, retry) = problem_words(&e);
                Some((m, retry && !gone))
            }
        };
        let Some((m, retry)) = failed else { return };
        if let Some(c) = self.compare.as_mut().filter(|c| c.state == CompareState::Reading && (c.left == Side::Version(n) || c.right == Side::Version(n))) {
            c.state = CompareState::Failed(m.clone());
        }
        if let Some(r) = self.restore.as_mut().filter(|r| r.version == n && r.state == RestoreState::Reading) {
            r.state = RestoreState::Failed(m.clone());
            self.fail_retry = retry;
        }
        // (round 5, critic I2) the question's own read: said in it, read again only on Try again
        if self.restore.as_ref().is_some_and(|r| r.version == n && r.state == RestoreState::Asking) {
            self.incoming_err = Some((n, m.clone(), retry));
        }
        // (round 5, critic I6) a Show's read
        if self.preview == Some(n) {
            self.preview_err = Some((n, m, retry));
        }
    }

    fn poll(&mut self, cache: &mut HistoryCache, now: i64) {
        let mut got = Vec::new();
        let round = self.round;
        self.jobs.retain(|(g, rx)| match rx.try_recv() {
            Ok(m) => {
                if *g == round {
                    got.push(m);
                }
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => false,
        });
        for m in got {
            match m {
                Msg::Load(r) => self.on_load(*r, cache),
                Msg::Body(n, r) => self.on_body(n, *r, cache, now),
                Msg::Picture(sha, r) => {
                    let r = r.map(|b| b.bytes).map_err(|e| match e {
                        PoolError::Api(ApiError::SignedOut) => "Not signed in. Sign in, then press Restore again.".to_string(),
                        e => format!("The version's picture could not be read: {}", words::circle_words(&e.message())),
                    });
                    self.picture = Some((sha, r));
                }
            }
        }
    }

    pub fn busy(&self) -> bool {
        !self.jobs.is_empty()
    }

    /// The body of version `n` this page may use: read under its scope, and (critic B1) never one read in full while the
    /// page says this person sees the viewers' form (that one is dropped, to be read again).
    fn usable_body(&self, cache: &mut HistoryCache, n: u64) -> Option<Arc<VersionBody>> {
        let b = cache.body(&self.cid, &self.bid, &self.scope, n)?;
        // (round 4, critic S-m5) the lines as read now: a version since held back or no longer kept is not used
        let unreadable = self.lines.iter().any(|l| l.version == n && (l.withheld || !l.kept));
        if unreadable || (self.viewers_only() && !b.keys_hidden) {
            cache.drop_body(&self.cid, &self.bid, n);
            return None;
        }
        Some(b)
    }

    /// Reads the versions `sides` need that this session does not hold.
    fn read_sides(&mut self, sides: &[Side], cache: &mut HistoryCache) -> Result<(), String> {
        for s in sides {
            if let Side::Version(n) = s {
                let _ = self.usable_body(cache, *n);
            }
        }
        let need = reads_needed(&self.lines, cache, &self.cid, &self.bid, &self.scope, sides).map_err(|e| match e {
            NotReadable::Withheld(n) => format!("Version {n} is held back for review, so it can not be compared."),
            NotReadable::NotKept(n) => format!("Version {n} is no longer kept in full, so it can not be compared."),
        })?;
        for n in need {
            if self.reading.contains(&n) {
                continue;
            }
            if !self.launch(Work::Body { cid: self.cid.clone(), bid: self.bid.clone(), n }) {
                return Err(SIGNED_OUT.into());
            }
            self.reading.push(n);
        }
        Ok(())
    }

    /// Compare two sides (plan 4.6): two version reads, none for "Your copy" or a version read this session.
    pub fn open_compare(&mut self, left: Side, right: Side, cache: &mut HistoryCache) {
        let state = match self.read_sides(&[left, right], cache) {
            Ok(()) => CompareState::Reading,
            Err(m) => CompareState::Failed(m),
        };
        self.compare = Some(CompareView { left, right, state });
        self.refresh_compare(cache);
    }

    pub fn close_compare(&mut self) {
        self.compare = None;
    }

    fn refresh_compare(&mut self, cache: &mut HistoryCache) {
        let Some(c) = self.compare.clone().filter(|c| c.state == CompareState::Reading) else { return };
        let Some(qid) = self.qid.clone() else { return };
        let mut hidden = self.viewers_only();
        let mut side = |p: &QuestionPage, s: Side| -> Option<Option<Shown>> {
            match s {
                Side::YourCopy => Some(p.copy.clone()),
                Side::Version(n) => p.usable_body(cache, n).map(|b| {
                    hidden |= b.keys_hidden;
                    side_of_version(&b, &qid)
                }),
            }
        };
        let (Some(l), Some(r)) = (side(self, c.left), side(self, c.right)) else { return };
        if let Some(cv) = self.compare.as_mut() {
            cv.state = CompareState::Ready(compare(l.as_ref(), r.as_ref(), hidden));
        }
    }

    /// "Restore...": the question is asked first; nothing is read or changed. Only on an edit link, and (critic P6) never
    /// in place of a question already asked.
    pub fn ask_restore(&mut self, version: u64) {
        if self.restore_ok && !self.restore.as_ref().is_some_and(|r| matches!(r.state, RestoreState::Asking | RestoreState::Reading)) {
            self.restore = Some(RestoreView { version, state: RestoreState::Asking });
            self.incoming_asked = None;
            self.incoming_err = None;
        }
    }

    /// Cancel (or Escape, or Dismiss). (round 4, critic P-4) A second restore cancelled: the last one's words, and its
    /// Undo, come back.
    pub fn cancel_restore(&mut self) {
        let asked = self.restore.as_ref().is_some_and(|r| r.state == RestoreState::Asking);
        self.restore = match (&self.undo, &self.undo_text) {
            (Some(u), Some(t)) if asked => Some(RestoreView { version: u.version, state: RestoreState::Done(t.clone()) }),
            _ => None,
        };
    }

    /// (round 4) "Try again" on a failed restore: asked once more.
    pub fn retry_restore(&mut self) {
        if let Some(r) = self.restore.as_mut().filter(|r| matches!(r.state, RestoreState::Failed(_))) {
            r.state = RestoreState::Asking;
            self.incoming_asked = None;
            self.incoming_err = None;
        }
    }

    /// (round 5, critic I2) Try again on the question's own read.
    pub fn retry_incoming(&mut self) {
        self.incoming_err = None;
        self.incoming_asked = None;
    }

    /// (round 4, critic P-6) "Show": the whole question of a version (one read, none when held).
    pub fn show(&mut self, version: u64, cache: &mut HistoryCache) {
        if self.preview == Some(version) {
            self.preview = None;
            return;
        }
        self.preview = Some(version);
        self.preview_err = None;
        if let Err(m) = self.read_sides(&[Side::Version(version)], cache) {
            self.preview_err = Some((version, m, false));
        }
    }

    /// The body of a version for drawing (a preview, the incoming question of a restore).
    pub(super) fn shown_side(&self, cache: &mut HistoryCache, version: u64) -> Option<Option<Shown>> {
        let qid = self.qid.clone()?;
        self.usable_body(cache, version).map(|b| side_of_version(&b, &qid))
    }

    /// "Yes, restore": the version is read (none when it was just compared); the bank changes in the next `sync`.
    pub fn confirm_restore(&mut self, cache: &mut HistoryCache) {
        let Some(v) = self.restore.as_ref().filter(|r| r.state == RestoreState::Asking).map(|r| r.version) else { return };
        if !self.restore_ok {
            self.restore = None;
            return;
        }
        self.picture = None;
        self.picture_asked = false;
        // (round 5, critic I3) not while a question of the bank is open in the editor
        if self.draft_open {
            self.fail_retry = true;
            self.restore = Some(RestoreView { version: v, state: RestoreState::Failed(format!("{DRAFT_OPEN}: its draft would be saved over the restored question.")) });
            return;
        }
        let state = match self.read_sides(&[Side::Version(v)], cache) {
            Ok(()) => RestoreState::Reading,
            Err(m) => RestoreState::Failed(m),
        };
        self.fail_retry = true;
        self.restore = Some(RestoreView { version: v, state });
    }

    /// "Undo restore" (critic P2): done in the next `sync`.
    pub fn ask_undo(&mut self) {
        if self.restore_ok && self.undo.is_some() {
            self.undo_asked = true;
        }
    }

    /// The question the circle removed (by the lists), or one no longer paired with the circle's.
    fn comes_back_new(&self, link: Option<&CircleLink>) -> bool {
        let Some(qid) = &self.qid else { return false };
        self.page.as_ref().and_then(|p| removed_at_head(p, qid)) == Some(true) || link.is_some_and(|l| !l.qids.iter().any(|x| &x.qid == qid))
    }

    fn refresh(&mut self, link: &CircleLink, bank: &QuestionBank, cache: &mut HistoryCache) {
        let inputs = Inputs {
            bank_modified: bank.modified,
            questions: bank.questions.len(),
            base_version: link.base_version,
            bank_print_at: link.bank_modified,
            pairs: link.qids.len(),
            head_seen: link.head_seen,
            base: link.base.iter().find(|b| b.local_id == self.local_id).and_then(|b| serde_json::to_string(b).ok()).unwrap_or_default(),
            scope: self.scope.clone(),
            show_all: self.show_all,
            data: self.data,
            local_id: self.local_id.clone(),
        };
        if self.inputs.as_ref() == Some(&inputs) {
            return;
        }
        self.inputs = Some(inputs);
        let mut held: Vec<u64> = self.lines.iter().filter(|l| l.withheld).map(|l| l.version).collect();
        held.sort_unstable();
        held.dedup();
        self.held = held;
        let (number, heading) = heading_of(bank, &self.local_id);
        self.number = number;
        self.heading = heading;
        self.restore_ok = restore_offered(link);
        self.base_version = link.base_version;
        self.copy = side_of_copy(link, bank, &self.local_id);
        let mut yc = your_copy(link, bank, &self.local_id);
        // (round 3, critic P9) a difference the link's record can not see (a figure's settings), told by the base version's
        // body when this session read it. (round 4, critic P-2) Never "changed, not sent yet" while the strip counts none:
        // it is said not to be counted.
        if let (YourCopy::SameAs(b), Some(mine)) = (yc, self.copy.as_ref())
            && let Some(theirs) = self.usable_body(cache, b).filter(|x| !x.keys_hidden)
            && let Some(t) = self.qid.as_deref().and_then(|q| theirs.question(q))
            && (!super::merge::same_question(&mine.q, &t.q) || mine.sha != t.sha)
        {
            yc = YourCopy::NotCounted(b);
        }
        self.your_copy = yc;
        // (round 4, critic P-3) a compare with "Your copy" as a side is worked out again from the copy as it is now
        if let Some(c) = self.compare.as_mut()
            && matches!(c.state, CompareState::Ready(_))
            && (c.left == Side::YourCopy || c.right == Side::YourCopy)
        {
            c.state = CompareState::Reading;
        }
        self.timeline_builds += 1;
        self.timeline = self.page.as_ref().filter(|_| self.load == Load::Ready).map(|page| timeline(&TimelineIn { lines: &self.lines, changes: page, link, bank, local_id: &self.local_id, show_all: self.show_all, qid: self.qid.as_deref() }));
    }

    /// The restore, once its version (and picture) is here: [`restore_question`] writes the restore point, then the banks.
    fn restore_step(&mut self, cache: &mut HistoryCache, banks: &mut [QuestionBank], link: &CircleLink, save: &dyn Fn(&[QuestionBank]) -> Result<(), String>, now: i64) -> Option<(String, Option<CircleLink>)> {
        let v = self.restore.as_ref().filter(|r| r.state == RestoreState::Reading)?.version;
        let qid = self.qid.clone()?;
        let body = self.usable_body(cache, v)?;
        let fail = |p: &mut QuestionPage, m: String, retry: bool| {
            p.fail_retry = retry;
            p.restore = Some(RestoreView { version: v, state: RestoreState::Failed(m) });
            None
        };
        let Some(t) = body.question(&qid) else { return fail(self, format!("Version {v} does not have this question. Nothing was changed."), false) };
        let removed = self.comes_back_new(Some(link));
        let local = self.local_id.clone();
        let plain = RestoreAsk { link, local_id: Some(local.as_str()), qid: &qid, body: &body, picture: None, store_picture: None, removed_in_circle: removed };
        // (critic M2) first without a picture: a restore refused for any other reason stores nothing; only one that needs
        // the version's picture fetches it, keeps its bytes in memory, and (round 4, critic S-m3) stores it once the restore
        // point is saved
        let mut r = restore_question(banks, &plain, &*self.env.restore, save, now);
        if r == Err(RestoreRefused::PictureMissing) {
            let sha = t.sha.clone()?;
            match &self.picture {
                Some((s, Ok(bytes))) if *s == sha => {
                    let (images, bytes) = (self.env.images.clone(), bytes.clone());
                    let store = move || images.put(&bytes);
                    let with = RestoreAsk { store_picture: Some(&store), ..plain };
                    r = restore_question(banks, &with, &*self.env.restore, save, now);
                    if r == Err(RestoreRefused::PictureMissing) {
                        return fail(self, format!("Version {v}'s picture couldn't be stored on this computer, so nothing was changed."), true);
                    }
                }
                Some((s, Err(m))) if *s == sha => {
                    let m = nothing_changed(m);
                    return fail(self, m, true);
                }
                _ if !self.picture_asked => {
                    self.picture_asked = true;
                    if !self.launch(Work::Picture { cid: self.cid.clone(), bid: self.bid.clone(), sha }) {
                        return fail(self, SIGNED_OUT.into(), true);
                    }
                    return None;
                }
                _ => return None,
            }
        }
        match r {
            Ok(done) => {
                self.local_id = done.local_id.clone();
                let back_new = if removed { format!(" {BACK_NEW}") } else { String::new() };
                let what = question_words(self.number);
                // (round 5, critic I1) said from the copy as it is now, against the link as it is now
                let after = done.link.as_ref().unwrap_or(link);
                let now_is = banks.iter().find(|b| b.id == self.local_bank_id).map(|b| your_copy(after, b, &done.local_id));
                let counts = match now_is {
                    Some(YourCopy::SameAs(n)) => format!(" Your copy of it is now the same as version {n}, the version your copy is based on, so there is nothing to send for it."),
                    Some(YourCopy::NotCounted(_) | YourCopy::Unknown) => " Only your copy changed. This computer can't tell whether it counts as a change to send.".to_string(),
                    _ => " Only your copy changed: it counts as an unsent change until you send.".to_string(),
                };
                let text = format!("{} is now as it was in version {v}.{counts}{back_new} A restore point of all your banks was saved first, with verSHion's other safety copies. Undo is available until you leave this page.", capital(&what));
                self.undo_text = Some(text.clone());
                self.restore = Some(RestoreView { version: v, state: RestoreState::Done(text) });
                self.picture = None;
                let link = done.link.clone();
                self.undo = Some(done);
                self.data += 1;
                Some((self.local_bank_id.clone(), link))
            }
            Err(e) => {
                let retry = retryable(&e);
                fail(self, refused_words(&e, v), retry)
            }
        }
    }

    /// "Undo restore": the question the restore replaced is put back, a restore point first (critic P2).
    fn undo_step(&mut self, banks: &mut [QuestionBank], link: &CircleLink, save: &dyn Fn(&[QuestionBank]) -> Result<(), String>, now: i64) -> Option<(String, Option<CircleLink>)> {
        if !std::mem::take(&mut self.undo_asked) {
            return None;
        }
        let done = self.undo.take()?;
        let v = done.version;
        match undo_restore(banks, link, &done, &*self.env.restore, save, now) {
            Ok(before) => {
                self.local_id = done.local_id.clone();
                self.undo_text = None;
                self.restore = Some(RestoreView { version: v, state: RestoreState::Done(format!("The restore was undone: {} is back as it was before. A restore point was saved first.", question_words(self.number))) });
                self.data += 1;
                Some((self.local_bank_id.clone(), before))
            }
            Err(e) => {
                // (round 5, critic I4) the undo's own failure: its Try again undoes, and only where that can help
                self.undo = Some(done);
                self.fail_retry = retryable(&e);
                self.restore = Some(RestoreView { version: v, state: RestoreState::UndoFailed(refused_words(&e, v)) });
                None
            }
        }
    }
}

/// A failure's words, ending once with "Nothing was changed." (round 4, minor: never twice).
fn nothing_changed(m: &str) -> String {
    if m.contains("Nothing was changed") { m.to_string() } else { format!("{} Nothing was changed.", m.trim_end()) }
}

/// (round 5, critic I4) A refusal that may pass by itself (a disk, a picture's read); the rest are lasting.
fn retryable(e: &RestoreRefused) -> bool {
    matches!(e, RestoreRefused::RestorePoint(_) | RestoreRefused::NotSaved(_) | RestoreRefused::PictureMissing)
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

fn refused_words(e: &RestoreRefused, v: u64) -> String {
    match e {
        RestoreRefused::FollowLink | RestoreRefused::ViewersForm => "Restore is offered only on a copy you can send from. Nothing was changed.".into(),
        RestoreRefused::KeysHidden => format!("Version {v} was read without its answer keys, so it can not be restored here. Nothing was changed."),
        RestoreRefused::NoBank => "The bank is not on this computer any more. Nothing was changed.".into(),
        RestoreRefused::Stale => "This bank was put back from a safety copy, so which version it matches is not known. Use Get latest first. Nothing was changed.".into(),
        RestoreRefused::NotInVersion => format!("Version {v} does not have this question. Nothing was changed."),
        RestoreRefused::Same => format!("Your copy of this question is already the same as version {v}'s. Nothing was changed."),
        RestoreRefused::PictureMissing => format!("Version {v}'s picture could not be fetched, so nothing was changed. Press Restore again to try again."),
        RestoreRefused::RestorePoint(m) => format!("A copy of your banks could not be saved first ({m}), so nothing was changed."),
        RestoreRefused::NotSaved(m) => format!("Your banks couldn't be saved ({}), so nothing was changed.", words::save_problem(m)),
        RestoreRefused::NotCountable => "This version differs only in figure settings that verSHion can't count as a change here, so it can't be restored from this page. Nothing was changed.".into(),
        RestoreRefused::EditedSince => "This question changed after the restore (an edit, or a difficulty taken from the circle), or was deleted, so Undo would lose that. Nothing was changed; change it back by hand if you need to.".into(),
        RestoreRefused::LinkMoved => "Your copy has moved to another version of the circle's bank since the restore, so Undo can't put the question back as it was. Nothing was changed.".into(),
        RestoreRefused::Moved => "This question is no longer paired with the circle's question it was opened on (a merge or a send changed it). Open its history again. Nothing was changed.".into(),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn run(session: SharedSession, retry: RetryPolicy, work: Work, tx: Sender<Msg>, repaint: Option<Arc<dyn Fn() + Send + Sync>>) {
    super::transport::block_on(run_async(session, retry, work, tx, repaint));
}

async fn run_async(session: SharedSession, retry: RetryPolicy, work: Work, tx: Sender<Msg>, repaint: Option<Arc<dyn Fn() + Send + Sync>>) {
    let pool = Pool::new(&*session).with_retry(retry);
    let msg = match work {
        Work::Load { cid, bid, cached, lines, known } => {
            let r = async {
                let vp = if lines { Some(pool.circle_versions(&cid, &bid, None).await?) } else { None };
                let page = match known {
                    Some(p) => *p,
                    None => pool.circle_changes_if(&cid, &bid, cached.as_deref()).await?,
                };
                Ok((vp, page))
            }
            .await;
            Msg::Load(Box::new(r))
        }
        Work::Body { cid, bid, n } => Msg::Body(n, Box::new(pool.circle_version(&cid, &bid, n).await)),
        // (critic M2) the bytes only, checked against their hash by `circle_blob`; stored when the restore is made
        Work::Picture { cid, bid, sha } => {
            let r = pool.circle_blob(&cid, &bid, &sha).await;
            Msg::Picture(sha, r)
        }
    };
    let _ = tx.send(msg);
    if let Some(r) = repaint {
        r();
    }
}

/// From `CircleEditModel::sync`: the page's answers, the bank and link it draws, and a restore (or its undo) waiting.
/// The page closes when its link or bank is gone, or the circle no longer lists the person (critic M1).
/// (round 5, critic I3) `draft`: a question of the bank is open in the app's editor; `shown`: the page was in front of the
/// person in the frame before (a restore or undo is made only then, so its result is always seen).
#[allow(clippy::too_many_arguments)]
pub(super) fn sync_page(p: &mut QuestionPage, cache: &mut HistoryCache, banks: &mut [QuestionBank], links: &[CircleLink], session: Option<SharedSession>, save: &dyn Fn(&[QuestionBank]) -> Result<(), String>, now: i64, draft: bool, shown: bool) -> SyncOut {
    p.draft_open = draft;
    // (critic I4) another account in the session: nothing read for the first one is used, and the page reads again
    let tag = super::bank_links::account_tag(session.as_ref().and_then(|s| s.account()).as_deref());
    let had_session = p.env.session.is_some();
    p.env.session = session;
    if p.account.as_ref() != Some(&tag) {
        let first = p.account.is_none();
        cache.note_account(tag.as_deref());
        p.account = Some(tag);
        if !first {
            p.forget_reads(cache, false);
            p.reload(cache);
        }
    }
    let Some(link) = links.iter().find(|l| l.local_bank_id == p.local_bank_id) else { return SyncOut { close: true, ..SyncOut::default() } };
    if link.gone() || link.circle_id != p.cid || link.circle_bank_id != p.bid || !banks.iter().any(|b| b.id == p.local_bank_id) {
        return SyncOut { close: true, ..SyncOut::default() };
    }
    // (critic B1, P10) another role or hiding state: what was read under the old one goes, and the page reads again.
    // (round 6, critic N4) The page stays open for the same person: the last restore's Undo and its words stay.
    let scope = scope_of(link);
    if scope != p.scope {
        p.forget_reads(cache, true);
        p.scope = scope;
        p.hiding_viewer = hiding_viewer_of(link);
        p.reload(cache);
    }
    // (critic P11) signed in from the signed-out state: the page reads by itself
    if p.waiting_session && p.env.session.is_some() && (!had_session || p.load == Load::SignedOut) {
        p.reload(cache);
    }
    // (round 4, critic S-I4) answers are taken in only after the account, the link and the role are checked: one asked
    // before a reset is dropped by its round
    p.poll(cache, now);
    let Some(bank) = banks.iter().find(|b| b.id == p.local_bank_id) else { return SyncOut::default() };
    p.refresh(link, bank, cache);
    p.refresh_compare(cache);
    // (round 4, critic P-6) the question a restore asked about would bring in, read to be shown in the question; and a
    // version shown whole
    // (round 5, critic I2) read once per ask: a failure is kept (and said), read again only on Try again
    if let Some(v) = p.restore.as_ref().filter(|r| r.state == RestoreState::Asking).map(|r| r.version) {
        if p.incoming.as_ref().is_none_or(|(n, _)| *n != v) {
            match p.shown_side(cache, v) {
                Some(side) => p.incoming = Some((v, side.map_or_else(|| "(not in that version)".into(), |s| super::merge::short_title(&s.q.stem, 90)))),
                None if p.incoming_asked != Some(v) && p.incoming_err.is_none() => {
                    p.incoming_asked = Some(v);
                    if let Err(m) = p.read_sides(&[Side::Version(v)], cache) {
                        p.incoming_err = Some((v, m, false));
                    }
                }
                None => {}
            }
        }
    } else {
        p.incoming = None;
    }
    // (round 5, critic S2) a body read without its keys is drawn as such, whatever the lists said
    p.preview_view = match p.preview {
        Some(v) => {
            let viewers = p.viewers_only();
            let qid = p.qid.clone();
            p.usable_body(cache, v).map(|b| (v, qid.as_deref().and_then(|q| side_of_version(&b, q)), viewers || b.keys_hidden))
        }
        None => None,
    };
    // (round 5, critic I3) a question of the bank open in the editor: nothing is restored or undone under it
    if draft {
        if let Some(r) = p.restore.as_mut().filter(|r| r.state == RestoreState::Reading) {
            r.state = RestoreState::Failed(format!("{DRAFT_OPEN}: its draft would be saved over the restored question."));
            p.fail_retry = true;
        }
        if std::mem::take(&mut p.undo_asked)
            && let Some(u) = &p.undo
        {
            p.fail_retry = true;
            p.restore = Some(RestoreView { version: u.version, state: RestoreState::UndoFailed(format!("{DRAFT_OPEN}: its draft would be saved over the question put back.")) });
        }
    }
    // only while the page is in front of the person, so the result is seen
    let changed = if shown && !draft { p.restore_step(cache, banks, link, save, now).or_else(|| p.undo_step(banks, link, save, now)) } else { None };
    if changed.is_some()
        && let Some(bank) = banks.iter().find(|b| b.id == p.local_bank_id)
    {
        p.refresh(link, bank, cache);
    }
    SyncOut { changed, close: false }
}

/// Two columns from this width (plan 4.6); stacked below.
pub const COLUMNS_FROM: f32 = 760.0;

pub fn compare_columns(width: f32) -> bool {
    width >= COLUMNS_FROM
}

/// Below this width each row takes two lines (plan 4.8).
const NARROW: f32 = 520.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Act {
    None,
    Back,
    Reload,
    SignIn,
    ShowAll(bool),
    Compare(Side, Side),
    SetLeft(Side),
    SetRight(Side),
    CloseCompare,
    AskRestore(u64),
    ConfirmRestore,
    CancelRestore,
    DismissRestore,
    RetryRestore,
    RetryIncoming,
    Undo,
    Show(u64),
    RetryShow(u64),
    ClosePreview,
}

/// The page (from `circle_edit_ui::draw_page`).
pub(super) fn draw(ui: &mut Ui, m: &mut CirclesModel) {
    let now = m.now_ms();
    let back = m.edit.host.as_ref().map(|h| h.back.clone()).unwrap_or_else(|| "< Back".into());
    let act = match &m.edit.view {
        EditView::Question(p) => draw_question(ui, p, &back, now),
        _ => return,
    };
    if p_busy(m) {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(300));
    }
    let e = &mut m.edit;
    if act == Act::Back {
        e.close();
        return;
    }
    if act == Act::SignIn {
        e.ask_account();
        return;
    }
    let EditView::Question(p) = &mut e.view else { return };
    let cache = &mut e.question_cache;
    match act {
        Act::None | Act::Back | Act::SignIn => {}
        Act::Reload => p.reload(cache),
        Act::ShowAll(on) => p.show_all = on,
        Act::Compare(l, r) => p.open_compare(l, r, cache),
        Act::SetLeft(s) => {
            let r = p.compare.as_ref().map_or(Side::YourCopy, |c| c.right);
            p.open_compare(s, r, cache);
        }
        Act::SetRight(s) => {
            let l = p.compare.as_ref().map_or(Side::YourCopy, |c| c.left);
            p.open_compare(l, s, cache);
        }
        Act::CloseCompare => p.close_compare(),
        Act::AskRestore(v) => p.ask_restore(v),
        Act::ConfirmRestore => p.confirm_restore(cache),
        Act::CancelRestore => p.cancel_restore(),
        Act::DismissRestore => p.restore = None,
        Act::RetryRestore => p.retry_restore(),
        Act::RetryIncoming => p.retry_incoming(),
        Act::Undo => p.ask_undo(),
        Act::Show(v) => p.show(v, cache),
        Act::RetryShow(v) => {
            p.preview = None;
            p.show(v, cache);
        }
        Act::ClosePreview => p.preview = None,
    }
}

fn p_busy(m: &CirclesModel) -> bool {
    matches!(&m.edit.view, EditView::Question(p) if p.busy())
}

fn who_when(l: Option<&VersionLine>, now: i64) -> Vec<String> {
    let Some(l) = l else { return Vec::new() };
    let mut bits = Vec::new();
    let who = api::plain(&l.by);
    if !who.is_empty() {
        bits.push(who);
    }
    let ago = words::ago(l.at, now);
    if !ago.is_empty() {
        bits.push(ago);
    }
    bits
}

fn message_of(l: Option<&VersionLine>) -> Option<String> {
    l.map(|l| api::plain(&l.message)).filter(|m| !m.trim().is_empty()).map(|m| format!("\"{}\"", super::merge::short_title(&m, 80)))
}

fn tag_word(t: Tag) -> &'static str {
    match t {
        Tag::Added => "Added",
        Tag::Changed => "Changed",
        Tag::Removed => "Removed",
        Tag::AsItWas => "As it was then",
    }
}

fn spinner_line(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.spinner();
        ui.label(theme::muted(text));
    });
}

/// The held-back versions strictly between two versions (critic P8).
/// (round 5, critic S3) From the page's sorted list of them, worked out once per read.
fn held_between(p: &QuestionPage, a: u64, b: u64) -> Vec<u64> {
    let (lo, hi) = (a.min(b), a.max(b));
    let from = p.held.partition_point(|v| *v <= lo);
    let to = p.held.partition_point(|v| *v < hi);
    p.held[from..to.max(from)].to_vec()
}

/// A ghost button that is drawn but does nothing while `on` is false (styled as `kit::ghost_button`, added in place so a
/// wrapped row never squeezes it).
fn button_if(ui: &mut Ui, on: bool, label: &str) -> bool {
    if on {
        return kit::ghost_button(ui, label).clicked();
    }
    let b = egui::Button::new(egui::RichText::new(label).family(theme::bold_family()).color(theme::ink()))
        .fill(egui::Color32::TRANSPARENT)
        .stroke(egui::Stroke::new(1.0, theme::rule()))
        .corner_radius(egui::CornerRadius::same(17))
        .min_size(egui::vec2(0.0, 34.0));
    ui.add_enabled(false, b).clicked()
}

/// A restore asked about or under way: other rows' Restore and Compare wait (critic P6, round 4 P-3).
fn restore_busy(p: &QuestionPage) -> bool {
    p.restore.as_ref().is_some_and(|r| matches!(r.state, RestoreState::Asking | RestoreState::Reading))
}

fn draw_question(ui: &mut Ui, p: &QuestionPage, back: &str, now: i64) -> Act {
    let mut act = Act::None;
    if back_link(ui, back) {
        act = Act::Back;
    }
    ui.add_space(4.0);
    ui.add(egui::Label::new(theme::serif("History in the circle").size(20.0)).wrap());
    ui.add(egui::Label::new(theme::strong(p.heading.as_str()).size(14.0)).wrap());
    ui.add_space(4.0);
    // (critic P6) Escape takes back a question asked (round 4: only one in view, not behind a compare)
    // (round 5) then a compare, a version shown, a result
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        let asking = p.restore.as_ref().is_some_and(|r| r.state == RestoreState::Asking);
        act = if p.compare.is_none() && asking {
            Act::CancelRestore
        } else if p.compare.is_some() {
            Act::CloseCompare
        } else if p.preview.is_some() {
            Act::ClosePreview
        } else if p.restore.as_ref().is_some_and(|r| matches!(r.state, RestoreState::Done(_) | RestoreState::Failed(_) | RestoreState::UndoFailed(_))) {
            Act::DismissRestore
        } else {
            act
        };
    }
    let free = !restore_busy(p);
    // "Your copy: ..." and Compare with...
    let compare_ok = p.load == Load::Ready && p.copy.is_some() && p.lines.iter().any(|l| l.kept && !l.withheld);
    text_row(ui, |ui| {
        ui.add(egui::Label::new(theme::text(your_copy_text(p.your_copy)).size(13.0)).wrap());
        if compare_ok && p.compare.is_none() && button_if(ui, free, "Compare with...") {
            let base = Side::Version(p.base_version);
            let left = if p.lines.iter().any(|l| l.version == p.base_version && l.kept && !l.withheld) { base } else { sides_of(p).into_iter().next().unwrap_or(base) };
            act = Act::Compare(left, Side::YourCopy);
        }
    });
    ui.add_space(6.0);
    match &p.load {
        Load::Unknown => {
            para(ui, "The circle doesn't have this question yet: it is new in your copy. Once you send it, its history is kept here.");
            return act;
        }
        Load::SignedOut => {
            para(ui, "Not signed in. Sign in, then press Try again.");
            ui.horizontal_wrapped(|ui| {
                if kit::primary_button(ui, "Sign in...").clicked() {
                    act = Act::SignIn;
                }
                if kit::ghost_button(ui, "Try again").clicked() {
                    act = Act::Reload;
                }
            });
            return act;
        }
        Load::Loading => {
            spinner_line(ui, "Reading this question's history...");
            return act;
        }
        Load::Failed(m, retry) => {
            warn_box(ui, m);
            if *retry && kit::primary_button(ui, "Try again").clicked() {
                act = Act::Reload;
            }
            return act;
        }
        Load::Ready => {}
    }
    if let Some(c) = &p.compare {
        // (round 4, critic P-3) a restore's result (and its Undo) stays in view above an open compare
        if let Some(r) = &p.restore {
            restore_panel(ui, p, r, &mut act);
        }
        compare_panel(ui, p, c, &mut act);
        return act;
    }
    let Some(t) = &p.timeline else { return act };
    if !t.per_question {
        para(ui, "This bank hides its answer keys from viewers, so this list shows the bank's versions, not which questions changed. Compare and Show show versions as viewers see them; each version read uses 1 of your 100 daily reads of hidden banks.");
    }
    let narrow = ui.available_width() < NARROW;
    if let Some(e) = empty_text(t) {
        line(ui, e);
    }
    // a restore asked about is drawn under its own row; one whose row is not listed, at the top
    let restore_row = p.restore.as_ref().filter(|r| t.rows.iter().any(|x| matches!(x, Row::Version { version, .. } if *version == r.version)));
    if let Some(r) = p.restore.as_ref().filter(|_| restore_row.is_none()) {
        restore_panel(ui, p, r, &mut act);
    }
    let stale = p.your_copy == YourCopy::Stale;
    let listed = |v: u64| t.rows.iter().any(|r| matches!(r, Row::Version { version, .. } if *version == v));
    for row in &t.rows {
        match row {
            Row::Version { version, line: l, tag, compared_to, compare, restore } => {
                // (critic P7) no Restore on a bank put back from a safety copy, nor to the version the copy already is;
                // (round 4) nor to the version just restored
                let just = p.undo.as_ref().is_some_and(|u| u.version == *version);
                let restore = *restore && !stale && p.your_copy != YourCopy::SameAs(*version) && p.your_copy != YourCopy::NotCounted(*version) && !just;
                // (round 4, minor) a compare against "Your copy" says so
                let with_copy = compared_to.is_none();
                kit::card().inner_margin(Margin::symmetric(10, 8)).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    let mut head = vec![format!("Version {version}")];
                    head.extend(who_when(l.as_ref(), now));
                    let mut msg = message_of(l.as_ref());
                    // (critic P8) a list worked out across a held-back version says what it is against
                    if let Some(c) = compared_to.filter(|c| tag.is_some() && !held_between(p, *c, *version).is_empty()) {
                        let what = match tag {
                            Some(Tag::Added) => "added",
                            Some(Tag::Removed) => "removed",
                            _ => "changed",
                        };
                        // (round 4, minor) a version named that has no row of its own says why
                        let why = if listed(c) { String::new() } else { format!(" (version {c} has no row: this question was the same in it as before)") };
                        let since = format!("{what} since version {c}{why}");
                        msg = Some(msg.map_or(since.clone(), |m| format!("{m} · {since}")));
                    }
                    if just {
                        msg = Some(msg.map_or("your copy is now this".into(), |m| format!("{m} · your copy is now this")));
                    }
                    // the tag and buttons, in reading order (`from_right`: drawn into a right-to-left layout)
                    let buttons = |ui: &mut Ui, act: &mut Act, from_right: bool| {
                        let mut parts: Vec<u8> = vec![0, 1, 2, 3];
                        if from_right {
                            parts.reverse();
                        }
                        for part in parts {
                            match part {
                                0 => {
                                    if let Some(t) = tag {
                                        pill(ui, tag_word(*t), if *t == Tag::AsItWas { theme::accent_faded() } else { theme::accent_col() }, if *t == Tag::AsItWas { theme::text_col() } else { theme::on_accent_col() });
                                    }
                                }
                                1 => {
                                    // (round 4, critic P-6) the whole question of that version
                                    let label = if p.preview == Some(*version) { "Hide" } else { "Show" };
                                    if kit::ghost_button(ui, label).clicked() {
                                        *act = Act::Show(*version);
                                    }
                                }
                                2 => {
                                    let label = if with_copy { "Compare with your copy" } else { "Compare" };
                                    if *compare && button_if(ui, free, label) {
                                        let left = compared_to.map(Side::Version).unwrap_or(Side::Version(*version));
                                        let right = if compared_to.is_some() { Side::Version(*version) } else { Side::YourCopy };
                                        *act = Act::Compare(left, right);
                                    }
                                }
                                _ => {
                                    if restore && button_if(ui, free, "Restore...") {
                                        *act = Act::AskRestore(*version);
                                    }
                                    // (round 5, critic I5) Undo stays reachable on the row restored, the result dismissed
                                    if just && p.restore_ok && !matches!(p.restore.as_ref().map(|r| &r.state), Some(RestoreState::Done(_) | RestoreState::UndoFailed(_))) && button_if(ui, free, "Undo restore") {
                                        *act = Act::Undo;
                                    }
                                }
                            }
                        }
                    };
                    if narrow {
                        // (plan 4.8) who and when, then the message, then the tag and buttons
                        ui.add(egui::Label::new(theme::strong(head.join(" · ")).size(13.0)).wrap());
                        if let Some(m) = &msg {
                            ui.add(egui::Label::new(theme::muted(m.as_str()).size(12.5)).wrap());
                        }
                        ui.horizontal_wrapped(|ui| buttons(ui, &mut act, false));
                    } else {
                        // (critic P12) the words at the left, the tag and buttons at the right, as the plan's mock
                        let mut all = head.clone();
                        all.extend(msg.clone());
                        egui::Sides::new().shrink_left().wrap().show(ui, |ui| ui.add(egui::Label::new(theme::text(all.join(" · ")).size(13.0)).wrap()), |ui| buttons(ui, &mut act, true));
                    }
                });
                if let Some((v, side, hidden)) = p.preview_view.as_ref().filter(|(v, _, _)| v == version) {
                    preview_panel(ui, *v, side.as_ref(), *hidden);
                } else if let Some((_, m, retry)) = p.preview_err.as_ref().filter(|(v, _, _)| v == version) {
                    // (round 5, critic I6) why it could not be shown, with Try again where it can help
                    warn_box(ui, m);
                    ui.horizontal_wrapped(|ui| {
                        if *retry && kit::ghost_button(ui, "Try again").clicked() {
                            act = Act::RetryShow(*version);
                        }
                        if kit::ghost_button(ui, "Close").clicked() {
                            act = Act::ClosePreview;
                        }
                    });
                } else if p.preview == Some(*version) {
                    spinner_line(ui, &format!("Reading version {version}..."));
                }
                if let Some(r) = restore_row.filter(|r| r.version == *version) {
                    restore_panel(ui, p, r, &mut act);
                }
                ui.add_space(4.0);
            }
            Row::HeldBack { version, .. } => {
                kit::card().inner_margin(Margin::symmetric(10, 8)).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.add(egui::Label::new(theme::muted(format!("Version {version} · held back for review · no details")).size(13.0)).wrap());
                });
                ui.add_space(4.0);
            }
            Row::Gap(g) => {
                // (critic P12) the body text colour: italic small type keeps a clear contrast on the cream page
                ui.add(egui::Label::new(theme::text(gap_text(g)).size(12.5).italics()).wrap());
                ui.add_space(4.0);
            }
            Row::LinesOnly { line: l } => {
                let mut bits = vec![format!("Version {}", l.version)];
                bits.extend(who_when(Some(l), now));
                bits.extend(message_of(Some(l)));
                bits.push("no longer kept in full".into());
                ui.add(egui::Label::new(theme::muted(bits.join(" · ")).size(12.5)).wrap());
                ui.add_space(4.0);
            }
            Row::Older { from, to, count } => {
                let what = if *count == 1 || from == to { format!("Version {to} is") } else { format!("Versions {from}-{to} are") };
                text_row(ui, |ui| {
                    ui.add(egui::Label::new(theme::muted(format!("{what} no longer kept in full: who and when only.")).size(12.5)).wrap());
                    if kit::ghost_button(ui, "Show every version").clicked() {
                        act = Act::ShowAll(true);
                    }
                });
            }
        }
    }
    if p.show_all {
        ui.add_space(4.0);
        if kit::ghost_button(ui, "Show fewer versions").clicked() {
            act = Act::ShowAll(false);
        }
    } else if !t.rows.iter().any(|r| matches!(r, Row::Older { .. })) && p.lines.iter().any(|l| l.kept && !l.withheld && !listed(l.version)) {
        // (round 5) kept versions that did not change the question are listed on asking, also with none no longer kept
        ui.add_space(4.0);
        if kit::ghost_button(ui, "Show every version").clicked() {
            act = Act::ShowAll(true);
        }
    }
    act
}

/// (round 4, critic P-6) One version's whole question: each field and its value (the key and explanation hidden in the
/// viewers' form), the picture named.
fn preview_panel(ui: &mut Ui, version: u64, side: Option<&Shown>, hidden: bool) {
    kit::card().inner_margin(Margin::same(12)).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(theme::strong(format!("Version {version}, as it was")).size(13.0));
        match side {
            None => line(ui, format!("Version {version} does not have this question.")),
            Some(s) => question_fields(ui, s, hidden),
        }
    });
}

/// Every field of one side's question, once. (round 5) Nothing is compared here: a picture is "a picture" or "no picture",
/// and what the viewers' form does not show is "hidden from viewers".
fn question_fields(ui: &mut Ui, s: &Shown, hidden: bool) {
    for f in compare(Some(s), None, hidden).fields {
        let v = if f.hidden { "hidden from viewers".to_string() } else { f.left };
        ui.add(egui::Label::new(theme::text(format!("{}: {v}", f.field)).size(13.0)).wrap());
    }
}

fn restore_panel(ui: &mut Ui, p: &QuestionPage, r: &RestoreView, act: &mut Act) {
    let what = question_words(p.number);
    let back_new = p.comes_back_new(None);
    let mut yes: Option<egui::Response> = None;
    let mut ready = false;
    let resp = kit::card().inner_margin(Margin::same(12)).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        match &r.state {
            RestoreState::Asking => {
                let mut ask = format!("Replace your copy of {what} with version {}'s? Only your copy changes; the circle gets it when you send. A restore point is saved first.", r.version);
                // (critic P2) unsent edits are named before they go; (round 4, minor) also where they can not be counted
                match p.your_copy {
                    YourCopy::Unsent => ask.push_str(&format!(" Your unsent changes to {what} will be replaced.")),
                    YourCopy::Unknown | YourCopy::NotCounted(_) => ask.push_str(&format!(" Any changes to {what} you have not sent will be replaced.")),
                    _ => {}
                }
                // (round 5, minor) a question deleted from the copy comes back at its end
                if p.number == 0 {
                    ask.push_str(" It comes back at the end of the bank.");
                }
                // (critic P3) a question the circle removed
                if back_new {
                    ask.push_str(&format!(" {BACK_NEW}"));
                }
                ui.add(egui::Label::new(theme::text(ask).size(13.0)).wrap());
                // (round 5, critic I3) a question of the bank open in the editor holds it back
                if p.draft_open {
                    line(ui, format!("{DRAFT_OPEN}: its draft would be saved over the restored question."));
                }
                // (round 4, critic P-6) what comes in; (round 5, critic I2) or why it could not be read, with Try again
                match (p.incoming.as_ref().filter(|(v, _)| *v == r.version), p.incoming_err.as_ref().filter(|(v, _, _)| *v == r.version)) {
                    (Some((_, stem)), _) => {
                        ready = true;
                        line(ui, format!("Version {}'s question: \"{stem}\"", r.version));
                    }
                    (None, Some((_, m, retry))) => {
                        warn_box(ui, m);
                        if *retry && kit::ghost_button(ui, "Try again").clicked() {
                            *act = Act::RetryIncoming;
                        }
                    }
                    (None, None) => spinner_line(ui, &format!("Reading version {}...", r.version)),
                }
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    // (round 5, critic I2) Yes waits for the version (and for the editor's question to be closed)
                    let on = ready && !p.draft_open;
                    let y = ui.add_enabled(on, egui::Button::new(egui::RichText::new("Yes, restore").family(theme::bold_family()).color(theme::on_accent_col())).fill(theme::accent_col()).stroke(egui::Stroke::NONE).corner_radius(egui::CornerRadius::same(17)).min_size(egui::vec2(0.0, 34.0)));
                    if y.clicked() {
                        *act = Act::ConfirmRestore;
                    }
                    yes = Some(y);
                    if kit::ghost_button(ui, "Cancel").clicked() {
                        *act = Act::CancelRestore;
                    }
                });
            }
            RestoreState::Reading => spinner_line(ui, &format!("Reading version {}...", r.version)),
            RestoreState::Done(t) => {
                ui.add(egui::Label::new(theme::text(t.as_str()).size(13.0)).wrap());
                ui.horizontal_wrapped(|ui| {
                    if p.undo.is_some() && p.restore_ok && kit::ghost_button(ui, "Undo restore").clicked() {
                        *act = Act::Undo;
                    }
                    if kit::ghost_button(ui, "Dismiss").clicked() {
                        *act = Act::DismissRestore;
                    }
                });
            }
            RestoreState::Failed(t) | RestoreState::UndoFailed(t) => {
                warn_box(ui, &nothing_changed(t));
                ui.horizontal_wrapped(|ui| {
                    // (round 5, critic I4) Try again only where it can help, and for an undo it undoes
                    if p.fail_retry && kit::ghost_button(ui, "Try again").clicked() {
                        *act = if matches!(r.state, RestoreState::UndoFailed(_)) { Act::Undo } else { Act::RetryRestore };
                    }
                    if kit::ghost_button(ui, "Dismiss").clicked() {
                        *act = Act::DismissRestore;
                    }
                });
            }
        }
    });
    // (critic P6) brought into view once, when it is first drawn in this state; (round 4, minor) the question's Yes takes
    // the keyboard focus once it can be pressed
    let key = ui.make_persistent_id(("qh_restore_seen", r.version, std::mem::discriminant(&r.state), ready));
    if !ui.data(|d| d.get_temp::<bool>(key)).unwrap_or(false) {
        ui.data_mut(|d| d.insert_temp(key, true));
        resp.response.scroll_to_me(None);
        if let Some(y) = yes.as_ref().filter(|_| ready) {
            y.request_focus();
        }
    }
    ui.add_space(8.0);
}

/// The sides a picker offers: every kept, readable version, newest first (critic P13, whatever order the lines came in),
/// and "Your copy".
#[cfg(test)]
pub(super) fn picker_sides(p: &QuestionPage) -> Vec<Side> {
    sides_of(p)
}

fn sides_of(p: &QuestionPage) -> Vec<Side> {
    let mut vs: Vec<u64> = p.lines.iter().filter(|l| l.kept && !l.withheld).map(|l| l.version).collect();
    vs.sort_unstable_by(|a, b| b.cmp(a));
    vs.dedup();
    let mut v: Vec<Side> = vs.into_iter().map(Side::Version).collect();
    if p.copy.is_some() {
        v.push(Side::YourCopy);
    }
    v
}

fn picker(ui: &mut Ui, id: &str, label: &str, now: Side, all: &[Side]) -> Option<Side> {
    let mut chosen = None;
    ui.label(theme::muted(label).size(12.5));
    egui::ComboBox::from_id_salt(("qh_picker", id)).selected_text(side_name(now)).show_ui(ui, |ui| {
        for s in all {
            if ui.selectable_label(*s == now, side_name(*s)).clicked() && *s != now {
                chosen = Some(*s);
            }
        }
    });
    chosen
}

/// "Points, topics: the same" (plan 4.6).
fn folded(names: &[&str], what: &str) -> String {
    let list: Vec<String> = names.iter().enumerate().map(|(i, n)| if i == 0 { n.to_string() } else { n.to_lowercase() }).collect();
    format!("{}: {what}", list.join(", "))
}

/// A changed field's cell: the left accent bar, its name and "changed", then `body`.
fn field_cell(ui: &mut Ui, field: &str, body: &mut dyn FnMut(&mut Ui)) {
    let r = egui::Frame::new().inner_margin(Margin { left: 10, right: 4, top: 4, bottom: 4 }).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        text_row(ui, |ui| {
            ui.label(theme::strong(field).size(13.0));
            pill(ui, "changed", theme::accent_faded(), theme::text_col());
        });
        body(ui);
    });
    let rect = r.response.rect;
    ui.painter().rect_filled(egui::Rect::from_min_max(rect.min, egui::pos2(rect.min.x + 3.0, rect.max.y)), 1.0, theme::accent_col());
}

fn compare_panel(ui: &mut Ui, p: &QuestionPage, c: &CompareView, act: &mut Act) {
    let all = sides_of(p);
    ui.label(theme::strong("Compare").size(15.0));
    ui.horizontal_wrapped(|ui| {
        if let Some(s) = picker(ui, "left", "From", c.left, &all) {
            *act = Act::SetLeft(s);
        }
        if let Some(s) = picker(ui, "right", "To", c.right, &all) {
            *act = Act::SetRight(s);
        }
        if kit::ghost_button(ui, "Close compare").clicked() {
            *act = Act::CloseCompare;
        }
    });
    // (critic P8) a held-back version between the two sides
    if let (Side::Version(a), Side::Version(b)) = (c.left, c.right) {
        let held = held_between(p, a, b);
        if !held.is_empty() {
            let v: Vec<String> = held.iter().map(u64::to_string).collect();
            line(ui, format!("This includes version {}, held back for review.", v.join(", ")));
        }
    }
    ui.add_space(6.0);
    let cmp = match &c.state {
        CompareState::Reading => {
            let reads: Vec<String> = [c.left, c.right].iter().filter(|s| matches!(s, Side::Version(_))).map(|s| side_name(*s).to_lowercase()).collect();
            spinner_line(ui, &format!("Reading {}...", reads.join(" and ")));
            return;
        }
        CompareState::Failed(m) => {
            warn_box(ui, m);
            return;
        }
        CompareState::Ready(cmp) => cmp,
    };
    // (round 4, critic P-9) one side does not have the question: said once, and the other side's question shown once
    if cmp.left_absent || cmp.right_absent {
        let (absent, present) = if cmp.left_absent { (c.left, c.right) } else { (c.right, c.left) };
        line(ui, format!("{} does not have this question.", side_name(absent)));
        if !(cmp.left_absent && cmp.right_absent) {
            kit::card().inner_margin(Margin::same(12)).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(theme::strong(format!("{}:", side_name(present))).size(13.0));
                for f in &cmp.fields {
                    let v = if cmp.left_absent { &f.right } else { &f.left };
                    ui.add(egui::Label::new(theme::text(format!("{}: {v}", f.field)).size(13.0)).wrap());
                }
            });
        }
        return;
    }
    let changed: Vec<&FieldRow> = cmp.fields.iter().filter(|f| f.changed).collect();
    let same = cmp.same_fields();
    let hidden = cmp.hidden_fields();
    let columns = compare_columns(ui.available_width());
    if changed.is_empty() {
        para(ui, &format!("The two are the same. {}.", folded(&same, "the same")));
    } else if columns {
        ui.columns(2, |cols| {
            cols[0].label(theme::strong(side_name(c.left)).size(13.5));
            cols[1].label(theme::strong(side_name(c.right)).size(13.5));
        });
        // (critic P12) each column carries the field's name, "changed" and the bar
        for f in &changed {
            ui.columns(2, |cols| {
                field_cell(&mut cols[0], f.field, &mut |ui| {
                    ui.add(egui::Label::new(theme::text(f.left.as_str()).size(13.0)).wrap());
                });
                field_cell(&mut cols[1], f.field, &mut |ui| {
                    ui.add(egui::Label::new(theme::text(f.right.as_str()).size(13.0)).wrap());
                });
            });
            ui.add_space(4.0);
        }
        if !same.is_empty() {
            line(ui, folded(&same, "the same"));
        }
    } else {
        // stacked (critic P12): the field once, both sides under it
        for f in &changed {
            field_cell(ui, f.field, &mut |ui| {
                ui.add(egui::Label::new(theme::text(format!("{}: {}", side_name(c.left), f.left)).size(13.0)).wrap());
                ui.add(egui::Label::new(theme::text(format!("{}: {}", side_name(c.right), f.right)).size(13.0)).wrap());
            });
            ui.add_space(4.0);
        }
        if !same.is_empty() {
            line(ui, folded(&same, "the same"));
        }
    }
    // (round 4, critic P-8) what the viewers' form does not show is named so, never "the same"
    if !hidden.is_empty() {
        line(ui, folded(&hidden, "hidden from viewers, not compared"));
    }
}

/// Hook H3 (approved by the owner): the question card's right-click menu item "History in the circle". Draws nothing for
/// a bank with no link, and reads nothing: the page reads when it opens. (round 4, critic S-I3) While a question of this bank
/// is open in the editor, the page does not open (its draft would later be written over a restored question).
pub fn question_menu(ui: &mut Ui, app: &mut crate::App, bank_id: &str, question_id: &str) {
    let draft_open = app.qedit.as_ref().is_some_and(|d| d.bank_id == bank_id && d.qid.is_some());
    let crate::App { shared, store, .. } = app;
    let Some(bank) = store.banks.iter().find(|b| b.id == bank_id) else { return };
    if question_menu_of(ui, &mut shared.circles, bank, question_id, draft_open) {
        ui.close();
    }
}

/// The words beside the item while a question of the bank is open in the editor.
pub const DRAFT_OPEN: &str = "Finish or close the open question first";

/// [`question_menu`] with its parts (tests). True when the page opened.
pub(super) fn question_menu_of(ui: &mut Ui, c: &mut CirclesModel, bank: &QuestionBank, question_id: &str, draft_open: bool) -> bool {
    if c.edit.with_link(&bank.id, |_| ()).is_none() {
        return false;
    }
    if draft_open {
        ui.add_enabled(false, egui::Button::new("History in the circle")).on_disabled_hover_text(DRAFT_OPEN);
        ui.label(theme::muted(DRAFT_OPEN).size(12.0));
        return false;
    }
    ui.button("History in the circle").clicked() && c.edit.open_question_history(bank, question_id)
}
