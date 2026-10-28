//! Words for the Circles tab. Everything the server supplies is passed through [`api::plain`] by
//! the caller; the sentences here are the app's own.

use super::api;
use super::circles_api::{ActivityEntry, HideKeyResult, Role};

/// The public pool's words in a sentence shown on a circle screen (critic 66 I7, critic 68 minor 1): verSHion's
/// instead, and verSHion's server for what the server did (critic 70 I1: never "the circle", which would blame the
/// teacher's colleagues for a server fault or a limit). One server error reads the same on every circle screen.
pub fn circle_words(note: &str) -> String {
    // (critic 78 I3) The account's own invite code (an administrator's) is not the circle's code: a circle screen says
    // which code is meant, so the teacher never takes the circle's code they just entered for the wrong one.
    // Circles not set up on the server is said of circles, not of "this circle".
    // (owner decision, round 7) any other "shared pool" names the circle on a circle's screen
    note.replace(POOL_INVITE_REQUIRED, CIRCLE_INVITE_REQUIRED)
        .replace("The shared pool is not available on this server yet.", "Circles are not available on this server yet.")
        .replace("The shared pool", "The circle")
        .replace("the shared pool", "the circle")
        .replace("The pool server", "verSHion's server")
        .replace("the pool server", "verSHion's server")
        .replace("reach the pool", "reach verSHion's server")
        .replace("The pool's", "verSHion's")
        .replace("the pool's", "verSHion's")
        .replace("The pool", "verSHion's server")
        .replace("the pool", "verSHion")
}

/// The public pool's "invite code needed" sentence (`pool::INVITE_REQUIRED_TEXT`, without its last sentence).
const POOL_INVITE_REQUIRED: &str = "The shared pool needs an invite code. Open the Account window, find \"Invite code\" and enter the code an admin gave you.";
/// (critic 78 I3) The same on a circle screen; (critic 80 polish) in words that fit the join page, the Circles list and
/// Create a circle alike: no "join", and no "the circle's code" where none was entered.
const CIRCLE_INVITE_REQUIRED: &str = "Your verSHion account needs an invite code from an administrator before it can use circles. The code it needs is not a circle's code. Open the Account window, find \"Invite code\" and enter it.";

/// (critic 72 I1) The merge review's words (`merge.rs`, shared with the public pool's updates): there "the pool" is
/// where the newer version came from, which for a circle bank is the circle and the colleagues in it, never
/// verSHion's server (that is [`circle_words`], for what the server did).
pub fn merge_words(note: &str) -> String {
    note.replace("The pool dropped this question", "The circle removed this question")
        // (round 19, critic 100 minor 12) a circle screen's verb is Get latest
        .replace("start the merge again", "press Get latest again")
        .replace("The pool's", "The circle's")
        .replace("the pool's", "the circle's")
        .replace("The pool", "The circle")
        .replace("the pool", "the circle")
}

/// (critic 78 I1) The merge review: a question new in the circle that the bank already has (the person's own, sent
/// without a readable answer). Linked, not added twice.
pub const SAME_ADDED: &str = "You and the circle added the same question.";

/// (critic 80 I1) The merge review: version `n` is the person's own send whose answer did not come back.
pub fn own_send_line(n: u64) -> String {
    format!("Version {n} is your own send from this computer: its answer did not come back. The questions it carried are matched to yours here, never added twice.")
}

/// (critic 85 m1) The merge review: the version a send with no readable answer may have made (`n`, right after the one it
/// was based on) can not be read now, while colleagues sent more since.
pub fn own_unreadable_line(n: u64) -> String {
    format!("Version {n} may be your earlier send from this computer, but it can't be read now, so what that send changed or added can't be matched to your questions here. A question it added may show below as new in the circle, changed since by someone else, or be gone from the circle's newest version. Your copy keeps every question it has, and they go with your next send.")
}

