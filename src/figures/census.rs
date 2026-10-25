//! Render census (QA, ignored by default): writes the SVG of every figure in many states to
//! `$VERSHION_CENSUS_DIR`, one file per case, so two builds can be compared file by file
//! (`diff -r before after`). Cases per figure: plain defaults (what saved figures, presets and
//! examples build on), the opening state, every example, every classic setup, every option of
//! every choice and both values of every switch (over the plain defaults), and a slider nudge.
//!
//! `VERSHION_CENSUS_DIR=/tmp/x cargo test --release census_dump -- --ignored`
use super::*;

fn put(dir: &std::path::Path, name: String, svg: &str) {
    let safe: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '.' { c } else { '_' }).collect();
    std::fs::write(dir.join(format!("{safe}.svg")), svg).unwrap();
}

#[test]
#[ignore]
fn census_dump() {
    let Ok(dir) = crate::env_var("VERSHION_CENSUS_DIR") else { return };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut n = 0;
    for d in registry() {
        let defs = (d.params)();
        let base = Params::defaults(&defs);
        let mut case = |tag: String, p: &Params| {
            put(&dir, format!("{}__{tag}", d.id), &d.svg(p));
            n += 1;
        };
        case("defaults".into(), &base);
        case("start".into(), &start_params(&d));
        let over = |list: &[(&'static str, Value)]| {
            let mut p = base.clone();
            for (k, v) in list {
                p.0.insert(k.to_string(), v.clone());
            }
            p.normalize(&defs, false);
            p
        };
        for (i, e) in examples_for(d.id).iter().enumerate() {
            case(format!("ex{i}"), &over(&e.params));
        }
        for (i, e) in classic_setups(d.id).iter().enumerate() {
            case(format!("setup{i}"), &over(&e.params));
        }
        for pd in &defs {
            match &pd.kind {
                Kind::Choice(opts) => {
                    for (i, o) in opts.iter().enumerate() {
                        let mut p = base.clone();
                        p.set_s(pd.key, o);
                        case(format!("{}={i}", pd.key), &p);
                    }
                }
                Kind::Bool => {
                    let mut p = base.clone();
                    p.set_b(pd.key, !base.b(pd.key));
                    case(format!("{}=flip", pd.key), &p);
                }
                Kind::Float { min, max, .. } => {
                    let mut p = base.clone();
                    p.set_f(pd.key, min + (max - min) * 0.3);
                    case(format!("{}=0.3", pd.key), &p);
                }
                Kind::Int { min, max } => {
                    let mut p = base.clone();
                    p.set_i(pd.key, min + (max - min) / 3);
                    case(format!("{}=third", pd.key), &p);
                }
                _ => {}
            }
        }
    }
    // composite presets (stored as spec text)
    for name in math::COMPOSITE_STARTS {
        let d = registry().into_iter().find(|d| d.id == "composite-shape").unwrap();
        let mut p = Params::defaults(&(d.params)());
        p.set_s("spec", math::preset_spec(name).unwrap_or(""));
        put(&dir, format!("composite-shape__preset_{name}"), &d.svg(&p));
        n += 1;
    }
    eprintln!("census: {n} renders in {}", dir.display());
}

// ---- baseline gate ---------------------------------------------------------------------------

/// FNV-1a 64 of the SVG text (no dependency; stable across platforms and runs).
fn fnv64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// `figure-id__case` -> hash for the `defaults` and `start` cases of every figure, sorted.
fn census_hashes() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for d in registry() {
        let base = Params::defaults(&(d.params)());
        out.push((format!("{}__defaults", d.id), format!("{:016x}", fnv64(&d.svg(&base)))));
        out.push((format!("{}__start", d.id), format!("{:016x}", fnv64(&d.svg(&start_params(&d))))));
    }
    out.sort();
    out
}

fn census_fixture() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/figure_census.txt")
}

