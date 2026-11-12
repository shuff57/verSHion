//! Pulley figure: Atwood machine (with an optional third mass), block on a table or a ramp tied
//! over a pulley to a hanging mass, and a block and tackle with 2-5 supporting strands.
use super::common::*;
use crate::figures::{ParamDef, Params};

const LM: [&str; 3] = ["Text", "Blank line", "None"];

/// Which controls the chosen setup uses (as the reference shows them): Atwood has the two
/// hanging masses, "hangs lower" and the third mass; table and ramp the cart/block choice, a
/// second object, the surface, normal force and friction (the ramp also its angle); the block
/// and tackle its strands and the load. Everything else stays put but out of sight.
pub(crate) fn pulley_relevant(key: &str, p: &Params) -> bool {
    let setup = p.s("setup");
    let (atwood, tackle) = (setup == "Atwood", setup == "Block & tackle");
    let flat = !atwood && !tackle; // table or ramp
    match key {
        "l_kind" | "second" | "surface" | "normal" | "norm_lab_mode" | "norm_lab" | "friction" | "fric_lab_mode" | "fric_lab" => flat,
        "s_lab" => flat && p.b("second") && !super::pulley_b10::refused(p),
        "joined" => flat && p.b("second"),
        "contact" => touch(p).is_some(),
        "cont_lab_mode" => touch(p).is_some() && p.b("contact"),
        "cont_lab" => touch(p).is_some() && p.b("contact") && p.s("cont_lab_mode") == "Text",
        "angle" | "ang_lab_mode" | "ang_lab" => setup == "Ramp",
        "lower" | "third" => atwood,
        "third_side" | "t_lab_mode" | "t_lab" | "t_size" => atwood && p.b("third"),
        "r_lab_mode" | "r_lab" | "r_size" => !tackle,
        "strands" => tackle,
        _ => true,
    }
}

/// The panel changed the setup from `old`: a block and tackle's load reads "m" (the reference's
/// label) while the other setups' left object reads m_1, so a label still at the other setup's
/// default follows the switch. A typed label is kept, and saved figures are not touched.
pub(crate) fn pulley_setup_changed(old: &str, p: &mut Params) {
    const TACKLE: &str = "Block & tackle";
    let new = p.s("setup").to_string();
    match (old == TACKLE, new == TACKLE, p.s("l_lab")) {
        (false, true, "m_1") => p.set_s("l_lab", "m"),
        (true, false, "m") => p.set_s("l_lab", "m_1"),
        _ => {}
    }
}

pub(super) fn pulley_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("setup", "Setup", &["Atwood", "Table", "Ramp", "Block & tackle"], "Atwood"),
        ParamDef::choice("l_kind", "Object on the table / ramp", &["Block", "Cart"], "Block"),
        ParamDef::choice("l_lab_mode", "Left (table / ramp / load) label shows", &LM, "Text"),
        ParamDef::text("l_lab", "Left object label", "m_1"),
        ParamDef::int("l_size", "Left object size (%)", 50, 200, 100),
        ParamDef::choice("r_lab_mode", "Right (hanging) label shows", &LM, "Text"),
        ParamDef::text("r_lab", "Right object label", "m_2"),
        ParamDef::int("r_size", "Right object size (%)", 50, 200, 100),
        ParamDef::choice("lower", "Hangs lower", &["Neither", "Left", "Right"], "Neither"),
        ParamDef::bool("third", "Hang a third object below", false),
        ParamDef::choice("third_side", "The third hangs below", &["Left", "Right"], "Right"),
        ParamDef::choice("t_lab_mode", "Third object label shows", &LM, "Text"),
        ParamDef::text("t_lab", "Third object label", "m_3"),
        ParamDef::int("t_size", "Third object size (%)", 50, 200, 77),
        ParamDef::bool("second", "Add an object on the table / ramp, tied behind", false),
        ParamDef::text("s_lab", "Its label", "m_3"),
        ParamDef::choice("joined", "Joined to the first", &super::pulley_b10::JOINED, "By a string"),
        ParamDef::int("strands", "Block & tackle: strands holding up the load", 2, 5, 2),
        ParamDef::float("angle", "Ramp angle (deg)", 15.0, 60.0, 1.0, 30.0),
        ParamDef::choice("ang_lab_mode", "Angle label shows", &LM, "Text"),
        ParamDef::text("ang_lab", "Angle label", "theta"),
        ParamDef::choice("surface", "Surface (table / ramp)", &["Smooth", "Rough"], "Smooth"),
        ParamDef::bool("tension", "Tension", false),
        ParamDef::choice("tens_lab_mode", "Tension label shows", &LM, "Text"),
        ParamDef::text("tens_lab", "Tension label", "T"),
        ParamDef::bool("gravity", "Gravity", false),
        ParamDef::choice("grav_lab_mode", "Gravity labels show", &LM, "Text"),
        ParamDef::text("grav_lab", "Gravity label (blank = the object's label + g)", ""),
        ParamDef::bool("normal", "Normal force (table / ramp)", false),
        ParamDef::choice("norm_lab_mode", "Normal force label shows", &LM, "Text"),
        ParamDef::text("norm_lab", "Normal force label", "F_N"),
        ParamDef::choice("friction", "Friction (table / ramp)", &["None", "Toward the pulley", "Away from it"], "None"),
        ParamDef::choice("fric_lab_mode", "Friction label shows", &LM, "Text"),
        ParamDef::text("fric_lab", "Friction label", "F_f"),
        ParamDef::bool("contact", "Contact forces, where the objects touch", false),
        ParamDef::choice("cont_lab_mode", "Contact force labels show", &LM, "Text"),
        ParamDef::text("cont_lab", "Contact force label", "P"),
        ParamDef::choice("accel", "Acceleration (right / hanging / load falls)", &["None", "Right falls", "Left falls"], "None"),
        ParamDef::choice("acc_lab_mode", "Acceleration label shows", &LM, "Text"),
        ParamDef::text("acc_lab", "Acceleration label", "a"),
        ParamDef::bool("mirror", "Mirror (flip left to right)", false),
        ParamDef::bool("color", "Color (for slides)", false),
    ]
}

