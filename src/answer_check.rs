//! The answer check: the AI solves each question on its own, blind to the answer the bank already has, and the two
//! are compared. A question whose key the AI reproduces is *verified*, one it disagrees with is *flagged* for the
//! teacher, and one with no key gets the AI's answer as a *suggestion*. The AI never writes into the key: the key
//! stays whatever the source said, so a wrong guess cannot pass for the teacher's answer.
//!
//! Questions that need a picture, and matching questions, are not checked (the model sees only text).
use crate::import_eval::similarity;
use crate::model::{is_keyless, Kind, PartKind, Question};
use serde::Serialize;
use serde_json::{json, Value};

/// How many questions go in one model call.
pub const CHUNK: usize = 20;

/// What the check made of one question (or one part of a multipart question).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Verdict {
    /// The AI's own answer matches the key.
    Agrees,
    /// The AI's own answer is different from the key, and the AI is sure.
    Differs,
    /// The question has no key; the AI's answer is offered as a suggestion.
    Suggests,
    /// The AI could not settle on an answer, or gave none.
    Unsure,
    /// Not checked: it needs its picture, or its type is not supported.
    Skipped,
}

#[derive(Debug, Clone, Serialize)]
pub struct Check {
    /// 1-based question number, and the 1-based part for a multipart question.
    pub question: usize,
    pub part: Option<usize>,
    pub verdict: Verdict,
    /// The AI's answer, readable.
    pub ai: String,
    /// The key it was compared with ("" when there is none).
    pub key: String,
    /// The AI's structured answer, kept so the teacher can accept it into the key.
    #[serde(skip)]
    pub solved: Option<Solved>,
}

/// What the AI must answer, shaped like the question.
#[derive(Debug, Clone)]
pub enum Shape {
    Mc(Vec<String>),
    Tf,
    Blank(usize),
    Short,
}

/// The answer a question already holds.
#[derive(Debug, Clone, PartialEq)]
pub enum Key {
    /// Texts of the choices marked correct.
    Mc(Vec<String>),
    Tf(bool),
    Blank(Vec<Vec<String>>),
    Short(String),
}

impl Key {
    fn show(&self) -> String {
        match self {
            Key::Mc(c) => c.join(" / "),
            Key::Tf(b) => b.to_string(),
            Key::Blank(b) => b.iter().map(|a| a.join(" OR ")).collect::<Vec<_>>().join("; "),
            Key::Short(s) => s.clone(),
        }
    }
}

/// One thing to be answered: a whole question, or one part of a multipart.
#[derive(Debug, Clone)]
pub struct Item {
    pub question: usize,
    pub part: Option<usize>,
    pub text: String,
    pub shape: Shape,
    /// `None` when the bank holds no answer for it.
    pub key: Option<Key>,
}

impl Item {
    pub fn id(&self) -> String {
        match self.part {
            Some(p) => format!("{}.{}", self.question, p),
            None => self.question.to_string(),
        }
    }
}

fn mc_key(answers: &[crate::model::Answer]) -> Option<Key> {
    let c: Vec<String> = answers.iter().filter(|a| a.correct).map(|a| a.text.clone()).collect();
    (!c.is_empty()).then_some(Key::Mc(c))
}

