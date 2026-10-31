//! PATCH /circles/:cid/banks/:bid {hide_key, ...decisions} (worker/src/pool/circles.ts patchBank / patchHideKey / hideOn / hideOff): turning the
//! answer-key hiding of a circle bank on or off, with the viewers' form checked against confirmed student data and copyright blocks.
//! D1's batches are one transaction here; its first statement is a compare-and-set and every later one is gated on that CAS having won (the
//! audit row it writes, `won`). The Worker's isolate heavy slot is dropped, as everywhere in the port.
use super::circlebankedit::{OWNER_OR_AUTHOR, patch_license};
use super::circlebanks::{BankRow, require_readable};
use super::circlebase::{Membership, explain, require_open};
use super::circlecommit::charge_held_publishes;
use super::circlehide::{DECISION_FIELDS, StoredStrip, StripDecisions, decisions_json, decisions_of, hash_pairs, read_strip_decisions, same_decisions, strip_stored, stripped_qkeys_sql, with_stored_full};
use super::circles::bad_field;
use super::circlesendmeta::{Scan, ScanForm, images_at, privacy_scan, strip_refusal};
use super::circlesendprep::SendEnv;
use super::circlesendsql::*;
use super::circlesql::*;
use super::core::questions_of;
use super::ctx::{Fail, PoolError, PoolNotice, Res, not_found};
use super::qjson::Json;
use super::sqlfrag::{DAY_MS, FLAG_DETAIL_MAX, alive_sql, day_of, new_id};
use rusqlite::{OptionalExtension, params};
use serde_json::{Map, Value, json};

/// A PATCH body's size at most (the strip decisions of a 1,000-question bank; the size of a send's meta).
pub const PATCH_BODY_MAX: usize = 1024 * 1024;
/// Older versions with no decisions recorded that a PATCH turning hidden keys on checks too, at most (by their stored bytes, and in number).
pub const HIDE_OLDER_BYTES: i64 = 4 * 1024 * 1024;
pub const HIDE_OLDER_MAX: i64 = 19;

fn is_version_no(v: &Value) -> bool {
    v.as_i64().or_else(|| v.as_f64().filter(|f| f.fract() == 0.0).map(|f| f as i64)).is_some_and(|n| (1..=1 << 31).contains(&n))
}
fn to_json(v: &Value) -> Json {
    Json::parse(v.to_string().as_bytes()).unwrap_or(Json::Null)
}
fn limited(r: Res<()>, message: &str) -> Res<()> {
    match r {
        Err(Fail::Pool(mut p)) if p.code == "daily_limit" => {
            p.extra.insert("message".into(), json!(message));
            Err(Fail::Pool(p))
        }
        o => o,
    }
}
fn few(xs: &[usize]) -> Value {
    json!({ "count": xs.len(), "first": xs.iter().take(FLAG_DETAIL_MAX).collect::<Vec<_>>() })
}

/// The body of a PATCH of a bank (after `patch_gate`): the answer-key setting with its decisions, or the licence; one setting a request.
pub fn patch_bank(env: &SendEnv, me: &str, cid: &str, m: &Membership, b: &BankRow, body: &Map<String, Value>) -> Res<Value> {
    for k in body.keys() {
        if k != "license" && k != "hide_key" && k != "base_version" && !DECISION_FIELDS.contains(&k.as_str()) {
            return Err(bad_field(k, &[]));
        }
    }
    let Some(hk) = body.get("hide_key") else { return patch_license(env.x, me, cid, m, b, body) };
    if body.contains_key("license") {
        return Err(bad_field("license", &[("message", json!("Change the licence and the answer-key setting one at a time."))]));
    }
    let Some(on) = hk.as_bool() else { return Err(bad_field("hide_key", &[])) };
    if !on {
        for k in DECISION_FIELDS {
            if body.contains_key(k) {
                return Err(bad_field(k, &[("message", json!("Strip decisions go with hide_key: true."))]));
            }
        }
    }
    let base = match body.get("base_version") {
        None => None,
        Some(v) if is_version_no(v) => v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)),
        Some(_) => return Err(bad_field("base_version", &[])),
    };
    let d = read_strip_decisions(body)?;
    patch_hide_key(env, m, b, me, cid, on, &d, base)
}

#[derive(Debug, Clone)]
struct Head {
    id: String,
    version: i64,
    full_key: String,
    withheld_at: Option<i64>,
    strip_opts: Option<String>,
}

