//! The pool's hooks into the auth package (worker/src/cleanup.ts): what an account's deletion, suspension and restore do to its pool content. Each runs
//! inside the auth transaction that changes the account.
use super::poolcontent::{delete_content, standing_audit};
use crate::auth::router::AuthState;
use rusqlite::{OptionalExtension, Transaction};

/// Account deletion: the author's items are removed at once and purged after 14 days.
pub fn on_delete(tx: &Transaction, user: &str, now: i64) -> rusqlite::Result<()> {
    delete_content(tx, user, now)
}

/// Suspension or restore (the hook runs after the account's flag changed): only the audit row, since every non-author read joins the author's standing.
pub fn standing(tx: &Transaction, user: &str, now: i64) -> rusqlite::Result<()> {
    let suspended: Option<bool> = tx.query_row("SELECT suspended_at IS NOT NULL FROM users WHERE id = ?1", [user], |r| r.get(0)).optional()?;
    standing_audit(tx, user, now, if suspended == Some(true) { "author_suspended" } else { "author_restored" })
}

/// Connects the hooks to the auth state. (The data export needs no extra section: the pool's own GET /me/export carries the pool's data.)
pub fn install(auth: &mut AuthState) {
    auth.on_delete = Box::new(on_delete);
    auth.standing = Box::new(standing);
}

#[cfg(test)]
#[path = "hooks_tests.rs"]
mod tests;
