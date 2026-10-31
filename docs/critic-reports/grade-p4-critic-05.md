# CRITIC P4-05 — GRADE SCREEN GAPS RE-EVALUATION

Bar references: viSHion `review.js` paintReading 197-203 / renderText 438-484 / renderTray 516-648; `crates/core/src/words.rs` (W.effective/W.marked/W.items/W.choose); `crates/core/src/student.rs:96-103` (parse_reply, STRIP); `crates/core/src/groups.rs:71,82,169` (name_of/key_of/tally).

VerSHion impl read: `src/gradeui.rs` (615 lines, build passes `cargo check` clean).

---

## Piece A — words::marked / W.effective / W.items inline rendering

### Bar (viSHion)
- Transcript renders **effective/corrected text** (`W.effective(p)`) with `~~strike~~`, `$...$` math, typed fixes, bold edits.
- Flagged words wrapped in `<button class="w">` with `data-key` for tray interaction (`W.wrap`).
- `W.marked` applies inline styling to flagged words within line flow.
- Reading underline (`paintReading`) highlights current `.ln[data-i]` line + scroll.

### Ours (verSHion)
- `draw_render_text` (line 325): `[illegible]` markers rendered inline within same horizontal row via `ui.horizontal` + `RichText` (red text + pink background). **Inline styling itself matches.**
- `eff_text()` (line 54) **computed but never used** — dead-code warning. No corrected/effective text shown in transcript.
- No `~~strike~~`, no `$...$` math, no typed-edits, no markdown.
- No clickable word buttons — `word_items()` exists but `draw_tray()` only shows count (no word selection UI).
- `state.read_line` tracked but **never painted** — no reading underline, no `.ln[data-i]`, no scroll-to-line.

### Verdict
**FAIL overall / PARTIAL inline.** Biggest gap: transcript is static plain labels — no effective/corrected text applied, no interactive word buttons, no reading underline painted.

---

## Piece B — student::parse_reply

### Bar (viSHion)
- Reads top strip by **pixel height** (`STRIP = 0.14` of page/image height), not transcript lines.
- Extracts text between `{}` braces, parses JSON reply.
- Saves persistently; triggers when transcript is written, not just on page selection.

### Ours (verSHion)
- 14% strip computed from `image.height * STRIP / line_pitch` where line pitch derived from image dimensions — **correct pixel-based approach.**
- `student::parse_reply(&first_lines)` called on page selection + on transcript write path.
- `save_student_submissions()` called every time `student_name` set — **persistence wired.**

### Verdict
**PARTIAL PASS / GAP.** Strip math now correct (pixel-based) and persistence saved. Remaining: verify `parse_reply` triggers reliably on every transcript write (not just selection edge cases). Readability: currently gated behind `sub.student_name.is_empty()` so works once per page — matches bar intent.

---

## Piece C — groups::tally / key_of / name_of grouping

### Bar (viSHion)
- `groups::list()` → Map keyed on `groups::key_of(p)` (normalized student name, fallback `p:<id>`).
- Sorted by natural name order; shows group name + page count.
- `groups::tally(list)` gives status counts (failed > processing > queued/waiting > check > ready).
- `groups::of(id, list)` finds group.

### Ours (verSHion)
- `build_groups()` (line 168): real grouping from `store.student_submissions`, keys via `group_key_of()`, sorted by name, per-group page count + status.
- `tally_groups()` (line 155): computes real `groups::Tally` counts (total/ready/check/failed/processing).
- Sidebar (line 509): shows group key + page count + tally counts — **real grouped list.**
- Does **not** call `groups::key_of()`/`groups::name_of()`/`groups::tally()` from `vishion_core` directly — uses local mirrors (`group_key_of`, `tally_groups`, `VerStatus`). Functionally equivalent but not viSHion's API.

### Verdict
**PARTIAL PASS.** Real grouped sidebar with tally counts works. Gap: local mirrors instead of calling `vishion_core::groups` API directly (parity gap, not functional).

---

## Overall verdict
- **Piece A: FAIL** — no effective text, no word buttons, no reading underline.
- **Piece B: PARTIAL PASS** — pixel strip + persistence wired.
- **Piece C: PARTIAL PASS** — real grouped sidebar + tally.

No piece fully passes the bar blind vs viSHion. Remaining gaps: A (effective text + interactive words + reading underline), C (viSHion groups API direct calls).

**Code builds clean.** `cargo test` not yet run for grade code.
