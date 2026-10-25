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
    crate::qcheck::blank_count(stem)
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
    /// The control the body last scrolled to (so a control taking focus is always brought into view).
    scrolled_id: Option<UiId>,
    /// Ctrl+Enter was pressed in a formula: save once it has been committed.
    save_after: bool,
    /// The confirm bar has already taken focus for its current question.
    confirm_focused: u8,
    /// What is missing, as far as it is shown inline now (see `refresh_issues`).
    issues: Vec<crate::qcheck::Issue>,
    /// Fields the focus has left at least once in this session (their warnings may show).
    left: Vec<EqField>,
    /// The field the focus was in at the start of the last frame.
    focus_seen: Option<EqField>,
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
}

/// Whether field `f` is part of where issue `at` is shown.
fn in_region(at: crate::qcheck::Where, f: &EqField) -> bool {
    use crate::qcheck::Where as W;
    match (at, f) {
        (W::Stem, EqField::Stem) => true,
        (W::Blanks, EqField::Stem | EqField::Blank(_)) => true,
        (W::Answers, EqField::Mc(_) | EqField::Short) => true,
        (W::Answer(i), EqField::Mc(j)) => i == *j,
        (W::Pairs, EqField::MatchL(_) | EqField::MatchR(_) | EqField::Distractor(_)) => true,
        (W::Pair(i), EqField::MatchL(j) | EqField::MatchR(j)) => i == *j,
        (W::Blank(i), EqField::Blank(j)) => i == *j,
        (W::Parts, EqField::PartStem(_) | EqField::PartMc(..) | EqField::PartShort(_)) => true,
        (W::Part(i), EqField::PartStem(j) | EqField::PartMc(j, _) | EqField::PartShort(j)) => i == *j,
        _ => false,
    }
}

/// An issue is shown inline once the teacher has been in its field and moved on (or at once for a
/// question that is being edited), and never while the caret is in that field: nothing nags
/// while the answer is being typed.
fn issue_shown(is_edit: bool, left: &[EqField], focus: Option<&EqField>, at: crate::qcheck::Where) -> bool {
    if focus.is_some_and(|f| in_region(at, f)) {
        return false;
    }
    // (a blank's row and a part are on screen the whole time, so their gaps show at once, visited or not)
    is_edit || matches!(at, crate::qcheck::Where::Blank(_) | crate::qcheck::Where::Part(_)) || left.iter().any(|f| in_region(at, f))
}

/// Works out, once per frame, which issues the fields draw. An empty draft shows none.
fn refresh_issues(ctx: &egui::Context, key: UiId, draft: &Draft) {
    let all = if draft.is_empty_draft() { Vec::new() } else { draft.issues() };
    ctx.data_mut(|d| {
        let t = d.get_temp_mut_or_default::<Temp>(key);
        if let Some(old) = t.focus_seen.take().filter(|o| t.focus_last.as_ref() != Some(o)) {
            if !t.left.contains(&old) {
                t.left.push(old);
            }
        }
        t.focus_seen = t.focus_last.clone();
        let (left, focus) = (t.left.clone(), t.focus_last.clone());
        let short = matches!(draft.kind, Kind::Short { .. });
        t.issues = all
            .into_iter()
            // (the model answer box is on screen the whole time: its gap shows at once too)
            .filter(|i| issue_shown(draft.is_edit() || (short && i.at == crate::qcheck::Where::Answers), &left, focus.as_ref(), i.at))
            .collect();
    });
}

/// Amber for a warning (a draft is not an error), readable on light and dark.
fn warn_col(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode { Color32::from_rgb(235, 175, 90) } else { Color32::from_rgb(166, 90, 0) }
}

impl EqCtx {
    /// The shown messages for the places `pred` picks, in order.
    fn warns(&self, pred: impl Fn(crate::qcheck::Where) -> bool) -> Vec<String> {
        self.temp(|t| t.issues.iter().filter(|i| pred(i.at)).map(|i| i.msg.clone()).collect())
    }

    /// One reserved line (it is always there, so a warning never moves a field) under a field or
    /// list: the first message, plus a count of the rest.
    fn warn_line(&self, ui: &mut Ui, w: f32, pred: impl Fn(crate::qcheck::Where) -> bool) {
        let msgs = self.warns(pred);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, WARN_H), Sense::hover());
        if let Some(first) = msgs.first() {
            let more = if msgs.len() > 1 { format!("  (+{} more)", msgs.len() - 1) } else { String::new() };
            ui.painter().with_clip_rect(rect).text(rect.left_center(), egui::Align2::LEFT_CENTER, format!("{first}{more}"), kit::ui_font(11.5), warn_col(ui));
        }
    }

    /// Row-level warning drawn inside an empty field, at its right end (no height of its own).
    fn warn_in(&self, ui: &Ui, field: Rect, at: crate::qcheck::Where) {
        if let Some(m) = self.warns(|w| w == at).first() {
            ui.painter().text(Pos2::new(field.right() - 10.0, field.center().y), egui::Align2::RIGHT_CENTER, m, kit::ui_font(11.5), warn_col(ui));
        }
    }
}

/// 'Saves as a draft: 2 things to fix' (`saved`: 'Saved as a draft: ...').
pub fn draft_summary(n: usize, saved: bool) -> String {
    format!("{} as a draft: {n} {} to fix", if saved { "Saved" } else { "Saves" }, if n == 1 { "thing" } else { "things" })
}

/// Height of the reserved warning line under a field.
const WARN_H: f32 = 16.0;

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

    /// What is missing from this draft, as the editor holds it (see `qcheck`). Saving never waits
    /// for any of it: a question with gaps is saved as a draft. The same rules gate the exam and
    /// the pool, through `qcheck::check_question` on the stored question.
    pub fn issues(&self) -> Vec<crate::qcheck::Issue> {
        use crate::qcheck::{Issue, Severity, Where};
        // (the rows of a Fill question beyond its blanks are not saved)
        let kind = match &self.kind {
            Kind::Blank { accept } => Kind::Blank { accept: accept.iter().take(blank_count(&self.stem)).cloned().collect() },
            k => k.clone(),
        };
        let mut v = crate::qcheck::check(&self.stem, &kind);
        // A question imported without its key: a True/False nobody has picked yet is no key.
        if self.keyless {
            let unset = |at: Option<usize>| !self.tf_keyed.contains(&at);
            match &self.kind {
                Kind::Tf { .. } if unset(None) => {
                    v.push(Issue { at: Where::Answers, severity: Severity::Fix, msg: "Pick True or False".to_string(), key: true });
                }
                Kind::Multipart { parts } => {
                    for (i, p) in parts.iter().enumerate() {
                        if matches!(p.kind, PartKind::Tf { .. }) && unset(Some(i)) {
                            v.push(Issue { at: Where::Part(i), severity: Severity::Fix, msg: format!("Part {} needs True or False picked", i + 1), key: true });
                        }
                    }
                }
                _ => {}
            }
        }
        v
    }

    /// The messages of what must be fixed before the question can go on an exam or be shared.
    pub fn validate(&self) -> Vec<String> {
        self.issues().into_iter().filter(|i| i.severity == crate::qcheck::Severity::Fix).map(|i| i.msg).collect()
    }

    /// A question with nothing in it: no stem and nothing typed into any answer. That is the one
    /// thing Save refuses; anything else saves, as a draft when it has gaps.
    pub fn is_empty_draft(&self) -> bool {
        self.stem.trim().is_empty() && content_items(&self.kind).is_empty()
    }

    /// Save is on unless the draft is empty.
    pub fn can_save(&self) -> bool {
        !self.is_empty_draft()
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
                // (a pair with one side typed is kept as typed: a draft loses nothing the teacher wrote)
                pairs: pairs
                    .iter()
                    .filter(|p| !p.left.trim().is_empty() || !p.right.trim().is_empty())
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

/// Rendered preview of a field that holds markup or equations; clicking an equation edits it in
/// the field above.
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
        // (the preview is not where the equation is edited: the field above it is)
        let _ = r;
        crate::wysiwyg::edit_equation(&eq.ctx, field_editor_id(eq.key, &field), h.span);
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
        // edited in place at the caret (or around the selection), no dialog
        crate::wysiwyg::request_equation(&eq.ctx, id);
    }
    if has_picture(text) {
        eq_preview(ui, eq, field, text, 0.0, 13.0);
    }
    out
}

