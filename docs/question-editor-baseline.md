# Question editor baseline (phase 0)

Measured 2026-10-24 on branch `question-editor-polish`, no source changes.

## Method

- Target: **web build** (`trunk serve`, Trunk.toml, 127.0.0.1:8781). Trunk built first try, no wasm-opt error. Native `cargo run` was not used.
- Driven with `playwright-cli -s=qed` at 1280x900 (also 900x800 and 640x800 for the narrow test), mouse and keyboard only (egui canvas, no DOM). Fresh browser state: no bank existed; created "Untitled Question Bank", then one question per type, then edited.
- Screenshots and crops are in `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/a17e7f5d-98a9-464e-aeef-3ce7dd6036c1/scratchpad/qed-baseline/` (referenced below as `NN-name.png`; `c-NN-name.png` are crops).
- "Click" counts include the click that gives a field focus. "Keys" are non-text keystrokes.

### Harness caveats (what I could not measure)

- **Double-click and triple-click word/line selection: not measurable.** A stock egui field (the bank Search box, `19c-control-dbl.png`) also showed no selection under playwright, so the harness does not deliver multi-clicks in a way egui accepts. Not counted as a defect of ours.
- **IME / composition: not tested** (no way to drive it).
- Fast mouse drags drop intermediate moves (selection stopped short in `18-drag.png`); slow drags work (`18b-drag-slow`). Harness artifact, not counted.
- Typing right after a click or Tab sometimes lost the first keystrokes (first attempt at the stem lost `What is $x^2`; search box lost its first string; Tab into answer 1 then `T1` produced nothing, `31-tab1`). Waiting about 1 s after focusing always worked. Cause unknown; probably egui-web's hidden input needs a repaint before it takes text. **Unconfirmed**, worth one native check in phase 1.

## 1. Click and keystroke counts

Starting point is the bank page. "Open editor" = 1 click on `Add question` (new) or 1 click on the card (edit); both expand a card in place.

| Task | Clicks | Keys | Evidence / notes |
|---|---|---|---|
| Add an answer | 2 (`+ Add answer`, then click the new field) | 0 | `37-add-answer`: new row appears unfocused, no caret. Enter in an answer field does nothing (`35-enter-on-last`). |
| Add an answer, keyboard only | not possible | | Enter does nothing; `+ Add answer` is only reachable by Tab through ~5 stops per answer row (Σ, up, down, lock, close), with no visible focus ring on those buttons (`32-tab1..4`). |
| Mark one correct | 1 (circle) | 0 | `36-correct-down-add`. Several circles can be ticked (multiple answer); there is no separate type for it. |
| Reorder an answer | 1 per step (up/down arrows, 24 px targets) | 0 | `36`. No drag. Moving an answer 3 places = 3 clicks. |
| Switch type mid-edit | 1 (type chip) | 0 | `47-switch-tf`, `48-switch-back-mc`. **Content is lost**: stem, points, difficulty, topics survive; every answer is dropped and replaced by defaults, with no warning and no undo. MC -> TF -> MC returned 4 empty answers. TF is set to True silently. Multipart part-type chips do the same per part (`Draft::set_kind`, `multipart_editor`). |
| Add a topic | 1 (field) | type, Enter | `38`, `39`. Chip label reads `algebra x` (a literal x). Suggestion chips for existing topics appear below the field even when it is empty (`60`). |
| Set difficulty | 1 | 0 | Toggle chip; no way to see it is a toggle. |
| Set points | 1 | type | Free text field; non-numbers parse to "none" silently (`set_points_text`). |
| Insert a formula (answer, via Σ) | 1 (Σ) | type, Enter | `41-eq-panel`..`43-eq-inserted`. |
| Insert a formula (stem, typed) | 0 | `$`, type, `$` | Typesets when the closing `$` is typed (`09`, `10`). |
| Edit an existing formula | 1 (click it) | edit, Enter | `44-eq-edit-open`. Panel reopens with the current formula. |
| Make a blank | 1 (`___ Blank`) after selecting a word | 3 to select a word by keyboard (End, Left, Ctrl+Shift+Left); drag = 0 keys | `67`, `68`. The accepted answer is pre-filled with the selected word. Floating and fixed toolbars both show the button. |
| Save | 1 (Done) or Ctrl/Cmd+Enter | | `57-ctrl-enter` works. The footer says only "Changes save when you press Done"; the shortcut is not shown. Done is below the fold for an MC question at 900 px tall (`56-top`). |
| Cancel | 1 | | `26`, `27`. Silent discard, no prompt, even for a multi-paragraph stem. |
| Esc | 1 key | | **One Esc while typing in the stem discards the whole edit** with no prompt (`75-edited` then `76-esc1`: " EDITED" gone, card collapsed, page scrolled to top). |
| Delete | 1 | | `78-after-delete-click`: immediate delete, then a banner with Undo above the grid. No confirm, but recoverable until dismissed. |

