//! Question editor (V4): the `Draft` state the bank screen opens, validation, and the
//! centered modal editor drawn over the bank list (one column: a metadata strip for type,
//! difficulty, topics and points, then the stem, the per-type answers with tappable
//! correct-circles, and collapsible explanation and image sections). Supports all six kinds (MC/MA, True/False, Matching, Fill in the blank,
//! Short answer, Multipart). `bankui` owns the list underneath; this module owns the modal.

use std::collections::HashMap;

use egui::{
    Align, Button, Color32, CornerRadius, Frame, Key, KeyboardShortcut, Layout, Margin, Modifiers,
    Pos2, Rect, RichText, Sense, Stroke, StrokeKind, TextEdit, Ui, Vec2,
};
// egui's widget id type vs the store's question id (a String).
use egui::Id as UiId;

use crate::model::{
    Answer, Difficulty, Id, ImageRef, Kind, Pair, Part, PartKind, QKind, Question,
};
use crate::{kit, screens, theme, App};

/// Type selector order and the short tags the V4 board uses.
const KINDS: [(QKind, &str); 6] = [
    (QKind::Mc, "MC"),
    (QKind::Tf, "TF"),
    (QKind::Multipart, "MULTI"),
    (QKind::Matching, "MATCH"),
    (QKind::Blank, "FILL"),
    (QKind::Short, "SHORT"),
];

/// Short tag for a question type ("MC", "TF", "MULTI", "MATCH", "FILL", "SHORT").
pub fn type_tag(k: QKind) -> &'static str {
    KINDS.iter().find(|(q, _)| *q == k).map(|(_, t)| *t).unwrap_or("?")
}

fn empty_answer() -> Answer {
    Answer {
        text: String::new(),
        correct: false,
        locked: false,
    }
}

fn empty_answers(n: usize) -> Vec<Answer> {
    (0..n).map(|_| empty_answer()).collect()
}

/// How a blank is made; the stem hint and the "no blank yet" check both say it the same way.
pub const BLANK_HOW: &str = "select a word in the question and press ___ Blank or Ctrl+Shift+K (or type ___)";

/// Number of "___" blanks in a Fill-in-the-blank stem.
fn blank_count(stem: &str) -> usize {
    stem.matches("___").count()
}

/// Splits one editor row ("red OR dark red") into accepted alternatives.
fn split_alts(s: &str) -> Vec<String> {
    s.split(" OR ")
        .map(|a| a.trim().to_string())
        .filter(|a| !a.is_empty())
        .collect()
}

/// Small red label.
fn red(ui: &mut Ui, msg: &str) {
    ui.label(RichText::new(msg).color(theme::bad()).size(12.0));
}

/// Small ghost pill used inside the editor ("+ Add answer", "Attach image...").
fn ghost_button(ui: &mut Ui, label: &str) -> egui::Response {
    let r = ui.add(
        Button::new(RichText::new(label).family(theme::bold_family()).size(12.5).color(theme::accent_col()))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, theme::border()))
            .corner_radius(CornerRadius::same(255))
            .min_size(Vec2::new(0.0, 28.0)),
    );
    if r.has_focus() {
        ui.painter().rect_stroke(r.rect, CornerRadius::same(255), Stroke::new(2.0, theme::accent_col()), StrokeKind::Outside);
    }
    r
}

// ---------- keyboard flow through the answer rows ----------

/// A key the answer rows route themselves (taken before the text field sees it).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Nav {
    Enter,
    Tab,
    BackTab,
    /// Backspace in a row that has nothing in it.
    Backspace,
    /// Alt+Enter: mark the row correct (or not).
    MarkCorrect,
    MoveUp,
    MoveDown,
}

/// What a routed key does to a ring of `len` rows when the focus is on row `idx`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Step {
    Goto(usize),
    /// Add a row after the last and focus it.
    Add,
    /// Focus the "+ Add" button after the last row.
    AddButton,
    /// Walk back out of the first row (to the field before the list).
    Before,
    /// Tab past the last row when there is no Add button: on to the next control.
    Leave,
    Remove(usize),
    Stay,
}

/// Pure routing decision: Enter on the last row adds one, Enter elsewhere and Tab go to the next
/// row (Tab on the last goes to the Add button), Shift+Tab goes back (from the first row to the
/// field before the list), Backspace on an empty removable row removes it.
fn nav_step(nav: Nav, idx: usize, len: usize, can_add: bool, removable: bool) -> Step {
    match nav {
        Nav::Enter if idx + 1 < len => Step::Goto(idx + 1),
        Nav::Enter if can_add => Step::Add,
        Nav::Tab if idx + 1 < len => Step::Goto(idx + 1),
        Nav::Tab if can_add => Step::AddButton,
        Nav::Tab => Step::Leave,
        Nav::BackTab if idx > 0 => Step::Goto(idx - 1),
        Nav::BackTab => Step::Before,
        Nav::Backspace if removable => Step::Remove(idx),
        _ => Step::Stay,
    }
}

/// Where the focus goes after row `idx` of `len` is removed: the row before it, else the new first.
fn after_remove(idx: usize) -> usize {
    idx.saturating_sub(1)
}

/// Takes the routed key (if any) out of this frame's input. `enter`: plain Enter is routed (false
/// for multi-line fields); `empty`: the focused row is empty, so Backspace removes it (a held key
/// repeating never does).
fn take_nav(ctx: &egui::Context, enter: bool, empty: bool) -> Option<Nav> {
    use egui::Event;
    ctx.input_mut(|i| {
        let table = [
            (Key::Enter, Modifiers::NONE, Nav::Enter, enter),
            (Key::Tab, Modifiers::NONE, Nav::Tab, true),
            (Key::Tab, Modifiers::SHIFT, Nav::BackTab, true),
            (Key::Backspace, Modifiers::NONE, Nav::Backspace, empty),
            (Key::Enter, Modifiers::ALT, Nav::MarkCorrect, true),
            (Key::ArrowUp, Modifiers::ALT, Nav::MoveUp, true),
            (Key::ArrowDown, Modifiers::ALT, Nav::MoveDown, true),
        ];
        for (key, mods, nav, on) in table {
            if !on {
                continue;
            }
            let pos = i.events.iter().position(|e| {
                matches!(e, Event::Key { key: k, pressed: true, repeat, modifiers, .. }
                    if *k == key && modifiers.matches_exact(mods) && !(nav == Nav::Backspace && *repeat))
            });
            if let Some(p) = pos {
                i.events.remove(p);
                return Some(nav);
            }
        }
        None
    })
}

/// What the editor body reports each frame.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum QAction {
    None,
    /// Done pressed (draft validated; caller commits it).
    Done,
    /// Cancel pressed (caller drops the draft).
    Cancel,
    /// Delete pressed (caller removes the stored question, if any).
    Delete,
}

/// Context memory for one editing session.
#[derive(Clone, Default)]
struct Temp {
    /// Raw text of each "accepted answers" row, alternatives joined with " OR ".
    /// The frozen `Kind::Blank` shape can't hold raw text, so this mirrors it and
    /// is synced into `Kind::Blank` each frame the Blank editor draws/edits.
    blank_texts: Option<Vec<String>>,
    /// Draft text of the "add topic" field.
    topics_text: String,
    /// The open equation request (set by a sigma button, the word-initial '/' key or a click on
    /// a rendered equation chip); the equation panel reads and clears it.
    eq_req: Option<EqReq>,
    /// The field that had the keyboard focus while this frame drew (read as `focus_last` next frame).
    focus_now: Option<EqField>,
    /// The field that had focus last frame: keys are routed from it before the field sees them.
    focus_last: Option<EqField>,
    /// A field to scroll into view the next time it is drawn.
    reveal: Option<EqField>,
    /// The "+ Add ..." button (named by its editor's first field) that takes focus when drawn.
    focus_add: Option<EqField>,
    /// The control that had focus last frame (a change scrolls it into view).
    ring_id: Option<UiId>,
    /// The confirm bar has already taken focus for its current question.
    confirm_focused: u8,
}

/// Which text field an equation is being spliced into.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum EqField {
    Stem,
    Mc(usize),
    MatchL(usize),
    MatchR(usize),
    Distractor(usize),
    Blank(usize),
    Short,
    PartStem(usize),
    PartMc(usize, usize),
    PartShort(usize),
    Explanation,
}

/// An equation edit in progress: where it goes and what it starts as.
#[derive(Clone, Debug)]
struct EqReq {
    field: EqField,
    /// Char index to insert at (new equation).
    caret: usize,
    /// Char range of the `$..$` being replaced (editing an existing one).
    span: Option<std::ops::Range<usize>>,
    /// Linear content to prefill.
    initial: String,
    /// Screen point the panel opens from (the field or equation chip that asked).
    anchor: Pos2,
    /// Frame the request was made in; the opening click must not close the panel.
    frame: u64,
}

/// Context-memory key for the per-draft editor state.
fn temp_key(bank_id: &str, qid: Option<&Id>) -> UiId {
    UiId::new("testparrot_qedit_temp")
        .with(bank_id)
        .with(qid.map_or("", |s| s.as_str()))
}

/// Widget id of the WYSIWYG editor drawing field `f` (also used to hand it the caret back after
/// the equation dialog).
fn field_editor_id(key: UiId, f: &EqField) -> UiId {
    key.with(("wys_field", f))
}

/// Everything a field needs to ask for the equation modal.
#[derive(Clone)]
struct EqCtx {
    ctx: egui::Context,
    key: UiId,
}

impl EqCtx {
    fn temp<R>(&self, f: impl FnOnce(&mut Temp) -> R) -> R {
        self.ctx.data_mut(|d| f(d.get_temp_mut_or_default::<Temp>(self.key)))
    }

    /// The field that had focus last frame.
    fn last_focus(&self) -> Option<EqField> {
        self.temp(|t| t.focus_last.clone())
    }

    /// Focus `f` with the caret after its `nchars` chars, and scroll it into view.
    fn focus(&self, f: &EqField, nchars: usize) {
        crate::wysiwyg::focus_end(&self.ctx, field_editor_id(self.key, f), nchars);
        self.temp(|t| {
            t.reveal = Some(f.clone());
            t.focus_last = None;
        });
    }

    /// Tab onward to whatever control comes next.
    fn leave(&self) {
        self.ctx.memory_mut(|m| m.move_focus(egui::FocusDirection::Next));
        self.ctx.request_repaint();
    }

    /// Focus the "+ Add ..." button of the editor named by `anchor`.
    fn want_add(&self, anchor: EqField) {
        self.temp(|t| {
            t.focus_add = Some(anchor);
            t.focus_last = None;
        });
        self.ctx.request_repaint();
    }

    /// True once (and clears) when the Add button named by `anchor` should take focus now.
    fn take_add(&self, anchor: &EqField) -> bool {
        self.temp(|t| if t.focus_add.as_ref() == Some(anchor) { t.focus_add = None; true } else { false })
    }

    fn request(&self, field: EqField, caret: usize, span: Option<std::ops::Range<usize>>, initial: String, anchor: Pos2) {
        let frame = self.ctx.cumulative_frame_nr();
        self.ctx.data_mut(|d| {
            d.get_temp_mut_or_default::<Temp>(self.key).eq_req = Some(EqReq { field, caret, span, initial, anchor, frame });
        });
    }
}

// ---------- the draft ----------


#[derive(Clone, Debug)]
pub struct Draft {
    pub bank_id: Id,
    pub qid: Option<Id>,
    pub stem: String,
    pub kind: Kind,
    pub points: Option<f64>,
    pub difficulty: Option<Difficulty>,
    pub topics: Vec<String>,
    pub image: Option<ImageRef>,
    pub explanation: Option<String>,
    /// Figure-engine origin of `image` (None for attached/imported pictures).
    pub figure: Option<crate::model::FigureSpec>,
    /// The question was imported without its answer key (and has not been keyed yet): its
    /// True/False values are placeholders, shown unset until the teacher picks one.
    pub keyless: bool,
    /// The True/False answers the teacher picked in this edit (`None` = the question, `Some(i)` =
    /// part `i`): picking counts as entering the key even when it equals the placeholder.
    pub tf_keyed: Vec<Option<usize>>,
    /// Fingerprint of the saved content, taken once the first frame has settled; the editor is
    /// "dirty" when the content no longer matches it. Not part of any saved data.
    baseline: Option<String>,
    /// A pending confirm step shown in the modal footer.
    confirm: Option<Confirm>,
    /// The modal has skipped its opening frame.
    armed: bool,
}

/// A question the footer asks before it acts.
#[derive(Clone, Debug, PartialEq)]
enum Confirm {
    /// Esc / Cancel / click outside with unsaved changes.
    Discard,
    /// Delete pressed.
    Delete,
    /// Type chip pressed while the current type holds answers that the switch would drop.
    Switch(QKind),
}

/// What Esc, Cancel or a click outside does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseStep {
    /// Nothing changed: close now.
    CloseNow,
    /// Unsaved changes: ask discard or keep editing.
    AskDiscard,
}

/// Esc / Cancel decision from the dirty flag.
pub fn close_step(dirty: bool) -> CloseStep {
    if dirty { CloseStep::AskDiscard } else { CloseStep::CloseNow }
}

/// The empty content of a freshly chosen type.
fn default_kind(k: QKind) -> Kind {
    match k {
        QKind::Mc => Kind::Mc { answers: empty_answers(4) },
        QKind::Tf => Kind::Tf { correct: true },
        QKind::Matching => Kind::Matching {
            pairs: (0..2).map(|_| Pair { left: String::new(), right: String::new() }).collect(),
            distractors: vec![String::new()],
        },
        QKind::Blank => Kind::Blank { accept: vec![vec![String::new()]] },
        QKind::Short => Kind::Short { sample: String::new() },
        QKind::Multipart => Kind::Multipart {
            parts: vec![
                Part { stem: String::new(), kind: PartKind::Tf { correct: true } },
                Part { stem: String::new(), kind: PartKind::Mc { answers: empty_answers(3) } },
            ],
        },
    }
}

impl Draft {
    /// A new unsaved question: empty stem, multiple choice with four empty choices.
    pub fn new(bank_id: Id) -> Draft {
        Draft {
            bank_id,
            qid: None,
            stem: String::new(),
            kind: Kind::Mc {
                answers: empty_answers(4),
            },
            points: None,
            difficulty: None,
            topics: Vec::new(),
            image: None,
            explanation: None,
            figure: None,
            keyless: false,
            tf_keyed: Vec::new(),
            baseline: None,
            confirm: None,
            armed: false,
        }
    }

