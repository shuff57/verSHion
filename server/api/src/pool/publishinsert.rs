//! The staging half of publish.ts's "stage" section (2346-2505): the one INSERT that holds the quota, the shared-question cap and the id reservation, then the
//! bodies, the question rows and everything keyed to the publish, all written under the stage token; the row turns 'staged' only when every write succeeded,
//! and a failure discards what was written. Nothing here is visible to anyone until the commit.
use super::blobs::usage;
use super::chain::ChainLink;
use super::core::{kind_tag, questions_of};
use super::croncore::discard_staged;
use super::ctx::{PoolCtx, PoolError, Res, chunks};
use super::publishderive::Derived;
use super::publishfacts::Facts;
use super::publishhash::{Hashed, SEARCH_WORDS_MAX};
use super::publishmeta::PublishMeta;
use super::publishrewrite::Rewritten;
use super::publishstage::{Limits, PART_KEYS_MAX, Seqs, check_quotas};
use super::sqlfrag::{REMOVED_SQL, anon_chain, new_id, shared_sql, usage_sql};
use super::textutil::{search_key, title_search};
use crate::blobs::BlobStore;
use rusqlite::params;
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashSet};

const KIND_BITS: [(&str, i64); 6] = [("Mc", 1), ("Tf", 2), ("Matching", 4), ("Blank", 8), ("Short", 16), ("Multipart", 32)];

/// `{"allowed":..,"binders":..}` plus the marks readers are told (`soft`, `claimed`, `earlier`), in that order, keeping what is already there.
pub fn with_marks(aj: &str, soft: bool, claimed: bool, earlier: bool) -> String {
    if !soft && !claimed && !earlier {
        return aj.to_string();
    }
    let Ok(Value::Object(o)) = serde_json::from_str::<Value>(aj) else { return aj.to_string() };
    let mut out = aj.trim_end().trim_end_matches('}').to_string();
    for (k, on) in [("soft", soft), ("claimed", claimed), ("earlier", earlier)] {
        if on && !o.contains_key(k) {
            out.push_str(if o.is_empty() && out.ends_with('{') { "" } else { "," });
            out.push_str(&format!("\"{k}\":true"));
        }
    }
    out.push('}');
    out
}

/// The sha of a question's served image, if its `image.file` is `media/<sha>.<ext>`.
pub fn image_sha(q: &Value) -> Option<String> {
    let f = q.get("image")?.get("file")?.as_str()?;
    let rest = f.strip_prefix("media/")?;
    let (sha, ext) = rest.split_once('.')?;
    (sha.len() == 64 && sha.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) && matches!(ext, "png" | "jpg" | "gif" | "webp")).then(|| sha.to_string())
}
fn uniq(it: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    it.into_iter().filter(|s| seen.insert(s.clone())).collect()
}

/// Tags with the merged topics, lower-cased; never silently dropped.
pub fn tags_of(meta_tags: &[String], qs: &[Value]) -> BTreeSet<String> {
    let mut tags: BTreeSet<String> = meta_tags.iter().map(|t| t.to_lowercase()).collect();
    for q in qs {
        for t in q.get("topics").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
            if !t.trim().is_empty() {
                tags.insert(t.trim().to_lowercase());
            }
        }
    }
    tags
}

/// What the staged row records about the payload.
#[derive(Debug)]
pub struct Plan {
    pub kinds_mask: i64,
    pub has_images: i64,
    pub bank_name: String,
    pub title: String,
    pub schema_version: i64,
    pub tags: BTreeSet<String>,
    pub shas: Vec<String>,
    pub stripped_shas: Vec<String>,
    pub served_shas: Vec<String>,
    pub acks: String,
    pub full_bytes: i64,
    pub stripped_bytes: i64,
}