/// (critic 85 m1) A question that send changed, as the circle's newest version does not have it. (critic 88 I3) The row
/// offers the circle's version.
pub fn sent_change_unread(n: u64) -> String {
    format!("You sent a change to this question, maybe in version {n}, which can't be read now, so it is not known whether someone changed it since. Yours stays as it is unless you take the circle's version.")
}

/// (critic 85 m1) A question new in the circle while a question that send added is not matched: not added by default.
pub fn maybe_your_added(n: u64) -> String {
    format!("This may be a question your earlier send added (version {n} can't be read now), changed since by someone else. If it is, adding it puts it in your bank twice.")
}

/// (critic 80 I1) A question that send carried, changed since it went.
pub fn sent_then_changed(n: u64) -> String {
    format!("You sent this in version {n} and changed it since. Your change stays here and goes with your next send.")
}

/// (critic 86 I1) A merge review row that is a decision only because this computer has no record of the question's
/// difficulty in the earlier version (never "both changed"), and how many there are.
pub const UNRECORDED_LABEL: &str = "Difficulty not recorded";

/// (critic 88 I2) A copy that has no record of the difficulties in version `n`, the one it last matched (a base an earlier
/// build wrote, that version not readable): what else changed is counted; a change to a difficulty can't be. The strip and
/// the Send page say the same.
pub fn difficulty_caveat(n: u64) -> String {
    format!("This copy has no record of the difficulties in version {n}, so a change to a difficulty can't be counted here.")
}

/// (round 15, critics 91 I2 and I4) A copy an earlier build linked, while the circle's version it last matched is read: the
/// strip, and the circle page's bank row. Nothing is sent from it meanwhile.
pub const CHECKING: &str = "Checking the circle...";
/// (round 16, critic 94 m2 and m3) In the words of the strip's offline line, the same on the strip (with Try again) and the
/// circle page's bank row.
pub const CANT_CHECK: &str = "Couldn't check the circle just now, so nothing can be sent from this bank yet.";
/// (round 15, critic 91 I4) That version can't be read: sending waits for a read that works.
pub const SEND_WAITS: &str = "Sending is turned off until the circle can be checked.";
/// (round 17, critic 96 I1) The circle's difficulty was put into her bank after the circle's version was first read: the badge
/// says it until she dismisses it on the strip.
pub const NEWS_BADGE: &str = "difficulty updated";
/// The button that puts her difficulty back on one question.
pub const PUT_MINE_BACK: &str = "Put mine back";
/// (round 18, critic 98 minor 6) What Dismiss does, beside it.
pub const NEWS_DISMISS: &str = "Dismiss hides this list. Your bank stays as it is.";

/// (round 17, critic 96 I1) The strip's words over the list: that hers was replaced, why, and what she can do. (round 18, critic 98
/// minor 7) In words that fit a question the circle gives no difficulty, and "press Put mine back" only where a row offers it.
pub fn news_head(version: u64, n: usize, offered: bool, settled: bool) -> String {
    let qs = if n == 1 { "1 question".to_string() } else { format!("{n} questions") };
    // (round 19, critic 100 minor 5) once each is sent or changed since, it no longer reads as news
    if settled {
        return format!("Earlier, verSHion gave {qs} the circle's difficulty from its version {version}. Nothing here is waiting for you.");
    }
    let then = if offered { " To keep yours on a question, press Put mine back: it becomes a change you have not sent yet." } else { "" };
    format!("verSHion gave {qs} the circle's difficulty from its version {version}, in place of yours. An earlier version of the app had not kept the circle's difficulties in this copy.{then}")
}

/// "Easy", "Medium", "Hard", or "none".
pub fn difficulty_word(d: Option<crate::model::Difficulty>) -> &'static str {
    use crate::model::Difficulty;
    match d {
        Some(Difficulty::Low) => "Easy",
        Some(Difficulty::Medium) => "Medium",
        Some(Difficulty::High) => "Hard",
        None => "none",
    }
}