    /// A deep copy of a stored question for opening in the editor.
    pub fn from_question(bank_id: Id, q: &Question) -> Draft {
        Draft {
            bank_id,
            qid: Some(q.id.clone()),
            stem: q.stem.clone(),
            kind: q.kind.clone(),
            points: q.points,
            difficulty: q.difficulty,
            topics: q.topics.clone(),
            image: q.image.clone(),
            explanation: q.explanation.clone(),
            figure: q.figure.clone(),
            keyless: crate::model::is_keyless(q),
            tf_keyed: Vec::new(),
            baseline: None,
            confirm: None,
            armed: false,
        }
    }

    pub fn is_edit(&self) -> bool {
        self.qid.is_some()
    }

    /// The kind as one of the six combobox values.
    pub fn qkind(&self) -> QKind {
        self.kind.qkind()
    }

    /// Switches question type; the old kind's data is replaced with sane defaults.
    pub fn set_kind(&mut self, k: QKind) {
        if self.kind.qkind() == k {
            return;
        }
        self.kind = default_kind(k);
    }

    /// Everything the editor can change, as one comparable string.
    fn fingerprint(&self) -> String {
        format!(
            "{:?}",
            (
                &self.stem, &self.kind, self.points, self.difficulty, &self.topics, &self.image,
                &self.explanation, &self.figure, self.keyless, &self.tf_keyed,
            )
        )
    }

    /// True when the content differs from what the editor opened with.
    pub fn is_dirty(&self) -> bool {
        self.baseline.as_deref().is_some_and(|b| b != self.fingerprint())
    }

    /// True when switching type would drop typed answers (the stem always carries over).
    pub fn kind_has_content(&self) -> bool {
        if matches!(self.kind, Kind::Tf { .. }) {
            return false;
        }
        format!("{:?}", self.kind) != format!("{:?}", default_kind(self.qkind()))
    }

    /// How the points edit field shows the current points ("3", "3.5", empty for none).
    pub fn points_text(&self) -> String {
        match self.points {
            Some(p) if p.fract() == 0.0 => format!("{}", p as i64),
            Some(p) => p.to_string(),
            None => String::new(),
        }
    }

    pub fn set_points_text(&mut self, s: &str) {
        let t = s.trim();
        self.points = if t.is_empty() { None } else { t.parse().ok() };
    }

    /// Everything the Save button refuses to write without.
    pub fn validate(&self) -> Vec<String> {
        let mut errs = Vec::new();
        if self.stem.trim().is_empty() {
            errs.push("Add a question stem.".to_string());
        }
        match &self.kind {
            Kind::Mc { answers } => {
                let filled = answers.iter().filter(|a| !a.text.trim().is_empty()).count();
                if filled < 2 {
                    errs.push("Add at least two answer choices.".to_string());
                }
                if !answers
                    .iter()
                    .any(|a| a.correct && !a.text.trim().is_empty())
                {
                    errs.push("Mark at least one correct answer.".to_string());
                }
            }
            Kind::Tf { .. } => {}
            Kind::Matching { pairs, .. } => {
                let filled = pairs
                    .iter()
                    .filter(|p| !p.left.trim().is_empty() && !p.right.trim().is_empty())
                    .count();
                if filled < 2 {
                    errs.push("Add at least two matching pairs (both sides filled).".to_string());
                }
            }
            Kind::Blank { accept } => {
                if blank_count(&self.stem) == 0 {
                    errs.push(format!("Add a blank: {BLANK_HOW}."));
                }
                for (i, row) in accept.iter().enumerate().take(blank_count(&self.stem)) {
                    if row.iter().all(|a| a.trim().is_empty()) {
                        errs.push(format!("Blank {} needs at least one accepted answer.", i + 1));
                    }
                }
            }
            Kind::Short { .. } => {}
            Kind::Multipart { parts } => {
                if parts.is_empty() {
                    errs.push("Add at least one part.".to_string());
                }
                for (i, p) in parts.iter().enumerate() {
                    if p.stem.trim().is_empty() {
                        errs.push(format!("Part {} needs its own stem.", i + 1));
                    }
                    if let PartKind::Mc { answers } = &p.kind {
                        let filled = answers
                            .iter()
                            .filter(|a| !a.text.trim().is_empty())
                            .count();
                        if filled >= 2
                            && !answers
                                .iter()
                                .any(|a| a.correct && !a.text.trim().is_empty())
                        {
                            errs.push(format!(
                                "Part {} needs at least one correct answer.",
                                i + 1
                            ));
                        }
                    }
                }
            }
        }
        errs
    }

    /// Builds the stored `Question`: strings trimmed, empties dropped, order kept.
    pub fn build_question(&self, existing: Option<&Question>) -> Question {
        let clean_mc = |answers: &[Answer]| -> Vec<Answer> {
            answers
                .iter()
                .filter(|a| !a.text.trim().is_empty())
                .map(|a| Answer {
                    text: a.text.trim().to_string(),
                    correct: a.correct,
                    locked: a.locked,
                })
                .collect()
        };
        let kind = match &self.kind {
            Kind::Mc { answers } => Kind::Mc {
                answers: clean_mc(answers),
            },
            Kind::Tf { correct } => Kind::Tf { correct: *correct },
            Kind::Matching { pairs, distractors } => Kind::Matching {
                pairs: pairs
                    .iter()
                    .filter(|p| !p.left.trim().is_empty() && !p.right.trim().is_empty())
                    .map(|p| Pair {
                        left: p.left.trim().to_string(),
                        right: p.right.trim().to_string(),
                    })
                    .collect(),
                distractors: distractors
                    .iter()
                    .map(|d| d.trim().to_string())
                    .filter(|d| !d.is_empty())
                    .collect(),
            },
            Kind::Blank { accept } => Kind::Blank {
                accept: accept
                    .iter()
                    .take(blank_count(&self.stem))
                    .map(|row| {
                        row.iter()
                            .map(|a| a.trim().to_string())
                            .filter(|a| !a.is_empty())
                            .collect()
                    })
                    .collect(),
            },
            Kind::Short { sample } => Kind::Short {
                sample: sample.trim().to_string(),
            },
            Kind::Multipart { parts } => Kind::Multipart {
                parts: parts
                    .iter()
                    .map(|p| Part {
                        stem: p.stem.trim().to_string(),
                        kind: match &p.kind {
                            PartKind::Mc { answers } => PartKind::Mc {
                                answers: clean_mc(answers),
                            },
                            PartKind::Tf { correct } => PartKind::Tf { correct: *correct },
                            PartKind::Short { sample } => PartKind::Short {
                                sample: sample.trim().to_string(),
                            },
                        },
                    })
                    .collect(),
            },
        };

        let mut topics: Vec<String> = Vec::new();
        for t in &self.topics {
            let t = t.trim().to_string();
            if !t.is_empty() && !topics.contains(&t) {
                topics.push(t);
            }
        }

        // An existing question keeps its id and created time; a new one is born now.
        let id = existing
            .map(|q| q.id.clone())
            .or_else(|| self.qid.clone())
            .unwrap_or_else(crate::model::new_id);
        let created = existing
            .map(|q| q.created)
            .unwrap_or_else(|| screens::now_secs());
        let t = screens::now_secs();
        let explanation = match self.explanation.as_deref() {
            // A question imported without its key keeps the "no answer key" marker until the
            // teacher has entered a key: editing the marker away does not make one up.
            _ if existing.is_some_and(|q| crate::model::is_keyless(q) && !crate::model::rekeyed_with(q, &kind, &|at| self.tf_keyed.contains(&at))) => {
                Some(crate::model::KEYLESS_SENTINEL.to_string())
            }
            Some(s) if s.trim() == crate::model::KEYLESS_SENTINEL => None,
            Some(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
            _ => None,
        };
        Question {
            id,
            stem: self.stem.trim().to_string(),
            kind,
            points: self.points,
            difficulty: self.difficulty,
            topics,
            image: self.image.clone(),
            explanation,
            figure: if self.image.is_some() { self.figure.clone() } else { None },
            created,
            modified: t,
        }
    }
}

// ---------- switching type ----------

/// What switching a draft to another type does: the new content, and in words what carries over
/// and what is dropped (so the warning can name it).
#[derive(Clone, Debug)]
pub struct SwitchPlan {
    pub stem: String,
    pub kind: Kind,
    /// Things that come across ("the correct answer as the model answer").
    pub kept: Vec<String>,
    /// Things that are lost ("3 answer choices", "the answer key").
    pub lost: Vec<String>,
    /// The target is True/False and the old type's key does not map onto it: the teacher must
    /// pick True or False (nothing is keyed silently).
    pub ask_tf: bool,
}

impl SwitchPlan {
    /// True when the switch needs a warning before it runs.
    pub fn needs_confirm(&self) -> bool {
        !self.lost.is_empty() || self.ask_tf
    }

    /// One line for the footer: what is dropped, what is kept.
    pub fn summary(&self) -> String {
        let join = |v: &[String]| match v.len() {
            0 => String::new(),
            1 => v[0].clone(),
            n => format!("{} and {}", v[..n - 1].join(", "), v[n - 1]),
        };
        let mut s = String::new();
        if !self.lost.is_empty() {
            s.push_str(&format!("Drops {}. ", join(&self.lost)));
        }
        let mut kept = vec!["the question text".to_string()];
        kept.extend(self.kept.iter().cloned());
        s.push_str(&format!("Keeps {}.", join(&kept)));
        s
    }
}

fn filled(s: &str) -> bool {
    !s.trim().is_empty()
}

/// "True" / "False" typed as an answer.
fn tf_word(s: &str) -> Option<bool> {
    match s.trim().trim_end_matches('.').to_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// What the current type holds that the teacher typed or keyed (None: nothing but defaults).
fn content_items(kind: &Kind) -> Vec<String> {
    match kind {
        Kind::Mc { answers } => {
            let n = answers.iter().filter(|a| filled(&a.text)).count();
            let mut v = Vec::new();
            if n > 0 {
                v.push(plural(n, "answer choice", "answer choices"));
            }
            if answers.iter().any(|a| a.correct && filled(&a.text)) {
                v.push("the answer key".to_string());
            }
            v
        }
        Kind::Tf { .. } => Vec::new(),
        Kind::Matching { pairs, distractors } => {
            let n = pairs.iter().filter(|p| filled(&p.left) || filled(&p.right)).count();
            let d = distractors.iter().filter(|x| filled(x)).count();
            let mut v = Vec::new();
            if n > 0 {
                v.push(plural(n, "matching pair", "matching pairs"));
            }
            if d > 0 {
                v.push(plural(d, "distractor", "distractors"));
            }
            v
        }
        Kind::Blank { accept } => {
            if accept.iter().any(|r| r.iter().any(|a| filled(a))) { vec!["the accepted answers".to_string()] } else { Vec::new() }
        }
        Kind::Short { sample } => if filled(sample) { vec!["the model answer".to_string()] } else { Vec::new() },
        Kind::Multipart { parts } => {
            if format!("{kind:?}") == format!("{:?}", default_kind(QKind::Multipart)) { Vec::new() } else { vec![plural(parts.len(), "part", "parts")] }
        }
    }
}

/// The stem with each "___" put back as the accepted answer it stood for, so a question leaving
/// Fill in the blank does not keep dead markers ("(blank)" where no answer was entered).
fn unblank(stem: &str, accept: &[Vec<String>]) -> String {
    let mut out = String::new();
    for (k, piece) in stem.split("___").enumerate() {
        if k > 0 {
            let word = accept.get(k - 1).and_then(|r| r.iter().find(|a| filled(a))).map(|a| a.trim().to_string());
            out.push_str(&word.unwrap_or_else(|| "(blank)".to_string()));
        }
        out.push_str(piece);
    }
    out
}

/// Plans a switch from `from` to `to`. The stem always stays. Multiple choice, True/False and
/// Short answer carry what makes sense between them (the correct choice's text is the model
/// answer, a model answer or "True"/"False" text becomes the matching content); everything else
/// is listed as lost. `tf_keyed`: the True/False value was set on purpose (a saved question, or
/// picked in this edit), so it is worth carrying; an untouched default is not.
pub fn plan_switch(stem: &str, from: &Kind, to: QKind, tf_keyed: bool) -> SwitchPlan {
    let mut plan = SwitchPlan { stem: stem.to_string(), kind: default_kind(to), kept: Vec::new(), lost: content_items(from), ask_tf: false };
    if from.qkind() == to {
        plan.kind = from.clone();
        plan.lost.clear();
        return plan;
    }
    if let Kind::Blank { accept } = from
        && blank_count(stem) > 0
    {
        plan.stem = unblank(stem, accept);
        plan.kept.push("the blanks as plain text".to_string());
        plan.lost = if accept.iter().any(|r| r.iter().filter(|a| filled(a)).count() > 1) {
            vec!["the extra accepted alternatives".to_string()]
        } else {
            Vec::new() // the answers are written back into the text
        };
    }
    let had_answers = !content_items(from).is_empty() && !matches!(from, Kind::Blank { .. });
    match (from, to) {
        (Kind::Mc { answers }, QKind::Tf) => {
            let right: Vec<&Answer> = answers.iter().filter(|a| a.correct && filled(&a.text)).collect();
            match right.as_slice() {
                [one] if tf_word(&one.text).is_some() => {
                    let b = tf_word(&one.text).unwrap_or(true);
                    plan.kind = Kind::Tf { correct: b };
                    plan.kept.push(format!("the key ({})", if b { "True" } else { "False" }));
                    plan.lost.retain(|l| l != "the answer key");
                }
                _ => plan.ask_tf = had_answers,
            }
        }
        (Kind::Mc { answers }, QKind::Short) => {
            let right: Vec<String> = answers.iter().filter(|a| a.correct && filled(&a.text)).map(|a| a.text.trim().to_string()).collect();
            if !right.is_empty() {
                let others = answers.iter().filter(|a| filled(&a.text) && !a.correct).count();
                plan.kind = Kind::Short { sample: right.join("\n") };
                plan.kept.push("the correct answer as the model answer".to_string());
                plan.lost = if others > 0 { vec![plural(others, "other answer choice", "other answer choices")] } else { Vec::new() };
            }
        }
        (Kind::Short { sample }, QKind::Tf) => match tf_word(sample) {
            Some(b) => {
                plan.kind = Kind::Tf { correct: b };
                plan.kept.push(format!("the key ({})", if b { "True" } else { "False" }));
                plan.lost.clear();
            }
            None => plan.ask_tf = filled(sample),
        },
        (Kind::Short { sample }, QKind::Mc) if filled(sample) => {
            let mut answers = empty_answers(4);
            answers[0] = Answer { text: sample.trim().to_string(), correct: true, locked: false };
            plan.kind = Kind::Mc { answers };
            plan.kept.push("the model answer as the first choice, marked correct".to_string());
            plan.lost.clear();
        }
        (Kind::Tf { correct }, QKind::Mc) if tf_keyed => {
            let mut answers = empty_answers(4);
            answers[0] = Answer { text: "True".into(), correct: *correct, locked: false };
            answers[1] = Answer { text: "False".into(), correct: !*correct, locked: false };
            plan.kind = Kind::Mc { answers };
            plan.kept.push("True and False as the first two choices".to_string());
        }
        (Kind::Tf { correct }, QKind::Short) if tf_keyed => {
            plan.kind = Kind::Short { sample: if *correct { "True" } else { "False" }.to_string() };
            plan.kept.push("the key as the model answer".to_string());
        }
        _ => {
            // Nothing maps across: a teacher-entered key into True/False is never guessed.
            if to == QKind::Tf {
                plan.ask_tf = had_answers;
            }
        }
    }
    plan
}

impl Draft {
    /// A new unsaved question of the chosen type (picked from the New menu).
    pub fn new_of(bank_id: Id, k: QKind) -> Draft {
        let mut d = Draft::new(bank_id);
        d.kind = default_kind(k);
        d
    }

    /// True when the True/False value on screen was set on purpose.
    fn tf_is_keyed(&self) -> bool {
        // a keyless import holds a placeholder: only a pick made in this edit is a key
        self.tf_tapped() || (self.is_edit() && !self.keyless)
    }

    fn tf_tapped(&self) -> bool {
        self.tf_keyed.contains(&None)
    }

    /// The plan for switching this draft to `k`.
    pub fn switch_plan(&self, k: QKind) -> SwitchPlan {
        plan_switch(&self.stem, &self.kind, k, self.tf_is_keyed())
    }

    /// Switches type keeping what is compatible. `tf`: the True/False value the teacher chose
    /// when the plan asked for one.
    pub fn switch_to(&mut self, k: QKind, tf: Option<bool>) {
        let p = self.switch_plan(k);
        self.stem = p.stem;
        self.kind = p.kind;
        if let (Kind::Tf { correct }, Some(b)) = (&mut self.kind, tf) {
            *correct = b;
            if !self.tf_keyed.contains(&None) {
                self.tf_keyed.push(None);
            }
        }
    }
}

// ---------- small widget helpers ----------

#[derive(Clone, Copy, PartialEq)]
enum Ico {
    Up,
    Down,
    Lock,
    Close,
}

/// A 24 px square painter-drawn icon button (reorder / lock / remove).
fn icon_btn(ui: &mut Ui, ico: Ico, on: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
    let p = ui.painter();
    if on {
        p.rect_filled(rect, CornerRadius::same(7), theme::accent_faded());
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(7), theme::hover_bg());
    }
    let col = if on {
        theme::accent_col()
    } else if resp.hovered() {
        theme::ink()
    } else {
        theme::muted_col()
    };
    use egui_phosphor::regular as ph;
    let glyph = match ico {
        Ico::Up => ph::CARET_UP,
        Ico::Down => ph::CARET_DOWN,
        Ico::Lock => ph::LOCK_SIMPLE,
        Ico::Close => ph::X,
    };
    p.text(rect.center(), egui::Align2::CENTER_CENTER, glyph, theme::icon_font(15.0), col);
    resp
}

/// A row control that shows only while the row is hovered or has focus. Its 24 px stay reserved
/// when hidden, so rows never reflow; a lock that is on stays drawn (dim) as the row's state.
fn row_btn(ui: &mut Ui, ico: Ico, on: bool, show: bool) -> Option<egui::Response> {
    if show {
        return Some(icon_btn(ui, ico, on));
    }
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
    if on {
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, egui_phosphor::regular::LOCK_SIMPLE, theme::icon_font(15.0), theme::accent_col().gamma_multiply(0.6));
    }
    None
}

