//! Round 11 (critic22a, critic22b): the generic trailing-annotation net, scores, key texts behind
//! labels, label alphabets of other scripts, choice lists pasted into stems, letters naming a
//! position students can see, True/False edge tokens, Blank answers restated, initials, symbols
//! and their names; the Blank scanner's cost.
use serde_json::{Value, json};
use share_core::wasm::*;

fn j(s: &str) -> Value {
    serde_json::from_str(s).unwrap()
}
fn payload(stem: &str, kind: Value) -> String {
    json!({"schema_version": 2, "question": {"id":"q10000001","stem": stem,"kind": kind,"points":1.0,"difficulty":null,"topics":[],"created":0,"modified":0}}).to_string()
}
fn mc(texts: &[&str], correct: usize) -> Value {
    json!({"Mc": {"answers": texts.iter().enumerate().map(|(i, t)| json!({"text": t, "correct": i == correct, "locked": false})).collect::<Vec<_>>()}})
}
fn pp(p: &str, o: &str) -> Value {
    j(&publish_prepare_json(p.as_bytes(), o))
}
fn hard(r: &Value) -> bool {
    r["ok"] == false && r["errors"].as_array().is_some_and(|e| e.iter().any(|x| x["ack_allowed"] == false))
}
const ALL: &str = r#"{"hide_key":true,"ack_formatting":[0],"order_acks":[{"question":0}],"labels":[{"question":0,"choice":"keep"}]}"#;
const PLAIN: &str = r#"{"hide_key":true,"ack_formatting":[0],"order_acks":[{"question":0}]}"#;

