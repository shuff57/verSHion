# frequency-table — blind critic 4 (7230f40, figures-gapfill)

Verdict: **FAIL — 0 blocker, 0 major, 1 MED, 4 LOW, 6 minor.** 30/30 module tests pass (1 ignored); `census_baseline`, `handle_coverage_report` and `every_figure_survives_extreme_numbers_through_the_load_path` pass. Probe: `scratchpad/probe` (registry → SVG with resvg ink boxes), 440-case count/label/relative sweep plus 300 boundary-heavy grouped sets, 22 dumped student/key pairs, 66 % raster pixels. **No image input: I did not look at any PNG.**

## BLOCKER
none
## MAJOR
none
## MED
1. **Tiny-range classes are counted against boundaries the table does not print.** Auto grouped, `1, 1.0000000000000002, 1.0000000000000004, 1.0000000000000007` prints `0.9999999999999996 to <1` f=0, `1 to <1.0000000000000007` f=**2**, `1.0000000000000007 to 1.0000000000000011` f=**2**; the printed rule gives 0,3,1 — `1.0000000000000004` is shown inside the middle class and counted in the top one. Same at 1e9: `1000000000, …0000001, …0000002, …0000004, …0000005` gives f 1,2,2 where the labels give 2,1,2. Cause: `class_index`'s `64*EPSILON*max(scale/width,|q|)` is 28–120 classes wide when the width is 2–2.25 ulps, so `q=0.5` (or anything in the upper half of a class) snaps to the next boundary; and the bins' edges are themselves rounded (`start + i*width`), so quotient and printed edges disagree even at tol 0. The round-4 test only asserts sum=n and rows>1 for these sets, so it passes. Fix: assign against the stored `bins[i].lo/hi` (tie right), or force an integer-ulp fallback width with tol<0.25; add a 4-value near-1 assertion.

## LOW
- A value within ~64 ulps *below* a boundary at a normal width is still snapped up: manual `start=0, width=1`, data `0, 0.9999999999999999, 1, 1.5` → `1 to 2` f=3; auto `0, 999999999999.99, 1000000000000` → the 1e12 class f=2 though 999999999999.99 prints below it. The header's "strict … stays in the class below" is false; the tolerance is ~7× the subtraction error it guards.
- Start advice still misdiagnoses a near-min start: `width=0.1, start=9.0`, data `8.99999999995, 9.1, 9.2, 9.3` → "Class width 0.1 makes more than 200 classes … Set Class width to 0.2 or more (the automatic width)" although width 0.1 makes two classes and the cause is the start. Applying 0.2 repeats the identical note; the start fix is suppressed because |9.0 − 8.99999999995| < 1e-9·width. Round-3 LOW 3 is fixed only for the −10000 variant.
- Small table text is 12.73 px vs Medium 13.30 (4 %); at 66 % it is exactly 8.40 px nominal, no headroom (measured glyph ink ≈4.9 px). Unchanged.
- The empty grouped table no longer draws the caption (verified in `empty_grouped-student.svg`), but nothing pins it: the mutant `m.mode == Grouped && !m.empty` → `m.mode == Grouped` survives all 30 tests.

## minor
- The tiny-range note also appears with auto off and a manual width (`width=0.01, start=1e-20` on 1e-20 data), where the automatic width is irrelevant.
- The tall-page note says "turn off a column to shorten it"; columns change width, not height (200 rows at Large is 7821 px).
- The refusal for `1,2345` offers "12345" as the one-number reading, not the decimal `1.2345`; following it silently changes the data.
- FAQ "the number of values at or below the top of that row" is false at an inner boundary (the value at the top is in the next class); the next sentence states the correct running sum.
- Print's header band is `#a0a0a0` (160 grey, measured): it disappears from a 50 %-threshold bitonal copy; the 66 % test only asserts luminance <190. The header's 1.6 px rules survive (solid 0 rows at 0.66).
- The round-4 manual 1e-20 boundary test bypasses `Params::normalize`; via JSON/UI a manual width below 0.01 clamps to 0.01, so that path is not reachable end-to-end.

## verified
Every count, label, cumulative, cumulative-relative, fraction, half-up percent and decimal matched an independent recomputation over 200 random manual grouped sets (widths 0.01–1000, negatives, boundary values, empty classes), 120 auto grouped sets and 120 raw sets; the only mismatches were the MED, LOW 1 and values I injected 1e-9–1e-15 of a width below a boundary. Boundary rule: `12.34..12.38` auto and manual → 1,1,1,1,1; a five-class table at 0.01; every value on a boundary; negatives (−2.02…); closed last class; totals n. `999999999999` stays below 1e12; `1e12+1` is reported as beyond the limit, not "not a number". 1e-20 → 3 classes with the relative-width note; 5e-13/1.5e-12 → 3. Fallback near-1/near-1e9 3-value sets keep 3 classes and n. Student copy: 22 pairs, no `fcell/rcell/ccell/crcell/tally` element, no key note, same viewBox, every answer column and the total row blank. Print: every pixel R=G=B; 0.66 row rules solid; all text ≥8.4 px nominal at 66 %. Layout: 15 renders including 120-char title, 40-char heading, 200 rows at Large, Print Small and 200 rows: no ink collision, off-canvas glyph or glyph-on-rule; min text 12.73/13.30/18.62 px. Examples, FAQ arm (6 entries), NO_HANDLES, census, purity and the 200-draw CPU test pass.

## mutants
14 tried, 13 killed: tol→0, widen `*10`→`*1`, relative step→absolute, fallback floor→1.0, too-big wording, student answers drawn, percent half-up→floor, 66 % floor removed, tally 5→4, tall-page note off, decimal-sum note off, start advice not named, last class opened. **Survivor: empty-grouped caption (LOW 4).**

## not checked
No PNG looked at (no image input), so no judgement of appearance or apparent readability; ink boxes use resvg's system fonts, not the app's bundled face. No full lib suite, no release timing, no real copier/PDF, no egui/browser. The histogram's rendered SVG was not inspected, only its `bins()`/`auto_width` outputs.

## biggest gap
No midpoint column, and classes are written "a to <b" rather than textbook stated limits with midpoints (40–49, boundaries 39.5–49.5); every class still needs hand-editing before it goes on an exam or an ogive.
