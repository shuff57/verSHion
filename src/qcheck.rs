//! What is missing from a question: ONE set of rules for the editor, the exam and the pool.
//!
//! A question may be saved at any time, however much is missing (a draft). This module only
//! says what is missing and where. The editor shows each [`Issue`] inline on its field and in
//! the footer; adding a question to an exam, publishing it and printing its key ask the same
//! [`check_question`] (so the rules cannot drift apart). Nothing here changes what is stored: a
//! question with no correct answer is a `Kind::Mc` whose answers are all `correct: false`, and a
//! question imported without its key keeps the `KEYLESS_SENTINEL` explanation.

use crate::model::{self, Kind, PartKind, Question};

/// How much an issue matters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// The question is not finished: it saves as a draft, but cannot go on an exam, be shared or
    /// print an answer key.
    Fix,
    /// Worth a look, blocks nothing (an empty row between filled ones, which is not saved).
    Note,
}

/// The field an issue belongs to (where the editor draws it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Where {
    Stem,
    /// The answer list as a whole (no correct answer, too few answers, True/False not picked).
    Answers,
    /// Answer row `i` (0-based).
    Answer(usize),
    /// The pair list as a whole.
    Pairs,
    Pair(usize),
    /// The blanks as a whole (none in the stem, or not as many answers as blanks).
    Blanks,
    Blank(usize),
    /// The part list as a whole.
    Parts,
    Part(usize),
}

/// One thing to fix (or note) about a question.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    pub at: Where,
    pub severity: Severity,
    /// Says what is wrong in the words the teacher sees, without a full stop.
    pub msg: String,
    /// The answer key is what is missing, so a printed key would be wrong or empty.
    pub key: bool,
}

fn fix(at: Where, msg: impl Into<String>, key: bool) -> Issue {
    Issue { at, severity: Severity::Fix, msg: msg.into(), key }
}

fn note(at: Where, msg: impl Into<String>) -> Issue {
    Issue { at, severity: Severity::Note, msg: msg.into(), key: false }
}

fn filled(s: &str) -> bool {
    !s.trim().is_empty()
}

/// How many blanks the stem holds (each `___`).
pub fn blank_count(stem: &str) -> usize {
    stem.matches("___").count()
}

/// Rows of an answer list that are empty but would be missed: one before a filled row, or any when
/// fewer than two rows are filled (the rows trailing a complete list are simply not saved).
fn empty_rows(answers: &[model::Answer]) -> Vec<Issue> {
    let n_filled = answers.iter().filter(|a| filled(&a.text)).count();
    let last = answers.iter().rposition(|a| filled(&a.text));
    answers
        .iter()
        .enumerate()
        .filter(|(i, a)| !filled(&a.text) && (n_filled < 2 || last.is_some_and(|l| *i < l)))
        .map(|(i, _)| note(Where::Answer(i), format!("Answer {} is empty", i + 1)))
        .collect()
}

