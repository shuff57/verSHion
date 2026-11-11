//! Waves, batch 9: end caps on the axes (the reference's x-axis Left end / Right end and y-axis Bottom end / Top
//! end): none, a filled triangle, an open line arrow or an open circle, in the coordinate grid's house style
//! (`math::grid_b7::draw_axes`: the axis runs 20 px past the graph, a triangle's tip 2.6 px further, a circle beyond
//! the line's end). An end takes room only when it is set, so every wave with plain ends, the default, is laid out
//! and drawn exactly as before.
use super::*;

pub(crate) const ENDS: [&str; 4] = ["None", "Triangle arrow", "Line arrow", "Circle"];
/// x left, x right, y bottom, y top.
pub(crate) const KEYS: [&str; 4] = ["x_left_end", "x_right_end", "y_bottom_end", "y_top_end"];

pub(super) fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice(KEYS[0], "Left end", &ENDS, "None"),
        ParamDef::choice(KEYS[1], "Right end", &ENDS, "None"),
        ParamDef::choice(KEYS[2], "Bottom end", &ENDS, "None"),
        ParamDef::choice(KEYS[3], "Top end", &ENDS, "None"),
    ]
}

/// Are the x-axis ends drawn? Only on an axis line: the graph's axes (numbered or gridded) or the distance axis under
/// a longitudinal wave alone (the bare rest line with neither has no ends).
pub(crate) fn x_applies(p: &Params) -> bool { p.s("kind") == "Longitudinal" || p.b("numbers") || p.b("grid") }
/// Are the y-axis ends drawn? Only on the transverse graph's y-axis.
pub(crate) fn y_applies(p: &Params) -> bool { p.s("kind") != "Longitudinal" && (p.b("numbers") || p.b("grid")) }

/// The panel shows an end's control exactly when the end can be drawn.
pub(crate) fn relevant(key: &str, p: &Params) -> bool {
    match key {
        "x_left_end" | "x_right_end" => x_applies(p),
        "y_bottom_end" | "y_top_end" => y_applies(p),
        _ => true,
    }
}

/// The four ends as drawn (None where the axis has no ends).
pub(crate) fn kinds(p: &Params) -> [&'static str; 4] {
    let mut out = ["None"; 4];
    for (i, k) in KEYS.iter().enumerate() {
        let on = if i < 2 { x_applies(p) } else { y_applies(p) };
        if on { out[i] = ENDS.iter().copied().find(|e| *e == p.s(k)).unwrap_or("None"); }
    }
    out
}

/// How far a drawn end reaches past the graph: the 20 px run, a triangle's tip, a circle's width.
pub(crate) fn reach(kind: &str) -> f64 {
    match kind { "Triangle arrow" => 22.6, "Line arrow" => 20.0, "Circle" => 30.0, _ => 0.0 }
}

/// The room each end takes (x left, x right, y bottom, y top); all 0 for plain ends.
pub(crate) fn room(p: &Params) -> [f64; 4] { kinds(p).map(reach) }

