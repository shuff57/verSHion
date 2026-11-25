//! The numbers behind the slope field (`slope-fields`): the typed f(x, y) as a tree, its value at a point, solution curves
//! by an adaptive Runge–Kutta method, Euler's method (exact where the inputs are exact rationals), and the equilibrium
//! solutions (horizontal lines y = c on which f is 0 for every x).
//!
//! The tree is the tangent figure's reading of a typed expression (`tangent_expr`, the coordinate grid's grammar kept as a
//! tree). It reads one variable, x; the second variable is written into the text as a marker number (`SENT`, a value no
//! one types) before it is read, so every y in the tree is a `Num` node holding that marker, and `ev` reads it as y. Exact
//! values come from the same tree with the marker replaced by an exact number, evaluated by `tangent_expr::exact`.
//!
//! Solution curves: Dormand–Prince 5(4) with step control (relative and absolute tolerance 1e-10, at most 3 px a step
//! inside the window), stopping where the solution leaves the window by ten window heights, where a step cannot be taken
//! (the slope grows without bound: a vertical tangent or a blow-up) and where the curve would cross a point at which f is
//! undefined — a zero of a denominator, the edge of a logarithm's or a root's domain — found by the sign of each such
//! quantity along the curve (`guards`), so a curve is never drawn through a singularity even where no step lands on it.
use super::tangent_expr::{self as ex, Ex, Node, R};
use std::f64::consts::{E as EULER, PI};

/// The marker read as y.
pub(crate) const SENT: f64 = 7.77e-300;
pub(crate) const SENT_TXT: &str = "(7.77e-300)";

fn b(n: Node) -> Box<Node> { Box::new(n) }
fn is_y(n: &Node) -> bool { matches!(n, Node::Num(v, _) if *v == SENT) }

/// The value of the tree at (x, y), as the tangent figure's tree computes it (the grid's reader, with the real odd root).
pub(crate) fn ev(n: &Node, x: f64, y: f64) -> f64 {
    use Node::*;
    match n {
        Num(v, _) => if *v == SENT { y } else { *v },
        X => x,
        Pi => PI,
        Euler => EULER,
        Neg(a) => -ev(a, x, y),
        Add(a, c) => ev(a, x, y) + ev(c, x, y),
        Sub(a, c) => ev(a, x, y) - ev(c, x, y),
        Mul(a, c) => ev(a, x, y) * ev(c, x, y),
        Div(a, c) => ev(a, x, y) / ev(c, x, y),
        Pow(a, e) => {
            let (bv, evv) = (ev(a, x, y), ev(e, x, y));
            if bv < 0.0 {
                if let Some((p, _)) = ex::odd_root(e) { let m = bv.abs().powf(evv); return if p % 2 == 0 { m } else { -m }; }
            }
            bv.powf(evv)
        }
        LogB(bs, a) => ev(a, x, y).ln() / ev(bs, x, y).ln(),
        Func(f, a) => {
            let a = ev(a, x, y);
            match *f {
                "sqrt" => a.sqrt(), "abs" => a.abs(), "ln" => a.ln(), "log" => a.log10(), "exp" => a.exp(),
                "sin" => a.sin(), "cos" => a.cos(), "tan" => a.tan(), "sec" => 1.0 / a.cos(), "csc" => 1.0 / a.sin(), "cot" => 1.0 / a.tan(),
                "arcsin" => a.asin(), "arccos" => a.acos(), "arctan" => a.atan(),
                n => super::grid_b7::arc_recip(n, a),
            }
        }
    }
}

/// `ev` with every part finite: None where any part of the expression is infinite or not a number.
pub(crate) fn evs(n: &Node, x: f64, y: f64) -> Option<f64> {
    use Node::*;
    let ok = |v: f64| v.is_finite().then_some(v);
    match n {
        Num(..) | X | Pi | Euler => ok(ev(n, x, y)),
        Neg(a) => ok(-evs(a, x, y)?),
        Add(a, c) => ok(evs(a, x, y)? + evs(c, x, y)?),
        Sub(a, c) => ok(evs(a, x, y)? - evs(c, x, y)?),
        Mul(a, c) => ok(evs(a, x, y)? * evs(c, x, y)?),
        Div(a, c) => { let (p, q) = (evs(a, x, y)?, evs(c, x, y)?); if q == 0.0 { None } else { ok(p / q) } }
        Pow(a, e) | LogB(a, e) => { evs(a, x, y)?; evs(e, x, y)?; ok(ev(n, x, y)) }
        Func(_, a) => { evs(a, x, y)?; ok(ev(n, x, y)) }
    }
}

pub(crate) fn has_x(n: &Node) -> bool {
    use Node::*;
    match n { X => true, Num(..) | Pi | Euler => false, Neg(a) | Func(_, a) => has_x(a), Add(a, c) | Sub(a, c) | Mul(a, c) | Div(a, c) | Pow(a, c) | LogB(a, c) => has_x(a) || has_x(c) }
}
pub(crate) fn has_y(n: &Node) -> bool {
    use Node::*;
    match n { Num(..) => is_y(n), X | Pi | Euler => false, Neg(a) | Func(_, a) => has_y(a), Add(a, c) | Sub(a, c) | Mul(a, c) | Div(a, c) | Pow(a, c) | LogB(a, c) => has_y(a) || has_y(c) }
}

/// The tree with x replaced by `xn` and y by `yn` (where given).
pub(crate) fn subst(n: &Node, xn: Option<&Node>, yn: Option<&Node>) -> Node {
    use Node::*;
    let s = |a: &Node| b(subst(a, xn, yn));
    match n {
        X => xn.cloned().unwrap_or(X),
        Num(..) if is_y(n) => yn.cloned().unwrap_or_else(|| n.clone()),
        Num(..) | Pi | Euler => n.clone(),
        Neg(a) => Neg(s(a)),
        Func(f, a) => Func(f, s(a)),
        Add(a, c) => Add(s(a), s(c)),
        Sub(a, c) => Sub(s(a), s(c)),
        Mul(a, c) => Mul(s(a), s(c)),
        Div(a, c) => Div(s(a), s(c)),
        Pow(a, c) => Pow(s(a), s(c)),
        LogB(a, c) => LogB(s(a), s(c)),
    }
}

/// A rational as a tree node.
pub(crate) fn rat_node(p: i128, q: i128) -> Option<Node> { let r = R::new(p, q)?; Some(Node::Num(r.0 as f64 / r.1 as f64, Some(r))) }
fn ex_rat(e: &Ex) -> Option<(i128, i128)> { e.rational() }

/// The exact value of the tree at an exact rational x and y, or None where it is not derived exactly.
pub(crate) fn exact_at(n: &Node, x: &Ex, y: &Ex) -> Option<Ex> {
    let (p, q) = ex_rat(y)?;
    let t = subst(n, None, Some(&rat_node(p, q)?));
    ex::exact(&t, Some(x))
}
/// Whether some part of the tree divides by an exact 0 (or is ln 0, tan at π/2, …) at the exact point.
pub(crate) fn exact_pole(n: &Node, x: &Ex, y: &Ex) -> bool {
    let Some((p, q)) = ex_rat(y) else { return false };
    let Some(yn) = rat_node(p, q) else { return false };
    ex::zero_at(&subst(n, None, Some(&yn)), x)
}

// ───────────────────────── the typed equation ─────────────────────────

/// f(x, y) read from the equation, with the quantities whose sign change marks a point where f is undefined.
#[derive(Clone, Debug)]
pub(crate) struct F { pub tree: Node, pub guards: Vec<Node>, pub branch: Vec<Node> }
impl F {
    pub(crate) fn new(tree: Node) -> F { let (mut g, mut br) = (vec![], vec![]); guards_of(&tree, &mut g, &mut br); F { tree, guards: g, branch: br } }
    /// The slope at (x, y), or None where f is undefined: where any part of it is not a finite number (1/ln y at y = 0
    /// is undefined, not the −0 that 1/(−∞) gives). A finite slope however large is a value.
    pub(crate) fn at(&self, x: f64, y: f64) -> Option<f64> { evs(&self.tree, x, y) }
}

/// The factors whose zero makes a product zero (a denominator's zero set): x² and x·(y − 1) by their factors.
fn factors(n: &Node, out: &mut Vec<Node>) {
    use Node::*;
    match n {
        Mul(a, c) => { factors(a, out); factors(c, out); }
        Neg(a) | Func("abs", a) => factors(a, out),
        Pow(a, e) if !has_x(e) && !has_y(e) && ex::exact(e, None).and_then(|v| v.rational()).is_some_and(|(p, q)| p > 0 && q == 1) => factors(a, out),
        Div(a, _) => factors(a, out),
        Num(..) if !is_y(n) => {}
        Pi | Euler => {}
        _ => out.push(n.clone()),
    }
}
/// `out`: where f stops being defined (denominators, domains). `br`: where f stops being smooth in y though defined (the
/// base of a root with a power under 1 that holds y: y^(2/3), √y and ∛(y − 1) at 0), where the solution need not be unique.
fn guards_of(n: &Node, out: &mut Vec<Node>, br: &mut Vec<Node>) {
    use Node::*;
    match n {
        Num(..) | X | Pi | Euler => {}
        Neg(a) => guards_of(a, out, br),
        Add(a, c) | Sub(a, c) | Mul(a, c) => { guards_of(a, out, br); guards_of(c, out, br); }
        Div(a, c) => { guards_of(a, out, br); guards_of(c, out, br); factors(c, out); }
        Pow(a, e) => {
            guards_of(a, out, br);
            guards_of(e, out, br);
            let k = if has_x(e) || has_y(e) { None } else { ex::exact(e, None).and_then(|v| v.rational()) };
            if has_y(a) && k.is_some_and(|(p, q)| p > 0 && p < q) { factors(a, br); }
            match k {
                // a whole power ≥ 0 and a real odd root of a positive power are defined for every base
                Some((p, 1)) if p >= 0 => {}
                Some(_) if ex::odd_root(e).is_some_and(|(p, _)| p > 0) => {}
                _ => factors(a, out),
            }
        }
        LogB(bs, a) => { guards_of(bs, out, br); guards_of(a, out, br); out.push((**a).clone()); out.push((**bs).clone()); }
        Func(f, a) => {
            guards_of(a, out, br);
            if *f == "sqrt" && has_y(a) { factors(a, br); }
            let one = || Num(1.0, R::new(1, 1));
            let two = || Num(2.0, R::new(2, 1));
            match *f {
                "ln" | "log" | "sqrt" => out.push((**a).clone()),
                "tan" | "sec" => out.push(Func("cos", a.clone())),
                "csc" | "cot" => out.push(Func("sin", a.clone())),
                "arcsin" | "arccos" => out.push(Sub(b(one()), b(Pow(a.clone(), b(two()))))),
                "arcsec" | "arccsc" => out.push(Sub(b(Pow(a.clone(), b(two()))), b(one()))),
                _ => {}
            }
        }
    }
}

