//! Batch 10 on the pulley: the second object touching the first on a table or ramp, and the contact forces.
use crate::figures::{interact, notes_for, param_relevant, registry, start_params, FigureDef, Params, Value};

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == "pulley").unwrap() }

fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
fn i(k: &'static str, v: i64) -> (&'static str, Value) { (k, Value::I(v)) }
fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }

fn with(list: &[(&'static str, Value)]) -> Params {
    let d = def();
    let defs = (d.params)();
    let mut p = start_params(&d);
    for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&defs, false);
    p
}

fn svg(p: &Params) -> String { def().svg(p) }

fn touching(setup: &str) -> Vec<(&'static str, Value)> { vec![s("setup", setup), b("second", true), s("joined", "Touching")] }

// ------------------------------------------------------------------ the physics, by hand

const G: f64 = 9.8;

/// Newton's second law for each body, solved as a linear system (no closed form used): the tied object `m1` on the
/// surface at `th` degrees with kinetic friction `mu`, the object `m3` in front of it (pushed by it), the hanging `m2`.
/// `toward` = they move toward the pulley (friction then acts away from it). Returns (a toward the pulley, T, P).
fn solve(m1: f64, m3: f64, m2: f64, th: f64, mu: f64, toward: bool) -> (f64, f64, f64) {
    let (sn, cs) = (th.to_radians().sin(), th.to_radians().cos());
    let fr = if toward { 1.0 } else { -1.0 }; // friction along "away from the pulley" when moving toward it
    // unknowns x = (a, T, P); rows: m3 a = P - m3 g sin - fr mu m3 g cos
    //                              m1 a = T - P - m1 g sin - fr mu m1 g cos
    //                              m2 a = m2 g - T
    let m = [[m3, 0.0, -1.0], [m1, -1.0, 1.0], [m2, 1.0, 0.0]];
    let r = [-m3 * G * (sn + fr * mu * cs), -m1 * G * (sn + fr * mu * cs), m2 * G];
    let det = |m: [[f64; 3]; 3]| m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]) + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    let d = det(m);
    let col = |c: usize| { let mut x = m; for k in 0..3 { x[k][c] = r[k]; } det(x) / d };
    (col(0), col(1), col(2))
}

