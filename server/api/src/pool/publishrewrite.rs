//! The middle of a publish (worker/src/pool/publish.ts `publishOnce`, "validate, rewrite, validate again"): the author's metadata, the payload's shape, the
//! question ids, the images, and the server's own rewrite of ids, dates and image references. Everything the author sent that the server must own (ids, dates,
//! provenance) is replaced here; only share-core's validated output is ever stored (the next step).
use super::core::{Core, CoreErr, Meta, PublishOpts, questions_of};
use super::ctx::{Fail, PoolCtx, PoolError, Res};
use super::jsonscan::scan_json;
use super::publishmeta::PublishMeta;
use super::publishstart::Landing;
use super::qjson::sort_utf16;
use super::sqlfrag::{DAY_MS, alive_sql, is_sha, new_id};
use crate::blobs::BlobStore;
use rusqlite::params;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

/// JS `\s`: what `/\s+/u` collapses.
fn js_ws(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}
/// Runs of whitespace as one space, trimmed (share-core's Line collapse).
fn collapse_ws(s: &str) -> String {
    s.split(js_ws).filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" ")
}
fn nfc(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfc().collect()
}
fn nfkc(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfkc().collect()
}

/// The author's NFC text for a metadata line share-core accepted as `cleaned` (its NFKC form): used only when share-core changed nothing but the
/// compatibility fold (and so its checks hold for it), and it is no longer than what share-core allowed. Otherwise share-core's text.
pub fn authors_text(raw: Option<&Value>, cleaned: &str) -> String {
    let Some(raw) = raw.and_then(Value::as_str) else { return cleaned.to_string() };
    let n = collapse_ws(&nfc(raw));
    if collapse_ws(&nfkc(&n)) == cleaned && n.chars().count() <= cleaned.chars().count() { n } else { cleaned.to_string() }
}

/// share-core's cleaned metadata with the author's own NFC display text restored wherever share-core changed nothing else (the NFKC fold is only the search
/// column's). Tags are sorted.
pub fn authors_meta(raw: &Value, meta: &Meta) -> Meta {
    let cleaned: HashSet<&str> = meta.tags.iter().map(String::as_str).collect();
    let mut tags: Vec<String> = vec![];
    let mut used: HashSet<String> = HashSet::new();
    for t in raw.get("tags").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
        let Some(t) = t.as_str() else { continue };
        let n = collapse_ws(&nfc(t)).to_lowercase();
        let k = collapse_ws(&nfkc(&n)).to_lowercase();
        if cleaned.contains(k.as_str()) && !used.contains(&k) && n.chars().count() <= k.chars().count() {
            if !tags.contains(&n) {
                tags.push(n);
            }
            used.insert(k);
        }
    }
    for k in &meta.tags {
        if !used.contains(k) && !tags.contains(k) {
            tags.push(k.clone());
        }
    }
    sort_utf16(&mut tags);
    Meta {
        title: authors_text(raw.get("title"), &meta.title),
        subject: authors_text(raw.get("subject"), &meta.subject),
        course: authors_text(raw.get("course"), &meta.course),
        grade: authors_text(raw.get("grade"), &meta.grade),
        tags,
        standards: meta.standards.clone(),
    }
}

fn server_error() -> Fail {
    PoolError::new(500, "server_error").into()
}
/// A share-core refusal as the author sees it; a trap or a missing key is a plain 500.
pub fn core_fail(e: CoreErr) -> Fail {
    match e {
        CoreErr::Fail(f) => super::circlesendmeta::strip_refusal(&f).into(),
        _ => server_error(),
    }
}

pub fn image_ext(content_type: &str) -> &'static str {
    match content_type {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        _ => "webp",
    }
}

/// The sha of an image reference `media/<sha256>.<ext>`.
fn media_sha(file: &str) -> Option<&str> {
    let rest = file.strip_prefix("media/")?;
    let (sha, ext) = rest.split_once('.')?;
    (is_sha(sha) && matches!(ext, "png" | "jpg" | "gif" | "webp")).then_some(sha)
}
fn img_of(q: &Value) -> Option<String> {
    q.get("image").and_then(|i| i.get("file")).and_then(Value::as_str).and_then(media_sha).map(String::from)
}

