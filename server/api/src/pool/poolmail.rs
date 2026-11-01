//! Mail for pool notices (worker/src/poolmail.ts). Ids only: no item text goes into a mail. `render` is pure (who, subject, text); `deliver` looks up an
//! author's current address and sends over the auth package's SMTP settings.
use super::circlemail::is_deliverable;
use super::ctx::PoolNotice;
use crate::auth::smtp::{MailEnv, send_raw, send_to};
use rusqlite::{Connection, OptionalExtension};

pub const CIRCLE_INVITE_SUBJECT: &str = "verSHion: you are invited to a circle";

#[derive(Debug, PartialEq)]
pub enum To {
    Admins,
    Everyone,
    /// An account's current address (None once it has none: nothing is sent).
    Author(String),
    Address(String),
}

#[derive(Debug, PartialEq)]
pub struct Mail {
    pub to: To,
    pub subject: String,
    pub text: String,
}

fn is_email(s: &str) -> bool {
    let Some((local, domain)) = s.split_once('@') else { return false };
    let bad = |c: char| c.is_whitespace() || "@\"(),:;<>[]\\".contains(c);
    (1..=64).contains(&local.chars().count()) && !local.chars().any(bad) && domain.contains('.') && domain.len() <= 253 && !domain.starts_with('.') && !domain.ends_with('.') && domain.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') && domain.rsplit('.').next().is_some_and(|t| (2..=63).contains(&t.len()) && t.chars().all(|c| c.is_ascii_alphabetic()))
}

fn is_circle_code(c: &str) -> bool {
    let parts: Vec<&str> = c.split('-').collect();
    parts.len() == 7 && parts[0] == "C" && parts[1..].iter().all(|p| p.len() == 4 && p.bytes().all(|b| b.is_ascii_digit() || b.is_ascii_uppercase()))
}

fn utc_minute(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", rem / 3600, rem % 3600 / 60)
}
fn utc_date(ms: i64) -> String {
    utc_minute(ms)[..10].to_string()
}

/// The circle invitation's fixed text: nothing a person chose, not even the circle's name.
pub fn circle_invite_text(app_url: &str, role: &str, code: &str, expires_at: i64) -> String {
    let as_ = if role == "editor" { "an editor" } else { "a viewer" };
    let link = if app_url.is_empty() { String::new() } else { format!("\nOr open this link:\n{}/circles/join#code={code}\n", app_url.trim_end_matches('/')) };
    format!(
        "You are invited to join a circle on verSHion as {as_}. A circle is a small private group of teachers who share question banks. The app shows you the circle's name and its owner before you join.\n\n\
         To join, you need a verSHion account that an administrator has approved and that has redeemed an invite code for the shared pool. If you do not have one, this code cannot be used yet.\n\n\
         In verSHion, open Shared, then Circles, then \"Join a circle with a code\", and enter this code:\n\n{code}\n{link}\n\
         The code works once and expires on {} UTC. If you did not expect this invitation, ignore this mail: nothing happens unless the code is used.",
        utc_minute(expires_at)
    )
}

fn admin(subject: impl Into<String>, text: impl Into<String>) -> Option<Mail> {
    Some(Mail { to: To::Admins, subject: subject.into(), text: text.into() })
}

