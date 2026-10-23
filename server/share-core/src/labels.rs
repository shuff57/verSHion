//! Choice labels and sibling references: what `strip` may never decide on its own.
//!
//! A hand-typed label ("A. Venus") keeps its original slot after a shuffle and gives the order
//! away; removing one that is really text ("A. thaliana", "I. only", "1: 2") corrupts the
//! question. So share-core **never strips labels silently**. It detects them broadly, shows the
//! author a before/after preview ([`LabelFinding`]), and the author decides per question or part:
//! [`LabelChoice::Strip`] or [`LabelChoice::Keep`]. A `hide_key` publish with an undecided finding
//! is refused (`labels_need_decision`).
//!
//! # Detection
//! A choice carries a **label** when it starts with (after optional whitespace):
//!
//! | Part | Accepted |
//! |---|---|
//! | optional word | `Option`, `Choice`, `Answer` (any case), then whitespace |
//! | optional opening bracket | `(` `[` and their full-width forms |
//! | token | one letter of the Latin (incl. full-width), Greek, Cyrillic or Hebrew alphabets, or a katakana/hiragana gojūon letter; an enclosed alphanumeric (Ⓐ ⓐ ① ⑴ ⒈ ⒜ ❶ ➀ ➊ 🄐 🄰 🅐 🅰); one or two decimal digits of any script; a roman numeral i-xxvi / I-XXVI or Ⅰ-Ⅻ / ⅰ-ⅻ |
//! | closer | the matching `)` `]` when a bracket was opened; otherwise one of `)` `]` `.` `:` `-` `–` `—` `、` `/` `・` (full-width forms included), spaces allowed before it ("A ) x"); not needed after an enclosed token or after the word form |
//! | after | whitespace, or the end, or nothing at all after `)` `]` `、` `・` a full-width closer or an enclosed token, or a capital letter after `.` ("A.Venus") |
//!
//! or ends with whitespace and a bracketed token ("Venus (A)"). Each token has values per
//! *style* (Latin letter, Greek, Cyrillic, Hebrew, kana, roman, number, enclosed) and the
//! position of each style's first letter is 1.
//!
//! A question has a **finding** when two or more choices carry a label, or exactly one does and
//! its value is its position (1-based) in some style. The labels are **order-revealing** when, in
//! one style, every labelled choice's value is its position plus one constant (A, B, C... and
//! also B, C, D...), or the single labelled choice is at its position.
//!
//! # Decisions
//! * **Strip** is allowed only when every choice carries a label of the same kind (all prefix or
//!   all suffix), the labels are exactly positions 1, 2, 3... in one style, and every result is
//!   visible and distinct after `hash_form`. Further layers ("A. (i) x") are removed while the same
//!   holds. If not allowed, asking for Strip is refused (`labels_strip_refused`).
//! * **Keep** leaves the text alone. If the labels are order-revealing, **nothing in the question
//!   is shuffled** (the order is preserved and listed in `needs_order_ack`); otherwise (labels that
//!   are text, like "E. coli", "S. aureus") the question is shuffled as usual.
//!
//! # Sibling references
//! A choice that names other choices cannot follow a shuffle: "Both A and B", "A and C only",
//! "I and II", "Neither A nor B", "II only", "A, C", "Ni A ni B", "А и Б", "AとB". Any such choice ⇒
//! the question is not shuffled and is listed in `needs_order_ack` (reason `sibling_refs`). A bare
//! token ("B", "4", "x") or a list of numbers/lowercase letters ("2, 3") is such a reference only
//! when another choice carries a label: alone, it is an answer. "All/None/Both/Neither of the above" (or
//! "of these") is fine where it is **locked** (it keeps its slot) and blocks the shuffle when it
//! is not.
#[allow(unused_imports)]
use crate::format::FastClass;
use crate::text::{collapse, has_visible};
use serde::{Deserialize, Serialize};
use unicode_general_category::{GeneralCategory, get_general_category};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LabelChoice {
    Strip,
    Keep,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Style {
    Latin,
    Greek,
    Cyrillic,
    Hebrew,
    Kana,
    Roman,
    Number,
    Enclosed,
}

/// One choice's label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    /// Byte range of the label in the text (a prefix starts at 0; a suffix ends at the end).
    pub start: usize,
    pub end: usize,
    pub suffix: bool,
    pub values: Vec<(Style, u32)>,
}

/// One choice in a finding: the text as it is and as Strip would leave it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Preview {
    pub before: String,
    pub after: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Analysis {
    pub labelled: usize,
    pub order_revealing: bool,
    pub strip_allowed: bool,
    /// Byte offset to cut from the start, and byte length to drop from the end, per choice, if
    /// stripped.
    #[serde(skip)]
    pub cut: Vec<(usize, usize)>,
}

impl Analysis {
    /// The before/after preview for the author (built only when asked for: it copies the text).
    pub fn previews(&self, texts: &[&str]) -> Vec<Preview> {
        texts.iter().zip(&self.cut).map(|(t, &c)| Preview { before: t.to_string(), after: apply(t, c) }).collect()
    }
}

const LATIN_UP: u32 = 'A' as u32;

fn kana_value(c: char) -> Option<u32> {
    const K: &str = "アイウエオカキクケコサシスセソタチツテトナニヌネノ";
    const H: &str = "あいうえおかきくけこさしすせそたちつてとなにぬねの";
    K.chars().position(|k| k == c).or_else(|| H.chars().position(|k| k == c)).map(|p| p as u32 + 1)
}

fn hebrew_value(c: char) -> Option<u32> {
    const A: &str = "אבגדהוזחטיכלמנסעפצקרשת";
    A.chars().position(|k| k == c).map(|p| p as u32 + 1)
}

fn greek_value(c: char) -> Option<u32> {
    const L: &str = "αβγδεζηθικλμνξοπρστυφχψω";
    const U: &str = "ΑΒΓΔΕΖΗΘΙΚΛΜΝΞΟΠΡΣΤΥΦΧΨΩ";
    L.chars().position(|k| k == c).or_else(|| U.chars().position(|k| k == c)).map(|p| p as u32 + 1)
}

fn cyrillic_value(c: char) -> Option<u32> {
    // The order Russian test makers letter choices in (no ё, й, ъ, ы, ь).
    const L: &str = "абвгдежзиклмнопрстуфхцчшщэюя";
    const U: &str = "АБВГДЕЖЗИКЛМНОПРСТУФХЦЧШЩЭЮЯ";
    L.chars().position(|k| k == c).or_else(|| U.chars().position(|k| k == c)).map(|p| p as u32 + 1)
}

/// Enclosed alphanumerics: (value, style) for ⓐ Ⓐ ① ⑴ ⒈ ⒜ ❶ ➀ ➊ 🄐 🄰 🅐 🅰 and roman Ⅰ ⅰ.
fn enclosed(c: char) -> Option<(Style, u32)> {
    let u = c as u32;
    let r = |base: u32, n: u32| (base..base + n).contains(&u).then(|| u - base + 1);
    r(0x24B6, 26)
        .or_else(|| r(0x24D0, 26))
        .or_else(|| r(0x249C, 26))
        .or_else(|| r(0x1F110, 26))
        .or_else(|| r(0x1F130, 26))
        .or_else(|| r(0x1F150, 26))
        .or_else(|| r(0x1F170, 26))
        .map(|v| (Style::Latin, v))
        .or_else(|| r(0x2460, 20).or_else(|| r(0x2474, 20)).or_else(|| r(0x2488, 20)).or_else(|| r(0x2776, 10)).or_else(|| r(0x2780, 10)).or_else(|| r(0x278A, 10)).map(|v| (Style::Number, v)))
        .or_else(|| r(0x2160, 12).or_else(|| r(0x2170, 12)).map(|v| (Style::Roman, v)))
}

fn fold(c: char) -> char {
    let u = c as u32;
    if (0xFF01..=0xFF5E).contains(&u) { char::from_u32(u - 0xFEE0).unwrap_or(c) } else { c }
}

fn wide(c: char) -> bool {
    (0xFF01..=0xFF5E).contains(&(c as u32))
}

/// The value of a decimal digit of any script (`None` if `c` is not Nd).
fn digit_value(c: char) -> Option<u32> {
    if get_general_category(c) != GeneralCategory::DecimalNumber {
        return None;
    }
    // Decimal digit blocks are runs of ten consecutive code points starting at zero; adjacent
    // blocks (the mathematical digits) are multiples of ten, so the value is the offset from the
    // start of the run modulo ten.
    let mut u = c as u32;
    let mut k = 0;
    while k < 60 && u > 0 && char::from_u32(u - 1).is_some_and(|p| get_general_category(p) == GeneralCategory::DecimalNumber) {
        u -= 1;
        k += 1;
    }
    Some(k % 10)
}

fn roman(t: &str) -> Option<u32> {
    const R: [&str; 26] = [
        "i", "ii", "iii", "iv", "v", "vi", "vii", "viii", "ix", "x", "xi", "xii", "xiii", "xiv", "xv", "xvi", "xvii", "xviii", "xix", "xx",
        "xxi", "xxii", "xxiii", "xxiv", "xxv", "xxvi",
    ];
    let lower = t.chars().all(|c| c.is_ascii_lowercase());
    let upper = t.chars().all(|c| c.is_ascii_uppercase());
    if !(lower || upper) || t.is_empty() {
        return None;
    }
    let l = t.to_ascii_lowercase();
    R.iter().position(|r| *r == l).map(|p| p as u32 + 1)
}

