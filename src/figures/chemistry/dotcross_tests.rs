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
