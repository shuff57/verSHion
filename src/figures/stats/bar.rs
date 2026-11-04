//! Bar chart for categorical data: one bar per category (its count, or its percent of the group),
//! with gaps between the bars because the categories are separate groups, not intervals on a
//! number line (that is a histogram, whose bars touch). An optional second count column draws two
//! groups side by side. Original design on the shared pieces (`chart` ticks, `Fills`, `show_answers`).
//!
//! The data is typed text, one category per line (`Walk, 12`, or `Walk, 12, 9` for two groups),
//! not `Kind::Data`: that control tidies a paste down to numbers and would wipe the names.
//!
//! Layout is worked out once from the data and the options, never from Show answers, so the
//! student copy and the key are the same size and every label has a reserved place: value labels
//! sit in their own bar's column (above the bar, or above the cumulative line there), category
//! names in their own slot (wrapped, or turned a quarter turn when a word does not fit).
use crate::figures::chart::{self, GRID, INK};
use crate::figures::svg::{self, esc, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub const ID: &str = "bar-chart";
/// Most categories drawn (after repeated names are merged).
pub const MAX_CATS: usize = 30;
/// Longest category or group name, in characters.
pub const MAX_NAME: usize = 40;
/// Largest count on one line.
pub const MAX_COUNT: u64 = 1_000_000_000;
/// Longest chart title, in characters (axis titles: [`MAX_AXIS_TITLE`]).
pub const MAX_TITLE: usize = 150;
pub const MAX_AXIS_TITLE: usize = 80;

pub const COUNT: &str = "Frequency (count)";
pub const PERCENT: &str = "Relative frequency (percent)";
pub const ORDERS: [&str; 5] = ["As typed", "Tallest first", "Shortest first", "A to Z", "Pareto (tallest first, cumulative line)"];
pub const PARETO: &str = ORDERS[4];
pub const BLANKS: [&str; 3] = ["The values on the bars", "The bars", "The scale numbers"];
const FONT: &str = "Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif";
/// Print's bars and gridlines. The shared Print palette's first series is a plain `#d9d9d9` wash
/// and its grid is `#cccccc`: both are lighter than a 66% bitonal copy's threshold, so a bar
/// chart, whose fill carries the series and whose grid the reading, uses these (grey only, as
/// Print requires). The grid is 3 px so that a 58% or 66% copy always covers a whole pixel,
/// whatever subpixel offset the line lands on (a 2.4 px line vanished on some offsets); the bars
/// are darker still, so a bar stays apart from the grid in a copy.
const PRINT_BAR: &str = "#7a7a7a";
const PRINT_GRID: &str = "#8c8c8c";
const PRINT_GRID_W: f64 = 3.0;
/// Glyph extent above and below the baseline, and line pitch, in em (Noto Sans, with room for accents).
const ASC: f64 = 0.95;
const DES: f64 = 0.3;
const PITCH: f64 = 1.3;

pub fn def() -> FigureDef {
    FigureDef {
        id: ID,
        subject: Subject::Math,
        title: "Bar Chart",
        blurb: "A bar chart of categories and their counts or percents, with gaps between the bars: one group or two side by side, in typed, sorted or Pareto order.",
        params,
        render,
    }
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("data", "Categories and counts", "Walk, 12\nCar, 9\nBus, 6\nBike, 3")
            .hint("One category per line: its name, a comma, then how many. A second count after another comma compares two groups. A comma between groups of three digits (1,200, no space after it) or spaces between them (1 200) are thousands groups, so Walk, 1,200 is 1200. A first line of headings (Way, Boys, Girls) names the axis and the groups"),
        ParamDef::text("name1", "First group's name", "").hint("Shown in the legend when there are two counts per line; blank uses the headings line, or Group 1"),
        ParamDef::text("name2", "Second group's name", "").hint("Blank uses the headings line, or Group 2"),
        ParamDef::choice("orient", "Bars", &["Vertical", "Horizontal"], "Vertical"),
        ParamDef::choice("measure", "Bar height", &[COUNT, PERCENT], COUNT).hint("Relative frequency is each category's percent of its group's total, so each group's bars add to 100%"),
        ParamDef::choice("order", "Order", &ORDERS, "As typed").hint("Categories have no natural order unless they are ordered words (like Never, Sometimes, Often): keep those as typed"),
        ParamDef::choice("gap", "Gap between bars", &["Narrow", "Medium", "Wide"], "Medium").hint("Bars of a bar chart never touch: the categories are separate"),
        ParamDef::bool("values", "Write the value on each bar", true),
        ParamDef::int("decimals", "Decimals in percents", 0, 2, 0),
        ParamDef::bool("scale_auto", "Choose the scale automatically", true),
        ParamDef::float("scale_min", "Scale start", 0.0, 1e9, 1.0, 0.0).hint("Anything above 0 cuts the bars short, so their lengths are no longer in proportion"),
        ParamDef::float("scale_max", "Scale top", 0.0, 1e12, 1.0, 20.0),
        ParamDef::bool("grid", "Gridlines", true),
        ParamDef::text("title", "Title", ""),
        ParamDef::text("cat_title", "Category axis title", ""),
        ParamDef::text("val_title", "Value axis title (blank = the bar height)", ""),
        ParamDef::choice("blank", "Student copy leaves out", &BLANKS, BLANKS[0])
            .hint("The bars: students draw them from a table of the counts. The scale numbers: students number the axis from the values on the bars"),
        ParamDef::bool("questions", "Questions under the chart", false).hint("Three numbered questions; the key answers them"),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off makes the student copy: what is left out is set by Student copy leaves out; the cumulative line and the answers to the questions are always left out"),
    ]
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        (ID, "scale_min" | "scale_max") => !p.b("scale_auto"),
        (ID, "decimals") => p.s("measure") == PERCENT || p.b("questions"),
        (ID, "name1" | "name2") => parse(p.s("data")).is_ok_and(|t| t.groups == 2),
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        ID => vec![
            Example { name: "Getting to school", params: vec![] },
            Example {
                name: "Two classes, percents",
                params: vec![
                    ("data", s("Pet, Class A, Class B\nDog, 11, 8\nCat, 7, 9\nFish, 3, 2\nNone, 4, 9")),
                    ("measure", s(PERCENT)),
                    ("title", s("Pets owned by two classes")),
                ],
            },
            Example {
                name: "Pareto: late homework",
                params: vec![
                    ("data", s("Forgot, 14\nNo time, 9\nToo hard, 5\nSick, 3\nOther, 2")),
                    ("order", s(PARETO)),
                    ("title", s("Reasons homework was late")),
                    ("cat_title", s("Reason")),
                ],
            },
            Example {
                name: "Long names, sideways",
                params: vec![
                    ("data", s("Basketball, 8\nAmerican football, 6\nSoccer (association football), 11\nTrack and field, 4\nSwimming and diving, 3\nVolleyball, 5")),
                    ("orient", s("Horizontal")),
                    ("order", s("Tallest first")),
                    ("cat_title", s("Favourite sport")),
                ],
            },
            Example {
                name: "Misleading: axis starts at 40",
                params: vec![
                    ("data", s("Brand A, 46\nBrand B, 44\nBrand C, 42")),
                    ("scale_auto", Value::B(false)),
                    ("scale_min", Value::F(40.0)),
                    ("scale_max", Value::F(48.0)),
                    ("title", s("Is Brand A really twice as popular?")),
                ],
            },
        ],
        _ => return None,
    })
}

// ───────────────────────── reading the data ─────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub struct Cat {
    pub name: String,
    /// One count per group.
    pub counts: Vec<u64>,
    /// The (1-based) line it was first typed on.
    pub line: usize,
}

/// Categories typed more than once and merged: the name as first typed, its lines, and each
/// line's counts.
#[derive(Clone, Debug, PartialEq)]
pub struct Merge {
    pub name: String,
    pub lines: Vec<usize>,
    pub parts: Vec<Vec<u64>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub cats: Vec<Cat>,
    /// 1 or 2 counts per category.
    pub groups: usize,
    /// A first line of headings: its line number, the text as typed, and its cells (empty when
    /// it had no separators to split it).
    pub head: Option<(usize, String, Vec<String>)>,
    pub merged: Vec<Merge>,
    /// Lines whose count was read out of a thousands group that could also be two counts (or a
    /// name and a count): the line, its text, the other reading's cells, and the count used.
    pub ambiguous: Vec<(usize, String, Vec<String>, u64)>,
    /// Lines with a comma inside a number but no three digits after it: line, text, its two sides.
    pub short_comma: Vec<(usize, String, u64, u64)>,
}

impl Table {
    pub fn total(&self, g: usize) -> u64 {
        self.cats.iter().map(|c| c.counts[g]).sum()
    }
}

/// A token that reads as a number (it starts like one: `inf` and `NaN` are words here).
fn numeric_like(t: &str) -> bool {
    let t = t.trim().replace('\u{2212}', "-");
    let t = t.trim_end_matches('%').trim();
    t.chars().next().is_some_and(|c| c.is_ascii_digit() || matches!(c, '+' | '-' | '.')) && t.parse::<f64>().is_ok()
}

/// The value of a count written in words, up to `nine hundred ninety-nine thousand ...`
/// (`twelve`, `twenty-five`, `two hundred`). `None` when any part is not a number word, as in
/// `Two-year-olds`: that is a name, and guessing a count for it would invent data.
pub fn word_value(c: &str) -> Option<u64> {
    const ONES: [&str; 20] = [
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve", "thirteen", "fourteen", "fifteen", "sixteen", "seventeen",
        "eighteen", "nineteen",
    ];
    const TENS: [(&str, u64); 8] = [("twenty", 20), ("thirty", 30), ("forty", 40), ("fifty", 50), ("sixty", 60), ("seventy", 70), ("eighty", 80), ("ninety", 90)];
    let text = c.trim().to_lowercase();
    if text == "dozen" {
        return Some(12);
    }
    let parts: Vec<&str> = text.split([' ', '-']).filter(|p| !p.is_empty()).collect();
    if parts.is_empty() || parts.len() > 4 {
        return None;
    }
    let (mut total, mut cur) = (0u64, 0u64);
    for p in parts {
        let v = if p == "none" {
            0
        } else if let Some(i) = ONES.iter().position(|w| *w == p) {
            i as u64
        } else if let Some((_, t)) = TENS.iter().find(|(w, _)| *w == p) {
            *t
        } else if p == "hundred" || p == "thousand" {
            let scale = if p == "hundred" { 100 } else { 1000 };
            cur = cur.max(1) * scale;
            if scale == 1000 {
                total += cur;
                cur = 0;
            }
            continue;
        } else {
            return None;
        };
        cur += v;
    }
    Some(total + cur)
}

