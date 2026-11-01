//! Inline editing of one `$..$` equation at the caret (question editor phase 3).
//!
//! This file is the whole of the math-atom interaction, kept apart from the text-editing model in
//! `wysiwyg.rs` so phase 4 can replace that model and port only the calls listed here:
//!
//! * `wysiwyg.rs` lays an atom out as a reserved slot and paints it (its own concern). While an
//!   equation is being edited it is told which char range to treat as an atom
//!   ([`set_forced`] / [`forced_for`]) and hands the slot's screen rect back ([`report`]).
//! * This file owns one [`MathEdit`] per editor id: the equaSHion `EquationEditor` drawn over the
//!   slot (`inline`, `overlay`, text style), the text range it writes back to, and how the session
//!   ends ([`Leave`]). The pure decisions are free functions with tests ([`finish`],
//!   [`write_back`], [`entry`], [`wrapped`]).
//!
//! Rules, in one place:
//!
//! * Undo. While the formula has the keyboard, Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y belong to the
//!   equation editor's own history (the text field has no focus, so it never sees them). The whole
//!   session is ONE step of the text field's undo: the text before the session is pushed when it
//!   ends with a different text, so Ctrl+Z in the text then removes or restores the whole formula.
//!   An inserted formula that is left empty is removed and pushes nothing.
//! * Esc leaves and keeps what was typed. There is no discard: a changed formula is undone with
//!   the text field's Ctrl+Z, an empty one disappears.
//! * Enter, an arrow at an edge, Esc and a click outside all end the session; the first three put
//!   the text caret back (after the formula, or before it for Left), a click leaves the caret to
//!   the click.
//! * The text is rewritten only when the equation tree changed, so opening and closing an equation
//!   without editing it leaves the markup byte for byte as it was.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;

use egui::{Id as UiId, Rect};

pub use crate::equation::Edge;
use crate::equation::EquationEditor;

use super::{Ana, AtomKind, Doc, Mem, Out, Want};

/// Text the equation editor shows, dimmed, while the formula is empty.
pub const PLACEHOLDER: &str = "formula";
/// Width of an empty formula's slot, in multiples of the font size (fits the placeholder).
pub const EMPTY_SLOT_EM: f32 = 4.4;
/// Blank space the host leaves around a typeset formula, in multiples of the font size; the
/// formula is centred in it. Must stay in step with `atom_box` in `wysiwyg.rs`.
pub const SLOT_EXTRA_EM: f32 = 0.24;
/// The padding `equation::math_image` lays an equation out with (points).
const HOST_PAD: f32 = 1.0;
/// A session whose editor was not drawn for this many frames is abandoned (the field vanished).
const STALE_FRAMES: u64 = 4;

/// What starts an editing session (queued in the editor's memory, started at the end of a frame
/// when the layout can tell where the formula is).
#[derive(Clone, Debug, PartialEq)]
pub enum Start {
    /// Replace the chars `at` (empty = the caret) with a new formula and edit it. `initial` is the
    /// linear text it starts with ("" for a new, empty one).
    Insert { at: Range<usize>, initial: String },
    /// Edit the existing `$..$` at `start..start + len`, caret at `edge`.
    Edit { start: usize, len: usize, edge: Edge },
}

/// How a session ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Leave {
    /// Enter: keep it, text caret after it.
    Enter,
    /// Esc: leave and keep, text caret after it.
    Esc,
    /// An arrow key at the edge of the formula.
    Exit(Edge),
    /// A press elsewhere or focus lost: keep it, the text caret is wherever the click put it.
    Click,
}

/// What the text looks like after a session ended.
#[derive(Clone, Debug, PartialEq)]
pub struct Finished {
    pub stem: String,
    /// Where the text caret goes (global char); None: leave it where the user clicked.
    pub caret: Option<usize>,
    /// The formula was empty and has been removed.
    pub removed: bool,
}

/// An editing session on one equation.
pub struct MathEdit {
    /// Global char index of the opening `$`.
    pub start: usize,
    /// Chars of the whole `$..$` now in the text (2 when empty: `$$`).
    pub len: usize,
    /// The editor; an Option only so its builder methods can take it by value.
    ed: Option<EquationEditor>,
    /// The text before the session (the undo step) and the caret then.
    pub base: String,
    pub base_caret: usize,
    /// Screen rect of the slot together with the editor's canvas, last frame (outside-click test).
    pub rect: Option<Rect>,
    /// Frames drawn, and the last frame number it was drawn in.
    pub frames: u32,
    last_frame: u64,
    /// Show the "no $" hint until this time.
    hint_until: f64,
}

/// The screen place of the formula's slot in the text (reported by `show_unit`).
#[derive(Clone, Copy, Debug)]
pub struct Slot {
    pub unit: UiId,
    /// Char index of the `$` inside the unit's text.
    pub local_start: usize,
    pub rect: Rect,
    /// y of the text baseline in the slot.
    pub base_y: f32,
    /// Left edge and width of the text area the formula sits in.
    pub line_left: f32,
    pub avail: f32,
}

