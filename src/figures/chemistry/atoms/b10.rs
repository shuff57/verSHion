//! Batch 10: the particle diagram's lattice rows, columns and spacing, and the Bohr model's protons
//! set by hand.
//!
//! The lattice. Rows and columns of 0 size the lattice from the particle counts, as before. A
//! teacher who sets them gets a full lattice of rows x columns places, and the particle counts give
//! the kinds' ratio (4 Cl⁻ and 4 Na⁺ is 1 : 1, so a 6 by 6 lattice holds 18 of each, in the same
//! alternating or spread pattern as before). With only one of them set, the other is the fewest
//! that holds every particle and keeps the ratio. A size that cannot keep the ratio (a 5 by 5
//! lattice at 1 : 1) is refused: the box is left empty and the panel says why and what would work,
//! rather than drawing a wrong composition. "Touching" packs the particles as before; "Spaced
//! apart" opens a gap of 0.4 of a particle between neighbours. Where the box is full every
//! particle shrinks alike, so the kinds keep their sizes against each other and the gap stays.
use super::*;

/// Most rows or columns a lattice has (the reference's 12 by 12).
pub const MAX_SIDE: usize = 12;
/// Pitch over particle size when spaced apart: a gap of 0.4 of a particle.
const SPACED: f64 = 1.4;
/// Smallest type drawn in a lattice particle (batch 7 lesson 3: never under 8 px).
const MIN_FONT: f64 = 8.0;

/// The lattice settings of one figure.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Lat { pub rows: usize, pub cols: usize, pub spaced: bool }

impl Lat {
    pub fn of(p: &Params) -> Lat {
        Lat {
            rows: p.i("lat_rows").clamp(0, MAX_SIDE as i64) as usize,
            cols: p.i("lat_cols").clamp(0, MAX_SIDE as i64) as usize,
            spaced: p.s("lat_spacing") == "Spaced apart",
        }
    }
    /// Whether the teacher set anything (otherwise the lattice draws exactly as before batch 10).
    pub fn custom(&self) -> bool { self.rows > 0 || self.cols > 0 || self.spaced }
}

/// What a lattice draws: its size and how many of each kind (in the order the kinds are given).
#[derive(Clone, PartialEq, Debug)]
pub struct Plan { pub rows: usize, pub cols: usize, pub counts: Vec<usize>, pub full: bool }

fn gcd(a: usize, b: usize) -> usize { if b == 0 { a } else { gcd(b, a % b) } }

/// Why a lattice size is refused.
#[derive(Clone, PartialEq, Debug)]
pub enum No {
    /// rows x columns places cannot hold the kinds in the counts' ratio, whose parts add to `unit`.
    Ratio { rows: usize, cols: usize, unit: usize },
    /// The other side would need more than 12.
    TooLong { set_rows: bool, side: usize, need: usize, unit: usize },
}

/// The lattice for kinds of `counts` (each above 0) under `lat`.
pub fn plan(counts: &[usize], lat: &Lat) -> Result<Plan, No> {
    let n: usize = counts.iter().sum();
    let (r, c) = (lat.rows, lat.cols);
    if r == 0 && c == 0 {
        let cols = ((n as f64).sqrt().ceil() as usize).max(1);
        return Ok(Plan { rows: n.div_ceil(cols).max(1), cols, counts: counts.to_vec(), full: false });
    }
    let g = counts.iter().copied().filter(|&k| k > 0).fold(0, gcd).max(1);
    let unit = (n / g).max(1);
    let fill = |rows: usize, cols: usize| -> Plan {
        let m = rows * cols / unit;
        Plan { rows, cols, counts: counts.iter().map(|&k| k / g * m).collect(), full: true }
    };
    if r > 0 && c > 0 {
        return if (r * c) % unit == 0 { Ok(fill(r, c)) } else { Err(No::Ratio { rows: r, cols: c, unit }) };
    }
    let side = r.max(c);
    // the fewest of the other that holds every particle and keeps the ratio
    let need = (1..).find(|&o| side * o >= n && (side * o) % unit == 0).unwrap_or(usize::MAX);
    if need > MAX_SIDE {
        return Err(No::TooLong { set_rows: r > 0, side, need, unit });
    }
    Ok(if r > 0 { fill(r, need) } else { fill(need, c) })
}

