# frequency-table — blind critic 2 (ca043f8, figures-gapfill)

Verdict: **FAIL — 0 blocker, 0 major, 2 MED, 5 LOW, 4 minor.** 23/23 module tests pass (1 ignored); `census_baseline` green with ca043f8's blessed `a8007b8d…`; 10/12 mutants killed, 2 survivors (test gaps, below). Probe: `scratchpad/probe`, a crate against the frozen lib: 118 key+student SVG pairs, every count, label, fraction, percent, decimal, cumulative value and total recomputed in Python from the typed data. **No image input: I did not look at any PNG.**

## MED
1. **The new duplicate-boundary fallback retries the same width, so it can never succeed.** After `boundary_texts` fails, `grouped()` (frequency.rs:673-685) sets `width = super::auto_width(values)` — the identical width whenever `auto_classes` is on, which is the default. Repro: `mode=Grouped classes`, `data=1.0000000000000002, 1.0000000000000004, 1.0000000000000007` (10 or 100 copies) → no table; note "Grouped classes could not be made … Turn on Choose the class width automatically", advice already in effect. `suggest_width()` sits unused there and returns 0.01 (one class), so a one-line fix exists; the claimed "fallback" falls nowhere.
2. **Tiny ranges get ~10× too many classes.** `axes::nice_step_with` compares `raw <= f*mag + 1e-12` with an absolute epsilon; below ~1e-11 the `1*mag` branch always wins, so the width is a decade too small. `data=1e-20, 2e-20, 3e-20` (the round-2 test's own case) → **21 classes, width 1e-21, 18 empty rows**; `0.0000000000005, 0.0000000000015` → 11 rows for 2 values. Counts and labels are right; the table is absurd. Shared with the histogram (same helper), so fixing axes.rs moves the histogram census, or the table must widen.

## LOW
- **A value below a huge boundary is counted in the class above it:** `0, 999999999999` → `1000000000000 to 1500000000000` gets the 1e12 class with f=1 while `500000000000 to <1000000000000` gets 0 (`bins()`'s `*1e9` snap is ±5e-10 of the width).
- **FAQ "can add to 99% or 101%" is too narrow:** 15 distinct values at the 1-decimal default print 6.7% × 15 = **100.5%**; the `i, i%3` pair set (200 rows, 400 values) prints **109.9%**. The key note is honest; the FAQ is not.
- **FAQ tally reason is stale:** "Counts above 25 are not tallied (the column would be wider than the page)" — the column is now a fixed 25-stroke width (97.93 px); 26 is blank because it would exceed that width, not the page.
- **"OpenStax grouped table" example reuses the exercise-minutes data under "Heights of students"/"Height (cm)": 52–93 cm**, and its five counts (2,5,8,4,1) are not OpenStax's heights.
- **No note when Decimal cells miss 1.00:** `1, 2, 3` at 2 dp shows 0.33 + 0.33 + 0.33 = 0.99 with only the Total 1.00; Percent gets a key note, Decimal does not.

## minor
- The student page is 2–5 note lines shorter than the key whenever the key carries an answer note (8/118 pairs); width and every column rule match, nothing leaks, but "same page size" holds only for note-free states — exactly the cases the student test tries.
- `Small` is only 4% smaller than `Medium` for table text (12.73 vs 13.30 px) because the 8.4 px floor binds; only the title/notes shrink visibly.
- Tally-cap advice says "Choose Off for Tally column" — a choice value for a checkbox.
- The student `<desc>` still says the table has tally, frequency, relative, cumulative and cumulative-relative values when all are blank.

## verified (all recomputed independently)
Raw counts/labels/cum/rel/crel over 40+ sets (negatives, 0/−0, decimals, 1e−320, 1e12, 200 distinct, 400 values); half-up percent and decimal from exact integers at 0/1/2 dp incl. ties (1/8→13%, 1/16→6.3%, 1/400→0%); fractions reduced; totals n, 1/1.00/100%; grouped classes = the histogram's helpers, counts sum to n, boundary goes right, last class closed, empty classes kept; tally width 97.93 px identical for counts 1,2,10,11,12,13,20,25 and both copies, 26 blank with note; `1,000`, `1, 000`, `12,5`, `1,0000`, `−1,000`; 5001 values on one line → 5000 + note; `99.9%%` gone; Small 12.73 px → 8.40 px at 66%, Print 0 non-grey paints, band #a0a0a0 (lum 160) at 66%; `1e−20` shortest; width 0.01 → "1 or more", 5 classes; >200-distinct advice; Decimal choice, default-on cumulative relative column, its arithmetic/unit/totals/student blanks. Student copy: no answer class, stroke or text in 118 pairs. Empty/refusals/hostile input no panic; purity, CPU, registration, FAQ arm, NO_HANDLES, census pass.

## mutants (12 one-line, each vs its pinning test)
Killed: percent half-up→floor, unreduced fractions, tally width from the count, `%%` in the note, tally cap, 4-digit thousands, student answers drawn, one-line cap removed, 66% floor removed, cum_rel from row not running. **Survivors: (a) `let start = 6`** in `boundary_texts` passes `grouped_boundaries_print…` — the loop still climbs to distinct labels, so the test pins distinctness but not "precision chosen from the width" nor the width printed in the note; **(b) `cum_rel` default true→false** passes all 23 tests — the default-on claim is unpinned.

## not checked
No PNG looked at; no browser/egui/live panel, no real 66% copier or PDF/DOCX; full lib suite not run (per instructions); the histogram's rendered SVG not inspected (helpers and inputs compared only); no release-mode timing (debug bound only); the tally's visual style not judged.

## biggest gap
Intervals print as `a to <b` (a literal `<` inside a table cell) instead of textbook class limits or intervals, and there is no midpoint column, so the output still needs hand-editing for an exam; the 21-row tiny-range table (MED 2) is the worst symptom.
