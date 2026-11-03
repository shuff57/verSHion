# Critic 1 on b5ebb1a: energy pyramid (accuracy, teacher, regression)

A first critic pass over `b5ebb1a` ("figures phase 4: energy pyramid (typed levels, proportional or equal steps,
transfer percent as an answer)"), scope `f42232f..b5ebb1a`. I ran it on my own `git archive` copy with my own
harness. The harness is a child test module of `energypyramid.rs` with its own SVG parser for rects, lines,
polygons and text. I also ran six mutants and rendered PNGs at 1.3x, plus their 66 % black-and-white copies, and
looked at them with the image reader: the module's own 21-case PNG test and 13 more cases of mine.

**Verdict: FAIL (0 blocker, 0 major, 2 MED, 5 LOW, 5 minor).** The machinery is sound. Bar widths match the typed
values. In 16,000 random inputs the canvas, `warnings()` and `notes_for` never disagreed. A 7,200-render layout
sweep found no text off the canvas, overlapping text or text crossing a bar, arrow or leader. Nothing leaks into
the student copy. Every refusal is 420 wide with a scaled header. The food web is untouched.

The two MEDs are about what a teacher and a student actually see and read:
1. In the default figure and in two of the six bundled examples, the top level is a 0.40 px bar sitting on the
   same x as the 1.2 px arrow shafts below it, so it reads as the arrow carrying on, not as a level.
2. The printed percent rounds exact ties to even (31.25 % prints `31.2`). The test that claims "the printed
   percent agrees with the printed values" uses `fmt_pct` as its own oracle, so it cannot catch this, and a
   half-up mutant passes all 21 tests.

## Claim table

| # | Claim / check | Verdict | Evidence |
|---|---|---|---|
| 1 | Proportional widths are the typed values | **CORRECT** | 400 px base; 1000:1 gives 0.40, 1000:999 gives 399.60, 1e21:1e18 gives 0.40, all centred on one axis. Mutants: `sqrt` widths fail 2 tests; a 0.6 px offset on levels above the base fails 3. |
| 2 | Ratio limit 1,000:1, Proportional only | **CORRECT at the number, wrong in its wording** (LOW 3) | 1000 and 1000.000001 draw; 1001 is refused; Equal steps takes 1e300:1e-300 with no NaN. |
| 3 | Equal steps flagged on the picture and in `<desc>` | **CORRECT** | Caption "Not to scale: the bars step in by a fixed ratio, so read the energy from the numbers, not the widths."; desc ends "Not to scale." (`energypyramid-equal.png`, `c-equal-student.png`). |
| 4 | Values inside if they fit with 10 px spare, else beside on an 8 px leader | **CORRECT** | Sweep below: 10 value sets (1e21, 123456789.123, 0.001/5e-4/2e-6, 1e-7, 1e15, 1000:1, 6 near-equal, the 6-level 10 % chain, mixed) × 5 name sets (short, 40-char, CJK, 30×M, 2-char) × 2 scales × 3 sizes × transfer × key × 4 units = 7,200 renders. Measured: **0** texts off the canvas, **0** text–text overlaps, **0** texts across a bar edge, **0** inside labels within 3 px of their bar's edge, **0** texts touching an arrowhead polygon, **0** leader/arrow/blank lines crossing a text box, **0** NaN/inf. |
| 5 | Percent hidden under SHOW_ANSWERS; student gets a blank | **CORRECT** | Student SVG (S/M/L × both scales) contains none of 16.2 / 11.4 / 5.48; `<title>` "Energy pyramid"; `<desc>` lists names and values only; the editor note is empty for a valid pyramid; arrow polygons are byte-identical between key and student. Mutants: always-key fails the leak test; a percent appended to `<desc>` fails it too. |
| 6 | Refusals structured, 420 wide, header scales, note agrees | **CORRECT** | 17 refusal inputs × S/M/L: width 420 every time. Header baseline 36.4 / 40.0 / 47.2 (= 22 + 18·lab). Bottom margin under the last baseline 23.2 / 24.0 / 25.6 px for fonts 9.6 / 12 / 16.8, so the food web's 9.2 px at Large is not repeated. Names containing "More than", "energy", "ratio" or "Nothing is drawn:" never change the second line. Random sweep: 4,000 texts built from names {A, B, a, Producers, "", C:D, #x, Ratio} and values {10000, 1,000, 1e3, 0, -1, x, 1,00, 1e-3, 1e300, 1e-300, "", 1.5 × 10^2, 1001, NaN, …} × 2 scales × Small/Large = **16,000**, and **0** cases where the canvas, `read()`, `warnings()` and `notes_for` disagree, or where the canvas lacks the note's reason. Mutant: `warnings()` built on `parse()` instead of `read()` fails the agreement test. |
| 7 | Numbers print as typed | **MOSTLY** (minor 1, LOW 5) | `1.5e6` prints 1,500,000; `1.5 x 10^6`, `1.5x10^6`, `1.5 * 10^6` and `1.5E+06` are read as 1.5e6; `123456789.123` prints 123,456,789.123; `1,00,000`, `1.000,5` and `1 000` are refused with a reason. |
| 8 | Regression | **CLEAN** | Diff: `energypyramid.rs` (new), `biology.rs` (+3 registration lines), `foodweb.rs` (`text_el` and `wrap` now `pub(super)`, nothing else), `mod.rs` (one `notes_for` arm), `faq.rs` (+arm), `interact.rs` (id added to the expected-id list), census (+2 lines), progress doc. No protected file is touched and no test is deleted or loosened. Census: see the last section. |
| 9 | Old links | **SAFE** | Figures are looked up by `id` (`registry().find(\|d\| d.id == id)` in `figfit.rs` and `figure_agent.rs`), not by index, and the new def is appended last in `biology::defs()`. |
| 10 | Code quality | **CLEAN, apart from minor 4** | The new module adds no compiler warning. Each `unwrap` sits on a value that cannot fail (`split_once` after counting exactly one colon; the test helpers). Hostile inputs (U+0000, RTL, 5,000-char names, 1e-323, ９) all render. |

