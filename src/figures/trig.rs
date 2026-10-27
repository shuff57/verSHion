//! Right-triangle trig: a right triangle (right angle at C) with one acute angle marked theta and the three sides named relative to it
//! (opposite, adjacent, hypotenuse, or the letters a, b, c), and a readout of sin, cos, tan (as fractions of the side lengths and as
//! decimals) and of theta. Drawn with `svg::Doc` only, to scale in shape (the lengths are numbers to read, the picture is fitted).
//!
//! The triangle is set by two sides or by an angle and a side (`define`); every value (the three sides, theta, sin, cos, tan) has a
//! state: Given (printed on both copies), Ask (a blank on the student copy, the value on the key) or Hidden. Angles are in degrees.
//!
//! Layout: vertex letters, theta and the side labels are placed by a small candidate search (`place`): a spot that touches another
//! label or a line of the drawing is rejected, and the canvas grows to hold whatever was placed. Labels carry a 2 px white halo and
//! never a plate.
use super::circle::{num, seg_hits, tw, Rect};
use super::interact::Handle;
use super::svg::{self, Doc};
use super::*;

type Pt = (f64, f64);

pub const ID: &str = "right-triangle-trig";
const INK: &str = "#111111";
const BLANK: &str = "_____";
/// The marked angle is kept between these (degrees) so the picture stays readable.
const TH_MIN: f64 = 10.0;
const TH_MAX: f64 = 80.0;
/// The triangle is fitted into this box (px).
const BW: f64 = 300.0;
const BH: f64 = 190.0;
const MARGIN: f64 = 16.0;
const DEFINES: [&str; 5] = ["Two legs", "Leg and hypotenuse", "Angle and opposite", "Angle and adjacent", "Angle and hypotenuse"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum V { Opp, Adj, Hyp, Theta, Sin, Cos, Tan }

/// (value, state key, label, default state)
const VALS: [(V, &str, &str, &str); 7] = [
    (V::Opp, "g_opp", "Opposite side is", "Given"),
    (V::Adj, "g_adj", "Adjacent side is", "Given"),
    (V::Hyp, "g_hyp", "Hypotenuse is", "Ask"),
    (V::Theta, "g_theta", "The angle theta is", "Ask"),
    (V::Sin, "g_sin", "sin of theta is", "Ask"),
    (V::Cos, "g_cos", "cos of theta is", "Ask"),
    (V::Tan, "g_tan", "tan of theta is", "Ask"),
];

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef { id: ID, subject: Subject::Math, title: "Right-triangle trig", blurb: "A right triangle with the sides named opposite, adjacent and hypotenuse from the marked angle, and sin, cos and tan worked out from the sides.", params, render }]
}

fn params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("define", "The triangle is set by", &DEFINES, "Two legs").hint("Which numbers below are used: both legs; one leg and the hypotenuse; or the angle and one side. The other values are worked out"),
        ParamDef::float("opp_len", "Opposite side", 0.1, 10000.0, 0.1, 3.0),
        ParamDef::float("adj_len", "Adjacent side", 0.1, 10000.0, 0.1, 4.0),
        ParamDef::float("hyp_len", "Hypotenuse", 0.1, 10000.0, 0.1, 5.0),
        ParamDef::float("theta", "Angle theta (degrees)", TH_MIN, TH_MAX, 0.01, 30.0),
        ParamDef::choice("theta_at", "Theta is at vertex", &["A", "B"], "A").hint("Swaps which acute angle is theta: the opposite and adjacent sides swap with it"),
        ParamDef::text("unit", "Unit", "cm"),
        ParamDef::choice("side_labels", "Name the sides with", &["Words", "Letters"], "Words").hint("Words: opposite, adjacent, hypotenuse. Letters: a, b, c (a faces A, b faces B, c is the hypotenuse)"),
        ParamDef::bool("show_names", "Letter the vertices", true),
        ParamDef::bool("show_right", "Right-angle mark", true),
        ParamDef::bool("soh", "Show the SOH CAH TOA hint", false),
        ParamDef::int("dec", "Decimal places", 0, 4, 2),
    ];
    for e in VALS {
        v.push(ParamDef::choice(e.1, e.2, &["Given", "Ask", "Hidden"], e.3).hint("Given is printed on the student copy; Ask is blank there and filled on the key; Hidden is not shown"));
    }
    v.push(answers_param());
    v.push(svg::style_param());
    v.push(svg::label_size_param());
    v
}

fn render(p: &Params) -> String { build(p).0 }

// ---------------------------------------------------------------------------------------------------------------- the maths

/// The solved triangle, relative to theta: the side opposite it, the side next to it, the hypotenuse, theta in degrees.
#[derive(Clone, Copy, Debug)]
pub struct Sol { pub opp: f64, pub adj: f64, pub hyp: f64, pub th: f64, pub clamped: bool }

fn fin(x: f64, lo: f64, hi: f64, d: f64) -> f64 {
    if x.is_nan() { d } else { x.clamp(lo, hi) }
}

