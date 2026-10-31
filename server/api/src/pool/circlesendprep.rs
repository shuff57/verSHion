//! Sending a version of a circle bank, part 3 (worker/src/pool/circlesend.ts `precheck`, `checkHead`, `sendOnce`): the cheap checks before the
//! heavy work, then the payload's shape, qids, images, origins, share-core's one validation and the scan, ending in the commit spec.
//! The Worker's heavy slot, byte reservations and account lock belong to the route layer (or have no native counterpart).
use super::circlebanks::keys_hidden_unavailable;
use super::circlecommit::{Bank, CommitSpec, Kind, Listing, conflict, fresh_bank, license_author_only};
use super::circlehide::{StripDecisions, decisions_json, hash_pairs, kept_images, recorded_decisions};
use super::circlesendmeta::*;
use super::core::{Core, CoreErr, CoreError, Fail as CoreFail, PublishOpts, questions_of};
use super::ctx::{Fail, PoolCtx, PoolError, Res};
use super::fingerprint::Fingerprinter;
use super::license::allowed_by_all;
use super::poolkeys::PoolKeys;
use super::qjson::Json;
use super::readmisc::ext_of;
use super::sqlfrag::{DAY_MS, alive_sql, is_id, new_id, visible_to};
use crate::auth::validate::clean_text;
use crate::blobs::BlobStore;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};

pub struct SendEnv<'a> {
    pub x: &'a PoolCtx<'a>,
    pub core: &'a Core,
    pub fp: &'a Fingerprinter,
    pub keys: &'a PoolKeys,
    pub store: &'a BlobStore,
}

/// A credited origin of a question: the public item it came from and the licence it carried.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Origin {
    pub pos: usize,
    pub item_id: String,
    pub qid: Option<String>,
    pub version: Option<i64>,
    pub license: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cq: Option<String>,
}

/// What a send has parsed of its request: the raw bytes (for the request hash), the meta line and the payload.
pub struct ReadSend<'a> {
    pub raw: &'a [u8],
    pub meta: &'a Map<String, Value>,
    pub payload: &'a [u8],
}

pub struct PreGo {
    pub sm: SendMeta,
    pub request_sha: String,
    pub merged_from: Option<i64>,
}
pub enum Pre {
    /// The first answer of this commit_id, replayed.
    Replay(Value),
    Go(PreGo),
}

