//! The client data layer of the shared pool's item routes (`server/worker/README.md`, "The shared
//! pool"): typed calls over [`Client`](super::client::Client)'s transport, and no UI.
//!
//! - **Publish / republish** ([`Pool::publish`]): `POST /items` or `PUT /items/:id` with the
//!   `application/vnd.vershion.publish` body (`<meta JSON>\n<payload bytes>`, an exact
//!   `Content-Length`), the options the Worker reads (`hide_key`, label decisions, order
//!   acknowledgements, formatting acknowledgements, `image_ok`, an independence statement,
//!   visibility), and image blobs (`PUT /blobs/:sha256`, raw bytes) uploaded first or on demand
//!   ([`BlobOrder`]).
//! - **Refusals** ([`PoolError`], [`Refusal`], [`RefusalKind`]): every non-2xx answer keeps its whole
//!   [`ErrorDetail`] and is also sorted into a kind the UI can render: share-core's payload
//!   refusals with their findings, the licence refusal with `bound_by`, quota and daily limits,
//!   hold and removal states, `pool_not_configured`.
//! - **Reading**: browse and search ([`BrowseQuery`]), one item with `ETag` / `If-None-Match`
//!   ([`ItemCache`]), import (the served head and the payload bytes untouched), a served image,
//!   `versions`, `mine` (with the held, removed and needs-reupload states), reports, appeals,
//!   counter-notices, deletion and the delete-account preference.
//! - **Student view** ([`StudentView`]): the stripped payload as a summary a person can read, for the
//!   mandatory confirmation before a keyless publish; it can not hold a key, and says so loudly when
//!   the payload it was given still does.
//! - **Retry** ([`RetryPolicy`]): backoff for 429, 503 `busy`, 409 `publish_in_progress` and a lost
//!   connection, honouring `Retry-After`; calls that are not safe to repeat (a report) retry only the
//!   answers that mean "not processed".
//!
//! # What the app model still lacks
//!
//! The payload is handed over as bytes (the app serialises its bank or question as schema v2). The
//! Worker (and `share-core`) expect fields the app's `model.rs` does not carry yet: `keyless`,
//! `keep_order`, `rekeyed`, `origin` and `published` on a question. This module never builds a
//! payload, so it does not need them; the code that does must write them (see the report in
//! `docs/phase1-progress.md`).
//!
//! # Not here
//!
//! There is no dry-run route: the stripped form that students are served is produced by
//! `share-core` (`publish_prepare` with `hide_key`). The app has to run it locally (wasm or native)
//! and hand its `stripped.payload` to [`StudentView::from_payload`].

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::api::{ApiError, ErrorDetail};
use super::client::{Client, route_url};
use super::secret::Secret;
use super::transport::{LocalFuture, Method, Request, Response};

// ---- limits the Worker enforces (publish.ts, image.ts, moderation.ts), checked before sending ----

/// Largest payload the Worker takes (`MAX_PAYLOAD`).
pub const MAX_PAYLOAD: usize = 4 * 1024 * 1024;
/// Largest meta line (`MAX_META`).
pub const MAX_META: usize = 1024 * 1024;
/// Largest image (`MAX_IMAGE_BYTES`).
pub const MAX_IMAGE_BYTES: usize = 2 * 1024 * 1024;
/// A served item is a few MB; this is the read cap for item and import answers.
pub const MAX_ITEM_RESPONSE: usize = 16 * 1024 * 1024;
/// Largest piece of an account export (the first answer is up to 8 MB of bodies, parts 4 MB).
pub const MAX_EXPORT_RESPONSE: usize = 24 * 1024 * 1024;
/// `versions` takes at most this many ids per call.
pub const MAX_VERSION_IDS: usize = 200;
/// The attestation text revision the Worker accepts (`ATTEST_TEXT_REVS`).
pub const ATTEST_TEXT_REV: &str = "2026-10-19";
/// The publishing terms the person affirms on every publish (`attest.terms_version`, round 23). The
/// Worker answers 422 `attestation_required` for any other value.
pub const POOL_TERMS_VERSION: &str = "2026-10-22";
/// The affirmation as the Worker words it (`ATTEST_STATEMENT`); the person ticks a box next to this
/// text. Revision [`ATTEST_TEXT_REV`].
pub const ATTEST_STATEMENT: &str = "I confirm this content is my own original work, or I have the right to share it under the licence I chose. It is not copied from a published source (textbooks, publisher test banks, exam boards, paid worksheets) without permission.";
/// The publish body's type.
pub const PUBLISH_CONTENT_TYPE: &str = "application/vnd.vershion.publish";
/// What the person sees when the pool wants an invite code first (round 28).
pub const INVITE_REQUIRED_TEXT: &str = "The shared pool needs an invite code. Open the Account window, find \"Invite code\" and enter the code an admin gave you. Everything else in verSHion keeps working.";

/// A report note is cut to the Worker's 1000 characters.
pub const MAX_REPORT_NOTE: usize = 1000;

