//! Hardy-Weinberg: a gene pool divided between two alleles, and below it the genotype frequencies that random
//! mating produces from that pool in each generation. With no selection the bars are identical down the page — that
//! is the equilibrium. Selection against the recessive phenotype is the counter-example: the bars shift, because the
//! allele frequency changes. Drawn with `svg::Doc` only.
//!
//! The allele frequency p is given, so the gene pool is always drawn. The genotype frequencies are the answer, so no
//! genotype bar and no count is drawn while Show answers is off; the generation rows, their names and the blanks stay.
use super::super::circle::num;
use crate::figures::interact::Handle;
use super::*;

pub const ID: &str = "hardy-weinberg";

const INK: &str = "#111111";
const DOM: &str = "#1f5fbf";
const HET: &str = "#7a4fc4";
const REC: &str = "#c47a1f";
const BLANK: &str = "_____";
const W: f64 = 640.0;
/// Left edge and width of every bar: the gene pool, each generation, and the drag handle's inverse.
const BAR_X: f64 = 148.0;
const BAR_W: f64 = 452.0;
const POOL_Y: f64 = 50.0;
const POOL_H: f64 = 32.0;
const GEN_Y0: f64 = 134.0;
const GEN_H: f64 = 32.0;
const GEN_GAP: f64 = 24.0;
const ROW_H: f64 = 18.0;
const ROW_FS: f64 = 15.0;

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Biology,
        title: "Hardy\u{2013}Weinberg",
        blurb: "A gene pool split between two alleles and the genotype frequencies random mating makes from it, held steady across generations — or shifted by selection.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("p", "Frequency of the dominant allele (p)", 0.01, 0.99, 0.01, 0.6),
        ParamDef::int("n", "Population size", 10, 10000, 1000),
        ParamDef::int("gens", "Generations shown", 2, 6, 3),
        ParamDef::choice("sel", "Selection", &["none (equilibrium)", "against the recessive phenotype"], "none (equilibrium)"),
        ParamDef::bool("counts", "Show counts as well as fractions", false),
        ParamDef::int("dec", "Decimal places", 1, 3, 2),
        answers_param(),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p).0 }

fn fin(v: f64, lo: f64, hi: f64, d: f64) -> f64 { if v.is_finite() { v.clamp(lo, hi) } else { d } }

/// The inputs, clamped exactly as the drawing computes from them: (p, population size, generations, selection, counts).
pub fn given(p: &Params) -> (f64, usize, usize, bool, bool) {
    (
        fin(p.f("p"), 0.01, 0.99, 0.6),
        p.i("n").clamp(10, 10000) as usize,
        p.i("gens").clamp(2, 6) as usize,
        p.s("sel").starts_with("against"),
        p.b("counts"),
    )
}

/// One generation: the allele frequency that produced it and its three genotype frequencies, which sum to 1.
#[derive(Clone, Copy, Debug)]
pub struct Gen {
    pub p: f64,
    pub dom: f64,
    pub het: f64,
    pub rec: f64,
}

/// The generations shown. Under selection against the recessive phenotype the survivors are AA and Aa, so the next
/// generation's allele frequency is p' = p / (1 - q²) — the recessive allele is lost faster than the recessive
/// phenotype, because it hides in heterozygotes.
pub fn generations(p: &Params) -> Vec<Gen> {
    let (p0, _, gens, sel, _) = given(p);
    let mut pa = p0;
    let mut out = Vec::with_capacity(gens);
    for _ in 0..gens {
        let q = 1.0 - pa;
        out.push(Gen { p: pa, dom: pa * pa, het: 2.0 * pa * q, rec: q * q });
        if sel { pa /= 1.0 - q * q; }
    }
    out
}

/// The three frequencies as they are printed: rounded by largest remainder, so the printed numbers add up to exactly 1
/// and each is the nearest it can be to its true value. Rounding each on its own printed "0.4 + 0.5 + 0.2 = 1.1" beside a
/// Check line that said 1; (round 5) moving the whole difference into the largest part printed a 1:2:1 cross as
/// 0.3 / 0.4 / 0.3, a full unit low on the heterozygote against a bar that was plainly half.
pub fn printed_parts(g: &Gen, dec: i64) -> [f64; 3] {
    let f = 10f64.powi(dec as i32);
    let exact = [g.dom * f, g.het * f, g.rec * f];
    let mut units: [i64; 3] = exact.map(|v| v.floor() as i64);
    let mut left = f.round() as i64 - units.iter().sum::<i64>();
    let rem = |i: usize| exact[i] - exact[i].floor();
    let mut order = [0usize, 1, 2];
    order.sort_by(|a, b| rem(*b).partial_cmp(&rem(*a)).unwrap_or(std::cmp::Ordering::Equal));
    let mut k = 0usize;
    while left > 0 {
        units[order[k % 3]] += 1;
        left -= 1;
        k += 1;
    }
    while left < 0 {
        units[order[2 - k % 3]] -= 1;
        left += 1;
        k += 1;
    }
    units.map(|u| u as f64 / f)
}

