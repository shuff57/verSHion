# Critic 3 on e29ba45: energy pyramid round 3 (a 1.5 px minimum bar, real print-copy test, aria-label for exponents)

This is a verification pass over `e29ba45`. Its whole change is `2060f43..e29ba45` restricted to `energypyramid.rs`,
`faq.rs`, `figures-progress.md` and the census; `481b0ce` between them is docs only. I ran everything on my own
`git archive e29ba45` copy with its own target dir, and ignored the other session's uncommitted edits in the worktree.
My harness (`critic3.rs`, a child of the file's `tests` module, so it reuses the builder's SVG parsers) adds:
- a boundary sweep of the drawn width, its centre and the caption at S/M/L and both scales;
- a print check: for 39 cases × 1x and 1.3x it looks for ink in the 66 % copy along every bar row, line, arrowhead and text box;
- a 10,608-render geometry sweep with 1 px padding, a caption-vs-pyramid check, a `roxmltree` parse of every SVG and an aria-label audit;
- a parse table and a 12,000-input canvas/note agreement sweep that includes the new tokens;
- 10 mutants.
I looked at PNGs with the image reader.

**Verdict: PASS (0 blocker, 0 major, 0 MED, 2 LOW, 6 minor).**
- MED 1 of round 2 is closed. In the 66 % print copy at 1x the top bar is now a solid 1.5 px stroke at every size, and
  every element I checked has ink.
- The 1.5 px rule is exact on both sides of the line and symmetric about the axis.
- The transfer-off assertion is real again. The aria-label is right.
- Every mutant I ran was killed. The census is clean.

What remains is a consequence of the owner decision (two unequal levels can now draw identical bars) plus small wording
and parse edges.

## Claim table

| # | Claim | Verdict | Evidence |
|---|---|---|---|
| 1 | Bars under 1.5 px drawn exactly 1.5 px, solid black; ≥ 1.5 px exact; caption discloses only when it happens; Equal steps unaffected; print-copy test samples the bar | **FIXED** | `boundary.txt`, inputs `A: 400 / B: x` and two 3-level variants, S/M/L × both scales. 0.4, 1.0, 1.40, 1.49, 1.4999 and 1.49999999999 draw **1.5000**. 1.5, 1.5000000001 and 1.5001 draw 1.5000 (to 2 dp). 1.6, 3 and 5.9 draw exactly 1.6, 3 and 5.9. The width is continuous with no jump, and the centre is on the axis to 0.0000 in all 242 drawn rows. The "under 1.5 px" clause appears for every to-scale width under 1.5 and for none at or above it. The "thinner than 6 px … still to scale" sentence appears alone for 1.5 ≤ w < 6. Neither appears at 6, 6.1 or 40, or anywhere in Equal steps (its narrowest bar is 67 px; the sweep found no Equal-steps sliver). Print: `print.txt` reports "all ink present" for all 78 renders. That covers default, student, 1000:1, six-level proportional and Equal steps, long names, 10^21, 10^-9, two thin bars, thin plus capped, transfer off and hostile names, each at S/M/L × 1x/1.3x. For every row of every bar it checks the bar's columns (a thin bar) or both outlines (a wide bar), and it also checks every cap, leader, shaft, blank, arrowhead and text. Looked at: `default-Medium-1x-bw66.png` (the top level is an I-beam with a visible stem), `ratio1000-Large-1x-bw66.png`, `default-student-Small-1x-bw66.png` (plus a 4× zoom), `sci-tiny-Medium-1x-bw66.png`. The mutant `MIN_DRAWN = 0.4` is killed, but by **3** tests, not the 7 the builder reports (minor 6). A mutant that draws the 1.5 px sliver grey (`#bbbbbb`) is killed by the print-copy test, so that test reads the bar and not the caps. |
| 2 | LOW 1: transfer-off assertion real | **FIXED** | The `||` clause is gone. The new clauses are not vacuous: drawing the arrows with the transfer off (`if true {` on the column) is killed by the leak test and `the_layout_holds…`, and a canvas that always reserves the column is killed by the leak test's "narrower" clause. The clause now runs only in Equal steps. Proportional with transfer off is still covered by my sweep (0 polygons and no `%` text in `no-transfer-Medium.svg`; the only `%` is the background rect's `100%`). |
| 3 | LOW 2: a sliver's name 16 px from its caps | **FIXED** | The sweep's closest cap-to-name gap is **16.0 px**, over 10,608 renders that include 46-character single words, CJK, 60×W, hostile names and 50-character wrapped names. The leftmost name box starts at **15.996 px** (canvas margin 16), so nothing is pushed off the canvas. In `energypyramid-six-long-large-student.png` the caps no longer read as punctuation. The mutant gap = 6.5 is killed by `the_layout_holds…` (the literal 15.99 check). |
| 4 | LOW 3: aria-label on raised-exponent labels | **FIXED** | Across all 10,608 sweep renders, an aria-label sits on exactly the `<text>` elements that have a raised tspan. It equals the drawn text with `^` (`1 × 10^21 kcal/m²/yr`, `4 × 10^-7 kcal/m²/yr`). It is never a percent and always starts with a typed value's `fmt_num`. Every SVG, hostile names (`R&D "q" <x> 'y' &amp;`) included, parses with `roxmltree`. Only number and unit text reaches the attribute: units come from a fixed list and names never do, so `esc` not escaping `"` cannot bite here. The pixel-identity assertion exists in the test. The mutant that removes the aria-label is killed. The FAQ answer is true. |
| 5 | Minors: spaced exponent, `1e-400` message | **FIXED, with new edges** | `10 ^ 6`, `10 ⁶`, `1.5 X 10 ^ -3`, `1.5 × 10 ⁻³`, `1.5 x 10 ^ +6` and `1.5 ×10 ^6` read correctly. `1e-400`, `2e-324` and `00001e-400` give "smaller than the figure can read". `0e5`, `0.000`, `0.0e-400`, `-0` and `-0e5` give "not above zero". `5e-324` and `1e-308` draw. `1e400` gives "too large". `+5` and trailing spaces are fine. Three new edges are minors 1–3. Both parse mutants (no whitespace strip, old message) are killed. The three recorded limits are not made worse. |
| 6 | Suite 1975/0/5; census moves only the two energy-pyramid ids | **CONFIRMED** | See the regression section. |

