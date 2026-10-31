//! The AI part of the Worker's hourly cron (`pruneAi`): expired jobs, their blob results and old usage rows.
use super::routes::AiState;
use super::upstream::Transport;

/// Returns how many jobs were dropped. A failure is logged and reported as zero: it must not stop the other stages.
pub fn prune<T: Transport>(st: &AiState<T>, job_ttl_ms: i64) -> usize {
    let ids = match st.store(|s| s.prune(st.now(), job_ttl_ms)) {
        Ok(ids) => ids,
        Err(_) => {
            tracing::error!("ai: prune failed");
            return 0;
        }
    };
    if let Some(blobs) = &st.blobs {
        for id in &ids {
            let _ = blobs.delete(&format!("ai/jobs/{id}/result.json"));
        }
    }
    ids.len()
}
