//! Circle banks in My Banks (phase D1, docs/circles-phaseD-plan.md sections 4-6, 11): the badge on the Banks
//! list, the status strip on a linked bank's page, the quiet check of where the circle bank stands, and the
//! choices behind "Add to My Banks" and "Make my own copy". No egui: `bank_links_ui` draws this.
//!
//! # Rules this module keeps
//!
//! - The badge reads this computer only: the link file and the bank. It never starts a request.
//! - A check (`POST /circles/heads` and `GET /circles`) runs only while a linked bank's page is open, only when
//!   the account is signed in, at most once every [`CHECK_EVERY`] per bank, or when the person presses "Check now".
//! - What a check said is written into the link (`head_seen`, `role_seen`, `seen`), so the badge and the strip say
//!   it after a restart without asking again. Another account signing in drops what was read in memory (the
//!   throttle and the last check's state).
//! - A copy that follows the circle (a viewer's link, or one made from the viewers' form) never offers a send.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::time::Duration;

use web_time::Instant;

use super::api::{self, ApiError};
use super::circle_edit::{CircleEditModel, LinkStatus, Perm};
use super::circle_links::{CircleLink, LinkMode, ROLE_NONE};
use super::circles_api::{CircleDetail, CircleSummary, HeadEntry, HeadState, Role};
use super::model::Runner;
use super::pool::{Pool, PoolError, RetryPolicy};
use super::publish::{AccountSnapshot, Gate, SharedSession};
use crate::model::QuestionBank;

/// How often a linked bank's page asks the circle where its bank stands (Q8).
pub const CHECK_EVERY: Duration = Duration::from_secs(10 * 60);

// ---- the badge ---------------------------------------------------------------------------------------------------

/// The quiet line under a linked bank's name on the Banks list: the circle's name, and at most one state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Badge {
    pub circle: String,
    pub note: Option<String>,
}

/// The circle's name as the link remembers it, cleaned.
pub fn circle_name(l: &CircleLink) -> String {
    let n = api::plain(&l.circle_name);
    if n.is_empty() { "a circle".into() } else { n }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 { format!("1 {one}") } else { format!("{n} {many}") }
}

/// The badge of a linked bank, from this computer only. By priority: why it can no longer be sent ("circle closed",
/// "no longer in the circle", "removed from the circle"), then "newer version" (with the unsent changes beside it,
/// critic 64), "N unsent changes", or "changes not counted". Only a copy that can send, while the person may send,
/// has unsent changes.
pub fn badge_of(l: &CircleLink, status: Option<&LinkStatus>) -> Badge {
    badge_of_sent(l, status, None)
}

/// [`badge_of`], knowing whether the last send went through with an answer that could not be read (critic 74 I1): its
/// changes are then not unsent, and the copy is behind the version it made. `Some(true)`: the bank changed after that
/// send (critic 76 I1), so it has unsent changes, which can not be counted apart from the ones that went.
pub fn badge_of_sent(l: &CircleLink, status: Option<&LinkStatus>, sent_unread: Option<bool>) -> Badge {
    let sendable = l.can_send() && role_of(l).can_add_banks();
    let unread = sent_unread.is_some() && sendable;
    let since = sent_unread == Some(true) && sendable;
    // (critic 88 I2) what is counted, also while a change to a difficulty can't be
    let unsent = status.filter(|s| sendable && !unread && (s.changes.known || s.changes.partial())).map_or(0, |s| s.changes.total());
    let uncounted = status.is_some_and(|s| sendable && !unread && !s.changes.known);
    let note = if l.seen.circle_closed {
        Some("circle closed".to_string())
    } else if l.gone() {
        Some("no longer in the circle".to_string())
    } else if l.seen.removed {
        Some("removed from the circle".to_string())
    } else if (l.behind() > 0 || unread) && !keys_kept(l) {
        // (critic 65 minor) only where the strip offers Get latest: a copy that keeps its keys is not offered it
        Some(if unsent > 0 {
            format!("newer version · {unsent} unsent")
        } else if since {
            "newer version · unsent changes".to_string()
        } else {
            "newer version".to_string()
        })
    } else if unsent > 0 {
        Some(plural(unsent, "unsent change", "unsent changes"))
    } else {
        uncounted.then(|| "changes not counted".to_string())
    };
    Badge { circle: circle_name(l), note }
}

/// The badge's state is a warning (the copy can no longer be sent): drawn in the warning colour.
pub fn badge_warns(l: &CircleLink) -> bool {
    l.gone() || l.seen.removed
}

// ---- the strip ------------------------------------------------------------------------------------------------------

