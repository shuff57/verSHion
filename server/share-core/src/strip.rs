//! `strip`: the stripped (keyless) form of docs/shared-pool.md ("The stripped (keyless) form").
//!
//! Applied by the server only, once per `hide_key` publish. **The shuffle seed is drawn inside
//! this crate** ([`strip`]: `getrandom`, which is `crypto.getRandomValues` in the Worker's wasm and
//! the OS generator natively); the caller cannot choose it, so a caller cannot replay a chosen
//! order. One ChaCha20 stream per payload: the questions of a bank are shuffled one after another
//! from it, so each gets an independent permutation. [`strip_with_seed`] takes an explicit seed
//! for tests and fixtures only; the wasm build exports it only with the `test-seed` feature.
//!
//! | Kind | Stripped form |
//! |---|---|
//! | `Mc` (single or multiple answer) | every `correct` set to `false`; labels removed **only** where the author chose `Strip` (`labels`); the unlocked answers shuffled once (uniform permutation; `locked` answers keep their slot) **unless** the question keeps order-revealing labels (`Keep`), names sibling choices ("Both A and B", an unlocked "All of the above"), or has fewer than two unlocked answers: then **nothing in it moves** and its position is in `needs_order_ack` with the reason in `order_reasons` |
//! | `Tf` | `correct = false` |
//! | `Matching` | `pairs[].left` kept in order; `pairs[].right = ""`; `distractors` = the **word bank**: every non-empty right item and distractor, de-duplicated by exact text (as Moodle's match question does), in one random order. See `import` for how a client turns this into its own model. |
//! | `Blank` | every `accept` list emptied; the number of blanks is kept |
//! | `Short` | `sample = ""` |
//! | `Multipart` | the rules above for each part (`Mc`, `Tf`, `Short`) |
//! | every question | `explanation` removed; `figure` **always** removed; `image` removed unless its position is in `image_ok`; `published` removed (author-local state); `origin.served_sha256` and `origin.match_hash` removed; `keyless = true` |
//! | the bank | `published` removed; `origin.served_sha256` and `origin.match_hash` removed |
//!
//! Kept: `id`, `stem`, part stems, answer and left-item texts, `locked`, `topics`, `points`,
//! `difficulty`, `created`, `modified`, the rest of `origin`, and the bank's name and details.
//! The payload is written as schema v2 (`SCHEMA_STRIPPED`), the first version with `keyless`.
#[allow(unused_imports)]
use crate::format::FastClass;
use crate::format;
use crate::labels::{self, Analysis, LabelChoice, Preview, SiblingRef};
use crate::import;
use crate::text;
use crate::validate::{MAX_ERRORS, ValidationError};
use crate::wire::*;
use rand_chacha::ChaCha20Rng;
use rand_core::{RngCore, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The shuffle source: ChaCha20 keyed by a 32-byte seed.
pub struct ShuffleRng(ChaCha20Rng);

impl ShuffleRng {
    pub fn from_seed(seed: [u8; 32]) -> Self {
        ShuffleRng(ChaCha20Rng::from_seed(seed))
    }
    /// Seeded from the platform's cryptographic generator.
    pub fn from_entropy() -> Result<Self, getrandom::Error> {
        let mut seed = [0u8; 32];
        getrandom::getrandom(&mut seed)?;
        Ok(Self::from_seed(seed))
    }
    /// The next raw 64-bit output (tests compare `below` against a reference built on it).
    pub fn next_u64(&mut self) -> u64 {
        self.0.next_u64()
    }
    /// Uniform in `0..n` (`n >= 1`), without modulo bias.
    pub fn below(&mut self, n: usize) -> usize {
        let n = n.max(1) as u64;
        // 2^64 mod n: values below it would make the low residues more likely.
        let threshold = n.wrapping_neg() % n;
        loop {
            let x = self.0.next_u64();
            if x >= threshold {
                return (x % n) as usize;
            }
        }
    }
    /// Fisher-Yates.
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i + 1);
            v.swap(i, j);
        }
    }
}

/// A question (and optionally a part of a multipart question) by position in the payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Pos {
    pub question: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<usize>,
}

/// The author's label decision for one question or part.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabelDecision {
    pub question: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<usize>,
    pub choice: LabelChoice,
}

/// The decisions of one question, by part (`None` = the question's own choices).
pub type QuestionLabels = BTreeMap<Option<usize>, LabelChoice>;

/// A label finding for the author's preview (see `labels`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LabelFinding {
    pub question: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part: Option<usize>,
    /// A Matching right column (the word bank), not Mc choices. Keep is refused when the labels
    /// are order-revealing.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub matching: bool,
    pub order_revealing: bool,
    pub strip_allowed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<LabelChoice>,
    /// The before/after preview (empty in strip results; capped in `label_findings`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<Preview>,
    /// The preview was left out to stay within the preview budget.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
}

/// Why a question's order is preserved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderReason {
    /// Fewer than two unlocked answers.
    FewUnlocked,
    /// Order-revealing labels the author chose to keep.
    LabelsKept,
    /// A choice names its siblings.
    SiblingRefs,
    /// The author set `keep_order` on the question.
    AuthorKeepOrder,
}

/// The served order of one shuffled question, part or word bank.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ServedOrder {
    pub question: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part: Option<usize>,
    /// A Matching word bank (indices into the author's right column), not Mc choices.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub matching: bool,
    pub order: Vec<usize>,
    /// A Matching question: the served order of its left items (indices into the author's
    /// `pairs`). Absent when the author kept the order (`keep_order`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left_order: Option<Vec<usize>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct OrderNote {
    pub question: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part: Option<usize>,
    pub reason: OrderReason,
}

#[derive(Clone, Debug, Default)]
pub struct StripOptions {
    /// Question positions whose `image` the author ticked "does not reveal the answer".
    /// A position outside the payload is an error.
    pub image_ok: Vec<usize>,
    /// The author's label decisions (required for every finding).
    pub labels: Vec<LabelDecision>,
    /// Questions whose low-risk formatting findings the author acknowledged as content
    /// (`ack_formatting`; see `format`). High-risk markers are refused whatever this says.
    pub ack_formatting: Vec<usize>,
    /// Remove `origin` from every question and the bank: the server writes its own provenance
    /// (a client's claimed `origin` is not trusted).
    pub drop_client_provenance: bool,
    /// The caller has already run `validate_for_publish` with `hide_key` and these decisions (the
    /// wasm `strip` does), so the formatting checks are not repeated. Leave `false` otherwise.
    #[doc(hidden)]
    pub formatting_checked: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stripped {
    pub payload: Payload,
    /// Mc questions or parts whose order is preserved: the publish must carry the author's
    /// acknowledgement for each, or be refused. The reasons are in `order_reasons`.
    pub needs_order_ack: Vec<Pos>,
    pub order_reasons: Vec<OrderNote>,
    /// Positions where labels were removed (decision `Strip`).
    pub labels_removed: Vec<Pos>,
    /// Every label finding with its decision: store these with the publish (`acks_json`), they
    /// are what `match_hash_with` needs to hash the author's full body consistently.
    pub label_findings: Vec<LabelFinding>,
    pub images_kept: Vec<usize>,
    pub images_removed: Vec<usize>,
    pub figures_removed: Vec<usize>,
    /// What only the author may see (see [`AuthorOnly`]).
    pub author_only: AuthorOnly,
}

/// The part of a strip report that, next to the served form, gives the key away: it must go to
/// the publishing author (their publish response) and nowhere else: not to a listing, a
/// non-author's response, a log, an audit record or a report. It is neither `Serialize` nor
/// printable with `{:?}` (both redact): the only way out is [`AuthorOnly::author_json`], and in
/// wasm the separate `strip_author_data` export.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct AuthorOnly {
    /// Every Mc question or part, and every Matching word bank, that **was shuffled**, with its
    /// served order (`order[k]` = the author's index now at position `k`; for a word bank, an
    /// index into the author's right column, so `order[k] < pairs` **is** the pairing).
    pub served_orders: Vec<ServedOrder>,
}

impl std::fmt::Debug for AuthorOnly {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AuthorOnly(<{} served orders, redacted>)", self.served_orders.len())
    }
}

impl AuthorOnly {
    /// `{"served_orders": [...]}`: for the publishing author only.
    pub fn author_json(&self) -> String {
        serde_json::json!({ "served_orders": self.served_orders }).to_string()
    }
}

enum Order<'a> {
    Shuffle(&'a mut ShuffleRng),
    Sort,
}

struct Ctx<'a, 'r> {
    order: Order<'r>,
    labels: &'a BTreeMap<(usize, Option<usize>), LabelChoice>,
    out: Stripped,
    errors: Vec<ValidationError>,
    /// The current question's `keep_order`.
    keep_order: bool,
    /// `ack_formatting`, and `drop_client_provenance`.
    acks: BTreeSet<usize>,
    drop_provenance: bool,
    formatting_checked: bool,
}

fn label_path(at: Pos) -> String {
    match at.part {
        Some(p) => format!("labels[question={},part={p}]", at.question),
        None => format!("labels[question={}]", at.question),
    }
}

fn texts(answers: &[Answer]) -> Vec<&str> {
    answers.iter().map(|a| a.text.as_str()).collect()
}

/// What may happen to one Mc's choices given its labels, its decision and its sibling refs.
struct McPlan {
    analysis: Option<Analysis>,
    strip: bool,
    keep_order: Option<OrderReason>,
}

fn plan_mc(answers: &[Answer], decision: Option<LabelChoice>, author_keep: bool) -> Result<McPlan, &'static str> {
    let analysis = labels::analyze(&texts(answers));
    let mut strip = false;
    let mut keep_order = None;
    if let Some(a) = &analysis {
        match decision {
            None => return Err("labels_need_decision"),
            Some(LabelChoice::Strip) if !a.strip_allowed => return Err("labels_strip_refused"),
            Some(LabelChoice::Strip) => strip = true,
            Some(LabelChoice::Keep) => {
                if a.order_revealing {
                    keep_order = Some(OrderReason::LabelsKept);
                }
            }
        }
    }
    // (Round 12, critic23a B3: with labels, references are read on the text without them, "I. 2",
    // "II. 3" are the answers 2 and 3; the labels themselves name the choices.)
    let sibling = if analysis.is_some() {
        let bare: Vec<String> = answers.iter().map(|a| match labels::detect(&a.text) {
            Some(h) if h.start == 0 && text::has_visible(&a.text[h.end..]) => a.text[h.end..].trim().to_string(),
            _ => a.text.clone(),
        }).collect();
        let raw_refs = labels::refers_to_siblings(&answers.iter().map(|a| (a.text.as_str(), a.locked)).collect::<Vec<_>>());
        let bare_refs = labels::refers_to_siblings(&bare.iter().zip(answers).map(|(t, a)| (t.as_str(), a.locked)).collect::<Vec<_>>());
        // A letter reference in the content ("Both (a) and (b)") still counts with the labels.
        raw_refs && (bare_refs || answers.iter().any(|a| matches!(labels::sibling_ref(&a.text), Some(SiblingRef::Labels)) && bare.iter().any(|b| labels::sibling_ref(b) == Some(SiblingRef::Labels))))
    } else {
        labels::refers_to_siblings(&answers.iter().map(|a| (a.text.as_str(), a.locked)).collect::<Vec<_>>())
    };
    if keep_order.is_none() && sibling {
        keep_order = Some(OrderReason::SiblingRefs);
    }
    if author_keep {
        keep_order = Some(OrderReason::AuthorKeepOrder);
    }
    Ok(McPlan { analysis, strip, keep_order })
}

fn strip_mc(answers: &mut [Answer], at: Pos, cx: &mut Ctx) {
    let decision = cx.labels.get(&(at.question, at.part)).copied();
    let plan = match plan_mc(answers, decision, cx.keep_order) {
        Ok(p) => p,
        Err(code) => {
            if cx.errors.len() < MAX_ERRORS {
                cx.errors.push(ValidationError::new(code, &label_path(at)));
            }
            return;
        }
    };
    if let Some(a) = &plan.analysis {
        // Decisions only: the previews were the author's, and copying them here is what made a
        // large bank's strip use too much memory.
        cx.out.label_findings.push(LabelFinding {
            question: at.question,
            part: at.part,
            matching: false,
            order_revealing: a.order_revealing,
            strip_allowed: a.strip_allowed,
            decision,
            choices: Vec::new(),
            truncated: false,
        });
        if plan.strip {
            for (ans, &c) in answers.iter_mut().zip(&a.cut) {
                ans.text = labels::apply(&ans.text, c);
            }
            cx.out.labels_removed.push(at);
        }
    }
    for a in answers.iter_mut() {
        a.correct = false;
        // Served as students see it: no invisible marks at the ends or between letters.
        a.text = format::served_text(&a.text);
    }
    let slots: Vec<usize> = answers.iter().enumerate().filter(|(_, a)| !a.locked).map(|(i, _)| i).collect();
    let reason = plan.keep_order.or((slots.len() < 2 && !answers.is_empty()).then_some(OrderReason::FewUnlocked));
    if let Some(r) = reason {
        cx.out.needs_order_ack.push(at);
        cx.out.order_reasons.push(OrderNote { question: at.question, part: at.part, reason: r });
        // The served order is the author's, and must stay so wherever the question goes: every
        // choice is served locked, so an importer's exam builder (which shuffles unlocked choices)
        // keeps it too.
        for a in answers.iter_mut() {
            a.locked = true;
        }
        if matches!(cx.order, Order::Shuffle(_)) {
            return;
        }
    }
    match &mut cx.order {
        Order::Shuffle(rng) => {
            let mut moving: Vec<(usize, Answer)> = slots.iter().map(|&i| (i, answers[i].clone())).collect();
            rng.shuffle(&mut moving);
            let mut order: Vec<usize> = (0..answers.len()).collect();
            for (slot, (from, a)) in slots.into_iter().zip(moving) {
                order[slot] = from;
                answers[slot] = a;
            }
            cx.out.author_only.served_orders.push(ServedOrder { question: at.question, part: at.part, matching: false, order, left_order: None });
        }
        Order::Sort => answers.sort_by_cached_key(|a| text::hash_form(&a.text)),
    }
}

