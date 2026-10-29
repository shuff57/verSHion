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

/// The three frequencies as they are printed: each rounded to `dec` places, then the largest moved by the rounding
/// difference, so the printed numbers add up to exactly 1. Rounding each on its own printed "0.4 + 0.5 + 0.2 = 1.1"
/// beside a Check line that said 1.
pub fn printed_parts(g: &Gen, dec: i64) -> [f64; 3] {
    let f = 10f64.powi(dec as i32);
    let r = |v: f64| (v * f).round() / f;
    let mut v = [r(g.dom), r(g.het), r(g.rec)];
    let diff = r(1.0 - (v[0] + v[1] + v[2]));
    if diff != 0.0 {
        let i = (0..3).max_by(|a, b| v[*a].partial_cmp(&v[*b]).unwrap_or(std::cmp::Ordering::Equal)).unwrap_or(0);
        v[i] = r(v[i] + diff);
    }
    v
}

/// The decimal places the picture is printed at: `dec`, raised until no real frequency would print as zero — and, since
/// round 3, until the allele frequencies themselves print honestly: p = 0.99 at one decimal place printed "1" beside a
/// q of "0.01" and a genotype set that plainly does not come from p = 1.
pub fn shown_dec(p: &Params) -> i64 {
    let g = generations(p);
    let dec = p.i("dec").clamp(1, 3);
    let (p0, _, _, _, _) = given(p);
    let q0 = 1.0 - p0;
    let collapses = |v: f64, d: i64| {
        let shown = num(v, d).replace('\u{2212}', "-").parse::<f64>().unwrap_or(v);
        (v < 1.0 && shown >= 1.0) || (v > 0.0 && shown <= 0.0)
    };
    let needs_more = |gg: &Gen, d: i64| {
        let pr = printed_parts(gg, d);
        [gg.dom, gg.het, gg.rec].iter().zip(pr.iter()).any(|(ex, sh)| *ex != 0.0 && *sh == 0.0)
    };
    let mut dp = dec;
    while dp < 5 && (needs_more(&g[0], dp) || collapses(p0, dp) || collapses(q0, dp)) { dp += 1; }
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
    vec![format!(
        "Selection removes every recessive homozygote, so the population is not in Hardy\u{2013}Weinberg equilibrium: p rises from {} to {} over the {} generations shown, and to {} in the next.",
        num(g[0].p, 2),
        num(last, 2),
        gens,
        num(next, 2)
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
    let w = (W * (1.0 + 0.55 * (f - 1.0))).max(W);
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
    let split = BAR_X + p0 * BAR_W;
    d.rect(BAR_X, POOL_Y, p0 * BAR_W, POOL_H, allele_a, ink, 1.4);
    d.rect(split, POOL_Y, q0 * BAR_W, POOL_H, "#ffffff", ink, 1.4);
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
            let wseg = fr[i] * BAR_W;
            seg[i] = (x, wseg);
            if ans && wseg > 0.0 {
                d.rect(x, y, wseg, GEN_H, geno[i], ink, 1.2);
                let label = format!("{} {}", names[i], val(shown[i], dp));
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
            d.rect(BAR_X, y, BAR_W, GEN_H, "none", ink, 1.2);
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
                format!("{name}  {formula} = {}  ({} of {})", val(*v, dp), c[i], n)
            } else {
                format!("{name}  {formula} = {}", val(*v, dp))
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
        let ps: Vec<String> = if ans { g.iter().map(|x| val(x.p, dec)).collect() } else { (0..gens).map(|_| BLANK.to_string()).collect() };
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
    Some(vec![Handle::knob((rep.split, POOL_Y + POOL_H / 2.0), "Drag the boundary to set the frequency of the dominant allele", move |p, pt| {
        if !pt.0.is_finite() { return; }
        let f = ((pt.0 - BAR_X) / BAR_W).clamp(0.01, 0.99);
        p.set_f("p", (f * 100.0).round() / 100.0);
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