/// Rounded field surface (radius 12, field fill, hairline border).
fn field_frame(margin: Margin) -> Frame {
    Frame::new()
        .fill(theme::field())
        .stroke(Stroke::new(1.0, theme::border()))
        .corner_radius(CornerRadius::same(kit::R_FIELD))
        .inner_margin(margin)
}

/// Single-line field exactly `w` wide.
fn field_line(ui: &mut Ui, text: &mut String, w: f32, hint: &str) -> egui::Response {
    let inner = (w - 24.0 - 2.0).max(30.0);
    field_frame(Margin::symmetric(12, 6))
        .show(ui, |ui| {
            ui.add(
                TextEdit::singleline(text)
                    .frame(Frame::new())
                    .hint_text(hint)
                    .desired_width(inner),
            )
        })
        .inner
}

/// Painter-drawn sigma button (opens the equation modal).
fn sigma_button(ui: &mut Ui, eq: &EqCtx, field: &EqField, center: Pos2, size: f32) -> egui::Response {
    let btn = Rect::from_center_size(center, Vec2::splat(size));
    // not a Tab stop: Tab walks the rows
    let br = ui.interact(btn, eq.key.with(("eq_sigma", field)), Sense::CLICK);
    ui.painter().rect(
        btn,
        CornerRadius::same(8),
        if br.hovered() { theme::accent_faded() } else { theme::card() },
        Stroke::new(1.0, theme::border()),
        StrokeKind::Inside,
    );
    let k = size / 26.0;
    let s = Stroke::new(1.5, theme::accent_col());
    let d = |x: f32, y: f32| center + Vec2::new(x, y) * k;
    let pts = [d(4.0, -5.0), d(-4.0, -5.0), d(1.0, 0.0), d(-4.0, 5.0), d(4.0, 5.0)];
    for w2 in pts.windows(2) {
        ui.painter().line_segment([w2[0], w2[1]], s);
    }
    br.on_hover_text("Insert equation (or type / at the start of a word)")
}

/// True when a field's text renders differently from its raw form (stem markup, `$math$`
/// or a `[picture]` placeholder an importer left).
fn renders_rich(text: &str) -> bool {
    crate::richtext::has_markup(text) || text.contains('*') || has_picture(text)
}

/// Does `text` hold a real `[picture]` / `[picture: alt]` marker?
fn has_picture(text: &str) -> bool {
    text.contains("[picture") && crate::bankops::split_pictures(text).iter().any(|p| matches!(p, crate::bankops::StemPart::Picture(_)))
}

/// `text` cut into the runs a preview draws: (char offset of the run in `text`, run) for
/// text, `None` offset for a picture placeholder (its alt in the run).
fn preview_runs(text: &str) -> Vec<(Option<usize>, String)> {
    use crate::bankops::StemPart;
    let mut byte = 0usize;
    crate::bankops::split_pictures(text)
        .into_iter()
        .map(|p| match p {
            StemPart::Text(t) => {
                let at = text[byte..].find(&t).map_or(byte, |i| byte + i);
                byte = at + t.len();
                (Some(text[..at].chars().count()), t)
            }
            StemPart::Picture(alt) => (None, alt.unwrap_or_default()),
        })
        .collect()
}

/// The editor's stand-in for a `[picture]` marker: a dashed "Picture needed" box with its
/// description (the picture itself is added with Attach image after import).
fn picture_box(ui: &mut Ui, alt: &str, size: f32) {
    let w = ui.available_width().clamp(120.0, 420.0);
    let h = if alt.is_empty() { size * 2.6 } else { size * 3.6 };
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    let p = ui.painter();
    let red = theme::red();
    p.rect_filled(rect, CornerRadius::same(6), red.gamma_multiply(0.08));
    p.extend(egui::Shape::dashed_line(
        &[rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom(), rect.left_top()],
        Stroke::new(1.0, red),
        4.0,
        3.0,
    ));
    let cy = if alt.is_empty() { rect.center().y } else { rect.center().y - size * 0.6 };
    p.text(Pos2::new(rect.center().x, cy), egui::Align2::CENTER_CENTER, "Picture needed", egui::FontId::new(size, theme::bold_family()), red);
    if !alt.is_empty() {
        let a: String = if alt.chars().count() > 60 { alt.chars().take(60).chain(std::iter::once('…')).collect() } else { alt.to_string() };
        p.text(Pos2::new(rect.center().x, cy + size * 1.25), egui::Align2::CENTER_CENTER, a, egui::FontId::new(size * 0.85, theme::serif_family()), theme::muted_col());
    }
    ui.add_space(4.0);
}

/// Rendered preview of a field that holds markup or equations; clicking an equation opens it
/// for editing.
fn eq_preview(ui: &mut Ui, eq: &EqCtx, field: EqField, text: &str, indent: f32, size: f32) {
    if !renders_rich(text) {
        return;
    }
    let mut hit = None;
    ui.horizontal(|ui| {
        ui.add_space(indent);
        ui.vertical(|ui| {
            let o = crate::equation::Rich::new(size, theme::text_col()).editable(true);
            if !has_picture(text) {
                let (r, h) = crate::equation::rich_ui(ui, text, &o);
                hit = h.map(|h| (h, r.rect));
                return;
            }
            for (at, run) in preview_runs(text) {
                match at {
                    Some(at) => {
                        let (r, h) = crate::equation::rich_ui(ui, &run, &o);
                        if let Some(mut h) = h {
                            h.span = h.span.start + at..h.span.end + at;
                            hit = Some((h, r.rect));
                        }
                    }
                    None => picture_box(ui, &run, size),
                }
            }
        });
    });
    if let Some((h, r)) = hit {
        eq.request(field, 0, Some(h.span), h.content, r.left_bottom());
    }
    ui.add_space(2.0);
}

/// The stem's rendered preview: how the question prints (tables, lists, bold, scripts, math),
/// in a soft card under the field. Shown once the stem has any markup or math.
fn stem_preview(ui: &mut Ui, eq: &EqCtx, text: &str, w: f32) {
    if !renders_rich(text) {
        return;
    }
    ui.add_space(6.0);
    Frame::new()
        .fill(theme::card())
        .stroke(Stroke::new(1.0, theme::border()))
        .corner_radius(CornerRadius::same(kit::R_FIELD))
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width((w - 26.0).max(40.0));
            ui.label(theme::muted("Preview").size(11.5));
            ui.add_space(2.0);
            eq_preview(ui, eq, EqField::Stem, text, 0.0, 14.0);
        });
}

/// A field drawn by the WYSIWYG editor (inline markup and typeset math in place) inside the
/// rounded field frame, with the sigma button at its right edge. `structured`: lists and tables
/// as blocks (part stems, model answers); `newlines`: Enter adds a line.
#[allow(clippy::too_many_arguments)]
fn wys_field(ui: &mut Ui, eq: &EqCtx, field: EqField, text: &mut String, w: f32, hint: &str, structured: bool, newlines: bool, size: f32, sigma_top: bool, lock_tab: bool) -> crate::wysiwyg::Out {
    let inner = (w - 24.0 - 2.0 - 26.0).max(30.0);
    let id = field_editor_id(eq.key, &field);
    let fr = field_frame(Margin { left: 12, right: 30, top: 6, bottom: 6 }).show(ui, |ui| {
        ui.set_width(inner);
        crate::wysiwyg::editor(ui, id, text, crate::wysiwyg::Opts { size, width: inner, structured, newlines, hint, blanks: None, toolbar: false, lock_tab })
    });
    let out = fr.inner;
    let r = fr.response.rect;
    mark_field(ui, eq, &field, &out, r);
    let c = if sigma_top { Pos2::new(r.right() - 17.0, r.top() + 15.0) } else { Pos2::new(r.right() - 17.0, r.center().y) };
    if sigma_button(ui, eq, &field, c, 22.0).clicked() {
        eq.request(field.clone(), out.caret.unwrap_or(text.chars().count()), None, String::new(), r.left_bottom());
    }
    if let Some(a) = &out.eq {
        eq.request(field.clone(), a.caret, a.span.clone(), a.initial.clone(), r.left_bottom());
    }
    if has_picture(text) {
        eq_preview(ui, eq, field, text, 0.0, 13.0);
    }
    out
}

/// After a field drew: draw its focus ring, remember it for key routing, scroll it into view if
/// it was just added.
fn mark_field(ui: &mut Ui, eq: &EqCtx, field: &EqField, out: &crate::wysiwyg::Out, r: Rect) {
    if out.focused {
        ui.painter().rect_stroke(r, CornerRadius::same(kit::R_FIELD), Stroke::new(1.5, theme::accent_col()), StrokeKind::Inside);
    }
    let reveal = eq.temp(|t| {
        if out.focused {
            t.focus_now = Some(field.clone());
        }
        if t.reveal.as_ref() == Some(field) {
            t.reveal = None;
            true
        } else {
            false
        }
    });
    if reveal {
        ui.scroll_to_rect(r.expand(10.0), None);
    }
}

/// One-line answer field (MC choice, matching item, blank answers); Tab belongs to the rows.
fn eq_line(ui: &mut Ui, eq: &EqCtx, field: EqField, text: &mut String, w: f32, hint: &str) -> crate::wysiwyg::Out {
    wys_field(ui, eq, field, text, w, hint, false, false, 13.5, false, true)
}

/// A multipart part's stem: its sub-lines, bullets and table rows edit as blocks.
fn eq_line_multi(ui: &mut Ui, eq: &EqCtx, field: EqField, text: &mut String, w: f32, hint: &str) {
    wys_field(ui, eq, field, text, w, hint, true, true, 13.5, true, false);
}

/// A multi-line answer (model answers).
fn eq_multi(ui: &mut Ui, eq: &EqCtx, field: EqField, text: &mut String, w: f32, _rows: usize, hint: &str) {
    wys_field(ui, eq, field, text, w, hint, true, true, 13.5, true, false);
}

fn label(ui: &mut Ui, text: &str) {
    kit::small_caps(ui, text);
    ui.add_space(3.0);
}

// ---------- the in-place editor ----------

impl App {
    /// Writes a validated draft into the store (new or edit) and toasts.
    pub(crate) fn commit_draft(&mut self, draft: Draft) {
        let existing = draft
            .qid
            .as_ref()
            .and_then(|qid| self.store.question(&draft.bank_id, qid))
            .cloned();
        let q = draft.build_question(existing.as_ref());
        let t = q.modified;
        if let Some(bank) = self.store.bank_mut(&draft.bank_id) {
            match draft.qid.as_ref() {
                Some(qid) => {
                    if let Some(slot) = bank.questions.iter_mut().find(|x| x.id == *qid) {
                        *slot = q;
                    } else {
                        bank.questions.push(q);
                    }
                }
                None => {
                    bank.questions.push(q);
                }
            }
            bank.modified = t;
        }
        self.store.save_banks();
        self.toast("Question saved");
    }
}

