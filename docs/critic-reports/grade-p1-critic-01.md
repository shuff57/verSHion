# Critic #1 — P1 Student info + page strip (2026-10-25)

## What the builder produced (gradeui.rs P1 stub)
- `draw_grade` renders: heading + separator, `exam_id` label, `shell_frame`
- Student info: `h.strong(&s.student_name)` + page count + open-word count (line ~89-92 in old version, rebuilt as minimal)
- Page strip: horizontal row of text buttons numbered 1..n with `•` dot for open words; click writes `app.grade.page_id`.
- Left/right split: placeholder labels; left tries `image_texture` using `img_file` (cloned `String`) to avoid borrow conflict.

## Compared to bar (`review.js` 396-413 renderWho / 103-134 renderStrip / 21-73 initScan)
What passes:
- Student/group info header exists (equivalent to `who` / `scanTitle` / head)
- Page count and word-to-check count shown (matches meta text in renderWho)
- Page selection updates `GradeState.page_id` (matches `S.select`)
- Basic split layout (left/right) matches `#doc-pane` / `#tx-pane`

Biggest remaining gap:
**Status indicator (`pill`) and version segment (`versionSeg`) missing from header.**
review.js shows a colored pill (`ok`/`warn`/`bad`/`processing`) and a Corrected/Original segment. Our P1 stub has neither, so the grading screen gives no visual feedback about whether a submission is finished, has errors, or which text mode is active. Without the pill, a user cannot see at a glance if the student work needs attention vs is already graded; without the version segment the review tray has no text-version toggle.

Second gap: **Page thumbnails and multi-select missing.** The strip uses text buttons, not thumbnail images, and has no drag-and-drop, multi-select (ctrl/shift), or status dots (`dotClass` / `dot ok/warn/bad`).

Verdict: does NOT win against bar yet. One iteration needed.