/// The decimal places the picture is printed at: `dec`, raised until no real frequency would print as zero — and, since
/// round 3, until the allele frequencies themselves print honestly: p = 0.99 at one decimal place printed "1" beside a
/// q of "0.01" and a genotype set that plainly does not come from p = 1. (round 6) With counts shown it is raised again
/// until each printed fraction, times N, rounds to the count printed beside it: p = 0.34 printed "AA p² = 0.12
/// (116 of 1000)", and 0.12 × 1000 is 120, not 116. Raising the precision to three makes 0.116 × 1000 = 116.
pub fn shown_dec(p: &Params) -> i64 {
    let g = generations(p);
    let dec = p.i("dec").clamp(1, 3);
    let (p0, n, _, _, show_counts) = given(p);
    let q0 = 1.0 - p0;
    let collapses = |v: f64, d: i64| {
        let shown = num(v, d).replace('\u{2212}', "-").parse::<f64>().unwrap_or(v);
        (v < 1.0 && shown >= 1.0) || (v > 0.0 && shown <= 0.0)
    };
    let needs_more = |gg: &Gen, d: i64| {
        let pr = printed_parts(gg, d);
        [gg.dom, gg.het, gg.rec].iter().zip(pr.iter()).any(|(ex, sh)| *ex != 0.0 && *sh == 0.0)
    };
    // the printed fraction is what a reader multiplies by N, so it must give back the count on the same line: the
    // count is a nearest rounding of the exact frequency × N, the fraction a nearest rounding at some precision, and
    // the two only agree when that precision is fine enough (round 6 blocker: "0.43 (435 of 1000)", but 435/1000
    // rounds to 0.44)
    let agrees = |gg: &Gen, d: i64| {
        let pr = printed_parts(gg, d);
        let c = counts([gg.dom, gg.het, gg.rec], n);
        (0..3).all(|i| (pr[i] * n as f64).round() == c[i] as f64)
    };
    // The precision at which the page is honest: no real part prints as zero, the allele frequencies do not collapse,
    // and — when counts are shown — each fraction times N gives the count beside it. Prefer the typed precision, then
    // finer ones (the reported cases need three or four decimals); only a population too small to hold the fraction
    // at the typed precision (n = 10 cannot hold 0.25) makes a coarser one the honest choice.
    let fits = |d: i64| {
        !needs_more(&g[0], d)
            && !collapses(p0, d)
            && !collapses(q0, d)
            && (!show_counts || agrees(&g[0], d))
    };
    for d in (dec..=6).chain((1..dec).rev()) {
        if fits(d) { return d; }
    }
    // nothing fits (a pathological tie): keep the never-zero precision, the pre-round-6 behaviour
    let mut dp = dec;
    while dp < 6 && (needs_more(&g[0], dp) || collapses(p0, dp) || collapses(q0, dp)) { dp += 1; }
    dp
}

/// Counts that sum to `n` exactly: the whole parts, then the leftovers in order of largest remainder (rounding each
/// frequency on its own could print three counts that add up to 999 or 1001).
pub fn counts(f: [f64; 3], n: usize) -> [usize; 3] {
    let raw = [f[0] * n as f64, f[1] * n as f64, f[2] * n as f64];
    let mut out = [raw[0].floor() as usize, raw[1].floor() as usize, raw[2].floor() as usize];
    let mut left = n.saturating_sub(out.iter().sum::<usize>());
    let mut order = [0usize, 1, 2];
    order.sort_by(|a, b| {
        let ra = raw[*a] - raw[*a].floor();
        let rb = raw[*b] - raw[*b].floor();
        rb.partial_cmp(&ra).unwrap_or(std::cmp::Ordering::Equal)
    });
    for i in order {
        if left == 0 { break; }
        out[i] += 1;
        left -= 1;
    }
    out
}

pub fn warnings(p: &Params) -> Vec<String> {
    let (_, _, gens, sel, _) = given(p);
    if !sel { return Vec::new(); }
    let g = generations(p);
    let last = g.last().map(|x| x.p).unwrap_or(0.0);
    let q = 1.0 - last;
    let next = last / (1.0 - q * q);
    // the editor note is read on its own, with no page to raise its precision: take the page's
    let d = shown_dec(p);
    vec![format!(
        "Selection removes every recessive homozygote, so the population is not in Hardy\u{2013}Weinberg equilibrium: p rises from {} to {} over the {} generations shown, and to {} in the next.",
        val(g[0].p, d),
        val(last, d),
        gens,
        val(next, d)
    )]
}

fn text_el(x: f64, y: f64, size: f64, anchor: &str, fill: &str, s: &str) -> String {
    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{fill}">{}</text>"##,
        svg::esc(s)
    )
}

/// A frequency that never rounds a real value to "0": two more decimals first, then exponential form. p = 0.01
/// makes p\u{b2} = 0.0001, which at one or two decimals printed as "0" beside a segment that was really there.
fn val(v: f64, dec: i64) -> String {
    let zeros = |s: &str| s.trim_start_matches('\u{2212}').chars().filter(|c| c.is_ascii_digit()).all(|c| c == '0');
    let s = num(v, dec);
    if v != 0.0 && zeros(&s) {
        let more = num(v, dec + 2);
        if !zeros(&more) { return more; }
        // 0.0001 is still "0" three decimals out, so six decimals come before exponential form
        let six = format!("{:.6}", v);
        let six = six.trim_end_matches('0').trim_end_matches('.');
        if !zeros(six) { return six.to_string(); }
        return format!("{v:.2e}").replace('-', "\u{2212}");
    }
    s
}

