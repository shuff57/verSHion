//! Working copies: a local bank linked to a circle bank. The link is kept in the side file
//! `share-circles.json` (never in `banks.json`), so a bank file shared or exported carries nothing of
//! it. No egui and no network here: the record, the payload to send, and the count of what changed.
//!
//! A link holds what a three-way merge needs: the version the copy was last in step with
//! (`base_version`), what each linked question was then (`base`, bounded like an import's), and which
//! local question is which circle question (`qids`: ids only, so a send keeps every question's
//! identity even when the snapshot was too big to keep).

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::browse::theirs_of;
use super::circles_api::{BankHead, SentBank};
use super::merge::{self, BaseQuestion, Theirs};
use super::pool::{ItemHead, ItemMeta, ItemView, License, is_item_id};
use super::publish::QidLink;
use crate::model::{Question, QuestionBank};

pub const LINKS_DOC: &str = "share-circles.json";

/// The send payload's schema: 2 is the one that carries `published.qid`.
pub const SEND_SCHEMA: u32 = 2;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CircleLink {
    pub circle_id: String,
    #[serde(default)]
    pub circle_name: String,
    pub circle_bank_id: String,
    pub local_bank_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub course: String,
    #[serde(default)]
    pub grade: String,
    /// The bank's licence (wire form): a send goes with it; only the bank's author changes it.
    #[serde(default)]
    pub license: String,
    /// The version this copy was last in step with, and that version's id.
    pub base_version: u64,
    #[serde(default)]
    pub base_version_id: String,
    #[serde(default)]
    pub qids: Vec<QidLink>,
    /// Empty when too big to keep: then a merge reads the base from the circle, or is two-way.
    #[serde(default)]
    pub base: Vec<BaseQuestion>,
    /// The newest version the last quiet check saw, and who sent it and when.
    #[serde(default)]
    pub head_seen: u64,
    #[serde(default)]
    pub head_seen_at: Option<i64>,
    #[serde(default)]
    pub head_seen_by: Option<String>,
    pub linked_at: i64,
    /// The local bank's `modified` when this link was last written with the bank (a copy, a merge, a
    /// send). A bank OLDER than this was put back from a saved copy: the link is ahead of it.
    #[serde(default)]
    pub bank_modified: i64,
    /// A fingerprint of the bank's questions as the app last wrote them with this link. A bank whose
    /// content is the same is never "put back"; one that is older AND different is.
    #[serde(default)]
    pub bank_print: String,
    /// Phase D1: `edit` (the owner's and editors' link: send and get the latest) or `follow` (a viewer's: get the
    /// latest only). Every link written before phase D is an edit link.
    #[serde(default)]
    pub mode: LinkMode,
    /// Made from the viewers' form of a bank that hides its answer keys: never sendable, whatever the role.
    #[serde(default)]
    pub viewers_form: bool,
    /// The person's role in the circle at the last check (`owner`, `editor`, `viewer`), or `none` when the
    /// circle no longer lists them (they left or were removed, or it is gone). Not checked yet: `None`.
    #[serde(default)]
    pub role_seen: Option<String>,
    #[serde(default)]
    pub role_seen_at: Option<i64>,
    /// Whose role `role_seen` is: a short fingerprint of the account that saw it (never the address itself), so
    /// another account's check on this computer does not overwrite it (critic 63 I1). `None`: not known.
    #[serde(default)]
    pub role_seen_by: Option<String>,
    /// The person is the bank's author: the history offers restore and trim.
    #[serde(default)]
    pub is_author: bool,
    /// `is_author` is known (from the read the link was made from, a check, or the circle's page). A link written
    /// before phase D does not know it: its first check learns it (critic 63 I5).
    #[serde(default)]
    pub author_known: bool,
    /// What the last check said about the bank and the circle, so the badge and the strip say it without asking.
    #[serde(default)]
    pub seen: BankSeen,
    /// (critic 74 I1, critic 75 minor 5) The last send from this copy went through, but its answer could not be read,
    /// so the link could not move on to the version it made: kept in the file, so that after a restart the changes it
    /// carried are still not counted as unsent and Send is not offered into a conflict with that version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sent_unread: Option<SentUnread>,
    /// (critic 79 I2, critic 80 I1) The last send from this copy whose answer did not say what it made (no clear answer
    /// came, or one with no version this app can read): the version it was based on and what it carried. Kept in the
    /// file, so that after a restart too a newer version is told apart from the person's own send, and Get latest
    /// matches the circle's questions to what was sent, even to a question changed since. Dropped once a send's answer
    /// is read or the copy catches up with the circle. (critic 81 minor 2) One this build can not read is dropped alone,
    /// never the link with it.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "record_or_none")]
    pub sent_record: Option<SentRecord>,
    /// (critic 89 I1) The base version whose difficulties were learnt from the circle's own copy of it. `None`: a link an
    /// earlier build wrote (the phase C build kept no difficulty, the round 12 build filled them from the bank), read once
    /// from the circle and learnt over what is there; `Some(0)`: a link this build wrote, its difficulties known as written.
    /// A base whose version was read is not read again, even where that version lacks a question (critic 89 m4).
    #[serde(default)]
    pub difficulty_read: Option<u64>,
    /// (the owner's decision, 2026-10-26; critic 90 I1) Difficulties were just learnt from the circle's version: a rating
    /// the circle has there that the copy lacks, on a question the teacher has not edited since the copy last matched, is
    /// inherited into the copy (see [`inherit_difficulty`]). Kept until a pass finds nothing left to inherit.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub inherit: bool,
}

