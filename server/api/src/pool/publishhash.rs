//! The one share-core call of a publish and everything derived from it (worker/src/pool/publish.ts `publishOnce`, "the one share-core call" to the keyed
//! sketches): validation, the stripped form, the match and content hashes, the key-free facts a non-author can see, the licence-sense hashes and the stem
//! sketches. Nothing here reads the database but the reuse of an unchanged hidden form.
use super::core::{Core, CoreErr, Prepared, PublishOpts, Stripped, questions_of};
use super::ctx::{Fail, PoolCtx, PoolError, Res};
use super::fingerprint::Fingerprinter;
use super::publishearlier::{Reused, reuse_served};
use super::publishmeta::PublishMeta;
use super::publishrewrite::{Rewritten, core_fail};
use super::publishstart::Landing;
use super::qhash::{body_tokens, keyfree_hash, near_hash, near_kf_hash, stem_bands, stem_hash, stem_tokens};
use super::qjson::Json;
use super::textutil::search_key;
use super::visible::{Subject, subject_for};
use crate::blobs::BlobStore;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

/// Distinct search words of served stems, at most this many per publish.
pub const SEARCH_WORDS_MAX: usize = 2000;

fn server_error() -> Fail {
    PoolError::new(500, "server_error").into()
}
fn to_json(v: &Value) -> Json {
    Json::parse(v.to_string().as_bytes()).unwrap_or(Json::Null)
}
fn img_of(q: &Json) -> Option<String> {
    super::circlesendmeta::images_at(std::slice::from_ref(q)).into_iter().next().flatten()
}

/// Distinct search words of served stems (folded, split on punctuation), words of 1 to 40 UTF-16 units, at most `SEARCH_WORDS_MAX`.
pub fn served_words(qs: &[Value]) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<String> = vec![];
    for q in qs {
        let Some(stem) = q.get("stem").and_then(Value::as_str) else { continue };
        for w in search_key(stem).split(' ') {
            let len = w.encode_utf16().count();
            if len == 0 || len > 40 {
                continue;
            }
            if seen.insert(w.to_string()) {
                out.push(w.to_string());
                if out.len() >= SEARCH_WORDS_MAX {
                    return out;
                }
            }
        }
    }
    out
}

/// Everything a publish needs of its questions once share-core has accepted them. All vectors are per question, in order.
#[derive(Debug)]
pub struct Hashed {
    pub prep: Prepared,
    pub reused: Option<Reused>,
    pub public_version: i64,
    /// None: this commit's time.
    pub public_at: Option<i64>,
    /// The questions as served to non-authors (the stripped form when the key is hidden).
    pub served: Vec<Value>,
    pub full_qs: Vec<Value>,
    pub match_hashes: Vec<String>,
    pub full_hashes: Vec<String>,
    pub full_content: Vec<String>,
    pub stripped_content: Option<Vec<String>>,
    pub keyfree: Vec<String>,
    pub near_full: Vec<String>,
    pub near_stripped: Option<Vec<Option<String>>>,
    pub bare_full: Vec<String>,
    pub bare_stripped: Option<Vec<Option<String>>>,
    pub no_expl: Vec<bool>,
    pub near_kf: Vec<String>,
    pub subjects: Vec<Subject>,
    pub stems: Vec<Option<String>>,
    pub tokens: Vec<Vec<String>>,
    pub bands: Vec<Vec<String>>,
    pub search_words: Vec<String>,
    pub body_tokens: Vec<Vec<String>>,
    pub tok_sk: Vec<Vec<String>>,
    pub body_sk: Vec<Vec<String>>,
    pub k_bands: Vec<Vec<String>>,
}

