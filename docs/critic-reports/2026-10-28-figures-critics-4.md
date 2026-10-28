# The two phase-3 figures: combined critic, 2026-10-28

One combined critic (accuracy + teacher + regression, read-only, own target dir and PNGs) over
`energy-bar-chart` (2f9924a) and `collision-momentum` (20753f9). Verdict: FAIL — 4 blockers, 5 majors, 8 minors.

## Blockers, and what was done

| # | Finding | State |
|---|---------|-------|
| B1 | **The collisions student copy leaked both after velocities through the arrows**, and the before arrows shared a scale computed from the hidden after speeds, so an after speed could be read off by ratio. | **fixed**: arrows scale by the given speeds only, and the after arrows are drawn only with Show answers on. A test counts arrowheads (2 on the student copy, 4 on the key). |
| B2 | At 0 decimal places a real 0.49 J printed as `0 J` beside a full-height bar. | **fixed**: energies print at one decimal at least; a test pins the minimum case. |
| B3 | **The energy student copy carried no givens at all** — no mass, heights, loss or g — so it could not be answered. | **fixed**: a givens line prints on both copies; the FAQ's claim about empty frames (which the code never drew) is corrected. |
| B4 | The collision kind (elastic or perfectly inelastic) appeared nowhere on the figure, so a student could not know whether kinetic energy is conserved. | **fixed**: the kind is the first word of the data line. |

## Majors, and what was done

| # | Finding | State |
|---|---------|-------|
| M1 | The given speeds printed at the answer's precision, so `4.5` was stated as `4` while the arithmetic used 4.5. | **fixed**: givens print at two places regardless of the setting. |
| M2 | The no-collision guard was unreachable, and the real invalid case (A not closing on B) was unchecked. | **fixed**: a note fires whenever u(A) ≤ u(B), for either kind; three cases tested. |
| M3 | At Large label size the readout ran off the fixed 470 px canvas in both figures. | **fixed**: the canvas widens with the label size. |
| M4 | In Print and at 66% the cart letters were invisible against the carts. | **fixed**: the letters sit above the carts. |
| M5 | In Print the kinetic and gravitational tones both thresholded to black and the thermal bar to white — a hollow bar that reads as a blank answer. | **fixed**: each store gets its own hatch (45°, −45°, horizontal), so the bitonal copy distinguishes all three and no bar looks empty. |

## Minors, and what was done
- "Speed" for a signed quantity → **velocity**, in the control labels and the readout.
- No stated sign convention → the data line says positive is to the right.
- Arrow lengths had a 6 px floor, exaggerating small speeds → dropped; the scale no longer depends on the answers.
- No y-axis label on the energy chart → `energy (J)`, rotated.
- A zero store had no marker → a baseline stub, so a gap is not read as a bar that failed to draw.
- Dead constants (`H` in both, `ROWS_Y0` in energy) removed.
- The shared style hint promises hatching in Print; the energy figure now does it.

## Left open
- The momentum figure's `dec` is raised to at least 1, so the control's 0 setting is inert (as the fraction figure's
  is); the control still offers it.
- The energy chart has no tick marks, only the axis label and the full-height reference bar.

## Verified correct (so silence is not approval)
- Energy arithmetic across 15 probes including NaN, infinities, out-of-range and negative inputs: the total is
  conserved exactly, and every bar top equals BASE − (e/total)·MAXBAR.
- Momentum: both solutions recomputed independently, momentum conserved exactly, elastic kinetic energy conserved to
  2.3e−13, and the inelastic loss equal to ½·(m₁m₂/(m₁+m₂))·(u₁−u₂)² to 1e−9.
- Arrow directions match the printed signs; no panic on any extreme input; the suite passes with no test removed.
- Print neutrality: every fill and stroke is exactly neutral in both figures.
