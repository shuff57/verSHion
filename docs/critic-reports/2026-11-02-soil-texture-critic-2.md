# Soil texture triangle, critic 2 (accuracy + teacher + regression), commit db599c3

Verdict: FAIL. 0 blocker, 0 major, 2 MED, 5 LOW, 4 minor.
(The critic could not write its report file; condensed from its final answer. Tools, probe, PNGs and mutation log: scratchpad/soil-crit2/.)

Sources: Soil Survey Manual 2017 ch. 3 pp. 123-125 (fig 3-7 rendered and compared), NRCS Field Book Nov 2024. The Manual says loamy sand has "between 70 and 90 percent sand": only the inclusive reading leaves no gap.

Both round-1 problems are fixed and verified from scratch: own classifier from the Manual's wording has no gaps or overlaps over all 50,015,001 hundredth points and `texture_class` agrees at every one; `classify().win` agrees on the 0.1 % grid and every hundredth within 0.02 of a Manual line (1,022,150 points, 0 disagreements); polygons exact; the rendered key agrees typed all four ways (5,120 renders, 0 disagreements); shipped and FAQ examples correct (52/20 is Loam); orientation matches fig 3-7 (corners, number direction, tick slants, arrows, gridlines, guide landings); refusal advice applied with the critic's own parser on 6,000 fuzzed inputs, all ranges hold; sum tolerance 99.5/100.5 accepted, 99.49/100.51 refused; student copy differs from key only in description, answer outline and answer text; 35 module tests; census only the two soil-texture lines; first draw 145 ms.

## MED 1: the shipped student example sits on a line, and a guide gives away the hidden value
Example "Student copy: classify a sample": sand 30, silt 50, clay worked out as 20, guides on, show_worked off, answers off. The point is on the Loam / Silt loam line (silt exactly 50); the student copy prints no rule for points on a line, the key says Silt loam, so a student answering "loam" is marked wrong on a question the page cannot settle. With show_worked off the sheet prints "clay ____ %" but the clay guide's landing dot sits on the "20" tick, filling the blank. Repro: png/stu-noworked.png. Same as round 1 (critic 1 missed it).
Fix: ship an interior sample (e.g. sand 20, silt 60); with show_worked off drop the guide to the worked-out axis or say in the FAQ and hint that guides reveal it; optionally warn in the editor note when a student-copy sample is on a line.

## MED 2: at Large, labels still cover construction lines for a common band of samples
Critic 1's MED 1 is fixed for the named cases but fails one step over. Dense Large sweep (every 2 % of sand and clay, guides on): 76 of 1,326 renders (5.7 %) put a region label on a guide, hiding it 16-28 px and running through the letters (SiC 46 cases, SiCL 27). Sand 8-10 % with clay 34-92 % is hit every time (the sand guide splits SiCL and SiC into halves too narrow for the abbreviation; e.g. sand 10 / clay 35); silt 48 % (silt guide crosses SiC, e.g. sand 52/clay 0, 32/20, 40/12). Shipped two-sample example at Large puts "SiC" on sample 2's clay guide next to the marker under the "2" tag (png/x-two-example-Large.png). The builder's 7 % grid saw only 17 cases and its test accepts all (`could_be_free` finds no clear box). The FAQ "A label is given up for a shorter one before it is ever placed over a line" is false here. Mutant L2 (full name before abbreviation) survives the full suite.
Fix: when no clear place exists put the label outside the region on a leader (as S, LS and Si already are), or draw guides above the plates with a white halo on the text; assert that no label ever covers a guide with a sweep including sand 7-13 and silt 46-50.

## LOW
1. Clay guide through the leader dots (round 1 LOW 1 fixed only for the named case): at clay 1 %, any sand and size (343 renders), the clay guide passes 1.8 px from the centre of the Sand and Loamy sand leader dots (r 2.2), also Silt when the sample is to its right; at clay 0-0.6 % it crosses the leaders. Repro sand 60, clay 1, Medium, guides on. Fix: raise the dots above the clay-2 % line or end leaders at the edge with no dot.
2. Print erases the key's answer outline: the white strip under each guide wipes the 3 px outline where a guide runs along it, exactly when the sample is on a line (sand 52 / clay 20 in Print: two sides of the Loam outline become dashes, png/pt0-52-20-Large-print.png). Fix: draw the outline after the guides.
3. Print guide strength untested: mutants P2 (guide #999999, the builder's own survivor) and P3 (0.6 px) survive; the 66 % test asks only for dark ink at 45 % of points in a 3x3 window.
4. Three typed values: the rounding rule can change the class. 50/50/0.5 gives Silt loam but moving silt, or scaling in proportion, gives Sandy loam; 52.3/28/20 gives Loam but scaling in proportion gives Sandy loam. The note saying what was moved appears only in the editor. Fix: scale all three in proportion or print the adjustment in the key.
5. Stale UI hint: the sand field hint (soiltexture.rs line 83) still says three typed values "must add to exactly 100", contradicting the 99.5-100.5 tolerance.

## minor
- The FAQ says Sand, Loamy sand and Silt are "too narrow for their names"; at Medium "Silt" fits inside its region and the Manual names all three inside.
- Six fills (four in Print) cover twelve classes (known).
- Two close samples (40/20 and 41/21): markers overlap and the "2" tag plate sits on the key outline (known).
- Mutant L5 (leader landing-dot margin 0) survives; another rule covers it.

## Mutation: 25 mutants, 21 killed, 4 survive the full suite
Survivors that matter: L2 (MED 2), P2, P3 (LOW 3); L5 in effect equivalent.

## Teacher verdict and biggest gap
A textbook names all twelve classes inside their regions and never hides a construction line. This figure names Sand, Loamy sand and Silt below the axis on leaders, at Large its SiC and SiCL labels hide guides for common silty soils (sand 8-10 %), and its shipped student copy can put the question on a line with no rule to settle it.

## Not checked
A full figures:: run (load 12-20); release timing; live egui editor, browser fonts, PDF/DOCX; a dense Small/Medium sweep of the sand 8-10 band; sand subclasses (not offered).
