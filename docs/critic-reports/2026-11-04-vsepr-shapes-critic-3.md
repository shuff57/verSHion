# vsepr-shapes — blind critic 3 (decision round)

Commit 55fd7fb (`figures-gapfill`), /tmp/figures-crit/vsepr3. No image input: I never viewed a PNG; every number below is measured from the SVG, the report geometry, rendered pixels, or an independent probe. 36/36 module tests pass; census baseline and handle-coverage pass; 14/14 mutants killed.

**Verdict: FAIL — 0 blocker, 0 major, 2 med, 5 low, 3 minor.** Ships? No: the depth order and one panel note are wrong at SF6's default view.

## MED
1. **All bonds are painted before all balls, so a farther ball erases a nearer stick.** Repro: `molecule=SF6`, turn 0, tilt 0 — the view the dropdown gives. In the SVG, bond b3 (+y, `data-d="0.17101"`, y 284.00→205.12, w 3.65) is followed by atom a6 (−z, `data-d="-0.93969"`, centre 258.00,259.64, r 16), so the far axial F erases the near +y stick for y 243.6–275.6, where the stick's local depth is +0.12 and the ball's is −0.94 (1.06 behind). 2,283/23,976 views (5°×5°, all nine molecules) have a bond middle inside a farther ball; worst depth error 1.53 (PCl5 10/−70, b3); per molecule SF6 812, PCl5 440. The shipped collision test never checks depth. Fix: interleave bonds with the balls (sort each bond by its far end, or split it), then test it.
2. **The "same line of sight" note claims a hiding that is not there** (vsepr.rs:1067–1071). Same SF6 default: the panel prints "Two bonds lie on the same line of sight; one atom is behind the other." while a3 (y 189.1) and a6 (y 259.6) are 70.5 px apart and both fully visible. The trigger is `angle_gap(screen angle) < 2°` (:1043) — a shared *ray from the centre*, not a shared line of sight — and it is singular although two such pairs (four bonds) lie on x=258. It fires in 678/23,976 views, 498 with no collinear pair within 20 px, and again at SF6 abs 0/−1 beside the near-centre note. Reword ("project onto the same line on the page") and drop the hiding clause.

## LOW
3. H2O opens with 104.849° on the page against the printed 104.5° (+0.349°). Inside the 0.4° policy and 0.2 px at the 35 px radius, so not a MED, but it is the worst opening and "the arc is the true angle" is a 0.4° tolerance, not exact. Sweep max 0.3994° over 4,973 arcs.
4. BeCl2: 216 of its 2,654 drawn arcs have chord deviation <0.5 px (at or near the edge-on views: tilt ±90, or turn 90/270 with |tilt|>10°), all 216 wholly under the two bond sticks and the centre ball, so only the floating "180°" label is visible (e.g. turn 0, tilt 90: the polyline is y=300.00, x 223.9–292.1).
5. 66 %: at the openings PCl5 b2/b4/b5 read 89/82/71 and CH4 69–90, but PCl5 at relative 90/0, Small, Print has b5 (13.3 px, w 2.46) at 5×5 min 132 for t=0.30–0.65; the "every bond" test samples only the three opening views.
6. 55/10,656 lone-molecule views (XeF4 rel 20/±75–80) put both dots of a pair inside a *nearer* ligand ball; 70 views hide one dot. Correct depth order, but the pair vanishes with no note.
7. Header:35–36, hint:532, footer:1122 and FAQ:3957 name only "lands on a bond or collapses" as omission reasons; the third case (two pairs on top of each other, :1086) exists only in the panel note.

## minor
8. `Mol::display` is documented "Formula with subscripts" (:117) but all nine are ASCII; the panel, choice and FAQ print CH4/H2O/SF6, not CH₄/H₂O/SF₆.
9. SF6 abs 0/−1: the far axial F is 2.06 px behind the centre and effectively hidden by the S ball, but the 0.6 px rule (:803) does not flag it, so no "away" note; a ~2 px cap of that far F peeks below the centre ball, unlabelled and unexplained (the near-centre and the false collinear notes are the only ones printed).
10. docs/figures-progress.md:1005 still says "first critic in flight"; no round-2/3 entry (round-2 minor 14 open).

## verified
- Chemistry from coordinates: 9/9 atom, lone-pair, domain and AX counts; shapes and electron-domain names; every stated angle within 0.03° of its coordinates (CH4 prints 109.5, coordinates 109.4712; NH3 107.00, H2O 104.50, SF4 102.00 exact); XeF4 pairs 180° apart and 90° from every bond; H2O pairs 115°, 109.2° to a bond; SF4 axial/equatorial 90°; the lone-pair compressions are in the coordinates.
- Projection: R = Rx(tilt)·Ry(turn), orthonormal, det 1, z orthographic and unscaled; 360° folds to exactly 0° byte-for-byte for all nine in both styles.
- Lone pairs: 10,656 views, 0 drawn pair >2° from the projected 3-D direction, 0 omitted without a "not drawn" note, 0 drawn dot within 2 px of a bond (min 4.21), between-pair dots ≥5.03 px; XeF4 0/0, 0/20, 30/20 and SF4 90/0 match their four notes; the texts say "truly projects".
- Depth of dots: no farther ball paints over a nearer lone dot in 10,656 views; XeF4 Large abs 0/80 omits both on-bond pairs with the note.
- Arcs: 23,976 views, 4,973 arcs, none reads >0.4° from its label, no empty number; the student copy has none.
- Description: matches what is drawn in all 23,976 views (bond angles iff an arc, lone pairs iff a dot); 0 leaks in 23,976 student views (10 shape words, 8 angle strings, no `data-o="angle"`).
- Collisions: 10,800 resvg-ink audits (10 turns × 10 tilts × 9 × 2 styles × 3 sizes × 2 copies) plus 7 examples at Small Print: 0 text-text, text-canvas, text-ball, text-bond, text-dot or visible-text-arc problems. My "text on arc" hits were all the documented angle-label halo.
- 66 %: centre label ink 11–51, min type 8.712 px, every arc segment dark in 5×5 at the openings; only LOW 5 fails.
- Registration, FAQ(6), examples(7), NO_HANDLES, census `f6e352ecedd9ae53` (defaults = start) pass; 504 pole builds, turn 0–360, tilt ±90, switches off, 10,000-char name, NaN/∞, 10k fuzz and purity/CPU tests pass.
- XeF4 opening: honesty judged acceptable, not a MED — no pairs are drawn, the panel says why, the description omits lone pairs, and four texts state the rule; a made-up stick would be worse. Same for the H2O residual (LOW 3).

## mutants (14, all killed)
dir nudge (+5°), drop on-bond note, on-bond threshold 0, reverse depth sort, drop near-centre note, "away"→"at", thin bonds, no arcs, arc filter 5°, why_plain leak, desc always lone, dot-overlap threshold 0, panel max removed, turn not folded. 0 survived.

## not checked
No image input: I did not view a PNG, so aesthetics, "does the broken stick look wrong", and photocopy appearance are inferred from geometry and thresholded pixels. My ink audit used the system Noto/Liberation fonts (the crate's bundled+system db was not reproduced; the shipped audit agrees). Live egui, browser metrics, PDF/DOCX, release timing and the full lib suite were not run.

## biggest gap
The model's depth order is only partial: bonds sort among themselves and balls+dots sort among themselves, but every ball beats every bond, so 2,283/23,976 views (including SF6's default) draw a far ball over a near stick; the panel then adds a collinear note that claims a hiding which is not there. Interleave the sticks with the balls and reword that note.
