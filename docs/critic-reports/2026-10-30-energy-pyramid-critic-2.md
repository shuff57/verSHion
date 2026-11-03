# Critic 2 on 2060f43: energy pyramid round 2 (arrows beside the pyramid, half-up percent, raised exponents)

This is a verification pass over `2060f43`. Its own change is `b5ebb1a..2060f43` restricted to `energypyramid.rs`,
`faq.rs`, `figures-progress.md` and the census. `df9c7c2`, which sits between them, is another session's biology
commit and is not counted here. I ran everything on my own `git archive` copy with its own target dir. My harness is
a child module of the file's `tests` module (so it reuses the builder's SVG parser) and adds its own checks:
- an arrow-column geometry check;
- a Python `decimal` oracle for the percent;
- a 15,000-input canvas/note sweep;
- a student-leak search;
- PNGs at 1x and 1.3x with their 66 % print copies, which I looked at with the image reader.

**Verdict: FAIL (0 blocker, 0 major, 1 MED, 3 LOW, 5 minor).** Most of the round holds:
- The half-up percent is exact against `decimal` over 21,460 inputs, apart from the disclosed 1e-7 tie window.
- The arrow column is clean in 20,640 renders.
- The 6 px cap rule is exact at the boundary.
- The ratio refusal is true on both sides of the line and never appears in Equal steps.
- The canvas and the note agree, and nothing leaks into the student copy.
- All nine mutants I ran were killed.
- The census is clean.

The one MED is the case round 1 named as LOW 4: **in the 66 % print copy at 1x the top level's bar still disappears
at every label size.** Only its two caps survive, so the top level prints as three loose dashes. The new test only
looks for dark pixels on the caps, so it passes anyway.

## Claim table

