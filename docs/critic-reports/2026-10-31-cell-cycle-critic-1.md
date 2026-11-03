# Cell cycle, critic 1 (accuracy + teacher + regression), commit de1fedd

**Verdict: FAIL**: 0 blockers, 1 major, 3 MED, 4 LOW, 6 minor.

How I tested: I made my own clone (`ccrit1`) checked out at de1fedd, with its own target dir. My probes are in a separate file,
`scratchpad/critic.rs`, which I included into the module's `tests` only for the runs; the clone was restored afterwards.
- Text extents come from **resvg ink boxes** (`Text::flattened().abs_bounding_box()` with the repo's fontdb), not from the module's
  width tables. Discs, leaders, ticks, rules, rects, polygons and the ring are read back from the SVG.
- I checked that the audit can fail: a text moved onto "Cell cycle" is flagged, and a G1 label moved 60 px is flagged three ways.
- Images: 1.3x/1.5x PNGs and 66 % bitonal PNGs (the builder's 34 cases plus 17 of mine), all looked at.

## Findings

### MAJOR 1: the render panics on some typed text (the editor crashes)
- `parse_hours` lowercases the input and then slices the original string with the lowercased string's byte length
  (`s[..head.len()]`, cellcycle.rs:151). When lowercasing changes the byte length, the slice cuts through a character and panics.
- Panics I measured:
  - `g1 = "Kh"` (the Kelvin sign U+212A, then h)
  - `g1 = "ẞh"` (capital sharp s)
  - `g1 = "KKhours"` (Kelvin signs)
- `build()` and `warnings()` panic too: `crit_panics` reports "BUILD PANICKED" three times.
- No caller catches it: there is no `catch_unwind` in `src/`.
- This breaks docs/new-figure.md ("Never panic on an odd value"), and contradicts the builder's "render-never-panics fuzz" claim. The
  fuzz never generates non-ASCII letters.
- Smallest fix: strip the unit only if the input ends with it ignoring ASCII case (`s.len() >= u.len() && s[s.len()-u.len()..].eq_ignore_ascii_case(u)`
  on a char boundary). Or parse the lowercased string. Add `"Kh"` and `"ẞh"` to `hours_are_read_exactly`.

### MED 1: the advice search gives up when a one-step fix exists
- `advice_plan` is greedy. It commits to the first candidate that removes the current problem's class, never backtracks, and
  stops after 8 steps.
- How often the message ends "Nothing is drawn until this is changed":
  - 406 of 4,565 random refusals (8.9 %) in my `crit_refusals`.
  - 576 refusals in my family grid.
- Reproductions, each with a one-step fix that draws:
  - `g1=2, s=52.691, g2=207, m=0.62, pointer=G0, g0 off` (default everything else). Says "The marker is set to G0, but Show G0 is
    off ... Nothing is drawn until this is changed." **Set the marker to None** alone draws. The search instead turns G0 on, then
    loops between G1 raises (too small because the thin M is raised to 5°) and "Turn off Show G0", which brings the marker problem back.
  - `g1=1, s=1, g2=1, m=300, pointer=G0`, with g0 off (288 grid cells) or g0 on (288 grid cells). No advice. With g0 off, setting
    the marker to None draws; with g0 on, Equal wedges draws.
  - `g1=0.64, s=0.66, g2=5.835, m=582.9, g0 on, pointer G0`: no advice, but **Set Wedge sizes to Equal** draws.
- Related: the Ratio reason names only the **first** tiny phase (`break` in `solve`).
  - Example: `1000/0.5/0.5/0.5` says only "S length 0.5 h is under 0.1 %", although G2 and M are under 0.1 % as well.
  - Of the no-advice Ratio refusals, 319 have two or more tiny phases.
- The FAQ promises "the message gives the length that draws" and that G0 "tells you what to change". That is not always true.
- When advice is given, it is sound: 4,159 of 4,159 advised refusals draw when I parse the sentences and apply them myself. One
  advice chain is six steps of leap-frogging (`g1=1,s=100,g2=100,m=0.2,g0`: "Make M at least 0.202 … Make G1 at least 11.777 … Make M at least 0.212 …
  Make G1 at least 11.828 … Make M at least 0.213 … Turn off Show G0").
- Fix direction:
  - Search breadth-first over candidate sets of depth ≤ 3, keeping the shortest plan that draws, rather than the first class-removing step.
  - Always try the one-switch fallbacks ("Set the marker to None", "Set Wedge sizes to Equal") before declaring no advice.
  - List every phase under 0.1 % in the reason.
  - Add a test that `pointer=G0, g0 off` with a raised thin wedge gets advice. Mutant M13 (below) shows the "marker to None"
    candidate is never exercised.

### MED 2: phases with equal hours get unequal percents in the key
- The "earlier phase wins a tie" rule breaks ties between phases whose exact shares are identical. Their printed shares then differ.
- In my SVG read-back, 117 of 1,954 drawn random cases (6 %) print different percents for two phases with the same hours.
  - `g1=1, s=1, g2=1, m=300` (key): "1 h · 0.4 %", "1 h · 0.3 %", "1 h · 0.3 %".
  - `g1=7, s=2.0, g2=0.93, m=2`: S 2 h prints 16.7 %, M 2 h prints 16.8 %.
  - `g1=13, s=3.00, g2=384, m=3`: 0.8 vs 0.7.
- A student who rounds correctly writes the same value for the same hours. A teacher reading the key sees an apparent error.
- This is not the recorded "four exact ties can need two steps" note: the issue is equal givens getting unequal answers, and it
  is common.
- Fix direction:
  - When the furthest-rounded phases tie and have equal hours, move the 0.1 to the next phase with a different exact share.
  - If none exists (for example three equal thirds), do not force 100.0: print 99.9 or 100.1 and say why in the caption.
  - Add a test that equal hours always give equal shares.

### MED 3: the spindle checkpoint blank has the wrong shape (two defensible answers)
- The student table gives every checkpoint row, including the spindle row (`IKind::Tick(2)`), the form "___ / ___ checkpoint".
- The key prints "Spindle checkpoint" (Letters: "Spindle"), which is one word. The two-part blank steers students to
  "metaphase / anaphase checkpoint" or "M / spindle".
- A teacher has to accept answers that the key does not show.
- Reproduction: defaults with Show answers off, row 12 (row 13 with G0).
- Fix: give the spindle row one rule followed by "checkpoint". Or print "metaphase/anaphase (spindle)" in the key so it matches
  the slash form.

### LOW 1: the "too small a share to print" reason is false when no share is printed
- `need_times` is true for Proportional even with Show hours and percent off, so the 0.1 % check still runs.
- Reproduction: `g1=1000, s=8, g2=4, m=0.5, show_times off` is refused with "M length 0.5 h is under 0.1 % … too small a share to
  print". No percent would be printed, and the wedge would be drawn at 5° anyway (Equal draws fine).
- Fix: run the share check only when `show_times` is on, or reword the reason.

### LOW 2: the marker reads as a second arrowhead and is never defined
- The marker is a solid triangle about 8 px outside the cycle-direction arc (base at ri − 18, arc at ri − 26), pointing outward.
- In the start state (marker on S) and in `equal-student` (marker on G2) it looks like a second arrowhead on the direction arrow.
- Nothing in the figure or caption says "▲ marks where the cell is". The centre says only "At the marker:".
- On raised 5° wedges the triangle's base spans ±3.6° and spills into the neighbouring wedge: 63 of 480 marker renders in my
  thin families, for example thin-M Anaphase, where the base runs 353.9° to 1.1° on a 355° to 360° wedge. The tip is always in
  the right wedge, and the gap to a tick is at least 2.16 px (tripleB, Small, S).
- Fix: add one caption sentence when a marker is on. Optionally make the triangle hollow, or move it outside the ring.

### LOW 3: "1,000" is read as 1 hour, silently
- A comma is always read as a decimal comma, so a teacher typing a thousands separator gets 1 h and a different figure, with no
  message.
- Fix: refuse, or say how it was read, when a comma is followed by exactly three digits and no other separator.

### LOW 4: three of my 16 mutants survive (untested boundaries)
- M8: the G0 threshold `sp[0] < 20` changed to `< 19`. The 20° boundary is not pinned.
- M12: the Ratio check `<` changed to `<=` (a phase exactly 0.1 % of the cycle is refused). The documented "at least 0.1 %"
  boundary is not pinned.
- M13: the "Set the marker to None" candidate deleted. No test needs it (see MED 1).

### Minor
1. `g1 = "000001"` (and `"0000000.5"`) is refused as "is more than 1000 hours". That is false: the integer part is length-checked
   before leading zeros are stripped.
2. `g1=0.764, g0 on`: "G1 would be drawn only 20.0 degrees wide … (it needs 20)". The 19.96° is shown rounded, so the sentence
   contradicts itself. Truncate to one decimal or show two decimals.
3. The builder claims 29 module tests; there are 28 `#[test]`s (cargo filter: 28 passed).
4. Range answers in the key ("2C to 4C" for S, "4C to 2C" and "92 to 46" for cytokinesis) are fair, but a student who writes
   "46" at cytokinesis or "4C" at S has a defensible answer. The caption does not say a range is expected.
5. No prometaphase. Many AP texts (Campbell) teach five stages of mitosis plus cytokinesis.
6. Telophase is counted as 4n (92) in the whole cell. That is defensible and the caption states the convention, but many keys
   say 46 per nucleus. Consider a "per cell / per nucleus" note.

## Teacher verdict and the biggest gap
- The default key, the student copy and Print are clean and exam-usable. Leaders never cross, the strip is clear, the hatches
  survive 66 % grey (G2's stipple is the faintest but stays visible), and Large labels fit.
- What would stop me dropping it into an exam:
  - equal hours with unequal percents (MED 2);
  - the spindle blank (MED 3).
- **Biggest gap against a biology teacher's diagram: there is no Interphase grouping.** Nothing marks G1 + S + G2 as interphase
  as opposed to M phase. The most common cell-cycle question ("which phases make up interphase?", "label interphase") cannot be
  asked from this figure or its student copy. An outer arc labelled "Interphase" over G1 to G2 (a numbered blank in the student
  copy) would close it.

## Verified correct (measured)
- **Biology**
  - Phase order G1 → S → G2 → M, clockwise from the top.
  - G1/S tick at the end of G1, G2/M tick at the end of G2, spindle checkpoint between Metaphase and Anaphase on the strip
    (mutants M10 and M14 are killed).
- **Readout at every marker stage** (2n = 8):
  - DNA: G0 2C, G1 2C, S "2C to 4C", G2/prophase/metaphase/anaphase/telophase 4C, cytokinesis "4C to 2C".
  - Chromosomes: 8 up to metaphase, 16 at anaphase and telophase, "16 to 8" at cytokinesis.
  - All correct under the caption's stated whole-cell convention. The C definition is correct.
  - The M wedge and the strip agree: a stage marker thickens the M wedge and the strip box.
- **Percents**
  - Recomputed independently from the SVG for 1,954 drawn cases: random log-uniform hours, 64 tie constructions and 80
    thin-phase sweeps.
  - 0 mismatches against exact-rational half-up plus the written rule; every sum is exactly 100.0.
  - The total is always printed correctly; the caption sentence appears exactly when an adjustment fired.
  - 723 cases needed adjusting; 1 needed two steps.
- **Collisions**
  - 25,248 drawn renders in my family grid: 22 duration families (thin M, triple-thin in all 4 positions, adjacent thin pairs,
    equal, ties, the G0 edge, 1000 h, 0.001 h, half M) × 3 label sizes × key/student × G0 × checkpoints × strip × 5 markers ×
    names × wedge mode × show_times.
  - Plus 12,000 random drawn renders.
  - 0 collisions by my resvg ink-box audit: text/text, text/disc, disc/disc, leader/text, leader/disc, leader/tick, leader/leader
    (2.5 px), leader inside the ring, text on the ring, text/tick, text/rule, centre text vs the arrow, polygon/text, polygon/disc,
    text crossing a box edge, text off the canvas.
  - 0 Layout refusals.
- **Refusals**
  - Every refusal's note equals its canvas: 4,565 refusals, 0 mismatches, every reason present.
  - Typed text is quoted.
  - Edge inputs refused with true reasons: empty, NaN, inf, ∞, 1e3, negatives, unicode digits (١١, １１), "+-1", more than 3
    decimals, over 1000.
  - "11 Hours", "5,5 h" and a NBSP or thin space are accepted.
  - Chromosome edges: 0, 1, odd, 1002, 4294967296, "-46", "46.0" and "+46" are refused with working advice; "046", "46 ", 2 and
    1000 draw.
- **Leaks**
  - 713 student copies scanned across the whole source (text, attributes, desc, title, ids, comments).
  - No phase or stage name, no "2C/4C/4n", no share, no total, no readout.
  - "chromatid" appears only in the allowed caption. The title is always "Cell cycle".
  - The builder's `markup_leaks` and mutants M9 (names shown) and M16 (names in desc) are killed.
- **Print**: grey-only (M11 killed). All four hatches are distinct at 66 % bitonal.
- **Mutation**: 16 real mutants, 13 killed, 3 survive (LOW 4).
- **Regression**
  - The diff touches exactly the 8 listed files, and the census gains exactly the two `cell-cycle__defaults` and `__start` lines
    (338 → 340).
  - `cell-cycle` is in `NO_HANDLES`, and the module has no handles.
  - 0 compiler warnings in cellcycle.rs.
  - The start state (pointer S, DNA readout) draws.
  - The new figure touches no saved format.
  - Full `figures::` lib suite on de1fedd: **1600 passed, 0 failed, 4 ignored**. This includes `census_baseline`,
    `handle_coverage_report`, `overrides_are_valid_and_render`, `all_figures_render_with_defaults` and the FAQ tests.

## Not checked
- The live egui editor (typing, the panel, the note display) and browser font metrics.
- PDF/DOCX export and the Student + key pair export.
- Whether a panic in render aborts the whole app or only a thread. No catch exists, so I treated it as a crash.
- Hand-typed spreadsheet pastes.
- Tie behaviour beyond the sampled 1,954 cases.
