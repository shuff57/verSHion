# Leaf cross-section, critic 2 (verification), commit 8744543

Verdict: PASS. 0 blocker, 0 major, 0 MED, 4 LOW, 5 minor.
(The critic could not write its report file; condensed from its final answer. Scripts, probe and 41 PNGs: scratchpad/leaf-crit2/.)

Critic 1's major and three MEDs re-checked one step past each fix and fixed: open guard cell 281 px2 vs closed 174 px2 (1.61x) in the section and larger crescents round a wide pore from below; no white halo anywhere, lower cuticle continuous under both closed stomata, no leader ends at a closed pore; Print phloem ruled, every sieve tube has a 100 % ink row at 66 % grey at all 12 Print renders; every "Switch on ..." note applied in all 3,072 states labels something, and "Set Stomata to Open" restores what the note says was left out.

## LOW
1. Presets regressed: the new "Label a chloroplast" switch is on by default and was not turned off in examples 1 ("The layers, student copy", now 6 blanks), 2 ("Gas exchange through a stoma, student copy", now 8 blanks) and 4 ("The vein: xylem and phloem"); at 4dd15cd none had it. Fix: `b("show_chloroplast", false)` in examples 1, 2, 4 and assert each example's label set.
2. Only the phloem is protected from the dot pattern in Print: mutants print_companion_dots and print_sheath_dots survived all 22 tests. Fix: assert only chloroplast ellipses are dotted and companion and sheath interiors are white at 66 %.
3. A closed stoma cannot be asked for: the Stoma blank is dropped when closed (disclosed and consistent). Fix: a short bracket under the guard-cell pair named Stoma, or accept "stoma" on the Guard cell blank when closed.
4. The FAQ reason for not labelling the bundle sheath is inaccurate: a leader from the bottom label rows to the lower-left sheath cell near (451, 182) would cross only the lower cuticle and epidermis. Fix: label the bundle sheath (Campbell does) or reword the FAQ as a choice.

## minor
1. Xylem and Phloem leaders pass sheath-ring gaps 1.14-1.15 px from cell outlines (about 0.14 px white left).
2. The Xylem leader at x=474 runs 22 px down a 1.4 px gap between upper epidermal cells and reads as a thick wall.
3. The Chloroplast leader ends 3.7 px inside a palisade cell touching a 3 x 4.8 px dot; "Palisade cells" is an accepted answer elsewhere.
4. Density palisade/spongy 2.08x per area (thin margin over the test's 2.0); guard cells (1.07 open, 1.72 closed per 100 px2) denser than spongy, wrong way round; spongy chloroplasts sit at half radius.
5. Open guard cells are circles with a 12 px pore (63 % of cell width), no thickened inner wall: schematic.

## Biggest remaining gap
The spongy mesophyll is still separate floating blobs; no spongy cell touches another, the palisade or the vein. The missing bundle-sheath label comes next.

## Sweep and regression
3,072 renders (6 label switches x Open/Closed x key/student x 3 sizes x Colour/Print x view from below): 0 collisions; all 1,536 answer keys 0 pointer problems; 1,536 student copies no leak, numbering consistent; odd inputs fine; no added_in needed (leaf exists only on figures-gapfill). Census 9b47927 to 8744543 changed only the two leaf ids. 18 tests became 22, none deleted. Full figures:: run 1,854 passed, 1 failed (celldiagram a_first_layout_is_fast_enough_to_follow_a_checkbox, 12.6 s at load 40, passes alone at 1.3 s; load flake).
Mutation: 23 own mutants, 20 killed; survivors that matter print_companion_dots, print_sheath_dots; harmless chl_target_left.

## Not checked
Live egui editor; PDF/DOCX; browser font metrics; Campbell and Raven directly; real printer output.
