//! Import eval: how closely a converted test matches a reference, with numbers instead of a feeling.
//!
//! The reference is a bank the teacher already trusts (a `.parrot.json`). The eval converts the same source (a PDF)
//! through the real import path, lines the converted questions up with the reference ones, and scores what the
//! hosted AI is weakest at: finding every question, the right type, the answer key (handwritten keys above all),
//! the points, and whether a picture was kept.
//!
//! Two halves. [`compare`] is pure and runs anywhere (the tests below pin it). [`convert_pdf`] (native) rasterises
//! the PDF, asks the configured provider page by page and keeps each page's raw reply in a cache folder, so a
//! later run with `VERSHION_EVAL_REPLAY=1` scores the same replies again for free after the scorer changes.
//!
//! Headless: `VERSHION_IMPORT_EVAL=<file.pdf> VERSHION_IMPORT_TRUTH=<ref.parrot.json> [VERSHION_EVAL_CACHE=<dir>]
//! [VERSHION_EVAL_REPLAY=1] [VERSHION_EVAL_OUT=<report.json>] [VERSHION_HINT=...]`. Set `VERSHION_STORE_DIR` to a
//! scratch folder: the import crops pictures into the store.

use crate::model::{Kind, Question};
use serde::Serialize;

/// Lower-case alphanumeric words of `s`; punctuation, markup and spacing are ignored.
fn words(s: &str) -> Vec<String> {
    s.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_string).collect()
}

/// Dice similarity of the two texts' word multisets, 0..=1. Two empty texts are the same.
pub fn similarity(a: &str, b: &str) -> f64 {
    let (wa, wb) = (words(a), words(b));
    if wa.is_empty() && wb.is_empty() {
        return 1.0;
    }
    if wa.is_empty() || wb.is_empty() {
        return 0.0;
    }
    let mut pool = wb.clone();
    let mut common = 0usize;
    for w in &wa {
        if let Some(i) = pool.iter().position(|p| p == w) {
            pool.swap_remove(i);
            common += 1;
        }
    }
    2.0 * common as f64 / (wa.len() + wb.len()) as f64
}

/// What identifies a question when lining two lists up: its stem plus its choices or parts.
fn signature(q: &Question) -> String {
    let mut s = q.stem.clone();
    match &q.kind {
        Kind::Mc { answers } => answers.iter().for_each(|a| {
            s.push(' ');
            s.push_str(&a.text)
        }),
        Kind::Matching { pairs, .. } => pairs.iter().for_each(|p| {
            s.push(' ');
            s.push_str(&p.left)
        }),
        Kind::Multipart { parts } => parts.iter().for_each(|p| {
            s.push(' ');
            s.push_str(&p.stem)
        }),
        _ => {}
    }
    s
}

/// For each reference question, the index of the converted question it lines up with (best word overlap first,
/// each converted question used once, nothing below `floor`).
pub fn align(truth: &[Question], got: &[Question], floor: f64) -> Vec<Option<usize>> {
    let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
    for (i, t) in truth.iter().enumerate() {
        for (j, g) in got.iter().enumerate() {
            let s = similarity(&signature(t), &signature(g));
            if s >= floor {
                pairs.push((s, i, j));
            }
        }
    }
    pairs.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let (mut out, mut used) = (vec![None; truth.len()], vec![false; got.len()]);
    for (_, i, j) in pairs {
        if out[i].is_none() && !used[j] {
            out[i] = Some(j);
            used[j] = true;
        }
    }
    out
}

