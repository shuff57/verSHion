//! Tests for `modgc.rs`: blob gc keeps what is referenced or pending, orphan retries back off, strikes raise one flag per count, voiding is reversible.
use super::*;
use crate::db;
use std::path::Path;

fn world() -> (Connection, tempfile::TempDir, BlobStore) {
    let d = tempfile::tempdir().unwrap();
    let mut c = db::open(&d.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    let s = BlobStore::new(d.path().join("blobs")).unwrap();
    (c, d, s)
}
fn blob(c: &Connection, s: &BlobStore, sha: &str) {
    c.execute("INSERT INTO blobs (sha256, size, content_type, width, height, first_uploaded_at) VALUES (?1, 1, 'image/png', 1, 1, 1)", [sha]).unwrap();
    s.put(&format!("blobs/{sha}"), b"x").unwrap();
}
fn n(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn gc_deletes_only_unreferenced_unpending_blobs() {
    let (c, _d, s) = world();
    for h in ["aa", "bb", "cc", "dd"] {
        blob(&c, &s, h);
    }
    c.execute("INSERT INTO item_blobs (publish_id, form, sha256) VALUES ('p', 'full', 'bb')", []).unwrap();
    c.execute("INSERT INTO pending_blobs (sha256, uploader_id, size, until) VALUES ('cc', 'u', 1, 10000000)", []).unwrap();
    let x = PoolCtx::new(&c, 5_000_000);
    assert_eq!(gc_blobs(&x, &s, Some(&["aa".to_string(), "bb".to_string(), "cc".to_string()])).unwrap(), 1);
    assert!(s.get("blobs/aa").unwrap().is_none() && s.get("blobs/cc").unwrap().is_some() && s.get("blobs/dd").unwrap().is_some());
    // Without a list, everything unreferenced goes (dd), and a lapsed pending row protects nothing.
    c.execute("UPDATE pending_blobs SET until = 1", []).unwrap();
    let gone = gc_blobs(&PoolCtx::new(&c, 50_000_000), &s, None).unwrap();
    assert!(gone >= 2);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM blobs"), n(&c, "SELECT COUNT(*) FROM blobs b WHERE EXISTS (SELECT 1 FROM item_blobs i WHERE i.sha256 = b.sha256)"));
}

#[test]
fn orphans_are_forgotten_on_success() {
    let (c, _d, s) = world();
    s.put("items/k1", b"x").unwrap();
    c.execute("INSERT INTO r2_orphans (key, at) VALUES ('items/k1', 1), ('items/k2', 1)", []).unwrap();
    assert_eq!(retry_orphans(&PoolCtx::new(&c, 10), &s).unwrap(), 2);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM r2_orphans"), 0);
    assert!(s.get("items/k1").unwrap().is_none());
    // A key not yet due waits.
    c.execute("INSERT INTO r2_orphans (key, at, next_at) VALUES ('items/k3', 1, 1000)", []).unwrap();
    assert_eq!(retry_orphans(&PoolCtx::new(&c, 10), &s).unwrap(), 0);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM r2_orphans"), 1);
}

#[test]
fn a_failed_delete_backs_off_exponentially() {
    let (c, _d, s) = world();
    // An invalid key (a path escape) fails every time.
    c.execute("INSERT INTO r2_orphans (key, at) VALUES ('../bad', 1)", []).unwrap();
    assert_eq!(retry_orphans(&PoolCtx::new(&c, 100), &s).unwrap(), 0);
    let (att, next): (i64, i64) = c.query_row("SELECT attempts, next_at FROM r2_orphans", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!((att, next), (1, 100 + 3_600_000));
    assert_eq!(retry_orphans(&PoolCtx::new(&c, 200), &s).unwrap(), 0);
    assert_eq!(n(&c, "SELECT attempts FROM r2_orphans"), 1, "not due yet");
    retry_orphans(&PoolCtx::new(&c, 100 + 3_600_000), &s).unwrap();
    assert_eq!(n(&c, "SELECT next_at FROM r2_orphans"), 100 + 3_600_000 + 7_200_000);
}

fn strikes(c: &Connection, user: &str, k: i64, at: i64, upheld: bool) {
    let base = n(c, &format!("SELECT COUNT(*) FROM strikes WHERE user_id = '{user}'"));
    for i in base..base + k {
        c.execute("INSERT INTO strikes (id, user_id, action_id, at, upheld_at) VALUES (?1, ?2, ?3, ?4, ?5)", params![format!("s{user}{i}"), user, format!("a{user}{i}"), at, upheld.then_some(at)]).unwrap();
    }
}

#[test]
fn three_upheld_strikes_in_a_year_flag_once_and_mail() {
    let (c, _d, _s) = world();
    let now = 400 * 86_400_000;
    strikes(&c, "u", 2, now - 1000, true);
    let x = PoolCtx::new(&c, now);
    repeat_infringer_check(&x, "u").unwrap();
    assert_eq!(n(&c, "SELECT COUNT(*) FROM admin_flags"), 0);
    strikes(&c, "u", 1, now - 500, true);
    repeat_infringer_check(&x, "u").unwrap();
    repeat_infringer_check(&x, "u").unwrap();
    assert_eq!(n(&c, "SELECT COUNT(*) FROM admin_flags WHERE kind = 'repeat_infringer'"), 1);
    assert_eq!(x.take_notices().len(), 1);
    // Un-upheld strikes and strikes older than a year do not count; the sweep finds a missed account.
    strikes(&c, "w", 3, now - 1000, false);
    strikes(&c, "old", 3, now - 400 * 86_400_000, true);
    strikes(&c, "v", 3, now - 1000, true);
    assert_eq!(repeat_infringer_sweep(&PoolCtx::new(&c, now)).unwrap(), 1);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM admin_flags WHERE user_id = 'v'"), 1);
}

#[test]
fn voiding_marks_served_rows_and_the_item_and_restore_undoes_it() {
    let (c, _d, _s) = world();
    c.execute("INSERT INTO items (id, author_id, kind, visibility, license, created_at, updated_at) VALUES ('i1', 'a', 'bank', 'public', 'cc-by', 1, 1)", []).unwrap();
    c.execute("INSERT INTO served (reader_id, item_id, publish_id, kind, license, licenses_json, first_at, at) VALUES ('r', 'i1', 'p', 'import', 'cc-by', '[]', 1, 1)", []).unwrap();
    void_evidence(&c, Who::Item("i1"), 50, false, Some("admin"), "takedown_upheld", None).unwrap();
    assert_eq!(n(&c, "SELECT void_at FROM served"), 50);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM void_items WHERE item_id = 'i1' AND reason = 'takedown_upheld'"), 1);
    void_evidence(&c, Who::Item("i1"), 60, true, None, "x", None).unwrap();
    assert_eq!(n(&c, "SELECT COUNT(*) FROM served WHERE void_at IS NOT NULL"), 0);
    assert_eq!(n(&c, "SELECT COUNT(*) FROM void_items"), 0);
    // By reader: only the served rows.
    void_evidence(&c, Who::User("r"), 70, false, None, "admin", None).unwrap();
    assert_eq!((n(&c, "SELECT void_at FROM served"), n(&c, "SELECT COUNT(*) FROM void_items")), (70, 0));
}
