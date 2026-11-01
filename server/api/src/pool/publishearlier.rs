//! Pieces of worker/src/pool/publish.ts that decide what a publish may keep or credit: the earlier-publication lookup (in two steps, so no near hash is followed
//! more than once per chunk), and the two served-body comparisons that let a hidden republish keep its previous stripped body.
use super::core::{canonical, questions_of};
use super::qjson::sort_utf16;
use super::sqlfrag::visible_to_others;
use rusqlite::{Connection, params};
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};

const US: &str = "\u{1f}";

/// (column, extra condition) for each near-hash column the lookup tries, one at a time, each by its index.
const EARLIER_MATCH: [(&str, &str); 5] = [("near_full", "p.hide_key = 0 AND "), ("near_bare_full", "p.hide_key = 0 AND "), ("near_stripped", ""), ("near_bare_stripped", ""), ("near_kf", "")];

/// ?1 the publisher, ?2 the chunk's distinct near hashes, ?3 / ?4 the pairs hash + U+001F + her explanation key / answer key: for each hash, the questions it
/// finds in the served sense on an item visible to others by another author, and of those the FIRST in the lookup's order (seq, publish, lineage index,
/// rowid): overall, and among those with each asked-for explanation key, and each asked-for answer key.
pub fn earlier_firsts_sql() -> String {
    let cand = EARLIER_MATCH
        .iter()
        .map(|(col, hide)| {
            format!(
                "
       SELECT v.hv, iq.rowid AS rid, iq.seq, iq.publish_id, iq.lineage_seq, iq.bk_expl, iq.bk_ans
         FROM vals v CROSS JOIN item_questions iq INDEXED BY item_questions_{col} ON iq.{col} = v.hv
              CROSS JOIN item_publishes p ON p.publish_id = iq.publish_id CROSS JOIN items i ON i.public_publish_id = iq.publish_id CROSS JOIN users u ON u.id = i.author_id
        WHERE {hide}i.author_id <> ?1 AND {}",
                visible_to_others()
            )
        })
        .collect::<Vec<_>>()
        .join("\n       UNION ALL");
    format!(
        "WITH vals AS MATERIALIZED (SELECT DISTINCT value AS hv FROM json_each(?2)),
     cand AS MATERIALIZED ({cand}),
     ranked AS MATERIALIZED (SELECT hv, rid, bk_expl, bk_ans,
                                    ROW_NUMBER() OVER (PARTITION BY hv ORDER BY seq, publish_id, lineage_seq, rid) AS r0,
                                    ROW_NUMBER() OVER (PARTITION BY hv, bk_expl ORDER BY seq, publish_id, lineage_seq, rid) AS re,
                                    ROW_NUMBER() OVER (PARTITION BY hv, bk_ans ORDER BY seq, publish_id, lineage_seq, rid) AS ra FROM cand)
SELECT hv, rid, bk_expl, bk_ans, r0 = 1 AS f0, re = 1 AS fe, ra = 1 AS fa FROM ranked
 WHERE r0 = 1 OR (re = 1 AND hv || char(31) || bk_expl IN (SELECT value FROM json_each(?3))) OR (ra = 1 AND hv || char(31) || bk_ans IN (SELECT value FROM json_each(?4)))"
    )
}

