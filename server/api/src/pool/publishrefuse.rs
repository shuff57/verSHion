//! What follows the per-question derivation (worker/src/pool/publish.ts `publishOnce`, 1999-2155): a question that credits nobody but equals, or nearly equals,
//! what another author published visibly and earlier is credited as an earlier publication; then the two `license_incompatible` refusals (the hidden-material
//! one only its author sees, and the general one naming what binds each question); then the questions flagged `origin_suspect` for the admin.
use super::chain::ChainLink;
use super::ctx::{Fail, PoolCtx, PoolError, Res};
use super::fuzzynear::{PubPos, public_near_earlier};
use super::hit::stem_hits;
use super::license::allowed_for_bank;
use super::publishderive::Derived;
use super::publishearlier::{Want, earlier_candidates, earlier_sql};
use super::publishevid::Evid;
use super::publishfacts::Facts;
use super::publishhash::Hashed;
use super::publishmeta::PublishMeta;
use super::publishrewrite::Rewritten;
use super::sqlfrag::{is_id, visible_to_others};
use rusqlite::params;
use serde_json::{Value, json};
use std::collections::HashMap;

fn push_unique(v: &mut Vec<usize>, x: usize) {
    if !v.contains(&x) {
        v.push(x);
    }
}
fn min_pos(xs: impl IntoIterator<Item = Option<i64>>) -> Option<i64> {
    xs.into_iter().flatten().filter(|x| *x > 0).min()
}

struct Wanted {
    i: usize,
    n: Vec<String>,
    first: Option<i64>,
    loose: Option<i64>,
    expl: Option<String>,
    ans: Option<String>,
    lexpl: Option<String>,
    lans: Option<String>,
}

