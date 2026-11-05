//! The two text helpers the grade screen needs, kept here rather than taking a dependency on
//! viSHion's `core` crate (28k lines) for two functions.

/// Answer text in the form two answers are compared in: lower case, whitespace collapsed to one
/// space, and outer punctuation dropped. `"  Mercury. "` and `"mercury"` become the same string.
pub fn clean(s: &str) -> String {
    let lowered = s.to_lowercase();
    let mut out = String::with_capacity(lowered.len());
    let mut space = false;
    for c in lowered.chars() {
        if c.is_whitespace() {
            space = true;
            continue;
        }
        if space && !out.is_empty() {
            out.push(' ');
        }
        space = false;
        out.push(c);
    }
    out.trim_matches(|c: char| c.is_ascii_punctuation() && c != '-' && c != '\'').to_string()
}

/// Levenshtein distance over UTF-16 units, the measure viSHion's `judge::lev` uses (so a distance
/// scored there means the same thing here).
pub fn lev(a: &str, b: &str) -> usize {
    let a: Vec<u16> = a.encode_utf16().collect();
    let b: Vec<u16> = b.encode_utf16().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i];
        for j in 1..=b.len() {
            let sub = prev[j - 1] + usize::from(a[i - 1] != b[j - 1]);
            cur.push((prev[j] + 1).min(cur[j - 1] + 1).min(sub));
        }
        prev = cur;
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_folds_case_space_and_outer_punctuation() {
        assert_eq!(clean("  Mercury. "), "mercury");
        assert_eq!(clean("4.5  million\npeople"), "4.5 million people");
        assert_eq!(clean("don't"), "don't");
    }

    #[test]
    fn lev_counts_edits_the_way_a_teacher_does() {
        assert_eq!(lev("mercury", "mercury"), 0);
        assert_eq!(lev("mercury", "mercuri"), 1);
        assert_eq!(lev("venus", "venis"), 1);
        assert_eq!(lev("", "mars"), 4);
    }
}