pub fn solve(p: &Params) -> Sol {
    let len = |k: &str, d: f64| fin(p.f(k), 0.1, 10000.0, d);
    let (o, a, h) = (len("opp_len", 3.0), len("adj_len", 4.0), len("hyp_len", 5.0));
    let tin = fin(p.f("theta"), TH_MIN, TH_MAX, 30.0);
    let cl = |t: f64| (t.clamp(TH_MIN, TH_MAX), t < TH_MIN || t > TH_MAX);
    let st = |t: f64| (t.to_radians().sin(), t.to_radians().cos(), t.to_radians().tan());
    let (opp, adj, hyp, th, clamped) = match p.s("define") {
        "Leg and hypotenuse" => {
            let (th, c) = cl((o / h).min(1.0).asin().to_degrees());
            if c {
                let (s, co, _) = st(th);
                let hy = o / s;
                (o, hy * co, hy, th, true)
            } else {
                (o, (h * h - o * o).max(0.0).sqrt(), h, th, false)
            }
        }
        "Angle and opposite" => { let (s, _, t) = st(tin); (o, o / t, o / s, tin, false) }
        "Angle and adjacent" => { let (_, c, t) = st(tin); (a * t, a, a / c, tin, false) }
        "Angle and hypotenuse" => { let (s, c, _) = st(tin); (h * s, h * c, h, tin, false) }
        _ => {
            let (th, c) = cl(o.atan2(a).to_degrees());
            if c {
                let (s, _, t) = st(th);
                (o, o / t, o / s, th, true)
            } else {
                (o, a, o.hypot(a), th, false)
            }
        }
    };
    Sol { opp, adj, hyp, th, clamped }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Md { Given, Ask, Hidden }

fn mode(p: &Params, key: &str) -> Md {
    match p.s(key) { "Given" => Md::Given, "Hidden" => Md::Hidden, _ => Md::Ask }
}

fn vi(v: V) -> usize { VALS.iter().position(|e| e.0 == v).unwrap_or(0) }

struct Ctx { s: Sol, dec: i64, unit: String, words: bool, ta: bool, md: [Md; 7] }

fn ctx(p: &Params) -> Ctx {
    Ctx {
        s: solve(p),
        dec: p.i("dec").clamp(0, 4),
        unit: p.s("unit").trim().chars().filter(|c| !c.is_control()).take(12).collect(),
        words: p.s("side_labels") != "Letters",
        ta: p.s("theta_at") != "B",
        md: std::array::from_fn(|i| mode(p, VALS[i].1)),
    }
}

impl Ctx {
    fn md(&self, v: V) -> Md { self.md[vi(v)] }
    fn value(&self, v: V) -> f64 {
        let s = &self.s;
        match v {
            V::Opp => s.opp, V::Adj => s.adj, V::Hyp => s.hyp, V::Theta => s.th,
            V::Sin => s.opp / s.hyp, V::Cos => s.adj / s.hyp, V::Tan => s.opp / s.adj,
        }
    }
    fn n(&self, x: f64) -> String { num(x, self.dec) }
    fn side_name(&self, v: V) -> &'static str {
        if self.words {
            match v { V::Opp => "opposite", V::Adj => "adjacent", _ => "hypotenuse" }
        } else if v == V::Hyp { "c" } else if (v == V::Opp) == self.ta { "a" } else { "b" }
    }
    /// The label of a side on the figure; `ans` is the key (the student copy blanks an Ask).
    fn side_text(&self, v: V, ans: bool) -> String {
        let nm = self.side_name(v);
        let u = if self.unit.is_empty() { String::new() } else { format!(" {}", self.unit) };
        match (self.md(v), ans) {
            (Md::Hidden, _) => nm.to_string(),
            (Md::Ask, false) => format!("{nm} = {BLANK}{u}"),
            _ => format!("{nm} = {}{u}", self.n(self.value(v))),
        }
    }
    /// "3/5 = 0.6": the ratio as a fraction of the side lengths and as a decimal.
    fn ratio_rhs(&self, v: V) -> String {
        let s = &self.s;
        let (a, b) = match v { V::Sin => (s.opp, s.hyp), V::Cos => (s.adj, s.hyp), _ => (s.opp, s.adj) };
        format!("{}/{} = {}", self.n(a), self.n(b), self.n(self.value(v)))
    }
    /// Every text that would give the value away: the number itself and, for a ratio, its fraction.
    fn tokens(&self, v: V) -> Vec<String> {
        let mut t = vec![self.n(self.value(v))];
        if matches!(v, V::Sin | V::Cos | V::Tan) { t.push(self.ratio_rhs(v).split(" = ").next().unwrap_or("").to_string()); }
        t
    }
}

struct Row { key: String, stu: String }

fn rows(c: &Ctx, hint: bool) -> Vec<Row> {
    let mut out = vec![];
    for (v, name) in [(V::Theta, "\u{3b8}"), (V::Sin, "sin \u{3b8}"), (V::Cos, "cos \u{3b8}"), (V::Tan, "tan \u{3b8}")] {
        let md = c.md(v);
        if md == Md::Hidden { continue; }
        let (key, blank) = if v == V::Theta {
            let f = if hint && md == Md::Ask { format!("arctan({}/{}) = ", c.n(c.s.opp), c.n(c.s.adj)) } else { String::new() };
            (format!("{name} = {f}{}\u{b0}", c.n(c.s.th)), format!("{name} = {BLANK}\u{b0}"))
        } else {
            (format!("{name} = {}", c.ratio_rhs(v)), format!("{name} = {BLANK}"))
        };
        let stu = if md == Md::Ask { blank } else if v == V::Theta { format!("{name} = {}\u{b0}", c.n(c.s.th)) } else { key.clone() };
        out.push(Row { key, stu });
    }
    out
}

/// How many values the sheet shows as Given and how many it asks for.
pub fn sheet_values(p: &Params) -> (usize, usize) {
    let c = ctx(p);
    (c.md.iter().filter(|m| **m == Md::Given).count(), c.md.iter().filter(|m| **m == Md::Ask).count())
}

/// Whether the Given values fix the triangle: a side (the scale) and one more value (the other side, the angle or a ratio).
fn determined(p: &Params) -> bool {
    let c = ctx(p);
    let sides = [V::Opp, V::Adj, V::Hyp].iter().filter(|v| c.md(**v) == Md::Given).count();
    let all = c.md.iter().filter(|m| **m == Md::Given).count();
    sides >= 1 && all >= 2
}

pub fn warnings(p: &Params) -> Vec<String> {
    let mut w = vec![];
    if solve(p).clamped {
        w.push(format!("The angle was kept between {TH_MIN} and {TH_MAX} degrees so the picture stays readable; the sides were adjusted to match."));
    }
    let (given, asks) = sheet_values(p);
    if asks > 0 && given == 0 {
        w.push("Nothing is Given: the student copy shows no value to work from.".to_string());
    } else if asks > 0 && !determined(p) {
        w.push("The Given values do not fix the triangle: give one side and one more value (the other side, the angle, or sin, cos or tan).".to_string());
    }
    w
}

// ---------------------------------------------------------------------------------------------------------------- layout

fn dist(a: Pt, b: Pt) -> f64 { (a.0 - b.0).hypot(a.1 - b.1) }
fn unit_of(a: Pt, b: Pt) -> Pt { let d = dist(a, b).max(1e-9); ((b.0 - a.0) / d, (b.1 - a.1) / d) }
fn half_ext(d: Pt, w: f64, h: f64) -> f64 { d.0.abs() * w / 2.0 + d.1.abs() * h / 2.0 }
fn rect_at(c: Pt, w: f64, h: f64) -> Rect { Rect { x0: c.0 - w / 2.0, y0: c.1 - h / 2.0, x1: c.0 + w / 2.0, y1: c.1 + h / 2.0 } }
fn grow(r: &Rect, t: f64) -> Rect { Rect { x0: r.x0 - t, y0: r.y0 - t, x1: r.x1 + t, y1: r.y1 + t } }

