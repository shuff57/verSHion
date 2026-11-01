//! Account deletion's pool part, standing, preferences and the pending-account path gates (worker/src/pool/index.ts). `delete_content` is the auth package's
//! `on_delete` hook (it runs inside the deletion transaction, once the account is certainly gone); the daily `erasure_resweep` repeats it for what a request
//! already in flight wrote afterwards.
use super::circleerase::circle_erasure;
use super::ctx::{Fail, PoolError, Res};
use super::sqlfrag::{DAY_MS, alive_sql};
use crate::auth::crypto::uuid_v4;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value, json};

/// The methods the pool answers (the CORS allow list of its routes).
pub const POOL_METHODS: &str = "GET, HEAD, POST, PUT, PATCH, DELETE";

/// Account deletion: the author's items are removed at once and purged after 14 days (an item under a DMCA action follows the same date), filed reports are
/// kept anonymised, pending uploads, counters, limits and the importer's per-question records go, and audit notes about the account are nulled.
pub fn delete_content(conn: &Connection, user: &str, now: i64) -> rusqlite::Result<()> {
    let anon = format!("deleted:{}", uuid_v4().replace('-', ""));
    let purge_at = now + 14 * DAY_MS;
    // Keep (anonymised) when the author chose so; with no choice made, keep exactly the items other teachers imported, delete the rest. An explicit "no"
    // deletes everything.
    let keep = "(EXISTS (SELECT 1 FROM pool_prefs WHERE user_id = ?1 AND keep_on_delete = 1)
    OR (NOT EXISTS (SELECT 1 FROM pool_prefs WHERE user_id = ?1)
        AND EXISTS (SELECT 1 FROM imports im WHERE im.item_id = items.id AND im.user_id <> ?1 AND im.kind = 'import')))";
    // The author chose to keep their shared items: they stay, shown as by "deleted teacher", with licences and attribution intact.
    conn.execute(&format!("UPDATE items SET anonymized = 1, updated_at = ?2 WHERE author_id = ?1 AND removed_at IS NULL AND {keep}"), params![user, now])?;
    conn.execute(
        &format!("UPDATE items SET removed_at = ?2, removed_reason = 'account_deleted', purge_after = ?3, updated_at = ?2 WHERE author_id = ?1 AND removed_at IS NULL AND NOT {keep}"),
        params![user, now, purge_at],
    )?;
    conn.execute("UPDATE items SET purge_after = MIN(COALESCE(purge_after, ?2), ?2) WHERE author_id = ?1 AND removed_at IS NOT NULL AND purged_at IS NULL", params![user, purge_at])?;
    // Free text the account wrote goes with it: report notes, and the statements and notes in admin flags (independence statements, appeal notes).
    conn.execute("UPDATE reports SET note = NULL WHERE reporter_id = ?1 AND note IS NOT NULL", [user])?;
    conn.execute("UPDATE admin_flags SET detail = json_remove(detail, '$.statement', '$.note') WHERE user_id = ?1 AND detail IS NOT NULL AND json_valid(detail)", [user])?;
    // The served text a key_leak flag of this reporter quoted goes (by report id).
    conn.execute(
        "UPDATE admin_flags SET detail = json_remove(detail, '$.served') WHERE kind = 'key_leak' AND json_valid(detail) AND detail ->> '$.report_id' IN (SELECT id FROM reports WHERE reporter_id = ?1)",
        [user],
    )?;
    conn.execute("UPDATE reports SET reporter_id = NULL WHERE reporter_id = ?1", [user])?;
    conn.execute("DELETE FROM pending_blobs WHERE uploader_id = ?1", [user])?;
    // The account's served rows go: it can publish nothing a record could bind.
    conn.execute("DELETE FROM served WHERE reader_id = ?1", [user])?;
    conn.execute("DELETE FROM usage_daily WHERE user_id = ?1", [user])?;
    // The AI credits go with the account (purchased credits never expire except here): the ledger, the sessions that priced them, and the usage rows.
    // Stripe keeps the payment record itself.
    conn.execute("DELETE FROM credit_ledger WHERE principal = ?1", [user])?;
    conn.execute("DELETE FROM ai_sessions WHERE principal = ?1", [user])?;
    conn.execute("DELETE FROM ai_usage WHERE principal = ?1", [user])?;
    conn.execute("DELETE FROM pool_limits WHERE user_id = ?1", [user])?;
    // (the account's publish lock row goes too; the cron's erasure_resweep repeats all of this for anything a request already running wrote after this)
    conn.execute("DELETE FROM pool_locks WHERE user_id = ?1", [user])?;
    // Its invite redemption goes; the invite keeps its use count.
    conn.execute("DELETE FROM pool_invite_redemptions WHERE user_id = ?1", [user])?;
    // Circles: step down, hand on or close, banks kept anonymised or removed (its keep_on_delete choice, read before pool_prefs goes below), messages and
    // notes, codes, memberships.
    circle_erasure(conn, user, now)?;
    // What the account typed, and system notes about it; an admin's note is evidence and stays, its personal data scrubbed when written.
    conn.execute("UPDATE pool_audit SET note = NULL WHERE target_user_id = ?1 AND note IS NOT NULL AND actor_kind <> 'admin'", [user])?;
    // The counter-notices the account sent keep their ids and dates, not the person's name, address, phone or signature.
    conn.execute(
        "UPDATE dmca_counter_notices SET name = '[deleted]', address = '[deleted]', phone = '[deleted]', signature = '[deleted]', material_desc = '[deleted]'
          WHERE action_id IN (SELECT id FROM dmca_actions WHERE author_id = ?1)",
        [user],
    )?;
    // Others' stored chains naming the account: rewritten by the cron in bounded batches (reads already show "deleted teacher" from the account's state).
    conn.execute("INSERT OR IGNORE INTO deletion_rewrites (user_id, tbl, at) VALUES (?1, 'item_questions', ?2), (?1, 'ghost_questions', ?2)", params![user, now])?;
    conn.execute("DELETE FROM pool_prefs WHERE user_id = ?1", [user])?;
    // The account's serves stay counted (import counts) but no longer name it; its report audits no longer link back to it. One anonymous key per deleted
    // account, so its serves are still counted once per item.
    conn.execute("UPDATE imports SET user_id = ?2 WHERE user_id = ?1", params![user, anon])?;
    conn.execute("UPDATE pool_audit SET actor_id = NULL, detail = json_remove(detail, '$.report_id') WHERE actor_id = ?1 AND action = 'item.report'", [user])?;
    // The account's durable authorship records go with it (they only ever served its own publishes, and nothing can be published by it again).
    conn.execute("DELETE FROM author_contents WHERE author_id = ?1", [user])?;
    conn.execute("DELETE FROM author_firsts WHERE author_id = ?1", [user])?;
    conn.execute("DELETE FROM author_stems WHERE author_id = ?1", [user])?;
    conn.execute("DELETE FROM stem_bands WHERE user_id IN (?1, 'own:' || ?1)", [user])?;
    conn.execute("DELETE FROM carried_keys WHERE author_id = ?1", [user])?;
    conn.execute("DELETE FROM blob_uploads WHERE uploader_id = ?1", [user])?;
    // The account's open appeals close (nobody is left to answer), so its removed items are purged at the normal time, not kept to the appeal's end.
    conn.execute(
        "UPDATE admin_flags SET resolved_at = ?2, detail = json_set(COALESCE(CASE WHEN json_valid(detail) THEN detail END, '{}'), '$.outcome', 'author_erased')
          WHERE kind = 'appeal' AND resolved_at IS NULL AND (user_id = ?1 OR item_id IN (SELECT id FROM items WHERE author_id = ?1))",
        params![user, now],
    )?;
    // ... and every note the account itself wrote.
    conn.execute("UPDATE pool_audit SET note = NULL WHERE note IS NOT NULL AND actor_id = ?1", [user])?;
    conn.execute("INSERT INTO pool_audit (id, at, actor_id, actor_kind, action, target_user_id) VALUES (?1, ?2, ?3, 'user', 'account_deleted.content_removed', ?3)", params![uuid_v4(), now, user])?;
    // Done, in the same transaction; a deleted account without this row is finished by the cron.
    conn.execute("INSERT OR IGNORE INTO erasure_done (user_id, at) VALUES (?1, ?2)", params![user, now])?;
    Ok(())
}

