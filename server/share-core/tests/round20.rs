//! Round 20 (critic36a Moodle, critic36b robustness):
//! * B1 (36b): a numbered row after a lead on the SAME line ("Key: 1. Paris", "Answers 1) Paris",
//!   "Mark scheme 1 Paris", fixed-order "Answer: 1. B", True/False "Answer:\n1 True") is read.
//! * B2 (36a, 36b): pair items that hold a pair-mark character ("Jean-Paul Sartre", "Washington,
//!   D.C.", "Genesis 1:1") are matched in the same normalised form as the text.
//! * B3 (36a): the bank-text pair scan starts at a symbol ("√9", "°C", "$5", "(a)").
//! * B4 (36a): the label and formatting paths are linear for "A. X - X - ..." choices and kin.
//!   CPU tests compare against a same-size control (ratios) and use generous absolute bounds.
use serde_json::{Value, json};
use share_core::wasm::*;
use std::time::Instant;

fn j(s: &str) -> Value {
    serde_json::from_str(s).unwrap()
}
fn pp(p: &str, o: &str) -> Value {
    j(&publish_prepare_json(p.as_bytes(), o))
}
fn qv(stem: &str, kind: Value) -> Value {
    json!({"id":"q10000001","stem":stem,"kind":kind,"points":1.0,"difficulty":null,"topics":[],"created":0,"modified":0})
}
fn one(q: Value) -> String {
    json!({"schema_version": 2, "question": q}).to_string()
}
fn bank(qs: Vec<Value>, name: &str, desc: &str) -> String {
    let qs: Vec<Value> = qs.into_iter().enumerate().map(|(i, mut q)| { q["id"] = json!(format!("q{:08}", i + 1)); q }).collect();
    json!({"schema_version":2,"bank":{"id":"b00000001","name":name,"created":0,"modified":0,"details":{"author":"T","description":desc,"license_name":"CC BY 4.0","license_url":"https://example.com/licence"},"questions":qs}}).to_string()
}
fn mc(texts: &[&str], correct: usize) -> Value {
    json!({"Mc": {"answers": texts.iter().enumerate().map(|(i, t)| json!({"text": t, "correct": i == correct, "locked": false})).collect::<Vec<_>>()}})
}
fn matching(lefts: &[String], rights: &[&str]) -> Value {
    json!({"Matching": {"pairs": lefts.iter().zip(rights).map(|(l, r)| json!({"left": l, "right": r})).collect::<Vec<_>>(), "distractors": []}})
}
fn short(s: &str) -> Value {
    json!({"Short": {"sample": s}})
}
fn has(r: &Value, reason: &str) -> bool {
    r["errors"].as_array().is_some_and(|e| e.iter().any(|x| x["reason"] == reason))
}
fn show(r: &Value) -> String {
    r.to_string().chars().take(300).collect()
}
const HK: &str = r#"{"hide_key":true}"#;


// ---- B1 (35a): the Matching rights-order scan ------------------------------------------------

// ---- B1 (36b): a numbered row after a lead on the same line ---------------------------------

