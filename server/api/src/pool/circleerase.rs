//! The circle part of account erasure and of the daily erasure resweep (worker/src/pool/circles.ts `circleErasureStatements`,
//! `circleResweepStatements`, `OWNERLESS_SQL`). Each statement matches only what is not yet clean, so a rerun changes nothing.
use super::circlebase::CIRCLE_UNDO_MS;
use super::circlemembers::succession;
use rusqlite::{Connection, params};

/// Open circles without an active owner the daily resweep repairs at most a run.
pub const RESWEEP_CIRCLES: usize = 50;

/// The ids of open circles without an active owner, and of owner-closed circles inside their undo window without one (up to
/// RESWEEP_CIRCLES each): the safety net's pick for `circle_resweep`.
pub fn ownerless_circles(conn: &Connection, now: i64) -> rusqlite::Result<Vec<String>> {
    let mut ids: Vec<String> = conn
        .prepare(&format!(
            "SELECT c2.id FROM circles c2 INDEXED BY circles_open WHERE c2.closed_at IS NULL AND c2.purged_at IS NULL
  AND NOT EXISTS (SELECT 1 FROM circle_members o2 WHERE o2.circle_id = c2.id AND o2.role = 'owner' AND o2.left_at IS NULL) LIMIT {RESWEEP_CIRCLES}"
        ))?
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let closed: Vec<String> = conn
        .prepare(&format!(
            "SELECT c3.id FROM circles c3 INDEXED BY circles_purge WHERE c3.purge_after IS NOT NULL AND c3.purged_at IS NULL
  AND c3.close_reason = 'owner_closed' AND c3.undo_until > ?1
  AND NOT EXISTS (SELECT 1 FROM circle_members o3 WHERE o3.circle_id = c3.id AND o3.role = 'owner' AND o3.left_at IS NULL) LIMIT {RESWEEP_CIRCLES}"
        ))?
        .query_map([now], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    for c in closed {
        if !ids.contains(&c) {
            ids.push(c);
        }
    }
    Ok(ids)
}

/// The circle statements of the account-deletion batch, in order: memberships step down (audited), succession and last-member close,
/// its banks removed (when it chose not to keep its shared items) or anonymised, its free text gone, its codes revoked, its mail
/// counters and ownership offers deleted, and finally its membership rows.
pub fn circle_erasure(conn: &Connection, user: &str, now: i64) -> rusqlite::Result<()> {
    let purge_at = now + CIRCLE_UNDO_MS;
    let no_keep = "EXISTS (SELECT 1 FROM pool_prefs WHERE user_id = ?1 AND keep_on_delete = 0)";
    conn.execute(
        "INSERT INTO pool_audit (id, at, actor_id, actor_kind, action, item_id, target_user_id)
         SELECT lower(hex(randomblob(16))), ?2, NULL, 'system', 'circle.member_erased', circle_id, user_id FROM circle_members WHERE user_id = ?1 AND left_at IS NULL",
        params![user, now],
    )?;
    conn.execute("UPDATE circle_members SET left_at = ?2, left_reason = 'account_deleted' WHERE user_id = ?1 AND left_at IS NULL", params![user, now])?;
    succession(conn, "SELECT circle_id FROM circle_members WHERE user_id = ?1", &[&user], now, purge_at)?;
    conn.execute(&format!("UPDATE circle_banks SET removed_at = ?2, removed_reason = 'account_deleted', purge_after = ?3, updated_at = ?2 WHERE author_id = ?1 AND removed_at IS NULL AND purged_at IS NULL AND {no_keep}"), params![user, now, purge_at])?;
    conn.execute(&format!("UPDATE circle_banks SET anonymized = 1 WHERE author_id = ?1 AND anonymized = 0 AND purged_at IS NULL AND NOT {no_keep}"), params![user])?;
    conn.execute("UPDATE circle_versions SET message = NULL WHERE committer_id = ?1 AND message IS NOT NULL", [user])?;
    conn.execute("UPDATE circle_reports SET note = NULL, reporter_id = NULL WHERE reporter_id = ?1", [user])?;
    // (and the masked addresses of its email invites)
    conn.execute(
        "UPDATE circle_invites SET revoked_at = COALESCE(revoked_at, ?2), note = NULL, email_label = NULL, email_key = NULL WHERE created_by = ?1 AND (revoked_at IS NULL OR note IS NOT NULL OR email_label IS NOT NULL OR email_key IS NOT NULL)",
        params![user, now],
    )?;
    // (its hourly invitation count, the one row that names it, and its share of each domain's hour; no LIKE: the prefix and the id after the hmac are compared exactly)
    conn.execute("DELETE FROM pool_alert_budget WHERE channel = 'circle_email_inviter:' || ?1", [user])?;
    conn.execute("DELETE FROM pool_alert_budget WHERE substr(channel, 1, 20) = 'circle_email_domain:' AND substr(channel, 21 + instr(substr(channel, 21), ':')) = ?1", [user])?;
    conn.execute("DELETE FROM circle_transfers WHERE from_user = ?1", [user])?;
    conn.execute("DELETE FROM circle_transfers WHERE to_user = ?1", [user])?;
    conn.execute("DELETE FROM circle_members WHERE user_id = ?1", [user])?;
    Ok(())
}

/// The daily erasure_resweep's circle part: every circle table a request already in flight could have written under an erased
/// account's id is cleaned, and the open circles `ownerless` get their succession. Idempotent.
pub fn circle_resweep(conn: &Connection, now: i64, ownerless: &[String]) -> rusqlite::Result<()> {
    let done = "SELECT user_id FROM erasure_done";
    let stray = format!("SELECT DISTINCT circle_id FROM circle_members WHERE user_id IN ({done})");
    conn.execute(
        &format!(
            "INSERT INTO pool_audit (id, at, actor_id, actor_kind, action, item_id, target_user_id)
         SELECT lower(hex(randomblob(16))), ?1, NULL, 'system', 'circle.member_erased', circle_id, user_id FROM circle_members WHERE user_id IN ({done}) AND left_at IS NULL"
        ),
        [now],
    )?;
    conn.execute(&format!("UPDATE circle_members SET left_at = ?1, left_reason = 'account_deleted' WHERE user_id IN ({done}) AND left_at IS NULL"), [now])?;
    let purge_at = now + CIRCLE_UNDO_MS;
    let list = serde_json::to_string(ownerless).unwrap();
    succession(conn, &format!("{stray} UNION SELECT value FROM json_each(?3)"), &[&now, &purge_at, &list], now, purge_at)?;
    conn.execute(&format!("UPDATE circle_banks SET anonymized = 1 WHERE author_id IN ({done}) AND anonymized = 0 AND purged_at IS NULL"), [])?;
    conn.execute(&format!("UPDATE circle_versions SET message = NULL WHERE committer_id IN ({done}) AND message IS NOT NULL"), [])?;
    conn.execute(&format!("UPDATE circle_reports SET note = NULL, reporter_id = NULL WHERE reporter_id IN ({done})"), [])?;
    conn.execute(
        &format!("UPDATE circle_invites SET revoked_at = COALESCE(revoked_at, ?1), note = NULL, email_label = NULL, email_key = NULL WHERE created_by IN ({done}) AND (revoked_at IS NULL OR note IS NOT NULL OR email_label IS NOT NULL OR email_key IS NOT NULL)"),
        [now],
    )?;
    // (the hour's mail counters keyed by an erased inviter)
    conn.execute(&format!("DELETE FROM pool_alert_budget WHERE channel LIKE 'circle_email_inviter:%' AND substr(channel, 22) IN ({done})"), [])?;
    conn.execute(&format!("DELETE FROM pool_alert_budget WHERE channel LIKE 'circle_email_domain:%:%' AND substr(channel, 21 + instr(substr(channel, 21), ':')) IN ({done})"), [])?;
    conn.execute(&format!("DELETE FROM circle_transfers WHERE from_user IN ({done})"), [])?;
    conn.execute(&format!("DELETE FROM circle_transfers WHERE to_user IN ({done})"), [])?;
    conn.execute(&format!("DELETE FROM circle_members WHERE user_id IN ({done})"), [])?;
    Ok(())
}

#[cfg(test)]
#[path = "circleerase_tests.rs"]
mod tests;
