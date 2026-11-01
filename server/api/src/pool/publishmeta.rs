//! The meta part of a publish request (worker/src/pool/publish.ts `readMeta`): every field checked, nothing preselected. A refusal names the field.
use super::core::{LabelDecision, Pos};
use super::ctx::PoolError;
use super::license::{ATTEST_TEXT_REVS, is_license};
use super::sqlfrag::{is_id, is_publish_id};
use crate::auth::validate::clean_text;
use serde_json::{Map, Value};
use std::collections::HashSet;

/// What the author attested about the publish's independence from items they only looked at.
#[derive(Debug, Clone, PartialEq)]
pub struct Attest {
    pub own_work: bool,
    pub terms_version: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PublishMeta {
    pub publish_id: String,
    pub base: Option<String>,
    pub hide_key: bool,
    pub license: String,
    pub attest_rev: String,
    /// The publishing-terms affirmation (checked at publish, recorded; None when absent or malformed).
    pub attest: Option<Attest>,
    pub labels: Vec<LabelDecision>,
    pub order_acks: Vec<Pos>,
    pub image_ok: Vec<usize>,
    /// Questions whose low-risk formatting findings the author acknowledged.
    pub ack_formatting: Vec<usize>,
    /// "public" | "unlisted"; None on a republish.
    pub visibility: Option<String>,
    pub meta: Value,
    /// Items whose plain reads the author attests had no part in this publish.
    pub independent: Vec<String>,
    /// The author's own words for an independence claim (stored in the audit and the admin flag).
    pub statement: Option<String>,
}

fn bad(field: &str) -> PoolError {
    PoolError::new(400, "invalid_request").with("field", field.into())
}

/// A natural number below a million (`Number.isInteger(x) && x >= 0 && x < 1_000_000`).
fn nat(v: &Value) -> Option<usize> {
    let n = v.as_f64()?;
    (n.fract() == 0.0 && (0.0..1_000_000.0).contains(&n)).then_some(n as usize)
}

fn pos_of(a: &Value) -> Option<(usize, Option<usize>)> {
    let o = a.as_object()?;
    let question = nat(o.get("question")?)?;
    let part = match o.get("part") {
        None => None,
        Some(p) => Some(nat(p)?),
    };
    Some((question, part))
}

pub fn read_meta(m: &Map<String, Value>, is_new: bool) -> Result<PublishMeta, PoolError> {
    let publish_id = match m.get("publish_id").and_then(Value::as_str) {
        Some(s) if is_publish_id(s) => s.to_string(),
        _ => return Err(bad("publish_id")),
    };
    let base = if is_new {
        if m.get("base_publish_id").is_some_and(|v| !v.is_null()) {
            return Err(bad("base_publish_id"));
        }
        None
    } else {
        match m.get("base_publish_id").and_then(Value::as_str) {
            Some(s) if is_publish_id(s) => Some(s.to_string()),
            _ => return Err(bad("base_publish_id")),
        }
    };
    let Some(hide_key) = m.get("hide_key").and_then(Value::as_bool) else { return Err(bad("hide_key")) };
    // Nothing is preselected: the licence must be chosen explicitly on every publish.
    let license = match m.get("license").and_then(Value::as_str) {
        Some(l) if is_license(l) => l.to_string(),
        _ => return Err(bad("license")),
    };
    let at = m.get("attestation").and_then(Value::as_object);
    let attest_rev = match at {
        Some(a) if a.get("accepted") == Some(&Value::Bool(true)) => match a.get("text_rev").and_then(Value::as_str) {
            Some(r) if ATTEST_TEXT_REVS.contains(&r) => r.to_string(),
            _ => return Err(bad("attestation")),
        },
        _ => return Err(bad("attestation")),
    };
    let list = |key: &str, max: usize| -> Result<Vec<Value>, PoolError> {
        match m.get(key) {
            None | Some(Value::Null) => Ok(vec![]),
            Some(Value::Array(a)) if a.len() <= max => Ok(a.clone()),
            _ => Err(bad(key)),
        }
    };
    let mut labels = vec![];
    for l in list("labels", 60_000)? {
        let o = l.as_object().ok_or_else(|| bad("labels"))?;
        let choice = o.get("choice").and_then(Value::as_str).filter(|c| *c == "strip" || *c == "keep").ok_or_else(|| bad("labels"))?;
        let (question, part) = pos_of(&l).ok_or_else(|| bad("labels"))?;
        labels.push(LabelDecision { question, part, choice: choice.into() });
    }
    let mut order_acks = vec![];
    for a in list("order_acks", 60_000)? {
        let (question, part) = pos_of(&a).ok_or_else(|| bad("order_acks"))?;
        order_acks.push(Pos { question, part });
    }
    let image_ok: Vec<usize> = list("image_ok", 1000)?.iter().map(nat).collect::<Option<_>>().ok_or_else(|| bad("image_ok"))?;
    let ack_raw: Vec<usize> = list("ack_formatting", 60_000)?.iter().map(nat).collect::<Option<_>>().ok_or_else(|| bad("ack_formatting"))?;
    let mut seen = HashSet::new();
    let ack_formatting: Vec<usize> = ack_raw.into_iter().filter(|x| seen.insert(*x)).collect();
    let mut visibility = None;
    if is_new {
        let v = match m.get("visibility").and_then(Value::as_str) {
            Some(v @ ("public" | "unlisted")) => v.to_string(),
            _ => return Err(bad("visibility")),
        };
        if v == "public" && license == "circle-only" {
            return Err(PoolError::new(422, "license_not_public"));
        }
        visibility = Some(v);
    }
    // "I wrote these independently of items I only looked at": voids those READS (never imports) as evidence for this publish. Attested, audited and flagged
    // for the admin.
    let mut independent: Vec<String> = vec![];
    let mut statement = None;
    if let Some(ind) = m.get("independent_of").filter(|v| !v.is_null()) {
        let obj = ind.as_object();
        let items = obj.and_then(|o| o.get("items")).and_then(Value::as_array);
        let ok = obj.is_some_and(|o| o.get("accepted") == Some(&Value::Bool(true)))
            && items.is_some_and(|i| !i.is_empty() && i.len() <= 50 && i.iter().all(|x| x.as_str().is_some_and(is_id)));
        if !ok {
            return Err(bad("independent_of"));
        }
        let mut seen = HashSet::new();
        independent = items.unwrap().iter().filter_map(Value::as_str).filter(|s| seen.insert(*s)).map(String::from).collect();
        let st = obj.and_then(|o| o.get("statement")).and_then(|s| clean_text(s, 1000));
        match st {
            Some(s) if s.chars().count() >= 10 => statement = Some(s),
            _ => return Err(bad("independent_of.statement")),
        }
    }
    let attest = m.get("attest").and_then(Value::as_object).and_then(|a| {
        let tv = a.get("terms_version").and_then(Value::as_str)?;
        Some(Attest { own_work: a.get("own_work") == Some(&Value::Bool(true)), terms_version: tv.chars().take(40).collect() })
    });
    Ok(PublishMeta {
        publish_id,
        base,
        hide_key,
        license,
        attest_rev,
        attest,
        labels,
        order_acks,
        image_ok,
        ack_formatting,
        visibility,
        meta: m.get("meta").filter(|v| !v.is_null()).cloned().unwrap_or_else(|| Value::Object(Map::new())),
        independent,
        statement,
    })
}

#[cfg(test)]
#[path = "publishmeta_tests.rs"]
mod tests;