/// The checkable items of `questions`, in order. Skipped ones are left out (see [`skipped`]).
pub fn items(questions: &[Question]) -> Vec<Item> {
    let mut out = Vec::new();
    for (i, q) in questions.iter().enumerate() {
        if q.image.is_some() || q.figure.is_some() {
            continue;
        }
        let n = i + 1;
        let keyless = is_keyless(q);
        match &q.kind {
            Kind::Mc { answers } => {
                let texts = answers.iter().map(|a| a.text.clone()).collect();
                out.push(Item { question: n, part: None, text: q.stem.clone(), shape: Shape::Mc(texts), key: if keyless { None } else { mc_key(answers) } });
            }
            Kind::Tf { correct } => out.push(Item { question: n, part: None, text: q.stem.clone(), shape: Shape::Tf, key: (!keyless).then_some(Key::Tf(*correct)) }),
            Kind::Blank { accept } => {
                let blanks = q.stem.matches("___").count().max(accept.len());
                let has = accept.iter().any(|a| a.iter().any(|x| !x.trim().is_empty()));
                out.push(Item { question: n, part: None, text: q.stem.clone(), shape: Shape::Blank(blanks), key: (has && !keyless).then(|| Key::Blank(accept.clone())) });
            }
            Kind::Short { sample } => out.push(Item { question: n, part: None, text: q.stem.clone(), shape: Shape::Short, key: (!sample.trim().is_empty() && !keyless).then(|| Key::Short(sample.clone())) }),
            Kind::Matching { .. } => {}
            Kind::Multipart { parts } => {
                for (j, p) in parts.iter().enumerate() {
                    let text = format!("{}\n{}", q.stem, p.stem);
                    match &p.kind {
                        PartKind::Mc { answers } => {
                            let texts = answers.iter().map(|a| a.text.clone()).collect();
                            out.push(Item { question: n, part: Some(j + 1), text, shape: Shape::Mc(texts), key: if keyless { None } else { mc_key(answers) } });
                        }
                        PartKind::Tf { correct } => out.push(Item { question: n, part: Some(j + 1), text, shape: Shape::Tf, key: (!keyless).then_some(Key::Tf(*correct)) }),
                        PartKind::Short { sample } => out.push(Item { question: n, part: Some(j + 1), text, shape: Shape::Short, key: (!sample.trim().is_empty() && !keyless).then(|| Key::Short(sample.clone())) }),
                    }
                }
            }
        }
    }
    out
}

/// Question numbers the check cannot look at: a picture is needed, or the type is matching.
pub fn skipped(questions: &[Question]) -> Vec<usize> {
    questions.iter().enumerate().filter(|(_, q)| q.image.is_some() || q.figure.is_some() || matches!(q.kind, Kind::Matching { .. })).map(|(i, _)| i + 1).collect()
}

const INSTRUCTIONS: &str = "You are checking a test. Solve every question below yourself, carefully, from your own knowledge. \
You are NOT shown any answer key: work each one out. Reply with ONLY a JSON object {\"answers\": [...]}, one entry per question, \
using each question's \"id\" exactly as given.\n\
- multiple-choice: {\"id\": \"3\", \"choice\": [\"B\"], \"sure\": true}. Several letters when several choices are right.\n\
- true-false: {\"id\": \"4\", \"answer\": true, \"sure\": true}\n\
- fill-in-the-blank: {\"id\": \"5\", \"blanks\": [\"first blank\", \"second blank\"], \"sure\": true}, one string per ___ in reading order.\n\
- short-answer: {\"id\": \"6\", \"answer\": \"the answer, with units, in as few words as possible\", \"sure\": true}\n\
Set \"sure\" to false only when the question is ambiguous, has a missing part, or you cannot settle on one answer. \
Do not explain. Do not skip a question.";

fn letter(i: usize) -> char {
    (b'A' + i as u8) as char
}

/// The prompt for one chunk of items: the questions only, never their keys.
pub fn prompt_for(chunk: &[Item]) -> String {
    let list: Vec<Value> = chunk
        .iter()
        .map(|it| match &it.shape {
            Shape::Mc(c) => json!({"id": it.id(), "type": "multiple-choice", "question": it.text, "choices": c.iter().enumerate().map(|(i, t)| format!("{}. {}", letter(i), t)).collect::<Vec<_>>()}),
            Shape::Tf => json!({"id": it.id(), "type": "true-false", "question": it.text}),
            Shape::Blank(n) => json!({"id": it.id(), "type": "fill-in-the-blank", "question": it.text, "blanks": n}),
            Shape::Short => json!({"id": it.id(), "type": "short-answer", "question": it.text}),
        })
        .collect();
    format!("{INSTRUCTIONS}\n\nQuestions:\n{}", serde_json::to_string(&list).unwrap_or_default())
}

/// What the AI answered for one item.
#[derive(Debug, Clone, Default)]
pub struct Solved {
    pub id: String,
    pub choice: Vec<usize>,
    pub tf: Option<bool>,
    pub blanks: Vec<String>,
    pub text: String,
    pub sure: bool,
}

fn as_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.trim().to_string(),
        Value::Null => String::new(),
        other => other.to_string().trim_matches('"').to_string(),
    }
}

