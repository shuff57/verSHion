//! (critic 79 I3, critic 81 I1) Text compared as verSHion's server stores it. The server (share-core `text::clean`) keeps
//! every text of a question with its line breaks as LF, no byte-order mark, the characters of Word's Symbol font (pasted
//! as private-use U+F0xx) as the Unicode ones they stand for, in Unicode's composed form (NFC); a one-line text (a topic)
//! also with its runs of white space collapsed and trimmed, then as students are served it (`format::served_text`:
//! invisible hyphens and spaces removed, and the joiners a script or an emoji sequence does not need); and a question's
//! topics without the blank (nothing visible) and repeated ones.
//! This app keeps what was typed or pasted, which may be decomposed ("e" followed by a combining accent). Two texts are
//! the same when they are canonically equivalent: their full canonical decompositions (NFD, equal exactly when their NFC
//! is) are the same. `bank_links_fix10_tests` checks this against share-core itself, every character and many questions.
//!
//! No dependency: the tables are in `canon_tables`. (critic 81 minor 4: `icu_normalizer` is in the web build's tree only
//! as a dependency of a dependency; naming it would mean adding it to `Cargo.toml`.) The tables are Unicode 16, as the
//! server's are (`unicode-normalization` 0.1.25, `unicode-general-category` 1.1.0). (critic 81 minor 3) This app's
//! standard library is Unicode 17: a letter Unicode 17 added counts as visible here, where the server drops a topic of
//! nothing else. Accepted: such a topic is new text, so at worst it shows as a change.

use std::borrow::Cow;
use std::cmp::Ordering;

use super::canon_tables::{CCC, DECOMP, MARKS};

/// A multi-line text (a stem, a choice, an explanation) in the form it is compared in.
pub fn block(s: &str) -> Cow<'_, str> {
    form(s, '\n')
}

/// A one-line text (a topic) in the form it is compared in: also its white space collapsed and trimmed.
pub fn line(s: &str) -> String {
    form(s, ' ').split_whitespace().collect::<Vec<_>>().join(" ")
}