fn word_bank(pairs: &mut [Pair], distractors: &mut Vec<String>, at: usize, cx: &mut Ctx) {
    let pos = Pos { question: at, part: None };
    // Labels on the right items ("a. France" with "1. Paris") travel with the shuffled bank and give
    // the pairing away when they follow the pairs' order: the same finding and decision as Mc,
    // except that Keep of order-revealing labels is refused (keeping the order would not help:
    // the left column is never shuffled).
    let (column, paired) = import::right_column_paired(pairs, distractors);
    let refs: Vec<&str> = column.iter().map(String::as_str).collect();
    let mut cut = None;
    if let Some(a) = labels::analyze_column(&refs, paired) {
        let decision = cx.labels.get(&(at, None)).copied();
        let err = match decision {
            None => Some("labels_need_decision"),
            Some(LabelChoice::Strip) if !a.strip_allowed => Some("labels_strip_refused"),
            Some(LabelChoice::Keep) if a.order_revealing => Some("labels_keep_refused"),
            _ => None,
        };
        if let Some(code) = err
            && cx.errors.len() < MAX_ERRORS
        {
            cx.errors.push(ValidationError::new(code, &label_path(pos)));
        }
        cx.out.label_findings.push(LabelFinding { question: at, part: None, matching: true, order_revealing: a.order_revealing, strip_allowed: a.strip_allowed, decision, choices: Vec::new(), truncated: false });
        if decision == Some(LabelChoice::Strip) && a.strip_allowed {
            cx.out.labels_removed.push(pos);
            cut = Some(a.cut);
        }
    }
    for p in pairs.iter_mut() {
        p.right.clear();
        p.left = format::served_text(&p.left);
    }
    distractors.clear();
    let mut bank: Vec<(usize, String)> = column
        .into_iter()
        .enumerate()
        .map(|(k, t)| match &cut {
            Some(c) => (k, format::served_text(&labels::apply(&t, c[k]))),
            None => (k, format::served_text(&t)),
        })
        .collect();
    match &mut cx.order {
        Order::Shuffle(rng) => {
            rng.shuffle(&mut bank);
            // The left items too (round 8): in the author's order a natural sequence (stages,
            // dates, steps) pairs with an ordered bank ("Step 1", "Step 2"...).
            let left_order = (!cx.keep_order).then(|| {
                let mut idx: Vec<usize> = (0..pairs.len()).collect();
                rng.shuffle(&mut idx);
                let old: Vec<Pair> = pairs.to_vec();
                for (slot, &k) in idx.iter().enumerate() {
                    pairs[slot] = old[k].clone();
                }
                idx
            });
            // `order[k]` indexes the author's right column (`import::right_column` of the full
            // question); `left_order[k]` the author's pairs.
            cx.out.author_only.served_orders.push(ServedOrder { question: at, part: None, matching: true, order: bank.iter().map(|b| b.0).collect(), left_order });
        }
        Order::Sort => bank.sort_by_cached_key(|s| text::hash_form(&s.1)),
    }
    *distractors = bank.into_iter().map(|b| b.1).collect();
}

/// The texts of one Mc as served, for the formatting check: the bodies after the label (Strip:
/// what Strip leaves; Keep: each choice's text after its own label), and, when labels are kept,
/// the whole texts as a second view.
fn served_views(answers: &[Answer], decision: Option<LabelChoice>) -> Vec<Vec<String>> {
    let t = texts(answers);
    match labels::analyze(&t) {
        Some(a) if decision == Some(LabelChoice::Strip) && a.strip_allowed => {
            vec![t.iter().zip(&a.cut).map(|(x, &c)| format::served_text(&labels::apply(x, c))).collect()]
        }
        // Kept labels that count the choices off: judge what follows each label too ("A. *Venus").
        // Labels that are text ("pascal (Pa)", "newton (N)") are served as they are.
        Some(a) if a.order_revealing => {
            let body = |x: &str| match labels::detect(x) {
                Some(h) if h.suffix => x[..h.start].to_string(),
                Some(h) => x[h.end..].to_string(),
                None => x.to_string(),
            };
            vec![t.iter().map(|x| format::served_text(&body(x))).collect(), t.iter().map(|x| format::served_text(x)).collect()]
        }
        _ => vec![t.iter().map(|x| format::served_text(x)).collect()],
    }
}

/// The flags of one Mc as served (every view), sorted by choice.
pub fn mc_format_flags(answers: &[Answer], decision: Option<LabelChoice>) -> Vec<format::Flag> {
    let mut flags: Vec<format::Flag> = Vec::new();
    for view in served_views(answers, decision) {
        let c: Vec<(&str, bool)> = view.iter().zip(answers).map(|(t, a)| (t.as_str(), a.correct)).collect();
        for f in format::check(&c) {
            if !flags.iter().any(|x| x.index == f.index && x.reason == f.reason) {
                flags.push(f);
            }
        }
    }
    flags.sort_by_key(|f| (f.index, f.reason));
    flags
}

/// Choices that are not answers and are usually locked at the end ("I don't know", "Other").
fn non_answer(t: &str) -> bool {
    const WORDS: &[&str] = &[
        "i don't know", "i do not know", "don't know", "dont know", "not sure", "unsure", "no idea", "other", "skip", "prefer not to say",
        "no sé", "no lo sé", "je ne sais pas", "weiß nicht", "ich weiß es nicht", "keine ahnung", "não sei", "non lo so", "не знаю",
        "わからない", "分からない", "不知道", "모르겠다", "모름", "bilmiyorum", "nie wiem", "weet ik niet",
    ];
    let low = t.trim().trim_end_matches(['.', '!', '?']).to_lowercase().replace('\u{2019}', "'");
    WORDS.contains(&low.as_str())
}

/// Whether the served `locked` flags give the key away (the question is shuffled; "all / none of
/// the above" and non-answers like "I don't know", which are locked by nature, aside). With L the
/// locked and M the movable choices among the rest, and C the correct ones: refused when C lies
/// inside L or inside M and that set has at most |C| + 2 members (and is not everything): "Venus"
/// and "Mercury" movable with every wrong choice locked, "Venus" movable with one other, "Venus"
/// locked with up to two others.
fn locked_leak(answers: &[Answer], decision: Option<LabelChoice>, keep_order: bool) -> bool {
    if !answers.iter().any(|a| a.locked) || !answers.iter().any(|a| a.correct) {
        return false;
    }
    // (References, "All of the above", "Both of the above", "A and B", are locked as references,
    // not as answers.)
    // (Read without a label or bullet: "d) All of the above", "• None of these".)
    let bare = |t: &str| -> String {
        let t = t.trim_start().trim_start_matches(['\u{2022}', '\u{25CF}', '\u{25E6}', '-', '*', '\u{2013}']).trim_start();
        match labels::detect(t) {
            Some(h) if h.start == 0 && text::has_visible(&t[h.end..]) => t[h.end..].trim().to_string(),
            _ => t.to_string(),
        }
    };
    // (A choice that names two or more of the others by their text, "Both mitochondria and
    // chloroplasts", refers to them like "Both of the above": locked as a reference, round 12.)
    let folded: Vec<String> = answers.iter().map(|a| format::fold_key(&bare(&a.text))).collect();
    let names_siblings = |i: usize| folded.get(i).is_some_and(|me| folded.iter().enumerate().filter(|(j, t)| *j != i && t.chars().count() >= 2 && format::holds_token(me, t)).take(2).count() >= 2);
    let universe: Vec<usize> = (0..answers.len()).filter(|&i| { let b = bare(&answers[i].text); !matches!(labels::sibling_ref(&b), Some(SiblingRef::Above | SiblingRef::Labels)) && !non_answer(&b) && !names_siblings(i) }).collect();
    let locked: Vec<usize> = universe.iter().copied().filter(|&i| answers[i].locked).collect();
    let movable: Vec<usize> = universe.iter().copied().filter(|&i| !answers[i].locked).collect();
    let correct: Vec<usize> = universe.iter().copied().filter(|&i| answers[i].correct).collect();
    if locked.is_empty() || movable.is_empty() || correct.is_empty() || answers.iter().filter(|a| !a.locked).count() < 2 {
        return false;
    }
    let frozen = match plan_mc(answers, decision, keep_order) {
        Ok(p) => p.keep_order.is_some(),
        Err(_) => false,
    };
    if frozen {
        return false;
    }
    let narrows = |set: &[usize]| correct.iter().all(|c| set.contains(c)) && set.len() <= correct.len() + 2 && set.len() < universe.len();
    narrows(&locked) || narrows(&movable)
}

/// `key_in_stem` (reason `aiken`, `gift`, `cloze`, `answer_line`, `left_names_right`): a served
/// text that gives the key away. Never ackable; author only.
/// Whether students see the choices by position, so a letter names one: the order does not
/// move (keep_order, references, kept labels that count them off), or labels are kept on them.
/// Labels stripped from shuffled choices name nothing (round 13, critic24a: "(a) How long..."
/// over "a. 2.0 s" with Strip was refused).
fn positional(answers: &[Answer], decision: Option<LabelChoice>, keep_order: bool) -> bool {
    let frozen = keep_order || plan_mc(answers, decision, keep_order).map_or(true, |p| p.keep_order.is_some());
    frozen || (labels::analyze(&texts(answers)).is_some() && decision != Some(LabelChoice::Strip))
}

/// A folded text with every pair separator ("-", ">", ":", "=", ",") made one mark (`\u{1}`) and
/// the spaces around it dropped: the form in which a listed left-right pair ("Bauxite -
/// Aluminium", "=Bauxite -> Aluminium") is found.
fn pair_mark_form(f: &str) -> String {
    let mut g = String::with_capacity(f.len());
    for part in f.split_inclusive(['-', '>', ':', '=', ',']) {
        match part.char_indices().last() {
            Some((i, '-' | '>' | ':' | '=' | ',')) => {
                g.push_str(part[..i].trim_end_matches(' '));
                if !g.ends_with('\u{1}') {
                    g.push('\u{1}');
                }
            }
            _ => g.push_str(part),
        }
    }
    // (A comma too: "Danube,Hungary" lines, round 15.)
    g.replace("\u{1} ", "\u{1}")
}

/// A listed pair in the form `pair_mark_form` gives the text: the folded left and right with
/// every pair mark inside them made a mark too ("Jean-Paul Sartre - Existentialism" and
/// "jean-paul sartre: existentialism" are one entry). Round 20 (critic36a B36a-2, critic36b B2):
/// an item holding "-", ",", ":", "=" or ">" was never found.
fn pair_entry(l: &str, r: &str) -> String {
    pair_mark_form(&format!("{l} - {r}"))
}

