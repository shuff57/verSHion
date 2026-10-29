# Phase-3 sweep: combined critic over four figures, 2026-10-28

One combined critic (accuracy + teacher + regression, read-only, own target dir and PNGs) swept all four phase-3
figures: `energy-bar-chart`, `collision-momentum`, `circular-motion`, `refraction`. Verdict: FAIL — 2 blockers,
4 majors, 9 minors. Its brief named the defect classes that had already recurred, and the same classes came back,
including in a figure written after the previous round.

## Blockers

| # | Finding | State |
|---|---------|-------|
| B1 | **Refraction leaked the answer on the diagram**: the student copy printed `θ₂ = 25.4°` under a blank readout *and* drew the refracted ray at the true angle. The blank-copy test only checked the readout. | **fixed**: the refracted ray, its label and the total-internal-reflection sentence are key-only; a test asserts `θ₂` never appears on a student copy for three index pairs. |
| B2 | **Collisions printed a false zero**: a 0.5 kg cart into a 20 kg one leaves a real 0.0015 J, printed `after 0 J` (and 0.00079 J on the extreme case). | **fixed**: the never-round-to-zero printer used by circular motion now covers the kinetic energy row; tested on both cases. |

## Majors

| # | Finding | State |
|---|---------|-------|
| M1 | The collisions data line clipped at the **default** size for large givens (20 kg at ±10 m/s), losing the sign convention. | **fixed**: the line shrinks to fit instead of being cut. |
| M2 | Refraction's `θ₂` label collided with the givens at small refraction angles, and its halo erased the givens text. | **fixed**: the label is clamped out of the text band. |
| M3 | Refraction announced "total internal reflection" on the student copy and drew the reflected ray — the student is being asked for the angle. | **fixed** with B1. |
| M4 | Circular motion's velocity label was erased by the givens over 273–317° of the angle slider — a side effect of the previous round's label-offset fix, which moved it off the arrowhead and into the text band. | **fixed**: clamped out of the band. |

## Minors, and what was done
- Refraction never printed `c`, which its speed row needs — **fixed**.
- Equal refractive indices read as "going into a denser medium" — **fixed**.
- The speed row did not say which speed belonged to which medium — **fixed**.
- The Decimal places controls were inert or partly inert (energy and collisions always 2 dp, refraction 0 dead) —
  **fixed**: energy 2–4, collisions 2–4, refraction 1–4, so the control does something.
- Energy's givens bypassed the clamped accessor, so a hand-edited link could print `m = 0 kg` beside a chart computed
  for 0.1 kg — **fixed**: both use one `given()`.
- Energy's module doc said the bars are outlined-but-blank, while with answers off none are drawn — **fixed**.
- The energy axis stopped at the original width on a widened canvas — **fixed**.
- `energy_pngs` and `momentum_pngs` had no Large case, though those two figures had had Large bugs — **fixed**.

## Left open
- Off-grid values in a hand-edited share link (finer precision than the sliders) can still print a real small value
  as 0 through the QA hook; the UI sliders snap, so it is link-only.
- The 66% check still rasterises at 1.3x only.
- The energy givens do not say "released from rest" (only the FAQ does).

## Verified correct (so silence is not approval)
- **The previous rounds' fixes hold**: the collisions student copy draws only the given arrows and their tips are
  identical when the masses change; energy's minimum energies print 0.49 J at every precision; the givens always
  print; the collision kind heads the data line; the Notes box now reaches every figure (verified by reading the
  call site); the Print hatches stay distinct at 66%, including a 3.75 px bar that still reads as hatched.
- **Circular motion's fixes hold**: no false zero at any precision at either extreme; the givens/readout gap scales;
  the radius is dashed; both labels have halos; the vectors keep their geometry.
- **Arithmetic recomputed independently** (its own formulas, randomised settings): energy stores and conservation,
  both collision solutions and the inelastic loss, v²/r and mv²/r and ω and T, Snell and the critical angle and TIR
  and c/n — zero mismatches.
- **No leaks** in energy, collisions or circular motion on the student copy.
- **Print is neutral** in all four and every 66% copy keeps its distinctions.
- Suite: 1744 passed, 0 failed at the commit under review; no test deleted.