/// A count written with a comma inside it: `12,5` (a decimal comma) or a broken thousands group.
/// `None` when the cell has no comma.
fn comma_count(c: &str) -> Option<(u64, u64)> {
    let (a, b) = c.split_once(',')?;
    let (a, b) = (a.trim().replace('\u{2212}', "-"), b.trim().to_string());
    if !a.chars().all(|ch| ch.is_ascii_digit()) || !b.chars().all(|ch| ch.is_ascii_digit()) || a.is_empty() || b.is_empty() {
        return None;
    }
    Some((a.parse().unwrap_or(0), b.parse().unwrap_or(0)))
}

/// Whether the line's quotes pair up. An unmatched one is a literal character (`55" TV`), not a
/// delimiter: treating it as one hid the line's count and made the advice fold the count into the
/// name (MAJOR).
fn balanced_quotes(line: &str) -> bool {
    line.matches('"').count() % 2 == 0
}

fn csv_cells(line: &str, sep: char) -> Vec<String> {
    let (mut out, mut cur, mut quoted) = (Vec::new(), String::new(), false);
    let quotes = balanced_quotes(line);
    for ch in line.chars() {
        match ch {
            '"' if quotes => quoted = !quoted,
            c if c == sep && !quoted => out.push(std::mem::take(&mut cur)),
            _ => cur.push(ch),
        }
    }
    out.push(cur);
    out
}

/// A count typed with thousands separators (`1,200`, `1 200`, `12 345 678`): its digits. One to
/// three digits, then groups of exactly three, all after the same separator, so `1, 200` (a
/// space after the comma) is not one: that comma starts the next cell.
pub fn thousands(s: &str) -> Option<u128> {
    let t = s.trim();
    let sep = t.chars().find(|c| *c == ',' || *c == ' ')?;
    let parts: Vec<&str> = if sep == ' ' { t.split_whitespace().collect() } else { t.split(sep).collect() };
    let (first, rest) = parts.split_first()?;
    let digits = |p: &str, max: usize| !p.is_empty() && p.len() <= max && p.chars().all(|c| c.is_ascii_digit());
    if !digits(first, 3) || rest.is_empty() || !rest.iter().all(|p| p.len() == 3 && digits(p, 3)) {
        return None;
    }
    let mut v = first.parse::<u128>().ok()?;
    for p in rest {
        v = v * 1000 + p.parse::<u128>().ok()?;
        if v > u64::MAX as u128 {
            return Some(u64::MAX as u128 + 1);
        }
    }
    Some(v)
}

/// Commas inside a count written in groups of three (`1,200`), removed before the line is split,
/// so a thousands separator is part of its count and not the start of the next. A comma with a
/// space after it (`, 1`) is a cell separator, as typed data usually writes it; text inside
/// quotes is a name and is left exactly as typed.
fn join_thousands(line: &str) -> String {
    let c: Vec<char> = line.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let mut quoted = false;
    let quotes = balanced_quotes(line);
    while i < c.len() {
        if c[i] == '"' && quotes {
            quoted = !quoted;
            out.push(c[i]);
            i += 1;
            continue;
        }
        if quoted || !c[i].is_ascii_digit() {
            out.push(c[i]);
            i += 1;
            continue;
        }
        let mut j = i;
        while j < c.len() && c[j].is_ascii_digit() {
            j += 1;
        }
        let mut end = j;
        if j - i <= 3 {
            let mut k = j;
            while k + 3 < c.len() && c[k] == ',' && c[k + 1..k + 4].iter().all(|d| d.is_ascii_digit()) && !c.get(k + 4).is_some_and(|d| d.is_ascii_digit()) {
                k += 4;
                end = k;
            }
        }
        if end > j {
            out.extend(&c[i..j]);
            for part in c[j..end].chunks(4) {
                out.extend(part.iter().skip(1));
            }
            i = end;
        } else {
            out.extend(&c[i..j]);
            i = j;
        }
    }
    out
}

/// The cell a line with no separator ends in: `Walk 12`, `Walk 1 200` (a thousands group written
/// with spaces), and the other reading of that group (`Walk 1` with count 200). `Err` holds the
/// number of trailing numbers when more than one is left over and they are not groups of three
/// (`Grade 9 12`: which is the count?).
fn tail_count(line: &str) -> Result<(Vec<String>, Option<Vec<String>>), usize> {
    let toks: Vec<&str> = line.split_whitespace().collect();
    let n = toks.len();
    let group = |t: &str, max: usize| !t.is_empty() && t.len() <= max && t.chars().all(|c| c.is_ascii_digit());
    let spaced = (0..n.saturating_sub(1)).find(|&s| group(toks[s], 3) && toks[s + 1..].iter().all(|t| t.len() == 3 && group(t, 3)));
    if let Some(s) = spaced {
        let alt = if s > 0 { Some(vec![toks[..s + 1].join(" "), toks[s + 1..].concat()]) } else { None };
        return Ok((vec![toks[..s].join(" "), toks[s..].concat()], alt));
    }
    let tail = toks.iter().rev().take_while(|t| numeric_like(t)).count();
    if tail >= 2 {
        return Err(tail);
    }
    if tail == 0 {
        Ok((vec![toks.join(" ")], None))
    } else {
        Ok((vec![toks[..n - 1].join(" "), toks[n - 1].to_string()], None))
    }
}

/// A comma with no space after it and not exactly three digits after it (`12,5`, `1,2000`): the
/// digits on each side, which the line reads as two counts.
fn short_comma(line: &str) -> Option<(u64, u64)> {
    let c: Vec<char> = line.chars().collect();
    let quotes = balanced_quotes(line);
    let mut quoted = false;
    for (i, ch) in c.iter().enumerate() {
        if *ch == '"' && quotes {
            quoted = !quoted;
            continue;
        }
        if quoted || *ch != ',' || i == 0 || !c[i - 1].is_ascii_digit() {
            continue;
        }
        let mut k = i + 1;
        while k < c.len() && c[k].is_ascii_digit() {
            k += 1;
        }
        let after = k - (i + 1);
        if after == 0 || after == 3 {
            continue;
        }
        let mut s = i;
        while s > 0 && c[s - 1].is_ascii_digit() {
            s -= 1;
        }
        let left: String = c[s..i].iter().collect();
        let right: String = c[i + 1..k].iter().collect();
        return Some((left.parse().unwrap_or(0), right.parse().unwrap_or(0)));
    }
    None
}

/// The cells of a line, and the other reading a thousands group hides.
struct Cells {
    cells: Vec<String>,
    /// The cells the line would have if its thousands group were not one count (`A, 3,100` = 3 and
    /// 100; `A 1 200` = the name "A 1" with count 200), when that is a name and one or two counts.
    alt: Option<Vec<String>>,
    /// A comma inside a number that is not a thousands group (`12,5`): its two sides.
    split: Option<(u64, u64)>,
}

/// A line's cells, split at tabs, else semicolons (a comma is then a thousands or decimal mark
/// inside a cell), else commas, else the last word is the count.
fn split_cells(line: &str) -> Result<(Vec<String>, Option<Vec<String>>), usize> {
    let mut cells: Vec<String> = if line.contains('\t') {
        line.split('\t').map(str::to_string).collect()
    } else if line.contains(';') {
        csv_cells(line, ';')
    } else if line.contains(',') {
        csv_cells(line, ',')
    } else {
        return tail_count(line);
    };
    for c in cells.iter_mut() {
        *c = c.trim().trim_matches('"').trim().to_string();
    }
    while cells.len() > 1 && cells.last().is_some_and(|c| c.is_empty()) {
        cells.pop();
    }
    Ok((cells, None))
}

/// A line's cells, with the readings a thousands group hides kept so the editor can say which one
/// it used (J1: `1,200` is one count, but `A, 3,100` could also be 3 and 100).
fn split_line(line: &str) -> Result<Cells, usize> {
    let joined = join_thousands(line);
    let (cells, mut alt) = split_cells(&joined)?;
    if joined != line {
        if let Ok((a, _)) = split_cells(line) {
            if (2..=3).contains(&a.len()) {
                alt = Some(a);
            }
        }
    }
    let split = if joined == line { short_comma(line) } else { None };
    Ok(Cells { cells, alt, split })
}

/// A cell the first line may read as a group name: it holds a letter, no cell punctuation, and is
/// not a count (`12 students`, `l2`, inf, a number word). A name may hold a number (`9th grade`),
/// but a symbol, a full-width digit or `N/A` is not a name (MED 3).
fn name_cell(c: &str) -> bool {
    let t = c.trim();
    t.chars().any(|ch| ch.is_ascii_alphabetic()) && !t.chars().any(|ch| matches!(ch, ',' | ';' | '/' | '"')) && !count_like(t)
}

/// The refusal a first line gets when it is neither a line of headings nor a category with counts:
/// the bad cell is named and both routes that keep the data are offered (MED 3).
fn first_line_ambiguous(n: usize, l: &str, cell: &str, name: &str, cells: usize) -> String {
    let example = if cells >= 3 { example_line(name, &["1".to_string(), "2".to_string()]) } else { example_line(name, &["12".to_string()]) };
    format!(
        "Line {n} ({}) is the first line: {} is neither a number I can read nor a clear group name, so the line is neither a category with counts nor a line of headings. Either delete line {n} and type the group names in {} and {}, or write line {n} as a category and its counts, for example {}.",
        q(l),
        q(cell),
        q("Group 1 name"),
        q("Group 2 name"),
        q(&example)
    )
}

/// A cell the first line should read as a mistyped count rather than as a group name. A cell is a
/// number only when the whole cell is one: `12`, `12.5`, `25%`, `-3`, a number then a word
/// (`12 students`, `1 200`), a bare number word (`twelve`, but not `None`, which names an answer
/// option), an unbroken token holding a digit (`l2`), or inf/NaN. Everything else is a name, so
/// `9th grade`, `Grade 9` and `One or more` are names (MED 1).
fn count_like(c: &str) -> bool {
    let t = c.trim();
    if t.is_empty() {
        return false;
    }
    let low = t.to_lowercase();
    if matches!(low.as_str(), "inf" | "infinity" | "nan" | "-inf" | "+inf") {
        return true;
    }
    let first = t.split_whitespace().next().unwrap_or("");
    if !first.is_empty() && first.chars().all(|ch| ch.is_ascii_digit()) {
        return true;
    }
    if low == "none" {
        return false;
    }
    if parse_count(t).is_ok() || word_value(t).is_some() {
        return true;
    }
    !t.contains(' ') && t.chars().any(|ch| ch.is_ascii_digit())
}

enum CountErr {
    Missing,
    NotNumber,
    Comma,
    Percent(f64),
    Negative(f64),
    Fraction(f64),
    TooBig(f64),
}

