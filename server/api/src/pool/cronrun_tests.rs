//! Tests for `cronrun.rs`: one hourly and one daily run over a real migrated database.
use super::*;
use crate::db;
use crate::pool::core::Core;
use crate::pool::ctx::{PoolError, PoolNotice};
use crate::pool::fingerprint::Fingerprinter;
use crate::pool::gate::Caller;
use crate::pool::poolio::outbox_insert;
use crate::pool::poolkeys::PoolKeys;
use crate::pool::readslots::OpenReads;
use crate::pool::routes::{PoolAdminGuard, PoolAuth};
use crate::pool::publishrun::HeavySlots;
use axum::http::HeaderMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const ROOT: &str = "7f757fb737085dcaaad0727deb89a14c18141681dc56be0a215a592c715f8e3b";
const NOW: i64 = 50 * 86_400_000;

struct Nobody;
impl PoolAuth for Nobody {
    fn caller(&self, _: &HeaderMap) -> Option<Caller> {
        None
    }
}
impl PoolAdminGuard for Nobody {
    fn admin(&self, _: &HeaderMap, _: bool) -> Result<String, PoolError> {
        Err(PoolError::new(401, "unauthorized"))
    }
}

fn state() -> (PoolState, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut c = db::open(&dir.path().join("t.db")).unwrap();
    db::migrate(&mut c, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/migrations")).unwrap();
    let st = PoolState {
        conn: Mutex::new(c),
        store: crate::blobs::BlobStore::new(dir.path().join("blobs")).unwrap(),
        core: Core::new(Some(ROOT.into())),
        fp: Fingerprinter::from_root_hex(ROOT).unwrap(),
        keys: PoolKeys::from_secret("s"),
        limit_key: [1u8; 32],
        ip_prefix: 64,
        allowed_origins: vec![],
        auth: Box::new(Nobody),
        admin: Box::new(Nobody),
        clock: Arc::new(|| NOW),
        heavy: HeavySlots::default(),
        opens: OpenReads::new(),
        heavy_wait: Duration::from_secs(1),
        body_deadline: Duration::from_secs(1),
        notify: Arc::new(|_| {}),
    };
    (st, dir)
}

#[test]
fn the_hourly_run_drops_expired_uploads_and_keeps_notices_it_cannot_deliver() {
    let (st, _d) = state();
    {
        let c = st.conn.lock().unwrap();
        c.execute("INSERT INTO pending_blobs (sha256, uploader_id, size, until) VALUES (?1, 'u', 1, 5)", ["a".repeat(64)]).unwrap();
        let n = PoolNotice::Cap { user_id: "u".into(), metric: "publishes".into() };
        assert!(outbox_insert(&c, "n1", &n, NOW, "1").unwrap());
    }
    let r = run_hourly(&st, &mut |_| Err("no mail".to_string()));
    assert_eq!(r.pending, 1);
    let c = st.conn.lock().unwrap();
    let unsent: i64 = c.query_row("SELECT COUNT(*) FROM pool_outbox WHERE sent_at IS NULL", [], |r| r.get(0)).unwrap();
    assert_eq!(unsent, 1, "undeliverable notices wait for the next run");
    let flags: i64 = c.query_row("SELECT COUNT(*) FROM admin_flags WHERE kind = 'cron_failure'", [], |r| r.get(0)).unwrap();
    assert_eq!(flags, 0, "no stage failed");
}

#[test]
fn the_daily_run_completes_over_an_empty_pool() {
    let (st, _d) = state();
    let r = run_daily(&st);
    assert_eq!((r.purged, r.restored), (0, 0));
    let c = st.conn.lock().unwrap();
    let flags: i64 = c.query_row("SELECT COUNT(*) FROM admin_flags WHERE kind = 'cron_failure'", [], |r| r.get(0)).unwrap();
    assert_eq!(flags, 0);
}