/// The parts whose 0 leaves f with no value: a denominator's factors, the inside of ln or log, a base to a negative power, cos
/// under tan or sec, sin under csc or cot. (Not the inside of √ or of a positive fractional power: f has a value where
/// those are 0, √0 = 0, so an equilibrium can sit at the edge they make.)
pub(crate) fn open_guards(n: &Node, out: &mut Vec<Node>) {
    use Node::*;
    match n {
        Num(..) | X | Pi | Euler => {}
        Neg(a) => open_guards(a, out),
        Add(a, c) | Sub(a, c) | Mul(a, c) => { open_guards(a, out); open_guards(c, out); }
        Div(a, c) => { open_guards(a, out); open_guards(c, out); factors(c, out); }
        Pow(a, e) => {
            open_guards(a, out);
            open_guards(e, out);
            // (a constant power above 0, rational or not, has a value at a base of 0: 0^π = 0; a power in x alone is decided
            // at each x, by `xpow_guards`: 0^(x² + 1) = 0)
            let positive = !has_x(e) && !has_y(e) && ev(e, 0.0, 0.0) > 0.0;
            if !positive && (has_y(e) || !has_x(e)) { factors(a, out); }
        }
        LogB(bs, a) => { open_guards(bs, out); open_guards(a, out); out.push((**a).clone()); out.push((**bs).clone()); }
        Func(f, a) => {
            open_guards(a, out);
            match *f {
                "ln" | "log" => out.push((**a).clone()),
                "tan" | "sec" => out.push(Func("cos", a.clone())),
                "csc" | "cot" => out.push(Func("sin", a.clone())),
                _ => {}
            }
        }
    }
}

/// Bases raised to a power in x alone (y^(x² + 1), y^x): f has no value where the base is 0 only at the x's where the power
/// is 0 or less there.
pub(crate) fn xpow_guards(n: &Node, out: &mut Vec<(Node, Node)>) {
    use Node::*;
    match n {
        Num(..) | X | Pi | Euler => {}
        Neg(a) | Func(_, a) => xpow_guards(a, out),
        Add(a, c) | Sub(a, c) | Mul(a, c) | Div(a, c) | LogB(a, c) => { xpow_guards(a, out); xpow_guards(c, out); }
        Pow(a, e) => {
            xpow_guards(a, out);
            xpow_guards(e, out);
            if has_x(e) && !has_y(e) { let mut fs = vec![]; factors(a, &mut fs); for q in fs { out.push((q, (**e).clone())); } }
        }
    }
}

// ───────────────────────── solution curves ─────────────────────────

/// Why a curve ends where it does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Stop {
    /// It reached the side of the window (or the last x asked for).
    Edge,
    /// It left the window far above or below (it does not come back within ten window heights).
    Left,
    /// It reaches a point where dy/dx is undefined (a zero of a denominator, the edge of a domain).
    Singular,
    /// No step can be taken: the slope grows without bound (a vertical tangent, or a blow-up).
    Stalled,
    /// The start point itself has no slope.
    Undefined,
    /// Too many steps (a curve that winds without end).
    Steps,
    /// It reaches a point where dy/dx is defined but not smooth in y (y^(2/3) at y = 0): past it the solution need not be
    /// unique, so it is not followed (at the start itself: no curve).
    Branch,
}

/// The window the curves are drawn in, and its scale in px per unit (for the 3 px steps).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Win { pub x0: f64, pub x1: f64, pub y0: f64, pub y1: f64, pub ux: f64, pub uy: f64, pub cap: Option<f64>, pub tol: f64 }

#[derive(Clone, Debug)]
pub(crate) struct Run { pub pts: Vec<(f64, f64)>, pub stop: Stop, pub end: (f64, f64), pub at_stops: Vec<Option<f64>> }

const C: [f64; 7] = [0.0, 0.2, 0.3, 0.8, 8.0 / 9.0, 1.0, 1.0];
const A: [[f64; 6]; 7] = [
    [0.0; 6],
    [0.2, 0.0, 0.0, 0.0, 0.0, 0.0],
    [3.0 / 40.0, 9.0 / 40.0, 0.0, 0.0, 0.0, 0.0],
    [44.0 / 45.0, -56.0 / 15.0, 32.0 / 9.0, 0.0, 0.0, 0.0],
    [19372.0 / 6561.0, -25360.0 / 2187.0, 64448.0 / 6561.0, -212.0 / 729.0, 0.0, 0.0],
    [9017.0 / 3168.0, -355.0 / 33.0, 46732.0 / 5247.0, 49.0 / 176.0, -5103.0 / 18656.0, 0.0],
    [35.0 / 384.0, 0.0, 500.0 / 1113.0, 125.0 / 192.0, -2187.0 / 6784.0, 11.0 / 84.0],
];
const E5: [f64; 7] = [71.0 / 57600.0, 0.0, -71.0 / 16695.0, 71.0 / 1920.0, -17253.0 / 339200.0, 22.0 / 525.0, -1.0 / 40.0];

/// One Dormand–Prince step from (x, y) with slope k1: (y5, error, slope at the end), or None where a stage is undefined.
fn dp_step(f: &F, x: f64, y: f64, k1: f64, h: f64) -> Option<(f64, f64, f64)> {
    let mut k = [0.0f64; 7];
    k[0] = k1;
    for s in 1..7 {
        let mut yi = y;
        for j in 0..s { yi += h * A[s][j] * k[j]; }
        k[s] = f.at(x + C[s] * h, yi)?;
    }
    let y5 = y + h * (0..6).map(|j| A[6][j] * k[j]).sum::<f64>();
    let err = (h * (0..7).map(|j| E5[j] * k[j]).sum::<f64>()).abs();
    Some((y5, err, k[6]))
}

/// The guard quantities at a point (NaN where one is undefined).
fn guard_vals(f: &F, x: f64, y: f64) -> Vec<f64> { f.guards.iter().map(|g| ev(g, x, y)).collect() }
fn crossed(a: &[f64], c: &[f64]) -> bool { a.iter().zip(c).any(|(u, v)| u.is_finite() && v.is_finite() && ((*u > 0.0 && *v <= 0.0) || (*u < 0.0 && *v >= 0.0))) }

/// The solution through (x0, y0), one way (`dir` = 1 forward, −1 back), to the side of the window or to the farthest of
/// `stops` (x values where its y is wanted, in order away from x0).
pub(crate) fn integrate(f: &F, x0: f64, y0: f64, dir: f64, w: &Win, stops: &[f64]) -> Run {
    let span = (w.x1 - w.x0).abs().max(1e-9);
    let hy = (w.y1 - w.y0).abs().max(1e-9);
    let mut at_stops = vec![None; stops.len()];
    let side = if dir > 0.0 { w.x1 } else { w.x0 };
    let target = stops.iter().fold(side, |m, s| if dir > 0.0 { m.max(*s) } else { m.min(*s) });
    let Some(mut k1) = f.at(x0, y0) else { return Run { pts: vec![(x0, y0)], stop: Stop::Undefined, end: (x0, y0), at_stops } };
    let (atol, rtol) = (w.tol * hy, w.tol);
    let hmin = 1e-11 * span;
    let (mut x, mut y) = (x0, y0);
    let mut h = span / 200.0;
    let mut pts = vec![(x, y)];
    let mut gv = guard_vals(f, x, y);
    let bvals = |x: f64, y: f64| -> Vec<f64> { f.branch.iter().map(|g| ev(g, x, y)).collect() };
    let mut bv = bvals(x, y);
    if bv.iter().any(|v| *v == 0.0) { return Run { pts: vec![(x0, y0)], stop: Stop::Branch, end: (x0, y0), at_stops }; }
    let mut si = 0;
    // a stop at the start itself
    while si < stops.len() && (stops[si] - x).abs() <= 1e-12 * span { at_stops[si] = Some(y); si += 1; }
    let mut steps = 0;
    let stop = loop {
        if (target - x) * dir <= 1e-13 * span { break Stop::Edge; }
        steps += 1;
        if steps > 6000 { break Stop::Steps; }
        let inside = x >= w.x0 - 1e-9 && x <= w.x1 + 1e-9 && y >= w.y0 - 0.05 * hy && y <= w.y1 + 0.05 * hy;
        let mut hmax = span / 100.0;
        if inside { hmax = hmax.min(3.0 / (w.ux * w.ux + (k1 * w.uy).powi(2)).sqrt()); }
        let limit = if si < stops.len() && (stops[si] - target) * dir < 0.0 { stops[si] } else { target };
        let mut hh = h.min(hmax).max(hmin);
        let to_limit = (limit - x) * dir;
        let landing = hh >= to_limit;
        if landing { hh = to_limit; }
        let sh = hh * dir;
        let Some((yn, err, kn)) = dp_step(f, x, y, k1, sh) else {
            if hh <= hmin * 1.0001 { break Stop::Stalled; }
            h = hh * 0.25;
            continue;
        };
        let tol = atol + rtol * y.abs().max(yn.abs());
        if err > tol {
            if hh <= hmin * 1.0001 { break Stop::Stalled; }
            h = hh * (0.9 * (tol / err).powf(0.2)).clamp(0.1, 0.9);
            continue;
        }
        let xn = if landing { limit } else { x + sh };
        let gn = guard_vals(f, xn, yn);
        if crossed(&gv, &gn) {
            // the point where the guard is 0, along the step (the guard taken as linear across it: a step is at most 3 px
            // inside the window, and a partial step next to 0/0 is not to be trusted)
            let t = gv.iter().zip(&gn).filter(|(u, v)| u.is_finite() && v.is_finite() && ((**u > 0.0 && **v <= 0.0) || (**u < 0.0 && **v >= 0.0)))
                .map(|(u, v)| u / (u - v)).fold(1.0f64, f64::min).clamp(0.0, 1.0);
            pts.push((x + (xn - x) * t, y + (yn - y) * t));
            break Stop::Singular;
        }
        let bn = bvals(xn, yn);
        // (touching 0: to the start's scale, not the window's, so the answer does not depend on the window)
        if crossed(&bv, &bn) || bn.iter().any(|v| v.abs() <= 1e-12 * (1.0 + y0.abs())) {
            let t = bv.iter().zip(&bn).filter(|(u, v)| u.is_finite() && v.is_finite() && ((**u > 0.0 && **v <= 0.0) || (**u < 0.0 && **v >= 0.0)))
                .map(|(u, v)| u / (u - v)).fold(1.0f64, f64::min).clamp(0.0, 1.0);
            pts.push((x + (xn - x) * t, y + (yn - y) * t));
            break Stop::Branch;
        }
        bv = bn;
        x = xn;
        y = yn;
        k1 = kn;
        gv = gn;
        pts.push((x, y));
        while si < stops.len() && (stops[si] - x) * dir <= 1e-12 * span { at_stops[si] = Some(y); si += 1; }
        // (drawing: ten window heights out is far enough; a value worked out for a table: up to ±cap, wherever the window is)
        let far = match w.cap { Some(c) => y.abs() > c, None => y < w.y0 - 10.0 * hy || y > w.y1 + 10.0 * hy };
        if far { break Stop::Left; }
        let grow = if err > 0.0 { (0.9 * (tol / err).powf(0.2)).clamp(0.2, 5.0) } else { 5.0 };
        h = if landing { h.max(hh) } else { hh * grow };
    };
    let end = *pts.last().unwrap_or(&(x0, y0));
    Run { pts, stop, end, at_stops }
}