const COMPASS: [Pt; 8] = [(1.0, 0.0), (-1.0, 0.0), (0.0, -1.0), (0.0, 1.0), (0.7071, -0.7071), (-0.7071, -0.7071), (0.7071, 0.7071), (-0.7071, 0.7071)];

fn text_el(x: f64, y: f64, size: f64, anchor: &str, bold: bool, halo: bool, fill: &str, s: &str) -> String {
    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" font-weight="{}" fill="{fill}"{}>{}</text>"##,
        if bold { "bold" } else { "normal" },
        if halo { r##" stroke="#fff" stroke-width="2" paint-order="stroke" stroke-linejoin="round""## } else { "" },
        svg::esc(s)
    )
}

/// The first candidate centre whose box touches no placed box and no drawn line; the last one when none is clean (they run far out).
fn place(cands: &[Pt], w: f64, h: f64, placed: &[Rect], segs: &[(Pt, Pt)]) -> Pt {
    for &c in cands {
        let r = rect_at(c, w, h);
        let g = grow(&r, 1.0);
        if !placed.iter().any(|q| g.hit(q)) && !segs.iter().any(|s| seg_hits(s.0, s.1, &g)) { return c; }
    }
    *cands.last().unwrap_or(&(0.0, 0.0))
}

fn side_cands(a: Pt, b: Pt, nrm: Pt, w: f64, h: f64) -> Vec<Pt> {
    let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let len = dist(a, b);
    let t = unit_of(a, b);
    let he = half_ext(nrm, w, h);
    let mut v = vec![];
    for k in [0.0, 4.0, 9.0, 16.0, 26.0, 40.0, 60.0, 90.0, 130.0, 200.0, 300.0] {
        for s in [0.0, -0.15, 0.15, -0.3, 0.3, -0.4, 0.4] {
            let off = he + 4.0 + k;
            v.push((mid.0 + t.0 * len * s + nrm.0 * off, mid.1 + t.1 * len * s + nrm.1 * off));
        }
    }
    v
}

fn vertex_cands(vx: Pt, dir: Pt, w: f64, h: f64) -> Vec<Pt> {
    let mut dirs = vec![dir];
    dirs.extend(COMPASS);
    let mut v = vec![];
    for k in [0.0, 3.0, 7.0, 12.0, 20.0, 30.0, 45.0, 70.0, 110.0, 160.0, 240.0] {
        for d in &dirs { let off = half_ext(*d, w, h) + 3.0 + k; v.push((vx.0 + d.0 * off, vx.1 + d.1 * off)); }
    }
    v
}

fn theta_cands(t: Pt, bis: Pt, r_arc: f64, w: f64, h: f64) -> Vec<Pt> {
    let mut v = vec![];
    for j in 0..16 {
        let d = r_arc + 9.0 + 8.0 * j as f64;
        v.push((t.0 + bis.0 * d, t.1 + bis.1 * d));
    }
    for k in [0.0, 6.0, 14.0, 24.0, 40.0, 70.0, 110.0, 170.0, 260.0] {
        for d in COMPASS { let off = half_ext(d, w, h) + 6.0 + k; v.push((t.0 + d.0 * off, t.1 + d.1 * off)); }
    }
    v
}

/// What a render also knows about itself (for tests): the placed labels, the drawn segments, the corners, the canvas.
#[derive(Clone, Debug)]
pub struct Report {
    /// (name, box) in figure coordinates (before `shift`).
    pub labels: Vec<(String, Rect)>,
    pub segs: Vec<(Pt, Pt)>,
    pub w: f64,
    pub h: f64,
    pub shift: Pt,
    /// Final canvas coordinates of the theta vertex, the right-angle vertex and the other acute vertex.
    pub t: Pt,
    pub c: Pt,
    pub o: Pt,
    pub rows: Vec<Rect>,
}

