//! The faces of each net, in the shape's own units with y down.
//!
//! Every edge carries a key: the pair of corners of the folded solid it lies on. Two faces' edges with the same key at
//! the same place are one fold; a key whose two edges lie apart is a pair of cut edges that meet when the net is folded.
//! A cube's net is folded square by square to find its keys; the other solids' corners are numbered directly.
use super::{Base, Model, Shape};
use std::f64::consts::PI;

pub type Pt = (f64, f64);

#[derive(Clone, Debug)]
pub enum Outline {
    Poly(Vec<Pt>),
    Circle(Pt, f64),
    /// A sector with its apex, radius and the angles (radians, clockwise on the page from +x) its arc runs between.
    Sector { apex: Pt, r: f64, a0: f64, a1: f64 },
}

#[derive(Clone, Debug)]
pub struct Edge {
    pub a: Pt,
    pub b: Pt,
    pub key: (u32, u32),
    /// The measure this edge's length is (length, width, side, height, slant, circ …), if any.
    pub role: Option<&'static str>,
}

#[derive(Clone, Debug)]
pub struct Face {
    pub outline: Outline,
    /// A polygon's edges, vertex i to i + 1; a sector's two straight edges, the left one first. A circle has none.
    pub edges: Vec<Edge>,
}

impl Face {
    /// The middle the face's name and area are written at: a polygon's vertex average, a circle's centre, 0.62 of the
    /// way out along a sector's middle.
    pub fn centre(&self) -> Pt {
        match &self.outline {
            Outline::Poly(v) => {
                let n = v.len() as f64;
                (v.iter().map(|q| q.0).sum::<f64>() / n, v.iter().map(|q| q.1).sum::<f64>() / n)
            }
            Outline::Circle(c, _) => *c,
            Outline::Sector { apex, r, a0, a1 } => {
                let a = (a0 + a1) / 2.0;
                (apex.0 + 0.62 * r * a.cos(), apex.1 + 0.62 * r * a.sin())
            }
        }
    }
    pub fn area(&self) -> f64 {
        match &self.outline {
            Outline::Poly(v) => {
                let mut s = 0.0;
                for i in 0..v.len() {
                    let (a, b) = (v[i], v[(i + 1) % v.len()]);
                    s += a.0 * b.1 - b.0 * a.1;
                }
                s.abs() / 2.0
            }
            Outline::Circle(_, r) => PI * r * r,
            Outline::Sector { r, a0, a1, .. } => r * r * (a1 - a0) / 2.0,
        }
    }
    /// Points along the outline (a polygon's corners; 96 points round a circle or along an arc, with a sector's apex).
    pub fn outline_points(&self) -> Vec<Pt> {
        match &self.outline {
            Outline::Poly(v) => v.clone(),
            Outline::Circle(c, r) => (0..96).map(|k| {
                let a = k as f64 * 2.0 * PI / 96.0;
                (c.0 + r * a.cos(), c.1 + r * a.sin())
            }).collect(),
            Outline::Sector { apex, r, a0, a1 } => {
                let mut v = vec![*apex];
                v.extend((0..=96).map(|k| {
                    let a = a0 + (a1 - a0) * k as f64 / 96.0;
                    (apex.0 + r * a.cos(), apex.1 + r * a.sin())
                }));
                v
            }
        }
    }
    /// The unit normal of edge `i` pointing out of the face.
    pub fn out_normal(&self, i: usize) -> Pt {
        let e = &self.edges[i];
        let (dx, dy) = (e.b.0 - e.a.0, e.b.1 - e.a.1);
        let len = (dx * dx + dy * dy).sqrt().max(1e-12);
        let n = (dy / len, -dx / len);
        let c = match &self.outline {
            // a sector's straight edges: away from the arc's middle
            Outline::Sector { apex, r, a0, a1 } => {
                let a = (a0 + a1) / 2.0;
                (apex.0 + 0.5 * r * a.cos(), apex.1 + 0.5 * r * a.sin())
            }
            _ => self.centre(),
        };
        let m = ((e.a.0 + e.b.0) / 2.0, (e.a.1 + e.b.1) / 2.0);
        if (m.0 - c.0) * n.0 + (m.1 - c.1) * n.1 >= 0.0 { n } else { (-n.0, -n.1) }
    }
}

fn key(a: u32, b: u32) -> (u32, u32) { (a.min(b), a.max(b)) }

