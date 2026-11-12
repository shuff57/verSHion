//! Reading a drawn particle diagram's boxes: which particle circles overlap, and which leave their
//! box. Used by the batch 10 tests over new lattices and over every frozen old figure.

/// (cx, cy, r) of every circle, in the order drawn.
pub fn circles(svg: &str) -> Vec<(f64, f64, f64)> {
    let attr = |tag: &str, name: &str| -> Option<f64> {
        let k = format!(" {name}=\"");
        let i = tag.find(&k)? + k.len();
        tag[i..].split('"').next()?.parse().ok()
    };
    svg.match_indices("<circle ").filter_map(|(i, _)| {
        let tag = &svg[i..i + svg[i..].find('>')?];
        Some((attr(tag, "cx")?, attr(tag, "cy")?, attr(tag, "r")?))
    }).collect()
}

/// The particle diagram's boxes as drawn: (left, top, side) of the one box or both.
pub fn boxes(two: bool) -> Vec<(f64, f64, f64)> {
    if two { vec![(16.0, 16.0, 300.0), (426.0, 20.0, 300.0)] } else { vec![(16.0, 16.0, 300.0)] }
}

/// The molecules of a box's circles in drawing order: each molecule's outer atoms come just before
/// its centre atom, all of one size, each at the bond length 0.9 (r + ro) from the centre.
pub fn molecules(cs: &[(f64, f64, f64)]) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    let mut i = cs.len();
    while i > 0 {
        let c = i - 1;
        let mut g = vec![c];
        let mut j = c;
        while j > 0 && g.len() < 5 {
            let o = cs[j - 1];
            let d = (o.0 - cs[c].0).hypot(o.1 - cs[c].1);
            let same = g.len() == 1 || (o.2 - cs[g[1]].2).abs() < 0.02;
            if same && (d - 0.9 * (o.2 + cs[c].2)).abs() < 0.05 { g.push(j - 1); j -= 1; } else { break; }
        }
        i = j;
        out.push(g);
    }
    out
}

/// What is wrong with the particles drawn in `boxes`: circles of two different particles that
/// overlap by more than 0.5 px, or a circle that reaches outside its box.
pub fn faults(svg: &str, boxes: &[(f64, f64, f64)]) -> Vec<String> {
    let all = circles(svg);
    let mut bad = Vec::new();
    for &(x0, y0, s) in boxes {
        let cs: Vec<(f64, f64, f64)> = all.iter().copied().filter(|c| c.0 > x0 && c.0 < x0 + s && c.1 > y0 && c.1 < y0 + s).collect();
        let mols = molecules(&cs);
        let mut owner = vec![0usize; cs.len()];
        for (m, g) in mols.iter().enumerate() { for &k in g { owner[k] = m; } }
        for (a, ca) in cs.iter().enumerate() {
            if ca.0 - ca.2 < x0 - 0.01 || ca.0 + ca.2 > x0 + s + 0.01 || ca.1 - ca.2 < y0 - 0.01 || ca.1 + ca.2 > y0 + s + 0.01 {
                bad.push(format!("circle {ca:?} leaves the box at {x0},{y0}"));
            }
            for (b, cb) in cs.iter().enumerate().skip(a + 1) {
                if owner[a] == owner[b] { continue; }
                let d = (ca.0 - cb.0).hypot(ca.1 - cb.1);
                if d < ca.2 + cb.2 - 0.5 { bad.push(format!("{ca:?} and {cb:?} overlap by {:.2}", ca.2 + cb.2 - d)); }
            }
        }
    }
    bad
}

/// Edge-to-edge gaps between the particles of the first box, each particle's smallest.
pub fn nearest_gaps(svg: &str, bx: (f64, f64, f64)) -> Vec<(f64, f64)> {
    let cs: Vec<(f64, f64, f64)> = circles(svg).into_iter().filter(|c| c.0 > bx.0 && c.0 < bx.0 + bx.2 && c.1 > bx.1 && c.1 < bx.1 + bx.2).collect();
    cs.iter().enumerate().map(|(a, ca)| {
        let g = cs.iter().enumerate().filter(|(b, _)| *b != a).map(|(_, cb)| (ca.0 - cb.0).hypot(ca.1 - cb.1) - ca.2 - cb.2).fold(f64::MAX, f64::min);
        (g, ca.2)
    }).collect()
}