| # | Claim | Verdict | Evidence |
|---|---|---|---|
| 1 | Percent rounds half up at 3 s.f. (31.25 → 31.3; 56.25 → 56.3; 3.125 → 3.13), pinned by an exact-integer oracle | **FIXED** | `c2_pct_dump` renders 21,460 two-level pyramids through the whole figure: 30,000 random typed mantissas of 1–6 digits with exponents, 1,791 k/2^j ties at 9 scales, and a targeted list (12.35/100, 1.235/1000, 1235/10000, 1.005/100, 99.95/100, 9.995/1e6, 0.0000099999/1, 1e-3/1e15 …). I compared each printed percent with Python `Decimal` ROUND_HALF_UP from both the **typed** and the **printed** values. There were **3** disagreements, all inside the builder's disclosed window: `1000000000 / 312499999.9`, `…/312499999.99` and `1e10 / 3124999999.9` print **31.3**, but the exact value is 31.2499999990 → 31.2 (minor 4). Every printed value equals the typed value to 15 s.f. (0 of 21,460 differ), so the printed numbers and the printed percent never contradict outside that window. The 99.9 rule holds: 9995/10000, 99.95/100 and 99.94999/100 match the oracle. The mutant `round_ties_even` fails `the_percent_rounds_half_up_against_exact_arithmetic`. |
| 2a | Arrows in their own column right of every bar and label, each from just above one row to just below the next | **FIXED** | `c2_column_sweep` ran 20,640 renders: 2–6 levels × 7 ratios (to 1000) × 4 magnitudes (1e3, 1e21, 2.5e-7, 123456789012.345) plus late-drop, early-drop and 9.99e14 sets × name sets (plain, 85-char, CJK, 60×W, 2-char) × both scales × S/M/L × transfer × answers × unit/none. On top of the builder's `check_layout` it checks: every shaft sits at one x; no text, bar, leader or cap within 8 px left of the shaft; every percent or blank starts right of the shaft; each arrow's tail lies inside the lower row and its tip inside the upper row; exactly one percent or blank between each tail and tip; nothing outside the canvas. It found **0 problems**. The break between consecutive arrows is 8–14 px (10·lab), centred on each row. With transfer off there are no shafts and no polygons, and the figure is re-centred. |
| 2b | Bar < 6 px solid black with a 7 px cap at each end; widths still exactly proportional; caption only when capped | **FIXED** | 400:B at S/M/L × both scales. B = 5.9, 5.99 and 5.999999 give `#111111` with 2 caps and the caption. B = 6, 6.000001 and 6.1 give `#bdbdbd` with an outline, 0 caps and no caption. Equal steps never caps (the narrowest is 400·0.7^5 = 67 px) and never shows the sentence. The measured `width` is exactly the typed value (5.9000, 0.4000, …). The closest a cap comes to a name is 6.1 px (Medium, 0.4 px bar). A cap never meets the leader, which is at the bar's mid-height. The mutant SLIVER = 3 is killed. |
| 2c | Top level stays visible and labelled in the 66 % print copy at 1x and 1.3x, all three sizes | **PARTLY** (MED 1) | The caps and the label survive. **The bar does not survive at 1x**; see MED 1. |
| 3 | Ratio reason says "thinner than 0.4 px"; second line says set Equal steps; nothing switches; Proportional only | **FIXED** | At 1000:1 the top bar is **0.40 px at Small, Medium and Large** (BASE_W does not scale with the label size). 1000.000001:1 draws (1e-9 tolerance, 0.3999996 px, which prints 0.40). 1000.00001:1, 1001:1 and 1e6:1 are refused with a true reason. The second line reads "Nothing is drawn: set Bar widths to Equal steps to draw this chain (it is marked Not to scale), or bring the largest and smallest values closer." I checked that wording at S/M/L (`ratio-refusal-large-1.3x.png`: wraps in 420 px, nothing clipped). The same inputs in Equal steps give **no refusal and an empty note** at every size, and my 15,000-input sweep found 0 ratio refusals in Equal steps. |
| 4 | BOM ignored; 48 px blank with free paper; 15 s.f.; capital X and `10⁶`; quoted names; leak test derives strings | **FIXED, with minors** | A leading U+FEFF is stripped, and its render is byte-identical to the render without it. A second BOM (`\u{feff}\u{feff}A`) or one on line 2 still ends up in the name (minor; only the first is a real BOM). The blank is 48·lab long, with no text, line or head within 18·lab above it (`large-student.png`, `six-long-prop-large-student-1.3x.png`). `10000.000000000001` prints 10,000. `1.5 X 10^6`, `1.5×10^6`, `10⁶`, `10⁻³`, `1.5 × 10⁺⁶` and `1.5\t×\t10^6` are read; `1.5 × 10 ^ 6` and `10 ⁶` are refused (minor 1). The names are quoted. The leak test computes its strings, but see LOW 1 for a loosened neighbour. |
| 5 | Owner decisions 2/3/4 | **FIXED** | Equal values draw at 100 %. 99.96 prints 99.9 (and the FAQ says so). 1e15 → `1 × 10^15`; 9.99e14 → `999,000,000,000,000`; 1e-3 → `0.001`; 0.00099 → `9.9 × 10^-4`. In the SVG the exponent is a `<tspan>` with dy = −0.42·fs at 0.7·fs, returned by +0.42·fs before the unit. I looked at `sci-narrow-1.3x.png` (beside a capped bar, Large), `energypyramid-sci-equal-*.png` and `sci-student-small-1.3x.png` (Equal steps, student, Small): there is no collision with the row above, and no number prints in two notations in one figure. `<desc>` and the note keep `^` (disclosed). |
| 6 | Suite / census / mutants | **CONFIRMED** | See the regression section. |

FAQ: I read the four changed answers against the code, and all four are true: 15 s.f., the `10^` notation, the 400 px
base and the 0.4 px limit, Equal steps never switching on by itself, half-up with the 99.9 rule, and the 6 px solid bar
with caps. `every_figure_panel_has_a_faq` passes.

## New defects (ranked)

