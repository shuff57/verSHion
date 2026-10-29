# Five physics figures: verification critic on the fixes, 2026-10-28

A verification critic took `8f40aff` through `git archive` into `/tmp/verify-k` with its own target dir and checked each
of the eight fixes against the figure it was meant to repair, rather than reviewing the figures afresh. Its full report
is at `/tmp/verify-k-report.md`; this is the disposition.

Suite on the frozen copy: 1775 passed, 0 failed, 5 ignored. Census diff for `8f40aff`: exactly six lines, in
`electric-field`, `magnetic-field` and `pv-diagram`.

## Verdicts

| # | Fix | Verdict | Evidence |
|---|-----|---------|----------|
| 1 | PV student copy: no pressures, no shaded work | **FIXED** | student SVG has 0 polygons and 0 y labels (key: 2 and 3); every example; endpoints exactly the path's first and last point |
| 2 | Standing markers key-only | **FIXED** | circles key/student at n = 1..6: 3/0, 5/0, 7/0, 9/0, 11/0, 13/0 |
| 3 | Magnetic arrows on their own circles | **FIXED** | arrows at y = 37.00 / 78.04 / 110.44 = 145 − r for r = 108 / 66.96 / 34.56; heads left for out of the page, right for into it |
| 4 | Electric force arrow for attraction | **FIXED** | 100 renders (r = 0.1…5.0 step 0.1 × q₀ = ±2): F drawn in all 100; none with q₀ = 0; attraction down, repulsion up |
| 5 | PV direction and numbered ends | **FIXED** | plot-region diff expansion vs compression 219 px (12 before the fix); arrow right for expansion, left for compression |
| 6 | PV givens say T1 | **FIXED** | both processes read `T1 = 300 K` and `R = 8.314 J/(mol K)` |
| 7 | PV title clear of the y labels at Large | **PARTLY** | default at Large: 1.9 px gap, nothing clipped. The high-pressure case (V1 = 1 L, 1 mol, 300 K — the slider minimum) overlapped by ~15 px at Large, 8 px at Medium, 6 px at Small |
| 8 | PV Print hatch survives 66 % | **FIXED** | pattern in exactly the Print SVG, shade references it, hatch visible at 0.7×, 1.3× and 2.0× |

## What the PARTLY verdict and the new defects turned into (fixed in the commit after)

- **PV left margin, the real cause of #7.** The margin grew by 34 px per label step while the tick labels grew by
  about 37, and a seven-digit pressure (V1 = 1 L, which the slider reaches) is wider than anything the original
  margin allowed for. The margin is now measured from the widest label the axis will actually print
  (`left_margin(lab, widest)`), with the rotated title's band added, and a test sweeps 1, 4, 7 and 8 digit labels at
  Large asserting the label's left edge clears the title. Two worst-case PNG cases were added to the figure's export
  (`v1one`, `high`) so this layout cannot regress unseen.
- **PV endpoint numbers near the top of the canvas.** Numbering the ends put the `1` a few px from the top edge on a
  path that starts at its highest pressure. The label now drops below the point when there is no room above it.
- **Electric distance fidelity.** The 40 px floor that fixed the missing attraction arrow made 0.1 m and 1.2 m draw
  identically, which is a real regression for a worksheet. The floor is 16 px (enough to clear the source disc) and
  the arrow is now drawn *beside* the test charge rather than on the radial line between the charges, so it needs no
  room in the gap at all. 0.1 m and 1.0 m are 8 px apart again, and a test asserts that and that the arrow is drawn
  at every r from 0.1 to 5.0 for both signs of test charge, absent when the force is zero and absent on the student
  copy.
- **Magnetic inner arrowhead.** A 9 px head on a 7.6 px half-chord read as a stub. The chord is 0.34 r clamped to
  12–26 px, so the inner arrow is proportionate while still a chord of its own circle.

## Left open

- The critic did not verify the live app UI or its own 2× export path (all rendering went through the lib's
  `to_png`/`to_rgba`), share links and `added_in`, Small label size for magnetic/standing/electric, or a negative
  source charge in the electric figure (the code is sign-symmetric but it was not rendered).
- The electric equipotential labels still speckle at 66 % around 1.3–1.5×; the circles survive.
- The magnetic field prints the direction on the student copy, deliberately (the FAQ says so).

## Verified correct, beyond the eight

No label newly collides, no blank moved, the hatch hides no text, no key-only arrow left a hole in the student copy,
and no value now prints as zero. The PV student copy still works as a worksheet: the x axis is labelled, the path and
numbered ends stay, the givens carry n, T1, V1, V2 and R, and W, P1 and P2 all follow from them.
