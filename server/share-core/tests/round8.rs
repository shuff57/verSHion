//! Round 8: locked flags that narrow the key, marks of any kind on every wrong choice, look-alike
//! letters, Matching tags of letters or other number forms, linear Matching and HTML checks;
//! notation, French typography, units and intervals as content; letter references only to
//! choice labels; shuffled Matching lefts; more markers; the key in stems and left items; the
//! single-call refusal shape.
mod common;
use serde_json::{Value, json};
use share_core::wasm::*;

fn j(s: &str) -> Value {
    serde_json::from_str(s).unwrap()
}
fn q(stem: &str, kind: Value) -> String {
    json!({"schema_version": 2, "question": {"id":"q1","stem": stem,"kind": kind,"points":1.0,"difficulty":null,"created":0,"modified":0}}).to_string()
}
fn mc_kind(texts: &[&str], correct: &[usize], locked: &[usize]) -> Value {
    json!({"Mc": {"answers": texts.iter().enumerate().map(|(i, t)| json!({"text": t, "correct": correct.contains(&i), "locked": locked.contains(&i)})).collect::<Vec<_>>()}})
}
fn mc(texts: &[&str], correct: &[usize], locked: &[usize]) -> String {
    q("Which?", mc_kind(texts, correct, locked))
}
fn matching(pairs: &[(&str, &str)], d: &[&str]) -> String {
    q("Match", json!({"Matching": {"pairs": pairs.iter().map(|(l, r)| json!({"left": l, "right": r})).collect::<Vec<_>>(), "distractors": d}}))
}
fn pp(p: &str, o: Value) -> Value {
    let mut o = o;
    o["hide_key"] = json!(true);
    j(&publish_prepare_json(p.as_bytes(), &o.to_string()))
}
/// "code/reason" of every error, "!" when it cannot be acknowledged.
fn errs(r: &Value) -> Vec<String> {
    r["errors"].as_array().map(|e| e.iter().map(|x| format!("{}/{}{}", x["code"].as_str().unwrap(), x["reason"].as_str().unwrap_or(""), if x["ack_allowed"] == false { "!" } else { "" })).collect()).unwrap_or_default()
}
fn hard(r: &Value) -> bool {
    r["ok"] == false && r["errors"].as_array().unwrap().iter().any(|e| e["ack_allowed"] == false || (e["code"] != "key_in_formatting" && e["code"] != "order_ack_required" && !e["code"].as_str().unwrap().starts_with("labels_")))
}

#[derive(Debug, PartialEq)]
enum Out {
    Served,
    Acked,
    Refused(Vec<String>),
}

/// An honest author through `publish_prepare` alone: label decisions from the findings, order
/// acks and low-risk formatting acks from the refusal.
fn drive(p: &str) -> Out {
    let mut o = json!({"labels": [], "order_acks": [], "ack_formatting": []});
    let mut acked = false;
    for _ in 0..6 {
        let r = pp(p, o.clone());
        if r["ok"] == true {
            return if acked { Out::Acked } else { Out::Served };
        }
        let mut changed = false;
        if let Some(f) = r["label_findings"].as_array()
            && o["labels"].as_array().unwrap().is_empty()
        {
            o["labels"] = json!(f.iter().map(|x| {
                let mut d = json!({"question": x["question"], "choice": if x["strip_allowed"] == true { "strip" } else { "keep" }});
                if !x["part"].is_null() {
                    d["part"] = x["part"].clone();
                }
                d
            }).collect::<Vec<_>>());
            changed = true;
        }
        let e = r["errors"].as_array().unwrap();
        if e.iter().any(|x| x["code"] == "labels_keep_refused") {
            for l in o["labels"].as_array_mut().unwrap() {
                l["choice"] = json!("strip");
            }
            changed = true;
        }
        if let Some(n) = r["needs_order_ack"].as_array()
            && o["order_acks"].as_array().unwrap().is_empty()
        {
            o["order_acks"] = json!(n);
            changed = true;
        }
        if e.iter().any(|x| x["code"] == "key_in_formatting") && e.iter().filter(|x| x["code"] == "key_in_formatting").all(|x| x["ack_allowed"] == true) && !acked {
            o["ack_formatting"] = json!([0]);
            acked = true;
            changed = true;
        }
        if !changed {
            return Out::Refused(errs(&r));
        }
    }
    Out::Refused(vec!["loop".into()])
}

