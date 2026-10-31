//! Password policy, Argon2id hashing and PHC encoding (port of auth/src/password.ts). The hash itself is
//! `share_core::argon2id`, the same function the Worker called as wasm.
use super::common_passwords::{COMMON_BASE_WORDS, COMMON_PASSWORDS};
use super::crypto::{b64, from_b64, random_bytes, timing_safe_equal};
use std::collections::HashSet;
use unicode_normalization::UnicodeNormalization;

pub const MAX_PASSWORD_CHARS: usize = 512;
const SALT_BYTES: usize = 16;
const TAG_BYTES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Argon2Params {
    /// Memory in KiB.
    pub m: u32,
    /// Iterations.
    pub t: u32,
    /// Parallelism.
    pub p: u32,
}

/// NIST SP 800-63B: normalise Unicode so the same password typed on two keyboards matches.
pub fn normalize_password(pw: &str) -> String {
    pw.nfkc().collect()
}

fn leet(c: char) -> char {
    match c {
        '0' => 'o',
        '1' => 'i',
        '3' => 'e',
        '4' => 'a',
        '5' => 's',
        '7' => 't',
        '@' => 'a',
        '$' => 's',
        c => c,
    }
}

/// Undo substitutions inside a word only (p@ssw0rd -> password); leading and trailing non-letters stay.
fn de_leet(s: &str) -> String {
    let rest = s.trim_start_matches(|c: char| !c.is_alphabetic());
    let lead = &s[..s.len() - rest.len()];
    let mid = rest.trim_end_matches(|c: char| !c.is_alphabetic());
    let tail = &rest[mid.len()..];
    // The Worker's regex has no dotAll flag: a line break inside the word leaves the string as it was.
    if mid.contains(['\n', '\r', '\u{2028}', '\u{2029}']) {
        return s.to_string();
    }
    format!("{lead}{}{tail}", mid.chars().map(leet).collect::<String>())
}

fn is_run_or_repeat(s: &str) -> bool {
    let u: Vec<i32> = s.encode_utf16().map(i32::from).collect();
    if u.len() < 3 {
        return false;
    }
    let step = u[1] - u[0];
    step.abs() <= 1 && u.windows(2).skip(1).all(|w| w[1] - w[0] == step)
}

/// Cheap, synchronous policy check (no I/O): length, the built-in common-password list, "common word plus
/// digits or symbols", runs like 1234567890, and the user's own email. A reason code, or None.
pub fn check_password_policy(password: &str, email: &str, min_length: usize, extra: &HashSet<String>) -> Option<&'static str> {
    let chars = password.chars().count();
    if chars < min_length {
        return Some("too_short");
    }
    if chars > MAX_PASSWORD_CHARS {
        return Some("too_long");
    }
    let lower = password.to_lowercase();
    if COMMON_PASSWORDS.binary_search(&lower.as_str()).is_ok() || extra.contains(&lower) {
        return Some("common");
    }
    let core = de_leet(&lower);
    let core = core.trim_matches(|c: char| !c.is_alphabetic());
    if COMMON_BASE_WORDS.binary_search(&core).is_ok() || extra.contains(core) {
        return Some("common");
    }
    if is_run_or_repeat(&lower) || lower.chars().collect::<HashSet<_>>().len() <= 2 {
        return Some("common");
    }
    let local = email.split('@').next().unwrap_or("");
    if lower == email || lower.contains(email) || (local.chars().count() >= 4 && lower.contains(local)) {
        return Some("contains_email");
    }
    None
}

/// The raw Argon2id tag. Parameters are bounded first because `share_core::argon2id` reports a bad one as a
/// wasm-only error type; a panic is caught and treated as a failure too.
pub fn argon2id(password: &[u8], salt: &[u8], p: Argon2Params) -> Result<Vec<u8>, ()> {
    if p.m < 8 || p.m > 4 * 1024 * 1024 || p.t < 1 || p.t > 100 || p.p < 1 || p.p > 64 || p.m < 8 * p.p || salt.len() < 8 {
        return Err(());
    }
    std::panic::catch_unwind(|| share_core::argon2id(password, salt, p.m, p.t, p.p)).map_err(|_| ())?.map_err(|_| ())
}

pub struct StoredHash {
    pub phc: String,
    pub params: Argon2Params,
}