#[derive(Default)]
struct Frame {
    forced: Option<(UiId, Range<usize>)>,
    slot: Option<Slot>,
}

thread_local! {
    /// Active sessions by editor id. (The equaSHion editor is not `Send`, so it cannot live in
    /// egui's data store; the app is single threaded.)
    static ACTIVE: RefCell<HashMap<UiId, MathEdit>> = Default::default();
    static FRAME: RefCell<Frame> = Default::default();
}

// ---------- per-frame hand-over between wysiwyg.rs and this file ----------

/// Start of a frame of editor: forget the last frame's hand-over.
pub fn begin_frame() {
    FRAME.with(|f| *f.borrow_mut() = Frame::default());
}

/// Treat chars `range` of unit `unit`'s text as the equation being edited, whatever the text says.
pub fn set_forced(unit: UiId, range: Range<usize>) {
    FRAME.with(|f| f.borrow_mut().forced = Some((unit, range)));
}

/// The forced char range of this unit, if it holds the equation being edited.
pub fn forced_for(unit: UiId) -> Option<Range<usize>> {
    FRAME.with(|f| f.borrow().forced.clone().filter(|(u, _)| *u == unit).map(|(_, r)| r))
}

/// `show_unit` reports where the (forced or just pressed) formula sits.
pub fn report(slot: Slot) {
    FRAME.with(|f| f.borrow_mut().slot = Some(slot));
}

pub fn slot() -> Option<Slot> {
    FRAME.with(|f| f.borrow().slot)
}

// ---------- session store ----------

pub fn take(id: UiId) -> Option<MathEdit> {
    ACTIVE.with(|a| a.borrow_mut().remove(&id))
}

pub fn put(id: UiId, me: MathEdit) {
    ACTIVE.with(|a| a.borrow_mut().insert(id, me));
}

/// Marks the frame in which any editor had an equation open, for [`active_recently`].
fn marker() -> UiId {
    UiId::new("wys_math_active_frame")
}

pub fn mark_active(ctx: &egui::Context) {
    // (read the frame number first: asking the context for it inside its own data lock deadlocks)
    let frame = ctx.cumulative_frame_nr();
    ctx.data_mut(|d| d.insert_temp(marker(), frame));
}

/// Was an equation being edited this frame or the last? The modal uses it so Esc, Ctrl+Enter and
/// a click on its backdrop reach the equation first.
pub fn active_recently(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<u64>(marker())).is_some_and(|f| f + 1 >= ctx.cumulative_frame_nr())
}

// ---------- pure decisions ----------

/// `text` without its `$` characters, and whether any were dropped.
pub fn strip_dollars(text: &str) -> (String, bool) {
    let clean: String = text.chars().filter(|&c| c != '$').collect();
    let dropped = clean.len() != text.len();
    (clean, dropped)
}

/// What to put around a new formula inserted between chars `before` and `after` so it cannot
/// fuse with a neighbouring `$..$` into `$a$$b$`: a space on the side that touches a `$`.
pub fn gaps(before: Option<char>, after: Option<char>) -> (&'static str, &'static str) {
    (if before == Some('$') { " " } else { "" }, if after == Some('$') { " " } else { "" })
}

/// `$linear$`, or `$$` for a blank equation.
pub fn wrapped(linear: &str) -> String {
    if linear.trim().is_empty() { "$$".to_string() } else { crate::equation::wrap_math(linear) }
}

/// True when `linear` can be written between `$`s (it has no `$` of its own to end it early).
pub fn writable(linear: &str) -> bool {
    !linear.contains('$') && !linear.contains('\n')
}

/// `stem` with the `len` chars at `start` replaced by the formula `linear`; and the new length.
pub fn write_back(stem: &str, start: usize, len: usize, linear: &str) -> (String, usize) {
    let w = wrapped(linear);
    let n = w.chars().count();
    (super::splice(stem, start..start + len, &w), n)
}

/// How the text looks when the session ends: the empty formula is removed, the caret goes where
/// `how` says.
pub fn finish(stem: &str, start: usize, len: usize, linear: &str, how: Leave) -> Finished {
    let blank = linear.trim().is_empty();
    let (stem, len) = if blank {
        (super::splice(stem, start..start + len, ""), 0)
    } else {
        (stem.to_string(), len)
    };
    let caret = match how {
        Leave::Exit(Edge::Left) => Some(start),
        Leave::Exit(Edge::Right) | Leave::Enter | Leave::Esc => Some(start + len),
        Leave::Click => None,
    };
    Finished { stem, caret, removed: blank }
}

/// Does the text before the session differ from the text after it (so it is one undo step)?
pub fn is_undo_step(base: &str, after: &str) -> bool {
    base != after
}