#[test]
fn the_contact_force_from_newtons_second_law_by_hand() {
    // worked on paper, g = 9.8 m/s^2: tied m1, pushed m3 in front, hanging m2
    // table, smooth, 2 kg / 1 kg / 3 kg: a = 3g / 6 = 4.9, P = m3 a = 4.9
    let close = |a: f64, b: f64| (a - b).abs() < 1e-3;
    let (a, t, p) = solve(2.0, 1.0, 3.0, 0.0, 0.0, true);
    assert!(close(a, 4.9) && close(p, 4.9) && close(t, 3.0 * (G - 4.9)), "{a} {t} {p}");
    // table, rough mu = 0.2: a = (3 - 0.2*3) g / 6 = 3.92, P = m3 (a + mu g) = 3.92 + 1.96 = 5.88
    let (a, _, p) = solve(2.0, 1.0, 3.0, 0.0, 0.2, true);
    assert!(close(a, 3.92) && close(p, 5.88), "{a} {p}");
    // ramp 30 degrees, smooth: a = (3 - 3 sin 30) g / 6 = 2.45, P = m3 (a + g sin 30) = 2.45 + 4.9 = 7.35
    let (a, _, p) = solve(2.0, 1.0, 3.0, 30.0, 0.0, true);
    assert!(close(a, 2.45) && close(p, 7.35), "{a} {p}");
    // ramp 30 degrees, mu = 0.2, up the ramp: sin + mu cos = 0.67321, a = (3 - 3 * 0.67321) g / 6 = 1.60128,
    // P = 1 * (1.60128 + 9.8 * 0.67321) = 8.19872
    let (a, _, p) = solve(2.0, 1.0, 3.0, 30.0, 0.2, true);
    assert!(close(a, 1.60128) && close(p, 8.19872), "{a} {p}");
    // ramp 30 degrees, mu = 0.1, sliding DOWN (tied 4 kg, front 2 kg, hanging 1 kg): sin - mu cos = 0.41340,
    // a down = (6 * 0.41340 - 1) g / 7 = 2.07258, P = 2 (g * 0.41340 - 2.07258) = 3.95751
    let (a, _, p) = solve(4.0, 2.0, 1.0, 30.0, 0.1, false);
    assert!(close(-a, 2.07258) && close(p, 3.95751), "{a} {p}");
    // the closed form the figure's doc states, m3 m2 g (1 + sin +- mu cos) / (m1 + m2 + m3), over a sweep; and the
    // force is a push (> 0) every time, so the pair is drawn the right way round whichever way they move
    let mut n = 0;
    for m1 in [0.5, 1.0, 2.0, 5.0, 20.0] {
        for m3 in [0.5, 1.0, 3.0, 10.0] {
            for m2 in [0.2, 1.0, 4.0, 15.0] {
                for th in [0.0, 10.0, 15.0, 30.0, 45.0, 60.0] {
                    for mu in [0.0, 0.1, 0.3, 0.6] {
                        for toward in [true, false] {
                            let (_, t, p) = solve(m1, m3, m2, th, mu, toward);
                            let (sn, cs) = (f64::to_radians(th).sin(), f64::to_radians(th).cos());
                            let pm = if toward { 1.0 } else { -1.0 };
                            let want = m3 * m2 * G * (1.0 + sn + pm * mu * cs) / (m1 + m2 + m3);
                            assert!((p - want).abs() < 1e-9 * want.max(1.0), "{m1} {m3} {m2} {th} {mu} {toward}: {p} vs {want}");
                            assert!(p > 0.0 && t > 0.0, "a push and a taut string: {p} {t}");
                            n += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(n, 3840);
}

#[test]
fn an_object_behind_the_tied_one_feels_no_contact_force() {
    // why the touching object goes in front: put m3 BEHIND the tied m1 and suppose they stay together. Then the
    // force m1 would have to put on m3, toward the pulley, comes out positive every time: a pull, which an object
    // touching m3's front can never give (it can only push m3 back). So they part and the contact force is zero.
    for (th, mu, toward) in [(0.0, 0.0, true), (0.0, 0.3, true), (30.0, 0.0, true), (30.0, 0.2, true), (30.0, 0.1, false), (45.0, 0.3, false)] {
        let (m1, m3, m2) = (2.0, 1.0, 3.0);
        let (sn, cs) = (f64::to_radians(th).sin(), f64::to_radians(th).cos());
        let fr = if toward { 1.0 } else { -1.0 };
        // together: (m1 + m3 + m2) a = m2 g - (m1 + m3) g (sin + fr mu cos); on m3 alone the push from m1 (toward the
        // pulley, positive) is m3 a + m3 g (sin + fr mu cos)
        let a = (m2 * G - (m1 + m3) * G * (sn + fr * mu * cs)) / (m1 + m2 + m3);
        let push = m3 * a + m3 * G * (sn + fr * mu * cs);
        // m1 is ahead: it can only pull m3 along (push > 0) through a string, never by touching its back
        assert!(push > 0.0, "the rear object would need pulling: {th} {mu} {toward}");
    }
}

// ------------------------------------------------------------------ the drawing

const TEAL: &str = "#0e7c8b"; // PAL[5]: the contact forces' colour for slides

fn num(tag: &str, k: &str) -> f64 {
    let i = tag.find(&format!(" {k}=\"")).unwrap() + k.len() + 3;
    tag[i..].split('"').next().unwrap().parse().unwrap()
}

/// Elements of the SVG whose tag starts with `name`.
fn tags<'a>(t: &'a str, name: &str) -> Vec<&'a str> {
    t.match_indices(&format!("<{name} ")).map(|(i, _)| { let e = t[i..].find('>').unwrap(); &t[i..i + e + 1] }).collect()
}

/// The contact arrows' heads (tip, then the base corners) in colour.
fn pair_heads(t: &str) -> Vec<Vec<(f64, f64)>> {
    tags(t, "polygon").into_iter().filter(|h| h.contains(&format!("fill=\"{TEAL}\""))).map(|h| {
        let i = h.find("points=\"").unwrap() + 8;
        h[i..].split('"').next().unwrap().split_whitespace().map(|q| { let (x, y) = q.split_once(',').unwrap(); (x.parse().unwrap(), y.parse().unwrap()) }).collect()
    }).collect()
}

#[test]
fn touching_draws_the_second_object_in_front_and_the_pair_the_right_way() {
    for setup in ["Table", "Ramp"] {
        for mirror in [false, true] {
            let base = with(&touching(setup));
            let tied = with(&[s("setup", setup), b("second", true)]);
            assert_ne!(svg(&base), svg(&tied), "{setup}: touching draws differently from tied");
            let mut l = touching(setup);
            l.extend([b("contact", true), b("color", true), b("mirror", mirror)]);
            let t = svg(&with(&l));
            let heads = pair_heads(&t);
            assert_eq!(heads.len(), 2, "{setup}: one pair");
            // the pulley is to the right (left when mirrored): one head points toward it, one away
            let dir = |h: &Vec<(f64, f64)>| h[0].0 - (h[1].0 + h[2].0) / 2.0;
            let toward = if mirror { -1.0 } else { 1.0 };
            let (fwd, back): (Vec<_>, Vec<_>) = heads.iter().partition(|h| dir(h) * toward > 0.0);
            assert_eq!((fwd.len(), back.len()), (1, 1), "{setup}: equal and opposite");
            // the forward one (on the second object) lies nearer the pulley than the backward one (on the first)
            assert!((fwd[0][0].0 - back[0][0].0) * toward > 0.0, "{setup} {mirror}: the pair points apart");
            // one label names the pair, as on the inclined plane
            let labs = texts(&t).into_iter().filter(|&(a, b)| plain(&t[a..b]) == "P").count();
            assert_eq!(labs, 1, "{setup}: one label for the pair");
        }
    }
}

#[test]
fn the_pair_is_where_they_touch() {
    // the pair's two heads straddle the place the two objects meet: the second object's back face
    for setup in ["Table", "Ramp"] {
        for kind in ["Block", "Cart"] {
            for size in [50, 100, 150] {
                let mut l = touching(setup);
                l.extend([b("contact", true), b("color", true), s("l_kind", kind), i("l_size", size)]);
                let p = with(&l);
                // the smallest carts have no room inside for their labels: refused (too_big_to_fit_is_refused...)
                let Some(tc) = super::pulley::touch(&p) else { assert!(kind == "Cart" && size == 50, "{setup} {kind} {size} fits"); continue };
                let t = svg(&p);
                let heads = pair_heads(&t);
                // project onto the surface: the interface is at s1 + hl along it
                let th = if setup == "Ramp" { p.f("angle").to_radians() } else { 0.0 };
                let (ax, ground) = if setup == "Ramp" { (40.0, 420.0) } else { (50.0, 230.0) };
                let along = |q: (f64, f64)| (q.0 - ax) * th.cos() - (q.1 - ground) * th.sin();
                let up = |q: (f64, f64)| -(q.0 - ax) * th.sin() - (q.1 - ground) * th.cos();
                let i0 = tc.s1 + tc.hl;
                let tips: Vec<f64> = heads.iter().map(|h| along(h[0]) - i0).collect();
                assert!(tips.iter().any(|d| *d > 8.0) && tips.iter().any(|d| *d < -8.0), "{setup} {kind} {size}: {tips:?}");
                assert!(tips.iter().all(|d| d.abs() < 40.0), "{setup} {kind} {size}: the pair stays at the contact {tips:?}");
                for h in &heads {
                    let u = up(h[0]);
                    assert!(u > tc.top + 4.0 && u < tc.h - 8.0, "{setup} {kind} {size}: between the tops and the string ({u})");
                }
            }
        }
    }
}

/// States for "shown == changes the drawing": the table and ramp with and without each switch, sizes that fit and
/// sizes that do not, and the other setups.
fn states() -> Vec<Vec<(&'static str, Value)>> {
    let mut out = vec![];
    for setup in ["Atwood", "Table", "Ramp", "Block & tackle"] {
        for second in [false, true] {
            for joined in ["By a string", "Touching"] {
                for contact in [false, true] {
                    for mode in ["Text", "Blank line", "None"] {
                        for (kind, size, angle) in [("Block", 100, 30.0), ("Cart", 200, 60.0), ("Block", 200, 60.0), ("Cart", 50, 15.0)] {
                            out.push(vec![s("setup", setup), b("second", second), s("joined", joined), b("contact", contact), s("cont_lab_mode", mode), s("l_kind", kind), i("l_size", size), f("angle", angle), b("normal", true)]);
                        }
                    }
                }
            }
        }
    }
    out
}

#[test]
fn each_new_control_is_shown_exactly_where_it_changes_the_drawing() {
    let d = def();
    let defs = (d.params)();
    let mut n = 0;
    for st in states() {
        let p = with(&st);
        let t = svg(&p);
        for key in ["joined", "contact", "cont_lab_mode", "cont_lab", "s_lab"] {
            let pd = defs.iter().find(|x| x.key == key).unwrap();
            let alts: Vec<Value> = match &pd.kind {
                crate::figures::Kind::Bool => vec![Value::B(!p.b(key))],
                crate::figures::Kind::Choice(o) => o.iter().filter(|x| **x != p.s(key)).map(|x| Value::S(x.to_string())).collect(),
                _ => vec![Value::S("Q_x".into()), Value::S(String::new())],
            };
            let changes = alts.into_iter().any(|v| { let mut q = p.clone(); q.0.insert(key.into(), v); svg(&q) != t });
            assert_eq!(param_relevant("pulley", key, &p), changes, "{key} over {st:?}");
            n += 1;
        }
    }
    assert!(n > 1500, "{n}");
}

#[test]
fn old_controls_still_act_while_touching() {
    // every force switch and the labels still change the touching drawing, so none is hidden there
    for setup in ["Table", "Ramp"] {
        let mut l = touching(setup);
        l.push(b("contact", true));
        let t = svg(&with(&l));
        for (k, v) in [b("tension", true), b("gravity", true), b("normal", true), s("friction", "Toward the pulley"), s("friction", "Away from it"), s("accel", "Right falls"), s("accel", "Left falls"), s("surface", "Rough"), s("l_kind", "Cart"), i("l_size", 70), s("l_lab", "M"), s("s_lab", "m_9"), b("mirror", true), b("color", true)] {
            let mut q = l.clone();
            q.push((k, v.clone()));
            assert_ne!(svg(&with(&q)), t, "{setup}: {k} = {v:?} acts");
            assert!(param_relevant("pulley", k, &with(&q)), "{setup}: {k} shown");
        }
    }
}

#[test]
fn too_big_to_fit_is_refused_with_a_note_in_the_panel_only() {
    let big = |setup: &str, kind: &str, size: i64, angle: f64| { let mut l = touching(setup); l.extend([s("l_kind", kind), i("l_size", size), f("angle", angle), b("contact", true)]); with(&l) };
    // refused: two 200% carts on the table, two 200% blocks on a 60 degree ramp
    // and too small: two 50% carts, whose 15 px bodies hold no label of 8 px or more with room round it
    for (setup, kind, size, angle) in [("Table", "Cart", 200, 30.0), ("Ramp", "Block", 200, 60.0), ("Ramp", "Cart", 180, 45.0), ("Table", "Cart", 50, 30.0), ("Ramp", "Cart", 50, 15.0)] {
        let p = big(setup, kind, size, angle);
        assert!(super::pulley::touch(&p).is_none(), "{setup} {kind} {size} {angle} refused");
        let notes = notes_for("pulley", &p);
        assert_eq!(notes.len(), 1, "{notes:?}");
        let t = svg(&p);
        // drawn as the first object alone, exactly
        let mut alone = p.clone();
        alone.set_b("second", false);
        assert_eq!(t, svg(&alone), "{setup}: the second is left off");
        for w in notes[0].split_whitespace().filter(|w| w.len() > 4) {
            assert!(!t.contains(w.trim_matches(|c: char| !c.is_alphanumeric())), "note text '{w}' in the drawing");
        }
    }
    // fits, no note: the opening sizes everywhere, and the biggest that fit
    for (setup, kind, size, angle) in [("Table", "Block", 100, 30.0), ("Table", "Block", 200, 30.0), ("Table", "Cart", 150, 30.0), ("Ramp", "Block", 100, 60.0), ("Ramp", "Cart", 100, 45.0), ("Ramp", "Block", 200, 15.0), ("Ramp", "Cart", 70, 60.0), ("Table", "Block", 50, 30.0), ("Ramp", "Block", 50, 60.0)] {
        let p = big(setup, kind, size, angle);
        assert!(super::pulley::touch(&p).is_some(), "{setup} {kind} {size} {angle} fits");
        assert!(notes_for("pulley", &p).is_empty());
    }
    // the tied second and the other setups never get the note
    for setup in ["Atwood", "Table", "Ramp", "Block & tackle"] {
        assert!(notes_for("pulley", &with(&[s("setup", setup), b("second", true), s("l_kind", "Cart"), i("l_size", 200)])).is_empty());
    }
}

/// Crowded touching states: every force on, both frictions, both accelerations, each size and kind, the ramp's
/// angles, long labels and Large-ish text, mirrored.
fn crowded() -> Vec<Vec<(&'static str, Value)>> {
    let mut out = vec![];
    let mut k = 0;
    for setup in ["Table", "Ramp"] {
        for angle in if setup == "Ramp" { vec![15.0, 25.0, 30.0, 40.0, 50.0, 60.0] } else { vec![30.0] } {
            for kind in ["Block", "Cart"] {
                for size in [50, 70, 100, 130, 160, 200] {
                    for fr in ["None", "Toward the pulley", "Away from it"] {
                        for acc in ["Right falls", "Left falls"] {
                            k += 1;
                            let labs = match k % 3 { 0 => ("P", "m_1", "m_3"), 1 => ("F_{c}", "M", "m"), _ => ("", "m_{1}", "5 kg") };
                            let mut l = touching(setup);
                            l.extend([f("angle", angle), s("l_kind", kind), i("l_size", size), s("friction", fr), s("accel", acc), b("contact", true), b("tension", true), b("gravity", true), b("normal", true), s("surface", "Rough"), b("mirror", k % 4 == 0)]);
                            l.extend([s("cont_lab", labs.0), s("l_lab", labs.1), s("s_lab", labs.2)]);
                            if labs.0.is_empty() { l.push(s("cont_lab_mode", "Blank line")); }
                            // only what the figure draws: the states it refuses are tested on their own
                            let p = with(&l);
                            if super::pulley::touch(&p).is_some() { out.push(l); }
                        }
                    }
                }
            }
        }
    }
    out
}

/// The text elements of the drawing with their positions in the string.
fn texts(t: &str) -> Vec<(usize, usize)> {
    t.match_indices("<text ").map(|(i, _)| (i, i + t[i..].find("</text>").unwrap() + 7)).collect()
}

/// Lines and polylines' segments, with their stroke colour.
fn segments(t: &str) -> Vec<((f64, f64), (f64, f64), String)> { segments_w(t).into_iter().map(|x| (x.0, x.1, x.2)).collect() }

/// The drawn strokes: lines and every polygon's edges, with colour (teal = the contact pair) and half width.
fn segments_w(t: &str) -> Vec<((f64, f64), (f64, f64), String, f64)> {
    let sw = |h: &str| h.split("stroke-width=\"").nth(1).and_then(|x| x.split('"').next()).and_then(|x| x.parse::<f64>().ok()).unwrap_or(1.0) / 2.0;
    let mut v = vec![];
    for h in tags(t, "line") {
        let c = h.split("stroke=\"").nth(1).unwrap().split('"').next().unwrap().to_string();
        v.push(((num(h, "x1"), num(h, "y1")), (num(h, "x2"), num(h, "y2")), c, sw(h)));
    }
    // arrowheads and outlines: each polygon's edges, with its fill (heads) or stroke colour
    for h in tags(t, "polygon") {
        let fill = h.split("fill=\"").nth(1).unwrap().split('"').next().unwrap();
        let c = if fill == TEAL { TEAL.to_string() } else { "other".to_string() };
        let i = h.find("points=\"").unwrap() + 8;
        let pts: Vec<(f64, f64)> = h[i..].split('"').next().unwrap().split_whitespace().map(|q| { let (x, y) = q.split_once(',').unwrap(); (x.parse().unwrap(), y.parse().unwrap()) }).collect();
        for k in 0..pts.len() { v.push((pts[k], pts[(k + 1) % pts.len()], c.clone(), sw(h))); }
    }
    v
}

/// A text element's letters, tags taken out.
fn plain(e: &str) -> String {
    let mut out = String::new();
    let mut tag = false;
    for c in e.chars() {
        match c { '<' => tag = true, '>' => tag = false, c if !tag && c.is_alphanumeric() => out.push(c), _ => {} }
    }
    out
}

fn seg_dist(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l = (dx * dx + dy * dy).max(1e-9);
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l).clamp(0.0, 1.0);
    ((p.0 - a.0 - t * dx).powi(2) + (p.1 - a.1 - t * dy).powi(2)).sqrt()
}

/// Where text element `t[i0..i1]` touches other ink (2 px at 2x) or leaves the page, if it does.
fn label_trouble(t: &str, i0: usize, i1: usize) -> Option<String> {
    let (w, h, a) = crate::figures::to_rgba(t, 2.0).unwrap();
    let t0 = format!("{}{}", &t[..i0], &t[i1..]);
    let (_, _, z) = crate::figures::to_rgba(&t0, 2.0).unwrap();
    let lum = |px: &[u8], x: i64, y: i64| -> u8 { if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 { 255 } else { let o = ((y as u32 * w + x as u32) * 4) as usize; px[o].min(px[o + 1]).min(px[o + 2]) } };
    let mut ink = 0;
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            if lum(&a, x, y) < 110 && lum(&z, x, y) > 200 {
                ink += 1;
                if !(x > 2 && y > 2 && x < w as i64 - 3 && y < h as i64 - 3) { return Some(format!("off the page at ({}, {})", x as f64 / 2.0, y as f64 / 2.0)); }
                for dy in -4..=4 { for dx in -4..=4 {
                    if lum(&z, x + dx, y + dy) <= 170 { return Some(format!("touches other ink at ({}, {})", (x + dx) as f64 / 2.0, (y + dy) as f64 / 2.0)); }
                } }
            }
        }
    }
    if ink <= 10 { return Some("draws nothing".into()); }
    None
}

/// A text element without its position, for matching the same label across two drawings.
fn text_body(e: &str) -> String {
    let i = e.find(" font-size").unwrap();
    e[i..].to_string()
}

#[test]
fn every_label_touches_no_other_ink_as_rasterized() {
    // measured from pixels, not boxes: each text's own dark pixels (there with it, white without it) have no other
    // ink within 2 px, and lie inside the page, at 2x, over the crowded touching states. Every label in the figure
    // is checked, not only the new ones: the touching layout moves the others. A label the old single-object figure
    // draws in the same place (the same text, moved only with the ramp) that already touched other ink there is the
    // old figure's (the ramp's angle label and the hanging mass's weight on the shallowest ramps); listed, not failed
    let (mut labels, mut old_trouble) = (0, std::collections::BTreeSet::new());
    for st in crowded().into_iter().step_by(3) {
        let p = with(&st);
        let t = svg(&p);
        let mut q = p.clone();
        q.set_b("second", false);
        let old = svg(&q);
        let (gn, go) = (super::pulley::pulley_geo(&p), super::pulley::pulley_geo(&q));
        let (dx, dy) = match (gn.ramp, go.ramp) { (Some(a), Some(b)) => (a.0 - b.0, a.1 - b.1), _ => (0.0, 0.0) };
        let mir = p.b("mirror");
        let ux = |x: f64, w: f64| if mir { w - x } else { x };
        for (i0, i1) in texts(&t) {
            let e = &t[i0..i1];
            labels += 1;
            let Some(why) = label_trouble(&t, i0, i1) else { continue };
            // the same label in the old figure: moved only with the ramp, or (the hanging mass's, which hangs from the
            // wheel the touching string raises) the same text with the same trouble there
            let (x, y) = (ux(num(e, "x"), gn.w), num(e, "y"));
            let olds: Vec<(usize, usize)> = texts(&old).into_iter().filter(|&(o0, o1)| text_body(&old[o0..o1]) == text_body(e)).collect();
            // (the first object itself may sit a little further from the pulley than the old one, to leave the second
            // room: its labels move with it by that much along the surface)
            let tc = super::pulley::touch(&p).unwrap();
            let th = if p.s("setup") == "Ramp" { p.f("angle").clamp(15.0, 60.0).to_radians() } else { 0.0 };
            let len = if p.s("setup") == "Ramp" { (250.0 / th.tan()).min(330.0) / th.cos() } else { 290.0 };
            let shift = tc.s1 - len * if p.s("setup") == "Ramp" { 0.5 } else { 0.55 };
            let (sx, sy) = (shift * th.cos(), -shift * th.sin());
            let moved = olds.iter().find(|&&(o0, o1)| {
                let f = &old[o0..o1];
                [(0.0, 0.0), (sx, sy)].iter().any(|(ex, ey)| (ux(num(f, "x"), go.w) + dx + ex - x).abs() < 0.02 && (num(f, "y") + dy + ey - y).abs() < 0.02)
            });
            let hanging = x > gn.boxes[0].cx - gn.boxes[0].s && y > gn.boxes[0].top;
            let excused = match moved {
                Some(&(o0, o1)) => label_trouble(&old, o0, o1).is_some(),
                None => hanging && olds.iter().any(|&(o0, o1)| label_trouble(&old, o0, o1).is_some()),
            };
            if excused { old_trouble.insert(format!("{} ({}{})", plain(e), p.s("setup"), p.f("angle"))); } else { panic!("{e} {why} {st:?}"); }
        }
    }
    assert!(labels > 300, "{labels}");
    eprintln!("labels drawn as the old figure draws them, with the old figure's touching: {old_trouble:?}");
}

#[test]
fn the_contact_arrows_touch_no_other_ink_as_rasterized() {
    // the pair's own ink (taken out by colour) keeps 2 px from every other stroke and text, at 2x
    let mut n = 0;
    for mut st in crowded().into_iter().step_by(2) {
        st.push(b("color", true));
        let p = with(&st);
        let t = svg(&p);
        // the drawing without the pair's two shafts and heads (the teal elements)
        let mut t0 = t.clone();
        for name in ["line", "polygon"] {
            for h in tags(&t, name).into_iter().filter(|h| h.contains(TEAL)) {
                t0 = t0.replacen(h, "", 1);
            }
        }
        // ... and without its label (its text is the contact label's letters)
        let want: String = p.s("cont_lab").chars().filter(|c| c.is_alphanumeric()).collect();
        if p.s("cont_lab_mode") == "Text" {
            let mut k = 0;
            for (i0, i1) in texts(&t) {
                let e = &t[i0..i1];
                if plain(e) == want { t0 = t0.replacen(e, "", 1); k += 1; }
            }
            assert_eq!(k, 1, "{st:?}");
        }
        let (w, h, a) = crate::figures::to_rgba(&t, 2.0).unwrap();
        let (_, _, z) = crate::figures::to_rgba(&t0, 2.0).unwrap();
        let lum = |px: &[u8], x: i64, y: i64| -> u8 { if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 { 255 } else { let o = ((y as u32 * w + x as u32) * 4) as usize; px[o].min(px[o + 1]).min(px[o + 2]) } };
        let mut ink = 0;
        for y in 0..h as i64 {
            for x in 0..w as i64 {
                if lum(&a, x, y) < 150 && lum(&z, x, y) > 230 {
                    ink += 1;
                    for dy in -4..=4 { for dx in -4..=4 {
                        assert!(lum(&z, x + dx, y + dy) > 170, "the pair touches other ink at ({}, {}) {st:?}", (x + dx) as f64 / 2.0, (y + dy) as f64 / 2.0);
                    } }
                }
            }
        }
        assert!(ink > 100, "the pair is drawn {st:?}");
        n += 1;
    }
    assert!(n > 100);
}

#[test]
fn the_contact_label_is_nearest_its_own_pair() {
    // read from the drawn glyph box (the label's ink, rasterized at 2x with and without it, boxed): the nearest stroke
    // edge to the pair's label is the pair's own (an arrow, or the leader to it), by 2 px over any other stroke
    use super::pulley_b10::{box_seg, Seg};
    let mut n = 0;
    for mut st in crowded().into_iter().step_by(2) {
        st.extend([b("color", true), s("cont_lab", "P"), s("cont_lab_mode", "Text")]);
        let p = with(&st);
        let t = svg(&p);
        let segs = segments_w(&t);
        let (w, h, a) = crate::figures::to_rgba(&t, 2.0).unwrap();
        let found: Vec<(usize, usize)> = texts(&t).into_iter().filter(|&(a, b)| plain(&t[a..b]) == "P").collect();
        assert_eq!(found.len(), 1, "{st:?}");
        let (i0, i1) = found[0];
        let t0 = format!("{}{}", &t[..i0], &t[i1..]);
        let (_, _, z) = crate::figures::to_rgba(&t0, 2.0).unwrap();
        let mut r = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        for y in 0..h {
            for x in 0..w {
                let o = ((y * w + x) * 4) as usize;
                if a[o].min(a[o + 1]).min(a[o + 2]) < 160 && z[o].min(z[o + 1]).min(z[o + 2]) > 200 {
                    let q = (x as f64 / 2.0, y as f64 / 2.0);
                    r = (r.0.min(q.0), r.1.min(q.1), r.2.max(q.0 + 0.5), r.3.max(q.1 + 0.5));
                }
            }
        }
        let d: Vec<f64> = segs.iter().map(|sg| box_seg(r, &Seg { a: sg.0, b: sg.1, hw: sg.3 })).collect();
        let mine = (0..segs.len()).filter(|k| segs[*k].2 == TEAL).map(|k| d[k]).fold(f64::INFINITY, f64::min);
        let ko = (0..segs.len()).filter(|k| segs[*k].2 != TEAL).min_by(|x, y| d[*x].total_cmp(&d[*y])).unwrap();
        assert!(d[ko] >= mine + 2.0, "the pair's label is as near another stroke ({:.1}, {:?}) as the pair ({mine:.1}) {st:?}", d[ko], segs[ko]);
        n += 1;
    }
    assert!(n > 50, "{n}");
}

/// The pair's label as drawn: the text elements whose letters are the typed label's.
fn contact_texts<'a>(t: &'a str, lab: &str) -> Vec<&'a str> {
    let want: String = lab.chars().filter(|c| c.is_alphanumeric()).collect();
    texts(t).into_iter().map(|(a, b)| &t[a..b]).filter(|e| plain(e) == want).collect()
}

