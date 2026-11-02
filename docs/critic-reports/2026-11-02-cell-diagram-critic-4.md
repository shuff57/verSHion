# Cell diagram, critic 4 (verification), commit dae1d3e

Verdict: FAIL. 0 blocker, 0 major, 1 MED, 5 LOW, 3 minor.
(The critic could not write its report file; condensed from its final answer. Probe, analysis and 3,326 renders: scratchpad/cd-crit4/.)

Critic 3's MED is fixed in the sense named: no drawn ribosome lies under any leader or halo in 3,326 renders (I1: 0 violations, ink clearance at least 2.79 px). The fix repeats the old stud-versus-own-leader problem one step over, and the routing meant to protect the studs does nothing measurable.

## MED 1: the Rough ER leader runs through one of its own protected studs and that stud is left out, in every plant layout
1,252 of 1,252 plant renders (including the opening state and plant defaults at every size) and 36 animal pointers. One of the 4 kept studs lies on the leader's own path, so `draw` leaves it out; the leader passes 0.1-1.6 px from that stud's centre in 532 plant cases; the omitted stud is on the pointed sac itself in 1,228 cases. Left near the pointer: exactly 3 drawn studs within 14 px (I2 holds with zero margin), exactly 1 within 10 px in all 1,252 renders; 2 of the 3 are on the inner sac, the pointed sac's outer face has one stud near the pointer and the place where the leader leaves the sac is bare. Cause: one own stud costs only a quarter crossing, cheaper than any other route, so the router takes it every time. The FAQ ("no leader, its own included, runs over them") and the code comment ("routing keeps each pointer's studs") are false: the own leader runs over a kept stud in 1,288 of 1,953 Rough ER pointers (66 %). Mutant M11 (every stud unprotected) passes all 35 tests and gives the same I2 distribution: I2 holds only because of where the studs happen to sit. Critic 2's rule "never omit a stud within about 15 px of a Rough ER pointer" is broken in 100 % of plant layouts. Same pattern as every round: critic 2 a bare sac, critic 3 a stud on the own leader, now the own leader removes the stud.
Repro: Plant cell, defaults or opening state, any size, blank 5 (Rough ER). Crops zoom/S_start_asis_rer.png, zoom/S_start_Medium_0_stu_Print_rer.png, zoom/plantL_rer_bw66.png.
Fix: one own kept stud costs at least a full crossing, or move it to its mirror angle on the same sac instead of omitting it; assert no kept stud is ever left out; assert at least 2 drawn studs on the pointed sac within 12 px; at minimum correct the FAQ and the comment.

## LOW
1. A 406 px leader in the animal defaults at Large with functions (bound 420): the Nuclear envelope leader runs diagonally across the lower half (round 3 drew 197-261 px). The stud terms cause it: removing the 3.7 px lane (M09) or all stud protection (M11) gives 263 px. Also example 0 and masks 12/26/27/30/31 at Large with functions.
2. New crossings in the grid (defaults, start and examples as-is are clean): 268 of 26,523 leaders (1.0 %). The Rough ER leader over the Smooth ER (7 plant layouts: masks 24/25/29 Medium with functions, 9/13 Large with functions), the very pair the blank must tell apart; the Nuclear envelope leader skims the nucleus top within the halo of a centriole (animal mask 28, all sizes); the Rough ER leader over a lysosome 20 times (8 in round 3).
3. A kept stud sits partly under the Smooth ER: plant stud 29 is 55 % covered by the Smooth ER tube at its join with the Rough ER, stud 31 fully hidden; in Print at 66 % the half stud reads as a bump on the smooth tube. The "touches only its own sac" test checks only Rough ER sacs.
4. The tightened speed bounds fail under load: at load about 35 the speed test failed in 4 of 21 mutant runs that do not affect speed (plant + plasmodesmata scene 8.99 s vs 8 s; layout worst 6.8 s vs 5 s). Fix: about 20 s or CPU time.
5. Test gaps: M11, M12, M20 survive outright; M08, M09 survive apart from the speed flake; M06, M07, M10, M21 are killed only by pinned cost constants in `hit_testing_follows_the_stroke_width_and_the_fill`, never by a test on the picture.

## minor
1. The Capsule leader crosses a pilus in the bacterial defaults at Large with functions (already in round 3; the FAQ "for the defaults... none does" is true only at the as-is size).
2. Free ribosomes left out: as few as 12 of 18 drawn (animal), 11 of 14 (plant), 24 of 28 (bacterium), as in round 3; a targeted ribosome is never left out (0 of 3,326).
3. The redrawn Rough ER outline crosses the inner 0.6 px of every stud (visible at 8x only).

## Mutation (21 own mutants)
12 killed by tests on the picture (M01-M05, M13-M19), 4 killed only by pinned constants (M06, M07, M10, M21), 5 survived (M08 two own studs cost a quarter; M09 no 3.7 px lane; M11 every stud unprotected; M12 protected band narrowed; M20 tighter spacing).

## Verified correct
I3: 0 intruders within 6 px of any drawn leader end over 26,523 leaders and all 31 group sets (nearest other leader 7.006 px); I4: stud radius 2.5, flush on its sac; longest boundary run 38.5 px; no leader crosses a drawn ribosome; 0 text overlaps (1,670 Colour renders); 0 leaks in 1,658 student copies via `FigureDef::svg`; all 1,668 keys correct; Print 66 % wall/membrane two lines, leaders visible; census exactly the two cell-diagram ids; test names 34 to 35, none removed, assertions 231 to 238; module 35/35, census_baseline, faq 15/15, start 5/5, handle 23/23; full figures:: 1,859 passed 0 failed 6 ignored.

## Teacher verdict and biggest gap
Animal and bacterial cells usable in an exam; plant usable but the Rough ER blank rests on a single stud on its own sac, with a half stud on the smooth tube where they join. Biggest gap vs a textbook: the plant Rough ER is token-sized (two short sacs, the outer about 43 px, 13 studs in total); a textbook draws a broad, densely studded network joined to the envelope. Drawing it larger would end the stud, leader and pointer conflicts that have failed every round.

## Not checked
Release-build timing; live egui editor and PDF/DOCX; browser font metrics; a real photocopier; more than about 20 PNGs by eye; caption biology (unchanged this round).
