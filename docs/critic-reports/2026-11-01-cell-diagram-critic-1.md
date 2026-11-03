# Cell diagram, critic 1 (accuracy + teacher + regression), commit b587bcb

**Verdict: FAIL**: 0 blockers, 1 major, 4 MED, 4 LOW, 9 minor.

How I tested
- Own clone `dcrit1` at b587bcb, plus a second clone `dcrit1m` for mutants. Both were restored afterwards (`git status` clean).
- My probes are in `scratchpad/dcrit1-probe.rs`. I included them into the module's `tests` only for the runs.
- Text boxes are resvg ink boxes (`Text::flattened().abs_bounding_box()` with the repo fontdb).
- Discs, dots, leaders, rules and every scene polygon/polyline are read back from the SVG. Landing and clearance use top-most hit tests on the drawn shapes, textures included, in drawing order.
- About 120 PNGs (1.3x, 3x, 66 % bitonal) are in `scratchpad/dcrit1-png`. These cover every cell type × key/student × Small/Medium/Large × Colour/Print × functions, each group alone, the start state and all 6 examples. I opened about 20 of them plus zoom crops.
- I do not trust the module's constants. My first-pass clearance audit was wrong because it ignored overdraw, and I discarded it. The numbers below come from the corrected top-hit audit (`c_grid2`).

## Findings

