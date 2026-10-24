//! The circle routes (`server/worker/README.md`, rounds 32 and 33) over a [`Pool`]: typed answers, no
//! UI. A circle is a small private group of teachers who share banks with each other.
//!
//! Every text the server sends is kept raw here and drawn through [`super::api::plain`] by the UI. A
//! circle code is a [`Secret`]: it is shown once, never logged and not in `Debug`.

use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use super::api::ApiError;
use super::pool::*;
use super::secret::Secret;
use super::transport::Method;

// ---- ids and bounds ------------------------------------------------------------------------------

pub const NAME_MAX: usize = 80;
pub const DESCRIPTION_MAX: usize = 1000;
pub const INVITE_NOTE_MAX: usize = 200;
pub const MESSAGE_MAX: usize = 500;
/// `POST /circles/heads` takes at most this many banks.
pub const MAX_HEADS: usize = 200;

fn check_id(id: &str) -> Result<(), PoolError> {
    if is_item_id(id) { Ok(()) } else { Err(LocalProblem::BadId(id.to_string()).into()) }
}

/// A circle code as someone may paste it: `C-` and 24 letters and digits in groups, or a link or a
/// sentence holding one. Returns the code in its plain form, or `None`.
pub fn code_in(s: &str) -> Option<String> {
    // Look for "C-" followed by 24 Crockford characters, with spaces, dashes and underscores between.
    let up: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i + 1 < up.len() {
        if (up[i] == 'C' || up[i] == 'c') && up[i + 1] == '-' && (i == 0 || !up[i - 1].is_ascii_alphanumeric()) {
            let mut out = String::from("C-");
            let mut n = 0;
            let mut j = i + 2;
            while j < up.len() && n < 24 {
                let c = up[j];
                if c.is_ascii_alphanumeric() {
                    out.push(c.to_ascii_uppercase());
                    n += 1;
                } else if !(c == '-' || c == ' ' || c == '_') {
                    break;
                }
                j += 1;
            }
            if n == 24 && !(j < up.len() && up[j].is_ascii_alphanumeric()) {
                return Some(out);
            }
        }
        i += 1;
    }
    None
}

/// A code in groups of four for reading aloud: `C-ABCD-EFGH-...`.
/// The server answers with the code already grouped (`C-AT6S-A85Q-...`), `code_in` with it bare.
pub fn code_groups(code: &str) -> String {
    let body: Vec<char> = code.strip_prefix("C-").unwrap_or(code).chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let groups: Vec<String> = body.chunks(4).map(|c| c.iter().collect()).collect();
    format!("C-{}", groups.join("-"))
}

// ---- roles -----------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Owner,
    Editor,
    Viewer,
}

impl Role {
    pub fn parse(s: &str) -> Role {
        match s {
            "owner" => Role::Owner,
            "editor" => Role::Editor,
            _ => Role::Viewer,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Owner => "owner",
            Role::Editor => "editor",
            Role::Viewer => "viewer",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Role::Owner => "owner",
            Role::Editor => "editor",
            Role::Viewer => "viewer",
        }
    }
    /// What the role may do, in one phrase.
    pub fn can(self) -> &'static str {
        match self {
            Role::Owner => "manages the circle and adds banks",
            Role::Editor => "adds banks",
            Role::Viewer => "can look at and copy banks",
        }
    }
    pub fn can_add_banks(self) -> bool {
        matches!(self, Role::Owner | Role::Editor)
    }
}

// ---- answers ------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct CircleSummary {
    pub id: String,
    pub name: String,
    pub role: String,
    pub joined_at: i64,
    pub closed_at: Option<i64>,
    pub close_reason: Option<String>,
    pub undo_until: Option<i64>,
    pub updated_at: i64,
    pub members: u32,
    pub banks: u32,
    /// Set when the owner offered this circle to me: until when I can accept.
    pub ownership_offered_until: Option<i64>,
}

