//! The pool's shared constants and SQL fragments (the pure parts of worker/src/pool/shared.ts): ids and their patterns,
//! the visibility rule, the limits table, the admin flag severities, the usage and quota SQL, and the block-key
//! comparisons every hold, release, plan and check uses. Each fragment is the ONE place its rule is written, exactly as
//! in the Worker; the tests prepare them against the real migrated schema, so a typo is a failing test, not a 500.
use crate::auth::crypto::random_bytes;
use std::fmt::Write;

pub const MB: i64 = 1024 * 1024;
pub const DAY_MS: i64 = 86_400_000;
pub const HOUR_MS: i64 = 3_600_000;

// ---- ids and hashes ----

/// 128 random bits, hex: item ids, qids, blob key parts, report ids.
pub fn new_id() -> String {
    hex(&random_bytes(16))
}

pub fn hex(b: &[u8]) -> String {
    b.iter().fold(String::with_capacity(b.len() * 2), |mut s, x| {
        let _ = write!(s, "{x:02x}");
        s
    })
}

/// `^[0-9a-f]{32}$`
pub fn is_id(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}
/// Client-generated publish ids: at least 128 bits of base64url (or hex): `^[A-Za-z0-9_-]{22,64}$`.
pub fn is_publish_id(s: &str) -> bool {
    (22..=64).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
/// `^[0-9a-f]{64}$`
pub fn is_sha(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

pub fn day_of(ms: i64) -> i64 {
    ms.div_euclid(DAY_MS)
}

// ---- visibility ----

/// The author's standing ("Content follows the author's standing"). `u` = the author's users row.
pub const STANDING: &str = "u.trust IN ('teacher', 'admin') AND u.suspended_at IS NULL AND u.deleted_at IS NULL";

/// Visible to someone other than its author: public, committed, not removed, not held, author in good standing.
pub fn visible_to_others() -> String {
    format!(
        "i.visibility = 'public' AND i.current_publish_id IS NOT NULL AND i.removed_at IS NULL
  AND NOT EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = i.id)
  AND (({STANDING}) OR (i.anonymized = 1 AND u.deleted_at IS NOT NULL AND u.trust IN ('teacher', 'admin') AND u.suspended_at IS NULL))"
    )
}

/// Visible to the caller bound at `me` (a parameter reference such as `?2`).
pub fn visible_to(me: &str) -> String {
    format!("((i.author_id = {me} AND i.removed_at IS NULL AND i.current_publish_id IS NOT NULL) OR (i.author_id <> {me} AND {}))", visible_to_others())
}

/// The guard that lives IN a write: a request authenticated before its account was erased keeps running; every row it
/// then writes under the caller's id is conditioned, in the SAME statement, on the account not being erased. `p` is the
/// SQL for the caller's user id.
pub fn alive_sql(p: &str) -> String {
    format!("NOT EXISTS (SELECT 1 FROM users du WHERE du.id = {p} AND du.deleted_at IS NOT NULL)")
}

// ---- limits ----

/// Defaults ("Limits"); each can be raised per user or site-wide by an admin. In the Worker's order.
pub const DEFAULT_LIMITS: &[(&str, i64)] = &[
    ("publishes", 50),
    ("imports", 100),
    ("item_reads", 2000),
    ("browse", 500),
    ("blob_uploads", 500),
    ("blob_bytes", 200 * MB),
    ("blob_miss", 100),
    ("item_miss", 100),
    ("reports", 20),
    ("storage_bytes", 200 * MB),
    ("shared_questions", 5000),
    ("report_threshold", 3),
    ("removed_bytes", 200 * MB),
    ("publish_refusals", 300),
    ("block_common_k", 3),
    ("appeals", 10),
    ("exports", 3),
    ("instant_holds_global", 50),
    ("previews", 200),
    ("preview_units", 2_000_000),
    ("reads_max", 200_000),
    ("imports_max", 20_000),
    ("author_records", 500_000),
    ("invite_tries", 10),
    ("circle_creates", 5),
    ("circle_invites", 50),
    ("circle_join_tries", 10),
    ("circle_commits", 100),
    ("circle_rollbacks", 20),
    ("circles_owned", 5),
    ("circle_memberships", 20),
    ("circle_memberships_total", 100),
    ("circle_transfer_offers", 10),
    ("circle_members", 50),
    ("circle_banks", 100),
    ("circle_bytes", 200 * MB),
    ("circle_questions", 5000),
    ("circle_open_invites", 10),
    ("circle_records", 50_000),
    ("circle_email_invites", 40),
    ("circle_email_address", 3),
    ("circle_email_hourly", 100),
    ("circle_email_invites_hourly", 30),
    ("circle_email_domain_hourly", 30),
    ("circle_email_inviter_domain_hourly", 10),
    ("circle_open_email_invites", 40),
    ("circle_send_flags", 5),
    ("circle_hide_changes", 20),
    ("circle_hide_attempts", 60),
    ("circle_hidden_reads", 100),
    ("circle_change_reads", 200),
    ("circle_hide_checks", 60),
];

/// The default for a metric, None for a name that is not one.
pub fn default_limit(metric: &str) -> Option<i64> {
    DEFAULT_LIMITS.iter().find(|(m, _)| *m == metric).map(|(_, v)| *v)
}

// ---- admin flags ----

/// Positions / items listed in one flag's detail (the rest is a count).
pub const FLAG_DETAIL_MAX: usize = 5;
/// Flags per account, KIND and day kept as their own rows; past it, the kind folds into one aggregated row with a count
/// (never dropped). Integrity kinds are never folded, so no burst of one kind can silence another; origin_unverified
/// always folds (one row a day with a count).
pub const FLAGS_PER_ACCOUNT_DAY: i64 = 20;
pub const INTEGRITY_FLAGS: [&str; 10] = [
    "independent_claim",
    "independent_repeat",
    "license_soft",
    "license_conflict",
    "license_grandfathered",
    "blocklist",
    "privacy_text_match",
    "evidence_overflow",
    "origin_suspect",
    "origin_modified",
];
pub const COLLAPSED_FLAGS: [&str; 1] = ["origin_unverified"];
/// Admin flag severity (0 = most urgent): the queue is ordered by it, then newest first.
pub const FLAG_SEVERITY: &[(&str, i64)] = &[
    ("privacy_text_match", 0),
    ("instant_hold_cap", 1),
    ("privacy_backlog", 1),
    ("cron_failure", 1),
    ("repeat_infringer", 1),
    ("blocklist", 1),
    ("dmca_copy", 1),
    ("appeal_escalated", 0),
    ("appeal_overdue", 1),
    ("appeal_auto_restored", 2),
    ("hold_cleared", 2),
    ("license_mismatch", 1),
    ("counter_notice", 1),
    ("needs_reupload", 2),
    ("appeal", 2),
    ("independent_repeat", 2),
    ("license_conflict", 2),
    ("license_soft", 2),
    ("cap", 3),
    ("independent_claim", 3),
    ("evidence_overflow", 3),
    ("license_grandfathered", 3),
    ("origin_unverified", 4),
    ("origin_modified", 4),
    ("origin_suspect", 3),
];
pub fn severity_of(kind: &str) -> i64 {
    FLAG_SEVERITY.iter().find(|(k, _)| *k == kind).map(|(_, v)| *v).unwrap_or(3)
}
pub fn flag_severity_sql() -> String {
    let whens: Vec<String> = FLAG_SEVERITY.iter().map(|(k, v)| format!("WHEN '{k}' THEN {v}")).collect();
    format!("CASE kind {} ELSE 3 END", whens.join(" "))
}

// ---- quota ----

/// What an account is charged (in bytes): the images of its publishes that are not removed, plus its live pending
/// uploads, the body bytes of those publishes, and 1 KB per item not removed (removal releases the charge). Computed,
/// not counted, so it cannot drift; one statement, so it is consistent. Binds `?1` = user, `?2` = now.
/// `include`: an item id (parameter reference) counted as live although removed (the undelete check).
pub fn usage_sql(include: Option<&str>) -> String {
    let live = |alias: &str| {
        let extra = include.map(|i| format!(" AND it.id <> {i}")).unwrap_or_default();
        format!("NOT EXISTS (SELECT 1 FROM items it WHERE it.id = {alias}.item_id AND it.removed_at IS NOT NULL{extra})")
    };
    let items_extra = include.map(|i| format!(" OR id = {i}")).unwrap_or_default();
    format!(
        "(
  COALESCE((SELECT SUM(size) FROM (
      SELECT ib.sha256, b.size FROM item_blobs ib
        JOIN item_publishes p ON p.publish_id = ib.publish_id
        JOIN blobs b ON b.sha256 = ib.sha256
       WHERE p.author_id = ?1 AND {live_p}
      UNION
      SELECT sha256, size FROM pending_blobs WHERE uploader_id = ?1 AND until >= ?2)), 0)
  + COALESCE((SELECT SUM(p.full_bytes + p.stripped_bytes) FROM item_publishes p WHERE p.author_id = ?1 AND {live_p}), 0)
  + 1024 * (SELECT COUNT(*) FROM items WHERE author_id = ?1 AND (removed_at IS NULL{items_extra}))
)",
        live_p = live("p")
    )
}

