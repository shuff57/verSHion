# Question editor phase 4: typing engine design

Written before any phase 4 code change. Branch `question-editor-polish`, HEAD 82464f4.

## 1. Where the engine stands (re-measured, not assumed)

Re-run on the web build before deciding (`trunk serve`, port 8781, `qedb4`). The baseline was taken
before phases 2 and 3; some of its defects no longer reproduce:

| Baseline defect | Today |
|---|---|
| 1. Backspace next to a formula destroys it | Fixed already: Backspace beside a formula removes it whole in one press (`05b`), Ctrl+Z restores it. Left/Right at a formula edge enters the inline equation editor (phase 3), so the caret cannot sit inside the markup. |
| 3. Ctrl+A selects one paragraph | Still true for a stem of several blocks: with a bullet list in the stem Ctrl+A selected only the last paragraph (`07b`). Plain multi-line paragraphs are one unit and select fine. |
| 4. Invisible selection over hidden markup, Shift+Arrow over raw `$` | Still true: four Shift+Right presses beside a formula show no selection at all (`08a-e`). |
| 2. `>` converts the line while typing | Still true. See section 4: the stored format reads a leading `>` as a quote, so a literal leading `>` cannot exist. |
| 8. Placeholder paragraphs shift layout | Still true for the "add text below" line. |
| 9. Paste parsed as markup | Inherent to the format (no escape syntax). |
| 7. Undo per field | Still true. |

## 2. Decision: TARGETED FIXES on a thin navigation layer, not a full replacement

Full replacement means writing our own caret, selection, hit-testing, wrapping, scrolling,
clipboard and IME handling over a block/inline model, then re-porting the toolbar, slash menu, link
popup, table bar, side-by-side panels, block rail, Make Blank, blank remapping and the equation overlay
(`mathedit.rs` addresses atoms by unit id and local char offset). egui's `TextEdit` gives us IME,
wrapping, cursor-from-pixel and clipboard for free, and none of that can be tested here without a
native IME. The risk of regressing phases 1 to 3 is high, the saved-format risk is zero only if every
one of the 73 existing wysiwyg tests keeps its subject, and the remaining measurable defects do not
need a new model:

* The markup string is already the single source of truth and atoms are already units for Backspace,
  Delete and plain arrows (`Ana::canonical`, `Ana::spot`).
* What is missing is (a) a selection model over the whole field and (b) atom-aware Shift+Arrow with
  visible selection. Both are small pure layers that sit above `TextEdit`.

What is added is therefore a *navigation model*, not a new document model. It is the part of the
recommended design that removes the measured defects (one selection over the whole field, atoms and
hidden markup as units, one undo order), kept behind pure functions so a later full replacement can
reuse them.

## 3. The model (what is added)

New file `src/wysiwyg/nav.rs`, pure functions over the existing `Ana` of one unit (no egui):

* **Spots.** A visible position is a `spot` (`Ana::spot`). An atom (`$..$`, `___`) is one spot;
  collapsed markers (`Role::Hidden`, `Tail`) are zero spots. `nav::step(ana, caret, dir)` returns the
  nearest char offset that is a canonical position of the next/previous spot, skipping hidden
  markers and jumping over a whole atom, so a caret never lands inside an atom or between hidden
  markers that look like the same place.
* **Selection snapping.** `nav::snap_range(ana, anchor, caret, prev_caret)` moves a selection's
  ends to atom boundaries (the end the user moved expands outward over an atom it touches) and to
  the outermost hidden-marker boundary, so what is selected is always what is drawn. A range that
  covers no visible spot collapses.
* **Extend.** `nav::extend(ana, anchor, caret, dir)` is Shift+Left/Right: one spot per press,
  atoms whole. The editor takes the key event and writes the result with `set_cursor`, so
  `TextEdit` never selects raw `$` characters.
* **Whole-field selection.** `Mem::all` (one flag in the existing per-editor memory). Ctrl+A in a
  field with more than one editable unit sets it; every unit paints its text as selected (an
  overlay, not `TextEdit` state). While set, the editor takes the keys: Backspace/Delete/Cut clear
  the whole stem (structure included), a typed character or paste replaces it, Copy copies the
  markup, arrows collapse to the start or end, any other key or a click clears it. A field with a
  single unit keeps egui's native Select All.