/// The kinds of one box as drawn: runs of equal particles, each with the slot (0..4) it came from.
fn runs_of(p: &Params, after: bool) -> Vec<(usize, Species, usize)> {
    let mut runs: Vec<(usize, Species, usize)> = Vec::new();
    for (i, k) in KINDS.iter().enumerate() {
        let n = p.i(&format!("{k}{}_count", if after { "2" } else { "" })).clamp(0, 24) as usize;
        if n == 0 { continue; }
        let sp = species(p, k);
        match runs.last_mut() { Some(r) if r.1 == sp => r.2 += n, _ => runs.push((i, sp, n)) }
    }
    runs
}

/// Whether the lattice controls change the drawing: a lattice with particles in a box drawn.
pub fn lattice_relevant(p: &Params) -> bool {
    p.s("layout") == "Lattice" && p.s("key_show") != "Key only"
        && (!runs_of(p, false).is_empty() || (p.s("boxes") == "Before and after" && !runs_of(p, true).is_empty()))
}

fn kind_name(p: &Params, slot: usize) -> String {
    let nm = p.s(&format!("{}_name", KINDS[slot])).trim().to_string();
    if nm.is_empty() { format!("{} {}", kind_noun(p, slot), slot + 1) } else { nm }
}

fn ratio(runs: &[(usize, Species, usize)]) -> String { runs.iter().map(|r| r.2.to_string()).collect::<Vec<_>>().join(" : ") }

/// The panel note under the lattice's columns: what a set size draws, or why it is refused.
pub fn lattice_note(p: &Params) -> Option<(String, bool)> {
    let lat = Lat::of(p);
    if !lattice_relevant(p) || (lat.rows == 0 && lat.cols == 0) { return None; }
    let two = p.s("boxes") == "Before and after";
    let mut parts = Vec::new();
    let mut refused = false;
    for after in if two { vec![false, true] } else { vec![false] } {
        let runs = runs_of(p, after);
        if runs.is_empty() { continue; }
        let lead = if two { format!("{}: ", if after { p.s("lab_after") } else { p.s("lab_before") }) } else { String::new() };
        let counts: Vec<usize> = runs.iter().map(|r| r.2).collect();
        match plan(&counts, &lat) {
            Ok(pl) => {
                let each: Vec<String> = runs.iter().zip(&pl.counts).map(|(r, k)| format!("{k} {}", kind_name(p, r.0))).collect();
                let kept = if runs.len() > 1 { format!(", in the counts\u{2019} {} ratio", ratio(&runs)) } else { String::new() };
                parts.push(format!("{lead}{} rows by {} columns: {}{kept}.", pl.rows, pl.cols, list(&each)));
            }
            Err(no) => {
                refused = true;
                parts.push(format!("{lead}{}", refusal(&no, &runs)));
            }
        }
    }
    if parts.is_empty() { None } else { Some((parts.join(" "), refused)) }
}

