=== CRITIC P3 — TRANSCRIPT PANE (review.js renderText 357-484) ===
BAR reference: renderText 438-484, initText 357-385, makeWord 426-436, paintReading 197-203.

PASS against bar:
- Transcript shown (matches txText.textContent = '')
- Corrections listed (matches W.marked + W.wrap)
- Illegible markers listed (matches makeWord)
- Plain/formatted toggle (matches versionSeg +9)
- Word-click opens tray (matches 376-378)

FAIL / biggest gap:
**NO READING UNDERLINE (paintReading 197-203).** The bar underlines the current line in the transcript and scrolls it into view. Our candidate has no readLine state, no underline, no scroll-to-line. Without this, the transcript is a static block — users can't track where they are while reading the page.

Second gap: **NO STREAMING INDICATOR (review.js 445-448).** If the page is mid-transcription, the bar shows "Reading this page. The text appears as it is read." Our candidate has no status indicator.

Third gap: **NO ERROR BOX (review.js 455-458).** If the page failed to transcribe, the bar shows an error box with a retry button. Our candidate has no error handling.

Verdict: needs another cycle — add readLine state + underline + scroll-to-line.
