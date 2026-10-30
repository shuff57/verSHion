//! A chi-square goodness-of-fit table: the counts you observed beside the counts a ratio predicts, each category's
//! contribution (O - E)^2 / E, and the statistic, degrees of freedom, p-value and decision. This is the genetics
//! test (does 315 : 101 : 108 : 32 fit 9 : 3 : 3 : 1?), not the test of independence drawn by `chi-square-table`,
//! where the expected counts come from the row and column totals. Drawn with `svg::Doc` only.
//!
//! The observed counts and the ratio are the question, so they always print. The expected counts, the contributions
//! and the whole statistics block are the answer and blank while Show answers is off.
use super::super::circle::num;
use super::super::stats::dist::chisq_cdf;
use super::*;

pub const ID: &str = "chi-square-fit";

const INK: &str = "#111111";
const BLANK: &str = "_____";
const W: f64 = 640.0;
const HEAD_Y: f64 = 46.0;
const ROW_H: f64 = 26.0;
const FS: f64 = 14.0;
const MAX_CATS: usize = 6;
/// Column right edges: the three number columns.
const COL_O: f64 = 330.0;
const COL_E: f64 = 450.0;
const COL_T: f64 = 612.0;
const ROW_FS: f64 = 15.0;

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Biology,
        title: "Chi-square Goodness of Fit",
        blurb: "Counts beside the counts a ratio predicts, with each category's contribution, the chi-square statistic, its degrees of freedom, the p-value and the decision.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::text("names", "Categories (comma separated)", "Round yellow, Round green, Wrinkled yellow, Wrinkled green")
            .hint("Leave empty for Category 1, Category 2, ..."),
        ParamDef::text("obs", "Observed counts", "315, 101, 108, 32"),
        ParamDef::text("ratio", "Expected ratio", "9, 3, 3, 1"),
        ParamDef::float("alpha", "Significance level (\u{3b1})", 0.01, 0.10, 0.01, 0.05),
        ParamDef::int("dec", "Decimal places", 1, 3, 2),
        answers_param().hint("Off keeps the observed counts and the ratio and blanks the expected counts, the contributions and the statistics"),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p) }

pub fn split_list(s: &str) -> Vec<String> {
    s.split([',', ';']).map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()
}

/// The non-negative numbers in a list, ignoring anything that is not one.
fn numbers(s: &str) -> Vec<f64> {
    split_list(s).iter().filter_map(|t| t.parse::<f64>().ok()).filter(|v| v.is_finite() && *v >= 0.0).collect()
}

/// A count as the test uses it: the typed number rounded once, exactly, to six decimals.
#[derive(Clone, Debug)]
struct Count {
    /// The typed text.
    typed: String,
    /// The rounded number in decimal, ungrouped and without trailing zero decimals ("929059410.673343", "0").
    text: String,
    /// The rounded number as a double. Every value the test computes starts from this.
    value: f64,
    /// The rounded number in integer units at its own decimals, while it fits an i128 (it does below about 1e31).
    units: Option<i128>,
    /// Its own decimals, 0 to 6.
    dec: i64,
    /// The typed number has a nonzero digit past the sixth decimal, so rounding changed it.
    changed: bool,
}

/// (round 11 blocker) A typed decimal ("929059410.6733431", "1e-14", "2.5e-6") rounded half up to six decimals, in
/// exact decimal arithmetic on the typed digits, so the count the page prints is the count it computes with. None when
/// the text is not a plain decimal (the caller falls back to the double's own shortest decimal).
fn exact_six(t: &str) -> Option<Count> {
    let typed = t.trim();
    let body = typed.strip_prefix('+').or_else(|| typed.strip_prefix('-')).unwrap_or(typed);
    let (mant, exp) = match body.find(['e', 'E']) { Some(i) => (&body[..i], Some(&body[i + 1..])), None => (body, None) };
    let exp: i64 = match exp {
        None => 0,
        Some(e) => {
            let (neg, d) = match e.strip_prefix('-') { Some(r) => (true, r), None => (false, e.strip_prefix('+').unwrap_or(e)) };
            if d.is_empty() || !d.bytes().all(|b| b.is_ascii_digit()) { return None; }
            let x = d.bytes().fold(0i64, |x, b| (x * 10 + (b - b'0') as i64).min(1_000_000));
            if neg { -x } else { x }
        }
    };
    let (ip, fp) = mant.split_once('.').unwrap_or((mant, ""));
    if (ip.is_empty() && fp.is_empty()) || !ip.bytes().chain(fp.bytes()).all(|b| b.is_ascii_digit()) { return None; }
    let mut ds: Vec<u8> = ip.bytes().chain(fp.bytes()).map(|b| b - b'0').collect();
    // ds[..point] is the whole part of the number
    let mut point = ip.len() as i64 + exp;
    let lead = ds.iter().take_while(|&&d| d == 0).count();
    ds.drain(..lead);
    point -= lead as i64;
    let zero = |changed: bool| Count { typed: typed.to_string(), text: "0".to_string(), value: 0.0, units: Some(0), dec: 0, changed };
    if ds.is_empty() { return Some(zero(false)); }
    // ds[..keep] are the digits kept: the whole part and six decimals (a finite double keeps this below ~320)
    let keep = point + 6;
    if keep > 400 { return None; }
    let (mut kept, rest): (Vec<u8>, Vec<u8>) = if keep <= 0 {
        (Vec::new(), ds.clone())
    } else if keep as usize >= ds.len() {
        let mut k = ds.clone();
        k.resize(keep as usize, 0);
        (k, Vec::new())
    } else {
        (ds[..keep as usize].to_vec(), ds[keep as usize..].to_vec())
    };
    let changed = rest.iter().any(|&d| d != 0);
    // the seventh decimal decides; below 1e-7 it is a zero in front of the typed digits
    let up = keep >= 0 && rest.first().is_some_and(|&d| d >= 5);
    if up {
        let mut i = kept.len();
        loop {
            if i == 0 { kept.insert(0, 1); break; }
            i -= 1;
            if kept[i] == 9 { kept[i] = 0; } else { kept[i] += 1; break; }
        }
    }
    let lead = kept.iter().take_while(|&&d| d == 0).count();
    kept.drain(..lead);
    if kept.is_empty() { return Some(zero(changed)); }
    while kept.len() < 7 { kept.insert(0, 0); }
    let digits: String = kept.iter().map(|d| char::from(b'0' + d)).collect();
    let (whole, frac) = digits.split_at(digits.len() - 6);
    let whole = whole.trim_start_matches('0');
    let whole = if whole.is_empty() { "0" } else { whole };
    let frac = frac.trim_end_matches('0');
    let text = if frac.is_empty() { whole.to_string() } else { format!("{whole}.{frac}") };
    let value = text.parse::<f64>().ok().filter(|v| v.is_finite())?;
    let units = if whole.len() + frac.len() <= 37 { format!("{whole}{frac}").parse::<i128>().ok() } else { None };
    Some(Count { typed: typed.to_string(), text, value, units, dec: frac.len() as i64, changed })
}

/// The observed counts, each rounded once to six decimals, ignoring anything that is not a non-negative number.
fn counts(s: &str) -> Vec<Count> {
    split_list(s)
        .iter()
        .filter_map(|t| {
            let v = t.parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0)?;
            exact_six(t).or_else(|| exact_six(&format!("{v}")).map(|c| Count { typed: t.clone(), ..c }))
        })
        .collect()
}

/// The digits a count needs to print in full at its own decimals; a double carries 15.
fn fits_a_double(c: &Count) -> bool { c.units.is_some_and(|u| u < 1_000_000_000_000_000) }

/// One row of the table.
#[derive(Clone, Debug)]
pub struct Row {
    pub name: String,
    pub observed: f64,
    pub expected: f64,
    /// (O - E)^2 / E. Zero when the expected count is zero and nothing was observed there.
    pub term: f64,
}

#[derive(Clone, Debug)]
pub struct Fit {
    pub rows: Vec<Row>,
    pub chi2: f64,
    pub df: usize,
    /// 1 - cdf, i.e. P(X^2 >= chi2). None when the test cannot be run (no rows, or a zero ratio).
    pub p: Option<f64>,
    pub alpha: f64,
    /// The ratio parts actually used, in order, for the line under the table.
    pub ratio: Vec<f64>,
    /// Categories that carry a degree of freedom: those with an expected count.
    pub kept: usize,
    /// A category expected to be empty has counts in it, so the statistic is infinite.
    pub impossible: bool,
}

fn clamp_alpha(v: f64) -> f64 { if v.is_finite() { v.clamp(0.01, 0.10) } else { 0.05 } }

/// The whole test, computed from the typed text.
pub fn fit(p: &Params) -> Fit {
    let alpha = clamp_alpha(p.f("alpha"));
    // (round 11 blocker) the counts as printed: rounded once to six decimals, so "0.0000004" is a count of 0 here too
    let obs: Vec<f64> = counts(p.s("obs")).iter().map(|c| c.value).collect();
    let ratio = numbers(p.s("ratio"));
    let names = split_list(p.s("names"));
    let k = obs.len().min(MAX_CATS).min(ratio.len());
    let total_obs: f64 = obs.iter().take(k).sum();
    let total_ratio: f64 = ratio.iter().take(k).sum();
    let mut rows = Vec::with_capacity(k);
    let mut chi2 = 0.0;
    let mut impossible = false;
    let mut kept = 0usize;
    for i in 0..k {
        let o = obs[i];
        let r = ratio[i];
        let e = if total_ratio > 0.0 { r / total_ratio * total_obs } else { 0.0 };
        let term = if e > 0.0 {
            let t = (o - e) * (o - e) / e;
            // (round 4) counts exactly in the ratio leave a floating-point crumb (1e-13); printing it as a nonzero
            // statistic turns a perfect fit at huge counts into "χ² = 4.36e−13"
            if t < 1e-12 { 0.0 } else { t }
        } else if o > 0.0 {
            // a category the ratio says cannot happen has counts in it: the statistic is infinite, and the test
            // cannot be run at all rather than reporting a made-up finite number
            impossible = true;
            0.0
        } else {
            0.0
        };
        // a category with no expected count and no observed count is not part of the test and carries no degree of
        // freedom; counting it gave "df = 1" and a confident "do not reject" on a test that never ran
        if r > 0.0 || o > 0.0 { kept += 1; }
        chi2 += term;
        let name = names.get(i).cloned().unwrap_or_else(|| format!("Category {}", i + 1));
        rows.push(Row { name, observed: o, expected: e, term });
    }
    let df = kept.saturating_sub(1);
    // (round 6 blocker) with no observations every expected count is 0 · ratio = 0, so (O - E)² / E is 0/0 for every
    // row: there is no statistic to compute. It used to fall through to χ² = 0, df = 1 and a confident "p = 1.00 …
    // do not reject". A test with nothing observed cannot be run.
    let p = if total_obs > 0.0 && !impossible && df > 0 && total_ratio > 0.0 && chi2.is_finite() {
        Some((1.0 - chisq_cdf(chi2, df as f64)).clamp(0.0, 1.0))
    } else {
        None
    };
    Fit { rows, chi2, df, p, alpha, ratio: ratio.iter().take(k).cloned().collect(), kept, impossible }
}