/// (critic 81 I1) A question's topics in the form they are compared in: each as the server serves it (see `topic`), then
/// without the ones with nothing visible and the repeated ones, in their order.
pub fn topics(list: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(list.len());
    for t in list {
        let t = topic(t);
        if t.chars().any(visible) && !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

/// One topic as the server serves it (share-core `format::served_text` after its Line cleaning, see `line`): the soft
/// hyphen, zero-width space, word joiner and combining grapheme joiner removed, and (critic 83 minor) a zero-width
/// joiner or non-joiner kept only where the server keeps it: between two letters of a script that needs one (a non-joiner
/// never after a letter that does not join on its left, like alef), or a joiner inside an emoji sequence. Judged round
/// after round as the server does, each round on the text left by the one before. A change of nothing but such a joiner
/// is then a change, and a topic is always the same as what the server makes of it.
fn topic(t: &str) -> String {
    let f = form(t, ' ');
    if f.is_ascii() {
        return f.split_whitespace().collect::<Vec<_>>().join(" ");
    }
    let mut v: Vec<char> = f.chars().collect();
    while let Some(next) = drop_invisible(&v) {
        v = next;
        // (removing a character can bring two runs of marks together: canonical order again, as the server's NFC does)
        reorder(&mut v);
    }
    v.into_iter().collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
}

fn joiner(c: char) -> bool {
    matches!(c, '\u{200C}' | '\u{200D}')
}

/// The invisible characters the server's served form always removes.
fn removed(c: char) -> bool {
    matches!(c, '\u{AD}' | '\u{200B}' | '\u{2060}' | '\u{34F}' | '\u{FEFF}')
}

/// One round of the server's removals (share-core `format::drop_invisible`) on decomposed text; `None` when nothing goes.
/// A joiner's neighbours are the nearest characters that are not joiners: before it, not one removed either; after it,
/// whatever comes (a removed one included, which the joiner then goes with).
fn drop_invisible(v: &[char]) -> Option<Vec<char>> {
    let mut next_of: Vec<Option<char>> = vec![None; v.len() + 1];
    for k in (0..v.len()).rev() {
        next_of[k] = if joiner(v[k]) { next_of[k + 1] } else { Some(v[k]) };
    }
    let mut prev: Option<usize> = None;
    let mut out = Vec::with_capacity(v.len());
    let mut changed = false;
    for (k, &c) in v.iter().enumerate() {
        if removed(c) {
            changed = true;
            continue;
        }
        if joiner(c) {
            let keep = match (prev, next_of[k + 1]) {
                (Some(p), Some(b)) => {
                    let a = composed_last(v, p);
                    (joining_script(a) && joining_script(b) && !(c == '\u{200C}' && right_joining_only(a))) || (c == '\u{200D}' && emoji_part(a) && emoji_part(b))
                }
                _ => false,
            };
            if !keep {
                changed = true;
                continue;
            }
        } else {
            prev = Some(k);
        }
        out.push(c);
    }
    changed.then_some(out)
}

/// The character before a joiner as the server judges it, in its composed text (NFC): the last character of `v[..=p]`
/// composed. Here the text is decomposed, so the letter `p` belongs to and its marks after it are composed as NFC does:
/// each mark in turn, into the letter (as composed so far) when a character is the two and no mark left between them has a
/// class as high; otherwise it stays. The last one that stays, or else the letter as composed. (A character Unicode keeps
/// from composing, like Devanagari qa, is taken as composed here: it is of the same script as its parts.)
fn composed_last(v: &[char], p: usize) -> char {
    let mut s = p;
    while s > 0 && class(v[s]) != 0 {
        s -= 1;
    }
    let start = v[s];
    // a letter alone. (Marks after a joiner or a removed character stay: no character is one of those with a mark. Marks
    // with no letter before them are not composed by NFC; the few characters made of marks only (Greek dialytika tonos,
    // three Tibetan vowels) are of the same script as their marks, so the joiner is judged the same either way.)
    if s == p {
        return v[p];
    }
    let mut parts: Vec<u32> = vec![start as u32];
    let mut stays: Vec<char> = Vec::new();
    for &m in &v[s + 1..=p] {
        let blocked = stays.last().is_some_and(|x| class(*x) >= class(m));
        parts.push(m as u32);
        if blocked || composite(&parts).is_none() {
            parts.pop();
            stays.push(m);
        }
    }
    stays.last().copied().or_else(|| composite(&parts)).unwrap_or(start)
}

/// The character whose full canonical decomposition is `parts` (a letter and one or more marks), if there is one.
fn composite(parts: &[u32]) -> Option<char> {
    DECOMP.iter().find(|(_, d)| *d == parts).and_then(|(c, _)| char::from_u32(*c))
}

/// The scripts that need a joiner between two of their letters (share-core `format::joining_script`).
fn joining_script(c: char) -> bool {
    matches!(c as u32, 0x0600..=0x08FF | 0x0900..=0x0DFF | 0x0F00..=0x0FFF | 0x1000..=0x109F | 0x1780..=0x18AF | 0xA8E0..=0xA8FF | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF | 0x10A00..=0x10A5F | 0x11000..=0x111FF)
}

/// Arabic-script letters that never join on their left (share-core `format::right_joining_only`).
fn right_joining_only(c: char) -> bool {
    matches!(c as u32, 0x0621..=0x0625 | 0x0627 | 0x0629 | 0x062F..=0x0632 | 0x0648 | 0x0671..=0x0673 | 0x0675..=0x0677 | 0x0688..=0x0699 | 0x06C0 | 0x06C3..=0x06CB | 0x06CD | 0x06CF | 0x06D2 | 0x06D3 | 0x06D5 | 0x06EE | 0x06EF | 0xFE80..=0xFE8E | 0xFEA9..=0xFEB0 | 0xFEED | 0xFEEE)
}

/// Emoji parts a joiner joins (share-core `format::emoji_part`).
fn emoji_part(c: char) -> bool {
    matches!(c as u32, 0x1F000..=0x1FAFF | 0x2600..=0x27BF | 0x2B00..=0x2BFF | 0xFE0F)
}

/// A character the server counts as visible in a topic it keeps (share-core `text::is_visible`, for the characters a
/// topic may have): not white space, a direction mark, a joiner or a combining mark. (Tag characters and fillers are
/// invisible too, but the server refuses a topic that has them, except a flag, which is visible.)
fn visible(c: char) -> bool {
    let cp = c as u32;
    !c.is_whitespace() && !matches!(cp, 0x200C..=0x200F) && MARKS.binary_search_by(|&(first, last)| in_range(first, last, cp)).is_err()
}

fn in_range(first: u32, last: u32, cp: u32) -> Ordering {
    if last < cp {
        Ordering::Less
    } else if first > cp {
        Ordering::Greater
    } else {
        Ordering::Equal
    }
}

/// Word's Symbol font, pasted as private-use U+F0xx: the characters teachers type with it and the Unicode ones the server
/// stores in their place (share-core `text::SYMBOL_FONT`, the Adobe Symbol encoding). The server refuses any other U+F0xx.
const SYMBOL_FONT: [(u8, char); 66] = [
    (0x22, '\u{2200}'), (0x24, '\u{2203}'), (0x27, '\u{220B}'), (0x40, '\u{2245}'), (0x5C, '\u{2234}'), (0x5E, '\u{22A5}'), (0xA2, '\u{2032}'),
    (0xA3, '\u{2264}'), (0xA5, '\u{221E}'), (0xAC, '\u{2190}'), (0xAD, '\u{2191}'), (0xAE, '\u{2192}'), (0xAF, '\u{2193}'), (0xB0, '\u{B0}'),
    (0xB1, '\u{B1}'), (0xB2, '\u{2033}'), (0xB3, '\u{2265}'), (0xB4, '\u{D7}'), (0xB5, '\u{221D}'), (0xB6, '\u{2202}'), (0xB7, '\u{2022}'),
    (0xB8, '\u{F7}'), (0xB9, '\u{2260}'), (0xBA, '\u{2261}'), (0xBB, '\u{2248}'), (0xC6, '\u{2205}'), (0xC7, '\u{2229}'), (0xC8, '\u{222A}'),
    (0xC9, '\u{2283}'), (0xCC, '\u{2282}'), (0xCE, '\u{2208}'), (0xCF, '\u{2209}'), (0xD0, '\u{2220}'), (0xD5, '\u{220F}'), (0xD6, '\u{221A}'),
    (0xD8, '\u{AC}'), (0xD9, '\u{2227}'), (0xDA, '\u{2228}'), (0xDB, '\u{21D4}'), (0xDE, '\u{21D2}'), (0xE5, '\u{2211}'), (0xF2, '\u{222B}'),
    (0x61, '\u{3B1}'), (0x62, '\u{3B2}'), (0x63, '\u{3C7}'), (0x64, '\u{3B4}'), (0x65, '\u{3B5}'), (0x66, '\u{3C6}'), (0x67, '\u{3B3}'),
    (0x68, '\u{3B7}'), (0x69, '\u{3B9}'), (0x6B, '\u{3BA}'), (0x6C, '\u{3BB}'), (0x6D, '\u{3BC}'), (0x6E, '\u{3BD}'), (0x70, '\u{3C0}'),
    (0x71, '\u{3B8}'), (0x72, '\u{3C1}'), (0x73, '\u{3C3}'), (0x74, '\u{3C4}'), (0x77, '\u{3C9}'), (0x78, '\u{3BE}'), (0x79, '\u{3C8}'),
    (0x7A, '\u{3B6}'), (0x44, '\u{394}'), (0x57, '\u{3A9}'),
];

fn symbol_font(c: char) -> Option<char> {
    let cp = c as u32;
    if !(0xF000..=0xF0FF).contains(&cp) {
        return None;
    }
    SYMBOL_FONT.iter().find(|(b, _)| u32::from(*b) == cp - 0xF000).map(|(_, m)| *m)
}

fn form(s: &str, nl: char) -> Cow<'_, str> {
    // (ASCII is its own decomposition: only its line breaks can change)
    if s.bytes().all(|b| b.is_ascii() && b != b'\r' && b != 0x0B) {
        return Cow::Borrowed(s);
    }
    let mut out: Vec<char> = Vec::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\r' => {
                if it.peek() == Some(&'\n') {
                    it.next();
                }
                out.push(nl);
            }
            '\u{B}' | '\u{2028}' | '\u{2029}' => out.push(nl),
            '\u{FEFF}' => {}
            c => decompose(symbol_font(c).unwrap_or(c), &mut out),
        }
    }
    reorder(&mut out);
    Cow::Owned(out.into_iter().collect())
}