// ---- D1: locked flags that narrow the key ----------------------------------------------------------

#[test]
fn locks_that_narrow_the_key_are_refused() {
    let locked = |t: &[&str], c: &[usize], l: &[usize]| errs(&pp(&mc(t, c, l), json!({}))).contains(&"locked_key/".to_string());
    let five = ["Venus", "Mercury", "Mars", "Jupiter", "Saturn"];
    // Correct Venus + Mercury, every wrong locked: the movable choices are the key.
    assert!(locked(&five, &[0, 1], &[2, 3, 4]));
    // One correct among two movable choices (a 50/50), or among three (key + 2).
    assert!(locked(&five, &[0], &[2, 3, 4]));
    assert!(locked(&five, &[0], &[3, 4]));
    // The locked choices hold the key with at most two more.
    assert!(locked(&five, &[2], &[1, 2, 4]));
    assert!(locked(&five, &[0, 1], &[0, 1]));
    // A Multipart part too.
    let part = json!({"Multipart": {"parts": [{"stem": "a", "kind": mc_kind(&five, &[0, 1], &[2, 3, 4])}]}});
    assert!(errs(&pp(&q("Two parts", part), json!({}))).contains(&"locked_key/".to_string()));
    // Not: three or more movable wrong choices beside the key; a locked "None of the above".
    assert!(!locked(&["Venus", "Mercury", "Mars", "Jupiter", "Saturn", "Neptune"], &[0], &[4, 5]));
    assert!(!locked(&["Venus", "Mars", "Earth", "Jupiter", "None of the above"], &[0], &[4]));
    // References locked below the movable choices are references, not answers (critic18a B6).
    let r = pp(&mc(&["7", "8", "Both of the above", "None of the above"], &[0], &[2, 3]), json!({}));
    assert_eq!(r["ok"], true, "{r}");
}

// ---- D2: every wrong choice marked, each its own way --------------------------------------------

#[test]
fn every_wrong_marked_any_kind_is_refused() {
    for t in [
        ["Venus", "Mars (wrong)", "Earth \u{2717}", "Jupiter \u{D7}"],
        ["Venus", "~Mars", "Earth (no)", "J\u{336}u\u{336}p\u{336}i\u{336}t\u{336}e\u{336}r\u{336}"],
        ["Venus", "Mars 0", "Earth F", "Jupiter -"],
    ] {
        let r = pp(&mc(&t, &[0], &[]), json!({"ack_formatting": [0]}));
        assert!(hard(&r), "{t:?}: {r}");
    }
    // With a reference among the wrong choices, the outlier net's "marked at all" still sees it.
    let r = pp(&mc(&["Venus", "Mars (wrong)", "Earth \u{2717}", "None of the above"], &[0], &[]), json!({}));
    assert!(errs(&r).iter().any(|e| e == "key_in_formatting/outlier"), "{r}");
    // "*Venus (c)" is the only choice with two marks.
    assert!(hard(&pp(&mc(&["*Venus (c)", "*Mars", "Earth (c)", "Jupiter"], &[0], &[]), json!({"ack_formatting": [0]}))));
    // Unit symbols are content, not marks: "pascal (Pa)" among "newton (N)", "joule (J)".
    for k in 0..4 {
        assert_eq!(drive(&mc(&["pascal (Pa)", "newton (N)", "joule (J)", "watt (W)"], &[k], &[])), Out::Served, "{k}");
    }
}

// ---- D3: look-alike letters ------------------------------------------------------------------------

#[test]
fn look_alike_letters_are_refused() {
    // Lisu, a Roman numeral, Armenian, Cyrillic, Cherokee: high risk.
    for w in ["\u{A4D1}aris", "\u{217C}ondon", "L\u{585}ndon", "P\u{430}ris", "\u{13A1}aris"] {
        let r = pp(&mc(&[w, "Rome", "Madrid", "Berlin"], &[0], &[]), json!({"ack_formatting": [0]}));
        assert!(errs(&r).iter().any(|e| e == "key_in_formatting/mixed_script!"), "{w}: {r}");
    }
    // IPA and real orthographies' letters: low risk.
    for w in ["P\u{251}ris", "Par\u{131}s", "Bi\u{261} Ben"] {
        let r = pp(&mc(&[w, "Rome", "Madrid", "Berlin"], &[0], &[]), json!({}));
        assert!(errs(&r).iter().any(|e| e == "key_in_formatting/mixed_script"), "{w}: {r}");
    }
}

