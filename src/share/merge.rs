//! Three-way merge of a newer pool version into a bank that was imported from it, like version
//! control: BASE is what the last import (or merge) put in, OURS is the bank now, THEIRS is the
//! pool's new version. This file is pure: it builds the review (`build_plan`) and applies the
//! person's decisions to a copy of the bank (`apply_merge`). It reads and writes nothing; the model
//! in `browse.rs` decides when, and keeps BASE in `share-imported.json`.
//!
//! What it promises:
//!
//! - Only questions the record still links (by the local id it stored) are merged. A question the
//!   teacher added, copied in from another bank, or that was deleted, is not part of it.
//! - A key the teacher typed into a question that was imported without one is never replaced by
//!   the "no key" placeholder: when the pool changes only the wording, the wording is taken and the
//!   local key kept; when the pool changes the choices too, taking the pool's version is not
//!   offered (keep both, or keep yours).
//! - A conflict has no default. Nothing is decided for the person except the safe defaults: take a
//!   change only the pool made, keep a change only they made, add what is new, keep what the pool
//!   dropped.
//! - Applying works on a copy of the bank and replaces the bank only when everything succeeded.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::api;
use super::browse_text as text;
use crate::model::{Kind, KEYLESS_SENTINEL, Part, PartKind, Question, QuestionBank, is_keyless, rekeyed};

/// Most questions a record keeps a base for, and the most bytes of base kept for one record.
/// Past either, the record has no base and the person is told a merge is not possible for it.
pub const BASE_MAX_QUESTIONS: usize = 500;
pub const BASE_MAX_BYTES: usize = 1_500_000;

// ---- BASE ------------------------------------------------------------------------------------------------

/// A question's content as it was imported: enough to tell later who changed what.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snap {
    pub stem: String,
    pub kind: Kind,
    #[serde(default)]
    pub points: Option<f64>,
    #[serde(default)]
    pub topics: Vec<String>,
    /// The explanation, `None` for the keyless placeholder.
    #[serde(default)]
    pub explanation: Option<String>,
    /// The picture's name on the pool (a hash), `None` for no picture.
    #[serde(default)]
    pub image: Option<String>,
    /// Imported without its answer key.
    #[serde(default)]
    pub keyless: bool,
}

/// One imported question, linked to the bank by its local id.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BaseQuestion {
    pub local_id: String,
    /// The pool's id for the question, which stays the same across versions.
    #[serde(default)]
    pub qid: String,
    /// Fingerprint of the pool's text, for matching when the pool gave no id.
    #[serde(default)]
    pub fp: String,
    pub snap: Snap,
    /// The picture's file name in this app's store when imported.
    #[serde(default)]
    pub local_image: Option<String>,
}

impl PartialEq for BaseQuestion {
    fn eq(&self, o: &Self) -> bool {
        serde_json::to_value(self).ok() == serde_json::to_value(o).ok()
    }
}

fn explanation_of(q: &Question) -> Option<String> {
    if is_keyless(q) { None } else { q.explanation.clone().filter(|e| !e.trim().is_empty()) }
}

pub fn snap_of(q: &Question, image_sha: Option<String>) -> Snap {
    Snap { stem: q.stem.clone(), kind: q.kind.clone(), points: q.points, topics: q.topics.clone(), explanation: explanation_of(q), image: image_sha, keyless: is_keyless(q) }
}

pub fn base_of(q: &Question, qid: &str, fp: &str, sha: Option<String>) -> BaseQuestion {
    BaseQuestion { local_id: q.id.clone(), qid: qid.to_string(), fp: fp.to_string(), snap: snap_of(q, sha), local_image: q.image.as_ref().map(|i| i.file.clone()) }
}

/// The base to keep, or none when it would be too big.
pub fn bounded(base: Vec<BaseQuestion>) -> Vec<BaseQuestion> {
    if base.len() > BASE_MAX_QUESTIONS || serde_json::to_vec(&base).map_or(true, |b| b.len() > BASE_MAX_BYTES) { Vec::new() } else { base }
}

// ---- the key, apart from the rest -------------------------------------------------------------------------

fn strip_part(p: &Part) -> Part {
    Part {
        stem: p.stem.clone(),
        kind: match &p.kind {
            PartKind::Mc { answers } => PartKind::Mc { answers: answers.iter().map(|a| crate::model::Answer { text: a.text.clone(), correct: false, locked: a.locked }).collect() },
            PartKind::Tf { .. } => PartKind::Tf { correct: false },
            PartKind::Short { .. } => PartKind::Short { sample: String::new() },
        },
    }
}

