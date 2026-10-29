# Ray diagram and motion map: combined critic, 2026-10-29

One combined accuracy + teacher + regression critic over `ray-diagram` (65e8082) and `motion-map` (1e9451f), at
`1e9451f`, read-only, in its own `git archive` copy. Verdict: **FAIL — 4 blockers, 11 majors, 12 minors.** It
recomputed every value independently: the *physics* was right in both figures at every setting it tried; what failed
was what the figures **printed** and what they **drew** at their extremes.

Fixed in the commit after. Its own report is at `/tmp/crit-m-report.md` (its working copy; not kept in the repo).

## Blockers

| # | Finding | State |
|---|---------|-------|
| B1 | u = 2f reported "Real, inverted **and smaller**" with m = −1 — in the shipped example whose whole point is that the image is the same size. | **fixed**: a magnification within 2 % of 1 is "the same size". Test added. |
| B2 | The printed magnification and image height disagreed with the printed object height: "Magnification −0.7, image height −2.7 cm" beside h = 4 cm, where −0.7 × 4 = −2.8. Widespread (diverging u = 60: 0.1 and 0.6). | **fixed**: `printed()` computes each number from the previous printed one — the magnification from the printed image distance, the height from the printed magnification. Test checks four settings. |
| B3 | The motion map printed a = 2 m/s² in the givens and "Acceleration −2 m/s²" in the readout for the same slowing figure. | **fixed**: the givens print the signed acceleration. |
| B4 | Float noise in the gaps then the never-zero fallback printed "0.1, 0.1, 0.1, 5.00e−2, 5.00e−2 …" on a **constant-velocity** map, under the line "Equal gaps: constant velocity". | **fixed**: gaps are computed from the velocities where that is exact (mean velocity × dt) and from the positions only where the stop clamps them; the formatter tries two more decimals before exponential form. Test asserts equal gaps print equally and never in scientific style. |

## Majors

| # | Finding | State |
|---|---------|-------|
| M1 | Near the focal point the scale's lower clamp broke the fit: u = 10.5 put the image at x = 675 on a 640 canvas, u = 10.2 at 1125, u = 9.8 at −375. | **fixed**: the scale is computed from the fit with a low clamp (0.3), so the image stays on the canvas; the drawing compresses instead. Test sweeps u = 9.8, 10.2, 10.5, f = 40 with u = 2, and f = 4 with u = 60. |
| M2 | The canvas height ignored the image height, so a tall image was clipped (f = 10, u = 6, h = 10 → label at y = −166). | **fixed**: one uniform scale is fitted to the vertical extent as well as the horizontal, and the axis is centred in the band. |
| M3 | Label collisions at the extremes ("imtbage", the image over F′, the object over the lens caption). | **fixed in part**: captions are dropped when they would print into the lens caption, the image label is clamped to the canvas, and the focal labels are dropped when the two foci are closer than 24 px. |
| M4 | The motion map's scale clamp drew fewer dots than the readout listed: v0 = 10, a = 5, dt = 1, n = 8 put dots 6–8 off a 620 px canvas. | **fixed**: the scale fits every dot (low clamp 1.0). Test checks the last dot's position for two extreme settings. |
| M5 | Flash numbers overprinted when dots coincide (six dots at the stop). | **fixed in part**: numbers sit on two rows and a number is skipped when it would land within 20 px of the previous one on its row. |
| M6 | Student copies leaked: the ray diagram printed the real/virtual sentence, the motion map printed the acceleration and "v throughout". | **fixed**: the nature sentence is part of the answer (a ______, ______ and ______ line with answers off), and the motion givens keep only the strobe interval and the motion type. Test asserts no `v0`/`a =` on the student copy. |
| M7 | The virtual image's arrow was drawn solid while the FAQ promised a dashed image. | **fixed**: the virtual arrow is dashed too, which is what survives a Print copy where the colour cue is gone. |
| M8 | The lens symbols were swapped: the converging lens had its heads pointing inward. | **fixed**: converging points outward, diverging inward. |
| M9 | The motion map's handle inverted an unclamped parabola while the flashes clamp at the stop, so grabbing the knob changed v0 (5 → 8) or did nothing. | **fixed**: where the later flashes are at the stop the handle is not offered; a test asserts the default has one and a stopped map has none. |
| M10 | Two example names were false: "Speeding up from rest" starts at 0.5 m/s, "Slowing down until it stops" never stopped. | **fixed**: renamed to "Speeding up from 0.5 m/s", and the stopping example now stops (dt = 0.5, n = 6). |
| M11 | The FAQ said the readout gives the stopping time and distance; that is the editor's Notes box, not the figure. | **fixed**: the FAQ says so, and that the dots at the stop carry no arrow. |

## Minors

Left open (recorded): the diverging lens's F/F′ naming follows one of two conventions and the far focus is never used
by a ray; the module doc said "three principal rays" where a diverging lens draws two (corrected in the doc); the motion
givens do not name the motion type for a constant-velocity figure (they do now — "steady"); a gap is the mean of the
neighbouring velocities times dt, which the label does not say; `fmt_qty`-style scientific notation is not used in
either figure (fixed by B4's formatter); arrows are scaled per figure with no legend, so all constant-velocity maps
look alike; the ray handle's gain changes with the scale near the focal point; the motion drag is quantised to
0.25 m/s.

## Verified correct (so silence is not approval)

- The lens equation and the signs of v, m and h′ for f = 10 at u = 5, 6, 10, 15, 20, 25, 40, 60 and the same u for a
  diverging lens; u = f gives no image with genuinely parallel rays; a diverging lens is always virtual, upright and
  smaller; all drawn rays and dashed extensions meet the computed image to < 0.01 px; the third-ray rule is right for
  a converging lens with u > f.
- The motion kinematics x = v0t + ½at², v = v0 + at, gap = mean velocity × dt, the stop clamp and v0²/(2|a|) are
  correct; nothing reverses; stopped dots carry v = 0 and no arrow.
- Both figures are registered in all four places, neither is in `NO_HANDLES`, and both return handles.
- Print and the 66 % bitonal copy are legible for the defaults and all five examples of each; the dashed virtual lines
  survive thresholding.
- No printed value falsely rounds to zero, and exact zeros are only where the physics has one.