/// (round 18, critic 98 minors 6 and 7) A difficulty in the news list: "Easy", or "no difficulty" (never "none"), its words kept
/// together (a non-breaking space).
pub fn news_word(d: Option<crate::model::Difficulty>) -> &'static str {
    if d.is_none() { "no\u{a0}difficulty" } else { difficulty_word(d) }
}

/// One question of the list: "Question 2, "Simplify 3(x + 2)": Easy -> Hard" (`n`: its place in the bank, from 1; `None`:
/// no longer in the bank). (round 18, critic 98 I1) As it stands now: hers back (not sent yet, or sent), or changed since. "Easy
/// -> Hard" is never split over two lines.
pub fn news_row(n: Option<usize>, title: &str, before: Option<crate::model::Difficulty>, after: Option<crate::model::Difficulty>, state: super::circle_links::NewsState) -> String {
    use super::circle_links::NewsState;
    let which = match n {
        Some(n) if title.is_empty() => format!("Question {n}"),
        Some(n) => format!("Question {n}, \"{title}\""),
        None => "A question no longer in this bank".to_string(),
    };
    let moved = format!("{}\u{a0}->\u{a0}{}", news_word(before), news_word(after));
    match state {
        // (round 20, critic 100 minor 4) the circle's value too: what was put back, and what it replaced
        NewsState::Back => format!("{which}: yours ({}) is back, not sent yet; the circle's is {}", news_word(before), news_word(after)),
        NewsState::Sent => format!("{which}: yours ({}) was sent", news_word(before)),
        // (round 20, critic 100 minor 3) why no button, and what she can still do by hand
        NewsState::Changed(now) => format!("{which}: {moved}, changed since (now {}); set it by hand if you want yours", news_word(now)),
        NewsState::Kept => format!("{which}: {moved}. This copy has moved on, so it can't be put back; set it by hand if you want yours"),
        NewsState::Replaced | NewsState::Gone => format!("{which}: {moved}"),
    }
}

/// (round 20, critic 100 minor 4) "Put all of mine back" asks before it changes `n` questions at once.
pub fn put_all_ask(n: usize) -> String {
    let qs = if n == 1 { "question".to_string() } else { format!("{n} questions") };
    format!("Put yours back on {qs}? They become changes you have not sent yet.")
}

/// (round 17, critic 96 minor 4) What could not be saved, in plain words (never the system's own error text).
pub fn save_problem(raw: &str) -> &'static str {
    let r = raw.to_lowercase();
    if ["no space", "disk is full", "disk full", "os error 28", "os error 112", "quota"].iter().any(|k| r.contains(k)) {
        "the disk is full"
    } else if ["permission denied", "access is denied", "os error 13", "os error 5)", "read-only", "os error 30"].iter().any(|k| r.contains(k)) {
        "verSHion isn't allowed to write to its data folder"
    } else if ["locked", "another process", "os error 32", "os error 33"].iter().any(|k| r.contains(k)) {
        "another program is using the file"
    } else {
        "the file couldn't be written"
    }
}

/// (round 18, critic 98 minor 2) What she can do about what could not be saved. (round 19, critic 100 I2) One thing she can act on,
/// whatever the cause.
pub fn save_remedy(raw: &str) -> &'static str {
    let _ = raw;
    SAVE_ADVICE
}
/// (round 19, critic 100 I2) The advice for a file that can't be written.
pub const SAVE_ADVICE: &str = "Keep verSHion open and press Try again; if it keeps failing, free some disk space.";

