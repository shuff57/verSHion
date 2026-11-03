# Critic 2: reaction-energy-diagram (cb150cf)

Scope: `src/figures/chemistry/reactionenergy.rs`, its FAQ arm in `faq.rs`, its two census lines. Science accuracy and teacher usability in one pass. Baseline for every before/after: f4175bb (`p6-base/vershion`, and a dump tool built from `git archive f4175bb`).

PNG paths below are relative to `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/0d1ac786-d5e8-40ac-b5ff-5513b2ee7026/scratchpad/p7c-pngs/` (`X.png` = cb150cf, `X-base.png` = f4175bb). 148 PNGs rendered with the two binaries (plus zooms and side-by-side pairs). Every PNG cited here was opened and looked at.

## VERDICT: PASS (with minors)

- Blockers: **0**
- Majors: **0** (one finding, N1, is a borderline MAJOR; I rated it MINOR and say why. If you read it as MAJOR, the verdict is FAIL.)
- Minors: **9** new or remaining (N1-N9), plus 2 pre-existing ones still open (P1, P2).

The B1 chemistry is fixed and stays right at every boundary I could construct. Every printed value in 1,969 numbered key renders matches an exact Decimal recomputation from the drawn levels. Nothing leaked into 2,402 student renders. The attribution failures the first critic measured fell from 811 to 37 on my independent real-glyph audit. The price is a new, milder pattern: arrow names get parked on the arrowhead, next to the peak's name (N1).

## How I checked (independent of the builder's tests)

- Dump tool: my own crates (`p7c-dump-new`, `p7c-dump-base`) link each source tree. For every case they print the SVG, the editor notes and the **usvg real-glyph ink box** of every `<text>`. Rotated text is mapped through its `rotate(-90)`. The new dump's output is byte-identical to the binary's PNG pipeline input: the `<desc>` names the steps and catalyst, and the binary's strings contain the new desc text while the base binary's do not.
- Audit: `p7c-tools/audit.py`, written from scratch, not the builder's. It checks label-on-label (ink boxes, area > 0.5 px²), label-on-arrow (shaft and head), label-on-curve (sampled Béziers, main and catalysed), label-on-axis, text-on-text (axis, legend, readout), text off the canvas, and attribution with the stated rule (own feature > 15 px while another feature ≥ 6 px nearer, or > 40 px × size). Features are the reactants and products plateaus, each peak, the valley, the catalysed peak and each arrow, all read from the SVG geometry. Two extra checks: "parked" (an up-pointing Ea arrow's name lying wholly above its own arrowhead's tip) and the catalysed knob against the other knobs.
- Values: the drawn level of each stationary point is read from the SVG path. It is matched to the bold given at the axis through that level's 1.4-wide tick (nearest y, refused if ambiguous). Then ΔH, Ea (each step), the overall and reverse Ea as the energetic span `max(TS1−R, TS2−min(R,I))` / `max(TS2−P, TS1−min(P,I))`, the catalysed Ea and "lowers Ea by" are recomputed in `Decimal`. Each printed line is parsed, and its operands, its arithmetic, the word, the "why" sentences, the Check line (printed only if the identity holds) and overall ≥ each step are all verified. Qualitative keys are checked from pixel geometry.
- Student copy: every text, `<title>`, `<desc>` and `aria-*` in all 2,402 student renders. No digit outside "(step N)" / "complex N", and none of the answer words outside the question prompts.

Sweep: **5,454 cases per build, same cases** (seed 20261030, `p7c-cases.jsonl`):
- 864 plain classroom cases: 18 textbook level sets × catalyst × Small/Medium/Large × Colour/Print × key/student × numbered/unlabelled.
- 1,290 span-boundary cases: intermediate = R, = P, R ± 0.01, P ± 0.01, between, below both, −500, −4000, equal TS heights, ea2 clamps, catalysed Ea 10/30/70/200.
- 3,300 random cases: extreme values, all toggles, ts names, and formula/Unicode/long names.
- Plus a 3,240-case grid with the intermediate near the reactants, and repro and boundary lists.

By drawn configuration: one-step 929, one-step + catalyst 936, two-step 1,498, two-step + catalyst 2,091.

## Counts (same 5,454 cases, f4175bb -> cb150cf)