/// After a field drew: draw its focus ring, remember it for key routing, scroll it into view if
/// it was just added.
fn mark_field(ui: &mut Ui, eq: &EqCtx, field: &EqField, out: &crate::wysiwyg::Out, r: Rect) {
    if out.focused || out.math {
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

/// A section label: sentence case, readable, bold.
fn label(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).family(theme::bold_family()).size(13.5).color(theme::ink()));
    ui.add_space(6.0);
}

/// A section label with a quiet one-line hint beside it.
fn label_hint(ui: &mut Ui, text: &str, hint: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.label(RichText::new(text).family(theme::bold_family()).size(13.5).color(theme::ink()));
        ui.label(theme::muted(hint).size(11.5));
    });
    ui.add_space(6.0);
}

/// A section label in a row of its own, `h` tall and sitting on its bottom: the formatting bar
/// floats over the right end of that row while its field is in use.
fn label_row(ui: &mut Ui, text: &str, h: f32) {
    ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), h), Layout::left_to_right(Align::Max), |ui| {
        ui.label(RichText::new(text).family(theme::bold_family()).size(13.5).color(theme::ink()));
    });
}

/// A small muted caption over a control of the metadata strip.
fn meta_label(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).family(theme::bold_family()).size(11.5).color(theme::muted_col()));
    ui.add_space(2.0);
}

// ---------- the in-place editor ----------

impl App {
    /// Writes the draft into the store (new or edit) and toasts. An unfinished one saves as a draft.
    pub(crate) fn commit_draft(&mut self, draft: Draft) {
        let existing = draft
            .qid
            .as_ref()
            .and_then(|qid| self.store.question(&draft.bank_id, qid))
            .cloned();
        let q = draft.build_question(existing.as_ref());
        let t = q.modified;
        let fixes = crate::qcheck::blockers(&q).len();
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
        if fixes > 0 {
            self.toast(draft_summary(fixes, true));
        } else {
            self.toast("Question saved");
        }
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
    // keys of one frame reach the fields one press at a time, in order, against the text as it is
    crate::wysiwyg::frame_keys(ctx);
    let first_frame = draft.baseline.is_none();
    if !draft.armed {
        // Opening happens on a click that is still live this frame, often right over the stem:
        // draw from the next frame, or the editor would take that click as a press in the text
        // and pop the floating toolbar over a blank.
        draft.armed = true;
        // a new editing session: no undo history of an earlier question reaches this one
        crate::wysiwyg::new_session(ctx);
        // Nor does any of its scratch state: the memory is keyed by bank and question id, which every
        // NEW question shares (`qid` is None), so the blank answer rows of a discarded Fill question
        // would otherwise show up as 'Blank 1' of the next new one.
        ctx.data_mut(|d| d.remove_temp::<Temp>(key));
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
    refresh_issues(ctx, key, draft);
    let mut action = QAction::None;
    // The discard prompt answers an Enter that follows the Esc in the same burst: the bar's button
    // only takes the focus a frame or two after it shows, and until then the stem would have the
    // Enter (a new line in the text about to be discarded).
    if draft.confirm == Some(Confirm::Discard)
        && ctx.data_mut(|d| d.get_temp_mut_or_default::<Temp>(key).confirm_focused) <= 2
        && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter))
    {
        action = QAction::Cancel;
    }
    // While an equation is edited in place it owns Esc, Ctrl+Enter and the outside click.
    let eq_open = crate::wysiwyg::math_active(ctx);
    let mut want_close = false;
    // Ctrl+Enter inside a formula commits the formula (the formula editor sees the Enter) and saves
    // on the next frame, so it is still one press.
    let save_after = ctx.data_mut(|d| std::mem::take(&mut d.get_temp_mut_or_default::<Temp>(key).save_after));
    if eq_open && ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Key { key: Key::Enter, pressed: true, modifiers, .. } if modifiers.command))) {
        ctx.data_mut(|d| d.get_temp_mut_or_default::<Temp>(key).save_after = true);
    }
    if !eq_open
        && draft.confirm.is_none()
        && (ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Enter))) || save_after)
        && draft.can_save()
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
        let mut scroll = egui::ScrollArea::vertical()
            .id_salt(key.with("body_scroll"))
            .min_scrolled_height(body_h)
            .max_height(body_h)
            .auto_shrink([false, false]);
        if first_frame {
            // a modal opens at the top, whatever an earlier session scrolled this id to
            scroll = scroll.vertical_scroll_offset(0.0);
        }
        let scrolled = scroll
            .show(ui, |ui| {
                let bw = (w - 16.0).max(240.0);
                ui.set_width(bw);
                meta_strip(ui, ctx, draft, key, bw, topic_pool);
                ui.add_space(6.0);
                body(ui, ctx, draft, key, bw, textures);
                // a control that just took focus (Tab, Shift+Tab, a click) is always brought into view
                // (text fields scroll themselves to the caret)
                if let Some(id) = ctx.memory(|m| m.focused())
                    && ctx.data_mut(|d| {
                        let t = d.get_temp_mut_or_default::<Temp>(key);
                        t.focus_now.is_none() && std::mem::replace(&mut t.scrolled_id, Some(id)) != Some(id)
                    })
                    && let Some(r) = ctx.read_response(id)
                    && r.rect.is_positive()
                    // (a control of the footer, like the confirm bar's button, is not in this body:
                    // scrolling to it would jump the content under the cursor)
                    && ctx.data(|d| d.get_temp::<f32>(UiId::new("qedit_footer_top"))).is_none_or(|top| r.rect.top() < top)
                {
                    ui.scroll_to_rect(r.rect.expand(8.0), None);
                }
            });
        #[cfg(test)]
        ctx.data_mut(|d| d.insert_temp(UiId::new("qedit_test_scroll_id"), scrolled.id));
        #[cfg(not(test))]
        let _ = scrolled;
        ui.add_space(8.0);
        let y = ui.cursor().top();
        ui.painter().hline(ui.max_rect().x_range(), y, Stroke::new(1.0, theme::rule()));
        ctx.data_mut(|d| d.insert_temp(UiId::new("qedit_footer_top"), y));
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
    // (a formula edited in place marks itself: the field's own ring, the caret in the formula)
    if crate::wysiwyg::math_active(ctx) {
        return;
    }
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