/// Arrowing from the text into a formula: the atom (index into `ana.atoms`) a collapsed caret at
/// `lc` is next to in the arrow's direction, and the edge of the formula the caret enters at.
/// "Next to" is by visible spot, so hidden markers between do not matter.
pub fn entry(ana: &Ana, lc: usize, going_right: bool) -> Option<(usize, Edge)> {
    ana.atoms.iter().position(|a| {
        matches!(a.kind, AtomKind::Math(_))
            && if going_right { lc <= a.start && ana.spot(lc) == ana.spot(a.start) } else { lc >= a.end && ana.spot(lc) == ana.spot(a.end) }
    }).map(|k| (k, if going_right { Edge::Left } else { Edge::Right }))
}

/// Is the selection `lo..hi` (local chars, `lo < hi`) one formula and nothing else visible? Enter
/// then edits it. A selection that reaches only part of the formula's markup counts: Shift+Arrow
/// from beside a formula ends inside its collapsed chars.
pub fn selected_formula(ana: &Ana, lo: usize, hi: usize) -> Option<usize> {
    ana.atoms.iter().position(|a| {
        matches!(a.kind, AtomKind::Math(_))
            && lo < a.end
            && a.start < hi
            && (lo..hi.min(ana.role.len())).all(|i| (a.start..a.end).contains(&i) || ana.role[i] == super::Role::Hidden)
    })
}

/// Where the editor's canvas goes so its formula sits exactly on the one the host drew:
/// baseline on the text baseline, formula's left edge `x_ink` (the host's image `x`).
pub fn canvas_origin(x_img: f32, base_y: f32, ed_baseline: Option<f32>, host_baseline: f32) -> egui::Pos2 {
    let pad = EquationEditor::inline_pad();
    // (the editor pads its layout more than the host does, hence the baseline guess before its first frame)
    let b = ed_baseline.unwrap_or(host_baseline + (pad.y - HOST_PAD));
    egui::Pos2::new(x_img + HOST_PAD - pad.x, base_y - b)
}

// ---------- the session ----------

/// What one frame of a session decided.
pub struct Frame1 {
    /// The text changed: new stem and the formula's new length.
    pub write: Option<(String, usize)>,
    pub leave: Option<Leave>,
}

impl MathEdit {
    /// A session on the `$..$` at `start..start + len` of `stem`, starting from `linear`.
    pub fn new(ctx: &egui::Context, stem_before: &str, caret_before: usize, start: usize, len: usize, linear: &str, edge: Edge, size: f32) -> Self {
        let mut ed = EquationEditor::with_linear(linear)
            .id_salt("vershion_inline_equation")
            .font_size(f64::from(size))
            .inline(true)
            .overlay(true)
            .display_style(false)
            .exit_at_edges(true)
            .placeholder(PLACEHOLDER)
            .auto_focus(true);
        ed.caret_to(edge);
        MathEdit { start, len, ed: Some(ed), base: stem_before.to_string(), base_caret: caret_before, rect: None, frames: 0, last_frame: ctx.cumulative_frame_nr(), hint_until: 0.0 }
    }

    pub fn linear(&self) -> String {
        self.ed.as_ref().map(|e| e.linear()).unwrap_or_default()
    }

    /// Flush a half-typed `\name` and return the final linear text.
    pub fn flush(&mut self) -> String {
        if let Some(e) = self.ed.as_mut() {
            e.mark_clean();
        }
        self.linear()
    }

    pub fn stale(&self, ctx: &egui::Context) -> bool {
        ctx.cumulative_frame_nr() > self.last_frame + STALE_FRAMES
    }

    pub fn surrender_focus(&self, ctx: &egui::Context) {
        if let Some(e) = &self.ed {
            ctx.memory_mut(|m| m.surrender_focus(e.canvas_id()));
        }
    }

