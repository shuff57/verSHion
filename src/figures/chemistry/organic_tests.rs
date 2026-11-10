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

// ---------------------------------------------------------------------------------------------
// Carbons and Groups, read off what is drawn
// ---------------------------------------------------------------------------------------------

/// Every text of the SVG: (x, y, font size, text with subscripts written as _n, bold).
fn texts(svg: &str) -> Vec<(f64, f64, f64, String)> {
    let mut out = vec![];
    for (k, _) in svg.match_indices("<text ") {
        let end = k + svg[k..].find('>').unwrap();
        let tag = &svg[k..end];
        let num = |n: &str| tag.split(&format!(" {n}=\"")).nth(1).and_then(|r| r.split('"').next()).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
        let body = &svg[end + 1..end + 1 + svg[end + 1..].find("</text>").unwrap()];
        let mut text = String::new();
        let mut rest = body;
        while let Some(i) = rest.find("<tspan class=\"sub\"") {
            text.push_str(&rest[..i]);
            text.push('_');
            rest = &rest[i + rest[i..].find('>').unwrap() + 1..];
        }
        text.push_str(rest);
        // the other tags (the run raised back after a subscript) carry no text
        let mut plain = String::new();
        let mut inside = false;
        for c in text.chars() { if c == '<' { inside = true } else if c == '>' { inside = false } else if !inside { plain.push(c) } }
        let text = plain;
        out.push((num("x"), num("y"), num("font-size"), text));
    }
    out
}

/// The group written at each carbon, left to right (skipping the O and OH labels, numbers and the key).
fn carbon_labels(svg: &str) -> Vec<String> {
    let mut t: Vec<(f64, f64, f64, String)> = texts(svg).into_iter().filter(|t| t.3.contains('C') && !t.3.contains("Molecular") && !t.3.contains('O')).collect();
    t.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    t.into_iter().map(|t| t.3).collect()
}

const ALKANE: &str = "None (alkane)";
const OH: &str = "\u{2013}OH (alcohol)";
const COOH: &str = "\u{2013}COOH (carboxylic acid)";
const ENE: &str = "C=C double bond (alkene)";
const YNE: &str = "C\u{2261}C triple bond (alkyne)";

/// Groups writes each carbon's hydrogens where it sits, 4 minus its bonds, checked by hand, with the
/// subscripts drawn lowered (not as Unicode subscript digits).
#[test]
fn groups_write_each_carbons_hydrogens_by_hand() {
    for (g, n, want) in [
        (ALKANE, 4, vec!["H_3C", "CH_2", "CH_2", "CH_3"]),
        (ALKANE, 2, vec!["H_3C", "CH_3"]),
        (ALKANE, 1, vec!["CH_4"]),
        (ENE, 3, vec!["H_2C", "CH", "CH_3"]),
        (ENE, 2, vec!["H_2C", "CH_2"]),
        (YNE, 3, vec!["HC", "C", "CH_3"]),
        (YNE, 2, vec!["HC", "CH"]),
        (OH, 3, vec!["H_3C", "CH_2", "CH_2"]),
        (OH, 1, vec!["CH_3"]),
        (COOH, 2, vec!["H_3C", "C"]),
        (COOH, 1, vec!["CH"]),
    ] {
        let svg = def().svg(&with(&[s("mode", "Groups"), s("group", g), i("carbons", n)]));
        assert_eq!(carbon_labels(&svg), want, "{g} with {n} carbons");
        assert!(!svg.contains('\u{2082}') && !svg.contains('\u{2083}') || svg.contains("Molecular formula"), "{g}: Unicode subscripts on the drawing");
        let all = texts(&svg).into_iter().map(|t| t.3).collect::<Vec<_>>();
        match g {
            OH => assert!(all.iter().any(|t| t == "OH"), "{g}"),
            COOH => assert!(all.iter().any(|t| t == "OH") && all.iter().any(|t| t == "O"), "{g}"),
            _ => {}
        }
    }
    // every chain: the hydrogens written at the carbons, plus the -OH hydrogen, are the molecule's hydrogens
    for g in [ALKANE, OH, COOH, ENE, YNE] {
        for n in 1..=12 {
            if n < 2 && matches!(g, ENE | YNE) { continue; }
            let p = with(&[s("mode", "Groups"), s("group", g), i("carbons", n)]);
            let svg = def().svg(&p);
            let written: usize = carbon_labels(&svg).iter().map(|l| {
                if !l.contains('H') { 0 } else { l.split('_').nth(1).and_then(|r| r.chars().next()).and_then(|c| c.to_digit(10)).unwrap_or(1) as usize }
            }).sum();
            let oh = usize::from(matches!(g, OH | COOH));
            let (_, h, _) = super::atom_count(&p);
            assert_eq!(written + oh, h, "{g} with {n} carbons: {written} H written");
            assert_eq!(carbon_labels(&svg).len() as i64, n, "{g} {n}: a group at every carbon");
        }
    }
}

