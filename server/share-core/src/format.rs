//! Keys carried by formatting: what in a choice's appearance, other than its words, could tell a
//! student which choice is correct.
//!
//! Judged on **what students are served** ([`served_text`]: edges trimmed, invisible
//! hyphenation and word-joining characters removed, labels stripped or kept as decided), with the
//! correct flags carried alongside, by comparing a closed set of **marker features** per choice
//! ([`features`]). A feature that only the correct choice(s) have (or, for marker types, the
//! absence of a marker every wrong choice shares) refuses the question with `key_in_formatting`,
//! naming the choice and the feature type (`reason`). One pass per choice, no repeated scanning.
//!
//! Content is never a marker: a choice with no letter or digit (`∞`, `→`, `!`, an emoji answer),
//! signs, charges and units next to what they qualify (`-2`, `Na⁺`, `Cl-`, `Ca2+`, `50 %`, `90°`,
//! `$5`), programming and music tokens (`C++`, `C#`, `.NET`, `len()`, `a[0]`, `F#`, `C♯`), math
//! (`|x|`, `n!`, `(a + b)²`, intervals, sets), quotes, ordinary parentheticals (`Mitosis (cell
//! division)`), sentence punctuation in sentences, acronyms of any length among other acronyms,
//! CJK text with a Latin acronym (`DNAポリメラーゼ`), any arrow or symbol between words.
//!
//! | feature (reason) | risk | what |
//! |---|---|---|
//! | `markup` | high | `**x**`, `__x__`, `_x_`, `` `x` ``, `<b>`/`<i>`/`<strong>`/`<em>`/`<u>`/`<mark>` anywhere |
//! | `star` | high | `*` attached to a word or at either end (`*Venus`, `Venus*`, `Venus ***`); not `2 * 3` |
//! | `check`, `cross` | high | ✓ ✔ ☑ ✅ / ✗ ✘ ☒ ❌ anywhere (a cross on every wrong choice, too) |
//! | `emoji` | high | a pictograph in a choice with words, when no wrong choice has one |
//! | `presentation` | high | U+FE0F / U+FE0E differing between choices |
//! | `key_word` | high | `correct`, `answer`, `ans`, `right`, `key` (and ~25 languages) as an annotation: in brackets, after a delimiter (`-`, `--`, `—`, `:`, `=`, `<-`, `<==`), as a trailing word, or `Answer:` before the text; `true`/`yes`/`this one` in brackets |
//! | `gift` | high | a leading `=` or `~` (GIFT) |
//! | `box` | high | a spaced bracket token of up to 3 characters (`[x]`, `( )`, `(*)`, `(✓)`, `(T)`, `(1)`, `(C)`) that the wrong choices lack or all share with a different value |
//! | `lead_token` | high | a leading `1`/`0`/`T`/`F`/`+`/`-` token before words, every wrong choice sharing another |
//! | `edge` | high / low | marker symbols at either end: arrows (`<-`, `->`, `←`), `#`, `+`/`-` before a capitalised word or apart, `=`, `~`, `>`, `!!`, bullets, `»«`, `§`... (high: arrows, `#`, `=`, `~`, `!!`, `+`, `-`, `>`, `<`) |
//! | `mixed_script` | high | a word mixing Latin and Cyrillic letters (homoglyphs; Greek and whole-word lookalikes are not detected) |
//! | `enclosing` | high | an enclosing combining mark (U+20DD-20E4, not the keycap) |
//! | `terminal` | low | a one-word choice ending in `.` or `!` (`Venus.`); sentences are content |
//! | `capitals` | low | an all-capital ASCII word of 5+ letters when every wrong choice is mixed-case words without an acronym |
//! | `initial_capital` | low | a capitalised one-word choice among lowercase one-word choices |
//! | `fullwidth` | low | fullwidth forms (U+FF01-FF5E) only on the correct choice |
//! | `invisible` | low | direction marks and invisible operators that differ (inside or at the ends) |
//!
//! High-risk features always refuse; low-risk ones refuse unless the author acknowledges the
//! question (`ack_formatting`: "this is part of the content"). A wrong choice with a feature the
//! correct one lacks is fine, except the marker types marked "absence" (`cross`, `gift`, `box`,
//! `lead_token`, high-risk `edge`) shared by at least two wrong choices.
use serde::Serialize;
use unicode_general_category::{GeneralCategory as G, get_general_category};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    Low,
    High,
}

/// One marker feature of one choice.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Feature {
    pub kind: &'static str,
    pub value: String,
    pub risk: Risk,
}

/// A flagged choice: its index, the feature type and its risk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Flag {
    pub index: usize,
    pub reason: &'static str,
    pub risk: Risk,
}

/// Letters of scripts that join with ZWJ / ZWNJ (Arabic, Syriac, Indic, Sinhala, Myanmar,
/// Khmer, Mongolian...): there the joiners change how a word is written, so they are kept.
fn joining_script(c: char) -> bool {
    matches!(c as u32, 0x0600..=0x08FF | 0x0900..=0x0DFF | 0x0F00..=0x0FFF | 0x1000..=0x109F | 0x1780..=0x18AF | 0xA8E0..=0xA8FF | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF | 0x10A00..=0x10A5F | 0x11000..=0x111FF)
}
/// Arabic-script letters that never join to the left (alef, dal, reh, waw...): a ZWNJ after one
/// changes nothing and can only mark.
fn right_joining_only(c: char) -> bool {
    matches!(c as u32, 0x0621..=0x0625 | 0x0627 | 0x0629 | 0x062F..=0x0632 | 0x0648 | 0x0671..=0x0673 | 0x0675..=0x0677 | 0x0688..=0x0699 | 0x06C0 | 0x06C3..=0x06CB | 0x06CD | 0x06CF | 0x06D2 | 0x06D3 | 0x06D5 | 0x06EE | 0x06EF | 0xFE80..=0xFE8E | 0xFEA9..=0xFEB0 | 0xFEED | 0xFEEE)
}
/// A word of two letters or more that is not a joiner ("and", "only"): "A and O" is a list.
fn content_word(t: &str) -> bool {
    let w: String = t.chars().filter(|c| c.fast_alpha()).flat_map(|c| c.to_lowercase()).collect();
    w.chars().count() >= 2 && !matches!(w.as_str(), "and" | "or" | "only" | "und" | "oder" | "et" | "ou" | "y" | "o" | "e" | "nur" | "solo" | "seulement" | "both" | "not" | "but" | "nor" | "plus")
}

/// The trailing annotation of a choice: the text after a delimiter (" - ", " – ", " — ", " <- ",
/// " ← ", " = ") or inside spaced brackets at its end, of at most six words, with content before;
/// or (flag true) the tail after a copula, " is correct", " is the correct answer". None for
/// none.
fn trailing_annotation(text: &str) -> Option<(&str, bool)> {
    let t = text.trim();
    if t.len() > 400 {
        return None;
    }
    let content_before = |i: usize| t[..i].chars().any(crate::format::is_alnum);
    let short = |a: &str| (1..=6).contains(&a.split_whitespace().count()) && a.chars().any(crate::format::is_alpha);
    // Spaced brackets at the end.
    if let Some(z) = t.chars().last()
        && matches!(z, ')' | ']' | '\u{FF09}')
    {
        let open = match z { ')' => '(', ']' => '[', _ => '\u{FF08}' };
        if let Some(o) = t.rfind(open)
            && o > 0
            && t[..o].ends_with(char::is_whitespace)
            && content_before(o)
        {
            let inner = &t[o + open.len_utf8()..t.len() - z.len_utf8()];
            if short(inner) || inner.chars().any(|c| c.is_ascii_digit()) && inner.contains('%') {
                return Some((inner, false));
            }
        }
    }
    // After a delimiter.
    for d in [" - ", " \u{2013} ", " \u{2014} ", " <- ", " \u{2190} ", " <= ", " = ", " -- "] {
        if let Some(i) = t.rfind(d)
            && content_before(i)
        {
            let a = &t[i + d.len()..];
            if short(a) {
                return Some((a, false));
            }
        }
    }
    // A copula tail: "Leonardo da Vinci is correct", "... is the correct answer".
    // (ASCII lowercase only: the same byte offsets as `t`. "İ" lowercases to three bytes and "ẞ"
    // to two, and an offset found in a full lowercase copy cut `t` inside a character: a wasm
    // trap, round 14 critic26b.)
    let low = t.to_ascii_lowercase();
    for c in [" is ", " ist ", " est ", " es ", " \u{E8} ", " \u{E9} ", " is the "] {
        if let Some(i) = low.rfind(c)
            && content_before(i)
        {
            let a = &t[i + c.len()..];
            if (1..=4).contains(&a.split_whitespace().count()) {
                return Some((a, true));
            }
        }
    }
    None
}

fn wrong_set() -> &'static std::collections::HashSet<String> {
    use std::sync::OnceLock;
    static S: OnceLock<std::collections::HashSet<String>> = OnceLock::new();
    S.get_or_init(|| WRONG_WORDS.iter().map(|w| fold_key(w)).collect())
}
fn wrong_bracket_set() -> &'static std::collections::HashSet<String> {
    use std::sync::OnceLock;
    static S: OnceLock<std::collections::HashSet<String>> = OnceLock::new();
    S.get_or_init(|| WRONG_BRACKET.iter().map(|w| fold_key(w)).collect())
}

/// Whether a choice opens with an answer or verdict lead and a separator ("Correct answer: Mars",
/// "Right answer - Mars", "resp. correcta: ...", "正确答案:", "정답:", "الإجابة:"): at most three
/// words before the separator, one of them a lead or key word (or a CJK lead inside).
fn answer_prefix(text: &str) -> Option<&'static str> {
    // A verdict line of its own before the content: "Correct!\nDovetail joint", "Correct
    // Answer\n...", "Correcta\n..." (a pasted review page, round 12).
    if let Some((first, rest)) = text.split_once('\n')
        && first.chars().count() <= 24
        && rest.chars().any(crate::format::is_alnum)
    {
        let f = fold_key(first.trim().trim_end_matches(['!', '.', ':', ' ']));
        let words: Vec<&str> = f.split(' ').filter(|w| !w.is_empty()).collect();
        let verdict = |w: &str| folded_set(0).contains(w) || folded_set(6).contains(w) || (folded_set(4).contains(w) && w.chars().count() >= 4);
        if (1..=3).contains(&words.len()) && words.iter().all(|w| verdict(w) || matches!(*w, "your" | "the" | "you" | "answered" | "is")) && words.iter().any(|w| verdict(w)) {
            return Some("answer_prefix");
        }
    }
    // Round 15 (critic28a): a verdict before the content with no separator, "Correct! Mars",
    // "Correct Answer Mars", or a praise after it, "Mars OK", "Mars Well done!".
    {
        let f = fold_key(&text.chars().take(80).collect::<String>());
        let words: Vec<&str> = f.split(' ').filter(|w| !w.is_empty()).collect();
        let verdict = |w: &str| matches!(w.trim_end_matches('!'), "correct" | "right" | "correcto" | "correcta" | "richtig" | "juste" | "corretto" | "certo" | "correta" | "bravo");
        if words.len() >= 2 {
            let w0 = words[0];
            let bang = w0.ends_with('!') && verdict(w0);
            let phrase = verdict(w0) && matches!(words[1], "answer" | "answer!" | "answer:" | "choice" | "option");
            if (bang || phrase) && words.len() > if phrase { 2 } else { 1 } {
                return Some("answer_prefix");
            }
        }
        let tail = f.trim_end_matches(['!', '.', ' ']);
        for p in [" ok", " okay", " well done", " bravo", " excellent", " good job", " nice one", " correct"] {
            if let Some(h) = tail.strip_suffix(p)
                && h.chars().any(crate::format::is_alnum)
                && (f.trim_end().ends_with('!') || matches!(p, " ok" | " okay" | " well done" | " good job" | " nice one"))
            {
                return Some("answer_prefix");
            }
        }
    }
    let head: String = text.chars().take(40).collect();
    let f = fold_key(&head);
    let sep = f.find([':', '-', '=', '>', '\u{FF1A}'])?;
    let before = f[..sep].trim();
    let after = f[sep..].trim_start_matches([':', '-', '=', '>', ' ', '.']);
    if before.is_empty() || !after.chars().any(crate::format::is_alnum) {
        return None;
    }
    let words: Vec<&str> = before.split(|c: char| !c.fast_alnum()).filter(|w| !w.is_empty()).collect();
    if words.is_empty() || words.len() > 3 {
        return None;
    }
    // (Words that are content as often as leads, "Solution: a mixture", "key: value", count only
    // with a qualifier.)
    let ambiguous = |w: &str| matches!(w, "key" | "right" | "correct" | "ok" | "solution" | "solutions" | "sol" | "resp" | "clave" | "kunci" | "feedback" | "losung" | "lösung" | "solucion" | "solución" | "soluzione" | "solucao" | "solução" | "rozwiazanie" | "ratkaisu" | "megoldas" | "megoldás" | "resenie" | "riesenie" | "solutie" | "cozum" | "çözüm" | "oplossing" | "løsning" | "losning" | "lösning" | "hal");
    let lead = words.iter().any(|w| (folded_set(0).contains(*w) || folded_set(6).contains(*w)) && !ambiguous(w)) || folded_list(1).iter().any(|l| before.contains(l.as_str()));
    // ("Correct" or "Right" alone before a colon is often content: "Right: the angle..."; with
    // "answer" it is a lead.)
    let pair = words.len() >= 2 && words.iter().any(|w| matches!(*w, "correct" | "right" | "richtige" | "bonne" | "correcta" | "corretta" | "correta" | "juiste" | "poprawna")) && words.iter().any(|w| folded_set(0).contains(*w));
    (lead || pair).then_some("answer_prefix")
}

/// A combining mark inside a word (Devanagari virama, Bengali vowel signs, Arabic harakat).
fn is_word_mark(c: char) -> bool {
    !c.is_ascii() && matches!(get_general_category(c), G::NonspacingMark | G::SpacingMark)
}

fn nfc(s: &str) -> String {
    if unicode_normalization::is_nfc_quick(s.chars()) == unicode_normalization::IsNormalized::Yes { s.to_string() } else { unicode_normalization::UnicodeNormalization::nfc(s).collect() }
}

/// Unicode Default_Ignorable_Code_Point (and the format characters that render as nothing): the
/// ranges, inclusive. A key word with one of them inside ("Pa\u{200E}ris") reads as "Paris" to a
/// student, so every key and hint scan folds them out of its copy of the text (the served text is
/// untouched). Round 21 (critic37b B1): only some of them were folded, the rest (LRM, RLM, ALM, the
/// invisible math operators, the Mongolian vowel separator) defeated every scan. The canary test
/// lists every format character and fails for one that is in neither this table nor
/// `VISIBLE_FORMAT`.
pub const DEFAULT_IGNORABLE: &[(u32, u32)] = &[
    (0x00AD, 0x00AD),
    (0x034F, 0x034F),
    (0x061C, 0x061C),
    (0x115F, 0x1160),
    (0x17B4, 0x17B5),
    (0x180B, 0x180F),
    (0x200B, 0x200F),
    (0x202A, 0x202E),
    (0x2060, 0x206F),
    (0x3164, 0x3164),
    (0xFE00, 0xFE0F),
    (0xFEFF, 0xFEFF),
    (0xFFA0, 0xFFA0),
    (0xFFF0, 0xFFF8),
    (0x1BCA0, 0x1BCA3),
    (0x1D173, 0x1D17A),
    (0xE0000, 0xE0FFF),
];

/// Format (Cf) characters that DO show something (number signs, a sign above, a mark of the
/// Syriac abbreviation, interlinear annotation anchors, musical and shorthand marks that draw):
/// kept in a folded text.
pub const VISIBLE_FORMAT: &[(u32, u32)] = &[
    (0x0600, 0x0605),
    (0x06DD, 0x06DD),
    (0x070F, 0x070F),
    (0x0890, 0x0891),
    (0x08E2, 0x08E2),
    (0x110BD, 0x110BD),
    (0x110CD, 0x110CD),
    (0xFFF9, 0xFFFB),
    (0x13430, 0x1343F),
];

/// Whether a character is invisible and ignorable in a key scan.
pub fn is_default_ignorable(c: char) -> bool {
    let u = c as u32;
    if u < 0xAD {
        return false;
    }
    DEFAULT_IGNORABLE.iter().any(|&(a, b)| u >= a && u <= b)
}

/// One round of `served_text`'s removals: soft hyphen, ZWSP, WJ, CGJ, BOM, and ZWJ / ZWNJ outside
/// joining scripts and emoji sequences. Whether anything was removed.
fn drop_invisible(s: &str) -> (String, bool) {
    let src: Vec<char> = s.chars().collect();
    let joiner = |c: char| matches!(c, '\u{200C}' | '\u{200D}');
    // The nearest non-joiner after each position, in one backward pass (runs of joiners stay linear).
    let mut next_of: Vec<Option<char>> = vec![None; src.len() + 1];
    for k in (0..src.len()).rev() {
        next_of[k] = if joiner(src[k]) { next_of[k + 1] } else { Some(src[k]) };
    }
    let mut prev: Option<char> = None;
    let mut t = String::with_capacity(s.len());
    let mut changed = false;
    for (k, &c) in src.iter().enumerate() {
        if !joiner(c) && !matches!(c, '\u{AD}' | '\u{200B}' | '\u{2060}' | '\u{034F}' | '\u{FEFF}') {
            prev = Some(c);
        }
        match c {
            '\u{AD}' | '\u{200B}' | '\u{2060}' | '\u{034F}' | '\u{FEFF}' => {
                changed = true;
                continue;
            }
            '\u{200C}' | '\u{200D}' => {
                let next = next_of[k + 1];
                let keep = match (prev, next) {
                    (Some(a), Some(b)) => (joining_script(a) && joining_script(b) && !(c == '\u{200C}' && right_joining_only(a))) || (c == '\u{200D}' && emoji_part(a) && emoji_part(b)),
                    _ => false,
                };
                if !keep {
                    changed = true;
                    continue;
                }
            }
            _ => {}
        }
        t.push(c);
    }
    (t, changed)
}

/// Emoji parts a ZWJ joins ("👨‍👩‍👧", "🏳️‍🌈", "👩‍⚕️").
fn emoji_part(c: char) -> bool {
    let u = c as u32;
    matches!(u, 0x1F000..=0x1FAFF | 0x2600..=0x27BF | 0x2B00..=0x2BFF | 0xFE0F)
}

/// What students are served of a choice: whitespace trimmed at both ends, runs of whitespace
/// between words one space (en spaces, NBSP and tabs included; indentation at the start of a line
/// kept, for code), soft hyphens, zero-width spaces, word joiners and the combining grapheme
/// joiner removed, ZWJ / ZWNJ removed except between letters of a script that needs them or
/// inside an emoji sequence, NFC (removing a joiner can bring a mark next to a new base). They
/// change nothing a reader should see, and can only mark. `hash_form` applies the same, so hashes
/// of the author's form and of the served form agree.
pub fn served_text(s: &str) -> String {
    // NFC first, then the invisible characters judged on the NFC text, then NFC again: removing a
    // character can bring a mark next to a new base ("ا͏ٓ" → "آ"), which changes the joiners'
    // neighbours. Repeated until nothing changes, so served_text(served_text(x)) == served_text(x).
    // (Each round removes at least one character: at most a few rounds, usually one.)
    // (ASCII has nothing to normalise or remove: straight to the spaces.)
    let mut t = if s.is_ascii() { s.to_string() } else { nfc(s) };
    while !t.is_ascii() {
        let (u, changed) = drop_invisible(&t);
        if !changed {
            break;
        }
        t = nfc(&u);
    }
    let mut out = String::with_capacity(t.len());
    for (n, line) in t.trim().split('\n').enumerate() {
        if n > 0 {
            out.push('\n');
        }
        let body = line.trim_start_matches([' ', '\t']);
        out.push_str(&line[..line.len() - body.len()]);
        let mut space = false;
        for c in body.trim_end().chars() {
            if c.is_whitespace() {
                space = true;
                continue;
            }
            if space {
                out.push(' ');
                space = false;
            }
            out.push(c);
        }
    }
    out
}

fn is_check(c: char) -> bool {
    matches!(c, '\u{2713}' | '\u{2714}' | '\u{2611}' | '\u{2705}' | '\u{1F5F8}')
}
fn is_cross(c: char) -> bool {
    matches!(c, '\u{2717}' | '\u{2718}' | '\u{2612}' | '\u{274C}' | '\u{274E}' | '\u{2715}' | '\u{2716}')
}
fn is_star(c: char) -> bool {
    matches!(c, '\u{2605}' | '\u{2606}' | '\u{2B50}' | '\u{2731}' | '\u{2733}' | '\u{2734}' | '\u{2747}' | '\u{203B}' | '\u{1F31F}' | '\u{2730}' | '\u{272A}'..='\u{272F}' | '\u{2729}')
}
/// Asterisk look-alikes used like '*' ("Paris∗", "Paris⁎", "＊Paris").
fn is_asterisk(c: char) -> bool {
    matches!(c, '*' | '\u{2217}' | '\u{204E}' | '\u{FE61}' | '\u{FF0A}' | '\u{2055}')
}
/// Marks teachers use for "this one": circles, pointers, hearts, ticks as roots.
fn mark_class(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{25CB}' | '\u{25CE}' | '\u{25CF}' | '\u{25EF}' | '\u{25C9}' | '\u{3007}' | '\u{2B55}' | '\u{2299}' => "circle",
        '\u{2726}' | '\u{2727}' | '\u{2756}' | '\u{203C}' | '\u{2757}' | '\u{2755}' | '\u{1F197}' | '\u{1F44C}' | '\u{1F4AF}' => "star",
        '\u{25C0}' | '\u{25C4}' | '\u{25B6}' | '\u{25BA}' | '\u{261A}'..='\u{261F}' | '\u{1F448}' | '\u{1F449}' => "pointer",
        '\u{2665}' | '\u{2661}' | '\u{2764}' | '\u{2763}' | '\u{2666}' | '\u{2662}' => "heart",
        _ => return None,
    })
}
/// Mathematical alphanumeric letters (𝐏𝐚𝐫𝐢𝐬, 𝑽𝒆𝒏𝒖𝒔).
fn is_math_letter(c: char) -> bool {
    matches!(c as u32, 0x1D400..=0x1D7FF)
}
/// Enclosed alphanumerics (Ⓟⓐⓡⓘⓢ, Ⓒ, 🄲, ①).
fn is_enclosed(c: char) -> bool {
    matches!(c as u32, 0x2460..=0x24FF | 0x1F100..=0x1F1FF | 0x3248..=0x32FF)
}
/// Small capitals and modifier (superscript) letters (ᴘᴀʀɪꜱ, ᵃ, ᶜ); not ʻokina, apostrophes, stress
/// marks, or the ⁿ and ⁱ of mathematics.
fn is_modifier_letter(c: char) -> bool {
    let u = c as u32;
    (matches!(u, 0x1D00..=0x1DBF | 0xA730 | 0xA731 | 0x2090..=0x209C | 0xAA | 0xBA) || (get_general_category(c) == G::ModifierLetter && !matches!(u, 0x02BB | 0x02BC | 0x02C8 | 0x02CC | 0x02B9 | 0x02BA | 0x207F | 0x2071 | 0x3005 | 0x309D | 0x309E | 0x30FC..=0x30FE | 0xFF70 | 0x0640)))
        && !matches!(u, 0x207F | 0x2071)
}
/// Combining marks that draw a line through or under a letter (strike, underline, overline).
fn is_line_mark(c: char) -> bool {
    matches!(c as u32, 0x0305 | 0x0332 | 0x0333 | 0x0335..=0x0338 | 0x033F | 0x20D2 | 0x20E5 | 0x0334)
}
/// Greek letters that look like Latin ones.
fn greek_lookalike(c: char) -> bool {
    matches!(c, 'α' | 'ο' | 'ν' | 'ρ' | 'ε' | 'ι' | 'κ' | 'τ' | 'υ' | 'χ' | 'Α' | 'Β' | 'Ε' | 'Ζ' | 'Η' | 'Ι' | 'Κ' | 'Μ' | 'Ν' | 'Ο' | 'Ρ' | 'Τ' | 'Υ' | 'Χ')
}
/// Letters that look like Latin ones but come from another script or a symbol block: Cyrillic,
/// Armenian, Cherokee, Lisu, Coptic, Number Forms (Roman numerals), Letterlike Symbols, fullwidth
/// Latin, small capitals, Latin Extended-D.
fn confusable_high(c: char) -> bool {
    matches!(c as u32, 0x0400..=0x052F | 0x0530..=0x058F | 0x13A0..=0x13FF | 0xAB70..=0xABBF | 0xA4D0..=0xA4FF | 0x2C80..=0x2CFF | 0x2160..=0x2188 | 0x2100..=0x214F | 0xFF21..=0xFF3A | 0xFF41..=0xFF5A | 0x1D00..=0x1D2B | 0xA720..=0xA7FF | 0x10A0..=0x10FF)
}
/// Latin letters of real orthographies and IPA that look like plain ones: dotless i, long s,
/// script a and g, clicks, IPA extensions.
fn confusable_low(c: char) -> bool {
    matches!(c as u32, 0x0131 | 0x017F | 0x01C0..=0x01C3 | 0x0250..=0x02AF)
}

/// A symbol that is content wherever it is (units, music, legal signs).
fn content_symbol(c: char) -> bool {
    matches!(c, '\u{B0}' | '\u{2103}' | '\u{2109}' | '\u{2030}' | '\u{2031}' | '\u{2113}' | '\u{2116}' | '\u{A9}' | '\u{AE}' | '\u{2122}' | '\u{266D}' | '\u{266E}' | '\u{266F}' | '\u{2126}' | '\u{212B}' | '\u{A6}' | '\u{A7}')
}
fn is_emoji(c: char) -> bool {
    let u = c as u32;
    matches!(u, 0x1F300..=0x1F5FF | 0x1F600..=0x1F64F | 0x1F680..=0x1F6FF | 0x1F7E0..=0x1F7EB | 0x1F900..=0x1F9FF | 0x1FA70..=0x1FAFF | 0x2B55 | 0x2764 | 0x1F004 | 0x1F0CF)
}
fn is_arrow(c: char) -> bool {
    matches!(c as u32, 0x2190..=0x21FF | 0x27F5..=0x27FF | 0x2B05..=0x2B07 | 0x27A1 | 0x279C | 0x2794 | 0x261C | 0x261E | 0x1F448 | 0x1F449)
}
fn is_quote(c: char) -> bool {
    matches!(c, '"' | '\'' | '\u{201C}' | '\u{201D}' | '\u{2018}' | '\u{2019}' | '\u{201E}' | '\u{201A}' | '\u{300C}' | '\u{300D}' | '\u{300E}' | '\u{300F}')
}
pub fn is_alnum(c: char) -> bool {
    c.is_ascii_alphanumeric() || (!c.is_ascii() && class_of(c) & 2 != 0)
}
/// Letter (1) and letter-or-digit (2) bits of every character of the Basic Multilingual Plane,
/// built once: the standard tables are a binary search per call, round 17 (critic31b, Malayalam
/// and Kannada texts spent most of their time there). Others ask the standard library.
fn class_of(c: char) -> u8 {
    static T: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    let u = c as u32;
    if u >= 0x10000 {
        return u8::from(c.is_alphabetic()) | (u8::from(c.is_alphanumeric()) << 1);
    }
    let t = T.get_or_init(|| {
        let mut v = vec![0u8; 0x10000 / 4];
        for u in 0..0x10000u32 {
            if let Some(c) = char::from_u32(u) {
                let bits = u8::from(c.is_alphabetic()) | (u8::from(c.is_alphanumeric()) << 1);
                v[(u / 4) as usize] |= bits << ((u % 4) * 2);
            }
        }
        v
    });
    (t[(u / 4) as usize] >> ((u % 4) * 2)) & 3
}
/// `char::is_alphabetic` and `char::is_alphanumeric` by `class_of`.
pub trait FastClass {
    fn fast_alpha(self) -> bool;
    fn fast_alnum(self) -> bool;
}
impl FastClass for char {
    #[inline]
    fn fast_alpha(self) -> bool {
        is_alpha(self)
    }
    #[inline]
    fn fast_alnum(self) -> bool {
        is_alnum(self)
    }
}
/// `crate::format::is_alpha` with the large syllabaries and ideographs answered by range: the
/// standard table search is slow for them (round 12: a Korean bank spent half its time there).
pub fn is_alpha(c: char) -> bool {
    c.is_ascii_alphabetic() || (!c.is_ascii() && class_of(c) & 1 != 0)
}
fn is_super_sub(c: char) -> bool {
    matches!(c as u32, 0x00B2 | 0x00B3 | 0x00B9 | 0x2070..=0x209F)
}

/// Annotation words that say "this is the key", any case, in many languages.
const KEY_WORDS: &[&str] = &[
    "correct", "right", "answer", "ans", "richtig", "korrekt", "antwort", "lösung", "correcto", "correcta", "respuesta", "correcte",
    "réponse", "reponse", "juste", "corretto", "corretta", "risposta", "giusto", "giusta", "certo", "certa", "correta", "resposta",
    "правильно", "правильный", "правильная", "верно", "ответ", "doğru", "cevap", "benar", "jawaban", "đúng", "juist", "goed", "antwoord",
    "poprawna", "poprawny", "odpowiedź", "rätt", "svar", "riktig", "oikein", "vastaus", "bonne", "corr", "corect", "helyes", "správně",
    "správná", "správný", "σωστό", "σωστή", "σωστός", "صحيح", "صح", "נכון", "सही", "正しい", "맞음", "tačno", "tacno", "točno", "rigtig",
    "oikea", "betul", "tama", "درست", "ถูก", "вірно", "вярно", "правилно", "rett", "teisingai", "pareizi", "rétt", "zuzen", "cert",
    "corectă", "corect", "doğrudur", "chính", "xác", "tepat", "igaz", "jó", "sahi", "lösningen",
    // Round 8: more languages (Swahili, Polish, Danish, Bengali, Tamil, Hindi, Urdu, Persian,
    // Thai, Indonesian, Tagalog, Hausa, Yoruba, Zulu, Amharic, Georgian, Armenian, Ukrainian,
    // Serbian, Croatian, Slovenian, Slovak, Lithuanian, Latvian, Estonian, Basque, Catalan,
    // Galician, Welsh, Irish, Maltese, Albanian, Macedonian, Kazakh, Uzbek, Azerbaijani, Mongolian,
    // Nepali, Sinhala, Khmer, Lao, Burmese, Telugu, Kannada, Malayalam, Marathi, Gujarati, Punjabi).
    "sahihi", "jibu", "dobrze", "esatta", "esatto", "gabarito", "옳음", "정답임", "נכונה", "גערעכט", "ריכטיק", "صحيحة", "правильный", "правилен", "ትክክለኛ", "prawidłowa", "prawidłowo", "rigtigt", "সঠিক", "উত্তর", "சரி", "விடை", "उत्तर", "صحیح",
    "جواب", "پاسخ", "คำตอบ", "tama", "wasto", "daidai", "ìdáhùn", "okulungile", "ትክክል", "სწორი", "ճիշտ", "правильна", "відповідь",
    "тачно", "одговор", "pravilno", "odgovor", "správne", "teisingas", "atsakymas", "pareizs", "atbilde", "õige", "vastus", "erantzuna",
    "cywir", "ateb", "ceart", "freagra", "korrett", "tweġiba", "saktë", "përgjigje", "точно", "дұрыс", "жауап",
    "to'g'ri", "javob", "düzgün", "cavab", "зөв", "хариулт", "ठीक", "නිවැරදි", "ត្រឹមត្រូវ", "ຖືກ", "မှန်", "సరైన", "ಸರಿ", "ശരി", "बरोबर",
    "સાચું", "ਸਹੀ",
    // Round 15 (critic28a): Basque, Yoruba, Zulu, Xhosa, Swahili.
    "zuzena", "òtítọ́", "otito", "kulungile", "ichanekile", "iqiniso", "kweli", "inyaniso",
];
/// Only inside brackets: words that are ordinary content elsewhere.
/// Key words that may end a choice as a bare annotation ("Venus correct"): ones that rarely end
/// real content.
const TRAILING_WORDS: &[&str] = &["correct", "answer", "ans", "korrekt", "richtig", "correcto", "correcta", "correcte", "corretto", "corretta", "correta", "правильно", "верно", "juist", "poprawna", "rätt", "oikein"];
const BRACKET_WORDS: &[&str] = &["key", "kunci", "đáp", "án", "الإجابة", "الصحيحة", "jawaban", "benar", "sol", "soln", "solution", "solución", "solução", "soluzione", "rozwiązanie", "ratkaisu", "megoldás", "řešení", "ndiyo", "ndio", "evet", "hai", "haan", "हाँ", "ঠিক", "ஆம்", "true", "yes", "ok", "this", "ja", "sí", "si", "oui", "да", "yep", "one", "wahr", "sim", "evet", "tak", "igen", "ano", "kyllä", "ναι", "نعم", "כן", "हाँ", "はい", "네", "예", "对", "是"];
const CJK_KEY_WORDS: &[&str] = &["正解", "正确", "答案", "正答", "정답", "正確", "答え", "解答", "こたえ", "対", "對", "正确答案", "참", "맞다", "ถูกต้อง"];
/// The same for "this is wrong" annotations (on every wrong choice, they mark the correct one by
/// their absence).
const WRONG_WORDS: &[&str] = &["distractor", "decoy", "unused", "wrong", "incorrect", "incorreta", "incorreto", "errata", "틀림", "오답임", "falsch", "fehlerhaft", "שגוי", "שגויה", "λάθος", "غلط", "ভুল", "தவறு", "ผิด", "anghywir", "makosa", "błędnie", "źle", "forkert", "ভুল", "தவறு", "غلط", "نادرست", "ผิด", "неправильна", "netačno", "netočno", "nepravilno", "nesprávne", "neteisingai", "nepareizi", "anghywir", "mícheart", "ħażin", "gabim", "қате", "noto'g'ri", "буруу", "ভূল", "తప్పు", "ತಪ್ಪು", "തെറ്റ്", "चूक", "ખોટું", "ਗਲਤ", "falsch", "false", "incorrecto", "incorrecta", "faux", "fausse", "sbagliato", "errato", "errado", "errada", "неверно", "неправильно", "fout", "błędna", "fel", "yanlış", "salah", "sai", "rossz", "špatně", "λάθος", "خطأ", "לא", "गलत", "不正解", "错误", "오답", "falso", "falsa"];
const WRONG_BRACKET: &[&str] = &["no", "nein", "non", "нет", "nee", "nie", "hayır", "tidak", "nem", "ne", "όχι", "hapana", "fehler", "טעות", "خطا", "اشتباه", "ስህተት", "erreur", "errore", "erro", "error", "virhe", "hiba", "chyba", "greška", "napaka", "klaida", "kļūda", "viga", "помилка", "ошибка", "грешка", "kosa", "earráid", "nahi", "nahin", "नहीं", "না", "இல்லை"];
const WRONG_CJK: &[&str] = &["不正解", "错误", "錯誤", "오답", "誤り", "不对", "不對", "错", "ผิด"];

