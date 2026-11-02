# CRITIC P4-03 — GRADE SCREEN GAPS (words::marked / student::parse_reply / groups::tally+key_of)

Bar references: viSHion `review.js` 197-203, 438-484, 445-448, 455-458; `crates/core/src/words.rs` (W.marked);
`crates/core/src/student.rs:96-103` (parse_reply); `crates/core/src/groups.rs:71,82,169` (name_of/key_of/tally);
JS `js/groups.js:1-63`, `js/student.js:1-66`.

## Piece A — W.marked inline rendering parity

**FAIL (biggest gap):** Our builder rewrote `draw_render_text` to split lines at `[illegible]` and render
alternating segments, with illegible text red via RichText. The bar wraps the marker in
`<span class="illegible">[illegible]</span>` and CSS paints it `background: var(--bad-bg); color: var(--bad);` with underline.

**Where we diverge:**
1. **Layout:** The bar renders `[illegible]` *inline within the line's text flow* (a span in the same text node).
   Our split-and-render-as-separate-`ui.label()` produces a separate egui label per segment → each lands on its own
   line. Marker is NOT inline in the sentence. Biggest remaining gap: the transcript lines are broken into fragments.
2. **Background:** The bar applies a highlighter background (`--bad-bg`) + red (`--bad`) + dashed underline.
   Ours: red text only, no background, no underline. Highlighter background missing → visual parity fails.
3. **Wrap semantics:** Bar wraps each flagged word with class `w.illegible` for hover/tray selection (`.w .illegible`,
   `data-key`). Ours: no `.w`-style wrapper, no clickable word keys → tray sync broken for illegible.

**Praise:** The red color choice (`0xcc, 0x44, 0x44`) approximates `--bad`. At least we stopped using `<[illegible]>`.

## Piece B — student::parse_reply auto-population

**PASS / PARTIAL:** Builder now mutates `self.store.student_submissions` when a page is selected; student_name empty;
takes first ~14% of transcript lines; calls `vishion_core::student::parse_reply`; saves result.

**Where we diverge / gaps:**
1. The bar reads STRIP = 0.14 of the *page* (image height fraction), and the ASK strip. Our impl takes
   `transcript.lines().count()` * 0.14 * ceil — that is 14% of the *transcript line count*, not of a page. For a page whose
   transcript has many short lines, this lands wrong.
2. The bar's `parse_reply` extracts text between first `{` and last `}` then JSON-parses. Our call passes joined
   lines — should still work for JSON replies, but the 14% slice may not include the name line at all for typical transcripts.
3. Save path: we mutate the store but never call `save_student_submissions()`. Persistence not wired (P3 gap pattern).

**Biggest remaining gap:** 14% computed off transcript lines instead of page pixels; save not persisted.

## Piece C — groups::tally / key_of / name_of grouping

**FAIL (biggest gap):** Builder added group-key display (`n:<norm>` / `p:<id>`) and a stub Tally print.

**Where we diverge:**
1. The bar groups via `G.list()` → Map keyed on `G.keyOf(p)` (student-name normalized, fallback `p:<id>`), sorts by
   natural name order, shows group name + page count, `G.tally(list)` gives status counts, `G.of(id, list)` finds group.
   Our impl only prints a single group's key for the *selected student* — no actual grouping across submissions,
   no page counts, no per-group status, no `G.names()`.
2. We stubbed `groups::Tally::default()` (all zeros) instead of calling `groups::tally()` over real groups — the bar's
   tally has status logic (failed > processing > queued/waiting > check > ready). Ours is a dummy.

**Biggest remaining gap:** No real grouped sidebar list — no per-student group with page count; tally is a stub.

## Verdict

- words::marked: **FAIL** — inline-layout + background parity broken; markers rendered out-of-flow. Biggest gap: inline styling within line.
- student::parse_reply: **PARTIAL** — logic present, slice math + persistence missing. Biggest gap: 14% of wrong base + no save.
- groups::tally/key_of/name_of: **FAIL** — grouping UI + tally stubbed. Biggest gap: no grouped list with counts; tally dummy.

Cycle P4-03: **NOT DONE.** Builder implemented shape-of-parity stubs; critic found structural gaps (inline layout,
background highlight, correct slice base, real grouping, real tally) that the stub implementation misses.

Next: builder reworks `draw_render_text` for true inline rendering (single label with RichText runs if possible, else
manual galley; red + background + underline), fixes parse_reply slice to page-14% + persist save, and builds real
`groups::list/tally` grouped sidebar from `store.student_submissions`.