Full MC question from the bank page, minimum: Add question (1), stem (1 + typing), 4 answers (4 clicks + typing), mark correct (1), Done (1) = 8 clicks. TestParrot-style Enter-to-next-answer would remove 3 of the 4 answer clicks.

## 2. Typing feel (stem and answer fields)

Findings are for the stem unless stated. Answers use the same engine with `structured: false`.

1. **The model leaks raw markup.** Arrow keys walk the raw characters of a `$..$` atom, so a caret can sit inside it: from the start of `What is $x^2$`, 9 Right presses then typing `Z` put the Z inside the formula (`80-atom-arrow`: shows `Zx²`). Two Backspaces then deleted the Z and the opening `$`, leaving visible raw text `x^2$` (`81-atom-backspace`). The formula is destroyed by editing next to it.
2. **Typing `>` at the start of a line converts the line to a block quote** and swallows the character: Home then `>>` gave a quote bar with one `>` (`11-home`). The quote persists after Ctrl+A and Backspace (`15-clean`), until the quote button is clicked (`16-unquote`). Markdown-looking input is interpreted as structure while typing.
3. **Ctrl+A selects only the current paragraph/list item**, not the stem (`22-selectall`: only `bullet?` selected; Backspace deleted only that item). Shift+Up/Down does extend a selection across paragraphs (`24-shiftup`), so cross-paragraph selection exists but Select All does not use it.
4. **Caret after a click** is correct in plain text and past the end of the line (click at x=600 in a short answer put the caret after the last char, `82-84`). Around atoms the caret rendered as a detached dot (`10-closed-dollar`) and a stray 1 px mark persists below a field in several screenshots (`52`, `60`, `54`).
5. **Home/End**: work per visual line (`11-home`, End then `<<` landed at line end).
6. **Selection**: slow drag and shift-arrows work (`12-end-shiftleft`, `18b-drag-slow`). Double-click not measurable (see caveats).
7. **Undo/redo**: works with Ctrl+Z, Ctrl+Shift+Z and Ctrl+Y. Typing coalesces by time (a pause splits runs, `82-84`). Granularity is coarse: one undo step reverted both a typed run and the quote formatting (`13-undo-all`). **Undo is per field**: Ctrl+Z focused in an answer undid the answer, not the damaged stem above it.
8. **Newlines**: Enter adds a line, Enter twice a blank line (`20-enter`). After a list or quote, grey "(type here to add text above/below)" paragraphs appear around the block (`11`, `21`) and shift the layout.
9. **Paste**: works through the clipboard (`21-paste`). Pasted text is parsed as markup: `**bold**`, `$y^2$` and `- bullet` became bold, math and a list with no notice. A literal `*` or `$` cannot be pasted as text.
10. **`$x^2$` typesets as you type**: yes, at the moment the closing `$` is typed, not before (`09-open-dollar` raw, `10-closed-dollar` typeset). So phase 3 item a is already satisfied for the closing-`$` case. The rendered atom is grey, smaller and italic, and in answer rows the typeset fraction is about half text height (`43-eq-inserted`).
11. **Typing `a/b+sqrt(x)` in the equation panel gives `a/(b+sqrt(x))`** (`42-eq-typed`): the whole tail becomes the denominator. This is equaSHion behaviour, noted because it affects what teachers type.
12. **Floating toolbar on a selection**: appears below the selection with B, I, S, x2, x2 (sup), Centre, code, Σ, Link, Clear and, in a blank question, `___ Blank` (`18-drag`, `67-fill-select`). Problems: (a) it duplicates the fixed toolbar above the field, which is always visible (two toolbars, two rows each); (b) it is placed over whatever is under the stem, covering Points, Difficulty and the next label (`12-end-shiftleft`, `25-before-cancel`, `67`); (c) Centre is greyed in the fixed bar and active in the floating bar for the same selection.
13. Block rail: hover shows `+` and a grip left of the stem (`17b`); undocumented in the UI.