## Defects (ranked)

1. **MED: the top level of the default pyramid reads as the arrow carrying on.** This covers the default, the
   bundled "An ocean with a huge producer base" example, and any 1000:1 pyramid with transfer on. Measured in
   `A: 1000 / 100 / 10 / 1` (Medium):
   - the top bar is `x=287.16 width=0.40` (solid black, no stroke);
   - the arrow shafts below it are `x=287.36 stroke-width=1.2`;
   - the last arrowhead's tip is at y=55 and the sliver's bottom at y=52.

   So the top "bar" is a third as wide as the arrow line under it, exactly collinear with it, and starts 3 px
   above the arrowhead. At 1x the sliver's pixels are luminance 136–206, and at Large 1x they are 195 and 206,
   both above the 66 % print cut-off of 168 (LOW 4).

   I looked at `energypyramid-default.png`, `energypyramid-ex1.png` ("Seals") and `c-ratio1000-large-bw66.png`.
   In each, the top level looks like a vertical tick continuing the arrow, with the name and value hanging off
   it. A student cannot tell that it is a bar, which is the whole point of Proportional. The 4 px "Secondary
   consumers" bar is only 3.3 times the shaft width, and in the bw66 copy it also reads as an outline.

   The honest-scale choice is fine; the confusion comes from the arrows sitting on the axis. Possible fixes:
   - move the transfer arrows off the axis (to the left of the narrower bar's name, or to the right of a fixed
     column);
   - or keep a visible marker for a sub-3 px bar (a short horizontal tick of its own, or caps), so that it can
     never merge with a shaft.

   The `SLIVER` rule (solid black under 3 px) makes this worse, because it makes the bar the same ink as the
   arrow.
