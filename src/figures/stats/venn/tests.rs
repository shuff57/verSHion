//! Venn diagram tests: old saved figures, layout, shading, the click and typing on the figure.
pub(crate) mod golden {
    /// Saved Venn figures from before batch 3 (only the keys that existed then), loaded through the real load path.
    pub fn old_specs() -> Vec<(String, crate::model::FigureSpec)> {
        use serde_json::json;
        let shades = ["None", "A", "B", "C", "A and B", "A or B", "A only", "B only", "Not A", "Not B", "Outside"];
        let counts2 = ["12, 9, 5, 4", "4 6 8, 3 5 7, 2, 1 9", "x, 2x+1, 5, 4", "1.5, 2.25, 0, 10", "", "a very very long value indeed, b, c, d", "7, , , 3"];
        let counts3 = ["8, 6, 5, 4, 3, 2, 1, 7", "1, 2, 3, 4, 5, 6, 7, 8", "a, b, c, d, e, f, g, h", "", "10, 0, 0, 2.5, 0, 0, 1, 3", "word, 2 4, x+1, 9, 8, 7, 6, 5", "3"];
        let names = [("Music", "Sport", "Art"), ("Even", "Prime", "Odd"), ("Mathematician", "Basketballers", "Choir members")];
        let labels = ["Large", "Medium", "Small"];
        let mut out = vec![];
        let mut i = 0usize;
        for sets in ["2", "3"] {
            for c in 0..7 {
                for (ni, nm) in names.iter().enumerate() {
                    for answers in [true, false] {
                        for totals in [true, false] {
                            for lab in labels {
                                i += 1;
                                let counts = if sets == "2" { counts2[c] } else { counts3[c] };
                                let params = json!({
                                    "sets": sets, "a_name": nm.0, "b_name": nm.1, "c_name": nm.2, "counts": counts,
                                    "shade": shades[i % shades.len()], "totals": totals, "title": if i % 3 == 0 { "Class survey" } else { "" },
                                    "style": if i % 4 == 1 { "Print" } else { "Colour" }, "label_size": lab, "show_answers": answers,
                                });
                                out.push((format!("{sets}-{c}-{ni}-{answers}-{totals}-{lab}-{i}"), spec(params)));
                            }
                        }
                    }
                }
            }
            for sh in shades {
                for style in ["Colour", "Print"] {
                    out.push((format!("{sets}-{sh}-{style}").replace(' ', "_"), spec(json!({"sets": sets, "shade": sh, "style": style, "counts": if sets == "2" { "12, 9, 5, 4" } else { "8, 6, 5, 4, 3, 2, 1, 7" }}))));
                }
            }
        }
        out
    }

    fn spec(params: serde_json::Value) -> crate::model::FigureSpec {
        let m = params.as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        crate::model::FigureSpec { id: "venn-diagram".into(), params: m }
    }

    pub fn fnv64(s: &str) -> u64 {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in s.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }

    pub fn fixture() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/venn_old_specs.txt") }

    /// Every saved spec above renders byte for byte as it did at main bd2ea88 (the hashes were written by that build).
    #[test]
    fn old_specs_render_as_before() {
        let def = super::super::def();
        let now: Vec<(String, String)> = old_specs().into_iter().map(|(n, s)| (n, format!("{:016x}", fnv64(&def.svg(&crate::figures::spec::params_from_spec(&def, &s)))))).collect();
        if let Ok(dir) = crate::env_var("VERSHION_VENN_DUMP") {
            std::fs::create_dir_all(&dir).unwrap();
            for (n, s) in old_specs() {
                std::fs::write(format!("{dir}/{n}.svg"), def.svg(&crate::figures::spec::params_from_spec(&def, &s))).unwrap();
            }
        }
        if crate::env_var("VERSHION_VENN_BLESS").is_ok_and(|v| v == "1") {
            std::fs::write(fixture(), now.iter().map(|(n, h)| format!("{n} {h}\n")).collect::<String>()).unwrap();
            return;
        }
        let text = std::fs::read_to_string(fixture()).unwrap();
        let old: std::collections::BTreeMap<&str, &str> = text.lines().filter_map(|l| l.split_once(' ')).collect();
        let changed: Vec<&String> = now.iter().filter(|(n, h)| old.get(n.as_str()) != Some(&h.as_str())).map(|x| &x.0).collect();
        let unexpected: Vec<&&String> = changed.iter().filter(|n| !mended(n)).collect();
        let kept: Vec<&String> = now.iter().map(|x| &x.0).filter(|n| mended(n) && !changed.contains(n)).collect();
        println!("changed: {changed:?}");
        assert!(unexpected.is_empty(), "{} of {} old Venn specs render differently: {unexpected:?}", unexpected.len(), now.len());
        assert!(kept.is_empty(), "these were drawn over a line before and still are: {kept:?}");
        println!("{} old specs byte-identical, {} changed where the old drawing was broken", now.len() - changed.len(), changed.len());
    }