/// The question without its answer key: what students see.
pub fn strip_key(k: &Kind) -> Kind {
    match k {
        Kind::Mc { answers } => Kind::Mc { answers: answers.iter().map(|a| crate::model::Answer { text: a.text.clone(), correct: false, locked: a.locked }).collect() },
        Kind::Tf { .. } => Kind::Tf { correct: false },
        Kind::Matching { pairs, distractors } => {
            Kind::Matching { pairs: pairs.iter().map(|p| crate::model::Pair { left: p.left.clone(), right: String::new() }).collect(), distractors: distractors.clone() }
        }
        Kind::Blank { accept } => Kind::Blank { accept: accept.iter().map(|_| Vec::new()).collect() },
        Kind::Short { .. } => Kind::Short { sample: String::new() },
        Kind::Multipart { parts } => Kind::Multipart { parts: parts.iter().map(strip_part).collect() },
    }
}

fn shape(k: &Kind) -> String {
    serde_json::to_string(&strip_key(k)).unwrap_or_default()
}

/// Only the key of a question, for comparing.
fn key_value(k: &Kind) -> String {
    let v: Value = match k {
        Kind::Mc { answers } => answers.iter().map(|a| a.correct).collect::<Vec<_>>().into(),
        Kind::Tf { correct } => (*correct).into(),
        Kind::Matching { pairs, .. } => pairs.iter().map(|p| p.right.clone()).collect::<Vec<_>>().into(),
        Kind::Blank { accept } => serde_json::to_value(accept).unwrap_or_default(),
        Kind::Short { sample } => sample.clone().into(),
        Kind::Multipart { parts } => parts
            .iter()
            .map(|p| match &p.kind {
                PartKind::Mc { answers } => answers.iter().map(|a| a.correct).collect::<Vec<_>>().into(),
                PartKind::Tf { correct } => (*correct).into(),
                PartKind::Short { sample } => sample.clone().into(),
            })
            .collect::<Vec<Value>>()
            .into(),
    };
    v.to_string()
}

/// The choices of a question in words (no key).
pub fn kind_text(k: &Kind) -> String {
    let list = |a: &[crate::model::Answer]| a.iter().map(|x| text::shown(&x.text)).collect::<Vec<_>>().join(" | ");
    match k {
        Kind::Mc { answers } => list(answers),
        Kind::Tf { .. } => "True / False".into(),
        Kind::Matching { pairs, distractors } => {
            let mut s = pairs.iter().map(|p| text::shown(&p.left)).collect::<Vec<_>>().join(" | ");
            if !distractors.is_empty() {
                s.push_str(&format!(" (extra: {})", distractors.iter().map(|d| text::shown(d)).collect::<Vec<_>>().join(", ")));
            }
            s
        }
        Kind::Blank { accept } => format!("{} blank{}", accept.len(), if accept.len() == 1 { "" } else { "s" }),
        Kind::Short { .. } => "Written answer".into(),
        Kind::Multipart { parts } => parts.iter().map(|p| text::shown(&p.stem)).collect::<Vec<_>>().join(" / "),
    }
}

fn key_text(k: &Kind, keyless: bool) -> String {
    if keyless && !rekeyed_kind(k) {
        return "no key (the author hid it)".into();
    }
    match k {
        Kind::Mc { answers } => {
            let c: Vec<String> = answers.iter().enumerate().filter(|(_, a)| a.correct).map(|(i, _)| (i + 1).to_string()).collect();
            if c.is_empty() { "none marked".into() } else { format!("choice {}", c.join(", ")) }
        }
        Kind::Tf { correct } => (if *correct { "True" } else { "False" }).into(),
        Kind::Matching { pairs, .. } => pairs.iter().map(|p| text::shown(&p.right)).collect::<Vec<_>>().join(" | "),
        Kind::Blank { accept } => accept.iter().map(|a| a.join(" / ")).collect::<Vec<_>>().join(" | "),
        Kind::Short { sample } => text::shown(sample),
        Kind::Multipart { .. } => "per part".into(),
    }
}

