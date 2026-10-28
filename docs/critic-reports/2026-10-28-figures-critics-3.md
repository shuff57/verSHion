# The four uncritiqued figures: combined critic, 2026-10-28

One combined critic (accuracy + teacher + regression, read-only, own target dir and PNGs) over the figures that
phase 2 added after the first critic round: `protractor`, the `fraction-models` **area model**, `base-ten-blocks`
and `solid-net`. Verdict: FAIL — 2 blockers, 8 majors, 13 minors.

## Blockers, and what was done

| # | Finding | State |
|---|---------|-------|
| B1 | **Nets could not be answered on paper.** The net is scaled to fit its box, so every similar box renders the identical picture (a cube of edge 1 and of edge 3 were byte-identical), and the student copy blanked the edges — leaving no numbers at all. | **fixed**: the edges stay on the student copy, as the given; the FAQ no longer says the net can be measured. |
| B2 | At 0 decimal places a half-unit edge printed `Edges 0 x 3 x 2`, and a real volume rounded to `0`. | **fixed**: edges, area and volume print at one decimal at least; a test covers both solids at dec 0–2. |

## Majors, and what was done

| # | Finding | State |
|---|---------|-------|
| M1 | The protractor's ray was drawn over the scale numbers and struck through both "90"s and the 120/60 pair. | **fixed**: the ray is drawn under the numbers, and every scale number now carries a white halo. |
| M2 | Nothing marked which baseline arm is the reference, so 55 and 125 were both defensible readings. | **fixed**: the right-hand half of the baseline is drawn heavier as the fixed arm; the FAQ says so. |
| M3 | The readout ignored the drawing: a protractor with no ray still printed a reading, and a hidden second scale still printed "the other scale reads". | **fixed**: no ray prints "No ray drawn"; the second row is suppressed when that scale is not drawn; a test covers both. |
| M4 | The fraction percentage was derived from the already-rounded decimal, so 1/12 at 0 dp printed `Decimal 0 / Percent 0%`. | **fixed**: a fraction prints at least two decimals, so the decimal can never be 0 for a nonzero fraction. |
| M5 | At 66% the shaded cells merged into one black shape, losing the subdivisions — the point of the model. | **fixed**: Print shades with a diagonal hatch pattern; the bitonal copy keeps every cell distinct. |
| M6 | Base-ten's student template was always three blanks while the key dropped zero places, so student and key disagreed (305: three blanks vs two terms). | **fixed**: every place is named in both copies, zeros written as 0 — `3 hundreds + 0 tens + 5 ones` against `_____ hundreds + _____ tens + _____ ones`. |
| M7 | The net's faces carried no dimensions, so a teacher could not point at a face and name it. | **fixed**: each face is labelled `a x c`, `b x c` or `a x b`. |

## Left open (minors)
- The protractor example named "read on the inner scale" shows nothing special at 90°.
- At the 1.3x raster the inner "120" loses its leading digit; it survives at the app's 2x export scale.
- Half-degree angles: the complement uses the rounded reading (124 instead of 125).
- Dead constants (`H`, `ROWS_Y0`) in three of the modules.
- Base-ten: the tens/ones labels sit 8 px below the blocks and their ascenders can touch the rods at Large; the
  "hundreds" label is not centred for one or three flats.
- `n = 0` prints "Number 0", which is also the answer.
- The nets blurb says "three-digit number" only for base-ten; nets carry no units and no stated scale.
- Protractor tests still do not render Large or the inner-off key at every state.

## Verified correct (so silence is not approval)
- Suite 1707 passed with an independent target dir; census baseline matched at the commit under review.
- Protractor: reading and complement always sum to 180, tip on the rim at 18 angles including 0, 0.5, 89.5 and
  180, both scales printing the same 0..180 set; no panic on NaN or extremes.
- Area model: every den 1..12 with every num and dec 0..3 gives a grid whose dimensions multiply to the parts and
  are closest to square, all cells equal and on canvas, exactly num shaded in reading order.
- Base-ten: all 1000 values have the correct places and expansion; the block counts match; the flats really are
  ten by ten.
- Nets: for all 7^3 edge combinations the six faces are exactly {a×c, a×c, b×c, b×c, a×b, a×b}, they share the
  right edges, and the net folds to the box; area and volume match the formulas.
- No leaks on any of the four student copies (the one reverse issue was B1, fixed).
- Print neutrality: every fill and stroke in all four default Print renders is exactly neutral.
