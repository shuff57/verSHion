//! Wording for "My shared items", in plain English. Every server string passes through
//! [`api::plain`] before it is returned; nothing here is markup and nothing is a link the server
//! supplied. The legal links are built here from the app's own configured server address.

use super::api;
pub use super::browse_text::{count, date};
use super::pool::{ItemState, MineItem};
pub use super::publish_text::{kind_name, license_name, shown};

/// One line saying where an item stands, for the list.
pub fn state_label(it: &MineItem) -> &'static str {
    match it.state {
        ItemState::Live if it.is_unlisted() => "Private (only you can see it)",
        ItemState::Live => "Live (listed in the pool)",
        ItemState::Deleted => "Deleted by you",
        ItemState::TakenDown => "Taken down after a copyright notice",
        ItemState::TakedownPending => "Copyright notice received",
        ItemState::RemovedPrivacy => "Removed: student information",
        ItemState::Removed => "Removed",
        ItemState::NeedsReupload => "A picture needs uploading again",
        ItemState::KeyLeakReview => "Held: answer key may be visible",
        ItemState::HeldForReview => "Held for review",
        ItemState::Other => "Status not known to this version of verSHion",
    }
}

/// What the author may be told about a hold: the reasons the Worker names, never who reported.
pub fn hold_reason(it: &MineItem) -> String {
    if let Some(n) = it.notice.as_deref().filter(|n| !n.trim().is_empty()) {
        return api::plain(n);
    }
    match it.hold_class.as_deref() {
        Some("student_data") => "An admin confirmed that a question this item shares with another item contains student data. Remove those questions and republish, or appeal if your copy is different.".into(),
        Some("copyright") => "A copyright takedown covers questions this item shares with the item taken down. Remove those questions and republish, or appeal if you have the right to share them.".into(),
        _ => "This item is held for review. It is hidden from other teachers until it has been looked at. The pool does not say why or who raised it; if you think it is a mistake you can appeal.".into(),
    }
}

/// A paragraph saying what the state means and what happens next.
pub fn explain(it: &MineItem, now_ms: i64) -> String {
    match it.state {
        ItemState::Live if it.is_unlisted() => "Only you can see this item. Other teachers can not find or copy it. You can make it public at any time.".into(),
        ItemState::Live => "Any approved teacher can find this item, read it and copy it under its licence.".into(),
        ItemState::Deleted => match it.undo_until {
            Some(u) if u > now_ms => format!("You deleted this item. It is hidden from everyone. You can undo the delete until {}; after that it is removed for good.", date(u)),
            _ => "You deleted this item and the time to undo that has passed. It is being removed for good.".into(),
        },
        ItemState::TakenDown => match it.counter_notice.as_deref() {
            Some("received") => "A copyright notice took this item down. You have sent a counter-notice; it was forwarded to the person who complained. If they do not file a court action, the item is restored after 10 business days.".into(),
            Some("available") => {
                let until = match it.counter_notice_by.filter(|b| *b > now_ms) {
                    Some(b) => format!(" You can do this until {}.", date(b)),
                    None => String::new(),
                };
                format!("A copyright notice took this item down. If you believe it was a mistake you can send a counter-notice (below). It is forwarded to the person who complained, with the details you give.{until}")
            }
            _ => "A copyright notice took this item down and the counter-notice window has closed. Nothing more can be done from here.".into(),
        },
        ItemState::TakedownPending => {
            let by = it.counter_notice_by.filter(|b| *b > 0);
            if it.counter_notice_sent {
                "A copyright notice arrived for an item you had already deleted. You have sent a counter-notice; it was forwarded to the person who complained.".into()
            } else if let Some(b) = by.filter(|b| *b > now_ms) {
                format!("A copyright notice arrived for an item you had already deleted. You can send a counter-notice until {}.", date(b))
            } else {
                "A copyright notice arrived for an item you had already deleted, and the 30-day counter-notice window has closed.".into()
            }
        }
        ItemState::RemovedPrivacy => {
            let base = it.notice.as_deref().map(api::plain).filter(|s| !s.is_empty()).unwrap_or_else(|| {
                "An admin confirmed this item contained student data and removed it. If that is wrong, appeal within 14 days; you can then publish it again.".into()
            });
            match it.appeal_until.filter(|u| *u > 0) {
                Some(u) if u > now_ms => format!("{base} You can appeal until {}.", date(u)),
                Some(_) => format!("{base} The 14 days to appeal have passed."),
                None => base,
            }
        }
        ItemState::Removed => {
            let why = match it.removed_reason.as_deref() {
                Some("admin") => "An admin removed this item.",
                Some("privacy") => "This item was removed because it contained student information.",
                Some("takedown") => "This item was removed after a copyright notice.",
                _ => "This item was removed.",
            };
            if it.appeal.as_deref() == Some("available") {
                format!("{why} If you think that was a mistake you can appeal once; an admin will look at it.")
            } else {
                format!("{why} It can not be appealed.")
            }
        }
        ItemState::NeedsReupload => "A picture used by this item was lost on the server. Open the bank, make sure the picture is there, and share it again.".into(),
        ItemState::KeyLeakReview => it
            .notice
            .as_deref()
            .map(api::plain)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Teachers reported that what students see gives the hidden answer key away. Fix the wording and share again; the hold ends with a changed version.".into()),
        ItemState::HeldForReview => hold_reason(it),
        ItemState::Other => "This version of verSHion does not know this status. Nothing can be done with it here.".into(),
    }
}