/// The key and bracket word tables of `KEY` (0) or `WRONG` (1) as sets, built once.
fn word_sets(which: usize) -> (&'static std::collections::HashSet<&'static str>, &'static std::collections::HashSet<&'static str>) {
    use std::collections::HashSet;
    use std::sync::OnceLock;
    static SETS: OnceLock<[HashSet<&'static str>; 4]> = OnceLock::new();
    let s = SETS.get_or_init(|| [KEY.key.iter().copied().collect(), KEY.bracket.iter().copied().collect(), WRONG.key.iter().copied().collect(), WRONG.bracket.iter().copied().collect()]);
    (&s[2 * which], &s[2 * which + 1])
}

/// Word tables for one kind of annotation.
struct Words {
    key: &'static [&'static str],
    trailing: &'static [&'static str],
    bracket: &'static [&'static str],
    cjk: &'static [&'static str],
}
const KEY: Words = Words { key: KEY_WORDS, trailing: TRAILING_WORDS, bracket: BRACKET_WORDS, cjk: CJK_KEY_WORDS };
const WRONG: Words = Words { key: WRONG_WORDS, trailing: WRONG_WORDS, bracket: WRONG_BRACKET, cjk: WRONG_CJK };
const COPULAS: &[&str] = &["is", "are", "was", "were", "be", "not", "and", "or", "very", "most", "least", "more", "less", "seems", "looks", "sounds", "grammatically", "politically", "factually", "ist", "es", "est", "è", "sind", "être"];

/// The key-word annotation features (see the module table): (key word, wrong word). One pass over
/// the text; only words in an annotation position are compared (in brackets, after a delimiter,
/// first before a colon, last), so a long choice costs one scan.
fn key_words(chars: &[char]) -> (bool, bool) {
    // Words: (start, end, bracket depth at start); the outermost bracket group of each word, and
    // whether a group holds a digit.
    let mut words: Vec<(usize, usize, i32)> = Vec::new();
    let mut group_of: Vec<Option<usize>> = Vec::new();
    let mut group_digit: Vec<bool> = Vec::new();
    let mut group_first: Vec<usize> = Vec::new();
    // (Braces are not annotation brackets: "$\frac{1}{x}$", "\bar{x}" are maths.)
    let mut d = 0i32;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.fast_alpha() {
            let s = i;
            // (A word goes on through its combining marks: viramas and vowel signs, "उत्तर".)
            while i < chars.len() && (chars[i].fast_alpha() || is_word_mark(chars[i])) {
                i += 1;
            }
            if d > 0 && group_first.last() == Some(&usize::MAX) {
                *group_first.last_mut().unwrap() = words.len();
            }
            words.push((s, i, d));
            group_of.push((d > 0).then(|| group_digit.len() - 1));
            continue;
        }
        match c {
            '(' | '[' | '\u{FF08}' | '\u{FF3B}' | '\u{3010}' | '\u{3014}' | '\u{300C}' | '\u{300E}' => {
                if d == 0 {
                    group_digit.push(false);
                    group_first.push(usize::MAX);
                }
                d += 1;
            }
            ')' | ']' | '\u{FF09}' | '\u{FF3D}' | '\u{3011}' | '\u{3015}' | '\u{300D}' | '\u{300F}' => d = (d - 1).max(0),
            c if d > 0 && c.is_ascii_digit() => {
                if let Some(g) = group_digit.last_mut() {
                    *g = true;
                }
            }
            _ => {}
        }
        i += 1;
    }
    let lower = |k: usize| -> String { chars[words[k].0..words[k].1].iter().flat_map(|c| c.to_lowercase()).collect() };
    let first_alnum = chars.iter().position(|c| c.fast_alnum()).unwrap_or(usize::MAX);
    let last_alnum = chars.iter().rposition(|c| c.fast_alnum()).unwrap_or(0);
    let mut found = (false, false);
    let n = words.len();
    for (ti, t) in [&KEY, &WRONG].into_iter().enumerate() {
        let (keys, brackets) = word_sets(ti);
        let is_key = |w: &str| keys.contains(w);
        let is_bracket = |w: &str| brackets.contains(w);
        let annot = |k: usize| words[k].1 - words[k].0 <= 16 && { let x = lower(k); is_key(&x) || is_bracket(&x) };
        // Groups whose words are all annotation words ("(yes)", "(this one)"), checked once each;
        // and the first word from which every word to the end is one ("Venus - this one").
        let mut group_all: Vec<Option<bool>> = vec![None; group_digit.len()];
        let mut tail_from = n;
        while tail_from > 0 && annot(tail_from - 1) {
            tail_from -= 1;
        }
        let mut hit = false;
        for (k, &(s, e, depth)) in words.iter().enumerate() {
            // Annotation positions only.
            let mut p = s;
            while p > 0 && chars[p - 1].is_whitespace() {
                p -= 1;
            }
            // (Content before or after may be a number, a formula, a date: "6 - correct", "1945
            // (correct)", "$40 (correct)".)
            let delim_before = p > 0 && first_alnum < p && (matches!(chars[p - 1], '-' | '\u{2013}' | '\u{2014}' | ':' | '=' | '<' | '>' | '|' | '~' | '/' | '#' | '\u{FF1A}') || is_arrow(chars[p - 1]));
            let colon_after = k == 0 && chars.get(e) == Some(&':') && last_alnum > e;
            let last = k + 1 == n && first_alnum < s && chars[e..].iter().all(|c| !c.fast_alnum());
            let other_content = first_alnum < s || last_alnum >= e;
            if depth == 0 && !delim_before && !colon_after && !last {
                continue;
            }
            if e - s > 16 {
                continue;
            }
            let w = lower(k);
            let key = is_key(&w);
            let bracket = is_bracket(&w);
            if !key && !bracket {
                continue;
            }
            // In brackets, with other words in the choice ("Venus (correct)", "(yes)"). A short
            // word ("yes", "no", "ok") counts only as the bracket's whole content: "(yes)", not
            // "(no change)".
            let _ = (s, e);
            let mut sole = || match group_of[k] {
                Some(g) => *group_all[g].get_or_insert_with(|| !group_digit[g] && (group_first[g]..n).take_while(|&j| group_of[j] == Some(g)).all(annot)),
                None => false,
            };
            // ("(right)" alone or "(right-hand)" is often a direction: the low-risk rule below.)
            let direction = w == "right" && (chars.get(e) == Some(&'-') || group_of[k].is_some_and(|g| (group_first[g]..n).take_while(|&j| group_of[j] == Some(g)).count() == 1));
            if depth > 0 && other_content && !direction && (key || sole()) {
                hit = true;
                break;
            }
            // After a delimiter ("Venus - correct", "Venus - yes", "Venus <== answer"); a short
            // word only as the last one ("y = x + C" is maths).
            // (A "/" or "|" is a gloss's separator as often: "Rupay / sol" (Quechua / Spanish),
            // "km/h", so a bracket-only word there is content, round 17 critic31a A6.)
            let slash = p > 0 && matches!(chars[p - 1], '/' | '|');
            if delim_before && (key || (k >= tail_from && !slash)) {
                hit = true;
                break;
            }
            if !key {
                continue;
            }
            if colon_after {
                hit = true;
                break;
            }
            // A trailing word: "Venus correct" (not "This is correct", not "The correct one").
            if last && t.trailing.contains(&w.as_str()) {
                // (A digit before the word, "1Answer", "100% correct", forms no word: no previous
                // word to read.)
                let prev = k.checked_sub(1).map(lower).unwrap_or_default();
                if !COPULAS.contains(&prev.as_str()) && !is_key(&prev) {
                    hit = true;
                    break;
                }
            }
        }
        // CJK key words: in brackets or after a delimiter or a space.
        if !hit && chars.iter().any(|c| !c.is_ascii()) {
            let text: String = chars.iter().collect();
            for kw in t.cjk {
                if let Some(b) = text.find(kw) {
                    let before = &text[..b];
                    let has_body = before.chars().any(|c| c.fast_alnum());
                    let last = before.chars().rev().find(|c| !c.is_whitespace());
                    let delim = last.is_some_and(|c| matches!(c, '(' | '[' | '\u{FF08}' | '\u{3010}' | '\u{3014}' | '\u{300C}' | '\u{300E}' | '-' | ':' | '\u{FF1A}' | '=') || is_arrow(c)) || before.ends_with(char::is_whitespace);
                    if has_body && delim {
                        hit = true;
                        break;
                    }
                }
            }
        }
        if ti == 0 {
            found.0 = hit;
        } else {
            found.1 = hit;
        }
    }
    found
}

/// Letters or digits outside `<...>` tags.
fn outside_tags_has_letters(chars: &[char]) -> bool {
    let mut depth = 0;
    for &c in chars {
        match c {
            '<' => depth += 1,
            '>' if depth > 0 => depth -= 1,
            c if depth == 0 && c.fast_alnum() => return true,
            _ => {}
        }
    }
    false
}

/// Wrapping markup that is not HTML: LaTeX emphasis, BBCode, Markdown highlight, strike and links.
fn wrapper_markup(text: &str) -> Option<&'static str> {
    let low = text.to_ascii_lowercase();
    const LATEX: &[&str] = &["\\textbf", "\\textit", "\\emph", "\\underline", "\\mathbf", "\\boldsymbol", "\\bf ", "\\bf{", "\\it ", "\\boxed", "\\colorbox", "\\textcolor", "\\color{", "\\fbox", "\\cancel", "\\sout", "\\st{", "\\hl{", "\\overline", "\\textsc"];
    // Boxes, colours, highlights and strikes are never maths notation.
    const LATEX_MARK: &[&str] = &["\\boxed", "\\colorbox", "\\textcolor", "\\color{", "\\fbox", "\\cancel", "\\sout", "\\st{", "\\hl{", "\\fcolorbox", "\\framebox"];
    if LATEX_MARK.iter().any(|k| low.contains(k)) {
        return Some("latex_mark");
    }
    if LATEX.iter().any(|k| low.contains(k)) {
        return Some("latex");
    }
    const BB: &[&str] = &["[b]", "[/b]", "[i]", "[/i]", "[u]", "[/u]", "[s]", "[/s]", "[color", "[/color]", "[size", "[highlight", "[mark]"];
    if BB.iter().any(|k| low.contains(k)) {
        return Some("bbcode");
    }
    let t = text.trim();
    if t.len() > 4 && ((t.starts_with("==") && t.ends_with("==")) || (t.starts_with("~~") && t.ends_with("~~")) || (t.starts_with("++") && t.ends_with("++"))) {
        return Some("md");
    }
    if t.contains("](") && t.starts_with('[') {
        return Some("link");
    }
    None
}

/// The marker features of one served choice (see the module table). Empty for a choice with no
/// letter or digit: it is all content.
pub fn features(text: &str) -> Vec<Feature> {
    // HTML character references are served as typed and may be rendered: "&#10007;" is a cross.
    // (Decoded once: "&amp;#10007;" stays "&#10007;", which the outlier net still compares.)
    if let Some(decoded) = decode_entities(text) {
        let mut f = features_of(&decoded);
        if f.len() <= MAX_FEATURES {
            f.push(Feature { kind: "s_entity", value: String::new(), risk: Risk::Low });
        }
        return f;
    }
    features_of(text)
}

fn features_of(text: &str) -> Vec<Feature> {
    let chars: Vec<char> = text.chars().collect();
    let mut f: Vec<Feature> = Vec::new();
    let Some(first) = chars.iter().position(|c| is_alnum(*c)) else { return f };
    let last = chars.iter().rposition(|c| is_alnum(*c)).unwrap_or(first);
    // (At most MAX_FEATURES + 1 are kept: one past the cap marks the choice too complex.)
    let add = |f: &mut Vec<Feature>, kind: &'static str, value: String, risk: Risk| {
        if f.len() <= MAX_FEATURES && !f.iter().any(|x| x.kind == kind && x.value == value) {
            f.push(Feature { kind, value, risk });
        }
    };
    let alnum_at = |k: isize| k >= 0 && (k as usize) < chars.len() && is_alnum(chars[k as usize]);
    // Per-character features.
    let mut star_seen = false;
    // (Whether there is text outside tags: once per choice, not once per "<".)
    let outside = std::cell::OnceCell::new();
    for (k, &c) in chars.iter().enumerate() {
        let u = c as u32;
        // ASCII letters, digits and spaces carry no feature: the common case is one comparison.
        if c.is_ascii_alphanumeric() || c == ' ' {
            continue;
        }
        if is_check(c) {
            add(&mut f, "check", String::new(), Risk::High);
        } else if is_cross(c) {
            add(&mut f, "cross", String::new(), Risk::High);
        } else if is_star(c) {
            add(&mut f, "star", String::new(), Risk::High);
        } else if is_emoji(c) {
            add(&mut f, "emoji", String::new(), Risk::High);
        }
        if let Some(m) = mark_class(c) {
            add(&mut f, "mark", m.into(), Risk::High);
        }
        if is_math_letter(c) {
            add(&mut f, "math_letters", String::new(), Risk::Low);
        } else if is_enclosed(c) {
            add(&mut f, "enclosed", String::new(), Risk::Low);
        } else if is_modifier_letter(c) {
            add(&mut f, "modifier_letters", String::new(), Risk::Low);
        } else if is_line_mark(c) {
            add(&mut f, "line_mark", String::new(), Risk::High);
        }
        // The net (`check`'s outlier rule): structure no word needs, compared across the choices.
        if !c.is_ascii() {
            match get_general_category(c) {
                // (An arrow inside the text is an equation's: "CH₃COOH ⇌ CH₃COO⁻ + H⁺".)
                G::OtherSymbol if !content_symbol(c) && !is_emoji(c) && !(is_arrow(c) && k > first && k < last) => add(&mut f, "s_symbol", String::new(), Risk::Low),
                // (On a lone letter a mark is a maths accent: "x̄", "ŷ", "v⃗".)
                // (After a letter of a script that writes with marks, Hebrew onwards, "ශ්‍රී", "ភ្នំ":
                // orthography, round 13.)
                G::NonspacingMark | G::EnclosingMark if !matches!(c as u32, 0x20E3 | 0xFE0E | 0xFE0F) && !chars[..k].iter().rev().find(|x| !is_word_mark(**x) && !matches!(**x, '\u{200C}' | '\u{200D}')).is_some_and(|b| b.fast_alpha() && *b as u32 >= 0x0590) && !(k >= 1 && (chars[k - 1].is_ascii_alphabetic() || ('\u{3B1}'..='\u{3C9}').contains(&chars[k - 1])) && !alnum_at(k as isize - 2) && !chars.get(k + 1).is_some_and(|x| x.fast_alpha())) => add(&mut f, "s_combining", String::new(), Risk::Low),
                G::OtherPunctuation if !matches!(c, '\u{BF}' | '\u{A1}' | '\u{B7}' | '\u{2026}' | '\u{3001}' | '\u{3002}' | '\u{FF0C}' | '\u{FF0E}' | '\u{FF1A}' | '\u{FF1B}' | '\u{FF01}' | '\u{FF1F}' | '\u{30FB}' | '\u{060C}' | '\u{061F}' | '\u{0964}') => add(&mut f, "s_punct", String::new(), Risk::Low),
                _ => {}
            }
        }
        match c {
            '\u{FE0F}' => add(&mut f, "presentation", "emoji".into(), Risk::High),
            // Standardised and ideographic variation selectors, Mongolian free variation selectors.
            '\u{FE00}'..='\u{FE0D}' | '\u{E0100}'..='\u{E01EF}' | '\u{180B}'..='\u{180D}' | '\u{180F}' => add(&mut f, "variation", format!("{u:X}"), Risk::High),
            '\u{FE0E}' => add(&mut f, "presentation", "text".into(), Risk::High),
            '\u{20DD}'..='\u{20E0}' | '\u{20E2}'..='\u{20E4}' => add(&mut f, "enclosing", String::new(), Risk::High),
            '\u{FF01}'..='\u{FF5E}' => add(&mut f, "fullwidth", String::new(), Risk::Low),
            '`' => add(&mut f, "markup", "`".into(), Risk::High),
            c if is_asterisk(c) && star_seen => {}
            c if is_asterisk(c) => {
                let (p, n) = (alnum_at(k as isize - 1), alnum_at(k as isize + 1));
                let edge = k < first || k > last;
                let doubled = chars.get(k + 1).is_some_and(|x| is_asterisk(*x)) || (k > 0 && is_asterisk(chars[k - 1]));
                if (p != n) || edge || (doubled && (p || n)) {
                    // (A grade: "A*", "A**" after a lone capital: low risk.)
                    let grade = k >= 1 && chars[..k].iter().rev().take_while(|c| is_asterisk(**c)).count() < 2 && {
                        let before: Vec<&char> = chars[..k].iter().rev().skip_while(|c| is_asterisk(**c)).collect();
                        before.first().is_some_and(|c| c.is_ascii_uppercase()) && !before.get(1).is_some_and(|c| c.fast_alnum())
                    } && chars[k..].iter().all(|c| is_asterisk(*c));
                    add(&mut f, "star", String::new(), if grade { Risk::Low } else { Risk::High });
                    star_seen = true;
                }
            }
            // '_' emphasis wraps a word ("_Venus_", "__Venus__"); "_count" or "snake_case" are names.
            // (Low risk: also Python's __name__.)
            '_' if k < first && chars[last + 1..].contains(&'_') => add(&mut f, "markup", "_".into(), Risk::Low),
            '<' => {
                // An emphasis tag.
                let tag: String = chars[k + 1..].iter().skip_while(|c| **c == '/').take_while(|c| c.is_ascii_alphabetic()).collect::<String>().to_ascii_lowercase();
                let after = k + 1 + chars[k + 1..].iter().take_while(|c| **c == '/').count() + tag.len();
                // A tag that marks up text ("<b>Venus</b>"), not a tag that is the answer ("<b>").
                if matches!(tag.as_str(), "b" | "strong" | "i" | "em" | "u" | "mark" | "span" | "font" | "big" | "s" | "del" | "ins")
                    && chars.get(after).is_some_and(|c| *c == '>' || c.is_whitespace())
                    && *outside.get_or_init(|| outside_tags_has_letters(&chars))
                {
                    add(&mut f, "markup", tag, Risk::High);
                }
            }
            _ => {}
        }
        if !c.is_ascii() && get_general_category(c) == G::Format && !matches!(u, 0x200C | 0x200D | 0xFE0E | 0xFE0F) {
            // Invisible format characters (direction marks, invisible operators, the Mongolian
            // vowel separator, number signs) served on one choice can only mark it: high risk.
            let kind = if k < first || k > last { "invisible_edge" } else { "invisible" };
            add(&mut f, kind, format!("{u:X}"), Risk::High);
        }
    }
    // Look-alike letters from other scripts or blocks inside an otherwise Latin word ("Pаris"
    // with Cyrillic а, "ꓑaris" Lisu, "Lօndon" Armenian, "ⅼondon" a Roman numeral, "Pαris"):
    // high risk. Latin letters that real orthographies and IPA use ("Parıs", "Pɑris", "Biɡ"):
    // low risk.
    // (Turkish writes "ı": a text with its other letters, or "ı" in two words, is Turkish, round 14.)
    let turkish = text.chars().any(|c| matches!(c, '\u{15F}' | '\u{15E}' | '\u{11F}' | '\u{11E}' | '\u{130}')) || text.split_whitespace().filter(|w| w.contains('\u{131}')).count() >= 2;
    for w in text.split_whitespace() {
        let latin_n = w.chars().filter(|c| c.is_ascii_alphabetic()).count();
        if latin_n < 2 {
            continue;
        }
        let digits = w.chars().any(|c| c.is_ascii_digit());
        if w.chars().any(|c| confusable_high(c) || (greek_lookalike(c) && !digits)) {
            add(&mut f, "mixed_script", String::new(), Risk::High);
        } else if w.chars().any(confusable_low) && !turkish {
            add(&mut f, "mixed_script", "latin_ext".into(), Risk::Low);
        }
    }
    if let Some(m) = wrapper_markup(text) {
        add(&mut f, "markup", m.into(), Risk::High);
    }
    // LaTeX ticks and crosses ("\\checkmark", "\\ding{51}", "\\cmark"; "\\xmark", "\\ding{55}"):
    // the marks themselves, high risk (round 14, critic26a).
    if text.contains('\\') {
        let low = text.to_ascii_lowercase();
        if ["\\checkmark", "\\ding{51}", "\\ding{52}", "\\cmark", "\\tick", "\\checked"].iter().any(|m| low.contains(m)) {
            add(&mut f, "check", String::new(), Risk::High);
        }
        if ["\\xmark", "\\ding{55}", "\\ding{56}", "\\crossmark"].iter().any(|m| low.contains(m)) {
            add(&mut f, "cross", String::new(), Risk::High);
        }
    }
    // A spreadsheet's flag column glued on ("E7018,1" / "E6010,0", "E7018;1"): the tail value.
    {
        let t = text.trim_end();
        if let Some((head, v)) = t.rsplit_once([',', ';', '\t'])
            && matches!(v.trim(), "1" | "0" | "100" | "true" | "false" | "TRUE" | "FALSE")
            && head.trim_end().ends_with(|c: char| c.fast_alnum())
            && head.chars().any(crate::format::is_alpha)
        {
            add(&mut f, "tail_token", v.trim().to_lowercase(), Risk::High);
        }
    }
    // Export attributes that mark the key: Moodle XML `fraction="100"`, an HTML radio `checked`,
    // `class="correct"`, `correct="true"` (round 13, critic24a variants).
    if text.contains('<') && text.contains('>') {
        let low = text.to_ascii_lowercase().replace('\'', "\"");
        let attr = EXPORT_ATTRS.iter().find(|a| low.contains(**a));
        if let Some(a) = attr {
            add(&mut f, "markup", a.trim().to_string(), Risk::High);
        }
    }
    // The net: LaTeX commands, $...$ maths, braces.
    if text.contains('\\') && text.split('\\').skip(1).any(|t| t.chars().next().is_some_and(|c| c.is_ascii_alphabetic())) {
        add(&mut f, "s_latex", String::new(), Risk::Low);
    }
    if text.matches('$').count() >= 2 {
        add(&mut f, "s_dollars", String::new(), Risk::Low);
    }
    // The whole choice in quotes or brackets ("“Venus”", "[Venus]", "(Venus)").
    if let (Some(a), Some(z)) = (chars.first(), chars.last())
        && chars.len() > 2
        && matches!((a, z), ('"', '"') | ('\'', '\'') | ('\u{201C}', '\u{201D}') | ('\u{2018}', '\u{2019}') | ('\u{AB}', '\u{BB}') | ('\u{BB}', '\u{AB}') | ('(', ')') | ('[', ']') | ('{', '}') | ('\u{300C}', '\u{300D}'))
        // (An interval or a union of them is wrapped: "(−∞, −2) ∪ (2, ∞)"; "(x − 2)(x + 3)" is not.)
        && (!matches!(a, '(' | '[') || matching_close(&chars) == Some(chars.len() - 1) || chars.iter().any(|c| matches!(c, '\u{222A}' | '\u{2229}')))
    {
        add(&mut f, "s_wrapped", String::new(), Risk::Low);
    }
    // A lowercase choice among capitalised ones is compared by the outlier rule.
    if chars.iter().find(|c| c.fast_alpha()).is_some_and(|c| c.is_ascii_lowercase()) && text.split_whitespace().count() == 1 && chars[0].fast_alnum() && chars.iter().filter(|c| c.is_ascii_alphabetic()).count() >= 2 {
        add(&mut f, "s_lower", String::new(), Risk::Low);
    }
    // A choice that names its siblings ("Both (a) and (b)", "모두 정답") has brackets and words
    // like "correct" for that reason.
    // (Asked only when it matters: sibling_ref is the costliest check here.)
    let sib = std::cell::OnceCell::new();
    let sibling = || *sib.get_or_init(|| crate::labels::sibling_ref(text).is_some());
    let (kw, ww) = key_words(&chars);
    if (kw || ww) && !sibling() {
        if kw {
            add(&mut f, "key_word", String::new(), Risk::High);
        }
        if ww {
            add(&mut f, "wrong_word", String::new(), Risk::High);
        }
    }
    // GIFT: a leading = or ~.
    if matches!(chars[0], '=' | '~') && chars.len() > 1 {
        add(&mut f, "gift", chars[0].to_string(), Risk::High);
    }
    let own_label = std::cell::OnceCell::new();
    // Spaced bracket tokens of up to 3 characters.
    let mut k = 0;
    while k < chars.len() {
        let c = chars[k];
        let close = match c {
            '(' => ')',
            '[' => ']',
            '\u{FF08}' => '\u{FF09}',
            '\u{FF3B}' => '\u{FF3D}',
            _ => {
                k += 1;
                continue;
            }
        };
        // (Fullwidth brackets follow CJK text with no space: "巴黎（√）".)
        let spaced = k == 0 || chars[k - 1].is_whitespace() || matches!(c, '\u{FF08}' | '\u{FF3B}');
        // (A choice's own label, "(1) 22/7", "(b) Mars", is the label analysis's, not a box.)
        if k == 0 && *own_label.get_or_init(|| crate::labels::detect(text).is_some_and(|h| h.start == 0 && (text[h.end..].starts_with(char::is_whitespace) || text[..h.end].ends_with(char::is_whitespace)) && crate::text::has_visible(&text[h.end..]))) {
            k += 1;
            continue;
        }
        if let Some(len) = chars[k + 1..].iter().take(5).position(|x| *x == close) {
            let inner: String = chars[k + 1..k + 1 + len].iter().collect::<String>().trim().to_lowercase();
            let end = k + 1 + len;
            let after_ok = end + 1 >= chars.len() || chars[end + 1].is_whitespace() || !is_alnum(chars[end + 1]);
            // A word in brackets ("(red)", "(Pa)", "(II)") is content; a mark or a letter, digit or
            // symbol token ("(x)", "(*)", "(T)", "(1)", "( )") is a box.
            let word = inner.chars().count() >= 2 && inner.chars().all(crate::format::is_alpha);
            if spaced && after_ok && inner.chars().count() <= 3 && !word && !sibling() {
                // A bracket holding the whole choice is content ("(1, 2)" is longer anyway).
                let whole = k == 0 && end + 1 == chars.len();
                if !whole {
                    // A unit or element symbol after its name ("ampere (A)", "newton (N)", "metre
                    // (m)"): low risk.
                    let prev: String = chars[..k].iter().rev().skip_while(|c| c.is_whitespace()).take_while(|c| c.fast_alpha()).collect::<String>().chars().rev().collect();
                    let letters: String = inner.chars().filter(|c| c.fast_alpha()).collect();
                    let abbr = prev.chars().count() >= 3 && !letters.is_empty() && inner.chars().all(|c| c.fast_alpha() || c == '\u{B0}') && letters.chars().next().zip(prev.chars().next()).is_some_and(|(a, b)| a.to_lowercase().eq(b.to_lowercase()));
                    // (Compared only by presence: "pascal (Pa)" among "newton (N)", "joule (J)".)
                    // (A Roman numeral after a name, "Vagus (X)", "Facial (VII)": low risk.)
                    let raw: String = chars[k + 1..k + 1 + len].iter().collect::<String>();
                    let roman_num = raw.trim().chars().all(|c| matches!(c, 'I' | 'V' | 'X' | 'L')) && !raw.trim().is_empty() && prev.chars().count() >= 3 && prev.chars().next().is_some_and(char::is_uppercase);
                    // (A unit or constant symbol, "ohm (Ω)", "degree (°)", "pi (π)": non-ASCII and
                    // no mark.)
                    let symbol = prev.chars().count() >= 2 && !raw.trim().is_empty() && raw.trim().chars().all(|c| !c.is_ascii() && !is_check(c) && !is_cross(c) && !is_star(c) && !is_emoji(c) && !is_arrow(c) && mark_class(c).is_none() && !matches!(c, '\u{221A}' | '\u{D7}' | '\u{25CB}' | '\u{25EF}' | '\u{25CF}' | '\u{25A1}' | '\u{25A0}' | '\u{25C7}' | '\u{25C6}' | '\u{2610}' | '\u{3007}'));
                    if abbr || symbol {
                        add(&mut f, "s_unit", inner, Risk::Low);
                    } else if roman_num {
                        add(&mut f, "box", inner, Risk::Low);
                    } else {
                        add(&mut f, "box", inner, Risk::High);
                    }
                }
            }
            k = end + 1;
        } else {
            k += 1;
        }
    }
    // Leading and trailing tokens: binary scores and flags ("1 Venus" / "0 Mars", "Venus 100%" /
    // "Mars 0%", "O Paris" / "X London", "Venus T" / "Mars F"), compared with the wrong choices'
    // (they count only when every wrong choice carries the same other one), and one-letter marks
    // ("Paris x", "X Paris") that no wrong choice has.
    let toks: Vec<&str> = text.split_whitespace().collect();
    let binary = |t: &str| -> Option<String> {
        let v: String = t.trim_matches(|c: char| matches!(c, '(' | ')' | '[' | ']' | '#' | '=' | ':' | '{' | '}' | '*')).to_lowercase();
        matches!(v.as_str(), "1" | "0" | "t" | "f" | "y" | "n" | "+" | "-" | "+1" | "-1" | "100%" | "0%" | "%100%" | "%0%" | "1pt" | "0pt" | "true" | "false" | "yes" | "no" | "o" | "x" | "\u{2713}" | "\u{2717}" | "\u{2714}" | "\u{2718}" | "\u{25CB}" | "\u{D7}" | "\u{25EF}" | "\u{3007}")
            .then_some(v)
    };
    if toks.len() >= 2 {
        let words = |a: usize, b: usize| toks[a..b].iter().any(|t| t.chars().any(crate::format::is_alpha));
        if let Some(v) = binary(toks[0]).filter(|_| words(1, toks.len())) {
            // ("O negative", "X chromosome": a letter before a content word is low risk.)
            let letter_word = matches!(v.as_str(), "x" | "o") && toks[1..].iter().any(|t| content_word(t));
            add(&mut f, "lead_token", v.clone(), if letter_word { Risk::Low } else { Risk::High });
            if letter_word {
                add(&mut f, "letter_mark", format!("<{v}"), Risk::Low);
            }
        }
        // The last token, or the one before a unit of score ("[1 pt]", "(0 points)").
        let n = toks.len();
        let last = toks[n - 1].trim_matches(|c: char| matches!(c, ')' | ']' | '.' | ','));
        let tail = if matches!(last.to_lowercase().as_str(), "pt" | "pts" | "point" | "points" | "punkt" | "punkte") && n >= 3 { toks[n - 2] } else { toks[n - 1] };
        if let Some(v) = binary(tail).filter(|_| words(0, n - 1)) {
            add(&mut f, "tail_token", v.clone(), Risk::High);
            if matches!(v.as_str(), "x" | "o") && toks[..n - 1].iter().any(|t| content_word(t)) {
                add(&mut f, "letter_mark", format!(">{v}"), Risk::Low);
            }
        }
    }
    // Edge tokens made of marks ("○ Paris", "Paris √", "Paris ◀", "× London", "Paris ♥").
    // (A unit sign is content after a number, a mark after a word: "90 °" vs "Venus °".)
    let has_digit = text.chars().any(|c| c.is_ascii_digit());
    let mark_token = |t: &str| {
        !t.chars().any(crate::format::is_alnum)
            && (t.chars().any(|c| !matches!(c, '(' | ')' | '[' | ']' | '{' | '}' | '"' | '\'' | '.' | ',' | ';' | ':' | '?' | '\u{201C}' | '\u{201D}' | '\u{2018}' | '\u{2019}' | '\u{2026}' | '-' | '\u{2013}' | '\u{2014}' | '%' | '$' | '\u{20AC}' | '\u{A3}' | '&' | '/') && !(content_symbol(c) && has_digit))
                // A punctuation mark standing apart: "Venus ?", "Venus ;", "Venus ..."
                || t.chars().all(|c| matches!(c, '.' | ',' | ';' | ':' | '?' | '!' | '\u{2026}')))
    };
    // (A unit symbol in brackets, "degree (°)", "ohm (Ω)", is no mark.)
    let unit_token = |t: &str| {
        let inner: Vec<char> = t.trim_start_matches(['(', '[']).trim_end_matches([')', ']']).chars().collect();
        t.starts_with(['(', '[']) && inner.len() == 1 && !inner[0].is_ascii() && !is_check(inner[0]) && !is_cross(inner[0]) && !is_star(inner[0]) && !is_emoji(inner[0]) && !is_arrow(inner[0]) && mark_class(inner[0]).is_none() && !matches!(inner[0], '\u{221A}' | '\u{D7}')
    };
    // (Interval ends are maths: "(−∞, −2) ∪ (2, ∞)".)
    let interval = |t: &str| t.contains('\u{221E}') || (t.contains([',', ';']) && t.contains(['(', ')', '[', ']']));
    // French spacing puts a space before ! ? ; : ("Viens ici !", "Tu viens ?"): low risk at the end.
    let spaced_punct = |t: &str| t.chars().count() <= 3 && t.chars().all(|c| matches!(c, '!' | '?' | ';' | ':' | '\u{BB}'));
    // A whole edge token of circled or parenthesised digits and letters ("Venus ①", "ⓐ Venus").
    let enclosed_token = |t: &str| !t.is_empty() && t.chars().all(is_enclosed);
    if toks.len() >= 2 {
        for (t, side) in [(toks[0], '<'), (toks[toks.len() - 1], '>')] {
            if enclosed_token(t) {
                add(&mut f, "edge_token", format!("{side}enclosed"), Risk::High);
            }
        }
        if mark_token(toks[0]) && !interval(toks[0]) && !unit_token(toks[0]) {
            add(&mut f, "edge_token", format!("<{}", toks[0]), Risk::High);
        }
        let z = toks[toks.len() - 1];
        if mark_token(z) && !interval(z) && !unit_token(z) {
            // (French spacing after a phrase: "Viens ici !", "Tu viens ?" is the sentence's own.)
            if !(spaced_punct(z) && toks.len() >= 3) {
                add(&mut f, "edge_token", format!(">{z}"), if spaced_punct(z) { Risk::Low } else { Risk::High });
            }
        }
    }
    // Edge marker symbols.
    let body_first = chars.get(first).copied();
    let body_last = chars.get(last).copied();
    let first_one_letter = chars[first..].iter().take_while(|c| is_alnum(**c)).count() == 1;
    let last_one_letter = chars[..=last].iter().rev().take_while(|c| is_alnum(**c)).count() == 1;
    let edge = |run: &[char], leading: bool| -> (String, bool) {
        // Markers sit next to the text or at its very end: the 32 characters at each end of a run
        // are enough, and keep this linear whatever the run's length.
        let s: String = if run.len() > 64 { run[..32].iter().chain(&run[run.len() - 32..]).collect() } else { run.iter().collect() };
        let s = if !s.contains(['<', '>', '=']) { s } else { s.replace("<==", "\u{2190}").replace("==>", "\u{2192}").replace("<--", "\u{2190}").replace("-->", "\u{2192}").replace("<-", "\u{2190}").replace("->", "\u{2192}").replace("<=", "\u{2190}").replace("=>", "\u{2192}") };
        let rc: Vec<char> = s.chars().collect();
        let mut out = String::new();
        let mut high = false;
        // Whitespace positions once: "attached" = no space between the char and the body.
        let last_ws = rc.iter().rposition(|x| x.is_whitespace());
        let first_ws = rc.iter().position(|x| x.is_whitespace());
        let bangs = rc.iter().filter(|x| **x == '!').count();
        for (j, &c) in rc.iter().enumerate() {
            // Is this char directly attached to the body (no space between)?
            let attached = if leading { last_ws.is_none_or(|w| w < j) } else { first_ws.is_none_or(|w| w > j) };
            let body_next = if leading { body_first } else { body_last };
            let one_letter_word = if leading { first_one_letter } else { last_one_letter };
            let keep = match c {
                '#' => {
                    // C#, F#, #1 are content.
                    !(attached && (if leading { body_next.is_some_and(|b| b.is_ascii_digit()) } else { body_next.is_some_and(|b| b.fast_alpha()) && one_letter_word }))
                }
                '+' | '-' | '\u{2212}' => {
                    // Next to something (no space between): a sign before a number, a variable,
                    // a lowercase suffix, a math symbol ("-x", "-ing", "-∞"); after something, a
                    // charge or C++. A sign before a capitalised word ("-Mars") or standing apart
                    // ("- Venus", "Venus -") marks.
                    let next = if leading { rc.get(j + 1).copied().or(body_next) } else { None };
                    let prev_close = !leading && (j == 0 || !rc[j - 1].is_whitespace());
                    if leading {
                        match next {
                            Some(n) if n.is_whitespace() => true,
                            Some(n) if n.fast_alpha() && n.is_uppercase() && j + 1 >= rc.len() && !one_letter_word => true,
                            Some(_) => false,
                            None => true,
                        }
                    } else {
                        !prev_close
                    }
                }
                '!' => !leading && bangs >= 2,
                '=' | '~' | '>' | '<' | '\u{2248}' | '\u{2022}' | '\u{25CF}' | '\u{25AA}' | '\u{25B6}' | '\u{25BA}' | '\u{BB}' | '\u{AB}' | '\u{203A}' | '\u{2039}' | '\u{A7}' | '\u{B6}' | '^' | '@' | '\u{2020}' | '\u{2021}' | '\u{25C6}' | '\u{25C7}' | '\u{25A0}' | '\u{25A1}' => true,
                c if is_arrow(c) => true,
                _ => false,
            };
            if keep {
                out.push(c);
                if matches!(c, '#' | '+' | '-' | '\u{2212}' | '!' | '=' | '~' | '>' | '<') || is_arrow(c) {
                    high = true;
                }
            }
        }
        (out, high)
    };
    let (lead, lh) = edge(&chars[..first], true);
    if !lead.is_empty() && !(lead.len() == 1 && matches!(chars[0], '=' | '~')) {
        add(&mut f, "edge", format!("<{lead}"), if lh { Risk::High } else { Risk::Low });
    }
    let (trail, th) = edge(&chars[last + 1..], false);
    if !trail.is_empty() {
        add(&mut f, "edge", format!(">{trail}"), if th { Risk::High } else { Risk::Low });
    }
    // A one-word choice ending in . or ! (a word, not an abbreviation or factorial).
    if toks.len() == 1 {
        let t: Vec<char> = toks[0].chars().collect();
        let n = t.len();
        if n >= 3 && !matches!(t[0], '\u{A1}' | '\u{BF}') && matches!(t[n - 1], '.' | '!' | '\u{FF0E}' | '\u{FF01}') && t[n - 2].fast_alpha() && t[..n - 1].iter().filter(|c| c.fast_alpha()).count() >= 2 && !t[..n - 1].contains(&'.') && !(t[n - 1] == '.' && abbreviation(&t[..n - 1].iter().collect::<String>())) {
            add(&mut f, "terminal", String::new(), Risk::Low);
        }
    }
    // GIFT answer weights ("~%50%yellow", "=%100%Paris", "~%-100%red"): their sign is the key.
    {
        let t: String = chars.iter().skip_while(|c| matches!(c, '=' | '~' | ' ')).collect();
        if let Some(rest) = t.strip_prefix('%')
            && let Some(end) = rest.find('%')
            && let Ok(w) = rest[..end].trim().parse::<f64>()
        {
            let sign = if w > 0.0 { "+" } else if w < 0.0 { "-" } else { "0" };
            add(&mut f, "gift_weight", sign.into(), Risk::High);
        }
    }
    // GIFT feedback left in a choice ("yellow # well done", "red #try again", glued:
    // "English#Sorry…", "Mars#Good job"): a word of two letters or more, "#", then a phrase (two
    // words, or ending in . ! ? …). Not "C#", "#1", "obj#method".
    if let Some(h) = text.find(" #") {
        if text[..h].chars().any(is_alnum) && text[h + 2..].trim_start().chars().next().is_some_and(crate::format::is_alpha) {
            add(&mut f, "gift_feedback", text[h + 2..].trim().to_lowercase(), Risk::High);
        }
    } else if let Some(h) = text.find('#')
        && h > 0
        && text[..h].chars().rev().take_while(|c| c.fast_alpha()).count() >= 2
        && text[h + 1..].chars().next().is_some_and(crate::format::is_alpha)
    {
        let fb = text[h + 1..].trim();
        if fb.split_whitespace().count() >= 2 || fb.ends_with(['.', '!', '?', '\u{2026}']) {
            add(&mut f, "gift_feedback", fb.to_lowercase(), Risk::High);
        }
    }
    // An answer lead opening the choice ("Correct answer: Mars", "Right answer - Mars",
    // "bonne réponse:", "правильный ответ:", "正确答案:", "정답:"): high risk.
    if let Some(r) = answer_prefix(text) {
        add(&mut f, "key_word", r.into(), Risk::High);
    }
    // A short bracketed annotation at either end of a longer choice ("Paris (sahihi)", "巴黎【答案】",
    // "(planet) Mercury"), whatever the language: the outlier net compares it.
    {
        let t = text.trim();
        let open_close = |a: char| match a {
            '(' => Some(')'),
            '[' => Some(']'),
            '\u{FF08}' => Some('\u{FF09}'),
            '\u{3010}' => Some('\u{3011}'),
            '\u{3014}' => Some('\u{3015}'),
            '\u{FF3B}' => Some('\u{FF3D}'),
            _ => None,
        };
        // One lowercase word ("(sahihi)", "(solution)"), or text in CJK brackets ("【答案】"):
        // an annotation; "(Mitosis)", "(cell division)", "(FR)" are content.
        let cjk_bracket = |t: &str| t.starts_with(['\u{3010}', '\u{3014}', '\u{FF08}']) || t.ends_with(['\u{3011}', '\u{3015}', '\u{FF09}']);
        let short = |inner: &str| {
            let w: Vec<&str> = inner.split_whitespace().collect();
            (cjk_bracket(t) && inner.chars().count() <= 8 && inner.chars().any(crate::format::is_alpha))
                || (w.len() == 1 && inner != "aq" && (2..=16).contains(&inner.chars().count()) && inner.chars().next().is_some_and(|c| c.is_lowercase() || !c.is_uppercase() && !c.is_ascii()) && inner.chars().all(crate::format::is_alpha))
        };
        let mut ann = false;
        if let Some(z) = t.chars().last()
            && let Some(o) = t.char_indices().rev().find(|(_, c)| open_close(*c) == Some(z)).map(|x| x.0)
            && o > 0
            && t[..o].chars().any(crate::format::is_alpha)
            // (Spaced: "O(n)", "f(x)" are notation.)
            && (t[..o].ends_with(char::is_whitespace) || cjk_bracket(t))
        {
            let inner = &t[o + t[o..].chars().next().map_or(1, char::len_utf8)..t.len() - z.len_utf8()];
            ann |= short(inner) && !inner.contains([',', ';']);
        }
        if let Some(a) = t.chars().next()
            && let Some(z) = open_close(a)
            && let Some(e) = t.find(z)
            && t[e + z.len_utf8()..].chars().any(crate::format::is_alpha)
        {
            ann |= short(&t[a.len_utf8()..e]);
        }
        if ann {
            add(&mut f, "s_annotation", String::new(), Risk::Low);
        }
    }
    // Round 11, the generic net: a trailing annotation after a delimiter or in spaced brackets
    // (up to six words with a letter: "Nitrogen - the correct answer", "Herz - richtige Antwort",
    // "(die richtige)", "<- richtige Antwort", "Pacifique (100 %)"), or a copula tail ("... is
    // correct", "... is the correct answer"). With an answer or verdict word in it: high risk
    // (`key_word` / `wrong_word`); otherwise the outlier net compares it (`s_annotation`).
    if let Some((ann, copula)) = trailing_annotation(text) {
        let words: Vec<String> = fold_key(ann).split(|c: char| !c.fast_alnum()).filter(|w| !w.is_empty()).map(str::to_string).collect();
        // ("Turn - right", "(no change)": a direction or a plain word alone is content; "is right",
        // "right answer" are verdicts.)
        let weak = |w: &str| matches!(w, "right" | "ok" | "yes" | "true" | "this" | "one");
        let lead = words.iter().any(|w| folded_set(0).contains(w) && !matches!(w.as_str(), "key" | "kunci" | "solution" | "sol" | "clave" | "feedback" | "correct"));
        // (Inflected forms too: "richtige", "corretta", "правильный" from "richtig", "corrett", ...)
        let inflected = |w: &str| folded_list(6).iter().any(|v| v.chars().count() >= 6 && w.starts_with(v.as_str()) && w.len() <= v.len() + 4);
        let verdict = words.iter().any(|w| (folded_set(6).contains(w) || folded_set(0).contains(w) || inflected(w)) && !matches!(w.as_str(), "key" | "kunci" | "solution" | "sol" | "clave" | "feedback") && (!weak(w) || copula || lead));
        let wrong = words.iter().any(|w| wrong_set().contains(w)) || (words.len() == 1 && wrong_bracket_set().contains(&words[0]));
        if wrong && words.len() <= 4 {
            add(&mut f, "wrong_word", String::new(), Risk::High);
        } else if verdict && (words.len() <= 6 || copula) {
            add(&mut f, "key_word", String::new(), Risk::High);
        } else if !copula && words.iter().any(|w| w.chars().any(crate::format::is_alpha)) {
            add(&mut f, "s_annotation", String::new(), Risk::Low);
        }
    }
    // Superscripts and quotation marks, compared by the outlier net ("Paris¹", "“Paris”").
    // (After a word: "Paris¹"; not "m/s²", "x²", "NH₄⁺".)
    // (Digits only: "mol⁻¹", "H⁺" are charges and units.)
    if chars.iter().enumerate().any(|(k, c)| is_super_sub(*c) && !matches!(*c as u32, 0x207A..=0x207E | 0x208A..=0x208E) && k >= 3 && chars[k - 3..k].iter().all(|x| x.fast_alpha()) && chars[k - 3..k].iter().any(|x| x.is_lowercase())) {
        add(&mut f, "s_super", String::new(), Risk::Low);
    }
    // (Typographic quotes only: ASCII quotes are strings in code.)
    if chars.iter().enumerate().any(|(k, c)| is_quote(*c) && !matches!(c, '"' | '\'') && !(*c == '\u{2019}' && alnum_at(k as isize - 1) && alnum_at(k as isize + 1))) {
        add(&mut f, "s_quote", String::new(), Risk::Low);
    }
    // Round 9 (critic20a), structural rules:
    // * scores and points: "(100%)", "[1 pt]", "+1" / "−1", "1/1" at either end. Bracketed,
    //   signed or with a unit of points: high risk; a bare percent or fraction: low.
    {
        let t = text.trim();
        let edge_tokens: Vec<&str> = {
            let w: Vec<&str> = t.split_whitespace().collect();
            if w.len() >= 2 { vec![w[0], w[w.len() - 1]] } else { Vec::new() }
        };
        // (A spaced bracket: "xs[-1]" is an index.)
        let bracket_end = t.rfind(['(', '[']).filter(|&o| t[o..].ends_with([')', ']']) && o > 0 && t[..o].ends_with(char::is_whitespace)).map(|o| &t[o + 1..t.len() - 1]);
        let points = |x: &str| {
            let l = x.trim().to_lowercase();
            // ("worth 1 point", "for 2 marks")
            let l = l.strip_prefix("worth ").or_else(|| l.strip_prefix("for ")).or_else(|| l.strip_prefix("value ")).unwrap_or(&l).to_string();
            // ("+1pt", "score: 1", "full marks", "=100", "%100")
            if let Some(n) = l.strip_prefix('=')
                && !n.is_empty()
                && n.chars().all(|c| c.is_ascii_digit())
            {
                return true;
            }
            let l = l.trim_start_matches(['+', '=']).to_string();
            if matches!(l.as_str(), "full marks" | "full mark" | "full credit" | "full points" | "max points") {
                return true;
            }
            if let Some(n) = l.strip_prefix("score:").or_else(|| l.strip_prefix("score ")).or_else(|| l.strip_prefix("points:")).or_else(|| l.strip_prefix("puntos:"))
                && n.trim().chars().all(|c| c.is_ascii_digit() || c == '.')
                && !n.trim().is_empty()
            {
                return true;
            }
            if let Some(n) = l.strip_prefix('%')
                && !n.is_empty()
                && n.chars().all(|c| c.is_ascii_digit())
            {
                return true;
            }
            let num = l.trim_end_matches(|c: char| c.fast_alpha() || c == ' ' || c == '.');
            let unit = l[num.len()..].trim().trim_end_matches('.');
            !num.is_empty() && num.chars().all(|c| c.is_ascii_digit() || c == '.') && matches!(unit, "pt" | "pts" | "point" | "points" | "punkt" | "punkte" | "p" | "marks" | "mark" | "puntos" | "punto" | "punti" | "ponto" | "pontos" | "puan" | "poäng" | "pkt" | "балл" | "балла" | "баллов" | "점" | "分" | "点" | "%" | "% ")
        };
        let score = |x: &str| -> Option<Risk> {
            let x = x.trim().replace('\u{2212}', "-").replace(' ', "");
            let (sign, body) = match x.strip_prefix(['+', '-']) { Some(b) => (true, b.to_string()), None => (false, x.clone()) };
            let pct = body.strip_suffix('%').is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit() || c == '.'));
            let frac = body.split_once('/').is_some_and(|(a, b)| !a.is_empty() && !b.is_empty() && a.chars().all(|c| c.is_ascii_digit()) && b.chars().all(|c| c.is_ascii_digit()));
            let int = !body.is_empty() && body.chars().all(|c| c.is_ascii_digit()) && body.len() <= 3;
            if sign && (int || pct) && !text.contains(['=', '<', '>', '\u{2264}', '\u{2265}']) {
                Some(Risk::High)
            } else if pct || frac {
                Some(Risk::Low)
            } else {
                None
            }
        };
        // Bracketed points or a signed score next to any content ("6 (1 pt)", "6 +1", "1945
        // (100%)"); after a delimiter at the end ("Mars = 1 point", "Mars — 5 pts").
        if let Some(b) = bracket_end
            && (points(b) || score(b) == Some(Risk::High) || (score(b).is_some() && t.chars().filter(|c| c.fast_alpha()).count() >= 3))
        {
            add(&mut f, "score", b.trim().replace('\u{2212}', "-"), Risk::High);
        }
        if let Some(sep) = t.rfind(['=', ':', '-', '\u{2013}', '\u{2014}'])
            && sep > 0
            && t[..sep].chars().any(crate::format::is_alnum)
            && points(t[sep..].trim_start_matches(['=', ':', '-', '\u{2013}', '\u{2014}', ' ']))
        {
            add(&mut f, "score", "points".into(), Risk::High);
        }
        if let Some(last) = edge_tokens.last()
            && score(last) == Some(Risk::High)
            && edge_tokens.len() == 2
        {
            add(&mut f, "score", last.replace('\u{2212}', "-"), Risk::High);
        }
        // The last one or two tokens a score ("Mars 1 mark", "Mars 2 marks", "Mars +1pt", "Mars =100",
        // "Mars %100") after other words.
        {
            let w: Vec<&str> = t.split_whitespace().collect();
            let n = w.len();
            if n >= 2 && w[..n - 1].iter().any(|x| x.chars().any(crate::format::is_alpha)) && points(w[n - 1]) {
                add(&mut f, "score", "points".into(), Risk::High);
            }
            if n >= 3 && w[..n - 2].iter().any(|x| x.chars().any(crate::format::is_alpha)) && points(&format!("{} {}", w[n - 2], w[n - 1])) {
                add(&mut f, "score", "points".into(), Risk::High);
            }
        }
        if t.chars().filter(|c| c.fast_alpha()).count() >= 3 {
            for e in &edge_tokens {
                if points(e) {
                    add(&mut f, "score", e.to_string(), Risk::High);
                } else if let Some(r) = score(e)
                    && (r == Risk::High || t.trim_end().ends_with(e.trim()))
                {
                    // (A fraction or percentage before words is a quantity, "1/4 inch per foot":
                    // a low-risk score only at the end, round 13.)
                    add(&mut f, "score", e.replace('\u{2212}', "-"), r);
                }
            }
        }
    }
    // * a whole-choice wrapper that is not ordinary text punctuation ("【Nairobi】", "⟨Nairobi⟩",
    //   "《...》", "|Nairobi|", "/Nairobi/"): high risk where only some choices have it.
    if chars.len() > 2 {
        let (a, z) = (chars[0], chars[chars.len() - 1]);
        let fancy_open = !matches!(a, '(' | '[' | '{' | '"' | '\'' | '\u{201C}' | '\u{2018}' | '\u{AB}' | '\u{BB}' | '\u{201E}' | '\u{300C}' | '\u{FF08}' | '\u{BF}' | '\u{A1}');
        let pair = matches!(get_general_category(a), G::OpenPunctuation | G::InitialPunctuation) && matches!(get_general_category(z), G::ClosePunctuation | G::FinalPunctuation);
        let same = matches!((a, z), ('|', '|') | ('/', '/') | ('\u{2016}', '\u{2016}'));
        if (pair && fancy_open) || (same && chars.len() > 3 && chars[1].fast_alpha()) {
            add(&mut f, "wrap", format!("{a}{z}"), Risk::High);
        }
    }
    // * "KEY Nairobi": an all-capitals key word first.
    if let Some(w) = text.split_whitespace().next()
        && text.split_whitespace().nth(1).is_some()
        && matches!(w.trim_end_matches([':', '-', '.']), "KEY" | "ANS" | "ANSWER" | "CORRECT" | "RIGHT" | "SOLUTION")
    {
        add(&mut f, "key_word", String::new(), Risk::High);
    }
    // * "Key: Nairobi" (capitalised): high; "key: value" (code): low.
    if let Some(r) = text.trim_start().strip_prefix("Key:").or_else(|| text.trim_start().strip_prefix("key:"))
        && r.chars().any(crate::format::is_alnum)
    {
        add(&mut f, "key_word", "key".into(), if text.trim_start().starts_with('K') { Risk::High } else { Risk::Low });
    }
    // * "(right)": a direction as often as a verdict: low risk.
    if (text.contains("ight") || text.contains("IGHT")) && text.to_lowercase().split(['(', '[']).skip(1).any(|g| g.split([')', ']']).next().is_some_and(|w| w.trim() == "right")) {
        add(&mut f, "key_word", "right".into(), Risk::Low);
    }
    // A symbol and its name ("✓ (check)", "Backspace (←)", "× (multiply)"): the symbol is the
    // content asked about: its marks are low risk.
    if symbol_named(text) {
        for x in f.iter_mut() {
            if matches!(x.kind, "check" | "cross" | "star" | "mark" | "emoji" | "edge" | "edge_token" | "box") {
                x.risk = Risk::Low;
            }
        }
    }
    f
}