Map between caret/selection and markup offsets: unchanged. The caret is a global char offset into
the markup string (`m.sel`), a unit is the exact char range of its raw text, so selection ends and
the markup string never disagree. Lists, quotes, tables, headings, side-by-side panels, blanks,
pictures and math stay what they are today (units cut by `richtext::segment`, atoms by
`analyze`), so the serialised string is the stored string: nothing is serialised from a second
representation.

### Undo order across fields

Each editor keeps its own snapshot stacks (they carry the blanks rows and caret). A new
ctx-wide *order* list records which editor pushed each snapshot. Ctrl+Z pops the newest entry
of that list: if it belongs to the focused editor it runs as today; if it belongs to another editor
that was drawn in the last frames, that editor is told (via its `Mem`) to undo and take focus on
its next frame; entries of editors that are not drawn any more are dropped. Redo mirrors it. So
Ctrl+Z after typing in the stem and then in an answer undoes the answer, then the stem, in order.

## 4. How each listed defect is handled

1. **Arrows / Backspace walk raw `$`.** Already atom-safe for plain arrows, Backspace and Delete
   (re-verified live). Closed for Shift+Arrow by `nav::extend` and for mouse and Shift selections
   by `nav::snap_range`.
2. **Typed `>` converts the line.** The stored format treats a line starting with `>` as a quote
   (`richtext::quote`, no escape syntax), so a literal leading `>` cannot be stored without a
   format change, which is a stop rule. Not fixed as a conversion rule. Mitigated: Backspace at the
   start of the quote leaves it, Ctrl+Z restores, and the new whole-field Select All then Delete
   clears structure too (the quote that stuck after Ctrl+A in the baseline).
3. **Ctrl+A one paragraph.** Whole-field selection above.
4. **Stray caret dot, invisible selection.** Selection snapping makes every selection cover whole
   visible spots; the atom highlight already paints when an atom is inside the range.
7. **Undo per field and coarse.** Cross-field order above. Typing runs already coalesce by time per
   unit; toolbar ops, Make Blank and structure edits stay single steps.
8. **Layout-shifting placeholders.** The "add text below" row reserves its height always (the
   "above" row already did); only the hint text appears on focus.
9. **Paste parsed as markup.** The format has no escape syntax, so `$a$`, `**b**` or a leading `- `
   in pasted text is markup whatever the editor does. An unmatched `*` or `$` is already literal.
   Not fixed; needs a format decision (an escape) from the owner.
10. **First keystrokes lost after a click or Tab.** Investigated, section 7.

## 5. How phases 1 to 3 are preserved

No call site changes: `editor(ui, id, &mut String, Opts) -> Out`, `focus_end`, `request_equation`,
`edit_equation`, `record_external`, `is_plain` and the `Out` fields stay as they are.

* Autofocus and the Enter/Tab/Backspace answer-row flow (`lock_tab`, `focus_end`): untouched; the
  new key handling only runs for Shift+Arrow without Ctrl/Alt, Ctrl+A in a multi-unit field, and
  Ctrl+Z routing.
* `mathedit.rs`: untouched. Whole-field selection ends before a math session starts (a click or
  Enter on a formula clears it); `nav` only reads `Ana`.
* Make Blank, Ctrl+Shift+K, remap of blank rows: whole-field delete goes through the same `commit`,
  which already remaps the rows.
* Wide-formula shrink, link popup, floating and fixed toolbars: untouched; the floating toolbar
  is hidden while the whole field is selected (there is no unit selection to anchor it to).
* Alt+Enter and the rest of the modal keys live in `qedit.rs`: untouched.

## 6. Migration plan (each step leaves the app working and `cargo test` green)

1. `nav.rs` with `step`, `extend`, `snap_range` and property tests (pure, unused).
2. Shift+Arrow and selection snapping wired in `editor` / `after_unit`.
3. Reserve the "add text below" row.
4. Whole-field selection (`Mem::all`, painting, key handling) with tests for the pure part.
5. Cross-field undo order.
6. Round-trip corpus tests (every construct plus the fixture banks).
7. Live verification and regression pass.