    /// Draw the editor over `slot` and report what happened. `host` is the typeset image the host
    /// draws for the formula now (texture, size, baseline) when it can; the editor draws the
    /// formula itself when it cannot (half-typed input the host's parser refuses).
    #[allow(clippy::too_many_arguments)]
    pub fn show(&mut self, ui: &mut egui::Ui, slot: Slot, size: f32, color: egui::Color32, stem: &str) -> Frame1 {
        let ctx = ui.ctx().clone();
        self.last_frame = ctx.cumulative_frame_nr();
        self.frames += 1;
        let mut ed = self.ed.take().expect("editor present");
        ed.set_font_size(f64::from(size));
        // Esc is ours (the field above treats it as "close"): leave and keep.
        let esc = self.has_focus_of(&ctx, &ed) && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        // `$` ends a formula in the markup, so it cannot be typed in one: it is dropped from what
        // is typed or pasted (the rest is kept) and the hint says so. The editor therefore shows
        // exactly what will be stored.
        let dropped = ui.input_mut(|i| {
            let mut any = false;
            for e in i.events.iter_mut() {
                if let egui::Event::Text(t) | egui::Event::Paste(t) = e {
                    let (clean, d) = strip_dollars(t);
                    if d {
                        *t = clean;
                        any = true;
                    }
                }
            }
            i.events.retain(|e| !matches!(e, egui::Event::Text(t) | egui::Event::Paste(t) if t.is_empty()));
            any
        });
        let now_t = ui.input(|i| i.time);
        if dropped {
            self.hint_until = now_t + 2.5;
        }

        // The host's picture of what is in the text now (drawn under the editor's caret).
        let content = crate::wysiwyg::inner_content(stem, self.start, self.len);
        let host = (!content.is_empty()).then(|| crate::equation::math_image(&ctx, &content, size, f32::INFINITY)).flatten();
        let x_img = slot.rect.left() + size * SLOT_EXTRA_EM * 0.5;
        let host_b = host.as_ref().map_or(size, |h| h.2);
        let origin = canvas_origin(x_img, slot.base_y, ed.inline_baseline(), host_b);
        // Too wide for the line: the text shows it shrunk to fit (equaSHion's editor cannot wrap
        // its caret), so while it is edited it takes the whole line as an editor of its own
        // that shrinks and scrolls to keep the caret in view.
        let unwrapped = (!content.is_empty()).then(|| crate::equation::math_image(&ctx, &content, size, f32::INFINITY)).flatten().map_or(0.0, |h| h.1.x);
        let wide = unwrapped + size * SLOT_EXTRA_EM > slot.avail;
        let ed_draws = !content.is_empty() && host.is_none();
        let canvas_h = slot.rect.height().max(size * 3.0);
        ed = ed.inline(!wide).overlay(!ed_draws && !wide).show_palette(false).show_linear_field(false).canvas_height(canvas_h);
        let origin = if wide { egui::Pos2::new(slot.line_left, slot.rect.top()) } else { origin };

        // Reserve the paint order slot for the host image: under the editor's caret.
        let painter = ui.painter().clone();
        let idx = painter.add(egui::Shape::Noop);
        let area = Rect::from_min_size(origin, if wide { egui::vec2(slot.avail, canvas_h + 40.0) } else { egui::vec2(3000.0, (slot.rect.height() + 40.0).max(80.0)) });
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area).layout(egui::Layout::top_down(egui::Align::Min)));
        // the editor draws its caret, selection and active place in the visuals' selection colours:
        // use the house accent so they read on both themes
        child.visuals_mut().selection.stroke.color = crate::theme::accent_col();
        child.visuals_mut().selection.bg_fill = crate::theme::accent_col().gamma_multiply(0.45);
        let r = ed.show(&mut child);

        // The picture of the formula as it is after this frame's typing.
        let now = ed.linear();
        let now_content = now.trim().to_string();
        let img = if now_content.is_empty() { None } else { crate::equation::math_image(&ctx, &now_content, size, f32::INFINITY) };
        if let Some((tex, sz, b)) = img.filter(|_| !wide) {
            let y = slot.base_y - b;
            let pos = egui::Pos2::new(x_img, y);
            painter.set(idx, egui::Shape::image(tex, Rect::from_min_size(pos, sz), Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)), color));
        }
        if now_t < self.hint_until {
            let g = ui.painter().layout_no_wrap("$ is not used inside a formula".to_string(), egui::FontId::proportional(10.5), crate::theme::muted_col());
            let at = egui::Pos2::new(slot.rect.left(), slot.rect.bottom() + 1.0);
            ui.painter().rect_filled(Rect::from_min_size(at, g.size()).expand(2.0), 3.0, crate::theme::card());
            ui.painter().galley(at, g, crate::theme::muted_col());
            ctx.request_repaint_after(std::time::Duration::from_millis(300));
        }
        // Next frame's outside-click area: the slot and the canvas.
        self.rect = Some(slot.rect.union(r.rect).expand(3.0));
        // a formula being edited is marked, so it is clear a click opened it (the caret blinks)
        ui.painter().rect_stroke(
            slot.rect.expand2(egui::vec2(-1.0, -1.0)),
            egui::CornerRadius::same(5),
            egui::Stroke::new(1.5, crate::theme::accent_col().gamma_multiply(0.8)),
            egui::StrokeKind::Inside,
        );

        let mut write = None;
        if r.changed && writable(&now) && now != self.linear_in_text(stem) {
            write = Some(write_back(stem, self.start, self.len, &now));
        }
        let leave = if r.committed {
            Some(Leave::Enter)
        } else if esc {
            Some(Leave::Esc)
        } else if let Some(e) = r.exit {
            Some(Leave::Exit(e))
        } else {
            None
        };
        self.ed = Some(ed);
        if let Some((_, n)) = &write {
            self.len = *n;
            ctx.request_repaint();
        }
        Frame1 { write, leave }
    }

    fn has_focus_of(&self, ctx: &egui::Context, ed: &EquationEditor) -> bool {
        ctx.memory(|m| m.has_focus(ed.canvas_id()))
    }

    fn linear_in_text(&self, stem: &str) -> String {
        crate::wysiwyg::inner_content(stem, self.start, self.len)
    }
}


