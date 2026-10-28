//! Circles phase D3: the payload for a partial send (docs/circles-phaseD-plan.md section 6). Pure: no egui, no
//! network, no files. The caller reads the base version's body once, works out which questions changed against it,
//! and ticks the changes to send.
//!
//! `changed` is authoritative: the caller's send summary compares every field the server hashes (so a change to a
//! `figure` counts, unlike a `merge::Snap`, which keeps neither a figure nor always a difficulty). An added question
//! (no circle id) is a change too; an id in `changed` that is not in this bank is ignored. A move alone is not a
//! change: the order sent follows this bank.
//!
//! An unchanged question is sent either way. An unticked change keeps the base version as it was: a changed question
//! is sent as the base's own raw question JSON (with its `published.qid`, so the server keeps its identity and sees
//! it as unchanged), and a removal (a pair in `link.qids` whose question the bank no longer has) is put back where
//! the base had it. The base body must therefore round-trip as unchanged; that is the plan's risk and is checked
//! against a local worker at staging, not here.
//!
//! On `Err` the caller must fall back to a whole-bank send and say so plainly to the person (plan 4.7), so a bank
//! that could not be composed is never sent silently as everything.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::circle_links::{CircleLink, SEND_SCHEMA, published_block};
use super::merge::same_question;
use crate::model::{ImageRef, Question, QuestionBank, is_keyless};

/// The base body could not be read, so which questions changed can not be told.
const UNREADABLE: &str = "The version this copy is based on could not be read.";
/// A change has no question in the base body: there is nothing to send or to put back.
const MISSING: &str = "The version this copy is based on does not have one of the questions this copy changed or removed.";

/// [`changes_against_body`]'s error for a body it can not read. It says what the caller does then (plan 4.7).
const UNREADABLE_CHANGES: &str = "The version this copy is based on could not be read, so its changes can not be listed. Send everything instead.";
/// [`changes_against_body`]'s error for a linked question the body does not have. Same fallback.
const MISSING_CHANGES: &str = "The version this copy is based on does not have one of this copy's linked questions, so its changes can not be listed. Send everything instead.";

/// The unsent changes of a bank against the base version's body (plan section 6): the questions the send summary
/// ticks. A pure function: no UI, no network, no files.
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct BodyChanges {
    /// Local ids of questions in the bank that differ from the base body's own question for their qid.
    pub changed: BTreeSet<String>,
    /// Local ids of questions with no circle id.
    pub added: BTreeSet<String>,
    /// Local ids in `link.qids` whose question the bank no longer has.
    pub removed: BTreeSet<String>,
}

impl BodyChanges {
    /// Every change's local id: a changed question, an added question, a removed one. The send summary ticks these,
    /// and the ones left unticked are what the link keeps in `held_back`.
    pub fn ids(&self) -> BTreeSet<String> {
        self.changed.iter().chain(&self.added).chain(&self.removed).cloned().collect()
    }

    /// Whether `local` is one of the changes (only a change may be ticked).
    pub fn contains(&self, local: &str) -> bool {
        self.changed.contains(local) || self.added.contains(local) || self.removed.contains(local)
    }

    /// Whether the bank has nothing to send that differs from its base.
    pub fn is_empty(&self) -> bool {
        self.changed.is_empty() && self.added.is_empty() && self.removed.is_empty()
    }
}

/// (phase D3, plan 4.7) What kind of change a send summary row is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendKind {
    Changed,
    Added,
    Removed,
}

/// (phase D3, plan 4.7) One row of the send summary as the page draws it: a change's kind, the question's number,
/// and the stem in quotes. `number` is the question's place among this bank's questions (a removal's place in the
/// base); both are `None` for a removal whose base snapshot a bounded link no longer keeps, which the page lists as a
/// plain "Removed" row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SendRow {
    /// The local id the tick box toggles (`toggle_change`).
    pub local: String,
    pub kind: SendKind,
    pub number: Option<usize>,
    pub stem: Option<String>,
}

