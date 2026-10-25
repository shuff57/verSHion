//! Words for the Circles tab. Everything the server supplies is passed through [`api::plain`] by
//! the caller; the sentences here are the app's own.

use super::api;
use super::circles_api::{ActivityEntry, HideKeyResult, Role};

pub const EMPTY_LIST_TITLE: &str = "You are not in any circle yet";
pub const EMPTY_LIST_HELP: &str = "A circle is a small private group of teachers who share banks with each other. Create one, or enter a code someone gave you.";
pub const EMPTY_BANKS: &str = "No banks in this circle yet.";
pub const EMPTY_BANKS_HELP_EDITOR: &str = "Share a bank to the circle from the Banks page (Share to pool..., then choose a circle), or with the button above.";
pub const EMPTY_BANKS_HELP_VIEWER: &str = "An owner or editor can add banks. You can look at and copy anything they add.";
pub const EMPTY_ACTIVITY: &str = "Nothing has happened in this circle yet.";
pub const NO_CODES: &str = "No codes yet. Make one to invite another teacher.";

pub const CODE_SHOWN_ONCE: &str = "Anyone with this code can join until it expires or is used up. It is shown only now: copy it before you close this.";
pub const CODE_PASTE_HELP: &str = "Paste the code, or the link or the whole message from an invitation email. Nothing is used up until you press Join.";
pub const ANSWER_KEYS_NOTE: &str = "The questions are shown here without their answers. Everyone in this circle can see the answer keys: they come with every copy.";
pub const COPY_LINE: &str = "A copy is private to you and made on this computer. The circle's bank is not changed, and nothing you change in your copy goes back to it.";
pub const WORKING_COPY_LINE: &str = "A working copy stays linked to the circle's bank. It is private to you and on this computer, and the circle is changed only when you press Send. You can also get the newer versions others send, and merge them in when you choose.";
pub const HAS_WORKING_COPY: &str = "You already have a working copy of this bank (see Versions, below), so this makes a plain copy that is not linked to the circle.";
pub const WORKING_COPY_DONE: &str = "Your working copy is under Versions, below: send your changes from there, or get the circle's newer versions.";
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
// ---- phase C: invite by email ----------------------------------------------------------------------------------

pub const EMAIL_HELP: &str = "verSHion mails the teacher an invitation with a code that works once, for 7 days. The mail is a fixed text: it does not name you or the circle (the app shows both before they join).";
pub const EMAIL_NEEDS_ACCOUNT: &str = "To join, they need a verSHion account that an administrator has approved and that has redeemed an invite code for the shared pool. Until then the code can not be used.";
pub const EMAIL_PRIVACY: &str = "The address is not kept: it is used for this one mail and cleared from this form once it is sent. The list below shows only a masked label (like j***@school.org) so you can tell invitations apart.";
pub const EMAIL_PACE: &str = "Invitations go out at a steady pace. If you invite many colleagues at one school, some may have to wait for the next hour; the app says so, and nothing is lost.";
pub const EMAIL_RESENT: &str = "This address already had an open invitation from this circle: a new code was mailed, and the earlier one no longer works.";

// ---- phase C: answer keys hidden from viewers ----------------------------------------------------------------------