// Hangul syllables decompose by rule (Unicode 3.12).
const S_BASE: u32 = 0xAC00;
const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11A7;
const T_COUNT: u32 = 28;
const N_COUNT: u32 = 21 * T_COUNT;
const S_COUNT: u32 = 19 * N_COUNT;

fn decompose(c: char, out: &mut Vec<char>) {
    let cp = c as u32;
    if (S_BASE..S_BASE + S_COUNT).contains(&cp) {
        let i = cp - S_BASE;
        let jamo = [Some(L_BASE + i / N_COUNT), Some(V_BASE + (i % N_COUNT) / T_COUNT), (i % T_COUNT != 0).then_some(T_BASE + i % T_COUNT)];
        out.extend(jamo.into_iter().flatten().filter_map(char::from_u32));
        return;
    }
    match DECOMP.binary_search_by_key(&cp, |e| e.0) {
        Ok(i) => out.extend(DECOMP[i].1.iter().filter_map(|x| char::from_u32(*x))),
        Err(_) => out.push(c),
    }
}

/// The canonical combining class of `c` (0: a starter).
fn class(c: char) -> u8 {
    let cp = c as u32;
    match CCC.binary_search_by(|&(first, last, _)| in_range(first, last, cp)) {
        Ok(i) => CCC[i].2,
        Err(_) => 0,
    }
}

/// Canonical ordering: each run of combining marks sorted by class, keeping the order of marks of the same class.
fn reorder(v: &mut [char]) {
    let mut i = 0;
    while i < v.len() {
        if class(v[i]) == 0 {
            i += 1;
            continue;
        }
        let start = i;
        while i < v.len() && class(v[i]) != 0 {
            i += 1;
        }
        v[start..i].sort_by_key(|c| class(*c));
    }
}