/// (phase D3, plan 4.7) The send summary's rows, in the order the page draws them: this bank in order (changed and
/// added questions as they stand here), then the removals in the base's order. A removal lists the base's
/// last-known stem from `link.base`; when the link is bounded and kept no snapshot for it, the row has no stem and
/// takes its number from the pair's place in `link.qids`, so two such removals can still be told apart. Pure: no UI,
/// no network, no files.
///
/// A removal's number is its 1-based place in `link.base`. That is the base order as this copy keeps it; after a
/// partial send `circle_links::after_partial_send` can leave a question's snapshot out when this build could not read
/// it, so a later summary's removal numbers can shift by one in that rare case (the local ids and the list itself do
/// not change).
pub fn send_rows(link: &CircleLink, bank: &QuestionBank, changes: &BodyChanges) -> Vec<SendRow> {
    let mut rows = Vec::new();
    for (i, q) in bank.questions.iter().enumerate() {
        let kind = if changes.changed.contains(&q.id) {
            SendKind::Changed
        } else if changes.added.contains(&q.id) {
            SendKind::Added
        } else {
            continue;
        };
        rows.push(SendRow { local: q.id.clone(), kind, number: Some(i + 1), stem: Some(q.stem.clone()) });
    }
    for (i, b) in link.base.iter().enumerate() {
        if changes.removed.contains(&b.local_id) {
            rows.push(SendRow { local: b.local_id.clone(), kind: SendKind::Removed, number: Some(i + 1), stem: Some(b.snap.stem.clone()) });
        }
    }
    for id in &changes.removed {
        if !link.base.iter().any(|b| b.local_id == *id) {
            // A bounded link: the pairs keep the base order even when their snapshots were not kept.
            let number = link.qids.iter().position(|p| p.local == *id).map(|i| i + 1);
            rows.push(SendRow { local: id.clone(), kind: SendKind::Removed, number, stem: None });
        }
    }
    rows
}

/// One question of the composed bank, and the circle id it keeps (none: new to the circle).
struct Slot {
    value: Value,
    local: String,
    qid: Option<String>,
    question: Option<Question>,
}

/// One question of the composed payload, for the bookkeeping a partial send leaves behind: its local id in this
/// bank, the circle id it keeps (none for a question new to the circle, which the send's answer then names), and the
/// question itself when this build can read it. A held or restored raw question written by a newer client (an
/// unknown `Kind` or `Difficulty`) is still composed into the payload as it is, but has no `Question` here: its
/// local id and qid pair is kept, its base snapshot is not.
#[derive(Clone, Debug)]
pub struct ComposedQuestion {
    pub local: String,
    pub qid: Option<String>,
    pub question: Option<Question>,
}

/// What [`compose_partial_parts`] made: the send payload, and each of its questions with the local id it stands for.
#[derive(Clone, Debug)]
pub struct ComposedPartial {
    pub payload: Vec<u8>,
    pub questions: Vec<ComposedQuestion>,
}

/// The base body's questions by circle id (the first with each id) and in their order.
///
/// A base body is usually the circle's own (each question's `id` is its circle id). A payload composed here keeps a
/// linked question's local `id` and its circle id in `published.qid`, so such a question is indexed under both, and
/// the order uses the circle id where there is one. A ticked addition has no `published.qid` and is indexed only by
/// its local id, so a composed payload is not in general the next comparison's base: the app reads the circle's
/// served version for that, where every question's `id` is its qid.
///
/// Shared by [`compose_partial_parts`] and [`changes_against_body`] so the two can not read the base differently.
fn base_index(base: &Value) -> Result<(BTreeMap<&str, &Value>, Vec<&str>), String> {
    let base_qs = base.get("bank").and_then(|b| b.get("questions")).and_then(Value::as_array).ok_or_else(|| UNREADABLE.to_string())?;
    let mut raw: BTreeMap<&str, &Value> = BTreeMap::new();
    let mut order: Vec<&str> = Vec::new();
    for q in base_qs {
        let id = q.get("id").and_then(Value::as_str).filter(|id| !id.is_empty());
        let published = q.pointer("/published/qid").and_then(Value::as_str).filter(|id| !id.is_empty());
        if let Some(key) = published.or(id) {
            order.push(key);
        }
        if let Some(id) = id {
            raw.entry(id).or_insert(q);
        }
        if let Some(published) = published {
            raw.entry(published).or_insert(q);
        }
    }
    Ok((raw, order))
}