/// Diff two `name hash` tables into changed / added / removed ids.
fn census_diff(old: &[(String, String)], new: &[(String, String)]) -> Vec<String> {
    let om: std::collections::BTreeMap<&str, &str> = old.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let nm: std::collections::BTreeMap<&str, &str> = new.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let mut msgs = Vec::new();
    for (k, v) in &nm {
        match om.get(k) {
            None => msgs.push(format!("added: {k}")),
            Some(o) if o != v => msgs.push(format!("changed: {k}")),
            _ => {}
        }
    }
    for k in om.keys() {
        if !nm.contains_key(k) {
            msgs.push(format!("removed: {k}"));
        }
    }
    msgs
}

/// Every figure's plain-defaults and opening-state render must match the committed hash. A
/// deliberate rendering change is blessed with `VERSHION_CENSUS_BLESS=1 cargo test --release --lib census_baseline`
/// (rewrites tests/fixtures/figure_census.txt; review the diff of ids in the commit).
#[test]
fn census_baseline() {
    let now = census_hashes();
    let path = census_fixture();
    if crate::env_var("VERSHION_CENSUS_BLESS").is_ok_and(|v| v == "1") {
        let text: String = now.iter().map(|(k, h)| format!("{k} {h}\n")).collect();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        return;
    }
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}; run with VERSHION_CENSUS_BLESS=1 to create it", path.display()));
    let old: Vec<(String, String)> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let (a, b) = l.split_once(' ').unwrap_or_else(|| panic!("bad census line {l:?}"));
            (a.to_string(), b.trim().to_string())
        })
        .collect();
    let diff = census_diff(&old, &now);
    assert!(
        diff.is_empty(),
        "figure render census differs from tests/fixtures/figure_census.txt ({} ids):\n  {}\nIf the change is intended, regenerate with:\n  VERSHION_CENSUS_BLESS=1 cargo test --release --lib census_baseline",
        diff.len(),
        diff.join("\n  ")
    );
}

#[test]
fn census_diff_reports_changed_added_removed() {
    let a = vec![("x__defaults".to_string(), "1".to_string()), ("y__defaults".to_string(), "2".to_string())];
    let b = vec![("x__defaults".to_string(), "9".to_string()), ("z__defaults".to_string(), "3".to_string())];
    let d = census_diff(&a, &b);
    assert_eq!(d, vec!["changed: x__defaults", "added: z__defaults", "removed: y__defaults"]);
}

// ---- Print style stays grey ------------------------------------------------------------------

/// Neutral enough to print as grey: channels within `NEUTRAL_SPREAD` of each other. The figures'
/// ink colours (#111827, #6b7280, #9ca3af, #dfe3ea) are slightly blue-tinted greys; anything with
/// real chroma (the series colours) is far outside this.
const NEUTRAL_SPREAD: i32 = 24;

fn neutral(ch: &[i32]) -> bool {
    ch.iter().max().unwrap() - ch.iter().min().unwrap() <= NEUTRAL_SPREAD
}

/// Is this paint value black, white, (near-)grey, none or a pattern reference?
fn paint_is_grey(v: &str) -> bool {
    let v = v.trim().to_ascii_lowercase();
    if v.is_empty() || v.starts_with("url(") || matches!(v.as_str(), "none" | "transparent" | "currentcolor" | "inherit" | "context-fill" | "context-stroke") {
        return true;
    }
    if let Some(h) = v.strip_prefix('#') {
        let d: Vec<u32> = h.chars().filter_map(|c| c.to_digit(16)).collect();
        if d.len() != h.chars().count() {
            return false;
        }
        let ch: Vec<i32> = match d.len() {
            3 | 4 => d[..3].iter().map(|x| (*x * 17) as i32).collect(),
            6 | 8 => (0..3).map(|i| (d[2 * i] * 16 + d[2 * i + 1]) as i32).collect(),
            _ => return false,
        };
        return neutral(&ch);
    }
    if let Some(args) = v.strip_prefix("rgb(").or_else(|| v.strip_prefix("rgba(")) {
        let nums: Vec<&str> = args.trim_end_matches(')').split([',', ' ', '/']).filter(|s| !s.is_empty()).collect();
        let ch: Vec<i32> = nums.iter().take(3).filter_map(|n| n.trim_end_matches('%').parse::<f64>().ok()).map(|x| x as i32).collect();
        return ch.len() == 3 && neutral(&ch);
    }
    matches!(
        v.as_str(),
        "black" | "white" | "gray" | "grey" | "silver" | "gainsboro" | "lightgray" | "lightgrey" | "darkgray" | "darkgrey" | "dimgray" | "dimgrey" | "whitesmoke"
    )
}