fn record_or_none<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<SentRecord>, D::Error> {
    Ok(serde_json::from_value(Value::deserialize(d)?).ok())
}

/// (critic 79 I2, critic 80 I1) What a send whose answer did not say what it made carried (see `CircleLink::sent_record`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SentRecord {
    /// A short fingerprint of the account that sent it (see `bank_links::account_tag`); empty when not known.
    pub by: String,
    /// The version the send was based on: a newest version above it may be this send, one at it is not.
    pub base: u64,
    /// A colleague's version came right after `base`: the send did not go through (it could only be refused now).
    /// (critic 81 minor 5: the circle found still at `base` is not proof; a slow commit may land after the check.)
    pub not_through: bool,
    /// The bank's `modified` when it was sent (see `SentUnread::modified`).
    pub modified: i64,
    /// The bank's questions as they were sent (none when too big to keep, like a base).
    pub questions: Vec<SentQuestion>,
}

/// One question of such a send, and the circle's id it went with (empty: new to the circle).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SentQuestion {
    #[serde(default)]
    pub qid: String,
    pub q: Question,
}

impl PartialEq for SentQuestion {
    fn eq(&self, o: &Self) -> bool {
        self.qid == o.qid && serde_json::to_value(&self.q).ok() == serde_json::to_value(&o.q).ok()
    }
}

/// The record of a send from `bank` (linked by `link`) based on version `base`, by the account `by`.
pub fn sent_record(link: &CircleLink, bank: &QuestionBank, base: u64, by: String) -> SentRecord {
    let questions: Vec<SentQuestion> = bank.questions.iter().map(|q| SentQuestion { qid: link.qid_of(&q.id).unwrap_or_default().to_string(), q: q.clone() }).collect();
    let small = questions.len() <= merge::BASE_MAX_QUESTIONS && serde_json::to_vec(&questions).is_ok_and(|b| b.len() <= merge::BASE_MAX_BYTES);
    SentRecord { by, base, not_through: false, modified: bank.modified, questions: if small { questions } else { Vec::new() } }
}

/// (critic 75 minor 5, critic 76 I1) A send whose answer could not be read: who sent it, and the bank as it was then.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SentUnread {
    /// A short fingerprint of the account that sent it (see `bank_links::account_tag`); empty when not known. Another
    /// account signed in on this computer is not told.
    pub by: String,
    /// The bank's `modified` when it was sent: a bank changed after it has changes that were not sent.
    pub modified: i64,
}

/// How a bank in My Banks follows its circle bank. Read leniently: a missing value is `Edit` (a link written
/// before phase D), and a value this build does not know is `Follow`, which never sends.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LinkMode {
    #[default]
    Edit,
    Follow,
}

impl Serialize for LinkMode {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(match self {
            LinkMode::Edit => "edit",
            LinkMode::Follow => "follow",
        })
    }
}

impl<'de> Deserialize<'de> for LinkMode {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<LinkMode, D::Error> {
        let v = Value::deserialize(d)?;
        Ok(if v.as_str() == Some("edit") { LinkMode::Edit } else { LinkMode::Follow })
    }
}

/// What the last check said about the circle bank and its circle. Missing or unknown fields are "nothing known".
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BankSeen {
    /// The bank was removed from the circle; until when it can be brought back, when the pool said.
    pub removed: bool,
    pub removed_until: Option<i64>,
    /// The bank is held back for review (the pool's `under_review`).
    pub under_review: bool,
    /// The circle is closed.
    pub circle_closed: bool,
    /// The bank hides its answer keys from viewers (a viewer is served the viewers' form).
    pub hide_key: bool,
}