/// A rough "has any key content" used only for wording.
fn rekeyed_kind(k: &Kind) -> bool {
    match k {
        Kind::Mc { answers } => answers.iter().any(|a| a.correct),
        Kind::Tf { .. } => true,
        Kind::Matching { pairs, .. } => pairs.iter().any(|p| !p.right.is_empty()),
        Kind::Blank { accept } => accept.iter().any(|a| !a.is_empty()),
        Kind::Short { sample } => !sample.is_empty(),
        Kind::Multipart { .. } => true,
    }
}

// ---- THEIRS ----------------------------------------------------------------------------------------------

/// One question of the pool's version, in this app's shape.
#[derive(Clone, Debug)]
pub struct Theirs {
    /// Position in the pool's payload.
    pub index: usize,
    pub qid: String,
    pub fp: String,
    pub sha: Option<String>,
    pub q: Question,
}

/// Turns a pool question into a local one: new id, the key placeholder where the key is hidden,
/// the picture left for the caller. Also returns the picture's name on the pool.
pub fn convert_question(mut raw: Value, keyless_form: bool, at_ms: i64) -> Option<(Question, Option<String>)> {
    let file = raw.pointer("/image/file").and_then(Value::as_str).map(str::to_string);
    let keyless = keyless_form || raw.get("keyless").and_then(Value::as_bool) == Some(true);
    if let Some(o) = raw.as_object_mut() {
        o.remove("image");
    }
    let mut q = serde_json::from_value::<Question>(raw).ok()?;
    q.id = crate::model::new_id();
    q.created = at_ms;
    q.modified = at_ms;
    if keyless {
        q.explanation = Some(KEYLESS_SENTINEL.to_string());
    }
    let sha = file.and_then(|f| f.strip_prefix("media/").and_then(|r| r.split_once('.')).map(|(s, _)| s.to_string()));
    Some((q, sha))
}

// ---- the review ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    /// Neither side changed it (or both made the same change).
    Unchanged,
    /// Only the pool changed it.
    TheirsChanged,
    /// Only you changed it.
    OursChanged,
    /// Both changed it, differently.
    Conflict,
    /// New in the pool's version.
    NewInTheirs,
    /// The pool's version no longer has it.
    RemovedInTheirs,
    /// You deleted it from the bank.
    RemovedLocally,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Undecided,
    /// Take the pool's version (replaces yours; your key is kept when the pool hid its own).
    Take,
    /// Keep yours.
    Keep,
    /// Conflict: keep yours.
    Ours,
    /// Conflict: use the pool's.
    Theirs,
    /// Conflict: keep yours and add the pool's as a second question.
    Both,
    Add,
    Skip,
    /// Delete it from the bank.
    Remove,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldDiff {
    pub field: &'static str,
    pub base: String,
    pub ours: String,
    pub theirs: String,
}

#[derive(Clone, Debug)]
pub struct Row {
    pub kind: RowKind,
    pub title: String,
    pub local_id: Option<String>,
    /// The local id of the base entry this row came from (also for a question you deleted).
    pub base_id: String,
    pub theirs_index: Option<usize>,
    pub theirs_qid: String,
    pub theirs_fp: String,
    pub diffs: Vec<FieldDiff>,
    pub choice: Choice,
    pub options: Vec<Choice>,
    pub note: Option<String>,
    /// Taking the pool's version keeps your local answer key.
    pub carry_key: bool,
    /// You entered a key into a question that came without one.
    pub ours_rekeyed: bool,
}

impl Row {
    pub fn needs_decision(&self) -> bool {
        self.kind == RowKind::Conflict && self.choice == Choice::Undecided
    }
}

#[derive(Clone, Debug)]
pub struct MergePlan {
    pub item_id: String,
    pub bank_id: String,
    pub bank_name: String,
    pub old_version: u64,
    pub new_version: u64,
    pub rows: Vec<Row>,
    /// Questions in the bank that are not linked to the pool (yours, or copied in): left alone.
    pub unlinked: usize,
    pub license: Option<(String, String)>,
    pub new_credit: String,
}

impl MergePlan {
    pub fn undecided(&self) -> usize {
        self.rows.iter().filter(|r| r.needs_decision()).count()
    }
    pub fn count(&self, k: RowKind) -> usize {
        self.rows.iter().filter(|r| r.kind == k).count()
    }
    /// True when pressing "Apply merge" is allowed.
    pub fn ready(&self) -> bool {
        self.undecided() == 0
    }
    /// Whether anything would change in the bank.
    pub fn changes_anything(&self) -> bool {
        self.rows.iter().any(|r| matches!(r.choice, Choice::Take | Choice::Theirs | Choice::Both | Choice::Add | Choice::Remove))
    }
}

