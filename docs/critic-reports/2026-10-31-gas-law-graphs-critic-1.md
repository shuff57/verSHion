# gas-law-graphs, critic 1 (science and look together)

Verdict: FAIL (no blocker, six majors). Numerics were checked over about 1,500 random cases against an independent exact-fraction recomputation (all five Find modes, 4 pressure x 2 volume units, K and C, sig 2-6, extreme magnitudes): zero mismatches apart from one rounding tie. All majors addressed in the commit that adds this file.

1. MAJOR (fixed): student copy label depended on the answer (state 1 dot read "1, 2" when state 2 landed within 7 px). Now independent of the answer; test sweeps student SVGs for equal givens and different answers.
2. MAJOR (fixed): Combined key never drew state 2's own curve, so dot 2 floated off every curve. The key now draws the curve through state 2 and labels both; every key state dot lies within 1 px of a drawn curve (1,500-case test). The student copy draws neither state 2 nor its curve.
3. MAJOR (fixed): Celsius T2 was converted from an already-rounded kelvin value (322.85 C at 3 sf). Now rounded once from the unrounded kelvin value.
4. MAJOR (fixed): slope-derived n differed between graphs and disagreed with the true value at 2 sf. Now worked from the unrounded slope and rounded once (tested on every graph, sig 2-6).
5. MAJOR (fixed): Print extrapolation was indistinguishable from the second curve's dash. Extrapolation is always dotted with an end tick, in both styles.
6. MAJOR (fixed): Identify with one Combined curve called it "combined" while drawing Boyle's hyperbola. One curve is named by what it plots; "combined" needs two or more distinct curves.
7-10. MINOR (fixed): extrapolation labelled as an ideal-gas idealisation, half-away rounding of typed givens, exact curve names, R(kPa) wording.
11. MINOR (open): crowded "1" label among three extrapolations (Large, Print, Celsius, Gay-Lussac); state 2 dot on an axis at tiny values.

Open and unreviewed: the fix round has not been re-reviewed by a critic. Not checked by the critic: drag handles and the live editor; Charles Print and PV-against-P Print renders.
