//! Character rules and text normalisation.
//!
//! # Character rules
//!
//! Text is checked code point by code point **before** normalisation. A few code points are
//! *mapped* (CR, VT and the Unicode line/paragraph separators become line breaks; a BOM is
//! dropped); everything else that breaks a rule is **refused**, never silently cleaned, so what is
//! stored is what the author typed. Two classes:
//!
//! * **Block** text: stems, part stems, explanations, answer texts, match items, distractors,
//!   accepted blanks, samples, bank descriptions, text figure params. Multi-line.
//! * **Line** text: bank names, topics, authors, licence names and URLs, attribution names, and
//!   publish metadata (titles, tags, standards). One line.
//!
//! | Code points | Block | Line | Why |
//! |---|---|---|---|
//! | LF U+000A, TAB U+0009 | allowed | refused | stem markup is line based |
//! | CR U+000D (and CRLF), VT U+000B, LINE SEPARATOR U+2028, PARAGRAPH SEPARATOR U+2029 | mapped to LF | mapped to a space (then collapsed) | Windows, Word and PDF text |
//! | BOM / ZWNBSP U+FEFF | dropped | dropped | editors insert it |
//! | every other Cc (C0, DEL, C1 incl. NEL U+0085) | refused | refused | fake line breaks, terminal escapes |
//! | ZWNJ U+200C, ZWJ U+200D, LRM U+200E, RLM U+200F | allowed | allowed | Persian, Indic half-forms, emoji ZWJ sequences, mixed-direction text |
//! | ZWSP U+200B, ALM U+061C, soft hyphen U+00AD, U+2060-2064 (word joiner, invisible math operators) | allowed | refused | Thai/Khmer word breaks, Arabic, hyphenation, MathML |
//! | script format characters: Arabic number signs U+0600-0605, U+06DD, U+08E2; Syriac U+070F; Mongolian vowel separator U+180E; Kaithi U+110BD, U+110CD | allowed | refused | needed to write those scripts |
//! | tag characters U+E0020-E007F | allowed **only** as the flags of England, Scotland and Wales (U+1F3F4, the tags "gbeng" / "gbsct" / "gbwls", U+E007F: Unicode's recommended tag sequences) | same | any other run spells hidden ASCII |
//! | bidi embeddings and overrides U+202A-202E, isolates U+2066-2069 | refused | refused | reorder what a reader sees ("Trojan Source") |
//! | deprecated format controls U+206A-206F, interlinear annotations U+FFF9-FFFB, U+E0001, every other Cf | refused | refused | invisible or deprecated |
//! | surrogates (Cs), private use (Co), unassigned and noncharacters (Cn) | refused | refused | no meaning to share; Cn as of `UNICODE_VERSION` |
//! | Hangul fillers U+115F, U+1160, U+3164, U+FFA0; Braille blank U+2800 | refused | refused | render as nothing; used to fake blank text |
//! | more than [`MAX_MARK_RUN`] consecutive combining marks (Mn, Mc, Me), counted in canonical decomposition, with ZWJ/ZWNJ not ending a run | refused | refused | "Zalgo" stacking; also bounds NFC work |
//! | variation selectors U+FE00-FE0F, U+E0100-E01EF | allowed (they are Mn) | allowed | emoji presentation (U+FE0F), CJK variants |
//! | everything else assigned (letters, marks, numbers, punctuation, symbols, spaces) | allowed | allowed | |
//!
//! Then text is NFC-normalised (Line text also has whitespace runs collapsed and is trimmed), and
//! measured after normalisation; normalisation stops as soon as the limit is passed.
//!
//! Stems and choices must also be **visible**: contain at least one letter, number, punctuation
//! or symbol (L, N, P, S), not only spaces, marks and invisible characters ([`has_visible`]).
//!
//! The categories are those of `unicode-general-category` (Unicode [`UNICODE_VERSION`]); NFC is
//! `unicode-normalization`. Both are pinned exactly in `Cargo.toml`; the app links this crate and
//! the Worker runs it as wasm, so both sides always use the same tables.
#[allow(unused_imports)]
use crate::format::FastClass;
use unicode_general_category::{GeneralCategory, get_general_category};
use unicode_normalization::UnicodeNormalization;

