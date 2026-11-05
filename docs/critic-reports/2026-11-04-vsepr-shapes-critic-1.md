# vsepr-shapes — blind critic 1
Commit 374b712 (`figures-gapfill`), /tmp/figures-crit/vsepr1. No image input: I never looked at a PNG; findings below are read from the SVG, text and geometry. Own suite 25/25 pass; census hash verified.

**Verdict: FAIL — 1 blocker, 3 major, 3 med, 4 low, 2 minor.**

## BLOCKER
1. **The student arc is not the angle it labels.** The arcs are 3-D great-circle projections, so the screen angle between the two sticks differs from the printed 3-D value. Default (255/16), measured from the SVG endpoints: H2O 45.17° (label 104.5), BF3 and PCl5 162.52° (120), XeF4 135.83° (90), SF4 154.18° (102), NH3 91.97° (107), CH4 104.79° (109.5), SF6 85.77° (90); only BeCl2 is exact. The shipped H2O example (40/14) spans 132.5° vs 104.5. Header line 45 says the student arcs stay "so the angle can be measured"; with no number on the student copy that is the only route. Repro: `cd /tmp/figures-crit/probe && ./target/debug/probe /tmp/h2o.svg H2O 255 16 student Colour Medium labels`; arc endpoints (271.69,309.00)/(286.18,294.09), vertex (258,300).

## MAJOR
2. **The lone-pair caption is false against the drawing.** Footer vsepr.rs:969, hint :487 and faq.rs:3929 say "two dots on the central atom"; code :775 puts them 0.30*118 = 35.4 px from the centre, outside the 14.5 px ball, on a dashed guide (:1009). tests.rs:604 asserts `d >= r_atom`, so the suite enforces the opposite of the caption.
3. **Atoms are not depth-sorted; the central atom is forced last (:1034).** At CH4 turn 35/tilt -30, H4 is (-16.73,11.56) from C, depth +0.985 (nearest), drawn before C (depth 0), label suppressed and its bond `drawn=false`; the near H is a crescent behind C with no label. tests.rs:358 asserts a0 is last, so the suite pins the rule instead of catching it.
4. **Lone pairs are drawn on bonds in face/edge views.** XeF4 0/0: guides l0/l1 run (258,314.5)->(258,335.4) and (258,285.5)->(258,264.6), exactly on bonds b4/b3, with the dots at 335.4/264.6 on those bonds. SF4 90/0 puts the single pair on the lower vertical bond (to F2); H2O 35/-30 has a guide 0.28 px off b1. The dots read as bond decoration, not lone pairs.

## MED
5. **Edge-on views silently drop substituents.** BeCl2 90/0 has three atoms at (258,300), hides both Cl, draws both bonds with `drawn=false`, and the key still says Linear, 180°; the student drawing is one Be ball. BF3/NH3/H2O/PCl5 down-y and SF4 ±90 draw two ligand atoms at the same point (one hides the other). No note.
6. **Key and student canvases differ at Large.** PCl5 639 vs 583 px, SF4 727 vs 669, XeF4 639 vs 610, because the key's "Trigonal bipyramidal" wraps into more panel lines than "______"; `answer_pair` renders both unpadded. Medium is 545/545.
7. **Small is below the 66 % floor.** At lab 0.8 the fonts are 10.4-14 px, so 6.86-7.39 px at 66 %, under the repo's 8.4 px floor; the 66 % test uses only the default Medium and only H2O/SF6/SF4, and never Small/Large.

## LOW
8. PCl5 and SF4 print two angles in the key ("120° and 90°", "≈102° and 90°") but the student has one `Bond angle: ______`, so the two values are not both asked for.
9. The footer says "atoms are labelled circles"; at the H2O example (40/14) the far H is unlabelled because its label box clips the lone-pair clearance by under 1 px.
10. Print central atom is `Fills::series(0)` = hatch0, a plain #d9d9d9 wash with no foreground mark; a 1-bit 66 % copy loses that fill (outline/label remain). Ligand hatch3 survives; the 66 % test checks only dark-pixel count.
11. Bundled Noto Sans lacks U+2082/2083/2084 and U+2248 (fontTools); "H₂O" and "≈102°" lines fall back to Liberation Sans. Typographic only.

## minor
- comment at :761 says 0.35 of a bond, code :775 uses 0.30.
- choice values are ASCII ("BeCl2") while the panel uses "BeCl₂".

## verified
- Recomputed independently: all 9 bond/lone counts, AX, domains, domain geometry and shape; NH3 pyramidal not tetrahedral, H2O bent, XeF4 square planar, SF4 see-saw. All 11 stated angles within 0.03° (CH4 109.471 vs 109.5).
- View matrix orthonormal; orthographic projection; rotation preserves lengths/angles; 360° identity; deterministic tie rule.
- Arc endpoints are exactly 0.30/0.32 of the projected bonds and the vertex is central; label text equals the table; edge-on pairs skipped (BF3/BeCl2/XeF4 90/0); student H2O has no shape/angle/degree text, three blanks, fixed ASCII desc, arc retained; whole-source scan clean.
- 288-render independent sweep (9 x 8 views x 2 sizes x key/student): no text off-canvas; the 864-render real-glyph audit passes. Print attribute-grey; patterns present.
- Refusals/NaN/∞/empty/10,000-char (probe) and 10k fuzz render; pure; CPU test passes.
- Registration: chemistry::defs includes vsepr (26 chemistry ids), FAQ arm, NO_HANDLES, 2 census ids; FNV-64 of the default render = 84c7a33b2ae90e3c, matching both fixture lines.

## mutants (11, full vsepr suite each)
Killed: M3 H2O 109.5 (6 tests), M4 reversed bond depth, M5 always-key shape, M6 arc radius 0.28, M7 pure depth order [killed by the a0-last assertion], M8 dots on the ball, M9 never hide. Survived: M1 CH4 109.0 (within the 1° tolerance), M2 lone_count-1 in the panel, M10 footer "dots on the ligand atoms", M11 lone fallback to +x (the face-view dots-on-bond is untested).

## not checked
- No image input: no PNG was viewed; visual legibility and print quality are arithmetic, not seen. Live egui editor, browser font metrics, PDF/DOCX export, release timing and the full lib suite were not run.
- I did not rasterise a real 66 % bitonal copy of Small/Large; the 6.86-7.39 px figures are computed.

## biggest gap versus a textbook VSEPR figure
A textbook draws each angle in the page plane at its true value and shows the lone pair as a lobe on the central atom; this figure draws a 3-D arc that reads 15-59° away from its own label and asks the student to measure it, and puts the pair on a dashed stick 35 px out, so it cannot serve as the measure-the-angle exercise it claims.
