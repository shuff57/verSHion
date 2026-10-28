# Circular motion: combined critic, 2026-10-28

One combined critic (accuracy + teacher + regression, read-only, own target dir and PNGs) over `circular-motion`
(0e4624e). Verdict: FAIL — 1 blocker, 2 majors, 9 minors.

## Blocker
**The key printed false zeros at reachable settings.** At the minimum speed (0.2 m/s) the acceleration read `0 m/s²`
for a real 0.0267, the force `0.1 N` for 0.0533 and the angular speed `0.1` for 0.1333; at default precision a force
of 0.0008 N read `0 N`, and at the fast end a period of 0.0419 s read `0 s`. A decimal-places minimum does not fix
this — the value can be smaller than one unit of the last place. **Fixed**: the readout prints more places rather than
ever rounding a real value to zero, with a test on the minimum-speed, maximum-mass and default cases.

## Majors
| # | Finding | State |
|---|---------|-------|
| M1 | At Large label size the givens line collided with the first readout row (fixed 14 px gap, fonts to 21 px). | **fixed**: the gap scales with the font. My first attempt scaled only the position, which moved both lines together and changed nothing — the render caught it. The PNG export now includes a Large case. |
| M2 | The acceleration label sat on the solid radius line and fused with it in a 66% bitonal copy; the radius was indistinguishable from the acceleration vector. | **fixed**: the radius is dashed and stops where the acceleration arrow begins; both vector labels sit beside their arrows with a white halo. |

## Minors
- The velocity label touched its own arrowhead at 285–305° — **fixed** by the same label offset plus halo.
- The blurb promised a force arrow that is not drawn — **fixed**.
- The Decimal places control offered 0 while the code used 1 — **fixed**: the control now offers 1 to 4.
- No test covered the vector geometry — **fixed**: a test asserts the velocity is perpendicular to the radius and the
  acceleration radial at eight angles.
- Left open: malformed share links (a string in a float field) fall back to the clamped minimum rather than the
  default; the 66% check rasterises only at 1.3x; the rotation sense is not stated; readout spacing is tight at
  Medium; the FAQ's satellite/gravity sentence is loose.

## Verified correct
- `a = v²/r`, `F = mv²/r`, `ω = v/r`, `T = 2πr/v`, and `T·ω = 2π` exactly, recomputed independently and matched
  against every rendered example.
- Geometry: the body is exactly on the rim at every angle tested; the velocity is exactly perpendicular to the
  radius; the acceleration passes exactly through the centre; the two arrows are 90° apart.
- No leak with Show answers off: both arrows are fixed lengths independent of the parameters, so neither encodes an
  answer (the neighbours' blocker class does not apply here).
- Print is neutral; at 66% the circle, both shafts, both heads and the v label survive.
- No panic at any extreme or malformed input through the load path; the suite and census are green.

## Test-suite note
`figuresui::tests::dragging_a_label_moves_it_as_one_undo_step` failed in two full parallel runs (1735/1 and 1733/1)
and passed every time it ran alone and in both clean full runs (1734 and 1736). It is timing-sensitive under CPU
contention, not a regression; the proper fix is for that test to drive enough frames to settle rather than depend on
scheduling.