/// Everything missing from a stem and kind. Works on a draft as the editor holds it (empty rows and
/// half-filled pairs still present) and on a stored question alike.
pub fn check(stem: &str, kind: &Kind) -> Vec<Issue> {
    let mut v = Vec::new();
    if !filled(stem) {
        v.push(fix(Where::Stem, "Add a question stem", false));
    }
    match kind {
        Kind::Mc { answers } => {
            if answers.iter().filter(|a| filled(&a.text)).count() < 2 {
                v.push(fix(Where::Answers, "Add at least two answer choices", false));
            }
            if !answers.iter().any(|a| a.correct && filled(&a.text)) {
                v.push(fix(Where::Answers, "Mark one answer correct", true));
            }
            v.extend(empty_rows(answers));
        }
        Kind::Tf { .. } => {}
        Kind::Matching { pairs, .. } => {
            for (i, p) in pairs.iter().enumerate() {
                match (filled(&p.left), filled(&p.right)) {
                    (true, false) => v.push(fix(Where::Pair(i), format!("Pair {} has no match on the right", i + 1), true)),
                    (false, true) => v.push(fix(Where::Pair(i), format!("Pair {} has no item on the left", i + 1), false)),
                    _ => {}
                }
            }
            if pairs.iter().filter(|p| filled(&p.left) && filled(&p.right)).count() < 2 {
                v.push(fix(Where::Pairs, "Add at least two matching pairs (both sides filled)", false));
            }
        }
        Kind::Blank { accept } => {
            let n = blank_count(stem);
            if n == 0 {
                v.push(fix(Where::Blanks, format!("Add a blank: {}", crate::qedit::BLANK_HOW), false));
            } else if accept.len() < n {
                v.push(fix(Where::Blanks, "Blanks and answers do not match", true));
            } else if accept.len() > n {
                // (answers no blank asks for are never printed: worth a look, nothing is missing)
                v.push(note(Where::Blanks, "Blanks and answers do not match"));
            }
            for (i, row) in accept.iter().enumerate().take(n) {
                if !row.iter().any(|a| filled(a)) {
                    v.push(fix(Where::Blank(i), format!("Blank {} has no accepted answer", i + 1), true));
                }
            }
        }
        Kind::Short { sample } => {
            if !filled(sample) {
                v.push(note(Where::Answers, "No model answer: the key will be blank"));
            }
        }
        Kind::Multipart { parts } => {
            if parts.is_empty() {
                v.push(fix(Where::Parts, "Add at least one part", false));
            }
            for (i, p) in parts.iter().enumerate() {
                if !filled(&p.stem) {
                    v.push(fix(Where::Part(i), format!("Part {} needs its own stem", i + 1), false));
                }
                match &p.kind {
                    PartKind::Mc { answers } => {
                        if answers.iter().filter(|a| filled(&a.text)).count() < 2 {
                            v.push(fix(Where::Part(i), format!("Part {} needs at least two answer choices", i + 1), false));
                        }
                        if !answers.iter().any(|a| a.correct && filled(&a.text)) {
                            v.push(fix(Where::Part(i), format!("Part {} needs a correct answer", i + 1), true));
                        }
                    }
                    PartKind::Tf { .. } => {}
                    PartKind::Short { sample } => {
                        if !filled(sample) {
                            v.push(note(Where::Part(i), format!("Part {} has no model answer: the key will be blank", i + 1)));
                        }
                    }
                }
            }
        }
    }
    v
}

/// [`check`] for a stored question, plus the one thing only a stored question can be: imported
/// without its key (the key issues the content would show are replaced by that one).
pub fn check_question(q: &Question) -> Vec<Issue> {
    let mut v = check(&q.stem, &q.kind);
    if model::is_keyless(q) {
        v.retain(|i| !i.key);
        v.push(fix(Where::Answers, "Imported without its answer key", true));
    }
    v
}

/// The issues that keep a question off an exam and out of the pool.
pub fn blockers(q: &Question) -> Vec<Issue> {
    check_question(q).into_iter().filter(|i| i.severity == Severity::Fix).collect()
}

/// What keeps a question off an exam and out of the pool: the gaps in its content. A question
/// imported without its key is not held back by that alone: it says so wherever it shows, its key
/// prints as "Key not provided", and the pool has its own rule for it (`hide_key`).
pub fn gate_blockers(q: &Question) -> Vec<Issue> {
    let mut v = check(&q.stem, &q.kind);
    if model::is_keyless(q) {
        v.retain(|i| !i.key);
    }
    v.retain(|i| i.severity == Severity::Fix);
    v
}

/// Whether a printed answer key for `q` would be wrong or empty: it was imported without its key,
/// or the key is incomplete. Those print "Key not provided".
pub fn key_missing(q: &Question) -> bool {
    model::is_keyless(q) || check(&q.stem, &q.kind).iter().any(|i| i.key)
}

/// The messages of `issues` as one sentence fragment: "Mark one answer correct; Blank 2 has no
/// accepted answer".
pub fn describe(issues: &[Issue]) -> String {
    issues.iter().map(|i| i.msg.as_str()).collect::<Vec<_>>().join("; ")
}

/// A short label for a question in a message: its first line, cut.
pub fn label(q: &Question) -> String {
    let s = crate::equation::stem_preview(&q.stem);
    let line = s.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    if line.is_empty() {
        return "(no question text)".to_string();
    }
    let cut: String = line.chars().take(40).collect();
    if line.chars().count() > 40 { format!("{cut}...") } else { cut }
}

