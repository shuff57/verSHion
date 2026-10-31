//! Circles email invites (worker/src/pool/circlemail.ts): POST /circles/:cid/invites/email {email, role}. An ordinary single-use circle code
//! mailed from a fixed text that names no circle and no person; no account is ever looked up (no oracle); only HMACs of the mailbox are kept
//! and a masked label for the owner's list. The mail itself is a `PoolNotice::CircleEmailInvite` for the mailer (SMTP).
use super::circlebase::{CIRCLE_CODE_DAYS, circle_code_hash, circle_or_miss, explain, new_circle_code, normalise_circle_code, require_open, require_role};
use super::circles::bad_field;
use super::circlesql::*;
use super::ctx::{AuditEntry, Fail, PoolCtx, PoolError, PoolNotice, Res, audit};
use super::poolkeys::PoolKeys;
use super::sqlfrag::{DAY_MS, HOUR_MS, alive_sql, day_of, new_id};
use rusqlite::{OptionalExtension, params};
use serde_json::{Map, Value, json};
use unicode_general_category::{GeneralCategory as G, get_general_category};
use unicode_normalization::UnicodeNormalization;

/// Mails an inviter may have sent this hour and still use the hour's last fifth (a share kept for others).
pub const EMAIL_SMALL_INVITER: i64 = 3;

fn printable(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| (0x21..=0x7e).contains(&b))
}
fn local_ok(l: &str) -> bool {
    let n = l.chars().count();
    (1..=64).contains(&n) && !l.chars().any(|c| c.is_whitespace() || "@\"(),:;<>[]\\".contains(c))
}
fn label_ok(l: &str) -> bool {
    let b = l.as_bytes();
    (1..=63).contains(&b.len())
        && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
        && b[0] != b'-'
        && b[b.len() - 1] != b'-'
}

/// The auth package's address rule: a plain local part and a host name with a dot, at most 254 characters. Typed in NFKC, an IDN domain in its
/// ASCII (punycode) form, trailing dots dropped; the local part must then be printable ASCII.
pub fn normalise_invite_email(raw: &Value) -> Option<String> {
    let raw = raw.as_str()?;
    if raw.encode_utf16().count() > 320 {
        return None;
    }
    let t: String = raw.nfkc().collect();
    let t = t.trim();
    let at = t.rfind('@').filter(|a| *a > 0)?;
    let local = &t[..at];
    let mut domain = t[at + 1..].trim_end_matches('.').to_string();
    if !printable(local) || domain.is_empty() {
        return None;
    }
    if !printable(&domain) {
        // Only letters, digits, dots and hyphens of any script reach the IDN conversion.
        let ok = domain.chars().all(|c| c == '.' || c == '-' || c.is_alphanumeric() || matches!(get_general_category(c), G::NonspacingMark | G::SpacingMark | G::EnclosingMark));
        if !ok {
            return None;
        }
        domain = url::Url::parse(&format!("http://{domain}/")).ok()?.host_str()?.to_string();
    }
    let e = format!("{local}@{domain}").to_lowercase();
    if e.len() > 254 {
        return None;
    }
    let (l, d) = e.rsplit_once('@')?;
    let labels: Vec<&str> = d.split('.').collect();
    (local_ok(l) && !l.contains('@') && d.len() <= 253 && labels.len() >= 2 && labels.iter().all(|x| label_ok(x))).then_some(e)
}
/// An address this service's mailer sends to: a letters-only top-level domain.
pub fn is_deliverable(e: &str) -> bool {
    if !(3..=254).contains(&e.len()) || !printable(e) {
        return false;
    }
    let Some((l, d)) = e.split_once('@') else { return false };
    let Some((head, tld)) = d.rsplit_once('.') else { return false };
    local_ok(l)
        && !d.contains('@')
        && (1..=253).contains(&head.len())
        && head.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
        && (2..=63).contains(&tld.len())
        && tld.bytes().all(|b| b.is_ascii_lowercase())
}
/// The MAILBOX an address reaches, for the per-address cap: a `+tag` dropped, Gmail's dots ignored and googlemail.com read as gmail.com.
pub fn email_cap_key(e: &str) -> String {
    let at = e.rfind('@').unwrap_or(0);
    let mut local = e[..at].to_string();
    let mut domain = e[at + 1..].to_string();
    if let Some(p) = local.find('+') {
        local.truncate(p);
    }
    if domain == "googlemail.com" {
        domain = "gmail.com".into();
    }
    if domain == "gmail.com" {
        local = local.replace('.', "");
    }
    format!("{local}@{domain}")
}
/// The masked label an owner's list shows: the first character and the domain; a one-character local part is not shown at all.
pub fn mask_email(e: &str) -> String {
    let at = e.rfind('@').unwrap_or(0);
    format!("{}***{}", if at > 1 { &e[..e.chars().next().map(char::len_utf8).unwrap_or(1)] } else { "" }, &e[at..])
}