// ---- D4: Matching tags of letters or another number form ---------------------------------------

#[test]
fn matching_tags_are_folded() {
    for pairs in [
        [("France (FR)", "Paris (FR)"), ("Spain (ES)", "Madrid (ES)"), ("Italy (IT)", "Rome (IT)")],
        [("France \u{3B1}", "Paris \u{3B1}"), ("Spain \u{3B2}", "Madrid \u{3B2}"), ("Italy \u{3B3}", "Rome \u{3B3}")],
        [("France [red]", "Paris [red]"), ("Spain [blue]", "Madrid [blue]"), ("Italy [green]", "Rome [green]")],
        [("France-a", "Paris-a"), ("Spain-b", "Madrid-b"), ("Italy-c", "Rome-c")],
        [("France\u{B9}", "Paris 1"), ("Spain\u{B2}", "Madrid 2"), ("Italy\u{B3}", "Rome 3")],
    ] {
        let r = pp(&matching(&pairs, &["Berlin"]), json!({"ack_formatting": [0]}));
        assert!(errs(&r).iter().any(|e| e == "key_in_formatting/pairing_token!"), "{pairs:?}: {r}");
    }
    // Weak cues (the left's initials on the right): low risk.
    for pairs in [[("France", "Paris FR"), ("Spain", "Madrid ES"), ("Italy", "Rome IT")], [("France", "F: Paris"), ("Spain", "S: Madrid"), ("Italy", "I: Rome")]] {
        let r = pp(&matching(&pairs, &["Berlin"]), json!({}));
        assert!(!hard(&r) && errs(&r).iter().any(|e| e.ends_with("/initials")), "{pairs:?}: {r}");
    }
    // Maths: a variable in both columns is not a tag ("sin x" / "cos x", "eˣ" / "eˣ").
    assert!(!hard(&pp(&matching(&[("x\u{B2}", "2x"), ("sin x", "cos x"), ("e\u{2E3}", "e\u{2E3}"), ("ln x", "1/x")], &["x"]), json!({}))));
}

// ---- D5, D6: linear Matching and HTML checks ----------------------------------------------------

fn time<F: FnMut()>(mut f: F) -> f64 {
    (0..7).map(|_| { let t = std::time::Instant::now(); f(); t.elapsed().as_secs_f64() }).fold(f64::MAX, f64::min)
}
fn loaded() -> bool {
    std::env::var("SHARE_CORE_MUTATION").is_ok()
}

#[test]
fn matching_and_html_checks_are_linear() {
    use share_core::format;
    // names_left: every left of n words "a a ... b", every right n "a": was O(pairs² · words²).
    let left = |n: usize, k: usize| format!("{} b{k}", "a ".repeat(n));
    let right = |n: usize| "a ".repeat(2 * n);
    let run = |n: usize| {
        let ls: Vec<String> = (0..50).map(|k| left(n, k)).collect();
        let r = right(n);
        let pairs: Vec<(&str, &str)> = ls.iter().map(|l| (l.as_str(), r.as_str())).collect();
        time(|| {
            std::hint::black_box(format::matching_names_left(&pairs));
        })
    };
    let (a, b) = (run(250), run(500));
    assert!(b / a < 3.2 || b < 0.01 || loaded(), "names_left: {a:.4} -> {b:.4}");
    // "<b " repeated: whether there is text outside tags is decided once per choice.
    let html = |n: usize| format!("{}z", "<b ".repeat(n));
    // (Four times the length: linear is ~4x, the old rescan ~16x.)
    let (h1, h2) = (html(1000), html(4000));
    let (t1, t2) = (time(|| for _ in 0..10 { std::hint::black_box(format::features(&h1)); }), time(|| for _ in 0..10 { std::hint::black_box(format::features(&h2)); }));
    assert!(t2 / t1 < 8.0 || t2 < 0.005 || loaded(), "html tags: {t1:.4} -> {t2:.4}");
    // The new scans: key words in many bracket groups, a left naming its right many times,
    // answer lines, braces.
    let kw = |n: usize| format!("w {}", "(yes x)".repeat(n));
    let (k1, k2) = (kw(140), kw(280));
    let (t1, t2) = (time(|| for _ in 0..50 { std::hint::black_box(format::features(&k1)); }), time(|| for _ in 0..50 { std::hint::black_box(format::features(&k2)); }));
    assert!(t2 / t1 < 3.2 || t2 < 0.01 || loaded(), "key words: {t1:.4} -> {t2:.4}");
    let lr = |n: usize| "aaa ".repeat(n);
    let (l1, l2) = (lr(250), lr(500));
    let (t1, t2) = (time(|| for _ in 0..200 { std::hint::black_box(format::left_names_right(&l1, "aaa")); }), time(|| for _ in 0..200 { std::hint::black_box(format::left_names_right(&l2, "aaa")); }));
    assert!(t2 / t1 < 3.2 || t2 < 0.01 || loaded(), "left_names_right: {t1:.4} -> {t2:.4}");
    let key = format::KeyInfo { texts: vec!["zz".repeat(500)], choices: true, ..Default::default() };
    let st = |n: usize| format!("{}{}", "answer: zz ".repeat(n), "{x ".repeat(n));
    let (s1, s2) = (st(300), st(600));
    let (t1, t2) = (time(|| for _ in 0..50 { std::hint::black_box(format::stem_key(&s1, &key)); }), time(|| for _ in 0..50 { std::hint::black_box(format::stem_key(&s2, &key)); }));
    assert!(t2 / t1 < 3.2 || t2 < 0.01 || loaded(), "stem_key: {t1:.4} -> {t2:.4}");
}