fn stem_key_errors(q: &Question, qp: &str, acked: bool, decision: &dyn Fn(Option<usize>) -> Option<LabelChoice>, out: &mut Vec<ValidationError>) {
    let low = |t: &str| format::served_text(t).to_lowercase();
    // (`blank_hint`, the accepted word in brackets after its blank, is low risk: an ESL form
    // hint "I ___ (eat)" as often as a leak "____ (Au)".)
    let err = |path: String, reason: &'static str| {
        let low = format::LOW_STEM_REASONS.contains(&reason);
        let hint = match reason {
            "word_bank_order" => HINT_WORD_BANK,
            "bank_word_bank_order" | "bank_word_bank_three" => HINT_BANK_ORDER,
            "scan_budget" => HINT_BUDGET,
            "word_bank_pair" => HINT_WORD_BANK_PAIR,
            "part_states_key" | "key_heading_row" => HINT_PART_KEY,
            "choices_pasted" => HINT_PASTED,
            _ if low => HINT_LOW,
            _ => HINT_STEM,
        };
        ValidationError { reason: Some(reason), ack_allowed: Some(low), author_only: true, hint: Some(hint), ..ValidationError::new("key_in_stem", &path) }
    };
    let mut info = format::KeyInfo::default();
    if !q.keyless {
        match &q.kind {
            Kind::Mc { answers } => {
                info.choices = true;
                // (Each correct text also without its label: "B. Nitrogen" is named "Nitrogen".)
                let unlabelled = |t: &str| -> Option<String> { labels::detect(t).filter(|h| h.start == 0 && text::has_visible(&t[h.end..])).map(|h| low(&t[h.end..])) };
                info.texts = answers.iter().filter(|a| a.correct).flat_map(|a| std::iter::once(low(&a.text)).chain(unlabelled(&a.text))).collect();
                // (Read only by the rule on a stem's own lines: a one-line stem needs none.)
                if q.stem.contains(['\n', '\r', '\u{2028}', '\u{2029}', '\u{B}']) {
                    info.wrong = answers.iter().filter(|a| !a.correct).flat_map(|a| std::iter::once(low(&a.text)).chain(unlabelled(&a.text))).collect();
                }
                info.letters = answers.iter().enumerate().filter(|(_, a)| a.correct).map(|(i, _)| (b'a' + (i % 26) as u8) as char).collect();
                // (A letter names a position students can see: an order that does not move, or
                // labels the choices carry.)
                info.positional = positional(answers, decision(None), q.keep_order);
            }
            Kind::Tf { correct } => {
                info.tf = true;
                info.tf_value = Some(*correct);
            }
            Kind::Short { sample } => info.texts = vec![low(sample)],
            Kind::Blank { accept } => {
                info.texts = accept.iter().flatten().map(|a| low(a)).collect();
                info.blank = true;
                info.order_groups = accept.iter().map(|g| g.iter().map(|a| low(a)).collect()).collect();
            }
            Kind::Matching { pairs, .. } => {
                info.texts = pairs.iter().map(|p| low(&p.right)).collect();
                // (With a fixed order, a stem list of the rights in pair order is the key.)
                if q.keep_order {
                    info.order_groups = pairs.iter().map(|p| vec![low(&p.right)]).collect();
                }
            }
            Kind::Multipart { .. } => info.choices = true,
        }
    } else {
        info.choices = matches!(q.kind, Kind::Mc { .. } | Kind::Multipart { .. });
        info.tf = matches!(q.kind, Kind::Tf { .. });
    }
    // (Folded once for the stem and every topic: round 10, critic21a's 31 s.)
    let info = info.folded();
    if let Some(r) = format::stem_key(&q.stem, &info)
        && !(acked && format::LOW_STEM_REASONS.contains(&r))
    {
        out.push(err(format!("{qp}.stem"), r));
    }
    // Topics and the id are served too (round 9, critic20a: high risk): the same scan, and a
    // topic that is a correct choice's text.
    // (Round 14, critic26a: every Multipart part's key too, its True/False value, and a key of
    // four characters or more standing in the topic as a token, "Unit 4: Photosynthesis".)
    let mut topic_keys: Vec<String> = info.texts.clone();
    let mut topic_tf: Vec<bool> = info.tf_value.into_iter().collect();
    if let (Kind::Multipart { parts }, false) = (&q.kind, q.keyless) {
        for part in parts {
            match &part.kind {
                PartKind::Mc { answers } => topic_keys.extend(answers.iter().filter(|a| a.correct).flat_map(|a| std::iter::once(format::fold_key(&a.text)).chain(labels::detect(&a.text).filter(|h| h.start == 0 && text::has_visible(&a.text[h.end..])).map(|h| format::fold_key(&a.text[h.end..]))))),
                PartKind::Short { sample } => topic_keys.push(format::fold_key(sample)),
                PartKind::Tf { correct } => topic_tf.push(*correct),
            }
        }
    }
    topic_keys.retain(|k| !k.is_empty());
    let tf_named = |t: &str| topic_tf.iter().any(|v| format::tf_value_word(t, &format::KeyInfo { tf: true, tf_value: Some(*v), ..Default::default() }));
    for (ti, t) in q.topics.iter().enumerate() {
        let ft = format::fold_key(t);
        let names = topic_keys.iter().any(|k| *k == ft || (k.chars().count() >= 4 && format::holds_token(&ft, k)));
        let r = format::stem_key(t, &info).or_else(|| (!t.trim().is_empty() && (names || tf_named(t))).then_some("topic_is_key"));
        if let Some(r) = r {
            out.push(err(format!("{qp}.topics[{ti}]"), r));
        }
    }
    if let Some(r) = format::stem_key(&q.id, &format::KeyInfo::default()) {
        out.push(err(format!("{qp}.id"), r));
    }
    match &q.kind {
        Kind::Multipart { parts } => {
            for (pi, part) in parts.iter().enumerate() {
                let mut pinfo = format::KeyInfo::default();
                if !q.keyless {
                    match &part.kind {
                        PartKind::Mc { answers } => {
                            pinfo.choices = true;
                            pinfo.texts = answers.iter().filter(|a| a.correct).flat_map(|a| std::iter::once(low(&a.text)).chain(labels::detect(&a.text).filter(|h| h.start == 0 && text::has_visible(&a.text[h.end..])).map(|h| low(&a.text[h.end..])))).collect();
                            pinfo.positional = positional(answers, decision(Some(pi)), q.keep_order);
                            pinfo.letters = answers.iter().enumerate().filter(|(_, a)| a.correct).map(|(i, _)| (b'a' + (i % 26) as u8) as char).collect();
                        }
                        PartKind::Tf { correct } => {
                            pinfo.tf = true;
                            pinfo.tf_value = Some(*correct);
                        }
                        PartKind::Short { sample } => pinfo.texts = vec![low(sample)],
                    }
                }
                if let Some(r) = format::stem_key(&part.stem, &pinfo.folded()) {
                    out.push(err(format!("{qp}.kind.Multipart.parts[{pi}].stem"), r));
                }
            }
            // Round 12 (critic23a B1): a part stem (or the main stem) that states another part's
            // key: an answer line naming it ("Answer to (a): 4.0 m/s²", "ANSWER: False") or a
            // reference with its value ("(a) is False") is high risk; the key text alone ("use
            // 4.0 m/s²") is low risk, given data as often as a leak, unless the main stem gives
            // it too.
            if !q.keyless {
                // Each part's key: its correct texts (without labels) and its True/False value.
                // (Every part, every key text: round 14 critic26b, parts 16 to 49 were not read.
                // At most 50 parts x 26 choices x 2 forms + samples: 2,650 texts, all kept.)
                let keys: Vec<(Vec<String>, Option<bool>)> = parts.iter().map(|part| match &part.kind {
                    PartKind::Mc { answers } => (answers.iter().filter(|a| a.correct).flat_map(|a| std::iter::once(low(&a.text)).chain(labels::detect(&a.text).filter(|h| h.start == 0 && text::has_visible(&a.text[h.end..])).map(|h| low(&a.text[h.end..])))).collect(), None),
                    PartKind::Tf { correct } => (Vec::new(), Some(*correct)),
                    PartKind::Short { sample } => (vec![low(sample)], None),
                }).collect();
                let main = format::fold_key(&q.stem);
                // Every part's key texts folded once, without those the main stem gives (data) or
                // that are True/False words; at most 256 for the question.
                let main_index = format::TokenIndex::new(&main);
                let tf_key = format::KeyInfo { tf: true, ..Default::default() };
                let mut cands: Vec<(usize, String, bool)> = Vec::new();
                for (j, k) in keys.iter().enumerate() {
                    for t in &k.0 {
                        let t = format::fold_key(t);
                        // (No cap: a key at the 4,097th place was skipped silently. The enforced
                        // limits bound this at 50 parts x 52 forms + 50 samples = 2,650; round 17
                        // critic31b B2, round17::caps_cannot_hide_a_key.)
                        if !t.is_empty() && !format::tf_value_word(&t, &tf_key) && main_index.find(&t).is_empty() && !cands.iter().any(|c| c.0 == j && c.1 == t) {
                            let numeric = t.chars().any(|c| c.is_ascii_digit());
                            cands.push((j, t, numeric));
                        }
                    }
                }
                // One reading for the question: every part's keys (a stem naming its own part's key
                // was refused by its own scan already), any True/False value when the parts hold
                // both, looked up once (no cap of 64 texts).
                let all = {
                    let values: Vec<bool> = keys.iter().filter_map(|k| k.1).collect();
                    let one_value = values.first().copied().filter(|v| values.iter().all(|w| w == v));
                    let mut texts: Vec<String> = keys.iter().flat_map(|k| k.0.iter().map(|t| format::fold_key(t))).filter(|t| !t.is_empty()).collect();
                    texts.sort();
                    texts.dedup();
                    format::KeyInfo { texts, tf: !values.is_empty(), tf_value: one_value, lines_only: true, prefolded: true, ..Default::default() }.with_maps()
                };
                // The main stem (and each part stem) listing the parts' keys in part order
                // ("Results: 3 m/s², True, Newton's second law"): round 14, critic26a; part stems
                // round 17, critic31a A6's "Results so far: ..." in the last part.
                let mut groups_of: std::collections::BTreeMap<String, Vec<usize>> = std::collections::BTreeMap::new();
                for (j, k) in keys.iter().enumerate() {
                    let words: Vec<String> = k.0.iter().map(|t| format::fold_key(t)).chain(k.1.map(|v| if v { "true".to_string() } else { "false".to_string() })).collect();
                    for w in words.into_iter().filter(|w| !w.is_empty()) {
                        let v = groups_of.entry(w).or_default();
                        if v.last() != Some(&j) {
                            v.push(j);
                        }
                    }
                }
                let list_of = |t: &str| groups_of.get(t).cloned().unwrap_or_default();
                if !out.iter().any(|e| e.path == format!("{qp}.stem"))
                    && let Some(r) = format::bank_list_order(&q.stem, &list_of)
                    && !(acked && format::LOW_STEM_REASONS.contains(&r))
                {
                    out.push(err(format!("{qp}.stem"), r));
                }
                if !out.iter().any(|e| e.path == format!("{qp}.stem"))
                    && let Some(r) = format::stem_key(&q.stem, &all)
                {
                    out.push(err(format!("{qp}.stem"), r));
                }
                for (pi, part) in parts.iter().enumerate() {
                    let path = format!("{qp}.kind.Multipart.parts[{pi}].stem");
                    if out.iter().any(|e| e.path == path) {
                        continue;
                    }
                    let folded = format::fold_key(&part.stem);
                    let high = format::stem_key(&part.stem, &all).or_else(|| {
                        format::part_tf_refs(&folded).into_iter().any(|(j, v)| j != pi && keys.get(j).is_some_and(|k| k.1 == Some(v))).then_some("answer_line")
                    });
                    if let Some(r) = high {
                        out.push(err(path, r));
                        continue;
                    }
                    // (This part stem listing the parts' keys in part order.)
                    if let Some(r) = format::bank_list_order(&part.stem, &list_of)
                        && !(acked && format::LOW_STEM_REASONS.contains(&r))
                    {
                        out.push(err(path, r));
                        continue;
                    }
                    if acked {
                        continue;
                    }
                    // (Own choices are not another part's key: both parts may offer "2 Ω".)
                    let own: std::collections::BTreeSet<String> = match &part.kind {
                        PartKind::Mc { answers } => answers.iter().map(|a| format::fold_key(&a.text)).collect(),
                        _ => Default::default(),
                    };
                    let f = folded;
                    // (Every place of each word: a key after 64 decoys is still found, round 14.)
                    let index = format::TokenIndex::with_cap(&f, usize::MAX);
                    // Sentence ends, and the references to other parts with their positions, once.
                    let ends: Vec<usize> = f.match_indices(['\n', '.', '?']).map(|(i, _)| i).collect();
                    let sentence = |p: usize| {
                        let k = ends.partition_point(|&e| e < p);
                        let a = if k == 0 { 0 } else { ends[k - 1] + 1 };
                        let z = ends.get(k).copied().unwrap_or(f.len());
                        (a, z)
                    };
                    // (A part is named by its letter, parts 1-26, or its number, 1-50.)
                    // (Each part's references found once per stem.)
                    let cache: std::cell::RefCell<std::collections::HashMap<usize, Vec<usize>>> = Default::default();
                    let refs = |j: usize| -> Vec<usize> {
                        cache.borrow_mut().entry(j).or_insert_with(|| {
                            let mut names = vec![format!("({})", j + 1), format!("part {}", j + 1)];
                            if j < 26 {
                                let letter = (b'a' + j as u8) as char;
                                names.push(format!("({letter})"));
                                names.push(format!("part {letter}"));
                            }
                            names.iter().flat_map(|n| f.match_indices(n.as_str()).map(|(i, _)| i).collect::<Vec<_>>()).collect()
                        }).clone()
                    };
                    // (A number carried forward, "use 4.0 m/s²"; a word only after a reference to
                    // its part, "from (a) (Mg)": "Its acceleration at the top is zero" after "(a)
                    // ... zero" is a statement, round 12.)
                    let states = cands.iter().filter(|c| c.0 != pi && !own.contains(&c.1)).any(|(j, t, numeric)| {
                        let at = index.find(t);
                        if at.is_empty() {
                            return false;
                        }
                        if *numeric {
                            return true;
                        }
                        let r = refs(*j);
                        at.iter().any(|&p| {
                            let (a, z) = sentence(p);
                            r.iter().any(|&x| x >= a && x < z)
                        })
                    });
                    if states {
                        out.push(err(path, "part_states_key"));
                    }
                }
            }
        }
        Kind::Matching { pairs, .. } => {
            // The stem listing the pairing ("Key: France–Paris, Japan–Tokyo"): two pairs or more
            // written left, separator, right.
            if !q.keyless {
                // (Separators with their spaces made one mark once, so each pair is one search
                // of the stem, not eight.)
                let g = pair_mark_form(&format::fold_key(&q.stem));
                let listed = pairs.iter().filter(|p| {
                    let (l, r) = (format::fold_key(&p.left), format::fold_key(&p.right));
                    !l.is_empty() && !r.is_empty() && g.contains(&pair_entry(&l, &r))
                }).take(2).count();
                if listed >= 2 {
                    out.push(err(format!("{qp}.stem"), "pairs_listed"));
                }
            }
            for (i, p) in pairs.iter().enumerate() {
                let pinfo = format::KeyInfo { texts: if q.keyless { Vec::new() } else { vec![p.right.clone()] }, ..Default::default() }.folded();
                // (The whole left item: round 14, the first 512 bytes only was fail-open.)
                if let Some(r) = format::stem_key(&p.left, &pinfo) {
                    out.push(err(format!("{qp}.kind.Matching.pairs[{i}].left"), r));
                }
            }
            // Round 13 (critic24a B2): a left item that points to its right item by the label of a
            // list in the stem ("Bauxite ___e___" with "e. Aluminium" listed), or of the rights.
            if !q.keyless {
                let list = format::stem_list(&q.stem);
                for (i, p) in pairs.iter().enumerate() {
                    if format::left_blank_holds(&p.left, &p.right) {
                        out.push(err(format!("{qp}.kind.Matching.pairs[{i}].left"), "left_points_to_right"));
                        continue;
                    }
                    let Some(v) = format::left_points_to(&p.left) else { continue };
                    let right = format::fold_key(&p.right);
                    let by_stem = list.iter().any(|(k, item)| *k == v && (*item == right || format::holds_token(item, &right)));
                    let by_right = labels::detect(&p.right).is_some_and(|h| h.start == 0 && h.values.iter().any(|x| x.1 == v));
                    if by_stem || by_right {
                        out.push(err(format!("{qp}.kind.Matching.pairs[{i}].left"), "left_points_to_right"));
                    }
                }
            }
            // A left item that gives its own right item as an annotation ("France (Paris)",
            // "France: Paris", "France - Paris"); not a definition that uses the word ("a choice
            // of multiple responses" / "Choice").
            if !q.keyless {
                for (i, p) in pairs.iter().enumerate() {
                    // (Or after a tab, a spreadsheet's two columns: "Danube\tHungary", round 15.)
                    let tabbed = p.left.rsplit_once('\t').is_some_and(|(a, b)| a.chars().any(crate::format::is_alnum) && format::fold_key(b) == format::fold_key(&p.right));
                    if tabbed || format::left_names_right(&low(&p.left), &low(&p.right)) {
                        out.push(err(format!("{qp}.kind.Matching.pairs[{i}].left"), "left_names_right"));
                    }
                }
            }
        }
        _ => {}
    }
}

