# Cell cycle, critic 4 (verification), commit ad36770

**Verdict: PASS**: 0 blockers, 0 major, 0 MED, 0 LOW, 5 minor.

## How I tested
- I worked in my own clone `ccrit4` at ad36770 with its own target. Mutants ran in a second clone, `ccrit4m`. Nothing was
  committed or pushed, and nothing was run in the main repo.
- My probes are in `scratchpad/critic4.rs`, added as a child module of the module's `tests` in my clone only.
  - The collision audit, the arc audit, the advice parser, the family grid and the finer label search are reused from
    critic 3's probe file. Critic 3 proved those audits can fail.
  - The percent checks, the cache checks, the end-mark clearance, the reason check, the raster crops and the mutants are
    new and mine.
- Text boxes are resvg ink boxes. Shares, captions, end marks, ticks, leaders and the band are read back from the
  rendered SVG, not from module constants.
- A share counts as correct only if the printed value t meets (2t−1)·T ≤ 2000·h < (2t+1)·T in exact integers. This
  checks the value; it does not recompute it with the module's formula.

## Findings

### Minor 1: the arc-reason check still treats the end marks as obstacles
- Where: `solve_with`, the `Some(ring) else` branch. It decides whether the arc is the reason by calling
  `arrange(&g, &without, items)`, but `g.caps` still holds the two end marks. A figure drawn without the arc has no end
  marks.