## 3. Layout

- **Two-column eye travel** (`04-addq`): the order a teacher needs is Type (right, top) -> stem (left, top) -> answers (left, below) -> Points/Difficulty/Topics (right) -> Explanation/Image (right, bottom) -> errors and Done (bottom). That is a Z path with the type chosen last in the reading order although it should be first (type determines what the left column contains). Type chips sit about 480 px from the stem.
- **Fixed formatting toolbar wraps to two rows** inside the stem field (about 90 px) before any text is visible (`04`).
- **Layout jumps (all measured):**
  - Expanding a card turns a ~265 px grid tile into a full-width card, reflows the grid, and scrolls the page: 134 px (`52-expand-mc`), 270 px (`74-fill-expanded`), 160 px (`60`). The page jumps again on Cancel/Done (`27`, `76-esc1`: scrolled to top).
  - **Validation errors collapsing move the controls under the cursor.** Filling the second answer removed one error line and shifted everything 24 px; marking a correct answer removed the last error and moved the footer 28 px (`33` vs `34`, `49` vs `50`). My next click then hit the wrong control (a "move down" landed on answer 1 instead of 2, `36`; an `+ Add answer` click missed, `36`). A user will do the same.
  - Switching type changes the card height (`61-tf`, `63-match`), so Done moves.
  - A stem growing a line moves the footer: Cancel was at y=841 with a quote open, 821 without (`25`, `26`); a click on the old position missed.
  - Delete adds a 50 px banner above the grid (`78`).
  - Inserting a formula into an answer row grows the row 16 px (`43`).
- **Floating toolbar and the equation panel overlay content** (see 2.12 and section 4).
- **Narrow window (< 640 px editor width):** at a 640 px window the editor stacks (`53-narrow-640`, `54-narrow-scrolled`). Type, Points, Difficulty, Topics, Explanation and Image all move below the answers, so switching type means scrolling past the answers it will delete. At 900 px it is still two columns (`53-narrow-900`). Outside the editor, at 640 px the bank header controls clip: `Difficulty` chip cut and the grid/list toggle overlaps it.
- **Scrolling:** the whole bank page is the scroll container; the editor has no scroll area of its own. At 1280x900 an MC card with metadata is taller than the viewport, so Done requires scrolling (`56-top`), and errors/Done are at the far end from the stem.

## 4. Equation panel

- **Opens** on: the Σ button in an answer or the stem's fixed toolbar (1 click), a click on an existing formula, or typed `/` at a word start. It is a floating `egui::Area` with a popup frame, not inline (`equation_panel`, `qedit.rs`).
- **Where:** anchored at the field's bottom-left, then clamped to the screen. It is far larger than the code's `PANEL_HEIGHT = 250.0` assumes: measured about 650 x 800 px (`41-eq-panel`): a palette of Structures, Greek, Operators (about 450 px of buttons), a handwriting-style canvas, a `Linear` text box, a "Display on its own line" radio and two buttons. At 1280x900 it covers the stem, every answer and the field being edited. The user cannot see what the formula goes into.
- **Steps to edit an existing formula:** click it (1), edit, Enter (1 key) = 1 click + keys. But the click target is the tiny atom (about 30 px wide in an answer row) and the panel then hides it.
- **Closing:** Enter inserts (`43`); Esc cancels and discards (`45-after-esc`); a click outside cancels silently and discards the edit (`46-click-outside`: typed `+1` lost); the buttons `Insert/Update equation` and `Cancel` also work. There is no unsaved-edit prompt for the click-outside case.
- Auto-focus works (typing worked after about 1 s).

## 5. Validation

Source: `Draft::validate` (`qedit.rs`), shown in `draw_inline` footer.

- **Blocks Done:** empty stem; MC needs >= 2 non-empty answers and >= 1 correct non-empty answer; Matching needs >= 2 complete pairs; Fill needs >= 1 blank in the stem and an accepted answer for each blank; Multipart needs a non-empty stem for each part and at least one part. TF and Short never block beyond the stem.
- **Display:** one red 12 px line per error, stacked above the footer buttons at the bottom of the card, away from the field (`04-addq`, `34`). No marking on the offending field. Done is greyed; the reasons are only the red lines. The list shrinks as you fix things, which moves the layout (section 3).
- **Holes:** a Multipart MC part with zero filled answers passes (check only fires when >= 2 answers are filled), and was saved as "b. Pick one." with no choices (`72`, `73-multi-saved`). Fill shows a `Blank 1` answer row before any blank exists (`66-fill`), then the error says "select a word ... and press ___ Blank".
- **Not validated:** points that fail to parse are silently dropped; no duplicate-answer check.