/// Counts as near the given ones' ratio as whole particles allow that a lattice of `places` holds:
/// adding up to the places themselves where every count stays within 1 to 24, else to the largest
/// whole share of them that does.
pub fn fit_counts(counts: &[usize], places: usize) -> Option<Vec<usize>> {
    let n: usize = counts.iter().sum();
    if n == 0 || counts.is_empty() { return None; }
    let share = |m: usize| -> Option<Vec<usize>> {
        let exact: Vec<f64> = counts.iter().map(|&c| c as f64 * m as f64 / n as f64).collect();
        let mut v: Vec<usize> = exact.iter().map(|x| x.floor() as usize).collect();
        let mut left = m - v.iter().sum::<usize>();
        let mut by: Vec<usize> = (0..v.len()).collect();
        by.sort_by(|&a, &b| (exact[b] - exact[b].floor()).total_cmp(&(exact[a] - exact[a].floor())).then(a.cmp(&b)));
        for &k in by.iter().cycle() { if left == 0 { break; } v[k] += 1; left -= 1; }
        while let Some(z) = v.iter().position(|&x| x == 0) {
            let big = (0..v.len()).max_by_key(|&k| (v[k], std::cmp::Reverse(k)))?;
            if v[big] <= 1 { return None; }
            v[big] -= 1;
            v[z] += 1;
        }
        (v.iter().all(|&x| (1..=24).contains(&x)) && v != counts).then_some(v)
    };
    (1..=places).rev().filter(|m| places % m == 0 && *m >= counts.len()).find_map(share)
}

fn list(v: &[String]) -> String {
    match v.len() {
        0 => String::new(),
        1 => v[0].clone(),
        _ => format!("{} and {}", v[..v.len() - 1].join(", "), v[v.len() - 1]),
    }
}

fn refusal(no: &No, runs: &[(usize, Species, usize)]) -> String {
    match no {
        No::Ratio { rows, cols, unit } => {
            let counts: Vec<usize> = runs.iter().map(|r| r.2).collect();
            let or = match fit_counts(&counts, rows * cols) {
                Some(f) => format!(", or set the counts to {}", list(&f.iter().map(|k| k.to_string()).collect::<Vec<_>>())),
                None => String::new(),
            };
            format!(
                "Nothing is drawn: {rows} by {cols} is {} places, which can\u{2019}t hold the kinds in the counts\u{2019} {} ratio. Pick rows and columns that multiply to a multiple of {unit}{or}.",
                rows * cols, ratio(runs)
            )
        }
        No::TooLong { set_rows, side, need, unit } => {
            let (have, other) = if *set_rows { ("rows", "columns") } else { ("columns", "rows") };
            if *need == usize::MAX {
                format!("Nothing is drawn: {side} {have} can\u{2019}t hold the kinds in the counts\u{2019} {} ratio with 12 {other} or fewer. Set both, multiplying to a multiple of {unit}.", ratio(runs))
            } else {
                format!("Nothing is drawn: {side} {have} would need {need} {other} to hold every particle in the counts\u{2019} ratio, and a lattice has 12 at most. Set more {have}.")
            }
        }
    }
}

fn atom8(d: &mut Doc, x: f64, y: f64, r: f64, fill: &str, ch: &str) {
    d.circle(x, y, r, fill, INK, 1.5);
    if ch != "None" && !ch.is_empty() {
        let col = if matches!(fill, "#161616" | "#5c5c5c") { "#ffffff" } else { INK };
        let fs = if ch.chars().count() > 1 { r * 0.95 } else { r * 1.3 };
        tx(d, x, y + fs.max(MIN_FONT) * 0.36, fs.max(MIN_FONT), "middle", col, true, ch);
    }
}

fn draw_mol8(d: &mut Doc, sp: &Species, x: f64, y: f64) {
    let m = mol_centre(sp);
    for (ox, oy) in mol_offsets(sp) { atom8(d, x + ox - m.0, y + oy - m.1, sp.ro, sp.fillo, &sp.cho); }
    atom8(d, x - m.0, y - m.1, sp.r, sp.fill, &sp.ch);
}

/// Whether a box's particles are of different sizes.
pub fn mixed_sizes(items: &[Species]) -> bool {
    let b: Vec<f64> = items.iter().map(bound).collect();
    b.iter().any(|&x| (x - b[0]).abs() > 1e-9)
}

