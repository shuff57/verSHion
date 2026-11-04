# Ogive, critic 2, commit 7384c3f

Verdict: **FAIL. 0 blocker, 2 major, 5 MED, 6 LOW, 4 minor.** Probe scripts, ~7k SVGs and logs in `scratchpad/`; labels/geometry parsed from the SVG, every printed number recomputed with Python `Fraction` + ROUND_HALF_UP.

## MAJOR
1. **Census baseline not blessed, so `cargo test --lib figures` is red.** `cargo test --lib census_baseline` -> `changed: ogive__defaults, ogive__start` (also two photosynthesis ids, pre-existing). Current FNV-1a: `ad637a01d7cd338a` / `41570b24cc61dfe2`; fixture still `f122f5892b507abe` / `39b076bdfeb0d79d`. 7384c3f touches only 3 files, no fixture. The loop requires blessing an intended render change.
2. **A refusal's advice loops at the 1e12 cell limit.** `0, 500, 3 / 1000, 2000, 5 / 1000000000000, 2000000000000, 1` says "for example \"1000000000000, 1000000001000, 1\""; typing that returns the identical refusal (upper 1.000000001e12 > MAX_ABS 1e12), forever. `cell_number` silently rejects >1e12 and no note names the limit, so the teacher cannot fix the table.

## MED
1. **Stated limits with gap > 1 are converted wrongly and the rule misreads a missing class.** `0, 10, 3 / 20, 30, 4` -> "take 5 off every lower limit and add 5 to every upper limit" -> `-5, 15 / 15, 35`: width 20 not 10, total at 35 not 30. `40,50 / 52,60` -> `39,51 / 51,61`; the canonical one-missing-class table is changed, not padded. `width >= gap` fires for any equal gap; the arithmetic only holds when the gap is the limit unit (1 or 0.1).
2. **The "no room" note's fix deletes the guide it names.** Default table, P49 + quartiles, Medium: note "the guide line for M has no room ... To label it, set Mark to None"; after the step there is no M guide at all (1 guide, the P49). In my sweeps 199 of 222 notes offer `set Mark to None/Median` (removes M/Q1/Q3); only the 23 label-size steps add a label. The test passes because `draw(q).1.is_empty()` is vacuous once the named reading is gone.
3. **The leader fallback misattributes labels.** Unequal table, value 13, quartiles, Large: label "6.7 (16.8%)" is 131 px from its own guide and 25 px from the median's; its 125 px leader crosses the Q1 and median guides. Over 3,465 keys: 547 leaders, 224 cross a rival guide, 62 are >60 px, max 125 px (default P16 Large: 52 px, own 58.5 px vs Q1 29.7 px). The new "label is nearest its own line" invariant is waived for exactly these.
4. **The accepted range contradicts the plateau note.** Flat table P60: note "from 15 to 20 ... every value there fits. The figure reads the lowest, 15"; answer "about 15.0 (accept 14.2 to 15.8)", so a student who reads 18 (valid per the note) is outside the key's range.
5. **Advice keeps the numbers but not the data.** `40 -49, 3` -> "-49, 40, 3" (class becomes -49..40); `40- 49, 3` -> "39, 49, 3" (lower boundary moved); `1,000, 2,000, 5` (spreadsheet paste) -> "1, 0, 2". All ordinary near-misses of the claimed formats; applying the example changes what is counted.

## LOW
- `Percentile of a value` answers have no accepted range ("about 29.5 of the 50 values, so about 59.0%"), unlike percentile/quartile answers.
- Value at the first boundary (40): the guide is a 15 px dash on top of the x-axis (zero-height vertical), label "0.0 (0.0%)" (critic-1 LOW 6).
- Print fine grid `#a4a4a4` is 164, only 4/255 below 66 % (168); the 66 % test renders at 3x. At 1x and 0.66x both grid kinds rasterise 182-210, i.e. paler than the threshold.
- `40 − 50, 3` (U+2212 separator) -> advice "40, 50, 50": frequency 3 becomes 50 (U+2212 accepted in numbers, not between them).
- Header with a number, `Class 1, Frequency`, -> "1, 11, 1", adding a bogus class before the data.
- Quartile/percentile side labels drop their name when the full text fits: "1.25 = 25% of 5" on the Q1 guide, not "Q1"; "20 = 40% of 50", not P40.

## minor
200 classes -> 201 x-tick marks 2.6 px apart with 17 labels (a comb); "about 29.5 of the 50 values" (fractional count); x title "Test score" for raw data; `plain()` rounds advice boundaries to 6 dp.

## verified (independent method)
- **MED 1:** 1,485 single-reading keys (5 datasets x P1-P99 x 3 sizes) + 297 value readings: every guide has a label, 0 short-only. For 222 crowded keys every unlabelled distinct guide has a note; the 23 label-size steps label every remaining guide.
- **MED 2:** 3,135 keys through Python fractions, decimals 0-4: 12,375 printed percentile/quartile/IQR values and 58 cumulative cells, 0 disagreements; 12.5->13, 8.85->8.9, 1.625->1.63, Q1 62/Q3 81/IQR 19; guide x within 0.3 % of the exact span.
- **MED 3:** 0-1/2-3 -> -0.5/1.5/3.5/5.5/7.5; gap 0.1 and width>gap correct (gap>1 is MED 1).
- **MED 4:** `40 - 49`, `40 to 50`, `40–50`, `40—50`, `40 through 50` parse; `Total, 50` -> delete line 3; the even midpoint table settles to 40-50/50-60/60-70.
- **MED 5:** default has 25 horizontal (5 labelled, 20 fine) and 6 vertical grid lines; grid elements precede curve/guides/plates; 0 grid-through-text without a plate; Print 140/164 < 168; all 12,375 ranges contain the exact value (widest 32.5 % of a class).
- FAQ names OpenStax's (n+1) convention and says the figure uses p/100 x n. 4,400+ student/key pairs: no answer/guide/tcum/leader class, no answer body, no "about"/"accept", same viewBox and element list. ~6,700 renders plus 10,000-row table/raw: no panic; 200 draws+notes 3.13 s CPU (debug).

## mutants
13 one-line mutants: 10 killed (units >=, IQR printed difference, stated `>=` -> `>`, grid grey, fine-line count, leader loop, total-line number, cum_text float, note find->next, unlabelled push). Survivors: midpoint two-column branch removed (tests still green; only a 5-spaced midpoint table shows it), vertical grid ignores `grid=false` (no test sets grid off; real code is correct), `read_tol` clamp removed (no test checks range width).

## not checked
No image input, so no visual pass (done separately); live egui/browser fonts/PDF; the full lib suite. Of the 23 active ogive tests, 21 ran green; the two 60 s+ resvg ink audits were still running when killed at ~13 min (no failure shown). `census_baseline` was run and failed as above.

## biggest gap versus a textbook ogive
No fine vertical grid: the x-axis has class boundaries only, yet the accepted range assumes five x-subdivisions per class, so the one-decimal keyed readings are finer than the graph supports; in crowded keys the long leaders then tie labels to the wrong guide.
