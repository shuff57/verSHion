//! Batch 5 tests: the figures saved and shared before batch 5 still draw exactly what they drew, and the batch's own controls.
use super::*;

pub(super) fn def() -> FigureDef { registry().into_iter().find(|d| d.id == "circle").unwrap() }
pub(super) fn with(list: &[(&'static str, Value)]) -> Params {
    let d = def();
    let mut p = start_params(&d);
    for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&(d.params)(), false);
    p
}
pub(super) fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
pub(super) fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
pub(super) fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }

fn fnv(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() { h ^= *b as u64; h = h.wrapping_mul(0x100000001b3); }
    format!("{h:016x}")
}

/// The figures a teacher could make before batch 5: every example, every switch and choice on its own and over a busy figure,
/// the editor's lists (more points, lines, tangents, labels, shaded areas, in every form they read), and a sweep of point
/// positions. Their saved spec (every key) and their share link (the keys that differ from the opening state) are written
/// once from the build before batch 5 (VERSHION_CIRCLE_OLD_BLESS=1), with the hash of what they drew.
fn old_cases() -> Vec<Vec<(&'static str, Value)>> {
    let mut cases: Vec<Vec<(&'static str, Value)>> = vec![vec![]];
    for e in examples("circle").unwrap() { cases.push(e.params); }
    let busy = || vec![b("show_chord", true), b("show_bisector", true), b("show_tangent", true), b("show_secant", true), b("show_central", true), b("show_inscribed", true), s("arc", "Minor"), s("sector", "Minor"), b("m_arc", true), b("m_sector", true), b("m_chord", true)];
    for pd in (def().params)() {
        match &pd.kind {
            Kind::Bool => {
                let dflt = matches!(pd.default, Value::B(true));
                cases.push(vec![(pd.key, Value::B(!dflt))]);
                let mut v = busy();
                let cur = busy().iter().find(|x| x.0 == pd.key).map(|x| matches!(x.1, Value::B(true))).unwrap_or(dflt);
                v.retain(|x| x.0 != pd.key);
                v.push((pd.key, Value::B(!cur)));
                cases.push(v);
            }
            Kind::Choice(opts) => for o in opts {
                cases.push(vec![(pd.key, Value::S(o.to_string()))]);
                let mut v = busy();
                v.push((pd.key, Value::S(o.to_string())));
                cases.push(v);
            },
            _ => {}
        }
    }
    let lists: Vec<Vec<(&'static str, Value)>> = vec![
        vec![s("more_points", "D 270, E 150")],
        vec![s("more_points", "D 270; 45; X 10; 300")],
        vec![s("more_points", "D270, P 1 30, E, -90")],
        vec![s("more_points", "D 270 1.5, E 100 2")],
        vec![s("more_points", "D 270"), s("joins", "AD, OD, DM")],
        vec![b("show_b", true), s("more_points", "D 270"), s("joins", "AD, BD, OD"), s("marks", "arc BD degrees, BD length, angle ADB, AD x")],
        vec![b("show_b", true), b("show_c", true), s("tangents", "B, C"), s("marks", "tangent B, tangent C t1, tangent angle BA")],
        vec![b("show_b", true), s("tangents", "B"), b("show_tangent", true), s("shade", "at 1.2 0.6")],
        vec![b("show_b", true), s("more_points", "D 200"), s("shade", "segment BD, sector AB, major segment AD")],
        vec![b("show_b", true), s("marks", "major arc AB length, arc AB 70\u{b0}")],
        vec![f("a1", 210.0), f("a2", 330.0), s("tangents", "B"), b("show_tangent", true), b("show_b", true)],
        vec![b("show_b", true), b("show_c", true), s("more_points", "D 300"), s("joins", "AC, BD"), s("shade", "at 0.1 0.1")],
        vec![b("show_chord", true), s("marks", "OA x, angle OAB, angle OAB 40\u{b0}")],
        vec![b("show_b", true), s("marks", "arc AB x, arc AB length, arc AB degrees"), s("label_size", "Large")],
        vec![b("show_b", true), s("joins", "OB"), s("marks", "OB length"), s("style", "Print")],
    ];
    cases.extend(lists);
    let mut seed: u64 = 0x5eed_c1c1e;
    let mut rnd = || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; seed };
    for _ in 0..20 {
        let mut v = vec![f("a1", (rnd() % 360) as f64), f("a2", (rnd() % 360) as f64), f("c_angle", (rnd() % 360) as f64)];
        for k in ["show_chord", "show_bisector", "show_tangent", "show_secant", "show_central", "show_inscribed", "show_diameter"] { if rnd() % 2 == 0 { v.push(b(k, true)); } }
        if rnd() % 3 == 0 { v.push(s("arc", "Minor")); }
        if rnd() % 3 == 0 { v.push(s("sector", "Major")); }
        if rnd() % 2 == 0 { v.push(b(SHOW_ANSWERS, false)); }
        cases.push(v);
    }
    cases
}

fn old_fixture() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/circle_old.txt") }

#[test]
fn old_saved_figures_and_links_render_as_before() {
    use crate::figures::{share, spec};
    let d = def();
    if std::env::var("VERSHION_CIRCLE_OLD_BLESS").is_ok_and(|v| v == "1") {
        let mut out = String::new();
        for c in old_cases() {
            let p = with(&c);
            let saved = serde_json::to_string(&spec::spec_for("circle", &p)).unwrap();
            let link = share::encode("circle", &p, &start_params(&d));
            out.push_str(&format!("{}\t{saved}\t{link}\n", fnv(&d.svg(&p))));
        }
        std::fs::write(old_fixture(), out).unwrap();
        return;
    }
    let text = std::fs::read_to_string(old_fixture()).expect("tests/fixtures/circle_old.txt");
    let mut n = 0;
    for line in text.lines().filter(|l| !l.is_empty()) {
        let mut it = line.split('\t');
        let (hash, saved, link) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
        let sp: crate::model::FigureSpec = serde_json::from_str(saved).unwrap();
        let p = spec::params_from_spec(&d, &sp);
        assert_eq!(fnv(&d.svg(&p)), hash, "an old saved figure draws differently: {saved}");
        let (id, changed) = share::decode(link).unwrap();
        assert_eq!(id, "circle");
        let p = share::link_params(&d, changed, share::version(link));
        assert_eq!(fnv(&d.svg(&p)), hash, "an old share link draws differently: {link}");
        n += 1;
    }
    assert!(n >= 150, "only {n} old cases");
}