/// Turning hiding on (or on again with other decisions): the head is stripped with the decisions given and recorded, the served form's hashes
/// join the circle's re-share records, older kept versions with no decisions are checked too, and a match withholds the version and holds the bank
/// as a send's does. Off: viewers read the full text again, and what an earlier PATCH withheld alone is released when its full form is clean.
/// One compare-and-set on the head: a version sent meanwhile is `409 conflict`. Only a PATCH that changes something counts toward the day's
/// `circle_hide_changes`; every one that does work counts toward `circle_hide_attempts`.
#[allow(clippy::too_many_arguments)]
pub fn patch_hide_key(env: &SendEnv, m: &Membership, b: &BankRow, me: &str, cid: &str, on: bool, d: &StripDecisions, base: Option<i64>) -> Res<Value> {
    let x = env.x;
    let bid = b.id.as_str();
    require_open(m)?;
    if !(m.role == "owner" || (b.author_id == me && m.role == "editor")) {
        return Err(PoolError::new(403, "circle_role_required").with("needed", json!(OWNER_OR_AUTHOR)).with("role", json!(m.role)).into());
    }
    require_readable(x, m, b, me)?;
    if base.is_some_and(|v| v != b.head_version) {
        return Err(PoolError::new(409, "conflict").with("head_version", json!(b.head_version)).with("message", json!("A new version arrived meanwhile; do it again for the new version.")).into());
    }
    if !on && !b.hide_key {
        return Ok(json!({ "ok": true, "hide_key": false, "unchanged": true }));
    }
    // A decision for a question the head does not have is refused before any work.
    let outside = |field: &str, xs: Vec<usize>| -> Res<()> {
        if xs.iter().any(|q| *q as i64 >= b.question_count) {
            return Err(bad_field(field, &[("message", json!(format!("This bank's current version has {} questions.", b.question_count)))]));
        }
        Ok(())
    };
    outside("labels", d.labels.iter().map(|l| l.question).collect())?;
    outside("order_acks", d.order_acks.iter().map(|l| l.question).collect())?;
    outside("image_ok", d.image_ok.clone())?;
    outside("ack_formatting", d.ack_formatting.clone())?;
    let head: Option<Head> = match (&b.head_version_id, on) {
        (Some(hid), true) => x
            .conn
            .query_row("SELECT id, version, full_key, withheld_at, strip_opts FROM circle_versions WHERE id = ?1 AND status = 'current'", [hid], |r| {
                Ok(Head { id: r.get(0)?, version: r.get(1)?, full_key: r.get(2)?, withheld_at: r.get(3)?, strip_opts: r.get(4)? })
            })
            .optional()?,
        _ => None,
    };
    let head_readable = head.as_ref().is_some_and(|h| h.withheld_at.is_none() && !h.full_key.is_empty());
    if on && b.hide_key && (!head_readable || head.as_ref().is_some_and(|h| h.strip_opts.is_some() && same_decisions(&decisions_of(h.strip_opts.as_deref()), d))) {
        return Ok(json!({ "ok": true, "hide_key": true, "unchanged": true }));
    }
    limited(x.require_daily(me, "circle_hide_attempts", 1), "You have tried to change answer-key settings as many times as you may today. Try again tomorrow.")?;
    if let Err(e) = limited(x.require_daily(me, "circle_hide_changes", 1), "You have changed answer-key settings as many times as you may today. Try again tomorrow.") {
        let _ = x.give_daily(me, "circle_hide_attempts", 1);
        return Err(e);
    }
    let r = if on { hide_on(env, b, me, cid, d, head.as_ref(), head_readable) } else { hide_off(env, me, cid, bid) };
    match r {
        Ok(v) => {
            if v.get("unchanged").and_then(Value::as_bool) == Some(true) {
                let _ = x.give_daily(me, "circle_hide_changes", 1);
            }
            Ok(v)
        }
        Err(e) => {
            let _ = x.give_daily(me, "circle_hide_changes", 1);
            Err(e)
        }
    }
}

/// SQL: no open send flag on version `v` but the copyright matches an answer-key PATCH raised (never a student-data flag: turning hiding off does
/// not change what the full form holds).
fn only_hide_flags(v: &str) -> String {
    format!("NOT EXISTS (SELECT 1 FROM admin_flags xf WHERE xf.publish_id = {v} AND xf.kind IN {SEND_FLAG_KINDS} AND xf.resolved_at IS NULL AND (xf.kind IS NOT 'blocklist' OR (xf.detail ->> '$.via') IS NOT 'hide_key'))")
}

