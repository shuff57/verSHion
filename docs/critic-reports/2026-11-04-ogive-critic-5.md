# Ogive, critic 5, commit ca043f8 (figures-gapfill)
Verdict: **FAIL. 0 blocker, 0 major, 1 MED, 4 LOW, 3 minor.** Probes in `scratchpad/` (public-registry probe dumping SVGs, notes and rasters; `check_rand.py`, `geomcheck.py`, `collide.py`, `printcheck.py`, `fuzzadvice.py`, `mutate5.py`). **No image input: no PNG was looked at**; every statement is arithmetic, SVG geometry or pixel measurement. 594 readings recomputed with `Fraction` half-up, 400 random tables, 106 curve fits.
## BLOCKER
None.
## MAJOR
None.
## MED
1. **A headerless 3-column midpoint/frequency/cumulative paste is still destroyed by the advice.** `1000, 3, 3 / 2000, 5, 8 / 3000, 2, 10` (midpoints, f, cf) is read as lower/upper/f; the chain offers `"3, 1000, 3"`, then `"5, 2000, 8"`, then `"2, 3000, 10"`, merges, and settles on **one class `2 to 3000` with frequency 21**. True reading: 500-1500 f3, 1500-2500 f5, 2500-3500 f2, total 10. `midpoint, f, cf` works, so the layout is known; without the header the cumulative signal (`3, 8, 10` = running sum of `3, 5, 2`) is in hand and never used, and the note asserts a bounds swap instead of naming the midpoint reading.
## LOW
- **`exact_text`'s 1e-12 relative tolerance is 1.0 at 1e12**, so advice rounds boundaries by up to 0.5. `999999999997, 999999999998, 3 / 999999999999, 1000000000000, 5` offers `"999999999996, ..., 3"`, whose re-parse produces **the same note** (a fixed point); `...998.5, ...999.5 / ...999.5, 1000000000000.5` is reported as boundary `1000000000000` and its scaled example is a zero-width `"100000000000, 100000000000, 5"`, and that chain never settles (fuzz `c019`). Classroom scale unaffected, but it is the documented limit and the same root as the fixed `same`.
- **Value readings at a guide crowded by a mark lose the side label.** `read=Percentile of a value, value=70, marks=Median` draws `70` at the foot and `M: 25 = 50% of 50` at the side; no `70: 22.0 (44.0%)` and no note. `value=100` likewise has only the foot `100`. Height is on the axis and in the answer, so not wrong, just unnamed.
- **`value=40` (first boundary) draws a zero-height guide**: `72.60,342 87.32,342 87.32,342`, lying exactly on the x-axis. The label `40: 0.0 (0.0%)` is correct; the guide is invisible. Honest, not misleading.
- **The Print grid at 16 classes is heavy**: 35 horizontal + 32 vertical 1.8 px lines; 17.5 % of the plot below L=128 and 38.6 % below L=250, against 1.5-1.9 % for curve+axes. The y grid is 9.1 px pitch at 1x (6.0 px at 66 %); the x pitch is now 15.3 px (MIN_PITCH holds).
## minor
- A Print refusal is not grey: `style=Print` + a bad table renders `fill="#111827"` (blue hue), because `refusal()` never takes the style; the grey-only test skips refusals.
- Small + Print at 66 % gives 7.0 px labels (Medium 8.8 px); the 66 % legibility test only covers Medium.
- The table's relative-frequency column is d+2 dp while the key's reading is d dp; by design, but not literally "the decimals setting".
## verified (independent)
- 594 readings (percentile, value, marks, IQR, accept ranges, flat range) over 400 random tables at d=0-4: 0 discrepancies; IQR = printed Q3 − printed Q1; every range contains the exact value and any plateau endpoints.
- 106 cases with >=2 well-spaced guides: every curve vertex maps to (upper boundary, running total), starts at (first lo, 0), ends at (last hi, n). 400 random + 14 targeted keys: no text-text, label-curve, label-guide, label-dot or off-canvas collision with resvg ink boxes.
- Round-4 claims: 4-col thousands paste gives 1000-2000 f5 / 2000-3000 f7 with the mismatch note; `midpoint, f, cf` gives 500-1500 f3 / 1500-2500 f5; one-unit classes at 1e9 and 1e12 stay apart and one-unit gaps are seen; sub-micro and 1e-7 advice quotes settle; 200 classes draw 22 ticks/17 labels at >=19.6 px; 16-class fine x pitch 15.3 px; refusals carry `<title>`, `<desc>`, `role="img"`.
- 10,000-row table refused in one render; NaN/inf/1e13/emoji/300-char title/zero/duplicate/reversed lines: no panic, non-trivial SVG; 10,000 raw values fine.
- Coincident labels at Small: P50+M and P25+Q1 share one guide and one label; a group of three is unreachable (one read plus at most three marks, and no two distinct t share an x); dense P1 at Small is noted as unlabelled with no offered fix.
- Student copy (800 renders): same viewBox, no guide/plate/side/foot/tcum/answer/leader, no "about"/"accept". Print (58 random + 5 targeted): all fills/strokes grey; grid medians 120-123 at 1x and 125-139 at 66 % (threshold 168). Table cumulative over 400 tables: 0 bad.
- Census `census_baseline` green; `nothing_collides...` (615 keys) and `every_distinct_guide...` green.
## mutants
13 one-line mutants, each reverted; 12 killed (MIN_PITCH, divisions cap, 4-col thousands, midpoint header, group names, `same`, `exact_text`, tick thinning, refusal describe, step-of-2, read_tol half-step, value_at tie). **Survivor: table cumulative percent pinned to 1 dp** — the test only calls `cum_text(...,1)`, so "the table follows decimals" at d!=1 is unpinned (behaviour itself verified above).
## not checked
No image input: nothing about how it looks, whether the 16-class grid "overwhelms" in the eye, or the Small/66 % aesthetic. Not run: whole debug suite, full release sweep, live egui/PDF/font rendering, agent-eval cases, census byte-diff (only the blessed `census_baseline`).
## biggest gap
The table reader still has a data-destroying ambiguity: a headerless 3-column midpoint/frequency/cumulative paste is refused with advice that rewrites the counts (10 -> 21) into one invented class, though the cumulative column is in hand.