pub const ANSWER_KEYS_HIDDEN_NOTE: &str = "The questions are shown here without their answers. This bank hides its answer keys from viewers: editors and the owner see the full bank; viewers get it without the correct answers and explanations.";
pub const KEYS_HIDDEN_VIEWER: &str = "The answer keys are hidden for viewers. You see these questions without their correct answers and explanations, and pictures may be left out. The bank's author or the circle's owner can show them again.";
pub const COPY_NO_KEYS: &str = "Your copy will have no answer keys: the questions arrive marked \"No answer key\", and you add the answers yourself before printing a key. It is a plain copy, not linked to the circle.";
pub const COPY_NO_KEYS_DONE: &str = "This copy has no answer keys. Each question is marked \"No answer key\"; add the answers yourself before you print a key.";
pub const HIDE_SECTION: &str = "Answer keys for viewers";
pub const HIDE_OFF_LINE: &str = "Everyone in this circle sees the answer keys, viewers too.";
pub const HIDE_ON_LINE: &str = "Viewers get this bank without its answer keys. Editors and the owner still see everything, and can edit it as before.";
pub const HIDE_ON_ASK: &str = "Viewers will get every version of this bank without the correct answers, the explanations and the pictures, and with the choices shuffled. Editors and the owner keep seeing the full bank. If the newest version has pictures, you choose next which of them viewers may keep. The pool first checks the viewers' copy; it may ask you to decide about letter labels (A, B, C ...) or choice orders.";
pub const HIDE_OFF_ASK: &str = "Viewers will see the answer keys again, in every version of this bank.";
/// The bank page when the newest version of a bank that hides its keys is held back and the pool said turning
/// hiding off releases it (critic 58 B2, critic 59 B1).
pub const HIDE_HELD_HEAD: &str = "The newest version of this bank is held back while an administrator reviews it, so it can not be opened. It was held back when the answer keys were hidden from viewers: turning hiding off makes it readable again.";
/// Hiding was turned off and the bank, read again, still has its newest version held back (critic 60 B2).
pub const HIDE_OFF_STILL_HELD: &str = "Turning hiding off did not release the newest version. It is still held back, and only an administrator can release it.";
/// The picture step (before hiding is turned on, and before a send to a bank that hides its keys).
pub const PICTURES_HEADING: &str = "Pictures";
pub const PICTURES_HELP: &str = "Viewers get a picture only if you keep it for them. A picture you keep is shown to viewers as it is, so keep it only if it does not show an answer or a hint.";
pub const PICTURES_BANK_INTRO: &str = "The newest version of this bank has pictures. Choose for each one whether viewers get it. Nothing has changed yet.";
pub const PICTURES_SEND_INTRO: &str = "This bank hides its answer keys from viewers, so the pool makes a viewers' copy of what you send. Choose for each picture whether viewers get it. Nothing was sent yet; your changes are still on this computer.";
pub const PICTURE_KEEP: &str = "Keep it for viewers";
pub const PICTURE_DROP: &str = "Drop it";
/// The confirmation's button when the picture step comes next (critic 62 minor: one "Hide the keys", on the step).
pub const HIDE_NEXT: &str = "Next";
/// While the keys are hidden: choosing again which pictures viewers get (critic 62 I2). The app does not know what
/// was chosen before, so every picture starts as dropped; each save is a change of the setting for the day's count.
pub const PICTURES_CHANGE: &str = "Change picture choices";
pub const PICTURES_SAVE: &str = "Save picture choices";
pub const PICTURES_CHANGE_INTRO: &str = "Choose again for each picture of the newest version whether viewers get it. Every picture starts as dropped here, whatever was chosen before. Nothing has changed yet. Saving new choices counts as a change of the answer-key setting, and there is a daily number of those.";
pub const PICTURES_CHANGE_DECIDE: &str = "Before the new picture choices can be saved, the pool needs these decisions about the copy viewers will get. Nothing has changed yet.";

/// A new version arrived before a change of the setting went through (the head read right before it moved, or the
/// pool's `409 conflict` on the PATCH pinned to the version, critic 62 B1): nothing changed, and whatever was decided
/// for the older version is dropped. `change`: it came from "Change picture choices".
pub fn hide_moved_message(decisions: bool, pictures: bool, change: bool) -> String {
    let again = if change { "press Change picture choices again to choose for the new version" } else { "press the switch again for the new version" };
    let dropped = match (decisions, pictures) {
        (true, true) => " Your decisions and picture choices were dropped: they were for the older version.",
        (true, false) => " Your decisions were dropped: they were for the older version.",
        (false, true) => " Your picture choices were dropped: they were for the older version.",
        (false, false) => "",
    };
    format!("A new version of this bank arrived meanwhile, so nothing was changed.{dropped} The bank was read again: {again}.")
}

/// "questions 2 and 4" (0-based positions in, numbers out).
pub fn question_numbers(qs: &[usize]) -> String {
    let n: Vec<String> = qs.iter().map(|q| (q + 1).to_string()).collect();
    match n.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("questions {} and {last}", rest.join(", ")),
        Some((last, _)) => format!("question {last}"),
        None => String::new(),
    }
}

/// A send to a bank this computer did not know hides its answer keys: the step was not shown, and the viewers' copy
/// left out these pictures (critic 62 I5). `ask_next`: the next send asks (the bank is known to hide them now).
pub fn pictures_not_asked_line(qs: &[usize], ask_next: bool) -> Option<String> {
    let next = if ask_next { " Send again to choose which pictures viewers get." } else { "" };
    match qs.len() {
        0 => None,
        1 => Some(format!("The picture of {} was left out of the viewers' copy. You were not asked about it: this computer did not know the bank hides its answer keys.{next}", question_numbers(qs))),
        _ => Some(format!("The pictures of {} were left out of the viewers' copy. You were not asked about them: this computer did not know the bank hides its answer keys.{next}", question_numbers(qs))),
    }
}

/// A send made on the picture step, to a bank that turned out not to hide its answer keys any more: viewers get the
/// version in full (critic 62 I1). `dropped`: the pictures the person chose to drop.
pub fn hiding_stopped_line(dropped: &[usize]) -> String {
    let also = match dropped.len() {
        0 => String::new(),
        1 => format!(", the picture of {} that you chose to drop too", question_numbers(dropped)),
        _ => format!(", the pictures of {} that you chose to drop too", question_numbers(dropped)),
    };
    format!("This bank no longer hides its answer keys from viewers, so viewers get this version in full: its answer keys and every picture{also}.")
}

