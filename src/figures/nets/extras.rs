//! The net's extras: glue tabs, the faces' names and matching tints, and the solid drawn beside its net.
use super::geom::{Face, Outline, Pt};
use super::{Base, Model, Shape};
use std::f64::consts::PI;

fn sub(a: Pt, b: Pt) -> Pt { (a.0 - b.0, a.1 - b.1) }
fn len(a: Pt) -> f64 { a.0.hypot(a.1) }

// ───────────────────────── glue tabs ─────────────────────────

/// A glue tab: its outline (the hinge first, a to b, then the outer side), and the face and edge it hangs from (no edge
/// for a circle's tooth).
#[derive(Clone, Debug)]
pub struct Tab {
    pub poly: Vec<Pt>,
    pub face: usize,
    pub edge: Option<usize>,
}

/// Whether point q is inside polygon v.
fn in_poly(v: &[Pt], q: Pt) -> bool {
    let mut inside = false;
    let n = v.len();
    for i in 0..n {
        let (a, b) = (v[i], v[(i + 1) % n]);
        if (a.1 > q.1) != (b.1 > q.1) && q.0 < a.0 + (q.1 - a.1) / (b.1 - a.1) * (b.0 - a.0) {
            inside = !inside;
        }
    }
    inside
}

fn cross(a: Pt, b: Pt, c: Pt, d: Pt) -> bool {
    let o = |p: Pt, q: Pt, r: Pt| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    o(c, d, a) * o(c, d, b) < 0.0 && o(a, b, c) * o(a, b, d) < 0.0
}

/// Whether two outlines overlap by more than a sliver: the tab is shrunk by 2% of its size first (the reference ignores
/// overlaps under 2% of a tab), so one touching an edge doesn't count.
fn overlap(tab: &[Pt], other: &[Pt]) -> bool {
    let n = tab.len() as f64;
    let c = (tab.iter().map(|q| q.0).sum::<f64>() / n, tab.iter().map(|q| q.1).sum::<f64>() / n);
    // 2% of its width off each side, toward its middle (so a tab only touching along its hinge doesn't overlap)
    let k = 0.96;
    let t: Vec<Pt> = tab.iter().map(|q| (c.0 + (q.0 - c.0) * k, c.1 + (q.1 - c.1) * k)).collect();
    if t.iter().any(|q| in_poly(other, *q)) || other.iter().any(|q| in_poly(&t, *q)) {
        return true;
    }
    let m = other.len();
    (0..t.len()).any(|i| (0..m).any(|j| cross(t[i], t[(i + 1) % t.len()], other[j], other[(j + 1) % m])))
}

/// A trapezoid tab on edge a-b, on the side `out`: depth min(T, 0.4·edge), each end chamfered in by min(depth, 0.3·edge).
fn trapezoid(a: Pt, b: Pt, out: Pt, t: f64) -> Vec<Pt> {
    let l = len(sub(b, a)).max(1e-12);
    let u = ((b.0 - a.0) / l, (b.1 - a.1) / l);
    let d = t.min(0.4 * l);
    let inset = d.min(0.3 * l);
    vec![a, b, (b.0 - u.0 * inset + out.0 * d, b.1 - u.1 * inset + out.1 * d), (a.0 + u.0 * inset + out.0 * d, a.1 + u.1 * inset + out.1 * d)]
}

