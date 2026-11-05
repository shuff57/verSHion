# frequency-table — blind critic 3 (75d3306, figures-gapfill)

Verdict: **FAIL — 0 blocker, 0 major, 1 MED, 5 LOW, 4 minor.** 28/29 module tests pass (1 ignored); `census_baseline`, `handle_coverage_report` and the load-path extreme-number test pass. Probe: `scratchpad/probe` (registry → SVG), 331 renders; 205 grouped and 39 raw tables recomputed independently in Python. **No image input: I did not look at any PNG.**

## BLOCKER
none
## MAJOR
none
## MED
1. **A value exactly on a class boundary is counted in the class below when `start` and `width` cancel — ordinary 2-decimal data.** `mode=Grouped classes`, `data=12.34, 12.35, 12.36, 12.37, 12.38` prints `12.34 to <12.35` f=**2**, `12.35 to <12.36` f=1, `12.36 to <12.37` f=1, `12.37 to <12.38` f=**0**, `12.38 to 12.39` f=1: 12.35 and 12.37 sit in classes that print them as excluded. Manual `width=0.01, start=12.34` on `12.34, 12.35, 12.36` gives f=2,1,0. `class_index`'s tol `1e-14*|q|` is smaller than the ~1e-13 cancellation error in `(v-start)/width`, so it floors left. The histogram's own `bins()` (1e-9 snap) gives 1,1,1,1,1, so round 3 fixed the 1e12 case by breaking the boundary rule and the "same classes as the histogram" promise for normal data.

## LOW
- Tiny ranges diverge from the histogram (`1e-20, 2e-20, 3e-20` → 3 classes here, 21 in the histogram); the header says so, but the user-facing FAQ still claims "the automatic class width is the histogram's own choice for the same values", and no note appears when it happens.
- The fallback's first workable width is the 0.01 control floor, so near-1 / near-1e9 data becomes one class (`1 to 1.01`, f=30). The note is honest, but the distribution is lost; widths below 0.01 are skipped.
- Control-reachable `class_start=-10000, class_width=10` prints "Set Class width to 10 or more … the classes now use 10" when the width is already 10; the real cause is the start silently moving to 40, which the note never names and which following "10" cannot clear. The same hardcoded "makes more than 200 classes" text appears when a start 1e-10 above the minimum drops a value.
- `1000000000000, 1000000000001` → n=1 with "1 cell that is not a number was skipped" (`data::MAX_ABS` 1e12); the range is named only when nothing usable remains. Shared data-layer wording.
- Small table text is 12.73 px vs Medium 13.30 (4 %): the 66 % floor binds, landing exactly on 8.40 px; smallest glyph ink is 7.45 px at 1×, 4.92 px at 0.66.

## minor
- "Class width (0) is not a number above 0" reads awkwardly; the fallback repeats the number ("…0.01; the classes now use 0.01").
- An empty grouped box draws the interval caption above the instruction.
- 200 rows at Large make a 7,745 px page (≈80 in at 96 dpi) with no note; nothing clips, but it will not fit one printed page.

## verified
Every frequency/relative/cumulative/cumulative-relative cell, fraction, half-up percent and decimal, total and `n` for negatives, −0, 1e−320, 1e12, 200/400 values, duplicates, empty classes, one-row tables; classes contiguous, last closed and covering max; 129 auto grouped sets byte-equal to the histogram's bins (1e12 differs by design); the near-1 / near-1e9 fallback names 0.01, counts 30, never advises the switch while auto is on; 1e−20 → 3 rows, 5e−13/1.5e−12 → 3; billion value below 1e12; thousands/`12,5` notes; 201-distinct and no-number refusals; six examples and six FAQ entries match the claims; student copy: 0 answer texts in 146 pairs, no answer classes/strokes, key and student `viewBox` identical everywhere; Print only #000/#a0a0a0/none; no ink-box collision, off-canvas glyph or glyph/rule touch in 154 renders (incl. 200 rows, long titles, all sizes); 200-row draw under budget (debug); purity.

## mutants
12/13 killed: billion snap→1e−9, tiny width→shared absolute step, caption last-class, tally groups of four, percent half-up→floor, decimal rounding note, student answers drawn, 66 % floor removed, tally width from count, `cum_rel` default off, fallback prefers narrowest width, stale FAQ tally wording. **Survivor:** `candidate *= 10.0` → `*= 1.0` in the distinct-boundary fallback still passes `grouped_tiny_differences…`; 0.01 resolves first, so the "widen until distinct" loop is never exercised.

## not checked
No PNG looked at (no image input), so no visual judgement of 66 % legibility, apparent collisions or aesthetics; resvg ink boxes only. No full lib suite, no release timing, no real copier/PDF, no egui/browser. The histogram's rendered SVG was not inspected, only its public `bins()`/`auto_width` outputs.

## biggest gap
No class-midpoint column, and intervals are written `a to <b` instead of textbook limits (40–49, boundaries 39.5–49.5); every class needs hand-editing before it goes on an exam.
