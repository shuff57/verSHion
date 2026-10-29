//! The circle version routes (`server/worker/README.md`, rounds 37 to 40) over a [`Pool`]: sending a
//! new version of a bank, its history, reading an older version, the merge-initiate answer, rolling
//! back and trimming. Typed answers, no UI.
//!
//! Server text may arrive as `null` (a withheld version, an erased person): it is read as empty,
//! never as a parse failure. Every text is kept raw here and drawn through `api::plain`.

use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use super::api::ApiError;
use super::circles_api::{BankDoc, BankHead, CircleSend, MESSAGE_MAX, SentBank, check_id, null_text};
use super::pool::*;
use super::transport::Method;

/// The most a bank keeps in full; "trim" keeps 1 to one fewer.
pub const KEPT_VERSIONS: u32 = 20;

fn null_num<'de, D: serde::Deserializer<'de>, T: Default + Deserialize<'de>>(d: D) -> Result<T, D::Error> {
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

/// One line of a bank's history.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct VersionLine {
    #[serde(deserialize_with = "null_num")]
    pub version: u64,
    #[serde(deserialize_with = "null_text")]
    pub version_id: String,
    #[serde(deserialize_with = "null_text")]
    pub by: String,
    #[serde(deserialize_with = "null_num")]
    pub at: i64,
    /// Blank for a withheld version.
    #[serde(deserialize_with = "null_text")]
    pub message: String,
    #[serde(deserialize_with = "null_num")]
    pub added: u32,
    #[serde(deserialize_with = "null_num")]
    pub changed: u32,
    #[serde(deserialize_with = "null_num")]
    pub removed: u32,
    #[serde(deserialize_with = "null_num")]
    pub question_count: u32,
    #[serde(deserialize_with = "null_text")]
    pub license: String,
    /// Its body is still kept (can be looked at and restored).
    #[serde(deserialize_with = "null_num")]
    pub kept: bool,
    /// Held back for an administrator's review: hidden until an administrator releases it (the pool
    /// does not say why: student data and a copyright match look the same).
    #[serde(deserialize_with = "null_num")]
    pub withheld: bool,
    pub rollback_of: Option<u64>,
    pub merged_from: Option<u64>,
    /// Round 48, a withheld version only: `full` (its text is still kept for the review) or `hashes_only`
    /// (only a record of what matched is kept, as fingerprints, for the administrator).
    pub review_copy: Option<String>,
}

impl VersionLine {
    /// Kept for the review as fingerprints only: its text is gone.
    pub fn fingerprints_only(&self) -> bool {
        self.withheld && self.review_copy.as_deref() == Some("hashes_only")
    }
}

/// A page of the history, newest first.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VersionsPage {
    pub bank_id: String,
    pub head_version: u64,
    pub oldest_kept: Option<u64>,
    pub pinned_versions: Vec<u64>,
    pub versions: Vec<VersionLine>,
    pub next_before: Option<u64>,
}

/// The answer to merge-initiate: what happened since the version the copy was made from.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MergeStart {
    pub base_version: u64,
    /// The base is still kept in full on the server.
    pub base_kept: bool,
    pub head_version: u64,
    pub head_version_id: String,
    pub head_at: i64,
    pub head_by: String,
    /// The first 50 versions after the base, oldest first.
    pub between: Vec<VersionLine>,
    pub between_total: u64,
    pub merge_token: String,
    pub oldest_kept: Option<u64>,
}

/// What a trim did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Trimmed {
    pub pruned: u64,
    pub oldest_kept: Option<u64>,
    pub pinned_versions: Vec<u64>,
    /// Versions kept for an administrator's review (round 47/48): a trim can not delete them.
    pub evidence_kept: u64,
}

/// Phase D2 (plan 7): one entry of `GET .../changes`: what changed in `version` since `compared_to` (the
/// next readable kept version below it), by the circle's question ids. `gap`: its lists could not be
/// worked out (an index is gone, or the entry did not carry them): nothing is known about it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChangeEntry {
    pub version: u64,
    pub compared_to: u64,
    pub gap: bool,
    pub added: Vec<String>,
    pub changed: Vec<String>,
    pub removed: Vec<String>,
}