    /// The old specs whose drawing was broken and is now mended (each checked by eye, old against new): a region value
    /// of 14 characters ran across the circles at Medium and Large (case 5 of two sets, "a very very lo"); with the long
    /// names, the third set's name ran past the box at Large, and n(Choir member) = ran into its total (or its blank) at Medium and Large
    /// (cases 2 and 5 of three sets are text, so no total is drawn with the answers).
    pub fn mended(name: &str) -> bool {
        let f: Vec<&str> = name.split('-').collect();
        let [sets, counts, names, answers, totals, label, ..] = f[..] else { return false };
        let big = label == "Large" || label == "Medium";
        (sets == "2" && counts == "5" && answers == "true" && big)
            || (sets == "3" && names == "2" && (label == "Large" || (label == "Medium" && totals == "true" && (answers == "false" || !matches!(counts, "2" | "5")))))
    }
}

use super::*;

fn defaults() -> Params { Params::defaults(&params()) }

fn with(list: &[(&str, &str)]) -> Params {
    let mut p = defaults();
    for (k, v) in list {
        match *v {
            "true" | "false" => p.set_b(k, *v == "true"),
            _ => p.set_s(k, v),
        }
    }
    p
}

fn size_of(s: &str) -> String { s[s.find("viewBox").unwrap()..].split('"').nth(1).unwrap().to_string() }

/// The five layouts, as (sets, arrangement), with their circles written out here by hand (centre x, y, radius, no title).
fn layouts() -> Vec<(&'static str, &'static str, Vec<(f64, f64, f64)>)> {
    vec![
        ("1", "Overlapping", vec![(260.0, 168.0, 110.0)]),
        ("2", "Overlapping", vec![(205.0, 160.0, 105.0), (315.0, 160.0, 105.0)]),
        ("2", "Separate", vec![(140.0, 168.0, 100.0), (380.0, 168.0, 100.0)]),
        ("2", "B inside A", vec![(260.0, 172.0, 118.0), (300.0, 192.0, 62.0)]),
        ("3", "Overlapping", vec![(205.0, 150.0, 95.0), (315.0, 150.0, 95.0), (260.0, 245.0, 95.0)]),
    ]
}

fn layout_params(sets: &str, arr: &str) -> Params { with(&[("sets", sets), ("arrangement", arr)]) }

/// Membership of `q` by the hand-written circles.
fn mask_of(c: &[(f64, f64, f64)], q: Pt) -> usize {
    c.iter().enumerate().filter(|(_, c)| ((q.0 - c.0).powi(2) + (q.1 - c.1).powi(2)).sqrt() <= c.2).map(|(i, _)| 1 << i).sum()
}

#[test]
fn two_sets_show_regions_and_totals() {
    let svg = render(&defaults());
    for t in [">Music<", ">Sport<", ">12<", ">9<", ">5<", ">4<"] {
        assert!(svg.contains(t), "{t}");
    }
    // 12 + 5 = 17 in Music, 9 + 5 = 14 in Sport, 5 in both, 26 in either, 30 in all
    for t in [">17<", ">14<", ">26<", ">30<"] {
        assert!(svg.contains(t), "{t}");
    }
    assert_eq!(svg.matches("<circle").count(), 2);
}

#[test]
fn three_sets_total_by_membership() {
    let p = with(&[("sets", "3"), ("counts", "8, 6, 5, 4, 3, 2, 1, 7")]);
    let svg = render(&p);
    // A = 8 + 4 + 3 + 1 = 16, B = 6 + 4 + 2 + 1 = 13, C = 5 + 3 + 2 + 1 = 11, all = 36
    for t in [">16<", ">13<", ">11<", ">36<"] {
        assert!(svg.contains(t), "{t}");
    }
    assert_eq!(svg.matches("<circle").count(), 3);
}

