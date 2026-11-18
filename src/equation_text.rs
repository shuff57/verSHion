//! Text-only equation helpers: splitting `$...$` out of a stem, parsing linear content, and the
//! Unicode plain-text rendering used where math cannot be drawn. No UI or rendering
//! dependencies; [`crate::equation`] re-exports everything here.

use std::ops::Range;

use equashion_math::{Math, from_linear, to_linear, unknown_commands};

// ---------- segments ----------

/// A piece of question text: plain text, or the linear content between a pair of `$`.
#[derive(Clone, Debug, PartialEq)]
pub enum Segment {
    Text(String),
    Math(String),
}

/// A segment with the char range it covers in the source (delimiters included).
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub seg: Segment,
    pub span: Range<usize>,
}

/// Split text on `$...$`. `\$` is a literal dollar. A `$` pair only counts as math when the
/// content is non-empty and neither starts nor ends with whitespace (so "costs $5 and $6" stays text).
pub fn pieces(text: &str) -> Vec<Piece> {
    let ch: Vec<char> = text.chars().collect();
    let mut out: Vec<Piece> = Vec::new();
    let mut buf = String::new();
    let mut buf_start = 0usize;
    let mut i = 0;
    let flush = |buf: &mut String, out: &mut Vec<Piece>, start: usize, end: usize| {
        if !buf.is_empty() {
            out.push(Piece {
                seg: Segment::Text(std::mem::take(buf)),
                span: start..end,
            });
        }
    };
    while i < ch.len() {
        let c = ch[i];
        if c == '\\' && ch.get(i + 1) == Some(&'$') {
            if buf.is_empty() {
                buf_start = i;
            }
            buf.push('$');
            i += 2;
            continue;
        }
        if c == '$' {
            let dbl = ch.get(i + 1) == Some(&'$');
            let start = if dbl { i + 2 } else { i + 1 };
            let mut j = start;
            let mut close = None;
            while j < ch.len() {
                if ch[j] == '\\' {
                    j += 2;
                    continue;
                }
                if ch[j] == '$' {
                    close = Some(j);
                    break;
                }
                j += 1;
            }
            if let Some(j) = close {
                let inner: String = ch[start..j].iter().collect();
                let ok = !inner.trim().is_empty()
                    && !inner.starts_with(char::is_whitespace)
                    && !inner.ends_with(char::is_whitespace);
                if ok {
                    flush(&mut buf, &mut out, buf_start, i);
                    let end = j + if dbl && ch.get(j + 1) == Some(&'$') {
                        2
                    } else {
                        1
                    };
                    out.push(Piece {
                        seg: Segment::Math(inner),
                        span: i..end,
                    });
                    i = end;
                    continue;
                }
            }
            if buf.is_empty() {
                buf_start = i;
            }
            buf.push('$');
            i += 1;
            continue;
        }
        if buf.is_empty() {
            buf_start = i;
        }
        buf.push(c);
        i += 1;
    }
    flush(&mut buf, &mut out, buf_start, ch.len());
    out
}

pub fn rich_segments(text: &str) -> Vec<Segment> {
    pieces(text).into_iter().map(|p| p.seg).collect()
}

/// True when `text` contains at least one `$...$` equation.
pub fn has_math(text: &str) -> bool {
    text.contains('$')
        && pieces(text)
            .iter()
            .any(|p| matches!(p.seg, Segment::Math(_)))
}

/// Wrap linear content as an inline equation chip string.
pub fn wrap_math(linear: &str) -> String {
    format!("${}$", linear.trim())
}

/// Replace the char range `span` of `text` (or insert at char `caret` when `span` is None).
pub fn splice(text: &str, span: Option<Range<usize>>, caret: usize, insert: &str) -> String {
    let mut chars: Vec<char> = text.chars().collect();
    let r = match span {
        Some(r) => r.start.min(chars.len())..r.end.min(chars.len()),
        None => {
            let c = caret.min(chars.len());
            c..c
        }
    };
    chars.splice(r, insert.chars());
    chars.into_iter().collect()
}

/// The linear content of the equation covering `span` (delimiters stripped).
pub fn content_of(text: &str, span: &Range<usize>) -> String {
    pieces(text)
        .into_iter()
        .find(|p| p.span == *span)
        .and_then(|p| {
            if let Segment::Math(m) = p.seg {
                Some(m)
            } else {
                None
            }
        })
        .unwrap_or_default()
}

// ---------- parsing ----------