2. **MED: the printed percent rounds exact ties to even, against school rounding, and the test meant to pin it is
   tautological.**
   - `fmt_pct` uses `format!("{x:.d$}")`, and Rust rounds an exactly representable tie to even.
   - Repro: `Producers: 8000 / Herbivores: 2500`. 2500/8000 = 31.25 % exactly, and the key prints **`31.2 %`**.
     A student who rounds 31.25 to three significant figures writes 31.3 and is marked against the key.
   - The same happens to every k/16, k/32 or k/64 that lands on a tie: 5/16 gives `31.2`, 9/16 `56.2`,
     13/16 `81.2`, 1/32 `3.12`, 2/64 `3.12`. I stopped listing at 20 cases among a/b for b ≤ 400, so there are at least 20.
   - `the_printed_percent_agrees_with_the_printed_values` compares the printed percent with
     `fmt_pct(printed_b / printed_a * 100)`, so it checks `fmt_pct` against itself. I replaced the rounding with
     half-up and **all 21 tests still passed** (mutant m4).
   - Fix: round half-up on the decimal value (or print the exact value when it has ≤ 4 significant figures), and
     give the test an independent oracle, such as exact rational arithmetic on the printed integers, with the
     ties 8000→2500 and 16→5 in its table.
3. **LOW: the ratio refusal's reason is false at its own boundary.**
   - The refusal says "a proportional bar for the smallest would be under half a pixel wide". The const doc says
     "Past this the smallest proportional bar is under half a pixel". The FAQ says "A bar cannot be drawn
     narrower than about half a pixel".
   - But the accepted 1000:1 case already draws **0.40 px** (400/1000), which is under half a pixel, and 1001:1
     would draw 0.3996 px.
   - So the case on either side of the line fails the stated test. Either the limit should be 800:1 (0.5 px), or
     the words should say "under 0.4 px" (or "too thin to see").
4. **LOW: in print at 1x the 0.4 px sliver drops out.** At Large and 1x, the pixel row through the top bar of
   `A: 1000 / 100 / 10 / 1` reads `[255, 195, 206, 91, 91]`, where the 91s are the leader. Both sliver pixels are
   above the 66 % threshold (168), so the 66 % B/W copy loses the top level and keeps only its leader. At 1.3x
   it survives (136; `c-ratio1000-large-bw66.png` shows a hairline). This is MED 1 again under print. A minimum
   drawn width (for example 1 px, with the caption saying so) would fix both.
5. **LOW: a byte-order mark breaks the first line.**
   - `"\u{feff}# note\nProducers: 10000\nB: 1"` is refused with "Line 1 has no value: write Name: value…",
     although line 1 is a note.
   - `"\u{feff}Producers: 10000\nB: 1"` draws, but the name carries an invisible U+FEFF.
   - Text pasted from a Windows file or some LMS exports starts this way. Fix: strip a leading U+FEFF in `parse()`.
6. **LOW: `99.9` truncates where three significant figures round up.** Every value from 9,995 to 9,999 of 10,000
   (99.95 % to 99.99 %) prints `99.9 %`. A student working to three significant figures writes 100 (or 99.95),
   and either disagrees with the key. The rule exists so that unequal bars never read as equal, which is
   reasonable. But it is not in the FAQ, and the key then disagrees with the student's correct rounding.
   Owner decision; at least document it.
7. **LOW: the student blank is small for handwriting.** The rule is 30·lab px long (30 px at Medium, about 8 mm
   at 96 dpi), centred in a 30 px gap, so there are about 12 px above the rule to write in
   (`energypyramid-student.png`, `c-small-student.png`; Small gives 24 px by 9.6 px of room). "16.2" fits in
   neat handwriting and "0.583" does not. Consider 40·lab, or a boxed blank.
8. **minor 1: some over-precise inputs print a different number from the one typed.** `10000.000000000001` prints
   `10,000.000000000002`, and `12345678901234567890` prints `1.2345678901234567e19`. These are limits of f64,
   reached only by unusual input.
9. **minor 2: `1.5 X 10^6` (capital X) and `1.5 × 10⁶` (a typed superscript) are refused, although `x` and `×`
   with `^` are read.** The message ("as 10000, 10,000 or 1.5e6") does show the way forward.
10. **minor 3: names are not quoted in the "more energy" refusal**, so "More than energy ratio: 1 / B: 2" reads
    "B has more energy than More than energy ratio below it." The second line is correct, though.
