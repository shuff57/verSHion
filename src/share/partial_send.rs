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
use crate::model::{Question, QuestionBank};

/// The base body could not be read, so which questions changed can not be told.
const UNREADABLE: &str = "The version this copy is based on could not be read.";
/// A change has no question in the base body: there is nothing to send or to put back.
const MISSING: &str = "The version this copy is based on does not have one of the questions this copy changed or removed.";

/// One question of the composed bank, and the circle id it keeps (none: new to the circle).
struct Slot {
    value: Value,
    qid: Option<String>,
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
    if bank.questions.is_empty() {
        return Err("The bank has no questions. A circle bank needs at least one.".into());
    }
    let base: Value = serde_json::from_slice(base_body).map_err(|_| UNREADABLE.to_string())?;
    let base_qs = base.get("bank").and_then(|b| b.get("questions")).and_then(Value::as_array).ok_or_else(|| UNREADABLE.to_string())?;
    // Each base question's raw JSON, by its circle id, and the base order (for putting a removal back).
    let mut raw: BTreeMap<&str, &Value> = BTreeMap::new();
    let mut order: Vec<&str> = Vec::new();
    for q in base_qs {
        let Some(qid) = q.get("id").and_then(Value::as_str).filter(|id| !id.is_empty()) else { continue };
        order.push(qid);
        raw.entry(qid).or_insert(q);
    }

    // This bank in order. A changed question that is unticked is replaced by the base's raw question; an added
    // question that is unticked is left out.
    let mut out: Vec<Slot> = Vec::new();
    for q in &bank.questions {
        let Some(qid) = link.qid_of(&q.id).filter(|id| !id.is_empty()) else {
            if ticked.contains(&q.id) {
                out.push(Slot { qid: None, value: ours(link, q) });
            }
            continue;
        };
        let held = changed.contains(&q.id) && !ticked.contains(&q.id);
        let value = if held { base_value(link, &q.id, raw.get(qid).copied())? } else { ours(link, q) };
        out.push(Slot { qid: Some(qid.to_string()), value });
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
        let at = anchor(&out, &order, &p.qid);
        out.insert(at, Slot { qid: Some(p.qid.clone()), value });
    }

    let mut v = serde_json::to_value(bank).map_err(|e| e.to_string())?;
    if let Some(o) = v.as_object_mut() {
        o.insert("questions".into(), Value::Array(out.into_iter().map(|s| s.value).collect()));
    }
    Ok(format!("{{\"schema_version\":{SEND_SCHEMA},\"bank\":{v}}}").into_bytes())
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