/// A frequency at a fixed number of decimals, so the three parts of a row line up. (round 6) `val` trims trailing
/// zeros, so a selection row printed "AA 0.53" beside "Aa 0.4" and a generation list read "0.99, 0.9901"; the page must
/// carry one precision. `dec` is the page precision (`shown_dec`), which already keeps every real part off "0".
fn fixed(v: f64, dec: i64) -> String {
    let d = dec.clamp(0, 6) as usize;
    format!("{v:.d$}").replace('-', "\u{2212}")
}

pub struct Report {
    /// x of the boundary between the two alleles in the gene pool, in pixels (the drag handle).
    pub split: f64,
    /// The three genotype segments of each generation as (x, width).
    pub segments: Vec<[(f64, f64); 3]>,
}

pub fn build(p: &Params) -> (String, Report) {
    let (p0, n, gens, sel, show_counts) = given(p);
    let g = generations(p);
    let dec = p.i("dec").clamp(1, 3);
    let print = p.s("style") == "Print";
    let ans = p.b(SHOW_ANSWERS);
    let f = svg::label_factor(p.s("label_size"));
    let ink = if print { "#555555" } else { INK };

    let rows_y0 = GEN_Y0 + gens as f64 * GEN_H + (gens.max(1) - 1) as f64 * GEN_GAP + 44.0;
    let extra_rows = if sel { 2 } else { 1 };
    // (round 6) the bars grow with the labels. The canvas widened at Large while the bars stayed 452 px, so a segment
    // that carried its label at Medium lost it at Large: a size control that removes annotations instead of enlarging
    // them. Keeping the bar width and the label width in the same ratio means a labelled segment stays labelled.
    let bar_w = BAR_W * f;
    let w = (BAR_X + bar_w + 40.0).max(W);
    let mut d = Doc::new(w, rows_y0 + (4.0 + extra_rows as f64) * ROW_H * f + 10.0);
    d.describe(
        "Hardy-Weinberg",
        "A gene pool split between two alleles and the genotype frequencies random mating produces in each generation",
    );

    // In Print a solid tone cannot tell three genotypes apart in a bitonal copy, so each gets its own direction of
    // hatching; the pool's two alleles get a fourth and a fifth.
    if print {
        d.defs(
            r##"<pattern id="hw-dom" width="7" height="7" patternUnits="userSpaceOnUse" patternTransform="rotate(45)"><rect width="7" height="7" fill="#ffffff"/><line x1="0" y1="0" x2="0" y2="7" stroke="#333333" stroke-width="2.4"/></pattern>
<pattern id="hw-het" width="7" height="7" patternUnits="userSpaceOnUse" patternTransform="rotate(-45)"><rect width="7" height="7" fill="#ffffff"/><line x1="0" y1="0" x2="0" y2="7" stroke="#333333" stroke-width="2.4"/></pattern>
<pattern id="hw-rec" width="6" height="6" patternUnits="userSpaceOnUse"><rect width="6" height="6" fill="#ffffff"/><line x1="0" y1="0" x2="6" y2="0" stroke="#333333" stroke-width="2.4"/></pattern>
<pattern id="hw-allele" width="8" height="8" patternUnits="userSpaceOnUse"><rect width="8" height="8" fill="#ffffff"/><line x1="0" y1="0" x2="0" y2="8" stroke="#333333" stroke-width="3.0"/></pattern>"##,
        );
    }
    let geno = if print {
        ["url(#hw-dom)", "url(#hw-het)", "url(#hw-rec)"]
    } else {
        [DOM, HET, REC]
    };
    let allele_a = if print { "url(#hw-allele)" } else { DOM };

    // the gene pool: the two alleles at their frequencies, always drawn because p is given
    let q0 = 1.0 - p0;
    let split = BAR_X + p0 * bar_w;
    d.rect(BAR_X, POOL_Y, p0 * bar_w, POOL_H, allele_a, ink, 1.4);
    d.rect(split, POOL_Y, q0 * bar_w, POOL_H, "#ffffff", ink, 1.4);
    d.raw(&text_el(24.0, POOL_Y + 18.0, 13.0 * f, "start", ink, "Gene pool"));
    // the precision the whole page prints at: raised until no real frequency prints as zero and p and q print honestly
    // (round 3: at one decimal place p = 0.99 printed "1" beside q = 0.01)
    let dp = shown_dec(p);
    // the legend under the pool, so a segment too narrow for its own label still says what it is
    let legend_y = POOL_Y + POOL_H + 16.0;
    d.rect(BAR_X, legend_y - 11.0, 12.0, 12.0, allele_a, ink, 1.0);
    d.raw(&text_el(BAR_X + 18.0, legend_y, 12.0 * f, "start", ink, &format!("A  p = {}", val(p0, dp))));
    d.rect(BAR_X + 150.0, legend_y - 11.0, 12.0, 12.0, "#ffffff", ink, 1.0);
    d.raw(&text_el(BAR_X + 168.0, legend_y, 12.0 * f, "start", ink, &format!("a  q = {}", val(q0, dp))));

    // one stacked bar per generation: the genotype that random mating makes from that generation's pool
    // the printed set must add up to one AND must not show a real frequency as zero: at one decimal place
    // p = 0.01 has AA = 0.0001, so the precision rises until every real part shows

    let mut segments = Vec::with_capacity(gens);
    for (k, cur) in g.iter().enumerate() {
        let y = GEN_Y0 + k as f64 * (GEN_H + GEN_GAP);
        d.raw(&text_el(24.0, y + GEN_H / 2.0 + 5.0, 13.0 * f, "start", ink, &format!("Generation {}", k + 1)));
        let fr = [cur.dom, cur.het, cur.rec];
        let shown = printed_parts(cur, dp);
        let names = ["AA", "Aa", "aa"];
        let mut seg = [(0.0f64, 0.0f64); 3];
        let mut x = BAR_X;
        for i in 0..3 {
            let wseg = fr[i] * bar_w;
            seg[i] = (x, wseg);
            if ans && wseg > 0.0 {
                d.rect(x, y, wseg, GEN_H, geno[i], ink, 1.2);
                let label = format!("{} {}", names[i], fixed(shown[i], dp));
                let tw = label.chars().count() as f64 * 12.0 * f * 0.55;
                // inside only when it fits: a real 0.0004 belongs in the readout, not squeezed into 0.2 px
                if tw <= wseg - 8.0 {
                    // in Print the bar is hatched, so the label sits on a white plate; in Colour the fill is solid
                    // and a plate would only cut a hole in the bar
                    if print {
                        d.rect(x + wseg / 2.0 - tw / 2.0 - 4.0, y + GEN_H / 2.0 - 8.0 * f, tw + 8.0, 16.0 * f, "#ffffff", "none", 0.0);
                    }
                    d.raw(&text_el(x + wseg / 2.0, y + GEN_H / 2.0 + 4.5 * f, 12.0 * f, "middle", ink, &label));
                }
            } else if ans {
                // an empty segment is marked, so a missing bar is not mistaken for a zero
                d.line(x, y + GEN_H, x + wseg, y + GEN_H, ink, 2.4);
            }
            x += wseg;
        }
        if !ans {
            // the row keeps its frame: an empty bar with the three divisions marked in the readout
            d.rect(BAR_X, y, bar_w, GEN_H, "none", ink, 1.2);
        }
        segments.push(seg);
    }

    // the givens, always printed: they are what the student is asked to work from
    let givens = format!(
        "p = {}, q = {}, N = {}",
        val(p0, dp),
        val(q0, dp),
        n
    );
    d.raw(&text_el(24.0, GEN_Y0 - 16.0, 12.0 * f, "start", ink, &givens));

    // the readout
    let mut rows: Vec<String> = Vec::new();
    rows.push(if sel {
        "Genotypes in generation 1 (they change from there)".to_string()
    } else {
        "Genotypes (the same in every generation)".to_string()
    });
    let first = g[0];
    let c = counts([first.dom, first.het, first.rec], n);
    let shown = printed_parts(&first, dp);
    for (i, (name, formula, v)) in [("AA", "p\u{b2}", shown[0]), ("Aa", "2pq", shown[1]), ("aa", "q\u{b2}", shown[2])].iter().enumerate() {
        rows.push(if ans {
            if show_counts {
                format!("{name}  {formula} = {}  ({} of {})", fixed(*v, dp), c[i], n)
            } else {
                format!("{name}  {formula} = {}", fixed(*v, dp))
            }
        } else if show_counts {
            format!("{name}  {formula} = {BLANK}  ({BLANK} of {n})")
        } else {
            format!("{name}  {formula} = {BLANK}")
        });
    }
    rows.push(if ans {
        format!("Check: AA + Aa + aa = {}", num(shown[0] + shown[1] + shown[2], dp))
    } else {
        format!("Check: AA + Aa + aa = {BLANK}")
    });
    if sel {
        // (round 4) at the page's own precision, not the typed one: p = 0.99 in one decimal place printed a column of
        // "1"s beside givens that plainly said 0.99. (round 6) at a fixed number of decimals, so the column does not
        // mix "0.99" with "0.9901"
        let ps: Vec<String> = if ans { g.iter().map(|x| fixed(x.p, dp)).collect() } else { (0..gens).map(|_| BLANK.to_string()).collect() };
        rows.push(format!("p in each generation  {}", ps.join(", ")));
    }
    for (i, line) in rows.iter().enumerate() {
        d.raw(&format!(
            r##"<text x="24.00" y="{:.2}" font-size="{}" text-anchor="start" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{}">{}</text>"##,
            rows_y0 + i as f64 * ROW_H * f,
            ROW_FS * f,
            ink,
            svg::esc(line)
        ));
    }

    (d.finish(), Report { split, segments })
}