/// `role_seen` when the circle no longer lists the person.
pub const ROLE_NONE: &str = "none";

impl CircleLink {
    /// Linked from a finished copy of a circle bank (`b` is the head it was made from).
    pub fn from_head(circle_id: &str, circle_name: &str, b: &BankHead, local_bank_id: &str, base: Vec<BaseQuestion>, now: i64) -> CircleLink {
        let qids = qids_of(&base);
        CircleLink {
            circle_id: circle_id.to_string(),
            circle_name: circle_name.to_string(),
            circle_bank_id: b.id.clone(),
            local_bank_id: local_bank_id.to_string(),
            title: b.title.clone(),
            subject: b.subject.clone(),
            course: b.course.clone(),
            grade: b.grade.clone(),
            license: if b.license.is_empty() { b.version_license.clone() } else { b.license.clone() },
            base_version: b.version,
            base_version_id: b.version_id.clone(),
            qids,
            base: merge::bounded(base),
            head_seen: b.version,
            head_seen_at: Some(b.committed_at).filter(|a| *a > 0),
            head_seen_by: b.committed_by.clone(),
            linked_at: now,
            bank_modified: 0,
            bank_print: String::new(),
            mode: LinkMode::Edit,
            viewers_form: false,
            role_seen: None,
            role_seen_at: None,
            role_seen_by: None,
            is_author: b.is_author,
            // (the read it was made from says)
            author_known: true,
            seen: BankSeen::default(),
            sent_unread: None,
            sent_record: None,
            difficulty_read: Some(0),
            inherit: false,
        }
    }

    /// Changes can be sent from this copy: an edit link not made from the viewers' form. (The role is checked
    /// separately: a viewer never sends, and the pool refuses it too.)
    pub fn can_send(&self) -> bool {
        self.mode == LinkMode::Edit && !self.viewers_form
    }

    /// The person's role at the last check; `None` when not checked yet or no longer a member.
    pub fn role(&self) -> Option<super::circles_api::Role> {
        self.role_seen.as_deref().filter(|r| *r != ROLE_NONE && !r.is_empty()).map(super::circles_api::Role::parse)
    }

    /// The circle no longer lists the person, or it is closed: the copy stays, nothing goes either way.
    pub fn gone(&self) -> bool {
        self.role_seen.as_deref() == Some(ROLE_NONE) || self.seen.circle_closed
    }

    pub fn meta(&self) -> ItemMeta {
        ItemMeta { title: self.title.chars().take(200).collect(), subject: self.subject.clone(), course: self.course.clone(), grade: self.grade.clone(), ..ItemMeta::default() }
    }

    pub fn licence(&self) -> Option<License> {
        License::parse(&self.license)
    }

    /// The bank went back in time (a saved copy of the banks was put back) while the link stayed
    /// at the newer version: the link no longer says what the bank was last in step with.
    pub fn stale_for(&self, bank: &QuestionBank) -> bool {
        // Bank times are seconds. A value this large is milliseconds written by an earlier build.
        let at = if self.bank_modified > 100_000_000_000 { self.bank_modified / 1000 } else { self.bank_modified };
        // The same for the bank: an earlier build wrote milliseconds into it.
        let bank_at = if bank.modified > 100_000_000_000 { bank.modified / 1000 } else { bank.modified };
        if at <= 0 || bank_at >= at {
            return false;
        }
        // Older: stale only when the content is not what this app wrote (a file put back).
        self.bank_print.is_empty() || bank_print(bank) != self.bank_print
    }

    /// Says the bank as it is now is what this link is in step with (after a copy, merge or send).
    pub fn mark_bank(&mut self, bank: &QuestionBank) {
        self.bank_modified = bank.modified;
        self.bank_print = bank_print(bank);
    }

    /// How many versions the circle has that this copy has not seen.
    pub fn behind(&self) -> u64 {
        self.head_seen.saturating_sub(self.base_version)
    }

    /// The circle's question id for a local question.
    pub fn qid_of(&self, local: &str) -> Option<&str> {
        self.qids.iter().find(|q| q.local == local).map(|q| q.qid.as_str())
    }
}

/// `(local id, circle id)` for each base question that has one.
pub fn qids_of(base: &[BaseQuestion]) -> Vec<QidLink> {
    base.iter().filter(|b| !b.qid.is_empty()).map(|b| QidLink { local: b.local_id.clone(), qid: b.qid.clone() }).collect()
}

