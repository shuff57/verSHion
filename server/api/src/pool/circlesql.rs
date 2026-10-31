//! SQL fragments over the circle tables (worker/src/pool/circles.ts "SQL fragments"), shared by every circle module. Each takes SQL
//! expressions (a column or a `?n` reference) and returns a condition or subquery to embed.
use super::sqlfrag::DAY_MS;

pub const CIRCLE_CODE_MAX_DAYS: i64 = 30;

/// `me` is an ACTIVE member of circle `cid` with one of `roles` (all roles when empty).
pub fn active_sql(cid: &str, me: &str, roles: &[&str]) -> String {
    let r = if roles.is_empty() { String::new() } else { format!(" AND am.role IN ({})", roles.iter().map(|r| format!("'{r}'")).collect::<Vec<_>>().join(", ")) };
    format!("EXISTS (SELECT 1 FROM circle_members am WHERE am.circle_id = {cid} AND am.user_id = {me} AND am.left_at IS NULL{r})")
}
/// Circle `cid` is open (not closed, not purged).
pub fn open_sql(cid: &str) -> String {
    format!("EXISTS (SELECT 1 FROM circles oc WHERE oc.id = {cid} AND oc.closed_at IS NULL AND oc.purged_at IS NULL)")
}
/// Circles `me` owns now (an owner-closed circle counts until it is purged or reopened).
pub fn owned_sql(me: &str) -> String {
    format!("(SELECT COUNT(*) FROM circle_members om JOIN circles oc2 ON oc2.id = om.circle_id WHERE om.user_id = {me} AND om.left_at IS NULL AND om.role = 'owner' AND oc2.purged_at IS NULL)")
}
/// Circles `me` is an active member of (closed ones included until purged or left).
pub fn memberships_sql(me: &str) -> String {
    format!("(SELECT COUNT(*) FROM circle_members mm JOIN circles mc ON mc.id = mm.circle_id WHERE mm.user_id = {me} AND mm.left_at IS NULL AND mc.purged_at IS NULL)")
}
/// The account `uid` is in good standing for a CIRCLE: confirmed (`pending` counts), not suspended, not erased.
pub fn in_standing_sql(uid: &str) -> String {
    format!("EXISTS (SELECT 1 FROM users su WHERE su.id = {uid} AND su.trust IN ('teacher', 'admin', 'pending') AND su.suspended_at IS NULL AND su.deleted_at IS NULL)")
}
/// The hold sources a SEND's own content check raises; none of them freezes the bank.
pub const SEND_HOLD_SOURCES: &str = "('privacy', 'privacy_near', 'blocklist')";
pub const SEND_FLAG_KINDS: &str = "('privacy_text_match', 'blocklist')";
/// A hold that freezes a bank: a report's or an administrator's, kept as evidence.
pub fn blocking_hold_sql(bid: &str) -> String {
    format!("EXISTS (SELECT 1 FROM circle_holds xh WHERE xh.bank_id = {bid} AND xh.source NOT IN {SEND_HOLD_SOURCES})")
}
/// Bank row `b`'s content follows its author's standing (or the erased author's anonymised bank is kept).
pub fn author_ok_sql(b: &str) -> String {
    format!("EXISTS (SELECT 1 FROM users au WHERE au.id = {b}.author_id AND au.trust IN ('teacher', 'admin', 'pending') AND au.suspended_at IS NULL AND (au.deleted_at IS NULL OR {b}.anonymized = 1))")
}
/// Membership rows of `me`, former ones included.
pub fn membership_rows_sql(me: &str) -> String {
    format!("(SELECT COUNT(*) FROM circle_members rm WHERE rm.user_id = {me})")
}
/// A code invite `i` lets `me` (re)join only if issued after `me` was removed, while its issuer is still the active owner in good standing.
pub fn code_usable_sql(i: &str, me: &str) -> String {
    format!(
        "(NOT EXISTS (SELECT 1 FROM circle_members rr WHERE rr.circle_id = {i}.circle_id AND rr.user_id = {me} AND rr.left_reason = 'removed' AND rr.left_at >= {i}.created_at)
    AND EXISTS (SELECT 1 FROM circle_members io WHERE io.circle_id = {i}.circle_id AND io.user_id = {i}.created_by AND io.role = 'owner' AND io.left_at IS NULL)
    AND {})",
        in_standing_sql(&format!("{i}.created_by"))
    )
}
/// The role `me` (re)joins with by code `i`.
pub fn join_role_sql(i: &str, me: &str) -> String {
    format!(
        "CASE WHEN EXISTS (SELECT 1 FROM circle_members jr WHERE jr.circle_id = {i}.circle_id AND jr.user_id = {me} AND jr.left_at IS NOT NULL AND jr.left_at >= {i}.created_at AND jr.role = 'viewer') THEN 'viewer' ELSE {i}.role END"
    )
}
/// Active members of circle `cid`.
pub fn members_sql(cid: &str) -> String {
    format!("(SELECT COUNT(*) FROM circle_members xm WHERE xm.circle_id = {cid} AND xm.left_at IS NULL)")
}
/// Usable codes (`email` = false) or email invites (`email` = true) of circle `cid` at `now`.
pub fn open_invites_sql(cid: &str, now: &str, email: bool) -> String {
    format!(
        "(SELECT COUNT(*) FROM circle_invites oi WHERE oi.circle_id = {cid} AND oi.created_at > {now} - {} AND oi.revoked_at IS NULL AND oi.expires_at > {now} AND oi.uses < oi.max_uses AND oi.kind = '{}')",
        CIRCLE_CODE_MAX_DAYS * DAY_MS,
        if email { "email" } else { "code" }
    )
}
