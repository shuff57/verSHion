//! Tests of the VSEPR shapes. The science tests recompute every bond angle from the coordinates in
//! the module; the drawing tests read the SVG back, check the depth order and the angle arcs, and
//! measure every text with the real glyphs the PNG renderer draws.
use super::*;
use crate::figures::chemistry::reactionenergy::{hits, seg_hits};
use crate::figures::{registry, svg_size, to_png, FigureDef};
use std::collections::{BTreeMap, BTreeSet, HashMap};

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).expect("vsepr-shapes is registered") }
fn with(list: &[(&'static str, Value)]) -> Params {
    let d = def();
    let mut p = Params::defaults(&(d.params)());
    for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&(d.params)(), false);
    p
}
/// Never normalised: render must survive what a hostile link or an old file holds.
fn raw(list: &[(&'static str, Value)]) -> Params {
    let d = def();
    let mut p = Params::defaults(&(d.params)());
    for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
    p
}
fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }
fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
fn i(k: &'static str, v: i64) -> (&'static str, Value) { (k, Value::I(v)) }
fn svg_of(list: &[(&'static str, Value)]) -> String { def().svg(&with(list)) }
fn build_of(list: &[(&'static str, Value)]) -> (String, Report) { build(&with(list)) }
fn key(list: &[(&'static str, Value)]) -> Report { let mut l = list.to_vec(); l.push(b(SHOW_ANSWERS, true)); build_of(&l).1 }
fn student(list: &[(&'static str, Value)]) -> Report { let mut l = list.to_vec(); l.push(b(SHOW_ANSWERS, false)); build_of(&l).1 }
fn notes_of(list: &[(&'static str, Value)]) -> Vec<String> { warnings(&with(list)) }
fn has(notes: &[String], what: &str) -> bool { notes.iter().any(|n| n.contains(what)) }
fn rel(m: &Mol, aturn: f64, atilt: f64) -> (f64, f64) {
    ((aturn - m.open.0).rem_euclid(360.0), atilt - m.open.1)
}
fn close(a: f64, b: f64, rel: f64) -> bool { (a - b).abs() <= rel * a.abs().max(b.abs()).max(1.0) }
fn bad_number_word(s: &str) -> bool {
    s.split(|c: char| !c.is_alphanumeric()).any(|w| matches!(w, "NaN" | "inf" | "Infinity"))
}

fn ax_counts(ax: &str) -> (usize, usize) {
    let s = ax.strip_prefix("AX").unwrap_or(ax);
    match s.split_once('E') {
        Some((n, e)) => (n.parse().unwrap_or(0), e.parse().unwrap_or(1)),
        None => (s.parse().unwrap_or(0), 0),
    }
}

// ---------------------------------------------------------------------------------------------
// Independent geometry
// ---------------------------------------------------------------------------------------------

fn vlen(v: [f64; 3]) -> f64 { (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() }
fn vdot(a: [f64; 3], b: [f64; 3]) -> f64 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn vsub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn vang(a: [f64; 3], b: [f64; 3]) -> f64 { (vdot(a, b) / (vlen(a) * vlen(b)).max(1e-300)).clamp(-1.0, 1.0).acos().to_degrees() }

fn screen_angle(a: Pt, b: Pt) -> f64 { (b.1 - a.1).atan2(b.0 - a.0).to_degrees().rem_euclid(360.0) }
fn ang_diff(a: f64, b: f64) -> f64 { let d = (a - b).rem_euclid(360.0); d.min(360.0 - d) }
fn seg_dist(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    if l2 < 1e-12 { return dist(p, a); }
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0);
    dist(p, (a.0 + t * dx, a.1 + t * dy))
}

// ---------------------------------------------------------------------------------------------
// Reading the SVG back
// ---------------------------------------------------------------------------------------------

fn attr(tag: &str, k: &str) -> Option<String> {
    let pat = format!(" {k}=\"");
    let at = tag.find(&pat)? + pat.len();
    tag[at..].split('"').next().map(|v| v.to_string())
}
fn attr_f(tag: &str, k: &str) -> Option<f64> { attr(tag, k)?.parse().ok() }
fn unesc(s: &str) -> String { s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&amp;", "&") }

#[derive(Clone, Debug)]
struct T {
    x: f64,
    y: f64,
    fs: f64,
    anchor: String,
    text: String,
    kind: String,
    feat: String,
    fill: String,
}

fn texts(svg: &str) -> Vec<T> {
    let mut out = Vec::new();
    for part in svg.split("<text").skip(1) {
        let Some((tag, rest)) = part.split_once('>') else { continue };
        let text = unesc(rest.split("</text>").next().unwrap_or(""));
        out.push(T {
            x: attr_f(tag, "x").unwrap_or(0.0),
            y: attr_f(tag, "y").unwrap_or(0.0),
            fs: attr_f(tag, "font-size").unwrap_or(12.0),
            anchor: attr(tag, "text-anchor").unwrap_or_else(|| "start".into()),
            text,
            kind: attr(tag, "data-o").unwrap_or_default(),
            feat: attr(tag, "data-f").unwrap_or_default(),
            fill: attr(tag, "fill").unwrap_or_default(),
        });
    }
    out
}

#[derive(Clone, Debug)]
struct Line { kind: String, feat: String, depth: f64, a: Pt, b: Pt, stroke: String, width: f64 }
#[derive(Clone, Debug)]
struct Circ { kind: String, feat: String, depth: f64, c: Pt, r: f64, fill: String }
#[derive(Clone, Debug)]
struct Arc { kind: String, feat: String, pts: Vec<Pt> }

struct Geo { lines: Vec<Line>, circles: Vec<Circ>, arcs: Vec<Arc> }

fn pts_attr(s: &str) -> Vec<Pt> {
    s.split_whitespace()
        .filter_map(|tok| {
            let (a, b) = tok.split_once(',')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        })
        .collect()
}

fn geo(svg: &str) -> Geo {
    let mut g = Geo { lines: Vec::new(), circles: Vec::new(), arcs: Vec::new() };
    for part in svg.split('<').skip(1) {
        let tag = part.split('>').next().unwrap_or("");
        let kind = attr(tag, "data-o").unwrap_or_default();
        let feat = attr(tag, "data-f").unwrap_or_default();
        let depth = attr_f(tag, "data-d").unwrap_or(f64::NAN);
        if tag.starts_with("line ") {
            g.lines.push(Line {
                kind,
                feat,
                depth,
                a: (attr_f(tag, "x1").unwrap_or(0.0), attr_f(tag, "y1").unwrap_or(0.0)),
                b: (attr_f(tag, "x2").unwrap_or(0.0), attr_f(tag, "y2").unwrap_or(0.0)),
                stroke: attr(tag, "stroke").unwrap_or_default(),
                width: attr_f(tag, "stroke-width").unwrap_or(0.0),
            });
        } else if tag.starts_with("circle ") {
            g.circles.push(Circ {
                kind,
                feat,
                depth,
                c: (attr_f(tag, "cx").unwrap_or(0.0), attr_f(tag, "cy").unwrap_or(0.0)),
                r: attr_f(tag, "r").unwrap_or(0.0),
                fill: attr(tag, "fill").unwrap_or_default(),
            });
        } else if tag.starts_with("polyline ") {
            g.arcs.push(Arc { kind, feat, pts: pts_attr(&attr(tag, "points").unwrap_or_default()) });
        }
    }
    g
}

fn grey_hex(v: &str) -> Option<u8> {
    let h = v.strip_prefix('#')?;
    let h: String = if h.len() == 3 { h.chars().flat_map(|c| [c, c]).collect() } else { h.to_string() };
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) { return None; }
    let c = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    let (r, g, b) = (c(0)?, c(2)?, c(4)?);
    (r == g && g == b).then_some(r)
}
fn paint_is_grey(v: &str) -> bool { v == "none" || v.starts_with("url(") || grey_hex(v).is_some() }

fn box_hits_disc(b: Bx, c: Pt, r: f64) -> bool {
    let nx = c.0.clamp(b.0, b.1);
    let ny = c.1.clamp(b.2, b.3);
    (nx - c.0).powi(2) + (ny - c.1).powi(2) < r * r
}

// ---------------------------------------------------------------------------------------------
// Registration, table and controls
// ---------------------------------------------------------------------------------------------

#[test]
fn it_is_registered_with_a_faq_examples_and_its_controls() {
    let d = def();
    assert_eq!(d.id, "vsepr-shapes");
    assert_eq!(d.subject, Subject::Chemistry);
    assert!(d.title.contains("VSEPR"));
    let faq = crate::figures::faq::faq_for(ID);
    assert!((4..=6).contains(&faq.len()), "{} entries", faq.len());
    let keys: Vec<&str> = (d.params)().iter().map(|p| p.key).collect();
    for k in ["molecule", "lone", "angles", "labels", "turn", "tilt", SHOW_ANSWERS, "style", "label_size"] {
        assert!(keys.contains(&k), "no {k} param");
    }
    assert!(crate::figures::chemistry::skin_intro(ID).is_some());
    assert!(!crate::figures::notes_for(ID, &raw(&[s("molecule", "Xx")])).is_empty());
    let names: Vec<&str> = crate::figures::examples_for(ID).iter().map(|e| e.name).collect();
    assert!(names.len() >= 3, "{names:?}");
    assert!(names.iter().any(|n| n.contains("Methane")), "{names:?}");
    for e in crate::figures::examples_for(ID) {
        let p = with(&e.params);
        assert!(warnings(&p).is_empty(), "{}: {:?}", e.name, warnings(&p));
        let r = build(&p).1;
        for a in r.atoms.iter().filter(|a| !a.hidden) {
            assert!(a.labelled, "{}: atom {} is unlabelled at the example view", e.name, a.index);
        }
        for ans in [true, false] {
            let mut q = p.clone();
            q.set_b(SHOW_ANSWERS, ans);
            assert!(warnings(&q).is_empty(), "{}: {:?}", e.name, warnings(&q));
            let svg = def().svg(&q);
            assert!(svg.contains("<svg") && svg.len() > 1500, "{}", e.name);
        }
    }
}

#[test]
fn the_molecule_table_has_the_stated_atoms_lone_pairs_and_angles() {
    assert_eq!(molecules().len(), NAMES.len());
    let want = [
        ("BeCl2", 3usize, 0usize, "Linear", "Linear", 2usize),
        ("BF3", 4, 0, "Trigonal planar", "Trigonal planar", 3),
        ("CH4", 5, 0, "Tetrahedral", "Tetrahedral", 4),
        ("NH3", 4, 1, "Tetrahedral", "Trigonal pyramidal", 4),
        ("H2O", 3, 2, "Tetrahedral", "Bent", 4),
        ("PCl5", 6, 0, "Trigonal bipyramidal", "Trigonal bipyramidal", 5),
        ("SF6", 7, 0, "Octahedral", "Octahedral", 6),
        ("XeF4", 5, 2, "Octahedral", "Square planar", 6),
        ("SF4", 5, 1, "Trigonal bipyramidal", "See-saw", 5),
    ];
    let opens = [(0.0, 0.0), (0.0, 0.0), (202.0, 21.0), (0.0, 27.0), (44.0, 62.0), (250.0, 0.0), (0.0, 20.0), (0.0, 0.0), (296.0, 36.0)];
    for ((m, (name, atoms, lone, domain, shape, domains)), open) in molecules().iter().zip(want).zip(opens) {
        assert_eq!(m.name, name);
        assert_eq!(m.open, open, "{name}: opening view");
        assert_eq!(m.display, m.name, "{name}: the panel and the choice use the same formula");
        assert_eq!(m.atoms.len(), atoms, "{name}");
        assert_eq!(m.lone.len(), lone, "{name}");
        assert_eq!(m.domains, domains, "{name}");
        assert_eq!(m.domain_geo, domain, "{name}");
        assert_eq!(m.shape, shape, "{name}");
        assert!(!m.display.is_empty() && !m.ax.is_empty() && !m.why.is_empty(), "{name}");
        let (n, e) = ax_counts(m.ax);
        assert_eq!(n, atoms - 1, "{name}: {} against {}", m.ax, atoms - 1);
        assert_eq!(e, lone, "{name}: {} against {}", m.ax, lone);
        assert_eq!(m.domains, (atoms - 1) + lone, "{name}");
        assert!(m.atoms[0].central, "{name}");
        assert_eq!(m.atoms[0].pos, [0.0, 0.0, 0.0], "{name}: the centre is the origin");
        assert_eq!(m.atoms.iter().filter(|a| a.central).count(), 1, "{name}");
        for a in &m.atoms[1..] {
            assert!(!a.central, "{name}");
            assert!(a.sym.len() >= 1 && a.sym.chars().all(|c| c.is_ascii_alphabetic()), "{name}: {:?}", a.sym);
            assert!((vlen(a.pos) - 1.0).abs() < 1e-9, "{name}: a bond is not one bond length: {:?}", a.pos);
            assert!(vlen(a.pos) > 0.1, "{name}");
        }
        for (j, a) in m.atoms[1..].iter().enumerate() {
            for b in &m.atoms[j + 2..] {
                assert!(vlen(vsub(a.pos, b.pos)) > 0.1, "{name}: two atoms share a position");
            }
        }
        for d in m.lone {
            assert!((vlen(*d) - 1.0).abs() < 1e-9, "{name}: a lone pair is not a unit direction");
        }
        for (j, d) in m.lone.iter().enumerate() {
            for e in &m.lone[j + 1..] {
                assert!(vlen(vsub(*d, *e)) > 0.1, "{name}: two lone pairs share a direction");
            }
        }
        assert!(!m.pairs.is_empty(), "{name}: no angle pair");
        for p in m.pairs {
            assert!(p.a >= 1 && p.a < m.atoms.len() && p.b >= 1 && p.b < m.atoms.len(), "{name}");
            assert_ne!(p.a, p.b);
            assert!(p.text.ends_with('°'), "{name}: {:?}", p.text);
        }
        if lone == 0 { assert_eq!(m.shape, m.domain_geo, "{name}: no lone pair, so the shape is the domain geometry"); }
        let angles: Vec<&str> = m.pairs.iter().map(|p| p.text).collect();
        let joined = angles.join(" and ");
        assert_eq!(m.angle_text(), joined, "{name}");
    }
}

#[test]
fn every_bond_angle_matches_the_coordinates_within_one_degree() {
    let want: [(&str, &[f64]); 9] = [
        ("BeCl2", &[180.0]),
        ("BF3", &[120.0]),
        ("CH4", &[109.5]),
        ("NH3", &[107.0]),
        ("H2O", &[104.5]),
        ("PCl5", &[120.0, 90.0]),
        ("SF6", &[90.0]),
        ("XeF4", &[90.0]),
        ("SF4", &[102.0, 90.0]),
    ];
    let mut cases = 0;
    for (m, (name, angles)) in molecules().iter().zip(want) {
        assert_eq!(m.name, name);
        assert_eq!(m.pairs.len(), angles.len(), "{name}");
        for (p, want) in m.pairs.iter().zip(angles) {
            let got = angle_between(m.atoms[p.a].pos, m.atoms[p.b].pos);
            assert!((got - *want).abs() <= 1.0, "{name}: the coordinates give {got}, the table says {want}");
            assert!((got - p.angle).abs() <= 1.0, "{name}: the pair says {} and the coordinates {got}", p.angle);
            assert!((p.angle - *want).abs() < 1e-9, "{name}: the stated angle is {} not {want}", p.angle);
            cases += 1;
        }
    }
    assert_eq!(cases, 11);
    // the exact tetrahedral angle from the coordinates
    let ch4 = &molecules()[2];
    let exact = angle_between(ch4.atoms[1].pos, ch4.atoms[2].pos);
    assert!((exact - (-1.0f64 / 3.0).acos().to_degrees()).abs() < 1e-9, "{exact}");
    assert!((exact - 109.471_220_634_490_69).abs() < 1e-6, "{exact}");
}

#[test]
fn the_lone_pair_compressions_are_drawn_not_the_ideal_tetrahedral_angle() {
    let ch4 = &molecules()[2];
    for a in 1..ch4.atoms.len() {
        for b in a + 1..ch4.atoms.len() {
            let angle = angle_between(ch4.atoms[a].pos, ch4.atoms[b].pos);
            assert!((angle - 109.471_220_634_490_69).abs() < 1e-6, "CH4 {angle}");
            assert!((vdot(ch4.atoms[a].pos, ch4.atoms[b].pos) + 1.0 / 3.0).abs() < 1e-9);
        }
    }
    let nh3 = &molecules()[3];
    let hh = angle_between(nh3.atoms[1].pos, nh3.atoms[2].pos);
    assert!((hh - 107.0).abs() < 1e-6, "NH3 H-N-H {hh}");
    let lp_h = angle_between(nh3.lone[0], nh3.atoms[1].pos);
    assert!(lp_h > 109.471_220_634_490_69, "NH3 lone pair {lp_h} is not farther than a tetrahedral bond");
    assert!((lp_h - 111.8).abs() < 0.2, "NH3 lone pair {lp_h}");
    let h2o = &molecules()[4];
    let hh = angle_between(h2o.atoms[1].pos, h2o.atoms[2].pos);
    assert!((hh - 104.5).abs() < 1e-6, "H2O H-O-H {hh}");
    assert!((hh - 109.5).abs() > 4.0, "H2O claims a tetrahedral angle: {hh}");
    let lp_lp = angle_between(h2o.lone[0], h2o.lone[1]);
    assert!((lp_lp - 115.0).abs() < 1e-6, "H2O lone pairs {lp_lp}");
    assert!(lp_lp > 109.471_220_634_490_69, "H2O lone pairs are not spread more than a tetrahedron");
    let lp_h = angle_between(h2o.lone[0], h2o.atoms[1].pos);
    assert!((108.0..=110.0).contains(&lp_h), "H2O lone pair to bond {lp_h}");
    // water's key must not claim 109.5 anywhere
    for ans in [true, false] {
        let svg = svg_of(&[s("molecule", "H2O"), b(SHOW_ANSWERS, ans)]);
        for t in texts(&svg) { assert!(!t.text.contains("109.5"), "water's text {:?}", t.text); }
        assert!(!svg.contains("109.5°"), "water's SVG claims 109.5°");
    }
    // XeF4: the lone pairs are axial, 90° from every bond and 180° from each other
    let xe = &molecules()[7];
    assert!((angle_between(xe.lone[0], xe.lone[1]) - 180.0).abs() < 1e-9);
    for a in &xe.atoms[1..] {
        for l in xe.lone { assert!((angle_between(*l, a.pos) - 90.0).abs() < 1e-9); }
    }
    // SF4: see-saw, the equatorial pair 102°, the axial bonds 90° from them
    let sf = &molecules()[8];
    assert!((angle_between(sf.atoms[1].pos, sf.atoms[2].pos) - 102.0).abs() < 1e-9);
    for a in [1usize, 2] {
        for b in [3usize, 4] { assert!((angle_between(sf.atoms[a].pos, sf.atoms[b].pos) - 90.0).abs() < 1e-9); }
    }
}

#[test]
fn one_bond_per_terminal_atom_and_one_central_atom() {
    for m in molecules() {
        let n = m.atoms.len() - 1;
        let svg = svg_of(&[s("molecule", m.name)]);
        let g = geo(&svg);
        let bonds: Vec<&Line> = g.lines.iter().filter(|l| l.kind == "bond").collect();
        let feats: BTreeSet<String> = bonds.iter().map(|l| l.feat.clone()).collect();
        assert_eq!(feats.len(), n, "{}: {n} bonds", m.name);
        let want: BTreeSet<String> = (1..=n).map(|k| format!("b{k}")).collect();
        assert_eq!(feats, want, "{}", m.name);
        let atoms: Vec<&Circ> = g.circles.iter().filter(|c| c.kind == "atom").collect();
        assert_eq!(atoms.len(), m.atoms.len(), "{}", m.name);
        let afeats: BTreeSet<String> = atoms.iter().map(|c| c.feat.clone()).collect();
        let awant: BTreeSet<String> = (0..m.atoms.len()).map(|k| format!("a{k}")).collect();
        assert_eq!(afeats, awant, "{}", m.name);
        // atoms are in depth order, far first; the centre is wherever its depth puts it
        let mut prev = f64::NEG_INFINITY;
        for c in &atoms {
            assert!(c.depth >= prev - 1e-9, "{}: atom depth order", m.name);
            prev = c.depth;
        }
        let r = key(&[s("molecule", m.name)]);
        let centre = r.atoms[0].p;
        let at_centre = r.atoms.iter().filter(|a| dist(a.p, centre) < 0.5).count();
        assert_eq!(at_centre, 1, "{}: {at_centre} atoms at the centre", m.name);
        assert_eq!(r.atoms.iter().filter(|a| a.hidden).count(), 0, "{}: a ligand is hidden at the default view", m.name);
        assert!(r.bonds.iter().all(|b| b.drawn), "{}: a bond is not drawn at the default view", m.name);
    }
    // Looking straight down a bond: the ligand on the axis is not drawn a second time at the centre,
    // and the central atom is the one left there.
    for (name, aturn, atilt) in [("SF6", 90.0, 0.0), ("BeCl2", 90.0, 0.0), ("XeF4", 90.0, 0.0)] {
        let m = molecules().iter().find(|m| m.name == name).unwrap();
        let (turn, tilt) = rel(m, aturn, atilt);
        let r = key(&[s("molecule", name), f("turn", turn), f("tilt", tilt)]);
        let centre = r.atoms[0].p;
        assert_eq!(r.atoms.iter().filter(|a| !a.hidden && dist(a.p, centre) < 0.5).count(), 1, "{name}: more than one atom at the centre");
        assert!(r.atoms.iter().skip(1).any(|a| a.hidden), "{name}: no ligand is hidden at the edge-on view");
        let g = geo(&svg_of(&[s("molecule", name), f("turn", turn), f("tilt", tilt)]));
        let at: Vec<&Circ> = g.circles.iter().filter(|c| c.kind == "atom" && dist(c.c, centre) < 0.5).collect();
        assert_eq!(at.len(), 1, "{name}: {} atom circles at the centre", at.len());
        assert_eq!(at[0].feat, "a0", "{name}: a ligand ball covers the central atom");
    }
    // A near substituent is drawn after the centre and keeps its label and its bond.
    let r = key(&[s("molecule", "CH4"), f("turn", 35.0), f("tilt", -30.0)]);
    let centre = r.atoms[0].p;
    let near = r.atoms.iter().skip(1).max_by(|a, b| a.depth.total_cmp(&b.depth)).unwrap();
    assert!(near.depth > r.atoms[0].depth, "the nearest H is not in front of the centre");
    assert!(near.labelled, "the nearest H lost its label");
    assert!(r.atoms[0].labelled, "the centre lost its label when a near substituent was in front");
    assert!(r.bonds.iter().all(|b| b.drawn), "a bond was dropped at a near substituent");
    let g = geo(&svg_of(&[s("molecule", "CH4"), f("turn", 35.0), f("tilt", -30.0)]));
    let atom_circles: Vec<&Circ> = g.circles.iter().filter(|c| c.kind == "atom").collect();
    assert_eq!(atom_circles.last().map(|c| c.feat.as_str()), Some(format!("a{}", near.index).as_str()), "the nearest H is not drawn last");
    assert!(atom_circles.iter().all(|c| c.feat != format!("a{}", near.index) || c.depth >= 0.0));
    let _ = centre;
}

// ---------------------------------------------------------------------------------------------
// The projection
// ---------------------------------------------------------------------------------------------

#[test]
fn the_projection_matrix_is_orthonormal_and_orthographic() {
    let id = view_matrix(0.0, 0.0);
    assert_eq!(id, [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    let q = view_matrix(90.0, 0.0);
    let x = apply(&q, [1.0, 0.0, 0.0]);
    assert!(close(x[0], 0.0, 1e-12) && close(x[1], 0.0, 1e-12) && close(x[2], -1.0, 1e-12), "{x:?}");
    for turn in [0.0, 17.0, 90.0, 180.0, 271.0, 359.0] {
        for tilt in [-90.0, -45.0, 0.0, 18.0, 90.0] {
            let m = view_matrix(turn, tilt);
            for r in 0..3 {
                for c in r..3 {
                    let d = (0..3).map(|k| m[r][k] * m[c][k]).sum::<f64>();
                    assert!(close(d, if r == c { 1.0 } else { 0.0 }, 1e-12), "{turn}/{tilt}: rows {r},{c} dot {d}");
                }
            }
            let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
                - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
                + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
            assert!(close(det, 1.0, 1e-12), "{turn}/{tilt}: det {det}");
            let v = [0.3, -0.7, 0.5];
            let (p, z) = View::new(turn, tilt).screen(v, (10.0, 20.0), 100.0);
            let rot = apply(&m, v);
            assert!(close(p.0, 10.0 + 100.0 * rot[0], 1e-12));
            assert!(close(p.1, 20.0 - 100.0 * rot[1], 1e-12));
            assert!(close(z, rot[2], 1e-12));
        }
    }
    // orthographic: the depth does not scale the point, and a z-only move changes nothing
    for z in [-9.0, 0.0, 3.5] {
        assert_eq!(project_rotated([0.25, -0.5, z], (7.0, 11.0), 50.0), (19.5, 36.0));
    }
    assert_eq!(project_rotated([0.0, 0.0, 0.0], (3.0, 4.0), 10.0), (3.0, 4.0));
}

#[test]
fn rotation_preserves_lengths_and_angles_across_the_controls() {
    let mut cases = 0;
    for m in molecules() {
        for turn in [0.0, 15.0, 90.0, 180.0, 275.0, 360.0] {
            for tilt in [-90.0, -45.0, 0.0, 18.0, 90.0] {
                let v = View::new(turn, tilt);
                let rot: Vec<[f64; 3]> = m.atoms.iter().map(|a| v.rotate(a.pos)).collect();
                for a in 0..m.atoms.len() {
                    for b in a + 1..m.atoms.len() {
                        let before = vlen(vsub(m.atoms[a].pos, m.atoms[b].pos));
                        let after = vlen(vsub(rot[a], rot[b]));
                        assert!(close(before, after, 1e-12), "{}/{turn}/{tilt}: {before} against {after}", m.name);
                        if a > 0 {
                            let ang = angle_between(m.atoms[a].pos, m.atoms[b].pos);
                            let ang2 = angle_between(rot[a], rot[b]);
                            assert!((ang - ang2).abs() < 1e-9, "{}/{turn}/{tilt}: {ang} against {ang2}", m.name);
                        }
                        cases += 1;
                    }
                }
                for d in m.lone {
                    assert!(close(vlen(v.rotate(*d)), 1.0, 1e-12));
                    assert!(close(vlen(v.rotate(*d)), vlen(*d), 1e-12));
                }
            }
        }
    }
    assert!(cases > 1000, "{cases}");
}

// ---------------------------------------------------------------------------------------------
// The drawing
// ---------------------------------------------------------------------------------------------

#[test]
fn a_full_turn_is_the_identity_and_the_angle_controls_refuse_odd_values() {
    for m in molecules() {
        let a = svg_of(&[s("molecule", m.name), f("turn", 0.0)]);
        let z = svg_of(&[s("molecule", m.name), f("turn", 360.0)]);
        assert_eq!(a, z, "{}: 360° is not the same view as 0°", m.name);
    }
    let a = svg_of(&[s("molecule", "CH4"), f("turn", 0.0)]);
    let q = svg_of(&[s("molecule", "CH4"), f("turn", 17.0)]);
    assert_ne!(a, q, "turn changes nothing");
    let a = svg_of(&[s("molecule", "CH4"), f("tilt", 0.0)]);
    let q = svg_of(&[s("molecule", "CH4"), f("tilt", 45.0)]);
    assert_ne!(a, q, "tilt changes nothing");
    for (v, want) in [(f64::NAN, TURN_DEFAULT), (f64::INFINITY, TURN_HI), (f64::NEG_INFINITY, TURN_LO), (-5.0, TURN_LO), (400.0, TURN_HI)] {
        let (_, r) = build(&raw(&[f("turn", v)]));
        assert_eq!(r.setup.turn, want, "turn {v}");
    }
    for (v, want) in [(f64::NAN, TILT_DEFAULT), (f64::INFINITY, TILT_HI), (f64::NEG_INFINITY, TILT_LO), (-100.0, TILT_LO), (100.0, TILT_HI)] {
        let (_, r) = build(&raw(&[f("tilt", v)]));
        assert_eq!(r.setup.tilt, want, "tilt {v}");
    }
    for list in [vec![f("tilt", -90.0)], vec![f("tilt", 90.0)], vec![f("turn", 90.0), f("tilt", 0.0)]] {
        let svg = svg_of(&list);
        assert!(svg.contains("<svg") && svg.matches("data-o=\"atom\"").count() >= 1, "{list:?}");
    }
}

#[test]
fn the_bonds_are_depth_sorted_far_thin_light_first() {
    for m in molecules() {
        for (tag, turn, tilt) in [("default", TURN_DEFAULT, TILT_DEFAULT), ("face", 0.0, 0.0), ("y", 90.0, 0.0), ("oblique", 35.0, -30.0), ("back", 300.0, 60.0)] {
            let list = [s("molecule", m.name), f("turn", turn), f("tilt", tilt)];
            let (svg, r) = build_of(&list);
            assert_eq!(svg, build_of(&list).0, "{}/{}: not deterministic", m.name, tag);
            for w in r.bonds.windows(2) {
                assert!(w[0].depth <= w[1].depth + 1e-12, "{}/{tag}: bonds out of depth order", m.name);
                if (w[0].depth - w[1].depth).abs() < 1e-12 { assert!(w[0].index < w[1].index, "{}/{tag}: a depth tie is not by index", m.name); }
            }
            let g = geo(&svg);
            let bonds: Vec<&Line> = g.lines.iter().filter(|l| l.kind == "bond").collect();
            let bfeats: BTreeSet<String> = bonds.iter().map(|l| l.feat.clone()).collect();
            assert_eq!(bfeats.len(), m.atoms.len() - 1, "{}/{tag}", m.name);
            if bonds.is_empty() { continue; }
            let mut prev_depth = f64::NEG_INFINITY;
            let mut prev_width = 0.0f64;
            let mut prev_grey = 255u8;
            for l in &bonds {
                assert!(l.depth >= prev_depth - 1e-12, "{}/{tag}: depth order {} after {}", m.name, l.depth, prev_depth);
                assert!(l.width >= prev_width - 1e-9, "{}/{tag}: width {} after {}", m.name, l.width, prev_width);
                let gv = grey_hex(&l.stroke).unwrap_or_else(|| panic!("{}/{tag}: bond stroke {:?}", m.name, l.stroke));
                assert!(gv <= prev_grey, "{}/{tag}: a nearer bond is lighter: {gv} after {prev_grey}", m.name);
                prev_depth = l.depth;
                prev_width = l.width;
                prev_grey = gv;
            }
        }
    }
    // a planar molecule at turn 0 / tilt 0 has all depths equal: index order decides
    let g = geo(&svg_of(&[s("molecule", "BF3"), f("turn", 0.0), f("tilt", 0.0)]));
    let mut order: Vec<String> = Vec::new();
    for l in g.lines.iter().filter(|l| l.kind == "bond") { if !order.contains(&l.feat) { order.push(l.feat.clone()); } }
    assert_eq!(order, ["b1", "b2", "b3"]);
}

#[test]
fn the_default_view_shows_every_ligand_and_no_overlap() {
    for m in molecules() {
        let r = key(&[s("molecule", m.name)]);
        assert_eq!(r.atoms.iter().filter(|a| a.hidden).count(), 0, "{}: a ligand is hidden", m.name);
        assert!(r.atoms.iter().all(|a| a.labelled), "{}: a label is hidden", m.name);
        let centre = r.atoms[0].p;
        for a in &r.atoms[1..] {
            assert!(dist(a.p, centre) > 0.25 * 118.0, "{}: a bond is too short at the default view", m.name);
        }
        for a in 0..r.atoms.len() {
            for b in a + 1..r.atoms.len() {
                assert!(dist(r.atoms[a].p, r.atoms[b].p) > 2.0 * r.atoms[0].r + 2.0, "{}: two balls overlap", m.name);
            }
        }
        for a in 0..r.atoms.len() {
            for b in a + 1..r.atoms.len() {
                if dist(r.atoms[a].p, r.atoms[b].p) < r.atoms[a].r + r.atoms[b].r {
                    assert!(r.view_notes.iter().any(|n| n.contains("overlap")), "{}: two atoms overlap at the opening view and no note names them", m.name);
                }
            }
        }
        assert!(!r.angles.is_empty(), "{}: no true angle at the opening view", m.name);
        for a in &r.angles {
            assert!(!a.text.is_empty(), "{}: a key angle label is blank at the opening view", m.name);
            let va = (r.atoms[a.a].p.0 - centre.0, r.atoms[a.a].p.1 - centre.1);
            let vb = (r.atoms[a.b].p.0 - centre.0, r.atoms[a.b].p.1 - centre.1);
            let shown = (va.0 * vb.1 - va.1 * vb.0).abs().atan2(va.0 * vb.0 + va.1 * vb.1).to_degrees();
            assert!((shown - a.angle).abs() <= 0.4, "{}: the opening arc reads {shown} but says {}", m.name, a.angle);
        }
    }
}

#[test]
fn the_switches_add_and_remove_atoms_bonds_lone_pairs_and_labels() {
    for (name, atoms, bonds, lone, angles) in [
        ("BeCl2", 3usize, 2usize, 0usize, 1usize),
        ("BF3", 4, 3, 0, 1),
        ("CH4", 5, 4, 0, 1),
        ("NH3", 4, 3, 1, 1),
        ("H2O", 3, 2, 2, 1),
        ("PCl5", 6, 5, 0, 1),
        ("SF6", 7, 6, 0, 1),
        ("XeF4", 5, 4, 2, 1),
        ("SF4", 5, 4, 1, 1),
    ] {
        let g = geo(&svg_of(&[s("molecule", name)]));
        assert_eq!(g.circles.iter().filter(|c| c.kind == "atom").count(), atoms, "{name} atoms");
        let bfeats: BTreeSet<&str> = g.lines.iter().filter(|l| l.kind == "bond").map(|l| l.feat.as_str()).collect();
        assert_eq!(bfeats.len(), bonds, "{name} bonds");
        let drawn = if name == "XeF4" { 0 } else { 2 * lone };
        assert_eq!(g.circles.iter().filter(|c| c.kind == "lone").count(), drawn, "{name} lone dots");
        let gfeats: BTreeSet<&str> = g.lines.iter().filter(|l| l.kind == "loneguide").map(|l| l.feat.as_str()).collect();
        assert_eq!(gfeats.len(), drawn / 2, "{name} lone-pair sticks");
        if drawn < 2 * lone {
            assert!(g.circles.iter().filter(|c| c.kind == "lone").count() == 0, "{name}");
            assert!(texts(&svg_of(&[s("molecule", name)])).iter().any(|t| t.text.contains("not drawn")), "{name}: omitted pair without a note");
        }
        assert_eq!(g.arcs.iter().filter(|a| a.kind == "anglearc").count(), angles, "{name} arcs");
        assert_eq!(texts(&svg_of(&[s("molecule", name)])).iter().filter(|t| t.kind == "angle").count(), angles, "{name} angle labels");
        assert_eq!(texts(&svg_of(&[s("molecule", name)])).iter().filter(|t| t.kind == "alabel").count(), atoms, "{name} atom labels");
        let panel = texts(&svg_of(&[s("molecule", name)])).into_iter().filter(|t| t.kind == "panel").map(|t| t.text).collect::<Vec<_>>().join(" ");
        assert!(panel.contains(&format!("Lone pairs: {lone}")), "{name}: {panel}");
    }
    // lone pairs off
    let g = geo(&svg_of(&[s("molecule", "H2O"), b("lone", false)]));
    assert_eq!(g.circles.iter().filter(|c| c.kind == "lone").count(), 0);
    assert_eq!(g.lines.iter().filter(|l| l.kind == "loneguide").count(), 0);
    // labels off: the balls stay
    let g = geo(&svg_of(&[s("molecule", "CH4"), b("labels", false)]));
    assert_eq!(g.circles.iter().filter(|c| c.kind == "atom").count(), 5);
    assert_eq!(texts(&svg_of(&[s("molecule", "CH4"), b("labels", false)])).iter().filter(|t| t.kind == "alabel").count(), 0);
    // angles off: no arcs and no angle text
    let svg = svg_of(&[s("molecule", "CH4"), b("angles", false)]);
    assert_eq!(geo(&svg).arcs.iter().filter(|a| a.kind == "anglearc").count(), 0);
    assert_eq!(texts(&svg).iter().filter(|t| t.kind == "angle").count(), 0);
    let joined = texts(&svg).iter().map(|t| t.text.as_str()).collect::<Vec<_>>().join(" ");
    assert!(joined.contains("Bond angle: not shown"), "{joined}");
    let joined = texts(&svg).iter().map(|t| t.text.as_str()).collect::<Vec<_>>().join(" ");
    assert!(!joined.contains("107") && !joined.contains("109.5"), "the explanation still prints an angle while Show bond angles is off: {joined}");
    assert!(joined.contains("far apart"), "the explanation lost its angle-free form: {joined}");
    // answers off removes the arc and blanks the value
    let svg = svg_of(&[s("molecule", "CH4"), b(SHOW_ANSWERS, false)]);
    assert_eq!(geo(&svg).arcs.iter().filter(|a| a.kind == "anglearc").count(), 0);
    assert_eq!(texts(&svg).iter().filter(|t| t.kind == "angle").count(), 0);
    let joined = texts(&svg).iter().map(|t| t.text.as_str()).collect::<Vec<_>>().join(" ");
    assert!(joined.contains("Bond angle: ______"), "the student angle is not blank");
    // the central and terminal fills differ, and each atom is drawn once
    let g = geo(&svg_of(&[s("molecule", "CH4")]));
    let central = g.circles.iter().find(|c| c.feat == "a0").unwrap();
    let ligand = g.circles.iter().find(|c| c.feat == "a1").unwrap();
    assert_ne!(central.fill, ligand.fill);
    assert_eq!(central.fill, "#4aa36b");
    assert_eq!(ligand.fill, "#c9494f");
    let gp = geo(&svg_of(&[s("molecule", "CH4"), s("style", "Print")]));
    assert_eq!(gp.circles.iter().find(|c| c.feat == "a0").unwrap().fill, "url(#vs-hatch2)");
    assert_eq!(gp.circles.iter().find(|c| c.feat == "a1").unwrap().fill, "url(#vs-hatch3)");
}

#[test]
fn lone_pairs_are_on_the_central_atom_and_point_between_the_bonds() {
    for m in molecules().iter().filter(|m| !m.lone.is_empty()) {
        let mut list = vec![s("molecule", m.name)];
        if m.name == "XeF4" {
            let (turn, tilt) = rel(m, 30.0, 20.0);
            list.push(f("turn", turn));
            list.push(f("tilt", tilt));
        }
        let r = key(&list);
        assert_eq!(r.lone.len(), m.lone.len(), "{}", m.name);
        let centre = r.atoms[0].p;
        for (k, l) in r.lone.iter().enumerate() {
            assert_eq!(l.pair, k);
            let d = dist(l.p, centre);
            assert!(d >= r.atoms[0].r, "{}: a lone pair sits inside the central ball", m.name);
            assert!(d <= 0.6 * 118.0, "{}: a lone pair is not on the central atom", m.name);
            assert!(dist(l.dots[0], l.dots[1]) > 2.0, "{}: the two dots coincide", m.name);
            for dot in l.dots {
                assert!(dist(dot, centre) > r.atoms[0].r, "{}: a lone dot is inside the central ball", m.name);
                for a in &r.atoms[1..] { assert!(dist(dot, a.p) > a.r + 2.0, "{}: a lone dot is on a ligand", m.name); }
            }
            // the 3-D lone pair points away from every bond (90° or more)
            for a in &m.atoms[1..] {
                let ang = angle_between(m.lone[k], a.pos);
                assert!(ang >= 89.9, "{}: lone pair {k} is {ang}° from a bond", m.name);
            }
        }
        for a in 1..r.lone.len() {
            assert!(dist(r.lone[a - 1].p, r.lone[a].p) > 8.0, "{}: two lone pairs overlap", m.name);
        }
        let g = geo(&svg_of(&list));
        assert_eq!(g.circles.iter().filter(|c| c.kind == "lone").count(), 2 * m.lone.len());
        let mut off_list = list.clone();
        off_list.push(b("lone", false));
        let off = geo(&svg_of(&off_list));
        assert_eq!(off.circles.iter().filter(|c| c.kind == "lone").count(), 0);
    }
}

#[test]
fn the_angle_label_sits_at_the_central_vertex_and_names_the_pair() {
    for m in molecules() {
        let r = key(&[s("molecule", m.name)]);
        assert!(!r.angles.is_empty(), "{}: no angle at the opening view", m.name);
        let centre = r.atoms[0].p;
        for a in r.angles.iter() {
            assert_eq!(a.vertex, centre, "{}: the arc is not at the central atom", m.name);
            assert!((angle_between(m.atoms[a.a].pos, m.atoms[a.b].pos) - a.angle).abs() <= 1.0, "{}", m.name);
            let arc_r = 0.30 + 0.02 * a.pair as f64;
            let pa = r.atoms[a.a].p;
            let pb = r.atoms[a.b].p;
            let p0 = (centre.0 + (pa.0 - centre.0) * arc_r, centre.1 + (pa.1 - centre.1) * arc_r);
            let p1 = (centre.0 + (pb.0 - centre.0) * arc_r, centre.1 + (pb.1 - centre.1) * arc_r);
            assert!(dist(a.arc[0], p0) < 0.1, "{}: the arc does not start on the first bond", m.name);
            assert!(dist(*a.arc.last().unwrap(), p1) < 0.1, "{}: the arc does not end on the second bond", m.name);
            let rr = dist(a.at, centre);
            assert!(rr > 0.35 * 118.0 && rr < 0.85 * 118.0, "{}: the label is {rr} from the vertex", m.name);
            assert_eq!(a.text, m.pairs[a.pair].text);
            assert!(!a.text.is_empty());
            let mid = a.arc[a.arc.len() / 2];
            let la = screen_angle(centre, a.at);
            let lm = screen_angle(centre, mid);
            let diff = ang_diff(la, lm);
            assert!(diff < 50.0, "{}: the label is {diff}° off the arc midpoint", m.name);
            assert!(texts(&svg_of(&[s("molecule", m.name)])).iter().any(|t| t.kind == "angle" && t.feat == format!("p{}", a.pair) && t.text == a.text), "{}: no angle text for pair {}", m.name, a.pair);
        }
        let panel = texts(&svg_of(&[s("molecule", m.name)])).into_iter().filter(|t| t.kind == "panel").map(|t| t.text).collect::<Vec<_>>().join(" ");
        assert!(panel.contains("Bond angle"), "{panel}");
        assert!(panel.contains(&m.angle_text()), "{panel}");
        // the student copy has no angle arc or label at all
        let st = student(&[s("molecule", m.name)]);
        assert!(st.angles.is_empty());
    }
}

#[test]
fn the_angle_switch_hides_the_arc_and_the_value() {
    let k = key(&[s("molecule", "NH3")]);
    assert_eq!(k.angles.len(), 1);
    assert_eq!(k.angles[0].text, "107°");
    let svg = svg_of(&[s("molecule", "NH3")]);
    assert_eq!(geo(&svg).arcs.iter().filter(|a| a.kind == "anglearc").count(), 1);
    let joined = texts(&svg).iter().map(|t| t.text.as_str()).collect::<Vec<_>>().join(" ");
    assert!(joined.contains("Bond angle: 107°"), "{joined}");
    let svg = svg_of(&[s("molecule", "NH3"), b("angles", false)]);
    assert_eq!(geo(&svg).arcs.iter().filter(|a| a.kind == "anglearc").count(), 0);
    assert_eq!(texts(&svg).iter().filter(|t| t.kind == "angle").count(), 0);
    let joined = texts(&svg).iter().map(|t| t.text.as_str()).collect::<Vec<_>>().join(" ");
    assert!(joined.contains("Bond angle: not shown"), "{joined}");
    let joined = texts(&svg).iter().map(|t| t.text.as_str()).collect::<Vec<_>>().join(" ");
    assert!(!joined.contains("107") && !joined.contains("109.5"), "the explanation still prints an angle while Show bond angles is off: {joined}");
    assert!(joined.contains("trigonal pyramid"), "the explanation lost its angle-free form: {joined}");
    // an edge-on view leaves out an arc that would collapse to a line or a hidden bond
    for (name, turn, tilt) in [("XeF4", 90.0, 0.0), ("BF3", 90.0, 0.0), ("BeCl2", 90.0, 0.0)] {
        let r = key(&[s("molecule", name), f("turn", turn), f("tilt", tilt)]);
        assert!(r.angles.is_empty(), "{name} at {turn}/{tilt}: an edge-on arc is still drawn");
        let svg = svg_of(&[s("molecule", name), f("turn", turn), f("tilt", tilt)]);
        assert_eq!(geo(&svg).arcs.iter().filter(|a| a.kind == "anglearc").count(), 0, "{name}");
        assert!(texts(&svg).iter().any(|t| t.text.contains("Bond angle:")), "{name}: the panel lost the angle line");
    }
    // a visible 90° pair survives at an edge-on view of the octahedron
    let r = key(&[s("molecule", "SF6"), f("turn", 90.0), f("tilt", 0.0)]);
    assert_eq!(r.angles.len(), 1);
    assert_eq!(r.angles[0].text, "90°");
}

#[test]
fn the_student_copy_blanks_the_geometry_and_the_angle_without_leaking() {
    let angle_words = ["180°", "120°", "109.5°", "107°", "104.5°", "90°", "102°", "~102°"];
    let shape_words = ["linear", "trigonal", "planar", "tetrahedral", "pyramidal", "bent", "bipyramidal", "octahedral", "square", "see-saw"];
    for m in molecules() {
        let key_svg = def().svg(&with(&[s("molecule", m.name), b(SHOW_ANSWERS, true)]));
        let kpanel = texts(&key_svg).into_iter().filter(|t| t.kind == "panel").map(|t| t.text).collect::<Vec<_>>().join(" ");
        assert!(kpanel.contains(m.shape), "{}: the key does not name {:?}: {kpanel}", m.name, m.shape);
        assert!(kpanel.contains(m.domain_geo), "{}: the key does not name {:?}: {kpanel}", m.name, m.domain_geo);
        for pair in m.pairs { assert!(kpanel.contains(pair.text), "{}: the key does not print {:?}", m.name, pair.text); }
        let p = with(&[s("molecule", m.name), b(SHOW_ANSWERS, false)]);
        let svg = def().svg(&p);
        let low = svg.to_ascii_lowercase();
        for w in shape_words {
            assert!(!low.contains(w), "{}: the student source contains the shape word {w}", m.name);
        }
        for w in angle_words {
            assert!(!svg.contains(w), "{}: the student source contains the angle {w}", m.name);
        }
        assert!(!svg.contains("data-o=\"angle\""), "{}: the student has an angle text", m.name);
        let ts = texts(&svg);
        assert!(!ts.iter().any(|t| t.text.contains('°')), "{}: the student has a degree sign in its text", m.name);
        for t in &ts {
            for tok in t.text.split(|c: char| !c.is_ascii_digit() && c != '.') {
                if let Ok(v) = tok.parse::<f64>() {
                    assert!(![90.0, 102.0, 104.5, 107.0, 109.5, 120.0, 180.0].contains(&v), "{}: the student text {:?} leaks {v}", m.name, t.text);
                }
            }
        }
        let joined = ts.iter().map(|t| t.text.as_str()).collect::<Vec<_>>().join(" ");
        // the label and the value are separate text elements, so count the blanks themselves
        let blanks: usize = ts.iter().map(|t| t.text.matches(BLANK).count()).sum();
        assert!(blanks >= 2 + m.pairs.len(), "{}: {blanks} blanks for {} angles", m.name, m.pairs.len());
        assert!(joined.contains("Molecular shape: ______"), "{}: {joined}", m.name);
        assert!(joined.contains("Bond angle"), "{}: {joined}", m.name);
        assert!(joined.contains("Explain the shape and the angle"), "{}: {joined}", m.name);
        assert_eq!(geo(&svg).arcs.iter().filter(|a| a.kind == "anglearc").count(), 0, "{}: the student has an angle arc", m.name);
        // the key's explanation is not on the student copy
        assert!(!svg.contains(m.why), "{}: the student carries the key's explanation", m.name);
    }
}

#[test]
fn print_is_grey_only_and_survives_sixty_six_percent() {
    for m in molecules() {
        for ans in [true, false, true] {
            let list = [s("molecule", m.name), s("style", "Print"), b(SHOW_ANSWERS, ans)];
            let svg = svg_of(&list);
            for k in ["fill", "stroke"] {
                for v in svg.split(&format!(" {k}=\"")).skip(1).map(|x| x.split('"').next().unwrap()) {
                    assert!(paint_is_grey(v), "{}: Print draws a hue: {k}={v}", m.name);
                }
            }
            let g = geo(&svg);
            let central = g.circles.iter().find(|c| c.feat == "a0").unwrap();
            let ligand = g.circles.iter().find(|c| c.feat == "a1").unwrap();
            assert!(central.fill.starts_with("url(#vs-hatch") && ligand.fill.starts_with("url(#vs-hatch"), "{}", m.name);
            assert_ne!(central.fill, ligand.fill, "{}: the central and terminal atoms share a fill", m.name);
            let small = texts(&svg).iter().map(|t| t.fs).fold(f64::INFINITY, f64::min);
            assert!(small * 0.66 >= 8.4, "{}: the smallest type is {small} px", m.name);
            if matches!(m.name, "H2O" | "SF6" | "SF4") {
                let png = to_png(&svg, 0.66).unwrap();
                let img = image::load_from_memory(&png).unwrap().to_rgba8();
                let mut dark = 0;
                for px in img.pixels() {
                    assert!(px[0] == px[1] && px[1] == px[2], "{}: colour survives a 66 % Print copy", m.name);
                    if px[0] < 128 { dark += 1; }
                }
                assert!(dark > 200, "{}: the 66 % copy is nearly blank ({dark} dark pixels)", m.name);
            }
        }
    }
    // Colour uses the two series colours and is not grey
    let svg = svg_of(&[s("molecule", "CH4")]);
    let g = geo(&svg);
    let central = g.circles.iter().find(|c| c.feat == "a0").unwrap();
    let ligand = g.circles.iter().find(|c| c.feat == "a1").unwrap();
    assert!(grey_hex(&central.fill).is_none() && grey_hex(&ligand.fill).is_none(), "{} {}", central.fill, ligand.fill);
}

#[test]
fn label_size_scales_the_type_and_the_canvas() {
    let mut sizes = Vec::new();
    let mut minfs = Vec::new();
    for name in ["Small", "Medium", "Large"] {
        let svg = svg_of(&[s("molecule", "CH4"), s("label_size", name)]);
        let (w, h) = svg_size(&svg);
        sizes.push((w, h));
        minfs.push(texts(&svg).iter().map(|t| t.fs).fold(f64::INFINITY, f64::min));
        assert_eq!(w, 760.0, "{name}");
        assert!(h > 400.0, "{name}: {h}");
        let atom_fs: Vec<f64> = texts(&svg).iter().filter(|t| t.kind == "alabel").map(|t| t.fs).collect();
        assert!(!atom_fs.is_empty() && atom_fs.iter().all(|f| close(*f, 17.0 * crate::figures::svg::label_factor(name), 1e-9)));
    }
    assert!(sizes[0].1 < sizes[1].1 && sizes[1].1 < sizes[2].1, "{sizes:?}");
    assert!(minfs[0] < minfs[1] && minfs[1] < minfs[2], "{minfs:?}");
    assert!(close(minfs[1] / minfs[0], 1.25, 1e-9), "{minfs:?}");
    assert!(close(minfs[2] / minfs[1], 1.4, 1e-9), "{minfs:?}");
}

// ---------------------------------------------------------------------------------------------
// Real glyphs: the collision audit over the option space
// ---------------------------------------------------------------------------------------------

struct Glyphs { opt: resvg::usvg::Options<'static>, memo: HashMap<String, Option<Bx>> }

impl Glyphs {
    fn new() -> Self {
        let mut opt = resvg::usvg::Options::default();
        opt.fontdb = crate::figures::fontdb();
        Glyphs { opt, memo: HashMap::new() }
    }
    fn rel(&mut self, t: &T) -> Option<Bx> {
        let key = format!("{}|{:.2}|{}", t.text, t.fs, t.anchor);
        if let Some(v) = self.memo.get(&key) { return *v; }
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="6000" height="600"><text x="3000" y="300" font-size="{:.2}" text-anchor="{}" font-family="{FONT}">{}</text></svg>"#,
            t.fs, t.anchor, crate::figures::svg::esc(&t.text)
        );
        fn find(g: &resvg::usvg::Group) -> Option<resvg::usvg::Rect> {
            for n in g.children() {
                match n {
                    resvg::usvg::Node::Text(t) => return Some(t.flattened().abs_bounding_box()),
                    resvg::usvg::Node::Group(g2) => { if let Some(r) = find(g2) { return Some(r); } }
                    _ => {}
                }
            }
            None
        }
        let out = resvg::usvg::Tree::from_str(&svg, &self.opt).ok().and_then(|tree| find(tree.root())).filter(|r| r.width() > 0.0)
            .map(|r| (r.left() as f64 - 3000.0, r.right() as f64 - 3000.0, r.top() as f64 - 300.0, r.bottom() as f64 - 300.0));
        self.memo.insert(key, out);
        out
    }
    fn ink(&mut self, t: &T) -> Option<Bx> {
        let r = self.rel(t)?;
        Some((t.x + r.0, t.x + r.1, t.y + r.2, t.y + r.3))
    }
}

fn audit(svg: &str, g: &mut Glyphs) -> Vec<String> {
    let (w, h) = svg_size(svg);
    let all = texts(svg);
    let ts: Vec<&T> = all.iter().filter(|t| t.fill != "none").collect();
    let gm = geo(svg);
    let mut bad = Vec::new();
    let inks: Vec<Option<Bx>> = ts.iter().map(|t| g.ink(t)).collect();
    for (t, bx) in ts.iter().zip(&inks) {
        let Some(bx) = bx else { continue };
        if bx.0 < 0.0 || bx.1 > w || bx.2 < 0.0 || bx.3 > h {
            bad.push(format!("{:?} leaves the {w}x{h} canvas: {bx:?}", t.text));
        }
    }
    for a in 0..ts.len() {
        for c in a + 1..ts.len() {
            if let (Some(x), Some(y)) = (inks[a], inks[c]) {
                if hits(x, y) { bad.push(format!("{:?} on {:?}", ts[a].text, ts[c].text)); }
            }
        }
    }
    let atom_depth: HashMap<String, f64> = gm.circles.iter().filter(|c| c.kind == "atom").map(|c| (c.feat.clone(), c.depth)).collect();
    for (t, bx) in ts.iter().zip(&inks) {
        let Some(bx) = bx else { continue };
        for l in &gm.lines {
            if l.kind == "rule" { continue; }
            // An angle label carries a white halo, the documented way to cross a stroke.
            if t.kind == "angle" { continue; }
            // A zero-length or overlapping stick is inside a ball and cannot strike a label.
            let mid = ((l.a.0 + l.b.0) / 2.0, (l.a.1 + l.b.1) / 2.0);
            if gm.circles.iter().any(|c| c.kind == "atom" && dist(mid, c.c) < c.r) { continue; }
            // An atom's own ball is drawn over every stick that crosses it.
            if t.kind == "alabel" {
                if let Some(idx) = t.feat.strip_prefix('a') {
                    if l.feat == format!("b{idx}") { continue; }
                    if let Some(c) = gm.circles.iter().find(|c| c.kind == "atom" && c.feat == t.feat) {
                        let bb = (c.c.0 - c.r, c.c.0 + c.r, c.c.1 - c.r, c.c.1 + c.r);
                        if seg_hits(l.a, l.b, bb) { continue; }
                    }
                }
            }
            if seg_hits(l.a, l.b, *bx) { bad.push(format!("{:?} box {bx:?} on a {} stroke {:?}-{:?}", t.text, l.kind, l.a, l.b)); }
        }
        for c in &gm.circles {
            if c.kind == "atom" && box_hits_disc(*bx, c.c, c.r) {
                if t.kind == "alabel" && (t.feat == c.feat || t.feat == "a0") { continue; }
                let nearer = match (t.kind.as_str(), atom_depth.get(&t.feat)) {
                    ("alabel", Some(d)) => c.depth > d + 1e-9,
                    _ => true,
                };
                if nearer { bad.push(format!("{:?} box {bx:?} on the {} ball at {:?}", t.text, c.feat, c.c)); }
            }
            if c.kind == "lone" && box_hits_disc(*bx, c.c, c.r) {
                bad.push(format!("{:?} box {bx:?} on a lone-pair dot at {:?}", t.text, c.c));
            }
        }
    }
    bad
}

fn view_cases() -> Vec<(&'static str, f64, f64)> {
    vec![
        ("default", TURN_DEFAULT, TILT_DEFAULT),
        ("face", 0.0, 0.0),
        ("down-y", 90.0, 0.0),
        ("back", 180.0, 0.0),
        ("tilt90", 0.0, 90.0),
        ("tilt-90", 0.0, -90.0),
        ("oblique", 35.0, -30.0),
        ("oblique2", 300.0, 60.0),
    ]
}

fn scenario_cases() -> Vec<(String, Vec<(&'static str, Value)>)> {
    let mut v: Vec<(String, Vec<(&'static str, Value)>)> = Vec::new();
    for name in NAMES {
        for (tag, turn, tilt) in view_cases() {
            v.push((format!("{name}/{tag}"), vec![s("molecule", name), f("turn", turn), f("tilt", tilt)]));
        }
    }
    v.push(("no-lone".into(), vec![s("molecule", "H2O"), b("lone", false)]));
    v.push(("no-labels".into(), vec![s("molecule", "SF6"), b("labels", false)]));
    v.push(("no-angles".into(), vec![s("molecule", "SF4"), b("angles", false)]));
    v.push(("nothing-extra".into(), vec![s("molecule", "XeF4"), b("lone", false), b("angles", false), b("labels", false)]));
    v
}

#[test]
fn labels_clear_every_text_and_the_canvas_across_the_option_space() {
    let mut g = Glyphs::new();
    let mut problems: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut n = 0;
    for (name, base) in scenario_cases() {
        for style in ["Colour", "Print"] {
            for size in ["Small", "Medium", "Large"] {
                for ans in [true, false] {
                    let mut l = base.clone();
                    l.extend([s("style", style), s("label_size", size), b(SHOW_ANSWERS, ans)]);
                    let p = with(&l);
                    let svg = build(&p).0;
                    n += 1;
                    for m in audit(&svg, &mut g) {
                        let key: String = m.chars().filter(|c| !c.is_ascii_digit()).take(60).collect();
                        let e = problems.entry(key).or_default();
                        if e.len() < 3 { e.push(format!("{name}/{style}/{size}/{ans}: {m}")); }
                    }
                }
            }
        }
    }
    assert!(n >= 500, "{n} renders");
    assert!(problems.is_empty(), "{} kinds of problem:\n{}", problems.len(), problems.values().map(|v| v.join("\n")).collect::<Vec<_>>().join("\n\n"));
}

// ---------------------------------------------------------------------------------------------
// Refusals and hostile input
// ---------------------------------------------------------------------------------------------

#[test]
fn refusals_name_the_box_the_reason_and_a_fixed_value() {
    let cases: Vec<(&str, Vec<(&'static str, Value)>, &[&str])> = vec![
        ("molecule", vec![s("molecule", "Unobtainium")], &["Molecule \"Unobtainium\"", "CH4 is used"]),
        ("molecule-type", vec![i("molecule", 3)], &["Molecule is not a name", "CH4 is used"]),
        ("turn-low", vec![f("turn", -10.0)], &["Turn = −10°", "below the 0°", "0° is used"]),
        ("turn-high", vec![f("turn", 400.0)], &["Turn = 400°", "above the 360°", "360° is used"]),
        ("turn-nan", vec![f("turn", f64::NAN)], &["Turn", "not a number", "0° is used"]),
        ("turn-inf", vec![f("turn", f64::INFINITY)], &["Turn = ∞°", "above the 360°", "360° is used"]),
        ("turn-neg-inf", vec![f("turn", f64::NEG_INFINITY)], &["Turn = −∞°", "below the 0°", "0° is used"]),
        ("tilt-low", vec![f("tilt", -100.0)], &["Tilt = −100°", "below the −90°", "−90° is used"]),
        ("tilt-high", vec![f("tilt", 100.0)], &["Tilt = 100°", "above the 90°", "90° is used"]),
        ("tilt-nan", vec![f("tilt", f64::NAN)], &["Tilt", "not a number", "0° is used"]),
        ("lone-type", vec![s("lone", "yes")], &["Show lone pairs", "not on or off", "on by default"]),
        ("labels-type", vec![i("labels", 1)], &["Show atom labels", "not on or off", "on by default"]),
    ];
    for (tag, list, wants) in cases {
        let p = raw(&list);
        let notes = warnings(&p);
        for w in wants { assert!(has(&notes, w), "{tag}: {notes:?} lacks {w:?}"); }
        assert_eq!(crate::figures::notes_for(ID, &p), notes, "{tag}");
        let svg = def().svg(&p);
        assert!(svg.contains("<svg") && svg.matches("data-o=\"atom\"").count() >= 1, "{tag}: the rest of the figure is missing");
    }
    // the fixed value is the one used
    for (bad, good) in [
        (vec![f("turn", -10.0)], vec![f("turn", 0.0)]),
        (vec![f("turn", 400.0)], vec![f("turn", 360.0)]),
        (vec![f("turn", f64::INFINITY)], vec![f("turn", 360.0)]),
        (vec![f("tilt", 100.0)], vec![f("tilt", 90.0)]),
        (vec![f("tilt", -100.0)], vec![f("tilt", -90.0)]),
        (vec![s("molecule", "Unobtainium")], vec![s("molecule", "CH4")]),
        (vec![s("lone", "yes")], vec![b("lone", true)]),
        (vec![i("labels", 1)], vec![b("labels", true)]),
    ] {
        let a = svg_of(&bad);
        let c = svg_of(&good);
        assert_eq!(a, c, "{bad:?} did not use the fixed value {good:?}");
    }
    // a wrong-typed answer switch is the student copy, not the key
    let p = raw(&[s(SHOW_ANSWERS, "yes")]);
    let (svg, r) = build(&p);
    assert!(!r.setup.answers);
    assert_eq!(texts(&svg).iter().filter(|t| t.kind == "angle").count(), 0);
}

#[test]
fn ten_thousand_odd_inputs_never_panic_or_print_nan() {
    let keys = ["molecule", "lone", "angles", "labels", "turn", "tilt", "style", "label_size", SHOW_ANSWERS, "bogus"];
    let vals = [
        Value::F(f64::NAN), Value::F(f64::INFINITY), Value::F(f64::NEG_INFINITY), Value::F(1e300), Value::F(-1e300),
        Value::F(0.0), Value::F(360.0), Value::F(361.0), Value::F(-1.0), Value::I(i64::MAX), Value::I(i64::MIN),
        Value::I(0), Value::I(1), Value::B(true), Value::B(false), Value::S(String::new()),
        Value::S("CH4".into()), Value::S("h2o".into()), Value::S("Xx".into()), Value::S("x".repeat(1000)),
        Value::S("Colour".into()), Value::S("Print".into()), Value::S("Large".into()), Value::S("Small".into()),
    ];
    let mut seed = 0x1234_5678_9abc_def0u64;
    let mut rnd = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) as usize
    };
    let t0 = std::time::Instant::now();
    for n in 0..10_000 {
        let mut p = raw(&[]);
        let k = keys[rnd() % keys.len()];
        p.0.insert(k.to_string(), vals[rnd() % vals.len()].clone());
        if n % 3 == 0 {
            let k2 = keys[rnd() % keys.len()];
            p.0.insert(k2.to_string(), vals[rnd() % vals.len()].clone());
        }
        let (svg, rep) = build(&p);
        assert!(svg.contains("<svg") && svg.len() > 800, "{n}: {k}");
        assert!(!bad_number_word(&svg), "{n}: {k}");
        assert!(!rep.notes.iter().any(|l| bad_number_word(l)), "{n}: {k}: {:?}", rep.notes);
        for t in texts(&svg) { assert!(!bad_number_word(&t.text), "{n}: {k}: {:?}", t.text); }
        if n % 97 == 0 { assert_eq!(svg, build(&p).0, "{n}: not deterministic"); }
    }
    println!("vsepr fuzz: 10000 builds in {:?} (debug build)", t0.elapsed());
    assert!(t0.elapsed() < std::time::Duration::from_secs(120), "{:?}", t0.elapsed());
}

#[test]
fn drawing_is_pure_and_within_a_cpu_budget() {
    let base = [s("molecule", "SF4"), f("turn", 35.0), f("tilt", 16.0), s("label_size", "Large")];
    let a = build_of(&base).0;
    let _ = svg_of(&[b(SHOW_ANSWERS, false)]);
    assert_eq!(a, build_of(&base).0);
    assert_eq!(a, build_of(&base).0);
    let mut worst = std::time::Duration::ZERO;
    let mut total = std::time::Duration::ZERO;
    let mut n = 0;
    for name in NAMES {
        for (turn, tilt) in [(0.0, 0.0), (TURN_DEFAULT, TILT_DEFAULT), (90.0, 0.0), (300.0, 60.0)] {
            for ans in [true, false] {
                let p = with(&[s("molecule", name), f("turn", turn), f("tilt", tilt), b(SHOW_ANSWERS, ans), s("label_size", "Large")]);
                let t = std::time::Instant::now();
                let svg = (def().render)(&p);
                let dt = t.elapsed();
                assert!(svg.len() > 1500, "{name}");
                worst = worst.max(dt);
                total += dt;
                n += 1;
            }
        }
    }
    println!("vsepr: {n} draws, worst {worst:?}, mean {:?} (debug build)", total / n);
    assert!(worst < std::time::Duration::from_millis(1500), "{worst:?}");
    assert!(total / n < std::time::Duration::from_millis(100), "{:?}", total / n);
}

#[test]
fn the_description_is_ascii_and_names_no_answer() {
    for list in [vec![], vec![s("molecule", "H2O")], vec![b(SHOW_ANSWERS, false)]] {
        let svg = svg_of(&list);
        let desc = svg.split("<desc>").nth(1).and_then(|x| x.split("</desc>").next()).unwrap_or("").to_string();
        let title = svg.split("<title>").nth(1).and_then(|x| x.split("</title>").next()).unwrap_or("").to_string();
        assert!(desc.is_ascii() && !desc.is_empty() && desc.len() <= 320, "{desc}");
        assert!(title.is_ascii(), "{title}");
        assert!(!desc.chars().any(|c| c.is_ascii_digit()), "{desc}");
        for w in ["Linear", "Tetrahedral", "Bent", "Octahedral", "See-saw", "°", "="] {
            assert!(!desc.contains(w), "{desc}");
        }
    }
    let desc_of = |svg: &str| svg.split("<desc>").nth(1).and_then(|x| x.split("</desc>").next()).unwrap_or("").to_string();
    let key_desc = desc_of(&svg_of(&[]));
    assert!(key_desc.contains("atom labels") && key_desc.contains("bond angles"), "{key_desc}");
    assert!(!key_desc.contains("lone pairs"), "CH4 has no lone pair to draw: {key_desc}");
    let water_desc = desc_of(&svg_of(&[s("molecule", "H2O")]));
    assert!(water_desc.contains("lone pairs") && water_desc.contains("bond angles"), "{water_desc}");
    let xe_desc = desc_of(&svg_of(&[s("molecule", "XeF4")]));
    assert!(!xe_desc.contains("lone pairs"), "the face-on XeF4 pairs are omitted: {xe_desc}");
    let student_desc = desc_of(&svg_of(&[b(SHOW_ANSWERS, false)]));
    assert!(!student_desc.contains("bond angles"), "{student_desc}");
    let no_labels_desc = desc_of(&svg_of(&[b(SHOW_ANSWERS, false), b("labels", false)]));
    assert!(!no_labels_desc.contains("atom labels") && !no_labels_desc.contains("bond angles"), "{no_labels_desc}");
}

#[test]
fn the_header_and_faq_state_the_chemistry() {
    let src = include_str!("../vsepr.rs");
    for w in ["electron-domain geometry", "molecular shape", "lone pair", "109.5", "107", "104.5", "ball-and-stick", "no double bonds", "resonance", "not to scale"] {
        assert!(src.contains(w), "the module header lacks {w}");
    }
    let faq = crate::figures::faq::faq_for(ID);
    assert_eq!(faq.len(), 6);
    let all: String = faq.iter().map(|f| format!("{} {}", f.q, f.a)).collect::<Vec<_>>().join(" ");
    for w in ["water", "104.5", "ammonia", "pyramid", "lone pair", "SF6", "octahedral", "double bond", "resonance", "bond length", "electron-domain"] {
        assert!(all.contains(w), "the FAQ lacks {w}");
    }
    let water = faq.iter().find(|f| f.q.contains("water")).unwrap();
    assert!(water.a.contains("104.5") && water.a.contains("109.5") && water.a.contains("never"), "{}", water.a);
    let h2o = key(&[s("molecule", "H2O")]);
    let panel = h2o.panel.join(" ");
    assert!(panel.contains("104.5"), "{panel}");
    assert!(!panel.contains("109.5"), "{panel}");
}

#[test]
fn the_share_migration_is_the_documented_note_path() {
    assert!(crate::figures::start::added_in(ID).is_empty(), "a migration entry was added without revisiting this note");
    // v7 (the Venn diagram's tint) changed nothing here: this figure still has no versioned opening key
    assert_eq!(crate::figures::share::VERSION, 7, "the global share version moved; revisit the VSEPR migration note");
    let src = include_str!("../vsepr.rs");
    let flat = src.replace("//!", " ").split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(flat.contains("new and unreleased"), "the header note is missing");
    assert!(flat.contains("saved specification or link can predate"), "the header note is incomplete");
}

#[test]
fn vsepr_pngs() {
    let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
    let _ = std::fs::create_dir_all(&dir);
    let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = Vec::new();
    for m in molecules() {
        cases.push((format!("{}-key", m.name), vec![s("molecule", m.name)]));
        cases.push((format!("{}-student", m.name), vec![s("molecule", m.name), b(SHOW_ANSWERS, false)]));
        cases.push((format!("{}-print", m.name), vec![s("molecule", m.name), s("style", "Print")]));
    }
    cases.push(("CH4-large".into(), vec![s("molecule", "CH4"), s("label_size", "Large")]));
    cases.push(("CH4-small".into(), vec![s("molecule", "CH4"), s("label_size", "Small")]));
    let mut abs = |tag: &str, name: &str, turn: f64, tilt: f64| {
        let m = molecules().iter().find(|m| m.name == name).unwrap();
        let (r, t) = rel(m, turn, tilt);
        cases.push((tag.into(), vec![s("molecule", name), f("turn", r), f("tilt", t)]));
    };
    abs("NH3-abs-edge", "NH3", 0.0, 0.0);
    abs("SF6-abs-edge", "SF6", 90.0, 0.0);
    abs("BeCl2-abs-edge", "BeCl2", 90.0, 0.0);
    abs("XeF4-abs-face", "XeF4", 0.0, 0.0);
    abs("XeF4-abs-edge", "XeF4", 90.0, 0.0);
    abs("BF3-abs-edge", "BF3", 90.0, 0.0);
    abs("SF4-abs-90", "SF4", 90.0, 0.0);
    abs("CH4-near", "CH4", 35.0, -30.0);
    let d = def();
    for (tag, list) in cases {
        let p = with(&list);
        let png = to_png(&d.svg(&p), 1.5).unwrap();
        std::fs::write(format!("{dir}/{ID}-{tag}.png"), png).unwrap();
    }
}


#[test]
fn every_opening_view_shows_its_true_angle_and_no_other_view_claims_one() {
    fn shown(r: &Report, a: &AngleReport) -> f64 {
        let va = (r.atoms[a.a].p.0 - a.vertex.0, r.atoms[a.a].p.1 - a.vertex.1);
        let vb = (r.atoms[a.b].p.0 - a.vertex.0, r.atoms[a.b].p.1 - a.vertex.1);
        (va.0 * vb.1 - va.1 * vb.0).abs().atan2(va.0 * vb.0 + va.1 * vb.1).to_degrees()
    }
    for m in molecules() {
        let r = key(&[s("molecule", m.name)]);
        assert!(close(r.view.0, m.open.0.rem_euclid(360.0), 1e-12), "{}: opening turn {:?} not {:?}", m.name, r.view, m.open);
        assert!(close(r.view.1, m.open.1, 1e-12), "{}: opening tilt {:?} not {:?}", m.name, r.view, m.open);
        assert!(!r.angles.is_empty(), "{}: no true angle at the opening view", m.name);
        for a in &r.angles {
            let got = shown(&r, a);
            assert!((got - a.angle).abs() <= 0.4, "{}: the opening arc reads {got} but says {}", m.name, a.angle);
            assert_eq!(a.text, m.pairs[a.pair].text);
        }
        let panel = texts(&svg_of(&[s("molecule", m.name)])).into_iter().filter(|t| t.kind == "panel").map(|t| t.text).collect::<Vec<_>>().join(" ");
        assert!(panel.contains(&m.angle_text()), "{}: the panel does not name {}", m.name, m.angle_text());
        // Any other view may draw an arc only when it too reads true on the page.
        for (tag, turn, tilt) in [
            ("old-default", 255.0, 16.0),
            ("oblique", 35.0, -30.0),
            ("down-y", 90.0, 0.0),
            ("face", 0.0, 90.0),
            ("back", 180.0, 45.0),
        ] {
            let q = key(&[s("molecule", m.name), f("turn", turn), f("tilt", tilt)]);
            for a in &q.angles {
                let got = shown(&q, a);
                assert!((got - a.angle).abs() <= 0.4, "{} at {tag}: the arc reads {got} but says {}", m.name, a.angle);
            }
        }
        // At a foreshortened view the water angle is left off (the panel still names it).
        if m.name == "H2O" {
            let q = key(&[s("molecule", "H2O"), f("turn", 255.0 - m.open.0), f("tilt", 16.0 - m.open.1)]);
            assert!(q.angles.is_empty(), "H2O still draws an angle at the old foreshortened view");
            assert!(q.panel.join(" ").contains("104.5"), "H2O lost its angle from the panel");
        }
        // The student copy has no arc and no angle text at all.
        let st = student(&[s("molecule", m.name)]);
        assert!(st.angles.is_empty(), "{}: the student has an angle report", m.name);
        let svg = def().svg(&with(&[s("molecule", m.name), b(SHOW_ANSWERS, false)]));
        assert_eq!(geo(&svg).arcs.iter().filter(|a| a.kind == "anglearc").count(), 0, "{}: the student has an arc", m.name);
        assert!(!svg.contains("data-o=\"angle\""), "{}: the student has angle text", m.name);
    }
    let src = include_str!("../vsepr.rs");
    assert!(src.contains("the arc is the true angle") || src.contains("The arc is the true angle"), "the header does not explain the arc");
    assert!(src.contains("orthographic projection"), "the header does not name the projection");
    let faq = crate::figures::faq::faq_for(ID).iter().map(|f| format!("{} {}", f.q, f.a)).collect::<Vec<_>>().join(" ");
    assert!(faq.contains("foreshorten"), "the FAQ does not explain foreshortening");
}

#[test]
fn lone_pairs_are_on_a_short_dashed_stick_clear_of_every_bond() {
    fn point_seg(p: Pt, a: Pt, b: Pt) -> f64 {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let l2 = dx * dx + dy * dy;
        if l2 < 1e-12 { return dist(p, a); }
        let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0);
        dist(p, (a.0 + t * dx, a.1 + t * dy))
    }
    let mut checked = 0;
    for m in molecules().iter().filter(|m| !m.lone.is_empty()) {
        for turn in (0..360).step_by(15) {
            for tilt in (-90..=90).step_by(30) {
                let r = key(&[s("molecule", m.name), f("turn", turn as f64), f("tilt", tilt as f64)]);
                for l in &r.lone {
                    let d = dist(l.p, r.atoms[0].p);
                    assert!(d > r.atoms[0].r + 2.0, "{}: a lone pair is inside the central ball", m.name);
                    assert!(d < 0.6 * 118.0, "{}: a lone pair is not on a short stick", m.name);
                    for b in &r.bonds {
                        assert!(point_seg(l.p, b.a, b.b) > 4.0, "{} at {turn}/{tilt}: a lone-pair stick is on a bond", m.name);
                        for dot in l.dots {
                            assert!(point_seg(dot, b.a, b.b) > 2.0, "{} at {turn}/{tilt}: a lone dot is on a bond", m.name);
                        }
                    }
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 200, "{checked} pairs checked");
    // The two named views from the critic: XeF4 face-on and SF4 down-x.
    for (name, aturn, atilt) in [("XeF4", 0.0, 0.0), ("SF4", 90.0, 0.0)] {
        let m = molecules().iter().find(|m| m.name == name).unwrap();
        let (turn, tilt) = rel(m, aturn, atilt);
        let r = key(&[s("molecule", name), f("turn", turn), f("tilt", tilt)]);
        assert!(r.lone.is_empty(), "{name} at {aturn}/{atilt}: a collapsed pair is still drawn");
        assert!(r.view_notes.iter().any(|n| n.contains("not drawn")), "{name} at {aturn}/{atilt}: {r:?}", r = r.view_notes);
    }
    // The words match the drawing, and the old false caption is gone.
    let src = include_str!("../vsepr.rs");
    let footer = key(&[s("molecule", "H2O")]).footer.join(" ");
    assert!(src.contains("short dashed stick"), "the header/hint lacks the stick wording");
    assert!(src.contains("behind a nearer atom") && src.contains("onto another pair"), "the omission texts do not name the behind/overlap cases");
    assert!(footer.contains("short dashed stick") && footer.contains("central atom"), "{footer}");
    assert!(!footer.contains("two dots on the central atom"), "{footer}");
    assert!(!footer.contains("ligand atoms"), "{footer}");
    let faq = crate::figures::faq::faq_for(ID).iter().map(|f| format!("{} {}", f.q, f.a)).collect::<Vec<_>>().join(" ");
    assert!(faq.contains("short dashed stick"), "the FAQ lacks the stick wording");
}

#[test]
fn key_and_student_copies_have_the_same_height_at_every_size() {
    for m in molecules() {
        for size in ["Small", "Medium", "Large"] {
            let k = svg_of(&[s("molecule", m.name), s("label_size", size), b(SHOW_ANSWERS, true)]);
            let st = svg_of(&[s("molecule", m.name), s("label_size", size), b(SHOW_ANSWERS, false)]);
            let (kw, kh) = svg_size(&k);
            let (sw, sh) = svg_size(&st);
            assert_eq!((kw, kh), (sw, sh), "{} at {size}: key {kw}x{kh}, student {sw}x{sh}", m.name);
        }
    }
}

#[test]
fn the_66_percent_floor_holds_at_every_size_and_the_centre_has_a_mark() {
    for m in molecules() {
        for size in ["Small", "Medium", "Large"] {
            let svg = svg_of(&[s("molecule", m.name), s("label_size", size), s("style", "Print")]);
            let small = texts(&svg).iter().map(|t| t.fs).fold(f64::INFINITY, f64::min);
            assert!(small * 0.66 >= 8.4, "{} at {size}: the smallest type is {small} px ({:.2} at 66 %)", m.name, small * 0.66);
            let g = geo(&svg);
            let central = g.circles.iter().find(|c| c.feat == "a0").unwrap();
            let ligand = g.circles.iter().find(|c| c.feat == "a1").unwrap();
            assert_eq!(central.fill, "url(#vs-hatch2)", "{}: the central atom is a plain wash", m.name);
            assert_eq!(ligand.fill, "url(#vs-hatch3)", "{}", m.name);
            if matches!(m.name, "H2O" | "SF6" | "SF4") {
                let png = to_png(&svg, 0.66).unwrap();
                let img = image::load_from_memory(&png).unwrap().to_rgba8();
                let mut dark = 0;
                for px in img.pixels() {
                    assert!(px[0] == px[1] && px[1] == px[2], "{} at {size}: colour in Print", m.name);
                    if px[0] < 128 { dark += 1; }
                }
                assert!(dark > 100, "{} at {size}: the 66 % copy is nearly blank ({dark})", m.name);
            }
        }
    }
}

#[test]
fn edge_on_views_say_what_is_hidden() {
    for (name, aturn, atilt) in [("SF6", 90.0, 0.0), ("BeCl2", 90.0, 0.0), ("BF3", 90.0, 0.0), ("XeF4", 90.0, 0.0)] {
        let m = molecules().iter().find(|m| m.name == name).unwrap();
        let (turn, tilt) = rel(m, aturn, atilt);
        let r = key(&[s("molecule", name), f("turn", turn), f("tilt", tilt)]);
        assert!(!r.view_notes.is_empty(), "{name} at {aturn}/{atilt}: no view note");
        let panel = r.panel.join(" ");
        assert!(panel.contains("line of sight") || panel.contains("straight at") || panel.contains("straight away") || panel.contains("overlap"), "{name}: {panel}");
        assert!(r.bonds.iter().all(|b| b.drawn), "{name}: a bond was dropped");
        let (turn, tilt) = rel(m, aturn, atilt);
        let g = geo(&svg_of(&[s("molecule", name), f("turn", turn), f("tilt", tilt)]));
        let bfeats: BTreeSet<&str> = g.lines.iter().filter(|l| l.kind == "bond").map(|l| l.feat.as_str()).collect();
        assert_eq!(bfeats.len(), m.atoms.len() - 1, "{name}: a bond element is missing");
    }
}


#[test]
fn lone_pair_stick_is_the_projected_direction_or_is_omitted() {
    let scale = 118.0;
    let mut checked = 0;
    for m in molecules().iter().filter(|m| !m.lone.is_empty()) {
        for rel_turn in (0..360).step_by(30) {
            for rel_tilt in (-90..=90).step_by(30) {
                let r = key(&[s("molecule", m.name), f("turn", rel_turn as f64), f("tilt", rel_tilt as f64)]);
                let centre = r.atoms[0].p;
                let view = View::new(r.view.0, r.view.1);
                for (k, d) in m.lone.iter().enumerate() {
                    let (pp, _) = view.screen(*d, centre, scale);
                    let v = (pp.0 - centre.0, pp.1 - centre.1);
                    let l = (v.0 * v.0 + v.1 * v.1).sqrt();
                    let drawn = r.lone.iter().find(|x| x.pair == k);
                    if let Some(x) = drawn {
                        let u = (x.p.0 - centre.0, x.p.1 - centre.1);
                        let ul = (u.0 * u.0 + u.1 * u.1).sqrt();
                        assert!(l > 0.12 * scale, "{} at {rel_turn}/{rel_tilt}: a collapsed pair is drawn", m.name);
                        let drawn_dir = screen_angle((0.0, 0.0), u);
                        let true_dir = screen_angle((0.0, 0.0), v);
                        assert!(ang_diff(drawn_dir, true_dir) <= 2.0, "{} at {rel_turn}/{rel_tilt}: pair {k} is {:.1}° off the projected 3-D direction", m.name, ang_diff(drawn_dir, true_dir));
                        assert!(ul > 0.12 * scale);
                        for b in &r.bonds {
                            assert!(seg_dist(x.p, b.a, b.b) > 2.0, "{} at {rel_turn}/{rel_tilt}: a drawn pair is on a bond", m.name);
                            for dot in x.dots { assert!(seg_dist(dot, b.a, b.b) > 1.0, "{} at {rel_turn}/{rel_tilt}: a drawn dot is on a bond", m.name); }
                        }
                        for atom in r.atoms.iter().skip(1) {
                            if atom.depth > x.depth + 1e-9 {
                                for dot in x.dots {
                                    assert!(dist(dot, atom.p) >= atom.r, "{} at {rel_turn}/{rel_tilt}: a dot is inside a nearer ball", m.name);
                                }
                            }
                        }
                        checked += 1;
                    } else {
                        assert!(r.view_notes.iter().any(|n| n.contains("not drawn")), "{} at {rel_turn}/{rel_tilt}: pair {k} vanished without a note", m.name);
                    }
                }
            }
        }
    }
    assert!(checked > 200, "{checked} drawn pairs");
    // The XeF4 / SF4 repros: no invented direction.
    for (name, aturn, atilt, word) in [("XeF4", 0.0, 0.0, "straight at or away"), ("XeF4", 0.0, 20.0, "along bonds"), ("SF4", 90.0, 0.0, "straight at or away")] {
        let m = molecules().iter().find(|m| m.name == name).unwrap();
        let (turn, tilt) = rel(m, aturn, atilt);
        let r = key(&[s("molecule", name), f("turn", turn), f("tilt", tilt)]);
        assert!(r.lone.is_empty(), "{name} at {aturn}/{atilt}: a pair with no true projected direction is drawn");
        let notes = r.view_notes.join(" ");
        assert!(notes.contains("not drawn") && notes.contains(word), "{name} at {aturn}/{atilt}: {notes}");
    }
    // A clear view draws both XeF4 pairs along their true projections.
    let m = molecules().iter().find(|m| m.name == "XeF4").unwrap();
    let (turn, tilt) = rel(m, 30.0, 20.0);
    let r = key(&[s("molecule", "XeF4"), f("turn", turn), f("tilt", tilt)]);
    assert_eq!(r.lone.len(), 2);
    let centre = r.atoms[0].p;
    let view = View::new(r.view.0, r.view.1);
    for (k, d) in m.lone.iter().enumerate() {
        let (pp, _) = view.screen(*d, centre, 118.0);
        let x = r.lone.iter().find(|x| x.pair == k).expect("pair");
        assert!(ang_diff(screen_angle((0.0, 0.0), (x.p.0 - centre.0, x.p.1 - centre.1)), screen_angle((0.0, 0.0), (pp.0 - centre.0, pp.1 - centre.1))) <= 2.0);
    }
}

#[test]
fn lone_dots_are_depth_sorted_with_every_ball() {
    let mut dots = 0;
    for m in molecules().iter().filter(|m| !m.lone.is_empty()) {
        for rel_turn in (0..360).step_by(30) {
            for rel_tilt in (-90..=90).step_by(30) {
                let svg = svg_of(&[s("molecule", m.name), f("turn", rel_turn as f64), f("tilt", rel_tilt as f64), s("label_size", "Large")]);
                let g = geo(&svg);
                let items: Vec<&Circ> = g.circles.iter().filter(|c| c.kind == "atom" || c.kind == "lone").collect();
                for a in 1..items.len() {
                    assert!(items[a - 1].depth <= items[a].depth + 1e-9, "{} at {rel_turn}/{rel_tilt}: a farther item is drawn after a nearer one", m.name);
                }
                for c in &items { if c.kind == "lone" { dots += 1; } }
            }
        }
    }
    assert!(dots > 50, "{dots} dots");
    // The critic's Large near view: the true pairs are on the F bonds, so they are omitted, not
    // left to be painted over by a farther ball.
    let m = molecules().iter().find(|m| m.name == "XeF4").unwrap();
    let (turn, tilt) = rel(m, 0.0, 80.0);
    let r = key(&[s("molecule", "XeF4"), f("turn", turn), f("tilt", tilt), s("label_size", "Large")]);
    assert!(r.lone.is_empty(), "a pair that the true projection puts on a bond is still drawn");
    assert!(r.view_notes.iter().any(|n| n.contains("not drawn")), "{:?}", r.view_notes);
}

#[test]
fn a_near_ligand_says_which_atom_hides_the_centre() {
    let m = molecules().iter().find(|m| m.name == "SF6").unwrap();
    let (turn, tilt) = rel(m, 0.0, -1.0);
    let r = key(&[s("molecule", "SF6"), f("turn", turn), f("tilt", tilt)]);
    let centre = r.atoms[0].p;
    assert!(r.atoms.iter().skip(1).any(|a| !a.hidden && dist(a.p, centre) < r.atoms[0].r), "no ligand is inside the centre's radius at this view");
    let notes = r.view_notes.join(" ");
    assert!(notes.contains("lies over the centre") && notes.contains("hides the S atom"), "{notes}");
    assert!(notes.contains("overlap") && notes.contains("hides the farther"), "the far axial F is not explained: {notes}");
    assert!(r.panel.join(" ").contains("lies over the centre"), "{:?}", r.panel);
    // Either the centre label is kept on top, or the note explains why it is not.
    assert!(r.atoms[0].labelled || notes.contains("hides the S atom"), "{notes}");
}

#[test]
fn hidden_atom_note_says_at_or_away_from_the_viewer() {
    let bf = molecules().iter().find(|m| m.name == "BF3").unwrap();
    let (turn, tilt) = rel(bf, 0.0, -90.0);
    let away = key(&[s("molecule", "BF3"), f("turn", turn), f("tilt", tilt)]);
    assert!(away.view_notes.join(" ").contains("straight away"), "{:?}", away.view_notes);
    assert!(!away.view_notes.join(" ").contains("straight at the viewer"), "{:?}", away.view_notes);
    let (turn, tilt) = rel(bf, 0.0, 90.0);
    let toward = key(&[s("molecule", "BF3"), f("turn", turn), f("tilt", tilt)]);
    assert!(toward.view_notes.join(" ").contains("straight at the viewer"), "{:?}", toward.view_notes);
    assert!(!toward.view_notes.join(" ").contains("straight away"), "{:?}", toward.view_notes);
    for (name, aturn, atilt) in [("BeCl2", 90.0, 0.0), ("SF6", 90.0, 0.0)] {
        let m = molecules().iter().find(|m| m.name == name).unwrap();
        let (turn, tilt) = rel(m, aturn, atilt);
        let r = key(&[s("molecule", name), f("turn", turn), f("tilt", tilt)]);
        assert!(r.view_notes.join(" ").contains("straight at and away"), "{name}: {:?}", r.view_notes);
    }
}

#[test]
fn two_lone_pairs_do_not_project_on_top_of_each_other() {
    for m in molecules().iter().filter(|m| m.lone.len() >= 2) {
        for rel_turn in (0..360).step_by(45) {
            for rel_tilt in (-90..=90).step_by(30) {
                let r = key(&[s("molecule", m.name), f("turn", rel_turn as f64), f("tilt", rel_tilt as f64)]);
                if r.lone.len() == 2 {
                    let mut best = f64::INFINITY;
                    for a in &r.lone[0].dots { for b in &r.lone[1].dots { best = best.min(dist(*a, *b)); } }
                    assert!(best > 4.5, "{} at {rel_turn}/{rel_tilt}: pair dots {best:.1} px apart", m.name);
                } else if r.lone.len() < 2 {
                    assert!(r.view_notes.iter().any(|n| n.contains("not drawn")), "{} at {rel_turn}/{rel_tilt}: a pair disappeared without a note", m.name);
                }
            }
        }
    }
    // A near-face-on H2O view brings the two pairs closest; one must be left off with a note
    // rather than drawn as a four-dot blob.
    let h = molecules().iter().find(|m| m.name == "H2O").unwrap();
    let (turn, tilt) = rel(h, 44.0, 82.0);
    let r = key(&[s("molecule", "H2O"), f("turn", turn), f("tilt", tilt)]);
    assert_eq!(r.lone.len(), 1, "the close H2O pairs were both drawn");
    assert!(r.view_notes.iter().any(|n| n.contains("top of each other")), "{:?}", r.view_notes);
}

#[test]
fn sixty_six_percent_keeps_every_bond_and_arc_dark() {
    fn dark_near(img: &image::RgbaImage, x: f64, y: f64) -> bool {
        let (cx, cy) = (x.round() as i32, y.round() as i32);
        for dy in -2..=2 {
            for dx in -2..=2 {
                let (px, py) = (cx + dx, cy + dy);
                if px >= 0 && py >= 0 && (px as u32) < img.width() && (py as u32) < img.height() {
                    if img.get_pixel(px as u32, py as u32)[0] < 128 { return true; }
                }
            }
        }
        false
    }
    let views = [("opening", 0.0, 0.0), ("turn90", 90.0, 0.0), ("oblique", 35.0, -30.0), ("tilt60", 0.0, 60.0), ("back", 180.0, 45.0)];
    for m in molecules() {
        for (tag, rt, ti) in views {
            let (turn, tilt) = rel(m, rt, ti);
            let (svg, r) = build_of(&[s("molecule", m.name), f("turn", turn), f("tilt", tilt), s("label_size", "Small"), s("style", "Print")]);
            let (w, h) = svg_size(&svg);
            let png = to_png(&svg, 0.66).unwrap();
            let img = image::load_from_memory(&png).unwrap().to_rgba8();
            let (fx, fy) = (img.width() as f64 / w, img.height() as f64 / h);
            for b in &r.bonds {
                if dist(b.a, b.b) < 4.0 { continue; }
                for j in 1..=12 {
                    let t = 0.2 + 0.6 * j as f64 / 12.0;
                    let local = b.da + (b.db - b.da) * t;
                    let p = (b.a.0 + (b.b.0 - b.a.0) * t, b.a.1 + (b.b.1 - b.a.1) * t);
                    // A sample inside a nearer ball is not a visible stick.
                    if r.atoms.iter().any(|a| a.index != 0 && !a.hidden && a.depth > local + 1e-9 && dist(p, a.p) < a.r) { continue; }
                    assert!(dark_near(&img, p.0 * fx, p.1 * fy), "{} at {tag}: bond b{} has no dark pixel at {t:.2} of its length", m.name, b.index);
                }
            }
            if tag == "opening" {
                for a in &r.angles {
                    for s in a.arc.windows(2) {
                        let x = ((s[0].0 + s[1].0) / 2.0) * fx;
                        let y = ((s[0].1 + s[1].1) / 2.0) * fy;
                        assert!(dark_near(&img, x, y), "{}: the opening angle arc has no dark pixel at a segment", m.name);
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
struct Drawn {
    kind: String,
    feat: String,
    depth: f64,
    mid: Pt,
    r: f64,
}

fn drawn_items(svg: &str) -> Vec<Drawn> {
    let mut out = Vec::new();
    for part in svg.split('<').skip(1) {
        let tag = part.split('>').next().unwrap_or("");
        let kind = attr(tag, "data-o").unwrap_or_default();
        if !matches!(kind.as_str(), "bond" | "loneguide" | "atom" | "lone") { continue; }
        let Some(depth) = attr_f(tag, "data-d") else { continue };
        let feat = attr(tag, "data-f").unwrap_or_default();
        if tag.starts_with("line ") {
            let a = (attr_f(tag, "x1").unwrap_or(0.0), attr_f(tag, "y1").unwrap_or(0.0));
            let b = (attr_f(tag, "x2").unwrap_or(0.0), attr_f(tag, "y2").unwrap_or(0.0));
            out.push(Drawn { kind, feat, depth, mid: ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0), r: 0.0 });
        } else if tag.starts_with("circle ") {
            let c = (attr_f(tag, "cx").unwrap_or(0.0), attr_f(tag, "cy").unwrap_or(0.0));
            let r = attr_f(tag, "r").unwrap_or(0.0);
            out.push(Drawn { kind, feat, depth, mid: c, r });
        }
    }
    out
}

#[test]
fn sticks_balls_and_dots_are_one_depth_order() {
    let mut checked = 0usize;
    for m in molecules() {
        for rel_turn in (0..360).step_by(30) {
            for rel_tilt in (-90..=90).step_by(30) {
                let svg = svg_of(&[s("molecule", m.name), f("turn", rel_turn as f64), f("tilt", rel_tilt as f64)]);
                let items = drawn_items(&svg);
                for i in 1..items.len() {
                    assert!(items[i - 1].depth <= items[i].depth + 1e-9, "{} at {rel_turn}/{rel_tilt}: depth order {} after {}", m.name, items[i].depth, items[i - 1].depth);
                }
                for (si, it) in items.iter().enumerate() {
                    if it.kind != "bond" && it.kind != "loneguide" { continue; }
                    for (ai, a) in items.iter().enumerate() {
                        if a.kind == "atom" && a.depth < it.depth - 1e-9 && dist(it.mid, a.mid) < a.r {
                            assert!(ai < si, "{} at {rel_turn}/{rel_tilt}: a farther ball is drawn after a nearer stick", m.name);
                        }
                    }
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 500, "{checked} stick pieces");
    // The critic's SF6 default: the near +y stick b3 is crossed by the far axial F a6 and must be
    // drawn over it.
    let sf = svg_of(&[s("molecule", "SF6")]);
    let items = drawn_items(&sf);
    let a6 = items.iter().position(|it| it.kind == "atom" && it.feat == "a6").expect("the far axial F");
    let a = &items[a6];
    let mut crossed = 0;
    for (si, it) in items.iter().enumerate() {
        if it.kind == "bond" && it.feat == "b3" && dist(it.mid, a.mid) < a.r && it.depth > a.depth + 1e-9 {
            assert!(a6 < si, "the far axial F is drawn after the near +y stick");
            crossed += 1;
        }
    }
    assert!(crossed > 0, "the SF6 default no longer has the near stick crossing the far F");
}

#[test]
fn collinear_note_only_for_real_overlaps() {
    let sf = key(&[s("molecule", "SF6")]); // the control default, abs (0, 20)
    let notes = sf.view_notes.join(" ");
    assert!(!notes.contains("line of sight") && !notes.contains("behind the other") && !notes.contains("overlap"), "SF6's default claims a hiding that is not there: {notes}");
    let pa = sf.atoms.iter().find(|a| a.index == 3).unwrap().p;
    let pb = sf.atoms.iter().find(|a| a.index == 6).unwrap().p;
    assert!(dist(pa, pb) > sf.atoms[3].r + sf.atoms[6].r, "the two axial F atoms are far apart");
    let bf = key(&[s("molecule", "BF3"), f("turn", 90.0), f("tilt", 0.0)]);
    let notes = bf.view_notes.join(" ");
    assert!(notes.contains("overlap") && notes.contains("F") && notes.contains("hides the farther"), "{notes}");
    assert!(bf.panel.join(" ").contains("overlap"), "{:?}", bf.panel);
}

#[test]
fn a_flat_180_arc_is_left_off() {
    let m = molecules().iter().find(|m| m.name == "BeCl2").unwrap();
    let (turn, tilt) = rel(m, 0.0, 90.0);
    let r = key(&[s("molecule", "BeCl2"), f("turn", turn), f("tilt", tilt)]);
    assert!(r.angles.is_empty(), "an edge-on 180° arc is still drawn as a line");
    let svg = svg_of(&[s("molecule", "BeCl2"), f("turn", turn), f("tilt", tilt)]);
    assert_eq!(geo(&svg).arcs.iter().filter(|a| a.kind == "anglearc").count(), 0);
    assert!(r.panel.join(" ").contains("180"), "the 180° value is still named");
    let opening = key(&[s("molecule", "BeCl2")]);
    assert!(!opening.angles.is_empty(), "the opening 180° arc disappeared");
}

#[test]
fn lone_pairs_behind_a_nearer_ball_are_noted() {
    let m = molecules().iter().find(|m| m.name == "XeF4").unwrap();
    let mut noted = 0;
    for (aturn, atilt) in [(20.0, 75.0), (20.0, -75.0), (20.0, 80.0), (20.0, -80.0)] {
        let (turn, tilt) = rel(m, aturn, atilt);
        let r = key(&[s("molecule", "XeF4"), f("turn", turn), f("tilt", tilt)]);
        for l in &r.lone {
            for atom in r.atoms.iter().skip(1) {
                if atom.depth > l.depth + 1e-9 {
                    for dot in l.dots { assert!(dist(dot, atom.p) >= atom.r, "XeF4 at {aturn}/{atilt}: a drawn dot is inside a nearer ball"); }
                }
            }
        }
        let notes = r.view_notes.join(" ");
        if notes.contains("behind a ligand atom") {
            noted += 1;
            assert!(r.lone.len() < m.lone.len(), "the hidden pair is still drawn");
            assert!(r.panel.join(" ").contains("not drawn"), "{:?}", r.panel);
        }
    }
    assert!(noted > 0, "no XeF4 view left a behind-a-ball pair off with a note");
}