/// Parse linear (or legacy LaTeX-ish) content. None when it is empty or has commands the
/// parser does not know (those would draw as literal text, so they count as a failure here).
pub fn parse(content: &str) -> Option<Math> {
    parse_notation(content, true)
}

fn parse_notation(content: &str, preserve_grouping: bool) -> Option<Math> {
    let m = from_linear(&source_linear_notation(content, preserve_grouping));
    if m.is_empty() || !unknown_commands(&m).is_empty() {
        return None;
    }
    Some(m)
}

fn literal_math_command(command: &str) -> bool {
    matches!(
        command,
        "text"
            | "textrm"
            | "mathrm"
            | "operatorname"
            | "mathbf"
            | "textbf"
            | "mathcal"
            | "mathscr"
            | "mathfrak"
            | "mathit"
            | "mathsf"
            | "mathtt"
            | "boldsymbol"
            | "bm"
            | "textit"
            | "textsf"
            | "emph"
            | "mbox"
            | "hbox"
            | "mathnormal"
    )
}

fn notation_space(ch: &[char], mut i: usize) -> usize {
    while i < ch.len() && ch[i].is_whitespace() {
        i += 1;
    }
    i
}

fn notation_command(ch: &[char], start: usize) -> (String, usize) {
    let mut end = start + 1;
    while end < ch.len() && ch[end].is_ascii_alphabetic() {
        end += 1;
    }
    (ch[start + 1..end].iter().collect(), end)
}