/// "symbol (name)" or "name (symbol)": one side one or two non-alphanumeric characters, the other a
/// word of three letters or more.
fn symbol_named(text: &str) -> bool {
    let t = text.trim();
    let Some(inner) = t.strip_suffix(')').and_then(|x| x.rsplit_once(" (")) else { return false };
    let (outside, inside) = (inner.0.trim(), inner.1.trim());
    let symbol = |x: &str| (1..=2).contains(&x.chars().count()) && !x.chars().any(crate::format::is_alnum);
    let word = |x: &str| x.chars().filter(|c| c.fast_alpha()).count() >= 3 && x.split_whitespace().count() <= 3;
    (symbol(outside) && word(inside)) || (word(outside) && symbol(inside))
}

/// Where the bracket at `chars[0]` closes ("(x − 2)(x + 3)": in the middle).
fn matching_close(chars: &[char]) -> Option<usize> {
    let (open, close) = match chars.first()? {
        '(' => ('(', ')'),
        '[' => ('[', ']'),
        _ => return None,
    };
    let mut d = 0usize;
    for (k, &c) in chars.iter().enumerate() {
        if c == open {
            d += 1;
        } else if c == close {
            d -= 1;
            if d == 0 {
                return Some(k);
            }
        }
    }
    None
}

/// `text` with HTML character references decoded ("&#10007;", "&#x2713;", "&check;", "&times;"),
/// or None when it has none.
fn decode_entities(text: &str) -> Option<String> {
    if !text.contains('&') || !text.contains(';') {
        return None;
    }
    const NAMED: &[(&str, char)] = &[("check", '\u{2713}'), ("checkmark", '\u{2713}'), ("cross", '\u{2717}'), ("times", '\u{D7}'), ("star", '\u{2606}'), ("starf", '\u{2605}'), ("bigstar", '\u{2605}'), ("hearts", '\u{2665}'), ("rarr", '\u{2192}'), ("larr", '\u{2190}'), ("larrb", '\u{21E4}'), ("lArr", '\u{21D0}'), ("rArr", '\u{21D2}'), ("bull", '\u{2022}'), ("nbsp", '\u{A0}'), ("amp", '&'), ("lt", '<'), ("gt", '>'), ("quot", '"'), ("apos", '\''), ("ast", '*'), ("midast", '*'), ("sext", '\u{2736}'), ("cir", '\u{25CB}'), ("squ", '\u{25A1}'), ("blacksquare", '\u{25AA}'), ("diams", '\u{2666}'), ("laquo", '\u{AB}'), ("raquo", '\u{BB}'), ("dagger", '\u{2020}'), ("Dagger", '\u{2021}'), ("sect", '\u{A7}'), ("para", '\u{B6}'), ("deg", '\u{B0}'), ("hellip", '\u{2026}'), ("ndash", '\u{2013}'), ("mdash", '\u{2014}'), ("shy", '\u{AD}'), ("zwj", '\u{200D}'), ("zwnj", '\u{200C}'), ("lrm", '\u{200E}'), ("rlm", '\u{200F}')];
    let mut out = String::with_capacity(text.len());
    let mut changed = false;
    let mut rest = text;
    while let Some(a) = rest.find('&') {
        out.push_str(&rest[..a]);
        let tail = &rest[a + 1..];
        let end = tail.char_indices().take(12).find(|(_, c)| *c == ';').map(|x| x.0);
        let decoded = end.and_then(|e| {
            let name = &tail[..e];
            if let Some(num) = name.strip_prefix('#') {
                let v = if let Some(h) = num.strip_prefix(['x', 'X']) { u32::from_str_radix(h, 16).ok() } else { num.parse::<u32>().ok() };
                v.and_then(char::from_u32).map(|c| (c, e))
            } else {
                NAMED.iter().find(|(n, _)| *n == name).map(|(_, c)| (*c, e))
            }
        });
        match decoded {
            Some((c, e)) => {
                out.push(c);
                rest = &tail[e + 1..];
                changed = true;
            }
            None => {
                out.push('&');
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    changed.then_some(out)
}

/// Uppercase ASCII word of 5+ letters (the whole choice's words are ASCII letters).
fn shouting(t: &str) -> bool {
    t.split_whitespace().any(|w| {
        let l: Vec<char> = w.chars().filter(|c| c.fast_alpha()).collect();
        l.len() >= 5 && w.chars().all(|c| c.is_ascii_uppercase() || !c.fast_alpha()) && l.iter().all(|c| c.is_ascii_uppercase())
    })
}
/// Mixed-case words with no acronym (an all-capital word of 2+ letters).
fn plain_mixed(t: &str) -> bool {
    t.chars().any(|c| c.is_lowercase())
        && !t.split_whitespace().any(|w| {
            let l: Vec<char> = w.chars().filter(|c| c.fast_alpha()).collect();
            l.len() >= 2 && l.iter().all(|c| c.is_uppercase())
        })
}

/// At most this many features per choice: past it the choice is refused as `too_complex` (no
/// acknowledgement), which also bounds the comparison work.
pub const MAX_FEATURES: usize = 24;

/// Kinds that are syntax in code and maths: low risk when the choices look like code or maths.
fn syntax_kind(kind: &str) -> bool {
    matches!(kind, "star" | "markup" | "edge" | "box" | "edge_token" | "lead_token" | "tail_token" | "s_latex" | "s_dollars")
}
/// Marker kinds: anything that annotates a choice (for "every wrong choice is marked, each its own
/// way" and the outlier net's "marked at all").
fn marker_kind(f: &Feature) -> bool {
    match f.kind {
        // A leading or trailing score marks only when it says "wrong" ("Mars 0", "Earth F").
        "lead_token" | "tail_token" => negative_value(&f.value),
        "check" | "cross" | "star" | "mark" | "emoji" | "key_word" | "wrong_word" | "gift" | "edge_token" | "markup" | "line_mark" | "wrap" => f.risk == Risk::High || matches!(f.kind, "edge_token" | "markup"),
        "letter_mark" => true,
        "score" => f.risk == Risk::High,
        "box" => true,
        "edge" => f.risk == Risk::High,
        _ => false,
    }
}

/// Score and flag values that say "wrong".
fn negative_value(v: &str) -> bool {
    matches!(v, "0" | "f" | "n" | "no" | "false" | "x" | "-" | "-1" | "0%" | "%0%" | "0pt" | "\u{2717}" | "\u{2718}" | "\u{D7}")
}

/// The kinds of marks a choice carries, one token counted once ("(N)" is a box and a trailing
/// token, one mark).
fn mark_groups(f: &[Feature]) -> usize {
    let mut g: Vec<&str> = f.iter().filter(|x| marker_kind(x)).map(|x| if matches!(x.kind, "box" | "lead_token" | "tail_token" | "edge_token" | "letter_mark") { "token" } else { x.kind }).collect();
    g.sort();
    g.dedup();
    g.len()
}

/// Kinds compared by value against wrong choices that all carry one value ("[x]" / "[ ]").
fn uniform_kind(kind: &str) -> bool {
    matches!(kind, "box" | "lead_token" | "tail_token" | "edge_token")
}
/// Kinds whose absence on the correct choice counts when every wrong choice has the same one
/// ("~Mars", "Mars (wrong)", "Mars ×", "M̶a̶r̶s̶").
fn absence_kind(kind: &str) -> bool {
    matches!(kind, "cross" | "gift" | "markup" | "star" | "check" | "emoji" | "box" | "lead_token" | "tail_token" | "edge_token" | "edge" | "wrong_word" | "line_mark" | "letter_mark" | "mark")
}

/// Code-like choices: program syntax (`;`, braces, `&`, `|`, `\\`, `$`, `@`, `^`, backticks, `=`,
/// `#`, a call `f(`, an index `a[`, `->`, `=>`, `::`, `//`, `/*`, `<!--`) in at least half of
/// them; or every wrong choice carries its own, different syntax mark ("`code`" among
/// "**code**", "_code_", "> code"). Then syntax only the correct choice uses is the answer.
fn code_like(choices: &[(&str, bool)], feats: &[Vec<Feature>]) -> bool {
    let syntax = |t: &str| {
        let c: Vec<char> = t.chars().collect();
        c.iter().any(|x| matches!(x, ';' | '{' | '}' | '&' | '|' | '\\' | '$' | '@' | '^' | '`' | '=' | '#'))
            || c.windows(2).any(|w| (w[0].fast_alnum() && matches!(w[1], '(' | '[')) || matches!((w[0], w[1]), ('-', '>') | ('=', '>') | (':', ':') | ('/', '/') | ('/', '*') | ('<', '!')))
    };
    let n = choices.iter().filter(|c| syntax(c.0)).count();
    if n * 2 >= choices.len() && n >= 2 {
        return true;
    }
    let wrong: Vec<usize> = (0..choices.len()).filter(|&i| !choices[i].1).collect();
    // (Syntax proper: emphasis, stars, edge symbols; not scores or tokens, which mark wrong choices.)
    let syntax_mark = |f: &Feature| matches!(f.kind, "star" | "markup" | "edge" | "s_latex" | "s_dollars");
    if wrong.len() < 2 || !wrong.iter().all(|&w| feats[w].iter().any(syntax_mark)) {
        return false;
    }
    let mut seen: Vec<(&str, &str)> = Vec::new();
    for &w in &wrong {
        for f in feats[w].iter().filter(|f| syntax_mark(f)) {
            if seen.contains(&(f.kind, f.value.as_str())) {
                return false;
            }
            seen.push((f.kind, f.value.as_str()));
        }
    }
    true
}

/// Notation, more loosely than `code_like`: shell, Markdown, selector and chess marks too ("#nav",
/// ".nav", ">> out.txt", "> quote", "~x", "Qh5+", "out.txt") in at least half the choices. There,
/// syntax only the correct choice has is low risk (the author acknowledges it), not a key mark.
fn notation_like(choices: &[(&str, bool)]) -> bool {
    let syntax = |t: &str| {
        let c: Vec<char> = t.chars().collect();
        c.iter().any(|x| matches!(x, ';' | '{' | '}' | '&' | '|' | '\\' | '$' | '@' | '^' | '`' | '=' | '#' | '~' | '<' | '>'))
            || c.windows(2).any(|w| (w[0].fast_alnum() && matches!(w[1], '(' | '[' | '+' | '*')) || matches!((w[0], w[1]), ('-', '>') | ('=', '>') | (':', ':') | ('/', '/') | ('/', '*') | ('<', '!')))
            || (c.first() == Some(&'.') && c.get(1).is_some_and(|x| x.fast_alpha()))
            || c.windows(5).any(|w| w[0].fast_alpha() && w[1].fast_alpha() && w[2] == '.' && w[3].fast_alpha() && w[4].fast_alpha())
    };
    // (Or a diff or list marker standing first: "+ new line", "- old line", "! line".)
    let marker = |t: &str| t.split_whitespace().next().is_some_and(|w| w.chars().count() <= 2 && w.chars().all(|c| c.is_ascii_punctuation()));
    let n = choices.iter().filter(|c| syntax(c.0) || marker(c.0)).count();
    n * 2 >= choices.len() && n >= 2
}

/// Only after a word of two letters or more ("Mars.", "Why?"; not "n!", "x.").
fn trail_punct(t: &str) -> Option<char> {
    let t = t.trim_end();
    // "¡Hola!", "¿Dónde?": paired punctuation is the sentence's own.
    if t.starts_with(['\u{A1}', '\u{BF}']) {
        return None;
    }
    let p = t.chars().last().filter(|c| matches!(c, '.' | '!' | '?' | ',' | ';' | ':' | '\u{FF0E}' | '\u{3002}' | '\u{FF01}' | '\u{FF1F}'))?;
    let body = t[..t.len() - p.len_utf8()].trim_end_matches(['.', '!', '?']);
    let word = body.chars().rev().take_while(|c| c.fast_alpha()).count();
    let last: String = body.chars().rev().take_while(|c| c.fast_alpha()).collect::<String>().chars().rev().collect();
    (word >= 2 && !(p == '.' && abbreviation(&last))).then_some(p)
}

/// A short abbreviation that ends in a full stop ("Jr.", "Dr.", "St.", "etc.", "vs."): not
/// sentence punctuation.
fn abbreviation(w: &str) -> bool {
    let n = w.chars().count();
    (n <= 3 && w.chars().next().is_some_and(char::is_uppercase) && w.chars().skip(1).all(char::is_lowercase))
        || matches!(w, "etc" | "al" | "vs" | "approx" | "esp" | "incl" | "ca" | "cf" | "resp" | "usw" | "bzw" | "z" | "ecc" | "pp")
}

/// The flagged choices of one served choice list (texts as served, correct flags), with the
/// feature type and risk; empty when every or no choice is correct. Two rules:
/// * the closed list (module table): a marker only the correct choice(s) have, or that every wrong
///   choice shares and the correct one lacks; high or low risk as listed, syntax kinds low when
///   the choices look like code or maths;
/// * the outlier net (low risk, reason `outlier`): any feature, a styled-letter or symbol class,
///   or trailing punctuation present on exactly the correct choices, or (3+ choices) absent on
///   exactly them. Choices with no letter or digit are left out of both.
pub fn check(choices: &[(&str, bool)]) -> Vec<Flag> {
    use std::collections::HashMap;
    // Round 15 (critic28a): HTML whose tags alone differ: the correct choices' tags and
    // attributes (style="font-weight:700", <b>, a class) unlike every wrong choice's, which are all
    // alike. The visible text is no part of the comparison.
    {
        let skeleton = |t: &str| -> String {
            let mut out = String::new();
            let mut rest = t;
            while let Some(a) = rest.find('<') {
                let Some(z) = rest[a..].find('>') else { break };
                let tag = rest[a..a + z + 1].to_ascii_lowercase();
                // (Notation tags are content: H<sub>2</sub>O, x<sup>2</sup>, a line break.)
                let name: String = tag.trim_start_matches(['<', '/']).chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
                // (Formatting elements only: "<stdio.h>", "<!-- -->", "<>" are code.)
                if matches!(name.as_str(), "p" | "span" | "div" | "b" | "strong" | "i" | "em" | "u" | "font" | "mark" | "s" | "del" | "ins" | "small" | "big" | "a" | "li" | "label" | "o") && tag[1..].trim_start_matches('/').starts_with(name.as_str()) {
                    out.push_str(&tag.split_whitespace().collect::<Vec<_>>().join(" "));
                }
                rest = &rest[a + z + 1..];
            }
            out
        };
        let sk: Vec<String> = choices.iter().map(|c| skeleton(c.0)).collect();
        let wrongs: Vec<&String> = choices.iter().zip(&sk).filter(|(c, _)| !c.1).map(|(_, s)| s).collect();
        // (Every choice with visible text: a choice that is a tag, "<a>", is the content.)
        let visible = choices.iter().all(|c| strip_tags(c.0).chars().any(crate::format::is_alnum));
        if visible && sk.iter().any(|s| !s.is_empty()) && wrongs.len() >= 2 && wrongs.iter().all(|s| *s == wrongs[0]) {
            let mut fl = Vec::new();
            for (i, c) in choices.iter().enumerate() {
                if c.1 && sk[i] != *wrongs[0] {
                    fl.push(Flag { index: i, reason: "markup", risk: Risk::High });
                }
            }
            if !fl.is_empty() {
                let mut rest = {
                    // (The rest of the reading on the visible texts.)
                    let bare: Vec<String> = choices.iter().map(|c| strip_tags(c.0)).collect();
                    let b: Vec<(&str, bool)> = bare.iter().zip(choices).map(|(t, c)| (t.as_str(), c.1)).collect();
                    check(&b)
                };
                fl.append(&mut rest);
                fl.sort_by_key(|f| (f.index, f.reason));
                fl.dedup_by_key(|f| (f.index, f.reason));
                return fl;
            }
        }
    }
    // Shapes before every choice, the correct ones filled and every wrong one outlined (or the
    // other way round): a mark (round 15, critic28a: "▲ Mars" among "△ Venus", "◇", "□").
    if choices.len() >= 2 {
        let fill = |t: &str| -> Option<bool> {
            let c = t.trim_start().chars().next()?;
            if matches!(c, '\u{25B2}' | '\u{25C6}' | '\u{25CF}' | '\u{25A0}' | '\u{25BC}' | '\u{25BA}' | '\u{25C0}' | '\u{2605}' | '\u{25AA}') {
                Some(true)
            } else if matches!(c, '\u{25B3}' | '\u{25C7}' | '\u{25CB}' | '\u{25A1}' | '\u{25BD}' | '\u{25B7}' | '\u{25C1}' | '\u{2606}' | '\u{25AB}') {
                Some(false)
            } else {
                None
            }
        };
        let f: Vec<Option<bool>> = choices.iter().map(|c| fill(c.0)).collect();
        if f.iter().all(Option::is_some) {
            let right: Vec<bool> = choices.iter().zip(&f).filter(|(c, _)| c.1).map(|(_, x)| x.unwrap()).collect();
            let wrong: Vec<bool> = choices.iter().zip(&f).filter(|(c, _)| !c.1).map(|(_, x)| x.unwrap()).collect();
            if !right.is_empty() && !wrong.is_empty() && right.iter().all(|x| *x == right[0]) && wrong.iter().all(|x| *x != right[0]) {
                return choices.iter().enumerate().filter(|(_, c)| c.1).map(|(i, _)| Flag { index: i, reason: "mark", risk: Risk::High }).collect();
            }
        }
    }
    // Kahoot's shapes, one each from the filled quartet ▲ ◆ ● ■ before every choice: a label
    // scheme, not marks (round 15, critic28a). An outlined shape (△ ◇ ○ □) is no part of it.
    if (2..=4).contains(&choices.len()) {
        let shape = |t: &str| t.trim_start().chars().next().filter(|c| matches!(c, '\u{25B2}' | '\u{25C6}' | '\u{25CF}' | '\u{25A0}')).filter(|c| t.trim_start()[c.len_utf8()..].starts_with(' '));
        let shapes: Vec<Option<char>> = choices.iter().map(|c| shape(c.0)).collect();
        let mut distinct: Vec<char> = shapes.iter().flatten().copied().collect();
        distinct.sort();
        distinct.dedup();
        if shapes.iter().all(Option::is_some) && distinct.len() == choices.len() {
            let bare: Vec<(&str, bool)> = choices.iter().map(|c| { let t = c.0.trim_start(); let k = t.chars().next().map_or(0, char::len_utf8); (t[k..].trim_start(), c.1) }).collect();
            return check(&bare);
        }
    }
    // Every choice in one same element ("<p dir=\"ltr\">faux</p>", Atto's paragraphs): judged as
    // its content, like the same choices without HTML (round 14, critic26a).
    if choices.len() >= 2 {
        let inner: Vec<Option<(&str, &str)>> = choices.iter().map(|c| html_wrapped(c.0)).collect();
        if let Some(Some((tag, _))) = inner.first()
            && inner.iter().all(|x| x.is_some_and(|(t, _)| t == *tag))
        {
            let bare: Vec<(&str, bool)> = inner.iter().zip(choices).map(|(x, c)| (x.map_or(c.0, |x| x.1), c.1)).collect();
            return check(&bare);
        }
    }
    let n = choices.len();
    let n_correct = choices.iter().filter(|c| c.1).count();
    if n_correct == 0 || n_correct == n {
        return Vec::new();
    }
    let mut flags: Vec<Flag> = Vec::new();
    let flag = |flags: &mut Vec<Flag>, index: usize, reason: &'static str, risk: Risk| {
        if let Some(x) = flags.iter_mut().find(|x| x.index == index && x.reason == reason) {
            x.risk = x.risk.max(risk);
        } else {
            flags.push(Flag { index, reason, risk });
        }
    };
    let mut feats: Vec<Vec<Feature>> = Vec::with_capacity(n);
    // (A text read once: equal choices, or bodies equal once their labels are cut, share it.)
    let mut seen: HashMap<&str, Vec<Feature>> = HashMap::new();
    for (i, c) in choices.iter().enumerate() {
        let mut f = match seen.get(c.0) {
            Some(raw) => raw.clone(),
            None => {
                let raw = features(c.0);
                seen.insert(c.0, raw.clone());
                raw
            }
        };
        if f.len() > MAX_FEATURES {
            flag(&mut flags, i, "too_complex", Risk::High);
            f.truncate(MAX_FEATURES);
        }
        // Any trailing punctuation, one kind: "Kommst du morgen?" among statements is content,
        // "Mars." on every wrong choice but not on "Venus" is the cue.
        if trail_punct(c.0).is_some() {
            f.push(Feature { kind: "s_trail_punct", value: String::new(), risk: Risk::Low });
        }
        feats.push(f);
    }
    // A list's punctuation: ";" (or ",") after every choice but the last, "." after the last
    // ("Neon; Argon; ... Xenon."): the list's style, no choice's mark (round 13).
    if n >= 3 {
        let ends = |t: &str, set: &[char]| t.trim_end().ends_with(set);
        if choices[..n - 1].iter().all(|c| ends(c.0, &[';', ','])) && ends(choices[n - 1].0, &['.', ';', ',']) {
            for f in feats.iter_mut() {
                f.retain(|x| !matches!(x.kind, "s_trail_punct" | "terminal"));
            }
        }
    }
    // Round 13 (critic24a B1): LaTeX quiz packages mark the key by its command: exam class
    // "\CorrectChoice" among "\choice", moodle.sty "\item*" among "\item", AMC "\bonne" among
    // "\mauvaise". The correct choices' leading commands differ from the wrong ones': high risk.
    {
        let cmd = |t: &str| -> Option<String> {
            let t = t.trim_start().strip_prefix('\\')?;
            let n = t.chars().take_while(|c| c.is_ascii_alphabetic()).count();
            (n > 0).then(|| format!("{}{}", t[..n].to_ascii_lowercase(), if t[n..].starts_with('*') { "*" } else { "" }))
        };
        let cmds: Vec<Option<String>> = choices.iter().map(|c| cmd(c.0)).collect();
        let of = |want: bool| -> std::collections::BTreeSet<Option<String>> { choices.iter().zip(&cmds).filter(|(c, _)| c.1 == want).map(|(_, x)| x.clone()).collect() };
        let (right, wrong) = (of(true), of(false));
        // (Only the quiz packages' choice commands: "\\frac" on the key and "\\sqrt" on the others
        // is mathematics.)
        const QUIZ: &[&str] = &["choice", "correctchoice", "item", "item*", "bonne", "mauvaise", "wrongchoice", "correct", "wrong", "true", "false"];
        let quiz = cmds.iter().flatten().any(|c| QUIZ.contains(&c.as_str()));
        // (Export attributes too: only the correct choices carry `fraction="100"`, `checked`.)
        let attr = |i: usize| feats[i].iter().any(|f| f.kind == "markup" && EXPORT_ATTRS.iter().any(|a| a.trim() == f.value));
        let attr_key = (0..n).any(|i| choices[i].1 && attr(i)) && (0..n).all(|i| choices[i].1 || !attr(i));
        if (quiz && right.is_disjoint(&wrong)) || attr_key {
            let reason = if attr_key { "export_key" } else { "latex_key" };
            for (i, c) in choices.iter().enumerate() {
                if c.1 {
                    flag(&mut flags, i, reason, Risk::High);
                }
            }
        }
    }
    let content: Vec<bool> = choices.iter().map(|c| c.0.chars().any(crate::format::is_alnum)).collect();
    let codey = code_like(choices, &feats);
    let notation = codey || notation_like(choices);
    let wrong: Vec<usize> = (0..n).filter(|&i| !choices[i].1).collect();
    let several = wrong.len() >= 2;
    // Counts over the wrong choices, once: (kind, value) and kind.
    let mut wv: HashMap<(&str, &str), usize> = HashMap::new();
    let mut wk: HashMap<&str, usize> = HashMap::new();
    let mut wk_value: HashMap<&str, &str> = HashMap::new();
    let mut wk_uniform: HashMap<&str, bool> = HashMap::new();
    for &w in &wrong {
        let mut seen_kinds: Vec<&str> = Vec::new();
        for ft in &feats[w] {
            *wv.entry((ft.kind, ft.value.as_str())).or_default() += 1;
            if !seen_kinds.contains(&ft.kind) {
                seen_kinds.push(ft.kind);
                *wk.entry(ft.kind).or_default() += 1;
            }
            // (Every value a wrong choice carries, not only its first of a kind: "- [ ] Elm (U)"
            // has two boxes, round 14 critic26a.)
            match wk_value.get(ft.kind) {
                None => {
                    wk_value.insert(ft.kind, ft.value.as_str());
                    wk_uniform.insert(ft.kind, true);
                }
                Some(v) if *v != ft.value => {
                    wk_uniform.insert(ft.kind, false);
                }
                _ => {}
            }
        }
    }
    for i in (0..n).filter(|&i| choices[i].1) {
        for ft in &feats[i] {
            if ft.kind.starts_with("s_") {
                continue;
            }
            let none = !wk.contains_key(ft.kind);
            let unique = match ft.kind {
                // A one-word "Venus." counts only when no wrong choice ends in sentence punctuation.
                "terminal" => !wrong.iter().any(|&w| choices[w].0.trim_end().ends_with(['.', '!', '?', '\u{2026}', '\u{FF0E}', '\u{FF01}', '\u{FF1F}', '\u{3002}'])),
                // (A score on every choice, each its own, "Bogey (+1)", "Birdie (-1)", is content:
                // round 14, critic26a.)
                "score" => !wv.contains_key(&(ft.kind, ft.value.as_str())) && (!wk.contains_key(ft.kind) || wk_uniform.get(ft.kind) == Some(&true)),
                "markup" | "mark" | "letter_mark" | "presentation" | "edge" | "gift" | "invisible" | "invisible_edge" | "variation" | "gift_weight" | "gift_feedback" | "wrap" => !wv.contains_key(&(ft.kind, ft.value.as_str())),
                k if uniform_kind(k) => {
                    // A leading or trailing score counts only against wrong choices that all carry
                    // another one; a box or mark token also when no wrong choice has one.
                    (!matches!(k, "lead_token" | "tail_token") && none)
                        || several && wk.get(k) == Some(&wrong.len()) && wk_uniform.get(k) == Some(&true) && wk_value.get(k).is_some_and(|v| *v != ft.value)
                }
                _ => none,
            };
            // Code and notation: syntax that only the correct choice uses may be the answer ("*p",
            // "#menu", "=yellow", "Qh5#"): low risk, the author acknowledges it. Unless every
            // wrong choice carries one same mark of that kind ("=Paris" / "~Rome", "~Madrid"),
            // and boxes, colours and strikes in LaTeX, which are never notation.
            // Code and maths: syntax only the correct choice uses is the answer, not a mark ("*p",
            // "#menu", "(x) => x * 2"); looser notation ("#nav" among ".nav", "=yellow" among
            // "~yellow", "#yellow", "Qh5#"): low risk. Not when every wrong choice carries one same
            // mark of that kind ("=Paris" / "~Rome", "~Madrid"), nor for boxes, colours and strikes
            // in LaTeX, which are never notation.
            let shared = several && wk.get(ft.kind) == Some(&wrong.len()) && wk_uniform.get(ft.kind) == Some(&true);
            let pattern = uniform_kind(ft.kind) && several && wk.get(ft.kind) == Some(&wrong.len());
            let syntaxy = (syntax_kind(ft.kind) || ft.kind == "gift") && ft.value != "latex_mark";
            if unique && !(codey && syntax_kind(ft.kind) && ft.value != "latex_mark" && !pattern) {
                let low = (codey || notation) && syntaxy && !shared;
                flag(&mut flags, i, ft.kind, if low || (codey && syntax_kind(ft.kind) && ft.value != "latex_mark") { Risk::Low } else { ft.risk });
            }
        }
        // Absence of a marker every wrong choice shares.
        if several {
            for ft in &feats[wrong[0]] {
                if !absence_kind(ft.kind) || (ft.kind == "edge" && ft.risk == Risk::Low) {
                    continue;
                }
                // Scores and flags on every wrong choice count by absence only when they say "wrong"
                // ("Mars 0", "Mars F", "Mars ×"), not a shared "1" in maths.
                let negative = negative_value(&ft.value);
                if matches!(ft.kind, "lead_token" | "tail_token") && !negative {
                    continue;
                }
                if wv.get(&(ft.kind, ft.value.as_str())) == Some(&wrong.len()) && !feats[i].iter().any(|x| x.kind == ft.kind) {
                    flag(&mut flags, i, ft.kind, if codey && syntax_kind(ft.kind) { Risk::Low } else { Risk::High });
                }
            }
        }
        // Every wrong choice marked, each its own way ("Earth ✗", "Mars (wrong)", "Jupiter ×"), and
        // the correct one not at all; or the correct one is the only one with two or more marks
        // ("*Venus (c)" among "*Mars", "Earth (c)").
        // (In code and notation each choice has its own syntax: low risk.)
        {
            let risk = if notation { Risk::Low } else { Risk::High };
            let marks = |k: usize| mark_groups(&feats[k]);
            if several && marks(i) == 0 && wrong.iter().all(|&w| marks(w) > 0) {
                flag(&mut flags, i, "marked_wrongs", risk);
            }
            if marks(i) >= 2 && wrong.iter().all(|&w| marks(w) < marks(i)) {
                flag(&mut flags, i, "marker_combination", risk);
            }
        }
        // Capitals: VENUS among Mars, Earth (not NAFTA among GATT, the WTO).
        if shouting(choices[i].0) && wrong.iter().all(|&w| plain_mixed(choices[w].0)) {
            flag(&mut flags, i, "capitals", Risk::Low);
        }
    }
    // The outlier net, over every kind (features of the closed list included, at kind level).
    // (Choices naming the others, "both", "All of the above", are compared as references, not
    // as answers; "¿...?" and "¡...!" carry their own punctuation.)
    let paired = |i: usize| choices[i].0.trim_start().starts_with(['\u{A1}', '\u{BF}']);
    let kinds_of = |i: usize| -> Vec<String> {
        // (Leading and trailing score tokens are compared by value only: a number before a
        // unit, "1 and 2", is not a kind of decoration.)
        let mut kinds: Vec<String> = feats[i]
            .iter()
            .filter(|f| !matches!(f.kind, "lead_token" | "tail_token") && !(codey && syntax_kind(f.kind)))
            .map(|f| if f.kind == "presentation" { format!("{}:{}", f.kind, f.value) } else { f.kind.to_string() })
            .collect();
        // "Marked at all", whatever the mark.
        if !codey && feats[i].iter().any(|f| marker_kind(f) && !matches!(f.kind, "lead_token" | "tail_token")) {
            kinds.push("marker".into());
        }
        kinds.sort();
        kinds.dedup();
        kinds
    };
    let all_kinds: Vec<Vec<String>> = (0..n).map(|i| if content[i] { kinds_of(i) } else { Vec::new() }).collect();
    // (Every choice with content of the same kinds, none "¡...!" or "¿...?": no subset of them
    // has an outlier, and the sibling-reference reading below, the costliest here, is skipped.
    // Round 12: a bank of small plain questions.)
    let first_content = (0..n).find(|&i| content[i]);
    let uniform = first_content.is_some_and(|f| (0..n).filter(|&i| content[i]).all(|i| all_kinds[i] == all_kinds[f] && !paired(i)));
    let considered: Vec<usize> = if uniform {
        Vec::new()
    } else {
        (0..n).filter(|&i| content[i] && (choices[i].0.len() > 200 || !matches!(crate::labels::sibling_ref(choices[i].0), Some(crate::labels::SiblingRef::Labels | crate::labels::SiblingRef::Above)))).collect()
    };
    if considered.len() >= 2 {
        let mut by_kind: HashMap<String, Vec<usize>> = HashMap::new();
        for &i in &considered {
            for k in all_kinds[i].iter().cloned() {
                by_kind.entry(k).or_default().push(i);
            }
        }
        let correct: Vec<usize> = considered.iter().copied().filter(|&i| choices[i].1).collect();
        let wrongs: Vec<usize> = considered.iter().copied().filter(|&i| !choices[i].1).collect();
        if !correct.is_empty() && !wrongs.is_empty() {
            let no_absence = ["s_lower", "s_unit"];
            for (kind, present) in &by_kind {
                let (present, correct, wrongs): (Vec<usize>, Vec<usize>, Vec<usize>) = if kind == "s_trail_punct" {
                    let f = |v: &[usize]| v.iter().copied().filter(|&i| !paired(i)).collect::<Vec<_>>();
                    (f(present), f(&correct), f(&wrongs))
                } else {
                    (present.clone(), correct.clone(), wrongs.clone())
                };
                if correct.is_empty() {
                    continue;
                }
                // Present on exactly the correct choices, or (3+ choices) on exactly the wrong ones.
                // (Lowercase counts only where the correct choice has it: a capitalised noun among
                // lowercase words is the grammar being asked about.)
                if present == correct || (considered.len() >= 3 && wrongs.len() >= 2 && present == wrongs && !no_absence.contains(&kind.as_str())) {
                    for &i in &correct {
                        flag(&mut flags, i, "outlier", Risk::Low);
                    }
                }
            }
        }
    }
    flags.sort_by_key(|f| (f.index, f.reason));
    flags
}

/// One form for the many ways of writing a number in a tag: superscript, subscript, circled,
/// parenthesised, dingbat and fullwidth digits become ASCII digits ("¹", "①", "⑴", "❶" → "1").
fn fold_digits(t: &str) -> String {
    let mut out = String::with_capacity(t.len());
    for c in t.chars() {
        let u = c as u32;
        let n: Option<u32> = match u {
            0x2070 => Some(0),
            0xB9 => Some(1),
            0xB2 => Some(2),
            0xB3 => Some(3),
            0x2074..=0x2079 => Some(u - 0x2070),
            0x2080..=0x2089 => Some(u - 0x2080),
            0x2460..=0x2473 => Some(u - 0x2460 + 1),
            0x2474..=0x2487 => Some(u - 0x2474 + 1),
            0x2488..=0x249B => Some(u - 0x2488 + 1),
            0x2776..=0x277F => Some(u - 0x2776 + 1),
            0x2780..=0x2789 => Some(u - 0x2780 + 1),
            0x278A..=0x2793 => Some(u - 0x278A + 1),
            0xFF10..=0xFF19 => Some(u - 0xFF10),
            0x24EA => Some(0),
            _ => None,
        };
        match n {
            Some(n) => out.push_str(&n.to_string()),
            None => out.extend(c.to_lowercase()),
        }
    }
    out
}

/// The tag tokens at the edges of an item, folded (`fold_digits`, lowercase): a whole first or
/// last word with no letter ("①", "#1", "(1)", "- 1", an emoji); a bracketed short word ("(FR)",
/// "[red]"); a lone letter of any script ("α"); the non-letter run attached to the end of the last
/// word or the start of the first ("France¹", "France①", "France-1", "1)France"), and a short
/// letter suffix after a hyphen ("France-a"). Digits inside a word ("L12", "H2O", "CO2") are part
/// of it.
fn tag_tokens(t: &str) -> Vec<String> {
    let trim = |w: &str| fold_digits(w.trim_matches(|c: char| matches!(c, '(' | ')' | '[' | ']' | '{' | '}' | '.' | ',' | ':' | ';' | '-' | '\u{2013}' | '\u{2014}' | '_')));
    let words: Vec<&str> = t.split_whitespace().collect();
    let mut out: Vec<String> = Vec::new();
    let mut push = |w: String| {
        if !w.is_empty() && !out.contains(&w) {
            out.push(w);
        }
    };
    let n = words.len();
    // (Once per item, not per edge word: both read every word.)
    let function_word = words.iter().any(|x| {
        let x = x.trim_matches(|c: char| !is_alpha(c));
        ["sin", "cos", "tan", "log", "ln", "exp", "lim", "sec", "csc", "cot", "arcsin", "arccos", "arctan", "sinh", "cosh", "tanh", "max", "min", "det", "mod", "gcd"].iter().any(|f| x.eq_ignore_ascii_case(f))
    });
    let content = words.iter().any(|x| x.chars().filter(|c| is_alpha(*c)).nth(2).is_some());
    for (k, w) in words.iter().enumerate() {
        if !(k == 0 || k + 1 == n) {
            continue;
        }
        let letters = w.chars().filter(|c| is_alpha(*c)).count();
        if letters == 0 {
            push(trim(w));
            continue;
        }
        // Only tags at the end of a longer item, or at its start.
        if n >= 2 {
            let bracketed = matches!((w.chars().next(), w.chars().last()), (Some('('), Some(')')) | (Some('['), Some(']')) | (Some('{'), Some('}')));
            if bracketed && letters <= 8 {
                push(trim(w));
            }
            // A lone letter: of another script ("α"), or Latin after a content word that is not a
            // maths function ("Peru b"; "sin x" / "cos x" are variables). A short lowercase Roman
            // numeral after a word ("Japan iii").
            if letters == 1 && w.chars().filter(|c| c.fast_alnum()).count() == 1 && (!w.chars().any(|c| c.is_ascii_alphabetic()) || (content && !function_word && k + 1 == n && w.chars().all(|c| c.is_ascii_lowercase()))) {
                push(trim(w));
            }
            if k + 1 == n && (2..=4).contains(&letters) && w.chars().all(|c| matches!(c, 'i' | 'v' | 'x')) && content {
                push(w.to_string());
            }
        }
        // A letter of another script glued to a Latin word ("Kenyaα").
        if k + 1 == n
            && let Some(z) = w.chars().last()
            && z.fast_alpha()
            && (z as u32) >= 0x370
            && !(0x1E00..=0x1EFF).contains(&(z as u32))
            && w.chars().rev().nth(1).is_some_and(|c| c.is_ascii_alphabetic())
        {
            push(z.to_lowercase().collect());
        }
        let chars: Vec<char> = w.chars().collect();
        let last = chars.iter().rposition(|c| is_alpha(*c)).unwrap_or(0);
        let tail: String = chars[last + 1..].iter().collect();
        let tail_attached_digits = tail.chars().next().is_some_and(|c| c.is_ascii_digit());
        // (An exponent on a variable or unit is maths: "x²", "cm³".)
        let exponent = letters <= 2 && tail.chars().all(|c| is_super_sub(c) || matches!(c as u32, 0x207A..=0x207E));
        if !tail.is_empty() && !tail_attached_digits && !exponent {
            push(trim(&tail));
        }
        if let Some(h) = w.rfind('-') {
            let suffix = &w[h + 1..];
            if (1..=3).contains(&suffix.chars().count()) && suffix.chars().all(crate::format::is_alpha) && w[..h].chars().filter(|c| is_alpha(*c)).count() >= 3 {
                push(fold_digits(suffix));
            }
        }
        let first = chars.iter().position(|c| is_alpha(*c)).unwrap_or(0);
        let head: String = chars[..first].iter().collect();
        if !head.is_empty() && !head.chars().last().is_some_and(|c| c.is_ascii_digit()) {
            push(trim(&head));
        }
    }
    out
}

/// A weak cue: right items tagged with the start of their left item ("France" / "Paris FR",
/// "F: Paris"), in at least two pairs. Low risk. Indices into `pairs`.
pub fn matching_initials(pairs: &[(&str, &str)]) -> Vec<usize> {
    let mut hits = Vec::new();
    for (i, (l, r)) in pairs.iter().enumerate() {
        let left: String = l.split_whitespace().find(|w| w.chars().any(crate::format::is_alpha)).unwrap_or("").chars().filter(|c| c.fast_alpha()).flat_map(|c| c.to_lowercase()).collect();
        let words: Vec<&str> = r.split_whitespace().collect();
        if words.len() < 2 || left.is_empty() {
            continue;
        }
        let tag = |w: &str| {
            let t: String = w.trim_end_matches(':').chars().collect();
            (1..=3).contains(&t.chars().count()) && t.chars().all(|c| c.is_uppercase()) && left.starts_with(&t.to_lowercase())
        };
        if tag(words[0]) || tag(words[words.len() - 1]) {
            hits.push(i);
        }
    }
    if hits.len() >= 2 {
        return hits;
    }
    Vec::new()
}

/// Maths in a matching item: an equation or expression, where a number shared with the other side
/// is what the skill derives ("y = 2x + 5" / "5").
fn mathy(t: &str) -> bool {
    t.chars().any(|c| matches!(c, '=' | '+' | '\u{D7}' | '\u{F7}' | '^' | '<' | '>' | '\u{2264}' | '\u{2265}' | '\u{221A}' | '\u{222B}' | '\u{2211}' | '\u{B2}' | '\u{B3}' | '/' | '*'))
}

/// Matching pairs whose left and right items carry the same tag token, and no other right item
/// does ("France ①" / "Paris ①", "France¹" / "Paris¹", "France-1" / "Paris-1", "#1", a coloured
/// emoji): the tag pairs them whatever the served order. High risk; low (acknowledgeable) when
/// the shared token is a number in maths ("y = 2x + 5" / "5"). Linear: one count of every right
/// item's tokens. Indices into `pairs`.
pub fn matching_shared_tokens(pairs: &[(&str, &str)], distractors: &[&str]) -> Vec<(usize, Risk)> {
    use std::collections::BTreeMap;
    let rights: Vec<Vec<String>> = pairs.iter().map(|p| tag_tokens(p.1)).chain(distractors.iter().map(|d| tag_tokens(d))).collect();
    let mut count: BTreeMap<&str, usize> = BTreeMap::new();
    for r in &rights {
        for t in r {
            *count.entry(t.as_str()).or_default() += 1;
        }
    }
    let mut out = Vec::new();
    for (i, (l, r)) in pairs.iter().enumerate() {
        let lt = tag_tokens(l);
        // (A sentence's own punctuation, "?" ending a question and its translation, pairs
        // nothing: round 14, critic26a.)
        let terminal = |t: &str| t.chars().all(|c| matches!(c, '?' | '!' | '.' | '\u{BF}' | '\u{A1}' | '\u{2026}' | '\u{FF1F}' | '\u{FF01}' | '\u{3002}' | '\u{61F}'));
        let shared: Vec<&String> = rights[i].iter().filter(|t| count.get(t.as_str()) == Some(&1) && lt.contains(t) && !terminal(t)).collect();
        if shared.is_empty() {
            continue;
        }
        let numeric = shared.iter().all(|t| t.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-'));
        let maths = mathy(l) || mathy(r);
        let risk = if (numeric && (maths || !r.chars().any(crate::format::is_alpha))) || (maths && shared.iter().all(|t| t.chars().count() == 1)) { Risk::Low } else { Risk::High };
        out.push((i, risk));
    }
    out
}

/// Case-folded words of an item (letters and digits), for "the right item names its left item".
/// The words of an already lowercased item (one allocation per item, not per word).
fn words_of(low: &str) -> Vec<&str> {
    low.split(|c: char| !is_alnum(c)).filter(|w| !w.is_empty()).collect()
}

/// Matching pairs whose right item contains its own left item ("France" / "Paris (France)",
/// "Capital of France" / "Paris, capital of France"), or some other left item: low risk (round 13).
/// Only left items of three letters or more and at most eight words
/// ("Mg" / "Mg²⁺" is the question). Linear: every left item's word sequence is hashed once, and
/// each right item's word windows of up to eight words are looked up. Indices into `pairs`.
pub fn matching_names_left(pairs: &[(&str, &str)]) -> Vec<(usize, Risk)> {
    use std::collections::hash_map::DefaultHasher;
    use std::collections::HashMap;
    use std::hash::{Hash, Hasher};
    const MAX_WORDS: usize = 8;
    let word_hash = |w: &str| {
        let mut h = DefaultHasher::new();
        w.hash(&mut h);
        h.finish()
    };
    let combine = |a: u64, b: u64| a.wrapping_mul(0x100000001B3).rotate_left(5) ^ b;
    let low_lefts: Vec<String> = pairs.iter().map(|p| p.0.to_lowercase()).collect();
    let lefts: Vec<Vec<&str>> = low_lefts.iter().map(|l| words_of(l)).collect();
    // Scripts without spaces: a right item that contains its own left item as text
    // ("ケニア" / "ケニアの首都ナイロビ").
    let unspaced = |t: &str| t.chars().any(|c| matches!(c as u32, 0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF | 0x0E00..=0x0E7F));
    let mut cjk_own: Vec<usize> = Vec::new();
    for (i, (l, r)) in pairs.iter().enumerate() {
        let l = l.trim();
        if unspaced(l) && l.chars().count() >= 2 && r.contains(l) {
            cjk_own.push(i);
        }
    }
    let mut by_hash: HashMap<u64, Vec<usize>> = HashMap::new();
    for (i, l) in lefts.iter().enumerate() {
        let letters: usize = l.iter().map(|w| w.chars().filter(|c| c.fast_alpha()).count()).sum();
        if l.is_empty() || l.len() > MAX_WORDS || letters < 3 {
            continue;
        }
        let h = l.iter().fold(0u64, |a, w| combine(a, word_hash(w)));
        let e = by_hash.entry(h).or_default();
        // Only identical items share an entry (a hash collision keeps the first sequence).
        if e.is_empty() || lefts[e[0]] == *l {
            e.push(i);
        }
    }
    let mut out = Vec::new();
    for (i, p) in pairs.iter().enumerate() {
        let low_r = p.1.to_lowercase();
        let r = words_of(&low_r);
        let hashes: Vec<u64> = r.iter().map(|w| word_hash(w)).collect();
        let (mut own, mut other) = (false, false);
        for start in 0..r.len() {
            let mut h = 0u64;
            for len in 1..=MAX_WORDS.min(r.len() - start) {
                h = combine(h, hashes[start + len - 1]);
                // (Equal left items share one entry: one comparison per window.)
                if let Some(cands) = by_hash.get(&h)
                    && lefts[cands[0]].len() == len
                    && lefts[cands[0]][..] == r[start..start + len]
                {
                    let mine = cands.binary_search(&i).is_ok();
                    own |= mine;
                    other |= cands.len() > usize::from(mine);
                }
            }
            if own {
                break;
            }
        }
        // (Round 13, critic24a: low risk, acknowledged by the author: a right that holds its own
        // left is content as often as a cue, "Cotton" / "Seed hairs of the cotton plant", "Forte"
        // / "Forte di volume", Irish "Cat" / "Cat".)
        if own || cjk_own.contains(&i) || other {
            out.push((i, Risk::Low));
        }
    }
    out
}

/// Whether `left` (lowercase, served form) gives `right` as an annotation: in brackets, or after a
/// separator at its end ("france (paris)", "france: paris", "france → paris"). Rights of three
/// letters or more. One scan of `left` per occurrence of `right`.
pub fn left_names_right(left: &str, right: &str) -> bool {
    let r = right.trim();
    if r.chars().filter(|c| c.fast_alpha()).count() < 3 || r.len() > left.len() {
        return false;
    }
    // "Nairobi is in Kenya" / "Nairobi": the left item starts with its right item and a copula.
    if let Some(rest) = left.strip_prefix(r)
        && rest.starts_with(' ')
        && rest.split_whitespace().next().is_some_and(|w| folded_list(2).iter().any(|c| c == w) || matches!(w, "are" | "lies" | "liegt" | "se" | "-" | ":"))
    {
        return true;
    }
    // (Bracket depth carried from one occurrence to the next: linear.)
    let (mut depth, mut seen) = (0i64, 0usize);
    for (i, _) in left.match_indices(r) {
        for c in left[seen..i].chars() {
            match c {
                '(' | '[' | '\u{FF08}' | '\u{3010}' => depth += 1,
                ')' | ']' | '\u{FF09}' | '\u{3011}' => depth -= 1,
                _ => {}
            }
        }
        seen = i;
        let before = left[..i].chars().next_back();
        let after_s = &left[i + r.len()..];
        let after = after_s.chars().next();
        if before.is_some_and(crate::format::is_alnum) || after.is_some_and(crate::format::is_alnum) {
            continue;
        }
        let prev = left[..i].trim_end().chars().next_back();
        let rest = after_s.trim_start();
        let opened = depth > 0;
        let tail = rest.is_empty() || rest.chars().all(|c| matches!(c, '.' | ')' | ']' | '\u{FF09}' | '\u{3011}' | '!' | '?'));
        let sep = prev.is_some_and(|c| matches!(c, ':' | '\u{FF1A}' | '-' | '\u{2013}' | '\u{2014}' | '=' | '/' | '|' | ',' | '(' | '[' | '\u{FF08}' | '\u{3010}' | '>') || is_arrow(c));
        if (opened && tail) || (sep && tail && i > 0) {
            return true;
        }
    }
    false
}

/// The word banks of a folded stem: lists of three or more short items (after a lead and a
/// colon, a whole line, a bracket group, or bulleted lines), none of them a blank. Each bank's
/// byte range in `f` and its items, at most 32 banks.
fn word_banks(f: &str) -> Vec<(std::ops::Range<usize>, Vec<&str>)> {
    fn item_ok(t: &str) -> bool {
        !t.is_empty() && t.len() <= 80 && t.split(' ').filter(|w| !w.is_empty()).count() <= 6 && t.chars().any(crate::format::is_alnum) && !t.contains("__")
    }
    type Banks<'a> = Vec<(std::ops::Range<usize>, Vec<&'a str>)>;
    fn push<'a>(start: usize, region: &'a str, out: &mut Banks<'a>) {
        // (No count of banks, sentences or line lengths limits this: each region is read once,
        // in time linear in the text. Round 16, critic28b: 256 decoy banks, 64 sentences, a line of
        // 4 KB hid a word bank.)
        if region.contains("___") {
            return;
        }
        // (A slash separates only where nothing else does: "3 m/s², True, 12 N" keeps its unit.)
        let items: Vec<&str> = if region.contains([',', ';', '|', '\u{B7}', '\u{3001}']) {
            region.split([',', ';', '|', '\u{B7}', '\u{3001}']).map(|t| t.trim().trim_end_matches(['.', '!']).trim()).collect()
        } else {
            region.split('/').map(|t| t.trim().trim_end_matches(['.', '!']).trim()).collect()
        };
        // (An odd item, an empty one after a trailing comma, "...", "and one more that is not
        // used", is skipped, not the whole list: three good items, at least half of them.)
        let good: Vec<&str> = items.iter().copied().filter(|t| item_ok(t)).collect();
        if good.len() >= 3 && good.len() * 2 >= items.iter().filter(|t| !t.is_empty()).count() {
            out.push((start..start + region.len(), good));
        }
    }
    let mut out: Banks = Vec::new();
    let mut off = 0;
    let lines: Vec<(usize, &str)> = f.split_inclusive('\n').map(|l| {
        let o = off;
        off += l.len();
        (o, l.trim_end_matches('\n'))
    }).collect();
    for &(o, line) in lines.iter() {
        // Each sentence after its last colon (or whole).
        let mut so = 0;
        for sentence in line.split_inclusive(". ") {
            let cut = sentence.rfind(':').map_or(0, |i| i + 1);
            let region = &sentence[cut..];
            // (A bracket group at the end is read on its own below.)
            let region = if region.trim_end().ends_with(')') { region.rsplit_once('(').map_or(region, |x| x.0) } else { region };
            push(o + so + cut, region, &mut out);
            so += sentence.len();
        }
        // Bracket groups.
        let mut from = 0;
        while let Some(a) = line[from..].find('(') {
            let a = from + a;
            let Some(z) = line[a..].find(')') else { break };
            let z = a + z;
            push(o + a + 1, &line[a + 1..z], &mut out);
            from = z + 1;
        }
    }
    // Bulleted (or bare short) lines in a run of three or more, after a line that ends with a colon
    // or each with a bullet.
    let bullet = |l: &str| l.trim_start().starts_with(['\u{2022}', '-', '*', '\u{B7}', '\u{25AA}', '\u{25E6}']);
    let mut i = 0;
    while i < lines.len() {
        let after_colon = i > 0 && lines[i - 1].1.trim_end().ends_with(':');
        let mut j = i;
        while j < lines.len() && !lines[j].1.trim().is_empty() && (bullet(lines[j].1) || after_colon) && item_ok(lines[j].1.trim().trim_start_matches(['\u{2022}', '-', '*', '\u{B7}', '\u{25AA}', '\u{25E6}']).trim()) {
            j += 1;
        }
        // (A run is read once, whatever its length: its first 64 lines are the bank. Restarting
        // at each line of a long run was quadratic, critic23a's bullets_bank.)
        if j - i >= 3 {
            let items = lines[i..j].iter().map(|(_, l)| l.trim().trim_start_matches(['\u{2022}', '-', '*', '\u{B7}', '\u{25AA}', '\u{25E6}']).trim().trim_end_matches('.')).collect();
            out.push((lines[i].0..lines[j - 1].0 + lines[j - 1].1.len(), items));
        }
        i = j.max(i + 1);
    }
    out
}

/// A word bank against the answers in their order: `Some("word_bank_order")` when two or more of
/// them stand in the bank in the order students fill them, `Some("word_bank_first")` when the
/// one blank's answer is the bank's first word; the ranges of every bank holding an answer (its
/// occurrences there are no restatement).
/// Round 14 (critic26a B1): a list in a bank's name, description or any stem holding the keys of
/// the bank's questions in question order (a worksheet's "Word bank for questions 1-10"): served
/// banks keep the author's order, so the list's order is the key. `groups`: each question's key
/// texts (folded), in question order. Four or more found in that order: high risk; three: low;
/// fewer, or any out of order: allowed.
pub fn bank_list_order(text: &str, groups_of: &dyn Fn(&str) -> Vec<usize>) -> Option<&'static str> {
    let f = fold_key(text);
    let mut best: Option<&'static str> = None;
    for (_, items) in word_banks(&f) {
        let cands = group_candidates_owned(&items, groups_of);
        let found = cands.iter().filter(|c| !c.is_empty()).count();
        let distinct = cands.iter().flatten().map(|c| c.0).collect::<std::collections::BTreeSet<_>>().len();
        let chain = ordered_chain(&cands).min(distinct.max(usize::from(distinct >= 3) * usize::MAX));
        // (Four or more in question order, at least 60% of those found: high risk; three: low.)
        if chain >= 4 && chain * 5 >= found * 3 {
            return Some("bank_word_bank_order");
        }
        if chain == 3 {
            best = Some("bank_word_bank_three");
        }
    }
    best
}