| check | one | one+cat | two | two+cat |
|---|---|---|---|---|
| Attribution failures (stated rule, real glyphs) | 10 -> **0** | 14 -> **0** | 75 -> **3** | 712 -> **34** |
| Arrow name parked above its own arrowhead | 0 -> 0 | 1 -> 2 | 43 -> **74** | 319 -> **766** |
| Label ink on label ink (> 0.5 px²) | 0 -> 0 | 0 -> 0 | 0 -> 29 | 58 -> 26 |
| Label on arrow / curve / axis, text on text, off canvas | 0 | 0 | 0 | 0 |
| Wrong value, operand or arithmetic (Decimal) | 0 | 0 | 307 -> **0** | 503 -> **0** |
| Overall Ea < a step Ea | 0 | 0 | 185 -> **0** | 356 -> **0** |
| Check line printed but false | 0 | 0 | 296 -> **0** | 494 -> **0** |
| "Lowers Ea by" wrong | 0 | 0 | 0 | 311 -> **0** |
| Catalysed peak not strictly between max(R,P) and the top peak | 0 | 32 -> 0 | 0 | 56 -> 0 |
| Qualitative key statement wrong | 0 | 0 | 134 -> 0 | 134 -> 0 |
| Student-copy leaks (text, title, desc, aria) | 0 | 0 | 0 | 0 |

- Attribution overall: **811 -> 37 of 5,454.** 794 cases fixed and 20 regressions (0 plain, 4 span-boundary, 16 random). Plain grid: 152 -> 4, with 0 regressions.
- The builder claimed 176 -> 5 of 3,200 and 1,045 -> 43 of 3,000 (two-step + catalyst) on a real-glyph audit. My independent rule gives 712 -> 34 of 2,091 for two-step + catalyst (1.6 %), the same order. **Verified in substance.**
- "Overprint 0 in both builds": **refuted** in both builds (N3). It is narrow, about 1 px, and the same count in each build (58 -> 55).
- "Off-canvas 0": **verified** (0 in both builds, real glyphs, legend and readout included).
- Parked-arrowhead labels: **363 -> 842 cases** (608 new-only). Plain two-step pages with "Ea (step N)" parked beside the peak's own name: **24 -> 96 of 480** (N1).

## Before/after: the first critic's findings

| # | repro | f4175bb | cb150cf | status |
|---|---|---|---|---|
| B1 | `{"steps":"Two steps","reactants":100,"products":80,"ea":20,"intermediate":0,"ea2":110}` | Overall Ea 20 next to Ea (step 2) 110; reverse 40; false Check | Overall 110 (I->TS2), reverse 120 (I->TS1), why-sentences, no Check, reverse arrow on the valley climb | **Fixed** (`B1.png`, `B1-base.png`) |
| M1 | M1a-M1f (exact JSON as in critic 1) | "Intermediate" 22-51 px off, under the dashed curve or the 100 line | "Intermediate" by its valley in all six | **Fixed.** Residue: at Large Print, "Ea (step 1)" and "Intermediate" are still staggered side by side (`M1c.png`), and the new "above the valley" place can lift it 57 px into the well (N4) |
| M2 | M2a, r=p=150 Large (M2b/M2c), student Print M1d | Ea (step 2) 50 px from its arrow, under the products; Ea (catalysed) under the reactants | Every Ea name within 15 px of its arrow | **Fixed for distance.** But "Ea (step N)" now often sits on its arrowhead beside "Activated complex N" (N1: `M1a.png`, `M2a.png`, `M2c.png`, `M1c.png`) |
| M3 | Haber long names, Large two-step | products name 100 px left, under the valley | 5-line block over its plateau, between the Ea (reverse) and ΔH arrows | **Fixed** (`M3.png`) |
| m1 | "complexs" | wrong | "activated complexes" | Fixed |
| m2 | intermediate 150, TS1 160 | false reason, drawn 145 | kept at 150, no note | Fixed |
| m3 | catalysed Ea = 20 and = 60 (= ΔH), endothermic | humpless path, no note for 60 | both drawn at 85 with a true note | Fixed |
| m4 | one hump for two-step catalysis | silent | legend "drawn as one hump", FAQ says so | Fixed |
| m5 | `CH4(g) + 2 O2(g)` | "CH4(g) + 2 / O2(g)" | "CH4(g) + / 2 O2(g)" | Fixed (new adjacency N6) |
| m6 | 0.001 / 12.345 | silent | one rounding note naming each value, equal levels and Ea 0 | Fixed |
| m8 | constant desc | constant | ASCII, names the steps, catalyst and the drawn arrows, no values | Fixed |
| m10 | Print arrows all #222 | same stroke | Ea solid 1.6, Ea (reverse) "6 3", ΔH 2.4, Ea (catalysed) #5a5a5a "2 3" (counted over 2,070 Print renders, 0 chroma) | Fixed |

