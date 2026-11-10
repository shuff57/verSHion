//! Old angles figures, frozen: share links and saved figures made before batch 6 (only the keys the figure had then) must
//! render byte-for-byte as main e0d5cd4 drew them.
use crate::figures::{registry, FigureDef};
use serde_json::{json, Map, Value};

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == "angles").unwrap() }

/// FNV-1a, so the hashes below are the same in any build.
fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Payloads using only the keys the figure had before batch 6: a sweep over every old control, several at once.
pub(super) fn old_payloads() -> Vec<Map<String, Value>> {
    let mut v = Vec::new();
    // ray rows 2-6 as typed (an empty row within the count is placed in the widest gap)
    let sets: [&[&str]; 14] = [
        &["60"], &["35", "180"], &["35", "145"], &["50"], &["50", "130", "250"], &["90", "180"], &["250"],
        &["30", "100", "200", "300", "330"], &["", "", ""], &["120", "", "", ""], &["", "", "", "", ""], &["abc", "-30", "400"], &[""], &["10", "20", "30", "40", "50"],
    ];
    let styles = ["Solid", "Dashed", "Dotted"];
    let unknowns = ["", "2", "1, 3", "5"];
    let hides = ["", "2, 4", "1"];
    let texts = ["", "1 = 2x + 10; 3 = y", "2 = x"];
    let sizes = ["Medium", "Large", "Small"];
    let ends = ["Arrows", "Plain", "Points"];
    let mut i = 0usize;
    for (si, set) in sets.iter().enumerate() {
        for variant in 0..6 {
            i += 1;
            let mut m = Map::new();
            for (k, r) in set.iter().enumerate() { m.insert(format!("ray{}", k + 2), json!(r)); }
            m.insert("ray_count".into(), json!((set.len() + 1).clamp(2, 6) as i64));
            m.insert("around".into(), json!(variant % 3 == 2));
            m.insert("unknown".into(), json!(unknowns[i % unknowns.len()]));
            m.insert("hide".into(), json!(hides[(i / 2) % hides.len()]));
            m.insert("sector_text".into(), json!(texts[(i / 3) % texts.len()]));
            m.insert("right".into(), json!(i % 5 != 1));
            m.insert("letters".into(), json!(variant % 2 == 0));
            m.insert("dec".into(), json!((i % 4) as i64));
            m.insert("turn".into(), json!([0.0, 40.0, -90.0, 180.0, 15.5][i % 5]));
            m.insert("ray_ends".into(), json!(ends[(i + si) % 3]));
            m.insert("shade".into(), json!(variant % 4 == 1));
            m.insert("label_size".into(), json!(sizes[(i + variant) % 3]));
            for r in 1..=6 {
                m.insert(format!("ray{r}_style"), json!(styles[(i + r) % 3]));
                m.insert(format!("ray{r}_line"), json!(if (i + r * 7) % 4 == 0 || (variant == 3 && r <= 2) { "Line" } else { "Ray" }));
            }
            v.push(m);
        }
    }
    // the opening state, the shipped examples and the census runs
    v.push(Map::new());
    let one = |pairs: &[(&str, Value)]| -> Map<String, Value> { pairs.iter().map(|(k, x)| (k.to_string(), x.clone())).collect() };
    v.push(one(&[("ray2", json!("125")), ("ray3", json!("180")), ("unknown", json!("2"))]));
    v.push(one(&[("ray2", json!("50")), ("ray1_line", json!("Line")), ("ray2_line", json!("Line")), ("unknown", json!("3")), ("hide", json!("2, 4"))]));
    v.push(one(&[("ray2", json!("100")), ("ray3", json!("220")), ("around", json!(true)), ("sector_text", json!("3 = 2x + 20"))]));
    v.push(one(&[("ray2", json!("35")), ("ray3", json!("90")), ("unknown", json!("2"))]));
    v.push(one(&[("ray2", json!("50")), ("ray1_line", json!("Line")), ("ray2_line", json!("Line")), ("turn", json!(40.0)), ("ray_ends", json!("Points")), ("sector_text", json!("1 = 2x")), ("shade", json!(false))]));
    v.push(one(&[("ray_count", json!(6)), ("letters", json!(true))]));
    v
}