// ---- things the person or the app can get wrong before anything is sent -------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalProblem {
    /// A publish id must be 22 to 64 characters of `A-Za-z0-9_-`.
    BadPublishId,
    /// An item or question id is 32 lowercase hex characters.
    BadId(String),
    BadSha(String),
    PayloadTooLarge { len: usize, limit: usize },
    MetaTooLarge { len: usize, limit: usize },
    PayloadNotJson,
    /// `circle-only` can not be public.
    LicenseNotPublic,
    /// The independence statement needs 10 characters; the list 1 to 50 item ids.
    IndependenceClaim(&'static str),
    /// A picture the payload names is not in the media store.
    MissingMedia { file: String },
    /// (phase D3, plan section 6 "Pictures") A base question's picture, for a change the person did not send, is not
    /// on this computer and could not be fetched from the circle: nothing is sent.
    BaseMediaUnavailable,
    /// A name is not `media/<sha256>.<png|jpg|gif|webp>`.
    BadMediaName { file: String },
    /// The bytes of a picture do not hash to the name it has.
    MediaHashMismatch { file: String },
    MediaTooLarge { file: String, len: usize },
    EmptyBlob,
    /// A search text, tag, subject or note outside the Worker's bounds.
    BadField(&'static str),
    /// A downloaded image did not hash to the name the server was asked for.
    BlobHashMismatch,
    /// The server's answer could not be split into head and payload.
    BadItemShape,
    /// The circle's version holds questions this build of the app can not read (made by a newer one).
    NewerQuestions,
}

impl LocalProblem {
    pub fn message(&self) -> String {
        match self {
            LocalProblem::BadPublishId => "Internal error: the publish id is not valid.".into(),
            LocalProblem::BadId(_) => "That item id is not valid.".into(),
            LocalProblem::BadSha(_) => "That image name is not valid.".into(),
            LocalProblem::PayloadTooLarge { len, limit } => {
                format!("This is too big to share ({} of {} allowed). Split the bank into smaller ones.", mb(*len), mb(*limit))
            }
            LocalProblem::MetaTooLarge { .. } => "The details sent with this publish are too long.".into(),
            LocalProblem::PayloadNotJson => "Internal error: the payload is not JSON.".into(),
            LocalProblem::LicenseNotPublic => "An all-rights-reserved (circle-only) item can not be public. Pick a Creative Commons licence or keep it unlisted.".into(),
            LocalProblem::IndependenceClaim(why) => (*why).into(),
            LocalProblem::MissingMedia { file } => format!("A picture used by this question ({file}) is missing from this device. Re-add it, or remove it from the question."),
            LocalProblem::BaseMediaUnavailable => "The picture for a change you are not sending could not be fetched from the circle, so nothing was sent. Try again.".into(),
            LocalProblem::BadMediaName { file } => format!("A picture has a name the pool can not take ({file})."),
            LocalProblem::MediaHashMismatch { file } => format!("The picture {file} on this device does not match its name. Re-add it to the question."),
            LocalProblem::MediaTooLarge { file, len } => {
                format!("The picture {file} is {}, but the pool takes pictures up to {}. Use a smaller picture.", mb(*len), mb(MAX_IMAGE_BYTES))
            }
            LocalProblem::EmptyBlob => "The picture is empty.".into(),
            LocalProblem::BadField(f) => format!("The {f} is not valid."),
            LocalProblem::BlobHashMismatch => "The picture that came back is not the one that was asked for. Nothing was saved.".into(),
            LocalProblem::BadItemShape => "The server sent an item the app does not understand.".into(),
            LocalProblem::NewerQuestions => "This bank has questions made by a newer verSHion that this one can not read. Update the app, then try again. Nothing was changed.".into(),
        }
    }
}

pub(super) fn mb(n: usize) -> String {
    super::transport::size_text(n)
}

pub fn is_item_id(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub fn is_sha256(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub fn is_publish_id(s: &str) -> bool {
    (22..=64).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

// ---- errors ---------------------------------------------------------------------------------------

/// Why a pool call failed.
#[derive(Clone, Debug, PartialEq)]
pub enum PoolError {
    /// Nothing came back from the server, or the session is gone: offline, TLS, too large,
    /// signed out (the [`ApiError`] wording applies).
    Api(ApiError),
    /// The server answered no. The whole body is in [`Refusal::detail`].
    Refused(Box<Refusal>),
    /// Stopped before anything was sent.
    Local(LocalProblem),
}

impl From<ApiError> for PoolError {
    fn from(e: ApiError) -> Self {
        PoolError::Api(e)
    }
}
impl From<LocalProblem> for PoolError {
    fn from(e: LocalProblem) -> Self {
        PoolError::Local(e)
    }
}

impl PoolError {
    fn from_response(r: &Response) -> PoolError {
        let detail = ErrorDetail::from_response(r);
        PoolError::Refused(Box::new(Refusal { kind: RefusalKind::classify(&detail), detail }))
    }

    pub fn refusal(&self) -> Option<&Refusal> {
        match self {
            PoolError::Refused(r) => Some(r),
            _ => None,
        }
    }
    pub fn kind(&self) -> Option<&RefusalKind> {
        self.refusal().map(|r| &r.kind)
    }
    pub fn code(&self) -> &str {
        match self {
            PoolError::Refused(r) => &r.detail.code,
            _ => "",
        }
    }
    pub fn status(&self) -> u16 {
        match self {
            PoolError::Refused(r) => r.detail.status,
            _ => 0,
        }
    }
    pub fn retry_after(&self) -> Option<u64> {
        self.refusal().and_then(|r| r.detail.retry_after)
    }
    /// 503 `pool_not_configured`: the server has the account routes but not the pool (its hash
    /// secret or bucket is missing). Nothing the person can fix.
    pub fn is_pool_not_configured(&self) -> bool {
        matches!(self.kind(), Some(RefusalKind::PoolNotConfigured))
    }
    pub fn is_signed_out(&self) -> bool {
        matches!(self, PoolError::Api(ApiError::SignedOut))
    }
    /// The sentence to show.
    pub fn message(&self) -> String {
        match self {
            PoolError::Api(e) => e.message(super::api::Flow::Status),
            PoolError::Local(l) => l.message(),
            PoolError::Refused(r) => r.message(),
        }
    }
}

/// An error answer: what the server said, and the kind it falls under.
#[derive(Clone, Debug, PartialEq)]
pub struct Refusal {
    pub detail: ErrorDetail,
    pub kind: RefusalKind,
}

/// How much of a limit was used, and what the refused request needed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LimitInfo {
    pub used: u64,
    pub needed: u64,
    pub limit: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RefusalKind {
    /// 422 `invalid_payload`: share-core refused the payload. Findings are in the struct.
    InvalidPayload(Box<PayloadRefusal>),
    /// 422 `labels_need_decision`: letter labels need a Strip or Keep decision per finding.
    LabelsNeedDecision(Box<PayloadRefusal>),
    /// 422 `order_ack_required`: a frozen choice order must be acknowledged (`needs_order_ack`).
    OrderAckRequired(Box<PayloadRefusal>),
    /// 422 `license_incompatible`.
    LicenseIncompatible(Box<LicenseRefusal>),
    /// 422 `license_not_public`: `circle-only` can not be public.
    LicenseNotPublic,
    /// 422 `attestation_required`: the own-work affirmation is missing or for other terms.
    AttestationRequired,
    /// 422 `blob_missing`: an image the payload names is not on the server (`reupload`: it was, and
    /// was lost). Positions are question indices.
    BlobMissing { positions: Vec<usize>, reupload: bool },
    /// 422 `image_refused` (always `reason: "not_accepted"`: a format problem and a privacy block
    /// look the same).
    ImageRefused,
    /// 422 `hash_mismatch` on a blob upload.
    HashMismatch,
    SchemaUpgradeRequired { message: String },
    KindMismatch,
    TooManyTags { limit: u64 },
    /// 409 `conflict`: someone (another device) republished first.
    Conflict { current_version: Option<u64>, current_publish_id: Option<String> },
    /// 409 `publish_id_conflict`: the publish id was used for a different request.
    PublishIdConflict,
    /// 409 `publish_in_progress`: another publish of this account is running; retry shortly.
    PublishInProgress,
    /// 409 `item_removed` (`purged`: gone for good, publish it as a new item).
    ItemRemoved { purged: bool, message: Option<String> },
    /// 409 `under_moderation`, `already_removed`, `nothing_to_appeal`, `decided`, `window_closed`,
    /// `notice_not_actionable`.
    ModerationState { message: Option<String> },
    /// 413 `quota_exceeded`.
    QuotaExceeded(LimitInfo),
    /// 413 `removed_storage_exceeded`.
    RemovedStorageExceeded { used: u64, limit: u64, message: Option<String> },
    /// 413 `body_too_large`.
    BodyTooLarge { limit: Option<u64> },
    /// 422 `shared_questions_limit`.
    SharedQuestionsLimit(LimitInfo),
    /// 429 `daily_limit` (`metric`: `publishes`, `item_reads`, `browse`, `imports`, `reports`,
    /// `appeals`, `blob_uploads`, `blob_bytes`, `held_publishes`, ...). Resets at UTC midnight
    /// (`Retry-After`).
    DailyLimit { metric: Option<String> },
    /// 429 `rate_limited`.
    RateLimited,
    /// 429 `too_many_open_reads`.
    TooManyOpenReads,
    /// 429 `too_many_uploads`.
    TooManyUploads,
    /// 503 `busy`.
    Busy,
    /// 503 `pool_not_configured`.
    PoolNotConfigured,
    /// 403 `account_not_approved`.
    NotApproved { message: Option<String> },
    /// 403 `invite_required`: an approved teacher who has not redeemed an invite code (round 28).
    InviteRequired,
    /// 422 `invite_invalid`: unknown, expired, withdrawn or used-up code (all the same answer).
    InviteInvalid,
    /// 404. An item the person may not see is a 404 as well.
    NotFound,
    /// 403 `circle_role_required`: the circle's owner (or an editor) is needed for this.
    CircleRoleRequired { needed: Option<String> },
    /// 422 `circle_invite_invalid`: unknown, expired, used up, withdrawn or no longer usable (all the same).
    CircleInviteInvalid,
    /// 409 `circle_full`.
    CircleFull { limit: Option<u64> },
    /// 422 `circles_limit`: `circles_owned`, `circle_memberships`, `circle_banks`, `circle_open_invites`...
    CirclesLimit { metric: Option<String>, used: u64, limit: u64 },
    /// 409 `circle_closed`: the owner closed it; `undo_until` is when the owner can no longer reopen it.
    CircleClosed { undo_until: Option<i64>, reason: Option<String> },
    /// 409 `bank_removed`.
    BankRemoved,
    /// 404 `version_not_retained` (phase B).
    VersionNotRetained,
    /// 404 `version_withheld`: a version that held confirmed student data; only an administrator releases it.
    VersionWithheld,
    /// 409 `conflict` on a circle bank: someone sent first.
    CircleConflict { head_version: Option<u64>, head_at: Option<i64>, head_by: Option<String> },
    /// 428 `confirm_large_required`: the version drops more than half of the bank's questions.
    ConfirmLarge { removed: u64, total: u64 },
    /// 422 `license_author_only`: only the bank's author chooses its licence.
    LicenseAuthorOnly { license: Option<String> },
    /// 422 `merge_token_invalid`.
    MergeTokenInvalid,
    /// 422 `circle_questions_limit`.
    CircleQuestionsLimit(LimitInfo),
    /// 413 `circle_records_full`.
    CircleRecordsFull,
    /// 413 `quota_exceeded` with `scope: "circle"`: the circle's own storage.
    CircleStorageFull(LimitInfo),
    /// 409 `already_revoked`.
    AlreadyRevoked,
    /// 409 `commit_id_conflict`: the send id was used for different content.
    CommitIdConflict,
    /// 422 `circle_material` on a public publish: text first shared inside a circle.
    CircleMaterial(Box<CircleMaterial>),
    /// 400 `own_item`: an author can not report their own item.
    OwnItem,
    /// 400 `counter_notice_incomplete`.
    CounterNoticeIncomplete { missing: Vec<String> },
    /// 400 `invalid_request` with the field the Worker named.
    InvalidRequest { field: Option<String> },
    /// 422 `invite_not_sent` (circles phase C, round 46): no invitation was mailed and nothing was made.
    /// `reason`: `address_limit` (that mailbox had its mails for the day) or `not_deliverable`.
    InviteNotSent { reason: Option<String> },
    /// 429 `hourly_limit` (round 46/47): `metric` is `circle_email_invites_hourly`,
    /// `circle_email_inviter_domain_hourly` (with `limit`) or `circle_email_domain_hourly`.
    HourlyLimit { metric: Option<String>, limit: Option<u64> },
    /// 409 `keys_hidden_unavailable`: a viewer of a bank that hides its answer keys asked for a version
    /// that can not be shown without them.
    KeysHiddenUnavailable,
    /// 409 `hide_key_changed`: the bank's answer-key setting changed while a send or a restore was on its
    /// way. Nothing was stored: send again.
    HideKeyChanged { hide_key: Option<bool>, head_version: Option<u64> },
    /// 422 `keyless_question`: questions copied without their answer keys can not be sent to a circle.
    /// `positions` are 0-based; `kinds` is each one's question kind (`Short`, `Mc`, ...).
    KeylessQuestion { positions: Vec<usize>, kinds: Vec<String> },
    Other,
}

pub(super) fn str_of(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(str::to_string)
}
pub(super) fn u64_of(v: &Value, k: &str) -> Option<u64> {
    v.get(k).and_then(Value::as_u64)
}
fn limit_of(v: &Value) -> LimitInfo {
    LimitInfo { used: u64_of(v, "used").unwrap_or(0), needed: u64_of(v, "needed").unwrap_or(0), limit: u64_of(v, "limit").unwrap_or(0) }
}
pub(super) fn usizes(v: &Value, k: &str) -> Vec<usize> {
    v.get(k).and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_u64().map(|n| n as usize)).collect()).unwrap_or_default()
}

impl RefusalKind {
    pub fn classify(d: &ErrorDetail) -> RefusalKind {
        let j = &d.json;
        let message = str_of(j, "message");
        match (d.status, d.code.as_str()) {
            (_, "invalid_payload") => RefusalKind::InvalidPayload(Box::new(PayloadRefusal::from_json(j))),
            (_, "labels_need_decision") => RefusalKind::LabelsNeedDecision(Box::new(PayloadRefusal::from_json(j))),
            (_, "order_ack_required") => RefusalKind::OrderAckRequired(Box::new(PayloadRefusal::from_json(j))),
            (_, "license_incompatible") => RefusalKind::LicenseIncompatible(Box::new(LicenseRefusal::from_json(j))),
            (_, "license_not_public") => RefusalKind::LicenseNotPublic,
            (_, "attestation_required") => RefusalKind::AttestationRequired,
            (_, "blob_missing") => RefusalKind::BlobMissing { positions: usizes(j, "positions"), reupload: str_of(j, "reason").as_deref() == Some("reupload") },
            (_, "image_refused") => RefusalKind::ImageRefused,
            (_, "hash_mismatch") => RefusalKind::HashMismatch,
            (_, "schema_upgrade_required") => RefusalKind::SchemaUpgradeRequired { message: message.unwrap_or_default() },
            (_, "kind_mismatch") => RefusalKind::KindMismatch,
            (_, "too_many_tags") => RefusalKind::TooManyTags { limit: u64_of(j, "limit").unwrap_or(0) },
            (409, "conflict") if j.get("head_version").is_some() => RefusalKind::CircleConflict { head_version: u64_of(j, "head_version"), head_at: j.get("head_at").and_then(Value::as_i64), head_by: str_of(j, "head_by") },
            (409, "conflict") => RefusalKind::Conflict { current_version: u64_of(j, "current_version"), current_publish_id: str_of(j, "current_publish_id") },
            (_, "publish_id_conflict") => RefusalKind::PublishIdConflict,
            (_, "publish_in_progress") => RefusalKind::PublishInProgress,
            (_, "item_removed") => RefusalKind::ItemRemoved { purged: j.get("purged").and_then(Value::as_bool).unwrap_or(false), message },
            (_, "under_moderation" | "already_removed" | "nothing_to_appeal" | "decided" | "window_closed" | "notice_not_actionable" | "bank_not_removed") => {
                RefusalKind::ModerationState { message }
            }
            (_, "quota_exceeded") if str_of(j, "scope").as_deref() == Some("circle") => RefusalKind::CircleStorageFull(limit_of(j)),
            (_, "quota_exceeded") => RefusalKind::QuotaExceeded(limit_of(j)),
            (_, "circle_role_required") => RefusalKind::CircleRoleRequired { needed: str_of(j, "needed") },
            (_, "circle_invite_invalid") => RefusalKind::CircleInviteInvalid,
            (_, "circle_full") => RefusalKind::CircleFull { limit: u64_of(j, "limit") },
            (_, "circles_limit") => RefusalKind::CirclesLimit { metric: str_of(j, "metric"), used: u64_of(j, "used").unwrap_or(0), limit: u64_of(j, "limit").unwrap_or(0) },
            (_, "circle_closed") => RefusalKind::CircleClosed { undo_until: j.get("undo_until").and_then(Value::as_i64), reason: j.get("reason").and_then(Value::as_str).map(str::to_string) },
            (_, "bank_removed") => RefusalKind::BankRemoved,
            (_, "version_not_retained") => RefusalKind::VersionNotRetained,
            (_, "version_withheld") => RefusalKind::VersionWithheld,
            (_, "confirm_large_required") => RefusalKind::ConfirmLarge { removed: u64_of(j, "removed").unwrap_or(0), total: u64_of(j, "total").unwrap_or(0) },
            (_, "license_author_only") => RefusalKind::LicenseAuthorOnly { license: str_of(j, "license") },
            (_, "merge_token_invalid") => RefusalKind::MergeTokenInvalid,
            (_, "circle_questions_limit") => RefusalKind::CircleQuestionsLimit(limit_of(j)),
            (_, "circle_records_full") => RefusalKind::CircleRecordsFull,
            (_, "already_revoked") => RefusalKind::AlreadyRevoked,
            (_, "commit_id_conflict") => RefusalKind::CommitIdConflict,
            (_, "circle_material") => RefusalKind::CircleMaterial(Box::new(CircleMaterial::from_json(j))),
            (_, "removed_storage_exceeded") => {
                RefusalKind::RemovedStorageExceeded { used: u64_of(j, "used").unwrap_or(0), limit: u64_of(j, "limit").unwrap_or(0), message }
            }
            (_, "body_too_large") => RefusalKind::BodyTooLarge { limit: u64_of(j, "limit") },
            (_, "shared_questions_limit") => RefusalKind::SharedQuestionsLimit(limit_of(j)),
            (_, "daily_limit") => RefusalKind::DailyLimit { metric: str_of(j, "metric") },
            (_, "rate_limited") => RefusalKind::RateLimited,
            (_, "too_many_open_reads") => RefusalKind::TooManyOpenReads,
            (_, "too_many_uploads") => RefusalKind::TooManyUploads,
            (_, "busy") => RefusalKind::Busy,
            (_, "pool_not_configured") => RefusalKind::PoolNotConfigured,
            (_, "account_not_approved") => RefusalKind::NotApproved { message },
            (_, "invite_required") => RefusalKind::InviteRequired,
            (_, "invite_invalid") => RefusalKind::InviteInvalid,
            (_, "own_item") => RefusalKind::OwnItem,
            (_, "counter_notice_incomplete") => RefusalKind::CounterNoticeIncomplete {
                missing: j.get("missing").and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default(),
            },
            (_, "invite_not_sent") => RefusalKind::InviteNotSent { reason: str_of(j, "reason") },
            (_, "hourly_limit") => RefusalKind::HourlyLimit { metric: str_of(j, "metric"), limit: u64_of(j, "limit") },
            (_, "keys_hidden_unavailable") => RefusalKind::KeysHiddenUnavailable,
            (_, "hide_key_changed") => RefusalKind::HideKeyChanged { hide_key: j.get("hide_key").and_then(Value::as_bool), head_version: u64_of(j, "head_version") },
            (_, "keyless_question") => RefusalKind::KeylessQuestion { positions: usizes(j, "positions"), kinds: strings(j, "kinds") },
            (400, "invalid_request") => RefusalKind::InvalidRequest { field: d.field.clone() },
            (404, _) => RefusalKind::NotFound,
            _ => RefusalKind::Other,
        }
    }
}

impl Refusal {
    pub fn code(&self) -> &str {
        &self.detail.code
    }

    /// A sentence for the person. Wording for the payload refusals is the caller's job (it renders
    /// the findings); this gives the headline.
    pub fn message(&self) -> String {
        let wait = || super::api::wait_text(self.detail.retry_after);
        match &self.kind {
            RefusalKind::InvalidPayload(_) => "The pool can not take this as it is. Nothing was sent.".into(),
            RefusalKind::LabelsNeedDecision(_) => "Some answer choices start with letter labels (A, B, C ...). Choose whether to strip them or keep them.".into(),
            RefusalKind::OrderAckRequired(_) => "The order of some choices has to stay fixed. Confirm that students will see them in this order.".into(),
            RefusalKind::LicenseIncompatible(l) => l.message.clone().unwrap_or_else(|| {
                "The licence you chose is not one that the material this is built on allows. Pick a licence from the allowed list, split the bank, or say you wrote the questions independently.".into()
            }),
            RefusalKind::LicenseNotPublic => LocalProblem::LicenseNotPublic.message(),
            RefusalKind::AttestationRequired => "Tick the box that says this is your own work (or that you have the right to share it). Nothing is shared without it.".into(),
            RefusalKind::BlobMissing { reupload: true, .. } => "A picture was lost on the server. It will be uploaded again.".into(),
            RefusalKind::BlobMissing { .. } => "A picture has not reached the server yet.".into(),
            RefusalKind::ImageRefused => "The pool did not accept this picture. Use a PNG, JPEG, GIF or WebP still picture up to 4096 pixels on a side and 2 MB.".into(),
            RefusalKind::HashMismatch => "The picture changed while it was being sent. Try again.".into(),
            RefusalKind::SchemaUpgradeRequired { message } if !message.is_empty() => message.clone(),
            RefusalKind::SchemaUpgradeRequired { .. } => "Republishing needs a newer version of verSHion.".into(),
            RefusalKind::KindMismatch => "A bank can not be republished over a single question, or the other way round.".into(),
            RefusalKind::TooManyTags { limit } => format!("Too many distinct tags and topics in this item (at most {limit})."),
            RefusalKind::Conflict { .. } => "This was changed from another device or tab since you last loaded it. Reload it, then republish.".into(),
            RefusalKind::PublishIdConflict => "This publish was already used for different content. Start the publish again.".into(),
            RefusalKind::PublishInProgress => "Another publish from your account is still running. Wait a few seconds and try again.".into(),
            RefusalKind::ItemRemoved { purged: true, .. } => "This item was deleted for good. Publish it again as a new item.".into(),
            RefusalKind::ItemRemoved { message, .. } | RefusalKind::ModerationState { message } | RefusalKind::NotApproved { message } => {
                message.clone().unwrap_or_else(|| "The server does not allow that right now.".into())
            }
            RefusalKind::QuotaExceeded(l) => format!("Your shared storage is full ({} of {} used). Delete something you shared, or use smaller pictures.", mb(l.used as usize), mb(l.limit as usize)),
            RefusalKind::RemovedStorageExceeded { .. } => "Deleted items still hold storage for 14 days. Undo a deletion or wait.".into(),
            RefusalKind::BodyTooLarge { limit } => match limit {
                Some(l) => format!("That is too big for the pool (the limit is {}).", mb(*l as usize)),
                None => "That is too big for the pool.".into(),
            },
            RefusalKind::SharedQuestionsLimit(l) => format!("You can share up to {} questions in all; this would make {}.", l.limit, l.used + l.needed),
            RefusalKind::CircleRoleRequired { needed } => match needed.as_deref() {
                Some("owner") => "Only the circle's owner can do that.".into(),
                Some("owner_or_author") => "Only the circle's owner, or the bank's author while an editor, can do that.".into(),
                Some("editor") => "You can only look at this circle's banks. The owner can let you add banks by changing your role to editor.".into(),
                _ => "Your role in this circle does not allow that.".into(),
            },
            RefusalKind::CircleInviteInvalid => "This circle code does not work. It may have expired, been used up or been withdrawn, or it may have been mistyped. Ask the circle's owner for a new one.".into(),
            RefusalKind::CircleFull { limit } => match limit {
                Some(l) => format!("This circle already has {l} members, the most it can have."),
                None => "This circle is full.".into(),
            },
            RefusalKind::CirclesLimit { metric, used, limit } => match metric.as_deref() {
                Some("circles_owned") => format!("You already own {used} circles, the most one account can own ({limit}). A circle you closed still counts for 14 days. Hand one to another member, or wait until a closed one is gone."),
                Some("circle_memberships") | Some("circle_memberships_total") => format!("You are already in {used} circles, the most one account can be in ({limit}). Leave one first."),
                Some("circle_banks") => format!("This circle already has {used} banks, the most it can hold ({limit}). Remove one first."),
                Some("circle_open_invites") => format!("This circle already has {used} open codes ({limit} at most). Withdraw one you no longer need."),
                Some("circle_open_email_invites") => format!(
                    "This circle already has {used} emailed invitations that have not been used yet ({limit} at most). More can be sent as people join, as invitations expire after 7 days, or when you withdraw one you no longer need."
                ),
                _ => "You have reached a limit on circles.".into(),
            },
            RefusalKind::CircleClosed { reason, .. } => match reason.as_deref() {
                Some("admin") => "An administrator closed this circle. It can not be reopened here.".into(),
                Some("last_member_gone") => "This circle closed because nobody was left in it.".into(),
                Some("owner") => "This circle was closed by its owner.".into(),
                _ => "This circle is closed.".into(),
            },
            RefusalKind::BankRemoved => "This bank was removed from the circle.".into(),
            // (round 48: a withheld version whose text was let go: only the record of what matched is kept)
            RefusalKind::VersionNotRetained if self.detail.json.get("review_copy").and_then(Value::as_str) == Some("hashes_only") || self.detail.json.get("withheld").and_then(Value::as_bool) == Some(true) => {
                "That version was held back for an administrator's review, and its text is no longer kept: only a record of what matched is kept, for the administrator. It can not be opened.".into()
            }
            RefusalKind::VersionNotRetained => "That version is no longer kept. A circle bank keeps its last 20 versions in full.".into(),
            RefusalKind::VersionWithheld => "This version is hidden while it is reviewed. Only an administrator can release it.".into(),
            RefusalKind::CircleConflict { .. } => "Someone else sent changes first. Get the latest changes and merge them, then send again.".into(),
            RefusalKind::ConfirmLarge { removed, total } => format!("This version removes {removed} of the bank's {total} questions. Send it anyway only if that is what you meant."),
            RefusalKind::LicenseAuthorOnly { .. } => "Only the bank's author chooses its licence. Your changes go with the bank's own licence.".into(),
            RefusalKind::MergeTokenInvalid => "This merge is out of date. Get the latest changes and merge again, then send.".into(),
            RefusalKind::CircleQuestionsLimit(l) => format!("A circle holds at most {} questions across its banks; this would make {}.", l.limit, l.used + l.needed),
            RefusalKind::CircleRecordsFull => "This circle has recorded as many different questions as it can keep. Remove a bank first.".into(),
            RefusalKind::CircleStorageFull(l) => circle_storage_text(l),
            RefusalKind::AlreadyRevoked => "That code was already withdrawn.".into(),
            RefusalKind::CommitIdConflict => "This send was already used for different content. Start it again.".into(),
            RefusalKind::CircleMaterial(m) => m.message(),
            RefusalKind::InviteRequired => INVITE_REQUIRED_TEXT.into(),
            RefusalKind::InviteInvalid => "This invite code is not valid: it may have expired, been used up or been withdrawn. Check that you typed it right, or ask an admin for a new one.".into(),
            RefusalKind::DailyLimit { metric } if metric.as_deref() == Some("invite_tries") => {
                format!("You have tried as many invite codes today as the pool allows. Try again {}.", wait())
            }
            RefusalKind::DailyLimit { metric } if metric.as_deref() == Some("key_leak_reports") => {
                format!("You can report a leaked answer key once per item a month, and five a day. Try again {}.", wait())
            }
            RefusalKind::DailyLimit { metric } if metric.as_deref() == Some("circle_hide_attempts") => format!(
                "You have tried to change answer-key settings as many times as the pool allows today. Every try counts, also the ones where the pool asked for decisions. Try again {}.",
                wait()
            ),
            RefusalKind::DailyLimit { metric } if metric.as_deref() == Some("circle_hide_changes") => {
                format!("You have changed answer-key settings as many times as the pool allows today. Try again {}.", wait())
            }
            RefusalKind::DailyLimit { metric } if metric.as_deref() == Some("circle_hidden_reads") => format!(
                "You have opened banks whose answer keys are hidden as many times as the pool allows today (each one is prepared for you without its keys). Try again {}.",
                wait()
            ),
            RefusalKind::DailyLimit { metric } if metric.as_deref() == Some("circle_email_invites") => {
                format!("You have emailed as many circle invitations today as the pool allows. Try again {}, or make a code and give it to the person yourself.", wait())
            }
            RefusalKind::DailyLimit { metric } => format!("You have reached today's limit{}. It resets {}.", metric_text(metric.as_deref()), wait()),
            RefusalKind::RateLimited | RefusalKind::TooManyOpenReads | RefusalKind::TooManyUploads => format!("Too many requests. Try again {}.", wait()),
            RefusalKind::Busy => "The server is busy. Try again in a moment.".into(),
            RefusalKind::PoolNotConfigured => "The shared pool is not available on this server yet. Nothing you can change; everything else works.".into(),
            RefusalKind::NotFound => "That item is not available (it may have been removed or made private).".into(),
            RefusalKind::OwnItem => "You can not report your own item.".into(),
            RefusalKind::CounterNoticeIncomplete { missing } => format!("The notice is incomplete: {}.", missing.join(", ")),
            RefusalKind::InvalidRequest { field } => match field.as_deref() {
                Some("code") => "That does not look like an invite code. It has 24 letters and digits, usually shown in groups of four.".into(),
                Some("reason") => "This item shows its answer key, so it can not be reported as a key leak. Report a wrong answer instead.".into(),
                Some("email") => "That is not an email address the pool can send to. Check it for a typo.".into(),
                Some("before") => "The list could not be continued from where it was, so it starts again from the newest.".into(),
                Some(f) => format!("The server did not accept the {}.", super::api::plain(&f.replace('_', " "))),
                None => "The server did not accept that request.".into(),
            },
            RefusalKind::InviteNotSent { reason } => match reason.as_deref() {
                Some("address_limit") => "No invitation was sent: this address has had as many circle invitations today as it may get (from anyone). Try again tomorrow, or make a code and give it to the person yourself.".into(),
                Some("not_deliverable") => "No invitation was sent: the pool can not send mail to this address. Check it for a typo, or make a code and give it to the person yourself.".into(),
                _ => "No invitation was sent. Check the address, or make a code and give it to the person yourself.".into(),
            },
            RefusalKind::HourlyLimit { metric, limit } => match metric.as_deref() {
                Some("circle_email_inviter_domain_hourly") => format!(
                    "You have sent as many invitations to addresses at this mail domain (for example one school) as you may in an hour{}. The others can go {}: nothing is lost, invite them then.",
                    limit.map(|l| format!(" ({l})")).unwrap_or_default(),
                    wait()
                ),
                Some("circle_email_domain_hourly") => format!("There have been too many invitations to addresses at this mail domain in the last hour, from everyone. Try again {}.", wait()),
                Some("circle_email_invites_hourly") => format!("You have sent as many circle invitations as you may this hour. Try again {}.", wait()),
                _ => format!("That has been done as many times as the pool allows in an hour. Try again {}.", wait()),
            },
            RefusalKind::KeysHiddenUnavailable => "The answer keys of this bank are hidden from viewers, and this version can not be shown without them. Ask the bank's author or the circle's owner.".into(),
            RefusalKind::HideKeyChanged { hide_key: Some(true), .. } => "The bank's answer keys were hidden from viewers while this was on its way, so nothing was stored. Send it again; the pool may ask how the viewers' copy should look.".into(),
            RefusalKind::HideKeyChanged { .. } => "The bank's answer-key setting changed while this was on its way, so nothing was stored. Send it again.".into(),
            RefusalKind::KeylessQuestion { positions, kinds } => keyless_text(positions, kinds),
            RefusalKind::Other => other_message(self.detail.status, &self.detail.code, wait()),
        }
    }
}

/// Plain words for the Worker's codes that have no [`RefusalKind`] of their own, with a default
/// that still says what happened. Covers every code the routes the app calls can return.
pub fn other_message(status: u16, code: &str, wait: String) -> String {
    let m = match code {
        "pending_review" => "An administrator is still reviewing a version of this bank. It can be deleted for good once that review is done. You can leave it removed until then.",
        "member_unavailable" => "That member's account can not take on the circle right now. Choose someone else.",
        "no_pending_transfer" => "There is no ownership offer to answer any more. It may have been withdrawn, taken back, or it expired.",
        "unauthorized" => "Your sign-in ended. Sign in again in the Account window.",
        "reauth_required" => "Sign in again to do this: it needs a sign-in from the last 24 hours.",
        "invalid_json" | "body_aborted" | "length_mismatch" | "length_required" | "unsupported_media_type" => {
            "The request did not arrive in a form the pool can read. Try again; if it keeps happening, update verSHion."
        }
        "body_timeout" => "The request took too long to arrive. Check your connection and try again.",
        "pool_key_mismatch" => "The pool server is being set up and can not take this yet. Try again later.",
        "pool_migrating" => "The pool is being upgraded. Try again in a few minutes.",
        "too_many_units" => "That is more questions than the pool checks in one go. Share a smaller bank.",
        "author_records_exceeded" => "You have shared as many items as the pool allows an account. Delete one you no longer need.",
        "imports_quota" => "You have copied as many different items as the pool allows an account.",
        "reads_quota" => "You have looked at as many different items as the pool allows an account.",
        "evidence_quota_exceeded" => "The pool can not take another import for this account right now. Nothing was imported.",
        "origin_not_allowed" => "The pool does not accept requests from this web address.",
        "client_ip_required" | "native_client_with_origin" => "The pool could not tell where this request came from. Nothing was changed.",
        "notice_not_found" => "That notice is not available to you.",
        "method_not_allowed" => "This version of verSHion asked for something the pool does not offer. Update verSHion.",
        "server_error" => "The pool had a problem of its own. Nothing was changed; try again in a minute.",
        "preview_required" | "confirm_large_required" | "selection_required" | "hold_has_own_route" | "action_closed" | "rightful_item_invalid" => {
            "That step is for the pool's administrators."
        }
        _ => "",
    };
    if !m.is_empty() {
        return m.into();
    }
    match status {
        401 => "Your sign-in ended. Sign in again in the Account window.".into(),
        403 => "The pool does not allow that for this account.".into(),
        408 => "The request took too long. Try again.".into(),
        429 => format!("Too many requests. Try again {wait}."),
        500..=599 => "The pool had a problem of its own. Nothing was changed; try again in a minute.".into(),
        _ if code.is_empty() => format!("The pool refused the request ({status})."),
        _ => format!("The pool refused the request ({code})."),
    }
}

fn metric_text(m: Option<&str>) -> String {
    match m {
        Some("exports") => " for downloading your data".into(),
        Some("previews" | "preview_units") => " for checking items before sharing".into(),
        Some("publish_refusals") => " for refused publishes".into(),
        Some("item_miss") => " for looking up items that are not there".into(),
        Some("publishes") => " for publishing".into(),
        Some("item_reads") => " for opening items".into(),
        Some("browse") => " for browsing".into(),
        Some("imports") => " for copying items".into(),
        Some("reports") => " for reports".into(),
        Some("appeals") => " for appeals".into(),
        Some("blob_uploads" | "blob_bytes") => " for picture uploads".into(),
        Some("held_publishes") => " for publishes that were held for review".into(),
        Some("circle_creates") => " for creating circles".into(),
        Some("circle_invites") => " for making circle codes".into(),
        Some("circle_join_tries") => " for circle codes that did not work".into(),
        Some("circle_commits") => " for sending banks to circles".into(),
        Some("circle_closes") => " for closing circles".into(),
        Some("circle_reports") => " for reports about circle banks".into(),
        Some("circle_rollbacks") => " for restoring older versions of circle banks".into(),
        Some("circle_transfer_offers") => " for offering a circle to someone".into(),
        Some("circle_email_invites") => " for emailed circle invitations".into(),
        Some("circle_hide_attempts") => " for tries at changing answer-key settings".into(),
        Some("circle_hide_changes") => " for changing answer-key settings".into(),
        Some("circle_hidden_reads") => " for opening banks whose answer keys are hidden".into(),
        _ => String::new(),
    }
}

/// The circle's own storage is full (413 `quota_exceeded`, `scope: circle`): what really frees space.
/// Versions kept for an administrator's review still count, and only an administrator can free them
/// (critic 56a V1); trimming and deleting free the rest.
pub fn circle_storage_text(l: &LimitInfo) -> String {
    format!(
        "This circle's storage is full ({} of {} used). The owner, or a bank's author, can free space by trimming the older versions of a bank (History of versions, then Trim old versions), or by removing a bank and deleting it for good. Versions kept for an administrator's review still count until an administrator has dealt with them; only an administrator can free that space. Smaller pictures also help.",
        mb(l.used as usize),
        mb(l.limit as usize)
    )
}

/// "question 3", "questions 1, 2 and 4", "questions 1, 2, 3 ... and 9 more" (from 0-based positions).
pub fn question_numbers(positions: &[usize]) -> String {
    let mut ns: Vec<usize> = positions.iter().map(|p| p + 1).collect();
    ns.sort_unstable();
    ns.dedup();
    match ns.len() {
        0 => "some questions".into(),
        1 => format!("question {}", ns[0]),
        n if n <= 8 => {
            let (last, rest) = ns.split_last().expect("not empty");
            format!("questions {} and {last}", rest.iter().map(usize::to_string).collect::<Vec<_>>().join(", "))
        }
        n => format!("questions {} ... and {} more", ns[..6].iter().map(usize::to_string).collect::<Vec<_>>().join(", "), n - 6),
    }
}

/// 422 `keyless_question` in plain words: which questions, and what each kind needs (round 48: a short answer or
/// essay has no correct answer to mark, only a sample answer to type).
pub fn keyless_text(positions: &[usize], kinds: &[String]) -> String {
    let short = kinds.iter().filter(|k| k.as_str() == "Short").count();
    let fix = if kinds.is_empty() {
        "give each of them its answer"
    } else if short == kinds.len() {
        "type a sample answer for each of them"
    } else if short == 0 {
        "mark the correct answer of each of them"
    } else {
        "give each of them its answer (mark the correct one, or type a sample answer)"
    };
    let one = positions.iter().collect::<std::collections::BTreeSet<_>>().len() == 1;
    format!(
        "In this copy, {} {} no answer {}: {} copied without {} (for example from a bank whose keys are hidden from viewers). The circle keeps the full questions with their keys, so nothing was sent. Read the full bank as an editor (make a new working copy from the circle and make your changes there), or {fix}.",
        question_numbers(positions),
        if one { "has" } else { "have" },
        if one { "key" } else { "keys" },
        if one { "it was" } else { "they were" },
        if one { "it" } else { "them" },
    )
}

// ---- circles: text first shared inside a circle (D13) -------------------------------------------------

/// One question the pool found first shared by another member of a circle.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CircleHit {
    /// 0-based question index in what was being published.
    pub pos: usize,
    pub circle_id: String,
    pub circle_name: Option<String>,
    pub bank_id: String,
    pub bank_title: Option<String>,
    /// A written statement that the questions are the author's own can lift the refusal.
    pub may_claim_independence: bool,
}

/// 422 `circle_material`: questions that were first shared by another member of a circle can not
/// be shared publicly from here.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CircleMaterial {
    pub positions: Vec<usize>,
    pub hits: Vec<CircleHit>,
}