fn bank_order(f: &str, groups: &[Vec<String>]) -> (Option<&'static str>, Vec<std::ops::Range<usize>>) {
    let mut ranges = Vec::new();
    let mut verdict = None;
    if groups.is_empty() {
        return (None, ranges);
    }
    // Round 20 (critic36a B36a-2): a key text that cannot be one list item ("Washington, D.C.",
    // "Genesis 1:1") is swapped for a plain name in the text and in the groups.
    let neutral = Neutral::new(groups.iter().flatten().map(|t| t.as_str()));
    let swapped = match neutral.apply(f) {
        Ok(x) => x,
        Err(OverBudget) => return (Some("scan_budget"), ranges),
    };
    let (f, swaps): (&str, Vec<Swap>) = match &swapped {
        Some((t, sw)) => (t.as_str(), sw.clone()),
        None => (f, Vec::new()),
    };
    let named = |t: &str| -> String { neutral.name(t).map_or_else(|| t.to_string(), |s| s.to_string()) };
    let renamed: Vec<Vec<String>> = if neutral.is_empty() { Vec::new() } else { groups.iter().map(|g| g.iter().map(|t| named(t)).collect()).collect() };
    let groups: &[Vec<String>] = if neutral.is_empty() { groups } else { &renamed };
    // Each answer text's groups (the blanks that accept it), once.
    let mut groups_of: std::collections::BTreeMap<&str, Vec<usize>> = std::collections::BTreeMap::new();
    for (g, texts) in groups.iter().enumerate() {
        for t in texts {
            let v = groups_of.entry(t.as_str()).or_default();
            if v.last() != Some(&g) && v.len() < 64 {
                v.push(g);
            }
        }
    }
    for (range, items) in word_banks(f) {
        let cands = group_candidates(&items, |it| groups_of.get(it).map_or(&[][..], |v| &v[..]));
        let found = cands.iter().filter(|c| !c.is_empty()).count();
        if found == 0 {
            continue;
        }
        ranges.push(if swaps.is_empty() { range } else { unswap(&swaps, range.start)..unswap(&swaps, range.end) });
        // (A chain counts its items once: two blanks both "cell" over one "cell" are one item.)
        let distinct = cands.iter().flatten().map(|c| c.0).collect::<std::collections::BTreeSet<_>>().len();
        let chain = ordered_chain(&cands).min(distinct);
        // (Three answers or more in blank order, at least 60% of those found: high risk. Two of
        // two: one chance in two for an honest bank, low risk.)
        if chain >= 3 && chain * 5 >= found * 3 {
            verdict = Some("word_bank_order");
        } else if chain == 2 && found == 2 && verdict != Some("word_bank_order") {
            verdict = Some("word_bank_pair");
        } else if groups.len() == 1 && cands[0].iter().any(|c| c.0 == 0) && verdict.is_none() {
            verdict = Some("word_bank_first");
        }
    }
    (verdict, ranges)
}

