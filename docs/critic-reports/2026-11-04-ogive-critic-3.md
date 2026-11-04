# Ogive, critic 3, commit 2292be3 (figures-gapfill)
Verdict: **FAIL. 0 blocker, 1 major, 4 MED, 5 LOW, 5 minor.** Probe scripts in `scratchpad/`; every printed number recomputed with Python `Fraction` + half-up; SVG read geometrically. I have no image input, so no visual pass (stated again under "not checked").

## BLOCKER
None.

## MAJOR
1. **A percentile that coincides with a mark loses both labels, silently.** Default table, `read=P50, marks=Median`: 2 coincident `polyline.guide` at (348.95, 182), **0** side/foot labels, `notes=[]`. `P52 + quartiles`: 4 guides, labels P52/Q1/Q3, the **M** guide unlabelled, no note. Cause: the per-reading rival check counts the identical guide as a rival at distance 0 (< `RIVAL_GAP` 12), so neither reading can ever fit; the round-2 "named guides must still be labelled" gate then suppresses the note. 57 of 1,188 keys (6 tables x pct 1..99/3 x 3 sizes, quartiles + a reading) have a distinct unlabelled guide and no note; 282 have one, 225 are noted.

## MED
1. **Fine x-grid is not gated per class.** `minor_x` uses `PLOT_W/n` (the average) and the same count is drawn in every class. `0,1,3 / 1,2,7 / 2,3,12 / 3,4,15 / 4,5,9 / 5,100,4` -> 4 fine lines inside a 5.2 px class, 0.98 px pitch; `0,0.5,1 / 0.5,100,5` -> 0.49 px. Print #8f8f8f merges into a grey band, so "five when the class is >=50 px, two at 20, none narrower" is false for unequal tables. The threshold also uses 520/n, not the drawn 490.6/n: n=26 has 18.9 px classes and still 2 lines. No test uses a narrow class.
2. **The 1e12 refusal's divide route does not work.** `0,500,3 / 1000,2000,5 / 1000000000000,1000000001000,1` says "delete line 3, or divide every boundary and frequency by a power of ten". Applying the division gives `0,50,0.3 / 100,200,0.5 / 1e11,1e11+100,0.1`, then "frequency 0.3 is not a whole number ... example 0, 50, 0"; the counts are destroyed. The test's applier only deletes, so the second route is never executed; the delete route is the only one that terminates, and it discards the class.
3. **Two-column midpoint/frequency pastes with thousands commas are still mangled.** `1,000, 3 / 2,000, 5` (and the same under `midpoint, f`) -> "upper boundary 0 is below lower boundary 1 ... example 0, 1, 3"; `1,000, 2,000` -> "example 1, 0, 2". `strip_thousands` only runs when the plain cell count is >3, so the 3-cell case never reaches it (`1,000, 2,000, 5` does).
4. **The 66% grid claim is not true at 1x.** Print, default, P40+quartiles: 1x medians fine vertical 147 / horizontal 152; direct 0.66 render 177 / 179 (fine), 161 / 168 (labelled); box-downsample of the 1x to 66% 164 / 172 (fine), 159 / 173 (labelled); threshold 0.66*255=168. So the horizontal fine grid fails both 66% models, the vertical fails the direct one. The test renders at 2x (`to_rgba(...,2.0)`) and never downsamples, so it cannot see this (its comment still says "at 3x").

## LOW
- `side_short` for a mark on a relative/percent axis duplicates the name: `M: M: 50%`, `M: M: 0.5` at P49, all sizes (12/144 of that sweep).
- Value readings at the ends: `value=40` draws a 15 px dash on the x-axis labelled "0.0 (0.0%)" (critic-1 LOW 6, unchanged); `value=100` keeps only the foot "100", dropping the side "50 (100%)". 90/324 value readings had no side label (v=90, 95, 99.9, 100) and no note.
- Large-boundary tolerance (`same` = 1e-9 * max boundary): `1000000000,1000000001,3 / 1000000001,1000000002,5` is valid and contiguous but refused as "lies inside", offered `1000000000,1000000001,8`; a real gap `...1002,1003,5` is silently closed (shown `1000000001 to 1000000003`, count kept).
- First data row dropped as a header: `0, 10, group / 10, 20, 4` is accepted with only `10 to 20` (any first line with <3 numbers and a header word is skipped).
- The P49 note's `set Mark to None` deletes the labelled median/quartiles it does not name (the named guide survives, so the test passes).