/// The old figures batch 10 redraws: a lattice with a box of particles of different sizes (the
/// lattice before batch 10 shrank each into its own square, the larger more, and put the first kind
/// on the larger checkerboard class whatever its count, so neighbours never touched and could overlap).
#[cfg(test)]
pub(crate) fn corrected(p: &Params) -> bool {
    let build = |after: bool| -> Vec<Species> {
        let mut v = Vec::new();
        for k in KINDS {
            let n = p.i(&format!("{k}{}_count", if after { "2" } else { "" })).clamp(0, 24);
            for _ in 0..n { v.push(species(p, k)); }
        }
        v
    };
    p.s("layout") == "Lattice" && p.s("key_show") != "Key only"
        && (mixed_sizes(&build(false)) || (p.s("boxes") == "Before and after" && mixed_sizes(&build(true))))
}

/// The in-box solids batch 10 redraws (round 1 of the check, so that no particle overlaps another
/// or leaves its box): rows that rose past the top of the box, now shrunk alike to fit; a small atom
/// of an interstitial alloy put in a dip a shorter top row sits over, now left out of that dip.
#[cfg(test)]
pub(crate) fn in_box_corrected(p: &Params) -> bool {
    let build = |after: bool| -> Vec<Species> {
        let mut v = Vec::new();
        for k in KINDS {
            let n = p.i(&format!("{k}{}_count", if after { "2" } else { "" })).clamp(0, 24);
            for _ in 0..n { v.push(species(p, k)); }
        }
        v
    };
    p.s("layout") == "In a box" && p.s("key_show") != "Key only"
        && ((p.s("state") == "Solid" && solid_redrawn(&build(false), p.b("gaps")))
            || (p.s("boxes") == "Before and after" && p.s("state2") == "Solid" && solid_redrawn(&build(true), p.b("gaps"))))
}

/// One drawn lattice particle: centre, the particle as scaled, and its kind (index into the runs).
pub struct Placed { pub x: f64, pub y: f64, pub sp: Species, pub kind: usize }

/// Which kind sits in each of the first `n` cells of a `rows` x `cols` lattice (read row by row),
/// as (row, column, kind) in the order they are drawn.
///
/// Two kinds whose counts are the two checkerboard classes alternate, the larger count on the
/// larger class (equal counts: the first kind on the class of the top left cell, as before batch
/// 10). Otherwise the most numerous kind fills the lattice and the others are spread through it:
/// each next one on the free cell farthest from those already placed, measured with the lattice
/// wrapped round (a torus) so the edges draw no more and no fewer than their share, and only on one
/// checkerboard class while it has room, so no two of them are side by side along a row or column.
pub fn arrange(counts: &[usize], rows: usize, cols: usize, n: usize) -> Vec<(usize, usize, usize)> {
    let cells: Vec<(usize, usize)> = (0..rows).flat_map(|r| (0..cols).map(move |c| (r, c))).take(n).collect();
    if counts.len() <= 1 { return cells.into_iter().map(|(r, c)| (r, c, 0)).collect(); }
    let even = cells.iter().filter(|q| (q.0 + q.1) % 2 == 0).count();
    let odd = cells.len() - even;
    if counts.len() == 2 && counts[0] + counts[1] == cells.len() && (counts[0] == even || counts[0] == odd) {
        let first_even = counts[0] == even;
        let mut v: Vec<(usize, usize, usize)> = cells.iter().map(|&(r, c)| {
            let e = (r + c) % 2 == 0;
            (r, c, if e == first_even { 0 } else { 1 })
        }).collect();
        v.sort_by_key(|&(r, c, _)| ((r + c) % 2, r, c));
        return v;
    }
    let major = (0..counts.len()).rev().max_by_key(|&k| counts[k]).unwrap_or(0);
    let mut kind: Vec<Option<usize>> = vec![None; cells.len()];
    let mut placed: Vec<usize> = Vec::new();
    let mut order: Vec<usize> = (0..counts.len()).filter(|&k| k != major).collect();
    order.sort_by_key(|&k| (counts[k], k));
    let minority: usize = order.iter().map(|&k| counts[k]).sum();
    // the class the minority sit on: the larger one (the top left cell's on a tie)
    let class = if odd > even { 1 } else { 0 };
    let class_room = if class == 1 { odd } else { even };
    let td = |a: usize, b: usize| -> i64 {
        let (p, q) = (cells[a], cells[b]);
        let dr = p.0.abs_diff(q.0).min(rows - p.0.abs_diff(q.0)) as i64;
        let dc = p.1.abs_diff(q.1).min(cols - p.1.abs_diff(q.1)) as i64;
        dr * dr + dc * dc
    };
    // for each cell: its nearest placed one (squared, wrapped) and how many placed lie at that distance
    let mut near: Vec<(i64, usize)> = vec![(i64::MAX, 0); cells.len()];
    for k in order {
        for _ in 0..counts[k] {
            let on_class = placed.len() < class_room.min(minority);
            let free = (0..cells.len()).filter(|&i| kind[i].is_none() && (!on_class || (cells[i].0 + cells[i].1) % 2 == class));
            let pick = if placed.is_empty() {
                // the first one a third of the way in; on a torus where it starts does not matter
                let (tr, tc) = ((rows as f64 - 1.0) / 3.0, (cols as f64 - 1.0) / 3.0);
                free.min_by(|&a, &b| {
                    let da = (cells[a].0 as f64 - tr).powi(2) + (cells[a].1 as f64 - tc).powi(2);
                    let db = (cells[b].0 as f64 - tr).powi(2) + (cells[b].1 as f64 - tc).powi(2);
                    da.total_cmp(&db).then(a.cmp(&b))
                })
            } else {
                // farthest from the placed ones; then the fewest placed at that distance; then reading order
                free.max_by_key(|&i| (near[i].0, std::cmp::Reverse(near[i].1), std::cmp::Reverse(i)))
            };
            let Some(i) = pick else { break };
            kind[i] = Some(k);
            placed.push(i);
            for (j, nj) in near.iter_mut().enumerate() {
                let dd = td(i, j);
                if dd < nj.0 { *nj = (dd, 1); } else if dd == nj.0 { nj.1 += 1; }
            }
        }
    }
    cells.iter().zip(kind).map(|(&(r, c), k)| (r, c, k.unwrap_or(major))).collect()
}

