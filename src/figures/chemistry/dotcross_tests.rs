//! Tests for "dot-and-cross": old saved figures and share links, typed formulas, the every-ion layout,
//! swapped marks and the on-figure edits (driven through `interact::pick`, as the workspace does).
use crate::figures::{registry, FigureDef, Params, Value};

const FIG: &str = "dot-and-cross";
fn def() -> FigureDef { registry().into_iter().find(|d| d.id == FIG).expect("dot-and-cross is registered") }
fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.into())) }
fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
fn i(k: &'static str, v: i64) -> (&'static str, Value) { (k, Value::I(v)) }
/// The opening state with `list` set on top.
fn with(list: &[(&'static str, Value)]) -> Params {
    let d = def();
    let mut p = crate::figures::start_params(&d);
    for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&(d.params)(), false);
    p
}
fn fnv(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() { h ^= *b as u64; h = h.wrapping_mul(0x100000001b3); }
    format!("{h:016x}")
}

const OLD_MOLECULES: [&str; 11] = ["H2", "Cl2", "HCl", "H2O", "CH4", "NH3", "O2", "N2", "CO2", "C2H4", "C2H6"];
const OLD_IONICS: [&str; 7] = ["NaCl", "MgO", "MgCl2", "CaF2", "Na2O", "Li2O", "Al2O3"];

/// The figures that could be saved or shared before batch 6: every molecule, compound and a spread of atoms and
/// ions, with every old switch alone and together.
fn old_cases() -> Vec<Vec<(&'static str, Value)>> {
    let mut cases: Vec<Vec<(&'static str, Value)>> = vec![vec![]];
    let looks: Vec<Vec<(&'static str, Value)>> = vec![
        vec![],
        vec![s("shells", "Every shell")],
        vec![b("blank", true)],
        vec![b("show_answers", false)],
        vec![s("style", "Print"), s("title", "Bonding")],
        vec![s("label_size", "Large"), s("shells", "Every shell"), b("show_answers", false)],
        vec![s("label_size", "Small"), b("blank", true), s("charges", "Blank")],
    ];
    for m in OLD_MOLECULES {
        for l in &looks {
            let mut c = vec![s("kind", "Molecule"), s("molecule", m)];
            c.extend(l.iter().cloned());
            cases.push(c);
        }
    }
    for x in OLD_IONICS {
        for l in &looks {
            for ch in ["Shown", "Blank", "Off"] {
                let mut c = vec![s("kind", "Ionic compound"), s("ionic", x), s("charges", ch)];
                c.extend(l.iter().cloned());
                cases.push(c);
            }
        }
    }
    for el in ["1 H - Hydrogen", "2 He - Helium", "6 C - Carbon", "8 O - Oxygen", "9 F - Fluorine", "11 Na - Sodium", "12 Mg - Magnesium", "13 Al - Aluminium", "17 Cl - Chlorine", "20 Ca - Calcium", "26 Fe - Iron"] {
        for ch in [-3, -2, -1, 0, 1, 2, 3] {
            for l in [vec![], vec![s("shells", "Every shell")], vec![s("charges", "Blank"), b("blank", true)], vec![s("charges", "Off"), b("show_answers", false)]] {
                let mut c = vec![s("kind", "Atom or ion"), s("element", el), i("charge", ch)];
                c.extend(l);
                cases.push(c);
            }
        }
    }
    cases.push(vec![s("kind", "Atom or ion"), s("element", "1 H - Hydrogen"), i("charge", 4)]);
    cases.push(vec![s("kind", "Atom or ion"), s("element", "8 O - Oxygen"), i("charge", -4)]);
    cases
}

fn old_fixture() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dot_and_cross_old.txt") }

/// Writes the fixture from the build before batch 6 (VERSHION_DOTCROSS_OLD_BLESS=1); kept for the record.
#[test]
fn old_saved_figures_fixture_bless() {
    use crate::figures::{share, spec, start_params};
    if !std::env::var("VERSHION_DOTCROSS_OLD_BLESS").is_ok_and(|v| v == "1") { return; }
    let d = def();
    let mut out = String::new();
    for c in old_cases() {
        let p = with(&c);
        let saved = serde_json::to_string(&spec::spec_for(FIG, &p)).unwrap();
        let link = share::encode(FIG, &p, &start_params(&d));
        out.push_str(&format!("{}\t{saved}\t{link}\n", fnv(&d.svg(&p))));
    }
    std::fs::write(old_fixture(), out).unwrap();
}

/// Old saved figures and share links draw byte for byte as before batch 6, except the drawings that were
/// wrong and are corrected (each kind named in `corrected`), so no other change slips by.
#[test]
fn old_saved_figures_and_links_render_as_before() {
    use crate::figures::{share, spec};
    let d = def();
    let text = std::fs::read_to_string(old_fixture()).expect("tests/fixtures/dot_and_cross_old.txt");
    let (mut same, mut fixed) = (0, Vec::new());
    let mut now: Vec<String> = Vec::new();
    for line in text.lines().filter(|l| !l.is_empty()) {
        let mut it = line.split('\t');
        let (hash, saved, link) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
        let sp: crate::model::FigureSpec = serde_json::from_str(saved).unwrap();
        let p = spec::params_from_spec(&d, &sp);
        let (id, changed) = share::decode(link).unwrap();
        assert_eq!(id, FIG);
        let q = share::link_params(&d, changed, share::version(link));
        assert_eq!(d.svg(&p), d.svg(&q), "a saved figure and its share link draw differently: {saved}");
        now.push(fnv(&d.svg(&p)));
        if fnv(&d.svg(&p)) == hash { same += 1; } else {
            assert!(corrected(&p), "an old saved figure draws differently: {saved}");
            fixed.push(saved.to_string());
        }
    }
    println!("dot-and-cross old cases: {same} the same, {} corrected", fixed.len());
    for f in &fixed { println!("CORRECTED {f}"); }
    assert!(same >= 250, "only {same} old cases draw as before");
    // the corrections are pinned too: every old case must draw exactly as it did once batch 6 was checked, so a later
    // change to an ionic compound or a corrected molecule cannot pass under `corrected` unnoticed (the batch 6 checker).
    // Re-pin deliberately with VERSHION_DOTCROSS_BLESS=1.
    let pinned = old_fixture().with_file_name("dot_and_cross_b6.txt");
    if std::env::var("VERSHION_DOTCROSS_BLESS").is_ok() {
        std::fs::write(&pinned, now.join("\n") + "\n").unwrap();
        return;
    }
    let want = std::fs::read_to_string(&pinned).expect("tests/fixtures/dot_and_cross_b6.txt");
    let want: Vec<&str> = want.lines().collect();
    assert_eq!(want.len(), now.len(), "pinned cases");
    for (i, (w, n)) in want.iter().zip(&now).enumerate() {
        assert_eq!(*w, n, "old case {} draws differently from how batch 6 left it: {}", i + 1, text.lines().filter(|l| !l.is_empty()).nth(i).unwrap());
    }
}

/// The old drawings batch 6 corrects: CO2's second oxygen had carbon's dots as its lone pairs; an anion's gained
/// electrons were drawn in its own mark (now the metal's, and the ionic key's charges are raised); lone pairs
/// placed by reading the free angles as one run from 0 degrees, which put a pair inside the bonded atom's shell
/// (every molecule with lone pairs moves them); shared pairs left on the student's blank diagram; and ethene's
/// and ethane's key, which ran off the canvas.
fn corrected(p: &Params) -> bool {
    let marks = !p.b("blank");
    let answers = p.b("show_answers");
    match p.s("kind") {
        "Molecule" => {
            let m = p.s("molecule");
            (matches!(m, "CO2" | "H2O" | "HCl" | "NH3" | "Cl2" | "O2" | "N2") && marks) || !marks || (answers && matches!(m, "C2H4" | "C2H6"))
        }
        "Ionic compound" => marks || answers,
        _ => false,
    }
}

// ---------------------------------------------------------------------------------------------
// The chemistry, read off what is drawn
// ---------------------------------------------------------------------------------------------

use super::{draw, Drawn, Mark};

fn typed(t: &str) -> Params { with(&[s("formula", t)]) }

/// Electrons drawn in or on atom `i`'s outer shell.
fn in_shell(dr: &Drawn, i: usize) -> usize {
    let a = &dr.symbols[i];
    dr.electrons.iter().filter(|e| e.mark != Mark::Gone && (e.x - a.x).hypot(e.y - a.y) <= a.r + 3.5).count()
}

/// Outer electrons of a neutral atom (main groups, by hand: H 1, C 4, N 5, O 6, halogens 7, Si 4, P 5, S 6).
fn valence_by_hand(z: usize) -> usize {
    match z { 1 => 1, 6 | 14 => 4, 7 | 15 => 5, 8 | 16 => 6, 9 | 17 | 35 | 53 => 7, _ => panic!("no valence for {z}") }
}

const DRAWABLE: [&str; 30] = [
    "H2", "Cl2", "HCl", "H2O", "CH4", "NH3", "O2", "N2", "CO2", "C2H4", "C2H6", "CH3OH", "C2H5OH",
    "HF", "HBr", "HI", "F2", "Br2", "I2", "H2S", "PH3", "SiH4", "CCl4", "CF4", "SiCl4", "NCl3", "PCl3", "OF2", "Cl2O", "HCN",
];
const DRAWABLE2: [&str; 6] = ["CS2", "CH2O", "CH3Cl", "CH2Cl2", "CHCl3", "HOCl"];

/// Every molecule drawn, typed or picked: each atom's shell holds 8 electrons (H 2), each atom contributes its
/// own outer electrons exactly, shared pairs sit in the overlap of both shells and lone pairs on the atom's own
/// shell clear of every other shell.
#[test]
fn every_molecule_has_octets_and_each_atom_its_own_electrons() {
    for f in DRAWABLE.iter().chain(DRAWABLE2.iter()) {
        let dr = draw(&typed(f));
        assert!(dr.ok, "{f} refused");
        for (i, a) in dr.symbols.iter().enumerate() {
            let want = if a.z == 1 { 2 } else { 8 };
            assert_eq!(in_shell(&dr, i), want, "{f}: {} has {} electrons in its shell", a.sym, in_shell(&dr, i));
            let own = dr.electrons.iter().filter(|e| e.atom == i).count();
            assert_eq!(own, valence_by_hand(a.z), "{f}: {} brings {own} electrons", a.sym);
        }
        // a mark inside two shells is a shared one; every mark is inside its owner's shell
        for e in &dr.electrons {
            let o = &dr.symbols[e.atom];
            assert!((e.x - o.x).hypot(e.y - o.y) <= o.r + 3.5, "{f}: an electron of {} lies outside its shell", o.sym);
        }
    }
}

/// The typed formula draws exactly what the picker draws for the listed molecules and compounds.
#[test]
fn typing_a_listed_formula_or_name_draws_the_picked_figure() {
    let d = def();
    for (t, kv) in [
        ("H2O", vec![]), ("water", vec![]), ("  Water ", vec![]), ("H\u{2082}O", vec![]),
        ("CH4", vec![s("molecule", "CH4")]), ("methane", vec![s("molecule", "CH4")]), ("ethene", vec![s("molecule", "C2H4")]),
        ("carbon dioxide", vec![s("molecule", "CO2")]), ("ethanol", vec![s("molecule", "C2H5OH")]), ("CH3OH", vec![s("molecule", "CH3OH")]),
        ("MgCl2", vec![s("kind", "Ionic compound"), s("ionic", "MgCl2")]), ("magnesium chloride", vec![s("kind", "Ionic compound"), s("ionic", "MgCl2")]),
        ("aluminium oxide", vec![s("kind", "Ionic compound"), s("ionic", "Al2O3")]), ("Al2O3", vec![s("kind", "Ionic compound"), s("ionic", "Al2O3")]),
        ("Na", vec![s("kind", "Atom or ion")]), ("sodium", vec![s("kind", "Atom or ion")]), ("sodium atom", vec![s("kind", "Atom or ion")]),
        ("Na+", vec![s("kind", "Atom or ion"), i("charge", 1)]), ("sodium ion", vec![s("kind", "Atom or ion"), i("charge", 1)]),
        ("Mg 2+", vec![s("kind", "Atom or ion"), s("element", "12 Mg - Magnesium"), i("charge", 2)]),
        ("Mg^2+", vec![s("kind", "Atom or ion"), s("element", "12 Mg - Magnesium"), i("charge", 2)]),
        ("Mg\u{b2}\u{207a}", vec![s("kind", "Atom or ion"), s("element", "12 Mg - Magnesium"), i("charge", 2)]),
        ("Mg2+", vec![s("kind", "Atom or ion"), s("element", "12 Mg - Magnesium"), i("charge", 2)]),
        ("Cl-", vec![s("kind", "Atom or ion"), s("element", "17 Cl - Chlorine"), i("charge", -1)]),
        ("Cl\u{2212}", vec![s("kind", "Atom or ion"), s("element", "17 Cl - Chlorine"), i("charge", -1)]),
        ("chloride ion", vec![s("kind", "Atom or ion"), s("element", "17 Cl - Chlorine"), i("charge", -1)]),
        ("O 2-", vec![s("kind", "Atom or ion"), s("element", "8 O - Oxygen"), i("charge", -2)]),
        ("oxide", vec![s("kind", "Atom or ion"), s("element", "8 O - Oxygen"), i("charge", -2)]),
    ] {
        let a = d.svg(&typed(t));
        let b = d.svg(&with(&kv));
        assert_eq!(a, b, "typed {t:?} draws differently from the picked figure");
    }
}

/// Ionic compounds by hand: the ratio from the charges, every ion drawn with its charge in brackets, the anion's
/// gained electrons in the metal's mark and each metal ion left with a full shell.
#[test]
fn ionic_compounds_have_the_ratio_charges_and_transferred_electrons_by_hand() {
    // (typed, metal symbol, metal ions, its charge, anion symbol, anions, its charge)
    for (t, ms, mn, mc, xs, xn, xc) in [
        ("NaCl", "Na", 1, 1, "Cl", 1, -1), ("MgCl2", "Mg", 1, 2, "Cl", 2, -1), ("CaF2", "Ca", 1, 2, "F", 2, -1),
        ("Na2O", "Na", 2, 1, "O", 1, -2), ("Al2O3", "Al", 2, 3, "O", 3, -2), ("Mg3N2", "Mg", 3, 2, "N", 2, -3),
        ("Li3N", "Li", 3, 1, "N", 1, -3), ("AlN", "Al", 1, 3, "N", 1, -3), ("K2S", "K", 2, 1, "S", 1, -2),
        ("SrI2", "Sr", 1, 2, "I", 2, -1), ("RbBr", "Rb", 1, 1, "Br", 1, -1), ("AlF3", "Al", 1, 3, "F", 3, -1),
        ("calcium bromide", "Ca", 1, 2, "Br", 2, -1), ("lithium nitride", "Li", 3, 1, "N", 1, -3), ("potassium sulphide", "K", 2, 1, "S", 1, -2),
    ] {
        let p = with(&[s("formula", t), s("ion_layout", "Every ion")]);
        let dr = draw(&p);
        assert!(dr.ok, "{t} refused");
        let metals: Vec<usize> = (0..dr.symbols.len()).filter(|&k| dr.symbols[k].sym == ms).collect();
        let anions: Vec<usize> = (0..dr.symbols.len()).filter(|&k| dr.symbols[k].sym == xs).collect();
        assert_eq!((metals.len(), anions.len()), (mn, xn), "{t}: ions drawn");
        assert_eq!(dr.charges.len(), mn + xn, "{t}: every ion bracketed with a charge");
        // charges balance
        assert_eq!(mn as i64 * mc + xn as i64 * xc, 0, "{t}");
        for &k in &metals {
            assert_eq!(dr.symbols[k].charge, mc, "{t}");
            // a metal ion keeps the full shell under the one it lost (Li+ its 2, the rest 8), all its own mark
            let want = if ms == "Li" { 2 } else { 8 };
            let es: Vec<_> = dr.electrons.iter().filter(|e| e.atom == k).collect();
            assert_eq!(es.len(), want, "{t}: {ms} ion's shell");
            assert!(es.iter().all(|e| e.mark == Mark::Dot), "{t}");
        }
        for &k in &anions {
            assert_eq!(dr.symbols[k].charge, xc, "{t}");
            let es: Vec<_> = dr.electrons.iter().filter(|e| e.atom == k).collect();
            assert_eq!(es.len(), 8, "{t}: {xs} ion has an octet");
            assert_eq!(es.iter().filter(|e| e.mark == Mark::Dot).count() as i64, -xc, "{t}: gained electrons in the metal's mark");
            assert_eq!(es.iter().filter(|e| e.mark == Mark::Cross).count() as i64, 8 + xc, "{t}: its own electrons");
        }
        // one of each, numbered: two ions with the counts in front
        let q = with(&[s("formula", t)]);
        let one = draw(&q);
        assert_eq!(one.charges.len(), 2, "{t}");
        let svg = crate::figures::chemistry::dotcross::draw(&q).svg;
        for (n, _) in [(mn, ms), (xn, xs)] {
            if n > 1 { assert!(svg.contains(&format!(">{n}</text>")), "{t}: {n} in front"); }
        }
    }
}

/// What can't be drawn is refused with a note, never drawn wrong; the panel gets the same note.
#[test]
fn what_cannot_be_drawn_is_refused_with_a_note() {
    for (t, says) in [
        ("CO", "dative"), ("carbon monoxide", "dative"), ("SO2", "dative"), ("sulfur dioxide", "dative"), ("PCl5", "more than an octet"),
        ("BF3", "B isn't drawn"), ("H2O2", "central atom"), ("hydrogen peroxide", "central atom"), ("NH4+", "more than one atom"), ("OH-", "more than one atom"),
        ("MgCl", "MgCl\u{2082}"), ("Na2Cl2", "NaCl"), ("Fe", "H to Ca"), ("Br-", "H to Ca"), ("bromine atom", "H to Ca"), ("Na 5+", "charge"),
        ("NaClO", "one metal"), ("C3H8", "central atom"), ("h2o", "capital"), ("Xx", "isn't an element"), ("rubidium ion", "H to Ca"),
        ("Na2", "one metal"), ("C2H6O", "C2H5OH"), ("C2", "quadruple"), ("Si2", "quadruple"), ("SiC", "quadruple"),
    ] {
        let p = typed(t);
        let dr = draw(&p);
        assert!(!dr.ok, "{t:?} was drawn");
        assert!(dr.electrons.is_empty() && dr.symbols.is_empty(), "{t:?}");
        let w = super::warnings(&p);
        assert!(w.len() == 1 && w[0].contains(says), "{t:?}: note {w:?} lacks {says:?}");
        let flat = dr.svg.replace("&#x27;", "'").replace("&apos;", "'");
        assert!(flat.contains("drawn") || flat.contains("type") || flat.contains("Type") || flat.contains("is ") || flat.contains("isn"), "{t:?}: the figure shows the note");
        let hs = def().handles(&p, (dr.w, dr.h));
        assert!(hs.is_empty(), "{t:?}: nothing to click on a refusal");
    }
}

/// Swap trades every dot for a cross and back, and nothing else.
#[test]
fn swap_trades_dots_and_crosses() {
    for f in ["H2O", "CH3OH", "MgCl2", "Na", "N2"] {
        let a = draw(&typed(f));
        let b = draw(&with(&[s("formula", f), b("swap", true)]));
        assert_eq!(a.electrons.len(), b.electrons.len());
        for (x, y) in a.electrons.iter().zip(&b.electrons) {
            let want = match x.mark { Mark::Dot => Mark::Cross, Mark::Cross => Mark::Dot, m => m };
            assert_eq!(y.mark, want, "{f}");
            assert_eq!((x.x, x.y), (y.x, y.y));
        }
        assert_ne!(a.svg, b.svg, "{f}");
    }
}

// ---------------------------------------------------------------------------------------------
// Clicking the figure, through the workspace's own hit test
// ---------------------------------------------------------------------------------------------

use crate::figures::interact::pick;

/// Clicks `pt` the way the workspace does: the handle `pick` chooses, then its click.
fn click_at(p: &mut Params, pt: (f64, f64), k: f64) -> bool {
    let d = def();
    let size = crate::figures::svg_size(&(d.render)(p));
    let hs = d.handles(p, size);
    let Some(h) = pick(&hs, pt, k) else { return false };
    if let Some(f) = &hs[h].click_at { f(p, pt); return true; }
    false
}

fn layouts() -> Vec<Params> {
    vec![
        with(&[]),
        typed("CH4"),
        typed("CO2"),
        typed("N2"),
        typed("CCl4"),
        typed("ethanol"),
        with(&[s("formula", "MgCl2"), s("ion_layout", "Every ion")]),
        with(&[s("formula", "Al2O3"), s("ion_layout", "Every ion"), s("label_size", "Large")]),
        with(&[s("kind", "Ionic compound"), s("ionic", "CaF2")]),
        with(&[s("formula", "Ca"), s("shells", "Every shell")]),
        with(&[s("formula", "Cl-"), s("shells", "Every shell"), s("label_size", "Large")]),
        with(&[s("formula", "H2O"), s("shells", "Every shell"), s("title", "Water")]),
    ]
}

/// A click on (or within 2 px of) any electron cycles that electron and only it: dot, cross, open circle, open
/// square, nothing, then back, and the edits box is empty again after the full cycle.
#[test]
fn clicking_an_electron_cycles_it_through_the_marks() {
    for base in layouts() {
        for k in [0.8, 1.0, 1.6] {
            let dr = draw(&base);
            for (i, e) in dr.electrons.iter().enumerate() {
                for (dx, dy) in [(0.0, 0.0), (1.5, -1.5), (-2.0, 1.0), (0.0, 2.0)] {
                    let mut p = base.clone();
                    let mut m = e.mark;
                    for _ in 0..5 {
                        assert!(click_at(&mut p, (e.x + dx, e.y + dy), k), "electron {i} at {:?} not clickable (k {k})", (e.x, e.y));
                        let after = draw(&p);
                        m = m.next();
                        assert_eq!(after.electrons[i].mark, m, "electron {i} of {} (k {k})", base.s("formula"));
                        for (j, o) in after.electrons.iter().enumerate() {
                            if j != i { assert_eq!(o.mark, dr.electrons[j].mark, "electron {j} changed by a click on {i}"); }
                        }
                    }
                    assert_eq!(p.s("edits"), "", "a full cycle leaves no edit");
                }
            }
        }
    }
}

/// A click on a symbol (anywhere in its box away from the electrons) blanks it and a second click shows it.
#[test]
fn clicking_a_symbol_blanks_and_shows_it() {
    for base in layouts() {
        let dr = draw(&base);
        for sy in &dr.symbols {
            let mut tried = 0;
            for fx in [-0.8, -0.3, 0.0, 0.4, 0.8] {
                for fy in [-0.7, 0.0, 0.7] {
                    let pt = (sy.x + fx * sy.hw, sy.y + fy * sy.hh);
                    if dr.electrons.iter().any(|e| (e.x - pt.0).hypot(e.y - pt.1) <= 6.0) { continue; }
                    tried += 1;
                    let mut p = base.clone();
                    let n0 = draw(&p).svg.matches(&format!(">{}</text>", sy.sym)).count();
                    assert!(click_at(&mut p, pt, 1.0), "{} not clickable", sy.sym);
                    let after = draw(&p);
                    assert!(!after.symbols[sy.atom].shown, "{} not blanked", sy.sym);
                    assert_eq!(after.svg.matches(&format!(">{}</text>", sy.sym)).count(), n0 - 1, "{} still drawn", sy.sym);
                    assert!(click_at(&mut p, pt, 1.0));
                    assert!(draw(&p).symbols[sy.atom].shown);
                    assert_eq!(p.s("edits"), "");
                }
            }
            assert!(tried >= 5, "{}: only {tried} points of its box are free", sy.sym);
        }
    }
}

/// "dot-and-cross": a click on one ion's charge shows it, makes it a blank box or leaves it off, that ion only.
#[test]
fn clicking_an_ions_charge_cycles_it_for_that_ion_only() {
    for base in [
        with(&[s("formula", "MgCl2"), s("ion_layout", "Every ion")]),
        with(&[s("kind", "Ionic compound"), s("ionic", "Al2O3")]),
        with(&[s("formula", "Mg 2+")]),
        with(&[s("formula", "Al2O3"), s("ion_layout", "Every ion"), s("charges", "Blank"), s("label_size", "Large")]),
    ] {
        let dr = draw(&base);
        assert!(!dr.charges.is_empty());
        for c in &dr.charges {
            for k in [1.0, 1.6] {
                let mut p = base.clone();
                let mut st = c.state;
                for _ in 0..3 {
                    assert!(click_at(&mut p, (c.x, c.y), k), "charge on ion {} not clickable", c.ion);
                    st = (st + 1) % 3;
                    let after = draw(&p);
                    for o in &after.charges {
                        let want = if o.ion == c.ion { st } else { o.base };
                        assert_eq!(o.state, want, "ion {} after a click on ion {}", o.ion, c.ion);
                    }
                    // drawn: a shown charge is text, a blank one a box, an off one nothing
                    let blank_boxes = after.svg.matches("fill=\"none\" stroke=\"#111\" stroke-width=\"1.4\"").count();
                    let _ = blank_boxes;
                }
                assert_eq!(p.s("edits"), "");
            }
        }
    }
    // what is drawn: Shown text, a Blank box, Off nothing, for the one ion clicked
    let mut p = with(&[s("formula", "Mg 2+")]);
    let c = draw(&p).charges[0].clone();
    let shown = def().svg(&p);
    assert!(shown.contains(">2\u{207a}</text>"));
    click_at(&mut p, (c.x, c.y), 1.0);
    let blank = def().svg(&p);
    assert!(!blank.contains(">2\u{207a}</text>") && blank.matches("<rect").count() > shown.matches("<rect").count(), "blank box");
    click_at(&mut p, (c.x, c.y), 1.0);
    let off = def().svg(&p);
    assert!(!off.contains(">2\u{207a}</text>") && off.matches("<rect").count() == shown.matches("<rect").count(), "off");
}

/// The opening state offers the electron and symbol clicks with their own tips; blank electrons can't be clicked.
#[test]
fn the_opening_state_offers_the_clicks() {
    let d = def();
    let p = crate::figures::start_params(&d);
    let size = crate::figures::svg_size(&(d.render)(&p));
    let tips: Vec<String> = d.handles(&p, size).iter().map(|h| h.tip.clone()).collect();
    assert!(tips.iter().any(|t| t.contains("Click an electron to change it")), "{tips:?}");
    assert_eq!(tips.iter().filter(|t| t.contains("Click the element symbol")).count(), 3, "{tips:?}");
    let q = with(&[b("blank", true)]);
    let tips: Vec<String> = d.handles(&q, size).iter().map(|h| h.tip.clone()).collect();
    assert!(!tips.iter().any(|t| t.contains("electron")), "{tips:?}");
    let q = with(&[s("formula", "MgCl2")]);
    let tips: Vec<String> = d.handles(&q, size).iter().map(|h| h.tip.clone()).collect();
    assert_eq!(tips.iter().filter(|t| t.contains("Click the ion's charge")).count(), 2, "{tips:?}");
}

/// The student copy leaks no key, in the drawing or in the SVG's title and description; edits stay (they are the
/// question) and the key, when shown, still describes the correct diagram.
#[test]
fn the_student_copy_has_no_key_and_the_key_ignores_edits() {
    let d = def();
    for f in ["H2O", "ethanol", "MgCl2", "Mg 2+", "CCl4"] {
        let p = with(&[s("formula", f), b("show_answers", false), s("edits", "e0=2 e3=4 s0=0 q0=1")]);
        let svg = d.svg(&p);
        for leak in ["lone pair", "outer electron", "bonding", "loses electrons", "single bond", "double bond", "): shells"] {
            assert!(!svg.contains(leak), "{f}: student copy shows {leak:?}");
        }
        let head = &svg[..svg.find("</desc>").unwrap_or(0)];
        assert!(head.contains(d.title) && head.contains(d.blurb), "{f}: title/desc are the figure's own");
        let with_key = with(&[s("formula", f), s("edits", "e0=2 e3=4 s0=0 q0=1")]);
        let plain = with(&[s("formula", f)]);
        let key = |svg: &str| svg[svg.rfind("<text").unwrap()..].to_string();
        let _ = key;
        let (a, b2) = (d.svg(&with_key), d.svg(&plain));
        // the key's lines are the same with and without edits
        let lines = |s: &str| s.split("<text").skip(1).filter(|t| t.contains("bond") || t.contains("electron") || t.contains("shells") || t.contains("gains")).map(String::from).collect::<Vec<_>>();
        assert_eq!(lines(&a), lines(&b2), "{f}: the key follows the edits");
        assert!(!lines(&a).is_empty(), "{f}: no key");
    }
}

/// A pick from the lists replaces the typed text; a new structure clears the clicked changes, a title does not.
#[test]
fn panel_changes_clear_edits_and_typed_text() {
    let d = def();
    let mut p = with(&[s("formula", "CCl4"), s("edits", "e0=1")]);
    let old = p.0.get("title").cloned().unwrap();
    crate::figures::after_panel_change(d.id, "title", &old, &mut p);
    assert_eq!((p.s("formula"), p.s("edits")), ("CCl4", "e0=1"));
    let old = p.0.get("ion_layout").cloned().unwrap();
    crate::figures::after_panel_change(d.id, "ion_layout", &old, &mut p);
    assert_eq!((p.s("formula"), p.s("edits")), ("CCl4", ""));
    p.set_s("edits", "e0=1");
    let old = p.0.get("molecule").cloned().unwrap();
    p.set_s("molecule", "NH3");
    crate::figures::after_panel_change(d.id, "molecule", &old, &mut p);
    assert_eq!((p.s("formula"), p.s("edits")), ("", ""));
    // the layout switch shows only for an ionic compound
    assert!(!crate::figures::param_relevant(d.id, "ion_layout", &with(&[])));
    assert!(crate::figures::param_relevant(d.id, "ion_layout", &with(&[s("formula", "lithium nitride")])));
    assert!(crate::figures::param_relevant(d.id, "ion_layout", &with(&[s("kind", "Ionic compound")])));
    // another figure's controls are untouched by this hook
    let bar = registry().into_iter().find(|x| x.id == "bar-chart").unwrap();
    let bp = crate::figures::start_params(&bar);
    for pd in (bar.params)() { let _ = crate::figures::param_relevant(bar.id, pd.key, &bp); }
}

/// Text stays on the page and readable at the ends of the ranges: the most ions, Large labels, a title and the key.
#[test]
fn the_biggest_figures_keep_their_text_on_the_page() {
    let d = def();
    for list in [
        vec![s("formula", "Mg3N2"), s("ion_layout", "Every ion"), s("label_size", "Large"), s("title", "Magnesium nitride, every ion drawn"), s("shells", "Every shell")],
        vec![s("formula", "Sr3N2"), s("ion_layout", "Every ion"), s("label_size", "Large")],
        vec![s("formula", "SrI2"), s("ion_layout", "Every ion"), s("shells", "Every shell"), s("label_size", "Large")],
        vec![s("formula", "C2H5OH"), s("label_size", "Large"), s("title", "Ethanol")],
        vec![s("formula", "SiCl4"), s("label_size", "Large"), s("title", "Silicon tetrachloride"), s("shells", "Every shell")],
        vec![s("formula", "CHCl3"), s("label_size", "Large")],
        vec![s("formula", "Na2Cl2"), s("label_size", "Large"), s("title", "x")],
        vec![s("kind", "Molecule"), s("molecule", "C2H6"), s("label_size", "Large")],
    ] {
        let p = with(&list);
        let svg = d.svg(&p);
        let (w, h) = crate::figures::svg_size(&svg);
        for (k, _) in svg.match_indices("<text ") {
            let tag = &svg[k..k + svg[k..].find('>').unwrap()];
            let num = |n: &str| tag.split(&format!(" {n}=\"")).nth(1).and_then(|r| r.split('"').next()).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
            let (x, y, fs) = (num("x"), num("y"), num("font-size"));
            let body_start = k + svg[k..].find('>').unwrap() + 1;
            let body = &svg[body_start..body_start + svg[body_start..].find("</text>").unwrap()];
            let plain: String = { let mut o = String::new(); let mut inside = false; for c in body.chars() { if c == '<' { inside = true } else if c == '>' { inside = false } else if !inside { o.push(c) } } o };
            let tw = plain.chars().count() as f64 * fs * 0.56;
            let anchor = tag.split("text-anchor=\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("start");
            let (x0, x1) = match anchor { "middle" => (x - tw / 2.0, x + tw / 2.0), "end" => (x - tw, x), _ => (x, x + tw) };
            assert!(fs >= 8.0, "{plain:?} at {fs} px");
            assert!(x0 >= 0.0 && x1 <= w + 0.5 && y - fs * 0.8 >= 0.0 && y <= h, "{:?}: {plain:?} at ({x0:.0}..{x1:.0}, {y:.0}) leaves the {w}x{h} page", list[0]);
        }
    }
}