/// Widest the modal gets; it shrinks on narrow windows.
const MODAL_W: f32 = 720.0;
/// Space the modal's header, footer and frame take from the window height.
const MODAL_CHROME: f32 = 230.0;

/// Draws the question editor as a centered modal over whatever is underneath (the bank list
/// stays where it is). One column: header, a scrolling body (metadata strip, stem, answers,
/// collapsible Explanation and Image) of fixed height, and a footer of fixed height. The caller
/// applies the returned action. `topic_pool` is the bank's existing topics (offered as one-tap
/// suggestions).
pub fn draw_modal(
    ctx: &egui::Context,
    draft: &mut Draft,
    topic_pool: &[String],
    textures: &mut HashMap<String, egui::TextureHandle>,
) -> QAction {
    let key = temp_key(&draft.bank_id, draft.qid.as_ref());
    let first_frame = draft.baseline.is_none();
    if !draft.armed {
        // Opening happens on a click that is still live this frame, often right over the stem:
        // draw from the next frame, or the editor would take that click as a press in the text
        // and pop the floating toolbar over a blank.
        draft.armed = true;
        // The stem takes the keyboard from the first frame the editor shows.
        EqCtx { ctx: ctx.clone(), key }.focus(&EqField::Stem, draft.stem.chars().count());
        ctx.request_repaint();
        return QAction::None;
    }
    // What had focus while the last frame drew is where this frame's keys are routed from.
    ctx.data_mut(|d| {
        let t = d.get_temp_mut_or_default::<Temp>(key);
        t.focus_last = t.focus_now.take();
    });
    let mut action = QAction::None;
    // While the equation panel is open it owns the keyboard and the outside click.
    let eq_open = ctx.data(|d| d.get_temp::<Temp>(key).and_then(|t| t.eq_req).is_some());
    let mut want_close = false;
    if !eq_open
        && draft.confirm.is_none()
        && ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Enter)))
        && draft.validate().is_empty()
    {
        action = QAction::Done;
    }

    let screen = ctx.content_rect();
    let w = MODAL_W.min((screen.width() - 56.0).max(280.0));
    let body_h = (screen.height() - MODAL_CHROME).clamp(180.0, 640.0);

    let frame = kit::card().inner_margin(Margin::symmetric(22, 16)).shadow(egui::Shadow {
        offset: [0, 6],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(60),
    });
    let modal = egui::Modal::new(UiId::new("qedit_modal")).frame(frame).show(ctx, |ui| {
        ui.set_width(w);
        // Header.
        ui.horizontal(|ui| {
            kit::type_pill(ui, type_tag(draft.qkind()));
            ui.label(theme::serif(if draft.is_edit() { "Edit question" } else { "New question" }).size(18.0));
        });
        ui.add_space(8.0);
        // Scrolling body of fixed height, so the modal never changes size under the cursor.
        egui::ScrollArea::vertical()
            .id_salt(key.with("body_scroll"))
            .min_scrolled_height(body_h)
            .max_height(body_h)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let bw = (w - 16.0).max(240.0);
                ui.set_width(bw);
                meta_strip(ui, ctx, draft, key, bw, topic_pool);
                ui.add_space(14.0);
                body(ui, ctx, draft, key, bw, textures);
            });
        ui.add_space(8.0);
        let y = ui.cursor().top();
        ui.painter().hline(ui.max_rect().x_range(), y, Stroke::new(1.0, theme::rule()));
        ui.add_space(8.0);
        footer(ui, draft, &mut action, &mut want_close);
    });

    focus_ring(ctx, key);
    if first_frame {
        // Taken after the first frame, so the fields' own normalising does not count as an edit.
        draft.baseline = Some(draft.fingerprint());
    }
    // Esc and a click outside behave like Cancel (the equation panel keeps its own Esc).
    if !eq_open && (modal.backdrop_response.clicked() || ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))) {
        want_close = true;
    }
    if want_close && action == QAction::None {
        if draft.confirm.is_some() {
            draft.confirm = None; // Esc on a prompt means keep editing
        } else {
            match close_step(draft.is_dirty()) {
                CloseStep::CloseNow => action = QAction::Cancel,
                CloseStep::AskDiscard => draft.confirm = Some(Confirm::Discard),
            }
        }
    }
    action
}

/// A visible ring on whichever control has keyboard focus (text fields draw their own), and a
/// scroll to bring a control that just took focus into view.
fn focus_ring(ctx: &egui::Context, key: UiId) {
    let Some(id) = ctx.memory(|m| m.focused()) else { return };
    let Some(resp) = ctx.read_response(id) else { return };
    let (text_field, changed) = ctx.data_mut(|d| {
        let t = d.get_temp_mut_or_default::<Temp>(key);
        let ch = t.ring_id != Some(id);
        t.ring_id = Some(id);
        (t.focus_now.is_some(), ch)
    });
    if changed && !text_field {
        resp.scroll_to_me(None);
    }
    if !text_field && resp.rect.is_positive() && resp.rect.width() < 700.0 {
        let p = egui::Painter::new(ctx.clone(), egui::LayerId::new(egui::Order::Tooltip, UiId::new("qedit_focus_ring")), ctx.content_rect());
        p.rect_stroke(resp.rect.expand(2.0), CornerRadius::same(10), Stroke::new(2.0, theme::accent_col()), StrokeKind::Outside);
    }
}

/// The footer: a fixed-height validation line, then one fixed-height button row that turns into
/// a confirm bar of the same height, so nothing moves when it changes.
fn footer(ui: &mut Ui, draft: &mut Draft, action: &mut QAction, want_close: &mut bool) {
    let errs = draft.validate();
    // Validation reads in a reserved line: the first problem, plus a count of the rest. While a
    // type switch waits for a yes, the line says exactly what it drops and keeps.
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::hover());
    if let Some(Confirm::Switch(k)) = &draft.confirm {
        let p = draft.switch_plan(*k);
        let mut msg = p.summary();
        if p.ask_tf {
            msg.push_str(" The answer key is not carried: pick True or False.");
        }
        ui.painter().with_clip_rect(rect).text(rect.left_center(), egui::Align2::LEFT_CENTER, msg, kit::ui_font(12.0), theme::ink());
    } else if let Some(first) = errs.first() {
        let more = if errs.len() > 1 { format!("  (+{} more)", errs.len() - 1) } else { String::new() };
        let p = ui.painter().with_clip_rect(rect);
        p.text(
            rect.left_center(),
            egui::Align2::LEFT_CENTER,
            format!("{first}{more}"),
            kit::ui_font(12.0),
            theme::bad(),
        );
    }
    ui.add_space(4.0);
    let confirm = draft.confirm.clone();
    if confirm.is_none() {
        let k = temp_key(&draft.bank_id, draft.qid.as_ref());
        ui.ctx().data_mut(|d| d.get_temp_mut_or_default::<Temp>(k).confirm_focused = 0);
    }
    ui.horizontal(|ui| {
        ui.set_min_height(36.0);
        match confirm {
            None => {
                if draft.is_edit() && kit::danger_text_button(ui, "Delete").clicked() {
                    draft.confirm = Some(Confirm::Delete);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let save = ui.add_enabled_ui(errs.is_empty(), |ui| kit::primary_button(ui, "Save")).inner;
                    if save.clicked() {
                        *action = QAction::Done;
                    }
                    if kit::ghost_button(ui, "Cancel").clicked() {
                        *want_close = true;
                    }
                    ui.label(theme::muted("Ctrl/Cmd+Enter saves").size(11.5));
                });
            }
            Some(c) => {
                let ask_tf = matches!(&c, Confirm::Switch(k) if draft.switch_plan(*k).ask_tf);
                let (msg, keep, go) = match &c {
                    Confirm::Discard => ("Discard your unsaved changes?  Enter discards, Esc keeps editing.".to_string(), "Keep editing", "Discard"),
                    Confirm::Delete => ("Delete this question?".to_string(), "Keep it", "Delete"),
                    Confirm::Switch(k) if ask_tf => (format!("Switch to {}: which is correct?", type_tag(*k)), "Keep type", "False"),
                    Confirm::Switch(k) => (format!("Switch to {}?", type_tag(*k)), "Keep type", "Switch"),
                };
                ui.label(RichText::new(msg).size(12.5).color(theme::ink()));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    // only the destructive choice is red; picking True/False is a plain primary action
                    let red_btn = |ui: &mut Ui, t: &str, red: bool| {
                        ui.add(
                            Button::new(RichText::new(t).family(theme::bold_family()).color(theme::on_accent_col()))
                                .fill(if red { theme::bad() } else { theme::accent_col() })
                                .stroke(Stroke::NONE)
                                .corner_radius(CornerRadius::same(255))
                                .min_size(Vec2::new(0.0, 34.0)),
                        )
                    };
                    let mut pick: Option<Option<bool>> = None;
                    let go_btn = red_btn(ui, go, !ask_tf);
                    // the safe choice has focus for Delete and the True/False pick; Enter discards
                    // or switches otherwise
                    let safe_first = matches!(c, Confirm::Delete) || ask_tf;
                    let seen = ui.ctx().data_mut(|d| {
                        let t = d.get_temp_mut_or_default::<Temp>(temp_key(&draft.bank_id, draft.qid.as_ref()));
                        t.confirm_focused = t.confirm_focused.saturating_add(1).min(3);
                        t.confirm_focused != 2 // focus lands on the second frame the bar shows
                    });
                    if !seen || ui.ctx().data_mut(|d| d.get_temp_mut_or_default::<Temp>(temp_key(&draft.bank_id, draft.qid.as_ref())).confirm_focused < 3) {
                        ui.ctx().request_repaint();
                    }
                    if !seen && !safe_first {
                        go_btn.request_focus();
                    }
                    if go_btn.clicked() {
                        match c {
                            Confirm::Discard => *action = QAction::Cancel,
                            Confirm::Delete => *action = QAction::Delete,
                            Confirm::Switch(_) => pick = Some(if ask_tf { Some(false) } else { None }),
                        }
                    }
                    if ask_tf && red_btn(ui, "True", false).clicked() {
                        pick = Some(Some(true));
                    }
                    if let (Some(choice), Confirm::Switch(k)) = (pick, &c) {
                        draft.switch_to(*k, choice);
                        draft.confirm = None;
                        let key = temp_key(&draft.bank_id, draft.qid.as_ref());
                        ui.ctx().data_mut(|d| d.remove_temp::<Temp>(key));
                        EqCtx { ctx: ui.ctx().clone(), key }.focus(&EqField::Stem, draft.stem.chars().count());
                    }
                    let keep_btn = kit::ghost_button(ui, keep);
                    if !seen && safe_first {
                        keep_btn.request_focus();
                    }
                    if keep_btn.clicked() {
                        draft.confirm = None;
                    }
                });
            }
        }
    });
}

/// One label/value row of the metadata strip.
fn meta_row(ui: &mut Ui, name: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.allocate_ui_with_layout(Vec2::new(78.0, 28.0), Layout::left_to_right(Align::Center), |ui| {
            kit::small_caps(ui, name);
        });
        ui.vertical(add);
    });
}

/// Type, Difficulty, Topics and Points as label/value rows above the question.
fn meta_strip(ui: &mut Ui, ctx: &egui::Context, draft: &mut Draft, key: UiId, w: f32, topic_pool: &[String]) {
    let vw = (w - 86.0).max(120.0);
    meta_row(ui, "TYPE", |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
            for (k, tag) in KINDS {
                if kit::chip_toggle(ui, tag, draft.qkind() == k).on_hover_text(k.label()).clicked() && draft.qkind() != k {
                    if draft.switch_plan(k).needs_confirm() {
                        draft.confirm = Some(Confirm::Switch(k));
                    } else {
                        draft.switch_to(k, None);
                        ctx.data_mut(|d| d.remove_temp::<Temp>(key));
                        EqCtx { ctx: ctx.clone(), key }.focus(&EqField::Stem, draft.stem.chars().count());
                    }
                }
            }
        });
    });
    meta_row(ui, "DIFFICULTY", |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
            for (d, name) in [(Difficulty::Low, "Easy"), (Difficulty::Medium, "Medium"), (Difficulty::High, "Hard")] {
                let on = draft.difficulty == Some(d);
                if kit::chip_toggle(ui, name, on).clicked() {
                    draft.difficulty = if on { None } else { Some(d) };
                }
            }
        });
    });
    meta_row(ui, "TOPICS", |ui| topics_value(ui, ctx, draft, key, vw, topic_pool));
    meta_row(ui, "POINTS", |ui| {
        let mut pt = draft.points_text();
        if field_line(ui, &mut pt, 96.0, "none").changed() {
            draft.set_points_text(&pt);
        }
    });
}

/// Topic chips and the add field share one wrapping row, so adding the first chip never changes
/// the row's height. Suggestions drop down below the field as an overlay (they never push or
/// overlap-reflow later rows) and exist only while a topic is being typed.
fn topics_value(ui: &mut Ui, ctx: &egui::Context, draft: &mut Draft, key: UiId, w: f32, topic_pool: &[String]) {
    let mut txt = ctx.data_mut(|d| d.get_temp_mut_or_default::<Temp>(key).topics_text.clone());
    let mut field: Option<egui::Response> = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
        ui.set_min_height(34.0);
        for t in draft.topics.clone() {
            if kit::chip_toggle(ui, &format!("{t}  x"), true).on_hover_text("Remove topic").clicked() {
                draft.topics.retain(|x| x != &t);
            }
        }
        field = Some(field_line(ui, &mut txt, w.min(260.0), "+ add topic (press Enter)"));
    });
    let r = field.expect("field drawn");
    // Autocomplete: the bank's topics matching what is typed (prefix first); Enter takes
    // the existing spelling when the typed text names a topic already in the bank.
    let sugg = crate::bankops::suggest_topics(topic_pool, &txt, &draft.topics, 8);
    let commit = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) && !txt.trim().is_empty();
    if commit {
        let t = crate::bankops::canonical_topic(topic_pool, &txt);
        if !draft.topics.contains(&t) {
            draft.topics.push(t);
        }
        txt.clear();
        r.request_focus();
    }
    let typed = txt.trim().to_string();
    let mut picked: Option<String> = None;
    // A press on a chip takes focus off the field, so the popup also stays while the pointer is on it.
    let pop_id = key.with("topic_suggest_rect");
    let on_pop = ctx
        .data(|d| d.get_temp::<Option<Rect>>(pop_id).flatten())
        .is_some_and(|pr| ctx.input(|i| i.pointer.hover_pos().or(i.pointer.interact_pos()).is_some_and(|p| pr.contains(p))));
    let mut pop_rect: Option<Rect> = None;
    if !typed.is_empty() && (r.has_focus() || on_pop) {
        let exists = topic_pool.iter().any(|p| p.trim().eq_ignore_ascii_case(&typed));
        let area = egui::Area::new(key.with("topic_suggest"))
            .order(egui::Order::Foreground)
            .fixed_pos(r.rect.left_bottom() + Vec2::new(0.0, 2.0))
            .show(ctx, |ui| {
                Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(w.min(420.0));
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                        ui.label(theme::muted(topic_hint(exists, !sugg.is_empty())).size(11.0));
                        for t in &sugg {
                            if kit::chip_toggle(ui, t, false).clicked() {
                                picked = Some(t.clone());
                            }
                        }
                    });
                });
            });
        pop_rect = Some(area.response.rect);
    }
    ctx.data_mut(|d| d.insert_temp(pop_id, pop_rect));
    if let Some(t) = picked {
        if !draft.topics.contains(&t) {
            draft.topics.push(t);
        }
        txt.clear();
    }
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Temp>(key).topics_text = txt);
}

