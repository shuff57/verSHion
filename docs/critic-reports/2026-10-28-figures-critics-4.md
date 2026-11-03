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

## Verification round (on 9f56030)

A verification critic re-checked the fixes independently. Six of nine checks passed outright: B1 (the student copy draws
only the given arrows, and the arrow tips are identical when the masses change, so no after velocity is recoverable),
B3, B4, M1, M4 and M5 (the three Print hatches are distinct at 66% at both 1.3x and 2x).

Three were still open, and two defects came from the fixes. All the code-local ones are fixed in the commit after it:

| | |
|---|---|
| B2 residual | `dec.max(1)` was not enough: the formatter trims a trailing zero, so a real 0.049 J still printed "0 J". Energies now print at two places, and the test covers the smallest reachable energy as well as 0.49 J. |
| M3 (momentum) | the canvas grew 1.28x while the font grew 1.4x, so the data line still clipped at Large. The width now grows 1.25x per unit of font scale. |
| new, from B3 | the givens printed the **typed** final height while the stores clamp it to the start, so "drops from 2 m to 5 m" could sit beside a chart drawn for 2 m. The printed height is clamped the same way. |
| new, from B1 | with the scale taken from the givens only, a fast after velocity could run off the key's canvas. On the key the scale now takes all four velocities; on the student copy it stays givens-only, so the no-leak property is kept. |
| small values | velocities and energies at two places, so 0.0122 m/s no longer prints "0 m/s". |

**Still open, and recorded rather than fixed:** M2. `figures::notes_for` returns the right note for these figures, but
the only place that displays it (`figuresui.rs:2562-2586`) lives inside `rows_card`, which is only drawn for the four
figures `figures::rows_for` reports a card for — so the note is unreachable for `collision-momentum`,
`energy-bar-chart`, and also for `circle`, `sine-cosine` and `slope-triangle`, whose Notes wiring was added earlier
today. Fixing it means moving that block out of `rows_card` so it runs for every figure; that is a UI change which
cannot be verified without running the app, so it is left for a session that can.

## Verified correct (so silence is not approval)
- Energy arithmetic across 15 probes including NaN, infinities, out-of-range and negative inputs: the total is
  conserved exactly, and every bar top equals BASE − (e/total)·MAXBAR.
- Momentum: both solutions recomputed independently, momentum conserved exactly, elastic kinetic energy conserved to
  2.3e−13, and the inelastic loss equal to ½·(m₁m₂/(m₁+m₂))·(u₁−u₂)² to 1e−9.
- Arrow directions match the printed signs; no panic on any extreme input; the suite passes with no test removed.
- Print neutrality: every fill and stroke is exactly neutral in both figures.
