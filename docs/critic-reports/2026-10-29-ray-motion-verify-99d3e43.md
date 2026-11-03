# Verification critic on 99d3e43: what it found

A verification pass over the fixes commit `99d3e43` (the ray diagram and the motion map), read-only, in its own
`git archive` copy, with its own harness tests. Its verdict: **4 blockers fixed, 7 majors fixed, M1 and M3 only
PARTLY**, and ten new defects, three of them serious. The blockers were genuinely fixed — this pass existed to find
out whether they were, and it did find the two that were not.

## The eight it confirmed fixed, with evidence

| # | Verdict | Evidence it gave |
|---|---|---|
| B1 u = 2f "same size" | FIXED | f = 10, u = 20 prints "Real, inverted and the same size"; its test fails on the old code |
| B2 printed m × h vs h′ | FIXED | at 1, 2 and 3 decimals the printed magnification times the printed object height equals the printed image height |
| B3 givens vs readout `a` | FIXED | a slowing case prints −2 m/s² in both places |
| B4 gaps 0.0 / scientific | FIXED | equal gaps print equally and never in exponential form |
| M2 tall image clipped | FIXED | f = 10, u = 6, h = 10 puts the image tip at y = 30 where it used to be −166 |
| M4 dots off the canvas | FIXED | the fastest map's last dot is at x = 510 on a 620 canvas |
| M6 student-copy leaks | FIXED | neither student copy prints an answer |
| M7 virtual arrow solid | FIXED | two dashed extensions and a dashed arrow shaft; a real image has none |
| M8 lens symbols | FIXED | converging heads outward, diverging inward |
| M9 handle at the stop | FIXED | no handle where the model clamps, one where it inverts |
| M10 false example names | FIXED (the two named) | both names are now true of their parameters |
| M11 FAQ claims readout | FIXED | the FAQ now names the Notes box |

## The two that were not fixed

- **M1 (partly).** The scale's lower clamp still broke the fit when the focal length is long: f = 40 with u = 40.5
  left the image at x = 1332 on a 640 canvas, and u = 39.5 at x = −588. 2,120 reachable settings were affected. The
  existing test's bound was `W * 1.7`, loose enough to pass on the old bug.
- **M3 (partly).** Label collisions remained at ordinary settings: f = 10, u = 60 printed "F′image"; f = 4, u = 15,
  h = 0.5 printed "F′image"; u = 2, h = 0.5 printed "imaget". Roughly 17,000 and 26,000 overlaps in its sweep.

## New defects the fixes introduced (its ranking)

1. **HIGH** — the M1 residue above: the image off the canvas while the readout quotes its distance.
2. **HIGH/MED** — the M3 collisions above.
3. **MED, new** — the vertical fit ignored the lens: f = 4, u = 4.5, h = 5 put the lens at y = −18.9 with its top
   arrowhead off the page (300 settings).
4. **MED, new** — a tall image pushed the axis down until the lens crossed the readout (1,300 settings).
5. **MED, new** — at one decimal place f = 10, u = 20.5 prints "Magnification −1" beside "smaller", because the
   sentence used the exact value while the readout used the rounded one.
6. **LOW/MED** — a 0.05 m gap printed as "0.1" at one decimal place.
7. **LOW** — the motion drag quantised to 0.25 while the control steps in 0.5, so the knob did not follow the slider.
8. **LOW** — the fifth motion example's name was still false ("the same motion" with v0 = 3 against the default 2).
9. **LOW** — six dots in 22.5 px print only numbers 1 and 2 (pre-existing).
10. **LOW** — at u = f the student copy said "No image: the rays leave parallel" twice (pre-existing).
11. Test-quality gaps: M5, M7, M8, M11 and B3 had no regression test; M1's bound was too loose; B2's tolerance would
    have passed the old bug.

## Fixed in the commit after

- **The image can no longer be off the page unannounced.** The scale is fitted with a floor; when even that floor
  cannot show the image the figure draws the rays leaving the frame and names the image at the edge ("image 3,240 cm
  to the right, beyond the canvas"), the readout keeps quoting the same distance, and the editor gets a note. With
  answers off the distance is a blank.
- **One placer for every label.** The lens caption, the foci, the object and the image are placed through a
  collision-aware placer: a label tries its own position, then 15 px up and down, and is dropped rather than printed
  over another. A test parses the rendered labels of 96 settings and fails on any overlap or any label off the canvas.
- **The lens is part of the fit.** The vertical fit reserves `2 × LENS_H` about the axis, so the lens cannot run off
  the top or cross the readout; a test sweeps 96 settings.
- **The ray lines are clipped to the drawing band**, so a near-parallel ray no longer prints through the readout.
- **The nature sentence uses the printed magnification**, so "Magnification −1" can no longer sit beside "smaller".
- **Gaps print at two decimals at least**; the motion drag rounds to the control's own 0.5 step; the fifth example is
  renamed "Eight flashes at 0.25 s intervals"; the duplicated "no image" line is gone.
- **New tests** for: the lens symbol direction (parsed from the drawn polygons), the dashed virtual image, the
  off-canvas note and its student-copy blank, the signed acceleration in the motion givens, and the small-gap print.
  The canvas test's bound is now the canvas.

Census after this commit: `hardy-weinberg__defaults` and `hardy-weinberg__start` added, and `ray-diagram__defaults`
and `ray-diagram__start` re-hashed — the ray geometry changed, which is what the census is for.
