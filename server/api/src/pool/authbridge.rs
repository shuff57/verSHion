//! The pool's view of the auth package: who is signed in (`PoolAuth`, trust read from the database on every request) and who may call the admin
//! routes (`PoolAdminGuard`: admin level, a known device, a sudo token for writes). Both run over the auth package's own connection to the file.
use super::ctx::PoolError;
use super::gate::Caller;
use super::routes::{PoolAdminGuard, PoolAuth};
use crate::auth::admin::guard_admin;
use crate::auth::config::AuthConfig;
use crate::auth::crypto::{Keys, derive_keys};
use crate::auth::http::{HttpError, client_info};
use crate::auth::session::{Ctx, authenticate};
use axum::http::HeaderMap;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

pub struct AuthBridge {
    db: Mutex<Connection>,
    cfg: Arc<AuthConfig>,
    keys: Keys,
    clock: Box<dyn Fn() -> i64 + Send + Sync>,
}

impl AuthBridge {
    pub fn new(db: Connection, cfg: Arc<AuthConfig>, clock: Box<dyn Fn() -> i64 + Send + Sync>) -> Self {
        let keys = derive_keys(&cfg.secret);
        Self { db: Mutex::new(db), cfg, keys, clock }
    }
    fn ctx(&self) -> Ctx<'_> {
        Ctx { db: &self.db, cfg: &self.cfg, keys: &self.keys, now: (self.clock)() }
    }
}

fn pool_err(e: HttpError) -> PoolError {
    let mut p = PoolError::new(e.status, &e.code);
    for (k, v) in e.extra {
        p = p.with(&k, v);
    }
    for (k, v) in e.headers {
        p = p.header(&k, v);
    }
    p
}

/// One bridge serves both roles, shared behind an `Arc`.
impl PoolAuth for Arc<AuthBridge> {
    fn caller(&self, headers: &HeaderMap) -> Option<Caller> {
        (**self).caller(headers)
    }
}
impl PoolAdminGuard for Arc<AuthBridge> {
    fn admin(&self, headers: &HeaderMap, write: bool) -> Result<String, PoolError> {
        (**self).admin(headers, write)
    }
}

impl PoolAuth for AuthBridge {
    fn caller(&self, headers: &HeaderMap) -> Option<Caller> {
        authenticate(&self.ctx(), headers).map(|p| Caller { user_id: p.user_id, trust: p.trust, family: Some(p.session_family) })
    }
}

impl PoolAdminGuard for AuthBridge {
    fn admin(&self, headers: &HeaderMap, write: bool) -> Result<String, PoolError> {
        let client = client_info(headers, &self.cfg.allowed_origins).map_err(pool_err)?;
        guard_admin(&self.ctx(), headers, &client, write).map(|a| a.user_id).map_err(pool_err)
    }
}

#[cfg(test)]
#[path = "authbridge_tests.rs"]
mod tests;