pub fn warnings(p: &Params) -> Vec<String> {
    let mut out = Vec::new();
    let cnts = counts(p.s("obs"));
    let obs: Vec<f64> = cnts.iter().map(|c| c.value).collect();
    let ratio = numbers(p.s("ratio"));
    // tokens that are not numbers are dropped silently by the parser; say so, or "100, x, 50" quietly becomes a
    // two-category test on 100 and 50
    for (what, text, parsed) in [("observed counts", p.s("obs"), obs.len()), ("ratio parts", p.s("ratio"), ratio.len())] {
        let tokens: Vec<String> = split_list(&text).iter().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
        let bad: Vec<&String> = tokens.iter().filter(|t| t.parse::<f64>().map(|v| !(v.is_finite() && v >= 0.0)).unwrap_or(true)).collect();
        if !bad.is_empty() {
            out.push(format!("These {} are not counts and were ignored: {}.", what, bad.iter().map(|b| format!("\u{201c}{b}\u{201d}")).collect::<Vec<_>>().join(", ")));
        }
    }
    // (round 10 minor) a count with more than six decimals is rounded to six on the page ("0.0000001" prints as 0):
    // say so, rather than change what was typed without a word. (round 11) Exactly when the typed digits change, at
    // any size ("1e-14", "929059410.6733431"), and naming what the test uses instead, because that is what it uses.
    let fine: Vec<String> = cnts.iter().filter(|c| c.changed).map(|c| format!("\u{201c}{}\u{201d}", c.typed)).collect();
    if !fine.is_empty() {
        let used: Vec<String> = cnts.iter().filter(|c| c.changed).map(|c| group(&c.text)).collect();
        let pairs: Vec<String> = fine.iter().zip(&used).enumerate().map(|(i, (f, u))| if i == 0 { format!("{f} is used as {u}") } else { format!("{f} as {u}") }).collect();
        out.push(format!("Counts are used to six decimals, so {}.", pairs.join(", ")));
    }
    if obs.is_empty() {
        out.push("There are no observed counts to test.".to_string());
    }
    // (round 6 blocker) an all-zero observation column is not a fit: nothing has been counted, so there is nothing to
    // test and no statistic to compute. Without this the editor said nothing and the page reported "p = 1.00 … do not
    // reject" on a 0/0.
    if !obs.is_empty() && obs.iter().all(|v| *v == 0.0) {
        // (round 11 blocker) a count that rounds to 0 at six decimals is a zero count
        let at_six = if cnts.iter().any(|c| c.changed) { " at six decimals" } else { "" };
        out.push(format!("Every observed count is zero{at_six}, so there is nothing to test and the statistic cannot be computed."));
    }
    if ratio.is_empty() {
        out.push("There is no expected ratio to test them against.".to_string());
    }
    if !obs.is_empty() && !ratio.is_empty() && obs.len() != ratio.len() {
        out.push(format!(
            "There are {} counts and {} ratio parts; the test uses the first {} pairs.",
            obs.len(),
            ratio.len(),
            obs.len().min(ratio.len())
        ));
    }
    if obs.len() > MAX_CATS || ratio.len() > MAX_CATS {
        out.push(format!("Only the first {MAX_CATS} categories are drawn."));
    }
    let f = fit(p);
    if f.rows.iter().any(|r| r.expected == 0.0 && r.observed > 0.0) {
        out.push("A category is expected to be empty but something was counted there, so the statistic cannot be computed.".to_string());
    }
    if f.impossible {
        out.push("A category the ratio expects to be empty has counts in it, so the statistic is infinite and the test cannot be run.".to_string());
    }
    if f.df == 0 && !f.rows.is_empty() {
        out.push("Only one category has an expected count, which leaves no degrees of freedom, so there is nothing to test.".to_string());
    }
    out
}

fn text_el(x: f64, y: f64, size: f64, anchor: &str, fill: &str, s: &str) -> String {
    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{fill}">{}</text>"##,
        svg::esc(s)
    )
}

/// A number that never rounds a real value to "0" — a contribution of 0.0004 is real and belongs on the page.
fn val(v: f64, dec: i64) -> String {
    let zeros = |s: &str| s.trim_start_matches('\u{2212}').chars().filter(|c| c.is_ascii_digit()).all(|c| c == '0');
    let s = num(v, dec);
    if v != 0.0 && zeros(&s) {
        let more = num(v, dec + 2);
        if !zeros(&more) { return more; }
        let six = format!("{:.6}", v);
        let six = six.trim_end_matches('0').trim_end_matches('.');
        if !zeros(six) { return six.to_string(); }
        return format!("{v:.2e}").replace('-', "\u{2212}");
    }
    s
}

/// A table column value at a fixed number of decimals, so the column lines up — but a real value that would round
/// to all zeros falls back to the never-zero printer, because 0.0002 is not zero.
fn col(v: f64, dec: i64) -> String {
    let s = format!("{v:.*}", dec as usize);
    if v != 0.0 && s.trim_start_matches('\u{2212}').chars().filter(|c| c.is_ascii_digit()).all(|c| c == '0') {
        return val(v, dec);
    }
    s.replace('-', "\u{2212}")
}

/// (round 3) Thousands separators on an already-formatted number, so the Expected column reads "31,275.00" beside an
/// Observed column that already groups ("31,500") instead of the two mixing formats in one row.
fn group(s: &str) -> String {
    let (sign, rest) = match s.strip_prefix('\u{2212}') { Some(r) => ("\u{2212}", r), None => ("", s) };
    let (int, frac) = match rest.split_once('.') { Some((i, f)) => (i, Some(f)), None => (rest, None) };
    let mut back = String::new();
    for (i, c) in int.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 { back.push(','); }
        back.push(c);
    }
    let int: String = back.chars().rev().collect();
    match frac { Some(f) => format!("{sign}{int}.{f}"), None => format!("{sign}{int}") }
}

/// Greedy word wrap to `max` characters.
fn wrap(s: &str, max: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for word in s.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > max {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() { cur.push(' '); }
        cur.push_str(word);
    }
    if !cur.is_empty() { out.push(cur); }
    out
}

/// The p-value, written the way a report writes it: two significant figures, and "< 0.001" below that.
pub fn p_text(p: f64) -> String {
    if p < 0.001 { return "< 0.001".to_string(); }
    let text = if p >= 0.1 { format!("{p:.2}") } else if p >= 0.01 { format!("{p:.3}") } else { format!("{p:.4}") };
    text.replace('-', "\u{2212}")
}

/// "p = 0.93", or "p < 0.001" — writing "p = < 0.001" reads as a typo.
pub fn p_phrase(p: f64) -> String {
    let t = p_text(p);
    if t.starts_with('<') { format!("p {t}") } else { format!("p = {t}") }
}

/// The decision in words, from the p-value and the significance level. (round 3) Judged on the p-value as PRINTED: at
/// p = 0.0503 the page said "p = 0.050 is below α = 0.05", which contradicts itself, so an equal printed p is called
/// borderline rather than placed on either side.
pub fn decision(f: &Fit) -> String {
    match f.p {
        None => "The test cannot be run on these numbers".to_string(),
        Some(p) => {
            let a = num(f.alpha, 3);
            let t = p_text(p);
            let shown = if t.starts_with('<') { 0.0 } else { t.parse::<f64>().unwrap_or(p) };
            if shown < f.alpha {
                format!("{} is below \u{3b1} = {}, so reject the hypothesis: the counts do not fit the ratio.", p_phrase(p), a)
            } else if shown > f.alpha {
                format!("{} is above \u{3b1} = {}, so do not reject the hypothesis: the counts are consistent with the ratio.", p_phrase(p), a)
            } else {
                format!("{} is level with \u{3b1} = {} at this precision: a borderline result, so do not reject the hypothesis.", p_phrase(p), a)
            }
        }
    }
}

