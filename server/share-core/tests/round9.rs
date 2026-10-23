//! Round 9: `served_text` is idempotent and the author's and served hashes agree for any input;
//! the key-in-stem scanner reads the served, folded stem (spaces, dashes, invisible characters,
//! fullwidth forms, look-alikes, qualifiers, decorated letters, GIFT and Cloze variants).
use serde_json::{Value, json};
use share_core::format::served_text;
use share_core::wasm::*;

fn j(s: &str) -> Value {
    serde_json::from_str(s).unwrap()
}
fn q(stem: &str, kind: Value) -> String {
    json!({"schema_version": 2, "question": {"id":"q1","stem": stem,"kind": kind,"points":1.0,"difficulty":null,"created":0,"modified":0}}).to_string()
}
fn mc_kind(texts: &[&str], correct: usize) -> Value {
    json!({"Mc": {"answers": texts.iter().enumerate().map(|(i, t)| json!({"text": t, "correct": i == correct, "locked": false})).collect::<Vec<_>>()}})
}
fn pp(p: &str) -> Value {
    j(&publish_prepare_json(p.as_bytes(), r#"{"hide_key":true}"#))
}
fn kind_of(k: &str) -> Value {
    match k {
        "MC" => mc_kind(&["Venus", "Mars", "Earth", "Jupiter"], 1),
        "TF" => json!({"Tf": {"correct": true}}),
        _ => json!({"Short": {"sample": "Paris"}}),
    }
}

/// xorshift, fixed seed.
struct Rnd(u64);
impl Rnd {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}
/// Joiners, invisible characters, combining marks and letters of joining and Indic scripts,
/// Hangul jamo, Tibetan, Balinese, Chakma, Kaithi, Myanmar, emoji, spaces.
const ALPHA: &str = "a e\u{301}\u{300}\u{327}\u{308}\u{34F}\u{AD}\u{200B}\u{2060}\u{FEFF}\u{200C}\u{200D}\u{627}\u{648}\u{64A}\u{6D5}\u{6D2}\u{653}\u{654}\u{655}\u{628}\u{915}\u{93C}\u{94D}\u{B47}\u{B3E}\u{1F468}\u{1F469}\u{FE0F}\u{2764}\t\u{A0}\u{3000}\u{1100}\u{1161}\u{11A8}\u{AC00}\u{F71}\u{F72}\u{F80}\u{1B05}\u{1B35}\u{11131}\u{11127}\u{110A5}\u{110BA}\u{1025}\u{102E}\u{1026}\u{644}\u{670}\u{6CC}\u{200E}\n";

#[test]
fn served_text_is_idempotent() {
    let alpha: Vec<char> = ALPHA.chars().collect();
    let mut r = Rnd(0x1234_5678_9abc_def1);
    let mut bad = Vec::new();
    for _ in 0..400_000 {
        let n = 2 + (r.next() % 9) as usize;
        let s: String = (0..n).map(|_| alpha[(r.next() % alpha.len() as u64) as usize]).collect();
        let Ok(v) = share_core::text::clean(&s, share_core::text::Class::Block, share_core::text::Max::Bytes(1 << 16)) else { continue };
        let a = served_text(&v);
        if served_text(&a) != a {
            bad.push(v.escape_unicode().to_string());
            if bad.len() > 5 {
                break;
            }
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
    // The critic's case: alef + CGJ + madda + ZWNJ: NFC makes U+0622, which never joins left.
    assert_eq!(served_text("\u{627}\u{34F}\u{653}\u{200C}\u{628}\u{627}\u{628}"), "\u{622}\u{628}\u{627}\u{628}");
    // Decomposed alef + madda is U+0622 once composed: the ZWNJ after it goes too.
    assert_eq!(served_text("\u{627}\u{653}\u{200C}\u{628}"), "\u{622}\u{628}");
}

#[test]
fn author_and_served_match_hashes_agree() {
    let alpha: Vec<char> = ALPHA.chars().filter(|c| *c != '\n').collect();
    let mut r = Rnd(0x0dd_ba11_cafe_f00d);
    let mut checked = 0;
    for _ in 0..3000 {
        let word = |r: &mut Rnd| -> String { let n = 2 + (r.next() % 5) as usize; (0..n).map(|_| alpha[(r.next() % alpha.len() as u64) as usize]).collect() };
        let texts: Vec<String> = (0..4).map(|k| format!("w{k}{}", word(&mut r))).collect();
        let stem = format!("s{}", word(&mut r));
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        let p = q(&stem, mc_kind(&refs, 0));
        let o = j(&publish_prepare_json(p.as_bytes(), r#"{"hide_key":true,"ack_formatting":[0],"order_acks":[{"question":0}]}"#));
        let _ = strip_author_data_json();
        if o["ok"] != true {
            continue;
        }
        checked += 1;
        assert_eq!(o["match_hashes"][0], o["stripped"]["match_hashes"][0], "{}", p.escape_unicode());
        let again = j(&match_hash_json(o["stripped"]["payload"].to_string().as_bytes(), ""));
        assert_eq!(again["hashes"][0], o["match_hashes"][0], "{}", p.escape_unicode());
    }
    assert!(checked > 500, "{checked}");
}

#[test]
fn the_critics_zwnj_key_is_not_served() {
    let p = q("Which?", mc_kind(&["\u{627}\u{34F}\u{653}\u{200C}\u{628}\u{627}\u{628}", "\u{622}\u{628}\u{627}\u{628}", "\u{622}\u{628}\u{627}\u{628} \u{628}", "\u{622}\u{628}\u{627}\u{628} \u{627}"], 0));
    let r = pp(&p);
    // Served, the key is the same word as a wrong choice: refused as a duplicate.
    assert_eq!(r["errors"][0]["code"], "duplicate_choice", "{r}");
}

/// critic20b p1_stem.mjs and p2_stem_inv.mjs: every line is refused with key_in_stem.
#[test]
fn key_in_stem_forms_are_refused() {
    let cases: &[(&str, &str, &str)] = &[
        ("es Respuesta correcta: B", "MC", "\u{00bf}Qu\u{00e9} planeta es rojo?\nRespuesta correcta: B"),
        ("pt Resposta correta: B", "MC", "Qual planeta \u{00e9} vermelho?\nResposta correta: B"),
        ("it Risposta corretta: B", "MC", "Quale pianeta \u{00e8} rosso?\nRisposta corretta: B"),
        ("fr Bonne r\u{00e9}ponse : B", "MC", "Quelle plan\u{00e8}te est rouge ?\nBonne r\u{00e9}ponse : B"),
        ("fr R\u{00e9}ponse correcte : B", "MC", "Quelle plan\u{00e8}te est rouge ?\nR\u{00e9}ponse correcte : B"),
        ("de Richtige Antwort: B", "MC", "Welcher Planet ist rot?\nRichtige Antwort: B"),
        ("de L\u{00f6}sung: B", "MC", "Welcher Planet ist rot? L\u{00f6}sung: B"),
        ("en The answer is B", "MC", "Which planet is red? (The answer is B)"),
        ("en Answer -> B (arrow)", "MC", "Which planet is red? Answer \u{2192} B"),
        ("en Answer:\\\\nB", "MC", "Which planet is red?\nAnswer:\nB"),
        ("en ANSWER fullwidth", "MC", "Which planet is red?\n\u{ff21}\u{ff2e}\u{ff33}\u{ff37}\u{ff25}\u{ff32}\u{ff1a} B"),
        ("en Answer: Mars", "MC", "Which planet is red? Answer: Mars"),
        ("en Answer: mars.", "MC", "Which planet is red? Answer:mars."),
        ("en Correct: Mars", "MC", "Which planet is red? [Correct: Mars]"),
        ("en Key - B", "MC", "Which planet is red? Key - B"),
        ("en (Ans: 2)", "MC", "Which planet is red? (Ans: 2)"),
        ("en Answer: (b)", "MC", "Which planet is red? Answer: (b)"),
        ("en Answer: Option B", "MC", "Which planet is red? Answer: Option B"),
        ("en Answer: B)", "MC", "Which planet is red? Answer: B) Mars"),
        ("en Answer: \u{2018}B\u{2019}", "MC", "Which planet is red? Answer: \u{2018}B\u{2019}"),
        ("en Answer: \\\"B\\\"", "MC", "Which planet is red? Answer: \"B\""),
        ("en Answer: **B**", "MC", "Which planet is red? Answer: **B**"),
        ("ru \u{041e}\u{0442}\u{0432}\u{0435}\u{0442} \u{2014} \u{0411}", "MC", "\u{041a}\u{0430}\u{043a}\u{0430}\u{044f} \u{043f}\u{043b}\u{0430}\u{043d}\u{0435}\u{0442}\u{0430} \u{043a}\u{0440}\u{0430}\u{0441}\u{043d}\u{0430}\u{044f}? \u{041e}\u{0442}\u{0432}\u{0435}\u{0442} \u{2014} \u{0411}"),
        ("ja \u{6b63}\u{89e3}\u{306f} B", "MC", "\u{8d64}\u{3044}\u{60d1}\u{661f}\u{306f}\u{ff1f} \u{6b63}\u{89e3}\u{306f}B"),
        ("zh \u{7b54}\u{6848}\u{662f}B", "MC", "\u{54ea}\u{9897}\u{884c}\u{661f}\u{662f}\u{7ea2}\u{8272}\u{7684}\u{ff1f}\u{7b54}\u{6848}\u{662f}B"),
        ("zh \u{7b54}\u{6848}\u{ff1a}B", "MC", "\u{54ea}\u{9897}\u{884c}\u{661f}\u{662f}\u{7ea2}\u{8272}\u{7684}\u{ff1f}\u{7b54}\u{6848}\u{ff1a}B"),
        ("zh \u{3010}\u{7b54}\u{6848}\u{3011}B", "MC", "\u{54ea}\u{9897}\u{884c}\u{661f}\u{662f}\u{7ea2}\u{8272}\u{7684}\u{ff1f}\u{3010}\u{7b54}\u{6848}\u{3011}B"),
        ("ko \u{c815}\u{b2f5}: 2", "MC", "\u{be68}\u{ac04} \u{d589}\u{c131}\u{c740}? \u{c815}\u{b2f5}: 2"),
        ("nl Juiste antwoord: B", "MC", "Welke planeet is rood? Juiste antwoord: B"),
        ("pl Poprawna odpowied\u{017a}: B", "MC", "Kt\u{00f3}ra planeta jest czerwona? Poprawna odpowied\u{017a}: B"),
        ("GIFT {=Mars ~Venus}", "MC", "Which planet is red {=Mars ~Venus}"),
        ("GIFT no space", "MC", "Which planet is red{=Mars ~Venus}"),
        ("GIFT after quote", "MC", "Which planet is 'red'{=Mars ~Venus}"),
        ("GIFT after ? ", "MC", "Which planet is red?{=Mars ~Venus ~Earth}"),
        ("GIFT long head", "MC", "Which planet is red? {                         =Mars ~Venus}"),
        ("TF {TRUE}", "TF", "The Sun is a star {TRUE}"),
        ("TF {True}", "TF", "The Sun is a star {True}"),
        ("TF {T} nospace", "TF", "The Sun is a star{T}"),
        ("TF (True)", "TF", "The Sun is a star (True)"),
        ("TF (Verdadero)", "TF", "El Sol es una estrella (Verdadero)"),
        ("TF (Richtig)", "TF", "Die Sonne ist ein Stern (Richtig)"),
        ("TF (correct)", "TF", "The Sun is a star (correct)"),
        ("TF - True", "TF", "The Sun is a star - True"),
        ("TF Answer: True", "TF", "The Sun is a star. Answer: True"),
        ("TF Answer: Verdadero", "TF", "El Sol es una estrella. Respuesta: Verdadero"),
        ("TF Answer: T.", "TF", "The Sun is a star. Ans. T"),
        ("TF Answer: Vrai fr", "TF", "Le Soleil est une \u{00e9}toile. R\u{00e9}ponse : vrai"),
        ("TF (\u{2714})", "TF", "The Sun is a star (\u{2714})"),
        ("TF (TRUE) fullwidth", "TF", "The Sun is a star \u{ff08}\u{ff34}\u{ff32}\u{ff35}\u{ff25}\u{ff09}"),
        ("TF (Correcto)", "TF", "El Sol es una estrella (Cierto)"),
        ("TF {#1}", "TF", "The Sun is a star {#1}"),
        ("Short Answer: Paris", "SH", "Capital of France? Answer: Paris"),
        ("Short Answer: PARIS", "SH", "Capital of France? Answer: PARIS"),
        ("Short Answer: Paris, France", "SH", "Capital of France? Answer: Paris, France"),
        ("Short Answer: the city of Paris", "SH", "Capital of France? Answer: the city of Paris"),
        ("Short Answer = Pa\u{00ad}ris shy", "SH", "Capital of France? Answer: Pa\u{00ad}ris"),
        ("Short (Paris)", "SH", "Capital of France? (Paris)"),
        ("Short Cloze lower", "SH", "Capital of France? {1:shortanswer:=Paris}"),
        ("Short Cloze SA", "SH", "Capital of France? {1:SA:=Paris}"),
        ("Short Cloze ws", "SH", "Capital of France? { 1 :SA:=Paris}"),
        ("Short Cloze  :NUMERICAL", "SH", "Value? {2:NUMERICAL:=3.14:0.01}"),
        ("Short {#3.14}", "SH", "Value of pi? {#3.14}"),
        ("Short {#\u{03c0}}", "SH", "Value of pi? {#=3.14}"),
        ("fr NBSP before colon", "MC", "Quelle plan\u{00e8}te est rouge ?\nR\u{00e9}ponse\u{00a0}: B"),
        ("fr NNBSP before colon", "MC", "Quelle plan\u{00e8}te est rouge ?\nR\u{00e9}ponse\u{202f}: B"),
        ("tab before colon", "MC", "Which planet is red?\nAnswer\t: B"),
        ("soft hyphen in lead", "MC", "Which planet is red?\nAn\u{00ad}swer: B"),
        ("ZWSP in lead", "MC", "Which planet is red?\nAns\u{200b}wer: B"),
        ("word joiner after lead", "MC", "Which planet is red?\nAnswer\u{2060}: B"),
        ("ZWNJ after colon", "MC", "Which planet is red?\nAnswer:\u{200c}B"),
        ("fullwidth colon + NBSP", "MC", "Which planet is red?\nAnswer\u{ff1a}\u{00a0}B"),
        ("ideographic space", "MC", "Which planet is red?\nAnswer\u{3000}:\u{3000}B"),
        ("Cyrillic \u{0430} in Answer", "MC", "Which planet is red?\n\u{0410}nswer: B"),
        ("combining in lead", "MC", "Which planet is red?\nAnswe\u{0301}r: B"),
        ("gift ZWSP {", "MC", "Which planet is red \u{200b}{=Mars ~Venus}"),
        ("gift soft-hyphen =", "MC", "Which planet is red {\u{00ad}=Mars ~Venus}"),
        ("TF {TR\u{00ad}UE}", "TF", "The Sun is a star {TR\u{00ad}UE}"),
        ("TF (Tr\u{200b}ue)", "TF", "The Sun is a star (Tr\u{200b}ue)"),
        ("TF (true) NBSP", "TF", "The Sun is a star (\u{00a0}true\u{00a0})"),
    ];
    let mut published = Vec::new();
    for (name, k, stem) in cases {
        let r = pp(&q(stem, kind_of(k)));
        let hit = r["errors"].as_array().is_some_and(|e| e.iter().any(|x| x["code"] == "key_in_stem" && x["ack_allowed"] == false));
        if !hit {
            published.push(format!("{name}: {r}"));
        }
    }
    assert!(published.is_empty(), "{}", published.join("\n"));
}

#[test]
fn honest_stems_publish() {
    for (stem, k) in [
        ("Which planet is red?", "MC"),
        ("Which describes the solution of x \u{2212} 3 > 2?", "MC"),
        ("Write your answer: in a full sentence.", "SH"),
        ("Answer the following question: which planet is red?", "MC"),
        ("Key terms: Venus, Mars. Which planet is red?", "MC"),
        (r"Compute $A^{T}$ and $\mathbf{F}$ for the matrix.", "MC"),
        ("The answer must be given in metres.", "SH"),
        ("Quelle plan\u{E8}te est rouge\u{A0}?", "MC"),
        ("R\u{E9}pondez\u{A0}: quelle plan\u{E8}te est rouge\u{A0}?", "MC"),
        ("The Sun is a star.", "TF"),
        // (Round 11: a capital letter in brackets ending a True/False statement is a value marker:
        // "(T)" is refused now, by the owner's capped rule.)
        ("Find the set {x | x > 2}.", "MC"),
        ("In the answer key, every item is worth one point. Which planet is red?", "MC"),
        ("Le Soleil est une \u{E9}toile\u{202F}!", "TF"),
        ("\u{7B54}\u{6848}\u{8BF7}\u{5199}\u{5728}\u{7EB8}\u{4E0A}\u{3002}\u{54EA}\u{9897}\u{884C}\u{661F}\u{662F}\u{7EA2}\u{8272}\u{7684}\u{FF1F}", "MC"),
    ] {
        let r = pp(&q(stem, kind_of(k)));
        assert_eq!(r["ok"], true, "{stem}: {r}");
    }
}

#[test]
fn served_stems_topics_and_names_are_normalised() {
    let p = json!({"schema_version": 2, "bank": {"id":"b","name":"Unit 3\u{A0}\u{A0}planets","created":1,"modified":2,"details":{"description":"Line\u{200B} one"},"questions":[{"id":"q1","stem":"Which\u{AD} planet\t is  red?","kind": mc_kind(&["Venus", "Mars"], 1),"points":null,"difficulty":null,"created":1,"modified":1,"topics":["Solar \u{A0} system"]}]}}).to_string();
    let r = pp(&p);
    assert_eq!(r["ok"], true, "{r}");
    let b = &r["stripped"]["payload"]["bank"];
    assert_eq!(b["name"], "Unit 3 planets");
    assert_eq!(b["details"]["description"], "Line one");
    assert_eq!(b["questions"][0]["stem"], "Which planet is red?");
    assert_eq!(b["questions"][0]["topics"][0], "Solar system");
    // Answer syntax in the bank name or a topic: low risk.
    let p2 = json!({"schema_version": 2, "bank": {"id":"b","name":"Quiz {=Mars ~Venus}","created":1,"modified":2,"questions":[{"id":"q1","stem":"Which?","kind": mc_kind(&["Venus", "Mars"], 1),"points":null,"difficulty":null,"created":1,"modified":1,"topics":["Answer: B"]}]}}).to_string();
    let r = pp(&p2);
    let e = r["errors"].as_array().unwrap();
    // Round 9 addendum (critic20a): high risk, like a stem.
    assert!(e.iter().any(|x| x["path"] == "bank.name" && x["ack_allowed"] == false), "{r}");
    assert!(e.iter().any(|x| x["path"] == "bank.questions[0].topics[0]" && x["ack_allowed"] == false), "{r}");
}

#[test]
fn stem_key_is_linear() {
    use share_core::format::{KeyInfo, stem_key};
    let key = KeyInfo { texts: vec!["mars".into()], choices: true, tf: true, ..Default::default() };
    let s = |n: usize| format!("{}{}{}", "answer correct x ".repeat(n), "{x ".repeat(n), "\u{7B54}\u{6848}x ".repeat(n));
    let time = |t: &str| (0..5).map(|_| { let a = std::time::Instant::now(); for _ in 0..20 { std::hint::black_box(stem_key(t, &key)); } a.elapsed().as_secs_f64() }).fold(f64::MAX, f64::min);
    let (a, b) = (s(400), s(1600));
    let (ta, tb) = (time(&a), time(&b));
    assert!(tb / ta < 8.0 || tb < 0.005 || std::env::var("SHARE_CORE_MUTATION").is_ok(), "{ta:.4} -> {tb:.4}");
}

// ---- Round 9 addendum (critic20a) -------------------------------------------------------------------

#[derive(Debug, PartialEq)]
enum Out {
    Served,
    Acked,
    Refused(Vec<String>),
}

/// The critic's honest author through `publish_prepare`: label decisions, order acks, low-risk
/// formatting acks.
fn drive(qv: &Value, i: usize) -> (Out, Value) {
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
            return (if acked { Out::Acked } else { Out::Served }, r);
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
        if !acked && e.iter().any(|x| x["code"] == "key_in_formatting" && x["ack_allowed"] == true) {
            o["ack_formatting"] = json!([0]);
            acked = true;
            changed = true;
        }
        if !changed {
            return (Out::Refused(e.iter().map(|x| format!("{}/{}", x["code"].as_str().unwrap(), x["reason"].as_str().unwrap_or(""))).collect()), r);
        }
    }
    (Out::Refused(vec!["loop".into()]), Value::Null)
}

fn corpus(name: &str) -> Vec<Value> {
    serde_json::from_str(&std::fs::read_to_string(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()).unwrap()
}

/// 595 honest items in 55 languages: no refusal but the one genuinely cued Moodle fixture
/// ("wrong answer" / "right answer"), formatting acknowledgements on at most 2%.
#[test]
fn critic20a_honest_corpus() {
    let items = corpus("critic20a_honest.json");
    let (mut refused, mut fmt_acks) = (Vec::new(), 0);
    for (i, it) in items.iter().enumerate() {
        let (out, _) = drive(&it["q"], i);
        match out {
            Out::Refused(e) if it["id"] != "h0551" => refused.push(format!("{} {e:?}", it["id"])),
            Out::Acked => fmt_acks += 1,
            _ => {}
        }
    }
    assert!(refused.is_empty(), "{}", refused.join("\n"));
    assert!(fmt_acks * 50 <= items.len(), "{fmt_acks} of {}", items.len());
}

/// 291 marked keys and 20 raw Moodle/GIFT fixtures: none served as it was. The only ones served
/// are invisible padding the served form removes (the served key is the plain word).
#[test]
fn critic20a_markers_and_raw_fixtures() {
    let mut accepted = Vec::new();
    for (k, file) in ["critic20a_markers.json", "critic20a_raw.json"].iter().enumerate() {
        for (i, it) in corpus(file).iter().enumerate() {
            if it["expect"] != "refuse" {
                continue;
            }
            let (out, r) = drive(&it["q"], 10_000 * k + i);
            if out != Out::Served {
                continue;
            }
            let name = it["name"].as_str().unwrap_or("");
            let invisible = ["nbsp_padded", "ideographic_space_end", "zwsp_between_letters", "word_joiner_end"].iter().any(|p| name.starts_with(p));
            let plain = r["stripped"]["payload"]["question"]["kind"]["Mc"]["answers"].as_array().is_some_and(|a| a.iter().all(|x| x["text"].as_str().unwrap().chars().all(|c| c.is_alphanumeric() || c == ' ')));
            // Documented limits (owner decision, round 9): not promised.
            let limit = ["letter_spaced", "combining_dot_below", "trailing_ellipsis", "smiley", "same_first_letter_pairs"].iter().any(|p| name.starts_with(p));
            if !(invisible && plain) && !limit {
                accepted.push(format!("{} {}", it["id"], name));
            }
        }
    }
    assert!(accepted.is_empty(), "{}", accepted.join("\n"));
}

#[test]
fn critic20a_stem_topic_and_freeze_cases() {
    let ke = |stem: &str| q(stem, mc_kind(&["Nairobi", "Mombasa", "Kisumu", "Nakuru"], 0));
    for p in [
        q("Moodle's abilities include handling user input that includes images:\nA) True\nB) False\nANSWER: A", json!({"Tf": {"correct": true}})),
        q("Moodle supports images. \u{2014} True", json!({"Tf": {"correct": true}})),
        q("Moodle is written in Java. (F)", json!({"Tf": {"correct": false}})),
        q("The Sun orbits the Earth. \u{2717}", json!({"Tf": {"correct": false}})),
        q("The chemical symbol for gold is ____ (Au).", json!({"Blank": {"accept": [["Au"]]}})),
        q("Gold \u{2192} Au: ____", json!({"Blank": {"accept": [["Au"]]}})),
        ke("Which city is the capital of Kenya?\n\u{7B54}\u{FF1A}A"),
        ke("Which city is the capital of Kenya?\n\u{412}\u{456}\u{434}\u{43F}\u{43E}\u{432}\u{456}\u{434}\u{44C}: A"),
        ke("Which city is the capital of Kenya? (Nairobi is correct)"),
        ke("Which city is the capital of Kenya? (A is correct)"),
        ke("Which city is the capital of Kenya?\nNairobi"),
        q("Which city is the capital of Kenya?", json!({"Mc": {"answers": [{"text": "Nairobi", "correct": true, "locked": false}, {"text": "Mombasa", "correct": false, "locked": false}]}, "_": 0})),
    ] {
        let mut v = j(&p);
        if v["question"]["kind"]["_"] == 0 {
            v["question"]["kind"].as_object_mut().unwrap().remove("_");
            v["question"]["topics"] = json!(["Nairobi"]);
        }
        let r = pp(&v.to_string());
        assert!(r["errors"].as_array().is_some_and(|e| e.iter().any(|x| x["code"] == "key_in_stem" && x["ack_allowed"] == false)), "{p}: {r}");
    }
    // Letters named by a noun are content, not references: these shuffle.
    use share_core::labels::refers_to_siblings;
    let r = |t: &[&str]| refers_to_siblings(&t.iter().map(|x| (*x, false)).collect::<Vec<_>>());
    assert!(!r(&["Type A", "Type B", "Types A and B equally", "Type D"]));
    assert!(!r(&["C18", "Column A and C", "Silica", "Cyano"]));
    assert!(r(&["Mitochondria", "Ribosome", "Both A and B", "Nucleus"]));
    // A localized correct "All of the above", locked: a reference, not a locked key.
    for all in ["Alle de ovennevnte", "Toate cele de mai sus", "\u{E16}\u{E39}\u{E01}\u{E17}\u{E38}\u{E01}\u{E02}\u{E49}\u{E2D}", "Visi i\u{161}vardyti"] {
        let p = q("Which are mammals?", json!({"Mc": {"answers": [{"text": "Dog", "correct": false, "locked": false}, {"text": "Cat", "correct": false, "locked": false}, {"text": "Whale", "correct": false, "locked": false}, {"text": all, "correct": true, "locked": true}]}}));
        assert_eq!(pp(&p)["ok"], true, "{all}");
    }
}

#[test]
fn critic20a_matching_tags_and_honest_lookalikes() {
    let mt = |pairs: &[(&str, &str)]| q("Match each country with its capital.", json!({"Matching": {"pairs": pairs.iter().map(|(l, r)| json!({"left": l, "right": r})).collect::<Vec<_>>(), "distractors": ["Lagos"]}}));
    let hard = |r: &Value| r["errors"].as_array().is_some_and(|e| e.iter().any(|x| x["ack_allowed"] == false));
    for p in [
        mt(&[("Kenya a", "Nairobi a"), ("Peru b", "Lima b"), ("Japan c", "Tokyo c"), ("Egypt d", "Cairo d")]),
        mt(&[("Kenya vi", "Nairobi vi"), ("Peru ii", "Lima ii"), ("Japan iii", "Tokyo iii"), ("Egypt iv", "Cairo iv")]),
        mt(&[("Nairobi is in Kenya", "Nairobi"), ("Lima is in Peru", "Lima"), ("Tokyo is in Japan", "Tokyo"), ("Cairo is in Egypt", "Cairo")]),
    ] {
        let r = pp(&p);
        assert!(hard(&r), "{p}: {r}");
    }
    // Honest: grades, blood-group genes.
    // (A grade star is low risk: acknowledged, never a hard refusal.)
    let (o, r) = drive(&json!({"stem": "Top grade?", "kind": mc_kind(&["A**", "A", "B", "C"], 0)}), 0);
    assert_eq!(o, Out::Acked, "{r}");
    let (o, r) = drive(&json!({"stem": "Genotype for blood group AB?", "kind": mc_kind(&["A and B", "A and O", "B and O", "O only"], 0)}), 1);
    assert_eq!(o, Out::Served, "{r}");
}