1. **MED: in the 66 % print copy at 1x, the top level's bar is gone at every size, and it prints as three dashes.**
   - Input: the default (`10000/1000/100/10`), whose top bar is 0.40 px.
   - The darkest pixel in each row of the bar's interior, over its whole height at 1x:
     - Large: **195** in every row;
     - Medium: 154–195, with 2 rows dark in bw66;
     - Small: 43–195, with 1 row dark.
     The print cut-off is 168, so the vertical stroke is not there.
   - What remains is the top cap, the leader and the bottom cap. In `default-Small-1x-bw66.png`, `default-Large-1x-bw66.png`
     and the 4x zoom `zoom_default-Small-1x-bw66.png`, "Tertiary consumers" is followed by `–`, `— 10 kcal/m²/yr`
     and `–` stacked vertically, with no bar. That is not a bar a student can compare with the others.
   - At 1.3x the stroke survives (luminance 76 at Small and 136 at Medium/Large, every row dark).
   - The caption on the same page says "drawn solid … so it can be seen", which is false on that copy.
   - `the_top_level_survives_the_print_copy` only tests for dark pixels at ±2–3 px on the two cap rows. Its own
     comment concedes that the bar "fades … and drops out", so the test cannot catch this.
   - The fix is the one round 1 offered: a minimum drawn stroke for a sliver (for example a 1 px black centre line
     or outline), still disclosed in the caption, with the test sampling the bar's interior rows.
2. **LOW: a test assertion was loosened until it is vacuous.**
   - In `the_percent_is_an_answer_and_a_student_copy_leaks_none_of_it`, b5ebb1a asserted
     `!off_key.contains("<polygon") && !joined(&off_key).contains('%')`.
   - 2060f43 appends `|| joined(&off_key).contains("proportional")`. `off_key` is rendered in the default Proportional
     scale, so its caption always contains "proportional". For the third chain it is a refusal canvas (1013:1), and
     the old assertion did not catch that either.
   - So for all four chains the "transfer off removes arrows and percents" check now always passes.
   - My column sweep shows the behaviour itself is right, so this is test hygiene only. Drop the `||` clause, or test
     the column with `heads()`/`lines()`.
3. **LOW: with long names at Large, a sliver's caps line up with the wrapped name lines and read as punctuation.**
   - Six levels of 50-character names, Proportional, Large, student (`six-long-prop-large-student-1.3x.png`).
   - The 1 px top bar's caps sit 6 px right of "Extraordinarily long" and of "going", so the block reads
     "Extraordinarily long ‾" / "going _".
   - This happens because the band (4 lines × 21 px) is taller than the bar (50.4 px), and the caps sit exactly at
     the line heights.
   - No geometric collision; a legibility issue only.
4. **LOW: the raised exponent is lost from the drawn text.** Selecting or copying the label, or a screen reader
   reading the SVG's text rather than its `<desc>`, gives **`1 × 1021 kcal/m²/yr`** for 10^21. I extracted this from
   `sci-narrow.svg`. `<desc>` says `1 × 10^21`, so assistive tech that reads `<desc>` is fine. A pasted label is a
   different number. A cheap fix is a `<title>` per value, or an `aria-label` on the `<text>`.
5. **minor 1:** a space before the caret is refused. `1.5 × 10 ^ 6`, `1.5 × 10 ^6` and `1.5 × 10 ⁶` give "not a
   number", while `1.5\t×\t10^6` is accepted. The cause is that `number()` tests `s.contains("10^")` before it strips
   whitespace.
6. **minor 2:** `1e-400` is refused as "is not above zero; every level has some energy". It is positive, but it
   underflows to 0. The message is wrong, though the refusal is right.
7. **minor 3:** the 15-figure read moves a value across the notation threshold. `999999999999999.9` is typed below
   10^15 and prints `1 × 10^15`. `1234567890123456` prints `1.23456789012346 × 10^15`. Both are disclosed by "read to
   15 significant figures"; I list them only because the printed form then differs from what was typed in two ways
   at once.
8. **minor 4 (disclosed):** the 1e-7 tie tolerance rounds non-ties up. `Producers: 1000000000 / B: 312499999.9`
   prints **31.3 %**, but the exact value is 31.24999999 %. The student sees both values printed in full, so the key
   is one digit off for a 10-significant-figure input.
9. **minor 5:** the canvas is not centred on the pyramid. With transfer on, the axis sits 23–179 px left of the
   canvas centre in the sweep. At Large the fixed `PCT_W = 74·lab` leaves up to **83.2 px** of empty right margin
   when the percents are short (`10 %`). This is by design, and the student blank needs that room, so I record it
   only.

Observations, not defects:
- A 5.9 px bar (solid black) beside a 6.0 px bar (grey with an outline) is an ink jump with no change in width
  (`capstrip.png`). The rule is continuous in width, and the caption explains the change in look.
- A lone CR (old Mac line ending) makes one line with "two colons". CRLF and tabs are fine.