pub fn build(p: &Params) -> String {
    let f = fit(p);
    let dec = p.i("dec").clamp(1, 3);
    let print = p.s("style") == "Print";
    let ans = p.b(SHOW_ANSWERS);
    let lab = svg::label_factor(p.s("label_size"));
    let ink = if print { "#555555" } else { INK };

    let n = f.rows.len();
    // every vertical offset scales with the label size, or Large prints the header into the title and the givens
    // into the statistics block
    let (head_y, row_h) = (HEAD_Y * lab, ROW_H * lab);
    let table_bottom = head_y + 8.0 * lab + row_h * (n as f64 + 1.0);
    // (round 8 minor) observed counts are whole numbers the student is given: printed in full with grouping up to 1e15
    // (at 1e7 they went "1.23e7" and could not be recovered, and 5,000,000 + 5,000,001 totalled "1.00e7"). The total is
    // the sum of the cells as printed. Only derived values go scientific.
    // (round 9 minor) The same for counts with decimals: "5.5, 10.25" totalled 15.75 in the table but "16 observations"
    // in the givens, and "12345678, 0.5" totalled "1.23e7". Each count prints as typed (up to six decimals, grouped),
    // the total is their exact sum in integer units at the finest decimals any of them carries, and the Observed
    // total, the Expected total and the givens print that one string. A column is plain while every cell is below 1e15
    // and exactly representable; past that the whole column (cells and total) follows one rule: plain below 1e7,
    // scientific from 1e7.
    let total_obs: f64 = f.rows.iter().map(|r| r.observed).sum();
    // (round 11) the same rounded counts the fit used, with their exact digits
    let cnts: Vec<Count> = counts(p.s("obs")).into_iter().take(f.rows.len()).collect();
    let obs_dec: Vec<i64> = cnts.iter().map(|c| c.dec).collect();
    let col_dec = obs_dec.iter().copied().max().unwrap_or(0);
    // (round 10 minor) The column used to be plain only while every count scaled to the COLUMN's decimals stayed under
    // 9e15, so "1000000000000, 0.123456" (1e18 units at six decimals) went "1.00e12" and "0.12". Each count is exact in
    // integer units at its OWN decimals (under 2^53), and is raised to the column's decimals in i128, where the sum of
    // any number of counts below 1e15 at six decimals fits; so the column is plain whenever each count is below 1e15
    // and exact at its own decimals, and the total is still the exact sum of the printed cells.
    // (round 11 minor 1) ...and a count prints in full only while that takes at most 15 digits, all a double carries:
    // "15410925012.328436" printed "15,410,925,012.32844", dropping a typed digit. A column with such a count follows
    // the rule for huge counts (scientific from 1e7, as before round 10).
    let own_units: Vec<i128> = cnts.iter().map(|c| c.units.unwrap_or(0)).collect();
    let obs_full = cnts.iter().all(fits_a_double);
    let col_units: Vec<i128> = if obs_full { own_units.iter().zip(&obs_dec).map(|(u, &d)| u * 10i128.pow((col_dec - d) as u32)).collect() } else { Vec::new() };
    let units_text = |u: i128, d: i64| -> String {
        let neg = u < 0;
        let u = u.abs();
        let p = 10i128.pow(d as u32);
        let body = if d == 0 { format!("{u}") } else { format!("{}.{:0w$}", u / p, u % p, w = d as usize) };
        let g = group(&body);
        if neg { format!("\u{2212}{g}") } else { g }
    };
    let big = |v: f64| v.is_finite() && v.abs() >= 1e7;
    // (round 5) an overflowing value prints "inf"; the rest of the figure writes ∞
    let sci = |v: f64| if v == 0.0 { "0".to_string() } else if !v.is_finite() { "\u{221e}".to_string() } else { format!("{v:.2e}").replace('-', "\u{2212}") };
    let fmt_cnt = |v: f64| if big(v) || !v.is_finite() { sci(v) } else { num(v, 2) };
    let obs_text: Vec<String> = if obs_full {
        own_units.iter().zip(&obs_dec).map(|(&u, &d)| units_text(u, d)).collect()
    } else {
        // (round 11 minor 2) a small count beside a huge one still prints as it is used ("0.000001" printed "0")
        cnts.iter().map(|c| match c.units { Some(u) if fits_a_double(c) && !big(c.value) => units_text(u, c.dec), _ => fmt_cnt(c.value) }).collect()
    };
    let total_text_obs = if obs_full {
        units_text(col_units.iter().sum(), col_dec)
    } else {
        fmt_cnt(total_obs)
    };
    // the Expected column follows its own column's rule: plain and grouped while the Observed column is (the expected
    // counts sum to the same total, so none reaches 1e15 then), and its total is the Observed total, printed the same
    // way. (round 9 minor) "5,000,000.50" twice above an Expected total of "1.00e7" beside an Observed "10,000,001".
    // (round 10, accepted) The Expected cells need not add up to the Expected total at the printed decimals ("10.3"
    // three times beside 31): the total IS the observed total by construction (E sums to O), and each cell is the
    // true expected count rounded for display. Forcing the cells to sum would print an expected count that is wrong.
    // (round 11 minor 1) ...but only while every expected cell prints in at most 15 digits: past that a double cannot
    // carry the last printed digit ("500,000,000,500,000.25" for an exact …500,000.2643), and the column follows the
    // rule for huge values, scientific from 1e7
    let digits = |s: &str| s.chars().filter(|c| c.is_ascii_digit()).skip_while(|c| *c == '0').count();
    let exp_full = obs_full && f.rows.iter().all(|r| !r.expected.is_finite() || { let t = col(r.expected, dec); t.contains('e') || digits(&t) <= 15 });
    let fmt_exp = |v: f64| if !v.is_finite() { sci(v) } else if exp_full { group(&col(v, dec)) } else if big(v) { sci(v) } else { group(&col(v, dec)) };
    let exp_text: Vec<String> = f.rows.iter().map(|r| fmt_exp(r.expected)).collect();
    // the canvas widens when a column's widest printed number would not fit between it and the column on its left
    // (a 15-digit expected count with its decimals is 22 characters)
    let cw = FS * lab * 0.58;
    let wide = |v: &[String]| v.iter().map(|t| t.chars().count()).max().unwrap_or(0) as f64 * cw;
    let w_obs = wide(&obs_text).max(total_text_obs.chars().count() as f64 * cw);
    let w_exp = wide(&exp_text).max(total_text_obs.chars().count() as f64 * cw);
    let w = (W * (1.0 + 0.5 * (lab - 1.0))).max(W).max((w_exp + 24.0) / (0.703 - 0.515)).max((24.0 + 6.0 * cw + 8.0 + w_obs) / 0.515);
    // the givens: the ratio the expected counts come from, always printed (in the table's own notation when huge).
    // (round 6) a ratio part that is not a whole number keeps its decimals: rounding them to integers printed
    // "1.5 : 2.5 : 1.5" as "2 : 2 : 2", which contradicts the expected counts beside it. A part too large for an i64
    // printed a saturated 9223372036854775807: it is written in scientific notation instead, the same way the expected
    // counts built from it are.
    let ratio_text: Vec<String> = f
        .ratio
        .iter()
        .map(|v| {
            if !v.is_finite() { "\u{221e}".to_string() }
            else if (v - v.round()).abs() < 1e-9 && v.abs() < 1e15 { format!("{}", v.round() as i64) }
            else if v.abs() >= 1e15 { format!("{v:.2e}").replace('-', "\u{2212}") }
            else { format!("{v}").replace('-', "\u{2212}") }
        })
        .collect();
    let total_shown = total_text_obs.clone();
    let givens = format!("Expected from the ratio {} out of {} observations", ratio_text.join(" : "), total_shown);
    // (round 5) long ratio parts ran off the canvas: wrapped to the width, the lines stacking up into the gap above
    // the statistics block
    let givens_chars = (((w - 48.0) / (12.0 * lab * 0.56)).floor() as usize).max(20);
    let givens_lines = wrap(&givens, givens_chars);
    // (round 8 minor) the givens' last line sat 14 px over the 15 px \u{3c7}\u{b2} line, its descenders a pixel from
    // "Chi-square": the statistics block moves down to leave a real gap, and each wrapped givens line pushes it further
    // rather than stacking up into the table's bottom rule
    let givens_gap = 20.0 * lab;
    let stats_y0 = table_bottom + (52.0 + 15.0 * (givens_lines.len().max(1) - 1) as f64) * lab;
    // the columns are fractions of the width, so they spread as the canvas grows
    let (col_o, col_e, col_t) = (w * 0.515, w * 0.703, w * 0.956);
    let decision_text = if ans { decision(&f) } else { "Decision  ______________________________________".to_string() };
    // (round 5) wrapped to the canvas, not to a fixed 70 characters: at Large the line ran past the right edge
    let dec_chars = (((w - 48.0) / (ROW_FS * lab * 0.58)).floor() as usize).max(24);
    let decision_lines = if ans { wrap(&decision_text, dec_chars) } else { vec![decision_text.clone()] };
    let mut d = Doc::new(w, stats_y0 + (3.0 + decision_lines.len() as f64) * ROW_FS * lab * 1.35 + 12.0);
    d.describe("Chi-square goodness of fit", "Observed counts beside the counts a ratio predicts, with the statistic, degrees of freedom and p-value");

    d.raw(&format!(
        r##"<text x="24.00" y="{:.2}" font-size="{}" text-anchor="start" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{}">Chi-square goodness of fit</text>"##,
        30.0 * lab,
        17.0 * lab,
        ink
    ));
    // nothing to test: say what is missing rather than drawing a table of zeros and "df = 0 (0 categories - 1)"
    if f.rows.is_empty() {
        let msg = if numbers(p.s("obs")).is_empty() { "Type the observed counts, one per category." } else { "Type an expected ratio with the same number of parts as there are counts." };
        d.raw(&text_el(24.0, 78.0 * lab, 13.0 * lab, "start", ink, msg));
        return d.finish();
    }

    // the header
    let head = FS * lab;
    d.raw(&text_el(24.0, head_y, head, "start", ink, "Category"));
    d.raw(&text_el(col_o, head_y, head, "end", ink, "Observed (O)"));
    d.raw(&text_el(col_e, head_y, head, "end", ink, "Expected (E)"));
    d.raw(&text_el(col_t, head_y, head, "end", ink, "(O \u{2212} E)\u{b2} / E"));
    let rule = |d: &mut Doc, y: f64, sw: f64| { d.line(20.0, y, w - 20.0, y, ink, sw); };
    rule(&mut d, head_y + 8.0 * lab, 1.6);

    // the rows
    // huge counts: one compact format for the whole table, or "1 × 10^12" sits beside "999999999999.56".
    // (round 5) A contribution can be enormous from a tiny expected count while every count is small, so the term is
    // part of the test too — printed in plain decimals it ran across the Expected and Total columns.
    // (round 6 blocker) nothing observed: every (O − E)² / E is 0/0, so there is no contribution, statistic or test.
    let no_obs = total_obs == 0.0;
    // a test that cannot run has no statistic to print: zero observations (0/0) or a single category (no degrees of
    // freedom) leave χ² undefined rather than a meaningful 0.00
    let no_test = no_obs || f.df == 0;
    // (round 7 minor) scientific notation belongs only to the values that need it. A huge typed ratio (1 : 1e20) made
    // the entire table "huge", so the small observed counts printed "1.00e0" and the total "2.00e0". A value is
    // scientific only when it is itself ≥ 1e7; small counts stay counts, while the huge ratio/expected stay scientific.
    // (round 5) the contributions are grouped like the counts (1,234.56), so one row does not mix two formats
    // (round 9 minor) one rule for the column and its total: when any contribution or the statistic reaches 1e7 the
    // whole column is scientific ("6,172,838.25" twice above a Total of "1.23e7" mixed two formats in one column)
    let term_sci = f.rows.iter().any(|r| big(r.term)) || big(f.chi2);
    let fmt_term = |v: f64| if !v.is_finite() { "\u{221e}".to_string() } else if term_sci || big(v) { sci(v) } else { group(&col(v, dec.max(2))) };
    // a long category name would print through the three number columns: the room it has is what is left of the
    // Observed column once its widest printed number is drawn (round 5: a grouped count ran under the name)
    let widest_obs = obs_text.iter().map(|s| s.chars().count()).max().unwrap_or(1) as f64 * FS * lab * 0.56;
    let name_chars = (((col_o - 24.0 - widest_obs - 8.0) / (FS * lab * 0.56)).floor() as usize).max(6);
    let short = |n: &str| -> String {
        if n.chars().count() <= name_chars { n.to_string() } else {
            let head: String = n.chars().take(name_chars.saturating_sub(1)).collect();
            format!("{head}\u{2026}")
        }
    };
    // (round 3) The total is the sum of what is PRINTED, and at the precision the column actually prints: `col` shows
    // more decimals when a real contribution would otherwise print as 0, so 0.0033 + 0.01 must total 0.0133, not 0.01.
    // (round 4) a category expected to be empty but counted has an INFINITE contribution, not 0: the row used to print
    // "0.00" beside a Total of ∞
    let term_text: Vec<String> = f
        .rows
        .iter()
        .map(|r| if r.expected == 0.0 && r.observed > 0.0 { "\u{221e}".to_string() } else if no_test { "\u{2014}".to_string() } else { fmt_term(r.term) })
        .collect();
    // (round 5) an overflowed contribution is infinite too: the total must not print 0 beside rows that print ∞
    let infinite = f.impossible || term_text.iter().any(|t| t == "\u{221e}") || !f.chi2.is_finite();
    let term_dec = term_text
        .iter()
        .map(|s| s.split_once('.').map_or(0, |(_, d)| d.chars().take_while(char::is_ascii_digit).count()) as i64)
        .max()
        .unwrap_or(0)
        .max(dec.max(2));
    let printed_chi2: f64 = term_text.iter().map(|s| s.replace(',', "").replace('\u{2212}', "-").parse::<f64>().unwrap_or(0.0)).sum();

    let mut y = head_y + 8.0 * lab + row_h - 7.0 * lab;
    for (i, r) in f.rows.iter().enumerate() {
        d.raw(&text_el(24.0, y, FS * lab, "start", ink, &short(&r.name)));
        d.raw(&text_el(col_o, y, FS * lab, "end", ink, &obs_text[i]));
        if ans {
            d.raw(&text_el(col_e, y, FS * lab, "end", ink, &exp_text[i]));
            d.raw(&text_el(col_t, y, FS * lab, "end", ink, &term_text[i]));
        } else {
            d.raw(&text_el(col_e, y, FS * lab, "end", ink, BLANK));
            d.raw(&text_el(col_t, y, FS * lab, "end", ink, BLANK));
        }
        y += row_h;
    }
    rule(&mut d, y - row_h + 9.0 * lab, 1.6);

    // the total row: the counts add up, and the statistic is the sum of the contributions
    d.raw(&text_el(24.0, y, FS * lab, "start", ink, "Total"));
    let obs_total_cell = total_text_obs.clone();
    d.raw(&text_el(col_o, y, FS * lab, "end", ink, &obs_total_cell));
    // (round 6 minor) the Expected total is the answer column's total: blank it with the cells it totals on the student
    // copy, or it hands back a number from a column the student is asked to fill. It equals the observed total and
    // prints the same way while it is small; at 1e7 and above it is a derived value in the Expected column, which goes
    // scientific like the cells above it (in full it ran into the observed total beside it).
    // (round 9) always the Observed total's string: the same number, printed the same way
    let expected_total = if !ans { BLANK.to_string() } else { obs_total_cell.clone() };
    d.raw(&text_el(col_e, y, FS * lab, "end", ink, &expected_total));
    // the total row: the counts add up, and the statistic is the sum of the contributions. (round 2) It must be the
    // number the line under the table prints, or the table's own total contradicts it: an impossible test has no
    // finite total at all
    let total_text = if !ans { BLANK.to_string() }
    else if infinite { "\u{221e}".to_string() }
    else if no_test { "\u{2014}".to_string() }
    else if term_sci || big(printed_chi2) { sci(printed_chi2) }
    else { group(&col(printed_chi2, term_dec)) };
    d.raw(&text_el(col_t, y, FS * lab, "end", ink, &total_text));
    rule(&mut d, y + 9.0 * lab, 1.6);

    for (i, line) in givens_lines.iter().rev().enumerate() {
        d.raw(&text_el(24.0, stats_y0 - givens_gap - i as f64 * 15.0 * lab, 12.0 * lab, "start", ink, line));
    }

    // the statistics block
    let lines = [
        if !ans { format!("Chi-square  \u{3c7}\u{b2} = {BLANK}") }
        else if f.impossible { "Chi-square  \u{3c7}\u{b2} = \u{221e}  (a category expected to be empty has counts)".to_string() }
        else if infinite { "Chi-square  \u{3c7}\u{b2} = \u{221e}  (the counts are too large)".to_string() }
        else if no_obs { "Chi-square  \u{3c7}\u{b2} = \u{2014}  (no observations to test)".to_string() }
        else if f.df == 0 { "Chi-square  \u{3c7}\u{b2} = \u{2014}  (no degrees of freedom)".to_string() }
        else if term_sci || big(printed_chi2) { format!("Chi-square  \u{3c7}\u{b2} = {}", sci(printed_chi2)) }
        else { format!("Chi-square  \u{3c7}\u{b2} = {}", group(&col(printed_chi2, term_dec))) },
        if ans {
            let (one, many) = ("1 category counted", "categories counted");
            let n = if f.kept == 1 { one.to_string() } else { format!("{} {many}", f.kept) };
            format!("Degrees of freedom  df = {}  ({n} \u{2212} 1)", f.df)
        } else {
            format!("Degrees of freedom  df = {BLANK}")
        },
        if ans { match f.p { Some(p) => format!("p-value  {}", p_phrase(p)), None => "p-value  p = \u{2014}".to_string() } } else { format!("p-value  p = {BLANK}") },
    ];
    let mut all: Vec<String> = lines.to_vec();
    all.extend(decision_lines.iter().cloned());
    for (i, line) in all.iter().enumerate() {
        let yy = stats_y0 + i as f64 * ROW_FS * lab * 1.35;
        d.raw(&format!(
            r##"<text x="24.00" y="{yy:.2}" font-size="{}" text-anchor="start" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{}">{}</text>"##,
            ROW_FS * lab,
            ink,
            svg::esc(line)
        ));
    }

    d.finish()
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    Some(vec![
        Example {
            name: "Mendel's 556 peas fit 9 : 3 : 3 : 1",
            params: vec![s("obs", "315, 101, 108, 32"), s("ratio", "9, 3, 3, 1")],
        },
        Example {
            name: "A monohybrid 3 : 1 cross",
            params: vec![s("names", "Round, Wrinkled"), s("obs", "787, 277"), s("ratio", "3, 1")],
        },
        Example {
            name: "A 1 : 1 testcross",
            params: vec![s("names", "Tall, Short"), s("obs", "52, 48"), s("ratio", "1, 1")],
        },
        Example {
            name: "A 1 : 2 : 1 cross that does not fit",
            params: vec![s("names", "Red, Pink, White"), s("obs", "45, 55, 20"), s("ratio", "1, 2, 1"), ("alpha", Value::F(0.05))],
        },
        Example {
            name: "Counts nowhere near a 3 : 1 ratio",
            params: vec![s("names", "Purple, Yellow"), s("obs", "300, 300"), s("ratio", "3, 1")],
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).unwrap() }
    fn with(list: &[(&'static str, Value)]) -> Params {
        let d = def();
        let mut p = Params::defaults(&(d.params)());
        for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
        p.normalize(&(d.params)(), false);
        p
    }
    fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }
    fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
    fn render_of(list: &[(&'static str, Value)]) -> String { (def().render)(&with(list)) }

    fn texts(svg: &str) -> String {
        let mut out = String::new();
        let mut rest = svg;
        while let Some(i) = rest.find("<text") {
            rest = &rest[i..];
            let Some(j) = rest.find('>') else { break };
            let body = &rest[j + 1..];
            let Some(k) = body.find("</text>") else { break };
            if !out.is_empty() { out.push('\n'); }
            out.push_str(&body[..k]);
            rest = &body[k..];
        }
        out
    }

    #[test]
    fn mendels_peas_give_the_textbook_statistic() {
        // 556 peas in 9 : 3 : 3 : 1 gives 312.75, 104.25, 104.25, 34.75 expected; chi-square = 0.4700
        let f = fit(&with(&[]));
        assert_eq!(f.rows.len(), 4);
        let e: Vec<f64> = f.rows.iter().map(|r| r.expected).collect();
        assert!((e[0] - 312.75).abs() < 1e-9, "{e:?}");
        assert!((e[1] - 104.25).abs() < 1e-9, "{e:?}");
        assert!((f.chi2 - 0.4700).abs() < 5e-4, "{}", f.chi2);
        assert_eq!(f.df, 3);
        assert!((f.p.unwrap() - 0.9254).abs() < 2e-3, "{:?}", f.p);
    }

    #[test]
    fn the_statistic_is_the_sum_of_the_contributions() {
        for (obs, ratio) in [("315, 101, 108, 32", "9, 3, 3, 1"), ("787, 277", "3, 1"), ("52, 48", "1, 1"), ("45, 55, 20", "1, 2, 1")] {
            let p = with(&[s("obs", obs), s("ratio", ratio)]);
            let f = fit(&p);
            let sum: f64 = f.rows.iter().map(|r| r.term).sum();
            assert!((sum - f.chi2).abs() < 1e-12, "{obs}: {sum} vs {}", f.chi2);
            // and the expected counts add up to the observations
            let e: f64 = f.rows.iter().map(|r| r.expected).sum();
            let o: f64 = f.rows.iter().map(|r| r.observed).sum();
            assert!((e - o).abs() < 1e-9, "{obs}: {e} vs {o}");
        }
    }

    #[test]
    fn a_perfect_fit_has_a_statistic_of_zero_and_a_p_of_one() {
        let p = with(&[s("obs", "30, 60, 30"), s("ratio", "1, 2, 1")]);
        let f = fit(&p);
        assert!(f.chi2.abs() < 1e-9, "{}", f.chi2);
        assert!((f.p.unwrap() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_wide_mismatch_is_rejected_and_a_close_one_is_not() {
        let bad = fit(&with(&[s("obs", "300, 300"), s("ratio", "3, 1")]));
        assert!(bad.p.unwrap() < 0.001, "{:?}", bad.p);
        assert!(decision(&bad).contains("reject the hypothesis"), "{}", decision(&bad));
        let good = fit(&with(&[]));
        assert!(decision(&good).contains("do not reject"), "{}", decision(&good));
    }

    #[test]
    fn a_ratio_part_of_zero_with_something_observed_cannot_be_tested() {
        let p = with(&[s("obs", "10, 5"), s("ratio", "1, 0")]);
        let f = fit(&p);
        assert!(f.p.is_none(), "{:?}", f.p);
        assert!(warnings(&p).iter().any(|w| w.contains("expected to be empty")), "{:?}", warnings(&p));
        // and an empty category nothing was counted in contributes nothing
        let ok = fit(&with(&[s("obs", "10, 0"), s("ratio", "1, 0")]));
        assert!((ok.chi2 - 0.0).abs() < 1e-9, "{}", ok.chi2);
    }

    #[test]
    fn a_category_the_ratio_cannot_produce_is_not_a_pass() {
        // an empty category with nothing counted in it carries no degree of freedom: one category left, so there is
        // no test — it used to print "df = 1" and "do not reject", a confident pass on a test that never ran
        let p = with(&[s("obs", "10, 0"), s("ratio", "1, 0")]);
        let f = fit(&p);
        assert_eq!(f.kept, 1, "only the category with an expected count counts");
        assert_eq!(f.df, 0);
        assert!(f.p.is_none(), "{:?}", f.p);
        assert!(decision(&f).contains("cannot be run"), "{}", decision(&f));
    }

    #[test]
    fn a_category_expected_to_be_empty_but_counted_makes_the_statistic_infinite() {
        // obs 5 where the ratio says none can occur: chi-square is infinite, not a finite 1.67
        let p = with(&[s("obs", "10, 5"), s("ratio", "1, 0")]);
        let f = fit(&p);
        assert!(f.impossible, "{f:?}");
        assert!(f.p.is_none());
        assert!(warnings(&p).iter().any(|w| w.contains("infinite")), "{:?}", warnings(&p));
        let t = texts(&render_of(&[s("obs", "10, 5"), s("ratio", "1, 0")]));
        assert!(t.contains("\u{221e}"), "the statistic is not shown as infinite: {t}");
        assert!(!t.contains("Chi-square  \u{3c7}\u{b2} = 1.67"), "{t}");
    }

    #[test]
    fn a_contribution_below_the_printed_precision_still_adds_up() {
        // (round 3) 1²/300 = 0.0033 and 1²/100 = 0.01: the first prints with four decimals (col's never-zero fallback)
        // while the total was rounded to two, so the column read 0.0033 + 0.01 against a total of 0.01
        let t = texts(&render_of(&[s("obs", "301, 99"), s("ratio", "3, 1")]));
        let line = t.lines().find(|l| l.contains("Chi-square  \u{3c7}\u{b2} =")).unwrap_or("");
        assert!(line.contains("= 0.0133"), "the printed total is not the sum of the printed contributions: {line}");
        assert_eq!(t.lines().filter(|l| *l == "0.0133").count(), 1, "the Total row disagrees with the line under the table: {t}");
    }

    /// Every drawn text as (x, y, size, anchor, content).
    fn boxes(svg: &str) -> Vec<(f64, f64, f64, String, String)> {
        let mut out = Vec::new();
        for part in svg.split("<text").skip(1) {
            let num = |k: &str| -> Option<f64> {
                let at = part.find(&format!("{k}=\""))? + k.len() + 2;
                part[at..].split('"').next()?.parse().ok()
            };
            let Some(x) = num("x") else { continue };
            let anchor = match part.find("text-anchor=\"") {
                Some(i) => part[i + 13..].split('"').next().unwrap_or("start").to_string(),
                None => "start".to_string(),
            };
            let content = part.split_once('>').map(|(_, rest)| rest.split("</text>").next().unwrap_or("").to_string()).unwrap_or_default();
            out.push((x, num("y").unwrap_or(0.0), num("font-size").unwrap_or(12.0), anchor, content));
        }
        out
    }

    #[test]
    fn no_text_runs_off_the_canvas() {
        // (round 5) the givens line was never wrapped, a wide grouped count printed under a truncated name, and a
        // contribution could be written in plain decimals across two columns
        let cases: Vec<Vec<(&'static str, Value)>> = vec![
            vec![s("names", "Round yellow seeds with a smooth coat on the tall parent plant, Other"), s("obs", "123456, 100"), s("ratio", "3, 1")],
            vec![s("obs", "100000,100000,100000,100000,100000,100000"), s("ratio", "100000,100000,100000,100000,100000,100000"), s("label_size", "Large")],
            vec![s("obs", "1e160, 1"), s("ratio", "3, 1")],
            vec![s("obs", "1, 1"), s("ratio", "1, 100000000000000000000")],
            vec![s("obs", "2, 3, 4, 5"), s("ratio", "1, 1, 1, 1")],
        ];
        for c in &cases {
            let svg = render_of(c);
            let w = {
                let at = svg.find("width=\"").expect("width") + 7;
                svg[at..].split('"').next().unwrap().parse::<f64>().unwrap()
            };
            for (x, _y, size, anchor, text) in boxes(&svg) {
                let text = text.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&").replace("&quot;", "\"");
                let n = text.chars().count() as f64 * size * 0.58;
                let right = match anchor.as_str() { "end" => x, "middle" => x + n / 2.0, _ => x + n };
                assert!(right <= w + 1.0, "{c:?}: {text:?} ends at {right:.1} on a {w} canvas");
            }
        }
        // an overflowing statistic writes ∞, not "inf"
        let t = texts(&render_of(&[s("obs", "1e160, 1"), s("ratio", "3, 1")]));
        assert!(!t.contains("inf"), "{t}");
        assert!(t.contains('\u{221e}'), "{t}");
        // a wide observed number no longer runs under the name, and the contributions are grouped like the counts
        let t = texts(&render_of(&[s("names", "Round yellow seeds with a smooth coat on the tall parent plant, Other"), s("obs", "123456, 100"), s("ratio", "3, 1")]));
        assert!(t.contains('\u{2026}'), "the long name is not shortened: {t}");
        assert!(t.contains("123,456"), "{t}");
        assert!(t.contains("10,229"), "the contribution is not grouped: {t}");
    }

    #[test]
    fn a_ratio_with_decimals_is_printed_as_typed() {
        // (round 6) "1.5 : 2.5 : 1.5" printed as "2 : 2 : 2", contradicting the expected counts beside it
        let t = texts(&render_of(&[s("obs", "100, 200, 100"), s("ratio", "1.5, 2.5, 1.5")]));
        assert!(t.contains("1.5 : 2.5 : 1.5"), "{t}");
        assert!(!t.contains("2 : 2 : 2"), "{t}");
        // whole numbers still print whole
        let t = texts(&render_of(&[s("obs", "315, 101, 108, 32"), s("ratio", "9, 3, 3, 1")]));
        assert!(t.contains("9 : 3 : 3 : 1"), "{t}");
    }

    #[test]
    fn an_impossible_category_prints_an_infinite_contribution_in_its_own_row() {
        // (round 4) the row printed 0.00 beside a Total of ∞
        let t = texts(&render_of(&[s("obs", "10, 5"), s("ratio", "1, 0")]));
        assert!(t.lines().filter(|l| *l == "\u{221e}").count() >= 2, "a row contribution is not infinite: {t}");
    }

    #[test]
    fn an_exact_fit_at_huge_counts_is_zero_not_floating_round_off() {
        // (round 4) counts exactly in 9:3:3:1 left 1e-13 crumbs that printed as χ² = 4.36e−13 on a perfect fit
        let t = texts(&render_of(&[s("obs", "1000000000000, 333333333333, 333333333333, 111111111111"), s("ratio", "9, 3, 3, 1")]));
        assert!(!t.contains("e\u{2212}13") && !t.contains("e-13"), "{t}");
        let line = t.lines().find(|l| l.contains("Chi-square  \u{3c7}")).unwrap_or("");
        assert!(line.contains("= 0"), "a perfect fit is not zero: {line}");
    }

    #[test]
    fn the_decision_never_contradicts_the_printed_p() {
        // p = 0.0503 prints as "0.050" at three decimals; the page then said "is below α = 0.05"
        let fit = Fit { rows: Vec::new(), chi2: 0.0, df: 1, p: Some(0.0503), alpha: 0.05, ratio: Vec::new(), kept: 2, impossible: false };
        let d = decision(&fit);
        assert!(!d.contains("is below"), "{d}");
        assert!(d.contains("level with"), "{d}");
        // and a clearly-below p still reads below
        let below = Fit { p: Some(0.001), ..fit.clone() };
        assert!(decision(&below).contains("is below"), "{}", decision(&below));
    }

    #[test]
    fn the_degrees_of_freedom_line_is_grammatical_and_counts_what_it_should() {
        let t = texts(&render_of(&[s("obs", "10, 0"), s("ratio", "1, 0")]));
        assert!(t.contains("(1 category counted \u{2212} 1)"), "{t}");
        let t = texts(&render_of(&[s("obs", "10, 5"), s("ratio", "1, 0")]));
        assert!(t.contains("(2 categories counted \u{2212} 1)"), "{t}");
    }

    #[test]
    fn the_expected_column_groups_its_thousands_like_the_observed_one() {
        // 31,500 beside 31275.00 is two formats in one row
        let t = texts(&render_of(&[s("obs", "31500, 10100, 10800, 3200"), s("ratio", "9, 3, 3, 1")]));
        assert!(t.contains("31,275.00"), "the expected count is not grouped: {t}");
        assert!(!t.contains("31275.00"), "{t}");
    }

    #[test]
    fn the_printed_contributions_add_up_to_the_printed_total() {
        // 2, 3, 4, 5 against 1:1:1:1: the terms print 0.64, 0.07, 0.07, 0.64, which add to 1.42, while the exact
        // statistic 1.4286 rounds to 1.43 — the printed total must be 1.42, or the column does not add up
        let t = texts(&render_of(&[s("obs", "2, 3, 4, 5"), s("ratio", "1, 1, 1, 1")]));
        let line = t.lines().find(|l| l.contains("Chi-square  \u{3c7}\u{b2} =")).unwrap_or("");
        assert!(line.contains("= 1.42"), "the total does not match its own column: {line}");
        // the table's own Total row must show that same number, not the exact 1.43
        assert_eq!(t.lines().filter(|l| *l == "1.42").count(), 1, "the Total row contradicts the line under the table: {t}");
        assert!(!t.lines().any(|l| l == "1.43"), "the exact statistic is printed beside the summed one: {t}");
        assert_eq!(t.lines().filter(|l| *l == "0.64").count(), 2, "{t}");
        assert_eq!(t.lines().filter(|l| *l == "0.07").count(), 2, "{t}");
    }

    #[test]
    fn a_long_name_and_huge_counts_do_not_overprint_the_columns() {
        let long = with(&[s("names", "Round yellow seeds with a smooth coat on the tall parent plant, Other")]);
        let t = texts(&render_of(&[s("names", "Round yellow seeds with a smooth coat on the tall parent plant, Other")]));
        assert!(t.contains("\u{2026}"), "a long name was not shortened: {t}");
        assert!(!t.contains("Round yellow seeds with a smooth coat on the tall parent plant"), "{t}");
        let _ = long;
        // huge counts: one format per column. (round 9 minor 5: this asserted the expected cells went scientific; the
        // rule is now plain and grouped up to 1e15 for a column and its total alike, so the Expected total no longer
        // reads "1.00e7" under "5,000,000.50" twice. What the check was for, that no column prints into the next, is
        // asserted here at every size, and across a sweep in `the_totals_and_the_givens_are_one_number_printed_one_way`.)
        for size in ["Small", "Medium", "Large"] {
            let svg = render_of(&[s("obs", "1000000000000, 333333333333, 333333333333, 111111111111"), s("ratio", "9, 3, 3, 1"), s("label_size", size)]);
            let t = texts(&svg);
            assert!(!t.contains("999999999999.56"), "an ungrouped long number slipped in: {t}");
            assert!(t.contains("999,999,999,999.56"), "the expected column is not one plain grouped format: {t}");
            let bs = boxes(&svg);
            let w: f64 = svg.split("width=\"").nth(1).and_then(|r| r.split('"').next()).and_then(|v| v.parse().ok()).unwrap();
            for y in bs.iter().map(|b| b.1).collect::<Vec<_>>() {
                let mut row: Vec<(f64, f64)> = bs.iter().filter(|b| (b.1 - y).abs() < 0.01).map(|b| {
                    let m = b.4.chars().count() as f64 * b.2 * 0.56;
                    if b.3 == "end" { (b.0 - m, b.0) } else { (b.0, b.0 + m) }
                }).collect();
                row.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                for p in row.windows(2) { assert!(p[0].1 + 4.0 <= p[1].0, "{size}: {p:?} collide: {t}"); }
                assert!(row.last().unwrap().1 <= w + 0.5, "{size}: a text runs off the canvas: {t}");
            }
        }
    }

    #[test]
    fn text_that_is_not_a_number_is_reported_and_empty_input_says_so() {
        let p = with(&[s("obs", "100, x, 50"), s("ratio", "3, 1")]);
        assert!(warnings(&p).iter().any(|w| w.contains("ignored")), "{:?}", warnings(&p));
        let t = texts(&render_of(&[s("obs", "")]));
        assert!(t.contains("Type the observed counts"), "{t}");
        assert!(!t.contains("df = 0 (0 categories"), "{t}");
    }

    #[test]
    fn a_mismatched_pair_of_lists_is_reported() {
        let p = with(&[s("obs", "10, 20, 30"), s("ratio", "1, 1")]);
        assert_eq!(fit(&p).rows.len(), 2);
        assert!(warnings(&p).iter().any(|w| w.contains("first 2 pairs")), "{:?}", warnings(&p));
        let empty = with(&[s("obs", "")]);
        assert!(fit(&empty).rows.is_empty());
        assert!(warnings(&empty).iter().any(|w| w.contains("no observed counts")), "{:?}", warnings(&empty));
    }

    #[test]
    fn the_answers_are_blank_on_the_student_copy() {
        let stu = texts(&render_of(&[b(SHOW_ANSWERS, false)]));
        assert!(stu.contains("Expected (E)") && stu.contains("Observed (O)"), "the table keeps its headings: {stu}");
        assert!(!stu.contains("312.75") && !stu.contains("0.47") && !stu.contains("0.93"), "an answer leaked: {stu}");
        // the observed counts and the ratio stay: they are the question
        assert!(stu.contains("315") && stu.contains("9 : 3 : 3 : 1"), "{stu}");
        let key = texts(&render_of(&[b(SHOW_ANSWERS, true)]));
        assert!(key.contains("312.75"), "{key}");
        assert!(key.contains("do not reject"), "{key}");
    }

    #[test]
    fn a_perfect_fit_never_prints_its_statistic_as_zero_decimals() {
        // three categories that fit exactly: every contribution is exactly 0, and the total prints "0"
        let t = texts(&render_of(&[s("obs", "30, 60, 30"), s("ratio", "1, 2, 1")]));
        assert!(t.contains("Chi-square  \u{3c7}\u{b2} = 0"), "{t}");
        // a contribution of 0.00025 is real and must not print as "0.00"
        let t2 = texts(&render_of(&[s("obs", "1000, 1000"), s("ratio", "1000, 1001")]));
        assert!(t2.contains("0.0002"), "a real contribution rounded away: {t2}");
    }

    #[test]
    fn a_test_with_no_observations_refuses_instead_of_passing() {
        // (round 6 blocker) obs 0,0 against 1,1 printed E = 0.00, χ² = 0.00, df = 1, p = 1.00 and "do not reject …
        // consistent with the ratio" — a confident pass on a test that ran on 0/0.
        for (obs, ratio) in [("0, 0", "1, 1"), ("0, 0, 0", "0, 1, 1")] {
            let p = with(&[s("obs", obs), s("ratio", ratio)]);
            let f = fit(&p);
            assert!(f.p.is_none(), "{obs} / {ratio}: a p-value on no observations: {:?}", f.p);
            assert!(warnings(&p).iter().any(|w| w.contains("Every observed count is zero")), "{obs}: {:?}", warnings(&p));
            let t = texts(&render_of(&[s("obs", obs), s("ratio", ratio)]));
            assert!(!t.contains("p = 1.00"), "a p-value on no observations: {t}");
            assert!(!t.contains("consistent with the ratio"), "a confident pass on no observations: {t}");
            assert!(t.contains("cannot be run"), "{t}");
            assert!(t.contains("no observations to test"), "{t}");
            // no contribution, statistic or total is invented out of 0/0
            assert!(!t.contains("Chi-square  \u{3c7}\u{b2} = 0"), "{t}");
        }
    }

    #[test]
    fn a_single_category_has_no_statistic_to_print() {
        // the decision already said "cannot be run" (df = 0) beside a χ² = 0.00 statistic
        let p = with(&[s("names", "Only"), s("obs", "10"), s("ratio", "1")]);
        assert!(fit(&p).p.is_none());
        let t = texts(&render_of(&[s("names", "Only"), s("obs", "10"), s("ratio", "1")]));
        assert!(!t.contains("Chi-square  \u{3c7}\u{b2} = 0.00"), "{t}");
        assert!(t.contains("no degrees of freedom"), "{t}");
        assert!(t.contains("cannot be run"), "{t}");
    }

    #[test]
    fn the_expected_total_is_blank_on_the_student_copy() {
        // (round 6 minor) the Expected cells were "_____" but the Expected Total row still read 556
        let stu = texts(&render_of(&[b(SHOW_ANSWERS, false)]));
        assert_eq!(stu.lines().filter(|l| *l == "556").count(), 1, "the Expected total still prints on the student copy: {stu}");
        // the observed total stays, because the observed column is the question
        assert_eq!(stu.lines().filter(|l| *l == "556").count(), 1);
        let key = texts(&render_of(&[b(SHOW_ANSWERS, true)]));
        assert_eq!(key.lines().filter(|l| *l == "556").count(), 2, "the key keeps both totals: {key}");
    }

    #[test]
    fn a_ratio_part_too_large_for_an_integer_is_printed_honestly() {
        // (round 6 minor) a typed 1 : 1e20 printed "1 : 9223372036854775807" (an i64 saturation) and the observed 1s
        // as "1.00e0"
        let p = with(&[s("names", "A, B"), s("obs", "1, 1"), s("ratio", "1, 100000000000000000000")]);
        assert!(fit(&p).p.is_some());
        let t = texts(&render_of(&[s("names", "A, B"), s("obs", "1, 1"), s("ratio", "1, 100000000000000000000")]));
        assert!(!t.contains("9223372036854775807"), "a saturated ratio part: {t}");
        assert!(t.contains("1.00e20"), "the ratio is not printed honestly: {t}");
        // (round 7 minor) the huge ratio made the whole table "huge", so the SMALL observed counts and their total
        // printed "1.00e0" / "2.00e0". Scientific notation is now used only for the values that need it.
        assert!(!t.contains("1.00e0"), "a small observed count is in scientific notation: {t}");
        assert!(!t.contains("2.00e0"), "the small observed total is in scientific notation: {t}");
        assert_eq!(t.lines().filter(|l| *l == "1").count(), 2, "the two observed 1s should print as counts: {t}");
        assert!(t.lines().any(|l| l == "2"), "the observed total of 2 should print as a count: {t}");
        assert!(t.contains("out of 2 observations"), "the given total is not a count: {t}");
        // the huge expected count stays scientific, while the small one beside it is a plain count
        assert!(t.contains("2.00e\u{2212}20"), "the tiny expected count lost its scientific form: {t}");
        assert!(t.contains("2.00\n") || t.lines().any(|l| l == "2.00"), "the small expected count is not a plain count: {t}");
        // ordinary whole and decimal ratios still print as typed
        assert!(texts(&render_of(&[s("obs", "315, 101, 108, 32"), s("ratio", "9, 3, 3, 1")])).contains("9 : 3 : 3 : 1"));
        assert!(texts(&render_of(&[s("obs", "100, 200, 100"), s("ratio", "1.5, 2.5, 1.5")])).contains("1.5 : 2.5 : 1.5"));
    }

    #[test]
    fn observed_counts_print_in_full_and_their_total_is_their_sum() {
        // (round 8 minor) an observed count is a given, but at or above 1e7 it printed with three significant figures:
        // the student saw "1.23e7" / "8.77e7" and could not recover the counts, and 5,000,000 + 5,000,001 totalled
        // "1.00e7". Whole counts now print in full up to 1e15, and the total is the sum of the printed cells.
        let parse = |t: &str| t.replace(',', "").parse::<f64>().ok();
        for (obs, ratio) in [
            ("12345678, 87654321", "1, 1"),
            ("5000000, 5000001", "1, 1"),
            ("9999999, 10000000", "1, 1"),
            ("123456789012345, 1", "1, 1"),
            ("999999999999999, 999999999999999, 3", "1, 1, 1"),
            ("315, 101, 108, 32", "9, 3, 3, 1"),
        ] {
            for ans in [true, false] {
                let svg = render_of(&[s("obs", obs), s("ratio", ratio), b(SHOW_ANSWERS, ans)]);
                let t = texts(&svg);
                let counts: Vec<f64> = obs.split(", ").map(|v| v.parse().unwrap()).collect();
                let bs = boxes(&svg);
                // the observed column: the cells right-aligned on the Observed column, below its header
                let col_x = bs.iter().find(|b| b.4 == "Observed (O)").map(|b| b.0).unwrap();
                let head_y = bs.iter().find(|b| b.4 == "Observed (O)").map(|b| b.1).unwrap();
                let cells: Vec<String> = bs.iter().filter(|b| (b.0 - col_x).abs() < 0.01 && b.1 > head_y).map(|b| b.4.clone()).collect();
                assert_eq!(cells.len(), counts.len() + 1, "{obs}: {cells:?}");
                for (c, want) in cells.iter().zip(&counts) {
                    assert_eq!(parse(c), Some(*want), "{obs} ({ans}): the count {want} prints as {c:?}");
                    assert!(!c.contains('e'), "{obs}: {c:?} is in scientific notation");
                }
                let total: f64 = counts.iter().sum();
                assert_eq!(parse(cells.last().unwrap()), Some(total), "{obs}: the total {:?} is not the sum of the cells", cells.last());
                assert!(t.contains(&format!("out of {} observations", cells.last().unwrap())), "{obs}: {t}");
                // the givens line names the observed total in full too (the Expected total, a derived value, may be scientific)
                assert!(!t.lines().any(|l| l.starts_with("Expected from") && l.contains('e') && l.contains("e7")), "{obs}: {t}");
            }
        }
        // (round 8) the Total row's cells never run into each other, at every size, however long the observed total
        for size in ["Small", "Medium", "Large"] {
            for obs in ["999999999999999, 999999999999999, 3", "12345678, 87654321", "315, 101, 108, 32"] {
                let svg = render_of(&[s("obs", obs), s("ratio", "1, 1, 1"), s("label_size", size)]);
                let bs = boxes(&svg);
                let ty = bs.iter().find(|b| b.4 == "Total").map(|b| b.1).unwrap();
                let mut row: Vec<(f64, f64)> = bs.iter().filter(|b| (b.1 - ty).abs() < 0.01).map(|b| {
                    let n = b.4.chars().count() as f64 * b.2 * 0.56;
                    if b.3 == "end" { (b.0 - n, b.0) } else { (b.0, b.0 + n) }
                }).collect();
                row.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                for w in row.windows(2) { assert!(w[0].1 + 4.0 <= w[1].0, "{size} {obs}: Total row cells {w:?} collide"); }
            }
        }
        // a count beyond 1e15 still goes scientific (it cannot print exactly), and so does its total
        let t = texts(&render_of(&[s("obs", "1e160, 1"), s("ratio", "3, 1")]));
        assert!(t.contains("1.00e160"), "{t}");
    }

    /// A printed plain number ("12,345,678.5") as integer units at `d` decimals, None for anything else.
    fn units_at(t: &str, d: usize) -> Option<i128> {
        let t = t.replace(',', "");
        if t.contains('e') || t.is_empty() { return None; }
        let (i, fr) = t.split_once('.').unwrap_or((&t, ""));
        if fr.len() > d { return None; }
        let digits = format!("{i}{fr}{}", "0".repeat(d - fr.len()));
        digits.parse::<i128>().ok()
    }

    /// (observed cells, observed total, expected cells, expected total, givens line, svg width) of a key render.
    fn columns(svg: &str) -> (Vec<String>, String, Vec<String>, String, String, f64) {
        let bs = boxes(svg);
        let (ox, oy) = bs.iter().find(|b| b.4 == "Observed (O)").map(|b| (b.0, b.1)).unwrap();
        let ex = bs.iter().find(|b| b.4 == "Expected (E)").map(|b| b.0).unwrap();
        let ty = bs.iter().find(|b| b.4 == "Total").map(|b| b.1).unwrap();
        let at = |x: f64, total: bool| -> Vec<String> { bs.iter().filter(|b| (b.0 - x).abs() < 0.01 && b.1 > oy && ((b.1 - ty).abs() < 0.01) == total).map(|b| b.4.clone()).collect() };
        let givens: String = texts(svg).lines().filter(|l| l.starts_with("Expected from") || l.contains("observations")).collect::<Vec<_>>().join(" ");
        let w: f64 = svg.split("width=\"").nth(1).and_then(|r| r.split('"').next()).and_then(|v| v.parse().ok()).unwrap();
        (at(ox, false), at(ox, true)[0].clone(), at(ex, false), at(ex, true)[0].clone(), givens, w)
    }

    #[test]
    fn the_totals_and_the_givens_are_one_number_printed_one_way() {
        // (round 9 minor) a fractional observed count broke the total: "5.5, 10.25" printed Total 15.75 but "out of 16
        // observations", and "12345678, 0.5" totalled "1.23e7". (minor) The Expected total went scientific ("1.00e7")
        // under plain cells and beside an Observed "10,000,001". Now the Observed total is the exact sum of the printed
        // cells, and the Expected total and the givens print that same string; a plain column stays plain.
        let mut cases: Vec<String> = vec![
            "5.5, 10.25".into(), "12345678, 0.5".into(), "5000000, 5000001".into(), "0.1, 0.2, 0.3".into(), "1.234567, 2".into(),
            "123456789012.345, 1.5".into(), "999999999999999, 999999999999999, 3".into(), "315, 101, 108, 32".into(), "0.000001, 3".into(),
        ];
        let mut rng = crate::figures::biology::Rng::new(9);
        for _ in 0..600 {
            let k = 2 + rng.below(4);
            let v: Vec<String> = (0..k).map(|_| {
                let mag = 10f64.powi(rng.below(15) as i32);
                let whole = (rng.below(1000) as f64 * mag / 100.0).floor().min(9.0e14);
                match rng.below(4) { 0 => format!("{whole}"), 1 => format!("{whole}.5"), 2 => format!("{}.{:02}", rng.below(1000), rng.below(100)), _ => format!("{}.{:03}", rng.below(100), rng.below(1000)) }
            }).collect();
            cases.push(v.join(", "));
        }
        let mut bad = Vec::new();
        for (n, obs) in cases.iter().enumerate() {
            let k = obs.split(", ").count();
            let ratio = vec!["1"; k].join(", ");
            let size = ["Small", "Medium", "Large"][n % 3];
            let svg = render_of(&[s("obs", obs), s("ratio", &ratio), s("label_size", size), b(SHOW_ANSWERS, true)]);
            let (oc, ot, ec, et, giv, w) = columns(&svg);
            let ctx = format!("{obs} ({size})");
            if !giv.contains(&format!("out of {ot} observations")) { bad.push(format!("{ctx}: the givens {giv:?} do not name the total {ot:?}")); }
            if et != ot { bad.push(format!("{ctx}: Expected total {et:?} vs Observed total {ot:?}")); }
            let plain = !oc.iter().any(|c| c.contains('e')) && !ot.contains('e');
            if plain {
                let d = oc.iter().chain(std::iter::once(&ot)).map(|c| c.split_once('.').map_or(0, |(_, f)| f.len())).max().unwrap();
                let sum: Option<i128> = oc.iter().map(|c| units_at(c, d)).sum();
                if sum != units_at(&ot, d) { bad.push(format!("{ctx}: cells {oc:?} do not sum to the total {ot:?}")); }
                // each cell is what was typed
                for (c, typed) in oc.iter().zip(obs.split(", ")) {
                    let tv: f64 = typed.parse().unwrap();
                    if (c.replace(',', "").parse::<f64>().unwrap() - tv).abs() > tv.abs() * 1e-15 + 1e-9 { bad.push(format!("{ctx}: {typed} printed as {c:?}")); }
                }
                // (round 11) an expected cell may be scientific in a plain column only when printing it in full at the
                // two decimals would need more than the 15 digits a double carries (at or above 1e13)
                if ec.iter().any(|c| c.contains('e') && !c.contains("e\u{2212}") && c.replace(',', "").parse::<f64>().map_or(true, |v| v < 1e13)) { bad.push(format!("{ctx}: a plain column's expected cells go scientific: {ec:?}")); }
            }
            // nothing on the Total row or in the table runs into its neighbour or off the canvas
            let bs = boxes(&svg);
            let ty = bs.iter().find(|b| b.4 == "Total").map(|b| b.1).unwrap();
            let rows: Vec<f64> = bs.iter().filter(|b| b.1 > ty - 400.0 && b.1 <= ty + 0.01 && b.0 > 100.0).map(|b| b.1).collect();
            for y in rows {
                let mut row: Vec<(f64, f64)> = bs.iter().filter(|b| (b.1 - y).abs() < 0.01).map(|b| {
                    let m = b.4.chars().count() as f64 * b.2 * 0.56;
                    if b.3 == "end" { (b.0 - m, b.0) } else { (b.0, b.0 + m) }
                }).collect();
                row.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                for p in row.windows(2) { if p[0].1 + 4.0 > p[1].0 { bad.push(format!("{ctx}: cells {p:?} collide on row {y}")); } }
                if row.last().map(|r| r.1 > w).unwrap_or(false) { bad.push(format!("{ctx}: a cell runs off the {w} px canvas")); }
            }
        }
        assert!(bad.is_empty(), "{} problems, e.g.:\n{}", bad.len(), bad.iter().take(12).cloned().collect::<Vec<_>>().join("\n"));
        // the reported cases, as printed
        let t = texts(&render_of(&[s("obs", "5.5, 10.25"), s("ratio", "1, 1"), b(SHOW_ANSWERS, true)]));
        assert!(t.contains("out of 15.75 observations") && t.lines().filter(|l| *l == "15.75").count() == 2, "{t}");
        let t = texts(&render_of(&[s("obs", "12345678, 0.5"), s("ratio", "1, 1"), b(SHOW_ANSWERS, true)]));
        assert!(t.contains("out of 12,345,678.5 observations") && t.lines().filter(|l| *l == "12,345,678.5").count() == 2, "{t}");
        let t = texts(&render_of(&[s("obs", "5000000, 5000001"), s("ratio", "1, 1"), b(SHOW_ANSWERS, true)]));
        assert_eq!(t.lines().filter(|l| *l == "10,000,001").count(), 2, "{t}");
        assert!(!t.contains("1.00e7"), "{t}");
        // and the contribution column follows one rule with its total: 6,172,838.25 twice summed to a Total of 1.23e7
        let svg = render_of(&[s("obs", "12345678, 0.5"), s("ratio", "1, 1"), b(SHOW_ANSWERS, true)]);
        let bs = boxes(&svg);
        let tx = bs.iter().find(|b| b.4.starts_with("(O")).map(|b| b.0).unwrap();
        let col_t: Vec<&String> = bs.iter().filter(|b| (b.0 - tx).abs() < 0.01 && !b.4.starts_with("(O")).map(|b| &b.4).collect();
        assert!(col_t.iter().all(|c| c.contains('e')) || col_t.iter().all(|c| !c.contains('e')), "{col_t:?}");
    }

    #[test]
    fn a_column_below_1e15_stays_plain_whatever_decimals_its_counts_carry() {
        // (round 10 minor) the column went scientific when a fractional count raised its scale past 9e15 units:
        // "1000000000000, 0.123456" printed "1.00e12" and "0.12". Each count is exact at its own decimals, and the
        // total is their exact sum in integers, so the column stays plain and the total is the sum of the printed cells.
        for (obs, cells, total) in [
            ("1000000000000, 0.123456", vec!["1,000,000,000,000", "0.123456"], "1,000,000,000,000.123456"),
            ("999999999999999, 1000001.5286", vec!["999,999,999,999,999", "1,000,001.5286"], "1,000,000,001,000,000.5286"),
            ("123456789012345, 0.000001, 7", vec!["123,456,789,012,345", "0.000001", "7"], "123,456,789,012,352.000001"),
        ] {
            for size in ["Small", "Medium", "Large"] {
                let k = obs.split(", ").count();
                let svg = render_of(&[s("obs", obs), s("ratio", &vec!["1"; k].join(", ")), s("label_size", size), b(SHOW_ANSWERS, true)]);
                let (oc, ot, _ec, et, giv, _) = columns(&svg);
                assert_eq!(oc, cells, "{obs} ({size})");
                assert_eq!(ot, total, "{obs} ({size})");
                assert_eq!(et, total, "{obs} ({size})");
                assert!(giv.contains(&format!("out of {total} observations")), "{obs} ({size}): {giv}");
            }
        }
        // and from 1e15 a column is still scientific as a whole
        let t = texts(&render_of(&[s("obs", "1000000000000000, 0.5"), s("ratio", "1, 1"), b(SHOW_ANSWERS, true)]));
        assert!(t.contains("1.00e15"), "{t}");
    }

    #[test]
    fn a_count_with_more_than_six_decimals_is_reported() {
        // (round 10 minor) "0.0000001" printed as 0 and "1.123456789" as 1.123457 without a word
        for obs in ["0.0000001, 5", "1.123456789, 2", "1e-7, 3"] {
            let w = warnings(&with(&[s("obs", obs), s("ratio", "1, 1")]));
            assert!(w.iter().any(|x| x.contains("six decimals")), "{obs}: {w:?}");
        }
        for obs in ["0.123456, 5", "315, 101", "1000000000000, 0.000001"] {
            let w = warnings(&with(&[s("obs", obs), s("ratio", "1, 1")]));
            assert!(!w.iter().any(|x| x.contains("six decimals")), "{obs}: {w:?}");
        }
    }

    #[test]
    fn counts_that_round_to_zero_are_no_observations() {
        // (round 11 blocker) "0.0000004, 0.0000004" printed Observed 0.000000 twice, "out of 0.000000 observations" and
        // Expected 4.00e−7 beside an Expected total of 0.000000, then p = 1.00 "do not reject"; "1e-14, 1e-14" the same
        // with no note at all. The counts are used at six decimals, so these are zero counts and the test is refused.
        for obs in ["0.0000004, 0.0000004", "1e-14, 1e-14", "0.00000000000001, 0.0000001", "4e-7, 0"] {
            for ans in [true, false] {
                let list = [s("obs", obs), s("ratio", "1, 1"), b(SHOW_ANSWERS, ans)];
                let p = with(&list);
                let f = fit(&p);
                assert!(f.p.is_none(), "{obs}: a p-value on counts that are all 0 at six decimals: {:?}", f.p);
                assert!(f.rows.iter().all(|r| r.observed == 0.0 && r.expected == 0.0), "{obs}: {:?}", f.rows);
                let w = warnings(&p);
                assert!(w.iter().any(|x| x.contains("Every observed count is zero")), "{obs}: {w:?}");
                assert!(w.iter().any(|x| x.contains("six decimals") && x.contains("used as 0")), "{obs}: {w:?}");
                let t = texts(&render_of(&list));
                assert!(!t.contains("p = 1.00") && !t.contains("consistent with the ratio"), "{obs}: a pass on no observations: {t}");
                assert!(!t.lines().any(|l| l.contains("e\u{2212}")), "{obs}: an unrounded count leaked: {t}");
                assert!(t.contains("out of 0 observations"), "{obs}: {t}");
                assert!(!t.contains("0.000000"), "{obs}: {t}");
                if ans {
                    assert!(t.contains("no observations to test") && t.contains("cannot be run"), "{obs}: {t}");
                }
            }
        }
        // the rounding is exact at the half: 0.0000005 is a count of 0.000001 and the test runs on it
        let p = with(&[s("obs", "0.0000005, 0.0000015"), s("ratio", "1, 1")]);
        let f = fit(&p);
        assert_eq!(f.rows.iter().map(|r| r.observed).collect::<Vec<_>>(), vec![0.000001, 0.000002]);
        assert!(f.p.is_some());
    }

    #[test]
    fn the_six_decimal_note_names_every_count_that_changes_and_what_it_becomes() {
        // (round 11 minors 2 and blocker B) the note missed 1e-14 (below the absolute tolerance) and 929059410.6733431
        // (inside the relative one); it now fires exactly when the typed number differs from its six-decimal rounding
        for (obs, typed, used) in [
            ("1e-14, 5", "1e-14", "0"),
            ("0.00000000000001, 5", "0.00000000000001", "0"),
            ("0.0000004, 0.0000004", "0.0000004", "0"),
            ("929059410.6733431, 45700113.464", "929059410.6733431", "929,059,410.673343"),
            ("1.123456789, 2", "1.123456789", "1.123457"),
            ("0.0000001, 5", "0.0000001", "0"),
            ("2.9999996, 1", "2.9999996", "3"),
            ("12345678901234.5000001, 1", "12345678901234.5000001", "12,345,678,901,234.5"),
            ("1e3, 2.5e-6", "2.5e-6", "0.000003"),
        ] {
            let w = warnings(&with(&[s("obs", obs), s("ratio", "1, 1")]));
            let note = w.iter().find(|x| x.contains("six decimals")).unwrap_or_else(|| panic!("{obs}: no note: {w:?}"));
            assert!(note.contains(&format!("\u{201c}{typed}\u{201d} is used as {used}")), "{obs}: {note}");
        }
        for obs in ["0.123456, 5", "315, 101", "1000000000000, 0.000001", "100.10000000, 3", "99999999999999.25, 0.000001, 7", "1e3, 2.5e-5"] {
            let w = warnings(&with(&[s("obs", obs), s("ratio", "1, 1")]));
            assert!(!w.iter().any(|x| x.contains("six decimals")), "{obs}: {w:?}");
        }
    }

    #[test]
    fn a_cell_that_needs_more_than_fifteen_digits_is_scientific_not_silently_cut() {
        // (round 11 minor 1) "15410925012.328436" (17 digits) printed "15,410,925,012.32844", and the expected half of
        // 1,000,000,001,000,000.5286 printed "…500,000.25" where the exact value is …500,000.2643: a double carries at
        // most 15 digits, so a cell that needs more is written in scientific notation, as before round 10
        let sci = |c: &str| c.contains('e');
        let svg = render_of(&[s("obs", "15410925012.328436, 556248838103.0769"), s("ratio", "4, 9"), ("dec", Value::I(2)), b(SHOW_ANSWERS, true)]);
        let (oc, ot, ec, et, giv, _) = columns(&svg);
        assert_eq!(oc, vec!["1.54e10", "5.56e11"], "{oc:?}");
        assert_eq!(ot, "5.72e11");
        assert_eq!(et, ot);
        assert!(giv.contains("out of 5.72e11 observations"), "{giv}");
        assert!(ec.iter().all(|c| sci(c)), "{ec:?}");
        let t = texts(&svg);
        assert!(!t.contains("15,410,925,012.32844") && !t.contains("571,659,763,115.40544"), "{t}");

        let svg = render_of(&[s("obs", "999999999999999, 1000001.5286"), s("ratio", "1, 1"), b(SHOW_ANSWERS, true)]);
        let (oc, ot, ec, et, _, _) = columns(&svg);
        assert_eq!(oc, vec!["999,999,999,999,999", "1,000,001.5286"]);
        assert_eq!(ot, "1,000,000,001,000,000.5286");
        assert_eq!(et, ot);
        assert_eq!(ec, vec!["5.00e14", "5.00e14"], "the expected cells need 17 digits: {ec:?}");
        let t = texts(&svg);
        assert!(!t.contains("500,000,000,500,000.25"), "{t}");
        for (ratio, want) in [("9, 3, 3, 1", vec!["5.63e14", "1.88e14", "1.88e14", "6.25e13"])] {
            let svg = render_of(&[s("obs", "999999999999999, 1000001.5286, 0, 0"), s("ratio", ratio), b(SHOW_ANSWERS, true)]);
            let (_, _, ec, _, _, _) = columns(&svg);
            assert_eq!(ec, want, "{ratio}");
        }
        // (round 11 minor 2) "99999999999999.25" needs 16 digits, so its column is scientific; the 0.000001 beside it is
        // a count used as typed (no note) and prints as one, not "0"
        let list = [s("obs", "99999999999999.25, 0.000001, 7"), s("ratio", "1, 2, 3"), ("dec", Value::I(3)), s("label_size", "Large"), s("style", "Print"), b(SHOW_ANSWERS, true)];
        let (oc, ot, _, et, giv, _) = columns(&render_of(&list));
        assert_eq!(oc, vec!["1.00e14", "0.000001", "7"], "{oc:?}");
        assert_eq!(ot, "1.00e14");
        assert_eq!(et, ot);
        assert!(giv.contains("out of 1.00e14 observations"), "{giv}");
        assert!(!warnings(&with(&list)).iter().any(|x| x.contains("six decimals")));
        // a 15-digit cell is still plain and exact, and its 15-digit neighbours too
        let (oc, ot, ec, _, _, _) = columns(&render_of(&[s("obs", "123456789.123456, 1.5"), s("ratio", "1, 1"), b(SHOW_ANSWERS, true)]));
        assert_eq!(oc, vec!["123,456,789.123456", "1.5"]);
        assert_eq!(ot, "123,456,790.623456");
        assert_eq!(ec, vec!["61,728,395.31", "61,728,395.31"]);
    }

    /// A typed count as an exact decimal (numerator, decimals), for the checks below; an independent parser.
    fn exact_dec(t: &str) -> (i128, u32) {
        let (m, e) = match t.split_once(['e', 'E']) { Some((m, e)) => (m, e.parse::<i32>().unwrap()), None => (t, 0) };
        let (i, f) = m.split_once('.').unwrap_or((m, ""));
        let mut num: i128 = format!("{i}{f}").parse().unwrap();
        let mut scale = f.len() as i32 - e;
        while scale < 0 { num *= 10; scale += 1; }
        (num, scale as u32)
    }

    /// The typed count rounded (half up) to six decimals, in millionths, and whether that changed it.
    fn six_units(t: &str) -> (i128, bool) {
        let (num, scale) = exact_dec(t);
        if scale <= 6 { return (num * 10i128.pow(6 - scale), false); }
        let p = 10i128.pow(scale - 6);
        let (q, r) = (num / p, num % p);
        (if 2 * r >= p { q + 1 } else { q }, r != 0)
    }

    /// A printed number: plain (grouped, any decimals) or "1.23e−7". (value, half a unit of its last printed digit)
    fn printed(c: &str) -> (f64, f64) {
        let c = c.replace(',', "").replace('\u{2212}', "-");
        match c.split_once('e') {
            Some((m, e)) => {
                let d = m.split_once('.').map_or(0, |(_, f)| f.len()) as i32;
                (c.parse().unwrap(), 0.5 * 10f64.powi(e.parse::<i32>().unwrap() - d))
            }
            None => (c.parse().unwrap(), 0.5 * 10f64.powi(-(c.split_once('.').map_or(0, |(_, f)| f.len()) as i32))),
        }
    }

    fn sig_digits(c: &str) -> usize { c.chars().filter(|c| c.is_ascii_digit()).skip_while(|c| *c == '0').count() }

    #[test]
    fn every_printed_number_derives_from_the_counts_at_six_decimals() {
        // (round 11 blocker) the page printed counts at six decimals but computed the expected counts, the statistic and
        // the decision from the raw typed values. Each count is now rounded once, and everything printed is checked here
        // against exact arithmetic on those rounded counts, over counts with 0-9+ decimals from 1e-14 to 1e16.
        let mut rng = crate::figures::biology::Rng::new(1111);
        let digit = |rng: &mut crate::figures::biology::Rng| char::from(b'0' + rng.below(10) as u8);
        let mut bad: Vec<String> = Vec::new();
        let mut kinds = [0usize; 4]; // refused, plain column, scientific column, a count changed by the rounding
        for case in 0..700 {
            let k = 2 + rng.below(4);
            let tiny_case = rng.below(10) == 0;
            let toks: Vec<String> = (0..k).map(|_| {
                let e = if tiny_case { -14 + rng.below(8) as i32 } else { rng.below(31) as i32 - 14 };
                match rng.below(8) {
                    0 => format!("{}e{}", 1 + rng.below(9), e),
                    1 => format!("{}.{}e{}", 1 + rng.below(9), rng.below(100), e - 1),
                    2 => ["0.0000005", "0.0000004999", "2.5000005", "0", "7", "0.000001", "999999999999999"][rng.below(7)].to_string(),
                    _ => {
                        let dd = rng.below(10);
                        if e >= 0 {
                            let mut s: String = std::iter::once(char::from(b'1' + rng.below(9) as u8)).chain((0..e).map(|_| digit(&mut rng))).collect();
                            if dd > 0 { s.push('.'); for _ in 0..dd { s.push(digit(&mut rng)); } }
                            s
                        } else {
                            let lead = (-e - 1) as usize;
                            let more = dd.saturating_sub(lead + 1);
                            format!("0.{}{}{}", "0".repeat(lead), char::from(b'1' + rng.below(9) as u8), (0..more).map(|_| digit(&mut rng)).collect::<String>())
                        }
                    }
                }
            }).collect();
            let ratio: Vec<i128> = (0..k).map(|_| 1 + rng.below(9) as i128).collect();
            let dec = 1 + rng.below(3) as i64;
            let obs = toks.join(", ");
            let list = [s("obs", &obs), s("ratio", &ratio.iter().map(|r| r.to_string()).collect::<Vec<_>>().join(", ")), ("dec", Value::I(dec)), s("label_size", ["Small", "Medium", "Large"][case % 3]), b(SHOW_ANSWERS, true)];
            let ctx = format!("{obs} : {ratio:?} dec {dec}");
            let units: Vec<(i128, bool)> = toks.iter().map(|t| six_units(t)).collect();
            let n: i128 = units.iter().map(|u| u.0).sum();
            let rt: i128 = ratio.iter().sum();
            let changed = units.iter().any(|u| u.1);
            if changed { kinds[3] += 1; }
            let w = warnings(&with(&list));
            if changed != w.iter().any(|x| x.contains("six decimals")) { bad.push(format!("{ctx}: the six-decimal note is {:?} but a count changed: {changed}", w)); }
            let svg = render_of(&list);
            let t = texts(&svg);
            let (oc, ot, ec, et, giv, _) = columns(&svg);
            if et != ot { bad.push(format!("{ctx}: Expected total {et:?} vs Observed total {ot:?}")); }
            if !giv.contains(&format!("out of {ot} observations")) { bad.push(format!("{ctx}: givens {giv:?} vs total {ot:?}")); }
            // the observed cells: exactly the rounded count when plain; scientific only in a column that holds a count
            // needing more than 15 digits, and then only from 1e7
            // the digits a count needs at its own decimals (trailing zero decimals dropped)
            let own = |u6: i128| { let mut x = u6; let mut k = 0; while k < 6 && x != 0 && x % 10 == 0 { x /= 10; k += 1; } x };
            let col_plain = units.iter().all(|u| own(u.0) < 10i128.pow(15));
            for (c, u) in oc.iter().zip(&units) {
                let v = u.0 as f64 / 1e6;
                if c.contains('e') {
                    if col_plain || v < 1e7 { bad.push(format!("{ctx}: {c:?} is scientific in a column that can print in full")); }
                    let (p, half) = printed(c);
                    if (p - v).abs() > half * (1.0 + 1e-9) { bad.push(format!("{ctx}: {c:?} is not {v}")); }
                } else if units_at(c, 6) != Some(u.0) {
                    bad.push(format!("{ctx}: the count {} (at six decimals) prints as {c:?}", u.0));
                }
            }
            if col_plain {
                kinds[1] += 1;
                if units_at(&ot, 6) != Some(n) { bad.push(format!("{ctx}: the total {ot:?} is not the sum {n} of the rounded counts")); }
            } else {
                kinds[2] += 1;
                let (p, half) = printed(&ot);
                if !ot.contains('e') || (p - n as f64 / 1e6).abs() > half * (1.0 + 1e-9) { bad.push(format!("{ctx}: the total {ot:?} is not {n}e-6")); }
            }
            if n == 0 {
                kinds[0] += 1;
                if fit(&with(&list)).p.is_some() || !t.contains("cannot be run") || !t.contains("no observations to test") || t.contains("p = ") && !t.contains("p = \u{2014}") {
                    bad.push(format!("{ctx}: a test ran on no observations: {t}"));
                }
                if !w.iter().any(|x| x.contains("Every observed count is zero")) { bad.push(format!("{ctx}: no zero-count refusal: {w:?}")); }
                continue;
            }
            // the expected cells: N·r/R from the rounded total, within half a unit of the last printed digit; plain only
            // while that takes at most 15 digits
            let mut chi2 = 0.0;
            for (i, c) in ec.iter().enumerate() {
                let e = (n * ratio[i]) as f64 / (rt as f64 * 1e6);
                let (p, half) = printed(c);
                if (p - e).abs() > half * (1.0 + 1e-9) + e.abs() * 2e-15 { bad.push(format!("{ctx}: expected {c:?} is not {e}")); }
                if !c.contains('e') && sig_digits(c) > 15 { bad.push(format!("{ctx}: expected {c:?} needs more digits than a double holds")); }
                let o = units[i].0 as f64 / 1e6;
                chi2 += (o - e) * (o - e) / e;
            }
            // the contributions, the statistic, df and p, from those same rounded counts
            let bs = boxes(&svg);
            let (tx, ty0) = bs.iter().find(|b| b.4.starts_with("(O")).map(|b| (b.0, b.1)).unwrap();
            let terms: Vec<String> = bs.iter().filter(|b| (b.0 - tx).abs() < 0.01 && b.1 > ty0).map(|b| b.4.clone()).collect();
            for (i, c) in terms.iter().take(k).enumerate() {
                let e = (n * ratio[i]) as f64 / (rt as f64 * 1e6);
                let o = units[i].0 as f64 / 1e6;
                let want = (o - e) * (o - e) / e;
                let (p, half) = printed(c);
                if (p - want).abs() > half * (1.0 + 1e-6) + want.abs() * 1e-7 + 1e-12 { bad.push(format!("{ctx}: contribution {c:?} is not {want}")); }
            }
            let df = k - 1;
            if !t.contains(&format!("df = {df}  ({k} categories counted")) { bad.push(format!("{ctx}: df is not {df}: {t}")); }
            let pv = 1.0 - chisq_cdf(chi2, df as f64);
            let pline = t.lines().find(|l| l.starts_with("p-value")).unwrap_or("").replace("&lt;", "<");
            if let Some(v) = pline.strip_prefix("p-value  p = ") {
                let (p, half) = printed(v);
                if (p - pv).abs() > half * (1.0 + 1e-6) + 1e-9 + pv * 1e-6 { bad.push(format!("{ctx}: {pline:?} but the rounded counts give p = {pv}")); }
            } else if pline == "p-value  p < 0.001" {
                if pv >= 0.001 + 1e-9 { bad.push(format!("{ctx}: {pline:?} but p = {pv}")); }
            } else {
                bad.push(format!("{ctx}: no p-value: {pline:?}"));
            }
        }
        assert!(bad.is_empty(), "{} problems, e.g.:\n{}", bad.len(), bad.iter().take(12).cloned().collect::<Vec<_>>().join("\n"));
        // the sweep reaches every kind of page
        assert!(kinds.iter().all(|&c| c >= 20), "{kinds:?}");
    }

    #[test]
    fn the_givens_line_has_a_real_gap_above_the_statistics() {
        // (round 8 minor) the givens line sat 14 px over the 15 px χ² line, its descenders a pixel from "Chi-square"
        for size in ["Small", "Medium", "Large"] {
            for ratio in ["9, 3, 3, 1", "123456789.25, 987654321.5, 3, 1"] {
                let svg = render_of(&[s("label_size", size), s("ratio", ratio)]);
                let bs = boxes(&svg);
                let chi = bs.iter().find(|b| b.4.starts_with("Chi-square  ")).unwrap();
                let giv: Vec<_> = bs.iter().filter(|b| b.1 < chi.1 && b.1 > chi.1 - 80.0 * svg::label_factor(size) && b.0 == 24.0 && b.4 != "Total").collect();
                assert!(!giv.is_empty(), "{size}: no givens line");
                let last = giv.iter().map(|b| b.1).fold(f64::MIN, f64::max);
                // the givens' descenders (0.25 em) to the χ² line's cap height (0.72 em): at least 4 px at every size
                let gap = (chi.1 - chi.2 * 0.72) - (last + 12.0 * svg::label_factor(size) * 0.25);
                assert!(gap >= 4.0, "{size} {ratio}: the givens line is {gap:.1} px from the \u{3c7}\u{b2} line");
                // and the first givens line clears the table's bottom rule
                let first = giv.iter().map(|b| b.1).fold(f64::MAX, f64::min);
                let rules: Vec<f64> = svg.split("<line ").skip(1).filter_map(|p| p.split("y1=\"").nth(1)?.split('"').next()?.parse().ok()).collect();
                let rule = rules.iter().cloned().fold(f64::MIN, f64::max);
                assert!(first - 12.0 * svg::label_factor(size) * 0.75 > rule + 2.0, "{size} {ratio}: the givens line {first} runs into the rule at {rule}");
            }
        }
    }

    #[test]
    fn print_is_neutral_grey_only() {
        let svg = render_of(&[s("style", "Print")]);
        for line in svg.lines().filter(|l| l.contains("stroke=\"#") || l.contains("fill=\"#")) {
            for part in line.split('"').filter(|x| x.starts_with('#') && x.len() == 7 && x[1..].chars().all(|c| c.is_ascii_hexdigit())) {
                let (r, g, bl) = (
                    u8::from_str_radix(&part[1..3], 16).unwrap(),
                    u8::from_str_radix(&part[3..5], 16).unwrap(),
                    u8::from_str_radix(&part[5..7], 16).unwrap(),
                );
                assert!(r == g && g == bl, "Print draws a hue: {part} in {line}");
            }
        }
    }

    /// Writes every case (Colour, Print, student, key) to $VERSHION_PNG_DIR.
    #[test]
    fn chisquare_fit_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let s2 = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![
            ("default".into(), vec![]),
            ("large".into(), vec![s2("label_size", "Large")]),
            ("two".into(), vec![s2("names", "Round, Wrinkled"), s2("obs", "787, 277"), s2("ratio", "3, 1")]),
            ("six".into(), vec![s2("names", "A, B, C, D, E, F"), s2("obs", "10, 20, 30, 40, 50, 60"), s2("ratio", "1, 1, 1, 1, 1, 1")]),
            ("big".into(), vec![s2("obs", "31500, 10100, 10800, 3200"), s2("ratio", "9, 3, 3, 1")]),
        ];
        for (idx, (_, l)) in examples(ID).unwrap().into_iter().map(|e| (e.name.to_string(), e.params)).enumerate() { cases.push((format!("ex{idx}"), l)); }
        for (tag, list) in cases {
            for (kind, extra) in [("colour", vec![]), ("print", vec![s2("style", "Print")]), ("student", vec![b(SHOW_ANSWERS, false)]), ("key", vec![b(SHOW_ANSWERS, true)])] {
                let mut l = list.clone();
                l.extend(extra);
                let svg = render_of(&l);
                std::fs::write(format!("{dir}/chifit-{tag}-{kind}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
                if kind == "print" {
                    let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
                    let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                    image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/chifit-{tag}-print-bw66.png")).unwrap();
                }
            }
        }
    }
}
