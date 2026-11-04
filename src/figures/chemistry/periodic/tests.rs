//! Tests of the periodic-table excerpt. The science tests re-derive the 118 placements, the group
//! and period of every element and the ten-category table from an independent hand-written grid;
//! the drawing tests read the SVG back, and the label audit measures every text with the real
//! glyphs the PNG renderer draws.
use super::*;
use crate::figures::chemistry::reactionenergy::{hits, seg_hits};
use crate::figures::{registry, svg_size, to_png};
use std::collections::{BTreeMap, BTreeSet, HashMap};

// ---------------------------------------------------------------------------------------------
// The independent table
// ---------------------------------------------------------------------------------------------

/// The 118 symbols and names in atomic-number order, written out from the periodic table, not read
/// from the module's data table: this is what the placement test and the name test compare against.
const SYMBOLS: &str = "H He Li Be B C N O F Ne Na Mg Al Si P S Cl Ar K Ca Sc Ti V Cr Mn Fe Co Ni Cu Zn \
    Ga Ge As Se Br Kr Rb Sr Y Zr Nb Mo Tc Ru Rh Pd Ag Cd In Sn Sb Te I Xe Cs Ba La Ce Pr Nd Pm Sm Eu \
    Gd Tb Dy Ho Er Tm Yb Lu Hf Ta W Re Os Ir Pt Au Hg Tl Pb Bi Po At Rn Fr Ra Ac Th Pa U Np Pu Am Cm \
    Bk Cf Es Fm Md No Lr Rf Db Sg Bh Hs Mt Ds Rg Cn Nh Fl Mc Lv Ts Og";
const NAMES: &str = "Hydrogen Helium Lithium Beryllium Boron Carbon Nitrogen Oxygen Fluorine Neon Sodium \
    Magnesium Aluminum Silicon Phosphorus Sulfur Chlorine Argon Potassium Calcium Scandium Titanium \
    Vanadium Chromium Manganese Iron Cobalt Nickel Copper Zinc Gallium Germanium Arsenic Selenium Bromine \
    Krypton Rubidium Strontium Yttrium Zirconium Niobium Molybdenum Technetium Ruthenium Rhodium Palladium \
    Silver Cadmium Indium Tin Antimony Tellurium Iodine Xenon Cesium Barium Lanthanum Cerium Praseodymium \
    Neodymium Promethium Samarium Europium Gadolinium Terbium Dysprosium Holmium Erbium Thulium Ytterbium \
    Lutetium Hafnium Tantalum Tungsten Rhenium Osmium Iridium Platinum Gold Mercury Thallium Lead Bismuth \
    Polonium Astatine Radon Francium Radium Actinium Thorium Protactinium Uranium Neptunium Plutonium \
    Americium Curium Berkelium Californium Einsteinium Fermium Mendelevium Nobelium Lawrencium \
    Rutherfordium Dubnium Seaborgium Bohrium Hassium Meitnerium Darmstadtium Roentgenium Copernicium \
    Nihonium Flerovium Moscovium Livermorium Tennessine Oganesson";

/// The main grid as it must be drawn, 7 rows of 18 columns; `.` is an empty cell. The group-3 cell
/// of periods 6 and 7 is empty because lanthanum and actinium start the pulled-out strips.
const GRID: [&str; 7] = [
    "H . . . . . . . . . . . . . . . . He",
    "Li Be . . . . . . . . . . B C N O F Ne",
    "Na Mg . . . . . . . . . . Al Si P S Cl Ar",
    "K Ca Sc Ti V Cr Mn Fe Co Ni Cu Zn Ga Ge As Se Br Kr",
    "Rb Sr Y Zr Nb Mo Tc Ru Rh Pd Ag Cd In Sn Sb Te I Xe",
    "Cs Ba . Hf Ta W Re Os Ir Pt Au Hg Tl Pb Bi Po At Rn",
    "Fr Ra . Rf Db Sg Bh Hs Mt Ds Rg Cn Nh Fl Mc Lv Ts Og",
];
const F_GRID: [&str; 2] = [
    "La Ce Pr Nd Pm Sm Eu Gd Tb Dy Ho Er Tm Yb Lu",
    "Ac Th Pa U Np Pu Am Cm Bk Cf Es Fm Md No Lr",
];

const ALKALI: [usize; 6] = [3, 11, 19, 37, 55, 87];
const EARTH: [usize; 6] = [4, 12, 20, 38, 56, 88];
const TRANSITION: [usize; 38] = [
    21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 72, 73, 74, 75,
    76, 77, 78, 79, 80, 104, 105, 106, 107, 108, 109, 110, 111, 112,
];
const POST: [usize; 11] = [13, 31, 49, 50, 81, 82, 83, 113, 114, 115, 116];
const NONMETAL: [usize; 7] = [1, 6, 7, 8, 15, 16, 34];
const NOBLE: [usize; 7] = [2, 10, 18, 36, 54, 86, 118];
const LANTH: [usize; 15] = [57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71];
const ACTIN: [usize; 15] = [89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103];

fn symbols() -> Vec<&'static str> { SYMBOLS.split_whitespace().collect() }
fn names() -> Vec<&'static str> { NAMES.split_whitespace().collect() }
fn z_of(sym: &str) -> usize { symbols().iter().position(|s| *s == sym).expect("a known symbol") + 1 }