/// The one-line hint over the topic suggestions.
fn topic_hint(exists: bool, has_matches: bool) -> &'static str {
    if exists {
        "Enter adds this topic."
    } else if !has_matches {
        "Enter adds a new topic."
    } else {
        "Matching:"
    }
}

/// Stem, per-type answers, then the collapsible Explanation and Image sections.
fn body(ui: &mut Ui, ctx: &egui::Context, draft: &mut Draft, key: UiId, w: f32, textures: &mut HashMap<String, egui::TextureHandle>) {
    left_col(ui, ctx, draft, key, w);
    ui.add_space(14.0);
    let has_expl = draft.explanation.as_deref().is_some_and(|e| !e.trim().is_empty());
    egui::CollapsingHeader::new(RichText::new("Explanation (optional)").family(theme::bold_family()).size(13.0))
        .id_salt(key.with("expl_section"))
        .default_open(has_expl)
        .show(ui, |ui| {
            let mut explanation = draft.explanation.clone().unwrap_or_default();
            // the WYSIWYG field (bold/italic/strike/scripts/math show as they print on the key)
            let eq = EqCtx { ctx: ctx.clone(), key };
            eq_multi(ui, &eq, EqField::Explanation, &mut explanation, w - 20.0, 2, "Shown with the answer key");
            draft.explanation = match explanation.trim() {
                "" => None,
                _ => Some(explanation),
            };
        });
    let has_img = draft.image.is_some() || draft.figure.is_some();
    egui::CollapsingHeader::new(RichText::new("Image").family(theme::bold_family()).size(13.0))
        .id_salt(key.with("image_section"))
        .default_open(has_img)
        .show(ui, |ui| draw_image_row(ui, draft, textures));
    ui.add_space(6.0);
}

/// The first answer field Tab leaves the stem for, with its text length (None: the kind has no
/// text field to go to, so Tab keeps its default). `blank_rows`: the Fill answer rows.
fn first_field(kind: &Kind, blank_rows: Option<&[String]>) -> Option<(EqField, usize)> {
    match kind {
        Kind::Mc { answers } => Some((EqField::Mc(0), chars(&answers.first()?.text))),
        Kind::Tf { .. } => None,
        Kind::Matching { pairs, .. } => Some((EqField::MatchL(0), chars(&pairs.first()?.left))),
        Kind::Blank { .. } => Some((EqField::Blank(0), blank_rows.and_then(|r| r.first()).map_or(0, |t| chars(t)))),
        Kind::Short { sample } => Some((EqField::Short, chars(sample))),
        Kind::Multipart { parts } => Some((EqField::PartStem(0), chars(&parts.first()?.stem))),
    }
}

fn left_col(ui: &mut Ui, ctx: &egui::Context, draft: &mut Draft, key: UiId, w: f32) {
    // Stem with the equation hook.
    let eq = EqCtx { ctx: ctx.clone(), key };
    label(ui, "Question");
    let is_blank = matches!(draft.kind, Kind::Blank { .. });
    let mut mirror = is_blank.then(|| blank_mirror(ctx, key, draft));
    let blank_hint = format!("Write the sentence, then {BLANK_HOW}...");
    let hint = if is_blank { blank_hint.as_str() } else { "Write the question..." };
    let inner = (w - 26.0).max(60.0);
    let sid = field_editor_id(key, &EqField::Stem);
    // Tab leaves the stem for the first answer field, unless the stem is a list or table (Tab
    // indents there). Enter keeps adding lines.
    let first = first_field(&draft.kind, mirror.as_deref());
    let tab_to_answers = first.is_some() && crate::wysiwyg::is_plain(&draft.stem);
    if tab_to_answers && eq.last_focus() == Some(EqField::Stem) {
        let tab = ctx.input_mut(|i| {
            let p = i.events.iter().position(|e| matches!(e, egui::Event::Key { key: Key::Tab, pressed: true, modifiers, .. } if modifiers.matches_exact(Modifiers::NONE)));
            p.map(|p| i.events.remove(p)).is_some()
        });
        if tab && let Some((f, n)) = &first {
            eq.focus(f, *n);
        }
    }
    let fr = field_frame(Margin::symmetric(12, 8)).show(ui, |ui| {
        ui.set_width(inner);
        crate::wysiwyg::editor(
            ui,
            sid,
            &mut draft.stem,
            crate::wysiwyg::Opts { size: 14.5, width: inner, structured: true, newlines: true, hint, blanks: mirror.as_mut(), toolbar: true, lock_tab: tab_to_answers },
        )
    });
    let out = fr.inner;
    mark_field(ui, &eq, &EqField::Stem, &out, fr.response.rect);
    if let Some(m) = mirror {
        ctx.data_mut(|d| d.get_temp_mut_or_default::<Temp>(key).blank_texts = Some(m));
    }
    if let Some(a) = out.eq {
        eq.request(EqField::Stem, a.caret, a.span, a.initial, fr.response.rect.left_bottom());
    }
    if has_picture(&draft.stem) {
        stem_preview(ui, &eq, &draft.stem, w);
    }
    equation_panel(ctx, draft, key);
    ui.add_space(12.0);

    // Per-kind answers.
    let stem_len = chars(&draft.stem);
    match &mut draft.kind {
        Kind::Mc { answers } => {
            label(ui, "Answers  -  click the circle or press Alt+Enter to mark correct; Enter adds a row");
            mc_editor(ui, &eq, answers, usize::MAX, w, &|i| EqField::Mc(i), (EqField::Stem, stem_len));
        }
        Kind::Tf { correct } => {
            label(ui, "Correct answer  -  click the circle");
            // An imported question without its key shows no answer until the teacher picks one.
            let unset = draft.keyless && !draft.tf_keyed.contains(&None);
            for (val, name) in [(true, "True"), (false, "False")] {
                ui.horizontal(|ui| {
                    if kit::check_circle(ui, !unset && *correct == val).clicked() {
                        *correct = val;
                        if !draft.tf_keyed.contains(&None) {
                            draft.tf_keyed.push(None);
                        }
                    }
                    ui.add_space(4.0);
                    ui.label(theme::text(name).size(13.5));
                });
            }
        }
        Kind::Matching { pairs, distractors } => matching_editor(ui, &eq, pairs, distractors, w, stem_len),
        Kind::Blank { .. } => blank_editor(ui, &eq, draft, w),
        Kind::Short { sample } => {
            label(ui, "Model answer");
            eq_multi(ui, &eq, EqField::Short, sample, w, 3, "What a good answer looks like...");
        }
        Kind::Multipart { parts } => {
            label(ui, "Parts");
            multipart_editor(ui, &eq, parts, w, draft.keyless, &mut draft.tf_keyed);
        }
    }
}

fn chars(s: &str) -> usize {
    s.chars().count()
}

/// Multiple choice / multiple answer. Each row: tappable correct circle (one ticked =
/// single choice; several = multiple answer), text, then reorder up/down, lock and remove, which
/// show on hover or while the row has focus. Keys: Enter adds/next, Tab/Shift+Tab walk, Backspace
/// on an empty row removes it, Alt+Enter marks correct, Alt+Up/Down moves the row.
/// `before` is the field Shift+Tab leaves to (and its text length).
fn mc_editor(ui: &mut Ui, eq: &EqCtx, answers: &mut Vec<Answer>, max: usize, w: f32, fld: &dyn Fn(usize) -> EqField, before: (EqField, usize)) {
    let mut to_remove: Option<usize> = None;
    let mut move_up: Option<usize> = None;
    let add_anchor = fld(0);
    // keyboard, before the fields see the keys
    if let Some(i) = eq.last_focus().and_then(|f| (0..answers.len()).find(|&i| fld(i) == f)) {
        let n = answers.len();
        let empty = crate::wysiwyg::visibly_empty(&answers[i].text);
        if let Some(nav) = take_nav(&eq.ctx, true, empty) {
            match nav {
                Nav::MarkCorrect => answers[i].correct = !answers[i].correct,
                Nav::MoveUp if i > 0 => {
                    move_up = Some(i);
                    eq.focus(&fld(i - 1), chars(&answers[i].text));
                }
                Nav::MoveDown if i + 1 < n => {
                    move_up = Some(i + 1);
                    eq.focus(&fld(i + 1), chars(&answers[i].text));
                }
                Nav::MoveUp | Nav::MoveDown => {}
                _ => match nav_step(nav, i, n, n < max, n > 2 && empty) {
                    Step::Goto(j) => eq.focus(&fld(j), chars(&answers[j].text)),
                    Step::Add => {
                        answers.push(empty_answer());
                        eq.focus(&fld(n), 0);
                    }
                    Step::AddButton => eq.want_add(add_anchor.clone()),
                    Step::Before => eq.focus(&before.0, before.1),
                    Step::Leave => eq.leave(),
                    Step::Remove(r) => {
                        let j = after_remove(r);
                        to_remove = Some(r);
                        let at = if r == 0 { 1 } else { j };
                        eq.focus(&fld(if r == 0 { 0 } else { j }), chars(&answers[at].text));
                    }
                    Step::Stay => {}
                },
            }
        }
    }
    let n = answers.len();
    let ctrl_w = 4.0 * 24.0 + 4.0 * 6.0;
    let field_w = (w - 22.0 - 6.0 - ctrl_w).max(120.0);
    for i in 0..n {
        let locked = answers[i].locked;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let start = ui.cursor().min;
            if kit::check_circle(ui, answers[i].correct).clicked() {
                answers[i].correct = !answers[i].correct;
            }
            let out = eq_line(ui, eq, fld(i), &mut answers[i].text, field_w, "Answer");
            if out.focused && chars(&answers[i].text) < 24 {
                // where the user is looking: the key that marks this row correct
                ui.painter().text(
                    Pos2::new(start.x + 22.0 + 6.0 + field_w - 40.0, out.rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    if answers[i].correct { "Alt+Enter = not correct" } else { "Alt+Enter = correct" },
                    kit::ui_font(11.0),
                    theme::muted_col(),
                );
            }
            // controls on hover or focus; the width stays reserved so the row never reflows
            let row = Rect::from_min_max(start, Pos2::new(start.x + w, ui.min_rect().bottom().max(start.y + 24.0)));
            let show = out.focused || ui.rect_contains_pointer(row);
            if let Some(r) = row_btn(ui, Ico::Up, false, show) {
                if r.on_hover_text("Move up (Alt+Up)").clicked() && i > 0 {
                    move_up = Some(i);
                }
            }
            if let Some(r) = row_btn(ui, Ico::Down, false, show) {
                if r.on_hover_text("Move down (Alt+Down)").clicked() && i + 1 < n {
                    move_up = Some(i + 1);
                }
            }
            if let Some(r) = row_btn(ui, Ico::Lock, locked, show) {
                if r.on_hover_text("Lock answer position (never shuffled)").clicked() {
                    answers[i].locked = !locked;
                }
            }
            if n > 2 {
                if let Some(r) = row_btn(ui, Ico::Close, false, show) {
                    if r.on_hover_text("Remove (Backspace in an empty row)").clicked() {
                        to_remove = Some(i);
                    }
                }
            } else {
                ui.add_space(24.0);
            }
        });
        ui.add_space(2.0);
    }
    if let Some(i) = to_remove {
        answers.remove(i);
    }
    if let Some(i) = move_up {
        answers.swap(i, i - 1);
    }
    ui.add_space(2.0);
    // Parts are capped at six answer choices; top-level questions are not.
    if answers.len() < max {
        let b = ghost_button(ui, "+ Add answer");
        if eq.take_add(&add_anchor) {
            b.request_focus();
        }
        if b.clicked() {
            answers.push(empty_answer());
            eq.focus(&fld(answers.len() - 1), 0);
        }
    }
}