fn letter_values(c: char) -> Vec<(Style, u32)> {
    let f = fold(c);
    let mut v = Vec::new();
    if f.is_ascii_alphabetic() {
        v.push((Style::Latin, (f.to_ascii_uppercase() as u32) - LATIN_UP + 1));
    }
    if let Some(x) = greek_value(c) {
        v.push((Style::Greek, x));
    }
    if let Some(x) = cyrillic_value(c) {
        v.push((Style::Cyrillic, x));
    }
    if let Some(x) = hebrew_value(c) {
        v.push((Style::Hebrew, x));
    }
    if let Some(x) = kana_value(c) {
        v.push((Style::Kana, x));
    }
    v
}

const CLOSERS: [char; 11] = [')', ']', '.', ':', '-', '\u{2013}', '\u{2014}', '\u{3001}', '/', '\u{30FB}', '\u{FF65}'];

/// Reads a prefix label (see the module docs).
fn prefix(s: &str) -> Option<Hit> {
    let b: Vec<(usize, char)> = s.char_indices().collect();
    let mut i = 0;
    let ws = |i: &mut usize| {
        while *i < b.len() && b[*i].1.is_whitespace() {
            *i += 1;
        }
    };
    ws(&mut i);
    // A list bullet before the label ("• A. Venus", "- A. Venus"), removed with it.
    if b.get(i).is_some_and(|x| matches!(x.1, '\u{2022}' | '\u{25E6}' | '\u{25AA}' | '\u{2023}' | '\u{00B7}' | '-' | '*' | '\u{2013}'))
        && b.get(i + 1).is_some_and(|x| x.1 == ' ' || x.1 == '\t')
    {
        i += 1;
        ws(&mut i);
    }
    // The word form.
    let mut word = false;
    for w in ["option", "choice", "answer"] {
        let rest = &s[b.get(i).map_or(s.len(), |x| x.0)..];
        if rest.get(..w.len()).is_some_and(|p| p.eq_ignore_ascii_case(w)) && rest[w.len()..].starts_with(char::is_whitespace) {
            i += w.chars().count();
            ws(&mut i);
            word = true;
            break;
        }
    }
    let opened = b.get(i).is_some_and(|x| matches!(fold(x.1), '(' | '['));
    if opened {
        i += 1;
    }
    let start_tok = i;
    let (values, enc) = match b.get(i).map(|x| x.1) {
        Some(c) if enclosed(c).is_some() => {
            i += 1;
            (vec![enclosed(c)?], true)
        }
        Some(c) if digit_value(c).is_some() => {
            let d1 = digit_value(c)?;
            i += 1;
            let mut v = d1;
            if let Some(d2) = b.get(i).and_then(|x| digit_value(x.1)) {
                v = d1 * 10 + d2;
                i += 1;
            }
            if d1 == 0 || b.get(i).is_some_and(|x| digit_value(x.1).is_some()) {
                return None;
            }
            (vec![(Style::Number, v)], false)
        }
        Some(_) => {
            // A run of letters: one letter (any listed alphabet) or an ASCII roman numeral.
            let mut j = i;
            while j < b.len() && j - i < 6 && (fold(b[j].1).fast_alpha()) {
                j += 1;
            }
            let tok: String = b[i..j].iter().map(|x| fold(x.1)).collect();
            let mut v = Vec::new();
            if j - i == 1 {
                v.extend(letter_values(b[i].1));
            }
            if let Some(r) = roman(&tok) {
                v.push((Style::Roman, r));
            }
            if v.is_empty() {
                return None;
            }
            i = j;
            (v, false)
        }
        None => return None,
    };
    let _ = start_tok;
    // The closer.
    let mut k = i;
    while k < b.len() && k - i < 2 && b[k].1 == ' ' {
        k += 1;
    }
    let mut closer = None;
    if let Some(&(_, c)) = b.get(k) {
        let f = fold(c);
        let ok = if opened { matches!(f, ')' | ']') } else { CLOSERS.contains(&f) || CLOSERS.contains(&c) };
        if ok {
            closer = Some(c);
            i = k + 1;
        }
    }
    // "A.)" and "1.)": a full stop and a parenthesis together.
    if !opened && closer.is_some_and(|c| fold(c) == '.') && b.get(i).is_some_and(|x| x.1 == ')') {
        i += 1;
        closer = Some(')');
    }
    // "A<tab>Venus": a tab right after the label is its separator.
    if !opened && closer.is_none() && b.get(i).is_some_and(|x| x.1 == '\t') {
        i += 1;
        closer = Some('\t');
    }
    if opened && closer.is_none() {
        return None;
    }
    if closer.is_none() && !enc && !word {
        return None;
    }
    // What may follow.
    let next = b.get(i).map(|x| x.1);
    let tight = closer.is_some_and(|c| c == '\t' || matches!(fold(c), ')' | ']') || matches!(c, '\u{3001}' | '\u{30FB}' | '\u{FF65}') || wide(c)) || enc && closer.is_none();
    let ok = match next {
        None => true,
        Some(c) if c.is_whitespace() => true,
        Some(c) if tight => !c.is_whitespace(),
        Some(c) if closer.is_some_and(|x| fold(x) == '.') => c.is_uppercase(),
        _ => false,
    };
    if !ok {
        return None;
    }
    ws(&mut i);
    // A ratio, range or time ("1 : 2", "1 - 2 hours", "3 / 4"): a number, not a label.
    if !enc && closer.is_some_and(|c| matches!(fold(c), ':' | '-' | '\u{2013}' | '\u{2014}' | '/')) && b.get(i).is_some_and(|x| x.1.is_ascii_digit()) {
        return None;
    }
    Some(Hit { start: 0, end: b.get(i).map_or(s.len(), |x| x.0), suffix: false, values })
}

/// Reads a suffix label: whitespace then `(token)` or `[token]` at the end.
fn suffix(s: &str) -> Option<Hit> {
    let t = s.trim_end();
    let close = t.chars().last()?;
    let open = match fold(close) {
        ')' => '(',
        ']' => '[',
        _ => return None,
    };
    let body_end = t.len() - close.len_utf8();
    let open_at = t[..body_end].rfind(|c: char| fold(c) == open)?;
    let tok = &t[open_at + t[open_at..].chars().next()?.len_utf8()..body_end];
    let before = &t[..open_at];
    if !before.ends_with(char::is_whitespace) || !has_visible(before) {
        return None;
    }
    let mut v = Vec::new();
    let chars: Vec<char> = tok.chars().collect();
    if chars.len() == 1 {
        // (A Greek letter in brackets after a word names it: "Alpha (α)", "Positron (β⁺)".)
        v.extend(letter_values(chars[0]).into_iter().filter(|(st, _)| *st != Style::Greek));
        if let Some(e) = enclosed(chars[0]) {
            v.push(e);
        }
        if let Some(d) = digit_value(chars[0]).filter(|d| *d > 0) {
            v.push((Style::Number, d));
        }
    }
    if chars.len() == 2
        && let (Some(a), Some(b)) = (digit_value(chars[0]), digit_value(chars[1]))
        && a > 0
    {
        v.push((Style::Number, a * 10 + b));
    }
    let folded: String = chars.iter().map(|c| fold(*c)).collect();
    if let Some(r) = roman(&folded) {
        v.push((Style::Roman, r));
    }
    if v.is_empty() {
        return None;
    }
    Some(Hit { start: before.trim_end().len(), end: s.len(), suffix: true, values: v })
}

/// The label of one choice, prefix first.
pub fn detect(s: &str) -> Option<Hit> {
    prefix(s).filter(|h| !initial(s, h)).or_else(|| suffix(s))
}

/// A capital and a full stop that are a name's initial or a genus abbreviation, not a label:
/// followed by another initial ("B. F. Skinner", "A. P. J. Abdul Kalam") or by a lowercase word
/// ("E. coli", "S. aureus").
fn initial(s: &str, h: &Hit) -> bool {
    let head = s[h.start..h.end].trim();
    let mut hc = head.chars();
    let (Some(c), Some('.'), None) = (hc.next(), hc.next(), hc.next()) else { return false };
    if !c.is_uppercase() {
        return false;
    }
    let rest = s[h.end..].trim_start();
    let mut rc = rest.chars();
    // (Only an ASCII capital before another initial: "Ⅰ. x", "A. x" stay labels.)
    // ("A. A. x" is two label layers, the old "same label twice" case: not an initial here.)
    c.is_ascii_uppercase() && matches!((rc.next(), rc.next()), (Some(a), Some('.')) if a.is_ascii_uppercase() && a != c)
}

fn styles() -> [Style; 8] {
    [Style::Latin, Style::Greek, Style::Cyrillic, Style::Hebrew, Style::Kana, Style::Roman, Style::Number, Style::Enclosed]
}

fn value(h: &Hit, st: Style) -> Option<u32> {
    h.values.iter().find(|(s, _)| *s == st).map(|(_, v)| *v)
}

/// Whether the labelled choices are order-revealing (see the module docs).
fn revealing(hits: &[Option<Hit>]) -> bool {
    let labelled: Vec<(usize, &Hit)> = hits.iter().enumerate().filter_map(|(i, h)| h.as_ref().map(|h| (i, h))).collect();
    if labelled.is_empty() {
        return false;
    }
    styles().iter().any(|&st| {
        let vals: Option<Vec<i64>> = labelled.iter().map(|(i, h)| value(h, st).map(|v| v as i64 - *i as i64)).collect();
        // One label at its own position; or labels whose values rise with their positions
        // (A, B, C; B, C, D; and with gaps, A, B, D or A, C, E): any of these gives the order away.
        match vals {
            Some(d) if labelled.len() == 1 => d[0] == 1,
            Some(_) => labelled.windows(2).all(|w| matches!((value(w[0].1, st), value(w[1].1, st)), (Some(a), Some(b)) if b > a)),
            None => false,
        }
    })
}