## 7. First keystrokes after a click or Tab (investigated, two causes)

1. **Misrouted first key (fixed).** `focus_end` (Tab / Enter between answer rows) only queued the
   focus; the target editor took it when it next drew. A key typed in the very next frame was drawn
   first by the field that still had the focus, so the first letter of the next row landed in the
   previous row (and, because egui hands one `Text` event to every focused field it passes, in both:
   `firsts` / `second`). Measured live with 30 ms key delay: stem Tab first Tab second Tab third
   gave a doubled first letter. `focus_end` now also takes the focus at once (on the unit
   the editor last reported as its last), and the same run types every row cleanly. Test:
   `a_key_typed_right_after_a_focus_move_goes_to_the_field_that_took_the_focus`.
2. **Typing straight after opening or clicking (not reproduced).** With autofocus on open, text typed
   with no wait, 250 ms and 600 ms after Enter on the type menu arrived complete in all three runs.
   The baseline's lost keys came from typing before the web build had a frame with the field
   focused; the retry in `m.retry` covers that on our side.

## 8. The stray 1 px mark (cause found, fixed)

It is not drawn by the editor (a headless dump of every shape of a focused field holds only the glyphs
and the caret). egui-web keeps a hidden `<input>` at the caret that mirrors the last typed text;
its 1 px width shows a sliver of the typed character, right of the caret. `index.html` now makes
that element's text transparent (`body > input`). Checked live: no mark after typing.

## 9. Result against the list

| # | Result |
|---|---|
| 1 arrows / Backspace over `$..$` | already atom-safe; Shift+Arrow and selection ends now atom-aware (`nav`), live and tested |
| 2 `>` converts the line | not changed: the stored format reads a leading `>` as a quote, no escape exists. Needs a format decision |
| 3 Ctrl+A one paragraph | fixed (whole-field selection, visible, Delete/type/paste/copy/cut/arrows), also Ctrl+Home/End |
| 4 stray dot, invisible selection | fixed (section 8, selection snapping) |
| 7 undo per field | fixed: one order across stem and answers, redo too |
| 8 shifting placeholders | fixed: the "add text below" row now reserves its room like the one above |
| 9 paste as markup | not changed: an unmatched `*` or `$` is pasted literally (verified); a matched `$a$`, `**b**` or a leading `- ` is markup by the format. Needs an escape in the format |
| first keystrokes | misrouted key after Tab fixed; not reproduced otherwise |

## 10. Round 2 changes (critic feedback)

- **Keys of one frame apply in order.** A frame applies either a run of typed text or ONE key press; the rest of the frame's events are held back and replayed, in order, on the next frame (`hold_back` / `release_held` in `wysiwyg.rs`, called from the editor and, for the whole modal, from `qedit::draw_modal` through `wysiwyg::frame_keys`). Modifier keys going down (the browser sends Ctrl itself before the C of Ctrl+C) are not key presses for this rule and never end a whole-field selection.
- **Tab rule.** In the question stem Tab always leaves the stem for the first answer's text field (or the first field of the type) and Shift+Tab goes back one control, whatever the stem holds. Inside a table Tab and Shift+Tab walk the cells (a Tab in the last cell adds a row). A list item is nested with Ctrl+] and un-nested with Ctrl+[ (Tab/Shift+Tab nest only in editors that do not hand Tab to a form). A control that takes focus (Tab, Shift+Tab, a click) is scrolled into view by the modal body.
- **Undo is per editing session and per text.** A text replaced from outside the editor (a type switch turning blanks back into text) ends that field's undo history, and an undo step that would change nothing is skipped.
- **Floating selection toolbar.** In the margin right of the field when it fits; otherwise above, below or flush with the window's right edge, whichever hides the least text (anything outside the field counts double after the first 20 px). It never covers the fixed toolbar.
- **A formula being edited is outlined** (accent stroke) so a click that opened it is visible between caret blinks.
- **Latency control** (same harness, keyboard.type zero delay, polling screenshots until the 166 typed characters show; headless Chromium, software GL): see the report. A stock egui text field inside the same modal costs the same per character as our editor, so the cost is egui-web under software GL, not the editor.