pub const UNICODE_VERSION: (u64, u64, u64) = unicode_general_category::UNICODE_VERSION;

/// The longest run of consecutive combining marks accepted. Tibetan stacks and Vedic accents
/// reach about ten; "Zalgo" text uses dozens.
pub const MAX_MARK_RUN: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Line,
    Block,
}

/// A length limit, in UTF-8 bytes or in characters, applied after normalisation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Max {
    Bytes(usize),
    Chars(usize),
}

impl Max {
    pub fn limit(self) -> u64 {
        match self {
            Max::Bytes(n) | Max::Chars(n) => n as u64,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextError {
    /// A refused code point at character index `offset` (and UTF-16 index `offset16`) of the
    /// input as sent.
    Forbidden { offset: usize, offset16: usize, cp: u32 },
    /// A run of more than `MAX_MARK_RUN` combining marks ending at `offset` / `offset16`.
    MarkRun { offset: usize, offset16: usize },
    TooLong,
}

fn is_mark(c: char) -> bool {
    matches!(get_general_category(c), GeneralCategory::NonspacingMark | GeneralCategory::SpacingMark | GeneralCategory::EnclosingMark)
}

fn is_tag(u: u32) -> bool {
    (0xE0020..=0xE007F).contains(&u)
}

fn is_vs(u: u32) -> bool {
    matches!(u, 0xFE00..=0xFE0F | 0xE0100..=0xE01EF)
}

fn is_cjk(u: u32) -> bool {
    matches!(u, 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x323AF)
}

/// True when `c` may never appear in text of class `class` on its own (the table above; tag
/// characters and variation selectors, which depend on their neighbours, are judged by [`check`]).
pub fn forbidden(c: char, class: Class) -> bool {
    let u = c as u32;
    match class {
        Class::Block if c == '\n' || c == '\t' => return false,
        Class::Block if matches!(u, 0xAD | 0x61C | 0x200B..=0x200F | 0x2060..=0x2064) => return false,
        Class::Block if matches!(u, 0x600..=0x605 | 0x6DD | 0x70F | 0x8E2 | 0x180E | 0x110BD | 0x110CD) => return false,
        Class::Line if matches!(u, 0x200C..=0x200F) => return false,
        _ => {}
    }
    // Fillers that render as nothing, and U+FFFC (what is left of an equation or object Word could
    // not export): not content.
    if matches!(u, 0x115F | 0x1160 | 0x3164 | 0xFFA0 | 0x2800 | 0xFFFC) {
        return true;
    }
    match get_general_category(c) {
        // The general-category tables (Unicode 16.0) lag the compiler's (17.0, `char::UNICODE_VERSION`
        // with the pinned toolchain): a letter or digit std knows is assigned, not refused. Any
        // other character our tables do not know is still refused. `tests/unicode_canary.rs`.
        GeneralCategory::Unassigned => !(c.fast_alpha() || c.is_numeric()),
        GeneralCategory::Control
        | GeneralCategory::Format
        | GeneralCategory::Surrogate
        | GeneralCategory::PrivateUse
        | GeneralCategory::LineSeparator
        | GeneralCategory::ParagraphSeparator => true,
        _ => false,
    }
}

/// The emoji tag sequences Unicode recommends (`RGI_Emoji_Tag_Sequence`): the flags of England,
/// Scotland and Wales. Any other run of tag characters could spell hidden ASCII.
const FLAG_TAGS: [&str; 3] = ["gbeng", "gbsct", "gbwls"];

/// Word's Symbol font, pasted as private-use U+F0xx: the characters teachers type with it,
/// mapped to their Unicode equivalents (Adobe Symbol encoding). Any other U+F0xx is refused with
/// `symbol_font_char` (the author re-types it).
const SYMBOL_FONT: [(u8, char); 66] = [
    (0x22, '∀'), (0x24, '∃'), (0x27, '∋'), (0x40, '≅'), (0x5C, '∴'), (0x5E, '⊥'), (0xA2, '′'), (0xA3, '≤'), (0xA5, '∞'), (0xAC, '←'),
    (0xAD, '↑'), (0xAE, '→'), (0xAF, '↓'), (0xB0, '°'), (0xB1, '±'), (0xB2, '″'), (0xB3, '≥'), (0xB4, '×'), (0xB5, '∝'), (0xB6, '∂'),
    (0xB7, '•'), (0xB8, '÷'), (0xB9, '≠'), (0xBA, '≡'), (0xBB, '≈'), (0xC6, '∅'), (0xC7, '∩'), (0xC8, '∪'), (0xC9, '⊃'), (0xCC, '⊂'),
    (0xCE, '∈'), (0xCF, '∉'), (0xD0, '∠'), (0xD5, '∏'), (0xD6, '√'), (0xD8, '¬'), (0xD9, '∧'), (0xDA, '∨'), (0xDB, '⇔'), (0xDE, '⇒'),
    (0xE5, '∑'), (0xF2, '∫'), (0x61, 'α'), (0x62, 'β'), (0x63, 'χ'), (0x64, 'δ'), (0x65, 'ε'), (0x66, 'φ'), (0x67, 'γ'), (0x68, 'η'),
    (0x69, 'ι'), (0x6B, 'κ'), (0x6C, 'λ'), (0x6D, 'μ'), (0x6E, 'ν'), (0x70, 'π'), (0x71, 'θ'), (0x72, 'ρ'), (0x73, 'σ'), (0x74, 'τ'),
    (0x77, 'ω'), (0x78, 'ξ'), (0x79, 'ψ'), (0x7A, 'ζ'), (0x44, 'Δ'), (0x57, 'Ω'),
];

/// The Unicode character Word's Symbol font meant by U+F0xx, if it is one of [`SYMBOL_FONT`].
pub fn symbol_font(c: char) -> Option<char> {
    let u = c as u32;
    if !(0xF000..=0xF0FF).contains(&u) {
        return None;
    }
    SYMBOL_FONT.iter().find(|(b, _)| *b as u32 == u - 0xF000).map(|(_, m)| *m)
}

/// The input as `clean` reads it, one character at a time, with each character's position in
/// the input (char index, UTF-16 index): CR, CRLF, VT and U+2028/2029 become a line break (Block)
/// or a space (Line), a BOM is dropped, and Word Symbol-font characters become their Unicode
/// equivalents. Nothing is copied: offsets in errors always refer to the input as sent.
#[derive(Clone)]
struct Mapped<'a> {
    it: std::iter::Peekable<std::str::Chars<'a>>,
    nl: char,
    idx: usize,
    off16: usize,
}

impl<'a> Mapped<'a> {
    fn new(s: &'a str, class: Class) -> Self {
        Mapped { it: s.chars().peekable(), nl: if class == Class::Block { '\n' } else { ' ' }, idx: 0, off16: 0 }
    }
}

impl Iterator for Mapped<'_> {
    type Item = (char, usize, usize);
    fn next(&mut self) -> Option<(char, usize, usize)> {
        loop {
            let c = self.it.next()?;
            let at = (self.idx, self.off16);
            self.idx += 1;
            self.off16 += c.len_utf16();
            let m = match c {
                '\r' => {
                    if self.it.peek() == Some(&'\n') {
                        self.it.next();
                        self.idx += 1;
                        self.off16 += 1;
                    }
                    self.nl
                }
                '\u{B}' | '\u{2028}' | '\u{2029}' => self.nl,
                '\u{FEFF}' => continue,
                c => symbol_font(c).unwrap_or(c),
            };
            return Some((m, at.0, at.1));
        }
    }
}


/// What `check_stream` needs of one character (not a selector or tag): whether each class
/// forbids it, whether it is a format character, its marks in canonical decomposition (and
/// whether a non-mark there ends a run first), and whether a variation selector may follow it.
#[derive(Clone, Copy)]
struct CharInfo {
    key: u32,
    forbidden: [bool; 2],
    format: bool,
    resets: bool,
    marks: u8,
    base: bool,
}

fn char_info_uncached(c: char) -> CharInfo {
    let u = c as u32;
    let forbidden = [forbidden(c, Class::Line), forbidden(c, Class::Block)];
    let format = get_general_category(c) == GeneralCategory::Format;
    let (mut resets, mut marks) = (false, 0usize);
    unicode_normalization::char::decompose_canonical(c, |d| {
        if is_mark(d) {
            marks += 1;
        } else {
            resets = true;
            marks = 0;
        }
    });
    // The base a selector would follow after NFC: none if `c` is (or decomposes to end with)
    // a mark (U+1D160 is a note plus marks), else `c` and its NFC form must both qualify.
    let mut last = c;
    unicode_normalization::char::decompose_canonical(c, |d| last = d);
    let mut base = !is_mark(c) && !is_mark(last) && !c.is_whitespace();
    if base {
        let n: Option<char> = {
            let mut it = std::iter::once(c).nfc();
            match (it.next(), it.next()) {
                (Some(x), None) => Some(x),
                _ => None,
            }
        };
        // A character NFC changes into another category (or into several) has no stable base.
        base = n.is_some_and(|n| n as u32 == u || get_general_category(n) == get_general_category(c) && is_cjk(n as u32) == is_cjk(u));
    }
    CharInfo { key: u, forbidden, format, resets, marks: marks.min(255) as u8, base }
}

fn char_info(c: char) -> CharInfo {
    use std::cell::RefCell;
    const EMPTY: CharInfo = CharInfo { key: u32::MAX, forbidden: [false; 2], format: false, resets: false, marks: 0, base: false };
    thread_local! {
        static CACHE: RefCell<[CharInfo; 1024]> = const { RefCell::new([EMPTY; 1024]) };
    }
    let u = c as u32;
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let slot = &mut cache[(u as usize) & 1023];
        if slot.key != u {
            *slot = char_info_uncached(c);
        }
        *slot
    })
}