// ───────────────────────── Euler's method ─────────────────────────

#[derive(Clone, Debug)]
pub(crate) struct Step { pub x: f64, pub y: f64, pub xe: Option<Ex>, pub ye: Option<Ex> }

/// Why Euler's method stopped short of n steps.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum EulerStop { Undefined(usize), TooBig(usize) }

/// n steps of Euler's method from (x0, y0) with step h: each y exact while the values are exact rationals.
pub(crate) fn euler(f: &F, x0: (f64, Option<Ex>), y0: (f64, Option<Ex>), h: (f64, Option<Ex>), n: usize) -> (Vec<Step>, Option<EulerStop>) {
    let rat = |e: &Option<Ex>| e.as_ref().filter(|v| v.rational().is_some()).cloned();
    let he = rat(&h.1);
    let mut cur = Step { x: x0.0, y: y0.0, xe: rat(&x0.1), ye: rat(&y0.1) };
    // (the start is written as typed whatever h is; with an h that is not rational, every later value is rounded)
    let mut out = vec![cur.clone()];
    if he.is_none() { cur.xe = None; cur.ye = None; }
    for k in 0..n {
        let exact_f = match (&cur.xe, &cur.ye) { (Some(xe), Some(ye)) => exact_at(&f.tree, xe, ye).filter(|v| v.rational().is_some()), _ => None };
        let pole = matches!((&cur.xe, &cur.ye), (Some(xe), Some(ye)) if exact_pole(&f.tree, xe, ye));
        let fv = f.at(cur.x, cur.y);
        if pole || fv.is_none() { return (out, Some(EulerStop::Undefined(k))); }
        let next_ye = match (&exact_f, &cur.ye, &he) { (Some(fe), Some(ye), Some(hh)) => hh.mul(fe).and_then(|d| ye.add(&d)).filter(|v| v.rational().is_some()), _ => None };
        // (x = x0 + kh stays exact whether or not y does)
        let next_xe = match (&cur.xe, &he) { (Some(xe), Some(hh)) => xe.add(hh), _ => None };
        let x = next_xe.as_ref().map(|e| e.value()).unwrap_or(x0.0 + (k + 1) as f64 * h.0);
        let y = next_ye.as_ref().map(|e| e.value()).unwrap_or(cur.y + h.0 * fv.unwrap_or(0.0));
        if !y.is_finite() || y.abs() > 1e6 { return (out, Some(EulerStop::TooBig(k + 1))); }
        cur = Step { x, y, xe: next_xe, ye: next_ye };
        out.push(cur.clone());
    }
    (out, None)
}

// ───────────────────────── equilibrium solutions ─────────────────────────

/// Small rationals with exact arithmetic (overflow refused), for the polynomial count of roots.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Q(pub i128, pub i128);
fn gcd(a: i128, b: i128) -> i128 { let (mut a, mut b) = (a.abs(), b.abs()); while b != 0 { let t = a % b; a = b; b = t; } a }
impl Q {
    fn new(p: i128, q: i128) -> Option<Q> { if q == 0 { return None; } let g = gcd(p, q).max(1); let (p, q) = (p / g, q / g); Some(if q < 0 { Q(p.checked_neg()?, q.checked_neg()?) } else { Q(p, q) }) }
    fn add(self, o: Q) -> Option<Q> { Q::new(self.0.checked_mul(o.1)?.checked_add(o.0.checked_mul(self.1)?)?, self.1.checked_mul(o.1)?) }
    fn sub(self, o: Q) -> Option<Q> { self.add(Q(-o.0, o.1)) }
    fn mul(self, o: Q) -> Option<Q> { let g1 = gcd(self.0, o.1).max(1); let g2 = gcd(o.0, self.1).max(1); Q::new((self.0 / g1).checked_mul(o.0 / g2)?, (self.1 / g2).checked_mul(o.1 / g1)?) }
    fn div(self, o: Q) -> Option<Q> { if o.0 == 0 { None } else { self.mul(Q::new(o.1, o.0)?) } }
    fn sign(self) -> i32 { self.0.signum() as i32 }
    /// A float that is exactly a short binary fraction (a typed window end such as −4 or 2.5).
    pub(crate) fn of_f64(v: f64) -> Option<Q> {
        for k in 0..20 { let s = v * (1u64 << k) as f64; if s.fract() == 0.0 && s.abs() < 1e15 { return Q::new(s as i128, 1i128 << k); } }
        None
    }
    /// Any finite float, exactly (None where its numerator or denominator would not fit): 0.3333333333333333 is
    /// 6004799503160661/2⁵⁴, which is less than 1/3.
    pub(crate) fn of_f64_exact(v: f64) -> Option<Q> {
        if !v.is_finite() { return None; }
        if v == 0.0 { return Some(Q(0, 1)); }
        let bits = v.to_bits();
        let ex = ((bits >> 52) & 0x7ff) as i32;
        let fr = (bits & ((1u64 << 52) - 1)) as i128;
        let (mut m, mut e) = if ex == 0 { (fr, -1074) } else { (fr | (1i128 << 52), ex - 1075) };
        while m & 1 == 0 && e < 0 { m >>= 1; e += 1; }
        let m = if v < 0.0 { -m } else { m };
        if e >= 0 { if e > 70 { return None; } Q::new(m.checked_mul(1i128 << e)?, 1) } else { if -e > 125 { return None; } Q::new(m, 1i128 << (-e)) }
    }
    fn le(self, o: Q) -> Option<bool> { Some(o.sub(self)?.0 >= 0) }
}

/// The polynomial degree of a tree in x (None: not a polynomial in x).
pub(crate) fn deg_x(n: &Node) -> Option<i128> {
    use Node::*;
    if !has_x(n) { return Some(0); }
    match n {
        X => Some(1),
        Neg(a) => deg_x(a),
        Add(a, c) | Sub(a, c) => Some(deg_x(a)?.max(deg_x(c)?)),
        Mul(a, c) => Some(deg_x(a)? + deg_x(c)?),
        Div(a, c) if !has_x(c) => deg_x(a),
        Pow(a, e) if !has_x(e) && !has_y(e) => { let (p, q) = ex::exact(e, None)?.rational()?; if q != 1 || p < 0 || p > 24 { return None; } Some(deg_x(a)? * p) }
        _ => None,
    }
}

/// Whether a tree in x (y already put in) is 0 for every x where it is defined: shown exactly, never from a float.
/// A factor that is exactly 0 makes a product 0, a numerator a quotient, a base a positive power; a polynomial in x of
/// degree d is 0 when it is exactly 0 at d + 1 points.
pub(crate) fn zero_fn(n: &Node) -> bool {
    use Node::*;
    // (a constant the exact layer holds is 0 or not; one it does not hold, 0^π, is decided by its structure below)
    if !has_x(n) { if let Some(v) = ex::exact(n, None) { return v.is_zero(); } }
    let poly_zero = || deg_x(n).filter(|d| *d <= 24).is_some_and(|d| (0..=d).all(|k| ex::exact(n, Some(&Ex::rat(R(k, 1)))).is_some_and(|v| v.is_zero())));
    match n {
        Mul(a, c) => zero_fn(a) || zero_fn(c) || poly_zero(),
        Div(a, _) => zero_fn(a),
        Neg(a) => zero_fn(a),
        Pow(a, e) if !has_x(e) => ex::exact(e, None).is_some_and(|v| v.value() > 0.0) && zero_fn(a),
        // (a power in x that is positive for every x, as x² + 1: 0 to it is 0)
        Pow(a, e) if !has_y(e) && positive(e) => zero_fn(a),
        Func("sqrt" | "abs" | "sin" | "tan" | "arcsin" | "arctan", a) => zero_fn(a),
        Add(a, c) | Sub(a, c) => poly_zero() || (zero_fn(a) && zero_fn(c)),
        _ => poly_zero(),
    }
}

/// The expression (in x, no y) is above 0 for every x where it has a value, shown by its structure: a positive constant, a
/// sum of things at least 0 with one above, a product of positives, e to anything, a positive base to any power.
fn positive(n: &Node) -> bool {
    use Node::*;
    match n {
        _ if !has_x(n) && !has_y(n) => ex::exact(n, None).is_some_and(|v| v.value() > 0.0) || matches!(n, Pi | Euler),
        Add(a, c) => (positive(a) && nonneg(c)) || (nonneg(a) && positive(c)),
        Mul(a, c) | Div(a, c) => positive(a) && positive(c),
        Pow(a, _) if positive(a) => true,
        Func("exp", _) => true,
        _ => false,
    }
}
/// The expression is at least 0 for every x where it has a value: a positive one, an even whole power, |u|, √u.
fn nonneg(n: &Node) -> bool {
    use Node::*;
    match n {
        _ if positive(n) => true,
        _ if !has_x(n) && !has_y(n) => ex::exact(n, None).is_some_and(|v| v.value() >= 0.0),
        Pow(_, e) => ex::exact(e, None).and_then(|v| v.rational()).is_some_and(|(p, q)| q == 1 && p % 2 == 0),
        Func("abs" | "sqrt", _) => true,
        Add(a, c) => nonneg(a) && nonneg(c),
        Mul(a, c) => nonneg(a) && nonneg(c),
        _ => false,
    }
}