/// The lookup's own test on each position's candidates (`$.c`) only. A position's question counts when `seq` is below her first (F) or, for a source that
/// lacks what she added, below her looser first L (L <= F; NULL passes). ?1 the publisher, ?2 the positions as JSON (`i`, `c`, `n`, `expl`, `ans`, `lexpl`,
/// `lans`, `first`, `loose`).
pub fn earlier_sql() -> String {
    format!(
        "SELECT CAST(j.value ->> '$.i' AS INTEGER) AS pos,
                  (SELECT json_object('item_id', i.id, 'qid', iq.qid, 'version', i.public_version, 'license', i.license, 'author_name', COALESCE(u.display_name, 'deleted teacher'))
                     FROM item_questions iq JOIN item_publishes p ON p.publish_id = iq.publish_id JOIN items i ON i.public_publish_id = iq.publish_id JOIN users u ON u.id = i.author_id
                    WHERE iq.rowid IN (SELECT value FROM json_each(j.value -> '$.c'))
                      AND ((p.hide_key = 0 AND (iq.near_full IN (SELECT value FROM json_each(j.value -> '$.n')) OR iq.near_bare_full IN (SELECT value FROM json_each(j.value -> '$.n'))))
                           OR iq.near_stripped IN (SELECT value FROM json_each(j.value -> '$.n')) OR iq.near_bare_stripped IN (SELECT value FROM json_each(j.value -> '$.n'))
                           OR iq.near_kf IN (SELECT value FROM json_each(j.value -> '$.n')))
                      AND i.author_id <> ?1 AND {}
                      AND COALESCE(iq.seq < CAST(CASE
                             WHEN ((j.value ->> '$.expl') IS NULL OR (j.value ->> '$.expl') = ''
                                    OR ((j.value ->> '$.lexpl') IS NOT NULL AND (j.value ->> '$.lexpl') <> '' AND (j.value ->> '$.lexpl') IS (j.value ->> '$.expl'))
                                    OR iq.bk_expl IS NULL OR iq.bk_expl IS NOT (j.value ->> '$.expl'))
                               AND ((j.value ->> '$.ans') IS NULL OR (j.value ->> '$.ans') = ''
                                    OR ((j.value ->> '$.lans') IS NOT NULL AND (j.value ->> '$.lans') <> '' AND (j.value ->> '$.lans') IS (j.value ->> '$.ans'))
                                    OR iq.bk_ans IS NULL OR iq.bk_ans IS NOT (j.value ->> '$.ans'))
                             THEN COALESCE(j.value ->> '$.loose', j.value ->> '$.first') ELSE j.value ->> '$.first' END AS INTEGER), 1)
                    ORDER BY iq.seq, iq.publish_id, iq.lineage_seq, iq.rowid LIMIT 1) AS m
             FROM json_each(?2) j",
        visible_to_others()
    )
}

/// One position's near hashes and the explanation / answer keys the publisher added.
#[derive(Debug, Clone, Default)]
pub struct Want {
    pub n: Vec<String>,
    pub expl: Option<String>,
    pub ans: Option<String>,
}

/// Each position's candidates (`c`): the first question of each of its hashes, and the first sharing its explanation or answer key.
pub fn earlier_candidates(conn: &Connection, me: &str, want: &[Want]) -> rusqlite::Result<Vec<Vec<i64>>> {
    let mut vals: Vec<&str> = vec![];
    let mut seen = HashSet::new();
    for w in want {
        for h in &w.n {
            if seen.insert(h.as_str()) {
                vals.push(h);
            }
        }
    }
    if vals.is_empty() {
        return Ok(want.iter().map(|_| vec![]).collect());
    }
    let pairs = |pick: fn(&Want) -> &Option<String>| -> String {
        let mut out: Vec<String> = vec![];
        let mut seen = HashSet::new();
        for w in want {
            if let Some(k) = pick(w) {
                if k.is_empty() {
                    continue;
                }
                for h in &w.n {
                    let p = format!("{h}{US}{k}");
                    if seen.insert(p.clone()) {
                        out.push(p);
                    }
                }
            }
        }
        json!(out).to_string()
    };
    let (e, a) = (pairs(|w| &w.expl), pairs(|w| &w.ans));
    let mut first: HashMap<String, i64> = HashMap::new();
    let mut st = conn.prepare(&earlier_firsts_sql())?;
    let rows = st.query_map(params![me, json!(vals).to_string(), e, a], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, Option<String>>(3)?, r.get::<_, i64>(4)?, r.get::<_, i64>(5)?, r.get::<_, i64>(6)?))
    })?;
    for row in rows {
        let (hv, rid, bk_expl, bk_ans, f0, fe, fa) = row?;
        if f0 != 0 {
            first.insert(hv.clone(), rid);
        }
        if fe != 0 {
            if let Some(x) = bk_expl {
                first.insert(format!("e{US}{hv}{US}{x}"), rid);
            }
        }
        if fa != 0 {
            if let Some(x) = bk_ans {
                first.insert(format!("a{US}{hv}{US}{x}"), rid);
            }
        }
    }
    Ok(want
        .iter()
        .map(|w| {
            let mut out: Vec<i64> = vec![];
            for h in &w.n {
                let cands = [
                    first.get(h).copied(),
                    w.expl.as_deref().filter(|x| !x.is_empty()).and_then(|x| first.get(&format!("e{US}{h}{US}{x}")).copied()),
                    w.ans.as_deref().filter(|x| !x.is_empty()).and_then(|x| first.get(&format!("a{US}{h}{US}{x}")).copied()),
                ];
                for c in cands.into_iter().flatten() {
                    if !out.contains(&c) {
                        out.push(c);
                    }
                }
            }
            out
        })
        .collect())
}