/// The checks over a mapped stream, in one pass and without allocating: the table, the
/// tag-sequence rule (a 7-character lookahead), the variation-selector rule, and the mark-run cap.
fn check_stream(mut it: Mapped<'_>, class: Class) -> Result<(), TextError> {
    let mut run = 0usize;
    // The previous character that could carry a variation selector (None after a selector,
    // a mark, a format character or whitespace).
    let mut vs_base: Option<u32> = None;
    while let Some((c, idx, off16)) = it.next() {
        let u = c as u32;
        // Printable ASCII: allowed in every class, no mark, its own decomposition and NFC form: the
        // run ends and the character is a selector's base unless it is a space (the general path
        // gives exactly this, at one table lookup and an NFC iterator per character).
        if (0x20..0x7F).contains(&u) {
            run = 0;
            vs_base = (u != 0x20).then_some(u);
            continue;
        }
        if u == 0x1F3F4 {
            let mut ahead = it.clone();
            let flag = FLAG_TAGS.iter().find(|t| {
                let mut a = ahead.clone();
                t.chars().all(|x| a.next().map(|n| n.0 as u32) == Some(0xE0000 + x as u32)) && a.next().map(|n| n.0 as u32) == Some(0xE007F)
            });
            if let Some(t) = flag {
                for _ in 0..=t.len() {
                    ahead.next();
                }
                it = ahead;
                run = 0;
                vs_base = None;
                continue;
            }
        }
        if is_vs(u) {
            // A variation selector belongs right after a base it can vary, one per base:
            // * U+FE0E/FE0F (text/emoji presentation) after a symbol, digit or punctuation
            //   (U+2714 U+FE0F, the keycap 1 U+FE0F U+20E3), never after a letter;
            // * U+FE00-FE0D (standardised variants) after a math or other symbol, a non-Latin
            //   letter or a CJK ideograph;
            // * U+E0100-E01EF (ideographic variants) after a CJK ideograph.
            // Anything else (a run of them, one after a mark or a Latin letter) can hide bytes.
            let ok = match vs_base.and_then(char::from_u32) {
                Some(b) if u >= 0xE0100 => is_cjk(b as u32),
                // Characters with an emoji presentation: digits, # and * (keycaps), (c) and (r), and
                // symbols from U+2000 up; not letters, not Latin-1 signs like U+00B1.
                Some(b) if u >= 0xFE0E => {
                    !b.fast_alpha() && (b.is_ascii_digit() || matches!(b, '#' | '*' | '\u{A9}' | '\u{AE}') || b as u32 >= 0x2000)
                }
                Some(b) => {
                    is_cjk(b as u32)
                        || !b.is_ascii()
                            && matches!(get_general_category(b), GeneralCategory::MathSymbol | GeneralCategory::OtherSymbol | GeneralCategory::OtherLetter)
                }
                None => false,
            };
            if !ok {
                return Err(TextError::Forbidden { offset: idx, offset16: off16, cp: u });
            }
            vs_base = None;
            continue;
        }
        if is_tag(u) {
            return Err(TextError::Forbidden { offset: idx, offset16: off16, cp: u });
        }
        // (What follows depends on the character alone: looked up in a small cache, so a text of
        // one script costs a table read per character, not a decomposition and an NFC pass.)
        let info = char_info(c);
        if info.forbidden[(class == Class::Block) as usize] {
            return Err(TextError::Forbidden { offset: idx, offset16: off16, cp: u });
        }
        // Marks are counted in their canonical decomposition (U+0F73 is two marks), so the cap
        // holds after NFC and the offset is the input's; format characters (ZWJ, ZWNJ, LRM, ZWSP,
        // the soft hyphen, word joiner, invisible operators) neither count nor end a run, so they
        // cannot be used to split one.
        if info.format {
            vs_base = None;
            continue;
        }
        run = if info.resets { info.marks as usize } else { run + info.marks as usize };
        if run > MAX_MARK_RUN {
            return Err(TextError::MarkRun { offset: idx, offset16: off16 });
        }
        vs_base = info.base.then_some(u);
    }
    Ok(())
}