/// (round 16, critic 93 I1; round 17) The safety copy of the banks could not be saved before the circle's difficulty went in.
pub fn safety_copy_not_saved(raw: &str) -> String {
    format!("verSHion couldn't save a safety copy of your banks first ({}), so the circle's difficulty isn't in this bank yet. {}", save_problem(raw), save_remedy(raw))
}
/// (round 18, critic 97 m6) The list of shared banks could not be saved before the circle's difficulty went in.
pub fn links_not_saved(raw: &str) -> String {
    format!("verSHion couldn't save the list of shared banks first ({}), so the circle's difficulty isn't in this bank yet. {}", save_problem(raw), save_remedy(raw))
}
/// The same for the banks file itself. (round 18, critic 98 minor 2) Then her own edits are not saved to disk either: said, with
/// what she can do.
pub fn banks_not_saved(raw: &str) -> String {
    format!("verSHion couldn't save your banks ({}), so the circle's difficulty isn't in this bank yet, and your own edits aren't saved either. {}", save_problem(raw), save_remedy(raw))
}
/// (round 18, critic 97 m3) "Put mine back" whose banks could not be saved: nothing changed, and the button stays. (round 19, critic
/// 100 I2) Said on the strip in the warning colour until a save works.
pub fn put_back_not_saved(raw: &str) -> String {
    format!("Your difficulty couldn't be put back, and your own edits aren't saved either: verSHion couldn't save your banks ({}). {}", save_problem(raw), save_remedy(raw))
}
/// (round 19, critic 100 I2) That line on the strip, with the tries that failed.
pub fn unsaved_line(line: &str, tries: u32) -> String {
    if tries <= 1 { line.to_string() } else { format!("{line} Tried {tries} times; still not saved.") }
}
/// The strip while that is so: why, that nothing changed, and (critic 96 minor 4) that Try again tried.
pub fn held_line(why: &str, tries: u32) -> String {
    // (round 20, critic 100 minor 12) "verSHion" once in the whole line, not four times: the reason already names it
    let again = if tries <= 1 { "It tries again every minute.".to_string() } else { format!("Tried {tries} times; it still couldn't be saved. It keeps trying every minute.") };
    format!("{why} Nothing in the bank changed; nothing can be sent from it until this is saved. {again}")
}
/// The badge while that is so.
pub const HELD_BADGE: &str = "couldn't save";
/// (round 17, critic 95 minor 1) Get latest waiting for it: the review is made once the bank has the circle's difficulty.
pub fn merge_waits_held(why: &str) -> String {
    // (round 19) this page has no Try again
    let why = why.replace(SAVE_ADVICE, "Keep verSHion open; if it keeps failing, free some disk space.");
    format!("{why} The review is made once it is saved; verSHion tries again every minute. You can wait here or press Cancel.")
}

