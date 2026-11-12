//! Projectile motion, batch 9: the maximum height, range and cliff height marks each get a label mode, Text, Blank
//! line (a line for students to write the label on) or None, as the reference's Marks rows and the figure's other
//! labels do. Text, the default, draws the typed label exactly as before.
use super::*;

/// (mark switch, label text key, label mode key)
pub(crate) const MARKS: [(&str, &str, &str); 3] = [("mark_h", "h_lab", "h_lab_mode"), ("mark_r", "r_lab", "r_lab_mode"), ("mark_cliff", "cliff_lab", "cliff_lab_mode")];

pub(super) fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("h_lab_mode", "Height label shows", &PMODES, "Text"),
        ParamDef::choice("r_lab_mode", "Range label shows", &PMODES, "Text"),
        ParamDef::choice("cliff_lab_mode", "Cliff height label shows", &PMODES, "Text"),
    ]
}

/// The label a mark draws: its typed text, "" for a blank line, "none" for no label (what `G::label_at` reads).
pub(super) fn label(p: &Params, text_key: &str, mode_key: &str) -> String {
    match p.s(mode_key) {
        "Blank line" => String::new(),
        "None" => "none".into(),
        _ => p.s(text_key).to_string(),
    }
}

/// Is the mark drawn? (The maximum height needs an upward launch, the cliff's height a cliff.)
pub(crate) fn mark_drawn(p: &Params, mark: &str) -> bool {
    match mark {
        "mark_h" => p.b("mark_h") && proj_geo(p, 640.0).vy > 0.05,
        "mark_cliff" => p.b("mark_cliff") && p.s("from") == "A cliff",
        _ => p.b(mark),
    }
}

/// The panel offers a mark's label mode while the mark is drawn, and its text while that text is shown.
pub(crate) fn relevant(key: &str, p: &Params) -> bool {
    for (mark, text, mode) in MARKS {
        if key == mode { return mark_drawn(p, mark); }
        if key == text { return mark_drawn(p, mark) && !matches!(p.s(mode), "Blank line" | "None"); }
    }
    true
}

/// Where the height mark's blank line goes, as the centre `label_at` takes (the line is 26 px long, 6.65 px under it):
/// 6 px right of the height arrow at its middle, as before, when that is clear of every stroke and ball by 4 px; else
/// the nearest clear spot beside the arrow, right or left, up and down its length; else just outside the arch on the
/// right (a steep launch's path hugs the arrow on both sides). `arrow` is (x, top, bottom).
pub(crate) fn height_blank_spot(arrow: (f64, f64, f64), strokes: &[Vec<Pt>], balls: &[(Pt, f64)], w: f64) -> (f64, f64) {
    let (ax, top, bot) = arrow;
    let drop = 19.0 * 0.35;
    let seg_d = |q: Pt, a: Pt, b: Pt| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let l2 = (dx * dx + dy * dy).max(1e-12);
        let u = (((q.0 - a.0) * dx + (q.1 - a.1) * dy) / l2).clamp(0.0, 1.0);
        ((q.0 - a.0 - dx * u).powi(2) + (q.1 - a.1 - dy * u).powi(2)).sqrt()
    };
    let clear = |cx: f64, cy: f64| {
        let y = cy + drop;
        if cx - 13.0 < 2.0 || cx + 13.0 > w - 2.0 { return false; }
        (0..=26).all(|i| {
            let q = (cx - 13.0 + i as f64, y);
            strokes.iter().all(|s| s.windows(2).all(|v| seg_d(q, v[0], v[1]) >= 4.0)) && balls.iter().all(|(c, r)| ((q.0 - c.0).powi(2) + (q.1 - c.1).powi(2)).sqrt() >= r + 4.0)
        })
    };
    let mid = (top + bot) / 2.0;
    if clear(ax + 19.0, mid) { return (ax + 19.0, mid); }
    let half = ((bot - top) / 2.0 - 12.0).max(0.0);
    let mut k = 0.0;
    while k <= half {
        for cy in [mid + k, mid - k] {
            for cx in [ax + 19.0, ax - 19.0] { if clear(cx, cy) { return (cx, cy); } }
        }
        k += 3.0;
    }
    // outside the arch: right of everything drawn across the middle row
    let y = mid + drop;
    let mut x = ax + 19.0;
    while x + 13.0 < w - 2.0 && !clear(x, mid) { x += 2.0; }
    if clear(x, mid) { return (x, mid); }
    let _ = y;
    (ax + 19.0, mid)
}