/// The tabs for cutting the net out, in the net's own units: one on one of each pair of cut edges that meet when it's
/// folded (the first in face order, else the second when the first would overlap a face or an earlier tab), and teeth
/// round each circle. The tab size is 0.22 of the root of the mean face area.
pub fn tabs(faces: &[Face], fold: &[Vec<Option<usize>>]) -> Vec<Tab> {
    let mean = faces.iter().map(|f| f.area()).sum::<f64>() / faces.len().max(1) as f64;
    let t = 0.22 * mean.sqrt();
    let outlines: Vec<Vec<Pt>> = faces.iter().map(|f| f.outline_points()).collect();
    let mut out: Vec<Tab> = Vec::new();
    // (not against its own face: a tab hangs outward from it, and a tooth's inner side is the circle's own arc)
    let clear = |poly: &[Pt], own: usize, out: &[Tab]| !outlines.iter().enumerate().any(|(i, o)| i != own && overlap(poly, o)) && !out.iter().any(|tb| overlap(poly, &tb.poly));
    // the cut edges, grouped by the folded edge they lie on
    let mut seen: Vec<(u32, u32)> = Vec::new();
    for (i, f) in faces.iter().enumerate() {
        for (j, e) in f.edges.iter().enumerate() {
            if fold[i][j].is_some() || seen.contains(&e.key) {
                continue;
            }
            let pair: Vec<(usize, usize)> = faces
                .iter()
                .enumerate()
                .flat_map(|(ii, g)| g.edges.iter().enumerate().filter(move |(jj, o)| o.key == e.key && fold[ii][*jj].is_none()).map(move |(jj, _)| (ii, jj)))
                .collect();
            seen.push(e.key);
            if pair.len() != 2 {
                continue;
            }
            for (fi, ej) in pair {
                let ed = &faces[fi].edges[ej];
                let poly = trapezoid(ed.a, ed.b, faces[fi].out_normal(ej), t);
                if clear(&poly, fi, &out) {
                    out.push(Tab { poly, face: fi, edge: Some(ej) });
                    break;
                }
            }
        }
    }
    // teeth round each circle: 12 slots of 30°, from +x going clockwise on the page
    for (i, f) in faces.iter().enumerate() {
        if let Outline::Circle(c, r) = f.outline {
            let depth = t.min(0.45 * r);
            let slot = PI / 6.0;
            for k in 0..12 {
                let a0 = k as f64 * slot;
                let p = |a: f64, rr: f64| (c.0 + rr * a.cos(), c.1 + rr * a.sin());
                let poly = vec![p(a0 + 0.12 * slot, r), p(a0 + slot - 0.12 * slot, r), p(a0 + slot - 0.27 * slot, r + depth), p(a0 + 0.27 * slot, r + depth)];
                if clear(&poly, i, &out) {
                    out.push(Tab { poly, face: i, edge: None });
                }
            }
        }
    }
    out
}

// ───────────────────────── faces ─────────────────────────

/// Each face's name, A, B, C … in reading order of the faces' middles: top to bottom, then left to right.
pub fn names(faces: &[Face]) -> Vec<String> {
    let mut order: Vec<usize> = (0..faces.len()).collect();
    order.sort_by(|&a, &b| {
        let (ca, cb) = (faces[a].centre(), faces[b].centre());
        if (ca.1 - cb.1).abs() <= 1e-6 * ca.1.abs().max(cb.1.abs()).max(1.0) { ca.0.total_cmp(&cb.0) } else { ca.1.total_cmp(&cb.1) }
    });
    let mut out = vec![String::new(); faces.len()];
    for (k, &i) in order.iter().enumerate() {
        out[i] = ((b'A' + (k % 26) as u8) as char).to_string();
    }
    out
}

/// Which faces match (the same shape and size: the same edge lengths and area; circles by radius; sectors all alike),
/// as a class number per face, the classes numbered in order of their first face.
pub fn classes(faces: &[Face]) -> Vec<usize> {
    let sig = |f: &Face| -> String {
        match &f.outline {
            Outline::Circle(_, r) => format!("c{r:.4}"),
            Outline::Sector { .. } => "s".into(),
            Outline::Poly(_) => {
                let mut l: Vec<String> = f.edges.iter().map(|e| format!("{:.4}", len(sub(e.b, e.a)))).collect();
                l.sort();
                format!("p{}|{:.4}", l.join(","), f.area())
            }
        }
    };
    let mut seen: Vec<String> = Vec::new();
    faces
        .iter()
        .map(|f| {
            let s = sig(f);
            match seen.iter().position(|x| *x == s) {
                Some(k) => k,
                None => {
                    seen.push(s);
                    seen.len() - 1
                }
            }
        })
        .collect()
}

// ───────────────────────── the solid ─────────────────────────

/// The solid in an oblique (cabinet) view, depth going up and to the right at 45° at half length: its visible lines and
/// its hidden ones, in the solid's units with y down.
#[derive(Default)]
pub struct Solid {
    pub lines: Vec<(Pt, Pt)>,
    pub hidden: Vec<(Pt, Pt)>,
    /// Elliptical arcs (centre, rx, ry, from, to angle; hidden), for a cylinder's and a cone's circles.
    pub arcs: Vec<(Pt, f64, f64, f64, f64, bool)>,
}

