# Bar chart, critic 2, b8b3cbb (figures-gapfill, frozen at /tmp/figures-crit/bar)

Verdict: **FAIL** — 0 blocker, 2 MAJOR, 4 MED, 6 LOW, 6 minor. Evidence: external probe linked to the lib (`scratchpad/probe*.rs`), PNGs in `scratchpad/png{,2}`, mutants in `scratchpad/mutants.log`.

## BLOCKER
None. `cargo test --lib --no-run` builds. 6,000 random typed tables through parse/render/answer_pair: 0 panics.

## MAJOR
M1. The thousands fix bought the inverse ambiguity, silently. `A, 3,100` / `B, 2,050` draws ONE group, A=3100, B=2050, `notes=[]` (scratchpad/png2/inverse-key.png: a perfectly plausible chart). The FAQ's "second count after another comma draws two groups" rule makes this look like two groups; only the hint's "no space after it" resolves it the other way, and no note confirms the choice. `A 1 200` (space) is 1200 with no note and is not in the FAQ at all. Same silent-wrong-chart class as J1, counts merged instead of split.
M2. The mismatch advice still loses or invents counts after a thousands join, contradicting "never advises `, 0`". `A, 5, 7 / B, 1,200` → "change line 2 to “B, 1200, 0”" (the J1 test's `!note.contains(", 0")` covers only four datasets, none with a thousands line second); applying draws B 1200/0, no note. `A, 1,200 / B, 5, 7` → "change line 2 to “B, 5”"; applying drops the typed 7, leaving one group A=1200, B=5, no note. The 45-state table does compare final pairs, but no state has a thousands line in a two-count table and its `Red,1 / Blue,3,5` state bakes in dropping the count.

## MED
1. Headings whose group names hold digits become a data row: `Year, 2019, 2020` + `Walk, 12, 15` + `Car, 9, 11` draws a "Year" category 2,019/2,020 with no note. `Grade, Grade 9, Grade 10` is refused with "for example “Grade, 12”", turning the heading into data. Only name1/name2 can name such groups, and the hint/FAQ never say so.
2. The exported percent table contradicts itself: measure=percent, blank=The bars, two groups 11/8, 7/9, 3/2, 4/9 → cells 44.0, 28.6, 28.0, 32.1, 12.0, 7.1, 16.0, 32.1, but Class B's column sums 99.9% while its Total row prints 100%; the rounding note is editor-only.
3. The rounding note's fix search only goes upward (`(decimals+1)..=2`). 8,9,2,9 of 28 at Decimals=1 sums 99.9% and is told to abandon percents ("Choose Frequency (count)"), though Decimals=0 sums exactly 100 (probe: no note at 0). The claimed LOW-7 fix exists but is not the one offered.
4. M3 coverage: `join_thousands` pins only 1–2 digit leading groups. Mutant `j - i <= 3` → `<= 2` survives the whole thousands test, so the `150,000` form the FAQ's rule covers is unpinned. A mutant dropping `%` from the percent axis also survives.

## LOW
- `Walk, 12,5` (comma-delimited) is silently two groups 12/5; only `Walk; 12,5` is refused, although the test's comment says "a decimal comma is refused, never read as two groups".
- The thousands pass rewrites quoted names: `"Bus, 12,300 riders", 4` → category "Bus, 12300 riders", no note.
- A headings line with number-word group names breaks: `Group, None, One or more` → refused as data, advice "Group, 0, …".
- Critic-1 LOW 2 is still open: Pareto student with blank=The scale numbers keeps "14" in a white chip and the other labels high, clearing an invisible cumulative line (png2/pareto-student-scale).
- Print at 66%: the #d9d9d9 single-series fill and #cccccc gridlines are lighter than the test's 168 threshold, so bars survive only as outlines and the grid is gone (2x→0.33, LANCZOS, threshold 168; the figure's own test checks only bars and text, at 66% only).
- Pareto + Relative frequency draws two identical 0–100% axes (left value axis and right cumulative axis), no note.

## minor
Table's first column is always "Category" even with "Pet" on the axis; the measure control still says "Bar height" for horizontal bars; "Other" is not held last in Pareto; every legitimate headings line raises "read as headings, not as a category"; A–Z is code point (Æble after Zebra, Café folded only by case); `<title>`/`<desc>` are the generic figure title/blurb, not the typed title.

## verified (independent method)
- Rounding: Python Decimal ROUND_HALF_UP vs the Rust integer form, all c ≤ total < 60 at d=0,1,2 plus 200,000 random (c,total,d): 0 mismatches, including 40 exact .5 ties. Rendered 44.0/28.6/28.0/32.1/12.0/7.1/16.0/32.1 recomputed by hand from totals 25/28.
- Pareto: SVG parse gives line fractions 0.4242, 0.6970, 0.8485, 0.9394, 1.0000 = 14,23,28,31,33 of 33; right axis 0–100 over the same 260 px.
- Student copy: whole source has no polyline/circle, no comments/data-/Cumulative percent; its text set = key minus value labels, right axis and five answer strings, plus three "Answer:" prompts; identical canvas size.
- Print: every fill/stroke in 26 Print SVGs is grey or `url(#vs-hatch)`.
- Data fixes: 1e15 refused for the whole table with no "change line"; twelve/nine→12/9; "12 students"→12; inf/NaN/l2/1O refused at line 1; `Walk; 12,5` refused; fmt_count retyping round-trips; 31 categories refused with "Other, 3"; 45-state table really has 45 states and compares final pairs.

## mutants run (15, one filtered test each; 11 killed)
killed: percent floor, label 1 px, truncation >1, MAX_CATS 31, Q3 decimals→0, natural string compare, reversed tie, Pareto axis 1.02, heading digit check, heading inf/NaN check, Pareto axis drawn in student.
survived: join_thousands 3-digit limit (real coverage gap, M3), axis "%" dropped (coverage gap), `sort_unstable` (equivalent, as claimed), label 5.0→5.5 (the ±0.6 tolerance hides a 0.5 px drift).

## not checked
Full `cargo test --lib figures` / figuresui, live egui, census, PDF/DOCX export, fonts beyond Latin, CJK/RTL, 58/75% bitonal, 30-category two-group layout at real print size, and the 45-state table re-run (I used direct probes instead).

## biggest gap versus a textbook bar graph (OpenStax Intro Stats 2.1)
The default output is still graph-only. The textbook pairing (bar graph + frequency and relative-frequency table) exists only as the key of "Student copy leaves out: The bars"; its first column is always "Category" regardless of the headings, and its percent column does not add to its printed Total.