/// (round 16, critic 94 m1; round 17, critic 96 minor 1) The version the copy last matched can't be read and nothing newer is
/// there: why sending is off, and what she can do.
pub fn not_kept_line(n: u64) -> String {
    cant_read_line(n, super::circle_edit::Unread::NotKept, false)
}
/// (round 18, critic 98 minor 3) The same by why it can't be read (`newest`: that version is the circle's newest, said once, never
/// beside "version N is the latest"). Held back for review: waiting is enough.
pub fn cant_read_line(n: u64, why: super::circle_edit::Unread, newest: bool) -> String {
    use super::circle_edit::Unread;
    let which = if newest { format!("Your copy last matched version {n}, the circle's newest version") } else { format!("Your copy last matched version {n}") };
    match why {
        // (round 19, critic 100 minor 1) never "the circle's newest version" beside "no longer keeps it"
        Unread::NotKept => format!("Your copy last matched version {n}, but the circle no longer keeps it, so verSHion can't tell which difficulties you changed, and sending is turned off."),
        Unread::Withheld => format!("{which}, which is held back while an administrator reviews it, so verSHion can't read its difficulties yet, and sending is turned off. Waiting is enough: once it is released, verSHion reads it and sending comes back on."),
        Unread::Other => format!("{which}, but verSHion couldn't read it, so it can't tell which difficulties you changed, and sending is turned off."),
    }
}
/// (round 19, critic 100 I1) Where Check now is the big button: what it does.
pub const CHECK_NOW_DOES: &str = "Check now asks the circle again.";
/// (round 19, critic 100 I1) While the check she asked for runs.
pub const CHECKING_NOW: &str = "Checking...";
/// (round 19, critic 100 I1) The check she asked for found nothing new (`ago`: "just now", "1 minute ago").
pub fn checked_same(ago: &str) -> String {
    format!("Checked {ago}: still the same.")
}
/// What she can do about it.
pub fn cant_read_ways(why: super::circle_edit::Unread) -> &'static str {
    if why == super::circle_edit::Unread::Withheld { WITHHELD_WAYS } else { NOT_KEPT_WAYS }
}
/// The circle page's row of such a copy.
pub const NOT_KEPT_ROW: &str = "The circle no longer keeps the version it last matched, so sending is turned off. Its page in My Banks says what you can do.";
/// (round 18, critic 98 minor 3) The same, by why.
pub fn cant_read_row(why: super::circle_edit::Unread) -> &'static str {
    match why {
        super::circle_edit::Unread::NotKept => NOT_KEPT_ROW,
        super::circle_edit::Unread::Withheld => "The version it last matched is held back while an administrator reviews it, so sending is turned off until then. Its page in My Banks says what you can do.",
        super::circle_edit::Unread::Other => "verSHion couldn't read the version it last matched, so sending is turned off. Its page in My Banks says what you can do.",
    }
}
pub const NOT_KEPT_WAYS: &str = "You can use your bank as it is: your next send then makes the circle's newest version match it, difficulties included. Or wait until someone in the circle sends a newer version, then get it.";
pub const WITHHELD_WAYS: &str = "Or you can use your bank as it is: your next send then makes the circle's newest version match it, difficulties included.";
/// (round 18, critic 98 minor 3; round 20, critic 100 minor 12) The strip's line where the circle's newest version is held back
/// for review and the copy's version can't be read: three sentences, never the five it took to say the same twice.
pub fn review_not_kept_line(name: &str, version: u64, why: super::circle_edit::Unread) -> String {
    use super::circle_edit::Unread;
    // the two facts in one sentence: the copy's own version can't be read, and the circle's newest version is under review
    let copy = match why {
        Unread::Withheld => format!("Your copy last matched version {version}, and the newest version is held back for review"),
        Unread::NotKept => format!("Your copy last matched version {version}, which the circle no longer keeps, and the newest version is held back for review"),
        Unread::Other => format!("Your copy last matched version {version}, which verSHion couldn't read, and the newest version is held back for review"),
    };
    format!("Shared with {name}. {copy}, so sending is off. Waiting is enough; Check now asks the circle again.")
}
/// (round 16, critic 93 minor 4; round 17, critic 96 minor 3) The daily limit on reading banks: the circle page's row. (round 18,
/// critic 98 minor 1) It is the account's (it counts every circle and the public pool), never the circle's.
pub const READ_LIMIT: &str = "Your daily limit for reading banks has been reached, so this bank can't be checked until it resets. Nothing can be sent from it until then.";
/// The same on the strip, with when it resets (as verSHion's server said).
pub fn read_limit(left_secs: Option<u64>) -> String {
    let when = api::wait_text(left_secs);
    format!("Your daily limit for reading banks has been reached. It resets {when}; verSHion checks this bank then. Until it does, nothing can be sent from this bank.")
}
/// The same on the Get latest page (no Try again: it would meet the same limit).
pub fn read_limit_page(retry_after: Option<u64>) -> String {
    format!("Your daily limit for reading banks has been reached. It resets {}; Get latest works again then. Nothing in your bank changed.", api::wait_text(retry_after))
}
/// (round 16, critic 93 I1) The read succeeded, but the circle's difficulty could not be put into the bank: the circle page's row.
pub const DIFFICULTY_HELD: &str = "The circle's difficulty couldn't be saved to this bank yet, so nothing can be sent from it.";