#[cfg(test)]
mod tests {
    use crate::figures::{registry, start_params, FigureDef, Params, Value};

    fn def() -> FigureDef { registry().into_iter().find(|d| d.id == "projectile-motion").unwrap() }
    fn params(over: &[(&str, Value)]) -> Params {
        let d = def();
        let mut p = start_params(&d);
        for (k, v) in over { p.0.insert(k.to_string(), v.clone()); }
        p.normalize(&(d.params)(), false);
        p
    }
    fn s(v: &str) -> Value { Value::S(v.into()) }
    fn b(v: bool) -> Value { Value::B(v) }
    fn all_marks() -> Vec<(&'static str, Value)> {
        vec![("from", s("A cliff")), ("mark_h", b(true)), ("mark_r", b(true)), ("mark_cliff", b(true)), ("h_lab", s("Hx")), ("r_lab", s("Rq")), ("cliff_lab", s("Kc"))]
    }
    /// Text elements as (x, y, content).
    fn texts(svg: &str) -> Vec<(f64, f64, String)> {
        let mut out = vec![];
        for part in svg.split("<text").skip(1) {
            let head = part.split('>').next().unwrap();
            let f = |k: &str| -> f64 { head.split(&format!(" {k}=\"")).nth(1).unwrap().split('"').next().unwrap().parse().unwrap() };
            let body = part.split_once('>').unwrap().1.split("</text>").next().unwrap();
            let mut t = String::new();
            let mut inside = false;
            for c in body.chars() { match c { '<' => inside = true, '>' => inside = false, _ if !inside => t.push(c), _ => {} } }
            out.push((f("x"), f("y"), t));
        }
        out
    }
    /// Short horizontal lines 26 px long: the blank lines `label_at` draws.
    fn blanks(svg: &str) -> Vec<(f64, f64)> {
        let mut out = vec![];
        for el in svg.split("<line").skip(1) {
            let f = |k: &str| -> f64 { el.split(&format!(" {k}=\"")).nth(1).unwrap().split('"').next().unwrap().parse().unwrap() };
            let (x1, y1, x2, y2) = (f("x1"), f("y1"), f("x2"), f("y2"));
            if (y1 - y2).abs() < 1e-6 && ((x2 - x1).abs() - 26.0).abs() < 1e-6 { out.push(((x1 + x2) / 2.0, y1)); }
        }
        out
    }

    #[test]
    fn projectile_marks_text_is_the_old_drawing() {
        let d = def();
        let base = d.svg(&params(&all_marks()));
        let mut o = all_marks();
        o.extend([("h_lab_mode", s("Text")), ("r_lab_mode", s("Text")), ("cliff_lab_mode", s("Text"))]);
        assert_eq!(d.svg(&params(&o)), base);
    }