fn poly_eval(c: &[Q], v: Q) -> Option<Q> { let mut acc = Q(0, 1); for k in c.iter().rev() { acc = acc.mul(v)?.add(*k)?; } Some(acc) }
fn trim(mut c: Vec<Q>) -> Vec<Q> { while c.len() > 1 && c.last().is_some_and(|q| q.0 == 0) { c.pop(); } c }
fn poly_rem(a: &[Q], d: &[Q]) -> Option<Vec<Q>> {
    let mut r = a.to_vec();
    let dl = *d.last()?;
    while r.len() >= d.len() && !(r.len() == 1 && r[0].0 == 0) {
        let k = r.len() - d.len();
        let t = r.last()?.div(dl)?;
        for (i, di) in d.iter().enumerate() { r[k + i] = r[k + i].sub(t.mul(*di)?)?; }
        r.pop();
        if r.is_empty() { r.push(Q(0, 1)); }
        r = trim(r);
        if d.len() == 1 { return Some(vec![Q(0, 1)]); }
    }
    Some(trim(r))
}
/// a / d, the quotient of polynomial long division (None on overflow).
fn poly_div(a: &[Q], d: &[Q]) -> Option<Vec<Q>> {
    let d = trim(d.to_vec());
    let mut r = trim(a.to_vec());
    if r.len() < d.len() { return Some(vec![Q(0, 1)]); }
    let dl = *d.last()?;
    let mut q = vec![Q(0, 1); r.len() - d.len() + 1];
    while r.len() >= d.len() && !(r.len() == 1 && r[0].0 == 0) {
        let k = r.len() - d.len();
        let t = r.last()?.div(dl)?;
        q[k] = t;
        for (i, di) in d.iter().enumerate() { r[k + i] = r[k + i].sub(t.mul(*di)?)?; }
        r.pop();
        if r.is_empty() { r.push(Q(0, 1)); }
        r = trim(r);
    }
    Some(trim(q))
}
fn is_zero_poly(c: &[Q]) -> bool { c.iter().all(|q| q.0 == 0) }
/// The distinct real roots of a square-free c in (lo, hi], each isolated by Sturm counts and then found by halving: (its
/// float, its exact value where it is a rational the halving lands on or a short fraction). None on overflow.
/// A polynomial with rational coefficients at a float, in double-double arithmetic (about 32 digits): beside two roots
/// 10⁻⁹ apart its value, about 10⁻¹⁹, is far below a float's rounding of its terms, but not of these.
fn dd_eval(c: &[Q], y: f64) -> f64 {
    let two_sum = |a: f64, b: f64| { let s = a + b; let bb = s - a; (s, (a - (s - bb)) + (b - bb)) };
    let two_prod = |a: f64, b: f64| { let p = a * b; (p, a.mul_add(b, -p)) };
    // (each coefficient p/q as hi + lo: hi the float quotient, lo the exact residual p − hi·q over q)
    let coef = |q: &Q| { let (n, d) = (q.0 as f64, q.1 as f64); let hi = n / d; let r = (-hi).mul_add(d, n); (hi, r / d) };
    let mut acc = (0.0f64, 0.0f64);
    for q in c.iter().rev() {
        // acc = acc·y + q
        let (p, pe) = two_prod(acc.0, y);
        let pe = pe + acc.1 * y;
        let (ch, cl) = coef(q);
        let (s, se) = two_sum(p, ch);
        let lo = se + pe + cl;
        let (h, l) = two_sum(s, lo);
        acc = (h, l);
    }
    acc.0 + acc.1
}

/// Each distinct real root in [lo, hi]: its float, its exact value where rational, its interval as floats and exactly (the
/// root is the only one in (a, b], or b itself).
fn isolate_roots(c: &[Q], lo: Q, hi: Q) -> Option<Vec<(f64, Option<Q>, (f64, f64), (Q, Q))>> {
    let count = |a: Q, b: Q| -> Option<usize> { let n = sturm_count(c, a, b)?; Some(n - (poly_eval(c, a)?.0 == 0) as usize) };
    let ev = |y: f64| dd_eval(c, y);
    let mut out = vec![];
    let mut stack = vec![(lo, hi, 0u32)];
    while let Some((a, b, depth)) = stack.pop() {
        let n = count(a, b)?;
        if n == 0 { continue; }
        if poly_eval(c, b)?.0 == 0 { let bf = b.0 as f64 / b.1 as f64; out.push((bf, Some(b), (bf, bf), (b, b))); if n > 1 { let m = a.add(b)?.mul(Q(1, 2))?; stack.push((a, m, depth + 1)); stack.push((m, b, depth + 1)); } continue; }
        if n == 1 || depth > 90 {
            let (mut u, mut v) = (a.0 as f64 / a.1 as f64, b.0 as f64 / b.1 as f64);
            // (oriented by the exact sign at the right end, which is no root here; a, the left end, may be one)
            let sv = poly_eval(c, b)?.0 > 0;
            for _ in 0..200 { let mm = 0.5 * (u + v); if mm <= u || mm >= v { break; } let vm = ev(mm); if vm == 0.0 { u = mm; v = mm; break; } if (vm > 0.0) == sv { v = mm; } else { u = mm; } }
            let r = 0.5 * (u + v);
            // (a rational root, recovered from its float by continued fractions and checked exactly)
            let mut rq = None;
            let (mut h0, mut h1, mut k0, mut k1, mut xv) = (0i128, 1i128, 1i128, 0i128, r);
            for _ in 0..40 {
                let ai = xv.floor();
                if ai.abs() > 1e15 { break; }
                let ai_i = ai as i128;
                let (h2, k2) = (ai_i.checked_mul(h1).and_then(|t| t.checked_add(h0)), ai_i.checked_mul(k1).and_then(|t| t.checked_add(k0)));
                let (Some(h2), Some(k2)) = (h2, k2) else { break };
                if k2 > 1_000_000_000_000 { break; }
                (h0, h1, k0, k1) = (h1, h2, k1, k2);
                // (only a root inside this root's own interval: another root of the polynomial is another line)
                if let Some(q) = Q::new(h1, k1) {
                    let inside = q.sub(a).is_some_and(|d| d.0 > 0) && b.sub(q).is_some_and(|d| d.0 >= 0);
                    if inside && poly_eval(c, q).is_some_and(|v| v.0 == 0) { rq = Some(q); break; }
                }
                let fr = xv - ai;
                if fr.abs() < 1e-18 { break; }
                xv = 1.0 / fr;
            }
            out.push((rq.map_or(r, |q| q.0 as f64 / q.1 as f64), rq, (a.0 as f64 / a.1 as f64, b.0 as f64 / b.1 as f64), (a, b)));
            continue;
        }
        let m = a.add(b)?.mul(Q(1, 2))?;
        stack.push((a, m, depth + 1));
        stack.push((m, b, depth + 1));
    }
    out.sort_by(|p, q| p.0.total_cmp(&q.0));
    // (a root on the end of two intervals is found twice, as the same exact value; two roots the Sturm counts keep apart are
    // never merged, however close: 100 and 100.0000000001 are two lines)
    out.dedup_by(|p, q| p.1.is_some() && p.1 == q.1);
    Some(out)
}

/// The greatest common divisor of two polynomials, made monic (None on overflow).
fn poly_gcd(a: &[Q], b: &[Q]) -> Option<Vec<Q>> {
    let (mut a, mut b) = (trim(a.to_vec()), trim(b.to_vec()));
    while !is_zero_poly(&b) { let r = poly_rem(&a, &b)?; a = b; b = r; }
    let lead = *a.last()?;
    if lead.0 == 0 { return Some(a); }
    a.iter().map(|c| c.div(lead)).collect()
}
/// The polynomial with each of its roots once: c / gcd(c, c′).
fn square_free(c: &[Q]) -> Option<Vec<Q>> {
    let c = trim(c.to_vec());
    if c.len() <= 1 { return Some(c); }
    let d: Vec<Q> = (1..c.len()).map(|k| c[k].mul(Q(k as i128, 1))).collect::<Option<_>>()?;
    let g = poly_gcd(&c, &d)?;
    poly_div(&c, &g)
}

/// The number of distinct real roots of the polynomial in [lo, hi] (Sturm's theorem), or None on overflow.
pub(crate) fn sturm_count(c: &[Q], lo: Q, hi: Q) -> Option<usize> {
    let c = trim(c.to_vec());
    if c.len() == 1 { return if c[0].0 == 0 { None } else { Some(0) }; }
    let d: Vec<Q> = (1..c.len()).map(|k| c[k].mul(Q(k as i128, 1))).collect::<Option<_>>()?;
    let mut seq = vec![c.clone(), trim(d)];
    while seq.last()?.len() > 1 {
        let n = seq.len();
        let r = poly_rem(&seq[n - 2], &seq[n - 1])?;
        if r.len() == 1 && r[0].0 == 0 { break; }
        seq.push(r.iter().map(|q| Q(-q.0, q.1)).collect());
    }
    let changes = |v: Q| -> Option<i32> {
        let s: Vec<i32> = seq.iter().map(|p| poly_eval(p, v).map(|q| q.sign())).collect::<Option<Vec<_>>>()?.into_iter().filter(|s| *s != 0).collect();
        Some(s.windows(2).filter(|w| w[0] != w[1]).count() as i32)
    };
    // roots in (lo, hi], and lo itself when it is a root
    let n = changes(lo)? - changes(hi)?;
    let at_lo = poly_eval(&c, lo)?.0 == 0;
    Some((n.max(0) + at_lo as i32) as usize)
}