fn not_sent(reason: &str) -> Fail {
    PoolError::new(422, "invite_not_sent")
        .with("emailed", json!(false))
        .with("reason", json!(reason))
        .with(
            "message",
            json!(if reason == "address_limit" {
                "No invitation was sent: this address has had as many circle invitations today as it may get. Try again tomorrow, or give the person a code yourself."
            } else {
                "No invitation was sent: we cannot send mail to this address. Check it, or give the person a code yourself."
            }),
        )
        .into()
}
fn circle_full(limit: i64) -> Fail {
    PoolError::new(409, "circle_full").with("limit", json!(limit)).with("message", json!(format!("This circle has {limit} members, the most allowed."))).into()
}

/// What `give_back` needs: the day's counts taken and the hour's slots taken.
struct Taken<'a> {
    x: &'a PoolCtx<'a>,
    me: &'a str,
    hour: i64,
    daily: Vec<&'static str>,
    slots: Vec<String>,
}
impl Taken<'_> {
    fn give_back(&self) {
        for m in &self.daily {
            let _ = self.x.give_daily(self.me, m, 1);
        }
        for ch in &self.slots {
            let _ = self.x.conn.execute("UPDATE pool_alert_budget SET sent = MAX(0, sent - 1) WHERE hour = ?1 AND channel = ?2", params![self.hour, ch]);
        }
    }
    /// One of the hour's counters: its count after taking one, or None at its cap.
    fn slot(&mut self, channel: &str, cap: i64) -> Res<Option<i64>> {
        let r: Option<i64> = self
            .x
            .conn
            .query_row(
                "INSERT INTO pool_alert_budget (hour, channel, sent) SELECT ?1, ?2, 1 WHERE ?3 >= 1 ON CONFLICT (hour, channel) DO UPDATE SET sent = sent + 1 WHERE sent < ?3 RETURNING sent",
                params![self.hour, channel, cap],
                |r| r.get(0),
            )
            .optional()?;
        if r.is_some() {
            self.slots.push(channel.to_string());
        }
        Ok(r)
    }
}