fn parse_count(cell: &str) -> Result<u64, CountErr> {
    let c = cell.trim().replace('\u{2212}', "-");
    if c.is_empty() {
        return Err(CountErr::Missing);
    }
    if let Some(num) = c.strip_suffix('%') {
        return match numeric_like(num) {
            true => Err(CountErr::Percent(num.trim().parse::<f64>().unwrap_or(0.0))),
            false => Err(CountErr::NotNumber),
        };
    }
    if let Some(v) = thousands(&c) {
        return if v > MAX_COUNT as u128 { Err(CountErr::TooBig(v as f64)) } else { Ok(v as u64) };
    }
    if c.contains(',') {
        return Err(CountErr::Comma);
    }
    if !numeric_like(&c) {
        return Err(CountErr::NotNumber);
    }
    let v: f64 = c.parse().unwrap_or(f64::NAN);
    if v.is_nan() {
        Err(CountErr::NotNumber)
    } else if v < 0.0 {
        Err(CountErr::Negative(v))
    } else if v.is_infinite() || v > MAX_COUNT as f64 {
        Err(CountErr::TooBig(v))
    } else if v.fract() != 0.0 {
        Err(CountErr::Fraction(v))
    } else {
        Ok(v as u64)
    }
}

/// The key two names are the same category under: case and runs of spaces do not matter.
pub fn name_key(name: &str) -> String {
    name.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// A line as it could be typed back: a name holding a separator is quoted.
fn example_line(name: &str, counts: &[String]) -> String {
    let name: String = name.chars().map(|c| match c { '"' | '\u{201c}' | '\u{201d}' => '\'', '\t' => ' ', c => c }).collect();
    let name = if name.contains(',') || name.contains(';') { format!("\"{name}\"") } else { name };
    let mut s = name;
    for c in counts {
        s.push_str(", ");
        s.push_str(c);
    }
    s
}

/// Text quoted inside a message (curly quotes stand for themselves elsewhere).
fn q(s: &str) -> String {
    let t: String = s.chars().take(60).map(|c| match c { '\u{201c}' | '\u{201d}' => '"', c => c }).collect();
    let t = if s.chars().count() > 60 { format!("{t}\u{2026}") } else { t };
    format!("\u{201c}{t}\u{201d}")
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn and_list(items: &[String]) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].clone(),
        n => format!("{} and {}", items[..n - 1].join(", "), items[n - 1]),
    }
}

/// Plain digits (no thousands commas: a fix must not write a comma the reader would take for a
/// thousands separator, so a teacher retyping the fix gets the count it names).
fn digits(v: f64) -> String {
    format!("{}", v.round().clamp(0.0, MAX_COUNT as f64) as u64)
}

/// Reads the typed lines. `Err` is the first problem in line order, with a fix that works when
/// applied ("change line N to “…”").
pub fn parse(text: &str) -> Result<Table, String> {
    let lines: Vec<(usize, &str)> = text.lines().enumerate().map(|(i, l)| (i + 1, l.trim())).filter(|(_, l)| !l.is_empty()).collect();
    if lines.is_empty() {
        return Err("Type one category and its count on each line, like \u{201c}Walk, 12\u{201d}.".into());
    }
    // a first line with no numbers, followed by data, is a line of headings
    let mut head = None;
    let mut start = 0;
    if lines.len() >= 2 {
        let (n, l) = lines[0];
        if let Ok(Cells { cells, .. }) = split_line(l) {
            // a heading cell is a name: a cell is a count only if the whole cell is one, so
            // `9th grade` and `None` name groups and `12 students` is a mistyped count (MED 1)
            let words_only = cells[1..].iter().all(|c| name_cell(c));
            let separated = l.contains('\t') || l.contains(',') || l.contains(';');
            if words_only && (cells.len() >= 2 || !separated) && !cells[0].is_empty() {
                head = Some((n, l.to_string(), if separated { cells } else { Vec::new() }));
                start = 1;
            } else {
                // not headings, so it must be data: refuse it when a name sits where a count belongs
                // or when two or more cells cannot be read at all
                let bad: Vec<&String> = cells[1..].iter().filter(|c| parse_count(c).is_err() && !name_cell(c)).collect();
                let name_in_counts = cells[1..].iter().any(|c| name_cell(c));
                // a word a count could not be (`N/A`, `Jones, 3`) is no better than a number
                let looks_like_name = |c: &String| c.chars().any(|ch| ch.is_ascii_alphabetic()) && !count_like(c);
                if cells.len() >= 2 && (name_in_counts || bad.len() >= 2 || bad.iter().any(|c| looks_like_name(c))) {
                    let first_bad = bad.first().map(|c| (*c).clone()).or_else(|| cells[1..].iter().find(|c| name_cell(c)).cloned());
                    if let Some(cell) = &first_bad {
                        return Err(first_line_ambiguous(n, l, cell, &cells[0], cells.len()));
                    }
                }
            }
        }
    }
    let mut cats: Vec<Cat> = Vec::new();
    let mut merged: Vec<Merge> = Vec::new();
    let mut ambiguous: Vec<(usize, String, Vec<String>, u64)> = Vec::new();
    let mut short_commas: Vec<(usize, String, u64, u64)> = Vec::new();
    let mut groups = 0usize;
    let mut first_row: Option<(usize, String, Vec<u64>)> = None;
    let mut first_alt: Option<Vec<String>> = None;
    let mut index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut merge_index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for &(n, l) in &lines[start..] {
        let Cells { cells, alt, split } = match split_line(l) {
            Ok(c) => c,
            Err(_) => {
                let toks: Vec<&str> = l.split_whitespace().collect();
                let k = toks.len();
                let a = format!("{}, {}", toks[..k - 1].join(" "), toks[k - 1]);
                let b = format!("{}, {}, {}", toks[..k - 2].join(" "), toks[k - 2], toks[k - 1]);
                let b = if k >= 3 { format!(" or to {}", q(&b)) } else { String::new() };
                return Err(format!(
                    "Line {n} ({}) ends in more than one number, so its count is unclear. Put a comma after the name: change line {n} to {}{b}.",
                    q(l),
                    q(&a)
                ));
            }
        };
        let name = cells[0].clone();
        let raw: Vec<String> = cells[1..].to_vec();
        let want = if groups == 0 { 1 } else { groups };
        if name.is_empty() {
            let mut c = raw.clone();
            c.resize(want.max(c.len().min(2)), "0".into());
            return Err(format!("Line {n} has a count but no category name: change line {n} to {}.", q(&example_line(&format!("Category {n}"), &c))));
        }
        if raw.is_empty() {
            return Err(format!(
                "Line {n} ({}) has no count. Add how many after a comma: change line {n} to {}.",
                q(l),
                q(&example_line(&name, &vec!["0".to_string(); want]))
            ));
        }
        if raw.len() > 2 {
            // MAJOR: no rewrite. The old advice ("change line n to …" with the first two counts)
            // silently dropped the extra count; the limit is stated and the data is left alone
            return Err(format!(
                "Line {n} ({}) holds {} ({}), and this figure takes at most two counts per category: one count, or two to compare two groups. Nothing is dropped for you here: type the line with two counts, or draw the extra counts in a second chart (a second table).",
                q(l),
                plural(raw.len(), "count", "counts"),
                and_list(&raw.iter().map(|c| q(&match parse_count(c) { Ok(v) => fmt_count(v), Err(_) => c.trim().to_string() })).collect::<Vec<_>>())
            ));
        }
        let mut counts = Vec::new();
        for (i, cell) in raw.iter().enumerate() {
            let fix = |v: String| {
                let mut c = raw.clone();
                c[i] = v;
                format!("change line {n} to {}", q(&example_line(&name, &c)))
            };
            match parse_count(cell) {
                Ok(v) => counts.push(v),
                Err(CountErr::Missing) => {
                    return Err(format!("Line {n} ({}) has an empty count. Type 0 if there were none: {}.", q(l), fix("0".into())));
                }
                Err(CountErr::NotNumber) => {
                    let t = cell.trim();
                    let lead: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
                    let after = &t[lead.len()..];
                    // a count and then a word ("12 students"): the number is the count
                    if (1..=9).contains(&lead.len()) && after.starts_with(' ') && after.trim_start().chars().any(|c| c.is_alphabetic()) {
                        return Err(format!("Line {n} ({}): {} is a count followed by a word. Type the count alone: {}.", q(l), q(cell), fix(lead)));
                    }
                    // a count written as a word ("twelve"): the number-word list gives the count
                    if let Some(w) = word_value(t) {
                        return Err(format!("Line {n} ({}): {} is a number written as a word. Type it in digits: {}.", q(l), q(cell), fix(w.to_string())));
                    }
                    // nothing to go on: refuse without inventing a count (a "Fruit, 0" fix would
                    // quietly add a category the teacher never typed)
                    return Err(format!(
                        "Line {n} ({}): {} is not a number. Type how many in digits after the name and a comma, for example {}.",
                        q(l),
                        q(cell),
                        q(&example_line(&name, &["12".to_string()]))
                    ));
                }
                Err(CountErr::Comma) => {
                    let two = comma_count(cell).map(|(a, b)| example_line(&name, &[a.to_string(), b.to_string()]));
                    let two = match two {
                        Some(s) => format!(" If you meant two counts, put each in its own cell: {}.", q(&s)),
                        None => String::new(),
                    };
                    return Err(format!(
                        "Line {n} ({}): {} has a comma inside a number. A comma starts the next count, so a count cannot hold one: write a decimal point (12.5) or the two counts in their own cells.{two}",
                        q(l),
                        q(cell)
                    ));
                }
                Err(CountErr::Percent(v)) => {
                    return Err(format!(
                        "Line {n} ({}): {} is a percent, but each line needs a count (how many). Type the count; choose {} for Bar height to show percents. For example, {}.",
                        q(l),
                        q(cell),
                        q(PERCENT),
                        fix(digits(v.abs()))
                    ));
                }
                Err(CountErr::Negative(v)) => {
                    return Err(format!("Line {n} ({}): {} is negative, and a count (how many) is 0 or more. For example, {}.", q(l), q(cell), fix(digits(v.abs().min(MAX_COUNT as f64)))));
                }
                Err(CountErr::Fraction(v)) => {
                    return Err(format!(
                        "Line {n} ({}): {} is not a whole number, and a count is. Round it: {} (or {}).",
                        q(l),
                        q(cell),
                        fix(digits(v.floor())),
                        digits(v.ceil())
                    ));
                }
                Err(CountErr::TooBig(v)) => {
                    // the unit must really bring the count under the cap (LOW): 1e15 needs millions
                    let unit = [("thousands", 1e3), ("millions", 1e6), ("billions", 1e9)]
                        .into_iter()
                        .find(|(_, s)| v.is_finite() && v / s <= MAX_COUNT as f64);
                    let advice = match unit {
                        Some((name, _)) => format!("divide every line by the same number (count in {name}) and say the unit in the value axis title"),
                        None => format!("this count is far above the largest count this chart takes, so type counts of at most {MAX_COUNT} and say the unit in the value axis title"),
                    };
                    return Err(format!(
                        "Line {n} ({}): {} is more than {MAX_COUNT}, the largest count this chart takes. The bars are drawn on one scale, so every line must count in the same unit: {advice}. Changing one line alone would change what the other bars mean.",
                        q(l),
                        q(cell)
                    ));
                }
            }
        }
        if groups == 0 {
            groups = counts.len();
            first_row = Some((n, l.to_string(), counts.clone()));
            first_alt = alt.clone();
        } else if counts.len() != groups {
            // MAJOR 2: no rewrite. A rewrite would drop or invent a count (the old ", 0" padding
            // and the silent drop of a typed count); the teacher must choose the shape.
            let (fn_, fl, fc) = first_row.as_ref().expect("the first line sets the shape");
            let hint = |a: &Option<Vec<String>>, line: &str, c: &[u64]| -> String {
                match a {
                    Some(a) if a.len() == 3 => format!(" {} is one count, {}; two counts would be {}.", q(line), fmt_count(c[0]), q(&example_line(&a[0], &a[1..]))),
                    _ => String::new(),
                }
            };
            let alt_hint = format!("{}{}", hint(&alt, l, &counts), hint(&first_alt, fl, fc));
            return Err(format!(
                "Line {n} ({}) holds {} ({}), but line {fn_} ({}) holds {} ({}). Every line must hold the same number of counts: either add the missing count on the lines that hold fewer, or write one count on every line.{alt_hint} A comma with no space after it, between groups of three digits, is a thousands separator, so a thousands group is one count.",
                q(l),
                plural(counts.len(), "count", "counts"),
                counts.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(" and "),
                q(fl),
                plural(groups, "count", "counts"),
                fc.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(" and "),
            ));
        }
        // J1: the thousands reading was a choice; keep it so the editor can say which it used
        if counts.len() == 1 {
            if let Some(a) = &alt {
                let other: Option<Vec<u64>> = a[1..].iter().map(|c| parse_count(c).ok()).collect();
                if let Some(other) = other {
                    if (1..=2).contains(&other.len()) && other[0] != counts[0] {
                        ambiguous.push((n, l.to_string(), a.clone(), counts[0]));
                    }
                }
            }
        }
        if let Some((x, y)) = split {
            short_commas.push((n, l.to_string(), x, y));
        }
        let len = name.chars().count();
        if len > MAX_NAME {
            let short: String = name.chars().take(MAX_NAME).collect::<String>().trim_end().to_string();
            return Err(format!(
                "The name on line {n} has {len} characters; the most is {MAX_NAME}. Shorten it: change line {n} to {}.",
                q(&example_line(&short, &raw))
            ));
        }
        let key = name_key(&name);
        match index.get(&key) {
            Some(&i) => {
                let cat = &mut cats[i];
                match merge_index.get(&key) {
                    Some(&m) => {
                        merged[m].lines.push(n);
                        merged[m].parts.push(counts.clone());
                    }
                    None => {
                        merge_index.insert(key.clone(), merged.len());
                        merged.push(Merge { name: cat.name.clone(), lines: vec![cat.line, n], parts: vec![cat.counts.clone(), counts.clone()] });
                    }
                }
                for (a, b) in cat.counts.iter_mut().zip(&counts) {
                    *a += b;
                }
            }
            None => {
                index.insert(key, cats.len());
                cats.push(Cat { name, counts, line: n });
            }
        }
    }
    if cats.len() > MAX_CATS {
        let (other, n_small) = combine_smallest(&cats, groups);
        return Err(format!(
            "There are {} categories; this chart takes at most {MAX_CATS}. Combine the {n_small} smallest into one line, like {}.",
            cats.len(),
            q(&other)
        ));
    }
    for g in 0..groups {
        if cats.iter().all(|c| c.counts[g] == 0) {
            let c0 = &cats[0];
            let mut c: Vec<String> = c0.counts.iter().map(|v| v.to_string()).collect();
            c[g] = "1".into();
            let which = if groups == 1 { "Every count is 0".to_string() } else { format!("Every count in the {} group is 0", ["first", "second"][g]) };
            return Err(format!("{which}, so there are no bars to draw. For example, change line {} to {}.", c0.line, q(&example_line(&c0.name, &c))));
        }
    }
    Ok(Table { cats, groups, head, merged, ambiguous, short_comma: short_commas })
}