/// Body and image bytes still held by `?1`'s removed-but-unpurged items (storage amplification bound).
pub const REMOVED_SQL: &str = "(
  COALESCE((SELECT SUM(p.full_bytes + p.stripped_bytes) FROM item_publishes p JOIN items it ON it.id = p.item_id
             WHERE p.author_id = ?1 AND it.removed_at IS NOT NULL AND it.purged_at IS NULL), 0)
  -- and the images only those items keep (each once).
  + COALESCE((SELECT SUM(size) FROM (SELECT DISTINCT ib.sha256, b.size FROM item_blobs ib JOIN item_publishes p ON p.publish_id = ib.publish_id JOIN items it ON it.id = p.item_id JOIN blobs b ON b.sha256 = ib.sha256
             WHERE p.author_id = ?1 AND it.removed_at IS NOT NULL AND it.purged_at IS NULL)), 0))";

/// Questions shared by `me` across items other than `except`: committed live items, plus every other item's publish still
/// in flight (staging/staged): a new item's whole count, a live item's republish only what it adds (MAX(0, new - current)).
pub fn shared_sql(me: &str, except: &str) -> String {
    format!(
        "(
  (SELECT COALESCE(SUM(question_count), 0) FROM items WHERE author_id = {me} AND removed_at IS NULL AND id <> {except})
  + (SELECT COALESCE(SUM(question_count), 0) FROM item_publishes WHERE author_id = {me} AND status IN ('staging', 'staged') AND item_id <> {except}
       AND NOT EXISTS (SELECT 1 FROM items x WHERE x.id = item_publishes.item_id))
  + (SELECT COALESCE(SUM(MAX(0, p.question_count - x.question_count)), 0) FROM item_publishes p JOIN items x ON x.id = p.item_id
      WHERE p.author_id = {me} AND p.status IN ('staging', 'staged') AND p.item_id <> {except} AND x.removed_at IS NULL))"
    )
}