/// How much of the answer key came through, 0..=1; `None` when the reference has no key to compare against.
fn answer_score(t: &Question, g: &Question) -> Option<f64> {
    if t.kind.qkind() != g.kind.qkind() {
        return Some(0.0);
    }
    match (&t.kind, &g.kind) {
        (Kind::Mc { answers: ta }, Kind::Mc { answers: ga }) => {
            let key = |a: &[crate::model::Answer]| -> Vec<String> { a.iter().filter(|x| x.correct).map(|x| words(&x.text).join(" ")).collect() };
            let (tk, gk) = (key(ta), key(ga));
            if tk.is_empty() {
                return None;
            }
            let hit = tk.iter().filter(|k| gk.contains(k)).count();
            let union = tk.len() + gk.iter().filter(|k| !tk.contains(k)).count();
            Some(hit as f64 / union as f64)
        }
        (Kind::Tf { correct: a }, Kind::Tf { correct: b }) => Some(if a == b { 1.0 } else { 0.0 }),
        (Kind::Blank { accept: ta }, Kind::Blank { accept: ga }) => {
            let blanks: Vec<&Vec<String>> = ta.iter().filter(|a| !a.is_empty()).collect();
            if blanks.is_empty() {
                return None;
            }
            let ok = blanks.iter().enumerate().filter(|(i, t)| ga.get(*i).is_some_and(|g| g.iter().any(|x| similarity(x, &t[0]) >= 0.8))).count();
            Some(ok as f64 / blanks.len() as f64)
        }
        (Kind::Short { sample: a }, Kind::Short { sample: b }) => {
            if a.trim().is_empty() {
                None
            } else {
                Some(similarity(a, b))
            }
        }
        (Kind::Matching { pairs: tp, .. }, Kind::Matching { pairs: gp, .. }) => {
            if tp.is_empty() {
                return None;
            }
            let ok = tp.iter().filter(|t| gp.iter().any(|g| similarity(&g.left, &t.left) >= 0.7 && similarity(&g.right, &t.right) >= 0.7)).count();
            Some(ok as f64 / tp.len() as f64)
        }
        (Kind::Multipart { parts: a }, Kind::Multipart { parts: b }) => {
            // Parts line up by position; each is scored on its own answer. A different number of parts costs the same
            // share it is off by, and a reference that keeps no answers for any part leaves nothing to compare.
            let scores: Vec<f64> = a.iter().zip(b).filter_map(|(t, g)| part_score(t, g)).collect();
            let count = a.len().min(b.len()) as f64 / a.len().max(b.len()).max(1) as f64;
            if scores.is_empty() {
                return if a.is_empty() { None } else { Some(count) };
            }
            Some(count * scores.iter().sum::<f64>() / scores.len() as f64)
        }
        _ => Some(0.0),
    }
}

/// One part's answer, 0..=1; `None` when the reference part keeps no answer.
fn part_score(t: &crate::model::Part, g: &crate::model::Part) -> Option<f64> {
    use crate::model::PartKind as P;
    match (&t.kind, &g.kind) {
        (P::Short { sample: a }, P::Short { sample: b }) => (!a.trim().is_empty()).then(|| similarity(a, b)),
        (P::Tf { correct: a }, P::Tf { correct: b }) => Some(if a == b { 1.0 } else { 0.0 }),
        (P::Mc { answers: a }, P::Mc { answers: b }) => {
            let key = |x: &[crate::model::Answer]| -> Vec<String> { x.iter().filter(|y| y.correct).map(|y| words(&y.text).join(" ")).collect() };
            let (tk, gk) = (key(a), key(b));
            (!tk.is_empty()).then(|| tk.iter().filter(|k| gk.contains(k)).count() as f64 / (tk.len() + gk.iter().filter(|k| !tk.contains(k)).count()) as f64)
        }
        _ => Some(0.0),
    }
}

/// One reference question and how its converted twin scored.
#[derive(Debug, Clone, Serialize)]
pub struct QScore {
    /// The reference question, as its first words.
    pub reference: String,
    pub found: bool,
    pub stem_similarity: f64,
    pub kind_ok: bool,
    /// 0..=1 share of the answer key that came through; `None` when the reference has no key here.
    pub answer: Option<f64>,
    /// `None` when the reference gives no points.
    pub points_ok: Option<bool>,
    /// `None` when the reference has no picture.
    pub picture_kept: Option<bool>,
}

