//! Nuclide notation, batch 7: old figures frozen, and the isotope list, particle-count input,
//! positron, particle symbol forms, hyphen-only and short hyphen forms and serif type checked
//! against nuclear facts and against what is drawn.
use super::*;
use crate::figures::{registry, Value};

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).expect("nuclide-notation is registered") }
fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.into())) }
fn i(k: &'static str, v: i64) -> (&'static str, Value) { (k, Value::I(v)) }
fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
/// The opening state with `list` set on top.
fn with_list(list: &[(&'static str, Value)]) -> Params {
    let d = def();
    let mut p = crate::figures::start_params(&d);
    for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&(d.params)(), false);
    p
}
/// FNV-1a, so the hashes in the fixture are the same in any build.
fn fnv(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() { h ^= *b as u64; h = h.wrapping_mul(0x100000001b3); }
    format!("{h:016x}")
}

/// Every old choice alone and on a few bases (an ion with a two-letter symbol, a heavy element at
/// Large, a particle, hyphen and both notations with blanks): the figures saved and shared before
/// batch 7, written once from batch7-base 47e760a with VERSHION_NUCLIDE_OLD_BLESS=1.
fn old_cases() -> Vec<Vec<(&'static str, Value)>> {
    let mut cases: Vec<Vec<(&'static str, Value)>> = vec![vec![]];
    let mut opts: Vec<(&'static str, Value)> = vec![];
    for k in KINDS { opts.push(s("kind", k)); }
    for e in ["1 H - Hydrogen", "8 O - Oxygen", "17 Cl - Chlorine", "26 Fe - Iron", "74 W - Tungsten", "92 U - Uranium", "118 Og - Oganesson"] { opts.push(s("element", e)); }
    for m in [1, 14, 37, 235, 300] { opts.push(i("mass", m)); }
    for c in [-8, -3, -1, 1, 2, 3, 8] { opts.push(i("charge", c)); }
    for key in ["pos_mass", "pos_z", "pos_symbol", "pos_charge"] {
        for v in POSITIONS { opts.push(s(key, v)); }
    }
    for v in BLANKS { opts.push(s("blanks", v)); }
    for v in NOTATIONS { opts.push(s("notation", v)); }
    for v in ["Large", "Small"] { opts.push(s("label_size", v)); }
    opts.extend([s("symbol", "Qz"), s("symbol", "Xyz1"), s("title", "Carbon-14"), b("show_answers", false)]);
    let bases: Vec<Vec<(&'static str, Value)>> = vec![
        vec![s("element", "17 Cl - Chlorine"), i("mass", 37), i("charge", -1)],
        vec![s("element", "82 Pb - Lead"), i("mass", 208), i("charge", 2), s("label_size", "Large")],
        vec![s("kind", "Custom symbol"), s("symbol", "Qz"), i("charge", 3)],
        vec![s("kind", "Beta particle")],
        vec![s("notation", "Hyphen notation"), s("pos_mass", "Blank"), i("charge", 2)],
        vec![s("notation", "Both"), s("pos_symbol", "Blank"), s("blanks", "Line"), i("charge", -2)],
    ];
    for o in &opts {
        cases.push(vec![o.clone()]);
        for base in &bases {
            let mut c = base.clone();
            c.push(o.clone());
            cases.push(c);
        }
    }
    cases
}

/// The corrections batch 7 made to old figures (137 of the 344 cases; their hashes were renewed
/// once with VERSHION_NUCLIDE_OLD_RENEW=1, their specs and links kept as they were):
/// - a beta particle drew no bottom number (⁰e¹⁻); it is ⁰₋₁e, its bottom number its charge, as the
///   FAQ and decay equations write it (46 cases);
/// - "Hyphen notation" with the Symbol Off drew the symbol and numbers as "Both" did; it now draws
///   the hyphen form alone, the name left off;
/// - a hyphen line with a blank was placed by width guesses, so the gaps either side of a box were
///   uneven and " ion, " lost its spaces (a box touched "ion"); it is now laid out by the drawn
///   face's advance widths with an equal gap each side (hyphen-only and "Both" lines);
/// - a blank symbol's box (90 px wide) had the numbers at 34 px from its centre, so they ran into
///   the box; they now stand 6 px clear of it.
fn intended_change(p: &Params) -> bool {
    let hyphen_blank = p.s("notation") != "Symbol"
        && (p.s("pos_mass") == "Blank" || p.s("pos_symbol") == "Blank" || (p.s("notation") == "Hyphen notation" && p.s("pos_charge") == "Blank"));
    let blank_symbol = p.s("notation") != "Hyphen notation" && p.s("pos_symbol") == "Blank";
    p.s("kind") == "Beta particle" || (p.s("notation") == "Hyphen notation" && p.s("pos_symbol") == "Off") || hyphen_blank || blank_symbol
}

fn old_fixture() ->std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/nuclide_old.txt") }

#[test]
fn old_saved_figures_and_links_render_as_before() {
    use crate::figures::{share, spec, start_params};
    let d = def();
    if std::env::var("VERSHION_NUCLIDE_OLD_BLESS").is_ok_and(|v| v == "1") {
        let mut out = String::new();
        for c in old_cases() {
            let p = with_list(&c);
            let saved = serde_json::to_string(&spec::spec_for(ID, &p)).unwrap();
            let link = share::encode(ID, &p, &start_params(&d));
            out.push_str(&format!("{}\t{saved}\t{link}\n", fnv(&d.svg(&p))));
        }
        std::fs::write(old_fixture(), out).unwrap();
        return;
    }
    let text = std::fs::read_to_string(old_fixture()).expect("tests/fixtures/nuclide_old.txt");
    if std::env::var("VERSHION_NUCLIDE_OLD_RENEW").is_ok_and(|v| v == "1") {
        // renews the hashes of the intended corrections only, keeping every old spec and link as it was
        let mut out = String::new();
        for line in text.lines().filter(|l| !l.is_empty()) {
            let mut it = line.split('\t');
            let (hash, saved, link) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
            let sp: crate::model::FigureSpec = serde_json::from_str(saved).unwrap();
            let p = spec::params_from_spec(&d, &sp);
            let now = fnv(&d.svg(&p));
            if now != hash {
                assert!(intended_change(&p), "an unintended change: {saved}");
            }
            out.push_str(&format!("{now}\t{saved}\t{link}\n"));
        }
        std::fs::write(old_fixture(), out).unwrap();
        return;
    }
    let mut n = 0;
    let mut bad = vec![];
    for line in text.lines().filter(|l| !l.is_empty()) {
        let mut it = line.split('\t');
        let (hash, saved, link) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
        let sp: crate::model::FigureSpec = serde_json::from_str(saved).unwrap();
        let p = spec::params_from_spec(&d, &sp);
        if fnv(&d.svg(&p)) != hash { bad.push(format!("saved {saved}")); }
        let (id, changed) = share::decode(link).unwrap();
        assert_eq!(id, ID);
        let p = share::link_params(&d, changed, share::version(link));
        if fnv(&d.svg(&p)) != hash { bad.push(format!("link {link}")); }
        n += 1;
    }
    assert!(bad.is_empty(), "{} old figures draw differently:\n{}", bad.len(), bad.join("\n"));
    assert!(n >= 300, "only {n} old cases");
}

// ---------------------------------------------------------------------------------------------
// Reading what is drawn
// ---------------------------------------------------------------------------------------------

fn svg_of(list: &[(&'static str, Value)]) -> String { def().svg(&with_list(list)) }

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let k = format!(" {name}=\"");
    let i = tag.find(&k)? + k.len();
    tag[i..].split('"').next()
}
fn num(tag: &str, name: &str) -> f64 { attr(tag, name).and_then(|v| v.parse().ok()).unwrap_or(f64::NAN) }

/// Every drawn text: (body, font size, font family).
fn texts(svg: &str) -> Vec<(String, f64, String)> {
    svg.split("<text ").skip(1).map(|el| {
        let head = el.split('>').next().unwrap();
        let body = el[head.len() + 1..].split("</text>").next().unwrap().replace("&amp;", "&").replace("&#39;", "'").replace("&apos;", "'");
        (body, num(head, "font-size"), attr(head, "font-family").unwrap_or("").to_string())
    }).collect()
}
fn bodies(svg: &str) -> Vec<String> { texts(svg).into_iter().map(|t| t.0).collect() }

type Bx = (f64, f64, f64, f64);
/// The ink box of every text as resvg lays it out with the app's fonts, in order.
fn ink_boxes(svg: &str) -> Vec<(String, Bx)> {
    fn walk(g: &resvg::usvg::Group, out: &mut Vec<(String, Bx)>) {
        for n in g.children() {
            match n {
                resvg::usvg::Node::Group(g) => walk(g, out),
                resvg::usvg::Node::Text(t) => {
                    let r = t.flattened().abs_bounding_box();
                    let txt: String = t.chunks().iter().map(|c| c.text()).collect();
                    out.push((txt, (r.left() as f64, r.top() as f64, r.right() as f64, r.bottom() as f64)));
                }
                _ => {}
            }
        }
    }
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = crate::figures::fontdb();
    let tree = resvg::usvg::Tree::from_str(&crate::figures::serif_stack(svg), &opt).expect("the SVG parses");
    let mut v = Vec::new();
    walk(tree.root(), &mut v);
    v
}
/// The boxes and underlines drawn as blanks: (left, top, right, bottom).
fn blanks(svg: &str) -> Vec<Bx> {
    let mut v: Vec<Bx> = svg.split("<rect ").skip(1).map(|el| {
        let h = &format!(" {}", el.split("/>").next().unwrap());
        let (x, y, w, hh) = (num(h, "x"), num(h, "y"), num(h, "width"), num(h, "height"));
        (x, y, x + w, y + hh)
    }).filter(|b| b.2 - b.0 < 500.0).collect();
    v.extend(svg.split("<line ").skip(1).map(|el| {
        let h = format!(" {}", el.split("/>").next().unwrap());
        (num(&h, "x1"), num(&h, "y1") - 1.0, num(&h, "x2"), num(&h, "y2") + 1.0)
    }));
    v
}
fn overlap(a: Bx, b: Bx) -> bool { a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3 }

/// Text clear of other text and of the blanks, inside the page, never under 8 px as drawn.
fn assert_readable(svg: &str, what: &str) {
    let ink = ink_boxes(svg);
    let ts: Vec<(String, f64, String)> = texts(svg).into_iter().filter(|t| !t.0.is_empty()).collect();
    assert_eq!(ink.len(), ts.len(), "{what}: one laid-out text per text element");
    for (t, sz, _) in &ts {
        if !t.trim().is_empty() { assert!(*sz >= 8.0, "{what}: {t:?} at {sz} px"); }
    }
    let bl = blanks(svg);
    for (k, (t, b)) in ink.iter().enumerate() {
        if t.trim().is_empty() { continue; }
        assert!(b.0 >= 0.0 && b.2 <= 520.0 && b.1 >= 0.0 && b.3 <= 300.0, "{what}: {t:?} leaves the page at {b:?}");
        for (u, c) in ink.iter().skip(k + 1) {
            if u.trim().is_empty() { continue; }
            assert!(!overlap(*b, *c), "{what}: {t:?} {b:?} overlaps {u:?} {c:?}");
        }
        for r in &bl {
            assert!(!overlap(*b, *r), "{what}: {t:?} {b:?} runs into a blank {r:?}");
        }
    }
    for (k, r) in bl.iter().enumerate() {
        for q in bl.iter().skip(k + 1) {
            assert!(!overlap((r.0 - 1.0, r.1 - 1.0, r.2 + 1.0, r.3 + 1.0), *q), "{what}: blanks {r:?} and {q:?} touch");
        }
    }
    for r in &bl {
        assert!(r.0 >= 0.0 && r.2 <= 520.0 && r.1 >= 0.0 && r.3 <= 300.0, "{what}: a blank leaves the page at {r:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// cap-01c: the element's natural isotopes
// ---------------------------------------------------------------------------------------------

/// The isotope list holds each element's naturally occurring isotopes, checked by hand against the
/// IUPAC table (H to Ca, and the heavier teaching elements), and offers only the chosen element's.
#[test]
fn the_isotope_list_holds_the_real_natural_isotopes() {
    let offered = |el: &str| -> Vec<String> {
        let p = with_list(&[s("element", el)]);
        isotope_choices().iter().filter(|c| **c != TYPED && isotope_offered(c, &p)).map(|c| c.to_string()).collect()
    };
    for (el, want) in [
        ("1 H - Hydrogen", "H-1 H-2"), ("2 He - Helium", "He-3 He-4"), ("3 Li - Lithium", "Li-6 Li-7"), ("4 Be - Beryllium", "Be-9"),
        ("5 B - Boron", "B-10 B-11"), ("6 C - Carbon", "C-12 C-13"), ("7 N - Nitrogen", "N-14 N-15"), ("8 O - Oxygen", "O-16 O-17 O-18"),
        ("9 F - Fluorine", "F-19"), ("10 Ne - Neon", "Ne-20 Ne-21 Ne-22"), ("11 Na - Sodium", "Na-23"), ("12 Mg - Magnesium", "Mg-24 Mg-25 Mg-26"),
        ("13 Al - Aluminum", "Al-27"), ("14 Si - Silicon", "Si-28 Si-29 Si-30"), ("15 P - Phosphorus", "P-31"), ("16 S - Sulfur", "S-32 S-33 S-34 S-36"),
        ("17 Cl - Chlorine", "Cl-35 Cl-37"), ("18 Ar - Argon", "Ar-36 Ar-38 Ar-40"), ("19 K - Potassium", "K-39 K-40 K-41"),
        ("20 Ca - Calcium", "Ca-40 Ca-42 Ca-43 Ca-44 Ca-46 Ca-48"), ("26 Fe - Iron", "Fe-54 Fe-56 Fe-57 Fe-58"), ("29 Cu - Copper", "Cu-63 Cu-65"),
        ("35 Br - Bromine", "Br-79 Br-81"), ("47 Ag - Silver", "Ag-107 Ag-109"), ("53 I - Iodine", "I-127"), ("55 Cs - Cesium", "Cs-133"),
        ("82 Pb - Lead", "Pb-204 Pb-206 Pb-207 Pb-208"), ("83 Bi - Bismuth", "Bi-209"), ("90 Th - Thorium", "Th-232"), ("92 U - Uranium", "U-234 U-235 U-238"),
    ] {
        assert_eq!(offered(el).join(" "), want, "{el}");
    }
    // technetium and promethium have no stable or primordial isotope: no list
    for el in ["43 Tc - Technetium", "61 Pm - Promethium", "118 Og - Oganesson"] {
        assert!(offered(el).is_empty(), "{el}");
        assert!(!relevant("isotope", &with_list(&[s("element", el)])), "{el}");
    }
    // each element's abundances add up to 100%
    for (z, sym, list) in natural_isotopes() {
        let sum: f64 = list.iter().map(|i| i.1).sum();
        assert!((sum - 100.0).abs() < 0.1, "{sym} (Z = {z}) adds to {sum}");
        assert_eq!(data::symbol(z), sym);
    }
    assert_eq!(isotope_label("C-13"), "Carbon-13 (1.07%)");
    assert_eq!(isotope_label("U-235"), "Uranium-235 (0.7204%)");
}

/// Picking an isotope draws its mass number; an isotope of another element is not used; the panel
/// keeps the list and the mass number box in step (through the app's own after-change hook).
#[test]
fn picking_an_isotope_sets_the_mass_number() {
    let c13 = svg_of(&[s("isotope", "C-13")]);
    assert!(bodies(&c13).contains(&"13".to_string()) && !bodies(&c13).contains(&"12".to_string()), "{c13}");
    assert_ne!(c13, svg_of(&[]));
    assert_eq!(c13, svg_of(&[i("mass", 13)]), "the isotope draws as the typed mass number does");
    // an isotope left over from another element is not drawn
    assert_eq!(svg_of(&[s("element", "8 O - Oxygen"), s("isotope", "C-13")]), svg_of(&[s("element", "8 O - Oxygen")]));
    let d = def();
    let mut p = with_list(&[]);
    let old = Value::S(TYPED.into());
    p.set_s("isotope", "C-13");
    crate::figures::after_panel_change(ID, "isotope", &old, &mut p);
    assert_eq!(p.i("mass"), 13);
    // a new element picks its main isotope
    let old = p.0["element"].clone();
    p.set_s("element", "17 Cl - Chlorine");
    crate::figures::after_panel_change(ID, "element", &old, &mut p);
    assert_eq!((p.s("isotope"), p.i("mass")), ("Cl-35", 35));
    assert!(bodies(&d.svg(&p)).contains(&"35".to_string()));
    // a typed mass number takes the list back to Typed
    let old = p.0["mass"].clone();
    p.set_i("mass", 36);
    crate::figures::after_panel_change(ID, "mass", &old, &mut p);
    assert_eq!(p.s("isotope"), TYPED);
    assert!(bodies(&d.svg(&p)).contains(&"36".to_string()));
    // the list is offered only for an element set by its numbers
    assert!(relevant("isotope", &with_list(&[])));
    assert!(!relevant("isotope", &with_list(&[s("kind", "Alpha particle")])));
    assert!(!relevant("isotope", &with_list(&[s("set_by", "Particles")])));
    // every isotope of every element draws its own mass number, clear and on the page, at Large too
    for (z, sym, list) in natural_isotopes() {
        for (a, _) in list {
            let el = ELEMENT_CHOICES[z - 1];
            let choice = format!("{sym}-{a}");
            let svg = svg_of(&[s("element", el), s("isotope", &choice), s("label_size", "Large"), i("charge", -3)]);
            assert!(bodies(&svg).contains(&a.to_string()), "{choice}");
            assert_readable(&svg, &choice);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// cap-01d: protons, neutrons and electrons
// ---------------------------------------------------------------------------------------------

/// Typed particles draw the element, mass number and charge they make: 17 p, 20 n, 18 e is ³⁷₁₇Cl⁻,
/// byte for byte as the numbers draw it; 26 p, 30 n, 23 e is ⁵⁶₂₆Fe³⁺; 1 p, 0 n, 0 e is ¹₁H⁺.
#[test]
fn protons_neutrons_and_electrons_set_the_nuclide() {
    let parts = |pr: i64, n: i64, e: i64| -> Vec<(&'static str, Value)> { vec![s("set_by", "Particles"), i("protons", pr), i("neutrons", n), i("electrons", e)] };
    assert_eq!(svg_of(&parts(17, 20, 18)), svg_of(&[s("element", "17 Cl - Chlorine"), i("mass", 37), i("charge", -1)]));
    assert_eq!(svg_of(&parts(26, 30, 23)), svg_of(&[s("element", "26 Fe - Iron"), i("mass", 56), i("charge", 3)]));
    assert_eq!(svg_of(&parts(1, 0, 0)), svg_of(&[s("element", "1 H - Hydrogen"), i("mass", 1), i("charge", 1)]));
    let fe = bodies(&svg_of(&parts(26, 30, 23)));
    for t in ["56", "26", "Fe", "3+", "Iron: 26 protons, 30 neutrons, 23 electrons"] {
        assert!(fe.contains(&t.to_string()), "{t} in {fe:?}");
    }
    // the default counts draw the opening figure, so only a typed count changes it
    assert_eq!(svg_of(&[s("set_by", "Particles")]), svg_of(&[]));
    assert_ne!(svg_of(&parts(8, 10, 10)), svg_of(&[]));
    // the extremes the boxes allow: Og with 182 neutrons (mass 300), every charge to 8− and 8+
    let og = svg_of(&parts(118, 182, 126));
    assert!(bodies(&og).contains(&"300".to_string()) && bodies(&og).contains(&"8\u{2212}".to_string()));
    assert_readable(&og, "Og-300 8-");
    assert_readable(&def().svg(&{ let mut p = with_list(&parts(118, 182, 110)); p.set_s("label_size", "Large"); p }), "Og-300 8+ Large");
    // what is not an element is refused with a note, drawn in the figure and given in the panel
    for (pr, e, why) in [(0, 0, "0 protons"), (119, 119, "119 protons"), (150, 150, "150 protons"), (8, 17, "9\u{2212}"), (26, 0, "26+"), (20, 11, "9+")] {
        let p = with_list(&parts(pr, 10, e));
        let svg = def().svg(&p);
        let note = bodies(&svg).join(" ");
        assert!(note.contains("Nothing is drawn") && note.contains(why), "{pr} p, {e} e: {note}");
        assert!(!svg.contains("font-size=\"82\""), "no symbol is drawn for {pr} p, {e} e");
        assert!(crate::figures::notes_for(ID, &p).iter().any(|n| n.contains(why)), "the panel says why");
        assert_readable(&svg, &format!("refusal {pr} {e}"));
        let mut l = p.clone();
        l.set_s("label_size", "Large");
        assert_readable(&def().svg(&l), &format!("refusal {pr} {e} Large"));
    }
    // ±8 is still drawn; the boxes clamp what is typed past their ends
    assert!(refusal(&with_list(&parts(8, 8, 16))).is_none() && refusal(&with_list(&parts(16, 16, 8))).is_none());
    let p = with_list(&[s("set_by", "Particles"), i("protons", 1000), i("neutrons", 1000), i("electrons", -5)]);
    assert_eq!((p.i("protons"), p.i("neutrons"), p.i("electrons")), (150, 182, 0));
    // switching to particles fills the counts from the numbers, and back again
    let mut p = with_list(&[s("element", "16 S - Sulfur"), i("mass", 34), i("charge", -2)]);
    let before = def().svg(&p);
    p.set_s("set_by", "Particles");
    crate::figures::after_panel_change(ID, "set_by", &Value::S("Numbers".into()), &mut p);
    assert_eq!((p.i("protons"), p.i("neutrons"), p.i("electrons")), (16, 18, 18));
    assert_eq!(def().svg(&p), before);
    p.set_i("neutrons", 16);
    p.set_s("set_by", "Numbers");
    crate::figures::after_panel_change(ID, "set_by", &Value::S("Particles".into()), &mut p);
    assert_eq!((p.s("element"), p.i("mass"), p.i("charge")), ("16 S - Sulfur", 32, -2));
    assert!(relevant("protons", &p) == false && relevant("mass", &p));
}

// ---------------------------------------------------------------------------------------------
// cap-03c, cap-03g: the positron and the particles' symbol forms
// ---------------------------------------------------------------------------------------------

/// Each particle's numbers as drawn, checked by hand: (symbol, top left, bottom left, top right).
#[test]
fn particles_are_written_with_their_mass_and_charge_numbers() {
    let m = "\u{2212}";
    for (kind, form, want) in [
        ("Alpha particle", "He, e, p", vec!["He", "4", "2", "2+"]),
        ("Alpha particle", "\u{3b1}, \u{3b2}, H", vec!["\u{3b1}", "4", "2", "2+"]),
        ("Beta particle", "He, e, p", vec!["e", "0", "\u{2212}1", "1\u{2212}"]),
        ("Beta particle", "\u{3b1}, \u{3b2}, H", vec!["\u{3b2}", "0", "\u{2212}1", "1\u{2212}"]),
        ("Positron", "He, e, p", vec!["e", "0", "+1", "1+"]),
        ("Positron", "\u{3b1}, \u{3b2}, H", vec!["\u{3b2}", "0", "+1", "1+"]),
        ("Proton", "He, e, p", vec!["p", "1", "1", "1+"]),
        ("Proton", "\u{3b1}, \u{3b2}, H", vec!["H", "1", "1", "1+"]),
        ("Neutron", "He, e, p", vec!["n", "1", "0"]),
        ("Gamma ray", "He, e, p", vec!["\u{3b3}", "0", "0"]),
    ] {
        let b: Vec<String> = bodies(&svg_of(&[s("kind", kind), s("particle_form", form), b("show_answers", false)])).into_iter().filter(|t| !t.is_empty()).collect();
        assert_eq!(b, want, "{kind} {form} ({m})");
    }
    let pos = bodies(&svg_of(&[s("kind", "Positron")])).join(" ");
    assert!(pos.contains("positron: 0 protons, 0 neutrons, 0 electrons; charge 1+"), "{pos}");
    // the positron differs from the beta particle, and every form from the other
    assert_ne!(svg_of(&[s("kind", "Positron")]), svg_of(&[s("kind", "Beta particle")]));
    for kind in ["Alpha particle", "Beta particle", "Positron", "Proton"] {
        assert_ne!(svg_of(&[s("kind", kind)]), svg_of(&[s("kind", kind), s("particle_form", FORMS[1])]), "{kind}");
        assert!(relevant("particle_form", &with_list(&[s("kind", kind)])));
    }
    for kind in ["Element", "Neutron", "Gamma ray", "Custom symbol"] {
        assert!(!relevant("particle_form", &with_list(&[s("kind", kind)])), "{kind}");
    }
}

// ---------------------------------------------------------------------------------------------
// cap-05a, cap-05c: hyphen notation alone, long and short
// ---------------------------------------------------------------------------------------------

/// Hyphen notation alone draws only the hyphen form, long (Carbon-14) or short (C-14), and Both
/// puts the chosen form under the symbol.
#[test]
fn hyphen_notation_alone_draws_only_the_hyphen_form() {
    let long = svg_of(&[s("notation", "Hyphen notation"), i("mass", 14), b("show_answers", false)]);
    assert_eq!(bodies(&long), vec!["Carbon-14"]);
    let short = svg_of(&[s("notation", "Hyphen notation"), i("mass", 14), s("hyphen_form", "Symbol"), b("show_answers", false)]);
    assert_eq!(bodies(&short), vec!["C-14"]);
    let both = svg_of(&[s("notation", "Both"), i("mass", 14), s("hyphen_form", "Symbol"), b("show_answers", false)]);
    assert_eq!(bodies(&both), vec!["C", "14", "6", "", "C-14"]);
    let ion = svg_of(&[s("notation", "Hyphen notation"), s("hyphen_form", "Symbol"), s("element", "17 Cl - Chlorine"), i("mass", 37), i("charge", -1), b("show_answers", false)]);
    assert_eq!(bodies(&ion), vec!["Cl-37 ion, 1\u{2212}"]);
    // no symbol or numbers are drawn at the symbol's size, whatever the positions
    for pos in POSITIONS {
        for key in ["pos_mass", "pos_z", "pos_symbol", "pos_charge"] {
            let svg = svg_of(&[s("notation", "Hyphen notation"), i("charge", 2), s(key, pos)]);
            assert!(!svg.contains("font-size=\"82\"") && !svg.contains("font-size=\"30\""), "{key} {pos}: {svg}");
        }
    }
    // the atomic number is not part of hyphen notation, so its position draws nothing different
    assert_eq!(svg_of(&[s("notation", "Hyphen notation"), s("pos_z", "Blank")]), svg_of(&[s("notation", "Hyphen notation")]));
    // the forms differ from each other and from every notation
    let all = [
        svg_of(&[]), svg_of(&[s("notation", "Hyphen notation")]), svg_of(&[s("notation", "Both")]),
        svg_of(&[s("notation", "Hyphen notation"), s("hyphen_form", "Symbol")]), svg_of(&[s("notation", "Both"), s("hyphen_form", "Symbol")]),
    ];
    for (a, x) in all.iter().enumerate() {
        for y in all.iter().skip(a + 1) { assert_ne!(x, y); }
    }
    assert!(!relevant("hyphen_form", &with_list(&[])) && relevant("hyphen_form", &with_list(&[s("notation", "Both")])));
}

/// On the student copy the blanks of the hyphen form sit an equal gap from the text either side, as
/// laid out by resvg with the app's fonts (the old line put "ion" against a box), keep the space
/// before "ion", fit their answer, and stay clear of the text and on the page.
#[test]
fn hyphen_blanks_are_evenly_spaced_as_drawn() {
    let mut cases: Vec<Vec<(&'static str, Value)>> = vec![];
    for form in HYPHEN_FORMS {
        for font in FONTS {
            for size in ["Small", "Medium", "Large"] {
                for style in BLANKS {
                    for (el, m, c) in [("17 Cl - Chlorine", 37, -1), ("6 C - Carbon", 14, 0), ("92 U - Uranium", 238, 3), ("118 Og - Oganesson", 294, -8), ("74 W - Tungsten", 184, 2)] {
                        for blanked in [vec!["pos_mass"], vec!["pos_symbol"], vec!["pos_charge"], vec!["pos_mass", "pos_charge"], vec!["pos_symbol", "pos_mass", "pos_charge"]] {
                            for notation in ["Hyphen notation", "Both"] {
                                let mut v = vec![s("notation", notation), s("hyphen_form", form), s("font", font), s("label_size", size), s("blanks", style), s("element", el), i("mass", m), i("charge", c), b("show_answers", false)];
                                for k in &blanked { v.push(s(k, "Blank")); }
                                cases.push(v);
                            }
                        }
                    }
                }
            }
        }
    }
    let mut checked = 0;
    for c in &cases {
        let svg = svg_of(c);
        let what = format!("{c:?}");
        assert_readable(&svg, &what);
        let hy = c.iter().any(|(k, v)| *k == "notation" && *v == Value::S("Hyphen notation".into()));
        // the hyphen line's texts and blanks, left to right
        // (under the symbol in Both: everything centred between the symbol's baseline and the key)
        let on_line = |b: &Bx| hy || ((b.1 + b.3) / 2.0 > 160.0 && (b.1 + b.3) / 2.0 < 250.0);
        let ink: Vec<(String, Bx)> = ink_boxes(&svg).into_iter().filter(|(t, b)| !t.is_empty() && on_line(b)).collect();
        // (not the blank symbol's own line, 90.2 px wide about the centre)
        let bl: Vec<Bx> = blanks(&svg).into_iter().filter(|b| on_line(b) && !((b.0 - 214.9).abs() < 0.05 && (b.2 - 305.1).abs() < 0.05)).collect();
        if bl.is_empty() { continue; }
        let mut row: Vec<(f64, f64, bool, String)> = ink.iter().map(|(t, b)| (b.0, b.2, false, t.clone())).collect();
        row.extend(bl.iter().map(|b| (b.0, b.2, true, String::new())));
        row.sort_by(|a, b| a.0.total_cmp(&b.0));
        let size = texts(&svg).iter().filter(|t| t.1 > 0.0).map(|t| t.1).fold(0.0, f64::max).min(if hy { 1e9 } else { 22.0 * 1.4 });
        for w in row.windows(2) {
            let gap = w[1].0 - w[0].1;
            assert!(gap > 0.0, "{what}: {:?} touches {:?}", w[0], w[1]);
            // a blank's gap to a word after a space (", □", "□ ion") is about a word space; after a hyphen it is small
            let spaced = (w[0].2 && w[1].3.starts_with("ion")) || (!w[0].2 && w[0].3.ends_with(','));
            let (lo, hi) = if spaced { (0.15, 0.6) } else { (0.0, 0.35) };
            assert!(gap >= lo * size * 0.5 && gap <= hi * size * 1.4 + 3.0, "{what}: gap {gap:.1} between {:?} and {:?} (size {size})", w[0], w[1]);
        }
        // a blank is wide enough for its answer as it would be drawn
        for b in &bl { assert!(b.2 - b.0 >= 0.9 * size * 0.55, "{what}: blank {b:?} narrower than an answer"); }
        checked += 1;
    }
    assert!(checked > 750, "only {checked} lines checked");
    // the space before "ion" is kept: a box never touches "ion" (the round 6 failure)
    let svg = svg_of(&[s("notation", "Hyphen notation"), s("element", "17 Cl - Chlorine"), i("mass", 37), i("charge", -1), s("pos_mass", "Blank"), s("pos_charge", "Blank")]);
    let ink = ink_boxes(&svg);
    let ion = ink.iter().find(|(t, _)| t.starts_with("ion")).expect("ion drawn").1;
    let boxes = blanks(&svg);
    let first = boxes.iter().min_by(|a, b| a.0.total_cmp(&b.0)).unwrap();
    let hyph = ink.iter().find(|(t, _)| t.starts_with("Chlorine")).unwrap().1;
    let (before, after) = (first.0 - hyph.2, ion.0 - first.2);
    assert!(after > before + 5.0, "a word space before ion: {before:.1} after the hyphen, {after:.1} before ion");
}

/// The student copy gives nothing away: a blanked piece is in no text, and the SVG's title and
/// description are the figure's own name and blurb.
#[test]
fn the_student_copy_gives_no_answer() {
    for (form, notation) in [("Name", "Hyphen notation"), ("Symbol", "Hyphen notation"), ("Name", "Both"), ("Symbol", "Both"), ("Name", "Symbol")] {
        let svg = svg_of(&[s("notation", notation), s("hyphen_form", form), s("element", "17 Cl - Chlorine"), s("isotope", "Cl-37"), i("charge", -1),
            s("pos_mass", "Blank"), s("pos_z", "Blank"), s("pos_symbol", "Blank"), s("pos_charge", "Blank"), b("show_answers", false)]);
        let all = bodies(&svg).join("|");
        for leak in ["37", "17", "Cl", "Chlor", "1\u{2212}", "18", "20"] {
            assert!(!all.contains(leak), "{form} {notation}: {leak} in {all}");
        }
        let head = &svg[..svg.find("</desc>").unwrap()];
        assert!(head.contains("<title>Nuclide Notation</title>") && !head.contains("37") && !head.contains("Chlor"), "{head}");
    }
    let svg = svg_of(&[s("set_by", "Particles"), i("protons", 26), i("neutrons", 30), i("electrons", 23), s("pos_mass", "Blank"), s("pos_symbol", "Blank"), s("pos_charge", "Blank"), b("show_answers", false)]);
    let all = bodies(&svg).join("|");
    assert!(!all.contains("56") && !all.contains("Fe") && !all.contains("3+"), "{all}");
}

// ---------------------------------------------------------------------------------------------
// cap-06a: serif type
// ---------------------------------------------------------------------------------------------

/// Serif sets every text in Times New Roman, or its bundled metric twin Liberation Serif, which has
/// every glyph the figure draws (so no fallback face changes the widths), and the widths the layout
/// uses are the widths resvg draws.
#[test]
fn serif_type_is_measured_as_drawn() {
    let svg = svg_of(&[s("font", "Serif"), s("notation", "Both"), s("title", "Isotopes"), i("charge", -2)]);
    let ts = texts(&svg);
    assert!(ts.iter().all(|t| t.2 == SERIF), "{svg}");
    assert!(texts(&svg_of(&[])).iter().all(|t| t.2.starts_with("Arial")));
    assert_ne!(svg_of(&[s("font", "Serif")]), svg_of(&[]));
    for serif in [false, true] {
        for bold in [false, true] {
            assert!(has_glyphs("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-+,;:() \u{2212}\u{3b1}\u{3b2}\u{3b3}", serif, bold), "{serif} {bold}");
        }
    }
    // the advance widths used for placing agree with resvg's layout to within 2%
    for serif in [false, true] {
        for t in ["Chlorine-37 ion, 1\u{2212}", "Oganesson-294", "W-184", "\u{3b1}", "\u{3b2}", "\u{3b3}", "Te", "AV", "Wolfram", "Yttrium-89", "Typed"] {
            let fam = if serif { SERIF } else { super::super::FONT };
            let svg = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="900" height="100"><text x="0" y="60" font-size="50" font-family="{fam}" font-weight="bold">{t}</text><text x="0" y="60" font-size="50" font-family="{fam}" font-weight="bold">|{t}|</text><text x="0" y="60" font-size="50" font-family="{fam}" font-weight="bold">||</text></svg>"##);
            let ink = ink_boxes(&svg);
            // the text between two bars: its advance is the bars' span less the two bars' own
            let span = (ink[1].1).2 - (ink[1].1).0;
            let bars = (ink[2].1).2 - (ink[2].1).0;
            let drawn = span - bars;
            let ours = advance(t, 50.0, serif, true);
            assert!((drawn - ours).abs() <= 0.005 * ours + 0.2, "{serif} {t}: drawn {drawn:.1}, measured {ours:.1}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Every option, every extreme
// ---------------------------------------------------------------------------------------------

/// Each new option draws differently from each other option of its control.
#[test]
fn every_new_option_draws_differently() {
    let check = |base: Vec<(&'static str, Value)>, key: &'static str, opts: &[&str]| {
        let r: Vec<String> = opts.iter().map(|o| { let mut v = base.clone(); v.push(s(key, o)); svg_of(&v) }).collect();
        for a in 0..r.len() { for c in a + 1..r.len() { assert_ne!(r[a], r[c], "{key}: {} and {} draw the same", opts[a], opts[c]); } }
    };
    check(vec![], "kind", &KINDS);
    check(vec![s("kind", "Alpha particle")], "particle_form", &FORMS);
    check(vec![s("notation", "Hyphen notation")], "hyphen_form", &HYPHEN_FORMS);
    check(vec![s("notation", "Both")], "hyphen_form", &HYPHEN_FORMS);
    check(vec![], "font", &FONTS);
    check(vec![i("protons", 8)], "set_by", &SET_BY);
    check(vec![], "isotope", &[TYPED, "C-13"]);
    // Typed with no mass number is the usual isotope, U-238, so it is set apart from the others
    check(vec![s("element", "92 U - Uranium")], "isotope", &[TYPED, "U-234", "U-235"]);
    check(vec![s("element", "92 U - Uranium")], "isotope", &["U-234", "U-235", "U-238"]);
}

/// The crowded and extreme cases every control allows together: two-letter symbols, three-digit
/// mass numbers, charges to 8− and 8+, every particle and form, serif and sans, Small to Large, a
/// title and the key: no text on text or on a blank, none off the page or under 8 px.
#[test]
fn crowded_cases_stay_readable() {
    let mut n = 0;
    for font in FONTS {
        for size in ["Small", "Medium", "Large"] {
            for title in ["", "Isotopes of the heavy elements and their ions"] {
                for notation in NOTATIONS {
                    let mut kinds: Vec<Vec<(&'static str, Value)>> = vec![];
                    for (el, m, c) in [("118 Og - Oganesson", 300, -8), ("74 W - Tungsten", 184, 8), ("1 H - Hydrogen", 3, -1), ("92 U - Uranium", 238, 6), ("53 I - Iodine", 131, -3)] {
                        kinds.push(vec![s("element", el), i("mass", m), i("charge", c)]);
                    }
                    kinds.push(vec![s("kind", "Custom symbol"), s("symbol", "Wmw"), i("mass", 300), i("charge", -8)]);
                    for k in ["Alpha particle", "Beta particle", "Positron", "Neutron", "Proton", "Gamma ray"] {
                        for f in FORMS { kinds.push(vec![s("kind", k), s("particle_form", f)]); }
                    }
                    for k in kinds {
                        for pos in ["Shown", "Blank"] {
                            let mut v = k.clone();
                            v.extend([s("font", font), s("label_size", size), s("title", title), s("notation", notation), s("pos_mass", pos), s("pos_charge", pos)]);
                            let svg = svg_of(&v);
                            assert_readable(&svg, &format!("{v:?}"));
                            n += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(n > 500, "{n}");
}