// ---------- glue to the text editor (wysiwyg.rs) ----------

/// Is the text still the `$..$` this session is about?
pub(super) fn valid(stem: &str, e: &MathEdit) -> bool {
    let ch: Vec<char> = stem.chars().collect();
    e.len >= 2 && e.start + e.len <= ch.len() && ch[e.start] == '$' && ch[e.start + e.len - 1] == '$'
}

/// The flat unit holding the equation and the equation's char range inside that unit's text.
pub(super) fn unit_of(d: &Doc, e: &MathEdit) -> Option<(usize, Range<usize>)> {
    d.flat().iter().enumerate().find_map(|(k, &at)| {
        let u = d.unit(at);
        (u.virt.is_none() && u.start <= e.start && e.start + e.len <= u.end).then(|| (k, e.start - u.start..e.start + e.len - u.start))
    })
}

/// Start a session. An insert puts `$..$` (or `$$`) into the text first; the editor appears next
/// frame, once the layout has a slot for it. None when the request no longer fits the text.
pub(super) fn begin(ctx: &egui::Context, stem: &mut String, req: Start, size: f32, m: &mut Mem, out: &mut Out) -> Option<MathEdit> {
    let me = match req {
        Start::Insert { at, initial } => {
            let n = stem.chars().count();
            let (a, b) = (at.start.min(n), at.end.clamp(at.start.min(n), n));
            let initial = if writable(&initial) { initial } else { String::new() };
            let base = stem.clone();
            let w = wrapped(&initial);
            let ch: Vec<char> = stem.chars().collect();
            let (pre, post) = gaps(a.checked_sub(1).and_then(|i| ch.get(i)).copied(), ch.get(b).copied());
            *stem = super::splice(stem, a..b, &format!("{pre}{w}{post}"));
            out.changed = true;
            let a = a + pre.len();
            MathEdit::new(ctx, &base, a, a, w.chars().count(), &initial, Edge::Right, size)
        }
        Start::Edit { start, len, edge } => {
            let probe = MathEdit::new(ctx, stem, start, start, len, "", edge, size);
            if !valid(stem, &probe) {
                return None;
            }
            let linear = super::inner_content(stem, start, len);
            MathEdit::new(ctx, stem, start, start, len, &linear, edge, size)
        }
    };
    // the text field must not take the keyboard back while the formula has it
    m.want = None;
    m.retry = None;
    m.pending = None;
    ctx.request_repaint();
    Some(me)
}