fn q_of(order: &Value, key: &str) -> Option<i64> {
    order.get(key).and_then(Value::as_i64)
}

/// The kind object of a question, or of one part of a Multipart question.
fn kind_at<'a>(qs: &[&'a Value], o: &Value) -> Option<&'a Value> {
    let q = qs.get(q_of(o, "question")? as usize)?;
    let k = q.get("kind")?;
    match o.get("part").filter(|p| !p.is_null()) {
        None => Some(k),
        Some(p) => k.get("Multipart")?.get("parts")?.get(p.as_u64()? as usize)?.get("kind"),
    }
}

fn without_id(v: &Value) -> Value {
    match v {
        Value::Object(o) => Value::Object(o.iter().filter(|(k, _)| k.as_str() != "id").map(|(k, x)| (k.clone(), x.clone())).collect::<Map<_, _>>()),
        other => other.clone(),
    }
}

/// old[i] -> the index j of the same content in fresh (each j used once) -> map[j].
fn remap(old: &[Value], new: &[Value], map: &[Value]) -> Option<Vec<Value>> {
    if old.len() != new.len() || map.len() != new.len() {
        return None;
    }
    let mut used = vec![false; new.len()];
    let new_c: Vec<String> = new.iter().map(canonical).collect();
    let mut out = vec![];
    for x in old {
        let c = canonical(x);
        let j = new_c.iter().enumerate().position(|(k, y)| !used[k] && *y == c)?;
        used[j] = true;
        out.push(map[j].clone());
    }
    Some(out)
}

fn arr_of<'a>(v: &'a Value, path: &[&str]) -> Option<&'a Vec<Value>> {
    let mut cur = v;
    for p in path {
        cur = cur.get(*p)?;
    }
    cur.as_array()
}

/// The author's served orders for a REUSED stripped body (a hidden republish that changed nothing served keeps the previous body, so the orders this publish's
/// strip drew describe another shuffle): each entry remapped onto the reused body by content, item for item. None when some item has no counterpart (then the
/// body is not reused).
pub fn remap_served_orders(reused: &Value, fresh: &Value, served_orders: &[Value]) -> Option<Vec<Value>> {
    let (q_old, q_new) = (questions_of(reused), questions_of(fresh));
    let mut out = vec![];
    for raw in served_orders {
        let (ko, kn) = (kind_at(&q_old, raw)?, kind_at(&q_new, raw)?);
        let order = raw.get("order")?.as_array()?;
        let mut e = raw.as_object()?.clone();
        if raw.get("matching") == Some(&Value::Bool(true)) {
            let (mo, mn) = (ko.get("Matching")?, kn.get("Matching")?);
            let strip = |v: &Vec<Value>| v.iter().map(without_id).collect::<Vec<_>>();
            let o = remap(&strip(arr_of(mo, &["distractors"])?), &strip(arr_of(mn, &["distractors"])?), order)?;
            e.insert("order".into(), Value::Array(o));
            if let Some(lo) = raw.get("left_order").and_then(Value::as_array) {
                let lefts = |m: &Value| arr_of(m, &["pairs"]).map(|p| p.iter().map(|x| x.get("left").cloned().unwrap_or(Value::Null)).collect::<Vec<_>>());
                e.insert("left_order".into(), Value::Array(remap(&lefts(mo)?, &lefts(mn)?, lo)?));
            }
        } else {
            let (mo, mn) = (ko.get("Mc")?, kn.get("Mc")?);
            let strip = |v: &Vec<Value>| v.iter().map(without_id).collect::<Vec<_>>();
            let o = remap(&strip(arr_of(mo, &["answers"])?), &strip(arr_of(mn, &["answers"])?), order)?;
            e.insert("order".into(), Value::Array(o));
        }
        out.push(Value::Object(e));
    }
    Some(out)
}