// ---------------------------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------------------------

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).expect("periodic-table is registered") }
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
fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }
fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
fn svg_of(list: &[(&'static str, Value)]) -> String { def().svg(&with(list)) }
fn build_of(list: &[(&'static str, Value)]) -> (String, Report) { build(&with(list)) }
fn rep_of(list: &[(&'static str, Value)]) -> Report { build_of(list).1 }
fn warns(list: &[(&'static str, Value)]) -> Vec<String> { warnings(&with(list)) }
fn has(notes: &[String], what: &str) -> bool { notes.iter().any(|n| n.contains(what)) }
fn key(list: &[(&'static str, Value)]) -> Vec<(&'static str, Value)> {
    let mut v = list.to_vec();
    v.push(b(SHOW_ANSWERS, true));
    v
}
fn student(list: &[(&'static str, Value)]) -> Vec<(&'static str, Value)> {
    let mut v = list.to_vec();
    v.push(b(SHOW_ANSWERS, false));
    v
}

// ---------------------------------------------------------------------------------------------
// Reading the SVG back
// ---------------------------------------------------------------------------------------------

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
    weight: String,
    z: Option<usize>,
}

fn attr(tag: &str, k: &str) -> Option<String> {
    let pat = format!(" {k}=\"");
    let at = tag.find(&pat)? + pat.len();
    tag[at..].split('"').next().map(|v| v.to_string())
}
fn attr_f(tag: &str, k: &str) -> Option<f64> { attr(tag, k)?.parse().ok() }
fn unesc(s: &str) -> String { s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&amp;", "&") }

fn texts(svg: &str) -> Vec<T> {
    let mut out = Vec::new();
    for part in svg.split("<text").skip(1) {
        let Some((tag, rest)) = part.split_once('>') else { continue };
        // the white halo under Print type draws no ink of its own and would double every cell text
        if tag.contains(r#"fill="none""#) || attr(tag, "data-o").as_deref() == Some("halo") { continue; }
        let text = unesc(rest.split("</text>").next().unwrap_or(""));
        out.push(T {
            x: attr_f(tag, "x").unwrap_or(0.0),
            y: attr_f(tag, "y").unwrap_or(0.0),
            fs: attr_f(tag, "font-size").unwrap_or(12.0),
            anchor: attr(tag, "text-anchor").unwrap_or_else(|| "start".into()),
            kind: attr(tag, "data-o").unwrap_or_default(),
            feat: attr(tag, "data-f").unwrap_or_default(),
            fill: attr(tag, "fill").unwrap_or_default(),
            weight: attr(tag, "font-weight").unwrap_or_else(|| "normal".into()),
            z: attr(tag, "data-z").and_then(|v| v.parse().ok()),
            text,
        });
    }
    out
}

type Seg = ((f64, f64), (f64, f64));

struct Geo {
    lines: Vec<(String, Seg)>,
    polys: Vec<(String, Vec<(f64, f64)>)>,
    rects: Vec<(String, String, Bx)>,
}

fn geo(svg: &str) -> Geo {
    let mut g = Geo { lines: Vec::new(), polys: Vec::new(), rects: Vec::new() };
    for part in svg.split('<').skip(1) {
        let tag = part.split('>').next().unwrap_or("");
        let kind = attr(tag, "data-o").unwrap_or_default();
        let feat = attr(tag, "data-f").unwrap_or_default();
        if tag.starts_with("line ") {
            if let (Some(x1), Some(y1), Some(x2), Some(y2)) = (attr_f(tag, "x1"), attr_f(tag, "y1"), attr_f(tag, "x2"), attr_f(tag, "y2")) {
                g.lines.push((kind, ((x1, y1), (x2, y2))));
            }
        } else if tag.starts_with("polygon ") {
            let pts = attr(tag, "points").unwrap_or_default();
            let v: Vec<(f64, f64)> = pts.split_whitespace().filter_map(|p| {
                let (a, b) = p.split_once(',')?;
                Some((a.parse().ok()?, b.parse().ok()?))
            }).collect();
            if v.len() >= 3 { g.polys.push((kind, v)); }
        } else if tag.starts_with("rect ") {
            if let (Some(x), Some(y), Some(w), Some(h)) = (attr_f(tag, "x"), attr_f(tag, "y"), attr_f(tag, "width"), attr_f(tag, "height")) {
                g.rects.push((kind, feat, Bx(x, x + w, y, y + h)));
            }
        }
    }
    g
}

// ---------------------------------------------------------------------------------------------
// Registration and the controls
// ---------------------------------------------------------------------------------------------

#[test]
fn it_is_registered_with_a_faq_examples_and_its_controls() {
    let d = def();
    assert_eq!(d.id, "periodic-table");
    assert_eq!(d.subject, Subject::Chemistry);
    assert!((4..=6).contains(&crate::figures::faq::faq_for(ID).len()), "{} FAQs", crate::figures::faq::faq_for(ID).len());
    let keys: Vec<&str> = (d.params)().iter().map(|p| p.key).collect();
    for k in ["block", "labels", "key", "trends", "highlight", SHOW_ANSWERS, "style", "label_size"] {
        assert!(keys.contains(&k), "no {k} param");
    }
    assert!(crate::figures::chemistry::skin_intro(ID).is_some());
    assert!(!crate::figures::notes_for(ID, &raw(&[s("highlight", "Xx")])).is_empty());
    assert!(crate::figures::notes_for(ID, &with(&[])).is_empty());
    let names: Vec<&str> = crate::figures::examples_for(ID).iter().map(|e| e.name).collect();
    assert!(names.len() >= 5, "{names:?}");
    assert!(names.iter().any(|n| n.contains("Full table")), "{names:?}");
    for e in crate::figures::examples_for(ID) {
        let p = with(&e.params);
        assert!(warnings(&p).is_empty(), "{}: {:?}", e.name, warnings(&p));
        for ans in [true, false] {
            let mut q = p.clone();
            q.set_b(SHOW_ANSWERS, ans);
            assert!(warnings(&q).is_empty(), "{}: {:?}", e.name, warnings(&q));
            let svg = def().svg(&q);
            assert!(svg.contains("<svg") && svg.len() > 10_000, "{}", e.name);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The science
// ---------------------------------------------------------------------------------------------

#[test]
fn the_element_table_is_118_correct_symbols_and_names() {
    let syms = symbols();
    let nms = names();
    assert_eq!(syms.len(), 118);
    assert_eq!(nms.len(), 118);
    assert_eq!(data::ELEMENTS.len(), 118);
    let uniq_s: BTreeSet<&str> = syms.iter().copied().collect();
    let uniq_n: BTreeSet<&str> = nms.iter().copied().collect();
    assert_eq!(uniq_s.len(), 118, "a symbol repeats");
    assert_eq!(uniq_n.len(), 118, "a name repeats");
    for (k, &(sym, name, _)) in data::ELEMENTS.iter().enumerate() {
        assert_eq!(sym, syms[k], "Z = {}: symbol", k + 1);
        assert_eq!(name, nms[k], "Z = {}: name", k + 1);
        let s = sym.chars().collect::<Vec<char>>();
        assert!(s[0].is_ascii_uppercase(), "{sym}");
        assert!(s.len() <= 2 && (s.len() == 1 || s[1].is_ascii_lowercase()), "{sym}");
        assert!(name.chars().all(|c| c.is_ascii_alphabetic()), "{name}");
        assert_eq!(data::symbol(k + 1), sym);
        assert_eq!(data::name(k + 1), name);
    }
    // the spots a typo would hit hardest
    for (z, sym) in [(57, "La"), (63, "Eu"), (74, "W"), (78, "Pt"), (103, "Lr"), (118, "Og")] {
        assert_eq!(data::symbol(z), sym);
    }
}

#[test]
fn every_element_sits_in_exactly_one_cell() {
    let r = rep_of(&[s("block", BLOCKS[2])]);
    assert_eq!(r.cells.len(), 118, "the full setting draws 118 cells");
    let zs: BTreeSet<usize> = r.cells.iter().map(|c| c.z).collect();
    assert_eq!(zs.len(), 118, "an element is in two cells");
    let spots: BTreeSet<(usize, usize)> = r.cells.iter().map(|c| (c.col, c.row)).collect();
    assert_eq!(spots.len(), 118, "two elements share a cell");
    // the hand-written grid
    for (row, line) in GRID.iter().enumerate() {
        let toks: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(toks.len(), 18, "row {} is not 18 wide", row + 1);
        for (col, tok) in toks.iter().enumerate() {
            if *tok == "." { continue; }
            let z = z_of(tok);
            let c = r.cells.iter().find(|c| c.z == z).unwrap_or_else(|| panic!("{tok} ({z}) is not drawn"));
            assert_eq!((c.col, c.row), (col, row), "{tok} is in the wrong cell");
            let p = place(z).unwrap();
            assert_eq!(p.period, row as u8 + 1, "{tok}: period");
            assert_eq!(p.group, Some(col as u8 + 1), "{tok}: group");
        }
    }
    for (k, line) in F_GRID.iter().enumerate() {
        let toks: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(toks.len(), 15, "f row {} is not 15 wide", k + 1);
        for (j, tok) in toks.iter().enumerate() {
            let z = z_of(tok);
            let c = r.cells.iter().find(|c| c.z == z).unwrap_or_else(|| panic!("{tok} is not drawn"));
            assert_eq!((c.col, c.row), (j + 2, 7 + k), "{tok} is in the wrong f-block cell");
            assert!(place(z).unwrap().f_row.is_some());
        }
    }
    // H and He in period 1, and every period 1-7 has a cell except the pulled-out columns
    let h = r.cells.iter().find(|c| c.z == 1).unwrap();
    let he = r.cells.iter().find(|c| c.z == 2).unwrap();
    assert_eq!((h.col, h.row), (0, 0));
    assert_eq!((he.col, he.row), (17, 0));
    // the narrower settings draw exactly the elements they promise
    assert_eq!(rep_of(&[]).cells.len(), 50);
    assert_eq!(rep_of(&[s("block", BLOCKS[1])]).cells.len(), 88);
    for z in 1..=118 {
        assert_eq!(block_shows(0, z), is_main(z), "{z}");
        assert_eq!(block_shows(1, z), !is_f(z), "{z}");
        assert!(block_shows(2, z));
    }
}

#[test]
fn the_f_block_is_two_strips_of_fifteen_under_group_three() {
    let r = rep_of(&[s("block", BLOCKS[2])]);
    let cell = r.cell;
    for (k, first) in [(0usize, 57usize), (1, 89)] {
        let row: Vec<&Drawn> = r.cells.iter().filter(|c| c.z >= first && c.z < first + 15).collect();
        assert_eq!(row.len(), 15, "{}", if k == 0 { "lanthanides" } else { "actinides" });
        for (j, z) in (first..first + 15).enumerate() {
            let c = row.iter().find(|c| c.z == z).unwrap_or_else(|| panic!("{z} missing"));
            assert_eq!(c.col, j + 2, "Z {z} column");
            assert_eq!(c.row, 7 + k, "Z {z} row");
        }
        let x0 = row.iter().map(|c| c.x0).fold(f64::MAX, f64::min);
        let x1 = row.iter().map(|c| c.x1).fold(f64::MIN, f64::max);
        assert!((x1 - x0 - 15.0 * cell).abs() < 0.01, "strip is not 15 cells wide: {}", x1 - x0);
        // it starts under group 3: the same left edge as scandium (Sc, group 3 period 4)
        let sc = r.cells.iter().find(|c| c.z == 21).unwrap();
        assert!((x0 - sc.x0).abs() < 0.01, "strip starts at {} not group 3's {}", x0, sc.x0);
    }
    // below the seventh period, with the period rows above
    let row7 = r.cells.iter().filter(|c| c.row == 6).map(|c| c.y1).fold(f64::MIN, f64::max);
    assert!(r.cells.iter().filter(|c| c.row >= 7).all(|c| c.y0 > row7), "the strips are not below the grid");
    // the strip labels and the two group-3 markers
    let ts = texts(&build_of(&[s("block", BLOCKS[2])]).0);
    let frow: Vec<&T> = ts.iter().filter(|t| t.kind == "frow").collect();
    for want in ["Lanthanides", "Actinides"] {
        assert!(frow.iter().any(|t| t.text == want), "{want} is not labelled");
    }
    assert!(ts.iter().any(|t| t.kind == "gap-text" && t.text == "57-71"));
    assert!(ts.iter().any(|t| t.kind == "gap-text" && t.text == "89-103"));
    // the d-block setting has the markers but no strips
    let d = build_of(&[s("block", BLOCKS[1])]).0;
    assert!(!d.contains(r#"data-o="frow""#));
    assert!(d.contains("57-71") && d.contains("89-103"));
    // the main-group setting omits both
    let m = build_of(&[]).0;
    assert!(!m.contains("57-71") && !m.contains(r#"data-o="gap""#));
}

#[test]
fn the_group_and_period_labels_are_the_ones_drawn() {
    for (block, want) in [
        (BLOCKS[0], vec![1, 2, 13, 14, 15, 16, 17, 18]),
        (BLOCKS[1], (1..=18).collect()),
        (BLOCKS[2], (1..=18).collect()),
    ] {
        let (svg, r) = build_of(&[s("block", block)]);
        assert_eq!(r.group_labels, want, "{block}");
        let ts = texts(&svg);
        let gs: Vec<u8> = ts.iter().filter(|t| t.kind == "group").map(|t| t.text.parse().unwrap()).collect();
        assert_eq!(gs, want, "{block}: group labels");
        let ps: Vec<u8> = ts.iter().filter(|t| t.kind == "period").map(|t| t.text.parse().unwrap()).collect();
        assert_eq!(ps, vec![1, 2, 3, 4, 5, 6, 7], "{block}: period labels");
        // a group number is centred over its own column and a period number sits in the gutter
        for t in ts.iter().filter(|t| t.kind == "group") {
            let g: usize = t.text.parse().unwrap();
            let idx = want.iter().position(|x| *x == g as u8).unwrap();
            assert!((t.x - (r.grid.0 + idx as f64 * r.cell + r.cell / 2.0)).abs() < 0.01, "group {g} is off its column");
        }
        for t in ts.iter().filter(|t| t.kind == "period") {
            assert!(t.x < r.grid.0, "period {} is not in the gutter", t.text);
        }
    }
}

#[test]
fn the_category_table_partitions_all_118_elements() {
    assert_eq!(CATS.len(), 10);
    let uniq: BTreeSet<&str> = CATS.iter().copied().collect();
    assert_eq!(uniq.len(), 10);
    let mut counts = [0usize; 10];
    for z in 1..=118 { counts[category(z)] += 1; }
    assert_eq!(counts, [6, 6, 38, 11, 7, 7, 6, 7, 15, 15], "{counts:?}");
    assert_eq!(counts.iter().sum::<usize>(), 118);
    let expect: [(usize, &[usize]); 10] = [
        (0, &ALKALI[..]), (1, &EARTH[..]), (2, &TRANSITION[..]), (3, &POST[..]), (4, &METALLOIDS[..]),
        (5, &NONMETAL[..]), (6, &HALOGENS[..]), (7, &NOBLE[..]), (8, &LANTH[..]), (9, &ACTIN[..]),
    ];
    let mut seen = [0usize; 118];
    for (cat, zs) in expect {
        for z in zs {
            assert_eq!(category(*z), cat, "Z {z} is not {}", CATS[cat]);
            seen[z - 1] += 1;
        }
    }
    assert!(seen.iter().all(|n| *n == 1), "an element is unclassified or classified twice");
    // the stated boundary elements, one by one
    for z in [5, 14, 32, 33, 51, 52, 84] { assert_eq!(CATS[category(z)], "metalloid", "Z {z}"); }
    for z in [9, 17, 35, 53, 85, 117] { assert_eq!(CATS[category(z)], "halogen", "Z {z}"); }
    for z in [13, 31, 49, 50, 81, 82, 83, 113, 114, 115, 116] { assert_eq!(CATS[category(z)], "post-transition metal", "Z {z}"); }
    assert_eq!(CATS[category(1)], "reactive nonmetal", "hydrogen is not an alkali metal");
    assert_eq!(CATS[category(2)], "noble gas");
    assert_eq!(CATS[category(57)], "lanthanide");
    assert_eq!(CATS[category(89)], "actinide");
    assert_eq!(CATS[category(71)], "lanthanide");
    assert_eq!(CATS[category(103)], "actinide");
    // the drawn cells carry the same ten indices, and none is left uncoloured
    let (svg, r) = build_of(&[s("block", BLOCKS[2])]);
    for c in &r.cells { assert_eq!(c.cat, category(c.z)); assert!(c.cat < 10); }
    let g = geo(&svg);
    let cell_rects: Vec<&(String, String, Bx)> = g.rects.iter().filter(|r| r.0 == "cell").collect();
    assert_eq!(cell_rects.len(), 118);
    let fills: BTreeSet<&str> = cell_rects.iter().map(|r| r.1.as_str()).collect();
    assert_eq!(fills.len(), 10, "the cells do not use all ten category fills: {fills:?}");
}

#[test]
fn the_header_and_the_faq_state_the_same_boundary_and_source() {
    let src = include_str!("../periodic.rs");
    let faq = crate::figures::faq::faq_for(ID).iter().map(|q| format!("{} {}", q.q, q.a)).collect::<String>();
    for (what, text) in [("header", src), ("FAQ", faq.as_str())] {
        assert!(text.contains("B, Si, Ge, As, Sb, Te, Po"), "{what} does not state the staircase");
        assert!(text.contains("Royal Society of Chemistry"), "{what} does not cite the source");
    }
    assert!(faq.contains("simplification"), "the FAQ does not call the trends a simplification");
    assert!(faq.contains("no Pauling electronegativity"), "the FAQ does not warn about the noble gases");
    assert!(faq.contains("hydrogen"), "the FAQ does not explain hydrogen");
    assert!(faq.contains("fifteen"), "the FAQ does not say why the f-block is pulled out");
}

#[test]
fn the_highlight_lookup_takes_a_symbol_or_an_atomic_number_in_any_case() {
    for (typed, want) in [
        ("Au", 79), ("au", 79), ("AU", 79), (" 79 ", 79), ("79", 79), ("079", 79),
        ("H", 1), ("h", 1), ("HE", 2), ("he", 2), ("118", 118), ("Lr", 103),
    ] {
        assert_eq!(lookup(typed), Hit::Element(want), "{typed}");
    }
    assert_eq!(lookup(""), Hit::None);
    assert_eq!(lookup("   "), Hit::None);
    for bad in ["0", "119", "-1", "1.5", "12a", "Xx", "Unobtainium"] {
        match lookup(bad) {
            Hit::Refused(n) => {
                assert!(n.contains("Highlight") && n.contains("no element is highlighted"), "{bad}: {n}");
            }
            other => panic!("{bad}: {other:?}"),
        }
    }
    // a typo gets advice, and the test applies it as a student would
    match lookup("auu") {
        Hit::Refused(n) => {
            assert!(n.contains("Did you mean Au"), "{n}");
            assert_eq!(suggest("auu"), Some("Au"));
            assert!(matches!(lookup(suggest("auu").unwrap()), Hit::Element(79)));
        }
        other => panic!("{other:?}"),
    }
    match lookup("gld") {
        Hit::Refused(n) => {
            let sym = suggest("gld").expect("a close symbol");
            assert!(n.contains(&format!("Did you mean {sym}")), "{n}");
            assert!(matches!(lookup(sym), Hit::Element(_)), "{sym} does not resolve");
        }
        other => panic!("{other:?}"),
    }
    // no close symbol: the generic advice still resolves
    match lookup("zzzz") {
        Hit::Refused(n) => {
            assert!(n.contains("such as Au"), "{n}");
            assert!(matches!(lookup("Au"), Hit::Element(79)));
        }
        other => panic!("{other:?}"),
    }
    // every symbol, in lower case, is itself
    for (k, sym) in symbols().iter().enumerate() {
        assert_eq!(lookup(&sym.to_lowercase()), Hit::Element(k + 1), "{sym}");
    }
}

#[test]
fn the_highlight_panel_reads_out_name_z_group_period_and_category() {
    let au = rep_of(&key(&[s("block", BLOCKS[1]), s("highlight", "Au")]));
    let p = au.panel.join(" ");
    for want in ["Highlighted element: Au", "Gold", "Z = 79", "group 11", "period 6", "transition metal"] {
        assert!(p.contains(want), "the key panel lacks {want:?}: {p}");
    }
    // a main-group element also gets the valence line, with the group 13-18 rule stated
    let al = rep_of(&key(&[s("highlight", "Al")])).panel.join(" ");
    for want in ["Aluminum", "Z = 13", "group 13", "period 3", "post-transition metal", "Valence shell: 3 electrons (group 13 - 10)"] {
        assert!(al.contains(want), "the key panel lacks {want:?}: {al}");
    }
    // helium is the exception: 2 electrons in group 18
    let he = rep_of(&key(&[s("highlight", "He")])).panel.join(" ");
    assert!(he.contains("group 18") && he.contains("period 1") && he.contains("noble gas"), "{he}");
    assert!(he.contains("Valence shell: 2 electrons") && he.contains("helium"), "{he}");
    // a halogen in group 17
    let br = rep_of(&key(&[s("highlight", "Br")])).panel.join(" ");
    assert!(br.contains("group 17") && br.contains("period 4") && br.contains("halogen"), "{br}");
    assert!(br.contains("Valence shell: 7 electrons (group 17 - 10)"), "{br}");
    // the f-block has no group number, and the panel says so
    let la = rep_of(&key(&[s("block", BLOCKS[2]), s("highlight", "La")])).panel.join(" ");
    assert!(la.contains("Lanthanum") && la.contains("Z = 57") && la.contains("group - (f-block)"), "{la}");
    assert!(la.contains("period 6") && la.contains("lanthanide"), "{la}");
    assert!(!la.contains("Valence shell"), "the f-block has no group valence count: {la}");
    // and the d-block does not claim one either
    let pd = rep_of(&key(&[s("block", BLOCKS[1]), s("highlight", "Pd")])).panel.join(" ");
    assert!(!pd.contains("Valence shell"), "{pd}");
    // an empty box has no read-out
    let none = rep_of(&key(&[])).panel.join(" ");
    assert!(none.contains("none") && !none.contains("Z ="), "{none}");
}

#[test]
fn the_valence_count_follows_the_stated_rule_for_every_main_group_element() {
    for z in 1..=118 {
        let want = if z == 2 { Some(2u8) } else { match place(z).unwrap().group {
            Some(1) => Some(1),
            Some(2) => Some(2),
            Some(g) if g >= 13 => Some(g - 10),
            _ => None,
        } };
        assert_eq!(valence(z), want, "Z {z} ({})", data::symbol(z));
    }
    assert_eq!(valence(2), Some(2), "helium: 2, not 8");
    assert_eq!(valence(18), Some(8));
    assert_eq!(valence(26), None, "iron is not a main-group element");
    assert_eq!(valence(57), None);
    // the fixed rule is printed on both copies
    for ans in [true, false] {
        let svg = svg_of(&[b(SHOW_ANSWERS, ans)]);
        let foot: String = texts(&svg).iter().filter(|t| t.kind == "foot").map(|t| t.text.as_str()).collect::<Vec<_>>().join(" ");
        assert!(foot.contains("Main-group valence electrons: group 1 = 1, group 2 = 2, groups 13-18 = group minus 10; helium has 2 in group 18."), "{ans}: {foot}");
    }
}

// ---------------------------------------------------------------------------------------------
// Labels and the drawing
// ---------------------------------------------------------------------------------------------

struct Glyphs { opt: resvg::usvg::Options<'static>, memo: HashMap<String, Option<Bx>> }

impl Glyphs {
    fn new() -> Self {
        let mut opt = resvg::usvg::Options::default();
        opt.fontdb = crate::figures::fontdb();
        Glyphs { opt, memo: HashMap::new() }
    }
    fn rel(&mut self, t: &T) -> Option<Bx> {
        let key = format!("{}|{:.2}|{}|{}", t.text, t.fs, t.anchor, t.weight);
        if let Some(v) = self.memo.get(&key) { return *v; }
        let font = super::super::FONT;
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="6000" height="600"><text x="3000" y="300" font-size="{:.2}" text-anchor="{}" font-family="{font}" font-weight="{}">{}</text></svg>"#,
            t.fs, t.anchor, t.weight, crate::figures::svg::esc(&t.text)
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
            .map(|r| Bx(r.left() as f64 - 3000.0, r.right() as f64 - 3000.0, r.top() as f64 - 300.0, r.bottom() as f64 - 300.0));
        self.memo.insert(key, out);
        out
    }
    fn ink(&mut self, t: &T) -> Option<Bx> {
        let r = self.rel(t)?;
        Some(Bx(t.x + r.0, t.x + r.1, t.y + r.2, t.y + r.3))
    }
}

fn clash(a: Bx, b: Bx) -> bool {
    a.0 + 0.25 < b.1 && b.0 + 0.25 < a.1 && a.2 + 0.25 < b.3 && b.2 + 0.25 < a.3
}

fn audit(svg: &str, rep: &Report, g: &mut Glyphs) -> Vec<String> {
    let (w, h) = svg_size(svg);
    let ts = texts(svg);
    let gm = geo(svg);
    let mut bad = Vec::new();
    let inks: Vec<Option<Bx>> = ts.iter().map(|t| g.ink(t)).collect();
    for (t, bx) in ts.iter().zip(&inks) {
        let Some(bx) = bx else { bad.push(format!("{:?} did not measure", t.text)); continue };
        if bx.0 < -0.01 || bx.1 > w + 0.01 || bx.2 < -0.01 || bx.3 > h + 0.01 {
            bad.push(format!("{:?} leaves the {w}x{h} canvas: {bx:?}", t.text));
        }
        if let Some(z) = t.z {
            let Some(c) = rep.cells.iter().find(|c| c.z == z) else { bad.push(format!("{:?}: no cell {z}", t.text)); continue };
            if bx.0 < c.x0 + 0.4 || bx.1 > c.x1 - 0.4 || bx.2 < c.y0 + 0.4 || bx.3 > c.y1 - 0.4 {
                bad.push(format!("{:?} (Z {z}) leaves its cell at {bx:?} in ({}, {}, {}, {})", t.text, c.x0, c.y0, c.x1, c.y1));
            }
        }
    }
    // text against text, only boxes whose x ranges overlap
    let mut order: Vec<usize> = (0..ts.len()).filter(|k| inks[*k].is_some()).collect();
    order.sort_by(|a, b| inks[*a].unwrap().0.total_cmp(&inks[*b].unwrap().0));
    for a in 0..order.len() {
        for b in a + 1..order.len() {
            let (x, y) = (inks[order[a]].unwrap(), inks[order[b]].unwrap());
            if y.0 >= x.1 - 0.01 { break; }
            if clash(x, y) { bad.push(format!("{:?} on {:?}", ts[order[a]].text, ts[order[b]].text)); }
        }
    }
    // text against the trend arrows (the only strokes on the figure) and the key swatches
    let swatches: Vec<Bx> = gm.rects.iter().filter(|r| r.0 == "swatch").map(|r| r.2).collect();
    for (t, bx) in ts.iter().zip(&inks) {
        let Some(bx) = bx else { continue };
        for (_, seg) in &gm.lines {
            if seg_hits(seg.0, seg.1, (bx.0, bx.1, bx.2, bx.3)) { bad.push(format!("{:?} on a line {seg:?}", t.text)); }
        }
        for (_, pts) in &gm.polys {
            for w2 in pts.windows(2) {
                if seg_hits(w2[0], w2[1], (bx.0, bx.1, bx.2, bx.3)) { bad.push(format!("{:?} on an arrow head", t.text)); }
            }
        }
        for s in &swatches {
            if hits((bx.0, bx.1, bx.2, bx.3), (s.0, s.1, s.2, s.3)) { bad.push(format!("{:?} on a key swatch", t.text)); }
        }
    }
    bad
}

fn report(bad: &BTreeMap<String, Vec<String>>, n: usize) {
    assert!(bad.is_empty(), "{} kinds of problem in {n} renders:\n{}", bad.len(),
        bad.values().map(|v| v.join("\n")).collect::<Vec<_>>().join("\n\n"));
}

#[test]
fn names_are_drawn_only_at_large_and_every_cell_label_fits_its_cell() {
    let mut g = Glyphs::new();
    let mut checked = 0;
    for block in BLOCKS {
        for size in ["Small", "Medium", "Large"] {
            for labels in LABELS {
                let (svg, r) = build_of(&[s("block", block), s("label_size", size), s("labels", labels)]);
                let ts = texts(&svg);
                let n = ts.iter().filter(|t| t.kind == "symbol").count();
                assert_eq!(n, r.cells.len(), "{block}/{size}/{labels}: symbols");
                let nums = ts.iter().filter(|t| t.kind == "number").count();
                assert_eq!(nums, if labels == LABELS[1] { 0 } else { r.cells.len() }, "{block}/{size}/{labels}: numbers");
                let names: Vec<&T> = ts.iter().filter(|t| t.kind == "name").collect();
                let want_names = labels == LABELS[2] && size == "Large";
                assert_eq!(names.len(), if want_names { r.cells.len() } else { 0 }, "{block}/{size}/{labels}: names");
                for t in &names {
                    assert!(t.fs > 0.0 && t.fs < 0.2 * r.cell, "name size {} in a {} cell", t.fs, r.cell);
                    assert!(text_w(&t.text, t.fs) <= r.cell - 2.0, "{:?} does not fit a {} cell", t.text, r.cell);
                }
                // every label of a cell is inside that cell, measured with the real glyphs
                for t in ts.iter().filter(|t| t.z.is_some()) {
                    let bx = g.ink(t).expect("a measured glyph");
                    let c = r.cells.iter().find(|c| c.z == t.z.unwrap()).unwrap();
                    assert!(bx.0 >= c.x0 + 0.4 && bx.1 <= c.x1 - 0.4 && bx.2 >= c.y0 + 0.4 && bx.3 <= c.y1 - 0.4,
                        "{block}/{size}/{labels}: {:?} at {bx:?} in ({}, {}, {}, {})", t.text, c.x0, c.y0, c.x1, c.y1);
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 3 * 3 * 3 * 50, "{checked} cell labels measured");
}

fn audit_cases() -> Vec<(&'static str, Vec<(&'static str, Value)>)> {
    let mut v: Vec<(&'static str, Vec<(&'static str, Value)>)> = Vec::new();
    // the full grid of table options: block x labels x size x key x style x answers
    for block in BLOCKS {
        for labels in LABELS {
            for size in ["Small", "Medium", "Large"] {
                for key_on in [true, false] {
                    for style in ["Colour", "Print"] {
                        for ans in [true, false] {
                            v.push(("grid", vec![s("block", block), s("labels", labels), s("label_size", size), b("key", key_on), s("style", style), b(SHOW_ANSWERS, ans)]));
                        }
                    }
                }
            }
        }
    }
    // every trend, style, size and answer state, with an element highlighted in each block
    for block in BLOCKS {
        let hl = if block == BLOCKS[0] { "H" } else if block == BLOCKS[1] { "Au" } else { "La" };
        for trend in TRENDS {
            for size in ["Small", "Medium", "Large"] {
                for style in ["Colour", "Print"] {
                    for ans in [true, false] {
                        v.push(("trend", vec![s("block", block), s("trends", trend), s("label_size", size), s("style", style), b(SHOW_ANSWERS, ans), s("highlight", hl)]));
                    }
                }
            }
        }
    }
    // the highlight variants, including the refusals and the advisory
    for hl in ["Au", "He", "Al", "Br", "Og", "La", "Xx", "auu", "", "119"] {
        for size in ["Small", "Large"] {
            for style in ["Colour", "Print"] {
                for ans in [true, false] {
                    v.push(("highlight", vec![s("block", BLOCKS[2]), s("highlight", hl), s("label_size", size), s("style", style), b(SHOW_ANSWERS, ans)]));
                }
            }
        }
    }
    v
}

#[test]
fn labels_clear_every_text_the_arrows_the_swatches_and_the_canvas() {
    let mut g = Glyphs::new();
    let mut problems: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut n = 0;
    for (name, list) in audit_cases() {
        let p = with(&list);
        let (svg, rep) = build(&p);
        n += 1;
        for m in audit(&svg, &rep, &mut g) {
            let k: String = m.chars().filter(|c| !c.is_ascii_digit()).take(70).collect();
            let e = problems.entry(k).or_default();
            if e.len() < 3 { e.push(format!("{name}/{:?}: {m}", list)); }
        }
    }
    assert!(n >= 300, "{n} renders");
    report(&problems, n);
}

#[test]
fn print_is_grey_only_with_ten_distinct_hatches_that_survive_66_percent() {
    fn grey(h: &str) -> bool {
        let h = h.trim_start_matches('#');
        let h: Vec<char> = if h.len() == 3 { h.chars().flat_map(|c| [c, c]).collect() } else { h.chars().collect() };
        h.len() == 6 && h.iter().all(|c| c.is_ascii_hexdigit()) && {
            let c = |i: usize| u8::from_str_radix(&h[i..i + 2].iter().collect::<String>(), 16).unwrap_or(0);
            c(0) == c(2) && c(2) == c(4)
        }
    }
    for block in BLOCKS {
        for ans in [true, false] {
            let list = [s("block", block), s("style", "Print"), s("labels", LABELS[2]), s("label_size", "Large"), b(SHOW_ANSWERS, ans)];
            let svg = svg_of(&list);
            for k in ["fill", "stroke"] {
                for v in svg.split(&format!(" {k}=\"")).skip(1).map(|x| x.split('"').next().unwrap()) {
                    assert!(v == "none" || !v.starts_with('#') || grey(v), "Print draws a hue: {k}={v}");
                }
            }
            assert_eq!(svg.matches("<pattern").count(), 10, "Print must define ten hatches");
            for w in svg.split(" stroke-width=\"").skip(1).map(|x| x.split('"').next().unwrap().parse::<f64>().unwrap()) {
                assert!(w * 0.66 >= 0.6, "a Print stroke {w} is too thin for 66 %");
            }
        }
    }
    // the key's ten swatches carry ten different hatches after a 66 % bitonal copy
    let list = [s("block", BLOCKS[1]), s("style", "Print")];
    let (svg, r) = build_of(&list);
    assert_eq!(r.swatches.len(), 10);
    let png = to_png(&svg, 0.66).unwrap();
    let img = image::load_from_memory(&png).unwrap().to_rgba8();
    let (iw, ih) = img.dimensions();
    assert!(img.pixels().all(|p| p[0] == p[1] && p[1] == p[2]), "a 66 % Print copy has colour");
    let mut sigs: BTreeSet<Vec<bool>> = BTreeSet::new();
    for (i, sw) in &r.swatches {
        let x0 = ((sw.0 * 0.66).ceil() as u32 + 1).min(iw - 1);
        let x1 = ((sw.1 * 0.66).floor() as u32).saturating_sub(1).min(iw - 1);
        let y0 = ((sw.2 * 0.66).ceil() as u32 + 1).min(ih - 1);
        let y1 = ((sw.3 * 0.66).floor() as u32).saturating_sub(1).min(ih - 1);
        assert!(x1 > x0 + 3 && y1 > y0 + 2, "swatch {i} is too small to test");
        let mut sig = Vec::new();
        let mut dark = 0;
        for y in y0..=y1 {
            for x in x0..=x1 {
                let p = img.get_pixel(x, y);
                let d = p[0] < 150;
                if d { dark += 1; }
                sig.push(d);
            }
        }
        assert!(dark >= 4, "swatch {i} lost its hatch at 66 %");
        assert!(sig.iter().any(|d| !*d), "swatch {i} is solid");
        sigs.insert(sig);
    }
    assert_eq!(sigs.len(), 10, "two category hatches are the same at 66 %");
    // Colour does use distinct colours, and no pattern definitions
    let (csvg, cr) = build_of(&[s("block", BLOCKS[2])]);
    assert!(!csvg.contains("<pattern"));
    let g = geo(&csvg);
    let colours: BTreeSet<String> = g.rects.iter().filter(|r| r.0 == "cell").map(|r| r.1.clone()).collect();
    assert_eq!(colours.len(), 10, "{colours:?}");
    assert_eq!(cr.cells.len(), 118);
}

#[test]
fn the_student_copy_blanks_the_read_out_and_leaks_no_answer() {
    let base = [s("block", BLOCKS[1]), s("highlight", "Au"), b("key", false)];
    let (ksvg, kr) = build_of(&key(&base));
    let (ssvg, sr) = build_of(&student(&base));
    assert_eq!(svg_size(&ksvg), svg_size(&ssvg), "the two copies differ in size");
    assert_eq!(kr.cells.len(), sr.cells.len());
    for (a, b) in kr.cells.iter().zip(&sr.cells) { assert_eq!((a.z, a.x0, a.y0), (b.z, b.x0, b.y0)); }
    let kt = ksvg.split(r#"data-o="answer""#).count();
    assert!(kt >= 2, "the key has no answer text");
    assert!(!ssvg.contains(r#"data-o="answer""#), "the student copy prints an answer-marked text");
    let st = texts(&ssvg);
    assert!(st.iter().filter(|t| t.kind == "blank").count() >= 1, "the student panel has no blank line");
    let blanks: usize = st.iter().map(|t| t.text.matches("______").count()).sum();
    assert!(blanks >= 5, "{blanks} blanks: {:?}", st.iter().filter(|t| t.kind == "blank").map(|t| &t.text).collect::<Vec<_>>());
    let sj = sr.panel.join(" ");
    assert!(sj.contains("Highlighted element: Au"), "{sj}");
    // the name, the Z, the group, the period and the category are nowhere in the source
    for leak in ["Gold", "Z = 79", "group 11", "period 6", "transition metal"] {
        assert!(!ssvg.contains(leak), "the student copy leaks {leak:?}");
    }
    // the key has them all
    let kj = kr.panel.join(" ");
    for want in ["Gold", "Z = 79", "group 11", "period 6", "transition metal"] {
        assert!(kj.contains(want), "the key lacks {want:?}");
    }
    // the table itself is a given and stays on both copies
    assert_eq!(ssvg.matches(r#"data-o="cell""#).count(), kr.cells.len());
    assert!(!ssvg.contains(r#"data-o="swatch""#), "the key was switched off");
    // a main-group element's valence line is blank as well
    let al = build_of(&student(&[s("highlight", "Al")])).1.panel.join(" ");
    assert!(al.contains("Valence shell: ______ electrons."), "{al}");
    assert!(!al.contains("3 electrons"), "{al}");
}

#[test]
fn refusals_name_the_box_the_reason_and_a_fixed_value_and_the_rest_still_draws() {
    let cases: Vec<(&str, Vec<(&'static str, Value)>, &[&str])> = vec![
        ("block", vec![s("block", "All of it")], &["Block \"All of it\"", "Main group is used"]),
        ("labels", vec![s("labels", "Tiny")], &["Labels \"Tiny\"", "Numbers and symbols is used"]),
        ("trends", vec![s("trends", "Density")], &["Trends \"Density\"", "Off is used"]),
        ("highnum", vec![s("highlight", "119")], &["Highlight \"119\"", "not an atomic number from 1 to 118", "no element is highlighted"]),
        ("highzero", vec![s("highlight", "0")], &["Highlight \"0\"", "from 1 to 118", "no element is highlighted"]),
        ("highbad", vec![s("highlight", "Unobtainium")], &["Highlight \"Unobtainium\"", "no element is highlighted", "such as Au"]),
        ("highnumeral", vec![f("highlight", 79.0)], &["Highlight", "no element is highlighted", "such as Au"]),
        ("notshown", vec![s("highlight", "Pd")], &["not in the block shown", "With transition metals"]),
        ("style", vec![s("style", "Sepia")], &["Style \"Sepia\"", "Colour is used"]),
        ("size", vec![s("label_size", "Enormous")], &["Label size \"Enormous\"", "Medium is used"]),
        ("keytype", vec![f("key", 1.0)], &["Category key", "it is shown"]),
    ];
    for (tag, list, wants) in cases {
        let p = raw(&list);
        let notes = warnings(&p);
        for w in wants { assert!(has(&notes, w), "{tag}: {notes:?} lacks {w:?}"); }
        assert_eq!(crate::figures::notes_for(ID, &p), notes, "{tag}");
        let (svg, rep) = build(&p);
        assert!(svg.contains("<svg"), "{tag}");
        // the rest of the figure is still drawn: 50 main-group cells, the key, the panel
        assert_eq!(rep.cells.len(), 50, "{tag}");
        assert_eq!(rep.swatches.len(), 10, "{tag}");
        assert!(svg.contains(r#"data-o="panel-box""#), "{tag}");
        assert!(svg.contains(r#"data-o="symbol""#), "{tag}");
    }
    // a highlighted element that is shown still draws its outline
    let (svg, _) = build_of(&[s("block", BLOCKS[1]), s("highlight", "Pd")]);
    assert!(svg.contains(r#"data-o="highlight" data-z="46""#));
    // and a refused highlight draws none
    let (svg, _) = build(&raw(&[s("highlight", "Xx")]));
    assert!(!svg.contains(r#"data-o="highlight""#));
    // a refused block does not take the highlight away from the fallback block
    let (_, rep) = build(&raw(&[s("block", "nonsense"), s("highlight", "H")]));
    assert_eq!(rep.cells.len(), 50);
    assert_eq!(rep.setup.block, 0);
}

#[test]
fn odd_inputs_never_panic_and_never_print_nan_or_infinity() {
    let bad_vals = [
        Value::F(f64::NAN), Value::F(f64::INFINITY), Value::F(f64::NEG_INFINITY), Value::F(1e300),
        Value::S(String::new()), Value::S("√π".into()), Value::S("x".repeat(10_000)), Value::B(true), Value::I(i64::MAX),
    ];
    for k in ["block", "labels", "key", "trends", "highlight", "style", "label_size", SHOW_ANSWERS] {
        for v in &bad_vals {
            let mut p = raw(&[]);
            p.0.insert(k.to_string(), v.clone());
            for ans in [true, false] {
                p.set_b(SHOW_ANSWERS, ans);
                let (svg, rep) = build(&p);
                assert!(svg.contains("<svg"), "{k} {v:?}");
                assert!(!svg.contains("NaN") && !svg.contains("inf") && !svg.contains("Infinity"), "{k} {v:?}");
                assert!(!rep.panel.iter().any(|l| l.contains("NaN") || l.contains("inf")), "{k} {v:?}: {:?}", rep.panel);
                assert!(!rep.notes.iter().any(|l| l.contains("NaN") || l.contains("inf")), "{k} {v:?}: {:?}", rep.notes);
                for t in texts(&svg) {
                    assert!(!t.text.contains("NaN") && !t.text.contains("inf"), "{k} {v:?}: {:?}", t.text);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Hostile input, purity and cost
// ---------------------------------------------------------------------------------------------

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T { &v[(self.next() as usize) % v.len()] }
}

#[test]
fn ten_thousand_hostile_inputs_render_deterministically() {
    let defs = (def().params)();
    let blocks: Vec<Value> = vec![
        Value::S("Main group".into()), Value::S("With transition metals".into()), Value::S("Full with f-block".into()),
        Value::S("nonsense".into()), Value::S(String::new()), Value::F(f64::NAN), Value::I(2), Value::B(true),
    ];
    let labels: Vec<Value> = vec![
        Value::S("Numbers and symbols".into()), Value::S("Symbols only".into()), Value::S("Add names at Large".into()),
        Value::S("names".into()), Value::S(String::new()), Value::F(0.0), Value::I(-1),
    ];
    let trends: Vec<Value> = vec![
        Value::S("Off".into()), Value::S("Atomic radius".into()), Value::S("Electronegativity".into()),
        Value::S("radius".into()), Value::S(String::new()), Value::B(false),
    ];
    let highlights: Vec<Value> = vec![
        Value::S(String::new()), Value::S("au".into()), Value::S("79".into()), Value::S("0".into()),
        Value::S("119".into()), Value::S("Xx".into()), Value::S("gld".into()), Value::S("A".into()),
        Value::S("zzzz".into()), Value::S("√π".into()), Value::S("x".repeat(10_000)), Value::S("  He  ".into()),
        Value::F(f64::NAN), Value::I(46), Value::B(true),
    ];
    let styles: Vec<Value> = vec![Value::S("Colour".into()), Value::S("Print".into()), Value::S("print".into()), Value::F(1.0)];
    let sizes: Vec<Value> = vec![Value::S("Small".into()), Value::S("Medium".into()), Value::S("Large".into()), Value::S("Huge".into()), Value::I(1)];
    let answers: Vec<Value> = vec![Value::B(true), Value::B(false), Value::S("yes".into()), Value::I(1)];

    let mut r = Rng(0x5eed_1234_9abc_def0);
    let mut drawn = 0usize;
    let mut t = std::time::Instant::now();
    for k in 0..10_000 {
        let mut p = Params::defaults(&defs);
        p.0.insert("block".into(), r.pick(&blocks).clone());
        p.0.insert("labels".into(), r.pick(&labels).clone());
        p.0.insert("trends".into(), r.pick(&trends).clone());
        p.0.insert("highlight".into(), r.pick(&highlights).clone());
        p.0.insert("style".into(), r.pick(&styles).clone());
        p.0.insert("label_size".into(), r.pick(&sizes).clone());
        p.0.insert("key".into(), if r.next() % 2 == 0 { Value::B(true) } else { Value::B(false) });
        p.0.insert(SHOW_ANSWERS.into(), r.pick(&answers).clone());
        let (svg, rep) = build(&p);
        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"), "iteration {k}");
        assert!(svg.len() < 400_000, "iteration {k}: {} bytes", svg.len());
        assert!(!svg.contains("NaN") && !svg.contains("inf") && !svg.contains("Infinity"), "iteration {k}");
        let expected = (1..=118).filter(|z| block_shows(rep.setup.block, *z)).count();
        assert_eq!(rep.cells.len(), expected, "iteration {k}: block {}", rep.setup.block);
        let uniq: BTreeSet<usize> = rep.cells.iter().map(|c| c.z).collect();
        assert_eq!(uniq.len(), expected, "iteration {k}: a duplicate cell");
        assert!(rep.cells.iter().all(|c| c.cat == category(c.z)), "iteration {k}");
        drawn += rep.cells.len();
        if k % 101 == 0 {
            let (again, _) = build(&p);
            assert_eq!(svg, again, "iteration {k} is not pure");
        }
    }
    let dt = t.elapsed();
    println!("periodic fuzz: 10000 inputs, {drawn} cells drawn, {dt:?}");
    assert!(dt < std::time::Duration::from_secs(120), "{dt:?}");
}

#[test]
fn drawing_is_pure_and_within_a_cpu_budget() {
    let base = vec![s("block", BLOCKS[2]), s("labels", LABELS[2]), s("label_size", "Large"), s("highlight", "Au"), s("trends", TRENDS[2])];
    let a = svg_of(&base);
    assert_eq!(a, svg_of(&base));
    assert_eq!(a, svg_of(&base));
    let mut student = base.clone();
    student.push(b(SHOW_ANSWERS, false));
    assert_eq!(svg_size(&a), svg_size(&svg_of(&student)), "the student copy changes the canvas size");
    assert_ne!(a, svg_of(&student));
    let mut print = base.clone();
    print.push(s("style", "Print"));
    assert_ne!(a, svg_of(&print));
    let mut worst = std::time::Duration::ZERO;
    let mut total = std::time::Duration::ZERO;
    let mut n = 0;
    for block in BLOCKS {
        for size in ["Small", "Medium", "Large"] {
            for style in ["Colour", "Print"] {
                for hl in ["Au", "La", ""] {
                    for ans in [true, false] {
                        let p = with(&[s("block", block), s("label_size", size), s("style", style), s("highlight", hl), b(SHOW_ANSWERS, ans)]);
                        let t = std::time::Instant::now();
                        let svg = (def().render)(&p);
                        let dt = t.elapsed();
                        assert!(svg.len() > 9_000);
                        worst = worst.max(dt);
                        total += dt;
                        n += 1;
                    }
                }
            }
        }
    }
    println!("periodic: {n} draws, worst {worst:?}, mean {:?} (debug build)", total / n);
    assert!(worst < std::time::Duration::from_millis(1500), "{worst:?}");
}

#[test]
fn the_description_is_ascii_and_names_no_number_and_no_answer() {
    for list in [vec![], vec![s("block", BLOCKS[1]), s("highlight", "Au"), s("trends", TRENDS[1])], vec![s("block", BLOCKS[2]), s("highlight", "La"), s("trends", TRENDS[2])], student(&[s("highlight", "Al")])] {
        let svg = svg_of(&list);
        let desc = svg.split("<desc>").nth(1).and_then(|x| x.split("</desc>").next()).unwrap_or("").to_string();
        let title = svg.split("<title>").nth(1).and_then(|x| x.split("</title>").next()).unwrap_or("").to_string();
        assert!(desc.is_ascii() && !desc.is_empty() && desc.len() <= 320, "{desc}");
        assert!(title.is_ascii() && !title.is_empty(), "{title}");
        assert!(!desc.chars().any(|c| c.is_ascii_digit()), "{desc}");
        assert!(!desc.contains("Gold") && !desc.contains("Aluminum") && !desc.contains("Lanthanum"), "{desc}");
        assert!(!desc.contains("transition") && !desc.contains("metalloid"), "{desc}");
    }
}

// ---------------------------------------------------------------------------------------------
// Key, trends and the PNG dump
// ---------------------------------------------------------------------------------------------

#[test]
fn the_key_lists_all_ten_categories_and_switches_off() {
    let (on, r) = build_of(&[]);
    assert_eq!(r.swatches.len(), 10);
    let ts = texts(&on);
    let labels: Vec<&str> = ts.iter().filter(|t| t.kind == "key").map(|t| t.text.as_str()).collect();
    assert_eq!(labels, CATS, "{labels:?}");
    let notes = ts.iter().filter(|t| t.kind == "key-note").map(|t| t.text.as_str()).collect::<String>();
    assert!(notes.contains("B, Si, Ge, As, Sb, Te, Po") && notes.contains("Royal Society"), "{notes}");
    let (off, roff) = build_of(&[b("key", false)]);
    assert!(roff.swatches.is_empty());
    assert!(!off.contains(r#"data-o="swatch""#) && !off.contains("Category key"));
    assert!(svg_size(&off).1 < svg_size(&on).1, "switching the key off does not shorten the figure");
    assert_eq!(roff.cells.len(), r.cells.len(), "the table moved when the key went off");
}

#[test]
fn the_trends_are_a_stated_direction_and_do_not_move_the_table() {
    let base = rep_of(&[s("block", BLOCKS[1])]);
    for trend in [TRENDS[1], TRENDS[2]] {
        let (svg, r) = build_of(&[s("block", BLOCKS[1]), s("trends", trend)]);
        let ts = texts(&svg);
        let line: String = ts.iter().filter(|t| t.kind == "trend").map(|t| t.text.as_str()).collect::<Vec<_>>().join(" | ");
        if trend == TRENDS[1] {
            assert!(line.contains("Atomic radius decreases across a period"), "{line}");
            assert!(line.contains("Atomic radius increases down a group"), "{line}");
        } else {
            assert!(line.contains("Electronegativity increases across a period"), "{line}");
            assert!(line.contains("Electronegativity decreases down a group"), "{line}");
        }
        // the arrows are drawn, and the cells have not moved
        let g = geo(&svg);
        assert!(g.lines.len() >= 2, "{trend}: the arrows are missing");
        assert!(g.polys.len() >= 2, "{trend}: no arrow heads");
        assert_eq!(r.cells.len(), base.cells.len());
        for (a, b) in r.cells.iter().zip(&base.cells) { assert_eq!((a.z, a.x0, a.y0), (b.z, b.x0, b.y0)); }
        assert!(svg_size(&svg).1 > 0.0);
    }
    let off = svg_of(&[s("block", BLOCKS[1])]);
    assert!(!off.contains(r#"data-o="trend""#));
    let off_rep = rep_of(&[s("block", BLOCKS[1])]);
    let on_rep = rep_of(&[s("block", BLOCKS[1]), s("trends", TRENDS[2])]);
    assert!(svg_size(&svg_of(&[s("block", BLOCKS[1]), s("trends", TRENDS[2])])).1 > svg_size(&off).1);
    assert_eq!(off_rep.cells.len(), on_rep.cells.len());
}

#[test]
fn periodic_pngs() {
    let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
    let cases: Vec<(&str, Vec<(&'static str, Value)>)> = vec![
        ("defaults", vec![]),
        ("full-medium", vec![s("block", BLOCKS[2])]),
        ("full-large-names", vec![s("block", BLOCKS[2]), s("labels", LABELS[2]), s("label_size", "Large")]),
        ("full-print-large-names", vec![s("block", BLOCKS[2]), s("labels", LABELS[2]), s("label_size", "Large"), s("style", "Print")]),
        ("main-small", vec![s("label_size", "Small")]),
        ("main-large-names", vec![s("labels", LABELS[2]), s("label_size", "Large")]),
        ("gold-key", vec![s("block", BLOCKS[1]), s("highlight", "Au"), s("trends", TRENDS[1])]),
        ("gold-student", vec![s("block", BLOCKS[1]), s("highlight", "Au"), b(SHOW_ANSWERS, false)]),
        ("full-print", vec![s("block", BLOCKS[2]), s("style", "Print")]),
        ("d-trend-en", vec![s("block", BLOCKS[1]), s("trends", TRENDS[2])]),
    ];
    for (tag, list) in cases {
        let svg = svg_of(&list);
        std::fs::write(format!("{dir}/periodic-{tag}.png"), to_png(&svg, 1.5).unwrap()).unwrap();
        std::fs::write(format!("{dir}/periodic-{tag}.svg"), &svg).unwrap();
    }
}