/// Where the last check of one bank stands (in memory only).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Check {
    /// Not checked since the app started (or since another account signed in).
    #[default]
    NotYet,
    Checking,
    Done,
    /// The pool could not be reached (or answered with an error).
    Offline,
    /// The circle does not list the account signed in now, and the link was made or last seen by another account on
    /// this computer: nothing was written into the link (critic 63 I1).
    OtherAccount,
}

/// The account, as the strip needs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignIn {
    Ready,
    /// Being looked up: the strip shows what it last knew.
    Unknown,
    Out,
}

pub fn sign_in_of(gate: &Gate) -> SignIn {
    match gate {
        Gate::Ready => SignIn::Ready,
        Gate::Checking | Gate::Unreachable => SignIn::Unknown,
        Gate::NotSetUp | Gate::NoServer | Gate::SignedOut | Gate::NotApproved(_) => SignIn::Out,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StripButton {
    Send,
    GetLatest,
    History,
    OpenAccount,
    TryAgain,
    TreatInStep,
    Unlink,
    AddFullCopy,
}

impl StripButton {
    pub fn label(self) -> &'static str {
        match self {
            StripButton::Send => "Send my changes...",
            StripButton::GetLatest => "Get latest",
            StripButton::History => "History",
            StripButton::OpenAccount => "Sign in...",
            StripButton::TryAgain => "Try again",
            StripButton::TreatInStep => "Use my bank as it is...",
            StripButton::Unlink => "Unlink...",
            StripButton::AddFullCopy => "Add a full copy...",
        }
    }
}

/// What the strip says and offers, primary button first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StripView {
    pub line: String,
    pub buttons: Vec<StripButton>,
    /// A check is on its way (a spinner).
    pub busy: bool,
    /// Something is wrong with the link (drawn as a warning).
    pub warn: bool,
    /// The "..." menu offers the bank's history (it can be read) and the circle's page (it can be opened).
    pub menu_history: bool,
    pub menu_circle_page: bool,
}

pub struct StripInput<'a> {
    pub link: &'a CircleLink,
    pub status: Option<&'a LinkStatus>,
    pub check: Check,
    pub sign_in: SignIn,
    pub now_ms: i64,
    /// (critic 69 I1) A restore or trim of this bank on its way, in words (see `CircleEditModel::act_line`).
    pub act: Option<String>,
    /// (critic 74 I1) The last send from this bank went through, but its answer could not be read (see
    /// `CircleEditModel::sent_unread`): the changes it carried are not unsent, and Get latest catches up.
    pub sent_unread: bool,
    /// (critic 76 I1) The bank changed after that send: those changes are not sent.
    pub changed_since: bool,
    /// (critic 73 minor 1) A restore or trim of this bank had no answer in time and may still land: the newest version
    /// known is not said to be the latest, nor the copy up to date, until the next check or History read.
    pub unsure: bool,
}

/// The person's role for this link: as the last check saw it, else what the link was made for.
pub fn role_of(l: &CircleLink) -> Role {
    l.role().unwrap_or(match l.mode {
        LinkMode::Edit => Role::Editor,
        LinkMode::Follow => Role::Viewer,
    })
}

/// What the person may do with this copy's circle bank, for the send, merge and history screens. A copy that can
/// not send is given a viewer's permission, whatever the role (the screens then offer no send).
pub fn perm_of(l: &CircleLink) -> Perm {
    let role = if l.can_send() { role_of(l) } else { Role::Viewer };
    Perm { role, is_author: l.is_author, closed: l.seen.circle_closed }
}

/// A copy with its answer keys (not made from the viewers' form) of a bank that hides them from viewers, while the
/// person is a viewer: the circle would serve it the viewers' form (critic 63 B1).
pub fn keys_kept(l: &CircleLink) -> bool {
    !l.viewers_form && l.seen.hide_key && role_of(l) == Role::Viewer
}

/// "3 Nov": the day a removed bank can be brought back until.
pub fn day_month(ms: i64) -> String {
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let (_, m, d) = crate::time::civil_from_days(ms.div_euclid(86_400_000));
    format!("{d} {}", MONTHS[(m.clamp(1, 12) - 1) as usize])
}

/// "25 Oct 2026".
pub fn day_month_year(ms: i64) -> String {
    let (y, _, _) = crate::time::civil_from_days(ms.div_euclid(86_400_000));
    format!("{} {y}", day_month(ms))
}

/// The strip for one linked bank: every state of the plan's table (section 4.2) except "kept back" (phase D3).
/// While a check runs the strip keeps what the link last knew, with its buttons, and only adds the spinner, so the
/// page does not jump (critic 64 I9); a link never checked yet says it is checking.
pub fn strip_view(i: &StripInput) -> StripView {
    if i.check == Check::Checking && i.link.role_seen_at.is_none() {
        return StripView { line: format!("Shared with {}. Checking for a newer version...", circle_name(i.link)), buttons: Vec::new(), busy: true, warn: false, menu_history: true, menu_circle_page: true };
    }
    let mut v = strip_view_known(i);
    v.busy |= i.check == Check::Checking;
    v
}

