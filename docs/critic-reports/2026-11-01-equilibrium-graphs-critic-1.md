# Critic report: equilibrium graphs, round 1 (FAIL, fixed)

Critic ran on a frozen copy of d0f446f. 3 blockers, 4 majors, minors.

Blockers
1. "Multiplied out" line dropped leading coefficients by relative size, so the printed degree was false when K was far from 1 (e.g. K = 1e13 printed degree 1).
2. Quadratic roots printed as identical or contradicting the x used in the next line near complete conversion (double-precision cancellation).
3. Temperature rates used kf/sqrt(K2/K), so heating an exothermic reaction lowered kf; both rate constants must rise with T. Curves were not stated to be schematic.

Majors
4. Student copy showed the answer through post-disturbance curves, K2 line and answer-dependent axes.
5. Ion names lost their charge (Fe3+ printed as Fe3).
6. "Remove 900 M HI" stated an amount larger than existed.
7. Equal-givens leakage test varied only ignored inputs; a guide-stroke assertion was vacuous.

Mutation testing: 8 of 15 mutants survived (Q/K direction, shift wording, volume, van 't Hoff sign, K2, added amount, plateau guides, rate constants).

Fixed in the round-2 commit: true polynomial and stable (Vieta) rejected root, Arrhenius-style rates with both constants rising, givens-only student axes and no post-disturbance curves, charges kept, clamped removal label, real leakage tests, 7 mutants killed, a label-audit check that each value line stays attached to its name line (it found a 3 px leader defect, fixed), and a font-coverage test (infinity sign replaced by the word).

Open minors: rounding-tie slack in the exact-root checks; key/student frame heights differ; student unbounded-side axis is a givens-only approximation; the live editor and handle dragging were not checked by any critic; the round-2 fixes were not critic-reviewed.