impl CircleMaterial {
    pub fn from_json(j: &Value) -> CircleMaterial {
        let hits = j
            .get("hits")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|h| CircleHit {
                        pos: u64_of(h, "pos").unwrap_or(0) as usize,
                        circle_id: str_of(h, "circle_id").unwrap_or_default(),
                        circle_name: str_of(h, "circle_name"),
                        bank_id: str_of(h, "bank_id").unwrap_or_default(),
                        bank_title: str_of(h, "bank_title"),
                        may_claim_independence: h.get("may_claim_independence").and_then(Value::as_bool).unwrap_or(false),
                    })
                    .collect()
            })
            .unwrap_or_default();
        CircleMaterial { positions: usizes(j, "positions"), hits }
    }

    /// The banks the hits name, once each, in order.
    pub fn banks(&self) -> Vec<&CircleHit> {
        let mut seen: Vec<&str> = Vec::new();
        let mut out = Vec::new();
        for h in &self.hits {
            if !seen.contains(&h.bank_id.as_str()) {
                seen.push(&h.bank_id);
                out.push(h);
            }
        }
        out
    }

    /// Every named bank allows a written statement.
    pub fn all_may_claim(&self) -> bool {
        !self.hits.is_empty() && self.hits.iter().all(|h| h.may_claim_independence)
    }

    pub fn message(&self) -> String {
        "Some of these questions were first shared by another member of a circle you are or were in. They can not be shared publicly from here.".into()
    }
}

