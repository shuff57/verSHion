//! Round 21 (critic37a Moodle, critic37b robustness):
//! * B1 (37b): every Default_Ignorable / invisible format character inside a key word is folded
//!   out of the key and hint scans (the served text is untouched); the table is checked against
//!   every Cf code point so a character class added in future is caught.
//! * B2 (37b, 37a): `Ans.` is a lead for a numbered row; a lead that ends a line after prose, with
//!   a numbered row on the next line, is read like one that opens its line; the weaker lead words
//!   ("Keys", "Marking scheme", "Marking guide", "Rubric", "Memo", "Marks") are low-risk leads.
use serde_json::{Value, json};
use share_core::wasm::*;

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


fn pairs_q(lefts: &[&str], rights: &[&str], keep: bool) -> Value {
    let l: Vec<String> = lefts.iter().map(|s| s.to_string()).collect();
    let mut q = qv("Match each item.", matching(&l, rights));
    q["keep_order"] = json!(keep);
    q
}

// ---- B1 (37b): invisible characters inside key words ----------------------------------------

fn ignorable_points() -> Vec<char> {
    let mut v = Vec::new();
    for &(a, b) in share_core::format::DEFAULT_IGNORABLE {
        for u in a..=b {
            if let Some(c) = char::from_u32(u) {
                v.push(c);
            }
        }
    }
    v
}

fn with_mark(word: &str, at: usize, c: char) -> String {
    let mut s = String::new();
    for (i, ch) in word.chars().enumerate() {
        if i == at {
            s.push(c);
        }
        s.push(ch);
    }
    s
}

#[test]
fn b1_every_format_character_is_ignorable_or_visible() {
    use unicode_general_category::{GeneralCategory, get_general_category};
    let mut stray = Vec::new();
    for u in 0..=0x10FFFFu32 {
        let Some(c) = char::from_u32(u) else { continue };
        if get_general_category(c) != GeneralCategory::Format {
            continue;
        }
        let known = share_core::format::DEFAULT_IGNORABLE.iter().chain(share_core::format::VISIBLE_FORMAT).any(|&(a, b)| u >= a && u <= b);
        if !known {
            stray.push(format!("U+{u:04X}"));
        }
    }
    assert!(stray.is_empty(), "format characters in neither table: {stray:?}");
}

#[test]
fn b1_invisible_characters_inside_a_key_word_change_nothing() {
    let pts = ignorable_points();
    let mut bad = Vec::new();
    for &c in &pts {
        for at in 1..5 {
            let w = with_mark("Paris", at, c);
            let r = pp(&one(qv(&format!("Capital?\nAnswer: {w}"), short("Paris"))), HK);
            // Refused by the key scan, or by the text rules (a character that may not be stored).
            if r["ok"] == true {
                bad.push(format!("U+{:04X}@{at}", c as u32));
            }
        }
    }
    assert!(bad.is_empty(), "{} published: {:?}", bad.len(), &bad[..bad.len().min(12)]);
}

