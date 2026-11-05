# Photosynthesis, critic 3, commit a78566c (branch figures-gapfill)

Verdict: FAIL. 0 blocker, 1 major, 1 MED, 4 LOW, 4 minor. Probes, PNGs and logs in scratchpad/.

## MAJOR
1. **The suite is red: the render changed and the census fixture was not reblessed.** `cargo test --lib figures::census::census_baseline` -> FAILED, `changed: photosynthesis__start` (live hash edb585c8388d7bbf, `tests/fixtures/figure_census.txt` 251b0d8a23bee58a; a78566c touches no fixture). The opening state is exactly the state whose gridline colours and widths this commit changed. Any `cargo test --lib` on the branch therefore fails, although the loop's gate is that it passes. Fix: `VERSHION_CENSUS_BLESS=1 cargo test --release --lib census_baseline`, commit the one-line diff.

## MED
1. **At "Decimal places for P" = 0 (a legal setting) the key prints a rate its own band refuses.** dec 0, Light, pmax 2, K 3, P 1: "Rate at P (x = 1 %): 1 arbitrary units" beside "accept any value from 0.5 to 0.6 arbitrary units". Also pmax 1/K 3/P 1 -> prints "0", band 0.25..0.3; pmax 5/K 10/P 53 -> prints "4", band 4.2..4.4. 3.27 % of 200,000 random states at dec 0 (0 % at dec 1-3), more for small plateaus. Cause: `accept()` snaps to gridlines while `rate_text()` rounds half up to `dec`; at dec 0 the rounding step (0.5) exceeds the fine spacing (0.05-0.5). A student who writes the key's own number is refused. No test compares the two: `the_rate_at_p_is_rounded_half_up_and_exact` itself prints "1" for pmax 2/K 3/P 1 at dec 0 and never looks at the band.

## LOW
1. **The caption/FAQ over-claims "whichever of the two neighbours a student reads is inside".** Temperature, pmax 10, optimum 30, P 30: exact 10, band "10 to 10.5"; the neighbours of 10 are 9.5 and 10.5 and 9.5 is outside. 20.6 % of 29,700 typed states are within 0.05 of a gridline; exact-on-line states occur whenever the rate lands on one (every temperature optimum whose peak is a multiple of the fine spacing; Light pmax 97.46/K 11.23/P 37.5 gives exact 75, band 75..80). "Read to the nearest fine gridline" still works; the stated reason does not.
2. **The Colour key still has the faintest gridlines, and the new test skips exactly that state.** #b8c0cc is 1.83:1 on white; at 1x its own pixels render at 190-206 of 255 (1.24-1.28:1 effective), and across the plot only 3.8 % of columns are darker than 66 % grey (those are the curve's crossings, not the line). The test runs (Colour, student), (Print, student), (Print, key) - never the default opening state (Colour, key). The FAQ's "the fine gridlines are ... dark enough to survive a photocopy at 66 %" is false for the key. The student sheet is fixed; the key/student jump (1.24 vs 2.1:1 effective) is the part still odd.
3. **A doubled equation that satisfies the stated rule is not refused.** 12CO2 + 12H2O -> 2C6H12O6 + 12O2 has no substance on both sides and is absent from the "Not accepted" clause; only "for making one molecule of glucose" rules it out. After the fix the clause reads as exhaustive.
4. **"1,500 ppm" and "1 000" in a box that does not take ppm are "not a number".** The new unit path fires only when the unit belongs to the box: Light mark "1,500 ppm" -> `not a number`, losing both the ambiguous-comma advice and the unit name; only "400 ppm" -> `ends with "ppm" ...`. LOW 4's fix is partial.

## minor
- The P letter's white pad punches a visible white notch in the darker student grid (a plate over the grid, not only over a curve).
- The caption sentence is 43 words with a nested parenthesis ("the fine gridline at or just below the exact rate up to the next one"); parseable, but one read is not enough.
- The student Print sheet (24 horizontal lines, 1.4-1.6 px, every 6.5-9.6 px) now reads as ruled paper; the curve is still the darkest thing, but not by much.
- `docs/figures-progress.md` still states the old "half of the fine spacing either side" rule.

## verified (independent method)
- MED 1: gcd(6,12,1,6,6)=1; atom count says the refused form is balanced and subtracting 6 H2O gives the accepted form; key, FAQ, student sentence and tests agree; no "smallest"/"larger coefficients" left in user-facing text; "light left out" is answered.
- MED 2: I rewrote nice_step, minor_divisions, y_axis, the curves and the band from the prose (Python, 200,000 states; a Rust probe through the real code path (read_graph/accept, 29,700 typed states)). Band = [floor(r/fine)*fine, +fine] in every state (the single flag was my own float 1 ulp below a gridline at exact 75, where the code's integer band was right); exactly one spacing wide; every gridline within half a spacing inside; the third outside; hi <= axis top always; printed ends parse back to gridline values. The 5.769 case now accepts both 5.5 and 6.
- MED 3: colours and widths in the SVG as claimed; contrast recomputed 2.52/1.83 (Colour) and 6.90/4.29 (Print). At 1x every student-copy fine line survives 66 % (97.3-100 % of columns, darkest pixel 121-155).
- LOW 1: "-1,500" prints "-1.5 or -1500" and "Neither ... 1 to 100" is true; typing -1.5 gives the below-range message. LOW 2: ends are gridline values in all states.
- LOW 4: "20 ppm", "20 pct", "30 degF", "10 mg" name the unit and the units the box takes; the advice types back.
- Hostile matrix: 46 inputs x 9 boxes = 414 results; every extractable advice value types back, including both readings of every comma.
- Biology vs OpenStax 8.1: stage locations, O2 out of the water-splitting stack, ATP/NADPH to the Calvin cycle and ADP+Pi/NADP+ back, G3P as the direct product, the 12 C fall labelled "a feature of this simplified curve". Unchanged by the fix.
- Student copy leaks nothing: no answer word or rate in the whole SVG source.
- Module tests: 53 passed, 0 failed, 1 ignored. Census is the only red test.
- No param, default or share encoding changed: the origin/main CO2-link question is exactly as critic 2 left it (owner decision, not re-reported).

## mutants run (3, all killed)
FINE_SW 1.4 -> 0.7: killed by the 1x 66 % pixel test (2 of 530 columns on the first line). Student Colour copy keeps the light greys: killed ("the student copy takes the Print greys"). Band starts at the nearest gridline instead of the one below: killed ("exact 78.838 outside 80 .. 85"). The dec-0 hole has no test to kill it - that is the MED.

## not checked
egui panel and dark theme, a real printer at 150/300 dpi, browser fonts, the rest of the full lib suite, whether any CO2 share link exists in the wild.

## biggest gap vs a textbook figure
The chloroplast still shows where, not how: no lumen, photosystems, electron transport or ATP synthase, and nothing inside the Calvin loop (no RuBP, fixation/reduction/regeneration); the light and CO2 curves have no compensation point, so the rate is gross photosynthesis and never says so.
