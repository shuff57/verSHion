//! Phase D2 client, round 2 (plan 4.5, 4.6, 4.8, 5.5): the question history page drawn in place of the bank's questions,
//! its compare view and its restore, driven through the bank page's strip as `bankui` drives it, against a wired session
//! that keeps every request's headers. Headless frames at 420 and 1280 px.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::api::ApiError;
use super::bank_links_fix_tests::FakeAccount;
use super::bank_links_tests::{LOCAL, QA, QB, circles_list, heads, hex, linked};
use super::browse::{BrowseModel, ImportLog, RestoreSink};
use super::circle_edit::{CircleEditModel, EditView};
use super::circle_links::{LinkLog, LinkMode};
use super::circles::CirclesModel;
use super::circles_tests::{BID, CID, DAY, Mem, NOW, detail, model};
use super::model::Runner;
use super::pool::*;
use super::publish::{AccountSnapshot, Gate};
use super::question_history::Side;
use super::question_history_ui::*;
use super::transport::{LocalFuture, Request, Response};
use crate::model::QuestionBank;

// ---- a wired session: queued replies by "Method /path", sticky ones, and every request with its headers -------------

/// A request as kept: its "Method /path" and headers.
type Seen = (String, Vec<(String, String)>);

#[derive(Default)]
pub(super) struct Wire {
    routes: Mutex<HashMap<String, VecDeque<Response>>>,
    sticky: Mutex<HashMap<String, Value>>,
    seen: Mutex<Vec<Seen>>,
    /// Who the session says is signed in (round 3, critic I4).
    who: Mutex<Option<String>>,
    /// (round 4) The next request for the named "Method /path" waits here, after taking its reply, until released.
    gate: Mutex<Option<(String, std::sync::mpsc::Receiver<()>)>>,
}

fn reply(status: u16, body: &Value, etag: Option<&str>) -> Response {
    let mut r = Response::new(status, &body.to_string());
    r.bytes = body.to_string().into_bytes();
    r.etag = etag.map(str::to_string);
    r
}

impl Wire {
    fn new() -> Arc<Wire> {
        let w = Arc::new(Wire::default());
        w.keep("Post", "/circles/heads", heads(5));
        w.keep("Get", "/circles", circles_list("editor"));
        w.keep("Get", &format!("/circles/{CID}"), detail("editor", json!({})));
        w
    }
    fn keep(&self, m: &str, path: &str, body: Value) {
        self.sticky.lock().unwrap().insert(format!("{m} {path}"), body);
    }
    fn on(&self, m: &str, path: &str, status: u16, body: Value, etag: Option<&str>) {
        self.routes.lock().unwrap().entry(format!("{m} {path}")).or_default().push_back(reply(status, &body, etag));
    }
    fn raw(&self, path: &str, bytes: Vec<u8>) {
        let mut r = Response::new(200, "");
        r.bytes = bytes;
        self.routes.lock().unwrap().entry(format!("Get {path}")).or_default().push_back(r);
    }
    /// The next request for `key` ("Method /path") waits, after taking its reply, until the sender is used (or dropped).
    fn hold(&self, key: &str) -> std::sync::mpsc::Sender<()> {
        let (tx, rx) = std::sync::mpsc::channel();
        *self.gate.lock().unwrap() = Some((key.to_string(), rx));
        tx
    }
    fn calls(&self, needle: &str) -> usize {
        self.seen.lock().unwrap().iter().filter(|(p, _)| p.contains(needle)).count()
    }
    /// The headers of the last request whose path ends with `tail`.
    fn last_header(&self, tail: &str, name: &str) -> Option<String> {
        let seen = self.seen.lock().unwrap();
        let (_, h) = seen.iter().rev().find(|(p, _)| p.ends_with(tail))?;
        h.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.clone())
    }
    fn reads(&self) -> Vec<String> {
        self.seen.lock().unwrap().iter().map(|(p, _)| p.clone()).filter(|p| p.contains("/changes") || p.contains("/versions")).collect()
    }
}

impl PoolSession for Wire {
    fn base(&self) -> String {
        "https://pool.test".into()
    }
    fn account(&self) -> Option<String> {
        self.who.lock().unwrap().clone()
    }
    fn send<'a>(&'a self, build: &'a dyn Fn() -> Request) -> LocalFuture<'a, Result<Response, ApiError>> {
        Box::pin(async move {
            let r = build();
            let path = r.url.trim_start_matches("https://pool.test").split('?').next().unwrap_or_default().to_string();
            let key = format!("{:?} {}", r.method, path);
            self.seen.lock().unwrap().push((key.clone(), r.headers.iter().map(|(k, v)| (k.clone(), v.expose().to_string())).collect()));
            let queued = self.routes.lock().unwrap().get_mut(&key).and_then(VecDeque::pop_front);
            if let Some(q) = queued {
                let g = {
                    let mut gate = self.gate.lock().unwrap();
                    if gate.as_ref().is_some_and(|(k, _)| *k == key) { gate.take() } else { None }
                };
                if let Some((_, rx)) = g {
                    let _ = rx.recv_timeout(std::time::Duration::from_secs(10));
                }
                return Ok(q);
            }
            match self.sticky.lock().unwrap().get(&key) {
                Some(b) => Ok(reply(200, b, None)),
                None => panic!("no wired reply for {key}; seen {:?}", self.seen.lock().unwrap().iter().map(|(p, _)| p.clone()).collect::<Vec<_>>()),
            }
        })
    }
}

// ---- the bank and its circle ------------------------------------------------------------------------------------------

fn versions_path() -> String {
    format!("/circles/{CID}/banks/{BID}/versions")
}

fn version_path(n: u64) -> String {
    format!("{}/{n}", versions_path())
}

fn changes_path() -> String {
    format!("/circles/{CID}/banks/{BID}/changes")
}

/// Versions 5 to 1: 4 is held back for review, 1 is no longer kept.
fn lines_json() -> Value {
    let kept = |v: u64, by: &str, msg: &str| json!({"version": v, "version_id": hex(v), "by": by, "at": NOW - (6 - v as i64) * DAY, "message": msg, "added": 0, "changed": 1, "removed": 0, "question_count": 2, "kept": true, "withheld": false});
    json!({"bank_id": BID, "head_version": 5, "oldest_kept": 2, "pinned_versions": [], "next_before": null, "versions": [
        kept(5, "B. Teacher", "Fixed a distractor"),
        {"version": 4, "by": "", "at": NOW - 2 * DAY, "message": null, "kept": false, "withheld": true, "review_copy": "full"},
        kept(3, "A. Teacher", "A second question"),
        kept(2, "A. Teacher", "Unit 2 rewrite"),
        {"version": 1, "by": "A. Teacher", "at": NOW - 5 * DAY, "message": "start", "kept": false, "withheld": false},
    ]})
}

/// 5 vs 3 (4 held back) changed A; 3 vs 2 added B.
fn changes_json() -> Value {
    json!({"bank_id": BID, "head_version": 5, "per_question": true, "versions": [
        {"version": 5, "compared_to": 3, "added": [], "changed": [QA], "removed": []},
        {"version": 3, "compared_to": 2, "added": [QB], "changed": [], "removed": []},
    ]})
}

fn qj(id: &str, stem: &str) -> Value {
    json!({"id": id, "stem": stem, "kind": {"Mc": {"answers": [{"text": "A", "correct": true}, {"text": "B", "correct": false}]}}, "points": 1.0, "difficulty": null, "created": 5, "modified": 6})
}

fn served(version: u64, qs: &[Value], keys_hidden: bool) -> Vec<u8> {
    let head = json!({"circle": {"id": CID, "name": "Algebra team"}, "bank": {"id": BID, "title": "Unit 2 quiz", "version": version, "version_id": hex(version), "question_count": qs.len(), "committed_at": NOW - DAY, "hide_key": keys_hidden, "keys_hidden": keys_hidden}, "origins": null});
    let h = head.to_string();
    let p = json!({"schema_version": 2, "bank": {"id": BID, "name": "Unit 2 quiz", "questions": qs, "created": 1, "modified": 2}}).to_string();
    format!("{},\"payload\":{}}}", &h[..h.len() - 1], p).into_bytes()
}

const A5: &str = "Solve 2x + 3 = 7";
const A3: &str = "Solve 2x + 3 = 9";
const B: &str = "Simplify 3(x + 2)";

#[derive(Default)]
struct CountRestore(Mutex<usize>, std::sync::atomic::AtomicBool);

impl RestoreSink for CountRestore {
    fn save(&self, _: &[QuestionBank]) -> Result<String, String> {
        // (round 5) a disk that is full for now
        if self.1.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("the disk is full".into());
        }
        *self.0.lock().unwrap() += 1;
        Ok("memory".into())
    }
}

struct Rig {
    m: CirclesModel,
    w: Arc<Wire>,
    banks: Vec<QuestionBank>,
    acct: FakeAccount,
    ctx: egui::Context,
    restores: Arc<CountRestore>,
    who: String,
    /// Every texture the context sent, for the screenshots.
    painter: Painter,
    pics: Arc<Pics>,
}

fn rig_with(mode: LinkMode, viewers_form: bool) -> Rig {
    rig_link(mode, viewers_form, |_| {})
}

#[derive(Default)]
struct Pics(Mutex<usize>);

impl super::browse::ImageSink for Pics {
    fn put(&self, bytes: &[u8]) -> Result<crate::model::ImageRef, String> {
        *self.0.lock().unwrap() += 1;
        Ok(crate::model::ImageRef { file: format!("media/{}.webp", sha256_hex(bytes)), width: 2, height: 2 })
    }
}

fn rig_link(mode: LinkMode, viewers_form: bool, f: impl FnOnce(&mut super::circle_links::CircleLink)) -> Rig {
    let mut m = model();
    let pics = Arc::new(Pics::default());
    let restores = Arc::new(CountRestore::default());
    let media: super::publish::SharedMedia = Arc::new(HashMap::<String, Vec<u8>>::new());
    m.edit = CircleEditModel::new(Runner::Inline, pics.clone(), media, restores.clone(), LinkLog::memory()).with_retry(RetryPolicy::none());
    m.set_clock(NOW);
    let (mut bank, mut l) = linked(&[(QA, A5), (QB, B)]);
    bank.name = "Unit 2 quiz".into();
    l.base_version = 5;
    l.base_version_id = hex(5);
    l.head_seen = 5;
    l.mode = mode;
    l.viewers_form = viewers_form;
    f(&mut l);
    l.mark_bank(&bank);
    m.edit.add_link(l).unwrap();
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    Rig { m, w: Wire::new(), banks: vec![bank], acct: FakeAccount { open: true, ..FakeAccount::default() }, ctx, restores, who: "olga".into(), painter: Painter { textures: HashMap::new() }, pics }
}

fn rig() -> Rig {
    rig_with(LinkMode::Edit, false)
}

impl Rig {
    fn snap(&self) -> AccountSnapshot {
        AccountSnapshot { account: Some(format!("https://pool.test\n{}@school.org", self.who)), ..AccountSnapshot::ready(self.w.clone()) }
    }

    fn input(&self, width: f32, events: Vec<egui::Event>) -> egui::RawInput {
        let t0 = self.ctx.input(|i| i.time);
        egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 6000.0))), time: Some(t0 + 0.05), events, ..Default::default() }
    }

    /// One frame of the bank's page (the strip, or the page drawn in place of the questions) at `width`; the text, and
    /// where each text was drawn.
    fn frame_at(&mut self, width: f32, events: Vec<egui::Event>) -> Vec<(String, egui::Rect)> {
        let snap = self.snap();
        let input = self.input(width, events);
        let mut browse = BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), ImportLog::memory());
        let (m, banks, acct) = (&mut self.m, &mut self.banks, &mut self.acct);
        let out = self.ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                super::bank_links_ui::strip_with(ui, m, banks, &mut browse, acct, &snap, web_time::Instant::now(), NOW, LOCAL);
            });
        });
        let texts = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) => Some((t.galley.text().replace('\u{a0}', " "), t.galley.rect.translate(t.pos.to_vec2()))),
                _ => None,
            })
            .collect();
        self.painter.apply(&out.textures_delta);
        out.drop_without_applying_deltas();
        texts
    }

    fn page(&mut self, width: f32) -> String {
        self.frame_at(width, Vec::new());
        self.frame_at(width, Vec::new()).into_iter().map(|(t, _)| t).collect::<Vec<_>>().join("\n")
    }

    /// Clicks the `nth` text that reads `label` (a button's words). False when it is not drawn.
    fn click(&mut self, width: f32, label: &str, nth: usize) -> bool {
        self.frame_at(width, Vec::new());
        let hits: Vec<egui::Rect> = self.frame_at(width, Vec::new()).into_iter().filter(|(t, _)| t == label).map(|(_, r)| r).collect();
        let Some(p) = hits.get(nth).map(|r| r.center()) else { return false };
        let press = |pressed| egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        self.frame_at(width, vec![egui::Event::PointerMoved(p), press(true), press(false)]);
        self.frame_at(width, Vec::new());
        true
    }

    /// The history page for the bank's question `i` (0-based), opened as its card's menu opens it.
    fn open(&mut self, i: usize) {
        // (the bank's page is drawn first, as in the app: the model has the session, and its check has said the role)
        self.frame_at(1280.0, Vec::new());
        self.frame_at(1280.0, Vec::new());
        self.open_cold(i);
    }

    /// The same before any frame gave the model a session (signed out).
    fn open_cold(&mut self, i: usize) {
        let bank = self.banks[0].clone();
        let id = bank.questions[i].id.clone();
        let opened = self.m.edit.open_question_history(&bank, &id);
        assert!(opened, "the page opened");
    }

    fn script_load(&self) {
        self.w.on("Get", &versions_path(), 200, lines_json(), None);
        self.w.on("Get", &changes_path(), 200, changes_json(), Some("\"e1\""));
    }

    fn qpage(&self) -> &QuestionPage {
        match &self.m.edit.view {
            EditView::Question(p) => p,
            other => panic!("not on the question's history: {other:?}"),
        }
    }

    fn qpage_mut(&mut self) -> &mut QuestionPage {
        match &mut self.m.edit.view {
            EditView::Question(p) => p,
            other => panic!("not on the question's history: {other:?}"),
        }
    }
}