fn letters(v: &Value, n: usize) -> Vec<usize> {
    let items: Vec<&Value> = match v {
        Value::Array(a) => a.iter().collect(),
        other => vec![other],
    };
    let mut out = Vec::new();
    for x in items {
        let s = as_text(x);
        for tok in s.split(|c: char| !c.is_alphanumeric()).filter(|t| !t.is_empty()) {
            let up = tok.to_uppercase();
            let mut cs = up.chars();
            if let (Some(c), None) = (cs.next(), cs.next()) {
                if c.is_ascii_uppercase() && ((c as u8 - b'A') as usize) < n {
                    out.push((c as u8 - b'A') as usize);
                }
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// The answers in a model reply (tolerates a code fence, a bare list and a root closed early).
pub fn parse_reply(text: &str, items: &[Item]) -> Vec<Solved> {
    let Some(v) = crate::ditto::json_object(text) else { return Vec::new() };
    let list = v.get("answers").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut out = Vec::new();
    for a in &list {
        let id = a.get("id").map(as_text).unwrap_or_default();
        let Some(it) = items.iter().find(|it| it.id() == id) else { continue };
        let n = if let Shape::Mc(c) = &it.shape { c.len() } else { 0 };
        let sure = a.get("sure").and_then(Value::as_bool).unwrap_or(true);
        let mut s = Solved { id, sure, ..Default::default() };
        match &it.shape {
            Shape::Mc(_) => s.choice = letters(a.get("choice").or_else(|| a.get("choices")).or_else(|| a.get("answer")).unwrap_or(&Value::Null), n),
            Shape::Tf => {
                s.tf = match a.get("answer") {
                    Some(Value::Bool(b)) => Some(*b),
                    Some(Value::String(t)) => match t.trim().to_lowercase().as_str() {
                        "true" | "t" | "yes" => Some(true),
                        "false" | "f" | "no" => Some(false),
                        _ => None,
                    },
                    _ => None,
                }
            }
            Shape::Blank(_) => {
                s.blanks = match a.get("blanks").or_else(|| a.get("answer")) {
                    Some(Value::Array(b)) => b.iter().map(as_text).collect(),
                    Some(other) => vec![as_text(other)],
                    None => Vec::new(),
                }
            }
            Shape::Short => s.text = a.get("answer").map(as_text).unwrap_or_default(),
        }
        out.push(s);
    }
    out
}

/// Plain form for comparing: lower case, Unicode sub/superscripts and arrows folded, punctuation spaced out.
fn norm(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        let c = match c {
            '₀' | '⁰' => '0',
            '₁' | '¹' => '1',
            '₂' | '²' => '2',
            '₃' | '³' => '3',
            '₄' | '⁴' => '4',
            '₅' | '⁵' => '5',
            '₆' | '⁶' => '6',
            '₇' | '⁷' => '7',
            '₈' | '⁸' => '8',
            '₉' | '⁹' => '9',
            '→' | '⟶' => '>',
            '−' | '–' | '—' => '-',
            other => other,
        };
        out.extend(c.to_lowercase());
    }
    out
}

/// Every number in `s`, sorted (`2H2O` has the numbers 2, 2).
fn numbers(s: &str) -> Vec<String> {
    let s = norm(s);
    let mut cs: Vec<char> = s.chars().collect();
    // "1,102.50" is one number: a comma between a digit and exactly three digits is a thousands separator.
    let mut k = 1;
    while k + 3 < cs.len() {
        if cs[k] == ',' && cs[k - 1].is_ascii_digit() && cs[k + 1..k + 4].iter().all(char::is_ascii_digit) && cs.get(k + 4).is_none_or(|c| !c.is_ascii_digit()) {
            cs.remove(k);
        } else {
            k += 1;
        }
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        if cs[i].is_ascii_digit() {
            let mut j = i;
            while j < cs.len() && (cs[j].is_ascii_digit() || (cs[j] == '.' && j + 1 < cs.len() && cs[j + 1].is_ascii_digit())) {
                j += 1;
            }
            let mut n: String = cs[i..j].iter().collect();
            if n.contains('.') {
                n = n.trim_end_matches('0').trim_end_matches('.').to_string();
            }
            let neg = i > 0 && cs[i - 1] == '-' && (i < 2 || !cs[i - 2].is_alphanumeric());
            out.push(if neg { format!("-{n}") } else { n });
            i = j;
        } else {
            i += 1;
        }
    }
    out.sort();
    out
}

/// Whether two short answers say the same thing: the same numbers when there are any, else the same words.
pub fn same_answer(key: &str, ai: &str) -> bool {
    let (kn, an) = (numbers(key), numbers(ai));
    if !kn.is_empty() || !an.is_empty() {
        return kn == an;
    }
    let (k, a) = (norm(key), norm(ai));
    let (k, a) = (k.trim(), a.trim());
    if k.is_empty() || a.is_empty() {
        return false;
    }
    k == a || a.contains(k) || k.contains(a) || similarity(k, a) >= 0.6
}

/// Compares what the AI answered with `key`: `Some(true)` agree, `Some(false)` differ, `None` for no answer at all.
fn matches(shape: &Shape, key: &Key, s: &Solved) -> Option<bool> {
    match (shape, key) {
        (Shape::Mc(choices), Key::Mc(correct)) => {
            if s.choice.is_empty() {
                return None;
            }
            let mut got: Vec<String> = s.choice.iter().filter_map(|i| choices.get(*i)).map(|t| norm(t)).collect();
            let mut want: Vec<String> = correct.iter().map(|t| norm(t)).collect();
            got.sort();
            want.sort();
            Some(got == want)
        }
        (Shape::Tf, Key::Tf(b)) => s.tf.map(|t| t == *b),
        (Shape::Blank(_), Key::Blank(accept)) => {
            if s.blanks.iter().all(|b| b.trim().is_empty()) {
                return None;
            }
            let wanted: Vec<&Vec<String>> = accept.iter().filter(|a| a.iter().any(|x| !x.trim().is_empty())).collect();
            Some(wanted.len() <= s.blanks.len() && wanted.iter().enumerate().all(|(i, alts)| alts.iter().any(|x| same_answer(x, &s.blanks[i]))))
        }
        (Shape::Short, Key::Short(k)) => (!s.text.trim().is_empty()).then(|| same_answer(k, &s.text)),
        _ => None,
    }
}

/// The AI's answer for `it`, readable.
fn show(it: &Item, s: &Solved) -> String {
    match &it.shape {
        Shape::Mc(c) => s.choice.iter().filter_map(|i| c.get(*i)).cloned().collect::<Vec<_>>().join(" / "),
        Shape::Tf => s.tf.map(|b| b.to_string()).unwrap_or_default(),
        Shape::Blank(_) => s.blanks.join("; "),
        Shape::Short => s.text.clone(),
    }
}

/// The verdict for one item given the key to hold it against (`None`: no key) and the AI's answer (`None`: it gave none).
pub fn judge(it: &Item, key: Option<&Key>, s: Option<&Solved>) -> Check {
    let mut c = Check { question: it.question, part: it.part, verdict: Verdict::Unsure, ai: String::new(), key: key.map(Key::show).unwrap_or_default(), solved: None };
    let Some(s) = s else { return c };
    c.ai = show(it, s);
    c.solved = Some(s.clone());
    let Some(key) = key else {
        c.verdict = if c.ai.trim().is_empty() || !s.sure { Verdict::Unsure } else { Verdict::Suggests };
        return c;
    };
    c.verdict = match matches(&it.shape, key, s) {
        Some(true) => Verdict::Agrees,
        Some(false) if s.sure => Verdict::Differs,
        _ => Verdict::Unsure,
    };
    c
}

/// Judges every item against the key it holds, from the replies already parsed.
pub fn judge_all(its: &[Item], solved: &[Solved]) -> Vec<Check> {
    its.iter().map(|it| judge(it, it.key.as_ref(), solved.iter().find(|s| s.id == it.id()))).collect()
}

/// Runs the check against the configured provider: the questions in chunks, one model call each, a retry when a reply is
/// not JSON. On the hosted AI, call `begin_service(cfg, "answer_key", 0)` first.
#[cfg(not(target_arch = "wasm32"))]
pub fn run(questions: &[Question], cfg: &crate::ai::AiCfg, cache: Option<&std::path::Path>) -> Result<Vec<Check>, String> {
    let its = items(questions);
    let mut solved = Vec::new();
    for (n, chunk) in its.chunks(CHUNK).enumerate() {
        let file = cache.map(|d| d.join(format!("chunk-{}.txt", n + 1)));
        let prompt = prompt_for(chunk);
        let mut reply = match file.as_ref().and_then(|f| std::fs::read_to_string(f).ok()) {
            Some(r) => r,
            None => {
                let r = crate::ai::chat_turns(cfg, &[crate::ai::Turn::new("user", prompt.clone())], 0.0)?;
                if let Some(f) = &file {
                    let _ = std::fs::write(f, &r);
                }
                r
            }
        };
        if parse_reply(&reply, chunk).is_empty() {
            let again = format!("{prompt}\n\nYour previous reply could not be read. Reply with only the JSON object {{\"answers\": [...]}}.");
            if let Ok(r) = crate::ai::chat_turns(cfg, &[crate::ai::Turn::new("user", again)], 0.0) {
                reply = r;
            }
        }
        solved.extend(parse_reply(&reply, chunk));
    }
    Ok(judge_all(&its, &solved))
}

/// Writes the AI's answer in `c` into the question's key, because the teacher accepted it. Returns false when there is nothing
/// to write. A question imported without a key stops being marked keyless once it has one.
pub fn accept(q: &mut Question, c: &Check) -> bool {
    let Some(s) = &c.solved else { return false };
    let done = match (&mut q.kind, c.part) {
        (Kind::Mc { answers }, None) => set_mc(answers, s),
        (Kind::Tf { correct }, None) => s.tf.map(|b| *correct = b).is_some(),
        (Kind::Blank { accept }, None) => {
            let filled = s.blanks.iter().any(|b| !b.trim().is_empty());
            if filled {
                *accept = s.blanks.iter().map(|b| vec![b.clone()]).collect();
            }
            filled
        }
        (Kind::Short { sample }, None) => {
            let ok = !s.text.trim().is_empty();
            if ok {
                *sample = s.text.clone();
            }
            ok
        }
        (Kind::Multipart { parts }, Some(p)) => match parts.get_mut(p.wrapping_sub(1)).map(|x| &mut x.kind) {
            Some(PartKind::Mc { answers }) => set_mc(answers, s),
            Some(PartKind::Tf { correct }) => s.tf.map(|b| *correct = b).is_some(),
            Some(PartKind::Short { sample }) if !s.text.trim().is_empty() => {
                *sample = s.text.clone();
                true
            }
            _ => false,
        },
        _ => false,
    };
    if done && is_keyless(q) {
        q.explanation = None;
    }
    done
}

fn set_mc(answers: &mut [crate::model::Answer], s: &Solved) -> bool {
    if s.choice.is_empty() || s.choice.iter().any(|i| *i >= answers.len()) {
        return false;
    }
    for (i, a) in answers.iter_mut().enumerate() {
        a.correct = s.choice.contains(&i);
    }
    true
}

/// A check running in the background (a thread on the desktop, a promise in the browser), polled every frame.
#[derive(Default)]
pub struct Job {
    rx: Option<std::sync::mpsc::Receiver<Result<Vec<Check>, String>>>,
}

impl Job {
    pub fn busy(&self) -> bool {
        self.rx.is_some()
    }

    /// Starts checking `questions`. On the hosted AI this first opens an `answer_key` service session (1 credit).
    pub fn start(&mut self, questions: Vec<Question>, cfg: crate::ai::AiCfg, ctx: eframe::egui::Context) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.rx = Some(rx);
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            let r = crate::ai::begin_service(&cfg, "answer_key", 0).and_then(|_| run(&questions, &cfg, None));
            let _ = tx.send(r);
            ctx.request_repaint();
        });
        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(async move {
            let r = match crate::ai::begin_service_async(&cfg, "answer_key", 0).await {
                Ok(_) => run_async(&questions, &cfg).await,
                Err(e) => Err(e),
            };
            let _ = tx.send(r);
            ctx.request_repaint();
        });
    }

    /// The finished result, if the job is done.
    pub fn poll(&mut self) -> Option<Result<Vec<Check>, String>> {
        let rx = self.rx.as_ref()?;
        match rx.try_recv() {
            Ok(r) => {
                self.rx = None;
                Some(r)
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.rx = None;
                Some(Err("the check ended without a result".to_string()))
            }
        }
    }
}

