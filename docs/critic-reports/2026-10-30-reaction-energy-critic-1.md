# Critic 1: reaction-energy-diagram (f4175bb)

Scope: `src/figures/chemistry/reactionenergy.rs` and its wiring (chemistry.rs, chemistry/handles.rs, faq.rs arm, mod.rs notes_for, census).
(Restored from the critic's final report after a reboot cleared the scratchpad. The critic's PNGs, dump crate and checker scripts were lost; repro parameters are exact, so re-render them with the binary.)

## VERDICT: FAIL

- Blockers: **1** (B1, misleading or contradictory chemistry in the two-step "overall Ea" and "Ea (reverse)" when the intermediate lies below the reactants or products. Borderline: it follows the textbook definition, but the page then contradicts itself. If ruled MAJOR, the verdict is still FAIL.)
- Majors: **3** (M1 Intermediate label misplaced, M2 Ea (step 2) / Ea (catalysed) labels far from their arrows, M3 long or formula product names placed away from the plateau)
- Minors: 10

## Counts

- Sweep: 3,776 renders (3,200 random over the full parameter space, seed 11, plus 576 plain classroom cases covering sizes x Colour/Print x student/key x one/two steps x catalyst x axis). 1,851 were student copies, 1,778 had a catalyst, 1,549 were two-step, 1,887 were Print.
- Real-glyph geometry audit (usvg boxes, not the builder's `tw()` estimate):
  - Label on label, label on arrow/curve/axis, axis text on axis text, any text off the canvas: **0 / 3,776**. The no-overprint claim holds.
  - Labels crossing a dotted level line (haloed, soft): 40 cases.
  - **Label attribution failures** (a label more than 15 px from its own feature and at least 6 px nearer a different one, or more than 40 px x size factor from its own): **234 cases**. By configuration: one-step no catalyst 3/1,199; one-step + catalyst 10/1,028; two-step no catalyst 25/799; **two-step + catalyst 196/750 (26%)**. In the plain grid alone, two-step + catalyst: **44/144 (31%)**.
- Value read-back: stationary points, Bezier control points, level ticks, tick linearity, each arrow's tail and tip, and every key working line recomputed in Decimal: 0 real mismatches (two flagged cases were the critic's own half-up rounding of 3-decimal inputs where Rust f64 rounds down: 4.685 -> 4.68; the page is self-consistent).
- Arrow ends: every arrow end lies on the curve or on a dotted level line in all 3,776 renders.
- Student-copy leaks: text, `<title>`, `<desc>`, aria in 1,851 student renders: 0 leaks. The key carried every value line in all key renders.
- Determinism: 500/500 SVGs byte-identical over two runs.

## Findings

### B1 (BLOCKER, chemistry) - two-step "Overall Ea" / "Ea (reverse)" contradict the mechanism when the intermediate is well below the reactants or products
`reactionenergy.rs:131-135` (`top`, `ea`, `ea_rev`), readout `:911`, `:917`, qualitative key `:964-968`, FAQ `faq.rs:3654`.
`VERSHION_FIG_PARAMS='{"steps":"Two steps","reactants":100,"products":80,"ea":20,"intermediate":0,"ea2":110}'`.
The key prints "Ea (step 2) = 110 - 0 = 110 kJ/mol" then "Overall Ea = E(highest peak) - E(reactants) = 120 - 100 = **20**" and "Ea (reverse) = 120 - 80 = **40**".
- Forward: the system drops into a 100 kJ/mol well and must climb 110: overall barrier (energetic span) is 110, not 20.
- Reverse: from the products the path climbs to TS2, drops to 0 and must climb 120 to TS1: reverse barrier is 120, not 40.
The page states an overall Ea smaller than one of its own step Ea values. The qualitative key says the same ("the overall Ea runs from the reactants up to activated complex 1, the higher peak"). Any intermediate below the reactants is accepted without a note (`:172` only bounds it from above). In the random two-step sweep 471/1,261 cases printed an Overall Ea below Ea (step 2).
"Reactants to the highest TS" is the common textbook definition and is correct when the intermediate lies above both ends. Options: bound the intermediate to at least max(reactants, products) with a note; or compute the span (TS minus the lowest state before it) and word the line accordingly; or at minimum add a note when the intermediate is below either end.

