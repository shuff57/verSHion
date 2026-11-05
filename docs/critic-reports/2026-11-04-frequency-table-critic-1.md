# frequency-table — blind critic 1 (ce6c194, figures-gapfill)

Verdict: **FAIL — 1 blocker, 0 major, 2 MED, 4 LOW, 2 minor.** 17/17 module tests pass; `census_baseline` green (frequency lines 7e5203… identical at HEAD); 11/11 mutants killed. Probes: `scratchpad/harness` (public registry, own gcd/half-up/count code, resvg ink boxes).

## BLOCKER
1. **Grouped auto classes print every boundary as 0 when the auto width is under 1e-6.** `mode=Grouped classes`, `data=0.0000001, 0.0000002, 0.0000003` → rows labelled `0 to <0`, `0 to <0`, `0 to 0`, and the note says “The classes are 0 wide, starting at 0 … upper boundary 0, above the largest value (0)”. Counts (1,1,1) are right; the class limits — the table's content — are false and duplicated. Cause: `chart::fmt` → `data::fmt_num("{:.6}")` (frequency.rs:390, chart.rs:39). Fix: format boundaries with `value_text`/scientific, or refuse sub-1e-6 widths; add a no-duplicate-label test.

## MAJOR
None.

## MED
1. **The student copy leaks the largest count through the tally column width.** Widths are computed from `Cell::Tally(f)` before `draw_row` blanks it, and `answer_pair` requires the same viewBox. Single-row `1`×11/12/13 → tally column 52.18/55.64/59.09 px (≤10 clamps to 48); a student can read the blanked count off the ruler. Fix: a fixed max-tally column width shared by both copies.
2. **`1,000` is read as two values, 1 and 0.** `data=1,000, 2,000, 3,000` → n=6, rows `0:3, 1:1, 2:1, 3:1`, no warning (only the percent-sum note); `canonical()` bakes it as `1, 0, 2, 0, 3, 0`. The sibling ogive already strips thousands (ogive.rs:365), so the same paste means different numbers in two stats figures.

## LOW
- Key note prints **two percent signs**: “the cells add to 99.9%%” / “100.1%%” (`format!("…{}%…", pct_text_scaled(..))`, frequency.rs:578).
- **The 5,000-value cap is bypassed by a single comma-separated line.** 5,001 values → total 5001 and no truncation note; `data::read` checks `MAX_ROWS` per line, not per cell (data.rs:73), so the 10,000-value test only checks that it draws.
- **Small at 66 % is 7.02 px** (10.64 × 0.66), below the suite's own 8.4 px floor; that floor is enforced for Medium only, though the claim says “legible at 66 %”.
- **`value_text` is not the claimed shortest round-trip for sub-1e-19 values:** typed `1e-20` prints `9.9999999999999995e−21` (17-digit fallback, not `1e−20`).

## minor
- A refused width of 0.01 on `1,2,3,4,5` becomes 0.05 → 101 classes, 96 empty; the advice (“0.05 or more”) satisfies the cap but is not the readable fix.
- The >200-distinct refusal says “type fewer values” without a number to type.

## Verified (recomputed independently)
Raw counts, ascending labels, cumulative sums, reduced fractions and half-up percents (0–2 dp) over 9 data sets incl. 12.5, 6.25, 0.5, 0.25, 66.67, 1/200, 1/400, negatives, decimals, one value, all-identical, 1e-20, 200 rows; 201 distinct refused. Grouped auto width/start/bins equal `stats::auto_width/auto_start/bins` and the histogram's default; boundary 10/20/20/30 → 10–20:1, 20–30:2, 30–40:1; last upper 40; width 100 > range → one class; width 3 → 1–4:3, 4–7:2; zero/negative width and start-above-data fixed with typed-back values. Totals n / 1 / 100.0 % / n and cells sum. Tally 7 = 7 strokes, 1 diagonal (five group). resvg ink: no clip, collision or column escape at S/M/L, key/student, long title/heading, unicode, 200 rows, empty, refusal, tally 25. Empty data = instruction, not panic. Student text set = key minus answer cells; no answer class, tally stroke or key note. Print grey only, band darker than paper. Hostile NaN/inf/1e400/emoji/10k chars no panic; pure; 50 grouped-200 renders = 51 ms release. Registry, defs/relevant/examples, FAQ arm, NO_HANDLES pass.

## Mutants (11 one-line, each vs its pinning test)
pct floor, fraction unreduced, cumulative from 1, last class `<`, student answers drawn, tally group 6, bins closed-right, total = row count, tally cap 26, value_text→chart::fmt, percent padding 1 — **all 11 killed**, 0 survivors.

## Not checked
**No image input: I did not look at any PNG**; visual claims are SVG/resvg geometry only. The given `/tmp/figures-crit/freq1` did not exist, so I rebuilt it from the live tree at ce6c194 and verified `frequency.rs`/tests sha256 match HEAD (unrelated periodic/vsepr/bar files in that tree were mid-edit; the frequency census hashes match HEAD). Not checked: browser font metrics, live egui, PDF/DOCX, full lib suite (per instructions), the histogram's rendered bars (helpers compared, not its SVG), a real 66 % copier.

## Biggest gap vs a textbook frequency table
No **cumulative relative frequency** column (and relative frequency cannot be a decimal/proportion, only a reduced fraction or a percent). OpenStax's grouped heights table is Heights | Frequency | Relative frequency (0.05…) | Cumulative relative frequency (0.05…1.00) plus a Total row; this figure prints cumulative *count* only, so a teacher must add the standard column by hand, and the classes read “a to <b” rather than the textbook interval.