/// The chip a bank card wears when the question is not finished: its words and a tooltip that
/// names what is missing. `None` for a finished question.
pub fn chip(q: &Question) -> Option<(&'static str, String)> {
    let b = blockers(q);
    if b.is_empty() {
        return None;
    }
    let label = if model::is_keyless(q) {
        "No answer key"
    } else {
        use Where as W;
        match b[0].at {
            W::Stem => "Needs a stem",
            W::Answers | W::Answer(_) if b[0].key => "No correct answer",
            W::Answers | W::Answer(_) => "Needs answers",
            W::Blank(_) => "No accepted answer",
            W::Blanks => "Needs a blank",
            W::Pairs | W::Pair(_) => "Needs matching pairs",
            W::Parts | W::Part(_) => "Needs part details",
        }
    };
    Some((label, format!("{}. It can not go on an exam or be shared until this is fixed.", describe(&b))))
}

/// The sentence for the first of `qs` (each with the number to call it by) that is not finished:
/// `None` when every question is fine. `verb` finishes "... can not be {verb}: ..." ("added to an
/// exam", "shared"). Returns the index in `qs` too, so the caller can open that question for
/// editing.
pub fn first_blocked(qs: &[(usize, &Question)], verb: &str) -> Option<(usize, String)> {
    let bad: Vec<(usize, Vec<Issue>)> = qs.iter().enumerate().map(|(i, (_, q))| (i, gate_blockers(q))).filter(|(_, b)| !b.is_empty()).collect();
    let (first, issues) = bad.first()?;
    let (number, q) = qs[*first];
    let more = if bad.len() > 1 { format!(" ({} more need work too)", bad.len() - 1) } else { String::new() };
    Some((*first, format!("Question {number} \u{201c}{}\u{201d} is a draft and can not be {verb}: {}.{more}", label(q), describe(issues))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Answer, Pair, Part};

    fn a(t: &str, c: bool) -> Answer {
        Answer { text: t.to_string(), correct: c, locked: false }
    }

    fn q(stem: &str, kind: Kind) -> Question {
        Question { id: "q".into(), stem: stem.into(), kind, points: None, difficulty: None, topics: vec![], image: None, explanation: None, figure: None, created: 0, modified: 0 }
    }

    fn msgs(v: &[Issue]) -> Vec<&str> {
        v.iter().map(|i| i.msg.as_str()).collect()
    }

    #[test]
    fn mc_with_no_correct_answer_is_flagged_as_a_key_problem() {
        let v = check("Pick", &Kind::Mc { answers: vec![a("x", false), a("y", false)] });
        assert_eq!(msgs(&v), ["Mark one answer correct"]);
        assert!(v[0].key && v[0].severity == Severity::Fix && v[0].at == Where::Answers);
    }

    #[test]
    fn a_correct_mark_on_an_empty_row_is_not_a_key() {
        let v = check("Pick", &Kind::Mc { answers: vec![a("x", false), a("y", false), a("", true)] });
        assert!(msgs(&v).contains(&"Mark one answer correct"));
    }

    #[test]
    fn mc_too_few_answers_and_empty_rows() {
        let v = check("Pick", &Kind::Mc { answers: vec![a("x", true), a("", false), a("", false)] });
        assert_eq!(msgs(&v), ["Add at least two answer choices", "Answer 2 is empty", "Answer 3 is empty"]);
        assert_eq!(v[1].at, Where::Answer(1));
        assert_eq!(v[1].severity, Severity::Note, "an empty row blocks nothing by itself");
    }

    #[test]
    fn empty_rows_after_a_complete_list_say_nothing_but_a_gap_does() {
        let ok = check("Pick", &Kind::Mc { answers: vec![a("x", true), a("y", false), a("", false), a("", false)] });
        assert!(ok.is_empty(), "{ok:?}");
        let gap = check("Pick", &Kind::Mc { answers: vec![a("x", true), a("", false), a("y", false)] });
        assert_eq!(msgs(&gap), ["Answer 2 is empty"]);
    }

    #[test]
    fn missing_stem_is_flagged_on_every_kind() {
        let kinds = [
            Kind::Mc { answers: vec![a("x", true), a("y", false)] },
            Kind::Tf { correct: true },
            Kind::Short { sample: "s".into() },
        ];
        for k in kinds {
            let v = check("  ", &k);
            assert_eq!(v.first().map(|i| (i.at, i.msg.as_str())), Some((Where::Stem, "Add a question stem")), "{k:?}");
        }
    }

    #[test]
    fn true_false_and_short_are_complete_when_they_have_a_stem() {
        assert!(check("T or F?", &Kind::Tf { correct: false }).is_empty());
        // (an empty model answer is a note, never a blocker)
        assert!(check("Explain", &Kind::Short { sample: String::new() }).iter().all(|i| i.severity == Severity::Note));
        assert!(check("Explain", &Kind::Short { sample: "x".into() }).is_empty());
    }

    #[test]
    fn matching_flags_half_filled_pairs_and_too_few() {
        let k = Kind::Matching {
            pairs: vec![Pair { left: "a".into(), right: "1".into() }, Pair { left: "b".into(), right: String::new() }, Pair { left: String::new(), right: "3".into() }],
            distractors: vec![],
        };
        let v = check("Match", &k);
        assert_eq!(msgs(&v), ["Pair 2 has no match on the right", "Pair 3 has no item on the left", "Add at least two matching pairs (both sides filled)"]);
        assert!(v[0].key && !v[1].key);
        let two = Kind::Matching { pairs: vec![Pair { left: "a".into(), right: "1".into() }, Pair { left: "b".into(), right: "2".into() }, Pair { left: String::new(), right: String::new() }], distractors: vec![] };
        assert!(check("Match", &two).is_empty(), "a wholly empty pair is just not saved");
    }

    #[test]
    fn blank_issues_name_the_blank() {
        let none = check("no blank here", &Kind::Blank { accept: vec![] });
        assert!(msgs(&none)[0].starts_with("Add a blank"));
        let k = Kind::Blank { accept: vec![vec!["x".into()], vec![]] };
        let v = check("a ___ b ___", &k);
        assert_eq!(msgs(&v), ["Blank 2 has no accepted answer"]);
        assert_eq!(v[0].at, Where::Blank(1));
        let off = check("a ___ b ___ c ___", &k);
        assert!(msgs(&off).contains(&"Blanks and answers do not match"), "{off:?}");
        assert!(off.iter().any(|i| i.at == Where::Blanks && i.key && i.severity == Severity::Fix));
        let extra = check("a ___", &Kind::Blank { accept: vec![vec!["x".into()], vec!["y".into()]] });
        assert_eq!(msgs(&extra), ["Blanks and answers do not match"]);
        assert_eq!(extra[0].severity, Severity::Note, "an unused extra answer blocks nothing");
    }

    #[test]
    fn multipart_flags_parts() {
        let none = check("Q", &Kind::Multipart { parts: vec![] });
        assert_eq!(msgs(&none), ["Add at least one part"]);
        let k = Kind::Multipart {
            parts: vec![
                Part { stem: String::new(), kind: PartKind::Tf { correct: true } },
                Part { stem: "p2".into(), kind: PartKind::Mc { answers: vec![a("x", false), a("y", false)] } },
                Part { stem: "p3".into(), kind: PartKind::Mc { answers: vec![a("", false), a("", false)] } },
            ],
        };
        let v = check("Q", &k);
        // (part 3 has a stem and two empty rows: that is no answers at all, so it is incomplete too)
        assert_eq!(
            msgs(&v),
            ["Part 1 needs its own stem", "Part 2 needs a correct answer", "Part 3 needs at least two answer choices", "Part 3 needs a correct answer"]
        );
        assert_eq!(v[1].at, Where::Part(1));
    }

    #[test]
    fn keyless_import_is_flagged_and_replaces_the_content_key_issues() {
        let mut k = q("Pick", Kind::Mc { answers: vec![a("x", false), a("y", false)] });
        k.explanation = Some(model::KEYLESS_SENTINEL.to_string());
        let v = check_question(&k);
        assert_eq!(msgs(&v), ["Imported without its answer key"]);
        assert!(key_missing(&k) && !blockers(&k).is_empty());
        let mut t = q("T?", Kind::Tf { correct: false });
        t.explanation = Some(model::KEYLESS_SENTINEL.to_string());
        assert_eq!(msgs(&check_question(&t)), ["Imported without its answer key"], "a keyless True/False has nothing else to show it");
        assert!(key_missing(&t));
    }

    #[test]
    fn a_complete_question_has_no_blockers_and_a_key() {
        let c = q("Pick", Kind::Mc { answers: vec![a("x", true), a("y", false)] });
        assert!(blockers(&c).is_empty() && !key_missing(&c));
    }

    #[test]
    fn a_keyless_import_is_let_through_the_gate_by_its_marker_but_not_by_other_gaps() {
        let mut k = q("Pick", Kind::Mc { answers: vec![a("x", false), a("y", false)] });
        k.explanation = Some(model::KEYLESS_SENTINEL.to_string());
        assert!(gate_blockers(&k).is_empty(), "flagged and shown as keyless, but allowed");
        let mut bad = q("", Kind::Mc { answers: vec![a("x", false), a("y", false)] });
        bad.explanation = Some(model::KEYLESS_SENTINEL.to_string());
        assert_eq!(msgs(&gate_blockers(&bad)), ["Add a question stem"]);
    }

    #[test]
    fn notes_do_not_block() {
        let c = q("Pick", Kind::Mc { answers: vec![a("x", true), a("", false), a("y", false)] });
        assert_eq!(check_question(&c).len(), 1);
        assert!(blockers(&c).is_empty());
    }

    #[test]
    fn chips_name_the_first_gap() {
        assert!(chip(&q("Fine", Kind::Tf { correct: true })).is_none());
        let (l, tip) = chip(&q("Pick", Kind::Mc { answers: vec![a("x", false), a("y", false)] })).unwrap();
        assert_eq!(l, "No correct answer");
        assert!(tip.contains("Mark one answer correct"));
        assert_eq!(chip(&q("", Kind::Mc { answers: vec![a("x", true), a("y", false)] })).unwrap().0, "Needs a stem");
        let f = q("One ___ two", Kind::Blank { accept: vec![vec![String::new()]] });
        assert_eq!(chip(&f).unwrap().0, "No accepted answer");
        assert_eq!(chip(&q("Match", Kind::Matching { pairs: vec![], distractors: vec![] })).unwrap().0, "Needs matching pairs");
        let mut k = q("Pick", Kind::Mc { answers: vec![a("x", false), a("y", false)] });
        k.explanation = Some(model::KEYLESS_SENTINEL.to_string());
        assert_eq!(chip(&k).unwrap().0, "No answer key");
    }

    #[test]
    fn the_gate_sentence_names_the_question_and_what_is_missing() {
        let ok = q("Fine", Kind::Tf { correct: true });
        let bad = q("Which is prime?", Kind::Mc { answers: vec![a("2", false), a("4", false)] });
        let bad2 = q("", Kind::Short { sample: String::new() });
        let (i, s) = first_blocked(&[(1, &ok), (2, &bad), (3, &bad2)], "added to an exam").unwrap();
        assert_eq!(i, 1);
        assert!(s.starts_with("Question 2 \u{201c}Which is prime?\u{201d} is a draft and can not be added to an exam: Mark one answer correct."), "{s}");
        assert!(s.contains("1 more"), "{s}");
        assert!(first_blocked(&[(1, &ok)], "shared").is_none());
    }

    #[test]
    fn a_multipart_part_needs_what_a_top_level_question_of_its_kind_needs() {
        let mp = |kind: PartKind| Kind::Multipart { parts: vec![Part { stem: "p".into(), kind }] };
        // MC part with a stem and no answers at all: blocked at its own part
        let k = q("Q", mp(PartKind::Mc { answers: vec![] }));
        let b = gate_blockers(&k);
        assert_eq!(msgs(&b), ["Part 1 needs at least two answer choices", "Part 1 needs a correct answer"]);
        assert!(b.iter().all(|i| i.at == Where::Part(0)));
        assert!(key_missing(&k) && chip(&k).is_some());
        // one answer, ticked: still too few; two with a tick: fine
        assert_eq!(gate_blockers(&q("Q", mp(PartKind::Mc { answers: vec![a("x", true)] }))).len(), 1);
        assert!(gate_blockers(&q("Q", mp(PartKind::Mc { answers: vec![a("x", true), a("y", false)] }))).is_empty());
        // TF part is complete with its stem; short part without a model answer only notes
        assert!(gate_blockers(&q("Q", mp(PartKind::Tf { correct: true }))).is_empty());
        let s = q("Q", mp(PartKind::Short { sample: String::new() }));
        assert!(gate_blockers(&s).is_empty());
        let n = check_question(&s);
        assert_eq!((n.len(), n[0].at, n[0].severity), (1, Where::Part(0), Severity::Note));
    }

    #[test]
    fn a_short_question_without_a_model_answer_is_noted_not_blocked() {
        let k = q("Explain", Kind::Short { sample: String::new() });
        let v = check_question(&k);
        assert_eq!(msgs(&v), ["No model answer: the key will be blank"]);
        assert!(blockers(&k).is_empty() && chip(&k).is_none());
    }
}