/// A question that credits nobody but equals (licence sense) what another author published, visibly, earlier is credited as an "earlier identical publication"
/// (it binds downstream copies like any link, never this publish), and readers see `license_check` unverified_earlier_match. Public data only; short statements
/// and common questions are left alone. Compared in every served form, never through a hidden key.
#[allow(clippy::too_many_arguments)]
pub fn earlier_identical(x: &PoolCtx, me: &str, m: &PublishMeta, rw: &Rewritten, h: &Hashed, f: &Facts, e: &Evid, d: &mut Derived) -> Res<()> {
    let hidden = h.prep.stripped.is_some();
    let mine = |i: usize| -> Vec<String> {
        let mut out: Vec<String> = vec![];
        let served = if hidden { h.near_stripped.as_ref().and_then(|v| v.get(i).cloned().flatten()) } else { Some(h.near_full[i].clone()) };
        let bare = if hidden { h.bare_stripped.as_ref().and_then(|v| v.get(i).cloned().flatten()) } else { Some(h.bare_full[i].clone()) };
        for s in [served, Some(h.near_kf[i].clone()), bare].into_iter().flatten() {
            if !out.contains(&s) {
                out.push(s);
            }
        }
        out
    };
    // "Earlier" than the publisher's own first publish of this question (its match_hash, its content, its lineage or its own near-rewordings).
    let early_first = |i: usize| min_pos([e.fuzzy_first(f, i), f.actx.first_at.get(&h.match_hashes[i]).copied()]);
    // Her earliest own version with the same choices even when this one ADDS an explanation or answer: what counts against a source that lacks the addition.
    let loose_first = |i: usize| super::fuzzy::min_first(early_first(i), e.own_loose.get(&i).map(|l| l.seq));
    let want: Vec<Wanted> = (0..rw.n)
        .filter(|i| d.entries[*i].chain.is_empty() && !f.short_q[*i] && !f.actx.common.contains(&h.match_hashes[*i]))
        .filter_map(|i| {
            let n = mine(i);
            (!n.is_empty()).then(|| Wanted {
                i,
                n,
                first: early_first(i),
                loose: loose_first(i),
                expl: f.bk_fields[i].0.clone(),
                ans: f.bk_fields[i].1.clone(),
                lexpl: e.own_loose.get(&i).and_then(|l| l.expl.clone()),
                lans: e.own_loose.get(&i).and_then(|l| l.ans.clone()),
            })
        })
        .collect();
    // Candidates are QUESTIONS, never whole publishes; each near hash is followed once per chunk, and a position's test runs on its few candidates.
    for wch in want.chunks(1000) {
        let wants: Vec<Want> = wch.iter().map(|w| Want { n: w.n.clone(), expl: w.expl.clone(), ans: w.ans.clone() }).collect();
        let cands = earlier_candidates(x.conn, me, &wants)?;
        let positions: Vec<Value> = wch
            .iter()
            .zip(&cands)
            .map(|(w, c)| json!({ "i": w.i, "c": c, "n": w.n, "expl": w.expl, "ans": w.ans, "lexpl": w.lexpl, "lans": w.lans, "first": w.first, "loose": w.loose }))
            .collect();
        let found: Vec<(usize, Option<String>)> = {
            let mut st = x.conn.prepare(&earlier_sql())?;
            let rows = st.query_map(params![me, Value::Array(positions).to_string()], |r| Ok((r.get::<_, i64>(0)? as usize, r.get::<_, Option<String>>(1)?)))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        for (pos, mm) in found {
            let Some(mm) = mm.filter(|s| !s.is_empty()) else { continue };
            let l: ChainLink = serde_json::from_str(&mm).unwrap_or_default();
            // An item the publisher attests independence of stays credited as a possible source.
            let ind = m.independent.contains(&l.item_id);
            d.entries[pos].chain = vec![ChainLink { modified: false, earlier_match: Some(true), possible_source: ind.then_some(true), ..l }];
            if ind {
                d.marks[pos].claimed = true;
            } else {
                d.marks[pos].earlier = true;
            }
            push_unique(&mut d.soft_only, pos);
        }
        // ... and a near-rewording of a served stem of the same kind (one added sentence, a changed choice, a distractor more or less): credited the same way, marked as modified.
        let served_kind = |i: usize| super::core::kind_tag(&h.served[i]);
        let left: Vec<PubPos> = want
            .iter()
            .filter(|w| d.entries[w.i].chain.is_empty())
            .map(|w| PubPos {
                pos: w.i,
                tokens: h.tokens[w.i].clone(),
                bands: h.bands[w.i].clone(),
                qkind: served_kind(w.i),
                first: w.first,
                body: Some(h.body_tokens[w.i].clone()),
                loose_first: w.loose,
                loose_expl: w.lexpl.clone(),
                loose_ans: w.lans.clone(),
                expl: w.expl.clone(),
                ans: w.ans.clone(),
            })
            .collect();
        let (near, cut) = public_near_earlier(x.conn, me, &left)?;
        // A question whose earlier candidates were cut is flagged; the earliest examined is credited.
        let mut cut: Vec<usize> = cut.into_iter().collect();
        cut.sort_unstable();
        for k in cut {
            push_unique(&mut d.overflowed, k);
        }
        let mut near: Vec<_> = near.into_iter().collect();
        near.sort_by_key(|(p, _)| *p);
        for (pos, l) in near {
            let ind = m.independent.contains(&l.item_id);
            d.entries[pos].chain = vec![ChainLink {
                item_id: l.item_id,
                qid: l.qid,
                version: l.version,
                license: l.license,
                author_name: l.author_name,
                modified: true,
                earlier_match: Some(true),
                possible_source: ind.then_some(true),
                ..Default::default()
            }];
            if ind {
                d.marks[pos].claimed = true;
            } else {
                d.marks[pos].earlier = true;
            }
            push_unique(&mut d.soft_only, pos);
        }
    }
    Ok(())
}

fn first_n<T: Clone>(v: &[T], n: usize) -> Vec<T> {
    v.iter().take(n).cloned().collect()
}

/// The `license_incompatible` refusals. A hidden-material refusal (only the author sees it: she knows her hidden key and what she was shown) comes only when
/// nothing else is incompatible; otherwise the general one names, per question, the licences it allows and the items that bind it.
pub fn refuse(x: &PoolCtx, me: &str, m: &PublishMeta, d: &Derived) -> Res<()> {
    let items = |ids: &[String]| -> rusqlite::Result<HashMap<String, (String, String)>> {
        let mut out = HashMap::new();
        if ids.is_empty() {
            return Ok(out);
        }
        // Only items visible to others are named: an unlisted, held or removed source is a neutral entry (it still binds; its id, title and author are not shown).
        let mut st = x.conn.prepare(&format!(
            "SELECT i.id, i.title, COALESCE(u.display_name, 'deleted teacher') FROM items i JOIN users u ON u.id = i.author_id WHERE i.id IN (SELECT value FROM json_each(?1)) AND {}",
            visible_to_others()
        ))?;
        for r in st.query_map([json!(ids).to_string()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))? {
            let (id, title, author) = r?;
            out.insert(id, (title, author));
        }
        Ok(out)
    };
    if d.incompatible.is_empty() && !d.hidden_refused.is_empty() {
        let ids: Vec<String> = d.hidden_from.iter().map(|(k, _)| k.clone()).filter(|k| is_id(k)).take(50).collect();
        let shown = items(&ids)?;
        let sources: Vec<Value> = ids
            .iter()
            .map(|id| {
                let lics = d.hidden_from.iter().find(|(k, _)| k == id).map(|(_, v)| v.clone()).unwrap_or_default();
                match shown.get(id) {
                    Some((title, _)) => json!({ "item_id": id, "title": title, "link": format!("/items/{id}"), "licenses": lics }),
                    None => json!({ "item_id": null, "title": "an item no longer listed", "licenses": lics }),
                }
            })
            .collect();
        let named: Vec<String> = sources
            .iter()
            .map(|s| format!("\"{}\" ({})", s["title"].as_str().unwrap_or(""), s["licenses"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")).unwrap_or_default()))
            .collect();
        return Err(PoolError::new(422, "license_incompatible")
            .with("positions", json!(first_n(&d.hidden_refused, 100)))
            .with("hidden_material", json!(true))
            .with("sources", json!(sources))
            .with("message", json!(format!("The hidden explanation or model answer of these questions is material you were shown in {}, under a licence that does not allow this one.", named.join("; "))))
            .into());
    }
    if d.incompatible.is_empty() {
        return Ok(());
    }
    // Which questions force which licences, and which items bind them (so an honest author can say "I wrote this independently" of exactly those, or split the bank).
    let mut named: Vec<String> = vec![];
    for pos in d.incompatible.iter().take(100) {
        for id in d.binders[*pos].iter().cloned().chain(d.true_bound[*pos].iter().map(|(_, l)| l.item_id.clone())) {
            if is_id(&id) && !named.contains(&id) {
                named.push(id);
            }
        }
    }
    named.truncate(200);
    let info = items(&named)?;
    // A binder this account was earlier SHOWN under the licence it asks for (the author relicensed it since): say so, and what to do (CC grants are irrevocable: the
    // version received under that licence stays usable; a later version is bound by its own licence).
    let mut relicensed: Vec<Value> = vec![];
    let mut relicensed_now = String::new();
    if !named.is_empty() {
        let mut st = x.conn.prepare(
            "SELECT s.item_id, json_group_array(DISTINCT l.value) AS earlier, i.license AS now FROM json_each(?2) it CROSS JOIN served s ON s.item_id = it.value AND s.reader_id = ?1
             JOIN items i ON i.id = s.item_id, json_each(s.licenses_json) l
            WHERE s.void_at IS NULL GROUP BY s.item_id",
        )?;
        for r in st.query_map(params![me, json!(named).to_string()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))? {
            let (item, earlier, now) = r?;
            let earlier: Vec<String> = serde_json::from_str(&earlier).unwrap_or_default();
            if earlier.contains(&m.license) && now != m.license && info.contains_key(&item) {
                if relicensed.is_empty() {
                    relicensed_now = now.clone();
                }
                relicensed.push(json!({ "item_id": item, "earlier": earlier, "now": now }));
            }
        }
    }
    let pq: Vec<Value> = d
        .incompatible
        .iter()
        .take(100)
        .map(|pos| {
            let bound_by: Vec<Value> = d.binders[*pos]
                .iter()
                .flat_map(|id| {
                    let up: Vec<&(String, ChainLink)> = d.true_bound[*pos].iter().filter(|(via, _)| via == id).collect();
                    if !up.is_empty() {
                        // The upstream restriction itself (its item, author and licence), the relabel only as "via".
                        return up
                            .into_iter()
                            .map(|(_, l)| {
                                let mut o = match info.get(&l.item_id) {
                                    Some((title, author)) => json!({ "item_id": l.item_id, "title": title, "author_name": author }),
                                    None => json!({ "item_id": null, "title": "", "author_name": l.author_name }),
                                };
                                o["license"] = json!(l.license);
                                o["via"] = if info.contains_key(id) { json!(id) } else { Value::Null };
                                o
                            })
                            .collect::<Vec<_>>();
                    }
                    vec![match info.get(id) {
                        Some((title, author)) => json!({ "item_id": id, "title": title, "author_name": author }),
                        None => json!({ "item_id": null, "title": "", "author_name": "", "note": "an item that is no longer shown" }),
                    }]
                })
                .collect();
            let mut o = json!({ "pos": pos, "allowed": d.per_question[*pos], "bound_by": bound_by });
            if d.overflowed.contains(pos) {
                o["evidence_overflow"] = json!(true);
            }
            o
        })
        .collect();
    let mut err = PoolError::new(422, "license_incompatible");
    if !relicensed.is_empty() {
        err = err.with("relicensed", json!(first_n(&relicensed, 20))).with(
            "message",
            json!(format!(
                "A source was relicensed after you received it (now {relicensed_now}). The version you received under {} stays usable under it: keep your copy as it was, or publish this update under {relicensed_now}.",
                m.license
            )),
        );
    }
    Err(Fail::from(
        err.with("positions", json!(first_n(&d.incompatible, 100)))
            .with("allowed", json!(allowed_for_bank(&d.per_question)))
            .with("per_question", json!(pq)),
    ))
}

/// The questions flagged `origin_suspect` for the admin: a loose stem match with a served question (never for a source the author's own first publish precedes),
/// a near stem, or a walk on the author's own lineage (which is also soft).
#[allow(clippy::too_many_arguments)]
pub fn suspects(x: &PoolCtx, me: &str, h: &Hashed, f: &Facts, e: &Evid, d: &mut Derived) -> Res<Vec<usize>> {
    let want: Vec<String> = h.stems.iter().enumerate().filter(|(k, s)| s.is_some() && d.entries[*k].chain.is_empty()).filter_map(|(_, s)| s.clone()).collect();
    let hits = stem_hits(x.conn, me, &want)?;
    let before = |k: usize, seq: Option<i64>| seq.is_some_and(|s| e.fuzzy_first(f, k).is_none_or(|ff| s < ff));
    let suspect: Vec<usize> = h
        .stems
        .iter()
        .enumerate()
        .filter(|(k, s)| {
            (s.as_ref().is_some_and(|s| d.entries[*k].chain.is_empty() && before(*k, hits.get(s).copied()))) || d.fuzzy_pos.contains(k) || e.lineage_soft.contains(k)
        })
        .map(|(k, _)| k)
        .collect();
    let mut soft: Vec<usize> = e.lineage_soft.iter().copied().collect();
    soft.sort_unstable();
    for k in soft {
        push_unique(&mut d.soft_only, k);
    }
    Ok(suspect)
}

#[cfg(test)]
#[path = "publishrefuse_tests.rs"]
mod tests;