/// Exactly positions 1, 2, 3... in one style, every choice labelled the same way.
fn exact(hits: &[Option<Hit>]) -> bool {
    if hits.len() < 2 || hits.iter().any(|h| h.is_none()) {
        return false;
    }
    let suffix = hits[0].as_ref().is_some_and(|h| h.suffix);
    if hits.iter().any(|h| h.as_ref().is_some_and(|h| h.suffix != suffix)) {
        return false;
    }
    styles().iter().any(|&st| hits.iter().enumerate().all(|(i, h)| h.as_ref().and_then(|h| value(h, st)) == Some(i as u32 + 1)))
}

/// Analyses one question's (or part's) choices in the author's order. `None` when there is no
/// finding.
pub fn analyze(texts: &[&str]) -> Option<Analysis> {
    // (A choice that is nothing but a number with a full stop, "2.", is its content: round 12,
    // critic23a "1. / 2. / 4. / Depends on population.".)
    let hits: Vec<Option<Hit>> = texts.iter().map(|t| detect(t).filter(|h| h.suffix || crate::text::has_visible(&t[h.end..]) || !t[..h.start].trim().is_empty() || !t[h.start..h.end].trim().trim_end_matches('.').chars().all(|c| c.is_ascii_digit()))).collect();
    let labelled = hits.iter().filter(|h| h.is_some()).count();
    let finding = labelled >= 2 || labelled == 1 && revealing(&hits);
    if !finding {
        return None;
    }
    let order_revealing = revealing(&hits);
    // Genus abbreviations ("E. coli", "S. aureus", "B. subtilis"): every choice a capital, a full
    // stop and a lowercase word, the capitals in no order: content, no label.
    let binomial = |t: &str, h: &Hit| {
        let head = t[h.start..h.end].trim();
        let rest = t[h.end..].trim_start();
        head.len() == 2 && head.ends_with('.') && head.chars().next().is_some_and(|c| c.is_ascii_uppercase()) && rest.split_whitespace().next().is_some_and(|w| w.chars().count() >= 4 && w.chars().all(char::is_lowercase))
    };
    if !order_revealing && labelled == texts.len() && texts.iter().zip(&hits).all(|(t, h)| h.as_ref().is_some_and(|h| binomial(t, h))) {
        return None;
    }
    // Strip: remove layers while every choice keeps an exact position label.
    let n = texts.len();
    let mut cut: Vec<(usize, usize)> = vec![(0, 0); n];
    let mut layer = hits.clone();
    let mut layers = 0;
    // At most 8 layers ("A. (i) x" is two); more is not a label, and the leftover rule below then
    // refuses Strip. This also bounds the work per question.
    // A second layer is removed only when it is a different label ("A. (i) x"), never the same
    // label again ("A. A. Einstein": the initial is part of the name).
    let mut prev_labels: Option<Vec<String>> = None;
    while exact(&layer) && layers < 8 {
        let these: Vec<String> = (0..n).map(|i| layer[i].as_ref().map_or(String::new(), |h| texts[i][cut[i].0..texts[i].len() - cut[i].1][h.start..h.end].trim().to_string())).collect();
        if prev_labels.as_ref() == Some(&these) {
            break;
        }
        let next: Vec<(usize, usize)> = (0..n)
            .map(|i| {
                let h = layer[i].as_ref().map_or((0, 0), |h| (h.start, h.end));
                let suffix = layer[i].as_ref().is_some_and(|h| h.suffix);
                let (a, z) = cut[i];
                if suffix { (a, z + (texts[i].len() - z - a - h.0)) } else { (a + h.1, z) }
            })
            .collect();
        // The texts are already NFC (validated), so the comparison form only needs whitespace
        // collapsed per line: no normalisation work per layer.
        let results: Vec<String> = (0..n).map(|i| lines_collapsed(&texts[i][next[i].0..texts[i].len() - next[i].1])).collect();
        let distinct = {
            let mut r = results.clone();
            r.sort();
            r.dedup();
            r.len() == n
        };
        if !distinct || results.iter().any(|r| r.is_empty() || !has_visible(r)) {
            break;
        }
        cut = next;
        layers += 1;
        prev_labels = Some(these);
        layer = (0..n).map(|i| prefix(&texts[i][cut[i].0..texts[i].len() - cut[i].1])).collect();
    }
    // Labels still left after the last removable layer (a 65th layer, or one whose removal would
    // empty or merge choices) would give the order away after a shuffle: then Strip is refused.
    let rest: Vec<Option<Hit>> = (0..n).map(|i| detect(&texts[i][cut[i].0..texts[i].len() - cut[i].1])).collect();
    let leftover = rest.iter().any(|h| h.is_some()) && revealing(&rest);
    let strip_allowed = layers > 0 && !leftover;
    if !strip_allowed {
        cut = vec![(0, 0); n];
    }
    Some(Analysis { labelled, order_revealing, strip_allowed, cut })
}

/// [`analyze`] for a Matching right column. The column's labels give the key away only when
/// they count the pairs off one by one ("a, b, c", "(1), (2), (3)", "b, c, d"): the item at
/// position `k` pairs with left item `k`. Labels that merely rise ("E. coli", "S. cerevisiae") are
/// text, as an alphabetical column is. Only the first `paired` items (the pairs' own rights) can
/// give anything away: a keyless question's bank (no rights in its pairs) never does.
pub fn analyze_column(texts: &[&str], paired: usize) -> Option<Analysis> {
    let mut a = analyze(texts)?;
    let hits: Vec<Option<Hit>> = texts.iter().take(paired).map(|t| detect(t)).collect();
    let labelled: Vec<(usize, &Hit)> = hits.iter().enumerate().filter_map(|(i, h)| h.as_ref().map(|h| (i, h))).collect();
    a.order_revealing = !labelled.is_empty()
        && styles().iter().any(|&st| {
        let d: Option<Vec<i64>> = labelled.iter().map(|(i, h)| value(h, st).map(|v| v as i64 - *i as i64)).collect();
        match d {
            Some(d) if d.len() == 1 => d[0] == 1,
            Some(d) => d.windows(2).all(|w| w[0] == w[1]),
            None => false,
        }
    });
    Some(a)
}

/// `hash_form` for text that is already NFC.
fn lines_collapsed(s: &str) -> String {
    let lines: Vec<String> = s.split('\n').map(collapse).collect();
    lines.join("\n").trim_matches('\n').to_string()
}

/// The text Strip leaves.
pub fn apply(text: &str, cut: (usize, usize)) -> String {
    text[cut.0..text.len() - cut.1].to_string()
}

// ---- sibling references ---------------------------------------------------------------------

/// The kinds of reference a choice makes to its siblings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SiblingRef {
    /// "All/None of the above", "Todas las anteriores", "Все перечисленные": names *every* other
    /// choice, so it is safe where it is locked.
    Above,
    /// "Both A and B", "I and II only", "A, C", "Ni A ni B", "А и Б", "AとB", "The first two",
    /// bare "Both": names particular choices, so the question is never shuffled.
    Labels,
    /// A bare token that could be a label ("B", "4", "x", "C") or a list of digits and lowercase
    /// letters ("2, 3", "x and y"): a reference only when the other choices carry labels
    /// ([`refers_to_siblings`]). Alone, these are answers ("4", "x", "π", "C" for carbon).
    Weak,
}