// ---- share-core's payload refusals --------------------------------------------------------------

/// A question, and a part of it (a Multipart's part) by position in the payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Pos {
    pub question: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<usize>,
}

impl Pos {
    pub fn question(question: usize) -> Pos {
        Pos { question, part: None }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LabelChoice {
    Strip,
    Keep,
}

/// The author's decision for one finding of `labels_need_decision`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelDecision {
    pub question: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<usize>,
    pub choice: LabelChoice,
}

/// One entry of share-core's `errors` array. Everything but `code` and `path` is optional.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Issue {
    pub code: String,
    pub path: String,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
    pub offset16: Option<u64>,
    pub cp: Option<String>,
    /// `key_in_formatting` / `key_in_stem`: the feature (`star`, `key_word`, `capitals`, ...).
    pub reason: Option<String>,
    pub ack_allowed: Option<bool>,
    /// The issue names the key by what marks it: it is for the publishing author's eyes only.
    pub author_only: bool,
    /// What the author can do, in English.
    pub hint: Option<String>,
    /// The Worker's own sentence for some codes (`CORE_MESSAGES`).
    pub message: Option<String>,
}

impl Issue {
    /// A key shows through the formatting or the stem (or a lock gives it away).
    pub fn is_key_leak(&self) -> bool {
        matches!(self.code.as_str(), "key_in_formatting" | "key_in_stem" | "locked_key" | "placeholder_key")
    }
    /// Low-risk findings the author can acknowledge (`ack_formatting`).
    pub fn can_acknowledge(&self) -> bool {
        self.ack_allowed == Some(true)
    }
    /// The question this refers to, read from `bank.questions[3]...` / `question....` paths.
    pub fn question_index(&self) -> Option<usize> {
        question_of_path(&self.path)
    }
}

fn question_of_path(path: &str) -> Option<usize> {
    if path.starts_with("question") && !path.starts_with("questions") {
        return Some(0);
    }
    let rest = path.split("questions[").nth(1).or_else(|| path.split("question=").nth(1))?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// A finding from `formatting_findings`: formatting that might give away the key, with the choice
/// text as students would see it.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct FormattingFinding {
    pub question: usize,
    pub code: String,
    pub path: String,
    pub reason: Option<String>,
    pub ack_allowed: bool,
    pub served: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct LabelPreview {
    pub before: String,
    pub after: String,
}

/// An undecided letter label: before and after what Strip would leave.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct LabelFinding {
    pub question: usize,
    pub part: Option<usize>,
    /// A Matching word bank, not Mc choices.
    pub matching: bool,
    /// Keeping the labels would reveal the order (and so freeze it).
    pub order_revealing: bool,
    pub strip_allowed: bool,
    pub decision: Option<LabelChoice>,
    pub choices: Vec<LabelPreview>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct OrderNote {
    pub question: usize,
    pub part: Option<usize>,
    /// `few_unlocked`, `labels_kept`, `sibling_refs`, `author_keep_order`.
    pub reason: String,
}

/// What will move, per Mc question or part and Matching word bank.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct OrderPreview {
    pub question: usize,
    pub part: Option<usize>,
    pub matching: bool,
    pub movable: Vec<usize>,
    pub frozen: Option<String>,
    pub frozen_if_keep: bool,
}

/// The author-facing detail of `invalid_payload`, `labels_need_decision` and `order_ack_required`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PayloadRefusal {
    pub issues: Vec<Issue>,
    pub formatting_findings: Vec<FormattingFinding>,
    /// All findings, of which `formatting_findings` may hold only the first ones.
    pub formatting_findings_total: usize,
    pub label_findings: Vec<LabelFinding>,
    pub needs_order_ack: Vec<Pos>,
    pub order_reasons: Vec<OrderNote>,
    pub order_preview: Vec<OrderPreview>,
}

fn list_of<T: DeserializeOwned>(v: &Value, k: &str) -> Vec<T> {
    v.get(k).and_then(Value::as_array).map(|a| a.iter().filter_map(|x| serde_json::from_value(x.clone()).ok()).collect()).unwrap_or_default()
}

impl PayloadRefusal {
    pub fn from_json(j: &Value) -> PayloadRefusal {
        let formatting_findings: Vec<FormattingFinding> = list_of(j, "formatting_findings");
        PayloadRefusal {
            issues: list_of(j, "errors"),
            formatting_findings_total: u64_of(j, "formatting_findings_total").map_or(formatting_findings.len(), |n| n as usize),
            formatting_findings,
            label_findings: list_of(j, "label_findings"),
            needs_order_ack: list_of(j, "needs_order_ack"),
            order_reasons: list_of(j, "order_reasons"),
            order_preview: list_of(j, "order_preview"),
        }
    }

    /// Issues that name the key (author only), whatever the author does about them.
    pub fn key_leaks(&self) -> impl Iterator<Item = &Issue> {
        self.issues.iter().filter(|i| i.is_key_leak())
    }

    /// Questions where every key issue can be acknowledged. Offer "I checked, these do not give the
    /// answer away" for these; the rest have to be edited.
    pub fn acknowledgeable_questions(&self) -> Vec<usize> {
        let mut by_q: std::collections::BTreeMap<usize, bool> = Default::default();
        for i in self.key_leaks() {
            if let Some(q) = i.question_index() {
                let e = by_q.entry(q).or_insert(true);
                *e &= i.can_acknowledge();
            }
        }
        by_q.into_iter().filter(|(_, ok)| *ok).map(|(q, _)| q).collect()
    }

    /// Issues that must be fixed in the question (not acknowledgeable, not a decision).
    pub fn blocking(&self) -> Vec<&Issue> {
        self.issues.iter().filter(|i| i.code != "order_ack_required" && i.code != "labels_need_decision" && !i.can_acknowledge()).collect()
    }

    /// Findings with no decision yet.
    pub fn undecided_labels(&self) -> Vec<Pos> {
        self.label_findings.iter().filter(|f| f.decision.is_none()).map(|f| Pos { question: f.question, part: f.part }).collect()
    }
}

/// `license_incompatible`: which questions can not take the licence, why, and what is allowed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LicenseRefusal {
    pub positions: Vec<usize>,
    /// Licences every question allows (empty: none; split the bank).
    pub allowed: Vec<String>,
    pub per_question: Vec<LicenseQuestion>,
    pub relicensed: Vec<Relicensed>,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LicenseQuestion {
    pub pos: usize,
    pub allowed: Vec<String>,
    pub bound_by: Vec<BoundBy>,
    pub evidence_overflow: bool,
}

/// An item whose terms bind a question. `item_id` is `None` (with `note`) for an item that is no
/// longer shown.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct BoundBy {
    pub item_id: Option<String>,
    pub title: String,
    pub author_name: String,
    pub license: Option<String>,
    /// The relabel this restriction came through.
    pub via: Option<String>,
    pub note: Option<String>,
}

/// A source relicensed after the account received it.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Relicensed {
    pub item_id: String,
    pub earlier: Vec<String>,
    pub now: String,
}

fn strings(v: &Value, k: &str) -> Vec<String> {
    v.get(k).and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

impl LicenseRefusal {
    pub fn from_json(j: &Value) -> LicenseRefusal {
        let per_question = j
            .get("per_question")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|q| LicenseQuestion {
                        pos: u64_of(q, "pos").unwrap_or(0) as usize,
                        allowed: strings(q, "allowed"),
                        bound_by: list_of(q, "bound_by"),
                        evidence_overflow: q.get("evidence_overflow").and_then(Value::as_bool).unwrap_or(false),
                    })
                    .collect()
            })
            .unwrap_or_default();
        LicenseRefusal { positions: usizes(j, "positions"), allowed: strings(j, "allowed"), per_question, relicensed: list_of(j, "relicensed"), message: str_of(j, "message") }
    }

    /// Every distinct item that binds any question.
    pub fn binders(&self) -> Vec<&BoundBy> {
        let mut seen = HashSet::new();
        self.per_question.iter().flat_map(|q| q.bound_by.iter()).filter(|b| seen.insert((b.item_id.clone(), b.author_name.clone(), b.license.clone()))).collect()
    }
}

// ---- retry and backoff ----------------------------------------------------------------------------

/// How a call is repeated. Safe calls (reads, a publish with its id, a blob upload) also retry a
/// lost connection and a 5xx; every call retries answers that mean "not processed" (429, 503
/// `busy`). A wait longer than `max_wait` (a daily limit's `Retry-After`) is never slept: the
/// error is returned with its `retry_after`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RetryPolicy {
    /// Tries in all (1 = no retry).
    pub attempts: u32,
    /// First wait without a `Retry-After`; doubles each time.
    pub base: Duration,
    pub max_wait: Duration,
    /// 100 sleeps as long as decided; 0 never sleeps (tests).
    pub time_scale_percent: u32,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        RetryPolicy { attempts: 4, base: Duration::from_millis(500), max_wait: Duration::from_secs(30), time_scale_percent: 100 }
    }
}

impl RetryPolicy {
    pub fn none() -> RetryPolicy {
        RetryPolicy { attempts: 1, ..RetryPolicy::default() }
    }
    /// Decides as usual, never sleeps.
    pub fn instant(attempts: u32) -> RetryPolicy {
        RetryPolicy { attempts, time_scale_percent: 0, ..RetryPolicy::default() }
    }

    /// The wait before try `attempt + 1`, or `None` when the server asked for longer than
    /// `max_wait`.
    pub fn wait(&self, attempt: u32, retry_after: Option<u64>) -> Option<Duration> {
        let w = match retry_after {
            Some(s) => Duration::from_secs(s),
            None => self.base.saturating_mul(1u32 << attempt.min(10)),
        };
        (w <= self.max_wait).then_some(w)
    }

    fn scaled(&self, d: Duration) -> Duration {
        d * self.time_scale_percent / 100
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn sleep(d: Duration) {
    // Native jobs run on their own thread (see `model::Runner`), so blocking it is fine.
    if !d.is_zero() {
        std::thread::sleep(d);
    }
}

#[cfg(target_arch = "wasm32")]
async fn sleep(d: Duration) {
    if d.is_zero() {
        return;
    }
    let ms = d.as_millis().min(i32::MAX as u128) as i32;
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        if let Some(w) = web_sys::window() {
            let _ = w.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
        }
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

fn retryable(kind: &RefusalKind, status: u16, safe: bool) -> bool {
    match kind {
        RefusalKind::Busy | RefusalKind::TooManyOpenReads | RefusalKind::TooManyUploads | RefusalKind::RateLimited => true,
        RefusalKind::PublishInProgress => safe,
        RefusalKind::DailyLimit { .. } | RefusalKind::PoolNotConfigured => false,
        _ => safe && matches!(status, 408 | 500 | 502 | 504),
    }
}

// ---- the session a Pool talks through ------------------------------------------------------------

/// What the pool layer needs of a signed-in client: the server address and an authenticated send
/// that hands back the whole answer. [`Client`] is the real one; tests bring their own.
pub trait PoolSession {
    fn base(&self) -> String;
    /// Sends the request `build` makes (with the bearer token added); a 401 `unauthorized` is
    /// refreshed and retried once by the session. The answer comes back whatever its status.
    fn send<'a>(&'a self, build: &'a dyn Fn() -> Request) -> LocalFuture<'a, Result<Response, ApiError>>;
    /// The account signed in, when the session knows it (see [`pinned`]).
    fn account(&self) -> Option<String> {
        None
    }
    /// [`send`](Self::send) for `account` only (critic 73 I2): nothing goes, and the answer is `SignedOut`, once another
    /// account is signed in. A session that retries inside one call (the real [`Client`]'s refresh-and-retry) checks
    /// again before each try.
    fn send_as<'a>(&'a self, account: Option<&'a str>, build: &'a dyn Fn() -> Request) -> LocalFuture<'a, Result<Response, ApiError>> {
        if account.is_some() && self.account().as_deref() != account {
            return Box::pin(async { Err(ApiError::SignedOut) });
        }
        self.send(build)
    }
}

impl PoolSession for Client {
    fn base(&self) -> String {
        Client::base(self).to_string()
    }
    fn send<'a>(&'a self, build: &'a dyn Fn() -> Request) -> LocalFuture<'a, Result<Response, ApiError>> {
        Box::pin(self.authed_raw(build))
    }
    fn send_as<'a>(&'a self, account: Option<&'a str>, build: &'a dyn Fn() -> Request) -> LocalFuture<'a, Result<Response, ApiError>> {
        Box::pin(self.authed_raw_as(account, build))
    }
    fn account(&self) -> Option<String> {
        Client::account(self)
    }
}