/// The card's menu item, drawn as the menu draws it (a plain button); the text drawn, and whether it was clicked open.
fn menu_text(m: &mut CirclesModel, bank: &QuestionBank, question_id: &str) -> String {
    super::bank_links_tests::text_of(1280.0, &mut |ui| {
        super::question_history_ui::question_menu_of(ui, m, bank, question_id, false);
    })
}

const BOTH: [f32; 2] = [420.0, 1280.0];

// ---- hook H3: the menu item ---------------------------------------------------------------------------------------------

#[test]
fn d2u_the_menu_item_is_drawn_only_for_a_linked_bank_and_reads_nothing() {
    let mut r = rig();
    let bank = r.banks[0].clone();
    let id = bank.questions[0].id.clone();
    assert!(menu_text(&mut r.m, &bank, &id).contains("History in the circle"));
    // a bank with no link: nothing drawn, nothing asked
    let mut other = bank.clone();
    other.id = "not-linked".into();
    let mut bare = model();
    assert!(!menu_text(&mut bare, &other, &id).contains("History in the circle"));
    assert!(!menu_text(&mut r.m, &other, &id).contains("History in the circle"));
    assert!(!r.m.edit.open_question_history(&other, &id), "an unlinked bank never opens it");
    assert!(r.w.reads().is_empty(), "{:?}", r.w.reads());
    assert!(!r.m.edit.active());
}

#[test]
fn d2u_nothing_is_read_until_the_page_opens() {
    let mut r = rig();
    for w in BOTH {
        let t = r.page(w);
        assert!(!t.contains("History in the circle") || t.contains("Circle"));
    }
    assert!(r.w.reads().is_empty(), "the bank's page alone reads no history: {:?}", r.w.reads());
    r.script_load();
    r.open(0);
    r.page(1280.0);
    assert_eq!((r.w.calls("/changes"), r.w.calls("/versions")), (1, 1), "{:?}", r.w.reads());
}

// ---- the page -------------------------------------------------------------------------------------------------------------

#[test]
fn d2u_the_page_opens_the_right_questions_timeline_in_place_of_the_questions() {
    let mut r = rig();
    r.script_load();
    r.open(0);
    for w in BOTH {
        let t = r.page(w);
        assert!(t.contains("< Back to Unit 2 quiz"), "{t}");
        assert!(t.contains("Question 1 · Multiple choice · \"Solve 2x + 3 = 7\""), "{t}");
        assert!(t.contains("Your copy: the same as version 5."), "{t}");
        assert!(t.contains("Version 5") && t.contains("Changed") && t.contains("Fixed a distractor"), "{t}");
        assert!(t.contains("Changes between version 3 and version 5: version 4 is held back."), "{t}");
        assert!(t.contains("Version 2") && t.contains("As it was then"), "{t}");
        assert!(t.contains("Version 1 is no longer kept in full: who and when only.") && t.contains("Show every version"), "{t}");
        assert!(!t.contains("Added"), "B's addition is not A's history: {t}");
    }
    // Back: the bank's questions again
    assert!(r.click(1280.0, "< Back to Unit 2 quiz", 0));
    assert!(!r.m.edit.active());
    // B: its own question's history, read from the session's cache (nothing read again)
    let before = r.w.reads().len();
    r.w.on("Get", &changes_path(), 304, json!({}), Some("\"e1\""));
    r.open(1);
    let t = r.page(1280.0);
    assert!(t.contains("Question 2 · Multiple choice · \"Simplify 3(x + 2)\"") && t.contains("Added") && !t.contains("As it was then"), "{t}");
    assert_eq!(r.w.reads().len(), before + 1, "only the change lists again: {:?}", r.w.reads());
}

#[test]
fn d2u_show_every_version_lists_the_held_back_and_no_longer_kept_lines() {
    let mut r = rig();
    r.script_load();
    r.open(0);
    for w in BOTH {
        r.page(w);
        if !r.qpage().show_all {
            assert!(r.click(w, "Show every version", 0));
        }
        let t = r.page(w);
        assert!(t.contains("Version 4 · held back for review · no details"), "{t}");
        assert!(t.contains("no longer kept in full") && t.contains("start"), "{t}");
        assert!(t.contains("Version 3") && t.contains("Show fewer versions"), "{t}");
        assert!(r.click(w, "Show fewer versions", 0));
        assert!(!r.qpage().show_all);
    }
}

#[test]
fn d2u_a_question_the_circle_does_not_know_says_so_and_reads_nothing() {
    let mut r = rig();
    let mut q = r.banks[0].questions[0].clone();
    q.id = "new-here".into();
    r.banks[0].questions.push(q);
    r.banks[0].modified += 1;
    let bank = r.banks[0].clone();
    assert!(r.m.edit.open_question_history(&bank, "new-here"));
    for w in BOTH {
        let t = r.page(w);
        assert!(t.contains("The circle doesn't have this question yet"), "{t}");
        assert!(t.contains("Your copy: new here, not sent yet."), "{t}");
    }
    assert!(r.w.reads().is_empty(), "{:?}", r.w.reads());
}

#[test]
fn d2u_signed_out_offline_and_errors_say_what_to_do() {
    // signed out: no session
    let mut r = rig();
    r.open_cold(0);
    // (the session the page was opened with is the model's: none until a frame gave it one)
    assert_eq!(r.qpage().load, Load::SignedOut);
    // (round 3, critic P11) once a session is there, the page reads by itself (twice here: the strip's first check then says
    // the role, and a read made before that is not used under it)
    r.script_load();
    r.script_load();
    r.page(1280.0);
    let t = r.page(1280.0);
    assert!(t.contains("Version 5") && t.contains("Changed"), "{t}");
    // offline, then an error the pool answered: Try again reads again
    let mut r = rig();
    r.w.on("Get", &versions_path(), 500, json!({"error": "internal"}), None);
    r.open(0);
    for w in BOTH {
        let t = r.page(w);
        assert!(t.contains("Try again"), "{t}");
        assert!(!t.contains("Version 5"), "{t}");
    }
    r.script_load();
    assert!(r.click(1280.0, "Try again", 0));
    let t = r.page(1280.0);
    assert!(t.contains("Version 5") && t.contains("Changed"), "{t}");
    // signed out while open (the session said so on a read): says so, with Sign in
    let mut r = rig();
    r.script_load();
    r.open(0);
    r.page(1280.0);
    r.qpage_mut().load = Load::SignedOut;
    for w in BOTH {
        let t = r.page(w);
        assert!(t.contains("Not signed in. Sign in, then press Try again.") && t.contains("Sign in..."), "{t}");
    }
    // loading
    r.qpage_mut().load = Load::Loading;
    for w in BOTH {
        assert!(r.page(w).contains("Reading this question's history..."));
    }
}

#[test]
fn d2u_a_viewer_of_a_hiding_bank_sees_bank_lines_only() {
    let mut r = rig_with(LinkMode::Follow, true);
    r.w.on("Get", &versions_path(), 200, lines_json(), None);
    r.w.on("Get", &changes_path(), 200, json!({"bank_id": BID, "head_version": 5, "per_question": false, "reason": "keys_hidden"}), None);
    r.open(0);
    for w in BOTH {
        let t = r.page(w);
        assert!(t.contains("This bank hides its answer keys from viewers"), "{t}");
        assert!(t.contains("Version 5") && t.contains("Version 3") && t.contains("Version 2"), "{t}");
        for tag in ["Changed", "Added", "As it was then", "Changes between"] {
            assert!(!t.contains(tag), "no per-question rows ({tag}): {t}");
        }
        assert!(t.contains("Compare") && !t.contains("Restore..."), "{t}");
    }
}

#[test]
fn d2u_a_hiding_bank_page_with_lists_still_shows_no_per_question_rows() {
    // a page that (wrongly) carries lists beside per_question: false is drawn as bank lines
    let mut r = rig_with(LinkMode::Follow, true);
    r.w.on("Get", &versions_path(), 200, lines_json(), None);
    let mut bad = changes_json();
    bad["per_question"] = json!(false);
    r.w.on("Get", &changes_path(), 200, bad, None);
    r.open(0);
    let t = r.page(1280.0);
    assert!(!t.contains("Changed") && !t.contains("Changes between"), "{t}");
}

// ---- compare ----------------------------------------------------------------------------------------------------------------

#[test]
fn d2u_compare_reads_two_versions_flags_changes_and_folds_the_rest() {
    for w in BOTH {
        let mut r = rig();
        r.script_load();
        r.w.raw(&version_path(3), served(3, &[qj(QA, A3), qj(QB, B)], false));
        r.w.raw(&version_path(5), served(5, &[qj(QA, A5), qj(QB, B)], false));
        r.open(0);
        r.page(w);
        assert!(r.click(w, "Compare", 0), "Compare on version 5's row");
        let t = r.page(w);
        let c = r.qpage().compare.clone().expect("compare open");
        assert_eq!((c.left, c.right), (Side::Version(3), Side::Version(5)), "the chosen version and the one before it");
        assert_eq!(r.w.calls("/versions/"), 2, "two version reads");
        assert!(t.contains("Question text") && t.contains("changed"), "{t}");
        assert!(t.contains(A3) && t.contains(A5), "{t}");
        assert!(t.contains("Choices, answer key, points, difficulty, topics, explanation, picture, figure: the same"), "{t}");
        // two columns from 760 px, stacked below: the stacked view names each side above its block
        assert_eq!(compare_columns(w), w >= 760.0);
        assert!(r.click(w, "Close compare", 0));
        assert!(r.qpage().compare.is_none());
    }
}

#[test]
fn d2u_your_copy_is_a_side_and_costs_no_read() {
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
    r.open(0);
    r.w.raw(&version_path(5), served(5, &[qj(QA, A5), qj(QB, B)], false));
    r.page(1280.0);
    assert!(r.click(1280.0, "Compare with...", 0));
    r.page(1280.0);
    let c = r.qpage().compare.clone().expect("compare open");
    assert_eq!(c.right, Side::YourCopy);
    assert_eq!(c.left, Side::Version(5));
    // the copy's own base version is one read, the copy none; then version 2 against the copy
    assert_eq!(r.w.calls("/versions/"), 1);
    let cache = &mut r.m.edit.question_cache;
    match &mut r.m.edit.view {
        EditView::Question(p) => p.open_compare(Side::Version(2), Side::YourCopy, cache),
        _ => panic!(),
    }
    let t = r.page(1280.0);
    assert!(t.contains("Your copy") && t.contains(A3) && t.contains(A5), "{t}");
    assert_eq!(r.w.calls("/versions/2"), 1);
}

// ---- restore ----------------------------------------------------------------------------------------------------------------

#[test]
fn d2u_restore_asks_first_then_restores_that_question_as_one_unsent_change() {
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
    r.open(0);
    r.page(1280.0);
    let before = serde_json::to_string(&r.banks).unwrap();
    // (round 3, critic P7) version 5 is what the copy is: only version 2's row ("as it was then") offers Restore
    assert!(r.click(1280.0, "Restore...", 0));
    let t = r.page(1280.0);
    assert!(t.contains("Replace your copy of question 1 with version 2's? Only your copy changes; the circle gets it when you send. A restore point is saved first."), "{t}");
    assert_eq!(serde_json::to_string(&r.banks).unwrap(), before, "nothing changes before the yes");
    assert_eq!(*r.restores.0.lock().unwrap(), 0);
    // (round 4, critic P-6) the question names what comes in: version 2 is read for it (the restore's one read)
    assert_eq!(r.w.calls("/versions/2"), 1);
    assert!(t.contains(A3), "{t}");
    assert!(r.page(420.0).contains("Replace your copy of question 1 with version 2's?"), "the question at 420 px too");
    // No: nothing
    assert!(r.click(1280.0, "Cancel", 0));
    assert!(r.qpage().restore.is_none());
    assert!(r.click(1280.0, "Restore...", 0));
    assert!(r.click(1280.0, "Yes, restore", 0));
    let t = r.page(1280.0);
    assert_eq!(*r.restores.0.lock().unwrap(), 1, "a restore point first");
    assert_eq!(r.banks[0].questions[0].stem, A3);
    assert_eq!(r.banks[0].questions[1].stem, B);
    assert!(t.contains("Your copy: changed, not sent yet."), "{t}");
    assert!(t.contains("version 2"), "{t}");
    // back on the bank: the strip counts one unsent change
    assert!(r.click(1280.0, "< Back to Unit 2 quiz", 0));
    let strip = r.page(1280.0);
    assert_eq!(r.m.edit.status(LOCAL).map(|s| s.changes.total()), Some(1), "{strip}");
    assert!(strip.contains("1 unsent change"), "{strip}");
}

#[test]
fn d2u_restore_from_a_version_just_compared_reads_nothing_more() {
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(3), served(3, &[qj(QA, A3), qj(QB, B)], false));
    r.w.raw(&version_path(5), served(5, &[qj(QA, A5), qj(QB, B)], false));
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Compare", 0));
    r.page(1280.0);
    assert_eq!(r.w.calls("/versions/"), 2);
    let cache = &mut r.m.edit.question_cache;
    match &mut r.m.edit.view {
        EditView::Question(p) => {
            p.ask_restore(3);
            p.confirm_restore(cache);
        }
        _ => panic!(),
    }
    r.page(1280.0);
    assert_eq!(r.w.calls("/versions/"), 2, "the compared body is used");
    assert_eq!(r.banks[0].questions[0].stem, A3);
}