/// An origin the author declared on a question: recorded for the hard check, then dropped; provenance is the server's.
#[derive(Debug, Clone, PartialEq)]
pub struct Declared {
    pub item: String,
    pub qid: String,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlobInfo {
    pub content_type: String,
    pub width: i64,
    pub height: i64,
    pub privacy_purged_at: Option<i64>,
}

#[derive(Debug)]
pub struct Rewritten {
    pub meta: Meta,
    /// "bank" or "question".
    pub kind: &'static str,
    pub n: usize,
    /// The qid each question arrived with (None when it sent none).
    pub sent: Vec<Option<String>>,
    pub qids: Vec<String>,
    pub new_qids: Vec<String>,
    pub declared: Vec<Option<Declared>>,
    pub blob_info: HashMap<String, BlobInfo>,
    /// The sha of each question's image, if any.
    pub images: Vec<Option<String>>,
    /// The rewritten payload, ready for share-core's one validating, stripping, hashing call.
    pub bytes: Vec<u8>,
}

fn invalid_raw(core: &Core, payload: &[u8], m: &PublishMeta) -> Fail {
    let opts = PublishOpts { hide_key: m.hide_key, labels: m.labels.clone(), order_acks: m.order_acks.clone(), image_ok: m.image_ok.clone(), ack_formatting: m.ack_formatting.clone() };
    match core.publish_prepare(payload, &opts) {
        Err(e) => core_fail(e),
        Ok(_) => PoolError::new(422, "invalid_payload").with("errors", json!([{ "code": "invalid", "path": "" }])).into(),
    }
}

/// Validates the metadata and the payload's shape, assigns question ids (a sent qid is kept only if it already belongs to this item and appears once),
/// checks the images are verified and the author's (and extends the author's hold on each), and rewrites ids, dates and image references. Nothing is
/// stored but the pending-blob hold; a dry run stores nothing.
#[allow(clippy::too_many_arguments)]
pub fn rewrite(x: &PoolCtx, store: &BlobStore, core: &Core, me: &str, land: &Landing, is_new: bool, m: &PublishMeta, payload: &[u8], dry: bool) -> Res<Rewritten> {
    let mut meta = core.validate_meta(&m.meta).map_err(core_fail)?;
    meta = authors_meta(&m.meta, &meta);
    // share-core's structural limits on the RAW bytes first (depth, values, objects, duplicate keys), so this parse never sees a different document than
    // share-core validates and never recurses without bound.
    if !scan_json(payload).ok {
        return Err(invalid_raw(core, payload, m));
    }
    let Ok(mut raw) = serde_json::from_slice::<Value>(payload) else { return Err(invalid_raw(core, payload, m)) };
    let is_obj = |v: &Value| v.is_object();
    let shaped = is_obj(&raw)
        && raw.get("schema_version").is_some_and(Value::is_number)
        && match raw.get("bank").filter(|b| !b.is_null()) {
            Some(b) => b.get("questions").and_then(Value::as_array).is_some_and(|q| !q.is_empty() && q.iter().all(is_obj)) && raw.get("question").is_none(),
            None => raw.get("question").is_some_and(is_obj) && raw.get("bank").is_none(),
        };
    if !shaped {
        return Err(invalid_raw(core, payload, m));
    }
    let kind = if raw.get("bank").is_some_and(|b| !b.is_null()) { "bank" } else { "question" };
    // A republish must be schema v2: v1 cannot carry `published`, so its questions would lose their identity (and the hidden-change and provenance rules
    // with it).
    if !is_new && raw["schema_version"].as_f64().unwrap_or(0.0) < 2.0 {
        return Err(PoolError::new(422, "schema_upgrade_required").with("message", json!("Republishing needs schema version 2 (update verSHion).")).into());
    }
    if land.item_kind.as_deref().is_some_and(|k| k != kind) {
        return Err(PoolError::new(422, "kind_mismatch").into());
    }
    let qs: Vec<Value> = questions_of(&raw).into_iter().cloned().collect();
    let n = qs.len();
    if n == 0 {
        return Err(PoolError::new(422, "invalid_payload").with("errors", json!([{ "code": "no_questions", "path": "bank.questions" }])).into());
    }
    // qids: a sent qid is kept only if it already belongs to this item and appears once.
    let sent: Vec<Option<String>> = qs.iter().map(|q| q.get("published").and_then(|p| p.get("qid")).and_then(Value::as_str).map(String::from)).collect();
    let mut owned: HashSet<String> = HashSet::new();
    if !is_new && sent.iter().any(Option::is_some) {
        let distinct: Vec<&String> = sent.iter().flatten().collect::<HashSet<_>>().into_iter().collect();
        let mut st = x.conn.prepare("SELECT qid FROM question_ids WHERE item_id = ?1 AND qid IN (SELECT value FROM json_each(?2))")?;
        for r in st.query_map(params![land.item_id, json!(distinct).to_string()], |r| r.get::<_, String>(0))? {
            owned.insert(r?);
        }
    }
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for s in sent.iter().flatten() {
        *counts.entry(s).or_insert(0) += 1;
    }
    let qids: Vec<String> = sent.iter().map(|s| s.as_ref().filter(|s| owned.contains(*s) && counts.get(s.as_str()) == Some(&1)).cloned().unwrap_or_else(new_id)).collect();
    let new_qids: Vec<String> = qids.iter().enumerate().filter(|(i, q)| Some(*q) != sent[*i].as_ref() || !owned.contains(*q)).map(|(_, q)| q.clone()).collect();

    // Images: every referenced blob must be verified and the author's (pending, or in one of their publishes).
    let images: Vec<Option<String>> = qs.iter().map(img_of).collect();
    let mut shas: Vec<String> = vec![];
    for s in images.iter().flatten() {
        if !shas.contains(s) {
            shas.push(s.clone());
        }
    }
    let mut blob_info: HashMap<String, BlobInfo> = HashMap::new();
    if !shas.is_empty() {
        let list = json!(shas).to_string();
        let mut st = x.conn.prepare(
            "SELECT b.sha256, b.content_type, b.width, b.height, b.privacy_purged_at FROM blobs b
          WHERE b.sha256 IN (SELECT value FROM json_each(?1))
            AND (EXISTS (SELECT 1 FROM pending_blobs pb WHERE pb.sha256 = b.sha256 AND pb.uploader_id = ?2 AND pb.until >= ?3)
              OR EXISTS (SELECT 1 FROM item_blobs ib JOIN item_publishes ip ON ip.publish_id = ib.publish_id WHERE ib.sha256 = b.sha256 AND ip.author_id = ?2))",
        )?;
        for r in st.query_map(params![list, me, x.now], |r| Ok((r.get::<_, String>(0)?, BlobInfo { content_type: r.get(1)?, width: r.get(2)?, height: r.get(3)?, privacy_purged_at: r.get(4)? })))? {
            let (sha, info) = r?;
            blob_info.insert(sha, info);
        }
        let missing: Vec<usize> = images.iter().enumerate().filter(|(_, s)| s.as_ref().is_some_and(|s| !blob_info.contains_key(s))).map(|(i, _)| i).take(100).collect();
        if !missing.is_empty() {
            return Err(PoolError::new(422, "blob_missing").with("positions", json!(missing)).into());
        }
        // Validation extends the author's hold on every referenced blob, so the GC cannot collect one between this check and the commit. Already charged:
        // no quota change.
        if !dry {
            x.conn.execute(
                &format!(
                    "INSERT INTO pending_blobs (sha256, uploader_id, size, until) SELECT sha256, ?2, size, ?3 FROM blobs WHERE sha256 IN (SELECT value FROM json_each(?1)) AND {}
           ON CONFLICT (sha256, uploader_id) DO UPDATE SET until = MAX(until, excluded.until)",
                    alive_sql("?2")
                ),
                params![list, me, x.now + DAY_MS],
            )?;
        }
        // A blob GC could have raced an upload: the bytes must really be there. A privacy-blocked image's bytes are never required (they may be gone, or
        // never stored for this upload): such a publish is held at commit, never refused here.
        let blocked: HashSet<String> = x
            .conn
            .prepare("SELECT hash FROM privacy_blocks WHERE kind = 'blob' AND hash IN (SELECT value FROM json_each(?1))")?
            .query_map([&list], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        for s in &shas {
            let gone = blob_info[s].privacy_purged_at.is_none() && !blocked.contains(s) && store.len(&format!("blobs/{s}")).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?.is_none();
            if gone {
                if !dry {
                    x.conn.execute("UPDATE blobs SET missing_at = COALESCE(missing_at, ?2) WHERE sha256 = ?1", params![s, x.now])?;
                }
                let positions: Vec<usize> = images.iter().enumerate().filter(|(_, i)| i.as_deref() == Some(s)).map(|(i, _)| i).take(100).collect();
                return Err(PoolError::new(422, "blob_missing").with("reason", json!("reupload")).with("positions", json!(positions)).into());
            }
        }
    }

    // Declared origins are recorded for the hard check, then dropped: provenance is the server's.
    let declared: Vec<Option<Declared>> = qs
        .iter()
        .map(|q| {
            let o = q.get("origin").filter(|o| o.is_object())?;
            Some(Declared {
                item: o.get("item_id").and_then(Value::as_str)?.to_string(),
                qid: o.get("qid").and_then(Value::as_str).unwrap_or("").to_string(),
                version: o.get("version").and_then(Value::as_i64).unwrap_or(-1),
            })
        })
        .collect();
    let day = x.now.div_euclid(DAY_MS) * DAY_MS;
    let fix_q = |q: &mut Value, i: usize| {
        let Some(o) = q.as_object_mut() else { return };
        o.insert("id".into(), json!(qids[i]));
        o.insert("created".into(), json!(day));
        o.insert("modified".into(), json!(day));
        o.remove("origin");
        o.remove("published");
        if let Some(s) = images[i].as_ref() {
            if o.get("image").is_some_and(|v| !v.is_null()) {
                let b = &blob_info[s];
                o.insert("image".into(), json!({ "file": format!("media/{s}.{}", image_ext(&b.content_type)), "width": b.width, "height": b.height }));
            }
        }
    };
    if let Some(bank) = raw.get_mut("bank").filter(|b| !b.is_null()).and_then(Value::as_object_mut) {
        bank.insert("id".into(), json!(land.item_id));
        bank.insert("created".into(), json!(day));
        bank.insert("modified".into(), json!(day));
        bank.remove("origin");
        bank.remove("published");
        if let Some(Value::Array(list)) = bank.get_mut("questions") {
            for (i, q) in list.iter_mut().enumerate() {
                fix_q(q, i);
            }
        }
    } else if let Some(q) = raw.get_mut("question") {
        fix_q(q, 0);
    }
    let bytes = serde_json::to_vec(&raw).map_err(|_| server_error())?;
    Ok(Rewritten { meta, kind, n, sent, qids, new_qids, declared, blob_info, images, bytes })
}

#[cfg(test)]
#[path = "publishrewrite_tests.rs"]
mod tests;
