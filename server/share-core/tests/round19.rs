//! Round 19 (critic35a Moodle, critic35b robustness):
//! * B1 (35a): the Matching rights-order scan is linear and charged to the work budget (480
//!   fixed-order Matching questions sharing "Yes / No / Maybe" and 480 stems listing them took
//!   25 s); an input that would still cost too much is refused with `scan_budget`, never skipped.
//! * B2 (35a): GIFT True/False with feedback (`{TRUE#well done}`, `{F#no}`,
//!   `{TRUE#if wrong#if right}`) is read like the bare form.
//! * B3 (35b): 950 questions of 26 choices that each start "x - x - ..." took 10 s in
//!   `publish_prepare` with the author's label decisions: the sibling-reference answers are kept
//!   for the length of a publish (one hash per repeat) and each reading is charged.
//! * B4 (35b): a numbered row under a `Key` / `Answer key` heading line is read; the dichotomous
//!   key worksheet (`1a`/`1b` couplets) and honest lists under "Key terms" are not refused.
//!   CPU tests compare against a same-size control (ratios) and use generous absolute bounds, so a
//!   loaded machine does not flake them.
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

fn secs(p: &str, o: &str) -> f64 {
    (0..2)
        .map(|_| {
            let a = Instant::now();
            std::hint::black_box(publish_prepare_json(p.as_bytes(), o));
            a.elapsed().as_secs_f64()
        })
        .fold(f64::MAX, f64::min)
}

// ---- B1 (35a): the Matching rights-order scan ------------------------------------------------

fn rights_bank(m: usize, c: usize, text_bytes: usize, keep: bool) -> String {
    let mut qs = Vec::new();
    for i in 0..m {
        let lefts: Vec<String> = ["a", "b", "c"].iter().map(|x| format!("Statement {i} {x}")).collect();
        let mut q = qv(&format!("Classify {i}"), matching(&lefts, &["Yes", "No", "Maybe"]));
        q["keep_order"] = json!(keep);
        qs.push(q);
    }
    let unit = "Yes, No, Maybe, ";
    let mut t = unit.repeat(text_bytes / unit.len() + 1);
    t.truncate(text_bytes - 5);
    for _ in 0..c {
        qs.push(qv(&t, short("x")));
    }
    bank(qs, "B", "")
}

/// The critic's 4 MB shape (480 questions sharing the rights, 480 carrier stems): the same bytes
/// with `keep_order` off take about a second; with it on they took 25 s. Bound by the control and
/// by a generous absolute time.
#[test]
fn b1_rights_order_scan_is_linear() {
    let keep = rights_bank(480, 480, 8000, true);
    let shuf = rights_bank(480, 480, 8000, false);
    assert!(keep.len() > 3_900_000 && keep.len() <= 4 * 1024 * 1024, "{}", keep.len());
    let (tk, ts) = (secs(&keep, HK), secs(&shuf, HK));
    eprintln!("rights order: keep {tk:.2}s, shuffled {ts:.2}s");
    assert!(tk < 20.0, "keep_order shape took {tk:.2} s");
    assert!(tk < 8.0 * ts + 2.0, "keep_order {tk:.2} s against {ts:.2} s for the same bytes");
    // (Whatever it returns, it is an answer: never a silent skip. The rights stand in a list in
    // pair order thousands of times, so a refusal is the right result as well.)
    let r = pp(&keep, HK);
    assert!(r["ok"] == true || has(&r, "word_bank_order") || has(&r, "scan_budget"), "{}", show(&r));
}

/// Questions that share one rights column are one group: the scan's answer does not depend on how
/// many there are. The list in pair order is refused for 1, 2, 5 and 300 sharing questions, and a
/// list out of pair order is not.
#[test]
fn b1_shared_columns_keep_their_verdicts() {
    for m in [1usize, 2, 5, 300] {
        let mut qs = Vec::new();
        for i in 0..m {
            let lefts: Vec<String> = ["a", "b", "c"].iter().map(|x| format!("Statement {i} {x}")).collect();
            let mut q = qv(&format!("Classify {i}"), matching(&lefts, &["Aluminium", "Soft", "Roof"]));
            q["keep_order"] = json!(true);
            qs.push(q);
        }
        let r = pp(&bank(qs.clone(), "B", "Aluminium, Soft, Roof"), HK);
        assert!(has(&r, "word_bank_order"), "m={m}: {}", show(&r));
        let r = pp(&bank(qs.clone(), "B", "Roof, Aluminium, Soft"), HK);
        assert_eq!(r["ok"], true, "m={m}, out of order: {}", show(&r));
        // A text is read on its own: an earlier list does not carry its state into the next.
        let mut more = qs;
        more.push(qv("Aluminium, Soft", short("x")));
        more.push(qv("Roof, Aluminium, Soft", short("y")));
        let r = pp(&bank(more, "B", ""), HK);
        // (Lists of two and of three out of pair order: the pair-order verdict is not given.)
        assert!(!has(&r, "word_bank_order"), "m={m}, split lists: {}", show(&r));
    }
}

