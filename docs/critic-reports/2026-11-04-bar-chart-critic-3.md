# Bar chart, critic 3, f8de30e (`figures-gapfill`, frozen at /tmp/figures-crit/bar3)

Verdict: **FAIL** — 0 blocker, 1 major, 4 MED, 5 LOW, 6 minor. External probe linked against `libvershion` (scratchpad/probe); no image input.

## BLOCKER
None. 5,000 random tables through notes/render: no panic; 10,000 rows refused in 58 ms; 30-cat/40-char canvases stay bounded (1921×534, 640×1951).

## MAJOR
M1. The >2-count refusal still rewrites away a typed count, the class MAJOR 2 claims closed. `A, 5, 7, 3 / B, 1, 2` → “Line 1 has 3 counts; … Change line 1 to “A, 5, 7”.” Applying it draws A 5/7 and silently drops the 3. Same for `A, 9, 12,5` → “A, 9, 12” (drops the 5) and `B, 3, 4, 9` → “B, 3, 4”. “No message anywhere … drops a count” is false.

## MED
M2. The rounding fix cannot settle on two-group tables; it tests one group at a time. `Pet, Class A, Class B / Dog, 8, 1 / Cat, 9, 2 / Fish, 2, 3 / None, 9, 2` at percent, Decimals 1 → “Class A … 99.9% … Set Decimals to 0”; at 0 → “Class B … 101% … Set Decimals to 1”. No setting makes both groups 100, the COUNT fallback is never offered, and applying the note flips it forever.
M3. The Print grid still dies in an honest 66% copy, contrary to the claim. #999999 at 1.4 px is 0.92 px at 66%: sampled between bars the gridline minima are 175/184/191 (58%: 190–204), all > 168; at 75% two of three are 183/191. The test renders at 1.32× and samples one un-downsampled centre, so it cannot see this. Bars do survive (interiors 100% black at 58/66/75%; hatch 25–28% at 66%).
M4. MED 1's heading classifier still mishandles common headings. `Grade, 9th grade, 10th grade` is refused as data (“9th grade” is not a number … for example “Grade, 12”), so following the fix turns the heading into a category. `Group, None` (axis + one group named None) is “fixed” to `Group, 0`, drawing a zero bar the teacher never typed; `Response, None, One or more` is refused with “cells hold words with numbers in them” though neither cell has a digit. Number words are not numbers in a heading.
M5. The key now contradicts itself on percents. `A, 1 / B, 1 / C, 1`, percent, blank=The bars, questions on: the key's own table prints A=34% (largest remainder) while question 3 answers “33%” (half-up) in the same SVG; likewise 32.2% vs 32.1% for Class B Cat in the critic dataset. Two rounding schemes, no note explains which answer is expected.

## LOW
- `A, 1,200 / B, 950` note: “Two counts would be “A, 1, 200”, with a space after the comma on every line.” B has no second count; following it invents one or triggers the mismatch refusal. The primary “change line 1 to “A, 1200”” works.
- `A 1 200` note cites the comma rule (“a comma with no space after it…”) for a line with no comma; the FAQ still documents only `1,200`, never `1 200`.
- Reverse mismatch `A, 1,200 / B, 5, 7` never names A's second reading (1 and 200), the respelling that resolves it; alt_hint only inspects the offending line.
- Student Pareto with blank=scale numbers still has a 19.7×19.75 white chip inside the first bar at the hidden cumulative line's level (0 polylines), so label placement telegraphs the line.
- Numeric headings outside 1900–2100 (`Week, 1, 2`) are silently data. `Year, 2019, 2020` is drawn as a 2,019/2,020 category; the editor note is the only signal and is absent from the SVG; the heading note also says “has no numbers” for `Pets 2024, Boys, Girls`.

## minor
First table column is always “Category” while the axis says “Pet”; every headings line raises the “read as headings” note; the table Total prints “100%” beside “44.0%” cells; `<title>/<desc>` are generic; “Bar height” labels the measure under Horizontal; “Other” is not held last in Pareto.

## verified (independent method)
- Half-up labels: 6 datasets × d=0,1,2 exact (13/88, 12.5/87.5, 25.93/33.33/40.74; 1e9−1 of 1e9 → 100%/0%).
- Pareto: parsed line fractions 0.42424…1.00000 = 14,23,28,31,33 of 33; right axis 0–100 over the same 260 px; percent mode has one vertical axis plus the polyline.
- Gaps measured 0.25/0.40/0.60 of a slot; two-group bars touch within a category; B(4,9) sorts before A(10,0); A–Z apple<Eclair<Item 2<Item 10<Zebra.
- Q1/Q2/Q3 recomputed: 3-way tie, total 1,234,570, 11/21=52.38%, 8/19=42.11%; student Pareto-percent source has no polyline/circle/comment/data-/Cumulative percent/value labels and is 640×351 like the key.
- Percent table: 81 random columns (1–2 groups, 2–6 cats, d=0–2) sum exactly 100.0; 11/8,7/9,3/2,4/9 at d1 = 44.0/28.0/12.0/16.0 and 28.6/32.2/7.1/32.1 with Total 100%.
- Print: all fills/strokes grey; first series solid #999999; grid #999999 at 1.4 px; bars survive the honest 66% copy.
- Ambiguity matrix: silent for tab, 3-cell, two-count-with-thousands-in-both-columns, quoted names and semicolons; fires for `1,200`, `1 200`, `A 1 200`, `12,000`; fixes keep the parsed pairs.
- 150,000 and 1e9 parse/print; comma-decimal noted; 30 cats × 40-char names keep all text inside the canvas.
- Fuzz: 5,000 random tables/options, no panic, max 726×729, 0.5 s; `census_baseline` passes (defaults/start unchanged).

## mutants (debug, one pinning test each)
14 run. Killed 11: half-up, cumulative-from-first-bar, `%` tick, 150,000 join, table total, MAX_CATS 31, 0.5 truncation, 5 px label gap, Q3 decimals, Print fill, repeated-name sum. Survivors 3: equal-value ambiguity suppression (`A, 0,000` stays silent although the second reading is a different shape); “Wide” gap value (0.9 passes — only ordering and gap≥20% are asserted); largest-remainder tie order (later-category wins passes — the test checks the multiset of printed percents, not which row gets the extra 0.1). The first merge mutant (`*a = b`) did not compile and was rerun as `*a = *b` (killed).

## not checked
Visual/PNG appearance (no image input; the visual pass is separate); 58/75% bar-outline survival beyond the grid measurement; live egui, fonts beyond Latin, CJK/RTL, PDF/DOCX, full suites.

## biggest gap versus a textbook bar graph
Default and key output are still graph-only. The textbook pairing (OpenStax Intro Stats 2.1: bar graph + frequency and relative-frequency table) exists only in the student “The bars” copy; its first column is always “Category” and the key never prints the table.