### M1 (MAJOR) - "Intermediate" label placed away from its valley, under the reactants' line, among the arrow labels
`reactionenergy.rs:732-735` (`around_point(..., above=false)`: only under the valley or beside it) plus `Placer::find` `:309-332` (a tier-0 place further away beats a near place crossing a dotted line).
The space under the valley is taken by the catalysed dashed curve, the reactants' dotted line or the Ea (step 1)/(catalysed) arrow feet. The label then drops below the reactants' level, often sideways next to the Ea (step 1)/Ea (catalysed) labels.
- `{"steps":"Two steps","reactants":100,"products":20,"ea":60,"intermediate":110,"ea2":70,"catalyst":true,"ea_cat":30,"style":"Print"}`: reads "Ea (step 1) Intermediate" on one baseline, under the 100 line, ~50 px left of and 30 px below the valley.
- The same with `"label_size":"Large"` (Colour, and Print): a stacked "Intermediate / Ea (step 1) / Ea (catalysed)" block while the valley sits 90 px to the right.
- `{"reactants":150,"products":100,"ea":150,"label_size":"Large","style":"Print","show_answers":false,"steps":"Two steps","catalyst":true,"axis":"Numbered (kJ/mol)","intermediate":190,"ea2":20,"ea_cat":30}` (student Print): "Intermediate" in the middle of the dashed curve under the 150 line, 51 px from the valley.
- Without a catalyst, the builder's own example "Two-step reaction with an intermediate" and `{"steps":"Two steps","reactants":100,"products":20,"ea":60,"intermediate":110,"ea2":50,"axis":"Unlabelled"}` put the label under the reactants' dotted line, with that line between the label and its valley (milder).
Sweep: misattributed in 28 of 144 plain two-step + catalyst cases. Fix: try above the valley (inside the well, between the peaks), or accept a dotted-line crossing near the feature before a clear place far away.

### M2 (MAJOR) - "Ea (step 2)" and "Ea (catalysed)" labels land far from their arrows; in Print the student cannot match them
Placement order and far rings: `:750-756` (arrow names are placed last), `ring` `:404-415`, `beside_arrow` `:378-401` (candidates all near the arrow, get exhausted).
- Two-step + catalyst at Large (Colour and Print): "Ea (step 2)" at the very top of the canvas, above "Activated complex 2", reading as a title for the peak; its arrow is 70 px below.
- `{"reactants":40,"products":40,"ea":80,"label_size":"Large","style":"Colour","show_answers":true,"steps":"Two steps","catalyst":true,"axis":"Numbered (kJ/mol)","intermediate":60,...}`: "Ea (step 2)" between the peaks, 62 px from its arrow and nearer the Ea (step 1) arrow.
- r=150, p=150, ea=100, Large, two-step + catalyst: "Ea (step 2)" bottom right by the products, "Ea (catalysed)" bottom left under the reactants, 62 px from its arrow.
- The student Print case above: "Ea (step 2)" floats above "Activated complex 2"; "Ea (catalysed)" sits under the 100 line.
In Print every arrow except the catalysed one is the same #222 (`:569`), so position is the only cue. Sweep: 22 "Ea (step N)" and 13 "Ea (catalysed)" labels more than 40 px x size from their arrow in the random sweep; 6 and 2 in the plain grid. This goes well beyond the commit's acknowledged weak case ("Ea (step 2) under its arrow's tail"), which itself occurs ("Ea / (step 2)" with the 110 dotted line through it) and is minor by comparison.

### M3 (MAJOR) - a products name longer than the plateau is placed in mid-plot, separated from its plateau by the curve
`name_variants`/`wrap_px` `:419-463`, the products `put` at `:743-747`.
`{"reactant_text":"N2(g) + 3 H2(g) reactants in the gas phase at 298 K","product_text":"2 NH3(g) the ammonia product of the Haber process","label_size":"Large","steps":"Two steps","reactants":100,"products":20,"ea":60,"intermediate":110,"ea2":70}`: the products name is a 4-line block under the valley, ~100 px left of the products' plateau, with the steep descent between. Random sweep: 10 reactants and 8 products long-name cases misattributed, plus 4 more than 40 px away. These names are deliberately long; the realistic formula "CO2(g) + 2 H2O(l)" is fine, so MAJOR for teacher-typed descriptive names only.