#[derive(Default, Clone, Copy)]
struct Delta {
    stem: bool,
    shape: bool,
    key: bool,
    expl: bool,
    points: bool,
    topics: bool,
    image: bool,
}

impl Delta {
    fn any(&self) -> bool {
        self.stem || self.shape || self.key || self.expl || self.points || self.topics || self.image
    }
    fn only_key(&self) -> bool {
        self.key && !(self.stem || self.shape || self.expl || self.points || self.topics || self.image)
    }
}

fn title_of(stem: &str) -> String {
    let p = text::shown(stem);
    if p.chars().count() > 90 { format!("{}...", p.chars().take(90).collect::<String>()) } else { p }
}

fn pic(s: &Option<String>) -> String {
    if s.is_some() { "a picture".into() } else { "no picture".into() }
}

fn pts(p: Option<f64>) -> String {
    p.map_or("none".into(), |p| p.to_string())
}

/// What the person is shown for each changed field.
fn diffs(base: &BaseQuestion, ours: &Question, theirs: &Question, theirs_sha: &Option<String>, od: Delta, td: Delta) -> Vec<FieldDiff> {
    let b = &base.snap;
    let on = |a: bool, c: bool| a || c;
    let mut v = Vec::new();
    let oexp = explanation_of(ours);
    let texp = explanation_of(theirs);
    if on(od.stem, td.stem) {
        v.push(FieldDiff { field: "Question text", base: text::shown(&b.stem), ours: text::shown(&ours.stem), theirs: text::shown(&theirs.stem) });
    }
    if on(od.shape, td.shape) {
        v.push(FieldDiff { field: "Choices", base: kind_text(&b.kind), ours: kind_text(&ours.kind), theirs: kind_text(&theirs.kind) });
    }
    if on(od.key, td.key) {
        v.push(FieldDiff { field: "Answer key", base: key_text(&b.kind, b.keyless), ours: key_text(&ours.kind, b.keyless), theirs: key_text(&theirs.kind, is_keyless(theirs)) });
    }
    if on(od.expl, td.expl) {
        let s = |e: &Option<String>| e.as_deref().map_or("none".to_string(), text::shown);
        v.push(FieldDiff { field: "Explanation", base: s(&b.explanation), ours: s(&oexp), theirs: s(&texp) });
    }
    if on(od.points, td.points) {
        v.push(FieldDiff { field: "Points", base: pts(b.points), ours: pts(ours.points), theirs: pts(theirs.points) });
    }
    if on(od.topics, td.topics) {
        v.push(FieldDiff { field: "Topics", base: b.topics.join(", "), ours: ours.topics.join(", "), theirs: theirs.topics.join(", ") });
    }
    if on(od.image, td.image) {
        v.push(FieldDiff {
            field: "Picture",
            base: pic(&b.image),
            ours: pic(&ours.image.as_ref().map(|i| i.file.clone())),
            theirs: pic(theirs_sha),
        });
    }
    v
}

fn ours_delta(base: &BaseQuestion, o: &Question) -> Delta {
    let b = &base.snap;
    let key = if b.keyless {
        let mut before = o.clone();
        before.kind = b.kind.clone();
        rekeyed(&before, &o.kind)
    } else {
        key_value(&o.kind) != key_value(&b.kind)
    };
    Delta {
        stem: o.stem != b.stem,
        shape: shape(&o.kind) != shape(&b.kind),
        key,
        expl: explanation_of(o) != b.explanation,
        points: o.points != b.points,
        topics: o.topics != b.topics,
        image: o.image.as_ref().map(|i| &i.file) != base.local_image.as_ref(),
    }
}

/// Whether the bank's question differs from what it was when last in step with the circle.
pub fn ours_changed(base: &BaseQuestion, q: &Question) -> bool {
    ours_delta(base, q).any()
}

/// The text a two-way review shows where the earlier version is not known.
pub const NOT_KEPT: &str = "(the earlier version is not kept)";