#[test]
fn one_set_separate_and_inside_total_by_their_regions() {
    // one set: 7 in A, 3 outside
    let svg = render(&with(&[("sets", "1"), ("counts", "7, 3")]));
    assert_eq!(svg.matches("<circle").count(), 1);
    assert!(svg.contains(">n(Music) =<") && svg.contains(">10<") && !svg.contains("Sport"));
    // separate: A 5, B 6, neither 2: nothing in both, 11 in either, 13 in all
    let svg = render(&with(&[("arrangement", "Separate"), ("counts", "5, 6, 2")]));
    for t in [">5<", ">6<", ">2<", ">0<", ">11<", ">13<"] {
        assert!(svg.contains(t), "separate {t}");
    }
    // B inside A: A only 6, B 4, outside 3: n(A) = 10, n(B) = 4, both 4, either 10, all 13
    let svg = render(&with(&[("arrangement", "B inside A"), ("counts", "6, 4, 3")]));
    for t in [">10<", ">4<", ">13<"] {
        assert!(svg.contains(t), "inside {t}");
    }
    // a value too many for these layouts is ignored, not an error (the overlapping ones keep their message)
    assert!(render(&with(&[("arrangement", "Separate"), ("counts", "1, 2, 3, 4")])).contains("<circle"));
    assert!(render(&with(&[("counts", "1, 2, 3, 4, 5")])).contains("there are 5 values"));
}

#[test]
fn student_copy_hides_every_answer_even_in_the_description() {
    let def = def();
    for p in [
        with(&[("shade", "A and B"), ("counts", "red flesh; mushy, 2x+1, 5, 4")]),
        with(&[("mode", "Sets"), ("sets", "3")]),
        with(&[("mode", "Sets"), ("arrangement", "Separate")]),
    ] {
        let (student, key) = crate::figures::answer_pair(&def, &p).unwrap();
        assert_eq!(size_of(&student), size_of(&key));
        assert!(student.contains("vn-in0") && student.contains(">Music<"));
        for t in ["red flesh", "2x+1", ">5<", ">26<", "Ana", "Cara", "Dev", "Gus", "Not drawn"] {
            assert!(!student.contains(t), "student copy shows {t}");
        }
        // the description exported with the SVG says nothing about the values either
        let desc: String = student.split("<desc>").nth(1).map(|s| s.split("</desc>").next().unwrap().to_string()).unwrap_or_default();
        let title: String = student.split("<title>").nth(1).map(|s| s.split("</title>").next().unwrap().to_string()).unwrap_or_default();
        for t in ["12", "Ana", "flesh", "Cara"] {
            assert!(!desc.contains(t) && !title.contains(t), "{t} in the description");
        }
    }
    assert!(crate::figures::answer_pair(&def, &with(&[("mode", "Sets")])).unwrap().1.contains("Cara"));
}

/// The fixed Shade list's meaning (as it was): every old option still shades exactly those regions.
#[test]
fn old_shade_options_shade_what_they_did() {
    fn old(shade: &str, mask: usize) -> bool {
        let (a, b, c) = (mask & 1 != 0, mask & 2 != 0, mask & 4 != 0);
        match shade {
            "A" => a,
            "B" => b,
            "C" => c,
            "A and B" => a && b,
            "A or B" => a || b,
            "A only" => a && !b && !c,
            "B only" => b && !a && !c,
            "Not A" => !a,
            "Not B" => !b,
            "Outside" => mask == 0,
            _ => false,
        }
    }
    for sets in ["2", "3"] {
        let g = Geo::new(&with(&[("sets", sets)]));
        for sh in ["None", "A", "B", "C", "A and B", "A or B", "A only", "B only", "Not A", "Not B", "Outside"] {
            let got = parse_shade(sh, &g.names, g.all());
            for m in g.masks() {
                assert_eq!(got & (1 << m) != 0, old(sh, m), "{sets} sets, {sh}, region {m}");
            }
        }
    }
}

#[test]
fn shading_reads_set_expressions() {
    let g = Geo::new(&with(&[("sets", "3")]));
    let r = |s: &str| parse_shade(s, &g.names, g.all());
    let set = |ms: &[usize]| ms.iter().fold(0u8, |a, m| a | 1 << m);
    assert_eq!(r("(A or B) and not C"), set(&[1, 2, 3]));
    assert_eq!(r("A \u{2229} B"), set(&[3, 7]));
    assert_eq!(r("A \u{222a} C"), set(&[1, 3, 4, 5, 6, 7]));
    assert_eq!(r("A'"), set(&[0, 2, 4, 6]));
    assert_eq!(r("(A \u{222a} B)'"), set(&[0, 4]));
    assert_eq!(r("A minus B"), set(&[1, 5]));
    assert_eq!(r("A and B only"), set(&[3]));
    assert_eq!(r("all three"), set(&[7]));
    assert_eq!(r("Music and Sport and not Art"), set(&[3]));
    assert_eq!(r("A only, C only"), set(&[1, 4]));
    assert_eq!(r("outside, A and B and C"), set(&[0, 7]));
    assert_eq!(r("Everything"), g.all());
    assert_eq!(r("none"), 0);
    assert_eq!(r("A and"), 0, "a part that does not read shades nothing");
    assert_eq!(r("banana, B only"), set(&[2]));
    // in B inside A there is no 'B only' region; B is the inner circle
    let g = Geo::new(&layout_params("2", "B inside A"));
    assert_eq!(parse_shade("B", &g.names, g.all()), set(&[3]));
    assert_eq!(parse_shade("B only", &g.names, g.all()), 0);
    assert_eq!(parse_shade("A only", &g.names, g.all()), set(&[1]));
}