/// Whether a text (in `fold_key` form) cannot be one item of a word-bank list as it stands: the
/// list reading splits on commas, semicolons, bars and middle dots, cuts a sentence at its last
/// colon and at ". ", reads brackets on their own, trims a final full stop and a bullet, drops an
/// item of more than six words or 80 bytes. "Washington, D.C.", "Genesis 1:1", "Mr.", "(a)" are
/// never an item, so a list of them was read as nothing. Round 20 (critic36a B36a-2).
pub fn splits_in_list(t: &str) -> bool {
    t.contains([',', ';', '|', '\u{B7}', '\u{3001}', ':', '/', '(', ')', '\n', '\t'])
        || t.ends_with(['.', '!'])
        || t.contains(". ")
        || t.contains("__")
        || t.len() > 80
        || t.split(' ').filter(|w| !w.is_empty()).count() > 6
        || t.starts_with(['\u{2022}', '-', '*', '\u{25AA}', '\u{25E6}'])
}

/// Texts that cannot be list items, each swapped for a plain one-word name before a list is read
/// (the same swap in the text and in the keys), so a list of "Paris, France; Rome, Italy" is a
/// list of two items. Never a skip: a text that cannot be read within the work budget is an
/// error to the caller.
#[derive(Default)]
pub struct Neutral {
    lex: Lex,
    names: std::collections::BTreeMap<String, String>,
}

/// One swap: (start in the new text, end in the new text, start in the old, end in the old).
pub type Swap = (usize, usize, usize, usize);

/// The work budget ran out.
#[derive(Debug)]
pub struct OverBudget;

impl Neutral {
    /// For the texts among `texts` that `splits_in_list`.
    pub fn new<'a>(texts: impl Iterator<Item = &'a str>) -> Neutral {
        let mut set: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
        for t in texts {
            if !t.is_empty() && splits_in_list(t) {
                set.insert(t);
            }
        }
        let names: std::collections::BTreeMap<String, String> = set.iter().enumerate().map(|(i, t)| (t.to_string(), format!("pbq{i}pbq"))).collect();
        let items: Vec<(String, u32)> = set.iter().map(|t| (t.to_string(), 0)).collect();
        Neutral { lex: Lex::build(items), names }
    }
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
    /// The plain name of a text that cannot be a list item.
    pub fn name(&self, t: &str) -> Option<&str> {
        self.names.get(t).map(|s| s.as_str())
    }
    /// `f` with each such text at a word boundary swapped for its name (the longest at each start);
    /// None when nothing is swapped; Err when the work budget ran out.
    pub fn apply(&self, f: &str) -> Result<Option<(String, Vec<Swap>)>, OverBudget> {
        if self.is_empty() {
            return Ok(None);
        }
        let fb = f.as_bytes();
        let mut out = String::new();
        let mut swaps: Vec<Swap> = Vec::new();
        let (mut copied, mut skip_to) = (0usize, 0usize);
        let mut prev_alnum = false;
        for (at, c) in f.char_indices() {
            let word_start = !prev_alnum;
            prev_alnum = is_alnum(c);
            if at < skip_to || !word_start || c == ' ' {
                continue;
            }
            let mut best: Option<usize> = None;
            let over = self.lex.walk(fb, at, fb.len() - at, &mut |end, _ents| {
                // (A match ends at a word boundary: not inside a word.)
                let last = f[..end].chars().next_back().is_some_and(is_alnum);
                let next = f[end..].chars().next().is_some_and(is_alnum);
                if !(last && next) {
                    best = Some(end);
                }
                false
            });
            if over || spend(1) {
                return Err(OverBudget);
            }
            if let Some(end) = best {
                let name = &self.names[&f[at..end]];
                out.push_str(&f[copied..at]);
                let ns = out.len();
                out.push_str(name);
                swaps.push((ns, out.len(), at, end));
                copied = end;
                skip_to = end;
            }
        }
        if swaps.is_empty() {
            return Ok(None);
        }
        out.push_str(&f[copied..]);
        Ok(Some((out, swaps)))
    }
}

/// A position of a swapped text, back in the old one.
pub fn unswap(swaps: &[Swap], p: usize) -> usize {
    let i = swaps.partition_point(|s| s.1 <= p);
    if i < swaps.len() && swaps[i].0 <= p {
        return swaps[i].2;
    }
    if i == 0 {
        p
    } else {
        p + swaps[i - 1].3 - swaps[i - 1].1
    }
}

/// The word-bank item lists of a text already in `fold_key` form: the regions `bank_order` reads,
/// each a list of at least three items. Round 18 (critic34a B2): the bank-wide reading of a
/// Matching question's rights in pair order.
pub fn word_bank_lists(f: &str) -> Vec<Vec<&str>> {
    word_banks(f).into_iter().map(|(_, items)| items).collect()
}

/// `bank_order`'s verdict for a text already in `fold_key` form and folded `groups`.
pub fn bank_order_folded(f: &str, groups: &[Vec<String>]) -> Option<&'static str> {
    bank_order(f, groups).0
}

/// For each group (a blank, a question, a part), the list positions of its texts, with whether a
/// tie is allowed (a text shared by groups is one item for both; a True/False word is not).
fn group_candidates<'a>(items: &[&'a str], groups_of: impl Fn(&str) -> &'a [usize]) -> Vec<Vec<(usize, bool)>> {
    // (Sparse, in group order: a bank of thousands of groups costs only those the list names.)
    let mut cands: std::collections::BTreeMap<usize, Vec<(usize, bool)>> = std::collections::BTreeMap::new();
    for (k, it) in items.iter().enumerate() {
        let tie = !folded_set(4).contains(*it);
        for &g in groups_of(it) {
            let c = cands.entry(g).or_default();
            // (Every position: a capped list could lose the one that orders it.)
            c.push((k, tie));
        }
    }
    cands.into_values().collect()
}

/// `group_candidates` with groups given by value.
fn group_candidates_owned(items: &[&str], groups_of: &dyn Fn(&str) -> Vec<usize>) -> Vec<Vec<(usize, bool)>> {
    let mut cands: std::collections::BTreeMap<usize, Vec<(usize, bool)>> = std::collections::BTreeMap::new();
    for (k, it) in items.iter().enumerate() {
        let tie = !folded_set(4).contains(*it);
        for g in groups_of(it) {
            let c = cands.entry(g).or_default();
            // (Every position: a capped list could lose the one that orders it.)
            c.push((k, tie));
        }
    }
    cands.into_values().collect()
}

/// The longest chain of groups, in group order, whose chosen list positions never go back (an
/// equal position only where a tie is allowed): a longest non-decreasing subsequence, each group
/// choosing one of its positions (round 16, critic28b: questions sharing a key, repeated
/// True/False values).
fn ordered_chain(cands: &[Vec<(usize, bool)>]) -> usize {
    // tails[l]: the smallest last position of a chain of length l + 1.
    let mut tails: Vec<usize> = Vec::new();
    for c in cands.iter().filter(|c| !c.is_empty()) {
        // (Largest first, so one group's positions do not chain with each other.)
        let mut ps = c.clone();
        ps.sort_by_key(|x| std::cmp::Reverse(x.0));
        for (p, tie) in ps {
            let l = if tie { tails.partition_point(|&t| t <= p) } else { tails.partition_point(|&t| t < p) };
            if l == tails.len() {
                tails.push(p);
            } else if p < tails[l] {
                tails[l] = p;
            }
        }
    }
    tails.len()
}

/// Whether the folded text `f` holds the folded key `k` as a whole token: no letter or digit
/// glued to it, a number not continued ("4.0" is not in "4.05", "2" not in "2,5"). Every
/// occurrence is read until one is a whole token (round 17, critic31b B2: a cap of 256 places
/// skipped the rest silently; `.any` stops at the first match, so the scan is linear).
pub fn holds_token(f: &str, k: &str) -> bool {
    !k.is_empty() && f.match_indices(k).any(|(p, _)| token_at(f, p, k))
}

/// Whether the folded key `k`, found at byte `p` of `f`, stands there as a whole token (see
/// `holds_token`).
pub fn token_at(f: &str, p: usize, k: &str) -> bool {
    if k.is_empty() || !f.is_char_boundary(p) || !f[p..].starts_with(k) {
        return false;
    }
    let before = f[..p].chars().next_back();
    let rest = &f[p + k.len()..];
    let mut after = rest.chars();
    let a1 = after.next();
    let a2 = after.next();
    let glued_before = before.is_some_and(|c| c.fast_alnum()) && k.chars().next().is_some_and(crate::format::is_alnum);
    let glued_after = a1.is_some_and(|c| c.fast_alnum()) && k.chars().next_back().is_some_and(crate::format::is_alnum);
    let continued = k.ends_with(|c: char| c.is_ascii_digit()) && matches!(a1, Some('.' | ',')) && a2.is_some_and(|c| c.is_ascii_digit());
    let preceded = k.starts_with(|c: char| c.is_ascii_digit()) && matches!(before, Some('.' | ',' | '-' | '\u{2212}')) && f[..p].chars().rev().nth(1).is_some_and(|c| c.is_ascii_digit());
    !glued_before && !glued_after && !continued && !preceded
}

/// The words (runs of letters and digits) of a folded text by their start, built once: the keys
/// of many parts are looked up by their first word, not searched for one by one (round 13,
/// critic24b B2). A sorted vector, not a hash map: no attacker-chosen text reaches a hasher
/// (round 17, critic31b).
pub struct TokenIndex<'a> {
    f: &'a str,
    words: Vec<(&'a str, Vec<usize>)>,
}

