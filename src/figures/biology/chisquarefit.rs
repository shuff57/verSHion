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
    let obs = numbers(p.s("obs"));
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
    let obs = numbers(p.s("obs"));
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
    if obs.is_empty() {
        out.push("There are no observed counts to test.".to_string());
    }
    // (round 6 blocker) an all-zero observation column is not a fit: nothing has been counted, so there is nothing to
    // test and no statistic to compute. Without this the editor said nothing and the page reported "p = 1.00 … do not
    // reject" on a 0/0.
    if !obs.is_empty() && obs.iter().all(|v| *v == 0.0) {
        out.push("Every observed count is zero, so there is nothing to test and the statistic cannot be computed.".to_string());
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
    let stats_y0 = table_bottom + 46.0 * lab;
    let w = (W * (1.0 + 0.5 * (lab - 1.0))).max(W);
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
    let total_obs: f64 = f.rows.iter().map(|r| r.observed).sum();
    // (round 6 blocker) nothing observed: every (O − E)² / E is 0/0, so there is no contribution, statistic or test.
    let no_obs = total_obs == 0.0;
    // a test that cannot run has no statistic to print: zero observations (0/0) or a single category (no degrees of
    // freedom) leave χ² undefined rather than a meaningful 0.00
    let no_test = no_obs || f.df == 0;
    // (round 7 minor) scientific notation belongs only to the values that need it. A huge typed ratio (1 : 1e20) made
    // the entire table "huge", so the small observed counts printed "1.00e0" and the total "2.00e0". A value is
    // scientific only when it is itself ≥ 1e7; small counts stay counts, while the huge ratio/expected stay scientific.
    let big = |v: f64| v.is_finite() && v.abs() >= 1e7;
    // (round 5) an overflowing value prints "inf"; the rest of the figure writes ∞
    let sci = |v: f64| if v == 0.0 { "0".to_string() } else if !v.is_finite() { "\u{221e}".to_string() } else { format!("{v:.2e}").replace('-', "\u{2212}") };
    let fmt_cnt = |v: f64| if big(v) || !v.is_finite() { sci(v) } else { num(v, 2) };
    // (round 5) the contributions are grouped like the counts (1,234.56), so one row does not mix two formats
    let fmt_term = |v: f64| if big(v) { sci(v) } else if !v.is_finite() { "\u{221e}".to_string() } else { group(&col(v, dec.max(2))) };
    // an expected count only goes scientific when it is itself huge (a tiny one already falls back through `col`)
    let fmt_exp = |v: f64| if big(v) || !v.is_finite() { sci(v) } else { group(&col(v, dec)) };
    // a long category name would print through the three number columns: the room it has is what is left of the
    // Observed column once its widest printed number is drawn (round 5: a grouped count ran under the name)
    let obs_text: Vec<String> = f.rows.iter().map(|r| fmt_cnt(r.observed)).collect();
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
            d.raw(&text_el(col_e, y, FS * lab, "end", ink, &fmt_exp(r.expected)));
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
    d.raw(&text_el(col_o, y, FS * lab, "end", ink, &fmt_cnt(total_obs)));
    // (round 6 minor) the Expected total is the answer column's total: blank it with the cells it totals on the student
    // copy, or it hands back a number from a column the student is asked to fill
    let expected_total = if ans { fmt_cnt(total_obs) } else { BLANK.to_string() };
    d.raw(&text_el(col_e, y, FS * lab, "end", ink, &expected_total));
    // the total row: the counts add up, and the statistic is the sum of the contributions. (round 2) It must be the
    // number the line under the table prints, or the table's own total contradicts it: an impossible test has no
    // finite total at all
    let total_text = if !ans { BLANK.to_string() }
    else if infinite { "\u{221e}".to_string() }
    else if no_test { "\u{2014}".to_string() }
    else if big(printed_chi2) { sci(printed_chi2) }
    else { group(&col(printed_chi2, term_dec)) };
    d.raw(&text_el(col_t, y, FS * lab, "end", ink, &total_text));
    rule(&mut d, y + 9.0 * lab, 1.6);

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
    let total_shown = if big(total_obs) { sci(total_obs) } else { val(total_obs, 0) };
    let givens = format!("Expected from the ratio {} out of {} observations", ratio_text.join(" : "), total_shown);
    // (round 5) long ratio parts ran off the canvas: wrapped to the width, the lines stacking up into the gap above
    // the statistics block
    let givens_chars = (((w - 48.0) / (12.0 * lab * 0.56)).floor() as usize).max(20);
    for (i, line) in wrap(&givens, givens_chars).iter().rev().enumerate() {
        d.raw(&text_el(24.0, stats_y0 - 14.0 * lab - i as f64 * 15.0 * lab, 12.0 * lab, "start", ink, line));
    }

    // the statistics block
    let lines = [
        if !ans { format!("Chi-square  \u{3c7}\u{b2} = {BLANK}") }
        else if f.impossible { "Chi-square  \u{3c7}\u{b2} = \u{221e}  (a category expected to be empty has counts)".to_string() }
        else if infinite { "Chi-square  \u{3c7}\u{b2} = \u{221e}  (the counts are too large)".to_string() }
        else if no_obs { "Chi-square  \u{3c7}\u{b2} = \u{2014}  (no observations to test)".to_string() }
        else if f.df == 0 { "Chi-square  \u{3c7}\u{b2} = \u{2014}  (no degrees of freedom)".to_string() }
        else if big(printed_chi2) { format!("Chi-square  \u{3c7}\u{b2} = {}", sci(printed_chi2)) }
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
        // huge counts: one compact format, so no column is wider than the others
        let t = texts(&render_of(&[s("obs", "1000000000000, 333333333333, 333333333333, 111111111111"), s("ratio", "9, 3, 3, 1")]));
        assert!(!t.contains("999999999999.56"), "a plain long number slipped in: {t}");
        assert!(t.contains("e12") || t.contains("e11") || t.contains("\u{d7}"), "{t}");
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
