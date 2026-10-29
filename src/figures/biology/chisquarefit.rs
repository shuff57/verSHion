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
}

fn clamp_alpha(v: f64) -> f64 { if v.is_finite() { v.clamp(0.01, 0.10) } else { 0.05 } }

/// The whole test, computed from the typed text.
pub fn fit(p: &Params) -> Fit {
    let alpha = clamp_alpha(p.f("alpha"));
    let obs = numbers(p.s("obs"));
    let ratio = numbers(p.s("ratio"));
    let names = split_list(p.s("names"));
    let k = obs.len().min(MAX_CATS).min(ratio.len()).max(0);
    let total_obs: f64 = obs.iter().take(k).sum();
    let total_ratio: f64 = ratio.iter().take(k).sum();
    let mut rows = Vec::with_capacity(k);
    let mut chi2 = 0.0;
    let mut usable = k > 0 && total_ratio > 0.0;
    for i in 0..k {
        let o = obs[i];
        let e = if total_ratio > 0.0 { ratio[i] / total_ratio * total_obs } else { 0.0 };
        let term = if e > 0.0 {
            (o - e) * (o - e) / e
        } else if o > 0.0 {
            // an expected count of zero with something observed in it: the statistic is infinite, so the test
            // cannot be run at all rather than reporting a made-up number
            usable = false;
            0.0
        } else {
            0.0
        };
        chi2 += term;
        let name = names.get(i).cloned().unwrap_or_else(|| format!("Category {}", i + 1));
        rows.push(Row { name, observed: o, expected: e, term });
    }
    let df = k.saturating_sub(1);
    let p = if usable && df > 0 && chi2.is_finite() { Some((1.0 - chisq_cdf(chi2, df as f64)).clamp(0.0, 1.0)) } else { None };
    Fit { rows, chi2, df, p, alpha, ratio: ratio.iter().take(k).cloned().collect() }
}

pub fn warnings(p: &Params) -> Vec<String> {
    let mut out = Vec::new();
    let obs = numbers(p.s("obs"));
    let ratio = numbers(p.s("ratio"));
    if obs.is_empty() {
        out.push("There are no observed counts to test.".to_string());
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
    if f.df == 0 && !f.rows.is_empty() {
        out.push("One category leaves no degrees of freedom, so there is nothing to test.".to_string());
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

/// The decision in words, from the p-value and the significance level.
pub fn decision(f: &Fit) -> String {
    match f.p {
        None => "The test cannot be run on these numbers".to_string(),
        Some(p) => {
            let a = num(f.alpha, 3);
            if p < f.alpha {
                format!("{} is below \u{3b1} = {}, so reject the hypothesis: the counts do not fit the ratio.", p_phrase(p), a)
            } else {
                format!("{} is above \u{3b1} = {}, so do not reject the hypothesis: the counts are consistent with the ratio.", p_phrase(p), a)
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
    let decision_lines = if ans { wrap(&decision_text, 70) } else { vec![decision_text.clone()] };
    let mut d = Doc::new(w, stats_y0 + (3.0 + decision_lines.len() as f64) * ROW_FS * lab * 1.35 + 12.0);
    d.describe("Chi-square goodness of fit", "Observed counts beside the counts a ratio predicts, with the statistic, degrees of freedom and p-value");

    d.raw(&format!(
        r##"<text x="24.00" y="{:.2}" font-size="{}" text-anchor="start" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{}">Chi-square goodness of fit</text>"##,
        30.0 * lab,
        17.0 * lab,
        ink
    ));

    // the header
    let head = FS * lab;
    d.raw(&text_el(24.0, head_y, head, "start", ink, "Category"));
    d.raw(&text_el(col_o, head_y, head, "end", ink, "Observed (O)"));
    d.raw(&text_el(col_e, head_y, head, "end", ink, "Expected (E)"));
    d.raw(&text_el(col_t, head_y, head, "end", ink, "(O \u{2212} E)\u{b2} / E"));
    let rule = |d: &mut Doc, y: f64, sw: f64| { d.line(20.0, y, w - 20.0, y, ink, sw); };
    rule(&mut d, head_y + 8.0 * lab, 1.6);

    // the rows
    let mut y = head_y + 8.0 * lab + row_h - 7.0 * lab;
    for r in &f.rows {
        d.raw(&text_el(24.0, y, FS * lab, "start", ink, &r.name));
        d.raw(&text_el(col_o, y, FS * lab, "end", ink, &val(r.observed, 2)));
        if ans {
            d.raw(&text_el(col_e, y, FS * lab, "end", ink, &col(r.expected, dec)));
            d.raw(&text_el(col_t, y, FS * lab, "end", ink, &col(r.term, dec.max(2))));
        } else {
            d.raw(&text_el(col_e, y, FS * lab, "end", ink, BLANK));
            d.raw(&text_el(col_t, y, FS * lab, "end", ink, BLANK));
        }
        y += row_h;
    }
    rule(&mut d, y - row_h + 9.0 * lab, 1.6);

    // the total row: the counts add up, and the statistic is the sum of the contributions
    let total_obs: f64 = f.rows.iter().map(|r| r.observed).sum();
    d.raw(&text_el(24.0, y, FS * lab, "start", ink, "Total"));
    d.raw(&text_el(col_o, y, FS * lab, "end", ink, &num(total_obs, 2)));
    d.raw(&text_el(col_e, y, FS * lab, "end", ink, &num(total_obs, 2)));
    let total_text = if ans { col(f.chi2, dec.max(2)) } else { BLANK.to_string() };
    d.raw(&text_el(col_t, y, FS * lab, "end", ink, &total_text));
    rule(&mut d, y + 9.0 * lab, 1.6);

    // the givens: the ratio the expected counts come from, always printed
    let ratio_text: Vec<String> = f.ratio.iter().map(|v| val(*v, 0)).collect();
    let givens = format!("Expected from the ratio {} out of {} observations", ratio_text.join(" : "), val(total_obs, 0));
    d.raw(&text_el(24.0, stats_y0 - 14.0 * lab, 12.0 * lab, "start", ink, &givens));

    // the statistics block
    let lines = [
        if ans { format!("Chi-square  \u{3c7}\u{b2} = {}", col(f.chi2, dec.max(2))) } else { format!("Chi-square  \u{3c7}\u{b2} = {BLANK}") },
        if ans { format!("Degrees of freedom  df = {}  ({} categories \u{2212} 1)", f.df, n) } else { format!("Degrees of freedom  df = {BLANK}") },
        if ans { match f.p { Some(p) => format!("p-value  {}", p_phrase(p)), None => "p-value  p = \u{2014}".to_string() } } else { format!("p-value  p = {BLANK}") },
    ];
    let mut all: Vec<String> = lines.to_vec();
    let first = if ans { 0.0 } else { 0.0 };
    let _ = first;
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