impl<'a> TokenIndex<'a> {
    pub fn new(f: &'a str) -> TokenIndex<'a> {
        // (No cap: a key at the 65th place was skipped silently, round 17 critic31b B2.)
        TokenIndex::with_cap(f, usize::MAX)
    }
    /// The index keeping at most `cap` places per word.
    pub fn with_cap(f: &'a str, cap: usize) -> TokenIndex<'a> {
        let mut flat: Vec<(&'a str, usize)> = Vec::new();
        let mut start = None;
        for (i, c) in f.char_indices().chain(std::iter::once((f.len(), ' '))) {
            match (c.fast_alnum() && i < f.len(), start) {
                (true, None) => start = Some(i),
                (false, Some(s0)) => {
                    flat.push((&f[s0..i], s0));
                    start = None;
                }
                _ => {}
            }
        }
        // (A stable sort keeps the places of one word in increasing order.)
        flat.sort_by(|a, b| a.0.cmp(b.0));
        let mut words: Vec<(&'a str, Vec<usize>)> = Vec::new();
        for (w, at) in flat {
            match words.last_mut() {
                Some((last, v)) if *last == w => {
                    if v.len() < cap {
                        v.push(at);
                    }
                }
                _ => words.push((w, vec![at])),
            }
        }
        TokenIndex { f, words }
    }
    /// Every place where the folded key `k` stands as a whole token (as kept by the index).
    pub fn find_all(&self, k: &str) -> Vec<usize> {
        self.find(k)
    }
    /// Where the folded key `k` stands as a whole token (every place kept unless a cap was asked).
    pub fn find(&self, k: &str) -> Vec<usize> {
        let Some((o, w)) = k.char_indices().find(|c| c.1.fast_alnum()).map(|(o, _)| (o, k[o..].split(|c: char| !c.fast_alnum()).next().unwrap_or(""))) else { return Vec::new() };
        let i = self.words.partition_point(|(x, _)| *x < w);
        match self.words.get(i) {
            Some((x, v)) if *x == w => v.iter().filter(|&&s| s >= o && token_at(self.f, s - o, k)).map(|&s| s - o).collect(),
            _ => Vec::new(),
        }
    }
}

/// The copulas each with a space after, built once.
fn copulas_spaced() -> &'static [String] {
    static C: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    C.get_or_init(|| copulas_chars().iter().map(|c| format!("{} ", c.iter().collect::<String>())).collect())
}

/// Whether `text` states part `letter`'s True/False value next to a reference to it: "(a) is
/// False", "(a): False", "part a is true".
pub fn tf_of_part(text: &str, letter: char, value: bool) -> bool {
    tf_of_part_folded(&fold_key(text), letter, value)
}

/// `tf_of_part` on a text already folded (a part stem read against many parts, folded once).
pub fn tf_of_part_folded(f: &str, letter: char, value: bool) -> bool {
    let refs = [format!("({letter})"), format!("part {letter}"), format!("{letter})")];
    refs.iter().any(|r| {
        f.match_indices(r.as_str()).any(|(p, _)| {
            let tail: String = f[p + r.len()..].chars().take(24).collect();
            let tail = tail.trim_start_matches([' ', ':', '=', '-', '>']);
            let tail = copulas_spaced().iter().find_map(|c| tail.strip_prefix(c.as_str())).unwrap_or(tail);
            let w: String = tail.trim_start().chars().take_while(|c| c.fast_alnum() || *c == ' ').collect();
            let w = w.split(' ').next().unwrap_or("");
            folded_set(4).contains(w) && !folded_set(5).contains(w) && true_set().contains(w) == value
        })
    })
}

/// Every part reference with a True/False value after it in a folded text, read once: "(a) is
/// False", "(27): true", "part c is false" → (part index, value). Letters name parts 1-26,
/// numbers 1-50 (round 16, critic28b: parts past "z" were not named; one reading per part was
/// quadratic).
pub fn part_tf_refs(f: &str) -> Vec<(usize, bool)> {
    let mut out = Vec::new();
    let fc: Vec<(usize, char)> = f.char_indices().collect();
    let value_after = |byte: usize| -> Option<bool> {
        let tail: String = f[byte..].chars().take(24).collect();
        let tail = tail.trim_start_matches([' ', ':', '=', '-', '>']);
        let tail = copulas_spaced().iter().find_map(|c| tail.strip_prefix(c.as_str())).unwrap_or(tail);
        let w: String = tail.trim_start().chars().take_while(|c| c.fast_alnum() || *c == ' ').collect();
        let w = w.split(' ').next().unwrap_or("");
        (folded_set(4).contains(w) && !folded_set(5).contains(w)).then(|| true_set().contains(w))
    };
    let label = |t: &str| -> Option<usize> {
        if t.len() == 1 && t.chars().all(|c| c.is_ascii_lowercase()) {
            return Some((t.as_bytes()[0] - b'a') as usize);
        }
        t.parse::<usize>().ok().filter(|n| (1..=50).contains(n)).map(|n| n - 1)
    };
    let mut k = 0;
    // (Every reference: a cap here skipped the rest of the text silently. The enforced part and
    // stem limits bound the count: an 8 KB stem holds at most 2,048 of "(a)t" (4 bytes each);
    // round 17 critic31b B2.)
    while k < fc.len() {
        let (b, c) = fc[k];
        // "(x)" or "x)" at a word start, "part x".
        let open = c == '(';
        let at_word = k == 0 || !fc[k - 1].1.fast_alnum();
        if open || (at_word && c.fast_alnum()) {
            let s0 = if open { k + 1 } else { k };
            let mut e = s0;
            while e < fc.len() && e - s0 < 3 && fc[e].1.is_ascii_alphanumeric() {
                e += 1;
            }
            if e > s0 && e < fc.len() && fc[e].1 == ')' && (open || at_word) {
                let t: String = fc[s0..e].iter().map(|x| x.1).collect();
                if let (Some(i), Some(v)) = (label(&t), value_after(fc[e].0 + 1)) {
                    out.push((i, v));
                }
                k = e + 1;
                continue;
            }
        }
        if at_word && f[b..].starts_with("part ") {
            let rest = &f[b + 5..];
            let t: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
            if let (Some(i), Some(v)) = (label(&t), value_after(b + 5 + t.len())) {
                out.push((i, v));
            }
        }
        k += 1;
    }
    out
}

/// The list position a Matching left item points to at its end: a letter or number in a blank
/// ("Bauxite ___e___"), after a dash, colon or arrow ("Bauxite — A"), or in brackets ("HTTP (5)",
/// "Bauxite (Answer: E)"). Round 13 (critic24a B2): an answer-key copy of a worksheet.
pub fn left_points_to(left: &str) -> Option<u32> {
    let f = fold_key(left);
    let t = f.trim_end_matches(['.', ' ']);
    let value = |tok: &str| -> Option<u32> {
        let tok = tok.trim();
        if tok.len() == 1 && tok.chars().all(|c| c.is_ascii_lowercase()) {
            return Some(tok.as_bytes()[0] as u32 - b'a' as u32 + 1);
        }
        tok.parse::<u32>().ok().filter(|v| (1..=999).contains(v) && tok.len() <= 3)
    };
    // In a blank.
    let blank = t.trim_end_matches('_');
    if blank.len() < t.len() || blank.ends_with(|c: char| c.is_ascii_alphanumeric()) {
        let tok_len = blank.chars().rev().take_while(|c| c.is_ascii_alphanumeric()).count();
        let (head, tok) = blank.split_at(blank.len() - tok_len);
        if tok_len > 0 && head.trim_end().ends_with("__") && head.trim().chars().any(crate::format::is_alnum) {
            return value(tok);
        }
    }
    // In brackets at the end, with or without a lead.
    if let Some(inner) = t.strip_suffix([')', ']']).and_then(|x| x.rsplit_once(['(', '['])).filter(|x| x.0.trim().chars().any(crate::format::is_alnum)).map(|x| x.1) {
        let inner = inner.trim();
        let inner = inner.rsplit_once([':', ' ']).filter(|x| folded_set(0).contains(x.0.trim().trim_end_matches(':'))).map_or(inner, |x| x.1);
        return value(inner);
    }
    // After a dash, colon, equals or arrow.
    let tok_len = t.chars().rev().take_while(|c| c.is_ascii_alphanumeric()).count();
    if (1..=2).contains(&tok_len) {
        let (head, tok) = t.split_at(t.len() - tok_len);
        let h = head.trim_end();
        if head.ends_with(' ') && h.ends_with(['-', ':', '=', '>']) && h.trim_end_matches(['-', ':', '=', '>', ' ']).chars().any(crate::format::is_alnum) {
            return value(tok);
        }
    }
    None
}

/// Whether a Matching left item fills its blank with its own right item ("Bauxite ___Aluminium___",
/// "Bauxite ___ Aluminium ___"): the answer-key copy of a worksheet (round 13).
pub fn left_blank_holds(left: &str, right: &str) -> bool {
    let (f, r) = (fold_key(left), fold_key(right));
    if r.is_empty() {
        return false;
    }
    let parts: Vec<&str> = f.split("__").collect();
    parts.len() >= 3 && parts[1..parts.len() - 1].iter().any(|p| p.trim_matches(['_', ' ']) == r)
}

/// A labelled list in a stem, on lines or in one line ("a. Tin\nb. Mercury", "Options: 1. 25  2. 53"):
/// each item's position and folded text.
pub fn stem_list(stem: &str) -> Vec<(u32, String)> {
    let mut out = Vec::new();
    let push = |out: &mut Vec<(u32, String)>, seg: &str| {
        let seg = seg.trim();
        // (Numbers past the label alphabet too: "151. item", round 16.)
        let digits = seg.chars().take_while(char::is_ascii_digit).count();
        if (1..=3).contains(&digits) && seg[digits..].starts_with(['.', ')']) && seg[digits + 1..].starts_with(' ') {
            if let Ok(v) = seg[..digits].parse::<u32>() {
                out.push((v, fold_key(seg[digits + 1..].trim())));
            }
            return;
        }
        if let Some(h) = crate::labels::detect(seg).filter(|h| h.start == 0 && !h.suffix && crate::text::has_visible(&seg[h.end..]))
            && let Some(&(_, v)) = h.values.first()
        {
            out.push((v, fold_key(seg[h.end..].trim())));
        }
    };
    for line in stem.lines() {
        // (Items in one line, two spaces or a semicolon apart, after a colon; or label tokens in
        // sequence one space apart, "A. Tin B. Mercury C. Lead".)
        let body = line.rsplit_once(": ").map_or(line, |x| x.1);
        let toks: Vec<&str> = body.split(' ').collect();
        let label_at: Vec<(usize, u32)> = toks.iter().enumerate().filter_map(|(i, t)| {
            let core = t.strip_suffix(['.', ')'])?;
            let v = if core.len() == 1 && core.chars().all(|c| c.is_ascii_alphabetic()) { core.to_ascii_lowercase().as_bytes()[0] as u32 - b'a' as u32 + 1 } else { core.parse::<u32>().ok().filter(|_| core.len() <= 2)? };
            Some((i, v))
        }).collect();
        if label_at.len() >= 3 && label_at.iter().enumerate().all(|(k, x)| x.1 == k as u32 + 1) {
            for (k, &(i, v)) in label_at.iter().enumerate() {
                let end = label_at.get(k + 1).map_or(toks.len(), |x| x.0);
                if end > i + 1 {
                    out.push((v, fold_key(&toks[i + 1..end].join(" "))));
                }
            }
            continue;
        }
        if body.contains("  ") || body.contains("; ") {
            for seg in body.split("  ").flat_map(|x| x.split("; ")) {
                push(&mut out, seg);
            }
        } else {
            push(&mut out, line);
        }
    }
    out
}

/// Attributes of quiz exports that mark the key (lowercase, double quotes).
const EXPORT_ATTRS: &[&str] = &["fraction=\"100\"", " checked", "class=\"correct", "correct=\"true\"", "correct=\"yes\"", "iscorrect=\"true\""];

/// The rest of a text after a leading pair of opposite True/False values ("T F", "V  F", "True /
/// False", "✓ ✗"), or None.
fn tf_pair_prefix(t: &str) -> Option<&str> {
    let t = t.trim_start();
    let mut rest = t;
    let mut vals: Vec<bool> = Vec::new();
    for _ in 0..2 {
        rest = rest.trim_start_matches([' ', '\t', '/', '|']);
        let end = rest.find([' ', '\t', '/', '|']).unwrap_or(rest.len());
        let tok = rest[..end].trim_matches(['(', ')', '[', ']', '.', ':', ',']);
        let w = fold_key(tok);
        if tok.is_empty() || tok.chars().count() > 12 || !folded_set(4).contains(w.as_str()) {
            return None;
        }
        vals.push(true_set().contains(&w));
        rest = &rest[end..];
    }
    (vals[0] != vals[1] && rest.trim().chars().any(crate::format::is_alnum)).then_some(rest.trim_start_matches([' ', '\t', '/', '|']))
}

/// A text that is one paragraph-like element and nothing else ("<p dir=\"ltr\">x</p>",
/// "<span class=\"a\">x</span>", "<div>x</div>"): its opening tag and inner text, when the inner
/// text has no further tags.
fn html_wrapped(t: &str) -> Option<(&str, &str)> {
    let t = t.trim();
    let rest = t.strip_prefix('<')?;
    let name_len = rest.chars().take_while(|c| c.is_ascii_alphabetic()).count();
    let name = &rest[..name_len];
    if !matches!(name.to_ascii_lowercase().as_str(), "p" | "span" | "div") {
        return None;
    }
    let open_end = rest.find('>')? + 1;
    let close = format!("</{name}>");
    let body = rest[open_end..].strip_suffix(close.as_str()).or_else(|| rest[open_end..].strip_suffix(close.to_ascii_lowercase().as_str()))?;
    // (A trailing line break inside, Atto's "<br>", is no content.)
    let body = body.trim_end().trim_end_matches("<br>").trim_end_matches("<br/>").trim_end_matches("<br />");
    // (The whole opening tag is returned: choices are unwrapped only when theirs are the same,
    // so `<p class="correct">` on one choice stays a mark.)
    (!body.contains('<') && !body.trim().is_empty()).then_some((&t[..open_end + 1], body.trim()))
}

/// Whether a text holds Anki cloze syntax: "{{c" digits "::", or "{{type:" / "{{cloze:".
pub fn anki_cloze(text: &str) -> bool {
    // (Every "{{" is read: a cap here would skip a cloze past it, round 17 critic31b B2.)
    text.match_indices("{{").any(|(p, _)| {
        let r = &text[p + 2..];
        let r = r.trim_start();
        let low: String = r.chars().take(8).flat_map(char::to_lowercase).collect();
        if low.starts_with("type:") || low.starts_with("cloze:") {
            return true;
        }
        let Some(rest) = r.strip_prefix(['c', 'C']) else { return false };
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        digits > 0 && rest[digits..].starts_with("::")
    })
}

/// A text without its HTML tags.
fn strip_tags(t: &str) -> String {
    let mut out = String::new();
    let mut rest = t;
    while let Some(a) = rest.find('<') {
        out.push_str(&rest[..a]);
        match rest[a..].find('>') {
            Some(z) => rest = &rest[a + z + 1..],
            None => {
                rest = &rest[a..];
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The work budget of one publish (character steps of the key scans): past it, a text that needs
/// more reading is refused (`scan_budget`) rather than read in part (round 16, critic28b).
const WORK_BUDGET: u64 = 20_000_000;

thread_local! {
    static WORK: std::cell::Cell<(u32, u64)> = const { std::cell::Cell::new((0, 0)) };
}

/// A publish's scope for the work budget: the count starts at zero when the outermost scope
/// opens. Outside any scope nothing is counted.
pub struct WorkScope;

impl WorkScope {
    #[allow(clippy::new_without_default)]
    pub fn new() -> WorkScope {
        WORK.with(|w| {
            let (d, n) = w.get();
            w.set((d + 1, if d == 0 { 0 } else { n }));
        });
        WorkScope
    }
}

impl Drop for WorkScope {
    fn drop(&mut self) {
        let outer = WORK.with(|w| {
            let (d, n) = w.get();
            w.set((d.saturating_sub(1), n));
            d <= 1
        });
        // (The publish's remembered answers go with its scope.)
        if outer {
            crate::labels::clear_sibling_memo();
        }
    }
}

/// Whether a publish's work scope is open.
pub fn in_work_scope() -> bool {
    WORK.with(|w| w.get().0 > 0)
}

/// Whether the work budget of the open scope is spent.
pub fn budget_spent() -> bool {
    WORK.with(|w| {
        let (d, used) = w.get();
        d > 0 && used > WORK_BUDGET
    })
}

/// `spend` for the bank-level scans of `strip`.
pub fn spend_work(n: u64) -> bool {
    spend(n)
}

/// Counts `n` steps; whether the budget is spent.
fn spend(n: u64) -> bool {
    WORK.with(|w| {
        let (d, used) = w.get();
        if d == 0 {
            return false;
        }
        let used = used.saturating_add(n);
        w.set((d, used));
        used > WORK_BUDGET
    })
}

/// `stem_key` reasons the author can acknowledge (low risk): a form hint after a blank, the
/// first word of a word bank being the only answer, both True/False values pasted at the end, a
/// part stem holding another part's key text (given data as often as a leak).
pub const LOW_STEM_REASONS: &[&str] = &["blank_hint", "word_bank_first", "word_bank_pair", "bank_word_bank_three", "choices_pasted", "part_states_key", "key_heading_row"];

/// What a served text (a stem, a part stem, a left item) must not say about the key.
#[derive(Default)]
pub struct KeyInfo {
    /// The key's texts, in `served_text` form and lowercase (correct choices, the sample, accepted
    /// blanks, a left item's paired right).
    pub texts: Vec<String>,
    /// The choices have positions a letter can name (Mc, Multipart).
    pub choices: bool,
    /// A True/False question.
    pub tf: bool,
    /// The wrong choices' texts (a stem line equal to a correct choice gives it away only when
    /// no line equals a wrong one).
    pub wrong: Vec<String>,
    /// A Blank question: an accepted answer next to the blank ("____ (Au)", "→ Au").
    pub blank: bool,
    /// The correct choices' letters by position ("a" for the first choice), when known: "Answer:
    /// B" names the key only if B is a correct position (round 10: "Answer A, B, C or D." is an
    /// instruction).
    pub letters: Vec<char>,
    /// A True/False question's value, when known.
    pub tf_value: Option<bool>,
    /// Students see the choices by position (an order that does not move, labels kept): a bare or
    /// marked letter in the stem ("[B]", "✓B", "→ B") names the key.
    pub positional: bool,
    /// The answers in the order students fill or match them (each blank's accepted answers; with
    /// a fixed Matching order, each pair's right): a word bank in the stem that lists them in
    /// this order is the key (round 12, critic23a B2).
    pub order_groups: Vec<Vec<String>>,
    /// A string of choice letters after a lead is an answer key ("Key: ABT", "Answers: BDAC"):
    /// the bank's name and description (round 13).
    pub letter_keys: bool,
    /// Any readable text after an answer lead counts as a key: whether a text has an answer line
    /// at all (the fail-closed check when a bank's keys exceed what is looked up).
    pub any_key: bool,
    /// Only answer lines ("Answer: X", "答案：X"): the scan of one question's stem against other
    /// questions' or parts' keys.
    pub lines_only: bool,
    /// The key texts as a set and by first character, built once for many stems
    /// (`KeyInfo::with_maps`).
    pub maps: Option<std::rc::Rc<KeyMaps>>,
    /// `texts` (and `wrong`) are already in `fold_key` form (`KeyInfo::folded`): a question's key
    /// is folded once, not once per stem, topic and part.
    pub prefolded: bool,
}

/// How far after an answer lead a key may START, in characters (the line is read to this window
/// plus the longest key, so a key starting inside it may end past it; round 17, critic31b B1).
const KEY_WINDOW: usize = 200;

/// One lex entry: (arena offset, byte length, tag).
pub type Entry = (u32, u32, u32);
/// A cached walk transition: ((lo, hi, depth, byte), (lo, hi)).
type Transition = ((u32, u32, u32, u8), (u32, u32));

/// Texts in one arena, sorted by their bytes, each with a tag (a question, a group, an entry of a
/// side table): exact lookups and prefix walks by binary search, about 12 bytes per text beyond
/// the text itself (round 17, critic31a B6: a trie of 2.5 M nodes took 140 MB).
#[derive(Default, Debug)]
pub struct Lex {
    arena: String,
    ents: Vec<Entry>,
    /// A small direct-mapped transition cache for the walk: (lo, hi, byte) -> (lo, hi). Keys that
    /// share a long prefix (a bank's numbered keys, "q-q-q-...") otherwise pay two binary searches
    /// per byte on every start; the cache makes the shared prefix one step per node. Integer keys,
    /// a fixed 64 KB, never attacker text. Round 17, critic31a B6's arena was slower than the old
    /// trie on exactly those shapes.
    cache: std::cell::RefCell<Vec<Transition>>,
}

impl Lex {
    /// From (text, tag) pairs; equal pairs once.
    pub fn build(mut items: Vec<(String, u32)>) -> Lex {
        items.sort_unstable();
        items.dedup();
        let mut arena = String::with_capacity(items.iter().map(|x| x.0.len()).sum());
        let mut ents: Vec<Entry> = Vec::with_capacity(items.len());
        let mut last: Option<(u32, u32)> = None;
        for (t, tag) in items {
            // (Equal texts share their bytes.)
            let (off, len) = match last {
                Some((o, l)) if &arena[o as usize..(o + l) as usize] == t.as_str() => (o, l),
                _ => {
                    let o = arena.len() as u32;
                    arena.push_str(&t);
                    (o, t.len() as u32)
                }
            };
            last = Some((off, len));
            ents.push((off, len, tag));
        }
        Lex { arena, ents, cache: Default::default() }
    }
    pub fn is_empty(&self) -> bool {
        self.ents.is_empty()
    }
    fn text(&self, i: usize) -> &[u8] {
        let (o, l, _) = self.ents[i];
        &self.arena.as_bytes()[o as usize..(o + l) as usize]
    }
    /// The entries whose text is exactly `t`.
    pub fn exact(&self, t: &str) -> &[Entry] {
        let t = t.as_bytes();
        let lo = self.ents.partition_point(|e| &self.arena.as_bytes()[e.0 as usize..(e.0 + e.1) as usize] < t);
        let hi = lo + self.ents[lo..].partition_point(|e| &self.arena.as_bytes()[e.0 as usize..(e.0 + e.1) as usize] == t);
        &self.ents[lo..hi]
    }
    /// The tag of an entry.
    pub fn tag(e: &Entry) -> u32 {
        e.2
    }
    /// Walks the texts that are prefixes of `s[p..]`, at most `max` bytes: `f(end, entries)` for
    /// each, in increasing length; `f` returning true stops. Each byte step is one `spend` of the
    /// work budget; whether the budget ran out.
    pub fn walk(&self, s: &[u8], p: usize, max: usize, f: &mut dyn FnMut(usize, &[Entry]) -> bool) -> bool {
        self.walk_counted(s, p, max, &std::cell::Cell::new(0), f)
    }
    /// `walk`, also counting each step in `local` (a text's own budget: past 100,000, out).
    pub fn walk_counted(&self, s: &[u8], p: usize, max: usize, local: &std::cell::Cell<u32>, f: &mut dyn FnMut(usize, &[Entry]) -> bool) -> bool {
        self.walk_limited(s, p, max, local, 100_000, f)
    }
    /// `walk_counted` with the text's own limit given.
    pub fn walk_limited(&self, s: &[u8], p: usize, max: usize, local: &std::cell::Cell<u32>, limit: u32, f: &mut dyn FnMut(usize, &[Entry]) -> bool) -> bool {
        let (mut lo, mut hi) = (0usize, self.ents.len());
        let end = s.len().min(p + max);
        const CACHE: usize = 2048;
        if !self.ents.is_empty() && self.cache.borrow().is_empty() {
            self.cache.borrow_mut().resize(CACHE, ((u32::MAX, u32::MAX, u32::MAX, 0), (0, 0)));
        }
        for d in 0..end.saturating_sub(p) {
            if lo >= hi {
                return false;
            }
            local.set(local.get() + 1);
            if spend(1) || local.get() > limit {
                return true;
            }
            let b = s[p + d];
            // (Within lo..hi every text agrees on its first d bytes; shorter ones come first.)
            let at = |i: usize| self.text(i).get(d).copied();
            // (First index in lo..hi where pred fails.)
            let bound = |mut a: usize, mut z: usize, pred: &dyn Fn(usize) -> bool| {
                while a < z {
                    let mid = a + (z - a) / 2;
                    if pred(mid) {
                        a = mid + 1;
                    } else {
                        z = mid;
                    }
                }
                a
            };
            // (The transition of one prefix range by one byte, cached: the shared prefixes of
            // thousands of keys are walked once, not per start. Round 17.)
            let slot = ((lo as u64).wrapping_mul(0x9E37_79B1).rotate_left(7) ^ ((hi as u64) << 20) ^ ((d as u64) << 3) ^ b as u64) as usize & (CACHE - 1);
            let cached = self.cache.borrow()[slot];
            let (nlo, nhi) = if cached.0.0 == lo as u32 && cached.0.1 == hi as u32 && cached.0.2 == d as u32 && cached.0.3 == b {
                (cached.1.0 as usize, cached.1.1 as usize)
            } else {
                let nlo = bound(lo, hi, &|i| at(i).is_none_or(|x| x < b));
                let nhi = bound(nlo, hi, &|i| at(i) == Some(b));
                self.cache.borrow_mut()[slot] = ((lo as u32, hi as u32, d as u32, b), (nlo as u32, nhi as u32));
                (nlo, nhi)
            };
            lo = nlo;
            hi = nhi;
            if lo >= hi {
                return false;
            }
            // Texts of exactly d + 1 bytes come first in the range.
            let full = self.ents[lo..hi].partition_point(|e| e.1 as usize == d + 1);
            if full > 0 && f(p + d + 1, &self.ents[lo..lo + full]) {
                return false;
            }
        }
        false
    }
    /// Whether some text starts with the character at `s[p..]`.
    pub fn may_start(&self, c: char) -> bool {
        let mut buf = [0u8; 4];
        let b = c.encode_utf8(&mut buf).as_bytes();
        let i = self.ents.partition_point(|e| &self.arena.as_bytes()[e.0 as usize..(e.0 + e.1) as usize] < b);
        i < self.ents.len() && self.text(i).starts_with(b)
    }
}

/// The key texts looked up by a lead's scan (each at most KEY_WINDOW characters: a longer key is
/// known by its start), in one sorted arena.
#[derive(Default, Debug)]
pub struct KeyMaps {
    lex: Lex,
    longest: usize,
    /// A key longer than `KEY_WINDOW`: its first `KEY_WINDOW` characters are in the lex with tag 1,
    /// so a match may end past the window (round 17, critic31b C1).
    has_long: bool,
}

impl KeyMaps {
    fn new(keys: &[String]) -> KeyMaps {
        // (Whole keys: one starting in the window may end past it, round 17.)
        let longest = keys.iter().map(|k| k.chars().count()).max().unwrap_or(0);
        // (A key longer than the window is also known by its first KEY_WINDOW characters, tag 1:
        // a line showing that much of it names it.)
        let mut items: Vec<(String, u32)> = keys.iter().filter(|k| !k.is_empty()).map(|k| (k.clone(), 0)).collect();
        items.extend(keys.iter().filter(|k| k.chars().nth(KEY_WINDOW).is_some()).map(|k| (k.chars().take(KEY_WINDOW).collect::<String>(), 1)));
        KeyMaps { lex: Lex::build(items), longest, has_long: longest > KEY_WINDOW }
    }
    /// Whether a key may start at `fc[d]`.
    fn may_start(&self, fc: &[char], d: usize) -> bool {
        fc.get(d).is_some_and(|&c| self.lex.may_start(c))
    }
    /// Whether a key is exactly `t` (or its first KEY_WINDOW characters).
    fn contains(&self, t: &str) -> bool {
        !self.lex.exact(t).is_empty()
    }
}

impl KeyInfo {
    /// The folded information with its lookup maps built once (a bank's keys against every stem).
    pub fn with_maps(self) -> KeyInfo {
        let mut k = self.folded();
        k.maps = Some(std::rc::Rc::new(KeyMaps::new(&k.texts)));
        k
    }
    /// The same information with its texts folded once.
    pub fn folded(mut self) -> KeyInfo {
        if !self.prefolded {
            // (Every text: a question's keys are bounded by its limits, 26 choices, 50 blanks x 20
            // answers, 50 pairs; round 14 read only 64.)
            self.texts = self.texts.iter().map(|t| fold_key(t)).filter(|t| !t.is_empty()).collect();
            self.texts.sort();
            self.texts.dedup();
            self.wrong = self.wrong.iter().map(|t| fold_key(t)).collect();
            self.order_groups = self.order_groups.iter().map(|g| g.iter().map(|t| fold_key(t)).filter(|t| !t.is_empty()).collect()).collect();
            self.prefolded = true;
        }
        self
    }
}

/// Words that introduce a key ("Answer: B", "Ans. 42", "Lösung: Paris", "Respuesta correcta: B"),
/// in about 40 languages; compared after `fold_key`. Qualifiers before them ("Bonne réponse",
/// "Richtige Antwort") need no entry: up to two words may also stand between a lead and its
/// separator ("Respuesta correcta:").
const ANSWER_LEADS: &[&str] = &[
    "answer", "answers", "ans", "key", "solution", "solutions", "sol", "correct", "respuesta", "solución", "réponse", "antwort", "lösung",
    "risposta", "soluzione", "resposta", "solução", "ответ", "решение", "antwoord", "oplossing", "odpowiedź", "rozwiązanie", "svar", "løsning",
    "lösning", "vastaus", "ratkaisu", "válasz", "megoldás", "odpověď", "řešení", "odpoveď", "riešenie", "răspuns", "soluție", "απάντηση",
    "λύση", "clave", "rpta", "resp", "gabarito", "corrigé", "corrige", "trả lời", "feedback", "retroalimentación", "rétroaction", "cevap", "yanıt", "çözüm", "jawaban", "jawapan", "kunci", "đáp án", "lời giải", "sagot", "jibu", "відповідь", "отговор",
    "одговор", "odgovor", "atsakymas", "atbilde", "vastus", "erantzuna", "resposta", "ateb", "freagra", "tweġiba", "përgjigje", "תשובה",
    "פתרון", "إجابة", "الإجابة", "الجواب", "جواب", "پاسخ", "उत्तर", "हल", "উত্তর", "விடை", "సమాధానం", "ಉತ್ತರ", "ഉത്തരം", "คำตอบ", "เฉลย",
    // Round 14 (critic26a): mark schemes.
    "accept", "mark scheme", "indicative content", "acceptable answers", "acceptable answer", "model answer", "examiner",
    // Round 15 (critic28a): hints, rationales, definitions (Quizlet), keywords, Zulu, Amharic.
    "hint", "rationale", "explanation", "definition", "keywords", "keyword", "tip", "impendulo", "\u{1218}\u{120D}\u{1235}",
    // Round 17 (critic31a A6): Punjabi, Sinhala, Burmese, Lao, Pashto, Somali, Maori, Malagasy,
    // Gaelic, Yiddish, Armenian, Georgian, Tajik, Albanian.
    "ਉੱਤਰ", "පිළිතුර", "အဖြေ", "ຄຳຕອບ", "ځواب",
    "jawaab", "whakautu", "valiny", "freagairt", "ענטפֿער", "պատասխան", "პასუხი", "ҷавоб", "përgjigja",
    // (The critic's marker corpus: Punjabi "javab", Kurdish, Quechua, Samoan, Javanese, Kinyarwanda,
    // Shona, Luxembourgish, Breton, Kyrgyz.)
    "ਜਵਾਬ", "bersiv", "kutichiy", "tali", "wangsulan", "igisubizo", "mhinduro", "äntwert", "respont", "жооп",
];
/// Leads of scripts without spaces: found anywhere, no separator needed ("答案是B", "【答案】B").
const ANSWER_LEADS_CJK: &[&str] = &[
    "答", "答案", "答え", "正解", "解答", "정답", "답", "正答", "参考答案", "解析",
    // (Round 17 critic31a A6: scripts written without spaces.)
    "පිළිතුර", "အဖြေ", "ຄຳຕອບ",
];
/// Words that join a lead to its key like a colon ("The answer is B", "La respuesta es B").
const ANSWER_COPULAS: &[&str] = &["is", "are", "sont", "son", "sind", "was", "es", "era", "est", "ist", "war", "è", "é", "e", "è:", "jest", "är", "er", "on", "je", "este", "είναι", "dir", "adalah", "ialah", "là", "ay", "ni", "是", "は", "为", "為", "は、", "이", "가", "는"];
/// Words that may stand before a letter ("Option B", "Choice (b)").
const LETTER_WORDS: &[&str] = &["option", "choice", "letter", "opción", "opcion", "opción", "opção", "opzione", "choix", "variante", "alternativa", "alternative", "antwort", "möglichkeit", "вариант", "seçenek", "pilihan", "lựa", "chọn"];
/// True/False values, any language, after `fold_key`.
const TF_WORDS: &[&str] = &[
    "true", "false", "t", "f", "vrai", "faux", "wahr", "falsch", "richtig", "verdadero", "verdadera", "falso", "falsa", "cierto", "cierta",
    "correcto", "correcta", "incorrecto", "incorrecta", "vero", "sì", "si", "yes", "no", "correct", "incorrect", "right", "wrong", "juist", "patiesi", "үнэн", "fìor",
    "onjuist", "waar", "niet waar", "prawda", "fałsz", "sant", "falskt", "sann", "usann", "rigtigt", "forkert", "tosi", "epätosi", "igaz",
    "hamis", "pravda", "nepravda", "adevărat", "fals", "αληθές", "ψευδές", "doğru", "yanlış", "benar", "salah", "đúng", "sai", "verdadeiro",
    "правда", "ложь", "верно", "неверно", "истина", "正确", "错误", "對", "错", "对", "錯", "正しい", "誤り", "○", "×", "✓", "✔", "✗", "✘", "\u{1F44D}", "\u{1F44E}", "맞음",
    "틀림", "참", "거짓", "صح", "خطأ", "נכון", "לא נכון", "सही", "गलत", "जी हाँ",
    // Round 11: symbols and the letters of about 15 languages' True/False ("V/F", pt "C/E", vi "Đ/S",
    // ko "O/X", tr "D/Y", id "B/S"), Hindi, Arabic, Russian.
    "√", "◯", "〇", "✕", "v", "c", "e", "đ", "s", "o", "x", "d", "y", "b", "सत्य", "असत्य", "صحيح", "صواب", "да", "нет", "certo", "errado",
];
/// Whether `w` names one of the correct positions `letters` ("a" first): a Latin letter, a letter
/// of another label alphabet by position, Persian "الف", or a number 1-26. Any single letter
/// when the positions are unknown.
fn is_key_letter(w: &str, letters: &[char]) -> bool {
    let pos = |v: usize| letters.contains(&((b'a' + (v - 1).min(25) as u8) as char));
    let mut cs = w.chars();
    match (cs.next(), cs.next()) {
        (Some(c), None) if c.is_ascii_digit() => c != '0' && (letters.is_empty() || pos(c as usize - '0' as usize)),
        (Some(c), None) if letters.is_empty() => c.fast_alpha(),
        (Some(c), None) => letters.contains(&c) || letter_position(c).is_some_and(pos),
        _ if w == "\u{627}\u{644}\u{641}" => letters.is_empty() || pos(1),
        _ => w.parse::<usize>().is_ok_and(|v| (1..=26).contains(&v) && (letters.is_empty() || pos(v))),
    }
}

/// The position of a choice letter of another alphabet ("б" is the second, "γ" the third).
fn letter_position(c: char) -> Option<usize> {
    // Cyrillic, Greek, Devanagari (क ख ग घ), Bengali, Arabic (ا ب ج د, abjad order), Hebrew,
    // Japanese iroha and aiueo, Korean, Chinese heavenly stems and numerals.
    const ALPHABETS: [&str; 11] = [
        "абвгдежзийклмнопрстуфхцчшщъыьэюя",
        "αβγδεζηθικλμνξοπρστυφχψω",
        "\u{915}\u{916}\u{917}\u{918}\u{919}\u{91A}\u{91B}\u{91C}\u{91D}\u{91E}",
        "\u{995}\u{996}\u{997}\u{998}\u{999}\u{99A}\u{99B}\u{99C}\u{99D}\u{99E}",
        "\u{627}\u{628}\u{62C}\u{62F}\u{647}\u{648}\u{632}\u{62D}\u{637}\u{64A}",
        "\u{5D0}\u{5D1}\u{5D2}\u{5D3}\u{5D4}\u{5D5}\u{5D6}\u{5D7}\u{5D8}\u{5D9}",
        "\u{30A4}\u{30ED}\u{30CF}\u{30CB}\u{30DB}\u{30D8}\u{30C8}",
        "\u{30A2}\u{30A4}\u{30A6}\u{30A8}\u{30AA}\u{30AB}\u{30AD}\u{30AF}",
        "\u{AC00}\u{B098}\u{B2E4}\u{B77C}\u{B9C8}\u{BC14}\u{C0AC}\u{C544}",
        "\u{7532}\u{4E59}\u{4E19}\u{4E01}\u{620A}\u{5DF1}\u{5E9A}\u{8F9B}",
        "\u{4E00}\u{4E8C}\u{4E09}\u{56DB}\u{4E94}\u{516D}\u{4E03}\u{516B}",
    ];
    ALPHABETS.iter().find_map(|a| a.chars().position(|x| x == c)).map(|p| p + 1)
}

/// Whether `text` is exactly a True/False word naming this True/False question's value (a topic
/// "True").
pub fn tf_value_word(text: &str, key: &KeyInfo) -> bool {
    let w = fold_key(text);
    let w = w.trim();
    key.tf && folded_set(4).contains(w) && !folded_set(5).contains(w) && key.tf_value.is_none_or(|v| true_set().contains(w) == v)
}

/// The True values of `TF_WORDS` (the rest are False).
const TRUE_WORDS: &[&str] = &[
    "true", "t", "vrai", "v", "wahr", "w", "richtig", "verdadero", "verdadera", "cierto", "cierta", "correcto", "correcta", "vero", "sì", "si", "yes",
    "correct", "right", "juist", "waar", "prawda", "sant", "sann", "rigtigt", "tosi", "igaz", "pravda", "adevărat", "αληθές", "doğru", "benar", "đúng",
    "verdadeiro", "правда", "верно", "истина", "正确", "對", "对", "正しい", "○", "✓", "✔", "맞음", "참", "صح", "נכון", "सही", "जी हाँ", "r", "đ", "o",
    "√", "◯", "〇", "c", "d", "b", "सत्य", "صحيح", "صواب", "да", "certo", "\u{1F44D}",
];

/// TF words that are only values inside braces or after an answer lead ("{T}", "Ans. T"), not as
/// a bracketed annotation ("temperature (T)").
const TF_SHORT: &[&str] = &["t", "f", "si", "no", "e", "o", "x", "v", "w", "c", "s", "d", "y", "b", "đ", "r"];

/// The form the key checks read: the served text, NFKC (fullwidth "ＡＮＳＷＥＲ：" is "ANSWER:"),
/// combining marks dropped ("Answér"), lowercase, Cyrillic and Greek look-alikes as Latin
/// ("Аnswer"), every space as " " (lines kept), dashes as "-", arrows as ">", typographic quotes
/// as ASCII.
pub fn fold_key(s: &str) -> String {
    fold_served(&served_text(s))
}

/// `fold_key` of a text already in `served_text` form.
fn fold_served(served: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    let mut out = String::with_capacity(served.len());
    // (Hangul syllables stay composed: their jamo are slow to classify and carry no marks.)
    let mut pieces: Vec<char> = Vec::with_capacity(served.len());
    for c in served.chars() {
        // (Compatibility underscores "＿", "﹍" stay themselves: a blank is ASCII "___", as validation
        // counts it.)
        if c.is_ascii()
            || ('\u{AC00}'..='\u{D7A3}').contains(&c)
            // (Ethiopic has no decomposition, case or marks: a repeated Amharic lead folds at
            // one push per character, not through the NFKD tables. Round 17, critic31b B3.)
            || matches!(c as u32, 0x1200..=0x139F | 0x2D80..=0x2DDF | 0xAB00..=0xAB2F)
            || matches!(c, '\u{FF3F}' | '\u{FE4D}' | '\u{FE4E}' | '\u{FE4F}' | '\u{FE33}' | '\u{FE34}') {
            pieces.push(c);
        } else {
            // (A long compatibility expansion, "ﷺ" → 18 letters, is no letter of a lead: kept as
            // one character, so a repeated one costs no more than any other.)
            let start = pieces.len();
            pieces.extend(std::iter::once(c).nfkd());
            if pieces.len() - start > 4 {
                pieces.truncate(start);
                pieces.push(c);
            }
        }
    }
    for c0 in pieces {
        // (Invisible characters: nothing a scan reads.)
        if !c0.is_ascii() && is_default_ignorable(c0) {
            continue;
        }
        // (Hangul syllables and CJK ideographs have no case, mark or look-alike: as they are,
        // without a case-table search each.)
        if ('\u{AC00}'..='\u{D7A3}').contains(&c0) || ('\u{4E00}'..='\u{9FFF}').contains(&c0) || matches!(c0 as u32, 0x0621..=0x063A | 0x0641..=0x064A | 0x0E01..=0x0E30 | 0x0E32..=0x0E33 | 0x0E40..=0x0E46 | 0x10D0..=0x10FA | 0x1200..=0x139F | 0x2D80..=0x2DDF | 0xAB00..=0xAB2F) {
            out.push(c0);
            continue;
        }
        if !c0.is_ascii() && matches!(get_general_category(c0), G::NonspacingMark | G::EnclosingMark) {
            continue;
        }
        // (Lowercase first: "Відповідь" and "відповідь" fold alike.)
        for c in c0.to_lowercase() {
        let m = match c {
            '\n' => '\n',
            c if c.is_whitespace() => ' ',
            '\u{2010}'..='\u{2015}' | '\u{2212}' | '\u{FE58}' | '\u{FE63}' | '\u{FF0D}' | '\u{2E3A}' | '\u{2E3B}' => '-',
            c if is_arrow(c) => '>',
            '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' | '\u{2032}' => '\'',
            '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' | '\u{2033}' | '\u{AB}' | '\u{BB}' => '"',
            '\u{430}' | '\u{410}' | '\u{3B1}' | '\u{391}' => 'a',
            '\u{435}' | '\u{415}' | '\u{3B5}' | '\u{395}' => 'e',
            '\u{43E}' | '\u{41E}' | '\u{3BF}' | '\u{39F}' => 'o',
            '\u{440}' | '\u{420}' | '\u{3C1}' | '\u{3A1}' => 'p',
            '\u{441}' | '\u{421}' => 'c',
            '\u{445}' | '\u{425}' | '\u{3C7}' | '\u{3A7}' => 'x',
            '\u{443}' | '\u{423}' => 'y',
            '\u{456}' | '\u{406}' | '\u{3B9}' | '\u{399}' => 'i',
            '\u{458}' | '\u{408}' => 'j',
            '\u{455}' | '\u{405}' => 's',
            '\u{41D}' | '\u{397}' => 'h',
            '\u{41A}' | '\u{39A}' => 'k',
            '\u{41C}' | '\u{39C}' => 'm',
            '\u{422}' | '\u{3A4}' => 't',
            '\u{412}' | '\u{392}' => 'b',
            '\u{39D}' => 'n',
            '\u{396}' => 'z',
            c => c,
        };
        out.push(m);
        }
    }
    out
}

fn folded_list(which: usize) -> &'static [String] {
    use std::sync::OnceLock;
    static L: OnceLock<[Vec<String>; 7]> = OnceLock::new();
    let l = L.get_or_init(|| {
        let f = |v: &[&str]| v.iter().map(|w| fold_key(w)).collect::<Vec<_>>();
        [f(ANSWER_LEADS), f(ANSWER_LEADS_CJK), f(ANSWER_COPULAS), f(LETTER_WORDS), f(TF_WORDS), f(TF_SHORT), f(KEY_WORDS)]
    });
    &l[which]
}

/// `folded_list(which)` as a set, and the first characters of the leads (`which` 0), built once.
fn folded_set(which: usize) -> &'static std::collections::HashSet<String> {
    use std::sync::OnceLock;
    static S: OnceLock<Vec<std::collections::HashSet<String>>> = OnceLock::new();
    &S.get_or_init(|| (0..7).map(|w| folded_list(w).iter().cloned().collect()).collect())[which]
}
/// The True/False values and "Option" words by first character, as character vectors.
fn tf_letter_by_first() -> &'static std::collections::HashMap<char, Vec<Vec<char>>> {
    use std::sync::OnceLock;
    static S: OnceLock<std::collections::HashMap<char, Vec<Vec<char>>>> = OnceLock::new();
    S.get_or_init(|| {
        let mut m: std::collections::HashMap<char, Vec<Vec<char>>> = std::collections::HashMap::new();
        for w in folded_list(4).iter().chain(folded_list(3)) {
            if let Some(c) = w.chars().next() {
                m.entry(c).or_default().push(w.chars().collect());
            }
        }
        m
    })
}
/// Whether one of `words` (grouped by first character) starts at `fc[d..]`.
fn starts_any(fc: &[char], d: usize, words: &std::collections::HashMap<char, Vec<Vec<char>>>) -> bool {
    fc.get(d).and_then(|c| words.get(c)).is_some_and(|ws| ws.iter().any(|w| fc.len() >= d + w.len() && fc[d..d + w.len()] == w[..]))
}
fn true_set() -> &'static std::collections::HashSet<String> {
    use std::sync::OnceLock;
    static S: OnceLock<std::collections::HashSet<String>> = OnceLock::new();
    S.get_or_init(|| TRUE_WORDS.iter().map(|w| fold_key(w)).collect())
}
fn copulas_chars() -> &'static [Vec<char>] {
    use std::sync::OnceLock;
    static S: OnceLock<Vec<Vec<char>>> = OnceLock::new();
    S.get_or_init(|| folded_list(2).iter().filter(|c| !c.is_empty()).map(|c| c.chars().collect()).collect())
}
fn copula_firsts() -> &'static std::collections::HashSet<char> {
    use std::sync::OnceLock;
    static S: OnceLock<std::collections::HashSet<char>> = OnceLock::new();
    S.get_or_init(|| folded_list(2).iter().filter_map(|l| l.chars().next()).collect())
}
/// The spaced answer leads by first character: (the lead, its characters).
type LeadsByFirst = std::collections::HashMap<char, Vec<(&'static String, Vec<char>)>>;
fn leads_by_first() -> &'static LeadsByFirst {
    use std::sync::OnceLock;
    static S: OnceLock<LeadsByFirst> = OnceLock::new();
    S.get_or_init(|| {
        let mut m: LeadsByFirst = std::collections::HashMap::new();
        for l in folded_list(0) {
            if let Some(c) = l.chars().next() {
                m.entry(c).or_default().push((l, l.chars().collect()));
            }
        }
        m
    })
}
/// The leads of scripts without spaces by first character, built once: one pass over the text
/// finds every lead (the loop over the leads scanned the text once per lead, round 17 critic31b
/// B3).
fn cjk_leads_by_first() -> &'static LeadsByFirst {
    use std::sync::OnceLock;
    static S: OnceLock<LeadsByFirst> = OnceLock::new();
    S.get_or_init(|| {
        let mut m: LeadsByFirst = std::collections::HashMap::new();
        for l in folded_list(1) {
            if let Some(c) = l.chars().next() {
                m.entry(c).or_default().push((l, l.chars().collect()));
            }
        }
        m
    })
}