/// The rule the too-many message gives: the smallest categories (by their total over the groups,
/// the later-typed first among equals) that leave `MAX_CATS - 1` standing, combined into a line
/// named Other. Returns that line and how many were combined.
pub fn combine_smallest(cats: &[Cat], groups: usize) -> (String, usize) {
    let mut idx: Vec<usize> = (0..cats.len()).collect();
    let tot = |i: usize| cats[i].counts.iter().sum::<u64>();
    idx.sort_by(|&a, &b| tot(a).cmp(&tot(b)).then(cats[b].line.cmp(&cats[a].line)));
    let n_small = cats.len() + 1 - MAX_CATS;
    let mut sums = vec![0u64; groups];
    for &i in &idx[..n_small] {
        for (s, c) in sums.iter_mut().zip(&cats[i].counts) {
            *s += c;
        }
    }
    (example_line("Other", &sums.iter().map(|v| v.to_string()).collect::<Vec<_>>()), n_small)
}

// ───────────────────────── numbers and text ─────────────────────────

/// A count with thousands commas.
pub fn fmt_count(v: u64) -> String {
    let s = v.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// `c` out of `total` as a percent rounded half up to `d` decimals, scaled by 10^d (exact).
pub fn pct_scaled(c: u64, total: u64, d: u32) -> u128 {
    let total = total.max(1) as u128;
    (2 * c as u128 * 100 * 10u128.pow(d) + total) / (2 * total)
}

/// A group's percents in one column, each rounded on its own (half up), scaled by 10^d, in row
/// order. One rule for every printed percent: a table cell, the value on its bar and the answer
/// key all show this number (MED 4), so the column's Total row prints what its cells add to.
pub fn pct_column(counts: &[u64], total: u64, d: u32) -> Vec<u128> {
    counts.iter().map(|c| pct_scaled(*c, total, d)).collect()
}

pub fn pct_text(c: u64, total: u64, d: u32) -> String {
    let s = pct_scaled(c, total, d);
    let p = 10u128.pow(d);
    if d == 0 { format!("{s}%") } else { format!("{}.{:0w$}%", s / p, s % p, w = d as usize) }
}

/// Noto Sans advances of printable ASCII (U+0020 to U+007E), in thousandths of an em.
const ADV: [u16; 95] = [
    260, 269, 408, 646, 572, 831, 732, 225, 300, 300, 551, 572, 268, 322, 268, 372, 572, 572, 572, 572, 572, 572, 572, 572, 572, 572, 268, 268, 572, 572, 572, 434, 899, 639,
    650, 632, 730, 556, 519, 728, 741, 339, 273, 619, 524, 907, 760, 781, 605, 781, 622, 549, 556, 731, 600, 930, 586, 566, 572, 329, 372, 329, 572, 444, 281, 561, 615, 480,
    615, 564, 344, 615, 618, 258, 258, 534, 258, 935, 618, 605, 615, 615, 413, 479, 361, 618, 508, 786, 529, 510, 470, 380, 551, 380, 572,
];

/// Width of `s` at `size` px, on the generous side outside ASCII (a fallback face may be wider).
pub fn tw(s: &str, size: f64) -> f64 {
    let em: f64 = s
        .chars()
        .map(|c| match c as u32 {
            0x20..=0x7e => ADV[c as usize - 0x20] as f64 / 1000.0,
            0x2026 => 1.0,
            0x0300..=0x036f => 0.0,
            0x00a0..=0x024f => 0.9,
            u if u >= 0x1f000 => 1.3,
            _ => 1.05,
        })
        .sum();
    em * size * 1.02 + 1.0
}

/// Greedy word wrap to `width`; a word wider than the line is split between letters when
/// `break_words`, otherwise the wrap fails.
pub fn wrap(s: &str, width: f64, size: f64, break_words: bool) -> Option<Vec<String>> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in s.split_whitespace() {
        let cand = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
        if tw(&cand, size) <= width {
            cur = cand;
            continue;
        }
        if !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
        }
        if tw(word, size) <= width {
            cur = word.to_string();
            continue;
        }
        if !break_words {
            return None;
        }
        for ch in word.chars() {
            let cand = format!("{cur}{ch}");
            if tw(&cand, size) > width && !cur.is_empty() {
                lines.push(std::mem::take(&mut cur));
                cur.push(ch);
            } else {
                cur = cand;
            }
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    Some(lines)
}

fn clip_chars(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() } else { format!("{}\u{2026}", s.chars().take(n).collect::<String>().trim_end()) }
}

// ───────────────────────── options ─────────────────────────

struct Opts {
    horizontal: bool,
    percent: bool,
    order: usize,
    gap: f64,
    values: bool,
    decimals: u32,
    grid: bool,
    title: String,
    cat_title: String,
    val_title: String,
    blank: usize,
    questions: bool,
    print: bool,
    fills: Fills,
    k: f64,
    names: [String; 2],
    /// Pareto asked for and drawable (one group, vertical bars).
    pareto: bool,
}

fn opts(p: &Params, t: &Table) -> Opts {
    let head_cells = t.head.as_ref().map(|h| h.2.clone()).unwrap_or_default();
    let names = [0, 1].map(|g| {
        let typed = p.s(["name1", "name2"][g]).trim();
        let from_head = head_cells.get(g + 1).map(|s| s.trim()).unwrap_or("");
        let n = if !typed.is_empty() { typed } else if !from_head.is_empty() { from_head } else { ["Group 1", "Group 2"][g] };
        clip_chars(n, MAX_NAME)
    });
    let order = ORDERS.iter().position(|o| *o == p.s("order")).unwrap_or(0);
    let horizontal = p.s("orient") == "Horizontal";
    let cat_title = match p.s("cat_title").trim() {
        "" => head_cells.first().map(|s| s.trim().to_string()).unwrap_or_default(),
        s => s.to_string(),
    };
    Opts {
        horizontal,
        percent: p.s("measure") == PERCENT,
        order,
        gap: match p.s("gap") {
            "Narrow" => 0.25,
            "Wide" => 0.6,
            _ => 0.4,
        },
        values: p.b("values"),
        decimals: p.i("decimals").clamp(0, 2) as u32,
        grid: p.b("grid"),
        title: clip_chars(p.s("title").trim(), MAX_TITLE),
        cat_title: clip_chars(&cat_title, MAX_AXIS_TITLE),
        val_title: clip_chars(p.s("val_title").trim(), MAX_AXIS_TITLE),
        blank: BLANKS.iter().position(|b| *b == p.s("blank")).unwrap_or(0),
        questions: p.b("questions"),
        print: p.s("style") == "Print",
        fills: Fills::new(p),
        k: svg::label_factor(p.s("label_size")),
        names,
        pareto: order == 4 && t.groups == 1 && !horizontal,
    }
}