#[test]
fn d2u_follow_and_viewers_form_links_offer_compare_but_no_restore() {
    for (mode, vf) in [(LinkMode::Follow, false), (LinkMode::Edit, true)] {
        let mut r = rig_with(mode, vf);
        r.script_load();
        r.open(0);
        for w in BOTH {
            let t = r.page(w);
            assert!(t.contains("Compare") && !t.contains("Restore..."), "{mode:?} {vf}: {t}");
        }
        // asked anyway: refused, nothing read or written
        let before = serde_json::to_string(&r.banks).unwrap();
        let cache = &mut r.m.edit.question_cache;
        match &mut r.m.edit.view {
            EditView::Question(p) => {
                p.ask_restore(2);
                p.confirm_restore(cache);
            }
            _ => panic!(),
        }
        r.page(1280.0);
        assert_eq!(serde_json::to_string(&r.banks).unwrap(), before);
        assert_eq!(*r.restores.0.lock().unwrap(), 0);
        assert_eq!(r.w.calls("/versions/"), 0);
    }
}

// ---- the session's cache --------------------------------------------------------------------------------------------------

#[test]
fn d2u_the_change_lists_are_kept_for_the_session_and_dropped_for_another_account() {
    let mut r = rig();
    r.script_load();
    r.open(0);
    r.page(1280.0);
    assert_eq!(r.w.last_header("/changes", "If-None-Match"), None);
    assert!(r.click(1280.0, "< Back to Unit 2 quiz", 0));
    // again: the lists are asked with their tag (a 304 is free) and the lines are not read again
    r.w.on("Get", &changes_path(), 304, json!({}), Some("\"e1\""));
    r.open(0);
    let t = r.page(1280.0);
    assert!(t.contains("Changed"), "{t}");
    assert_eq!(r.w.last_header("/changes", "If-None-Match").as_deref(), Some("\"e1\""));
    assert_eq!(r.w.calls("/versions"), 1);
    // another account on this computer: nothing read for olga is used for pete
    assert!(!r.m.edit.question_cache.is_empty());
    r.who = "pete".into();
    r.page(1280.0);
    assert!(r.m.edit.question_cache.is_empty(), "{:?}", r.m.edit.question_cache);
    r.script_load();
    r.open(0);
    r.page(1280.0);
    assert_eq!(r.w.last_header("/changes", "If-None-Match"), None);
    assert_eq!(r.w.calls("/versions"), 2);
    // signed out: dropped too
    r.m.edit.forget_account(false);
    assert!(r.m.edit.question_cache.is_empty());
    let _ = Gate::SignedOut;
}

// ---- headless screenshots (run by hand: D2FRAMES=<dir> cargo test --lib d2u_frames -- --ignored) -----------------------

/// A small software painter for egui's meshes: enough to look at a frame without a window (no GPU, no Xvfb).
struct Painter {
    textures: HashMap<egui::TextureId, egui::ColorImage>,
}

impl Painter {
    fn apply(&mut self, delta: &egui::TexturesDelta) {
        for (id, list) in &delta.set {
            for d in list {
                let egui::ImageData::Color(img) = &d.image;
                match d.pos {
                    None => {
                        self.textures.insert(*id, (**img).clone());
                    }
                    Some([x0, y0]) => {
                        let Some(t) = self.textures.get_mut(id) else { continue };
                        for y in 0..img.size[1] {
                            for x in 0..img.size[0] {
                                t.pixels[(y0 + y) * t.size[0] + x0 + x] = img.pixels[y * img.size[0] + x];
                            }
                        }
                    }
                }
            }
        }
    }

    fn paint(&self, ctx: &egui::Context, out: egui::FullOutput, w: usize, h: usize) -> image::RgbaImage {
        let mut px = vec![[0f32; 4]; w * h];
        let prims = ctx.tessellate(out.shapes, out.pixels_per_point);
        for p in prims {
            let egui::epaint::Primitive::Mesh(mesh) = p.primitive else { continue };
            let tex = self.textures.get(&mesh.texture_id);
            let clip = p.clip_rect;
            for tri in mesh.indices.chunks(3) {
                let [a, b, c] = [mesh.vertices[tri[0] as usize], mesh.vertices[tri[1] as usize], mesh.vertices[tri[2] as usize]];
                let area = (b.pos.x - a.pos.x) * (c.pos.y - a.pos.y) - (c.pos.x - a.pos.x) * (b.pos.y - a.pos.y);
                if area.abs() < 1e-6 {
                    continue;
                }
                let x0 = a.pos.x.min(b.pos.x).min(c.pos.x).max(clip.min.x).max(0.0).floor() as usize;
                let x1 = (a.pos.x.max(b.pos.x).max(c.pos.x).min(clip.max.x).min(w as f32).ceil() as usize).min(w);
                let y0 = a.pos.y.min(b.pos.y).min(c.pos.y).max(clip.min.y).max(0.0).floor() as usize;
                let y1 = (a.pos.y.max(b.pos.y).max(c.pos.y).min(clip.max.y).min(h as f32).ceil() as usize).min(h);
                for y in y0..y1 {
                    for x in x0..x1 {
                        let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                        let w0 = ((b.pos.x - fx) * (c.pos.y - fy) - (c.pos.x - fx) * (b.pos.y - fy)) / area;
                        let w1 = ((c.pos.x - fx) * (a.pos.y - fy) - (a.pos.x - fx) * (c.pos.y - fy)) / area;
                        let w2 = 1.0 - w0 - w1;
                        if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                            continue;
                        }
                        let mix = |f: fn(&egui::epaint::Vertex) -> f32| w0 * f(&a) + w1 * f(&b) + w2 * f(&c);
                        let col = [mix(|v| v.color.r() as f32), mix(|v| v.color.g() as f32), mix(|v| v.color.b() as f32), mix(|v| v.color.a() as f32)];
                        let t = tex.map_or([255.0; 4], |t| {
                            let u = (mix(|v| v.uv.x) * t.size[0] as f32) as usize;
                            let v = (mix(|v| v.uv.y) * t.size[1] as f32) as usize;
                            let c = t.pixels[v.min(t.size[1] - 1) * t.size[0] + u.min(t.size[0] - 1)];
                            [c.r() as f32, c.g() as f32, c.b() as f32, c.a() as f32]
                        });
                        let src: Vec<f32> = (0..4).map(|i| col[i] * t[i] / 255.0).collect();
                        let d = &mut px[y * w + x];
                        let k = 1.0 - src[3] / 255.0;
                        for i in 0..4 {
                            d[i] = src[i] + d[i] * k;
                        }
                    }
                }
            }
        }
        image::RgbaImage::from_fn(w as u32, h as u32, |x, y| {
            let p = px[y as usize * w + x as usize];
            image::Rgba([p[0].min(255.0) as u8, p[1].min(255.0) as u8, p[2].min(255.0) as u8, 255])
        })
    }
}

impl Rig {
    /// The bank's page at `width` as a PNG in `dir`, from the rig's own context (its textures kept since the first frame).
    fn shot(&mut self, dir: &str, name: &str, width: f32) {
        let h = 1500usize;
        self.frame_at(width, Vec::new());
        let snap = self.snap();
        let t0 = self.ctx.input(|i| i.time);
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, h as f32))), time: Some(t0 + 0.05), events: vec![egui::Event::PointerGone], ..Default::default() };
        let mut browse = BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), ImportLog::memory());
        let (m, banks, acct) = (&mut self.m, &mut self.banks, &mut self.acct);
        let mut out = self.ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                super::bank_links_ui::strip_with(ui, m, banks, &mut browse, acct, &snap, web_time::Instant::now(), NOW, LOCAL);
            });
        });
        self.painter.apply(&out.textures_delta);
        out.textures_delta.clear();
        let img = self.painter.paint(&self.ctx, out, width as usize, h);
        let path = format!("{dir}/{name}-{}.png", width as u32);
        img.save(&path).unwrap();
        println!("{path}");
    }
}

#[test]
#[ignore = "writes PNG frames; run by hand with D2FRAMES=<dir>"]
fn d2u_frames() {
    let dir = std::env::var("D2FRAMES").expect("D2FRAMES");
    std::fs::create_dir_all(&dir).unwrap();
    let both = |r: &mut Rig, name: &str, w: f32| {
        r.page(w);
        r.shot(&dir, name, w);
    };
    for w in BOTH {
        // the panel with its rows, gap line and held-back line
        let mut r = rig();
        r.script_load();
        r.open(0);
        both(&mut r, "01-panel", w);
        r.qpage_mut().show_all = true;
        both(&mut r, "02-every-version", w);
        r.qpage_mut().show_all = false;
        // the loading state
        r.qpage_mut().load = Load::Loading;
        both(&mut r, "03-loading", w);
        // an Added row (question B)
        let mut r = rig();
        r.script_load();
        r.open(1);
        both(&mut r, "04-added-row", w);
        // a compare where one side doesn't have the question (version 2 has no B)
        r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
        r.w.raw(&version_path(3), served(3, &[qj(QA, A3), qj(QB, B)], false));
        let cache = &mut r.m.edit.question_cache;
        if let EditView::Question(p) = &mut r.m.edit.view {
            p.open_compare(Side::Version(2), Side::Version(3), cache);
        }
        both(&mut r, "05-compare-one-side-missing", w);
        // an unsent edit, the restore question with its warning, then done with Undo
        let mut r = rig();
        r.script_load();
        r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
        r.banks[0].questions[0].stem = "Solve 2x + 3 = 11 (my unsent edit)".into();
        r.banks[0].modified += 1;
        r.open(0);
        r.page(w);
        r.qpage_mut().ask_restore(2);
        both(&mut r, "06-restore-ask-unsent", w);
        let cache = &mut r.m.edit.question_cache;
        if let EditView::Question(p) = &mut r.m.edit.view {
            p.confirm_restore(cache);
        }
        both(&mut r, "07-restore-done-undo", w);
        assert!(r.click(w, "< Back to Unit 2 quiz", 0));
        both(&mut r, "08-strip-after", w);
        // compare: two versions, and your copy as one side
        let mut r = rig();
        r.script_load();
        r.w.raw(&version_path(3), served(3, &[qj(QA, A3), qj(QB, B)], false));
        r.w.raw(&version_path(5), served(5, &[qj(QA, A5), qj(QB, B)], false));
        r.open(0);
        r.page(w);
        assert!(r.click(w, "Compare", 0));
        both(&mut r, "09-compare", w);
        r.banks[0].questions[0].stem = "Solve 2x + 3 = 11".into();
        r.banks[0].modified += 1;
        let cache = &mut r.m.edit.question_cache;
        if let EditView::Question(p) = &mut r.m.edit.view {
            p.open_compare(Side::Version(5), Side::YourCopy, cache);
        }
        both(&mut r, "10-compare-your-copy", w);
        // a Removed row, the restore question for it, and the page after restoring it
        let mut r = removed_rig();
        r.open(0);
        both(&mut r, "11-removed-row", w);
        r.qpage_mut().ask_restore(2);
        both(&mut r, "12-removed-restore-ask", w);
        let cache = &mut r.m.edit.question_cache;
        if let EditView::Question(p) = &mut r.m.edit.view {
            p.confirm_restore(cache);
        }
        both(&mut r, "13-removed-restored", w);
        // a failed load (offline / the pool's error), with Try again
        let mut r = rig();
        r.w.on("Get", &versions_path(), 500, json!({"error": "internal"}), None);
        r.open(0);
        both(&mut r, "14-failed-load", w);
        // a failed restore: the picture could not be read
        let mut r = rig();
        r.script_load();
        let sha = sha256_hex(b"a picture");
        r.w.raw(&version_path(2), served(2, &[pictured(&sha)], false));
        r.w.on("Get", &format!("/circles/{CID}/banks/{BID}/blobs/{sha}"), 500, json!({"error": "internal"}), None);
        r.open(0);
        r.page(w);
        r.qpage_mut().ask_restore(2);
        let cache = &mut r.m.edit.question_cache;
        if let EditView::Question(p) = &mut r.m.edit.view {
            p.confirm_restore(cache);
        }
        both(&mut r, "15-restore-failed-picture", w);
        // a stale bank: no Restore, the copy's line says why
        let mut r = rig();
        r.script_load();
        r.m.edit.update_link(LOCAL, |l| {
            l.bank_modified = r.banks[0].modified + 100;
            l.bank_print = "another".into();
            true
        });
        r.open(0);
        both(&mut r, "16-stale-bank", w);
        // a plain follow-link viewer: compare, no Restore
        let mut r = rig_with(LinkMode::Follow, false);
        r.script_load();
        r.open(0);
        both(&mut r, "17-follow-viewer", w);
        // a viewer of a hiding bank: bank lines (the pool blanks the messages), then its compare of two viewers' forms
        let mut r = hiding_viewer();
        let mut blank = lines_json();
        for v in blank["versions"].as_array_mut().unwrap() {
            v["message"] = json!("");
        }
        r.w.on("Get", &versions_path(), 200, blank, None);
        r.w.on("Get", &changes_path(), 200, json!({"bank_id": BID, "head_version": 5, "per_question": false, "reason": "keys_hidden"}), None);
        // (round 5) the copy's question is the Matching one compared in frame 19
        let mq: crate::model::Question = serde_json::from_value(json!({"id": r.banks[0].questions[0].id, "stem": "Match the organelle to its job", "kind": {"Matching": {"pairs": [{"left": "Nucleus", "right": ""}, {"left": "Ribosome", "right": ""}], "distractors": ["DNA", "proteins", "sugar"]}}, "points": 1.0, "difficulty": null, "created": 5, "modified": 6})).unwrap();
        r.banks[0].questions[0] = mq.clone();
        let bank = r.banks[0].clone();
        r.m.edit.update_link(LOCAL, |l| {
            if let Some(b) = l.base.iter_mut().find(|b| b.local_id == mq.id) {
                b.snap = super::merge::snap_of(&mq, None);
            }
            l.mark_bank(&bank);
            true
        });
        r.open(0);
        both(&mut r, "18-hiding-viewer", w);
        let m3 = json!({"id": QA, "stem": "Match the organelle to its job", "kind": {"Matching": {"pairs": [{"left": "Nucleus", "right": ""}, {"left": "Ribosome", "right": ""}], "distractors": ["proteins", "DNA", "sugar"]}}, "points": 1.0, "difficulty": null, "created": 5, "modified": 6, "keyless": true});
        let mut m5 = m3.clone();
        m5["kind"]["Matching"]["pairs"] = json!([{"left": "Ribosome", "right": ""}, {"left": "Nucleus", "right": ""}]);
        m5["kind"]["Matching"]["distractors"] = json!(["sugar", "glucose", "DNA"]);
        r.w.raw(&version_path(3), served(3, &[m3], true));
        r.w.raw(&version_path(5), served(5, &[m5], true));
        assert!(r.click(w, "Compare", 0));
        both(&mut r, "19-hiding-viewer-compare", w);
        // "No version that is still kept changed this question."
        let mut r = rig();
        r.w.on("Get", &versions_path(), 200, lines_json(), None);
        r.w.on("Get", &changes_path(), 200, json!({"bank_id": BID, "head_version": 5, "per_question": true, "versions": [{"version": 5, "compared_to": 3, "gap": true}, {"version": 3, "compared_to": 2, "gap": true}]}), None);
        r.open(0);
        both(&mut r, "20-no-version-changed-it", w);
        // "The circle doesn't have this question yet"
        let mut r = rig();
        let mut q = r.banks[0].questions[0].clone();
        q.id = "new-here".into();
        q.stem = "A question only in my copy".into();
        r.banks[0].questions.push(q);
        r.banks[0].modified += 1;
        let bank = r.banks[0].clone();
        r.frame_at(w, Vec::new());
        assert!(r.m.edit.open_question_history(&bank, "new-here"));
        both(&mut r, "21-not-in-the-circle", w);
        // signed out
        let mut r = rig();
        r.script_load();
        r.open(0);
        r.page(w);
        r.qpage_mut().load = Load::SignedOut;
        both(&mut r, "22-signed-out", w);
        // (round 4, critic P-6) a version shown whole
        let mut r = rig();
        r.script_load();
        let mut q = qj(QA, A3);
        q["explanation"] = json!("Subtract 3, then divide by 2.");
        q["topics"] = json!(["linear equations"]);
        r.w.raw(&version_path(2), served(2, &[q], false));
        r.open(0);
        r.page(w);
        let cache = &mut r.m.edit.question_cache;
        if let EditView::Question(p) = &mut r.m.edit.view {
            p.show(2, cache);
        }
        both(&mut r, "23-show-version", w);
        // (round 4, critic P-2) a restore the count could not see, refused
        let mut r = rig();
        r.script_load();
        let mut q = qj(QA, A5);
        q["figure"] = json!({"id": "triangle", "params": {"a": 4}});
        r.w.raw(&version_path(2), served(2, &[q], false));
        r.banks[0].questions[0].figure = Some(crate::model::FigureSpec { id: "triangle".into(), params: [("a".to_string(), json!(3))].into_iter().collect() });
        r.banks[0].modified += 1;
        r.open(0);
        r.page(w);
        r.qpage_mut().ask_restore(2);
        r.page(w);
        let cache = &mut r.m.edit.question_cache;
        if let EditView::Question(p) = &mut r.m.edit.view {
            p.confirm_restore(cache);
        }
        both(&mut r, "24-restore-refused-figure-only", w);
        // (round 4, critic P-4) a second restore cancelled: the first one's Undo is back
        let mut r = rig();
        r.script_load();
        r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
        r.w.raw(&version_path(5), served(5, &[qj(QA, A5), qj(QB, B)], false));
        r.open(0);
        r.page(w);
        r.qpage_mut().ask_restore(2);
        r.page(w);
        let cache = &mut r.m.edit.question_cache;
        if let EditView::Question(p) = &mut r.m.edit.view {
            p.confirm_restore(cache);
        }
        r.page(w);
        r.qpage_mut().ask_restore(5);
        r.page(w);
        r.qpage_mut().cancel_restore();
        both(&mut r, "25-undo-after-cancel", w);
        // (round 5, critic I2) the question's version could not be read: said, with Try again, Yes waiting
        let mut r = rig();
        r.script_load();
        r.w.on("Get", &version_path(2), 503, json!({"error": "busy"}), None);
        r.open(0);
        r.page(w);
        r.qpage_mut().ask_restore(2);
        both(&mut r, "26-restore-read-failed", w);
        // (round 5, critic I6) a Show whose read failed
        let mut r = rig();
        r.script_load();
        r.w.on("Get", &version_path(2), 503, json!({"error": "busy"}), None);
        r.open(0);
        r.page(w);
        let cache = &mut r.m.edit.question_cache;
        if let EditView::Question(p) = &mut r.m.edit.view {
            p.show(2, cache);
        }
        both(&mut r, "27-show-failed", w);
        // (round 5, critic I5) the result dismissed: Undo stays on the row restored
        let mut r = rig();
        r.script_load();
        r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
        r.open(0);
        r.page(w);
        r.qpage_mut().ask_restore(2);
        r.page(w);
        let cache = &mut r.m.edit.question_cache;
        if let EditView::Question(p) = &mut r.m.edit.view {
            p.confirm_restore(cache);
        }
        r.page(w);
        r.qpage_mut().restore = None;
        both(&mut r, "28-undo-after-dismiss", w);
        // (round 5, critic I3) a question of the bank open in the editor: the restore waits
        let mut r = rig();
        r.script_load();
        r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
        r.open(0);
        r.page(w);
        r.qpage_mut().ask_restore(2);
        r.m.edit.set_question_draft(Some(LOCAL));
        both(&mut r, "29-restore-under-draft", w);
    }
}


