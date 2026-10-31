//! Committing a version of a circle bank (worker/src/pool/circlesend.ts `commitVersion`, `whyLost`, `conflict`, `chargeHeldPublishes`):
//! stage, put, commit in one transaction whose first statement is the compare-and-set, or clean up and say why it lost. Shared by a send
//! and a rollback. The per-account lock around it belongs to the route layer.
use super::circlebankedit::OWNER_OR_AUTHOR;
use super::circlebase::{KEPT_VERSIONS, name_of};
use super::circlepurge::delete_keys;
use super::circlesendmeta::Scan;
use super::circlesendsql::*;
use super::circlesql::*;
use super::circlehide::stripped_qkeys_sql;
use super::ctx::{AuditEntry, Fail, PoolCtx, PoolError, PoolNotice, Res, audit, not_found};
use super::license::POOL_TERMS_VERSION;
use super::sqlfrag::{DAY_MS, FLAG_DETAIL_MAX, alive_sql, day_of, new_id};
use crate::blobs::BlobStore;
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};

/// Held questions a day an account may have before the administrator is flagged (the public path's budget, publish.ts).
pub const HELD_PUBLISHES_PER_DAY: i64 = 20;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Send,
    Rollback,
}

#[derive(Debug, Clone)]
pub struct Listing {
    pub title: String,
    pub subject: String,
    pub course: String,
    pub grade: String,
}

