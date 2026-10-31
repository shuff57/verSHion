//! The SQL fragments and the evidence bound of circle sends (worker/src/pool/circlesend.ts, the constants and statement builders at its top).
use super::circlesql::*;
use super::served::rightful_exempts;
use rusqlite::{Connection, params};

pub const MESSAGE_MAX: usize = 500;

/// The circle's kept bytes: one indexed SUM over its current and previous versions.
pub fn bytes_sql(cid: &str) -> String {
    format!("(SELECT COALESCE(SUM(sv.bytes), 0) FROM circle_versions sv WHERE sv.circle_id = {cid} AND sv.status IN ('current', 'previous'))")
}
/// Questions across the circle's current versions, every bank not purged but `except`.
pub fn questions_sql(cid: &str, except: &str) -> String {
    format!("(SELECT COALESCE(SUM(sb.question_count), 0) FROM circle_banks sb WHERE sb.circle_id = {cid} AND sb.purged_at IS NULL AND sb.id <> {except})")
}
/// Banks of the circle that are not purged (removed ones count until purged).
pub fn banks_sql(cid: &str) -> String {
    format!("(SELECT COUNT(*) FROM circle_banks nb WHERE nb.circle_id = {cid} AND nb.purged_at IS NULL)")
}
/// A confirmed student-data block matches one of the block keys (JSON array `keys`) or images (`shas`); each by the primary key.
pub fn privacy_sql(keys: &str, shas: &str) -> String {
    format!(
        "(EXISTS (SELECT 1 FROM privacy_blocks pk WHERE pk.kind = 'match' AND pk.hash IN (SELECT value FROM json_each({keys})))
    OR EXISTS (SELECT 1 FROM privacy_blocks pi WHERE pi.kind = 'blob' AND pi.hash IN (SELECT value FROM json_each({shas}))))"
    )
}
/// An ACTIVE copyright re-upload block on blocked_hashes row `bh` that holds content sent by `sender` (a parameter reference): never one
/// the administrator exempted the sender from, and never the rightful item's own material for its author or an account it was served to.
pub fn copyright_active(bh: &str, now: &str, sender: &str) -> String {
    format!(
        "{bh}.lifted_at IS NULL AND ({bh}.expires_at IS NULL OR {bh}.expires_at > {now}) AND {bh}.action_id IS NOT NULL
   AND NOT EXISTS (SELECT 1 FROM dmca_exempt ce WHERE ce.action_id = {bh}.action_id AND ce.user_id = {sender})
   AND NOT EXISTS (SELECT 1 FROM dmca_actions ca WHERE ca.id = {bh}.action_id AND ca.rightful_item_id IS NOT NULL AND {})",
        rightful_exempts("ca.rightful_item_id", bh, sender, "NULL")
    )
}
/// A copyright re-upload block holds one of the block keys (JSON array `keys`) or images (`shas`); each by the primary key.
pub fn copyright_sql(keys: &str, shas: &str, now: &str, sender: &str) -> String {
    format!(
        "(EXISTS (SELECT 1 FROM blocked_hashes ck WHERE ck.kind = 'match' AND ck.hash IN (SELECT value FROM json_each({keys})) AND {})
    OR EXISTS (SELECT 1 FROM blocked_hashes ci WHERE ci.kind = 'blob' AND ci.hash IN (SELECT value FROM json_each({shas})) AND {}))",
        copyright_active("ck", now, sender),
        copyright_active("ci", now, sender)
    )
}
/// The bank's listing fields from its newest version that is not withheld (with none, the fields are blank). `?1` is the bank.
pub fn listing_refresh_sql(cond: &str) -> String {
    format!(
        "UPDATE circle_banks SET (title, subject, course, grade) = (
     SELECT COALESCE(json_extract(x.l, '$.title'), ''), COALESCE(json_extract(x.l, '$.subject'), ''), COALESCE(json_extract(x.l, '$.course'), ''), COALESCE(json_extract(x.l, '$.grade'), '')
       FROM (SELECT (SELECT lv.listing FROM circle_versions lv WHERE lv.bank_id = ?1 AND lv.version IS NOT NULL AND lv.status <> 'staging' AND lv.withheld_at IS NULL AND lv.listing IS NOT NULL
                      ORDER BY lv.version DESC LIMIT 1) AS l) x)
    WHERE id = ?1 AND {cond}"
    )
}
/// The bytes of bank `bid`'s versions that a commit making version `up_to + KEPT_VERSIONS` prunes in the same batch (past the 20 kept,
/// never while the bank is held, never one an open report names). The cap is checked net of them.
pub fn prunable_sql(bid: &str, up_to: &str) -> String {
    format!(
        "(SELECT COALESCE(SUM(pv.bytes), 0) FROM circle_versions pv WHERE pv.bank_id = {bid} AND pv.status = 'previous' AND pv.version <= {up_to}
      AND NOT {}
      AND NOT EXISTS (SELECT 1 FROM circle_reports xr WHERE xr.bank_id = {bid} AND xr.resolved_at IS NULL AND xr.version = pv.version)
      AND NOT {})",
        blocking_hold_sql(bid),
        pinned_evidence_sql("pv")
    )
}
/// A withheld version whose student-data or copyright flag is still open is the admin's evidence: no commit prunes it. `v` is the version row.
pub fn pinned_evidence_sql(v: &str) -> String {
    format!("({v}.withheld_at IS NOT NULL AND EXISTS (SELECT 1 FROM admin_flags ef WHERE ef.publish_id = {v}.id AND {}))", evidence_flag_sql("ef"))
}
/// An open student-data or copyright flag whose evidence the administrator still needs (one marked `evidence_captured` pins nothing).
pub fn evidence_flag_sql(f: &str) -> String {
    format!("({f}.kind IN {SEND_FLAG_KINDS} AND {f}.resolved_at IS NULL AND COALESCE({f}.detail ->> '$.evidence_captured', 0) = 0)")
}

