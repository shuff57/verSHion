# Cell diagram, critic 2 (verification: accuracy, teacher, regression), commit eedf094

Verdict: FAIL. 0 blocker, 0 major, 1 MED, 4 LOW, 7 minor.
(The critic could not write its report file; this is its final answer, condensed. Probe and outputs: scratchpad/cd-crit2/.)

Critic 1's major and four MEDs are fixed for the cases they named and hold under the critic's own measurement (754 layouts, 5,809 leaders: every leader ends on its own structure, no dots, ribosome leaders stop 0.78-0.80 px from the dot, 0 drawn ribosomes under a halo, non-ribosome pointers at least 6.5 px clear, 3,072 renders have identical leader geometry across key/student/Colour/Print, 0 leaks in 1,536 student copies, 0 text overlaps in 576 renders, Print 66 % wall and membrane stay two lines, biology of all 23 captions correct vs OpenStax 4.2/4.3, 250-step cache walk no stale render, full figures:: run 1,851 passed 0 failed). Leader crossings by the critic's count: 12 of 5,809 (0.21 %), all Large.

## MED 1: the Rough ER pointer ends on a bare sac that looks like the Smooth ER (plant start state)
`rough_er` removes studs within (RING+3.5)/r of each pointer place, on the pointed sac and the sac inside it, and `draw` also drops any stud under a leader; nothing checks the sac still looks studded. Measured (rer.txt) studs near the end of each Rough ER leader: plant 288 pointers, 0 studs within 12 px and 0 within 20 px (the whole pointed sac carries 4 studs, all at the far end); animal 144 pointers, 0 within 12, 2-4 within 20. Plant start state: pointer at scene (-150.3, -41.2), Smooth ER tube 29 px away, same fill (#bcd9f2, white in Print), outline and width. The caption says "made on its dotted surface", and rough vs smooth ER is a standard exam question: a student writing "smooth ER" for blank 5 is marked wrong. See crop-plant-rer.png, png/t1-print-key-Large.png.
Fix: keep the 6 px ring; stud 7-9 px either side of the pointer and along the plant's outer sac; never omit a stud within about 15 px of a Rough ER pointer; test: at least 3 drawn studs within 14 px of every Rough ER pointer over the whole grid (no current test protects this; mutant K23 was killed only by side effects).

## LOW
1. Every ribosome leader ends 3.0-5.75 px from the cell membrane (388 cases under 6 px, all Ribosome pointers, intruder the Membrane; worst 3.0 px Animal mask 8, functions, Large; defaults 3.25, example 4 3.5, start 5.0). `Scene::valid` tests the ring round the dot centre, but the leader stops 0.8 px outside the dot toward the label, about 3.3 px nearer the membrane. FAQ "nothing else lies within 6 px of the end of a leader" is wrong. Fix: run the ring test at the drawn leader end, or keep ribosome pointer places at least RING + r + 0.8 px from the membrane.
2. A Rough ER leader runs through a labelled plasmodesma for 3.2 px: plant, plasmodesmata and functions on, Large, masks 8, 9, 11, 12, 13, 15, 19, 23, 26, 27, 30, 31. Fix: count a plasmodesma as an obstacle for any leader not pointing at it.
3. The Cytoskeleton leader runs along the bottom wall band for 91.5 px (plant, mask 25, functions, Medium). `W_ALONG` = 3 per px is too low. Fix: raise it, or treat a run along a boundary over about 12 px as a violation.
4. At Large with functions leaders get very long: plant student Print, plasmodesmata, all groups, functions, Large: discs spread over about 1,015 px for a cell about 330 px tall, leaders up to 351 px. `leaders_stay_short_enough_to_follow` checks only the three defaults.

## minor
1. Test `the_solid_print_tones_read_black_at_sixty_six_percent_grey` was removed (27 to 31 tests = 5 added, 1 removed); its lysosome and centriole checks moved into the new 66 % test, but the rule is "never deleted".
2. Accepted words inconsistent: DNA accepted for Nucleoid but not Plasmid; plurals accepted for mitochondria, pili, plasmodesmata, centrioles but not chloroplasts, ribosomes, lysosomes, plasmids, flagella; "cell surface membrane" (UK GCSE) not accepted.
3. Halos make fake nuclear pores (a 4.4 px gap where a leader crosses the nuclear envelope; animal defaults leader 3).
4. Pili roots vanish into the capsule hatching in the bacterium's Print copy.
5. Ribosome studs r 2.1 vs free ribosomes r 2.5, both 80S.
6. Code comment calls the rod through the wall the "hook"; the real hook is outside the wall.
7. Vacuole is 56.2 % of the space inside the membrane (meets the claim; textbooks usually show 70-90 %).

## Mutation (24 of the critic's own mutants): 23 killed, 1 survived
Survivor K18: the basal-body bars removed (test gap, nothing pins the bars).

## Teacher verdict
Animal and bacterial defaults usable in an exam; plant start state not usable as is (blanks 5 and 6 point at identical plain tubes). Biggest gap vs a textbook: the endomembrane system (OpenStax Fig 4.8 / Campbell show a densely studded rough ER continuous with the nuclear envelope); no chromatin, no vesicle traffic, no peroxisome.

## Not checked
Release-build timing; the live egui editor, PDF/DOCX and student-plus-key export; screenshots of the real app; params_from_json beyond the module's 10,000-input test; hand inspection of all about 3,000 renders (about 15 PNGs plus zooms looked at).
