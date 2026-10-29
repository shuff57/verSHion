//! Phase D2 (plan 4.5, 4.6, 4.8): one question's history in its circle, drawn on the bank's page in place of the
//! questions, as the bank's History is (`EditView::Question`, hosted by `bank_links_ui::strip`). It opens only from the
//! question card's menu ("History in the circle", hook H3 in `bankui::draw_card`) and reads nothing before.
//!
//! The page runs its own reads (history lines, change lists, version bodies, a picture) on the circle model's runner and
//! keeps them in the session's [`HistoryCache`] (`CircleEditModel::question_cache`, dropped on sign-out and account switch).
//! The bank changes only in [`sync_page`], called from `CircleEditModel::sync`: a restore of one question goes through
//! [`restore_question`] (restore point first). Server text is drawn through `api::plain`.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use egui::{Margin, Ui};

use super::api::{self, ApiError};
use super::browse::{Problem, SharedImages, SharedRestore};
use super::circle_edit::EditView;
use super::circle_links::{CircleLink, sha_of_file};
use super::circle_versions_api::{ChangesPage, VersionLine, VersionsPage};
use super::circles::CirclesModel;
use super::circles_api::BankDoc;
use super::circles_text as words;
use super::circles_ui::{back_link, line, pill, text_row};
use super::model::Runner;
use super::pool::{LocalProblem, Pool, PoolError, RetryPolicy};
use super::publish::SharedSession;
use super::publish_ui::{para, warn_box};
use super::question_history::*;
use crate::model::{ImageRef, Kind, QuestionBank};
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
}

#[derive(Clone, Debug, PartialEq)]
pub struct RestoreView {
    pub version: u64,
    pub state: RestoreState,
}

enum Work {
    Load { cid: String, bid: String, cached: Option<ChangesPage>, lines: bool },
    Body { cid: String, bid: String, n: u64 },
    Picture { cid: String, bid: String, sha: String },
}

enum Msg {
    Load(Result<(Option<VersionsPage>, ChangesPage), PoolError>),
    Body(u64, Box<Result<BankDoc, PoolError>>),
    Picture(String, Result<Option<ImageRef>, PoolError>),
}

pub struct QuestionPage {
    pub cid: String,
    pub bid: String,
    pub local_bank_id: String,
    /// The question in the bank (a restored question that came back new has a new one).
    pub local_id: String,
    pub qid: Option<String>,
    /// "Question 5 · Multiple choice · "Solve 2x + 3 = 7"".
    pub heading: String,
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
    copy: Option<Shown>,
    /// The newest version the link knew when the lines were read (the lines are kept per it).
    head_key: u64,
    reading: Vec<u64>,
    picture: Option<(String, Option<ImageRef>)>,
    picture_asked: bool,
    lines_again: bool,
    env: Env,
    jobs: Vec<Receiver<Msg>>,
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
    let p = Problem { message: e.message(), retry: matches!(e, PoolError::Api(_)), retry_at: None, account: false };
    let retry = p.retry || matches!(e, PoolError::Refused(_));
    (words::circle_words(&p.message), retry)
}

