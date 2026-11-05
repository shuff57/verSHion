# Xylem and phloem, critic 1 (accuracy + teacher + regression), commit 5bdb028

Verdict: FAIL. 0 blocker, 0 major, 4 MED, 7 LOW, 4 minor. (Condensed from the critic's final answer; probe, parser, PNGs, mutant logs in scratchpad/xylem-crit1/.)
Method: 2,304 SVGs (32 label sets x Stem/Root/Both x lengthwise on/off x key/student x 3 sizes x Colour/Print), own Python parser, resvg ink boxes, 37 PNGs, 24 mutants via an env switch.

## MED
1. "Water and minerals" and "Sucrose" blanks sit closer to other parts than to the arrows they name. Opening state (Large, answers off, Colour/Print); example 3 student copy at Medium. Disc 20 has no leader: 14.0 px from the vessel wall, 28.0 px from the nearest water arrow (disc 16 Xylem vessel beside the same vessel). Disc 21 is 8.0 px from the leaf tip, 18.0 px from the arrow tail (starts inside the leaf); "Mature leaf" disc 22 right below. A student writes "xylem"/"leaf"; key accepts neither. Example 3 does not label the vessel at all. Fix: short leaders ending on an arrow shaft mid-element (not the loading arrow in the leaf); test each arrow blank is nearer its own arrow than any named part or organ name.
2. Vascular bundle bracket reads as a plain line ending at the pith (Stem/Both, label xylem and phloem on; disc 1 in example 3 at Medium, disc 3 default student copy at Large). Ticks 3 px (~0.8 pt printed), spine straight with its own lead, inner end 2.5 px from a pith cell. Student writes "pith"/"ground tissue". Tests check span and side, not visibility. Fix: ticks 7-8 px, lead from spine midpoint at a right angle, or bracket outside the ring; test tick length at print scale.
3. Default/opening state cannot be printed legibly. Both + lengthwise is 1,779 px wide at Large, 1,566 Medium, 1,459 Small; label size scales text only. layout::picture_size_pt prints at 0.75 pt/px capped to stem column (<468 pt Letter) and 216 pt tall: scale <= 0.263; Large names 4.8 pt, captions 4.4 pt, discs 7.7 pt across; Medium names 3.9 pt. Other widths at Large: Stem 679, Root 600, Stem+lengthwise 1,180, Root+lengthwise 1,101, Both w/o lengthwise 1,278. Empty ~140 px band under the stem and root panels. Fix (any): open on a narrower view, stack panels vertically, or at least an editor note when names would print below ~8 pt.
4. Tests do not check the key's biology: 7 of 24 mutants survive with all 21 module tests passing. M2 caption "xylem toward the epidermis, phloem toward the pith"; M3 "sucrose moves from sinks to a source"; M4 Source tag on "Young leaves and fruit" and Sink on "Mature leaf"; M10 "pericycle surrounds the endodermis"; M17 one xylem arm with no vessels; M19 last student table row has no writing line; M24 water "up or down". M5 (swap Water/Sucrose names) is killed only by the collision sweep. Fix: assert each caption sentence, organ-to-role tags (Mature leaf Source; Young leaves and fruit, Roots Sink), >= 3 vessels per arm smallest at the tip, one writing line per table row, each arrow name nearest its own arrow.

## LOW
1. Implies four arms is THE dicot pattern; FAQ says "an X of four arms". Dicot roots 2-6 arms (tomato 2, pea 3, Vicia 4, Ranunculus 5). Add "four drawn; dicot roots have 2-6".
2. Root phloem floats mid-way: spans r 28-48 in a stele of r 58.8, ~11 px inside the pericycle. PATCH_R about 47.
3. Key wording: "Xylem vessels are dead, hollow cells with thick lignified walls": a vessel is a tube of many dead cells; OpenStax 30.2 says vessel elements have thinner walls than tracheids. Use "tubes of dead vessel elements with lignified walls and open ends".
4. Casparian strip text says "in its radial walls"; strip is in radial and end walls (cross-section shows radial). Drawing right.
5. Xylem vessel pointer at y=72 lies on a wall-thickening bar: VESSEL_Y 76.
6. Panel title "Lengthwise view of the stem" runs from roots to shoot tip.
7. M5 killed only by accident (see MED 4).

## minor
Companion cells 2 px from sieve elements (really adjoin); vessel wall mixes first-formed ring pattern with uniform thick wall; root xylem pointer runs down an arm over two vessels and ends in a third's centre (known); stem ground tissue flat colour, no cells.

## Biggest gap vs textbook
Root phloem floats mid-way between the xylem arms; textbooks (OpenStax fig 30.18, Campbell Ranunculus root) put it at the stele edge between arm tips, just inside the pericycle.

## Verified correct
Biology vs OpenStax 30.2/30.3/30.5 (stem layout and orientation, root exarch xylem, Casparian marks per radial wall, root hairs, open vessel, sieve plates, one companion cell with nucleus per element, directions, source/sink). 1,152 Colour renders: zero collisions of any kind, no pointers/arrows within 3 px, nothing off canvas. 576 answer keys: pointers end >= 2.0 px inside their part, Casparian stops 0.80 px short, none crosses another named part (sieve-plate pointer brushes its two elements by design). Print: all 1,152 have identical geometry to Colour, grey only, 66% grey keeps cambium/Casparian/plates/nuclei dark, phloem ruled, sucrose dashed vs solid water. No leaks in 1,152 student copies. All 7 notes name real controls. Odd params safe. Defaults/start right. Regression: 21 module tests, census 4 (fixture differs only by the two xylem lines vs 55c35e7), faq 1, start 5, handles 5, panel-faq 1, full figures:: 1,893 passed 0 failed; only a 20 s CPU-time bound.

## Not checked
Live egui/wasm editor (preview fit read from code), real PDF/DOCX export (print sizes computed), Campbell/Raven directly, browser fonts, photocopier.
