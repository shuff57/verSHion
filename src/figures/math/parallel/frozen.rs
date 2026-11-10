//! Old parallel-lines figures, frozen: share links and saved figures made before batch 5 (only the keys the figure had then)
//! must render byte-for-byte as main 303d812 drew them.
use crate::figures::{registry, FigureDef};
use serde_json::{json, Map, Value};

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == "parallel-lines").unwrap() }

/// FNV-1a, so the hashes below are the same in any build.
fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Payloads using only the keys the figure had before batch 5: a sweep over every old control, several at once.
pub(super) fn old_payloads() -> Vec<Map<String, Value>> {
    let mut v = Vec::new();
    let angles = [65.0, 30.0, 140.0, 90.0, 12.0, 168.0];
    let angle_bs = [115.0, 50.0, 90.0, 160.0];
    let marks = ["", "2, 6", "1, 2, 3, 4, 5, 6, 7, 8", "3, 6, 10, 14", "9, 10, 11, 12", "2, 13, 18, 24"];
    let unknowns = ["", "6", "2, 10", "13"];
    let texts = ["", "2 = 3x + 5; 6 = y", "14 = y; 3 = 2x"];
    let styles = ["Solid", "Dashed", "Dotted"];
    let arrows = ["None", "1", "2", "3"];
    let mut i = 0usize;
    for lines in [2, 3] {
        for trans in [1, 2] {
            for (ai, a) in angles.iter().enumerate() {
                for labels in ["Degrees", "Numbers"] {
                    for size in ["Medium", "Large", "Small"] {
                        i += 1;
                        let mut m = Map::new();
                        m.insert("lines".into(), json!(lines));
                        m.insert("transversals".into(), json!(trans));
                        m.insert("angle".into(), json!(a));
                        m.insert("angle_b".into(), json!(angle_bs[(ai + i) % angle_bs.len()]));
                        m.insert("labels".into(), json!(labels));
                        m.insert("label_size".into(), json!(size));
                        m.insert("marked".into(), json!(marks[i % marks.len()]));
                        m.insert("unknown".into(), json!(unknowns[i % unknowns.len()]));
                        m.insert("angle_text".into(), json!(texts[i % texts.len()]));
                        m.insert("gap".into(), json!([1.0, 0.6, 2.2, 3.0][i % 4]));
                        m.insert("turn".into(), json!([0.0, -20.0, 35.0, 180.0, 0.0][i % 5]));
                        m.insert("dec".into(), json!((i % 3) as i64));
                        m.insert("names".into(), json!(i % 7 != 3));
                        m.insert("line_ends".into(), json!(if i % 4 == 1 { "Plain" } else { "Arrowheads" }));
                        for (k, key) in ["m", "n", "p", "t", "u"].iter().enumerate() {
                            m.insert(format!("{key}_style"), json!(styles[(i + k) % 3]));
                            m.insert(format!("{key}_arrows"), json!(arrows[(i * 3 + k) % 4]));
                        }
                        if i % 5 == 2 {
                            m.insert("name_m".into(), json!("AB"));
                            m.insert("name_n".into(), json!("long line name"));
                            m.insert("name_p".into(), json!(""));
                            m.insert("name_t".into(), json!("s"));
                            m.insert("name_u".into(), json!("v"));
                        }
                        v.push(m);
                    }
                }
            }
        }
    }
    // the opening state and the shipped examples' params
    v.push(Map::new());
    for (k, val) in [("marked", "2, 6"), ("marked", "4, 6"), ("marked", "3, 6")] {
        let mut m = Map::new();
        m.insert(k.into(), json!(val));
        v.push(m);
    }
    v
}

pub(super) fn hashes() -> Vec<u64> {
    let d = def();
    old_payloads().into_iter().map(|m| fnv(&(d.render)(&crate::figures::share::link_params(&d, m, crate::figures::share::VERSION)))).collect()
}

