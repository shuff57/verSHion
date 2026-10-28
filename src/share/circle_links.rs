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
use super::partial_send::ComposedQuestion;
use super::pool::{ItemHead, ItemMeta, ItemView, License, is_item_id};
use super::publish::QidLink;
use crate::model::{Difficulty, Question, QuestionBank};

pub const LINKS_DOC: &str = "share-circles.json";
/// The banks file of the same store (`Store::save_banks` writes it there too).
pub const BANKS_DOC: &str = "banks.json";

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
    /// (phase D3, plan 5.1) Local question ids the person unticked at the last partial send, so the next send summary
    /// starts them unticked again. A link written before phase D3 has none, and a whole-bank send clears it. Read
    /// leniently: a value this build can not read is none, never the link with it.
    #[serde(default, deserialize_with = "ids_or_empty")]
    pub held_back: Vec<String>,
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
    /// (the owner's rule, 2026-10-26; round 15) Difficulties were just learnt from the circle's version: the copy takes the
    /// circle's difficulty for each question that version has (see [`inherit_difficulty`]) at the next pass that has the bank.
    /// (round 19, critic 99 m8) Read leniently: a value this build can't read is "no", never the link with it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not", deserialize_with = "lenient_bool")]
    pub inherit: bool,
    /// (round 17, critic 95 I1) While `inherit` waits (its first pass could not be saved): each base question's difficulty in
    /// the bank as the first pass after the read found it. A question whose difficulty differs from it now was changed by her
    /// since, deliberately: it keeps hers, in this session and after a restart. `None`: not recorded yet.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "at_read_or_none")]
    pub at_read: Option<Vec<AtRead>>,
    /// (round 19, critic 99 m1-m3) Written just before a pass writes the banks file: the bank's time before it and the time the pass
    /// gives it. While it is here the banks file may have the pass in it (a restart then takes the pass as done, and moves the time of
    /// an unclear send on with it); a pass that could not write the banks file takes it out again. `None`: no such write.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "times_or_none")]
    pub pass_times: Option<(i64, i64)>,
    /// (round 16, critic 93 minor 2; round 19, critic 99 m4) Base questions the bank did not have when it took the circle's
    /// difficulties (deleted before the read): one put back (Undo) takes the circle's at the next pass. The base version then, and
    /// the ids. Kept in the file, so that a pass held for such a question is still waited for after a restart.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "missing_or_none")]
    pub missing_at_read: Option<(u64, Vec<String>)>,
    /// (round 17, critic 96 I1) What the circle's difficulty replaced in the bank, kept until she dismisses it: the badge and
    /// the bank's strip say it, whichever page was open when it happened, and each question can be put back.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "news_or_none")]
    pub difficulty_news: Option<DifficultyNews>,
    /// (round 17, critic 96 I2) "Use my bank as it is" was confirmed: the next send makes the circle's newest version match
    /// the bank, difficulties included (its Send page says so). Cleared once the copy moves to another version.
    #[serde(default, skip_serializing_if = "std::ops::Not::not", deserialize_with = "lenient_bool")]
    pub as_is: bool,
}

/// (round 17, critic 95 I1) A base question's difficulty in the bank when the read's difficulties were first to be taken.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtRead {
    pub id: String,
    /// (round 19, critic 99 m8) Never left out: an entry without it is not known (dropped), never "none".
    #[serde(deserialize_with = "present")]
    pub difficulty: Option<Difficulty>,
}

/// (round 19, critic 99 m8) A value that must be there (`null` included).
fn present<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Difficulty>, D::Error> {
    Option::<Difficulty>::deserialize(d)
}

/// (round 17, critic 96 I1) The circle's difficulty put into the bank after the read of `version`: each question it changed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DifficultyNews {
    pub version: u64,
    pub questions: Vec<NewsItem>,
}

/// One question of that news: its id in the bank, her difficulty before and the circle's after, and whether she put hers back.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NewsItem {
    pub id: String,
    pub before: Option<Difficulty>,
    pub after: Option<Difficulty>,
    /// (round 19, critic 99 m8) Read leniently, as `changed`: a value this build can't read is "no", never the news with it.
    #[serde(deserialize_with = "lenient_bool")]
    pub put_back: bool,
    /// (round 18, critic 98 I1) The question's difficulty was changed since (by her, or by a version she got): once seen, "Put
    /// mine back" is never offered for it again, so it never writes over her newer choice or a colleague's newer difficulty.
    #[serde(skip_serializing_if = "std::ops::Not::not", deserialize_with = "lenient_bool")]
    pub changed: bool,
}

