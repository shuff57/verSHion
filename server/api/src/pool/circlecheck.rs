//! The one link between circles and the public pool (worker/src/pool/circlecheck.ts): a publish may not carry material a circle member
//! first sent, unless the publisher sent it first, had published it publicly earlier, it is a short statement, or she names the circle
//! bank in an independence statement (never covering an exact copy of something she can still read). Otherwise 422 `circle_material`.
use super::circlesql::{active_sql, author_ok_sql};
use super::ctx::{PoolCtx, PoolError, Res, bad};
use super::sqlfrag::is_id;
use crate::auth::validate::clean_text;
use rusqlite::params;
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
pub struct CircleClaim {
    pub banks: Vec<String>,
    pub statement: String,
}

/// The meta's `circle_independent_of` (None when absent or null; 400 when malformed).
pub fn read_circle_claim(m: &Map<String, Value>) -> Res<Option<CircleClaim>> {
    let x = match m.get("circle_independent_of") {
        None | Some(Value::Null) => return Ok(None),
        Some(x) => x,
    };
    let bad_claim = |f: &str| -> crate::pool::ctx::Fail { bad(f).into() };
    let o = x.as_object();
    let banks = o.and_then(|o| o.get("banks")).and_then(Value::as_array);
    let accepted = o.and_then(|o| o.get("accepted")) == Some(&Value::Bool(true));
    let Some(banks) = banks.filter(|b| accepted && !b.is_empty() && b.len() <= 50 && b.iter().all(|b| b.as_str().is_some_and(is_id))) else {
        return Err(bad_claim("circle_independent_of"));
    };
    let statement = o.and_then(|o| o.get("statement")).and_then(|s| clean_text(s, 1000)).filter(|s| s.encode_utf16().count() >= 10);
    let Some(statement) = statement else { return Err(bad_claim("circle_independent_of.statement")) };
    let mut seen = HashSet::new();
    Ok(Some(CircleClaim { banks: banks.iter().map(|b| b.as_str().unwrap().to_string()).filter(|b| seen.insert(b.clone())).collect(), statement }))
}

/// The earliest sender of each hash across the publisher's circles, one row per hash that has a hit.
pub const CIRCLE_HITS_SQL: &str = "WITH h AS MATERIALIZED (SELECT DISTINCT value AS mh FROM json_each(?2)),
     my AS MATERIALIZED (SELECT m.circle_id FROM circle_members m JOIN circles ci ON ci.id = m.circle_id AND ci.qkeys > 0 AND ci.purged_at IS NULL
                          WHERE m.user_id = ?1 ORDER BY m.left_at IS NOT NULL, m.left_at DESC, m.circle_id LIMIT ?3),
     x AS MATERIALIZED (SELECT h.mh AS mh, my.circle_id AS circle_id,
                  (SELECT json_array(k.bank_id, k.first_seq, k.first_by) FROM circle_qkeys k INDEXED BY circle_qkeys_first
                    WHERE k.circle_id = my.circle_id AND k.match_hash = h.mh ORDER BY k.first_seq, k.bank_id LIMIT 1) AS hit
             FROM h CROSS JOIN my)
SELECT mh, circle_id, hit FROM (SELECT mh, circle_id, hit, ROW_NUMBER() OVER (PARTITION BY mh ORDER BY hit ->> '$[1]', circle_id, hit ->> '$[0]') AS rn FROM x WHERE hit IS NOT NULL)
 WHERE rn = 1";

/// Hashes a circle-only bank the publisher can still read holds, first sent by someone else.
pub const READABLE_OTHERS_SQL: &str = "SELECT DISTINCT j.value AS mh FROM json_each(?2) j
 WHERE EXISTS (SELECT 1 FROM circle_members m JOIN circles ci ON ci.id = m.circle_id AND ci.closed_at IS NULL AND ci.purged_at IS NULL
                 JOIN circle_qkeys k ON k.circle_id = m.circle_id AND k.match_hash = j.value
                 JOIN circle_banks b ON b.id = k.bank_id AND b.removed_at IS NULL AND b.purged_at IS NULL AND b.license = 'circle-only'
                WHERE m.user_id = ?1 AND m.left_at IS NULL AND k.first_by <> ?1)";