#[test]
fn a_typed_contact_label_is_drawn_or_the_panel_says_it_was_left_off() {
    // checker round 1: "F_{contact}" on a ramp drew no label and said nothing. Over the table and ramps of 10 to 60
    // degrees, every force switch together, object sizes and labels from P to F_{1 on 2}: the pair's label is in the
    // drawing (once, as typed), or a note in the panel names it as left off; on ordinary figures (the table, ramps of
    // 20 to 45 degrees, the opening sizes) it is never left off
    let labels = ["P", "F_{c}", "F_{31}", "F_{contact}", "F_{1 on 2}"];
    let setups: Vec<(&str, f64)> = std::iter::once(("Table", 30.0)).chain([10.0, 15.0, 20.0, 30.0, 40.0, 45.0, 50.0, 60.0].into_iter().map(|a| ("Ramp", a))).collect();
    let combos = || {
        let mut v = vec![];
        for m in 0..8 { for fr in ["None", "Toward the pulley", "Away from it"] { for ac in ["None", "Right falls", "Left falls"] {
            v.push(vec![b("tension", m & 1 != 0), b("gravity", m & 2 != 0), b("normal", m & 4 != 0), s("friction", fr), s("accel", ac)]);
        } } }
        v
    };
    let (mut drawn, mut off) = (0, 0);
    let mut check = |setup: &str, angle: f64, size: i64, lab: &str, extra: &[(&'static str, Value)]| {
        let mut l = touching(setup);
        l.extend([f("angle", angle), i("l_size", size), b("contact", true), s("cont_lab", lab)]);
        l.extend(extra.iter().cloned());
        let p = with(&l);
        if super::pulley::touch(&p).is_none() { return; }
        let t = svg(&p);
        let n = contact_texts(&t, lab).len();
        let notes = notes_for("pulley", &p);
        let noted = notes.iter().any(|x| x.contains("left off") && x.contains(lab));
        assert!(n == 1 || (n == 0 && noted), "{lab} on {setup} {angle} {size}: drawn {n} times, notes {notes:?}");
        assert!(n == 0 || !noted, "{lab}: drawn and also noted as left off");
        let ordinary = size == 100 && (setup == "Table" || (20.0..=45.0).contains(&angle));
        assert!(n == 1 || !ordinary, "{lab} left off on an ordinary figure: {setup} {angle} {extra:?}");
        if n == 1 { drawn += 1 } else { off += 1 }
    };
    for &(setup, angle) in &setups {
        for c in combos() {
            for lab in ["F_{contact}", "F_{1 on 2}"] { check(setup, angle, 100, lab, &c); }
        }
        for size in [50, 70, 100, 150, 200] {
            for lab in labels {
                check(setup, angle, size, lab, &[]);
                check(setup, angle, size, lab, &[b("tension", true), b("gravity", true), b("normal", true), s("friction", "Away from it"), s("accel", "Right falls")]);
            }
        }
    }
    eprintln!("contact label drawn {drawn}, left off with a note {off}");
    assert!(drawn > 1000 && off * 50 < drawn, "{drawn} {off}");
}

#[test]
fn a_blank_line_stands_for_the_pairs_label() {
    // checker round 1: Blank line drew a blank over one arrow only. The pair has one label slot (as on the inclined
    // plane), and Blank line draws its 26 px answer line there, just above the pair, with no text in its place
    for setup in ["Table", "Ramp"] {
        let mut l = touching(setup);
        l.extend([b("contact", true), b("tension", true), b("gravity", true), b("normal", true), s("accel", "Right falls"), s("cont_lab_mode", "Blank line"), b("color", true)]);
        let p = with(&l);
        let t = svg(&p);
        let blanks: Vec<&str> = tags(&t, "line").into_iter().filter(|e| (num(e, "x2") - num(e, "x1")).abs() > 25.9 && (num(e, "x2") - num(e, "x1")).abs() < 26.1 && (num(e, "y2") - num(e, "y1")).abs() < 0.01 && e.contains("stroke-width=\"1.5\"")).collect();
        assert_eq!(blanks.len(), 1, "{setup}: one blank for the pair");
        // it sits by the pair: within 30 px of a pair arrowhead
        let heads = pair_heads(&t);
        let e = blanks[0];
        let mid = ((num(e, "x1") + num(e, "x2")) / 2.0, num(e, "y1"));
        let near = heads.iter().map(|h| (h[0].0 - mid.0).hypot(h[0].1 - mid.1)).fold(f64::INFINITY, f64::min);
        assert!(near < 40.0, "{setup}: the blank is by the pair ({near})");
        let mut q = p.clone();
        q.set_s("cont_lab_mode", "Text");
        assert_eq!(contact_texts(&svg(&q), "P").len(), 1);
    }
}

#[test]
fn the_contact_label_is_the_size_of_the_other_force_labels() {
    // checker round 1: contact labels shrank to about 9 px against 22 px for every other force. Read from the drawn
    // text: the pair's label is set at the size the tension, normal, friction and weight labels are
    for (setup, angle) in [("Table", 30.0), ("Ramp", 20.0), ("Ramp", 40.0), ("Ramp", 60.0)] {
        for lab in ["P", "F_{c}", "F_{31}", "F_{contact}"] {
            for size in [50, 100, 200] {
                let mut l = touching(setup);
                l.extend([f("angle", angle), i("l_size", size), b("contact", true), b("tension", true), b("gravity", true), b("normal", true), s("friction", "Away from it"), s("cont_lab", lab)]);
                let p = with(&l);
                if super::pulley::touch(&p).is_none() { continue; }
                let t = svg(&p);
                let size_of = |e: &str| num(e, "font-size");
                let others: Vec<f64> = texts(&t).into_iter().map(|(a, b)| &t[a..b]).filter(|e| ["T", "FN", "Ff"].contains(&plain(e).as_str())).map(size_of).collect();
                assert!(!others.is_empty());
                for e in contact_texts(&t, lab) {
                    assert!(others.iter().all(|o| size_of(e) >= *o), "{lab} at {} against {others:?} on {setup} {angle} {size}", size_of(e));
                }
            }
        }
    }
}

#[test]
fn one_label_for_the_pair_as_on_the_inclined_plane() {
    // checker round 1: the inclined plane labels its touching pair once, above the pair; the pulley labelled each
    // arrow. Both now draw one label, just above the pair's middle (the pulley's within the pair's span, over it)
    let ip = registry().into_iter().find(|d| d.id == "inclined-plane").unwrap();
    let mut q = crate::figures::start_params(&ip);
    for (k, v) in [i("n", 2), s("joined", "Touching"), b("contact", true)] { q.0.insert(k.into(), v); }
    q.normalize(&(ip.params)(), false);
    let it = ip.svg(&q);
    assert_eq!(texts(&it).into_iter().filter(|&(a, b)| plain(&it[a..b]) == "P").count(), 1, "the inclined plane's convention");
    for setup in ["Table", "Ramp"] {
        for st in [vec![], vec![b("tension", true), b("gravity", true), b("normal", true), s("friction", "Away from it"), s("accel", "Right falls")]] {
            let mut l = touching(setup);
            l.extend([b("contact", true), b("color", true)]);
            l.extend(st);
            let t = svg(&with(&l));
            let labs = contact_texts(&t, "P");
            assert_eq!(labs.len(), 1, "{setup}");
            let heads = pair_heads(&t);
            let (x0, x1) = (heads[0][0].0.min(heads[1][0].0), heads[0][0].0.max(heads[1][0].0));
            let x = num(labs[0], "x");
            assert!(x > x0 && x < x1, "{setup}: the label sits over the pair ({x} in {x0}..{x1})");
            // above it: on the far side of the pair's line from the objects (the surface normal's side)
            let (h0, h1) = (heads[0][0], heads[1][0]);
            let mut nrm = (h1.1 - h0.1, -(h1.0 - h0.0));
            if nrm.1 > 0.0 { nrm = (-nrm.0, -nrm.1); }
            let q = (num(labs[0], "x"), num(labs[0], "y") - 6.0);
            assert!((q.0 - h0.0) * nrm.0 + (q.1 - h0.1) * nrm.1 > 0.0, "{setup}: above it");
            assert!(!tags(&t, "line").iter().any(|e| e.contains(TEAL) && e.contains("stroke-width=\"1.2\"")), "{setup}: no leader needed");
        }
    }
}

#[test]
fn the_whole_drawing_stays_on_the_page() {
    // every coordinate of every touching state (crowded, and the ends of the sizes and angles) inside the canvas
    for st in crowded() {
        let t = svg(&with(&st));
        let (w, h) = crate::figures::svg_size(&t);
        for e in tags(&t, "line") {
            for k in ["x1", "x2"] { assert!((0.0..=w).contains(&num(e, k)), "{e} {st:?}"); }
            for k in ["y1", "y2"] { assert!((0.0..=h).contains(&num(e, k)), "{e} {st:?}"); }
        }
        for e in tags(&t, "text") {
            assert!((0.0..=w).contains(&num(e, "x")) && (10.0..=h).contains(&num(e, "y")), "{e} {st:?}");
        }
        assert!(!t.contains("NaN") && !t.contains("inf"), "{st:?}");
    }
}

#[test]
fn the_handles_follow_the_touching_drawing_through_pick() {
    // the hanging mass's resize corner and hang-lower knob sit where it is drawn, the ramp's angle knob drags the
    // angle, all through the workspace's hit test
    let d = def();
    for setup in ["Table", "Ramp"] {
        let mut l = touching(setup);
        l.push(b("contact", true));
        let p = with(&l);
        let t = svg(&p);
        let size = crate::figures::svg_size(&t);
        let hs = d.handles(&p, size);
        let geo = super::pulley::pulley_geo(&p);
        let hb = &geo.boxes[0];
        // the drawn hanging box: a rect polygon with that top-left corner
        assert!(t.contains(&format!("{:.2},{:.2}", hb.cx - hb.s / 2.0, hb.top)), "{setup}: the hanging box is where the handles think");
        let k = interact::pick(&hs, (hb.cx + hb.s / 2.0, hb.top), 1.0).expect("a handle at the corner");
        assert!(hs[k].tip.contains("resize"), "{}", hs[k].tip);
        let mut q = p.clone();
        (hs[k].drag.as_ref().unwrap())(&mut q, (hb.cx + hb.s, hb.top));
        assert_eq!(q.i("r_size"), 200, "{setup}: dragged through pick");
        if setup == "Ramp" {
            let (ax, ground) = geo.ramp.unwrap();
            let kp = (ax + 60.0 * p.f("angle").to_radians().cos(), ground - 60.0 * p.f("angle").to_radians().sin());
            let k = interact::pick(&hs, kp, 1.0).expect("the angle knob");
            assert!(hs[k].tip.contains("angle"), "{}", hs[k].tip);
            let mut q = p.clone();
            (hs[k].drag.as_ref().unwrap())(&mut q, (ax + 60.0, ground - 60.0));
            assert_eq!(q.f("angle").round(), 45.0);
            assert_ne!(svg(&q), t);
        }
    }
}

#[test]
fn the_summary_names_the_contact_forces_only_where_drawn() {
    let sum = |p: &Params| super::summary::summary("pulley", "Forces and motion", p).unwrap_or_default();
    let mut l = touching("Table");
    l.push(b("contact", true));
    assert!(sum(&with(&l)).contains("contact forces"));
    l.push(s("joined", "By a string"));
    assert!(!sum(&with(&l)).contains("contact"));
    let mut l = touching("Table");
    l.extend([b("contact", true), s("l_kind", "Cart"), i("l_size", 200)]);
    assert!(!sum(&with(&l)).contains("contact"), "refused: not drawn, not named");
}

/// Eyeballing aid: PULLEY_ZOOM='<params json>|x0,y0,x1,y1|<out.png>' writes that part of the drawing at 4x.
#[test]
fn zoom_dump() {
    let Ok(spec) = std::env::var("PULLEY_ZOOM") else { return };
    let mut it = spec.split('|');
    let (js, bx, out) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
    let p = crate::figures::params_from_json(&def(), &serde_json::from_str(js).unwrap());
    let t = svg(&p);
    let v: Vec<f64> = bx.split(',').map(|x| x.parse().unwrap()).collect();
    let i = t.find("<svg").unwrap();
    let e = t[i..].find('>').unwrap() + i;
    let head = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="{} {} {} {}">"#, v[2] - v[0], v[3] - v[1], v[0], v[1], v[2] - v[0], v[3] - v[1]);
    let z = format!("{}{}{}", &t[..i], head, &t[e + 1..]);
    std::fs::write(out, crate::figures::to_png(&z, 4.0).unwrap()).unwrap();
}

#[test]
fn the_angle_mark_is_the_old_one() {
    // the touching ramp moves down the page (and right, for a long normal label): its angle arc and label are the
    // old figure's, moved with the ramp's foot and nothing else
    for angle in [15.0, 30.0, 45.0, 60.0] {
        for size in [50, 100, 200] {
            let mut l = touching("Ramp");
            l.extend([f("angle", angle), i("l_size", size), b("normal", true), b("contact", true)]);
            let p = with(&l);
            if super::pulley::touch(&p).is_none() { continue; }
            let mut old = p.clone();
            old.set_b("second", false);
            let (gn, go) = (super::pulley::pulley_geo(&p).ramp.unwrap(), super::pulley::pulley_geo(&old).ramp.unwrap());
            let (dx, dy) = (gn.0 - go.0, gn.1 - go.1);
            let theta = |t: &str| { let e = texts(t).into_iter().map(|(a, b)| &t[a..b]).find(|e| plain(e) == "\u{3b8}").unwrap().to_string(); (num(&e, "x"), num(&e, "y")) };
            let (a, b) = (theta(&svg(&p)), theta(&svg(&old)));
            assert!((a.0 - b.0 - dx).abs() < 0.02 && (a.1 - b.1 - dy).abs() < 0.02, "{angle} {size}: {a:?} vs {b:?} + ({dx}, {dy})");
        }
    }
}

