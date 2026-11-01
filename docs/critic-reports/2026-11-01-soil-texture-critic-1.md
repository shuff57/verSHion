# Soil texture triangle, critic 1 (accuracy + teacher + regression), commit dc12883

Verdict: FAIL. 1 blocker, 1 major, 3 MED, 4 LOW, 4 minor.
(The critic could not write its report file; this is its final answer, transcribed. Probe tools, extracted Manual text and PNGs are in scratchpad/scrit1-tools/, main results out/official.txt, out/collisions.txt, out/refusals.txt, out/leaks.txt, mut.log.)

Authority: the official Soil Survey Manual (2017) chapter 3, definitions on p. 124 and the triangle in figure 3-7 on p. 125 (extracted text scrit1-tools/ssm3.txt, page render scrit1-tools/ssm-p43-043.png). Classifier written twice from the Manual's wording (Python official.py, Rust probe.rs); no gaps or overlaps at any point tested.

## BLOCKER 1: points on a line get the wrong class, including a shipped example
The Manual's limits ("7 to less than 27 % clay", "52 % or less sand") already give every point on a line exactly one class. The figure overrides this with its own rule (class listed later in the legend wins), which is wrong on four line segments:

| Line | Figure says | Manual says |
|---|---|---|
| silt = 40 %, clay 40-60 % | Clay | Silty clay |
| clay = 12 %, silt 80-88 % | Silt | Silt loam |
| silt = 28 %, clay 20-27 % | Sandy clay loam | Loam |
| sand = 45 %, clay 35-40 % | Sandy clay | Clay loam |

Measured from the rendered key: 42 of 5,151 whole-percent samples are wrong (silt-40 line 21, clay-12 line 9, silt-28 line 7, sand-45 line 5); on the 0.1 % grid 402 of the 2,952 on-line points are wrong. All 498,549 interior points agree.
Repros (sand and clay typed): sand 52 clay 20 -> key says Sandy clay loam, Manual says Loam (this is the shipped example "A sample on a boundary", the FAQ worked example and an asserted test); sand 20 clay 40 -> Clay instead of Silty clay (also sand 10/clay 50, sand 0/clay 60); sand 45 clay 35 -> Sandy clay instead of Clay loam; sand 5 clay 12 -> Silt instead of Silt loam.
Why the builder's check missed it: the test cascade encodes the Manual correctly, but `near()` merges the cascade answers within +-2 hundredths into a set and the test only asserts `win == max(set)`; on a line the cascade's exact answer is never compared. The FAQ admits the Manual words limits as "at least"/"more than" then says the figure's rule settles them; every student copy prints "A point exactly on a line belongs to the class listed later in the legend", which no USDA source says; the stated reason ("roughly the finer one") also fails (Silt beats Silt loam).
Fix: classify with the Manual's cascade (already in the tests), keep polygons for drawing and the "on a line" note, drop "by convention" and the printed rule; in the test assert the exact point's cascade result including on lines.

## MAJOR 1: the triangle is a mirror image of the USDA triangle
Manual figure 3-7: 100 % sand at bottom-LEFT, 100 % silt at bottom-RIGHT; sand read on the bottom axis rising right to left, silt on the right edge, clay on the left. This figure has silt bottom-left and sand bottom-right (`xy()`: x = 600*(clay/2 + sand)/100), so every region is left-right mirrored while titled "USDA soil texture triangle". Internally consistent (guides and tick slants follow its own axes) but a student trained on the textbook triangle finds silt where sand should be.
Fix: x = 600*(clay/2 + silt)/100, re-place the three outside labels (Sand and Loamy sand move bottom-left, Silt bottom-right), re-derive gridline/guide/tick geometry and every test and example/FAQ sentence that states the orientation.

## MED 1: region labels cover the construction lines
Placement tries the full name allowing a guide underneath before the abbreviation with no guide underneath (loop order wrong); the label backing (0.88 opacity, solid in Print) is drawn after the guides and hides the dashed line. With one sample and guides on: Large 205 of 704 renders (29 %) have a label over a guide, Medium 121 (17 %), Small 65 (9 %); overall 819 text crossings and 752 backing-over-guide cases in 4,686 renders. Repros: sand 0 clay 40 Medium (silt guide under "Silty clay loam"); sand 50 clay 45 Large (clay guide through "Silty clay"). The FAQ line "a label that ... a construction line would cover moves" is false. Mutant M15 (always allow overlap) survives.
Fix: try every label variant strictly first, then relaxed, or draw guides after the labels; add a label-vs-guide assertion.