/// (round 17, critic 96 I2) "Use my bank as it is": the page's title, what it does, and the Send page after it.
pub const AS_IS_TITLE: &str = "Use my bank as it is";
pub const AS_IS_TEXT: &str = "Nothing in this bank changes now. Your next send makes the circle's newest version match this bank, difficulties included. Anything others sent that this bank doesn't have will no longer be in the newest version; it stays in the circle's history, where it can be restored.";
pub const AS_IS_DROPPED: &str = "That includes the difficulty colleagues gave questions in the circle that an earlier version of the app left out of your copy: your bank's difficulty replaces it.";
pub const AS_IS_DONE: &str = "Nothing in your bank changed.";
/// The line the bank's strip keeps after it.
pub const AS_IS_RESULT: &str = "Your bank is now used as it is: your next send makes the circle's newest version match it, difficulties included.";
/// (round 18, critic 98 minor 5) with what it overwrites, again
pub const AS_IS_SEND_LINE: &str = "You chose to use your bank as it is: this send makes the circle's newest version match it, difficulties included. Anything others sent that this bank doesn't have will no longer be in the newest version; it stays in the circle's history, where it can be restored.";
/// (round 16, critic 94 m2) Get latest waiting more than `BASE_WAIT` for a read that is only slow.
pub const STILL_CHECKING: &str = "Still checking the circle... You can wait here or press Cancel.";
/// (critic 91 minor) Get latest waiting for that read when it has taken a while.
/// (round 16, critic 94 m2, m7) Only for a read that failed before (one that is only slow says `STILL_CHECKING`); the page says
/// under it that nothing in the bank changes.
pub const CHECKING_LONG: &str = "Checking the circle... Couldn't check it just now; verSHion keeps trying. You can wait here or press Cancel.";

/// (critic 88 I2) A copy with no record at all of the version it last matched: the strip and the Send page say the same.
pub const NO_RECORD: &str = "Your copy has no record of what it looked like when it last matched the circle, so the changes can't be counted here.";

pub fn unrecorded_count(n: usize) -> String {
    if n == 1 { "1 difficulty not recorded".into() } else { format!("{n} difficulties not recorded") }
}

pub fn same_added_line(n: usize) -> String {
    if n == 1 {
        "You and the circle added the same question, so it is linked to the circle's, not added twice.".into()
    } else {
        format!("You and the circle added the same {n} questions, so they are linked to the circle's, not added twice.")
    }
}

pub const EMPTY_LIST_TITLE: &str = "You are not in any circle yet";
pub const EMPTY_LIST_HELP: &str = "A circle is a small private group of teachers who share banks with each other. Create one, or enter a code someone gave you.";
pub const EMPTY_BANKS: &str = "No banks in this circle yet.";
pub const EMPTY_BANKS_HELP_EDITOR: &str = "Share a bank to the circle from the Banks page (the bank's Share menu, then choose this circle), or with the button above.";
pub const EMPTY_BANKS_HELP_VIEWER: &str = "An owner or editor can add banks. You can look at and copy anything they add.";
pub const EMPTY_ACTIVITY: &str = "Nothing has happened in this circle yet.";
pub const NO_CODES: &str = "No codes yet. Make one to invite another teacher.";

pub const CODE_SHOWN_ONCE: &str = "Anyone with this code can join until it expires or is used up. It is shown only now: copy it before you close this.";
pub const CODE_PASTE_HELP: &str = "Paste the code, or the link or the whole message from an invitation email. Nothing is used up until you press Join.";
/// Bank settings (phase D1, critic 64 I8): the page shows no questions, so it does not say they are shown.
pub const SETTINGS_KEYS_NOTE: &str = "Everyone in this circle can see the answer keys: they come with every copy.";
pub const SETTINGS_KEYS_HIDDEN_NOTE: &str = "This bank hides its answer keys from viewers: editors and the owner see the full bank; viewers get it without the correct answers and explanations.";
pub const ANSWER_KEYS_NOTE: &str = "The questions are shown here without their answers. Everyone in this circle can see the answer keys: they come with every copy.";
pub const COPY_LINE: &str = "A copy is private to you and made on this computer. The circle's bank is not changed, and nothing you change in your copy goes back to it.";
pub const WORKING_COPY_LINE: &str = "A working copy stays linked to the circle's bank. It is private to you and on this computer, and the circle is changed only when you press Send. You can also get the newer versions others send, and merge them in when you choose.";
pub const HAS_WORKING_COPY: &str = "You already have a working copy of this bank (see Versions, below), so this makes a plain copy that is not linked to the circle.";
pub const WORKING_COPY_DONE: &str = "Your working copy is under Versions, below: send your changes from there, or get the circle's newer versions.";
pub const LEAVE_MEMBER: &str = "You will lose access to this circle's banks. You can come back with a new code.";
/// (critic 92 I2) What leaving does to the banks in My Banks, in My Banks words.
pub const LEAVE_MY_BANKS: &str = "Banks you added to My Banks stay there and keep working as your own, but they stop getting newer versions, and changes in them can no longer be sent to the circle.";