/// Two-way fallback for a circle bank whose base version is gone: every linked question that both
/// sides have is treated as changed on both sides, so a difference is a conflict the person
/// decides, never an overwrite. A question the circle no longer has is offered for removal.
pub fn unknown_base(bank: &QuestionBank, links: &[(String, String)], theirs: &[Theirs]) -> Vec<BaseQuestion> {
    let mut out = Vec::new();
    for (local, qid) in links {
        let Some(q) = bank.questions.iter().find(|q| q.id == *local) else {
            // Deleted here: the circle's question counts as unchanged since, so it stays deleted
            // (the review lists it as "deleted by you") instead of coming back as "new".
            if let Some(t) = theirs.iter().find(|t| t.qid == *qid) {
                let mut b = base_of(&t.q, qid, &t.fp, t.sha.clone());
                b.local_id = local.clone();
                out.push(b);
            }
            continue;
        };
        let mut b = base_of(q, qid, "", None);
        if theirs.iter().any(|t| t.qid == *qid) {
            b.snap.stem = NOT_KEPT.to_string();
            b.snap.points = Some(-1.0);
            b.snap.topics = vec![NOT_KEPT.to_string()];
            b.snap.explanation = Some(NOT_KEPT.to_string());
            b.snap.image = theirs.iter().find(|t| t.qid == *qid).and_then(|t| t.sha.clone()).filter(|_| q.image.is_some());
            b.local_image = q.image.as_ref().map(|i| i.file.clone());
        }
        out.push(b);
    }
    out
}

fn theirs_delta(base: &BaseQuestion, t: &Theirs) -> Delta {
    let b = &base.snap;
    let tk = is_keyless(&t.q);
    let key = if b.keyless && tk { false } else { b.keyless != tk || key_value(&t.q.kind) != key_value(&b.kind) };
    Delta {
        stem: t.q.stem != b.stem,
        shape: shape(&t.q.kind) != shape(&b.kind),
        key,
        expl: explanation_of(&t.q) != b.explanation,
        points: t.q.points != b.points,
        topics: t.q.topics != b.topics,
        image: t.sha != b.image,
    }
}

/// Whether yours and the pool's end up the same on everything but pictures.
fn same_result(o: &Question, t: &Question) -> bool {
    o.stem == t.stem
        && shape(&o.kind) == shape(&t.kind)
        && key_value(&o.kind) == key_value(&t.kind)
        && explanation_of(o) == explanation_of(t)
        && o.points == t.points
        && o.topics == t.topics
}

