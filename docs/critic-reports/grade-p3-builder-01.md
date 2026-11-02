=== BUILDER P3 — TRANSCRIPT PANE (review.js renderText 357-484) ===
Target: renderText 438-484 (transcription viewer + word wrapping), initText 357-385 (pane wiring).

Plan — add to gradeui.rs:
1. transcript_pane(ui, app, page, state) renders:
   - ui.label("Transcript:") + full transcript text
   - For each correction/illegible, render a clickable "word" button (review.js makeWord 426-436)
   - On click: set state.sel = (page_id, key); state.tray_open = true (mirror review.js 376-378)
   - Reading underline: highlight the current readLine (review.js paintReading 197-203)
   - Plain/formatted toggle: ui.button("Show plain text") -> state.text_view = "raw" (mirror 479)
   - "Transcribe this page again" button -> calls again() (review.js 486-490)
2. Wire word-click -> ray hover -> tray open
3. Show "Reading this page" streaming indicator if status == transcribing (review.js 445-448)
4. Show error box if status == error (review.js 455-458)
5. Show pending state if status == pending (review.js 449-453)

Borrow-safety: pass page as &crate::model::SubmittedPage (immutable) — no Store borrow needed.