/// Matching pairs and extra distractors (the "model answer" for MATCH). Keys as in the answer
/// list: Enter goes left item -> right item -> next pair (a new one after the last), Tab and
/// Shift+Tab walk the same order, Backspace in an empty pair removes it.
fn matching_editor(ui: &mut Ui, eq: &EqCtx, pairs: &mut Vec<Pair>, distractors: &mut Vec<String>, w: f32, stem_len: usize) {
    let mut remove_pair: Option<usize> = None;
    let mut remove_dis: Option<usize> = None;
    let side = |j: usize| if j % 2 == 0 { EqField::MatchL(j / 2) } else { EqField::MatchR(j / 2) };
    let side_len = |pairs: &Vec<Pair>, j: usize| if j % 2 == 0 { chars(&pairs[j / 2].left) } else { chars(&pairs[j / 2].right) };
    let last_right = |pairs: &Vec<Pair>| (EqField::MatchR(pairs.len().saturating_sub(1)), pairs.last().map_or(0, |p| chars(&p.right)));
    if let Some(f) = eq.last_focus() {
        let pj = (0..pairs.len() * 2).find(|&j| side(j) == f);
        let di = (0..distractors.len()).find(|&i| EqField::Distractor(i) == f);
        if let Some(j) = pj {
            let n = pairs.len() * 2;
            let both_empty = crate::wysiwyg::visibly_empty(&pairs[j / 2].left) && crate::wysiwyg::visibly_empty(&pairs[j / 2].right);
            if let Some(nav) = take_nav(&eq.ctx, true, both_empty) {
                match nav_step(nav, j, n, true, pairs.len() > 2 && both_empty) {
                    Step::Goto(k) => eq.focus(&side(k), side_len(pairs, k)),
                    Step::Add => {
                        pairs.push(Pair { left: String::new(), right: String::new() });
                        eq.focus(&EqField::MatchL(pairs.len() - 1), 0);
                    }
                    Step::AddButton => eq.want_add(EqField::MatchL(0)),
                    Step::Before => eq.focus(&EqField::Stem, stem_len),
                    Step::Remove(_) => {
                        let p = j / 2;
                        remove_pair = Some(p);
                        if p == 0 {
                            eq.focus(&EqField::MatchL(0), chars(&pairs[1].left));
                        } else {
                            eq.focus(&EqField::MatchR(p - 1), chars(&pairs[p - 1].right));
                        }
                    }
                    Step::Leave => eq.leave(),
                    Step::Stay => {}
                }
            }
        } else if let Some(i) = di {
            let empty = crate::wysiwyg::visibly_empty(&distractors[i]);
            if let Some(nav) = take_nav(&eq.ctx, true, empty) {
                match nav_step(nav, i, distractors.len(), true, empty) {
                    Step::Goto(k) => eq.focus(&EqField::Distractor(k), chars(&distractors[k])),
                    Step::Add => {
                        distractors.push(String::new());
                        eq.focus(&EqField::Distractor(distractors.len() - 1), 0);
                    }
                    Step::AddButton => eq.want_add(EqField::Distractor(0)),
                    Step::Before => {
                        let (f, n) = last_right(pairs);
                        eq.focus(&f, n);
                    }
                    Step::Remove(r) => {
                        remove_dis = Some(r);
                        if r > 0 {
                            eq.focus(&EqField::Distractor(r - 1), chars(&distractors[r - 1]));
                        } else {
                            let (f, n) = last_right(pairs);
                            eq.focus(&f, n);
                        }
                    }
                    Step::Leave => eq.leave(),
                    Step::Stay => {}
                }
            }
        }
    }
    label(ui, "Pairs  -  left item, right item");
    let ctrl = 24.0;
    let fw = ((w - 20.0 - ctrl - 3.0 * 6.0) / 2.0).max(80.0);
    for i in 0..pairs.len() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let start = ui.cursor().min;
            let a = eq_line(ui, eq, EqField::MatchL(i), &mut pairs[i].left, fw, "Left");
            // painter arrow
            let (r, _) = ui.allocate_exact_size(Vec2::new(20.0, 24.0), Sense::hover());
            let s = Stroke::new(1.5, theme::muted_col());
            let c = r.center();
            ui.painter().line_segment([c + Vec2::new(-5.0, 0.0), c + Vec2::new(5.0, 0.0)], s);
            ui.painter().line_segment([c + Vec2::new(5.0, 0.0), c + Vec2::new(2.0, -3.0)], s);
            ui.painter().line_segment([c + Vec2::new(5.0, 0.0), c + Vec2::new(2.0, 3.0)], s);
            let b = eq_line(ui, eq, EqField::MatchR(i), &mut pairs[i].right, fw, "Right");
            let row = Rect::from_min_max(start, Pos2::new(start.x + w, ui.min_rect().bottom().max(start.y + 24.0)));
            let show = a.focused || b.focused || ui.rect_contains_pointer(row);
            if let Some(r) = row_btn(ui, Ico::Close, false, show) {
                if r.on_hover_text("Remove pair (Backspace in an empty pair)").clicked() {
                    remove_pair = Some(i);
                }
            }
        });
        ui.add_space(2.0);
    }
    if let Some(i) = remove_pair {
        pairs.remove(i);
    }
    let b = ghost_button(ui, "+ Add pair");
    if eq.take_add(&EqField::MatchL(0)) {
        b.request_focus();
    }
    if b.clicked() {
        pairs.push(Pair { left: String::new(), right: String::new() });
        eq.focus(&EqField::MatchL(pairs.len() - 1), 0);
    }

    ui.add_space(8.0);
    label(ui, "Distractors  -  extra right-side items");
    for i in 0..distractors.len() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let start = ui.cursor().min;
            let out = eq_line(ui, eq, EqField::Distractor(i), &mut distractors[i], (w - 30.0).max(120.0), "Distractor");
            let row = Rect::from_min_max(start, Pos2::new(start.x + w, ui.min_rect().bottom().max(start.y + 24.0)));
            let show = out.focused || ui.rect_contains_pointer(row);
            if let Some(r) = row_btn(ui, Ico::Close, false, show) {
                if r.on_hover_text("Remove (Backspace in an empty row)").clicked() {
                    remove_dis = Some(i);
                }
            }
        });
        ui.add_space(2.0);
    }
    if let Some(i) = remove_dis {
        distractors.remove(i);
    }
    let b = ghost_button(ui, "+ Add distractor");
    if eq.take_add(&EqField::Distractor(0)) {
        b.request_focus();
    }
    if b.clicked() {
        distractors.push(String::new());
        eq.focus(&EqField::Distractor(distractors.len() - 1), 0);
    }
}

/// The Fill-in-the-blank answer rows (raw text, alternatives joined with " OR "), one per `___`
/// in the stem, from context memory (or the stored answers the first time).
fn blank_mirror(ctx: &egui::Context, key: UiId, draft: &Draft) -> Vec<String> {
    let n = blank_count(&draft.stem);
    ctx.data_mut(|d| {
        let st = d.get_temp_mut_or_default::<Temp>(key);
        if st.blank_texts.is_none()
            && let Kind::Blank { accept } = &draft.kind
        {
            st.blank_texts = Some(accept.iter().map(|v| v.join(" OR ")).collect());
        }
        let mut v = st.blank_texts.clone().unwrap_or_default();
        while v.len() < n {
            v.push(String::new());
        }
        v
    })
}

/// Fill in the blank: one accepted-alternatives row per "___" in the stem.
fn blank_editor(ui: &mut Ui, eq: &EqCtx, draft: &mut Draft, w: f32) {
    let key = eq.key;
    let ctx = &eq.ctx;
    let blanks = blank_count(&draft.stem).max(1);

    let mut texts = ctx.data_mut(|d| {
        let st = d.get_temp_mut_or_default::<Temp>(key);
        if st.blank_texts.is_none() {
            if let Kind::Blank { accept } = &draft.kind {
                st.blank_texts = Some(accept.iter().map(|v| v.join(" OR ")).collect());
            }
        }
        let texts = st.blank_texts.get_or_insert_with(Vec::new);
        while texts.len() < blanks {
            texts.push(String::new());
        }
        texts.clone()
    });
    if let Some(i) = eq.last_focus().and_then(|f| (0..blanks).find(|&i| EqField::Blank(i) == f)) {
        if let Some(nav) = take_nav(&eq.ctx, true, false) {
            match nav_step(nav, i, blanks, false, false) {
                Step::Goto(j) => eq.focus(&EqField::Blank(j), chars(&texts[j])),
                Step::Before => eq.focus(&EqField::Stem, chars(&draft.stem)),
                Step::Leave => eq.leave(),
                _ => {}
            }
        }
    }
    for i in 0..blanks {
        label(ui, &format!("Blank {}  -  accepted answers, separate alternatives with OR", i + 1));
        eq_line(ui, eq, EqField::Blank(i), &mut texts[i], w, "e.g. 1 OR one");
        ui.add_space(4.0);
    }
    // Mirror -> draft; the text the user types stays in the mirror.
    let accept = texts.iter().map(|t| split_alts(t)).collect::<Vec<Vec<String>>>();
    draft.kind = Kind::Blank { accept };
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Temp>(key).blank_texts = Some(texts));
}

/// Multipart: each part has a stem, a kind selector and an expanded editor below.
fn multipart_editor(ui: &mut Ui, eq: &EqCtx, parts: &mut Vec<Part>, w: f32, keyless: bool, tf_keyed: &mut Vec<Option<usize>>) {
    let mut remove: Option<usize> = None;
    for i in 0..parts.len() {
        let cur = match &parts[i].kind {
            PartKind::Mc { .. } => 0,
            PartKind::Tf { .. } => 1,
            PartKind::Short { .. } => 2,
        };
        let mut sel = cur;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.label(theme::muted(format!("{}.", i + 1)).size(12.0));
            eq_line_multi(ui, eq, EqField::PartStem(i), &mut parts[i].stem, (w - 24.0 - 3.0 * 52.0 - 30.0).max(120.0), "Part stem");
            for (v, tag) in [(0, "MC"), (1, "TF"), (2, "Short")] {
                if kit::chip_toggle(ui, tag, sel == v).clicked() {
                    sel = v;
                }
            }
            if parts.len() > 1 && icon_btn(ui, Ico::Close, false).on_hover_text("Remove part").clicked() {
                remove = Some(i);
            }
        });
        if sel != cur {
            parts[i].kind = match sel {
                0 => PartKind::Mc {
                    answers: empty_answers(3),
                },
                1 => PartKind::Tf { correct: true },
                _ => PartKind::Short {
                    sample: String::new(),
                },
            };
        }
        let stem_i = parts[i].stem.clone();
        ui.push_id(i, |ui| {
            ui.add_space(2.0);
            Frame::new().inner_margin(Margin::symmetric(18, 6)).show(ui, |ui| {
                let iw = w - 36.0;
                match &mut parts[i].kind {
                    PartKind::Mc { answers } => mc_editor(ui, eq, answers, 6, iw, &|j| EqField::PartMc(i, j), (EqField::PartStem(i), chars(&stem_i))),
                    PartKind::Tf { correct } => {
                        let unset = keyless && !tf_keyed.contains(&Some(i));
                        for (val, name) in [(true, "True"), (false, "False")] {
                            ui.horizontal(|ui| {
                                if kit::check_circle(ui, !unset && *correct == val).clicked() {
                                    *correct = val;
                                    if !tf_keyed.contains(&Some(i)) {
                                        tf_keyed.push(Some(i));
                                    }
                                }
                                ui.add_space(4.0);
                                ui.label(theme::text(name).size(13.0));
                            });
                        }
                    }
                    PartKind::Short { sample } => {
                        eq_multi(ui, eq, EqField::PartShort(i), sample, iw, 2, "Model answer");
                    }
                }
            });
        });
    }
    if let Some(i) = remove {
        parts.remove(i);
    }
    ui.add_space(2.0);
    if ghost_button(ui, "+ Add part").clicked() {
        parts.push(Part {
            stem: String::new(),
            kind: PartKind::Tf { correct: true },
        });
        eq.focus(&EqField::PartStem(parts.len() - 1), 0);
    }
}

// ---------- images ----------

/// Loads (and caches in the shared media map) the texture for a stored image.
fn media_texture(
    textures: &mut HashMap<String, egui::TextureHandle>,
    ctx: &egui::Context,
    file: &str,
) -> Option<egui::TextureHandle> {
    if let Some(t) = textures.get(file) {
        return Some(t.clone());
    }
    let bytes = crate::store::read_media(file)?;
    let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let tex = ctx.load_texture(
        format!("testparrot:{file}"),
        egui::ColorImage::from_rgba_unmultiplied(
            [img.width().max(1) as usize, img.height().max(1) as usize],
            &img,
        ),
        egui::TextureOptions::LINEAR,
    );
    textures.insert(file.to_string(), tex.clone());
    Some(tex)
}

/// Attach / preview / remove image row.
fn draw_image_row(
    ui: &mut Ui,
    draft: &mut Draft,
    textures: &mut HashMap<String, egui::TextureHandle>,
) {
    let key = UiId::new("testparrot_qedit_media_status");

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
        if ghost_button(ui, "Attach image...").clicked()
            && let Some(path) = crate::platform::pick_file("Images", &["png", "jpg", "jpeg", "gif", "webp"])
        {
            match crate::platform::read(&path) {
                Ok(bytes) => match crate::store::put_image(&bytes) {
                    Ok(img) => {
                        media_texture(textures, ui.ctx(), &img.file);
                        draft.image = Some(img);
                        draft.figure = None;
                        ui.ctx().data_mut(|d| d.insert_temp(key, None::<(String, bool)>));
                    }
                    Err(e) => {
                        ui.ctx().data_mut(|d| d.insert_temp(key, Some((e, true))));
                    }
                },
                Err(e) => ui.ctx().data_mut(|d| {
                    d.insert_temp(key, Some((format!("Couldn't read the file: {e}"), true)));
                }),
            }
        }
        if draft.image.is_some() && ghost_button(ui, "Remove").clicked() {
            draft.image = None;
            draft.figure = None;
        }
        if ghost_button(ui, "Insert figure...").clicked() {
            crate::figuresui::request(ui.ctx(), crate::figuresui::REQ_INSERT);
        }
        if draft.figure.is_some() && ghost_button(ui, "Edit figure").clicked() {
            crate::figuresui::request(ui.ctx(), crate::figuresui::REQ_EDIT);
        }
        if ghost_button(ui, "Import SVG/PNG...").clicked() {
            crate::figuresui::request(ui.ctx(), crate::figuresui::REQ_IMPORT);
        }
    });
    if let Some(f) = &draft.figure {
        ui.label(theme::muted(format!("Generated figure: {}", f.id)).size(11.0));
    }

    if let Some(status) = ui.ctx().data_mut(|d| d.get_temp::<(String, bool)>(key)) {
        if !status.0.is_empty() {
            red(ui, &status.0);
        }
    }

    if let Some(img) = draft.image.clone()
        && let Some(tex) = media_texture(textures, ui.ctx(), &img.file)
    {
        ui.add_space(4.0);
        Frame::new()
            .fill(theme::field())
            .corner_radius(CornerRadius::same(kit::R_FIELD))
            .inner_margin(Margin::same(6))
            .show(ui, |ui| {
                ui.add(egui::Image::new(&tex).max_height(110.0));
            });
    }
}

// ---------- tests ----------

#[cfg(test)]
mod tests {
    #[test]
    fn rich_preview_only_for_markup_or_math() {
        assert!(!renders_rich("What is 2+2?"));
        assert!(renders_rich("a\nb"));
        assert!(renders_rich("$x$"));
        assert!(renders_rich("**b**"));
    }

    use super::*;