/// (critic 71 minor 3) A session for one job, kept to the account signed in when the job started. One `Client` serves
/// every account on this computer and reads the sign-in of whoever is signed in on each try: a job's retry (after a
/// busy server's wait, or a send's later picture uploads) would otherwise go out with the next account's sign-in once
/// another account signed in meanwhile. Once the account is another one, nothing more is sent: the job ends as signed
/// out. A session that does not know its account is never held back.
struct PinnedSession {
    inner: super::publish::SharedSession,
    account: Option<String>,
}

pub fn pinned(inner: super::publish::SharedSession) -> super::publish::SharedSession {
    let account = inner.account();
    std::sync::Arc::new(PinnedSession { inner, account })
}

impl PoolSession for PinnedSession {
    fn base(&self) -> String {
        self.inner.base()
    }
    fn send<'a>(&'a self, build: &'a dyn Fn() -> Request) -> LocalFuture<'a, Result<Response, ApiError>> {
        // (critic 73 I2) the account goes down to the session, which checks it again before each of its own tries
        self.inner.send_as(self.account.as_deref(), build)
    }
    // (critic 75 r05, r06) no `send_as` of its own: no session is pinned twice, so the trait's default (refuse another
    // account, else `send`) is all a pinned session needs.
    fn account(&self) -> Option<String> {
        self.account.clone()
    }
}

/// Where the pictures a payload names are read from.
pub trait MediaSource {
    fn read(&self, file: &str) -> Option<Vec<u8>>;
}

/// The app's media store (`store::read_media`).
pub struct StoreMedia;

impl MediaSource for StoreMedia {
    fn read(&self, file: &str) -> Option<Vec<u8>> {
        crate::store::read_media(file)
    }
}

impl MediaSource for HashMap<String, Vec<u8>> {
    fn read(&self, file: &str) -> Option<Vec<u8>> {
        self.get(file).cloned()
    }
}

// ---- the call plumbing ---------------------------------------------------------------------------

pub(super) enum Body {
    None,
    Json(String),
    Bytes { data: Vec<u8>, content_type: String },
}

pub(super) struct Call {
    pub(super) method: Method,
    pub(super) path: String,
    pub(super) body: Body,
    pub(super) headers: Vec<(String, String)>,
    pub(super) binary: bool,
    pub(super) max_response: Option<usize>,
    pub(super) timeout: Option<Duration>,
    /// Repeating it changes nothing the person would notice.
    pub(super) safe: bool,
}

impl Call {
    pub(super) fn new(method: Method, path: impl Into<String>) -> Call {
        Call { method, path: path.into(), body: Body::None, headers: Vec::new(), binary: false, max_response: None, timeout: None, safe: true }
    }
    pub(super) fn json(mut self, v: &Value) -> Call {
        self.body = Body::Json(v.to_string());
        self
    }
    pub(super) fn not_safe(mut self) -> Call {
        self.safe = false;
        self
    }
    fn request(&self, url: &str) -> Request {
        let mut r = Request { method: self.method, url: url.to_string(), binary: self.binary, max_response: self.max_response, timeout: self.timeout, ..Request::default() };
        match &self.body {
            Body::None => {}
            Body::Json(s) => r.body = Some(Secret::new(s.clone())),
            Body::Bytes { data, content_type } => {
                r.bytes = Some(data.clone());
                r.content_type = Some(content_type.clone());
            }
        }
        for (k, v) in &self.headers {
            r.headers.push((k.clone(), Secret::new(v.clone())));
        }
        r
    }
}

pub(super) fn parse<T: DeserializeOwned>(r: &Response) -> Result<T, PoolError> {
    serde_json::from_str(r.body.expose()).map_err(|e| PoolError::Api(ApiError::BadReply(format!("{e}"))))
}

/// The pool's item routes over a session.
pub struct Pool<'a> {
    session: &'a dyn PoolSession,
    retry: RetryPolicy,
}

impl<'a> Pool<'a> {
    pub fn new(session: &'a dyn PoolSession) -> Pool<'a> {
        Pool { session, retry: RetryPolicy::default() }
    }
    pub fn with_retry(mut self, retry: RetryPolicy) -> Pool<'a> {
        self.retry = retry;
        self
    }

    /// One call with retries. Returns a 2xx (or 304) answer; anything else is an error.
    pub(super) async fn call(&self, c: &Call) -> Result<Response, PoolError> {
        let url = route_url(&self.session.base(), &c.path).ok_or(PoolError::Api(ApiError::NotConfigured))?;
        let build = || c.request(&url);
        let mut attempt = 0u32;
        loop {
            let last = attempt + 1 >= self.retry.attempts.max(1);
            match self.session.send(&build).await {
                Err(ApiError::Offline(why)) => {
                    if c.safe
                        && !last
                        && let Some(w) = self.retry.wait(attempt, None)
                    {
                        sleep(self.retry.scaled(w)).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(PoolError::Api(ApiError::Offline(why)));
                }
                Err(e) => return Err(PoolError::Api(e)),
                Ok(resp) if resp.is_success() || resp.not_modified() => return Ok(resp),
                Ok(resp) => {
                    let err = PoolError::from_response(&resp);
                    if !last
                        && let Some(k) = err.kind()
                        && retryable(k, resp.status, c.safe)
                        && let Some(w) = self.retry.wait(attempt, resp.retry_after)
                    {
                        sleep(self.retry.scaled(w)).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(err);
                }
            }
        }
    }
}

// ---- licences and options -------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum License {
    Cc0,
    CcBy,
    CcBySa,
    CcByNc,
    CcByNcSa,
    /// All rights reserved inside the author's circles; can not be public.
    CircleOnly,
}

impl License {
    pub const ALL: [License; 6] = [License::CcBy, License::Cc0, License::CcBySa, License::CcByNc, License::CcByNcSa, License::CircleOnly];

    /// The wire string (`license.ts` `LICENSES`).
    pub fn as_str(self) -> &'static str {
        match self {
            License::CcBy => "CC-BY-4.0",
            License::Cc0 => "CC0-1.0",
            License::CcBySa => "CC-BY-SA-4.0",
            License::CcByNc => "CC-BY-NC-4.0",
            License::CcByNcSa => "CC-BY-NC-SA-4.0",
            License::CircleOnly => "circle-only",
        }
    }
    pub fn parse(s: &str) -> Option<License> {
        License::ALL.into_iter().find(|l| l.as_str() == s)
    }
}

impl Serialize for License {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Public,
    /// Private: readable only by its author, still republishable.
    Unlisted,
}

impl Visibility {
    pub fn as_str(self) -> &'static str {
        match self {
            Visibility::Public => "public",
            Visibility::Unlisted => "unlisted",
        }
    }
}

/// Listing details (the Worker's `meta`); empty fields are left out.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ItemMeta {
    pub title: String,
    pub subject: String,
    pub course: String,
    pub grade: String,
    pub tags: Vec<String>,
    pub standards: Vec<String>,
}

impl ItemMeta {
    fn to_json(&self) -> Value {
        let mut m = serde_json::Map::new();
        for (k, v) in [("title", &self.title), ("subject", &self.subject), ("course", &self.course), ("grade", &self.grade)] {
            if !v.is_empty() {
                m.insert(k.into(), json!(v));
            }
        }
        if !self.tags.is_empty() {
            m.insert("tags".into(), json!(self.tags));
        }
        if !self.standards.is_empty() {
            m.insert("standards".into(), json!(self.standards));
        }
        Value::Object(m)
    }
}

/// "I wrote these independently of items I only looked at": the items, and the author's own words
/// (10 characters at least; kept in the audit and shown to the admin).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndependentOf {
    pub items: Vec<String>,
    pub statement: String,
}

/// The independence statement for questions the pool found in circle banks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CircleIndependent {
    pub banks: Vec<String>,
    pub statement: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PublishOptions {
    /// Serve students the keyless form (the key stays with the author).
    pub hide_key: bool,
    pub license: License,
    /// Used when the item is new; a republish changes visibility with [`Pool::set_visibility`].
    pub visibility: Visibility,
    pub labels: Vec<LabelDecision>,
    pub order_acks: Vec<Pos>,
    /// Question indices whose low-risk formatting findings the author acknowledged.
    pub ack_formatting: Vec<usize>,
    /// Question indices whose picture "does not reveal the answer".
    pub image_ok: Vec<usize>,
    pub independent_of: Option<IndependentOf>,
    /// "I wrote these questions myself" for the circle banks the pool named (`circle_material`): the
    /// bank ids and the author's own words (10 characters at least).
    pub circle_independent_of: Option<CircleIndependent>,
    pub meta: ItemMeta,
    /// The accepted attestation text (`ATTEST_TEXT_REV`). The person ticks it; this records which.
    pub attestation_rev: String,
}

impl PublishOptions {
    pub fn new(license: License, visibility: Visibility, hide_key: bool) -> PublishOptions {
        PublishOptions {
            hide_key,
            license,
            visibility,
            labels: Vec::new(),
            order_acks: Vec::new(),
            ack_formatting: Vec::new(),
            image_ok: Vec::new(),
            independent_of: None,
            circle_independent_of: None,
            meta: ItemMeta::default(),
            attestation_rev: ATTEST_TEXT_REV.to_string(),
        }
    }

    /// Records a label decision (replacing an earlier one for the same place).
    pub fn decide_label(&mut self, at: Pos, choice: LabelChoice) {
        self.labels.retain(|l| (l.question, l.part) != (at.question, at.part));
        self.labels.push(LabelDecision { question: at.question, part: at.part, choice });
    }

    /// Takes over the order acknowledgements a refusal asked for (after the person confirmed them).
    pub fn acknowledge_orders(&mut self, from: &PayloadRefusal) {
        for p in &from.needs_order_ack {
            if !self.order_acks.contains(p) {
                self.order_acks.push(*p);
            }
        }
    }