/// The chain JSON `expr` with the name of every link to a DELETED account's item replaced by "deleted teacher", applied
/// whenever a chain is stored, so a stored row never brings a deleted name back.
pub fn anon_chain(expr: &str) -> String {
    format!(
        "(SELECT COALESCE(json_group_array(CASE WHEN EXISTS (SELECT 1 FROM items ax JOIN users au ON au.id = ax.author_id WHERE ax.id = al.value ->> '$.item_id' AND au.deleted_at IS NOT NULL)
                                          THEN json_set(al.value, '$.author_name', 'deleted teacher') ELSE json(al.value) END), '[]')
      FROM json_each(COALESCE({expr}, '[]')) al)"
    )
}

// ---- block keys ----

/// Every key a privacy or copyright block can name for a question: the exact match_hash, its normalised skeleton (`n:`),
/// the normalised full content (`f:`), the normalised explanation (`e:`) and the normalised model / accepted answers
/// (`a:`). A block row of kind 'match' holds any of them; these helpers are the ONE comparison every hold, release, plan
/// and check uses.
pub const QKEY_COLS: [&str; 6] = ["match_hash", "bk_norm", "bk_stem", "bk_full", "bk_expl", "bk_ans"];

/// SQL: the block keys of every question of publish `pub` (a subquery usable after IN). A Multipart question also has the
/// block keys of each PART (item_part_keys), so student data in a part is held as a standalone question and inside any
/// other Multipart, and the other way round.
pub fn q_keys_of(pub_: &str) -> String {
    let cols: Vec<String> = QKEY_COLS.iter().map(|k| format!("qkq.{k}")).collect();
    format!("(SELECT qk.value FROM item_questions qkq, json_each(json_array({})) qk WHERE qkq.publish_id = {pub_} AND qk.value IS NOT NULL)", cols.join(", "))
}

/// SQL: `x` is a block key of publish `pub` (its questions' keys or their parts').
pub fn in_q_keys_of(x: &str, pub_: &str) -> String {
    format!("({x} IN {} OR {x} IN (SELECT pk.key FROM item_part_keys pk WHERE pk.publish_id = {pub_}))", q_keys_of(pub_))
}

/// SQL: question row `iq` has a key in `list` (a parenthesised subquery or list).
pub fn q_key_in(iq: &str, list: &str) -> String {
    let ors: Vec<String> = QKEY_COLS.iter().map(|k| format!("{iq}.{k} IN {list}")).collect();
    format!("({} OR EXISTS (SELECT 1 FROM item_part_keys pk WHERE pk.publish_id = {iq}.publish_id AND pk.pos = {iq}.pos AND pk.key IN {list}))", ors.join(" OR "))
}

/// SQL: question row `iq` has the key `x`.
pub fn q_key_eq(iq: &str, x: &str) -> String {
    let cols: Vec<String> = QKEY_COLS.iter().map(|k| format!("{iq}.{k}")).collect();
    format!("({x} IN ({}) OR EXISTS (SELECT 1 FROM item_part_keys pk WHERE pk.publish_id = {iq}.publish_id AND pk.pos = {iq}.pos AND pk.key = {x}))", cols.join(", "))
}

#[cfg(test)]
#[path = "sqlfrag_tests.rs"]
mod tests;