// ---- round 3 (critics on round 2): each test written first ------------------------------------------------------------

/// The history list's own reads (not a version's).
fn list_reads(w: &Wire) -> usize {
    w.seen.lock().unwrap().iter().filter(|(p, _)| p.ends_with("/versions")).count()
}

fn qk(id: &str, stem: &str, correct_first: bool) -> Value {
    json!({"id": id, "stem": stem, "kind": {"Mc": {"answers": [{"text": "x = 2", "correct": correct_first}, {"text": "x = 5", "correct": !correct_first}]}}, "points": 1.0, "difficulty": null, "created": 5, "modified": 6, "explanation": "SECRET EXPLANATION"})
}

/// The viewers' form of a question: no key, no explanation.
fn qs(id: &str, stem: &str) -> Value {
    json!({"id": id, "stem": stem, "kind": {"Mc": {"answers": [{"text": "x = 2", "correct": false}, {"text": "x = 5", "correct": false}]}}, "points": 1.0, "difficulty": null, "created": 5, "modified": 6, "keyless": true})
}

#[test]
fn critic_cached_full_body_and_lines_survive_hiding_and_withholding() {
    // (critic B1, ported) a viewer (follow link) of a bank that does not hide its keys yet
    let mut r = rig_with(LinkMode::Follow, false);
    r.script_load();
    r.w.raw(&version_path(3), served(3, &[qk(QA, "WITHHELD STUDENT TEXT", false), qj(QB, B)], false));
    r.w.raw(&version_path(5), served(5, &[qk(QA, A5, true), qj(QB, B)], false));
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Compare", 0));
    r.page(1280.0);
    assert!(r.click(1280.0, "< Back to Unit 2 quiz", 0));
    // the author turns hiding on (PATCH): the same PATCH withholds version 3; no new version, head stays 5. The server now
    // says per_question:false for this viewer, the lines say 3 is withheld, messages are blank.
    let mut l2 = lines_json();
    l2["versions"][2] = json!({"version": 3, "by": "", "at": NOW - 3 * DAY, "message": "", "kept": false, "withheld": true, "review_copy": "full"});
    l2["versions"][0]["message"] = json!("");
    r.w.on("Get", &versions_path(), 200, l2, None);
    r.w.on("Get", &changes_path(), 200, json!({"bank_id": BID, "head_version": 5, "per_question": false, "reason": "keys_hidden"}), None);
    // what the pool serves this viewer now: the viewers' form
    r.w.raw(&version_path(2), served(2, &[qs(QA, A3)], true));
    r.w.raw(&version_path(5), served(5, &[qs(QA, A5), qs(QB, B)], true));
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Compare", 0), "compare offered on version 5");
    let t = r.page(1280.0);
    for leak in ["WITHHELD STUDENT TEXT", "choice 1", "choice 2", "Fixed a distractor", "SECRET EXPLANATION"] {
        assert!(!t.contains(leak), "{leak} leaked: {t}");
    }
}

#[test]
fn d2f_a_new_change_list_tag_drops_the_banks_bodies_and_reads_its_lines_again() {
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(3), served(3, &[qj(QA, A3), qj(QB, B)], false));
    r.w.raw(&version_path(5), served(5, &[qj(QA, A5), qj(QB, B)], false));
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Compare", 0));
    r.page(1280.0);
    assert_eq!(r.m.edit.question_cache.bodies(), 2);
    assert!(r.click(1280.0, "< Back to Unit 2 quiz", 0));
    // something moved at the pool (a new tag, the same head): what was read for the bank is not used again
    r.w.on("Get", &changes_path(), 200, changes_json(), Some("\"e2\""));
    r.w.on("Get", &versions_path(), 200, lines_json(), None);
    r.open(0);
    r.page(1280.0);
    assert_eq!(r.m.edit.question_cache.bodies(), 0);
    assert_eq!(list_reads(&r.w), 2, "{:?}", r.w.reads());
    // and a tag missing from the answer is "moved" too
    assert!(r.click(1280.0, "< Back to Unit 2 quiz", 0));
    r.w.on("Get", &changes_path(), 200, changes_json(), None);
    r.w.on("Get", &versions_path(), 200, lines_json(), None);
    r.open(0);
    r.page(1280.0);
    assert_eq!(list_reads(&r.w), 3, "{:?}", r.w.reads());
}

#[test]
fn d2f_a_withheld_or_mismatched_read_is_never_kept() {
    let mut r = rig();
    r.script_load();
    r.w.on("Get", &version_path(3), 404, json!({"error": "version_withheld"}), None);
    r.w.raw(&version_path(5), served(5, &[qj(QA, A5), qj(QB, B)], false));
    r.open(0);
    r.page(1280.0);
    // a body kept from earlier this session for that version goes too
    let scope = super::question_history::scope_of(&r.m.edit.link_for_local(LOCAL).unwrap());
    r.m.edit.question_cache.put_body(CID, BID, "someone|false|false", super::question_history::VersionBody::from_payload(3, &[], false, NOW));
    assert!(r.click(1280.0, "Compare", 0));
    let t = r.page(1280.0);
    assert!(t.contains("hidden while it is reviewed"), "{t}");
    assert!(r.m.edit.question_cache.body(CID, BID, &scope, 3).is_none() && r.m.edit.question_cache.body(CID, BID, "someone|false|false", 3).is_none());
    // (critic M4) an answer for another version, or another bank, is not kept as this one
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(3), served(4, &[qj(QA, A3), qj(QB, B)], false));
    r.w.raw(&version_path(5), served(5, &[qj(QA, A5), qj(QB, B)], false));
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Compare", 0));
    r.page(1280.0);
    assert!(matches!(r.qpage().compare.as_ref().map(|c| &c.state), Some(CompareState::Failed(_))));
    let scope = super::question_history::scope_of(&r.m.edit.link_for_local(LOCAL).unwrap());
    assert!(r.m.edit.question_cache.body(CID, BID, &scope, 3).is_none());
}

fn hiding_viewer() -> Rig {
    let r = rig_link(LinkMode::Follow, true, |l| {
        l.role_seen = Some("viewer".into());
        l.seen.hide_key = true;
    });
    // (the circle's own checks say the same: a viewer, of a bank that hides its keys)
    let mut h = heads(5);
    h["banks"][0]["hide_key"] = json!(true);
    r.w.keep("Post", "/circles/heads", h);
    r.w.keep("Get", "/circles", circles_list("viewer"));
    r.w.keep("Get", &format!("/circles/{CID}"), detail("viewer", json!({})));
    r
}

#[test]
fn d2f_a_viewer_of_a_hiding_bank_gets_bank_lines_whatever_the_answer_says() {
    // (critic I3) the link says viewer of a bank that hides its keys; the answer (wrongly) carries lists
    let mut r = hiding_viewer();
    r.script_load();
    r.open(0);
    let t = r.page(1280.0);
    assert!(!t.contains("Changed") && !t.contains("As it was then"), "{t}");
    assert!(t.contains("1 of your 100 daily"), "(critic P11; round 4: one read per version) {t}");
    assert!(r.click(1280.0, "< Back to Unit 2 quiz", 0));
    // and its change lists are never asked with a tag (a 304 would bring back a page read under another role)
    r.w.on("Get", &changes_path(), 200, json!({"bank_id": BID, "head_version": 5, "per_question": false, "reason": "keys_hidden"}), Some("\"v1\""));
    // (the answer is not the one read before: the lines are read again)
    r.w.on("Get", &versions_path(), 200, lines_json(), None);
    r.open(0);
    r.page(1280.0);
    assert_eq!(r.w.last_header("/changes", "If-None-Match"), None);
}

#[test]
fn d2f_the_timeline_is_worked_out_only_when_what_it_shows_changes() {
    // (critic I2)
    let mut r = rig();
    r.script_load();
    r.open(0);
    for _ in 0..5 {
        r.page(1280.0);
    }
    let n = r.qpage().timeline_builds;
    assert!(n <= 2, "{n}");
    r.qpage_mut().show_all = true;
    r.page(1280.0);
    assert_eq!(r.qpage().timeline_builds, n + 1);
    r.banks[0].questions[0].stem = "Edited".into();
    r.banks[0].modified += 1;
    r.page(1280.0);
    assert_eq!(r.qpage().timeline_builds, n + 2);
}

