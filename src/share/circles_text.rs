//! Words for the Circles tab. Everything the server supplies is passed through [`api::plain`] by
//! the caller; the sentences here are the app's own.

use super::api;
use super::circles_api::{ActivityEntry, Role};

pub const EMPTY_LIST_TITLE: &str = "You are not in any circle yet";
pub const EMPTY_LIST_HELP: &str = "A circle is a small private group of teachers who share banks with each other. Create one, or enter a code someone gave you.";
pub const EMPTY_BANKS: &str = "No banks in this circle yet.";
pub const EMPTY_BANKS_HELP_EDITOR: &str = "Share a bank to the circle from the Banks page (Share to pool..., then choose a circle), or with the button above.";
pub const EMPTY_BANKS_HELP_VIEWER: &str = "An owner or editor can add banks. You can look at and copy anything they add.";
pub const EMPTY_ACTIVITY: &str = "Nothing has happened in this circle yet.";
pub const NO_CODES: &str = "No codes yet. Make one to invite another teacher.";

pub const CODE_SHOWN_ONCE: &str = "Anyone with this code can join until it expires or is used up. It is shown only now: copy it before you close this.";
pub const CODE_PASTE_HELP: &str = "Paste the code (or the message you were sent). Nothing is used up until you press Join.";
pub const ANSWER_KEYS_NOTE: &str = "The questions are shown here without their answers. Everyone in this circle can see the answer keys: they come with every copy.";
pub const COPY_LINE: &str = "A copy is private to you and made on this computer. The circle's bank is not changed, and nothing you change in your copy goes back to it.";
pub const LEAVE_MEMBER: &str = "You will lose access to this circle's banks. Copies you already made stay on your computer. You can come back with a new code.";
pub const LEAVE_OWNER: &str = "You own this circle. If you leave, it passes to the member who has been here longest (an editor first). If nobody else is in it, the circle closes.";
pub const CLOSE_CIRCLE: &str = "Everyone loses access in 14 days. Copies on their computers stay. Until then you can reopen it.";
pub const REMOVE_BANK: &str = "The bank disappears for everyone in the circle. You can bring it back for 14 days; after that it is deleted. Copies people made stay with them.";
pub const REPORT_THANKS: &str = "Thank you. Your report goes to the people who run the pool. The author is not told who sent it.";
pub const TRANSFER_NOTE: &str = "This is an offer: they have 7 days to accept. Until they do you stay the owner. If they accept, they become the owner and you become an editor.";
pub const NOT_REACHED: &str = "Could not reach the pool.";
pub const HIDDEN_BANK_TITLE: &str = "Hidden while a review is open";
pub const HIDDEN_BANK_LINE: &str = "Its title and author are not shown while it is reviewed, or while its author's account is.";
pub const PURGE_BANK: &str = "This deletes the bank for good now, instead of after its 14 days. It can not be brought back.";
pub const HELD_BANK: &str = "This bank is on hold while the pool's reviewers look at it. Only you can see it; the other members cannot until it is cleared.";
pub const LICENSE_SETTING_NOTE: &str = "This is the licence you attach to the bank. Not yet: sharing a circle bank's questions publicly is not available in this version, whatever the licence. A later version may let members do it for Creative Commons banks. Earlier versions keep the licence they were shared under.";

/// A role on a pill: "Owner", "Editor", "Viewer".
pub fn role_title(r: Role) -> &'static str {
    match r {
        Role::Owner => "Owner",
        Role::Editor => "Editor",
        Role::Viewer => "Viewer",
    }
}

pub fn role_name(r: &str) -> &'static str {
    Role::parse(r).name()
}

pub fn role_choice_name(r: Role) -> &'static str {
    match r {
        Role::Owner => "Owner",
        Role::Editor => "Can add banks",
        Role::Viewer => "Can only look and copy",
    }
}

pub fn members_text(n: u32) -> String {
    if n == 1 { "1 member".into() } else { format!("{n} members") }
}

pub fn banks_text(n: u32) -> String {
    if n == 1 { "1 bank".into() } else { format!("{n} banks") }
}

pub fn questions_text(n: u32) -> String {
    if n == 1 { "1 question".into() } else { format!("{n} questions") }
}