fn side_name(s: Side) -> String {
    match s {
        Side::Version(n) => format!("Version {n}"),
        Side::YourCopy => "Your copy".into(),
    }
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
            copy: side_of_copy(bank, question_id),
            head_key: link.head_seen.max(link.base_version),
            reading: Vec::new(),
            picture: None,
            picture_asked: false,
            lines_again: false,
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
        let (retry, repaint, images) = (self.env.retry, self.env.repaint.clone(), self.env.images.clone());
        match self.env.runner {
            #[cfg(not(target_arch = "wasm32"))]
            Runner::Inline => run(session, images, retry, work, tx, repaint),
            #[cfg(not(target_arch = "wasm32"))]
            Runner::Background => {
                std::thread::spawn(move || run(session, images, retry, work, tx, repaint));
            }
            #[cfg(target_arch = "wasm32")]
            _ => wasm_bindgen_futures::spawn_local(async move { run_async(session, images, retry, work, tx, repaint).await }),
        }
        self.jobs.push(rx);
        true
    }

    /// The first read, or Try again: the change lists (with the tag of the ones read before: a 304 is free) and, when this
    /// session has none for the bank's newest version, the history lines.
    pub fn reload(&mut self, cache: &mut HistoryCache) {
        if self.qid.is_none() {
            return;
        }
        let lines = cache.lines(&self.cid, &self.bid, self.head_key).map(<[VersionLine]>::to_vec);
        let need_lines = lines.is_none() || self.lines_again;
        if let Some(l) = lines.filter(|_| !self.lines_again) {
            self.lines = l;
        }
        let cached = cache.page(&self.cid, &self.bid).cloned();
        self.load = Load::Loading;
        if !self.launch(Work::Load { cid: self.cid.clone(), bid: self.bid.clone(), cached, lines: need_lines }) {
            self.load = Load::SignedOut;
        }
    }

    fn on_load(&mut self, r: Result<(Option<VersionsPage>, ChangesPage), PoolError>, cache: &mut HistoryCache) {
        match r {
            Ok((vp, page)) => {
                let read_lines = vp.is_some();
                if let Some(vp) = vp {
                    self.lines = vp.versions;
                    cache.put_lines(&self.cid, &self.bid, self.head_key, self.lines.clone());
                }
                cache.put_page(&self.cid, &self.bid, page.clone());
                // lines kept from earlier this session that end below the circle's newest version: read once more
                let newest = self.lines.iter().map(|l| l.version).max().unwrap_or(0);
                if !read_lines && newest < page.head_version && !self.lines_again {
                    self.lines_again = true;
                    self.reload(cache);
                    return;
                }
                self.lines_again = false;
                self.page = Some(page);
                self.load = Load::Ready;
            }
            Err(PoolError::Api(ApiError::SignedOut)) => self.load = Load::SignedOut,
            Err(e) => {
                let (m, retry) = problem_words(&e);
                self.load = Load::Failed(m, retry);
            }
        }
    }

    fn on_body(&mut self, n: u64, r: Result<BankDoc, PoolError>, cache: &mut HistoryCache, now: i64) {
        self.reading.retain(|v| *v != n);
        let failed = match r {
            Ok(doc) => {
                let body = VersionBody::from_payload(n, &doc.payload, doc.bank.keys_hidden, now);
                if super::circle_links::payload_question_count(&doc.payload) > body.questions.len() {
                    Some(PoolError::Local(LocalProblem::NewerQuestions).message())
                } else {
                    cache.put_body(&self.cid, &self.bid, body);
                    None
                }
            }
            Err(PoolError::Api(ApiError::SignedOut)) => Some("Not signed in. Sign in, then try again.".into()),
            Err(e) => Some(problem_words(&e).0),
        };
        let Some(m) = failed else { return };
        if let Some(c) = self.compare.as_mut().filter(|c| c.state == CompareState::Reading && (c.left == Side::Version(n) || c.right == Side::Version(n))) {
            c.state = CompareState::Failed(m.clone());
        }
        if let Some(r) = self.restore.as_mut().filter(|r| r.version == n && r.state == RestoreState::Reading) {
            r.state = RestoreState::Failed(m);
        }
    }

    fn poll(&mut self, cache: &mut HistoryCache, now: i64) {
        let mut got = Vec::new();
        self.jobs.retain(|rx| match rx.try_recv() {
            Ok(m) => {
                got.push(m);
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => false,
        });
        for m in got {
            match m {
                Msg::Load(r) => self.on_load(r, cache),
                Msg::Body(n, r) => self.on_body(n, *r, cache, now),
                Msg::Picture(sha, r) => self.picture = Some((sha, r.ok().flatten())),
            }
        }
    }

    pub fn busy(&self) -> bool {
        !self.jobs.is_empty()
    }

    /// Reads the versions `sides` need that this session does not hold. False when one can not be read here.
    fn read_sides(&mut self, sides: &[Side], cache: &mut HistoryCache) -> Result<(), String> {
        let need = reads_needed(&self.lines, cache, &self.cid, &self.bid, sides).map_err(|e| match e {
            NotReadable::Withheld(n) => format!("Version {n} is held back for review, so it can not be compared."),
            NotReadable::NotKept(n) => format!("Version {n} is no longer kept in full, so it can not be compared."),
        })?;
        for n in need {
            if self.reading.contains(&n) {
                continue;
            }
            if !self.launch(Work::Body { cid: self.cid.clone(), bid: self.bid.clone(), n }) {
                return Err("Not signed in. Sign in, then try again.".into());
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
        let Some(c) = self.compare.as_mut().filter(|c| c.state == CompareState::Reading) else { return };
        let Some(qid) = self.qid.clone() else { return };
        let mut hidden = false;
        let mut side = |s: Side| -> Option<Option<Shown>> {
            match s {
                Side::YourCopy => Some(self.copy.clone()),
                Side::Version(n) => cache.body(&self.cid, &self.bid, n).map(|b| {
                    hidden |= b.keys_hidden;
                    side_of_version(&b, &qid)
                }),
            }
        };
        let (Some(l), Some(r)) = (side(c.left), side(c.right)) else { return };
        c.state = CompareState::Ready(compare(l.as_ref(), r.as_ref(), hidden));
    }

    /// "Restore...": the question is asked first; nothing is read or changed. Only on an edit link.
    pub fn ask_restore(&mut self, version: u64) {
        if self.restore_ok {
            self.restore = Some(RestoreView { version, state: RestoreState::Asking });
        }
    }

    pub fn cancel_restore(&mut self) {
        self.restore = None;
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
        let state = match self.read_sides(&[Side::Version(v)], cache) {
            Ok(()) => RestoreState::Reading,
            Err(m) => RestoreState::Failed(m),
        };
        self.restore = Some(RestoreView { version: v, state });
    }

    fn refresh(&mut self, link: &CircleLink, bank: &QuestionBank, cache: &HistoryCache) {
        let (number, heading) = heading_of(bank, &self.local_id);
        self.number = number;
        self.heading = heading;
        self.your_copy = your_copy(link, bank, &self.local_id);
        self.restore_ok = restore_offered(link);
        self.base_version = link.base_version;
        self.copy = side_of_copy(bank, &self.local_id);
        self.timeline = self.page.as_ref().filter(|_| self.load == Load::Ready).map(|page| timeline(&TimelineIn { lines: &self.lines, changes: page, link, bank, local_id: &self.local_id, show_all: self.show_all }));
        let _ = cache;
    }

    /// The restore, once its version (and picture) is here: [`restore_question`] writes the restore point, then the banks.
    fn restore_step(&mut self, cache: &mut HistoryCache, banks: &mut [QuestionBank], link: &CircleLink, save: &dyn Fn(&[QuestionBank]) -> Result<(), String>, now: i64) -> Option<(String, Option<CircleLink>)> {
        let v = self.restore.as_ref().filter(|r| r.state == RestoreState::Reading)?.version;
        let qid = self.qid.clone()?;
        let body = cache.body(&self.cid, &self.bid, v)?;
        let fail = |p: &mut QuestionPage, m: String| {
            p.restore = Some(RestoreView { version: v, state: RestoreState::Failed(m) });
            None
        };
        let Some(t) = body.question(&qid) else { return fail(self, format!("Version {v} does not have this question. Nothing was changed.")) };
        // its picture, unless the copy has it already: fetched by its hash, and nothing is written without it
        let have = banks.iter().find(|b| b.id == self.local_bank_id).and_then(|b| b.questions.iter().find(|q| q.id == self.local_id)).and_then(|q| q.image.as_ref()).and_then(|i| sha_of_file(&i.file));
        let mut picture = None;
        if let Some(sha) = t.sha.clone().filter(|s| have.as_deref() != Some(s.as_str())) {
            match &self.picture {
                Some((s, got)) if *s == sha => picture = got.clone(),
                _ if !self.picture_asked => {
                    self.picture_asked = true;
                    if !self.launch(Work::Picture { cid: self.cid.clone(), bid: self.bid.clone(), sha }) {
                        return fail(self, "Not signed in. Sign in, then try again.".into());
                    }
                    return None;
                }
                _ => return None,
            }
        }
        let removed = self.page.as_ref().and_then(|p| removed_at_head(p, &qid)) == Some(true);
        let ask = RestoreAsk { link, local_id: Some(&self.local_id), qid: &qid, body: &body, picture, removed_in_circle: removed };
        match restore_question(banks, &ask, &*self.env.restore, save, now) {
            Ok(done) => {
                self.local_id = done.local_id.clone();
                let n = self.number.max(1);
                let text = format!("Question {n} is now as it was in version {v}. Only your copy changed: it counts as an unsent change until you send. A restore point was saved first: {}.", done.restore_point);
                self.restore = Some(RestoreView { version: v, state: RestoreState::Done(text) });
                Some((self.local_bank_id.clone(), done.link))
            }
            Err(e) => fail(self, refused_words(&e, v)),
        }
    }
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
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn run(session: SharedSession, images: SharedImages, retry: RetryPolicy, work: Work, tx: Sender<Msg>, repaint: Option<Arc<dyn Fn() + Send + Sync>>) {
    super::transport::block_on(run_async(session, images, retry, work, tx, repaint));
}

async fn run_async(session: SharedSession, images: SharedImages, retry: RetryPolicy, work: Work, tx: Sender<Msg>, repaint: Option<Arc<dyn Fn() + Send + Sync>>) {
    let pool = Pool::new(&*session).with_retry(retry);
    let msg = match work {
        Work::Load { cid, bid, cached, lines } => {
            let r = async {
                let vp = if lines { Some(pool.circle_versions(&cid, &bid, None).await?) } else { None };
                let page = pool.circle_changes_if(&cid, &bid, cached.as_ref()).await?;
                Ok((vp, page))
            }
            .await;
            Msg::Load(r)
        }
        Work::Body { cid, bid, n } => Msg::Body(n, Box::new(pool.circle_version(&cid, &bid, n).await)),
        Work::Picture { cid, bid, sha } => {
            let r = fetch_picture(&pool, &cid, &bid, &sha, &*images).await;
            Msg::Picture(sha, r)
        }
    };
    let _ = tx.send(msg);
    if let Some(r) = repaint {
        r();
    }
}

/// From `CircleEditModel::sync`: the page's answers, the bank and link it draws, and a restore waiting for its version.
/// Returns the bank's id (and its link to keep, when the question no longer pairs with the circle's) when the bank changed.
pub(super) fn sync_page(p: &mut QuestionPage, cache: &mut HistoryCache, banks: &mut [QuestionBank], links: &[CircleLink], session: Option<SharedSession>, save: &dyn Fn(&[QuestionBank]) -> Result<(), String>, now: i64) -> Option<(String, Option<CircleLink>)> {
    p.env.session = session;
    p.poll(cache, now);
    let link = links.iter().find(|l| l.local_bank_id == p.local_bank_id)?;
    let bank = banks.iter().find(|b| b.id == p.local_bank_id)?;
    p.refresh(link, bank, cache);
    p.refresh_compare(cache);
    let out = p.restore_step(cache, banks, link, save, now);
    if out.is_some()
        && let Some(bank) = banks.iter().find(|b| b.id == p.local_bank_id)
    {
        p.refresh(link, bank, cache);
    }
    out
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
        Act::CancelRestore | Act::DismissRestore => p.cancel_restore(),
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

fn draw_question(ui: &mut Ui, p: &QuestionPage, back: &str, now: i64) -> Act {
    let mut act = Act::None;
    if back_link(ui, back) {
        act = Act::Back;
    }
    ui.add_space(4.0);
    ui.add(egui::Label::new(theme::serif("History in the circle").size(20.0)).wrap());
    ui.add(egui::Label::new(theme::strong(p.heading.as_str()).size(14.0)).wrap());
    ui.add_space(4.0);
    // "Your copy: ..." and Compare with...
    let compare_ok = p.load == Load::Ready && p.copy.is_some() && p.lines.iter().any(|l| l.kept && !l.withheld);
    text_row(ui, |ui| {
        ui.add(egui::Label::new(theme::text(your_copy_text(p.your_copy)).size(13.0)).wrap());
        if compare_ok && p.compare.is_none() && kit::ghost_button(ui, "Compare with...").clicked() {
            let base = Side::Version(p.base_version);
            let left = if p.lines.iter().any(|l| l.version == p.base_version && l.kept && !l.withheld) { base } else { p.lines.iter().find(|l| l.kept && !l.withheld).map_or(base, |l| Side::Version(l.version)) };
            act = Act::Compare(left, Side::YourCopy);
        }
    });
    ui.add_space(6.0);
    if let Some(r) = &p.restore {
        restore_panel(ui, p, r, &mut act);
    }
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
        compare_panel(ui, p, c, &mut act);
        return act;
    }
    let Some(t) = &p.timeline else { return act };
    if !t.per_question {
        para(ui, "This bank hides its answer keys from viewers, so this list shows the bank's versions, not which questions changed. Compare shows two versions as viewers see them.");
    }
    let narrow = ui.available_width() < NARROW;
    if t.rows.is_empty() || (t.per_question && !t.rows.iter().any(|r| matches!(r, Row::Version { .. }))) {
        line(ui, "No version that is still kept changed this question.");
    }
    for row in &t.rows {
        match row {
            Row::Version { version, line: l, tag, compared_to, compare, restore } => {
                kit::card().inner_margin(Margin::symmetric(10, 8)).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    let mut head = vec![format!("Version {version}")];
                    head.extend(who_when(l.as_ref(), now));
                    let msg = message_of(l.as_ref());
                    let buttons = |ui: &mut Ui, act: &mut Act| {
                        if let Some(t) = tag {
                            pill(ui, tag_word(*t), if *t == Tag::AsItWas { theme::accent_faded() } else { theme::accent_col() }, if *t == Tag::AsItWas { theme::text_col() } else { theme::on_accent_col() });
                        }
                        if *compare && kit::ghost_button(ui, "Compare").clicked() {
                            let left = compared_to.map(Side::Version).unwrap_or(Side::Version(*version));
                            let right = if compared_to.is_some() { Side::Version(*version) } else { Side::YourCopy };
                            *act = Act::Compare(left, right);
                        }
                        if *restore && kit::ghost_button(ui, "Restore...").clicked() {
                            *act = Act::AskRestore(*version);
                        }
                    };
                    if narrow {
                        ui.add(egui::Label::new(theme::strong(head.join(" · ")).size(13.0)).wrap());
                        ui.horizontal_wrapped(|ui| {
                            if let Some(m) = &msg {
                                ui.add(egui::Label::new(theme::muted(m.as_str()).size(12.5)).wrap());
                            }
                            buttons(ui, &mut act);
                        });
                    } else {
                        text_row(ui, |ui| {
                            let mut all = head.clone();
                            all.extend(msg.clone());
                            ui.add(egui::Label::new(theme::text(all.join(" · ")).size(13.0)).wrap());
                            buttons(ui, &mut act);
                        });
                    }
                });
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
                ui.add(egui::Label::new(theme::muted(gap_text(g)).size(12.5).italics()).wrap());
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
    }
    act
}

fn restore_panel(ui: &mut Ui, p: &QuestionPage, r: &RestoreView, act: &mut Act) {
    let n = p.number.max(1);
    kit::card().inner_margin(Margin::same(12)).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        match &r.state {
            RestoreState::Asking => {
                ui.add(egui::Label::new(theme::text(format!("Replace your copy of question {n} with version {}'s? Only your copy changes; the circle gets it when you send. A restore point is saved first.", r.version)).size(13.0)).wrap());
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    if kit::primary_button(ui, "Yes, restore").clicked() {
                        *act = Act::ConfirmRestore;
                    }
                    if kit::ghost_button(ui, "Cancel").clicked() {
                        *act = Act::CancelRestore;
                    }
                });
            }
            RestoreState::Reading => spinner_line(ui, &format!("Reading version {}...", r.version)),
            RestoreState::Done(t) => {
                text_row(ui, |ui| {
                    ui.add(egui::Label::new(theme::text(t.as_str()).size(13.0)).wrap());
                    if kit::ghost_button(ui, "Dismiss").clicked() {
                        *act = Act::DismissRestore;
                    }
                });
            }
            RestoreState::Failed(t) => {
                warn_box(ui, t);
                if kit::ghost_button(ui, "Dismiss").clicked() {
                    *act = Act::DismissRestore;
                }
            }
        }
    });
    ui.add_space(8.0);
}