fn strip_view_known(i: &StripInput) -> StripView {
    use StripButton::*;
    let l = i.link;
    let name = circle_name(l);
    let view = |line: String, buttons: Vec<StripButton>| StripView { line, buttons, busy: false, warn: false, menu_history: true, menu_circle_page: true };
    let warn = |line: String, buttons: Vec<StripButton>| StripView { line, buttons, busy: false, warn: true, menu_history: true, menu_circle_page: true };
    let can_send = l.can_send() && role_of(l).can_add_banks();
    let changes = i.status.map(|s| s.changes).unwrap_or_default();
    let stale = i.status.is_some_and(|s| s.stale);
    let head = l.head_seen.max(l.base_version);
    // Facts the last check wrote: they hold whether or not the pool can be reached now.
    if i.check == Check::OtherAccount {
        // (critic 65 minor) a link made by a share, or before phase D, does not say which account it was made by
        let by = if l.role_seen_by.is_some() { " by another account on this computer" } else { "" };
        return view(format!("Shared with {name}{by}. The account signed in now is not in this circle, so it can't send or get newer versions here. Your copy stays as it is."), Vec::new());
    }
    // (critic 66 I6) No longer in the circle, the circle closed, the bank removed: said calmly, in the strip's usual
    // colour, as the bank works as a normal bank. Unlink is a small button, never the big one (critic 64 I4). The
    // menu offers nothing that would fail: the bank's history can not be read, nor the circle's page by someone no
    // longer in it.
    let own_bank = |line: String, circle_page: bool| StripView { line: format!("{line} You can keep using it as your own bank."), buttons: vec![Unlink], busy: false, warn: false, menu_history: false, menu_circle_page: circle_page };
    if l.gone() {
        let line = if l.seen.circle_closed { format!("The circle {name} was closed. Your copy stays; it can't be sent.") } else { format!("You are no longer in {name}. Your copy stays; it can't be sent.") };
        return own_bank(line, l.role_seen.as_deref() != Some(ROLE_NONE));
    }
    if l.seen.removed {
        let line = match l.seen.removed_until.filter(|u| *u > i.now_ms) {
            Some(u) => format!("The bank was removed from the circle (it can be brought back until {}). Your copy stays as it is. Ask the circle's owner if it should come back.", day_month(u)),
            None => "The bank was removed from the circle. Your copy stays as it is.".to_string(),
        };
        return own_bank(line, true);
    }
    if i.sign_in == SignIn::Out {
        let when = l.role_seen_at.or(l.head_seen_at).map(|at| super::circles_text::ago(at, i.now_ms)).filter(|s| !s.is_empty());
        let line = match when {
            Some(w) => format!("Shared with {name}. Version {head} when last checked, {w}. Sign in to send or get newer versions."),
            None => format!("Shared with {name}. Version {head} when last checked. Sign in to send or get newer versions."),
        };
        return view(line, vec![OpenAccount]);
    }
    if i.check == Check::Offline {
        // (critic 64 I7, polish 3) said quietly: the bank works as usual
        return view("Couldn't check the circle just now. Your bank works as usual.".into(), vec![TryAgain]);
    }
    if stale {
        let mut b = vec![GetLatest];
        if can_send {
            b.push(TreatInStep);
        }
        b.push(Unlink);
        return warn(format!("This bank was put back from a saved copy, so verSHion can't tell what you changed since version {}.", l.base_version), b);
    }
    if let Some(a) = &i.act {
        // (critic 69 I1) the circle's newest version may be changing: never "up to date" meanwhile
        return StripView { line: format!("Shared with {name}. {a}"), buttons: Vec::new(), busy: true, warn: false, menu_history: true, menu_circle_page: true };
    }
    if i.sent_unread && can_send {
        // (critic 74 I1) the changes went in a version this copy has not caught up with: never counted as unsent;
        // (critic 76 I1) but what was changed after that send was not sent, and is never said to be
        if i.changed_since {
            return view(format!("Shared with {name}. Your earlier changes were sent. Changes you made since are not sent yet. Get the latest first, then send them."), vec![GetLatest, Send, History]);
        }
        return view(format!("Shared with {name}. Your changes were sent. Get the latest so that this copy has the version they made."), vec![GetLatest, History]);
    }
    if l.seen.under_review {
        // (critic 66 minor 3) Send only with something to send: with nothing, it would only replace the circle's version
        let something = !changes.known || changes.total() > 0;
        return view(format!("Shared with {name}. The circle's newest version is held back for review. You can keep working here."), if can_send && something { vec![Send] } else { Vec::new() });
    }
    let behind = l.behind() > 0;
    // (critic 63 B1) A copy that has its answer keys, of a bank that now hides them from viewers, read by a viewer:
    // the newest version would come without them, so Get latest is not offered (the merge refuses it too).
    if keys_kept(l) {
        return view(format!("Shared with {name} (you can view). Answer keys are hidden from viewers in this bank, but your copy has them. Get latest is turned off here so that you keep them."), vec![History]);
    }
    if l.viewers_form {
        let mut b = vec![GetLatest];
        let mut line = format!("Shared with {name}. Answer keys are hidden from viewers in this bank, so your copy has none.");
        if full_copy_offered(l, role_of(l)) {
            b.push(AddFullCopy);
            // (critic 66 minor 3) why a full copy is offered
            line.push_str(" You can edit now: add a full copy to get the answer keys and send changes.");
        }
        return view(line, b);
    }
    // (critic 73 minor 1) a restore or trim with no answer may still land: the version known is "when last checked"
    let latest = if i.unsure { format!("Version {head} when last checked.") } else { format!("Version {head} is the latest.") };
    let up_to_date = if i.unsure { "" } else { " Your copy is up to date." };
    // (critic 63 I3, critic 64 I2) A viewer's copy, the person an editor now: the copy is never changed in place.
    if full_copy_offered(l, role_of(l)) {
        let first = format!("Shared with {name}. {latest}");
        return view(format!("{first} You can send changes now, but this copy was added when you could only view. Add a full copy to send from it."), vec![AddFullCopy, GetLatest]);
    }
    if !can_send {
        let first = format!("Shared with {name} (you can view). {latest}");
        let n = if changes.known { changes.total() } else { 0 };
        if n > 0 {
            return view(format!("{first} You changed {} here. They stay on this computer; viewers can't send.", plural(n, "question", "questions")), vec![GetLatest]);
        }
        if behind {
            return view(format!("{first} You have version {}.", l.base_version), vec![GetLatest, History]);
        }
        return view(format!("{first}{up_to_date}"), vec![History]);
    }
    let first = format!("Shared with {name}. {latest}");
    if !changes.known {
        // (critic 88 I2) What is counted is said, with why a change to a difficulty may be missing from it, as the Send page
        // says it; a copy with no record at all says that. (critic 66 minor 5) Get latest only when there is something newer
        // to get, and then first, with the version the copy has (Q10).
        let n = if changes.partial() { changes.total() } else { 0 };
        let why = if changes.partial() { super::circles_text::difficulty_caveat(l.base_version) } else { super::circles_text::NO_RECORD.to_string() };
        let have = match (behind, n > 0) {
            (true, true) => format!(" You have version {} and {}.", l.base_version, plural(n, "unsent change", "unsent changes")),
            (true, false) => format!(" You have version {}.", l.base_version),
            (false, true) => format!(" You have {}.", plural(n, "unsent change", "unsent changes")),
            (false, false) => String::new(),
        };
        let then = if behind { " Get the latest first, then send." } else { "" };
        return view(format!("{first}{have} {why}{then}"), if behind { vec![GetLatest, Send, History] } else { vec![Send, History] });
    }
    let n = changes.total();
    match (behind, n > 0) {
        (true, true) => view(format!("{first} You have version {} and {}. Get the latest first, then send.", l.base_version, plural(n, "unsent change", "unsent changes")), vec![GetLatest, Send, History]),
        (true, false) => view(format!("{first} You have version {}.", l.base_version), vec![GetLatest, History]),
        (false, true) => view(format!("{first} You have {}.", plural(n, "unsent change", "unsent changes")), vec![Send, History]),
        (false, false) => view(format!("{first}{up_to_date}"), vec![History]),
    }
}