/// Carbons: C at every carbon of the skeleton and no hydrogen anywhere on the drawing.
#[test]
fn carbons_write_c_at_every_carbon_and_no_hydrogen() {
    for (sk, g, n) in [("Straight chain", ALKANE, 4), ("Straight chain", ALKANE, 12), ("Straight chain", OH, 3), ("Straight chain", ENE, 5), ("Straight chain", COOH, 2), ("Benzene ring", ALKANE, 4), ("Cyclohexane ring", ALKANE, 4), ("Cyclopentane ring", ALKANE, 4)] {
        let p = with(&[s("mode", "Carbons"), s("skeleton", sk), s("group", g), i("carbons", n), b("show_answers", false)]);
        let svg = def().svg(&p);
        let (c, _, _) = super::atom_count(&p);
        let t = texts(&svg);
        assert_eq!(t.iter().filter(|x| x.3 == "C").count(), c, "{sk} {g} {n}");
        assert!(t.iter().all(|x| !x.3.contains('H') || x.3 == "OH"), "{sk} {g} {n}: a hydrogen is drawn");
        // the bonds are still drawn
        assert!(svg.matches("<line").count() >= c.saturating_sub(1), "{sk}: bonds");
    }
    // rings in Groups: benzene CH at each corner, the others CH2
    for (sk, want, n) in [("Benzene ring", "CH", 6), ("Cyclohexane ring", "CH_2", 6), ("Cyclopentane ring", "CH_2", 5)] {
        let svg = def().svg(&with(&[s("mode", "Groups"), s("skeleton", sk)]));
        assert_eq!(texts(&svg).iter().filter(|x| x.3 == want).count(), n, "{sk}");
    }
}

/// No bond runs into a label: every line end is outside every group's box, at Medium and Large.
#[test]
fn bonds_stop_short_of_the_labels() {
    for size in ["Medium", "Large", "Small"] {
        for (mode, g, n, sk) in [("Groups", ALKANE, 12, "Straight chain"), ("Groups", COOH, 3, "Straight chain"), ("Carbons", OH, 6, "Straight chain"), ("Groups", ENE, 4, "Straight chain"), ("Groups", ALKANE, 4, "Benzene ring"), ("Carbons", ALKANE, 4, "Cyclopentane ring")] {
            let svg = def().svg(&with(&[s("mode", mode), s("group", g), i("carbons", n), s("skeleton", sk), s("label_size", size)]));
            let boxes: Vec<(f64, f64, f64, f64)> = texts(&svg).into_iter().filter(|t| t.3.contains('C') || t.3.starts_with('O')).map(|(x, y, fs, t)| {
                let w: f64 = t.split('_').enumerate().map(|(k, part)| if k == 0 { part.chars().count() as f64 * fs * 0.62 } else { 0.7 * fs * 0.62 + (part.chars().count() as f64 - 1.0) * fs * 0.62 }).sum();
                (x, y - fs * 0.35, w / 2.0 - 1.0, fs * 0.4)
            }).collect();
            for (k, _) in svg.match_indices("<line ") {
                let tag = &svg[k..k + svg[k..].find("/>").unwrap()];
                let num = |n: &str| tag.split(&format!(" {n}=\"")).nth(1).and_then(|r| r.split('"').next()).and_then(|v| v.parse::<f64>().ok()).unwrap();
                for (px, py) in [(num("x1"), num("y1")), (num("x2"), num("y2"))] {
                    for &(bx, by, hw, hh) in &boxes {
                        assert!(!((px - bx).abs() < hw && (py - by).abs() < hh), "{mode} {g} {n} {size}: a bond ends inside a label at ({px:.0},{py:.0})");
                    }
                }
            }
        }
    }
}