#[test]
fn d2f_another_account_in_the_session_drops_what_was_read() {
    // (critic I4) the session's account changed under the page
    let mut r = rig();
    *r.w.who.lock().unwrap() = Some("https://pool.test\nolga@school.org".into());
    r.script_load();
    r.open(0);
    r.page(1280.0);
    assert!(!r.m.edit.question_cache.is_empty());
    // (and another bank's reads go too: the whole session's, not only this page's)
    r.m.edit.question_cache.put_body(CID, "0000000000000000000000000000000f", "editor|false|false", super::question_history::VersionBody::from_payload(3, &[], false, NOW));
    *r.w.who.lock().unwrap() = Some("https://pool.test\npete@school.org".into());
    r.script_load();
    r.page(1280.0);
    assert_eq!(list_reads(&r.w), 2, "read again for pete: {:?}", r.w.reads());
    assert!(r.m.edit.question_cache.body(CID, "0000000000000000000000000000000f", "editor|false|false", 3).is_none());
}

#[test]
fn d2f_the_page_closes_when_its_link_goes_and_drops_the_banks_reads() {
    // (critic M1) left the circle (or removed, or closed)
    let mut r = rig();
    r.script_load();
    r.open(0);
    r.page(1280.0);
    r.m.edit.update_link(LOCAL, |l| {
        l.role_seen = Some(super::circle_links::ROLE_NONE.into());
        true
    });
    r.page(1280.0);
    assert!(!r.m.edit.active(), "{:?}", r.m.edit.view);
    assert!(r.m.edit.question_cache.is_empty());
    // the bank deleted
    let mut r = rig();
    r.script_load();
    r.open(0);
    r.page(1280.0);
    r.banks[0].id = "gone".into();
    r.page(1280.0);
    assert!(!r.m.edit.active());
}

fn pictured(sha: &str) -> Value {
    let mut q = qj(QA, A3);
    q["image"] = json!({"file": format!("media/{sha}.webp"), "width": 2, "height": 2});
    q
}

#[test]
fn d2f_a_picture_is_stored_only_when_the_restore_goes_through() {
    // (critic M2)
    let bytes = b"the picture of version 2".to_vec();
    let sha = sha256_hex(&bytes);
    let blob = format!("/circles/{CID}/banks/{BID}/blobs/{sha}");
    // refused (the bank was put back from a safety copy after the question was asked): nothing fetched or stored
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(2), served(2, &[pictured(&sha)], false));
    r.open(0);
    r.page(1280.0);
    r.qpage_mut().ask_restore(2);
    r.m.edit.update_link(LOCAL, |l| {
        l.bank_modified = r.banks[0].modified + 100;
        l.bank_print = "another".into();
        true
    });
    let cache = &mut r.m.edit.question_cache;
    if let EditView::Question(p) = &mut r.m.edit.view {
        p.confirm_restore(cache);
    }
    r.page(1280.0);
    assert_eq!(*r.pics.0.lock().unwrap(), 0);
    assert_eq!(r.w.calls("/blobs/"), 0);
    // signed out while fetching: said so
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(2), served(2, &[pictured(&sha)], false));
    r.w.on("Get", &blob, 401, json!({"error": "unauthorized"}), None);
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Restore...", 0));
    assert!(r.click(1280.0, "Yes, restore", 0));
    let t = r.page(1280.0);
    assert!(t.contains("Sign in") && !t.contains("could not be fetched"), "{t}");
    assert_eq!(*r.pics.0.lock().unwrap(), 0);
    // through: stored once
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(2), served(2, &[pictured(&sha)], false));
    r.w.raw(&blob, bytes.clone());
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Restore...", 0));
    assert!(r.click(1280.0, "Yes, restore", 0));
    r.page(1280.0);
    assert_eq!(*r.pics.0.lock().unwrap(), 1);
    assert_eq!(r.banks[0].questions[0].image.as_ref().map(|i| i.file.clone()), Some(format!("media/{sha}.webp")));
}

#[test]
fn d2f_restore_warns_of_unsent_edits_and_can_be_undone() {
    // (critic P2)
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
    r.banks[0].questions[0].stem = "My unsent edit".into();
    r.banks[0].modified += 1;
    let mine = serde_json::to_value(&r.banks[0].questions).unwrap();
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Restore...", 1));
    let t = r.page(1280.0);
    assert!(t.contains("Your unsent changes to question 1 will be replaced."), "{t}");
    assert!(r.click(1280.0, "Yes, restore", 0));
    let t = r.page(1280.0);
    assert_eq!(r.banks[0].questions[0].stem, A3);
    assert!(t.contains("Undo restore") && t.contains("restore point") && !t.contains("memory") && !t.contains("backups/"), "{t}");
    assert!(r.click(1280.0, "Undo restore", 0));
    r.page(1280.0);
    assert_eq!(serde_json::to_value(&r.banks[0].questions).unwrap(), mine, "exactly the question it replaced");
    assert_eq!(*r.restores.0.lock().unwrap(), 2, "a restore point before each");
    assert_eq!(r.m.edit.status(LOCAL).map(|s| s.changes.total()), Some(1));
    assert!(r.page(1280.0).contains("Your copy: changed, not sent yet."));
}

/// Version 5 removed A; the copy is behind, at version 3, and still has A.
fn removed_rig() -> Rig {
    let r = rig_link(LinkMode::Edit, false, |l| {
        l.base_version = 3;
        l.base_version_id = hex(3);
    });
    r.w.on("Get", &versions_path(), 200, lines_json(), None);
    r.w.on("Get", &changes_path(), 200, json!({"bank_id": BID, "head_version": 5, "per_question": true, "versions": [
        {"version": 5, "compared_to": 3, "added": [], "changed": [], "removed": [QA]},
        {"version": 3, "compared_to": 2, "added": [QB], "changed": [], "removed": []},
    ]}), Some("\"r1\""));
    r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
    r
}

#[test]
fn d2f_restoring_a_question_the_circle_removed_says_it_comes_back_new_and_keeps_its_history() {
    // (critic P3)
    let mut r = removed_rig();
    r.open(0);
    let t = r.page(1280.0);
    assert!(t.contains("Removed"), "{t}");
    assert!(r.click(1280.0, "Restore...", 0));
    let t = r.page(1280.0);
    assert!(t.contains("It comes back as a new question when you send."), "{t}");
    assert!(r.click(1280.0, "Yes, restore", 0));
    let t = r.page(1280.0);
    assert!(t.contains("comes back as a new question when you send"), "{t}");
    assert!(t.contains("Removed") && t.contains("Version 2") && !t.contains("No version that is still kept changed this question"), "{t}");
    let link = r.m.edit.link_for_local(LOCAL).unwrap();
    assert_eq!(link.qid_of(&r.banks[0].questions[0].id), None, "no longer paired with the removed question");
}

fn text_rects(r: &mut Rig, width: f32) -> Vec<(String, egui::Rect)> {
    r.frame_at(width, Vec::new());
    r.frame_at(width, Vec::new())
}

#[test]
fn d2f_the_restore_question_is_asked_under_its_row_and_escape_cancels() {
    // (critic P6) an edited copy: both rows offer Restore
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
    r.banks[0].questions[0].stem = "Edited here".into();
    r.banks[0].modified += 1;
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Restore...", 1));
    let rects = text_rects(&mut r, 1280.0);
    let y = |needle: &str| rects.iter().find(|(t, _)| t.starts_with(needle)).map(|(_, r)| r.min.y).unwrap_or_else(|| panic!("{needle} not drawn"));
    let (row, ask, below) = (y("Version 2"), y("Replace your copy"), y("Version 1 is no longer kept"));
    assert!(row < ask && ask < below, "{row} {ask} {below}");
    // another row's Restore does not take the question elsewhere
    assert!(r.click(1280.0, "Restore...", 0));
    assert_eq!(r.qpage().restore.as_ref().map(|x| x.version), Some(2));
    // Escape: nothing restored
    r.frame_at(1280.0, vec![egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }]);
    r.page(1280.0);
    assert!(r.qpage().restore.is_none());
}

#[test]
fn d2f_restore_is_hidden_on_a_stale_bank_and_for_the_version_the_copy_is() {
    // (critic P7) in step with version 5: no Restore on its row
    let mut r = rig();
    r.script_load();
    r.open(0);
    let t = r.page(1280.0);
    assert_eq!(t.matches("Restore...").count(), 1, "only version 2's: {t}");
    assert!(t.contains("Compare with your copy"), "the as-it-was row compares with the copy: {t}");
    // a bank put back from a safety copy: none at all
    let mut r = rig_link(LinkMode::Edit, false, |_| {});
    r.script_load();
    r.m.edit.update_link(LOCAL, |l| {
        l.bank_modified = r.banks[0].modified + 100;
        l.bank_print = "another".into();
        true
    });
    r.open(0);
    let t = r.page(1280.0);
    assert!(!t.contains("Restore..."), "{t}");
}

#[test]
fn d2f_a_version_listed_after_a_held_back_one_says_so() {
    // (critic P8)
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(3), served(3, &[qj(QA, A3), qj(QB, B)], false));
    r.w.raw(&version_path(5), served(5, &[qj(QA, A5), qj(QB, B)], false));
    r.open(0);
    let t = r.page(1280.0);
    assert!(t.contains("changed since version 3"), "{t}");
    assert!(t.contains("Version 4 · held back for review · no details"), "{t}");
    assert!(r.click(1280.0, "Compare", 0));
    let t = r.page(1280.0);
    assert!(t.contains("includes version 4, held back"), "{t}");
}

#[test]
fn d2f_your_copy_agrees_with_the_strip() {
    // (critic P9) a base with no record of a difficulty: never "the same"
    let mut r = rig_link(LinkMode::Edit, false, |l| {
        for b in &mut l.base {
            b.snap.difficulty = None;
        }
    });
    // (the circle model asks the base version for its difficulties; it is no longer kept, so they stay unknown)
    for _ in 0..3 {
        r.w.on("Get", &version_path(5), 404, json!({"error": "version_not_retained"}), None);
    }
    r.script_load();
    r.open(0);
    let t = r.page(1280.0);
    assert!(!t.contains("the same as version"), "{t}");
    // a figure-only change, told by the base version's body read this session
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(5), served(5, &[qj(QA, A5), qj(QB, B)], false));
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Compare with...", 0));
    r.page(1280.0);
    r.banks[0].questions[0].figure = Some(crate::model::FigureSpec { id: "triangle".into(), params: Default::default() });
    r.banks[0].modified += 1;
    let t = r.page(1280.0);
    // (round 4, critic P-2) the strip counts none: never "changed, not sent yet", nor "the same"
    assert!(t.contains("no change counted since version 5") && !t.contains("changed, not sent yet") && !t.contains("the same as version"), "{t}");
}

#[test]
fn d2f_a_role_change_drops_the_banks_reads() {
    // (critic P10)
    let mut r = rig();
    r.script_load();
    r.open(0);
    r.page(1280.0);
    let before = super::question_history::scope_of(&r.m.edit.link_for_local(LOCAL).unwrap());
    assert!(r.m.edit.question_cache.page(CID, BID, &before).is_some());
    r.script_load();
    r.m.edit.update_link(LOCAL, |l| {
        l.role_seen = Some("viewer".into());
        true
    });
    r.page(1280.0);
    assert!(r.m.edit.question_cache.page(CID, BID, &before).is_none());
    assert_eq!(list_reads(&r.w), 2, "read again under the new role");
}

#[test]
fn d2f_the_pickers_list_versions_newest_first() {
    // (critic P13) whatever order the lines came in
    let mut r = rig();
    let mut l = lines_json();
    l["versions"].as_array_mut().unwrap().reverse();
    r.w.on("Get", &versions_path(), 200, l, None);
    r.w.on("Get", &changes_path(), 200, changes_json(), None);
    r.open(0);
    r.page(1280.0);
    assert_eq!(picker_sides(r.qpage()), vec![Side::Version(5), Side::Version(3), Side::Version(2), Side::YourCopy]);
}

#[test]
fn d2f_the_bankui_hook_sits_in_the_question_cards_menu() {
    // the one H3 line is inside draw_card's menu (right-click, or Shift+F10 from the keyboard), before the card's own click
    let src = include_str!("../bankui.rs");
    let card = src.find("fn draw_card(").expect("draw_card");
    let menu = card + src[card..].find("egui::Popup::context_menu(&hr)").expect("the card's menu");
    let after = menu + src[menu..].find("if hr.clicked()").expect("the card's click");
    let hook = src.find("crate::share::question_history_ui::question_menu(").expect("hook H3");
    assert!(menu < hook && hook < after, "H3 lies in the menu");
    assert_eq!(src.matches("question_history_ui::question_menu(").count(), 1);
    // (round 6, critic) and the menu has a keyboard route: Shift+F10
    let keyed = src.find("egui::Key::F10").expect("a keyboard route to the card's menu");
    assert!(card < keyed && keyed < hook, "the keyboard route opens the menu the hook is in");
    assert_eq!(src.matches("egui::Key::F10").count(), 1);
    // (round 7, critic M7) the hint is on the card itself, not only inside the menu it describes
    let hint = src.find("Right-click for Duplicate, Delete and History in the circle").expect("a hint on the card");
    assert!(card < hint && hint < menu, "the hint sits on the card, before the menu it describes");
    // (round 8, critic P) and it promises only what an unlinked bank's menu holds
    let unlinked = src.find("Right-click for Duplicate and Delete; Shift+F10").expect("the unlinked banks' hint");
    assert!(hint < unlinked && unlinked < menu, "both hints sit on the card, before the menu");
}