/// Checks every code point of `s` (see [`check_stream`]).
pub fn check(s: &str, class: Class) -> Result<(), TextError> {
    check_stream(Mapped::new(s, class), class)
}

/// The most input bytes a field may have before any character work: a limit of `max` after
/// normalisation allows at most 4x that in the input (NFC never shrinks text by more; CRLF and
/// BOMs only by a little). Longer is refused at once, so a 4 MB stem costs nothing.
fn raw_cap(max: Max) -> usize {
    match max {
        Max::Bytes(n) => n.saturating_mul(4).saturating_add(64),
        Max::Chars(n) => n.saturating_mul(16).saturating_add(64),
    }
}

/// NFC of a mapped stream into a `String`, stopping (with `None`) as soon as the output passes `max`.
#[cfg(test)]
fn nfc_bounded(it: Mapped<'_>, max: Max) -> Option<String> {
    nfc_bounded_as(it, max, false)
}

/// `nfc_bounded`; `ascii`: the input is ASCII, which NFC leaves as it is (no NFC pass).
fn nfc_bounded_as(it: Mapped<'_>, max: Max, ascii: bool) -> Option<String> {
    let mut out = String::new();
    let mut chars = 0usize;
    let mut push = |c: char| -> bool {
        out.push(c);
        chars += 1;
        match max {
            Max::Bytes(n) => out.len() <= n,
            // Line text is collapsed afterwards, which only shortens it; allow slack for that.
            Max::Chars(n) => chars <= n.saturating_mul(4).saturating_add(64),
        }
    };
    if ascii {
        for (c, _, _) in it {
            if !push(c) {
                return None;
            }
        }
    } else {
        for c in it.map(|m| m.0).nfc() {
            if !push(c) {
                return None;
            }
        }
    }
    Some(out)
}

