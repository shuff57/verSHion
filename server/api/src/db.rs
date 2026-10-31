//! The SQLite file and its migrations.
//!
//! The Worker's `migrations/NNNN_name.sql` files are the schema, unchanged: this applies them in name
//! order and records each in `_migrations`, so a restart applies only what is new. WAL mode and a busy
//! timeout let readers run while one writer commits; a Worker `batch([...])` becomes one transaction.
use rusqlite::Connection;
use std::path::Path;

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(conn)
}

/// Apply every `*.sql` file in `dir` that `_migrations` does not list yet, each in its own transaction.
/// Returns the names applied this call. A failing file rolls back alone and stops the run.
pub fn migrate(conn: &mut Connection, dir: &Path) -> Result<Vec<String>, String> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS _migrations (name TEXT PRIMARY KEY, applied_at INTEGER NOT NULL)")
        .map_err(|e| e.to_string())?;
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "sql"))
        .collect();
    files.sort();
    let mut applied = Vec::new();
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let seen: bool = conn
            .query_row("SELECT 1 FROM _migrations WHERE name = ?1", [&name], |_| Ok(true))
            .unwrap_or(false);
        if seen {
            continue;
        }
        let sql = std::fs::read_to_string(&path).map_err(|e| format!("{name}: {e}"))?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute_batch(&sql).map_err(|e| format!("{name}: {e}"))?;
        tx.execute(
            "INSERT INTO _migrations (name, applied_at) VALUES (?1, strftime('%s','now') * 1000)",
            [&name],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| format!("{name}: {e}"))?;
        applied.push(name);
    }
    Ok(applied)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worker_migrations() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")
    }

    #[test]
    fn every_worker_migration_applies_to_a_fresh_file_and_only_once() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = open(&dir.path().join("pool.db")).unwrap();
        let first = migrate(&mut conn, &worker_migrations()).unwrap();
        assert!(first.len() >= 26, "applied {}", first.len());
        assert!(first.windows(2).all(|w| w[0] < w[1]), "applied in name order");
        // The newest tables exist: credits ledger and AI cost columns.
        conn.prepare("SELECT delta, kind, ref, cents FROM credit_ledger").unwrap();
        conn.prepare("SELECT cost_usd, session FROM ai_usage").unwrap();
        assert!(migrate(&mut conn, &worker_migrations()).unwrap().is_empty(), "a rerun applies nothing");
    }

    #[test]
    fn a_failing_migration_rolls_back_alone_and_stops_the_run() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("0001_ok.sql"), "CREATE TABLE a (x)").unwrap();
        std::fs::write(dir.path().join("0002_bad.sql"), "CREATE TABLE b (x); NOT SQL").unwrap();
        std::fs::write(dir.path().join("0003_never.sql"), "CREATE TABLE c (x)").unwrap();
        let mut conn = Connection::open_in_memory().unwrap();
        let err = migrate(&mut conn, dir.path()).unwrap_err();
        assert!(err.starts_with("0002_bad.sql"), "{err}");
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name IN ('a','b','c')")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(tables, vec!["a".to_string()]);
    }
}