#[test]
fn d2f_compare_columns_each_carry_their_field_and_the_rows_buttons_sit_right() {
    // (critic P12)
    for w in BOTH {
        let mut r = rig();
        r.script_load();
        r.w.raw(&version_path(3), served(3, &[qj(QA, A3), qj(QB, B)], false));
        r.w.raw(&version_path(5), served(5, &[qj(QA, A5), qj(QB, B)], false));
        r.open(0);
        r.page(w);
        if w > 1000.0 {
            let rects = text_rects(&mut r, w);
            let restore = rects.iter().find(|(t, _)| t == "Restore...").map(|(_, r)| r.max.x).unwrap();
            assert!(restore > w - 80.0, "the row's buttons sit at its right: {restore}");
        }
        assert!(r.click(w, "Compare", 0));
        let t = r.page(w);
        let (names, pills) = (t.matches("Question text").count(), t.lines().filter(|l| *l == "changed").count());
        if compare_columns(w) {
            assert_eq!((names, pills), (2, 2), "each column has its field: {t}");
        } else {
            assert_eq!((names, pills), (1, 1), "stacked: the field once, both sides under it: {t}");
        }
    }
}


#[test]
fn d2f_an_answer_without_lists_is_bank_lines_even_when_the_link_does_not_know_yet() {
    // (round 2 test, round 3 rig) an edit link whose last check did not see the hiding yet (a demoted author, a PATCH since):
    // the pool's per_question:false is followed
    let mut r = rig();
    r.w.on("Get", &versions_path(), 200, lines_json(), None);
    let mut bad = changes_json();
    bad["per_question"] = json!(false);
    r.w.on("Get", &changes_path(), 200, bad, None);
    r.open(0);
    let t = r.page(1280.0);
    assert!(!t.contains("Changed") && !t.contains("Changes between") && t.contains("hides its answer keys"), "{t}");
}

#[test]
fn d2f_a_full_body_held_is_never_compared_beside_the_viewers_form() {
    // (critic B1) a body read in full is in the cache under the page's own scope; the page says viewers' form
    let mut r = hiding_viewer();
    r.w.on("Get", &versions_path(), 200, lines_json(), None);
    r.w.on("Get", &changes_path(), 200, json!({"bank_id": BID, "head_version": 5, "per_question": false, "reason": "keys_hidden"}), None);
    r.open(0);
    r.page(1280.0);
    let scope = super::question_history::scope_of(&r.m.edit.link_for_local(LOCAL).unwrap());
    let full = String::from_utf8(served(5, &[qk(QA, A5, true), qj(QB, B)], false)).unwrap();
    let payload = full.split("\"payload\":").nth(1).unwrap().trim_end_matches('}').to_string();
    r.m.edit.question_cache.put_body(CID, BID, &scope, super::question_history::VersionBody::from_payload(5, payload.as_bytes(), false, NOW));
    // what the pool serves now: the viewers' form
    r.w.raw(&version_path(3), served(3, &[qs(QA, A3), qs(QB, B)], true));
    r.w.raw(&version_path(5), served(5, &[qs(QA, A5), qs(QB, B)], true));
    assert!(r.click(1280.0, "Compare", 0));
    let t = r.page(1280.0);
    assert!(!t.contains("SECRET") && !t.contains("choice 1"), "{t}");
    assert_eq!(r.w.calls("/versions/5"), 1, "read again in the viewers' form");
}

// ---- round 4 (second critic pair) -------------------------------------------------------------------------------------

#[test]
fn zz_undo_overwrites_an_edit_made_after_the_restore() {
    // (critic S-I2, ported) an edit after the restore is never undone
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Restore...", 0));
    assert!(r.click(1280.0, "Yes, restore", 0));
    r.page(1280.0);
    assert_eq!(r.banks[0].questions[0].stem, A3);
    // an edit after the restore (pop-over window, AI job, any other writer)
    r.banks[0].questions[0].stem = "Edited after the restore".into();
    r.banks[0].modified += 5;
    r.page(1280.0);
    let t = r.page(1280.0);
    assert!(t.contains("Undo restore"), "undo still offered after an edit: {t}");
    assert!(r.click(1280.0, "Undo restore", 0));
    let t = r.page(1280.0);
    assert_eq!(r.banks[0].questions[0].stem, "Edited after the restore", "undo silently overwrote the later edit");
    assert!(t.contains("changed after the restore"), "{t}");
}

#[test]
fn zz_undo_writes_back_a_stale_whole_link() {
    // (critic S-I1, ported) undo puts back only the restore's own change to the link
    let mut r = removed_rig();
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Restore...", 0));
    assert!(r.click(1280.0, "Yes, restore", 0));
    r.page(1280.0);
    // something else moves the link on meanwhile (a quiet check, settle_all_news, ...)
    r.m.edit.update_link(LOCAL, |l| {
        l.head_seen = 9;
        l.head_seen_by = Some("C. Teacher".into());
        true
    });
    r.page(1280.0);
    assert!(r.click(1280.0, "Undo restore", 0));
    r.page(1280.0);
    let l = r.m.edit.link_for_local(LOCAL).unwrap();
    assert_eq!(l.head_seen, 9, "undo reverted unrelated link state");
    assert_eq!(l.qid_of(&r.banks[0].questions[0].id), Some(QA), "the pair is back");
}

#[test]
fn zz_page_survives_its_link_pointing_at_another_circle_bank() {
    // (critic S-m1, ported)
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Restore...", 0));
    // the local bank is now linked to another circle bank (re-linked)
    r.m.edit.update_link(LOCAL, |l| {
        l.circle_bank_id = "bk_other000000000000".into();
        true
    });
    assert!(r.click(1280.0, "Yes, restore", 0) || !matches!(r.m.edit.view, EditView::Question(_)));
    r.page(1280.0);
    let open = matches!(r.m.edit.view, EditView::Question(_));
    assert!(!open || r.banks[0].questions[0].stem != A3, "old circle bank's version restored into a bank linked elsewhere");
    assert!(!open, "the page closes");
}

#[test]
fn d2g_the_page_does_not_open_while_a_question_of_the_bank_is_open_in_the_editor() {
    // (critic S-I3) its draft would later be written over a restored question
    let mut r = rig();
    let bank = r.banks[0].clone();
    let id = bank.questions[0].id.clone();
    let t = super::bank_links_tests::text_of(1280.0, &mut |ui| {
        assert!(!super::question_history_ui::question_menu_of(ui, &mut r.m, &bank, &id, true));
    });
    assert!(t.contains(super::question_history_ui::DRAFT_OPEN), "{t}");
    assert!(!r.m.edit.active());
    assert!(r.w.reads().is_empty());
}

#[test]
fn d2g_answers_asked_before_a_reset_are_never_applied_after_it() {
    // (critic S-I4, S-m2) a version read for olga, answered after pete signed in on this session
    let mut r = rig();
    *r.w.who.lock().unwrap() = Some("https://pool.test\nolga@school.org".into());
    r.script_load();
    r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
    r.open(0);
    r.page(1280.0);
    r.qpage_mut().ask_restore(2);
    let cache = &mut r.m.edit.question_cache;
    if let EditView::Question(p) = &mut r.m.edit.view {
        p.confirm_restore(cache);
    }
    // the answer is waiting, not yet taken in; another account now
    *r.w.who.lock().unwrap() = Some("https://pool.test\npete@school.org".into());
    r.script_load();
    r.page(1280.0);
    assert_eq!(r.m.edit.question_cache.bodies(), 0, "olga's read is not kept for pete");
    assert_eq!(r.banks[0].questions[0].stem, A5, "nor restored for him");
    assert!(r.qpage().restore.is_none(), "the restore under way is gone, so the Restore buttons work again");
}

#[test]
fn d2g_a_first_answer_for_a_bank_drops_what_was_held_from_before() {
    // (critic S-m5) a body held from earlier in the session, its bank's lists since evicted
    let mut r = rig();
    let scope = super::question_history::scope_of(&r.m.edit.link_for_local(LOCAL).unwrap());
    r.frame_at(1280.0, Vec::new());
    r.frame_at(1280.0, Vec::new());
    let scope_now = super::question_history::scope_of(&r.m.edit.link_for_local(LOCAL).unwrap());
    let _ = scope;
    r.m.edit.question_cache.put_body(CID, BID, &scope_now, super::question_history::VersionBody::from_payload(3, &[], false, NOW));
    r.script_load();
    r.open_cold(0);
    r.page(1280.0);
    assert!(r.m.edit.question_cache.body(CID, BID, &scope_now, 3).is_none());
}

#[test]
fn d2g_a_figure_only_restore_is_refused_and_the_copy_agrees_with_the_strip() {
    // (critic P-2) version 2 differs from the copy only in a figure's settings
    let mut r = rig();
    r.script_load();
    let mut q = qj(QA, A5);
    q["figure"] = json!({"id": "triangle", "params": {"a": 4}});
    r.w.raw(&version_path(2), served(2, &[q], false));
    r.banks[0].questions[0].figure = Some(crate::model::FigureSpec { id: "triangle".into(), params: [("a".to_string(), json!(3))].into_iter().collect() });
    r.banks[0].modified += 1;
    r.open(0);
    r.page(1280.0);
    let before = serde_json::to_string(&r.banks).unwrap();
    assert!(r.click(1280.0, "Restore...", 0));
    assert!(r.click(1280.0, "Yes, restore", 0));
    let t = r.page(1280.0);
    assert!(t.contains("figure is not the one your copy is based on"), "{t}");
    assert_eq!(serde_json::to_string(&r.banks).unwrap(), before);
    assert!(!t.contains("changed, not sent yet"), "the strip counts none: {t}");
    assert_eq!(r.m.edit.status(LOCAL).map(|s| s.changes.total()), Some(0));
}

#[test]
fn d2g_a_compare_with_your_copy_follows_the_copy_and_waits_for_a_restore() {
    // (critic P-3)
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(5), served(5, &[qj(QA, A5), qj(QB, B)], false));
    r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Compare with...", 0));
    assert!(r.page(1280.0).contains("The two are the same"));
    r.banks[0].questions[0].stem = "Edited while comparing".into();
    r.banks[0].modified += 1;
    let t = r.page(1280.0);
    assert!(!t.contains("The two are the same") && t.lines().any(|l| l == "changed") && t.lines().any(|l| l == "Edited while comparing"), "worked out again: {t}");
    assert!(r.click(1280.0, "Close compare", 0));
    // while a restore is asked, Compare waits
    assert!(r.click(1280.0, "Restore...", 0));
    assert!(r.click(1280.0, "Compare", 0));
    assert!(r.qpage().compare.is_none(), "{:?}", r.qpage().compare);
    // the result and its Undo stay in view over a compare
    r.w.raw(&version_path(3), served(3, &[qj(QA, A3), qj(QB, B)], false));
    assert!(r.click(1280.0, "Yes, restore", 0));
    r.page(1280.0);
    assert!(r.click(1280.0, "Compare", 0));
    let t = r.page(1280.0);
    assert!(r.qpage().compare.is_some() && t.contains("Undo restore"), "{t}");
}

#[test]
fn d2g_undo_says_how_long_it_lasts_and_survives_a_second_restore_cancelled() {
    // (critic P-4)
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
    r.w.raw(&version_path(5), served(5, &[qj(QA, A5), qj(QB, B)], false));
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Restore...", 0));
    assert!(r.click(1280.0, "Yes, restore", 0));
    let t = r.page(1280.0);
    assert!(t.contains("Undo is available until you leave this page"), "{t}");
    // (round 4, minor) version 2 is what the copy is now: no Restore on it; version 5 offers one
    assert!(t.contains("your copy is now this"), "{t}");
    assert!(r.click(1280.0, "Restore...", 0));
    assert!(r.click(1280.0, "Cancel", 0));
    let t = r.page(1280.0);
    assert!(t.contains("Undo restore"), "{t}");
}

#[test]
fn d2g_show_draws_a_versions_whole_question() {
    // (critic P-6)
    let mut r = rig();
    r.script_load();
    let mut q = qj(QA, A3);
    q["explanation"] = json!("Subtract 3, then divide by 2.");
    q["topics"] = json!(["linear equations"]);
    r.w.raw(&version_path(2), served(2, &[q], false));
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Show", 1));
    let t = r.page(1280.0);
    for want in ["Version 2, as it was", A3, "Choices: A | B", "Answer key: choice 1", "Topics: linear equations", "Explanation: Subtract 3, then divide by 2.", "Picture: no picture"] {
        assert!(t.contains(want), "{want}: {t}");
    }
    assert!(r.click(1280.0, "Hide", 0));
    assert!(r.qpage().preview.is_none());
}

#[test]
fn d2g_a_compare_with_a_missing_side_shows_the_other_once() {
    // (critic P-9) version 2 has no B
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
    r.w.raw(&version_path(3), served(3, &[qj(QA, A3), qj(QB, B)], false));
    r.open(1);
    r.page(1280.0);
    let cache = &mut r.m.edit.question_cache;
    if let EditView::Question(p) = &mut r.m.edit.view {
        p.open_compare(Side::Version(2), Side::Version(3), cache);
    }
    let t = r.page(1280.0);
    assert_eq!(t.matches("Version 2 does not have this question.").count(), 1, "{t}");
    assert!(t.contains(&format!("Question text: {B}")) && !t.contains("Version 2: \n"), "{t}");
}

#[test]
fn d2g_a_viewers_form_compare_names_what_it_did_not_compare() {
    // (critic P-8)
    let mut r = hiding_viewer();
    r.w.on("Get", &versions_path(), 200, lines_json(), None);
    r.w.on("Get", &changes_path(), 200, json!({"bank_id": BID, "head_version": 5, "per_question": false, "reason": "keys_hidden"}), None);
    r.w.raw(&version_path(3), served(3, &[qs(QA, A3), qs(QB, B)], true));
    r.w.raw(&version_path(5), served(5, &[qs(QA, A5), qs(QB, B)], true));
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Compare", 0));
    let t = r.page(1280.0);
    assert!(t.contains("Answer key, explanation: hidden from viewers, not compared"), "{t}");
    assert!(!t.contains("answer key, points") && !t.contains("explanation, picture"), "not called the same: {t}");
}