/// An input whose reading would pass the text's step budget is refused with `scan_budget`: 480
/// questions with different rights columns that all hold "Yes" and a stem that lists "Yes" 1,500
/// times is 720,000 steps for one text.
#[test]
fn b1_over_budget_refuses() {
    let mut qs = Vec::new();
    for i in 0..480 {
        let lefts: Vec<String> = ["a", "b", "c"].iter().map(|x| format!("Statement {i} {x}")).collect();
        let (n, m) = (format!("No{i}"), format!("Maybe{i}"));
        let mut q = qv(&format!("Classify {i}"), matching(&lefts, &["Yes", n.as_str(), m.as_str()]));
        q["keep_order"] = json!(true);
        qs.push(q);
    }
    qs.push(qv(&"Yes, ".repeat(1500), short("x")));
    let a = Instant::now();
    let r = pp(&bank(qs, "B", ""), HK);
    assert!(has(&r, "scan_budget"), "{}", show(&r));
    assert!(a.elapsed().as_secs_f64() < 20.0);
}

/// The pairs scan counts per question without a search: many questions sharing one pair, and one
/// pair listed twice in a text, are still `pairs_listed`; one pair once is not.
#[test]
fn b1_pairs_scan_dense_counts() {
    let mk = |i: usize| qv("Classify", matching(&[format!("Yes{i}"), "No".to_string()], &["Y", "N"]));
    let qs: Vec<Value> = (0..200).map(mk).collect();
    let r = pp(&bank(qs.clone(), "B", "Yes7 - Y; No - N"), HK);
    assert!(has(&r, "pairs_listed"), "{}", show(&r));
    let r = pp(&bank(qs, "B", "Yes7 - Y"), HK);
    assert_eq!(r["ok"], true, "{}", show(&r));
}

// ---- B2 (35a): GIFT True/False with feedback -----------------------------------------------

#[test]
fn b2_gift_true_false_with_feedback_is_read() {
    let forms = [
        "{TRUE#well done}", "{FALSE#Not quite}", "{T#good}", "{F#no}", "{TRUE#Great job!#Check your notes}", "{TRUE#}", "{true#Yes}", "{ T # fb }",
        "{TRUE #well done}", "{TRUE\n#well done}", "{TRUE#wrong#right}", "{FALSE#if wrong#if right}",
    ];
    for f in forms {
        let stem = format!("Mars is a planet. {f}");
        let r = pp(&one(qv(&stem, json!({"Tf": {"correct": true}}))), HK);
        assert!(has(&r, "gift"), "{f:?} in a Tf stem: {}", show(&r));
        let part = json!({"stem": stem, "kind": {"Tf": {"correct": false}}});
        let r = pp(&one(qv("Read.", json!({"Multipart": {"parts": [part, {"stem": "Why?", "kind": {"Short": {"sample": "x"}}}]}}))), HK);
        assert!(has(&r, "gift"), "{f:?} in a Tf part stem: {}", show(&r));
    }
    // The bare form and the choice-feedback forms are still read.
    for f in ["{TRUE}", "{F}", "{=Mars#right ~Venus#wrong}", "{~Venus#wrong =Mars#right}", "{=Mars#fb}", "{#4}"] {
        let r = pp(&one(qv(&format!("Which planet is red? {f}"), mc(&["Mars", "Venus"], 0))), HK);
        assert!(r["ok"] == false, "{f:?}: {}", show(&r));
    }
    // Honest braces: LaTeX, set notation, a brace with no feedback, no closing brace.
    for f in ["Let A^{T} be the transpose.", "The set {1, 2, 3} has three elements.", "Use {curly} braces for a dictionary.", "Write {TRUE or FALSE for each.", "A {True story} about Mars."] {
        let r = pp(&one(qv(f, mc(&["Mars", "Venus"], 0))), HK);
        assert_eq!(r["ok"], true, "{f:?}: {}", show(&r));
    }
}