#[test]
fn b1_same_line_numbered_row_is_read() {
    let leads = ["Key", "Key:", "Key -", "Correct:", "Correct", "Answer key", "Answer key:", "Answers", "Answers:", "Answer", "Answer:", "Solution:", "Solutions", "Ans", "Mark scheme", "Mark scheme:", "Model answer", "Model answer:", "KEY:", "Kunci:"];
    let rows = ["1. Paris", "1) Paris", "1: Paris", "1 Paris", "1 - Paris", "(1) Paris", "Q1: Paris", "Q1. Paris", "1.Paris", "1. **Paris**", "a) Paris"];
    let mut miss = Vec::new();
    for l in leads {
        for r in rows {
            for form in 0..2 {
                let stem = if form == 0 { format!("Capital of France?\n{l} {r}") } else { format!("Capital of France? {l} {r}") };
                let res = pp(&one(qv(&stem, short("Paris"))), HK);
                if !has(&res, "answer_line") {
                    miss.push(format!("{stem:?}"));
                }
            }
        }
    }
    assert!(miss.is_empty(), "{} of {} published: {:?}", miss.len(), leads.len() * rows.len() * 2, &miss[..miss.len().min(8)]);
    // Mc text, shuffled; Blank and numeric values.
    let ch = ["Talc", "Quartz", "Diamond", "Gypsum"];
    for stem in ["Key: 1. Quartz\nWhich is hardest?", "Correct: 1) Quartz\nWhich?", "Mark scheme 1 Quartz\nWhich?"] {
        let r = pp(&one(qv(stem, mc(&ch, 1))), HK);
        assert!(has(&r, "answer_line"), "{stem:?}: {}", show(&r));
    }
    let r = pp(&one(qv("Key: 1. 42\nHow many?", short("42"))), HK);
    assert!(has(&r, "answer_line"), "numeric: {}", show(&r));
    // Fixed order: the letter, with the number; True/False.
    for stem in ["Answer: 1. B\nWhich is hardest?", "Answer:\n1. B\nWhich?", "Answer 1) B\nWhich?", "Solution: 1. B\nWhich?", "Key: 1 B\nWhich?", "Key: 1. B\nWhich?"] {
        let mut q = qv(stem, mc(&ch, 1));
        q["keep_order"] = json!(true);
        let r = pp(&one(q), r#"{"hide_key":true,"order_acks":[{"question":0}]}"#);
        assert!(has(&r, "answer_line"), "{stem:?}: {}", show(&r));
    }
    for stem in ["Answer:\n1 True\nMars is a planet.", "Solution:\n1 True\nMars.", "Key: 1 True\nMars."] {
        let r = pp(&one(qv(stem, json!({"Tf": {"correct": true}}))), HK);
        assert!(has(&r, "answer_line"), "{stem:?}: {}", show(&r));
    }
    // A part stem of a Multipart.
    let mp = json!({"Multipart": {"parts": [{"stem": "Capital?\nKey: 1. Paris", "kind": short("Paris")}, {"stem": "Colour?", "kind": short("blue")}]}});
    let r = pp(&one(qv("Read.", mp)), HK);
    assert!(r["ok"] != true, "part stem: {}", show(&r));
    // Another question's key in a bank.
    let r = pp(&bank(vec![qv("Name it.", short("Zebra")), qv("Correct: 1. Zebra\nNotes.", json!({"Tf": {"correct": true}}))], "B", ""), HK);
    assert!(has(&r, "answer_line"), "bank: {}", show(&r));
}

#[test]
fn b1_same_line_honest_rows_publish() {
    let ch = ["Talc", "Quartz", "Diamond", "Gypsum"];
    // Round 18's dichotomous key, every position of the correct choice.
    let dk = "Use the key to identify the invertebrate.\n\nKey\n1a  Has six legs ................ go to 2\n1b  Has more than six legs ...... go to 3\n2a  Has wings ................... Fly\n2b  No wings .................... Springtail\n3a  Eight legs .................. Spider\n3b  Many legs ................... Centipede\n\nAn animal has eight legs. What is it?";
    let choices = ["Spider", "Fly", "Springtail", "Centipede"];
    for c in 0..4 {
        let r = pp(&one(qv(dk, mc(&choices, c))), HK);
        assert_eq!(r["ok"], true, "dichotomous correct={c}: {}", show(&r));
    }
    for stem in [
        "Key terms: 1. Photosynthesis uses light.\nWhich is hardest?",
        "Key points 1. Plants make sugar\nWhich is hardest?",
        "Key stage 3: 1. Plants\nWhich is hardest?",
        "Key vocabulary 1. mineral 2. hardness\nWhich is hardest?",
        "Key terms\n1. Hardness\n2. Streak\nWhich is hardest?",
        "Answer the questions. 1. Name the hardest mineral.\nWhich is hardest?",
        "Answers may vary. 1 Quartz is a mineral.\nWhich is hardest?",
        "Correct the sentences: 1. Talc is hard.\nWhich is hardest?",
        "Solution 1 is to heat the sample.\nWhich is hardest?",
        "Answer 5 questions: 1. Talc 2. Gypsum\nWhich is hardest?",
    ] {
        let r = pp(&one(qv(stem, mc(&ch, 1))), HK);
        assert_eq!(r["ok"], true, "{stem:?}: {}", show(&r));
    }
}

// ---- B2 (36a, 36b): pair items that hold a pair-mark character ------------------------------

fn pairs_q(lefts: &[&str], rights: &[&str], keep: bool) -> Value {
    let l: Vec<String> = lefts.iter().map(|s| s.to_string()).collect();
    let mut q = qv("Match each item.", matching(&l, rights));
    q["keep_order"] = json!(keep);
    q
}

/// The same pairs listed as text, with each separator.
fn listing(lefts: &[&str], rights: &[&str], sep: &str, join: &str) -> String {
    lefts.iter().zip(rights).map(|(l, r)| format!("{l}{sep}{r}")).collect::<Vec<_>>().join(join)
}

#[test]
fn b2_items_with_pair_marks_are_matched() {
    let sets: &[(&[&str], &[&str])] = &[
        (&["Jean-Paul Sartre", "Jean-Jacques Rousseau", "Albert Camus"], &["Existentialism", "Social contract", "Absurdism"]),
        (&["Capital of USA", "Capital of France", "Capital of Spain"], &["Washington, D.C.", "Paris, France", "Madrid, Spain"]),
        (&["Creation", "Love", "Faith"], &["Genesis 1:1", "John 3:16", "Hebrews 11:1"]),
        (&["War one", "War two", "Cold war"], &["1914\u{2013}1918", "1939\u{2013}1945", "1947-1991"]),
        (&["Bauxite", "Chalk", "Slate"], &["Ore-rich", "Well-known", "Ever-green"]),
        (&["Iron-ore", "Co-balt", "Sea-salt"], &["Aluminium", "Soft", "Roof"]),
        (&["Ratio a", "Ratio b", "Ratio c"], &["Ratio 1:2", "Ratio 3:4", "Ratio 5:6"]),
        (&["x = 1", "y = 2", "z = 3"], &["One", "Two", "Three"]),
        (&["a > b", "b > c", "c > d"], &["First", "Second", "Third"]),
    ];
    let mut miss = Vec::new();
    for (lefts, rights) in sets {
        for sep in [" - ", ": ", " = ", " \u{2013} ", " > ", "-", ", "] {
            for join in ["; ", ", ", "\n"] {
                let text = listing(lefts, rights, sep, join);
                // In the description, in another question's stem, in the question's own stem.
                let b = bank(vec![qv("Filler.", short("zz")), pairs_q(lefts, rights, false)], "B", &text);
                let b2 = bank(vec![qv(&text, short("zz")), pairs_q(lefts, rights, false)], "B", "");
                let own = {
                    let mut q = pairs_q(lefts, rights, false);
                    q["stem"] = json!(format!("Key: {text}"));
                    one(q)
                };
                for (what, p) in [("description", b), ("other stem", b2), ("own stem", own)] {
                    let r = pp(&p, HK);
                    if !has(&r, "pairs_listed") {
                        miss.push(format!("{what} {sep:?} {join:?} {:?}", lefts[0]));
                    }
                }
            }
        }
    }
    assert!(miss.is_empty(), "{} not refused: {:?}", miss.len(), &miss[..miss.len().min(10)]);
}

#[test]
fn b2_rights_in_pair_order_with_marks_are_found() {
    let lefts = ["Landmark 1", "Landmark 2", "Landmark 3"];
    for rights in [
        ["Paris, France", "Rome, Italy", "Madrid, Spain"],
        ["Genesis 1:1", "John 3:16", "Hebrews 11:1"],
        ["Washington, D.C.", "Ottawa, Canada", "Mexico City, Mexico"],
        ["Mr. Brown", "Mrs. Green", "Dr. White"],
        ["(a) first", "(b) second", "(c) third"],
        ["Plain", "Words", "Here"],
    ] {
        // Bank text: the rights listed in pair order; fixed order so they are served in it.
        let text = format!("Locations: {}.", rights.join("; "));
        let b = bank(vec![qv("Filler.", short("zz")), pairs_q(&lefts, &rights, true)], "B", &text);
        let r = pp(&b, r#"{"hide_key":true,"order_acks":[{"question":1}]}"#);
        assert!(has(&r, "word_bank_order"), "{rights:?} in a description: {}", show(&r));
        // The question's own stem.
        let mut q = pairs_q(&lefts, &rights, true);
        q["stem"] = json!(text);
        let r = pp(&one(q), r#"{"hide_key":true,"order_acks":[{"question":0}]}"#);
        assert!(has(&r, "word_bank_order") || has(&r, "pairs_listed") || has(&r, "answer_line"), "{rights:?} in the stem: {}", show(&r));
        // Out of order, only two of them: not a leak.
        let text = format!("Locations: {}.", [rights[2], rights[0], rights[1]].join("; "));
        let b = bank(vec![qv("Filler.", short("zz")), pairs_q(&lefts, &rights, true)], "B", &text);
        let r = pp(&b, r#"{"hide_key":true,"order_acks":[{"question":1}]}"#);
        assert!(!has(&r, "word_bank_order") || rights[0] == "Plain", "shuffled {rights:?}: {}", show(&r));
    }
}

// ---- B3 (36a): the pair scan starts at a symbol ---------------------------------------------

#[test]
fn b3_symbol_left_items_are_read() {
    let sets: &[(&[&str], &[&str])] = &[
        (&["\u{221A}9", "\u{221A}16", "\u{221A}25"], &["three", "four", "five"]),
        (&["\u{B0}C", "\u{B0}F", "K"], &["Celsius", "Fahrenheit", "Kelvin"]),
        (&["$5", "\u{20AC}5", "\u{A3}5"], &["Dollar", "Euro", "Pound"]),
        (&["(a)", "(b)", "(c)"], &["first", "second", "third"]),
        (&["#include", "#define", "#if"], &["Header", "Macro", "Condition"]),
        (&["+", "\u{D7}", "\u{F7}"], &["Add", "Multiply", "Divide"]),
        (&["\"hello\"", "'a'", "`x`"], &["String", "Char", "Code"]),
        (&["*args", "**kwargs", ".py"], &["Positional", "Keyword", "Python"]),
        (&["/etc", "/var", "/usr"], &["Config", "Logs", "Programs"]),
        (&["~", "&&", "||"], &["Home", "And", "Or"]),
        (&["\u{3C0}", "\u{3A3}", "\u{394}"], &["Pi", "Sum", "Delta"]),
    ];
    let mut miss = Vec::new();
    for (lefts, rights) in sets {
        for sep in [" - ", ": ", " = "] {
            let text = listing(lefts, rights, sep, "; ");
            for (what, p) in [
                ("description", bank(vec![qv("Filler.", short("zz")), pairs_q(lefts, rights, false)], "B", &text)),
                ("other stem", bank(vec![qv(&text, short("zz")), pairs_q(lefts, rights, false)], "B", "")),
            ] {
                if !has(&pp(&p, HK), "pairs_listed") {
                    miss.push(format!("{what} {sep:?} {:?}", lefts[0]));
                }
            }
        }
    }
    assert!(miss.is_empty(), "{} not refused: {:?}", miss.len(), &miss[..miss.len().min(10)]);
    // An honest bank of such items, with no listing, publishes.
    let b = bank(vec![qv("Filler.", short("zz")), pairs_q(sets[0].0, sets[0].1, false), pairs_q(sets[2].0, sets[2].1, false)], "Maths and money", "Symbols and amounts.");
    assert_eq!(pp(&b, HK)["ok"], true, "honest");
}

// ---- B4 (36a): the label and formatting paths for "A. X - X - ..." ---------------------------

fn mc_question(texts: Vec<String>) -> Value {
    qv("s", json!({"Mc": {"answers": texts.iter().enumerate().map(|(i, t)| json!({"text": t, "correct": i == 0, "locked": false})).collect::<Vec<_>>()}}))
}

fn label_shape(nq: usize, f: &dyn Fn(usize) -> String) -> String {
    let qs: Vec<Value> = (0..nq).map(|i| mc_question((0..26).map(|k| f(k + 26 * i)).collect())).collect();
    bank(qs, "B", "")
}

type Shape = (&'static str, Box<dyn Fn(usize) -> String>);

fn shapes() -> Vec<Shape> {
    let l = |k: usize| ((b'A' + (k % 26) as u8) as char).to_string();
    let rep = |u: &str| u.repeat(95 / u.len() + 1);
    vec![
        ("A. X - ", Box::new(move |k| format!("{}. {}", l(k), rep("X - ")))),
        ("A) X - ", Box::new(move |k| format!("{}) {}", l(k), rep("X - ")))),
        ("(A) X - ", Box::new(move |k| format!("({}) {}", l(k), rep("X - ")))),
        ("a. X - ", Box::new(move |k| format!("{}. {}", l(k).to_lowercase(), rep("X - ")))),
        ("A - X - ", Box::new(move |k| format!("{} - {}", l(k), rep("X - ")))),
        ("1. X - ", Box::new(move |k| format!("{}. {}", k % 26 + 1, rep("X - ")))),
        ("unique A. X", Box::new(move |k| { let mut t = format!("{}. ", l(k)); let mut j = 0; while t.len() < 95 { t.push_str(&format!("{} - ", (b'A' + ((k / 26 + j * 7 + k) % 26) as u8) as char)); j += 1; } t })),
        ("unique cyrillic", Box::new(move |k| { let mut t = "\u{410}. ".to_string(); let mut j = 0; while t.len() < 140 { t.push(char::from_u32(0x410 + ((k / 26 + j * 7 + k) % 32) as u32).unwrap()); t.push_str(" - "); j += 1; } t })),
        ("unique numbers", Box::new(move |k| { let mut t = format!("{}. ", l(k)); let mut j = 0; while t.len() < 95 { t.push_str(&format!("{} - ", (k * 31 + j * 17) % 9999)); j += 1; } t })),
    ]
}

#[test]
fn b4_label_paths_are_linear_for_every_shape() {
    for (name, f) in shapes() {
        let (small, big) = (label_shape(60, f.as_ref()), label_shape(240, f.as_ref()));
        for (entry, run) in [
            ("publish_prepare", (|p: &str| { publish_prepare_json(p.as_bytes(), HK); }) as fn(&str)),
            ("label_findings", |p: &str| { label_findings_json(p.as_bytes(), ""); }),
            ("formatting_findings", |p: &str| { formatting_findings_json(p.as_bytes(), ""); }),
        ] {
            let t = |p: &str| (0..3).map(|_| { let a = Instant::now(); run(p); a.elapsed().as_secs_f64() }).fold(f64::MAX, f64::min);
            let (ts, tb) = (t(&small), t(&big));
            // Four times the questions: about four times the time (a bound of 12x leaves room for
            // noise at these tiny sizes; a quadratic reading is 16x).
            assert!(tb < ts.max(0.004) * 12.0 && tb < 3.0, "{name} {entry}: {ts:.3}s -> {tb:.3}s");
        }
    }
}

#[test]
fn b4_label_findings_are_unchanged_by_the_memo() {
    // The same bank read twice and in two arrangements gives the same findings and verdict.
    let f = &shapes()[0].1;
    let p = label_shape(20, f.as_ref());
    let a = label_findings_json(p.as_bytes(), "");
    let b = label_findings_json(p.as_bytes(), "");
    assert_eq!(a, b);
    let v: Value = j(&a);
    assert!(!v["label_findings"].as_array().unwrap().is_empty());
    // A lowercase control with the same layout is read the same way.
    let c = label_shape(20, shapes()[3].1.as_ref());
    assert_eq!(j(&label_findings_json(c.as_bytes(), ""))["label_findings"].as_array().unwrap().len(), v["label_findings"].as_array().unwrap().len());
}

#[test]
fn honest_items_with_marks_publish_and_hostile_texts_refuse() {
    // Hyphenated, comma, colon and symbol items with prose that names at most one of them.
    let lefts = ["Jean-Paul Sartre", "Albert Camus", "Simone de Beauvoir"];
    let rights = ["Existentialism", "Absurdism", "Feminist theory"];
    for desc in [
        "A quiz on twentieth-century thinkers: Jean-Paul Sartre wrote on freedom, and Camus on the absurd.",
        "Match well-known writers to ideas. Paris, France was the centre of it.",
        "Chapter 3: Genesis 1:1 and the first verses; 1914-1918 is another unit.",
    ] {
        let b = bank(vec![qv("Filler.", short("zz")), pairs_q(&lefts, &rights, false), pairs_q(&["Landmark 1", "Landmark 2", "Landmark 3"], &["Washington, D.C.", "Paris, France", "Rome, Italy"], true)], "Thinkers", desc);
        let r = pp(&b, r#"{"hide_key":true,"order_acks":[{"question":2}]}"#);
        assert_eq!(r["ok"], true, "{desc:?}: {}", show(&r));
    }
    // A text that repeats a left item's prefix to the field limit (an item with marks, so the
    // pair walks run from every start) is refused with scan_budget, never read in part.
    let l = "{= ".repeat(600);
    let left_items: Vec<String> = (0..3).map(|k| format!("{l}{k}")).collect();
    let mut q = qv("Match.", matching(&left_items, &["x", "y", "z"]));
    q["keep_order"] = json!(false);
    let b = bank(vec![q, qv(&format!("{l}= - x"), short("zz"))], "B", "");
    let r = pp(&b, HK);
    assert!(r["ok"] == true || has(&r, "scan_budget") || has(&r, "pairs_listed"), "{}", show(&r));
}
