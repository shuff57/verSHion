//! The pool's cron stages on a timer (worker/src/pool/cron.ts `hourly` and `daily`, driven by the Worker's `scheduled`): the core stages plus the two that
//! need the fingerprint key (the stem backfill and the licence re-key). HTTP-free; `main` calls these from its hourly tick (the daily stage at 04:xx UTC).
//! Not ported: the legacy `backfill` of items published before rounds 11-12 (see the cutover checklist) and the D1 size check (SQLite on a disk has no
//! such limit).
use super::croncore::{HourlyReport, hourly, stage};
use super::crondaily::{DailyReport, daily};
use super::cronpublish::{licence_rekey, stem_backfill};
use super::ctx::PoolCtx;
use super::routes::PoolState;

/// One hourly run. Notices the outbox cannot deliver stay in it (no pool mail is wired yet, so every delivery is refused and retried next hour).
pub fn run_hourly(st: &PoolState) -> HourlyReport {
    let conn = st.conn.lock().unwrap_or_else(|p| p.into_inner());
    let x = PoolCtx::new(&conn, (st.clock)());
    let mut deliver = |n: &super::ctx::PoolNotice| -> Result<(), String> {
        tracing::warn!(?n, "pool notice kept in the outbox (mail not wired yet)");
        Err("pool mail is not wired".into())
    };
    let report = hourly(&x, &st.store, &mut deliver);
    stage(&x, "stem_backfill", 0, || stem_backfill(&x, &st.store, &st.fp));
    stage(&x, "licence_rekey", 0, || licence_rekey(&x));
    (st.notify)(x.take_notices());
    report
}

/// One daily run.
pub fn run_daily(st: &PoolState) -> DailyReport {
    let conn = st.conn.lock().unwrap_or_else(|p| p.into_inner());
    let x = PoolCtx::new(&conn, (st.clock)());
    // A daily run may be the only cron a deployment gets: finish the stem backfill and old licence keys first.
    stage(&x, "stem_backfill", 0, || stem_backfill(&x, &st.store, &st.fp));
    stage(&x, "licence_rekey", 0, || licence_rekey(&x));
    let report = daily(&x, &st.store);
    (st.notify)(x.take_notices());
    report
}

#[cfg(test)]
#[path = "cronrun_tests.rs"]
mod tests;