/// Words that start an "all / none of the above" choice, in about 25 languages.
const ALL_WORDS: &[&str] = &[
    "all", "none", "any", "each", "every", "todas", "todos", "ninguna", "ninguno", "toutes", "tous", "aucune", "aucun", "alle", "keine",
    "keiner", "tutte", "tutti", "nessuna", "nessuno", "nenhuma", "nenhum", "все", "всё", "ни", "ничего", "geen", "wszystkie",
    "żadne", "żadna", "allt", "inget", "ingen", "semua", "tidak", "tất", "không", "hepsi", "hiçbiri", "kaikki", "mikään", "alla",
    "toate", "niciuna", "όλα", "κανένα", "כל", "ninguém",
];
/// Words that may follow them ("of the above", "les réponses précédentes", "der oben genannten").
const ABOVE_FILLER: &[&str] = &[
    "of", "the", "above", "these", "them", "those", "preceding", "previous", "foregoing", "listed", "mentioned", "given", "options",
    "choices", "answers", "answer", "statements", "are", "is", "correct", "true", "right", "here", "abovementioned", "above-mentioned",
    "others", "other", "rest", "remaining", "alternatives",
    "de", "las", "los", "anteriores", "anterior", "respuestas", "opciones", "son", "correctas", "les", "réponses", "précédentes",
    "ces", "ci-dessus", "sont", "bonnes", "der", "oben", "genannten", "obigen", "antworten", "sind", "richtig", "aussagen", "vorherigen",
    "le", "precedenti", "delle", "risposte", "sono", "corrette", "sopra", "as", "das", "dos", "respostas", "estão", "corretas",
    "перечисленное", "перечисленные", "перечисленных", "вышеперечисленное", "вышеперечисленные", "из", "один", "ответы", "верны",
    "варианты", "van", "bovenstaande", "antwoorden", "zijn", "juist", "z", "powyższe", "powyższych", "odpowiedzi", "są", "poprawne",
    "ovanstående", "svaren", "är", "rätt", "benar", "di", "atas", "jawaban", "đều", "đúng", "trên", "cả", "câu", "doğru", "yukarıdakilerin",
    "yukarıdakiler", "edellä", "mainitut", "ovan", "nämnda", "de", "mai", "sus", "răspunsurile", "παραπάνω", "το", "τα",
];
/// Whole choices (after case folding) that mean "all / none of the above" in scripts without
/// spaces, or as one word.
const ABOVE_WHOLE: &[&str] = &["hepsi", "hiçbiri", "모두 정답", "위의 모든 것", "모두", "以上都对", "以上皆是", "以上都不是", "以上皆非", "上記のすべて", "上記のいずれでもない", "上述皆是", "全部正确", "すべて正しい"];
/// "All / none of the above" in about 60 languages, whole choices (round 9, critic20a).
const ABOVE_LOCALIZED: &[&str] = &[
    // Nordic
    "alle de ovennevnte", "ingen av de ovennevnte", "alle ovenstående", "ingen af ovenstående", "alla ovanstående", "inget av ovanstående",
    "allt ofangreint", "ekkert af ofangreindu", "kaikki edellä mainitut", "ei mikään edellä mainituista", "kõik eelnevad", "mitte ükski eelnevatest",
    // Central and eastern Europe
    "všechny výše uvedené", "žádná z výše uvedených", "všetky uvedené", "žiadna z uvedených", "mindegyik fenti", "egyik sem a fentiek közül",
    "mind a fentiek", "toate cele de mai sus", "niciuna dintre cele de mai sus", "wszystkie powyższe", "żadne z powyższych", "усі перелічені",
    "жоден з перелічених", "всі перелічені", "всички изброени", "нито едно от изброените", "сви наведени", "ниједан од наведених", "svi navedeni",
    "nijedan od navedenih", "vsi navedeni", "noben od navedenih", "visi iepriekš minētie", "neviens no iepriekš minētajiem", "visi išvardyti",
    "nė vienas iš išvardytų", "все вышеперечисленные", "ничего из вышеперечисленного", "все перечисленные", "ни один из перечисленных",
    "sve navedeno", "të gjitha më sipër", "asnjë nga më sipër", "сите наведени", "ниту еден од наведените",
    // Western Europe
    "totes les anteriors", "cap de les anteriors", "aurreko guztiak", "aurrekoetako bat ere ez", "pob un o'r uchod", "dim un o'r uchod",
    "iad seo ar fad", "ceann ar bith díobh seo", "todas las anteriores", "ninguna de las anteriores", "todas as anteriores", "nenhuma das anteriores",
    "toutes les réponses", "toutes ces réponses", "aucune de ces réponses", "tutte le precedenti", "nessuna delle precedenti", "alle bovenstaande",
    "geen van bovenstaande", "alle oben genannten", "keine der oben genannten", "alle genannten", "keine der genannten", "alle antworten",
    "kollha ta' hawn fuq", "l-ebda waħda minn ta' hawn fuq", "τα παραπάνω", "όλα τα παραπάνω", "κανένα από τα παραπάνω",
    // Middle East, South and Central Asia, Africa
    "جميع ما سبق", "لا شيء مما سبق", "كل ما سبق", "כל התשובות נכונות", "כל התשובות", "אף אחת מהתשובות", "כל הנ\"ל", "אף אחד מהנ\"ל",
    "همه موارد", "هیچکدام", "هیچ‌کدام", "همه موارد بالا", "مندرجہ بالا سبھی", "مندرجہ بالا میں سے کوئی نہیں", "उपर्युक्त सभी",
    "उपर्युक्त में से कोई नहीं", "उपरोक्त सभी", "इनमें से कोई नहीं", "উপরের সবগুলো", "উপরের কোনোটিই নয়", "মেধ সবগুলো", "மேற்கூறிய அனைத்தும்",
    "மேற்கூறியவற்றில் எதுவுமில்லை", "పైవన్నీ", "ఏదీ కాదు", "ಮೇಲಿನ ಎಲ್ಲವೂ", "ಮೇಲಿನ ಯಾವುದೂ ಅಲ್ಲ", "മുകളിൽ പറഞ്ഞവയെല്ലാം", "वरील सर्व",
    "उपरोक्त सबै", "ઉપરના બધા", "ਉਪਰੋਕਤ ਸਾਰੇ", "ሁሉም መልሶች", "ከላይ ያሉት ሁሉም", "yote yaliyotajwa", "hakuna kati ya hayo", "zonke ezingenhla",
    "duk na sama", "gbogbo awọn ti o wa loke", "ყველა ზემოთ ჩამოთვლილი", "არცერთი ზემოთ ჩამოთვლილი", "բոլորը", "ոչ մեկը", "жоғарыдағылардың бәрі",
    "жоғарыдағылардың ешқайсысы", "дээрх бүгд", "дээрхийн аль нь ч биш", "yuqoridagilarning hammasi", "yuxarıdakıların hamısı", "hepsi doğru",
    "yukarıdakilerin hepsi", "yukarıdakilerin hiçbiri", "אַלע אויבנדערמאָנטע",
    // South-east and east Asia
    "ถูกทุกข้อ", "ไม่มีข้อใดถูก", "ทั้งหมดที่กล่าวมา", "tất cả các đáp án trên", "tất cả các ý trên", "không có đáp án nào", "lahat ng nabanggit",
    "wala sa nabanggit", "semua di atas", "semua jawaban benar", "tidak ada yang benar", "semua yang di atas", "ទាំងអស់ខាងលើ", "ທັງໝົດຂ້າງເທິງ",
    "အထက်ပါအားလုံး", "以上全部", "以上都是", "以上皆对", "以上均正确", "以上均不正确", "以上いずれも", "全て", "すべて", "위 모두", "위의 모든 것",
];
/// Markers of "above" in scripts without spaces.
const ABOVE_CJK: &[&str] = &["以上", "上記", "上述", "위의", "上面"];

/// Phrases that name particular choices by position (the choice is short).
const POSITIONS: &[&str] = &[
    "first two", "first three", "last two", "last three", "previous two", "previous three", "preceding two", "former",
    "latter", "the first and", "the second and", "the first one", "the last one", "both of", "neither of", "two above",
    "first and the third", "first and third",
];

/// Words that join or qualify choice names, in about 25 languages ("A and B", "A + B", "all except
/// C", "B only", "ni A ni B", "sowohl A als auch B", "tanto A como B", "A as well as B", "A through
/// C", "Weder A noch B", "Sia A che B", "A ainsi que B", "A en B", "A ve B", "A och B", "A dan B",
/// "A và B", "А и Б"). Words of other scripts of up to 4 characters ("と", "および", "와") count too.
const JOINERS: &[&str] = &[
    "and", "or", "nor", "and/or", "&", "+", "/", "y", "o", "et", "ou", "und", "oder", "e", "but", "except", "not", "only", "both",
    "neither", "either", "plus", "ni", "sowohl", "als", "auch", "tanto", "como", "и", "или", "ни", "только", "solo", "as", "well",
    "along", "with", "together", "through", "thru", "to", "bis", "weder", "noch", "sia", "che", "né", "ne", "ainsi", "que", "en",
    "ve", "och", "og", "dan", "và", "i", "oraz", "lub", "albo", "zowel", "ja", "tai", "eller", "samt", "-", "\u{2013}", "\u{2014}",
    "~", "\u{3001}", "\u{FF0C}", "also", "ambas", "ambos", "beide", "оба", "обе", "seulement", "nur",
];

/// Joiners that make a list a reference whatever its tokens ("both a and b", "only 2").
const QUALIFIERS: &[&str] = &["both", "neither", "nor", "ni", "ни", "except", "not", "only", "solo", "только", "but", "either", "sowohl", "tanto", "and/or", "weder", "né"];

/// `JOINERS` and `QUALIFIERS` as sets (a lookup per token, not a scan of the table).
fn joiner_set() -> &'static std::collections::HashSet<&'static str> {
    static S: std::sync::OnceLock<std::collections::HashSet<&'static str>> = std::sync::OnceLock::new();
    S.get_or_init(|| JOINERS.iter().copied().collect())
}
fn qualifier_set() -> &'static std::collections::HashSet<&'static str> {
    static S: std::sync::OnceLock<std::collections::HashSet<&'static str>> = std::sync::OnceLock::new();
    S.get_or_init(|| QUALIFIERS.iter().copied().collect())
}

/// Characters that separate list items even without spaces ("A+B", "AとB", "A和B", "A、B").
fn splits(c: char) -> bool {
    matches!(c, '+' | '&' | '/' | 'と' | '和' | '、' | '或' | '及' | '与' | '，')
}

/// A letter of a script without spaces between words (CJK, kana, Hangul, Thai).
fn unspaced_script(c: char) -> bool {
    matches!(c as u32, 0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF | 0x1100..=0x11FF | 0x0E00..=0x0E7F | 0xF900..=0xFAFF)
}