/// Suspension and restore change nothing in the pool: every non-author read joins the author's standing, so the items hide and show at once. Only the audit
/// row. `action` is `author_suspended` or `author_restored`.
pub fn standing_audit(conn: &Connection, user: &str, now: i64, action: &str) -> rusqlite::Result<()> {
    conn.execute("INSERT INTO pool_audit (id, at, actor_kind, action, target_user_id) VALUES (?1, ?2, 'system', ?3, ?4)", params![uuid_v4(), now, action, user])?;
    Ok(())
}

/// GET /items/prefs: what happens to my shared items if I delete my account. null: no choice made yet (the app asks at account deletion); the default then
/// keeps, anonymised, the items other teachers imported and deletes the rest.
pub fn get_prefs(conn: &Connection, user: &str) -> Res<Value> {
    let r: Option<i64> = conn.query_row("SELECT keep_on_delete FROM pool_prefs WHERE user_id = ?1", [user], |r| r.get(0)).optional()?;
    Ok(json!({ "keep_on_delete": r.map(|v| v == 1), "default": "keep_imported_delete_rest" }))
}

/// PUT /items/prefs {keep_on_delete}: written only while the account stands (same statement); else the caller gets the 401 an unauthenticated request gets.
pub fn put_prefs(conn: &Connection, user: &str, now: i64, body: &Map<String, Value>) -> Res<Value> {
    let Some(keep) = body.get("keep_on_delete").and_then(Value::as_bool) else {
        return Err(PoolError::new(400, "invalid_request").with("field", json!("keep_on_delete")).into());
    };
    let w = conn.execute(
        &format!("INSERT INTO pool_prefs (user_id, keep_on_delete, updated_at) SELECT ?1, ?2, ?3 WHERE {} ON CONFLICT (user_id) DO UPDATE SET keep_on_delete = ?2, updated_at = ?3", alive_sql("?1")),
        params![user, keep as i64, now],
    )?;
    if w == 0 {
        return Err(Fail::Pool(PoolError::new(401, "unauthorized")));
    }
    Ok(json!({ "keep_on_delete": keep }))
}

