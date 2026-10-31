//! The history side of shared editing (worker/src/pool/circleversions.ts): the version history, per-question change lists, merge-initiate
//! and trim. A kept version's body is `circlebanks::bank_read` with a version.
use super::circlebankedit::OWNER_OR_AUTHOR;
use super::circlebanks::{BankRow, bank_or_miss, kept_info, require_readable};
use super::circlebase::{KEPT_VERSIONS, Membership, name_of, require_open, require_role, text};
use super::circlesendmeta::{Index, read_index};
use super::circlesendsql::{PRUNED_NOW_SQL, pinned_evidence_sql};
use super::circlesql::{active_sql, blocking_hold_sql, open_sql};
use super::ctx::{AuditEntry, Fail, PoolCtx, PoolError, Res, audit};
use super::poolkeys::PoolKeys;
use super::sqlfrag::alive_sql;
use crate::blobs::BlobStore;
use rusqlite::{OptionalExtension, params};
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};

/// History lines a page.
pub const HISTORY_PAGE: usize = 50;
/// Versions between the base and the head a merge-initiate lists at most (`between_total` says how many there are).
pub const MERGE_BETWEEN_MAX: i64 = 50;
/// How many versions' per-question indexes one call reads.
pub const CHANGE_VERSION_MAX: i64 = 40;

struct Line {
    version: i64,
    id: String,
    committed_at: Option<i64>,
    message: Option<String>,
    added: i64,
    changed: i64,
    removed: i64,
    question_count: i64,
    license: String,
    rollback_of: Option<i64>,
    merged_from: Option<i64>,
    status: String,
    full_key: String,
    withheld_at: Option<i64>,
    display_name: Option<String>,
    deleted_at: Option<i64>,
}
const LINE_COLS: &str = "v.version, v.id, v.committed_at, v.message, v.added, v.changed, v.removed, v.question_count, v.license, v.rollback_of, v.merged_from, v.status, v.full_key, v.withheld_at, u.display_name, u.deleted_at";
fn line_of_row(r: &rusqlite::Row) -> rusqlite::Result<Line> {
    Ok(Line {
        version: r.get(0)?,
        id: r.get(1)?,
        committed_at: r.get(2)?,
        message: r.get(3)?,
        added: r.get::<_, Option<i64>>(4)?.unwrap_or(0),
        changed: r.get::<_, Option<i64>>(5)?.unwrap_or(0),
        removed: r.get::<_, Option<i64>>(6)?.unwrap_or(0),
        question_count: r.get(7)?,
        license: r.get(8)?,
        rollback_of: r.get(9)?,
        merged_from: r.get(10)?,
        status: r.get(11)?,
        full_key: r.get(12)?,
        withheld_at: r.get(13)?,
        display_name: r.get(14)?,
        deleted_at: r.get(15)?,
    })
}
/// A version members may read (kept in full and not withheld).
fn available(status: &str, full_key: &str, withheld_at: Option<i64>) -> bool {
    (status == "current" || status == "previous") && !full_key.is_empty() && withheld_at.is_none()
}
/// One history line as members see it: who (a name, "deleted teacher" once erased), when, the message and counts; no ids of people. A
/// withheld version is `withheld: true`, `kept: false`, and its message is not shown; to a viewer of a bank that hides its keys no message is.
fn line_of(r: &Line, hide_text: bool) -> Value {
    let mut o = Map::new();
    o.insert("version".into(), json!(r.version));
    o.insert("version_id".into(), json!(r.id));
    o.insert("by".into(), json!(name_of(r.display_name.as_deref(), r.deleted_at)));
    o.insert("at".into(), json!(r.committed_at));
    o.insert("message".into(), json!(if r.withheld_at.is_some() || hide_text { String::new() } else { text(r.message.as_deref()) }));
    o.insert("added".into(), json!(r.added));
    o.insert("changed".into(), json!(r.changed));
    o.insert("removed".into(), json!(r.removed));
    o.insert("question_count".into(), json!(r.question_count));
    o.insert("license".into(), json!(r.license));
    o.insert("kept".into(), json!(available(&r.status, &r.full_key, r.withheld_at)));
    o.insert("withheld".into(), json!(r.withheld_at.is_some()));
    // A withheld version's text is kept for the review ("full") or no longer ("hashes_only": only the record of what matched).
    if r.withheld_at.is_some() {
        o.insert("review_copy".into(), json!(if (r.status == "current" || r.status == "previous") && !r.full_key.is_empty() { "full" } else { "hashes_only" }));
    }
    if let Some(v) = r.rollback_of {
        o.insert("rollback_of".into(), json!(v));
    }
    if let Some(v) = r.merged_from {
        o.insert("merged_from".into(), json!(v));
    }
    Value::Object(o)
}

