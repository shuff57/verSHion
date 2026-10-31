//! GET/HEAD /items/:id and POST /items/:id/import (worker/src/pool/read.ts `getItem`, `importItem`), HTTP-free: what the
//! route has to do before and after is the body read (the optional JSON of an import) and the writing of the reply.
//!
//! A reply is `{...head, "payload": <the stored canonical bytes, unchanged>}`: the head is built here, the payload is the
//! stored body spliced in unchanged (`payload_prefix`, the bytes, `PAYLOAD_TAIL`). The Worker's isolate byte reservation
//! (`reserveBytes`) and the R2 stream's idle / lifetime timers have no counterpart here: a stored body is read from disk
//! whole (bounded by the publish size limits) and the open-read slots (`readslots.rs`) are what bounds concurrency.
//! Key order inside the head follows serde_json (sorted), not the Worker's insertion order, so ETags differ once across
//! the cutover; clients re-fetch one time.
use super::ctx::{PoolCtx, Res, not_found};
use super::fingerprint::assert_not_migrating;
use super::itemview::{Form, ItemRow, item_view, load_visible, questions_for, served_form};
use super::license::LICENSES;
use super::readslots::{OpenReads, ReadSlots, etag_of};
use super::recordserved::{ServeKind, hold_imported_images, record_served, tags_for};
use super::republish::may_republish_as;
use super::sqlfrag::is_id;
use crate::blobs::BlobStore;
use rusqlite::params;
use serde_json::{Value, json};

pub const PAYLOAD_TAIL: &[u8] = b"}";

/// `{...head` up to and including `"payload":` (the head is a JSON object string).
pub fn payload_prefix(head: &str, key: &str) -> Vec<u8> {
    let inner = &head[..head.len() - 1];
    format!("{inner}{}\"{key}\":", if head.len() > 2 { "," } else { "" }).into_bytes()
}

/// A reply with the stored body spliced in; holds the read slots until it is dropped (sent or abandoned).
#[derive(Debug)]
pub struct Streamed {
    pub head: String,
    pub etag: Option<String>,
    pub payload: Vec<u8>,
    pub slots: ReadSlots,
}
impl Streamed {
    pub fn bytes(&self) -> Vec<u8> {
        let mut out = payload_prefix(&self.head, "payload");
        out.extend_from_slice(&self.payload);
        out.extend_from_slice(PAYLOAD_TAIL);
        out
    }
}

#[derive(Debug)]
pub enum ReadOut {
    /// HEAD: the headers a GET would send (ETag and Content-Length), no body, nothing recorded.
    Head { etag: String, content_length: u64 },
    /// A matching If-None-Match: 304, nothing recorded.
    NotModified { etag: String },
    Body(Streamed),
}

fn miss(x: &PoolCtx, me: &str) -> Res<std::convert::Infallible> {
    x.count_miss(me, "item_miss")?;
    Err(not_found().into())
}

fn item_id_or_miss(x: &PoolCtx, me: &str, id: &str) -> Res<()> {
    x.miss_guard(me, "item_miss")?;
    if !is_id(id) {
        match miss(x, me)? {}
    }
    Ok(())
}

/// If-None-Match uses the weak comparison (RFC 9110 13.1.2): W/"x" matches "x".
pub fn if_none_match_hits(header: &str, etag: &str) -> bool {
    header.split(',').any(|t| {
        let t = t.trim();
        t.strip_prefix("W/").unwrap_or(t) == etag || t == "*"
    })
}

fn body_of(store: &BlobStore, key: Option<&str>) -> Option<Vec<u8>> {
    key.filter(|k| !k.is_empty()).and_then(|k| store.get(k).ok().flatten())
}

pub struct Reader<'a> {
    pub x: &'a PoolCtx<'a>,
    pub store: &'a BlobStore,
    pub opens: &'a OpenReads,
    pub me: &'a str,
    /// The address bucket of the client (the route computes it from the proxy header).
    pub addr: &'a str,
}

fn key_of(r: &ItemRow, form: Form) -> Option<String> {
    if form == Form::Stripped { r.stripped_key.clone() } else { Some(r.full_key.clone()) }
}