#[test]
fn d2g_the_empty_state_with_gaps_says_nothing_is_known_to_have_changed_it() {
    // (critic P-10)
    let mut r = rig();
    r.w.on("Get", &versions_path(), 200, lines_json(), None);
    r.w.on("Get", &changes_path(), 200, json!({"bank_id": BID, "head_version": 5, "per_question": true, "versions": [{"version": 5, "compared_to": 3, "gap": true}, {"version": 3, "compared_to": 2, "gap": true}]}), None);
    r.open(0);
    let t = r.page(1280.0);
    assert!(t.contains("No kept version is known to have changed this question.") && !t.contains("No version that is still kept changed"), "{t}");
}

#[test]
fn d2g_escape_only_cancels_a_question_in_view_and_the_oldest_viewer_row_compares_with_the_copy() {
    // (round 4, minors)
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
    r.w.raw(&version_path(3), served(3, &[qj(QA, A3), qj(QB, B)], false));
    r.w.raw(&version_path(5), served(5, &[qj(QA, A5), qj(QB, B)], false));
    r.open(0);
    r.page(1280.0);
    r.qpage_mut().ask_restore(2);
    let cache = &mut r.m.edit.question_cache;
    if let EditView::Question(p) = &mut r.m.edit.view {
        p.open_compare(Side::Version(3), Side::Version(5), cache);
    }
    r.page(1280.0);
    r.frame_at(1280.0, vec![egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }]);
    r.page(1280.0);
    assert!(r.qpage().restore.is_some(), "a question behind the compare is not cancelled unseen");
    // a viewer of a hiding bank: the oldest kept version compares with the copy, and says so
    let mut r = hiding_viewer();
    r.w.on("Get", &versions_path(), 200, lines_json(), None);
    r.w.on("Get", &changes_path(), 200, json!({"bank_id": BID, "head_version": 5, "per_question": false, "reason": "keys_hidden"}), None);
    r.open(0);
    let t = r.page(1280.0);
    assert_eq!(t.matches("Compare with your copy").count(), 1, "{t}");
}

#[test]
fn d2g_a_list_answered_after_a_role_change_is_not_taken_for_the_new_role() {
    // (critic S-I4) the page's first read is held at the pool while the role changes; the read for the new role is answered
    // first, then the held one: that one belongs to the old role and is never used
    let mut r = rig();
    let media: super::publish::SharedMedia = Arc::new(HashMap::<String, Vec<u8>>::new());
    let links = r.m.edit.links().to_vec();
    r.m.edit = CircleEditModel::new(Runner::Background, r.pics.clone(), media, r.restores.clone(), LinkLog::memory()).with_retry(RetryPolicy::none());
    r.m.edit.set_clock(NOW);
    for l in links {
        r.m.edit.add_link(l).unwrap();
    }
    r.frame_at(1280.0, Vec::new());
    r.frame_at(1280.0, Vec::new());
    // the old role's read: its lines (its lists, tag e1, are queued below: it asks for them only once released)
    r.w.on("Get", &versions_path(), 200, lines_json(), None);
    let release = r.w.hold(&format!("Get {}", versions_path()));
    r.open_cold(0);
    // (round 5) deterministic: the old read has asked for its lines (and so holds the gate) before anything else is asked
    for _ in 0..2000 {
        if r.w.gate.lock().unwrap().is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(r.w.gate.lock().unwrap().is_none(), "the old read waits at the gate");
    assert_eq!(list_reads(&r.w), 1);
    // (the circle's own checks say viewer from now on too)
    r.w.keep("Get", "/circles", circles_list("viewer"));
    r.w.keep("Get", &format!("/circles/{CID}"), detail("viewer", json!({})));
    r.m.edit.update_link(LOCAL, |l| {
        l.role_seen = Some("viewer".into());
        true
    });
    // the new role's read
    r.w.on("Get", &versions_path(), 200, lines_json(), None);
    r.w.on("Get", &changes_path(), 200, json!({"bank_id": BID, "head_version": 5, "per_question": true, "versions": []}), Some("\"viewer-1\""));
    r.w.on("Get", &changes_path(), 200, changes_json(), Some("\"e1\""));
    let s = super::question_history::scope_of(&r.m.edit.link_for_local(LOCAL).unwrap());
    for _ in 0..1000 {
        r.page(1280.0);
        if r.m.edit.question_cache.page(CID, BID, &s).is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(r.m.edit.question_cache.page(CID, BID, &s).and_then(|p| p.etag.clone()).as_deref(), Some("\"viewer-1\""));
    // now the old read's answer arrives
    release.send(()).unwrap();
    // (until the old read's answer is in: its job is no longer waiting)
    for _ in 0..500 {
        r.page(1280.0);
        if !r.qpage().busy() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    r.page(1280.0);
    assert_eq!(r.m.edit.question_cache.page(CID, BID, &s).and_then(|p| p.etag.clone()).as_deref(), Some("\"viewer-1\""), "the old role's answer is not used");
    assert!(!r.page(1280.0).contains("Changed"), "nor drawn");
}


// ---- round 5 (third critic pair): the critics' probes, ported to assertions ----------------------------------------

#[test]
fn probe_u1_asking_with_a_failing_read_loops() {
    // (critic I2) the question's read fails: read once, the error said with Try again, Yes waits for the version
    let mut r = rig();
    r.script_load();
    for _ in 0..200 {
        r.w.on("Get", &version_path(2), 503, json!({"error": "busy"}), None);
    }
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Restore...", 0));
    let n0 = r.w.calls("/versions/2");
    for _ in 0..20 {
        r.page(1280.0);
    }
    let t = r.page(1280.0);
    assert_eq!(r.w.calls("/versions/2"), n0, "read once per ask");
    assert_eq!(n0, 1);
    assert!(!t.contains("Reading version 2...") && t.contains("Try again"), "{t}");
    // Yes waits for the version
    assert!(r.click(1280.0, "Yes, restore", 0));
    assert_eq!(r.qpage().restore.as_ref().map(|x| x.state.clone()), Some(RestoreState::Asking));
    // Try again reads once more
    assert!(r.click(1280.0, "Try again", 0));
    assert_eq!(r.w.calls("/versions/2"), 2);
}

#[test]
fn critic3_restore_question_rereads_a_failing_version_every_frame() {
    // (critic S1, ported) a persistent failure (not a version body)
    let mut r = rig();
    r.script_load();
    r.w.keep("Get", &version_path(2), json!({"nope": true}));
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Restore...", 0));
    let after_click = r.w.calls("/versions/2");
    for _ in 0..30 {
        r.frame_at(1280.0, Vec::new());
    }
    let n = r.w.calls("/versions/2");
    assert!(n <= after_click + 1, "the confirmation re-reads the version every frame: {after_click} -> {n}");
}

#[test]
fn probe_u2_show_failure_spins_forever() {
    // (critic I6) a failed Show says so, with Try again
    let mut r = rig();
    r.script_load();
    r.w.on("Get", &version_path(5), 503, json!({"error": "busy"}), None);
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Show", 0));
    for _ in 0..10 {
        r.page(1280.0);
    }
    let t = r.page(1280.0);
    assert!(!t.contains("Reading version 5...") && t.contains("Try again"), "{t}");
    assert_eq!(r.w.calls("/versions/5"), 1);
    r.w.raw(&version_path(5), served(5, &[qj(QA, A5), qj(QB, B)], false));
    assert!(r.click(1280.0, "Try again", 0));
    let t = r.page(1280.0);
    assert!(t.contains("Version 5, as it was"), "{t}");
}

#[test]
fn probe_u3_restore_base_says_unsent() {
    // (critic I1) restoring the version the copy is based on: the copy is in step again, and the Done words say so
    let mut r = rig();
    r.script_load();
    r.banks[0].questions[0].stem = "My own unsent edit".into();
    r.banks[0].modified += 10;
    r.w.raw(&version_path(5), served(5, &[qj(QA, A5), qj(QB, B)], false));
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Restore...", 0));
    assert!(r.click(1280.0, "Yes, restore", 0));
    let t = r.page(1280.0);
    assert!(!t.contains("counts as an unsent change") && t.contains("the same as version 5"), "{t}");
    assert!(r.click(1280.0, "< Back to Unit 2 quiz", 0));
    r.page(1280.0);
    assert_eq!(r.m.edit.status(LOCAL).map(|s| s.changes.total()), Some(0));
}

#[test]
fn probe_u4_try_again_after_a_failed_undo_asks_to_restore() {
    // (critic I4) a failed Undo is the undo's own: a lasting reason has no Try again, a passing one retries the undo
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Restore...", 0));
    assert!(r.click(1280.0, "Yes, restore", 0));
    r.page(1280.0);
    // something changes the question after the restore
    r.banks[0].questions[0].difficulty = Some(crate::model::Difficulty::High);
    r.banks[0].modified += 5;
    assert!(r.click(1280.0, "Undo restore", 0));
    let t = r.page(1280.0);
    assert!(!t.contains("Try again"), "a lasting reason: {t}");
    assert!(!r.click(1280.0, "Try again", 0));
    // a restore point that can not be saved for now: Try again undoes, never restores again
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Restore...", 0));
    assert!(r.click(1280.0, "Yes, restore", 0));
    r.page(1280.0);
    r.restores.1.store(true, std::sync::atomic::Ordering::Relaxed);
    assert!(r.click(1280.0, "Undo restore", 0));
    let t = r.page(1280.0);
    assert!(t.contains("disk is full") && t.contains("Try again"), "{t}");
    r.restores.1.store(false, std::sync::atomic::Ordering::Relaxed);
    assert!(r.click(1280.0, "Try again", 0));
    let t = r.page(1280.0);
    assert!(!t.contains("Replace your copy of question 1 with version 2's?"), "Try again asks to restore: {t}");
    assert_eq!(r.banks[0].questions[0].stem, A5, "the undo was made");
}

#[test]
fn probe_u5_dismiss_loses_undo() {
    // (critic I5) Dismiss leaves Undo reachable until the page is left
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Restore...", 0));
    assert!(r.click(1280.0, "Yes, restore", 0));
    r.page(1280.0);
    assert!(r.click(1280.0, "Dismiss", 0));
    let t = r.page(1280.0);
    assert!(t.contains("Undo restore"), "{t}");
    assert!(r.click(1280.0, "Undo restore", 0));
    r.page(1280.0);
    assert_eq!(r.banks[0].questions[0].stem, A5);
}

#[test]
fn critic3_show_draws_a_stripped_body_as_if_it_had_its_key() {
    // (critic S2, ported) hiding turned on after the lists were read: the body comes stripped
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], true));
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Show", 1));
    let t = r.page(1280.0);
    assert!(!t.contains("Answer key: none marked") && !t.contains("Answer key: no key") && !t.contains("Answer key: choice"), "a stripped body's key drawn as a real one:\n{t}");
}

#[test]
fn critic3_a_flood_of_history_lines_hangs_the_page() {
    // (critic S3, ported) 20,000 history lines
    let mut r = rig();
    let n: u64 = 20000;
    let lines: Vec<Value> = (1..=n).rev().map(|v| json!({"version": v, "kept": true, "withheld": false, "by": "x", "at": NOW, "message": "m"})).collect();
    r.w.on("Get", &versions_path(), 200, json!({"bank_id": BID, "head_version": n, "versions": lines}), None);
    r.w.on("Get", &changes_path(), 200, json!({"bank_id": BID, "head_version": n, "per_question": true, "versions": []}), Some("\"e1\""));
    r.open(0);
    let t = std::time::Instant::now();
    r.frame_at(1280.0, Vec::new());
    let first = t.elapsed();
    r.qpage_mut().show_all = true;
    let t = std::time::Instant::now();
    r.frame_at(1280.0, Vec::new());
    r.frame_at(1280.0, Vec::new());
    assert!(first < std::time::Duration::from_millis(300), "first frame {first:?}");
    assert!(t.elapsed() < std::time::Duration::from_millis(500), "{:?}", t.elapsed());
}

#[test]
fn d2h_a_restore_waits_while_a_question_of_the_bank_is_open_in_the_editor() {
    // (critic I3) the pop-over or the exams page opened an editor draft for this bank while the page was open
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Restore...", 0));
    r.m.edit.set_question_draft(Some(LOCAL));
    let cache = &mut r.m.edit.question_cache;
    if let EditView::Question(p) = &mut r.m.edit.view {
        p.confirm_restore(cache);
    }
    let t = r.page(1280.0);
    assert_eq!(r.banks[0].questions[0].stem, A5, "not restored under an open draft");
    assert!(t.contains(super::question_history_ui::DRAFT_OPEN), "{t}");
    // the draft closed: asked again, then made
    r.m.edit.set_question_draft(None);
    r.qpage_mut().ask_restore(2);
    r.page(1280.0);
    assert!(r.click(1280.0, "Yes, restore", 0));
    r.page(1280.0);
    assert_eq!(r.banks[0].questions[0].stem, A3);
    // and Undo waits for a draft too
    r.m.edit.set_question_draft(Some(LOCAL));
    assert!(r.click(1280.0, "Undo restore", 0));
    let t = r.page(1280.0);
    assert_eq!(r.banks[0].questions[0].stem, A3);
    assert!(t.contains(super::question_history_ui::DRAFT_OPEN), "{t}");
}

#[test]
fn d2h_a_confirmed_restore_is_made_only_while_its_page_is_shown() {
    // (critic I3) the bank's page left after Yes, before the version came: another page's sync does not make it unseen
    let mut r = rig();
    r.script_load();
    r.open(0);
    r.page(1280.0);
    r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
    r.qpage_mut().ask_restore(2);
    r.page(1280.0);
    let cache = &mut r.m.edit.question_cache;
    if let EditView::Question(p) = &mut r.m.edit.view {
        p.confirm_restore(cache);
    }
    // frames pass with the page not drawn (another screen), the circle model synced from elsewhere
    for k in 0..5u64 {
        r.m.edit.set_pass(10_000 + k);
        r.m.edit.sync(&mut r.banks);
    }
    assert_eq!(r.banks[0].questions[0].stem, A5, "not made while nobody sees it");
}

