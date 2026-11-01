//! Wording for the pool browser, in plain English. Every server string passes through
//! [`api::plain`] (or [`publish_text::shown`]) before it is returned; nothing here is markup.

use super::api;
use super::pool::{ChainLink, LicenseCheck, LicenseStatus, ReportReason};
pub use super::publish_text::{kind_name, license_name, shown};

/// An author as shown: an erased account reads "deleted teacher" (the server says so itself; an
/// empty name is shown the same way).
pub fn author(name: &str) -> String {
    let p = api::plain(name);
    if p.is_empty() { "deleted teacher".to_string() } else { p }
}

/// "3 questions".
pub fn count(n: u32) -> String {
    if n == 1 { "1 question".into() } else { format!("{n} questions") }
}

/// "2026-10-26" from the server's millisecond time, or nothing for a missing one.
pub fn date(ms: i64) -> String {
    if ms <= 0 { String::new() } else { api::date_text(ms) }
}

/// The web address of a Creative Commons licence deed, for the bank's licence field. Empty for a
/// licence that has none.
pub fn license_url(wire: &str) -> &'static str {
    match wire {
        "CC-BY-4.0" => "https://creativecommons.org/licenses/by/4.0/",
        "CC-BY-SA-4.0" => "https://creativecommons.org/licenses/by-sa/4.0/",
        "CC-BY-NC-4.0" => "https://creativecommons.org/licenses/by-nc/4.0/",
        "CC-BY-NC-SA-4.0" => "https://creativecommons.org/licenses/by-nc-sa/4.0/",
        "CC0-1.0" => "https://creativecommons.org/publicdomain/zero/1.0/",
        _ => "",
    }
}

/// One line each, shown on a result card and at the top of the preview. `None` for a plain "ok".
pub fn check_marker(c: &LicenseCheck) -> Option<&'static str> {
    match c.status {
        LicenseStatus::Ok => None,
        LicenseStatus::UnverifiedEarlierMatch => Some("Another author published these questions earlier."),
        LicenseStatus::Mismatch => Some("The licence may not match the material this was built from."),
        LicenseStatus::Unverified => Some("The sources of these questions could not be fully checked."),
        LicenseStatus::ClaimedIndependent => Some("The author says they wrote these themselves; an earlier similar publication exists."),
        LicenseStatus::Other => Some("The licence check gave an answer this version of verSHion does not know."),
    }
}

/// The paragraph under "Licence" in the preview.
pub fn check_text(c: Option<&LicenseCheck>) -> String {
    let Some(c) = c else { return "The pool did not say whether the licence was checked.".into() };
    let base = match c.status {
        LicenseStatus::Ok => "The licence is consistent with the sources the pool has on record for these questions.",
        LicenseStatus::UnverifiedEarlierMatch => {
            "Another author published these questions earlier. The pool credits them below, and the licence could not be fully verified against theirs."
        }
        LicenseStatus::Mismatch => "The licence does not match the sources the pool has on record. Use these questions with care; you can report it.",
        LicenseStatus::Unverified => "The pool could not fully verify the licence against the sources of these questions.",
        LicenseStatus::ClaimedIndependent => {
            "The author says they wrote these questions independently of an earlier publication that looks similar. The earlier author is credited below."
        }
        LicenseStatus::Other => "This version of verSHion does not know what the pool's licence check means.",
    };
    match c.reason.as_deref().map(api::plain).filter(|r| !r.is_empty()) {
        Some(r) => format!("{base} ({r})"),
        None => base.to_string(),
    }
}

/// One credit in a question's chain, earliest first.
pub fn chain_line(l: &ChainLink) -> String {
    if l.is_placeholder() {
        let mut s = author(&l.author_name);
        if !l.collapsed_licenses.is_empty() {
            let names: Vec<String> = l.collapsed_licenses.iter().map(|x| license_name(x)).collect();
            s.push_str(&format!(" (licences: {})", names.join(", ")));
        }
        return s;
    }
    let who = author(&l.author_name);
    let lic = if l.license.is_empty() { String::new() } else { format!(", {}", license_name(&l.license)) };
    let ver = if l.version > 0 { format!(" (version {}{lic})", l.version) } else { String::new() };
    let changed = if l.modified { " - changed by the next author" } else { "" };
    if l.earlier_match {
        format!("Another author published these questions earlier: {who}{ver}{changed}")
    } else if l.possible_source {
        format!("Possible source, which the author says they did not use: {who}{ver}{changed}")
    } else {
        format!("Derived from the work of {who}{ver}{changed}")
    }
}

pub fn report_reason(r: ReportChoice) -> (&'static str, &'static str) {
    match r {
        ReportChoice::WrongAnswer => ("The answer is wrong", "A question marks the wrong answer as correct, or has none."),
        ReportChoice::StudentData => ("It contains student information", "Names, photos or other personal details of students. The item is held for review at once."),
        ReportChoice::Copyright => ("It is copyrighted material", "Taken from a textbook, a test bank or another source without permission."),
        ReportChoice::KeyLeak => ("The answer key is visible", "The item says its key is hidden, but the answers can be read in it."),
        ReportChoice::Inappropriate => ("It is inappropriate", "Content that does not belong in a classroom pool."),
        ReportChoice::Other => ("Something else", "Tell the reviewers what is wrong in the note."),
    }
}

/// What a person can report. `KeyLeak` is the Worker's own reason (`key_leak`) and is only offered
/// for an item whose key is hidden (the Worker refuses it for any other).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportChoice {
    WrongAnswer,
    StudentData,
    Copyright,
    KeyLeak,
    Inappropriate,
    Other,
}

impl ReportChoice {
    pub const ALL: [ReportChoice; 6] =
        [ReportChoice::WrongAnswer, ReportChoice::StudentData, ReportChoice::Copyright, ReportChoice::KeyLeak, ReportChoice::Inappropriate, ReportChoice::Other];

    pub fn reason(self) -> ReportReason {
        match self {
            ReportChoice::WrongAnswer => ReportReason::WrongAnswer,
            ReportChoice::StudentData => ReportReason::StudentData,
            ReportChoice::Copyright => ReportReason::Copyright,
            ReportChoice::Inappropriate => ReportReason::Inappropriate,
            ReportChoice::KeyLeak => ReportReason::KeyLeak,
            ReportChoice::Other => ReportReason::Other,
        }
    }
}

pub const THANKS: &str = "Thank you. A report is read by the people who run the pool. Nothing is shown to the author about who sent it.";
pub const REPORT_OWN: &str = "You can not report your own item.";
pub const SIGNED_OUT_LINE: &str = "Browsing the pool needs an account. Sign in, or create one, in the Account window. Everything else in verSHion keeps working without one.";
pub const LICENSE_FILTER_NOTE: &str = "The pool can search by words, subject, kind of question, tag, standard and licence. Grade is only matched among the results already on the page.";
pub const IMPORT_COPY_LINE: &str = "The import makes a private copy on this device. The answer key is included only if the author shared it.";
pub const UPDATE_SAFE_LINE: &str = "The new version is added as a new bank. Your current bank is not touched, so nothing you changed is lost; delete whichever you do not want.";
pub const KEYLESS_LINE: &str = "The author hid the answer key. The questions are imported without it and are marked so; add the correct answers before printing a key.";