// ---- what changed since the copy was in step -----------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChangeCount {
    pub added: usize,
    pub changed: usize,
    pub removed: usize,
    /// False when the snapshot was not kept: the changes can not be counted, only sent. (critic 85 I1) Also false when the
    /// copy has no record of some question's difficulty in that version (see `difficulty`).
    pub known: bool,
    /// (critic 88 I2) Not known only for that reason: what is counted is right, and only a change to a difficulty may be
    /// missing from it.
    pub difficulty: bool,
}

impl ChangeCount {
    pub fn total(&self) -> usize {
        self.added + self.changed + self.removed
    }
    pub fn any(&self) -> bool {
        self.total() > 0 || !self.known
    }
    /// (critic 88 I2) Counted, except that a change to a difficulty may be missing.
    pub fn partial(&self) -> bool {
        !self.known && self.difficulty
    }
}

/// Compares the local bank with what it was when last in step.
pub fn changes_since(link: &CircleLink, bank: &QuestionBank) -> ChangeCount {
    let mut whole = !link.base.is_empty() || link.qids.is_empty();
    let mut c = ChangeCount { known: whole, ..ChangeCount::default() };
    for q in &bank.questions {
        match link.qid_of(&q.id) {
            None => c.added += 1,
            Some(_) => match link.base.iter().find(|b| b.local_id == q.id) {
                Some(b) => {
                    if merge::ours_changed(b, q) {
                        c.changed += 1;
                    } else if b.snap.difficulty.is_none() {
                        // (critic 85 I1) a base an earlier build wrote: whether its difficulty changed can not be told, so
                        // the count is never said to be complete (never "up to date")
                        c.known = false;
                        c.difficulty = true;
                    }
                }
                None => {
                    c.known = false;
                    whole = false;
                }
            },
        }
    }
    // (critic 88 I2) only where everything else is counted
    c.difficulty &= whole;
    c.removed = link.qids.iter().filter(|l| !bank.questions.iter().any(|q| q.id == l.local)).count();
    c
}

/// (critic 85 I1) A base an earlier build wrote kept no difficulty (see `merge::Snap::difficulty`).
pub fn difficulty_unknown(link: &CircleLink) -> bool {
    link.base.iter().any(|b| b.snap.difficulty.is_none())
}

/// (critic 87 B1, critic 89 I1) The base's difficulties are read from the circle's own version at the link's base version:
/// a link an earlier build wrote (no record of a read: once, whatever its base says, since the round 12 build filled it from
/// the bank), or a base that lacks some (once per base version: a question that version lacks is not read for again).
pub fn needs_difficulty_read(l: &CircleLink) -> bool {
    l.base_version > 0
        && !l.base.is_empty()
        && match l.difficulty_read {
            None => true,
            Some(n) => n != l.base_version && difficulty_unknown(l),
        }
}

/// (critic 87 B1) Such a base learns each difficulty from the circle's own version at the link's base version (`theirs`, as
/// read from the circle), by the circle's question id. Never from the bank: the phase C build ignored difficulty in a merge,
/// so a bank exactly as the app last wrote it can still hold a rating the circle does not have (the teacher's unsent one),
/// or lack one it has (a colleague's that merge dropped). (critic 89 I1) What a link an earlier build wrote says is learnt
/// over, and a question the version lacks is then not known. The read is recorded, and the copy is to inherit what it lacks
/// (see [`inherit_difficulty`]). Returns whether the link changed.
pub fn learn_difficulty(link: &mut CircleLink, theirs: &[Theirs]) -> bool {
    let before = link.clone();
    let earlier = link.difficulty_read.is_none();
    for b in link.base.iter_mut().filter(|b| !b.qid.is_empty()) {
        match theirs.iter().find(|t| t.qid == b.qid) {
            Some(t) => b.snap.difficulty = Some(t.q.difficulty),
            None if earlier => b.snap.difficulty = None,
            None => {}
        }
    }
    link.difficulty_read = Some(link.base_version);
    link.inherit = true;
    *link != before
}

/// Bank and link times are seconds; an earlier build wrote milliseconds (and a question copied or merged from the circle
/// still has them).
fn secs(t: i64) -> i64 {
    if t > 100_000_000_000 { t / 1000 } else { t }
}