fn merge_fields(o: &mut Value, fields: [(&'static str, Value); 2]) {
    for (k, v) in fields {
        o[k] = v;
    }
}

/// GET /circles/:cid/banks/:bid/versions?before=<n>: the history, newest first.
pub fn history(x: &PoolCtx, me: &str, cid: &str, bid: &str, before: Option<&str>) -> Res<Value> {
    let (m, b) = bank_or_miss(x, cid, bid, me)?;
    require_readable(x, &m, &b, me)?;
    let before = match before {
        None => b.head_version + 1,
        Some(s) if !s.is_empty() && s.len() <= 9 && !s.starts_with('0') && s.bytes().all(|c| c.is_ascii_digit()) => s.parse().unwrap(),
        Some(_) => return Err(PoolError::new(400, "invalid_request").with("field", json!("before")).into()),
    };
    let rows: Vec<Line> = x
        .conn
        .prepare(&format!("SELECT {LINE_COLS} FROM circle_versions v LEFT JOIN users u ON u.id = v.committer_id WHERE v.bank_id = ?1 AND v.version < ?2 ORDER BY v.version DESC LIMIT ?3"))?
        .query_map(params![bid, before, HISTORY_PAGE as i64 + 1], line_of_row)?
        .collect::<Result<_, _>>()?;
    let page = &rows[..rows.len().min(HISTORY_PAGE)];
    let hide = b.hide_key && m.role == "viewer";
    let mut out = json!({
        "bank_id": bid,
        "head_version": b.head_version,
        "versions": page.iter().map(|r| line_of(r, hide)).collect::<Vec<_>>(),
        "next_before": if rows.len() > HISTORY_PAGE { page.last().map(|l| json!(l.version)).unwrap_or(Value::Null) } else { Value::Null },
    });
    merge_fields(&mut out, kept_info(x, bid, b.head_version)?.fields());
    Ok(out)
}

fn diff_of(older: &Index, newer: &Index) -> (Vec<String>, Vec<String>, Vec<String>) {
    let before: HashMap<&String, &String> = older.qids.iter().zip(older.h.iter()).collect();
    let after: HashSet<&String> = newer.qids.iter().collect();
    let (mut added, mut changed) = (vec![], vec![]);
    for (k, q) in newer.qids.iter().enumerate() {
        match before.get(q) {
            None => added.push(q.clone()),
            Some(was) if **was != newer.h[k] => changed.push(q.clone()),
            _ => {}
        }
    }
    let removed = older.qids.iter().filter(|q| !after.contains(q)).cloned().collect();
    (added, changed, removed)
}

pub enum Changes {
    NotModified(String),
    Body(Value, String),
}

/// GET /circles/:cid/banks/:bid/changes: the per-question change lists of a bank's kept, readable versions, newest first, read from the
/// indexes the server already keeps. Each version is compared with the next readable kept one below it, and the oldest in the window is left
/// out. A version whose index is gone is a gap, never an error. A viewer of a bank that hides its keys gets `per_question: false`.
pub fn changes(x: &PoolCtx, store: &BlobStore, me: &str, cid: &str, bid: &str, if_none_match: Option<&str>) -> Res<Changes> {
    let (m, b) = bank_or_miss(x, cid, bid, me)?;
    require_readable(x, &m, &b, me)?;
    if b.hide_key && m.role == "viewer" {
        return Ok(Changes::Body(json!({ "bank_id": bid, "head_version": b.head_version, "per_question": false, "reason": "keys_hidden" }), String::new()));
    }
    let rows: Vec<(i64, String, String)> = x
        .conn
        .prepare("SELECT version, id, index_key FROM circle_versions WHERE bank_id = ?1 AND status IN ('current', 'previous') AND full_key <> '' AND withheld_at IS NULL ORDER BY version DESC LIMIT ?2")?
        .query_map(params![bid, CHANGE_VERSION_MAX], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    // The ETag names the head and the readable kept versions: a new send changes it, and nothing here does.
    let names: Vec<String> = std::iter::once(b.head_version_id.clone().unwrap_or_default()).chain(rows.iter().map(|r| format!("{}:{}", r.0, r.1))).collect();
    let etag = format!("\"{}\"", &super::blobs::sha256_hex(names.join("\n").as_bytes())[..32]);
    if if_none_match.is_some_and(|h| h.split(',').any(|t| t.trim().trim_start_matches("W/") == etag || t.trim() == "*")) {
        return Ok(Changes::NotModified(etag));
    }
    x.require_daily(me, "item_reads", 1)?;
    x.require_daily(me, "circle_change_reads", 1)?;
    let mut out: Vec<Value> = Vec::new();
    let mut held: Option<(i64, Option<Index>)> = None;
    for (version, _, index_key) in &rows {
        let idx = read_index(store, index_key);
        if let Some((hv, hidx)) = &held {
            out.push(match (hidx, &idx) {
                (Some(h), Some(i)) => {
                    let (a, c, r) = diff_of(i, h);
                    json!({ "version": hv, "compared_to": version, "added": a, "changed": c, "removed": r })
                }
                _ => json!({ "version": hv, "compared_to": version, "gap": true }),
            });
        }
        held = Some((*version, idx));
    }
    Ok(Changes::Body(json!({ "bank_id": bid, "head_version": b.head_version, "per_question": true, "versions": out }), etag))
}

/// POST /circles/:cid/banks/:bid/merge {base_version}: merge-initiate. Informational: the head, whether the base is still kept, the versions in
/// between, and a merge token. Nothing is stored and nothing is locked.
pub fn merge_initiate(x: &PoolCtx, keys: &PoolKeys, me: &str, cid: &str, bid: &str, body: &Map<String, Value>) -> Res<Value> {
    let (m, b) = bank_or_miss(x, cid, bid, me)?;
    require_readable(x, &m, &b, me)?;
    require_role(&m, &["owner", "editor"])?;
    let head = b.head_version;
    let base = match body.get("base_version").and_then(|v| v.as_i64().or_else(|| v.as_f64().filter(|f| f.fract() == 0.0).map(|f| f as i64))) {
        Some(n) if n >= 1 && n < head => n,
        _ => {
            return Err(PoolError::new(400, "invalid_request")
                .with("field", json!("base_version"))
                .with("head_version", json!(head))
                .with("message", json!("Merge from an earlier version than the circle's current one."))
                .into());
        }
    };
    // The FIRST versions after the base, oldest first.
    let rows: Vec<Line> = x
        .conn
        .prepare(&format!("SELECT {LINE_COLS} FROM circle_versions v LEFT JOIN users u ON u.id = v.committer_id WHERE v.bank_id = ?1 AND v.version > ?2 AND v.version <= ?3 ORDER BY v.version LIMIT ?4"))?
        .query_map(params![bid, base, head, MERGE_BETWEEN_MAX], line_of_row)?
        .collect::<Result<_, _>>()?;
    let base_row: Option<(String, String, Option<i64>)> = x.conn.query_row("SELECT status, full_key, withheld_at FROM circle_versions WHERE bank_id = ?1 AND version = ?2", params![bid, base], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?;
    let top: Option<(Option<i64>, Option<String>, Option<i64>)> = x
        .conn
        .query_row("SELECT v.committed_at, u.display_name, u.deleted_at FROM circle_versions v LEFT JOIN users u ON u.id = v.committer_id WHERE v.bank_id = ?1 AND v.version = ?2", params![bid, head], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .optional()?;
    let (token, exp) = keys.merge_token(x.now, me, bid, base, head);
    let mut out = json!({
        "bank_id": bid,
        "base_version": base,
        "base_kept": base_row.as_ref().is_some_and(|r| available(&r.0, &r.1, r.2)),
        "head_version": head,
        "head_version_id": b.head_version_id,
        "head_at": top.as_ref().and_then(|t| t.0).unwrap_or(b.updated_at),
        "head_by": top.as_ref().map(|t| name_of(t.1.as_deref(), t.2)).unwrap_or_else(|| name_of(None, Some(1))),
        "between": rows.iter().map(|r| line_of(r, false)).collect::<Vec<_>>(),
        "between_total": head - base,
        "merge_token": token,
        "merge_token_expires_at": exp,
    });
    merge_fields(&mut out, kept_info(x, bid, head)?.fields());
    Ok(out)
}

fn trim_rights(m: &Membership, b: &BankRow, me: &str) -> bool {
    m.role == "owner" || (b.author_id == me && m.role == "editor")
}

/// POST /circles/:cid/banks/:bid/trim {keep} (the owner, or the bank's author while an editor): the bank's versions older than its newest
/// `keep` (1 to KEPT_VERSIONS - 1) are pruned now, as the daily stage prunes past the 20: their history lines stay, their bodies stop counting
/// toward the circle's bytes at once. Never while the bank is under review, never a version an open report names.
pub fn trim_versions(x: &PoolCtx, me: &str, cid: &str, bid: &str, body: &Map<String, Value>) -> Res<Value> {
    let (m, b) = bank_or_miss(x, cid, bid, me)?;
    require_readable(x, &m, &b, me)?;
    require_open(&m)?;
    if !trim_rights(&m, &b, me) {
        return Err(PoolError::new(403, "circle_role_required").with("needed", json!(OWNER_OR_AUTHOR)).with("role", json!(m.role)).into());
    }
    let keep = match body.get("keep").and_then(Value::as_i64) {
        Some(k) if (1..KEPT_VERSIONS).contains(&k) => k,
        _ => return Err(PoolError::new(400, "invalid_request").with("field", json!("keep")).with("min", json!(1)).with("max", json!(KEPT_VERSIONS - 1)).into()),
    };
    // A student-data hold a send raised does not stop a trim, a report's or an admin's does.
    if b.held && b.blocking {
        return Err(PoolError::new(409, "under_moderation").with("message", json!("This bank is under review; its versions stay until an administrator decides.")).into());
    }
    let now = x.now;
    let tx = x.conn.unchecked_transaction()?;
    let pruned = tx.execute(
        &format!(
            "UPDATE circle_versions SET status = 'pruned', pruned_at = ?4 WHERE bank_id = ?1 AND status = 'previous'
            AND version <= (SELECT head_version FROM circle_banks WHERE id = ?1) - ?3
            AND EXISTS (SELECT 1 FROM circle_banks tb WHERE tb.id = ?1 AND tb.circle_id = ?2 AND tb.removed_at IS NULL AND tb.purged_at IS NULL
                          AND ({} OR (tb.author_id = ?5 AND {})))
            AND {} AND {}
            AND NOT {}
            AND NOT EXISTS (SELECT 1 FROM circle_reports pr WHERE pr.bank_id = ?1 AND pr.resolved_at IS NULL AND pr.version = circle_versions.version)
            AND NOT {}",
            active_sql("?2", "?5", &["owner"]),
            active_sql("?2", "?5", &["editor"]),
            open_sql("?2"),
            alive_sql("?5"),
            blocking_hold_sql("?1"),
            pinned_evidence_sql("circle_versions")
        ),
        params![bid, cid, keep, now, me],
    )?;
    audit(&tx, &AuditEntry { actor_id: Some(me), actor_kind: "user", action: "circle.versions_trimmed", item_id: Some(cid), target_user_id: Some(me), detail: Some(json!({ "bank_id": bid, "keep": keep })), note: None }, now, Some("changes() > 0"))?;
    tx.execute(&format!("DELETE FROM circle_version_blobs WHERE version_id IN ({PRUNED_NOW_SQL})"), params![bid, now])?;
    tx.commit()?;
    if pruned == 0 {
        x.throw_if_erased(me)?;
    }
    let head: i64 = x.conn.query_row("SELECT head_version FROM circle_banks WHERE id = ?1", [bid], |r| r.get(0)).optional()?.unwrap_or(b.head_version);
    // The versions kept for the administrator's review are said, by number only.
    let evidence: Vec<i64> = x
        .conn
        .prepare(&format!("SELECT ev.version FROM circle_versions ev INDEXED BY circle_versions_bank WHERE ev.bank_id = ?1 AND ev.status IN ('current', 'previous') AND ev.full_key <> '' AND {} ORDER BY ev.version", pinned_evidence_sql("ev")))?
        .query_map([bid], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let ki = kept_info(x, bid, head)?;
    let mut pinned: Vec<i64> = ki.pinned_versions.iter().copied().chain(evidence.iter().copied()).collect::<HashSet<_>>().into_iter().collect();
    pinned.sort();
    let mut out = json!({ "ok": true, "pruned": pruned, "evidence_kept": evidence.len() });
    merge_fields(&mut out, ki.fields());
    out["pinned_versions"] = json!(pinned);
    Ok(out)
}

#[allow(dead_code)]
fn _keep(_: Fail) {}

#[cfg(test)]
#[path = "circleversions_tests.rs"]
mod tests;
