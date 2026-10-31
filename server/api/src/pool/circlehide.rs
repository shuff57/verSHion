//! Circles "hide keys from viewers" (worker/src/pool/circlehide.ts): the author's strip decisions, their record with a version, and the
//! stripping of a stored body for a viewer. The Worker's isolate heavy slot and byte reservations have no native counterpart (the
//! process has no 128 MB ceiling), so only the daily cap `circle_hidden_reads` remains of the cost control.
use super::core::{Core, CoreErr, Fail as CoreFail, LabelDecision, Pos, PublishOpts, Stripped, questions_of};
use super::ctx::{Fail, PoolCtx, PoolError, Res};
use crate::blobs::BlobStore;
use serde_json::{Map, Value, json};
use std::collections::HashSet;

/// The author's strip decisions for one version (share-core's options; the publish's meta fields).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StripDecisions {
    pub labels: Vec<LabelDecision>,
    pub order_acks: Vec<Pos>,
    pub image_ok: Vec<usize>,
    pub ack_formatting: Vec<usize>,
}
impl StripDecisions {
    pub fn opts(&self) -> PublishOpts {
        PublishOpts { hide_key: true, labels: self.labels.clone(), order_acks: self.order_acks.clone(), image_ok: self.image_ok.clone(), ack_formatting: self.ack_formatting.clone() }
    }
}
pub const DECISION_FIELDS: [&str; 4] = ["labels", "order_acks", "ack_formatting", "image_ok"];

fn bad(field: &str) -> PoolError {
    PoolError::new(400, "invalid_request").with("field", json!(field))
}
fn nat(v: &Value) -> Option<usize> {
    v.as_u64().filter(|n| *n < 1_000_000).map(|n| n as usize).or_else(|| v.as_f64().filter(|f| f.fract() == 0.0 && (0.0..1_000_000.0).contains(f)).map(|f| f as usize))
}
fn absent(m: &Map<String, Value>, k: &str) -> bool {
    m.get(k).is_none_or(Value::is_null)
}
fn list<'a>(m: &'a Map<String, Value>, k: &str, max: usize) -> Result<&'a [Value], PoolError> {
    if absent(m, k) {
        return Ok(&[]);
    }
    match m[k].as_array() {
        Some(a) if a.len() <= max => Ok(a),
        _ => Err(bad(k)),
    }
}
fn pos_of(o: &Map<String, Value>) -> Option<(usize, Option<usize>)> {
    let q = nat(o.get("question")?)?;
    match o.get("part") {
        None => Some((q, None)),
        Some(p) => Some((q, Some(nat(p)?))),
    }
}

/// The decisions in a request body or a send's meta, checked as a publish's are; 400 on the field.
pub fn read_strip_decisions(m: &Map<String, Value>) -> Result<StripDecisions, PoolError> {
    let mut out = StripDecisions::default();
    for l in list(m, "labels", 60_000)? {
        let o = l.as_object().ok_or_else(|| bad("labels"))?;
        let (question, part) = pos_of(o).ok_or_else(|| bad("labels"))?;
        let choice = match o.get("choice").and_then(Value::as_str) {
            Some(c @ ("strip" | "keep")) => c.to_string(),
            _ => return Err(bad("labels")),
        };
        out.labels.push(LabelDecision { question, part, choice });
    }
    for a in list(m, "order_acks", 60_000)? {
        let o = a.as_object().ok_or_else(|| bad("order_acks"))?;
        let (question, part) = pos_of(o).ok_or_else(|| bad("order_acks"))?;
        out.order_acks.push(Pos { question, part });
    }
    let uniq = |k: &str, max: usize| -> Result<Vec<usize>, PoolError> {
        let mut seen = HashSet::new();
        let mut v = Vec::new();
        for x in list(m, k, max)? {
            let n = nat(x).ok_or_else(|| bad(k))?;
            if seen.insert(n) {
                v.push(n);
            }
        }
        Ok(v)
    };
    out.image_ok = uniq("image_ok", 1000)?;
    out.ack_formatting = uniq("ack_formatting", 60_000)?;
    Ok(out)
}

/// The image hashes a stripped payload keeps (what the image route may serve a viewer).
pub fn kept_images(payload: &Value) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for q in questions_of(payload) {
        let Some(f) = q.get("image").and_then(|i| i.get("file")).and_then(Value::as_str) else { continue };
        let Some((sha, ext)) = f.strip_prefix("media/").and_then(|r| r.split_once('.')) else { continue };
        if super::sqlfrag::is_sha(sha) && matches!(ext, "png" | "jpg" | "gif" | "webp") && !out.iter().any(|s| s == sha) {
            out.push(sha.to_string());
        }
    }
    out
}