/// Bar heights: counts, or percents of each group's own total.
fn heights(t: &Table, percent: bool) -> Vec<Vec<f64>> {
    let totals: Vec<u64> = (0..t.groups).map(|g| t.total(g)).collect();
    t.cats.iter().map(|c| (0..t.groups).map(|g| if percent { c.counts[g] as f64 * 100.0 / totals[g].max(1) as f64 } else { c.counts[g] as f64 }).collect()).collect()
}

/// The drawing order of the categories (indices into `t.cats`). Equal bars keep their typed
/// order: the comparison is an exact integer, and a tie falls back to the typed index, so no
/// float sum and no sorting algorithm can swap two equal categories.
pub fn order(t: &Table, percent: bool, how: &str) -> Vec<usize> {
    let totals: Vec<u64> = (0..t.groups).map(|g| t.total(g)).collect();
    // the sum of a category's two bars, in integers: relative frequencies are c*100/total, so
    // c0*T1 + c1*T0 orders them exactly the way their sum does (LOW 1: no float tie)
    let size = |i: usize| -> u128 {
        let c = &t.cats[i].counts;
        if !percent || t.groups == 1 {
            c.iter().map(|v| *v as u128).sum()
        } else {
            c[0] as u128 * totals[1] as u128 + c[1] as u128 * totals[0] as u128
        }
    };
    let mut idx: Vec<usize> = (0..t.cats.len()).collect();
    match how {
        "Tallest first" | PARETO => idx.sort_by(|&a, &b| size(b).cmp(&size(a)).then(a.cmp(&b))),
        "Shortest first" => idx.sort_by(|&a, &b| size(a).cmp(&size(b)).then(a.cmp(&b))),
        "A to Z" => idx.sort_by(|&a, &b| natural_cmp(&t.cats[a].name, &t.cats[b].name).then(a.cmp(&b))),
        _ => {}
    }
    idx
}

/// Natural, case-insensitive order: `apple` before `Zebra`, `Item 2` before `Item 10`.
fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    // pieces of digits and of everything else, lowercased
    let pieces = |s: &str| -> Vec<(bool, String)> {
        let mut out: Vec<(bool, String)> = Vec::new();
        for ch in s.to_lowercase().chars() {
            let d = ch.is_ascii_digit();
            match out.last_mut() {
                Some((was, text)) if *was == d => text.push(ch),
                _ => out.push((d, ch.to_string())),
            }
        }
        out
    };
    let (pa, pb) = (pieces(a), pieces(b));
    for (x, y) in pa.iter().zip(&pb) {
        let o = if x.0 && y.0 {
            // a number: by value, so 2 comes before 10, and 02 after 2
            let (nx, ny) = (x.1.trim_start_matches('0'), y.1.trim_start_matches('0'));
            nx.len().cmp(&ny.len()).then(nx.cmp(ny)).then(x.1.len().cmp(&y.1.len()))
        } else {
            x.1.cmp(&y.1)
        };
        if o != Ordering::Equal {
            return o;
        }
    }
    pa.len().cmp(&pb.len()).then(a.cmp(b))
}

/// The value axis: from `lo` to `hi` with ticks every `step`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scale {
    pub lo: f64,
    pub hi: f64,
    pub step: f64,
}

fn auto_scale(top: f64, counts: bool) -> Scale {
    let mut step = chart::nice_step(top.max(1e-9), 5.0);
    if counts {
        step = step.max(1.0);
    }
    Scale { lo: 0.0, hi: ((top / step) - 1e-9).ceil().max(1.0) * step, step }
}

/// The manual scale when it is usable (finite, top above start).
fn manual_scale(p: &Params, counts: bool) -> Option<Scale> {
    let (lo, hi) = (p.f("scale_min"), p.f("scale_max"));
    if !(lo.is_finite() && hi.is_finite() && lo >= 0.0 && hi > lo) {
        return None;
    }
    let mut step = chart::nice_step(hi - lo, 5.0);
    if counts && hi - lo >= 5.0 {
        step = step.max(1.0);
    }
    Some(Scale { lo, hi, step })
}

fn scale_for(p: &Params, t: &Table, o: &Opts) -> Scale {
    let h = heights(t, o.percent);
    let top = h.iter().flatten().cloned().fold(0.0, f64::max);
    if o.pareto {
        let total = if o.percent { 100.0 } else { t.total(0) as f64 };
        let mut step = chart::nice_step(total, 5.0);
        if !o.percent {
            step = step.max(1.0);
        }
        return Scale { lo: 0.0, hi: total, step };
    }
    if !p.b("scale_auto") {
        if let Some(s) = manual_scale(p, !o.percent) {
            return s;
        }
    }
    auto_scale(top, !o.percent)
}

fn tick_text(v: f64, percent: bool) -> String {
    if percent {
        format!("{}%", chart::fmt(v))
    } else if (v - v.round()).abs() < 1e-9 && v >= 0.0 && v < 1e15 {
        fmt_count(v.round() as u64)
    } else {
        chart::fmt(v)
    }
}

/// The value written on each bar, [cat][group].
fn value_labels(t: &Table, o: &Opts) -> Vec<Vec<String>> {
    let totals: Vec<u64> = (0..t.groups).map(|g| t.total(g)).collect();
    t.cats.iter().map(|c| (0..t.groups).map(|g| if o.percent { pct_text(c.counts[g], totals[g], o.decimals) } else { fmt_count(c.counts[g]) }).collect()).collect()
}

// ───────────────────────── notes ─────────────────────────