fn poly(v: Vec<Pt>, keys: Vec<(u32, u32)>, roles: Vec<Option<&'static str>>) -> Face {
    let n = v.len();
    let edges = (0..n).map(|i| Edge { a: v[i], b: v[(i + 1) % n], key: keys[i], role: roles[i] }).collect();
    Face { outline: Outline::Poly(v), edges }
}

// ───────────────────────── cube ─────────────────────────

/// The 11 nets of a cube, as (row, column) squares; the first square is where folding starts.
pub const CUBE_NETS: [[(i32, i32); 6]; 11] = [
    [(0, 1), (1, 0), (1, 1), (1, 2), (1, 3), (2, 1)],
    [(0, 0), (1, 0), (1, 1), (1, 2), (1, 3), (2, 0)],
    [(0, 0), (1, 0), (1, 1), (1, 2), (1, 3), (2, 1)],
    [(0, 0), (1, 0), (1, 1), (1, 2), (1, 3), (2, 2)],
    [(0, 0), (1, 0), (1, 1), (1, 2), (1, 3), (2, 3)],
    [(0, 1), (1, 0), (1, 1), (1, 2), (1, 3), (2, 2)],
    [(0, 0), (0, 1), (1, 1), (1, 2), (1, 3), (2, 1)],
    [(0, 0), (0, 1), (1, 1), (1, 2), (1, 3), (2, 2)],
    [(0, 0), (0, 1), (1, 1), (1, 2), (1, 3), (2, 3)],
    [(0, 0), (0, 1), (1, 1), (1, 2), (2, 2), (2, 3)],
    [(0, 0), (0, 1), (0, 2), (1, 2), (1, 3), (1, 4)],
];

type V3 = (i32, i32, i32);
fn add(a: V3, b: V3) -> V3 { (a.0 + b.0, a.1 + b.1, a.2 + b.2) }
fn neg(a: V3) -> V3 { (-a.0, -a.1, -a.2) }
fn corner_id(c: V3) -> u32 { ((c.0 + 2) * 25 + (c.1 + 2) * 5 + (c.2 + 2)) as u32 }

/// Folds net `k` (1 to 11): for each square, where its page corners go on the unit cube. A square's map is its top-left
/// corner, the cube directions of the page's +x and +y, and the normal into the cube; the next square across an edge
/// turns up about that edge.
pub fn fold_cube(k: usize) -> Vec<[V3; 4]> {
    let sq = &CUBE_NETS[k.clamp(1, 11) - 1];
    let mut map: Vec<Option<(V3, V3, V3, V3)>> = vec![None; 6];
    map[0] = Some(((0, 0, 0), (1, 0, 0), (0, 1, 0), (0, 0, 1)));
    let mut queue = vec![0usize];
    while let Some(i) = queue.pop() {
        let (o, ex, ey, n) = map[i].unwrap();
        let (r, c) = sq[i];
        for j in 0..6 {
            if map[j].is_some() {
                continue;
            }
            let next = match (sq[j].0 - r, sq[j].1 - c) {
                (0, 1) => (add(o, ex), n, ey, neg(ex)),
                (0, -1) => (add(o, n), neg(n), ey, ex),
                (1, 0) => (add(o, ey), ex, n, neg(ey)),
                (-1, 0) => (add(o, n), ex, neg(n), ey),
                _ => continue,
            };
            map[j] = Some(next);
            queue.insert(0, j);
        }
    }
    map.into_iter()
        .map(|m| {
            let (o, ex, ey, _) = m.expect("a cube net is connected");
            [o, add(o, ex), add(add(o, ex), ey), add(o, ey)]
        })
        .collect()
}

fn cube(m: &Model) -> Vec<Face> {
    let s = m.v("side");
    let sq = &CUBE_NETS[m.net.clamp(1, 11) - 1];
    let corners = fold_cube(m.net);
    // faces in reading order (row by row), not the folding order
    let mut order: Vec<usize> = (0..6).collect();
    order.sort_by_key(|&i| sq[i]);
    order
        .into_iter()
        .map(|i| {
            let (r, c) = (sq[i].0 as f64, sq[i].1 as f64);
            let v = vec![(c * s, r * s), ((c + 1.0) * s, r * s), ((c + 1.0) * s, (r + 1.0) * s), (c * s, (r + 1.0) * s)];
            let k = corners[i];
            let keys = (0..4).map(|e| key(corner_id(k[e]), corner_id(k[(e + 1) % 4]))).collect();
            poly(v, keys, vec![Some("side"); 4])
        })
        .collect()
}

