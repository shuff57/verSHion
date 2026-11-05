# vsepr-shapes — blind critic 2
Commit d026eec (`figures-gapfill`), /tmp/figures-crit/vsepr2. No image input: I never viewed a PNG; numbers below are measured from the SVG, report geometry or rendered pixels. 30/30 module tests pass; census hash matches.

**Verdict: FAIL — 0 blocker, 1 major, 4 med, 6 low, 3 minor.**

## MAJOR
1. **The lone-pair stick still does not point where the pair sits, and no note says so.** XeF4's opening (0,20) truly projects both pairs at 90°/270° (on the vertical F bonds) but draws them at 45°/225°; face-on (0,0) draws 45°/135° (90° apart) although the pairs are ±z (180° apart); SF4 90/0 draws the pair at 45° although it projects to a point (depth −1). Header:35, hint:501, footer:1054 and FAQ:3941 say "pointing where it sits". Repro: probe one XeF4 0 -20 / one SF4 154 -36.

## MED
2. **Lone dots are not depth-sorted.** 1,200/50,544 audited renders paint a *farther* atom ball over a nearer dot. XeF4 Large, relative 0/60 (abs 0/80): dot (273.9,323.4) is 16.0 px from the F4 centre (r 22.4, depth −0.98) and disappears, so the pair shows one dot.
3. **A near ligand can hide the centre with no note.** SF6 relative 0/−21 (abs −1): F5 at 2.1 px, depth +1.0, is drawn over the S ball; the S label is dropped (a0 labelled=0), so the key shows a centre ball labelled F, and the only note is the two-bonds-collinear one. 774 sweep views have a ligand 0.6–3 px from the centre.
4. **The tilt −90 note reverses the line of sight.** BF3 0/−90 hides F1 at depth −1.0 (behind the centre) yet prints "A bond points straight at the viewer"; same for PCl5 (182+184 sweep renders). At BeCl2 90/0 two atoms are hidden but the note says "A bond".
5. **Print/Small at 66 % loses bonds and most of the arc.** PCl5 b2/b4/b5 have no pixel <128 anywhere along them at scale 0.66 (5×5 minima 132/129/155); XeF4 b2 0/40 samples; CH4 Small's arc is <128 at 6/25 points. The 66 % test only counts total dark pixels.

## LOW
6. Arc truth is ±1°, not exact: 5,768/26,174 arcs read 0.5–1.0° off (NH3 106.0 vs 107, PCl5 91.0 vs 90; H2O opening 104.85 vs 104.5).
7. The centre label is drawn over a nearer ball in 420/50,544 renders (BF3 80/−30, BeCl2 100/0 Small) and dropped in 5,320 views (SF6 abs −30, XeF4 abs 80); the resvg audit exempts a0, so the header's "left off rather than drawn through it" is not true for the centre.
8. "Show bond angles" off: the key panel says "Bond angle: not shown" but the explanation still prints "at 109.5°" (CH4).
9. The 18°/25° clearance is for the stick, not the dots: dot-to-bond-ray minimum 9.8° (XeF4 18/78); dots of two pairs come within 7.9 px (H2O 0/39).
10. Small is 8.448 px at 66 % — 0.05 px over the 8.4 floor; Small bonds are only 1.9–2.4 px before scaling.
11. The description says "with … atom labels and bond angles" on a student copy that draws no angle and may have labels off; it names capabilities, not what is drawn.

## minor
12. best_pair's edge-on guard (:715) is dead given the 1° guard (:720); mutant M3 survives on it.
13. The panel max (:1047) is dead: key and student panels wrap to equal line counts at every size; M13 survives.
14. docs/figures-progress.md still says VSEPR is "first critic in flight"; no round-2 entry.

## verified
- Chemistry from coordinates: 9/9 counts, AX, domains, shape; every stated angle within 0.03° (NH3 106.99, H2O 104.50, SF4 102.00).
- Opening on-screen angles: 180/120/109.461/106.925/104.849/90/90/90/89.983; 98,820-view sweep: no drawn arc >0.999° from its label, no student arc.
- Depth far-first, centre not forced last; CH4 35/−30 near H drawn last, labelled, bond present; openings and all 6 examples label every non-hidden atom.
- 50,544-render resvg audit: no off-canvas, text-text, text-stroke, text-dot or dot-on-bond; sticks ≥18° from bonds and ≥25° apart.
- Key/student heights equal 9×3; Print centre hatch survives Small 66 % (62 dark px, labels off); grey-only.
- Registration, FAQ(6), NO_HANDLES, examples, 30 tests, census 966fec87047e5439; turn 0–360/tilt ±90 swept; 10k fuzz, switches, purity, CPU pass.

## mutants (13, debug suite each)
Killed 10: M1 CH4 109.0, M2 no 1° arc guard, M4 atoms near-first, M5 bonds near-first, M6 lone 18°→0, M8 never hide, M9 drop viewer note, M10 centre label always, M11 footer 15 px, M12 student arcs.
Survived 3: M3 redundant edge-on guard; M7 pair-gap 25°→0 (the score, not the guard, separates pairs); M13 panel max (panels already equal).

## not checked
No image input: I did not view a PNG, so legibility, aesthetics and "does the arc look broken" are not judged; the 66 % findings are thresholded pixels, not a real photocopy. Live egui, browser metrics, PDF/DOCX, release timing and the full lib suite were not run.

## biggest gap
The lone-pair stick is screen-space invention, not a projection: at XeF4's opening it is 45° off the true pair direction and face-on the pairs are drawn 90° apart instead of 180°, while four texts say it points where the pair sits. A textbook draws a lobe on the central atom or a dot pair along the true axis, with "toward/away" when it is out of the page.