/// Runs share-core once on the rewritten payload and derives every hash. A hidden republish whose served form is unchanged keeps the previous stripped body
/// and the public version, so nothing a non-author sees reveals that something hidden changed.
#[allow(clippy::too_many_arguments)]
pub fn prepare(x: &PoolCtx, store: &BlobStore, core: &Core, fp: &Fingerprinter, land: &Landing, is_new: bool, m: &PublishMeta, rw: &Rewritten) -> Res<Hashed> {
    let n = rw.n;
    let opts = PublishOpts { hide_key: m.hide_key, labels: m.labels.clone(), order_acks: m.order_acks.clone(), image_ok: m.image_ok.clone(), ack_formatting: m.ack_formatting.clone() };
    let prep = core.publish_prepare(&rw.bytes, &opts).map_err(core_fail)?;
    let kind = if prep.full.payload.get("bank").is_some() { "bank" } else { "question" };
    let full_qs: Vec<Value> = questions_of(&prep.full.payload).into_iter().cloned().collect();
    if kind != rw.kind || full_qs.len() != n {
        return Err(PoolError::new(422, "invalid_payload").with("errors", json!([{ "code": "invalid", "path": "" }])).into());
    }
    let st: Option<&Stripped> = prep.stripped.as_ref();
    let reused = match (st, land.prev_hidden, land.base_of.as_deref()) {
        (Some(st), true, Some(base)) => reuse_served(x.conn, store, base, &rw.qids, st, &m.order_acks)?,
        _ => None,
    };
    let public_version = if is_new { 1 } else if reused.is_some() { land.prev_public_version } else { land.prev_public_version + 1 };
    let public_at = reused.as_ref().and(land.prev_public_at);
    let match_hashes = st.map_or_else(|| prep.match_hashes.clone(), |s| s.match_hashes.clone());
    let full_hashes = prep.full_hashes.clone();
    let full_content = core.content_hashes(&full_qs).map_err(|_| server_error())?;
    let stripped_qs: Option<Vec<Value>> = st.map(|s| questions_of(&s.v.payload).into_iter().cloned().collect());
    let stripped_content = match (&reused, &stripped_qs) {
        (Some(r), _) => Some(r.contents.clone()),
        (None, Some(q)) => Some(core.content_hashes(q).map_err(|_| server_error())?),
        _ => None,
    };
    if match_hashes.len() != n || full_hashes.len() != n || full_content.len() != n {
        return Err(server_error());
    }
    let served: Vec<Value> = stripped_qs.clone().unwrap_or_else(|| full_qs.clone());
    let served_j: Vec<Json> = served.iter().map(to_json).collect();
    let full_j: Vec<Json> = full_qs.iter().map(to_json).collect();
    let keyfree: Vec<String> = served_j.iter().enumerate().map(|(i, q)| keyfree_hash(fp, &match_hashes[i], q, &img_of)).collect();
    let near_full: Vec<String> = full_j.iter().map(|q| near_hash(fp, q, false)).collect();
    let bare_full: Vec<String> = full_j.iter().map(|q| near_hash(fp, q, true)).collect();
    let near_stripped = match (&reused, st) {
        (Some(r), _) => Some(r.near.clone()),
        (None, Some(_)) => Some(served_j.iter().map(|q| Some(near_hash(fp, q, false))).collect()),
        _ => None,
    };
    let bare_stripped = match (&reused, st) {
        (Some(r), _) => Some(r.bare.clone()),
        (None, Some(_)) => Some(served_j.iter().map(|q| Some(near_hash(fp, q, true))).collect()),
        _ => None,
    };
    let no_expl: Vec<bool> = full_qs.iter().map(|q| q.get("explanation").and_then(Value::as_str).is_none_or(|e| e.trim().is_empty())).collect();
    let near_kf: Vec<String> = served_j.iter().enumerate().map(|(i, q)| near_kf_hash(fp, &match_hashes[i], q, &img_of)).collect();
    let subjects: Vec<Subject> = (0..n).map(|i| subject_for(m.hide_key, &keyfree[i], &full_content[i], &near_full[i], &near_kf[i], Some(&bare_full[i]), no_expl[i])).collect();

    let stem_of = |q: &Json| q.get("stem").cloned().unwrap_or(Json::Null);
    let stems: Vec<Option<String>> = served_j.iter().map(|q| stem_hash(fp, &stem_of(q))).collect();
    let tokens: Vec<Vec<String>> = served_j.iter().map(|q| stem_tokens(&stem_of(q))).collect();
    let search_words = served_words(&served);
    let bands: Vec<Vec<String>> = tokens.iter().map(|t| stem_bands(t)).collect();
    let body_toks: Vec<Vec<String>> = served_j.iter().map(body_tokens).collect();
    let mut memo: HashMap<String, String> = HashMap::new();
    let tok_sk: Vec<Vec<String>> = tokens.iter().map(|t| fp.keyed_set(t, &mut memo, "t:")).collect();
    let body_sk: Vec<Vec<String>> = body_toks.iter().map(|b| fp.keyed_body(b, &mut memo)).collect();
    let k_bands: Vec<Vec<String>> = bands.iter().map(|b| fp.keyed_bands(b, &mut memo)).collect();

    Ok(Hashed {
        prep,
        reused,
        public_version,
        public_at,
        served,
        full_qs,
        match_hashes,
        full_hashes,
        full_content,
        stripped_content,
        keyfree,
        near_full,
        near_stripped,
        bare_full,
        bare_stripped,
        no_expl,
        near_kf,
        subjects,
        stems,
        tokens,
        bands,
        search_words,
        body_tokens: body_toks,
        tok_sk,
        body_sk,
        k_bands,
    })
}

#[allow(dead_code)]
fn _keep(_: CoreErr) {}

#[cfg(test)]
#[path = "publishhash_tests.rs"]
mod tests;