/// What a notice mails, to whom. None: nothing to send (a malformed invite, an address that is not an email).
pub fn render(n: &PoolNotice, app_url: &str) -> Option<Mail> {
    use PoolNotice::*;
    match n {
        StudentDataReport { item_id, report_id } => admin(
            "verSHion: student data reported (item hidden)",
            format!("Item {item_id} was reported as containing student data and is hidden until you review it (report {report_id}). Confirm and purge, or release the hold, from the pool reports."),
        ),
        TakedownAuthor { item_id, author_id, action_id } => Some(Mail {
            to: To::Author(author_id.clone()),
            subject: "verSHion: an item was taken down after a copyright notice".into(),
            text: format!("One of your shared items ({item_id}) was taken down after a copyright notice (action {action_id}). If you believe it was removed by mistake or misidentification, you can send a counter-notice within 30 days from \"My shared items\" in the app (an item you deleted is listed there as \"taken down, awaiting your counter-notice\"). Each takedown that is not reversed counts as a strike under the repeat-infringer policy in the Terms."),
        }),
        CounterNotice { action_id, complainant_contact, restore_not_before } => {
            let text = format!("A counter-notice was received for DMCA action {action_id}. The material will be restored no sooner than {} (10 business days) unless you notify us that you have filed a court action.", utc_date(*restore_not_before));
            let c = complainant_contact.trim();
            if is_email(c) {
                Some(Mail { to: To::Address(c.to_string()), subject: "Counter-notice received".into(), text })
            } else {
                admin("verSHion: forward this counter-notice by hand", format!("{text}\n\nThe complainant's contact is not an email address; forward it to them yourself."))
            }
        }
        Cap { user_id, metric } => admin(format!("verSHion: an account reached its daily {metric} limit"), format!("Account {user_id} reached its daily {metric} limit. See the pool flags.")),
        PrivacyTextMatch { item_id, circle_id: Some(circle), version, .. } => admin(
            "verSHion: a circle bank matches confirmed student-data text",
            match version {
                Some(v) => format!("Version {v} of bank {item_id} in circle {circle} has question text that matches (or rewords) a confirmed student-data block. That version is withheld from every member; while it is the newest version the bank is hidden from the circle but for its author and owner, who can roll it back or replace it. Until you decide, the bank cannot be purged. See the pool flags (privacy_text_match); open it with GET /admin/pool/circle-bank?id={item_id}&version={v}&basis=privacy (audited). If it is a false positive, release THAT version: POST /admin/pool/circle-release {{\"bank_id\": \"{item_id}\", \"version\": {v}, \"note\": \"...\"}}. A bank-wide release (without \"version\") lifts the bank's holds but leaves the version withheld."),
                None => format!("A version of bank {item_id} in circle {circle} has question text that matches (or rewords) a confirmed student-data block; that version is withheld from every member and the bank cannot be purged until you decide. The privacy_text_match flag in the pool flags names the version: open it with GET /admin/pool/circle-bank (id, that version, basis=privacy; audited), and if it is a false positive release that version with POST /admin/pool/circle-release {{bank_id, version, note}}. A bank-wide release (without a version) leaves it withheld."),
            }
        ),
        PrivacyTextMatch { item_id, held, .. } => admin(
            "verSHion: a new publish matches confirmed student-data text",
            if *held {
                format!("Item {item_id} was just published with question text that matches a confirmed student-data block. It is held for review; its author sees only \"held for review\". See the pool flags (privacy_text_match), and release it with /admin/pool/release-hold if it is a false positive.")
            } else {
                format!("Item {item_id} was just published with question text close to (a rewording of) a confirmed student-data block. It is live: review it now in the pool flags (privacy_text_match, \"near\"), and hold or purge it if it contains the student's data.")
            },
        ),
        CronFailure { stage } => admin(format!("verSHion: pool maintenance step failed ({stage})"), format!("The scheduled pool step \"{stage}\" failed; the other steps still ran. It is retried on the next run. See the server logs and the pool flags (cron_failure).")),
        WasmTrap { traps } => admin("verSHion: share-core trapped repeatedly", format!("share-core trapped {traps} times in an hour; each trap failed one request. See the server logs and the pool flags (wasm_trap).")),
        KeyMismatch { key_id, stored_key_id } => admin(
            "verSHion: the pool refuses to run under a different hash key",
            format!("POOL_HASH_KEY in this deployment (key id {key_id}) is not the key the pool's data was made with (key id {stored_key_id}). Every pool write and cron run is refused (503 pool_key_mismatch) until the original key is restored. The key cannot be rotated: restore it from your backup."),
        ),
        D1Size { percent, bytes } => admin(format!("verSHion: pool database at {percent}% of its size limit"), format!("The pool database is {bytes} bytes, {percent}% of its size limit. Nothing has failed yet; plan room now. See the pool flags (d1_size).")),
        CopyHeld { item_id, author_id, cause } => {
            let why = if cause == "student_data" { "an admin confirmed that a question it shares with another item contains student data" } else { "a copyright takedown covers questions it shares with the item taken down" };
            Some(Mail { to: To::Author(author_id.clone()), subject: "verSHion: one of your shared items is on hold".into(), text: format!("Your shared item {item_id} is on hold: {why}. See \"My shared items\" in the app to remove those questions and republish, or to appeal.") })
        }
        KeyLeakReport { item_id, author_id } => Some(Mail {
            to: To::Author(author_id.clone()),
            subject: "verSHion: a teacher reported a visible answer hint".into(),
            text: format!("A teacher who uses your shared item {item_id} reported that what students see gives the hidden answer key away. Please check the question text and choices, fix them and republish. If two teachers report it, the item is held for review until you do."),
        }),
        CircleReport { circle_id, bank_id, report_id, reason, held } => admin(
            if reason == "student_data" { format!("verSHion: student data reported in a circle{}", if *held { " (bank hidden)" } else { "" }) } else { "verSHion: a circle bank was reported".to_string() },
            format!("Bank {bank_id} in circle {circle_id} was reported ({reason}, report {report_id}).{} See GET /admin/pool/circle-reports; opening its content is audited.", if *held { " It is hidden from the circle until you review it." } else { "" }),
        ),
        CircleCopyrightMatch { item_id, circle_id, version } => admin(
            "verSHion: a circle bank matches a copyright re-upload block",
            format!("Version {version} of bank {item_id} in circle {circle_id} holds material a takedown's re-upload block covers. That version is withheld from every member; while it is the newest version the bank is hidden from the circle but for its author and owner, who can roll it back or replace it. Until you decide, the bank cannot be purged. See the pool flags (blocklist); open it with GET /admin/pool/circle-bank?id={item_id}&version={version}&basis=copyright (audited). If the sender may share it, release THAT version: POST /admin/pool/circle-release {{\"bank_id\": \"{item_id}\", \"version\": {version}, \"note\": \"...\"}}. A bank-wide release (without \"version\") lifts the bank's holds but leaves the version withheld."),
        ),
        CircleEmailInvite { to, code, role, expires_at } => {
            if !is_deliverable(to) || !is_circle_code(code) {
                return None;
            }
            Some(Mail { to: To::Address(to.clone()), subject: CIRCLE_INVITE_SUBJECT.into(), text: circle_invite_text(app_url, role, code, *expires_at) })
        }
        RepeatInfringer { user_id, strikes } => Some(Mail {
            to: To::Everyone,
            subject: "verSHion: repeat-infringer threshold reached".into(),
            text: format!("Account {user_id} has {strikes} upheld copyright strikes in 12 months. Under the repeat-infringer policy, suspend it with /admin/suspend."),
        }),
    }
}

/// Sends one notice. An author who has no address any more gets nothing (that is success, not a retry).
pub fn deliver(env: &MailEnv, conn: &Connection, n: &PoolNotice) -> Result<(), String> {
    let Some(m) = render(n, &env.app_url) else { return Ok(()) };
    match m.to {
        To::Admins => send_to(env, false, &m.subject, &m.text),
        To::Everyone => send_to(env, true, &m.subject, &m.text),
        To::Address(a) => send_raw(env, &a, &m.subject, &m.text),
        To::Author(id) => {
            let email: Option<String> = conn.query_row("SELECT email FROM users WHERE id = ?1 AND deleted_at IS NULL", [&id], |r| r.get(0)).optional().map_err(|_| "database".to_string())?;
            match email.filter(|e| !e.is_empty()) {
                Some(e) => send_raw(env, &e, &m.subject, &m.text),
                None => Ok(()),
            }
        }
    }
}

#[cfg(test)]
#[path = "poolmail_tests.rs"]
mod tests;
