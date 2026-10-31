//! Sending a version of a circle bank, part 2 (worker/src/pool/circlesend.ts): the send's meta, idempotency by commit_id, the per-question
//! index and its diff, the keyless/large-change refusals, and the student-data and copyright scan of a version. The commit batch follows.
use super::circlesendsql::EVIDENCE_HASHES;
use super::core::Fail as CoreFail;
use super::ctx::{PoolCtx, PoolError, Res};
use super::fingerprint::Fingerprinter;
use super::license::{ATTEST_STATEMENT, POOL_TERMS_VERSION, is_license};
use super::privacynear::{StemSketch, privacy_near_positions};
use super::qhash::{field_keys, near_hash, norm_skeleton_key, part_keys_of, stem_bands, stem_key, stem_tokens};
use super::qjson::Json;
use super::sqlfrag::is_publish_id;
use crate::auth::validate::clean_text;
use crate::blobs::BlobStore;
use rusqlite::{OptionalExtension, params};
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};

pub const MESSAGE_MAX: usize = 500;
/// An attempt still in flight this long is not abandoned.
pub const IN_FLIGHT_MS: i64 = 60_000;
pub const PRIVACY_BAND_ROWS_MAX: usize = 2000;

#[derive(Debug, Clone)]
pub struct SendMeta {
    pub commit_id: String,
    pub license: String,
    pub message: Option<String>,
    pub base: Option<i64>,
    pub meta: Value,
    pub merge_token: Option<String>,
    pub merged_from: Option<i64>,
    pub confirm: bool,
    /// A new bank that hides its keys from viewers from its first version (POST only).
    pub hide_key: bool,
}

fn bad(field: &str) -> PoolError {
    PoolError::new(400, "invalid_request").with("field", field.into())
}
fn int_at_least_1(v: &Value) -> Option<i64> {
    v.as_i64().filter(|n| (1..=(1i64 << 31)).contains(n)).or_else(|| v.as_f64().filter(|f| f.fract() == 0.0 && (1.0..=2147483648.0).contains(f)).map(|f| f as i64))
}
fn present(m: &Map<String, Value>, k: &str) -> bool {
    m.contains_key(k)
}
fn some(m: &Map<String, Value>, k: &str) -> bool {
    m.get(k).is_some_and(|v| !v.is_null())
}

fn message_of(v: Option<&Value>) -> Result<Option<String>, PoolError> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.is_empty() => Ok(None),
        Some(v) => clean_text(v, MESSAGE_MAX).map(Some).ok_or_else(|| bad("message")),
    }
}

/// Reads and validates the meta line of a send (`is_new`: POST, the first version of a new bank).
pub fn read_send_meta(m: &Map<String, Value>, is_new: bool) -> Result<SendMeta, PoolError> {
    let commit_id = match m.get("commit_id").and_then(Value::as_str) {
        Some(c) if is_publish_id(c) => c.to_string(),
        _ => return Err(bad("commit_id")),
    };
    // Chosen explicitly on every send, like a public publish's.
    let license = match m.get("license").and_then(Value::as_str) {
        Some(l) if is_license(l) => l.to_string(),
        _ => return Err(bad("license")),
    };
    // The own-work attestation is ticked on every send. Recorded, not verified.
    let attested = m.get("attest").and_then(Value::as_object).is_some_and(|a| a.get("own_work") == Some(&Value::Bool(true)) && a.get("terms_version").and_then(Value::as_str) == Some(POOL_TERMS_VERSION));
    if !attested {
        return Err(PoolError::new(422, "attestation_required").with("terms_version", POOL_TERMS_VERSION.into()).with("statement", ATTEST_STATEMENT.into()));
    }
    let message = message_of(m.get("message"))?;
    let mut base = None;
    if is_new {
        if some(m, "base_version") {
            return Err(bad("base_version"));
        }
        if present(m, "merge_token") || present(m, "merged_from") {
            return Err(bad("merge_token"));
        }
    } else {
        base = Some(m.get("base_version").and_then(int_at_least_1).ok_or_else(|| bad("base_version"))?);
    }
    if some(m, "merge_token") && !m["merge_token"].as_str().is_some_and(|t| t.encode_utf16().count() <= 500) {
        return Err(bad("merge_token"));
    }
    let merge_token = m.get("merge_token").and_then(Value::as_str).map(String::from);
    // merged_from is what the token records; given alone it is refused, given with the token it must agree.
    if some(m, "merged_from") && (m.get("merged_from").and_then(int_at_least_1).is_none() || merge_token.is_none()) {
        return Err(bad("merged_from"));
    }
    if present(m, "confirm_large_change") && !m["confirm_large_change"].is_boolean() {
        return Err(bad("confirm_large_change"));
    }
    // Only a new bank's first version may say it hides its keys; later, PATCH changes the setting.
    if present(m, "hide_key") && (!is_new || !m["hide_key"].is_boolean()) {
        return Err(bad("hide_key"));
    }
    Ok(SendMeta {
        commit_id,
        license,
        message,
        base,
        meta: m.get("meta").filter(|v| !v.is_null()).cloned().unwrap_or_else(|| json!({})),
        merge_token,
        merged_from: m.get("merged_from").and_then(Value::as_i64),
        confirm: m.get("confirm_large_change") == Some(&Value::Bool(true)),
        hide_key: m.get("hide_key") == Some(&Value::Bool(true)),
    })
}

