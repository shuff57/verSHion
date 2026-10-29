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
    fn send<'a>(&'a self, build: &'a dyn Fn() -> Request) -> LocalFuture<'a, Result<Response, ApiError>> {
        Box::pin(async move {
            let r = build();
            let path = r.url.trim_start_matches("https://pool.test").split('?').next().unwrap_or_default().to_string();
            let key = format!("{:?} {}", r.method, path);
            self.seen.lock().unwrap().push((key.clone(), r.headers.iter().map(|(k, v)| (k.clone(), v.expose().to_string())).collect()));
            if let Some(q) = self.routes.lock().unwrap().get_mut(&key).and_then(VecDeque::pop_front) {
                return Ok(q);
            }
            match self.sticky.lock().unwrap().get(&key) {
                Some(b) => Ok(reply(200, b, None)),
                None => panic!("no wired reply for {key}"),
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
    json!({"id": id, "stem": stem, "kind": {"Mc": {"answers": [{"text": "x = 2", "correct": true}, {"text": "x = 5", "correct": false}]}}, "points": 1.0, "difficulty": null, "created": 5, "modified": 6})
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
struct CountRestore(Mutex<usize>);

impl RestoreSink for CountRestore {
    fn save(&self, _: &[QuestionBank]) -> Result<String, String> {
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
}

fn rig_with(mode: LinkMode, viewers_form: bool) -> Rig {
    let mut m = model();
    let restores = Arc::new(CountRestore::default());
    let media: super::publish::SharedMedia = Arc::new(HashMap::<String, Vec<u8>>::new());
    m.edit = CircleEditModel::new(Runner::Inline, Arc::new(Mem::default()), media, restores.clone(), LinkLog::memory()).with_retry(RetryPolicy::none());
    m.set_clock(NOW);
    let (mut bank, mut l) = linked(&[(QA, A5), (QB, B)]);
    bank.name = "Unit 2 quiz".into();
    l.base_version = 5;
    l.base_version_id = hex(5);
    l.head_seen = 5;
    l.mode = mode;
    l.viewers_form = viewers_form;
    l.mark_bank(&bank);
    m.edit.add_link(l).unwrap();
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    Rig { m, w: Wire::new(), banks: vec![bank], acct: FakeAccount { open: true, ..FakeAccount::default() }, ctx, restores, who: "olga".into(), painter: Painter { textures: HashMap::new() } }
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
        // (the bank's page is drawn first, as in the app: the model has the session then)
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
        super::question_history_ui::question_menu_of(ui, m, bank, question_id);
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
    for w in BOTH {
        let t = r.page(w);
        assert!(t.contains("Try again"), "{t}");
    }
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
    // signed out while open: says so, with Sign in
    let mut r = rig();
    r.open_cold(0);
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
        assert!(t.contains("Choices, answer key, points, difficulty, topics, explanation, picture: the same"), "{t}");
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
    // Version 5's row, then version 2's ("as it was then")
    assert!(r.click(1280.0, "Restore...", 1));
    let t = r.page(1280.0);
    assert!(t.contains("Replace your copy of question 1 with version 2's? Only your copy changes; the circle gets it when you send. A restore point is saved first."), "{t}");
    assert_eq!(serde_json::to_string(&r.banks).unwrap(), before, "nothing changes before the yes");
    assert_eq!(*r.restores.0.lock().unwrap(), 0);
    assert_eq!(r.w.calls("/versions/2"), 0, "nothing read before the yes");
    assert!(r.page(420.0).contains("Replace your copy of question 1 with version 2's?"), "the question at 420 px too");
    // No: nothing
    assert!(r.click(1280.0, "Cancel", 0));
    assert!(r.qpage().restore.is_none());
    assert!(r.click(1280.0, "Restore...", 1));
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
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, h as f32))), time: Some(t0 + 0.05), ..Default::default() };
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
    for w in BOTH {
        // the panel with its rows and gap line
        let mut r = rig();
        r.script_load();
        r.open(0);
        r.page(w);
        r.shot(&dir, "1-panel", w);
        // every version
        r.qpage_mut().show_all = true;
        r.page(w);
        r.shot(&dir, "2-every-version", w);
        r.qpage_mut().show_all = false;
        // the restore question, then done
        r.w.raw(&version_path(2), served(2, &[qj(QA, A3)], false));
        r.qpage_mut().ask_restore(2);
        r.page(w);
        r.shot(&dir, "3-restore-ask", w);
        let cache = &mut r.m.edit.question_cache;
        if let EditView::Question(p) = &mut r.m.edit.view {
            p.confirm_restore(cache);
        }
        r.page(w);
        r.shot(&dir, "4-restore-done", w);
        assert!(r.click(w, "< Back to Unit 2 quiz", 0));
        r.page(w);
        r.shot(&dir, "5-strip-after", w);
        // compare
        let mut r = rig();
        r.script_load();
        r.w.raw(&version_path(3), served(3, &[qj(QA, A3), qj(QB, B)], false));
        r.w.raw(&version_path(5), served(5, &[qj(QA, A5), qj(QB, B)], false));
        r.open(0);
        r.page(w);
        assert!(r.click(w, "Compare", 0));
        r.page(w);
        r.shot(&dir, "6-compare", w);
        // a viewer of a hiding bank
        let mut r = rig_with(LinkMode::Follow, true);
        r.w.on("Get", &versions_path(), 200, lines_json(), None);
        r.w.on("Get", &changes_path(), 200, json!({"bank_id": BID, "head_version": 5, "per_question": false, "reason": "keys_hidden"}), None);
        r.open(0);
        r.page(w);
        r.shot(&dir, "7-hiding-viewer", w);
        // signed out, and a question the circle does not have
        let mut r = rig();
        r.open_cold(0);
        r.page(w);
        r.shot(&dir, "8-signed-out", w);
    }
}