pub struct CommitSpec {
    pub kind: Kind,
    pub me: String,
    pub cid: String,
    pub bid: String,
    pub vid: String,
    pub is_new: bool,
    pub base: Option<i64>,
    pub version: i64,
    pub commit_id: String,
    pub request_sha: String,
    pub message: Option<String>,
    pub license: String,
    /// The bank's listing fields (a rollback keeps the bank's: None).
    pub meta: Option<Listing>,
    /// This version's own listing fields (JSON), recorded with it; a rollback's are its target's.
    pub listing: Option<String>,
    pub n: i64,
    pub has_images: i64,
    pub bytes: i64,
    pub new_keys: i64,
    pub hashes: Vec<String>,
    pub shas: Vec<String>,
    pub origins: Vec<Value>,
    pub merged_from: Option<i64>,
    pub rollback_of: Option<i64>,
    pub privacy: Scan,
    /// The strip decisions recorded with the version (a rollback copies its target's).
    pub strip_opts: Option<String>,
    /// [full, served] key-free hashes of positions a label stripped for viewers changes.
    pub pairs: Vec<(String, String)>,
    /// The bank's hide_key the send was checked for (None: nothing to compare).
    pub hide_key: Option<i64>,
    pub result: Value,
    pub audit_detail: Value,
    /// The stored full body and the per-question index.
    pub full_bytes: Vec<u8>,
    pub index_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
pub struct Caps {
    pub bytes: i64,
    pub questions: i64,
    pub records: i64,
    pub banks: i64,
}
pub fn caps_of(x: &PoolCtx) -> Res<Caps> {
    Ok(Caps { bytes: x.limit_of("*", "circle_bytes")?, questions: x.limit_of("*", "circle_questions")?, records: x.limit_of("*", "circle_records")?, banks: x.limit_of("*", "circle_banks")? })
}

/// The bank as it is NOW.
#[derive(Debug, Clone)]
pub struct Bank {
    pub id: String,
    pub head_version: i64,
    pub head_version_id: Option<String>,
    pub author_id: String,
    pub license: String,
    pub updated_at: i64,
    pub hide_key: i64,
}
pub fn fresh_bank(x: &PoolCtx, cid: &str, bid: &str) -> Res<Bank> {
    x.conn
        .query_row(
            "SELECT id, head_version, head_version_id, author_id, license, updated_at, hide_key FROM circle_banks WHERE id = ?1 AND circle_id = ?2 AND purged_at IS NULL AND head_version > 0",
            params![bid, cid],
            |r| Ok(Bank { id: r.get(0)?, head_version: r.get(1)?, head_version_id: r.get(2)?, author_id: r.get(3)?, license: r.get(4)?, updated_at: r.get(5)?, hide_key: r.get(6)? }),
        )
        .optional()?
        .ok_or_else(|| not_found().into())
}

pub fn license_author_only(license: &str) -> PoolError {
    PoolError::new(422, "license_author_only").with("license", json!(license)).with("message", json!("Only the bank's author chooses its licence. Send your version with the bank's current licence."))
}

/// 409 conflict with the true head (who sent it, when).
pub fn conflict(x: &PoolCtx, bid: &str) -> Res<PoolError> {
    let h: Option<(i64, Option<i64>, Option<String>, Option<i64>)> = x
        .conn
        .query_row(
            "SELECT b.head_version, v.committed_at, u.display_name, u.deleted_at FROM circle_banks b LEFT JOIN circle_versions v ON v.id = b.head_version_id LEFT JOIN users u ON u.id = v.committer_id WHERE b.id = ?1",
            [bid],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    Ok(PoolError::new(409, "conflict")
        .with("head_version", json!(h.as_ref().map(|h| h.0)))
        .with("head_at", json!(h.as_ref().and_then(|h| h.1)))
        .with("head_by", json!(h.as_ref().map(|h| name_of(h.2.as_deref(), h.3))))
        .with("message", json!("The circle bank changed since you got it. Get the latest changes and merge them, then send again.")))
}

fn few(xs: &[usize]) -> Value {
    json!({ "count": xs.len(), "first": xs.iter().take(FLAG_DETAIL_MAX).collect::<Vec<_>>() })
}

/// Questions a copyright re-upload block held count toward the account's held-publish budget; past it the administrator is flagged.
pub fn charge_held_publishes(x: &PoolCtx, me: &str, n: i64) -> Res<()> {
    let held: Option<i64> = x
        .conn
        .query_row(
            &format!(
                "INSERT INTO usage_daily (user_id, day, metric, count) SELECT ?1, ?2, 'held_publishes', ?3 WHERE {}
       ON CONFLICT (user_id, day, metric) DO UPDATE SET count = count + excluded.count RETURNING count",
                alive_sql("?1")
            ),
            params![me, day_of(x.now), n],
            |r| r.get(0),
        )
        .optional()?;
    if held.unwrap_or(0) >= HELD_PUBLISHES_PER_DAY {
        x.cap_alert(me, "held_publishes")?;
    }
    Ok(())
}

pub fn full_key_of(cid: &str, bid: &str, vid: &str) -> (String, String) {
    (format!("circles/{cid}/{bid}/{vid}.json"), format!("circles/{cid}/{bid}/{vid}.idx.json"))
}

/// Stage, put, commit (one transaction whose first statement is the compare-and-set), or clean up and say why it lost. The answer is the
/// stored result (`held`: the bank is hidden from its members now; set in the same transaction), as (status, body).
pub fn commit_version(x: &PoolCtx, store: &BlobStore, s: &CommitSpec) -> Res<(u16, Value)> {
    let (me, cid, bid, vid, version, now) = (s.me.as_str(), s.cid.as_str(), s.bid.as_str(), s.vid.as_str(), s.version, x.now);
    let caps = caps_of(x)?;
    let day0 = day_of(now) * DAY_MS;
    let flag_cap = x.limit_of(me, "circle_send_flags")?;
    let (full_key, index_key) = full_key_of(cid, bid, vid);
    let int = |k: &str| s.result.get(k).and_then(Value::as_i64).unwrap_or(0);
    let staged = x.conn.execute(
        &format!(
            "INSERT INTO circle_versions (id, bank_id, circle_id, version, commit_id, request_sha256, committer_id, base_version, merged_from, rollback_of, message, license, status, full_key, index_key, bytes, question_count, added, changed, removed, origins, created_at, listing, strip_opts)
       SELECT ?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'staging', ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22 WHERE {}",
            alive_sql("?6")
        ),
        params![
            vid, bid, cid, s.commit_id, s.request_sha, me, s.base, s.merged_from, s.rollback_of, s.message, s.license, full_key, index_key, s.bytes, s.n, int("added"), int("changed"), int("removed"),
            if s.origins.is_empty() { None } else { Some(serde_json::to_string(&s.origins).unwrap()) }, now, s.listing, s.strip_opts
        ],
    )?;
    if staged != 1 {
        x.throw_if_erased(me)?;
        return Err(PoolError::new(409, "publish_in_progress").header("Retry-After", "5").into());
    }
    store.put(&full_key, &s.full_bytes).map_err(|_| PoolError::new(500, "server_error"))?;
    store.put(&index_key, &s.index_bytes).map_err(|_| PoolError::new(500, "server_error"))?;
    let won = format!("EXISTS (SELECT 1 FROM circle_banks wb WHERE wb.id = '{bid}' AND wb.head_version_id = '{vid}')");
    let keys_json = serde_json::to_string(&s.privacy.keys).unwrap();
    let p_shas = serde_json::to_string(&s.privacy.shas).unwrap();
    let near_n = s.privacy.near.len() as i64;
    let tx = x.conn.unchecked_transaction()?;
    // (the sequence number this commit's "first sent" records carry; a lost commit leaves a harmless gap)
    tx.execute("UPDATE pool_seq SET n = n + 1 WHERE id = 1", [])?;
    let cas = if s.is_new {
        let m = s.meta.as_ref().expect("a new bank has listing fields");
        tx.execute(
            &format!(
                "INSERT INTO circle_banks (id, circle_id, author_id, title, subject, course, grade, license, head_version, head_version_id, question_count, has_images, created_at, updated_at, hide_key)
           SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?10, ?11, ?12, ?12, ?19
            WHERE {} AND {} AND {}
              AND {} < ?13 AND {} + ?14 <= ?15 AND {} + ?10 <= ?16
              AND (SELECT qkeys FROM circles WHERE id = ?2) + ?17 <= ?18
              AND EXISTS (SELECT 1 FROM circle_versions sv WHERE sv.id = ?9 AND sv.status = 'staging')",
                open_sql("?2"),
                active_sql("?2", "?3", &["owner", "editor"]),
                alive_sql("?3"),
                banks_sql("?2"),
                bytes_sql("?2"),
                questions_sql("?2", "?1")
            ),
            params![bid, cid, me, m.title, m.subject, m.course, m.grade, s.license, vid, s.n, s.has_images, now, caps.banks, s.bytes, caps.bytes, caps.questions, s.new_keys, caps.records, s.hide_key.unwrap_or(0)],
        )?
    } else {
        let rights = if s.kind == Kind::Send { active_sql("?2", "?3", &["owner", "editor"]) } else { rollback_rights_sql("circle_banks", "?2", "?3") };
        let m = s.meta.as_ref();
        // Any editor or the owner (a rollback: the owner, or the author while an editor); the licence is the author's to change; a held bank,
        // or one whose author is under review, takes versions from its author only.
        tx.execute(
            &format!(
                "UPDATE circle_banks SET head_version = head_version + 1, head_version_id = ?4, title = COALESCE(?5, title), subject = COALESCE(?6, subject), course = COALESCE(?7, course), grade = COALESCE(?8, grade),
                  license = ?9, question_count = ?10, has_images = ?11, updated_at = ?12
            WHERE id = ?1 AND circle_id = ?2 AND head_version = ?13 AND removed_at IS NULL AND purged_at IS NULL
              AND (author_id = ?3 OR license = ?9)
              AND (author_id = ?3 OR ((NOT EXISTS (SELECT 1 FROM circle_holds hh WHERE hh.bank_id = ?1) OR (NOT {} AND {})) AND {}))
              AND {rights} AND {} AND {}
              AND {} - {} + ?14 <= ?15 AND {} + ?10 <= ?16
              AND (SELECT qkeys FROM circles WHERE id = ?2) + ?17 <= ?18
              AND (?20 IS NULL OR hide_key = ?20)
              AND EXISTS (SELECT 1 FROM circle_versions sv WHERE sv.id = ?4 AND sv.status = 'staging')",
                blocking_hold_sql("?1"),
                active_sql("?2", "?3", &["owner"]),
                author_ok_sql("circle_banks"),
                open_sql("?2"),
                alive_sql("?3"),
                bytes_sql("?2"),
                prunable_sql("?1", "?19"),
                questions_sql("?2", "?1")
            ),
            params![
                bid, cid, me, vid, m.map(|m| m.title.clone()), m.map(|m| m.subject.clone()), m.map(|m| m.course.clone()), m.map(|m| m.grade.clone()), s.license, s.n, s.has_images, now, s.base, s.bytes, caps.bytes,
                caps.questions, s.new_keys, caps.records, version - KEPT_VERSIONS, s.hide_key
            ],
        )?
    };
    tx.execute(&format!("UPDATE circle_versions SET status = 'previous' WHERE bank_id = ?1 AND status = 'current' AND id <> ?2 AND {won}"), params![bid, vid])?;
    // The last KEPT_VERSIONS versions are kept in full; older ones keep their history line. Never while the bank is held, never a version an
    // open report names, never evidence an open flag pins. Before this commit's own holds are set.
    tx.execute(
        &format!(
            "UPDATE circle_versions SET status = 'pruned', pruned_at = ?3 WHERE bank_id = ?1 AND status = 'previous' AND version <= ?2 AND {won}
           AND NOT {}
           AND NOT EXISTS (SELECT 1 FROM circle_reports pr WHERE pr.bank_id = ?1 AND pr.resolved_at IS NULL AND pr.version = circle_versions.version)
           AND NOT {}",
            blocking_hold_sql("?1"),
            pinned_evidence_sql("circle_versions")
        ),
        params![bid, version - KEPT_VERSIONS, now],
    )?;
    tx.execute(&format!("DELETE FROM circle_version_blobs WHERE version_id IN ({PRUNED_NOW_SQL}) AND {won}"), params![bid, now])?;
    // Confirmed student data is re-checked INSIDE the commit, against this version's own keys. A version without it lifts the hold an
    // earlier one caused; an admin's own hold and report holds stay.
    tx.execute(&format!("DELETE FROM circle_holds WHERE bank_id = ?1 AND source = 'privacy' AND {won} AND NOT {}", privacy_sql("?2", "?3")), params![bid, keys_json, p_shas])?;
    tx.execute(
        &format!("INSERT OR IGNORE INTO circle_holds (bank_id, reason, source, at, by) SELECT ?1, 'student_data', 'privacy', ?4, NULL WHERE {won} AND {}", privacy_sql("?2", "?3")),
        params![bid, keys_json, p_shas, now],
    )?;
    tx.execute(&format!("DELETE FROM circle_holds WHERE bank_id = ?1 AND source = 'privacy_near' AND ?2 = 0 AND {won}"), params![bid, near_n])?;
    tx.execute(&format!("INSERT OR IGNORE INTO circle_holds (bank_id, reason, source, at, by) SELECT ?1, 'student_data', 'privacy_near', ?3, NULL WHERE ?2 > 0 AND {won}"), params![bid, near_n, now])?;
    // A copyright re-upload block, by the public path's rules, re-checked here: the same pattern as student data.
    tx.execute(
        &format!("DELETE FROM circle_holds WHERE bank_id = ?1 AND source = 'blocklist' AND {won} AND NOT {}", copyright_sql("?2", "?3", "?4", "?5")),
        params![bid, keys_json, p_shas, now, me],
    )?;
    tx.execute(
        &format!("INSERT OR IGNORE INTO circle_holds (bank_id, reason, source, at, by) SELECT ?1, 'copyright', 'blocklist', ?4, NULL WHERE {won} AND {}", copyright_sql("?2", "?3", "?4", "?5")),
        params![bid, keys_json, p_shas, now, me],
    )?;
    tx.execute(
        &format!(
            "UPDATE circle_versions SET status = 'current', version = ?2, seq = (SELECT n FROM pool_seq WHERE id = 1), committed_at = ?3,
                withheld_at = CASE WHEN EXISTS (SELECT 1 FROM circle_holds w1 WHERE w1.bank_id = ?5 AND w1.source IN {SEND_HOLD_SOURCES}) THEN ?3 END,
                result_json = json_set(?4,
                  '$.held', json(CASE WHEN EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = ?5) THEN 'true' ELSE 'false' END),
                  '$.withheld', json(CASE WHEN EXISTS (SELECT 1 FROM circle_holds w2 WHERE w2.bank_id = ?5 AND w2.source IN {SEND_HOLD_SOURCES}) THEN 'true' ELSE 'false' END))
          WHERE id = ?1 AND status = 'staging' AND {won}"
        ),
        params![vid, version, now, s.result.to_string(), bid],
    )?;
    tx.execute(&listing_refresh_sql(&won), [bid])?;
    tx.execute(&format!("INSERT OR IGNORE INTO circle_version_blobs (version_id, sha256) SELECT ?1, value FROM json_each(?2) WHERE {won}"), params![vid, serde_json::to_string(&s.shas).unwrap()])?;
    let qkeys = |sql: String, p: &[&dyn rusqlite::ToSql]| -> rusqlite::Result<()> {
        tx.execute(&sql, p)?;
        tx.execute(&format!("UPDATE circles SET qkeys = qkeys + changes() WHERE id = ?1 AND {won}"), [cid])?;
        Ok(())
    };
    let hashes_json = serde_json::to_string(&s.hashes).unwrap();
    qkeys(
        format!("INSERT OR IGNORE INTO circle_qkeys (circle_id, match_hash, bank_id, first_version, first_seq, first_by) SELECT ?1, j.value, ?2, ?3, (SELECT n FROM pool_seq WHERE id = 1), ?4 FROM json_each(?5) j WHERE {won}"),
        &[&cid, &bid, &version, &me, &hashes_json],
    )?;
    // The served form's key-free hashes where a label stripped for viewers changes them, under the first sender of each full text.
    let pairs: Vec<[&String; 2]> = s.pairs.iter().map(|(a, b)| [a, b]).collect();
    qkeys(stripped_qkeys_sql("?1", "?2", "?3", &won), &[&cid, &bid, &serde_json::to_string(&pairs).unwrap()])?;
    // The own-work affirmation, recorded with the version as a publish's is; a rollback sends no new work.
    if s.kind == Kind::Send {
        tx.execute(&format!("INSERT OR IGNORE INTO attestations (publish_id, item_id, user_id, at, terms_version) SELECT ?1, ?2, ?3, ?4, ?5 WHERE {won}"), params![vid, bid, me, now, POOL_TERMS_VERSION])?;
    }
    // One flag per version that matches confirmed student data (ids, positions and counts only), with the matching block hashes, and at most
    // circle_send_flags a day from one sender in one circle.
    let rb = |mut d: Value| {
        if let Some(r) = s.rollback_of {
            d["rollback_of"] = json!(r);
        }
        d.to_string()
    };
    tx.execute(
        &format!(
            "INSERT INTO admin_flags (id, kind, item_id, publish_id, user_id, detail, at)
         SELECT ?1, 'privacy_text_match', ?2, ?3, ?4, ?5, ?6 WHERE {won} AND EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = ?2 AND h.source IN ('privacy', 'privacy_near'))
           AND {}",
            flag_room_sql("?4", "?7", "?8", "?9")
        ),
        params![new_id(), bid, vid, me, rb(json!({ "circle_id": cid, "version": version, "positions": few(&s.privacy.exact), "near": few(&s.privacy.near), "hashes": s.privacy.hits })), now, cid, day0, flag_cap],
    )?;
    tx.execute(
        &format!(
            "INSERT INTO admin_flags (id, kind, item_id, publish_id, user_id, detail, at)
         SELECT ?1, 'blocklist', ?2, ?3, ?4, ?5, ?6 WHERE {won} AND EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = ?2 AND h.source = 'blocklist')
           AND {}",
            flag_room_sql("?4", "?7", "?8", "?9")
        ),
        params![new_id(), bid, vid, me, rb(json!({ "circle_id": cid, "version": version, "positions": few(&s.privacy.copyright), "actions": s.privacy.actions, "hashes": s.privacy.chits })), now, cid, day0, flag_cap],
    )?;
    // A version withheld past the day's flags is named on the bank's cap flag.
    let dedup = cap_dedup(bid, me, now);
    tx.execute(
        &cap_flag_sql(&won),
        params![
            new_id(), bid, me, cid, version, vid, serde_json::to_string(&s.privacy.actions).unwrap(), if s.privacy.exact.len() + s.privacy.near.len() > 0 { "true" } else { "false" }, dedup, now
        ],
    )?;
    // The evidence kept in full stays within its bound.
    evidence_bound(x.conn, cid, now, caps.bytes, &won)?;
    let action = if s.kind == Kind::Rollback { "circle.rollback" } else if s.is_new { "circle.bank_add" } else { "circle.version" };
    let mut detail = json!({ "bank_id": bid, "version": version, "version_id": vid });
    if let (Some(d), Some(a)) = (detail.as_object_mut(), s.audit_detail.as_object()) {
        d.extend(a.clone());
    }
    audit(x.conn, &AuditEntry { actor_id: Some(me), actor_kind: "user", action, item_id: Some(cid), target_user_id: Some(me), detail: Some(detail), note: None }, now, Some(&won))?;
    type Out = (String, bool, bool, bool, bool, Option<bool>);
    let out: Option<Out> = tx
        .query_row(
            "SELECT result_json, EXISTS (SELECT 1 FROM admin_flags f WHERE f.publish_id = ?1 AND f.kind = 'privacy_text_match') AS flagged,
                EXISTS (SELECT 1 FROM admin_flags f WHERE f.publish_id = ?1 AND f.kind = 'blocklist') AS cflagged,
                EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = ?2 AND h.source IN ('privacy', 'privacy_near')) AS pheld,
                EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = ?2 AND h.source = 'blocklist') AS cheld,
                (SELECT cf.at = ?3 AND json_array_length(cf.detail, '$.versions') = 1 FROM admin_flags cf WHERE cf.dedup = ?4) AS capnew
           FROM circle_versions WHERE id = ?1 AND status = 'current'",
            params![vid, bid, now, dedup],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get::<_, Option<bool>>(5)?)),
        )
        .optional()?;
    tx.commit()?;
    if cas != 1 {
        // Lost: the staging row and its objects go now (recorded as orphans first: a failed delete is retried hourly); the reason is read
        // from the state.
        let keys = vec![full_key, index_key];
        let tx = x.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM circle_versions WHERE id = ?1 AND status = 'staging'", [vid])?;
        tx.execute("INSERT OR IGNORE INTO r2_orphans (key, at) SELECT value, ?2 FROM json_each(?1)", params![serde_json::to_string(&keys).unwrap(), now])?;
        tx.commit()?;
        delete_keys(x, store, &keys)?;
        return Err(why_lost(x, s.kind, me, cid, if s.is_new { None } else { Some(bid) }, &Lost { base: s.base, license: &s.license, bytes: s.bytes, n: s.n, new_keys: s.new_keys, hide_key: s.hide_key, caps })?.into());
    }
    let Some((result_json, flagged, cflagged, _pheld, cheld, capnew)) = out else {
        let mut r = s.result.clone();
        r["held"] = json!(false);
        return Ok((if s.is_new { 201 } else { 200 }, r));
    };
    if flagged {
        x.alert_budgeted(PoolNotice::PrivacyTextMatch { item_id: bid.into(), held: true, circle_id: Some(cid.into()), version: Some(version) })?;
    }
    if cflagged {
        x.alert_budgeted(PoolNotice::CircleCopyrightMatch { item_id: bid.into(), circle_id: cid.into(), version })?;
    }
    // The matching questions count toward the account's held-publish budget (charged for every match, flagged or past the day's flags).
    if cheld {
        charge_held_publishes(x, me, (s.privacy.copyright.len() as i64).max(1))?;
    }
    // Past circle_send_flags the version is withheld without a flag; the admin is told once a day per bank.
    if capnew == Some(true) {
        x.alert_budgeted(PoolNotice::Cap { user_id: me.into(), metric: "circle_send_flags".into() })?;
    }
    Ok((if s.is_new { 201 } else { 200 }, serde_json::from_str(&result_json).unwrap_or(Value::Null)))
}