    pub fn acknowledge_formatting(&mut self, questions: &[usize]) {
        for q in questions {
            if !self.ack_formatting.contains(q) {
                self.ack_formatting.push(*q);
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublishTarget {
    New,
    /// `PUT /items/:id`, with the publish this edit is based on (the compare-and-set).
    Republish { item_id: String, base_publish_id: String },
}

/// One publish: send it again with the same `publish_id` and it is the same publish (the server
/// answers a committed one from its record with `replayed: true`). Keep the id while the person is
/// still working on the same publish; make a new one when the content or options change.
#[derive(Clone, Debug, PartialEq)]
pub struct PublishRequest {
    pub target: PublishTarget,
    pub publish_id: String,
    /// The payload bytes (`{"bank": ..}` or `{"question": ..}` with `schema_version`), sent as they
    /// are.
    pub payload: Vec<u8>,
    pub options: PublishOptions,
}

impl PublishRequest {
    pub fn new_item(payload: Vec<u8>, options: PublishOptions) -> PublishRequest {
        PublishRequest { target: PublishTarget::New, publish_id: new_publish_id(), payload, options }
    }
    pub fn republish(item_id: &str, base_publish_id: &str, payload: Vec<u8>, options: PublishOptions) -> PublishRequest {
        PublishRequest {
            target: PublishTarget::Republish { item_id: item_id.to_string(), base_publish_id: base_publish_id.to_string() },
            publish_id: new_publish_id(),
            payload,
            options,
        }
    }

    pub fn path(&self) -> String {
        match &self.target {
            PublishTarget::New => "/items".to_string(),
            PublishTarget::Republish { item_id, .. } => format!("/items/{item_id}"),
        }
    }
    /// `POST /items/preview` or `POST /items/:id/preview`: the dry run of this publish.
    pub fn preview_path(&self) -> String {
        match &self.target {
            PublishTarget::New => "/items/preview".to_string(),
            PublishTarget::Republish { item_id, .. } => format!("/items/{item_id}/preview"),
        }
    }
    pub fn method(&self) -> Method {
        match self.target {
            PublishTarget::New => Method::Post,
            PublishTarget::Republish { .. } => Method::Put,
        }
    }

    /// The meta line, checked against what the Worker would refuse with a 400.
    pub fn meta_json(&self) -> Result<String, LocalProblem> {
        if !is_publish_id(&self.publish_id) {
            return Err(LocalProblem::BadPublishId);
        }
        let o = &self.options;
        let mut m = serde_json::Map::new();
        m.insert("publish_id".into(), json!(self.publish_id));
        match &self.target {
            PublishTarget::New => {
                if o.visibility == Visibility::Public && o.license == License::CircleOnly {
                    return Err(LocalProblem::LicenseNotPublic);
                }
                m.insert("visibility".into(), json!(o.visibility.as_str()));
            }
            PublishTarget::Republish { item_id, base_publish_id } => {
                if !is_item_id(item_id) {
                    return Err(LocalProblem::BadId(item_id.clone()));
                }
                if !is_publish_id(base_publish_id) {
                    return Err(LocalProblem::BadPublishId);
                }
                m.insert("base_publish_id".into(), json!(base_publish_id));
            }
        }
        m.insert("hide_key".into(), json!(o.hide_key));
        m.insert("license".into(), json!(o.license.as_str()));
        m.insert("attestation".into(), json!({ "accepted": true, "text_rev": o.attestation_rev }));
        m.insert("attest".into(), json!({ "own_work": true, "terms_version": POOL_TERMS_VERSION }));
        m.insert("labels".into(), json!(o.labels));
        m.insert("order_acks".into(), json!(o.order_acks));
        m.insert("image_ok".into(), json!(o.image_ok));
        m.insert("ack_formatting".into(), json!(o.ack_formatting));
        m.insert("meta".into(), o.meta.to_json());
        if let Some(ind) = &o.independent_of {
            if ind.items.is_empty() || ind.items.len() > 50 || !ind.items.iter().all(|i| is_item_id(i)) {
                return Err(LocalProblem::IndependenceClaim("Name 1 to 50 items you only looked at."));
            }
            if ind.statement.trim().chars().count() < 10 {
                return Err(LocalProblem::IndependenceClaim("Say in a sentence why these questions are your own work (at least 10 characters)."));
            }
            m.insert("independent_of".into(), json!({ "items": ind.items, "accepted": true, "statement": ind.statement.trim() }));
        }
        if let Some(ci) = &o.circle_independent_of {
            if ci.banks.is_empty() || ci.banks.len() > 50 || !ci.banks.iter().all(|b| is_item_id(b)) {
                return Err(LocalProblem::IndependenceClaim("Name the circle banks your questions are independent of."));
            }
            if ci.statement.trim().chars().count() < 10 {
                return Err(LocalProblem::IndependenceClaim("Say in a sentence why these questions are your own work (at least 10 characters)."));
            }
            m.insert("circle_independent_of".into(), json!({ "banks": ci.banks, "accepted": true, "statement": ci.statement.trim() }));
        }
        let s = Value::Object(m).to_string();
        if s.len() > MAX_META {
            return Err(LocalProblem::MetaTooLarge { len: s.len(), limit: MAX_META });
        }
        Ok(s)
    }

    /// `<meta JSON>\n<payload bytes>`.
    pub fn body(&self) -> Result<Vec<u8>, LocalProblem> {
        if self.payload.len() > MAX_PAYLOAD {
            return Err(LocalProblem::PayloadTooLarge { len: self.payload.len(), limit: MAX_PAYLOAD });
        }
        if self.payload.iter().find(|b| !b.is_ascii_whitespace()) != Some(&b'{') {
            return Err(LocalProblem::PayloadNotJson);
        }
        let meta = self.meta_json()?;
        let mut out = Vec::with_capacity(meta.len() + 1 + self.payload.len());
        out.extend_from_slice(meta.as_bytes());
        out.push(b'\n');
        out.extend_from_slice(&self.payload);
        Ok(out)
    }
}

/// 32 hex characters (128 bits) for a publish id. The server uses it as an idempotency key; it is not
/// a secret.
pub fn new_publish_id() -> String {
    let b = entropy();
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn entropy() -> [u8; 16] {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut h = Sha256::new();
    h.update(COUNTER.fetch_add(1, Ordering::Relaxed).to_le_bytes());
    if let Ok(d) = web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH) {
        h.update(d.as_nanos().to_le_bytes());
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::hash::{BuildHasher, Hasher};
        for i in 0..4u64 {
            let mut s = std::collections::hash_map::RandomState::new().build_hasher();
            s.write_u64(i);
            h.update(s.finish().to_le_bytes());
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let mut buf = [0u8; 32];
        if let Some(c) = web_sys::window().and_then(|w| w.crypto().ok()) {
            let _ = c.get_random_values_with_u8_array(&mut buf);
        }
        h.update(buf);
    }
    let d = h.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&d[..16]);
    out
}

// ---- images ----------------------------------------------------------------------------------------

/// A picture a question names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageUse {
    /// Question index in the payload.
    pub position: usize,
    /// `media/<sha256>.<ext>`.
    pub file: String,
    pub sha256: String,
}

/// `media/<sha256>.<png|jpg|gif|webp>` -> its sha. Shared with a circle send's picture fetch (phase D3), so a name
/// the pool accepts can not be read two ways.
pub(super) fn media_sha(file: &str) -> Option<String> {
    let rest = file.strip_prefix("media/")?;
    let (stem, ext) = rest.split_once('.')?;
    (is_sha256(stem) && matches!(ext, "png" | "jpg" | "gif" | "webp")).then(|| stem.to_string())
}

/// The pictures of a payload (questions' `image.file`; the Worker reads nothing else), by question.
pub fn images_in_payload(payload: &[u8]) -> Result<Vec<ImageUse>, LocalProblem> {
    let v: Value = serde_json::from_slice(payload).map_err(|_| LocalProblem::PayloadNotJson)?;
    let questions: Vec<&Value> = match (v.pointer("/bank/questions").and_then(Value::as_array), v.get("question")) {
        (Some(qs), _) => qs.iter().collect(),
        (None, Some(q)) => vec![q],
        _ => return Err(LocalProblem::PayloadNotJson),
    };
    let mut out = Vec::new();
    for (position, q) in questions.into_iter().enumerate() {
        if let Some(file) = q.pointer("/image/file").and_then(Value::as_str) {
            let sha256 = media_sha(file).ok_or_else(|| LocalProblem::BadMediaName { file: file.to_string() })?;
            out.push(ImageUse { position, file: file.to_string(), sha256 });
        }
    }
    Ok(out)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Reads every picture of the list and checks what the Worker would refuse for size and name, so
/// nothing is sent for a publish that can not work. Returns `(use, bytes)` pairs, one per distinct
/// picture.
pub fn load_images(images: &[ImageUse], media: &dyn MediaSource) -> Result<Vec<(ImageUse, Vec<u8>)>, LocalProblem> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for u in images {
        if !seen.insert(u.sha256.clone()) {
            continue;
        }
        let bytes = media.read(&u.file).ok_or_else(|| LocalProblem::MissingMedia { file: u.file.clone() })?;
        if bytes.len() > MAX_IMAGE_BYTES {
            return Err(LocalProblem::MediaTooLarge { file: u.file.clone(), len: bytes.len() });
        }
        if sha256_hex(&bytes) != u.sha256 {
            return Err(LocalProblem::MediaHashMismatch { file: u.file.clone() });
        }
        out.push((u.clone(), bytes));
    }
    Ok(out)
}

/// `PUT /blobs/:sha256` answer.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct BlobInfo {
    pub sha256: String,
    pub size: u64,
    pub width: u32,
    pub height: u32,
}

/// When pictures go up relative to the publish.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlobOrder {
    /// Upload every picture, then publish. Right for a new item.
    First,
    /// Publish; upload only if the server says `blob_missing`, then publish again. Right for a
    /// republish whose pictures are already on the server (the author's own earlier publishes
    /// count), where re-sending 2 MB images is waste.
    OnDemand,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublishProgress {
    UploadingImage { done: usize, total: usize, file: String },
    Sending,
    /// The server lacked pictures; they are being sent again.
    ResendingImages { count: usize },
}

/// A publish answer (`POST /items`, `PUT /items/:id`).
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct PublishResult {
    pub item_id: String,
    pub publish_id: String,
    pub version: u64,
    pub kind: String,
    pub visibility: String,
    pub license: String,
    pub hide_key: bool,
    /// The server's ids of the questions, in order: put them in the local copy's `published`.
    pub qids: Vec<String>,
    pub public_version: u64,
    /// The id of an item the Worker thinks this duplicates, if any.
    pub possible_duplicate_of: Option<String>,
    /// Which served order each shuffled question got. For the author only: it maps the served
    /// choices back to the key. Never show it to anyone else or write it into a log.
    pub served_orders: AuthorOnly,
    pub order_reasons: Vec<OrderNote>,
    pub labels_removed: Vec<Value>,
    pub images_kept: Vec<Value>,
    pub images_removed: Vec<Value>,
    pub figures_removed: Vec<Value>,
    /// Notes on what the server dropped (a message per reason).
    pub dropped: Vec<Value>,
    /// The server answered from its record of this very publish (a retry after a lost answer).
    pub replayed: bool,
}

/// JSON that is for the publishing author only; `Debug` leaves it out.
#[derive(Clone, Default, PartialEq, Deserialize)]
#[serde(transparent)]
pub struct AuthorOnly(pub Value);

impl std::fmt::Debug for AuthorOnly {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AuthorOnly(<author only>)")
    }
}

/// The dry run's answer (`POST /items/preview`).
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct PreviewResult {
    pub ok: bool,
    pub dry_run: bool,
    pub hide_key: bool,
    /// With a hidden key: the exact stripped payload students are served (the choice order is drawn
    /// again at the real publish). Otherwise `{questions, key_shown: true}`.
    pub student_view: Value,
    /// What the server dropped or changed, one note each (`message` when the Worker words it).
    pub notes: Vec<Value>,
    pub label_findings: Vec<LabelFinding>,
    pub labels_removed: Vec<Pos>,
    /// Question indices.
    pub images_removed: Vec<usize>,
    pub figures_removed: Vec<usize>,
    pub order_reasons: Vec<OrderNote>,
    pub license_check: Option<LicenseCheck>,
}

impl<'a> Pool<'a> {
    /// `PUT /blobs/:sha256`. The bytes must hash to `sha256` and fit the Worker's 2 MB.
    pub async fn put_blob(&self, sha256: &str, bytes: &[u8]) -> Result<BlobInfo, PoolError> {
        if !is_sha256(sha256) {
            return Err(LocalProblem::BadSha(sha256.to_string()).into());
        }
        if bytes.is_empty() {
            return Err(LocalProblem::EmptyBlob.into());
        }
        if bytes.len() > MAX_IMAGE_BYTES {
            return Err(LocalProblem::MediaTooLarge { file: sha256.to_string(), len: bytes.len() }.into());
        }
        if sha256_hex(bytes) != sha256 {
            return Err(LocalProblem::BlobHashMismatch.into());
        }
        let mut c = Call::new(Method::Put, format!("/blobs/{sha256}"));
        c.body = Body::Bytes { data: bytes.to_vec(), content_type: "application/octet-stream".into() };
        c.timeout = Some(Duration::from_secs(120));
        parse(&self.call(&c).await?)
    }

    pub(super) async fn upload(&self, imgs: &[(ImageUse, Vec<u8>)], progress: &mut dyn FnMut(PublishProgress)) -> Result<(), PoolError> {
        for (i, (u, bytes)) in imgs.iter().enumerate() {
            progress(PublishProgress::UploadingImage { done: i, total: imgs.len(), file: u.file.clone() });
            self.put_blob(&u.sha256, bytes).await?;
        }
        Ok(())
    }

    async fn send_body(&self, req: &PublishRequest, body: &[u8], dry: bool) -> Result<Response, PoolError> {
        let mut c = if dry { Call::new(Method::Post, req.preview_path()) } else { Call::new(req.method(), req.path()) };
        c.body = Body::Bytes { data: body.to_vec(), content_type: PUBLISH_CONTENT_TYPE.into() };
        c.timeout = Some(Duration::from_secs(180));
        self.call(&c).await
    }

    /// Publishes (or republishes) with its pictures. Everything the Worker would refuse for size,
    /// names or ids is caught first. A `blob_missing` answer (a picture lost on the server) is
    /// answered once by sending the pictures it names again.
    ///
    /// A refusal that needs the person (`invalid_payload`, `labels_need_decision`,
    /// `order_ack_required`, `license_incompatible`, ...) comes back as [`PoolError::Refused`];
    /// fix the options or the content and call again (a new [`PublishRequest`] id when the content
    /// changed).
    pub async fn publish(
        &self,
        req: &PublishRequest,
        media: &dyn MediaSource,
        order: BlobOrder,
        progress: &mut dyn FnMut(PublishProgress),
    ) -> Result<PublishResult, PoolError> {
        parse(&self.run_publish(req, media, order, progress, false).await?)
    }

    /// The dry run of [`Pool::publish`] (`POST /items/preview`): the same checks, the same refusals,
    /// nothing stored and nothing shared. The Worker still needs the pictures to be on the server
    /// (privately, as pending uploads that expire in a day), so they go up first exactly as for a
    /// publish. Counted against the day's `previews`, never the publishes.
    pub async fn preview(
        &self,
        req: &PublishRequest,
        media: &dyn MediaSource,
        order: BlobOrder,
        progress: &mut dyn FnMut(PublishProgress),
    ) -> Result<PreviewResult, PoolError> {
        parse(&self.run_publish(req, media, order, progress, true).await?)
    }

    async fn run_publish(
        &self,
        req: &PublishRequest,
        media: &dyn MediaSource,
        order: BlobOrder,
        progress: &mut dyn FnMut(PublishProgress),
        dry: bool,
    ) -> Result<Response, PoolError> {
        let body = req.body()?;
        let uses = images_in_payload(&req.payload)?;
        let imgs = load_images(&uses, media)?;
        if order == BlobOrder::First {
            self.upload(&imgs, progress).await?;
        }
        let mut resent = false;
        loop {
            progress(PublishProgress::Sending);
            match self.send_body(req, &body, dry).await {
                Err(PoolError::Refused(r)) if !resent && matches!(r.kind, RefusalKind::BlobMissing { .. }) => {
                    resent = true;
                    let RefusalKind::BlobMissing { positions, .. } = &r.kind else { unreachable!() };
                    let wanted: Vec<(ImageUse, Vec<u8>)> = imgs
                        .iter()
                        .filter(|(u, _)| positions.is_empty() || uses.iter().any(|x| x.sha256 == u.sha256 && positions.contains(&x.position)))
                        .cloned()
                        .collect();
                    progress(PublishProgress::ResendingImages { count: wanted.len() });
                    self.upload(&wanted, progress).await?;
                }
                other => return other,
            }
        }
    }

    /// `PATCH /items/:id {visibility}`.
    pub async fn set_visibility(&self, item_id: &str, v: Visibility) -> Result<Visibility, PoolError> {
        check_item_id(item_id)?;
        let c = Call::new(Method::Patch, format!("/items/{item_id}")).json(&json!({ "visibility": v.as_str() }));
        let r: Value = parse(&self.call(&c).await?)?;
        Ok(if r.get("visibility").and_then(Value::as_str) == Some("public") { Visibility::Public } else { Visibility::Unlisted })
    }

    /// `DELETE /items/:id`: hidden at once, undo for 14 days. Returns `undo_until` (ms).
    pub async fn delete_item(&self, item_id: &str) -> Result<Option<i64>, PoolError> {
        check_item_id(item_id)?;
        let r: Value = parse(&self.call(&Call::new(Method::Delete, format!("/items/{item_id}"))).await?)?;
        Ok(r.get("undo_until").and_then(Value::as_i64))
    }