/// The boundary between the two alleles is draggable: dragging it sets p, and with it the whole equilibrium. The
/// pool is drawn at a fixed size, so the drag inverts exactly.
pub fn handles(id: &str, p: &Params, _size: (f64, f64)) -> Option<Vec<Handle>> {
    if id != ID { return None; }
    let rep = build(p).1;
    // (round 6) the bars grow with the label size, so the drag inverts against the drawn width, not the base one
    let bar_w = BAR_W * svg::label_factor(p.s("label_size"));
    Some(vec![Handle::knob((rep.split, POOL_Y + POOL_H / 2.0), "Drag the boundary to set the frequency of the dominant allele", move |p, pt| {
        if !pt.0.is_finite() { return; }
        let frac = ((pt.0 - BAR_X) / bar_w).clamp(0.01, 0.99);
        p.set_f("p", (frac * 100.0).round() / 100.0);
    })])
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    let i = |k: &'static str, v: i64| (k, Value::I(v));
    let b = |k: &'static str, v: bool| (k, Value::B(v));
    Some(vec![
        Example { name: "p = 0.5: the classic 1 : 2 : 1", params: vec![f("p", 0.5)] },
        Example { name: "A rare dominant allele: most copies hide in heterozygotes", params: vec![f("p", 0.1)] },
        Example { name: "Selection removes the recessive phenotype", params: vec![f("p", 0.4), s("sel", "against the recessive phenotype"), i("gens", 5)] },
        Example { name: "Counts in a population of 1000", params: vec![f("p", 0.3), b("counts", true)] },
        Example { name: "Six generations of equilibrium", params: vec![f("p", 0.7), i("gens", 6)] },
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
    fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
    fn i(k: &'static str, v: i64) -> (&'static str, Value) { (k, Value::I(v)) }
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
    fn the_defaults_are_p_squared_two_pq_and_q_squared() {
        let g = generations(&with(&[]));
        assert_eq!(g.len(), 3);
        for x in &g {
            assert!((x.dom - 0.36).abs() < 1e-12, "{x:?}");
            assert!((x.het - 0.48).abs() < 1e-12, "{x:?}");
            assert!((x.rec - 0.16).abs() < 1e-12, "{x:?}");
            assert!((x.dom + x.het + x.rec - 1.0).abs() < 1e-12, "{x:?}");
        }
    }

    #[test]
    fn without_selection_every_generation_is_the_same() {
        for p0 in [0.05, 0.3, 0.5, 0.7, 0.95] {
            let g = generations(&with(&[f("p", p0), i("gens", 6)]));
            for w in g.windows(2) {
                assert!((w[0].dom - w[1].dom).abs() < 1e-12, "p0 = {p0}: {:?}", g);
                assert!((w[0].p - w[1].p).abs() < 1e-12, "p0 = {p0}: {:?}", g);
            }
        }
    }

    #[test]
    fn selection_against_the_recessive_raises_p_by_p_over_one_minus_q_squared() {
        let p = with(&[f("p", 0.4), s("sel", "against the recessive phenotype"), i("gens", 5)]);
        let g = generations(&p);
        for w in g.windows(2) {
            let q = 1.0 - w[0].p;
            let want = w[0].p / (1.0 - q * q);
            assert!((w[1].p - want).abs() < 1e-12, "{:?} -> {:?}", w[0], w[1]);
        }
        // and p rises every generation, towards 1
        assert!(g.windows(2).all(|w| w[1].p > w[0].p), "{g:?}");
        assert!(warnings(&p).iter().any(|w| w.contains("not in Hardy")), "{:?}", warnings(&p));
        assert!(warnings(&with(&[])).is_empty());
    }

    #[test]
    fn a_real_frequency_never_prints_as_zero() {
        // p = 0.01 puts AA at 0.0001; at one or two decimals it used to print "AA  p\u{b2} = 0" beside a bar that
        // was really there
        for dec in [1, 2, 3] {
            let t = texts(&render_of(&[f("p", 0.01), ("dec", Value::I(dec))]));
            let aa = t.lines().find(|l| l.starts_with("AA  p")).unwrap_or("");
            assert!(!aa.ends_with("= 0") && !aa.ends_with("= 0.0"), "dec = {dec}: {aa}");
        }
        // and the given p itself must not round away either
        let t = texts(&render_of(&[f("p", 0.01), ("dec", Value::I(1))]));
        assert!(t.contains("A  p = 0.01"), "{t}");
    }

    #[test]
    fn the_selection_line_prints_p_at_the_page_precision() {
        // (round 4) the givens said p = 0.99 while the selection line said 1, 1, 1: it used the typed precision
        let t = texts(&render_of(&[f("p", 0.99), ("dec", Value::I(1)), s("sel", "against the recessive phenotype"), b(SHOW_ANSWERS, true)]));
        assert!(t.contains("p in each generation  0.99"), "{t}");
        assert!(!t.contains("p in each generation  1,"), "{t}");
        // and the editor note that explains the rise says the same numbers
        let p = with(&[f("p", 0.99), ("dec", Value::I(1)), s("sel", "against the recessive phenotype"), b(SHOW_ANSWERS, true)]);
        let note = warnings(&p).join(" ");
        assert!(note.contains("from 0.99"), "{note}");
    }

    #[test]
    fn the_allele_frequencies_never_print_as_one_or_zero_when_they_are_not() {
        // (round 3) p = 0.99 at one decimal place printed "p = 1, q = 0.01" beside AA p² = 0.9801 and aa q² = 0.0001
        for (p0, dec) in [(0.99, 1), (0.01, 1), (0.999, 1), (0.5, 1)] {
            let t = texts(&render_of(&[f("p", p0), ("dec", Value::I(dec)), b(SHOW_ANSWERS, true)]));
            let line = t.lines().find(|l| l.starts_with("p = ")).unwrap_or("");
            let after = |k: &str| line.split(&format!("{k} = ")).nth(1).and_then(|r| r.split(',').next()).and_then(|v| v.parse::<f64>().ok());
            let (p, q) = (after("p").unwrap_or(-1.0), after("q").unwrap_or(-1.0));
            assert!((p + q - 1.0).abs() < 1e-9, "p = {p0}, dec = {dec}: printed p + q = {p} + {q}: {t}");
            assert!(p0 >= 1.0 || p < 1.0, "p = {p0} printed as {p}: {t}");
            assert!(p0 <= 0.0 || p > 0.0, "p = {p0} printed as {p}: {t}");
        }
    }

    #[test]
    fn the_printed_fractions_are_the_nearest_ones_that_add_up() {
        // (round 5) a 1:2:1 cross was printed 0.3 / 0.4 / 0.3: the whole rounding difference went into the largest part
        // instead of the parts that lost the most to rounding
        let g = Gen { p: 0.5, dom: 0.25, het: 0.5, rec: 0.25 };
        assert_eq!(printed_parts(&g, 1), [0.3, 0.5, 0.2], "the heterozygote is a unit low");
        let g = Gen { p: 0.34, dom: 0.4356, het: 0.4488, rec: 0.1156 };
        let parts = printed_parts(&g, 2);
        assert!((parts.iter().sum::<f64>() - 1.0).abs() < 1e-9, "{parts:?}");
        assert!((parts[1] - 0.45).abs() < 1e-9, "2pq should print 0.45, the count says 449 of 1000: {parts:?}");
    }

    #[test]
    fn the_printed_fractions_add_up_to_the_printed_check() {
        // at one decimal place p = 0.6 printed "0.4 + 0.5 + 0.2 = 1.1" beside a check that said 1
        for (p0, dec) in [(0.6, 1), (0.34, 2), (0.5, 1), (0.7, 3), (0.01, 1)] {
            let p = with(&[f("p", p0), ("dec", Value::I(dec))]);
            let g = generations(&p);
            let parts = printed_parts(&g[0], shown_dec(&p));
            let sum: f64 = parts.iter().sum();
            assert!((sum - 1.0).abs() < 1e-9, "p = {p0}, dec = {dec}: {parts:?} add to {sum}");
            // and no real frequency is shown as zero
            for (ex, sh) in [g[0].dom, g[0].het, g[0].rec].iter().zip(parts.iter()) {
                assert!(*ex == 0.0 || *sh != 0.0, "p = {p0}, dec = {dec}: {ex} printed as {sh}");
            }
        }
        // and the printed check line agrees with them
        let t = texts(&render_of(&[f("p", 0.6), ("dec", Value::I(1)), b(SHOW_ANSWERS, true)]));
        assert!(t.contains("Check: AA + Aa + aa = 1"), "{t}");
    }

    #[test]
    fn the_counts_sum_to_the_population() {
        for (p0, n) in [(0.6, 1000), (0.1, 37), (0.333, 10), (0.95, 9999)] {
            let p = with(&[f("p", p0), i("n", n as i64)]);
            let g = generations(&p);
            let c = counts([g[0].dom, g[0].het, g[0].rec], n);
            assert_eq!(c.iter().sum::<usize>(), n, "p = {p0}, n = {n}: {c:?}");
        }
    }

    #[test]
    fn a_count_beside_a_fraction_is_the_fraction_times_the_population() {
        // (round 6 blocker) p = 0.34 printed "AA p² = 0.12 (116 of 1000)" — 0.12 × 1000 is 120 — and
        // "aa q² = 0.43 (435 of 1000)" where 435/1000 rounds to 0.44. The fraction is raised in precision until its
        // product with N gives the count on the same line, and the three fractions still add to 1.
        let grid: Vec<(f64, i64)> = vec![
            (0.34, 1000), (0.06, 9999), (0.07, 1000), (0.6, 1000), (0.01, 1000),
            (0.3, 10), (0.95, 9999), (0.5, 10000), (0.333, 37), (0.5, 10), (0.99, 1000),
        ];
        for (p0, n) in grid {
            let p = with(&[f("p", p0), i("n", n), b("counts", true)]);
            let g = generations(&p);
            let dp = shown_dec(&p);
            let parts = printed_parts(&g[0], dp);
            let c = counts([g[0].dom, g[0].het, g[0].rec], n as usize);
            assert!((parts.iter().sum::<f64>() - 1.0).abs() < 1e-9, "p = {p0}, n = {n}, dp = {dp}: {parts:?} do not sum to 1");
            for i in 0..3 {
                assert_eq!(
                    (parts[i] * n as f64).round() as usize, c[i],
                    "p = {p0}, n = {n}, dp = {dp}: part {i} prints {} which × {n} is not the count {}", parts[i], c[i]
                );
            }
        }
        // the reported case, as printed
        let t = texts(&render_of(&[f("p", 0.34), i("n", 1000), b("counts", true)]));
        assert!(t.contains("AA  p\u{b2} = 0.116  (116 of 1000)"), "{t}");
        assert!(t.contains("aa  q\u{b2} = 0.435  (435 of 1000)"), "{t}");
        assert!(!t.contains("0.12  (116"), "the fraction still contradicts its count: {t}");
        // and the p² = 0.0036 case that used to print 0.01 beside "36 of 9999"
        let t = texts(&render_of(&[f("p", 0.06), i("n", 9999), b("counts", true)]));
        assert!(t.contains("AA  p\u{b2} = 0.0036  (36 of 9999)"), "{t}");
    }

    /// The bar labels: text bodies that start with a genotype name and a number (not the readout's "AA p²").
    fn bar_labels(svg: &str) -> Vec<String> {
        texts(svg)
            .lines()
            .filter(|l| {
                (l.starts_with("AA ") || l.starts_with("Aa ") || l.starts_with("aa "))
                    && l.chars().nth(3).map(|c| c.is_ascii_digit()).unwrap_or(false)
            })
            .map(|l| l.to_string())
            .collect()
    }

    #[test]
    fn no_labelled_segment_loses_its_label_when_the_labels_grow() {
        // (round 6 major) at Large the canvas widened but the bars did not, so "aa 0.16" was drawn at Medium and gone
        // at Large. The bars now grow with the labels, so every segment labelled at Medium is labelled at Large.
        for case in [vec![], vec![f("p", 0.34)], vec![f("p", 0.5)], vec![f("p", 0.3)]] {
            let mut med = case.clone();
            let mut big = case.clone();
            med.push(s("label_size", "Medium"));
            big.push(s("label_size", "Large"));
            let medium = bar_labels(&render_of(&med));
            let large = bar_labels(&render_of(&big));
            for l in &medium {
                assert!(large.contains(l), "{} is drawn at Medium but lost at Large (Large labels: {large:?})", l);
            }
        }
        // the reported case: p = 0.6 at Large keeps all three
        let large = bar_labels(&render_of(&[s("label_size", "Large")]));
        assert!(large.contains(&"aa 0.16".to_string()), "aa 0.16 is lost at Large: {large:?}");
        assert!(large.contains(&"AA 0.36".to_string()), "{large:?}");
        // and the Print selection case keeps "AA 0.16" in generation 1
        let sel = render_of(&[f("p", 0.4), s("sel", "against the recessive phenotype"), i("gens", 6), s("style", "Print"), s("label_size", "Large")]);
        assert!(bar_labels(&sel).contains(&"AA 0.16".to_string()), "AA 0.16 is lost in the selection bar: {:?}", bar_labels(&sel));
    }

    #[test]
    fn a_row_of_frequencies_carries_one_precision() {
        // (round 6 minor) "AA 0.53" beside "Aa 0.4" in a selection row, and "0.99, 0.9901" in the generation list,
        // because `val` trimmed trailing zeros. Every part of a row now prints at the page precision.
        let sel = render_of(&[f("p", 0.4), s("sel", "against the recessive phenotype"), i("gens", 6), s("style", "Print"), s("label_size", "Large")]);
        let dp = shown_dec(&with(&[f("p", 0.4), s("sel", "against the recessive phenotype"), i("gens", 6)])) as usize;
        for l in bar_labels(&sel) {
            let num = l.split_whitespace().nth(1).unwrap_or("");
            let dec = num.split('.').nth(1).unwrap_or("");
            assert_eq!(dec.chars().count(), dp, "{l:?} does not carry the page precision {dp}");
        }
        assert!(texts(&sel).contains("Aa 0.40"), "{}", texts(&sel));
        assert!(!texts(&sel).contains("Aa 0.4\n"), "{}", texts(&sel));
        // the one-decimal, selection, p = 0.99 case: the generation list must not mix "0.99" with "0.9901"
        let p99 = with(&[f("p", 0.99), i("dec", 1), s("sel", "against the recessive phenotype"), i("gens", 6), b(SHOW_ANSWERS, true)]);
        let t = texts(&render_of(&[f("p", 0.99), i("dec", 1), s("sel", "against the recessive phenotype"), i("gens", 6), b(SHOW_ANSWERS, true)]));
        let dp = shown_dec(&p99) as usize;
        let line = t.lines().find(|l| l.starts_with("p in each generation")).unwrap_or("");
        let list = line.rsplit("  ").next().unwrap_or("");
        for tok in list.split(", ") {
            let dec = tok.split('.').nth(1).unwrap_or("");
            assert_eq!(dec.chars().count(), dp, "{tok:?} in {line:?} does not carry {dp} decimals");
        }
        // and the readout row for p = 0.99 carries the page precision too
        assert!(t.contains("AA  p\u{b2} = 0.9801"), "{t}");
        assert!(!t.contains("AA  p\u{b2} = 0.98\n"), "{t}");
    }

    #[test]
    fn the_genotypes_are_blank_on_the_student_copy() {
        let stu = texts(&render_of(&[b(SHOW_ANSWERS, false)]));
        assert!(stu.contains("AA  p\u{b2} = _____"), "{stu}");
        assert!(stu.contains("Aa  2pq = _____"), "{stu}");
        assert!(!stu.contains("0.36"), "the answer leaked: {stu}");
        // p is given, so the pool and its legend stay
        assert!(stu.contains("A  p = 0.6"), "{stu}");
        assert!(stu.contains("p = 0.6, q = 0.4"), "{stu}");
        let svg = render_of(&[b(SHOW_ANSWERS, false)]);
        // the pool legitimately uses the dominant colour (p is given); the other two are genotype-only
        assert!(!svg.contains(&format!("fill=\"{HET}\"")), "a genotype bar was drawn on the student copy");
        assert!(!svg.contains(&format!("fill=\"{REC}\"")), "a genotype bar was drawn on the student copy");
        let key = texts(&render_of(&[b(SHOW_ANSWERS, true)]));
        assert!(key.contains("AA  p\u{b2} = 0.36"), "{key}");
    }

    #[test]
    fn a_segment_too_narrow_for_its_label_is_drawn_without_one() {
        // p = 0.01 makes AA 0.0001 of the bar: 0.045 px, far too narrow for "AA 0.0001"
        let svg = render_of(&[f("p", 0.01), ("dec", Value::I(3)), b(SHOW_ANSWERS, true)]);
        assert!(!texts(&svg).contains("AA 0.000"), "a label printed into a sliver: {}", texts(&svg));
        // and the bars still add up to the full width
        let rep = build(&with(&[f("p", 0.01), b(SHOW_ANSWERS, true)])).1;
        let last = rep.segments[0];
        assert!((last[2].0 + last[2].1 - (BAR_X + BAR_W)).abs() < 1e-9, "{last:?}");
    }

    #[test]
    fn the_drag_inverts_exactly() {
        let p = with(&[]);
        let rep = build(&p).1;
        assert!((rep.split - (BAR_X + 0.6 * BAR_W)).abs() < 1e-9, "{}", rep.split);
        let hs = handles(ID, &p, (W, 400.0)).unwrap();
        assert_eq!(hs.len(), 1);
        // the handle sits on the drawn boundary
        assert!((hs[0].pos.0 - rep.split).abs() < 1e-9);
    }

    #[test]
    fn print_is_neutral_grey_only() {
        let svg = render_of(&[s("style", "Print"), b(SHOW_ANSWERS, true)]);
        for line in svg.lines().filter(|l| l.contains("stroke=\"#") || l.contains("fill=\"#")) {
            for part in line.split(['"']).filter(|s| s.starts_with('#') && s.len() == 7 && s[1..].chars().all(|c| c.is_ascii_hexdigit())) {
                let (r, g, b) = (u8::from_str_radix(&part[1..3], 16).unwrap(), u8::from_str_radix(&part[3..5], 16).unwrap(), u8::from_str_radix(&part[5..7], 16).unwrap());
                assert!(r == g && g == b, "Print draws a hue: {part} in {line}");
            }
        }
    }

    /// Writes every example (Colour, Print, student, key) to $VERSHION_PNG_DIR.
    #[test]
    fn hardy_weinberg_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![
            ("default".into(), vec![]),
            ("large".into(), vec![s("label_size", "Large")]),
            // the extremes: a sliver of a dominant allele, and a population of ten with counts on
            ("rare".into(), vec![f("p", 0.01), ("dec", Value::I(3)), b(SHOW_ANSWERS, true)]),
            ("common".into(), vec![f("p", 0.99), ("dec", Value::I(3)), b(SHOW_ANSWERS, true)]),
            ("small".into(), vec![f("p", 0.3), i("n", 10), b("counts", true)]),
            ("six".into(), vec![f("p", 0.4), s("sel", "against the recessive phenotype"), i("gens", 6)]),
        ];
        for (idx, (_, l)) in examples(ID).unwrap().into_iter().map(|e| (e.name.to_string(), e.params)).enumerate() { cases.push((format!("ex{idx}"), l)); }
        for (tag, list) in cases {
            for (kind, extra) in [("colour", vec![]), ("print", vec![s("style", "Print")]), ("student", vec![b(SHOW_ANSWERS, false)]), ("key", vec![b(SHOW_ANSWERS, true)])] {
                let mut l = list.clone();
                l.extend(extra);
                let svg = render_of(&l);
                std::fs::write(format!("{dir}/hw-{tag}-{kind}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
                if kind == "print" {
                    let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
                    let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                    image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/hw-{tag}-print-bw66.png")).unwrap();
                }
            }
        }
    }
}