impl CircleSummary {
    pub fn role(&self) -> Role {
        Role::parse(&self.role)
    }
    pub fn closed(&self) -> bool {
        self.closed_at.is_some()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct CircleInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub role: String,
    pub joined_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub closed_at: Option<i64>,
    pub close_reason: Option<String>,
    pub undo_until: Option<i64>,
}

impl CircleInfo {
    pub fn role(&self) -> Role {
        Role::parse(&self.role)
    }
    pub fn closed(&self) -> bool {
        self.closed_at.is_some()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Member {
    pub user_id: String,
    pub display_name: String,
    pub role: String,
    pub joined_at: i64,
    pub is_me: bool,
}

impl Member {
    pub fn role(&self) -> Role {
        Role::parse(&self.role)
    }
}

/// `removed: {reason, undo_until}` on a bank only its owner and author see.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct BankRemoved {
    pub reason: Option<String>,
    pub undo_until: Option<i64>,
}

/// A text the pool may send as `null` (a masked bank): read as empty.
fn null_text<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(d)?.unwrap_or_default())
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct BankEntry {
    pub id: String,
    #[serde(deserialize_with = "null_text")]
    pub title: String,
    #[serde(deserialize_with = "null_text")]
    pub subject: String,
    #[serde(deserialize_with = "null_text")]
    pub course: String,
    #[serde(deserialize_with = "null_text")]
    pub grade: String,
    pub license: String,
    #[serde(deserialize_with = "null_text")]
    pub author_name: String,
    /// A removed bank whose text the owner may not see while it is under review (or its author is not
    /// in good standing): listed without title, subject, course, grade or author.
    pub hidden: bool,
    pub is_author: bool,
    pub head_version: u64,
    pub question_count: u32,
    pub has_images: bool,
    pub created_at: i64,
    pub updated_at: i64,
    /// Only the author sees a held bank; the value says nothing more than "held".
    pub held: Value,
    pub removed: Option<BankRemoved>,
}

impl BankEntry {
    pub fn is_held(&self) -> bool {
        !matches!(self.held, Value::Null | Value::Bool(false))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct CircleDetail {
    pub circle: CircleInfo,
    pub members: Vec<Member>,
    pub banks: Vec<BankEntry>,
    /// An ownership offer, shown only to the owner and the member offered it.
    pub pending_transfer: Option<PendingTransfer>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct PendingTransfer {
    pub to_user_id: String,
    pub to_name: String,
    pub expires_at: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Invite {
    pub invite_id: String,
    pub role: String,
    pub created_at: i64,
    pub created_by: String,
    pub created_by_name: String,
    pub max_uses: u32,
    pub uses: u32,
    pub expires_at: i64,
    pub revoked_at: Option<i64>,
    pub note: Option<String>,
    /// `active`, `expired`, `used_up` or `revoked`.
    pub state: String,
}

impl Invite {
    pub fn active(&self) -> bool {
        self.state == "active"
    }
    pub fn role(&self) -> Role {
        Role::parse(&self.role)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InvitePage {
    pub invites: Vec<Invite>,
    pub next_cursor: Option<String>,
}

/// A code just made. Shown once: the server never gives it again.
#[derive(Clone, PartialEq)]
pub struct InviteCreated {
    pub invite_id: String,
    pub code: Secret,
    pub role: Role,
    pub max_uses: u32,
    pub expires_at: i64,
    pub note: Option<String>,
}

impl std::fmt::Debug for InviteCreated {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "InviteCreated({}, <code>)", self.invite_id)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct PreviewCircle {
    pub id: String,
    pub name: String,
    pub owner_name: String,
    pub members: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct JoinPreview {
    pub circle: PreviewCircle,
    pub role: String,
    pub already_member: bool,
}

impl JoinPreview {
    pub fn role(&self) -> Role {
        Role::parse(&self.role)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Joined {
    pub circle_id: String,
    pub role: String,
    pub already: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct ActivityEntry {
    pub at: i64,
    pub what: String,
    pub by: Option<String>,
    pub member: Option<String>,
    pub role: Option<String>,
    pub bank_id: Option<String>,
    pub bank_title: Option<String>,
    pub version: Option<u64>,
    pub license: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActivityPage {
    pub entries: Vec<ActivityEntry>,
    pub next_before: Option<String>,
}

/// The head of a circle bank as it was read.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct BankHead {
    pub id: String,
    pub title: String,
    pub subject: String,
    pub course: String,
    pub grade: String,
    pub license: String,
    pub version_license: String,
    pub author_name: String,
    pub is_author: bool,
    pub version: u64,
    pub version_id: String,
    pub question_count: u32,
    pub has_images: bool,
    pub committed_at: i64,
    pub committed_by: Option<String>,
    pub held: Value,
}

/// A bank as read: the head, the questions exactly as the server stored them, where they came from
/// (credits). Viewing and copying are this one read.
#[derive(Clone, Debug, PartialEq)]
pub struct BankDoc {
    pub circle_id: String,
    pub circle_name: String,
    pub bank: BankHead,
    pub origins: Value,
    pub payload: Vec<u8>,
    pub etag: Option<String>,
}

/// Where one bank stands for the person who has a copy.
#[derive(Clone, Debug, PartialEq)]
pub enum HeadState {
    /// The newest version, when it was sent and by whom.
    Current { version: u64, license: Option<String>, at: Option<i64>, by: Option<String> },
    Removed { until: Option<i64> },
    UnderReview,
    /// Left, removed, or the circle closed.
    Unavailable,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeadEntry {
    pub bank_id: String,
    pub state: HeadState,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct SentBank {
    pub circle_id: String,
    pub bank_id: String,
    pub version: u64,
    pub version_id: String,
    pub qids: Vec<String>,
    pub added: u32,
    pub changed: u32,
    pub removed: u32,
    pub replayed: bool,
}

/// What a person types to send a bank to a circle.
#[derive(Clone, Debug, PartialEq)]
pub struct CircleSend {
    /// Idempotency key: the same id and body is the same send (22 to 64 characters of `A-Za-z0-9_-`).
    pub commit_id: String,
    pub license: License,
    pub message: String,
    pub meta: ItemMeta,
    pub payload: Vec<u8>,
    pub attestation: bool,
}

impl CircleSend {
    pub fn new(payload: Vec<u8>, license: License, meta: ItemMeta) -> CircleSend {
        CircleSend { commit_id: new_publish_id(), license, message: String::new(), meta, payload, attestation: true }
    }

    pub fn meta_json(&self) -> Result<String, LocalProblem> {
        if !is_publish_id(&self.commit_id) {
            return Err(LocalProblem::BadPublishId);
        }
        let mut m = serde_json::Map::new();
        m.insert("commit_id".into(), json!(self.commit_id));
        m.insert("license".into(), json!(self.license.as_str()));
        m.insert("attest".into(), json!({ "own_work": true, "terms_version": POOL_TERMS_VERSION }));
        let msg = self.message.trim();
        if msg.chars().count() > MESSAGE_MAX {
            return Err(LocalProblem::BadField("message"));
        }
        if !msg.is_empty() {
            m.insert("message".into(), json!(msg));
        }
        m.insert("meta".into(), self.meta_value());
        Ok(Value::Object(m).to_string())
    }

    fn meta_value(&self) -> Value {
        let mut m = serde_json::Map::new();
        for (k, v) in [("title", &self.meta.title), ("subject", &self.meta.subject), ("course", &self.meta.course), ("grade", &self.meta.grade)] {
            if !v.is_empty() {
                m.insert(k.into(), json!(v));
            }
        }
        if !self.meta.tags.is_empty() {
            m.insert("tags".into(), json!(self.meta.tags));
        }
        if !self.meta.standards.is_empty() {
            m.insert("standards".into(), json!(self.meta.standards));
        }
        Value::Object(m)
    }

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

// ---- the routes ----------------------------------------------------------------------------------------------

fn obj_str(v: &Value, k: &str) -> Option<String> {
    str_of(v, k)
}

fn text_arg(what: &'static str, s: &str, max: usize, required: bool) -> Result<String, PoolError> {
    let t = s.trim();
    if (required && t.is_empty()) || t.chars().count() > max {
        return Err(LocalProblem::BadField(what).into());
    }
    Ok(t.to_string())
}

impl<'a> Pool<'a> {
    /// `GET /circles`.
    pub async fn circles(&self) -> Result<Vec<CircleSummary>, PoolError> {
        let v: Value = parse(&self.call(&Call::new(Method::Get, "/circles")).await?)?;
        Ok(serde_json::from_value(v.get("circles").cloned().unwrap_or(Value::Null)).unwrap_or_default())
    }

    /// `POST /circles`: the creator becomes the owner.
    pub async fn create_circle(&self, name: &str, description: &str) -> Result<CircleInfo, PoolError> {
        let mut body = json!({ "name": text_arg("circle name", name, NAME_MAX, true)? });
        let d = text_arg("description", description, DESCRIPTION_MAX, false)?;
        if !d.is_empty() {
            body["description"] = json!(d);
        }
        let v: Value = parse(&self.call(&Call::new(Method::Post, "/circles").json(&body).not_safe()).await?)?;
        Ok(serde_json::from_value(v.get("circle").cloned().unwrap_or(v)).unwrap_or_default())
    }

    /// `GET /circles/:cid`.
    pub async fn circle(&self, cid: &str) -> Result<CircleDetail, PoolError> {
        check_id(cid)?;
        parse(&self.call(&Call::new(Method::Get, format!("/circles/{cid}"))).await?)
    }

    /// `PATCH /circles/:cid`: the owner renames it or changes its description.
    pub async fn update_circle(&self, cid: &str, name: Option<&str>, description: Option<&str>) -> Result<(), PoolError> {
        check_id(cid)?;
        let mut body = serde_json::Map::new();
        if let Some(n) = name {
            body.insert("name".into(), json!(text_arg("circle name", n, NAME_MAX, true)?));
        }
        if let Some(d) = description {
            body.insert("description".into(), json!(text_arg("description", d, DESCRIPTION_MAX, false)?));
        }
        self.call(&Call::new(Method::Patch, format!("/circles/{cid}")).json(&Value::Object(body)).not_safe()).await?;
        Ok(())
    }

    /// `DELETE /circles/:cid`: closed with 14 days to undo. Returns `undo_until`.
    pub async fn close_circle(&self, cid: &str) -> Result<Option<i64>, PoolError> {
        check_id(cid)?;
        let v: Value = parse(&self.call(&Call::new(Method::Delete, format!("/circles/{cid}")).not_safe()).await?)?;
        Ok(v.get("undo_until").and_then(Value::as_i64))
    }

    /// `POST /circles/:cid/reopen`.
    pub async fn reopen_circle(&self, cid: &str) -> Result<(), PoolError> {
        check_id(cid)?;
        self.call(&Call::new(Method::Post, format!("/circles/{cid}/reopen")).not_safe()).await?;
        Ok(())
    }

    /// `GET /circles/:cid/activity`.
    pub async fn circle_activity(&self, cid: &str, before: Option<&str>) -> Result<ActivityPage, PoolError> {
        check_id(cid)?;
        let mut path = format!("/circles/{cid}/activity");
        if let Some(b) = before.filter(|b| !b.is_empty()) {
            if !b.bytes().all(|c| c.is_ascii_alphanumeric() || b"-_.:".contains(&c)) || b.len() > 100 {
                return Err(LocalProblem::BadField("cursor").into());
            }
            path.push_str(&format!("?before={b}"));
        }
        let v: Value = parse(&self.call(&Call::new(Method::Get, path)).await?)?;
        let entries = serde_json::from_value(v.get("activity").cloned().unwrap_or(Value::Null)).unwrap_or_default();
        let next_before = match v.get("next_before") {
            Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
            Some(Value::Number(n)) => Some(n.to_string()),
            _ => None,
        };
        Ok(ActivityPage { entries, next_before })
    }

    /// `POST /circles/:cid/invites`: the code comes back once.
    pub async fn create_invite(&self, cid: &str, role: Role, max_uses: u32, expires_in_days: u32, note: &str) -> Result<InviteCreated, PoolError> {
        check_id(cid)?;
        if role == Role::Owner {
            return Err(LocalProblem::BadField("role").into());
        }
        if !(1..=50).contains(&max_uses) {
            return Err(LocalProblem::BadField("number of people").into());
        }
        if !(1..=30).contains(&expires_in_days) {
            return Err(LocalProblem::BadField("number of days").into());
        }
        let mut body = json!({ "role": role.as_str(), "max_uses": max_uses, "expires_in_days": expires_in_days });
        let n = text_arg("note", note, INVITE_NOTE_MAX, false)?;
        if !n.is_empty() {
            body["note"] = json!(n);
        }
        let v: Value = parse(&self.call(&Call::new(Method::Post, format!("/circles/{cid}/invites")).json(&body).not_safe()).await?)?;
        let code = obj_str(&v, "code").ok_or_else(|| PoolError::Api(ApiError::BadReply("a circle code answer without a code".into())))?;
        Ok(InviteCreated {
            invite_id: obj_str(&v, "invite_id").unwrap_or_default(),
            code: Secret::new(code),
            role: Role::parse(&obj_str(&v, "role").unwrap_or_default()),
            max_uses: u64_of(&v, "max_uses").unwrap_or(1) as u32,
            expires_at: v.get("expires_at").and_then(Value::as_i64).unwrap_or(0),
            note: obj_str(&v, "note").filter(|n| !n.is_empty()),
        })
    }

    /// `GET /circles/:cid/invites`: never the codes themselves.
    pub async fn invites(&self, cid: &str, after: Option<&str>) -> Result<InvitePage, PoolError> {
        check_id(cid)?;
        let mut path = format!("/circles/{cid}/invites");
        if let Some(a) = after.filter(|a| !a.is_empty()) {
            if !a.bytes().all(|c| c.is_ascii_alphanumeric() || b"-_.:".contains(&c)) || a.len() > 100 {
                return Err(LocalProblem::BadField("cursor").into());
            }
            path.push_str(&format!("?after={a}"));
        }
        let v: Value = parse(&self.call(&Call::new(Method::Get, path)).await?)?;
        let invites = serde_json::from_value(v.get("invites").cloned().unwrap_or(Value::Null)).unwrap_or_default();
        let next_cursor = match v.get("next_cursor") {
            Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
            Some(Value::Number(n)) => Some(n.to_string()),
            _ => None,
        };
        Ok(InvitePage { invites, next_cursor })
    }

    /// `POST /circles/:cid/invites/revoke`.
    pub async fn revoke_invite(&self, cid: &str, invite_id: &str) -> Result<(), PoolError> {
        check_id(cid)?;
        if invite_id.is_empty() || invite_id.len() > 64 || !invite_id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_') {
            return Err(LocalProblem::BadField("code").into());
        }
        self.call(&Call::new(Method::Post, format!("/circles/{cid}/invites/revoke")).json(&json!({ "invite_id": invite_id })).not_safe()).await?;
        Ok(())
    }

    /// `POST /circles/join/preview`: what a code is for. Uses nothing.
    pub async fn join_preview(&self, code: &str) -> Result<JoinPreview, PoolError> {
        let code = code_in(code).ok_or(LocalProblem::BadField("circle code"))?;
        parse(&self.call(&Call::new(Method::Post, "/circles/join/preview").json(&json!({ "code": code })).not_safe()).await?)
    }

    /// `POST /circles/join`.
    pub async fn join_circle(&self, code: &str) -> Result<Joined, PoolError> {
        let code = code_in(code).ok_or(LocalProblem::BadField("circle code"))?;
        parse(&self.call(&Call::new(Method::Post, "/circles/join").json(&json!({ "code": code })).not_safe()).await?)
    }

    /// `POST /circles/:cid/leave`.
    pub async fn leave_circle(&self, cid: &str) -> Result<(), PoolError> {
        check_id(cid)?;
        self.call(&Call::new(Method::Post, format!("/circles/{cid}/leave")).not_safe()).await?;
        Ok(())
    }

    /// `POST /circles/:cid/members/remove`.
    pub async fn remove_member(&self, cid: &str, user_id: &str) -> Result<(), PoolError> {
        check_id(cid)?;
        check_user(user_id)?;
        self.call(&Call::new(Method::Post, format!("/circles/{cid}/members/remove")).json(&json!({ "user_id": user_id })).not_safe()).await?;
        Ok(())
    }

    /// `POST /circles/:cid/members/role`.
    pub async fn set_member_role(&self, cid: &str, user_id: &str, role: Role) -> Result<bool, PoolError> {
        check_id(cid)?;
        check_user(user_id)?;
        if role == Role::Owner {
            return Err(LocalProblem::BadField("role").into());
        }
        let v: Value = parse(&self.call(&Call::new(Method::Post, format!("/circles/{cid}/members/role")).json(&json!({ "user_id": user_id, "role": role.as_str() })).not_safe()).await?)?;
        Ok(v.get("changed").and_then(Value::as_bool).unwrap_or(true))
    }

    /// `POST /circles/:cid/transfer`: an OFFER (202). The member accepts within 7 days; a new offer
    /// replaces the pending one.
    pub async fn offer_ownership(&self, cid: &str, user_id: &str) -> Result<PendingTransfer, PoolError> {
        check_id(cid)?;
        check_user(user_id)?;
        let v: Value = parse(&self.call(&Call::new(Method::Post, format!("/circles/{cid}/transfer")).json(&json!({ "user_id": user_id })).not_safe()).await?)?;
        Ok(serde_json::from_value(v.get("pending").cloned().unwrap_or(Value::Null)).unwrap_or_default())
    }

    /// `POST /circles/:cid/transfer/accept`: the offered member takes the circle.
    pub async fn accept_ownership(&self, cid: &str) -> Result<(), PoolError> {
        check_id(cid)?;
        self.call(&Call::new(Method::Post, format!("/circles/{cid}/transfer/accept")).not_safe()).await?;
        Ok(())
    }

    /// `POST /circles/:cid/transfer/cancel`: the owner withdraws the offer, or the member declines it.
    pub async fn cancel_ownership_offer(&self, cid: &str) -> Result<(), PoolError> {
        check_id(cid)?;
        self.call(&Call::new(Method::Post, format!("/circles/{cid}/transfer/cancel")).not_safe()).await?;
        Ok(())
    }

    /// `GET /circles/:cid/banks/:bid`: look at it, or copy it (the same read).
    pub async fn circle_bank(&self, cid: &str, bid: &str) -> Result<BankDoc, PoolError> {
        check_id(cid)?;
        check_id(bid)?;
        let mut c = Call::new(Method::Get, format!("/circles/{cid}/banks/{bid}"));
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

    /// `GET /circles/:cid/banks/:bid/blobs/:sha256`; the bytes are checked against the name.
    pub async fn circle_blob(&self, cid: &str, bid: &str, sha256: &str) -> Result<Blob, PoolError> {
        check_id(cid)?;
        check_id(bid)?;
        if !is_sha256(sha256) {
            return Err(LocalProblem::BadSha(sha256.to_string()).into());
        }
        let mut c = Call::new(Method::Get, format!("/circles/{cid}/banks/{bid}/blobs/{sha256}"));
        c.binary = true;
        c.max_response = Some(MAX_IMAGE_BYTES + 1024);
        c.timeout = Some(Duration::from_secs(120));
        let r = self.call(&c).await?;
        if sha256_hex(&r.bytes) != sha256 {
            return Err(LocalProblem::BlobHashMismatch.into());
        }
        Ok(Blob { bytes: r.bytes, content_type: r.content_type })
    }

    /// `POST /circles/heads`: where each bank stands (up to 200).
    pub async fn circle_heads(&self, bank_ids: &[String]) -> Result<Vec<HeadEntry>, PoolError> {
        if bank_ids.is_empty() || bank_ids.len() > MAX_HEADS {
            return Err(LocalProblem::BadField("list of banks").into());
        }
        for b in bank_ids {
            check_id(b)?;
        }
        let v: Value = parse(&self.call(&Call::new(Method::Post, "/circles/heads").json(&json!({ "bank_ids": bank_ids }))).await?)?;
        let list = v.get("banks").and_then(Value::as_array).cloned().unwrap_or_default();
        Ok(list
            .iter()
            .filter_map(|e| {
                let bank_id = obj_str(e, "bank_id")?;
                let state = match e.get("status").and_then(Value::as_str)? {
                    "current" => HeadState::Current { version: u64_of(e, "version").unwrap_or(0), license: obj_str(e, "license"), at: e.get("at").and_then(Value::as_i64), by: obj_str(e, "by") },
                    "removed" => HeadState::Removed { until: e.get("until").and_then(Value::as_i64) },
                    "under_review" => HeadState::UnderReview,
                    _ => HeadState::Unavailable,
                };
                Some(HeadEntry { bank_id, state })
            })
            .collect())
    }

    /// `PATCH /circles/:cid/banks/:bid {license}`: the bank author's re-share setting.
    pub async fn set_bank_license(&self, cid: &str, bid: &str, license: License) -> Result<(), PoolError> {
        check_id(cid)?;
        check_id(bid)?;
        self.call(&Call::new(Method::Patch, format!("/circles/{cid}/banks/{bid}")).json(&json!({ "license": license.as_str() })).not_safe()).await?;
        Ok(())
    }

    /// `DELETE /circles/:cid/banks/:bid`: gone for the circle at once, 14 days to undo.
    pub async fn remove_circle_bank(&self, cid: &str, bid: &str) -> Result<Option<i64>, PoolError> {
        check_id(cid)?;
        check_id(bid)?;
        let v: Value = parse(&self.call(&Call::new(Method::Delete, format!("/circles/{cid}/banks/{bid}")).not_safe()).await?)?;
        Ok(v.get("undo_until").and_then(Value::as_i64))
    }

    /// `POST /circles/:cid/banks/:bid/purge`: the owner deletes a removed bank for good, now.
    pub async fn purge_circle_bank(&self, cid: &str, bid: &str) -> Result<(), PoolError> {
        check_id(cid)?;
        check_id(bid)?;
        self.call(&Call::new(Method::Post, format!("/circles/{cid}/banks/{bid}/purge")).not_safe()).await?;
        Ok(())
    }

    /// `POST /circles/:cid/banks/:bid/undelete`.
    pub async fn restore_circle_bank(&self, cid: &str, bid: &str) -> Result<(), PoolError> {
        check_id(cid)?;
        check_id(bid)?;
        self.call(&Call::new(Method::Post, format!("/circles/{cid}/banks/{bid}/undelete")).not_safe()).await?;
        Ok(())
    }

    /// `POST /circles/:cid/banks/:bid/report`: always answered with thanks.
    pub async fn report_circle_bank(&self, cid: &str, bid: &str, reason: &str, note: &str) -> Result<(), PoolError> {
        check_id(cid)?;
        check_id(bid)?;
        if !["student_data", "copyright", "inappropriate", "other"].contains(&reason) {
            return Err(LocalProblem::BadField("reason").into());
        }
        let mut body = json!({ "reason": reason });
        let n = note.trim();
        if !n.is_empty() {
            body["note"] = json!(n.chars().take(MAX_REPORT_NOTE).collect::<String>());
        }
        self.call(&Call::new(Method::Post, format!("/circles/{cid}/banks/{bid}/report")).json(&body).not_safe()).await?;
        Ok(())
    }

    /// `POST /circles/:cid/banks`: the first version of a new bank. Pictures go up first, as for a
    /// publish; a `blob_missing` answer sends them once more.
    pub async fn send_new_circle_bank(
        &self,
        cid: &str,
        req: &CircleSend,
        media: &dyn MediaSource,
        progress: &mut dyn FnMut(PublishProgress),
    ) -> Result<SentBank, PoolError> {
        check_id(cid)?;
        let body = req.body()?;
        let uses = images_in_payload(&req.payload)?;
        let imgs = load_images(&uses, media)?;
        self.upload(&imgs, progress).await?;
        let mut resent = false;
        loop {
            progress(PublishProgress::Sending);
            let mut c = Call::new(Method::Post, format!("/circles/{cid}/banks"));
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
}

fn check_user(id: &str) -> Result<(), PoolError> {
    // Account ids are opaque; keep to a conservative alphabet and length.
    if !id.is_empty() && id.len() <= 64 && id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_') {
        Ok(())
    } else {
        Err(LocalProblem::BadId(id.chars().take(8).collect()).into())
    }
}