/// The whole comparison.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub reference_questions: usize,
    pub converted_questions: usize,
    pub found: usize,
    /// Converted questions that line up with no reference question.
    pub extra: usize,
    /// Found / reference.
    pub recall: f64,
    /// Found / converted.
    pub precision: f64,
    pub mean_stem_similarity: f64,
    pub kind_accuracy: f64,
    /// Mean answer-key score over the found questions whose reference has a key.
    pub answer_accuracy: f64,
    pub points_accuracy: f64,
    pub picture_recall: f64,
    pub per_question: Vec<QScore>,
}

fn ratio(n: usize, d: usize) -> f64 {
    if d == 0 { 1.0 } else { n as f64 / d as f64 }
}

fn head(s: &str) -> String {
    s.split_whitespace().take(9).collect::<Vec<_>>().join(" ")
}

/// Scores `got` against `truth`.
pub fn compare(truth: &[Question], got: &[Question]) -> Report {
    let lined = align(truth, got, 0.45);
    let per: Vec<QScore> = truth
        .iter()
        .zip(&lined)
        .map(|(t, m)| {
            let g = m.map(|j| &got[j]);
            QScore {
                reference: head(&t.stem),
                found: g.is_some(),
                stem_similarity: g.map_or(0.0, |g| similarity(&t.stem, &g.stem)),
                kind_ok: g.is_some_and(|g| g.kind.qkind() == t.kind.qkind()),
                answer: g.and_then(|g| answer_score(t, g)),
                points_ok: t.points.map(|p| g.and_then(|g| g.points).is_some_and(|q| (p - q).abs() < 1e-6)),
                picture_kept: (t.image.is_some() || t.figure.is_some()).then(|| g.is_some_and(|g| g.image.is_some() || g.figure.is_some())),
            }
        })
        .collect();
    let found = per.iter().filter(|q| q.found).count();
    let answers: Vec<f64> = per.iter().filter(|q| q.found).filter_map(|q| q.answer).collect();
    let pts: Vec<bool> = per.iter().filter_map(|q| q.points_ok).collect();
    let pics: Vec<bool> = per.iter().filter_map(|q| q.picture_kept).collect();
    Report {
        reference_questions: truth.len(),
        converted_questions: got.len(),
        found,
        extra: got.len().saturating_sub(found),
        recall: ratio(found, truth.len()),
        precision: ratio(found, got.len()),
        mean_stem_similarity: if found == 0 { 0.0 } else { per.iter().filter(|q| q.found).map(|q| q.stem_similarity).sum::<f64>() / found as f64 },
        kind_accuracy: ratio(per.iter().filter(|q| q.kind_ok).count(), truth.len()),
        answer_accuracy: if answers.is_empty() { 1.0 } else { answers.iter().sum::<f64>() / answers.len() as f64 },
        points_accuracy: ratio(pts.iter().filter(|x| **x).count(), pts.len()),
        picture_recall: ratio(pics.iter().filter(|x| **x).count(), pics.len()),
        per_question: per,
    }
}

impl Report {
    /// A few lines a person can read.
    pub fn summary(&self) -> String {
        let pct = |x: f64| format!("{:>5.1}%", x * 100.0);
        let mut s = format!(
            "questions: {} reference, {} converted, {} found, {} extra\n  recall {}  precision {}  wording {}\n  type {}  answer key {}  points {}  pictures kept {}\n",
            self.reference_questions,
            self.converted_questions,
            self.found,
            self.extra,
            pct(self.recall),
            pct(self.precision),
            pct(self.mean_stem_similarity),
            pct(self.kind_accuracy),
            pct(self.answer_accuracy),
            pct(self.points_accuracy),
            pct(self.picture_recall),
        );
        for (i, q) in self.per_question.iter().enumerate() {
            let ans = q.answer.map_or("-".to_string(), |a| format!("{:.0}%", a * 100.0));
            s.push_str(&format!(
                "  {:>2}. {} | wording {:.0}% type {} key {} points {} picture {} | {}\n",
                i + 1,
                if q.found { "found  " } else { "MISSING" },
                q.stem_similarity * 100.0,
                if q.kind_ok { "ok" } else { "WRONG" },
                ans,
                q.points_ok.map_or("-", |b| if b { "ok" } else { "WRONG" }),
                q.picture_kept.map_or("-", |b| if b { "kept" } else { "LOST" }),
                q.reference
            ));
        }
        s
    }
}