pub fn plan(m: &PublishMeta, rw: &Rewritten, h: &Hashed, is_new: bool) -> (Plan, i64) {
    let kinds_mask = h.full_qs.iter().fold(0, |acc, q| acc | KIND_BITS.iter().find(|(k, _)| *k == kind_tag(q)).map_or(0, |(_, b)| *b));
    let has_images = h.served.iter().any(|q| q.get("image").is_some_and(|i| !i.is_null())) as i64;
    let bank_name = h.prep.full.payload.get("bank").map(|b| b.get("name").map(|n| n.as_str().map_or_else(|| n.to_string(), String::from)).unwrap_or_default()).unwrap_or_default();
    let title = if rw.meta.title.is_empty() { bank_name.clone() } else { rw.meta.title.clone() };
    let st = h.prep.stripped.as_ref();
    let acks = json!({
        "labels": st.map(|s| s.label_findings.clone()).unwrap_or_default(),
        "order_acks": m.order_acks,
        "image_ok": m.image_ok,
        "ack_formatting": m.ack_formatting,
        "order_reasons": st.map(|s| s.order_reasons.clone()).unwrap_or_default(),
    })
    .to_string();
    let full_bytes = h.prep.full.bytes.len() as i64;
    let stripped_bytes = match (&h.reused, st) {
        (Some(r), _) => r.bytes.len() as i64,
        (None, Some(s)) => s.v.bytes.len() as i64,
        _ => 0,
    };
    let charge = full_bytes + stripped_bytes + if is_new { 1024 } else { 0 };
    let p = Plan {
        kinds_mask,
        has_images,
        bank_name,
        title,
        schema_version: h.prep.full.payload.get("schema_version").and_then(Value::as_i64).unwrap_or(0),
        tags: tags_of(&rw.meta.tags, &h.full_qs),
        shas: uniq(rw.images.iter().flatten().cloned()),
        stripped_shas: st.map(|s| uniq(questions_of(&s.v.payload).into_iter().filter_map(image_sha))).unwrap_or_default(),
        served_shas: vec![],
        acks,
        full_bytes,
        stripped_bytes,
    };
    (p, charge)
}

#[derive(Debug)]
pub struct Staged {
    pub token: String,
    pub full_key: String,
    pub stripped_key: Option<String>,
}

pub struct StageIn<'a> {
    pub me: &'a str,
    pub item_id: &'a str,
    pub version: i64,
    pub request_sha: &'a str,
    pub m: &'a PublishMeta,
    pub rw: &'a Rewritten,
    pub h: &'a Hashed,
    pub f: &'a Facts,
    pub d: &'a Derived,
    pub s: &'a Seqs,
    pub plan: &'a Plan,
    pub charge: i64,
    pub limits: Limits,
    pub search_words: &'a [String],
    /// The served images (what a base publish's seqs are carried for).
    pub served_shas: &'a [String],
}

