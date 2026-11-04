# Bar chart, critic 4, f31433c (`figures-gapfill`, frozen at /tmp/figures-crit/bar4)

Verdict: **FAIL** — 0 blocker, 1 major, 2 MED, 5 LOW, 4 minor. External probe (scratchpad/probe) over `render`/`notes_for`/`answer_pair`; no image input.

## BLOCKER
None. 3,000 random tables × options: no panic; largest canvas 640×658 in that fuzz, 1007×601 for 30 categories, 200 worst-case renders in 0.49 s wall.

## MAJOR
M1. The "no rewrite drops a count" claim is false outside the >2 branch: a name with an unbalanced `"` makes the comma a quote character, so the line reads as having **no count** and the offered fix turns the typed count into part of the name and invents 0.
`Don"t know, 5 / B, 4` → “Line 1 (“Don"t know, 5”) has no count… change line 1 to “"Dont know, 5", 0”.” Applying it draws category `Dont know, 5` with count 0: the 5 is dropped and the quote deleted. Same for `55" TV, 12` (TV sizes) and `a"b, 3`. The >2 cases asked for (`A, 5, 7, 3`, `A, 9, 12,5`, `B, 3, 4, 9`) are correctly refused with every count quoted and no rewrite; this branch was not covered by the claim or the tests.

## MED
M2 (round 3's MED 3 does not generalise). At **66 %** the Print grid still dies for one line of six on a 30-category Print chart (bars hidden, 1× render + Lanczos, threshold 168): y=171.4 is min 176, median 178, **0/908 pixels below 168**; the other five are 157–163. Gridline subpixel alignment decides it, and 2.4 px is 1.58 px at 66 %, so a line landing on a half pixel disappears. The test uses the 4-bar default and `worst = min` over *all* gridlines, so one vanished line is invisible to it. My 4-bar numbers reproduce the claim (157–158 at 66 %), so the fix is real but not robust.
M3. The heading classifier still swallows a real data row. `A, １２` (full-width digits), `A, —`, `A, N/A`, `A, dozen`, `Smith; Jones, 3` → “read as headings, not as a category”, the row is gone from the chart (the rest is drawn), and only the editor note says so. Any count the reader cannot recognise becomes a heading; with 2+ rows there is no refusal.

## LOW
- Heading cells that are ranges, ordinals or codes are still refused as mistyped counts, with advice that does not work (`Age, 0-9, 10-19` → ““0-9” is not a number… change line 1 to “Age, 12””, which then fails on `10-19`); same for `Attempt, 1st, 2nd`, `Illness, COVID-19, Flu`, `Quarter, Q1, Q2`. The delete-and-name-the-groups advice exists only in the year note.
- `Year, 2019, 2020` is still drawn as the category `Year` with bars 2,001/2,002 (editor note only) — carried from critic 3.
- Print tone: bar fill `#999999` and grid `#9e9e9e` are 4–5/255 apart; at 168 both go black, so only the 1.4 px outline separates a bar from the 2.4 px gridline (the grid is the heavier mark).
- TooBig advice (“count in millions”) does not bring a count above ~1e15 under the cap: `A, 9999999999999999999999` → “count in millions” still refuses.
- The >2 message quotes the line verbatim but normalises a thousands count: `A, 1,200, 3, 4` → counts `“1200”, “3” and “4”` (“1200” is not what was typed).

## minor
First table column always “Category” while the axis says “Pet”; every correct headings line raises the “read as headings” note; `<title>/<desc>` name the figure “Bar Chart”, not the typed title; “Other” is sorted by size, not held last in Pareto.

## verified (independent)
- >2 counts: all three cases refused, every count named, no rewrite, no `, 0`; 4 and 10-count lines too, wrapped without clipping (5–6 lines, canvas 137/157 px).
- Half-up from integers: 6 datasets × d=0,1,2; labels, table cells and the q3 answer agree exactly (e.g. 3/19→15.8, 9/23→39, 1/3→33.3).
- Percent table Total = sum of its cells (99.9 / 100.0 at d1; 99 % for thirds); 87/87 suggested Decimals settings across 120 random tables clear the note; 32 two-group tables correctly say no setting works.
- Gaps 0.250/0.400/0.600 of a slot; ties keep typed order; A–Z natural; two-group percent sort by c0·T1+c1·T0 (B>A>C on 10,0 / 4,9 / 1,1).
- Pareto: points 14,23,28,31,33 of 33 at bar centres, right axis 0–100 % over the same 260 px; Q1 ties, totals, q3 recomputed.
- Thousands: `1,200`, `1 200`, `12 345 678`, `1,000,000,000` round-trip through the printed labels; 1e9+1 refused.
- Student copy: 48 states same canvas as the key, no polyline/circle/white chip, no answers, and the value/bar/tick leak is exactly the selected blank; Pareto student 640×351.
- Print: every fill/stroke grey; 4-bar grid 157–158 at 66 %; bars 153 with black outline.
- Census: bar-chart `__defaults` aee0af800ed41b11 and `__start` 208a340577ea2008 recomputed via FNV-1a over the annotated SVG — both match `tests/fixtures/figure_census.txt`; `census_baseline` passes.
- The module's own 36 tests pass (including the collision audit, fuzz, CPU, Print, student-leak, table/label/answer tests), so every finding below is a gap the suite does not pin.

## mutants (release/debug, scratchpad/mut copy, one pinning test each)
15 run, 14 killed, 1 survivor. Killed: half-up→floor; table Total forced to 100 %; Print grid 2.4→1.4 px; Print bar #999999→#d9d9d9; >2-count refusal rewritten to two; count mismatch silenced; “12 students” made a heading; rounding note checking group 0 only; student Pareto chip restored (killed by `pareto_right_axis_ticks…`, not by the student-leak test); percent two-group sort by plain sum (killed by `equal_bars…`); thousands groups not joined; MAX_CATS 30→31; percent ticks losing `%`; student keeping tick numbers. **Survivor:** “Wide” gap 0.6→0.9 (the bars test only asserts gap ≥ 20 % of a bar, so any large fraction passes); the exact Wide fraction is unpinned.

## not checked
Visual/PNG appearance (no image input: I measured pixels and boxes, I did not see a rendered figure); live egui and non-Latin font rendering (CJK/RTL); PDF/DOCX export; other figures' census ids and the rest of the lib suite.

## biggest gap versus a textbook bar graph
Unchanged from critic 3: the key is graph-only. The frequency + relative-frequency table (OpenStax Intro Stats 2.1 pairs both) appears only in the student “The bars” copy; its first column still says “Category” whatever the axis is.