/// "a", "(b)", "C.", "iv", "3)": what a choice's own label looks like, standing alone.
fn label_token(w: &str) -> bool {
    // A quoted letter is a string ("'a'"), not a choice's name.
    if w.starts_with(['"', '\'', '\u{201C}', '\u{2018}', '`']) {
        return false;
    }
    let core = w.trim_matches(|c: char| matches!(c, '(' | ')' | '[' | ']' | '.' | ',' | ':' | ';' | '?' | '!'));
    if core.is_empty() || core.chars().count() > 5 {
        return false;
    }
    // (A single ASCII letter or digit: no table to read.)
    if let [b] = core.as_bytes()
        && (b.is_ascii_alphanumeric())
    {
        return true;
    }
    let mut cs = core.chars();
    let single = cs.next().filter(|_| cs.next().is_none());
    match single {
        // Latin and Cyrillic letters name choices; a Greek letter in a choice is maths ("Σ 1/n").
        Some(c) if c.fast_alpha() && letter_values(c).iter().any(|(st, _)| *st != Style::Greek) => true,
        Some(c) if c.is_ascii_digit() => true,
        _ => roman(&core.to_ascii_lowercase()).is_some() || core.parse::<u32>().is_ok_and(|n| (1..=26).contains(&n)),
    }
}

/// A label token that is also an everyday answer: a number or a lowercase letter ("2", "x").
fn weak_token(w: &str) -> bool {
    let core = w.trim_matches(|c: char| !c.fast_alnum());
    core.chars().all(|c| c.is_ascii_digit()) || core.chars().count() == 1 && core.chars().all(|c| c.is_lowercase())
}

/// An uppercase Roman numeral of two or more letters, or "I": in "I only", "I and II" these name
/// the stem's statements, not the choices.
fn roman_token(w: &str) -> bool {
    let core = w.trim_matches(|c: char| !c.fast_alnum());
    !core.is_empty() && core.chars().all(|c| matches!(c, 'I' | 'V' | 'X')) && roman(&core.to_ascii_lowercase()).is_some()
}

/// The tokens `sibling_ref` reads: whitespace and commas split, `+ & /` and connectors of other
/// scripts split even without spaces ("AとB", "A와 B", "AおよびB"), ranges split ("A-C", "A–C").
fn sib_tokens(text: &str) -> Vec<String> {
    let mut toks: Vec<String> = Vec::new();
    for w in text.split(|c: char| c.is_whitespace() || c == ',' || c == ';').filter(|w| !w.is_empty()) {
        if w.eq_ignore_ascii_case("and/or") {
            toks.push(w.to_string());
            continue;
        }
        let mut cur = String::new();
        let mut cur_unspaced = false;
        for c in w.chars() {
            let u = unspaced_script(c);
            // A sign at the end of a word is a charge ("K+", "Cl-"), not a joiner; a joiner of an
            // unspaced script ("と", "和") splits only between Latin letters or digits ("AとB", not
            // "ありがとう").
            let cjk_split = !matches!(c, '+' | '&' | '/' | '\u{FF0C}');
            // (Round 19: read only for a splitting character; it was computed, with a search of
            // the word, for every character of every word.)
            let latin_around = || cur.chars().last().is_some_and(|p| p.is_ascii_alphanumeric()) && w[w.find(c).map_or(0, |b| b + c.len_utf8())..].chars().next().is_some_and(|n| n.is_ascii_alphanumeric());
            if splits(c) && !(matches!(c, '+' | '&' | '/') && w.ends_with(c) && !cur.is_empty()) && (!cjk_split || latin_around()) {
                if !cur.is_empty() {
                    toks.push(std::mem::take(&mut cur));
                }
                toks.push(c.to_string());
                continue;
            }
            if !cur.is_empty() && u != cur_unspaced {
                toks.push(std::mem::take(&mut cur));
            }
            cur_unspaced = u;
            cur.push(c);
        }
        if !cur.is_empty() {
            toks.push(cur);
        }
    }
    // Ranges: "A-C", "II–IV".
    let mut out = Vec::with_capacity(toks.len());
    for t in toks {
        let parts: Vec<&str> = t.split(['-', '\u{2013}', '\u{2014}']).collect();
        // A range runs forward between two different labels ("A-C"; "C-C" is a bond).
        if parts.len() == 2 && parts[0] < parts[1] && parts.iter().all(|p| !p.is_empty() && p.chars().count() <= 4 && label_token(p) && !p.chars().all(|c| c.is_ascii_digit())) {
            out.push(parts[0].to_string());
            out.push("-".into());
            out.push(parts[1].to_string());
        } else {
            out.push(t);
        }
    }
    out
}

/// The label values a letter list names ("A and C", "Both (b) and (d)", "II, III"): (an
/// uppercase Latin letter, its position) per label token, other words skipped ("Both A and B are
/// correct"); None for a reference without letter tokens ("Both", "The first two").
/// Readings of a letter list: per reading, (an uppercase Latin letter, its value) per token.
type Readings = Vec<Vec<(bool, u32)>>;

fn letter_list(text: &str) -> Option<Readings> {
    // (Letter lists are short: nothing longer is read.)
    if text.len() > 160 {
        return None;
    }
    // (Not the choice's own label, "c) Both (a) and (b)", "A. Both A and B": written unlike the
    // labels it names. "(a) and (c)" starts with a name, not a label.)
    let text = match detect(text) {
        Some(h) if h.start == 0 && has_visible(&text[h.end..]) => {
            let rest = &text[h.end..];
            let first_label = sib_tokens(&collapse(rest)).into_iter().find(|t| label_token(t));
            let own = text[..h.end].trim();
            match first_label {
                Some(t) if t.starts_with('(') != own.starts_with('(') || t.ends_with(['.', ')']) != own.ends_with(['.', ')']) => rest,
                _ => text,
            }
        }
        _ => text,
    };
    let low = collapse(text);
    if low.len() > 120 {
        return None;
    }
    let toks = sib_tokens(&low);
    let bare = |w: &str| w.trim_matches(|c: char| !c.fast_alnum() && !splits(c) && !matches!(c, '-' | '\u{2013}' | '\u{2014}' | '~')).to_lowercase();
    // Two readings: letters ("C" = 3) and, when every token is one, Roman numerals ("I" = 1).
    let (mut letters, mut romans) = (Some(Vec::new()), Some(Vec::new()));
    for (k, t) in toks.iter().enumerate() {
        let core = t.trim_matches(|c: char| !c.fast_alnum());
        // (A one-letter joiner between two labels: "a y b", "A e B".)
        let joiner = JOINERS.contains(&bare(t).as_str()) && k > 0 && toks.get(k + 1).is_some_and(|n| label_token(n)) && label_token(&toks[k - 1]);
        if label_token(t) && !joiner && !core.chars().all(|c| c.is_ascii_digit()) {
            let mut cs = core.chars();
            let single = match (cs.next(), cs.next()) {
                (Some(c), None) => Some(c),
                _ => None,
            };
            match single.and_then(|c| letter_values(c).into_iter().find(|(st, _)| *st == Style::Latin).or_else(|| letter_values(c).into_iter().next()).map(|x| (c, x))) {
                Some((c, (st, v))) => {
                    if let Some(l) = letters.as_mut() {
                        l.push((st == Style::Latin && c.is_ascii_uppercase() && !t.starts_with('('), v));
                    }
                }
                None => letters = None,
            }
            match roman(&core.to_ascii_lowercase()) {
                Some(v) if core.chars().all(|c| matches!(c, 'I' | 'V' | 'X' | 'i' | 'v' | 'x')) => {
                    if let Some(r) = romans.as_mut() {
                        r.push((false, v));
                    }
                }
                _ => romans = None,
            }
        }
    }
    let _ = bare;
    let out: Vec<Vec<(bool, u32)>> = [letters, romans].into_iter().flatten().filter(|l| !l.is_empty()).collect();
    (!out.is_empty()).then_some(out)
}

/// Whether the first label token of a letter list follows a noun ("Type A", "Column A and C").
fn noun_before_letters(text: &str) -> bool {
    // (The first 200 bytes, like `sibling_ref`: `sib_tokens` reads each word once per splitter.)
    let mut cut = text.len().min(200);
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    let toks = sib_tokens(&collapse(&text[..cut]));
    let Some(i) = toks.iter().position(|t| label_token(t)) else { return false };
    if i == 0 {
        return false;
    }
    let prev: String = toks[i - 1].chars().filter(|c| c.fast_alpha()).flat_map(|c| c.to_lowercase()).collect();
    prev.chars().count() >= 3
        && !JOINERS.contains(&prev.as_str())
        && !QUALIFIERS.contains(&prev.as_str())
        && !ABOVE_FILLER.contains(&prev.as_str())
        && !matches!(prev.as_str(), "choice" | "choices" | "option" | "options" | "answer" | "answers" | "statement" | "statements" | "letters" | "letter" | "all" | "except" | "both" | "neither" | "either" | "ambas" | "ambos" | "beide" | "opciones" | "réponses" | "antworten")
}

/// A phrase pointing at the choices above ("Both of the above", "Neither of the above").
fn above_words(text: &str) -> bool {
    let low = text.to_lowercase();
    low.split(|c: char| !c.fast_alnum()).any(|w| matches!(w, "above" | "preceding" | "previous" | "anteriores" | "précédentes" | "oben" | "obigen" | "sopra" | "precedenti" | "powyższe" | "bovenstaande" | "ovanstående" | "yukarıdaki" | "παραπάνω")) || ABOVE_CJK.iter().any(|k| low.contains(k))
}