/// The quiet keyboard hint in the footer.
fn save_hint(k: QKind) -> &'static str {
    match k {
        QKind::Mc | QKind::Multipart => "Enter adds a row  ·  Alt+Enter marks correct  ·  Ctrl/Cmd+Enter saves",
        _ => "Ctrl/Cmd+Enter saves",
    }
}

/// The footer: a fixed-height validation line, then one fixed-height button row that turns into
/// a confirm bar of the same height, so nothing moves when it changes.
fn footer(ui: &mut Ui, draft: &mut Draft, action: &mut QAction, want_close: &mut bool) {
    let fixes = draft.validate();
    let can_save = draft.can_save();
    // The line says what saving will do. While a type switch waits for a yes, it says exactly what
    // the switch drops and keeps. Saving is never refused for gaps: they are warned about inline,
    // and the question saves as a draft that cannot go on an exam until they are fixed.
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::hover());
    let shown = ui.ctx().data_mut(|d| !d.get_temp_mut_or_default::<Temp>(temp_key(&draft.bank_id, draft.qid.as_ref())).issues.is_empty());
    if let Some(Confirm::Switch(k)) = &draft.confirm {
        let p = draft.switch_plan(*k);
        let mut msg = p.summary();
        if p.ask_tf {
            msg.push_str(" The answer key is not carried: pick True or False.");
        }
        ui.painter().with_clip_rect(rect).text(rect.left_center(), egui::Align2::LEFT_CENTER, msg, kit::ui_font(12.0), theme::ink());
    } else if !can_save {
        ui.painter().with_clip_rect(rect).text(rect.left_center(), egui::Align2::LEFT_CENTER, "Nothing to save yet: write the question or an answer.", kit::ui_font(12.0), theme::muted_col());
    } else if shown && !fixes.is_empty() {
        let p = ui.painter().with_clip_rect(rect);
        p.text(rect.left_center(), egui::Align2::LEFT_CENTER, draft_summary(fixes.len(), false), kit::ui_font(12.0), warn_col(ui));
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
                    let save = ui.add_enabled_ui(can_save, |ui| kit::primary_button(ui, "Save")).inner;
                    if save.clicked() {
                        *action = QAction::Done;
                    }
                    if kit::ghost_button(ui, "Cancel").clicked() {
                        *want_close = true;
                    }
                    ui.label(theme::muted(save_hint(draft.qkind())).size(11.5));
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

/// Type, Difficulty, Points and Topics as one compact strip: a first row of three small labelled
/// controls and a second of Topics. Each cell is a caption over its control.
fn meta_strip(ui: &mut Ui, ctx: &egui::Context, draft: &mut Draft, key: UiId, w: f32, topic_pool: &[String]) {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 18.0;
        ui.vertical(|ui| {
            meta_label(ui, "Type");
            let cur = draft.qkind();
            let mut pick: Option<QKind> = None;
            egui::ComboBox::from_id_salt(key.with("type_combo"))
                .width(168.0)
                .selected_text(RichText::new(format!("{}  {}", type_tag(cur), cur.label())).size(13.0))
                .show_ui(ui, |ui| {
                    for (k, tag) in KINDS {
                        if ui.selectable_label(cur == k, format!("{tag}  {}", k.label())).clicked() && cur != k {
                            pick = Some(k);
                        }
                    }
                });
            if let Some(k) = pick {
                if draft.switch_plan(k).needs_confirm() {
                    draft.confirm = Some(Confirm::Switch(k));
                } else {
                    draft.switch_to(k, None);
                    ctx.data_mut(|d| d.remove_temp::<Temp>(key));
                    EqCtx { ctx: ctx.clone(), key }.focus(&EqField::Stem, draft.stem.chars().count());
                }
            }
        });
        ui.vertical(|ui| {
            meta_label(ui, "Difficulty");
            Frame::new().stroke(Stroke::new(1.0, theme::border())).corner_radius(CornerRadius::same(8)).inner_margin(Margin::same(2)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for (d, name) in [(Difficulty::Low, "Easy"), (Difficulty::Medium, "Medium"), (Difficulty::High, "Hard")] {
                        let on = draft.difficulty == Some(d);
                        let b = egui::Button::new(RichText::new(name).size(12.5).color(if on { theme::accent_col() } else { theme::text_col() }))
                            .fill(if on { theme::accent_faded() } else { Color32::TRANSPARENT })
                            .stroke(Stroke::NONE)
                            .corner_radius(CornerRadius::same(6))
                            .min_size(Vec2::new(54.0, 22.0));
                        if ui.add(b).on_hover_text(if on { "Click again to clear" } else { "Set difficulty" }).clicked() {
                            draft.difficulty = if on { None } else { Some(d) };
                        }
                    }
                });
            });
        });
        ui.vertical(|ui| {
            meta_label(ui, "Points");
            let mut pt = draft.points_text();
            if field_line(ui, &mut pt, 76.0, "none").changed() {
                draft.set_points_text(&pt);
            }
        });
    });
    ui.add_space(6.0);
    meta_label(ui, "Topics");
    topics_value(ui, ctx, draft, key, w, topic_pool);
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
    ui.add_space(20.0);
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
    use crate::qcheck::Where;
    // Stem with the equation hook.
    let eq = EqCtx { ctx: ctx.clone(), key };
    label_row(ui, "Question", 36.0);
    let is_blank = matches!(draft.kind, Kind::Blank { .. });
    let mut mirror = is_blank.then(|| blank_mirror(ctx, key, draft));
    let blank_hint = format!("Write the sentence, then {BLANK_HOW}...");
    let hint = if is_blank { blank_hint.as_str() } else { "Write the question..." };
    let inner = (w - 26.0).max(60.0);
    let sid = field_editor_id(key, &EqField::Stem);
    // Tab always leaves the stem for the first answer's text field and Shift+Tab goes back one
    // control; Enter keeps adding lines. (Inside a table Tab walks the cells. A list item is
    // nested with Ctrl+] and un-nested with Ctrl+[.)
    let first = first_field(&draft.kind, mirror.as_deref());
    let tab_to_answers = first.is_some();
    if tab_to_answers && eq.last_focus() == Some(EqField::Stem) {
        let (keep_fwd, keep_back) = (crate::wysiwyg::tab_is_local(ctx, sid, false), crate::wysiwyg::tab_is_local(ctx, sid, true));
        let (tab, back) = ctx.input_mut(|i| {
            let find = |i: &egui::InputState, m: Modifiers| i.events.iter().position(|e| matches!(e, egui::Event::Key { key: Key::Tab, pressed: true, modifiers, .. } if modifiers.matches_exact(m)));
            let mut t = find(i, Modifiers::NONE);
            if let Some(p) = t.filter(|_| !keep_fwd) {
                i.events.remove(p);
            } else {
                t = None;
            }
            let mut b = find(i, Modifiers::SHIFT);
            if let Some(p) = b.filter(|_| !keep_back) {
                i.events.remove(p);
            } else {
                b = None;
            }
            (t.is_some(), b.is_some())
        });
        if tab && let Some((f, n)) = &first {
            eq.focus(f, *n);
        } else if back {
            ctx.memory_mut(|m| m.move_focus(egui::FocusDirection::Previous));
        }
    }
    let fr = field_frame(Margin::symmetric(12, 8)).show(ui, |ui| {
        ui.set_width(inner);
        // room to write: three lines, not a slit
        ui.set_min_height(56.0);
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
    if has_picture(&draft.stem) {
        stem_preview(ui, &eq, &draft.stem, w);
    }
    // (a reserved line: a warning appears here without moving anything)
    eq.warn_line(ui, w, |a| matches!(a, Where::Stem | Where::Blanks));
    ui.add_space(4.0);

    // Per-kind answers.
    let stem_len = chars(&draft.stem);
    match &mut draft.kind {
        Kind::Mc { answers } => {
            label(ui, "Answers");
            mc_editor(ui, &eq, answers, usize::MAX, w, &|i| EqField::Mc(i), (EqField::Stem, stem_len));
            eq.warn_line(ui, w, |a| matches!(a, Where::Answers));
        }
        Kind::Tf { correct } => {
            label(ui, "Correct answer");
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
            eq.warn_line(ui, w, |a| matches!(a, Where::Answers));
        }
        Kind::Matching { pairs, distractors } => {
            matching_editor(ui, &eq, pairs, distractors, w, stem_len);
            eq.warn_line(ui, w, |a| matches!(a, Where::Pairs));
        }
        Kind::Blank { .. } => blank_editor(ui, &eq, draft, w),
        Kind::Short { sample } => {
            label(ui, "Model answer");
            eq_multi(ui, &eq, EqField::Short, sample, w, 3, "What a good answer looks like...");
            eq.warn_line(ui, w, |a| matches!(a, Where::Answers));
        }
        Kind::Multipart { parts } => {
            label(ui, "Parts");
            multipart_editor(ui, &eq, parts, w, draft.keyless, &mut draft.tf_keyed);
            eq.warn_line(ui, w, |a| matches!(a, Where::Parts));
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
    // the field takes the row's full width; its controls sit inside it, over the end of the text
    let field_w = (w - 22.0 - 6.0 + 8.0).max(120.0);
    for i in 0..n {
        let locked = answers[i].locked;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let start = ui.cursor().min;
            if kit::check_circle(ui, answers[i].correct).on_hover_text(if answers[i].correct { "Correct (Alt+Enter to change)" } else { "Mark correct (Alt+Enter)" }).clicked() {
                answers[i].correct = !answers[i].correct;
            }
            let out = eq_line(ui, eq, fld(i), &mut answers[i].text, field_w, "Answer");
            if answers[i].text.trim().is_empty() && !out.focused {
                eq.warn_in(ui, out.rect, crate::qcheck::Where::Answer(i));
            }
            // controls on hover or focus, inside the field and left of its sigma: nothing moves
            let row = Rect::from_min_max(Pos2::new(start.x + 28.0, out.rect.top() - 6.0), Pos2::new(start.x + 28.0 + field_w - 8.0, out.rect.bottom() + 6.0));
            let show = out.focused || ui.rect_contains_pointer(row);
            let right = row.right() - 36.0;
            if show {
                let bw = if n > 2 { 4.0 * 24.0 + 3.0 * 2.0 } else { 3.0 * 24.0 + 2.0 * 2.0 };
                let bar = Rect::from_min_max(Pos2::new(right - bw - 4.0, row.center().y - 14.0), Pos2::new(right + 2.0, row.center().y + 14.0));
                ui.painter().rect_filled(bar, CornerRadius::same(8), theme::field());
                ui.scope_builder(egui::UiBuilder::new().max_rect(bar.shrink2(Vec2::new(4.0, 2.0))).layout(Layout::left_to_right(Align::Center)), |ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    if icon_btn(ui, Ico::Up, false).on_hover_text("Move up (Alt+Up)").clicked() && i > 0 {
                        move_up = Some(i);
                    }
                    if icon_btn(ui, Ico::Down, false).on_hover_text("Move down (Alt+Down)").clicked() && i + 1 < n {
                        move_up = Some(i + 1);
                    }
                    if icon_btn(ui, Ico::Lock, locked).on_hover_text("Lock answer position (never shuffled)").clicked() {
                        answers[i].locked = !locked;
                    }
                    if n > 2 && icon_btn(ui, Ico::Close, false).on_hover_text("Remove (Backspace in an empty row)").clicked() {
                        to_remove = Some(i);
                    }
                });
            } else if locked {
                ui.painter().text(Pos2::new(right - 8.0, row.center().y), egui::Align2::CENTER_CENTER, egui_phosphor::regular::LOCK_SIMPLE, theme::icon_font(14.0), theme::accent_col().gamma_multiply(0.6));
            }
        });
        ui.add_space(4.0);
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
    label_hint(ui, "Pairs", "left item, right item");
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
            // (the empty side of a half-filled pair says so)
            let (l_empty, r_empty) = (pairs[i].left.trim().is_empty(), pairs[i].right.trim().is_empty());
            if l_empty != r_empty {
                let (r, focused) = if l_empty { (a.rect, a.focused) } else { (b.rect, b.focused) };
                if !focused {
                    eq.warn_in(ui, r, crate::qcheck::Where::Pair(i));
                }
            }
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
    label_hint(ui, "Distractors", "extra right-side items");
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
        label_hint(ui, &format!("Blank {}", i + 1), "accepted answers, separate alternatives with OR");
        let out = eq_line(ui, eq, EqField::Blank(i), &mut texts[i], w, "e.g. 1 OR one");
        if texts[i].trim().is_empty() && !out.focused {
            eq.warn_in(ui, out.rect, crate::qcheck::Where::Blank(i));
        }
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
            // (each part's own reserved warning line, at the part)
            eq.warn_line(ui, w, |a| a == crate::qcheck::Where::Part(i));
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
    use egui::Event;

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

    /// Frames of the real modal, headless: Tab from the stem lands in the first answer's TEXT
    /// field whatever the stem holds, and what is typed next lands there (never lost).
    #[test]
    fn tab_from_any_stem_reaches_the_first_answer_and_the_next_text_lands_there() {
        use egui::{Event, Pos2, RawInput, Rect};
        for stem in ["plain words", "**bold** text", "- a\n- b", "1. a\n2. b", "> quoted", "# Head\nbody", "x $x^2$ y", "- a\n\ntail"] {
            let ctx = egui::Context::default();
            crate::theme::setup(&ctx);
            let mut d = Draft::new("b".to_string());
            d.stem = stem.to_string();
            let mut tex = HashMap::new();
            let mut t = 0.0;
            let mut frame = |d: &mut Draft, events: Vec<Event>| {
                let mut input = RawInput::default();
                input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1280.0, 900.0)));
                input.time = Some(t);
                input.events = events;
                t += 0.05;
                let o = ctx.run_ui(input, |ui| {
                    let _ = ui;
                    draw_modal(&ctx, d, &[], &mut tex);
                });
                o.drop_without_applying_deltas();
            };
            for _ in 0..8 {
                frame(&mut d, vec![]);
            }
            let tab = Event::Key { key: Key::Tab, physical_key: Some(Key::Tab), pressed: true, repeat: false, modifiers: Modifiers::NONE };
            frame(&mut d, vec![tab]);
            for _ in 0..4 {
                frame(&mut d, vec![]);
            }
            frame(&mut d, vec![Event::Text("zz".into())]);
            for _ in 0..4 {
                frame(&mut d, vec![]);
            }
            let Kind::Mc { answers } = &d.kind else { panic!() };
            assert_eq!(answers[0].text, "zz", "stem {stem:?}");
            assert_eq!(d.stem, stem, "the stem is untouched by the Tab and the text after it");
        }
    }

    /// The real modal, headless: Make Blank then Undo leaves no answer row of the blank behind.
    #[test]
    fn undo_of_make_blank_in_the_modal_removes_the_answer() {
        use egui::{Event, Pos2, RawInput, Rect};
        let ctx = egui::Context::default();
        crate::theme::setup(&ctx);
        let mut d = Draft::new("b".to_string());
        d.kind = default_kind(QKind::Blank);
        d.stem = "the quick fox".to_string();
        let mut tex = HashMap::new();
        let mut t = 0.0;
        let mut frame = |d: &mut Draft, events: Vec<Event>| {
            let mut input = RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1280.0, 900.0)));
            input.time = Some(t);
            input.events = events;
            t += 0.05;
            let o = ctx.run_ui(input, |_ui| {
                draw_modal(&ctx, d, &[], &mut tex);
            });
            o.drop_without_applying_deltas();
        };
        let key = |k: Key, m: Modifiers| Event::Key { key: k, physical_key: Some(k), pressed: true, repeat: false, modifiers: m };
        for _ in 0..8 {
            frame(&mut d, vec![]);
        }
        frame(&mut d, vec![key(Key::End, Modifiers::NONE)]);
        for _ in 0..3 {
            frame(&mut d, vec![key(Key::ArrowLeft, Modifiers::SHIFT)]);
            frame(&mut d, vec![]);
        }
        frame(&mut d, vec![key(Key::K, Modifiers::COMMAND | Modifiers::SHIFT)]);
        for _ in 0..4 {
            frame(&mut d, vec![]);
        }
        assert_eq!(d.stem, "the quick ___");
        let Kind::Blank { accept } = &d.kind else { panic!() };
        assert_eq!(accept, &vec![vec!["fox".to_string()]]);
        frame(&mut d, vec![key(Key::Z, Modifiers::COMMAND)]);
        for _ in 0..4 {
            frame(&mut d, vec![]);
        }
        assert_eq!(d.stem, "the quick fox");
        let Kind::Blank { accept } = &d.kind else { panic!() };
        assert!(accept.iter().all(|a| a.iter().all(|x| x.is_empty())), "no answer left over: {accept:?}");
    }

    /// Keys that arrive together in one frame reach the fields in order: text, Tab, text puts the
    /// first text in the stem and the second in the first answer; text, Enter, text in an answer
    /// row ends that row and starts the next with the second text.
    #[test]
    fn keys_of_one_frame_reach_the_modals_fields_in_order() {
        use egui::{Event, Pos2, RawInput, Rect};
        let ctx = egui::Context::default();
        crate::theme::setup(&ctx);
        let mut d = Draft::new("b".to_string());
        let mut tex = HashMap::new();
        let mut t = 0.0;
        let mut frame = |d: &mut Draft, events: Vec<Event>| {
            let mut input = RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1280.0, 900.0)));
            input.time = Some(t);
            input.events = events;
            t += 0.05;
            let o = ctx.run_ui(input, |_ui| {
                draw_modal(&ctx, d, &[], &mut tex);
            });
            o.drop_without_applying_deltas();
        };
        let key = |k: Key| Event::Key { key: k, physical_key: Some(k), pressed: true, repeat: false, modifiers: Modifiers::NONE };
        for _ in 0..8 {
            frame(&mut d, vec![]);
        }
        frame(&mut d, vec![Event::Text("ab".into()), key(Key::Tab), Event::Text("cd".into()), key(Key::Enter), Event::Text("ef".into())]);
        for _ in 0..14 {
            frame(&mut d, vec![]);
        }
        assert_eq!(d.stem, "ab");
        let Kind::Mc { answers } = &d.kind else { panic!() };
        assert_eq!((answers[0].text.as_str(), answers[1].text.as_str()), ("cd", "ef"), "{answers:?}");
    }

    /// A headless modal: frames of the real `draw_modal` with scripted input.
    struct Modal {
        ctx: egui::Context,
        tex: HashMap<String, egui::TextureHandle>,
        t: f64,
        pub w: f32,
        pub h: f32,
    }

    impl Modal {
        fn new() -> Modal {
            let ctx = egui::Context::default();
            crate::theme::setup(&ctx);
            Modal { ctx, tex: HashMap::new(), t: 0.0, w: 1280.0, h: 900.0 }
        }
        fn frame(&mut self, d: &mut Draft, events: Vec<egui::Event>) -> QAction {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(self.w, self.h)));
            input.time = Some(self.t);
            input.events = events;
            self.t += 0.05;
            let mut act = QAction::None;
            let o = self.ctx.run_ui(input, |_ui| {
                let a = draw_modal(&self.ctx, d, &[], &mut self.tex);
                if a != QAction::None {
                    act = a;
                }
            });
            o.drop_without_applying_deltas();
            act
        }
        fn settle(&mut self, d: &mut Draft, n: usize) -> QAction {
            let mut act = QAction::None;
            for _ in 0..n {
                let a = self.frame(d, vec![]);
                if a != QAction::None {
                    act = a;
                }
            }
            act
        }
    }

    fn press(k: Key) -> egui::Event {
        egui::Event::Key { key: k, physical_key: Some(k), pressed: true, repeat: false, modifiers: Modifiers::NONE }
    }

    /// A burst of Backspaces in ONE frame in an answer row deletes the text and then the empty
    /// row too, exactly as the same keys spread over several frames do.
    #[test]
    fn a_backspace_burst_in_one_frame_removes_the_emptied_answer_row() {
        for (typed, presses) in [("b", 2usize), ("abcd", 5), ("ab", 3)] {
            let mut m = Modal::new();
            let mut d = Draft::new("b".to_string());
            m.settle(&mut d, 8);
            m.frame(&mut d, vec![Event::Text("q".into()), press(Key::Tab)]);
            m.settle(&mut d, 6);
            m.frame(&mut d, vec![Event::Text("a".into()), press(Key::Enter), Event::Text(typed.into())]);
            m.settle(&mut d, 10);
            let Kind::Mc { answers } = &d.kind else { panic!() };
            assert_eq!(answers[1].text, typed);
            let before = answers.len();
            m.frame(&mut d, (0..presses).map(|_| press(Key::Backspace)).collect());
            m.settle(&mut d, 30);
            let Kind::Mc { answers } = &d.kind else { panic!() };
            assert_eq!(answers.len(), before - 1, "{typed:?} x{presses}: the emptied row goes: {answers:?}");
            assert_eq!(answers[1].text, "", "(the next row is the old third, empty)");
        }
    }

    /// A draft opened after another one never inherits its scratch state: a new Fill question
    /// after a discarded one shows empty blank rows (the temp memory is keyed by bank and question
    /// id, which every new question shares).
    #[test]
    fn a_new_draft_never_inherits_the_previous_drafts_temp_state() {
        let mut m = Modal::new();
        let mut d = Draft::new_of("b".to_string(), QKind::Blank);
        d.stem = "the quick three".to_string();
        m.settle(&mut d, 8);
        let k = Modifiers::SHIFT;
        let shift = |key: Key| Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers: k };
        m.frame(&mut d, vec![press(Key::End)]);
        for _ in 0..5 {
            m.frame(&mut d, vec![shift(Key::ArrowLeft)]);
            m.frame(&mut d, vec![]);
        }
        m.frame(&mut d, vec![Event::Key { key: Key::K, physical_key: Some(Key::K), pressed: true, repeat: false, modifiers: Modifiers::COMMAND | Modifiers::SHIFT }]);
        m.settle(&mut d, 6);
        assert_eq!(d.stem, "the quick ___");
        let key = temp_key("b", None);
        let held = |m: &Modal| m.ctx.data_mut(|dd| dd.get_temp_mut_or_default::<Temp>(key).blank_texts.clone());
        assert_eq!(held(&m).unwrap_or_default(), vec!["three".to_string()], "the first draft kept its blank text");
        // discarded; a new Fill question opens in the same context
        let mut d2 = Draft::new_of("b".to_string(), QKind::Blank);
        m.settle(&mut d2, 8);
        let t2 = held(&m).unwrap_or_default();
        assert!(t2.iter().all(|r| r.is_empty()), "no text of the earlier draft: {t2:?}");
        let Kind::Blank { accept } = &d2.kind else { panic!() };
        assert!(accept.iter().flatten().all(|a| a.is_empty()), "{accept:?}");
        assert!(d2.stem.is_empty());
    }

    /// Escape then Enter in one burst: the discard prompt takes the Enter and discards.
    #[test]
    fn esc_then_enter_in_one_burst_discards() {
        let mut m = Modal::new();
        let mut d = Draft::new("b".to_string());
        m.settle(&mut d, 8);
        m.frame(&mut d, vec![Event::Text("typed".into())]);
        m.settle(&mut d, 6);
        assert!(d.is_dirty());
        let mut act = m.frame(&mut d, vec![press(Key::Escape), press(Key::Enter)]);
        for _ in 0..10 {
            if act != QAction::None {
                break;
            }
            act = m.frame(&mut d, vec![]);
        }
        assert_eq!(act, QAction::Cancel);
        assert_eq!(d.stem, "typed", "the Enter did not become a new line");
    }

    /// Esc alone only asks; Esc again keeps editing.
    #[test]
    fn esc_alone_asks_and_does_not_discard() {
        let mut m = Modal::new();
        let mut d = Draft::new("b".to_string());
        m.settle(&mut d, 8);
        m.frame(&mut d, vec![Event::Text("typed".into())]);
        m.settle(&mut d, 6);
        assert_eq!(m.frame(&mut d, vec![press(Key::Escape)]), QAction::None);
        assert_eq!(m.settle(&mut d, 10), QAction::None);
        assert_eq!(d.confirm, Some(Confirm::Discard));
    }

    /// The modal opens at the top even when an earlier session scrolled the same body.
    #[test]
    fn the_modal_opens_scrolled_to_the_top() {
        use egui::containers::scroll_area::State;
        let mut m = Modal::new();
        let mut d = Draft::new("b".to_string());
        m.settle(&mut d, 8);
        let sid: UiId = m.ctx.data(|dd| dd.get_temp(UiId::new("qedit_test_scroll_id"))).unwrap();
        m.ctx.data_mut(|dd| {
            let mut st = State::default();
            st.offset = egui::vec2(0.0, 21.0);
            dd.insert_persisted(sid, st);
        });
        // a new session of the same question id
        let mut d2 = Draft::new("b".to_string());
        m.settle(&mut d2, 8);
        let off = State::load(&m.ctx, sid).map_or(0.0, |s| s.offset.y);
        assert!(off < 1.0, "scrolled {off} px on open");
    }

    /// A confirm bar (type switch, discard, delete) lives in the fixed footer: showing it never
    /// scrolls the body, so nothing jumps under the cursor.
    #[test]
    fn a_confirm_bar_never_scrolls_the_modal_body() {
        use egui::containers::scroll_area::State;
        for c in [Confirm::Switch(QKind::Tf), Confirm::Discard, Confirm::Delete] {
            let mut m = Modal::new();
            let mut d = Draft::new("b".to_string());
            d.stem = "Which is prime".to_string();
            d.kind = Kind::Mc { answers: (0..9).map(|i| Answer { text: format!("a{i}"), correct: i == 1, locked: false }).collect() };
            m.settle(&mut d, 8);
            let sid: UiId = m.ctx.data(|dd| dd.get_temp(UiId::new("qedit_test_scroll_id"))).unwrap();
            d.confirm = Some(c.clone());
            m.settle(&mut d, 12);
            let off = State::load(&m.ctx, sid).map_or(0.0, |s| s.offset.y);
            assert!(off < 1.0, "{c:?}: the body scrolled {off} px when the confirm bar showed");
        }
    }

    /// Tab in the LAST cell of a table in the stem leaves the table for the first answer (no row is
    /// added); Tab in an earlier cell walks to the next cell.
    #[test]
    fn tab_in_the_last_table_cell_leaves_the_table() {
        let table = "| a | b |\n| --- | --- |\n| c | d |";
        let mut m = Modal::new();
        let mut d = Draft::new("b".to_string());
        d.stem = table.to_string();
        m.settle(&mut d, 10); // focus lands at the end of the stem: the last cell
        m.frame(&mut d, vec![press(Key::Tab)]);
        m.settle(&mut d, 6);
        m.frame(&mut d, vec![Event::Text("zz".into())]);
        m.settle(&mut d, 6);
        assert_eq!(d.stem, table, "no row was added and nothing was typed into the table");
        let Kind::Mc { answers } = &d.kind else { panic!() };
        assert_eq!(answers[0].text, "zz");
    }

    #[test]
    fn tab_in_an_earlier_table_cell_walks_to_the_next_cell() {
        let table = "| a | b |\n| --- | --- |\n| c | d |";
        let mut m = Modal::new();
        let mut d = Draft::new("b".to_string());
        d.stem = table.to_string();
        m.settle(&mut d, 10);
        m.frame(&mut d, vec![Event::Key { key: Key::Home, physical_key: Some(Key::Home), pressed: true, repeat: false, modifiers: Modifiers::COMMAND }]);
        m.settle(&mut d, 6);
        m.frame(&mut d, vec![press(Key::Tab)]);
        m.settle(&mut d, 6);
        m.frame(&mut d, vec![Event::Text("X".into())]);
        m.settle(&mut d, 6);
        assert_eq!(d.stem, "| a | X |\n| --- | --- |\n| c | d |", "the next cell (its text selected) took the X");
        let Kind::Mc { answers } = &d.kind else { panic!() };
        assert_eq!(answers[0].text, "");
    }

    fn pair(l: &str, r: &str) -> Pair {
        Pair { left: l.to_string(), right: r.to_string() }
    }

    fn msgs_of(d: &Draft) -> Vec<String> {
        d.issues().into_iter().map(|i| i.msg).collect()
    }

    /// Every kind: what is missing is named, and saving it stores exactly what was entered.
    #[test]
    fn an_incomplete_draft_of_every_kind_names_its_gaps_and_saves_as_entered() {
        // multiple choice, no correct answer
        let mut d = Draft::new("b".to_string());
        d.stem = "Pick".to_string();
        d.kind = mc(&[("x", false), ("y", false), ("", false), ("", false)]);
        assert_eq!(msgs_of(&d), ["Mark one answer correct"]);
        let q = d.build_question(None);
        let Kind::Mc { answers } = &q.kind else { panic!() };
        assert_eq!(answers.iter().map(|a| (a.text.as_str(), a.correct)).collect::<Vec<_>>(), [("x", false), ("y", false)], "no key was invented");
        assert!(!crate::model::is_keyless(&q), "a draft is not marked as an import");
        assert!(crate::qcheck::key_missing(&q));
        // true/false: nothing to fix without a stem
        let mut d = Draft::new_of("b".to_string(), QKind::Tf);
        d.stem = "Sky is blue".to_string();
        assert!(d.issues().is_empty());
        d.stem.clear();
        assert_eq!(msgs_of(&d), ["Add a question stem"]);
        // matching: a half-filled pair is kept as typed
        let mut d = Draft::new_of("b".to_string(), QKind::Matching);
        d.stem = "Match".to_string();
        d.kind = Kind::Matching { pairs: vec![pair("a", "1"), pair("b", "")], distractors: vec![String::new()] };
        let m = msgs_of(&d);
        assert_eq!(m, ["Pair 2 has no match on the right", "Add at least two matching pairs (both sides filled)"]);
        let Kind::Matching { pairs, .. } = d.build_question(None).kind else { panic!() };
        assert_eq!((pairs.len(), pairs[1].left.as_str(), pairs[1].right.as_str()), (2, "b", ""), "the typed left item is not dropped");
        // fill in the blank: a blank without an answer, and blanks that do not match
        let mut d = Draft::new_of("b".to_string(), QKind::Blank);
        d.stem = "Fill ___ and ___".to_string();
        d.kind = Kind::Blank { accept: vec![vec!["one".to_string()], vec![]] };
        assert_eq!(msgs_of(&d), ["Blank 2 has no accepted answer"]);
        d.kind = Kind::Blank { accept: vec![vec!["one".to_string()]] };
        assert!(msgs_of(&d).contains(&"Blanks and answers do not match".to_string()), "{:?}", msgs_of(&d));
        let Kind::Blank { accept } = d.build_question(None).kind else { panic!() };
        assert_eq!(accept, vec![vec!["one".to_string()]], "saved as entered");
        // short answer: only the stem can block; no model answer is a note, and it saves
        let mut d = Draft::new_of("b".to_string(), QKind::Short);
        d.stem = "Explain".to_string();
        assert_eq!(msgs_of(&d), ["No model answer: the key will be blank"]);
        assert!(d.validate().is_empty());
        // multipart
        let mut d = Draft::new_of("b".to_string(), QKind::Multipart);
        d.stem = "Parts".to_string();
        if let Kind::Multipart { parts } = &mut d.kind {
            parts[1].stem = "second".to_string();
            parts[1].kind = PartKind::Mc { answers: vec![Answer { text: "x".into(), correct: false, locked: false }, Answer { text: "y".into(), correct: false, locked: false }] };
        }
        assert_eq!(msgs_of(&d), ["Part 1 needs its own stem", "Part 2 needs a correct answer"]);
        assert_eq!(d.build_question(None).stem, "Parts");
    }

    /// Save is never refused for gaps, only for a draft with nothing in it.
    #[test]
    fn save_is_refused_only_for_a_draft_with_nothing_in_it() {
        let mut d = Draft::new("b".to_string());
        assert!(!d.can_save() && d.is_empty_draft());
        d.stem = "   ".to_string();
        assert!(!d.can_save(), "spaces are nothing");
        d.stem = "A stem and nothing else".to_string();
        assert!(d.can_save());
        d.stem.clear();
        if let Kind::Mc { answers } = &mut d.kind {
            answers[2].text = "just an answer".to_string();
        }
        assert!(d.can_save(), "an answer alone is something");
        for k in crate::bankops::KINDS {
            assert!(!Draft::new_of("b".to_string(), k).can_save(), "{k:?} untouched");
        }
    }

    /// In the real modal, Ctrl+Enter saves an incomplete draft (it used to wait for a clean one) and
    /// does nothing for an empty one.
    #[test]
    fn ctrl_enter_saves_an_incomplete_draft_but_not_an_empty_one() {
        let ctrl_enter = Event::Key { key: Key::Enter, physical_key: Some(Key::Enter), pressed: true, repeat: false, modifiers: Modifiers::COMMAND };
        let mut m = Modal::new();
        let mut empty = Draft::new("b".to_string());
        m.settle(&mut empty, 8);
        let mut act = m.frame(&mut empty, vec![ctrl_enter.clone()]);
        act = if act == QAction::None { m.settle(&mut empty, 6) } else { act };
        assert_eq!(act, QAction::None);
        let mut m = Modal::new();
        let mut d = Draft::new("b".to_string());
        m.settle(&mut d, 8);
        m.frame(&mut d, vec![Event::Text("only a stem".into())]);
        m.settle(&mut d, 6);
        assert!(!d.validate().is_empty());
        let mut act = m.frame(&mut d, vec![ctrl_enter]);
        if act == QAction::None {
            act = m.settle(&mut d, 6);
        }
        assert_eq!(act, QAction::Done);
    }

    /// The inline warnings wait until the teacher has been in the field and left it (or the question
    /// is an existing one), and an empty draft shows none.
    #[test]
    fn warnings_wait_for_the_field_to_be_left() {
        let mut m = Modal::new();
        let mut d = Draft::new("b".to_string());
        m.settle(&mut d, 8);
        let shown = |m: &Modal, d: &Draft| m.ctx.data_mut(|dd| dd.get_temp_mut_or_default::<Temp>(temp_key(&d.bank_id, d.qid.as_ref())).issues.len());
        assert_eq!(shown(&m, &d), 0, "an empty draft shows nothing");
        m.frame(&mut d, vec![Event::Text("stem".into())]);
        m.settle(&mut d, 6);
        assert_eq!(shown(&m, &d), 0, "still typing in the stem");
        m.frame(&mut d, vec![press(Key::Tab)]);
        m.settle(&mut d, 6);
        m.frame(&mut d, vec![Event::Text("x".into()), press(Key::Enter), Event::Text("y".into())]);
        m.settle(&mut d, 12);
        assert_eq!(shown(&m, &d), 0, "typing in the answers: nothing nags yet");
        // leave the answers for the stem
        for _ in 0..2 {
            m.frame(&mut d, vec![Event::Key { key: Key::Tab, physical_key: Some(Key::Tab), pressed: true, repeat: false, modifiers: Modifiers::SHIFT }]);
            m.settle(&mut d, 4);
        }
        let first = d.issues();
        assert!(first.iter().any(|i| i.msg == "Mark one answer correct"));
        let now = m.ctx.data_mut(|dd| dd.get_temp_mut_or_default::<Temp>(temp_key("b", None)).issues.clone());
        assert!(now.iter().any(|i| i.msg == "Mark one answer correct"), "shown once the answers were left: {now:?}");
        // an existing question shows its gaps at once
        let mut q = sample_question();
        q.kind = mc(&[("x", false), ("y", false)]);
        let mut e = Draft::from_question("b".into(), &q);
        let mut m = Modal::new();
        m.settle(&mut e, 10);
        let now = m.ctx.data_mut(|dd| dd.get_temp_mut_or_default::<Temp>(temp_key("b", Some(&"q1".to_string()))).issues.clone());
        assert!(now.iter().any(|i| i.msg == "Mark one answer correct"), "{now:?}");
    }

    /// Gaps in fields that are on screen the whole time show at once, visited or not: a blank never
    /// clicked into, a Short with no model answer, and each incomplete part at its own part.
    #[test]
    fn unvisited_blanks_short_and_parts_warn_at_their_own_field() {
        let shown = |m: &Modal, d: &Draft| m.ctx.data_mut(|dd| dd.get_temp_mut_or_default::<Temp>(temp_key(&d.bank_id, d.qid.as_ref())).issues.clone());
        // Fill with two blanks, the second never visited (Make Blank filled the first)
        let mut d = Draft::new_of("b".to_string(), QKind::Blank);
        d.stem = "One ___ and two ___".to_string();
        d.kind = Kind::Blank { accept: vec![vec!["one".to_string()], vec![]] };
        let mut m = Modal::new();
        m.settle(&mut d, 8);
        m.frame(&mut d, vec![Event::Text("x".into())]); // typing in the stem, not in a blank
        m.settle(&mut d, 6);
        let v = shown(&m, &d);
        assert!(v.iter().any(|i| i.at == crate::qcheck::Where::Blank(1) && i.msg == "Blank 2 has no accepted answer"), "{v:?}");
        // Short with a stem and no model answer
        let mut d = Draft::new_of("b".to_string(), QKind::Short);
        d.stem = "Explain".to_string();
        let mut m = Modal::new();
        m.settle(&mut d, 8);
        m.frame(&mut d, vec![Event::Text("x".into())]);
        m.settle(&mut d, 6);
        let v = shown(&m, &d);
        assert!(v.iter().any(|i| i.msg == "No model answer: the key will be blank"), "{v:?}");
        // Multipart with two incomplete parts: each warning is addressed to its own part
        let mut d = Draft::new_of("b".to_string(), QKind::Multipart);
        d.stem = "Parts".to_string();
        if let Kind::Multipart { parts } = &mut d.kind {
            parts[0].stem = "first".into();
            parts[0].kind = PartKind::Mc { answers: vec![] };
            parts[1].stem = "second".into();
            parts[1].kind = PartKind::Mc { answers: vec![Answer { text: "x".into(), correct: false, locked: false }] };
        }
        // (a new draft, no part visited)
        let mut m = Modal::new();
        m.settle(&mut d, 10);
        m.frame(&mut d, vec![Event::Text("x".into())]);
        m.settle(&mut d, 6);
        let v = shown(&m, &d);
        for i in 0..2 {
            assert!(v.iter().any(|x| x.at == crate::qcheck::Where::Part(i) && x.severity == crate::qcheck::Severity::Fix), "part {i}: {v:?}");
        }
    }

    /// An imported keyless question stays keyless through an edit that does not key it, and is
    /// flagged; a keyless True/False nobody has picked is flagged in the editor.
    #[test]
    fn a_keyless_import_stays_keyless_and_flagged() {
        let mut q = sample_question();
        q.kind = mc(&[("x", false), ("y", false)]);
        q.explanation = Some(crate::model::KEYLESS_SENTINEL.to_string());
        let mut d = Draft::from_question("b".into(), &q);
        assert!(d.keyless);
        assert_eq!(msgs_of(&d), ["Mark one answer correct"]);
        d.stem = "Edited stem".to_string();
        let out = d.build_question(Some(&q));
        assert!(crate::model::is_keyless(&out), "still marked as imported without a key");
        assert!(crate::qcheck::key_missing(&out));
        assert!(matches!(&out.kind, Kind::Mc { answers } if answers.iter().all(|a| !a.correct)), "no key was made up");
        assert_eq!(crate::qcheck::blockers(&out).len(), 1);
        // keyed in the editor: the marker goes and the question is complete
        if let Kind::Mc { answers } = &mut d.kind {
            answers[0].correct = true;
        }
        let keyed = d.build_question(Some(&q));
        assert!(!crate::model::is_keyless(&keyed) && crate::qcheck::blockers(&keyed).is_empty());
        // true/false
        let t = tf_question(true, false);
        let mut dt = Draft::from_question("b".into(), &t);
        assert_eq!(msgs_of(&dt), ["Pick True or False"]);
        let saved = dt.build_question(Some(&t));
        assert!(crate::model::is_keyless(&saved) && crate::qcheck::key_missing(&saved));
        dt.tf_keyed.push(None);
        assert!(dt.issues().is_empty());
    }

    /// Saving an incomplete question stores it, names it a draft in the toast, and a complete one
    /// does not.
    #[test]
    fn the_save_toast_says_when_it_saved_a_draft() {
        assert_eq!(draft_summary(2, true), "Saved as a draft: 2 things to fix");
        assert_eq!(draft_summary(1, false), "Saves as a draft: 1 thing to fix");
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