/// Editor notes: a refusal (the same text the figure shows), or what is drawn differently from
/// what was asked, each with a fix that works when applied.
pub fn warnings(p: &Params) -> Vec<String> {
    let t = match parse(p.s("data")) {
        Ok(t) => t,
        Err(e) => return vec![e],
    };
    let o = opts(p, &t);
    let mut out = Vec::new();
    if let Some((n, l, cells)) = &t.head {
        let used = if cells.len() >= 2 { " Its first heading names the category axis unless you type one, and the others name the groups." } else { "" };
        out.push(format!("Line {n} ({}) was read as headings, not as a category.{used}", q(l)));
    }
    let name_on = |n: usize| t.cats.iter().find(|c| c.line == n).map(|c| c.name.clone()).unwrap_or_default();
    // MAJOR 1: a thousands group is one count, but `A, 3,100` could also be 3 and 100: say which
    // reading was used, how to write the other, and give a rewrite that keeps this one
    if !t.ambiguous.is_empty() {
        let (n, l, alt, value) = &t.ambiguous[0];
        let values: Vec<String> = t.ambiguous.iter().map(|(_, _, _, v)| fmt_count(*v)).collect();
        let other = &alt[1..];
        let other_note = if other.len() == 2 {
            format!("Two counts would be {}, written with a space after the comma.", q(&example_line(&alt[0], other)))
        } else {
            format!("It could also be the category {} with count {}, written {}.", q(&alt[0]), other[0], q(&example_line(&alt[0], &other[..1])))
        };
        // the reading comes from a comma inside the count, or from spaces between groups of three
        let rule = if alt.len() == 2 {
            "a space between groups of three digits is a thousands group"
        } else {
            "a comma with no space after it, between groups of three digits, is a thousands separator"
        };
        let who = if t.ambiguous.len() == 1 {
            format!("Line {n} ({}) is one count, {}: {rule}", q(l), fmt_count(*value))
        } else {
            let lines: Vec<String> = t.ambiguous.iter().map(|(n, _, _, _)| n.to_string()).collect();
            format!("Lines {} give a count in thousands, so each is one count ({}): {rule}", and_list(&lines), and_list(&values))
        };
        out.push(format!("{who}. {other_note} To keep {}, change line {n} to {}.", fmt_count(*value), q(&example_line(&name_on(*n), &[value.to_string()]))));
    }
    // LOW: a comma inside a number that is not a thousands group was read as two counts
    if let Some((n, l, a, b)) = t.short_comma.first() {
        out.push(format!(
            "Line {n} ({}) has a comma with no space after it and not three digits after it, so {} is not a thousands group: it was read as two counts, {} and {}. To keep them, change line {n} to {}.",
            q(l),
            q(&format!("{a},{b}")),
            a,
            b,
            q(&example_line(&name_on(*n), &[a.to_string(), b.to_string()]))
        ));
    }
    // MED 1: the first line read as data whose counts are all years is probably a headings line
    if t.head.is_none() && t.groups == 2 && t.cats.len() > 1 {
        let c0 = &t.cats[0];
        let is_year = |v: u64| (1900..=2100).contains(&v);
        let later_plain = t.cats[1..].iter().any(|c| c.counts.iter().any(|v| !is_year(*v)));
        if c0.counts.iter().all(|v| is_year(*v)) && later_plain {
            out.push(format!(
                "Line {} is the category {} with counts {} and {}, but those are years, so it may be a line of headings. If it is, delete line {} and type the group names in {} and {}.",
                c0.line,
                q(&c0.name),
                fmt_count(c0.counts[0]),
                fmt_count(c0.counts[1]),
                c0.line,
                q("Group 1 name"),
                q("Group 2 name")
            ));
        }
    }
    for m in &t.merged {
        let sums: Vec<String> = (0..t.groups)
            .map(|g| {
                let mut parts: Vec<String> = m.parts.iter().take(6).map(|c| c[g].to_string()).collect();
                if m.parts.len() > 6 {
                    parts.push("\u{2026}".into());
                }
                format!("{} = {}", parts.join(" + "), m.parts.iter().map(|c| c[g]).sum::<u64>())
            })
            .collect();
        let taken: Vec<String> = t.cats.iter().map(|c| name_key(&c.name)).collect();
        let mut k = 2;
        let fresh = loop {
            let cand = format!("{} {k}", m.name);
            if !taken.contains(&name_key(&cand)) && cand.chars().count() <= MAX_NAME {
                break cand;
            }
            if cand.chars().count() > MAX_NAME {
                break format!("{} {k}", m.name.chars().take(MAX_NAME - 3).collect::<String>().trim_end());
            }
            k += 1;
        };
        let second = m.lines[1];
        let counts: Vec<String> = m.parts[1].iter().map(|v| v.to_string()).collect();
        out.push(format!(
            "{} is on lines {}, so their counts were added ({}). To keep them apart, change line {second} to {}.",
            q(&m.name),
            if m.lines.len() <= 6 {
                and_list(&m.lines.iter().map(|l| l.to_string()).collect::<Vec<_>>())
            } else {
                format!("{}, {} and {} more", m.lines[0], m.lines[1], m.lines.len() - 2)
            },
            sums.join("; "),
            q(&example_line(&fresh, &counts))
        ));
    }
    for (key, label, max) in [("title", "Title", MAX_TITLE), ("cat_title", "Category axis title", MAX_AXIS_TITLE), ("val_title", "Value axis title (blank = the bar height)", MAX_AXIS_TITLE)] {
        let s = p.s(key).trim();
        if s.chars().count() > max {
            out.push(format!("{} has {} characters and is cut at {max}. Set {} to {}.", q(label), s.chars().count(), q(label), q(&s.chars().take(max).collect::<String>())));
        }
    }
    if t.groups == 2 {
        for (key, label) in [("name1", "First group's name"), ("name2", "Second group's name")] {
            let s = p.s(key).trim();
            if s.chars().count() > MAX_NAME {
                out.push(format!("{} has {} characters and is cut at {MAX_NAME}. Set {} to {}.", q(label), s.chars().count(), q(label), q(&s.chars().take(MAX_NAME).collect::<String>())));
            }
        }
    }
    if o.order == 4 && !o.pareto {
        if t.groups == 2 {
            out.push(format!(
                "A Pareto chart shows one group, so the bars are sorted tallest first with no cumulative line. Choose {} for {}.",
                q("Tallest first"),
                q("Order")
            ));
        } else {
            out.push(format!("The cumulative line is drawn on vertical bars only, so it is left out. Choose {} for {}.", q("Vertical"), q("Bars")));
        }
    }
    let h = heights(&t, o.percent);
    let top = h.iter().flatten().cloned().fold(0.0, f64::max);
    let unit = |v: f64| if o.percent { format!("{}%", chart::fmt((v * 100.0).round() / 100.0)) } else { chart::fmt(v) };
    if o.pareto && !p.b("scale_auto") {
        out.push(format!(
            "A Pareto chart sets its own scale (its top is the total, 100%) so the bars and the cumulative line agree; the scale settings are not used. Turn on {}.",
            q("Choose the scale automatically")
        ));
    } else if !p.b("scale_auto") {
        match manual_scale(p, !o.percent) {
            None => {
                let need = top.max(p.f("scale_min").max(0.0) + 1.0);
                let need = if o.percent { (need * 100.0).ceil() / 100.0 } else { need.ceil() };
                out.push(format!(
                    "{} ({}) is not above {} ({}), so the scale is chosen automatically. Set {} to at least {}.",
                    q("Scale top"),
                    chart::fmt(p.f("scale_max")),
                    q("Scale start"),
                    chart::fmt(p.f("scale_min")),
                    q("Scale top"),
                    chart::fmt(need)
                ));
            }
            Some(s) => {
                if s.lo > 0.0 {
                    let below: Vec<String> = t.cats.iter().zip(&h).filter(|(_, v)| v.iter().any(|x| *x <= s.lo)).map(|(c, _)| q(&c.name)).collect();
                    let hidden = if below.is_empty() { String::new() } else { format!(" The bars for {} do not reach the start and do not show.", and_list(&below)) };
                    out.push(format!(
                        "The value axis starts at {}, not 0, so the bars' lengths are not in proportion to the values: a bar twice as long does not show twice as many.{hidden} Set {} to 0.",
                        unit(s.lo),
                        q("Scale start")
                    ));
                }
                let over: Vec<String> = t.cats.iter().zip(&h).filter(|(_, v)| v.iter().any(|x| *x > s.hi + 1e-9)).map(|(c, _)| q(&c.name)).collect();
                if !over.is_empty() {
                    let need = if o.percent { (top * 100.0).ceil() / 100.0 } else { top };
                    out.push(format!(
                        "The bars for {} go past the scale top ({}) and are cut off. Set {} to at least {}.",
                        and_list(&over),
                        unit(s.hi),
                        q("Scale top"),
                        chart::fmt(need)
                    ));
                }
            }
        }
    }
    // printed percents that do not add to 100
    // MED 3: the percents written on the bars are rounded one by one, so they can add to 99% or
    // 101%; offer a Decimals setting that adds exactly, searched in both directions
    // MED 2/3/4: each percent is rounded on its own, so a column can add to 99% or 101% (the
    // table's Total row shows that sum). Offer one Decimals setting that leaves every group's
    // column adding to 100; when none does, say so and offer the counts instead -- one note for
    // the table, so two groups can never undo each other
    let printed = o.percent && (o.values || o.blank == 1);
    if printed {
        let column_sum = |g: usize, d: u32| -> u128 { let total = t.total(g); t.cats.iter().map(|c| pct_scaled(c.counts[g], total, d)).sum() };
        let exact = |d: u32| 100 * 10u128.pow(d);
        let bad = |d: u32| (0..t.groups).any(|g| column_sum(g, d) != exact(d));
        if bad(o.decimals) {
            let shown: Vec<String> = (0..t.groups)
                .map(|g| if t.groups == 1 { pct_from_scaled(column_sum(g, o.decimals), o.decimals) } else { format!("{} for {}", pct_from_scaled(column_sum(g, o.decimals), o.decimals), q(&o.names[g])) })
                .collect();
            let fix = (0..=2u32).filter(|d| !bad(*d)).min_by_key(|d| (*d as i32 - o.decimals as i32).abs());
            let fix = match fix {
                Some(d) => format!(" Set {} to {d}.", q("Decimals in percents")),
                None => format!(
                    " No Decimals setting makes {} add to 100%. Choose {} for {} if they must add up exactly.",
                    if t.groups == 1 { "them" } else { "every group" },
                    q(COUNT),
                    q("Bar height")
                ),
            };
            let tail = if t.groups == 1 { "not 100%" } else { "not 100% in every group" };
            out.push(format!("The printed percents add to {}, {tail}, because each is rounded.{fix}", and_list(&shown)));
        }
    }
    if !o.values && o.blank == 0 {
        out.push(format!(
            "The student copy leaves out the values on the bars, but {} is off, so there are none to leave out. Turn on {}.",
            q("Write the value on each bar"),
            q("Write the value on each bar")
        ));
    }
    if !o.values && o.blank == 2 {
        out.push(format!(
            "The student copy leaves out the scale numbers, and with {} off nothing on it gives the scale. Turn on {}.",
            q("Write the value on each bar"),
            q("Write the value on each bar")
        ));
    }
    out
}

fn pct_from_scaled(s: u128, d: u32) -> String {
    let p = 10u128.pow(d);
    if d == 0 { format!("{s}%") } else { format!("{}.{:0w$}%", s / p, s % p, w = d as usize) }
}

// ───────────────────────── drawing ─────────────────────────

/// A refusal or empty-data message, wrapped to the canvas.
pub fn message(msg: &str) -> String {
    let lines = wrap(msg, 580.0, 15.0, true).unwrap_or_default();
    let h = 40.0 + lines.len() as f64 * 15.0 * PITCH;
    let mut d = Doc::new(640.0, h.max(100.0));
    for (i, l) in lines.iter().enumerate() {
        put(&mut d, 30.0, 30.0 + 15.0 * ASC + i as f64 * 15.0 * PITCH, 15.0, "start", l, "#000000");
    }
    d.finish()
}