/// Round 12 (critic23a B1): a question's stem with an answer line naming another question's key
/// ("ANSWER: Pinot Noir" opening question 2, an Aiken block split at the wrong line). High risk.
/// `skip(i)`: question `i` already has a stem refusal. Every key of the bank is read once (at
/// most 4096), and a stem without an answer lead costs one scan.
pub fn bank_key_errors(qs: &[Question], base: &str, skip: &dyn Fn(usize) -> bool, skip_path: &dyn Fn(&str) -> bool) -> Vec<ValidationError> {
    let mut out = Vec::new();
    if qs.len() < 2 {
        return out;
    }
    let (texts, truncated) = bank_keys(qs);
    if texts.is_empty() {
        return out;
    }
    // A topic that is another question's key (round 14, critic26a: "Q2's topic is Q1's key"):
    // every key read against the set of topics, so no cap on the keys applies (round 16,
    // critic28b: past 65,536 keys topics were not read). A key of four characters or more
    // standing in the topic as a token ("Unit 4: Quartz") counts too (round 17, critic31a A6).
    {
        let mut topics: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
        let mut topic_texts: Vec<(String, String)> = Vec::new();
        for (i, q) in qs.iter().enumerate() {
            for (ti, t) in q.topics.iter().enumerate() {
                let path = format!("{base}[{i}].topics[{ti}]");
                let f = format::fold_key(t);
                if !skip_path(&path) && !f.is_empty() {
                    topics.entry(f.clone()).or_default().push(path.clone());
                    topic_texts.push((path, f));
                }
            }
        }
        let mut hit: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        if !topics.is_empty() {
            each_key(qs, &mut |k| {
                if let Some(paths) = topics.get(&format::fold_key(k)) {
                    hit.extend(paths.iter().cloned());
                }
            });
            // (The same keys as tokens: one sorted arena, one pass per topic.)
            let lex = format::Lex::build(texts.iter().filter(|k| k.chars().count() >= 4).map(|k| (k.clone(), 0u32)).collect());
            for (path, f) in &topic_texts {
                if hit.contains(path) || lex.is_empty() {
                    continue;
                }
                let fb = f.as_bytes();
                let mut idx = 0usize;
                while idx < fb.len() {
                    let at_word = idx == 0 || !f[..idx].chars().next_back().is_some_and(crate::format::is_alnum);
                    if at_word {
                        let mut found = false;
                        let out_of_work = lex.walk(fb, idx, fb.len() - idx, &mut |e, _| {
                            if !f[e..].chars().next().is_some_and(crate::format::is_alnum) {
                                found = true;
                                return true;
                            }
                            false
                        });
                        if out_of_work {
                            out.push(ValidationError { reason: Some("scan_budget"), ack_allowed: Some(false), author_only: true, hint: Some(HINT_BUDGET), ..ValidationError::new("key_in_stem", path) });
                            hit.insert(path.clone());
                            break;
                        }
                        if found {
                            hit.insert(path.clone());
                            break;
                        }
                    }
                    idx += f[idx..].chars().next().map_or(1, char::len_utf8);
                }
            }
        }
        for path in hit {
            out.push(ValidationError { reason: Some("topic_is_key"), ack_allowed: Some(false), author_only: true, hint: Some(HINT_STEM), ..ValidationError::new("key_in_stem", &path) });
        }
    }
    let info = format::KeyInfo { texts, lines_only: true, prefolded: true, ..Default::default() }.with_maps();
    // (More keys than are looked up: a stem with any answer line fails closed, round 14.)
    let any = format::KeyInfo { any_key: true, lines_only: true, prefolded: true, ..Default::default() };
    // (Every served text of every question: part stems, items, topics, choices too. Round 17,
    // critic31a B3.)
    for (i, q) in qs.iter().enumerate() {
        if skip(i) {
            continue;
        }
        for (path, t) in question_texts(q, &format!("{base}[{i}]")) {
            if skip_path(&path) {
                continue;
            }
            // (Every served text is read: a choice holding another question's key after a lead
            // without a separator ("Answer Mars") was skipped by a separator test, round 17
            // critic31a A2. A cheap necessary condition keeps a bank's 26,000 plain choices from
            // the full scan; a text that fails it has no lead at all.)
            if !format::may_have_answer_line(t) {
                continue;
            }
            let r = format::stem_key(t, &info).or_else(|| (truncated && format::stem_key(t, &any).is_some()).then_some("scan_budget"));
            if let Some(r) = r {
                out.push(ValidationError { reason: Some(r), ack_allowed: Some(false), author_only: true, hint: Some(if r == "scan_budget" { HINT_BUDGET } else { HINT_STEM }), ..ValidationError::new("key_in_stem", &path) });
            }
        }
    }
    out
}

/// Round 14 (critic26a B1): the bank's name, description and every stem against the questions'
/// keys in question order (`format::bank_list_order`). `skip(path)`: a refusal there already.
pub fn bank_order_errors(b: &QuestionBank, base: &str, acked: &dyn Fn(usize) -> bool, skip: &dyn Fn(&str) -> bool) -> Vec<ValidationError> {
    let mut out = Vec::new();
    // Each question's key texts in question order (a Blank's per blank), folded, as (text, group)
    // in one sorted arena (round 17, critic31a B6: a map of strings per value took memory).
    let mut items: Vec<(String, u32)> = Vec::new();
    let mut groups = 0u32;
    for q in b.questions.iter().filter(|q| !q.keyless) {
        let f = |t: &str| format::fold_key(t);
        match &q.kind {
            Kind::Mc { answers } => {
                for a in answers.iter().filter(|a| a.correct) {
                    items.push((f(&a.text), groups));
                    if let Some(h) = labels::detect(&a.text).filter(|h| h.start == 0 && text::has_visible(&a.text[h.end..])) {
                        items.push((f(&a.text[h.end..]), groups));
                    }
                }
                groups += 1;
            }
            Kind::Blank { accept } => {
                for g in accept {
                    items.extend(g.iter().map(|a| (f(a), groups)));
                    groups += 1;
                }
            }
            Kind::Short { sample } => {
                items.push((f(sample.as_str()), groups));
                groups += 1;
            }
            // Round 18 (critic34a B1/B2): every part's key is a slot in part order, and a
            // Matching question with a fixed left column has one slot per pair, in pair order.
            Kind::Multipart { parts } => {
                for part in parts {
                    match &part.kind {
                        PartKind::Mc { answers } => {
                            for a in answers.iter().filter(|a| a.correct) {
                                items.push((f(&a.text), groups));
                                if let Some(h) = labels::detect(&a.text).filter(|h| h.start == 0 && text::has_visible(&a.text[h.end..])) {
                                    items.push((f(&a.text[h.end..]), groups));
                                }
                            }
                        }
                        PartKind::Short { sample } => items.push((f(sample), groups)),
                        // (A True/False word is no shared item: it never says which part it
                        // answers.)
                        PartKind::Tf { .. } => {}
                    }
                    groups += 1;
                }
            }
            Kind::Matching { pairs, .. } => {
                if q.keep_order {
                    for p in pairs {
                        items.push((f(&p.right), groups));
                        groups += 1;
                    }
                }
            }
            Kind::Tf { .. } => {}
        }
    }
    if groups < 3 {
        return out;
    }
    items.retain(|x| !x.0.is_empty());
    let lex = format::Lex::build(items);
    // (A value of more than four groups, a word accepted everywhere, says nothing of the order.)
    let groups_of = |t: &str| -> Vec<usize> {
        let e = lex.exact(t);
        if e.len() > 4 { Vec::new() } else { e.iter().map(|x| format::Lex::tag(x) as usize).collect() }
    };
    let mk = |reason: &'static str, path: String| {
        let low = format::LOW_STEM_REASONS.contains(&reason);
        ValidationError { reason: Some(reason), ack_allowed: Some(low), author_only: true, hint: Some(HINT_BANK_ORDER), ..ValidationError::new("key_in_stem", &path) }
    };
    let root = base.trim_end_matches(".questions");
    let texts: Vec<(String, &str, Option<usize>)> = [(format!("{root}.name"), b.name.as_str(), None), (format!("{root}.details.description"), b.details.description.as_str(), None)]
        .into_iter()
        .chain(b.questions.iter().enumerate().map(|(i, q)| (format!("{base}[{i}].stem"), q.stem.as_str(), Some(i))))
        .collect();
    for (path, text, q) in texts {
        if skip(&path) {
            continue;
        }
        // (Three in order is low risk, acknowledged on its question; on the bank's name or
        // description, which no question index acknowledges, only four or more count.)
        if let Some(r) = format::bank_list_order(text, &groups_of)
            && !(format::LOW_STEM_REASONS.contains(&r) && q.is_none_or(acked))
        {
            out.push(mk(r, path));
        }
    }
    out
}

/// Every served text of a bank, with its path: the name, the description, the author and the
/// licence name, and for each question its stem, topics, choices, left and right items,
/// distractors, part stems and part choices (round 17, critic31a B3: only question stems were
/// read against other questions' keys).
pub fn bank_texts<'a>(b: &'a QuestionBank, base: &str) -> Vec<(String, &'a str)> {
    let root = base.trim_end_matches(".questions");
    let mut out: Vec<(String, &'a str)> = vec![
        (format!("{root}.name"), b.name.as_str()),
        (format!("{root}.details.description"), b.details.description.as_str()),
        (format!("{root}.details.author"), b.details.author.as_str()),
        (format!("{root}.details.license_name"), b.details.license_name.as_str()),
    ];
    for (i, q) in b.questions.iter().enumerate() {
        out.extend(question_texts(q, &format!("{base}[{i}]")));
    }
    out.retain(|x| !x.1.trim().is_empty());
    out
}