## Teacher view (PNGs looked at)

- **Default** (`energypyramid-default.png`): MED 1 of round 1 is fixed on screen. The top 0.4 px level now reads as an
  I-beam with its own caps, the 4 px level as a solid black bar, and the arrows stand apart in a column on the right.
  Each arrow starts and ends inside its two rows, and its percent sits beside it. I could tell which level feeds which
  from the rows. The column is up to about 300 px from the slivers, but the rows make the pairing clear.
- **Large student** (`energypyramid-large-student.png`): the blanks are long, with plenty of paper above them. Good.
- **Equal steps, 6 levels, Large** (`energypyramid-six-equal-large.png`): clean. This is the version teachers will use.
- **Long names, 6 levels** (`six-long-prop-*.png`): tidy, apart from LOW 3.
- **Very different widths** (`drop-late`, `drop-early`): correct, and readable.
- **Capped-bar caption:** when a bar is capped, the caption grows to 3 lines at Medium. It appears only then, and never
  in Equal steps, so it does not clutter a pyramid with nothing capped. That is acceptable.
- **66 % print at 1x:** the top level is lost (MED 1). At 1.3x it is fine.

Would a teacher drop it into an exam now? The Equal steps version, yes. The Proportional version on screen, yes. The
Proportional version printed at 100 %, not yet.

**Single biggest remaining gap:** what the printed top level looks like (MED 1). Beyond this figure's claims, the
round-1 scope note still stands: there are no pyramids of biomass or of numbers.

## Regression and census

- **Suite on my copy of 2060f43** (merged tree, which includes df9c7c2): lib **1974 passed, 0 failed, 5 ignored**.
  That is the builder's 1963 plus the 11 tests df9c7c2 adds (counted from the `#[test]` diff of its four files), and
  `absurd_windows_cannot_run_away` passed on the first run. With my 7 harness tests the module runs 33/33.
- `git diff b5ebb1a 2060f43 --stat`: only this commit's 4 files plus df9c7c2's. `screens.rs`, `theme.rs`, `fonts.rs`,
  `main.rs`, `examples/`, `Cargo.toml` and `Cargo.lock` are untouched. There is no dependency and no saved-format
  change (the params are unchanged).
- Module tests went from 21 to 26:
  - `a_sliver_keeps_a_whole_label_beside_it` was replaced by a stricter sliver test;
  - `labels_stay_clear_of_the_transfer_marks` was folded into `check_layout` (now every size, not just Large);
  - one assertion was loosened (LOW 2).
  The new module adds no compiler warning; every warning in the build comes from other modules.
- **Census:** `census_baseline` passes on my 2060f43 copy, so the fixture equals the recomputed hashes, including
  `energy-pyramid__defaults/__start 87a3bebc05d097f1`. `git diff df9c7c2 2060f43 -- tests/fixtures/figure_census.txt`
  changes only the two `energy-pyramid` lines. The `enzyme-kinetics` line (`92452f6d3bbbb3ac`) is byte-equal to
  df9c7c2's, so the merge is clean. `df9c7c2..2060f43` touches no rendering code outside `energypyramid.rs`, so the
  food web and every other figure render byte-identically. I did not rerun `census_dump` on df9c7c2.
- **Mutants (9, all killed):**

  | Mutant | Killed by |
  |---|---|
  | half-even rounding | `the_percent_rounds_half_up_against_exact_arithmetic` |
  | SLIVER = 3 | `a_sliver_is_a_bar_of_its_own…` |
  | arrows on the axis | 3 tests |
  | column only 2 px past the labels | `the_layout_holds_across_the_family` and the exponent test |
  | key always drawn | the leak test and the blank test |
  | a percent in a student-only `<metadata>` | the leak test |
  | `warnings()` on `parse()` | both agreement tests |
  | no return `dy` | the exponent test |
  | a 30 px blank | the blank test |

  None survived. The print-copy test was not mutated, because MED 1 already shows what it misses.

## Not checked

- The egui editor live: the hint text and how the note panel renders.
- RTL names visually.
- Print at the app's real export DPI (only 1x and 1.3x).
- `census_dump` file by file against df9c7c2; I relied on the code diff and the fixture instead.
- A screen reader's actual reading of the `<tspan>` (LOW 4 is inferred from the text nodes).