fn put(d: &mut Doc, x: f64, y: f64, size: f64, anchor: &str, s: &str, ink: &str) {
    d.raw(&format!(r#"<text x="{x:.2}" y="{y:.2}" font-size="{size:.2}" text-anchor="{anchor}" font-family="{FONT}" fill="{ink}">{}</text>"#, esc(s)));
}

/// Text turned a quarter turn counter-clockwise about (x, y): it reads upwards.
fn put_up(d: &mut Doc, x: f64, y: f64, size: f64, anchor: &str, s: &str, ink: &str) {
    d.raw(&format!(
        r#"<text transform="translate({x:.2} {y:.2}) rotate(-90)" font-size="{size:.2}" text-anchor="{anchor}" font-family="{FONT}" fill="{ink}">{}</text>"#,
        esc(s)
    ));
}

fn render(p: &Params) -> String {
    let t = match parse(p.s("data")) {
        Ok(t) => t,
        Err(msg) => return message(&msg),
    };
    let o = opts(p, &t);
    let scale = scale_for(p, &t, &o);
    draw(&t, &o, scale, p.b(SHOW_ANSWERS))
}

/// One table column: wrapped heading lines, a cell per row (drawn order, then the total row),
/// and whether its cells are answers.
struct Col {
    head: Vec<String>,
    cells: Vec<String>,
    answer: bool,
    w: f64,
}

fn draw(t: &Table, o: &Opts, s: Scale, key: bool) -> String {
    let fills = o.fills;
    let ink = if o.print { "#000000" } else { INK };
    let grid_col = if o.print { PRINT_GRID } else { GRID };
    // Print: the first series is a solid grey that survives a copy; the others keep the hatch
    let series = |g: usize| if o.print && g == 0 { PRINT_BAR.to_string() } else { fills.series(g) };
    let grid_w = if o.print { PRINT_GRID_W } else { 1.0 };
    let k = o.k;
    let (fs, fv, ft, fa) = (14.0 * k, 12.6 * k, 18.2 * k, 15.4 * k);
    let g_n = t.groups;
    let idx = order(t, o.percent, ORDERS[o.order]);
    let n = idx.len();
    let h = heights(t, o.percent);
    let labels = value_labels(t, o);
    let totals: Vec<u64> = (0..g_n).map(|g| t.total(g)).collect();
    let mut ticks: Vec<(f64, String)> = chart::ticks(s.lo, s.hi, s.step).into_iter().map(|(v, _)| (v, tick_text(v, o.percent))).collect();
    let tick_w = ticks.iter().map(|t| tw(&t.1, fs)).fold(0.0, f64::max);
    let val_title = if o.val_title.is_empty() { if o.percent { "Relative frequency" } else { "Frequency" }.to_string() } else { o.val_title.clone() };
    let vl_w = if o.values { labels.iter().flatten().map(|l| tw(l, fv)).fold(0.0, f64::max) } else { 0.0 };
    let show_vals = o.values && (key || o.blank == 2);
    let show_bars = key || o.blank != 1;
    let show_ticks = key || o.blank != 2;

    // the table (students draw the bars from it) and the questions: laid out for both copies
    let mut cols: Vec<Col> = Vec::new();
    if o.blank == 1 {
        let name_w = idx.iter().map(|&i| tw(&t.cats[i].name, fs)).fold(tw("Category", fs), f64::max).min(280.0 * k);
        let mut names: Vec<String> = idx.iter().map(|&i| t.cats[i].name.clone()).collect();
        names.push("Total".into());
        cols.push(Col { head: vec!["Category".into()], cells: names, answer: false, w: name_w });
        let head_of = |g: usize, pct: bool| -> String {
            match (g_n, pct) {
                (1, false) => "Frequency".into(),
                (1, true) => "Relative frequency".into(),
                (_, false) => o.names[g].clone(),
                (_, true) => format!("{} (%)", o.names[g]),
            }
        };
        for pct in [false, true] {
            if pct && !o.percent {
                continue;
            }
            for g in 0..g_n {
                // the Total row prints what the cells add to: on a percent column that is the
                // rounded sum (100% when they add up), never a 100% the cells contradict (MED 4)
                let (mut cells, total_cell): (Vec<String>, String) = if pct {
                    let col = pct_column(&idx.iter().map(|&i| t.cats[i].counts[g]).collect::<Vec<_>>(), totals[g], o.decimals);
                    (col.iter().map(|s| pct_from_scaled(*s, o.decimals)).collect(), pct_from_scaled(col.iter().sum::<u128>(), o.decimals))
                } else {
                    (idx.iter().map(|&i| fmt_count(t.cats[i].counts[g])).collect(), fmt_count(totals[g]))
                };
                cells.push(total_cell);
                let head = wrap(&head_of(g, pct), 150.0 * k, fs, true).unwrap_or_default();
                let w = head.iter().chain(cells.iter()).map(|c| tw(c, fs)).fold(40.0 * k, f64::max);
                cols.push(Col { head, cells, answer: pct, w });
            }
        }
    }
    let table_w: f64 = cols.iter().map(|c| c.w + 16.0).sum();
    let mut qa: Vec<(String, String)> = Vec::new();
    if o.questions {
        let tops: Vec<String> = (0..g_n)
            .map(|g| {
                let m = t.cats.iter().map(|c| c.counts[g]).max().unwrap_or(0);
                let names: Vec<String> = idx.iter().filter(|&&i| t.cats[i].counts[g] == m).map(|&i| t.cats[i].name.clone()).collect();
                if names.len() > 1 { format!("{} (tied)", and_list(&names)) } else { names[0].clone() }
            })
            .collect();
        let per = |v: Vec<String>| if g_n == 1 { v[0].clone() } else { format!("{}: {}; {}: {}", o.names[0], v[0], o.names[1], v[1]) };
        let first = &t.cats[idx[0]];
        qa.push((
            if g_n == 1 { "1. Which category has the most?".into() } else { "1. In each group, which category has the most?".into() },
            per(tops),
        ));
        qa.push((
            if g_n == 1 { "2. How many observations are there in all?".into() } else { "2. How many observations are in each group?".into() },
            per(totals.iter().map(|v| fmt_count(*v)).collect()),
        ));
        qa.push((
            if g_n == 1 { format!("3. What percent of the observations are in {}?", q(&first.name)) } else { format!("3. In each group, what percent are in {}?", q(&first.name)) },
            per((0..g_n).map(|g| pct_text(first.counts[g], totals[g], o.decimals)).collect()),
        ));
    }

    // widths
    let legend: Vec<(usize, f64)> = if g_n == 2 { (0..2).map(|g| (g, 18.0 * k + 6.0 + tw(&o.names[g], fs))).collect() } else { Vec::new() };
    let legend_row_w: f64 = legend.iter().map(|l| l.1).sum::<f64>() + 24.0;
    let slot_min = (g_n as f64 * fv * 1.45 / (1.0 - o.gap)).max(fs * 1.45);
    let plot_h_v = 260.0 + (k - 1.0) * 60.0;
    let mut w: f64 = 640.0;
    let (l, r);
    let mut vt_lines: Vec<String> = Vec::new();
    let mut ct_lines: Vec<String> = Vec::new();
    let mut name_lines: Vec<Vec<String>> = Vec::new();
    let plot_h;
    if !o.horizontal {
        vt_lines = wrap(&val_title, plot_h_v - 8.0, fa, true).unwrap_or_default();
        l = 14.0 + vt_lines.len() as f64 * fa * PITCH + 6.0 + tick_w + 12.0;
        // the right axis is the cumulative percent, so a percent bar chart already has it on the left
        r = if o.pareto && !o.percent { 12.0 + tw("100%", fs) + 10.0 + fa * PITCH + 12.0 } else { 24.0 };
        w = w.max(l + (n as f64 * slot_min).max(420.0) + r);
        plot_h = plot_h_v;
    } else {
        let name_w = idx.iter().map(|&i| tw(&t.cats[i].name, fs)).fold(0.0, f64::max).min(200.0 * k);
        name_lines = idx.iter().map(|&i| wrap(&t.cats[i].name, name_w, fs, true).unwrap_or_default()).collect();
        let max_lines = name_lines.iter().map(|v| v.len()).max().unwrap_or(1).max(1) as f64;
        let slot_h = (max_lines * fs * PITCH + 8.0).max(g_n as f64 * fv * 1.45 / (1.0 - o.gap));
        plot_h = (n as f64 * slot_h).max(150.0);
        ct_lines = if o.cat_title.is_empty() { Vec::new() } else { wrap(&o.cat_title, plot_h - 8.0, fa, true).unwrap_or_default() };
        l = 14.0 + ct_lines.len() as f64 * fa * PITCH + if ct_lines.is_empty() { 0.0 } else { 6.0 } + name_w + 12.0;
        r = (vl_w + 18.0).max(tick_w / 2.0 + 10.0).max(24.0);
        w = w.max(l + 260.0 + r);
    }
    w = w.max(table_w + 32.0);
    let plot_w = w - l - r;
    // horizontal: the scale numbers sit side by side under the axis; a wider step keeps them apart
    if o.horizontal {
        let mut step = s.step;
        for _ in 0..8 {
            let gap = step / (s.hi - s.lo).max(1e-12) * plot_w;
            let widest = ticks.iter().map(|t| tw(&t.1, fs)).fold(0.0, f64::max);
            if ticks.len() <= 1 || gap >= widest + 14.0 {
                break;
            }
            step = chart::nice_step(step * 1.5, 1.0);
            ticks = chart::ticks(s.lo, s.hi, step).into_iter().map(|(v, _)| (v, tick_text(v, o.percent))).collect();
        }
    }
    let slot = if o.horizontal { plot_h / n as f64 } else { plot_w / n as f64 };
    // a few categories on a wide chart: the bars keep a classroom width, centred in their slots
    let cluster = (slot * (1.0 - o.gap)).min(170.0 * k);
    let bar_t = cluster / g_n as f64;

    // vertical: category names wrapped in their slot, or turned when a word does not fit
    let mut names_up = false;
    if !o.horizontal {
        let tried: Vec<Option<Vec<String>>> = idx.iter().map(|&i| wrap(&t.cats[i].name, slot - 6.0, fs, false).filter(|v| v.len() <= 3)).collect();
        if tried.iter().all(|t| t.is_some()) {
            name_lines = tried.into_iter().map(|t| t.unwrap()).collect();
        } else {
            names_up = true;
            name_lines = idx.iter().map(|&i| vec![t.cats[i].name.clone()]).collect();
        }
        ct_lines = if o.cat_title.is_empty() { Vec::new() } else { wrap(&o.cat_title, plot_w, fa, true).unwrap_or_default() };
    } else {
        vt_lines = wrap(&val_title, plot_w, fa, true).unwrap_or_default();
    }
    // value labels across or turned
    let vals_up = !o.horizontal && o.values && !(if g_n == 1 { vl_w <= slot - 4.0 } else { vl_w <= bar_t - 2.0 });
    let label_ext = if vals_up { vl_w } else { fv * (ASC + DES) };

    // vertical stack
    let title_lines = if o.title.is_empty() { Vec::new() } else { wrap(&o.title, w - 32.0, ft, true).unwrap_or_default() };
    let mut y = 12.0;
    let title_y = y;
    y += title_lines.len() as f64 * ft * PITCH + if title_lines.is_empty() { 0.0 } else { 6.0 };
    let legend_stacked = legend_row_w > w - 32.0;
    let legend_y = y;
    let legend_row_h = fs * PITCH + 4.0;
    if g_n == 2 {
        y += legend_row_h * if legend_stacked { 2.0 } else { 1.0 } + 6.0;
    }
    let headroom = if o.horizontal { 10.0 } else if o.values { label_ext + 6.0 + if o.pareto { 9.0 } else { 0.0 } + 4.0 } else { 12.0 };
    let top = y + headroom;
    let bottom = top + plot_h;
    let mut yb = bottom;
    let names_h = if o.horizontal {
        8.0 + fs * (ASC + DES) + 8.0
    } else if names_up {
        8.0 + name_lines.iter().map(|v| tw(&v[0], fs)).fold(0.0, f64::max) + 8.0
    } else {
        8.0 + name_lines.iter().map(|v| v.len()).max().unwrap_or(1) as f64 * fs * PITCH + 4.0
    };
    yb += names_h;
    let below_lines = if o.horizontal { &vt_lines } else { &ct_lines };
    let below_title_y = yb;
    yb += below_lines.len() as f64 * fa * PITCH + if below_lines.is_empty() { 0.0 } else { 6.0 };
    // table rows
    let row_lines: Vec<usize> = if cols.is_empty() {
        Vec::new()
    } else {
        (0..=n).map(|r| if r < n { wrap(&cols[0].cells[r], cols[0].w, fs, true).map(|v| v.len()).unwrap_or(1).max(1) } else { 1 }).collect()
    };
    let head_lines = cols.iter().map(|c| c.head.len()).max().unwrap_or(0);
    let table_y = yb + 10.0;
    if !cols.is_empty() {
        yb = table_y + head_lines as f64 * fs * PITCH + 8.0 + row_lines.iter().map(|l| *l as f64 * fs * PITCH + 6.0).sum::<f64>() + 6.0;
    }
    let q_w = w - 40.0;
    let q_layout: Vec<(Vec<String>, Vec<String>)> = qa
        .iter()
        .map(|(qq, a)| (wrap(qq, q_w, fs, true).unwrap_or_default(), wrap(&format!("Answer: {a}"), q_w - 16.0, fs, true).unwrap_or_default()))
        .collect();
    let q_y = yb + 12.0;
    if !q_layout.is_empty() {
        yb = q_y + q_layout.iter().map(|(a, b)| (a.len() + b.len()) as f64 * fs * PITCH + 8.0).sum::<f64>();
    }
    let hgt = yb + 14.0;

    let mut d = Doc::new(w.ceil(), hgt.ceil());
    d.defs(&fills.defs());
    for (i, line) in title_lines.iter().enumerate() {
        put(&mut d, w / 2.0, title_y + ft * ASC + i as f64 * ft * PITCH, ft, "middle", line, ink);
    }
    if g_n == 2 {
        let sw = 18.0 * k;
        let sh = 13.0 * k;
        let mut x = if legend_stacked { 16.0 } else { (w - legend_row_w + 24.0) / 2.0 };
        for (row, (g, iw)) in legend.iter().enumerate() {
            let ry = legend_y + if legend_stacked { row as f64 * legend_row_h } else { 0.0 };
            let cy = ry + legend_row_h / 2.0;
            d.rect(x, cy - sh / 2.0, sw, sh, &series(*g), ink, 1.2);
            put(&mut d, x + sw + 6.0, cy + fs * 0.35, fs, "start", &o.names[*g], ink);
            if !legend_stacked {
                x += iw + 24.0;
            }
        }
    }

    // the value axis as a function of value
    let span = (s.hi - s.lo).max(1e-12);
    let vpos = |v: f64| -> f64 {
        let f = ((v - s.lo) / span).clamp(0.0, 1.0);
        if o.horizontal { l + f * plot_w } else { bottom - f * plot_h }
    };
    // gridlines
    if o.grid {
        for (v, _) in &ticks {
            let p = vpos(*v);
            if o.horizontal { d.line(p, top, p, bottom, grid_col, grid_w) } else { d.line(l, p, w - r, p, grid_col, grid_w) };
        }
    }
    // bars
    let bar_geom = |j: usize, g: usize| -> (f64, f64, f64, f64) {
        // (x, y, width, height) of the bar of the j-th drawn category, group g
        let i = idx[j];
        let v = h[i][g];
        let (a, b) = (vpos(s.lo), vpos(v.min(s.hi)));
        let start = j as f64 * slot + (slot - cluster) / 2.0 + g as f64 * bar_t;
        if o.horizontal {
            (a, top + start, (b - a).max(0.0), bar_t)
        } else {
            (l + start, b, bar_t, (a - b).max(0.0))
        }
    };
    if show_bars {
        for j in 0..n {
            for g in 0..g_n {
                let (x, yy, bw, bh) = bar_geom(j, g);
                if bw > 0.0 && bh > 0.0 {
                    d.rect(x, yy, bw, bh, &series(g), ink, 1.4);
                }
            }
        }
    }
    // axes over the bars' feet
    d.line(l, bottom, w - r, bottom, ink, 2.0);
    d.line(l, top, l, bottom, ink, 2.0);
    for (v, label) in &ticks {
        let p = vpos(*v);
        if o.horizontal {
            d.line(p, bottom, p, bottom + 6.0, ink, 1.6);
            if show_ticks {
                put(&mut d, p, bottom + 8.0 + fs * ASC, fs, "middle", label, ink);
            }
        } else {
            d.line(l - 6.0, p, l, p, ink, 1.6);
            if show_ticks {
                put(&mut d, l - 10.0, p + fs * 0.35, fs, "end", label, ink);
            }
        }
    }
    // axis titles
    if o.horizontal {
        for (i, line) in ct_lines.iter().enumerate() {
            put_up(&mut d, 14.0 + fa * ASC + i as f64 * fa * PITCH, (top + bottom) / 2.0, fa, "middle", line, ink);
        }
    } else {
        for (i, line) in vt_lines.iter().enumerate() {
            put_up(&mut d, 14.0 + fa * ASC + i as f64 * fa * PITCH, (top + bottom) / 2.0, fa, "middle", line, ink);
        }
    }
    for (i, line) in below_lines.iter().enumerate() {
        put(&mut d, (l + w - r) / 2.0, below_title_y + fa * ASC + i as f64 * fa * PITCH, fa, "middle", line, ink);
    }
    // category names
    for (j, lines) in name_lines.iter().enumerate() {
        if o.horizontal {
            let cy = top + (j as f64 + 0.5) * slot;
            let y0 = cy - (lines.len() as f64 - 1.0) * fs * PITCH / 2.0 + fs * 0.35;
            for (m, line) in lines.iter().enumerate() {
                put(&mut d, l - 12.0, y0 + m as f64 * fs * PITCH, fs, "end", line, ink);
            }
        } else {
            let cx = l + (j as f64 + 0.5) * slot;
            if names_up {
                put_up(&mut d, cx + fs * 0.33, bottom + 8.0, fs, "end", &lines[0], ink);
            } else {
                for (m, line) in lines.iter().enumerate() {
                    put(&mut d, cx, bottom + 8.0 + fs * ASC + m as f64 * fs * PITCH, fs, "middle", line, ink);
                }
            }
        }
    }
    // the cumulative line (Pareto): points at the bars' centres, the right axis 0-100%
    let mut line_pts: Vec<(f64, f64)> = Vec::new();
    if o.pareto {
        let mut cum = 0u64;
        for j in 0..n {
            cum += t.cats[idx[j]].counts[0];
            let f = cum as f64 / totals[0].max(1) as f64;
            line_pts.push((l + (j as f64 + 0.5) * slot, bottom - f * plot_h));
        }
        if key {
            // LOW: on a percent chart the value axis is 0-100% too, so the right axis would be the
            // same axis twice; the line is read off the left one
            if !o.percent {
                d.line(w - r, top, w - r, bottom, ink, 2.0);
                for pc in [0, 20, 40, 60, 80, 100] {
                    let yy = bottom - pc as f64 / 100.0 * plot_h;
                    d.line(w - r, yy, w - r + 6.0, yy, ink, 1.6);
                    put(&mut d, w - r + 10.0, yy + fs * 0.35, fs, "start", &format!("{pc}%"), ink);
                }
                put_up(&mut d, w - 12.0 - fa * DES, (top + bottom) / 2.0, fa, "middle", "Cumulative percent", ink);
            }
            d.polyline(&line_pts, ink, 2.0);
            for (x, yy) in &line_pts {
                d.circle(*x, *yy, 3.5, "#ffffff", ink, 1.6);
            }
        }
    }
    // value labels, each in its own bar's column
    if show_vals {
        for j in 0..n {
            for g in 0..g_n {
                let text = &labels[idx[j]][g];
                let (x, yy, bw, bh) = bar_geom(j, g);
                let lw = tw(text, fv);
                if o.horizontal {
                    put(&mut d, x + bw + 6.0, yy + bh / 2.0 + fv * 0.35, fv, "start", text, ink);
                    continue;
                }
                let cx = x + bw / 2.0;
                // the label's box sits on the bar; with a cumulative line it must also clear the
                // line (and its markers) across the label's width, so it goes above the line there
                // the label's box sits on its bar, in the bar's own column; with a cumulative line it
                // must also clear the line (and its markers) across the label's width: then it goes
                // inside the top of the bar on a white chip, or else above the line there
                let ext = if vals_up { lw } else { fv * (ASC + DES) };
                let mut bot = yy - 5.0;
                let mut chip = false;
                if key && !line_pts.is_empty() {
                    let half = if vals_up { fv * 0.7 } else { lw / 2.0 } + 3.0 + 3.5;
                    let (x0, x1) = (cx - half, cx + half);
                    let at = |px: f64| -> Option<f64> {
                        line_pts.windows(2).find(|w| w[0].0 <= px && px <= w[1].0).map(|w| w[0].1 + (w[1].1 - w[0].1) * (px - w[0].0) / (w[1].0 - w[0].0))
                    };
                    let mut ys: Vec<f64> = [x0, x1].iter().filter_map(|px| at(*px)).collect();
                    ys.extend(line_pts.iter().filter(|p| p.0 >= x0 && p.0 <= x1).map(|p| p.1));
                    if !ys.is_empty() {
                        let line_top = ys.iter().cloned().fold(f64::INFINITY, f64::min) - 3.5;
                        let line_low = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max) + 3.5;
                        let clear = |b: f64| b - ext > line_low + 3.0 || b < line_top - 3.0;
                        let inside = yy + 4.5 + 4.0 + ext;
                        if clear(bot) {
                        } else if inside <= bottom - 4.0 && clear(inside) && (if vals_up { fv * 1.25 } else { lw }) + 4.0 <= bw - 4.0 {
                            bot = inside;
                            chip = true;
                        } else {
                            bot = line_top - 3.0;
                        }
                    }
                }
                if chip {
                    let cw = if vals_up { fv * 1.25 } else { lw } + 4.0;
                    d.rect(cx - cw / 2.0, bot - ext - 2.0, cw, ext + 4.0, "#ffffff", "none", 0.0);
                }
                if vals_up {
                    put_up(&mut d, cx + fv * 0.33, bot, fv, "start", text, ink);
                } else {
                    put(&mut d, cx, bot - fv * DES, fv, "middle", text, ink);
                }
            }
        }
    }
    // the table
    if !cols.is_empty() {
        let x0 = (w - table_w) / 2.0;
        let mut xs = Vec::new();
        let mut x = x0;
        for c in &cols {
            xs.push(x);
            x += c.w + 16.0;
        }
        let x_end = x;
        let mut ty = table_y;
        d.line(x0, ty, x_end, ty, ink, 1.4);
        for (ci, c) in cols.iter().enumerate() {
            for (m, line) in c.head.iter().enumerate() {
                let yy = ty + 4.0 + fs * ASC + m as f64 * fs * PITCH;
                if ci == 0 { put(&mut d, xs[ci] + 8.0, yy, fs, "start", line, ink) } else { put(&mut d, xs[ci] + 8.0 + c.w / 2.0, yy, fs, "middle", line, ink) }
            }
        }
        ty += head_lines as f64 * fs * PITCH + 8.0;
        d.line(x0, ty, x_end, ty, ink, 1.0);
        for (rw, nl) in row_lines.iter().enumerate() {
            let rh = *nl as f64 * fs * PITCH + 6.0;
            if rw == n {
                d.line(x0, ty, x_end, ty, ink, 1.0);
            }
            for (ci, c) in cols.iter().enumerate() {
                let base = ty + 3.0 + fs * ASC;
                if ci == 0 {
                    for (m, line) in wrap(&c.cells[rw], c.w, fs, true).unwrap_or_default().iter().enumerate() {
                        put(&mut d, xs[ci] + 8.0, base + m as f64 * fs * PITCH, fs, "start", line, ink);
                    }
                } else if c.answer && !key || (rw == n && !key) {
                    d.line(xs[ci] + 10.0, base + fs * DES, xs[ci] + c.w + 6.0, base + fs * DES, ink, 1.0);
                } else {
                    put(&mut d, xs[ci] + 8.0 + c.w / 2.0, base, fs, "middle", &c.cells[rw], ink);
                }
            }
            ty += rh;
        }
        d.line(x0, ty, x_end, ty, ink, 1.4);
    }
    // the questions
    let mut qy = q_y;
    for (ql, al) in &q_layout {
        for line in ql {
            put(&mut d, 20.0, qy + fs * ASC, fs, "start", line, ink);
            qy += fs * PITCH;
        }
        if key {
            for line in al {
                put(&mut d, 36.0, qy + fs * ASC, fs, "start", line, ink);
                qy += fs * PITCH;
            }
        } else {
            put(&mut d, 36.0, qy + fs * ASC, fs, "start", "Answer:", ink);
            let ax = 36.0 + tw("Answer:", fs) + 6.0;
            d.line(ax, qy + fs * ASC + 2.0, (ax + 220.0 * k).min(w - 20.0), qy + fs * ASC + 2.0, ink, 1.0);
            qy += al.len() as f64 * fs * PITCH;
        }
        qy += 8.0;
    }
    d.finish()
}

#[cfg(test)]
mod tests;
