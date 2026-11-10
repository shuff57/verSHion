//! Tests for "organic-structure": old saved figures and share links, the condensed view written on the drawing and
//! the Carbons view.
use crate::figures::{registry, FigureDef, Params, Value};

const FIG: &str = "organic-structure";
fn def() -> FigureDef { registry().into_iter().find(|d| d.id == FIG).expect("organic-structure is registered") }
fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.into())) }
fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
fn i(k: &'static str, v: i64) -> (&'static str, Value) { (k, Value::I(v)) }
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

const OLD_GROUPS: [&str; 5] = ["None (alkane)", "\u{2013}OH (alcohol)", "\u{2013}COOH (carboxylic acid)", "C=C double bond (alkene)", "C\u{2261}C triple bond (alkyne)"];
const OLD_MODES: [&str; 3] = ["Skeletal", "Condensed", "Full"];

/// Every old skeleton, chain length, group and mode, with the switches alone and together.
fn old_cases() -> Vec<Vec<(&'static str, Value)>> {
    let mut cases: Vec<Vec<(&'static str, Value)>> = vec![vec![]];
    let looks: Vec<Vec<(&'static str, Value)>> = vec![
        vec![],
        vec![b("ends", false), b("numbers", true)],
        vec![b("show_answers", false), s("caption", "Butane")],
        vec![s("label_size", "Large"), b("numbers", true)],
    ];
    for m in OLD_MODES {
        for g in OLD_GROUPS {
            for n in [1, 2, 3, 4, 5, 8, 12] {
                for l in &looks {
                    let mut c = vec![s("mode", m), s("group", g), i("carbons", n)];
                    c.extend(l.iter().cloned());
                    cases.push(c);
                }
            }
        }
        for sk in ["Benzene ring", "Cyclohexane ring", "Cyclopentane ring"] {
            for l in &looks {
                let mut c = vec![s("mode", m), s("skeleton", sk)];
                c.extend(l.iter().cloned());
                cases.push(c);
            }
        }
    }
    cases
}

fn old_fixture() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/organic_structure_old.txt") }

/// Writes the fixture from the build before batch 6 (VERSHION_ORGANIC_OLD_BLESS=1); kept for the record.
#[test]
fn old_saved_figures_fixture_bless() {
    use crate::figures::{share, spec, start_params};
    if !std::env::var("VERSHION_ORGANIC_OLD_BLESS").is_ok_and(|v| v == "1") { return; }
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

/// Every figure saved or shared before batch 6 draws byte for byte as it did.
#[test]
fn old_saved_figures_and_links_render_as_before() {
    use crate::figures::{share, spec};
    let d = def();
    let text = std::fs::read_to_string(old_fixture()).expect("tests/fixtures/organic_structure_old.txt");
    let mut n = 0;
    for line in text.lines().filter(|l| !l.is_empty()) {
        let mut it = line.split('\t');
        let (hash, saved, link) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
        let sp: crate::model::FigureSpec = serde_json::from_str(saved).unwrap();
        let p = spec::params_from_spec(&d, &sp);
        assert_eq!(fnv(&d.svg(&p)), hash, "an old saved figure draws differently: {saved}");
        let (id, changed) = share::decode(link).unwrap();
        assert_eq!(id, FIG);
        let p = share::link_params(&d, changed, share::version(link));
        assert_eq!(fnv(&d.svg(&p)), hash, "an old share link draws differently: {link}");
        n += 1;
    }
    assert!(n >= 400, "only {n} old cases");
}