// ---- B3 (35b): single-letter label shape ----------------------------------------------------

fn label_bank(unit: &str, n: usize, ulen: usize) -> String {
    let unit_full = unit.repeat(ulen.div_ceil(unit.len()));
    let qs: Vec<Value> = (0..n)
        .map(|_| {
            let texts: Vec<String> = (0..26).map(|k| format!("{unit_full}{}", (b'a' + k as u8) as char)).collect();
            let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
            qv("s", mc(&refs, 0))
        })
        .collect();
    bank(qs, "B", "")
}

/// 950 Mc questions x 26 choices, each choice opening with a run of single-letter labels, with the
/// author's Keep decisions: it was 10 s against 2.3 s for the same bytes of two-letter units.
#[test]
fn b3_single_letter_labels_are_linear() {
    let hostile = label_bank("x - ", 950, 110);
    let control = label_bank("xy - ", 950, 110);
    assert!(hostile.len() < 4 * 1024 * 1024, "{}", hostile.len());
    let opts = |p: &str| {
        let f = j(&label_findings_json(p.as_bytes(), ""));
        let l: Vec<Value> = f["label_findings"].as_array().unwrap().iter().map(|x| json!({"question": x["question"], "choice": "keep"})).collect();
        json!({"hide_key": true, "labels": l}).to_string()
    };
    let (oh, oc) = (opts(&hostile), opts(&control));
    let (th, tc) = (secs(&hostile, &oh), secs(&control, &oc));
    eprintln!("labels: hostile {th:.2}s, control {tc:.2}s");
    assert!(th < 12.0, "label shape took {th:.2} s");
    assert!(th < 5.0 * tc + 2.0, "label shape {th:.2} s against {tc:.2} s for the control");
    let r = pp(&hostile, &oh);
    assert!(r["ok"] == true || has(&r, "scan_budget"), "{}", show(&r));
    // The findings export is quick too (it opens a work scope for the call).
    let a = Instant::now();
    std::hint::black_box(label_findings_json(hostile.as_bytes(), ""));
    assert!(a.elapsed().as_secs_f64() < 12.0);
}

/// The kept answers are exact: different choices never share an entry, and the same choices give
/// the same verdict inside and outside a publish.
#[test]
fn b3_sibling_memo_is_exact() {
    use share_core::labels::refers_to_siblings;
    let refs = [("Both B and C", false), ("A", false), ("B", false), ("C", false)];
    let plain = [("Mars", false), ("Venus", false), ("Earth", false), ("Jupiter", false)];
    let _scope = share_core::format::WorkScope::new();
    for _ in 0..3 {
        assert!(refers_to_siblings(&refs));
        assert!(!refers_to_siblings(&plain));
    }
    // One locked flag or one letter changes the key.
    let locked = [("Both B and C", true), ("A", false), ("B", false), ("C", false)];
    let a = refers_to_siblings(&locked);
    drop(_scope);
    assert_eq!(a, refers_to_siblings(&locked));
}

// ---- B4 (35b): a numbered row under a Key heading --------------------------------------------

