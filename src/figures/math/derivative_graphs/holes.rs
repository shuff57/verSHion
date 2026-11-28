//! Local analytic continuations of the typed expression, never finite differences of its noisy derivatives.
//! Formal Taylor arithmetic cancels only leading coefficients that are exactly zero. A pole, log at zero,
//! cusp, essential singularity or jump cannot pass this construction. The typed domain still excludes the hole.
use super::*;

#[derive(Clone)]
pub(super) struct Local { pub pi: usize, pub x: f64, pub radius: f64, pub xe: Option<XV>, c: Vec<f64> }
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
fn fractional_power(a: &[f64], k: f64, value: f64) -> Option<Series> {
    if a[0] == 0.0 || !value.is_finite() { return None; }
    let mut c = constant(value, a.len());
    for n in 1..c.len() {
        c[n] = (1..=n).map(|j| ((k+1.0)*j as f64-n as f64)*a[j]*c[n-j]).sum::<f64>()/(n as f64*a[0]);
    }
    Some(c)
}
pub(super) fn series(e: &Ex, x: &XV, zero: Option<&Ex>) -> Option<Series> {
    use sym::{Ex::*, F};
    const N: usize = 22;
    if !sym::has_x(e) {
        let v = exact::ev(e, x).map(|v| v.f()).unwrap_or_else(|| sym::ev(e, x.f()));
        return v.is_finite().then(|| constant(v, N));
    }
    let r = |a: &Ex| series(a, x, zero);
    let mut c = match e {
        Num(v) => constant(*v, N), Pi => constant(PI, N), E => constant(std::f64::consts::E, N),
        X => { let mut c = constant(x.f(), N); c[1] = 1.0; c }
        Neg(a) => scale(r(a)?, -1.0),
        Add(a, b) => add(r(a)?, r(b)?, 1.0), Sub(a, b) => add(r(a)?, r(b)?, -1.0),
        Mul(a, b) => mul(&r(a)?, &r(b)?), Div(a, b) => div(&r(a)?, &r(b)?)?,
        Pow(a, b) => {
            if let Some(k) = sym::konst(b).filter(|k| k.fract() == 0.0 && k.abs() <= 32.0) { power(&r(a)?, k as i32)? }
            else if let Some(k) = sym::konst(b) {
                fractional_power(&r(a)?, k, exact::ev(e,x).map(|v|v.f()).unwrap_or_else(||sym::ev(e,x.f())))?
            } else { exp(&mul(&r(b)?, &ln(&r(a)?)?)) }
        }
        LogB(b, a) => div(&ln(&r(a)?)?, &ln(&r(b)?)?)?,
        Fun(f, a) => {
            let u = r(a)?;
            match f {
                F::Exp => exp(&u), F::Ln => ln(&u)?, F::Log => scale(ln(&u)?, 1.0 / std::f64::consts::LN_10),
                F::Sqrt => fractional_power(&u, 0.5, exact::ev(e,x).map(|v|v.f()).unwrap_or_else(||u[0].sqrt()))?,
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
                    let d = series(&sym::deriv(e), x, zero)?;
                    let mut c = vec![v];
                    c.extend(d.iter().take(N - 1).enumerate().map(|(i, v)| v / (i + 1) as f64));
                    c
                }
                _ => return None,
            }
        }
    };
    if zero.is_none() { if let Some(v) = exact::ev(e, x) { c[0] = v.f(); } }
    if zero == Some(e) { c[0] = 0.0; }
    (!c.is_empty() && c.iter().all(|v| v.is_finite())).then_some(c)
}