/// Checks and canonicalises one text value: refuses a field over 4x its limit before reading it,
/// maps line breaks, the BOM and Symbol-font characters, refuses forbidden characters, stray
/// variation selectors and long mark runs, then applies NFC (Line: whitespace collapsed and
/// trimmed) and enforces `max`. Memory: the output only (bounded by `max`); the input is read in
/// place.
pub fn clean(s: &str, class: Class, max: Max) -> Result<String, TextError> {
    if s.len() > raw_cap(max) {
        return Err(TextError::TooLong);
    }
    check_stream(Mapped::new(s, class), class)?;
    let n = nfc_bounded_as(Mapped::new(s, class), max, s.is_ascii()).ok_or(TextError::TooLong)?;
    let out = match class {
        Class::Block => n,
        Class::Line => collapse(&n),
    };
    let ok = match max {
        Max::Bytes(m) => out.len() <= m,
        Max::Chars(m) => out.chars().count() <= m,
    };
    if !ok {
        return Err(TextError::TooLong);
    }
    // NFC composes only assigned characters; the check is cheap and keeps the guarantee local.
    check(&out, class)?;
    Ok(out)
}

/// Like `clean` (Line) but with NFKC, for publish metadata (titles, tags, standards), matching
/// `cleanText` in `server/auth/src/validate.ts`.
pub fn clean_meta(s: &str, max_chars: usize) -> Result<String, TextError> {
    if s.len() > raw_cap(Max::Chars(max_chars)) {
        return Err(TextError::TooLong);
    }
    check_stream(Mapped::new(s, Class::Line), Class::Line)?;
    let mut n = String::new();
    for (i, c) in Mapped::new(s, Class::Line).map(|m| m.0).nfkc().enumerate() {
        if i > max_chars.saturating_mul(4).saturating_add(64) {
            return Err(TextError::TooLong);
        }
        n.push(c);
    }
    let out = collapse(&n);
    if out.chars().count() > max_chars {
        return Err(TextError::TooLong);
    }
    check(&out, Class::Line)?;
    Ok(out)
}