/// The browser version of [`run`].
#[cfg(target_arch = "wasm32")]
pub async fn run_async(questions: &[Question], cfg: &crate::ai::AiCfg) -> Result<Vec<Check>, String> {
    let its = items(questions);
    let mut solved = Vec::new();
    for chunk in its.chunks(CHUNK) {
        let prompt = prompt_for(chunk);
        let mut reply = crate::ai::ask_images_async(&[], cfg, &prompt, 0.0).await?;
        if parse_reply(&reply, chunk).is_empty() {
            let again = format!("{prompt}\n\nYour previous reply could not be read. Reply with only the JSON object {{\"answers\": [...]}}.");
            if let Ok(r) = crate::ai::ask_images_async(&[], cfg, &again, 0.0).await {
                reply = r;
            }
        }
        solved.extend(parse_reply(&reply, chunk));
    }
    Ok(judge_all(&its, &solved))
}

/// Writes the prompt of each chunk of `questions` as `chunk-N.prompt.txt` in `dir` (for a tool that holds the sign-in),
/// returning how many chunks there are.
pub fn dump_prompts(questions: &[Question], dir: &std::path::Path) -> Result<usize, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
    let its = items(questions);
    for (n, chunk) in its.chunks(CHUNK).enumerate() {
        std::fs::write(dir.join(format!("chunk-{}.prompt.txt", n + 1)), prompt_for(chunk)).map_err(|e| e.to_string())?;
    }
    Ok(its.chunks(CHUNK).count())
}