/// "Unlink?" asked on the strip (critic 67 M3): what it does, and the unsent changes it strands for good when the copy
/// can send (`unsent`: `None` when they can not be counted).
pub fn unlink_question(unsent: Option<usize>, can_send: bool) -> String {
    unlink_ask(unsent, can_send, false)
}

/// [`unlink_question`], also while a send from this bank is on its way (critic 69 minor 2): that send still reaches
/// the circle, so its changes are not said to be stranded.
pub fn unlink_ask(unsent: Option<usize>, can_send: bool, sending: bool) -> String {
    let stranded = match unsent {
        _ if !can_send => String::new(),
        _ if sending => " A send from this bank is on its way: what it carries still reaches the circle, but nothing you change after it can be sent.".to_string(),
        Some(0) => String::new(),
        Some(1) => " Your 1 unsent change can then never be sent to the circle.".to_string(),
        Some(n) => format!(" Your {n} unsent changes can then never be sent to the circle."),
        None => " Changes you have not sent can then never be sent to the circle.".to_string(),
    };
    format!("Unlink? The bank stays here as it is, but it is no longer connected to the circle.{stranded} Adding the circle's bank again later makes a new bank; this one stays unconnected.")
}

/// The links file is there but could not be read: no bank can be told apart, so every bank page says it.
pub fn unreadable_view() -> StripView {
    StripView { line: "The list of shared banks couldn't be read. Nothing was changed.".into(), buttons: vec![StripButton::TryAgain], busy: false, warn: true, menu_history: true, menu_circle_page: true }
}