/// Every pixel of the universal box (every 3 units, at two zooms) goes through the workspace's own hit test: a click there
/// shades exactly the region the point is in (by the circles written out above) and a second click clears it.
#[test]
fn a_click_anywhere_shades_exactly_the_region_under_it() {
    for (sets, arr, circles) in layouts() {
        let p = layout_params(sets, arr);
        let svg = render(&p);
        let size = crate::figures::svg_size(&svg);
        let hs = crate::figures::interact::handles_for(ID, &p, size);
        let g = Geo::new(&p);
        let (mut shaded, mut names) = (std::collections::BTreeMap::<usize, usize>::new(), 0);
        for k in [1.0, 1.6] {
            let mut y = 1.0;
            while y < size.1 {
                let mut x = 1.0;
                while x < size.0 {
                    let q = (x, y);
                    x += 3.0;
                    let inside_box = q.0 >= 20.0 && q.0 <= 500.0 && q.1 >= 20.0 && q.1 <= g.rect.3;
                    let Some(i) = interact::pick(&hs, q, k) else {
                        assert!(!inside_box, "{sets} {arr}: nothing under {q:?}");
                        continue;
                    };
                    let h = &hs[i];
                    if h.edit.is_some() {
                        names += 1;
                        continue;
                    }
                    let f = h.click_at.as_ref().expect("the region area");
                    let mut q2 = p.clone();
                    f(&mut q2, q);
                    let got = parse_shade(q2.s("shade"), &g.names, g.all());
                    if !inside_box {
                        assert_eq!(got, 0, "{sets} {arr}: a click outside the box at {q:?} shaded {}", q2.s("shade"));
                        continue;
                    }
                    let want = mask_of(&circles, q);
                    assert_eq!(got, 1 << want, "{sets} {arr}: click at {q:?} shaded {:?}, not region {want}", q2.s("shade"));
                    *shaded.entry(want).or_default() += 1;
                    f(&mut q2, q);
                    assert_eq!(q2.s("shade"), "None", "{sets} {arr}: a second click at {q:?} clears it");
                }
                y += 3.0;
            }
        }
        let mut want: Vec<usize> = g.masks();
        want.sort();
        assert_eq!(shaded.keys().copied().collect::<Vec<_>>(), want, "{sets} {arr}: every region is clickable");
        assert!(names > 0, "{sets} {arr}: the names take their own double-click");
        println!("{sets} {arr}: {shaded:?}, {names} on names");
    }
}

/// Clicks build up any combination, written back as a familiar name when there is one.
#[test]
fn clicks_combine_and_read_back() {
    let p = with(&[("sets", "3")]);
    let g = Geo::new(&p);
    let mut s = "None".to_string();
    for m in [1, 3, 5, 7] {
        s = toggled(&g, &s, m);
    }
    assert_eq!(s, "A");
    s = toggled(&g, &s, 0);
    assert_eq!(parse_shade(&s, &g.names, g.all()), 0b1010_1011);
    assert_eq!(s, "A only, A and B only, A and C only, A and B and C, Outside");
    let p = layout_params("2", "Overlapping");
    let g = Geo::new(&p);
    assert_eq!(toggled(&g, "A or B", 3), "A only, B only");
    assert_eq!(toggled(&g, "A only, B only", 3), "A or B");
    assert_eq!(toggled(&g, "Not A", 0), "B only");
}