/// The decisions a successful strip actually used, as they are recorded with a version (bounded by the bank, never by a request).
pub fn recorded_decisions(d: &StripDecisions, st: &Stripped, questions: usize) -> StripDecisions {
    let key = |q: usize, p: Option<usize>| format!("{q}/{}", p.map(|p| p.to_string()).unwrap_or_default());
    let mut labels: Vec<(String, LabelDecision)> = Vec::new();
    for f in &st.label_findings {
        let (Some(question), Some(choice)) = (f.get("question").and_then(nat), f.get("decision").and_then(Value::as_str).filter(|c| matches!(*c, "strip" | "keep"))) else { continue };
        let part = f.get("part").and_then(nat);
        let (k, v) = (key(question, part), LabelDecision { question, part, choice: choice.to_string() });
        match labels.iter_mut().find(|(x, _)| *x == k) {
            Some(e) => e.1 = v,
            None => labels.push((k, v)),
        }
    }
    let mut acks: Vec<(String, Pos)> = Vec::new();
    for a in &st.needs_order_ack {
        let k = key(a.question, a.part);
        if !acks.iter().any(|(x, _)| *x == k) {
            acks.push((k, a.clone()));
        }
    }
    let mut seen = HashSet::new();
    StripDecisions {
        labels: labels.into_iter().map(|x| x.1).collect(),
        order_acks: acks.into_iter().map(|x| x.1).collect(),
        image_ok: st.images_kept.iter().copied().filter(|x| *x < questions && seen.insert(*x)).collect(),
        ack_formatting: d.ack_formatting.iter().copied().filter(|x| *x < questions).collect(),
    }
}

/// The JSON recorded in circle_versions.strip_opts.
pub fn decisions_json(d: &StripDecisions, image_shas: &[String]) -> String {
    json!({
        "labels": d.labels,
        "order_acks": d.order_acks,
        "ack_formatting": d.ack_formatting,
        "image_ok": d.image_ok,
        "image_shas": image_shas,
    })
    .to_string()
}
/// The decisions a version recorded (none when it recorded none, or the record does not read).
pub fn decisions_of(json: Option<&str>) -> StripDecisions {
    let Some(j) = json.filter(|j| !j.is_empty()) else { return StripDecisions::default() };
    match serde_json::from_str::<Value>(j) {
        Ok(Value::Object(m)) => read_strip_decisions(&m).unwrap_or_default(),
        _ => StripDecisions::default(),
    }
}

/// Positions whose stripped key-free hash differs from the full form's (a label stripped): [full, stripped] pairs.
pub fn hash_pairs(full: &[String], stripped: &[String]) -> Vec<(String, String)> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for (k, f) in full.iter().enumerate() {
        let Some(s) = stripped.get(k).filter(|s| !s.is_empty() && *s != f) else { continue };
        if seen.insert((f.clone(), s.clone())) {
            out.push((f.clone(), s.clone()));
        }
    }
    out
}

/// SQL (one statement): the stripped-form hashes `pairs` ([[full, stripped], ...]) recorded in circle_qkeys under the FIRST sender of the
/// full text each comes from, so the true first sender keeps passing with her own words and nobody else does. `cid`, `bid`, `pairs` are
/// parameter references.
pub fn stripped_qkeys_sql(cid: &str, bid: &str, pairs: &str, cond: &str) -> String {
    format!(
        "INSERT OR IGNORE INTO circle_qkeys (circle_id, match_hash, bank_id, first_version, first_seq, first_by)
   SELECT {cid}, json_extract(j.value, '$[1]'), {bid}, k.first_version, k.first_seq, k.first_by
     FROM json_each({pairs}) j CROSS JOIN circle_qkeys k ON k.circle_id = {cid} AND k.match_hash = json_extract(j.value, '$[0]') AND k.bank_id = {bid}
    WHERE {cond}"
    )
}

/// The same decisions, whatever their order (a PATCH that would record what is recorded is a no-op).
pub fn same_decisions(a: &StripDecisions, b: &StripDecisions) -> bool {
    let k = |d: &StripDecisions| {
        let mut l: Vec<String> = d.labels.iter().map(|x| format!("{}/{}/{}", x.question, x.part.map(|p| p.to_string()).unwrap_or_default(), x.choice)).collect();
        let mut o: Vec<String> = d.order_acks.iter().map(|x| format!("{}/{}", x.question, x.part.map(|p| p.to_string()).unwrap_or_default())).collect();
        let (mut i, mut f) = (d.image_ok.clone(), d.ack_formatting.clone());
        l.sort();
        o.sort();
        i.sort();
        i.dedup();
        f.sort();
        f.dedup();
        (l, o, i, f)
    };
    k(a) == k(b)
}

