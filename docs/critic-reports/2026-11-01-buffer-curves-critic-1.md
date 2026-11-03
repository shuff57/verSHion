# Critic report: buffer curves, round 1 (FAIL, fixed)

Critic ran on a frozen copy of 19d9bde. 2 blockers, 2 majors, minors.
pH arithmetic checked against an mpmath solver on 2,719 cases (0 mismatches); all 14 pKa values and 8 indicator ranges match CRC; no student-copy leak found.

Blockers
1. Find-pKa key blamed the wrong reaction for a reading off the shortcut (22 of 69 cases backwards).
2. Key arithmetic contradicted itself for a custom pKb at 3 decimals (rounded intermediates, false "agrees" claim).

Majors
3. "Jump too small" given as the reason no indicator fits when the jump was 1.5 pH or more.
4. Student copy has no curve but the key gave only the exact-curve capacity volume, not the shortcut result.

Mutation testing: 29 mutants, 2 survived (swapped indicator colours; small pKa-dependent axis drift).

Fixed in the round-2 commit: reaction named from the sign of [H+]-[OH-]; digits printed so shown arithmetic reproduces the result; true no-indicator reason; capacity worked from amounts with the exact-curve volume beside it; both survivors killed (14 of 14 mutants caught in the builder's second script); minors (dashed wording, non-breaking joins, titrant step, handle only for Custom, notes, neutral wording, no-salt wording).

Open minors: the capacity blank always says "or none if past it already" (changing it would reveal the answer); the 0.1 pH far-reading and 1 mL tiny-volume thresholds are the builder's choices; a line may end with "/"; the working repeats when the volume asked about is 0 mL; the Henderson-Hasselbalch line gives a generic reason; the live editor and handle dragging were not checked by any critic; the round-2 fixes were not critic-reviewed. The full figures suite showed one failure on the builder's base (math::absurd_windows_cannot_run_away, a timing flake under load, passes alone).