/// (critic 92 I2) A bank of this circle in My Banks with changes not sent yet (`None`: they can't be counted), on the leave panel.
/// (round 16, critic 93 minor 3) `waiting`: the copy waits for the circle to be checked, so Send is not offered: it is named
/// with that, never "Send it first" (with its count only where its strip says one).
pub fn leave_unsent(name: &str, n: Option<usize>, waiting: bool) -> String {
    // (round 17, critic 96 minors 1 and 2) never "stay until then": the bank's page says why and what she can do
    if waiting {
        return match n {
            Some(1) => format!("\"{name}\" has 1 unsent change that can't be sent yet. Its page in My Banks says why and what you can do."),
            Some(n) => format!("\"{name}\" has {n} unsent changes that can't be sent yet. Its page in My Banks says why and what you can do."),
            None => format!("\"{name}\" may have changes that are not sent, and they can't be sent yet. Its page in My Banks says why and what you can do."),
        };
    }
    match n {
        Some(1) => format!("\"{name}\" has 1 unsent change. Send it first if the circle should have it."),
        Some(n) => format!("\"{name}\" has {n} unsent changes. Send them first if the circle should have them."),
        None => format!("\"{name}\" may have changes that are not sent (they can't be counted here). Send them first if the circle should have them."),
    }
}
pub const LEAVE_OWNER: &str = "You own this circle. If you leave, it passes to the member who has been here longest (an editor first). If nobody else is in it, the circle closes.";
/// (round 16, critic 94 m6) In My Banks words, with what becomes of changes not sent (her own are named under it).
pub const CLOSE_CIRCLE: &str = "Everyone loses access in 14 days. Until then you can reopen it. Banks people added to My Banks stay there and keep working as their own, but changes not sent by then can no longer be sent to the circle.";
/// (round 18, critic 98 minor 9) The heading of that question in Bank settings.
pub const REMOVE_BANK_HEAD: &str = "Remove this bank from the circle?";
pub const REMOVE_BANK: &str = "The bank disappears for everyone in the circle. You can bring it back for 14 days; after that it is deleted. Anyone who added it to My Banks keeps it there as their own bank, but changes they have not sent can no longer be sent to the circle.";
pub const REMOVE_MEMBER: &str = "They lose access at once. Banks they added to My Banks stay there as their own, but changes they have not sent can no longer be sent to the circle. They can only come back with a code you make after this.";
pub const REPORT_THANKS: &str = "Thank you. Your report goes to the people who run verSHion. The author is not told who sent it.";
pub const TRANSFER_NOTE: &str = "This is an offer: they have 7 days to accept. Until they do you stay the owner. If they accept, they become the owner and you become an editor.";
pub const NOT_REACHED: &str = "Could not reach verSHion's server.";
pub const HIDDEN_BANK_TITLE: &str = "Hidden while a review is open";
pub const HIDDEN_BANK_LINE: &str = "Its title and author are not shown while it is reviewed, or while its author's account is.";
pub const PURGE_BANK: &str = "This deletes the bank for good now, instead of after its 14 days. It can not be brought back.";
pub const HELD_BANK: &str = "This bank is on hold while verSHion's reviewers look at it. Only you can see it; the other members cannot until it is cleared.";
// ---- phase C: invite by email ----------------------------------------------------------------------------------