/// The coefficients (lowest first) of f(xs, y) as a polynomial in y, exactly, when it is one with rational coefficients.
pub(crate) fn poly_in_y(f: &Node, xs: Q) -> Option<Vec<Q>> {
    // y as the variable of the tree, x as the number xs
    let t = subst(&swap_xy(f), None, Some(&rat_node(xs.0, xs.1)?));
    let d = deg_x(&t)?;
    if d > 12 { return None; }
    let vals: Vec<Q> = (0..=d).map(|k| ex::exact(&t, Some(&Ex::rat(R(k, 1)))).and_then(|v| v.rational()).map(|(p, q)| Q(p, q))).collect::<Option<_>>()?;
    // Newton's divided differences at 0, 1, …, d, then expanded
    let mut dd = vals.clone();
    for j in 1..dd.len() { for i in (j..dd.len()).rev() { dd[i] = dd[i].sub(dd[i - 1])?.div(Q(j as i128, 1))?; } }
    let mut coef = vec![Q(0, 1); dd.len()];
    let mut basis = vec![Q(1, 1)];
    for (i, a) in dd.iter().enumerate() {
        for (k, bk) in basis.iter().enumerate() { coef[k] = coef[k].add(a.mul(*bk)?)?; }
        // basis *= (y − i)
        let mut nb = vec![Q(0, 1); basis.len() + 1];
        for (k, bk) in basis.iter().enumerate() { nb[k + 1] = nb[k + 1].add(*bk)?; nb[k] = nb[k].sub(bk.mul(Q(i as i128, 1))?)?; }
        basis = nb;
    }
    Some(trim(coef))
}
/// The tree with its two variables exchanged (y read as x and x as y).
fn swap_xy(n: &Node) -> Node {
    use Node::*;
    let s = |a: &Node| b(swap_xy(a));
    match n {
        X => Num(SENT, None),
        Num(..) if is_y(n) => X,
        Num(..) | Pi | Euler => n.clone(),
        Neg(a) => Neg(s(a)),
        Func(f, a) => Func(f, s(a)),
        Add(a, c) => Add(s(a), s(c)),
        Sub(a, c) => Sub(s(a), s(c)),
        Mul(a, c) => Mul(s(a), s(c)),
        Div(a, c) => Div(s(a), s(c)),
        Pow(a, c) => Pow(s(a), s(c)),
        LogB(a, c) => LogB(s(a), s(c)),
    }
}

/// One equilibrium solution y = c: its value, its exact value where shown exactly, and the x runs where f is defined on it.
#[derive(Clone, Debug)]
pub(crate) struct Equil { pub y: f64, pub exact: Option<Ex>, pub runs: Vec<(f64, f64)> }
/// The equilibrium solutions in the window: the lines, whether the list is shown to be complete, and whether f is 0
/// everywhere (every horizontal line).
#[derive(Clone, Debug)]
pub(crate) struct Equils { pub lines: Vec<Equil>, pub complete: bool, pub everywhere: bool,
    /** heights the search found f near 0 on, but could not show to be equilibria exactly: never drawn as one */ pub unsure: Vec<f64> }

/// Whether f(x, c) = 0 for every x, shown exactly, for a candidate c: by substitution and exact arithmetic, or, for c = ln n
/// and c = ∛n, which the exact layer does not hold, by putting n for e^y (or exp y) and for y³ (y to a multiple of 3), and
/// then f must have no y left and be 0 for every x: √(2 − eʸ) at ln 2, √(y³ − 2) at ∛2.
pub(crate) fn proves(tree: &Node, c: &Node) -> bool {
    if zero_fn(&subst(tree, None, Some(c))) { return true; }
    fn map(n: &Node, k: &dyn Fn(&Node) -> Option<Node>) -> Node {
        use Node::*;
        if let Some(r) = k(n) { return r; }
        let m = |a: &Node| b(map(a, k));
        match n {
            Neg(a) => Neg(m(a)), Func(f, a) => Func(f, m(a)),
            Add(a, c) => Add(m(a), m(c)), Sub(a, c) => Sub(m(a), m(c)), Mul(a, c) => Mul(m(a), m(c)), Div(a, c) => Div(m(a), m(c)),
            Pow(a, c) => Pow(m(a), m(c)), LogB(a, c) => LogB(m(a), m(c)),
            other => other.clone(),
        }
    }
    let num = |v: i128| Node::Num(v as f64, R::new(v, 1));
    let replaced = match c {
        Node::Func("ln", a) => {
            let Some((nv, 1)) = ex::exact(a, None).and_then(|v| v.rational()) else { return false };
            map(tree, &|t| match t { Node::Pow(e, y) if matches!(**e, Node::Euler) && is_y(y) => Some(num(nv)), Node::Func("exp", y) if is_y(y) => Some(num(nv)), _ => None })
        }
        Node::Pow(a, e) if ex::exact(e, None).and_then(|v| v.rational()) == Some((1, 3)) => {
            let Some((nv, 1)) = ex::exact(a, None).and_then(|v| v.rational()) else { return false };
            map(tree, &|t| match t {
                Node::Pow(y, k) if is_y(y) => match ex::exact(k, None).and_then(|v| v.rational()) { Some((p, 1)) if p > 0 && p % 3 == 0 => nv.checked_pow((p / 3) as u32).map(num), _ => None },
                _ => None,
            })
        }
        _ => return false,
    };
    !has_y(&replaced) && zero_fn(&replaced)
}

/// The exact candidates near a root: p/q, (p/q)√n, (p/q)π (q ≤ 12, n square-free ≤ 50), tried only to be shown exactly.
fn candidates(y: f64) -> Vec<Node> {
    let mut out = vec![];
    for q in 1..=12i128 {
        let p = (y * q as f64).round() as i128;
        if ((p as f64 / q as f64) - y).abs() < 1e-6 { if let Some(n) = rat_node(p, q) { out.push(n); } }
    }
    for n in [2i128, 3, 5, 6, 7, 10, 11, 13, 14, 15, 17, 19, 21, 22, 23, 26, 29, 30, 31, 33, 34, 35, 37, 38, 39, 41, 42, 43, 46, 47] {
        let s = (n as f64).sqrt();
        for q in 1..=12i128 {
            let p = (y / s * q as f64).round() as i128;
            if p != 0 && ((p as f64 / q as f64) * s - y).abs() < 1e-6 { if let Some(c) = rat_node(p, q) { out.push(Node::Mul(b(c), b(Node::Func("sqrt", b(Node::Num(n as f64, R::new(n, 1))))))); } }
        }
    }
    for q in 1..=12i128 {
        let p = (y / PI * q as f64).round() as i128;
        if p != 0 && ((p as f64 / q as f64) * PI - y).abs() < 1e-6 { if let Some(c) = rat_node(p, q) { out.push(Node::Mul(b(c), b(Node::Pi))); } }
    }
    // (fractions of e, as y − e or y(1 − ln y) have them)
    for q in 1..=12i128 {
        let p = (y / EULER * q as f64).round() as i128;
        if p != 0 && ((p as f64 / q as f64) * EULER - y).abs() < 1e-9 * (1.0 + y.abs()) { if let Some(c) = rat_node(p, q) { out.push(Node::Mul(b(c), b(Node::Euler))); } }
    }
    // (quadratic surds (p ± √n)/q, the roots of a quadratic with whole coefficients: (3 ± √5)/2)
    for n in [2i128, 3, 5, 6, 7, 10, 11, 13, 14, 15, 17, 19, 21, 22, 23] {
        let sn = (n as f64).sqrt();
        for q in 1..=12i128 {
            for sg in [1.0, -1.0] {
                let p = (y * q as f64 - sg * sn).round() as i128;
                if p == 0 || ((p as f64 + sg * sn) / q as f64 - y).abs() >= 1e-9 * (1.0 + y.abs()) { continue; }
                let root = Node::Func("sqrt", b(Node::Num(n as f64, R::new(n, 1))));
                let top = if sg > 0.0 { Node::Add(b(Node::Num(p as f64, R::new(p, 1))), b(root)) } else { Node::Sub(b(Node::Num(p as f64, R::new(p, 1))), b(root)) };
                out.push(Node::Div(b(top), b(Node::Num(q as f64, R::new(q, 1)))));
            }
        }
    }
    // (decimals as typed, to 12 places: 0.15, 0.0625, 0.0001, 1.000000001)
    for k in 1..=12u32 {
        let q = 10i128.pow(k);
        let p = (y * q as f64).round() as i128;
        if ((p as f64 / q as f64) - y).abs() < 1e-9 * (1.0 + y.abs()) { if let Some(n) = rat_node(p, q) { out.push(n); } }
    }
    // (the logs and cube roots of small whole numbers, as an edge such as √(2 − eʸ) at ln 2 or √(y³ − 2) at ∛2 has them)
    for n in 2..=10i128 {
        let num = || Node::Num(n as f64, R::new(n, 1));
        for sg in [1.0, -1.0] {
            if ((n as f64).ln() * sg - y).abs() < 1e-9 { let l = Node::Func("ln", b(num())); out.push(if sg < 0.0 { Node::Neg(b(l)) } else { l }); }
            if ((n as f64).cbrt() * sg - y).abs() < 1e-9 { if let Some(t) = rat_node(1, 3) { let c = Node::Pow(b(num()), b(t)); out.push(if sg < 0.0 { Node::Neg(b(c)) } else { c }); } }
        }
    }
    out
}