/// "5 minutes ago" or "2 days ago" (or "today", "in the future" is shown as "just now").
pub fn ago(ms: i64, now: i64) -> String {
    if ms <= 0 {
        return String::new();
    }
    let s = ((now - ms) / 1000).max(0);
    match s {
        0..=59 => "just now".into(),
        60..=3599 => {
            let m = s / 60;
            if m == 1 { "1 minute ago".into() } else { format!("{m} minutes ago") }
        }
        3600..=86_399 => {
            let h = s / 3600;
            if h == 1 { "1 hour ago".into() } else { format!("{h} hours ago") }
        }
        _ => {
            let d = s / 86_400;
            if d == 1 { "yesterday".into() } else if d < 60 { format!("{d} days ago") } else { api::date_text(ms) }
        }
    }
}

/// "in 6 days" / "tomorrow" / "today" for an expiry.
pub fn until(ms: i64, now: i64) -> String {
    let s = (ms - now) / 1000;
    if s <= 0 {
        return "now".into();
    }
    let d = s / 86_400;
    match d {
        0 => {
            let h = (s / 3600).max(1);
            if h == 1 { "in 1 hour".into() } else { format!("in {h} hours") }
        }
        1 => "tomorrow".into(),
        _ => format!("in {d} days"),
    }
}

pub fn state_label(state: &str) -> &'static str {
    match state {
        "active" => "Active",
        "expired" => "Expired",
        "used_up" => "Used up",
        "revoked" => "Withdrawn",
        _ => "Unknown",
    }
}

/// One line of the activity list. Names come from the server and are cleaned.
pub fn activity_line(e: &ActivityEntry) -> String {
    let by = e.by.as_deref().map(api::plain).filter(|s| !s.is_empty()).unwrap_or_else(|| "Someone".into());
    let who = e.member.as_deref().map(api::plain).filter(|s| !s.is_empty()).unwrap_or_else(|| "a member".into());
    let title = e.bank_title.as_deref().map(api::plain).filter(|s| !s.is_empty()).map(|t| format!("\"{t}\"")).unwrap_or_else(|| "a bank".into());
    let role = e.role.as_deref().map(role_name).unwrap_or("member");
    match e.what.as_str() {
        "created" => format!("{by} made this circle."),
        "joined" => format!("{who} joined as {role}."),
        "left" => format!("{who} left."),
        "removed" => format!("{by} removed {who}."),
        "role_changed" => format!("{by} made {who} {}.", if role == "member" { "a different role" } else { role }),
        "ownership_transferred" => format!("{by} handed the circle to {who}."),
        "ownership_passed_on" => format!("The circle passed to {who}."),
        "closed" => format!("{by} closed the circle."),
        "reopened" => format!("{by} reopened the circle."),
        "renamed" => format!("{by} renamed the circle."),
        "bank_shared" => format!("{by} shared {title}."),
        "version_sent" => format!("{by} sent a new version of {title}{}.", e.version.map(|v| format!(" (version {v})")).unwrap_or_default()),
        "bank_removed" => format!("{by} removed {title}."),
        "bank_restored" => format!("{by} brought back {title}."),
        "license_changed" => format!("{by} changed the re-share setting of {title}."),
        _ => format!("{by} did something this version of verSHion does not know about."),
    }
}

/// What the licence of a circle bank means for re-sharing, in one line (shown after the licence's name).
pub fn reshare_line(license: &str) -> &'static str {
    // Today the pool refuses every public publish of circle text, whatever the licence (members' re-sharing of
    // Creative Commons circle banks waits for a later version), so no licence promises it.
    match license {
        "circle-only" => "Stays inside the circle.",
        _ => "Not shared publicly from here yet.",
    }
}

/// A grade as a person reads it: "8" becomes "Grade 8"; "Grade 8" stays; nothing stays nothing.
pub fn grade_text(grade: &str) -> String {
    let g = super::api::plain(grade);
    let g = g.trim();
    if g.is_empty() {
        String::new()
    } else if g.to_lowercase().starts_with("grade") || g.to_lowercase().starts_with("gr.") {
        g.to_string()
    } else {
        format!("Grade {g}")
    }
}

/// How long is left until `ms`: "6 days", "1 day", "5 hours", "1 hour" (for "for another ...").
pub fn remaining(ms: i64, now: i64) -> String {
    let s = ((ms - now) / 1000).max(0);
    let d = s / 86_400;
    if d >= 1 {
        return if d == 1 { "1 day".into() } else { format!("{d} days") };
    }
    let h = (s / 3600).max(1);
    if h == 1 { "1 hour".into() } else { format!("{h} hours") }
}

/// The 14-day undo, as a line.
pub fn undo_line(until_ms: Option<i64>, now: i64) -> String {
    match until_ms {
        Some(u) if u > now => format!("It can be brought back for another {}.", remaining(u, now)),
        Some(_) => "The time to undo has passed.".into(),
        None => String::new(),
    }
}