/// The pitch that makes the largest pair of neighbours that occurs touch: side by side the two
/// sizes together, corner to corner the two over root 2. Also gives that pair's two sizes together.
pub fn pitch_for(cells: &[(usize, usize, usize)], sizes: &[f64]) -> (f64, f64) {
    let mut at = std::collections::HashMap::new();
    for &(r, c, k) in cells { at.insert((r as i64, c as i64), sizes[k]); }
    let (mut req, mut sum): (f64, f64) = (0.0, 0.0);
    let mut keys: Vec<(i64, i64)> = at.keys().copied().collect();
    keys.sort();
    for (r, c) in keys {
        let a = at[&(r, c)];
        for (dr, dc, f) in [(0, 1, 1.0), (1, 0, 1.0), (1, 1, std::f64::consts::SQRT_2), (1, -1, std::f64::consts::SQRT_2)] {
            if let Some(&b) = at.get(&(r + dr, c + dc)) {
                if (a + b) / f > req { req = (a + b) / f; sum = a + b; }
            }
        }
    }
    if req == 0.0 { let m = sizes.iter().cloned().fold(0.0, f64::max); (2.0 * m, 2.0 * m) } else { (req, sum) }
}

/// The lattice of `items` (the box's particles, kinds in runs) in the box at (bx, by), or None
/// when its size is refused.
pub fn layout(bx: f64, by: f64, bs: f64, items: &[Species], lat: &Lat) -> Option<Vec<Placed>> {
    let mut runs: Vec<(usize, usize)> = Vec::new(); // (first item, count)
    for (i, it) in items.iter().enumerate() {
        match runs.last_mut() { Some(r) if items[r.0] == *it => r.1 += 1, _ => runs.push((i, 1)) }
    }
    let counts: Vec<usize> = runs.iter().map(|r| r.1).collect();
    let pl = plan(&counts, lat).ok()?;
    let (rows, cols) = (pl.rows, pl.cols);
    let n: usize = pl.counts.iter().sum();
    let m = 8.0;
    let sizes: Vec<f64> = runs.iter().map(|r| bound(&items[r.0])).collect();
    let maxb = sizes.iter().cloned().fold(0.0, f64::max);
    let cells = arrange(&pl.counts, rows, cols, n);
    // touching: the largest neighbouring pair meets (a 2 % gap, as before batch 10, at most 0.95 px
    // between the two, whether side by side or corner to corner)
    let (req, sum) = pitch_for(&cells, &sizes);
    let touch = if 0.02 * sum <= 0.95 { req * 1.02 } else { req + 0.95 * req / sum };
    let natural = if lat.spaced { req * 1.02 * SPACED } else { touch };
    // the grid's extent is its pitches plus the largest particle at each end (a large ion between
    // small ones reaches past its own square), kept inside the box's margin
    let side = cols.max(rows) as f64;
    let step = natural.min((bs - 2.0 * m) / (side - 1.0 + 2.0 * maxb / natural));
    // a full box shrinks every particle alike, so the kinds keep their sizes against each other
    let scale = step / natural;
    let gx = bx + (bs - step * cols as f64) / 2.0 + step / 2.0;
    let gy = by + (bs - step * rows as f64) / 2.0 + step / 2.0;
    Some(cells.into_iter().map(|(r, c, kind)| {
        let mut s2 = items[runs[kind].0].clone();
        s2.r *= scale;
        s2.ro *= scale;
        Placed { x: gx + c as f64 * step, y: gy + r as f64 * step, sp: s2, kind }
    }).collect())
}

