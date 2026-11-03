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
/// p is held to six decimals: the given p is the integer `k` over this.
const P_SCALE: i128 = 1_000_000;

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
/// (round 9 blocker) p is rounded ONCE, here, to six decimals, the finest the page prints: every number on the page
/// (the givens, p\u{b2}, the p list) is then computed from the one value that is printed. A typed 0.123456 used to
/// print as "p = 0.1235" (the shared formatter stops at four decimals) beside a p\u{b2} of 0.015241 = 0.123456\u{b2}.
pub fn given(p: &Params) -> (f64, usize, usize, bool, bool) {
    (
        (fin(p.f("p"), 0.01, 0.99, 0.6) * P_SCALE as f64).round() / P_SCALE as f64,
        p.i("n").clamp(10, 10000) as usize,
        p.i("gens").clamp(2, 6) as usize,
        p.s("sel").starts_with("against"),
        p.b("counts"),
    )
}

/// One generation: the allele frequency that produced it and its three genotype frequencies, which sum to 1.
/// `pn / pd` is the same p as an exact fraction: (round 9) every printed number is rounded from it in whole integer
/// units, so two equal true frequencies (p = 0.5: AA = aa = 0.25) can never be told apart by floating-point noise.
#[derive(Clone, Copy, Debug)]
pub struct Gen {
    pub p: f64,
    pub dom: f64,
    pub het: f64,
    pub rec: f64,
    pub pn: i128,
    pub pd: i128,
}

fn gcd(a: i128, b: i128) -> i128 { if b == 0 { a.abs().max(1) } else { gcd(b, a % b) } }

impl Gen {
    /// The generation made by p = n / d exactly.
    pub fn from_ratio(n: i128, d: i128) -> Gen {
        let p = n as f64 / d as f64;
        let q = 1.0 - p;
        let g = gcd(n, d);
        Gen { p, dom: p * p, het: 2.0 * p * q, rec: q * q, pn: n / g, pd: d / g }
    }
    /// The three genotype frequencies as exact numerators over one denominator: p\u{b2}, 2pq, q\u{b2} over pd\u{b2}.
    fn exact(&self) -> ([i128; 3], i128) {
        let (a, b) = (self.pn, self.pd);
        ([a * a, 2 * a * (b - a), (b - a) * (b - a)], b * b)
    }
    /// The next generation's p under selection against the recessive phenotype, exactly: p / (1 \u{2212} q\u{b2}) = 1 / (2 \u{2212} p).
    fn next_ratio(&self) -> (i128, i128) {
        let (n, d) = (self.pd, 2 * self.pd - self.pn);
        let g = gcd(n, d);
        (n / g, d / g)
    }
}

/// The generations shown. Under selection against the recessive phenotype the survivors are AA and Aa, so the next
/// generation's allele frequency is p' = p / (1 - q²) — the recessive allele is lost faster than the recessive
/// phenotype, because it hides in heterozygotes.
pub fn generations(p: &Params) -> Vec<Gen> {
    let (p0, _, gens, sel, _) = given(p);
    let mut pa = p0;
    let (mut a, mut b) = ((p0 * P_SCALE as f64).round() as i128, P_SCALE);
    let g0 = gcd(a, b);
    a /= g0;
    b /= g0;
    let mut out = Vec::with_capacity(gens);
    for _ in 0..gens {
        let q = 1.0 - pa;
        let cur = Gen { p: pa, dom: pa * pa, het: 2.0 * pa * q, rec: q * q, pn: a, pd: b };
        out.push(cur);
        if sel {
            pa /= 1.0 - q * q;
            (a, b) = cur.next_ratio();
        }
    }
    out
}

/// Splits `m` units in the proportions `n / den` (the `n` sum to `den`): the whole parts, then the leftover units to the
/// parts with the largest exact remainders, all in integers. Remainder ties go to the SMALLER true value (so a mirror
/// population, p and 1 \u{2212} p, gets the mirror allocation), then to the earlier part. With `keep_ties` a group of
/// EQUAL true values is never split (two equal bars never print unequal): the unit goes to the next part instead.
fn allocate(n: [i128; 3], m: i128, den: i128, keep_ties: bool) -> [i128; 3] {
    let mut u = n.map(|x| x * m / den);
    let rem = n.map(|x| x * m % den);
    let left = (m - u.iter().sum::<i128>()).clamp(0, 3) as u32;
    let splits = |mask: u8| (0..3).any(|i| (i + 1..3).any(|j| n[i] == n[j] && ((mask >> i) & 1) != ((mask >> j) & 1)));
    let pick = |respect: bool| -> Option<u8> {
        (0u8..8)
            .filter(|mask| mask.count_ones() == left && !(respect && splits(*mask)))
            .max_by_key(|&mask| {
                let on = |i: usize| (mask >> i) & 1 == 1;
                let r: i128 = (0..3).filter(|&i| on(i)).map(|i| rem[i]).sum();
                let v: i128 = (0..3).filter(|&i| on(i)).map(|i| n[i]).sum();
                (r, -v, -(mask as i32))
            })
    };
    let mask = if keep_ties { pick(true).or_else(|| pick(false)) } else { pick(false) }.unwrap_or(0);
    for (i, ui) in u.iter_mut().enumerate() {
        if (mask >> i) & 1 == 1 { *ui += 1; }
    }
    u
}

/// The remainder the largest-remainder rule gives away at `m` units with and without keeping equal values together.
/// They differ exactly when an equal pair straddles the cut, so that keeping them equal costs a unit somewhere else.
fn tie_costs(n: [i128; 3], m: i128, den: i128) -> bool {
    let score = |u: [i128; 3]| -> i128 { (0..3).map(|i| (u[i] * den - n[i] * m).abs()).sum() };
    score(allocate(n, m, den, true)) != score(allocate(n, m, den, false))
}

/// The printed genotype frequencies of `g` at `dec` decimals, in whole units of 10^-dec.
fn printed_units(g: &Gen, dec: i64) -> [i128; 3] {
    let (n, den) = g.exact();
    allocate(n, 10i128.pow(dec.clamp(0, 12) as u32), den, true)
}

/// A fraction n / d at `dec` decimals, rounded half up in exact integers.
fn fixed_ratio(n: i128, d: i128, dec: i64) -> String {
    let s = 10i128.pow(dec.clamp(0, 12) as u32);
    let u = (2 * n * s + d) / (2 * d);
    let dd = dec.clamp(0, 12) as usize;
    if dd == 0 { return u.to_string(); }
    format!("{}.{:0dd$}", u / s, u % s)
}