// ---- Non-blockers: parity, invisible characters, ZWNJ, weak cues --------------------------------

#[test]
fn publish_prepare_stripped_is_strips_response() {
    let p = json!({"schema_version": 2, "bank": {"id":"b","name":"n","created":1,"modified":2,"questions":[{"id":"q1","stem":"s","kind":{"Mc":{"answers":[{"text":"Venus","correct":true,"locked":false},{"text":"Mars","correct":false,"locked":false}]}},"points":null,"difficulty":null,"created":1,"modified":1,"zzz":1}]}}).to_string();
    let a = j(&publish_prepare_json(p.as_bytes(), r#"{"hide_key":true}"#));
    let s = j(&strip_json(p.as_bytes(), ""));
    assert_eq!(a["stripped"]["dropped"], s["dropped"]);
    assert_eq!(a["stripped"]["dropped"][0]["reason"], "unknown_field");
}

#[test]
fn invisible_characters_are_high_risk() {
    for c in ['\u{200E}', '\u{61C}', '\u{2062}', '\u{180E}', '\u{600}', '\u{110BD}'] {
        let w = format!("Pa{c}ris");
        let r = pp(&mc(&[&w, "Rome", "Madrid", "Berlin"], &[0], &[]), json!({"ack_formatting": [0]}));
        assert!(hard(&r), "U+{:04X}: {r}", c as u32);
    }
    // A variation selector after a CJK character.
    let r = pp(&mc(&["\u{6771}\u{FE00}\u{4EAC}", "\u{5927}\u{962A}", "\u{4EAC}\u{90FD}", "\u{798F}\u{5CA1}"], &[0], &[]), json!({"ack_formatting": [0]}));
    assert!(hard(&r), "{r}");
    // ZWNJ after a letter that never joins (alef) changes nothing: removed from the served text.
    assert_eq!(share_core::format::served_text("\u{643}\u{62A}\u{627}\u{200C}\u{628}"), "\u{643}\u{62A}\u{627}\u{628}");
    // Between two joining letters it is Persian spelling and stays.
    assert_eq!(share_core::format::served_text("\u{645}\u{6CC}\u{200C}\u{62E}\u{648}\u{627}\u{647}\u{645}"), "\u{645}\u{6CC}\u{200C}\u{62E}\u{648}\u{627}\u{647}\u{645}");
}

// ---- E1-E5: notation, maths, French spacing, units, intervals ----------------------------------

#[test]
fn notation_and_typography_are_never_hard_refusals() {
    for (stem, t) in [
        ("CSS id selector?", ["#nav", ".nav", "nav", "*nav"]),
        ("Append stdout?", [">> out.txt", "> out.txt", "< out.txt", "2> out.txt"]),
        ("Markdown blockquote?", ["> quote", "# quote", "- quote", "1. quote"]),
        ("GIFT correct line?", ["=yellow", "~yellow", "#yellow", "%yellow"]),
        ("Checkmate?", ["Qh5#", "Qh5+", "Qh5", "Qh5!"]),
        ("Diff: added line?", ["+ new line", "- new line", "! new line", "? new line"]),
        ("d/dx ln x?", ["$\\frac{1}{x}$", "$x$", "$\\ln x$", "$e^x$"]),
        ("Sample mean?", ["$\\bar{x}$", "$\\mu$", "$\\sigma$", "$s^2$"]),
        ("dy/dx = 1?", ["y = x + C", "y = Ce^x", "y = Cx", "y = 1"]),
        ("Impératif ?", ["Viens ici !", "Tu viens ici.", "Il vient ici.", "Venir ici"]),
        ("Current?", ["ampere (A)", "ohm", "watt", "volt"]),
        ("x² > 4?", ["(\u{2212}\u{221E}, \u{2212}2) \u{222A} (2, \u{221E})", "(\u{2212}2, 2)", "[\u{2212}2, 2]", "(0, 2)"]),
    ] {
        let r = pp(&q(stem, mc_kind(&t, &[0], &[])), json!({"ack_formatting": [0], "order_acks": [{"question": 0}], "labels": []}));
        assert!(!hard(&r), "{t:?}: {r}");
        // The author is told what to do.
        if let Some(e) = r["errors"].as_array().and_then(|e| e.iter().find(|x| x["code"] == "key_in_formatting")) {
            assert!(e["hint"].as_str().unwrap().contains("ack_formatting"), "{e}");
        }
    }
    // Still refused: one shared GIFT mark on every wrong choice; LaTeX boxes.
    assert!(hard(&pp(&mc(&["=Paris", "~Rome", "~Madrid", "~Berlin"], &[0], &[]), json!({"ack_formatting": [0]}))));
    assert!(hard(&pp(&mc(&["$\\boxed{\\text{Paris}}$", "$\\text{Rome}$", "$\\text{Madrid}$"], &[0], &[]), json!({"ack_formatting": [0]}))));
}

// ---- E6: letters name choices only as their labels ---------------------------------------------

#[test]
fn letters_name_only_choice_labels() {
    use share_core::labels::refers_to_siblings;
    let r = |t: &[&str]| refers_to_siblings(&t.iter().map(|x| (*x, false)).collect::<Vec<_>>());
    // Content (critic18a): notes, bases, vitamins, grades, elements, initials.
    for t in [
        ["C, E, G", "C, E\u{266D}, G", "C, F, A", "D, F, A"],
        ["A and G", "C and T", "A and T", "C and G"],
        ["vitamins A, D, E and K", "vitamins B and C", "vitamin B\u{2081}\u{2082} only", "vitamins C and K"],
        ["A, B and C", "D and E", "F only", "E and F"],
        ["F, Cl, Br, I", "Na, K, Li", "He, Ne, Ar", "O, S, Se"],
        ["George Orwell", "H. G. Wells", "T. S. Eliot", "E. M. Forster"],
        ["(a) and (c)", "(b) only", "(a), (b) and (d)", "(c) and (d)"],
        ["I and II only", "II and III only", "I only", "I, II and III"],
    ] {
        assert!(!r(&t), "{t:?}");
    }
    // Lowercase letters among unlabelled word choices name the stem's items.
    assert!(!r(&["Mitochondria", "Ribosome", "Nucleus", "a and c"]));
    assert!(r(&["a) Mitochondria", "b) Ribosome", "c) Nucleus", "a and c"]));
    // References: capitals in range and not the choice's own; labels the choices carry.
    assert!(r(&["Mitochondria", "Ribosome", "Nucleus", "Both A and B"]));
    assert!(r(&["Mitochondria", "Ribosome", "Nucleus", "A and C"]));
    assert!(r(&["a) Mitochondria", "b) Ribosome", "c) Both (a) and (b)", "d) Nucleus"]));
    assert!(r(&["Mitochondria", "Ribosome", "Nucleus", "Both of them"]));
    // The ordering question names every choice including its own: content.
    assert!(!r(&["B, A, C, D", "A, B, C, D", "D, C, B, A", "C, A, B, D"]));
}

// ---- E7: Matching left items are shuffled --------------------------------------------------------

#[test]
fn matching_lefts_are_shuffled_and_reported() {
    let pairs = [("Prophase", "Step 1"), ("Metaphase", "Step 2"), ("Anaphase", "Step 3"), ("Telophase", "Step 4")];
    let p = matching(&pairs, &[]);
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..20 {
        let r = pp(&p, json!({}));
        assert_eq!(r["ok"], true, "{r}");
        let lefts: Vec<String> = r["stripped"]["payload"]["question"]["kind"]["Matching"]["pairs"].as_array().unwrap().iter().map(|x| x["left"].as_str().unwrap().to_string()).collect();
        let a = j(&strip_author_data_json());
        let o = &a["served_orders"][0];
        assert_eq!(o["matching"], true);
        let lo: Vec<usize> = o["left_order"].as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as usize).collect();
        assert_eq!(lefts, lo.iter().map(|&k| pairs[k].0.to_string()).collect::<Vec<_>>());
        seen.insert(lefts);
    }
    assert!(seen.len() > 1, "the left column moves");
    // keep_order keeps it.
    let mut v = j(&p);
    v["question"]["keep_order"] = json!(true);
    let r = pp(&v.to_string(), json!({"order_acks": [{"question": 0}]}));
    let lefts: Vec<&str> = r["stripped"]["payload"]["question"]["kind"]["Matching"]["pairs"].as_array().unwrap().iter().map(|x| x["left"].as_str().unwrap()).collect();
    assert_eq!(lefts, ["Prophase", "Metaphase", "Anaphase", "Telophase"]);
    let _ = strip_author_data_json();
}

// ---- E8: more markers ------------------------------------------------------------------------------

#[test]
fn more_markers_are_refused() {
    for (t, c) in [
        (vec!["~%50%yellow", "~%-100%red", "~%50%off-beige", "~%-100%blue"], vec![0, 2]),
        (vec!["\u{5DF4}\u{9ECE}\u{FF08}\u{221A}\u{FF09}", "\u{7F57}\u{9A6C}\u{FF08}\u{D7}\u{FF09}", "\u{9A6C}\u{5FB7}\u{91CC}\u{FF08}\u{D7}\u{FF09}", "\u{67CF}\u{6797}\u{FF08}\u{D7}\u{FF09}"], vec![0]),
        (vec!["\u{5DF4}\u{9ECE}\u{3010}\u{7B54}\u{6848}\u{3011}", "\u{7F57}\u{9A6C}", "\u{9A6C}\u{5FB7}\u{91CC}", "\u{67CF}\u{6797}"], vec![0]),
        (vec!["\u{30D1}\u{30EA} \u{2190}\u{7B54}\u{3048}", "\u{30ED}\u{30FC}\u{30DE}", "\u{30DE}\u{30C9}\u{30EA}\u{30FC}\u{30C9}", "\u{30D9}\u{30EB}\u{30EA}\u{30F3}"], vec![0]),
        (vec!["Paris (solution)", "Rome", "Madrid"], vec![0]),
        (vec!["Paris", "Rome &#10007;", "Madrid &#10007;"], vec![0]),
        (vec!["yellow # well done", "red # try again", "blue # try again"], vec![0]),
        (vec!["$\\boxed{\\text{Paris}}$", "$\\text{Rome}$", "$\\text{Madrid}$"], vec![0]),
        (vec!["Paris (sahihi)", "Roma", "Madrid"], vec![0]),
        (vec!["Paris (dobrze)", "Roma", "Madrid"], vec![0]),
    ] {
        let r = pp(&mc(&t, &c, &[]), json!({"ack_formatting": [0]}));
        assert!(hard(&r), "{t:?}: {r}");
    }
    // Ambiguous: quotes or a footnote mark on the key only are low risk (refused, ackable).
    for t in [["\u{201C}Paris\u{201D}", "Rome", "Madrid"], ["Paris\u{B9}", "Rome", "Madrid"], ["Paris (capitale)", "Rome", "Madrid"]] {
        let r = pp(&mc(&t, &[0], &[]), json!({}));
        assert!(r["ok"] == false && !hard(&r), "{t:?}: {r}");
    }
}

// ---- E9: the key in stems, part stems and left items --------------------------------------------

#[test]
fn key_in_stems_is_refused() {
    let tf = |stem: &str, v: bool| q(stem, json!({"Tf": {"correct": v}}));
    for (p, reason) in [
        (q("The Moodle project was started by:\nANSWER: B", mc_kind(&["Petr Skoda", "Martin Dougiamas", "Eloy Lafuente", "Tim Hunt"], &[1], &[])), "answer_line"),
        (q("What's between orange and green? {=yellow ~red ~blue}", mc_kind(&["yellow", "red", "blue"], &[0], &[])), "gift"),
        (tf("42 is the Absolute Answer to everything.{FALSE}", false), "gift"),
        (tf("The Sun is a star.{T}", true), "gift"),
        (tf("The Sun is a star. (True)", true), "tf_value"),
        (q("The capital of France is {1:SHORTANSWER:%100%Paris#Congratulations!~%50%Marseille#No}.", json!({"Short": {"sample": "Paris"}})), "cloze"),
        (q("Capital of France? Answer: Paris", json!({"Short": {"sample": "Paris"}})), "answer_line"),
        (q("Ball thrown up.", json!({"Multipart": {"parts": [{"stem": "Its speed at the top is zero. (True)", "kind": {"Tf": {"correct": true}}}]}})), "tf_value"),
        (matching(&[("France (Paris)", "Paris"), ("Italy (Rome)", "Rome")], &["Oslo"]), "left_names_right"),
        (matching(&[("France: Paris", "Paris"), ("Italy", "Rome")], &["Oslo"]), "left_names_right"),
    ] {
        let r = pp(&p, json!({}));
        assert!(errs(&r).contains(&format!("key_in_stem/{reason}!")), "{p}: {r}");
    }
    // Not: LaTeX braces, a stem that asks for an answer, a definition using the right's word.
    for p in [
        q("Compute $A^{T}$ and $\\mathbf{F}$.", mc_kind(&["1", "2", "3"], &[0], &[])),
        q("Write your answer: in a full sentence.", json!({"Short": {"sample": "It rains."}})),
        q("Solve: x + 1 = 2. Answer: ____", json!({"Short": {"sample": "1"}})),
        matching(&[("A teacher asks a question and specifies a choice of multiple responses.", "Choice"), ("An activity supporting asynchronous discussions.", "Forum")], &["Chat"]),
    ] {
        let r = pp(&p, json!({}));
        assert_eq!(r["ok"], true, "{p}: {r}");
    }
}

// ---- The refusal shape ------------------------------------------------------------------------------

#[test]
fn refusals_carry_findings_and_the_order_preview() {
    let r = pp(&mc(&["Venus \u{2713}", "Mars", "Earth"], &[0], &[]), json!({}));
    assert_eq!(r["ok"], false);
    let f = &r["formatting_findings"][0];
    assert_eq!(f["code"], "key_in_formatting");
    assert_eq!(f["served"], "Venus \u{2713}");
    assert_eq!(f["path"], "question.kind.Mc.answers[0]");
    assert!(r["formatting_findings_total"].as_u64().unwrap() >= 1);
    assert_eq!(r["order_preview"][0]["movable"], json!([0, 1, 2]));
    assert!(r["errors"][0]["hint"].is_string());
    // A refusal that is not about formatting has empty findings; a label refusal carries them.
    let r = pp(&mc(&["A. Venus", "B. Mars", "C. Earth"], &[0], &[]), json!({}));
    assert_eq!(r["errors"][0]["code"], "labels_need_decision");
    assert_eq!(r["label_findings"][0]["question"], 0);
    assert_eq!(r["formatting_findings"], json!([]));
    assert!(r["order_preview"].is_array());
    // A payload that does not parse: errors only.
    let r = j(&publish_prepare_json(b"{", r#"{"hide_key":true}"#));
    assert!(r["formatting_findings"].is_null() && r["ok"] == false);
}

// ---- The critic18a corpora --------------------------------------------------------------------------

/// 485 honest general questions (about 50 languages) and 66 code and notation questions: no hard
/// refusal; formatting acknowledgements on at most 2% of the general ones.
#[test]
fn critic18a_corpora() {
    let items: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(common::fixtures_dir().join("../tests/data/critic18a_corpus.json")).unwrap()).unwrap();
    let (mut refused, mut fmt_acks, mut general) = (vec![], 0, 0);
    for (i, it) in items.iter().enumerate() {
        let mut qv = json!({"id": format!("q{i:010}"), "stem": "", "points": 1.0, "difficulty": null, "topics": [], "image": null, "explanation": null, "figure": null, "created": 1760000000000i64, "modified": 1760000000000i64});
        for (k, v) in it["q"].as_object().unwrap() {
            qv[k] = v.clone();
        }
        let p = json!({"schema_version": 2, "question": qv}).to_string();
        let is_general = it["set"] == "general";
        general += usize::from(is_general);
        // Formatting acks only (label decisions and order acks are not needless).
        let plain = drive(&p);
        if let Out::Refused(e) = &plain {
            refused.push(format!("{} {e:?}", it["id"]));
        }
        if is_general && plain == Out::Acked {
            fmt_acks += 1;
        }
    }
    assert!(refused.is_empty(), "{}", refused.join("\n"));
    assert!(fmt_acks * 50 <= general, "{fmt_acks} of {general} needed a formatting acknowledgement");
}

// ---- Needless acknowledgements (critic18a) ----------------------------------------------------------

#[test]
fn honest_content_needs_no_acknowledgement() {
    for (t, k) in [
        (vec!["Martin Luther King Jr.", "Malcolm X", "Rosa Parks", "John Lewis"], 0),
        (vec!["Dr.", "doctor", "doctoral", "doctors"], 0),
        (vec!["x\u{304}", "\u{3BC}", "\u{3C3}", "\u{3C1}"], 0),
        (vec!["CH\u{2083}COOH \u{21CC} CH\u{2083}COO\u{207B} + H\u{207A}", "CH\u{2083}COOH \u{2192} CH\u{2083}COO\u{207B} + H\u{207A}", "CH\u{2083}COOH \u{2190} CH\u{2083}COO\u{207B}", "CH\u{2083}COOH = CH\u{2083}COO\u{207B} + H\u{207A}"], 0),
        (vec!["(x \u{2212} 2)(x + 3)", "x\u{B2} \u{2212} 6", "x(x + 1) \u{2212} 6", "x\u{B2} + x \u{2212} 6"], 0),
        (vec!["6.022 \u{D7} 10\u{B2}\u{B3} mol\u{207B}\u{B9}", "1.602 \u{D7} 10\u{207B}\u{B9}\u{2079} C", "3.00 \u{D7} 10\u{2078} m/s", "9.81 m/s\u{B2}"], 0),
        (vec!["Viens ici !", "Tu viens ici.", "Il vient ici.", "Venir ici"], 0),
        (vec!["(\u{2212}\u{221E}, \u{2212}2) \u{222A} (2, \u{221E})", "(\u{2212}2, 2)", "[\u{2212}2, 2]", "(0, 2)"], 0),
        (vec!["y = x + C", "y = Ce^x", "y = Cx", "y = 1"], 0),
        (vec!["$\\frac{1}{x}$", "$x$", "$\\ln x$", "$e^x$"], 0),
        (vec!["pascal (Pa)", "newton (N)", "joule (J)", "watt (W)"], 0),
        (vec!["kelvin (K)", "degree Celsius (\u{B0}C)", "degree Fahrenheit (\u{B0}F)", "rankine (\u{B0}R)"], 0),
        (vec!["\u{F63}\u{FB7}\u{F0B}\u{F66}\u{F0D}", "\u{F49}\u{F72}\u{F44}\u{F0B}\u{F41}\u{FB2}\u{F72}\u{F0D}", "\u{F42}\u{F5E}\u{F72}\u{F66}\u{F0B}\u{F40}\u{F0B}\u{F62}\u{F99}\u{F7A}\u{F0D}", "\u{F62}\u{F99}\u{F7A}\u{F0B}\u{F50}\u{F44}\u{F0B}\u{F0D}"], 0),
        (vec!["O(n)", "O(1)", "O(n^2)", "O(n log n)"], 0),
        (vec!["(aq)", "(s)", "(l)", "(g)"], 0),
        // A short word inside a longer bracket is content ("no charge"), not "(no)".
    ] {
        assert_eq!(drive(&mc(&t, &[k], &[])), Out::Served, "{t:?}");
    }
    assert_eq!(drive(&matching(&[("x\u{B2} - 4", "(x - 2)(x + 2)"), ("x\u{B2} - 9", "(x - 3)(x + 3)"), ("x\u{B2} + 2x + 1", "(x + 1)\u{B2}")], &["(x - 4)(x + 4)"])), Out::Served);
}