#[derive(Debug, Clone)]
pub struct HideOffCandidate {
    pub id: String,
    pub version: i64,
    pub full_key: String,
    pub bytes: i64,
    pub committer_id: String,
}

/// The withheld versions turning hiding off checks for release: withheld only by an answer-key PATCH's copyright match, kept in full, newest
/// first, within HIDE_OLDER_MAX + 1 and HIDE_OLDER_BYTES. A head is never one while the bank has a student-data hold.
pub fn hide_off_candidates(x: &super::ctx::PoolCtx, bid: &str) -> Res<Vec<HideOffCandidate>> {
    let cands: Vec<HideOffCandidate> = x
        .conn
        .prepare(&format!(
            "SELECT v.id, v.version, v.full_key, v.bytes, v.committer_id FROM circle_versions v INDEXED BY circle_versions_bank
              WHERE v.bank_id = ?1 AND v.status IN ('current', 'previous') AND v.full_key <> '' AND v.withheld_at IS NOT NULL
                AND EXISTS (SELECT 1 FROM admin_flags hf WHERE hf.publish_id = v.id AND hf.kind = 'blocklist' AND hf.resolved_at IS NULL AND hf.detail ->> '$.via' = 'hide_key')
                AND {}
                AND NOT (v.status = 'current' AND EXISTS (SELECT 1 FROM circle_holds ph WHERE ph.bank_id = ?1 AND ph.source IN ('privacy', 'privacy_near')))
              ORDER BY v.version DESC LIMIT ?2",
            only_hide_flags("v.id")
        ))?
        .query_map(params![bid, HIDE_OLDER_MAX + 1], |r| {
            Ok(HideOffCandidate { id: r.get(0)?, version: r.get(1)?, full_key: r.get(2)?, bytes: r.get::<_, Option<i64>>(3)?.unwrap_or(0), committer_id: r.get::<_, Option<String>>(4)?.unwrap_or_default() })
        })?
        .collect::<Result<_, _>>()?;
    let mut budget = HIDE_OLDER_BYTES;
    let mut out = Vec::new();
    for v in cands {
        if v.bytes > budget {
            break;
        }
        budget -= v.bytes;
        out.push(v);
    }
    Ok(out)
}

/// Candidate `v`'s full form checked as hide-off checks it (the version's own sender's exemptions, as at its send): its scan when nothing
/// matches, else None.
fn full_form_clean(env: &SendEnv, v: &HideOffCandidate) -> Res<Option<Scan>> {
    let scanned = with_stored_full(env.core, env.store, &v.full_key, |qs, mh| {
        let js: Vec<Json> = qs.iter().map(to_json).collect();
        privacy_scan(env.x, env.fp, &[ScanForm { img_at: images_at(&js), qs: js, mh: mh.to_vec() }], &v.committer_id)
    })?;
    Ok(match scanned.transpose()? {
        Some(sc) if sc.exact.len() + sc.near.len() + sc.copyright.len() == 0 => Some(sc),
        _ => None,
    })
}

/// Would turning hiding off release version `vid`? It is one of the candidates and its full form is clean now. A check that can not run, or
/// past the reader's `circle_hide_checks` of the day, says false: nothing is promised. The caller checks that the bank hides its keys.
pub fn hide_off_releases(env: &SendEnv, me: &str, bid: &str, vid: &str) -> bool {
    let Ok(cands) = hide_off_candidates(env.x, bid) else { return false };
    let Some(v) = cands.into_iter().find(|c| c.id == vid) else { return false };
    if !env.x.take_daily(me, "circle_hide_checks", 1).unwrap_or(false) {
        return false;
    }
    match full_form_clean(env, &v) {
        Ok(r) => r.is_some(),
        Err(_) => {
            let _ = env.x.give_daily(me, "circle_hide_checks", 1);
            false
        }
    }
}

fn rights_sql() -> String {
    format!("({} OR (circle_banks.author_id = ?3 AND {}))", active_sql("?2", "?3", &["owner"]), active_sql("?2", "?3", &["editor"]))
}
fn won_sql(aid: &str) -> String {
    format!("EXISTS (SELECT 1 FROM pool_audit wa WHERE wa.id = '{aid}')")
}
fn won_now(conn: &rusqlite::Connection, aid: &str) -> rusqlite::Result<bool> {
    conn.query_row("SELECT EXISTS (SELECT 1 FROM pool_audit WHERE id = ?1)", [aid], |r| r.get(0))
}