## New defects (ranked)

1. **LOW: two unequal levels can now draw identical bars in a "proportional" pyramid.**
   - Input: `Grass: 1000 / Mice: 3 / Owls: 1`, Proportional, the default.
   - To scale, the bars are 1.2 px and 0.4 px. Both are drawn **1.5 px**, with identical caps (`two-thin-Medium-1.3x.png`).
   - The first caption sentence still says "Bar widths are proportional to energy". The exception sentence does not say
     which bar was widened, so a student told to compare widths sees Mice = Owls beside "33.3 %".
   - Any pair of levels whose to-scale widths both lie in 0.4–1.5 px collapses like this, which is a range of up to
     3.75 : 1. In the default the top bar is drawn at 3.75× its true width, so 100 : 10 reads as 4 px : 1.5 px.
   - This is the owner decision working as specified, and it is disclosed. I rank it LOW and list it for the owner.
2. **LOW: the default figure now always carries the long disclosure.**
   - The default (10,000 → 10) always has a 0.4 px level, so the commonest render has the 3-line caption at Medium
     (`default-Medium-1.3x.png`) and 4 lines at Large (`ratio1000-Large-1x-bw66.png`; sweep max 4 lines,
     `n2 r400 Large`).
   - Nothing collides: the caption top is always more than 4 px below the pyramid, and nothing is clipped at Small.
   - It is the clutter round 2 accepted only "when a bar is capped", and that is now every default. One shorter
     sentence would help, for example "Bars under 6 px are drawn solid with end caps; any under 1.5 px is widened to 1.5 px."
3. **minor 1: a negative underflow is called "too small".** `B: -1e-400` gives *"-1e-400" is smaller than the figure can
   read (the least is about 5 × 10^-324)*. It is negative, and round 2's "not above zero" was the true message. The
   test is `v == 0.0` plus a non-zero mantissa digit, and it ignores the sign. My agreement sweep hit this 417 times;
   there were no other disagreements.
4. **minor 2: the whitespace strip now joins split digits in more inputs, which can change the number silently.**
   - `1 5 × 10 ^ 6` reads as **15,000,000**. It was refused before this commit.
   - `1.5 × 1 0^6` reads as 1,500,000. It was also refused before.
   - `1.5 × 10⁶ 7` reads as **1.5 × 10^67**. This one is older: it was already accepted before, because the
     whitespace strip applied whenever `10^` appeared.
   - Plain `1 000` is still refused, so the rule is inconsistent.
   - A cheap fix is to strip whitespace only around `×`/`x`/`X`/`*` and `^`, not between digits.
5. **minor 3: a bar exactly 1.5 px wide can be disclosed as "widened".** `A: 6.42 / B: 0.024075` is exactly 1.5 px to
   scale, but `400·b/a` gives 1.4999999999999998. The caption then says a bar under 1.5 px is drawn 1.5 px wide, which
   is false for that input; 985 of my 19,999 constructed exact-1.5 inputs do this. The drawn width is right. A
   `1e-9` tolerance, like the ratio refusal's, would fix it.