/// Pictures the viewers' copy left out that the person did not choose to drop, in one plain sentence.
pub fn pictures_left_out_line(qs: &[usize]) -> Option<String> {
    match qs.len() {
        0 => None,
        1 => Some(format!("The picture of {} was left out of the viewers' copy, although you did not choose to drop it. Viewers do not see it.", question_numbers(qs))),
        _ => Some(format!("The pictures of {} were left out of the viewers' copy, although you did not choose to drop them. Viewers do not see them.", question_numbers(qs))),
    }
}

pub const HIDE_TRIES: &str = "Each press counts toward a daily number of tries, also when the pool asks for decisions, so decide everything listed before you press again.";

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
        "email_invited" => format!("{by} emailed an invitation to join as {}.", if role == "member" { "a member" } else { role }),
        "hide_key_changed" => match e.hide_key {
            Some(false) => format!("{by} showed the answer keys of {title} to viewers again."),
            _ => format!("{by} hid the answer keys of {title} from viewers."),
        },
        "rolled_back" => format!("{by} restored an older version of {title}{}.", e.rollback_of.map(|v| format!(" (version {v})")).unwrap_or_default()),
        "versions_trimmed" => format!("{by} trimmed the older versions of {title}."),
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

/// "Delete for good now" done only in part (round 48): honest about what is left and that Undo is gone.
pub fn purge_partial_line(evidence_kept: u64) -> String {
    let kept = match evidence_kept {
        0 => "Some versions are".to_string(),
        1 => "1 version is".to_string(),
        n => format!("{n} versions are"),
    };
    format!("Mostly deleted. {kept} kept while an administrator reviews them; everything else in the bank was deleted now. Undo is no longer possible, and the rest is deleted once the review is done.")
}

/// The line on a removed bank's card after a partial purge.
pub fn purge_pending_line(evidence_kept: u64) -> String {
    let kept = match evidence_kept {
        0 => "Some versions are".to_string(),
        1 => "1 version is".to_string(),
        n => format!("{n} versions are"),
    };
    format!("Mostly deleted: {kept} kept while an administrator reviews them. It can not be brought back; the rest is deleted once the review is done.")
}

/// What kind of invitation a line of the owner's list is.
pub fn invite_kind(emailed: bool) -> &'static str {
    if emailed { "Emailed" } else { "Code" }
}

/// What a change of the answer-key setting did, in words (the first line is the headline).
pub fn hide_result_lines(r: &HideKeyResult, head_held: bool) -> Vec<String> {
    let mut out = Vec::new();
    if r.unchanged {
        out.push(if r.hide_key { "Nothing changed: the answer keys were already hidden from viewers, with these decisions.".into() } else { "Nothing changed: viewers already see the answer keys.".into() });
        return out;
    }
    if !r.hide_key {
        // (critic 59 B1 item 4, critic 60 B2: the bank read again after the change, its newest version still held
        // back; critic 61 M3: then viewers see nothing of the bank, so that is the headline, not "they see the keys")
        out.push(if head_held { HIDE_OFF_STILL_HELD.into() } else { "Viewers see the answer keys again, in every version of this bank.".into() });
        match r.restored.len() {
            0 => {}
            1 => out.push(format!("Version {} had been held back when the keys were hidden; it can be read again.", r.restored[0])),
            _ => out.push(format!("Versions {} had been held back when the keys were hidden; they can be read again.", list_versions(&r.restored))),
        }
        return out;
    }
    out.push("The answer keys are now hidden from viewers. They get every version without the correct answers and explanations; editors and the owner still see everything.".into());
    let mut bits = Vec::new();
    let n = |k: usize, one: &str, many: &str| if k == 1 { format!("1 {one}") } else { format!("{k} {many}") };
    if r.labels_removed > 0 {
        bits.push(n(r.labels_removed, "set of letter labels was stripped", "sets of letter labels were stripped"));
    }
    if r.images_kept > 0 {
        bits.push(n(r.images_kept, "picture was kept", "pictures were kept"));
    }
    if r.images_removed > 0 {
        bits.push(n(r.images_removed, "picture was left out", "pictures were left out"));
    }
    if r.figures_removed > 0 {
        bits.push(n(r.figures_removed, "figure was left out", "figures were left out"));
    }
    if !bits.is_empty() {
        out.push(format!("In the viewers' copy: {}.", bits.join(", ")));
    }
    out.extend(pictures_left_out_line(&r.left_out_unasked));
    if r.withheld || r.held {
        // (critic 60 B1: whether turning hiding off makes it readable again is the pool's to say, on the bank's read:
        // HIDE_HELD_HEAD says it only then)
        out.push("The viewers' copy matched something the pool checks for, so the newest version is held back and the bank is hidden from the circle while an administrator reviews it. The questions are safe.".into());
    }
    out
}

fn list_versions(vs: &[u64]) -> String {
    match vs.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.iter().map(u64::to_string).collect::<Vec<_>>().join(", ")),
        Some((last, _)) => last.to_string(),
        None => String::new(),
    }
}