/// Reads the replies cached as `chunk-N.txt` in `dir` (no model call) and measures them against `truth`.
pub fn replay_quality(truth: &[Question], dir: &std::path::Path) -> Result<Quality, String> {
    let its = items(truth);
    let mut solved = Vec::new();
    for (n, chunk) in its.chunks(CHUNK).enumerate() {
        let f = dir.join(format!("chunk-{}.txt", n + 1));
        let reply = std::fs::read_to_string(&f).map_err(|e| format!("cannot read {}: {e}", f.display()))?;
        solved.extend(parse_reply(&reply, chunk));
    }
    if crate::env_var("VERSHION_CHECK_VERBOSE").as_deref() == Ok("1") {
        for c in judge_all(&its, &solved).iter().filter(|c| c.verdict != Verdict::Agrees) {
            eprintln!("  {:?} q{}{} | AI: {} | key: {}", c.verdict, c.question, c.part.map(|p| format!(".{p}")).unwrap_or_default(), c.ai, c.key);
        }
    }
    Ok(quality(truth, &solved))
}

/// Measured quality of a check on a bank whose answers are known to be right.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Quality {
    pub items: usize,
    pub keyed: usize,
    /// The true key held against the AI's answer.
    pub verified: usize,
    pub false_alarms: usize,
    pub unsure: usize,
    /// The key made wrong on purpose.
    pub caught: usize,
    pub missed: usize,
    /// Items with the key removed: the suggestion is right when it agrees with the real key.
    pub suggestions_right: usize,
    pub suggestions_made: usize,
}