/// Whether a text may hold an answer line at all: a cheap necessary condition for the bank's
/// cross-question scan. True for cloze syntax, any non-ASCII character (folding can produce a
/// lead from look-alikes), or an ASCII word at a word start that is the start of a spaced answer
/// lead (compared in place, one map lookup per word of two characters or more). A text that fails
/// it has no lead, so `stem_key` in its `lines_only` form can only return None (round 17: a bank
/// of 26,000 choices full of "a" cost 4.4 s through the full scan).
pub fn may_have_answer_line(t: &str) -> bool {
    if t.contains("{{") || !t.is_ascii() {
        return true;
    }
    // (The scan reads the served, folded text, where every run of spaces or tabs is one space:
    // a lead typed with a tab or two spaces ("mark  scheme:", "mark\tscheme:") folds to the
    // lead, so such a text must not turn the scan off. Round 18, critic34b B1: a text with a
    // tab or a run of two spaces takes the full scan, whatever the lead looks like here.)
    if t.as_bytes().contains(&b'\t') || t.contains("  ") {
        return true;
    }
    let b = t.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_alphanumeric() {
            let s = i;
            while i < b.len() && b[i].is_ascii_alphanumeric() {
                i += 1;
            }
            // (A one-letter word is no lead: every spaced lead has two characters or more.)
            if i - s < 2 {
                continue;
            }
            let Some(cands) = leads_by_first().get(&(b[s].to_ascii_lowercase() as char)) else {
                continue;
            };
            for (lead, _) in cands {
                let lb = lead.as_bytes();
                // (The lead loop matches a lead followed by a non-alnum; the raw ASCII text folds
                // to itself but for case.)
                if b.len() >= s + lb.len() && b[s..s + lb.len()].eq_ignore_ascii_case(lb) && !b.get(s + lb.len()).is_some_and(|c| c.is_ascii_alphanumeric()) {
                    return true;
                }
            }
        } else {
            i += 1;
        }
    }
    false
}

/// A key given away by a served text: Aiken's "ANSWER: B", GIFT answers ("{=yellow ~red}",
/// "{TRUE}", "star{T}", "{#3.14}"), Cloze ("{1:SHORTANSWER:%100%Paris}", any case and spacing),
/// a True/False statement ending in its value ("(True)", "（ＴＲＵＥ）", "(Richtig)", "- True"), a
/// stem ending in a key text in brackets ("(Paris)"), or an answer lead ("Answer:", "Ans.",
/// "Respuesta correcta:", "The answer is", "Ответ —", "答案是", "【答案】") followed by a choice
/// letter ("B", "(b)", "‘B’", "**B**", "Option B"), True/False, or a key text. Read on
/// `fold_key`. The reason, or None. Linear: each "{" reads at most 88 characters, each lead 60.
pub fn stem_key(text: &str, key: &KeyInfo) -> Option<&'static str> {
    // (The publish's work budget already spent: refused at once, not after the reading. Round
    // 17, critic31b C3.)
    if budget_spent() {
        return Some("scan_budget");
    }
    // (The served text once per call, the folded form from it.)
    let served_once = served_text(text);
    let f = fold_served(&served_once);
    let fc: Vec<char> = f.chars().collect();
    let n = fc.len();
    let tf_word = |w: &str| folded_set(4).contains(w);
    let tf_short = |w: &str| folded_set(5).contains(w);
    let keys: std::borrow::Cow<[String]> = if key.prefolded { std::borrow::Cow::Borrowed(&key.texts[..]) } else { std::borrow::Cow::Owned(key.texts.iter().map(|k| fold_key(k)).filter(|k| !k.is_empty()).collect()) };
    let last_close = fc.iter().rposition(|c| *c == '}');
    // Round 15 (critic28a B1): Anki cloze, "{{c1::Mitochondria}}", "{{c1::x::hint}}", and the
    // field references "{{type:...}}", "{{cloze:...}}": answer syntax wherever it is served, like
    // Moodle's Cloze. High risk, whatever the key, keyless too.
    if anki_cloze(text) {
        return Some("cloze");
    }
    // Round 13 (critic24a B1): a LaTeX quiz environment pasted into the stem with its key command
    // ("\CorrectChoice True \choice False", AMC "\bonne{}", moodle.sty "\item*").
    if !key.lines_only && f.contains('\\') {
        let marks = ["\\correctchoice", "\\correctchoices", "\\bonne", "\\mauvaise", "\\wrongchoice", "\\begin{solution}", "\\begin{solutionordottedlines}", "\\ifprintanswers", "\\answerline[", "\\fillin[", "\\answer{", "\\begin{solutionorbox}"];
        if marks.iter().any(|m| f.contains(m)) || (f.contains("\\item*") && f.contains("\\item ")) {
            return Some("latex_key");
        }
    }
    // Braces.
    for i in (0..n).filter(|&i| fc[i] == '{' && !key.lines_only) {
        // LaTeX: "x^{T}", "a_{1}", "\mathbf{F}", "\frac{1}{x}", "$...$".
        let prev = if i > 0 { Some(fc[i - 1]) } else { None };
        if prev.is_some_and(|c| matches!(c, '^' | '_' | '$' | '}' | '\\')) {
            continue;
        }
        let run = fc[..i].iter().rev().take_while(|c| c.is_ascii_alphabetic()).count();
        if run > 0 && i > run && fc[i - run - 1] == '\\' {
            continue;
        }
        let mut j = i + 1;
        while j < n && j < i + 65 && (fc[j] == ' ' || fc[j] == '\n') {
            j += 1;
        }
        let head: String = fc[j..n.min(j + 24)].iter().collect();
        // Cloze: "{1:SHORTANSWER:", "{ 1 :sa:", "{:MC:".
        {
            let h = head.trim_start_matches(|c: char| c.is_ascii_digit() || c == ' ');
            if let Some(r) = h.strip_prefix(':') {
                let w: String = r.chars().take_while(|c| c.is_ascii_alphabetic() || *c == '_').collect();
                if w.len() >= 2 && r[w.len()..].trim_start().starts_with(':') {
                    return Some("cloze");
                }
            }
        }
        // Round 19 (critic35a B2): GIFT's True/False with feedback, "{TRUE#well done}",
        // "{FALSE#Not quite}", "{T#good}", "{TRUE#if wrong#if right}" (Moodle writes general
        // feedback after the value): the value, then spaces, then "#", with a closing brace
        // somewhere after. Read like the bare form.
        if last_close.is_some_and(|z| z > i) {
            let wl = fc[j..].iter().take(6).take_while(|c| c.is_ascii_alphabetic()).count();
            if wl > 0 && wl <= 5 {
                let w: String = fc[j..j + wl].iter().collect();
                if matches!(w.as_str(), "true" | "false" | "t" | "f") {
                    let mut k = j + wl;
                    while k < n && matches!(fc[k], ' ' | '\n') {
                        k += 1;
                    }
                    if k < n && fc[k] == '#' {
                        return Some("gift");
                    }
                }
            }
        }
        if let Some(e) = head.find('}') {
            let w = head[..e].trim();
            if w == "true" || w == "false" || w == "t" || w == "f" {
                return Some("gift");
            }
        }
        if last_close.is_some_and(|z| z > i) {
            let mut hc = head.chars();
            match hc.next() {
                Some('=' | '~') if hc.as_str().trim_start().starts_with(|c: char| c.fast_alnum() || matches!(c, '%' | '-' | '(' | '"' | '\'')) => return Some("gift"),
                Some('#') if hc.as_str().trim_start().trim_start_matches('=').trim_start().starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '.') => return Some("gift"),
                _ => {}
            }
        }
    }
    // The last bracketed group: a True/False value, or (any kind) a key text.
    let trimmed = f.trim_end().trim_end_matches(['.', '!', '?']).trim_end();
    // (A bracket glued to a word is notation, "f(2)": round 15.)
    if !key.lines_only
        && let Some(inner) = trimmed.strip_suffix([')', ']']).and_then(|t| t.rsplit_once(['(', '['])).filter(|x| !x.0.ends_with(|c: char| c.fast_alnum())).map(|x| x.1.trim())
    {
        if key.tf && tf_word(inner) && !tf_short(inner) {
            return Some("tf_value");
        }
        // ("What is d/dx (eˣ)?" asks about the bracket.)
        if keys.iter().any(|k| k == inner) && !f.trim_end().ends_with(['?', '\u{FF1F}']) {
            return Some("answer_line");
        }
    }
    // "... - True", "... : vrai" at the end of a True/False statement.
    // Round 14 (critic26a): a worksheet's answer blank, both values before the statement ("T   F
    // Larches are conifers.", "V  F ...", "True / False ..."): it shows both, so it names
    // neither; the statement after it is read as the stem.
    // (Once: a run of pairs, "x ✓ x ✓ ...", is no answer blank and is read as it is, and the rest
    // after one pair has none, so it is never stripped again. Round 14: re-stripping was quadratic.)
    if key.tf
        && !key.lines_only
        && let Some(rest) = tf_pair_prefix(&served_once)
        && tf_pair_prefix(rest).is_none()
    {
        return stem_key(rest, key);
    }
    if key.tf && !key.lines_only {
        // (Once, not per separator.)
        let orig_tail = served_once.trim_end().trim_end_matches(['.', '!']).rsplit(['-', ':', '>', '=', '\u{2192}', '\u{2013}', '\u{2014}']).next().unwrap_or("").trim().to_string();
        for sep in ['-', ':', '>', '='] {
            // (A bare letter only after an arrow or "=": "... → F", "... = F".)
            // (A capital letter alone after any of them in the original: ". - T".)
            if let Some((_, w)) = trimmed.rsplit_once(sep)
                && tf_word(w.trim())
                && (!tf_short(w.trim()) || matches!(sep, '>' | '=') || (orig_tail.chars().count() == 1 && orig_tail.chars().all(char::is_uppercase)))
                && w.len() <= 16
            {
                return Some("tf_value");
            }
        }
    }
    // A True/False question written with lettered options ("A) True\nB) False\nANSWER: A"):
    // letters name its values.
    let tf_options = key.tf && f.lines().any(|l| {
        let l = l.trim();
        let mut cs = l.chars();
        cs.next().is_some_and(|c| c.fast_alpha() || c.is_ascii_digit()) && matches!(cs.next(), Some(')' | '.' | ':')) && tf_word(cs.as_str().trim().trim_end_matches('.'))
    });
    let choices = key.choices || tf_options;
    // Whether a True/False word names the question's value (any value when it is unknown).
    let tf_matches = |w: &str| tf_word(w) && key.tf_value.is_none_or(|v| true_set().contains(w) == v);
    // The end of a True/False statement: "(F)" after its full stop, a check or a cross mark; a
    // bare or wrapped value after the statement's last full stop ("... star. TRUE", "[T]", "*T",
    // "→ F", "= F"); a bracket that opens with a value and explains it ("(FALSE – it reflects
    // light)"); a single value letter opening the statement ("T The Sun is a star").
    if key.tf && !key.lines_only {
        let t = f.trim_end();
        // ("(This statement is true.)": a trailing bracket that states the value last.)
        if let Some(inner) = t.strip_suffix(')').and_then(|x| x.rsplit_once('(')).map(|x| x.1.trim().trim_end_matches(['.', '!']))
            && (inner.ends_with(" is true") || inner.ends_with(" is false"))
        {
            return Some("tf_value");
        }
        if let Some(p) = t.trim_end_matches(['.', '!']).rfind(['.', '!', '?', '\u{3002}']) {
            let after = p + t[p..].chars().next().map_or(1, char::len_utf8);
            let tok = t[after..].trim().trim_matches(|c: char| matches!(c, '[' | ']' | '(' | ')' | '*' | '>' | '=' | ' ' | '.' | '!' | '_'));
            if !tok.is_empty() && !tok.contains(' ') && tf_word(tok) {
                return Some("tf_value");
            }
        }
        if let Some(inner) = t.strip_suffix(')').and_then(|x| x.rsplit_once('(')).map(|x| x.1.trim()) {
            let first: String = inner.chars().take_while(|c| is_alnum(*c)).collect();
            let rest = inner[first.len()..].trim_start();
            if !first.is_empty() && !tf_short(&first) && tf_word(&first) && rest.starts_with(['-', ',', ':', ';']) {
                return Some("tf_value");
            }
        }
        // Round 11, B7: a bracketed token of up to three capital letters or one symbol ending the
        // statement ("(V)", "(Đ)", "（√）", "(Да)", "(सत्य)"), a value alone on the last line
        // ("...ocean\nFalse"), a value opening it with a delimiter ("TRUE: ...", "True - ...", "T. ...").
        let served = &served_once;
        let st = served.trim_end().trim_end_matches(['.', '!']);
        if let Some(z) = st.chars().last()
            && matches!(z, ')' | ']' | '\u{FF09}')
            && let Some((_, inner)) = st[..st.len() - z.len_utf8()].rsplit_once(['(', '[', '\u{FF08}'])
        {
            let inner = inner.trim();
            let n = inner.chars().count();
            // (A value is short: one fold of at most 24 characters.)
            let w = if n <= 24 { fold_key(inner) } else { String::new() };
            let w = w.trim();
            let caps = (1..=3).contains(&n) && inner.chars().all(|c| c.is_uppercase() || (c.fast_alpha() && !c.is_lowercase()));
            let symbol = n == 1 && tf_word(w);
            // (Either value: a True/False question has one, and a marker at its edge is not part of
            // the statement.)
            if (caps || symbol || (tf_word(w) && !tf_short(w))) && tf_word(w) {
                return Some("tf_value");
            }
        }
        // A True/False symbol ending the statement ("... Planet 👍", "... ✓"): round 15.
        if let Some(last) = served.split_whitespace().last()
            && !last.chars().any(crate::format::is_alnum)
            && served.split_whitespace().count() >= 2
            && tf_word(fold_key(last.trim_end_matches(['.', '!'])).trim())
        {
            return Some("tf_value");
        }
        // Both values on lines of their own at the end ("Select one:\nTrue\nFalse", a pasted
        // preview): the order names nothing, but it is no part of the statement either. Low risk
        // (round 12).
        {
            let tail: Vec<&str> = served.lines().map(str::trim).filter(|l| !l.is_empty()).rev().take(2).collect();
            if tail.len() == 2 && tail.iter().all(|l| l.len() <= 32) {
                let (a, b) = (fold_key(tail[0].trim_end_matches(['.', '!'])), fold_key(tail[1].trim_end_matches(['.', '!'])));
                if tf_word(&a) && tf_word(&b) && !tf_short(&a) && !tf_short(&b) && true_set().contains(&a) != true_set().contains(&b) {
                    return Some("choices_pasted");
                }
            }
        }
        if let Some(last) = served.lines().rfind(|l| !l.trim().is_empty())
            && last.len() <= 64
            && served.lines().filter(|l| !l.trim().is_empty()).count() >= 2
        {
            let w = fold_key(last.trim().trim_end_matches(['.', '!']));
            if tf_word(w.trim()) {
                return Some("tf_value");
            }
        }
        {
            let mut it = served.split_whitespace();
            if let (Some(a), Some(b)) = (it.next(), it.next()) {
                let core = a.trim_end_matches(['.', ':', '-', ',']);
                let delimited = core.len() < a.len() || matches!(b, "-" | "\u{2013}" | "\u{2014}" | ":");
                let initials = b.chars().count() == 2 && b.ends_with('.');
                let w = if core.len() <= 32 { fold_key(core) } else { String::new() };
                if delimited && !initials && tf_word(w.trim()) && (!tf_short(w.trim()) || core.chars().all(char::is_uppercase)) {
                    return Some("tf_value");
                }
            }
        }
        let orig = text.trim_start();
        let mut toks = orig.split_whitespace();
        if let (Some(a), Some(b)) = (toks.next(), toks.next()) {
            let a = a.trim_matches(|c: char| matches!(c, '[' | ']' | '(' | ')' | '*'));
            if matches!(a, "T" | "F" | "V" | "R" | "X" | "\u{110}" | "\u{2713}" | "\u{2717}" | "\u{221A}" | "\u{D7}") && b.chars().next().is_some_and(char::is_uppercase) && tf_word(&fold_key(a)) {
                return Some("tf_value");
            }
        }
        if t.ends_with(['\u{2713}', '\u{2714}', '\u{2717}', '\u{2718}', '\u{2705}', '\u{274C}', '\u{2611}', '\u{2612}']) && t.chars().filter(|c| c.fast_alpha()).count() >= 3 {
            return Some("tf_value");
        }
        if let Some(inner) = t.strip_suffix(')').and_then(|x| x.rsplit_once('(')).filter(|(before, _)| before.trim_end().ends_with(['.', '!', '?'])).map(|x| x.1.trim())
            && tf_word(inner)
        {
            return Some("tf_value");
        }
    }
    // Round 11, B5: a list of choices pasted into the stem with one line marked ("A. Oxygen /
    // B. Nitrogen ✓ / C. Argon", "*b) Nitrogen", "**B. Nitrogen**", "Nitrogen (correct)"): two or
    // more labelled or bulleted lines, exactly one of them carrying a high-risk mark.
    if !key.lines_only {
        let served = &served_once;
        // (Each line read in its first 2 KB, a choice's limit: a longer line is a choice with
        // more after it, and its mark is in that start. Round 14: longer lines were skipped.)
        let cut = |l: &str| -> usize { let mut c = l.len().min(2048); while !l.is_char_boundary(c) { c -= 1; } c };
        let cands: Vec<&str> = served.lines().map(str::trim).map(|l| &l[..cut(l)]).filter(|l| {
            let l = l.trim_start_matches(['*', '\u{2022}', '-', '\u{2713}', '\u{2714}', ' ']);
            crate::labels::detect(l).is_some_and(|h| h.start == 0 && crate::text::has_visible(&l[h.end..]))
        }).collect();
        if cands.len() >= 2 {
            let marked = |l: &str| l.starts_with(['*', '\u{2713}', '\u{2714}']) || features(l).iter().any(|f| f.risk == Risk::High && marker_kind(f));
            let n = cands.iter().filter(|l| marked(l)).count();
            if n == 1 {
                return Some("marked_list");
            }
        }
    }
    // B6: with an order students see, a bare or marked letter naming a correct position at the
    // stem's start or end or on a line of its own ("[B]", "✓B", "B ✓", "→ B", "**B**", "{B}").
    if choices && key.positional && !key.letters.is_empty() {
        let deco = |c: char| matches!(c, '[' | ']' | '(' | ')' | '{' | '}' | '*' | '\u{2713}' | '\u{2714}' | '>' | ':' | '-' | '\u{2022}' | '=' | '_' | ' ' | '.');
        let named = |tok: &str| {
            let core = tok.trim_matches(deco);
            core.chars().count() == 1 && core.chars().all(crate::format::is_alpha) && is_key_letter(core, &key.letters) && (core.len() < tok.trim().len() || true)
        };
        let ws: Vec<&str> = f.split_whitespace().collect();
        // (Decorated: "vitamin A" ending a stem is content.)
        let decorated = |w: &str| w.chars().any(|c| deco(c) && c != ' ' && c != '.');
        let edge_hit = ws.last().is_some_and(|w| decorated(w) && named(w))
            || (ws.len() >= 2 && ws[ws.len() - 2].chars().all(|c| deco(c) || matches!(c, '\u{2713}' | '\u{2714}')) && named(ws[ws.len() - 1]))
            || (ws.len() >= 2 && named(ws[ws.len() - 2]) && ws[ws.len() - 1].chars().all(|c| matches!(c, '\u{2713}' | '\u{2714}')))
            // (A stem's own label, "(a) How long...", "a) Which...", "[a]": brackets or a full
            // stop alone before more words number the stem; a mark, "✓B", "*B*", names a choice.
            // Round 13.)
            || ws.first().is_some_and(|w| decorated(w) && named(w) && (ws.len() == 1 || w.chars().any(|c| deco(c) && !matches!(c, '(' | ')' | '[' | ']' | '.' | ' '))));
        let line_hit = f.lines().skip(1).any(|l| named(l) && l.trim().chars().count() <= 6);
        if edge_hit || line_hit {
            return Some("answer_line");
        }
    }
    // A choice letter in brackets ending the stem ("Which planet is red? (B)").
    if choices
        && !key.letters.is_empty()
        && let Some(inner) = f.trim_end().strip_suffix(')').and_then(|x| x.rsplit_once('(')).filter(|x| !x.0.ends_with(|c: char| c.fast_alnum())).map(|x| x.1.trim())
        && inner.chars().count() <= 3
        && is_key_letter(inner, &key.letters)
    {
        return Some("answer_line");
    }
    // A clause that pairs a key with a verdict word ("(Nairobi is correct)", "(A is correct)",
    // "Nairobi — richtig"): a line or bracket group of at most eight words, no question, that
    // starts or ends with a key text (or a choice letter, a True/False value) and holds an answer
    // or key word of any language.
    if !key.lines_only {
        let verdicts = |w: &str| (folded_set(0).contains(w) || folded_set(6).contains(w)) && !matches!(w, "key" | "kunci");
        // (Computed once: per clause it would be quadratic in "....".)
        let whole_stem = f.trim().trim_end_matches(['.', '!']);
        // (Also a short text as a whole, its brackets read as words: the topic "Mars (correct)".)
        let short_whole: Option<String> = (f.len() <= 120 && !f.contains('\n')).then(|| f.replace(['(', ')', '[', ']', '\u{FF08}', '\u{FF09}'], " "));
        for clause in f.split(['\n', '(', ')', '[', ']', ';', '\u{FF08}', '\u{FF09}', '.', '\u{3002}']).chain(short_whole.as_deref()) {
            let c = clause.trim().trim_end_matches(['.', '!']);
            // (A stem that is only this clause states it: "The answer is true.")
            if c == whole_stem {
                continue;
            }
            let words: Vec<&str> = c.split(|ch: char| !ch.fast_alnum()).filter(|w| !w.is_empty()).collect();
            if words.len() < 2 || words.len() > 6 || c.contains('?') {
                continue;
            }
            let first = words[0];
            let last = words[words.len() - 1];
            // (The question's own key: a correct choice's letter, its True/False value; and not a
            // list of alternatives, "Answer A, B, C or D", "true or false".)
            let alternatives = words.iter().any(|w| matches!(*w, "or" | "o" | "ou" | "oder" | "или" | "atau" | "veya"));
            let is_key_end = |w: &str, at_start: bool| {
                !alternatives
                    && ((choices && w.chars().all(crate::format::is_alpha) && w.chars().count() <= 3 && is_key_letter(w, &key.letters))
                        || (key.tf && tf_word(w) && !tf_short(w) && tf_matches(w))
                        || keys.iter().any(|k| if at_start { c.starts_with(k.as_str()) } else { c.ends_with(k.as_str()) }))
            };
            let verdict_inside = words.iter().any(|w| verdicts(w));
            if verdict_inside && (is_key_end(first, true) && !verdicts(first) || is_key_end(last, false) && !verdicts(last)) {
                return Some("answer_line");
            }
        }
    }
    // A word bank in the order of the blanks (or of a fixed Matching order) is the key: stem text
    // is never shuffled (round 12, critic23a B2). In another order it is allowed.
    let (bank_verdict, bank_ranges) = if key.lines_only { (None, Vec::new()) } else { bank_order(&f, &key.order_groups) };
    if bank_verdict == Some("word_bank_order") {
        return bank_verdict;
    }
    // Round 15 (critic28a): an Anki or spreadsheet row, the key after a tab ("What is the capital
    // of Peru?\tLima").
    if !key.lines_only
        && let Some((head, tail)) = text.lines().rfind(|l| l.contains('\t')).and_then(|l| l.rsplit_once('\t'))
        && head.chars().any(crate::format::is_alnum)
        && keys.contains(&fold_key(tail.trim()))
    {
        return Some("answer_line");
    }
    // Round 15 (critic28a): the stem ending in a key text with a decoration: "**Mars**", "{Mars}",
    // "'Mars'", "→ Mars", "= Mars" (a bare one after a question mark is the next rule's).
    if !key.lines_only && !keys.is_empty() {
        let t = f.trim_end().trim_end_matches(['.', '!', ' ']);
        let closing = ['*', '}', '\'', '"', ']', '_', ')'];
        let core = t.trim_end_matches(closing);
        let closed = core.len() < t.len();
        for k in keys.iter().filter(|k| !k.is_empty()) {
            if let Some(head) = core.strip_suffix(k.as_str())
                && !head.ends_with(|c: char| c.fast_alnum())
            {
                let h = head.trim_end_matches(' ');
                // (A bracket glued to a word is notation, "f(2)".)
                let glued = h.ends_with(['(', '[']) && h[..h.len() - 1].ends_with(|c: char| c.fast_alnum());
                let opened = h.ends_with(['*', '{', '\'', '"', '[', '_', '>', '=', '(']) && !glued;
                if opened && (closed || h.ends_with(['>', '='])) && h.trim_end_matches(['*', '{', '\'', '"', '[', '_', '>', '=', '(', ' ']).chars().any(crate::format::is_alnum) {
                    return Some("answer_line");
                }
            }
        }
    }
    // A line of its own that is a correct choice, when no line is a wrong one (not a word bank's
    // line).
    if !key.lines_only && bank_ranges.is_empty() && f.lines().count() >= 2 {
        // (Also after an arrow or a bullet: "→ Dovetail joint", round 12.)
        let lines: Vec<&str> = f.lines().skip(1).map(|l| l.trim().trim_start_matches(['>', '-', '*', '\u{2022}', '=']).trim().trim_end_matches('.')).collect();
        let wrongs: std::borrow::Cow<[String]> = if key.prefolded { std::borrow::Cow::Borrowed(&key.wrong[..]) } else { std::borrow::Cow::Owned(key.wrong.iter().map(|w| fold_key(w)).collect()) };
        if lines.iter().any(|l| keys.iter().any(|k| k == l)) && !lines.iter().any(|l| wrongs.iter().any(|w| w == l)) {
            return Some("answer_line");
        }
    }
    // Blank: an accepted answer next to the blank, in brackets or after an arrow or a colon.
    if key.blank {
        // ("Word bank: Mars, Venus, Jupiter." lists the key among distractors: allowed.)
        // (Separators from each byte to the next full stop or line end, in one backward pass:
        // reading the tail per occurrence of the key was quadratic, critic22b.)
        let bytes = f.as_bytes();
        let mut seps_to_end = vec![0u16; bytes.len() + 1];
        for i in (0..bytes.len()).rev() {
            seps_to_end[i] = match bytes[i] {
                b'\n' | b'.' => 0,
                b',' | b'/' | b';' => seps_to_end[i + 1].saturating_add(1),
                _ => seps_to_end[i + 1],
            };
        }
        let listed = |p: usize| seps_to_end[p] >= 2;
        // Where each blank ends, once.
        let blank_ends: Vec<usize> = {
            let (mut v, mut run) = (Vec::new(), 0usize);
            for (i, b) in bytes.iter().enumerate() {
                if *b == b'_' {
                    run += 1;
                } else {
                    if run >= 3 {
                        v.push(i);
                    }
                    run = 0;
                }
            }
            if run >= 3 {
                v.push(bytes.len());
            }
            v
        };
        // (Each distinct accepted answer once, and at most 512 occurrences in all: fifty blanks
        // accepting the same word repeated through the stem cost no more than one.)
        let mut distinct: Vec<&String> = keys.iter().collect();
        distinct.sort();
        distinct.dedup();
        // (Each answer's places found through one word index of the stem, every place kept; at
        // most 4,096 places read, then it fails closed: round 14 critic26b. An answer without a
        // letter or digit is searched for.)
        let index = TokenIndex::with_cap(&f, usize::MAX);
        let mut budget = 4096usize;
        for k in distinct {
            let places: Vec<usize> = if k.chars().any(crate::format::is_alnum) { index.find_all(k) } else { f.match_indices(k.as_str()).take(4097).map(|(p, _)| p).collect() };
            for p in places {
                if budget == 0 {
                    return Some("scan_budget");
                }
                budget -= 1;
                if bank_ranges.iter().any(|r| r.contains(&p)) {
                    continue;
                }
                let before = f[..p].trim_end().chars().next_back();
                let after = f[p + k.len()..].trim_start().chars().next();
                let word = !f[..p].chars().next_back().is_some_and(crate::format::is_alnum) && !f[p + k.len()..].starts_with(crate::format::is_alnum);
                // (A bracket glued to a word is notation, "Poisson(λ)", "f(x)": round 14.)
                let notation = matches!(before, Some('(' | '[')) && f[..p].trim_end().trim_end_matches(['(', '[']).chars().next_back().is_some_and(crate::format::is_alnum) && !f[..p].trim_end().trim_end_matches(['(', '[']).ends_with(' ');
                if word && !notation && !listed(p) && (matches!(before, Some('(' | '[' | '>' | ':' | '=' | '\u{3010}')) && (matches!(before, Some('>' | ':' | '=')) || matches!(after, Some(')' | ']' | '\u{3011}')))) {
                    // In brackets right after the blank it may be the word to put in its form
                    // ("I ___ (eat) breakfast"): `blank_hint`, which the author can acknowledge.
                    let gap_before = f[..p].trim_end().trim_end_matches(['(', '[', '\u{3010}']).trim_end().ends_with("___");
                    return Some(if matches!(before, Some('(' | '[' | '\u{3010}')) && gap_before { "blank_hint" } else { "answer_line" });
                }
                // Round 11, B8: restated near the blank ("____. - Nairobi", "____.\nNairobi ✓") or
                // emphasised, quoted, ticked anywhere ("**Nairobi**", "*X*", "('X')", "X ✓").
                let near_blank = blank_ends.partition_point(|&e| e <= p).checked_sub(1).map(|i| blank_ends[i]).is_some_and(|e| p - e <= 160 && f[e..p].chars().count() <= 40 && !f[e..p].contains("___"));
                let raw_before = f[..p].chars().next_back();
                let raw_after = f[p + k.len()..].chars().next();
                let emphasised = matches!((raw_before, raw_after), (Some('*'), Some('*')) | (Some('_'), Some('_')) | (Some('\''), Some('\'')) | (Some('"'), Some('"')));
                let ticked = matches!(after, Some('\u{2713}' | '\u{2714}' | '\u{2705}'));
                if word && !listed(p) && (near_blank || emphasised || ticked) {
                    return Some("answer_line");
                }
            }
        }
    }
    // Answer leads.
    // ("Key terms: mitosis, ..." and "Correct: ..." name a key text only by letter or value.)
    // (Built at the first lead only, or once for a whole bank (`KeyInfo::with_maps`): a bank's
    // keys, scanned against every stem, cost nothing for a stem without one.)
    let lazy: std::cell::OnceCell<KeyMaps> = std::cell::OnceCell::new();
    let maps = || key.maps.as_deref().unwrap_or_else(|| lazy.get_or_init(|| KeyMaps::new(&keys)));
    // (Nothing to read at `d`: a word of two characters or more that no key text, True/False
    // value or "Option" word starts.)
    let nothing_at = |d: usize| d < n && fc[d].fast_alpha() && fc.get(d + 1).is_some_and(|c| is_alnum(*c)) && !maps().may_start(&fc, d) && !starts_any(&fc, d, tf_letter_by_first());
    let over_budget = std::cell::Cell::new(false);
    let local_steps = std::cell::Cell::new(0u32);
    // (Leads cost work, not a count: round 17, critic31a B5 refused a data table whose 63 rows
    // said "Key Stage". The budget refuses only texts that take real reading.)
    let charge = |n: u32| -> bool {
        local_steps.set(local_steps.get() + n);
        spend(n as u64) || local_steps.get() > 100_000
    };

    // (The contiguous range of text whose word starts have been walked with all the context a key
    // needs: a later window inside it resumes after it. Round 17, critic31b B3.)
    // (The nearest position at or after each one where a key may start a word, built once at the
    // first reading: "a key later in the line" is one lookup per lead. Round 17, critic31b C3.)
    let next_start: std::cell::OnceCell<Vec<usize>> = std::cell::OnceCell::new();
    let next_start = || {
        next_start.get_or_init(|| {
            let mut v = vec![usize::MAX; n + 1];
            for i in (0..n).rev() {
                let here = is_alnum(fc[i]) && (i == 0 || !is_alnum(fc[i - 1])) && maps().may_start(&fc, i);
                v[i] = if here { i } else { v[i + 1] };
            }
            v
        })
    };
    // (The longest key, in characters: a key starting in the window may end past it.)
    let longest = || maps().longest;
    // (The end of the alnum run at each position, built once: a window inside one run has no word
    // boundary, so no short key can stand there as a token. Round 17, critic31b B3: a CJK lead
    // repeated through a text cost a window read per character.)
    let run_end: std::cell::OnceCell<Vec<u32>> = std::cell::OnceCell::new();
    let run_end = || {
        run_end.get_or_init(|| {
            let mut v = vec![n as u32; n + 1];
            for i in (0..n).rev() {
                v[i] = if is_alnum(fc[i]) { v[i + 1] } else { i as u32 };
            }
            v
        })
    };
    // (The contiguous range of text whose word starts have been walked with all the context a key
    // needs: a later window inside it resumes after it, and the walk from a start needs `longest`
    // characters plus the one after the match. Round 17, critic31b B3.)
    let scanned: std::cell::Cell<Option<(usize, usize)>> = std::cell::Cell::new(None);
    // (The byte offset of each character, built once: a window is a slice of the folded text, not
    // a copy, and a repeated lead allocates nothing. Round 17, critic31b B3.)
    let cb_all: std::cell::OnceCell<Vec<u32>> = std::cell::OnceCell::new();
    let cb_all = || {
        cb_all.get_or_init(|| {
            let mut v: Vec<u32> = Vec::with_capacity(n + 1);
            v.extend(f.char_indices().map(|x| x.0 as u32));
            v.push(f.len() as u32);
            v
        })
    };
    let names_key = |at: usize, texts: bool| -> bool {
        // The text after the separator, on its first non-empty line, at most KEY_WINDOW characters.
        let mut k = at;
        while k < n && (fc[k] == ' ' || fc[k] == '\n') {
            k += 1;
        }
        // (Fast rejection, so a lead repeated thousands of times costs a few comparisons each: a
        // word of two characters or more that no key text, True/False value or "Option" word
        // starts with names nothing.)
        let mut d = k;
        while d < n && d < k + 8 && matches!(fc[d], '\'' | '"' | '*' | '_' | '`' | '(' | '[' | '\u{3010}' | '\u{300C}' | ' ') {
            d += 1;
        }
        // (Unless a key may start at a later word of the line: "Answer: as we said, Mars"; round
        // 16, a key past the first word was never read.)
        if nothing_at(d) && !key.letter_keys {
            let end = n.min(k + KEY_WINDOW);
            let later = texts && d + 1 < n && next_start()[d + 1] < end && !fc[d + 1..next_start()[d + 1]].contains(&'\n');
            if !later {
                return false;
            }
        }
        // (A key may START within the window and end past it: the line is read to the window
        // plus the longest key. Round 17, critic31b C1: a key crossing the window was missed.)
        let line_end = n.min(k + KEY_WINDOW + longest());
        // (The whole window is one alnum run and no key is longer than the window: a key starting
        // at any word start there (the run's start) is glued to the run, so no key can stand as a
        // token. A run to the end of the text shorter than the longest key may be a key; a longer
        // one has no key as a whole line either. Round 17, critic31b B3.)
        if texts && !maps().has_long && d < n && {
            let re = run_end()[d] as usize;
            re > line_end || (re == n && n - d > longest())
        } {
            return false;
        }
        // (A slice of the folded text: no copy per lead.)
        let cb_all = cb_all();
        let line = &f[cb_all[k] as usize..cb_all[line_end] as usize];
        let line = &line[..line.find('\n').unwrap_or(line.len())];
        // (The line is read in full: the charge is the work done, not a fraction of it, so the
        // text's budget stops a repeated lead after a bounded reading. Round 17, critic31b B3.)
        if charge(line.len() as u32 + 1) {
            over_budget.set(true);
            return true;
        }
        if key.any_key {
            return line.chars().any(crate::format::is_alnum);
        }
        // (Also a colon after a copula: "The correct answer is: E7018.")
        let mut t = line.trim_start_matches(['\'', '"', '*', '_', '`', '(', '[', '\u{3010}', '\u{300C}', ' ', ':']);
        for w in folded_list(3) {
            if let Some(r) = t.strip_prefix(w.as_str())
                && r.starts_with(' ')
            {
                t = r.trim_start_matches(['\'', '"', '*', '_', '`', '(', '[', ' ']);
                break;
            }
        }
        let first: String = t.chars().take_while(|c| is_alnum(*c)).collect();
        // A letter stands alone ("B", "B)", "B.", "**B**"), not as a variable ("y = 2").
        let rest = &t[first.len()..];
        // (Or followed by the next field of an exported row: "ANS: B  PTS: 1", ExamView.)
        let field = {
            let r = rest.trim_start();
            let w = r.chars().take_while(|c| c.fast_alpha()).count();
            rest.starts_with(' ') && (2..=12).contains(&w) && r.chars().nth(w) == Some(':')
        };
        let alone = field || rest.is_empty() || rest.starts_with([')', '.', ',', ';', ']', '*', '"', '\'', '}', '\u{3011}', '!', '\u{300D}']) || rest.trim_start().is_empty() || rest.trim_start().starts_with(['(', '-', '>']);
        // ("Answer A, B, C or D", "Answer true or false": instructions, not answers.)
        let after_first = rest.trim_start();
        let line_rest: String = after_first.chars().take(40).collect();
        let alternatives = [" or ", " o ", " ou ", " oder ", " или ", " atau ", " veya ", "/"].iter().any(|w| format!(" {line_rest}").contains(w));
        let instruction = ["or ", "o ", "ou ", "oder ", "или ", "atau ", "veya "].iter().any(|w| after_first.starts_with(w)) || (after_first.starts_with([',', '/']) && alternatives);
        // (The whole key first: "f/1.8" is no "A/B" instruction.)
        let whole = t.trim_end_matches([']', ')', '.', ' ', '!', '*', '"', '\'']);
        if maps().contains(whole) {
            return true;
        }
        if instruction {
            return false;
        }
        let key_letter = |w: &str| is_key_letter(w, &key.letters);
        // An ordinal when students see the order ("Answer: the second option", keep_order).
        if choices && key.positional {
            let o = t.strip_prefix("the ").unwrap_or(t);
            let w: String = o.chars().take_while(|c| c.fast_alpha()).collect();
            const ORD: [&str; 10] = ["first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth"];
            if let Some(i) = ORD.iter().position(|x| *x == w) {
                let next = o[w.len()..].trim_start();
                let noun = ["option", "choice", "answer", "one", "alternative"].iter().any(|n| next.starts_with(n)) || next.is_empty() || next.starts_with(['.', ')', ',']);
                if noun && key.letters.contains(&((b'a' + i as u8) as char)) {
                    return true;
                }
            }
        }
        // A string of choice letters (and T/F) as a whole key: "Key: ABT", "Answers: BDAC".
        // (Or one letter at a time: "A B T", "A, B, True".)
        if key.letter_keys {
            let toks: Vec<&str> = t.split([' ', ',', ';']).filter(|w| !w.is_empty()).take(26).collect();
            let letterish = |w: &str| (w.chars().count() == 1 && w.chars().all(|c| matches!(c, 'a'..='f' | 't'))) || (tf_word(w) && w.chars().count() >= 4);
            let run = toks.iter().take_while(|w| letterish(w.trim_end_matches(['.', ')'])) ).count();
            if run >= 2 {
                return true;
            }
        }
        if key.letter_keys && (2..=40).contains(&first.chars().count()) && first.chars().all(|c| c.is_ascii_lowercase()) && first.chars().any(|c| matches!(c, 'a'..='e')) && !first.chars().any(|c| matches!(c, 'g'..='s' | 'u'..='z')) && !after_first.starts_with(crate::format::is_alpha) {
            return true;
        }
        // An answer key of numbered letters or values ("1B 2T", "1-B, 2-True", "4B"): a key of the
        // whole bank, whatever this question's key (round 12).
        // Round 18 (critic34b B2): a letter attached to its number and followed by its
        // description ("1a  Has six legs ... Spider") is a dichotomous key's couplet label, a
        // part label, not question 1's choice A. The canonical honest form is not refused. A
        // key list's value follows a separator or another numbered value ("1 B", "4B",
        // "1C 2B 3D"), so those still match.
        {
            let toks: Vec<&str> = t.split([' ', ',', ';']).filter(|w| !w.is_empty()).take(13).collect();
            let mut count = 0;
            let mut glued = None;
            let mut couplet = false;
            for (i, tok) in toks.iter().enumerate() {
                let digits = tok.chars().take_while(char::is_ascii_digit).count();
                if digits == 0 || digits > 3 {
                    break;
                }
                let raw = &tok[digits..];
                let attached = !raw.starts_with(['-', '.', ')', ':', '=']);
                let v = raw.trim_start_matches(['-', '.', ')', ':', '=']).trim_end_matches(['.', ')', ']']);
                let value = (v.chars().count() == 1 && v.chars().all(|c| c.fast_alpha())) || tf_word(v) && !v.is_empty();
                if !value {
                    break;
                }
                if count == 0 {
                    couplet = attached
                        && v.chars().count() == 1
                        && v.chars().all(|c| c.fast_alpha())
                        && toks.get(i + 1).and_then(|w| w.chars().next()).is_some_and(|c| c.fast_alpha());
                    glued = Some(v.to_string());
                }
                count += 1;
            }
            if count >= 2 || (count == 1 && choices && !couplet && glued.is_some_and(|v| v.chars().count() == 1 && key_letter(&v))) {
                return true;
            }
        }
        if choices && !first.is_empty() && alone && key_letter(&first) {
            let letter = (first.chars().count() == 1 && first.chars().all(crate::format::is_alpha)) || first == "\u{627}\u{644}\u{641}";
            let number = first.chars().all(|c| c.is_ascii_digit()) && first.parse::<u32>().is_ok_and(|v| (1..=26).contains(&v));
            if letter || number {
                return true;
            }
        }
        if key.tf && tf_matches(&first) {
            return true;
        }
        // A key text within the first twelve words ("Paris", "the city of Paris", "Paris,
        // France", "we said in class it is Paris"; four until round 14).
        // (At each word start, one lookup per word end: a repeated lead costs a few lookups.)
        if !texts {
            return false;
        }
        // (Where `t` starts in the text: a start already walked by an earlier lead's reading is
        // not walked again. Round 16: overlapping windows of repeated leads.)
        let tlen = t.chars().count();
        let base = k + line.chars().count() - tlen;
        let win_end = base + tlen;
        // (A window whose word starts were all walked already: no second reading. Round 17,
        // critic31b B3.)
        if scanned.get().is_some_and(|(s, e)| base >= s && win_end <= e) {
            return false;
        }
        // (Starts far enough before the covered end that every key from them ended with all its
        // context were walked: resume after them. The walk from a start needs `longest` characters
        // and the one after the match.)
        let resume = match scanned.get() {
            Some((s, e)) if base >= s && base < e => e.saturating_sub(longest() + 1).saturating_sub(base),
            _ => 0,
        };
        if charge((tlen - resume.min(tlen)) as u32 + 1) {
            over_budget.set(true);
            return true;
        }
        // (Starts within the window; a walk may run past it.)
        let start_limit = KEY_WINDOW.saturating_sub(line.chars().count() - tlen);
        let limit = tlen.min(start_limit.max(1));
        // (Word starts only: a key is looked up where a word starts, so the non-starts of a long
        // run are skipped, not examined one by one. No character vector per lead: the folded
        // text's own byte offsets are used, and ASCII text (the common case) is walked as bytes.
        // Round 17, critic31b B3.)
        let m = maps();
        let tb = t.as_bytes();
        let walk = |bp: usize| -> Option<bool> {
            // One walk down the keys' arena from here: a key ends where a word ends (round 16: a
            // hash per key length per start was 31.8 s on 99 lengths). Each step is counted
            // against the publish's work budget; past it the text is refused (`scan_budget`).
            let mut found = false;
            let out = m.lex.walk_counted(tb, bp, tb.len() - bp, &local_steps, &mut |e, ents| {
                if ents.iter().any(|x| Lex::tag(x) == 1) {
                    found = true;
                    return true;
                }
                let after = t[e..].chars().next();
                let last = t[..e].chars().next_back();
                // (A key of three characters or fewer counts only as the first word after the
                // lead: "Save Answer" followed by "... which tube is collected" is a pasted
                // Moodle button and prose, not a key "is". Round 17, critic31a A6's bank corpus.)
                let short_late = bp > 0 && (e - bp <= 3 || t[bp..e].chars().nth(3).is_none());
                if short_late {
                    return false;
                }
                // (A number is named whole: "2" is not named by "2.5" or "2,5".)
                let numeric = last.is_some_and(|c| c.is_ascii_digit()) && matches!(after, Some('.' | ',')) && t[e..].chars().nth(1).is_some_and(|c| c.is_ascii_digit());
                if !after.is_some_and(is_alnum) && !numeric {
                    found = true;
                    return true;
                }
                false
            });
            if out { None } else { Some(found) }
        };
        if t.is_ascii() {
            let mut prev = false;
            for (p, &byte) in tb.iter().take(limit).enumerate() {
                let alnum = byte.is_ascii_alphanumeric();
                let start = p == 0 || !prev;
                if start && p >= resume && alnum && m.lex.may_start(byte as char) {
                    match walk(p) {
                        Some(true) => return true,
                        Some(false) => {}
                        None => {
                            over_budget.set(true);
                            return true;
                        }
                    }
                }
                prev = alnum;
            }
        } else {
            let mut prev = false;
            for (p, (bp, c)) in t.char_indices().enumerate() {
                if p >= limit {
                    break;
                }
                let start = p == 0 || !prev;
                if start && p >= resume && is_alnum(c) && m.lex.may_start(c) {
                    match walk(bp) {
                        Some(true) => return true,
                        Some(false) => {}
                        None => {
                            over_budget.set(true);
                            return true;
                        }
                    }
                }
                prev = is_alnum(c);
            }
        }
        // (This window's word starts are walked to its end with nothing found.)
        scanned.set(Some(match scanned.get() {
            Some((s, e)) if base <= e => (s.min(base), e.max(win_end)),
            _ => (base, win_end),
        }));
        false
    };
    // After a lead: up to two words, then a separator (":", "=", "-", ">", ".", a new line, a
    // closing bracket) or a copula ("is", "是"). Where the key would start, or None.
    let after_lead = |mut k: usize, cjk: bool| -> Option<(usize, bool)> {
        let mut words = 0;
        loop {
            while k < n && fc[k] == ' ' {
                k += 1;
            }
            if k >= n {
                return None;
            }
            let c = fc[k];
            // A bracketed label or plural ending among the qualifiers: "Answer to (a):", "Correct
            // answer(s):", "Respuesta (a):".
            if c == '(' && words <= 2 {
                let close = fc[k..n.min(k + 5)].iter().position(|x| *x == ')');
                if let Some(z) = close
                    && z >= 2
                    && fc[k + 1..k + z].iter().all(|x| x.fast_alnum())
                {
                    k += z + 1;
                    continue;
                }
            }
            // (After qualifier words only a colon or "=" ends the lead: "the solution of x - 3".)
            let sep = if words == 0 { matches!(c, ':' | '=' | '-' | '>' | '\n' | '.' | '\u{3011}' | ']' | ')' | '|' | '\u{3001}' | ',') } else { matches!(c, ':' | '=') };
            if sep {
                let mut e = k;
                while e < n && matches!(fc[e], ':' | '=' | '-' | '>' | '\n' | '.' | '\u{3011}' | ']' | ')' | '|' | ' ' | '\u{3001}' | ',') {
                    e += 1;
                }
                return Some((e, false));
            }
            // (After a CJK lead only a copula's few characters matter: reading the whole run would
            // be quadratic when the lead repeats, "答答答...". A spaced lead starts and ends at word
            // boundaries, so its next word lies before any later lead.)
            // (Compared in place, no copy: a repeated lead costs a few comparisons.)
            let wl = if cjk { fc[k..].iter().take(8).take_while(|c| is_alnum(**c)).count() } else { fc[k..].iter().take_while(|c| is_alnum(**c)).count() };
            if wl == 0 {
                return None;
            }
            let w = &fc[k..k + wl];
            if copulas_chars().iter().any(|c| c[..] == w[..]) {
                return Some((k + wl, true));
            }
            // A copula glued to a CJK lead ("答案是B", "正解はB").
            if cjk && let Some(cop) = copulas_chars().iter().find(|c| !c[0].is_ascii() && w.starts_with(c)) {
                return Some((k + cop.len(), true));
            }
            if cjk {
                return Some((k, false));
            }
            words += 1;
            // (A question number among the qualifiers: "The answer to 5 is Quartz", round 17.)
            let number = wl <= 3 && w.iter().all(|c| c.is_ascii_digit());
            if words > 2 || (wl < 2 && !number) || !(w.iter().all(|c| c.fast_alpha()) || number) {
                return None;
            }
            k += wl;
        }
    };
    // Where the first sentence ends ("The answer is true." alone is a statement).
    let first_stop = fc.iter().position(|c| matches!(c, '?' | '.' | '\n' | ':' | '!' | '(' | '\u{3002}' | '\u{FF1F}'));
    // (The lead at `pos`, exactly as the loop below takes it: the word's end is not an alnum and
    // no bare full stop ends it. Round 17, critic31b B3.)
    let lead_at = |pos: usize| -> Option<usize> {
        // (The loop below only reads a lead at a word start; a lead glued after a copula's letter
        // would not be read by it, so it is no safe skip.)
        if pos > 0 && is_alnum(fc[pos - 1]) {
            return None;
        }
        let cands = leads_by_first().get(fc.get(pos)?)?;
        cands.iter().find(|(lead, lc)| {
            pos + lc.len() <= n
                && fc[pos..pos + lc.len()] == lc[..]
                && !fc.get(pos + lc.len()).is_some_and(|c| is_alnum(*c))
                && !(fc.get(pos + lc.len()) == Some(&'.') && !matches!(lead.as_str(), "ans" | "sol" | "resp" | "rpta"))
        }).map(|(_, lc)| lc.len())
    };
    // (A lead immediately followed by another lead: the next occurrence reads everything after its
    // own separator, so only this occurrence's leading unit (the next lead and its separator) is
    // lost by skipping the window. Check that unit as a key, then skip. Round 17, critic31b B3:
    // "መልስ:መልስ:..." cost a window read per unit.)
    let unit_key = |at: usize, ll: usize| -> Option<bool> {
        if maps().has_long {
            return None;
        }
        let mut u = at + ll;
        while u < n && matches!(fc[u], ':' | '=' | '-' | '>' | '\n' | '.' | '\u{3011}' | ']' | ')' | '|' | ' ' | '\u{3001}' | ',') {
            u += 1;
        }
        let len = u - at;
        // (A key longer than the unit could span into the next one: read the window then.)
        if len == 0 || longest() > len {
            return None;
        }
        let unit: String = fc[at..u].iter().collect();
        let hit = unit.char_indices().map(|x| x.0).chain(std::iter::once(unit.len())).skip(1).any(|e| {
            let kk = &unit[..e];
            if !maps().contains(kk) {
                return false;
            }
            let ec = kk.chars().count();
            let after = fc.get(at + ec);
            let numeric = kk.ends_with(|c: char| c.is_ascii_digit()) && matches!(after, Some('.' | ',')) && fc.get(at + ec + 1).is_some_and(|c| c.is_ascii_digit());
            !after.is_some_and(|c| is_alnum(*c)) && !numeric
        });
        Some(hit)
    };
    // Round 19 (critic35b B2): a numbered row under a "Key" / "Answer key" heading line of its own
    // ("Answer key\n1. Paris", "Key\n1 Quartz", "Key\n1. B"). `key` and `correct` as leads never
    // compare the key text ("Key stage", "key terms"), and the numbered-letter rule needs two
    // values on one line, so one row under the heading was read by nothing. Only a heading that
    // stands alone on its line (at most one qualifier word before it, nothing but ":" after) and a
    // row that opens with a label (a bullet, "1.", "1)", "(1)", "Q1:", "a)", "1 ") are read.
    // A couplet label, a digit with a letter glued to it ("1a  Has six legs", a dichotomous key),
    // is no row of an answer key: that worksheet stays clean. The row's value that IS the key (the
    // whole rest of the line: the key text, its letter, its True/False value) is high risk; the
    // key text among the first words of a longer row is low risk (`key_heading_row`, ackable),
    // except in the bank's cross-question scan, which has no acknowledgement and refuses both.
    let heading_row = |k: usize, end: usize, low: bool| -> Option<&'static str> {
        const QUALIFIERS: [&str; 9] = ["answer", "answers", "ans", "correct", "solution", "solutions", "marking", "model", "teacher"];
        // Nothing but spaces and a colon after it, then a line break (the heading alone on its
        // line), or, round 20 (critic36b B1), a numbered row on the SAME line ("Key: 1. Paris",
        // "Answers 1) Paris", "Mark scheme 1 Paris"). Checked first: a repeated lead costs a few
        // comparisons, and nothing is built for it.
        let mut e = end;
        // ("Ans." is the one lead a full stop may end; no other lead gets here with one.)
        while e < n && matches!(fc[e], ' ' | ':' | '.') {
            e += 1;
        }
        // (A dash or "=" between: "Key - 1. Paris".)
        if e + 1 < n && matches!(fc[e], '-' | '=' | '\u{2013}') && fc[e + 1] == ' ' {
            e += 2;
            while e < n && fc[e] == ' ' {
                e += 1;
            }
        }
        let same_line = e < n && fc[e] != '\n';
        if e >= n {
            return None;
        }
        if same_line {
            // (A numbered label only: a digit, "Q1", "(1)", "a)".)
            let numbered = fc[e].is_ascii_digit() || (fc[e].fast_alpha() && fc.get(e + 1) == Some(&')') && fc.get(e + 2) == Some(&' ')) || (fc[e] == 'q' && fc.get(e + 1).is_some_and(|c| c.is_ascii_digit())) || (fc[e] == '(' && fc.get(e + 1).is_some_and(|c| c.is_ascii_digit()));
            if !numbered {
                return None;
            }
        }
        // The heading's own line: at most one qualifier word before the lead (read back from the
        // lead once, and only for a lead that ends its line). A same-line row after any text is
        // still read for the key's exact value; only the low-risk reading wants the heading form.
        let ls = fc[..k].iter().rposition(|c| *c == '\n').map_or(0, |p| p + 1);
        let mut qualified = true;
        if k - ls > 16 {
            qualified = false;
        } else {
            let before: String = fc[ls..k].iter().collect();
            let before = before.trim();
            if !(before.is_empty() || QUALIFIERS.contains(&before)) {
                qualified = false;
            }
        }
        if !same_line {
            // (Round 21, critic37b B2 / critic37a B37a-1: a lead that ends a line after prose,
            // "Capital of France? Key:" / "1. Paris", is read like one that opens its line.)
            while e < n && matches!(fc[e], ' ' | '\n') {
                e += 1;
            }
            if e >= n {
                return None;
            }
        }
        // The row's label.
        let digits = fc[e..].iter().take(4).take_while(|c| c.is_ascii_digit()).count();
        let v = if matches!(fc[e], '\u{2022}' | '-' | '*' | '\u{B7}' | '\u{25AA}' | '\u{25E6}' | '>' | '\u{2192}') && fc.get(e + 1) == Some(&' ') {
            e + 2
        } else if fc[e] == '(' {
            let w = fc[e + 1..].iter().take(4).take_while(|c| c.fast_alnum()).count();
            if w == 0 || fc.get(e + 1 + w) != Some(&')') {
                return None;
            }
            e + w + 2
        } else if fc[e] == 'q' && fc.get(e + 1).is_some_and(|c| c.is_ascii_digit()) {
            let d = fc[e + 1..].iter().take(4).take_while(|c| c.is_ascii_digit()).count();
            // ("Q1: K", "Q1. K", "Q1 - K", "Q1 K".)
            let mut x = e + 1 + d;
            while fc.get(x) == Some(&' ') {
                x += 1;
            }
            if fc.get(x).is_some_and(|c| matches!(c, ':' | '.' | ')' | '-' | '=')) {
                x += 1;
            }
            x
        } else if digits > 0 && digits <= 3 {
            let mut x = e + digits;
            match fc.get(x) {
                Some('.' | ')' | ':' | '-' | ' ') => {}
                // (A letter glued to its number is a couplet label: "1a  Has six legs".)
                _ => return None,
            }
            // ("1. K", "1) K", "1 - K", "1 K".)
            while fc.get(x) == Some(&' ') {
                x += 1;
            }
            if fc.get(x).is_some_and(|c| matches!(c, ':' | '.' | ')' | '-' | '=')) {
                x += 1;
            }
            x
        } else if fc[e].fast_alpha() && matches!(fc.get(e + 1), Some(')' | '.')) && fc.get(e + 2) == Some(&' ') {
            e + 2
        } else {
            return None;
        };
        let mut v = v;
        let label_end = v;
        while v < n && fc[v] == ' ' {
            v += 1;
        }
        if v >= n || fc[v] == '\n' {
            return None;
        }
        // (A same-line label that is only a number and a space, "Answer: 2 3", is data.)
        if same_line && fc[label_end - 1] == ' ' && fc[v].is_ascii_digit() {
            return None;
        }
        if charge(1) {
            return Some("scan_budget");
        }
        let line_end = fc[v..].iter().position(|c| *c == '\n').map_or(n, |p| v + p);
        let value: String = fc[v..line_end].iter().collect();
        let value = value.trim_end_matches(['.', ' ', ')', ']', '*', '!']).trim_start_matches(['(', '[', '*']);
        if value.is_empty() {
            return None;
        }
        let single = value.chars().count() == 1 || (value.chars().all(|c| c.is_ascii_digit()) && value.len() <= 2);
        let exact = maps().contains(value) || (choices && single && is_key_letter(value, &key.letters)) || (key.tf && tf_matches(value));
        if exact {
            // (Round 21: the weaker lead words are low risk whatever the row holds.)
            return Some(if low { if key.lines_only { "answer_line" } else { "key_heading_row" } } else { "answer_line" });
        }
        // The key among the first words of a longer row.
        if (qualified || !same_line) && names_key(v, true) {
            return Some(if over_budget.get() { "scan_budget" } else if key.lines_only { "answer_line" } else { "key_heading_row" });
        }
        None
    };
    // Latin and other spaced leads at word starts.
    let mut k = 0;
    while k < n {
        if !is_alnum(fc[k]) || (k > 0 && is_alnum(fc[k - 1])) {
            k += 1;
            continue;
        }
        // (Every spaced lead has two characters or more: a one-letter word ("a", "I", "w0") costs
        // no map lookup. Round 17: a bank of 26,000 choices full of "a" was 4.4 s without it.)
        if !fc.get(k + 1).is_some_and(|c| is_alnum(*c)) {
            k += 1;
            continue;
        }
        // Round 21: the weaker lead words of a mark scheme ("Keys", "Marking scheme", "Marking
        // guide", "Rubric", "Memo", "Marks") are read for a numbered row only, and only as low
        // risk (an honest "Marks: 3 of 4" is no key): `key_heading_row`, acknowledged by the author.
        if matches!(fc[k], 'k' | 'm' | 'r') {
            for w in ["keys", "marking scheme", "marking guide", "rubric", "memo", "marks"] {
                let wl = w.len();
                if k + wl <= n && w.chars().enumerate().all(|(i, c)| fc[k + i] == c) && !fc.get(k + wl).is_some_and(|c| is_alnum(*c)) {
                    if charge(1) {
                        return Some("scan_budget");
                    }
                    if let Some(r) = heading_row(k, k + wl, true) {
                        return Some(r);
                    }
                }
            }
        }
        // (Only the leads that start with this character, compared in place.)
        let Some(cands) = leads_by_first().get(&fc[k]) else {
            k += 1;
            continue;
        };
        for (lead, lc) in cands {
            if k + lc.len() <= n && fc[k..k + lc.len()] == lc[..] && !fc.get(k + lc.len()).is_some_and(|c| is_alnum(*c)) {
                if charge(1) {
                    return Some("scan_budget");
                }
                let end = k + lc.len();
                // "Ans." is the only lead a full stop may end.
                if fc.get(end) == Some(&'.') && !matches!(lead.as_str(), "ans" | "sol" | "resp" | "rpta") {
                    continue;
                }
                if matches!(lead.as_str(), "key" | "correct" | "kunci" | "answer" | "answers" | "ans" | "solution" | "solutions" | "sol" | "mark scheme" | "model answer" | "acceptable answer" | "acceptable answers")
                    && let Some(r) = heading_row(k, end, false)
                {
                    return Some(r);
                }
                // ("The answer is true." alone is a statement; after a question it gives the key.)
                let copula_ok = first_stop.is_some_and(|p| p < k);
                // (A copula then a colon, "The correct answer is: X", or the bank's own text, is an
                // answer line even in the first sentence.)
                let colon_at = |at: usize| fc[at.min(n)..].iter().find(|c| **c != ' ') == Some(&':');
                if let Some((at, copula)) = after_lead(end, false)
                    && (!copula || copula_ok || colon_at(at))
                {
                    // (A lead right after this one's separator is read by its own occurrence: only
                    // the leading unit is checked here, and the window is skipped. Round 17,
                    // critic31b B3.)
                    let skip = lead_at(at).and_then(|ll| unit_key(at, ll));
                    if skip == Some(true) {
                        return Some("answer_line");
                    }
                    if skip.is_none() && names_key(at, !matches!(lead.as_str(), "key" | "correct" | "kunci")) {
                        return Some(if over_budget.get() { "scan_budget" } else { "answer_line" });
                    }
                }
                // No separator ("Answer B", "Key B", "ans Mars"): only what is this question's
                // key (a correct choice's letter, the True/False value, exactly the key text).
                if (!key.letters.is_empty() || key.tf_value.is_some() || !keys.is_empty())
                    && fc.get(end) == Some(&' ')
                    && !nothing_at(end + 1)
                {
                    // (A True/False word there counts only as this question's value and at the end
                    // of its line: "Answer True", not "pick the answer that is true", "Chọn đáp
                    // án đúng." in a bank whose questions hold both values. Round 15.)
                    let word: String = fc[end + 1..].iter().take_while(|c| is_alnum(**c)).collect();
                    let rest_of_line = fc[(end + 1 + word.chars().count()).min(n)..].iter().take_while(|c| **c != '\n').any(|c| is_alnum(*c));
                    let tf_first = key.tf && tf_word(&word) && !(key.choices && word.chars().count() == 1);
                    // (After an article the lead is a noun in an instruction, "Elige la respuesta
                    // verdadera", "Pick the answer that is true".)
                    let prev_word: String = fc[..k].iter().rev().skip_while(|c| **c == ' ').take_while(|c| is_alnum(**c)).collect::<Vec<_>>().into_iter().rev().collect();
                    let article = matches!(prev_word.as_str(), "the" | "a" | "an" | "la" | "el" | "le" | "les" | "los" | "las" | "die" | "der" | "das" | "den" | "il" | "lo" | "gli" | "o" | "os" | "as" | "het" | "de" | "een" | "une" | "un" | "una" | "uno" | "this" | "that" | "each");
                    let tf_ok = !tf_first || (key.tf_value.is_some() && !rest_of_line && !article);
                    if tf_ok && names_key(end, false) {
                        return Some(if over_budget.get() { "scan_budget" } else { "answer_line" });
                    }
                }
            }
        }
        k += 1;
    }
    if fc.iter().any(|c| !c.is_ascii()) {
        // (One pass over the text: each character compares the leads that start with it, instead
        // of scanning the whole text once per lead. Round 17, critic31b B3: 13 leads x 4 MB.)
        for (start, (b, c)) in f.char_indices().enumerate() {
            let Some(cands) = cjk_leads_by_first().get(&c) else {
                continue;
            };
            for (lead, lc) in cands {
                if !f[b..].starts_with(lead.as_str()) {
                    continue;
                }
                let end = start + lc.len();
                // (Glued to a word that is no copula, key, value or "Option" word: nothing to read.)
                if fc.get(end).is_some_and(|c| !copula_firsts().contains(c)) && nothing_at(end) {
                    continue;
                }
                if charge(1) {
                    return Some("scan_budget");
                }
                // (The window is inside one alnum run and no key is longer than the window: no
                // key can stand there as a token, and a copula keeps the start inside the run, so
                // the read is skipped before `after_lead` costs anything. Round 17, critic31b B3.)
                let line_end = n.min(end + KEY_WINDOW + longest());
                if !maps().has_long && end < line_end && run_end()[end] > line_end as u32 {
                    continue;
                }
                if let Some((at, _)) = after_lead(end, true) {
                    let line_end = n.min(at + KEY_WINDOW + longest());
                    if !maps().has_long && at < line_end && run_end()[at] > line_end as u32 {
                        continue;
                    }
                    if names_key(at, true) {
                        return Some(if over_budget.get() { "scan_budget" } else { "answer_line" });
                    }
                }
            }
        }
    }
    // (Low risk, after every high-risk reading.)
    bank_verdict
}