/// The given p (or q) as typed: up to six decimals, trailing zeros trimmed. It is the exact `k / 10^6` that the page
/// computes from, so the givens and every number below them agree.
fn given_str(v: f64) -> String {
    let k = (v * P_SCALE as f64).round() as i128;
    let s = format!("{}.{:06}", k / P_SCALE, k % P_SCALE);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The three frequencies as they are printed: rounded by largest remainder, so the printed numbers add up to exactly 1
/// and each is the nearest it can be to its true value. Rounding each on its own printed "0.4 + 0.5 + 0.2 = 1.1" beside a
/// Check line that said 1; (round 5) moving the whole difference into the largest part printed a 1:2:1 cross as
/// 0.3 / 0.4 / 0.3, a full unit low on the heterozygote against a bar that was plainly half. (round 9) In exact integer
/// units, and two equal true values always print equal (`allocate`).
pub fn printed_parts(g: &Gen, dec: i64) -> [f64; 3] {
    let f = 10f64.powi(dec as i32);
    printed_units(g, dec).map(|u| u as f64 / f)
}

/// Whether a real frequency would print as a whole 1, or as 0, at `d` decimals — the allele-collapse test: p = 0.99
/// at one decimal place printed "1" beside q = 0.01.
fn collapses(v: f64, d: i64) -> bool {
    let shown = num(v, d).replace('\u{2212}', "-").parse::<f64>().unwrap_or(v);
    (v < 1.0 && shown >= 1.0) || (v > 0.0 && shown <= 0.0)
}

/// Whether any real genotype frequency would print as zero at `d` decimals (round 3: p = 0.01 makes AA = 0.0001).
fn needs_more(gg: &Gen, d: i64) -> bool {
    let (n, _) = gg.exact();
    let pr = printed_units(gg, d);
    (0..3).any(|i| n[i] != 0 && pr[i] == 0)
}

/// A precision the page may print at: no real part prints as zero and the allele frequency does not collapse to 1 or 0.
/// (round 9) Nor one where two EQUAL true frequencies straddle the rounding cut: p = 0.5 at one decimal printed
/// "AA 0.3, aa 0.2" beside two equal bars (an array-order tie-break), and keeping them equal there would push a whole
/// unit onto the heterozygote (0.2 / 0.6 / 0.2). One decimal more prints 0.25 / 0.50 / 0.25.
fn valid_dec(gg: &Gen, p0: f64, d: i64) -> bool {
    let (n, den) = gg.exact();
    !needs_more(gg, d) && !collapses(p0, d) && !collapses(1.0 - p0, d) && !tie_costs(n, 10i128.pow(d.clamp(0, 12) as u32), den)
}

/// Whether the selection trajectory reads as a rise at `d` decimals: every printed p in the list is above the one
/// before it, and the next generation's p (which the editor note names) is above the last. (round 9 minor) At one
/// decimal p = 0.5 printed "0.5, 0.7, 0.8, 0.8, 0.8, 0.9" while the note, at its own finer precision, said "0.50 to
/// 0.86 \u{2026} 0.88": the page and the note now share the one precision, raised until the list rises.
fn rises(g: &[Gen], d: i64) -> bool {
    let Some(last) = g.last() else { return true };
    let (nn, nd) = last.next_ratio();
    let mut ps: Vec<String> = g.iter().map(|x| fixed_ratio(x.pn, x.pd, d)).collect();
    ps.push(fixed_ratio(nn, nd, d));
    ps.windows(2).all(|w| w[1].parse::<f64>().unwrap_or(0.0) > w[0].parse::<f64>().unwrap_or(0.0))
}

/// The decimal places the picture is printed at: `dec`, raised until no real frequency would print as zero — and, since
/// round 3, until the allele frequencies themselves print honestly: p = 0.99 at one decimal place printed "1" beside a
/// q of "0.01" and a genotype set that plainly does not come from p = 1. (round 6) With counts shown it is raised again
/// until each printed fraction, times N, rounds to the count printed beside it: p = 0.34 printed "AA p² = 0.12
/// (116 of 1000)", and 0.12 × 1000 is 120, not 116. Raising the precision to three makes 0.116 × 1000 = 116.
/// (round 7) When no precision makes every fraction agree, it takes the valid one that contradicts the counts least.
/// (round 8 blocker) It is never COARSER than the typed precision, nor than the decimals of the typed p: the strict
/// pass used to try coarser precisions before the minimum-error rule ran, so a count tie broken on floating-point
/// noise dropped p = 0.5, N = 10 at two decimals to "0.3 / 0.5 / 0.2", and p = 0.21 printed as "p = 0.2" beside
/// "p² = 0.1". A population too small to hold the typed fraction now prints it with a count at most one off.
pub fn shown_dec(p: &Params) -> i64 {
    let g = generations(p);
    let dec = p.i("dec").clamp(1, 3);
    let (p0, n, _, _, show_counts) = given(p);
    // the floor: what was typed, and never fewer decimals than the given p itself, which must print exactly
    let lo = dec.max(own_decimals(p0)).min(6);
    let sel = given(p).3;
    // (round 9) under selection only the precisions at which the p list reads as a rise, unless none up to six does
    let readable: Vec<i64> = (lo..=6).filter(|&d| valid_dec(&g[0], p0, d) && (!sel || rises(&g, d))).collect();
    let valid: Vec<i64> = if readable.is_empty() { (lo..=6).filter(|&d| valid_dec(&g[0], p0, d)).collect() } else { readable };
    let Some(&first) = valid.first() else {
        // nothing short of six decimals keeps every real part off zero: six is the finest the page prints
        return 6;
    };
    if !show_counts { return first; }
    // The printed fraction is what a reader multiplies by N, so it should give back the count on the same line (round
    // 6 blocker: "0.43 (435 of 1000)"). The count is the largest-remainder allocation of N, so for some (p, N) no
    // precision agrees exactly; among the valid precisions, finest-needed only, take the one with the least total
    // contradiction, the typed one first on ties. The contradiction is counted in whole units from the printed
    // integer numerator, so floating-point noise cannot make two precisions look different.
    let c = counts_of(&g[0], n);
    let mut best = (i64::MAX, first);
    for &d in &valid {
        let e = count_error(&g[0], d, n, &c);
        if e < best.0 { best = (e, d); }
    }
    best.1
}

/// The decimals a typed value carries (0.21 → 2, 0.333 → 3, 0.6 → 1), at most six.
fn own_decimals(v: f64) -> i64 {
    for k in 0..=6i64 {
        let f = 10f64.powi(k as i32);
        if ((v * f).round() - v * f).abs() < 1e-7 { return k; }
    }
    6
}

/// round(units × N / 10^d), half up, in exact integers: what a reader gets multiplying the printed fraction by N.
fn implied_count(units: i64, n: usize, d: i64) -> i64 {
    let den = 10i64.pow(d.clamp(0, 9) as u32);
    (2 * units * n as i64 + den) / (2 * den)
}

/// The total |fraction × N − count| over the three rows at precision `d`, in whole units.
fn count_error(gg: &Gen, d: i64, n: usize, c: &[usize; 3]) -> i64 {
    let pr = printed_units(gg, d);
    (0..3).map(|i| (implied_count(pr[i] as i64, n, d) - c[i] as i64).abs()).sum()
}

/// Counts that sum to `n` exactly: the whole parts, then the leftovers in order of largest remainder (rounding each
/// frequency on its own could print three counts that add up to 999 or 1001). (round 9) In exact integers, so the
/// remainders of p = 0.07 and of its mirror 0.93 compare equal instead of on floating-point noise, and a remainder tie
/// goes to the smaller true value first, then to the earlier genotype.
pub fn counts_of(g: &Gen, n: usize) -> [usize; 3] {
    let (num, den) = g.exact();
    allocate(num, n as i128, den, false).map(|c| c.max(0) as usize)
}

pub fn warnings(p: &Params) -> Vec<String> {
    let (p0, _, gens, sel, _) = given(p);
    let mut out = Vec::new();
    let typed = p.f("p");
    if typed.is_finite() && (0.01..=0.99).contains(&typed) && (typed - p0).abs() > 1e-12 {
        out.push(format!("p is rounded to six decimal places, the finest the page prints: p = {}.", given_str(p0)));
    }
    if !sel { return out; }
    let g = generations(p);
    let Some(lastg) = g.last().copied() else { return out };
    let (nn, nd) = lastg.next_ratio();
    // the editor note is read on its own, with no page to raise its precision: take the page's. (round 8 minor) At
    // one fixed precision, like the page's p list: `val` trimmed, so the note read "0.41 → 0.851 → 0.87".
    // A rise that prints as no rise ("to 0.9 … and to 0.9 in the next") says nothing: the note's one precision is
    // raised until the three numbers it names read as rising.
    // (round 9 minor) the note and the page's p list carry ONE precision: the page's, which `shown_dec` already raised
    // until the list reads as a rise. The note used to raise its own, so it said "0.50 to 0.86" over a list of 0.8s.
    let d = shown_dec(p);
    out.push(format!(
        "Selection removes every recessive homozygote, so the population is not in Hardy\u{2013}Weinberg equilibrium: p rises from {} to {} over the {} generations shown, and to {} in the next.",
        fixed_ratio(g[0].pn, g[0].pd, d),
        fixed_ratio(lastg.pn, lastg.pd, d),
        gens,
        fixed_ratio(nn, nd, d)
    ));
    out
}

fn text_el(x: f64, y: f64, size: f64, anchor: &str, fill: &str, s: &str) -> String {
    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{fill}">{}</text>"##,
        svg::esc(s)
    )
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
    // the precision the whole page prints at: raised until no real frequency prints as zero and p and q print honestly
    // (round 3: at one decimal place p = 0.99 printed "1" beside q = 0.01)
    let dp = shown_dec(p);
    // (round 9) the p list of a six-generation run at six decimals is the widest line on the page: at Large it ran off
    // the right edge. The canvas widens to hold it (Noto Sans advances: digits 0.572 em, "." "," and space about 0.27,
    // a letter at most about 0.62).
    let plist: Vec<String> = g.iter().map(|x| fixed_ratio(x.pn, x.pd, dp)).collect();
    let em: f64 = format!("p in each generation  {}", plist.join(", "))
        .chars()
        .map(|c| if c.is_ascii_digit() { 0.572 } else if c == ' ' || c == ',' || c == '.' { 0.27 } else { 0.62 })
        .sum();
    let list_w = if sel && ans { 24.0 + em * ROW_FS * f + 12.0 } else { 0.0 };
    let w = (BAR_X + bar_w + 40.0).max(W).max(list_w);
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
    // the legend under the pool, so a segment too narrow for its own label still says what it is
    let legend_y = POOL_Y + POOL_H + 16.0;
    d.rect(BAR_X, legend_y - 11.0, 12.0, 12.0, allele_a, ink, 1.0);
    d.raw(&text_el(BAR_X + 18.0, legend_y, 12.0 * f, "start", ink, &format!("A  p = {}", given_str(p0))));
    d.rect(BAR_X + 150.0, legend_y - 11.0, 12.0, 12.0, "#ffffff", ink, 1.0);
    d.raw(&text_el(BAR_X + 168.0, legend_y, 12.0 * f, "start", ink, &format!("a  q = {}", given_str(q0))));

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
                // inside only when it fits: a real 0.0004 belongs in the readout, not squeezed into 0.2 px. (round 8
                // minor) The margin shrinks with the labels at Small: the bar and the label both scale by `f`, but a
                // fixed 8 px margin dropped a segment's label at Small that Medium carried ("AA 0.12" at p = 0.35).
                // It never grows past 8 px, so Large keeps every label it had.
                if tw <= wseg - 8.0 * f.min(1.0) {
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
        given_str(p0),
        given_str(q0),
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
    let c = counts_of(&first, n);
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
        let ps: Vec<String> = if ans { plist.clone() } else { (0..gens).map(|_| BLANK.to_string()).collect() };
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
        // (round 9) at one decimal a 1:2:1 cross cannot print all three within rounding AND keep AA = aa: the old
        // array-order tie-break printed 0.3 / 0.5 / 0.2 beside two equal bars. The two equal parts now stay equal, the
        // heterozygote is never a unit low, and the page never prints p = 0.5 at one decimal (`valid_dec`).
        let g = Gen::from_ratio(1, 2);
        let parts = printed_parts(&g, 1);
        assert_eq!(parts[0], parts[2], "AA and aa are equal and must print equal: {parts:?}");
        assert!(parts[1] >= 0.5, "the heterozygote is a unit low: {parts:?}");
        assert!((parts.iter().sum::<f64>() - 1.0).abs() < 1e-9, "{parts:?}");
        assert!(!valid_dec(&g, 0.5, 1), "p = 0.5 must not print at one decimal");
        assert_eq!(printed_parts(&g, 2), [0.25, 0.5, 0.25]);
        let g = Gen::from_ratio(34, 100);
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
            let c = counts_of(&g[0], n);
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
            let c = counts_of(&g[0], n as usize);
            assert!((parts.iter().sum::<f64>() - 1.0).abs() < 1e-9, "p = {p0}, n = {n}, dp = {dp}: {parts:?} do not sum to 1");
            // (round 8) the page is never coarser than the typed precision or the given p's own decimals. Where some
            // precision at or above that floor agrees exactly, the page must agree exactly; where none can (p = 0.5,
            // N = 10 at two decimals: 0.25 × 10 = 2.5 beside a count of 2 or 3; p = 0.333, N = 37: 0.445 × 37 = 16.5
            // beside the largest-remainder 17), the row is at most one off rather than the page going coarser.
            let floor = p.i("dec").clamp(1, 3).max(own_decimals(p0));
            let exact_possible = (floor..=6).any(|d| valid_dec(&g[0], p0, d) && count_error(&g[0], d, n as usize, &c) == 0);
            if !exact_possible {
                for i in 0..3 {
                    let implied = (parts[i] * n as f64).round() as i64;
                    assert!((implied - c[i] as i64).abs() <= 1, "p = {p0}, n = {n}, dp = {dp}: part {i} prints {} beside the count {}", parts[i], c[i]);
                }
                assert!(dp >= floor, "p = {p0}, n = {n}: the page went coarser than {floor}");
                continue;
            }
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

    /// The (fraction, count) of each genotype row in the readout, e.g. "AA  p\u{b2} = 0.25  (250 of 999)".
    fn readout_rows(t: &str) -> Vec<(f64, usize)> {
        let mut out = Vec::new();
        for name in ["AA", "Aa", "aa"] {
            let line = t.lines().find(|l| l.starts_with(&format!("{name}  ")) && l.contains('(')).unwrap_or("");
            let frac = line.split(" = ").nth(1).and_then(|r| r.split_whitespace().next()).and_then(|v| v.parse::<f64>().ok());
            let cnt = line.split('(').nth(1).and_then(|r| r.split_whitespace().next()).and_then(|v| v.parse::<usize>().ok());
            if let (Some(fv), Some(c)) = (frac, cnt) { out.push((fv, c)); }
        }
        out
    }

    #[test]
    fn a_printed_count_is_never_more_than_one_from_its_fraction_times_the_population() {
        // (round 7 blocker) counts allocate N by largest remainder, so a row can sit a whole unit off
        // round(exact × N), and for many (p, N) NO precision satisfies the strict `agrees`. The old fallback then
        // returned the typed (coarsest) precision: p = 0.5, N = 999, dec = 1 printed "AA 0.3 (250 of 999)" beside a
        // bar that was plainly a quarter, and p = 0.34, N = 9999 printed 0.12 / 0.45 / 0.43 for 1156 / 4487 / 4356.
        // The fallback now takes the valid precision that minimises the total contradiction: at worst one per row
        // (a frequency × N that is a half-integer can never agree exactly), and never a coarser set that a finer one
        // beats.
        let mut ps: Vec<f64> = (1..=99).map(|k| k as f64 / 100.0).collect();
        ps.extend([0.333, 0.1234, 0.6667]);
        let ns = [10usize, 37, 100, 500, 999, 1000, 1001, 1234, 4321, 5555, 9999, 10000];
        for &p0 in &ps {
            for &n in &ns {
                for dec in 1..=3 {
                    for sel in [false, true] {
                        let mut list = vec![f("p", p0), i("n", n as i64), ("dec", Value::I(dec)), b("counts", true)];
                        if sel { list.push(s("sel", "against the recessive phenotype")); }
                        let p = with(&list);
                        let g = generations(&p);
                        let dp = shown_dec(&p);
                        let parts = printed_parts(&g[0], dp);
                        let c = counts_of(&g[0], n);
                        // (a) every row is within one of fraction × N
                        for k in 0..3 {
                            let implied = (parts[k] * n as f64).round() as i64;
                            assert!(
                                (implied - c[k] as i64).abs() <= 1,
                                "p = {p0}, n = {n}, dec = {dec}, sel = {sel}: row {k} prints {} (\u{d7} {n} = {implied}) beside the count {}",
                                parts[k], c[k]
                            );
                        }
                        // (b) the printed fractions sum to 1 at the page precision
                        let sum: f64 = parts.iter().sum();
                        assert!((sum - 1.0).abs() < 1e-9, "p = {p0}, n = {n}, dec = {dec}, sel = {sel}: {parts:?} sum to {sum}");
                        // (d) no FINER valid precision has a strictly smaller total error
                        let err = |d: i64| -> i64 {
                            let pr = printed_parts(&g[0], d);
                            (0..3).map(|k| ((pr[k] * n as f64).round() as i64 - c[k] as i64).abs()).sum()
                        };
                        let e0 = err(dp);
                        for d2 in (dp + 1)..=6 {
                            if valid_dec(&g[0], p0, d2) {
                                assert!(
                                    e0 <= err(d2),
                                    "p = {p0}, n = {n}, dec = {dec}, sel = {sel}: chose d = {dp} (err {e0}) but the finer d = {d2} has err {}",
                                    err(d2)
                                );
                            }
                        }
                    }
                }
            }
        }
        // (c) the reported cases, as printed. p = 0.5, N = 999 is a half-integer: 0.25 / 0.50 / 0.25 is the honest set
        let t = texts(&render_of(&[f("p", 0.5), i("n", 999), ("dec", Value::I(1)), b("counts", true)]));
        assert!(t.contains("AA  p\u{b2} = 0.25  (250 of 999)"), "{t}");
        assert!(t.contains("Aa  2pq = 0.50  (499 of 999)"), "{t}");
        assert!(t.contains("aa  q\u{b2} = 0.25  (250 of 999)"), "{t}");
        assert!(!t.contains("AA  p\u{b2} = 0.3  "), "the coarsest set is still printed: {t}");
        for (p0, n, dec) in [(0.34, 9999, 2), (0.5, 9999, 1), (0.87, 500, 2), (0.94, 999, 2)] {
            let t = texts(&render_of(&[f("p", p0), i("n", n), ("dec", Value::I(dec)), b("counts", true)]));
            let rows = readout_rows(&t);
            assert_eq!(rows.len(), 3, "p = {p0}, n = {n}: {t}");
            let sum: f64 = rows.iter().map(|(fv, _)| fv).sum();
            assert!((sum - 1.0).abs() < 1e-9, "p = {p0}, n = {n}: the printed fractions sum to {sum}: {t}");
            for (fv, c) in rows {
                let implied = (fv * n as f64).round() as i64;
                assert!((implied - c as i64).abs() <= 1, "p = {p0}, n = {n}: {fv} \u{d7} {n} = {implied} beside {c}: {t}");
            }
        }
    }

    /// The decimals a printed number carries ("0.250" → 3).
    fn decimals(tok: &str) -> usize { tok.split('.').nth(1).map(|d| d.chars().take_while(char::is_ascii_digit).count()).unwrap_or(0) }

    #[test]
    fn the_page_is_never_coarser_than_typed_and_never_rewrites_the_given_p() {
        // (round 8 blocker) the strict pass tried COARSER precisions before the minimum-error rule ran, and a count tie
        // broken on float noise made a coarser one "agree": p = 0.5, N = 10 at two decimals printed 0.3 / 0.5 / 0.2 for a
        // 1 : 2 : 1 cross, and p = 0.21 printed "p = 0.2" with "p² = 0.1" (true 0.0441). The page precision is now never
        // below the typed decimals or p's own, the given p and q print exactly as typed, every count is within one of
        // its fraction × N, the fractions sum to 1, and every later generation prints at the same precision.
        let mut ps: Vec<f64> = (1..=99).map(|k| k as f64 / 100.0).collect();
        ps.extend([0.333, 0.1234, 0.6667, 0.005 + 0.5]);
        let ns = [10usize, 11, 12, 13, 17, 18, 20, 25, 34, 37, 50, 100, 101, 250, 500, 1000];
        for &p0 in &ps {
            for &n in &ns {
                for dec in 1..=3i64 {
                    for counts_on in [true, false] {
                        for sel in [false, true] {
                            let mut list = vec![f("p", p0), i("n", n as i64), ("dec", Value::I(dec)), b("counts", counts_on), b(SHOW_ANSWERS, true)];
                            if sel { list.push(s("sel", "against the recessive phenotype")); list.push(i("gens", 6)); }
                            let p = with(&list);
                            let dp = shown_dec(&p);
                            let ctx = format!("p = {p0}, n = {n}, dec = {dec}, counts = {counts_on}, sel = {sel}, dp = {dp}");
                            // (a) never coarser than typed, nor than p's own decimals
                            assert!(dp >= dec, "{ctx}: the page prints coarser than the typed {dec} decimals");
                            assert!(dp >= own_decimals(p0), "{ctx}: the page prints coarser than the given p");
                            let t = texts(&render_of(&list));
                            // (b) the givens print p and q exactly as typed
                            let line = t.lines().find(|l| l.starts_with("p = ")).unwrap_or("");
                            let after = |k: &str| line.split(&format!("{k} = ")).nth(1).and_then(|r| r.split(',').next()).and_then(|v| v.parse::<f64>().ok());
                            let (pp, qq) = (after("p").unwrap_or(-1.0), after("q").unwrap_or(-1.0));
                            assert!((pp - p0).abs() < 1e-9 && (qq - (1.0 - p0)).abs() < 1e-9, "{ctx}: the givens read {line:?}");
                            assert!(t.contains(&format!("A  p = {}", num(p0, 6))), "{ctx}: the legend rewrites p: {t}");
                            // (c) every row within one of fraction × N, and (d) the fractions sum to 1 at the page precision
                            if counts_on {
                                let rows = readout_rows(&t);
                                assert_eq!(rows.len(), 3, "{ctx}: {t}");
                                let sum: f64 = rows.iter().map(|r| r.0).sum();
                                assert!((sum - 1.0).abs() < 1e-9, "{ctx}: the fractions sum to {sum}: {t}");
                                assert_eq!(rows.iter().map(|r| r.1).sum::<usize>(), n, "{ctx}: the counts do not sum to N: {t}");
                                for (fv, c) in rows {
                                    let implied = (fv * n as f64).round() as i64;
                                    assert!((implied - c as i64).abs() <= 1, "{ctx}: {fv} \u{d7} {n} = {implied} beside {c}: {t}");
                                }
                            }
                            // every readout fraction and every bar label, in every generation, prints at the page precision
                            for l in t.lines().filter(|l| l.contains(" = ") && (l.starts_with("AA  ") || l.starts_with("Aa  ") || l.starts_with("aa  "))) {
                                let tok = l.split(" = ").nth(1).and_then(|r| r.split_whitespace().next()).unwrap_or("");
                                assert_eq!(decimals(tok), dp as usize, "{ctx}: {l:?}");
                            }
                            for l in bar_labels(&render_of(&list)) {
                                let tok = l.split_whitespace().nth(1).unwrap_or("");
                                assert_eq!(decimals(tok), dp as usize, "{ctx}: bar label {l:?}");
                                assert!(tok.parse::<f64>().map(|v| v > 0.0 && v < 1.0).unwrap_or(false), "{ctx}: bar label {l:?}");
                            }
                            if sel {
                                let pl = t.lines().find(|l| l.starts_with("p in each generation")).unwrap_or("");
                                for tok in pl.rsplit("  ").next().unwrap_or("").split(", ") {
                                    assert_eq!(decimals(tok), dp as usize, "{ctx}: {pl:?}");
                                }
                                assert!(pl.contains(&format!("generation  {}", fixed(p0, dp))), "{ctx}: the list does not start at the given p: {pl:?}");
                            }
                        }
                    }
                }
            }
        }
        // (e) the two reported cases, as printed
        let t = texts(&render_of(&[f("p", 0.5), i("n", 10), ("dec", Value::I(2)), b("counts", true), b(SHOW_ANSWERS, true)]));
        assert!(t.contains("p = 0.5, q = 0.5, N = 10"), "{t}");
        assert!(t.contains("AA  p\u{b2} = 0.25  ("), "{t}");
        assert!(t.contains("Aa  2pq = 0.50  (5 of 10)"), "{t}");
        assert!(t.contains("aa  q\u{b2} = 0.25  ("), "{t}");
        assert!(!t.contains("= 0.3  ") && !t.contains("= 0.2  "), "{t}");
        let t = texts(&render_of(&[f("p", 0.21), i("n", 10), ("dec", Value::I(2)), b("counts", true), b(SHOW_ANSWERS, true)]));
        assert!(t.contains("p = 0.21, q = 0.79, N = 10"), "{t}");
        assert!(t.contains("A  p = 0.21"), "{t}");
        // 0.21² = 0.0441: two decimals print it as 0.04 or (a remainder tie with 0.6241) 0.05, never 0.1
        assert!(t.contains("AA  p\u{b2} = 0.04  (") || t.contains("AA  p\u{b2} = 0.05  ("), "{t}");
        assert!(t.contains("Aa  2pq = 0.33  ("), "{t}");
        let t = texts(&render_of(&[f("p", 0.6), i("n", 10), ("dec", Value::I(2)), b("counts", true), b(SHOW_ANSWERS, true)]));
        assert!(t.contains("AA  p\u{b2} = 0.36  (") && t.contains("Aa  2pq = 0.48  (") && t.contains("aa  q\u{b2} = 0.16  ("), "{t}");
    }

    #[test]
    fn a_segment_labelled_at_medium_stays_labelled_at_small_and_large() {
        // (round 8 minor) the bar shrinks by the label factor at Small but the margin stayed a fixed 8 px, so "AA 0.12"
        // at p = 0.35 was drawn at Medium and gone at Small
        for k in 1..=99 {
            let p0 = k as f64 / 100.0;
            for extra in [vec![], vec![s("style", "Print")], vec![s("sel", "against the recessive phenotype"), i("gens", 6)], vec![b("counts", true), i("n", 10)], vec![("dec", Value::I(3))]] {
                let mut base = vec![f("p", p0), b(SHOW_ANSWERS, true)];
                base.extend(extra.clone());
                let at = |size: &str| { let mut l = base.clone(); l.push(s("label_size", size)); bar_labels(&render_of(&l)) };
                let (small, medium, large) = (at("Small"), at("Medium"), at("Large"));
                for l in &medium {
                    assert!(small.contains(l), "p = {p0} {extra:?}: {l} is drawn at Medium but lost at Small ({small:?})");
                    assert!(large.contains(l), "p = {p0} {extra:?}: {l} is drawn at Medium but lost at Large ({large:?})");
                }
            }
        }
        let small = bar_labels(&render_of(&[f("p", 0.35), s("label_size", "Small"), b(SHOW_ANSWERS, true)]));
        assert!(small.contains(&"AA 0.12".to_string()), "{small:?}");
    }

    #[test]
    fn the_selection_note_carries_one_precision_and_reads_as_a_rise() {
        // (round 8 minor) the note used `val`, which trims: "rises from 0.41 to 0.851 … and to 0.87", and at one decimal
        // "from 0.4 to 0.9 … and to 0.9 in the next", a rise to the same number
        for (p0, dec, n) in [(0.41, 2, 10), (0.41, 1, 10), (0.333, 1, 9999), (0.4, 1, 1000), (0.6, 1, 1000), (0.9, 1, 100), (0.99, 1, 1000), (0.05, 3, 37)] {
            for gens in 2..=6 {
                let p = with(&[f("p", p0), ("dec", Value::I(dec)), i("gens", gens), i("n", n), b("counts", true), s("sel", "against the recessive phenotype")]);
                let note = warnings(&p).join(" ");
                let nums: Vec<&str> = note
                    .split(|c: char| c == ' ' || c == ',')
                    .filter(|w| w.starts_with("0.") || w.starts_with("1."))
                    .map(|w| w.trim_end_matches('.'))
                    .collect();
                assert_eq!(nums.len(), 3, "p = {p0}, dec = {dec}, gens = {gens}: {note}");
                let d0 = decimals(nums[0]);
                assert!(nums.iter().all(|x| decimals(x) == d0), "p = {p0}, dec = {dec}, gens = {gens}: mixed precisions in {note}");
                assert!(d0 as i64 >= shown_dec(&p), "p = {p0}, dec = {dec}: the note is coarser than the page: {note}");
                let v: Vec<f64> = nums.iter().map(|x| x.parse().unwrap()).collect();
                assert!(v[0] < v[1] && v[1] < v[2], "p = {p0}, dec = {dec}, gens = {gens}: the note does not read as a rise: {note}");
                // and it starts at the given p, as typed
                assert!((v[0] - p0).abs() < 1e-9, "p = {p0}: the note rewrites p: {note}");
            }
        }
    }

    /// Parses a printed decimal ("0.015241") into whole units of 10^-d.
    fn units_of(tok: &str) -> (i128, i64) {
        let d = decimals(tok) as i64;
        let digits: String = tok.chars().filter(|c| c.is_ascii_digit()).collect();
        (digits.parse::<i128>().unwrap_or(-1), d)
    }

    /// n / den at d decimals as floor and ceiling units, and the half-up rounding.
    fn bracket(n: i128, den: i128, d: i64) -> (i128, i128, i128) {
        let s = 10i128.pow(d as u32);
        let fl = n * s / den;
        let ce = if (n * s) % den == 0 { fl } else { fl + 1 };
        (fl, ce, (2 * n * s + den) / (2 * den))
    }

    #[test]
    fn every_printed_number_is_derived_from_the_printed_p() {
        // (round 9 blocker) p = 0.123456 printed "p = 0.1235" (the shared formatter stops at four decimals) beside
        // "AA p² = 0.015241", which is 0.123456², and a p list starting 0.123456. p is now rounded once, at read time, to
        // six decimals, and every number on the page is computed from that one printed value.
        let ps = [
            0.5, 0.25, 0.6, 0.123, 0.1234, 0.12345, 0.123456, 0.333333, 0.666667, 0.987654, 0.010001, 0.98765, 0.0001, 0.9999,
            1.0, 0.0, 0.1234567, 0.9876543, 0.07, 0.93, 0.21, 0.5000004, 0.4999996,
        ];
        for &typed in &ps {
            for &n in &[10i64, 37, 1000, 9999] {
                for dec in 1..=3i64 {
                    for counts_on in [false, true] {
                        for sel in [false, true] {
                            let mut list = vec![f("p", typed), i("n", n), ("dec", Value::I(dec)), b("counts", counts_on), b(SHOW_ANSWERS, true)];
                            if sel { list.push(s("sel", "against the recessive phenotype")); list.push(i("gens", 6)); }
                            let t = texts(&render_of(&list));
                            let ctx = format!("typed p = {typed}, n = {n}, dec = {dec}, counts = {counts_on}, sel = {sel}");
                            // the given p, as printed, and its exact integer over 10^6
                            let line = t.lines().find(|l| l.starts_with("p = ")).unwrap_or("");
                            let ptok = line.trim_start_matches("p = ").split(',').next().unwrap_or("");
                            let qtok = line.split("q = ").nth(1).and_then(|r| r.split(',').next()).unwrap_or("");
                            let (pu, pd) = units_of(ptok);
                            let (qu, qd) = units_of(qtok);
                            assert!(pd <= 6 && qd <= 6, "{ctx}: {line}");
                            let k = pu * 10i128.pow((6 - pd) as u32);
                            let kq = qu * 10i128.pow((6 - qd) as u32);
                            assert_eq!(k + kq, 1_000_000, "{ctx}: p + q is not 1: {line}");
                            // the given p is what was typed, clamped, to the documented six decimals
                            let want = (typed.clamp(0.01, 0.99) * 1e6).round() as i128;
                            assert_eq!(k, want, "{ctx}: the givens rewrite p: {line}");
                            assert!(t.contains(&format!("A  p = {ptok}\n")), "{ctx}: the legend and the givens disagree: {t}");
                            assert!(t.contains(&format!("a  q = {qtok}\n")), "{ctx}: the legend and the givens disagree: {t}");
                            // the readout: every fraction is p², 2pq, q² of the PRINTED p, rounded to the page precision
                            // (a floor or a ceiling, and the plain rounding whenever the plain roundings sum to 1)
                            let den = 1_000_000i128 * 1_000_000;
                            let exact = [k * k, 2 * k * (1_000_000 - k), (1_000_000 - k) * (1_000_000 - k)];
                            let toks: Vec<&str> = ["AA  p\u{b2} = ", "Aa  2pq = ", "aa  q\u{b2} = "]
                                .iter()
                                .map(|pre| t.lines().find(|l| l.starts_with(pre)).and_then(|l| l[pre.len()..].split_whitespace().next()).unwrap_or(""))
                                .collect();
                            let dp = decimals(toks[0]) as i64;
                            assert!(dp >= pd, "{ctx}: the page ({dp} decimals) is coarser than the given p {ptok}");
                            let mut us = [0i128; 3];
                            let mut plain = [0i128; 3];
                            for j in 0..3 {
                                let (u, d) = units_of(toks[j]);
                                assert_eq!(d, dp, "{ctx}: mixed precisions {toks:?}");
                                let (fl, ce, r) = bracket(exact[j], den, dp);
                                assert!(u == fl || u == ce, "{ctx}: {} is not p\u{b2}/2pq/q\u{b2} of the printed p = {ptok}: {toks:?}", toks[j]);
                                us[j] = u;
                                plain[j] = r;
                            }
                            assert_eq!(us.iter().sum::<i128>(), 10i128.pow(dp as u32), "{ctx}: {toks:?}");
                            if plain.iter().sum::<i128>() == 10i128.pow(dp as u32) {
                                assert_eq!(us, plain, "{ctx}: {toks:?} is not the plain rounding of p = {ptok}");
                            }
                            for a in 0..3 {
                                for c in a + 1..3 {
                                    if exact[a] == exact[c] { assert_eq!(us[a], us[c], "{ctx}: equal genotypes print unequal: {toks:?}"); }
                                }
                            }
                            // the p list starts at the printed p and each entry is 1 / (2 - p) of the one before, exactly
                            if sel {
                                let pl = t.lines().find(|l| l.starts_with("p in each generation")).unwrap_or("");
                                let (mut a, mut bb) = (k, 1_000_000i128);
                                for tok in pl.rsplit("  ").next().unwrap_or("").split(", ") {
                                    let (u, d) = units_of(tok);
                                    assert_eq!(d, dp, "{ctx}: {pl}");
                                    assert_eq!(u, bracket(a, bb, dp).2, "{ctx}: {tok} is not p in its generation: {pl}");
                                    (a, bb) = (bb, 2 * bb - a);
                                }
                            }
                        }
                    }
                }
            }
        }
        // the reported case, as printed
        let t = texts(&render_of(&[f("p", 0.123456), i("n", 1000), ("dec", Value::I(2)), b("counts", true), s("sel", "against the recessive phenotype"), i("gens", 4), b(SHOW_ANSWERS, true)]));
        assert!(t.contains("p = 0.123456, q = 0.876544, N = 1000"), "{t}");
        assert!(t.contains("A  p = 0.123456"), "{t}");
        // 0.123456² = 0.015241384 and 0.876544² = 0.768329384 tie on their remainder; the unit the set needs to sum to 1
        // goes to the smaller one
        assert!(t.contains("AA  p\u{b2} = 0.015242  (15 of 1000)") && t.contains("aa  q\u{b2} = 0.768329  (768 of 1000)"), "{t}");
        assert!(t.contains("p in each generation  0.123456, "), "{t}");
        let t = texts(&render_of(&[f("p", 0.12345), ("dec", Value::I(1)), b(SHOW_ANSWERS, true)]));
        assert!(t.contains("p = 0.12345, q = 0.87655"), "{t}");
        // more than six decimals: rounded once, and the editor says so
        let p = with(&[f("p", 0.1234567)]);
        assert!(warnings(&p).iter().any(|w| w.contains("rounded to six decimal places") && w.contains("0.123457")), "{:?}", warnings(&p));
        assert!(!warnings(&with(&[f("p", 0.123456)])).iter().any(|w| w.contains("rounded")));
    }

    #[test]
    fn a_six_decimal_p_list_fits_the_canvas() {
        // (round 9, found while fixing the blocker) six generations at six decimals ran off the right edge at Large
        for typed in [0.123456, 0.888888, 0.010001, 0.98765, 0.5] {
            for size in ["Small", "Medium", "Large"] {
                for gens in 2..=6 {
                    let svg = render_of(&[f("p", typed), i("gens", gens), s("sel", "against the recessive phenotype"), s("label_size", size), b(SHOW_ANSWERS, true)]);
                    let w: f64 = svg.split("width=\"").nth(1).and_then(|r| r.split('"').next()).and_then(|v| v.parse().ok()).unwrap();
                    let t = texts(&svg);
                    let line = t.lines().find(|l| l.starts_with("p in each generation")).unwrap();
                    let fs = ROW_FS * svg::label_factor(size);
                    // measured Noto Sans advances, a little generous for the letters
                    let em: f64 = line.chars().map(|c| if c.is_ascii_digit() { 0.572 } else if c == ' ' || c == ',' || c == '.' { 0.27 } else { 0.62 }).sum();
                    assert!(24.0 + em * fs <= w, "p = {typed}, {size}, gens = {gens}: {line:?} is {} px wide on a {w} px canvas", 24.0 + em * fs);
                }
            }
        }
    }

    #[test]
    fn equal_frequencies_print_equal_and_a_mirror_population_prints_as_its_mirror() {
        // (round 9) remainder ties were settled by f64 noise or by array order: p = 0.07 at one decimal printed at two
        // decimals while its mirror p = 0.93 printed at three; p = 0.5, N = 10 at one decimal printed "AA 0.3, aa 0.2"
        // beside two equal bars. The rounding is now exact, ties go to the smaller true value, and equal true values
        // never print unequal.
        for k in 1..=99i64 {
            let (p0, m0) = (k as f64 / 100.0, (100 - k) as f64 / 100.0);
            for &n in &[10i64, 11, 37, 100, 999, 1000, 9999] {
                for dec in 1..=3i64 {
                    for counts_on in [false, true] {
                        let mk = |pv: f64, sel: bool| {
                            let mut l = vec![f("p", pv), i("n", n), ("dec", Value::I(dec)), b("counts", counts_on), b(SHOW_ANSWERS, true)];
                            if sel { l.push(s("sel", "against the recessive phenotype")); l.push(i("gens", 6)); }
                            l
                        };
                        let ctx = format!("p = {p0} / {m0}, n = {n}, dec = {dec}, counts = {counts_on}");
                        let (a, bm) = (with(&mk(p0, false)), with(&mk(m0, false)));
                        let (da, db) = (shown_dec(&a), shown_dec(&bm));
                        assert_eq!(da, db, "{ctx}: a population and its mirror print at different precisions");
                        let (ga, gb) = (generations(&a)[0], generations(&bm)[0]);
                        let (pa, pb) = (printed_parts(&ga, da), printed_parts(&gb, db));
                        assert_eq!([pa[0], pa[1], pa[2]], [pb[2], pb[1], pb[0]], "{ctx}: the mirror does not print as the mirror");
                        if k != 50 {
                            let (ca, cb) = (counts_of(&ga, n as usize), counts_of(&gb, n as usize));
                            assert_eq!([ca[0], ca[1], ca[2]], [cb[2], cb[1], cb[0]], "{ctx}: the mirror's counts are not the mirror");
                        }
                        // equal true values print equal, in every generation of a selection run too
                        for sel in [false, true] {
                            let pp = with(&mk(p0, sel));
                            let dp = shown_dec(&pp);
                            for g in generations(&pp) {
                                let (num, _) = g.exact();
                                let pr = printed_units(&g, dp);
                                for x in 0..3 {
                                    for y in x + 1..3 {
                                        if num[x] == num[y] { assert_eq!(pr[x], pr[y], "{ctx}, sel = {sel}: {g:?} prints {pr:?}"); }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        // the reported cases, as printed
        let t = texts(&render_of(&[f("p", 0.5), i("n", 10), ("dec", Value::I(1)), b("counts", true), s("sel", "against the recessive phenotype"), i("gens", 6), b(SHOW_ANSWERS, true)]));
        assert!(t.contains("AA  p\u{b2} = 0.25  (") && t.contains("aa  q\u{b2} = 0.25  ("), "{t}");
        assert!(t.contains("Aa  2pq = 0.50  (5 of 10)"), "{t}");
        // generation 2 has p = 2/3, so AA = Aa = 4/9: two equal bars, two equal labels
        let labels = bar_labels(&render_of(&[f("p", 0.5), i("n", 10), ("dec", Value::I(1)), b("counts", true), s("sel", "against the recessive phenotype"), i("gens", 6), b(SHOW_ANSWERS, true)]));
        assert!(labels.contains(&"AA 0.44".to_string()) && labels.contains(&"Aa 0.44".to_string()), "{labels:?}");
        let a = texts(&render_of(&[f("p", 0.07), ("dec", Value::I(1)), b(SHOW_ANSWERS, true)]));
        let z = texts(&render_of(&[f("p", 0.93), ("dec", Value::I(1)), b(SHOW_ANSWERS, true)]));
        assert!(a.contains("AA  p\u{b2} = 0.01") && a.contains("aa  q\u{b2} = 0.86"), "{a}");
        assert!(z.contains("AA  p\u{b2} = 0.86") && z.contains("aa  q\u{b2} = 0.01"), "{z}");
    }

    #[test]
    fn the_selection_note_and_the_p_list_carry_one_precision_and_both_rise() {
        // (round 9 minor) the note raised its own precision ("from 0.50 to 0.86 … and to 0.88") over a page list that
        // read "0.5, 0.7, 0.8, 0.8, 0.8, 0.9". One precision now serves both, raised until the list rises.
        for k in 1..=99i64 {
            let p0 = k as f64 / 100.0;
            for dec in 1..=3i64 {
                for gens in 2..=6i64 {
                    for (n, counts_on) in [(10i64, true), (1000, true), (1000, false)] {
                        let list = vec![f("p", p0), i("n", n), ("dec", Value::I(dec)), i("gens", gens), b("counts", counts_on), s("sel", "against the recessive phenotype"), b(SHOW_ANSWERS, true)];
                        let p = with(&list);
                        let ctx = format!("p = {p0}, dec = {dec}, gens = {gens}, n = {n}, counts = {counts_on}");
                        let t = texts(&render_of(&list));
                        let pl = t.lines().find(|l| l.starts_with("p in each generation")).unwrap_or("");
                        let toks: Vec<&str> = pl.rsplit("  ").next().unwrap_or("").split(", ").collect();
                        let vals: Vec<f64> = toks.iter().map(|x| x.parse().unwrap()).collect();
                        assert!(vals.windows(2).all(|w| w[1] > w[0]), "{ctx}: the p list does not rise: {pl}");
                        let note = warnings(&p).join(" ");
                        let nums: Vec<&str> = note.split(|c: char| c == ' ' || c == ',').filter(|w| w.starts_with("0.") || w.starts_with("1.")).map(|w| w.trim_end_matches('.')).collect();
                        assert_eq!(nums.len(), 3, "{ctx}: {note}");
                        let dp = shown_dec(&p) as usize;
                        for x in nums.iter().chain(toks.iter()) { assert_eq!(decimals(x), dp, "{ctx}: {x} in {note} / {pl}"); }
                        assert_eq!(nums[0], toks[0], "{ctx}: the note and the list start differently: {note} / {pl}");
                        assert_eq!(nums[1], *toks.last().unwrap(), "{ctx}: the note and the list end differently: {note} / {pl}");
                        assert!(nums[2].parse::<f64>().unwrap() > nums[1].parse::<f64>().unwrap(), "{ctx}: {note}");
                    }
                }
            }
        }
        let p = with(&[f("p", 0.5), i("n", 10), ("dec", Value::I(1)), b("counts", true), s("sel", "against the recessive phenotype"), i("gens", 6)]);
        let note = warnings(&p).join(" ");
        assert!(note.contains("from 0.50 to 0.86") && note.contains("to 0.88 in the next"), "{note}");
        let t = texts(&render_of(&[f("p", 0.5), i("n", 10), ("dec", Value::I(1)), b("counts", true), s("sel", "against the recessive phenotype"), i("gens", 6), b(SHOW_ANSWERS, true)]));
        assert!(t.contains("p in each generation  0.50, 0.67, 0.75, 0.80, 0.83, 0.86"), "{t}");
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