/// The ends of the x-axis (at height `axy`, from `lft` to `rgt`) and of the y-axis (at `axx`, from `bt` up to `tp`);
/// `y_on` false for the longitudinal wave's distance axis alone.
pub(super) fn draw(g: &mut G, k: [&str; 4], (lft, rgt, axy): (f64, f64, f64), (axx, bt, tp): (f64, f64, f64), axw: f64) {
    for (kind, (x, y), (dx, dy)) in [(k[0], (lft, axy), (-1.0, 0.0)), (k[1], (rgt, axy), (1.0, 0.0)), (k[2], (axx, bt), (0.0, 1.0)), (k[3], (axx, tp), (0.0, -1.0))] {
        if kind == "None" { continue; }
        let (ex, ey) = (x + dx * 20.0, y + dy * 20.0);
        g.line(x, y, ex, ey, INK, axw);
        match kind {
            "Triangle arrow" => {
                let (tx, ty) = (ex + dx * 2.6, ey + dy * 2.6);
                let (bx, by) = (ex - dx * 10.4, ey - dy * 10.4);
                g.raw(&format!(r##"<polygon points="{:.2},{ty:.2} {:.2},{:.2} {:.2},{:.2}" fill="{INK}" data-end="triangle"/>"##, g.x(tx), g.x(bx - dy * 6.5), by + dx * 6.5, g.x(bx + dy * 6.5), by - dx * 6.5));
            }
            "Line arrow" => {
                let (bx, by) = (ex - dx * 10.0, ey - dy * 10.0);
                g.raw(&format!(r##"<polyline points="{:.2},{:.2} {:.2},{ey:.2} {:.2},{:.2}" fill="none" stroke="{INK}" stroke-width="{:.2}" stroke-linejoin="miter" stroke-linecap="round" data-end="line"/>"##, g.x(bx - dy * 7.0), by + dx * 7.0, g.x(ex), g.x(bx + dy * 7.0), by - dx * 7.0, axw * 0.9));
            }
            _ => {
                g.raw(&format!(r##"<circle cx="{:.2}" cy="{:.2}" r="5" fill="#fff" stroke="{INK}" stroke-width="2" data-end="circle"/>"##, g.x(ex + dx * 5.0), ey + dy * 5.0));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::figures::{registry, start_params, FigureDef, Params, Value};
    type Bx = (f64, f64, f64, f64);

    fn def() -> FigureDef { registry().into_iter().find(|d| d.id == "waves").unwrap() }
    fn params(over: &[(&str, Value)]) -> Params {
        let d = def();
        let mut p = start_params(&d);
        for (k, v) in over { p.0.insert(k.to_string(), v.clone()); }
        p.normalize(&(d.params)(), false);
        p
    }
    fn s(v: &str) -> Value { Value::S(v.into()) }

    fn walk(g: &resvg::usvg::Group, out: &mut Vec<(String, Bx)>) {
        for n in g.children() {
            match n {
                resvg::usvg::Node::Group(g) => walk(g, out),
                resvg::usvg::Node::Text(t) => {
                    let r = t.flattened().bounding_box().transform(t.abs_transform()).unwrap_or(t.flattened().abs_bounding_box());
                    let txt: String = t.chunks().iter().map(|c| c.text()).collect();
                    out.push((txt, (r.left() as f64, r.top() as f64, r.right() as f64, r.bottom() as f64)));
                }
                _ => {}
            }
        }
    }
    /// Text ink boxes from the real renderer, in the SVG's own units.
    fn ink_texts(svg: &str) -> Vec<(String, Bx)> {
        let mut opt = resvg::usvg::Options::default();
        opt.fontdb = crate::figures::fontdb();
        let tree = resvg::usvg::Tree::from_str(&crate::figures::serif_stack(svg), &opt).expect("the SVG parses");
        let mut v = Vec::new();
        walk(tree.root(), &mut v);
        v.retain(|(t, bx)| !t.trim().is_empty() && bx.2 > bx.0);
        v
    }
    fn attr(el: &str, k: &str) -> Option<String> { Some(el.split(&format!(" {k}=\"")).nth(1)?.split('"').next()?.to_string()) }
    fn pts(el: &str) -> Vec<(f64, f64)> {
        attr(el, "points").unwrap().split_whitespace().map(|q| { let (a, b) = q.split_once(',').unwrap(); (a.parse().unwrap(), b.parse().unwrap()) }).collect()
    }
    /// The drawn end marks: (kind, ink box).
    fn marks(svg: &str) -> Vec<(String, Bx)> {
        let mut out = vec![];
        for el in svg.split('<') {
            let Some(kind) = attr(el, "data-end") else { continue };
            let b = if el.starts_with("circle") {
                let (cx, cy): (f64, f64) = (attr(el, "cx").unwrap().parse().unwrap(), attr(el, "cy").unwrap().parse().unwrap());
                (cx - 6.0, cy - 6.0, cx + 6.0, cy + 6.0)
            } else {
                let p = pts(el);
                let f = |g: fn(f64, f64) -> f64, i: usize| p.iter().map(|q| if i == 0 { q.0 } else { q.1 }).fold(if g(0.0, 1.0) == 0.0 { f64::MAX } else { f64::MIN }, g);
                (f(f64::min, 0) - 1.0, f(f64::min, 1) - 1.0, f(f64::max, 0) + 1.0, f(f64::max, 1) + 1.0)
            };
            out.push((kind, b));
        }
        out
    }
    fn hit(a: Bx, b: Bx) -> bool { a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3 }
    fn page(svg: &str) -> (f64, f64) {
        let vb = attr(svg, "viewBox").unwrap();
        let v: Vec<f64> = vb.split_whitespace().map(|x| x.parse().unwrap()).collect();
        (v[2], v[3])
    }

    #[test]
    fn waves_plain_ends_are_the_old_drawing() {
        let d = def();
        let base = d.svg(&params(&[]));
        for k in super::KEYS { assert_eq!(d.svg(&params(&[(k, s("None"))])), base); }
        assert!(!base.contains("data-end"));
    }

    #[test]
    fn waves_each_end_draws_its_own_mark_and_each_choice_differs() {
        let d = def();
        for kind in ["Transverse", "Both", "Longitudinal"] {
            let mut seen: Vec<String> = vec![d.svg(&params(&[("kind", s(kind))]))];
            for (i, k) in super::KEYS.iter().enumerate() {
                for e in &super::ENDS[1..] {
                    let p = params(&[("kind", s(kind)), (k, s(e))]);
                    let svg = d.svg(&p);
                    let applies = if i < 2 { super::x_applies(&p) } else { super::y_applies(&p) };
                    assert_eq!(crate::figures::param_relevant("waves", k, &p), applies);
                    if !applies {
                        assert_eq!(svg, seen[0], "{kind}: {k} is offered nowhere it draws nothing");
                        continue;
                    }
                    assert!(!seen.contains(&svg), "{kind}: {k} = {e} draws like another choice");
                    seen.push(svg.clone());
                    let m = marks(&svg);
                    let want = match *e { "Triangle arrow" => "triangle", "Line arrow" => "line", _ => "circle" };
                    assert_eq!(m.len(), 1, "{kind} {k} {e}");
                    assert_eq!(m[0].0, want);
                    // at its own end of its own axis
                    let l = super::super::wave_layout(&p);
                    let b = m[0].1;
                    let (cx, cy) = ((b.0 + b.2) / 2.0, (b.1 + b.3) / 2.0);
                    match (l.panel.as_ref(), i) {
                        (Some(ax), 0) => assert!(b.2 < ax.x0 && (cy - ax.py(0.0)).abs() < 1.0, "{kind} {k}"),
                        (Some(ax), 1) => assert!(b.0 > ax.x0 + ax.w && (cy - ax.py(0.0)).abs() < 1.0, "{kind} {k}"),
                        (Some(ax), 2) => assert!(b.1 > ax.y0 + ax.h && (cx - ax.x0).abs() < 1.0, "{kind} {k}"),
                        (Some(ax), _) => assert!(b.3 < ax.y0 && (cx - ax.x0).abs() < 1.0, "{kind} {k}"),
                        (None, 0) => assert!(b.2 < l.sx.x0, "{kind} {k}"),
                        (None, _) => assert!(b.0 > l.sx.x0 + l.sx.w, "{kind} {k}"),
                    }
                }
            }
        }
    }

    #[test]
    fn waves_ends_are_offered_exactly_where_they_draw() {
        let d = def();
        for kind in ["Transverse", "Both", "Longitudinal"] {
            for (num, grid) in [(true, true), (true, false), (false, true), (false, false)] {
                let base = [("kind", s(kind)), ("numbers", Value::B(num)), ("grid", Value::B(grid))];
                let a = d.svg(&params(&base));
                for k in super::KEYS {
                    let mut o = base.to_vec();
                    o.push((k, s("Circle")));
                    let p = params(&o);
                    assert_eq!(crate::figures::param_relevant("waves", k, &p), d.svg(&p) != a, "{kind} numbers {num} grid {grid}: {k}");
                }
            }
        }
    }

    /// Crowded and odd layouts: every end mark and the run of axis to it clear of every word and number as drawn,
    /// everything inside the page.
    #[test]
    fn waves_end_caps_clear_of_text_and_inside_the_page() {
        let d = def();
        let mut n = 0;
        for kind in ["Transverse", "Both", "Longitudinal"] {
            for against in ["Distance", "Time"] {
                for (num, grid) in [(true, true), (true, false), (false, true)] {
                    for ls in ["Medium", "Large"] {
                        for range in 0..3 {
                            for ends in [["Circle"; 4], ["Triangle arrow"; 4], ["Line arrow"; 4], ["Circle", "Triangle arrow", "Line arrow", "Circle"]] {
                                let mut o = vec![("kind", s(kind)), ("against", s(against)), ("numbers", Value::B(num)), ("grid", Value::B(grid)), ("label_size", s(ls)),
                                    ("mark_ct", Value::B(true)), ("title", s("Waves on a rope")), ("xt", s("Distance along the rope (m)")), ("yt", s("Displacement (cm)"))];
                                match range {
                                    1 => o.extend([("yaxis_fit", Value::B(false)), ("ymin", Value::F(1.0)), ("ymax", Value::F(6.0)), ("ystep", Value::F(1.0)), ("yaxis_num", s("Every line"))]),
                                    2 => o.extend([("xaxis_fit", Value::B(false)), ("xmin", Value::F(-2.0)), ("xmax", Value::F(8.0)), ("xstep", Value::F(1.0)), ("xaxis_num", s("Every line")),
                                        ("yaxis_fit", Value::B(false)), ("ymin", Value::F(-6.0)), ("ymax", Value::F(4.0)), ("ystep", Value::F(2.0))]),
                                    _ => {}
                                }
                                for (k, e) in super::KEYS.iter().zip(ends) { o.push((k, s(e))); }
                                let p = params(&o);
                                let svg = d.svg(&p);
                                let (w, h) = page(&svg);
                                let l = super::super::wave_layout(&p);
                                let texts = ink_texts(&svg);
                                // nothing goes off the page that was on it with plain ends (a manual y range that leaves
                                // the trough out already pushed its label off the page before batch 9)
                                let off = |t: &[(String, Bx)], w: f64, h: f64| t.iter().filter(|(_, b)| !(b.0 >= -0.5 && b.2 <= w + 0.5 && b.1 >= -0.5 && b.3 <= h + 0.5)).map(|x| x.0.clone()).collect::<Vec<_>>();
                                let plain = { let mut q = p.clone(); for k in super::KEYS { q.set_s(k, "None"); } d.svg(&q) };
                                let (pw, ph) = page(&plain);
                                let was = off(&ink_texts(&plain), pw, ph);
                                for t in off(&texts, w, h) { assert!(was.contains(&t), "{t:?} pushed off the page in {o:?}"); }
                                // and no words or numbers moved onto each other
                                let pairs = |t: &[(String, Bx)]| { let mut v = vec![]; for i in 0..t.len() { for j in i + 1..t.len() { if hit(t[i].1, t[j].1) { v.push((t[i].0.clone(), t[j].0.clone())); } } } v };
                                let before = pairs(&ink_texts(&plain));
                                // (a label that was already off the page with plain ends, the old manual-range case, is left out)
                                for pr in pairs(&texts) { assert!(before.contains(&pr) || was.contains(&pr.0) || was.contains(&pr.1), "{pr:?} overlap in {o:?}"); }
                                for (kd, b) in marks(&svg) {
                                    assert!(b.0 >= 0.0 && b.2 <= w && b.1 >= 0.0 && b.3 <= h, "{kd} off the page in {o:?}");
                                    // the mark as drawn, and the run of axis (2.4 px wide) from the graph out to it
                                    let (cx, cy) = ((b.0 + b.2) / 2.0, (b.1 + b.3) / 2.0);
                                    let run = match l.panel.as_ref() {
                                        Some(ax) => {
                                            let (lft, rgt, tp, bt) = (ax.x0, ax.x0 + ax.w, ax.y0, ax.y0 + ax.h);
                                            if b.2 < lft { (b.0, cy - 1.7, lft, cy + 1.7) } else if b.0 > rgt { (rgt, cy - 1.7, b.2, cy + 1.7) } else if b.1 > bt { (cx - 1.7, bt, cx + 1.7, b.3) } else { (cx - 1.7, b.1, cx + 1.7, tp) }
                                        }
                                        None => if b.2 < l.sx.x0 { (b.0, cy - 1.7, l.sx.x0, cy + 1.7) } else { (l.sx.x0 + l.sx.w, cy - 1.7, b.2, cy + 1.7) },
                                    };
                                    for (t, tb) in &texts { assert!(!hit(b, *tb), "{kd} end on {t:?} {tb:?} (mark {b:?}) in {o:?}"); }
                                    for (t, tb) in &texts { assert!(!hit(run, *tb), "{kd} end runs into {t:?} {tb:?} (mark {run:?}) in {o:?}"); }
                                }
                                n += 1;
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(n, 432);
    }

    #[test]
    fn waves_axis_summaries_name_the_end_caps() {
        let p = params(&[("x_right_end", s("Triangle arrow")), ("y_top_end", s("Circle"))]);
        let x = super::super::wave_summary("x-axis", &p).unwrap();
        let y = super::super::wave_summary("y-axis", &p).unwrap();
        assert!(x.ends_with("\u{b7} triangle arrow at the right"), "{x}");
        assert!(y.ends_with("\u{b7} circle at the top"), "{y}");
        let plain = params(&[]);
        assert!(!super::super::wave_summary("x-axis", &plain).unwrap().contains(" at the "));
    }

    #[test]
    fn waves_titles_and_numbers_move_clear_of_the_left_and_bottom_caps() {
        let d = def();
        let p = params(&[("x_left_end", s("Circle")), ("y_bottom_end", s("Circle")), ("yt", s("Displacement (cm)"))]);
        let l = super::super::wave_layout(&p);
        let ax = l.panel.unwrap();
        let svg = d.svg(&p);
        let t = ink_texts(&svg);
        let yt = t.iter().find(|x| x.0 == "Displacement (cm)").unwrap().1;
        assert!(yt.2 < ax.x0 - 30.0, "the y-axis title stays left of the circle: {yt:?}");
        let _ = start_params(&def());
    }
}