// ---- adding and copying -------------------------------------------------------------------------------------------

/// "Add to My Banks": an edit link for the owner and editors, a follow link for viewers (Q2). A read of the
/// viewers' form (a bank that hides its keys) is a follow link that can never send, whatever the role.
pub fn add_mode(role: Role, keys_hidden: bool) -> (LinkMode, bool) {
    if keys_hidden {
        (LinkMode::Follow, true)
    } else if role.can_add_banks() {
        (LinkMode::Edit, false)
    } else {
        (LinkMode::Follow, false)
    }
}

/// "Add a full copy" is offered for this copy: it can never send (made from the viewers' form, or a viewer's copy),
/// while the person, as `role`, could send from a full copy (critic 63 I3, critic 64 I2). The copy is never
/// changed in place: a viewer's copy may hold a viewers' form merged after hiding was turned on.
pub fn full_copy_offered(l: &CircleLink, role: Role) -> bool {
    !l.can_send() && role.can_add_banks()
}

/// The join offer ticks every bank when there are 10 or fewer, none above that.
pub fn join_ticks(n: usize) -> bool {
    n <= 10
}

/// The lineage note of "Make my own copy" (Q7): where it came from, in the bank's details.
pub fn own_copy_line(circle: &str, title: &str, author: &str, version: u64, at_ms: i64) -> String {
    format!("From the circle \"{circle}\": \"{title}\" by {author}, version {version}, {}.", day_month_year(at_ms))
}

// ---- what a check says ----------------------------------------------------------------------------------------------

/// A short fingerprint of the account signed in (`"server\naddress"`), kept in a link instead of the address.
pub fn account_tag(account: Option<&str>) -> Option<String> {
    account.filter(|a| !a.is_empty()).map(|a| super::pool::sha256_hex(a.as_bytes()).chars().take(16).collect())
}

/// Writes what a check said into the link. Returns true when something changed (the list is saved).
/// `heads`: the answer of `POST /circles/heads` for this bank; `circles`: `GET /circles`.
pub fn apply_check(l: &mut CircleLink, heads: Option<&[HeadEntry]>, circles: Option<&[CircleSummary]>, now_ms: i64) -> bool {
    apply_check_as(l, heads, circles, now_ms, None).0
}

/// [`apply_check`] for the account `tag` (see [`account_tag`]). Also returns true as its second value when the
/// circle does not list this account while the link's role was seen by another one, or by none this computer
/// remembers (a link made by a share, or before phase D): then nothing is written, as that other account may well
/// still be in the circle (critic 63 I1, critic 65 minor).
pub fn apply_check_as(l: &mut CircleLink, heads: Option<&[HeadEntry]>, circles: Option<&[CircleSummary]>, now_ms: i64, tag: Option<&str>) -> (bool, bool) {
    if let (Some(c), Some(t)) = (circles, tag)
        && l.role_seen_by.as_deref() != Some(t)
        && !c.iter().any(|c| c.id == l.circle_id)
    {
        return (false, true);
    }
    let before = l.clone();
    if let Some(c) = circles {
        match c.iter().find(|c| c.id == l.circle_id) {
            Some(c) => {
                l.role_seen = Some(c.role().as_str().to_string());
                l.seen.circle_closed = c.closed();
                if let Some(t) = tag {
                    // (another account's role: whether it is the bank's author is learnt again for this account, and
                    // until then it is not taken from the other one; also the author of a link no account was
                    // remembered for, made by a share, critic 65 I2)
                    if l.role_seen_by.as_deref() != Some(t) && (l.role_seen_by.is_some() || l.is_author) {
                        l.author_known = false;
                        l.is_author = false;
                    }
                    l.role_seen_by = Some(t.to_string());
                }
            }
            None => {
                l.role_seen = Some(ROLE_NONE.to_string());
                l.seen.circle_closed = false;
            }
        }
        l.role_seen_at = Some(now_ms);
    }
    if let Some(e) = heads.and_then(|h| h.iter().find(|e| e.bank_id == l.circle_bank_id)) {
        match &e.state {
            HeadState::Current { version, at, by, hide_key, .. } => {
                l.seen.removed = false;
                l.seen.removed_until = None;
                l.seen.under_review = false;
                if let Some(on) = hide_key {
                    l.seen.hide_key = *on;
                }
                if *version >= l.head_seen && (*version != l.head_seen || l.head_seen_at != *at) {
                    l.head_seen = *version;
                    l.head_seen_at = *at;
                    l.head_seen_by = by.clone();
                }
            }
            HeadState::Removed { until } => {
                l.seen.removed = true;
                l.seen.removed_until = *until;
                l.seen.under_review = false;
            }
            HeadState::UnderReview => {
                l.seen.removed = false;
                l.seen.under_review = true;
            }
            // Left, removed, or the circle closed: the circles list says which. A bank the pool no longer has while
            // the circle is there and lists the person counts as removed.
            HeadState::Unavailable => {
                let member = circles.is_some_and(|c| c.iter().any(|c| c.id == l.circle_id && !c.closed()));
                if member {
                    l.seen.removed = true;
                    l.seen.removed_until = None;
                }
            }
        }
    }
    (*l != before, false)
}