/// (the owner's decision, 2026-10-26; critic 90 I1) A link an earlier build wrote, its difficulties just learnt from the
/// circle's version: the copy inherits the circle's rating where it has none, unless the teacher edited that question since
/// the copy last matched (a later deliberate change of hers supersedes). A rating the copy has is never replaced (it may be
/// her unsent one), and nothing goes into a bank put back from a saved copy (what it matched is not known). Returns how
/// many questions took the circle's rating.
pub fn inherit_difficulty(link: &CircleLink, bank: &mut QuestionBank) -> usize {
    if link.stale_for(bank) {
        return 0;
    }
    // (the bank exactly as the app last wrote it with this link: nothing in it was edited since)
    let untouched = !link.bank_print.is_empty() && bank_print(bank) == link.bank_print;
    let since = secs(link.bank_modified);
    let mut n = 0;
    for b in &link.base {
        let Some(Some(d)) = b.snap.difficulty else { continue };
        let Some(q) = bank.questions.iter_mut().find(|q| q.id == b.local_id) else { continue };
        if q.difficulty.is_some() || !(untouched || secs(q.modified) <= since) {
            continue;
        }
        q.difficulty = Some(d);
        n += 1;
    }
    n
}

// ---- the payload ----------------------------------------------------------------------------------------

/// The bank as sent: its JSON, and for each question the circle already knows, `published.qid` so it
/// keeps its identity (an unchanged question is not counted as changed, nor is its credit lost).
pub fn payload_of(link: &CircleLink, bank: &QuestionBank) -> Result<Vec<u8>, String> {
    if bank.questions.is_empty() {
        return Err("The bank has no questions. A circle bank needs at least one.".into());
    }
    let mut v = serde_json::to_value(bank).map_err(|e| e.to_string())?;
    let pid = if super::pool::is_publish_id(&link.base_version_id) { link.base_version_id.clone() } else { link.circle_bank_id.clone() };
    if let Some(qs) = v.get_mut("questions").and_then(Value::as_array_mut) {
        for q in qs {
            let local = q.get("id").and_then(Value::as_str).unwrap_or("").to_string();
            if let Some(qid) = link.qid_of(&local).filter(|id| is_item_id(id))
                && let Some(o) = q.as_object_mut()
            {
                o.insert("published".into(), json!({ "item_id": link.circle_bank_id, "qid": qid, "publish_id": pid, "version": link.base_version.min(u64::from(u32::MAX)) }));
            }
        }
    }
    Ok(format!("{{\"schema_version\":{SEND_SCHEMA},\"bank\":{v}}}").into_bytes())
}

/// The picture's name on the circle (a hash) for a local file `media/<sha>.<ext>`.
pub fn sha_of_file(file: &str) -> Option<String> {
    file.strip_prefix("media/").and_then(|r| r.split_once('.')).map(|(s, _)| s.to_string())
}

fn base_for(q: &Question, qid: &str) -> BaseQuestion {
    merge::base_of(q, qid, "", q.image.as_ref().and_then(|i| sha_of_file(&i.file)))
}

/// The link after `sent` went through: the bank as sent is now what the circle has.
pub fn after_send(link: &CircleLink, bank: &QuestionBank, sent: &SentBank) -> CircleLink {
    let mut l = link.clone();
    let base: Vec<BaseQuestion> = if sent.qids.len() == bank.questions.len() { bank.questions.iter().zip(&sent.qids).map(|(q, id)| base_for(q, id)).collect() } else { Vec::new() };
    l.qids = if base.is_empty() { Vec::new() } else { qids_of(&base) };
    l.base = merge::bounded(base);
    l.base_version = sent.version;
    l.base_version_id = sent.version_id.clone();
    l.head_seen = l.head_seen.max(sent.version);
    l.mark_bank(bank);
    // (critic 89 I1) the base is the bank as sent: its difficulties are known as written
    l.difficulty_read = Some(0);
    l.inherit = false;
    l
}