/// The rendered shading covers exactly the chosen regions: pixels at least 4 units inside each region are filled when the
/// region is chosen and white when it is not, for every combination of regions in the one- and two-set layouts and a
/// spread of them with three sets, in a tint, a pattern and Print.
#[test]
fn shading_covers_exactly_the_chosen_regions() {
    for (sets, arr, _) in layouts() {
        let base = with(&[("sets", sets), ("arrangement", arr), ("counts", ""), ("totals", "false"), ("universal", "None")]);
        let g = Geo::new(&base);
        let masks = g.masks();
        let combos: Vec<u32> = if masks.len() <= 4 { (0..1u32 << masks.len()).collect() } else { (0..256).step_by(7).chain([255]).collect() };
        let avoid = g.name_boxes();
        let pts: Vec<(Pt, usize)> = (0..((g.rect.3 - g.rect.1) / 7.0) as usize)
            .flat_map(|j| (0..(480 / 7)).map(move |i| (24.0 + i as f64 * 7.0, g.rect.1 + 4.0 + j as f64 * 7.0)))
            .filter(|q| !avoid.iter().any(|b| q.0 > b.0 - 6.0 && q.0 < b.2 + 6.0 && q.1 > b.1 - 6.0 && q.1 < b.3 + 6.0))
            .filter_map(|q| masks.iter().find(|m| g.inside(**m, q, 6.0)).map(|m| (q, *m)))
            .collect();
        for fill in ["Blue", "Hatching", "Print"] {
            for c in &combos {
                let chosen: Vec<usize> = masks.iter().enumerate().filter(|(i, _)| c & (1 << i) != 0).map(|(_, m)| *m).collect();
                let text: Vec<&str> = chosen.iter().map(|m| g.region_code(*m)).collect();
                let mut p = base.clone();
                let joined = text.join(", ");
                p.set_s("shade", if text.is_empty() { "None" } else { &joined });
                if fill == "Print" { p.set_s("style", "Print") } else { p.set_s("fill", fill) }
                let png = crate::figures::to_png(&render(&p), 1.0).unwrap();
                let img = image::load_from_memory(&png).unwrap().to_rgba8();
                let sc = img.width() as f64 / 520.0;
                for (q, m) in &pts {
                    // a 5 x 5 window: any ink (a tint, a hatch line, a dot) counts as filled
                    let filled = (-2..=2).flat_map(|dx| (-2..=2).map(move |dy| (dx, dy))).any(|(dx, dy)| {
                        let px = img.get_pixel(((q.0 * sc) as i64 + dx) as u32, ((q.1 * sc) as i64 + dy) as u32);
                        px[0] < 245 || px[1] < 245 || px[2] < 245
                    });
                    assert_eq!(filled, chosen.contains(m), "{sets} {arr} {fill} {:?}: region {m} at {q:?}", p.s("shade"));
                }
            }
        }
    }
}

/// Region text stays inside its region, clear of every line and of the other labels, at least 8 units high, at every
/// label size, with long names and long text, in every layout and in Sets mode (measured on the real glyphs).
#[test]
fn region_text_fits_its_region() {
    let long = [("a_name", "Mathematicia"), ("b_name", "Basketballer"), ("c_name", "Choir member")];
    let texts = ["red flesh; mushy; savory, 2x + 1, crunchy; sweet; good in pies, 3/20, x, y, z, w", "12, 9, 5, 4, 3, 2, 1, 7", "a very long description of what goes here, another long one, x, y, z, a, b, c"];
    let mut checked = 0;
    for (sets, arr, circles) in layouts() {
        for size in ["Large", "Medium", "Small"] {
            for (ti, t) in texts.iter().enumerate() {
                for mode in ["Text", "Sets"] {
                    let mut p = with(&[("sets", sets), ("arrangement", arr), ("counts", t), ("label_size", size), ("mode", mode), ("universal", "Word"), ("u_word", "Everyone at our school")]);
                    if ti > 0 {
                        for (k, v) in long {
                            p.set_s(k, v);
                        }
                    }
                    if mode == "Sets" {
                        p.set_s("u_set", "1-30");
                        p.set_s("a_set", "2, 4, 6, 8, 10, 12, 14, 16, 18, 20");
                        p.set_s("b_set", "3, 6, 9, 12, 15, 18");
                        p.set_s("c_set", "5, 10, 15, 20, 25, 30");
                    }
                    let svg = render(&p);
                    let g = Geo::new(&p);
                    if !svg.contains("<circle") { continue; }
                    let boxes = ink_boxes(&svg);
                    let inner: Vec<&(String, Bx, f64)> = boxes.iter().filter(|b| b.1 .3 < g.rect.3 + 1.0).collect();
                    for (s, b, fsz) in &inner {
                        assert!(*fsz >= 8.0, "{sets} {arr} {size} {s:?}: font {fsz}");
                        assert!(b.0 >= g.rect.0 + 1.5 && b.2 <= g.rect.2 - 1.5 && b.1 >= g.rect.1 + 1.5 && b.3 <= g.rect.3 - 1.5, "{sets} {arr} {size} {s:?} {b:?} leaves the box");
                        for c in &circles {
                            for q in samples(b) {
                                let d = ((q.0 - c.0).powi(2) + (q.1 + 0.0 - c.1 - g.top).powi(2)).sqrt();
                                assert!((d - c.2).abs() >= 1.6, "{sets} {arr} {size} {mode} {s:?} {b:?} crosses a circle");
                            }
                        }
                    }
                    for i in 0..inner.len() {
                        for j in i + 1..inner.len() {
                            assert!(!overlaps(&inner[i].1, &inner[j].1), "{sets} {arr} {size} {mode}: {:?} overlaps {:?}", inner[i].0, inner[j].0);
                        }
                    }
                    checked += inner.len();
                }
            }
        }
    }
    println!("{checked} labels checked");
}