/// Every colour-bearing value (fill, stroke, stop-color, flood-color; as attribute or CSS) that is
/// not grey, black, white, none or a `url(#...)` reference, deduplicated.
fn non_grey_paints(svg: &str) -> Vec<String> {
    let mut bad: Vec<String> = Vec::new();
    for prop in ["fill", "stroke", "stop-color", "flood-color"] {
        for (pat, ends) in [(format!("{prop}=\""), '"'), (format!("{prop}:"), ';')] {
            let mut rest = svg;
            while let Some(i) = rest.find(&pat) {
                // the property name must start a token (not e.g. `text-fill`)
                let before = rest[..i].chars().last();
                rest = &rest[i + pat.len()..];
                if before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '-') {
                    continue;
                }
                let end = rest.find([ends, '"', '}']).unwrap_or(rest.len());
                let val = rest[..end].trim();
                if !paint_is_grey(val) && !bad.iter().any(|b| b == val) {
                    bad.push(val.to_string());
                }
            }
        }
    }
    bad
}

/// Figures whose Print style still emits a colour today. Print is meant to survive a black-and-
/// white copy; these are known gaps recorded here (not fixed, since that would change their
/// output). The test requires the list to equal the real offenders exactly: fixing a figure means
/// removing it here, and a new offender fails until it is fixed or added deliberately.
const KNOWN_PRINT_OFFENDERS: &[&str] = &[];

#[test]
fn print_style_is_grey_only() {
    assert!(paint_is_grey("#808080") && paint_is_grey("#FFF") && paint_is_grey("rgb(9, 9, 9)") && paint_is_grey("url(#hatch0)") && paint_is_grey("black"));
    assert!(paint_is_grey("#111827") && paint_is_grey("#dfe3ea") && paint_is_grey("rgb(1,2,3)"));
    assert!(!paint_is_grey("#4f7cd4") && !paint_is_grey("red") && !paint_is_grey("rgb(1,90,3)") && !paint_is_grey("#f00") && !paint_is_grey("#e0803c"));
    let mut offenders: Vec<String> = Vec::new();
    let mut checked = 0;
    for d in registry() {
        let defs = (d.params)();
        let has_print = defs.iter().any(|x| x.key == "style" && matches!(&x.kind, Kind::Choice(o) if o.contains(&"Print")));
        if !has_print {
            continue;
        }
        checked += 1;
        let mut bad: Vec<String> = Vec::new();
        for mut p in [Params::defaults(&defs), start_params(&d)] {
            p.set_s("style", "Print");
            bad.extend(non_grey_paints(&d.svg(&p)));
        }
        if !bad.is_empty() {
            bad.sort();
            bad.dedup();
            println!("PRINT OFFENDER {}: {bad:?}", d.id);
            offenders.push(d.id.to_string());
        }
    }
    offenders.sort();
    println!("print-style figures checked: {checked}");
    assert!(checked > 0);
    let known: Vec<String> = KNOWN_PRINT_OFFENDERS.iter().map(|s| s.to_string()).collect();
    assert_eq!(offenders, known, "figures whose Print style emits colour must equal KNOWN_PRINT_OFFENDERS");
}