impl ChangeEntry {
    /// What happened to the question `qid` in this version, by its lists.
    pub fn touches(&self, qid: &str) -> Option<QuestionChange> {
        if self.gap {
            return None;
        }
        let has = |l: &[String]| l.iter().any(|x| x == qid);
        if has(&self.added) {
            Some(QuestionChange::Added)
        } else if has(&self.changed) {
            Some(QuestionChange::Changed)
        } else if has(&self.removed) {
            Some(QuestionChange::Removed)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuestionChange {
    Added,
    Changed,
    Removed,
}

/// The change lists of a bank, newest first. `per_question: false` (`reason` `keys_hidden`): a viewer of a
/// bank that hides its keys, who gets no lists (plan 5.4, Q4). `etag` for `If-None-Match`; `from_cache`: a
/// 304 said nothing moved, and this is the page read before.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChangesPage {
    pub bank_id: String,
    pub head_version: u64,
    pub per_question: bool,
    pub reason: String,
    pub versions: Vec<ChangeEntry>,
    pub etag: Option<String>,
    pub from_cache: bool,
}

/// (round 3, critic I2) The most entries a change-list answer is read for: the pool reads at most 40 indexes.
pub const CHANGES_MAX: usize = 40;
/// The most question ids one entry's lists carry between them (a bank far bigger than any the app makes).
pub const CHANGE_IDS_MAX: usize = 5000;
/// The most bytes of a change-list answer read.
pub const CHANGES_RESPONSE_MAX: usize = 1 << 20;

/// A list of question ids; `None` when the key is missing, null or not a list.
fn ids_at(v: &Value, key: &str) -> Option<Vec<String>> {
    v.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string).collect())
}

fn change_entry(v: &Value) -> Option<ChangeEntry> {
    let version = v.get("version").and_then(Value::as_u64).filter(|n| *n > 0)?;
    let compared_to = v.get("compared_to").and_then(Value::as_u64).unwrap_or(0);
    if compared_to >= version {
        return None;
    }
    let lists = (ids_at(v, "added"), ids_at(v, "changed"), ids_at(v, "removed"));
    // (null-tolerant) an entry that carries no list at all says nothing: a gap, never "nothing changed"
    // (round 3, critic I2) lists longer than any bank the app makes are not read: the entry says nothing
    let too_long = [&lists.0, &lists.1, &lists.2].iter().map(|l| l.as_ref().map_or(0, Vec::len)).sum::<usize>() > CHANGE_IDS_MAX;
    let gap = v.get("gap").and_then(Value::as_bool) == Some(true) || compared_to == 0 || too_long || (lists.0.is_none() && lists.1.is_none() && lists.2.is_none());
    if gap {
        return Some(ChangeEntry { version, compared_to, gap: true, ..ChangeEntry::default() });
    }
    Some(ChangeEntry { version, compared_to, gap: false, added: lists.0.unwrap_or_default(), changed: lists.1.unwrap_or_default(), removed: lists.2.unwrap_or_default() })
}

/// The answer of `GET .../changes` as this app keeps it.
pub fn changes_page(v: &Value, etag: Option<String>) -> ChangesPage {
    let reason = text_of(v, "reason");
    let per_question = reason != "keys_hidden" && v.get("per_question").and_then(Value::as_bool).unwrap_or(true);
    let head = v.get("head_version").and_then(Value::as_u64).unwrap_or(0);
    // (round 3, critic I2) never more entries than the pool reads, none above its head, read no further than that
    let mut versions: Vec<ChangeEntry> = if per_question {
        v.get("versions").and_then(Value::as_array).map(|a| a.iter().take(4 * CHANGES_MAX).filter_map(change_entry).filter(|e| head == 0 || e.version <= head).collect()).unwrap_or_default()
    } else {
        Vec::new()
    };
    versions.sort_by_key(|a| std::cmp::Reverse(a.version));
    versions.dedup_by_key(|e| e.version);
    versions.truncate(CHANGES_MAX);
    ChangesPage { bank_id: text_of(v, "bank_id"), head_version: v.get("head_version").and_then(Value::as_u64).unwrap_or(0), per_question, reason, versions, etag, from_cache: false }
}

fn list_of<T: serde::de::DeserializeOwned>(v: &Value, key: &str) -> Result<Vec<T>, PoolError> {
    match v.get(key) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(x) => Ok(serde_json::from_value(x.clone()).map_err(|e| ApiError::BadReply(format!("{key}: {e}")))?),
    }
}

fn u64s(v: &Value, key: &str) -> Vec<u64> {
    v.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default()
}