#[test]
fn b1_every_scan_folds_them() {
    // The characters that are stored (the text rules accept them) must reach the scans: a sample
    // of each range, in every kind of scan.
    let sample: Vec<char> = [0x00AD, 0x034F, 0x061C, 0x115F, 0x180E, 0x200B, 0x200E, 0x200F, 0x2061, 0x2062, 0x2063, 0x2064, 0x3164, 0xFE0F, 0xFEFF, 0xFFA0, 0x1D173, 0xE0100]
        .iter()
        .filter_map(|u| char::from_u32(*u))
        .collect();
    let mut bad = Vec::new();
    for c in sample {
        let m = |w: &str| with_mark(w, 2, c);
        let u = format!("U+{:04X}", c as u32);
        // Mc fixed order, True/False, Blank.
        let ch = ["Talc", "Quartz", "Diamond", "Gypsum"];
        let mut q = qv(&format!("Hardest?\nAnswer: {}", m("Quartz")), mc(&ch, 1));
        q["keep_order"] = json!(true);
        let r = pp(&one(q), r#"{"hide_key":true,"order_acks":[{"question":0}]}"#);
        if r["ok"] == true { bad.push(format!("{u} mc")); }
        let r = pp(&one(qv(&format!("Mars is a planet.\nAnswer: {}", m("True")), json!({"Tf": {"correct": true}}))), HK);
        if r["ok"] == true { bad.push(format!("{u} tf")); }
        let r = pp(&one(qv(&format!("Name it ___.\nAnswer: {}", m("osmosis")), json!({"Blank": {"accept": [["osmosis"]]}}))), HK);
        if r["ok"] == true { bad.push(format!("{u} blank")); }
        // A bank key list in the description.
        let desc = format!("Answers: 1 {} 2 {} 3 {}", m("Paris"), m("Madrid"), m("Rome"));
        let b = bank(vec![qv("Capital of France?", short("Paris")), qv("Capital of Spain?", short("Madrid")), qv("Capital of Italy?", short("Rome"))], "B", &desc);
        let r = pp(&b, HK);
        if r["ok"] == true { bad.push(format!("{u} keylist")); }
        // Pairs listed, and a rights list.
        let l = ["Bauxite", "Chalk", "Slate"];
        let rr = ["Aluminium", "Soft", "Roof"];
        let text = format!("{} - {}; {} - {}; {} - {}", l[0], m(rr[0]), l[1], m(rr[1]), l[2], m(rr[2]));
        let b = bank(vec![qv("Filler.", short("zz")), pairs_q(&l, &rr, false)], "B", &text);
        if pp(&b, HK)["ok"] == true { bad.push(format!("{u} pairs")); }
        let text = format!("Terms: {}; {}; {}.", m(rr[0]), m(rr[1]), m(rr[2]));
        let b = bank(vec![qv("Filler.", short("zz")), pairs_q(&l, &rr, true)], "B", &text);
        if pp(&b, r#"{"hide_key":true,"order_acks":[{"question":1}]}"#)["ok"] == true { bad.push(format!("{u} rights")); }
        // The author's text is served as written (the mark is not stripped from a stem the
        // scans refuse; here an honest stem keeps it).
        let honest = format!("Which is the capital of {}?", m("France"));
        let r = pp(&one(qv(&honest, mc(&["Paris", "Rome", "Oslo"], 0))), HK);
        if r["ok"] == true && r["stripped"]["payload"]["question"]["stem"] != json!(honest) && c as u32 != 0xAD && c as u32 != 0x200B && c as u32 != 0xFEFF && c as u32 != 0x034F {
            bad.push(format!("{u} served text changed: {}", r["stripped"]["payload"]["question"]["stem"]));
        }
    }
    assert!(bad.is_empty(), "{} not refused: {:?}", bad.len(), &bad[..bad.len().min(12)]);
}

#[test]
fn b1_honest_emoji_and_variation_selectors_publish() {
    for stem in ["Which family is shown: \u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}?", "Pick the heart \u{2764}\u{FE0F} or the star \u{2B50}.", "Rainbow flag \u{1F3F3}\u{FE0F}\u{200D}\u{1F308} is used for what?", "Which is the warning sign \u{26A0}\u{FE0F}?", "Name this: \u{1F469}\u{200D}\u{2695}\u{FE0F}."] {
        let r = pp(&one(qv(stem, mc(&["Family", "Flag", "Doctor"], 1))), HK);
        assert_eq!(r["ok"], true, "{stem:?}: {}", show(&r));
        assert_eq!(r["stripped"]["payload"]["question"]["stem"], json!(stem), "served text unchanged");
    }
}

// ---- B2 (37b, 37a): key-lead layouts ---------------------------------------------------------

#[test]
fn b2_ans_with_a_full_stop_and_a_mid_line_lead() {
    let ch = ["Talc", "Quartz", "Diamond", "Gypsum"];
    let fixed = |stem: &str| {
        let mut q = qv(stem, mc(&ch, 1));
        q["keep_order"] = json!(true);
        pp(&one(q), r#"{"hide_key":true,"order_acks":[{"question":0}]}"#)
    };
    for line in ["Ans. 1 B", "Ans. 1. B", "Ans. (1) B", "Ans. Q1: B", "ans. 1 B", "Ans.\n1 B", "Ans.\n1. B"] {
        let r = fixed(&format!("Hardest?\n{line}"));
        assert!(has(&r, "answer_line"), "{line:?}: {}", show(&r));
    }
    for line in ["Ans. 1 True", "Ans. (1) True", "ans. 1 True", "Ans.\n1 True"] {
        let r = pp(&one(qv(&format!("Mars is a planet.\n{line}"), json!({"Tf": {"correct": true}}))), HK);
        assert!(has(&r, "answer_line"), "{line:?}: {}", show(&r));
    }
    // A lead at the end of a line after prose, the row on the next line.
    let leads = ["Key:", "Correct:", "Kunci:", "Answer:", "Answers:", "Answer key:", "Solution:", "Ans:", "Mark scheme:", "Model answer:", "Key", "Correct"];
    let mut miss = Vec::new();
    for l in leads {
        for (stem, kind) in [
            (format!("Capital of France? {l}\n1. Paris"), short("Paris")),
            (format!("Mars is a planet. {l}\n1 True"), json!({"Tf": {"correct": true}})),
            (format!("Name the capital. (2 marks) {l}\n1) Paris"), short("Paris")),
        ] {
            let r = pp(&one(qv(&stem, kind)), HK);
            if !has(&r, "answer_line") {
                miss.push(format!("{stem:?}"));
            }
        }
        let r = fixed(&format!("Hardest? {l}\n1. B"));
        if !has(&r, "answer_line") {
            miss.push(format!("fixed {l}"));
        }
    }
    assert!(miss.is_empty(), "{} published: {:?}", miss.len(), &miss[..miss.len().min(10)]);
    // Part stem and another question's stem in a bank.
    let mp = json!({"Multipart": {"parts": [{"stem": "Capital? Key:\n1. Paris", "kind": short("Paris")}, {"stem": "Colour?", "kind": short("blue")}]}});
    assert!(pp(&one(qv("Read.", mp)), HK)["ok"] != true);
    let r = pp(&bank(vec![qv("Name it.", short("Zebra")), qv("Notes. Correct:\n1. Zebra", json!({"Tf": {"correct": true}}))], "B", ""), HK);
    assert!(has(&r, "answer_line"), "bank: {}", show(&r));
}

#[test]
fn b2_honest_guards_hold() {
    let ch = ["Talc", "Quartz", "Diamond", "Gypsum"];
    // Round 18's dichotomous key, every position of the correct choice.
    let dk = "Use the key to identify the invertebrate.\n\nKey\n1a  Has six legs ................ go to 2\n1b  Has more than six legs ...... go to 3\n2a  Has wings ................... Fly\n2b  No wings .................... Springtail\n3a  Eight legs .................. Spider\n3b  Many legs ................... Centipede\n\nAn animal has eight legs. What is it?";
    for c in 0..4 {
        let r = pp(&one(qv(dk, mc(&["Spider", "Fly", "Springtail", "Centipede"], c))), HK);
        assert_eq!(r["ok"], true, "dichotomous {c}: {}", show(&r));
    }
    // Prose that merely ends in "answer:" or "key:", followed by the student's tasks.
    let mut hard = Vec::new();
    for stem in [
        "Read the passage and write your answer:\n1. Explain why the sample melts.\n2. Name one use.\nWhich is hardest?",
        "Write your answer:\n1. Talc\n2. Gypsum\nWhich is hardest?",
        "Use the key:\n1. Hardness\n2. Streak\nWhich is hardest?",
        "Circle the correct answer:\n1. Talc is soft\n2. Gypsum is soft\nWhich is hardest?",
        "Check your answers:\n1. Compare with a partner.\nWhich is hardest?",
        "Key terms\n1. Photosynthesis\n2. Respiration\nWhich is hardest?",
        "Key points:\n1. Plants make sugar\nWhich is hardest?",
        "Key stage 3\n1. Plants\nWhich is hardest?",
        "Key vocabulary\n1. mineral\n2. hardness\nWhich is hardest?",
        "Marks: 3 of 4\nWhich is hardest?",
        "Memo: bring a ruler.\nWhich is hardest?",
        "Rubric\n1. Neatness\n2. Accuracy\nWhich is hardest?",
    ] {
        for c in [1usize, 2] {
            let r = pp(&one(qv(stem, mc(&ch, c))), HK);
            if r["ok"] != true {
                let low = r["errors"].as_array().is_some_and(|e| e.iter().all(|x| x["reason"] == "key_heading_row" && x["ack_allowed"] == true));
                if !low {
                    hard.push(format!("{stem:?} c={c}: {}", show(&r)));
                }
            }
        }
    }
    assert!(hard.is_empty(), "hard refusals of honest text: {hard:#?}");
}

#[test]
fn b3_weaker_lead_words_are_low_risk() {
    for lead in ["Keys", "Marking scheme", "Marking guide", "Rubric", "Memo", "Marks"] {
        for row in ["1. Paris", "1) Paris", "(1) Paris"] {
            for sep in ["\n", ": ", " ", ":\n"] {
                let stem = format!("Capital of France?\n{lead}{sep}{row}");
                let r = pp(&one(qv(&stem, short("Paris"))), HK);
                assert!(has(&r, "key_heading_row") && r["errors"][0]["ack_allowed"] == true, "{stem:?}: {}", show(&r));
                let a = pp(&one(qv(&stem, short("Paris"))), r#"{"hide_key":true,"ack_formatting":[0]}"#);
                assert_eq!(a["ok"], true, "acknowledged {stem:?}: {}", show(&a));
            }
        }
    }
}