/// The reference bank's questions from a `.parrot.json` (or any format the importer reads).
pub fn read_reference(path: &str) -> Result<Vec<Question>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    Ok(crate::formats::detect_and_parse(&bytes, &name)?.questions)
}

/// Converts every page of a PDF through the configured provider, keeping each raw reply in `cache` (when given) as
/// `page-N.txt`. With `replay`, replies come only from the cache and a missing one is an error: no model call is made.
#[cfg(not(target_arch = "wasm32"))]
pub fn convert_pdf(pdf: &[u8], hint: &str, cfg: &crate::ai::AiCfg, cache: Option<&std::path::Path>, replay: bool) -> Result<Vec<Question>, String> {
    let pages = crate::platform::render_pdf_pages(pdf)?;
    if let Some(dir) = cache {
        std::fs::create_dir_all(dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
    }
    if !replay {
        crate::ai::begin_service(cfg, "import", pages.len())?;
    }
    let prompt = crate::ai::prompt_for(hint);
    let mut out = Vec::new();
    for (i, png) in pages.iter().enumerate() {
        let file = cache.map(|d| d.join(format!("page-{}.txt", i + 1)));
        let cached = file.as_ref().and_then(|f| std::fs::read_to_string(f).ok());
        let raw = match cached {
            Some(r) => r,
            None if replay => return Err(format!("replay: page {} has no cached reply", i + 1)),
            None => {
                let r = crate::ai::transcribe_checked(png, "image/png", cfg, &prompt).map_err(|e| format!("page {}: {e}", i + 1))?;
                if let Some(f) = &file {
                    let _ = std::fs::write(f, &r);
                }
                r
            }
        };
        let text = crate::ai::extract_import_text(&raw);
        let doc = if crate::ditto::is_ditto(&text) {
            crate::ditto::import_text(&text, Some(png), i + 1, "converted")?.doc
        } else {
            crate::formats::detect_and_parse(text.as_bytes(), "converted")?
        };
        out.extend(doc.questions);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Answer, ImageRef};

    fn q(stem: &str, kind: Kind) -> Question {
        Question { id: stem.into(), stem: stem.into(), kind, points: None, difficulty: None, topics: vec![], image: None, explanation: None, figure: None, created: 0, modified: 0 }
    }
    fn mc(stem: &str, choices: &[(&str, bool)]) -> Question {
        q(stem, Kind::Mc { answers: choices.iter().map(|(t, c)| Answer { text: (*t).into(), correct: *c, locked: false }).collect() })
    }
    fn blank(stem: &str, accept: &[&str]) -> Question {
        q(stem, Kind::Blank { accept: accept.iter().map(|a| vec![(*a).to_string()]).collect() })
    }

    fn quiz() -> Vec<Question> {
        let mut a = blank("Give the number of bonding and nonbonding electrons for the structure below: ___ ___", &["14", "10"]);
        a.points = Some(2.0);
        a.image = Some(ImageRef { file: "media/x.webp".into(), width: 10, height: 10 });
        vec![
            a,
            mc("Which of the following elements is the most electronegative?", &[("Ge", false), ("Ca", false), ("O", true), ("C", false)]),
            q("The bond between Na and Cl is ionic.", Kind::Tf { correct: true }),
        ]
    }

    #[test]
    fn the_same_test_scores_perfectly() {
        let t = quiz();
        let r = compare(&t, &quiz());
        assert_eq!((r.found, r.extra), (3, 0));
        assert!(r.recall == 1.0 && r.precision == 1.0 && r.kind_accuracy == 1.0 && r.answer_accuracy == 1.0);
        assert!(r.points_accuracy == 1.0 && r.picture_recall == 1.0 && r.mean_stem_similarity > 0.999, "{}", r.summary());
    }

    #[test]
    fn a_dropped_question_a_lost_key_and_a_lost_picture_are_each_counted() {
        let t = quiz();
        let mut g = quiz();
        g.remove(2); // a question the model never read
        g[0].image = None; // a picture that was not kept
        g[0].points = None;
        if let Kind::Blank { accept } = &mut g[0].kind {
            *accept = vec![vec!["14".into()], vec![String::new()]]; // one blank answered, the handwritten one not
        }
        if let Kind::Mc { answers } = &mut g[1].kind {
            answers.iter_mut().for_each(|a| a.correct = a.text == "C"); // the wrong choice marked
        }
        let r = compare(&t, &g);
        assert_eq!((r.found, r.converted_questions), (2, 2));
        assert!((r.recall - 2.0 / 3.0).abs() < 1e-9 && r.precision == 1.0);
        assert_eq!(r.picture_recall, 0.0);
        assert_eq!(r.points_accuracy, 0.0);
        // blank question: 1 of 2 blanks right = 0.5; multiple choice: wrong choice = 0.0
        assert!((r.answer_accuracy - 0.25).abs() < 1e-9, "{}", r.summary());
        assert!(!r.per_question[2].found);
    }

    #[test]
    fn a_wrong_type_scores_no_key_and_extra_questions_lower_precision() {
        let t = vec![mc("Pick the metal.", &[("Na", true), ("Cl", false)])];
        let g = vec![blank("Pick the metal.", &["Na"]), q("Something invented", Kind::Tf { correct: true })];
        let r = compare(&t, &g);
        assert_eq!((r.found, r.extra), (1, 1));
        assert_eq!(r.kind_accuracy, 0.0);
        assert_eq!(r.answer_accuracy, 0.0);
        assert!((r.precision - 0.5).abs() < 1e-9);
    }

    #[test]
    fn alignment_uses_each_converted_question_once_and_ignores_order_and_markup() {
        let t = vec![mc("Name the SO3 compound.", &[("sulfur trioxide", true)]), mc("Name the NO compound.", &[("nitrogen monoxide", true)])];
        let g = vec![mc("name the **NO** compound", &[("Nitrogen monoxide", true)]), mc("Name the SO3 compound!", &[("Sulfur trioxide", true)])];
        assert_eq!(align(&t, &g, 0.45), vec![Some(1), Some(0)]);
        assert_eq!(compare(&t, &g).answer_accuracy, 1.0);
    }

    #[test]
    fn a_multipart_question_is_scored_on_what_each_part_answers() {
        use crate::model::{Part, PartKind};
        let part = |stem: &str, sample: &str| Part { stem: stem.into(), kind: PartKind::Short { sample: sample.into() } };
        let t = vec![q("Draw a Lewis structure and give the shape.", Kind::Multipart { parts: vec![part("a. CO2", "linear nonpolar"), part("b. PF3", "pyramidal polar")] })];
        let blank_key = vec![q("Draw a Lewis structure and give the shape.", Kind::Multipart { parts: vec![part("a. CO2", ""), part("b. PF3", "")] })];
        let half = vec![q("Draw a Lewis structure and give the shape.", Kind::Multipart { parts: vec![part("a. CO2", "linear nonpolar"), part("b. PF3", "")] })];
        assert_eq!(compare(&t, &blank_key).answer_accuracy, 0.0, "right parts, no handwritten answers");
        assert!((compare(&t, &half).answer_accuracy - 0.5).abs() < 1e-9);
        assert_eq!(compare(&t, &t.clone()).answer_accuracy, 1.0);
    }

    #[test]
    fn similarity_is_word_overlap() {
        assert_eq!(similarity("", ""), 1.0);
        assert_eq!(similarity("a b", ""), 0.0);
        assert!((similarity("one two three", "one two four") - 2.0 / 3.0).abs() < 1e-9);
        assert_eq!(similarity("A, b!", "b a"), 1.0);
    }
}