// ───────────────────────── prism ─────────────────────────

/// The base polygon with edge 0 from (0, 0) along +x and the inside on the +v side, and each edge's measure.
fn base_shape(m: &Model) -> (Vec<Pt>, Vec<&'static str>) {
    match m.base {
        Base::Rect => {
            let (l, w) = (m.v("length"), m.v("width"));
            (vec![(0.0, 0.0), (l, 0.0), (l, w), (0.0, w)], vec!["length", "width", "length", "width"])
        }
        Base::RightTri => {
            let (b, t) = (m.v("tri_base"), m.v("tri_height"));
            (vec![(0.0, 0.0), (b, 0.0), (0.0, t)], vec!["tri_base", "hyp", "tri_height"])
        }
        Base::Isos => {
            let (b, t) = (m.v("tri_base"), m.v("tri_height"));
            (vec![(0.0, 0.0), (b, 0.0), (b / 2.0, t)], vec!["tri_base", "hyp", "hyp"])
        }
        Base::Regular(n) => {
            let s = m.v("side");
            let mut v = vec![(0.0, 0.0)];
            for k in 0..n - 1 {
                let a = k as f64 * 2.0 * PI / n as f64;
                let q = v[k];
                v.push((q.0 + s * a.cos(), q.1 + s * a.sin()));
            }
            (v, vec!["side"; n])
        }
    }
}

/// Which strip position holds base edge 0: the middle one, or just left of the middle for an even count.
pub fn strip_home(c: usize) -> usize { (c - 1) / 2 }

fn prism(m: &Model) -> Vec<Face> {
    let (bp, roles) = base_shape(m);
    let c = bp.len();
    let h = m.v("height");
    let len = |e: usize| {
        let (a, b) = (bp[e], bp[(e + 1) % c]);
        ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt()
    };
    let f = strip_home(c);
    // the corners: T_i on the top base, B_i on the bottom one
    let t = |i: usize| (i % c) as u32;
    let b = |i: usize| 100 + (i % c) as u32;
    let mut faces = Vec::new();
    let mut x = 0.0;
    let mut home_x = 0.0;
    for j in 0..c {
        let e = (j + c - f) % c;
        let w = len(e);
        if j == f {
            home_x = x;
        }
        let v = vec![(x, 0.0), (x + w, 0.0), (x + w, h), (x, h)];
        let keys = vec![key(t(e), t(e + 1)), key(t(e + 1), b(e + 1)), key(b(e), b(e + 1)), key(t(e), b(e))];
        faces.push(poly(v, keys, vec![Some(roles[e]), Some("height"), Some(roles[e]), Some("height")]));
        x += w;
    }
    // the two bases on the home rectangle, edge 0 on its top and bottom edges, outside the strip
    for top in [true, false] {
        let v: Vec<Pt> = bp.iter().map(|q| (home_x + q.0, if top { -q.1 } else { h + q.1 })).collect();
        let keys = (0..c).map(|i| if top { key(t(i), t(i + 1)) } else { key(b(i), b(i + 1)) }).collect();
        faces.push(poly(v, keys, roles.iter().map(|r| Some(*r)).collect()));
    }
    faces
}

// ───────────────────────── pyramid ─────────────────────────

/// The pyramid's base on the page, edge 0 along the bottom.
pub fn pyramid_base(m: &Model) -> (Vec<Pt>, Vec<&'static str>) {
    match m.base {
        Base::Regular(n) => {
            let r = m.v("side") / (2.0 * (PI / n as f64).sin());
            let v = (0..n)
                .map(|k| {
                    let a = -PI / 2.0 - PI / n as f64 + k as f64 * 2.0 * PI / n as f64;
                    (r * a.cos(), -r * a.sin())
                })
                .collect();
            (v, vec!["side"; n])
        }
        _ => {
            let (l, w) = (m.v("length"), m.v("width"));
            (vec![(0.0, 0.0), (l, 0.0), (l, -w), (0.0, -w)], vec!["length", "width", "length", "width"])
        }
    }
}

const APEX: u32 = 99;