## minor
200 classes -> 201 tick marks 2.6 px apart with 16 labels/gridlines (a comb); `1 classes` in the desc for a one-class table; raw values default x title "Test score"; "about 29.5 of the 50 values" (fractional count); `plain()` rounds advice boundaries to 6 dp.

## verified (independent method)
- 6,435 grouped percentile cases (13 tables incl. tenths, negatives, flat, 1/3, 1/7, n=1, 1e-4) x pct 1-99 x decimals 0-4: every `about` = exact Fraction half-up; every accepted range contains the exact x; IQR = printed Q3 - printed Q1; 25,740 guide corners on their exact (x, t) within 0.05 px.
- 540 raw cases (both edge rules): every value in exactly one shown class, total = n, percentiles exact; 78 curve checks: start at (first lo, 0), vertex per upper boundary, end at n; 630 value readings: percentage and range exact.
- 756 student/key pairs: same viewBox; no guide/plate/side/foot/tcum/leader/answer; no answer text or "about"/"accept"; no cumulative leak; Print fills/strokes grey only (suite raster hue 0).
- Label sweeps: 1,296 keys / 208 leaders, none crosses a rival guide, none nearer a rival (MED 3 holds); 1,188-key distinct-guide sweep: the 57 silent cases above; 1,782 single-reading keys: every guide labelled or noted (2 unlabelled, both with a working note). 44 malformed tables: advice terminates, no repeated note/table. 1e12 advice no longer loops via delete.
- Round-2 items: `0,10/20,30` -> `10,20,0`; `40,50/52,60` -> `50,52,0`; `0,0.9/1.0,1.9` -> `-0.05,0.95` / `0.95,1.95` (counts kept); `40 -49`, `40- 49`, `40 − 50`, `1,000, 2,000, 5` end at the intended classes; flat P60 note 15-20, accept 14.2-20 (18 inside); `U+2212` between numbers and `Class 1, Frequency` parse; the value reading's percentage range exists and is exact in 630 cases.
- `census_baseline` green; 23 ogive tests green, 1 ignored; 10k-row/200-class/raw-10k/extremes no panic; 200 draws+notes 3.1 s CPU (debug, bound 20 s).

## mutants
One-line mutants, one pinning test each, reverted; `git status` clean after each. Killed: M1 stated-limit unit check (advice test); M2 half-up tie (rounding); M3 IQR +1; M4 plateau range; M5 overflow check removed; M6 Print grid grey; M7 fine-x count 5->4; M8 student cumulative column; M9 raised-label nearest-guide guard; M10 no-room step may delete named guides; M11 thousands stripping. **Survivor: M12** (`value_at` `y1 >= t` -> `y1 > t`) leaves `printed_readings_match_the_recomputation` green; it is killed only by `a_flat_stretch_is_noted...`, so the readings-recomputation pin does not cover the plateau-start rule.

## not checked
No image input: I did not look at any PNG; all visual claims above are pixel/geometry measurements, not an aesthetic judgement. `nothing_collides_and_every_guide_ends_on_the_curve...` (the 2x resvg ink audit) was still running at 12 min and was killed (no failure shown); the full lib suite and live egui/PDF/font rendering were not run. The overflow divide route is not test-pinned.

## biggest gap
The key's in-plot labels are unreliable for the commonest combination (a percentile coinciding with a mark can lose both labels silently), and the fine grid is not per class and not photocopy-safe at 1x+66%, so the half-division accepted range still claims precision the printed grid does not show.