    /// `POST /items/:id/undelete`.
    pub async fn undelete_item(&self, item_id: &str) -> Result<(), PoolError> {
        check_item_id(item_id)?;
        self.call(&Call::new(Method::Post, format!("/items/{item_id}/undelete"))).await?;
        Ok(())
    }
}

fn check_item_id(id: &str) -> Result<(), PoolError> {
    if is_item_id(id) { Ok(()) } else { Err(LocalProblem::BadId(id.to_string()).into()) }
}

// ---- browse -----------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scope {
    #[default]
    Public,
    /// The author's own items (with their visibility).
    Mine,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BrowseQuery {
    pub scope: Scope,
    /// Every word starts a word of the title, subject, a tag or the question text as students see it.
    pub q: Option<String>,
    pub subject: Option<String>,
    /// `Mc`, `Tf`, `Matching`, `Blank`, `Short`, `Multipart`.
    pub kind: Option<String>,
    pub tag: Option<String>,
    pub standard: Option<String>,
    pub license: Option<String>,
    /// 1 to 50 (the Worker's cap); 20 when `None`.
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

impl BrowseQuery {
    /// The query string, checked against the Worker's bounds.
    pub fn to_path(&self) -> Result<String, LocalProblem> {
        let mut s = url::form_urlencoded::Serializer::new(String::new());
        if self.scope == Scope::Mine {
            s.append_pair("scope", "mine");
        }
        let mut field = |name: &'static str, key: &'static str, v: &Option<String>, max: usize| -> Result<(), LocalProblem> {
            if let Some(v) = v {
                if v.trim().is_empty() || v.chars().count() > max {
                    return Err(LocalProblem::BadField(name));
                }
                s.append_pair(key, v.trim());
            }
            Ok(())
        };
        field("search text", "q", &self.q, 100)?;
        field("subject", "subject", &self.subject, 100)?;
        field("kind", "kind", &self.kind, 16)?;
        field("tag", "tag", &self.tag, 64)?;
        field("standard", "standard", &self.standard, 64)?;
        field("licence", "license", &self.license, 32)?;
        field("page cursor", "cursor", &self.cursor, 1024)?;
        if let Some(l) = self.limit {
            if l == 0 {
                return Err(LocalProblem::BadField("page size"));
            }
            s.append_pair("limit", &l.min(50).to_string());
        }
        let qs = s.finish();
        Ok(if qs.is_empty() { "/items".into() } else { format!("/items?{qs}") })
    }
}

/// One row of a browse page.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct ListItem {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub subject: String,
    pub course: String,
    pub grade: String,
    pub kinds: Vec<String>,
    pub question_count: u32,
    pub version: u64,
    pub hide_key: bool,
    pub license: String,
    pub license_check: Option<LicenseCheck>,
    pub has_images: bool,
    pub author_name: String,
    pub committed_at: i64,
    /// Own items only.
    pub visibility: Option<String>,
    pub is_author: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct BrowsePage {
    pub items: Vec<ListItem>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LicenseStatus {
    Ok,
    Mismatch,
    Unverified,
    ClaimedIndependent,
    UnverifiedEarlierMatch,
    #[default]
    #[serde(other)]
    Other,
}

/// Whether an item's licence is consistent with the sources the server recorded for it.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct LicenseCheck {
    pub status: LicenseStatus,
    pub basis: String,
    pub positions: Vec<LicensePos>,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct LicensePos {
    pub pos: usize,
    pub allowed: Vec<String>,
}

impl<'a> Pool<'a> {
    /// `GET /items?...`
    pub async fn browse(&self, q: &BrowseQuery) -> Result<BrowsePage, PoolError> {
        let c = Call::new(Method::Get, q.to_path()?);
        parse(&self.call(&c).await?)
    }
}

// ---- one item: read, import, images ------------------------------------------------------------------

/// The listing details of an item as it is served to the caller.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct ItemView {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub subject: String,
    pub course: String,
    pub grade: String,
    pub kinds: Vec<String>,
    pub question_count: u32,
    pub version: u64,
    /// The publish this version is (a republish's `base_publish_id` is the author's own).
    pub publish_id: String,
    pub license: String,
    pub hide_key: bool,
    pub has_images: bool,
    pub author_name: String,
    pub committed_at: i64,
    /// `full` or `stripped`: the form served to the caller.
    pub form: String,
    pub is_author: bool,
    pub visibility: Option<String>,
    pub tags: Vec<String>,
    pub standards: Vec<String>,
}

/// One earlier contributor of a question.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct ChainLink {
    pub author_name: String,
    pub item_id: String,
    pub qid: String,
    pub version: u64,
    pub license: String,
    pub modified: bool,
    /// On the placeholder "and N contributors between": the licences of those left out.
    pub collapsed_licenses: Vec<String>,
    pub possible_source: bool,
    pub earlier_match: bool,
}

impl ChainLink {
    /// The "and N contributors between" stand-in (no item).
    pub fn is_placeholder(&self) -> bool {
        self.item_id.is_empty() && self.author_name.starts_with("and ")
    }
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct QuestionProv {
    pub pos: usize,
    pub qid: String,
    /// Hash of the served form of this question (the stripped form for a hidden key).
    pub served_sha256: Option<String>,
    pub match_hash: String,
    pub chain: Vec<ChainLink>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct ItemHead {
    pub item: ItemView,
    pub questions: Vec<QuestionProv>,
    pub license_check: Option<LicenseCheck>,
}

/// What an import adds to the head.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct ImportInfo {
    pub license: String,
    /// Licences this account may publish a verbatim copy under.
    pub may_republish_as: Vec<String>,
    pub previously_imported_versions: Vec<u64>,
    pub images: ImageHold,
    /// The server recorded the import (an author's own copy is not recorded).
    pub recorded: bool,
}

/// Pictures held for the importer for 30 days (`reason: "quota"` when they did not fit).
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct ImageHold {
    pub held: u64,
    pub until: Option<i64>,
    pub reason: Option<String>,
}

/// An item as served: the head, the payload bytes exactly as the server stored them, and for an
/// import the extra record.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemDoc {
    pub head: ItemHead,
    pub import: Option<ImportInfo>,
    /// The payload JSON, unparsed and unchanged.
    pub payload: Vec<u8>,
    pub etag: Option<String>,
    /// Came from [`ItemCache`] after a 304.
    pub from_cache: bool,
}

/// Splits `{<head fields>,"payload":<bytes>}` without parsing (and so without changing) the payload.
pub fn split_item(body: &[u8]) -> Result<(Value, &[u8]), LocalProblem> {
    const MARK: &[u8] = b"\"payload\":";
    let at = body.windows(MARK.len()).position(|w| w == MARK).ok_or(LocalProblem::BadItemShape)?;
    let mut head = body[..at].to_vec();
    while head.last().is_some_and(|b| b.is_ascii_whitespace() || *b == b',') {
        head.pop();
    }
    head.push(b'}');
    let payload = body[at + MARK.len()..].strip_suffix(b"}").ok_or(LocalProblem::BadItemShape)?;
    let head: Value = serde_json::from_slice(&head).map_err(|_| LocalProblem::BadItemShape)?;
    Ok((head, payload))
}

fn item_doc(r: &Response) -> Result<ItemDoc, PoolError> {
    let (head, payload) = split_item(&r.bytes)?;
    let import = head.get("may_republish_as").is_some().then(|| serde_json::from_value::<ImportInfo>(head.clone())).transpose().map_err(|e| ApiError::BadReply(e.to_string()))?;
    let parsed: ItemHead = serde_json::from_value(head).map_err(|e| ApiError::BadReply(e.to_string()))?;
    Ok(ItemDoc { head: parsed, import, payload: payload.to_vec(), etag: r.etag.clone(), from_cache: false })
}

/// `HEAD /items/:id`: the stamp of what would be served, nothing recorded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemStamp {
    pub etag: Option<String>,
    pub content_length: Option<u64>,
}

/// Items already read, by id, with their `ETag`, so a second look is a `304`. Bounded in bytes; the
/// oldest entry goes first.
#[derive(Debug, Default)]
pub struct ItemCache {
    map: HashMap<String, ItemDoc>,
    order: Vec<String>,
    bytes: usize,
    max_bytes: usize,
}

impl ItemCache {
    pub fn new(max_bytes: usize) -> ItemCache {
        ItemCache { max_bytes, ..ItemCache::default() }
    }
    pub fn get(&self, id: &str) -> Option<&ItemDoc> {
        self.map.get(id)
    }
    pub fn len(&self) -> usize {
        self.map.len()
    }
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
    pub fn remove(&mut self, id: &str) {
        if let Some(d) = self.map.remove(id) {
            self.bytes = self.bytes.saturating_sub(d.payload.len());
            self.order.retain(|x| x != id);
        }
    }
    pub fn clear(&mut self) {
        *self = ItemCache::new(self.max_bytes);
    }
    fn put(&mut self, id: &str, doc: ItemDoc) {
        self.remove(id);
        if doc.etag.is_none() || doc.payload.len() > self.max_bytes {
            return;
        }
        self.bytes += doc.payload.len();
        self.order.push(id.to_string());
        self.map.insert(id.to_string(), doc);
        while self.bytes > self.max_bytes && !self.order.is_empty() {
            let old = self.order[0].clone();
            self.remove(&old);
        }
    }
}

/// A downloaded picture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Blob {
    pub bytes: Vec<u8>,
    pub content_type: Option<String>,
}

impl<'a> Pool<'a> {
    /// `GET /items/:id`, with `If-None-Match` when `cache` holds the item. The read is recorded
    /// by the server as a look (evidence for the licence rules); use [`Pool::item_stamp`] to
    /// check for change without it.
    pub async fn read_item(&self, id: &str, cache: Option<&mut ItemCache>) -> Result<ItemDoc, PoolError> {
        check_item_id(id)?;
        let mut c = Call::new(Method::Get, format!("/items/{id}"));
        c.binary = true;
        c.max_response = Some(MAX_ITEM_RESPONSE);
        c.timeout = Some(Duration::from_secs(120));
        let cached = cache.as_ref().and_then(|c| c.get(id)).and_then(|d| d.etag.clone());
        if let Some(e) = &cached {
            c.headers.push(("If-None-Match".into(), e.clone()));
        }
        let r = self.call(&c).await?;
        if r.not_modified() {
            return match cache.as_ref().and_then(|c| c.get(id)) {
                Some(d) => Ok(ItemDoc { from_cache: true, ..d.clone() }),
                None => Err(PoolError::Api(ApiError::BadReply("304 without a cached copy".into()))),
            };
        }
        let doc = item_doc(&r)?;
        if let Some(cache) = cache {
            cache.put(id, doc.clone());
        }
        Ok(doc)
    }

    /// `HEAD /items/:id`.
    pub async fn item_stamp(&self, id: &str) -> Result<ItemStamp, PoolError> {
        check_item_id(id)?;
        let r = self.call(&Call::new(Method::Head, format!("/items/{id}"))).await?;
        Ok(ItemStamp { etag: r.etag.clone(), content_length: None })
    }

    /// `POST /items/:id/import`: the copy to keep. The answer is the head (`license_check`,
    /// `may_republish_as`, per-question `chain`), the payload bytes, and the record
    /// ([`ItemDoc::import`]). The served pictures are held for the importer for 30 days; fetch them
    /// with [`Pool::get_blob`] and store them before they lapse.
    pub async fn import_item(&self, id: &str) -> Result<ItemDoc, PoolError> {
        check_item_id(id)?;
        let mut c = Call::new(Method::Post, format!("/items/{id}/import"));
        c.binary = true;
        c.max_response = Some(MAX_ITEM_RESPONSE);
        c.timeout = Some(Duration::from_secs(120));
        let doc = item_doc(&self.call(&c).await?)?;
        if doc.import.is_none() {
            return Err(PoolError::Api(ApiError::BadReply("an import answer without may_republish_as".into())));
        }
        Ok(doc)
    }

    /// `GET /items/:id/blobs/:sha256`. The bytes are checked against the name.
    pub async fn get_blob(&self, item_id: &str, sha256: &str) -> Result<Blob, PoolError> {
        check_item_id(item_id)?;
        if !is_sha256(sha256) {
            return Err(LocalProblem::BadSha(sha256.to_string()).into());
        }
        let mut c = Call::new(Method::Get, format!("/items/{item_id}/blobs/{sha256}"));
        c.binary = true;
        c.max_response = Some(MAX_IMAGE_BYTES + 1024);
        c.timeout = Some(Duration::from_secs(120));
        let r = self.call(&c).await?;
        if sha256_hex(&r.bytes) != sha256 {
            return Err(LocalProblem::BlobHashMismatch.into());
        }
        Ok(Blob { bytes: r.bytes, content_type: r.content_type })
    }
}

// ---- versions and mine --------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum VersionState {
    /// Nothing newer.
    Current { version: u64, license: Option<String>, license_version: Option<u64> },
    /// A newer version of an item the account imported.
    Newer { version: u64, license: Option<String>, license_version: Option<u64> },
    /// Held, deleted within its undo window, taken down, or under privacy review: keep the copy.
    /// `reason`: `held`, `privacy`, `takedown`, `admin`, `deleted`.
    UnderReview { reason: String, until: Option<i64> },
    /// Gone for good.
    Removed { reason: String },
    Unavailable,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VersionEntry {
    pub id: String,
    pub state: VersionState,
}

fn version_entry(v: &Value) -> Option<VersionEntry> {
    let id = str_of(v, "id")?;
    let ver = u64_of(v, "version").unwrap_or(0);
    let state = match v.get("status").and_then(Value::as_str)? {
        "current" => VersionState::Current { version: ver, license: str_of(v, "license"), license_version: u64_of(v, "license_version") },
        "newer" => VersionState::Newer { version: ver, license: str_of(v, "license"), license_version: u64_of(v, "license_version") },
        "under_review" => VersionState::UnderReview { reason: str_of(v, "reason").unwrap_or_default(), until: v.get("until").and_then(Value::as_i64) },
        "removed" => VersionState::Removed { reason: str_of(v, "reason").unwrap_or_default() },
        _ => VersionState::Unavailable,
    };
    Some(VersionEntry { id, state })
}

/// Why an item of `mine` is in the state it is in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemState {
    /// Others can see it (if public).
    Live,
    /// Deleted by the author; `undo_until` says until when it can come back.
    Deleted,
    /// Copyright takedown; see `counter_notice`.
    TakenDown,
    /// Removed because it held student data; `appeal_until`.
    RemovedPrivacy,
    /// Removed by an admin; `appeal` is `available`.
    Removed,
    /// A picture was lost on the server: upload it again and republish.
    NeedsReupload,
    /// Held for review (a report, a block, a check): `appeal` is `available`; `hold_class` says more
    /// for admin-placed holds.
    HeldForReview,
    /// Teachers reported that what students see gives the hidden key away; ends with a new version.
    KeyLeakReview,
    /// A takedown notice arrived for an item the author had already deleted: no content, only the
    /// counter-notice dates (`counter_notice_by`).
    TakedownPending,
    #[default]
    #[serde(other)]
    Other,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct MineItem {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub subject: String,
    pub version: u64,
    pub visibility: String,
    pub license: String,
    pub hide_key: bool,
    pub question_count: u32,
    pub committed_at: Option<i64>,
    /// How many other teachers imported it.
    pub import_count: u64,
    pub state: ItemState,
    pub undo_until: Option<i64>,
    /// `received`, `available` or `closed` (taken-down items).
    pub counter_notice: Option<String>,
    pub notice: Option<String>,
    /// `available` when the author can appeal.
    pub appeal: Option<String>,
    pub appeal_until: Option<i64>,
    pub removed_reason: Option<String>,
    /// `student_data` or `copyright` for a hold an admin placed on an existing item.
    pub hold_class: Option<String>,
    /// The questions the hold names.
    pub positions: Vec<usize>,
    /// The takedown a counter-notice answers (taken-down and pending items).
    pub action_id: Option<String>,
    /// A counter-notice was already sent for a pending takedown.
    pub counter_notice_sent: bool,
    /// For a pending takedown: when the 30-day window ends (ms).
    pub counter_notice_by: Option<i64>,
}