/// A literal command body balances braces only: brackets and commas inside text are data.
fn notation_braces_end(ch: &[char], start: usize) -> Option<usize> {
    if ch.get(start) != Some(&'{') {
        return None;
    }
    let mut depth = 0;
    let mut i = start;
    while i < ch.len() {
        if ch[i] == '\\' {
            i = (i + 2).min(ch.len());
            continue;
        }
        match ch[i] {
            '{' => {
                depth += 1;
                if depth > equashion_math::MAX_DEPTH {
                    return None;
                }
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn notation_quote(ch: &[char], i: usize) -> bool {
    matches!(ch[i], '"' | '`')
        || (ch[i] == '\''
            && (i == 0
                || ch[i - 1].is_whitespace()
                || matches!(
                    ch[i - 1],
                    '[' | '{' | '(' | ',' | ';' | '=' | '+' | '-' | '*' | '/'
                )))
}

/// Protect code/quoted strings; a prime after a symbol, as in f'(x), is not a quote.
fn notation_quote_end(ch: &[char], start: usize) -> usize {
    let quote = ch[start];
    let width = if quote == '`' {
        ch[start..].iter().take_while(|&&c| c == '`').count()
    } else {
        1
    };
    let mut i = start + width;
    while i < ch.len() {
        if quote != '`' && ch[i] == '\\' {
            i = (i + 2).min(ch.len());
            continue;
        }
        if ch[i..].starts_with(&ch[start..start + width]) {
            return i + width;
        }
        i += 1;
    }
    ch.len()
}

/// Keep an existing TeX environment intact, including nested environments and literal bodies.
fn notation_environment_end(ch: &[char], command_end: usize) -> usize {
    let start = notation_space(ch, command_end);
    let Some(end) = notation_braces_end(ch, start) else {
        return ch.len();
    };
    let mut stack = vec![ch[start + 1..end - 1].iter().collect::<String>()];
    let mut i = end;
    while i < ch.len() {
        if ch[i] != '\\' {
            i += 1;
            continue;
        }
        let (name, next) = notation_command(ch, i);
        if matches!(name.as_str(), "begin" | "end") {
            let open = notation_space(ch, next);
            let Some(close) = notation_braces_end(ch, open) else {
                return ch.len();
            };
            let env: String = ch[open + 1..close - 1].iter().collect();
            if name == "begin" {
                stack.push(env);
                if stack.len() > equashion_math::MAX_DEPTH {
                    return ch.len();
                }
            } else if stack.pop().as_ref() != Some(&env) {
                return ch.len();
            } else if stack.is_empty() {
                return close;
            }
            i = close;
        } else {
            i = notation_protected_end(ch, i).unwrap_or(next.max(i + 1));
        }
    }
    ch.len()
}

fn notation_protected_end(ch: &[char], start: usize) -> Option<usize> {
    if notation_quote(ch, start) {
        return Some(notation_quote_end(ch, start));
    }
    if ch[start] != '\\' {
        return None;
    }
    let (command, end) = notation_command(ch, start);
    if command.is_empty() {
        return Some((start + 2).min(ch.len()));
    }
    if command == "begin" {
        return Some(notation_environment_end(ch, end));
    }
    if literal_math_command(&command) {
        let open = notation_space(ch, end);
        return Some(if ch.get(open) == Some(&'{') {
            notation_braces_end(ch, open).unwrap_or(ch.len())
        } else {
            end
        });
    }
    Some(end)
}

fn notation_group_end(ch: &[char], start: usize) -> Option<usize> {
    let mut stack = Vec::new();
    let mut i = start;
    while i < ch.len() {
        if let Some(end) = notation_protected_end(ch, i) {
            i = end;
            continue;
        }
        match ch[i] {
            '(' => stack.push(')'),
            '[' => stack.push(']'),
            '{' => stack.push('}'),
            ')' | ']' | '}' => {
                if stack.pop() != Some(ch[i]) {
                    return None;
                }
                if stack.is_empty() {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
        if stack.len() > equashion_math::MAX_DEPTH {
            return None;
        }
        i += 1;
    }
    None
}

fn nested_row_cells(ch: &[char], start: usize, end: usize) -> Option<Vec<String>> {
    let mut cells = Vec::new();
    let mut stack = Vec::new();
    let mut cell_start = start;
    let mut i = start;
    while i <= end {
        if i == end || (ch[i] == ',' && stack.is_empty()) {
            let cell: String = ch[cell_start..i].iter().collect();
            if cell.trim().is_empty() {
                return None;
            }
            cells.push(cell);
            cell_start = i + 1;
            i += 1;
            continue;
        }
        if notation_quote(ch, i) {
            return None;
        }
        if ch[i] == '\\' {
            let (command, _) = notation_command(ch, i);
            if matches!(command.as_str(), "begin" | "end") {
                return None;
            }
        }
        if let Some(next) = notation_protected_end(ch, i) {
            if next > end {
                return None;
            }
            i = next;
            continue;
        }
        match ch[i] {
            '[' => {
                // A cell which is itself a row/list, or contains another nested-row alias,
                // is higher-dimensional data. Bracket indices/root degrees remain ordinary math.
                if i == notation_space(ch, cell_start)
                    || ch.get(notation_space(ch, i + 1)) == Some(&'[')
                {
                    return None;
                }
                stack.push(']');
            }
            '(' => stack.push(')'),
            '{' => stack.push('}'),
            ')' | ']' | '}' => {
                if stack.pop() != Some(ch[i]) {
                    return None;
                }
            }
            _ => {}
        }
        if stack.len() > equashion_math::MAX_DEPTH {
            return None;
        }
        i += 1;
    }
    stack.is_empty().then_some(cells)
}

fn nested_matrix_at(ch: &[char], start: usize, end: usize) -> Option<String> {
    let mut rows = Vec::new();
    let mut columns: Option<usize> = None;
    let mut i = notation_space(ch, start + 1);
    while i < end - 1 {
        if ch[i] != '[' {
            return None;
        }
        let row_end = notation_group_end(ch, i)?;
        if row_end >= end {
            return None;
        }
        let cells = nested_row_cells(ch, i + 1, row_end - 1)?;
        if columns.is_some_and(|n| n != cells.len()) {
            return None;
        }
        columns = Some(cells.len());
        // Braces are invisible grouping. They prevent a cell's own comma/&/row-like syntax
        // from becoming separators in the existing bmatrix environment grammar.
        rows.push(
            cells
                .iter()
                .map(|s| format!("{{{s}}}"))
                .collect::<Vec<_>>()
                .join("&"),
        );
        i = notation_space(ch, row_end);
        if i == end - 1 {
            break;
        }
        if ch[i] != ',' {
            return None;
        }
        i = notation_space(ch, i + 1);
        if i == end - 1 {
            return None;
        }
    }
    (!rows.is_empty()).then(|| format!("\\begin{{bmatrix}}{}\\end{{bmatrix}}", rows.join("\\\\")))
}

/// Within math content only, `[[a,b],[c,d]]` is a rectangular matrix alias. No numeric
/// interpretation is involved. Existing TeX/literal/code spans and ordinary bracket groups
/// are protected. Invalid/empty/ragged/3-D candidates are copied as a whole, never partly fixed.
/// Explicit bmatrix keeps even 1x1, 1xN and Nx1 shapes; `[a,b]` alone is not a Matrix AST.
fn nested_matrix_aliases(content: &str) -> String {
    let ch: Vec<char> = content.chars().collect();
    let mut out = String::with_capacity(content.len());
    let mut i = 0;
    while i < ch.len() {
        if let Some(end) = notation_protected_end(&ch, i) {
            out.extend(&ch[i..end]);
            i = end;
        } else if ch[i] == '[' {
            let Some(end) = notation_group_end(&ch, i) else {
                out.extend(&ch[i..]);
                break;
            };
            let is_rows = ch.get(notation_space(&ch, i + 1)) == Some(&'[');
            if let Some(matrix) = is_rows.then(|| nested_matrix_at(&ch, i, end)).flatten() {
                out.push_str(&matrix);
            } else {
                out.extend(&ch[i..end]);
            }
            i = end;
        } else {
            out.push(ch[i]);
            i += 1;
        }
    }
    out
}

/// Preserve printed notation that the pinned linear parser otherwise flattens. In its
/// typed format `sin(x)` consumes the parentheses as syntax; a second group makes the
/// original pair a real Delim in the function's body. Only balanced, explicit bare trig
/// calls are adapted. TeX commands (including text arguments) keep their original input.
/// The three legacy vector tokens are accepted only as separate words, with paired
/// angle delimiters and a space-delimited cross operator; identifiers stay untouched.
fn source_linear_notation(content: &str, preserve_grouping: bool) -> String {
    let matrix_content = nested_matrix_aliases(content);
    let ch: Vec<char> = matrix_content.chars().collect();
    let mut closes = vec![None; ch.len()];
    let mut stack = Vec::new();
    for (i, &c) in ch.iter().enumerate() {
        if c == '(' {
            stack.push(i);
        } else if c == ')'
            && let Some(open) = stack.pop()
        {
            closes[open] = Some(i);
        }
    }
    let mut before = vec![false; ch.len()];
    let mut after = vec![false; ch.len()];
    let mut replacements: Vec<(usize, usize, &'static str)> = Vec::new();
    let mut angles = Vec::new();
    let mut i = 0;
    while i < ch.len() {
        // Do not reinterpret a valid command name or a command's literal text body.
        if ch[i] == '\\' {
            i += 1;
            let start = i;
            while i < ch.len() && ch[i].is_ascii_alphabetic() {
                i += 1;
            }
            let command: String = ch[start..i].iter().collect();
            if literal_math_command(&command) {
                while i < ch.len() && ch[i].is_whitespace() {
                    i += 1;
                }
                if ch.get(i) == Some(&'{') {
                    let mut depth = 1;
                    i += 1;
                    while i < ch.len() && depth > 0 {
                        match ch[i] {
                            '{' => depth += 1,
                            '}' => depth -= 1,
                            _ => {}
                        }
                        i += 1;
                    }
                }
            } else if i == start {
                i += usize::from(i < ch.len());
            }
            continue;
        }
        if !ch[i].is_ascii_alphabetic() {
            i += 1;
            continue;
        }
        let start = i;
        while i < ch.len() && ch[i].is_ascii_alphabetic() {
            i += 1;
        }
        let word: String = ch[start..i].iter().collect();
        if preserve_grouping
            && matches!(word.as_str(), "sin" | "cos" | "tan" | "cot" | "sec" | "csc")
            && ch.get(i) == Some(&'(')
            && let Some(close) = closes[i]
        {
            after[i] = true;
            before[close] = true;
        }
        let separate = (start == 0 || !ch[start - 1].is_alphanumeric() && ch[start - 1] != '_')
            && (i == ch.len() || !ch[i].is_alphanumeric() && ch[i] != '_');
        if !separate {
            continue;
        }
        match word.as_str() {
            "xx" if start > 0
                && i < ch.len()
                && ch[start - 1].is_whitespace()
                && ch[i].is_whitespace() =>
            {
                replacements.push((start, i, "×"))
            }
            "langle" => angles.push((start, i)),
            "rangle" => {
                if let Some((open, end)) = angles.pop() {
                    replacements.push((open, end, "⟨"));
                    replacements.push((start, i, "⟩"));
                }
            }
            _ => {}
        }
    }
    replacements.sort_unstable_by_key(|r| r.0);
    let mut replacements = replacements.into_iter().peekable();
    let mut out = String::with_capacity(content.len());
    let mut i = 0;
    while i < ch.len() {
        if let Some(&(start, end, replacement)) = replacements.peek()
            && start == i
        {
            out.push_str(replacement);
            i = end;
            replacements.next();
            continue;
        }
        if before[i] {
            out.push(')');
        }
        out.push(ch[i]);
        if after[i] {
            out.push('(');
        }
        i += 1;
    }
    out
}

// ---------- plain text ----------

const SUP: &str = "0123456789+-=()ni";
const SUPC: &str = "⁰¹²³⁴⁵⁶⁷⁸⁹⁺⁻⁼⁽⁾ⁿⁱ";
const SUB: &str = "0123456789+-=()";
const SUBC: &str = "₀₁₂₃₄₅₆₇₈₉₊₋₌₍₎";

fn script_run(rest: &[char], from: &str, to: &str) -> (String, usize) {
    let f: Vec<char> = from.chars().collect();
    let t: Vec<char> = to.chars().collect();
    let mut out = String::new();
    let mut n = 0;
    for (k, &c) in rest.iter().enumerate() {
        let c2 = if c == '−' { '-' } else { c };
        // a sign only belongs to the script when it leads it (x^-1, not x^2-9)
        if k > 0 && matches!(c2, '+' | '-' | '=' | '(' | ')') {
            break;
        }
        match f.iter().position(|x| *x == c2) {
            Some(i) => {
                out.push(t[i]);
                n += 1;
            }
            None => break,
        }
    }
    (out, n)
}

/// Unicode approximation of one equation: equaSHion's own linear print, with simple
/// exponents and subscripts turned into superscript/subscript characters.
pub fn to_plain_text(content: &str) -> String {
    // Linear printing supplies its own function-call parentheses. The extra display
    // group used by `parse` would otherwise produce a duplicate pair in plain previews.
    let Some(m) = parse_notation(content, false) else {
        return content.to_string();
    };
    let lin = to_linear(&m);
    let ch: Vec<char> = lin.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < ch.len() {
        let (map_from, map_to) = match ch[i] {
            '^' => (SUP, SUPC),
            '_' => (SUB, SUBC),
            _ => {
                out.push(ch[i]);
                i += 1;
                continue;
            }
        };
        let (s, n) = script_run(&ch[i + 1..], map_from, map_to);
        if n > 0 {
            out.push_str(&s);
            i += 1 + n;
        } else {
            out.push(ch[i]);
            i += 1;
        }
    }
    out
}

/// Plain-text rendering of a whole stem (equations converted with [`to_plain_text`]), for
/// exports and cards that cannot draw math. Never shows raw `$` delimiters for valid math.
pub fn stem_to_plain(text: &str) -> String {
    rich_segments(text)
        .into_iter()
        .map(|s| match s {
            Segment::Text(t) => t,
            Segment::Math(m) => to_plain_text(&m),
        })
        .collect()
}

/// One-line preview of a stem for cards, lists and search: markup flattened
/// (`richtext::plain`) and equations as their Unicode approximation.
pub fn stem_preview(text: &str) -> String {
    stem_to_plain(&scripts_to_unicode(&crate::richtext::plain(text)))
}

/// `_{2}` / `^{2+}` (left in by `richtext::plain`) as Unicode sub/superscripts where every
/// character has one, else the bare text ("x_{ab}" -> "x_ab").
fn scripts_to_unicode(s: &str) -> String {
    let ch: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let mut in_math = false;
    while i < ch.len() {
        let c = ch[i];
        if c == '$' {
            in_math = !in_math;
        }
        if !in_math
            && (c == '_' || c == '^')
            && ch.get(i + 1) == Some(&'{')
            && let Some(end) = (i + 2..ch.len()).find(|&j| ch[j] == '}')
        {
            let inner: String = ch[i + 2..end].iter().collect();
            match crate::richtext::unicode_script(&inner, c == '^') {
                Some(u) => out.push_str(&u),
                None => {
                    out.push(c);
                    out.push_str(&inner);
                }
            }
            i = end + 1;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

#[cfg(test)]
mod notation_tests {
    use super::*;
    use equashion_math::Node;

    #[test]
    fn explicit_trig_groups_retain_all_original_argument_nodes() {
        for (name, arg) in [
            ("sin", "θ-(5π)/(6)"),
            ("sin", "-3θ-(π)/(2)"),
            ("cos", "(θ)/(4)+(π)/(4)"),
            ("sin", "3θ"),
            ("cos", "2θ"),
        ] {
            let source = format!("{name}({arg})");
            assert_eq!(
                parse(&source).unwrap().body,
                vec![Node::Func {
                    name: name.into(),
                    body: vec![Node::Delim {
                        open: "(".into(),
                        close: ")".into(),
                        sep: "|".into(),
                        items: vec![from_linear(arg).body]
                    }]
                }],
                "{source}"
            );
            assert_eq!(
                to_plain_text(&source),
                to_linear(&from_linear(&source)),
                "plain preview has exactly the source pair"
            );
        }
        assert_eq!(
            parse("sin x"),
            Some(from_linear("sin x")),
            "no parentheses invented for bare arguments"
        );
        assert_eq!(
            parse("sine(x)"),
            Some(from_linear("sine(x)")),
            "longer identifiers stay literal"
        );
        let nested = parse("sin(cos(x)+1)").unwrap();
        assert!(format!("{nested:?}").matches("Delim").count() == 2);
    }

    #[test]
    fn legacy_vector_aliases_preserve_given_values_and_existing_tex() {
        for (source, canonical) in [
            ("vec(a) xx vec(b)", r"vec(a) \times vec(b)"),
            (
                "vec(a) = langle -9, 5, -3 rangle",
                r"vec(a) = \langle -9, 5, -3 \rangle",
            ),
            (
                "vec(b) = langle -7, -1, -8 rangle",
                r"vec(b) = \langle -7, -1, -8 \rangle",
            ),
        ] {
            assert_eq!(parse(source), Some(from_linear(canonical)), "{source}");
        }
        for source in [
            r"\sin{x}",
            r"\sin\left(x+1\right)",
            r"\langle -9,5,-3\rangle",
            r"\text{xx sin(x) langle rangle}",
            r"\textit{xx sin(x)}",
            "xx_1+maxx",
            "langle x",
            "my_langle x my_rangle",
            "sin(x",
        ] {
            assert_eq!(
                parse(source),
                Some(from_linear(source)),
                "existing syntax stays untouched: {source}"
            );
        }
    }

    fn only_matrix_rows(source: &str) -> Vec<Vec<equashion_math::Seq>> {
        let m = parse(source).expect("supported equation");
        let [
            Node::Delim {
                open, close, items, ..
            },
        ] = m.body.as_slice()
        else {
            panic!("Expected bracketed matrix: {m:?}");
        };
        assert_eq!((open.as_str(), close.as_str()), ("[", "]"));
        let [body] = items.as_slice() else {
            panic!("One matrix body");
        };
        let [Node::Matrix { rows }] = body.as_slice() else {
            panic!("A true Matrix node is required: {body:?}");
        };
        rows.clone()
    }

    fn matrix_shapes(m: &Math) -> Vec<(usize, usize)> {
        fn visit(v: &serde_json::Value, shapes: &mut Vec<(usize, usize)>) {
            if let Some(rows) = v
                .pointer("/Matrix/rows")
                .and_then(serde_json::Value::as_array)
            {
                shapes.push((rows.len(), rows[0].as_array().unwrap().len()));
            }
            match v {
                serde_json::Value::Object(o) => {
                    for child in o.values() {
                        visit(child, shapes);
                    }
                }
                serde_json::Value::Array(a) => {
                    for child in a {
                        visit(child, shapes);
                    }
                }
                _ => {}
            }
        }
        let mut shapes = Vec::new();
        visit(&serde_json::to_value(m).unwrap(), &mut shapes);
        shapes
    }

    #[test]
    fn rectangular_aliases_build_exact_matrix_shapes_and_ordered_cells() {
        for (source, cells) in [
            ("[[a,b],[c,d]]", vec![vec!["a", "b"], vec!["c", "d"]]),
            ("[[x]]", vec![vec!["x"]]),
            ("[[a,b,c]]", vec![vec!["a", "b", "c"]]),
            ("[[a],[b],[c]]", vec![vec!["a"], vec!["b"], vec!["c"]]),
            (
                r"[ [ -3, x_1, (a+b)/(c-d) ], [ \alpha, f(s,t), \frac{u+v}{w} ] ]",
                vec![
                    vec!["-3", "x_1", "(a+b)/(c-d)"],
                    vec![r"\alpha", "f(s,t)", r"\frac{u+v}{w}"],
                ],
            ),
            (
                r"[[\sqrt[3]{z},h'(x)],[q[i],\text{a,b}]]",
                vec![vec![r"\sqrt[3]{z}", "h'(x)"], vec!["q[i]", r"\text{a,b}"]],
            ),
            (
                r"[[sin(x),\frac{1}{2}],[(u)/(v),cos(y+1)]]",
                vec![vec!["sin(x)", r"\frac{1}{2}"], vec!["(u)/(v)", "cos(y+1)"]],
            ),
            (
                r"[[\text{[[r,s],[t,u]]},x],[y,z]]",
                vec![vec![r"\text{[[r,s],[t,u]]}", "x"], vec!["y", "z"]],
            ),
        ] {
            let expected: Vec<Vec<_>> = cells
                .iter()
                .map(|row| row.iter().map(|cell| parse(cell).unwrap().body).collect())
                .collect();
            assert_eq!(
                only_matrix_rows(source),
                expected,
                "Every cell stays in its source row/column: {source}"
            );
            assert_eq!(
                matrix_shapes(&parse(source).unwrap()),
                vec![(cells.len(), cells[0].len())]
            );
            assert_eq!(
                nested_matrix_aliases(&nested_matrix_aliases(source)),
                nested_matrix_aliases(source),
                "Alias adaptation is idempotent."
            );
        }
        assert!(
            matrix_shapes(&from_linear("[[a,b],[c,d]]")).is_empty(),
            "The pinned parser alone treats nested rows as delimiter groups."
        );
        assert!(
            matrix_shapes(&from_linear("[a,b]")).is_empty(),
            "A one-row alias cannot be flattened to an ordinary bracket list."
        );
    }

    #[test]
    fn matrix_aliases_preserve_surrounding_scalar_and_matrix_operations_without_evaluation() {
        for (source, explicit) in [
            (
                "3[[1,2],[3,4]]-[[a],[b]]/2+A*[[x,y]]",
                r"3\begin{bmatrix}1&2\\3&4\end{bmatrix}-\begin{bmatrix}a\\b\end{bmatrix}/2+A*\begin{bmatrix}x&y\end{bmatrix}",
            ),
            (
                "[[a,b],[c,d]][[x],[y]]",
                r"\begin{bmatrix}a&b\\c&d\end{bmatrix}\begin{bmatrix}x\\y\end{bmatrix}",
            ),
            (
                "A=[[u,-v],[w,z]]+k",
                r"A=\begin{bmatrix}u&-v\\w&z\end{bmatrix}+k",
            ),
            (
                r"\frac{[[a],[b]]}{q}+[[f(x,y),r],[s,t]]",
                r"\frac{\begin{bmatrix}a\\b\end{bmatrix}}{q}+\begin{bmatrix}f(x,y)&r\\s&t\end{bmatrix}",
            ),
        ] {
            assert_eq!(
                parse(source).expect("matrix alias with surrounding operations"),
                parse(explicit).expect("existing explicit matrix grammar"),
                "No multiplication, scalar, sign, operator or source cell is evaluated/reordered: {source}"
            );
            assert_eq!(to_plain_text(source), to_plain_text(explicit));
        }
    }

    #[test]
    fn invalid_nested_rows_are_left_whole_without_rewriting_their_interior() {
        for source in [
            "[[]]",
            "[[a,],[b,c]]",
            "[[,a],[b,c]]",
            "[[a,b],[]]",
            "[[a,b],[c]]",
            "[[a,b],]",
            "[[a,b];[c,d]]",
            "[[a,b),(c,d]]",
            "[[a,b],[c,d]",
            "[[[a,b],[c,d]],[[e,f],[g,h]]]",
            "[[[x]]]",
            "[[f([[a,b],[c,d]]),z],[r,s]]",
            "[[a,b],[c,d] + [[x,y]]",
            "[q+[[a,b],[c,d]]]",
            "[[\"a,b\",c],[d,e]]",
            "[['a,b',c],[d,e]]",
            "[[`a,b`,c],[d,e]]",
        ] {
            assert_eq!(
                nested_matrix_aliases(source),
                source,
                "Unaccepted notation stays verbatim: {source}"
            );
        }
        let source = "[[a,b],[c]] + [[x,y]]";
        assert_eq!(
            nested_matrix_aliases(source),
            r"[[a,b],[c]] + \begin{bmatrix}{x}&{y}\end{bmatrix}",
            "An invalid candidate is skipped as one unit; an independent later valid matrix can still adapt."
        );
        let deep = format!(
            "[[{}x{}]]",
            "(".repeat(equashion_math::MAX_DEPTH),
            ")".repeat(equashion_math::MAX_DEPTH)
        );
        assert_eq!(
            nested_matrix_aliases(&deep),
            deep,
            "Depth guard leaves source untouched."
        );
    }

    #[test]
    fn existing_tex_native_matrices_and_literal_code_are_not_reinterpreted() {
        for source in [
            r"\begin{bmatrix}1&-2\\3&4\end{bmatrix}",
            r"\begin{pmatrix}a&b\\c&d\end{pmatrix}",
            "bmatrix(a,b;c,d)",
            "[a,b;c,d]",
            "[a,b]",
            "[x]",
            r"\text{[[a,b],[c,d]]}",
            r"\mathbf{[[a,b],[c,d]]}",
            r"\operatorname{[[x],[y]]}",
            r"\text{literal \{ [[a,b],[c,d]] \}}",
            r"\[\[a,b\],\[c,d\]\]",
            "`[[a,b],[c,d]]`",
            "``[[a,b],[c,d]]``",
            "\"[[a,b],[c,d]]\"",
            "'[[a,b],[c,d]]'",
            r"\begin{bmatrix}\text{[[a,b],[c,d]]}&x\\y&z\end{bmatrix}",
            r"\begin{bmatrix}\begin{pmatrix}[[a,b],[c,d]]\end{pmatrix}\end{bmatrix}",
        ] {
            assert_eq!(
                nested_matrix_aliases(source),
                source,
                "Existing/literal notation stays untouched: {source}"
            );
        }
        for source in [
            r"\begin{bmatrix}1&-2\\3&4\end{bmatrix}",
            "bmatrix(a,b;c,d)",
            "[a,b;c,d]",
            "[a,b]",
            "[x]",
        ] {
            assert_eq!(
                parse(source),
                Some(from_linear(source)),
                "Existing supported AST unchanged: {source}"
            );
        }
        assert_eq!(
            nested_matrix_aliases(r"\text{[[a,b],[c,d]]}+[[x]]"),
            r"\text{[[a,b],[c,d]]}+\begin{bmatrix}{x}\end{bmatrix}"
        );
        assert_eq!(
            nested_matrix_aliases("`[[a,b],[c,d]]`+[[x]]"),
            r"`[[a,b],[c,d]]`+\begin{bmatrix}{x}\end{bmatrix}"
        );
    }

    #[test]
    fn matrix_alias_parsing_is_math_only_and_does_not_rewrite_imported_source() {
        let text = "Plain [[a,b],[c,d]] stays text.";
        assert_eq!(stem_to_plain(text), text);
        let stem = "Keep raw $A=[[x,2],[3,y]]$.";
        let raw=serde_json::json!({"format":crate::ditto::FORMAT,"bank":{"name":"Synthetic matrices","questions":[{"id":"q1","type":"short-answer","stem":stem,"suggestedAnswer":""}]}}).to_string();
        let imported = crate::ditto::import_text(&raw, None, 1, "Synthetic").unwrap();
        assert_eq!(
            imported.doc.questions[0].stem, stem,
            "Persistence keeps literal original notation."
        );
        let math = rich_segments(stem)
            .into_iter()
            .find_map(|s| match s {
                Segment::Math(m) => Some(m),
                _ => None,
            })
            .unwrap();
        assert_eq!(math, "A=[[x,2],[3,y]]");
        assert_eq!(matrix_shapes(&parse(&math).unwrap()), vec![(2, 2)]);
    }

    #[test]
    fn matrix_alias_omml_contains_real_rows_and_cells_and_roundtrips_their_order() {
        for (source, shape) in [
            ("[[a,b],[c,d]]", (2, 2)),
            ("[[u,v,w]]", (1, 3)),
            ("[[x],[y],[z]]", (3, 1)),
            ("[[q]]", (1, 1)),
        ] {
            let math = parse(source).unwrap();
            let xml = equashion_math::to_omml(&math);
            let doc = roxmltree::Document::parse(&xml).unwrap();
            let matrices: Vec<_> = doc
                .descendants()
                .filter(|n| n.has_tag_name((equashion_math::OMML_NS, "m")))
                .collect();
            assert_eq!(matrices.len(), 1);
            let rows: Vec<_> = matrices[0]
                .children()
                .filter(|n| n.has_tag_name((equashion_math::OMML_NS, "mr")))
                .collect();
            assert_eq!(rows.len(), shape.0);
            for row in rows {
                assert_eq!(
                    row.children()
                        .filter(|n| n.has_tag_name((equashion_math::OMML_NS, "e")))
                        .count(),
                    shape.1
                );
            }
            let reparsed = equashion_math::from_omml(&xml);
            assert_eq!(to_linear(&reparsed), to_linear(&math));
            assert_eq!(matrix_shapes(&reparsed), vec![shape]);
        }
    }
}
