//! The ad / link source tag (`?src=<tag>` on a link to the app), sent once with a sign-up and kept on the new account
//! for the admin's growth report. The app (`src/share/source_tag.rs`, which does not link this crate) has its own copy;
//! its tests hold the two to the same answers.
//!
//! The rule: trim, then 1 to 32 characters of ASCII letters, digits, `_` or `-`, lowercased. Anything else is no tag
//! (never an error: a bad tag must not block a sign-up). ASCII only on purpose: Unicode lowercasing maps some
//! non-ASCII letters (the Kelvin sign) onto ASCII ones, which would let look-alike tags through.

/// The longest tag kept.
pub const MAX_LEN: usize = 32;

/// The tag as stored, or None when `raw` is not a valid one.
pub fn normalise(raw: &str) -> Option<String> {
    let t = raw.trim();
    let ok = (1..=MAX_LEN).contains(&t.len()) && t.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    ok.then(|| t.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_are_trimmed_lowercased_and_limited_to_32_safe_ascii_characters() {
        assert_eq!(normalise("fb-math").as_deref(), Some("fb-math"));
        assert_eq!(normalise("  FB_Math-2 \n").as_deref(), Some("fb_math-2"));
        assert_eq!(normalise(&"a".repeat(32)).as_deref(), Some("a".repeat(32).as_str()));
        for bad in ["", "   ", &"a".repeat(33), "fb math", "fb.math", "fb/math", "<script>", "caf\u{e9}", "\u{212a}elvin", "a%20b", "a+b"] {
            assert_eq!(normalise(bad), None, "{bad:?}");
        }
    }
}