## MED 2: Print copies lose the guides and gridlines
Sandy clay's diagonal hatch runs at the same angle as the constant-sand guide: at sand 52 clay 20 in Print the sand guide disappears into the Sandy clay hatch and almost into the Sandy clay loam cross-hatch (crop scrit1-tools/out/crop-pt0-bw66.png). Gridlines (#888, 0.6 px) vanish entirely in the 66 % grey copy at 1x (scrit1-tools/png/st-print-student-Large-bw66.png); survive at 2x.
Fix: white underlay and solid black ink for guides, hatch angles no guide uses (re-check after the reorientation), gridlines 0.8 px or more.

## MED 3: real collision cases untested
M9 (remove the rule keeping an outside-label leader clear of a guide's landing dot) survives the 26 tests but matters: in 114 samples the rule moves a leader; without it the leader sits 1.15 to 4.85 px from the landing dot (example Small, sand 4, clay 5, guides on: Silt leader x 52.9 vs landing dot x 54.0). The sweep never puts a landing dot in a leader gap. M12 (number tags allowed over guides) survives; the audit found a guide crossing a "2" tag even unmutated.
Fix: sweep points that land each guide in the bottom label gaps; assert tag-vs-guide and label-vs-guide.

## LOW
1. Clay guide through leader dots: sand 95 clay 1 Large guides on: the clay guide (y 514.4) runs through the Silt and Loamy sand leader dots (y 512.6, r 2.2) and crosses all three leaders.
2. Chained refusal advice: sand "abc", silt "-4", clay "200": advice "0 to 100 in sand, for example 40. Type 0 to 60 in silt" - the second range only holds if sand is exactly 40.
3. Three typed values must sum to exactly 100.00: 33.33/33.33/33.33 is refused though lab sheets round to 99.9-100.1 (advice "clear silt" works).
4. No way to hide the worked-out value: the student copy always prints the computed silt, so "work out silt, then classify" cannot be set.

## minor
- Six fills serve twelve classes, so a legend swatch alone does not identify a class.
- Two samples 40/20 and 41/21 (Large): markers overlap, tag "1" equally close to both; tag backing covers the key outline.
- "1,000" refused as "more than 2 decimal places" (true but odd for a thousands separator).
- FAQ quotes loam as "7 to 27 % clay, 28 to 50 % silt", dropping the Manual's "less than".

## Builder's two "equivalent" survivors
- m23 (name-mode disc margin): equivalent, shown concretely (candidates sorted roomiest first; roomiest spare room at least 10.90 Small, 9.22 Medium, 5.86 Large, all Sand; any margin below 5.86 gives identical output).
- m20 (candidate clearance floor 4.0): equivalent for floors up to 10.69 on the critic's 2,112-setting sweep; not proven for all inputs.

## Mutation (critic's 16 mutants: 11 killed, 5 survived)
Survived: M9 (matters, MED 3), M12, M15 (MED 1), M13 (m20 floor 0, equivalent), M14 (m23 margin 0, equivalent). Killed: M1 (3 decimals), M2 (sum slack), M3 (tie rule), M4 (silt-28 vertex), M5 (silt guide end), M6 (silt tick numbers reversed), M7 (two Print neighbours share a fill), M8 (outline in student copy), M10 (advice example out of range), M11 (marker obstacle radius), M16 (three-typed fields swapped).

## Verified correct
All 12 polygons, every vertex, match the Manual p. 124 definitions; zero interior disagreements on the 0.1 % grid; 1,500 random hundredth samples typed four ways (sand+clay, sand+silt, silt+clay, all three) agree with the Manual and each other; abbreviations match the USDA set; 6,000 fuzzed inputs over all six fields (40 edge strings incl. NaN, inf, negatives, over 100, 100.0000001, commas, Arabic-Indic and fullwidth digits, NBSP, ZWSP, emoji): no panics, 5,958 refused, every advice (own parser) draws, every reason in the editor note, every bad field quoted; no leaks in student copies; 0 text/text, text/marker, text/tick, text/arrow, landing-dot/text or off-canvas collisions in 4,686 renders; debug first draw 82 ms, later 0.7-11 ms, no stale cache; diff touches exactly the 8 listed files, census gains exactly the two soil-texture lines and census_baseline passes, notes_for has both buffer-curves and soil-texture arms and compiles, NO_HANDLES right, 0 module warnings, 26 module tests pass; `cargo test --lib -j2 figures::` had 610 of 1,779 passing with 0 failures when the critic stopped (stalled on heavy tests from other figures).

## Not checked
Release-build timing (load about 60); live egui editor, browser fonts, PDF/DOCX export; sand subclasses (not offered); m20 beyond the sweep; every PNG.

Biggest gap against a textbook triangle: after BLOCKER 1, the mirrored layout.

Sources: Soil Survey Manual 2017 ch. 3 (NRCS, https://www.nrcs.usda.gov/sites/default/files/2022-09/SSM-ch3.pdf, fetched via web.archive.org); Florida rule (flrules.org notice 2556840) and SSSA glossary figure (agronomy.org soils-glossary figure-1.pdf), which agree on silty clay as 40 % or more clay and 40 % or more silt.