11. **minor 4: the leak test only looks for two hard-coded numbers in the `<desc>`** (`16.2`, `11.4`). It caught
    my mutant only because the first percent happened to be 16.2. A test that looks for every `fmt_pct` of the
    key in the student SVG and in `notes_for` would be general.
12. **minor 5 (nit): case-insensitive duplicates miss `ß`/`SS` and `İ`/`i`** (`to_lowercase`, not case folding).

## Owner decisions

- **A textbook 10 % chain of 5 or 6 levels is refused in the default scale.** `Producers: 10000 … Quaternary: 1`
  is 10,000:1, so the teacher has to discover Equal steps. The message says so, and the bundled 5-level example
  stays under the limit (625:1) for this reason. Options: switch to Equal steps automatically with the "Not to
  scale" caption, or keep the refusal.
- **Equal values are accepted (100 % transfer), but an increase is refused because "energy cannot grow".** 100 %
  is also impossible in biology. The test calls it "legal (if flat)". Decide whether a flat step should be refused
  (or at least captioned) as well.
- **`99.9` versus 3-sf rounding** (LOW 6), and **e-notation** (`1e21`, `5e-4`, `2e-6`) for values outside
  [0.001, 10^15) (`c-sci.png`, `c-tiny.png`). Exam papers write 1 × 10²¹; e-notation is calculator notation.

## Teacher view (PNGs looked at)

- **Default**: clean and to scale, but the top level reads as part of the arrow (MED 1). In
  `energypyramid-default.png` I first read "Tertiary consumers" as a label on the arrow.
- **Large student copy** (`energypyramid-large-student.png`): well spaced; the blanks are small (LOW 7).
- **Equal steps** (`energypyramid-equal.png`, `c-equal-student.png`, `energypyramid-six-equal-large.png`): this is
  the version a teacher will actually paste. It is clearly captioned. With six levels at Large, labels move
  beside the top three bars cleanly.
- **Long names at Large** (`energypyramid-long-names-large.png`): four-line names wrap tidily, and the arrows
  lengthen with the bands. Nothing collides.
- **CJK** (`energypyramid-cjk.png`): wraps to two lines, and there is no clipping. It is no worse than the food
  web; it uses the same `text_w`.
- **Print 66 %** (`energypyramid-default-bw66.png`): the grey fills drop to white, which is fine because the
  outlines remain. The 0.4 px sliver survives only at 1.3x (LOW 4).
- Would a biology teacher drop it into an exam? Equal steps, yes, today. Proportional: only after MED 1 is fixed.

**Single biggest scope gap:** only a pyramid *of energy* is offered. Pyramids of **biomass** and **numbers** are
equally common exam items, and they are the ones that can be inverted (one oak tree, thousands of caterpillars),
which this figure's "cannot grow" refusal forbids by design. A second biggest is the missing "energy lost as heat
/ respiration" arrow out of each level. Both are **nice-to-haves**, not defects of what was claimed.

## Regression and census

- Full suite on my copy, which includes my one extra harness test: lib **1959 passed, 0 failed, 5 ignored**, so
  1958 + 5 without the harness, matching the builder. The integration tests are clean. No flake, and
  `absurd_windows_cannot_run_away` passed on the first run.
- `census_baseline` passes.
- I recomputed the census instead of trusting the fixture: `census_dump` on both archives gave 5,780 SVGs from
  `f42232f` and 5,799 from `b5ebb1a`. `diff -rq` shows **only the 19 new `energy-pyramid__*` files**, and every
  existing file is byte-identical. That includes all of the food web's cases (defaults, start, its examples,
  every choice and switch flip). The fixture diff is exactly `energy-pyramid__defaults` and `__start`
  (c7d8466ed791a1a6). I did not render the food web's refusal canvases from both commits separately; the change
  to `foodweb.rs` is visibility only (`pub(super)` on two fns, with identical bodies), so it cannot change their
  output.

## Not checked

- The egui editor live (the controls, the hint text, the note panel rendering). I checked `notes_for` only.
- RTL/bidi names visually (they render, but I did not look at them).
- Release-mode timing.
- Output at the app's real export scale; my print checks are at 1x and 1.3x only.