/// The link for a bank just shared into a circle as a new circle bank (phase D1, Q6): that bank itself, in step
/// with the version the pool made, each question known by the id the pool gave it, so the author does not need a
/// second copy. `None` when the answer can not be matched to the bank's questions (the circle page then offers
/// "Add to My Banks").
#[allow(clippy::too_many_arguments)]
pub fn link_from_share(circle_id: &str, circle_name: &str, role: super::circles_api::Role, bank: &QuestionBank, sent: &SentBank, meta: &ItemMeta, license: License, now: i64) -> Option<CircleLink> {
    if sent.version == 0 || !is_item_id(circle_id) || !is_item_id(&sent.bank_id) || sent.qids.len() != bank.questions.len() || !sent.qids.iter().all(|q| is_item_id(q)) {
        return None;
    }
    let base: Vec<BaseQuestion> = bank.questions.iter().zip(&sent.qids).map(|(q, id)| base_for(q, id)).collect();
    let mut l = CircleLink {
        circle_id: circle_id.to_string(),
        circle_name: circle_name.to_string(),
        circle_bank_id: sent.bank_id.clone(),
        local_bank_id: bank.id.clone(),
        title: meta.title.clone(),
        subject: meta.subject.clone(),
        course: meta.course.clone(),
        grade: meta.grade.clone(),
        license: license.as_str().to_string(),
        base_version: sent.version,
        base_version_id: sent.version_id.clone(),
        qids: qids_of(&base),
        base: merge::bounded(base),
        head_seen: sent.version,
        head_seen_at: Some(now),
        head_seen_by: None,
        linked_at: now,
        bank_modified: 0,
        bank_print: String::new(),
        mode: LinkMode::Edit,
        viewers_form: false,
        role_seen: Some(role.as_str().to_string()),
        role_seen_at: Some(now),
        role_seen_by: None,
        is_author: true,
        author_known: true,
        seen: BankSeen::default(),
        sent_unread: None,
        sent_record: None,
        difficulty_read: Some(0),
        inherit: false,
    };
    // The bank as it was sent: an edit made while the send was on its way shows as unsent, never as "put back".
    l.mark_bank(bank);
    Some(l)
}

// ---- links made elsewhere (the publish window) ---------------------------------------------------------------

/// What happened to a link handed over with [`queue_link`], by circle bank id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkOutcome {
    Linked,
    /// The bank was already linked to another circle bank: that link stays, this one is not made.
    AlreadyLinked,
    /// The list of links could not be saved (the words say why).
    NotSaved(String),
}