/// Runs of Unicode whitespace become one ASCII space; leading and trailing whitespace go.
pub fn collapse(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for w in s.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(w);
    }
    out
}

/// At least one letter, number, punctuation mark or symbol.
pub fn has_visible(s: &str) -> bool {
    s.chars().any(is_visible)
}

pub fn is_visible(c: char) -> bool {
    !c.is_whitespace()
        && !is_tag(c as u32)
        && matches!(
            get_general_category(c),
            GeneralCategory::UppercaseLetter
                | GeneralCategory::LowercaseLetter
                | GeneralCategory::TitlecaseLetter
                | GeneralCategory::ModifierLetter
                | GeneralCategory::OtherLetter
                | GeneralCategory::DecimalNumber
                | GeneralCategory::LetterNumber
                | GeneralCategory::OtherNumber
                | GeneralCategory::ConnectorPunctuation
                | GeneralCategory::DashPunctuation
                | GeneralCategory::OpenPunctuation
                | GeneralCategory::ClosePunctuation
                | GeneralCategory::InitialPunctuation
                | GeneralCategory::FinalPunctuation
                | GeneralCategory::OtherPunctuation
                | GeneralCategory::MathSymbol
                | GeneralCategory::CurrencySymbol
                | GeneralCategory::ModifierSymbol
                | GeneralCategory::OtherSymbol
        )
        && !matches!(c as u32, 0x115F | 0x1160 | 0x3164 | 0xFFA0 | 0x2800)
}