    #[test]
    fn projectile_mark_labels_blank_or_none_each_on_its_own_mark() {
        let d = def();
        for mirror in [false, true] {
            for (i, (_, text, mode)) in super::MARKS.iter().enumerate() {
                let typed = ["Hx", "Rq", "Kc"][i];
                let mut o = all_marks();
                o.push(("mirror", b(mirror)));
                let full = d.svg(&params(&o));
                let at = texts(&full).into_iter().find(|t| t.2 == typed).unwrap_or_else(|| panic!("{typed} drawn"));
                let mut blank = o.clone();
                blank.push((mode, s("Blank line")));
                let bs = d.svg(&params(&blank));
                let mut none = o.clone();
                none.push((mode, s("None")));
                let ns = d.svg(&params(&none));
                assert!(bs != full && ns != full && bs != ns, "{mode}: each choice draws differently");
                // the label's text is gone from the drawing, and from the SVG's title and description
                for svg in [&bs, &ns] {
                    assert!(!texts(svg).iter().any(|t| t.0 == at.0 && t.1 == at.1), "{mode}: the typed {text} is still drawn");
                    assert_eq!(texts(svg).len(), texts(&full).len() - 1, "{mode}: only its own label goes");
                }
                // the blank line sits where the label was (centred under it), and only one more line is drawn
                let nb = blanks(&bs);
                let before = blanks(&full);
                let new: Vec<_> = nb.iter().filter(|q| !before.contains(q)).collect();
                assert_eq!(new.len(), 1, "{mode}: one blank line");
                assert!((new[0].0 - at.0).abs() <= 5.0 && (new[0].1 - at.1).abs() < 8.0, "{mode}: the blank line is at its own mark ({:?} vs {:?})", new[0], (at.0, at.1));
                // clear of every other line: its nearest stroke is at least 3 px away, and it is inside the page
                let (bx, by) = *new[0];
                assert!(bx - 13.0 >= 1.0 && bx + 13.0 <= 639.0, "{mode}: inside the page");
                for el in bs.split("<line").skip(1).chain(bs.split("<polyline").skip(1)) {
                    let nums: Vec<f64> = if el.contains("points=\"") {
                        el.split("points=\"").nth(1).unwrap().split('"').next().unwrap().split([' ', ',']).filter(|x| !x.is_empty()).map(|x| x.parse().unwrap()).collect()
                    } else {
                        ["x1", "y1", "x2", "y2"].iter().map(|k| el.split(&format!(" {k}=\"")).nth(1).unwrap().split('"').next().unwrap().parse().unwrap()).collect()
                    };
                    for seg in nums.chunks(2).collect::<Vec<_>>().windows(2) {
                        let (p1, p2) = ((seg[0][0], seg[0][1]), (seg[1][0], seg[1][1]));
                        if (p1.1 - by).abs() < 1e-6 && (p2.1 - by).abs() < 1e-6 && ((p1.0 + p2.0) / 2.0 - bx).abs() < 1e-6 { continue; } // itself
                        // distance from the segment to the blank line (sampled)
                        let mut dmin = f64::MAX;
                        for i in 0..=40 {
                            let t = i as f64 / 40.0;
                            let q = (p1.0 + (p2.0 - p1.0) * t, p1.1 + (p2.1 - p1.1) * t);
                            let dx = (q.0 - bx).abs().max(13.0) - 13.0;
                            dmin = dmin.min((dx * dx + (q.1 - by).powi(2)).sqrt());
                        }
                        assert!(dmin >= 3.0, "{mode}: the blank line touches another stroke {p1:?}-{p2:?} (mirror {mirror})");
                    }
                }
                assert!(blanks(&ns).len() == before.len(), "{mode}: None draws no line");
            }
        }
    }

    #[test]
    fn projectile_mark_label_modes_offered_where_they_draw() {
        let d = def();
        let cases: Vec<Vec<(&str, Value)>> = vec![
            vec![],
            all_marks(),
            vec![("mark_h", b(true))],
            vec![("mark_cliff", b(true))],
            vec![("from", s("A cliff")), ("angle", Value::F(0.0)), ("mark_h", b(true)), ("mark_cliff", b(true)), ("mark_r", b(true))],
            vec![("from", s("A cliff")), ("mark_cliff", b(true)), ("cliff_lab_mode", s("None"))],
        ];
        for c in cases {
            let p = params(&c);
            let a = d.svg(&p);
            for (_, text, mode) in super::MARKS {
                for v in ["Blank line", "None", "Text"] {
                    if p.s(mode) == v { continue; }
                    let mut q = p.clone();
                    q.set_s(mode, v);
                    assert_eq!(crate::figures::param_relevant("projectile-motion", mode, &p), d.svg(&q) != a, "{mode} -> {v} in {c:?}");
                }
                let mut q = p.clone();
                q.set_s(text, "Zz");
                assert_eq!(crate::figures::param_relevant("projectile-motion", text, &p), d.svg(&q) != a, "{text} in {c:?}");
            }
        }
    }

