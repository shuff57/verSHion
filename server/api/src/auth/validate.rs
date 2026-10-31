//! Text that a person typed and an admin will read (port of auth/src/validate.ts). Control and format characters
//! let an applicant fake labels (line breaks), hide or reorder text (bidi overrides and isolates) or smuggle
//! zero-width characters. They are refused, not stripped, so what is stored is exactly what was typed.
use serde_json::Value;
use unicode_general_category::{GeneralCategory as G, get_general_category};
use unicode_normalization::UnicodeNormalization;

/// Cc, Cf (incl. bidi controls and zero-width), private use, unassigned, line and paragraph separators.
/// (Lone surrogates cannot occur in a Rust string.)
pub fn is_forbidden(c: char) -> bool {
    matches!(get_general_category(c), G::Control | G::Format | G::PrivateUse | G::Unassigned | G::Surrogate | G::LineSeparator | G::ParagraphSeparator)
}

pub fn has_forbidden_chars(s: &str) -> bool {
    s.chars().any(is_forbidden)
}

/// A short line of display text: NFKC-normalised, runs of whitespace collapsed to one space, trimmed,
/// 1..max characters, free of control and format characters. None when invalid.
pub fn clean_text(v: &Value, max: usize) -> Option<String> {
    let s = v.as_str()?;
    if has_forbidden_chars(s) {
        return None;
    }
    let normal: String = s.nfkc().collect();
    let collapsed = normal.split(char::is_whitespace).filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" ");
    let n = collapsed.chars().count();
    if n == 0 || n > max || has_forbidden_chars(&collapsed) {
        return None;
    }
    Some(collapsed)
}

const MAX_PROFILE_JSON: usize = 4096;
const MAX_PROFILE_STRING: usize = 1000;

fn walk(v: &Value, depth: usize) -> bool {
    if depth > 4 {
        return false;
    }
    match v {
        Value::String(s) => !has_forbidden_chars(s) && s.chars().count() <= MAX_PROFILE_STRING,
        Value::Array(a) => a.iter().all(|x| walk(x, depth + 1)),
        Value::Object(o) => o.iter().all(|(k, x)| !has_forbidden_chars(k) && walk(x, depth + 1)),
        _ => true,
    }
}

/// Every string anywhere in an app-defined profile is free of control and format characters, and the whole is small.
pub fn profile_is_safe(profile: &Value) -> bool {
    profile.is_null() || (walk(profile, 0) && profile.to_string().encode_utf16().count() <= MAX_PROFILE_JSON)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn control_format_bidi_and_zero_width_characters_are_refused() {
        for bad in ["a\nb", "a\tb", "a\u{0}b", "a\u{200B}b", "a\u{202E}b", "a\u{2066}b", "a\u{200F}b", "a\u{2028}b", "a\u{2029}b", "a\u{E000}b", "a\u{FEFF}b", "a\u{0378}b"] {
            assert!(has_forbidden_chars(bad), "{bad:?}");
            assert_eq!(clean_text(&json!(bad), 50), None, "{bad:?}");
        }
        for ok in ["Jordan Smith", "日本語", "Zoë", "😀"] {
            assert!(!has_forbidden_chars(ok), "{ok:?}");
        }
    }

    #[test]
    fn clean_text_normalises_collapses_trims_and_counts_characters() {
        assert_eq!(clean_text(&json!("  Jordan   Smith  "), 50).as_deref(), Some("Jordan Smith"));
        assert_eq!(clean_text(&json!("ＡＢＣ"), 50).as_deref(), Some("ABC"), "NFKC folds full-width forms");
        assert_eq!(clean_text(&json!("a\u{00A0}b\u{3000}c"), 50).as_deref(), Some("a b c"), "unicode spaces collapse");
        assert_eq!(clean_text(&json!("😀😀😀"), 3).as_deref(), Some("😀😀😀"), "characters, not UTF-16 units");
        assert_eq!(clean_text(&json!("😀😀😀😀"), 3), None);
        assert_eq!(clean_text(&json!("   "), 50), None);
        assert_eq!(clean_text(&json!(""), 50), None);
        for not_text in [json!(null), json!(5), json!(["a"]), json!({})] {
            assert_eq!(clean_text(&not_text, 50), None);
        }
    }

    #[test]
    fn a_profile_is_small_shallow_and_free_of_hidden_characters() {
        assert!(profile_is_safe(&json!(null)));
        assert!(profile_is_safe(&json!({ "school": "Pleasant Valley", "grades": [9, 10], "a": { "b": { "c": { "d": "x" } } } })));
        assert!(!profile_is_safe(&json!({ "a": { "b": { "c": { "d": { "e": "x" } } } } })), "too deep");
        assert!(!profile_is_safe(&json!({ "name": "a\u{202E}b" })));
        assert!(!profile_is_safe(&json!({ "a\u{200B}": "x" })), "keys count too");
        assert!(!profile_is_safe(&json!({ "n": "x".repeat(1001) })));
        assert!(!profile_is_safe(&json!({ "a": "x".repeat(1000), "b": "x".repeat(1000), "c": "x".repeat(1000), "d": "x".repeat(1000), "e": "x".repeat(200) })), "whole is over 4096");
    }
}