#[test]
fn b1_b2_verdict_phrases_and_scores_on_the_key() {
    for k in [
        "Nitrogen - the correct answer", "Nitrogen - richtige Antwort", "Leonardo da Vinci is correct", "Nitrogen - la bonne r\u{E9}ponse", "Nitrogen - la respuesta correcta",
        "Nitrogen - a resposta certa", "Nitrogen - la risposta giusta", "Nitrogen - die richtige Antwort", "Nitrogen (die richtige)", "Nitrogen <- richtige Antwort",
        "Nitrogen is right", "Nitrogen is the correct answer", "Nitrogen \u{2013} this is correct",
        "Nitrogen (100 %)", "Nitrogen 1 mark", "Nitrogen 2 marks", "Nitrogen 1 pt", "Nitrogen 1 point", "Nitrogen +1pt", "Nitrogen %100", "Nitrogen (1 punto)",
        "Nitrogen (full marks)", "Nitrogen =100", "Nitrogen (score: 1)",
    ] {
        let r = pp(&payload("Which gas is most of the air?", mc(&[k, "Oxygen", "Argon", "Carbon dioxide"], 0)), PLAIN);
        assert!(hard(&r), "{k}: {r}");
    }
    // The net: any short trailing annotation on the key alone is at least low risk.
    let r = pp(&payload("Which gas?", mc(&["Nitrogen - most of it", "Oxygen", "Argon"], 0)), r#"{"hide_key":true}"#);
    assert_eq!(r["ok"], false, "{r}");
    // Content: a symbol and its name, keys and arrows.
    for t in [vec!["\u{2713} (check)", "\u{2717} (cross)", "\u{2192} (arrow)", "\u{2605} (star)"], vec!["Backspace (\u{2190})", "Enter", "Tab", "Shift"]] {
        assert!(!hard(&pp(&payload("Which?", mc(&t, 0)), PLAIN)), "{t:?}");
    }
}

#[test]
fn b3_b4_key_text_behind_labels_and_label_alphabets() {
    for (stem, choices, k) in [
        ("Which gas is most of the air?\nAnswer: Nitrogen", vec!["A. Oxygen", "B. Nitrogen", "C. Argon", "D. Helium"], 1),
        ("Which gas is most of the air?\nRespuesta: Nitr\u{F3}geno", vec!["a) Ox\u{ED}geno", "b) Nitr\u{F3}geno", "c) Arg\u{F3}n", "d) Helio"], 1),
        ("\u{915}\u{94C}\u{928}-\u{938}\u{940} \u{917}\u{948}\u{938}?\n\u{909}\u{924}\u{94D}\u{924}\u{930}: (\u{916})", vec!["(\u{915}) \u{911}\u{915}\u{94D}\u{938}\u{940}\u{91C}\u{928}", "(\u{916}) \u{928}\u{93E}\u{907}\u{91F}\u{94D}\u{930}\u{94B}\u{91C}\u{928}", "(\u{917}) \u{906}\u{930}\u{94D}\u{917}\u{928}", "(\u{918}) \u{939}\u{940}\u{932}\u{93F}\u{92F}\u{92E}"], 1),
        ("\u{645}\u{627} \u{627}\u{644}\u{63A}\u{627}\u{632}\u{61F}\n\u{627}\u{644}\u{625}\u{62C}\u{627}\u{628}\u{629}: \u{628}", vec!["\u{627}) \u{627}\u{644}\u{623}\u{643}\u{633}\u{62C}\u{64A}\u{646}", "\u{628}) \u{627}\u{644}\u{646}\u{64A}\u{62A}\u{631}\u{648}\u{62C}\u{64A}\u{646}", "\u{62C}) \u{627}\u{644}\u{623}\u{631}\u{63A}\u{648}\u{646}", "\u{62F}) \u{627}\u{644}\u{647}\u{64A}\u{644}\u{64A}\u{648}\u{645}"], 1),
    ] {
        for o in [r#"{"hide_key":true,"labels":[{"question":0,"choice":"strip"}]}"#, r#"{"hide_key":true,"labels":[{"question":0,"choice":"keep"}],"order_acks":[{"question":0}]}"#] {
            let r = pp(&payload(stem, mc(&choices, k)), o);
            assert!(hard(&r), "{stem}: {r}");
        }
    }
}

#[test]
fn b5_b6_choice_lists_and_letters_in_the_stem() {
    let plain = ["Oxygen", "Nitrogen", "Argon", "Helium"];
    for stem in ["Which gas?\nA. Oxygen\nB. Nitrogen \u{2713}\nC. Argon\nD. Helium", "Which gas?\na) Oxygen\n*b) Nitrogen\nc) Argon\nd) Helium", "Which gas?\nA. Oxygen\n**B. Nitrogen**\nC. Argon\nD. Helium", "Which gas?\nA. Oxygen\nB. Nitrogen (correct)\nC. Argon\nD. Helium"] {
        assert!(hard(&pp(&payload(stem, mc(&plain, 1)), PLAIN)), "{stem}");
    }
    let labelled = ["A. Oxygen", "B. Nitrogen", "C. Argon", "D. Helium"];
    for stem in ["Which gas? [B]", "Which gas? \u{2713}B", "Which gas?\nB \u{2713}", "Which gas? \u{2192} B", "Which gas? **B**", "Which gas? {B}"] {
        let r = pp(&payload(stem, mc(&labelled, 1)), ALL);
        assert!(hard(&r), "{stem}: {r}");
    }
    // Shuffled, unlabelled choices: a letter in the stem names no position students see.
    let r = pp(&payload("Which gas? [B]", mc(&plain, 1)), r#"{"hide_key":true}"#);
    assert_eq!(r["ok"], true, "{r}");
    // Kept labels are no boxes: "(1) 22/7" among "(2) √2", "(3) 3.14", "(4) π".
    let r = pp(&payload("Which is closest to pi?", mc(&["(1) 22/7", "(2) \u{221A}2", "(3) 3.14", "(4) \u{3C0}"], 0)), ALL);
    assert!(!hard(&r), "{r}");
    // A letter that is content, or names a wrong position: publishes.
    for stem in ["Which has vitamin A", "Which gas? [C]"] {
        let r = pp(&payload(stem, mc(&labelled, 1)), ALL);
        assert!(!hard(&r), "{stem}: {r}");
    }
}

#[test]
fn b7_true_false_edge_tokens() {
    let tf = |stem: &str, v: bool| payload(stem, json!({"Tf": {"correct": v}}));
    for (stem, v) in [
        ("El Sol es una estrella (V)", true), ("O Sol \u{E9} uma estrela (C)", true), ("M\u{1EB7}t Tr\u{1EDD}i l\u{E0} m\u{1ED9}t ng\u{F4}i sao (\u{110})", true), ("\u{D0DC}\u{C591}\u{C740} \u{BCC4}\u{C774}\u{B2E4} (O)", true),
        ("\u{539F}\u{5B50}\u{306F}\u{5206}\u{5272}\u{3067}\u{304D}\u{308B}\u{FF08}\u{221A}\u{FF09}", true), ("\u{0421}\u{043E}\u{043B}\u{043D}\u{0446}\u{0435} - \u{0437}\u{0432}\u{0435}\u{0437}\u{0434}\u{0430} (\u{0414}\u{0430})", true), ("G\u{FC}ne\u{15F} bir y\u{131}ld\u{131}zd\u{131}r (D)", true),
        ("The Sun is a star (T).", true), ("The Sun is a star. - T", true), ("T. The Sun is a star.", true), ("TRUE: the Sun is a star.", true), ("True - the Sun is a star.", true),
        ("The Atlantic is the largest ocean\nFalse", false), ("The Atlantic is the largest ocean (F)", false),
    ] {
        let r = pp(&tf(stem, v), r#"{"hide_key":true}"#);
        assert!(hard(&r), "{stem}: {r}");
    }
    let part = payload("Oceans.", json!({"Multipart": {"parts": [{"stem": "The Atlantic is the largest ocean (F)", "kind": {"Tf": {"correct": false}}}]}}));
    assert!(hard(&pp(&part, r#"{"hide_key":true}"#)));
    // Honest: a statement, an instruction.
    for stem in ["The Sun is a star.", "Answer true or false. The Sun is a star.", "T. S. Eliot wrote The Waste Land."] {
        assert_eq!(pp(&tf(stem, true), r#"{"hide_key":true}"#)["ok"], true, "{stem}");
    }
}

#[test]
fn b8_blank_answers_restated() {
    let blank = |stem: &str| payload(stem, json!({"Blank": {"accept": [["Nairobi"]]}}));
    for stem in ["The capital of Kenya is ____. - Nairobi", "The capital of Kenya is ____ **Nairobi**.", "The capital of Kenya is ____ *Nairobi*", "The capital of Kenya is ____ ('Nairobi')", "The capital of Kenya is ____. (Nairobi \u{2713})", "The capital of Kenya is ____.\nNairobi \u{2713}"] {
        let r = pp(&blank(stem), r#"{"hide_key":true}"#);
        assert!(hard(&r), "{stem}: {r}");
    }
    for stem in ["Word bank: Mombasa, Nairobi, Kisumu.\nThe capital of Kenya is ____.", "The capital of Kenya is ____. Mombasa is its port."] {
        assert_eq!(pp(&blank(stem), r#"{"hide_key":true}"#)["ok"], true, "{stem}");
    }
}

#[test]
fn initials_and_genus_abbreviations_need_no_decision() {
    for t in [["B. F. Skinner", "C. S. Lewis", "A. P. J. Abdul Kalam", "J. K. Rowling"], ["E. coli", "S. aureus", "B. subtilis", "C. albicans"]] {
        let r = pp(&payload("Which?", mc(&t, 0)), r#"{"hide_key":true}"#);
        assert_eq!(r["ok"], true, "{t:?}: {r}");
        assert!(r["stripped"]["order_reasons"].as_array().unwrap().is_empty());
    }
}

/// critic22b: "___ " + "a " x 8 KB with accept "a" was quadratic in the Blank scanner (14.7 s for
/// 400 questions in wasm). The cost of one stem grows linearly with its length.
#[test]
fn blank_scan_is_linear() {
    let stem = |n: usize| format!("___ {}", "a ".repeat(n));
    let key = share_core::format::KeyInfo { texts: vec!["a".into()], blank: true, ..Default::default() }.folded();
    let time = |s: &str| (0..5).map(|_| { let t = std::time::Instant::now(); for _ in 0..20 { std::hint::black_box(share_core::format::stem_key(s, &key)); } t.elapsed().as_secs_f64() }).fold(f64::MAX, f64::min);
    let (a, b) = (stem(1000), stem(4000));
    let (ta, tb) = (time(&a), time(&b));
    assert!(tb / ta < 8.0 || tb < 0.002 || std::env::var("SHARE_CORE_MUTATION").is_ok(), "{ta:.4} -> {tb:.4}");
}