/// The equilibrium solutions of y′ = f in the window [x0, x1] × [y0, y1].
pub(crate) fn equilibria(f: &F, x0: f64, x1: f64, y0: f64, y1: f64) -> Equils {
    let w = (x1 - x0).max(1e-9);
    let hy = (y1 - y0).max(1e-9);
    // x values that are no grid value (an irregular spread), where every equilibrium must be a root
    let xs: Vec<f64> = (0..401).map(|i| x0 + w * ((i as f64 + 0.5 * ((i * 7919) % 13) as f64 / 13.0) / 401.5)).collect();
    // the size of f: the median |f| over the window (one grid point beside a pole, where |f| is 1e16, must not make every
    // value look small), at least 1
    let scale = {
        let mut v: Vec<f64> = vec![];
        for i in 0..=20 { for j in 0..=20 { if let Some(q) = f.at(x0 + w * i as f64 / 20.0, y0 + hy * j as f64 / 20.0) { v.push(q.abs()); } } }
        v.sort_by(|a, b| a.total_cmp(b));
        v.get(v.len() / 2).copied().unwrap_or(1.0).clamp(1.0, 1e6)
    };
    // the x the roots are first looked for at: a short binary fraction (so its polynomial is exact) about 0.382 across
    let pick = |t: f64| { let v = x0 + t * w; let s = (v * 64.0).round() / 64.0; if s > x0 && s < x1 { s } else { v } };
    let x_try = [pick(0.381_966), pick(0.618_034)];
    // every y with f(x, y) = 0 for the x tried, by sign changes and by touching minima of |f|
    let opens = { let mut v = vec![]; open_guards(&f.tree, &mut v); v };
    let xpows = { let mut v = vec![]; xpow_guards(&f.tree, &mut v); v };
    // A value runs to 0 as y comes to b from one side (`side` points into it): its values 10⁻⁴, 10⁻⁶ and 10⁻⁸ away fall
    // steadily, by the same factor or faster each time, as any power of the distance does (√, ∛, a fourth root, y^π); a
    // pole's grow, a jump's and a constant's stay, and a log's slow fall slows down. None where it has no value there.
    let shrinks = |g: &dyn Fn(f64) -> Option<f64>, b: f64, side: f64| -> Option<bool> {
        let u = 1.0 + b.abs();
        let a = [g(b + side * 1e-4 * u)?.abs(), g(b + side * 1e-6 * u)?.abs(), g(b + side * 1e-8 * u)?.abs()];
        // (values that are 0 or denormal this close are underflow (e^(−1/y) just right of 0, y e^(−y²) far out), not a value
        // running to 0)
        if a.iter().any(|v| *v < 1e-300) { return Some(false); }
        if !(a[0] > a[1] && a[1] > a[2]) { return Some(false); }
        let (r1, r2) = (a[1] / a[0], a[2] / a[1]);
        Some(r1 <= 0.9 && r2 <= 0.9 && r2 <= r1 + 0.05)
    };
    // an open guard (a denominator, a log's inside, a base to a negative power, …) is 0 at (x, y): f has no value there,
    // however close to 0 its float comes beside it. Decided by the guard itself running to 0 there, from the side given (0:
    // from both), or its float being 0 or not finite there: ∛y under ln at y = 0, (1 − y²)^(1/4) under a quotient at ±1
    let open_at = |x: f64, y: f64, side: f64| xpows.iter().filter(|(_, e)| ev(e, x, y) <= 0.0).map(|(q, _)| q).chain(opens.iter()).any(|q| {
        let v = ev(q, x, y);
        if !v.is_finite() || v.abs() <= 1e-12 * (1.0 + y.abs()) { return true; }
        let gq = |t: f64| { let w = ev(q, x, t); w.is_finite().then_some(w) };
        if side != 0.0 { shrinks(&gq, y, side) == Some(true) } else { shrinks(&gq, y, -1.0) == Some(true) && shrinks(&gq, y, 1.0) == Some(true) }
    });
    // f runs to 0 as y comes to the edge b of its domain from the side it has values on, and no open guard is 0 there
    // (√(2 − y²) at √2, where rounding leaves no float on the line with a value; (2 − y) ln(2 − y) at 2 runs to 0 too, but
    // has no value at 2)
    let edge_zero = |x: f64, b: f64, side: f64| -> bool { !open_at(x, b, side) && shrinks(&|t| f.at(x, t), b, side) == Some(true) };
    // f is small at r against its own size beside it: its float there is at most 10⁻⁶ of the larger of its values 10⁻⁴ away
    // (a normal number, not an underflow). Never against a fixed size: 10⁻¹⁰(y − x) or y² + 10⁻⁹ is small everywhere, and is
    // 0 nowhere but on y = x and nowhere at all.
    let small_at = |x: f64, r: f64| -> bool {
        let beside = [-1.0, 1.0].iter().filter_map(|sd: &f64| f.at(x, r + sd * 1e-4 * (1.0 + r.abs()))).map(f64::abs).fold(0.0, f64::max);
        beside >= 1e-300 && f.at(x, r).is_some_and(|v| v.abs() <= 1e-6 * beside)
    };
    // f is 0 at the height r found inside its domain: small against its own size beside it, or running to 0 there from each
    // side it has values on (∛y at 0, whose float at the bracket's end is 10⁻⁵); a pole's sign change grows instead
    let zero_near = |x: f64, r: f64| -> Option<bool> {
        if small_at(x, r) { return Some(true); }
        match (shrinks(&|t| f.at(x, t), r, -1.0), shrinks(&|t| f.at(x, t), r, 1.0)) {
            (Some(false), _) | (_, Some(false)) => Some(false),
            (None, None) => None,
            _ => Some(true),
        }
    };
    let roots_at = |xr: f64| -> Vec<(f64, f64)> {
        let n = 4000;
        let ys: Vec<f64> = (0..=n).map(|j| y0 + hy * j as f64 / n as f64).collect();
        let g: Vec<Option<f64>> = ys.iter().map(|y| f.at(xr, *y)).collect();
        let mut out: Vec<(f64, f64)> = vec![];
        let bis = |mut a: f64, mut c: f64| { let ga = f.at(xr, a).unwrap_or(0.0); for _ in 0..200 { let m = 0.5 * (a + c); match f.at(xr, m) { Some(gm) if gm == 0.0 => return m, Some(gm) if (gm > 0.0) == (ga > 0.0) => a = m, _ => c = m } if c - a <= 1e-15 * (1.0 + a.abs()) { break; } } 0.5 * (a + c) };
        for j in 0..=n {
            // (a sample exactly 0 is a root, unless its neighbour is 0 too: a run of zeros is a value underflowing, e^(−y³) for
            // y past 9, e^(−1/y) just right of 0, not a line of roots)
            match g[j] {
                Some(v) if v == 0.0 && !(j > 0 && g[j - 1] == Some(0.0)) && !(j < n && g[j + 1] == Some(0.0))
                    && [-1.0, 1.0].iter().all(|sd: &f64| f.at(xr, ys[j] + sd * 1e-4 * (1.0 + ys[j].abs())).is_none_or(|w| w.abs() >= 1e-300)) => out.push((ys[j], 0.0)),
                _ => {}
            }
            // a root at the edge of f's domain (√y at y = 0, √(2 − y²) at ±√2, √(cos y) at ±π/2): no sign change and no sample
            // need land on it, and at an irrational edge no float on the line need have a value. The edge is found by halving
            // between a height where f is defined and one where it is not; it is a root where f runs to 0 there (`edge_zero`)
            if j < n && g[j].is_some() != g[j + 1].is_some() {
                let (mut a, mut c) = if g[j].is_some() { (ys[j], ys[j + 1]) } else { (ys[j + 1], ys[j]) };
                for _ in 0..200 { let m = 0.5 * (a + c); if f.at(xr, m).is_some() { a = m; } else { c = m; } if (c - a).abs() <= 1e-15 * (1.0 + a.abs()) { break; } }
                let side = if c > a { -1.0 } else { 1.0 };
                // (the edge itself: a float with a value on it is the edge where there is one, √y at 0; else the halving's end)
                if edge_zero(xr, a, side) { out.push((a, side)); }
            }
            if j < n {
                if let (Some(a), Some(c)) = (g[j], g[j + 1]) {
                    if a != 0.0 && c != 0.0 && (a > 0.0) != (c > 0.0) {
                        let r = bis(ys[j], ys[j + 1]);
                        // a sign change across a pole or a jump is no root
                        if zero_near(xr, r) == Some(true) { out.push((r, 0.0)); }
                    }
                }
            }
            // a touching root (no sign change): a small minimum of |f|, refined by golden section
            if j > 0 && j < n {
                if let (Some(a), Some(m), Some(c)) = (g[j - 1], g[j], g[j + 1]) {
                    if m != 0.0 && m.abs() <= a.abs() && m.abs() <= c.abs() && (a > 0.0) == (m > 0.0) && (c > 0.0) == (m > 0.0) && m.abs() < scale {
                        // (any small local minimum of |f| is looked at: √|y|'s nearest sample to 0 is about 0.02; whether it is a root
                        // is decided at the minimum itself)
                        let (mut lo, mut hi) = (ys[j - 1], ys[j + 1]);
                        let gr = 0.618_033_988_75;
                        for _ in 0..120 {
                            let (p1, p2) = (hi - gr * (hi - lo), lo + gr * (hi - lo));
                            let (v1, v2) = (f.at(xr, p1).map(f64::abs).unwrap_or(f64::MAX), f.at(xr, p2).map(f64::abs).unwrap_or(f64::MAX));
                            if v1 < v2 { hi = p2; } else { lo = p1; }
                        }
                        let r = 0.5 * (lo + hi);
                        // (a root of fractional order touches without a sign change too, √|y| at 0: its float at the minimum is about
                        // 10⁻⁸, and it runs to 0 from both sides)
                        // (but not merely a tiny minimum: y² + 10⁻⁷ is never 0, though its minimum is small beside a large scale)
                        let falls = shrinks(&|t| f.at(xr, t), r, -1.0) == Some(true) && shrinks(&|t| f.at(xr, t), r, 1.0) == Some(true);
                        if small_at(xr, r) || falls { out.push((r, 0.0)); }
                    }
                }
            }
        }
        out.sort_by(|a, c| a.0.total_cmp(&c.0));
        // (a root sampled exactly on an edge is found both ways: the one kept is the one with a value on it)
        out.dedup_by(|a, c| { let same = (a.0 - c.0).abs() < 1e-7 * hy; if same && c.1 != 0.0 && a.1 == 0.0 { *c = *a; } same });
        out
    };
    // f is 0 everywhere: every horizontal line
    if has_no_vars_zero(&f.tree) { return Equils { lines: vec![], complete: true, everywhere: true, unsure: vec![] }; }
    // The exact way, where it can be taken: f(x, y) at three rational x's is a polynomial in y with rational coefficients
    // (y − 10⁻¹⁰x, y² + 10⁻¹⁴, the logistic, (y − 1)(y − x)). A line y = c is an equilibrium only if c is a root at every x,
    // so the lines are the real roots of the exact greatest common divisor of those polynomials (each root once), found
    // where its square-free part changes sign and counted by Sturm's theorem; no size of f decides anything. (A common
    // root at three x's but not at every x needs f's coefficients to share a root at those three x's alone: the lines found
    // are checked at every x below as well.)
    // (f a polynomial in x too, of degree D: a root common to its polynomials in y at D + 1 x's is a root at every x, so a
    // line found so is shown exactly; otherwise three x's narrow the search, and a line is shown by an exact value below)
    let dx = deg_x(&f.tree).filter(|d| *d <= 12);
    let nx = dx.map_or(3, |d| (d as usize + 1).max(3));
    let xq: Vec<f64> = (0..nx).map(|k| if k < 2 { x_try[k] } else { pick(0.137_2 + 0.7 * (k - 2) as f64 / nx as f64) }).collect();
    let polys: Option<Vec<Vec<Q>>> = xq.iter().map(|x| Q::of_f64(*x).and_then(|q| poly_in_y(&f.tree, q))).collect();
    let distinct = { let mut v = xq.clone(); v.sort_by(|a, c| a.total_cmp(c)); v.windows(2).all(|p| p[0] != p[1]) };
    if let Some(polys) = polys.filter(|ps| ps.iter().any(|c| !is_zero_poly(c))) {
        let g = polys.iter().try_fold(vec![Q(0, 1)], |acc, c| poly_gcd(&acc, c));
        let lo = Q::of_f64(((y0 - 1.0 / 64.0) * 64.0).floor() / 64.0);
        let hi = Q::of_f64(((y1 + 1.0 / 64.0) * 64.0).ceil() / 64.0);
        let isolated = g.as_ref().and_then(|g| square_free(g)).zip(lo.zip(hi)).and_then(|(sf, (lo, hi))| isolate_roots(&sf, lo, hi).map(|r| (sf, r)));
        if let Some((sf, roots)) = isolated {
            let proven_all_x = dx.is_some() && distinct;
            let mut lines = vec![];
            let mut unsure = vec![];
            // (inside the window as the figure has it, strictly: 1.000000000001 is not in y 0..1; and a root whose float is the
            // edge's, exactly: 1/3 is not in y 0..0.3333333333333333, whose top is 6004799503160661/2⁵⁴ < 1/3)
            let (e0, e1) = (Q::of_f64_exact(y0), Q::of_f64_exact(y1));
            let count_in = |a: Q, b: Q| -> Option<usize> { Some(sturm_count(&sf, a, b)? - (poly_eval(&sf, a)?.0 == 0) as usize) };
            let in_window = |r: f64, rq: Option<Q>, (a, b): (Q, Q)| -> bool {
                if r < y0 || r > y1 { return false; }
                if r != y0 && r != y1 { return true; }
                let exact = || -> Option<bool> {
                    let (e0, e1) = (e0?, e1?);
                    if let Some(q) = rq { return Some(e0.le(q)? && q.le(e1)?); }
                    // (an irrational root, the only one in (a, b]: at or below the top when (a, top] holds it; at or above the
                    // bottom when (a, bottom) does not)
                    let below_top = if b.le(e1)? { true } else if e1.le(a)? { false } else { count_in(a, e1)? >= 1 };
                    let above_bottom = if e0.le(a)? { true } else if b.sub(e0)?.0 < 0 { false } else { count_in(a, e0)? - (poly_eval(&sf, e0)?.0 == 0) as usize == 0 };
                    Some(below_top && above_bottom)
                };
                exact().unwrap_or(true)
            };
            for (r, rq, (ia, ib), _) in roots.into_iter().filter(|(r, rq, _, iq)| in_window(*r, *rq, *iq)) {
                // (an exact value: the root itself where it is rational, else a short exact value f is 0 on for every x, and
                // only one inside this root's own isolating interval: a neighbouring root's value is another line's)
                let cand = candidates(r).into_iter().filter(|c| { let v = ev(c, 0.0, 0.0); v > ia && v <= ib + 1e-15 * (1.0 + ib.abs()) }).find(|c| proves(&f.tree, c));
                let exact = match rq { Some(q) => Some(Ex::rat(R(q.0, q.1))), None => cand.as_ref().and_then(|c| ex::exact(c, None)) };
                let shown = proven_all_x || cand.is_some();
                let yv = exact.as_ref().map(|e| e.value()).or_else(|| cand.as_ref().map(|c| ev(c, 0.0, 0.0))).unwrap_or(r);
                if !shown { unsure.push(yv); continue; }
                let mut runs: Vec<(f64, f64)> = vec![];
                let m = 480;
                let mut start: Option<f64> = None;
                for i in 0..=m {
                    let x = x0 + w * i as f64 / m as f64;
                    match (f.at(x, yv).is_some(), start) { (true, None) => start = Some(x), (false, Some(s0)) => { runs.push((s0, x0 + w * (i as f64 - 1.0) / m as f64)); start = None; } _ => {} }
                }
                if let Some(s0) = start { runs.push((s0, x1)); }
                lines.push(Equil { y: yv, exact, runs });
            }
            let _ = sf;
            // (two lines written with one value would be one line shown twice: never called complete)
            let twice = lines.windows(2).any(|p| (p[0].y - p[1].y).abs() <= 1e-15 * (1.0 + p[0].y.abs()));
            return Equils { complete: unsure.is_empty() && !twice, lines, everywhere: false, unsure };
        }
    }
    // f(x, y) = 0 for every y at the x tried (y′ = x − 0.37·…): try the other x; when 0 at both, nothing is claimed
    let flat = |xr: f64| (0..=40).all(|j| f.at(xr, y0 + hy * (j as f64 + 0.41) / 41.0) == Some(0.0));
    let Some(xr) = x_try.into_iter().find(|x| !flat(*x)) else { return Equils { lines: vec![], complete: false, everywhere: false, unsure: vec![] } };
    let found = roots_at(xr);
    // a "root" a rounding away from a height where f has no value at the exact x tried is no root: f only comes close to 0
    // there through a factor that cancels (x(y − 1)²/(y − 1), y²/y, sin²y/sin y), and the line itself has no values
    let xe = Q::of_f64(xr).map(|q| Ex::rat(R(q.0, q.1)));
    let gap = |r: f64| -> bool {
        candidates(r).into_iter().any(|c| {
            let Some(cv) = ex::exact(&c, None).map(|e| e.value()) else { return false };
            // (cancellation beside the gap leaves f's float near 0 up to about 10⁻⁸ away: (y² − 2y)/(y − 2) − 2 at 2 + 1.5·10⁻⁸)
            (cv - r).abs() <= 1e-6 * (1.0 + r.abs()) && (f.at(xr, cv).is_none() || xe.as_ref().is_some_and(|x| ex::zero_at(&subst(&f.tree, None, Some(&c)), x)))
        })
    };
    // (and at any height: a root where a denominator, a log's inside or a negative power's base is 0 is no root, (y − 0.15)²/
    // (y − 0.15) at 0.15, (y − e)²/(y − e) at e, whatever the window)
    // (an edge root is no gap: at an irrational edge, √2, the float of the exact value has no value, but the line does)
    let found: Vec<(f64, f64)> = found.into_iter().filter(|r| !(r.1 == 0.0 && gap(r.0)) && !open_at(xr, r.0, r.1)).collect();
    let mut lines = vec![];
    let mut unsure: Vec<f64> = vec![];
    for &(r, side) in &found {
        // f is 0 on the line y = r at every x where it is defined (and defined at most of them); on an edge, f runs to 0 there
        let ok_at = |x: f64| -> Option<bool> {
            // (an edge's own float is a rounding inside it, where √ of the rounding is about 10⁻⁸: the approach decides)
            if side != 0.0 {
                // (at an x where f is exactly 0 beside the edge, x√y at x = 0, the line is no less a solution there)
                let u = 1.0 + r.abs();
                let near: Vec<Option<f64>> = [1e-4, 1e-6, 1e-8].iter().map(|d| f.at(x, r + side * d * u)).collect();
                if near.iter().all(|v| *v == Some(0.0)) && !open_at(x, r, side) { return Some(true); }
                return near[0].map(|_| edge_zero(x, r, side));
            }
            if open_at(x, r, 0.0) { return None; }
            // (an x where f is exactly 0 on the line is on it: x(y − 1)^(1/3) and x√(1 − y²) at x = 0, whether or not the line
            // is an edge that fell on a sampled height; the underflow guard is the search's, at the x it searches)
            if f.at(x, r) == Some(0.0) { return Some(true); }
            // (on the line at this x: f's float there is a rounding of its size beside it, 10⁻⁹ of it for a crossing root
            // found by halving, 10⁻⁶ for a touching one found by golden section; or f keeps falling toward the line from
            // each side, five decades in, down to 10⁻¹² away, as every root of any order does and y + 10⁻¹⁰ sin x does not)
            let u = 1.0 + r.abs();
            let beside = [-1.0, 1.0].iter().filter_map(|sd: &f64| f.at(x, r + sd * 1e-4 * u)).map(f64::abs).fold(0.0, f64::max);
            let touching = match (f.at(xr, r - 1e-4 * u), f.at(xr, r + 1e-4 * u)) { (Some(a), Some(c)) => (a > 0.0) == (c > 0.0), _ => false };
            if let Some(v) = f.at(x, r) { if beside >= 1e-300 && v.abs() <= (if touching { 1e-6 } else { 1e-9 }) * beside { return Some(true); } }
            let deep = |sd: f64| -> Option<bool> {
                let a: Vec<f64> = [1e-4, 1e-6, 1e-8, 1e-10, 1e-12].iter().map(|d| f.at(x, r + sd * d * u).map(f64::abs)).collect::<Option<_>>()?;
                if a.iter().any(|v| *v < 1e-300) { return Some(false); }
                let rs: Vec<f64> = a.windows(2).map(|p| p[1] / p[0]).collect();
                Some(rs.iter().all(|q| *q <= 0.9) && rs.windows(2).all(|p| p[1] <= p[0] + 0.05))
            };
            match (deep(-1.0), deep(1.0)) { (Some(false), _) | (_, Some(false)) => Some(false), (None, None) => None, _ => Some(true) }
        };
        let vals: Vec<Option<bool>> = xs.iter().map(|x| ok_at(*x)).collect();
        let defined = vals.iter().filter(|v| v.is_some()).count();
        // (f is 0 on the line at every one of at least 25 x's where it is defined: a sign change across a pole is no root)
        if defined < 25 || !vals.iter().flatten().all(|ok| *ok) { continue; }
        // an exact value, shown exactly: f(x, c) is 0 for every x by exact arithmetic
        // (a candidate nearer this root than half the way to any other root found: a neighbouring root's exact value, 1/2 beside
        // 0.5000001, is another line's)
        // (and inside the window: a root found alone has no neighbour found, and the root just outside the window, 2 beside
        // 2.0000003 on y 2.0000001..2.0000005, is not this line. A value whose float is the window edge's is inside only when
        // it is exactly a rational the edge holds: 1/3 is outside y 0..0.3333333333333333)
        let gap = found.iter().filter(|o| o.0 != r).map(|o| (o.0 - r).abs()).fold(f64::INFINITY, f64::min);
        let inside = |c: &Node| -> bool {
            let v = ev(c, 0.0, 0.0);
            if !(v >= y0 && v <= y1) { return false; }
            if v != y0 && v != y1 { return true; }
            let q = ex::exact(c, None).and_then(|e| e.rational()).and_then(|(p, q)| Q::new(p, q));
            let edge = Q::of_f64_exact(v);
            matches!((q, edge), (Some(a), Some(b)) if a == b)
        };
        let proof = candidates(r).into_iter().filter(|c| (ev(c, 0.0, 0.0) - r).abs() < 0.5 * gap && inside(c)).find(|c| proves(&f.tree, c));
        let exact = proof.as_ref().and_then(|c| ex::exact(c, None));
        // (a value shown but not held exactly, ln 2: its float, to the last place)
        let yv = exact.as_ref().map(|e| e.value()).or_else(|| proof.as_ref().map(|c| ev(c, 0.0, 0.0))).unwrap_or(r);
        // (a line is drawn as an equilibrium only where it is shown so exactly: f(x, c) ≡ 0 for a value c. A height the search
        // finds f near 0 on, but no exact value shows, is named as possible, never drawn: no size of f decides it)
        if proof.is_none() { unsure.push(r); continue; }
        // the runs of x where f is defined on the line
        let mut runs: Vec<(f64, f64)> = vec![];
        let n = 480;
        let mut start: Option<f64> = None;
        for i in 0..=n {
            let x = x0 + w * i as f64 / n as f64;
            let ok = f.at(x, yv).is_some() || (side != 0.0 && edge_zero(x, r, side)) || (side == 0.0 && zero_near(x, r) == Some(true));
            match (ok, start) { (true, None) => start = Some(x), (false, Some(s)) => { runs.push((s, x0 + w * (i as f64 - 1.0) / n as f64)); start = None; } _ => {} }
        }
        if let Some(s) = start { runs.push((s, x1)); }
        lines.push(Equil { y: yv, exact, runs });
    }
    // complete: f(xr, y) is a polynomial in y whose distinct real roots in the window are exactly the roots found there
    let complete = Q::of_f64(xr).and_then(|q| poly_in_y(&f.tree, q)).zip(Q::of_f64(y0).zip(Q::of_f64(y1)))
        .and_then(|(c, (lo, hi))| sturm_count(&c, lo, hi)).is_some_and(|n| n == found.len());
    let twice = lines.windows(2).any(|p| (p[0].y - p[1].y).abs() <= 1e-15 * (1.0 + p[0].y.abs()));
    Equils { complete: complete && unsure.is_empty() && !twice, lines, everywhere: false, unsure }
}
/// f is the number 0 (no variables at all, or a product with an exact 0).
fn has_no_vars_zero(n: &Node) -> bool { !has_x(n) && !has_y(n) && ex::exact(n, None).is_some_and(|v| v.is_zero()) || matches!(n, Node::Mul(a, c) if has_no_vars_zero(a) || has_no_vars_zero(c)) }