/// A circle path a pending account may call: the list (GET /circles), join and its preview, heads, and every /circles/:cid route. (A confirmed `pending`
/// account genuinely awaiting its first decision may use circles; the handlers' membership and role checks remain the real gate.)
pub fn pending_circle_path(method: &str, path: &str) -> bool {
    if path == "/circles" {
        return method == "GET";
    }
    if matches!(path, "/circles/join/preview" | "/circles/join" | "/circles/heads") {
        return true;
    }
    // ^/circles/[0-9a-f]{32}(/|$)
    path.strip_prefix("/circles/").is_some_and(|rest| rest.len() >= 32 && rest.as_bytes()[..32].iter().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) && (rest.len() == 32 || rest.as_bytes()[32] == b'/'))
}
/// The image upload a circle send makes (`PUT /blobs/<sha256>`): content-addressed, and its quota/caps still apply.
pub fn pending_blob_path(method: &str, path: &str) -> bool {
    method == "PUT" && path.strip_prefix("/blobs/").is_some_and(|h| h.len() == 64 && h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
}
/// Trust levels that may use the public pool.
pub fn pool_trust(trust: &str) -> bool {
    matches!(trust, "teacher" | "admin")
}

/// True when the account's latest approval request is genuinely undecided. `trust = "pending"` alone is not enough: the auth package puts a revoked teacher
/// back on `pending`, and a rejected applicant stays there. A re-application after the cooldown is a new row with no decision, so it counts as waiting again.
pub fn awaiting_first_decision(conn: &Connection, user: &str) -> rusqlite::Result<bool> {
    let decided: bool = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM approval_requests ar
          WHERE ar.rowid = (SELECT ar2.rowid FROM approval_requests ar2 WHERE ar2.user_id = ?1 ORDER BY ar2.requested_at DESC, ar2.rowid DESC LIMIT 1)
            AND ar.decided_at IS NOT NULL AND ar.decision IN ('rejected', 'revoked'))",
        [user],
        |r| r.get(0),
    )?;
    Ok(!decided)
}

#[cfg(test)]
#[path = "poolcontent_tests.rs"]
mod tests;