    #[test]
    fn the_keyless_marker_cannot_be_edited_away() {
        let mut q = sample_question();
        q.kind = Kind::Tf { correct: false };
        q.explanation = Some(crate::model::KEYLESS_SENTINEL.into());
        // The teacher deletes the marker but enters no key: it comes back.
        let mut d = Draft::from_question("b".into(), &q);
        d.explanation = None;
        assert_eq!(d.build_question(Some(&q)).explanation.as_deref(), Some(crate::model::KEYLESS_SENTINEL));
        d.explanation = Some("Because it is a star.".into());
        assert_eq!(d.build_question(Some(&q)).explanation.as_deref(), Some(crate::model::KEYLESS_SENTINEL));
        // Entering a key (here: True) clears it; a kept marker text goes too.
        d.kind = Kind::Tf { correct: true };
        assert_eq!(d.build_question(Some(&q)).explanation.as_deref(), Some("Because it is a star."));
        d.explanation = Some(crate::model::KEYLESS_SENTINEL.into());
        assert_eq!(d.build_question(Some(&q)).explanation, None);
    }

    #[test]
    fn picking_false_keys_a_keyless_true_false() {
        let mut q = sample_question();
        q.kind = Kind::Tf { correct: false };
        q.explanation = Some(crate::model::KEYLESS_SENTINEL.into());
        let mut d = Draft::from_question("b".into(), &q);
        assert!(d.keyless, "shown unset");
        // Not picked: the placeholder false is not a key.
        d.explanation = Some("Why.".into());
        assert_eq!(d.build_question(Some(&q)).explanation.as_deref(), Some(crate::model::KEYLESS_SENTINEL));
        // The teacher clicks False (the placeholder's value): that is the key, one click.
        d.tf_keyed.push(None);
        assert_eq!(d.build_question(Some(&q)).explanation.as_deref(), Some("Why."));
        // A part: the same.
        q.kind = Kind::Multipart { parts: vec![Part { stem: "p".into(), kind: PartKind::Tf { correct: false } }] };
        let mut d = Draft::from_question("b".into(), &q);
        assert_eq!(d.build_question(Some(&q)).explanation.as_deref(), Some(crate::model::KEYLESS_SENTINEL));
        d.tf_keyed.push(Some(0));
        assert_eq!(d.build_question(Some(&q)).explanation, None);
    }

    fn sample_question() -> Question {
        Question {
            id: "q1".to_string(),
            stem: "What is 2+2?".to_string(),
            kind: Kind::Mc {
                answers: vec![
                    Answer {
                        text: "4".to_string(),
                        correct: true,
                        locked: false,
                    },
                    Answer {
                        text: "5".to_string(),
                        correct: false,
                        locked: false,
                    },
                ],
            },
            points: Some(2.0),
            difficulty: Some(Difficulty::Medium),
            topics: vec!["arith".to_string(), "  arith ".to_string()],
            image: None,
            explanation: Some(" 2+2 makes four ".to_string()),
            figure: None,
            created: 1_700_000_000,
            modified: 1_700_000_100,
        }
    }

    #[test]
    fn new_draft_defaults_to_four_mc_answers() {
        let d = Draft::new("bank".to_string());
        assert!(d.qid.is_none());
        assert_eq!(d.stem, "");
        match &d.kind {
            Kind::Mc { answers } => {
                assert_eq!(answers.len(), 4);
                assert!(answers.iter().all(|a| a.text.is_empty() && !a.correct));
            }
            other => panic!("wrong default kind: {other:?}"),
        }
        assert_eq!(d.points, None);
        assert_eq!(d.difficulty, None);
        assert!(d.topics.is_empty());
    }

    #[test]
    fn empty_draft_fails_validation() {
        let d = Draft::new("bank".to_string());
        let errs = d.validate();
        assert!(errs.iter().any(|e| e.contains("stem")));
        assert!(errs.iter().any(|e| e.contains("choices")));
        assert!(errs.iter().any(|e| e.contains("correct")));
    }

    #[test]
    fn valid_mc_draft_passes() {
        let mut d = Draft::new("bank".to_string());
        d.stem = "Pick one".to_string();
        if let Kind::Mc { answers } = &mut d.kind {
            answers[0].text = "A".to_string();
            answers[1].text = "B".to_string();
            answers[0].correct = true;
        }
        assert!(d.validate().is_empty());
    }

    #[test]
    fn set_kind_resets_data_to_defaults() {
        let mut d = Draft::new("bank".to_string());
        d.stem = "x".to_string();
        d.set_kind(QKind::Tf);
        assert!(matches!(d.kind, Kind::Tf { correct: true }));
        d.set_kind(QKind::Matching);
        match &d.kind {
            Kind::Matching { pairs, distractors } => {
                assert_eq!(pairs.len(), 2);
                assert_eq!(distractors.len(), 1);
            }
            other => panic!("{other:?}"),
        }
        d.set_kind(QKind::Blank);
        assert!(matches!(&d.kind, Kind::Blank { accept } if accept.len() == 1));
        d.set_kind(QKind::Short);
        assert!(matches!(&d.kind, Kind::Short { sample } if sample.is_empty()));
        d.set_kind(QKind::Multipart);
        match &d.kind {
            Kind::Multipart { parts } => assert_eq!(parts.len(), 2),
            other => panic!("{other:?}"),
        }
        d.set_kind(QKind::Mc);
        assert!(matches!(&d.kind, Kind::Mc { answers } if answers.len() == 4));
    }

    #[test]
    fn enter_on_the_last_row_adds_and_elsewhere_walks() {
        assert_eq!(nav_step(Nav::Enter, 3, 4, true, false), Step::Add);
        assert_eq!(nav_step(Nav::Enter, 1, 4, true, false), Step::Goto(2));
        assert_eq!(nav_step(Nav::Enter, 3, 4, false, false), Step::Stay, "a capped list adds nothing");
    }

    #[test]
    fn tab_walks_stem_rows_add_button_in_order() {
        assert_eq!(nav_step(Nav::Tab, 0, 4, true, false), Step::Goto(1));
        assert_eq!(nav_step(Nav::Tab, 3, 4, true, false), Step::AddButton);
        assert_eq!(nav_step(Nav::Tab, 3, 4, false, false), Step::Leave, "no Add button: Tab moves on, never traps");
        assert_eq!(nav_step(Nav::BackTab, 2, 4, true, false), Step::Goto(1));
        assert_eq!(nav_step(Nav::BackTab, 0, 4, true, false), Step::Before, "back out of the first row");
    }

    #[test]
    fn backspace_removes_only_a_removable_empty_row() {
        assert_eq!(nav_step(Nav::Backspace, 2, 4, true, true), Step::Remove(2));
        assert_eq!(nav_step(Nav::Backspace, 1, 2, true, false), Step::Stay, "the minimum rows stay");
        assert_eq!(after_remove(0), 0);
        assert_eq!(after_remove(3), 2);
    }

    #[test]
    fn tab_from_the_stem_goes_to_the_first_answer_field() {
        let d = Draft::new("b".to_string());
        assert_eq!(first_field(&d.kind, None), Some((EqField::Mc(0), 0)));
        assert_eq!(first_field(&Kind::Tf { correct: true }, None), None, "true/false has no field to go to");
        assert_eq!(first_field(&default_kind(QKind::Matching), None), Some((EqField::MatchL(0), 0)));
        assert_eq!(first_field(&default_kind(QKind::Short), None), Some((EqField::Short, 0)));
        assert_eq!(first_field(&default_kind(QKind::Blank), Some(&["red".to_string()])), Some((EqField::Blank(0), 3)));
    }

    #[test]
    fn new_of_opens_the_chosen_type_empty() {
        for k in crate::bankops::KINDS {
            let d = Draft::new_of("b".to_string(), k);
            assert_eq!(d.qkind(), k);
            assert!(d.stem.is_empty() && !d.is_edit());
        }
    }

    fn mc(texts: &[(&str, bool)]) -> Kind {
        Kind::Mc { answers: texts.iter().map(|(t, c)| Answer { text: t.to_string(), correct: *c, locked: false }).collect() }
    }

    #[test]
    fn mc_to_short_carries_the_correct_answer_and_names_the_rest() {
        let p = plan_switch("Capital?", &mc(&[("Paris", true), ("Rome", false), ("Oslo", false), ("", false)]), QKind::Short, false);
        assert_eq!(p.stem, "Capital?");
        assert!(matches!(&p.kind, Kind::Short { sample } if sample == "Paris"));
        assert_eq!(p.lost, vec!["2 other answer choices".to_string()]);
        assert!(p.needs_confirm() && !p.ask_tf);
        assert_eq!(p.summary(), "Drops 2 other answer choices. Keeps the question text and the correct answer as the model answer.");
    }

    #[test]
    fn mc_to_tf_never_keys_true_on_its_own() {
        let p = plan_switch("s", &mc(&[("Paris", true), ("Rome", false), ("", false), ("", false)]), QKind::Tf, false);
        assert!(p.ask_tf, "the teacher picks True or False");
        assert!(p.lost.contains(&"the answer key".to_string()) && p.lost.contains(&"2 answer choices".to_string()));
        // a single correct choice that reads True/False does carry
        let p = plan_switch("s", &mc(&[("True", false), ("False", true), ("", false), ("", false)]), QKind::Tf, false);
        assert!(!p.ask_tf);
        assert!(matches!(p.kind, Kind::Tf { correct: false }));
        assert!(p.kept.iter().any(|k| k.contains("False")));
        // nothing entered: nothing to key or lose
        let p = plan_switch("s", &default_kind(QKind::Mc), QKind::Tf, false);
        assert!(!p.needs_confirm());
    }

    #[test]
    fn switching_with_the_choice_keys_it_and_keeps_the_stem() {
        let mut d = Draft::new("b".to_string());
        d.stem = "Paris is the capital.".into();
        d.kind = mc(&[("Yes", true), ("No", false), ("", false), ("", false)]);
        assert!(d.switch_plan(QKind::Tf).ask_tf);
        d.switch_to(QKind::Tf, Some(false));
        assert!(matches!(d.kind, Kind::Tf { correct: false }));
        assert_eq!(d.stem, "Paris is the capital.");
        assert!(d.tf_keyed.contains(&None), "the picked value counts as a key");
    }

    #[test]
    fn short_and_tf_carry_across() {
        let p = plan_switch("s", &Kind::Short { sample: " True. ".into() }, QKind::Tf, false);
        assert!(matches!(p.kind, Kind::Tf { correct: true }) && !p.needs_confirm());
        let p = plan_switch("s", &Kind::Short { sample: "Because of gravity".into() }, QKind::Tf, false);
        assert!(p.ask_tf && p.lost == vec!["the model answer".to_string()]);
        let p = plan_switch("s", &Kind::Short { sample: "Because".into() }, QKind::Mc, false);
        match &p.kind {
            Kind::Mc { answers } => assert!(answers[0].text == "Because" && answers[0].correct && answers.len() == 4),
            o => panic!("{o:?}"),
        }
        assert!(!p.needs_confirm());
        // an untouched default True/False is not content; a keyed one is
        let p = plan_switch("s", &Kind::Tf { correct: false }, QKind::Mc, false);
        assert!(matches!(&p.kind, Kind::Mc { answers } if answers.iter().all(|a| a.text.is_empty())));
        let p = plan_switch("s", &Kind::Tf { correct: false }, QKind::Mc, true);
        assert!(matches!(&p.kind, Kind::Mc { answers } if answers[1].text == "False" && answers[1].correct));
    }

    #[test]
    fn leaving_fill_puts_the_blanks_back_in_the_text() {
        let kind = Kind::Blank { accept: vec![vec!["Paris".into(), "paris".into()], vec![]] };
        let p = plan_switch("The capital of ___ is ___.", &kind, QKind::Short, false);
        assert_eq!(p.stem, "The capital of Paris is (blank).");
        assert_eq!(blank_count(&p.stem), 0, "no marker is left behind");
        assert_eq!(p.lost, vec!["the extra accepted alternatives".to_string()]);
        let p = plan_switch("A ___ b", &Kind::Blank { accept: vec![vec!["x".into()]] }, QKind::Mc, false);
        assert_eq!(p.stem, "A x b");
        assert!(!p.needs_confirm(), "the answer is in the text, nothing is lost");
    }

    #[test]
    fn other_switches_name_exactly_what_is_dropped() {
        let k = Kind::Matching { pairs: vec![Pair { left: "a".into(), right: "b".into() }, Pair { left: String::new(), right: String::new() }], distractors: vec!["z".into()] };
        let p = plan_switch("s", &k, QKind::Mc, false);
        assert_eq!(p.lost, vec!["1 matching pair".to_string(), "1 distractor".to_string()]);
        assert!(matches!(p.kind, Kind::Mc { .. }));
        assert!(!plan_switch("s", &default_kind(QKind::Matching), QKind::Short, false).needs_confirm());
    }

    fn tf_question(keyless: bool, correct: bool) -> Question {
        let mut q = sample_question();
        q.kind = Kind::Tf { correct };
        q.explanation = if keyless { Some(crate::model::KEYLESS_SENTINEL.to_string()) } else { None };
        q
    }

    #[test]
    fn keyless_true_false_to_mc_invents_no_key() {
        let d = Draft::from_question("b".into(), &tf_question(true, false));
        assert!(d.keyless);
        let p = d.switch_plan(QKind::Mc);
        match &p.kind {
            Kind::Mc { answers } => assert!(answers.iter().all(|a| a.text.is_empty() && !a.correct), "the placeholder false is not a choice"),
            o => panic!("{o:?}"),
        }
        assert!(p.kept.is_empty());
    }

    #[test]
    fn keyless_true_false_to_short_invents_no_model_answer() {
        let mut d = Draft::from_question("b".into(), &tf_question(true, false));
        d.switch_to(QKind::Short, None);
        assert!(matches!(&d.kind, Kind::Short { sample } if sample.is_empty()));
        // and the saved question still says it has no key
        let q = d.build_question(Some(&tf_question(true, false)));
        assert_eq!(q.explanation.as_deref(), Some(crate::model::KEYLESS_SENTINEL));
    }

    #[test]
    fn keyed_true_false_to_mc_carries_the_key() {
        let d = Draft::from_question("b".into(), &tf_question(false, false));
        match &d.switch_plan(QKind::Mc).kind {
            Kind::Mc { answers } => assert!(answers[0].text == "True" && !answers[0].correct && answers[1].text == "False" && answers[1].correct),
            o => panic!("{o:?}"),
        }
        // a keyless one whose teacher then picked a value counts as keyed
        let mut k = Draft::from_question("b".into(), &tf_question(true, false));
        k.tf_keyed.push(None);
        assert!(matches!(&k.switch_plan(QKind::Short).kind, Kind::Short { sample } if sample == "False"));
    }

