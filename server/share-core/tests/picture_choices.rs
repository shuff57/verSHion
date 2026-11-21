//! Picture choices: an Mc answer (or an Mc part's) with an `image`, with or without text, and the `figure` that drew
//! it. The same path as a question's image and figure: the wire field, validation with the same limits, the strip's
//! `image_ok` tick, the keyless rule, the canonical writer and the hashes. A text choice is written exactly as before.
mod common;
use common::*;
use share_core::*;

fn img(c: char, w: u32, h: u32) -> ImageRef {
    ImageRef { file: format!("media/{}.png", c.to_string().repeat(64)), width: w, height: h }
}

fn fig() -> FigureSpec {
    let mut params = std::collections::BTreeMap::new();
    params.insert("slices".to_string(), serde_json::json!(4));
    params.insert("label".to_string(), serde_json::json!("B"));
    FigureSpec { id: "pie-chart".into(), params }
}

/// A picture-only choice, a picture choice with text and its figure, and a text choice.
fn pictures_mc() -> Question {
    question(
        "Which chart shows one quarter shaded?",
        Kind::Mc {
            answers: vec![
                Answer { image: Some(img('a', 200, 200)), ..answer("", false) },
                Answer { image: Some(img('b', 240, 180)), figure: Some(fig()), ..answer("Chart B", true) },
                answer("None of these", false),
            ],
        },
    )
}

fn pictures_multipart() -> Question {
    let mut q = question(
        "Look at the charts.",
        Kind::Multipart {
            parts: vec![
                Part { stem: "Which is a quarter?".into(), kind: PartKind::Mc { answers: vec![Answer { image: Some(img('c', 10, 10)), ..answer("", true) }, Answer { image: Some(img('d', 10, 10)), ..answer("", false) }] } },
                Part { stem: "Why?".into(), kind: PartKind::Short { sample: "One of four.".into() } },
            ],
        },
    );
    q.id = "q2".into();
    q
}

fn codes_of(e: Vec<ValidationError>) -> Vec<(&'static str, String)> {
    e.into_iter().map(|e| (e.code, e.path)).collect()
}

#[test]
fn picture_choices_round_trip_through_wire_and_validate() {
    for q in [pictures_mc(), pictures_multipart()] {
        for schema in [1, 2] {
            let json = Payload { schema_version: schema, body: Body::Question(q.clone()) }.to_json();
            let v = ok(&json);
            assert!(v.dropped.is_empty(), "nothing dropped: {:?}", v.dropped);
            assert_eq!(only_question(&v.payload), &q, "schema {schema}: every picture and figure kept");
            // The canonical writer agrees with serde's tree, and validation is idempotent on its own output.
            assert_eq!(v.payload.to_json(), canonical_json(&v.payload.to_value()));
            assert_eq!(ok(&v.payload.to_json()).payload, v.payload);
            // The picture keys are on the wire (serde) exactly where the app's model writes them.
            let wire = serde_json::to_value(only_question(&v.payload)).unwrap();
            let first = match &q.kind {
                Kind::Mc { .. } => &wire["kind"]["Mc"]["answers"][0],
                _ => &wire["kind"]["Multipart"]["parts"][0]["kind"]["Mc"]["answers"][0],
            };
            assert!(first["image"]["file"].as_str().is_some_and(|f| f.starts_with("media/")));
            assert_eq!(first["text"], "");
        }
    }
    // In a bank too.
    let bank = QuestionBank { id: "b1".into(), name: "Charts".into(), questions: vec![pictures_mc(), pictures_multipart()], created: 1, modified: 2, details: Default::default(), origin: None, published: None };
    let v = ok(&Payload { schema_version: 2, body: Body::Bank(bank.clone()) }.to_json());
    assert_eq!(v.payload.body, Body::Bank(bank));
}

