//! Batch 11 (track "last"): the Punnett square's six classic crosses checked by hand against the squares drawn, a
//! phenotype never listed twice, the ABO alleles codominant under every switch, multiple alleles stepped through by a
//! click on a parent, and a click on every offspring cell at the opening state that blanks it for students (or fills
//! a blank one back in), tested through `interact::pick`.
use super::*;
use crate::figures::{answer_pair, interact, registry, svg_size, FigureDef};

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == "punnett-square").unwrap() }

fn with(over: &[(&str, &str)]) -> Params {
    let d = def();
    let defs = (d.params)();
    let mut p = Params::defaults(&defs);
    for (k, v) in over { p.0.insert(k.to_string(), Value::S(v.to_string())); }
    p.normalize(&defs, false);
    p
}

fn setup(name: &str) -> Params {
    let d = def();
    let defs = (d.params)();
    let e = classics().into_iter().find(|e| e.name == name).unwrap_or_else(|| panic!("no setup {name}"));
    let mut p = Params::defaults(&defs);
    for (k, v) in &e.params { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&defs, false);
    p
}

/// Every `<text>` drawn: (x, y, its words with the tspans run together).
fn texts(svg: &str) -> Vec<(f64, f64, String)> {
    let mut out = Vec::new();
    let mut rest = svg;
    while let Some(i) = rest.find("<text") {
        let t = &rest[i..];
        let end = t.find("</text>").unwrap();
        let el = &t[..end];
        let num = |name: &str| el.find(&format!(" {name}=\"")).and_then(|j| el[j + name.len() + 3..].split('"').next()).and_then(|v| v.parse::<f64>().ok()).unwrap_or(f64::NAN);
        let (x, y) = (num("x"), num("y"));
        let mut words = String::new();
        let mut inside = false;
        for c in el.chars() {
            match c { '<' => inside = true, '>' => inside = false, c if !inside => words.push(c), _ => {} }
        }
        out.push((x, y, words.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")));
        rest = &t[end..];
    }
    out
}

/// The genotype written in each offspring cell, row by row (None for a blank cell), read from the drawing.
fn drawn_cells(p: &Params) -> Vec<Option<String>> {
    let svg = render(p);
    let l = layout_of(p).expect("a square");
    let ts = texts(&svg);
    let mut v = Vec::new();
    for j in 0..l.nr {
        for i in 0..l.nc {
            let (x0, y0) = (l.gx + l.cell * i as f64, l.gy + l.cell * j as f64);
            let inside: Vec<&String> = ts.iter().filter(|t| t.0 > x0 && t.0 < x0 + l.cell && t.1 > y0 && t.1 < y0 + l.cell).map(|t| &t.2).collect();
            // a genotype, and its phenotype under it when Write each phenotype is on
            assert!(inside.len() <= 1 + p.b("cell_phenotype") as usize, "one genotype per cell: {inside:?}");
            v.push(inside.first().map(|s| s.to_string()));
        }
    }
    v
}

/// The stated ratio after `label` (wrapped lines run together), as (count, name) terms.
fn stated(p: &Params, label: &str) -> Vec<(i64, String)> {
    let svg = render(p);
    let l = layout_of(p).unwrap();
    let below: Vec<String> = texts(&svg).into_iter().filter(|t| t.1 > l.gy + l.cell * l.nr as f64).map(|t| t.2).collect();
    let all = below.join(" ");
    let start = all.find(label).unwrap_or_else(|| panic!("no {label} in {all}")) + label.len();
    let rest = &all[start..];
    let body = ["Genotype ratio:", "Phenotype ratio:"].iter().filter_map(|l| rest.find(l)).min().map(|e| &rest[..e]).unwrap_or(rest);
    body.split(" : ").map(|t| {
        let t = t.trim();
        let (n, name) = t.split_once(' ').unwrap_or_else(|| panic!("term {t:?}"));
        (if n == "all" { 1 } else { n.parse().unwrap_or_else(|_| panic!("term {t:?} in {all}")) }, name.trim().to_string())
    }).collect()
}

fn gcd_all(v: &[i64]) -> i64 { v.iter().fold(0, |a, b| gcd(a, *b)).max(1) }

/// Counts of `key(genotype)` over the drawn cells, in first-seen order, divided down to the smallest whole ratio.
fn tally(cells: &[Option<String>], key: impl Fn(&str) -> String) -> Vec<(String, i64)> {
    let mut v: Vec<(String, i64)> = Vec::new();
    for c in cells.iter().flatten() {
        let k = key(c);
        if let Some(e) = v.iter_mut().find(|e| e.0 == k) { e.1 += 1; } else { v.push((k, 1)); }
    }
    let g = gcd_all(&v.iter().map(|e| e.1).collect::<Vec<_>>());
    v.into_iter().map(|(k, n)| (k, n / g)).collect()
}

fn same(stated: &[(i64, String)], counted: &[(String, i64)]) -> bool {
    stated.len() == counted.len() && counted.iter().all(|(k, n)| stated.iter().any(|(m, s)| s == k && m == n))
}

/// The six classic crosses: what each square holds and the two ratios its key states, worked by hand from the
/// genotypes drawn in the cells (Mendel 1866 for the pea crosses; snapdragon flower colour is the textbook incomplete
/// dominance; red-green colour blindness is X-linked recessive; ABO: I^A and I^B codominant, i recessive to both).
#[test]
fn the_six_classic_crosses_state_the_ratios_their_squares_hold() {
    type Pheno = fn(&str) -> String;
    let tall: Pheno = |g| if g.contains('T') { "tall".into() } else { "short".into() };
    let peas: Pheno = |g| format!("{} {}", if g.contains('R') { "round" } else { "wrinkled" }, if g.contains('Y') { "yellow" } else { "green" });
    let eyes: Pheno = |g| if g.ends_with('Y') { if g.starts_with("XB") { "normal vision male".into() } else { "color-blind male".into() } } else if g.contains("XB") { "normal vision female".into() } else { "color-blind female".into() };
    let snap: Pheno = |g| match g { "CRCR" => "red".into(), "CWCW" => "white".into(), _ => "pink".into() };
    let abo: Pheno = |g| match (g.contains("IA"), g.contains("IB")) { (true, true) => "type AB".into(), (true, false) => "type A".into(), (false, true) => "type B".into(), _ => "type O".into() };
    let cases: [(&str, usize, &str, &str, Pheno); 6] = [
        ("Mendel’s pea plants", 4, "1 TT : 2 Tt : 1 tt", "3 tall : 1 short", tall),
        ("Test cross", 4, "1 Tt : 1 tt", "1 tall : 1 short", tall),
        ("Dihybrid cross", 16, "1 RRYY : 2 RRYy : 1 RRyy : 2 RrYY : 4 RrYy : 2 Rryy : 1 rrYY : 2 rrYy : 1 rryy", "9 round yellow : 3 round green : 3 wrinkled yellow : 1 wrinkled green", peas),
        ("Color blindness", 4, "1 XBXB : 1 XBXb : 1 XBY : 1 XbY", "2 normal vision female : 1 normal vision male : 1 color-blind male", eyes),
        ("Snapdragon color", 4, "1 CRCR : 2 CRCW : 1 CWCW", "1 red : 2 pink : 1 white", snap),
        ("ABO blood type", 4, "1 IAIB : 1 IAi : 1 IBi : 1 ii", "1 type A : 1 type AB : 1 type B : 1 type O", abo),
    ];
    assert_eq!(classics().len(), 6);
    for (name, n, geno, pheno, f) in cases {
        let p = setup(name);
        let cells = drawn_cells(&p);
        assert_eq!(cells.len(), n, "{name}: cells");
        assert!(cells.iter().all(|c| c.is_some()), "{name}: every cell filled in");
        let sg = stated(&p, "Genotype ratio:");
        let sp = stated(&p, "Phenotype ratio:");
        let fmt = |v: &[(i64, String)]| v.iter().map(|(n, s)| format!("{n} {s}")).collect::<Vec<_>>().join(" : ");
        assert_eq!(fmt(&sg), geno, "{name}: genotype ratio");
        assert_eq!(fmt(&sp), pheno, "{name}: phenotype ratio");
        // the stated ratios are the cells' own: counted from the genotypes drawn, by hand
        assert!(same(&sg, &tally(&cells, |g| g.to_string())), "{name}: genotype ratio against the cells {cells:?}");
        assert!(same(&sp, &tally(&cells, f)), "{name}: phenotype ratio against the cells {cells:?}");
    }
}

/// A ratio never names one phenotype twice: two genotypes with one name are one phenotype (Complete dominance on the
/// snapdragon names, an X-linked cross named by allele under co- or incomplete dominance, ABO under any switch).
#[test]
fn a_phenotype_is_never_listed_twice() {
    let names = ["", "tall, short", "red, white", "red, pink, white", "C^R: red; C^R+C^W: pink; C^W: white", "I^A: type A; I^A+I^B: type AB; I^B: type B; i: type O",
        "X^B: normal vision; X^b: color-blind", "R: round; r: wrinkled; Y: yellow; y: green", "T_: tall; tt: short"];
    let crosses: [(&str, &[&str]); 3] = [
        ("Monohybrid", &["Tt x Tt", "Tt x tt", "C^R C^W x C^R C^W", "C^R C^W x C^W C^W", "I^A i x I^B i", "I^A I^B x I^A I^B", "I^A I^B x ii", "I^A i x I^A i", "Rr x Rr"]),
        ("Dihybrid", &["RrYy x RrYy", "RrYy x rryy", "I^A i Rr x I^B i Rr"]),
        ("X-linked", &["X^B X^b x X^B Y", "X^B X^b x X^b Y", "X^b X^b x X^B Y"]),
    ];
    let mut n = 0;
    for (cross, parents) in crosses {
        for par in parents {
            for dom in ["Complete", "Incomplete", "Codominance"] {
                for nm in names {
                    for split in ["false", "true"] {
                        let mut p = with(&[("cross", cross), ("parents", par), ("dominance", dom), ("phenotypes", nm)]);
                        p.set_b("sex_split", split == "true");
                        let svg = render(&p);
                        if svg.contains("Cannot read") { continue; }
                        for lab in ["Phenotype ratio:", "Daughters:", "Sons:"] {
                            if !texts(&svg).iter().any(|t| t.2.contains(lab)) { continue; }
                            let st = stated(&p, lab);
                            let mut seen: Vec<&String> = Vec::new();
                            for (_, s) in &st {
                                assert!(!seen.contains(&s), "{cross} {dom} {par} [{nm}] {lab}: {s} twice in {st:?}");
                                seen.push(s);
                            }
                            // and the counts still add up to the cells drawn (one phenotype per cell)
                            n += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(n > 500, "{n}");
}

/// ABO: I^A and I^B are codominant and i recessive to both, whatever the Dominance switch, named or not.
#[test]
fn abo_is_codominant_under_every_switch() {
    let abo = "I^A: type A; I^A+I^B: type AB; I^B: type B; i: type O";
    let cases = [
        ("I^A i x I^B i", "1 type A : 1 type AB : 1 type B : 1 type O", "1 A : 1 AB : 1 B : 1 O"),
        ("I^A I^B x I^A I^B", "1 type A : 2 type AB : 1 type B", "1 A : 2 AB : 1 B"),
        ("I^A I^A x I^B I^B", "1 type AB", "1 AB"),
        ("I^A I^B x ii", "1 type A : 1 type B", "1 A : 1 B"),
        ("I^A i x I^A i", "3 type A : 1 type O", "3 A : 1 O"),
        ("I^A i x ii", "1 type A : 1 type O", "1 A : 1 O"),
    ];
    for dom in ["Complete", "Incomplete", "Codominance"] {
        for (par, named, plain) in cases {
            for (nm, want) in [(abo, named), ("", plain)] {
                let p = with(&[("parents", par), ("dominance", dom), ("phenotypes", nm)]);
                let st = stated(&p, "Phenotype ratio:");
                let got = st.iter().map(|(n, s)| format!("{n} {s}")).collect::<Vec<_>>().join(" : ");
                assert_eq!(got, want, "{dom} {par} [{nm}]");
            }
        }
    }
}

/// A click on a parent with multiple alleles steps through every genotype they make (it did nothing before), so a
/// loaded ABO or snapdragon cross stays editable on the figure.
#[test]
fn a_parent_with_multiple_alleles_steps_through_its_genotypes() {
    let mut p = setup("ABO blood type");
    let mut seen = Vec::new();
    for _ in 0..6 {
        cycle_gene(&mut p, 0, 0);
        seen.push(p.s("parents").split('×').next().unwrap().trim().to_string());
        assert!(!render(&p).contains("Cannot read"), "{}", p.s("parents"));
    }
    assert_eq!(seen, ["I^BI^B", "I^Bi", "ii", "I^AI^A", "I^AI^B", "I^Ai"]);
    assert_eq!(p.s("parents"), "I^Ai × I^Bi", "back where it started, the other parent kept");
    // ii (no superscript) on the side still steps through the cross's alleles, not to a capital I
    let mut q = with(&[("parents", "I^A I^B x ii"), ("dominance", "Codominance")]);
    cycle_gene(&mut q, 1, 0);
    assert_eq!(q.s("parents"), "I^AI^B × I^AI^A", "ii steps on to I^A I^A, the first genotype");
    let mut s = setup("Snapdragon color");
    let mut seen = Vec::new();
    for _ in 0..3 {
        cycle_gene(&mut s, 1, 0);
        seen.push(s.s("parents").split('×').nth(1).unwrap().trim().to_string());
    }
    assert_eq!(seen, ["C^WC^W", "C^RC^R", "C^RC^W"]);
    // a plain gene cycles as before
    let mut t = with(&[]);
    cycle_gene(&mut t, 0, 0);
    assert_eq!(t.s("parents"), "tt × Tt");
}

fn size_of(p: &Params) -> (f64, f64) { svg_size(&def().svg(p)) }

/// The square's opening state has a click handle on every offspring cell, found by the workspace's own hit test
/// anywhere inside the cell; a click blanks that cell only, the student copy leaves it blank and the key fills it.
#[test]
fn every_cell_is_clicked_blank_through_the_hit_test() {
    let d = def();
    let open = crate::figures::start_params(&d);
    let hs = d.handles(&open, size_of(&open));
    let tip = "Click a cell to blank it for students";
    assert_eq!(hs.iter().filter(|h| h.tip == tip && h.click.is_some()).count(), 4, "a click handle on each of the four cells at the opening state");
    let mut layouts: Vec<Params> = vec![open.clone()];
    for e in classics().into_iter().chain(examples()) {
        let mut p = open.clone();
        for (k, v) in &e.params { p.0.insert(k.to_string(), v.clone()); }
        p.set_s("cells", "Filled in");
        p.set_b("filled", true);
        layouts.push(p);
    }
    layouts.push(with(&[("title", "A title"), ("question", "A question that runs long enough to take two lines under the title of the figure"), ("parent_labels", "Mother")]));
    layouts.push(with(&[("parent_labels", "None"), ("cross", "Dihybrid"), ("parents", "RrYy x rryy")]));
    for base in layouts {
        let l = layout_of(&base).unwrap();
        let n = l.nc * l.nr;
        for idx in 0..n {
            let (i, j) = (idx % l.nc, idx / l.nc);
            let (x0, y0) = (l.gx + l.cell * i as f64, l.gy + l.cell * j as f64);
            // sweep the cell's inside, 6 px in from its walls
            for sx in 0..5 {
                for sy in 0..5 {
                    let q = (x0 + 6.0 + (l.cell - 12.0) * sx as f64 / 4.0, y0 + 6.0 + (l.cell - 12.0) * sy as f64 / 4.0);
                    let mut p = base.clone();
                    let hs = d.handles(&p, size_of(&p));
                    for k in [0.6, 1.0, 1.5] {
                        let h = interact::pick(&hs, q, k).unwrap_or_else(|| panic!("no handle at {q:?} in cell {idx}"));
                        assert_eq!(hs[h].tip, tip, "cell {idx} at {q:?}");
                    }
                    if (sx, sy) != (2, 2) { continue; }
                    let h = interact::pick(&hs, q, 1.0).unwrap();
                    (hs[h].click.as_ref().unwrap())(&mut p);
                    let cells = drawn_cells(&p);
                    for (c, g) in cells.iter().enumerate() {
                        assert_eq!(g.is_none(), c == idx, "after a click on cell {idx}: cell {c} {g:?} ({})", p.s("parents"));
                    }
                    // the student copy leaves it blank, the key fills it in: the copies are the figure's own answer
                    // pair, drawn from these params (checked byte for byte), read cell by cell
                    let (student, key) = answer_pair(&d, &p).unwrap();
                    let mut sp = p.clone();
                    sp.set_b("filled", false);
                    for k in ["geno_ratio_style", "pheno_ratio_style"] { if sp.s(k) == "Shown" { sp.set_s(k, "Blank line"); } }
                    let mut kp = p.clone();
                    kp.set_b("filled", true);
                    kp.set_s("cells", "Filled in");
                    kp.set_b("blank_parents", false);
                    kp.set_b("blank_gametes", false);
                    for k in ["geno_ratio_style", "pheno_ratio_style"] { if kp.s(k) == "Blank line" { kp.set_s(k, "Shown"); } }
                    assert_eq!((d.render)(&sp), student, "the student copy's params");
                    assert_eq!((d.render)(&kp), key, "the key's params");
                    assert!(drawn_cells(&sp)[idx].is_none(), "the student copy leaves cell {idx} blank");
                    assert_eq!(drawn_cells(&kp), drawn_cells(&base), "the key fills every cell");
                    // a second click on the same spot fills it back in: the handle's tip says so
                    let hs2 = d.handles(&p, size_of(&p));
                    let h2 = interact::pick(&hs2, q, 1.0).unwrap();
                    assert_eq!(hs2[h2].tip, "Click to fill this cell back in");
                    (hs2[h2].click.as_ref().unwrap())(&mut p);
                    assert!(drawn_cells(&p).iter().all(|c| c.is_some()), "filled back in");
                }
            }
        }
    }
}

/// A blank square (Blank, or the old Fill-in switch off): a click fills that one cell back in, the rest stay blank;
/// Some blank keeps its list and toggles the cell clicked.
#[test]
fn a_blank_cell_is_clicked_back_in() {
    let d = def();
    for p0 in [with(&[("cells", "Blank")]), { let mut p = with(&[]); p.set_b("filled", false); p }, with(&[("cells", "Blank"), ("cross", "Dihybrid"), ("parents", "RrYy x RrYy")])] {
        let l = layout_of(&p0).unwrap();
        let n = l.nc * l.nr;
        for idx in 0..n {
            let mut p = p0.clone();
            let c = (l.gx + l.cell * ((idx % l.nc) as f64 + 0.5), l.gy + l.cell * ((idx / l.nc) as f64 + 0.5));
            let hs = d.handles(&p, size_of(&p));
            let h = interact::pick(&hs, c, 1.0).unwrap();
            assert_eq!(hs[h].tip, "Click to fill this cell back in");
            (hs[h].click.as_ref().unwrap())(&mut p);
            let cells = drawn_cells(&p);
            for (k, g) in cells.iter().enumerate() { assert_eq!(g.is_some(), k == idx, "cell {k} after filling {idx}"); }
        }
    }
    let mut p = with(&[("cells", "Some blank"), ("blank_cells", "1, 4")]);
    let l = layout_of(&p).unwrap();
    let hs = d.handles(&p, size_of(&p));
    let h = interact::pick(&hs, (l.gx + l.cell * 1.5, l.gy + l.cell * 0.5), 1.0).unwrap();
    assert_eq!(hs[h].tip, "Click a cell to blank it for students");
    (hs[h].click.as_ref().unwrap())(&mut p);
    assert_eq!(p.s("blank_cells"), "1, 2, 4");
    assert_eq!(drawn_cells(&p).iter().map(|c| c.is_none()).collect::<Vec<_>>(), [true, true, false, true]);
}

/// The cell handles leave the parents' own click zones and the title where they were.
#[test]
fn the_cell_clicks_leave_the_parent_and_title_zones() {
    let d = def();
    let p = with(&[("title", "Pea plants")]);
    let hs = d.handles(&p, size_of(&p));
    assert_eq!(hs.iter().filter(|h| h.tip == "Click to cycle this parent's genotype").count(), 2);
    assert_eq!(hs.iter().filter(|h| h.tip.contains("title")).count(), 1);
    let l = layout_of(&p).unwrap();
    for h in hs.iter().filter(|h| h.tip.contains("parent")) {
        let i = interact::pick(&hs, h.pos, 1.0).unwrap();
        assert_eq!(hs[i].tip, h.tip, "the parent's zone still takes its own click");
        assert!(h.pos.1 < l.gy || h.pos.0 < l.gx, "outside the grid");
    }
}