#[test]
fn b4_numbered_row_under_a_key_heading_is_read() {
    let paris = |stem: &str| pp(&one(qv(stem, short("Paris"))), HK);
    for stem in ["Answer key\n1. Paris\nCapital of France?", "Key\n1. Paris\nWhat is the capital of France?", "Answer key\n1. Paris", "Key:\n1) Paris\nCapital?", "Key\n(1) Paris\nCapital?", "Key\nQ1: Paris\nCapital?", "Key\na) Paris\nCapital?", "Key\n\u{2022} Paris\nCapital?", "Key\nQ1 - Paris\nCapital?", "Answer key\n1 Paris\nCapital?"] {
        let r = paris(stem);
        assert!(has(&r, "answer_line"), "{stem:?}: {}", show(&r));
    }
    let ch = ["Talc", "Quartz", "Diamond", "Gypsum"];
    let r = pp(&one(qv("Answer key\n1 Quartz\nWhich is hardest?", mc(&ch, 1))), HK);
    assert!(has(&r, "answer_line"), "Mc text row: {}", show(&r));
    // The same rows with a fixed order, the letter and the number of the correct position.
    for stem in ["Answer key\n1. B\nWhich is hardest?", "Answer key\n1) B\nWhich is hardest?", "Key\n(1) B\nWhich is hardest?", "Key\n1 B\nWhich is hardest?"] {
        let mut q = qv(stem, mc(&ch, 1));
        q["keep_order"] = json!(true);
        let r = pp(&one(q), r#"{"hide_key":true,"order_acks":[{"question":0}]}"#);
        assert!(has(&r, "answer_line"), "{stem:?}: {}", show(&r));
    }
    // A True/False value.
    let r = pp(&one(qv("Key\n1. True\nMars is a planet.", json!({"Tf": {"correct": true}}))), HK);
    assert!(has(&r, "answer_line"), "Tf row: {}", show(&r));
    // Another question's key in a bank is read the same way.
    let r = pp(&bank(vec![qv("Name it.", short("Zebra")), qv("Key\n1. Zebra\nNotes.", json!({"Tf": {"correct": true}}))], "B", ""), HK);
    assert!(has(&r, "answer_line"), "bank cross-question: {}", show(&r));
    // The key among the first words of a longer row is low risk, acknowledged by the author.
    let r = paris("Key\n1. The city of Paris on the Seine\nCapital of France?");
    assert!(has(&r, "key_heading_row") && r["errors"][0]["ack_allowed"] == true, "{}", show(&r));
    let r = pp(&one(qv("Key\n1. The city of Paris on the Seine\nCapital of France?", short("Paris"))), r#"{"hide_key":true,"ack_formatting":[0]}"#);
    assert_eq!(r["ok"], true, "acknowledged: {}", show(&r));
}

/// Round 18's dichotomous key, and honest lists under headings that merely hold the word "key".
#[test]
fn b4_honest_key_headings_publish() {
    // The couplet worksheet publishes for every position of the correct choice.
    let dk = "Use the key to identify the invertebrate.\n\nKey\n1a  Has six legs ................ go to 2\n1b  Has more than six legs ...... go to 3\n2a  Has wings ................... Fly\n2b  No wings .................... Springtail\n3a  Eight legs .................. Spider\n3b  Many legs ................... Centipede\n\nAn animal has eight legs. What is it?";
    let choices = ["Spider", "Fly", "Springtail", "Centipede"];
    for c in 0..4 {
        let r = pp(&one(qv(dk, mc(&choices, c))), HK);
        assert_eq!(r["ok"], true, "correct={c}: {}", show(&r));
    }
    // Other separators after the couplet label.
    for sep in ["1a. ", "1a) ", "1a: ", "1a  "] {
        let key = format!("Key\n{sep}Has wings ........ Fly\n1b  No wings ........ Ant\nWhat is it?");
        let r = pp(&one(qv(&key, mc(&["Fly", "Ant"], 0))), HK);
        assert_eq!(r["ok"], true, "{sep:?}: {}", show(&r));
    }
    // "Key terms", "Key points", "Key stage": the lead is not alone on its line.
    for stem in [
        "Key terms\n1. Photosynthesis uses light.\n2. Respiration releases energy.\nWhich process makes sugar?",
        "Key points\n1. Plants make sugar\n2. Animals eat it\nWhich is a producer?",
        "Key stage 3\n1. Plants\nWhich is a producer?",
        "Use the key\nto read the map.\nWhich is a producer?",
    ] {
        let r = pp(&one(qv(stem, mc(&["Plants", "Animals", "Fungi"], 0))), HK);
        assert_eq!(r["ok"], true, "{stem:?}: {}", show(&r));
    }
    // A legend under a "Key" heading whose rows do not hold this question's key.
    let r = pp(&one(qv("Key\n1. Lab\n2. Gym\n3. Hall\nWhere is the library?", mc(&["Office", "Gym", "Library"], 2))), HK);
    assert_eq!(r["ok"], true, "legend: {}", show(&r));
    let r = pp(&one(qv("Key\n1. low\n2. high\nWhat is the boiling point?", short("100 C"))), HK);
    assert_eq!(r["ok"], true, "{}", show(&r));
}