/// A box the handles can grab: centre x, top y, size, its rest top y, which side "Hangs lower"
/// names it (empty = none), its size key and label key.
pub(super) struct PBox { pub cx: f64, pub top: f64, pub s: f64, pub rest: f64, pub side: &'static str, pub size_key: &'static str, pub lab_key: &'static str }

pub(super) struct PGeo { pub w: f64, pub h: f64, pub boxes: Vec<PBox>, pub ramp: Option<(f64, f64)> }

fn sz(p: &Params, k: &str) -> f64 { 60.0 * p.f(k).clamp(50.0, 200.0) / 100.0 }

/// Ramp / table frame: foot (ax, ground), up-slope unit, outward normal, apex, ramp length.
struct Slope { ax: f64, ground: f64, uu: Pt, nn: Pt, apex: Pt }

fn slope(p: &Params) -> Slope { slope_with(p, touch(p).map(|t| (t.h, t.hl, t.top))) }

/// The surface frame. Objects touching on a ramp, `(string height, half length, height)`: the string runs high over
/// the second object, so the ramp moves down the page (which grows) to keep the wheel 48 px below the top, and right
/// (the page widens) so the first object's normal force and its label stay on the page.
fn slope_with(p: &Params, tp: Option<(f64, f64, f64)>) -> Slope {
    if p.s("setup") == "Ramp" {
        let th = p.f("angle").clamp(15.0, 60.0).to_radians();
        let bw = (250.0 / th.tan()).min(330.0);
        let (mut ax, mut ground): (f64, f64) = (40.0, 420.0);
        if let Some((h, hl, top)) = tp {
            // the wheel's tangent point sits h / cos(theta) above the ramp's top corner
            ground = ground.max(48.0 + bw * th.tan() + h / th.cos());
            if p.b("normal") {
                // the normal force at the first object's rear (its farthest back), 46 long, its label beyond the tip
                let len = bw / th.cos();
                let s1 = (len * 0.5).min(len - 6.0 - 3.0 * hl);
                let tip = (s1 - hl + 3.0) * th.cos() - (top + 46.0) * th.sin();
                let (w, _, _) = super::pulley_b10::tex_box(&lab_mode(p, "norm_lab", "norm_lab_mode"), 22.0);
                let d = norm_lab_dir((th.cos(), -th.sin()), (-th.sin(), -th.cos()));
                let lx = tip + 10.0 * d.0;
                let left = if d.0 < -0.4 { lx - w } else { lx - w / 2.0 };
                ax = ax.max(8.0 - left);
            }
        }
        Slope { ax, ground, uu: (th.cos(), -th.sin()), nn: (-th.sin(), -th.cos()), apex: (ax + bw, ground - bw * th.tan()) }
    } else {
        // the table top acts as a flat "ramp"; objects touching: it moves down the page as far as the string over
        // them, the acceleration above it (34 px) and its label need
        let ground = match tp { Some((h, _, _)) => (230.0f64).max(h + 90.0), None => 230.0 };
        Slope { ax: 50.0, ground, uu: (1.0, 0.0), nn: (0.0, -1.0), apex: (340.0, ground) }
    }
}

/// Pulley wheel position for the table / ramp: tangent to a string parallel to the surface at the
/// first object's mid height.
/// On the table the wheel is big and sits at the table's edge, its centre level with the table
/// top's middle (as on the reference), within 22..36 for very small / big objects.
fn slope_len(sl: &Slope) -> f64 { ((sl.apex.0 - sl.ax).powi(2) + (sl.apex.1 - sl.ground).powi(2)).sqrt() }

/// Half length along the surface and full height of the object on the table or ramp, size `ls` px.
fn obj_dims(p: &Params, ls: f64) -> (f64, f64) { if p.s("l_kind") == "Cart" { (ls * 0.7, ls * 0.76) } else { (ls / 2.0, ls) } }

/// How far above the objects' tops the string runs when they touch (it leaves the first object from a post and
/// passes over the second, which sits between the first and the pulley).
pub(super) const POST: f64 = 54.0;

/// Objects touching on the table or ramp, as drawn: the first object's centre along the surface, the objects' half
/// length and height, the string's height above the surface.
pub(super) struct Touch { pub s1: f64, pub hl: f64, pub top: f64, pub h: f64 }

/// The touching pair's layout, or None when not asked for or when the two do not fit on the surface side by side.
pub(super) fn touch(p: &Params) -> Option<Touch> {
    if !super::pulley_b10::asked(p) { return None; }
    let (hl, top) = obj_dims(p, sz(p, "l_size"));
    let h = top + POST;
    let sl = slope_with(p, Some((h, hl, top)));
    let len = slope_len(&sl);
    let ramp = p.s("setup") == "Ramp";
    let s_main = len * if ramp { 0.5 } else { 0.55 };
    // the second object (in front) ends 6 px short of the far end; the first starts at least 4 px from the table's
    // near end, or clear of the ramp's angle mark and its label (out to 64 px from the foot)
    let s1 = s_main.min(len - 6.0 - 3.0 * hl);
    if s1 - hl < if ramp { 84.0 } else { 4.0 } { return None; }
    if !labels_fit(p, &sl) { return None; }
    Some(Touch { s1, hl, top, h })
}

/// Both objects' labels fit inside them (at 8 px at the least, as `slope_setup` sizes them).
fn labels_fit(p: &Params, sl: &Slope) -> bool {
    let ls = sz(p, "l_size");
    let (hl, _) = obj_dims(p, ls);
    let up = if p.s("l_kind") == "Cart" { ls * 0.25 } else { ls / 2.0 };
    [lab_mode(p, "l_lab", "l_lab_mode"), p.s("s_lab").to_string()].iter().all(|lab| {
        let t = lab.trim();
        t.eq_ignore_ascii_case("none") || t.is_empty() || super::pulley_b10::fits(t, super::pulley_b10::fit_size(t, 22.0, 2.0 * hl - 8.0, up - 3.75, sl.uu.1.abs()), 2.0 * hl - 8.0, up - 3.75, sl.uu.1.abs())
    })
}

/// Asked for touching, and the two would fit side by side but for their labels.
pub(super) fn touch_fits_side_by_side(p: &Params) -> bool {
    if !super::pulley_b10::asked(p) { return false; }
    let (hl, top) = obj_dims(p, sz(p, "l_size"));
    let sl = slope_with(p, Some((top + POST, hl, top)));
    let len = slope_len(&sl);
    let s1 = (len * if p.s("setup") == "Ramp" { 0.5 } else { 0.55 }).min(len - 6.0 - 3.0 * hl);
    s1 - hl >= if p.s("setup") == "Ramp" { 84.0 } else { 4.0 } && !labels_fit(p, &sl)
}

/// Objects touching, along the surface: the contact pair's centre (where they meet) and its height above the surface,
/// each arrow's length, the post the string leaves the first object from (behind the pair's back arrow) and the first
/// object's normal force (behind the post, so the string never crosses it).
pub(super) struct PairGeo { pub i: f64, pub hc: f64, pub len: f64, pub post: f64, pub n_at: f64 }

pub(super) fn pair_geo(t: &Touch) -> PairGeo {
    let i = t.s1 + t.hl;
    let len = (2.0 * t.hl - 21.0).clamp(10.0, 28.0);
    let post = i - len - 11.0;
    let n_at = (post - 12.0).min(t.s1).max(t.s1 - t.hl + 3.0);
    PairGeo { i, hc: t.top + 12.0, len, post, n_at }
}

/// Objects touching: the direction the first object's normal force label sits in from its tip (up and back).
fn norm_lab_dir(uu: Pt, nn: Pt) -> Pt {
    let d = (nn.0 - 0.9 * uu.0, nn.1 - 0.9 * uu.1);
    let l = d.0.hypot(d.1);
    (d.0 / l, d.1 / l)
}

/// An arrow's strokes as `G::arrow` draws them: the shaft and the head's three edges.
pub(super) fn arrow_segs(a: Pt, b: Pt, sw: f64) -> Vec<super::pulley_b10::Seg> {
    use super::pulley_b10::Seg;
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l = (dx * dx + dy * dy).sqrt().max(1e-6);
    let (ux, uy) = (dx / l, dy / l);
    let hl = head_len(sw).min(l * 0.6);
    let base = (b.0 - ux * hl, b.1 - uy * hl);
    let (c1, c2) = ((base.0 - uy * hl * 0.45, base.1 + ux * hl * 0.45), (base.0 + uy * hl * 0.45, base.1 - ux * hl * 0.45));
    vec![Seg { a, b: (b.0 - ux * hl * 0.8, b.1 - uy * hl * 0.8), hw: sw / 2.0 }, Seg { a: b, b: c1, hw: 0.25 }, Seg { a: c1, b: c2, hw: 0.25 }, Seg { a: c2, b, hw: 0.25 }]
}

/// The contact pair's two arrows (on the second object, toward the pulley, then on the first), as drawn (page
/// coordinates before mirroring): from where the objects meet, apart.
pub(super) fn pair_arrows(p: &Params, t: &Touch) -> [(Pt, Pt); 2] {
    let sl = slope(p);
    let at = |l: f64, h: f64| (sl.ax + sl.uu.0 * l + sl.nn.0 * h, sl.ground + sl.uu.1 * l + sl.nn.1 * h);
    let g = pair_geo(t);
    [(at(g.i, g.hc), at(g.i + g.len, g.hc)), (at(g.i, g.hc), at(g.i - g.len, g.hc))]
}

/// Where the pair's one label goes (page coordinates as drawn, mirrored or not), with a leader from the pair when it
/// sits away from it. None inside: the teacher's label has no place anywhere near that reads as the pair's, and is
/// left off with a note. Outer None: no label to place (no pair, or label set to None).
pub(super) fn contact_label(p: &Params) -> Option<Option<super::pulley_b10::LabSpot>> {
    let t = touch(p)?;
    if !p.b("contact") { return None; }
    let lab = lab_mode(p, "cont_lab", "cont_lab_mode");
    if lab.trim().eq_ignore_ascii_case("none") { return None; }
    // the drawing without the pair: everything the label must keep clear of
    let base = render_with(p, PairDraw::Skip);
    let geo = pulley_geo(p);
    let mir = |q: Pt| if p.b("mirror") { (geo.w - q.0, q.1) } else { q };
    let arrows = pair_arrows(p, &t);
    let own: Vec<super::pulley_b10::Seg> = arrows.iter().flat_map(|(a, b)| arrow_segs(mir(*a), mir(*b), SW)).collect();
    let sl = slope(p);
    let g = pair_geo(&t);
    let at = |l: f64, h: f64| mir((sl.ax + sl.uu.0 * l + sl.nn.0 * h, sl.ground + sl.uu.1 * l + sl.nn.1 * h));
    // the ideal spot: just above the pair's middle, as the inclined plane puts it
    let anchor = at(g.i, g.hc + 0.45 * head_len(SW));
    let foot = at(g.i, g.hc);
    Some(super::pulley_b10::place_label(&base, &lab, 22.0, anchor, &own, (geo.w, geo.h)).map(|mut sp| {
        // a leader starts on the pair's shaft, where the objects meet
        if let Some((_, b)) = sp.leader { sp.leader = Some((foot, b)); }
        sp
    }))
}

/// How `slope_setup` draws the contact pair: not at all (the placing pass), or with its label at a found spot.
#[derive(Clone)]
pub(super) enum PairDraw { Skip, Draw(Option<super::pulley_b10::LabSpot>) }

fn slope_wheel(sl: &Slope, hh: f64) -> (Pt, f64) {
    let flat = sl.uu.1.abs() < 1e-9;
    let pr = if flat { (hh + 6.0).clamp(22.0, 36.0) } else { 22.0 };
    let q = if flat { (sl.apex.0 + 10.0 + pr, sl.apex.1 - hh) } else {
        let t = (-sl.uu.1 / sl.uu.0).abs(); // tan(theta)
        (sl.apex.0 + sl.nn.0 * hh + sl.uu.0 * hh * t, sl.apex.1 + sl.nn.1 * hh + sl.uu.1 * hh * t)
    };
    ((q.0 - sl.nn.0 * pr, q.1 - sl.nn.1 * pr), pr)
}

pub(super) fn pulley_geo(p: &Params) -> PGeo {
    let lower = p.s("lower");
    let (dl, dr) = (if lower == "Left" { 45.0 } else { 0.0 }, if lower == "Right" { 45.0 } else { 0.0 });
    let (ls, rs) = (sz(p, "l_size"), sz(p, "r_size"));
    let mut boxes = vec![];
    match p.s("setup") {
        "Table" | "Ramp" => {
            let ramp = p.s("setup") == "Ramp";
            let sl = slope(p);
            let hh = touch(p).map(|t| t.h).unwrap_or_else(|| mid_h(p, ls));
            let (c, pr) = slope_wheel(&sl, hh);
            let mut rest = c.1 + pr + 82.0;
            if ramp {
                // keep the hanging mass and its weight label above the ground; objects touching (the wheel raised
                // over the second object): where the old figure hangs it, moved down with the ramp
                let (c0, sl0) = if touch(p).is_some() { let s0 = slope_with(p, None); (slope_wheel(&s0, mid_h(p, ls)).0, s0) } else { (c, slope_with(p, None)) };
                let old = (c0.1 + pr + 82.0).min(sl0.ground - dr - rs - if p.b("gravity") { 84.0 } else { 20.0 }).max(c0.1 + 50.0);
                rest = if touch(p).is_some() { old + (sl.ground - sl0.ground) } else { rest.min(sl.ground - dr - rs - if p.b("gravity") { 84.0 } else { 20.0 }).max(c.1 + 50.0) };
            }
            boxes.push(PBox { cx: c.0 + pr, top: rest + dr, s: rs, rest, side: "Right", size_key: "r_size", lab_key: "r_lab" });
            // grow the canvas when a big or lowered hanging mass would run off the bottom
            let need = rest + dr + rs + if p.b("gravity") { 116.0 } else { 50.0 };
            let wide = if ramp && touch(p).is_some() { sl.apex.0 + 190.0 } else { 0.0 };
            let (w, h) = (wide.max(560.0), need.max(500.0).max(if ramp { sl.ground + 80.0 } else { 0.0 }));
            return PGeo { w, h, boxes, ramp: if ramp { Some((sl.ax, sl.ground)) } else { None } };
        }
        "Block & tackle" => {
            let n = p.i("strands").clamp(2, 5) as f64;
            let r = 22.0;
            let x0 = 240.0 - (n - 1.0) * r;
            let bar = 220.0 + r + 14.0;
            let lx = x0 + (n - 1.0) * r + if n as i64 % 2 == 0 { r } else { 0.0 };
            let _ = dl;
            boxes.push(PBox { cx: lx, top: bar + 26.0, s: ls, rest: bar + 26.0, side: "", size_key: "l_size", lab_key: "l_lab" });
        }
        _ => {
            boxes.push(PBox { cx: 240.0 - 42.0, top: 235.0 + dl, s: ls, rest: 235.0, side: "Left", size_key: "l_size", lab_key: "l_lab" });
            boxes.push(PBox { cx: 240.0 + 42.0, top: 235.0 + dr, s: rs, rest: 235.0, side: "Right", size_key: "r_size", lab_key: "r_lab" });
        }
    }
    let mut h: f64 = 430.0;
    if p.b("third") && p.s("setup") == "Atwood" {
        let b = boxes.iter().map(|b| b.top + b.s).fold(0.0, f64::max);
        h = h.max(b + 96.0 + sz(p, "t_size") + 80.0);
    }
    PGeo { w: 480.0, h, boxes, ramp: None }
}

/// Height above the surface of the string's attachment (the object's middle).
fn mid_h(p: &Params, ls: f64) -> f64 { if p.s("l_kind") == "Cart" { ls * 0.51 } else { ls / 2.0 } }

fn wheel(g: &mut G, cx: f64, cy: f64, r: f64) {
    g.circle(cx, cy, r, "#f3f4f7", INK, SW);
    g.circle(cx, cy, r * 0.34, "none", INK, 1.6);
    g.circle(cx, cy, 2.4, INK, INK, 1.0);
}

fn lab_at(g: &mut G, x: f64, y: f64, t: &str, anchor: &str) {
    if t.eq_ignore_ascii_case("none") { return; }
    if t.trim().is_empty() {
        let (a, b) = match anchor { "start" => (x, x + 24.0), "end" => (x - 24.0, x), _ => (x - 12.0, x + 12.0) };
        g.line(a, y + 2.0, b, y + 2.0, INK, 1.4);
    } else {
        g.tex(x, y, 22.0, anchor, t, ITALIC);
    }
}

/// Weight label for an object labelled `obj` (blank text = "<label> g").
fn weight_lab(p: &Params, obj: &str) -> String {
    match p.s("grav_lab_mode") {
        "Blank line" => return String::new(),
        "None" => return "none".into(),
        _ => {}
    }
    let t = p.s("grav_lab").trim();
    if !t.is_empty() { return t.to_string(); }
    let o = obj.trim();
    if o.is_empty() || o == "none" { "F_g".into() } else { format!("{o} g") }
}

/// Square box with its label; returns nothing (geometry is known to the caller).
fn mass_box(g: &mut G, cx: f64, top: f64, s: f64, lab: &str) {
    g.rect(cx - s / 2.0, top, s, s, FILL, INK, SW);
    if lab.trim().is_empty() { g.line(cx - 11.0, top + s / 2.0 + 6.0, cx + 11.0, top + s / 2.0 + 6.0, INK, 1.4); }
    else if lab != "none" { g.tex(cx, top + s / 2.0 + 7.0, 22.0, "middle", lab, ITALIC); }
}

struct Style { col: bool, tens: String, acc: String }

impl Style {
    fn tc(&self) -> &'static str { if self.col { PAL[0] } else { INK } }
    fn gc(&self) -> &'static str { if self.col { PAL[1] } else { INK } }
    fn ac(&self) -> &'static str { if self.col { PAL[2] } else { INK } }
}

/// Tension arrow of length 30 from `a` along unit `d`, label on side `lx` (-1 left / 1 right).
fn t_arrow(g: &mut G, st: &Style, a: Pt, d: Pt, lab: &str, side: f64) {
    let b = (a.0 + d.0 * 30.0, a.1 + d.1 * 30.0);
    g.arrow(a.0, a.1, b.0, b.1, st.tc(), 2.2);
    // label on screen-side `side` for vertical arrows; above (side > 0) / below for others
    let (mut nx, mut ny) = (-d.1, d.0);
    if nx.abs() > 0.3 { if nx * side < 0.0 { nx = -nx; ny = -ny; } } else if (ny < 0.0) != (side > 0.0) { nx = -nx; ny = -ny; }
    let m = ((a.0 + b.0) / 2.0 + nx * 14.0, (a.1 + b.1) / 2.0 + ny * 14.0 + 6.0);
    lab_at(g, m.0, m.1, lab, if nx > 0.3 { "start" } else if nx < -0.3 { "end" } else { "middle" });
}

/// Tension arrow of length 30 from `a` along unit `d` on a string that runs along the table /
/// ramp, its label on the side of the outward normal `n` (above the string, clear of the wheel).
fn t_arrow_along(g: &mut G, st: &Style, a: Pt, d: Pt, lab: &str, n: Pt) { t_arrow_along_off(g, st, a, d, lab, n, 14.0) }

/// `t_arrow_along` with the label `off` px from the string (objects touching: 18, clear of the arrow's own head on
/// a shallow ramp).
fn t_arrow_along_off(g: &mut G, st: &Style, a: Pt, d: Pt, lab: &str, n: Pt, off: f64) {
    let b = (a.0 + d.0 * 30.0, a.1 + d.1 * 30.0);
    g.arrow(a.0, a.1, b.0, b.1, st.tc(), 2.2);
    let m = ((a.0 + b.0) / 2.0 + n.0 * off, (a.1 + b.1) / 2.0 + n.1 * off + 6.0);
    lab_at(g, m.0, m.1, lab, if n.0 > 0.3 { "start" } else if n.0 < -0.3 { "end" } else { "middle" });
}

/// Weight arrow from the bottom of a hanging box.
fn w_arrow(g: &mut G, st: &Style, x: f64, y: f64, lab: &str) {
    g.arrow(x, y, x, y + 44.0, st.gc(), SW);
    lab_at(g, x, y + 66.0, lab, "middle");
}

/// Dashed acceleration arrow beside a hanging box (down when `down`).
fn a_arrow(g: &mut G, st: &Style, x: f64, top: f64, s: f64, down: bool, side: f64) {
    let (a, b) = if down { (top + s * 0.2, top + s * 0.2 + 34.0) } else { (top + s * 0.2 + 34.0, top + s * 0.2) };
    g.dash_arrow(x, a, x, b, st.ac(), 2.2);
    lab_at(g, x + side * 10.0, (a + b) / 2.0 + 6.0, &st.acc, if side > 0.0 { "start" } else { "end" });
}

pub(super) fn pulley_render(p: &Params) -> String {
    let lab = contact_label(p).flatten();
    render_with(p, PairDraw::Draw(lab))
}

fn render_with(p: &Params, pd: PairDraw) -> String {
    let geo = pulley_geo(p);
    let mut g = G::new(geo.w, geo.h, p.b("mirror"));
    let st = Style { col: p.b("color"), tens: lab_mode(p, "tens_lab", "tens_lab_mode"), acc: lab_mode(p, "acc_lab", "acc_lab_mode") };
    let (llab, rlab, tlab) = (lab_mode(p, "l_lab", "l_lab_mode"), lab_mode(p, "r_lab", "r_lab_mode"), lab_mode(p, "t_lab", "t_lab_mode"));
    let accel = p.s("accel").to_string();
    let (ten, grav) = (p.b("tension"), p.b("gravity"));
    match p.s("setup") {
        "Table" | "Ramp" => slope_setup(&mut g, p, &geo, &st, &llab, &rlab, &pd),
        "Block & tackle" => {
            let n = p.i("strands").clamp(2, 5) as usize;
            let r = 22.0;
            let x0 = 240.0 - (n as f64 - 1.0) * r;
            let (ty, by) = (100.0, 220.0);
            let bx = &geo.boxes[0];
            let bar = by + r + 14.0;
            let top_bar = 40.0;
            let (cl, cr) = (x0 - r - 40.0, x0 + (n as f64) * 2.0 * r + 30.0);
            g.hatch(cl, cr, top_bar, false);
            // pulleys alternate top (fixed) and bottom (moving); rope zigzags between them
            let pcx = |k: usize| x0 + 2.0 * r * k as f64;
            let pcy = |k: usize| if k % 2 == 0 { ty } else { by };
            // bottom pulleys hang from the load bar, top pulleys from the ceiling
            let mut bxs = vec![];
            for k in 0..n {
                let (x, y) = (pcx(k), pcy(k));
                if k % 2 == 0 { g.line(x, top_bar, x, y, INK, SW); } else { g.line(x, y, x, bar, INK, SW); bxs.push(x); }
            }
            // rope
            let pull_x = pcx(0) - r;
            g.line(pull_x, ty, pull_x, ty + 150.0, INK, 2.0);
            for k in 0..n {
                let (x, y) = (pcx(k), pcy(k));
                if k % 2 == 0 { g.polyline(&arc_pts(x, y, r, 180.0, 0.0), INK, 2.0); } else { g.polyline(&arc_pts(x, y, r, 180.0, 360.0), INK, 2.0); }
                if k + 1 < n { let nx = pcx(k + 1) - r; g.line(x + r, y, nx, pcy(k + 1), INK, 2.0); }
            }
            let lastx = pcx(n - 1) + r;
            let end_y = if (n - 1) % 2 == 0 { bar } else { top_bar };
            g.line(lastx, pcy(n - 1), lastx, end_y, INK, 2.0);
            let (b0, b1) = (bxs.first().copied().unwrap_or(lastx).min(lastx) - 10.0, bxs.last().copied().unwrap_or(lastx).max(lastx) + 10.0);
            g.line(b0, bar, b1, bar, INK, 4.0);
            for k in 0..n { let (x, y) = (pcx(k), pcy(k)); wheel(&mut g, x, y, r); }
            g.line(bx.cx, bar, bx.cx, bx.top, INK, 2.0);
            mass_box(&mut g, bx.cx, bx.top, bx.s, &llab);
            if ten {
                // pull end, and every strand holding the load
                t_arrow(&mut g, &st, (pull_x, ty + 110.0), (0.0, 1.0), &st.tens, -1.0);
                let mut xs: Vec<f64> = (0..n.saturating_sub(1)).map(|k| pcx(k) + r).collect();
                xs.push(lastx);
                let ym = (ty + by) / 2.0 + 16.0;
                for x in xs { t_arrow(&mut g, &st, (x, ym), (0.0, -1.0), &st.tens, 1.0); }
            }
            if grav { w_arrow(&mut g, &st, bx.cx, bx.top + bx.s, &weight_lab(p, &llab)); }
            if accel != "None" {
                a_arrow(&mut g, &st, bx.cx + bx.s / 2.0 + 16.0, bx.top, bx.s, accel == "Right falls", 1.0);
            }
        }
        _ => {
            // Atwood machine
            g.hatch(130.0, 350.0, 30.0, false);
            let (cx, cy, r) = (240.0, 105.0, 42.0);
            g.line(cx, 30.0, cx, cy - r, INK, SW);
            wheel(&mut g, cx, cy, r);
            let third = p.b("third");
            let tside = if p.s("third_side") == "Left" { 0 } else { 1 };
            let tl = |k: usize| if third { format!("{}_1", st.tens.split('_').next().unwrap_or("T")) } else { let _ = k; st.tens.clone() };
            let ts = sz(p, "t_size");
            for (k, b) in geo.boxes.iter().enumerate() {
                let side = if k == 0 { -1.0 } else { 1.0 };
                let x = b.cx;
                g.line(x, cy, x, b.top, INK, 2.0);
                mass_box(&mut g, x, b.top, b.s, if k == 0 { &llab } else { &rlab });
                let lab = if k == 0 { &llab } else { &rlab };
                let below = third && k == tside;
                if ten {
                    let tlab = if st.tens.is_empty() || st.tens == "none" { st.tens.clone() } else { tl(k) };
                    t_arrow(&mut g, &st, (x, cy + 8.0), (0.0, 1.0), &tlab, side);
                    t_arrow(&mut g, &st, (x, b.top), (0.0, -1.0), &tlab, side);
                }
                if grav {
                    if below {
                        // beside the string to the object below, label to the outside
                        let wx = x + side * 12.0;
                        g.arrow(wx, b.top + b.s, wx, b.top + b.s + 44.0, st.gc(), SW);
                        lab_at(&mut g, wx + side * 8.0, b.top + b.s + 40.0, &weight_lab(p, lab), if side > 0.0 { "start" } else { "end" });
                    } else {
                        w_arrow(&mut g, &st, x, b.top + b.s, &weight_lab(p, lab));
                    }
                }
                let down = (side > 0.0) == (accel == "Right falls");
                if accel != "None" { a_arrow(&mut g, &st, x + side * (b.s / 2.0 + 14.0), b.top, b.s, down, side); }
                if below {
                    let t3 = b.top + b.s + if grav || ten { 96.0 } else { 34.0 };
                    g.line(x, b.top + b.s, x, t3, INK, 2.0);
                    mass_box(&mut g, x, t3, ts, &tlab);
                    if ten {
                        let l2 = if st.tens.is_empty() || st.tens == "none" { st.tens.clone() } else { format!("{}_2", st.tens.split('_').next().unwrap_or("T")) };
                        t_arrow(&mut g, &st, (x - 2.0, b.top + b.s), (0.0, 1.0), &l2, -side);
                        t_arrow(&mut g, &st, (x, t3), (0.0, -1.0), &l2, -side);
                    }
                    if grav { w_arrow(&mut g, &st, x, t3 + ts, &weight_lab(p, &tlab)); }
                    if accel != "None" { a_arrow(&mut g, &st, x + side * (ts / 2.0 + 14.0), t3, ts, down, side); }
                }
            }
        }
    }
    g.finish()
}

/// Table and ramp setups share one drawing: an object on a surface tied over a pulley at the
/// surface's far end to a hanging mass.
fn slope_setup(g: &mut G, p: &Params, geo: &PGeo, st: &Style, llab: &str, rlab: &str, pd: &PairDraw) {
    let ramp = p.s("setup") == "Ramp";
    let sl = slope(p);
    let (uu, nn) = (sl.uu, sl.nn);
    let ls = sz(p, "l_size");
    let cart = p.s("l_kind") == "Cart";
    let accel = p.s("accel").to_string();
    // the surface
    if ramp {
        g.poly(&[(sl.ax, sl.ground), (sl.apex.0, sl.ground), sl.apex], "#f3f4f7", INK, SW);
        g.hatch(10.0, geo.w - 10.0, sl.ground, true);
        let th = p.f("angle").clamp(15.0, 60.0);
        g.polyline(&arc_pts(sl.ax, sl.ground, 46.0, 0.0, th), INK, 1.4);
        let m = pol(sl.ax, sl.ground, 64.0, th / 2.0);
        g.label_at(m.0, m.1, 22.0, &lab_mode(p, "ang_lab", "ang_lab_mode"), ITALIC);
    } else {
        let ground = geo.h - 24.0;
        g.hatch(10.0, geo.w - 10.0, ground, true);
        g.rect(sl.ax, sl.ground, sl.apex.0 - sl.ax, 12.0, "#f3f4f7", INK, SW);
        for x in [sl.ax + 14.0, sl.apex.0 - 26.0] { g.rect(x, sl.ground + 12.0, 12.0, ground - sl.ground - 12.0, "#f3f4f7", INK, 1.8); }
    }
    let ramp_len = slope_len(&sl);
    let touch = touch(p);
    if p.s("surface") == "Rough" {
        let mut s = 8.0;
        while s < ramp_len - 4.0 {
            let (qx, qy) = (sl.ax + uu.0 * s, sl.ground + uu.1 * s);
            g.line(qx, qy, qx - nn.0 * 7.0 - uu.0 * 5.0, qy - nn.1 * 7.0 - uu.1 * 5.0, INK, 1.2);
            s += 11.0;
        }
    }
    // the object on the surface (block or cart), centred at `s0` along it
    // returns (centre, centre-to-top, centre-to-bottom, half length)
    // objects touching: each label sized to its own object, so the two never run into each other (the old 22 px
    // otherwise)
    let fit_obj = touch.is_some();
    let obj = |g: &mut G, s0: f64, lab: &str| -> (Pt, f64, f64, f64) {
        let base = (sl.ax + uu.0 * s0, sl.ground + uu.1 * s0);
        let loc = |l: f64, h: f64| (base.0 + uu.0 * l + nn.0 * h, base.1 + uu.1 * l + nn.1 * h);
        let (com, up, dn, hl) = if cart {
            let (len, h, r) = (ls * 1.4, ls * 0.5, ls * 0.13);
            g.poly(&[loc(-len / 2.0, 2.0 * r), loc(len / 2.0, 2.0 * r), loc(len / 2.0, 2.0 * r + h), loc(-len / 2.0, 2.0 * r + h)], FILL, INK, SW);
            for l in [-len * 0.3, len * 0.3] { let c = loc(l, r); g.circle(c.0, c.1, r, "#fff", INK, SW); }
            (loc(0.0, 2.0 * r + h / 2.0), h / 2.0, 2.0 * r + h / 2.0, len / 2.0)
        } else {
            g.poly(&[loc(-ls / 2.0, 0.0), loc(ls / 2.0, 0.0), loc(ls / 2.0, ls), loc(-ls / 2.0, ls)], FILL, INK, SW);
            (loc(0.0, ls / 2.0), ls / 2.0, ls / 2.0, ls / 2.0)
        };
        if fit_obj {
            let fs = super::pulley_b10::fit_size(lab, 22.0, 2.0 * hl - 8.0, up - 3.75, uu.1.abs());
            if lab.trim().is_empty() { g.line(com.0 - 11.0, com.1 + 6.0, com.0 + 11.0, com.1 + 6.0, INK, 1.4); }
            else if lab != "none" {
                // the label's box centred on the object (`label_at` puts its baseline 0.35 size below the point given)
                let (_, a, b) = super::pulley_b10::tex_box(lab, fs);
                g.label_at(com.0, com.1 - (b - a) / 2.0, fs, lab, ITALIC);
            }
        } else if lab.trim().is_empty() { g.line(com.0 - 11.0, com.1 + 6.0, com.0 + 11.0, com.1 + 6.0, INK, 1.4); }
        else if lab != "none" { g.tex(com.0, com.1 + 7.0, 22.0, "middle", lab, ITALIC); }
        (com, up, dn, hl)
    };
    let s_main = match &touch { Some(t) => t.s1, None => ramp_len * if ramp { 0.5 } else { 0.55 } };
    let (com, up, dn, hl) = obj(g, s_main, llab);
    let (wc, pr) = slope_wheel(&sl, touch.as_ref().map(|t| t.h).unwrap_or_else(|| mid_h(p, ls)));
    // a point `l` along the surface and `h` above it
    let at = |l: f64, h: f64| (sl.ax + uu.0 * l + nn.0 * h, sl.ground + uu.1 * l + nn.1 * h);
    // string from the object's front face (at its mid height) to the top of the wheel; objects touching: from a post
    // on the first object, behind the contact pair, over the second (which sits in front, between it and the pulley)
    let sa = match &touch {
        Some(t) => {
            let i = t.s1 + t.hl;
            obj(g, i + t.hl, p.s("s_lab"));
            let post = pair_geo(t).post;
            let (foot, knot) = (at(post, t.top), at(post, t.h));
            g.line(foot.0, foot.1, knot.0, knot.1, INK, 2.4);
            knot
        }
        None => (com.0 + uu.0 * hl, com.1 + uu.1 * hl),
    };
    let tan_pt = (wc.0 + nn.0 * pr, wc.1 + nn.1 * pr);
    g.line(sa.0, sa.1, tan_pt.0, tan_pt.1, INK, 2.0);
    if touch.is_some() { g.circle(sa.0, sa.1, 2.6, INK, INK, 1.0); }
    // bracket from the surface corner to the wheel
    g.line(sl.apex.0, sl.apex.1 + if ramp { 0.0 } else { 6.0 }, wc.0, wc.1, INK, SW);
    let hb = &geo.boxes[0];
    g.line(hb.cx, wc.1, hb.cx, hb.top, INK, 2.0);
    wheel(g, wc.0, wc.1, pr);
    mass_box(g, hb.cx, hb.top, hb.s, rlab);
    // second object behind, tied by a string
    if p.b("second") && !super::pulley_b10::asked(p) {
        let gap = hl * 2.0 + 34.0;
        let (c2, _, _, hl2) = obj(g, s_main - gap, p.s("s_lab"));
        let a = (c2.0 + uu.0 * hl2, c2.1 + uu.1 * hl2);
        let b = (com.0 - uu.0 * hl, com.1 - uu.1 * hl);
        g.line(a.0, a.1, b.0, b.1, INK, 2.0);
    }
    let falls = accel == "Right falls";
    if p.b("tension") {
        // both labels above the string (on the table this is where they always were; on the ramp
        // the upper one used to land inside the wheel)
        t_arrow_along_off(g, st, sa, uu, &st.tens, nn, if touch.is_some() { 18.0 } else { 14.0 });
        t_arrow_along(g, st, (tan_pt.0 - uu.0 * 4.0, tan_pt.1 - uu.1 * 4.0), (-uu.0, -uu.1), &st.tens, nn);
        if hb.top - (wc.1 + pr) > 76.0 { t_arrow(g, st, (hb.cx, wc.1 + pr + 4.0), (0.0, 1.0), &st.tens, 1.0); }
        t_arrow(g, st, (hb.cx, hb.top), (0.0, -1.0), &st.tens, 1.0);
    }
    if p.b("gravity") {
        // weight of the object on the surface from its bottom face, straight down
        let from = (com.0 - nn.0 * dn, com.1 - nn.1 * dn);
        g.arrow(from.0, from.1, from.0, from.1 + 50.0, st.gc(), SW);
        lab_at(g, from.0, from.1 + 72.0, &weight_lab(p, llab), "middle");
        w_arrow(g, st, hb.cx, hb.top + hb.s, &weight_lab(p, rlab));
    }
    // the pair of contact forces where the objects touch, just above them under the string, as the inclined plane
    // draws it: from where they meet, on the second (pushed toward the pulley by the first) and on the first (pushed
    // back), equal and opposite, with one label for the pair at the force labels' 22 px, placed where it reads as the
    // pair's (`contact_label`), by a leader if it has to sit further off
    let n_at = touch.as_ref().map(|t| pair_geo(t).n_at).unwrap_or(s_main);
    if let (Some(t), true, PairDraw::Draw(spot)) = (touch.as_ref(), p.b("contact"), pd) {
        let col = if st.col { PAL[5] } else { INK };
        for (a, b) in pair_arrows(p, t) { g.arrow(a.0, a.1, b.0, b.1, col, SW); }
        if let Some(sp) = spot {
            // the spot is in page coordinates as drawn; `G` mirrors again
            let un = |q: Pt| if p.b("mirror") { (geo.w - q.0, q.1) } else { q };
            if let Some((a, b)) = sp.leader { let (a, b) = (un(a), un(b)); g.line(a.0, a.1, b.0, b.1, col, 1.2); }
            let c = un(sp.c);
            g.label_at(c.0, c.1, 22.0, &lab_mode(p, "cont_lab", "cont_lab_mode"), ITALIC);
        }
    }
    if p.b("normal") {
        let c = if touch.is_some() { at(n_at, dn) } else { com };
        let from = (c.0 + nn.0 * up, c.1 + nn.1 * up);
        let tip = (from.0 + nn.0 * 46.0, from.1 + nn.1 * 46.0);
        g.arrow(from.0, from.1, tip.0, tip.1, if st.col { PAL[3] } else { INK }, SW);
        // objects touching: the label goes up and back from the tip, away from the post just in front of it
        let d = if touch.is_some() { norm_lab_dir(uu, nn) } else { nn };
        g.label_dir(tip.0, tip.1, d.0, d.1, 22.0, &lab_mode(p, "norm_lab", "norm_lab_mode"), ITALIC);
    }
    let fr = p.s("friction");
    if fr != "None" {
        let d = if fr == "Toward the pulley" { uu } else { (-uu.0, -uu.1) };
        let sgn = if fr == "Toward the pulley" { 1.0 } else { -1.0 };
        let from = (com.0 + uu.0 * hl * sgn - nn.0 * dn * 0.6, com.1 + uu.1 * hl * sgn - nn.1 * dn * 0.6);
        let tip = (from.0 + d.0 * 40.0, from.1 + d.1 * 40.0);
        if let Some(t) = &touch {
            // the second object sits in front, so friction is drawn behind the first: from its back face (away from
            // the pulley) or pushing on it (toward), low down, its label above the arrow's middle
            let hgt = dn * 0.4;
            let (a, b) = if sgn > 0.0 { (t.s1 - t.hl - 44.0, t.s1 - t.hl) } else { (t.s1 - t.hl, t.s1 - t.hl - 44.0) };
            let (q0, q1) = (at(a, hgt), at(b, hgt));
            g.arrow(q0.0, q0.1, q1.0, q1.1, if st.col { PAL[4] } else { INK }, SW);
            let lab = lab_mode(p, "fric_lab", "fric_lab_mode");
            let (w, up, down) = super::pulley_b10::tex_box(&lab, 22.0);
            let en = w / 2.0 * uu.1.abs() + up.max(down) * uu.0.abs();
            let m = at(t.s1 - t.hl - 26.0, hgt + 6.2 + 3.0 + en);
            g.label_at(m.0, m.1, 22.0, &lab, ITALIC);
        } else {
            g.arrow(from.0, from.1, tip.0, tip.1, if st.col { PAL[4] } else { INK }, SW);
            g.label_dir(tip.0, tip.1, d.0, d.1, 22.0, &lab_mode(p, "fric_lab", "fric_lab_mode"), ITALIC);
        }
    }
    if accel != "None" {
        // on the surface object (above it, along the surface) and beside the hanging mass
        let d = if falls { uu } else { (-uu.0, -uu.1) };
        let c = match &touch {
            // above the string, over the second object and clear of the tension label at the post
            Some(t) => at((t.s1 + 2.0 * t.hl + 0.3 * t.hl + 10.0).max(t.s1 + t.hl + 50.0), t.h + 34.0),
            None => (com.0 + nn.0 * (up + 30.0) + uu.0 * (hl * 0.6 + 10.0), com.1 + nn.1 * (up + 30.0) + uu.1 * (hl * 0.6 + 10.0)),
        };
        let (a, b) = ((c.0 - d.0 * 18.0, c.1 - d.1 * 18.0), (c.0 + d.0 * 18.0, c.1 + d.1 * 18.0));
        g.dash_arrow(a.0, a.1, b.0, b.1, st.ac(), 2.2);
        g.label_dir(c.0 + nn.0 * 4.0, c.1 + nn.1 * 4.0, nn.0, nn.1, 22.0, &st.acc, ITALIC);
        a_arrow(g, st, hb.cx + hb.s / 2.0 + 14.0, hb.top, hb.s, falls, 1.0);
    }
}
