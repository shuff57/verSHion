# Cell diagram, critic 3 (verification), commit 75d7519

Verdict: FAIL. 0 blocker, 0 major, 1 MED, 3 LOW, 4 minor.
(The critic could not write its report file; condensed from its final answer. Probe, analysis scripts, 3,251 dumped SVGs and PNGs: scratchpad/cd-crit3/.)

Critic 2's MED and LOWs hold: every Rough ER pointer (432 grid + 49 sets) ends in a sac with 3-4 drawn studs within 14 px; clearance at the drawn end 0 of 6,482 leaders under 6 px (min 6.27); longest boundary run 38.75 px; longest leader 336 px (24.9 % of the diagonal); 0 text overlaps; 0 leaks in 1,620 student copies; Print 66 % wall/membrane two lines; biology correct vs OpenStax ch. 4; full figures:: run 1,858 passed 0 failed; 34 module tests (31 names kept, assertions 218 to 232); census only the two cell-diagram ids.

## MED 1: a stud now lies on the Rough ER leader itself, 9.5 px before its end (plant only)
In `draw`, studs within 15 px of a Rough ER pointer are always drawn (the `by_rer` exemption skips the rule that a ribosome under a leader is left out), and `leader_cost_obj` lets the Rough ER leader run over its own studs free for its first 17.5 px (`s == Sid::RoughEr && x == Sid::Ribosome && t <= STUD_KEEP + STUD`). The router therefore sends the leader across one of the studs placed just outside the ring (9.5 px from the pointer). Over the 432 grid Rough ER pointers: 20 plant layouts have the ink through the dot (P_9_1_Large 0.02 px from the centre; masks 24, 25, 28 at every size; 9, 13, 29 at some sizes; with plasmodesmata 18, 22, 24, 25, 29); 105 more plant layouts graze it at 2.9-3.0 px and the 4.4 px halo cuts away about 1.8 px, including the opening state and the plant defaults at every size (S_start_asis: stud 9.5 px from the end, 2.91 px off the line).
Repro: cell=Plant cell, g_membrane=off, g_energy=off, Medium, functions off, student copy, Print: blank 5 Rough ER and blank 7 Ribosome; the Rough ER line runs through a stud at the sac's end and in Print the stud is black slivers either side of the line, a blob on the line (png/zD.png, png/full_P25stu.png).
It brings back critic 1's MAJOR (ribosome dots under leaders read as leader ends) in a figure where Ribosome is also a blank, and makes the FAQ promise "no dot under a line can be read as where it points" false. The suite cannot see it: three round-3 excuses cover exactly this case (`own_stud` within 15 px in `runs_over`; `Hit::Is(Sid::Ribosome)` in the 66 % visibility test's `by_envelope`; the free 17.5 px stretch in `leader_cost_obj`).
Fix: make a drawn stud's disc (STUD + HALO + 0.5) an obstacle for its own Rough ER leader, or after routing move the one stud the halo touches to its mirror angle (keeping >= 3 studs within 14 px); then assert in the grid test that no drawn ribosome lies within r + HALO of any leader except the one pointed at, with no own-stud excuse. Dropping the free stretch wholesale breaks the 45 px boundary-run limit (mutant M14), so the obstacle must be the stud disc only.

## LOW
1. The plasmodesma fix covers only the case critic 2 named (the builder moved one plasmodesma instead of making every plasmodesma an obstacle). Plant + plasmodesmata, mask 28, functions off, every size: the Cytoskeleton leader runs over the right-wall plasmodesma and about 300 px diagonally across the vacuole hatch (png/q28c.png). Ink-only count: 26 of 5,712 grid leaders (81 with halo): Cytoskeleton over Vacuole 9; Nucleolus over Mitochondrion (animal masks 24, 28) 7; Cytoskeleton over Plasmodesma + Vacuole 4; Cytoskeleton over Mitochondrion 3; Cytoskeleton over Golgi 2; Rough ER over Lysosome 1. Defaults, opening state, examples clean; within the 2 % bound. Fix: count every plasmodesma as an obstacle.
2. The speed test's bound on building the scenes was loosened about 30x: each type's first layout including scene build had to be under 5 s (eedf094); now the four scenes are timed separately against 60 s. Measured debug: 5.9 s at load about 30, 1.75 s low load. A 10-30x slowdown of `build_scene` would still pass (the per-layout 5 s bound still holds, worst 1.75 s). Fix: about 20 s for all four, or 8 s per scene.
3. The round-3 test excuses fit the new risk exactly (see MED 1). Mutant M1 survived: shrinking the stud from r 2.5 to 2.1 passes every test, so "studs the size of a free 80S ribosome" is not pinned.

## minor
1. Studs on sac k reach 1.6 px into sac k+1 and over its outline (stud centre at rad + 11.1 with r 2.5, next sac starts at rad + 12).
2. The Rough ER leader's halo cuts a notch in the pointed sac's own outline where the leader leaves it (start state, P_25); redraw that outline over the halos as was done for the nuclear envelope.
3. The Print capsule now uses the same dot pattern as mitochondria; no cell has both, but at 66 % the capsule's r 1.1 dots look more like the bacterium's r 1.8 ribosome dots than lines did.
4. "DNA" is accepted for both Nucleoid and Plasmid, so one word earns two blanks; the FAQ could say so.

## Mutation (20 own mutants): 19 killed, 1 survived (M1, stud r 2.1)
Weakly killed: M7 (kept stud not an obstacle) by the hit_testing unit test only, nothing at grid level; M17 ("cell wall" accepted for the membrane) only because the accepted-words string is pinned; M18 (studs every 18 degrees) only by the coarse "at least 8 studs" count. No mutant tests whether a stud lies under the Rough ER leader, which is why MED 1 is invisible.

## Teacher verdict and biggest gap
Animal and bacterial cells usable in an exam. Plant opening state usable, but the halo bites the stud beside the Rough ER line; with Ribosome also asked (masks 24/25/28) the line runs through a dot. Biggest gap vs a textbook: the plant's endomembrane system is tiny (two short rough ER sacs, the outer about 43 px, where OpenStax shows a broad studded network, vesicles and peroxisomes), so the plant Rough ER blank relies on 3-4 dots to tell it from smooth ER.

## Not checked
Release-build timing; live egui editor; PDF/DOCX and export through `FigureDef::svg` with title and desc; browser font metrics (resvg Colour only); hand inspection beyond about 15 PNGs plus zooms; a real photocopier.