fn bad(field: &str) -> PoolError {
    PoolError::new(400, "invalid_request").with("field", json!(field))
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
fn invalid_payload(code: &str, path: &str) -> PoolError {
    PoolError::new(422, "invalid_payload").with("errors", json!([{ "code": code, "path": path }]))
}

/// True when the head moved past the send's base (a 409); the licence rule is an error (the licence is the author's to choose; an editor
/// sends with the bank's).
pub fn check_head(bank: Option<&Bank>, sm: &SendMeta, me: &str) -> Result<bool, PoolError> {
    let Some(bank) = bank else { return Ok(false) };
    if Some(bank.head_version) != sm.base {
        return Ok(true);
    }
    if bank.author_id != me && sm.license != bank.license {
        return Err(license_author_only(&bank.license));
    }
    Ok(false)
}

/// The cheap part of a send: its meta, idempotency (commit_id), the head it is based on, the licence rule and the merge token.
pub fn precheck(env: &SendEnv, me: &str, cid: &str, path_bid: Option<&str>, read: &ReadSend) -> Res<Pre> {
    let sm = read_send_meta(read.meta, path_bid.is_none())?;
    let request_sha = super::blobs::sha256_hex(read.raw);
    if let Some(v) = replay_of(env.x, env.store, me, cid, path_bid, &sm.commit_id, &request_sha)? {
        return Ok(Pre::Replay(v));
    }
    let bank = match path_bid {
        None => None,
        Some(b) => Some(fresh_bank(env.x, cid, b)?),
    };
    if check_head(bank.as_ref(), &sm, me)? {
        return Err(conflict(env.x, &bank.as_ref().unwrap().id)?.into());
    }
    let merged_from = match &sm.merge_token {
        None => None,
        Some(t) => Some(
            env.keys
                .verify_merge_token(env.x.now, t, me, &bank.as_ref().unwrap().id, sm.base.unwrap())
                .ok_or_else(|| PoolError::new(422, "merge_token_invalid").with("message", json!("This merge is out of date. Get the latest changes and merge again, then send.")))?,
        ),
    };
    if sm.merged_from.is_some() && sm.merged_from != merged_from {
        return Err(bad("merged_from").into());
    }
    Ok(Pre::Go(PreGo { sm, request_sha, merged_from }))
}

fn media_sha(v: &Value) -> Option<String> {
    let f = v.get("image")?.get("file")?.as_str()?;
    let (sha, ext) = f.strip_prefix("media/")?.split_once('.')?;
    (super::sqlfrag::is_sha(sha) && matches!(ext, "png" | "jpg" | "gif" | "webp")).then(|| sha.to_string())
}
fn to_json(v: &Value) -> Json {
    Json::parse(v.to_string().as_bytes()).unwrap_or(Json::Null)
}
fn dedup<T: Clone + Eq + std::hash::Hash>(xs: impl IntoIterator<Item = T>) -> Vec<T> {
    let mut seen = HashSet::new();
    xs.into_iter().filter(|x| seen.insert(x.clone())).collect()
}

struct BlobInfo {
    content_type: String,
    width: i64,
    height: i64,
    size: i64,
    privacy_purged_at: Option<i64>,
}

/// The heavy part: the payload as sent becomes a commit spec (or a refusal). `scan_ok`: the JSON scan of the payload passed.
pub fn send_once(env: &SendEnv, me: &str, cid: &str, path_bid: Option<&str>, pre: &PreGo, read: &ReadSend, scan_ok: bool) -> Res<CommitSpec> {
    let x = env.x;
    let is_new = path_bid.is_none();
    let (sm, request_sha, merged_from) = (&pre.sm, pre.request_sha.clone(), pre.merged_from);
    let now = x.now;
    // The head the sender based on, read again (checked once more in the commit's compare-and-set).
    let bank = match path_bid {
        None => None,
        Some(b) => Some(fresh_bank(x, cid, b)?),
    };
    if check_head(bank.as_ref(), sm, me)? {
        return Err(conflict(x, &bank.as_ref().unwrap().id)?.into());
    }

    // ---- the payload: shape, qids, images, origins, rewrite ----
    let payload = read.payload;
    let refuse_raw = || -> Fail {
        match env.core.publish_prepare(payload, &PublishOpts::default()) {
            Err(e) => match core_fail(e) {
                Ok(f) => invalid(&f).into(),
                Err(e) => e,
            },
            Ok(_) => invalid_payload("invalid", "").into(),
        }
    };
    if !scan_ok {
        return Err(refuse_raw());
    }
    let Ok(mut raw) = serde_json::from_slice::<Value>(payload) else { return Err(refuse_raw()) };
    // A circle bank is a bank (one question is a bank of one).
    let shaped = raw.get("schema_version").is_some_and(Value::is_number)
        && raw.get("bank").and_then(|b| b.get("questions")).and_then(Value::as_array).is_some_and(|q| !q.is_empty() && q.iter().all(Value::is_object))
        && raw.get("question").is_none();
    if !shaped {
        return Err(invalid_payload("bank_required", "bank").into());
    }
    let n = raw["bank"]["questions"].as_array().unwrap().len();
    // A keyless question (a copy made without the answer keys) is never a circle version, whatever the bank's setting: decided by
    // share-core's own rule, on the payload as sent.
    let kv = env.core.validate_for_publish(payload, false, &[], &[]);
    let keyless_again = kv.is_err();
    if let Err(CoreErr::Fail(f)) = &kv {
        let k = keyless_positions(f);
        if !k.is_empty() {
            return Err(keyless_question(&k, raw["bank"]["questions"].as_array().unwrap()).into());
        }
    }
    let bid = bank.as_ref().map(|b| b.id.clone()).unwrap_or_else(new_id);

    // qids: a sent qid (question.published.qid) is kept only if the head held it and it appears once.
    let mut prev_idx: Option<Index> = None;
    let mut prev_origins: Vec<Origin> = Vec::new();
    if let Some(hv) = bank.as_ref().and_then(|b| b.head_version_id.as_ref()) {
        let v: Option<(String, Option<String>)> = x.conn.query_row("SELECT index_key, origins FROM circle_versions WHERE id = ?1", [hv], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
        if let Some((ik, origins)) = v {
            prev_idx = read_index(env.store, &ik);
            prev_origins = origins.and_then(|o| serde_json::from_str(&o).ok()).unwrap_or_default();
        }
    }
    let prev_qids: HashSet<&String> = prev_idx.iter().flat_map(|p| p.qids.iter()).collect();
    let qs_ref = raw["bank"]["questions"].as_array().unwrap().clone();
    let sent: Vec<Option<String>> = qs_ref.iter().map(|q| q.get("published").and_then(|p| p.get("qid")).and_then(Value::as_str).map(String::from)).collect();
    let usable = |s: &Option<String>| s.as_deref().is_some_and(|s| is_id(s) && prev_qids.contains(&s.to_string()));
    // A qid sent more than once is a copy: one occurrence keeps the qid (the one whose content is unchanged from the head, else the first),
    // each other one is a new question.
    let repeated = sent.iter().enumerate().any(|(k, s)| usable(s) && sent.iter().position(|t| t == s) != Some(k));
    let raw_h = if repeated && prev_idx.is_some() { Some(env.core.content_hashes(&qs_ref).map_err(|_| server_error())?) } else { None };
    let head_h: HashMap<&String, &String> = prev_idx.iter().flat_map(|p| p.qids.iter().zip(p.h.iter())).collect();
    let mut holder: HashMap<String, usize> = HashMap::new();
    for (k, s) in sent.iter().enumerate() {
        if !usable(s) {
            continue;
        }
        let q = s.clone().unwrap();
        match holder.get(&q) {
            None => {
                holder.insert(q, k);
            }
            Some(cur) => {
                if let Some(rh) = &raw_h {
                    let hh = head_h.get(&q);
                    if Some(&&rh[k]) == hh && Some(&&rh[*cur]) != hh {
                        holder.insert(q, k);
                    }
                }
            }
        }
    }
    let qids: Vec<String> = sent.iter().enumerate().map(|(k, s)| if usable(s) && holder.get(s.as_ref().unwrap()) == Some(&k) { s.clone().unwrap() } else { new_id() }).collect();
    // The qid a later occurrence copies (the bank's author's copies carry its credit, below).
    let copy_of: Vec<Option<String>> = sent.iter().enumerate().map(|(k, s)| if usable(s) && holder.get(s.as_ref().unwrap()) != Some(&k) { s.clone() } else { None }).collect();
    // A version that drops more than half of the head's questions is confirmed (a mistake guard).
    if let (Some(p), false) = (&prev_idx, sm.confirm) {
        let kept: HashSet<&String> = qids.iter().collect();
        let dropped = p.qids.iter().filter(|q| !kept.contains(q)).count() as i64;
        if is_large(dropped, p.qids.len() as i64) {
            return Err(large_error(dropped, p.qids.len() as i64).into());
        }
    }

    // Images: verified, and the sender's (pending, in one of their publishes) or already in this bank's kept versions.
    let img_at: Vec<Option<String>> = qs_ref.iter().map(media_sha).collect();
    let shas: Vec<String> = dedup(img_at.iter().flatten().cloned());
    let mut blob_info: HashMap<String, BlobInfo> = HashMap::new();
    if !shas.is_empty() {
        let shas_json = serde_json::to_string(&shas).unwrap();
        let mut st = x.conn.prepare(
            "SELECT b.sha256, b.content_type, b.width, b.height, b.size, b.privacy_purged_at FROM blobs b
          WHERE b.sha256 IN (SELECT value FROM json_each(?1))
            AND (EXISTS (SELECT 1 FROM pending_blobs pb WHERE pb.sha256 = b.sha256 AND pb.uploader_id = ?2 AND pb.until >= ?3)
              OR EXISTS (SELECT 1 FROM item_blobs ib JOIN item_publishes ip ON ip.publish_id = ib.publish_id WHERE ib.sha256 = b.sha256 AND ip.author_id = ?2)
              OR EXISTS (SELECT 1 FROM circle_version_blobs vb JOIN circle_versions cv ON cv.id = vb.version_id WHERE vb.sha256 = b.sha256 AND cv.bank_id = ?4 AND cv.status IN ('current', 'previous')))",
        )?;
        let rows = st.query_map(params![shas_json, me, now, bid], |r| {
            Ok((r.get::<_, String>(0)?, BlobInfo { content_type: r.get(1)?, width: r.get::<_, Option<i64>>(2)?.unwrap_or(0), height: r.get::<_, Option<i64>>(3)?.unwrap_or(0), size: r.get(4)?, privacy_purged_at: r.get(5)? }))
        })?;
        for r in rows {
            let (k, v) = r?;
            blob_info.insert(k, v);
        }
        drop(st);
        let missing: Vec<usize> = img_at.iter().enumerate().filter(|(_, s)| s.as_ref().is_some_and(|s| !blob_info.contains_key(s))).map(|(i, _)| i).take(100).collect();
        if !missing.is_empty() {
            return Err(PoolError::new(422, "blob_missing").with("positions", json!(missing)).into());
        }
        // The sender's hold on each image is extended until the commit (the GC cannot collect it meanwhile).
        x.conn.execute(
            &format!(
                "INSERT INTO pending_blobs (sha256, uploader_id, size, until) SELECT sha256, ?2, size, ?3 FROM blobs WHERE sha256 IN (SELECT value FROM json_each(?1)) AND {}
         ON CONFLICT (sha256, uploader_id) DO UPDATE SET until = MAX(until, excluded.until)",
                alive_sql("?2")
            ),
            params![shas_json, me, now + DAY_MS],
        )?;
        // The bytes must really be stored (a privacy-blocked image's never are: never refused here, so a send cannot confirm a block).
        let mut st = x.conn.prepare("SELECT hash FROM privacy_blocks WHERE kind = 'blob' AND hash IN (SELECT value FROM json_each(?1))")?;
        let blocked: HashSet<String> = st.query_map([&shas_json], |r| r.get(0))?.collect::<Result<_, _>>()?;
        drop(st);
        for s in &shas {
            if !blocked.contains(s) && blob_info[s].privacy_purged_at.is_none() && env.store.len(&format!("blobs/{s}")).ok().flatten().is_none() {
                x.conn.execute("UPDATE blobs SET missing_at = COALESCE(missing_at, ?2) WHERE sha256 = ?1", params![s, now])?;
                let pos: Vec<usize> = img_at.iter().enumerate().filter(|(_, v)| v.as_ref() == Some(s)).map(|(i, _)| i).take(100).collect();
                return Err(PoolError::new(422, "blob_missing").with("reason", json!("reupload")).with("positions", json!(pos)).into());
            }
        }
    }

    // Public material sent into a circle bank: the bank's licence must be one each DECLARED origin allows (declared origins only; an origin
    // the sender cannot see is ignored; her own items allow anything). An origin recorded for a question the head held stays with it in every
    // later version, unless the sender declares it again; the same text sent again carries it too (by the key-free match hash, below). An
    // origin a NON-author declares on a question the head already holds is ignored.
    let kept = |k: usize| sent[k].is_some() && sent[k].as_ref() == Some(&qids[k]);
    struct Declared {
        pos: usize,
        item_id: String,
        qid: Option<String>,
        version: Option<i64>,
    }
    let declared: Vec<Declared> = qs_ref
        .iter()
        .enumerate()
        .filter_map(|(i, q)| {
            let o = q.get("origin")?.as_object()?;
            let item_id = o.get("item_id")?.as_str().filter(|s| is_id(s))?.to_string();
            Some(Declared { pos: i, item_id, qid: o.get("qid").and_then(Value::as_str).map(String::from), version: o.get("version").and_then(Value::as_i64) })
        })
        .collect();
    let mut origins: Vec<Origin> = Vec::new();
    let mut declared_at: HashSet<usize> = HashSet::new();
    if !declared.is_empty() {
        let ids = dedup(declared.iter().map(|d| d.item_id.clone()));
        let mut st = x.conn.prepare(&format!("SELECT i.id, i.license, i.author_id FROM items i JOIN users u ON u.id = i.author_id WHERE i.id IN (SELECT value FROM json_each(?1)) AND {}", visible_to("?2")))?;
        let by: HashMap<String, (String, String)> = st
            .query_map(params![serde_json::to_string(&ids).unwrap(), me], |r| Ok((r.get::<_, String>(0)?, (r.get::<_, String>(1)?, r.get::<_, String>(2)?))))?
            .collect::<Result<_, _>>()?;
        for d in &declared {
            let Some((lic, author)) = by.get(&d.item_id) else { continue };
            if author == me {
                continue;
            }
            if bank.as_ref().is_some_and(|b| b.author_id != me) && kept(d.pos) {
                continue;
            }
            origins.push(Origin { pos: d.pos, item_id: d.item_id.clone(), qid: d.qid.clone(), version: d.version, license: lic.clone(), cq: Some(qids[d.pos].clone()) });
            declared_at.insert(d.pos);
        }
    }
    let mut carried: HashMap<String, Origin> = HashMap::new();
    for o in &prev_origins {
        let cq = o.cq.clone().or_else(|| prev_idx.as_ref().and_then(|p| p.qids.get(o.pos).cloned()));
        if let Some(cq) = cq {
            carried.insert(cq, o.clone());
        }
    }
    let carry = |origins: &mut Vec<Origin>, o: &Origin, k: usize| origins.push(Origin { pos: k, item_id: o.item_id.clone(), qid: o.qid.clone(), version: o.version, license: o.license.clone(), cq: Some(qids[k].clone()) });
    // The bank's AUTHOR's copies of a credited question, exact or adapted, keep its credit; a non-author's copy keeps it only as an exact copy.
    let author_sends = bank.as_ref().is_none_or(|b| b.author_id == me);
    for (k, q) in qids.iter().enumerate() {
        let o = carried.get(q).or_else(|| if author_sends { copy_of[k].as_ref().and_then(|c| carried.get(c)) } else { None });
        if let Some(o) = o {
            if !declared_at.contains(&k) {
                carry(&mut origins, o, k);
            }
        }
    }
    // The head's origins by their key-free match hash: from the head index's `m`.
    let mut by_hash: HashMap<String, Origin> = HashMap::new();
    if let Some(m) = prev_idx.as_ref().and_then(|p| p.m.as_ref().map(|m| (p, m))) {
        for (cq, o) in &carried {
            if let Some(k) = m.0.qids.iter().position(|q| q == cq) {
                by_hash.insert(m.1[k].clone(), o.clone());
            }
        }
    }

    // Rewritten in place: ids, dates, provenance dropped (the server records the checked origins), image references from the verified blobs.
    let day = now.div_euclid(DAY_MS) * DAY_MS;
    let bank_name = raw["bank"].get("name").and_then(Value::as_str).unwrap_or("").to_string();
    {
        let rb = raw["bank"].as_object_mut().unwrap();
        rb.insert("id".into(), json!(bid));
        rb.insert("created".into(), json!(day));
        rb.insert("modified".into(), json!(day));
        rb.remove("origin");
        rb.remove("published");
        for (i, q) in rb.get_mut("questions").and_then(Value::as_array_mut).unwrap().iter_mut().enumerate() {
            let q = q.as_object_mut().unwrap();
            q.insert("id".into(), json!(qids[i]));
            q.insert("created".into(), json!(day));
            q.insert("modified".into(), json!(day));
            q.remove("origin");
            q.remove("published");
            if let (Some(s), true) = (&img_at[i], q.get("image").is_some_and(|v| !v.is_null() && v.as_bool() != Some(false))) {
                let b = &blob_info[s];
                q.insert("image".into(), json!({ "file": format!("media/{s}.{}", ext_of(&b.content_type)), "width": b.width, "height": b.height }));
            }
        }
    }
    let r_bytes = serde_json::to_vec(&raw).map_err(|_| server_error())?;
    drop(raw);
    if keyless_again {
        if let Err(CoreErr::Fail(f)) = env.core.validate_for_publish(&r_bytes, false, &[], &[]) {
            let k = keyless_positions(&f);
            if !k.is_empty() {
                return Err(keyless_question(&k, &qs_ref).into());
            }
        }
    }

    // ---- share-core: one validation (no strip), the key-free match hashes ----
    let vm = env.core.validate_meta(&sm.meta).map_err(|e| core_fail(e).map(|f| Fail::from(invalid(&f))).unwrap_or_else(|e| e))?;
    // A bank that hides its keys from viewers is checked as a hidden-key publish is, with the sender's decisions: a refusal has the
    // publish's shape and stores nothing; the decisions are recorded with the version, the stripped form itself is let go.
    let hide = if is_new { sm.hide_key } else { bank.as_ref().unwrap().hide_key == 1 };
    let d = if hide { sm.decisions.clone() } else { StripDecisions::default() };
    let opts = PublishOpts { hide_key: hide, labels: d.labels.clone(), order_acks: d.order_acks.clone(), image_ok: d.image_ok.clone(), ack_formatting: d.ack_formatting.clone() };
    let prep = match env.core.publish_prepare(&r_bytes, &opts) {
        Ok(p) => p,
        Err(e) => {
            let f = core_fail(e)?;
            return Err(if hide { strip_refusal(&f) } else { invalid(&f) }.into());
        }
    };
    let mut strip_opts: Option<String> = None;
    let mut pairs: Vec<(String, String)> = Vec::new();
    let mut viewer_form: Option<ScanForm> = None;
    if let (true, Some(st)) = (hide, &prep.stripped) {
        strip_opts = Some(decisions_json(&recorded_decisions(&d, st, n), &kept_images(&st.v.payload)));
        pairs = hash_pairs(&prep.match_hashes, &st.match_hashes);
        let vq: Vec<Json> = questions_of(&st.v.payload).into_iter().map(to_json).collect();
        viewer_form = Some(ScanForm { img_at: images_at(&vq), mh: st.match_hashes.clone(), qs: vq });
    }
    let full_qs: Vec<Value> = questions_of(&prep.full.payload).into_iter().cloned().collect();
    if prep.full.payload.get("bank").is_none() || full_qs.len() != n {
        return Err(invalid_payload("invalid", "").into());
    }
    let contents = env.core.content_hashes(&full_qs).map_err(|_| server_error())?;
    let has_images = full_qs.iter().any(|q| q.get("image").is_some_and(|i| !i.is_null())) as i64;
    // The student-data check on the validated form, and on the stripped form a bank that hides its keys serves its viewers.
    let mut forms = vec![ScanForm { qs: full_qs.iter().map(to_json).collect(), mh: prep.match_hashes.clone(), img_at: img_at.clone() }];
    forms.extend(viewer_form);
    let privacy = privacy_scan(x, env.fp, &forms, me)?;
    // The same text sent without its qid keeps the origin the head recorded for it.
    if !by_hash.is_empty() {
        let has: HashSet<usize> = origins.iter().map(|o| o.pos).collect();
        for (k, h) in prep.match_hashes.iter().enumerate() {
            if let Some(o) = by_hash.get(h) {
                if !has.contains(&k) {
                    carry(&mut origins, o, k);
                }
            }
        }
    }
    origins.sort_by_key(|o| o.pos);
    if !origins.is_empty() {
        let allowed = allowed_by_all(origins.iter().map(|o| o.license.as_str()));
        if !allowed.contains(&sm.license.as_str()) {
            let bad_pos: Vec<usize> = origins.iter().filter(|o| !allowed_by_all([o.license.as_str()]).contains(&sm.license.as_str())).map(|o| o.pos).take(100).collect();
            return Err(PoolError::new(422, "license_incompatible")
                .with("positions", json!(bad_pos))
                .with("allowed", json!(allowed))
                .with("message", json!("Some questions came from the public pool under a licence that does not allow the one chosen for this bank."))
                .into());
        }
    }

    // ---- what changed (per qid), the bytes, the records ----
    let (added, changed, removed) = diff(prev_idx.as_ref(), &qids, &contents);
    let index = serde_json::to_vec(&json!({ "qids": qids, "h": contents, "m": prep.match_hashes })).unwrap();
    let image_bytes: i64 = shas.iter().map(|s| blob_info.get(s).map(|b| b.size).unwrap_or(0)).sum();
    let bytes = prep.full.bytes.len() as i64 + index.len() as i64 + image_bytes;
    let hashes = dedup(prep.match_hashes.iter().cloned());
    // (and the served form's, where a label stripped for viewers changes it)
    let counted = dedup(hashes.iter().cloned().chain(pairs.iter().map(|p| p.1.clone())));
    let new_keys: i64 = x.conn.query_row(
        "SELECT COUNT(*) FROM json_each(?1) j WHERE NOT EXISTS (SELECT 1 FROM circle_qkeys k WHERE k.circle_id = ?2 AND k.match_hash = j.value AND k.bank_id = ?3)",
        params![serde_json::to_string(&counted).unwrap(), cid, bid],
        |r| r.get(0),
    )?;
    let title: String = if !vm.title.is_empty() { vm.title.clone() } else { clean_text(&json!(bank_name), 200).unwrap_or_default() };
    let meta = Listing { title: title.chars().take(200).collect(), subject: vm.subject.clone(), course: vm.course.clone(), grade: vm.grade.clone() };
    let vid = new_id();
    let version = if is_new { 1 } else { sm.base.unwrap() + 1 };
    let mut result = json!({ "circle_id": cid, "bank_id": bid, "version": version, "version_id": vid, "qids": qids, "added": added, "changed": changed, "removed": removed });
    let mut audit_detail = json!({ "added": added, "changed": changed, "removed": removed, "license": sm.license });
    if let Some(m) = merged_from {
        result["merged_from"] = json!(m);
        audit_detail["merged_from"] = json!(m);
    }
    if hide {
        result["keys_hidden"] = json!(true);
    }
    Ok(CommitSpec {
        kind: Kind::Send,
        me: me.into(),
        cid: cid.into(),
        bid,
        vid,
        is_new,
        base: sm.base,
        version,
        commit_id: sm.commit_id.clone(),
        request_sha,
        message: sm.message.clone(),
        license: sm.license.clone(),
        listing: Some(json!({ "title": meta.title, "subject": meta.subject, "course": meta.course, "grade": meta.grade }).to_string()),
        meta: Some(meta),
        n: n as i64,
        has_images,
        bytes,
        new_keys,
        hashes,
        shas,
        origins: origins.iter().map(|o| serde_json::to_value(o).unwrap()).collect(),
        merged_from,
        rollback_of: None,
        privacy,
        strip_opts,
        pairs,
        hide_key: Some(hide as i64),
        result,
        audit_detail,
        full_bytes: prep.full.bytes.clone(),
        index_bytes: index,
    })
}

#[allow(dead_code)]
fn _keep(_: CoreError, _: fn() -> PoolError) {
    let _ = keys_hidden_unavailable;
}

#[cfg(test)]
#[path = "circlesendprep_tests.rs"]
mod tests;