/// Builds the review. `theirs` is the pool's version (pictures not needed); `bank` is the target.
#[allow(clippy::too_many_arguments)]
pub fn build_plan(
    item_id: &str,
    base: &[BaseQuestion],
    bank: &QuestionBank,
    theirs: &[Theirs],
    old_version: u64,
    new_version: u64,
    license: Option<(String, String)>,
    new_credit: String,
) -> MergePlan {
    let mut used = vec![false; theirs.len()];
    let mut rows = Vec::new();
    let mut linked = 0usize;
    for b in base {
        // The pool's id first; the text only when the pool gave no id.
        let ti = (!b.qid.is_empty())
            .then(|| theirs.iter().enumerate().position(|(i, t)| !used[i] && t.qid == b.qid))
            .flatten()
            .or_else(|| theirs.iter().enumerate().position(|(i, t)| !used[i] && !b.fp.is_empty() && t.fp == b.fp));
        if let Some(i) = ti {
            used[i] = true;
        }
        let ours = bank.questions.iter().find(|q| q.id == b.local_id);
        let mut row = Row {
            kind: RowKind::Unchanged,
            title: String::new(),
            local_id: ours.map(|q| q.id.clone()),
            base_id: b.local_id.clone(),
            theirs_index: ti.map(|i| theirs[i].index),
            theirs_qid: ti.map(|i| theirs[i].qid.clone()).unwrap_or_default(),
            theirs_fp: ti.map(|i| theirs[i].fp.clone()).unwrap_or_default(),
            diffs: Vec::new(),
            choice: Choice::Undecided,
            options: Vec::new(),
            note: None,
            carry_key: false,
            ours_rekeyed: false,
        };
        let Some(o) = ours else {
            row.kind = RowKind::RemovedLocally;
            row.title = title_of(&b.snap.stem);
            row.note = Some("You deleted this from the bank. It stays deleted.".into());
            rows.push(row);
            continue;
        };
        linked += 1;
        let od = ours_delta(b, o);
        row.ours_rekeyed = b.snap.keyless && od.key;
        let Some(i) = ti else {
            row.kind = RowKind::RemovedInTheirs;
            row.title = title_of(&o.stem);
            row.options = vec![Choice::Keep, Choice::Remove];
            row.choice = Choice::Keep;
            row.note = Some(if od.any() { "The pool dropped this question. You changed it, so it is kept unless you remove it.".into() } else { "The pool dropped this question. It is kept unless you remove it. Kept questions are no longer linked to the pool.".into() });
            rows.push(row);
            continue;
        };
        let t = &theirs[i];
        let td = theirs_delta(b, t);
        row.title = title_of(&o.stem);
        row.diffs = diffs(b, o, &t.q, &t.sha, od, td);
        match (od.any(), td.any()) {
            (false, false) => row.kind = RowKind::Unchanged,
            (false, true) => {
                row.kind = RowKind::TheirsChanged;
                row.options = vec![Choice::Take, Choice::Keep];
                row.choice = Choice::Take;
            }
            (true, false) => {
                row.kind = RowKind::OursChanged;
                row.choice = Choice::Keep;
                row.note = Some("Only you changed this. It stays as it is.".into());
            }
            (true, true) => {
                let identical = !od.image && !td.image && same_result(o, &t.q);
                // Your key typed into a question that came without one, and the pool changed only
                // other things: take the pool's wording and keep the key.
                let key_only = row.ours_rekeyed && od.only_key() && is_keyless(&t.q) && shape(&o.kind) == shape(&t.q.kind);
                if identical {
                    row.kind = RowKind::Unchanged;
                    row.diffs.clear();
                    row.note = Some("You and the pool made the same change.".into());
                } else if key_only {
                    row.kind = RowKind::TheirsChanged;
                    row.options = vec![Choice::Take, Choice::Keep];
                    row.choice = Choice::Take;
                    row.carry_key = true;
                    row.note = Some("The pool changed this question and you entered its answer key. The pool's wording is taken and your key is kept.".into());
                } else {
                    row.kind = RowKind::Conflict;
                    row.options = vec![Choice::Ours, Choice::Theirs, Choice::Both];
                    if row.ours_rekeyed && is_keyless(&t.q) {
                        if shape(&o.kind) == shape(&t.q.kind) {
                            row.carry_key = true;
                            row.note = Some("Both changed this. If you use the pool's, your answer key is kept.".into());
                        } else {
                            row.options = vec![Choice::Ours, Choice::Both];
                            row.note = Some("You entered an answer key and the pool changed the choices, so the pool's version can not take your key. Keep yours, or keep both.".into());
                        }
                    }
                }
            }
        }
        rows.push(row);
    }
    for (i, t) in theirs.iter().enumerate().filter(|(i, _)| !used[*i]) {
        let _ = i;
        rows.push(Row {
            kind: RowKind::NewInTheirs,
            title: title_of(&t.q.stem),
            local_id: None,
            base_id: String::new(),
            theirs_index: Some(t.index),
            theirs_qid: t.qid.clone(),
            theirs_fp: t.fp.clone(),
            diffs: Vec::new(),
            choice: Choice::Add,
            options: vec![Choice::Add, Choice::Skip],
            note: None,
            carry_key: false,
            ours_rekeyed: false,
        });
    }
    MergePlan {
        item_id: item_id.to_string(),
        bank_id: bank.id.clone(),
        bank_name: bank.name.clone(),
        old_version,
        new_version,
        rows,
        unlinked: bank.questions.len().saturating_sub(linked),
        license,
        new_credit,
    }
}

// ---- applying ---------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MergeOutcome {
    pub taken: usize,
    pub kept: usize,
    pub added: usize,
    pub removed: usize,
    pub both: usize,
}

fn replaced(ours: &Question, theirs: &Question, carry: bool, now: i64) -> Question {
    let mut r = theirs.clone();
    r.id = ours.id.clone();
    r.created = ours.created;
    r.modified = now;
    if carry {
        r.kind = ours.kind.clone();
        r.explanation = ours.explanation.clone();
    }
    r
}

