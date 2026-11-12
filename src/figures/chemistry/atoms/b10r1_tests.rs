//! Batch 10, round 1 of the check: lattices never overlap, touch where "Touching", keep a gap where
//! "Spaced apart", spread a minority kind evenly, and a refused size offers counts that fit. Every
//! test reads the drawn SVG.
use super::b10::{arrange, fit_counts, plan, Lat};
use super::b10_check::{boxes, faults, nearest_gaps};
use crate::figures::{registry, start_params, FigureDef, Params, Value};

const PD: &str = "particle-diagram";
const SIZES: [&str; 5] = ["XS", "S", "M", "L", "XL"];

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == PD).unwrap() }
fn with(d: &FigureDef, list: &[(&str, Value)]) -> Params {
    let mut p = start_params(d);
    for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&(d.params)(), false);
    p
}
fn s(k: &str, v: &str) -> (&'static str, Value) { (Box::leak(k.to_string().into_boxed_str()), Value::S(v.to_string())) }
fn i(k: &str, v: i64) -> (&'static str, Value) { (Box::leak(k.to_string().into_boxed_str()), Value::I(v)) }

/// A lattice of lone particles: kind sizes, counts, rows, columns and spacing.
fn lat(sizes: &[&str], counts: &[i64], r: i64, c: i64, spaced: bool) -> Vec<(&'static str, Value)> {
    let mut v = vec![s("layout", "Lattice"), s("state", "Solid"), i("lat_rows", r), i("lat_cols", c), s("lat_spacing", if spaced { "Spaced apart" } else { "Touching" })];
    let shades = ["Light gray", "White", "Dark gray", "Black"];
    for (k, slot) in ["a", "b", "c", "d"].iter().enumerate() {
        let n = counts.get(k).copied().unwrap_or(0);
        v.push(i(&format!("{slot}_count"), n));
        if n > 0 {
            v.push(s(&format!("{slot}_size"), sizes[k]));
            v.push(s(&format!("{slot}_shade"), shades[k]));
            v.push(s(&format!("{slot}_shape"), "Alone"));
            v.push(s(&format!("{slot}_charge"), "None"));
        }
    }
    v
}

fn gcd(a: usize, b: usize) -> usize { if b == 0 { a } else { gcd(b, a % b) } }

/// The checks on one drawn lattice. `old_path`: a same-size lattice of unset size and Touching,
/// which still draws as before batch 10 (pinned), with its 2 % gap (1.04 px at XL).
fn check(d: &FigureDef, list: &[(&'static str, Value)], same: bool, full: bool, spaced: bool, old_path: bool) -> Result<(), String> {
    let p = with(d, list);
    let svg = d.svg(&p);
    let bad = faults(&svg, &boxes(false));
    if !bad.is_empty() { return Err(bad[0].clone()); }
    let g = nearest_gaps(&svg, boxes(false)[0]);
    if g.len() < 2 { return Ok(()); }
    let min = g.iter().map(|x| x.0).fold(f64::MAX, f64::min);
    let rmax = g.iter().map(|x| x.1).fold(0.0, f64::max);
    if spaced {
        if min < 3.0 || min < 0.25 * rmax { return Err(format!("spaced apart by only {min:.2} (radius {rmax:.2})")); }
        if same && full && g.iter().any(|x| (x.0 - min).abs() > 0.1) { return Err("the gaps are not uniform".into()); }
    } else {
        let tol = if old_path { 1.05 } else { 1.0 };
        if min > tol { return Err(format!("the nearest pair is {min:.2} px apart, not touching")); }
        if same && full && g.iter().any(|x| x.0 > tol) { return Err("a particle touches no neighbour".into()); }
    }
    Ok(())
}

/// Every pair of sizes, counts 1 to 8 of each, unset and every accepted size from 2x2 to 12x12,
/// touching and spaced apart; then three and four kinds, seeded. No overlap, inside the box,
/// touching or a uniform gap as asked.
#[test]
fn lattice_sweep_never_overlaps() {
    let d = def();
    let mut fails: Vec<String> = Vec::new();
    let mut n = 0;
    for (ai, sa) in SIZES.iter().enumerate() {
        for sb in SIZES.iter().skip(ai) {
            let same = sa == sb;
            let mut ratios = std::collections::BTreeSet::new();
            for a in 1..=8i64 {
                for b in 1..=8i64 {
                    for spaced in [false, true] {
                        n += 1;
                        let tot = (a + b) as usize;
                        let cols = ((tot as f64).sqrt().ceil()) as usize;
                        let full = cols * tot.div_ceil(cols) == tot;
                        if let Err(e) = check(&d, &lat(&[sa, sb], &[a, b], 0, 0, spaced), same, full, spaced, same && !spaced) { fails.push(format!("{sa}/{sb} {a}:{b} unset {spaced}: {e}")); }
                    }
                    let g = gcd(a as usize, b as usize);
                    ratios.insert((a as usize / g, b as usize / g));
                }
            }
            for &(a, b) in &ratios {
                for r in 2..=12usize {
                    for c in 2..=12usize {
                        if plan(&[a, b], &Lat { rows: r, cols: c, spaced: false }).is_err() { continue; }
                        for spaced in [false, true] {
                            n += 1;
                            if let Err(e) = check(&d, &lat(&[sa, sb], &[a as i64, b as i64], r as i64, c as i64, spaced), same, true, spaced, false) { fails.push(format!("{sa}/{sb} {a}:{b} {r}x{c} {spaced}: {e}")); }
                        }
                    }
                }
            }
        }
    }
    let mut seed: u64 = 0xb10_0001;
    let mut next = |m: u64| { seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (seed >> 33) % m };
    for _ in 0..1500 {
        let k = 3 + next(2) as usize;
        let sizes: Vec<&str> = (0..k).map(|_| SIZES[next(5) as usize]).collect();
        let counts: Vec<i64> = (0..k).map(|_| 1 + next(8) as i64).collect();
        let (r, c) = if next(3) == 0 { (0, 0) } else { (2 + next(11) as i64, 2 + next(11) as i64) };
        let spaced = next(2) == 0;
        n += 1;
        let p = with(&d, &lat(&sizes, &counts, r, c, spaced));
        let bad = faults(&d.svg(&p), &boxes(false));
        if !bad.is_empty() { fails.push(format!("{sizes:?} {counts:?} {r}x{c} {spaced}: {}", bad[0])); }
    }
    assert!(n > 10_000, "{n} lattices");
    assert!(fails.is_empty(), "{} of {n} lattices fail, e.g.\n{}", fails.len(), fails.iter().take(8).cloned().collect::<Vec<_>>().join("\n"));
}

/// The checker's own cases: unequal two-kind lattices whose large ions overlapped.
#[test]
fn the_checkers_overlapping_lattices_no_longer_overlap() {
    let d = def();
    let cases: Vec<Vec<(&'static str, Value)>> = vec![
        vec![s("layout", "Lattice"), i("a_count", 2), s("a_size", "M"), s("a_charge", "2+"), i("b_count", 4), s("b_size", "L"), s("b_shade", "White"), s("b_charge", "−"), i("lat_rows", 3), i("lat_cols", 3)],
        vec![s("layout", "Lattice"), i("a_count", 12), s("a_size", "M"), s("a_charge", "+"), i("b_count", 13), s("b_size", "XL"), s("b_charge", "−"), i("lat_rows", 5), i("lat_cols", 5)],
        vec![s("layout", "Lattice"), i("a_count", 12), s("a_size", "M"), s("a_charge", "+"), i("b_count", 13), s("b_size", "XL"), s("b_charge", "−")],
        vec![s("layout", "Lattice"), i("a_count", 3), i("b_count", 1)],
        vec![s("layout", "Lattice"), i("a_count", 8), s("a_size", "XL"), i("b_count", 7), s("b_size", "M")],
        vec![s("layout", "Lattice"), i("a_count", 13), s("a_size", "XL"), i("b_count", 11), s("b_size", "M")],
        vec![s("layout", "Lattice"), i("a_count", 2), s("a_size", "XL"), i("b_count", 1), s("b_size", "M")],
    ];
    for c in cases {
        let p = with(&d, &c);
        let bad = faults(&d.svg(&p), &boxes(false));
        assert!(bad.is_empty(), "{c:?}: {}", bad.join("; "));
    }
    // 12 Na⁺ and 13 Cl⁻ alternate, the 13 on the larger class: each Na⁺ touches its Cl⁻ neighbours
    let cells = arrange(&[12, 13], 5, 5, 25);
    for &(r, c, k) in &cells { assert_eq!(k, if (r + c) % 2 == 0 { 1 } else { 0 }); }
}

/// A minority kind is spread evenly: none side by side along a row or column where its class has
/// room, and the border holds about its share of them.
#[test]
fn a_minority_is_spread_evenly() {
    let d = def();
    for (a, b, r, c) in [(2i64, 4i64, 6i64, 6i64), (1, 3, 8, 8), (1, 2, 6, 6), (1, 5, 12, 12), (2, 7, 9, 9), (3, 5, 8, 8), (1, 8, 12, 12)] {
        let p = with(&d, &lat(&["M", "L"], &[a, b], r, c, false));
        let svg = d.svg(&p);
        // the drawn minority: the light gray circles, read off the grid
        let cs: Vec<(f64, f64, bool)> = svg.match_indices("<circle ").map(|(k, _)| {
            let tag = &svg[k..k + svg[k..].find('>').unwrap()];
            let num = |n: &str| -> f64 { let x = tag.find(&format!(" {n}=\"")).unwrap() + n.len() + 3; tag[x..].split('"').next().unwrap().parse().unwrap() };
            (num("cx"), num("cy"), tag.contains(&format!("fill=\"{}\"", super::shade("Light gray"))))
        }).filter(|q| q.0 > 16.0 && q.0 < 316.0).collect();
        let xs: Vec<f64> = { let mut v: Vec<f64> = cs.iter().map(|q| q.0).collect(); v.sort_by(f64::total_cmp); v.dedup_by(|x, y| (*x - *y).abs() < 0.01); v };
        let ys: Vec<f64> = { let mut v: Vec<f64> = cs.iter().map(|q| q.1).collect(); v.sort_by(f64::total_cmp); v.dedup_by(|x, y| (*x - *y).abs() < 0.01); v };
        assert_eq!((ys.len() as i64, xs.len() as i64), (r, c));
        let at = |x: f64, v: &[f64]| v.iter().position(|&q| (q - x).abs() < 0.01).unwrap();
        let minority: Vec<(usize, usize)> = cs.iter().filter(|q| q.2).map(|q| (at(q.1, &ys), at(q.0, &xs))).collect();
        let total = (r * c) as usize;
        let want = total * a as usize / (a + b) as usize;
        assert_eq!(minority.len(), want, "{a}:{b} {r}x{c}");
        let class_room = total.div_ceil(2);
        let like = minority.iter().filter(|p| minority.iter().any(|q| p.0.abs_diff(q.0) + p.1.abs_diff(q.1) == 1)).count();
        if want <= class_room { assert_eq!(like, 0, "{a}:{b} {r}x{c}: minority side by side"); }
        let border = |p: &(usize, usize)| p.0 == 0 || p.1 == 0 || p.0 + 1 == r as usize || p.1 + 1 == c as usize;
        let area = (total - (r as usize - 2) * (c as usize - 2)) as f64 / total as f64;
        let share = minority.iter().filter(|p| border(p)).count() as f64 / minority.len() as f64;
        assert!((share - area).abs() <= 0.2, "{a}:{b} {r}x{c}: {share:.2} of the minority on the border against its {area:.2} of the cells");
        // and no quarter of the lattice holds more than twice its share
        for (qr, qc) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
            let inq = minority.iter().filter(|p| (p.0 * 2 >= r as usize) as usize == qr && (p.1 * 2 >= c as usize) as usize == qc).count() as f64;
            assert!(inq <= 0.25 * minority.len() as f64 * 2.0 + 1.0, "{a}:{b} {r}x{c}: a quarter holds {inq} of {}", minority.len());
        }
    }
}

/// A refused size's note offers counts that fit it, and those counts then draw.
#[test]
fn a_refused_size_offers_counts_that_fit() {
    let d = def();
    let note = |p: &Params| crate::figures::chemistry::notes::key_note(PD, "lat_cols", p).unwrap();
    let p = with(&d, &[s("layout", "Lattice"), i("lat_rows", 5), i("lat_cols", 5)]);
    let n = note(&p);
    assert!(n.warn && n.text.ends_with("or set the counts to 13 and 12."), "{}", n.text);
    for (counts, r, c) in [(vec![4i64, 4], 5i64, 5i64), (vec![2, 1], 4, 4), (vec![3, 3, 2], 5, 7), (vec![24, 24], 7, 7), (vec![5, 5, 5, 5], 3, 3), (vec![1, 1], 11, 11)] {
        let p = with(&d, &lat(&["L", "S", "M", "XS"], &counts, r, c, false));
        assert!(d.svg(&p).matches("<circle ").count() == 0, "{counts:?} {r}x{c} is refused");
        let n = note(&p);
        let Some(i0) = n.text.find("or set the counts to ") else { panic!("{counts:?} {r}x{c}: no counts offered: {}", n.text) };
        let offered: Vec<i64> = n.text[i0 + 21..].trim_end_matches('.').replace(" and ", ", ").split(", ").map(|t| t.parse().unwrap()).collect();
        assert_eq!(offered.len(), counts.len());
        let q = with(&d, &lat(&["L", "S", "M", "XS"], &offered, r, c, false));
        assert_eq!(d.svg(&q).matches("<circle ").count() as i64, r * c, "{counts:?} {r}x{c}: the offered {offered:?} draw the lattice");
        assert!(crate::figures::chemistry::notes::key_note(PD, "lat_cols", &q).is_some_and(|n| !n.warn));
    }
    assert_eq!(fit_counts(&[4, 4], 25), Some(vec![13, 12]));
}
