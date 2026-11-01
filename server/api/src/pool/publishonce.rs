//! The publish itself (worker/src/pool/publish.ts `publish` and `publishOnce`): every step in the Worker's order, HTTP-free. The route reads the body (length
//! declared and exact, within the deadline) and holds the isolate's heavy slot around `publish`, which `enter_heavy` takes once the cheap checks have passed.
//!
//!   admit -> account lock -> parse -> before_heavy -> [heavy slot] -> read the meta -> idempotency / the item -> rewrite -> hash -> facts -> units -> evidence ->
//!   derive -> earlier-identical credit -> licence refusals -> origin_suspect -> soft check / blocklist / duplicates -> tag, record and quota limits ->
//!   (dry run: the preview) -> daily allowance -> sequences -> stage -> flags -> commit -> after commit.
use super::circlecheck::read_circle_claim;
use super::circlesendmeta::PRIVACY_BAND_ROWS_MAX;
use super::core::Core;
use super::ctx::{Fail, PoolCtx, PoolError, Res};
use super::fingerprint::Fingerprinter;
use super::poolio::{is_publish_type, parse_publish};
use super::privacynear::{StemSketch, privacy_near_positions};
use super::publishafter::{after_commit, dry_response};
use super::publishchecks;
use super::publishcommit::{CommitIn, commit};
use super::publishderive::derive;
use super::publishevid::collect;
use super::publishfacts::gather;
use super::publishflags::{FlagIn, build as build_flags};
use super::publishhash::prepare;
use super::publishinsert::{StageIn, plan, stage};
use super::publishmeta::read_meta;
use super::publishrefuse::{earlier_identical, refuse, suspects};
use super::publishrewrite::rewrite;
use super::publishrun::{refresh_account_lock, release_account_lock};
use super::publishstage::{check_quotas, check_records, check_tags, limits, record_keys, sequences};
use super::publishstart::{Started, admit, before_heavy, identify, settle, take_account_lock};
use super::publishunits::{UNIT_KEYS_MAX, lookup};
use super::sqlfrag::MB;
use super::blobs::sha256_hex;
use crate::blobs::BlobStore;
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};

pub const MAX_PAYLOAD: usize = 4 * MB as usize;
pub const MAX_META: usize = MB as usize;

pub struct Env<'a> {
    pub store: &'a BlobStore,
    pub core: &'a Core,
    pub fp: &'a Fingerprinter,
}

/// The response: 201 for a new item, 200 for a republish, a replay or a dry run.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub status: u16,
    pub body: Value,
}

fn dedup(it: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    it.into_iter().filter(|s| seen.insert(s.clone())).collect()
}

/// POST /items (`path_item_id` None) and POST /items/:id/versions, or a preview with `dry`. `enter_heavy` takes the isolate's heavy slot (and whatever byte
/// reservation goes with it) once the cheap checks have passed; the caller keeps it until this returns.
pub fn publish(x: &PoolCtx, env: &Env, me: &str, path_item_id: Option<&str>, content_type: Option<&str>, body: &[u8], dry: bool, enter_heavy: &mut dyn FnMut() -> Res<()>) -> Res<Outcome> {
    admit(x, me, dry)?;
    let lock = take_account_lock(x, me, dry)?;
    let out = (|| {
        if !is_publish_type(content_type) {
            return Err(Fail::from(PoolError::new(415, "unsupported_media_type")));
        }
        let (meta, start) = parse_publish(body, MAX_PAYLOAD, MAX_META)?;
        refresh_account_lock(x, me, &lock)?;
        publish_once(x, env, me, path_item_id, body, &meta, start, dry, &lock, enter_heavy)
    })();
    settle(x, me, dry, match &out {
        Err(Fail::Pool(e)) => Some(e),
        _ => None,
    })?;
    release_account_lock(x, me, &lock)?;
    out
}