fn pyramid(m: &Model) -> Vec<Face> {
    let (bv, roles) = pyramid_base(m);
    let n = bv.len();
    let h = m.v("height");
    let cx = bv.iter().map(|q| q.0).sum::<f64>() / n as f64;
    let cy = bv.iter().map(|q| q.1).sum::<f64>() / n as f64;
    let id = |i: usize| (i % n) as u32;
    let mut faces = vec![poly(bv.clone(), (0..n).map(|i| key(id(i), id(i + 1))).collect(), roles.iter().map(|r| Some(*r)).collect())];
    for e in 0..n {
        let (a, b) = (bv[e], bv[(e + 1) % n]);
        let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        let d = ((mid.0 - cx).powi(2) + (mid.1 - cy).powi(2)).sqrt();
        let out = ((mid.0 - cx) / d, (mid.1 - cy) / d);
        let sl = (h * h + d * d).sqrt();
        let apex = (mid.0 + out.0 * sl, mid.1 + out.1 * sl);
        faces.push(poly(vec![a, b, apex], vec![key(id(e), id(e + 1)), key(id(e + 1), APEX), key(id(e), APEX)], vec![Some(roles[e]), None, None]));
    }
    faces
}

/// The apex of the triangle on base edge `e` (a pyramid's face e + 1).
pub fn pyramid_apex(faces: &[Face], e: usize) -> Pt {
    match &faces[e + 1].outline {
        Outline::Poly(v) => v[2],
        _ => (0.0, 0.0),
    }
}

// ───────────────────────── cylinder and cone ─────────────────────────

fn cylinder(m: &Model) -> Vec<Face> {
    let (r, h) = (m.v("radius"), m.v("height"));
    let c = 2.0 * PI * r;
    vec![
        poly(vec![(0.0, 0.0), (c, 0.0), (c, h), (0.0, h)], vec![(200, 201), (300, 301), (202, 203), (300, 301)], vec![Some("circ"), Some("height"), Some("circ"), Some("height")]),
        Face { outline: Outline::Circle((c / 2.0, -r), r), edges: vec![] },
        Face { outline: Outline::Circle((c / 2.0, h + r), r), edges: vec![] },
    ]
}

fn cone(m: &Model) -> Vec<Face> {
    let (r, l) = (m.v("radius"), m.v("slant"));
    let th = (2.0 * PI * r / l).min(2.0 * PI - 1e-6);
    let (a0, a1) = (PI / 2.0 - th / 2.0, PI / 2.0 + th / 2.0);
    let p = |a: f64| (l * a.cos(), l * a.sin());
    vec![
        Face {
            outline: Outline::Sector { apex: (0.0, 0.0), r: l, a0, a1 },
            edges: vec![Edge { a: p(a1), b: (0.0, 0.0), key: (400, 401), role: Some("slant") }, Edge { a: (0.0, 0.0), b: p(a0), key: (400, 401), role: Some("slant") }],
        },
        Face { outline: Outline::Circle((0.0, l + r), r), edges: vec![] },
    ]
}

pub fn build(m: &Model) -> Vec<Face> {
    match m.shape {
        Shape::Cube => cube(m),
        Shape::Prism => prism(m),
        Shape::Pyramid => pyramid(m),
        Shape::Cylinder => cylinder(m),
        Shape::Cone => cone(m),
    }
}

fn same(a: Pt, b: Pt, eps: f64) -> bool { (a.0 - b.0).abs() <= eps && (a.1 - b.1).abs() <= eps }

/// For each face's edges: the face it folds onto (an edge with the same key at the same place), or None for a cut edge.
pub fn folds(faces: &[Face]) -> Vec<Vec<Option<usize>>> {
    let scale = faces.iter().flat_map(|f| f.outline_points()).fold(0.0f64, |s, q| s.max(q.0.abs()).max(q.1.abs())).max(1.0);
    let eps = 1e-7 * scale;
    faces
        .iter()
        .enumerate()
        .map(|(i, f)| {
            f.edges
                .iter()
                .map(|e| {
                    faces.iter().enumerate().find_map(|(j, g)| {
                        (j != i && g.edges.iter().any(|o| o.key == e.key && ((same(o.a, e.a, eps) && same(o.b, e.b, eps)) || (same(o.a, e.b, eps) && same(o.b, e.a, eps))))).then_some(j)
                    })
                })
                .collect()
        })
        .collect()
}

/// The net's extent: (x0, y0, x1, y1).
pub fn bounds(faces: &[Face]) -> (f64, f64, f64, f64) {
    let mut b = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for q in faces.iter().flat_map(|f| f.outline_points()) {
        b = (b.0.min(q.0), b.1.min(q.1), b.2.max(q.0), b.3.max(q.1));
    }
    b
}
