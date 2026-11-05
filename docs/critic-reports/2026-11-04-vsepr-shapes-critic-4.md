# vsepr-shapes — blind critic 4 (decision round)

Commit 25b1391 (`figures-gapfill`), /tmp/figures-crit/vsepr4. No image input: I never viewed a PNG; every number is from the SVG, the Report, a 23,976-view 5°×5° sweep (all nine molecules), 2,808 resvg renders at 0.66 (Small Print, 24×13×9) and my own projection. 40/40 module tests pass; blessed census ok in release; 14 mutants, 1 benign survivor.

**Verdict: PASS — 0 blocker, 0 major, 0 MED, 4 LOW, 8 minor.** Ships. The drawing and the chemistry are right in every view I sampled.

## LOW
1. **Small Print 66 %: the two thin carriers sit at the bitonal floor.** The arc is 2.2·0.8 = 1.76 px `#555` (under the 2.4 px Print-curve rule); at 0.66 it reads 85–128 grey, and 38 of 14,472 sampled mid-arc points are exactly 128 (the shipped rule is `<128`). The lone-pair guide is 1.12 px dashed `#555`: median 144 grey at 0.66, so it drops out at a 50 % threshold and the pair becomes two detached black dots. Tests sample arcs only at the opening and never sample the guide. Repro: H2O Small Print rel 0/0 (guide); BeCl2 rel 0/−45, CH4 rel 30/−30 (arc).
2. **Labels vanish with no note.** The centre label is dropped in 1,318/23,976 views (5.5 %) and 2,889 ligand labels; 913 of those have no note at all (the rest sit under a ball-overlap line that never mentions labels). Repro: BeCl2 rel 80/0 (abs 80/0) — the far `Cl` and the `Be` are both gone, one `Cl` stays, `view_notes` empty. Not untrue (the panel names the molecule) but it contradicts "the centre label is the reference"; the drop is a ~2 px overlap of the *estimated* boxes of "Cl" and "Be", not real ink.
3. **The overlap note names both balls with one symbol**: 1,916 views print "The F atom and the F atom overlap in this view; the nearer F atom hides the farther." Repro: SF6 rel 0/−21 (abs 0/−1), where it also repeats the near-centre note. Only the first pair is named: 328 views have two ligand–ligand overlaps (SF6 rel 0/−75 names (3,5), not (4,6)).
4. **760×1189 canvas at Large Print with two view notes** (SF6 rel 0/−21, switches off): panel and rule run to y 916, footer starts at 938, model ends at 473 — 23 % of the canvas is a blank band under the drawing. Small is 760×560; the width is fixed at 760 while every font grows 40 %.

## minor
5. The depth split is **inert for occlusion**: a ball that overlaps a stick is always further from the centre's depth plane than the stick at that projected radius (every atom sits on one unit sphere), so the stick's midpoint depth already orders it correctly against every ball it meets. 0 pieces are drawn on the wrong side in 23,976 views (79 samples per bond); the 2,740 piece/ball pairs with overlapping depth intervals all resolve the right way. What the split does change is each stick's taper and shade: `BOND_PIECES=1` differs from 12 in 1,000–2,400 px per render, max 108 grey.
6. `BondReport.width` (2.6+1.8t) is not the drawn width (3.0+2.0t): b6 SF6 report 3.077, SVG 3.44. `drawn` is documented "false when the two balls overlap" and is never set false.
7. The arc is left off in 19,339/23,976 views while the panel still prints "Bond angle: 90°"; no panel line says the arc is off (header and FAQ only).
8. 42 arcs have ≥2 of 24 points hidden mid-arc by a ball (SF6 abs 90/65, PCl5, BeCl2); 250 arcs are drawn while a ligand sits inside the centre ball, so the arc's end direction is not on any visible stick.
9. SF4 `why`: "the equatorial pair at about 102°" reads as the lone pair; the lone pair is the unshared equatorial domain.
10. `<desc>` says "rotatable" for a static SVG.
11. `Report.view` is not normalised: CH4 turn 360 reports 562° (the matrix folds it).
12. `docs/figures-progress.md:1005` still says "vsepr-shapes: first critic in flight" (rounds 2–4 missing).

## verified (my own probes, not the shipped tests)
- **Claim 1 (one depth order).** 23,976 views: 0 pieces drawn on the wrong side of any ball they overlap (79 samples per bond), so no ball erases a near stick anywhere (the round-3 failure) and no stick is painted over a far ball. The round-3 repro is fixed: SF6 rel 0/0, b3 (d 0.049–0.293) now follows a6 (−0.940); a3/a6 70.5 px apart. The fix is the single depth-sorted list (M4 is killed); the split adds no occlusion (minor 5).
- **Claim 2 (true overlap only).** SF6 rel 0/0 has no view note; the note fires only when dist < r_a+r_b (0 overclaims in 23,976); it names the atoms.
- **Claim 3 (66 % bonds at any view).** 2,808 Small Print views: 0 light bond samples at 5×5 over every bond piece; bond width 2.4–4.0 px at Small; centre-label ink min 0; Print hatch separates centre from ligand by 32 grey levels.
- **Geometry.** Projection matches an independent Rx·Ry to 7e-4 px over 300 views; 9/9 atom, lone, domain and AX counts; all 11 stated angles within 0.03° of the coordinates (CH4 prints 109.5 for 109.471); H2O 104.5/115/109.2, NH3 107/111.8, SF4 102/90, XeF4 90/180.
- **Arcs.** 4,637 arcs; every one has a 3D-matching pair whose page angle is within 0.4° of the printed number (0 exceptions), 0 blank numbers, label radius 53.6–63.0 px. PCl5 and SF4 open on the 90° arc only, as the header says. XeF4's opening omission is forced: in all 138 arc views every pair projection is either collapsed or <4.2 px from a bond line of sight.
- **States.** 81 student views: no shape word, no angle string, no `°`, no angle element; the four lone-pair omission reasons appear in the header, the hint, the footer and the FAQ. Turn 0–360 (360 byte-identical), tilt ±90, switches off, 10,000-char name, NaN/±∞, 10k fuzz: no panic, no bad number.
- **Registration, FAQ (6), examples (7), NO_HANDLES, census** `13d7d9df44a07d6f` defaults==start: pass at this commit.

## mutants (14; M5 survives, benign)
Killed: arc filter 0.4→5°, drop near-centre note, overlap note always on, reverse the one depth order, lighter sticks (66 % test), draw lone pairs on bonds, student keeps the key why, desc always "lone pairs", never hide a centre atom, lone dots at depth 0, one student blank for every molecule, "away"→"at", 360 not folded.
**M5 `BOND_PIECES 12 → 1`** (no split): all 40 tests green; none of them should fail, because the split changes no occlusion (minor 5). It does change every stick's taper, and the suite never asserts that the stick tapers or shades with depth, so the taper is unprotected. Not a correctness risk; a 5-line test could pin it if the taper is wanted.

## not checked
No image input: I never looked at a PNG, so "does it look right", the 12-piece stick seams and photocopy appearance are inferred from geometry and thresholded pixels. I did not independently re-run the resvg ink-box collision audit (the module's own 912-render audit passes); my legibility sweep is Small Print key only — Medium/Large/student/Colour were spot-checked. Live egui, browser metrics, PDF/DOCX, release timing and the full lib suite were not run (only the census test in release).

## biggest gap
The panel never admits the one thing the drawing silently drops: labels. The centre loses its symbol in 5.5 % of views and ligands in 2,889 instances with no note (LOW 2), even though the figure's own promise is that the centre label is the reference and the panel says when something is left off.