/// (round 18, critic 98 I1) Where one question of the difficulty news stands now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewsState {
    /// It still has the circle's difficulty from the read, and the copy is still at that version: "Put mine back" is offered.
    Replaced,
    /// It still has the circle's difficulty, but the copy has moved on (a send, Get latest): nothing is offered.
    Kept,
    /// Hers is back and not sent yet.
    Back,
    /// Hers is back, and the version the copy is in step with has it (she sent it).
    Sent,
    /// Its difficulty changed since (her own choice, or a colleague's in a version she got): what it is now.
    Changed(Option<Difficulty>),
    /// The question is no longer in the bank.
    Gone,
}

/// The difficulty the question is expected to have while nothing changed it: hers once she put it back, else the circle's.
fn news_expected(item: &NewsItem) -> Option<Difficulty> {
    if item.put_back { item.before } else { item.after }
}

/// (round 18, critic 98 I1) Where `item` of `link`'s news stands for `bank` as it is.
pub fn news_state(link: &CircleLink, item: &NewsItem, bank: &QuestionBank) -> NewsState {
    let Some(q) = bank.questions.iter().find(|q| q.id == item.id) else { return NewsState::Gone };
    if q.difficulty == item.before {
        // sent once the version the copy is in step with has hers; (round 19, critic 99 m9) said to be hers, sent, only where she put
        // it back: a colleague's version that happens to give her old difficulty is "changed since"
        let base = link.base.iter().find(|b| b.local_id == item.id).and_then(|b| b.snap.difficulty);
        return match base == Some(item.before) {
            true if item.put_back || !item.changed => NewsState::Sent,
            true => NewsState::Changed(q.difficulty),
            false => NewsState::Back,
        };
    }
    if item.changed || q.difficulty != news_expected(item) {
        return NewsState::Changed(q.difficulty);
    }
    let version = link.difficulty_news.as_ref().map_or(0, |n| n.version);
    if link.base_version == version { NewsState::Replaced } else { NewsState::Kept }
}

/// (round 19, critic 100 minor 5) Nothing of the news is left for her to do: no question still has the circle's difficulty at the
/// read's version (Put mine back offered), and none is hers back but not sent. The badge then no longer says it.
pub fn news_settled(link: &CircleLink, bank: &QuestionBank) -> bool {
    link.difficulty_news.as_ref().is_none_or(|n| n.questions.iter().all(|i| !matches!(news_state(link, i, bank), NewsState::Replaced | NewsState::Back)))
}

/// (round 18, critic 98 I1) Notes each question of the news whose difficulty changed since (see `NewsItem::changed`), for good.
/// Returns whether the link changed.
pub fn settle_news(link: &mut CircleLink, bank: &QuestionBank) -> bool {
    let Some(n) = link.difficulty_news.as_mut() else { return false };
    let mut changed = false;
    for item in n.questions.iter_mut().filter(|i| !i.changed) {
        if let Some(q) = bank.questions.iter().find(|q| q.id == item.id)
            && q.difficulty != news_expected(item)
        {
            item.changed = true;
            changed = true;
        }
    }
    changed
}

fn news_or_none<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<DifficultyNews>, D::Error> {
    Ok(serde_json::from_value(Value::deserialize(d)?).ok())
}

/// (round 17) One this build can not read is dropped alone, never the link with it (as `sent_record`). (round 18, critic 97 m7)
/// Entry by entry: one entry this build can't read is dropped alone, never the others.
fn at_read_or_none<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Vec<AtRead>>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Array(list) => Some(list.into_iter().filter_map(|e| serde_json::from_value(e).ok()).collect()),
        _ => None,
    })
}

/// (round 18, critic 97 m7) A flag this build can't read is "no", never the link with it.
fn lenient_bool<'de, D: serde::Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    Ok(Value::deserialize(d)?.as_bool().unwrap_or(false))
}

fn times_or_none<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<(i64, i64)>, D::Error> {
    Ok(serde_json::from_value(Value::deserialize(d)?).ok())
}

fn missing_or_none<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<(u64, Vec<String>)>, D::Error> {
    Ok(serde_json::from_value(Value::deserialize(d)?).ok())
}

fn record_or_none<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<SentRecord>, D::Error> {
    Ok(serde_json::from_value(Value::deserialize(d)?).ok())
}