pub(super) fn hashes() -> Vec<u64> {
    let d = def();
    old_payloads().into_iter().map(|m| fnv(&(d.render)(&crate::figures::share::link_params(&d, m, crate::figures::share::VERSION)))).collect()
}

/// The FNV hashes of the old payloads' SVGs as main (e0d5cd4) drew them, before batch 6.
const OLD_HASHES: &[u64] = &[
    3640240456810014425, 2866346082704618526, 12133699136040756798, 5325963465431358271, 9625318821616777381, 13966825970935212578,
    17320514408969919698, 16138987288170498336, 12739308749742572264, 9446149204706226119, 11783384929911685073, 8176203950898344895,
    11838351435975243046, 5013881701344176757, 15188161865565895092, 5777844275479179875, 9357077985037531632, 10959916863131460597,
    7980516048512352897, 6920148771661248269, 14760195044499593813, 15106407881859157259, 217625588115798518, 10404375397057471879,
    6696279075772220623, 13920019666742184028, 115206458592097546, 14539086008006247054, 7291714499348814614, 7470711097024684552,
    2115624564085578607, 5368120402627318120, 1194904652617223121, 5509987768515663751, 2215288520648024504, 13769041967886959412,
    11068453501665615220, 15594297296834509138, 8634387466807451409, 6197420447275864790, 18259227848374419643, 7602496574531467332,
    4531476120463128217, 10325279257937888683, 14734881618806783493, 6466801680272320942, 18400052440482498103, 267218114261138122,
    3242690173382027225, 17987091261992594369, 5499099733863254916, 11182178504172786137, 12612646958484159968, 4090273912214143788,
    12536090189869111362, 3185010785490546001, 12351880625407354892, 3472070340473968296, 12277212674312716592, 17936002368631714297,
    3493154779669581265, 3301016604845257084, 9460802852907420983, 15123449952784746370, 11756441158591443747, 3977017941292040950,
    3441616945569375686, 13070977058627916049, 10582340400908337770, 18186058846487896013, 15126632401455318283, 4733855094648507168,
    14110016507229932790, 17205558785135699934, 10218059625466563130, 12516043376614487997, 3090730427022197878, 9817313615903025789,
    6599594183306172400, 16346659502320300222, 1719194593364733217, 11071671508797535822, 9324237674740941875, 2618539739480936516,
    14388673162519089342, 18308717135652449648, 8014137478956652513, 17287272261291516861, 18352132566906078961, 17817708720503503024,
    14829191017366879563,
];

#[test]
fn old_share_links_and_saved_figures_render_byte_identically() {
    let got = hashes();
    if OLD_HASHES.is_empty() {
        println!("OLD_HASHES {got:?}");
        panic!("fill in OLD_HASHES from main");
    }
    assert_eq!(got.len(), OLD_HASHES.len());
    for (i, (g, o)) in got.iter().zip(OLD_HASHES).enumerate() {
        assert_eq!(g, o, "old link {i} ({:?}) renders differently", old_payloads()[i]);
    }
    // a saved figure stores every param it had: the same params as a full spec draw the same
    let d = def();
    for i in [3usize, 20, 47, 70] {
        let p0 = crate::figures::share::link_params(&d, old_payloads()[i].clone(), crate::figures::share::VERSION);
        let spec = crate::model::FigureSpec {
            id: d.id.into(),
            params: p0.0.iter().filter(|(k, _)| OLD_KEYS.contains(&k.as_str())).map(|(k, v)| (k.clone(), crate::figures::spec::value_to_json(v))).collect(),
        };
        let p = crate::figures::spec::params_from_spec(&d, &spec);
        assert_eq!(fnv(&(d.render)(&p)), OLD_HASHES[i], "saved figure {i}");
    }
}

/// Every key the figure had at e0d5cd4.
const OLD_KEYS: [&str; 29] = [
    "ray2", "ray3", "ray4", "ray5", "ray6", "ray1_style", "ray1_line", "ray2_style", "ray2_line", "ray3_style", "ray3_line", "ray4_style", "ray4_line",
    "ray5_style", "ray5_line", "ray6_style", "ray6_line", "around", "unknown", "hide", "sector_text", "right", "letters", "dec", "turn", "ray_ends",
    "shade", "label_size", "ray_count",
];