pub fn hash_password(password: &str, params: Argon2Params) -> Result<StoredHash, ()> {
    let salt = random_bytes(SALT_BYTES);
    let tag = argon2id(password.as_bytes(), &salt, params)?;
    if tag.len() != TAG_BYTES {
        return Err(());
    }
    Ok(StoredHash { phc: format!("$argon2id$v=19$m={},t={},p={}${}${}", params.m, params.t, params.p, b64(&salt), b64(&tag)), params })
}

pub struct Parsed {
    pub params: Argon2Params,
    pub salt: Vec<u8>,
    pub tag: Vec<u8>,
}

fn digits(s: &str, max_len: usize) -> Option<u32> {
    (!s.is_empty() && s.len() <= max_len && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse().ok()).flatten()
}

fn b64_field(s: &str) -> Option<Vec<u8>> {
    (s.len() >= 16 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/')).then(|| from_b64(s)).flatten()
}

pub fn parse_phc(phc: &str) -> Option<Parsed> {
    let rest = phc.strip_prefix("$argon2id$v=19$")?;
    let mut parts = rest.split('$');
    let (pr, salt, tag) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() {
        return None;
    }
    let mut kv = pr.split(',');
    let m = digits(kv.next()?.strip_prefix("m=")?, 7)?;
    let t = digits(kv.next()?.strip_prefix("t=")?, 3)?;
    let p = digits(kv.next()?.strip_prefix("p=")?, 3)?;
    if kv.next().is_some() {
        return None;
    }
    Some(Parsed { params: Argon2Params { m, t, p }, salt: b64_field(salt)?, tag: b64_field(tag)? })
}

pub fn verify_password(phc: &str, password: &str) -> bool {
    let Some(parsed) = parse_phc(phc) else { return false };
    argon2id(password.as_bytes(), &parsed.salt, parsed.params).is_ok_and(|tag| timing_safe_equal(&tag, &parsed.tag))
}

/// Full-cost hash whose result is discarded, for an email that has no account. A cheaper stand-in would itself
/// reveal which emails exist.
pub fn dummy_verify(password: &str, params: Argon2Params) -> bool {
    if let Ok(tag) = argon2id(password.as_bytes(), &random_bytes(SALT_BYTES), params) {
        timing_safe_equal(&tag, &random_bytes(TAG_BYTES));
    }
    false
}

pub fn same_params(a: &Argon2Params, b: &Argon2Params) -> bool {
    a == b
}

pub fn params_json(p: &Argon2Params) -> String {
    format!(r#"{{"alg":"argon2id","v":19,"m":{},"t":{},"p":{}}}"#, p.m, p.t, p.p)
}

#[cfg(test)]
mod tests {
    use super::*;

    const E: &str = "jordan.smith@school.example.org";
    fn check(pw: &str) -> Option<&'static str> {
        check_password_policy(pw, E, 10, &HashSet::new())
    }
    const FAST: Argon2Params = Argon2Params { m: 19456, t: 2, p: 1 };

    #[test]
    fn the_word_lists_are_sorted_for_binary_search() {
        assert!(COMMON_PASSWORDS.windows(2).all(|w| w[0] < w[1]) && COMMON_BASE_WORDS.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn accepts_ordinary_passphrases() {
        for pw in ["violet-harbor-tractor", "Tr0ub4dor&3-horse", "correct horse battery", "日本語のパスワードです"] {
            assert_eq!(check(pw), None, "{pw}");
        }
    }

    #[test]
    fn counts_characters_not_utf16_units() {
        assert_eq!(check("😀😀😀😀😀😀😀😀😀"), Some("too_short"));
        assert_eq!(check("😀😀😀🙂🙂🙂😎😎😎🤖"), None);
    }

    #[test]
    fn rejects_common_passwords_padded_common_words_and_runs() {
        for pw in ["1234567890", "PASSWORD123", "Password!!!!", "welcome2026", "qwerty123456", "abcdefghij", "9876543210", "zzzzzzzzzzzz", "P@ssw0rd!!!", "Admin2026!!", "!!!password!!!"] {
            assert_eq!(check(pw), Some("common"), "{pw}");
        }
    }

    #[test]
    fn rejects_passwords_built_on_the_email() {
        assert_eq!(check("jordan.smith2026"), Some("contains_email"));
        assert_eq!(check(E), Some("contains_email"));
    }

    #[test]
    fn enforces_length_bounds_and_an_extra_word_list() {
        assert_eq!(check(&"x".repeat(9)), Some("too_short"));
        assert!(check(&"ab".repeat(257)).is_some());
        assert_eq!(check(&"qz7".repeat(171)), Some("too_long"));
        let extra: HashSet<String> = ["pleasantvalley".to_string()].into();
        assert_eq!(check_password_policy("pleasantvalley", E, 10, &extra), Some("common"));
    }

    #[test]
    fn normalisation_folds_compatibility_forms() {
        assert_eq!(normalize_password("ｐａｓｓ"), "pass");
        assert_eq!(normalize_password("e\u{0301}"), "\u{e9}");
    }

    #[test]
    fn phc_round_trips_and_verifies() {
        let stored = hash_password("some-long-password", FAST).unwrap();
        let (head, rest) = stored.phc.split_at("$argon2id$v=19$m=19456,t=2,p=1$".len());
        assert_eq!(head, "$argon2id$v=19$m=19456,t=2,p=1$");
        let (salt, tag) = rest.split_once('$').unwrap();
        assert_eq!((salt.len(), tag.len()), (22, 43));
        assert_eq!(parse_phc(&stored.phc).unwrap().params, FAST);
        assert!(verify_password(&stored.phc, "some-long-password"));
        assert!(!verify_password(&stored.phc, "some-long-passwore"));
        assert!(!verify_password("$argon2i$v=19$m=1,t=1,p=1$AAAA$AAAA", "x"));
        assert!(!verify_password("garbage", "x"));
        assert_ne!(stored.phc, hash_password("some-long-password", FAST).unwrap().phc, "a fresh salt each time");
    }

    #[test]
    fn a_hash_made_by_the_worker_still_verifies() {
        // argon2id("password", "somesalt-16bytes", m=19456,t=2,p=1) as the Worker's wasm computes it (RFC 9106 reference
        // implementation agrees): the stored hashes of existing accounts must keep working after the cutover.
        let salt = b"somesalt-16bytes";
        let tag = share_core::argon2id(b"password", salt, 19456, 2, 1).unwrap();
        let phc = format!("$argon2id$v=19$m=19456,t=2,p=1${}${}", b64(salt), b64(&tag));
        assert!(verify_password(&phc, "password"));
        assert!(!verify_password(&phc, "Password"));
    }

    #[test]
    fn malformed_or_hostile_encodings_are_refused_not_run() {
        let good = hash_password("pw-pw-pw-pw", Argon2Params { m: 8, t: 1, p: 1 }).unwrap().phc;
        assert!(parse_phc(&good).is_some());
        let (a, b) = good.split_at(good.rfind('$').unwrap());
        for bad in [
            format!("{a}$AAAA"),
            format!("{good}$extra"),
            good.replace("m=8", "m=99999999"),
            good.replace("m=8,t=1,p=1", "m=8,t=1"),
            good.replace("v=19", "v=16"),
            format!("{a}{}", b.replace('A', "!")),
        ] {
            assert!(parse_phc(&bad).is_none() || !verify_password(&bad, "pw-pw-pw-pw"), "{bad}");
        }
        // A tiny salt or absurd parameters never reach the hasher.
        assert!(argon2id(b"x", b"salt", FAST).is_err());
        assert!(argon2id(b"x", b"saltsaltsalt", Argon2Params { m: 1 << 30, t: 1, p: 1 }).is_err());
        assert!(argon2id(b"x", b"saltsaltsalt", Argon2Params { m: 8, t: 0, p: 1 }).is_err());
    }

    #[test]
    fn dummy_verify_does_the_work_and_always_says_no() {
        assert!(!dummy_verify("whatever-password", Argon2Params { m: 8, t: 1, p: 1 }));
        assert!(!dummy_verify("x", Argon2Params { m: 0, t: 0, p: 0 }), "bad parameters still just say no");
    }

    #[test]
    fn params_are_compared_and_serialised() {
        assert!(same_params(&FAST, &Argon2Params { m: 19456, t: 2, p: 1 }));
        assert!(!same_params(&FAST, &Argon2Params { t: 3, ..FAST }));
        assert_eq!(params_json(&FAST), r#"{"alg":"argon2id","v":19,"m":19456,"t":2,"p":1}"#);
    }
}