/// "All / none of the above" as the whole choice (see [`ALL_WORDS`], [`ABOVE_FILLER`]).
fn above_phrase(low: &str) -> bool {
    let t = low.trim_matches(|c: char| matches!(c, '.' | '!' | '。' | '！' | ' '));
    // (The localized table as a lowercase set, built once.)
    static LOCALIZED: std::sync::OnceLock<std::collections::HashSet<String>> = std::sync::OnceLock::new();
    let localized = LOCALIZED.get_or_init(|| ABOVE_LOCALIZED.iter().map(|x| x.to_lowercase()).collect());
    if ABOVE_WHOLE.contains(&t) || localized.contains(t) {
        return true;
    }
    if t.chars().count() <= 12 && ABOVE_CJK.iter().any(|k| t.contains(k)) {
        return true;
    }
    let words: Vec<&str> = t.split_whitespace().collect();
    if words.len() < 2 || words.len() > 8 {
        return false;
    }
    let mut i = 0;
    while i < words.len() && ALL_WORDS.contains(&words[i]) {
        i += 1;
    }
    i > 0 && i < words.len() && words[i..].iter().all(|w| ABOVE_FILLER.contains(w))
        // at least one word that points at the other choices
        && words[i..].iter().any(|w| !matches!(*w, "of" | "the" | "de" | "are" | "is" | "sind" | "son" | "sont" | "sono" | "zijn" | "är" | "z" | "di"))
}

/// Whether `text` refers to sibling choices (see the module docs; case-insensitive), judged
/// alone. A question's choices are judged together by [`refers_to_siblings`]. Only the first 200
/// bytes are read: references are short, and this keeps the check cheap on long choices.
pub fn sibling_ref(text: &str) -> Option<SiblingRef> {
    let mut cut = text.len().min(200);
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    let long = cut < text.len();
    // (Round 20, critic36a B36a-1: the reading depends on those bytes alone and a publish asks it
    // of the same choice many times: the answer is kept for the length of a publish's work scope.
    // At most 200 bytes a distinct text, so the memory is bounded by the payload.)
    if !crate::format::in_work_scope() {
        return sibling_ref_read(&text[..cut], long);
    }
    let key = (text[..cut].to_string(), long);
    if let Some(v) = SIB_REF_MEMO.with(|m| m.borrow().get(&key).copied()) {
        return v;
    }
    let v = sibling_ref_read(&text[..cut], long);
    SIB_REF_MEMO.with(|m| m.borrow_mut().insert(key, v));
    v
}

thread_local! {
    static SIB_REF_MEMO: std::cell::RefCell<std::collections::HashMap<(String, bool), Option<SiblingRef>>> = std::cell::RefCell::new(std::collections::HashMap::new());
}

fn sibling_ref_read(text: &str, long: bool) -> Option<SiblingRef> {
    let cut = text.len();
    let orig = collapse(&text[..cut]);
    let low = orig.to_lowercase();
    if !long && above_phrase(&low) {
        return Some(SiblingRef::Above);
    }
    // Maths and formulas ("V = IR", "x² + C", "C≡C", "PV = nRT", "Σ 1/n²", "NaCl → Na⁺"): letters
    // there are variables, not choices; only the phrase forms above can make them references.
    let math = orig.chars().any(|c| matches!(c, '=' | '\u{2261}' | '\u{2248}' | '\u{2260}' | '<' | '>' | '\u{2264}' | '\u{2265}' | '\u{222B}' | '\u{2211}' | '\u{221A}' | '^' | '\u{2192}' | '\u{2190}' | '\u{21CC}' | '[' | ']' | '{' | '}' | '_') || (('\u{2070}'..='\u{209F}').contains(&c)) || matches!(c, '\u{B2}' | '\u{B3}' | '\u{B9}'));
    let toks = sib_tokens(&orig);
    // (Round 19, critic35b B1: the work of a reading is charged, a quarter step a byte; each
    // token's own reading below is computed once: a 110-byte "x - x - ..." text did it for every
    // token again in each loop, 50 microseconds a choice.)
    let _ = crate::format::spend_work(cut as u64 / 4 + 1);
    // Separators that make a list: a comma or semicolon, a joiner word, or + & / and the like.
    let separated = orig.contains([',', ';', '\u{3001}', '\u{FF0C}']) || toks.len() <= 1;
    let bare = |w: &str| w.trim_matches(|c: char| !c.fast_alnum() && !splits(c) && !matches!(c, '-' | '\u{2013}' | '\u{2014}' | '~')).to_lowercase();
    let bares: Vec<String> = toks.iter().map(|t| bare(t)).collect();
    let joiners: Vec<bool> = toks.iter().zip(&bares).map(|(w, b)| joiner_set().contains(b.as_str()) || w.eq_ignore_ascii_case("and/or") || (!b.is_empty() && b.chars().count() <= 4 && b.chars().all(unspaced_script))).collect();
    let label_toks: Vec<bool> = toks.iter().map(|t| label_token(t)).collect();
    let first = bares.first().cloned().unwrap_or_default();
    let qualified = bares.iter().any(|b| qualifier_set().contains(b.as_str()));
    // A label token right after a number is a unit ("5 A", "5 V").
    let is_label = |k: usize| label_toks[k] && !(k > 0 && toks[k - 1].chars().any(|c| c.is_ascii_digit()) && !toks[k].chars().any(|c| c.is_lowercase() || c.is_ascii_digit()));
    // A bare label ("B", "(c)", "4"): only a reference where the other choices are labelled.
    if toks.len() == 1 && is_label(0) {
        return Some(SiblingRef::Weak);
    }
    // "Both", "Neither", "Both of them", "Neither of the above": a reference. "Both are
    // amphibians", "Both make gametes": content (round 10).
    if toks.len() <= 4 && matches!(first.as_str(), "both" | "neither" | "ambas" | "ambos" | "ninguna" | "ninguno" | "aucune" | "beide" | "keine" | "оба" | "обе") && toks[1..].iter().all(|t| { let b = bare(t); ABOVE_FILLER.contains(&b.as_str()) || matches!(b.as_str(), "of" | "them" | "the" | "two" | "these" | "those" | "answers" | "choices" | "options" | "others" | "de" | "ellas" | "ellos" | "deux") || label_token(t) || joiner_set().contains(b.as_str()) }) {
        return Some(SiblingRef::Labels);
    }
    if toks.len() <= 6 && POSITIONS.iter().any(|p| low.contains(p)) {
        return Some(SiblingRef::Labels);
    }
    let labels: Vec<usize> = (0..toks.len()).filter(|&k| is_label(k)).collect();
    // Labels in one list share one style ("A + B", "1 and 2"; not "2 + C").
    let digit = |k: &usize| toks[*k].chars().any(|c| c.is_ascii_digit());
    let upper = |k: &usize| toks[*k].chars().any(char::is_uppercase);
    let one_style = (labels.iter().all(digit) || !labels.iter().any(digit)) && (labels.iter().all(upper) || !labels.iter().any(upper));
    // The pronoun "I" ("I came; and I saw.") is not a label unless another numeral is there.
    if !labels.is_empty() && labels.iter().all(|&k| toks[k].trim_matches(|c: char| !c.fast_alnum()) == "I") && toks.len() > 1 {
        return None;
    }
    let strength = |ls: &[usize]| {
        let all_roman = ls.iter().all(|&k| roman_token(&toks[k]));
        let all_weak = ls.iter().all(|&k| weak_token(&toks[k]));
        if all_roman || (all_weak && !qualified) { SiblingRef::Weak } else { SiblingRef::Labels }
    };
    // Only label tokens and connectors, at least two labels: "A, C", "I, III", "A y B", "AとB", "A–C".
    if math {
        return None;
    }
    if labels.len() >= 2 && one_style && (0..toks.len()).all(|k| is_label(k) || joiners[k]) && (separated || (0..toks.len()).any(|k| !is_label(k))) {
        return Some(strength(&labels));
    }
    // A label-like token next to a joiner: "A and", "and B", "(a) and (b)", "A + B", "except C".
    // Inside a longer text a joiner must be a word ("and", "y", "und"), not a sign ("log a + log b"),
    // and one label needs a qualifier ("only C", "all except A"): "He and I went" names nobody.
    let word_joiner = |j: usize| joiners[j] && bares[j].chars().any(crate::format::is_alpha);
    if labels.len() < 2 && !qualified {
        return None;
    }
    for &k in &labels {
        let t = &toks[k];
        let prev = k.checked_sub(1);
        let next = (k + 1 < toks.len()).then_some(k + 1);
        if !(prev.is_some_and(word_joiner) || next.is_some_and(word_joiner)) {
            continue;
        }
        // A lowercase "a" may be an article ("a large and bright star"): it counts only with a
        // second label across the joiner.
        if t == "a" {
            let label_at = |j: Option<usize>| j.and_then(|j| toks.get(j)).is_some_and(|w| label_token(w) && w != "a" || w == "b");
            let after = next.is_some_and(|j| joiners[j]) && label_at(Some(k + 2));
            let before = prev.is_some_and(|j| joiners[j]) && label_at(k.checked_sub(2));
            if !(after || before) {
                continue;
            }
        }
        return Some(strength(&[k]));
    }
    None
}

