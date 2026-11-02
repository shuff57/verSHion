# verSHion Grade Screen — P4 Critic Report

**Scope:** Compare verSHion's grade screen implementation against viSHion's reference bar for three functional pieces.

---

## Piece A: words::marked (inline `[illegible]` rendering)

### Bar (viSHion)
- `render.js` `R.unit` (line 42): `html = html.replace(/\[illegible\]/gi, '<span class="illegible">[illegible]</span>');`
- `[illegible]` replaced **inline** within each transcript `<div class="ln">` via markdown `renderFormatted` → markdownit → styled via CSS (`.illegible` class: red text + pink background).
- Also `W.wrap(wrap, makeWord(p))` at review.js:470 wraps flagged words in `<button class="w ...">` for tray interaction.
- Transcript renders **effective/corrected text** (`W.effective(p)`) with markdown formatting (`**bold**`, `~~strike~~`, `$...$` math, edited typed fixes) plus inline markers.
- Reading underline (`paintReading`) highlights the current `.ln[data-i]` line and scrolls it into view.

### Ours (verSHion)
- `draw_render_text` (gradeui.rs:193-237): renders `[illegible]` markers **inline within the same horizontal row** using `ui.horizontal` and `RichText::new("[illegible]")` with red color + pink background. **This specific inline rendering matches.**
- BUT: the transcript is **plain `ui.label` segments**, **not** effective/corrected text with markdown/KaTeX. The `eff_text` computed at line 196 is only shown in a label ("Effective text shown above — P4 rebuild pending") and the raw view hack (`eff.replace("[[]", " ").replace("]]]", "")`).
- No markdown rendering, no `~~strike~~`, no `$...$` math, no typed-edits applied via the `build()` algorithm.
- No clickable word buttons — no `<button class="w">` wrapping of flagged words.
- No reading underline — `state.read_line` is tracked but never painted; no `.ln[data-i]` class, no scroll-to.
- The "Show plain"/"Show formatted" toggle affects only the raw/eff display, not the transcript lines themselves.

### Verdict
- **FAIL** overall, but **PASS** for the specific inline `[illegible]` marker-on-same-line question.

### Single biggest remaining gap
The transcript renders **static plain labels** with **no effective/corrected text applied, no markdown formatting, no math, and no interactive word buttons**. The `[illegible]` inline styling is present but isolated from the rest of the viSHion experience — the transcript isn't the "marked" view the bar provides.

---

## Piece B: student::parse_reply (name auto-population)

### Bar (viSHion)
- `student.rs` `parse_reply(text)` (line 96): pure function that finds `{...}` JSON in the reader reply, parses `.name`, runs `clean()` (rejects null/none, length 2..=40, non-alpha, etc.).
- The **14% top-strip crop** (`STRIP = 0.14`) is measured from actual pixel line-bands of the scanned image (`js/image.js` line bands), **not** derived from transcript line count.
- Name is stored on the page object: `S.updatePage(id, { student: { name, source, confirmed } })`.
- Triggers on **both** page selection AND when the transcript becomes available (page:update event with raw text).

### Ours (verSHion)
- `draw_grade` (gradeui.rs:249-272): computes `lines_to_check` from pixel dimensions: `top_strip_px = img_h * STRIP`, then `line_pitch = img_h / n_lines`, then `lines_to_check = ceil(top_strip_px / line_pitch)`. This **derives** the 14% slice from transcript line count rather than measuring pixel bands.
- Calls `student::parse_reply` on the first `lines_to_check` lines — **calls clean()**, so the rejection logic (length 2..=40, non-alpha, etc.) matches.
- Persists `sub.student_name = name` and calls `self.store.save_student_submissions()` — **parity achieved.**
- Triggers **only on page selection** (when `state.page_id` is set and `student_name` is empty). **Does NOT trigger on transcript write.** There is no handler for page transcript updates in gradeui.rs.

### Verdict
- **FAIL** — the derived slice (from line count vs. measured bands) and missing transcript-write trigger are real gaps, even though the parse+clean and persistence logic are correct.

### Single biggest remaining gap
The 14% strip is **derived from transcript line count** (`img_h / n_lines`) rather than measured from **actual pixel line bands**, and the name population **does not fire when the transcript is written** (only on page selection). The measured-strip parity of viSHion is broken.

---

## Piece C: groups::tally/key_of/name_of (sidebar grouping)

### Bar (viSHion)
- `groups.rs` `name_of(p)` (line 71-79): returns `p.student_name()` if present and non-empty, else file stem.
- `groups.rs` `key_of(p)` (line 82-89): returns `format!("n:{}", collapsed_trimmed_name)` if student name present, else `format!("p:{}", p.id())`.
- `groups.rs` `tally(groups: &[Group])` (line 169-180): counts total, ready/check/failed/processing per group status enum (Failed > Processing > Queued/Waiting > Check > Ready).
- `groups.rs` `list(pages)` (line 118-151): groups pages by `key_of`, builds `Group { key, name, pages, open, words, failed, status, source }`, sorts by natural name order.
- Sidebar (`review.js` `renderWho`, line 396-413): shows per-group name + page count + pill status (failed/processing/queued/waiting/check/ready) + words-to-check count.
- The `who-head` shows `g.name` + meta (pages + words) + pill + change-student button.

### Ours (verSHion)
- `draw_grade` (gradeui.rs:303-318): computes a `key` string manually (`n:normalized_name` or `p:page_id`) — **partial key_of parity**.
- Shows page count and per-student name via `s.student_name` — **name display works**.
- **`let tally_ref = groups::Tally::default();`** (line 315) — a **default/empty `Tally` with all zeros**. **Never calls `groups::tally()` on real groups.**
- Displays `Tally: total=0 ready=0 check=0 failed=0 processing=0` — **stub behavior.**
- Does build `tally_words` counts (lines 294-300) showing per-student illegible+corrections word count, but this is a **different function** (`tally_words` in gradeui.rs) not `groups::tally`.
- No `Group` structs are constructed, no pages are grouped by key, no status computation happens.

### Verdict
- **FAIL** — the sidebar Tally is hardcoded zeros; groups are never built or tallied via the `groups` API.

### Single biggest remaining gap
The sidebar's group **Tally is `groups::Tally::default()` — all zeros**. Real grouping, key computation, status classification, and `groups::tally()` invocation are all missing. The sidebar shows only a stub.

---

## Summary Table

| Piece | Bar Reference | Ours | Verdict | Biggest Gap |
|------|---------------|------|---------|-------------|
| A | words::marked inline `[illegible]` | Inline red+highlight markers rendered, but transcript is plain labels with no effective text, no markdown, no word buttons, no reading underline | **FAIL** (PASS only on the narrow inline marker question) | Transcript renders static plain labels; no corrected text, no markdown/KaTeX, no interactive word buttons, no reading underline |
| B | student::parse_reply | Computes 14% slice from pixel dims, calls parse_reply+clean, persists via save_student_submissions, triggers on page selection only | **FAIL** | Strip derived from line-count approximation instead of measured pixel bands; no transcript-write trigger |
| C | groups::tally/key_of/name_of | Manual key computation, name shown, but Tally = `groups::Tally::default()` (zeros), no group building, no status | **FAIL** | Sidebar Tally hardcoded zeros; groups never built or tallied via groups API |

---

## Overall
All three pieces fall short of the viSHion bar. Piece A has the most visually matching narrow behavior (inline marker styling) but the transcript as a whole is a bare rendering. Piece B gets the parse+clean+persist logic right but breaks on strip measurement and trigger conditions. Piece C is a stub with zero-tally display. No piece wins against the bar.