/// Inserts the staging row, then everything under it. Errors map as the Worker's: another request holding the id is 409 `publish_in_progress`; else the
/// quota that stopped the INSERT is named.
pub fn stage(x: &PoolCtx, store: &BlobStore, p: &StageIn) -> Res<Staged> {
    let (m, h, pl) = (p.m, p.h, p.plan);
    let n = p.rw.n as i64;
    let token = new_id();
    let full_key = format!("items/{}/{}.json", p.item_id, new_id());
    let st = h.prep.stripped.as_ref();
    let stripped_key = st.map(|_| format!("items/{}/{}.json", p.item_id, new_id()));
    let sql = format!(
        "INSERT OR IGNORE INTO item_publishes (publish_id, item_id, author_id, base_publish_id, version, status, schema_version, hide_key, license,
           attested_at, attest_text_rev, acks_json, request_sha256, full_key, stripped_key, full_bytes, stripped_bytes,
           title, subject, course, grade, kinds_mask, question_count, has_images, created_at, stage_token, public_version)
         SELECT ?3, ?4, ?1, ?5, ?6, 'staging', ?7, ?8, ?9, ?2, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?2, ?26, ?29
          WHERE {} + ?24 <= ?25 AND NOT EXISTS (SELECT 1 FROM retired_publish_ids WHERE publish_id = ?3)
            AND {} + ?22 <= ?27 AND {} <= ?28",
        usage_sql(None),
        shared_sql("?1", "?4"),
        REMOVED_SQL
    );
    let changed = x.conn.execute(
        &sql,
        params![
            p.me,
            x.now,
            m.publish_id,
            p.item_id,
            m.base,
            p.version,
            pl.schema_version,
            m.hide_key as i64,
            m.license,
            m.attest_rev,
            pl.acks,
            p.request_sha,
            full_key,
            stripped_key,
            pl.full_bytes,
            pl.stripped_bytes,
            pl.title,
            p.rw.meta.subject,
            p.rw.meta.course,
            p.rw.meta.grade,
            pl.kinds_mask,
            n,
            pl.has_images,
            p.charge,
            p.limits.quota,
            token,
            p.limits.shared_cap,
            p.limits.removed_cap,
            h.public_version
        ],
    )?;
    if changed != 1 {
        if x.conn.query_row("SELECT 1 FROM item_publishes WHERE publish_id = ?1", [&m.publish_id], |_| Ok(())).is_ok() {
            return Err(PoolError::new(409, "publish_in_progress").header("Retry-After", "5").into());
        }
        let _ = usage(x, p.me)?;
        check_quotas(x, p.me, p.item_id, p.charge, p.rw.n, &p.limits)?;
        let qn: i64 = x.conn.query_row(&format!("SELECT {} AS n", shared_sql("?1", "?2")), params![p.me, p.item_id], |r| r.get(0))?;
        return Err(PoolError::new(422, "shared_questions_limit").with("used", json!(qn)).with("needed", json!(n)).with("limit", json!(p.limits.shared_cap)).into());
    }
    let out = Staged { token: token.clone(), full_key: full_key.clone(), stripped_key: stripped_key.clone() };
    if let Err(e) = write_rows(x, store, p, &token, &full_key, stripped_key.as_deref()) {
        let _ = discard_staged(x, store, &m.publish_id, p.item_id, &full_key, stripped_key.as_deref(), x.now);
        return Err(e);
    }
    Ok(out)
}

fn io(e: std::io::Error) -> super::ctx::Fail {
    super::ctx::Fail::Pool(PoolError::new(500, "storage_error").with("message", json!(e.to_string())))
}

