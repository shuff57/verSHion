//! Tests for `hooks.rs`: the audit row for suspension and restore, and the removal of a deleted author's items.
use super::*;
use crate::db;
use rusqlite::Connection;
use std::path::Path;

fn world() -> (Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut c = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    c.execute("INSERT INTO users (id, email, display_name, trust, created_at) VALUES ('u', 'u@x.org', 'U', 'teacher', 1)", []).unwrap();
    (c, dir)
}
fn actions(c: &Connection) -> Vec<String> {
    let mut st = c.prepare("SELECT action FROM pool_audit WHERE target_user_id = 'u' ORDER BY at, rowid").unwrap();
    st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
}

#[test]
fn suspension_and_restore_each_leave_one_audit_row_and_nothing_else() {
    let (mut c, _d) = world();
    c.execute("UPDATE users SET suspended_at = 5 WHERE id = 'u'", []).unwrap();
    let tx = c.transaction().unwrap();
    standing(&tx, "u", 5).unwrap();
    tx.commit().unwrap();
    c.execute("UPDATE users SET suspended_at = NULL WHERE id = 'u'", []).unwrap();
    let tx = c.transaction().unwrap();
    standing(&tx, "u", 6).unwrap();
    tx.commit().unwrap();
    assert_eq!(actions(&c), vec!["author_suspended", "author_restored"]);
}

#[test]
fn deleting_an_account_removes_its_items_at_once() {
    let (mut c, _d) = world();
    c.execute("INSERT INTO items (id, author_id, kind, visibility, license, created_at, updated_at) VALUES ('0123456789abcdef0123456789abcdef', 'u', 'bank', 'public', 'CC-BY-4.0', 1, 1)", []).unwrap();
    let tx = c.transaction().unwrap();
    on_delete(&tx, "u", 100).unwrap();
    tx.commit().unwrap();
    let (removed, reason, purge): (Option<i64>, Option<String>, Option<i64>) = c.query_row("SELECT removed_at, removed_reason, purge_after FROM items", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
    assert_eq!((removed, reason.as_deref(), purge), (Some(100), Some("account_deleted"), Some(100 + 14 * 86_400_000)));
}
