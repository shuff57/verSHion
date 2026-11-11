//! Batch 9: the ruler reading's Random button (the rest of the figure is in `math.rs`). It picks an
//! object length and where its left end sits, both at the precision the figure draws and keys them
//! (`ruler_values`): the nearest mark when "Read to" is the nearest mark, else one estimated digit
//! (hundredths of a centimetre on a millimetre ruler, tenths on a 5 mm or centimetre one,
//! hundredths of an inch), with the object wholly on the ruler.
use super::{ruler_read_dec, ruler_step, ruler_values};
use crate::figures::Params;

/// The grid a reading lands on: the smallest mark, or one estimated digit past it.
fn grain(p: &Params) -> f64 {
    let (step, metric) = ruler_step(p.s("marks"));
    if p.s("read_to") == "Nearest mark" { step } else if metric { 10f64.powi(-(ruler_read_dec(step, true) as i32)) } else { 0.01 }
}

/// Unit of the ruler: centimetres or inches, from its marks.
pub fn unit(p: &Params) -> &'static str { if ruler_step(p.s("marks")).1 { "cm" } else { "in" } }

/// `k` grains, cleaned of float noise (a grain is a power of ten or a power-of-two fraction of an inch).
fn on_grid(k: f64, g: f64) -> f64 { (k * g * 1e6).round() / 1e6 }

/// Sets a random length and left end the ruler can show; true when the figure is the ruler reading.
pub fn random(p: &mut Params, seed: u64) -> bool {
    let total = p.i("ruler").max(1) as f64;
    let g = grain(p);
    // from a twentieth of the ruler (at least half a unit, at least one grain) to nine tenths of it
    let lo = (total * 0.05).max(0.5).max(g);
    let (klo, khi) = ((lo / g).ceil(), ((total * 0.9) / g).floor().max((lo / g).ceil()));
    let u = crate::figures::unit_random(seed);
    let kl = (klo + (u * (khi - klo + 1.0)).floor()).min(khi);
    let len = on_grid(kl, g);
    // the left end anywhere that keeps the whole object on the ruler
    let room = ((total - len) / g + 1e-9).floor().max(0.0);
    let v = crate::figures::unit_random(seed ^ 0x5157_a2c3_9e11_77d1);
    let ks = (v * (room + 1.0)).floor().min(room);
    p.set_f("length", len);
    p.set_f("start", on_grid(ks, g));
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::{random_key, random_reading, registry, share, spec, start_params, FigureDef};

    const ID: &str = "ruler-reading";
    fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).unwrap() }

    /// Every random length and left end is on the grid the figure keys, inside the ruler, readable
    /// (at least half a unit, never the whole ruler), varied, drawn and keyed as picked, and kept by
    /// a saved figure and its link.
    #[test]
    fn random_length_and_position_are_on_the_ruler() {
        assert_eq!(random_key(ID), Some("length"));
        let d = def();
        for marks in ["1 cm", "5 mm", "1 mm", "1 in", "1/2 in", "1/4 in", "1/8 in", "1/16 in", "10 cm", "5 cm"] {
            for read_to in ["One estimated digit", "Nearest mark"] {
                for ruler in [3i64, 6, 15, 30, 100] {
                    let mut seen = std::collections::BTreeSet::new();
                    for seed in 0..120u64 {
                        let mut p = start_params(&d);
                        p.set_s("marks", marks);
                        p.set_s("read_to", read_to);
                        p.set_i("ruler", ruler);
                        p.set_b("answer", true);
                        assert!(random_reading(ID, &mut p, seed));
                        let (s, l) = (p.f("start"), p.f("length"));
                        let g = grain(&p);
                        let tag = format!("{marks} {read_to} {ruler}: start {s} length {l}");
                        assert!(((l / g).round() - l / g).abs() < 1e-6 && ((s / g).round() - s / g).abs() < 1e-6, "off the grid: {tag}");
                        assert!(l >= 0.5_f64.min(ruler as f64) - 1e-9 && l >= g - 1e-9, "too short to read: {tag}");
                        if g > 0.9 * ruler as f64 {
                            // marks coarser than the ruler (10 cm marks on a 3 cm ruler): the shortest
                            // length the figure draws, lined up with 0
                            assert!(s == 0.0 && l == g, "{tag}");
                        } else {
                            assert!(s >= 0.0 && s + l <= ruler as f64 + 1e-9, "off the ruler: {tag}");
                        }
                        // no float noise in what is stored, shown and saved
                        assert!(format!("{l}").len() <= 8 && format!("{s}").len() <= 8, "noisy: {tag}");
                        // what is drawn and keyed is what was picked
                        let (ds, dl) = ruler_values(&p);
                        assert!((ds - s).abs() < 1e-9 && (dl - l).abs() < 1e-9, "drawn as picked: {tag}");
                        let svg = d.svg(&p);
                        let key = super::super::ruler_fmt(l, &p);
                        assert!(svg.contains(&key), "the key states the drawn length {key}: {tag}");
                        seen.insert(((s * 1e4) as i64, (l * 1e4) as i64));
                        if seed % 40 == 0 {
                            let sp = spec::spec_for(ID, &p);
                            let q = spec::params_from_spec(&d, &sp);
                            assert_eq!(d.svg(&q), svg, "saved: {tag}");
                            let link = share::encode(ID, &p, &start_params(&d));
                            let (_, ch) = share::decode(&link).unwrap();
                            assert_eq!(d.svg(&share::link_params(&d, ch, share::version(&link))), svg, "link: {tag}");
                        }
                    }
                    let g = grain(&{ let mut p = start_params(&d); p.set_s("marks", marks); p.set_s("read_to", read_to); p });
                    assert!(seen.len() > if ruler as f64 / g >= 20.0 { 10 } else { 0 }, "{marks} {read_to} {ruler}: random readings should vary ({})", seen.len());
                }
            }
        }
        // the opening state's pick changes the figure
        let mut p = start_params(&d);
        let before = d.svg(&p);
        random_reading(ID, &mut p, 1);
        assert_ne!(d.svg(&p), before);
    }

    #[test]
    fn ruler_unit_follows_the_marks() {
        let mut p = start_params(&def());
        assert_eq!(unit(&p), "cm");
        p.set_s("marks", "1/8 in");
        assert_eq!(unit(&p), "in");
    }
}
