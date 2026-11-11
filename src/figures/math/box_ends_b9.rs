//! Box plot, batch 9: how each end of the number line finishes (the reference's Left end and Right end), in the
//! coordinate grid's house style (`grid_b7::draw_axes`): a filled triangle, an open line arrow, an open circle, or a
//! plain end at the last tick. Both ends at Triangle arrow, the default, draw through the old `axis_heads` exactly as
//! every box plot did before.
use super::*;

pub(crate) const BOX_ENDS: [&str; 4] = ["Triangle arrow", "Line arrow", "Circle", "None"];

pub(super) fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("left_end", "Left end", &BOX_ENDS, "Triangle arrow"),
        ParamDef::choice("right_end", "Right end", &BOX_ENDS, "Triangle arrow"),
    ]
}

fn end_of(p: &Params, key: &str) -> &'static str {
    BOX_ENDS.iter().copied().find(|e| *e == p.s(key)).unwrap_or("Triangle arrow")
}

/// The two ends as drawn, None when both are the default triangles (the old drawing).
pub(crate) fn ends(p: &Params) -> Option<[&'static str; 2]> {
    let e = [end_of(p, "left_end"), end_of(p, "right_end")];
    (e != ["Triangle arrow"; 2]).then_some(e)
}

/// The run of line past each end tick (22 px, as the triangles always had), none at a plain end.
fn ext(kind: &str) -> f64 { if kind == "None" { 0.0 } else { 22.0 } }

/// The number line's ends past the ticks at x0 and x1 (the line between them is drawn by the caller).
pub(super) fn draw(d: &mut Doc, p: &Params, x0: f64, x1: f64, y: f64) {
    let Some(e) = ends(p) else {
        axis_heads(d, x0, x1, y);
        return;
    };
    for (kind, xe, dx) in [(e[0], x0, -1.0), (e[1], x1, 1.0)] {
        let x = xe + dx * ext(kind);
        if kind != "None" { d.line(xe.min(x), y, xe.max(x), y, INK, 2.4); }
        match kind {
            // the triangle the line always had: 13 px long, its tip 2.6 px past the line's end
            "Triangle arrow" => { d.poly(&[(x + dx * 2.6, y), (x - dx * 10.4, y - 6.5), (x - dx * 10.4, y + 6.5)], INK, INK, 1.0); }
            // an open chevron of two strokes meeting at the line's end
            "Line arrow" => { d.raw(&format!(
                r##"<polyline points="{:.2},{:.2} {x:.2},{y:.2} {:.2},{:.2}" fill="none" stroke="{INK}" stroke-width="2.2" stroke-linejoin="miter" stroke-linecap="round" data-end="line"/>"##,
                x - dx * 10.0, y - 7.0, x - dx * 10.0, y + 7.0
            ));
            }
            // an open circle beyond the line's end, the line meeting its edge
            "Circle" => { d.raw(&format!(r##"<circle cx="{:.2}" cy="{y:.2}" r="5" fill="#fff" stroke="{INK}" stroke-width="2" data-end="circle"/>"##, x + dx * 5.0)); }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::figures::{registry, start_params, Params, Value};

    fn svg(over: &[(&str, &str)]) -> String {
        let d = registry().into_iter().find(|d| d.id == "box-plot").unwrap();
        let mut p: Params = start_params(&d);
        p.0.insert("data".into(), Value::S("2, 4, 5, 5, 6, 7, 8, 9, 10".into()));
        for (k, v) in over { p.0.insert(k.to_string(), Value::S(v.to_string())); }
        p.normalize(&(d.params)(), false);
        d.svg(&p)
    }

    /// x positions of the drawn end marks: (kind, centre or tip x)
    fn marks(s: &str) -> Vec<(&'static str, f64)> {
        let num = |t: &str, k: &str| -> Option<f64> { let i = t.find(&format!(" {k}=\""))? + k.len() + 3; t[i..].split('"').next()?.parse().ok() };
        let mut out = vec![];
        for el in s.split('<') {
            if el.starts_with("polygon") {
                let pts = el.split("points=\"").nth(1).unwrap().split('"').next().unwrap();
                let first: f64 = pts.split([',', ' ']).next().unwrap().parse().unwrap();
                out.push(("triangle", first));
            } else if el.starts_with("polyline") && el.contains("data-end=\"line\"") {
                let pts = el.split("points=\"").nth(1).unwrap().split('"').next().unwrap();
                let mid: f64 = pts.split(' ').nth(1).unwrap().split(',').next().unwrap().parse().unwrap();
                out.push(("line", mid));
            } else if el.starts_with("circle") && el.contains("data-end=\"circle\"") {
                out.push(("circle", num(el, "cx").unwrap()));
            }
        }
        out
    }

    #[test]
    fn box_plot_ends_draw_each_choice_at_its_own_end() {
        let base = svg(&[]);
        assert_eq!(svg(&[("left_end", "Triangle arrow"), ("right_end", "Triangle arrow")]), base, "the default is the old drawing");
        let mut seen = vec![base.clone()];
        for l in super::BOX_ENDS {
            for r in super::BOX_ENDS {
                let s = svg(&[("left_end", l), ("right_end", r)]);
                if (l, r) != ("Triangle arrow", "Triangle arrow") {
                    assert!(!seen.contains(&s), "{l} / {r} draws like another choice");
                    seen.push(s.clone());
                }
                let m = marks(&s);
                let left: Vec<_> = m.iter().filter(|(_, x)| *x < 338.0).collect();
                let right: Vec<_> = m.iter().filter(|(_, x)| *x > 338.0).collect();
                let want = |k: &str| match k { "Triangle arrow" => Some("triangle"), "Line arrow" => Some("line"), "Circle" => Some("circle"), _ => None };
                assert_eq!(left.iter().map(|x| x.0).collect::<Vec<_>>(), want(l).into_iter().collect::<Vec<_>>(), "{l} at the left");
                assert_eq!(right.iter().map(|x| x.0).collect::<Vec<_>>(), want(r).into_iter().collect::<Vec<_>>(), "{r} at the right");
                // each mark sits past its own end tick (38 and 638), inside the 676 page
                for (k, x) in &m {
                    let past = if *x < 338.0 { 38.0 - x } else { x - 638.0 };
                    assert!(past >= 20.0 && past <= 28.0, "{k} at {x}");
                    let r = if *k == "circle" { 5.0 } else { 0.0 };
                    assert!(x - r > 0.0 && x + r < 676.0, "{k} off the page");
                }
            }
        }
    }

    #[test]
    fn box_plot_plain_ends_stop_at_the_end_ticks() {
        let s = svg(&[("left_end", "None"), ("right_end", "None")]);
        assert!(!s.contains("<polygon"), "no arrowheads");
        // no line runs past the end ticks at 38 and 638
        for el in s.split("<line").skip(1) {
            let f = |k: &str| -> f64 { el.split(&format!("{k}=\"")).nth(1).unwrap().split('"').next().unwrap().parse().unwrap() };
            for k in ["x1", "x2"] { assert!(f(k) >= 37.9 && f(k) <= 638.1, "a line reaches {} past the ends", f(k)); }
        }
    }

    #[test]
    fn box_plot_ends_show_on_the_blank_number_line_too() {
        let d = registry().into_iter().find(|d| d.id == "box-plot").unwrap();
        let mut p = start_params(&d);
        let a = d.svg(&p);
        p.set_s("right_end", "Circle");
        assert_ne!(d.svg(&p), a, "the opening state (no data) shows the end");
        assert!(d.svg(&p).contains("data-end=\"circle\""));
    }
}