thread_local! {
    /// Which circle each bank of this computer is linked to, as the circles model last read or wrote its links (the
    /// publish window asks, critic 64 I11). On the UI thread only.
    static LINKED_TO: std::cell::RefCell<Vec<(String, String)>> = const { std::cell::RefCell::new(Vec::new()) };
    static QUEUED: std::cell::RefCell<Vec<CircleLink>> = const { std::cell::RefCell::new(Vec::new()) };
    static OUTCOMES: std::cell::RefCell<Vec<(String, LinkOutcome)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Notes which circle each local bank is linked to (the circles model, when it reads or writes its links).
pub(super) fn note_linked(links: &[CircleLink]) {
    LINKED_TO.with(|v| *v.borrow_mut() = links.iter().map(|l| (l.local_bank_id.clone(), l.circle_id.clone())).collect());
}

/// The circles this local bank is linked to (none, or one), as last read on this computer.
pub fn circles_of_bank(local: &str) -> Vec<String> {
    LINKED_TO.with(|v| v.borrow().iter().filter(|(b, _)| b == local).map(|(_, c)| c.clone()).collect())
}

/// Hands a link made outside the circles model (a bank shared into a circle) to it: the model takes it the next
/// time it reads its links (every frame of the Banks list, a bank's page or the Shared page). Kept in memory on
/// the UI thread only.
pub fn queue_link(link: CircleLink) {
    QUEUED.with(|q| q.borrow_mut().push(link));
}

pub(super) fn take_queued() -> Vec<CircleLink> {
    QUEUED.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

pub(super) fn note_outcome(circle_bank_id: &str, o: LinkOutcome) {
    OUTCOMES.with(|v| {
        let mut v = v.borrow_mut();
        v.retain(|(b, _)| b != circle_bank_id);
        v.push((circle_bank_id.to_string(), o));
        let n = v.len();
        if n > 20 {
            v.drain(..n - 20);
        }
    });
}

/// What became of the link queued for this circle bank; `None` while it waits (or none was queued).
pub fn outcome_of(circle_bank_id: &str) -> Option<LinkOutcome> {
    OUTCOMES.with(|v| v.borrow().iter().find(|(b, _)| b == circle_bank_id).map(|(_, o)| o.clone()))
}

// ---- where the links are kept ------------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct LinkFile {
    #[serde(default)]
    links: Vec<CircleLink>,
}

/// Where the links are kept: the app's store, a folder (tests: real files, a crash can be mimicked),
/// or a shared value (tests).
#[derive(Clone, Debug)]
pub enum LinkLog {
    Store,
    #[cfg(not(target_arch = "wasm32"))]
    Dir(std::path::PathBuf),
    Memory(Arc<Mutex<Vec<CircleLink>>>),
}

/// The links that parse; an entry that does not is skipped and the file's text is kept as `.bad`.
fn parse_links(bytes: &[u8]) -> (Vec<CircleLink>, bool) {
    let Ok(v) = serde_json::from_slice::<Value>(bytes) else { return (Vec::new(), true) };
    let Some(list) = v.get("links").and_then(Value::as_array) else { return (Vec::new(), v.get("links").is_some()) };
    let mut out = Vec::new();
    let mut bad = false;
    for e in list {
        match serde_json::from_value::<CircleLink>(e.clone()) {
            Ok(l) if is_item_id(&l.circle_id) && is_item_id(&l.circle_bank_id) => out.push(l),
            _ => bad = true,
        }
    }
    (out, bad)
}

impl LinkLog {
    pub fn memory() -> LinkLog {
        LinkLog::Memory(Arc::default())
    }

    fn read(&self) -> Option<Vec<u8>> {
        match self {
            LinkLog::Store => crate::store::with_backend(|b| b.read_doc(LINKS_DOC)),
            #[cfg(not(target_arch = "wasm32"))]
            LinkLog::Dir(d) => crate::store::Backend::read_doc(&crate::store::FsBackend::new(d.clone()), LINKS_DOC),
            LinkLog::Memory(_) => None,
        }
    }

    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        match self {
            LinkLog::Store => crate::store::with_backend(|b| b.write_doc(name, bytes)),
            #[cfg(not(target_arch = "wasm32"))]
            LinkLog::Dir(d) => crate::store::Backend::write_doc(&crate::store::FsBackend::new(d.clone()), name, bytes),
            LinkLog::Memory(_) => Ok(()),
        }
    }

    fn exists(&self) -> bool {
        match self {
            LinkLog::Store => crate::store::with_backend(|b| b.exists(LINKS_DOC)),
            #[cfg(not(target_arch = "wasm32"))]
            LinkLog::Dir(d) => crate::store::Backend::exists(&crate::store::FsBackend::new(d.clone()), LINKS_DOC),
            LinkLog::Memory(_) => false,
        }
    }

    fn read_named(&self, name: &str) -> Option<Vec<u8>> {
        match self {
            LinkLog::Store => crate::store::with_backend(|b| b.read_doc(name)),
            #[cfg(not(target_arch = "wasm32"))]
            LinkLog::Dir(d) => crate::store::Backend::read_doc(&crate::store::FsBackend::new(d.clone()), name),
            LinkLog::Memory(_) => None,
        }
    }

    /// Keeps the file as it was: the first copy is `.bad`, later different ones `.bad.2` to `.bad.9`.
    /// A copy that is already kept is not written again.
    fn keep_bad(&self, bytes: &[u8]) {
        for n in 1..=9 {
            let name = if n == 1 { format!("{LINKS_DOC}.bad") } else { format!("{LINKS_DOC}.bad.{n}") };
            match self.read_named(&name) {
                Some(old) if old == bytes => return,
                Some(_) => continue,
                None => {
                    let _ = self.write(&name, bytes);
                    return;
                }
            }
        }
    }

    /// The links. One entry that can not be read does not drop the others: it is skipped, and the
    /// file as it was is kept beside it as `share-circles.json.bad` before anything is written over it.
    pub fn load(&self) -> Vec<CircleLink> {
        self.load_checked().unwrap_or_default()
    }

    /// Like [`load`](Self::load), but a file that is there and could not be read is an error, not "no
    /// links": the caller must not write over it.
    pub fn load_checked(&self) -> Result<Vec<CircleLink>, String> {
        if let LinkLog::Memory(m) = self {
            return Ok(m.lock().unwrap_or_else(|p| p.into_inner()).clone());
        }
        let Some(bytes) = self.read() else {
            return if self.exists() { Err("the file could not be read just now".into()) } else { Ok(Vec::new()) };
        };
        let (links, bad) = parse_links(&bytes);
        if bad {
            self.keep_bad(&bytes);
        }
        Ok(links)
    }

    pub fn save(&self, links: &[CircleLink]) -> Result<(), String> {
        match self {
            LinkLog::Memory(m) => {
                *m.lock().unwrap_or_else(|p| p.into_inner()) = links.to_vec();
                Ok(())
            }
            _ => {
                let bytes = serde_json::to_vec(&LinkFile { links: links.to_vec() }).map_err(|e| e.to_string())?;
                // The backend writes beside the name and renames into place: a crash leaves the old file or the new, never half.
                self.write(LINKS_DOC, &bytes)
            }
        }
    }
}

