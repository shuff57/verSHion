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

fn old_fixture() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/nuclide_old.txt") }

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