/// Ink boxes of every text element (text, box, font size), from usvg's real layout.
fn ink_boxes(svg: &str) -> Vec<(String, Bx, f64)> {
    fn walk(g: &resvg::usvg::Group, out: &mut Vec<(String, Bx, f64)>) {
        for n in g.children() {
            match n {
                resvg::usvg::Node::Group(g) => walk(g, out),
                resvg::usvg::Node::Text(t) => {
                    let r = t.flattened().bounding_box().transform(t.abs_transform()).unwrap_or(t.flattened().abs_bounding_box());
                    let txt: String = t.chunks().iter().map(|c| c.text()).collect();
                    let fsz = t.chunks().first().and_then(|c| c.spans().first()).map(|s| s.font_size().get() as f64).unwrap_or(0.0);
                    out.push((txt, (r.left() as f64, r.top() as f64, r.right() as f64, r.bottom() as f64), fsz));
                }
                _ => {}
            }
        }
    }
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = crate::figures::fontdb();
    let tree = resvg::usvg::Tree::from_str(svg, &opt).expect("the SVG parses");
    let mut v = Vec::new();
    walk(tree.root(), &mut v);
    v
}

/// A double-click in a region (found through the real hit test) edits that region's text: commas start new lines,
/// the other regions keep theirs.
#[test]
fn double_click_types_in_the_region() {
    for (sets, arr, circles) in layouts() {
        let p = with(&[("sets", sets), ("arrangement", arr), ("counts", "")]);
        let g = Geo::new(&p);
        let size = crate::figures::svg_size(&render(&p));
        let hs = crate::figures::interact::handles_for(ID, &p, size);
        let mut q = p.clone();
        for (slot, (m, anchors)) in g.regions.iter().enumerate() {
            let at = anchors[0];
            assert_eq!(mask_of(&circles, (at.0, at.1 - g.top)), *m);
            let h = &hs[interact::pick(&hs, at, 1.0).unwrap()];
            let (key, f) = h.edit_at.as_ref().expect("the area types");
            assert_eq!(*key, "counts");
            let e = f(&q, at).expect("a region to type in");
            assert_eq!(e.text, "", "{sets} {arr} region {m} starts empty");
            (e.set)(&mut q, &format!("r{slot}, line two"));
            let e = f(&q, at).unwrap();
            assert_eq!(e.text, format!("r{slot}, line two"), "{sets} {arr}: reads back with commas");
        }
        let svg = render(&q);
        for slot in 0..g.regions.len() {
            assert!(svg.contains(&format!(">r{slot}<")), "{sets} {arr}: slot {slot} drawn: {}", q.s("counts"));
        }
        assert_eq!(svg.matches(">line two<").count() + svg.matches(">two<").count(), g.regions.len(), "{sets} {arr}: each second line on lines of its own");
        // retyping one region leaves the others
        let at = g.regions[0].1[0];
        let e = hs[interact::pick(&hs, at, 1.0).unwrap()].edit_at.as_ref().unwrap().1(&q, at).unwrap();
        (e.set)(&mut q, "");
        assert!(!render(&q).contains(">r0<") && render(&q).contains(">r1<"));
    }
    // Sets mode sorts the members itself: nothing to type in a region
    let p = with(&[("mode", "Sets")]);
    let hs = crate::figures::interact::handles_for(ID, &p, crate::figures::svg_size(&render(&p)));
    assert!(hs[0].edit_at.as_ref().unwrap().1(&p, (150.0, 165.0)).is_none());
}

