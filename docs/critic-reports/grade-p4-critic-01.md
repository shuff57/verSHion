# grade-p4-critic-01 — P4 WORD-CHECK TRAY + STREAMING/ERROR FULL PARITY

Bar: viSHion review.js 445-458 (stream/status), 455-458 (error box), 516-648 (tray), words::items/effective/marked, judge::heuristic/apply_found/rebuild/accept, student::parse_reply, groups::tally/name_of/key_of.

## PASS against bar

### PASS — Streaming/status indicator (review.js 445-448)
- gradeui.rs `PageStatus::Transcribing` branch renders "● Reading this page. The text appears as it is read." (green), mirroring `streamEl` + `foot.append(... 'Reading this page...')`.
- The bar requires a status indicator when `p.status === 'transcribing`; our candidate now renders it in `draw_transcript` / render path.

### PASS — Error box with retry (review.js 455-458)
- gradeui.rs `PageStatus::Error` branch renders red "⚠ The page could not be transcribed." plus a "Retry this page" button calling `app.grade.reset()` / page re-selection, mirroring `errbox` + `Retry this page` button (`Q.start([p.id], { force: true })`).
- Matches bar references: errbox role alert, warn icon, retry button.

### PASS — Reading underline exists (paintReading 197-203)
- `set_read_line` / `move_read_line` state + underline painted under current line via `ui.painter().hline(rect.bottom())`, plus scrollIntoView-equivalent via nav buttons (mirrors review.js paintReading 197-203).

### PASS — Plain/formatted toggle (review.js 479)
- "Show plain text" / "Show formatted" buttons toggle `state.text_view` between "raw" / "formatted", mirroring `versionSeg()` + button at renderText 479.

## FAIL / gaps still present

### FAIL — No persistent corrections (judge::toggle for undo/reapply)
The bar's `W.items` with `via` field tracks chosen values per word; `choose(via, text)` persists via `W.choose`, `advance()` moves to next word via `allItems`, `W.useAll` applies all suggestions at once, and `tray-note` says "You chose..." / "You typed..." with undo options. Our candidate:
- `toggle_correction()` exists but has no real undo/reapply UX.
- No `via`-style flag (`written`/`typed`/`suggested`/`marked`) per word key.
- No undo-indicator ("Uncheck this correction" exists but no state restoration).
- Single biggest gap among P4 items: **no undo/reapply state management** — changes are not persistent across navigation.
- Word list exists but has no `state.sel`-driven navigation with `advance()`.

### FAIL — No effective transcript rebuild (W.effective, W.marked, effText)
The bar's `effText()` returns corrected text: `S.d.textVersion === 'original' ? p.raw : W.effective(p)`, then `W.wrap()` renders `.w` buttons with checked/unchecked states, and `W.marked()` underlines corrected words. Our candidate:
- `eff_text()` builds replacement markers but the result is not actually rendered inline per-word.
- `draw_render_text()` shows raw/flat `eff_display` text; no `.w` word buttons, no `W.marked` underlining per word.
- No `shown` vs `original` vs `corrected` distinction.
- Bar reference: words::effective (rebuild transcript with corrections applied), words::marked (underline corrected words in-place).

### FAIL — No student::parse_reply integration
The bar's `changeStudent()` uses `G.names()` / student list + `student::parse_reply`-style name extraction from a modal (select existing / type new / "A new student…"). Our candidate:
- Shows `s.student_name` but never parses/extracted student names from page content.
- No modal with `sel2` select + `__new` option + `input` field.
- No `source: 'chosen'`, `confirmed: true` fields.
- Bar reference: student::parse_reply for choosing a student for a page.

### FAIL — No groups::tally/name_of/key_of page grouping
The bar groups pages by student via `G.of(id)`, `G.names()`, `g.key`, `g.pages`, counts open words per student (`W.open(p).length`), and `advance()` walks across pages via `allItems(g)`. Our candidate:
- `tally_words()` exists but is only referenced in a label; no `name_of` / `key_of` mapping used anywhere.
- No per-student word tally driving badge/dot status (bar `dotClass` / `pillFor`).
- No `trayGroup` comparison (`g.key !== trayGroup`) that auto-opens tray per student.
- Bar reference: groups::list/tally/name_of/key_of — grouping pages, tallying open words, extracting names, computing keys.

### FAIL — tray UI incomplete (review.js 516-648)
The bar tray renders: head ("Check this word" / "All words checked"), "Use all suggestions" button for 2+ unresolved, prev/next nav + count pill ("idx of n"), close/Done button, tray-note ("You chose...", "The model wrote...", "The model could not read..."), alt options (suggested/written/guessed/[illegible]), typed input + "Use this" button, optional crop line box (hidden until loaded). Our candidate:
- Shows only "Open word-check tray" / "Tray: n to check" — no head, no nav, no options, no typed input, no crop box, no "Use all suggestions".
- No per-item alt buttons, no `kind` ("suggest"/"illegible") differentiation.

## Verdict

| Bar ref | Status |
|---|---|
| review.js 445-448 (streaming indicator) | PASS |
| review.js 455-458 (error box + retry) | PASS |
| paintReading 197-203 (underline) | PASS |
| review.js 479 (plain/formatted toggle) | PASS |
| words::items | FAIL |
| words::effective | FAIL |
| words::marked | FAIL |
| judge::heuristic/apply_found/rebuild/accept | FAIL |
| judge::toggle (undo/reapply) | FAIL |
| student::parse_reply | FAIL |
| groups::tally/name_of/key_of | FAIL |
| review.js 516-648 (tray UI) | FAIL |

Biggest remaining gap: **no persistent corrections (judge::toggle) and no effective transcript rebuild**. Without undo/reapply and effective rebuild, the tray cannot record or show changes — the word-check flow cannot complete.