pub struct Lost<'a> {
    pub base: Option<i64>,
    pub license: &'a str,
    pub bytes: i64,
    pub n: i64,
    pub new_keys: i64,
    pub hide_key: Option<i64>,
    pub caps: Caps,
}

/// Why a commit's compare-and-set changed nothing, from the state now.
pub fn why_lost(x: &PoolCtx, kind: Kind, me: &str, cid: &str, bid: Option<&str>, w: &Lost) -> Res<PoolError> {
    x.throw_if_erased(me)?;
    #[derive(Default)]
    struct S {
        closed_at: Option<i64>,
        close_reason: Option<String>,
        undo_until: Option<i64>,
        qkeys: i64,
        role: Option<String>,
        used: i64,
        prunable: i64,
        questions: i64,
        banks: i64,
        head_version: Option<i64>,
        removed_at: Option<i64>,
        removed_reason: Option<String>,
        author_id: Option<String>,
        license: Option<String>,
        hide_key: Option<i64>,
        held: Option<bool>,
        blocking: Option<bool>,
        author_ok: Option<bool>,
    }
    let sql = format!(
        "SELECT ci.closed_at, ci.close_reason, ci.undo_until, ci.qkeys, m.role, {} AS used, {} AS prunable, {} AS questions, {} AS banks,
              b.head_version, b.removed_at, b.removed_reason, b.author_id, b.license, b.hide_key, EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = b.id) AS held, {} AS blocking, {} AS author_ok
         FROM circles ci LEFT JOIN circle_members m ON m.circle_id = ci.id AND m.user_id = ?2 AND m.left_at IS NULL LEFT JOIN circle_banks b ON b.id = ?3 AND b.circle_id = ci.id
        WHERE ci.id = ?1",
        bytes_sql("?1"),
        prunable_sql("?3", "?4"),
        questions_sql("?1", "?3"),
        banks_sql("?1"),
        blocking_hold_sql("b.id"),
        author_ok_sql("b")
    );
    let s = x
        .conn
        .query_row(&sql, params![cid, me, bid.unwrap_or(""), w.base.unwrap_or(0) + 1 - KEPT_VERSIONS], |r| {
            Ok(S {
                closed_at: r.get(0)?,
                close_reason: r.get(1)?,
                undo_until: r.get(2)?,
                qkeys: r.get(3)?,
                role: r.get(4)?,
                used: r.get(5)?,
                prunable: r.get(6)?,
                questions: r.get(7)?,
                banks: r.get(8)?,
                head_version: r.get(9)?,
                removed_at: r.get(10)?,
                removed_reason: r.get(11)?,
                author_id: r.get(12)?,
                license: r.get(13)?,
                hide_key: r.get(14)?,
                held: r.get(15)?,
                blocking: r.get(16)?,
                author_ok: r.get(17)?,
            })
        })
        .optional()?;
    let Some(s) = s else { return Ok(not_found()) };
    let Some(role) = s.role.clone() else { return Ok(not_found()) };
    if s.closed_at.is_some() {
        return Ok(PoolError::new(409, "circle_closed").with("reason", json!(s.close_reason)).with("undo_until", json!(s.undo_until)));
    }
    let needs = |needed: &str| PoolError::new(403, "circle_role_required").with("needed", json!(needed)).with("role", json!(role));
    if kind == Kind::Rollback && !(role == "owner" || (s.author_id.as_deref() == Some(me) && role == "editor")) {
        return Ok(needs(OWNER_OR_AUTHOR));
    }
    if role != "owner" && role != "editor" {
        return Ok(needs("editor"));
    }
    if let Some(bid) = bid {
        if s.removed_at.is_some() {
            return Ok(PoolError::new(409, "bank_removed").with("reason", json!(s.removed_reason)));
        }
        let not_author = s.author_id.as_deref() != Some(me);
        if not_author && ((s.held == Some(true) && !(role == "owner" && s.blocking == Some(false))) || s.author_ok != Some(true)) {
            return Ok(not_found());
        }
        if s.head_version != w.base {
            return conflict(x, bid);
        }
        if not_author && s.license.as_deref() != Some(w.license) {
            return Ok(license_author_only(s.license.as_deref().unwrap_or("")));
        }
        // The head did not move, the answer-key setting did: send again (not a merge).
        if let (Some(want), Some(have)) = (w.hide_key, s.hide_key) {
            if have != want {
                return Ok(PoolError::new(409, "hide_key_changed").with("hide_key", json!(have == 1)).with("head_version", json!(s.head_version)).with(
                    "message",
                    json!(if have == 1 {
                        "The bank's answer keys were hidden from viewers while this was being sent. Nothing was stored: send it again, with the decisions for its viewers' form."
                    } else {
                        "The bank's answer-key setting changed while this was being sent. Nothing was stored: send it again."
                    }),
                ));
            }
        }
    }
    let c = w.caps;
    if bid.is_none() && s.banks >= c.banks {
        return Ok(PoolError::new(422, "circles_limit").with("metric", json!("circle_banks")).with("used", json!(s.banks)).with("limit", json!(c.banks)).with("message", json!(format!("This circle has {} banks, the most allowed.", c.banks))));
    }
    if s.used - s.prunable + w.bytes > c.bytes {
        return Ok(PoolError::new(413, "quota_exceeded").with("scope", json!("circle")).with("used", json!(s.used - s.prunable)).with("needed", json!(w.bytes)).with("limit", json!(c.bytes)).with("message", json!(QUOTA_MESSAGE)));
    }
    if s.questions + w.n > c.questions {
        return Ok(PoolError::new(422, "circle_questions_limit").with("used", json!(s.questions)).with("needed", json!(w.n)).with("limit", json!(c.questions)).with("message", json!(format!("A circle holds at most {} questions across its banks.", c.questions))));
    }
    if s.qkeys + w.new_keys > c.records {
        return Ok(PoolError::new(413, "circle_records_full").with("used", json!(s.qkeys)).with("needed", json!(w.new_keys)).with("limit", json!(c.records)).with("message", json!("This circle has recorded as many different questions as it can keep.")));
    }
    if let Some(bid) = bid {
        return conflict(x, bid);
    }
    Ok(PoolError::new(409, "conflict").with("message", json!("Something changed meanwhile; try again.")))
}

#[allow(dead_code)]
fn _unused(_: Fail) {}

#[cfg(test)]
#[path = "circlecommit_tests.rs"]
mod tests;