#[test]
fn sets_mode_sorts_members_into_regions() {
    // 1 to 12, even and prime: 2 is both; 4 6 8 10 12 even only; 3 5 7 11 prime only; 1 and 9 neither
    let p = with(&[("mode", "Sets"), ("u_set", "1-12"), ("a_set", "2, 4, 6, 8, 10, 12"), ("b_set", "2, 3, 5, 7, 11")]);
    let g = Geo::new(&p);
    let (r, lost) = sorted(&p, &g);
    assert_eq!(r[1], ["4", "6", "8", "10", "12"]);
    assert_eq!(r[2], ["3", "5", "7", "11"]);
    assert_eq!(r[3], ["2"]);
    assert_eq!(r[0], ["1", "9"]);
    assert!(lost.is_empty());
    let svg = render(&p);
    // n(A) = 6, n(B) = 5, n(A and B) = 1, n(A or B) = 10, n(all) = 12
    for t in [">6<", ">5<", ">1<", ">10<", ">12<"] {
        assert!(svg.contains(t), "{t}");
    }
    // prime factors: 12 = 2 x 2 x 3, 18 = 2 x 3 x 3; the HCF 2 x 3 = 6 in the middle
    let p = with(&[("mode", "Sets"), ("u_set", ""), ("a_set", "2, 2, 3"), ("b_set", "3, 2, 3")]);
    let (r, _) = sorted(&p, &Geo::new(&p));
    assert_eq!((r[1].clone(), r[2].clone(), r[3].clone(), r[0].clone()), (vec!["2".to_string()], vec!["3".to_string()], vec!["2".to_string(), "3".to_string()], vec![]));
    // three sets, words, first-typed order
    let p = with(&[("mode", "Sets"), ("sets", "3")]);
    let (r, _) = sorted(&p, &Geo::new(&p));
    assert_eq!(r[1], ["Ana"]);
    assert_eq!(r[7], ["Dev"]);
    assert_eq!(r[5], ["Ben"]);
    assert_eq!(r[0], ["Gus"]);
    // separate sets cannot show a member of both: it is named under the figure (key only)
    let p = with(&[("mode", "Sets"), ("arrangement", "Separate")]);
    let svg = render(&p);
    assert!(svg.contains("Not drawn (the sets are drawn apart): Cara, Dev"), "{svg}");
    let p = with(&[("mode", "Sets"), ("arrangement", "B inside A"), ("a_set", "1, 2"), ("b_set", "2, 3"), ("u_set", "")]);
    assert!(render(&p).contains("Not drawn (B is drawn inside A): 3"));
    // ranges and odd input
    assert_eq!(members("1-3, x, , 7-5, -2"), ["1", "2", "3", "x", "7-5", "-2"]);
    assert_eq!(members("1-100000").len(), 1);
}

#[test]
fn old_saved_figures_migrate_and_new_ones_keep_their_choices() {
    let def = def();
    let spec = |v: serde_json::Value| crate::model::FigureSpec { id: ID.into(), params: v.as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.clone())).collect() };
    let old = crate::figures::spec::params_from_spec(&def, &spec(serde_json::json!({"shade": "A and B", "counts": "a value far too long to draw, 2"})));
    assert_eq!(old.s("fill"), "Strong blue");
    assert_eq!(old.s("counts"), "a value far to, 2");
    let new = crate::figures::spec::params_from_spec(&def, &spec(serde_json::json!({"arrangement": "Overlapping", "shade": "A and B", "fill": "Green", "counts": "a value far too long to draw, 2"})));
    assert_eq!(new.s("fill"), "Green");
    assert_eq!(new.s("counts"), "a value far too long to draw, 2");
    // a saved figure round-trips through its spec
    let p = with(&[("arrangement", "Separate"), ("fill", "Dots"), ("shade", "A, Outside")]);
    let back = crate::figures::spec::params_from_spec(&def, &crate::figures::spec::spec_for(ID, &p));
    assert_eq!(render(&back), render(&p));
}