    #[test]
    fn imported_keyless_true_false_round_trips_unchanged() {
        let q = tf_question(true, false);
        let d = Draft::from_question("b".into(), &q);
        let out = d.build_question(Some(&q));
        assert_eq!(format!("{:?}", out.kind), format!("{:?}", q.kind));
        assert_eq!(out.explanation.as_deref(), Some(crate::model::KEYLESS_SENTINEL));
        assert_eq!(out.stem, q.stem);
    }

    #[test]
    fn topic_hint_reads_by_state() {
        assert_eq!(topic_hint(true, true), "Enter adds this topic.");
        assert_eq!(topic_hint(false, false), "Enter adds a new topic.");
        assert_eq!(topic_hint(false, true), "Matching:");
    }

    #[test]
    fn close_step_asks_only_when_dirty() {
        assert_eq!(close_step(false), CloseStep::CloseNow);
        assert_eq!(close_step(true), CloseStep::AskDiscard);
    }

    #[test]
    fn dirty_tracks_edits_against_the_baseline() {
        let mut d = Draft::new("bank".to_string());
        assert!(!d.is_dirty(), "no baseline yet means not dirty");
        d.baseline = Some(d.fingerprint());
        assert!(!d.is_dirty());
        d.stem.push('x');
        assert!(d.is_dirty());
        d.stem.pop();
        assert!(!d.is_dirty(), "undoing the edit by hand is clean again");
        d.set_points_text("2");
        assert!(d.is_dirty());
        d.set_points_text("");
        d.topics.push("t".into());
        assert!(d.is_dirty());
        d.topics.clear();
        if let Kind::Mc { answers } = &mut d.kind {
            answers[0].correct = true;
        }
        assert!(d.is_dirty());
    }

    #[test]
    fn type_switch_warns_only_when_answers_would_be_lost() {
        let mut d = Draft::new("bank".to_string());
        d.stem = "stem only".into();
        assert!(!d.kind_has_content(), "empty default answers lose nothing");
        if let Kind::Mc { answers } = &mut d.kind {
            answers[1].text = "four".into();
        }
        assert!(d.kind_has_content());
        d.set_kind(QKind::Tf);
        assert!(!d.kind_has_content(), "true/false has nothing to lose");
        d.set_kind(QKind::Short);
        assert!(!d.kind_has_content());
        if let Kind::Short { sample } = &mut d.kind {
            *sample = "model".into();
        }
        assert!(d.kind_has_content());
        d.set_kind(QKind::Blank);
        assert!(!d.kind_has_content());
        d.set_kind(QKind::Multipart);
        if let Kind::Multipart { parts } = &mut d.kind {
            parts[0].stem = "a".into();
        }
        assert!(d.kind_has_content());
    }

    #[test]
    fn dirty_ignores_nothing_that_is_saved() {
        let q = sample_question();
        let mut d = Draft::from_question("bank".to_string(), &q);
        d.baseline = Some(d.fingerprint());
        d.explanation = Some("why".into());
        assert!(d.is_dirty());
        d.explanation = q.explanation.clone();
        assert!(!d.is_dirty());
    }

    #[test]
    fn points_parsing() {
        let mut d = Draft::new("bank".to_string());
        d.set_points_text("3");
        assert_eq!(d.points, Some(3.0));
        assert_eq!(d.points_text(), "3");
        d.set_points_text("3.5");
        assert_eq!(d.points, Some(3.5));
        d.set_points_text("");
        assert_eq!(d.points, None);
        d.set_points_text("junk");
        assert_eq!(d.points, None);
    }

    #[test]
    fn from_question_and_build_roundtrip() {
        let q = sample_question();
        let d = Draft::from_question("bank".to_string(), &q);
        assert!(d.is_edit());
        let rebuilt = d.build_question(Some(&q));
        assert_eq!(rebuilt.id, q.id);
        assert_eq!(rebuilt.stem, q.stem);
        assert_eq!(rebuilt.topics, vec!["arith".to_string()]);
        assert_eq!(rebuilt.explanation.as_deref(), Some("2+2 makes four"));
        assert_eq!(rebuilt.points, Some(2.0));
        assert_eq!(rebuilt.created, 1_700_000_000);
    }

    #[test]
    fn blank_validation_matches_blanks() {
        let mut d = Draft::new("bank".to_string());
        d.kind = Kind::Blank {
            accept: vec![vec!["one".to_string()], vec![]],
        };
        d.stem = "Fill ___ and ___ here.".to_string();
        let errs = d.validate();
        assert!(errs.iter().any(|e| e.contains("Blank 2")));
        // no blank yet: the error tells the same flow as the stem hint (select a word -> Blank)
        d.stem = "Fill here.".to_string();
        let errs = d.validate();
        assert!(errs.iter().any(|e| e.contains(BLANK_HOW)), "{errs:?}");
        assert!(BLANK_HOW.contains("select a word") && BLANK_HOW.contains("___ Blank"));
        d.stem = "Fill ___ and ___ here.".to_string();
        d.kind = Kind::Blank {
            accept: vec![vec!["one".to_string()], vec!["two".to_string()]],
        };
        assert!(d.validate().is_empty());
    }

    #[test]
    fn picture_markers_preview_as_placeholders() {
        let t = "[picture: Handwritten key for NH3]\nNumber of charge clouds: 4\nShape: $x^2$";
        assert!(has_picture(t));
        assert!(renders_rich(t), "a picture marker gets a preview even without markup");
        let runs = preview_runs(t);
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0], (None, "Handwritten key for NH3".to_string()));
        let (Some(at), run) = &runs[1] else { panic!("text run expected") };
        // The run's char offset maps a click on its equation back into the full text.
        assert_eq!(t.chars().skip(*at).take(run.chars().count()).collect::<String>(), *run);
        assert!(!has_picture("a [pictures] b"));
        assert!(!renders_rich("Number of charge clouds: 4"));
    }
}

// ---------- equation panel (opened by the sigma button, a word-initial '/', or a click on a chip) ----------

thread_local! {
    /// The one equation editor on screen (created when a request appears, dropped when it closes).
    static EQ_PANEL: std::cell::RefCell<Option<crate::equation::EquationEditor>> = const { std::cell::RefCell::new(None) };
}

/// The text of `f` inside the draft (the Blank rows live in the context-memory mirror instead).
fn field_text<'a>(draft: &'a mut Draft, f: &EqField) -> Option<&'a mut String> {
    match f {
        EqField::Stem => Some(&mut draft.stem),
        EqField::Mc(i) => match &mut draft.kind {
            Kind::Mc { answers } => answers.get_mut(*i).map(|a| &mut a.text),
            _ => None,
        },
        EqField::MatchL(i) => match &mut draft.kind {
            Kind::Matching { pairs, .. } => pairs.get_mut(*i).map(|p| &mut p.left),
            _ => None,
        },
        EqField::MatchR(i) => match &mut draft.kind {
            Kind::Matching { pairs, .. } => pairs.get_mut(*i).map(|p| &mut p.right),
            _ => None,
        },
        EqField::Distractor(i) => match &mut draft.kind {
            Kind::Matching { distractors, .. } => distractors.get_mut(*i),
            _ => None,
        },
        EqField::Short => match &mut draft.kind {
            Kind::Short { sample } => Some(sample),
            _ => None,
        },
        EqField::PartStem(i) => match &mut draft.kind {
            Kind::Multipart { parts } => parts.get_mut(*i).map(|p| &mut p.stem),
            _ => None,
        },
        EqField::PartMc(i, j) => match &mut draft.kind {
            Kind::Multipart { parts } => match parts.get_mut(*i).map(|p| &mut p.kind) {
                Some(PartKind::Mc { answers }) => answers.get_mut(*j).map(|a| &mut a.text),
                _ => None,
            },
            _ => None,
        },
        EqField::PartShort(i) => match &mut draft.kind {
            Kind::Multipart { parts } => match parts.get_mut(*i).map(|p| &mut p.kind) {
                Some(PartKind::Short { sample }) => Some(sample),
                _ => None,
            },
            _ => None,
        },
        EqField::Explanation => Some(draft.explanation.get_or_insert_with(String::new)),
        EqField::Blank(_) => None,
    }
}

/// Splice `$linear$` into the requested field (replacing the clicked equation, if any).
fn apply_equation(ctx: &egui::Context, draft: &mut Draft, key: UiId, req: &EqReq, linear: &str) {
    let wrapped = crate::equation::wrap_math(linear);
    let at = req.span.as_ref().map_or(req.caret, |r| r.start);
    let caret = at + wrapped.chars().count();
    let id = field_editor_id(key, &req.field);
    if let EqField::Blank(i) = req.field {
        ctx.data_mut(|d| {
            if let Some(t) = d.get_temp_mut_or_default::<Temp>(key).blank_texts.as_mut().and_then(|v| v.get_mut(i)) {
                let old = t.clone();
                *t = crate::equation::splice(t, req.span.clone(), req.caret, &wrapped);
                crate::wysiwyg::record_external(ctx, id, &old, caret, None);
            }
        });
    } else {
        let is_blank = matches!(draft.kind, Kind::Blank { .. });
        let rows = (req.field == EqField::Stem && is_blank).then(|| blank_mirror(ctx, key, draft));
        if let Some(t) = field_text(draft, &req.field) {
            let old = t.clone();
            *t = crate::equation::splice(t, req.span.clone(), req.caret, &wrapped);
            let new = t.clone();
            crate::wysiwyg::record_external(ctx, id, &old, caret, rows.as_deref());
            if let Some(r) = rows {
                let r = crate::wysiwyg::remap_blanks(&old, &new, &r);
                ctx.data_mut(|d| d.get_temp_mut_or_default::<Temp>(key).blank_texts = Some(r));
            }
        }
    }
}

/// The panel's estimated height, used to decide whether it opens below or above the field.
const PANEL_HEIGHT: f32 = 250.0;

/// Where the equation panel opens: at `anchor`, moved above/left when it would run off the
/// screen, and always kept inside `screen` with an 8 px margin.
fn panel_pos(anchor: Pos2, size: Vec2, screen: Rect) -> Pos2 {
    const M: f32 = 8.0;
    let min_x = screen.left() + M;
    let min_y = screen.top() + M;
    let max_x = (screen.right() - size.x - M).max(min_x);
    let max_y = (screen.bottom() - size.y - M).max(min_y);
    let x = if anchor.x > max_x { anchor.x - size.x } else { anchor.x };
    let y = if anchor.y > max_y { anchor.y - size.y } else { anchor.y };
    Pos2::new(x.clamp(min_x, max_x), y.clamp(min_y, max_y))
}

/// Shows the equation editor in a panel anchored to the field that asked for it, and splices
/// the finished `$linear$` into that field at the remembered caret (or over the equation being
/// edited).
fn equation_panel(ctx: &egui::Context, draft: &mut Draft, key: UiId) {
    let Some(req) = ctx.data_mut(|d| d.get_temp_mut_or_default::<Temp>(key).eq_req.clone()) else {
        return;
    };
    let mut verdict: Option<bool> = None; // Some(true) = insert, Some(false) = cancel
    let linear = EQ_PANEL.with(|p| {
        let mut slot = p.borrow_mut();
        let ed = slot.get_or_insert_with(|| {
            crate::equation::EquationEditor::with_linear(&req.initial)
                .id_salt("vershion_equation_panel")
                .font_size(26.0)
                .canvas_height(110.0)
                .auto_focus(true)
        });
        let editing = req.span.is_some();
        let screen = ctx.content_rect();
        let w = 640.0_f32.min((screen.width() - 16.0).max(320.0));
        let pos = panel_pos(req.anchor, Vec2::new(w, PANEL_HEIGHT), screen);
        let area = egui::Area::new(UiId::new("qedit_equation_panel"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .constrain(true)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(w);
                    kit::small_caps(ui, if editing { "Edit equation" } else { "Insert equation" });
                    ui.add_space(2.0);
                    ui.label(
                        theme::muted("Type it the way you would say it: 1/2, sqrt(x), x^2, pi. Enter inserts, Esc cancels.").size(12.0),
                    );
                    ui.add_space(8.0);
                    let r = ed.show(ui);
                    if r.committed {
                        verdict = Some(true);
                    } else if r.cancelled {
                        verdict = Some(false);
                    }
                    ui.add_space(10.0);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if kit::primary_button(ui, if editing { "Update equation" } else { "Insert equation" }).clicked() {
                            verdict = Some(true);
                        }
                        if kit::ghost_button(ui, "Cancel").clicked() {
                            verdict = Some(false);
                        }
                    });
                });
            });
        // A press outside closes it, but the press that opened it does not.
        if ctx.cumulative_frame_nr() > req.frame {
            if let Some(p) = ctx.input(|i| i.pointer.interact_pos().filter(|_| i.pointer.any_pressed())) {
                if !area.response.rect.contains(p) {
                    verdict = Some(false);
                }
            }
        }
        ed.linear()
    });
    if let Some(insert) = verdict {
        if insert && !linear.trim().is_empty() {
            apply_equation(ctx, draft, key, &req, linear.trim());
        }
        EQ_PANEL.with(|p| *p.borrow_mut() = None);
        ctx.data_mut(|d| d.get_temp_mut_or_default::<Temp>(key).eq_req = None);
    }
}

#[cfg(test)]
mod panel_pos_tests {
    use super::panel_pos;
    use egui::{Pos2, Rect, Vec2};

    fn screen() -> Rect {
        Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(1000.0, 800.0))
    }

    #[test]
    fn roomy_panel_stays_at_the_anchor() {
        assert_eq!(panel_pos(Pos2::new(100.0, 120.0), Vec2::new(640.0, 250.0), screen()), Pos2::new(100.0, 120.0));
    }

    #[test]
    fn panel_flips_left_and_up_near_the_edges() {
        assert_eq!(panel_pos(Pos2::new(900.0, 700.0), Vec2::new(640.0, 250.0), screen()), Pos2::new(260.0, 450.0));
    }

    #[test]
    fn panel_is_kept_inside_a_small_screen() {
        let tiny = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(300.0, 200.0));
        assert_eq!(panel_pos(Pos2::new(280.0, 190.0), Vec2::new(640.0, 250.0), tiny), Pos2::new(8.0, 8.0));
    }

    #[test]
    fn negative_anchor_is_clamped() {
        assert_eq!(panel_pos(Pos2::new(-50.0, -20.0), Vec2::new(640.0, 250.0), screen()), Pos2::new(8.0, 8.0));
    }
}
