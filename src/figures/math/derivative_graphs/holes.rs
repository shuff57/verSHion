//! Local analytic continuations of the typed expression, never finite differences of its noisy derivatives.
//! Formal Taylor arithmetic cancels only leading coefficients that are exactly zero. A pole, log at zero,
//! cusp, essential singularity or jump cannot pass this construction. The typed domain still excludes the hole.
use super::*;

#[derive(Clone)]
pub(super) struct Local { pub pi: usize, pub x: f64, pub radius: f64, c: Vec<f64> }
impl Local {
    pub fn at(&self, k: usize, x: f64) -> f64 {
        self.c.iter().enumerate().skip(k).rev().fold(0.0, |v, (i, c)| {
            v * (x - self.x) + c * (0..k).map(|j| (i - j) as f64).product::<f64>()
        })
    }
}

type Series = Vec<f64>;
fn constant(v: f64, n: usize) -> Series { let mut c = vec![0.0; n]; c[0] = v; c }
fn add(a: Series, b: Series, sign: f64) -> Series { a.into_iter().zip(b).map(|(a, b)| a + sign * b).collect() }
fn scale(mut a: Series, v: f64) -> Series { for c in &mut a { *c *= v; } a }
fn mul(a: &[f64], b: &[f64]) -> Series {
    (0..a.len().min(b.len())).map(|i| (0..=i).map(|j| a[j] * b[i - j]).sum()).collect()
}
fn div(a: &[f64], b: &[f64]) -> Option<Series> {
    let z = b.iter().position(|v| *v != 0.0)?;
    // No small-number threshold: a genuine nonzero constant divided by x is a pole, however tiny.
    if a.len() <= z || a[..z].iter().any(|v| *v != 0.0) { return None; }
    let (a, b) = (&a[z..], &b[z..]);
    let mut c = vec![0.0; a.len().min(b.len())];
    for i in 0..c.len() { c[i] = (a[i] - (1..=i).map(|j| b[j] * c[i - j]).sum::<f64>()) / b[0]; }
    Some(c)
}
fn exp(a: &[f64]) -> Series {
    let mut c = constant(a[0].exp(), a.len());
    for i in 1..c.len() { c[i] = (1..=i).map(|j| j as f64 * a[j] * c[i - j]).sum::<f64>() / i as f64; }
    c
}
fn sincos(a: &[f64], s0: f64, c0: f64) -> (Series, Series) {
    let (mut s, mut c) = (constant(s0, a.len()), constant(c0, a.len()));
    for i in 1..a.len() {
        s[i] = (1..=i).map(|j| j as f64 * a[j] * c[i - j]).sum::<f64>() / i as f64;
        c[i] = -(1..=i).map(|j| j as f64 * a[j] * s[i - j]).sum::<f64>() / i as f64;
    }
    (s, c)
}
fn ln(a: &[f64]) -> Option<Series> {
    if a[0] <= 0.0 { return None; }
    let d: Series = a.iter().enumerate().skip(1).map(|(i, c)| i as f64 * c).collect();
    let q = div(&d, a)?;
    let mut c = vec![a[0].ln()];
    c.extend(q.iter().enumerate().map(|(i, v)| v / (i + 1) as f64));
    Some(c)
}
fn power(a: &[f64], k: i32) -> Option<Series> {
    if k.abs() > 32 { return None; }
    let mut c = constant(1.0, a.len());
    for _ in 0..k.abs() { c = mul(&c, a); }
    if k < 0 { div(&constant(1.0, a.len()), &c) } else { Some(c) }
}
fn series(e: &Ex, x: &XV) -> Option<Series> {
    use sym::{Ex::*, F};
    const N: usize = 22;
    let r = |a: &Ex| series(a, x);
    let mut c = match e {
        Num(v) => constant(*v, N), Pi => constant(PI, N), E => constant(std::f64::consts::E, N),
        X => { let mut c = constant(x.f(), N); c[1] = 1.0; c }
        Neg(a) => scale(r(a)?, -1.0),
        Add(a, b) => add(r(a)?, r(b)?, 1.0), Sub(a, b) => add(r(a)?, r(b)?, -1.0),
        Mul(a, b) => mul(&r(a)?, &r(b)?), Div(a, b) => div(&r(a)?, &r(b)?)?,
        Pow(a, b) => {
            if let Some(k) = sym::konst(b).filter(|k| k.fract() == 0.0 && k.abs() <= 32.0) { power(&r(a)?, k as i32)? }
            else { exp(&mul(&r(b)?, &ln(&r(a)?)?)) }
        }
        LogB(b, a) => div(&ln(&r(a)?)?, &ln(&r(b)?)?)?,
        Fun(f, a) => {
            let u = r(a)?;
            match f {
                F::Exp => exp(&u), F::Ln => ln(&u)?, F::Log => scale(ln(&u)?, 1.0 / std::f64::consts::LN_10),
                F::Sqrt => exp(&scale(ln(&u)?, 0.5)),
                F::Abs if u[0] != 0.0 => scale(u.clone(), u[0].signum()),
                F::Sin | F::Cos | F::Tan | F::Sec | F::Csc | F::Cot => {
                    // Seed the recurrence with proved constant values, before computing *any* coefficient.
                    // sin(pi)'s libm residual would otherwise poison the linear term of 1+cos(pi+t),
                    // falsely making (1+cos x)/(x−pi)² a pole instead of a removable hole.
                    let s0 = exact::ev(&Fun(F::Sin, a.clone()), x).map(|v| v.f()).unwrap_or_else(|| u[0].sin());
                    let c0 = exact::ev(&Fun(F::Cos, a.clone()), x).map(|v| v.f()).unwrap_or_else(|| u[0].cos());
                    let (s, c) = sincos(&u, s0, c0);
                    match f {
                        F::Sin => s, F::Cos => c, F::Tan => div(&s, &c)?, F::Cot => div(&c, &s)?,
                        F::Sec => div(&constant(1.0, c.len()), &c)?, _ => div(&constant(1.0, s.len()), &s)?,
                    }
                }
                F::Asin | F::Acos | F::Atan | F::Asec | F::Acsc | F::Acot => {
                    let v = sym::ev(e, x.f());
                    if !v.is_finite() { return None; }
                    // Integrate the analytic derivative series, with the original function's constant term.
                    let d = series(&sym::deriv(e), x)?;
                    let mut c = vec![v];
                    c.extend(d.iter().take(N - 1).enumerate().map(|(i, v)| v / (i + 1) as f64));
                    c
                }
                _ => return None,
            }
        }
    };
    if let Some(v) = exact::ev(e, x) { c[0] = v.f(); }
    (!c.is_empty() && c.iter().all(|v| v.is_finite())).then_some(c)
}

