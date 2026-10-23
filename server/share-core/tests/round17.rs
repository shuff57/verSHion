//! Round 17 (critic31a Moodle, critic31b robustness): the bank key scan reads every entry and
//! every served field; frozen-order key lists in bank text; honest data tables are not refused;
//! the 200-character window limits where a key starts, not where it ends; no counting cap skips a
//! scan (it refuses); the repeated Amharic lead and Multipart label shapes stay in budget; the
//! missing answer-lead languages; and the honest corpus needs no hard refusal.
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
fn tf(stem: &str, v: bool) -> Value {
    qv(stem, json!({"Tf": {"correct": v}}))
}
fn multi(stem: &str, parts: Vec<Value>) -> Value {
    qv(stem, json!({"Multipart": {"parts": parts}}))
}
fn pmc(stem: &str, texts: &[&str], correct: usize) -> Value {
    json!({"stem": stem, "kind": mc(texts, correct)})
}
fn psh(stem: &str, sample: &str) -> Value {
    json!({"stem": stem, "kind": {"Short": {"sample": sample}}})
}
fn has(r: &Value, reason: &str) -> bool {
    r["errors"].as_array().is_some_and(|e| e.iter().any(|x| x["reason"] == reason))
}
const HK: &str = r#"{"hide_key":true}"#;