6. **minor 4: the new message advertises subnormals that print digits not typed.** `A: 1e-320 / B: 3.125e-321` prints
   **`3.127 × 10^-321`**, and `4.9e-324` prints `5 × 10^-324`. The "least is about 5 × 10^-324" text invites these
   values, but below about 2.2e-308 the 15-s.f. read is not exact. Saying "about 10^-308" would be true.
7. **minor 5: sliver names stand further left than the next wider level's name.** A sliver's name ends 19.5 px left of
   the axis; a 6–19 px bar's name ends 13–19.5 px left. In `energypyramid-six-long-large-student.png` the names of
   levels 4 and 5 step out left of level 3's. This is cosmetic.
8. **minor 6: the claim says the 0.4 px mutant is killed by 7 tests.** It is killed by 3 (`a_bar_near_the_minimum…`,
   `proportional_widths…`, `the_top_level_survives_the_print_copy`). Either way it is killed; the progress note
   overstates the count.

Not re-raised: the tie window, `999999999999999.9`, and the off-centre pyramid with transfer on (unchanged in my metrics).

## Teacher view (PNGs looked at)

- **Default** (`default-Medium-1.3x.png`, `-1x-bw66.png`): the top level is a solid I-beam on screen and in print, and
  the arrows and percents are clean. The caption takes three lines (LOW 2).
- **1000:1 Large** (`ratio1000-Large-1x-bw66.png`): every level survives the print copy. The caption takes four lines.
- **Student copy** (`default-student-Small-1x-bw66.png`, zoomed 4×): the blanks, `%` signs, slivers and leaders all
  print at Small 1x.
- **Equal steps, six levels** (`six-equal-Medium-1.3x.png`): clean, and unaffected by this round.
- **Long names, Large, student** (`energypyramid-six-long-large-student.png`): the caps stand clear of the names, and
  the 15-digit values print in full.
- **Two thin levels** (`two-thin-Medium-1.3x.png`): Mice and Owls look identical (LOW 1).

Would a biology teacher drop it into an exam now? Equal steps, yes. Proportional, yes, on screen and in a 100 % print,
as long as no question asks students to compare the widths of the two smallest levels.

**Single biggest remaining gap:** a widened bar is not marked as widened (LOW 1). The page tells the student some bar
was widened but not which one.

## Regression and census

- **Suite on my copy of e29ba45:** lib **1975 passed, 0 failed, 5 ignored** (666 s). `absurd_windows_cannot_run_away`
  passed on the first run. The module runs 27/27, and 32/32 with my 5 harness tests.
- `git diff --stat 2060f43 e29ba45` lists only the energy-pyramid docs, `energypyramid.rs`, `faq.rs` and the census.
  `screens.rs`, `theme.rs`, `fonts.rs`, `main.rs`, `examples/`, `Cargo.toml` and `Cargo.lock` are untouched. There is no
  dependency and no param or saved-format change.
- **Census:** `census_baseline` passes on my copy, so the fixture equals the recomputed hashes. The fixture diff changes
  only `energy-pyramid__defaults/__start` (87a3bebc05d097f1 → b241c17c237731d9). `energypyramid` is imported only by
  the biology registry and the warnings dispatch, so the food web and every other figure render byte-identically. I did
  not run `census_dump` file by file.
- **Agreement:** 12,000 random inputs (new tokens included) plus 10,608 sweep renders. Canvas, note and `read()` agree
  every time, with 0 panics and 0 XML parse failures.
- **Test diff:**
  - No test was deleted.
  - Width tolerances were tightened from 0.5 px to 0.011 px.
  - The leak test's `||` was removed.
  - The 1000:1 check now asserts both the 0.4 to-scale value and the 1.5 drawn width.
  - No assertion was loosened.
- **Mutants (10, all killed):**

  | Mutant | Killed by |
  |---|---|
  | `MIN_DRAWN = 0.4` | 3 tests incl. `the_top_level_survives_the_print_copy` |
  | sliver drawn grey `#bbbbbb` at 1.5 px | print-copy test, sliver test |
  | arrows drawn with transfer off | leak test, layout test |
  | column always reserved | leak test ("narrower") |
  | `SLIVER_NAME_GAP = 6.5` | `the_layout_holds_across_the_family` |
  | aria-label removed | the exponent test |
  | spaced-exponent strip reverted | the parser test |
  | `1e-400` message reverted | the parser test |
  | disclosure never shown | 2 tests |
  | sliver left-anchored (asymmetric widening) | 2 tests |

## Not checked

- The egui editor live, and what a real screen reader says for the aria-label (I checked only the attribute).
- Print at the app's real export DPI (only 1x and 1.3x).
- RTL names visually.
- `census_dump` file by file.