/// End a session: write the final formula (an empty one is removed), make the whole session one
/// undo step, and hand the keyboard back to the text unless a click took it elsewhere.
#[allow(clippy::too_many_arguments)]
pub(super) fn end(ctx: &egui::Context, m: &mut Mem, stem: &mut String, mut e: MathEdit, how: Leave, blanks: Option<&[String]>, now: f64, out: &mut Out) {
    let ok = valid(stem, &e);
    let (mut text, mut len) = (stem.clone(), e.len);
    if ok {
        let linear = e.flush();
        let shown = super::inner_content(stem, e.start, e.len);
        if writable(&linear) && linear != shown {
            (text, len) = write_back(stem, e.start, e.len, &linear);
        }
        let linear = if writable(&linear) { linear } else { shown };
        let f = finish(&text, e.start, len, &linear, how);
        text = f.stem;
        if let Some(c) = f.caret {
            m.want = Some(Want { anchor: c, caret: c, virt: None });
            m.retry = None;
        }
    }
    if text != *stem {
        *stem = text;
        out.changed = true;
    }
    if is_undo_step(&e.base, stem) {
        super::push_undo(m, &e.base, e.base_caret, blanks, now, None, false);
    }
    if how == Leave::Click || !ok {
        e.surrender_focus(ctx);
    }
    // (so Esc, or Ctrl+Enter, pressed after this frame belong to the modal again)
    ctx.data_mut(|d| d.remove_temp::<u64>(marker()));
    ctx.request_repaint();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wysiwyg::analyze;

    fn ana(t: &str) -> Ana {
        analyze(t, &|_| true)
    }

    #[test]
    fn dollars_are_dropped_and_the_rest_kept() {
        assert_eq!(strip_dollars("k$m"), ("km".to_string(), true));
        assert_eq!(strip_dollars("$k"), ("k".to_string(), true));
        assert_eq!(strip_dollars("k$"), ("k".to_string(), true));
        assert_eq!(strip_dollars("$$"), (String::new(), true));
        assert_eq!(strip_dollars("x^2"), ("x^2".to_string(), false));
    }

    #[test]
    fn a_new_formula_does_not_fuse_with_a_neighbour() {
        assert_eq!(gaps(Some('$'), None), (" ", ""));
        assert_eq!(gaps(None, Some('$')), ("", " "));
        assert_eq!(gaps(Some('a'), Some('b')), ("", ""));
    }

    #[test]
    fn blank_formula_is_two_dollars() {
        assert_eq!(wrapped(""), "$$");
        assert_eq!(wrapped("  "), "$$");
        assert_eq!(wrapped("x^2"), "$x^2$");
        assert_eq!(wrapped(" x^2 "), "$x^2$");
    }

    #[test]
    fn write_back_replaces_only_the_formula() {
        let (s, n) = write_back("a $x$ b", 2, 3, "x^2");
        assert_eq!((s.as_str(), n), ("a $x^2$ b", 5));
        // grows from empty
        let (s, n) = write_back("a $$ b", 2, 2, "1/2");
        assert_eq!((s.as_str(), n), ("a $1/2$ b", 5));
        // and back to empty
        let (s, n) = write_back("a $x$ b", 2, 3, "");
        assert_eq!((s.as_str(), n), ("a $$ b", 2));
    }

    #[test]
    fn write_back_counts_chars_not_bytes() {
        let (s, n) = write_back("é $x$ ü", 2, 3, "π");
        assert_eq!((s.as_str(), n), ("é $π$ ü", 3));
    }

    #[test]
    fn dollar_or_newline_in_linear_is_not_written() {
        assert!(writable("x^2"));
        assert!(!writable("a$b"));
        assert!(!writable("a\nb"));
    }

    #[test]
    fn leaving_right_puts_the_caret_after_the_formula() {
        let f = finish("a $x^2$ b", 2, 5, "x^2", Leave::Exit(Edge::Right));
        assert_eq!(f, Finished { stem: "a $x^2$ b".into(), caret: Some(7), removed: false });
    }

    #[test]
    fn leaving_left_puts_the_caret_before_the_formula() {
        let f = finish("a $x^2$ b", 2, 5, "x^2", Leave::Exit(Edge::Left));
        assert_eq!(f.caret, Some(2));
        assert!(!f.removed);
    }

    #[test]
    fn enter_and_esc_leave_after_and_keep() {
        for how in [Leave::Enter, Leave::Esc] {
            let f = finish("$a/b$", 0, 5, "a/b", how);
            assert_eq!((f.stem.as_str(), f.caret, f.removed), ("$a/b$", Some(5), false));
        }
    }

    #[test]
    fn a_click_leaves_the_caret_alone() {
        assert_eq!(finish("$x$", 0, 3, "x", Leave::Click).caret, None);
    }

    #[test]
    fn an_empty_formula_is_removed_whatever_ends_it() {
        for how in [Leave::Enter, Leave::Esc, Leave::Click, Leave::Exit(Edge::Left), Leave::Exit(Edge::Right)] {
            let f = finish("ab $$ cd", 3, 2, "", how);
            assert_eq!(f.stem, "ab  cd");
            assert!(f.removed);
            match how {
                Leave::Click => assert_eq!(f.caret, None),
                _ => assert_eq!(f.caret, Some(3)),
            }
        }
    }

    #[test]
    fn a_formula_emptied_by_editing_is_removed_too() {
        // an existing $x$ whose content was deleted
        let f = finish("a $$ b", 2, 2, "", Leave::Enter);
        assert_eq!(f.stem, "a  b");
        assert_eq!(f.caret, Some(2));
    }

    #[test]
    fn session_is_one_undo_step_only_when_the_text_changed() {
        assert!(is_undo_step("a", "a $x$"));
        assert!(!is_undo_step("a ", "a "));
    }

    #[test]
    fn right_arrow_before_a_formula_enters_at_its_left_edge() {
        let a = ana("ab$x$cd");
        assert_eq!(entry(&a, 2, true), Some((0, Edge::Left)));
        assert_eq!(entry(&a, 1, true), None);
        assert_eq!(entry(&a, 5, true), None);
    }

    #[test]
    fn left_arrow_after_a_formula_enters_at_its_right_edge() {
        let a = ana("ab$x$cd");
        assert_eq!(entry(&a, 5, false), Some((0, Edge::Right)));
        assert_eq!(entry(&a, 2, false), None);
    }

    #[test]
    fn a_blank_is_not_a_formula() {
        let a = ana("ab___cd");
        assert_eq!(entry(&a, 2, true), None);
    }

    #[test]
    fn markers_between_caret_and_formula_do_not_matter() {
        // **a**$x$: the caret after the closing ** is the same visible spot as before the formula
        let a = ana("**a**$x$");
        assert_eq!(entry(&a, 5, true), Some((0, Edge::Left)));
    }

    #[test]
    fn enter_edits_a_selected_formula_only() {
        let a = ana("ab$x$cd");
        assert_eq!(selected_formula(&a, 2, 5), Some(0));
        // Shift+Left from after the formula ends inside its collapsed chars: still the formula
        assert_eq!(selected_formula(&a, 4, 5), Some(0));
        assert_eq!(selected_formula(&a, 2, 4), Some(0));
        // any visible neighbour in the selection: not a formula selection
        assert_eq!(selected_formula(&a, 1, 5), None);
        assert_eq!(selected_formula(&a, 2, 6), None);
        // nothing to do with a formula
        assert_eq!(selected_formula(&a, 0, 2), None);
    }

    #[test]
    fn forced_range_makes_an_empty_formula_an_atom() {
        let a = crate::wysiwyg::analyze_with("a $$ b", &|_| false, Some(2..4));
        assert_eq!(a.atoms.len(), 1);
        assert_eq!((a.atoms[0].start, a.atoms[0].end), (2, 4));
        assert_eq!(a.atoms[0].kind, AtomKind::Math(String::new()));
        // without it the text is plain
        assert!(crate::wysiwyg::analyze_with("a $$ b", &|_| false, None).atoms.is_empty());
    }

    #[test]
    fn forced_range_overrides_a_formula_the_parser_refuses() {
        let a = crate::wysiwyg::analyze_with("$x/$", &|_| false, Some(0..4));
        assert_eq!(a.atoms.len(), 1);
        assert_eq!(a.atoms[0].kind, AtomKind::Math("x/".into()));
    }
}