/// The unsent changes of `bank` against the base version's body, comparing every field the server hashes (so a change
/// to `figure` or `difficulty` counts, unlike `merge::ours_changed` against a snapshot). `Err` when the body can not be
/// read or a linked question has no question in it, and the caller falls back to a whole-bank send (plan 4.7).
///
/// A bank question with no circle id is added. A bank question with a circle id is changed when its content differs
/// from the base body's question for that id: the server's content hash (`server/worker/src/` `core.ts`, `contentOf`)
/// drops `id`, `created`, `modified`, `origin` and `published` and hashes the rest, whose texts it has already put in
/// its stored form (`share-core` `text::clean`; see `merge::same_question` for the client's copy of that). A pair in
/// `link.qids` whose local id is not in the bank is removed; the full pairs are used, not `link.base`, so a bounded
/// link still lists its removals. A move alone is not a change.
///
/// `Err` when the bank is empty (the composer refuses it too), the body can not be read, or a linked question in
/// this bank has no question in it. A removal the body lacks is still listed; the composer refuses only if that
/// removal is unticked and so would have to be put back.
pub fn changes_against_body(link: &CircleLink, bank: &QuestionBank, base_body: &[u8]) -> Result<BodyChanges, String> {
    if bank.questions.is_empty() {
        return Err("The bank has no questions. A circle bank needs at least one.".into());
    }
    let base: Value = serde_json::from_slice(base_body).map_err(|_| UNREADABLE_CHANGES.to_string())?;
    let (raw, _order) = base_index(&base).map_err(|_| UNREADABLE_CHANGES.to_string())?;
    let mut out = BodyChanges::default();
    for q in &bank.questions {
        let Some(qid) = link.qid_of(&q.id).filter(|id| !id.is_empty()) else {
            out.added.insert(q.id.clone());
            continue;
        };
        let base_q = raw.get(qid).ok_or_else(|| MISSING_CHANGES.to_string())?;
        let base_q: Question = serde_json::from_value((*base_q).clone()).map_err(|_| UNREADABLE_CHANGES.to_string())?;
        if !same_content(q, &base_q) {
            out.changed.insert(q.id.clone());
        }
    }
    for p in &link.qids {
        if p.qid.is_empty() {
            continue;
        }
        if !bank.questions.iter().any(|q| q.id == p.local) {
            out.removed.insert(p.local.clone());
        }
    }
    Ok(out)
}

/// Whether a bank question is the same as the base body's question for its circle id, on every field the server
/// hashes. `merge::same_question` covers the stem, choices and key, explanation, points, topics, difficulty and figure
/// with the texts in the server's stored form; the picture and the keyless placeholder are compared here because
/// `same_question` compares pictures apart and `explanation_of` drops the keyless sentinel.
fn same_content(ours: &Question, base: &Question) -> bool {
    same_question(ours, base) && same_image(ours.image.as_ref(), base.image.as_ref()) && is_keyless(ours) == is_keyless(base)
}

/// The picture as the server hashes it: its name on the server and its size (an absent one is a change from a
/// picture, and the other way round).
fn same_image(a: Option<&ImageRef>, b: Option<&ImageRef>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a.file == b.file && a.width == b.width && a.height == b.height,
        _ => false,
    }
}

/// The payload for a partial send: this bank in order, with every unticked change left as the base has it.
///
/// `changed` holds the local ids the send summary found changed (an added question included; removals need no
/// entry); `ticked` holds those of them to send. A question with a circle id is sent as this bank has it unless it
/// is changed and unticked, when it is the base body's raw question; an added question is sent only when ticked.
/// An unchanged question is sent either way. An id in `changed` that is not in this bank is ignored.
///
/// The envelope is the same as [`super::circle_links::payload_of`]'s. `Err` (the caller then falls back to a
/// whole-bank send and must say so plainly, plan 4.7) when the bank is empty, the base body can not be read, or a
/// changed or removed qid has no question in it.
pub fn compose_partial(link: &CircleLink, bank: &QuestionBank, base_body: &[u8], changed: &BTreeSet<String>, ticked: &BTreeSet<String>) -> Result<Vec<u8>, String> {
    Ok(compose_partial_parts(link, bank, base_body, changed, ticked)?.payload)
}