impl Reader<'_> {
    pub fn get_item(&self, id: &str, head_only: bool, if_none_match: Option<&str>) -> Res<ReadOut> {
        let (x, me) = (self.x, self.me);
        item_id_or_miss(x, me, id)?;
        x.require_daily(me, "item_reads", 1)?;
        let slots = self.opens.hold(me, self.addr)?;
        let Some(r) = load_visible(x.conn, id, me)? else { match miss(x, me)? {} };
        let form = served_form(&r.author_id, r.p_hide, me);
        let key = key_of(&r, form);
        let (questions, license_check) = questions_for(x.conn, &r.current_publish_id, form, &r.p_license)?;
        let mut item = item_view(&r, me, form);
        let (tags, standards) = tags_for(x, &r.current_publish_id)?;
        item["tags"] = json!(tags);
        item["standards"] = json!(standards);
        let head = json!({ "item": item, "questions": questions, "license_check": license_check }).to_string();
        let etag = etag_of(&head);
        // HEAD and a matching If-None-Match serve no body: nothing is recorded (the caller was served it before, or not at all).
        if head_only || if_none_match.is_some_and(|h| if_none_match_hits(h, &etag)) {
            let Some(size) = key.as_deref().filter(|k| !k.is_empty()).and_then(|k| self.store.len(k).ok().flatten()) else { match miss(x, me)? {} };
            drop(slots);
            return Ok(if head_only {
                ReadOut::Head { content_length: payload_prefix(&head, "payload").len() as u64 + size + PAYLOAD_TAIL.len() as u64, etag }
            } else {
                ReadOut::NotModified { etag }
            });
        }
        // A non-author's full read records evidence: none while the backfill runs.
        if r.author_id != me {
            assert_not_migrating(x)?;
        }
        let Some(payload) = body_of(self.store, key.as_deref()) else { match miss(x, me)? {} };
        // A read is licence evidence too: a verbatim copy of something only read is still a copy, so the hard check must
        // see it. Recorded as a 'view', which never counts as an import. A takedown or hold that landed meanwhile turns
        // the read into a 404: no body is served.
        if r.author_id != me && !record_served(x, me, &r, form, ServeKind::View)? {
            match miss(x, me)? {}
        }
        Ok(ReadOut::Body(Streamed { head, etag: Some(etag), payload, slots }))
    }

    pub fn import_item(&self, id: &str) -> Res<ReadOut> {
        let (x, me) = (self.x, self.me);
        assert_not_migrating(x)?;
        item_id_or_miss(x, me, id)?;
        x.require_daily(me, "imports", 1)?;
        // An import that fails gives its import allowance back; only successes count.
        let refund = || {
            let _ = x.give_daily(me, "imports", 1);
        };
        if let Err(e) = x.require_daily(me, "item_reads", 1) {
            refund();
            return Err(e);
        }
        let out = self.opens.hold(me, self.addr).map_err(Into::into).and_then(|slots| self.import_inner(id, slots));
        if out.is_err() {
            refund();
        }
        out
    }

    fn import_inner(&self, id: &str, slots: ReadSlots) -> Res<ReadOut> {
        let (x, me) = (self.x, self.me);
        let Some(r) = load_visible(x.conn, id, me)? else { match miss(x, me)? {} };
        let form = served_form(&r.author_id, r.p_hide, me);
        let is_author = r.author_id == me;
        // Circle-only material is copied only inside its circles; here only its author.
        if !is_author && r.p_license == "circle-only" {
            match miss(x, me)? {}
        }
        let before: Vec<i64> = {
            let mut st = x.conn.prepare("SELECT version FROM imports WHERE user_id = ?1 AND item_id = ?2 AND kind = 'import' ORDER BY version")?;
            st.query_map(params![me, id], |q| q.get(0))?.collect::<Result<_, _>>()?
        };
        let Some(payload) = body_of(self.store, key_of(&r, form).as_deref()) else { match miss(x, me)? {} };
        let mut images = json!({ "held": 0 });
        if !is_author {
            // Recorded only if the item is STILL visible after the body was read: a takedown or hold that landed
            // meanwhile turns the import into a 404 with nothing recorded.
            if !record_served(x, me, &r, form, ServeKind::Import)? {
                match miss(x, me)? {}
            }
            let h = hold_imported_images(x, me, &r.current_publish_id, form)?;
            images = json!({ "held": h.held });
            if let Some(u) = h.until {
                images["until"] = json!(u);
            }
            if let Some(why) = h.reason {
                images["reason"] = json!(why);
            }
        }
        let (questions, license_check) = questions_for(x.conn, &r.current_publish_id, form, &r.p_license)?;
        let mut item = item_view(&r, me, form);
        let (tags, standards) = tags_for(x, &r.current_publish_id)?;
        item["tags"] = json!(tags);
        item["standards"] = json!(standards);
        let may: Vec<&str> = if is_author { LICENSES.to_vec() } else { may_republish_as(x, me, &r.current_publish_id, form)? };
        let head: Value = json!({
            "item": item,
            "questions": questions,
            "license_check": license_check,
            "license": r.p_license,
            "may_republish_as": may,
            "previously_imported_versions": before,
            "images": images,
            "recorded": !is_author,
        });
        Ok(ReadOut::Body(Streamed { head: head.to_string(), etag: None, payload, slots }))
    }
}

#[cfg(test)]
#[path = "read_tests.rs"]
mod tests;
