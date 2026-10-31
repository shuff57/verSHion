//! The append-only audit trail (`auth_audit`), written in the same transaction as the change it records
//! (port of `audit` in auth/src/admin.ts).
use rusqlite::{Connection, ToSql, params};
use serde_json::Value;

#[derive(Default)]
pub struct AuditFields<'a> {
    pub action: &'a str,
    pub target_id: Option<&'a str>,
    pub prev_trust: Option<&'a str>,
    pub new_trust: Option<&'a str>,
    pub evidence: Option<&'a str>,
    pub evidence_note: Option<&'a str>,
    pub note: Option<&'a str>,
    pub reason: Option<&'a str>,
    pub detail: Option<Value>,
}

/// Inserts one audit row. With `when`, an SQL condition that must hold (for "only if this transaction's change won");
/// it is spliced from code, never from input. With `trust_from_row`, the trust columns are the target's current trust.
#[allow(clippy::too_many_arguments)]
pub fn audit(conn: &Connection, actor_id: Option<&str>, kind: &str, e: &AuditFields, now: i64, when: Option<&str>, trust_from_row: bool, id: &str) -> rusqlite::Result<usize> {
    let cols = "id, at, actor_id, actor_kind, action, target_id, prev_trust, new_trust, evidence, evidence_note, note, reason, detail";
    let detail = e.detail.as_ref().map(Value::to_string);
    if trust_from_row {
        let cond = when.map(|w| format!(" AND {w}")).unwrap_or_default();
        conn.execute(
            &format!("INSERT INTO auth_audit ({cols}) SELECT ?1, ?2, ?3, ?4, ?5, ?6, u.trust, u.trust, ?7, ?8, ?9, ?10, ?11 FROM users u WHERE u.id = ?6{cond}"),
            params![id, now, actor_id, kind, e.action, e.target_id, e.evidence, e.evidence_note, e.note, e.reason, detail],
        )
    } else {
        let w = when.map(|w| format!(" WHERE {w}")).unwrap_or_default();
        let p: [&dyn ToSql; 13] = [&id, &now, &actor_id, &kind, &e.action, &e.target_id, &e.prev_trust, &e.new_trust, &e.evidence, &e.evidence_note, &e.note, &e.reason, &detail];
        conn.execute(&format!("INSERT INTO auth_audit ({cols}) SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13{w}"), &p[..])
    }
}