#[test]
fn d2h_the_back_label_follows_a_renamed_bank() {
    let mut r = rig();
    r.script_load();
    r.open(0);
    r.page(1280.0);
    r.banks[0].name = "Unit 2 quiz (renamed)".into();
    r.banks[0].modified += 1;
    let t = r.page(1280.0);
    assert!(t.contains("< Back to Unit 2 quiz (renamed)"), "{t}");
}

#[test]
fn d2h_show_every_version_is_offered_whenever_versions_are_hidden() {
    // (round 5) no version is "no longer kept", but kept versions that did not change the question are not listed
    let mut r = rig();
    let mut l = lines_json();
    l["versions"].as_array_mut().unwrap().pop();
    r.w.on("Get", &versions_path(), 200, l, None);
    r.w.on("Get", &changes_path(), 200, changes_json(), Some("\"e1\""));
    r.open(0);
    let t = r.page(1280.0);
    assert!(!t.contains("no longer kept in full") && t.contains("Show every version"), "{t}");
    assert!(r.click(1280.0, "Show every version", 0));
    assert!(r.page(1280.0).contains("Version 3"));
}

#[test]
fn d2h_show_compares_nothing_and_says_what_the_picture_is() {
    // (round 5) one version's question: no "the same picture", no "not compared"
    let mut r = rig();
    r.script_load();
    let sha = sha256_hex(b"pic");
    r.w.raw(&version_path(2), served(2, &[pictured(&sha)], false));
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Show", 1));
    let t = r.page(1280.0);
    assert!(t.contains("Picture: a picture") && !t.contains("the same picture") && !t.contains("not compared"), "{t}");
}

// ---- throwaway critic probes (round 6) ---------------------------------------------------------------------------------

#[test]
fn d2j_a_restore_is_not_made_from_another_page_after_leaving() {
    // (round 6, critic N3) a restore confirmed just before the person leaves (its version still on its way) is not made by
    // the next frame's sync from the Shared page: only the frame that draws its own page may make it, so its Done text and
    // Undo are never lost.
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
    r.open(0);
    r.page(1280.0);
    // Yes, restore pressed while the version is still on its way (Inline runner: the answer waits in the channel, as a
    // background read that arrives just after the page's last frame)
    let cache = &mut r.m.edit.question_cache;
    match &mut r.m.edit.view {
        EditView::Question(p) => {
            p.ask_restore(2);
            p.confirm_restore(cache);
        }
        _ => panic!(),
    }
    // the person goes to the Shared page: its frame runs the circle model's sync (shared_ui), the bank page is not drawn
    let next = r.ctx.cumulative_frame_nr();
    r.m.edit.set_pass(next);
    let _ = r.m.edit.sync(&mut r.banks);
    let applied = r.banks[0].questions[0].stem == A3;
    // back on the bank a few frames later: the page is closed by the stale-frame rule, its Done text and Undo gone
    r.m.edit.set_pass(next + 5);
    let t = r.page(1280.0);
    assert!(!t.contains("Undo restore") || !applied, "a restore was made away from its page yet its undo never shown:\n{t}");
    assert!(!applied, "a restore was made while its page was not in front of the person");
}

#[test]
fn d2j_a_restore_is_not_made_by_another_banks_strip() {
    // (round 6, critic N3) the draft flag is one value for the whole model: the last bank page drawn sets it. Another
    // linked bank's strip syncing the model must not make this bank's confirmed restore (or skip its draft guard).
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
    r.open(0);
    r.page(1280.0);
    r.m.edit.set_question_draft(Some(LOCAL));
    // a bank page for another bank ("other") is drawn next: its strip says "no draft" for it
    r.m.edit.set_question_draft(Some("other"));
    let cache = &mut r.m.edit.question_cache;
    match &mut r.m.edit.view {
        EditView::Question(p) => {
            p.ask_restore(2);
            p.confirm_restore(cache);
        }
        _ => panic!(),
    }
    r.m.edit.set_question_draft(None);
    let next = r.ctx.cumulative_frame_nr();
    r.m.edit.set_pass(next);
    let _ = r.m.edit.sync(&mut r.banks);
    assert_eq!(r.banks[0].questions[0].stem, A5, "another bank's strip made this bank's restore");
}

#[test]
fn d2j_a_restore_is_not_made_by_another_linked_banks_strip() {
    // (round 6, critic N3) a restore waiting for its picture while the person clicks another linked bank: its strip sets the
    // pass and syncs the same model. The restore is only made (and its result and Undo shown) by its own page's frame.
    let bytes = b"the picture of version 2".to_vec();
    let sha = sha256_hex(&bytes);
    let blob = format!("/circles/{CID}/banks/{BID}/blobs/{sha}");
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(2), served(2, &[pictured(&sha)], false));
    r.w.raw(&blob, bytes.clone());
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Restore...", 0));
    let cache = &mut r.m.edit.question_cache;
    if let EditView::Question(p) = &mut r.m.edit.view {
        p.confirm_restore(cache);
    }
    // one frame of the page: the restore asks for the picture ("Reading version 2...")
    let t: String = r.frame_at(1280.0, Vec::new()).into_iter().map(|(t, _)| t).collect::<Vec<_>>().join("\n");
    assert!(t.contains("Reading version 2"), "{t}");
    assert_eq!(r.banks[0].questions[0].stem, A5, "not yet");
    // the person clicks another linked bank: that page's strip sets the pass and syncs, as strip_with does
    let f = r.m.edit.host_frame.unwrap();
    r.m.edit.set_pass(f + 1);
    r.m.edit.sync(&mut r.banks);
    let applied = r.banks[0].questions[0].stem == A3;
    // more frames elsewhere, then back to the bank
    for _ in 0..3 {
        let input = r.input(1280.0, Vec::new());
        let _ = r.ctx.run_ui(input, |_| {});
    }
    let t = r.page(1280.0);
    assert!(!applied || t.contains("Undo restore"), "restore applied while the person was on another bank's page, and its result and Undo were never shown: {t}");
}

#[test]
fn d2j_undo_survives_a_role_or_hide_change_on_the_page() {
    // (round 6, critic N4) the owner turns on hiding of keys while she is still on the page (an editor: still per-question
    // lists). What was read under the old scope is dropped and read again, but the last restore's Undo and its words stay:
    // she never left the page.
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Restore...", 0));
    assert!(r.click(1280.0, "Yes, restore", 0));
    let t = r.page(1280.0);
    assert!(t.contains("Undo is available until you leave this page"), "{t}");
    r.script_load();
    r.m.edit.update_link(LOCAL, |l| {
        l.seen.hide_key = true;
        true
    });
    let t = r.page(1280.0);
    assert!(r.m.edit.active(), "still on the page");
    assert!(t.contains("Undo restore"), "she never left the page, but Undo is gone: {t}");
    // and it still undoes, over the re-read page
    assert!(r.click(1280.0, "Undo restore", 0));
    let _ = r.page(1280.0);
    assert_eq!(r.banks[0].questions[0].stem, A5);
}

#[test]
fn d2j_another_account_never_keeps_the_undo() {
    // (round 6, critic N4) the other side of it: a page whose account changed drops the last restore's Undo with the reads
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Restore...", 0));
    assert!(r.click(1280.0, "Yes, restore", 0));
    assert!(r.page(1280.0).contains("Undo restore"));
    r.who = "someone-else".into();
    r.script_load();
    let t = r.page(1280.0);
    assert!(!t.contains("Undo restore"), "another account saw the first one's Undo: {t}");
}

#[test]
fn d2k_a_difficulty_pass_never_writes_over_a_restored_question() {
    // (round 6, critic N5) version 2's question is rated Low; the person restores it. A difficulty pass learnt from the
    // circle afterwards (inherit, its first pass not yet run) would set the base version's rating (Hard) over it. The
    // restore's rating wins until the person changes it: the pass leaves a restored question alone.
    let mut r = rig();
    r.script_load();
    let mut q = qj(QA, A3);
    q["difficulty"] = json!("Low");
    r.w.raw(&version_path(2), served(2, &[q], false));
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Restore...", 0));
    assert!(r.click(1280.0, "Yes, restore", 0));
    r.page(1280.0);
    assert_eq!(r.banks[0].questions[0].difficulty, Some(crate::model::Difficulty::Low));
    // the circle rated the base version Hard, and a read has just learnt its difficulties: the pass is to run
    let hard = Some(Some(crate::model::Difficulty::High));
    r.m.edit.update_link(LOCAL, |l| {
        l.base[0].snap.difficulty = hard;
        l.inherit = true;
        l.at_read = None;
        true
    });
    r.page(1280.0);
    assert_eq!(r.banks[0].questions[0].difficulty, Some(crate::model::Difficulty::Low), "the difficulty pass wrote over the restore");
    // without that guard the very same pass does turn it to the circle's: the test above is not passing for nothing
    r.m.edit.update_link(LOCAL, |l| {
        l.restored.clear();
        l.base[0].snap.difficulty = hard;
        l.inherit = true;
        l.at_read = None;
        true
    });
    r.page(1280.0);
    assert_eq!(r.banks[0].questions[0].difficulty, Some(crate::model::Difficulty::High));
}

// ---- round 6, the "should fix" items -----------------------------------------------------------------------------------

#[test]
fn d2l_show_waits_while_a_restore_is_asked() {
    // (round 6, critic) Show reads a version the restore's own read would mix with: it waits while a restore is asked.
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
    r.w.raw(&version_path(5), served(5, &[qj(QA, A5)], false));
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Restore...", 0));
    r.click(1280.0, "Show", 0);
    assert!(r.qpage().preview.is_none(), "Show read a version while a restore was asked");
    // Cancel: the question is gone, and Show works again
    assert!(r.click(1280.0, "Cancel", 0));
    r.click(1280.0, "Show", 0);
    assert!(r.qpage().preview.is_some(), "Show did not come back after Cancel");
}

#[test]
fn d2l_a_compare_never_offers_the_same_side_in_both_pickers() {
    let sides = vec![Side::Version(5), Side::Version(3), Side::YourCopy];
    assert_eq!(picker_options(&sides, Side::Version(3)), vec![Side::Version(5), Side::YourCopy]);
    assert_eq!(picker_options(&sides, Side::YourCopy), vec![Side::Version(5), Side::Version(3)]);
    assert_eq!(picker_options(&[Side::Version(5), Side::YourCopy], Side::Version(5)), vec![Side::YourCopy]);
}

#[test]
fn d2l_open_compare_refuses_the_same_side() {
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(3), served(3, &[qj(QA, A3), qj(QB, B)], false));
    r.open(0);
    r.page(1280.0);
    let cache = &mut r.m.edit.question_cache;
    if let EditView::Question(p) = &mut r.m.edit.view {
        p.open_compare(Side::Version(3), Side::Version(3), cache);
    }
    assert!(r.qpage().compare.is_none(), "a version compared with itself");
    // a real pair still opens
    let cache = &mut r.m.edit.question_cache;
    if let EditView::Question(p) = &mut r.m.edit.view {
        p.open_compare(Side::Version(3), Side::YourCopy, cache);
    }
    assert!(r.qpage().compare.is_some());
}

// ---- round 7 (fifth critic pair): minors -------------------------------------------------------------------------------

#[test]
fn d2m_the_page_says_when_older_versions_are_not_shown() {
    // (round 7, critic M5; round 8, critic P) the pool serves one page of the bank's lines: said as the bank's own fact,
    // never as "this question has more", since a question's history may end well above the page
    let mut r = rig();
    let mut l = lines_json();
    l["next_before"] = json!(3);
    r.w.on("Get", &versions_path(), 200, l, None);
    r.w.on("Get", &changes_path(), 200, changes_json(), Some("\"e1\""));
    r.open(0);
    let t = r.page(1280.0);
    assert!(t.contains("This bank has older versions than this page shows"), "{t}");
    // one page and nothing older: nothing said
    let mut r = rig();
    r.script_load();
    r.open(0);
    let t = r.page(1280.0);
    assert!(!t.contains("older versions than this page shows"), "{t}");
}

#[test]
fn d2m_a_disabled_show_says_wait_for_the_restore() {
    // (round 7, critic M6; round 8, critic P) Show waits while a restore is busy, and the disabled button's words are the
    // state's own: Asking still has Yes and Cancel, Reading has neither a Cancel nor a working Escape
    let mut r = rig();
    r.script_load();
    r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
    r.open(0);
    r.page(1280.0);
    assert!(r.click(1280.0, "Restore...", 0));
    let t = hover_disabled_show(&mut r);
    assert!(t.contains("A restore is waiting for your Yes") && t.contains("Cancel"), "{t}");
    assert!(!t.contains("reading its version"), "nothing is being read while the question is asked: {t}");
    // the version read is under way: the words say to wait, and never offer a Cancel there is not. (Its body is dropped so
    // the restore stays Reading: with the answer already here the page would finish it in this frame.)
    let (cid, bid) = match &r.m.edit.view {
        EditView::Question(p) => (p.cid.clone(), p.bid.clone()),
        _ => panic!("the page"),
    };
    r.m.edit.question_cache.drop_body(&cid, &bid, 2);
    if let EditView::Question(p) = &mut r.m.edit.view {
        p.restore = Some(super::question_history_ui::RestoreView { version: 2, state: super::question_history_ui::RestoreState::Reading });
    }
    let t = hover_disabled_show(&mut r);
    assert!(t.contains("A restore is reading its version"), "{t}");
    assert!(!t.contains("Cancel") && !t.contains("Escape"), "Reading has no Cancel to offer: {t}");
}

/// The text drawn after half a second of a still pointer over the (disabled) Show button.
fn hover_disabled_show(r: &mut Rig) -> String {
    let show = r.frame_at(1280.0, Vec::new()).into_iter().find(|(t, _)| t == "Show").map(|(_, rect)| rect).expect("a Show button");
    let p = show.center();
    r.frame_at(1280.0, vec![egui::Event::PointerMoved(p)]);
    let mut t = String::new();
    for _ in 0..16 {
        t = r.frame_at(1280.0, Vec::new()).into_iter().map(|(t, _)| t).collect::<Vec<_>>().join("\n");
    }
    t
}