### MAJOR 1: the pointer dot hides the ribosome it marks, and every pointer looks like a ribosome
- What is drawn:
  - A pointer is a solid black disc of r 2.2 with a white 1.1 ring, so its outer radius is 2.75.
  - A free ribosome is a disc of r 2.5 (dark red in Colour, #111 in Print). RER ribosomes are r 1.9.
  - The Ribosome pointer sits on a free ribosome and covers it completely.
- What a student sees:
  - The Ribosome pointer looks exactly like the Cytoplasm pointer: a dot on a bare cytoplasm patch.
  - Zoom crops of the default animal key (pointers 2 and 4) are pixel-for-pixel the same design.
- Reproductions:
  - Defaults (animal), pointers 2 "Cytoplasm" and 4 "Ribosome".
  - Default plant, pointer 15.
  - Bacterium, pointer 7.
  - Example "Protein-making pathway, student copy" (ex4): pointer 5 is a bare dot in the cytoplasm, and cytoplasm is not labelled there, so a student will write "cytoplasm" and be marked wrong.
- The same marker hurts other pointers:
  - In Print the Rough ER pointer (default plant, pointer 5) is a black dot in a white sac among identical black ribosome dots. It reads as "ribosome". Its measured clearance to the nearest ribosome is 0.25 px.
  - Ribosome dots lying under other leaders look like leader ends. Measured: 1,022 ribosome dots under leaders, in 492 of 744 layouts (every type × plasmo × mask 1..31 × functions × size). Examples:
    - default animal, the Nucleus leader passes over a ribosome;
    - ex4, a ribosome sits on leader 2 (Nucleus).
- Fix direction:
  - Use a pointer mark that cannot be mistaken for a structure, for example an open ring of at least 4 px or a short bar or arrowhead, drawn so it does not hide the target.
  - For Ribosome, end the leader at the edge of a visible ribosome, or point at a small cluster (polysome), not on a single dot.
  - Treat ribosome dots as obstacles for leaders (CLR_A), or move the dots that lie under a chosen leader.

### MED 1: "the ring around the dot touches nothing else" protects 0.05 px; nucleus and RER pointers sit on neighbours
- `RING = 2.8` px is measured from the anchor, but the drawn dot's outer radius is already 2.75. The guaranteed clearance is therefore 0.05 px.
- Measured clearance from the dot's edge to the nearest structure that is not allowed:
  - Nucleus pointer: 0.35–0.5 px from the nucleolus in 162 of the grid's nucleus pointers.
    - Example: `cell=Plant cell, g_boundary+g_membrane only, Large`. The anchor is (−148.1, −74.1) and the nucleolus centre is (−134, −74) with r 11, so the dot edge is 0.35 px away.
    - The nucleolus is not labelled there, so a student who writes "nucleolus" has a defensible answer.
  - Rough ER pointer: 0.25 px from a ribosome in 288 pointers, including the default plant (start state).
- Fix direction: test the ring at the dot's outer radius plus about 3 px (RING ≈ 6), and re-run the anchor census.

### MED 2: Print at 66 %: the cell wall reads solid black and swallows the membrane, so the wall and membrane pointers look alike
- The wall is #8a8a8a in Print, which is darker than the 66 % threshold by design ("reads black"). The membrane is a #111 line on its inner edge.
- In the bitonal copy both form one black band. The plant and bacterial student copies show:
  - the Cell membrane pointer as a white ring at the band's inner edge;
  - the Cell wall pointer as a white ring in the middle of the band.
- Leaders vanish inside the band. Reproductions: `cell=Plant cell, style=Print, show_answers=off` (pointers 10 and 13), and the bacterium (pointers 4 and 6).
- The same cause makes the Central vacuole leader (plant, pointer 11) disappear into the horizontal hatch at 66 %.
- Fix direction:
  - Give the wall a sparse hatch, or a grey that stays white at 66 % (≥ #c0c0c0), so the membrane line stays visible.
  - Draw leaders with a white halo (underlay) inside hatched or dark fills.

### MED 3: leaders run across other organelles
- Measured over 744 layouts (5,712 leaders), not counting membrane, wall and capsule, which an inner pointer must cross:
  - 1,790 leaders cross another structure.
  - About 1,200 of these are not the natural envelope/nucleus crossing of a Nucleus or Nucleolus leader.
- Crossings in the defaults:
  - Animal defaults: the Nucleus leader runs through the rough ER and the Golgi stack, and the Nucleolus leader crosses a mitochondrion.
  - Plant start state: the Golgi leader crosses the central vacuole and a cytoskeleton fibre; the Vacuole and Smooth ER leaders cross cytoskeleton fibres.
  - ex4 (protein student copy): the Rough ER leader passes through an unlabelled lysosome.
- Other frequent cases:
  - RoughEr over SmoothEr + cytoskeleton: 165
  - Golgi over Vacuole: 140 + 47
  - Golgi over Chloroplast: 51
  - RoughEr over Lysosome: 49
  - Envelope over Mito: 37
  - Ribosome over Flagellum: 24
- A textbook figure keeps leaders off unrelated organelles. Here a line through the Golgi to the nucleus, with a ribosome on it, invites a wrong reading.
- Fix direction: add each non-target, non-container part's polygon as a cost term in `assign`, with a large weight per crossing (below a violation, above length). The search already explores side and slot.

### MED 4: blanks with a second defensible answer that the key does not accept
- **Nucleoid**: it is drawn as a dashed region full of DNA loops. "DNA", "chromosome" and "chromosomal DNA" are what many GCSE keys expect (AQA labels it "chromosomal DNA"). The key accepts only "nuclear region".
- **Centriole**: the pair at right angles is what many texts label "centrosome", and the key does not accept it.
- Fix direction: add "DNA, chromosome, chromosomal DNA (Nucleoid)" and "centrioles, centrosome (Centriole)" to `alt`, and state them in the FAQ.

### LOW 1: the plant vacuole does not dominate, and the FAQ says it does
- The vacuole is 34.2 % of the cell interior (36,757 / 107,439 px²). The test only requires more than 33 %.
- The FAQ says "a large central vacuole that fills most of the cell". A mature plant cell in a textbook shows about 50–90 %.
- Fix: enlarge the vacuole to at least about 55 % (moving the chloroplasts and mitochondria into the cortical cytoplasm), or reword the FAQ.

### LOW 2: structures that the captions name are not drawn
- The nuclear envelope has no pores, although its function caption says "Double layer with pores".
- The chloroplast has a single outline (1.6 px), while the mitochondrion has a double membrane, so it does not show the double envelope.
- Bacterial ribosomes are drawn the same size as eukaryotic free ribosomes (r 2.5), with no 70S cue.
- Fix: gaps in the two envelope rings; a second inner outline on the chloroplast.

### LOW 3: the module's tests do not pin what makes each organelle recognisable, the cache key, or Print series
My mutants: 20, 10 killed, 10 survived. 8 of the survivors are real test gaps; 2 are equivalent (M16, M20). Details are in Mutation below.
- Undetected changes to the recognisable shapes:
  - M4: no inner mitochondrial membrane
  - M5: no cristae
  - M6: no grana
  - M7: no ribosomes on the rough ER
  - M18: a single-line nuclear envelope
- M10: the layout cache key ignores the label size. This is not live (my 600-step random walk shows 0 stale renders), but nothing would catch it.
- M13: Print mitochondria use series 0. The builder says series 0 "vanishes at 66 %", but the 66 % test passes anyway.
- The builder's "equivalent" thick-line slack mutant is **not equivalent**:
  - Open-stroke slack +0.6 px moves the animal cytoplasm anchor (119, 31) → (114, 31).
  - It changes 2 renders: Animal, mask 19 and mask 23, functions on, Large.
  - It survives only because no test pins anchors or those renders.

### LOW 4: the flagellum's basal body is an unnamed solid black disc (r 4.5) inside the cytoplasm
- In the default bacterium it sits about 13 px from the Ribosome pointer. It reads as a large ribosome, or as the thing pointer 7 marks (see MAJOR 1).
- Fix: draw it as a small hook or ring that straddles the membrane, not a filled disc inside.

### Minor
1. The accepted words "slime layer" for Capsule, "fimbria" for Pilus and "cytosol" for Cytoplasm are not true synonyms at AP or college level. A slime layer is loose while a capsule is organised. Campbell separates fimbriae from sex pili. Cytosol excludes organelles. These are defensible at GCSE, but the FAQ presents them as the same structures.
2. The pili start on the capsule's outer edge and are not anchored through it to the wall or membrane.
3. Start state: the Plasmodesma leader (pointer 8) runs about 150 px inside the wall band before leaving it.
4. Debug-build first draw: animal scene construction 1.10 s, plant 0.43 s, bacterium 0.33 s (`free_dots` clearance grid). Release (opt-level z, LTO off): 104 / 40 / 35 ms, and later layouts are ≤ 15 ms. The builder's "1 s worst layout" is actually the scene, not the layout.
5. M16 (the advice names the last group that holds something instead of the first) survives. It is equivalent: both draw.
6. M20 (`Hit::Other` allowed in the ring) survives. It is equivalent in the current scenes: 0 anchor or render changes over 768 layouts. It would bite if a vesicle were moved.
7. The student copy keeps an empty heading row, about 40–60 px of white at the top.
8. "Releases energy as ATP" (mitochondrion) is GCSE phrasing. "Site of aerobic respiration; makes most of the cell's ATP" is safer.
9. Teacher-chosen groups: Nucleoid is listed under "protein-making pathway", which is a stretch. It is defensible as the DNA store, and the hint says so.

## Teacher verdict and the biggest gap
- Could I drop it into an exam? Not as is.
  - The ribosome blank and the cytoplasm blank look identical (MAJOR 1).
  - The Print photocopy merges membrane and wall (MED 2).
- Otherwise the cells are accurate and clean, the labels never collide, and the key and student copy match.
- **Biggest gap against a textbook cell diagram:** the pointer convention. Textbooks end the leader on the structure's edge with no dot, so a tiny structure such as a ribosome stays visible and no dot can be mistaken for a structure.
- On content alone, the biggest gap is that there are no nuclear pores (and no chromatin), although the envelope caption cites pores.

## Verified correct (measured)
- **Biology**
  - Animal cell: no wall, chloroplast, central vacuole or prokaryote parts.
  - Plant cell: no centrioles or lysosomes.
  - Bacterium: no nucleus, envelope, nucleolus, ER, Golgi, mitochondria, chloroplasts, vacuole, lysosomes, centrioles or cytoskeleton. It has capsule, wall, membrane, nucleoid (dashed, no envelope), plasmids, ribosomes, flagellum and pili. The FAQ says why there is no cytoskeleton and where respiration happens.
  - The rough ER touches the envelope. The smooth ER continues from the rough ER. The Golgi stands near the ER, and vesicles sit beside the Golgi.
  - The mitochondrion has a double membrane and cristae. The chloroplast has grana and a stroma.
  - The nucleolus is inside the nucleus. The plant membrane is inside the wall, and the capsule is outside the wall. The animal nucleus is 12.1 % of the cell.
  - All 23 function captions are factually right (minor 8 aside) and name no structure.
- **Collisions**
  - 3,072 renders: 3 types (plant with and without plasmodesmata) × 32 masks × functions × 3 sizes × key/student × Colour/Print.
  - Ink-box audit, all at 0:
    - text off canvas, text/text, text/disc, number outside its disc, disc/disc;
    - leader/leader < 2.5 px, leader/text, leader/other disc, leader < 4.7 px from another pointer dot;
    - text over the drawing, disc over the drawing;
    - rule/text, rule/disc, rule off canvas.
  - Every dot's top-most drawn shape, textures included, is its own structure. No dot lands on a crista, granum or DNA loop.
  - Large labels and the 2-column table at Large with functions fit.
- **Leaks**
  - 1,536 student copies via `FigureDef::svg`, the real export path including `annotate`, were scanned.
  - No structure name, accepted synonym, cell-type word, or function word with functions off (energy, protein, DNA, ATP, respiration, photosynthesis, ...).
  - No comment, aria, class or data- attribute; ids are only `vs-hatchN`; title "Cell diagram"; desc neutral.
  - With functions on, only the caption words appear.
- **Edge cases**
  - 748 odd JSON values through `params_from_json` across all 11 keys (strings, numbers, bools, non-ASCII, `<script>`, NUL, RTL): no panic, and the notes are truthful.
  - A spec with only `cell` loads every group on through `params_from_spec`. Unknown keys are ignored (13 discs).
  - A raw empty `Params` draws a bare cell with the "No label group" note.
  - The bacterium energy/membrane notes name the ticked groups and advise "Tick cell boundary instead", which draws.
- **Cache**
  - 600 random walk renders on one thread are byte-identical to fresh-thread renders.
  - Animal with plasmodesmata on equals animal without.
  - The keys cover every input to `compute`.
- **Performance**
  - Release: layouts mean 2.8 ms and worst 150 ms (the first animal scene). Clicks after the first are ≤ 4 ms first-time and 0.3 ms cached.
  - Debug: mean 14 ms, first animal scene 1.1 s.
- **Mutation:** 20 mutants, all valid. M16's first pattern was not unique; I re-ran it with a unique one.
  - Killed (10): M1 mito group, M2 centrioles removed, M3 lysosome in plant, M8 heading in student copy, M9 RING 0.5, M11 cache ignores plasmo, M14 nucleolus group, M15 flagellum anchors, M17 vacuole 25 %, M19 chloroplast in animal.
  - Survived (10): M4, M5, M6, M7, M10, M12, M13 and M18 are real test gaps (LOW 3). M16 (minor 5) and M20 (minor 6) are equivalent.
- **Regression**
  - The diff touches exactly the 8 listed files. The census gains exactly `cell-diagram__defaults` and `cell-diagram__start`.
  - `cell-diagram` is in `NO_HANDLES`, in the right alphabetical place, and the figure has no handles.
  - 0 compiler warnings in celldiagram.rs (all 26 lib-test warnings are in other files).
  - The module's 27 tests pass.
  - **Full `cargo test --lib figures::` on b587bcb: 1723 passed, 0 failed, 4 ignored.** This includes `census_baseline`, `handle_coverage_report`, `overrides_are_valid_and_render`, `all_figures_render_with_defaults` and `every_figure_has_a_short_faq`. `absurd_windows_cannot_run_away` passed this time.

## Not checked
- The live egui editor (whether render runs on the UI thread there), browser font metrics, and PDF/DOCX/Student + key pair exports.
- The production release profile with fat LTO. My release numbers used LTO off and opt-level z.
- Xvfb screenshots of the real app (`VERSHION_SCREEN`).
- Hand-reading of every one of the 3,072 images; I looked at about 20 plus zooms.
- How students actually read the marks. MAJOR 1 is my judgement from pixel-identical crops.
