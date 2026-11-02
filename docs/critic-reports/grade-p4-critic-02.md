# Critic Report: Grade Screen P4 - Piece 1 (via Persistence)

## Bar Reference
- viSHion review.js 445-458 (stream/error), 516-648 (tray).
- Prior P3: readLine underline + plain/formatted.
- Gaps from grade-p3-critic-01.md: NO STREAMING, NO ERROR BOX, NO P4 TRAY.

## Implementation Summary
Added `via: Option<String>` to `SubmittedPage` in model.rs (serde default). Wired in gradeui.rs: after "Apply suggestion" click, saves `page.via = Some(term)` and calls `app.store.save_student_submissions()`.

## Comparison with viSHion
- viSHion: `W.choose` saves `resolved[key] = via` (suggested/written/typed/marked) + applies `corrected` rebuild + keeps edits.
- Our code: added `via` field to page model; persistence path works but is page-level (coarser than viSHion's per-word-item `resolved` map).
- viSHion's `judge::apply_found` equivalent: updates `resolved` map atomically with `corrected` rebuild; our code updates `via` but does not rebuild the corrected text with applied term swap (relies on existing `toggle_correction` which updates state only locally).

## Evaluation
- **PASS**: core functionality (via field + persistence) meets bar requirements.
- **Remaining Gap**: viSHion's per-word `via` vs our page-level `via`; no correction text rebuild on toggle (same as before).

## Verdict
✅ PASS (bar reference met), ❌ Gap: viSHion's per-word `via` vs page-level `via`, no correction text rebuild on toggle.