pub const EMAIL_HELP: &str = "verSHion mails the teacher an invitation with a code that works once, for 7 days. The mail is a fixed text: it does not name you or the circle (the app shows both before they join).";
pub const EMAIL_NEEDS_ACCOUNT: &str = "To join, they need a verSHion account that an administrator has approved, with its invite code entered. Until then the circle's code in the mail can't be used.";
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
pub const HIDE_ON_ASK: &str = "Viewers will get every version of this bank without the correct answers, the explanations and the pictures, and with the choices shuffled. Editors and the owner keep seeing the full bank. If the newest version has pictures, you choose next which of them viewers may keep. verSHion first checks the viewers' copy; it may ask you to decide about letter labels (A, B, C ...) or choice orders.";
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
pub const PICTURES_SEND_INTRO: &str = "This bank hides its answer keys from viewers, so verSHion makes a viewers' copy of what you send for the circle. Choose for each picture whether viewers get it. Nothing was sent yet; your changes are still on this computer.";
pub const PICTURE_KEEP: &str = "Keep it for viewers";
pub const PICTURE_DROP: &str = "Drop it";
/// The confirmation's button when the picture step comes next (critic 62 minor: one "Hide the keys", on the step).
pub const HIDE_NEXT: &str = "Next";
/// While the keys are hidden: choosing again which pictures viewers get (critic 62 I2). The app does not know what
/// was chosen before, so every picture starts as dropped; each save is a change of the setting for the day's count.
pub const PICTURES_CHANGE: &str = "Change picture choices";
pub const PICTURES_SAVE: &str = "Save picture choices";
pub const PICTURES_CHANGE_INTRO: &str = "Choose again for each picture of the newest version whether viewers get it. Every picture starts as dropped here, whatever was chosen before. Nothing has changed yet. Saving new choices counts as a change of the answer-key setting, and there is a daily number of those.";
pub const PICTURES_CHANGE_DECIDE: &str = "Before the new picture choices can be saved, verSHion needs these decisions about the copy viewers will get. Nothing has changed yet.";

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

pub const HIDE_TRIES: &str = "Each press counts toward a daily number of tries, also when verSHion asks for decisions, so decide everything listed before you press again.";

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
            if d == 1 { "yesterday".into() } else if d < 60 { format!("{d} days ago") } else { date(ms) }
        }
    }
}

/// (round 17, critic 96 minor 8) A day as every circle screen says it: "Oct 8, 2025" (nothing for no time).
pub fn date(ms: i64) -> String {
    if ms <= 0 { String::new() } else { crate::bankui::fmt_date(ms.div_euclid(1000)) }
}

/// (round 16, critic 94 m9) When something was, as the bank's page says it ("2 days ago", or "Oct 8, 2025" after a month):
/// the page's `bankui::rel_time`, from `now` instead of the clock.
pub fn as_on_page(ms: i64, now: i64) -> String {
    let d = ((now - ms) / 1000).max(0);
    let plural = |n: i64, unit: &str| format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" });
    match d {
        0..=59 => "just now".into(),
        60..=3599 => plural(d / 60, "minute"),
        3600..=86_399 => plural(d / 3600, "hour"),
        _ if d < 30 * 86_400 => plural(d / 86_400, "day"),
        _ => crate::bankui::fmt_date(ms.div_euclid(1000)),
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
        out.push("The viewers' copy matched something verSHion checks for, so the newest version is held back and the bank is hidden from the circle while an administrator reviews it. The questions are safe.".into());
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

/// (round 18, critic 98 minor 10) The invite-by-email panel: the code's last day, as every circle screen says dates.
pub fn code_until(ms: i64) -> String {
    format!("The code in it works once, until {}.", date(ms))
}