fn write_rows(x: &PoolCtx, store: &BlobStore, p: &StageIn, token: &str, full_key: &str, stripped_key: Option<&str>) -> Res<()> {
    let (m, h, f, d, s, pl, rw) = (p.m, p.h, p.f, p.d, p.s, p.plan, p.rw);
    let pid = &m.publish_id;
    store.put(full_key, &h.prep.full.bytes).map_err(io)?;
    if let (Some(k), Some(st)) = (stripped_key, h.prep.stripped.as_ref()) {
        store.put(k, h.reused.as_ref().map_or(&st.v.bytes, |r| &r.bytes)).map_err(io)?;
    }
    let mine = "EXISTS (SELECT 1 FROM item_publishes WHERE publish_id = ?1 AND stage_token = ?3 AND status = 'staging')";
    let st = h.prep.stripped.as_ref();
    let opt = |v: &Option<Vec<Option<String>>>, i: usize| v.as_ref().and_then(|v| v.get(i).cloned().flatten());
    let q_rows: Vec<Value> = (0..rw.n)
        .map(|i| {
            let q = &h.full_qs[i];
            let e = &d.entries[i];
            let mk = d.marks[i];
            json!({
                "qid": rw.qids[i], "pos": i, "qkind": kind_tag(q), "difficulty": q.get("difficulty").and_then(Value::as_str),
                "mh": h.match_hashes[i], "fh": h.full_hashes[i],
                "sh": h.reused.as_ref().map(|r| r.hashes[i].clone()).or_else(|| st.map(|s| s.stripped_hashes[i].clone())),
                "fc": h.full_content[i], "kf": h.keyfree[i], "st": h.stems[i],
                "sc": h.stripped_content.as_ref().map(|v| v[i].clone()),
                "nf": h.near_full[i], "ns": opt(&h.near_stripped, i), "bf": h.bare_full[i],
                "sh2": f.short_q[i] as i64,
                "tk": json!(h.tokens[i]).to_string(), "bd": json!(h.bands[i]).to_string(),
                "bs": opt(&h.bare_stripped, i), "nk": h.near_kf[i],
                "chain": serde_json::to_string(&e.chain).unwrap_or_else(|_| "[]".into()),
                "ls": f.lineage[i].unwrap_or(s.q_seq[i]),
                "aj": with_marks(&d.allowed_json[i], mk.soft, mk.claimed, mk.earlier),
                "bh": f.body[i], "kn": f.bk_norm[i], "ks": f.bk_stem[i],
                "lr": json!(if f.lineage[i].is_some() { &f.lineage_root[i] } else { &h.tokens[i] }).to_string(),
                "bt": json!(h.body_tokens[i]).to_string(), "tsk": json!(h.tok_sk[i]).to_string(), "bsk": json!(h.body_sk[i]).to_string(), "kb": json!(h.k_bands[i]).to_string(),
                "kfl": f.bk_full[i], "ke": f.bk_fields[i].0, "ka": f.bk_fields[i].1,
                "oi": e.origin_item, "oq": e.origin_qid, "mod": e.modified.map(|b| b as i64),
                "seq": s.q_seq[i], "us": json!(s.unit_seqs[i]).to_string(),
            })
        })
        .collect();
    let tx = x.conn.unchecked_transaction()?;
    for ch in chunks(&q_rows, 150) {
        tx.execute(
            &format!(
                "INSERT INTO item_questions (publish_id, qid, pos, qkind, difficulty, match_hash, full_sha256, stripped_sha256, full_content, stripped_content, keyfree_content, near_full, near_stripped, near_kf, near_bare_full, near_bare_stripped, short, stem_tokens, stem_bands, stem_hash, chain_json, origin_item_id, origin_qid, modified, seq, lineage_seq, allowed_json, body_hash, bk_norm, bk_full, bk_expl, bk_ans, bk_stem, lineage_root, body_tokens, tok_sk, body_sk, kbands, unit_seqs)
                 SELECT ?1, j.value ->> '$.qid', j.value ->> '$.pos', j.value ->> '$.qkind', j.value ->> '$.difficulty', j.value ->> '$.mh', j.value ->> '$.fh',
                        j.value ->> '$.sh', j.value ->> '$.fc', j.value ->> '$.sc', j.value ->> '$.kf', j.value ->> '$.nf', j.value ->> '$.ns', j.value ->> '$.nk', j.value ->> '$.bf', j.value ->> '$.bs', j.value ->> '$.sh2', j.value ->> '$.tk', j.value ->> '$.bd', j.value ->> '$.st', {}, j.value ->> '$.oi', j.value ->> '$.oq', j.value ->> '$.mod', j.value ->> '$.seq', j.value ->> '$.ls', j.value ->> '$.aj', j.value ->> '$.bh', j.value ->> '$.kn', j.value ->> '$.kfl', j.value ->> '$.ke', j.value ->> '$.ka', j.value ->> '$.ks', j.value ->> '$.lr', j.value ->> '$.bt', j.value ->> '$.tsk', j.value ->> '$.bsk', j.value ->> '$.kb', j.value ->> '$.us'
                   FROM json_each(?2) j WHERE {mine}",
                anon_chain("j.value ->> '$.chain'")
            ),
            params![pid, json!(ch).to_string(), token],
        )?;
    }
    // Question-text search: the words of the SERVED stems only, plus the title, subject and tags.
    let tail = |s: &str| search_key(s).split(' ').map(String::from).collect::<Vec<_>>();
    let mut meta_words: Vec<String> = title_search(&pl.title).split(' ').map(String::from).collect();
    meta_words.extend(tail(&rw.meta.subject));
    meta_words.extend(pl.tags.iter().flat_map(|t| tail(t)));
    let words = uniq(p.search_words.iter().cloned().chain(meta_words.into_iter().filter(|w| !w.is_empty() && w.chars().count() <= 40)));
    let words: Vec<String> = words.into_iter().take(SEARCH_WORDS_MAX + 500).collect();
    for ch in chunks(&words, 500) {
        tx.execute(&format!("INSERT OR IGNORE INTO served_words (publish_id, word) SELECT ?1, value FROM json_each(?2) WHERE {mine}"), params![pid, json!(ch).to_string(), token])?;
    }
    let tags: Vec<String> = pl.tags.iter().cloned().collect();
    for ch in chunks(&tags, 200) {
        let pairs: Vec<Value> = ch.iter().map(|t| json!([t, search_key(t)])).collect();
        tx.execute(&format!("INSERT OR IGNORE INTO item_tags (publish_id, tag, fold) SELECT ?1, j.value ->> '$[0]', j.value ->> '$[1]' FROM json_each(?2) j WHERE {mine}"), params![pid, json!(pairs).to_string(), token])?;
    }
    let standards: BTreeSet<String> = rw.meta.standards.iter().map(|s| s.split_whitespace().collect::<Vec<_>>().join(" ").to_uppercase()).filter(|s| !s.is_empty()).collect();
    if !standards.is_empty() {
        tx.execute(&format!("INSERT OR IGNORE INTO item_standards (publish_id, code) SELECT ?1, value FROM json_each(?2) WHERE {mine}"), params![pid, json!(standards).to_string(), token])?;
    }
    let served: HashSet<&str> = p.served_shas.iter().map(String::as_str).collect();
    for (form, list) in [("full", &pl.shas), ("stripped", &pl.stripped_shas)] {
        if list.is_empty() {
            continue;
        }
        let rows: Vec<Value> = list.iter().map(|sha| json!({ "s": sha, "q": s.blob_seq(&served, sha) })).collect();
        tx.execute(
            &format!("INSERT OR IGNORE INTO item_blobs (publish_id, form, sha256, seq) SELECT ?1, ?4, j.value ->> '$.s', j.value ->> '$.q' FROM json_each(?2) j WHERE {mine}"),
            params![pid, json!(rows).to_string(), token, form],
        )?;
    }
    if !rw.new_qids.is_empty() {
        tx.execute(
            &format!("INSERT OR IGNORE INTO question_ids (qid, item_id, first_publish_id, created_at, seq) SELECT value, ?4, ?1, ?5, ?6 FROM json_each(?2) WHERE {mine}"),
            params![pid, json!(rw.new_qids).to_string(), token, p.item_id, x.now, s.new_seq],
        )?;
    }
    let mut pk: Vec<Value> = vec![];
    for (pos, list) in f.part_keys.iter().enumerate() {
        for (part, key) in list {
            let mut o = json!({ "pos": pos, "part": part, "key": key });
            if key.starts_with("s:") {
                let sk = f.part_stems.get(pos).and_then(|v| v.get(*part));
                o["tk"] = json!(json!(sk.map(|s| s.tokens.clone()).unwrap_or_default()).to_string());
                o["bd"] = json!(json!(sk.map(|s| s.bands.clone()).unwrap_or_default()).to_string());
                o["kb"] = json!(json!(f.part_k_bands.get(pos).and_then(|v| v.get(*part)).cloned().unwrap_or_default()).to_string());
            }
            pk.push(o);
        }
    }
    for (pos, list) in f.unit_keys.iter().enumerate() {
        for u in list {
            pk.push(json!({ "pos": pos, "part": u.part, "key": u.key }));
        }
    }
    pk.truncate(PART_KEYS_MAX);
    for ch in chunks(&pk, 300) {
        tx.execute(
            &format!("INSERT OR IGNORE INTO item_part_keys (publish_id, pos, part, key, tokens, bands, kbands) SELECT ?1, j.value ->> '$.pos', j.value ->> '$.part', j.value ->> '$.key', j.value ->> '$.tk', j.value ->> '$.bd', j.value ->> '$.kb' FROM json_each(?2) j WHERE {mine}"),
            params![pid, json!(ch).to_string(), token],
        )?;
    }
    tx.execute("UPDATE item_publishes SET status = 'staged' WHERE publish_id = ?1 AND status = 'staging' AND stage_token = ?2", params![pid, token])?;
    tx.commit()?;
    Ok(())
}

#[allow(dead_code)]
fn _chain_is_serialisable(_: &ChainLink) {}

#[cfg(test)]
#[path = "publishinsert_tests.rs"]
mod tests;