/// The evidence kept in full is bounded per bank, per circle, per sender and by a quarter of the circle's bytes.
pub const EVIDENCE_PER_BANK: i64 = 5;
pub const EVIDENCE_PER_CIRCLE: i64 = 20;
pub const EVIDENCE_SENDER_BANK: i64 = 2;
pub const EVIDENCE_SENDER_CIRCLE: i64 = 5;
/// The block hashes a flag records.
pub const EVIDENCE_HASHES: usize = 20;

/// Keeps the evidence bound for circle `cid` at `now`: prunes the withheld previous versions beyond it, marks their flags
/// `evidence_pruned`, and drops their image rows. `cond` is a SQL condition (the batch's own winning marker; "1" to always run).
pub fn evidence_bound(conn: &Connection, cid: &str, now: i64, circle_bytes: i64, cond: &str) -> rusqlite::Result<()> {
    let pruned_now = "SELECT pv.id FROM circle_versions pv INDEXED BY circle_versions_prune WHERE pv.pruned_at = ?2 AND pv.status = 'pruned' AND pv.full_key <> '' AND pv.circle_id = ?1 AND pv.withheld_at IS NOT NULL";
    conn.execute(
        &format!(
            "UPDATE circle_versions SET status = 'pruned', pruned_at = ?2 WHERE id IN (
           SELECT x.id FROM (
             SELECT h.id, h.pb, h.pc,
                    SUM(h.pb) OVER (PARTITION BY h.bank_id) AS npb,
                    ROW_NUMBER() OVER (PARTITION BY h.bank_id, h.pb ORDER BY h.version DESC) AS rub,
                    SUM(h.pc) OVER () AS npc,
                    ROW_NUMBER() OVER (PARTITION BY h.pc ORDER BY h.committed_at DESC, h.id DESC) AS ruc,
                    SUM(h.bytes) OVER (PARTITION BY h.pc ORDER BY h.committed_at DESC, h.id DESC ROWS UNBOUNDED PRECEDING) AS upto
               FROM (
                 SELECT ev.id, ev.bank_id, ev.version, ev.committed_at, ev.bytes,
                        ROW_NUMBER() OVER (PARTITION BY ev.bank_id, ev.committer_id ORDER BY ev.version DESC) <= ?6 AS pb,
                        ROW_NUMBER() OVER (PARTITION BY ev.committer_id ORDER BY ev.committed_at DESC, ev.id DESC) <= ?7 AS pc
                   FROM circle_versions ev
                  WHERE ev.circle_id = ?1 AND ev.withheld_at IS NOT NULL AND ev.full_key <> '' AND ev.status = 'previous'
                    AND EXISTS (SELECT 1 FROM admin_flags ef WHERE ef.publish_id = ev.id AND {})
                    AND NOT {}
                    AND NOT EXISTS (SELECT 1 FROM circle_reports er WHERE er.bank_id = ev.bank_id AND er.resolved_at IS NULL AND er.version = ev.version)) h) x
            WHERE (x.pb = 0 AND x.rub > MAX(0, ?3 - x.npb)) OR (x.pc = 0 AND (x.ruc > MAX(0, ?4 - x.npc) OR x.upto > ?5)))
           AND {cond}",
            evidence_flag_sql("ef"),
            blocking_hold_sql("ev.bank_id")
        ),
        params![cid, now, EVIDENCE_PER_BANK, EVIDENCE_PER_CIRCLE, circle_bytes / 4, EVIDENCE_SENDER_BANK, EVIDENCE_SENDER_CIRCLE],
    )?;
    conn.execute(
        &format!("UPDATE admin_flags SET detail = json_set(detail, '$.evidence_pruned', json('true'), '$.evidence_pruned_at', ?2) WHERE kind IN {SEND_FLAG_KINDS} AND resolved_at IS NULL AND json_valid(detail) AND publish_id IN ({pruned_now}) AND {cond}"),
        params![cid, now],
    )?;
    conn.execute(&format!("DELETE FROM circle_version_blobs WHERE version_id IN ({pruned_now}) AND {cond}"), params![cid, now])?;
    Ok(())
}

