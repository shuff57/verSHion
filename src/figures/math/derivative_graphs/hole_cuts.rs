//! Find denominator cuts independently of the outer spelling. Exact inverse operations first; numerical
//! bracketing is a fallback, with the denominator retained as an algebraic zero for Taylor arithmetic.
use super::*;
use sym::{Ex::*, F};

pub(super) struct Cut { pub x: f64, pub exact: Option<XV>, pub zero: Option<Ex> }
fn roots(e: &Ex, t: Ex, lo: f64, hi: f64, depth: usize) -> Vec<XV> {
    if depth > 16 { return vec![]; }
    let eq = sym::sub(e.clone(), t.clone());
    if let Some(z) = exact::zeros(&eq, lo, hi) { return z; }
    let constant = |e: &Ex| !sym::has_x(e);
    let r = |e: &Ex, t| roots(e, t, lo, hi, depth + 1);
    match e {
        Neg(a) => r(a, sym::neg(t)),
        Add(a,b) if constant(b) => r(a, sym::sub(t, (**b).clone())),
        Add(a,b) if constant(a) => r(b, sym::sub(t, (**a).clone())),
        Sub(a,b) if constant(b) => r(a, sym::add(t, (**b).clone())),
        Sub(a,b) if constant(a) => r(b, sym::sub((**a).clone(), t)),
        Mul(a,b) if constant(b) && sym::ev(b, 0.0) != 0.0 => r(a, sym::div(t, (**b).clone())),
        Mul(a,b) if constant(a) && sym::ev(a, 0.0) != 0.0 => r(b, sym::div(t, (**a).clone())),
        Div(a,b) if constant(b) => r(a, sym::mul(t, (**b).clone())),
        Fun(F::Sqrt, a) if sym::ev(&t, 0.0) >= 0.0 => r(a, sym::pow(t, Num(2.0))),
        Fun(F::Abs, a) if sym::ev(&t, 0.0) >= 0.0 => { let mut z = r(a, t.clone()); z.extend(r(a, sym::neg(t))); z },
        Fun(F::Ln, a) => r(a, Pow(Box::new(E), Box::new(t))),
        Fun(F::Exp, a) if sym::ev(&t, 0.0) > 0.0 => r(a, Fun(F::Ln, Box::new(t))),
        Pow(a,b) if constant(b) => {
            let k = sym::konst(b).unwrap_or(0.0);
            if k == 0.0 { return vec![]; }
            let target = if let Pow(base, exponent) = &t {
                if exponent == b { (**base).clone() } else { sym::pow(t.clone(), sym::div(Num(1.0), (**b).clone())) }
            } else { sym::pow(t.clone(), sym::div(Num(1.0), (**b).clone())) };
            let mut z = r(a, target.clone());
            if sym::ratio(k, 32).is_some_and(|(p,_)| p % 2 == 0) { z.extend(r(a, sym::neg(target))); }
            z.into_iter().filter(|x| match (exact::ev(e, x), exact::ev(&t, x)) { (Some(a),Some(b)) => a == b, _ => true }).collect()
        }
        _ => vec![],
    }
}
fn denominators(e: &Ex, out: &mut Vec<Ex>) {
    match e {
        Div(a,b) => { out.push((**b).clone()); denominators(a,out); denominators(b,out); }
        Pow(a,b) => { if sym::konst(b).is_some_and(|v| v < 0.0) { out.push((**a).clone()); } denominators(a,out); denominators(b,out); }
        Neg(a) | Fun(_,a) => denominators(a,out),
        Add(a,b) | Sub(a,b) | Mul(a,b) | LogB(a,b) => { denominators(a,out); denominators(b,out); }
        _ => {}
    }
}
fn quotients(e: &Ex, out: &mut Vec<(Ex,Ex)>) {
    match e {
        Div(a,b) => { out.push(((**a).clone(),(**b).clone())); quotients(a,out); quotients(b,out); }
        Neg(a) | Fun(_,a) => quotients(a,out),
        Add(a,b) | Sub(a,b) | Mul(a,b) | Pow(a,b) | LogB(a,b) => { quotients(a,out); quotients(b,out); }
        _=>{}
    }
}
pub(super) fn find(e: &Ex, lo: f64, hi: f64) -> Vec<Cut> {
    let mut cuts: Vec<Cut> = exact::special_points(e, lo, hi).into_iter().map(|x| Cut { x: x.f(), exact: Some(x), zero: None }).collect();
    // A common zero may be easier to derive from the numerator than from the denominator. For example,
    // (x²−2)/sin(x²−2) gives ±sqrt 2 directly, without solving a nonlinear trigonometric equation.
    let mut qs=vec![];
    quotients(e,&mut qs);
    for (n,d) in qs {
        for x in roots(&n,Num(0.0),lo,hi,0) {
            if cuts.iter().any(|c|(c.x-x.f()).abs()<1e-12*(1.0+c.x.abs())) { continue; }
            let dz=exact::ev(&d,&x);
            let (v,err)=sym::ev_err(&d,x.f());
            if dz.as_ref().is_some_and(|v|v.is_zero()) || (dz.is_none() && v.is_finite() && v.abs()<=16.0*err) {
                cuts.push(Cut { x:x.f(),exact:Some(x),zero:if dz.is_none(){Some(n.clone())}else{None} });
            }
        }
    }
    let mut ds = vec![];
    denominators(e, &mut ds);
    ds.dedup();
    for d in ds {
        let zs = roots(&d, Num(0.0), lo, hi, 0);
        for x in &zs {
            if !cuts.iter().any(|c| (c.x - x.f()).abs() < 1e-12 * (1.0 + c.x.abs())) {
                let proven = exact::ev(&d,x).is_some_and(|v| v.is_zero());
                let (v,err) = sym::ev_err(&d,x.f());
                if proven || v.abs() <= 16.0*err {
                    cuts.push(Cut { x: x.f(), exact: Some(x.clone()), zero: if proven { None } else { Some(d.clone()) } });
                }
            }
        }
        // Exact algebra covers ordinary root, power, abs and trig forms. For other analytic denominators,
        // bracket sign changes and touching zeros of d via its turning points. No fixed sample is declared a zero.
        if !zs.is_empty() || exact::zeros(&d, lo, hi).is_some() { continue; }
        let dd = sym::deriv(&d);
        let value = |x| { let v = sym::ev(&d,x); v.is_finite().then_some(v) };
        let slope = |x| { let v = sym::ev(&dd,x); v.is_finite().then_some(v) };
        let events = find_events(&value, &slope, &[], lo, hi).0;
        for ev in events.into_iter().filter(|v| v.k == EvK::Zero).take(64) {
            if cuts.iter().any(|c| (c.x-ev.x).abs() < 1e-8 * (1.0+c.x.abs())) { continue; }
            let (v,err) = sym::ev_err(&d,ev.x);
            let bound = err + sym::ev(&dd,ev.x).abs() * f64::EPSILON * (1.0+ev.x.abs()) * 16.0;
            if v.abs() <= bound { cuts.push(Cut { x: ev.x, exact: None, zero: Some(d.clone()) }); }
        }
    }
    cuts.sort_by(|a,b| a.x.total_cmp(&b.x));
    cuts
}