impl Quality {
    pub fn summary(&self) -> String {
        let pct = |n: usize, d: usize| if d == 0 { 0.0 } else { 100.0 * n as f64 / d as f64 };
        format!(
            "{} items | right key verified {:.1}% (false alarm {:.1}%, unsure {:.1}%) | wrong key caught {:.1}% (missed {:.1}%) | suggested answer right {:.1}%",
            self.items,
            pct(self.verified, self.keyed),
            pct(self.false_alarms, self.keyed),
            pct(self.unsure, self.keyed),
            pct(self.caught, self.keyed),
            pct(self.missed, self.keyed),
            pct(self.suggestions_right, self.keyed),
        )
    }
    /// One number: the mean of verified, caught and right-suggestion rates, 0..=100.
    pub fn score(&self) -> f64 {
        if self.keyed == 0 {
            return 0.0;
        }
        let d = self.keyed as f64;
        100.0 * (self.verified as f64 + self.caught as f64 + self.suggestions_right as f64) / (3.0 * d)
    }
}

/// A key that is certainly wrong for `it`: the next choice, the other truth value, or another item's answer of the same shape.
fn wrong_key(its: &[Item], at: usize) -> Option<Key> {
    let it = &its[at];
    match (&it.shape, it.key.as_ref()?) {
        (Shape::Mc(c), Key::Mc(correct)) => {
            let first = c.iter().position(|t| correct.contains(t))?;
            Some(Key::Mc(vec![c[(first + 1) % c.len()].clone()]))
        }
        (_, Key::Tf(b)) => Some(Key::Tf(!b)),
        (Shape::Blank(_), Key::Blank(own)) => its.iter().cycle().skip(at + 1).take(its.len()).find_map(|o| match o.key.as_ref() {
            Some(Key::Blank(k)) if k != own && k.len() == own.len() => Some(Key::Blank(k.clone())),
            _ => None,
        })
        .or_else(|| Some(Key::Blank(vec![vec!["unrelated".to_string()]; own.len()]))),
        (Shape::Short, Key::Short(own)) => its.iter().cycle().skip(at + 1).take(its.len()).find_map(|o| match o.key.as_ref() {
            Some(Key::Short(k)) if !same_answer(own, k) => Some(Key::Short(k.clone())),
            _ => None,
        })
        .or_else(|| Some(Key::Short("unrelated 987".to_string()))),
        _ => None,
    }
}