pub(super) fn find(ps: &[Piece], win: (f64, f64)) -> Vec<Local> {
    let mut out = vec![];
    for (pi, p) in ps.iter().enumerate() {
        // Rational cancellation already has its exact reduced expressions and marks.
        if p.rat.is_some() { continue; }
        let xs = exact::special_points(&p.e, win.0 - 1.0, win.1 + 1.0);
        for xe in &xs {
            let x = xe.f();
            if !in_dom(&p.dom, x) || !exact::undefined_at(&p.e, xe) { continue; }
            let Some(c) = series(&p.e, xe).filter(|c| c.len() >= 12) else { continue };
            let mut local = Local { pi, x, radius: 0.125, c };
            for other in &xs { if other != xe { local.radius = local.radius.min(0.25 * (other.f() - x).abs()); } }
            // The last half of the available series must be negligible, also after three derivatives. Shrink for
            // rapidly varying functions; never bridge another special point. Verify both joins against the original
            // derivative expressions wherever their rounding bound permits a comparison.
            for _ in 0..30 {
                let tail = (0..=3).map(|k| {
                    local.c.iter().enumerate().skip(local.c.len() / 2).map(|(i, c)| {
                        c.abs() * (0..k).map(|j| (i - j) as f64).product::<f64>() * local.radius.powi((i - k) as i32)
                    }).sum::<f64>()
                }).fold(0.0, f64::max);
                if tail < 1e-13 { break; }
                local.radius *= 0.5;
            }
            let joins_ok = [-1.0, 1.0].iter().all(|s| (0..=3).all(|k| {
                let t = x + s * local.radius;
                let (v, err) = sym::ev_err(if k == 0 { &p.e } else { &p.d[k - 1] }, t);
                let w = local.at(k, t);
                v.is_finite() && err.is_finite() && (v - w).abs() <= 32.0 * err + 1e-10 * (1.0 + w.abs())
            }));
            if joins_ok && local.radius > 1e-8 * (1.0 + x.abs()) { out.push(local); }
        }
    }
    out
}

impl Model {
    pub(super) fn local_value(&self, pi: usize, k: usize, x: f64) -> Option<f64> {
        self.locals.iter().find(|l| l.pi == pi && (x - l.x).abs() <= l.radius).map(|l| l.at(k, x))
    }
}