/// What a strip of a stored body leaves the caller.
#[derive(Debug)]
pub enum StoredStrip {
    Ok {
        bytes: Vec<u8>,
        /// The served form's key-free match hashes.
        match_hashes: Vec<String>,
        /// The full form's (only when asked).
        full_hashes: Option<Vec<String>>,
        /// The images the viewers' copy keeps.
        image_shas: Vec<String>,
        /// The decisions to record.
        recorded: StripDecisions,
        report: Value,
        stripped: Box<Stripped>,
    },
    Refused { fail: Box<CoreFail>, missing: bool },
}

fn server_error() -> Fail {
    PoolError::new(500, "server_error").into()
}
fn core_fail(e: CoreErr) -> Result<CoreFail, Fail> {
    match e {
        CoreErr::Fail(f) => Ok(*f),
        _ => Err(server_error()),
    }
}

/// Strips a stored body with `d` and (when asked) computes the full form's key-free match hashes. Only the stripped bytes and small
/// facts leave; the author-only served orders are dropped here.
pub fn strip_stored(core: &Core, store: &BlobStore, key: &str, d: &StripDecisions, with_full_hashes: bool) -> Res<StoredStrip> {
    let Some(bytes) = store.get(key).map_err(|_| server_error())? else {
        return Ok(StoredStrip::Refused { fail: Box::new(CoreFail { errors: vec![super::core::CoreError { code: "body_missing".into(), path: String::new(), ..Default::default() }], ..Default::default() }), missing: true });
    };
    let st = match core.strip(&bytes, &d.opts()) {
        Ok(s) => s,
        Err(e) => return Ok(StoredStrip::Refused { fail: Box::new(core_fail(e)?), missing: false }),
    };
    let full_hashes = if with_full_hashes {
        match core.match_hashes(&bytes) {
            Ok(h) => Some(h),
            Err(e) => return Ok(StoredStrip::Refused { fail: Box::new(core_fail(e)?), missing: false }),
        }
    } else {
        None
    };
    let recorded = recorded_decisions(d, &st, questions_of(&st.v.payload).len());
    let report = json!({
        "labels_removed": st.labels_removed,
        "images_kept": st.images_kept,
        "images_removed": st.images_removed,
        "figures_removed": st.figures_removed,
        "order_reasons": st.order_reasons,
    });
    Ok(StoredStrip::Ok { bytes: st.v.bytes.clone(), match_hashes: st.match_hashes.clone(), full_hashes, image_shas: kept_images(&st.v.payload), recorded, report, stripped: Box::new(st) })
}

/// A viewer's read of a version of a bank that hides its keys: the stripped bytes, or `409 keys_hidden_unavailable` (no text, no
/// findings). Charged to circle_hidden_reads, and given back for every read that serves nothing (409, 404, an error).
pub fn stripped_for_viewer(x: &PoolCtx, core: &Core, store: &BlobStore, me: &str, full_key: &str, strip_opts: Option<&str>) -> Res<Vec<u8>> {
    if let Err(e) = x.require_daily(me, "circle_hidden_reads", 1) {
        return Err(match e {
            Fail::Pool(mut p) if p.code == "daily_limit" => {
                p.extra.insert("message".into(), json!("You have opened as many banks with hidden answer keys as you may today. Try again tomorrow."));
                Fail::Pool(p)
            }
            o => o,
        });
    }
    let r = match strip_stored(core, store, full_key, &decisions_of(strip_opts), false) {
        Ok(r) => r,
        Err(e) => {
            let _ = x.give_daily(me, "circle_hidden_reads", 1);
            return Err(e);
        }
    };
    match r {
        StoredStrip::Ok { bytes, .. } => Ok(bytes),
        StoredStrip::Refused { missing, .. } => {
            let _ = x.give_daily(me, "circle_hidden_reads", 1);
            Err(if missing { super::ctx::not_found().into() } else { super::circlebanks::keys_hidden_unavailable().into() })
        }
    }
}

/// A stored body's FULL form (its questions and key-free match hashes) handed to `use_it`; None when the body is gone or share-core
/// refuses it.
pub fn with_stored_full<T>(core: &Core, store: &BlobStore, key: &str, use_it: impl FnOnce(&[Value], &[String]) -> T) -> Res<Option<T>> {
    let Some(bytes) = store.get(key).map_err(|_| server_error())? else { return Ok(None) };
    let mh = match core.match_hashes(&bytes) {
        Ok(h) => h,
        Err(CoreErr::Fail(_)) => return Ok(None),
        Err(_) => return Err(server_error()),
    };
    let Ok(payload) = serde_json::from_slice::<Value>(&bytes) else { return Ok(None) };
    let qs: Vec<Value> = questions_of(&payload).into_iter().cloned().collect();
    Ok(Some(use_it(&qs, &mh)))
}

#[cfg(test)]
#[path = "circlehide_tests.rs"]
mod tests;