/// Every token a scanner of this module starts on (the word tables and the syntax marks): the
/// repetition-amplification test repeats each one to fill every field. Tests only.
#[doc(hidden)]
pub fn scanner_tokens() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = Vec::new();
    for t in [KEY_WORDS, TRAILING_WORDS, BRACKET_WORDS, CJK_KEY_WORDS, WRONG_WORDS, WRONG_BRACKET, WRONG_CJK, ANSWER_LEADS, ANSWER_LEADS_CJK, ANSWER_COPULAS, LETTER_WORDS, TF_WORDS, TF_SHORT] {
        v.extend_from_slice(t);
    }
    v.extend_from_slice(&[
        "{", "}", "{=", "{~", "{#", "{1:SA:", "{:MC:", "{T}", "{TRUE}", "&", "&amp;", "&#", "&#10007;", "(", ")", "[", "]", "\u{3010}", "\u{3011}", "\u{27E8}", "|", "/",
        "*", "**", "_", "__", "`", "<b", "<b>", "</b>", "\\textbf{", "\\boxed{", "$", "$$", "#", " # ", "~", "=", "==", "%", "%50%", "+1", "-1", "\u{2212}1", "(100%)",
        "[1 pt]", "1/1", "-", "--", ":", "\u{2192}", "<==", "!", "?", "...", "\u{2026}", "\u{2713}", "\u{2717}", "\u{D7}", "\u{221A}", "\u{FF08}", "\u{FF09}", "A.", "a)",
        "(a)", "A", "a", "I", "x", "o", "X", "\u{200C}", "\u{200D}", "\u{34F}", "\u{301}", "\u{AD}", "\u{200B}", "\u{FE0F}", "\u{1F600}", "\u{627}", "\u{627}\u{653}",
        "\u{7B54}\u{6848}\u{662F}", "Answer: ", "ANSWER:", "(T)", "(right)", "right-", "Key:", "KEY", "\u{221E}", "(\u{2212}\u{221E},", "\u{222A}", "\u{FF21}", "\u{430}",
        "\u{A4D1}", "\u{2160}", "\u{B9}", "\u{2460}", "\u{3B1}", "\n", "\t", "\u{A0}", "\u{3000}", "Both", "and", "of the above", "None of the above",
    ]);
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    fn kif(c: &[&str]) -> Vec<usize> {
        let v: Vec<(&str, bool)> = c.iter().enumerate().map(|(i, t)| (*t, i == 0)).collect();
        check(&v).into_iter().map(|f| f.index).collect::<std::collections::BTreeSet<_>>().into_iter().collect()
    }
    #[test]
    fn wide_letters_are_letters() {
        for u in (0x20..0x3100).chain(0x4E00..=0x9FFF).chain(0xAC00..=0xD7A3).chain(0x10A0..=0x10FF) {
            if let Some(c) = char::from_u32(u) {
                assert_eq!(is_alpha(c), c.fast_alpha(), "{u:X}");
                assert_eq!(is_alnum(c), c.fast_alnum(), "{u:X}");
            }
        }
    }
    #[test]
    fn equal_texts_share_features() {
        // Every wrong choice marked, the same text twice: the second is read like the first.
        assert!(check(&[("Mars", true), ("Venus \u{2717}", false), ("Venus \u{2717}", false)]).iter().any(|f| f.index == 0));
    }
    #[test]
    fn basics() {
        assert_eq!(kif(&["*Venus", "Mars", "Earth"]), vec![0]);
        assert!(kif(&["Na+", "K+", "Cl-"]).is_empty());
        // Round 11: a parenthetical on the correct choice alone is a low-risk outlier (the net).
        assert!(check(&[("Mitosis (cell division)", true), ("Meiosis", false), ("Osmosis", false)]).iter().all(|f| f.risk == Risk::Low));
        assert_eq!(kif(&["Venus correct", "Mars", "Earth"]), vec![0]);
        assert!(kif(&["The correct one", "The wrong one", "Another one"]).is_empty());
    }
}