/// The sides a picker offers: every kept, readable version, newest first, and "Your copy".
fn sides_of(p: &QuestionPage) -> Vec<Side> {
    let mut v: Vec<Side> = p.lines.iter().filter(|l| l.kept && !l.withheld).map(|l| Side::Version(l.version)).collect();
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
fn folded(names: &[&str]) -> String {
    let list: Vec<String> = names.iter().enumerate().map(|(i, n)| if i == 0 { n.to_string() } else { n.to_lowercase() }).collect();
    format!("{}: the same", list.join(", "))
}

/// A changed field: the left accent bar, its name and "changed".
fn changed_field(ui: &mut Ui, f: &FieldRow, columns: bool, left_only: Option<bool>) {
    let r = egui::Frame::new().inner_margin(Margin { left: 10, right: 4, top: 4, bottom: 4 }).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        text_row(ui, |ui| {
            ui.label(theme::strong(f.field).size(13.0));
            pill(ui, "changed", theme::accent_faded(), theme::text_col());
        });
        if columns {
            ui.columns(2, |cols| {
                cols[0].add(egui::Label::new(theme::text(f.left.as_str()).size(13.0)).wrap());
                cols[1].add(egui::Label::new(theme::text(f.right.as_str()).size(13.0)).wrap());
            });
        } else {
            let t = if left_only == Some(true) { &f.left } else { &f.right };
            ui.add(egui::Label::new(theme::text(t.as_str()).size(13.0)).wrap());
        }
    });
    let rect = r.response.rect;
    ui.painter().rect_filled(egui::Rect::from_min_max(rect.min, egui::pos2(rect.min.x + 3.0, rect.max.y)), 1.0, theme::accent_col());
    ui.add_space(4.0);
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
    for (absent, s) in [(cmp.left_absent, c.left), (cmp.right_absent, c.right)] {
        if absent {
            line(ui, format!("{} does not have this question.", side_name(s)));
        }
    }
    let changed: Vec<&FieldRow> = cmp.fields.iter().filter(|f| f.changed).collect();
    let same = cmp.same_fields();
    let columns = compare_columns(ui.available_width());
    if changed.is_empty() {
        para(ui, &format!("The two are the same. {}.", folded(&same)));
        return;
    }
    if columns {
        ui.columns(2, |cols| {
            cols[0].label(theme::strong(side_name(c.left)).size(13.5));
            cols[1].label(theme::strong(side_name(c.right)).size(13.5));
        });
        for f in &changed {
            changed_field(ui, f, true, None);
        }
    } else {
        // stacked: the first side's block above the second's
        for (first, s) in [(true, c.left), (false, c.right)] {
            ui.label(theme::strong(side_name(s)).size(13.5));
            for f in &changed {
                changed_field(ui, f, false, Some(first));
            }
            ui.add_space(6.0);
        }
    }
    if !same.is_empty() {
        line(ui, folded(&same));
    }
}

/// Hook H3 (approved by the owner): the question card's right-click menu item "History in the circle". Draws nothing for
/// a bank with no link, and reads nothing: the page reads when it opens.
pub fn question_menu(ui: &mut Ui, app: &mut crate::App, bank_id: &str, question_id: &str) {
    let crate::App { shared, store, .. } = app;
    let Some(bank) = store.banks.iter().find(|b| b.id == bank_id) else { return };
    if question_menu_of(ui, &mut shared.circles, bank, question_id) {
        ui.close();
    }
}

/// [`question_menu`] with its parts (tests). True when the page opened.
pub(super) fn question_menu_of(ui: &mut Ui, c: &mut CirclesModel, bank: &QuestionBank, question_id: &str) -> bool {
    if c.edit.with_link(&bank.id, |_| ()).is_none() {
        return false;
    }
    ui.button("History in the circle").clicked() && c.edit.open_question_history(bank, question_id)
}