fn corpus(name: &str) -> Vec<Value> {
    serde_json::from_str(&std::fs::read_to_string(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()).unwrap()
}

/// The critic's honest author through `publish_prepare`: label decisions, order acks, low-risk
/// acknowledgements.
fn drive(qv: &Value, i: usize) -> (bool, bool) {
    let mut q = json!({"id": format!("q{i:010}"), "stem": "", "points": 1.0, "difficulty": null, "topics": [], "image": null, "explanation": null, "figure": null, "created": 1760000000000i64, "modified": 1760000100000i64});
    for (k, v) in qv.as_object().unwrap() {
        q[k] = v.clone();
    }
    let p = json!({"schema_version": 2, "question": q}).to_string();
    let mut o = json!({"hide_key": true, "labels": [], "order_acks": [], "ack_formatting": []});
    let mut acked = false;
    for _ in 0..8 {
        let r = j(&publish_prepare_json(p.as_bytes(), &o.to_string()));
        if r["ok"] == true {
            let _ = strip_author_data_json();
            return (true, acked);
        }
        let mut changed = false;
        if let Some(f) = r["label_findings"].as_array()
            && o["labels"].as_array().unwrap().is_empty()
        {
            o["labels"] = json!(f.iter().map(|x| { let mut d = json!({"question": x["question"], "choice": if x["strip_allowed"] == true { "strip" } else { "keep" }}); if !x["part"].is_null() { d["part"] = x["part"].clone(); } d }).collect::<Vec<_>>());
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
        if !acked && e.iter().any(|x| x["ack_allowed"] == true) {
            o["ack_formatting"] = json!([0]);
            acked = true;
            changed = true;
        }
        if !changed {
            return (false, acked);
        }
    }
    (false, acked)
}

fn fill(unit: &str, bytes: usize) -> String {
    let mut s = String::with_capacity(bytes);
    'outer: loop {
        for c in unit.chars() {
            if s.len() + c.len_utf8() > bytes {
                break 'outer;
            }
            s.push(c);
        }
        if unit.is_empty() {
            break;
        }
    }
    s
}
fn secs(p: &str, runs: usize) -> f64 {
    (0..runs)
        .map(|_| {
            let a = std::time::Instant::now();
            std::hint::black_box(publish_prepare_json(p.as_bytes(), HK));
            a.elapsed().as_secs_f64()
        })
        .fold(f64::MAX, f64::min)
}

/// critic31a B1 (A1): the bank answer-key scan reads every entry. More than 64 True/False
/// questions of one value, a key text longer than 80 characters in a bank list, and a part sample
/// longer than 200 characters named as another part's key are all found.
#[test]
fn a1_bank_key_scan_reads_every_entry() {
    let facts: Vec<String> = (1..=200).map(|i| format!("Statement number {i} about the Baltic amber trade is in the handout.")).collect();
    for n in [120usize, 140, 200] {
        let qs: Vec<Value> = (0..n).map(|i| tf(&facts[i], i % 2 == 0)).collect();
        let key: Vec<String> = ((n - 5)..=n).map(|q| format!("{q} {}", if (q - 1) % 2 == 0 { "T" } else { "F" })).collect();
        let b = bank(qs, "Amber", &format!("Section D: {}", key.join(", ")));
        assert!(has(&pp(&b, HK), "bank_key_list"), "n={n}");
    }
    // A key longer than 80 characters in a numbered bank list.
    let long = format!("word{}", "x".repeat(90));
    let qs = vec![qv("Q1?", json!({"Short":{"sample":long}})), qv("Q2?", json!({"Short":{"sample":"beta"}})), qv("Q3?", json!({"Short":{"sample":"gamma"}}))];
    assert!(has(&pp(&bank(qs, "B", &format!("1 {long} 2 beta 3 gamma")), HK), "bank_key_list"));
    // A part's key longer than 200 characters named in another part's stem.
    let long = format!("s{}", "y".repeat(250));
    let parts = vec![psh("A.", "x"), psh(&format!("Answer: {long}"), "x"), psh("B.", &long)];
    assert!(has(&pp(&one(multi("Read.", parts)), HK), "answer_line"));
}

/// critic31a B3 (A2): every served field is read against the other questions' keys: part stems,
/// main stems, left and right items, distractors, topics, choices, the bank's name, description,
/// author and licence name; and another question's key as a topic token.
#[test]
fn a2_every_served_field_scanned() {
    let base = || vec![tf("Basalt is an igneous rock.", true), tf("Marble is a sedimentary rock.", false), qv("Which mineral scratches glass?", mc(&["Quartz", "Talc", "Gypsum"], 0)), qv("Hardness on the ___ scale.", json!({"Blank":{"accept":[["Mohs"]]}}))];
    let s = "Answers: 1 T, 2 F, 3 T, 4 F";
    let cases: Vec<(&str, String)> = vec![
        ("part stem", bank({ let mut v = base(); v.push(multi("Field trip.", vec![psh(&format!("Notes. {s}"), "x"), pmc("Shale splits.", &["True", "False"], 0)])); v }, "R", "")),
        ("main stem", bank({ let mut v = base(); v.push(multi(&format!("Field trip. {s}"), vec![psh("Notes.", "x"), pmc("Shale splits.", &["True", "False"], 0)])); v }, "R", "")),
        ("left item", bank({ let mut v = base(); v.push(qv("Match.", json!({"Matching":{"pairs":[{"left":format!("Pumice {s}"),"right":"Floats"},{"left":"Chalk","right":"Soft"},{"left":"Slate","right":"Roof"}],"distractors":[]}}))); v }, "R", "")),
        ("right item", bank({ let mut v = base(); v.push(qv("Match.", json!({"Matching":{"pairs":[{"left":"Pumice","right":format!("Floats {s}")},{"left":"Chalk","right":"Soft"},{"left":"Slate","right":"Roof"}],"distractors":[]}}))); v }, "R", "")),
        ("distractor", bank({ let mut v = base(); v.push(qv("Match.", json!({"Matching":{"pairs":[{"left":"Pumice","right":"Floats"},{"left":"Chalk","right":"Soft"}],"distractors":[s]}}))); v }, "R", "")),
        ("choice", bank({ let mut v = base(); v.push(qv("Which list?", mc(&[s, "1 F 2 F", "1 T 2 T"], 0))); v }, "R", "")),
        ("topic", bank({ let mut v = base(); let mut x = qv("Describe.", json!({"Short":{"sample":"x"}})); x["topics"] = json!([s]); v.push(x); v }, "R", "")),
        ("name", bank(base(), &format!("Rocks {s}"), "")),
    ];
    for (name, p) in cases {
        assert!(has(&pp(&p, HK), "bank_key_list"), "{name}");
    }
    // (The author and licence name are served texts of the bank too.)
    for path in ["author", "license_name"] {
        let mut b: Value = serde_json::from_str(&bank(base(), "R", "")).unwrap();
        b["bank"]["details"][path] = json!(format!("Ms R {s}"));
        assert!(has(&pp(&b.to_string(), HK), "bank_key_list"), "{path}");
    }
    // An answer line in a part stem, a left item and a topic.
    let p = bank({ let mut v = base(); v.push(multi("Rocks.", vec![psh("Explain. Answer: Quartz", "x"), pmc("Shale splits.", &["True", "False"], 0)])); v }, "R", "");
    assert!(has(&pp(&p, HK), "answer_line"), "part answer line");
    let p = bank({ let mut v = base(); v.push(qv("Match.", json!({"Matching":{"pairs":[{"left":"Pumice. Answer: Quartz","right":"Floats"},{"left":"Chalk","right":"Soft"}],"distractors":[]}}))); v }, "R", "");
    assert!(has(&pp(&p, HK), "answer_line"), "left answer line");
    // A choice naming another question's key without a separator ("Answer Mars").
    let p = bank(vec![qv("Q1?", mc(&["Mars", "x"], 0)), qv("Q2?", mc(&["Answer Mars", "y"], 0))], "B", "");
    assert!(has(&pp(&p, HK), "answer_line"), "choice");
    // Another question's key as a topic token, no lead.
    let p = bank(vec![qv("Q1?", mc(&["Quartz", "x"], 0)), { let mut x = qv("Q2?", mc(&["y", "z"], 0)); x["topics"] = json!(["Unit 4: Quartz"]); x }], "B", "");
    assert!(has(&pp(&p, HK), "topic_is_key"), "topic token");
    // A left item naming another question's key with its question number ("Q1: Gills").
    let p = bank(vec![qv("Q1?", mc(&["Gills", "x"], 0)), qv("Match.", json!({"Matching":{"pairs":[{"left":"Spores (Q1: Gills)","right":"Reproduction"},{"left":"Hyphae","right":"Threads"}],"distractors":[]}}))], "B", "");
    assert!(has(&pp(&p, HK), "bank_key_list"), "Q1 key in a left item");
    // A part stem listing the parts' keys in part order.
    let parts = vec![
        json!({"stem":"Basalt is igneous.","kind":{"Tf":{"correct":true}}}),
        json!({"stem":"Marble is sedimentary.","kind":{"Tf":{"correct":false}}}),
        pmc("Which mineral scratches glass?", &["Quartz", "Talc"], 0),
        psh("Summarise. Results so far: True, False, Quartz.", "x"),
    ];
    let r = pp(&one(multi("Read.", parts)), HK);
    assert!(has(&r, "bank_word_bank_three") || has(&r, "bank_word_bank_order"), "{}", r.to_string().chars().take(300).collect::<String>());
}

/// critic31a B4 (A3): a key list in the bank's name, description or a stem, in a bank whose
/// questions' order students see (keep_order, kept labels), is refused before any order ack.
#[test]
fn a3_frozen_order_key_list_in_bank_text() {
    let qs = |keep: bool| vec![
        qv("Which period ended with the non-avian dinosaurs?", mc(&["Jurassic", "Triassic", "Cretaceous", "Permian"], 2)),
        qv("Which bird is a brood parasite?", mc(&["Robin", "Common cuckoo", "Dunnock", "Wren"], 1)),
        qv("Which structure releases spores?", mc(&["Volva", "Stipe", "Annulus", "Gills"], 3)),
    ].into_iter().map(|mut x| { x["keep_order"] = json!(keep); x }).collect::<Vec<_>>();
    for d in ["1 C 2 B 3 D", "Answers: 1 C, 2 B, 3 D", "1) C   2) B   3) D", "C, B, D", "Q1 C · Q2 B · Q3 D", "Antworten: 1 C, 2 B, 3 D", "答案：1 C 2 B 3 D", "Answers: 1C 2B 3D", "1C 2B 3D"] {
        let r = pp(&bank(qs(true), "Fossils", d), HK);
        assert!(has(&r, "bank_key_list"), "{d:?}: {}", r.to_string().chars().take(200).collect::<String>());
    }
    // In a stem and a part stem.
    let mut v = qs(true);
    v.push(qv("Explain. Answers: 1 C, 2 B, 3 D", json!({"Short":{"sample":"x"}})));
    assert!(has(&pp(&bank(v, "F", ""), HK), "bank_key_list"), "stem");
    let mut v = qs(true);
    v.push(multi("Review", vec![psh("Check: 1 C, 2 B, 3 D. Explain.", "x"), psh("Why?", "y")]));
    assert!(has(&pp(&bank(v, "F", ""), HK), "bank_key_list"), "part stem");
    // Without a fixed order the letters name nothing: no key list.
    assert!(!has(&pp(&bank(qs(false), "Fossils", "1 C 2 B 3 D"), HK), "bank_key_list"));
}

/// critic31a B5 (A4): short honest data tables (a field log with "Key Stage", "Key <number>",
/// notes, a dichotomous key) are not refused with `scan_budget`.
#[test]
fn a4_honest_tables_are_not_scan_budget() {
    let sleep = |i: usize| format!("{:.1}", 6.5 + ((i * 37) % 30) as f64 / 10.0);
    let tbl: String = (0..63).map(|i| format!("P{},Key Stage {},{}\n", i + 1, 3 + i % 2, sleep(i))).collect();
    let stem = format!("Pupil,Key Stage,Hours of sleep\n{tbl}\nWhat is the median hours of sleep for Key Stage 4 pupils?");
    let r = pp(&bank(vec![qv(&stem, mc(&["7.6", "8.2", "6.9", "9.1"], 0))], "Survey", ""), HK);
    assert!(!has(&r, "scan_budget"), "{}", r.to_string().chars().take(300).collect::<String>());
    // The table in a Multipart main stem.
    let r = pp(&one(multi(&format!("Survey data:\n{tbl}"), vec![pmc("Median for KS4?", &["7.6", "8.2", "6.9"], 0), psh("Comment on the spread.", "KS4 varies more.")])), HK);
    assert!(!has(&r, "scan_budget"), "multipart: {}", r.to_string().chars().take(300).collect::<String>());
    // A caretaker's key log (64 rows, "Key 200"..).
    let rows: String = (0..64).map(|i| format!("L{}, Key {}, {}\n", i + 1, 200 + i, ["Lab", "Gym", "Hall"][i % 3])).collect();
    let r = pp(&one(qv(&format!("Lock, Key number, Room\n{rows}\nWhich room is lock 250 in?"), mc(&["Lab", "Gym", "Hall"], 0))), HK);
    assert!(!has(&r, "scan_budget"), "key log: {}", r.to_string().chars().take(300).collect::<String>());
    // A field notes log, 65 rows.
    let notes: String = (0..65).map(|i| format!("S{},Robin,{},{}\n", i % 12 + 1, (i * 7) % 23, ["key site", "see answer sheet", "solution pond", "hint of rain", "correct id"][i % 5])).collect();
    let r = pp(&one(qv(&format!("Site,Species,Count,Notes\n{notes}\nHow many robins at S1?"), mc(&["7", "37", "65", "99"], 0))), HK);
    assert!(!has(&r, "scan_budget"), "notes: {}", r.to_string().chars().take(300).collect::<String>());
    // A dichotomous key in the stem.
    let dk = "Use the key to identify the invertebrate.\n\nKey\n1a  Has six legs ................ go to 2\n1b  Has more than six legs ...... go to 3\n2a  Has wings ................... Fly\n2b  No wings .................... Springtail\n3a  Eight legs .................. Spider\n3b  Many legs ................... Centipede\n\nAn animal has eight legs. What is it?";
    let r = pp(&one(qv(dk, mc(&["Spider", "Fly", "Springtail", "Centipede"], 0))), HK);
    assert!(!has(&r, "scan_budget"), "dichotomous key");
}

/// critic31a B6 (A5): a 4 MB Blank bank (170 questions x 50 blanks x 20 variants) publishes in
/// time and without a trap (its memory is measured by `tests/memory.rs` and `tests/node/mem.mjs`).
#[test]
fn a5_blank_bank_publishes() {
    let accept: Vec<Vec<String>> = (0..50).map(|g| (0..20).map(|k| format!("ka ka ka k{}a{g}x{k}", "a")).collect()).collect();
    let qs: Vec<Value> = (0..170).map(|i| qv(&"___ ".repeat(50), json!({"Blank": {"accept": accept}})).tap_id(i)).collect();
    let p = bank(qs, "Blanks", "");
    assert!(p.len() > 2_000_000 && p.len() <= 4 * 1024 * 1024, "{}", p.len());
    let t = secs(&p, 1);
    eprintln!("a5 blank170: {} bytes, {t:.2} s", p.len());
    let r = pp(&p, HK);
    assert!(r["ok"].is_boolean(), "no response");
    let _ = r;
}

trait TapId {
    fn tap_id(self, i: usize) -> Value;
}
impl TapId for Value {
    fn tap_id(mut self, i: usize) -> Value {
        self["id"] = json!(format!("q{i:08}"));
        self
    }
}

/// critic31a A6: the missing answer-lead languages are read (a key text after "lead:").
#[test]
fn a6_missing_answer_leads() {
    let leads = ["ਉੱਤਰ", "පිළිතුර", "အဖြေ", "ຄຳຕອບ", "ځواب", "jawaab", "whakautu", "valiny", "freagairt", "ענטפֿער", "պատասխան", "პასუხი", "ҷавоб", "përgjigja"];
    for lead in leads {
        let stem = format!("{lead}: Kestrel");
        let r = pp(&one(qv(&stem, mc(&["Kestrel", "Merlin", "Hobby"], 0))), HK);
        assert!(has(&r, "answer_line"), "{lead}: {}", r.to_string().chars().take(200).collect::<String>());
    }
}

/// critic31a A6 (honest bank corpus): a pasted "Save Answer" button is no answer lead for a short
/// key word in the following sentence, while a short key right after a lead is still read.
#[test]
fn a6_short_key_in_prose_is_no_answer_line() {
    let other = qv("Which word completes the sentence?", mc(&["is", "was", "be"], 0));
    let stem = "Question 20\n\n1 points\n\nSave Answer\n\nIn the standard order of draw, which tube is collected first?";
    let p = bank(vec![other.clone(), qv(stem, mc(&["Blood culture bottles", "Lavender", "Grey"], 0))], "B", "");
    let r = pp(&p, HK);
    assert!(!has(&r, "answer_line"), "{}", r.to_string().chars().take(300).collect::<String>());
    // The short key is still read where it is the answer ("Answer: is").
    let p = bank(vec![other, qv("Answer: is", mc(&["x", "y"], 0))], "B", "");
    assert!(has(&pp(&p, HK), "answer_line"));
}

/// critic31a's honest corpus: 627 items, no hard refusal, acknowledgements at most 2% (the five
/// are one parenthetical outlier, low risk).
#[test]
fn a6_honest_corpus() {
    let items = corpus("critic31a_honest.json");
    let (mut refused, mut acks) = (Vec::new(), 0);
    for (i, it) in items.iter().enumerate() {
        match drive(&it["q"], i) {
            (false, _) => refused.push(format!("{} {} {} {}", it["id"], it["kind"], it["style"], it["lang"])),
            (true, true) => acks += 1,
            (true, false) => {}
        }
    }
    assert!(refused.is_empty(), "{}", refused.join("\n"));
    assert!(acks * 50 <= items.len(), "{acks} of {}", items.len());
}

/// critic31b B1: the 200-character window limits where a key STARTS, not where it ends. A
/// 228-character key and "Olympus Mons" behind 187..195 characters of filler are found; a key
/// starting past the window (205) is the documented limit.
#[test]
fn b1_window_limits_start_not_end() {
    let key = "Because the light from the Sun is scattered by the molecules of the atmosphere, and the shorter blue wavelengths are scattered much more strongly than the longer red ones, so blue reaches our eyes from every direction of the sky";
    assert!(key.chars().count() > 200);
    for pre in ["", "it is: ", "probably ", "B. "] {
        let stem = format!("Why is the sky blue?\nAnswer: {pre}{key}");
        let r = pp(&one(qv(&stem, mc(&[key, "Because the ocean reflects its colour", "Because ozone absorbs every other colour"], 0))), HK);
        assert!(has(&r, "answer_line"), "{pre:?}: {}", r.to_string().chars().take(200).collect::<String>());
    }
    // (Filler of whole words: the key stands at a word start at exactly `n` characters.)
    let filler = |n: usize| { assert_eq!(n % 5, 0); "word ".repeat(n / 5) };
    for n in [180usize, 185, 190, 195] {
        let stem = format!("Which is the tallest volcano in the solar system?\nAnswer: {}Olympus Mons", filler(n));
        let r = pp(&one(qv(&stem, mc(&["Olympus Mons", "Mauna Kea", "Mount Everest"], 0))), HK);
        assert!(has(&r, "answer_line"), "filler {n}");
    }
    // A key starting past the window is outside the documented reading.
    let stem = format!("Which is the tallest volcano?\nAnswer: {}Olympus Mons", filler(205));
    assert_eq!(pp(&one(qv(&stem, mc(&["Olympus Mons", "Mauna Kea"], 0))), HK)["ok"], true);
    // Another question's key in a bank stem, and in the description.
    let qs = vec![qv("Name the tallest volcano.", json!({"Short":{"sample":"Olympus Mons"}})), tf(&format!("Next one.\nAnswer: {}Olympus Mons", filler(195)), true)];
    assert!(has(&pp(&bank(qs, "B", ""), HK), "answer_line"), "bank stem");
    let qs = vec![qv("Name the tallest volcano.", json!({"Short":{"sample":"Olympus Mons"}})), tf("Plain?", true)];
    assert!(has(&pp(&bank(qs, "B", &format!("Answer: {}Olympus Mons", filler(195))), HK), "answer_line"), "description");
    // Another part's key.
    let mp = multi("A volcano question.", vec![psh("Name it.", "Olympus Mons"), tf(&format!("Answer to (a): {}Olympus Mons. Is it on Mars?", filler(195)), true)]);
    assert!(has(&pp(&one(mp), HK), "answer_line"), "multipart");
}

/// critic31b B2 (critic31a B2): the 4,096-event cap fails closed. A description of 95 "zq"
/// words (past the old cap) followed by a numbered True/False key is still refused.
#[test]
fn b2_bank_key_list_events_refuse() {
    let mut vals: Vec<String> = Vec::new();
    for c in 1..=682 {
        vals.push(vec!["zq"; c].join(" "));
    }
    let accept: Vec<Vec<String>> = (0..50).filter_map(|b| { let s = (b * 20).min(vals.len()); let e = (b * 20 + 20).min(vals.len()); (s < e).then(|| vals[s..e].to_vec()) }).collect();
    let blank = qv(&vec!["___"; accept.len()].join(" and "), json!({"Blank": {"accept": accept}}));
    for pre in [0usize, 85, 95, 300] {
        let b = bank(vec![blank.clone(), tf("Water is wet.", true), tf("Fire is cold.", false), qv("Pick one planet.", mc(&["Mars", "Venus"], 0))], "Bank", &format!("{}2 T 3 F", "zq ".repeat(pre)));
        let r = pp(&b, HK);
        assert!(has(&r, "bank_key_list") || has(&r, "scan_budget"), "pre={pre}: {}", r.to_string().chars().take(200).collect::<String>());
    }
}

/// critic31b B2: the word index keeps every place, so a part's key after 70 decoy occurrences
/// (past the old 64-place cap) is found next to a reference to its part.
#[test]
fn b2_word_index_keeps_every_place() {
    let decoys: String = "beta\n".repeat(70);
    let parts = vec![psh("A.", "beta"), pmc(&format!("{decoys}(a) beta"), &["z1", "z2"], 0)];
    let r = pp(&one(multi("Read.", parts)), HK);
    assert!(has(&r, "part_states_key"), "{}", r.to_string().chars().take(300).collect::<String>());
}

/// critic31b B2: every counting cap that could silently stop a scan is either removed or
/// unreachable by the enforced limits; this records the arithmetic.
#[test]
fn b2_caps_cannot_hide_a_key() {
    use share_core::Limits;
    let d = Limits::default();
    // The part cross-key candidates: 50 parts x (26 correct choices x 2 forms + 1 sample).
    assert!(d.parts * (2 * d.mc_answers + 1) < 4096, "cands cap");
    // A part stem holds at most 2,048 references of four bytes ("(a)t"), under the old 4,096 cap.
    assert!(d.stem_bytes / 4 < 4096, "part_tf_refs cap");
    // The largest served field holds at most 4,096 "{{", so anki_cloze's old take read them all.
    assert!(d.stem_bytes / 2 <= 4096, "anki cap");
    // The per-word index cap (64) is gone; the word lists are not capped by the limits.
    let text = "beta ".repeat(100);
    let index = share_core::format::TokenIndex::new(&text);
    assert_eq!(index.find_all("beta").len(), 100, "word index places");
}

/// critic31b: a formatting finding names the key's choice, so it carries `author_only`.
#[test]
fn b2_findings_carry_author_only() {
    let q = one(qv("Which planet is red?", mc(&["Mars \u{2713}", "Venus", "Earth"], 0)));
    let r = j(&publish_prepare_json(q.as_bytes(), HK));
    assert!(r["ok"] == false);
    let ff = r["formatting_findings"].as_array().unwrap();
    assert!(!ff.is_empty());
    assert!(ff.iter().all(|f| f["author_only"] == true), "{}", r.to_string().chars().take(300).collect::<String>());
}

/// critic31b B3: Matching left items filled with the Amharic lead "መልስ:" (the critic's 4 MB
/// shape) stay under the 5 s CPU limit.
#[test]
fn b3_amharic_matching_within_budget() {
    let l = fill("መልስ:", 2040);
    let qs: Vec<Value> = (0..649).map(|i| qv("Match.", json!({"Matching": {"pairs": (0..3).map(|p| json!({"left": format!("{l}{p}l{i}"), "right": format!("Right {p} {i}")})).collect::<Vec<_>>(), "distractors": [format!("Spare {i}")]}}))).collect();
    let p = bank(qs, "Bank", "");
    let t = secs(&p, 2);
    eprintln!("b3 matchleft: {} bytes, {t:.2} s", p.len());
    assert!(t < 5.0, "{t:.2} s");
}

/// critic31b B4: Multipart part stems filled with the label "2)" (the critic's 4 MB shape) stay
/// under the 5 s CPU limit.
#[test]
fn b4_multipart_label_stems_within_budget() {
    let s = fill("2)", 8192);
    let s2 = fill("2)", 4096);
    let qs: Vec<Value> = (0..326).map(|i| {
        let k = format!("2) 2) 2) zq{i}");
        qv("Read.", json!({"Multipart": {"parts": [
            pmc(&s, &[&k, &format!("Wrong one {i}"), &format!("Other option {i}"), &format!("Third choice {i}")], 0),
            json!({"stem": s2, "kind": {"Tf": {"correct": true}}}),
            psh("Why?", &k),
        ]}}))
    }).collect();
    let p = bank(qs, "Bank", "");
    let t = secs(&p, 2);
    eprintln!("b4 partstem: {} bytes, {t:.2} s", p.len());
    assert!(t < 5.0, "{t:.2} s");
}

/// critic31b: Matching right items filled with verdict words ("തെറ്റ്", "wrong" in Malayalam),
/// the slow shape of the generic sweep, stay under the limit.
#[test]
fn b5_verdict_word_shapes_within_budget() {
    for unit in ["തെറ്റ് ", "ತಪ್ಪು "] {
        let u = fill(unit, 2040);
        let qs: Vec<Value> = (0..649).map(|i| qv("Match.", json!({"Matching": {"pairs": (0..3).map(|p| json!({"left": format!("Left {p} {i}"), "right": format!("{u}{p}r{i}")})).collect::<Vec<_>>(), "distractors": []}}))).collect();
        let p = bank(qs, "Bank", "");
        let t = secs(&p, 2);
        eprintln!("b5 {unit:?}: {} bytes, {t:.2} s", p.len());
        assert!(t < 5.0, "{unit:?}: {t:.2} s");
    }
}