#[test]
fn every_new_control_changes_the_drawing() {
    let base = render(&defaults());
    for (k, v) in [
        ("mode", "Sets"),
        ("sets", "1"),
        ("arrangement", "Separate"),
        ("arrangement", "B inside A"),
        ("universal", "\u{3be}"),
        ("universal", "S"),
        ("universal", "\u{3b5}"),
        ("universal", "None"),
        ("counts", "red flesh; mushy, 9, 5, 4"),
        ("shade", "A only, B only"),
    ] {
        assert_ne!(render(&with(&[(k, v)])), base, "{k} = {v}");
    }
    let sh = render(&with(&[("shade", "A")]));
    for f in FILLS.iter().filter(|f| **f != "Strong blue") {
        assert_ne!(render(&with(&[("shade", "A"), ("fill", f)])), sh, "fill {f}");
    }
    assert_ne!(render(&with(&[("universal", "Word"), ("u_word", "Class 9")])), base);
    // an empty region gets a line to write on (the filled ones keep their values)
    let gap = with(&[("counts", "12, 9, , 4")]);
    let mut lined = gap.clone();
    lined.set_s("empty", "A blank line");
    assert_eq!(render(&lined).matches("<line").count(), render(&gap).matches("<line").count() + 1);
    // the empty-region line shows on the student copy
    let s = render(&with(&[("empty", "A blank line"), ("show_answers", "false")]));
    assert!(s.matches("<line").count() >= 4 + 5, "a line in each of 4 regions and the 5 totals");
}

#[test]
fn tints_keep_black_text_readable_and_print_is_grey() {
    let lum = |hex: &str| {
        let c = |i: usize| {
            let v = u8::from_str_radix(&hex[1 + 2 * i..3 + 2 * i], 16).unwrap() as f64 / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * c(0) + 0.7152 * c(1) + 0.0722 * c(2)
    };
    let fills = Fills::new(&defaults());
    for f in FILLS.iter().filter(|f| !matches!(**f, "Hatching" | "Cross-hatching" | "Dots" | "Strong blue")) {
        let (c, _, _) = paint(&with(&[("fill", f)]), &fills);
        let ratio = (lum(&c) + 0.05) / (lum(INK) + 0.05);
        assert!(ratio >= 4.5, "{f} {c}: {ratio:.2}");
    }
    for f in FILLS {
        let p = with(&[("style", "Print"), ("shade", "A or B"), ("fill", f), ("sets", "3"), ("counts", "1, 2, 3, 4, 5, 6, 7, 8")]);
        let png = crate::figures::to_png(&render(&p), 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24), "{f} in Print");
    }
}

#[test]
fn hover_outlines_the_region_under_the_pointer() {
    for (sets, arr, circles) in layouts() {
        let p = layout_params(sets, arr);
        let g = Geo::new(&p);
        let hs = crate::figures::interact::handles_for(ID, &p, crate::figures::svg_size(&render(&p)));
        let hov = hs[0].hover_at.as_ref().unwrap();
        for (m, anchors) in &g.regions {
            let h = hov(anchors[0]).unwrap_or_else(|| panic!("{sets} {arr}: no hover in {m}"));
            assert!(h.closed && h.outline.len() >= 4);
            assert!(h.tip.contains("click to shade it") && h.tip.contains("double-click to type"), "{}", h.tip);
            if *m != 0 {
                // every outline point is on a circle that bounds the region
                for q in &h.outline {
                    assert!(circles.iter().any(|c| (((q.0 - c.0).powi(2) + (q.1 - c.1 - g.top).powi(2)).sqrt() - c.2).abs() < 0.5), "{sets} {arr} region {m}: {q:?}");
                }
            }
        }
        assert!(hov((5.0, 5.0)).is_none());
    }
}

#[test]
fn examples_render_and_hostile_input_is_safe() {
    for ex in examples(ID).unwrap() {
        let mut p = defaults();
        for (k, v) in &ex.params {
            match v {
                Value::S(s) => p.set_s(k, s),
                Value::B(b) => p.set_b(k, *b),
                _ => {}
            }
        }
        let svg = render(&p);
        assert!(svg.contains("<circle") && !svg.contains("NaN"), "{}", ex.name);
    }
    let hostile = ["", "\u{0}", ",,,", "nan, inf, -1, 1e308", "a very very long value indeed, b, c, d", ";;;, ; ;", "((((, )))", "<b>&amp;</b>, \u{202e}x"];
    for counts in hostile {
        for sets in ["1", "2", "3", "9"] {
            for arr in ARRANGEMENTS {
                for answers in [true, false] {
                    for shade in ["None", "A or B", "((", "not not not A'", counts] {
                        let mut p = with(&[("counts", counts), ("sets", sets), ("arrangement", arr), ("shade", shade), ("empty", "A blank line"), ("fill", "Dots")]);
                        p.set_b(SHOW_ANSWERS, answers);
                        for mode in ["Text", "Sets"] {
                            p.set_s("mode", mode);
                            p.set_s("a_set", counts);
                            p.set_s("u_set", counts);
                            let svg = render(&p);
                            assert!(svg.starts_with("<svg") && svg.ends_with("</svg>") && !svg.contains("NaN"), "{counts:?} {sets} {shade}");
                        }
                    }
                }
            }
        }
    }
}