/// (phase D3) A list of local ids read leniently: absent (by `default`) or unreadable is none, and a list keeps only
/// its string entries. One this build can not read never drops the link with it.
fn ids_or_empty<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Array(list) => list.into_iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    })
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
            held_back: Vec::new(),
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
            at_read: None,
            pass_times: None,
            missing_at_read: None,
            difficulty_news: None,
            as_is: false,
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
    link.at_read = None;
    link.pass_times = None;
    *link != before
}

/// (the owner's rule, 2026-10-26; round 15) A link an earlier build wrote, its difficulties just learnt from the circle's
/// version: the copy inherits the circle's difficulty for each question that version has, whatever the copy had (its rating,
/// another one, none) and whenever the question was edited (critic 91 I3, critic 92 I1). From then on a difference is her
/// deliberate change. Nothing goes into a bank put back from a saved copy (what it matched is not known). Returns each
/// question whose difficulty changed: its id, and its difficulty before and after.
///
/// (round 16, critic 93 I1) Except a rating she already pressed Send for: a question her own send record (`sent`, the send
/// whose answer did not say what it made) carried with a rating other than the circle's keeps the bank's (one it carried with
/// none takes the circle's: the phase C build sent none for a rating it had dropped). `only`: just
/// these questions (one put back after the read: see `CircleEditModel::inherit_pending`).
///
/// (round 17, critic 95 I1) And a question whose difficulty she changed since the first pass after the read (see
/// `CircleLink::at_read`): her later change supersedes.
pub fn inherit_difficulty(link: &CircleLink, bank: &mut QuestionBank, sent: Option<&SentRecord>, only: Option<&[String]>) -> Vec<(String, Option<Difficulty>, Option<Difficulty>)> {
    if link.stale_for(bank) {
        return Vec::new();
    }
    let mut moved = Vec::new();
    for b in &link.base {
        let Some(d) = b.snap.difficulty else { continue };
        if only.is_some_and(|o| !o.contains(&b.local_id)) {
            continue;
        }
        if sent.is_some_and(|r| r.questions.iter().any(|s| s.q.id == b.local_id && s.q.difficulty.is_some() && s.q.difficulty != d)) {
            continue;
        }
        let Some(q) = bank.questions.iter_mut().find(|q| q.id == b.local_id) else { continue };
        if link.at_read.as_ref().is_some_and(|r| r.iter().any(|a| a.id == q.id && a.difficulty != q.difficulty)) {
            continue;
        }
        if q.difficulty != d {
            moved.push((q.id.clone(), q.difficulty, d));
            q.difficulty = d;
        }
    }
    moved
}

/// (round 18, critic 97 m6) What an earlier pass of this read already put into the bank, told by the bank as the read found it
/// (`at_read`): each question that has the circle's difficulty now, and had another one then (id, before, after). A pass whose banks
/// file was written but whose links file was not is run again after a restart: these go into its news again.
pub fn inherited_before(link: &CircleLink, bank: &QuestionBank) -> Vec<(String, Option<Difficulty>, Option<Difficulty>)> {
    let Some(at) = &link.at_read else { return Vec::new() };
    link.base
        .iter()
        .filter_map(|b| {
            let d = b.snap.difficulty?;
            let a = at.iter().find(|a| a.id == b.local_id)?;
            let q = bank.questions.iter().find(|q| q.id == b.local_id)?;
            (a.difficulty != d && q.difficulty == d).then(|| (q.id.clone(), a.difficulty, d))
        })
        .collect()
}

// ---- the payload ----------------------------------------------------------------------------------------

/// The `published` block a send gives the question whose local id is `local` (none: not known by a usable circle
/// id). The server keeps the question's identity from `published.qid`; the rest is what the pool's sends carry.
pub(super) fn published_block(link: &CircleLink, local: &str) -> Option<Value> {
    let qid = link.qid_of(local).filter(|id| is_item_id(id))?;
    let pid = if super::pool::is_publish_id(&link.base_version_id) { link.base_version_id.clone() } else { link.circle_bank_id.clone() };
    Some(json!({ "item_id": link.circle_bank_id, "qid": qid, "publish_id": pid, "version": link.base_version.min(u64::from(u32::MAX)) }))
}