type P3 = (f64, f64, f64);
const K: f64 = 0.5 * std::f64::consts::FRAC_1_SQRT_2;

fn proj(p: P3) -> Pt { (p.0 + p.2 * K, -(p.1 + p.2 * K)) }

/// A polyhedron's visible and hidden edges: a face shows when its outward normal faces the viewer; an edge shows when
/// either of its faces does.
fn polyhedron(verts: &[P3], faces: &[Vec<usize>]) -> Solid {
    let n = verts.len() as f64;
    let c = (verts.iter().map(|v| v.0).sum::<f64>() / n, verts.iter().map(|v| v.1).sum::<f64>() / n, verts.iter().map(|v| v.2).sum::<f64>() / n);
    // the direction of sight, into the page
    let r = (-K, -K, 1.0);
    let shows: Vec<bool> = faces
        .iter()
        .map(|f| {
            let (a, b, cc) = (verts[f[0]], verts[f[1]], verts[f[2]]);
            let u = (b.0 - a.0, b.1 - a.1, b.2 - a.2);
            let w = (cc.0 - a.0, cc.1 - a.1, cc.2 - a.2);
            let mut nrm = (u.1 * w.2 - u.2 * w.1, u.2 * w.0 - u.0 * w.2, u.0 * w.1 - u.1 * w.0);
            let fc = f.iter().fold((0.0, 0.0, 0.0), |s, &i| (s.0 + verts[i].0, s.1 + verts[i].1, s.2 + verts[i].2));
            let fc = (fc.0 / f.len() as f64, fc.1 / f.len() as f64, fc.2 / f.len() as f64);
            if nrm.0 * (fc.0 - c.0) + nrm.1 * (fc.1 - c.1) + nrm.2 * (fc.2 - c.2) < 0.0 {
                nrm = (-nrm.0, -nrm.1, -nrm.2);
            }
            nrm.0 * r.0 + nrm.1 * r.1 + nrm.2 * r.2 < -1e-9
        })
        .collect();
    let mut edges: Vec<((usize, usize), bool)> = Vec::new();
    for (fi, f) in faces.iter().enumerate() {
        for k in 0..f.len() {
            let (a, b) = (f[k], f[(k + 1) % f.len()]);
            let key = (a.min(b), a.max(b));
            match edges.iter_mut().find(|e| e.0 == key) {
                Some(e) => e.1 |= shows[fi],
                None => edges.push((key, shows[fi])),
            }
        }
    }
    let mut s = Solid::default();
    for ((a, b), vis) in edges {
        let seg = (proj(verts[a]), proj(verts[b]));
        if vis { s.lines.push(seg) } else { s.hidden.push(seg) }
    }
    s
}

/// A prism over base polygon `base` (in the x–z plane) of height h, or a pyramid to an apex at height h.
fn upright(base: &[(f64, f64)], h: f64, pyramid: bool) -> Solid {
    let n = base.len();
    let mut verts: Vec<P3> = base.iter().map(|q| (q.0, 0.0, q.1)).collect();
    let mut faces: Vec<Vec<usize>> = vec![(0..n).collect()];
    if pyramid {
        let (cx, cz) = (base.iter().map(|q| q.0).sum::<f64>() / n as f64, base.iter().map(|q| q.1).sum::<f64>() / n as f64);
        verts.push((cx, h, cz));
        faces.extend((0..n).map(|i| vec![i, (i + 1) % n, n]));
    } else {
        verts.extend(base.iter().map(|q| (q.0, h, q.1)));
        faces.push((n..2 * n).collect());
        faces.extend((0..n).map(|i| vec![i, (i + 1) % n, n + (i + 1) % n, n + i]));
    }
    polyhedron(&verts, &faces)
}

/// The regular n-gon of side s in the x–z plane, an edge at the front.
fn regular(n: usize, s: f64) -> Vec<(f64, f64)> {
    let r = s / (2.0 * (PI / n as f64).sin());
    (0..n).map(|k| {
        let a = -PI / 2.0 - PI / n as f64 + k as f64 * 2.0 * PI / n as f64;
        (r * a.cos(), r * a.sin())
    }).collect()
}