pub const NO_ITEMS: &str = "You have not shared anything yet. Choose Share to pool… to share a bank or an exam.";

pub const STRIKES_LINE: &str = "How strikes work: only a strike an admin has upheld counts. Three upheld strikes within twelve months are \
     flagged for an admin to review the account; nothing is suspended automatically. verSHion does not show a count, because the pool \
     does not report one.";

pub const DRAFT_LEGAL: &str = "These pages are drafts. They have not had a legal review yet and are not legal advice.";

pub const DELETION_LINES: [&str; 5] = [
    "Your account stops working at once. Your email, name, school and password are erased and every device is signed out.",
    "Your name on copies other teachers made reads \"deleted teacher\".",
    "If you chose to keep what others copied, those items stay, without your name; the rest are hidden at once and removed after 14 days.",
    "A short record of what happened (not your name) is kept for 3 years for the safety of the pool.",
    "Your work on this device is not touched.",
];

pub const EXPORT_BLURB: &str = "Download everything the pool holds about you: your profile, your shared items with their full text (answer keys \
     included) and your reports. It is saved to a file you choose, and nothing is kept by verSHion afterwards.";

pub const COUNTER_INTRO: &str = "A counter-notice says you believe the item was taken down by mistake. What you type here is sent to the pool and \
     forwarded to the person who complained, so they can see your name, address and phone number. Do not send anything you are not willing \
     to share with them. If they do not file a court action within 10 business days, the item can be restored.";

pub const COUNTER_DRAFT: &str = "DRAFT wording: the statements below have not been reviewed by a lawyer. The pool supplies no text of its own.";

pub const COUNTER_NOT_KEPT: &str = "verSHion does not keep what you type: it is sent once and cleared from this window.";

pub const STATEMENT_PERJURY: &str = "I swear, under penalty of perjury, that I have a good faith belief that this material was removed or disabled by mistake or misidentification.";
pub const STATEMENT_JURISDICTION: &str = "I consent to the jurisdiction of the federal court for the district where my address is (or, if it is outside the United States, \
     any district where the pool's operator can be found).";
pub const STATEMENT_SERVICE: &str = "I will accept service of process from the person who sent the notice, or their agent.";

/// A form field's name as the person reads it.
pub fn field_name(wire: &str) -> &'static str {
    match wire {
        "signature" => "signature",
        "material_desc" => "description of the material",
        "name" => "full name",
        "address" => "address",
        "phone" => "phone number",
        "perjury_stmt" => "the statement under penalty of perjury",
        "consent_jurisdiction" => "the consent to jurisdiction",
        "accept_service" => "the acceptance of service",
        _ => "a required field",
    }
}

/// The pages the draft legal set has, in the order they are listed.
pub const LEGAL_PAGES: [(&str, &str); 4] = [
    ("Terms of use", "terms.html"),
    ("Privacy notice", "privacy.html"),
    ("Copyright and takedowns", "copyright.html"),
    ("Acceptable use", "acceptable-use.html"),
];

/// A legal page's web address on the configured server (`base` is the validated base URL), or
/// `None` when no server is configured. Never invented: there is no default domain.
pub fn legal_url(base: &str, file: &str) -> Option<String> {
    let b = base.trim().trim_end_matches('/');
    if b.is_empty() || !(b.starts_with("https://") || b.starts_with("http://")) {
        return None;
    }
    Some(format!("{b}/legal/{file}"))
}

/// A short size for the export ("1.2 MB").
pub fn bytes_text(n: usize) -> String {
    if n < 1024 {
        format!("{n} bytes")
    } else if n < 1024 * 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{:.1} MB", n as f64 / 1_048_576.0)
    }
}
