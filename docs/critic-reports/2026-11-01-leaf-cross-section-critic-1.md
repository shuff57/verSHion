# Leaf cross-section, critic 1 (accuracy + teacher + regression), commit 4dd15cd

Verdict: FAIL. 0 blocker, 1 major, 3 MED, 4 LOW, 4 minor.
(The critic could not write its report file; this is its final answer, condensed. Probe, mutation script and 44 renders: scratchpad/leaf-crit1/.)

Sources: OpenStax Biology 2e 30.4 (fig 30.27 xylem top / phloem bottom, 30.24c sub-stomatal air space, "guard cells are the only epidermal cells to contain chloroplasts"), university dicot-leaf handouts.

## MAJOR 1: closed guard cells are drawn bigger than open ones
Open guard cells rx 8, closed rx 11 (ry 9 both). In the section one guard cell is 224 px2 open and 308 px2 closed (closed 37 % larger); in the view from below 660 px2 open vs 830 px2 closed (outer outline fixed, open pore cut out). The caption and FAQ say swollen open cells and limp closed ones, so the drawing contradicts its own text; this is the one thing the Open/Closed control exists to teach. Repro: any settings, compare stomata=Open and Closed (png/z_stoma.png).
Fix: open guard cells larger and kidney-shaped; stop forcing both states to fill the same 46 px epidermis gap; in the view from below grow the outer outline when open. The test `open_and_closed_stomata_look_different...` locks the inversion in (a mutant with the proportions fixed was killed by its gap-filling check): replace with "open guard-cell area > closed guard-cell area".

## MED 1: closed stoma, the Stoma leader fakes a slit in the cuticle and ends on the guard-cell wall
The 3 px white halo under each leader is drawn over the leaf tissue; with stomata closed it erases a gap in the whole lower cuticle under the closed stoma. The leader ends 0.2 px from both guard cells where they meet; the Guard cell leader ends 3 px away in the left guard cell. At Small, blanks 10 and 11 point at the same pair of cells: a student writing "guard cells" for 11 is marked wrong. Repro: stomata=Closed, any size, key or student (png/stu_closed_pr_Small_66.png, png/z_closed_stoma.png).
Fix: no halo across leaf tissue; for a closed stoma use a bracket over the guard-cell pair, end the leader in the air chamber above it, or drop that blank and say so in a note.

## MED 2: in Print the phloem is filled with the same dark dots the legend calls chloroplasts
Print phloem uses the shared dot pattern: each sieve tube 0-2 random dots, dark interior 3-28 % per cell. The Print key says "Chloroplasts (dark dots)", teaching that sieve tubes contain chloroplasts; otherwise the phloem looks like companion cells and bundle sheath (png/z_vein_pr66.png). Untested: surviving mutants print_phloem_white and print_xylem_thin (Print xylem wall 2.2 to 1.0; the FAQ's "thick black xylem walls" is untested because the 66 % check samples only the outline).
Fix: a phloem fill that cannot be mistaken for dots (light grey #bfbfbf or fine horizontal ruling); Print tests for phloem fill and xylem wall thickness.

## MED 3: editor-note advice fails when followed
Repro: stomata=Closed with all five label switches off. The note advises switching on "Gas exchange arrows"; doing so still labels nothing and adds a second note. The builder's test checks this only with stomata Open.
Fix: leave "Gas exchange arrows" out of the advice while stomata are closed (or say "with Stomata set to Open"); extend the test to both states and every switch it names.

## LOW
1. Air chamber above each stoma unguarded: mutant `no_substomatal_chamber` (spongy cells fill the chamber) survived every test. Assert no cell lies inside the chamber.
2. Two pointer lines run through cells they do not name: the Phloem leader at x=471 passes through a bundle-sheath cell (the sheath gap is at x=474 +-1.1; doubled stroke at 66 %); the Vascular bundle lead line crosses two spongy cells. Run the phloem leader at x=474, add the bundle lead line to `spongy_keepout()`.
3. Biggest scope gap vs a textbook figure: bundle sheath and chloroplasts are drawn but cannot be labelled; the chamber above each stoma is unlabelled (Campbell labels "bundle-sheath cell"; worksheets include "chloroplast").
4. Text prints small: canvas 904-1004 px wide, Medium prints about 7.5 pt across 7.5 in, Small about 6 pt. Put the view from below under the section, or default to Large in Print.

## minor
1. Student caption always explains brackets and lines even when only arrows are shown.
2. The Air space leader ends in the empty region beside the cut edge next to a half-clipped cell.
3. Spongy cells never touch (2-4 px gaps) with large empty patches: floating blobs rather than a network.
4. Survivors air_free_small (tolerance from the same constant) and phloem_target_8 are not student-facing defects.

## Mutation (24 mutants): 18 killed, 6 survived
Survived and matter: print_phloem_white, print_xylem_thin, no_substomatal_chamber. Survived, do not matter: air_free_small, phloem_target_8, spongy_keepout_gone (confirmed equivalent).

## Verified correct
Layer order, columnar palisade over lobed spongy, thinner lower cuticle (3.5 vs 6), chloroplasts only in guard cells among epidermal cells, xylem above phloem with companion cells and sheath, stomata lower surface only, gas arrows and light arrow correct; key/FAQ wording has no scientific error found; "either order" honest; no leaks in student copies (title/desc/ids/attrs); no panics on odd inputs; full `figures` run 1928 passed, 1 failed = the known flake absurd_windows_cannot_run_away; census adds only the two leaf ids, census_baseline, handle coverage, FAQ and start tests pass; 18 leaf tests pass.

## Not checked
Live egui editor and how notes appear; PDF/DOCX; browser font metrics; every label-switch combination at every size by eye; release timing; Campbell and Raven directly.