/// Whether the person is the bank's author, from the circle's page (critic 63 I5). True when the link changed.
pub fn learn_author(l: &mut CircleLink, d: &CircleDetail) -> bool {
    let Some(b) = d.banks.iter().find(|b| b.id == l.circle_bank_id).filter(|_| d.circle.id == l.circle_id) else { return false };
    let changed = !l.author_known || l.is_author != b.is_author;
    l.is_author = b.is_author;
    l.author_known = true;
    changed
}

// ---- the quiet check -------------------------------------------------------------------------------------------------

struct CheckMsg {
    local: String,
    /// The account the check ran for (see [`account_tag`]).
    tag: Option<String>,
    heads: Result<Vec<HeadEntry>, PoolError>,
    circles: Result<Vec<CircleSummary>, PoolError>,
    /// The circle's page, read only for a link that does not know whether the person is the bank's author.
    detail: Option<Result<CircleDetail, PoolError>>,
}

struct CheckJob {
    local: String,
    rx: Receiver<CheckMsg>,
}

type Repaint = Option<Arc<dyn Fn() + Send + Sync>>;

#[allow(clippy::too_many_arguments)]
async fn check_async(session: SharedSession, retry: RetryPolicy, local: String, bid: String, author_of: Option<String>, tag: Option<String>, tx: Sender<CheckMsg>, repaint: Repaint) {
    let pool = Pool::new(&*session).with_retry(retry);
    let heads = pool.circle_heads(std::slice::from_ref(&bid)).await;
    // (a sign-in that ended is not asked twice)
    let circles = match &heads {
        Err(PoolError::Api(ApiError::SignedOut)) => Err(PoolError::Api(ApiError::SignedOut)),
        _ => pool.circles().await,
    };
    // (critic 63 I5) Whether the person is the bank's author, once, for a link from before phase D, while a member.
    let detail = match (&circles, author_of) {
        (Ok(c), Some(cid)) if c.iter().any(|c| c.id == cid) => Some(pool.circle(&cid).await),
        _ => None,
    };
    let _ = tx.send(CheckMsg { local, tag, heads, circles, detail });
    if let Some(r) = repaint {
        r();
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::too_many_arguments)]
fn check_run(session: SharedSession, retry: RetryPolicy, local: String, bid: String, author_of: Option<String>, tag: Option<String>, tx: Sender<CheckMsg>, repaint: Repaint) {
    super::transport::block_on(check_async(session, retry, local, bid, author_of, tag, tx, repaint));
}

/// The checks of linked banks and what they said, in memory. Lives beside the circles model.
pub struct BankLinks {
    runner: Runner,
    retry: RetryPolicy,
    repaint: Repaint,
    /// When each bank was last asked about (its page was open), for the 10-minute rule.
    last: HashMap<String, Instant>,
    state: HashMap<String, Check>,
    jobs: Vec<CheckJob>,
    /// "Check now" was pressed: asked even for a copy whose circle no longer lists the person (they may be back).
    asked: std::collections::HashSet<String>,
    /// The sign-in ended during a check.
    signed_out: bool,
    /// A check was answered "signed out" while the account still looks signed in: no check starts, and the strip
    /// says "Sign in", until the account changes or the person asks (critic 63 I2).
    out: bool,
    /// Links the circle no longer lists the person for, checked once since this account signed in (critic 63 I1).
    rechecked: std::collections::HashSet<String>,
    /// Links whose circle page was read to learn whether the person is the bank's author, once since this account
    /// signed in (or since "Check now"): a bank the page does not list is not asked about every 10 minutes.
    author_asked: std::collections::HashSet<String>,
}

impl Default for BankLinks {
    fn default() -> Self {
        BankLinks::new(Runner::Background)
    }
}

impl BankLinks {
    /// Does no I/O.
    pub fn new(runner: Runner) -> BankLinks {
        BankLinks { runner, retry: RetryPolicy::default(), repaint: None, last: HashMap::new(), state: HashMap::new(), jobs: Vec::new(), asked: Default::default(), signed_out: false, out: false, rechecked: Default::default(), author_asked: Default::default() }
    }

