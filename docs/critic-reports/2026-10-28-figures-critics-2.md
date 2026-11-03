# Figures 1.3–1.5 and phase 2: combined critics, 2026-10-28

Two combined critics (accuracy + teacher + regression, read-only, own target dirs) ran on the seven figures
committed after 1.3 round 3. Verdicts: both FAIL. This file records the findings and what was done about them.

## Scope
- Critic A: 1.3 round 3 `2cc0a58` (trig), 1.4 `634df72` (sincos), 1.5 `bb3e275` (unit circle).
- Critic B: `e7eac7c` (transformations), `ed8f6b2` (fraction models), `4786301` (triangle centres),
  `0f8a07a` (slope triangle).

## Blockers, and what was done

| # | Finding | State |
|---|---------|-------|
| A1 | sincos printed an equation whose B was rounded to the readout precision, so the equation described a different wave than the one drawn and than its own Period row (period 1080 printed as B = 0.3, implying 1200°). | **fixed**: B prints at 4 dp and A/D at one decimal at least; a test round-trips 360/B to the printed period at every period and dec. |
| A2 | Composite leaks: a Given equation printed an Asked amplitude/midline; a Given point P printed an Asked cos/sin on the student copy. | **fixed**: both composites are blanked unless all their parts are Given (or the key); a test for each. |
| B1 | The slope line was clipped only in x, so it ran through the readout and off the canvas. | **fixed**: clipped to the plot. |
| B2 | Slope printed `Gradient m = 0`, `y = 0x − 1` for m = 0.5 because `dec` defaulted to 0 while the control steps by 0.25. | **fixed**: `dec` defaults to 2. |
| B3 | The transformations student grid did not contain the image, so the exercise could not be done on paper. | **fixed**: the grid covers the image whether or not answers are shown. |
| B4 | Dilate by 0.5 printed whole-number coordinates for a half-unit image. | **fixed** by the same precision change. |

## Majors, and what was done

| # | Finding | State |
|---|---------|-------|
| A3 | Large labels made unit-circle value labels collide with axis numbers (46 pairs over 44 angles). | **fixed**: a ladder of candidate positions either side of the point; the sweep test now runs every whole degree at Small, Medium and Large. |
| A4 | sincos warnings were produced but never displayed (the Notes box listed three figure ids). | **fixed**: `sine-cosine` and `slope-triangle` added to the Notes condition; the false "second cycle past the axis" warning removed. |
| A5 | A cosine starting at a peak drew the amplitude/period markers on the y-axis. | **fixed**: the markers move to the next peak when the first sits on the axis. |
| A6 | In Print the unit-circle coordinate values were indistinguishable from axis tick numbers. | **fixed**: the labels are prefixed `x =` and `y =`. |
| A7 | 360° folded to 0°, so the documented 2π was unreachable. | **fixed**: a typed 360 is kept as typed (2π, point at (1, 0)); the control cannot reach beyond it. |
| A8 | The sincos FAQ claimed a 90° right shift turns sine into cosine; it gives minus cosine. | **fixed**: the wording now says so. |
| B5 | Reflection in y = x or y = −x never drew the mirror line. | **fixed**: drawn dashed. |
| B6 | An image side painted through an axis number in the key. | **fixed**: axis numbers are drawn after the shapes. |
| B7 | Large labels cut the readout off the canvas. | **fixed**: the canvas widens with the label size in transform, triangle centres and slope. |
| B8 | Triangle centres had no Show answers at all, so no student copy existed. | **fixed**: added; it blanks the dot, its name, the coordinates and the radius. |
| B9 | Triangle centres had no axis numbers. | **fixed**. |
| B10 | The orthocentre's altitudes stopped at the centre instead of reaching the sides. | **fixed**: drawn to the feet with right-angle marks. |
| B11 | Slope's triangle could leave the plot (steep gradients, large runs). | **fixed**: the anchor moves to a position that fits, with a note when none does. |
| B12 | Slope with m = 0 drew a degenerate right angle and put the rise label on the line. | **fixed**. |
| B13 | Fraction printed `0.62` beside `62.5%`. | **fixed**: the percentage is derived from the decimal as printed. |
| B14 | Print grids vanished in a 66% bitonal copy (five figures). | **fixed**: the Print grid is `#9a9a9a`, dark enough to survive the threshold. |

## Verification round (same day, on `e72a4bc`)

A verification critic re-checked that commit independently (own target dir, own PNGs, a probe linking the lib).
It confirmed the suite at 1685 green and re-derived most fixes: the equation round-trip over 17 periods, the two
composite blanks, the slope line staying in the plot across 220 settings, the transform grid across 148, the
unit-circle label sweep across 2160 renders, the fraction decimal/percentage pair across 2496 cases, and the
triangle-centres student copy and altitudes.

It found three fixes that did not hold and three defects the fixes introduced; all six were fixed in `4ff5ea5`:

| | |
|---|---|
| A5 not fixed | the cosine markers were still on the y-axis — the guard tested a condition that could never be true |
| B14 not fixed | the Print grid still vanished at the 1.3x raster (#9a9a9a blends to 179); darkened to #787878 |
| B11 partly fixed | the slope triangle still left the plot in 638 of 3444 settings; legs are now clipped to the plot |
| new major | the longer triangle-centres sentences ran to 700 px on a 470 px canvas and were cut off; shortened |
| new minor | the zero-rise slope label moved but its collision box did not |
| new minor | slope's Decimal places control offered 3 while build clamped to 2 |

Two critic rounds are now spent on this batch, which is the cap agreed for these figures. The remaining risk is
that no automated test pins 66% grid survivability; it is verified by measurement and the colour choice, not by a
test. That is recorded as a gap rather than closed.

## Left open (minors, recorded here and in the progress doc)
- sincos axis text goes through `chart::text`, whose ink is `#111827` (grey within the repo's spread of 24, but
  not exactly neutral); the module's Print test checks strokes only.
- trig's key prints a Hidden side inside a Given ratio's fraction (`sin θ = 7/7.3`); the student copy is clean.
- sincos can land on only two y tick labels for tall waves (a finer step would help read an Asked midline).
- unit-circle coordinate labels can sit over the drop lines at some angles (halo-compliant, but visible).
- Example names that overstate what they show ("45 degrees both ways" opens in Degrees only).
- trig prints thousands separators inside fractions (`1,234.5/2,650.63`).
- transform label crowding where a pre-image vertex and an image vertex coincide.
- transform prints `+ 0` for a zero translation and `1x, 1y` for scale 1.

## Verified correct (so silence is not approval)
- trig: every worked example, the 10–80 clamp, lengths at ≥2 dp, the printed fraction/right-triangle consistency,
  Print exact neutrality, 66%, no student-copy leak across all examples.
- sincos: the drawn curve against its equation at all four presets, the arrows spanning exactly A and one period,
  a 2880-case sweeps of marker labels, Print strokes neutral, the 66% copy keeps everything.
- unit-circle: cos/sin at 16 angles, the pi fractions, the drag handle, student-copy blanks, Medium and Small clean
  at every degree.
- transformations: all four operations and every option against the definitions; no image coordinate on the
  student copy; the key's grid holds both shapes.
- fraction: clamping, drawn part counts, and the decimal/percentage pair at dec 3.
- triangle centres: all four centres independently recomputed and checked against their definitions, the radii,
  and the Euler relation; no panic at extremes.
- slope: rise = m × run across the tested range; both corners on the line; the equation's signs.
- Registration and regression: census diffs purely additive at each commit; no test deleted; the module tests and
  the shared gates (handle coverage, extreme numbers, Print grey) pass.