/// Text stays on the page and labels never overlap, at the ends of the ranges (12 carbons, Large, numbers, caption).
/// The formula key under the drawing is the one every mode has always drawn (left-aligned), so it is not measured here.
#[test]
fn labels_stay_on_the_page_and_apart() {
    for mode in ["Groups", "Carbons"] {
        for g in [ALKANE, OH, COOH, ENE, YNE] {
            for n in [1, 2, 3, 12] {
                for size in ["Large", "Medium"] {
                    let svg = def().svg(&with(&[s("mode", mode), s("group", g), i("carbons", n), s("label_size", size), b("numbers", true), s("caption", "Structure")]));
                    let (w, h) = crate::figures::svg_size(&svg);
                    let t = texts(&svg);
                    let bx: Vec<(f64, f64, f64, f64, String)> = t.iter().filter(|x| !x.3.starts_with("Molecular formula")).map(|(x, y, fs, s)| {
                        let len = s.replace('_', "").chars().count() as f64;
                        let hw = len * fs * 0.6 / 2.0;
                        (*x, *y - fs * 0.35, hw, fs * 0.42, s.clone())
                    }).collect();
                    for (x, y, hw, hh, s) in &bx {
                        assert!(x - hw >= 0.0 && x + hw <= w && y - hh >= 0.0 && y + hh <= h, "{mode} {g} {n} {size}: {s:?} leaves the page");
                    }
                    for a in 0..bx.len() {
                        for c in a + 1..bx.len() {
                            let (p, q) = (&bx[a], &bx[c]);
                            let over = (p.0 - q.0).abs() < p.2 + q.2 - 1.0 && (p.1 - q.1).abs() < p.3 + q.3 - 1.0;
                            assert!(!over, "{mode} {g} {n} {size}: {:?} overlaps {:?}", p.4, q.4);
                        }
                    }
                }
            }
        }
    }
}

/// The student copy keeps the drawing the teacher chose and drops only the formula key; the ends switch shows
/// only where it draws.
#[test]
fn student_copy_and_the_ends_switch() {
    for mode in ["Groups", "Carbons"] {
        let svg = def().svg(&with(&[s("mode", mode), b("show_answers", false)]));
        assert!(!svg.contains("Molecular formula") && !svg.contains("Mr ="), "{mode}");
        let svg = def().svg(&with(&[s("mode", mode)]));
        assert!(svg.contains("Molecular formula C\u{2084}H\u{2081}\u{2080}"), "{mode}: the key for butane");
    }
    assert!(crate::figures::param_relevant(FIG, "ends", &with(&[])));
    for mode in ["Groups", "Carbons", "Condensed", "Full"] {
        assert!(!crate::figures::param_relevant(FIG, "ends", &with(&[s("mode", mode)])), "{mode}");
    }
    assert!(!crate::figures::param_relevant(FIG, "ends", &with(&[s("skeleton", "Benzene ring")])));
    // every new mode changes the drawing from the opening state
    let open = def().svg(&with(&[]));
    for mode in ["Groups", "Carbons"] { assert_ne!(def().svg(&with(&[s("mode", mode)])), open, "{mode}"); }
}
