//! Half-life graph tests: old figures frozen, and the batch 6 controls (custom half-life unit,
//! counts per minute, guide labels, axis titles, ranges, numbering and minor gridlines) checked
//! against the physics and against what is drawn.
use super::*;
use crate::figures::{registry, Params, Value};

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).expect("half-life-graph is registered") }
fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.into())) }
fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
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
/// FNV-1a, so the hashes in the fixture are the same in any build.
fn fnv(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() { h ^= *b as u64; h = h.wrapping_mul(0x100000001b3); }
    format!("{h:016x}")
}

/// Half-life graphs saved and shared before batch 6 (every old choice and switch, alone and with a
/// few others): their saved spec and share link with the hash of what they drew, written once from
/// main e0d5cd4 with VERSHION_HL_OLD_BLESS=1.
fn old_cases() -> Vec<Vec<(&'static str, Value)>> {
    let mut cases: Vec<Vec<(&'static str, Value)>> = vec![vec![]];
    let mut opts: Vec<(&'static str, Value)> = vec![];
    for c in CHOICES { opts.push(s("iso1", c)); }
    for y in Y_KINDS { opts.push(s("y", y)); }
    for u in UNITS { opts.push(s("unit", u)); }
    for g in 0..=4 { opts.push(i("guides", g)); }
    opts.extend([
        i("count", 2), i("count", 3), f("span", 1.0), f("span", 2.5), f("span", 8.0), f("amount", 50.0), f("amount", 1e12), f("amount", 0.001),
        b("curves", false), b("dots", false), b("daughter", true), b("hide_names", true), s("title", "Decay of carbon-14"),
        b("show_answers", false), s("style", "Print"), s("label_size", "Large"), s("label_size", "Small"),
        s("custom_name", "Zed-1"), f("custom_half", 3.0), f("custom_half", 0.001), f("custom_half", 1e9),
    ]);
    for o in &opts {
        cases.push(vec![o.clone()]);
        cases.push(vec![i("count", 3), s("iso2", "Custom"), s("iso3", "Fluorine-18 (110 minutes)"), b("daughter", true), o.clone()]);
        cases.push(vec![s("y", "Activity"), f("amount", 800.0), s("unit", "Days"), s("iso1", "Iodine-131 (8.02 days)"), i("guides", 4), o.clone()]);
    }
    cases.push(vec![s("iso1", "Custom"), f("custom_half", 3.0), s("custom_name", "Zed-1"), s("y", "Mass remaining"), f("amount", 50.0), b("hide_names", true), s("label_size", "Large")]);
    cases
}

fn old_fixture() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/half_life_old.txt") }

#[test]
fn old_saved_figures_and_links_render_as_before() {
    use crate::figures::{share, spec, start_params};
    let d = def();
    if std::env::var("VERSHION_HL_OLD_BLESS").is_ok_and(|v| v == "1") {
        let mut out = String::new();
        for c in old_cases() {
            let p = with(&c);
            let saved = serde_json::to_string(&spec::spec_for(ID, &p)).unwrap();
            let link = share::encode(ID, &p, &start_params(&d));
            out.push_str(&format!("{}\t{saved}\t{link}\n", fnv(&d.svg(&p))));
        }
        std::fs::write(old_fixture(), out).unwrap();
        return;
    }
    let text = std::fs::read_to_string(old_fixture()).expect("tests/fixtures/half_life_old.txt");
    let mut n = 0;
    for line in text.lines().filter(|l| !l.is_empty()) {
        let mut it = line.split('\t');
        let (hash, saved, link) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
        let sp: crate::model::FigureSpec = serde_json::from_str(saved).unwrap();
        let p = spec::params_from_spec(&d, &sp);
        assert_eq!(fnv(&d.svg(&p)), hash, "an old saved figure draws differently: {saved}");
        let (id, changed) = share::decode(link).unwrap();
        assert_eq!(id, ID);
        let p = share::link_params(&d, changed, share::version(link));
        assert_eq!(fnv(&d.svg(&p)), hash, "an old share link draws differently: {link}");
        n += 1;
    }
    assert!(n >= 150, "only {n} old cases");
}