/// Applies the decisions to a copy of `bank` and returns the new base; `bank` itself is replaced
/// only when everything worked. `fetched` is the pool's version with its pictures stored.
pub fn apply_merge(bank: &mut QuestionBank, plan: &MergePlan, fetched: &[Theirs], base: &[BaseQuestion], author: &str, now: i64) -> Result<(MergeOutcome, Vec<BaseQuestion>), String> {
    if !plan.ready() {
        return Err("Some questions still need a decision.".into());
    }
    let find = |row: &Row| -> Result<&Theirs, String> {
        let t = fetched.iter().find(|t| Some(t.index) == row.theirs_index).ok_or_else(|| "The pool's version is missing a question that was reviewed.".to_string())?;
        if t.qid != row.theirs_qid || t.fp != row.theirs_fp {
            return Err("The pool's version changed while you were reviewing. Nothing was applied; start the merge again.".into());
        }
        Ok(t)
    };
    let mut b = bank.clone();
    let mut out = MergeOutcome::default();
    let mut new_base: Vec<BaseQuestion> = Vec::new();
    for row in &plan.rows {
        let old_base = row.local_id.as_ref().and_then(|id| base.iter().find(|x| x.local_id == *id));
        match row.kind {
            RowKind::Unchanged | RowKind::OursChanged => {
                if let Some(ob) = old_base {
                    new_base.push(ob.clone());
                }
            }
            RowKind::TheirsChanged | RowKind::Conflict => {
                let id = row.local_id.clone().unwrap_or_default();
                let pos = b.questions.iter().position(|q| q.id == id).ok_or("A question of the bank went missing.")?;
                let t = find(row)?;
                let (take, both) = match (row.kind, row.choice) {
                    (RowKind::TheirsChanged, Choice::Take) | (RowKind::Conflict, Choice::Theirs) => (true, false),
                    (RowKind::TheirsChanged, Choice::Keep) | (RowKind::Conflict, Choice::Ours) => (false, false),
                    (RowKind::Conflict, Choice::Both) => (false, true),
                    _ => return Err("A question has no valid decision.".into()),
                };
                if take {
                    let ours = &b.questions[pos];
                    let r = replaced(ours, &t.q, row.carry_key, now);
                    // The one thing a merge must never do: swap a key you typed for the placeholder.
                    if row.ours_rekeyed && is_keyless(&t.q) && key_value(&r.kind) != key_value(&ours.kind) {
                        return Err("This merge would have replaced an answer key you entered. Nothing was applied.".into());
                    }
                    b.questions[pos] = r;
                    out.taken += 1;
                } else {
                    out.kept += 1;
                }
                if both {
                    let mut copy = t.q.clone();
                    copy.id = crate::model::new_id();
                    copy.topics.push(format!("From the pool: {}, version {}", api::plain(author), plan.new_version));
                    b.questions.insert(pos + 1, copy);
                    out.both += 1;
                }
                let mut nb = base_of(&b.questions[pos], &t.qid, &t.fp, t.sha.clone());
                // The base is what the pool's version is, not what is now in the bank.
                nb.snap = snap_of(&t.q, t.sha.clone());
                nb.local_image = t.q.image.as_ref().map(|i| i.file.clone());
                new_base.push(nb);
            }
            RowKind::NewInTheirs => {
                if row.choice == Choice::Add {
                    let t = find(row)?;
                    let q = t.q.clone();
                    new_base.push(base_of(&q, &t.qid, &t.fp, t.sha.clone()));
                    b.questions.push(q);
                    out.added += 1;
                }
            }
            RowKind::RemovedInTheirs => {
                if row.choice == Choice::Remove {
                    let id = row.local_id.clone().unwrap_or_default();
                    b.questions.retain(|q| q.id != id);
                    out.removed += 1;
                } else {
                    out.kept += 1;
                }
            }
            // Still remembered as deleted, so a later merge does not offer it as new.
            RowKind::RemovedLocally => {
                if let Some(t) = row.theirs_index.and_then(|_| find(row).ok()) {
                    let mut nb = base_of(&t.q, &t.qid, &t.fp, t.sha.clone());
                    nb.local_id = row.base_id.clone();
                    new_base.push(nb);
                }
            }
        }
    }
    // `now` is milliseconds; a bank's time is in seconds, like everywhere else in the app.
    b.modified = (now / 1000).max(b.modified);
    *bank = b;
    Ok((out, new_base))
}

/// Replaces the bank's credit line for this item (the line that names its id), or adds it. Other
/// lines the teacher wrote are left as they are.
pub fn update_credit(bank: &mut QuestionBank, item_id: &str, line: &str) {
    let needle = format!("item {item_id} version");
    let mut lines: Vec<String> = bank.details.description.lines().map(str::to_string).collect();
    match lines.iter().position(|l| l.starts_with("From the shared pool:") && l.contains(&needle)) {
        Some(i) => lines[i] = line.to_string(),
        None => lines.push(line.to_string()),
    }
    bank.details.description = lines.into_iter().filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n");
}
