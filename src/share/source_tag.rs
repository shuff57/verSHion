//! The ad / link source tag: a link to the web app may carry `?src=<tag>` (for example `?src=fb-math`), so the
//! admin's growth report can say which link brought a sign-up. The page reads it once at startup and keeps it in
//! memory only (no cookie, no localStorage, no sessionStorage); the sign-up request carries it once, and the server
//! keeps it on the new account. Nothing else about the visit is kept or sent. The desktop app never has one.
//!
//! [`normalise`] is the server's rule (`server/share-core/src/source_tag.rs`, which the app does not link); the
//! tests hold the two to the same answers.

use std::sync::OnceLock;

/// The longest tag kept.
pub const MAX_LEN: usize = 32;

/// The tag as the server stores it, or None when `raw` is not a valid one: trimmed, 1 to 32 ASCII letters, digits,
/// `_` or `-`, lowercased.
pub fn normalise(raw: &str) -> Option<String> {
    let t = raw.trim();
    let ok = (1..=MAX_LEN).contains(&t.len()) && t.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    ok.then(|| t.to_ascii_lowercase())
}

/// The `src` parameter of a URL query (`location.search`, with or without its `?`), normalised. The first `src`
/// wins; `+` is a space and `%XX` is decoded, as a browser encodes a query.
pub fn from_query(search: &str) -> Option<String> {
    let q = search.strip_prefix('?').unwrap_or(search);
    let raw = q.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        (decode(k)? == "src").then(|| decode(v))
    })??;
    normalise(&raw)
}

/// Percent-decoding of one query component. None when it does not decode to UTF-8.
fn decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok());
                match hex {
                    Some(x) => {
                        out.push(x);
                        i += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8(out).ok()
}

static REMEMBERED: OnceLock<Option<String>> = OnceLock::new();

/// Called once by the web shell at startup with `location.search`. Later calls change nothing.
pub fn remember_from_query(search: &str) {
    let _ = REMEMBERED.set(from_query(search));
}

/// The tag this page was opened with, if any.
pub fn remembered() -> Option<String> {
    REMEMBERED.get().cloned().flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cases the server's own test uses, and more: both rules must agree on every one.
    const CASES: &[&str] = &[
        "fb-math", "  FB_Math-2 \n", "Reddit", "a", "", "   ", "fb math", "fb.math", "fb/math", "<script>", "caf\u{e9}",
        "\u{212a}elvin", "a%20b", "a+b", "x\u{0}", "tab\tbed", "--", "__", "0123456789", "\u{ff21}bc",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    ];

    #[test]
    fn tags_follow_the_servers_rule() {
        assert_eq!(normalise("fb-math").as_deref(), Some("fb-math"));
        assert_eq!(normalise("  FB_Math-2 \n").as_deref(), Some("fb_math-2"));
        assert_eq!(normalise(&"a".repeat(32)).as_deref(), Some("a".repeat(32).as_str()));
        for bad in ["", "   ", &"a".repeat(33), "fb math", "fb.math", "<script>", "caf\u{e9}", "\u{212a}elvin", "a%20b"] {
            assert_eq!(normalise(bad), None, "{bad:?}");
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn the_app_and_the_server_normalise_every_case_alike() {
        for c in CASES {
            assert_eq!(normalise(c), share_core::source_tag::normalise(c), "{c:?}");
        }
        assert_eq!(MAX_LEN, share_core::source_tag::MAX_LEN);
    }

    #[test]
    fn the_tag_is_read_from_the_query_string() {
        assert_eq!(from_query("?src=fb-math").as_deref(), Some("fb-math"));
        assert_eq!(from_query("src=FB-Math").as_deref(), Some("fb-math"));
        assert_eq!(from_query("?a=1&src=Reddit&src=other").as_deref(), Some("reddit"), "the first one wins");
        assert_eq!(from_query("?src=%46B-math").as_deref(), Some("fb-math"), "percent-decoded");
        assert_eq!(from_query("?src=+fb+").as_deref(), Some("fb"), "plus is a space, then trimmed");
        assert_eq!(from_query("?%73rc=fb").as_deref(), Some("fb"), "the key is decoded too");
        for none in ["", "?", "?source=fb", "?src", "?src=", "?src=fb%20math", "?src=%E9", "?src=%zz", "?src=a%", "?src=<x>", "?xsrc=fb"] {
            assert_eq!(from_query(none), None, "{none:?}");
        }
        assert_eq!(from_query(&format!("?src={}", "a".repeat(33))), None);
    }
}
