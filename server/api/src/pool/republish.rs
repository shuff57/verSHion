//! Licences under which an account may publish a verbatim copy of what it was just served (worker/src/pool/read.ts
//! `mayRepublishAs`): per question, every import it holds of a question with that match_hash (this one included) with the
//! chains they carry, combined by the one rule in license.ts. Exactly what publish computes.
use super::authctx::author_context;
use super::ctx::{PoolCtx, Res};
use super::evidence::{CRoot, EvidenceOpts, PositionFacts, evidence};
use super::hit::{is_hard, unit_of};
use super::itemview::Form;
use super::license::{allowed_for_bank, decide_licence};
use super::visible::{subject_for, subject_param};
use rusqlite::params;

pub fn may_republish_as(x: &PoolCtx, me: &str, publish_id: &str, form: Form) -> Res<Vec<&'static str>> {
    struct Q {
        match_hash: String,
        full: Option<String>,
        keyfree: Option<String>,
        near: Option<String>,
        near_kf: Option<String>,
        short: i64,
        keywork: i64,
    }
    let mut st = x.conn.prepare(
        "SELECT match_hash, CASE WHEN ?2 = 'stripped' THEN stripped_content ELSE full_content END AS full, keyfree_content AS keyfree,
              CASE WHEN ?2 = 'stripped' THEN near_stripped ELSE near_full END AS near, near_kf, short,
              (?2 = 'full' AND qkind = 'Short' AND COALESCE(full_content, '') <> '') AS keywork FROM item_questions WHERE publish_id = ?1 ORDER BY pos",
    )?;
    let rows: Vec<Q> = st
        .query_map(params![publish_id, form.as_str()], |r| {
            Ok(Q { match_hash: r.get("match_hash")?, full: r.get("full")?, keyfree: r.get("keyfree")?, near: r.get("near")?, near_kf: r.get("near_kf")?, short: r.get("short")?, keywork: r.get("keywork")? })
        })?
        .collect::<Result<_, _>>()?;
    let hashes: Vec<String> = rows.iter().map(|r| r.match_hash.clone()).collect();
    // A verbatim copy of what was served, published the only way it can be (hidden when the serve was stripped): the same
    // Subject, query, rules and licence decision publish uses.
    let subjects: Vec<_> = rows
        .iter()
        .map(|q| subject_for(form == Form::Stripped, q.keyfree.as_deref().unwrap_or(""), q.full.as_deref().unwrap_or(""), q.near.as_deref().unwrap_or(""), q.near_kf.as_deref().unwrap_or(""), None, false))
        .collect();
    let actx = author_context(x, me, &hashes)?;
    let facts: Vec<PositionFacts> = subjects
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let q = &rows[i];
            PositionFacts {
                subject: subject_param(s, &hashes[i], vec![]),
                first: None,
                ownshown: s.full.as_deref().map(|f| actx.own.get(&hashes[i]).is_some_and(|o| o.contains(f))).unwrap_or(false),
                common: q.short == 1,
                keywork: q.keywork,
                short40: 0,
                croots: actx.roots.get(&hashes[i]).map(|v| v.iter().map(|r| CRoot { r: r.r.clone(), s: r.s, n: r.n.clone() }).collect()).unwrap_or_default(),
                ck: actx.k,
                ownkf: q.keywork == 0 && actx.own_kf.contains(q.near_kf.as_deref().unwrap_or("")),
            }
        })
        .collect();
    let mine: Vec<String> = actx.mine.iter().cloned().collect();
    let ev = evidence(x.conn, me, &facts, &EvidenceOpts { mine: &mine, independent: &[] })?;
    let per_question: Vec<Vec<&'static str>> = (0..rows.len())
        .map(|i| {
            let units: Vec<_> = ev.by_pos.get(&i).map(|hs| hs.iter().filter(|h| h.by_match && is_hard(h, false)).map(unit_of).collect()).unwrap_or_default();
            decide_licence(&units).allowed
        })
        .collect();
    Ok(allowed_for_bank(&per_question))
}

#[cfg(test)]
#[path = "republish_tests.rs"]
mod tests;
