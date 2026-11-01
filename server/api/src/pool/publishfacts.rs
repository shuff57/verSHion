//! The facts a publish's licence and provenance steps read (worker/src/pool/publish.ts `publishOnce`, "licence and provenance" up to the payload trees): the
//! previous rows of kept questions, every block key of the SERVED form, the lineage of each kept qid, the author's own record, the short-statement and
//! text-unit marks, and the circle-material claim. Nothing here decides an outcome; the evidence step does.
use super::authctx::{AuthorCtx, ContentSubject, author_context, own_content_first};
use super::circlecheck::{CircleClaim, check_circle_material};
use super::core::kind_tag;
use super::ctx::{PoolCtx, Res};
use super::cronpublish::stem_backfill_author;
use super::fingerprint::Fingerprinter;
use super::fuzzy::FUZZY_JACCARD;
use super::publishhash::{Hashed, to_json};
use super::publishrewrite::{Rewritten, js_ws};
use super::qhash::{field_keys, jaccard, near_hash, norm_skeleton_key, norm_text, part_keys_of, stem_bands, stem_key, stem_tokens, text_field_keys, unit_keys_of, UnitKey};
use super::qjson::Json;
use super::publishmeta::PublishMeta;
use super::sqlfrag::is_id;
use crate::blobs::BlobStore;
use rusqlite::params;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

/// A qid's lineage holds only while its stem stays this close (token Jaccard) to the lineage root.
pub const LINEAGE_ROOT_JACCARD: f64 = 0.6;
/// Words in a short statement's stem at most.
pub const SHORT_WORDS: usize = 8;
/// Words of the key (a Short sample, a Blank's accepted answers) or of an Mc's stem and choices together.
pub const SHORT_KEY_WORDS: usize = 12;
/// Words an explanation or model answer needs before copying it alone is a copy.
pub const LIFT_WORDS: usize = 25;

fn js_words(t: Option<&Value>) -> usize {
    t.and_then(Value::as_str).map_or(0, |s| s.split(js_ws).filter(|p| !p.is_empty()).count())
}
fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) | Some(Value::Bool(false)) => false,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0),
        _ => true,
    }
}

/// A short statement, decided on the SERVED form (key-free, so nothing about a hidden key decides it): no explanation, image or figure, and by kind a stem of
/// at most `SHORT_WORDS` words (Tf; Short and Blank with a served key of at most `SHORT_KEY_WORDS`) or an Mc whose stem and choices total at most
/// `SHORT_KEY_WORDS`. Matching and Multipart: never.
pub fn is_short(q: &Value) -> bool {
    if q.get("explanation").and_then(Value::as_str).is_some_and(|e| !e.trim().is_empty()) || truthy(q.get("image")) || truthy(q.get("figure")) || truthy(q.get("keyless")) {
        return false;
    }
    let k = kind_tag(q);
    let kind = q.get("kind").and_then(|x| x.get(&k));
    let stem = js_words(q.get("stem"));
    match k.as_str() {
        "Tf" => stem <= SHORT_WORDS,
        "Short" => {
            let sample = kind.and_then(|s| s.get("sample")).and_then(Value::as_str);
            stem <= SHORT_WORDS && sample.is_some_and(|s| !s.trim().is_empty() && js_words(Some(&json!(s))) <= SHORT_KEY_WORDS)
        }
        "Blank" => {
            let Some(acc) = kind.and_then(|s| s.get("accept")).and_then(Value::as_array).filter(|a| !a.is_empty()) else { return false };
            let n: usize = acc.iter().flat_map(|x| x.as_array().map_or_else(|| vec![x], |inner| inner.iter().collect())).map(|t| js_words(Some(t))).sum();
            stem <= SHORT_WORDS && n > 0 && n <= SHORT_KEY_WORDS
        }
        "Mc" => {
            let Some(ans) = kind.and_then(|s| s.get("answers")).and_then(Value::as_array) else { return false };
            stem + ans.iter().map(|a| js_words(a.get("text"))).sum::<usize>() <= SHORT_KEY_WORDS
        }
        _ => false,
    }
}

/// Words of a question as served: stem, choices, columns, served answers and explanation (image, figure, topics, difficulty and ids aside).
pub fn served_word_count(q: &Value) -> usize {
    fn walk(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::String(s) => out.push(s.clone()),
            Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            Value::Object(o) => {
                for (k, x) in o {
                    if !["id", "created", "modified", "origin", "published", "image", "figure", "topics", "difficulty"].contains(&k.as_str()) {
                        walk(x, out);
                    }
                }
            }
            _ => {}
        }
    }
    let mut texts = vec![];
    walk(q, &mut texts);
    texts.join(" ").split(js_ws).filter(|p| !p.is_empty()).count()
}