/// POST /circles/:cid/invites/email {email, role}: 202 {ok, invite_id, kind: "email", role, max_uses, expires_at, emailed: true, address_label}.
/// A second invite to a mailbox with an open email invite in the circle ROTATES it (`resent: true`).
pub fn email_invite(x: &PoolCtx, keys: &PoolKeys, me: &str, cid: &str, b: &Map<String, Value>) -> Res<(u16, Value)> {
    let m = circle_or_miss(x, cid, me)?;
    for k in b.keys() {
        if k != "email" && k != "role" {
            return Err(bad_field(k, &[("message", json!("Only an address and a role."))]));
        }
    }
    let Some(email) = b.get("email").and_then(|e| normalise_invite_email(&Value::String(e.as_str().unwrap_or_default().into())).filter(|_| e.is_string())) else {
        return Err(bad_field("email", &[("message", json!("This is not an email address we can send to."))]));
    };
    let role = match b.get("role").and_then(Value::as_str) {
        Some(r @ ("editor" | "viewer")) => r.to_string(),
        _ => return Err(bad_field("role", &[])),
    };
    require_open(&m)?;
    require_role(&m, &["owner"])?;
    let now = x.now;
    let day = day_of(now);
    // Up front, before anything is charged or made: a full circle; an address our mailer does not send to; a mailbox that had its mails for
    // today. Each depends on the address string (and today's count for it), never on whether anyone has an account.
    let members_cap = x.limit_of("*", "circle_members")?;
    let members: i64 = x.conn.query_row(&format!("SELECT {}", members_sql("?1")), [cid], |r| r.get(0))?;
    if members >= members_cap {
        return Err(circle_full(members_cap));
    }
    if !is_deliverable(&email) {
        return Err(not_sent("not_deliverable"));
    }
    let per_address = x.limit_of("*", "circle_email_address")?;
    let cap_key = email_cap_key(&email);
    let addr = keys.email_mac(&format!("circle-invite-email/v1:{cap_key}"));
    let sent_today = |x: &PoolCtx| -> rusqlite::Result<i64> { Ok(x.conn.query_row("SELECT n FROM circle_email_sends WHERE addr = ?1 AND day = ?2", params![addr, day], |r| r.get(0)).optional()?.unwrap_or(0)) };
    if sent_today(x)? >= per_address {
        return Err(not_sent("address_limit"));
    }
    let email_key = keys.email_mac(&format!("circle-invite-key/v1:{cid}:{cap_key}"));
    let open: Option<String> = x
        .conn
        .query_row(
            "SELECT id FROM circle_invites INDEXED BY circle_invites_email_key WHERE circle_id = ?1 AND email_key = ?2 AND kind = 'email' AND revoked_at IS NULL AND uses < max_uses AND expires_at > ?3
              ORDER BY created_at DESC LIMIT 1",
            params![cid, email_key, now],
            |r| r.get(0),
        )
        .optional()?;
    x.require_daily(me, "circle_email_invites", 1)?;
    let hour = now.div_euclid(HOUR_MS);
    let mut taken = Taken { x, me, hour, daily: vec!["circle_email_invites"], slots: vec![] };
    let domain_part = cap_key.rsplit_once('@').map(|d| d.1).unwrap_or_default();
    let domain_key = format!("circle_email_domain:{}", keys.email_mac(&format!("circle-invite-domain/v1:{domain_part}")));
    let retry = ((((hour + 1) * HOUR_MS - now) as f64) / 1000.0).ceil().max(1.0) as i64;
    let slots_run = |taken: &mut Taken| -> Res<()> {
        // (a rotated invite is no new code: it does not count toward the day's codes)
        if open.is_none() {
            x.require_daily(me, "circle_invites", 1)?;
            taken.daily.push("circle_invites");
        }
        // The hour: this inviter's share, this domain's share, then the service's budget, whose last fifth is kept for inviters who sent little.
        let mine = taken.slot(&format!("circle_email_inviter:{me}"), x.limit_of(me, "circle_email_invites_hourly")?)?;
        let Some(mine) = mine else {
            return Err(PoolError::new(429, "hourly_limit")
                .with("metric", json!("circle_email_invites_hourly"))
                .with("message", json!("You have sent as many circle invitations as you may this hour. Try again later."))
                .header("Retry-After", retry.to_string())
                .into());
        };
        let domain_cap = x.limit_of("*", "circle_email_domain_hourly")?;
        let share = domain_cap.min(x.limit_of(me, "circle_email_inviter_domain_hourly")?);
        let Some(here) = taken.slot(&format!("{domain_key}:{me}"), share)? else {
            return Err(PoolError::new(429, "hourly_limit")
                .with("metric", json!("circle_email_inviter_domain_hourly"))
                .with("limit", json!(share))
                .with("message", json!(format!("You have sent as many invitations to addresses at this mail domain as you may in an hour ({share}). Try again in an hour.")))
                .header("Retry-After", retry.to_string())
                .into());
        };
        let dcap = if here <= EMAIL_SMALL_INVITER { domain_cap } else { domain_cap - (domain_cap as f64 / 5.0).ceil() as i64 };
        if taken.slot(&domain_key, dcap)?.is_none() {
            return Err(PoolError::new(429, "hourly_limit")
                .with("metric", json!("circle_email_domain_hourly"))
                .with("message", json!("There have been too many invitations to addresses at this mail domain in the last hour. Try again in an hour."))
                .header("Retry-After", retry.to_string())
                .into());
        }
        let site = x.limit_of("*", "circle_email_hourly")?;
        let site_cap = if mine <= EMAIL_SMALL_INVITER { site } else { site - (site as f64 / 5.0).ceil() as i64 };
        if taken.slot("circle_email_invite", site_cap)?.is_none() {
            return Err(PoolError::new(503, "busy").with("message", json!("Too many invitations are being sent right now. Try again later.")).header("Retry-After", retry.to_string()).into());
        }
        Ok(())
    };
    if let Err(e) = slots_run(&mut taken) {
        taken.give_back();
        return Err(e);
    }
    let cap = x.limit_of("*", "circle_open_email_invites")?;
    let code = new_circle_code();
    let label = mask_email(&email);
    let id = open.clone().unwrap_or_else(new_id);
    let expires_at = now + CIRCLE_CODE_DAYS * DAY_MS;
    let code_hash = circle_code_hash(&normalise_circle_code(&Value::String(code.clone())).unwrap_or_default());
    let rotated = open.is_some();
    let made = (|| -> Res<Option<String>> {
        let tx = x.conn.unchecked_transaction()?;
        // The mailbox's count of the day first, by its HMAC only, and only while it has room: no race past the cap.
        tx.execute("INSERT INTO circle_email_sends (addr, day, n) SELECT ?1, ?2, 1 WHERE ?3 >= 1 ON CONFLICT (addr, day) DO UPDATE SET n = n + 1 WHERE n < ?3", params![addr, day, per_address])?;
        if rotated {
            tx.execute(
                &format!(
                    "UPDATE circle_invites SET code_hash = ?3, role = ?4, created_by = ?5, created_at = ?6, expires_at = ?7
                      WHERE id = ?1 AND circle_id = ?2 AND kind = 'email' AND revoked_at IS NULL AND uses < max_uses AND expires_at > ?6 AND changes() = 1
                        AND {} AND {} AND {} AND {} AND {} < ?8",
                    open_sql("?2"),
                    active_sql("?2", "?5", &["owner"]),
                    alive_sql("?5"),
                    in_standing_sql("?5"),
                    members_sql("?2")
                ),
                params![id, cid, code_hash, role, me, now, expires_at, members_cap],
            )?;
            tx.execute("UPDATE circle_email_sends SET n = MAX(0, n - 1) WHERE addr = ?1 AND day = ?2 AND NOT EXISTS (SELECT 1 FROM circle_invites WHERE id = ?3 AND code_hash = ?4)", params![addr, day, id, code_hash])?;
            let cond = format!("EXISTS (SELECT 1 FROM circle_invites WHERE id = '{id}' AND code_hash = '{code_hash}')");
            audit(&tx, &AuditEntry { actor_id: Some(me), actor_kind: "user", action: "circle.email_invite", item_id: Some(cid), target_user_id: None, detail: Some(json!({ "invite_id": id, "role": role, "max_uses": 1, "expires_at": expires_at, "resent": true })), note: None }, now, Some(&cond))?;
            let row = tx.query_row("SELECT email_label FROM circle_invites WHERE id = ?1 AND code_hash = ?2", params![id, code_hash], |r| r.get::<_, Option<String>>(0)).optional()?;
            tx.commit()?;
            return Ok(row.map(|l| l.unwrap_or_default()));
        }
        tx.execute(
            &format!(
                "INSERT INTO circle_invites (id, circle_id, code_hash, role, created_by, created_at, max_uses, uses, expires_at, note, kind, email_key, email_label)
                 SELECT ?1, ?2, ?3, ?4, ?5, ?6, 1, 0, ?7, NULL, 'email', ?11,
                        (SELECT CASE WHEN COUNT(*) = 0 THEN ?10 ELSE ?10 || ' #' || (COUNT(*) + 1) END FROM circle_invites li
                          WHERE li.circle_id = ?2 AND li.kind = 'email' AND (li.email_label = ?10 OR substr(li.email_label, 1, length(?10) + 2) = ?10 || ' #'))
                  WHERE changes() = 1 AND {} AND {} AND {} AND {} AND {} < ?8 AND {} < ?9",
                open_sql("?2"),
                active_sql("?2", "?5", &["owner"]),
                alive_sql("?5"),
                in_standing_sql("?5"),
                open_invites_sql("?2", "?6", true),
                members_sql("?2")
            ),
            params![id, cid, code_hash, role, me, now, expires_at, cap, members_cap, label, email_key],
        )?;
        tx.execute("UPDATE circle_email_sends SET n = MAX(0, n - 1) WHERE addr = ?1 AND day = ?2 AND NOT EXISTS (SELECT 1 FROM circle_invites WHERE id = ?3)", params![addr, day, id])?;
        let cond = format!("EXISTS (SELECT 1 FROM circle_invites WHERE id = '{id}')");
        audit(&tx, &AuditEntry { actor_id: Some(me), actor_kind: "user", action: "circle.email_invite", item_id: Some(cid), target_user_id: None, detail: Some(json!({ "invite_id": id, "role": role, "max_uses": 1, "expires_at": expires_at })), note: None }, now, Some(&cond))?;
        let row = tx.query_row("SELECT email_label FROM circle_invites WHERE id = ?1", [&id], |r| r.get::<_, Option<String>>(0)).optional()?;
        tx.commit()?;
        Ok(row.map(|l| l.unwrap_or_default()))
    })();
    let row = match made {
        Ok(r) => r,
        Err(e) => {
            taken.give_back();
            return Err(e);
        }
    };
    let Some(stored) = row else {
        taken.give_back();
        x.throw_if_erased(me)?;
        if sent_today(x)? >= per_address {
            return Err(not_sent("address_limit"));
        }
        if rotated {
            let members: i64 = x.conn.query_row(&format!("SELECT {}", members_sql("?1")), [cid], |r| r.get(0))?;
            if members >= members_cap {
                return Err(circle_full(members_cap));
            }
            let e = explain(x, cid, me, Some(&["owner"]));
            // (the open invite was used, revoked or expired meanwhile: invite again for a new one)
            return Err(if matches!(&e, Fail::Pool(p) if p.code == "conflict") { PoolError::new(409, "conflict").with("message", json!("That invitation changed meanwhile. Invite the address again.")).into() } else { e });
        }
        let (n, members): (i64, i64) = x.conn.query_row(&format!("SELECT {}, {}", open_invites_sql("?1", "?2", true), members_sql("?1")), params![cid, now], |r| Ok((r.get(0)?, r.get(1)?)))?;
        if n >= cap {
            return Err(PoolError::new(422, "circles_limit")
                .with("metric", json!("circle_open_email_invites"))
                .with("used", json!(n))
                .with("limit", json!(cap))
                .with("message", json!(format!("This circle has {cap} email invitations that have not been used yet. More can be sent as people join, as invitations expire after {CIRCLE_CODE_DAYS} days, or when you withdraw one you no longer need.")))
                .into());
        }
        if members >= members_cap {
            return Err(circle_full(members_cap));
        }
        return Err(explain(x, cid, me, Some(&["owner"])));
    };
    x.send(PoolNotice::CircleEmailInvite { to: email, code, role: role.clone(), expires_at });
    let mut out = json!({ "ok": true, "invite_id": id, "kind": "email", "role": role, "max_uses": 1, "expires_at": expires_at, "emailed": true, "address_label": if rotated { stored } else if stored.is_empty() { label } else { stored } });
    if rotated {
        out["resent"] = json!(true);
        out["message"] = json!("This address already had an open invitation from this circle: a new code was mailed and the earlier one no longer works.");
    }
    Ok((202, out))
}

#[cfg(test)]
#[path = "circlemail_tests.rs"]
mod tests;