    pub fn set_retry(&mut self, retry: RetryPolicy) {
        self.retry = retry;
    }

    pub fn set_repaint(&mut self, f: Arc<dyn Fn() + Send + Sync>) {
        self.repaint = Some(f);
    }

    pub fn busy(&self) -> bool {
        !self.jobs.is_empty()
    }

    pub fn take_signed_out(&mut self) -> bool {
        std::mem::take(&mut self.signed_out)
    }

    /// Another account signed in, or the account signed out: what was read goes, and the next page asks again.
    pub fn forget(&mut self) {
        self.last.clear();
        self.state.clear();
        self.jobs.clear();
        self.asked.clear();
        self.signed_out = false;
        self.out = false;
        self.rechecked.clear();
        self.author_asked.clear();
    }

    /// The person joined the circle (again): its links are checked again at once, also a copy the circle had said
    /// they were no longer in (critic 65 I1).
    pub fn rearm(&mut self, locals: &[String]) {
        for l in locals {
            self.last.remove(l);
            self.rechecked.remove(l);
            if self.state.get(l) != Some(&Check::Checking) {
                self.state.remove(l);
            }
        }
    }

    /// One of the circle's screens met an ended session: the strip says "Sign in..." and starts no check until the
    /// account changes or the person asks (critic 65 minor, critic 66 I7).
    pub fn session_ended(&mut self) {
        self.out = true;
    }

    /// The badge of a bank on the Banks list: this computer only, never a request.
    pub fn badge(&mut self, edit: &mut CircleEditModel, bank: &QuestionBank) -> Option<Badge> {
        let status = edit.status_of(bank)?;
        let unread = edit.sent_unread_of(bank);
        edit.with_link(&bank.id, |l| badge_of_sent(l, Some(&status), unread))
    }

    /// The check of this bank is due: none ran in the last 10 minutes and none is on its way.
    pub fn due(&self, local: &str, now: Instant) -> bool {
        !self.jobs.iter().any(|j| j.local == local) && self.last.get(local).is_none_or(|t| now.saturating_duration_since(*t) >= CHECK_EVERY)
    }

    /// A check of this bank would start now (see [`strip`](Self::strip)); `gone`: the link says the person is no
    /// longer in the circle.
    pub fn will_check(&self, local: &str, gone: bool, now: Instant) -> bool {
        !self.out && self.due(local, now) && (!gone || self.asked.contains(local) || !self.rechecked.contains(local))
    }

    /// "Check now": the next frame of the page asks again, whatever the time.
    pub fn check_now(&mut self, local: &str) {
        self.out = false;
        self.last.remove(local);
        self.author_asked.remove(local);
        self.asked.insert(local.to_string());
        if self.state.get(local) == Some(&Check::Offline) {
            self.state.remove(local);
        }
    }

    /// (critic 82 I1) The circle answered for this bank (a send, Get latest...): a strip that could not check it does not
    /// go on saying so; it checks again at once.
    pub fn answered(&mut self, local: &str) {
        if self.state.get(local) == Some(&Check::Offline) {
            self.check_now(local);
        }
    }

    fn start(&mut self, session: SharedSession, local: &str, bid: &str, author_of: Option<String>, tag: Option<String>, now: Instant) {
        self.last.insert(local.to_string(), now);
        self.state.insert(local.to_string(), Check::Checking);
        let (tx, rx) = channel::<CheckMsg>();
        let (retry, repaint, local_s, bid_s, tag_s, cid_s) = (self.retry, self.repaint.clone(), local.to_string(), bid.to_string(), tag, author_of);
        // (critic 71 minor 3) kept to the account signed in now: a retry after another account signed in is not sent as it
        let session = super::pool::pinned(session);
        match self.runner {
            #[cfg(not(target_arch = "wasm32"))]
            Runner::Inline => check_run(session, retry, local_s, bid_s, cid_s, tag_s, tx, repaint),
            #[cfg(target_arch = "wasm32")]
            Runner::Inline => wasm_bindgen_futures::spawn_local(async move { check_async(session, retry, local_s, bid_s, cid_s, tag_s, tx, repaint).await }),
            #[cfg(not(target_arch = "wasm32"))]
            Runner::Background => {
                std::thread::spawn(move || check_run(session, retry, local_s, bid_s, cid_s, tag_s, tx, repaint));
            }
            #[cfg(target_arch = "wasm32")]
            Runner::Background => wasm_bindgen_futures::spawn_local(async move { check_async(session, retry, local_s, bid_s, cid_s, tag_s, tx, repaint).await }),
        }
        self.jobs.push(CheckJob { local: local.to_string(), rx });
    }