/// The previous publish's row of a kept qid.
#[derive(Debug, Clone, Default)]
pub struct PrevRow {
    pub chain_json: String,
    pub origin_item_id: Option<String>,
    pub origin_qid: Option<String>,
    pub full_content: String,
    pub keyfree_content: String,
    pub match_hash: String,
    pub seq: i64,
    pub lineage_seq: Option<i64>,
    pub stem_tokens: Option<String>,
    pub allowed_json: Option<String>,
    pub body_hash: Option<String>,
    pub lineage_root: Option<String>,
    pub unit_seqs: Option<String>,
}

#[derive(Debug)]
pub struct PartSketch {
    pub tokens: Vec<String>,
    pub bands: Vec<String>,
}

#[derive(Debug, Default)]
pub struct Facts {
    pub prev: HashMap<String, PrevRow>,
    /// (item, qid) origins carried by kept questions.
    pub carried_refs: Vec<(String, String)>,
    /// Per position: the origin sources to look up (a declared one, a kept qid's recorded one).
    pub refs: Vec<Vec<(String, String)>>,
    pub bk_norm: Vec<String>,
    pub bk_stem: Vec<String>,
    pub bk_fields: Vec<(Option<String>, Option<String>)>,
    /// Per position: the full form's explanation key, kept in the author's durable stem record.
    pub stem_record: Vec<(usize, String)>,
    pub bk_full: Vec<String>,
    pub part_keys: Vec<Vec<(usize, String)>>,
    pub unit_keys: Vec<Vec<UnitKey>>,
    pub part_stems: Vec<Vec<PartSketch>>,
    pub part_k_bands: Vec<Vec<Vec<String>>>,
    pub block_keys: Vec<String>,
    /// Everything but the stem, as served.
    pub body: Vec<String>,
    pub keywork: Vec<bool>,
    pub lineage_root: Vec<Vec<String>>,
    pub lineage: Vec<Option<i64>>,
    pub actx: AuthorCtx,
    pub own_first: HashMap<usize, i64>,
    pub short_q: Vec<bool>,
    pub circle_claimed: Vec<String>,
    pub short40: Vec<bool>,
    pub served_text: Vec<Vec<String>>,
    pub hidden_text: Vec<Vec<String>>,
    pub img_at: Vec<Option<String>>,
}

fn text_keys(fp: &Fingerprinter, q: &Value) -> Vec<String> {
    let mut texts: Vec<Option<&Value>> = vec![q.get("explanation")];
    let kind = q.get("kind");
    texts.push(kind.and_then(|k| k.get("Short")).and_then(|s| s.get("sample")));
    for p in kind.and_then(|k| k.get("Multipart")).and_then(|m| m.get("parts")).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
        texts.push(p.get("explanation"));
        texts.push(p.get("kind").and_then(|k| k.get("Short")).and_then(|s| s.get("sample")));
    }
    let mut out: Vec<String> = vec![];
    for t in texts.into_iter().flatten().filter_map(Value::as_str) {
        if norm_text(t).split(' ').filter(|w| !w.is_empty()).count() >= LIFT_WORDS {
            for k in text_field_keys(fp, t) {
                if !out.contains(&k) {
                    out.push(k);
                }
            }
        }
    }
    out
}

fn dedup(xs: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    xs.into_iter().filter(|x| seen.insert(x.clone())).collect()
}