/// The served texts of one question, with their paths.
pub fn question_texts<'a>(q: &'a Question, qp: &str) -> Vec<(String, &'a str)> {
    let mut out: Vec<(String, &'a str)> = Vec::new();
    {
        out.push((format!("{qp}.stem"), q.stem.as_str()));
        out.extend(q.topics.iter().enumerate().map(|(t, x)| (format!("{qp}.topics[{t}]"), x.as_str())));
        match &q.kind {
            Kind::Mc { answers } => out.extend(answers.iter().enumerate().map(|(k, a)| (format!("{qp}.kind.Mc.answers[{k}]"), a.text.as_str()))),
            Kind::Matching { pairs, distractors } => {
                for (k, p) in pairs.iter().enumerate() {
                    out.push((format!("{qp}.kind.Matching.pairs[{k}].left"), p.left.as_str()));
                    out.push((format!("{qp}.kind.Matching.pairs[{k}].right"), p.right.as_str()));
                }
                out.extend(distractors.iter().enumerate().map(|(k, d)| (format!("{qp}.kind.Matching.distractors[{k}]"), d.as_str())));
            }
            Kind::Multipart { parts } => {
                for (pi, part) in parts.iter().enumerate() {
                    out.push((format!("{qp}.kind.Multipart.parts[{pi}].stem"), part.stem.as_str()));
                    if let PartKind::Mc { answers } = &part.kind {
                        out.extend(answers.iter().enumerate().map(|(k, a)| (format!("{qp}.kind.Multipart.parts[{pi}].kind.Mc.answers[{k}]"), a.text.as_str())));
                    }
                }
            }
            _ => {}
        }
    }
    out.retain(|x| !x.1.trim().is_empty());
    out
}

/// Round 15-17 (critic28a, critic31a): an answer key of any layout in any served text of a bank:
/// a run of three or more values, read in order, that are the keys of consecutive questions (key
/// texts, True/False values, and the letters of questions whose order students see), with or
/// without question numbers between them ("Mitochondrion, True, cell", "1 C 2 B 3 D", "T F T",
/// "1T 2F 3T", a two-row grid, a pipe table); or two values each right after its own question's
/// number ("135 T, 136 F", "Q1 (X) Q2 (Y)"). High risk. One pass per text, no cap: a repeated
/// token is one comparison per question list it names, counted against the work budget.
pub fn bank_key_list_errors(b: &QuestionBank, base: &str, decision: &dyn Fn(usize, Option<usize>) -> Option<LabelChoice>, skip: &dyn Fn(&str) -> bool) -> Vec<ValidationError> {
    let mut out = Vec::new();
    let n = b.questions.len();
    // (A single question has no list of questions' keys to read, but its Matching pairs and
    // rights listed in a bank text are still a leak: only the key-list automaton needs two.)
    let automaton = n >= 2;
    if n == 0 {
        return out;
    }
    // Per question: its key texts (in a sorted arena, tag = question), its True/False value, the
    // letters of its correct choices when students see the order. Round 18 (critic34a B1/B2):
    // every kind contributes: a Multipart part's key text and value, and a Matching pair's right.
    let mut items: Vec<(String, u32)> = Vec::new();
    let mut tf_of: Vec<[bool; 2]> = vec![[false; 2]; n];
    let mut letters: std::collections::HashMap<char, Vec<u32>> = std::collections::HashMap::new();
    for (qi, q) in b.questions.iter().enumerate() {
        if q.keyless {
            continue;
        }
        let mut add = |t: &str| {
            let f = format::fold_key(t);
            // (From its first letter or digit on: a value is found at a word start.)
            if let Some(o) = f.char_indices().find(|c| c.1.fast_alnum()).map(|x| x.0) {
                let v = f[o..].to_string();
                if v.chars().count() >= 2 || v.chars().all(|c| c.is_ascii_digit()) {
                    items.push((v, qi as u32));
                }
            }
        };
        // (A question's correct positions as letters and numbers, when students see them. The
        // part's own label decision is used: `part` is None for a top-level question.)
        let mut add_letters = |answers: &[Answer], part: Option<usize>| {
            if positional(answers, decision(qi, part), q.keep_order) {
                for (k, _) in answers.iter().enumerate().filter(|(_, a)| a.correct) {
                    let c = (b'a' + (k % 26) as u8) as char;
                    letters.entry(c).or_default().push(qi as u32);
                    let d = char::from_digit((k + 1) as u32, 10).filter(|_| k < 9);
                    if let Some(d) = d {
                        letters.entry(d).or_default().push(qi as u32);
                    }
                }
            }
        };
        match &q.kind {
            Kind::Mc { answers } => {
                answers.iter().filter(|a| a.correct).for_each(|a| {
                    add(&a.text);
                    // (Without its label too: a list "1 Nitrogen 2 Oxygen" for choices "B. Nitrogen".)
                    if let Some(h) = labels::detect(&a.text).filter(|h| h.start == 0 && text::has_visible(&a.text[h.end..])) {
                        add(&a.text[h.end..]);
                    }
                });
                add_letters(answers, None);
            }
            Kind::Blank { accept } => accept.iter().flatten().for_each(|a| add(a)),
            Kind::Short { sample } => add(sample),
            Kind::Tf { correct } => tf_of[qi][usize::from(*correct)] = true,
            Kind::Matching { pairs, .. } => pairs.iter().for_each(|p| add(&p.right)),
            Kind::Multipart { parts } => {
                for (pi, part) in parts.iter().enumerate() {
                    match &part.kind {
                        PartKind::Mc { answers } => {
                            answers.iter().filter(|a| a.correct).for_each(|a| {
                                add(&a.text);
                                if let Some(h) = labels::detect(&a.text).filter(|h| h.start == 0 && text::has_visible(&a.text[h.end..])) {
                                    add(&a.text[h.end..]);
                                }
                            });
                            add_letters(answers, Some(pi));
                        }
                        PartKind::Short { sample } => add(sample),
                        PartKind::Tf { correct } => tf_of[qi][usize::from(*correct)] = true,
                    }
                }
            }
        }
    }
    let lex = format::Lex::build(items);
    let mut tf_lists: [Vec<u32>; 2] = [Vec::new(), Vec::new()];
    for (qi, v) in tf_of.iter().enumerate() {
        for (k, on) in v.iter().enumerate() {
            if *on {
                tf_lists[k].push(qi as u32);
            }
        }
    }
    let tf_token = |w: &str| -> Option<bool> {
        match w {
            "true" | "t" | "vrai" | "v" | "wahr" | "w" | "verdadero" | "verdadera" | "richtig" | "\u{25CB}" | "\u{2713}" | "\u{2714}" => Some(true),
            "false" | "f" | "faux" | "falsch" | "falso" | "falsa" | "\u{D7}" | "\u{2717}" | "\u{2718}" => Some(false),
            _ => None,
        }
    };
    // Round 18 (critic34a B2): a Matching question's pairs listed in a bank text ("Bauxite -
    // Aluminium, Chalk - Soft, Slate - Roof", the GIFT "=Bauxite -> Aluminium" block), and its
    // rights in pair order when the left column is fixed (`keep_order`). The pair strings
    // (`left\u{1}right`) are found by walking each text; the order check reads the word banks.
    let mut pair_items: Vec<(String, u32)> = Vec::new();
    // (Round 19, critic35a B1: the rights column of each fixed-order Matching question, once per
    // distinct column: 480 questions that share "Yes / No / Maybe" are one group. An index from
    // each right to the (group, position) pairs holding it, dense per-group state, every step
    // charged to the work budget.)
    let mut group_of: std::collections::BTreeMap<Vec<String>, u32> = std::collections::BTreeMap::new();
    let mut order_of: std::collections::BTreeMap<String, Vec<(u32, u32)>> = std::collections::BTreeMap::new();
    for (qi, q) in b.questions.iter().enumerate() {
        if q.keyless {
            continue;
        }
        if let Kind::Matching { pairs, .. } = &q.kind {
            for p in pairs {
                let (l, r) = (format::fold_key(&p.left), format::fold_key(&p.right));
                if !l.is_empty() && !r.is_empty() {
                    pair_items.push((pair_entry(&l, &r), qi as u32));
                }
            }
            // (A word bank in the order of the pairs leaks only when students see the lefts in
            // pair order: keep_order. Three pairs are a word bank; fewer are no list.)
            if q.keep_order && pairs.len() >= 3 {
                let rights: Vec<String> = pairs.iter().map(|p| format::fold_key(&p.right)).filter(|g| !g.is_empty()).collect();
                let next = group_of.len() as u32;
                if let std::collections::btree_map::Entry::Vacant(e) = group_of.entry(rights) {
                    e.insert(next);
                }
            }
        }
    }
    let n_groups = group_of.len();
    // (Round 20, critic36a B36a-2: a right that cannot be one list item, "Paris, France", is
    // swapped for a plain name in the texts and here.)
    let neutral = format::Neutral::new(group_of.keys().flatten().map(|r| r.as_str()));
    for (rights, gid) in &group_of {
        for (pos, r) in rights.iter().enumerate() {
            let key = neutral.name(r).unwrap_or(r.as_str()).to_string();
            order_of.entry(key).or_default().push((*gid, pos as u32));
        }
    }
    drop(group_of);
    let pair_lex = format::Lex::build(pair_items);
    // Dense scratch, reset through the touched lists: no search over the questions seen so far.
    let mut pair_counts: Vec<u32> = vec![0; n];
    let mut order_state: Vec<(u32, Vec<u32>)> = vec![(0, Vec::new()); n_groups];
    let budget_err = |path: &str| ValidationError { reason: Some("scan_budget"), ack_allowed: Some(false), author_only: true, hint: Some(HINT_BUDGET), ..ValidationError::new("key_in_stem", path) };
    for (path, text) in bank_texts(b, base) {
        if skip(&path) {
            continue;
        }
        // (Key texts in a row count in prose: the bank's own texts, stems and part stems. A
        // choice, an item or a topic naming three keys, "Mercury, Venus and Earth", is content;
        // there only numbered values, letters and True/False values count.)
        let prose = path.ends_with(".stem") || !path.contains(".questions[") && !path.starts_with("question");
        let f = format::fold_key(text);
        // Round 18: two pairs of one Matching question in one text are `pairs_listed`; a fixed
        // Matching question's rights listed in pair order are `word_bank_order`.
        // A text's own step budget (past it, `scan_budget`: refused, never read in part).
        let local = std::cell::Cell::new(0u64);
        let charge = |k: u64| -> bool {
            local.set(local.get() + k);
            format::spend_work(k) || local.get() > TEXT_STEPS
        };
        // (An entry is `left` + a mark + `right`: a text with no mark completes none, and a start
        // after the last mark cannot reach one. The walks past the last mark are skipped, not
        // charged: a left item repeated to the field limit read from every word was quadratic.)
        let g = if pair_lex.is_empty() { String::new() } else { pair_mark_form(&f) };
        let last_mark = g.rfind('\u{1}');
        if !pair_lex.is_empty() && last_mark.is_some() {
            let last_mark = last_mark.unwrap_or(0);
            let gb = g.as_bytes();
            let mut touched: Vec<u32> = Vec::new();
            let over_pairs = std::cell::Cell::new(false);
            // (This text's own walk budget: every start walks the entries it matches, so a text
            // that repeats a left item's prefix reads it from each start; past 100,000 steps the
            // text is refused, never read in part.)
            let walk_steps = std::cell::Cell::new(0u32);
            let pair_hit = std::cell::Cell::new(false);
            for (at, c) in g.char_indices() {
                if at > last_mark {
                    break;
                }
                // (A pair starts at any character that is not a space: a left item may open with a
                // symbol, "\u{221A}9", "\u{B0}C", "$5", "(a)", "#include". Round 20, critic36a B36a-3.)
                if !c.is_whitespace() {
                    let walked = pair_lex.walk_limited(gb, at, gb.len() - at, &walk_steps, PAIR_WALK_STEPS, &mut |_e, ents| {
                        for e in ents {
                            if charge(1) {
                                over_pairs.set(true);
                                return true;
                            }
                            let qi = format::Lex::tag(e) as usize;
                            if pair_counts[qi] == 0 {
                                touched.push(qi as u32);
                            }
                            pair_counts[qi] += 1;
                            if pair_counts[qi] >= 2 {
                                pair_hit.set(true);
                                return true;
                            }
                        }
                        false
                    });
                    if walked {
                        over_pairs.set(true);
                    }
                    if over_pairs.get() || pair_hit.get() {
                        break;
                    }
                }
            }
            for qi in touched {
                pair_counts[qi as usize] = 0;
            }
            if over_pairs.get() {
                out.push(budget_err(&path));
                continue;
            }
            if pair_hit.get() {
                out.push(ValidationError { reason: Some("pairs_listed"), ack_allowed: Some(false), author_only: true, hint: Some(HINT_STEM), ..ValidationError::new("key_in_stem", &path) });
                continue;
            }
        }
        if n_groups > 0 {
            let mut order_hit = false;
            let mut over_order = false;
            let swapped = if neutral.is_empty() { Ok(None) } else { neutral.apply(&f) };
            let Ok(swapped) = swapped else {
                for st in order_state.iter_mut() {
                    st.0 = 0;
                    st.1.clear();
                }
                out.push(budget_err(&path));
                continue;
            };
            let fl: &str = swapped.as_ref().map_or(f.as_str(), |x| x.0.as_str());
            'lists: for items in format::word_bank_lists(fl) {
                // (Per distinct rights column, the pair positions its rights take in this list, in
                // order: the longest strictly increasing run of positions, an out-of-order list
                // or a word bank of distractors naming nothing; a run of three or more, 60% of
                // those found, is the key. The run is kept online as patience-sort tails.)
                let mut touched: Vec<u32> = Vec::new();
                for it in &items {
                    if charge(1) {
                        over_order = true;
                        break 'lists;
                    }
                    let Some(cands) = order_of.get(*it) else { continue };
                    for &(gid, pos) in cands {
                        if charge(2) {
                            over_order = true;
                            break 'lists;
                        }
                        let st = &mut order_state[gid as usize];
                        if st.0 == 0 {
                            touched.push(gid);
                        }
                        st.0 += 1;
                        let l = st.1.partition_point(|&t| t < pos);
                        if l == st.1.len() {
                            st.1.push(pos);
                        } else {
                            st.1[l] = pos;
                        }
                    }
                }
                for gid in touched {
                    let st = &mut order_state[gid as usize];
                    let (found, chain) = (st.0 as usize, st.1.len());
                    st.0 = 0;
                    st.1.clear();
                    if chain >= 3 && chain * 5 >= found * 3 {
                        order_hit = true;
                    }
                }
            }
            if over_order {
                // (A partly read list leaves its scratch dirty: clear it before the next text.)
                for st in order_state.iter_mut() {
                    st.0 = 0;
                    st.1.clear();
                }
                out.push(budget_err(&path));
                continue;
            }
            if order_hit {
                out.push(ValidationError { reason: Some("word_bank_order"), ack_allowed: Some(false), author_only: true, hint: Some(HINT_WORD_BANK), ..ValidationError::new("key_in_stem", &path) });
                continue;
            }
        }
        if !automaton {
            continue;
        }
        let fb = f.as_bytes();
        // The sequence automaton: the previous value's questions, the runs of consecutive
        // questions so far (question, length), the number just read, the numbered values found.
        let mut prev: Vec<u32> = Vec::new();
        let mut runs: Vec<(u32, u32)> = Vec::new();
        let mut number: Option<usize> = None;
        let numbered = std::cell::Cell::new(0usize);
        // (A number written as a question label, "Q1", names its question: one such value is
        // enough, "(Q1: C)".)
        let q_label = std::cell::Cell::new(false);
        let hit = std::cell::Cell::new(false);
        let mut over = false;
        let value = |cands: Vec<u32>, number: &mut Option<usize>, prev: &mut Vec<u32>, runs: &mut Vec<(u32, u32)>| -> bool {
            if format::spend_work((cands.len() + prev.len() + runs.len()) as u64 + 1) {
                return true;
            }
            if let Some(nq) = number.take()
                && nq >= 1
                && cands.binary_search(&((nq - 1) as u32)).is_ok()
            {
                numbered.set(numbered.get() + 1 + usize::from(q_label.get()));
            }
            let mut next: Vec<(u32, u32)> = runs.iter().filter(|(q, _)| cands.binary_search(&(q + 1)).is_ok()).map(|(q, l)| (q + 1, l + 1)).collect();
            // (New runs of two: a value of question q right after one of question q - 1.)
            let (mut i, mut j) = (0, 0);
            while i < prev.len() && j < cands.len() {
                match (prev[i] + 1).cmp(&cands[j]) {
                    std::cmp::Ordering::Less => i += 1,
                    std::cmp::Ordering::Greater => j += 1,
                    std::cmp::Ordering::Equal => {
                        if !next.iter().any(|x| x.0 == cands[j]) {
                            next.push((cands[j], 2));
                        }
                        i += 1;
                        j += 1;
                    }
                }
            }
            if next.iter().any(|x| x.1 >= 3) || numbered.get() >= 2 {
                hit.set(true);
            }
            *runs = next;
            *prev = cands;
            false
        };
        let reset = |number: &mut Option<usize>, prev: &mut Vec<u32>, runs: &mut Vec<(u32, u32)>| {
            *number = None;
            prev.clear();
            runs.clear();
        };
        // (Outside prose, a choice, an item, a topic: only a value right after its own question's
        // number counts; "A. A. A." labels and "Mercury, Venus and Earth" are content.)
        let unit = |cands: Vec<u32>, number: &mut Option<usize>, prev: &mut Vec<u32>, runs: &mut Vec<(u32, u32)>| -> bool {
            if prose {
                return value(cands, number, prev, runs);
            }
            if let Some(nq) = number.take()
                && nq >= 1
                && cands.binary_search(&((nq - 1) as u32)).is_ok()
            {
                numbered.set(numbered.get() + 1 + usize::from(q_label.get()));
                if numbered.get() >= 2 {
                    hit.set(true);
                }
            }
            prev.clear();
            runs.clear();
            false
        };
        let mut i = 0;
        while i < fb.len() && !hit.get() && !over {
            let c = f[i..].chars().next().unwrap_or(' ');
            let cl = c.len_utf8();
            if !c.fast_alnum() {
                // A True/False symbol is a value; other marks separate.
                if let Some(v) = tf_token(&f[i..i + cl])
                    && (prose || number.is_some())
                {
                    over = unit(tf_lists[usize::from(v)].clone(), &mut number, &mut prev, &mut runs);
                }
                i += cl;
                continue;
            }
            if i > 0 && f[..i].chars().next_back().is_some_and(crate::format::is_alnum) {
                i += cl;
                continue;
            }
            // At a word start: the longest key text here, else the word itself. (Outside prose a
            // key text counts only after its number: no walk without one.)
            let mut best: Option<(usize, Vec<u32>)> = None;
            let walk_max = if prose || number.is_some() { fb.len() - i } else { 0 };
            over |= lex.walk(fb, i, walk_max, &mut |e, ents| {
                if !f[e..].chars().next().is_some_and(crate::format::is_alnum) {
                    let mut qs: Vec<u32> = ents.iter().map(format::Lex::tag).collect();
                    qs.sort_unstable();
                    qs.dedup();
                    best = Some((e, qs));
                }
                false
            });
            let w_end = i + f[i..].find(|ch: char| !ch.fast_alnum()).unwrap_or(f.len() - i);
            let w = &f[i..w_end];
            // (A key text of more than four questions, "alpha" accepted everywhere, says nothing
            // of their order: only next to its number.)
            if let Some((e, qs)) = best.take() {
                let informative = prose && qs.len() <= 4;
                if informative {
                    over |= value(qs, &mut number, &mut prev, &mut runs);
                } else if let Some(nq) = number.take()
                    && nq >= 1
                    && qs.binary_search(&((nq - 1) as u32)).is_ok()
                {
                    // (A "Q" label names its question: one value is enough, "(Q1: Gills)".
                    // Round 17, critic31a A6's bank left item.)
                    numbered.set(numbered.get() + 1 + usize::from(q_label.get()));
                    if numbered.get() >= 2 {
                        hit.set(true);
                    }
                } else {
                    reset(&mut number, &mut prev, &mut runs);
                }
                i = e;
                continue;
            }
            // A number (a question number), "q1", or a number glued to a value ("1c", "135t").
            let digits = w.chars().take_while(char::is_ascii_digit).count();
            let lead_q = w.starts_with('q') && w[1..].chars().next().is_some_and(|c| c.is_ascii_digit());
            let (num_part, rest) = if lead_q {
                let d = w[1..].chars().take_while(char::is_ascii_digit).count();
                (&w[1..1 + d], &w[1 + d..])
            } else {
                (&w[..digits], &w[digits..])
            };
            if !num_part.is_empty() && num_part.len() <= 4 {
                number = num_part.parse().ok();
                q_label.set(lead_q);
                if rest.is_empty() {
                    i = w_end;
                    continue;
                }
            }
            let token = if num_part.is_empty() { w } else { rest };
            // (Outside prose with no number pending, a letter or value says nothing: skipped
            // without building its question list.)
            if !prose && number.is_none() {
                reset(&mut number, &mut prev, &mut runs);
                i = w_end;
                continue;
            }
            let mut cands: Vec<u32> = Vec::new();
            if let Some(v) = tf_token(token) {
                cands.extend(&tf_lists[usize::from(v)]);
            }
            if token.chars().count() == 1
                && let Some(l) = letters.get(&token.chars().next().unwrap_or(' '))
            {
                cands.extend(l);
            }
            if cands.is_empty() {
                reset(&mut number, &mut prev, &mut runs);
            } else {
                cands.sort_unstable();
                cands.dedup();
                over |= unit(cands, &mut number, &mut prev, &mut runs);
            }
            i = w_end;
        }
        if over {
            out.push(ValidationError { reason: Some("scan_budget"), ack_allowed: Some(false), author_only: true, hint: Some(HINT_BUDGET), ..ValidationError::new("key_in_stem", &path) });
        } else if hit.get() {
            out.push(ValidationError { reason: Some("bank_key_list"), ack_allowed: Some(false), author_only: true, hint: Some(HINT_STEM), ..ValidationError::new("key_in_stem", &path) });
        }
    }
    out
}