/// `id`, `created` and `modified` dropped at every depth.
fn drop_times(v: &Value) -> Value {
    match v {
        Value::Array(a) => Value::Array(a.iter().map(drop_times).collect()),
        Value::Object(o) => Value::Object(o.iter().filter(|(k, _)| !["id", "created", "modified"].contains(&k.as_str())).map(|(k, x)| (k.clone(), drop_times(x))).collect()),
        other => other.clone(),
    }
}
fn js(v: &Value) -> String {
    canonical(&drop_times(v))
}

fn pos_key(o: &Value) -> Option<String> {
    let q = q_of(o, "question")?;
    Some(match o.get("part").filter(|p| !p.is_null()).and_then(Value::as_i64) {
        None => format!("{q}"),
        Some(p) => format!("{q}/{p}"),
    })
}

/// Equal as served to non-authors, ignoring only ids, times and what strip shuffles: order matters everywhere share-core preserves it (questions, Multipart
/// parts, locked choices, acknowledged / kept orders, Matching left items); only the choices strip actually shuffled (`served_orders`: unlocked Mc choices of
/// that question or part) and the Matching word bank are compared as multisets.
pub fn same_served(a: &Value, b: &Value, served_orders: &[Value], kept: &[Value]) -> bool {
    let kept_set: HashSet<String> = kept.iter().filter_map(pos_key).collect();
    let known: Vec<String> = served_orders.iter().filter(|o| o.get("matching") != Some(&Value::Bool(true))).filter_map(pos_key).collect();
    let left_shuffled: HashSet<String> = served_orders
        .iter()
        .filter(|o| o.get("matching") == Some(&Value::Bool(true)) && o.get("left_order").is_some_and(Value::is_array))
        .filter_map(|o| q_of(o, "question").map(|q| q.to_string()))
        .collect();
    let shuffled: Option<HashSet<String>> = if known.is_empty() { None } else { Some(known.into_iter().collect()) };
    let is_shuffled = |k: &str| match &shuffled {
        Some(s) => s.contains(k),
        None => !kept_set.contains(k),
    };
    let sorted = |mut v: Vec<String>| {
        sort_utf16(&mut v);
        v
    };
    let answers = |list: &[Value], shuf: bool| -> Value {
        if shuf {
            let locked: Vec<Value> = list.iter().enumerate().filter(|(_, x)| x.get("locked").is_some_and(truthy)).map(|(k, x)| json!([k, js(x)])).collect();
            let free = sorted(list.iter().filter(|x| !x.get("locked").is_some_and(truthy)).map(js).collect());
            json!({ "locked": locked, "free": free })
        } else {
            json!(list.iter().map(js).collect::<Vec<_>>())
        }
    };
    fn kind_canon(kind: &Value, key: &str, is_shuffled: &dyn Fn(&str) -> bool, left_shuffled: &HashSet<String>, answers: &dyn Fn(&[Value], bool) -> Value, sorted: &dyn Fn(Vec<String>) -> Vec<String>) -> Value {
        if let Some(mc) = kind.get("Mc") {
            let list = mc.get("answers").and_then(Value::as_array).cloned().unwrap_or_default();
            return json!({ "Mc": answers(&list, is_shuffled(key)) });
        }
        if let Some(m) = kind.get("Matching") {
            let pairs = m.get("pairs").and_then(Value::as_array).cloned().unwrap_or_default();
            let mut left: Vec<String> = pairs.iter().map(|p| js(p.get("left").unwrap_or(&Value::Null))).collect();
            if left_shuffled.contains(key) {
                left = sorted(left);
            }
            let mut bank: Vec<String> = pairs.iter().map(|p| js(p.get("right").unwrap_or(&Value::Null))).collect();
            bank.extend(m.get("distractors").and_then(Value::as_array).cloned().unwrap_or_default().iter().map(js));
            return json!({ "Matching": { "left": left, "bank": sorted(bank) } });
        }
        if let Some(mp) = kind.get("Multipart") {
            let parts = mp.get("parts").and_then(Value::as_array).cloned().unwrap_or_default();
            let v: Vec<Value> = parts
                .iter()
                .enumerate()
                .map(|(k, p)| json!({ "stem": p.get("stem").cloned().unwrap_or(Value::Null), "kind": kind_canon(p.get("kind").unwrap_or(&Value::Null), &format!("{key}/{k}"), is_shuffled, left_shuffled, answers, sorted) }))
                .collect();
            return json!({ "Multipart": v });
        }
        drop_times(kind)
    }
    let q_canon = |q: &Value, i: usize| -> String {
        let mut base = drop_times(q);
        if let Value::Object(o) = &mut base {
            o.insert("kind".into(), kind_canon(q.get("kind").unwrap_or(&Value::Null), &i.to_string(), &is_shuffled, &left_shuffled, &answers, &sorted));
        }
        canonical(&base)
    };
    let (qa, qb) = (questions_of(a), questions_of(b));
    if qa.len() != qb.len() || !qa.iter().zip(&qb).enumerate().all(|(i, (x, y))| q_canon(x, i) == q_canon(y, i)) {
        return false;
    }
    let rest = |p: &Value| -> String {
        match p.get("bank").filter(|b| !b.is_null()) {
            Some(b) => {
                let mut b = b.clone();
                if let Value::Object(o) = &mut b {
                    o.insert("questions".into(), Value::Null);
                }
                js(&b)
            }
            None => {
                let mut p = p.clone();
                if let Value::Object(o) = &mut p {
                    o.insert("question".into(), Value::Null);
                }
                js(&p)
            }
        }
    };
    rest(a) == rest(b)
}