pub fn solid(m: &Model) -> Solid {
    match (m.shape, m.base) {
        (Shape::Cube, _) => {
            let s = m.v("side");
            upright(&[(0.0, 0.0), (s, 0.0), (s, s), (0.0, s)], s, false)
        }
        (Shape::Prism, Base::Rect) => {
            let (l, w) = (m.v("length"), m.v("width"));
            upright(&[(0.0, 0.0), (l, 0.0), (l, w), (0.0, w)], m.v("height"), false)
        }
        (Shape::Prism, Base::RightTri | Base::Isos) => {
            // lying on a rectangular face, the triangle at the front, its length going back
            let (b, t, l) = (m.v("tri_base"), m.v("tri_height"), m.v("height"));
            let tri: [(f64, f64); 3] = if m.base == Base::RightTri { [(0.0, 0.0), (b, 0.0), (0.0, t)] } else { [(0.0, 0.0), (b, 0.0), (b / 2.0, t)] };
            let mut verts: Vec<P3> = tri.iter().map(|q| (q.0, q.1, 0.0)).collect();
            verts.extend(tri.iter().map(|q| (q.0, q.1, l)));
            let faces = vec![vec![0, 1, 2], vec![3, 4, 5], vec![0, 1, 4, 3], vec![1, 2, 5, 4], vec![2, 0, 3, 5]];
            polyhedron(&verts, &faces)
        }
        (Shape::Prism, Base::Regular(n)) => upright(&regular(n, m.v("side")), m.v("height"), false),
        (Shape::Pyramid, Base::Regular(n)) => upright(&regular(n, m.v("side")), m.v("height"), true),
        (Shape::Pyramid, _) => {
            let (l, w) = (m.v("length"), m.v("width"));
            upright(&[(0.0, 0.0), (l, 0.0), (l, w), (0.0, w)], m.v("height"), true)
        }
        (Shape::Cylinder, _) => {
            let (r, h) = (m.v("radius"), m.v("height"));
            let ry = K * r;
            let mut s = Solid::default();
            s.lines.push(((-r, 0.0), (-r, -h)));
            s.lines.push(((r, 0.0), (r, -h)));
            s.arcs.push(((0.0, -h), r, ry, 0.0, 2.0 * PI, false));
            // the bottom circle's far half is hidden behind the side
            s.arcs.push(((0.0, 0.0), r, ry, 0.0, PI, false));
            s.arcs.push(((0.0, 0.0), r, ry, PI, 2.0 * PI, true));
            s
        }
        (Shape::Cone, _) => {
            let (r, h) = (m.v("radius"), m.v("height"));
            let ry = K * r;
            let mut s = Solid::default();
            // the sides touch the base circle where the line from the apex is tangent to it
            // the sides touch the base's ellipse where a line from the apex is tangent to it: at y = −ry²/h (behind the
            // middle), so the near arc between them shows and the far one is hidden
            if h > ry * 1.0001 {
                let yt = -ry * ry / h;
                let xt = r * (1.0 - (ry / h).powi(2)).sqrt();
                s.lines.push(((0.0, -h), (-xt, yt)));
                s.lines.push(((0.0, -h), (xt, yt)));
                let a_r = (yt / ry).atan2(xt / r);
                s.arcs.push(((0.0, 0.0), r, ry, a_r, PI - a_r, false));
                s.arcs.push(((0.0, 0.0), r, ry, PI - a_r, 2.0 * PI + a_r, true));
            } else {
                s.lines.push(((0.0, -h), (-r, 0.0)));
                s.lines.push(((0.0, -h), (r, 0.0)));
                s.arcs.push(((0.0, 0.0), r, ry, 0.0, 2.0 * PI, false));
            }
            s
        }
    }
}

/// The solid's extent: (x0, y0, x1, y1).
pub fn solid_bounds(s: &Solid) -> (f64, f64, f64, f64) {
    let mut b = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    let mut add = |q: Pt| b = (b.0.min(q.0), b.1.min(q.1), b.2.max(q.0), b.3.max(q.1));
    for (a, c) in s.lines.iter().chain(s.hidden.iter()) {
        add(*a);
        add(*c);
    }
    for (c, rx, ry, a0, a1, _) in &s.arcs {
        for k in 0..=48 {
            let t = a0 + (a1 - a0) * k as f64 / 48.0;
            add((c.0 + rx * t.cos(), c.1 + ry * t.sin()));
        }
    }
    b
}