pub fn build(p: &Params) -> (String, Report) {
    let f = svg::label_factor(p.s("label_size"));
    let c = ctx(p);
    let print = p.s("style") == "Print";
    let ans = p.b(SHOW_ANSWERS);
    let s = c.s;
    let (a_len, b_len) = if c.ta { (s.opp, s.adj) } else { (s.adj, s.opp) };
    let sc = (BW / b_len).min(BH / a_len);
    let (wpx, hpx) = (b_len * sc, a_len * sc);
    let (pa, pc, pb) = ((0.0, hpx), (wpx, hpx), (wpx, 0.0));
    let (t, o) = if c.ta { (pa, pb) } else { (pb, pa) };
    let role_col = |v: V| -> &'static str { if print { INK } else { match v { V::Opp => "#c0392b", V::Adj => "#2f8f4e", _ => "#1f5fbf" } } };
    let accent = if print { INK } else { "#7a4fc4" };
    let lw = if print { 2.6 } else { 3.0 };
    // side a = BC (right), side b = CA (bottom), side c = AB (hypotenuse)
    let side_a = if c.ta { V::Opp } else { V::Adj };
    let side_b = if c.ta { V::Adj } else { V::Opp };

    let mut fig = Doc::new(0.0, 0.0);
    let mut segs: Vec<(Pt, Pt)> = vec![(pb, pc), (pc, pa), (pa, pb)];
    fig.line(pb.0, pb.1, pc.0, pc.1, role_col(side_a), lw);
    fig.line(pc.0, pc.1, pa.0, pa.1, role_col(side_b), lw);
    fig.line(pa.0, pa.1, pb.0, pb.1, role_col(V::Hyp), lw);

    // the right angle at C (its room is kept even when the mark is off)
    let rs = 14.0;
    let (ua, ub) = (unit_of(pc, pa), unit_of(pc, pb));
    if p.b("show_right") { fig.right_angle(pc, ua, ub, rs, INK, 2.0); }
    let (r1, r3) = ((pc.0 + ua.0 * rs, pc.1 + ua.1 * rs), (pc.0 + ub.0 * rs, pc.1 + ub.1 * rs));
    let r2 = (r1.0 + ub.0 * rs, r1.1 + ub.1 * rs);
    segs.push((r1, r2));
    segs.push((r2, r3));

    // theta's arc at T, between the side T-C and the hypotenuse T-O
    let r_arc = (0.3 * dist(t, pc).min(dist(t, o))).clamp(16.0, 38.0);
    let ang = |from: Pt, to: Pt| (-(to.1 - from.1)).atan2(to.0 - from.0).to_degrees();
    let (a_c, a_o) = (ang(t, pc), ang(t, o));
    let dlt = (a_o - a_c + 540.0).rem_euclid(360.0) - 180.0;
    fig.arc(t.0, t.1, r_arc, a_c, a_c + dlt, accent, 2.0);
    let at = |deg: f64| { let r = deg.to_radians(); (t.0 + r_arc * r.cos(), t.1 - r_arc * r.sin()) };
    for i in 0..12 { segs.push((at(a_c + dlt * i as f64 / 12.0), at(a_c + dlt * (i + 1) as f64 / 12.0))); }
    let bis_deg = (a_c + dlt / 2.0).to_radians();
    let bis = (bis_deg.cos(), -bis_deg.sin());

    // labels, placed in this order
    let mut placed: Vec<Rect> = vec![];
    let mut labels: Vec<(String, Rect)> = vec![];
    let mut texts = String::new();
    let mut put = |name: &str, key: &str, stu: &str, size: f64, bold: bool, cands: &dyn Fn(f64, f64) -> Vec<Pt>, segs: &[(Pt, Pt)], fill: &str| {
        let (w, h) = (tw(key, size, bold).max(tw(stu, size, bold)) + 4.0, size * 1.2);
        let cs = cands(w, h);
        let ctr = place(&cs, w, h, &placed, segs);
        let r = rect_at(ctr, w, h);
        placed.push(r);
        labels.push((name.to_string(), r));
        let shown = if ans { key } else { stu };
        texts.push_str(&text_el(ctr.0, ctr.1 + size * 0.35, size, "middle", bold, true, fill, shown));
    };
    let vs = 17.0 * f;
    if p.b("show_names") {
        let cen = ((pa.0 + pb.0 + pc.0) / 3.0, (pa.1 + pb.1 + pc.1) / 3.0);
        for (nm, v) in [("A", pa), ("B", pb), ("C", pc)] {
            let dir = unit_of(cen, v);
            put(nm, nm, nm, vs, true, &|w, h| vertex_cands(v, dir, w, h), &segs, "#000");
        }
    }
    put("theta", "\u{3b8}", "\u{3b8}", vs, true, &|w, h| theta_cands(t, bis, r_arc, w, h), &segs, "#000");
    let ss = 15.0 * f;
    let hyp_n = (-(pc.1 - pb.1) / 1.0, -(pb.0 - pa.0)); // up-left of the hypotenuse
    let hn = { let l = hyp_n.0.hypot(hyp_n.1).max(1e-9); (hyp_n.0 / l, hyp_n.1 / l) };
    let sides = [(side_a, pb, pc, (1.0, 0.0)), (side_b, pc, pa, (0.0, 1.0)), (V::Hyp, pa, pb, hn)];
    for (v, a, b, nrm) in sides {
        let (key, stu) = (c.side_text(v, true), c.side_text(v, false));
        put(c.side_name(v), &key, &stu, ss, false, &|w, h| side_cands(a, b, nrm, w, h), &segs, "#000");
    }
    fig.raw(&texts);

    // the canvas: the figure, then the readout under it
    let mut bb = Rect { x0: 0.0, y0: 0.0, x1: wpx, y1: hpx };
    for r in &placed { bb = Rect { x0: bb.x0.min(r.x0), y0: bb.y0.min(r.y0), x1: bb.x1.max(r.x1), y1: bb.y1.max(r.y1) }; }
    let body = fig.cut(0);
    let rws = rows(&c, p.b("soh"));
    let rs_size = 16.0 * f;
    let hs_size = 13.0 * f;
    let hint = p.b("soh").then_some(["SOH  sin \u{3b8} = opposite / hypotenuse", "CAH  cos \u{3b8} = adjacent / hypotenuse", "TOA  tan \u{3b8} = opposite / adjacent"]);
    let row_w = rws.iter().map(|r| tw(&r.key, rs_size, false).max(tw(&r.stu, rs_size, false))).fold(0.0, f64::max);
    let hint_w = hint.map_or(0.0, |h| h.iter().map(|l| tw(l, hs_size, false)).fold(0.0, f64::max));
    let (fw, fh) = (bb.x1 - bb.x0, bb.y1 - bb.y0);
    let w = (fw.max(row_w).max(hint_w).max(340.0) + 2.0 * MARGIN).ceil();
    let has_readout = !rws.is_empty() || hint.is_some();
    let rowh = rs_size * 1.55;
    let ro_h = if has_readout { 16.0 + rws.len() as f64 * rowh + if hint.is_some() { 6.0 + 3.0 * hs_size * 1.4 } else { 0.0 } } else { 0.0 };
    let h = (MARGIN + fh + ro_h + MARGIN).ceil();
    let (dx, dy) = ((w - fw) / 2.0 - bb.x0, MARGIN - bb.y0);

    let mut d = Doc::new(w, h);
    d.describe("Right-triangle trigonometry", "A right triangle with its sides named opposite, adjacent and hypotenuse from the marked angle, and the sine, cosine and tangent of that angle");
    d.raw(&format!(r#"<g transform="translate({dx:.2},{dy:.2})">{body}</g>"#));
    let mut row_rects = vec![];
    if has_readout {
        let y0 = MARGIN + fh + 8.0;
        d.line(MARGIN, y0, w - MARGIN, y0, if print { "#888888" } else { "#bbbbbb" }, 1.0);
        let x = MARGIN + 6.0;
        let mut y = y0 + 8.0;
        for r in &rws {
            let shown = if ans { &r.key } else { &r.stu };
            let wd = tw(&r.key, rs_size, false).max(tw(&r.stu, rs_size, false));
            row_rects.push(Rect { x0: x, y0: y, x1: x + wd, y1: y + rowh });
            d.raw(&text_el(x, y + rowh * 0.7, rs_size, "start", false, false, "#000", shown));
            y += rowh;
        }
        if let Some(lines) = hint {
            y += 6.0;
            for l in lines {
                d.raw(&text_el(x, y + hs_size, hs_size, "start", false, false, "#333333", l));
                y += hs_size * 1.4;
            }
        }
    }
    let sh = (dx, dy);
    let fin_pt = |q: Pt| (q.0 + dx, q.1 + dy);
    let rep = Report { labels, segs, w, h, shift: sh, t: fin_pt(t), c: fin_pt(pc), o: fin_pt(o), rows: row_rects };
    (d.finish(), rep)
}

// ---------------------------------------------------------------------------------------------------------------- examples, handles

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let b = |k: &'static str, v: bool| (k, Value::B(v));
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    let st = |g: [&str; 7]| -> Vec<(&'static str, Value)> { VALS.iter().zip(g).map(|(e, m)| (e.1, Value::S(m.to_string()))).collect() };
    let ex = |name: &'static str, mut v: Vec<(&'static str, Value)>, g: [&str; 7]| { v.extend(st(g)); Example { name, params: v } };
    // order of the states: opposite, adjacent, hypotenuse, theta, sin, cos, tan
    Some(vec![
        ex("3-4-5: find the hypotenuse, then sin, cos and tan", vec![s("define", "Two legs"), f("opp_len", 3.0), f("adj_len", 4.0)], ["Given", "Given", "Ask", "Ask", "Ask", "Ask", "Ask"]),
        ex("5-12-13: the other acute angle is theta", vec![s("define", "Leg and hypotenuse"), f("opp_len", 5.0), f("hyp_len", 13.0), s("theta_at", "B")], ["Given", "Ask", "Given", "Hidden", "Ask", "Ask", "Ask"]),
        ex("30-60-90: hypotenuse 10", vec![s("define", "Angle and hypotenuse"), f("theta", 30.0), f("hyp_len", 10.0)], ["Ask", "Ask", "Given", "Given", "Ask", "Ask", "Ask"]),
        ex("Find the missing side: angle 35 and adjacent 12", vec![s("define", "Angle and adjacent"), f("theta", 35.0), f("adj_len", 12.0), b("soh", true)], ["Ask", "Given", "Ask", "Given", "Hidden", "Hidden", "Hidden"]),
        ex("Find the angle from two sides (inverse tangent)", vec![s("define", "Two legs"), f("opp_len", 7.0), f("adj_len", 10.0), b("soh", true), ("dec", Value::I(1)), s("side_labels", "Words")], ["Given", "Given", "Hidden", "Ask", "Hidden", "Hidden", "Ask"]),
    ])
}

fn round2(x: f64) -> f64 { (x * 100.0).round() / 100.0 }

pub fn handles(id: &str, p: &Params, _size: (f64, f64)) -> Option<Vec<Handle>> {
    if id != ID { return None; }
    let r = build(p).1;
    let (t, c, o) = (r.t, r.c, r.o);
    let tip = "Drag the far vertex to change the angle (the sizes follow)";
    Some(vec![Handle::knob(o, tip, move |p, pt| {
        if dist(pt, o) <= 1.0 || !pt.0.is_finite() || !pt.1.is_finite() { return; }
        let (u, w) = ((c.0 - t.0, c.1 - t.1), (pt.0 - t.0, pt.1 - t.1));
        let th = (u.0 * w.1 - u.1 * w.0).abs().atan2(u.0 * w.0 + u.1 * w.1).to_degrees().round();
        if !th.is_finite() { return; }
        let th = th.clamp(TH_MIN, TH_MAX);
        let cur = solve(p);
        match p.s("define") {
            "Two legs" => p.set_f("adj_len", round2(cur.opp / th.to_radians().tan()).clamp(0.1, 10000.0)),
            "Leg and hypotenuse" => p.set_f("hyp_len", round2(cur.opp / th.to_radians().sin()).clamp(0.1, 10000.0)),
            _ => p.set_f("theta", th),
        }
    })])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).unwrap() }
    fn with(list: &[(&'static str, Value)]) -> Params {
        let d = def();
        let mut p = Params::defaults(&(d.params)());
        for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
        p.normalize(&(d.params)(), false);
        p
    }
    fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
    fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
    fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }
    fn render_of(list: &[(&'static str, Value)]) -> String { (def().render)(&with(list)) }
    fn all_examples() -> Vec<(String, Vec<(&'static str, Value)>)> {
        examples(ID).unwrap().into_iter().map(|e| (e.name.to_string(), e.params)).collect()
    }

    /// The text of every <text> element, joined with newlines.
    fn texts(svg: &str) -> String {
        let mut out = String::new();
        let mut rest = svg;
        while let Some(i) = rest.find("<text ") {
            let after = &rest[i..];
            let Some(g) = after.find('>') else { break };
            let Some(e) = after.find("</text>") else { break };
            out.push_str(&after[g + 1..e]);
            out.push('\n');
            rest = &after[e + 7..];
        }
        out
    }
    /// Whether `tok` occurs in `hay` as a whole number (not inside a longer one).
    fn has_tok(hay: &str, tok: &str) -> bool {
        let mut from = 0;
        while let Some(i) = hay[from..].find(tok) {
            let at = from + i;
            let before = hay[..at].chars().next_back();
            let after = hay[at + tok.len()..].chars().next();
            let tail_digit = after.is_some_and(|c| c.is_ascii_digit());
            let tail_frac = after == Some('.') && hay[at + tok.len() + 1..].chars().next().is_some_and(|c| c.is_ascii_digit());
            if !before.is_some_and(|c| c.is_ascii_digit() || c == '.') && !tail_digit && !tail_frac { return true; }
            from = at + tok.len();
        }
        false
    }
    fn near(a: f64, b: f64) -> bool { (a - b).abs() < 1e-9 }

    #[test]
    fn values_3_4_5() {
        let p = with(&[s("define", "Two legs"), f("opp_len", 3.0), f("adj_len", 4.0)]);
        let c = ctx(&p);
        assert!(near(c.s.hyp, 5.0) && near(c.value(V::Sin), 0.6) && near(c.value(V::Cos), 0.8) && near(c.value(V::Tan), 0.75));
        assert!((c.s.th - 36.8698976).abs() < 1e-6);
        let key = texts(&render_of(&[b(SHOW_ANSWERS, true)]));
        assert!(key.contains("sin \u{3b8} = 3/5 = 0.6") && key.contains("cos \u{3b8} = 4/5 = 0.8") && key.contains("tan \u{3b8} = 3/4 = 0.75"), "{key}");
        assert!(key.contains("\u{3b8} = 36.87\u{b0}") && key.contains("hypotenuse = 5 cm"), "{key}");
    }

    #[test]
    fn values_5_12_13_and_the_swapped_angle() {
        // theta at A: opposite 5, adjacent 12
        let p = with(&[s("define", "Leg and hypotenuse"), f("opp_len", 5.0), f("hyp_len", 13.0)]);
        let c = ctx(&p);
        assert!(near(c.s.adj, 12.0) && near(c.value(V::Sin), 5.0 / 13.0) && near(c.value(V::Cos), 12.0 / 13.0) && near(c.value(V::Tan), 5.0 / 12.0));
        assert!((c.s.th - 22.6198649).abs() < 1e-6);
        // theta at B is the other angle: same triangle, the ratios swap and the angles add to 90
        let q = with(&[s("define", "Leg and hypotenuse"), f("opp_len", 12.0), f("hyp_len", 13.0), s("theta_at", "B")]);
        let d = ctx(&q);
        assert!((d.s.th + c.s.th - 90.0).abs() < 1e-9 && near(d.value(V::Sin), c.value(V::Cos)));
        // letters: with theta at A the opposite side is a, with theta at B it is b
        assert_eq!((c.words, ctx(&with(&[s("side_labels", "Letters")])).side_name(V::Opp)), (true, "a"));
        assert_eq!(ctx(&with(&[s("side_labels", "Letters"), s("theta_at", "B")])).side_name(V::Opp), "b");
        assert_eq!(ctx(&with(&[s("side_labels", "Letters"), s("theta_at", "B")])).side_name(V::Adj), "a");
        assert_ne!(render_of(&[]), render_of(&[s("theta_at", "B")]));
    }

    #[test]
    fn values_30_60_90_and_the_missing_side() {
        let p = with(&[s("define", "Angle and hypotenuse"), f("theta", 30.0), f("hyp_len", 10.0)]);
        let c = ctx(&p);
        assert!(near(c.s.opp, 5.0) && (c.s.adj - 5.0 * 3f64.sqrt()).abs() < 1e-9 && near(c.value(V::Sin), 0.5));
        assert!((c.value(V::Tan) - 1.0 / 3f64.sqrt()).abs() < 1e-9);
        // 60 degrees at B: the same triangle
        let q = ctx(&with(&[s("define", "Angle and hypotenuse"), f("theta", 60.0), f("hyp_len", 10.0), s("theta_at", "B")]));
        assert!(near(q.s.opp, 5.0 * 3f64.sqrt()) && (q.value(V::Cos) - 0.5).abs() < 1e-9);
        // the missing side: angle 35, adjacent 12 -> opposite = 12 tan 35, hypotenuse = 12 / cos 35
        let m = ctx(&with(&[s("define", "Angle and adjacent"), f("theta", 35.0), f("adj_len", 12.0)]));
        assert!((m.s.opp - 8.402_490_5).abs() < 1e-6 && (m.s.hyp - 14.649_295).abs() < 1e-5, "{:?}", m.s);
        // every definition gives the same triangle (Pythagoras and the ratios hold)
        for d in DEFINES {
            let c = ctx(&with(&[s("define", d), f("opp_len", 6.0), f("adj_len", 8.0), f("hyp_len", 10.0), f("theta", 36.0)]));
            let sl = c.s;
            assert!((sl.opp.powi(2) + sl.adj.powi(2) - sl.hyp.powi(2)).abs() < 1e-6 * sl.hyp.powi(2), "{d}: {sl:?}");
            assert!((sl.opp / sl.hyp - sl.th.to_radians().sin()).abs() < 1e-9 && (sl.opp / sl.adj - sl.th.to_radians().tan()).abs() < 1e-9, "{d}");
        }
    }

    #[test]
    fn the_inverse_trig_and_the_decimals() {
        let p = with(&[s("define", "Two legs"), f("opp_len", 7.0), f("adj_len", 10.0), ("dec", Value::I(1)), b(SHOW_ANSWERS, true)]);
        let t = texts(&(def().render)(&p));
        assert!(t.contains("\u{3b8} = 35\u{b0}") && t.contains("tan \u{3b8} = 7/10 = 0.7"), "{t}");
        let p0 = with(&[("dec", Value::I(0)), b(SHOW_ANSWERS, true)]);
        assert!(texts(&(def().render)(&p0)).contains("sin \u{3b8} = 3/5 = 1"), "{}", texts(&(def().render)(&p0)));
        // Ask on the angle with the hint on: the key names the inverse function
        let h = texts(&render_of(&[b("soh", true), b(SHOW_ANSWERS, true)]));
        assert!(h.contains("arctan(3/4) = 36.87\u{b0}") && h.contains("SOH") && h.contains("CAH") && h.contains("TOA"), "{h}");
        assert!(!texts(&render_of(&[])).contains("SOH"), "the hint is off by default");
    }

    /// No value that is asked for appears in the student copy (as text); the key shows every one of them.
    #[test]
    fn the_student_copy_never_prints_an_asked_value() {
        let mut cases = all_examples();
        cases.push(("defaults".into(), vec![]));
        let mut checked = 0;
        for (name, list) in cases {
            let p = with(&list);
            let c = ctx(&p);
            let mut stu = list.clone();
            stu.push(b(SHOW_ANSWERS, false));
            let mut key = list.clone();
            key.push(b(SHOW_ANSWERS, true));
            let (st, kt) = (texts(&render_of(&stu)), texts(&render_of(&key)));
            let given: Vec<String> = VALS.iter().filter(|e| c.md(e.0) == Md::Given).flat_map(|e| c.tokens(e.0)).collect();
            for e in VALS.iter().filter(|e| c.md(e.0) == Md::Ask) {
                for tok in c.tokens(e.0) {
                    assert!(has_tok(&kt, &tok), "{name}: the key lacks {tok}\n{kt}");
                    if given.contains(&tok) { continue; }
                    assert!(!has_tok(&st, &tok), "{name}: the student copy shows {tok} ({:?})\n{st}", e.0);
                    checked += 1;
                }
            }
            assert!(st.contains(BLANK) || sheet_values(&p).1 == 0, "{name}");
            // the two copies lay out alike: the same label boxes
            let (a, k) = (build(&with(&stu)).1, build(&with(&key)).1);
            assert_eq!(a.labels.len(), k.labels.len());
            for (x, y) in a.labels.iter().zip(&k.labels) { assert!(near(x.1.x0, y.1.x0) && near(x.1.y0, y.1.y0), "{name} {}", x.0); }
        }
        assert!(checked >= 20, "{checked}");
    }

    #[test]
    fn every_example_is_solvable_and_shows_a_given_value_when_it_asks() {
        let ex = all_examples();
        assert!(ex.len() >= 4);
        for (name, list) in ex {
            let p = with(&list);
            let (given, asks) = sheet_values(&p);
            assert!(asks > 0, "{name}");
            assert!(given >= 1, "{name}: asks for {asks} values and gives none");
            assert!(determined(&p), "{name}: the Given values do not fix the triangle");
            assert!(warnings(&p).is_empty(), "{name}: {:?}", warnings(&p));
            // a Given value is on the student sheet
            let t = texts(&render_of(&[list.clone(), vec![b(SHOW_ANSWERS, false)]].concat()));
            let c = ctx(&p);
            let shown = VALS.iter().filter(|e| c.md(e.0) == Md::Given).any(|e| c.tokens(e.0).iter().any(|k| has_tok(&t, k)));
            assert!(shown, "{name}: no Given value on the student sheet\n{t}");
        }
        // the shipped set covers the required setups
        let names: String = all_examples().iter().map(|e| e.0.clone()).collect::<Vec<_>>().join("|");
        for k in ["3-4-5", "5-12-13", "30-60-90", "missing side"] { assert!(names.contains(k), "{k}"); }
        // and the warnings fire for an unsolvable sheet
        let bad = with(&[s("g_opp", "Ask"), s("g_adj", "Ask")]);
        assert!(!warnings(&bad).is_empty());
        let one = with(&[s("g_adj", "Hidden")]);
        assert!(warnings(&one).iter().any(|w| w.contains("do not fix")), "{:?}", warnings(&one));
    }

    /// Print is exactly neutral (no channel differs) and the sides are ink, in every example with answers on and off.
    #[test]
    fn print_style_is_exactly_neutral_grey() {
        let mut cases = all_examples();
        cases.push(("defaults".into(), vec![]));
        for (name, list) in cases {
            for ans in [true, false] {
                let mut l = list.clone();
                l.push(s("style", "Print"));
                l.push(b(SHOW_ANSWERS, ans));
                l.push(b("soh", true));
                let svg = render_of(&l);
                let bad = crate::figures::census::non_grey_paints_within(&svg, 0);
                assert!(bad.is_empty(), "{name} answers {ans}: {bad:?}");
                assert!(svg.contains(r##"stroke="#111111" stroke-width="2.6""##), "{name}");
            }
        }
        assert!(!crate::figures::census::non_grey_paints_within(&render_of(&[]), 0).is_empty(), "Colour is coloured");
    }

    /// No label touches another label or a drawn line, nothing leaves the canvas, and the student and key copies place alike.
    #[test]
    fn labels_never_overlap_each_other_or_a_line() {
        let mut n = 0;
        for define in DEFINES {
            for at in ["A", "B"] {
                for words in ["Words", "Letters"] {
                    for th in [10.0, 15.0, 25.0, 36.87, 45.0, 60.0, 75.0, 80.0] {
                        for size in ["Small", "Medium", "Large"] {
                            for (unit, md) in [("cm", "Ask"), ("centimetres", "Given")] {
                                for hint in [false, true] {
                                    let mut l = vec![s("define", define), s("theta_at", at), s("side_labels", words), f("theta", th), s("label_size", size), s("unit", unit), b("soh", hint)];
                                    l.extend(VALS.iter().map(|e| s(e.1, md)));
                                    let (svg, r) = build(&with(&l));
                                    n += 1;
                                    for (i, (na, ra)) in r.labels.iter().enumerate() {
                                        for (nb, rb) in &r.labels[i + 1..] { assert!(!ra.hit(rb), "{define} {at} {words} {th} {size}: {na} touches {nb}"); }
                                        for sg in &r.segs { assert!(!seg_hits(sg.0, sg.1, ra), "{define} {at} {words} {th} {size} {unit}: {na} sits on a line"); }
                                        let (x0, y0, x1, y1) = (ra.x0 + r.shift.0, ra.y0 + r.shift.1, ra.x1 + r.shift.0, ra.y1 + r.shift.1);
                                        assert!(x0 >= 0.0 && y0 >= 0.0 && x1 <= r.w && y1 <= r.h, "{define} {at} {th} {size}: {na} outside the canvas {x0} {y0} {x1} {y1} of {} {}", r.w, r.h);
                                    }
                                    for rw in &r.rows { assert!(rw.x1 <= r.w && rw.y1 <= r.h); }
                                    assert!(svg.starts_with("<svg") && !svg.contains("NaN") && !svg.contains("inf"));
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(n >= 1920, "{n}");
    }

    /// Absurd numbers through the load path: nothing panics, the figure is drawn, no NaN or inf is printed.
    #[test]
    fn odd_values_are_clamped_not_fatal() {
        let d = def();
        let defs = (d.params)();
        for ext in [1e300, -1e300, 1e-300, 0.0, -1.0, 1e9, 0.1, 10000.0, 89.999, 90.0, 180.0, 1e15] {
            for define in DEFINES {
                let mut over = serde_json::Map::new();
                for pd in &defs {
                    match pd.kind {
                        Kind::Float { .. } => { over.insert(pd.key.to_string(), serde_json::json!(ext)); }
                        Kind::Int { .. } => { over.insert(pd.key.to_string(), serde_json::json!(ext as i64)); }
                        Kind::Text => { over.insert(pd.key.to_string(), serde_json::json!("<&\"'\u{e9}\u{4e2d}>".repeat(40))); }
                        _ => {}
                    }
                }
                over.insert("define".into(), serde_json::json!(define));
                over.insert("g_opp".into(), serde_json::json!("Ask"));
                let p = params_from_json(&d, &serde_json::Value::Object(over));
                let (svg, r) = build(&p);
                assert!(svg.len() > 300 && !svg.contains("NaN") && !svg.contains("inf"), "{define} {ext}");
                assert!(r.w < 4000.0 && r.h < 4000.0, "{define} {ext}: canvas {} x {}", r.w, r.h);
                let sol = solve(&p);
                assert!([sol.opp, sol.adj, sol.hyp, sol.th].iter().all(|x| x.is_finite() && *x > 0.0));
                let _ = warnings(&p);
                let _ = handles(ID, &p, (r.w, r.h));
            }
        }
        // a typed NaN and string numbers
        let mut p = with(&[]);
        p.set_f("opp_len", f64::NAN);
        p.set_f("theta", f64::INFINITY);
        p.set_s("define", "Angle and opposite");
        assert!(solve(&p).th.is_finite());
        // JSON integers and floats alike
        let p = params_from_json(&d, &serde_json::json!({"opp_len": 3, "adj_len": 4.0, "dec": 2.6}));
        assert!(near(solve(&p).hyp, 5.0) && p.i("dec") == 3);
    }

    #[test]
    fn an_extreme_shape_is_clamped_with_a_note() {
        let p = with(&[s("define", "Two legs"), f("opp_len", 1.0), f("adj_len", 1000.0)]);
        let sl = solve(&p);
        assert!(sl.clamped && near(sl.th, TH_MIN) && near(sl.opp, 1.0));
        assert!(warnings(&p).iter().any(|w| w.contains("kept between")));
        let q = with(&[s("define", "Leg and hypotenuse"), f("opp_len", 9.0), f("hyp_len", 3.0)]);
        let sq = solve(&q);
        assert!(sq.clamped && near(sq.th, TH_MAX) && sq.hyp > sq.opp);
        assert!(!solve(&with(&[])).clamped);
    }

    /// The right angle really is one: C is at 90 degrees in the drawing, and the drawn sides keep the solved ratio.
    #[test]
    fn the_drawing_is_to_scale() {
        for at in ["A", "B"] {
            let p = with(&[s("theta_at", at), f("opp_len", 5.0), f("adj_len", 12.0)]);
            let r = build(&p).1;
            let (t, c, o) = (r.t, r.c, r.o);
            let (u, w) = ((t.0 - c.0, t.1 - c.1), (o.0 - c.0, o.1 - c.1));
            assert!((u.0 * w.0 + u.1 * w.1).abs() < 1e-6 * u.0.hypot(u.1) * w.0.hypot(w.1));
            let ratio = dist(o, c) / dist(t, c);
            assert!((ratio - 5.0 / 12.0).abs() < 1e-9, "{at}: {ratio}");
            // theta at T is the angle between T-C and T-O
            let th = ((dist(t, c)) / dist(t, o)).acos().to_degrees();
            assert!((th - solve(&p).th).abs() < 1e-9);
        }
    }

    #[test]
    fn dragging_the_far_vertex_sets_the_angle() {
        let p = with(&[]);
        let hs = handles(ID, &p, (0.0, 0.0)).unwrap();
        assert_eq!(hs.len(), 1);
        let r = build(&p).1;
        let drag = hs[0].drag.clone().unwrap();
        // 45 degrees at T (theta at A, T bottom left, C to its right)
        let target = (r.t.0 + 100.0 * 0.5f64.sqrt(), r.t.1 - 100.0 * 0.5f64.sqrt());
        let mut q = p.clone();
        drag(&mut q, target);
        assert!((solve(&q).th - 45.0).abs() < 0.01, "{}", solve(&q).th);
        // grabbed and not moved: nothing changes; a hostile pointer changes nothing and does not panic
        let mut same = p.clone();
        drag(&mut same, r.o);
        drag(&mut same, (f64::NAN, 3.0));
        assert_eq!(params_key(&same), params_key(&p));
        // the angle definitions write the angle
        let a = with(&[s("define", "Angle and adjacent"), f("adj_len", 12.0)]);
        let ha = handles(ID, &a, (0.0, 0.0)).unwrap();
        let ra = build(&a).1;
        let mut qa = a.clone();
        (ha[0].drag.clone().unwrap())(&mut qa, (ra.t.0 + 100.0 * 0.5f64.sqrt(), ra.t.1 - 100.0 * 0.5f64.sqrt()));
        assert!((qa.f("theta") - 45.0).abs() < 1e-9 && near(qa.f("adj_len"), 12.0));
        // far outside the range is clamped
        let mut qc = p.clone();
        drag(&mut qc, (r.t.0 + 100.0, r.t.1 - 1.0));
        assert!(solve(&qc).th >= TH_MIN - 1e-9);
    }

    #[test]
    fn the_faq_has_at_most_six_entries() {
        let q = super::super::faq::faq_for(ID);
        assert!(!q.is_empty() && q.len() <= 6, "{}", q.len());
    }

    /// Print at 2x and a 66% bitonal threshold: the three sides stay solid.
    #[test]
    fn print_survives_a_bitonal_copy() {
        for at in ["A", "B"] {
            let p = with(&[s("style", "Print"), s("theta_at", at), b(SHOW_ANSWERS, true)]);
            let (svg, rep) = build(&p);
            let sc = 2.0f32;
            let (w, h, px) = to_rgba(&svg, sc).unwrap();
            let dark = |x: f64, y: f64| -> bool {
                let (cx, cy) = ((x * sc as f64).round() as i64, (y * sc as f64).round() as i64);
                for dy in -1..=1 { for dx in -1..=1 {
                    let (xx, yy) = (cx + dx, cy + dy);
                    if xx < 0 || yy < 0 || xx >= w as i64 || yy >= h as i64 { continue; }
                    let c = &px[((yy as u32 * w + xx as u32) * 4) as usize..][..4];
                    let a = c[3] as f64 / 255.0;
                    let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a);
                    if l < 0.66 * 255.0 { return true; }
                } }
                false
            };
            let (t, c, o) = (rep.t, rep.c, rep.o);
            for (a, b2) in [(t, c), (c, o), (t, o)] {
                let n = 60;
                let dark_n = (3..n - 3).filter(|i| { let k = *i as f64 / n as f64; dark(a.0 + (b2.0 - a.0) * k, a.1 + (b2.1 - a.1) * k) }).count();
                assert!(dark_n >= n - 6, "{at}: a side is {dark_n}/{} dark", n - 6);
            }
        }
    }

    /// Writes every example (Colour, Print, student, key, student Print, Print at a 66% bitonal threshold) to $VERSHION_PNG_DIR.
    #[test]
    fn trig_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![("default".into(), vec![])];
        for (i, (_, l)) in all_examples().into_iter().enumerate() { cases.push((format!("ex{i}"), l)); }
        cases.push(("letters".into(), vec![s("side_labels", "Letters"), s("theta_at", "B"), b("soh", true)]));
        cases.push(("thin10".into(), vec![s("define", "Angle and adjacent"), f("theta", 10.0), f("adj_len", 20.0), s("label_size", "Large")]));
        cases.push(("steep80".into(), vec![s("define", "Angle and adjacent"), f("theta", 80.0), f("adj_len", 2.0), s("theta_at", "B")]));
        cases.push(("long".into(), vec![s("unit", "centimetres"), s("label_size", "Large"), b("soh", true), f("opp_len", 1234.5), f("adj_len", 2345.6)]));
        for (tag, list) in cases {
            for (kind, extra) in [("colour", vec![]), ("print", vec![s("style", "Print")]), ("student", vec![b(SHOW_ANSWERS, false)]), ("key", vec![b(SHOW_ANSWERS, true)]), ("student-print", vec![s("style", "Print"), b(SHOW_ANSWERS, false)])] {
                let mut l = list.clone();
                l.extend(extra);
                let svg = render_of(&l);
                std::fs::write(format!("{dir}/trig-{tag}-{kind}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
                if kind == "print" {
                    let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
                    let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                    image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/trig-{tag}-print-bw66.png")).unwrap();
                }
            }
        }
    }
}