- Effect in principle: a figure whose layout fails only because of the arc could get the generic reason ("The labels,
  checkpoints and leader lines cannot be placed…") and not the arc reason. The advice would still be right, because it is
  checked against a real solve.
- Measured with my own rebuild of the item list (`without_arc_both`):
  - Two random arc-on batches, 60,000 and 80,000 states, many forced to Large with a long G1 and thin S/G2.
  - In 26 states the end marks alone flip the "without the arc" layout from fits to fails.
  - None of the 26 is a layout refusal that turning the arc off would cure: 3 draw anyway, and 23 are refused for
    another problem (the 0.1 % rule), which they keep with the arc off.
  - c4_reasons (30,000 random, 84 layout refusals) and the 27,744-case grid (5,344 refusals): 0 reason mismatches.
- So this is latent: I could not make it print a wrong reason.
- Fix: `let bare = Geo { caps: Vec::new(), ..g.clone() };` and pass `&bare` to that one `arrange` call.

### Minor 2: a surviving mutant shows the tick exemption is untested beyond the G2 mark
- Mutant `caps_exempt_all_ticks` exempts every tick leader from both end marks. It survives the whole module suite.
- The real code (only the G2/M tick's leader, only from the G2 mark) is right. In my sweeps the nearest any leader comes
  to either mark, outside that one exemption, is 3.06 px.
- But nothing pins this. When M is thin, the G2/M tick's leader starts a few degrees left of the top. If it flipped sides
  it could slant across the start mark at 0.4°, the way M's leader did in critic 3's LOW 1 repro A. I did not find such
  a figure under the mutant; I did not sweep the mutant.
- Fix: in `no_leader_crosses_or_grazes_an_end_mark_of_the_arc`, add an `arrange` case with a `Tick(1)` item at the G1
  start mark and one with a `Tick(0)` item at the G2 mark, both expected to be refused.

### Minor 3: the student-copy word allowlist still holds the deleted rule's words
- Where: the module's `ALLOWED` list (line ~2134). It still contains "change", "furthest", "wrong", "way", "earlier",
  "tie", "repeat", "until", "whose", "another", "stays", "leave", "short", "over" and others from the dropped rule.
- Effect: `a_student_copy_prints_no_answer_anywhere` would let the old sentence back in.
  `the_caption_gives_the_sum…` bans some of those words, but only on ~500 integer cycles.
- Fix: take the dropped rule's words out of `ALLOWED`, keeping only words the current captions use.

### Minor 4: nothing tells a student the shares may not add to 100.0
- The student copy says only "Round each percent half up to 0.1."
- The FAQ example 2/4/4/1 prints 18.2, 36.4, 36.4, 9.1. A student checking that the four add to 100 gets 100.1 and may
  "fix" one, then be marked wrong against the key.
- How often (my samples): 2,605 of 11,936 drawn keys miss 100.0 (1,350 at 100.1, 27 at 100.2, 1,228 at 99.9). In the
  828,149-set direct sample, 20.5 % miss it.
- This follows the owner's decision. It is a wording gap, not a computation error.
- Fix: "Round each percent half up to 0.1 (the four need not add to exactly 100.0)." This gives nothing away.

### Minor 5: advice turns the arc off when a fix of the same length would keep it (documented order, but worth a look)
- In 138 of 6,576 refusals the advice turns off Show interphase although a plan of the same length keeps the arc:
  - 109 with one step, 29 with two;
  - the arc-keeping plan uses a fix later in the stated order: in the repro, checkpoints off or Equal wedges, the only
    later switches left.
- Repro: `g1=1 s=1 g2=0.5 m=300`, Letters, show_times off, marker Telophase, student, Large. The advice is "Turn off Show
  interphase."
- This is the documented order (marker, arc, Label style, checkpoints, …), so it is not a defect.
- None of these advices is longer than the shortest plan (see below).
- Whether losing the arc beats losing the two checkpoint ticks is a teaching call for the owner.

## Round-4 claims verified (my own measurement)

**Percents (item 1)**
- Exact half up, direct:
  - 828,149 hour sets, all 4-tuples of 1–20 h and 400,000 random thousandths with forced ties;
  - 0 values not half up, 0 equal-hours-unequal.
  - Sums seen: 100.2 (212), 100.1 (87,102), 100.0 (658,577), 99.9 (82,258). 99.8 cannot happen and never did.
- Read back from rendered SVGs: 18,216 cases, 11,936 drawn.
  - Cases: tie families [a,a,b,c], [a,a,a,b], [a,a,b,b] and all-equal with rotations and swaps; tiny (0.001–0.09 h),
    huge (900–999.999 h) and three-decimal hours; decimal commas and "h"; Equal and Proportional; G0; Letters; Print;
    all three sizes; arc on and off.
  - Also: 2/4/4/1, 2/2/2/1, 1/2/3/3, 1/1/1/13, 13/1/1/1, 1/1/199/199, 1/7/4/4, 0.06/0.66/0.06/0.82, all-0.001,
    all-1000, and 5,5 / 5.5 / 5.500 / 5 h (equal: 25.0 each).
  - Every printed share is the exact half-up value. Equal hours always print equal percents. Each percent has one
    decimal.
  - The key's sentence "Percents are rounded half up to 0.1, so they add to X." is present exactly when the printed sum
    misses 100.0. X equals my own format of the sum read back: 100.1, 100.2 or 99.9, never a float artefact. It appears
    once, and no other "add to" sentence exists.
  - The student copy:
    - always has "Round each percent half up to 0.1." while times are shown;
    - never has a share, the sum or a "Percents are rounded" sentence;
    - the only 99.9/99.8 strings in its text are typed hours, such as "999.925 h".
  - With Show hours and percent off, key and student copies have no percent and no rounding sentence.
- Word scan over the whole SVG source of both copies:
  - "adjust", "change", "furthest", "nudg", "rather than", "exactly 100", "wrong way", "equal percent", "made to add",
    "forced" and "corrected": 0 hits in 23,872 sources.
  - "changes" appears only in the DNA/chromosome range sentence, which is biology, when a marker readout is shown.
- FAQ answer: true. It names 2/4/4/1 → 100.1, says nothing is adjusted, and says the key speaks only when the sum misses.

**Earlier fixes still hold (item 2)**
- Fuzz: 5,000 renders with 0–5 awkward pieces in 1–5 random fields, every field including style, palette, show_times,
  interphase and answers. Pieces include Kelvin, ẞ, İ, ß, ﬀ, ǅ, combining marks, full-width, Arabic and math digits, NUL,
  BOM, ZWSP, U+2028, U+10FFFF, RLO, emoji, quotes, < and &.
  - 0 panics through build, warnings, notes_for, read and `def().svg`;
  - every SVG parses.
- Comma rule:
  - only 1,000 is refused ("1,000", "+1,000", "1,000 h");
  - 0,125, 5,500, 12,345, 2,375, 01,000 and 1000,000 are decimals;
  - "1,0000" and "0.0005" are refused (more than 3 decimals); "1000.001" is refused (over 1000).
- Advice: 6,576 refusals over junk, non-ASCII and comma hours, chromosome junk, stages-off markers, G0 thin, ratio and
  Large+G0+arc.
  - 0 with no advice;
  - 0 whose advice, applied by my own parser, fails to draw;
  - 0 advised figures that draw with collisions;
  - 0 multi-step plans where one of 9 single switches draws;
  - 0 plans longer than a full-solve BFS of the module's moves to depth 2;
  - 0 that turn the arc off when a shorter plan keeps it.
  - Plan lengths: 4,561 one step, 1,826 two, 189 three.
- Arc reasons: 84 of 84 random layout refusals and the whole grid name the arc exactly when the figure without it draws.
  Every arc refusal's advice draws.
- Spindle: the key prints "Spindle (M) checkpoint"; the student blank is one rule, then "checkpoint".
- Interphase biology:
  - the arc spans G1+S+G2 and stops at the G2/M boundary;
  - M has no arc;
  - G0 is a box off G1, outside the ring;
  - S with a chromosome readout shows 2n (46), which is correct.
- Collisions:
  - Grid: 27,744 cases (34 hour families × 3 sizes × key/student × G0 × checkpoints × 10 marker positions × names ×
    wedges × times, Colour/Print mixed in).
    - 22,400 drawn: 0 failures in the collision audit, 0 in the arc audit.
    - 5,344 refused: 0 "Nothing is drawn", 0 reason mismatches.
  - Random: 9,000 drawn (3,000 forced Large with the arc), 7,530 with the arc: 0 failures.
  - End marks: the nearest any leader comes to either mark (only the G2/M tick's own leader is exempt, and only from the
    G2 mark) is 3.06 px on the grid and 3.11 px random. Both of critic 3's repros are clean.
- Leaks: 1,460 student copies, whole `def().svg()` body.
  - 0 phase names, "interphase", "spindle", "(m)", shares, sums, totals, "rounded up/down", "rather than", "add to",
    "furthest" or "adjust".
  - "chromatid" appears only in the allowed readout caption. `id="` matches only `vs-hatch`. "100.0" appears only as
    `width="100.00"`.

**The G2/M tick over the G2 end mark (item 3)**
- With checkpoints on, the G2 mark sits 1.12 px (Medium) or 1.38 px (Large) from the tick's centre line, under its
  2.4 px ink and 5.5 px white halo.
- 4× raster, outer half of the mark: Medium 15/19 dark, Small 13/19, thin M 9/19, Large 0/18. The dark pixels are the
  tick's own ink.
- In the crops (`ccrit4-png/cap-*-cap1.png`) the band clearly stops at the tick, with a ~1.5 px white gap from the halo.
  The tick runs out past the band and there is no band beyond it.
- With checkpoints off the mark is 19/19 dark, a clean T (`cap-nock-cap1.png`).
- The FAQ sentence ("at the end of G2 the checkpoint tick does the same job") is true in the pixels.
- Not a defect: a teacher reads the tick as the arc's end.

**Cache (item 4)**
- The cache is `thread_local`, one entry, keyed by `Debug` of the whole `BTreeMap` (injective for these values).
- One thread, 2,099 steps. Each base state was flipped by one of 24 settings (answers, style, size, marker, readout, 2n,
  names, arc, checkpoints, G0, strip, wedges, times, an M typed as "1", "1.0" and "1 h", junk G1, palette), flipped back
  and repeated, then compared with a fresh thread.
  - Canvas, `def().svg`, warnings and notes_for: 0 stale.
- Two threads running the sequence forward and backward at once: 0 stale.
- Unnormalised params (the `interphase` key missing): same output as a fresh thread.
- Key mutants `cache_key_hours_only`, `cache_key_no_answers` and `cache_key_no_size` are all killed by existing tests.
- No stale-cache path found.

**PNGs looked at (item 5)**
- Cases: default; student; key and student at Large Print with G0 (with the 66 % threshold copies); Large; thin M; G0
  student; arc off; 2/4/4/1 key and student; 1/2/3/3; 1/1/1/13; Equal with times; all 9 marker positions; the arc
  refusal; the generic refusal; both end-mark repros; and the cap crops.
- All are clean and exam-usable:
  - 2/4/4/1 reads "…so they add to 100.1.";
  - 1/2/3/3 reads "…99.9.";
  - the student copies carry one blank per answer and the one-sentence rule.
- Biggest remaining gap for a teacher: Minor 4. The student is told how to round but not that the four may not total
  100.0. That is exactly the case the owner decision creates, and the key explains it only to the teacher.
- Second gap: on cycles with a huge M and thin interphase (end-mark-b), the Interphase number points at one spot among
  three thin wedges rather than spanning the arc. It is legible, as critic 3 also noted.

## Mutation (23 real mutants of mine on the round-4 code; 22 killed, 1 survives)
- Killed:
  - shares: `shares_half_down`, `shares_truncate`, `shares_nudge_back` (a sum-fixing step put back);
  - caption: `caption_always`, `caption_never`, `caption_only_over`, `caption_wrong_sum`;
  - student rule: `student_rule_off`, `student_rule_leaks_sum`;
  - end marks: `caps_not_obstacles`, `caps_clear_1px` (heavy: the collision family test), `caps_exempt_none`,
    `caps_only_g2`, `arc_trim_zero`;
  - advice: `shorter_off`, `shorter_quick`, `fallback_order_swap`;
  - cache: `cache_ok_not_stored`, `cache_key_hours_only`, `cache_key_no_answers`, `cache_key_no_size`;
  - label places: `grid_least_room_first`.
- Survived: `caps_exempt_all_ticks` (Minor 2).
- The builder's survivors (`advice_shorter_same_depth` equivalent, `arc_reason_always` unreachable, `max_steps_6`
  unobservable) agree with what I saw: 0 of 6,576 refusals needed more than 3 steps, and 0 layout refusals took the
  generic reason with the arc on.

## Performance (not a finding)
- Debug, single thread, unloaded: the slowest refusals I found take about 600 ms (notes plus canvas). Examples:
  `g1=65 s=0.35 g2=0.10 m=0.75`, Letters, strip off, marker Cytokinesis, Large; and
  `g1=29.03 s=0.06 g2=0.479 m=0.10`, Print, strip off, marker Metaphase, Large.
- Almost all of that is the round-2 one-step full-layout check, not the new depth k−1 search. The quick search takes
  about 50 ms and finds a 2-step plan; the full check finds the 1-step one.
- Under load in debug: 1,720 refusals, median 0.5 ms, p99 124 ms, worst 1.48 s.
- I did not measure release. Critic 3 measured about 5× faster in release, which would put these near 120 ms.

## Regression (item 7)
- ad36770 touches exactly cellcycle.rs, faq.rs and figures-progress.md. No census line changed.
- `census_baseline` passes, so the default and opening-state renders hash the same as the blessed census.
- 0 compiler warnings in cellcycle.rs.
- Module suite at ad36770: 54 passed, 0 failed.
- `cargo test --lib -j2 figures::` (my probes skipped): **1685 passed, 1 failed, 4 ignored** (2,606 s). The one failure is the known flake `absurd_windows_cannot_run_away` ("took 11723 ms" under load). Rerun alone, it passes in 1.2 s.

## Not checked
- The live egui editor and real typing latency. Release-build timing.
- Browser font metrics. PDF/DOCX export.
- A byte-for-byte diff of the example renders against the parent commit. I relied on the census for defaults and the
  opening state only.
- Whether students really "correct" a 100.1 sum (Minor 4 argues from the wording, not from an observation).
- Whether the latent reason defect in Minor 1 is reachable at all. I found no input that reaches it.