/// Scores the AI's answers (`solved`) against a bank whose keys are all right: the right key must verify, a wrong key
/// must be caught, and the answer given for a missing key must be the right one.
pub fn quality(truth: &[Question], solved: &[Solved]) -> Quality {
    let its = items(truth);
    let mut q = Quality { items: its.len(), ..Default::default() };
    for (i, it) in its.iter().enumerate() {
        let Some(key) = it.key.as_ref() else { continue };
        q.keyed += 1;
        let s = solved.iter().find(|s| s.id == it.id());
        let right = judge(it, Some(key), s);
        match right.verdict {
            Verdict::Agrees => q.verified += 1,
            Verdict::Differs => q.false_alarms += 1,
            _ => q.unsure += 1,
        }
        if let Some(w) = wrong_key(&its, i) {
            if judge(it, Some(&w), s).verdict == Verdict::Differs {
                q.caught += 1;
            } else {
                q.missed += 1;
            }
        } else {
            q.missed += 1;
        }
        let keyless = judge(&Item { key: None, ..it.clone() }, None, s);
        if keyless.verdict == Verdict::Suggests {
            q.suggestions_made += 1;
            if right.verdict == Verdict::Agrees {
                q.suggestions_right += 1;
            }
        }
    }
    q
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Answer;

    fn q(stem: &str, kind: Kind) -> Question {
        crate::formats::make_question(stem.into(), kind)
    }
    fn mc(stem: &str, c: &[(&str, bool)]) -> Question {
        q(stem, Kind::Mc { answers: c.iter().map(|(t, ok)| Answer { text: t.to_string(), correct: *ok, locked: false }).collect() })
    }
    fn bank() -> Vec<Question> {
        vec![
            mc("Closest planet?", &[("Venus", false), ("Mercury", true), ("Mars", false)]),
            q("Water boils at 100 C.", Kind::Tf { correct: true }),
            q("Symbol for gold: ___", Kind::Blank { accept: vec![vec!["Au".into()]] }),
            q("15% of 240?", Kind::Short { sample: "36".into() }),
        ]
    }

    #[test]
    fn the_prompt_never_carries_the_key() {
        let its = items(&bank());
        let p = prompt_for(&its);
        assert!(p.contains("Mercury") && p.contains("Closest planet?"));
        assert!(!p.contains("correct") && !p.contains("\"Au\"") && !p.contains("\"36\""), "no key in the prompt: {p}");
    }

    #[test]
    fn a_matching_answer_is_verified_and_a_different_one_is_flagged() {
        let its = items(&bank());
        let reply = r#"```json
{"answers":[{"id":"1","choice":["B"]},{"id":"2","answer":false},{"id":"3","blanks":["Au"]},{"id":"4","answer":"36 (units)"}]}
```"#;
        let got = judge_all(&its, &parse_reply(reply, &its));
        let v: Vec<Verdict> = got.iter().map(|c| c.verdict).collect();
        assert_eq!(v, vec![Verdict::Agrees, Verdict::Differs, Verdict::Agrees, Verdict::Agrees]);
        assert_eq!(got[1].ai, "false");
    }

    #[test]
    fn an_unsure_model_never_raises_an_alarm_and_a_missing_key_gets_a_suggestion() {
        let mut b = bank();
        let its = items(&b);
        let got = judge_all(&its, &parse_reply(r#"{"answers":[{"id":"1","choice":["A"],"sure":false}]}"#, &its));
        assert_eq!(got[0].verdict, Verdict::Unsure, "unsure is not a flag");
        assert_eq!(got[1].verdict, Verdict::Unsure, "no reply at all is unsure too");
        b[3].kind = Kind::Short { sample: String::new() };
        let its = items(&b);
        let got = judge_all(&its, &parse_reply(r#"{"answers":[{"id":"4","answer":"36"}]}"#, &its));
        assert_eq!((got[3].verdict, got[3].ai.as_str(), got[3].key.as_str()), (Verdict::Suggests, "36", ""));
    }

    #[test]
    fn a_question_that_needs_its_picture_is_skipped_and_parts_are_checked_one_by_one() {
        let mut b = bank();
        b.push(q("Name the shape.", Kind::Short { sample: "circle".into() }));
        b[4].image = Some(crate::model::ImageRef { file: "x.png".into(), width: 1, height: 1 });
        b.push(q("Ball dropped.", Kind::Multipart { parts: vec![crate::model::Part { stem: "a. speed after 3 s".into(), kind: PartKind::Short { sample: "30 m/s".into() } }] }));
        assert_eq!(skipped(&b), vec![5]);
        let its = items(&b);
        assert_eq!(its.last().unwrap().id(), "6.1");
        assert!(its.iter().all(|i| i.question != 5));
    }

    #[test]
    fn accepting_a_suggestion_writes_it_into_the_key_and_clears_the_keyless_mark() {
        let mut b = vec![
            mc("Closest planet?", &[("Venus", false), ("Mercury", false), ("Mars", false)]),
            q("Ball dropped.", Kind::Multipart { parts: vec![crate::model::Part { stem: "a. speed after 3 s".into(), kind: PartKind::Short { sample: String::new() } }] }),
        ];
        b[0].explanation = Some(crate::model::KEYLESS_SENTINEL.into());
        let its = items(&b);
        let got = judge_all(&its, &parse_reply(r#"{"answers":[{"id":"1","choice":["B"]},{"id":"2.1","answer":"30 m/s"}]}"#, &its));
        assert_eq!((got[0].verdict, got[1].verdict), (Verdict::Suggests, Verdict::Suggests));
        assert!(accept(&mut b[0], &got[0]) && accept(&mut b[1], &got[1]));
        let Kind::Mc { answers } = &b[0].kind else { panic!() };
        assert_eq!(answers.iter().map(|a| a.correct).collect::<Vec<_>>(), vec![false, true, false]);
        assert!(!is_keyless(&b[0]), "a question with a key is not keyless any more");
        let Kind::Multipart { parts } = &b[1].kind else { panic!() };
        assert!(matches!(&parts[0].kind, PartKind::Short { sample } if sample == "30 m/s"));
        let none = Check { question: 1, part: None, verdict: Verdict::Unsure, ai: String::new(), key: String::new(), solved: None };
        assert!(!accept(&mut b[0], &none), "no answer, nothing to write");
    }

    #[test]
    fn short_answers_compare_numbers_then_words() {
        assert!(same_answer("60 km/h", "60"));
        assert!(same_answer("x = 2 and x = 3", "x=3, x=2"));
        assert!(same_answer("2H2 + O2 -> 2H2O", "2H₂ + O₂ → 2H₂O"));
        assert!(!same_answer("36", "63"));
        assert!(same_answer("William Shakespeare", "Shakespeare"));
        assert!(!same_answer("mitochondrion", "ribosome"));
        assert!(same_answer("0.50", "0.5"));
        assert!(same_answer("$1102.50", "$1,102.50"));
        assert!(same_answer("7200 s", "7,200 seconds"));
        assert!(!same_answer("4, 5", "4,567"));
    }

    #[test]
    fn quality_rewards_catching_a_wrong_key_and_penalises_a_false_alarm() {
        let truth = bank();
        let its = items(&truth);
        let all_right = parse_reply(r#"{"answers":[{"id":"1","choice":["B"]},{"id":"2","answer":true},{"id":"3","blanks":["Au"]},{"id":"4","answer":"36"}]}"#, &its);
        let r = quality(&truth, &all_right);
        assert_eq!((r.keyed, r.verified, r.false_alarms, r.caught, r.suggestions_right), (4, 4, 0, 4, 4), "{}", r.summary());
        assert!((r.score() - 100.0).abs() < 1e-9);
        let one_wrong = parse_reply(r#"{"answers":[{"id":"1","choice":["A"]},{"id":"2","answer":true},{"id":"3","blanks":["Au"]},{"id":"4","answer":"36"}]}"#, &its);
        let r = quality(&truth, &one_wrong);
        assert_eq!((r.verified, r.false_alarms), (3, 1));
    }
}