#[test]
fn a_text_choice_is_written_exactly_as_before() {
    let a = Answer::new("Mercury", true);
    assert_eq!(serde_json::to_string(&a).unwrap(), r#"{"text":"Mercury","correct":true,"locked":false}"#);
    let q = question("Closest planet?", Kind::Mc { answers: vec![answer("Mercury", true), answer("Venus", false)] });
    let json = q_payload_v1(&q);
    assert!(!json.contains("image\":{") && !json.contains("\"figure\":{"), "{json}");
    assert!(json.contains(r#"{"answers":[{"correct":true,"locked":false,"text":"Mercury"},{"correct":false,"locked":false,"text":"Venus"}]}"#), "{json}");
    // An old reader's input (no picture keys) is a text choice.
    let v = ok(&json);
    let Kind::Mc { answers } = &only_question(&v.payload).kind else { panic!() };
    assert!(answers.iter().all(|a| a.image.is_none() && a.figure.is_none()));
}

#[test]
fn a_bad_choice_picture_is_refused_with_its_exact_path() {
    let set = |q: &mut Question, i: usize, f: &dyn Fn(&mut Answer)| {
        let Kind::Mc { answers } = &mut q.kind else { panic!() };
        f(&mut answers[i]);
    };
    // A file name the store does not use.
    let mut q = pictures_mc();
    set(&mut q, 1, &|a| a.image.as_mut().unwrap().file = "../etc/passwd".into());
    assert_eq!(codes(&q_payload(&q)), [("bad_image_file", "question.kind.Mc.answers[1].image.file".to_string())]);
    // A side over the question image's limit (4096 px), and a zero side.
    let mut q = pictures_mc();
    set(&mut q, 2, &|a| a.image = Some(img('e', 5000, 0)));
    let e = errs(&q_payload(&q));
    assert_eq!(codes_of(e.clone()), [("out_of_range", "question.kind.Mc.answers[2].image.width".to_string()), ("out_of_range", "question.kind.Mc.answers[2].image.height".to_string())]);
    assert_eq!(e[0].limit, Some(Limits::default().image_side as u64));
    // The same limit as a question image, whatever it is set to.
    let mut q = pictures_mc();
    set(&mut q, 0, &|a| a.image = Some(img('a', 300, 300)));
    let lim = Limits { image_side: 256, ..Limits::default() };
    let e = validate(&q_payload(&q), &lim, &Structural).unwrap_err();
    assert_eq!(codes_of(e), [("out_of_range", "question.kind.Mc.answers[0].image.width".to_string()), ("out_of_range", "question.kind.Mc.answers[0].image.height".to_string())]);
    let mut qi = question("s", Kind::Short { sample: "x".into() });
    qi.image = Some(img('a', 300, 300));
    assert_eq!(validate(&q_payload(&qi), &lim, &Structural).unwrap_err().len(), 2, "the question's own image has the same limit");
    // A figure id that is not a registry slug.
    let mut q = pictures_mc();
    set(&mut q, 1, &|a| a.figure.as_mut().unwrap().id = "Not A Figure!".into());
    assert_eq!(codes(&q_payload(&q)), [("bad_figure_id", "question.kind.Mc.answers[1].figure.id".to_string())]);
    // Without a picture a choice still needs a visible text.
    let mut q = pictures_mc();
    set(&mut q, 0, &|a| a.image = None);
    assert_eq!(codes(&q_payload(&q)), [("blank_text", "question.kind.Mc.answers[0].text".to_string())]);
    // A wrongly shaped picture in a Multipart part of a bank.
    let mut mp = pictures_multipart();
    let Kind::Multipart { parts } = &mut mp.kind else { panic!() };
    let PartKind::Mc { answers } = &mut parts[0].kind else { panic!() };
    answers[1].image.as_mut().unwrap().file = "media/short.png".into();
    let bank = QuestionBank { id: "b1".into(), name: "Charts".into(), questions: vec![pictures_mc(), mp], created: 1, modified: 2, details: Default::default(), origin: None, published: None };
    let json = Payload { schema_version: 2, body: Body::Bank(bank) }.to_json();
    assert_eq!(codes(&json), [("bad_image_file", "bank.questions[1].kind.Multipart.parts[0].kind.Mc.answers[1].image.file".to_string())]);
    // A picture that is not an object.
    let raw = r#"{"schema_version":1,"question":{"id":"q1","stem":"s","kind":{"Mc":{"answers":[{"text":"","image":"media/x.png"},{"text":"y"}]}},"points":null,"difficulty":null,"created":0,"modified":0}}"#;
    // (Not a picture, so the empty text is reported too.)
    assert_eq!(codes(raw), [("expected_object", "question.kind.Mc.answers[0].image".to_string()), ("blank_text", "question.kind.Mc.answers[0].text".to_string())]);
}

#[test]
fn an_unknown_key_on_a_choice_is_still_dropped_and_reported() {
    let raw = r#"{"schema_version":1,"question":{"id":"q1","stem":"s","kind":{"Mc":{"answers":[{"text":"x","sound":"a.mp3"},{"text":"y"}]}},"points":null,"difficulty":null,"created":0,"modified":0}}"#;
    let v = ok(raw);
    assert_eq!(v.dropped, [Dropped { path: "question.kind.Mc.answers[0]".into(), reason: "unknown_field" }]);
}

#[test]
fn the_strip_treats_choice_pictures_as_the_questions_pictures() {
    let p = Payload { schema_version: 2, body: Body::Question(pictures_mc()) };
    // Ticked: every choice keeps its picture; the figure is removed, as a question's always is.
    let s = strip_with_seed(p.clone(), &opts_for(&p, vec![0]), seed(3)).unwrap();
    let q = only_question(&s.payload);
    let Kind::Mc { answers } = &q.kind else { panic!() };
    let mut files: Vec<&str> = answers.iter().filter_map(|a| a.image.as_ref().map(|i| i.file.as_str())).collect();
    files.sort();
    assert_eq!(files.len(), 2);
    assert!(answers.iter().all(|a| a.figure.is_none() && !a.correct));
    assert_eq!((s.images_kept.as_slice(), s.images_removed.as_slice(), s.figures_removed.as_slice()), (&[0][..], &[][..], &[0][..]));
    // The served form is itself valid and keeps its picture choices.
    assert_eq!(ok(&s.payload.to_json()).payload, s.payload);
    assert!(!s.payload.to_json().contains("pie-chart"), "a choice's figure is never served");

    // Not ticked: a picture-only choice would be served blank, so the strip refuses it by its path.
    let e = strip_with_seed(p.clone(), &opts_for(&p, vec![]), seed(3)).unwrap_err();
    assert_eq!(codes_of(e), [("choice_picture_needs_image_ok", "question.kind.Mc.answers[0].image".to_string())]);
    // In a bank the path names the question.
    let bank = QuestionBank { id: "b1".into(), name: "Charts".into(), questions: vec![question("Plain", Kind::Tf { correct: true }), pictures_multipart()], created: 1, modified: 2, details: Default::default(), origin: None, published: None };
    let pb = Payload { schema_version: 2, body: Body::Bank(bank) };
    let e = strip_with_seed(pb.clone(), &opts_for(&pb, vec![]), seed(3)).unwrap_err();
    assert_eq!(
        codes_of(e),
        [
            ("choice_picture_needs_image_ok", "bank.questions[1].kind.Multipart.parts[0].kind.Mc.answers[0].image".to_string()),
            ("choice_picture_needs_image_ok", "bank.questions[1].kind.Multipart.parts[0].kind.Mc.answers[1].image".to_string())
        ]
    );
    assert!(strip_with_seed(pb.clone(), &opts_for(&pb, vec![1]), seed(3)).is_ok());

    // Not ticked, every picture choice with a text: the pictures are removed and the texts served.
    let mut q = pictures_mc();
    let Kind::Mc { answers } = &mut q.kind else { panic!() };
    answers[0].text = "Chart A".into();
    let p = Payload { schema_version: 2, body: Body::Question(q) };
    let s = strip_with_seed(p.clone(), &opts_for(&p, vec![]), seed(3)).unwrap();
    let Kind::Mc { answers } = &only_question(&s.payload).kind else { panic!() };
    assert!(answers.iter().all(|a| a.image.is_none() && a.figure.is_none()));
    assert_eq!(s.images_removed, [0]);
}

#[test]
fn a_keyless_question_carries_no_choice_figure() {
    let mut q = pictures_mc();
    q.keyless = true;
    q.explanation = Some(KEYLESS_SENTINEL.into());
    let Kind::Mc { answers } = &mut q.kind else { panic!() };
    answers.iter_mut().for_each(|a| a.correct = false);
    assert_eq!(codes(&q_payload(&q)), [("keyless_key_material", "question.kind.Mc.answers[1].figure".to_string())]);
    // Its pictures may stay (the author ticked them).
    let Kind::Mc { answers } = &mut q.kind else { panic!() };
    answers[1].figure = None;
    assert!(validate(&q_payload(&q), &Limits::default(), &Structural).is_ok());
}

#[test]
fn picture_choices_are_compared_by_text_and_picture() {
    // Two picture-only choices with different pictures are not the same choice.
    let two = |a: char, b: char| question("Which?", Kind::Mc { answers: vec![Answer { image: Some(img(a, 5, 5)), ..answer("", true) }, Answer { image: Some(img(b, 5, 5)), ..answer("", false) }] });
    let opts = PublishOptions::default();
    assert!(validate_for_publish(q_payload(&two('a', 'b')).as_bytes(), &Limits::default(), &Structural, &opts).is_ok());
    let e = validate_for_publish(q_payload(&two('a', 'a')).as_bytes(), &Limits::default(), &Structural, &opts).unwrap_err();
    assert_eq!(codes_of(e), [("duplicate_choice", "question.kind.Mc.answers".to_string())]);
}

#[test]
fn the_hashes_see_a_choice_picture_like_a_question_picture() {
    let a = pictures_mc();
    let mut b = pictures_mc();
    let Kind::Mc { answers } = &mut b.kind else { panic!() };
    answers[0].image = Some(img('f', 200, 200));
    // The body's integrity hash covers every field, a choice's picture included...
    assert_ne!(full_hash(&a), full_hash(&b));
    // ...the match hash covers no picture (a question's or a choice's), as before.
    assert_eq!(match_hash(&a), match_hash(&b));
    // A text-only question hashes exactly as before picture choices existed (`fixtures/hashes.json` holds the rest).
    let t = question("Closest planet?", Kind::Mc { answers: vec![answer("Mercury", true), answer("Venus", false)] });
    let canon = canonical_json(&serde_json::to_value(&t).unwrap());
    let mut written = String::new();
    write_question(&t, &mut written);
    assert_eq!(written, canon);
}