/// The comparison form used by `match_hash`: NFC; within each line, runs of spaces and tabs
/// collapsed to one space and the ends trimmed; line breaks kept (a table is not a sentence);
/// leading and trailing blank lines removed.
pub fn hash_form(s: &str) -> String {
    // What students are served (`format::served_text`: invisible joiners and hyphens removed,
    // spacing collapsed, NFC), so the author's form and the served form hash alike.
    let served = crate::format::served_text(s);
    // (ASCII is its own NFC.)
    let n: String = if served.is_ascii() { Mapped::new(&served, Class::Block).map(|m| m.0).collect() } else { Mapped::new(&served, Class::Block).map(|m| m.0).nfc().collect() };
    let lines: Vec<String> = n.split('\n').map(collapse).collect();
    lines.join("\n").trim_matches('\n').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    const B: Max = Max::Bytes(1 << 20);

    #[test]
    fn refused_everywhere() {
        for bad in [
            "a\u{0}b", "a\u{7}b", "a\u{1b}b", "a\u{7f}b", "a\u{85}b", "a\u{202A}b", "a\u{202B}b", "a\u{202C}b", "a\u{202D}b",
            "a\u{202E}b", "a\u{2066}b", "a\u{2067}b", "a\u{2068}b", "a\u{2069}b", "a\u{206A}b", "a\u{206F}b", "a\u{FFF9}b",
            "a\u{FFFB}b", "a\u{E0001}b", "a\u{1D173}b", "a\u{E000}b", "a\u{F8FF}b", "a\u{F0000}b", "a\u{FDD0}b", "a\u{FFFE}b",
            "a\u{10FFFF}b", "a\u{0378}b", "a\u{115F}b", "a\u{1160}b", "a\u{3164}b", "a\u{FFA0}b", "a\u{2800}b",
            // Tag characters outside a complete flag sequence.
            "a\u{E0041}b", "Pick\u{E0049}\u{E0047}\u{E004E}", "\u{1F3F4}\u{E0067}\u{E0062}", "\u{1F3F4}\u{E0067}\u{E007F}",
            "x\u{E007F}",
        ] {
            assert!(clean(bad, Class::Line, B).is_err(), "{}", bad.escape_unicode());
            assert!(clean(bad, Class::Block, B).is_err(), "{}", bad.escape_unicode());
        }
    }

    #[test]
    fn mapped_not_refused() {
        assert_eq!(clean("a\r\nb\rc\u{B}d\u{2028}e\u{2029}f", Class::Block, B).as_deref(), Ok("a\nb\nc\nd\ne\nf"));
        assert_eq!(clean("\u{FEFF}What?", Class::Block, B).as_deref(), Ok("What?"));
        assert_eq!(clean("a\u{2028}b\u{B}c", Class::Line, B).as_deref(), Ok("a b c"));
        assert_eq!(clean("\u{FEFF}Title", Class::Line, B).as_deref(), Ok("Title"));
    }

    #[test]
    fn allowed_in_block_text() {
        for good in [
            "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}",
            "\u{2764}\u{FE0F}",
            "\u{645}\u{6CC}\u{200C}\u{62E}\u{648}\u{627}\u{647}\u{645}",
            "\u{915}\u{94D}\u{200D}\u{937}",
            "\u{5D4}\u{5E0}\u{5D5}\u{5E1}\u{5D7}\u{5D4} \u{200E}H\u{2082}O\u{200E}",
            "x \u{61C} y", "\u{E2A}\u{E27}\u{E31}\u{E2A}\u{200B}\u{E14}\u{E35}",
            "photo\u{AD}synthesis", "2\u{2062}x", "a\u{2060}b",
            "\u{1F3F4}\u{E0067}\u{E0062}\u{E0073}\u{E0063}\u{E0074}\u{E007F}",
            "\u{1F3F4}\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F} flag?",
            "\u{1F44B}\u{1F3FD}", "\u{1D465}", "x\u{302} + y\u{303}", "\u{8FBB}\u{E0100}", "a\tb\nc",
            "\u{1820}\u{180E}\u{1820}", "\u{1820}\u{180B}", "x\u{110BD}", "\u{600}123", "\u{70F}\u{710}", "\u{6DD}1", "\u{8E2}1",
        ] {
            assert!(clean(good, Class::Block, B).is_ok(), "{}", good.escape_unicode());
            assert!(has_visible(good), "{}", good.escape_unicode());
        }
        assert!(clean("\u{645}\u{6CC}\u{200C}\u{62E}", Class::Line, B).is_ok());
        assert!(clean("\u{1F468}\u{200D}\u{1F469}", Class::Line, B).is_ok());
        assert!(clean("\u{5D0} \u{200F}x", Class::Line, B).is_ok());
        assert!(clean("\u{1F3F4}\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}", Class::Line, B).is_ok(), "a flag in a topic");
        for bad in ["a\u{200B}b", "a\u{AD}b", "a\u{2062}b", "\u{600}1", "a\nb", "a\tb"] {
            assert!(clean(bad, Class::Line, B).is_err(), "{}", bad.escape_unicode());
        }
    }

    #[test]
    fn offsets_count_the_input_as_sent() {
        assert_eq!(clean("ab\u{202E}", Class::Block, B), Err(TextError::Forbidden { offset: 2, offset16: 2, cp: 0x202E }));
        // An astral character before the refused one: char offset 1, UTF-16 offset 2.
        assert_eq!(clean("\u{1D465}\u{202E}", Class::Block, B), Err(TextError::Forbidden { offset: 1, offset16: 2, cp: 0x202E }));
        // A dropped BOM and a CRLF before it do not shift the reported position.
        assert_eq!(clean("\u{FEFF}a\r\nb\u{202E}", Class::Block, B), Err(TextError::Forbidden { offset: 5, offset16: 5, cp: 0x202E }));
    }

    #[test]
    fn mark_runs_and_visibility() {
        assert!(clean(&format!("a{}", "\u{301}".repeat(MAX_MARK_RUN)), Class::Block, B).is_ok());
        // A Tibetan stack of nine marks is real text.
        assert!(clean("\u{F40}\u{F7A}\u{F74}\u{F71}\u{F72}\u{F80}\u{F82}\u{F83}\u{F37}\u{F39}", Class::Block, B).is_ok());
        assert_eq!(clean(&format!("a{}", "\u{301}".repeat(MAX_MARK_RUN + 1)), Class::Block, B), Err(TextError::MarkRun { offset: MAX_MARK_RUN + 1, offset16: MAX_MARK_RUN + 1 }));
        for blank in ["", "   ", "\u{200B}\u{200D}", "\u{301}", "\n\t", "\u{AD}", "\u{3164}"] {
            assert!(!has_visible(blank), "{}", blank.escape_unicode());
        }
    }

    #[test]
    fn bounded_normalisation() {
        let s = "\u{1D160}".repeat(10_000);
        assert_eq!(clean(&s, Class::Block, Max::Bytes(8192)), Err(TextError::TooLong));
        assert!(nfc_bounded(Mapped::new(&s, Class::Block), Max::Bytes(8192)).is_none());
        assert_eq!(nfc_bounded(Mapped::new("abc", Class::Block), Max::Bytes(3)).as_deref(), Some("abc"));
        assert_eq!(clean("e\u{301}", Class::Block, Max::Bytes(2)).as_deref(), Ok("\u{e9}"));
        assert_eq!(clean("  a \u{3000}  b ", Class::Line, B).as_deref(), Ok("a b"));
        assert_eq!(clean_meta("x\u{b2} \u{FB01}", 64).as_deref(), Ok("x2 fi"));
    }

    #[test]
    fn hash_form_keeps_lines() {
        assert_eq!(hash_form("  a  |\tb \n c | d\n\n"), "a | b\nc | d");
        assert_ne!(hash_form("a | b\nc | d"), hash_form("a | b c | d"));
        assert_ne!(hash_form("one\n\ntwo"), hash_form("one two"));
        assert_eq!(hash_form("Cafe\u{301}"), "Caf\u{e9}");
    }
}