### Minors
1. "activated complexs" - `:965`. `{"steps":"Two steps","reactants":100,"products":20,"ea":60,"intermediate":110,"ea2":50,"axis":"Unlabelled"}`: key says "The two activated complexs are equally high". Should be "complexes".
2. Wrong reason in the intermediate note - `:170-178`. `{"steps":"Two steps","reactants":100,"products":20,"ea":60,"intermediate":150,"ea2":70}` gives "The intermediate (150 kJ/mol) must lie below the first transition state (160 kJ/mol), so it is drawn at 145". 150 is below 160; the real rule is the hidden 25%-of-Ea gap. The drawing moves by 5 kJ/mol for a valid shallow intermediate and Ea (step 2) is computed from 145, not from the teacher's 150.
3. Catalysed Ea equal to delta H (endothermic) gives a humpless catalysed path and catalysed reverse Ea 0, with no note when typed as exactly delta H - `:213-220`. `{"reactants":40,"products":100,"ea":110,"catalyst":true,"ea_cat":20}`; there is a note for 20, none for 60. The dashed "pathway" rises and runs flat into the products, with no transition state.
4. One hump for a two-step catalysed path - `:616`, `:624`. Defensible (legend says "alternative pathway"), but it implicitly makes the catalysed mechanism one step; most texts draw as many (lower) humps. Not labelled as such; the key's "lowers Ea by overall - catalysed" compares a two-step overall Ea with a one-step one. Worth a word in the FAQ.
5. Formula wrapping splits a coefficient from its species: "CH4(g) + 2 / O2(g)" - `wrap_px` `:428-463` breaks at any space. `{"reactant_text":"CH4(g) + 2 O2(g)","product_text":"CO2(g) + 2 H2O(l)","steps":"Two steps",...}`. Break only at " + " or keep a coefficient with the next token.
6. Silent rounding to 0.01 - `:97`, `:111`. `{"reactants":0.001,"products":0.002,"ea":0.001}`: everything drawn and printed as 0, no note. `12.345` becomes 12.35 silently. Reachable by typing into the control (normalize does not snap).
7. Short arrows: 13-20 px arrows (9 px head) in 159 random cases (`MIN_ARROW` `:42`); the shaft is 4-11 px.
8. `<desc>` is constant - `:773`: says "with the activation energy and the enthalpy change marked" even with `show_ea`/`show_dh` off, and never mentions the catalyst or the two steps.
9. Unparsable values are ignored silently: `{"reactants":"abc","products":"","ea":"1e3"}` renders as the defaults, no note (`val` `:101`). Hostile-link only.
10. Print relies on the labels alone: Ea, Ea (reverse) and delta H arrows are the same #222 and only Ea (catalysed) is #5a5a5a (`:569`). In two-step + catalyst Large Print the "Ea (reverse)" label sits between the Ea (reverse) and delta H arrows. No dash or head style tells them apart, which is what makes M2 hurt in Print.

## Verified correct
- delta H, Ea, Ea (reverse), Ea (step 1/2), overall Ea, catalysed Ea, "lowers Ea by" and the Check line are arithmetically exact against the printed givens in every key render (Decimal recomputation, 1,925 key renders). Signs and brackets ("80 - (-60)") correct, minus sign U+2212, no NaN, inf or "?".
- Exothermic/endothermic/"neither" wording matches the sign of delta H, including delta H = 0 ("delta H = 0" label on the key only, no arrow).
- Read from the SVG path, plateaus and peaks sit exactly at py(level) (+-0.02 px). Bezier control points sit at their end levels (no overshoot). The catalysed path shares the exact endpoints. Bold level values equal the set of distinct levels and sit on their tick lines. Ticks linear; tick numbers suppressed where a level value sits.
- Each arrow's tail and tip lie exactly at its two levels. Ea (reverse) runs from the products to the highest peak.
- Endothermic Ea < delta H is raised to delta H with a correct note. A catalysed Ea >= Ea is set halfway with a note. "Cannot lower" is refused with a note. Ea of step 2 is raised so TS2 >= products, with a note. Huge inputs clamped by normalize to +-5000 and 10000. Ea = 0 drops the arrow with a note. Nothing panicked in 3,776 renders.
- The student copy blanks every computed value and both words, numbered and qualitative, all sizes, both styles. `<title>` "Reaction energy diagram" and `<desc>` are neutral. No hidden elements or aria leaks.
- FAQ (`faq.rs:3639-3660`) answers are correct as chemistry, except answer 4's "overall Ea" (see B1).
- Registered under Chemistry with a sensible title/blurb, skin_intro, examples, notes_for arm. Census adds exactly 2 lines for the new id and changes nothing else. The handles code inverts `py` exactly and the `set` closure no-ops on a repaired value.

## Not checked
- The live egui editor and handle dragging (read `handles` `:990-1026`; the catalysed-peak knob and the main-peak knob share an x and can overlap when the two Ea values are within a few px; not verified in the UI).
- Browser (non-resvg) SVG text metrics.

## Tests
- `cargo test --lib reactionenergy`: 19 passed, 0 failed.
- `--lib figures` on the frozen archive: 1443 passed, 0 failed, 4 ignored (the worktree's 1525 includes another session's work). `figuresui`: 82 passed, 0 failed.

## Single biggest remaining gap
Label placement in two-step diagrams, worst with the catalyst on. The placer guarantees no overprint but gives up on proximity: "Intermediate", "Ea (step 2)" and "Ea (catalysed)" end up 30-70 px away and nearer other features in 31% of plain two-step + catalyst cases. In Print, where the arrows are all the same grey, a student cannot tell which arrow is which. Prefer above-the-valley for the intermediate, place arrow names before the point labels, and treat "nearer another feature than its own" as a collision in the audit.