/// [`compose_partial`] with the composed questions kept: this bank in order, with every unticked change left as the
/// base has it, and for each question its local id and the circle id it keeps. The send's bookkeeping
/// (`circle_links::after_partial_send`) needs them to rebuild the base snapshot and the local id -> qid pairs.
///
/// A restored removal's composed question is the base body's raw question, whose own `id` is the circle's: its local
/// id here is the pair's local id. A question new to the circle has no qid yet; the send's answer names it. A held or
/// restored raw question this build can not deserialise (a newer client's `Kind` or `Difficulty`) still goes into the
/// payload byte for byte, but its `question` is `None`: composing never refuses a payload that is right.
pub fn compose_partial_parts(link: &CircleLink, bank: &QuestionBank, base_body: &[u8], changed: &BTreeSet<String>, ticked: &BTreeSet<String>) -> Result<ComposedPartial, String> {
    if bank.questions.is_empty() {
        return Err("The bank has no questions. A circle bank needs at least one.".into());
    }
    let base: Value = serde_json::from_slice(base_body).map_err(|_| UNREADABLE.to_string())?;
    // Each base question's raw JSON, by its circle id, and the base order (for putting a removal back).
    let (raw, order) = base_index(&base)?;

    // This bank in order. A changed question that is unticked is replaced by the base's raw question; an added
    // question that is unticked is left out.
    let mut out: Vec<Slot> = Vec::new();
    for q in &bank.questions {
        let Some(qid) = link.qid_of(&q.id).filter(|id| !id.is_empty()) else {
            if ticked.contains(&q.id) {
                out.push(Slot { value: ours(link, q), local: q.id.clone(), qid: None, question: Some(q.clone()) });
            }
            continue;
        };
        let held = changed.contains(&q.id) && !ticked.contains(&q.id);
        let value = if held { base_value(link, &q.id, raw.get(qid).copied())? } else { ours(link, q) };
        let question = if held { question_of(&value).ok() } else { Some(q.clone()) };
        out.push(Slot { value, local: q.id.clone(), qid: Some(qid.to_string()), question });
    }

    // Removals: a pair in `link.qids` whose question this bank no longer has. (The full pairs are kept even when
    // `link.base` was too big to keep, so a large bank still restores what it did not send.) An unticked one goes
    // back as the base has it, immediately after the question before it in the base order (chained removals keep
    // that order; at the head it goes first).
    for p in &link.qids {
        if p.qid.is_empty() || bank.questions.iter().any(|q| q.id == p.local) || ticked.contains(&p.local) {
            continue;
        }
        let value = base_value(link, &p.local, raw.get(p.qid.as_str()).copied())?;
        let question = question_of(&value).ok();
        let at = anchor(&out, &order, &p.qid);
        out.insert(at, Slot { value, local: p.local.clone(), qid: Some(p.qid.clone()), question });
    }

    let mut v = serde_json::to_value(bank).map_err(|e| e.to_string())?;
    if let Some(o) = v.as_object_mut() {
        o.insert("questions".into(), Value::Array(out.iter().map(|s| s.value.clone()).collect()));
    }
    let questions = out.into_iter().map(|s| ComposedQuestion { local: s.local, qid: s.qid, question: s.question }).collect();
    Ok(ComposedPartial { payload: format!("{{\"schema_version\":{SEND_SCHEMA},\"bank\":{v}}}").into_bytes(), questions })
}

/// A base body's raw question as this app's shape (its `published` block is not part of a `Question`). `Err` when
/// this build can not read it (a newer client's `Kind` or `Difficulty`): the caller composes the raw JSON anyway and
/// leaves the snapshot out, never refusing the payload.
fn question_of(value: &Value) -> Result<Question, String> {
    serde_json::from_value(value.clone()).map_err(|_| UNREADABLE.to_string())
}

/// This bank's question with the `published` block a send gives it (none for a question new to the circle).
fn ours(link: &CircleLink, q: &Question) -> Value {
    let mut v = serde_json::to_value(q).unwrap_or(Value::Null);
    if let Some(published) = published_block(link, &q.id)
        && let Some(o) = v.as_object_mut()
    {
        o.insert("published".into(), published);
    }
    v
}

/// The base body's raw question for `qid` (its own `id` and fields), with the `published` block that keeps that
/// identity, so the server hashes it as unchanged.
fn base_value(link: &CircleLink, local: &str, raw: Option<&Value>) -> Result<Value, String> {
    let mut v = raw.ok_or_else(|| MISSING.to_string())?.clone();
    if let Some(published) = published_block(link, local)
        && let Some(o) = v.as_object_mut()
    {
        o.insert("published".into(), published);
    }
    Ok(v)
}

/// Where a restored removal goes: after the nearest question before it in the base order that the output holds
/// (a chained removal that was itself sent is passed over), or at the front when there is none.
fn anchor(out: &[Slot], order: &[&str], qid: &str) -> usize {
    let Some(pos) = order.iter().position(|q| *q == qid) else { return out.len() };
    for before in order[..pos].iter().rev() {
        if let Some(i) = out.iter().position(|s| s.qid.as_deref() == Some(*before)) {
            return i + 1;
        }
    }
    0
}