impl MineItem {
    /// Not visible to others now (held, removed, deleted, taken down).
    pub fn is_hidden(&self) -> bool {
        !matches!(self.state, ItemState::Live)
    }
    /// Private: published but readable only by its author.
    pub fn is_unlisted(&self) -> bool {
        self.visibility == "unlisted"
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Storage {
    pub used: u64,
    pub limit: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct MineList {
    pub items: Vec<MineItem>,
    pub storage: Storage,
    /// Set when there is another page (`GET /items/mine?after=...`).
    pub next_cursor: Option<String>,
}

/// The answer to a redemption.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InviteAck {
    /// The account already held a redemption: nothing was used up.
    pub already: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Prefs {
    /// What happens to my shared items when I delete my account; `None`: no choice made yet.
    pub keep_on_delete: Option<bool>,
}

impl<'a> Pool<'a> {
    /// `POST /items/versions`: where each item stands, 200 ids a call (more are split).
    pub async fn versions(&self, ids: &[String]) -> Result<Vec<VersionEntry>, PoolError> {
        let mut out = Vec::new();
        for chunk in ids.chunks(MAX_VERSION_IDS) {
            if let Some(bad) = chunk.iter().find(|i| i.len() > 64 || i.is_empty()) {
                return Err(LocalProblem::BadId(bad.clone()).into());
            }
            let r: Value = parse(&self.call(&Call::new(Method::Post, "/items/versions").json(&json!({ "ids": chunk }))).await?)?;
            out.extend(r.get("items").and_then(Value::as_array).into_iter().flatten().filter_map(version_entry));
        }
        Ok(out)
    }

    /// `POST /me/invite {code}`: redeems an admin's invite code. Open to a pending account. The code
    /// is sent as typed (the Worker ignores case, spaces and hyphens) and is kept nowhere. A second
    /// redemption is `already`, not an error.
    pub async fn redeem_invite(&self, code: &str) -> Result<InviteAck, PoolError> {
        let c = code.trim();
        if c.is_empty() || c.chars().count() > 100 || c.chars().any(char::is_control) {
            return Err(LocalProblem::BadField("invite code").into());
        }
        let r: Value = parse(&self.call(&Call::new(Method::Post, "/me/invite").json(&json!({ "code": c })).not_safe()).await?)?;
        Ok(InviteAck { already: r.get("already").and_then(Value::as_bool).unwrap_or(false) })
    }

    /// `GET /items/mine`.
    pub async fn mine(&self) -> Result<MineList, PoolError> {
        self.mine_page(None).await
    }

    /// One page of `GET /items/mine` (newest first); `after` is a previous `next_cursor`.
    pub async fn mine_page(&self, after: Option<&str>) -> Result<MineList, PoolError> {
        let path = match after {
            Some(a) if a.len() <= 64 && a.chars().all(|c| c.is_ascii_hexdigit() || c == ':') => format!("/items/mine?after={a}"),
            Some(_) => return Err(LocalProblem::BadField("cursor").into()),
            None => "/items/mine".to_string(),
        };
        parse(&self.call(&Call::new(Method::Get, path)).await?)
    }

    /// `GET /me/export`: the whole export as bytes (never logged). When the answer is a manifest
    /// of parts (`export_too_large`), fetch each with [`Pool::export_part`].
    pub async fn export_account(&self) -> Result<Vec<u8>, PoolError> {
        let mut c = Call::new(Method::Get, "/me/export");
        c.binary = true;
        c.max_response = Some(MAX_EXPORT_RESPONSE);
        c.timeout = Some(Duration::from_secs(120));
        Ok(self.call(&c).await?.bytes)
    }

    /// `GET /me/export?part=n` (parts do not count against the daily allowance).
    pub async fn export_part(&self, n: u32) -> Result<Vec<u8>, PoolError> {
        let mut c = Call::new(Method::Get, format!("/me/export?part={n}"));
        c.binary = true;
        c.max_response = Some(MAX_EXPORT_RESPONSE);
        c.timeout = Some(Duration::from_secs(120));
        Ok(self.call(&c).await?.bytes)
    }

    pub async fn get_prefs(&self) -> Result<Prefs, PoolError> {
        let r: Value = parse(&self.call(&Call::new(Method::Get, "/items/prefs")).await?)?;
        Ok(Prefs { keep_on_delete: r.get("keep_on_delete").and_then(Value::as_bool) })
    }

    pub async fn set_prefs(&self, keep_on_delete: bool) -> Result<(), PoolError> {
        self.call(&Call::new(Method::Put, "/items/prefs").json(&json!({ "keep_on_delete": keep_on_delete }))).await?;
        Ok(())
    }
}

// ---- reports, appeals, counter-notices -------------------------------------------------------------------

/// The reasons the Worker takes (`REPORT_REASONS`). `KeyLeak` ("the answer key is visible") is only
/// for an item published with its key hidden: the Worker answers 400 `invalid_request` (field
/// `reason`) for any other, allows one such report per item per 30 days and five a day (429
/// `daily_limit`, metric `key_leak_reports`), and only importers who were served that version
/// count towards a hold. A student-data report holds the item at once; the others need three distinct teachers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ReportReason {
    WrongAnswer,
    Copyright,
    StudentData,
    Inappropriate,
    Other,
    KeyLeak,
}

impl ReportReason {
    pub const ALL: [ReportReason; 6] =
        [ReportReason::WrongAnswer, ReportReason::Copyright, ReportReason::StudentData, ReportReason::Inappropriate, ReportReason::Other, ReportReason::KeyLeak];

    pub fn as_str(self) -> &'static str {
        match self {
            ReportReason::WrongAnswer => "wrong_answer",
            ReportReason::Copyright => "copyright",
            ReportReason::StudentData => "student_data",
            ReportReason::Inappropriate => "inappropriate",
            ReportReason::Other => "other",
            ReportReason::KeyLeak => "key_leak",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            ReportReason::WrongAnswer => "The answer is wrong",
            ReportReason::Copyright => "This is copyrighted material",
            ReportReason::StudentData => "This contains student information",
            ReportReason::Inappropriate => "Inappropriate content",
            ReportReason::Other => "Something else",
            ReportReason::KeyLeak => "The answer key is visible",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Ack {
    pub ok: bool,
    pub message: Option<String>,
}

/// The fields of a DMCA 512(g)(3) counter-notice the author must fill and tick.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CounterNotice {
    pub signature: String,
    pub material_desc: String,
    pub name: String,
    pub address: String,
    pub phone: String,
    pub perjury_stmt: bool,
    pub consent_jurisdiction: bool,
    pub accept_service: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct CounterNoticeAck {
    pub restore_not_before: i64,
    pub restore_by: i64,
}

impl<'a> Pool<'a> {
    /// `POST /items/:id/report`. The answer is always thanks.
    pub async fn report(&self, item_id: &str, reason: ReportReason, note: Option<&str>) -> Result<Ack, PoolError> {
        check_item_id(item_id)?;
        let mut body = json!({ "reason": reason.as_str() });
        if let Some(n) = note.map(str::trim).filter(|n| !n.is_empty()) {
            body["note"] = json!(n.chars().take(MAX_REPORT_NOTE).collect::<String>());
        }
        // A second copy would be a second report: only the answers that say "not processed" retry.
        parse(&self.call(&Call::new(Method::Post, format!("/items/{item_id}/report")).json(&body).not_safe()).await?)
    }

    /// `POST /items/:id/appeal`: the author of an admin-removed, held or privacy-removed item asks
    /// for a review. 1 to 500 characters.
    pub async fn appeal(&self, item_id: &str, note: &str) -> Result<Ack, PoolError> {
        check_item_id(item_id)?;
        let n = note.trim();
        if n.is_empty() || n.chars().count() > 500 {
            return Err(LocalProblem::BadField("appeal note").into());
        }
        parse(&self.call(&Call::new(Method::Post, format!("/items/{item_id}/appeal")).json(&json!({ "note": n })).not_safe()).await?)
    }

    /// `POST /items/:id/counter-notice`.
    pub async fn counter_notice(&self, item_id: &str, n: &CounterNotice) -> Result<CounterNoticeAck, PoolError> {
        check_item_id(item_id)?;
        let body = json!({
            "signature": n.signature.trim(), "material_desc": n.material_desc.trim(), "name": n.name.trim(),
            "address": n.address.trim(), "phone": n.phone.trim(), "perjury_stmt": n.perjury_stmt,
            "consent_jurisdiction": n.consent_jurisdiction, "accept_service": n.accept_service,
        });
        parse(&self.call(&Call::new(Method::Post, format!("/items/{item_id}/counter-notice")).json(&body).not_safe()).await?)
    }
}

// ---- the student view -----------------------------------------------------------------------------------

/// Which form of a payload is being summarised.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Served {
    /// What students are served when the key is hidden: no key anywhere.
    Stripped,
    /// The whole thing, key included (a publish without "hide key" serves this to everyone).
    Full,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StudentChoice {
    pub text: String,
    /// The choice keeps its place when the order is shuffled.
    pub locked: bool,
    /// `Some` only in the [`Served::Full`] form.
    pub correct: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StudentBody {
    Mc { choices: Vec<StudentChoice> },
    /// In the full form, `answer` is the key.
    TrueFalse { answer: Option<bool> },
    /// The left items in the order they are served and the word bank (the right items, shuffled);
    /// the full form also lists the pairs.
    Matching { left: Vec<String>, word_bank: Vec<String>, pairs: Vec<(String, String)> },
    Blank { blanks: usize, accepted: Vec<Vec<String>> },
    Short { sample: Option<String> },
    Multipart { parts: Vec<StudentPart> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct StudentPart {
    pub stem: String,
    pub body: StudentBody,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StudentQuestion {
    /// 1-based.
    pub number: usize,
    pub stem: String,
    pub topics: Vec<String>,
    pub points: Option<f64>,
    pub has_image: bool,
    pub has_figure: bool,
    pub body: StudentBody,
    /// The full form only.
    pub explanation: Option<String>,
}

/// A reason not to trust a "stripped" payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewWarning {
    /// Key material is in a stripped payload (`what`: `correct choice`, `matching pair`, `accepted
    /// answer`, `sample answer`, `explanation`). Publishing it with the key hidden would be a bug.
    KeyDataPresent { question: usize, what: &'static str },
    /// A question of a stripped payload is not marked keyless.
    NotMarkedKeyless { question: usize },
}

/// The pre-publish confirmation: exactly what students will see.
#[derive(Clone, Debug, PartialEq)]
pub struct StudentView {
    pub form: Served,
    pub name: Option<String>,
    pub description: Option<String>,
    pub questions: Vec<StudentQuestion>,
    pub warnings: Vec<ViewWarning>,
}

impl StudentView {
    /// True when nothing in the payload contradicts "this is the keyless form". The UI should not
    /// offer to publish a hidden-key item while this is false.
    pub fn is_clean(&self) -> bool {
        self.warnings.is_empty()
    }

    /// The summary of a stripped payload (share-core's `stripped.payload`), or of the full payload
    /// for a publish that shows its key.
    pub fn from_payload(payload: &[u8], form: Served) -> Result<StudentView, LocalProblem> {
        let v: Value = serde_json::from_slice(payload).map_err(|_| LocalProblem::PayloadNotJson)?;
        let (name, description, qs): (Option<String>, Option<String>, Vec<&Value>) = if let Some(b) = v.get("bank") {
            let qs = b.get("questions").and_then(Value::as_array).ok_or(LocalProblem::PayloadNotJson)?;
            (str_of(b, "name"), b.pointer("/details/description").and_then(Value::as_str).map(str::to_string), qs.iter().collect())
        } else if let Some(q) = v.get("question") {
            (None, None, vec![q])
        } else {
            return Err(LocalProblem::PayloadNotJson);
        };
        let mut warnings = Vec::new();
        let mut questions = Vec::new();
        for (i, q) in qs.into_iter().enumerate() {
            let full = form == Served::Full;
            if !full && q.get("keyless").and_then(Value::as_bool) != Some(true) {
                warnings.push(ViewWarning::NotMarkedKeyless { question: i });
            }
            let mut warn = |what: &'static str| warnings.push(ViewWarning::KeyDataPresent { question: i, what });
            let explanation = q.get("explanation").and_then(Value::as_str).map(str::to_string);
            if !full && q.get("explanation").is_some_and(|e| !e.is_null()) {
                warn("explanation");
            }
            let body = q.get("kind").map(|k| body_of(k, full, &mut warn)).unwrap_or(StudentBody::Short { sample: None });
            questions.push(StudentQuestion {
                number: i + 1,
                stem: str_of(q, "stem").unwrap_or_default(),
                topics: strings(q, "topics"),
                points: q.get("points").and_then(Value::as_f64),
                has_image: q.get("image").is_some_and(|x| !x.is_null()),
                has_figure: q.get("figure").is_some_and(|x| !x.is_null()),
                body,
                explanation: if full { explanation } else { None },
            });
        }
        Ok(StudentView { form, name, description, questions, warnings })
    }
}

fn answers_of(v: &Value, full: bool, warn: &mut dyn FnMut(&'static str)) -> Vec<StudentChoice> {
    v.get("answers")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|x| {
                    let correct = x.get("correct").and_then(Value::as_bool).unwrap_or(false);
                    if !full && correct {
                        warn("correct choice");
                    }
                    StudentChoice { text: str_of(x, "text").unwrap_or_default(), locked: x.get("locked").and_then(Value::as_bool).unwrap_or(false), correct: full.then_some(correct) }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `kind` is the externally tagged enum of the payload: `{"Mc": {...}}`, `{"Tf": {"correct": b}}`, ...
fn body_of(kind: &Value, full: bool, warn: &mut dyn FnMut(&'static str)) -> StudentBody {
    let Some((tag, inner)) = kind.as_object().and_then(|o| o.iter().next()) else { return StudentBody::Short { sample: None } };
    match tag.as_str() {
        "Mc" => StudentBody::Mc { choices: answers_of(inner, full, warn) },
        "Tf" => StudentBody::TrueFalse { answer: full.then(|| inner.get("correct").and_then(Value::as_bool).unwrap_or(false)) },
        "Matching" => {
            let pairs = inner.get("pairs").and_then(Value::as_array).cloned().unwrap_or_default();
            let left = pairs.iter().map(|p| str_of(p, "left").unwrap_or_default()).collect();
            if !full && pairs.iter().any(|p| p.get("right").and_then(Value::as_str).is_some_and(|r| !r.is_empty())) {
                warn("matching pair");
            }
            StudentBody::Matching {
                left,
                word_bank: strings(inner, "distractors"),
                pairs: if full { pairs.iter().map(|p| (str_of(p, "left").unwrap_or_default(), str_of(p, "right").unwrap_or_default())).collect() } else { Vec::new() },
            }
        }
        "Blank" => {
            let accept: Vec<Vec<String>> = inner
                .get("accept")
                .and_then(Value::as_array)
                .map(|a| a.iter().map(|x| x.as_array().map(|y| y.iter().filter_map(|s| s.as_str().map(str::to_string)).collect()).unwrap_or_default()).collect())
                .unwrap_or_default();
            if !full && accept.iter().any(|a| !a.is_empty()) {
                warn("accepted answer");
            }
            StudentBody::Blank { blanks: accept.len(), accepted: if full { accept } else { Vec::new() } }
        }
        "Short" => {
            let sample = str_of(inner, "sample").unwrap_or_default();
            if !full && !sample.is_empty() {
                warn("sample answer");
            }
            StudentBody::Short { sample: full.then_some(sample) }
        }
        "Multipart" => StudentBody::Multipart {
            parts: inner
                .get("parts")
                .and_then(Value::as_array)
                .map(|ps| ps.iter().map(|p| StudentPart { stem: str_of(p, "stem").unwrap_or_default(), body: p.get("kind").map(|k| body_of(k, full, warn)).unwrap_or(StudentBody::Short { sample: None }) }).collect())
                .unwrap_or_default(),
        },
        _ => StudentBody::Short { sample: None },
    }
}
