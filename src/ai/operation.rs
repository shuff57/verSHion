use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// An operation owns its credentials and session. Cloning its configuration shares only this
/// operation, never an ambient session from another task or another signed-in account.
pub struct OperationContext {
    pub id: String,
    pub service: String,
    pub session: Option<String>,
    key: Option<String>,
    token: Option<TokenSource>,
    cancelled: Arc<AtomicBool>,
}
impl std::fmt::Debug for OperationContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OperationContext")
            .field("service", &self.service)
            .field("cancelled", &self.cancelled.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}
impl OperationContext {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
    pub fn check(&self) -> Result<(), String> {
        if self.cancelled.load(Ordering::Relaxed) {
            Err("The AI operation was cancelled.".into())
        } else {
            Ok(())
        }
    }
    pub(super) fn key(&self) -> Option<String> {
        self.token
            .as_ref()
            .map(|source| source())
            .unwrap_or_else(|| self.key.clone())
    }
}

#[derive(Clone, Debug)]
pub struct Started {
    pub cfg: AiCfg,
    pub credits: u32,
    pub left: Option<u32>,
}

pub(super) async fn start(cfg: &AiCfg, service: &str, pages: usize) -> Result<Started, String> {
    start_controlled(cfg, service, pages, Arc::new(AtomicBool::new(false))).await
}
pub(super) async fn start_controlled(
    cfg: &AiCfg,
    service: &str,
    pages: usize,
    cancelled: Arc<AtomicBool>,
) -> Result<Started, String> {
    if vershion_ai_core::Task::parse(service).is_none() {
        return Err("Unknown AI service.".into());
    }
    if cancelled.load(Ordering::Relaxed) {
        return Err("The AI operation was cancelled.".into());
    }
    let mut random = [0u8; 16];
    getrandom::getrandom(&mut random)
        .map_err(|_| "Could not create an AI operation.".to_string())?;
    let id: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let mut cfg = cfg.clone();
    cfg.operation = None;
    let token = if cfg.provider == "account" {
        // Pin the account's token source as well as its endpoint; a later sign-in cannot
        // redirect this operation's images or charge a different account.
        ACCOUNT_AI.lock().ok().and_then(|a| {
            a.as_ref()
                .filter(|(base, _)| format!("{}/ai", base.trim_end_matches('/')) == cfg.base)
                .map(|(_, source)| source.clone())
        })
    } else {
        None
    };
    if cfg.provider == "account" && token.is_none() {
        account_signed_in_check(&cfg)?;
        return Err("The account changed. Start the operation again.".into());
    }
    let key = if let Some(source) = &token {
        source()
    } else {
        request_key(&cfg)
    };
    if cfg.provider == "account" && key.is_none() {
        return Err("Sign in on the Account screen before using verSHion AI.".into());
    }
    let mut credits = 0;
    let mut left = None;
    let mut session = None;
    if cfg.is_relay() {
        account_signed_in_check(&cfg)?;
        let body = json!({"service":service,"pages":pages,"operation":id,"protocol_version":vershion_ai_core::VERSION});
        // Do not retry blindly: old relays may not support idempotency.
        let reply = post_json_async(
            &format!("{}/service", cfg.base.trim_end_matches('/')),
            &body,
            key.as_deref(),
        )
        .await
        .map_err(|e| credit_message(&e))?;
        session = reply["session"].as_str().map(str::to_string);
        credits = reply["credits"].as_u64().unwrap_or(0) as u32;
        left = reply["left"].as_u64().map(|n| n as u32);
        // Older Worker deployments keep working without receiving new protocol fields.
        if (cfg.output_mode == OutputMode::Schema
            && reply["capabilities"]["structured_outputs"] != true)
            || (cfg.output_mode == OutputMode::Tool && reply["capabilities"]["tools"] != true)
        {
            cfg.output_mode = OutputMode::Json;
        }
        if reply["capabilities"]["tools"] != true {
            cfg.native_tools = false;
        }
    }
    cfg.output = match service {
        "import" => Some(vershion_ai_core::ditto()),
        "handwriting" => Some(vershion_ai_core::handwriting()),
        "answer_key" => Some(vershion_ai_core::answer_check()),
        _ => cfg.output.clone(),
    };
    cfg.operation = Some(Arc::new(OperationContext {
        id,
        service: service.into(),
        session,
        key,
        token,
        cancelled,
    }));
    Ok(Started { cfg, credits, left })
}