/// Whether a question's choices refer to each other so that they cannot be shuffled: any
/// [`SiblingRef::Labels`]; an [`SiblingRef::Above`] that is not locked; or a [`SiblingRef::Weak`]
/// when the set says the tokens are labels:
/// * two other choices carry a label with text after it ("A. Venus", "b) 5"); or
/// * at least two other choices are words (no digits, a word of three letters or more:
///   "Mitochondria", "Ribosome"), so a "B" or "1 and 2" among them is not an answer of its kind; or
/// * the weak reference is a list ("1 and 2", "a, c") and the bare choices are exactly positions
///   1, 2, 3... ("A", "B", "C"; "1", "2", "3").
///
/// So "4, 5, 6, 7", "x, y, z", "C, H, O, N", "e, π, i", "2, 3 / 1, 2 / -1, 3" shuffle; "A, B, C,
/// D, Both A and B" and "Mitochondria, Ribosome, 1 and 2" do not.
pub fn refers_to_siblings(choices: &[(&str, bool)]) -> bool {
    // (Round 19, critic35b B1: one publish asks the same question about the same choices a dozen
    // times (the label findings, the order plan, each key scan). Inside a publish's work scope
    // the answer is kept under the SHA-256 of the exact choices, so a repeat costs one hash, and
    // no two different questions can share an entry.)
    if !crate::format::in_work_scope() {
        return refers_to_siblings_uncached(choices);
    }
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update((choices.len() as u64).to_le_bytes());
    for (t, locked) in choices {
        h.update((t.len() as u64).to_le_bytes());
        h.update(t.as_bytes());
        h.update([u8::from(*locked)]);
    }
    let key: [u8; 32] = h.finalize().into();
    if let Some(v) = SIBLING_MEMO.with(|m| m.borrow().get(&key).copied()) {
        return v;
    }
    let v = refers_to_siblings_uncached(choices);
    SIBLING_MEMO.with(|m| m.borrow_mut().insert(key, v));
    v
}

thread_local! {
    static SIBLING_MEMO: std::cell::RefCell<std::collections::BTreeMap<[u8; 32], bool>> = const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
}

/// Forgets the remembered answers (a publish's work scope closed).
pub fn clear_sibling_memo() {
    SIBLING_MEMO.with(|m| *m.borrow_mut() = std::collections::BTreeMap::new());
    SIB_REF_MEMO.with(|m| *m.borrow_mut() = std::collections::HashMap::new());
}

fn refers_to_siblings_uncached(choices: &[(&str, bool)]) -> bool {
    // (A bullet every choice starts with, "o I only" / "o I and II only", "• ...", is no label.)
    let bullet = ["o ", "\u{2022} ", "- ", "* ", "\u{25E6} ", "\u{25CF} "].into_iter().find(|b| choices.len() >= 2 && choices.iter().all(|c| c.0.trim_start().starts_with(b)));
    let stripped: Vec<(&str, bool)> = match bullet {
        Some(b) => choices.iter().map(|c| (c.0.trim_start()[b.len()..].trim_start(), c.1)).collect(),
        None => choices.to_vec(),
    };
    let choices = &stripped[..];
    let mut refs: Vec<Option<SiblingRef>> = choices.iter().map(|c| sibling_ref(c.0)).collect();
    let orig = refs.clone();
    // Round 8: a list of letters names sibling choices only when those letters are choice labels:
    // the labels the choices carry, or (unlabelled) the letters A, B, C... a renderer puts in
    // front of them. So the letters must be in range, not the choice's own, and of that style:
    // "C, E, G" (notes), "A and G" (bases), "vitamins A, D, E and K", "A, B and C" (grades, among
    // "D and E", "E and F"), "F, Cl, Br, I", "(a) and (c)" naming the stem's items with unlabelled
    // choices, "I and II only" (the stem's statements) are content.
    let n = choices.len() as u32;
    // (Labelled: two choices that are not themselves references carry a label.)
    let explicit = choices.iter().zip(&refs).filter(|(c, r)| r.is_none() && detect(c.0).is_some_and(|h| has_visible(&c.0[..h.start]) || has_visible(&c.0[h.end..]))).count() >= 2;
    let lists: Vec<Option<Readings>> = choices.iter().zip(&refs).map(|(c, r)| if r.is_some() && !matches!(r, Some(SiblingRef::Above)) { letter_list(c.0) } else { None }).collect();
    // A list of two or more letters out of range under every reading: the letters are content.
    let letter_content = lists.iter().flatten().any(|ls| ls.iter().all(|l| l.len() >= 2 && l.iter().any(|&(_, v)| v > n)));
    let fits = |ls: &Readings, k: usize| ls.iter().any(|l| l.iter().all(|&(upper_latin, v)| v >= 1 && v <= n && v != k as u32 + 1 && (explicit || upper_latin)));
    // Round 13 (critic24a B4): numbers read like letters. A choice of numbers and joining words
    // only ("1 and 3", "1 and 2 only") names choices when every number is in range and not its
    // own, and no such choice holds a number out of range ("7 and 9", "11 and 13" make them all
    // content); a number with a unit or a noun ("400 to 700 nm",
    // "pH 7 only") is content.
    // (Read without the choice's own label: "3. Both 1 and 2".)
    let numbers: Vec<Option<Vec<u32>>> = choices.iter().zip(&lists).map(|(c, l)| {
        let t = c.0;
        let t = detect(t).filter(|h| h.start == 0 && !h.suffix && has_visible(&t[h.end..])).map_or(t, |h| &t[h.end..]);
        if l.is_none() { number_list(t) } else { None }
    }).collect();
    // (Among the choices read as references: "a) 4", "c) 6" are answers, not references.)
    let number_content = numbers.iter().zip(&refs).filter(|(_, r)| matches!(r, Some(SiblingRef::Labels | SiblingRef::Weak))).filter_map(|(v, _)| v.as_ref()).any(|v| v.iter().any(|&x| x == 0 || x > n));
    let numbers_name = |k: usize| numbers[k].as_ref().is_some_and(|v| !number_content && v.iter().all(|&x| x >= 1 && x <= n && x != k as u32 + 1));
    let plain_number_words = |k: usize| lists[k].is_none() && choices[k].0.chars().any(|c| c.is_ascii_digit());
    for k in 0..choices.len() {
        if matches!(refs[k], Some(SiblingRef::Labels | SiblingRef::Weak)) && plain_number_words(k) && !numbers_name(k) {
            refs[k] = None;
            continue;
        }
        if refs[k] == Some(SiblingRef::Weak) {
            // A lowercase letter list ("a and c", "(b), (d)") with unlabelled choices names the
            // stem's items; with labelled ones, the labels in range. A capital in range ("B")
            // names a rendered label.
            if let Some(l) = &lists[k]
                && !(!letter_content && fits(l, k))
            {
                refs[k] = None;
            }
            continue;
        }
        if refs[k] != Some(SiblingRef::Labels) {
            continue;
        }
        // ("Shade 14 only", "Type A": a noun before the label tokens names content.)
        if noun_before_letters(choices[k].0) {
            refs[k] = None;
            continue;
        }
        if let Some(l) = &lists[k] {
            // ("Type A", "Types A and B equally", "Column A and C", "Hepatitis A and B": a noun names
            // the letters; "Both A and B", "Choices A and C" name choices.)
            let ok = !letter_content && fits(l, k);
            if !ok {
                refs[k] = None;
            }
        } else if choices[k].1 && (0..k).all(|j| !choices[j].1 || refs[j].is_some()) && (k + 1..choices.len()).all(|j| choices[j].1) && above_words(choices[k].0) {
            // "Both of the above" locked below every movable choice still names the ones above.
            refs[k] = Some(SiblingRef::Above);
        }
    }
    if refs.iter().zip(choices).any(|(r, c)| *r == Some(SiblingRef::Labels) || *r == Some(SiblingRef::Above) && !c.1) {
        return true;
    }
    let weak: Vec<usize> = (0..choices.len()).filter(|&k| refs[k] == Some(SiblingRef::Weak)).collect();
    if weak.is_empty() {
        return false;
    }
    let labelled = |k: usize| choices.iter().enumerate().filter(|(i, c)| *i != k && orig[*i].is_none() && detect(c.0).is_some_and(|h| has_visible(&c.0[..h.start]) || has_visible(&c.0[h.end..]))).count() >= 2;
    if weak.iter().any(|&k| labelled(k)) {
        return true;
    }
    let is_list = |k: usize| collapse(choices[k].0).split_whitespace().count() > 1 || choices[k].0.contains([',', '+', '&', '/', 'と', '和', '、']);
    let wordy = (0..choices.len())
        .filter(|&k| refs[k].is_none())
        .filter(|&k| !choices[k].0.chars().any(|c| c.is_ascii_digit() || matches!(c, '{' | '}' | '_' | '=' | '^' | '\\' | '$')) && choices[k].0.split(|c: char| !c.fast_alpha()).any(|w| w.chars().count() >= 3))
        .count()
        >= 2;
    if wordy {
        return true;
    }
    if !weak.iter().any(|&k| is_list(k)) {
        return false;
    }
    // Bare tokens at their own positions.
    let bare: Vec<(usize, Hit)> = (0..choices.len())
        .filter(|&k| refs[k] == Some(SiblingRef::Weak) && !is_list(k))
        .filter_map(|k| {
            let t = choices[k].0.trim().trim_end_matches(['.', ')', ':']).trim_start_matches('(');
            let v = letter_values(t.chars().next()?).into_iter().chain(t.parse::<u32>().ok().map(|n| (Style::Number, n))).chain(roman(&t.to_ascii_lowercase()).map(|r| (Style::Roman, r)));
            let values: Vec<(Style, u32)> = v.collect();
            (!values.is_empty()).then_some((k, Hit { start: 0, end: 0, suffix: false, values }))
        })
        .collect();
    bare.len() >= 2 && styles().iter().any(|&st| bare.iter().all(|(k, h)| value(h, st) == Some(*k as u32 + 1)))
}