/// What a hidden republish keeps of its previous stripped body when the served form did not change.
#[derive(Debug, Clone, PartialEq)]
pub struct Reused {
    pub bytes: Vec<u8>,
    pub hashes: Vec<String>,
    pub contents: Vec<String>,
    pub near: Vec<Option<String>>,
    pub bare: Vec<Option<String>>,
    pub served_orders: Vec<Value>,
}

/// A hidden republish whose served form is unchanged: the previous stripped body, its rows' served facts and the author's served orders remapped onto it; None
/// when it cannot be kept (a different question list, a missing or unreadable body, a changed served form, an order with no counterpart).
pub fn reuse_served(conn: &Connection, store: &crate::blobs::BlobStore, base_of: &str, qids: &[String], st: &super::core::Stripped, order_acks: &[super::core::Pos]) -> rusqlite::Result<Option<Reused>> {
    type PrevRow = (String, String, String, Option<String>, Option<String>);
    let prev: Vec<PrevRow> = conn
        .prepare("SELECT qid, stripped_sha256, stripped_content, near_stripped, near_bare_stripped FROM item_questions WHERE publish_id = ?1 ORDER BY pos")?
        .query_map([base_of], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
        .collect::<Result<_, _>>()?;
    let prev_key: Option<String> = conn.query_row("SELECT stripped_key FROM item_publishes WHERE publish_id = ?1", [base_of], |r| r.get(0)).ok().flatten();
    let Some(prev_key) = prev_key.filter(|k| !k.is_empty()) else { return Ok(None) };
    if prev.len() != qids.len() || !prev.iter().zip(qids).all(|(q, id)| &q.0 == id) {
        return Ok(None);
    }
    let Some(bytes) = store.get(&prev_key).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))? else { return Ok(None) };
    let Ok(prev_body) = serde_json::from_slice::<Value>(&bytes) else { return Ok(None) };
    // The author's key review must describe the body served (remapped onto it).
    let mut kept: Vec<Value> = order_acks.iter().map(|p| serde_json::to_value(p).unwrap_or(Value::Null)).collect();
    kept.extend(st.order_reasons.iter().cloned());
    let so = if same_served(&prev_body, &st.v.payload, &st.served_orders, &kept) { remap_served_orders(&prev_body, &st.v.payload, &st.served_orders) } else { None };
    let Some(served_orders) = so else { return Ok(None) };
    Ok(Some(Reused {
        bytes,
        hashes: prev.iter().map(|q| q.1.clone()).collect(),
        contents: prev.iter().map(|q| q.2.clone()).collect(),
        near: prev.iter().map(|q| q.3.clone()).collect(),
        bare: prev.iter().map(|q| q.4.clone()).collect(),
        served_orders,
    }))
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

#[cfg(test)]
#[path = "publishearlier_tests.rs"]
mod tests;
