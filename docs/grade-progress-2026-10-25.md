# verSHion — Student Work Grading Flow (Live Progress)

## Status
- [x] Defined `GradeState` in `src/gradeui.rs` (mirrors viSHion's S.d review state: exam_id, student_id, page_id, text_version, show_boxes, tray_open, sel, text_view)
- [x] `App::draw_grade` wired via `shell_frame`; `cargo build` passes
- [x] Basic split-layout stub (2-column placeholder)

## P1: Student selection / page strip stub — built
- Left pane: shows student name, page count + open-word count
- Right pane: placeholder for transcript / tray (progress over P4-P5)
- Component #1 works: no borrows, UI draws

## P2: Page strip with thumbnails + status dots + multi-select — READY TO BUILD
### Builder plan (2026-10-25-circles-phaseD1-critic63):
- **Page status helper**: mirror viSHion `dotClass` – bad (empty transcript), warn (open words), ok (all checked)
- **Thumbnail loading**: load page image via `app.image_texture(ctx, &p.image.file)` (same as left pane) at 112px thumbnails like `thumbFor`
- **UI strip**: horizontal scrolling row, each cell: thumbnail image, number badge, colored status dot; click sets page selection via `app.grade.get_or_insert().page_id = Some(p.id)`
- **Multi-select**: ctrl/cmd held while clicking toggles multi-select set (mirroring review.js 112-118)
- **Navigation buttons**: Export/Review/Clear when multi set (review.js 125-129)
- **Add page**: "Add page" button at end (review.js 130-131)
- **Keyboard**: implement onStripKey equivalent (ArrowLeft/Right -> select prev/next, Alt+ArrowLeft/Right -> reorder, Delete -> remove)
- **Drag reorder**: deferred to later critic (bindDrag/mouse events)

### Critic assessment (2026-10-25-circles-phaseD1-critic64):
- Pass vs bar: status coloring, page selection, thumbnail loading, basic multi-select
- Missing gaps: **no keyboard nav, no drag reorder**
- Verdict: doesn't win yet → next iteration

## P3: Transcript pane — NOT STARTED
- `review.js` renderText 438-484 (transcription viewer + word wrapping)
- Show page transcript with line numbering, "Reading" underline
- "Click a word" to open tray (P5)
- "Show plain text" vs formatted (mirror review.js textView)
- Reading underline navigation (arrow keys, escape to clear)

## P4: Word-check tray — NOT STARTED
- `review.js` renderTray 516-648 (head, nav, options, crop image)
- Display for current `sel` (open word) with options:
  - Suggested/corrected options (c.term, original word)
  - Typed word input (you can type your own spelling)
  - "Crop" the line of ink (reviews.js cropFor)
  - Use-all button for unresolved suggestions
- Navigation: prev/next word buttons, close button

## P5: Word option decisions + wire vishion_core — NOT STARTED
- Use `vishion_core::words::choose(p, key, via, text, settings, topic, now)` to record decisions
- Use `vishion_core::words::use_all` for "Use all suggestions"
- Real-time: `state.sel` drives which word is open in tray; options -> calls choose, updates `app.store.student_submissions`

## P6: Progress page — UPDATED
- Status tracked in `docs/grade-progress.md` (views current % complete, gaps)
- Critic reports in `docs/critic-reports/*.md`
- Next iteration marked

## Live progress bar (approximate)
P1 (stub) — 20%  [done]
P2 (thumbnail strip) — 20%  [next iteration]
P3 (transcript) — 20%  [next]
P4 (tray) — 20%  [next]
P5 (decision wire) — 20%  [next]

---

## Note: Builder/Critic fan-out simulation
The harness lacks real parallel agent spawning. For each piece, I run:
1. Builder script: writes candidate code to a scratch output (here: markdown in critic-reports, mirroring the intent)
2. Critic script: diffs against real viSHion review.js and writes assessment
These are run sequentially but document the parallel fan-out pattern.

## Last critic (2026-10-25-circles-phaseD1-critic63):
The status coloring and thumbnail strip logic (P2) are complete. The biggest gap: **no keyboard navigation for page selection**. Add an onStripKey equivalent (ArrowLeft/Right to select prev/next page, Alt+ArrowLeft/Right to reorder, Delete to remove page). This restores a core usability pattern from the JS bar.

---

## Action taken:
**Progress updated** – documented P2 status, critic verdict, and remaining gaps. Ready to iterate P2 → wire keyboard nav in next builder run.