/// The numbers of a choice made of numbers and joining words only ("1 and 3", "2, 4 only",
/// "Both 1 and 2"), or None (a unit, a noun, any other word).
fn number_list(t: &str) -> Option<Vec<u32>> {
    const JOIN: &[&str] = &["and", "or", "only", "both", "neither", "nor", "all", "none", "but", "not", "except", "&", "+", "et", "y", "e", "und", "oder", "ou", "o", "en", "of", "these", "items", "statements", "numbers", "choices", "choice", "options", "option", "answers", "answer", "the", "above"];
    let mut v = Vec::new();
    for w in t.split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '(' | ')' | '.')).filter(|w| !w.is_empty()) {
        if let Ok(x) = w.parse::<u32>() {
            v.push(x);
        } else if !JOIN.contains(&w.to_lowercase().as_str()) {
            return None;
        }
    }
    (!v.is_empty()).then_some(v)
}

/// Matching pairs whose right item carries the same label as its left item ("c. Paris" with
/// "France (c)"), or shares a non-letter token with it and no other right item ("France ①" /
/// "Paris ①", "- 1", "#1", a coloured emoji): it pairs them whatever the served order. Indices
/// into `pairs`.
/// Matching pairs whose right item carries the same label as its left item ("c. Paris" with
/// "France (c)"). Indices into `pairs`.
pub fn matching_label_refs(pairs: &[(&str, &str)]) -> Vec<usize> {
    pairs
        .iter()
        .enumerate()
        .filter(|(_, (l, r))| match (detect(l), detect(r)) {
            (Some(a), Some(b)) => a.values.iter().any(|v| b.values.contains(v)),
            _ => false,
        })
        .map(|(i, _)| i)
        .collect()
}

pub fn matching_key_refs(pairs: &[(&str, &str)], distractors: &[&str]) -> Vec<usize> {
    let shared: Vec<usize> = crate::format::matching_shared_tokens(pairs, distractors).into_iter().map(|x| x.0).collect();
    pairs
        .iter()
        .enumerate()
        .filter(|(i, (l, r))| {
            shared.contains(i)
                || match (detect(l), detect(r)) {
                    (Some(a), Some(b)) => a.values.iter().any(|v| b.values.contains(v)),
                    _ => false,
                }
        })
        .map(|(i, _)| i)
        .collect()
}

// ---- keys carried by formatting ---------------------------------------------------------------

/// The correct choices that formatting alone sets apart, judged on the texts given (see
/// [`crate::format`]); every feature type counts (no acknowledgement).
pub fn key_in_formatting(choices: &[(&str, bool)]) -> Vec<usize> {
    let mut v: Vec<usize> = crate::format::check(choices).into_iter().map(|f| f.index).collect();
    v.dedup();
    v
}

/// The tokens the sibling-reference tables start on (see `format::scanner_tokens`). Tests only.
#[doc(hidden)]
pub fn scanner_tokens() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = Vec::new();
    for t in [ALL_WORDS, ABOVE_FILLER, ABOVE_WHOLE, ABOVE_LOCALIZED, ABOVE_CJK, POSITIONS, JOINERS, QUALIFIERS] {
        v.extend_from_slice(t);
    }
    v
}


#[cfg(test)]
mod tests {
    use super::*;

    fn found(t: &[&str]) -> Option<Analysis> {
        analyze(t)
    }

    #[test]
    fn detection_is_broad() {
        for t in [
            vec!["A)Venus", "B)Mars", "C)Earth", "D)Jupiter"],
            vec!["A.Venus", "B.Mars", "C.Earth"],
            vec!["\u{24B6} Venus", "\u{24B7} Mars", "\u{24B8} Earth"],
            vec!["\u{2460} Venus", "\u{2461} Mars", "\u{2462} Earth"],
            vec!["Option A: Venus", "Option B: Mars", "Option C: Earth"],
            vec!["\u{3B1}) Venus", "\u{3B2}) Mars", "\u{3B3}) Earth"],
            vec!["\u{430}) Venus", "\u{431}) Mars", "\u{432}) Earth"],
            vec!["\u{661}) Venus", "\u{662}) Mars", "\u{663}) Earth"],
            vec!["A ) Venus", "B ) Mars", "C ) Earth"],
            vec!["Venus (A)", "Mars (B)", "Earth (C)"],
            vec!["B) Venus", "C) Mars", "D) Earth"],
            vec!["A\u{3001}Venus", "B\u{3001}Mars", "C\u{3001}Earth"],
            vec!["A/ Venus", "B/ Mars", "C/ Earth"],
            vec!["i) Venus", "ii) Mars", "iii) Earth"],
            vec!["A)\nVenus", "B)\nMars"],
            vec!["A)\tVenus", "B)\tMars"],
            vec!["\u{FF21}\u{FF0E}\u{6771}\u{4EAC}", "\u{FF22}\u{FF0E}\u{5927}\u{962A}"],
            vec!["\u{5D0}. \u{5E9}\u{5DC}\u{5D5}\u{5DD}", "\u{5D1}. \u{5DC}\u{5D4}\u{5EA}\u{5E8}\u{5D0}\u{5D5}\u{5EA}"],
            vec!["\u{30A2}. x", "\u{30A4}. y"],
            vec!["\u{2160}. x", "\u{2161}. y"],
        ] {
            let a = found(&t).unwrap_or_else(|| panic!("not found: {t:?}"));
            assert!(a.order_revealing, "{t:?}");
        }
    }

    #[test]
    fn strip_previews() {
        let after = |t: &[&str]| -> Vec<String> { found(t).unwrap().previews(t).into_iter().map(|p| p.after).collect() };
        assert!(found(&["A)Venus", "B)Mars", "C)Earth"]).unwrap().strip_allowed);
        assert_eq!(after(&["A)Venus", "B)Mars", "C)Earth"]), ["Venus", "Mars", "Earth"]);
        assert_eq!(after(&["Venus (A)", "Mars (B)"]), ["Venus", "Mars"]);
        assert_eq!(after(&["Option A: Venus", "Option B: Mars"])[1], "Mars");
        assert_eq!(after(&["A. (i) x", "B. (ii) y"])[1], "y", "two layers");
        // Strip refused: it would collide, empty a choice, or the labels are not 1, 2, 3...
        for t in [vec!["I. only", "II. only", "III. only"], vec!["A.", "B."], vec!["B) Venus", "C) Mars"], vec!["A. x", "B. y", "None of these"], vec!["A. x", "C. y"], vec!["A. x", "B. y", "D. z"]] {
            let a = found(&t).unwrap();
            assert!(!a.strip_allowed, "{t:?}");
            assert!(a.previews(&t).iter().all(|c| c.before == c.after), "{t:?}");
        }
        // A second layer that would merge the choices stops the removal; the first layer's
        // result ("A. x", "B. x") still numbers them, so Strip is refused outright.
        assert!(!found(&["A. A. x", "B. B. x"]).unwrap().strip_allowed);
        // The same label twice ("A. A. Einstein") is one label and an initial: Strip removes
        // nothing rather than half a name, and is refused (the first layer still numbers them).
        assert!(!found(&["A. A. x", "B. B. y"]).unwrap().strip_allowed, "same label twice");
        assert!(found(&["A. (i) x", "B. (ii) y"]).unwrap().strip_allowed, "two different clean layers");
    }

    #[test]
    fn text_is_not_mistaken_for_a_revealing_label() {
        // Round 11: genus abbreviations in no order are content, no label finding at all.
        assert!(found(&["E. coli", "S. aureus", "B. subtilis"]).is_none());
        // Not detected at all.
        for t in [vec!["A man", "B movie"], vec!["X-ray", "Y-axis"], vec!["1.5 kg", "2.5 kg"], vec!["Venus", "Mars"], vec!["U2", "R2"], vec!["e.g. x", "i.e. y"]] {
            assert!(found(&t).is_none(), "{t:?}");
        }
        // A lone label at its position is a finding; elsewhere it is not.
        assert!(found(&["A. Venus", "Mars"]).is_some());
        assert!(found(&["C. Venus", "Mars"]).is_none());
    }

    #[test]
    fn digits_of_any_script() {
        assert_eq!(digit_value('7'), Some(7));
        assert_eq!(digit_value('\u{661}'), Some(1));
        assert_eq!(digit_value('\u{967}'), Some(1));
        assert_eq!(digit_value('\u{1D7CF}'), Some(1), "mathematical bold one");
        assert_eq!(digit_value('\u{1D7E3}'), Some(1), "next block of mathematical digits");
        assert_eq!(digit_value('a'), None);
    }

    #[test]
    fn sibling_references() {
        for t in ["Both A and B", "A and C only", "Neither A nor B", "only C", "A, B and C"] {
            assert_eq!(sibling_ref(t), Some(SiblingRef::Labels), "{t}");
        }
        // Roman numerals name the stem's statements: references only where the choices are labelled.
        for t in ["I and II", "II only", "I, II and III"] {
            assert_eq!(sibling_ref(t), Some(SiblingRef::Weak), "{t}");
        }
        for t in ["All of the above", "None of these", "None of the others", "Todas las anteriores", "Toutes ces r\u{e9}ponses"] {
            assert_eq!(sibling_ref(t), Some(SiblingRef::Above), "{t}");
        }
        for t in ["Venus", "A large planet", "I think so", "Vitamin C", "Mars and Venus"] {
            assert_eq!(sibling_ref(t), None, "{t}");
        }
    }
}