    #[test]
    fn projectile_marks_summary_says_blank_line() {
        let mut o = all_marks();
        o.extend([("h_lab_mode", s("Blank line")), ("r_lab_mode", s("None"))]);
        let p = params(&o);
        let t = crate::figures::physics::skin_summary("projectile-motion", "Marks", &p).unwrap();
        assert_eq!(t, "maximum height blank line \u{b7} range \u{b7} cliff height \u{201c}Kc\u{201d}");
    }

    /// The height mark's blank line keeps clear of the dashed path, the balls and the launch arrow (4 px, read from the
    /// drawn SVG) at steep launches (75 to 89 degrees) and shallow ones (5 to 15) at every ball size, from the ground and
    /// off a cliff, mirrored or not; it stays beside its own arrow, nearer it than the range mark.
    #[test]
    fn projectile_height_blank_line_clear_of_the_path() {
        let d = def();
        let mut n = 0;
        for from in ["Level ground", "A cliff"] {
            for size in [30, 60, 100, 150] {
                for angle in (75..=89).chain(5..=15) {
                    for (steps, mirror) in [(0, false), (5, false), (0, true)] {
                        let mut o = vec![("from", s(from)), ("angle", Value::F(angle as f64)), ("size", Value::I(size)), ("mark_h", b(true)), ("mark_r", b(true)),
                            ("h_lab_mode", s("Blank line")), ("r_lab_mode", s("Blank line")), ("steps", Value::I(steps)), ("mirror", b(mirror))];
                        let svg = d.svg(&params(&o));
                        o.retain(|x| x.0 != "h_lab_mode");
                        o.push(("h_lab_mode", s("None")));
                        let none = d.svg(&params(&o));
                        let before = blanks(&none);
                        let new: Vec<_> = blanks(&svg).into_iter().filter(|q| !before.contains(q)).collect();
                        assert_eq!(new.len(), 1, "{from} {angle} {size}: one height blank line");
                        let (bx, by) = new[0];
                        // the dashed path (the long polyline) and the balls, as drawn
                        let path: Vec<(f64, f64)> = svg.split("<polyline").skip(1).filter_map(|el| {
                            let el = el.split('>').next().unwrap();
                            let ps: Vec<(f64, f64)> = el.split("points=\"").nth(1)?.split('"').next()?.split_whitespace().map(|q| { let (a, c) = q.split_once(',').unwrap(); (a.parse().unwrap(), c.parse().unwrap()) }).collect();
                            el.contains("stroke-dasharray=\"7 6\"").then_some(ps)
                        }).flatten().collect();
                        assert!(!path.is_empty(), "{from} {angle} {size} {steps}: the dashed path");
                        let dist = |q: (f64, f64)| ((q.0 - bx).abs().max(13.0) - 13.0).hypot(q.1 - by);
                        let dmin = path.windows(2).flat_map(|w| (0..=10).map(move |i| { let t = i as f64 / 10.0; (w[0].0 + (w[1].0 - w[0].0) * t, w[0].1 + (w[1].1 - w[0].1) * t) })).map(dist).fold(f64::MAX, f64::min);
                        assert!(dmin >= 3.0, "{from} {angle} deg size {size} steps {steps} mirror {mirror}: the height blank line is {dmin:.1} px from the path");
                        for el in svg.split("<circle").skip(1) {
                            let f = |k: &str| -> f64 { el.split(&format!(" {k}=\"")).nth(1).unwrap().split('"').next().unwrap().parse().unwrap() };
                            let c = (f("cx"), f("cy"));
                            assert!(dist(c) >= f("r") + 3.0, "{from} {angle} size {size}: on a ball");
                        }
                        assert!(bx - 13.0 >= 0.0 && bx + 13.0 <= 640.0);
                        n += 1;
                    }
                }
            }
        }
        assert_eq!(n, 2 * 4 * 26 * 3);
    }
}