/// The FNV hashes of the old payloads' SVGs as main (303d812) drew them, before batch 5.
const OLD_HASHES: &[u64] = &[
    10673094698287090136, 17404245748038873016, 9982922420186268980, 2270879145869335178, 16389147790092066932, 12245086746297062790,
    5912072376466317065, 2746458247896569980, 10182808815538227619, 9161599945371247694, 4854553925797875057, 17351835971444574967,
    9029021981482922138, 10453153747461203424, 679730950817851555, 12153945134316741984, 3485760841041002451, 2394271589983167162,
    5808347105514255194, 11210178301931086972, 11997929500194258188, 15094262573763414804, 3125878653269739355, 999733563102156543,
    3402123683061419691, 5026348180956422905, 13450420337141746905, 8106650866563952841, 10583604427532762000, 1320466217610534475,
    4704544743781903776, 3315049663133883366, 13752129129855220908, 13886063327688846413, 15142642822577897233, 15010209873107803995,
    11623458351560707203, 17886671368026911279, 7989518763323800747, 9037295353305943892, 1292416509722011765, 5367644039500473720,
    4917674826113987491, 3317718188723980834, 15853814989459049715, 12369903047609204905, 17963144512038510276, 14326808629498539759,
    3571310351356947623, 1666721604650347486, 5577262387211535094, 12626331422858639144, 10924802205179224850, 15506073149892180596,
    12565365402613202907, 7901728124083996097, 12226831185104568221, 1429928853512831058, 15881895563193721912, 15636487618036635363,
    17555363773331354079, 15404933209746092534, 18376752814263826023, 11978010331034664473, 9868961833287115903, 11960883597578340760,
    945189879771215917, 214217961931832717, 7971231025691809664, 4674319750442379797, 14001631205162424289, 12668860649912954452,
    9904184843785317310, 8090024650674536768, 6305071925186694545, 17009587391138857036, 10232802257044017911, 4095442195129831055,
    12155972742306477050, 6776839746541772576, 16853071574664631318, 5712747511128663788, 13795438130346932526, 10834393693158825820,
    270599712738312972, 12990463608706070270, 15252470265456619760, 15079303192208095180, 11755877123278641158, 2535513379015098584,
    18144537254067021077, 4136107920668260327, 5072183146835850428, 2037228248951397384, 15235499656911618036, 10670266857522312562,
    1514644644603319353, 16974630047854572729, 5268020415322622881, 4364426563292368939, 17078999377024668530, 13289247836724801858,
    3358058453993913049, 3164726250509707664, 6495085483528190146, 8134340067924927290, 10715395763956473537, 11490361976334761196,
    18206285179655094250, 1558831847153056125, 6770654276323499536, 8046329277535965069, 8743793869443029885, 3598961269172974048,
    14583248079267266062, 4779721973962067258, 16101155146565457843, 6398551893684540989, 14102999351783803201, 2459421931326077747,
    2057233422292082257, 17978665494615029707, 8051706152777738560, 473465809176493170, 7187254283971284417, 16568598460956095053,
    11380419681617918777, 2620610271296109630, 3314587996997532077, 1859243853953562562, 7725153981409203935, 17707792344353396855,
    5380244147560109371, 17919936470418567301, 4114324004553804204, 6635553999046201533, 5967798713031174909, 12881709021914795241,
    1559216765007788755, 10006868267112886159, 15582887917781881195, 4846035137204420900, 2620810991270077348, 18232818688327140980,
    1218330600637663169, 4378312910149889608, 14012116473138855277, 9617450270588752555,
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
    let p0 = crate::figures::share::link_params(&d, old_payloads()[7].clone(), crate::figures::share::VERSION);
    let old_keys: Vec<&str> = OLD_KEYS.to_vec();
    let spec = crate::model::FigureSpec {
        id: d.id.into(),
        params: p0.0.iter().filter(|(k, _)| old_keys.contains(&k.as_str())).map(|(k, v)| (k.clone(), crate::figures::spec::value_to_json(v))).collect(),
    };
    let p = crate::figures::spec::params_from_spec(&d, &spec);
    assert_eq!(fnv(&(d.render)(&p)), OLD_HASHES[7]);
}

/// Every key the figure had at 303d812.
const OLD_KEYS: [&str; 30] = [
    "lines", "transversals", "name_m", "name_n", "name_p", "name_t", "name_u", "angle", "angle_b", "m_style", "m_arrows", "n_style", "n_arrows", "p_style",
    "p_arrows", "t_style", "t_arrows", "u_style", "u_arrows", "gap", "labels", "marked", "unknown", "angle_text", "names", "dec", "turn", "line_ends",
    "label_size", "arrows",
];