#[derive(Clone)]
struct Best {
    circle: String,
    bank: String,
    seq: i64,
    by: String,
}
#[derive(Clone)]
struct Hit {
    best: Best,
}

fn hash_set(x: &PoolCtx, sql: &str, args: &[&dyn rusqlite::ToSql], mut f: impl FnMut(&rusqlite::Row) -> rusqlite::Result<()>) -> rusqlite::Result<()> {
    let mut st = x.conn.prepare(sql)?;
    let mut rows = st.query(args)?;
    while let Some(r) = rows.next()? {
        f(r)?;
    }
    Ok(())
}

/// `hashes_at[i]`: the key-free match hashes of position i (its full and served forms); `short_at[i]`: a short statement. Returns the
/// banks an independence statement was actually needed for, or 422 `circle_material`.
pub fn check_circle_material(x: &PoolCtx, me: &str, hashes_at: &[Vec<String>], short_at: &[bool], claim: Option<&CircleClaim>) -> Res<Vec<String>> {
    let member: Option<i64> = x.conn.query_row("SELECT 1 FROM circle_members WHERE user_id = ?1 LIMIT 1", [me], |r| r.get(0)).ok();
    if member.is_none() {
        return Ok(vec![]);
    }
    let mut all: Vec<&String> = Vec::new();
    let mut seen = HashSet::new();
    for h in hashes_at.iter().flatten() {
        if seen.insert(h) {
            all.push(h);
        }
    }
    let circles_max = x.limit_of(me, "circle_memberships_total")?;
    let mut best: HashMap<String, Best> = HashMap::new();
    for part in all.chunks(1000) {
        let js = serde_json::to_string(part).unwrap();
        hash_set(x, CIRCLE_HITS_SQL, &[&me, &js, &circles_max], |r| {
            let (mh, circle, hit): (String, String, String) = (r.get(0)?, r.get(1)?, r.get(2)?);
            let h: (String, i64, String) = serde_json::from_str(&hit).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let b = Best { circle: circle.clone(), bank: h.0.clone(), seq: h.1, by: h.2 };
            let better = match best.get(&mh) {
                None => true,
                Some(o) => b.seq < o.seq || (b.seq == o.seq && (b.circle < o.circle || (b.circle == o.circle && b.bank < o.bank))),
            };
            if better {
                best.insert(mh, b);
            }
            Ok(())
        })?;
    }
    if best.is_empty() {
        return Ok(vec![]);
    }
    // Per position: the earliest hit over its hashes.
    let hit_at: Vec<Option<Hit>> = hashes_at
        .iter()
        .map(|hs| {
            let mut out: Option<&Best> = None;
            for h in hs {
                if let Some(b) = best.get(h) {
                    if out.is_none_or(|o| b.seq < o.seq) {
                        out = Some(b);
                    }
                }
            }
            out.map(|b| Hit { best: b.clone() })
        })
        .collect();
    // "Already hers in the pool": her own public first of the text, before the circle's first sender sent it.
    let mut hit_hashes: Vec<&String> = Vec::new();
    let mut seen = HashSet::new();
    for (i, hs) in hashes_at.iter().enumerate() {
        if hit_at[i].as_ref().is_some_and(|h| h.best.by != me) && !short_at[i] {
            for h in hs {
                if seen.insert(h) {
                    hit_hashes.push(h);
                }
            }
        }
    }
    let mut firsts: HashMap<String, i64> = HashMap::new();
    for part in hit_hashes.chunks(1000) {
        let js = serde_json::to_string(part).unwrap();
        hash_set(x, "SELECT match_hash, first_seq FROM author_firsts WHERE author_id = ?1 AND match_hash IN (SELECT value FROM json_each(?2))", &[&me, &js], |r| {
            firsts.insert(r.get(0)?, r.get(1)?);
            Ok(())
        })?;
    }
    let open: Vec<usize> = (0..hashes_at.len())
        .filter(|&i| match &hit_at[i] {
            None => false,
            Some(h) => h.best.by != me && !short_at[i] && !hashes_at[i].iter().any(|x| firsts.get(x).is_some_and(|f| *f < h.best.seq)),
        })
        .collect();
    if open.is_empty() {
        return Ok(vec![]);
    }
    // What a statement may cover: never a text a circle-only bank she can still read holds, first sent by someone else.
    let mut readable: HashSet<String> = HashSet::new();
    let mut asked: HashSet<String> = HashSet::new();
    let mut ask = |readable: &mut HashSet<String>, hs: Vec<&String>| -> rusqlite::Result<()> {
        let mut todo: Vec<&String> = Vec::new();
        for h in hs {
            if asked.insert(h.clone()) {
                todo.push(h);
            }
        }
        for part in todo.chunks(1000) {
            let js = serde_json::to_string(part).unwrap();
            hash_set(x, READABLE_OTHERS_SQL, &[&me, &js], |r| {
                readable.insert(r.get(0)?);
                Ok(())
            })?;
        }
        Ok(())
    };
    let bank_of = |i: usize| hit_at[i].as_ref().unwrap().best.bank.clone();
    if let Some(c) = claim {
        ask(&mut readable, open.iter().filter(|&&i| c.banks.contains(&bank_of(i))).flat_map(|&i| hashes_at[i].iter()).collect())?;
    }
    let may_claim = |readable: &HashSet<String>, i: usize| !hashes_at[i].iter().any(|x| readable.contains(x));
    let mut claimed: Vec<String> = Vec::new();
    let mut refused: Vec<usize> = Vec::new();
    for &i in &open {
        if claim.is_some_and(|c| c.banks.contains(&bank_of(i))) && may_claim(&readable, i) {
            if !claimed.contains(&bank_of(i)) {
                claimed.push(bank_of(i));
            }
        } else {
            refused.push(i);
        }
    }
    if refused.is_empty() {
        return Ok(claimed);
    }
    ask(&mut readable, refused.iter().take(100).flat_map(|&i| hashes_at[i].iter()).collect())?;
    let mut banks: Vec<String> = Vec::new();
    for &i in &refused {
        let b = bank_of(i);
        if !banks.contains(&b) {
            banks.push(b);
        }
    }
    // A hit names the bank and circle only when the publisher can see that bank now; a held bank (or a suspended author's) gives a
    // generic hit; otherwise (a former member, a removed bank, a closed circle) the ids, to name in a statement, without names.
    struct Nm {
        title: String,
        circle_id: String,
        name: Option<String>,
        hidden: bool,
        visible: bool,
    }
    let mut names: HashMap<String, Nm> = HashMap::new();
    let js = serde_json::to_string(&banks.iter().take(100).collect::<Vec<_>>()).unwrap();
    hash_set(
        x,
        &format!(
            "SELECT b.id, b.title, ci.id AS circle_id, ci.name,
                  (EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = b.id) OR NOT {}) AS hidden,
                  (b.removed_at IS NULL AND b.purged_at IS NULL AND ci.closed_at IS NULL AND {}) AS visible
             FROM circle_banks b JOIN circles ci ON ci.id = b.circle_id WHERE b.id IN (SELECT value FROM json_each(?1))",
            author_ok_sql("b"),
            active_sql("ci.id", "?2", &[])
        ),
        &[&js, &me],
        |r| {
            names.insert(r.get(0)?, Nm { title: r.get(1)?, circle_id: r.get(2)?, name: r.get(3)?, hidden: r.get::<_, i64>(4)? == 1, visible: r.get::<_, i64>(5)? == 1 });
            Ok(())
        },
    )?;
    let hits: Vec<Value> = refused
        .iter()
        .take(100)
        .map(|&i| {
            let b = &hit_at[i].as_ref().unwrap().best;
            match names.get(&b.bank) {
                Some(nm) if !nm.hidden => {
                    json!({ "pos": i, "circle_id": b.circle, "circle_name": nm.visible.then(|| nm.name.clone()).flatten(), "bank_id": b.bank, "bank_title": nm.visible.then(|| nm.title.clone()), "may_claim_independence": may_claim(&readable, i) })
                }
                _ => json!({ "pos": i, "circle_id": null, "circle_name": null, "bank_id": null, "bank_title": null, "may_claim_independence": false }),
            }
        })
        .collect();
    let _ = params![];
    Err(PoolError::new(422, "circle_material")
        .with("positions", refused.iter().take(1000).map(|&i| json!(i)).collect::<Vec<_>>().into())
        .with("hits", hits.into())
        .with("message", "Some questions were first shared by another member of a circle. They cannot be shared publicly from here.".into())
        .into())
}

#[cfg(test)]
#[path = "circlecheck_tests.rs"]
mod tests;