## Findings

### N1 (MINOR, borderline MAJOR): arrow names parked on the arrowhead, beside the peak's name, four times as often as in f4175bb
`reactionenergy.rs:447-452` (`misattributed` cannot see it: the arrow's tip *is* the peak point, so the peak is never 4 px nearer), `:484-498` (`score`: within 15 px of the arrow segment, including its tip, is tier 0), the close `ring` around the arrow's midpoint `:601-612` used at `:1101`, and arrow names placed first `:991-1027`.

The fix for M2 trades "far from its arrow" for "on top of its arrowhead". There "Ea (step 2)" reads as a second caption for the peak: "Ea (step 2)  Activated complex 2" on one baseline, at the top of the canvas.
- `{"steps":"Two steps","reactants":100,"products":20,"ea":60,"intermediate":110,"ea2":70,"style":"Print"}` (the first critic's M1a without the catalyst; same with it): `M1a.png` vs `M1a-base.png`. f4175bb had "Ea / (step 2)" beside its arrow's foot.
- The builder's own example "Two-step reaction with an intermediate" at Large, student Print: `{"steps":"Two steps","reactants":100,"products":20,"ea":60,"intermediate":110,"ea2":70,"style":"Print","label_size":"Large","show_answers":false}` gives `ex4P_Ls.png`. Colour Large: `park400.png` vs `park400-base.png`.
- `M2c.png` (r = p = 150, Large Print): "Ea (step 1)  Activated / complex 1" at the top. This is the commit's "known limit at Small", but here it is at Large.
- `b1catP.png` (`{"steps":"Two steps","reactants":100,"products":80,"ea":20,"intermediate":0,"ea2":110,"catalyst":true,"ea_cat":15,"style":"Print","show_answers":false}`): "Ea (catalysed)" sits over activated complex 1's peak, 3.6 px from the Ea (step 1) arrowhead and 11 px from its own short dotted arrow. It passes the 15 px rule.

Counts: plain two-step pages with "Ea (step N)" parked beside "Activated complex N": **24 -> 96 of 480**. All parked arrow names: 363 -> 842 of 5,454.

Why MINOR: the label touches its own arrowhead (within about 5-15 px), and in Colour it shares the arrow's colour. It is not clearly the wrong feature. But in Print the arrowhead and the peak are the same point, so position cannot separate them. A stricter reader could call this M2 not fixed. Fix: count a label as "beside a peak label" or "past the head" as tier 2, or prefer the beside_arrow places at fractions 0.35-0.65 before the ring.

### N2 (MINOR): "ΔH = 0" floats away from any level (key only)
`:1084-1087`. The job has `own: None`, so the attribution ranking never penalises it and the re-run never promotes it. Example: `{"reactants":150,"products":150,"ea":100,"label_size":"Large","steps":"Two steps","catalyst":true,"style":"Print"}`. It sits at the right edge about 90 px above the shared level, level with "Ea (reverse)" (`M2c.png`; f4175bb put it beside "Products", `M2c-base.png`). Also `M2a.png`. More than 30 px off its level: plain 0 -> 6 of 72, random 2 -> 6.

### N3 (MINOR): overprint "Ea (step 1)Intermediate": the claim "overprint 0" is false in both builds
Placer `:485-491` (2 px pad on the `tw` estimate). The ink boxes overlap by 1.2-2.5 px², and the white halo of "Intermediate" eats the closing bracket.
- `{"ea":20,"ea2":30,"intermediate":100.01,"products":60,"reactants":100,"steps":"Two steps"}` gives `ov1224.png` and zoom `zoom-ov1224.png`: "Ea (step 1" + "Intermediate".
- `{"axis":"Numbered (kJ/mol)","ea":145.0,"ea2":120,"intermediate":-195,"label_size":"Medium","product_text":"C","products":-1000,"reactant_text":"A + B","reactants":-199.2,"show_answers":false,"steps":"Two steps","style":"Colour"}` gives `zoom-ov4375.png`.

55 cases in cb150cf and 58 in f4175bb, all but one with the intermediate within 0.01 kJ/mol of the reactants. A 3,240-case grid of intermediates 0.5-10 kJ/mol from the reactants found **0**. So the inputs are extreme and this is no regression.

### N4 (MINOR): the new "above the valley" places can lift "Intermediate" 57 px into the well, onto the second peak's flank
`:1053-1058`. `{"axis":"Unlabelled","catalyst":true,"ea":40,"ea2":30,"ea_cat":18,"intermediate":120,"label_size":"Large","products":60,"reactants":100,"steps":"Two steps","style":"Print"}` gives `att861.png` vs `att861-base.png`. It is plain but rare: 4 of 864 plain cases (f4175bb failed the same 4, differently). Of the 20 attribution regressions, the 4 span cases are `att1876.png`: "Activated complex 1" 41 px up-left of its peak, beyond the dashed curve, when a repaired intermediate puts the catalysed peak above TS1. JSON: `{"axis":"Numbered (kJ/mol)","catalyst":true,"ea":20,"ea2":60,"ea_cat":25,"intermediate":100,"products":100,"reactants":60,"show_answers":true,"steps":"Two steps"}`.

### N5 (MINOR): "why" sentences that explain nothing, and a Check line dropped while still true
`readout` `:1317`, `:1327`, `:1334`, `:1336`. The sentence prints whenever the intermediate is below an end, even when the textbook climb wins. Example: "The intermediate lies below the products, so the largest climb in reverse is from the products to activated complex 2." This appears in `att1876.png`; also `{"steps":"Two steps","reactants":100,"products":60,"ea":60,"intermediate":60,"ea2":60}` ("...below the reactants, so the largest climb is from the reactants to activated complex 1"). Each statement is true, but the "so" has nothing to follow from. 375 such numbered sentences, 123 qualitative. In 154 key renders the identity Ea (reverse) = Ea − ΔH still holds, yet the Check line is omitted. Never printed when false: 0 of 1,878.

### N6 (MINOR): formula wrap plus foot-of-arrow name reads "CH4(g) + Ea (step 1)"
`wrap_name` `:665-695` keeps the "+" at the line end, and the Ea (step 1) name now sits at its arrow's foot on the same baseline. `{"reactant_text":"CH4(g) + 2 O2(g)","product_text":"CO2(g) + 2 H2O(l)","steps":"Two steps","reactants":100,"products":20,"ea":60,"intermediate":110,"ea2":70,"label_size":"Large","style":"Print","show_answers":false}` gives `zoom-m5LP.png` (`m5L.png` in Colour). The second line "2 O2(g)" keeps the block legible, so this is cosmetic.

### N7 (MINOR): the Ea (catalysed) arrowhead lands on the uncatalysed curve, and the solid curve crosses the dashed hump's flat top
`:1140` (xcat 0.53/0.57 puts the catalysed peak beside the step-2 flank) and `:909`. `M1a.png` (zoom `zoom-M1a-catpeak.png`) and `longL.png`: the solid line crosses the dashed top at 130, and the arrowhead sits on it. Catalysed tip within 2.5 px of the solid curve: plain two-step 12 -> 20 of 240. The peak point itself within 3 px fell 52 -> 0, and the same crossing existed at x = 0.40 in f4175bb, so this is not a net regression.

### N8 (MINOR): the axis can gain a needless negative step
`:836-849` (`low_named` lowers y0 by up to 3 steps). One-step with all levels ≥ 0 now gets an axis running to −50 or −500, for example `{"axis":"Numbered (kJ/mol)","ea":200,"label_size":"Large","level_values":false,"products":20,"reactants":20,"show_answers":false,"style":"Print"}`. 39 random cases, 0 plain. Canvas height grows by up to 189 px in two-step key copies from the extra sentences and the wrapped legend; no text is clipped.

### N9 (MINOR): neighbouring labels that read as one phrase
`M1c.png`: the first critic's M1 phrase ("Ea (step 1) Intermediate") survives at Large Print as two labels 17 px apart in height and 10 px apart across. Each is by its own feature. The same happens with "Intermediate Ea (reverse)" on one baseline whenever the reverse arrow stands in the valley: `B1.png`, `tsEq.png`, `b1catP.png`.

### Pre-existing, still open
- **P1 (MINOR)**: catalysed-peak knob on the peak knob. Both are 14 × 14 hit boxes (`interact.rs` half 7) at `:1434` and `:1446`. Overlap in 121 of 751 one-step + catalyst renders in both builds, plus 3 new two-step cases with xcat 0.32 beside TS1. The intermediate knob never overlaps (the catalysed peak is always above both ends). Which knob wins a drag is not checked: I could not drive the editor.
- **P2 (MINOR)**: the commit's listed limits (short arrows, unparsable values, "Intermediate" about 30 px under its valley at Small) were not re-reported.

## Verified correct
- **Span rule** (`climb_fwd`/`climb_rev` `:179-196`) equals the energetic span: max over each TS of TS minus the lowest earlier state. It is continuous at intermediate = R and = P (no jump: deep_* switch at equality, where both definitions agree). Ties pick the reactants/products climb, with the same value. It holds for very deep intermediates (−4000), equal TS heights (`tsEq.png`: forward = reverse = 110, qualitative "equals"), an intermediate between the ends (`between.png`: 60, the energetic span, although TS2 < TS1) and ea2 clamps. The overall Ea was never below a step Ea in 2,091 + 1,498 two-step cases.
- **Catalysed comparison** uses the span overall: B1 + catalyst prints "lowers Ea by 110 − 10 = 100" (`b1cat.png`). The value was positive in every case, and the catalysed peak was strictly between max(R, P) and the top peak in all 1,002 numbered key renders with a catalysed path.
- **Reverse arrow** stands on the climb the working names: on the valley when the climb starts at the intermediate (`B1.png`, `b1catP.png`), otherwise on the products' plateau with a level line to the right peak.
- **Values**: 1,969 numbered key renders fully recomputed in Decimal: 0 mismatches in operands, arithmetic, words, why-sentences, Check or "lowers". Qualitative keys (764) are consistent with the drawn geometry.
- **Student copy**: 0 leaks in 2,402 renders (text, `<title>`, `<desc>`, aria, labels). "ΔH = 0" appears on the key only. The desc is ASCII in every render and contains no digits outside "(step N)".
- **No overprint against arrows, curves (even within 1 px of a stroke: 1 case in 5,454), axes, axis text, legend or readout. No off-canvas text** (real glyphs, both builds).
- **One-step pages** are not worse: attribution 24 -> 0. Pages with any label > 3 px further off: 38, almost all long names moving 8 -> 12.6 px (one "Ea (catalysed)" 6.7 -> 13.9 px), none in the plain grid. The textbook set (default, endothermic, catalyst on, equal levels, sizes, Print, student, qualitative: `pair-o_*.png`) is visually unchanged apart from the axis.
- **Legend**: it wraps to a second row and never touches the curve or the readout.
- **Print**: grey-only in 2,070 Print renders, with four distinct arrow strokes. ea1/ea2 share Ea's solid stroke, as designed.
- **Determinism**: 600 SVGs plus glyph boxes byte-identical over two runs, and identical to the sweep. Three PNGs byte-identical on re-render.
- **Census**: the commit changes exactly the two `reaction-energy-diagram__{defaults,start}` lines to `ddda473929152aaf`, and `cargo test --lib census_baseline` passes on the frozen source. (The f4175bb -> cb150cf range also moves action-potential's lines, from an intermediate commit, not cb150cf.)
- **FAQ** (`faq.rs`, the REACTION_ENERGY_DIAGRAM arm) is correct chemistry and follows the span rule:
  - "same amount" for forward and reverse lowering holds for a one-hump catalysed path when the intermediate is not deep, and is correctly excepted otherwise.
  - Answer 4 matches `climb_fwd`/`climb_rev`, and the arrow-style answer matches the strokes.
- **Notes**: intermediate rule, catalysed ≤ ΔH, rounding and the deep catalyst bound are all true as worded (m2, m3, m6, `b1cat` notes).

## Not checked
- The live egui editor and handle dragging: I cannot drive it. Handles were read in code and overlap-checked from geometry only (P1).
- Browser (non-resvg) text metrics.
- Exhaustive Large-size placement in every random case: I looked at about 60 of the 148 PNGs, not all 5,454 cases.

## Tests
- `cargo test -j2 --lib reactionenergy` on `p7-src`: **31 passed, 0 failed, 0 ignored** (183 s).
- `cargo test --lib census_baseline`: 1 passed.
- `cargo test -j2 --lib figures` on the frozen archive: **1544 passed, 0 failed, 4 ignored** (522 s). This matches the commit's claim. The `figures` filter also matched `figuresui::tests::*`, which ran and passed inside that count; `figuresui` was not run on its own.

## Single biggest remaining gap
Placement scores nearness to the arrow *segment*, and an Ea arrow's tip is the peak point. So the ranker cannot tell "beside the shaft" from "on the peak, next to the peak's own name". It now prefers the latter in 20 % of plain two-step pages (96/480, against 24 in f4175bb). In Print, where position is the only cue, that is the one place left where a student can read an Ea name as a peak name.
