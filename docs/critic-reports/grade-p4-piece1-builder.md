=== BUILDER P4-1 — STREAMING STATUS (review.js 445-448) ===
Target: renderText 445-448 (p.status === 'transcribing' => streamEl + foot message).
Plan: add PageStatus enum + transcribing branch to draw_transcript; render green "● Reading this page..." label.
Code: gradeui.rs PageStatus::Transcribing branch, stream_status(), draw_transcript.
Status: implemented in src/gradeui.rs.