#[allow(clippy::too_many_arguments)]
fn publish_once(x: &PoolCtx, env: &Env, me: &str, path_item_id: Option<&str>, raw: &[u8], raw_meta: &Map<String, Value>, start: usize, dry: bool, lock: &str, enter_heavy: &mut dyn FnMut() -> Res<()>) -> Res<Outcome> {
    let (store, core, fp) = (env.store, env.core, env.fp);
    let is_new = path_item_id.is_none();
    let over_cap = before_heavy(x, me, path_item_id)?;
    enter_heavy()?;
    let payload = &raw[start..];
    let m = read_meta(raw_meta, is_new)?;
    let claim = read_circle_claim(raw_meta)?;
    let request_sha = sha256_hex(raw);
    let land = match identify(x, store, me, path_item_id, &m, &request_sha, dry)? {
        Started::Replay(v) => return Ok(Outcome { status: 200, body: v }),
        Started::Go(l) => *l,
    };
    let rw = rewrite(x, store, core, me, &land, is_new, &m, payload, dry)?;
    let h = prepare(x, store, core, fp, &land, is_new, &m, &rw)?;
    let f = gather(x, store, fp, me, land.base_of.as_deref(), is_new, dry, &m, &rw, &h, claim.as_ref())?;
    refresh_account_lock(x, me, lock)?;
    let units = lookup(x, me, dry, &f.actx, &f, UNIT_KEYS_MAX)?;
    let e = collect(x, fp, me, &m, &rw, &h, &f, &units)?;
    let mut d = derive(&m, &rw, &h, &f, &e, &units, land.prev_license.as_deref(), land.prev_hidden);
    earlier_identical(x, me, &m, &rw, &h, &f, &e, &mut d)?;
    refuse(x, me, &m, &d)?;
    let suspect = suspects(x, me, &h, &f, &e, &mut d)?;
    let checks = publishchecks::run(x, store, me, &land.item_id, is_new, &m.license, &rw, &h, &f)?;
    refresh_account_lock(x, me, lock)?;

    // ---- stage ----
    let (pl, charge) = plan(&m, &rw, &h, is_new);
    check_tags(pl.tags.len())?;
    let l = limits(x, me)?;
    check_records(x, me, &land.item_id, is_new, &record_keys(&m, &h, &units))?;
    if dry {
        check_quotas(x, me, &land.item_id, charge, rw.n, &l)?;
        return Ok(Outcome { status: 200, body: dry_response(&m, &h, &d, rw.n) });
    }
    // Charged only now: a payload refused by any check above does not use up the day's publishes.
    x.require_daily(me, "publishes", 1)?;
    let seqs = sequences(x, &rw, &h, &f, &units, land.base_of.as_deref())?;
    let served_shas = dedup(f.img_at.iter().flatten().cloned());
    let staged = stage(
        x,
        store,
        &StageIn {
            me,
            item_id: &land.item_id,
            version: land.version,
            request_sha: &request_sha,
            m: &m,
            rw: &rw,
            h: &h,
            f: &f,
            d: &d,
            s: &seqs,
            plan: &pl,
            charge,
            limits: l,
            search_words: &h.search_words,
            served_shas: &served_shas,
        },
    )?;
    refresh_account_lock(x, me, lock)?;

    // ---- the result readers get ----
    let st = h.prep.stripped.as_ref();
    let result = json!({
        "item_id": land.item_id,
        "publish_id": m.publish_id,
        "version": land.version,
        "kind": rw.kind,
        "visibility": land.visibility,
        "license": m.license,
        "hide_key": m.hide_key,
        "qids": rw.qids,
        "public_version": h.public_version,
        "possible_duplicate_of": checks.possible_duplicate,
        // The author's review of what non-authors will be served (positions and orders, no text).
        "served_orders": match (&h.reused, st) { (Some(r), _) => r.served_orders.clone(), (None, Some(s)) => s.served_orders.clone(), _ => vec![] },
        "order_reasons": st.map(|s| s.order_reasons.clone()).unwrap_or_default(),
        "labels_removed": st.map(|s| json!(s.labels_removed)).unwrap_or(json!([])),
        "images_kept": st.map(|s| json!(s.images_kept)).unwrap_or(json!([])),
        "images_removed": st.map(|s| json!(s.images_removed)).unwrap_or(json!([])),
        "figures_removed": st.map(|s| json!(s.figures_removed)).unwrap_or(json!([])),
        "dropped": h.prep.full.dropped,
    });
    let result_json = result.to_string();

    // ---- the licence history keys: the CANONICAL served content of each question and each non-hidden unit ----
    let ckeys = dedup(
        (0..rw.n)
            .map(|i| {
                let near = if m.hide_key { h.near_stripped.as_ref().and_then(|v| v.get(i).cloned().flatten()).unwrap_or_else(|| h.near_full[i].clone()) } else { h.near_full[i].clone() };
                format!("q:{near}")
            })
            .chain(units.units_at.iter().flat_map(|us| us.iter().filter(|u| !u.hidden).map(|u| format!("u:{}", u.key)))),
    );

    // ---- near-rewordings of confirmed student-data text, and the flags ----
    let mut stems: Vec<Vec<StemSketch>> = vec![];
    let mut keys_at: Vec<Vec<Option<String>>> = vec![];
    for i in 0..rw.n {
        let mut ss = vec![StemSketch { tokens: h.tokens[i].clone(), bands: h.bands[i].clone() }];
        ss.extend(f.part_stems[i].iter().map(|p| StemSketch { tokens: p.tokens.clone(), bands: p.bands.clone() }));
        stems.push(ss);
        let mut ks = vec![Some(h.match_hashes[i].clone()), Some(f.bk_norm[i].clone()), Some(f.bk_stem[i].clone()), Some(f.bk_full[i].clone()), f.bk_fields[i].0.clone(), f.bk_fields[i].1.clone()];
        ks.extend(f.part_keys[i].iter().map(|(_, k)| Some(k.clone())));
        keys_at.push(ks);
    }
    let block_keys = f.block_keys.clone();
    let exact_of = || -> rusqlite::Result<HashSet<String>> {
        let mut st = x.conn.prepare("SELECT hash FROM privacy_blocks WHERE kind = 'match' AND hash IN (SELECT value FROM json_each(?1))")?;
        let rows = st.query_map([json!(block_keys).to_string()], |r| r.get::<_, String>(0))?;
        rows.collect()
    };
    let privacy_near = privacy_near_positions(x.conn, fp, &stems, &keys_at, exact_of, &mut HashMap::new(), PRIVACY_BAND_ROWS_MAX)?;
    // (A cut of the author's own near-rewordings is flagged too.)
    let mut own_cut: Vec<usize> = e.own_cut.iter().copied().collect();
    own_cut.sort_unstable();
    for k in own_cut {
        if !d.overflowed.contains(&k) {
            d.overflowed.push(k);
        }
    }
    let circle_statement = (!f.circle_claimed.is_empty()).then(|| claim.as_ref().map(|c| c.statement.as_str())).flatten();
    let (flags, folded) = build_flags(
        x,
        &FlagIn {
            me,
            m: &m,
            d: &d,
            soft: &checks.soft,
            blocked: checks.blocked.as_ref(),
            suspect: &suspect,
            privacy_near: &privacy_near,
            overflowed: &d.overflowed,
            circle_claimed: &f.circle_claimed,
            circle_statement,
        },
    )?;

    commit(
        x,
        store,
        &CommitIn {
            me,
            item_id: &land.item_id,
            is_new,
            kind: rw.kind,
            visibility: &land.visibility,
            version: land.version,
            m: &m,
            plan: &pl,
            subject: &rw.meta.subject,
            course: &rw.meta.course,
            grade: &rw.meta.grade,
            n: rw.n,
            public_version: h.public_version,
            public_at: h.public_at,
            reused: h.reused.is_some(),
            token: &staged.token,
            base_of: land.base_of.as_deref(),
            result_json: &result_json,
            ckeys: &ckeys,
            unit_record: &seqs.unit_record,
            stem_record: &f.stem_record,
            carried: checks.carried.as_ref(),
            flags: &flags,
            folded: &folded,
            circle_claimed: &f.circle_claimed,
            circle_statement,
            privacy_near: privacy_near.len(),
            full_key: &staged.full_key,
            stripped_key: staged.stripped_key.as_deref(),
        },
    )?;
    after_commit(x, store, me, &land.item_id, &m.publish_id, privacy_near.len(), over_cap)?;
    Ok(Outcome { status: if is_new { 201 } else { 200 }, body: result })
}

#[cfg(test)]
#[path = "publishonce_tests.rs"]
mod tests;
