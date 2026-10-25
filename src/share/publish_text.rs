//! Plain-English wording for the publish flow: licences, where a finding is, what a finding
//! means, why an order stays fixed. Pure functions, no egui, so tests cover every sentence.
//!
//! Anything that came from the server is passed through [`api::plain`] before it is drawn
//! (never markup, never a link); the fixed sentences here are ours.

use serde_json::Value;

use super::api;
use super::pool::{Issue, License};

/// Name and one-line meaning of each licence, for the picker.
pub struct LicenseInfo {
    pub license: License,
    pub name: &'static str,
    pub line: &'static str,
}

pub const LICENSES: [LicenseInfo; 6] = [
    LicenseInfo {
        license: License::CcBy,
        name: "CC BY 4.0 (credit me)",
        line: "Anyone may copy and change these questions, even to sell, as long as they credit you.",
    },
    LicenseInfo {
        license: License::CcBySa,
        name: "CC BY-SA 4.0 (credit me, share alike)",
        line: "Anyone may copy and change them with credit, but their changed copies must carry this same licence.",
    },
    LicenseInfo {
        license: License::CcByNc,
        name: "CC BY-NC 4.0 (credit me, not for sale)",
        line: "Teachers may copy and change them with credit, but not sell them.",
    },
    LicenseInfo {
        license: License::CcByNcSa,
        name: "CC BY-NC-SA 4.0 (credit me, not for sale, share alike)",
        line: "Credit you, no selling, and changed copies must carry this same licence.",
    },
    LicenseInfo {
        license: License::Cc0,
        name: "CC0 (public domain)",
        line: "Anyone may do anything with them. No credit needed, and you give up your rights.",
    },
    LicenseInfo {
        license: License::CircleOnly,
        name: "All rights reserved",
        line: "Nobody else may copy it. Only for an unlisted item that only you can see.",
    },
];

pub fn license_info(l: License) -> &'static LicenseInfo {
    LICENSES.iter().find(|i| i.license == l).expect("every licence has a row")
}

/// A licence the server named, in words (unknown strings are shown as they are, as plain text).
pub fn license_name(wire: &str) -> String {
    match License::parse(wire) {
        Some(l) => license_info(l).name.to_string(),
        None => api::plain(wire),
    }
}

pub const PUBLIC_LINE: &str = "Public: listed in the pool. Any approved teacher can find it, read it and copy it under the licence.";
pub const UNLISTED_LINE: &str = "Unlisted: kept on the server for you. Nobody else can see it. You can make it public later.";
pub const HIDE_KEY_LINE: &str = "Hide the answer key (recommended). Other teachers get the questions without the correct answers, and \
     the order of choices is shuffled. The key stays with you.";
pub const SHOW_KEY_LINE: &str = "Everyone who can open this item will see the correct answers, and so can their students.";
pub const PERMANENT_LINE: &str = "A Creative Commons licence can not be taken back from teachers who already copied the questions. \
     Unlisting or deleting later stops new copies only.";
pub const PICTURE_UPLOAD_LINE: &str = "Pictures are uploaded privately so they can be checked. Nothing is shared until you press Publish.";
pub const SHUFFLE_LINE: &str = "The order of the choices is drawn again when you publish, so the order below is an example. What is \
     hidden, removed or kept is exactly what students will get.";

/// 1-based question number from a 0-based index, or "this question" for a single question.
pub fn question_label(index: usize, single: bool) -> String {
    if single { "this question".to_string() } else { format!("Question {}", index + 1) }
}