/// The bank as sent: its JSON, and for each question the circle already knows, `published.qid` so it
/// keeps its identity (an unchanged question is not counted as changed, nor is its credit lost).
pub fn payload_of(link: &CircleLink, bank: &QuestionBank) -> Result<Vec<u8>, String> {
    if bank.questions.is_empty() {
        return Err("The bank has no questions. A circle bank needs at least one.".into());
    }
    let mut v = serde_json::to_value(bank).map_err(|e| e.to_string())?;
    if let Some(qs) = v.get_mut("questions").and_then(Value::as_array_mut) {
        for q in qs {
            let local = q.get("id").and_then(Value::as_str).unwrap_or("").to_string();
            if let Some(published) = published_block(link, &local)
                && let Some(o) = q.as_object_mut()
            {
                o.insert("published".into(), published);
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
    // A whole-bank send carries every change: nothing is left unticked for the next summary.
    l.held_back.clear();
    // (critic 89 I1) the base is the bank as sent: its difficulties are known as written, and (round 16) none is left to take
    l.difficulty_read = Some(0);
    l.inherit = false;
    l.at_read = None;
    l.pass_times = None;
    l.missing_at_read = None;
    // (round 17) the bank as it is was sent
    l.as_is = false;
    l
}

/// The link after a partial send (phase D3, plan section 6): `sent` made `composed`, this bank with every unticked
/// change left as the base had it. The copy is now in step with the version the send made, so the base snapshot and
/// the local id -> qid pairs are those of the COMPOSED questions: an unticked removal keeps its pair with the base
/// body's raw question, a ticked removal's pair is dropped, and a question new to the circle takes the qid the send's
/// answer gave it at its place.
///
/// `bank` is the bank as it was when the send started, not the composed version: the send does not change the bank,
/// so an edit made while it was in flight is newer and never "put back", and the next summary lists it as unsent.
/// `held_back` is the caller's unticked ids that still differ, stored as given for the next summary's defaults.
/// Pure: it reads its arguments and writes nothing.
///
/// A composed question this build could not read (a newer client's `Kind` or `Difficulty`) still keeps its local id
/// -> qid pair, but has no base snapshot. A bank question that then has a pair and no snapshot cannot have its
/// changes counted: `changes_since` says the count is not fully known, as for a base an earlier build wrote.
pub fn after_partial_send(link: &CircleLink, bank: &QuestionBank, composed: &[ComposedQuestion], sent: &SentBank, held_back: Vec<String>) -> CircleLink {
    let mut l = link.clone();
    // (critic) The answer's qids are one per composed question in payload order, and are authoritative where they
    // are there. A short or empty list is not aligned, so it is ignored whole: no question may borrow another's qid.
    let aligned = sent.qids.len() == composed.len();
    let mut qids: Vec<QidLink> = Vec::new();
    let mut base: Vec<BaseQuestion> = Vec::new();
    for (i, c) in composed.iter().enumerate() {
        // The qid it keeps: the server's when the answer names one, else the composed pair's own; an addition with
        // neither stays unlinked.
        let served = aligned.then(|| sent.qids[i].as_str()).filter(|q| !q.is_empty());
        let qid = served.or_else(|| c.qid.as_deref().filter(|q| !q.is_empty())).unwrap_or_default();
        if !qid.is_empty() {
            qids.push(QidLink { local: c.local.clone(), qid: qid.to_string() });
        }
        // Only a question this build can read goes into the snapshot; the pair above is kept either way.
        if let Some(q) = &c.question {
            let mut b = base_for(q, qid);
            // The composed question's own local id: for a restored removal the raw question's `id` is the circle's qid.
            b.local_id = c.local.clone();
            base.push(b);
        }
    }
    l.qids = qids;
    l.base = merge::bounded(base);
    l.base_version = sent.version;
    l.base_version_id = sent.version_id.clone();
    l.head_seen = l.head_seen.max(sent.version);
    l.held_back = held_back;
    l.mark_bank(bank);
    // The base is the composed version: its difficulties are known as written, and none is left to take.
    l.difficulty_read = Some(0);
    l.inherit = false;
    l.at_read = None;
    l.pass_times = None;
    l.missing_at_read = None;
    l.as_is = false;
    // A send of the full form no longer follows the viewers' form, as a merge of a full version (`finish_merge`).
    if !sent.keys_hidden {
        l.viewers_form = false;
    }
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
        held_back: Vec::new(),
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
        at_read: None,
        pass_times: None,
        missing_at_read: None,
        difficulty_news: None,
        as_is: false,
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

    /// (round 16, critic 93 minor 1) All the banks, into the banks file beside the links file (none for links in memory).
    pub fn save_banks(&self, banks: &[QuestionBank]) -> Result<(), String> {
        match self {
            LinkLog::Memory(_) => Ok(()),
            _ => {
                let bytes = serde_json::to_vec(banks).map_err(|e| e.to_string())?;
                self.write(BANKS_DOC, &bytes)
            }
        }
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