## Compared with TestParrot (from the brief, not driven)

| | verSHion today | TestParrot |
|---|---|---|
| Container | card expanded in a grid, whole page scrolls and jumps | one centered modal |
| Columns | two (type/points/topics on the right), stacked under 640 px | single column |
| Selection toolbar | floating bar plus a permanent two-row toolbar | floating bar only |
| `$..$` | typesets on the closing `$`; caret can enter the atom | renders as you type |
| Row controls | always visible: circle, up, down, lock, close (5 targets per row) | on hover |
| Esc | silently discards an edit | closes without a prompt (same data risk, but modal makes it expected) |
| Formula | 650 x 800 palette dialog covering the form | inline |

## Defect summary (ranked)

1. Backspace and arrows next to `$..$` expose and destroy markup; the caret enters the atom (`80`, `81`).
2. Esc, Cancel and click-outside-the-panel discard work silently, one key from loss (`76-esc1`, `46`, `27`).
3. Switching type deletes all answers without warning (`47`, `48`).
4. Validation errors collapsing shift controls under the cursor, causing mis-clicks (`36`, `50`); the same happens on expand/collapse and type switch.
5. The equation panel is a huge dialog that covers the form it edits (`41`).
6. Floating toolbar covers the next fields and duplicates the always-visible toolbar (`12`, `67`).
7. Ctrl+A selects one paragraph; `>` at line start converts to a quote and the quote sticks (`22`, `11`, `15`).
8. No keyboard path to add or walk answers; no visible focus ring on row buttons (`35`, `32`).
9. Pasted text is silently reinterpreted as markup (`21`).
10. Multipart accepts an MC part with no answers (`73`).

## Recommendation for phase 4

**The model needs replacing, not patching.** `wysiwyg.rs` (4289 lines) keeps the markup string as the only state and edits it through one egui `TextEdit` per paragraph/list item/table cell, re-segmenting each frame (module doc, lines 1-14). That shape produces defects 1, 7 and 9 directly:

- Math atoms are not units of the editing model. They are characters in the same raw string, reserved visually by a layouter. A caret and Backspace operate on the raw `$`, `^` and `$`, so they can step inside an atom and break it (defect 1). A fix has to intercept every key path (arrows, Backspace, Delete, selection, paste, undo) per-atom on top of `TextEdit`'s own cursor, which `TextEdit` does not allow to be atom-aware. The existing `Want`, `prev`, `rows` and `caret_rect` bookkeeping (`Mem`, lines 2289-2325) is already that workaround.
- Select All, selection across paragraphs and "stem as one document" fail because there are many independent `TextEdit`s with a global caret (char offset) reconciled by hand each frame (defect 7, `24-shiftup` shows cross-unit shifting exists but Select All does not use it).
- Markdown triggers (`>`, `-`, `1.`) are parsed from the live string, so typing structure characters changes structure while typing, and paste is parsed the same way (defects 2 and 9).
- Undo is a stem-level stack inside each editor instance, so there is no form-level undo (item 2.7). Phase 3 and the loop's own plan also need the inline equation editor to take over the caret; that needs atoms as first-class nodes.

What is salvageable: the pure functions on strings (`inline_op`, `make_blank`, `toggle_list`, `enter_op`, `table_op`, `slash_apply`, `remap_blanks`, the markup parser `richtext::segment`) and their tests (`mod tests`, line 4289). They map a markup string to a markup string and can sit under a new model unchanged, which is what keeps "imported questions, locked answers, pictures and blanks round-trip unchanged" safe.

Recommended direction for phase 4: a document model of blocks holding inline runs plus atom nodes (math, blank, picture), with one caret/selection state over the whole editor, serialised to the existing markup string on change and parsed from it on open. Stem and all answer fields would be views over that one model or over separate instances of it with a shared undo stack. Targeted fixes (skip atoms in arrow/Backspace handling, make Select All use the global range, disable `>` conversion) would remove defects 1 and 7 in the common case but leave 9, the per-field undo and the phase 3 caret handoff dependent on the same workaround layer. If phase 4 time-boxes to fixes only, the minimum is: atom-aware arrow/Backspace/Delete, global Select All, and a no-markdown-on-paste option.