/// Where a finding is, from its path (`bank.questions[2].kind.Mc.answers[1].text`).
pub fn where_text(path: &str, single: bool) -> String {
    let num = |key: &str| -> Option<usize> {
        let rest = path.split(key).nth(1)?;
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        digits.parse().ok()
    };
    let q = if let Some(i) = num("questions[") {
        Some(question_label(i, single))
    } else if path.starts_with("question") {
        Some("this question".to_string())
    } else {
        None
    };
    let part = num("parts[").map(|p| format!("part {}", p + 1));
    let spot = if path.contains(".answers[") {
        num("answers[").map(|a| format!("answer choice {}", a + 1))
    } else if path.contains(".pairs[") {
        num("pairs[").map(|a| format!("matching row {}", a + 1))
    } else if path.contains(".distractors[") {
        num("distractors[").map(|a| format!("extra word {}", a + 1))
    } else if path.ends_with(".stem") || path.ends_with("stem") {
        Some("the question text".to_string())
    } else if path.contains("explanation") {
        Some("the explanation".to_string())
    } else if path.contains("details.description") {
        Some("the bank description".to_string())
    } else if path.ends_with("bank.name") || path.ends_with(".name") {
        Some("the bank name".to_string())
    } else if path.starts_with("meta") {
        Some(format!("the listing details ({})", api::plain(path.trim_start_matches("meta."))))
    } else {
        None
    };
    let mut bits: Vec<String> = Vec::new();
    if let Some(q) = q {
        bits.push(q);
    }
    if let Some(p) = part {
        bits.push(p);
    }
    if let Some(s) = spot {
        bits.push(s);
    }
    if bits.is_empty() { format!("at {}", api::plain(path)) } else { bits.join(", ") }
}

/// What it was that gave the key away (share-core's `reason`).
pub fn reason_text(reason: &str) -> String {
    match reason {
        "star" => "a star or mark next to it".into(),
        "key_word" => "a word like \"correct\" or \"answer\"".into(),
        "capitals" => "different capital letters".into(),
        "padding" | "spacing" => "different spacing".into(),
        "answer_line" => "an \"Answer:\" line".into(),
        "gift" => "answer syntax from another quiz format (GIFT)".into(),
        "cloze" => "answer syntax from another quiz format (Cloze)".into(),
        "tf_value" => "the True or False value".into(),
        "left_names_right" => "a left item that names its match".into(),
        "blank_hint" => "a hint about the blank".into(),
        "word_bank_first" => "the word bank listing the answer first".into(),
        "latex_key" => "the answer written as a formula".into(),
        "pairing_token" => "a label or symbol shared by both sides of a pair".into(),
        other => api::plain(&other.replace('_', " ")),
    }
}

/// What a finding means, for the person who has to fix or acknowledge it.
/// The same for a send to a circle, which has no "hide the key" choice.
pub fn issue_text_circle(i: &Issue, single: bool) -> String {
    if i.code == "keyless_needs_hide_key" {
        return format!("{}: this copy came from a version shared without its key; set the answers yourself", where_text(&i.path, single));
    }
    issue_text(i, single)
}

pub fn issue_text(i: &Issue, single: bool) -> String {
    let at = where_text(&i.path, single);
    let what: String = match i.code.as_str() {
        "key_in_formatting" => "the correct choice looks different from the others".into(),
        "key_in_stem" => "the question text gives the answer away".into(),
        "locked_key" => "only the correct choice is locked, which gives the answer away".into(),
        "placeholder_key" => "this copy came without its key; set the answer yourself".into(),
        "keyless_key_material" => "a question shared without its key still holds key material".into(),
        "keyless_needs_hide_key" => "this copy came from a version shared without its key; hide the key, or set the answers yourself".into(),
        "labels_keep_refused" => "keeping these letters would give the answer away; strip them".into(),
        c => match i.message.as_deref().filter(|m| !m.is_empty()) {
            Some(m) => api::plain(m),
            None => generic_code_text(c, i.limit),
        },
    };
    let mut s = format!("{at}: {what}");
    if let Some(r) = i.reason.as_deref().filter(|r| !r.is_empty())
        && matches!(i.code.as_str(), "key_in_formatting" | "key_in_stem")
    {
        s.push_str(&format!(" ({})", reason_text(r)));
    }
    s
}