/// Draws the lattice of a box whose rows, columns or spacing the teacher set (nothing when refused).
pub fn lattice(d: &mut Doc, bx: f64, by: f64, bs: f64, items: &[Species], lat: &Lat) {
    for q in layout(bx, by, bs, items, lat).unwrap_or_default() { draw_mol8(d, &q.sp, q.x, q.y); }
}

// ---------------------------------------------------------------------------------------------
// Bohr model: protons by hand
// ---------------------------------------------------------------------------------------------

/// The protons drawn: the element's atomic number, or the teacher's own count (0 to 200).
pub fn protons(p: &Params) -> Option<usize> {
    let v = p.i("protons");
    (v >= 0).then(|| v.min(200) as usize)
}

/// Whether the proton count changes the drawing: a nucleus that shows protons, or a stated charge.
pub fn protons_relevant(p: &Params) -> bool {
    matches!(p.s("nucleus"), "Balls" | "Protons and neutrons") || p.b("caption") || p.b("brackets")
}

/// The panel note under Protons: what the teacher's count makes, beside the element picked.
pub fn protons_note(p: &Params) -> Option<String> {
    let pr = protons(p)?;
    let z = z_of(p.s("element"));
    let el = name(z).to_lowercase();
    let what = match pr {
        0 => "No element has 0 protons.".to_string(),
        x if x > 118 => format!("No known element has {x} protons; they run from 1 (hydrogen) to 118 (oganesson)."),
        x if x == z => format!("{x} protons, as {el} has."),
        x => format!("{x} protons make {} ({}), not {el}: the atom is drawn as set, for students to spot.", name(x).to_lowercase(), symbol(x)),
    };
    let g = bohr_geom(p);
    let e: u32 = g.shown.iter().sum();
    let stated = if p.b("caption") || p.b("brackets") {
        let c = g.charge;
        let ch = match c { 0 => "0".to_string(), c if c > 0 => format!("{c}+"), c => format!("{}\u{2212}", -c) };
        format!(" The charge shown is protons \u{2212} electrons: {pr} \u{2212} {e} = {ch}.")
    } else { String::new() };
    Some(format!("{what}{stated}"))
}