/// share-core's refusal as `422 invalid_payload {errors, formatting_findings?}`.
pub fn invalid(f: &CoreFail) -> PoolError {
    let mut e = PoolError::new(422, "invalid_payload").with("errors", serde_json::to_value(&f.errors).unwrap_or(Value::Null));
    if let Some(ff) = &f.formatting_findings {
        e = e.with("formatting_findings", Value::Array(ff.clone())).with("formatting_findings_total", json!(f.formatting_findings_total.unwrap_or(ff.len() as f64)));
    }
    e
}

pub fn large_error(removed: i64, total: i64) -> PoolError {
    PoolError::new(428, "confirm_large_required").with("removed", removed.into()).with("total", total.into()).with(
        "message",
        format!("This version removes {removed} of the bank's {total} questions. Send it again with confirm_large_change if that is what you meant.").into(),
    )
}
/// More than half of the head's questions dropped (exactly half is not).
pub fn is_large(removed: i64, total: i64) -> bool {
    total > 0 && removed * 2 > total
}

/// The questions share-core's publish check (without hide_key) refuses as having no real key: a keyless question
/// (`keyless_needs_hide_key`) or a stripped copy's placeholder True/False (`placeholder_key`), by position.
pub fn keyless_positions(f: &CoreFail) -> Vec<i64> {
    let mut out: Vec<i64> = Vec::new();
    for e in &f.errors {
        if e.code != "keyless_needs_hide_key" && e.code != "placeholder_key" {
            continue;
        }
        let pos = e.path.strip_prefix("bank.questions[").and_then(|r| r.split_once(']')).and_then(|(n, _)| n.parse::<i64>().ok()).or_else(|| {
            let p = &e.path;
            let word = p.strip_prefix("question").is_some_and(|r| !r.starts_with(|c: char| c.is_alphanumeric() || c == '_'));
            word.then_some(0)
        });
        if let Some(p) = pos {
            if !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// `422 keyless_question {positions, kinds, message}`: the words fit the questions named.
pub fn keyless_question(positions: &[i64], qs: &[Value]) -> PoolError {
    let named: Vec<i64> = positions.iter().copied().take(100).collect();
    let kinds: Vec<String> = named
        .iter()
        .map(|i| {
            qs.get(*i as usize)
                .and_then(|q| q.get("kind"))
                .and_then(Value::as_object)
                .and_then(|k| k.keys().next().cloned())
                .unwrap_or_default()
        })
        .collect();
    let short = kinds.iter().filter(|k| *k == "Short").count();
    let fix = if short == kinds.len() {
        "type a sample answer for each of them"
    } else if short == 0 {
        "mark the correct answer of each of them"
    } else {
        "give each of them its answer: mark the correct one, or type a sample answer"
    };
    PoolError::new(422, "keyless_question").with("positions", json!(named)).with("kinds", json!(kinds)).with(
        "message",
        format!("These questions were copied without their answer keys (a viewer's copy of a bank that hides its keys), so they cannot be sent: the circle keeps the full questions with their keys and explanations. Now that you can edit this bank, get the full bank again and make your changes there, or {fix}.").into(),
    )
}

/// Idempotency by commit_id: the same id, account, circle, bank and request replays the first answer; another request under it is
/// `409 commit_id_conflict`; an attempt still in flight is `409 publish_in_progress`; an abandoned one is discarded and the request runs again.
pub fn replay_of(x: &PoolCtx, store: &BlobStore, me: &str, cid: &str, bid: Option<&str>, commit_id: &str, request_sha: &str) -> Res<Option<Value>> {
    struct P {
        id: String,
        bank_id: String,
        circle_id: String,
        committer_id: String,
        request_sha256: String,
        result_json: Option<String>,
        status: String,
        full_key: String,
        index_key: String,
        created_at: i64,
    }
    let prior = x
        .conn
        .query_row(
            "SELECT id, bank_id, circle_id, committer_id, request_sha256, result_json, status, full_key, index_key, created_at FROM circle_versions WHERE commit_id = ?1",
            [commit_id],
            |r| {
                Ok(P { id: r.get(0)?, bank_id: r.get(1)?, circle_id: r.get(2)?, committer_id: r.get(3)?, request_sha256: r.get(4)?, result_json: r.get(5)?, status: r.get(6)?, full_key: r.get(7)?, index_key: r.get(8)?, created_at: r.get(9)? })
            },
        )
        .optional()?;
    let Some(prior) = prior else { return Ok(None) };
    if prior.committer_id != me || prior.request_sha256 != request_sha || prior.circle_id != cid || bid.is_some_and(|b| prior.bank_id != b) {
        return Err(PoolError::new(409, "commit_id_conflict").into());
    }
    if prior.status != "staging" {
        if let Some(j) = &prior.result_json {
            let mut v: Value = serde_json::from_str(j).unwrap_or_else(|_| json!({}));
            if let Some(o) = v.as_object_mut() {
                o.insert("replayed".into(), true.into());
            }
            return Ok(Some(v));
        }
    }
    let busy = || PoolError::new(409, "publish_in_progress");
    if prior.created_at > x.now - IN_FLIGHT_MS {
        return Err(busy().into());
    }
    // An abandoned attempt: start again with the same commit_id; its stored keys are recorded as orphans in the same step.
    let keys: Vec<String> = [prior.full_key, prior.index_key].into_iter().filter(|k| !k.is_empty()).collect();
    let tx = x.conn.unchecked_transaction()?;
    let del = tx.execute("DELETE FROM circle_versions WHERE id = ?1 AND status = 'staging' AND created_at = ?2", params![prior.id, prior.created_at])?;
    if del == 1 {
        tx.execute("INSERT OR IGNORE INTO r2_orphans (key, at) SELECT value, ?2 FROM json_each(?1)", params![serde_json::to_string(&keys).unwrap(), x.now])?;
    }
    tx.commit()?;
    if del != 1 {
        return Err(busy().into());
    }
    super::circlepurge::delete_keys(x, store, &keys)?;
    Ok(None)
}

/// A version's per-question index: qids, content hashes and the key-free match hashes.
#[derive(Debug, Clone, PartialEq)]
pub struct Index {
    pub qids: Vec<String>,
    pub h: Vec<String>,
    pub m: Option<Vec<String>>,
}
pub fn read_index(store: &BlobStore, key: &str) -> Option<Index> {
    if key.is_empty() {
        return None;
    }
    let bytes = store.get(key).ok().flatten()?;
    let x: Value = serde_json::from_slice(&bytes).ok()?;
    let strs = |v: &Value| v.as_array().map(|a| a.iter().map(|s| s.as_str().map(String::from)).collect::<Option<Vec<_>>>()).and_then(|o| o);
    let qids = strs(x.get("qids")?)?;
    let h = strs(x.get("h")?)?;
    if qids.len() != h.len() {
        return None;
    }
    let m = x.get("m").and_then(strs).filter(|m| m.len() == qids.len());
    Some(Index { qids, h, m })
}

/// What changed from `prev` to `next`, per qid: (added, changed, removed).
pub fn diff(prev: Option<&Index>, next_qids: &[String], next_h: &[String]) -> (i64, i64, i64) {
    let prev_h: HashMap<&String, &String> = prev.map(|p| p.qids.iter().zip(p.h.iter()).collect()).unwrap_or_default();
    let next_set: HashSet<&String> = next_qids.iter().collect();
    let added = next_qids.iter().filter(|q| !prev_h.contains_key(q)).count() as i64;
    let changed = next_qids.iter().enumerate().filter(|(k, q)| prev_h.get(q).is_some_and(|h| *h != &next_h[*k])).count() as i64;
    let removed = prev_h.keys().filter(|q| !next_set.contains(*q)).count() as i64;
    (added, changed, removed)
}

/// One form of a version's questions a member may be served (the full form, and for a bank that hides its keys the stripped form its
/// viewers read), with its key-free match hashes and each question's image.
pub struct ScanForm {
    pub qs: Vec<Json>,
    pub mh: Vec<String>,
    pub img_at: Vec<Option<String>>,
}
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scan {
    pub keys: Vec<String>,
    pub shas: Vec<String>,
    pub exact: Vec<usize>,
    pub near: Vec<usize>,
    pub copyright: Vec<usize>,
    pub actions: Vec<String>,
    /// The block hashes that matched (student data, copyright): the evidence a flag keeps when its version's body is pruned.
    pub hits: Vec<String>,
    pub chits: Vec<String>,
}

/// Each question's image hash (`media/<sha>.<ext>`), or None.
pub fn images_at(qs: &[Json]) -> Vec<Option<String>> {
    qs.iter()
        .map(|q| {
            let f = q.get("image").and_then(|i| i.get("file")).and_then(Json::as_str)?;
            let rest = f.strip_prefix("media/")?;
            let (sha, ext) = rest.split_once('.')?;
            (super::sqlfrag::is_sha(sha) && matches!(ext, "png" | "jpg" | "gif" | "webp")).then(|| sha.to_string())
        })
        .collect()
}

fn dedup<T: Clone + Eq + std::hash::Hash>(xs: impl IntoIterator<Item = T>) -> Vec<T> {
    let mut seen = HashSet::new();
    xs.into_iter().filter(|x| seen.insert(x.clone())).collect()
}

/// The student-data and copyright checks of a version (positions are question indices), over EVERY form a member may be served, each
/// position's keys and stems the union of its forms'.
pub fn privacy_scan(x: &PoolCtx, fp: &Fingerprinter, forms: &[ScanForm], sender: &str) -> rusqlite::Result<Scan> {
    let n = forms.first().map(|f| f.qs.len()).unwrap_or(0);
    let mut keys_at: Vec<Vec<Option<String>>> = Vec::new();
    let mut stems: Vec<Vec<StemSketch>> = Vec::new();
    let mut img_sets: Vec<Vec<Option<String>>> = Vec::new();
    for i in 0..n {
        let (mut ks, mut ss, mut im): (Vec<Option<String>>, Vec<StemSketch>, Vec<Option<String>>) = (vec![], vec![], vec![]);
        for fm in forms {
            let Some(q) = fm.qs.get(i) else { continue };
            let (expl, ans) = field_keys(fp, q);
            ks.push(fm.mh.get(i).cloned());
            ks.push(Some(norm_skeleton_key(fp, q)));
            ks.push(Some(stem_key(fp, q)));
            ks.push(Some(format!("f:{}", near_hash(fp, q, false))));
            ks.push(expl);
            ks.push(ans);
            ks.extend(part_keys_of(fp, q).into_iter().map(|(_, k)| Some(k)));
            let t = stem_tokens(q.get("stem").unwrap_or(&Json::Null));
            ss.push(StemSketch { bands: stem_bands(&t), tokens: t });
            let parts = q.get("kind").and_then(|k| k.get("Multipart")).and_then(|m| m.get("parts")).map(Json::as_arr).unwrap_or(&[]);
            for p in parts {
                let pt = stem_tokens(p.get("stem").unwrap_or(&Json::Null));
                ss.push(StemSketch { bands: stem_bands(&pt), tokens: pt });
            }
            im.push(fm.img_at.get(i).cloned().flatten());
        }
        keys_at.push(dedup(ks));
        stems.push(ss);
        img_sets.push(dedup(im));
    }
    let keys: Vec<String> = dedup(keys_at.iter().flatten().flatten().cloned());
    let shas: Vec<String> = dedup(img_sets.iter().flatten().flatten().cloned());
    let col = |sql: &str, vals: &[String], extra: Option<(i64, &str)>| -> rusqlite::Result<Vec<(String, Option<String>)>> {
        let mut st = x.conn.prepare(sql)?;
        let j = serde_json::to_string(vals).unwrap();
        let map = |r: &rusqlite::Row| -> rusqlite::Result<(String, Option<String>)> { Ok((r.get(0)?, r.get(1).ok())) };
        match extra {
            None => st.query_map([j], map)?.collect(),
            Some((now, sender)) => st.query_map(params![j, now, sender], map)?.collect(),
        }
    };
    let hit: HashSet<String> = col("SELECT hash FROM privacy_blocks WHERE kind = 'match' AND hash IN (SELECT value FROM json_each(?1))", &keys, None)?.into_iter().map(|r| r.0).collect();
    let blob_hit: HashSet<String> = if shas.is_empty() { HashSet::new() } else { col("SELECT hash FROM privacy_blocks WHERE kind = 'blob' AND hash IN (SELECT value FROM json_each(?1))", &shas, None)?.into_iter().map(|r| r.0).collect() };
    let at = |ks: &[Option<String>], im: &[Option<String>], h: &HashSet<String>, b: &HashSet<String>| ks.iter().flatten().any(|k| h.contains(k)) || im.iter().flatten().any(|s| b.contains(s));
    let exact: Vec<usize> = (0..n).filter(|i| at(&keys_at[*i], &img_sets[*i], &hit, &blob_hit)).collect();
    let near = privacy_near_positions(x.conn, fp, &stems, &keys_at, || Ok(hit.clone()), &mut HashMap::new(), PRIVACY_BAND_ROWS_MAX)?;
    // The same keys and images against the public pool's copyright re-upload blocks.
    let c_sql = |kind: &str, alias: &str| format!("SELECT {alias}.hash, {alias}.action_id FROM blocked_hashes {alias} WHERE {alias}.kind = '{kind}' AND {alias}.hash IN (SELECT value FROM json_each(?1)) AND {}", super::circlesendsql::copyright_active(alias, "?2", "?3"));
    let c_rows = col(&c_sql("match", "ck"), &keys, Some((x.now, sender)))?;
    let b_rows = if shas.is_empty() { vec![] } else { col(&c_sql("blob", "ci"), &shas, Some((x.now, sender)))? };
    let c_hit: HashSet<String> = c_rows.iter().map(|r| r.0.clone()).collect();
    let c_blob: HashSet<String> = b_rows.iter().map(|r| r.0.clone()).collect();
    let copyright: Vec<usize> = (0..n).filter(|i| at(&keys_at[*i], &img_sets[*i], &c_hit, &c_blob)).collect();
    let mut actions: Vec<String> = dedup(c_rows.iter().chain(b_rows.iter()).filter_map(|r| r.1.clone()));
    actions.sort();
    actions.truncate(50);
    let sorted_cut = |a: &HashSet<String>, b: &HashSet<String>| {
        let mut v: Vec<String> = a.iter().chain(b.iter()).cloned().collect();
        v.sort();
        v.truncate(EVIDENCE_HASHES);
        v
    };
    Ok(Scan { hits: sorted_cut(&hit, &blob_hit), chits: sorted_cut(&c_hit, &c_blob), keys, shas, exact, near, copyright, actions })
}

#[cfg(test)]
#[path = "circlesendmeta_tests.rs"]
mod tests;
