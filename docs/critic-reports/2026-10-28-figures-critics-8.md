# Five physics figures: combined critic, 2026-10-28

One combined accuracy + teacher + regression critic (read-only) over `electric-field`, `magnetic-field`,
`standing-wave`, `pv-diagram` and `refraction` at `cfd89f0`. Verdict: **FAIL — 2 blockers, 6 majors, 9 minors.**
Suite on the criticised commit: 1775 passed, 0 failed, 5 ignored (the critic's own target dir).

Fixed in `8f40aff`. Suite on the fix: 1775 passed, 0 failed, 5 ignored; census re-blessed with a six-line diff
(`electric-field`, `magnetic-field`, `pv-diagram` — the three whose key-visible content changed).

## Blockers

| # | Finding | State |
|---|---------|-------|
| B1 | **PV diagram: the student copy drew two of the three asked answers.** The y-axis carried the pressures as numbers and the area under the path was shaded — the shading *is* the work. The default student copy therefore showed ≈5.0e5 and ≈2.1e5 Pa on the axis and ~5.3 grid cells of area while the readout said `Pressure at the start _____ Pa`, `Work done by the gas _____ J`. | **fixed**: the y numbers and the shaded area are key-only. The axis keeps its ticks and gridlines, so the window and the path still read; nothing numeric is on it. |
| B2 | **Standing wave: the student copy drew the asked counts.** n+1 red node dots and n antinode rings sat beside `Nodes _____, antinodes _____` (n=2: exactly 3 dots and 2 rings). The counts are exact and trivially countable. | **fixed**: the markers are key-only. The wave still crosses the axis at each node and peaks between them, which is the reading exercise the row asks for. |

## Majors

| # | Finding | State |
|---|---------|-------|
| M1 | **Magnetic field: two of the three arrows were not on their field lines.** The chord was a fixed 22 px and two arrows were pushed down the canvas (`y + rr*0.5`). Measured: the k=2 arrow sat at radius 35.6 on a circle of 66.96 (visually on the *inner* circle), the k=3 arrow at 21.0 where no circle is drawn. The middle circle had no arrow; the inner one appeared to have two. | **fixed**: each arrow is at the top of its own circle, with a chord proportional to the radius (clamped 5–26 px). Direction was already correct and is unchanged. |
| M2 | **Electric field: the force arrow vanished for attraction at r ≤ 1.1 m** — including the default r = 1.0 and the shipped "pulled towards a positive one" example — because the test charge sat 24r px from the source and there was no room for the arrow. It contradicted the FAQ, which says the arrow is omitted only when answers are off. | **fixed**: the test charge now sits at least 40 px out, so the arrow is drawn whenever there is a force (19 px at r = 1.0). |
| M3 | **PV diagram: expansion and compression drew identically** while the work flips sign — a pixel diff of the plot region found only 12 differing anti-aliased pixels. A teacher could not tell which end was the start, and saw a positive area beside a negative work value. | **fixed**: a direction arrow on the key, and both copies number the ends 1 and 2. The arrow is key-only because the direction is what determines the sign of the work. |
| M4 | **PV diagram: the givens printed `T = 300 K` for an isobaric change**, where T is only the initial temperature (the default 5→12 L ends at 720 K; the reachable 1→40 L case at T = 800 K ends at 32,000 K). | **fixed**: `T1 = `, and the constant reads `J/(mol K)`. |
| M5 | **PV diagram at Large: the rotated `pressure (Pa)` title sat on the y tick labels**, and 8-digit tick labels were clipped off the left edge. | **fixed**: the left margin grows with the label size (`L + (lab-1)*34`). |
| M6 | **PV diagram in Print: the work area was lost at 66 %.** `#e8e8e8` is above the 168 bitonal threshold, so it became white; an isobaric figure — whose whole content is the rectangle — became a bare line. | **fixed**: Print shades the area with a diagonal hatch (`Doc::defs` pattern), which survives bitonal at any scale. |

## Minors, and what was done

Fixed in this batch:
- The progress doc still described the electric equipotentials as `V/1, V/2, V/3` after the reciprocal fix — corrected.

Left open (recorded here so the next round does not rediscover them):
- **Standing antinode rings sit on the envelope side opposite the drawn peak** (`up = -1` for even k, where the
  snapshot is `+AMP`). A valid turning point, but not the curve's peak.
- **Electric 66 %: the 10 px equipotential labels speckle** at 1.3–1.5×; the circles survive. May hold at the app's 2×.
- **Refraction's module doc and FAQ say the rays stay on the student copy**; the refracted and reflected rays are
  key-only and only the incident ray stays. Teacher-facing wording, not a render defect.
- **Refraction has no never-zero guard** (the other four do). Slider-reachable values never print zero; a hand-edited
  link at 0.04° does.
- **Electric: the bottom field-line arrowhead's white halo nicks the givens line** at Medium and Large.
- **Typography**: magnetic prints `mu0 = 4 pi × 10⁻⁷` and `uT`; PV's `J/mol K` is now `J/(mol K)`.
- **Magnetic prints the direction on the student copy.** Justified by the FAQ and visible in the arrows; blank it if a
  worksheet asks for direction.
- **PV's shaded area is in L·Pa (= 10⁻³ J)**, and the conversion to the readout's joules is never stated — a student
  counting grid cells gets 1000× too large.
- **`pv_diagram.rs`'s module doc** still says only the final pressure is an answer; both pressures blank.

## Verified correct (so silence is not approval)

- **No reachable false zero** in any of the five at any slider or precision: minimum asked values refraction θ₂ 0.3°,
  electric E 0.36 N/C and F 3.6e-10 N (exponential), magnetic 2e-7 T (exponential), standing λ 0.067 m, PV |W| ≥ 1.04 J.
- **Every formula recomputed independently** (separate Python) at every example and extreme: Snell, critical angle,
  TIR, c/n, kq/r², kq/r, kqq₀/r², μ₀I/2πr, 2L/n, nv/2L, n/n+1, nRT ln(V2/V1), nRT/V1, nRT/V2, PΔV — zero mismatches.
- **Equipotential labels are the reciprocal-fixed `V, 2V, 3V` inward** (negated for a negative charge); the
  field-line arrows reverse with the charge sign; the force direction is right wherever it is drawn.
- **Magnetic arrows point the right way**: out of the page → left at the top (anticlockwise), into it → right
  (clockwise); dot and cross are correct and both survive 66 %.
- **Refraction geometry**: rays on the correct sides, TIR only past the critical angle, glass→air bending away.
- **No leaks** in electric, magnetic or refraction student copies (refracted ray, force arrow and speeds key-only).
- **Print is neutral grey** in all five, and every 66 % copy keeps the line/hatch/text distinctions except the two
  named above.
- **Large layouts**: no clipping or collision in refraction (0°/89°/TIR), electric worst case, magnetic worst case or
  standing n=6, apart from the electric halo nick and the PV axis defect.
- All five are in `NO_HANDLES`; `handle_coverage_report` passes.

## Not checked

- Small label size (0.8) for any of the five.
- The 2× bitonal export; only 1.3× PNG-test output and simulated 1.5×, so the equipotential-label finding may not hold
  at 2×.
- Census byte-identity, share-link/`added_in` compatibility, and the live app UI (headless hook and unit tests only).
- Off-grid hand-edited link values beyond the one refraction case.