// ───────────────────────── a typed solution, along itself ─────────────────────────

/// Where a typed solution g stands between x0 and x, walked along g itself.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Along {
    /// It is the solution all the way to x.
    Yes,
    /// It is the solution up to x, where f stops being defined on it (a circle meeting the axis).
    End,
    /// It grows past ±1000000 before x (at the first x given).
    Big(f64),
    /// It reaches a point where f, or g, is undefined or not smooth in y before x: the solution is not followed past it.
    Stop(f64),
    /// g′ ≠ f(x, g) at this x, where f is well conditioned: g is not a solution there.
    Fails(f64),
}

/// g′ = f(x, g) at x, to 1e-8 and to what f's own rounding allows there: f's change when x and g move by 1e-13 of themselves,
/// ten times over (next to x = 0 in (y − 2)/x, (x + 2 − 2)/x loses digits; a Taylor polynomial's miss is no rounding).
pub(crate) fn satisfies(f: &F, x: f64, gv: f64, dv: f64) -> Option<bool> {
    let fv = f.at(x, gv)?;
    if !dv.is_finite() { return Some(true); }
    let (ex_, ey) = (1e-13 * (1.0 + x.abs()), 1e-13 * (1.0 + gv.abs()));
    let wob = [(x + ex_, gv), (x - ex_, gv), (x, gv + ey), (x, gv - ey)].iter().filter_map(|(a, c)| f.at(*a, *c)).map(|v| (v - fv).abs()).fold(0.0, f64::max);
    Some((dv - fv).abs() <= 1e-8 * (1.0 + fv.abs()) + 10.0 * wob)
}