/// Gathers the facts. A real publish first repairs the author's legacy stem records (bounded), so an earlier hidden version's explanation key is known
/// before the evidence rules read it; a dry run writes nothing.
#[allow(clippy::too_many_arguments)]
pub fn gather(x: &PoolCtx, store: &BlobStore, fp: &Fingerprinter, me: &str, base_of: Option<&str>, is_new: bool, dry: bool, m: &PublishMeta, rw: &Rewritten, h: &Hashed, claim: Option<&CircleClaim>) -> Res<Facts> {
    let n = rw.n;
    let mut prev: HashMap<String, PrevRow> = HashMap::new();
    if let (false, Some(base)) = (is_new, base_of) {
        let kept: Vec<&String> = rw.qids.iter().enumerate().filter(|(i, q)| rw.sent[*i].as_ref() == Some(*q)).map(|(_, q)| q).collect();
        if !kept.is_empty() {
            let mut st = x.conn.prepare(
                "SELECT qid, chain_json, origin_item_id, origin_qid, full_content, keyfree_content, match_hash, seq, lineage_seq, stem_tokens, allowed_json, body_hash, lineage_root, unit_seqs
             FROM item_questions WHERE publish_id = ?1 AND qid IN (SELECT value FROM json_each(?2))",
            )?;
            let rows = st.query_map(params![base, json!(kept).to_string()], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    PrevRow {
                        chain_json: r.get(1)?,
                        origin_item_id: r.get(2)?,
                        origin_qid: r.get(3)?,
                        full_content: r.get(4)?,
                        keyfree_content: r.get(5)?,
                        match_hash: r.get(6)?,
                        seq: r.get(7)?,
                        lineage_seq: r.get(8)?,
                        stem_tokens: r.get(9)?,
                        allowed_json: r.get(10)?,
                        body_hash: r.get(11)?,
                        lineage_root: r.get(12)?,
                        unit_seqs: r.get(13)?,
                    },
                ))
            })?;
            for r in rows {
                let (q, row) = r?;
                prev.insert(q, row);
            }
        }
    }
    let carried_refs: Vec<(String, String)> = prev.values().filter_map(|p| Some((p.origin_item_id.clone().filter(|s| !s.is_empty())?, p.origin_qid.clone().filter(|s| !s.is_empty())?))).collect();
    let refs: Vec<Vec<(String, String)>> = (0..n)
        .map(|i| {
            let mut out = vec![];
            if let Some(d) = &rw.declared[i] {
                if is_id(&d.item) && is_id(&d.qid) {
                    out.push((d.item.clone(), d.qid.clone()));
                }
            }
            if let Some(p) = prev.get(&rw.qids[i]) {
                if let (Some(a), Some(b)) = (p.origin_item_id.as_ref().filter(|s| !s.is_empty()), p.origin_qid.as_ref().filter(|s| !s.is_empty())) {
                    out.push((a.clone(), b.clone()));
                }
            }
            out
        })
        .collect();

    let served_j: Vec<Json> = h.served.iter().map(to_json).collect();
    let full_j: Vec<Json> = h.full_qs.iter().map(to_json).collect();
    let hidden = h.prep.stripped.is_some();
    let bk_norm: Vec<String> = served_j.iter().map(|q| norm_skeleton_key(fp, q)).collect();
    let bk_stem: Vec<String> = served_j.iter().map(|q| stem_key(fp, q)).collect();
    let bk_fields: Vec<(Option<String>, Option<String>)> = served_j.iter().map(|q| field_keys(fp, q)).collect();
    let bk_fields_full: Vec<(Option<String>, Option<String>)> = if hidden { full_j.iter().map(|q| field_keys(fp, q)).collect() } else { bk_fields.clone() };
    let stem_record: Vec<(usize, String)> = bk_fields_full.iter().enumerate().map(|(i, f)| (i, f.0.clone().unwrap_or_default())).collect();
    let bk_full: Vec<String> = if hidden {
        h.near_stripped.as_ref().map(|v| v.iter().map(|x| format!("f:{}", x.as_deref().unwrap_or("null"))).collect()).unwrap_or_default()
    } else {
        h.near_full.iter().map(|x| format!("f:{x}")).collect()
    };
    let part_keys: Vec<Vec<(usize, String)>> = served_j.iter().map(|q| part_keys_of(fp, q)).collect();
    let unit_keys: Vec<Vec<UnitKey>> = served_j.iter().map(|q| unit_keys_of(fp, q)).collect();
    let part_stems: Vec<Vec<PartSketch>> = served_j
        .iter()
        .map(|q| {
            let parts = q.get("kind").and_then(|k| k.get("Multipart")).and_then(|m| m.get("parts")).map(Json::as_arr).unwrap_or(&[]);
            parts
                .iter()
                .map(|p| {
                    let t = stem_tokens(&p.get("stem").cloned().unwrap_or(Json::Null));
                    PartSketch { bands: stem_bands(&t), tokens: t }
                })
                .collect()
        })
        .collect();
    let mut memo: HashMap<String, String> = HashMap::new();
    let part_k_bands: Vec<Vec<Vec<String>>> = part_stems.iter().map(|ps| ps.iter().map(|p| fp.keyed_bands(&p.bands, &mut memo)).collect()).collect();
    let mut keys: Vec<String> = h.match_hashes.clone();
    keys.extend(bk_norm.iter().cloned());
    keys.extend(bk_stem.iter().cloned());
    keys.extend(bk_full.iter().cloned());
    for f in &bk_fields {
        keys.extend(f.0.iter().cloned());
        keys.extend(f.1.iter().cloned());
    }
    for pk in &part_keys {
        keys.extend(pk.iter().map(|(_, k)| k.clone()));
    }
    let block_keys = dedup(keys);
    let body: Vec<String> = served_j
        .iter()
        .map(|q| {
            let mut q = q.clone();
            if let Json::Obj(o) = &mut q {
                match o.iter_mut().find(|(k, _)| k == "stem") {
                    Some(e) => e.1 = Json::s(""),
                    None => o.push(("stem".into(), Json::s(""))),
                }
            }
            near_hash(fp, &q, false)
        })
        .collect();
    let keywork: Vec<bool> = h
        .full_qs
        .iter()
        .map(|q| {
            kind_tag(q) == "Short"
                && match q.get("kind").and_then(|k| k.get("Short")).and_then(|s| s.get("sample")) {
                    None | Some(Value::Null) => false,
                    Some(Value::String(s)) => !s.trim().is_empty(),
                    Some(_) => true,
                }
        })
        .collect();
    let prev_tokens = |p: &PrevRow| serde_json::from_str::<Vec<String>>(p.stem_tokens.as_deref().unwrap_or("[]")).unwrap_or_default();
    let lineage_root: Vec<Vec<String>> = rw
        .qids
        .iter()
        .enumerate()
        .map(|(i, q)| match prev.get(q) {
            Some(p) => match p.lineage_root.as_deref().filter(|s| !s.is_empty()) {
                Some(r) => serde_json::from_str(r).unwrap_or_default(),
                None => prev_tokens(p),
            },
            None => h.tokens[i].clone(),
        })
        .collect();
    let lineage: Vec<Option<i64>> = rw
        .qids
        .iter()
        .enumerate()
        .map(|(i, q)| {
            let p = prev.get(q)?;
            if p.body_hash.as_deref() != Some(body[i].as_str()) {
                return None;
            }
            let cont = (p.match_hash == h.match_hashes[i] || jaccard(&prev_tokens(p), &h.tokens[i]) >= FUZZY_JACCARD) && jaccard(&lineage_root[i], &h.tokens[i]) >= LINEAGE_ROOT_JACCARD;
            cont.then(|| p.lineage_seq.unwrap_or(p.seq))
        })
        .collect();
    if !dry {
        stem_backfill_author(x, store, fp, me)?;
    }
    let actx = author_context(x, me, &h.match_hashes)?;
    let subj: Vec<ContentSubject> = (0..n)
        .map(|i| ContentSubject {
            n: if m.hide_key { h.near_stripped.as_ref().and_then(|v| v.get(i)).and_then(|x| x.as_deref()) } else { Some(h.near_full[i].as_str()) },
            kf: &h.near_kf[i],
            keywork: keywork[i],
        })
        .collect();
    let own_first = own_content_first(x, me, &subj)?;
    let short_q: Vec<bool> = h.served.iter().map(is_short).collect();
    let hashes_at: Vec<Vec<String>> = (0..n).map(|i| vec![h.match_hashes[i].clone(), h.prep.match_hashes[i].clone()]).collect();
    let circle_claimed = check_circle_material(x, me, &hashes_at, &short_q, claim)?;
    let short40: Vec<bool> = h.served.iter().map(|q| served_word_count(q) <= 40).collect();
    let served_text: Vec<Vec<String>> = h.served.iter().map(|q| text_keys(fp, q)).collect();
    let hidden_text: Vec<Vec<String>> = if hidden {
        h.full_qs.iter().enumerate().map(|(i, q)| text_keys(fp, q).into_iter().filter(|k| !served_text[i].contains(k)).collect()).collect()
    } else {
        vec![vec![]; n]
    };
    let img_at = super::circlesendmeta::images_at(&served_j);
    Ok(Facts {
        prev,
        carried_refs,
        refs,
        bk_norm,
        bk_stem,
        bk_fields,
        stem_record,
        bk_full,
        part_keys,
        unit_keys,
        part_stems,
        part_k_bands,
        block_keys,
        body,
        keywork,
        lineage_root,
        lineage,
        actx,
        own_first,
        short_q,
        circle_claimed,
        short40,
        served_text,
        hidden_text,
        img_at,
    })
}

#[cfg(test)]
#[path = "publishfacts_tests.rs"]
mod tests;