/// Put outer arithmetic over a common denominator for Taylor construction only. This makes sin(x)*x⁻¹
/// and sin(x)/x the same local calculation, while retaining the *typed* expression for its domain.
fn fraction(e: &Ex) -> (Ex,Ex) {
    use sym::{Ex::*, F};
    match e {
        Neg(a) => { let (n,d)=fraction(a); (sym::neg(n),d) }
        Add(a,b) | Sub(a,b) => {
            let (an,ad)=fraction(a); let (bn,bd)=fraction(b);
            let left=sym::mul(an,bd.clone()); let right=sym::mul(bn,ad.clone());
            (if matches!(e,Add(..)) {sym::add(left,right)} else {sym::sub(left,right)},sym::mul(ad,bd))
        }
        Mul(a,b) => { let (an,ad)=fraction(a); let (bn,bd)=fraction(b); (sym::mul(an,bn),sym::mul(ad,bd)) }
        Div(a,b) => { let (an,ad)=fraction(a); let (bn,bd)=fraction(b); (sym::mul(an,bd),sym::mul(ad,bn)) }
        Pow(a,b) if sym::konst(b).is_some_and(|k|k<0.0 && k.fract()==0.0 && k>=-32.0) => (Num(1.0),sym::pow((**a).clone(),sym::neg((**b).clone()))),
        // (the reciprocal trig functions as quotients, and their whole powers: x cot x, x² csc² x cancel as x cos x / sin x)
        Pow(a,b) if sym::konst(b).is_some_and(|k|k>1.0 && k.fract()==0.0 && k<=32.0) && matches!(**a, Fun(F::Tan|F::Cot|F::Sec|F::Csc,_)) => {
            let (n,d)=fraction(a); (sym::pow(n,(**b).clone()),sym::pow(d,(**b).clone()))
        }
        Fun(F::Cot,u) => (Fun(F::Cos,u.clone()),Fun(F::Sin,u.clone())),
        Fun(F::Csc,u) => (Num(1.0),Fun(F::Sin,u.clone())),
        Fun(F::Tan,u) => (Fun(F::Sin,u.clone()),Fun(F::Cos,u.clone())),
        Fun(F::Sec,u) => (Num(1.0),Fun(F::Cos,u.clone())),
        _ => (e.clone(),Num(1.0)),
    }
}
pub(super) fn find(ps: &[Piece], win: (f64, f64)) -> Vec<Local> {
    let mut out = vec![];
    for (pi, p) in ps.iter().enumerate() {
        // Rational cancellation already has its exact reduced expressions and marks.
        if p.rat.is_some() { continue; }
        let xs = hole_cuts::find(&p.e, win.0 - 1.0, win.1 + 1.0);
        for cut in &xs {
            let x = cut.x;
            let closure = p.dom.is_empty() || p.dom.iter().any(|iv| x >= iv.lo && x <= iv.hi);
            if !closure || !(cut.zero.is_some() || cut.exact.as_ref().is_some_and(|xe| exact::undefined_at(&p.e, xe))) { continue; }
            // A numerical cut is an approximation, never an exact answer displayed to the teacher.
            let xe = cut.exact.clone().or_else(|| exact::of_text(&format!("{x:?}")));
            let Some(c) = xe.as_ref().and_then(|xe| {
                series(&p.e,xe,cut.zero.as_ref()).or_else(|| {
                    let (n,d)=fraction(&p.e);
                    let e=sym::div(n,d);
                    if sym::size(&e)>4000 { None } else { series(&e,xe,cut.zero.as_ref()) }
                }).or_else(|| {
                    // A nonanalytic inner spelling may cancel: |x|/|x| or sin(|x|)/|x|. Prove that
                    // the two specialised analytic series agree, including derivatives, before continuing it.
                    let h=1e-5*(1.0+x.abs());
                    let form=|side| sym::simplify(&exact::specialize(&p.e,x+side*h),true);
                    let l=series(&form(-1.0),xe,cut.zero.as_ref())?;
                    let r=series(&form(1.0),xe,cut.zero.as_ref())?;
                    if l.len()!=r.len() || l.iter().zip(&r).any(|(a,b)|(a-b).abs()>1e-12*(1.0+a.abs()+b.abs())) { None }
                    else { Some(l.iter().zip(&r).map(|(a,b)|(a+b)*0.5).collect()) }
                })
            }).filter(|c| c.len() >= 12) else { continue };
            // (two holes closer than a hundredth: the series' radius would be so small that its joins with the formula, whose
            // rounding there is as large as the value, prove nothing; no series is built, and the tracer says so)
            if xs.iter().any(|o| (o.x - x).abs() > 1e-12 * (1.0 + x.abs()) && (o.x - x).abs() < 1e-2) { continue; }
            let mut local = Local { pi, x, radius: 0.125, xe: cut.exact.clone(), c };
            for other in &xs { if (other.x - x).abs() > 1e-12 * (1.0+x.abs()) { local.radius = local.radius.min(0.25 * (other.x - x).abs()); } }
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

impl Model {
    pub(super) fn is_isolated(&self, x: f64) -> bool {
        self.piece_at(x).is_some_and(|pi| {
            let d = &self.pieces[pi].dom;
            !d.is_empty() && d.iter().all(|iv| iv.lo == x && iv.hi == x)
        })
    }
    /// At an isolated piece, differentiating its constant expression is meaningless. Establish continuity
    /// and all derivatives up to k from the adjoining analytic pieces instead. A corner/jump fails this check.
    pub(super) fn extension_derivative(&self, k: usize, x: f64) -> Option<f64> {
        let h = 1e-7 * (1.0 + x.abs());
        let (lp, rp) = (self.piece_at(x-h)?, self.piece_at(x+h)?);
        let pi = self.piece_at(x)?;
        if lp == pi || rp == pi { return None; }
        let xe = self.pieces[pi].bounds.iter().find(|(v,_)| *v == x).and_then(|(_,pv)| pv.ex.as_ref());
        let own = self.g(0,x)?;
        let mut value = own;
        for n in 0..=k {
            // side() counts derivatives of the displayed f; g() differentiates the typed starting function.
            let displayed = if matches!(self.mode,Mode::Fp{..}) { n+1 } else { n };
            let l = self.side(lp,displayed,x,xe,-1.0,None)?.0;
            let r = self.side(rp,displayed,x,xe,1.0,None)?.0;
            if (l-r).abs() > 1e-8 * (1.0+l.abs()+r.abs()) { return None; }
            value = (l+r)*0.5;
            if n == 0 && (own-value).abs() > 1e-9*(1.0+own.abs()) { return None; }
        }
        Some(value)
    }
}

/// Last line of defence, after grid sampling and before any derivative stroke is used for label placement.
/// A run of eight points at one rounded pixel x, or a nearly vertical segment, must have trustworthy values
/// or evidence of a genuine discontinuity. Rounding noise is split out, never connected across the cut.
pub(super) fn guard(svg: &mut String, m: &Model, p: &Params, fis: &[usize], g: &GridL) -> bool {
    let colors = if p.s("style") == "Print" { ["#000000", "#444444", "#1c1c1c"] } else { COL };
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg.as_str();
    let mut refused = false;
    while let Some(start) = rest.find("<polyline ") {
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        let Some(end) = tail.find("/>").map(|i| i+2) else { out.push_str(tail); rest=""; break };
        let tag = &tail[..end];
        rest = &tail[end..];
        let fi = fis.iter().copied().find(|&fi| fi > 0 && tag.contains(&format!("stroke=\"{}\"", colors[fi])));
        let Some(fi) = fi else { out.push_str(tag); continue };
        let Some(st) = tag.find("points=\"").map(|i| i+8) else { out.push_str(tag); continue };
        let en = st + tag[st..].find('"').unwrap_or(0);
        let points: Vec<(f64,f64)> = tag[st..en].split_whitespace().filter_map(|s| { let (a,b)=s.split_once(',')?; Some((a.parse().ok()?,b.parse().ok()?)) }).collect();
        let mut suspects = vec![false; points.len()];
        let mut i = 0;
        while i < points.len() {
            let mut j=i+1;
            while j < points.len() && (points[j].0-points[i].0).abs() <= 0.02 { j+=1; }
            if j-i >= 8 { suspects[i..j].fill(true); }
            i=j;
        }
        for i in 1..points.len() {
            if (points[i].0-points[i-1].0).abs() <= 0.02 && (points[i].1-points[i-1].1).abs() > 0.5 { suspects[i-1]=true; suspects[i]=true; }
        }
        if !suspects.iter().any(|v| *v) { out.push_str(tag); continue; }
        // SVG coordinates are rounded. Evaluate suspected strokes at the underlying cut, which may be
        // an irrational root inside this pixel bucket, rather than trusting values at its rounded x.
        let cuts: Vec<_> = m.pieces.iter().enumerate().flat_map(|(pi,p)| hole_cuts::find(&p.e,g.x0,g.x1).into_iter().map(move |c|(pi,c.x))).collect();
        let mut bad = vec![false; points.len()];
        for i in 0..points.len() {
            if !suspects[i] { continue; }
            let (px,py)=points[i];
            let x=g.x0+(px-g.ml)/g.ux;
            let x=cuts.iter().filter(|(_,c)| (*c-x).abs()*g.ux<=0.025).min_by(|a,b|(a.1-x).abs().total_cmp(&(b.1-x).abs())).map(|c|c.1).unwrap_or(x);
            let Some(pi)=m.piece_at(x) else { bad[i]=true; continue };
            let k=match m.mode { Mode::F=>fi, Mode::Fp{..}=>fi-1 };
            if let Some(v)=m.local_value(pi,k,x) {
                bad[i]=(py-(g.mt+(g.y1-v)*g.uy)).abs()>0.25;
                continue;
            }
            let err=m.dp_err(pi,fi,x);
            if err.is_finite() && err*g.uy <= 0.25 { continue; }
            let h=((g.x1-g.x0)/3600.0).max(1e-7*(1.0+x.abs()));
            let get=|t| {
                let pi=m.piece_at(t)?;
                let v=m.dp(pi,fi,t)?;
                (m.dp_err(pi,fi,t)<1e-5*(1.0+v.abs())).then_some(v)
            };
            let sequences: Vec<Option<Vec<f64>>>=[-1.0,1.0].iter().map(|side| {
                [1.0,0.125,0.015625].iter().map(|scale|get(x+side*h*scale)).collect()
            }).collect();
            // A one-sided logarithm, root or pole is just as real as a two-sided asymptote.
            let growing=|v:&Vec<f64>| {
                (v[2].abs()>4.0*v[1].abs() && v[1].abs()>4.0*v[0].abs())
                    || (v[2]-v[1]).abs()>(v[1]-v[0]).abs()*0.5
            };
            let genuine=sequences.iter().flatten().any(growing) || match (&sequences[0],&sequences[1]) {
                (Some(l),Some(r)) => (l[2]-r[2]).abs()>0.5*(l[0]-r[0]).abs() && (l[2]-r[2]).abs()*g.uy>0.5,
                _=>false,
            };
            bad[i]=!genuine;
        }
        if !bad.iter().any(|v| *v) { out.push_str(tag); continue; }
        if !m.locals.iter().any(|l| { let px=g.ml+(l.x-g.x0)*g.ux; points.iter().zip(&bad).any(|(q,b)| *b && (q.0-px).abs()<l.radius*g.ux+0.1) }) { refused=true; }
        let mut run=vec![];
        let flush=|run:&mut Vec<String>, out:&mut String| {
            if run.len()>1 { out.push_str(&tag[..st]); out.push_str(&run.join(" ")); out.push_str(&tag[en..]); }
            run.clear();
        };
        for (q,b) in points.iter().zip(&bad) {
            if *b { flush(&mut run,&mut out); } else { run.push(format!("{:.2},{:.2}",q.0,q.1)); }
        }
        flush(&mut run,&mut out);
    }
    out.push_str(rest);
    *svg=out;
    refused
}

/// sin(nu)/sin(u)=U_(n−1)(cos u), n=1..4, from the sine addition formula. Derive derivatives
/// from this polynomial, but keep the original expression and every excluded sin(u)=0 point.
/// This avoids both quotient cancellation and repeated expensive root searches in the common triple-angle case.
pub(super) fn smooth_form(e: &Ex) -> Ex {
    use sym::{Ex::*,F};
    if let Div(a,b)=e {
        if let (Fun(F::Sin,u),Fun(F::Sin,v))=(a.as_ref(),b.as_ref()) {
            if let (Some((ak,ab)),Some((bk,bb)))=(exact::linear_x(u),exact::linear_x(v)) {
                for n in -4i128..=4 {
                    if n==0 { continue; }
                    let same_slope=ak.n.checked_mul(bk.d).zip(bk.n.checked_mul(ak.d).and_then(|v|v.checked_mul(n))).is_some_and(|(a,b)|a==b);
                    if !same_slope { continue; }
                    let mut offset=Some(XV::default());
                    for _ in 0..n.abs() { offset=offset.and_then(|a|a.add_pub(&bb)); }
                    if n<0 { offset=offset.map(|a|a.neg_pub()); }
                    if offset.as_ref()!=Some(&ab) { continue; }
                    let c=Fun(F::Cos,v.clone());
                    let mut prev=Num(1.0); let mut next=sym::mul(Num(2.0),c.clone());
                    let value=if n.abs()==1 { prev } else {
                        for _ in 2..n.abs() {
                            let value=sym::sub(sym::mul(sym::mul(Num(2.0),c.clone()),next.clone()),prev);
                            prev=next; next=value;
                        }
                        next
                    };
                    return if n<0 {sym::neg(value)}else{value};
                }
            }
        }
    }
    match e {
        Neg(a)=>sym::neg(smooth_form(a)), Add(a,b)=>sym::add(smooth_form(a),smooth_form(b)),
        Sub(a,b)=>sym::sub(smooth_form(a),smooth_form(b)), Mul(a,b)=>sym::mul(smooth_form(a),smooth_form(b)),
        Div(a,b)=>sym::div(smooth_form(a),smooth_form(b)), _=>e.clone(),
    }
}