/// What the bank's name and description must not say: every question's key texts, True/False
/// values when a question has one, a letter or number naming a choice, an answer key of letters
/// ("Key: ABT"). Round 13 (critic24a B3): "Answers: 1. E7018 2. Vilnius 3. True".
pub fn bank_text_info(qs: &[Question]) -> (format::KeyInfo, bool) {
    let tf = qs.iter().any(|q| matches!(q.kind, Kind::Tf { .. }) || matches!(&q.kind, Kind::Multipart { parts } if parts.iter().any(|p| matches!(p.kind, PartKind::Tf { .. }))));
    let (texts, truncated) = bank_keys(qs);
    (format::KeyInfo { texts, choices: true, tf, letter_keys: true, lines_only: true, prefolded: true, ..Default::default() }.with_maps(), truncated)
}

/// Every question's key texts, folded (a long one by its start: `format::KEY_WINDOW`), at most
/// 65,536 distinct; whether some were left out (then answer lines fail closed).
fn bank_keys(qs: &[Question]) -> (Vec<String>, bool) {
    // (Every key, no cap: they go into one sorted arena, `format::Lex`, about 12 bytes each beyond
    // their text. Round 17: the 65,536 cap and its fail-closed path are gone.)
    let mut texts: Vec<String> = Vec::new();
    each_key(qs, &mut |t: &str| {
        let f = format::fold_key(t);
        if !f.is_empty() {
            texts.push(f);
        }
    });
    texts.sort_unstable();
    texts.dedup();
    (texts, false)
}

/// Every key text of a bank's questions (raw): correct choices with and without labels, samples,
/// accepted answers, paired rights, parts' keys.
fn each_key(qs: &[Question], add: &mut dyn FnMut(&str)) {
    // (A correct choice's text with and without its label: "B. Nitrogen" is named "Nitrogen".)
    let choice = |a: &Answer, add: &mut dyn FnMut(&str)| {
        add(&a.text);
        if let Some(h) = labels::detect(&a.text).filter(|h| h.start == 0 && text::has_visible(&a.text[h.end..])) {
            add(&a.text[h.end..]);
        }
    };
    for q in qs.iter().filter(|q| !q.keyless) {
        match &q.kind {
            Kind::Mc { answers } => answers.iter().filter(|a| a.correct).for_each(|a| choice(a, add)),
            Kind::Short { sample } => add(sample),
            Kind::Blank { accept } => accept.iter().flatten().for_each(|a| add(a)),
            Kind::Matching { pairs, .. } => pairs.iter().for_each(|p| add(&p.right)),
            Kind::Multipart { parts } => parts.iter().for_each(|p| match &p.kind {
                PartKind::Mc { answers } => answers.iter().filter(|a| a.correct).for_each(|a| choice(a, add)),
                PartKind::Short { sample } => add(sample),
                PartKind::Tf { .. } => {}
            }),
            Kind::Tf { .. } => {}
        }
    }
}

/// What the author can do about a refusal (the `hint` of each error).
pub(crate) const HINT_STEM: &str = "Students see this text: remove the answer from it (answer lines, GIFT or Cloze answer syntax, the paired item).";
/// One text's own step budget in the bank-wide scans (past it, `scan_budget`).
const TEXT_STEPS: u64 = 100_000;
/// A text's own limit on the bytes its pair walks read, summed over its starts.
const PAIR_WALK_STEPS: u32 = 50_000;
const HINT_WORD_BANK: &str = "The word bank lists the answers in the order of the blanks, and students see the stem as written: put the words in another order (alphabetical, or mixed).";
pub(crate) const HINT_BUDGET: &str = "This text takes more reading than a publish allows (it repeats answer words or keys many times): shorten it, or split the question or bank.";
const HINT_BANK_ORDER: &str = "This list holds the bank's answers in the order of its questions, and students get the questions in that order: put the list in another order (alphabetical, or mixed).";
const HINT_WORD_BANK_PAIR: &str = "The word bank lists the two answers in the order of the blanks, and students see the stem as written: put the words in another order, or, if the order is chance, acknowledge it with ack_formatting.";
const HINT_PART_KEY: &str = "This text shows another part's answer. If the value is given data of the problem (the preview shows the served text), acknowledge it with ack_formatting; otherwise remove it.";
const HINT_PASTED: &str = "The stem ends with both values on lines of their own, pasted from a quiz preview: remove them (students are shown the choices), or acknowledge them with ack_formatting.";
const HINT_HIGH: &str = "Students would see this mark next to the answer: remove it, or give every choice the same formatting.";
const HINT_LOW: &str = "This formatting is only on the answer (or only missing from it). If it is part of the content (code, notation, units, a quotation), acknowledge it with ack_formatting; otherwise remove it.";

fn format_error(path: String, f: &format::Flag) -> ValidationError {
    let low = f.risk == format::Risk::Low;
    ValidationError { reason: Some(f.reason), ack_allowed: Some(low), author_only: true, hint: Some(if low { HINT_LOW } else { HINT_HIGH }), ..ValidationError::new("key_in_formatting", &path) }
}