fn text_of(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

impl<'a> Pool<'a> {
    /// `PUT /circles/:cid/banks/:bid`: a new version of a bank, over the version `base_version`.
    /// Pictures go up first; a `blob_missing` answer sends them once more. Refusals worth knowing:
    /// 409 `conflict` (someone sent first), 428 `confirm_large_required`, 422 `license_author_only`,
    /// 422 `merge_token_invalid`.
    pub async fn send_circle_version(
        &self,
        cid: &str,
        bid: &str,
        req: &CircleSend,
        media: &dyn MediaSource,
        progress: &mut dyn FnMut(PublishProgress),
    ) -> Result<SentBank, PoolError> {
        check_id(cid)?;
        check_id(bid)?;
        if req.base_version.is_none() {
            return Err(LocalProblem::BadField("base_version").into());
        }
        let body = req.body()?;
        let uses = images_in_payload(&req.payload)?;
        let imgs = load_images(&uses, media)?;
        self.upload(&imgs, progress).await?;
        let mut resent = false;
        loop {
            progress(PublishProgress::Sending);
            let mut c = Call::new(Method::Put, format!("/circles/{cid}/banks/{bid}"));
            c.body = Body::Bytes { data: body.clone(), content_type: PUBLISH_CONTENT_TYPE.into() };
            c.timeout = Some(Duration::from_secs(180));
            match self.call(&c).await {
                Err(PoolError::Refused(r)) if !resent && matches!(r.kind, RefusalKind::BlobMissing { .. }) => {
                    resent = true;
                    progress(PublishProgress::ResendingImages { count: imgs.len() });
                    self.upload(&imgs, progress).await?;
                }
                Ok(r) => return parse(&r),
                Err(e) => return Err(e),
            }
        }
    }

    /// `GET /circles/:cid/banks/:bid/versions?before=n`: the history, newest first.
    pub async fn circle_versions(&self, cid: &str, bid: &str, before: Option<u64>) -> Result<VersionsPage, PoolError> {
        check_id(cid)?;
        check_id(bid)?;
        let mut path = format!("/circles/{cid}/banks/{bid}/versions");
        if let Some(b) = before {
            path.push_str(&format!("?before={b}"));
        }
        let v: Value = parse(&self.call(&Call::new(Method::Get, path)).await?)?;
        Ok(VersionsPage {
            bank_id: text_of(&v, "bank_id"),
            head_version: v.get("head_version").and_then(Value::as_u64).unwrap_or(0),
            oldest_kept: v.get("oldest_kept").and_then(Value::as_u64),
            pinned_versions: u64s(&v, "pinned_versions"),
            versions: list_of(&v, "versions")?,
            next_before: v.get("next_before").and_then(Value::as_u64),
        })
    }

    /// Phase D2: `GET /circles/:cid/banks/:bid/changes`, the per-question change lists (one `item_reads`
    /// and one `circle_change_reads`).
    pub async fn circle_changes(&self, cid: &str, bid: &str) -> Result<ChangesPage, PoolError> {
        self.circle_changes_if(cid, bid, None).await
    }

    /// The same with `If-None-Match` from the page read before (`cached`, of this bank): a 304 is that page
    /// again (`from_cache`), and is not counted by the pool.
    pub async fn circle_changes_if(&self, cid: &str, bid: &str, cached: Option<&ChangesPage>) -> Result<ChangesPage, PoolError> {
        check_id(cid)?;
        check_id(bid)?;
        let cached = cached.filter(|p| p.bank_id == bid && p.etag.is_some());
        let mut c = Call::new(Method::Get, format!("/circles/{cid}/banks/{bid}/changes"));
        c.max_response = Some(CHANGES_RESPONSE_MAX);
        if let Some(e) = cached.and_then(|p| p.etag.clone()) {
            c.headers.push(("If-None-Match".into(), e));
        }
        let r = self.call(&c).await?;
        if r.not_modified() {
            return match cached {
                Some(p) => Ok(ChangesPage { from_cache: true, ..p.clone() }),
                None => Err(ApiError::BadReply("304 without a cached copy".into()).into()),
            };
        }
        let v: Value = parse(&r)?;
        Ok(changes_page(&v, r.etag.clone()))
    }

    /// `GET /circles/:cid/banks/:bid/versions/:n`: a kept version's body, in the shape of the head
    /// read. `404 version_not_retained` / `404 version_withheld` when it can not be shown.
    pub async fn circle_version(&self, cid: &str, bid: &str, n: u64) -> Result<BankDoc, PoolError> {
        check_id(cid)?;
        check_id(bid)?;
        let mut c = Call::new(Method::Get, format!("/circles/{cid}/banks/{bid}/versions/{n}"));
        c.binary = true;
        c.max_response = Some(MAX_ITEM_RESPONSE);
        c.timeout = Some(Duration::from_secs(120));
        let r = self.call(&c).await?;
        let (head, payload) = split_item(&r.bytes)?;
        let bank: BankHead = serde_json::from_value(head.get("bank").cloned().unwrap_or(Value::Null)).map_err(|e| ApiError::BadReply(e.to_string()))?;
        Ok(BankDoc {
            circle_id: head.pointer("/circle/id").and_then(Value::as_str).unwrap_or(cid).to_string(),
            circle_name: head.pointer("/circle/name").and_then(Value::as_str).unwrap_or_default().to_string(),
            bank,
            origins: head.get("origins").cloned().unwrap_or(Value::Null),
            payload: payload.to_vec(),
            etag: r.etag.clone(),
        })
    }

    /// `POST /circles/:cid/banks/:bid/merge {base_version}`: what changed since, and a token (30
    /// minutes) that a merged send carries. Informational: nothing is stored.
    pub async fn merge_start(&self, cid: &str, bid: &str, base_version: u64) -> Result<MergeStart, PoolError> {
        check_id(cid)?;
        check_id(bid)?;
        let v: Value = parse(&self.call(&Call::new(Method::Post, format!("/circles/{cid}/banks/{bid}/merge")).json(&json!({ "base_version": base_version }))).await?)?;
        let token = text_of(&v, "merge_token");
        if token.is_empty() {
            return Err(ApiError::BadReply("merge_token".into()).into());
        }
        Ok(MergeStart {
            base_version: v.get("base_version").and_then(Value::as_u64).unwrap_or(base_version),
            base_kept: v.get("base_kept").and_then(Value::as_bool).unwrap_or(false),
            head_version: v.get("head_version").and_then(Value::as_u64).unwrap_or(0),
            head_version_id: text_of(&v, "head_version_id"),
            head_at: v.get("head_at").and_then(Value::as_i64).unwrap_or(0),
            head_by: text_of(&v, "head_by"),
            between: list_of(&v, "between")?,
            between_total: v.get("between_total").and_then(Value::as_u64).unwrap_or(0),
            merge_token: token,
            oldest_kept: v.get("oldest_kept").and_then(Value::as_u64),
        })
    }

    /// `POST /circles/:cid/banks/:bid/rollback`: the old version becomes a NEW newest one. The
    /// `commit_id` makes a retry safe.
    #[allow(clippy::too_many_arguments)]
    pub async fn rollback_circle_bank(&self, cid: &str, bid: &str, commit_id: &str, base_version: u64, to_version: u64, message: &str, confirm_large: bool) -> Result<SentBank, PoolError> {
        check_id(cid)?;
        check_id(bid)?;
        if !is_publish_id(commit_id) {
            return Err(LocalProblem::BadPublishId.into());
        }
        let msg = message.trim();
        if msg.chars().count() > MESSAGE_MAX {
            return Err(LocalProblem::BadField("message").into());
        }
        let mut body = serde_json::Map::new();
        body.insert("commit_id".into(), json!(commit_id));
        body.insert("base_version".into(), json!(base_version));
        body.insert("to_version".into(), json!(to_version));
        if !msg.is_empty() {
            body.insert("message".into(), json!(msg));
        }
        if confirm_large {
            body.insert("confirm_large_change".into(), json!(true));
        }
        let mut c = Call::new(Method::Post, format!("/circles/{cid}/banks/{bid}/rollback")).json(&Value::Object(body));
        c.timeout = Some(Duration::from_secs(120));
        parse(&self.call(&c).await?)
    }

    /// `POST /circles/:cid/banks/:bid/trim {keep}`: older versions stop being kept now.
    pub async fn trim_circle_bank(&self, cid: &str, bid: &str, keep: u32) -> Result<Trimmed, PoolError> {
        check_id(cid)?;
        check_id(bid)?;
        if keep == 0 || keep >= KEPT_VERSIONS {
            return Err(LocalProblem::BadField("keep").into());
        }
        let v: Value = parse(&self.call(&Call::new(Method::Post, format!("/circles/{cid}/banks/{bid}/trim")).json(&json!({ "keep": keep })).not_safe()).await?)?;
        let pinned_versions = u64s(&v, "pinned_versions");
        let evidence_kept = v.get("evidence_kept").and_then(Value::as_u64).unwrap_or(pinned_versions.len() as u64);
        Ok(Trimmed { pruned: v.get("pruned").and_then(Value::as_u64).unwrap_or(0), oldest_kept: v.get("oldest_kept").and_then(Value::as_u64), pinned_versions, evidence_kept })
    }
}