/// The bank-level `cap` flag that names a version withheld past the sender's circle_send_flags (no flag of its own): one flag per bank,
/// sender and day (`dedup`). Binds: ?1 id, ?2 bank, ?3 sender, ?4 circle, ?5 version, ?6 version id, ?7 actions (JSON), ?8 student data
/// ('true'|'false'), ?9 dedup, ?10 now. Only when the version is withheld and no flag of its own was raised in this batch.
pub fn cap_flag_sql(cond: &str) -> String {
    format!(
        "INSERT INTO admin_flags (id, kind, item_id, user_id, detail, dedup, at)
   SELECT ?1, 'cap', ?2, ?3, json_object('metric', 'circle_send_flags', 'circle_id', ?4, 'bank_id', ?2, 'versions', json_array(json_object('version', ?5, 'version_id', ?6, 'actions', json(?7), 'student_data', json(?8)))), ?9, ?10
    WHERE {cond} AND EXISTS (SELECT 1 FROM circle_versions cv WHERE cv.id = ?6 AND cv.withheld_at IS NOT NULL)
      AND NOT EXISTS (SELECT 1 FROM admin_flags vf WHERE vf.publish_id = ?6 AND vf.kind IN {SEND_FLAG_KINDS} AND vf.at = ?10)
   ON CONFLICT (dedup) DO UPDATE SET detail = json_set(detail, '$.versions[#]', json(excluded.detail -> '$.versions[0]'))"
    )
}
pub fn cap_dedup(bid: &str, me: &str, now: i64) -> String {
    format!("circle_send_flags:{bid}:{me}:{}", super::sqlfrag::day_of(now))
}
/// True while account `me` has raised fewer than `cap` send flags in circle `cid` since `day0` (references are SQL parameters).
pub fn flag_room_sql(me: &str, cid: &str, day0: &str, cap: &str) -> String {
    format!("((SELECT COUNT(*) FROM admin_flags sf INDEXED BY admin_flags_user_at WHERE sf.user_id = {me} AND sf.at >= {day0} AND sf.kind IN {SEND_FLAG_KINDS} AND sf.detail ->> '$.circle_id' = {cid}) < {cap})")
}
pub const QUOTA_MESSAGE: &str = "This circle's storage is full: every kept version of every bank counts, with its images. The owner (or a bank's author) can trim a bank's older versions, or remove a bank and purge it.";
/// The versions of bank `?1` pruned at `?2` (a commit's or a trim's own), by the prune index.
pub const PRUNED_NOW_SQL: &str = "SELECT pn.id FROM circle_versions pn INDEXED BY circle_versions_prune WHERE pn.pruned_at = ?2 AND pn.status = 'pruned' AND pn.full_key <> '' AND pn.bank_id = ?1";
/// Who may roll a bank back: the owner, or the bank's author while an editor. `b` is the bank row (alias or table).
pub fn rollback_rights_sql(b: &str, cid: &str, me: &str) -> String {
    format!("({} OR ({b}.author_id = {me} AND {}))", active_sql(cid, me, &["owner"]), active_sql(cid, me, &["editor"]))
}

#[cfg(test)]
#[path = "circlesendsql_tests.rs"]
mod tests;