/// Every formatting and locked-flag refusal of one question as it would be served (see
/// `format`): Mc and Multipart Mc choices, and the Matching column (paired rights against
/// distractors, and rights that name or share a token with their left item). `qp` is the
/// question's path; `decision(part)` the author's label decision; `acked` the author's
/// `ack_formatting` for this question (low-risk features only).
pub fn question_format_errors(q: &Question, qp: &str, decision: &dyn Fn(Option<usize>) -> Option<LabelChoice>, acked: bool) -> Vec<ValidationError> {
    let mut out = Vec::new();
    // The key in served text: the stem, part stems, left items (round 8). High risk.
    stem_key_errors(q, qp, acked, decision, &mut out);
    // Round 15 (critic28a B1): Anki cloze in a choice, a right or a distractor, keyless too.
    {
        let mut texts: Vec<(String, &str)> = Vec::new();
        match &q.kind {
            Kind::Mc { answers } => texts.extend(answers.iter().enumerate().map(|(i, a)| (format!("{qp}.kind.Mc.answers[{i}]"), a.text.as_str()))),
            Kind::Matching { pairs, distractors } => {
                texts.extend(pairs.iter().enumerate().map(|(i, p)| (format!("{qp}.kind.Matching.pairs[{i}].right"), p.right.as_str())));
                texts.extend(distractors.iter().enumerate().map(|(i, d)| (format!("{qp}.kind.Matching.distractors[{i}]"), d.as_str())));
            }
            Kind::Multipart { parts } => {
                for (pi, part) in parts.iter().enumerate() {
                    if let PartKind::Mc { answers } = &part.kind {
                        texts.extend(answers.iter().enumerate().map(|(i, a)| (format!("{qp}.kind.Multipart.parts[{pi}].kind.Mc.answers[{i}]"), a.text.as_str())));
                    }
                }
            }
            _ => {}
        }
        for (path, t) in texts {
            if format::anki_cloze(t) {
                out.push(ValidationError { reason: Some("cloze"), ack_allowed: Some(false), author_only: true, hint: Some(HINT_STEM), ..ValidationError::new("key_in_formatting", &path) });
            }
        }
    }
    // A keyless question has no key to give away.
    if q.keyless {
        return out;
    }
    let mc = |answers: &[Answer], at: String, part: Option<usize>, out: &mut Vec<ValidationError>| {
        for f in mc_format_flags(answers, decision(part)) {
            if f.risk == format::Risk::High || !acked {
                out.push(format_error(format!("{at}[{}]", f.index), &f));
            }
        }
        if locked_leak(answers, decision(part), q.keep_order) {
            out.push(ValidationError { author_only: true, ..ValidationError::new("locked_key", &at) });
        }
    };
    match &q.kind {
        Kind::Mc { answers } => mc(answers, format!("{qp}.kind.Mc.answers"), None, &mut out),
        Kind::Multipart { parts } => {
            for (pi, part) in parts.iter().enumerate() {
                if let PartKind::Mc { answers } = &part.kind {
                    mc(answers, format!("{qp}.kind.Multipart.parts[{pi}].kind.Mc.answers"), Some(pi), &mut out);
                }
            }
        }
        Kind::Matching { pairs, distractors } => {
            let (column, paired) = import::right_column_paired(pairs, distractors);
            let refs: Vec<&str> = column.iter().map(String::as_str).collect();
            let cut = labels::analyze_column(&refs, paired).filter(|a| decision(None) == Some(LabelChoice::Strip) && a.strip_allowed).map(|a| a.cut);
            // (Only needed to apply Strip's cuts.)
            let index: BTreeMap<String, usize> = if cut.is_some() { column.iter().enumerate().map(|(k, c)| (text::hash_form(c), k)).collect() } else { BTreeMap::new() };
            let served = |t: &str| -> String {
                let k = if cut.is_some() { index.get(&text::hash_form(t)).copied() } else { None };
                match (&cut, k) {
                    (Some(c), Some(k)) => format::served_text(&labels::apply(t, c[k])),
                    _ => format::served_text(t),
                }
            };
            // The column as served (look-alikes once), each item with where it came from.
            if !distractors.is_empty() {
                let mut seen = BTreeSet::new();
                let mut items: Vec<(String, bool, String)> = Vec::new();
                let all = pairs.iter().enumerate().map(|(i, p)| (p.right.as_str(), true, format!("pairs[{i}].right")));
                for (t, is_pair, path) in all.chain(distractors.iter().enumerate().map(|(i, d)| (d.as_str(), false, format!("distractors[{i}]")))) {
                    if text::has_visible(t) && seen.insert(text::hash_form(t)) {
                        items.push((served(t), is_pair, path));
                    }
                }
                let c: Vec<(&str, bool)> = items.iter().map(|x| (x.0.as_str(), x.1)).collect();
                for f in format::check(&c) {
                    if f.risk == format::Risk::High || !acked {
                        out.push(format_error(format!("{qp}.kind.Matching.{}", items[f.index].2), &f));
                    }
                }
            }
            // Distractors tagged as such ("Lisbon (extra)", "Norway (not used)", "Chile – extra")
            // while no paired right item is: it tells which rights are unused. High risk (round 15,
            // critic28a; acknowledging it published the leak).
            let tagged = |t: &str| {
                let bracket = t.contains('(') && t.split('(').skip(1).any(|g| g.chars().take_while(|c| *c != ')').any(crate::format::is_alpha));
                let dashed = [" - ", " \u{2013} ", " \u{2014} "].iter().any(|d| t.rsplit_once(d).is_some_and(|(h, a)| h.chars().any(crate::format::is_alnum) && (1..=3).contains(&a.split_whitespace().count()) && a.chars().any(crate::format::is_alpha)));
                bracket || dashed
            };
            if !distractors.is_empty() && distractors.iter().all(|d| tagged(d)) && !pairs.iter().any(|p| tagged(&p.right)) {
                out.push(ValidationError { reason: Some("marked_distractors"), ack_allowed: Some(false), author_only: true, hint: Some(HINT_HIGH), ..ValidationError::new("key_in_formatting", &format!("{qp}.kind.Matching.distractors")) });
            }
            let rights: Vec<String> = pairs.iter().map(|p| served(&p.right)).collect();
            let pr: Vec<(&str, &str)> = pairs.iter().zip(&rights).map(|(p, r)| (p.left.as_str(), r.as_str())).collect();
            let ds: Vec<String> = distractors.iter().map(|d| served(d)).collect();
            let dr: Vec<&str> = ds.iter().map(String::as_str).collect();
            // Each pairing check once, with its risk; the highest per pair is reported.
            let mut refs: Vec<(usize, &'static str, format::Risk)> = format::matching_shared_tokens(&pr, &dr).into_iter().map(|(i, r)| (i, "pairing_token", r)).collect();
            if cut.is_none() {
                refs.extend(labels::matching_label_refs(&pr).into_iter().map(|i| (i, "pairing_token", format::Risk::High)));
            }
            refs.extend(format::matching_names_left(&pr).into_iter().map(|(i, r)| (i, "names_left", r)));
            refs.extend(format::matching_initials(&pr).into_iter().map(|i| (i, "initials", format::Risk::Low)));
            refs.sort_by_key(|x| (x.0, x.1, std::cmp::Reverse(x.2)));
            refs.dedup_by_key(|x| (x.0, x.1));
            for (i, reason, risk) in refs {
                if risk == format::Risk::High || !acked {
                    out.push(ValidationError { reason: Some(reason), ack_allowed: Some(risk == format::Risk::Low), author_only: true, ..ValidationError::new("key_in_formatting", &format!("{qp}.kind.Matching.pairs[{i}].right")) });
                }
            }
        }
        _ => {}
    }
    out
}

fn clear_origin(o: &mut Option<Origin>) {
    if let Some(o) = o.as_mut() {
        o.served_sha256.clear();
        o.match_hash.clear();
    }
}

fn strip_one(mut s: Question, image_ok: bool, pos: usize, cx: &mut Ctx) -> Question {
    // Formatting is judged on the served form (labels as decided), before anything is removed.
    if matches!(cx.order, Order::Shuffle(_)) && !cx.formatting_checked {
        let labels = cx.labels;
        for e in question_format_errors(&s, &format!("question[{pos}]"), &|part| labels.get(&(pos, part)).copied(), cx.acks.contains(&pos)) {
            if cx.errors.len() < MAX_ERRORS {
                cx.errors.push(e);
            }
        }
    }
    if cx.drop_provenance {
        s.origin = None;
    }
    s.explanation = None;
    // Round 9: every text students see goes through the served normalisation (the key checks
    // judge exactly that).
    s.stem = format::served_text(&s.stem);
    for t in s.topics.iter_mut() {
        *t = format::served_text(t);
    }
    if s.figure.take().is_some() {
        cx.out.figures_removed.push(pos);
    }
    if s.image.is_some() {
        if image_ok {
            cx.out.images_kept.push(pos);
        } else {
            s.image = None;
            cx.out.images_removed.push(pos);
        }
    }
    s.published = None;
    clear_origin(&mut s.origin);
    s.keyless = true;
    s.rekeyed = false;
    cx.keep_order = s.keep_order;
    let at = |part| Pos { question: pos, part };
    match &mut s.kind {
        Kind::Mc { answers } => strip_mc(answers, at(None), cx),
        Kind::Tf { correct } => *correct = false,
        Kind::Matching { pairs, distractors } => word_bank(pairs, distractors, pos, cx),
        Kind::Blank { accept } => {
            for a in accept.iter_mut() {
                a.clear();
            }
        }
        Kind::Short { sample } => sample.clear(),
        Kind::Multipart { parts } => {
            for (pi, p) in parts.iter_mut().enumerate() {
                p.stem = format::served_text(&p.stem);
                match &mut p.kind {
                    PartKind::Mc { answers } => strip_mc(answers, at(Some(pi)), cx),
                    PartKind::Tf { correct } => *correct = false,
                    PartKind::Short { sample } => sample.clear(),
                }
            }
        }
    }
    s
}

fn empty_report(payload: Payload) -> Stripped {
    Stripped {
        payload,
        needs_order_ack: Vec::new(),
        order_reasons: Vec::new(),
        labels_removed: Vec::new(),
        label_findings: Vec::new(),
        images_kept: Vec::new(),
        images_removed: Vec::new(),
        figures_removed: Vec::new(),
        author_only: AuthorOnly::default(),
    }
}

fn check_options(p: &Payload, opts: &StripOptions) -> Result<(), Vec<ValidationError>> {
    let n = p.questions().len();
    let mut bad: Vec<ValidationError> = Vec::new();
    for (k, i) in opts.image_ok.iter().enumerate() {
        if *i >= n {
            if bad.len() >= MAX_ERRORS {
                break;
            }
            bad.push(ValidationError { limit: Some(n as u64), ..ValidationError::new("image_ok_out_of_range", &format!("image_ok[{k}]")) });
        }
    }
    for (k, d) in opts.labels.iter().enumerate() {
        if d.question >= n && bad.len() < MAX_ERRORS {
            bad.push(ValidationError { limit: Some(n as u64), ..ValidationError::new("labels_out_of_range", &format!("labels[{k}]")) });
        }
    }
    if bad.is_empty() { Ok(()) } else { Err(bad) }
}

fn decisions(opts: &StripOptions) -> BTreeMap<(usize, Option<usize>), LabelChoice> {
    opts.labels.iter().map(|d| ((d.question, d.part), d.choice)).collect()
}

/// Strips `p` (consumed: no second copy of a large bank is made).
fn strip_owned(p: Payload, opts: &StripOptions, order: Order) -> Result<Stripped, Vec<ValidationError>> {
    // (One work budget for the scans of this payload, round 16.)
    let _work = crate::format::WorkScope::new();
    check_options(&p, opts)?;
    let de = decision_errors(&p, &opts.labels);
    if !de.is_empty() {
        return Err(de);
    }
    let image_ok: BTreeSet<usize> = opts.image_ok.iter().copied().collect();
    let labels = decisions(opts);
    let schema_version = p.schema_version.max(SCHEMA_STRIPPED);
    let mut cx = Ctx {
        order,
        labels: &labels,
        out: empty_report(Payload { schema_version, body: Body::Question(placeholder()) }),
        errors: Vec::new(),
        keep_order: false,
        acks: opts.ack_formatting.iter().copied().collect(),
        drop_provenance: opts.drop_client_provenance,
        formatting_checked: opts.formatting_checked,
    };
    let body = match p.body {
        Body::Bank(mut b) => {
            let qs = std::mem::take(&mut b.questions);
            b.questions = qs.into_iter().enumerate().map(|(i, q)| strip_one(q, image_ok.contains(&i), i, &mut cx)).collect();
            b.published = None;
            b.name = format::served_text(&b.name);
            b.details.description = format::served_text(&b.details.description);
            clear_origin(&mut b.origin);
            if cx.drop_provenance {
                b.origin = None;
            }
            Body::Bank(b)
        }
        Body::Question(q) => Body::Question(strip_one(q, image_ok.contains(&0), 0, &mut cx)),
    };
    if !cx.errors.is_empty() {
        return Err(cx.errors);
    }
    let mut out = cx.out;
    out.payload.body = body;
    out.needs_order_ack.sort();
    out.needs_order_ack.dedup();
    out.order_reasons.sort();
    Ok(out)
}

fn placeholder() -> Question {
    Question {
        id: String::new(),
        stem: String::new(),
        kind: Kind::Tf { correct: false },
        points: None,
        difficulty: None,
        topics: vec![],
        image: None,
        explanation: None,
        figure: None,
        created: 0,
        modified: 0,
        keyless: true,
        keep_order: false,
        origin: None,
        published: None,
        rekeyed: false,
    }
}

/// The stripped form of a validated payload, shuffled with a seed drawn from the platform's
/// cryptographic generator. This is the production entry point. Refused with
/// `labels_need_decision` when a label finding has no decision (see [`label_findings`]).
pub fn strip(p: Payload, opts: &StripOptions) -> Result<Stripped, Vec<ValidationError>> {
    let mut rng = ShuffleRng::from_entropy().map_err(|_| vec![ValidationError::new("rng_unavailable", "")])?;
    strip_owned(p, opts, Order::Shuffle(&mut rng))
}

/// `strip` with an explicit seed: **tests and fixtures only** (a caller-chosen seed is a
/// caller-chosen order). Not exported to wasm unless built with the `test-seed` feature.
pub fn strip_with_seed(p: Payload, opts: &StripOptions, seed: [u8; 32]) -> Result<Stripped, Vec<ValidationError>> {
    // (One work budget for the scans of this payload, round 16.)
    let _work = crate::format::WorkScope::new();
    let mut rng = ShuffleRng::from_seed(seed);
    strip_owned(p, opts, Order::Shuffle(&mut rng))
}

/// Total preview text `label_findings` returns before it leaves the rest out (`truncated`).
pub const PREVIEW_BUDGET: usize = 256 * 1024;

/// Every label finding of a payload, undecided, for the author's preview before publishing.
/// Previews stop after [`PREVIEW_BUDGET`] bytes of text; later findings are listed with
/// `truncated: true` and no preview (the app computes the full preview locally with this crate).
pub fn label_findings(p: &Payload) -> Vec<LabelFinding> {
    findings(p, Some(PREVIEW_BUDGET))
}

/// The findings without any preview: what the publish checks need.
pub fn label_finding_flags(p: &Payload) -> Vec<LabelFinding> {
    findings(p, None)
}

fn findings(p: &Payload, mut budget: Option<usize>) -> Vec<LabelFinding> {
    let mut out = Vec::new();
    let mut push = |q: usize, part: Option<usize>, matching: Option<usize>, t: Vec<&str>| {
        if let Some(a) = match matching {
            Some(paired) => labels::analyze_column(&t, paired),
            None => labels::analyze(&t),
        } {
            let matching = matching.is_some();
            let size: usize = t.iter().map(|x| 2 * x.len()).sum();
            let (choices, truncated) = match budget.as_mut() {
                None => (Vec::new(), false),
                Some(b) if size <= *b => {
                    *b -= size;
                    (a.previews(&t), false)
                }
                Some(_) => (Vec::new(), true),
            };
            out.push(LabelFinding { question: q, part, matching, order_revealing: a.order_revealing, strip_allowed: a.strip_allowed, decision: None, choices, truncated });
        }
    };
    for (i, q) in p.questions().iter().enumerate() {
        match &q.kind {
            Kind::Mc { answers } => push(i, None, None, texts(answers)),
            Kind::Matching { pairs, distractors } => {
                let (column, paired) = import::right_column_paired(pairs, distractors);
                push(i, None, Some(paired), column.iter().map(String::as_str).collect());
            }
            Kind::Multipart { parts } => {
                for (pi, part) in parts.iter().enumerate() {
                    if let PartKind::Mc { answers } = &part.kind {
                        push(i, Some(pi), None, texts(answers));
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// The label decisions checked against the findings (without previews): a finding with no
/// decision (`labels_need_decision`), two decisions for one question or part
/// (`labels_duplicate`), a decision with no finding (`labels_unused`), or Strip where it is not
/// allowed (`labels_strip_refused`). `validate_for_publish` and `strip` both use this.
pub fn decision_errors(p: &Payload, decisions: &[LabelDecision]) -> Vec<ValidationError> {
    let found = label_finding_flags(p);
    let mut errors = Vec::new();
    let mut push = |code, at: Pos| {
        if errors.len() < MAX_ERRORS {
            errors.push(ValidationError::new(code, &label_path(at)));
        }
    };
    // Maps, each decision and finding looked at once: O((d + f) log n), whatever their number.
    let findings: BTreeSet<(usize, Option<usize>)> = found.iter().map(|f| (f.question, f.part)).collect();
    let mut chosen: BTreeMap<(usize, Option<usize>), LabelChoice> = BTreeMap::new();
    for d in decisions {
        let at = Pos { question: d.question, part: d.part };
        if chosen.insert((d.question, d.part), d.choice).is_some() {
            push("labels_duplicate", at);
        } else if !findings.contains(&(d.question, d.part)) {
            push("labels_unused", at);
        }
    }
    for f in &found {
        let at = Pos { question: f.question, part: f.part };
        match chosen.get(&(f.question, f.part)) {
            None => push("labels_need_decision", at),
            Some(LabelChoice::Strip) if !f.strip_allowed => push("labels_strip_refused", at),
            Some(LabelChoice::Keep) if f.matching && f.order_revealing => push("labels_keep_refused", at),
            _ => {}
        }
    }
    errors
}

/// One question's decisions out of a payload's.
pub fn question_labels(all: &[LabelDecision], question: usize) -> QuestionLabels {
    all.iter().filter(|d| d.question == question).map(|d| (d.part, d.choice)).collect()
}

/// One question's stripped form with a given generator and decisions (tests).
pub fn strip_question(q: &Question, image_ok: bool, labels: &QuestionLabels, rng: &mut ShuffleRng) -> Result<Question, Vec<ValidationError>> {
    let map: BTreeMap<(usize, Option<usize>), LabelChoice> = labels.iter().map(|(p, c)| ((0, *p), *c)).collect();
    let mut cx = Ctx { order: Order::Shuffle(rng), labels: &map, out: empty_report(Payload { schema_version: SCHEMA_STRIPPED, body: Body::Question(placeholder()) }), errors: Vec::new(), keep_order: false, acks: BTreeSet::new(), drop_provenance: false, formatting_checked: false };
    let s = strip_one(q.clone(), image_ok, 0, &mut cx);
    if cx.errors.is_empty() { Ok(s) } else { Err(cx.errors) }
}

/// The spec's `strip_without_shuffle`: the stripped form with every unordered list **sorted**
/// instead of shuffled, labels removed only where `labels` says Strip (and Strip is allowed).
/// `match_hash` covers exactly what survives this (see `hash`). Undecided findings keep their text.
pub fn strip_without_shuffle(q: &Question, labels: &QuestionLabels) -> Question {
    // Undecided findings are treated as Keep here (hashing never fails).
    let mut map: BTreeMap<(usize, Option<usize>), LabelChoice> = BTreeMap::new();
    let mut fill = |part: Option<usize>, t: Vec<&str>| {
        // (Strip/Keep only: the same for `analyze` and `analyze_column`.)
        if let Some(a) = labels::analyze(&t) {
            let c = match labels.get(&part) {
                Some(LabelChoice::Strip) if a.strip_allowed => LabelChoice::Strip,
                _ => LabelChoice::Keep,
            };
            map.insert((0, part), c);
        }
    };
    match &q.kind {
        Kind::Mc { answers } => fill(None, texts(answers)),
        Kind::Matching { pairs, distractors } => {
            let column = import::right_column(pairs, distractors);
            fill(None, column.iter().map(String::as_str).collect());
        }
        Kind::Multipart { parts } => {
            for (pi, p) in parts.iter().enumerate() {
                if let PartKind::Mc { answers } = &p.kind {
                    fill(Some(pi), texts(answers));
                }
            }
        }
        _ => {}
    }
    let mut cx = Ctx { order: Order::Sort, labels: &map, out: empty_report(Payload { schema_version: SCHEMA_STRIPPED, body: Body::Question(placeholder()) }), errors: Vec::new(), keep_order: false, acks: BTreeSet::new(), drop_provenance: false, formatting_checked: true };
    strip_one(q.clone(), false, 0, &mut cx)
}

// ---- previews for the author (before publishing) ---------------------------------------------------

/// One formatting finding (see `format`): where, why, how risky, whether `ack_formatting` may
/// accept it, and the choice as it would be served.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FormattingFinding {
    pub question: usize,
    pub code: &'static str,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<&'static str>,
    pub ack_allowed: bool,
    /// Whether this finding names the key (and so must reach the author only). Round 17,
    /// critic31b: every formatting finding names the key's choice, unlike the refusal's error
    /// list, which already carries the flag.
    pub author_only: bool,
    /// The choice text as students would see it (Mc choices).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub served: Option<String>,
}

/// Every formatting finding of a payload, question by question (labels as decided, Keep where not):
/// what `validate_for_publish` with `hide_key` and `strip` would refuse, low-risk findings
/// included (an acknowledgement removes those).
pub fn formatting_findings(p: &Payload, decisions: &[LabelDecision]) -> Vec<FormattingFinding> {
    // (One work budget for the scans of this payload, round 16.)
    let _work = crate::format::WorkScope::new();
    findings_of(p, decisions, None)
}

/// The findings of publish errors already computed (`publish_errors`): the `key_in_formatting`,
/// `key_in_stem` and `locked_key` ones, with the served text. What `publish_prepare` returns
/// with a refusal, without checking anything twice.
pub fn findings_from_errors(p: &Payload, decisions: &[LabelDecision], errors: &[ValidationError]) -> Vec<FormattingFinding> {
    // (One work budget for the scans of this payload, round 16.)
    let _work = crate::format::WorkScope::new();
    findings_of(p, decisions, Some(errors))
}

fn findings_of(p: &Payload, decisions: &[LabelDecision], given: Option<&[ValidationError]>) -> Vec<FormattingFinding> {
    let map: BTreeMap<(usize, Option<usize>), LabelChoice> = decisions.iter().map(|d| ((d.question, d.part), d.choice)).collect();
    let mut out = Vec::new();
    for (i, q) in p.questions().iter().enumerate() {
        let qp = match p.body {
            Body::Bank(_) => format!("bank.questions[{i}]"),
            Body::Question(_) => "question".to_string(),
        };
        let dec = |part: Option<usize>| Some(map.get(&(i, part)).copied().unwrap_or(LabelChoice::Keep));
        // The served text of an Mc choice, by path.
        let served_of = |path: &str| -> Option<String> {
            let tail = path.strip_prefix(&qp)?;
            let (answers, part) = match &q.kind {
                Kind::Mc { answers } if tail.starts_with(".kind.Mc.answers[") => (answers, None),
                Kind::Multipart { parts } if tail.starts_with(".kind.Multipart.parts[") => {
                    let pi: usize = tail[".kind.Multipart.parts[".len()..].split(']').next()?.parse().ok()?;
                    match &parts.get(pi)?.kind {
                        PartKind::Mc { answers } => (answers, Some(pi)),
                        _ => return None,
                    }
                }
                _ => return None,
            };
            let k: usize = tail.rsplit('[').next()?.trim_end_matches(']').parse().ok()?;
            served_views(answers, dec(part)).first()?.get(k).cloned()
        };
        let errors: Vec<ValidationError> = match given {
            None => question_format_errors(q, &qp, &dec, false),
            Some(g) => {
                // ("question" is a prefix of nothing else; a bank path continues with '.')
                let mine = |path: &str| path.strip_prefix(&qp).is_some_and(|t| t.is_empty() || t.starts_with('.'));
                g.iter().filter(|e| matches!(e.code, "key_in_formatting" | "key_in_stem" | "locked_key") && mine(&e.path)).cloned().collect()
            }
        };
        for e in errors {
            out.push(FormattingFinding { question: i, code: e.code, served: served_of(&e.path), path: e.path, reason: e.reason, ack_allowed: e.ack_allowed.unwrap_or(false), author_only: e.author_only });
        }
    }
    out
}

/// What will move when a question is served, before anything is published.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OrderPreview {
    pub question: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part: Option<usize>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub matching: bool,
    /// The choices (Mc) that will be shuffled; empty when the order is kept. A word bank is always
    /// shuffled.
    pub movable: Vec<usize>,
    /// Why the order is kept (`sibling_refs`, `author_keep_order`, `few_unlocked`, `labels_kept`);
    /// absent when it is shuffled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frozen: Option<OrderReason>,
    /// Labels count the choices off: Keep would keep the order; Strip would shuffle.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub frozen_if_keep: bool,
}

/// The order preview of every Mc question or part and Matching word bank (labels as decided,
/// undecided ones reported as `frozen_if_keep`). The author can set `keep_order` on a question to
/// keep its order whatever this says.
pub fn order_preview(p: &Payload, decisions: &[LabelDecision]) -> Vec<OrderPreview> {
    let map: BTreeMap<(usize, Option<usize>), LabelChoice> = decisions.iter().map(|d| ((d.question, d.part), d.choice)).collect();
    let mut out = Vec::new();
    for (i, q) in p.questions().iter().enumerate() {
        let mut mc = |answers: &[Answer], part: Option<usize>| {
            let decision = map.get(&(i, part)).copied();
            let analysis = labels::analyze(&texts(answers));
            let frozen_if_keep = analysis.as_ref().is_some_and(|a| a.order_revealing) && decision.is_none();
            // Undecided labels are previewed as Strip where allowed, else Keep (`frozen_if_keep` says
            // what Keep would do).
            let assumed = decision.or_else(|| analysis.as_ref().map(|a| if a.strip_allowed { LabelChoice::Strip } else { LabelChoice::Keep }));
            let (frozen, movable) = match plan_mc(answers, assumed, q.keep_order) {
                Ok(plan) => {
                    let unlocked: Vec<usize> = answers.iter().enumerate().filter(|(_, a)| !a.locked).map(|(k, _)| k).collect();
                    let reason = plan.keep_order.or((unlocked.len() < 2 && !answers.is_empty()).then_some(OrderReason::FewUnlocked));
                    (reason, if reason.is_some() { Vec::new() } else { unlocked })
                }
                Err(_) => (None, Vec::new()),
            };
            out.push(OrderPreview { question: i, part, matching: false, movable, frozen, frozen_if_keep });
        };
        match &q.kind {
            Kind::Mc { answers } => mc(answers, None),
            Kind::Multipart { parts } => {
                for (pi, part) in parts.iter().enumerate() {
                    if let PartKind::Mc { answers } = &part.kind {
                        mc(answers, Some(pi));
                    }
                }
            }
            Kind::Matching { pairs, distractors } => {
                let n = import::right_column(pairs, distractors).len();
                out.push(OrderPreview { question: i, part: None, matching: true, movable: (0..n).collect(), frozen: None, frozen_if_keep: false });
            }
            _ => {}
        }
    }
    out
}