/// The whole interaction through real egui frames (no window): edit in place, edge exits, insert,
/// Esc, and the undo rule.
#[cfg(test)]
mod frames {
    use egui::{Event, Key, Modifiers, Pos2, RawInput, Rect};

    use crate::wysiwyg::{Opts, Out};

    use super::UiId;

    struct H {
        ctx: egui::Context,
        stem: String,
        t: f64,
        id: UiId,
        last: Option<Out>,
    }

    impl H {
        fn new(stem: &str) -> H {
            let ctx = egui::Context::default();
            crate::theme::setup(&ctx);
            let mut h = H { ctx, stem: stem.to_string(), t: 0.0, id: UiId::new("frames_editor"), last: None };
            h.run(3);
            h
        }

        fn frame(&mut self, events: Vec<Event>) {
            let mut input = RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(900.0, 600.0)));
            input.time = Some(self.t);
            input.events = events;
            self.t += 0.05;
            let (id, mut out) = (self.id, None);
            let stem = &mut self.stem;
            let o = self.ctx.run_ui(input, |ui| {
                out = Some(crate::wysiwyg::editor(ui, id, stem, Opts { size: 14.5, width: 600.0, structured: true, newlines: true, hint: "", blanks: None, toolbar: false, lock_tab: false }));
            });
            o.drop_without_applying_deltas();
            self.last = out;
        }

        fn run(&mut self, n: usize) {
            for _ in 0..n {
                self.frame(vec![]);
            }
        }

        fn key(&mut self, key: Key, modifiers: Modifiers) {
            self.frame(vec![Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers }]);
            self.run(3);
        }

        fn text(&mut self, s: &str) {
            self.frame(vec![Event::Text(s.to_string())]);
            self.run(3);
        }

        fn math(&self) -> bool {
            self.last.as_ref().is_some_and(|o| o.math)
        }
    }

    #[test]
    fn edit_in_place_then_enter_returns_the_keyboard_to_the_text() {
        let mut h = H::new("a $x^2$ b");
        crate::wysiwyg::edit_equation(&h.ctx, h.id, 2..7);
        h.run(6);
        assert!(h.math(), "the equation is being edited");
        assert_eq!(h.stem, "a $x^2$ b", "opening it changes nothing");
        h.text("+1");
        assert_eq!(h.stem, "a $x^2+1$ b");
        assert!(h.math());
        h.key(Key::Enter, Modifiers::NONE);
        assert!(!h.math(), "Enter leaves");
        h.text("Z");
        assert_eq!(h.stem, "a $x^2+1$Z b", "typing continues in the text, after the formula");
    }

    #[test]
    fn opening_and_leaving_without_a_change_keeps_the_markup_byte_for_byte() {
        let mut h = H::new("a $1/2$ b");
        crate::wysiwyg::edit_equation(&h.ctx, h.id, 2..7);
        h.run(6);
        assert!(h.math());
        h.key(Key::Escape, Modifiers::NONE);
        assert!(!h.math());
        assert_eq!(h.stem, "a $1/2$ b");
    }

    #[test]
    fn arrow_at_the_right_edge_moves_the_text_caret_past_the_formula() {
        let mut h = H::new("a $x$ b");
        crate::wysiwyg::edit_equation(&h.ctx, h.id, 2..5);
        h.run(6);
        h.key(Key::ArrowRight, Modifiers::NONE);
        assert!(!h.math());
        h.text("Q");
        assert_eq!(h.stem, "a $x$Q b");
    }

    #[test]
    fn arrow_at_the_left_edge_moves_the_text_caret_before_the_formula() {
        let mut h = H::new("a $x$ b");
        crate::wysiwyg::edit_equation(&h.ctx, h.id, 2..5);
        h.run(6);
        h.key(Key::ArrowLeft, Modifiers::NONE); // end -> before x
        assert!(h.math());
        h.key(Key::ArrowLeft, Modifiers::NONE); // at the edge: leaves
        assert!(!h.math());
        h.text("Q");
        assert_eq!(h.stem, "a Q$x$ b");
    }

    #[test]
    fn arrowing_from_the_text_into_a_formula_enters_it_at_the_near_edge() {
        let mut h = H::new("a $x$ b");
        crate::wysiwyg::focus_end(&h.ctx, h.id, 7);
        h.run(4);
        h.key(Key::ArrowLeft, Modifiers::NONE); // before b
        h.key(Key::ArrowLeft, Modifiers::NONE); // after the formula
        assert!(!h.math());
        h.key(Key::ArrowLeft, Modifiers::NONE); // into it, at its right edge
        assert!(h.math());
        h.text("Q");
        assert_eq!(h.stem, "a $xQ$ b");
        h.key(Key::ArrowLeft, Modifiers::NONE);
        h.key(Key::ArrowLeft, Modifiers::NONE);
        h.key(Key::ArrowLeft, Modifiers::NONE); // out at the left edge
        assert!(!h.math());
        h.key(Key::ArrowRight, Modifiers::NONE); // and back in at its left edge
        assert!(h.math());
        h.text("P");
        assert_eq!(h.stem, "a $PxQ$ b");
    }

    #[test]
    fn insert_with_no_selection_adds_an_empty_equation_and_esc_removes_it() {
        let mut h = H::new("ab");
        crate::wysiwyg::request_equation(&h.ctx, h.id);
        h.run(6);
        assert!(h.math());
        assert_eq!(h.stem, "ab$$", "an empty equation is in the text while it is edited");
        h.key(Key::Escape, Modifiers::NONE);
        assert!(!h.math());
        assert_eq!(h.stem, "ab", "left empty it is removed");
    }

    #[test]
    fn a_dollar_typed_in_a_formula_is_not_accepted_and_the_rest_is_kept() {
        let mut h = H::new("a $y_1$ b");
        crate::wysiwyg::edit_equation(&h.ctx, h.id, 2..7);
        h.run(6);
        h.text("k$m");
        assert_eq!(h.stem, "a $y_1km$ b", "what the editor shows is what is stored");
        h.text("$");
        h.text("$$");
        assert_eq!(h.stem, "a $y_1km$ b");
        h.key(Key::Enter, Modifiers::NONE);
        assert_eq!(h.stem, "a $y_1km$ b");
    }

    #[test]
    fn a_second_new_formula_right_after_a_formula_is_kept_apart() {
        let mut h = H::new("$a$");
        crate::wysiwyg::request_equation(&h.ctx, h.id);
        h.run(6);
        assert_eq!(h.stem, "$a$ $$");
        h.text("b");
        h.key(Key::Enter, Modifiers::NONE);
        assert_eq!(h.stem, "$a$ $b$");
    }

    #[test]
    fn a_typed_formula_is_kept_by_esc() {
        let mut h = H::new("ab");
        crate::wysiwyg::request_equation(&h.ctx, h.id);
        h.run(6);
        h.text("x^2");
        assert_eq!(h.stem, "ab$x^2$");
        h.key(Key::Escape, Modifiers::NONE);
        assert!(!h.math());
        assert_eq!(h.stem, "ab$x^2$", "Esc never discards typed math");
    }

    #[test]
    fn the_whole_session_is_one_text_undo_step() {
        let mut h = H::new("a $x^2$ b");
        crate::wysiwyg::edit_equation(&h.ctx, h.id, 2..7);
        h.run(6);
        h.text("+1");
        h.text("+2");
        h.key(Key::Enter, Modifiers::NONE);
        assert_eq!(h.stem, "a $x^2+1+2$ b");
        // the text field has the keyboard again: one Ctrl+Z undoes both edits
        h.key(Key::Z, Modifiers::COMMAND);
        assert_eq!(h.stem, "a $x^2$ b");
    }

    #[test]
    fn ctrl_z_inside_the_formula_undoes_inside_the_formula_only() {
        let mut h = H::new("a $x^2$ b");
        crate::wysiwyg::edit_equation(&h.ctx, h.id, 2..7);
        h.run(6);
        h.text("+1");
        assert_eq!(h.stem, "a $x^2+1$ b");
        h.key(Key::Z, Modifiers::COMMAND);
        assert!(h.math(), "still editing the formula");
        assert_eq!(h.stem, "a $x^2$ b", "the formula's own history stepped back");
    }
}