    /// Applies finished checks to the links (and saves them). Called every frame of a linked bank's page.
    pub fn frame(&mut self, edit: &mut CircleEditModel, now_ms: i64) {
        let mut done = Vec::new();
        let mut lost = Vec::new();
        for (i, j) in self.jobs.iter().enumerate() {
            match j.rx.try_recv() {
                Ok(m) => done.push((i, m)),
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => lost.push(i),
            }
        }
        let gone: Vec<usize> = done.iter().map(|(i, _)| *i).chain(lost.iter().copied()).collect();
        for i in &lost {
            if let Some(j) = self.jobs.get(*i) {
                self.state.insert(j.local.clone(), Check::Offline);
            }
        }
        for (_, m) in done {
            let signed_out = matches!(m.heads, Err(PoolError::Api(ApiError::SignedOut))) || matches!(m.circles, Err(PoolError::Api(ApiError::SignedOut)));
            let ok = m.heads.is_ok() && m.circles.is_ok();
            let heads = m.heads.as_ref().ok().map(Vec::as_slice);
            let circles = m.circles.as_ref().ok().map(Vec::as_slice);
            if let Some(h) = heads {
                for e in h {
                    if let HeadState::Current { hide_key: Some(on), .. } = &e.state {
                        edit.note_hiding(&e.bank_id, *on);
                    }
                }
            }
            let mut foreign = false;
            let before = edit.with_link(&m.local, |l| l.head_seen);
            if heads.is_some() || circles.is_some() {
                edit.update_link(&m.local, |l| {
                    let (changed, other) = apply_check_as(l, heads, circles, now_ms, m.tag.as_deref());
                    foreign = other;
                    changed
                });
            }
            // (critic 73 minors 1, 6) the newest version is known again; a newer one makes some results out of date
            if heads.is_some() && !foreign {
                let after = edit.with_link(&m.local, |l| l.head_seen);
                edit.checked(&m.local, after > before);
            }
            if let Some(Ok(d)) = &m.detail
                && !foreign
            {
                edit.update_link(&m.local, |l| learn_author(l, d));
            }
            if signed_out {
                self.signed_out = true;
                self.out = true;
                self.state.remove(&m.local);
            } else {
                self.state.insert(m.local.clone(), if foreign { Check::OtherAccount } else if ok { Check::Done } else { Check::Offline });
            }
        }
        let mut keep = Vec::new();
        for (i, j) in std::mem::take(&mut self.jobs).into_iter().enumerate() {
            if !gone.contains(&i) {
                keep.push(j);
            }
        }
        self.jobs = keep;
    }

    /// The strip of a linked bank's page (`None` for a bank with no link). Starts the check when it is due and the
    /// account is signed in: the only place a request starts.
    pub fn strip(&mut self, edit: &mut CircleEditModel, bank: &QuestionBank, snap: &AccountSnapshot, now: Instant, now_ms: i64) -> Option<StripView> {
        let status = edit.status_of(bank)?;
        let tag = account_tag(snap.account.as_deref());
        // (critic 63 I5, critic 65 I2) Whether the person is the bank's author: read when not known, or known for
        // another account (see `apply_check_as`); once per sign-in.
        let other = |l: &CircleLink| tag.is_some() && l.role_seen_by != tag && (l.role_seen_by.is_some() || l.is_author);
        let ids = edit.with_link(&bank.id, |l| (l.circle_bank_id.clone(), l.gone(), (!l.author_known || other(l)).then(|| l.circle_id.clone())))?;
        let sign_in = if self.out { SignIn::Out } else { sign_in_of(&snap.gate) };
        if let (SignIn::Ready, Some(session)) = (sign_in, snap.session.clone())
            && self.will_check(&bank.id, ids.1, now)
        {
            self.asked.remove(&bank.id);
            if ids.1 {
                self.rechecked.insert(bank.id.clone());
            }
            let author_of = ids.2.clone().filter(|_| self.author_asked.insert(bank.id.clone()));
            self.start(session, &bank.id, &ids.0, author_of, tag, now);
        }
        let mut check = self.state.get(&bank.id).cloned().unwrap_or_default();
        if snap.gate == Gate::Unreachable && check != Check::Checking {
            check = Check::Offline;
        }
        let act = edit.act_line(&bank.id);
        let unread = edit.sent_unread_of(bank);
        let (sent_unread, changed_since, unsure) = (unread.is_some(), unread == Some(true), edit.unsure(&bank.id));
        edit.with_link(&bank.id, |l| strip_view(&StripInput { link: l, status: Some(&status), check, sign_in, now_ms, act, sent_unread, changed_since, unsure }))
    }

    /// The last check's state for a bank (tests, the page's spinner).
    pub fn check_of(&self, local: &str) -> Check {
        self.state.get(local).cloned().unwrap_or_default()
    }
}