fn hide_off(env: &SendEnv, me: &str, cid: &str, bid: &str) -> Res<Value> {
    let x = env.x;
    let now = x.now;
    struct Clean {
        id: String,
        keys: String,
        shas: String,
        by: String,
    }
    let mut clean: Vec<Clean> = Vec::new();
    for v in hide_off_candidates(x, bid)? {
        if let Some(sc) = full_form_clean(env, &v)? {
            clean.push(Clean { id: v.id, keys: serde_json::to_string(&sc.keys).unwrap(), shas: serde_json::to_string(&sc.shas).unwrap(), by: v.committer_id });
        }
    }
    let aid = new_id();
    let won = won_sql(&aid);
    let tx = x.conn.unchecked_transaction()?;
    tx.execute(
        &format!(
            "UPDATE circle_banks SET hide_key = 0, updated_at = ?4 WHERE id = ?1 AND circle_id = ?2 AND hide_key = 1 AND removed_at IS NULL AND purged_at IS NULL AND {} AND {} AND {}",
            open_sql("?2"),
            rights_sql(),
            alive_sql("?3")
        ),
        params![bid, cid, me, now],
    )?;
    tx.execute(
        "INSERT INTO pool_audit (id, at, actor_id, actor_kind, action, item_id, target_user_id, detail) SELECT ?1, ?2, ?3, 'user', 'circle.bank_hide_key', ?4, ?3, ?5 WHERE changes() = 1",
        params![aid, now, me, cid, json!({ "bank_id": bid, "hide_key": false }).to_string()],
    )?;
    for v in &clean {
        // Its full form is what everyone reads now: checked again here against the blocks as they are, no gap; its recorded decisions go.
        tx.execute(
            &format!(
                "UPDATE circle_versions SET withheld_at = NULL, strip_opts = NULL WHERE id = ?1 AND withheld_at IS NOT NULL AND {won} AND {}
                   AND NOT (status = 'current' AND EXISTS (SELECT 1 FROM circle_holds ph WHERE ph.bank_id = ?6 AND ph.source IN ('privacy', 'privacy_near')))
                   AND NOT {} AND NOT {}",
                only_hide_flags("?1"),
                privacy_sql("?2", "?3"),
                copyright_sql("?2", "?3", "?4", "?5")
            ),
            params![v.id, v.keys, v.shas, now, v.by, bid],
        )?;
        tx.execute(
            &format!(
                "UPDATE admin_flags SET resolved_at = ?2, resolved_by = ?3, detail = json_set(detail, '$.resolved_via', 'hide_key_off')
                  WHERE publish_id = ?1 AND kind IN {SEND_FLAG_KINDS} AND resolved_at IS NULL AND json_valid(detail) AND {won} AND EXISTS (SELECT 1 FROM circle_versions rv WHERE rv.id = ?1 AND rv.withheld_at IS NULL)"
            ),
            params![v.id, now, me],
        )?;
        // The copyright hold only; a student-data hold is never lifted here.
        tx.execute(
            &format!("DELETE FROM circle_holds WHERE bank_id = ?1 AND source = 'blocklist' AND {won} AND EXISTS (SELECT 1 FROM circle_banks hb JOIN circle_versions hv ON hv.id = hb.head_version_id WHERE hb.id = ?1 AND hv.id = ?2 AND hv.withheld_at IS NULL)"),
            params![bid, v.id],
        )?;
    }
    if !clean.is_empty() {
        tx.execute(&listing_refresh_sql(&won), [bid])?;
    }
    let won_it = won_now(&tx, &aid)?;
    let restored: Vec<i64> = if won_it {
        let ids = serde_json::to_string(&clean.iter().map(|v| &v.id).collect::<Vec<_>>()).unwrap();
        let mut r: Vec<i64> = tx
            .prepare("SELECT rv.version FROM circle_versions rv WHERE rv.id IN (SELECT value FROM json_each(?1)) AND rv.withheld_at IS NULL")?
            .query_map([ids], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        r.sort();
        r
    } else {
        vec![]
    };
    tx.commit()?;
    if !won_it {
        x.throw_if_erased(me)?;
        let still: Option<bool> = x.conn.query_row("SELECT hide_key FROM circle_banks WHERE id = ?1", [bid], |r| r.get(0)).optional()?;
        if still == Some(true) {
            return Err(explain(x, cid, me, None));
        }
        return Ok(json!({ "ok": true, "hide_key": false, "unchanged": true }));
    }
    let mut out = json!({ "ok": true, "hide_key": false });
    if !restored.is_empty() {
        out["restored"] = json!(restored);
    }
    Ok(out)
}

struct Older {
    id: String,
    version: i64,
    scan: Scan,
    json: String,
}

fn hide_on(env: &SendEnv, b: &BankRow, me: &str, cid: &str, d: &StripDecisions, head: Option<&Head>, head_readable: bool) -> Res<Value> {
    let x = env.x;
    let bid = b.id.as_str();
    let now = x.now;
    let scan_of = |st: &super::core::Stripped| -> Res<Scan> {
        let vq: Vec<Json> = questions_of(&st.v.payload).into_iter().map(to_json).collect();
        Ok(privacy_scan(x, env.fp, &[ScanForm { img_at: images_at(&vq), mh: st.match_hashes.clone(), qs: vq }], me)?)
    };
    let mut strip_opts: Option<String> = None;
    let mut pairs: Vec<(String, String)> = Vec::new();
    let mut report = Map::new();
    let mut head_scan: Option<Scan> = None;
    if let (true, Some(h)) = (head_readable, head) {
        match strip_stored(env.core, env.store, &h.full_key, d, true)? {
            StoredStrip::Ok { match_hashes, full_hashes, image_shas, recorded, report: rep, stripped, .. } => {
                head_scan = Some(scan_of(&stripped)?);
                strip_opts = Some(decisions_json(&recorded, &image_shas));
                pairs = hash_pairs(&full_hashes.unwrap_or_default(), &match_hashes);
                if let Value::Object(o) = rep {
                    report = o;
                }
            }
            StoredStrip::Refused { missing: true, .. } => return Err(not_found().into()),
            StoredStrip::Refused { fail, .. } => return Err(strip_refusal(&fail).into()),
        }
    }
    // Older kept versions with no decisions recorded (sent while the keys were shown), newest first, within the bound.
    let older_rows: Vec<(String, i64, String, i64)> = x
        .conn
        .prepare(
            "SELECT id, version, full_key, bytes FROM circle_versions INDEXED BY circle_versions_bank
              WHERE bank_id = ?1 AND status IN ('current', 'previous') AND full_key <> '' AND withheld_at IS NULL AND strip_opts IS NULL AND id IS NOT ?2 ORDER BY version DESC LIMIT ?3",
        )?
        .query_map(params![bid, head.map(|h| h.id.as_str()), HIDE_OLDER_MAX], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get::<_, Option<i64>>(3)?.unwrap_or(0))))?
        .collect::<Result<_, _>>()?;
    let mut to_record: Vec<Older> = Vec::new();
    let mut to_withhold: Vec<Older> = Vec::new();
    let mut budget = HIDE_OLDER_BYTES;
    for (id, version, full_key, bytes) in older_rows {
        if bytes > budget {
            break;
        }
        budget -= bytes;
        if let StoredStrip::Ok { image_shas, recorded, stripped, .. } = strip_stored(env.core, env.store, &full_key, &StripDecisions::default(), false)? {
            let scan = scan_of(&stripped)?;
            let o = Older { id, version, json: decisions_json(&recorded, &image_shas), scan };
            if o.scan.exact.len() + o.scan.near.len() + o.scan.copyright.len() > 0 {
                to_withhold.push(o);
            } else {
                to_record.push(o);
            }
        }
    }
    let records = x.limit_of("*", "circle_records")?;
    let fresh: i64 = if pairs.is_empty() {
        0
    } else {
        let seconds: Vec<&String> = pairs.iter().map(|p| &p.1).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        x.conn.query_row(
            "SELECT COUNT(*) FROM json_each(?1) j WHERE NOT EXISTS (SELECT 1 FROM circle_qkeys k WHERE k.circle_id = ?2 AND k.match_hash = j.value AND k.bank_id = ?3)",
            params![serde_json::to_string(&seconds).unwrap(), cid, bid],
            |r| r.get(0),
        )?
    };
    let day0 = day_of(now) * DAY_MS;
    let flag_cap = x.limit_of(me, "circle_send_flags")?;
    let circle_bytes = x.limit_of("*", "circle_bytes")?;
    let room = flag_room_sql("?4", "?7", "?8", "?9");
    let aid = new_id();
    let won = won_sql(&aid);
    let head_strip = head.and_then(|h| h.strip_opts.as_deref());
    let tx = x.conn.unchecked_transaction()?;
    tx.execute(
        &format!(
            "UPDATE circle_banks SET hide_key = 1, updated_at = ?4 WHERE id = ?1 AND circle_id = ?2 AND head_version_id IS ?5 AND removed_at IS NULL AND purged_at IS NULL
               AND hide_key = ?8 AND (SELECT sv.strip_opts FROM circle_versions sv WHERE sv.id = ?5) IS ?9
               AND {} AND {} AND {} AND (SELECT qkeys FROM circles WHERE id = ?2) + ?6 <= ?7",
            open_sql("?2"),
            rights_sql(),
            alive_sql("?3")
        ),
        params![bid, cid, me, now, b.head_version_id, fresh, records, b.hide_key as i64, head_strip],
    )?;
    // An activity line when it is turned on; recording new decisions for a bank that already hides its keys is not one.
    tx.execute(
        "INSERT INTO pool_audit (id, at, actor_id, actor_kind, action, item_id, target_user_id, detail) SELECT ?1, ?2, ?3, 'user', ?4, ?5, ?3, ?6 WHERE changes() = 1",
        params![aid, now, me, if b.hide_key { "circle.bank_strip_decisions" } else { "circle.bank_hide_key" }, cid, json!({ "bank_id": bid, "hide_key": true }).to_string()],
    )?;
    tx.execute(&format!("UPDATE circle_versions SET strip_opts = ?2 WHERE id = ?1 AND ?2 IS NOT NULL AND {won}"), params![head.map(|h| h.id.as_str()).unwrap_or(""), strip_opts])?;
    let pair_arr: Vec<[&String; 2]> = pairs.iter().map(|(a, b)| [a, b]).collect();
    tx.execute(&stripped_qkeys_sql("?1", "?2", "?3", &won), params![cid, bid, serde_json::to_string(&pair_arr).unwrap()])?;
    tx.execute(&format!("UPDATE circles SET qkeys = qkeys + changes() WHERE id = ?1 AND {won}"), [cid])?;
    let mut head_withheld_changes = 0usize;
    if let (Some(hs), Some(h)) = (&head_scan, head) {
        // The viewers' form of the head, as a send's commit checks it (the exact keys again here: no gap).
        let kj = serde_json::to_string(&hs.keys).unwrap();
        let sj = serde_json::to_string(&hs.shas).unwrap();
        tx.execute(&format!("INSERT OR IGNORE INTO circle_holds (bank_id, reason, source, at, by) SELECT ?1, 'student_data', 'privacy', ?4, NULL WHERE {won} AND {}", privacy_sql("?2", "?3")), params![bid, kj, sj, now])?;
        tx.execute(&format!("INSERT OR IGNORE INTO circle_holds (bank_id, reason, source, at, by) SELECT ?1, 'student_data', 'privacy_near', ?3, NULL WHERE ?2 > 0 AND {won}"), params![bid, hs.near.len() as i64, now])?;
        tx.execute(
            &format!("INSERT OR IGNORE INTO circle_holds (bank_id, reason, source, at, by) SELECT ?1, 'copyright', 'blocklist', ?4, NULL WHERE {won} AND {}", copyright_sql("?2", "?3", "?4", "?5")),
            params![bid, kj, sj, now, me],
        )?;
        head_withheld_changes = tx.execute(
            &format!("UPDATE circle_versions SET withheld_at = ?2 WHERE id = ?1 AND withheld_at IS NULL AND {won} AND EXISTS (SELECT 1 FROM circle_holds wh WHERE wh.bank_id = ?3 AND wh.source IN {SEND_HOLD_SOURCES})"),
            params![h.id, now, bid],
        )?;
        tx.execute(
            &format!(
                "INSERT INTO admin_flags (id, kind, item_id, publish_id, user_id, detail, at)
                 SELECT ?1, 'privacy_text_match', ?2, ?3, ?4, ?5, ?6 WHERE {won} AND EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = ?2 AND h.source IN ('privacy', 'privacy_near')) AND {room}"
            ),
            params![new_id(), bid, h.id, me, json!({ "circle_id": cid, "version": h.version, "positions": few(&hs.exact), "near": few(&hs.near), "hashes": hs.hits, "via": "hide_key" }).to_string(), now, cid, day0, flag_cap],
        )?;
        tx.execute(
            &format!(
                "INSERT INTO admin_flags (id, kind, item_id, publish_id, user_id, detail, at)
                 SELECT ?1, 'blocklist', ?2, ?3, ?4, ?5, ?6 WHERE {won} AND EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = ?2 AND h.source = 'blocklist') AND {room}"
            ),
            params![new_id(), bid, h.id, me, json!({ "circle_id": cid, "version": h.version, "positions": few(&hs.copyright), "actions": hs.actions, "hashes": hs.chits, "via": "hide_key" }).to_string(), now, cid, day0, flag_cap],
        )?;
        // Withheld past the day's flags, named on the bank's cap flag.
        tx.execute(
            &cap_flag_sql(&won),
            params![new_id(), bid, me, cid, h.version, h.id, serde_json::to_string(&hs.actions).unwrap(), if hs.exact.len() + hs.near.len() > 0 { "true" } else { "false" }, cap_dedup(bid, me, now), now],
        )?;
        tx.execute(&listing_refresh_sql(&won), [bid])?;
    }
    for v in &to_record {
        tx.execute(&format!("UPDATE circle_versions SET strip_opts = ?2 WHERE id = ?1 AND strip_opts IS NULL AND withheld_at IS NULL AND {won}"), params![v.id, v.json])?;
    }
    for v in &to_withhold {
        // An older version, not the head: withheld, flagged, no bank hold; checked as of this PATCH.
        tx.execute(&format!("UPDATE circle_versions SET withheld_at = ?2 WHERE id = ?1 AND withheld_at IS NULL AND {won}"), params![v.id, now])?;
        if v.scan.exact.len() + v.scan.near.len() > 0 {
            tx.execute(
                &format!("INSERT INTO admin_flags (id, kind, item_id, publish_id, user_id, detail, at) SELECT ?1, 'privacy_text_match', ?2, ?3, ?4, ?5, ?6 WHERE {won} AND {room}"),
                params![new_id(), bid, v.id, me, json!({ "circle_id": cid, "version": v.version, "positions": few(&v.scan.exact), "near": few(&v.scan.near), "hashes": v.scan.hits, "via": "hide_key" }).to_string(), now, cid, day0, flag_cap],
            )?;
        }
        if !v.scan.copyright.is_empty() {
            tx.execute(
                &format!("INSERT INTO admin_flags (id, kind, item_id, publish_id, user_id, detail, at) SELECT ?1, 'blocklist', ?2, ?3, ?4, ?5, ?6 WHERE {won} AND {room}"),
                params![new_id(), bid, v.id, me, json!({ "circle_id": cid, "version": v.version, "positions": few(&v.scan.copyright), "actions": v.scan.actions, "hashes": v.scan.chits, "via": "hide_key" }).to_string(), now, cid, day0, flag_cap],
            )?;
        }
        tx.execute(
            &cap_flag_sql(&won),
            params![new_id(), bid, me, cid, v.version, v.id, serde_json::to_string(&v.scan.actions).unwrap(), if v.scan.exact.len() + v.scan.near.len() > 0 { "true" } else { "false" }, cap_dedup(bid, me, now), now],
        )?;
    }
    // The evidence kept in full stays within its bound.
    if (head_scan.is_some() && head.is_some()) || !to_withhold.is_empty() {
        evidence_bound(&tx, cid, now, circle_bytes, &won)?;
    }
    let won_it = won_now(&tx, &aid)?;
    let held: bool = tx.query_row("SELECT EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = ?1)", [bid], |r| r.get(0))?;
    let head_withheld: bool = tx.query_row("SELECT COALESCE((SELECT withheld_at IS NOT NULL FROM circle_versions WHERE id = ?1), 0)", [head.map(|h| h.id.as_str()).unwrap_or("")], |r| r.get(0))?;
    let flagged: Vec<(String, i64)> = if won_it {
        tx.prepare("SELECT f.kind, CAST(f.detail ->> '$.version' AS INTEGER) FROM admin_flags f WHERE f.item_id = ?1 AND f.at = ?2 AND f.detail ->> '$.via' = 'hide_key'")?
            .query_map(params![bid, now], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?
    } else {
        vec![]
    };
    let cap_new: bool = won_it && tx.query_row("SELECT COALESCE((SELECT cf.at = ?2 FROM admin_flags cf WHERE cf.dedup = ?1), 0)", params![cap_dedup(bid, me, now), now], |r| r.get(0))?;
    tx.commit()?;
    if !won_it {
        x.throw_if_erased(me)?;
        type S = (i64, Option<String>, Option<i64>, Option<String>, bool, i64, Option<String>, Option<bool>);
        let s: Option<S> = x
            .conn
            .query_row(
                "SELECT b.head_version, b.head_version_id, b.removed_at, b.removed_reason, b.hide_key, ci.qkeys, v.strip_opts, (v.withheld_at IS NULL AND v.full_key <> '')
                   FROM circle_banks b JOIN circles ci ON ci.id = b.circle_id LEFT JOIN circle_versions v ON v.id = b.head_version_id WHERE b.id = ?1",
                [bid],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)),
            )
            .optional()?;
        if let Some((head_version, head_id, removed_at, removed_reason, hide_key, qkeys, s_strip, readable)) = s {
            if removed_at.is_some() {
                return Err(PoolError::new(409, "bank_removed").with("reason", json!(removed_reason)).into());
            }
            if head_id != b.head_version_id {
                return Err(PoolError::new(409, "conflict").with("head_version", json!(head_version)).with("message", json!("A new version arrived meanwhile; do it again for the new version.")).into());
            }
            // Another PATCH changed the setting or the head's decisions after this one read them. When the bank now hides its keys as this PATCH
            // asked, it is the same setting again: `unchanged`, with nothing written. Else a conflict.
            if hide_key != b.hide_key || s_strip.as_deref() != head_strip {
                let same = match (&strip_opts, &s_strip) {
                    (Some(mine), Some(theirs)) => same_decisions(&decisions_of(Some(theirs)), &decisions_of(Some(mine))),
                    _ => false,
                };
                if hide_key && (readable != Some(true) || same) {
                    return Ok(json!({ "ok": true, "hide_key": true, "unchanged": true }));
                }
                return Err(explain(x, cid, me, None));
            }
            if qkeys + fresh > records {
                return Err(PoolError::new(413, "circle_records_full")
                    .with("used", json!(qkeys))
                    .with("needed", json!(fresh))
                    .with("limit", json!(records))
                    .with("message", json!("This circle has recorded as many different questions as it can keep."))
                    .into());
            }
        }
        return Err(explain(x, cid, me, None));
    }
    // The admin is told of each version withheld here (ids only, their own hourly budgets), and copyright matches count toward the account's
    // held-publish budget, as for a send.
    for (kind, version) in &flagged {
        if kind == "privacy_text_match" {
            x.alert_budgeted(PoolNotice::PrivacyTextMatch { item_id: bid.into(), held: true, circle_id: Some(cid.into()), version: Some(*version) })?;
        } else {
            x.alert_budgeted(PoolNotice::CircleCopyrightMatch { item_id: bid.into(), circle_id: cid.into(), version: *version })?;
        }
    }
    // `withheld` and `held` say what THIS PATCH did: it withheld the head (only a head it scanned, one members could read), and the bank's hold
    // came with it. A head withheld before is not reported: a GET says whether turning hiding off would release it.
    let withheld_now = head_scan.is_some() && head_withheld && head_withheld_changes == 1;
    let c_positions = (if withheld_now { head_scan.as_ref().map(|s| s.copyright.len()).unwrap_or(0) } else { 0 }) + to_withhold.iter().map(|v| v.scan.copyright.len()).sum::<usize>();
    if c_positions > 0 {
        charge_held_publishes(x, me, c_positions as i64)?;
    }
    if cap_new {
        x.alert_budgeted(PoolNotice::Cap { user_id: me.into(), metric: "circle_send_flags".into() })?;
    }
    let mut out = Map::new();
    out.insert("ok".into(), json!(true));
    out.insert("hide_key".into(), json!(true));
    out.insert("held".into(), json!(withheld_now && held));
    out.insert("withheld".into(), json!(withheld_now));
    out.extend(report);
    Ok(Value::Object(out))
}

#[cfg(test)]
#[path = "circlehidepatch_tests.rs"]
mod tests;
