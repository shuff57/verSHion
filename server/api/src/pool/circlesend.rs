//! Sending a version to a circle's bank (worker/src/pool/circlesend.ts `sendVersion`): POST /circles/:cid/banks (a new bank) and PUT
//! /circles/:cid/banks/:bid (a new version). HTTP-free: membership and role first (a non-member is a counted 404 whatever the body), the day's
//! `circle_commits` allowance and the account lock, the cheap precheck (replay, stale head, licence rule, merge token) before the heavy slot, then
//! the payload, the commit and its answer.
use super::circlebanks::{bank_or_miss, require_readable};
use super::circlebase::{circle_or_miss, require_open, require_role};
use super::circlecommit::commit_version;
use super::circlerollback::locked_daily;
use super::circlesendprep::{Pre, ReadSend, SendEnv, precheck, send_once};
use super::ctx::{Fail, PoolError, Res};
use super::jsonscan::scan_json;
use super::poolio::{is_publish_type, parse_publish};
use super::publishonce::{MAX_META, MAX_PAYLOAD};
use super::publishrun::{acquire_account_lock, refresh_account_lock, release_account_lock};
use serde_json::Value;

/// `path_bid` None is a new bank. `enter_heavy` takes the heavy slot once the cheap checks have passed (a replay or a stale send never queues for it).
/// Returns the status and body to send.
pub fn send_version(env: &SendEnv, me: &str, cid: &str, path_bid: Option<&str>, content_type: Option<&str>, body: &[u8], enter_heavy: &mut dyn FnMut() -> Res<()>) -> Res<(u16, Value)> {
    let x = env.x;
    // Membership and role first (cheap; a non-member is a counted 404 whatever the body).
    let m = match path_bid {
        None => circle_or_miss(x, cid, me)?,
        Some(bid) => {
            let (m, b) = bank_or_miss(x, cid, bid, me)?;
            require_open(&m)?;
            require_role(&m, &["owner", "editor"])?;
            // Any bank the sender may read (removed: 409 bank_removed; held or under review: its author only, else 404).
            require_readable(x, &m, &b, me)?;
            m
        }
    };
    require_open(&m)?;
    require_role(&m, &["owner", "editor"])?;
    if !is_publish_type(content_type) {
        return Err(PoolError::new(415, "unsupported_media_type").into());
    }
    locked_daily(env, me, "circle_commits", || {
        let Some(lock) = acquire_account_lock(x, me)? else {
            x.throw_if_erased(me)?;
            return Err(PoolError::new(409, "publish_in_progress").with("message", "Another publish or send from this account is still running.".into()).header("Retry-After", "5").into());
        };
        let out = (|| -> Res<(u16, Value, bool)> {
            let (meta, start) = parse_publish(body, MAX_PAYLOAD, MAX_META)?;
            let read = ReadSend { raw: body, meta: &meta, payload: &body[start..] };
            // A replay and a send based on an old head are answered before the heavy slot.
            let pre = match precheck(env, me, cid, path_bid, &read)? {
                Pre::Replay(v) => return Ok((200, v, true)),
                Pre::Go(p) => p,
            };
            refresh_account_lock(x, me, &lock)?;
            enter_heavy()?;
            let scan_ok = scan_json(read.payload).ok;
            let spec = send_once(env, me, cid, path_bid, &pre, &read, scan_ok)?;
            refresh_account_lock(x, me, &lock)?;
            let (status, v) = commit_version(x, env.store, &spec)?;
            Ok((status, v, false))
        })();
        release_account_lock(x, me, &lock).map_err(Fail::from)?;
        out
    })
}

#[cfg(test)]
#[path = "circlesend_tests.rs"]
mod tests;