/// What the bank's questions are right now: noticed edits, and a bank put back from a saved copy.
pub fn bank_print(bank: &QuestionBank) -> String {
    super::pool::sha256_hex(&serde_json::to_vec(&bank.questions).unwrap_or_default())
}

/// How many questions the payload holds, readable or not.
pub fn payload_question_count(payload: &[u8]) -> usize {
    serde_json::from_slice::<Value>(payload).ok().and_then(|v| v.pointer("/bank/questions").and_then(Value::as_array).map(Vec::len)).unwrap_or(0)
}

/// A circle bank's questions in this app's shape (pictures left for the caller). The circle's id for
/// a question is the `id` it was served with; it stays the same across versions.
pub fn circle_theirs(payload: &[u8], at_ms: i64) -> Vec<Theirs> {
    circle_theirs_as(payload, at_ms, false)
}

/// The same for a read in the viewers' form (`keys_hidden`, phase C): every question is marked as having
/// no key, whether or not the question says so itself.
pub fn circle_theirs_as(payload: &[u8], at_ms: i64, keys_hidden: bool) -> Vec<Theirs> {
    let ids: Vec<String> = serde_json::from_slice::<Value>(payload)
        .ok()
        .and_then(|v| v.pointer("/bank/questions").and_then(Value::as_array).map(|qs| qs.iter().map(|q| q.get("id").and_then(Value::as_str).unwrap_or("").to_string()).collect()))
        .unwrap_or_default();
    // A circle bank is served whole, unless it is the viewers' form of a bank that hides its keys.
    let head = ItemHead { item: ItemView { form: if keys_hidden { "stripped" } else { "full" }.into(), ..ItemView::default() }, ..ItemHead::default() };
    let mut theirs = theirs_of(payload, &head, at_ms);
    for t in &mut theirs {
        if let Some(id) = ids.get(t.index).filter(|i| is_item_id(i)) {
            t.qid = id.clone();
        }
    }
    theirs
}

// ---- the viewers' form: a fresh shuffle is not a change (critic 63 B2) -----------------------------------------------

/// A question's choices as the viewers' form can serve them: unlocked choices and the Matching columns are shuffled
/// afresh on every read (the pool's `sameServed`), so they are compared as sets; locked choices keep their places.
fn order_free(k: &crate::model::Kind) -> Value {
    use crate::model::{Answer, Kind, PartKind};
    fn answers(list: &[Answer]) -> Value {
        let locked: Vec<Value> = list.iter().enumerate().filter(|(_, a)| a.locked).map(|(i, a)| json!([i, a.text, a.correct])).collect();
        let mut free: Vec<String> = list.iter().filter(|a| !a.locked).map(|a| json!([a.text, a.correct]).to_string()).collect();
        free.sort();
        json!({ "locked": locked, "free": free })
    }
    match k {
        Kind::Mc { answers: a } => json!({ "Mc": answers(a) }),
        Kind::Matching { pairs, distractors } => {
            let mut left: Vec<&str> = pairs.iter().map(|p| p.left.as_str()).collect();
            left.sort_unstable();
            let mut bank: Vec<&str> = pairs.iter().map(|p| p.right.as_str()).chain(distractors.iter().map(String::as_str)).collect();
            bank.sort_unstable();
            json!({ "Matching": { "left": left, "bank": bank } })
        }
        Kind::Multipart { parts } => json!({ "Multipart": parts.iter().map(|p| json!({ "stem": p.stem, "kind": match &p.kind {
            PartKind::Mc { answers: a } => answers(a),
            other => serde_json::to_value(other).unwrap_or(Value::Null),
        } })).collect::<Vec<_>>() }),
        other => serde_json::to_value(other).unwrap_or(Value::Null),
    }
}

/// For a read in the viewers' form: each question whose choices are only shuffled, compared with what this copy had
/// (`base`, matched by the circle's question id), takes the order this copy had, so the review lists only real
/// changes and the copy's choices do not move. A changed choice is a change as before.
pub fn settle_shuffle(theirs: &mut [Theirs], base: &[BaseQuestion]) {
    for t in theirs.iter_mut() {
        let Some(b) = base.iter().find(|b| !b.qid.is_empty() && b.qid == t.qid) else { continue };
        if std::mem::discriminant(&t.q.kind) == std::mem::discriminant(&b.snap.kind) && order_free(&t.q.kind) == order_free(&b.snap.kind) {
            t.q.kind = b.snap.kind.clone();
        }
    }
}
