# Electric field: combined critic, 2026-10-28

One combined critic (accuracy + teacher + regression, read-only, own target dir and PNGs) over `electric-field`
(57077a3). Verdict: FAIL — 1 blocker, 4 majors, 7 minors.

## Blocker
**The equipotential labels were the reciprocal of the truth.** They read `V/1`, `V/2`, `V/3` from the outer circle
inwards, while the potential rises towards the charge as 1/r: at q = 12 nC the true values on the three circles are
in the ratio 1 : 2 : 3 inward, and the labels said 1 : ½ : ⅓. As printed the figure taught that the potential *falls*
towards the charge — the opposite of its own FAQ, and of the commit message, which carried the same conflation.
**Fixed:** the labels read `V`, `2V`, `3V` inward (negated for a negative charge), and each now sits in its own
direction so they cannot collide.

## Majors
| # | Finding | State |
|---|---------|-------|
| M1 | A force arrow was drawn when the force was exactly zero — the shipped "large charge, no test charge" example showed a red arrow beside `0 N`. | **fixed**: drawn only when there is a force. |
| M2 | The attraction arrow was swallowed by the source charge for r below about 1.8 m: the head vanished under the disc and the label landed below it. | **fixed**: clamped to stop at the source's edge, and skipped when the room is too small for a real arrow. |
| M3 | The repulsion arrow and its `F` label were clipped off the top of the canvas for r above about 4.5 m. | **fixed**: clamped to the canvas, and the PNG export would have caught it had an r = 5 case existed — it is now worth adding. |
| M4 | The `V/3` and `V/2` labels overlapped at Large. | **fixed** with the blocker's radial placement. |

## Minors, and what was done
- The force row printed a signed number with no sign convention — **fixed**: "(outward is positive)".
- The exponential fallback used an ASCII hyphen among U+2212 minus signs — **fixed**.

## Left open
- The distance is not to scale for r below 0.75 m (the test charge is clamped to 18 px), so the picture cannot
  distinguish 0.1 from 0.7 m while the readout does.
- The Decimal places control is partly inert for the force (the never-zero fallback picks its own precision).
- The canvas is left-heavy at Large.
- A hand-edited link with a string in a numeric field clamps to the minimum rather than the default.
- Bitonal is only produced at 1.3x, where the 1.1 px equipotentials break into dashes; at the app's 2x they survive.

## Verified correct (so silence is not approval)
- E = k|q|/r², V = kq/r and F = k q q₀ / r² recomputed independently at the defaults and four extremes: zero
  mismatches; the sign conventions are right; the field-line arrows reverse correctly with the sign of the charge.
- No false zero across every reachable slider value at every precision: 3.6e-10 N falls through to exponential form.
- No leak with Show answers off: the force arrow and its label are key-only, and the field lines and equipotential
  radii are fixed, so nothing encodes a value.
- Print is neutral; at 2x the 66% copy keeps the field lines, the equipotentials, the charge and the test charge
  distinct.
- No clipping of the givens or the readout at Medium or Large; the census diff is the two new ids only.