/// Walk g from x0 to x at n even steps: g finite and within ±1000000, f defined on it, g′ = f (see `satisfies`), and no
/// denominator or domain of f (or of g) changing sign and no root's base reaching 0 on the way. By uniqueness, a g with
/// g(x0) = y0 that passes is the solution through (x0, y0) on that stretch.
pub(crate) fn along(g: &Node, dg: &Node, f: &F, x0: f64, x: f64, n: usize) -> Along {
    if x == x0 { return Along::Yes; }
    let gf = F::new(g.clone());
    let y0 = ev(g, x0, 0.0);
    let gvals = |t: f64, y: f64| -> Vec<f64> { gf.guards.iter().map(|q| ev(q, t, 0.0)).chain(f.guards.iter().map(|q| ev(q, t, y))).collect() };
    let bvals = |t: f64, y: f64| -> Vec<f64> { f.branch.iter().map(|q| ev(q, t, y)).collect() };
    let mut prev_g = gvals(x0, y0);
    let mut prev_b = bvals(x0, y0);
    // every guard along g, each one's size at the start (its scale), for finding where one touches 0 without changing sign
    let all = |t: f64| -> Option<Vec<f64>> { let y = gf.at(t, 0.0)?; let mut v = gvals(t, y); v.extend(bvals(t, y)); Some(v) };
    let scale0: Vec<f64> = all(x0).unwrap_or_default().iter().map(|v| 1.0 + if v.is_finite() { v.abs() } else { 0.0 }).collect();
    let mut prev_t = x0;
    let mut prev_all = all(x0).unwrap_or_default();
    // A touch can occur before the first ordinary derivative sample, including
    // exactly at the first cell boundary. Check inside the incoming interval.
    let check_approach = |from: f64, to: f64| {
        let mid = from + (to - from) * 0.5;
        match gf.at(mid, 0.0).and_then(|y| satisfies(f, mid, y, ev(dg, mid, 0.0))) {
            Some(true) => None,
            Some(false) => Some(Along::Fails(mid)),
            None => Some(Along::Stop(from)),
        }
    };
    for k in 1..=n {
        let t = if k == n { x } else { x0 + (x - x0) * k as f64 / n as f64 };
        let at_end = k == n;
        // Bracket a crossed formula guard before looking for a tangency. The far end
        // of this cell can already be on a different branch of a reciprocal or tan.
        let mut limit = t;
        for (guard, before) in gf.guards.iter().zip(&prev_g) {
            let after = ev(guard, limit, 0.0);
            if (*before < 0.0 && after > 0.0) || (*before > 0.0 && after < 0.0) {
                let (mut safe, mut beyond) = (prev_t, limit);
                loop {
                    let mid = safe + (beyond - safe) * 0.5;
                    if mid == safe || mid == beyond { break; }
                    let value = ev(guard, mid, 0.0);
                    if value * before.signum() > 0.0 { safe = mid; } else { beyond = mid; }
                }
                limit = safe;
            }
        }
        if limit != t { return Along::Stop(limit); }
        // a guard touching 0 inside this step without changing sign (the base y = (2 − x)² of −2√y at x = 2, 1 − sin²x of
        // √(1 − y²) at π/2): its smallest size in the step, by golden section, within 1e-11 of its scale from 0 ends the walk there
        if let Some(cur_all) = all(t) {
            for (j, (pa, ca)) in prev_all.iter().zip(&cur_all).enumerate() {
                let sc = scale0.get(j).copied().unwrap_or(1.0);
                if !(pa.is_finite() && ca.is_finite()) || pa.abs().min(ca.abs()) > 1e-2 * sc { continue; }
                let b = |u: f64| all(u).and_then(|v| v.get(j).copied()).filter(|v| v.is_finite()).map(f64::abs).unwrap_or(f64::MAX);
                let (mut lo, mut hi) = (prev_t, t);
                let gr = 0.618_033_988_75;
                for _ in 0..90 {
                    let (p1, p2) = (hi - gr * (hi - lo), lo + gr * (hi - lo));
                    if b(p1) < b(p2) { hi = p2; } else { lo = p1; }
                }
                let xs = 0.5 * (lo + hi);
                // (a local minimum strictly inside the step: a guard still falling at the step's end, (x + 1)³ near −1, is
                // left to the sign-change and end tests; 1e-11 of its scale)
                let cell = (t - prev_t).abs();
                let inside = (xs - prev_t).abs() > 1e-6 * cell && (xs - t).abs() > 1e-6 * cell;
                let minimum = b(xs);
                let touch = (inside && minimum < pa.abs().min(ca.abs()) && minimum <= 1e-11 * sc).then_some(xs);
                if let Some(xt) = touch {
                    // (a guard that is 0 at the start itself, x for √x at x = 0, is no end)
                    if (xt - x0).abs() <= 1e-9 * (1.0 + x0.abs()) { continue; }
                    if let Some(stop) = check_approach(prev_t, xt) { return stop; }
                    return if (xt - x).abs() <= 1e-6 * (1.0 + x.abs()) { Along::End } else { Along::Stop(xt) };
                }
            }
            prev_all = cur_all;
        }
        let Some(gv) = gf.at(t, 0.0) else { return Along::Stop(t) };
        if gv.abs() > 1e6 { return Along::Big(t); }
        let cur_g = gvals(t, gv);
        let cur_b = bvals(t, gv);
        if crossed(&prev_b, &cur_b) || cur_b.iter().any(|v| v.abs() <= 1e-12 * (1.0 + y0.abs())) {
            if let Some(stop) = check_approach(prev_t, t) { return stop; }
            return if at_end { Along::End } else { Along::Stop(t) };
        }
        if crossed(&prev_g, &cur_g) && !at_end { return Along::Stop(t); }
        match satisfies(f, t, gv, ev(dg, t, 0.0)) {
            None => return if at_end { Along::End } else { Along::Stop(t) },
            Some(false) => return Along::Fails(t),
            Some(true) => {}
        }
        prev_t = t;
        prev_g = cur_g;
        prev_b = cur_b;
    }
    Along::Yes
}