/// Restrict the original row before the grid evaluates it. Keep one row per typed piece (including all eight
/// piecewise rows): no extra slots, no silent truncation. The small omitted intervals are drawn analytically below.
pub(super) fn domain(m: &Model, pi: usize) -> Option<String> {
    let pc = &m.pieces[pi];
    let mut ls: Vec<_> = m.locals.iter().filter(|l| l.pi == pi).collect();
    if ls.is_empty() { return pc.dom_text.clone(); }
    ls.sort_by(|a, b| a.x.total_cmp(&b.x));
    let all = [Interval { lo: f64::NEG_INFINITY, hi: f64::INFINITY, lo_c: false, hi_c: false }];
    let dom = if pc.dom.is_empty() { &all[..] } else { &pc.dom };
    let mut parts = vec![];
    for iv in dom {
        let (mut lo, mut closed) = (iv.lo, iv.lo_c);
        for l in &ls {
            let (a, b) = (l.x - l.radius, l.x + l.radius);
            if b <= lo || a >= iv.hi { continue; }
            if a > lo && a < iv.hi { parts.push(interval(lo, closed, a, true)); }
            if b > lo { lo = b; closed = true; }
        }
        if lo < iv.hi { parts.push(interval(lo, closed, iv.hi, iv.hi_c)); }
    }
    // An impossible domain if the window/piece lies entirely inside a continuation.
    Some(if parts.is_empty() { "x < 0 and x > 1".into() } else { parts.join(" or ") })
}
fn interval(a: f64, ac: bool, b: f64, bc: bool) -> String {
    let mut p = vec![];
    if a.is_finite() { p.push(format!("x {} {a:?}", if ac { ">=" } else { ">" })); }
    if b.is_finite() { p.push(format!("x {} {b:?}", if bc { "<=" } else { "<" })); }
    p.join(" and ")
}

/// Draw only the analytic neighbourhoods, in the panel's own coordinates and clip path. All other curves,
/// including vertical asymptotes, still go through the coordinate grid unchanged.
pub(super) fn draw(m: &Model, p: &Params, fis: &[usize], g: &GridL) -> String {
    let mut svg = String::new();
    let print = p.s("style") == "Print";
    let one = p.s("layout") == LAYOUTS[1];
    let colors = if print { ["#000000", "#444444", "#1c1c1c"] } else { COL };
    for &fi in fis {
        if !shows(p, fi) { continue; }
        let k = match m.mode { Mode::F => fi, Mode::Fp { .. } if fi > 0 => fi - 1, _ => continue };
        // Match the coordinate grid's Medium line in Print, including the chosen grid weight.
        let gw = match p.s("grid") { "Light" | "None" => 1.0, "Bold" => 1.8, _ => 1.4 };
        let lw = if print { 1.8 * gw * [1.0, 1.12, 1.4][fi] } else { 2.6 };
        let dash = dash_of(line_of(p, fi, one));
        let da = if dash.is_empty() { String::new() } else { format!(r#" stroke-dasharray="{dash}""#) };
        for l in &m.locals {
            let lo = (l.x - l.radius).max(g.x0);
            let hi = (l.x + l.radius).min(g.x1);
            if hi <= lo { continue; }
            let all = [Interval { lo, hi, lo_c: true, hi_c: true }];
            let dom = if m.pieces[l.pi].dom.is_empty() { &all[..] } else { &m.pieces[l.pi].dom };
            for iv in dom { for (a, b) in [(lo.max(iv.lo), hi.min(l.x).min(iv.hi)), (lo.max(l.x).max(iv.lo), hi.min(iv.hi))] {
                if b <= a { continue; }
                // Subpixel steps, bounded independently of the window and number of holes.
                let n = (((b - a) * g.ux * 4.0).ceil() as usize).clamp(8, 2048);
                let pts = (0..=n).filter_map(|i| {
                    let x = a + (b - a) * i as f64 / n as f64;
                    in_dom(&m.pieces[l.pi].dom, x).then(|| format!("{:.2},{:.2}", g.ml + (x - g.x0) * g.ux, g.mt + (g.y1 - l.at(k, x)) * g.uy))
                }).collect::<Vec<_>>().join(" ");
                svg.push_str(&format!(r#"<g clip-path="url(#plot)"><polyline points="{pts}" fill="none" stroke="{}" stroke-width="{lw}" stroke-linejoin="round"{da} data-curve="continuation"/></g>"#, colors[fi]));
            } }
        }
    }
    svg
}

/// Keep continuations in the grid's curve layer: late axis digits must still be drawn over the curves.
pub(super) fn insert(svg: &mut String, local: &str) {
    if local.is_empty() { return; }
    if let Some(start) = svg.find(r#"<clipPath id="plot">"#) {
        if let Some(end) = svg[start..].find("</clipPath>") { svg.insert_str(start + end + "</clipPath>".len(), local); }
    }
}