/// Findings that are not about a key: size, characters, missing fields.
pub fn generic_code_text(code: &str, limit: Option<u64>) -> String {
    match code {
        "missing" => "something required is missing".into(),
        "blank_text" => "this is empty".into(),
        "too_long" => match limit {
            Some(n) => format!("this is too long (at most {n} characters)"),
            None => "this is too long".into(),
        },
        "too_many" => match limit {
            Some(n) => format!("there are too many of these (at most {n})"),
            None => "there are too many of these".into(),
        },
        "too_few" => "there are too few of these".into(),
        "forbidden_char" => "this has a character the pool does not take".into(),
        "mark_run" => "this has a long run of accent marks".into(),
        "duplicate_choice" => "two answer choices are the same".into(),
        "no_correct" => "no answer choice is marked correct".into(),
        "blank_count_mismatch" => "the number of blanks does not match the answers".into(),
        "bad_image_file" => "a picture has a name the pool can not take".into(),
        "expected_string" | "expected_array" | "expected_object" | "expected_bool" | "expected_number" | "expected_integer" => {
            "this has the wrong shape".into()
        }
        "schema_version_unsupported" => "this file is from a newer version of verSHion".into(),
        "scan_budget" => "this text is too long or too repetitive to check; split it".into(),
        "normalized_too_large" => "this text is too large once cleaned up".into(),
        "no_questions" => "there are no questions to share".into(),
        "bad_url" => "this web address is not valid (it must start with https://)".into(),
        "bad_license" => "this licence is not one the pool knows".into(),
        "bad_id" | "bad_figure_id" => "this has an identifier the pool can not take; remake the item".into(),
        "bad_hash" => "a picture or figure reference is damaged; remake it".into(),
        "bad_form" => "this question is in a form the pool does not take".into(),
        "ambiguous_kind" | "unknown_kind" => "this question is of a kind the pool does not take".into(),
        "labels_need_decision" => "choose whether to strip or keep the letter labels".into(),
        "labels_unused" | "labels_out_of_range" | "labels_duplicate" => "a decision about letter labels no longer fits the bank (it changed); decide again".into(),
        "labels_strip_refused" => "these letters can not be stripped; keep them".into(),
        "order_ack_required" => "confirm that this order of choices may be shown".into(),
        c => format!("the pool refused it ({})", api::plain(c)),
    }
}

/// Why a choice order stays as written (share-core's order reasons).
pub fn order_reason_text(reason: &str) -> String {
    match reason {
        "few_unlocked" => "too few choices are free to move (the others are locked)".into(),
        "labels_kept" => "you kept the letter labels, so the order must stay".into(),
        "sibling_refs" => "a choice refers to another choice (\"both of the above\"), so the order must stay".into(),
        "author_keep_order" => "you asked for this order to be kept".into(),
        other => api::plain(&other.replace('_', " ")),
    }
}

/// A note the server made about what it dropped or changed (a `message`, else the `reason`).
pub fn note_text(v: &Value) -> String {
    if let Some(m) = v.get("message").and_then(Value::as_str).filter(|m| !m.is_empty()) {
        return api::plain(m);
    }
    for k in ["reason", "code"] {
        if let Some(r) = v.get(k).and_then(Value::as_str) {
            return api::plain(&r.replace('_', " "));
        }
    }
    "A change was made to the shared copy.".into()
}

/// A question kind name from the payload's tag.
pub fn kind_name(tag: &str) -> &'static str {
    match tag {
        "Mc" => "Multiple choice",
        "Tf" => "True or false",
        "Matching" => "Matching",
        "Blank" => "Fill in the blank",
        "Short" => "Short answer",
        "Multipart" => "Several parts",
        _ => "Question",
    }
}

/// Text from the server for the screen: plain, and marked when it was cut.
pub fn shown(s: &str) -> String {
    let p = api::plain(s);
    if s.chars().filter(|c| !c.is_control()).count() > 500 { format!("{p}...") } else { p }
}
