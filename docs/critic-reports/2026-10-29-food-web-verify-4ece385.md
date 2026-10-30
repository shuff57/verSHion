# Verification critic on 4ece385: the food web's second fix round

A verification pass over `4ece385` ("food web verification round 1 (narrow apex widened, refusal-specific second
line)"), read-only, in its own `git archive` copy, with its own SVG parser (independent `<line>`, `<polyline>`,
`<polygon>`, `<rect>`, `<text>` geometry) and its own PNG renders looked at with the image reader. Verdict:
**PASS** on the seven claims — no blocker, no major. One **MED** was found while attacking the family: routed
links whose prey and predators are disjoint on the same level pair merge into one shared horizontal bus, so a
typed web of two pairs is drawn as all-to-all. It is **pre-existing** (round 1's routing scheme, unchanged by this
commit) and was missed by the round-1 verification; it is not one of this round's claims and does not undo them.
Three LOW copy/matcher defects are new in this commit. Census is clean and the claimed suite count is confirmed.
The MED, not the claims, is what a round 3 should fix.

## Claim table

| # | Claim | Verdict | Evidence |
|---|---|---|---|
| 1 | A box narrower than its slots is widened to `width >= n*slot_need`, `slot_need = normal_hl*0.84 + 2*lab`, `normal_hl = max(EDGE_W*lab*4, 9)`, `n = max(in,out)`; slots `(k+0.5)/n` | **FIXED** | Rendered and parsed, not taken from the module's tests. `X` with 8 prey: box 29.36 → **76.48 px** (Medium), **73.28** (Small), **82.88** (Large); the eight arrival heads are **9.56 / 9.16 / 10.36 px** apart with a **7.56 px** base, so the visible clear gap is 2.00 / 1.60 / 2.80 px. Slots are symmetric about the box centre exactly (n=8 apex xs 413.18…480.10, mean 446.64 = box centre; pair deviations 0.000) and all strictly inside `left..left+width` (outermost slot 4.78 px from the edge at Medium). n=2..8 all checked: for n=2,3 the text box is wider than `n*slot_need`, so the spacing is text-driven and larger (14.68 px at n=2, 9.79 at n=3); from n=4 the box is widened and spacing is 9.56. Guaranteed family-wide because spacing = width/n >= slot_need whenever widened, and width >= text width otherwise. Text stays centred (measured centre-off <= 0.005 px on every parsed box). The widened box does not grow the canvas on a fan-in web: the apex row (76.48) is far narrower than the prey row, so the `fanin8` canvas is unchanged at 752.12 x 252 and `star20` at 1718.5 x 252. The worst case allowed by the 24-name cap is a 23-prey apex: box **219.88 px** for a one-letter `X`, 23 heads at 9.56 px spacing, canvas 1516.9 x 176 — wide but not absurd against a 1400 px prey row. |
| 2 | Head size uniform: straight and routed both use `head()` at `normal_hl`, matching `svg::arrow`'s `hl = max(sw*4,9)`, sw = `EDGE_W*lab`; straight links are `d.line` + `head` | **FIXED** | Every `<polygon>` in every rendered web is 9.00 px long and 7.56 px at the base, straight and routed alike (measured `fanin8`, `omni`, `skip3`, `deep-skip`). `normal_hl = max(1.5*lab*4, 9) = 9` at Small, Medium and Large, exactly `svg::arrow`'s `hl`. Two precision notes, neither a defect: the food-web base is `0.84*hl = 7.56` where `svg::arrow` would draw `0.80*hl = 7.20` (5 % wider), and the head is now always upright rather than aligned to a slanted shaft. Upright is semantically right here — a predator is always one or more levels above its prey, so every arrow points up — and it is why a straight fan-in head and a routed head finally match. |
| 3 | The second error line is refusal-specific via `second_line(e)` matching cycle / `More than` / `mixes` / `no arrow` / `empty name` / default | **FIXED for the six refusal templates; three new LOWs (matcher misfire, duplicate cap line, false "letters" advice)** | Rendered all six: cycle → "until the links form a one-way web" (true); 25 names → "split the web into two pictures" (true, but see LOW-2: it is the first line's instruction repeated verbatim); mixed arrows → "until every line uses one direction" (true); no arrow → "until every line is a link" (true); empty name → "until every name has letters" (true of the empty case but false advice, LOW-3); self-eat → generic "until the web can be read" (true). The prior LOW-4 (the name cap claiming a one-way web) is gone. But the matcher is a substring test on a message that interpolates a name, so a self-eat whose *name* contains a trigger word gets a false second line (LOW-1, four cases rendered). `More than` is prefix-anchored, so a name of "More than 24 names" does not misfire there. |
| 4 | `wrap`'s broken chunk is the last prefix that fits, not the first that overflows; a single glyph wider than the line is kept whole | **FIXED** | Battery run through the module's own `wrap`: `""` → 0 lines (no empty chunk); 200 `x` → 51,51,51,47 chars, every line 371.28 px against `max` 372; 201 `x` → last line 48 chars; 300 `x` → 51-char chunks, 371.28 px; "a word one glyph over" splits 51+1 (`52 x = 378.56 > 372`). No line exceeds `max` for any glyph `text_w` measures correctly, and the loop always advances (cut >= one char) so it cannot spin. The CJK under-measure is unchanged and not worse (a 120-`樹` name now breaks on 51 chars instead of 52; both render ~1 em/char and run off the 420 canvas — the recorded leftover). |
| 5 | Long-name test asserts the wrapped width; `a_narrow_predator_does_not_merge_its_heads` and `the_second_error_line_matches_the_refusal` added; `a_valid_web_draws_no_hue` checks fill and stroke; PNG export gained `fanin-narrow` and dropped duplicate "print" outputs | **FIXED** | All present in the source and all pass. The long-name test now loops over `wrap` and asserts `text_w(line) <= max` — non-trivial for Latin (the old `x < w` was always true at x=24), though it still measures with `text_w`, so it cannot see the CJK case. The narrow-apex test is a real regression test (old geometry 3.26 px apart against a 7.56 px base would fail it). The hue test runs over both `fill="` and `stroke="`; it tests the valid default render only (the error canvas is red, by design). `food_web_pngs` has the `fanin-narrow` case and writes one colour PNG + one 66 % grey copy per case; the duplicate `style=Print` copies are gone. |
| 6 | Unused `mut` (`let mut parts`) and unused loop variable (`for (l, row)`) are gone; FAQ says an arrow glyph cannot be in a name | **FIXED** | No `foodweb.rs` warning appears anywhere in a full recompile of the lib test target (the other 23 warnings are pre-existing and in other files). Source has `let parts:` and `for row in placed.rows.iter()`. The FAQ answer now ends "An arrow (-> or ←) is always read as an arrow, so a name cannot contain one." |
| 7 | Census: only `food-web__defaults` and `food-web__start` hashes moved; full suite 1905 passed, 0 failed, 5 ignored | **CLEAN / CONFIRMED** | `git diff ecd71c8 4ece385 -- tests/fixtures/figure_census.txt` changes exactly those two lines (737309558c9fe0e2 → a1214e52b36e5243), no id added or removed. My own FNV-1a 64 recomputation of both renders matches the fixture (`a1214e52b36e5243`), and `census_baseline` passed in the suite. My full `cargo test --lib -j2` (own copy, own target dir, 8 GB scope) gives **1907 passed, 0 failed, 5 ignored** in 623 s; my copy adds two harness tests to the commit's suite, so the claimed 1905/0/5 is confirmed. No protected file is touched, no test deleted or bound loosened (`print_draws_no_hue` was renamed to `a_valid_web_draws_no_hue` and strengthened, not weakened). |

## New / remaining defects (ranked)

1. **MED — two routed links with disjoint prey and predators on the same level pair merge into one bus, so the
   picture reads all-to-all (pre-existing; not introduced by this commit; not covered by round 1).**
   Repro: `A1 -> B1 / B1 -> C1 / A1 -> C1 / A2 -> B2 / B2 -> C2 / A2 -> C2`, Medium, levels on.
   Both routed polylines have the same departure y (172) and arrival y (68): route A1→C1 is
   `(243.36,180) (243.36,172) (327.48,172) (327.48,68) (243.36,68) (243.36,57)` and route A2→C2 is
   `(298.25,180) (298.25,172) (339.48,172) (339.48,68) (298.25,68) (298.25,57)`. At y=172 and again at y=68 the
   runs overlap collinearly for **29.23 px** (x 298.25…327.48); A2's departure stub ends on A1's run (a T at
   x=298.25, y=172) and C2's riser ends on A1's run (a T at x=298.25, y=68). The six typed links therefore render
   as a connected bus between {A1,A2} and {C1,C2}: two **extra** relations (A1→C2, A2→C1) are drawn, and with six
   pairs (`A1->B1/B1->C1/A1->C1` … `A6…`) it is a single six-lane bus (runs y=172 and y=68, x 188.83…552.56 on a
   578.56 px canvas). No bundled example has more than one routed link, so the shipped presets are not affected;
   any teacher web with two omnivores is. The fix is to give each routed link its own horizontal run y (a few px
   per lane index) instead of one shared `y0`/`y1`, or to mark junctions. Not a regression: round 2 moved `y1`
   from `qy+8` to `qy+11`, which only adds 3 px of clearance over the head base.
2. **LOW (new) — `second_line` misfires on a self-eat name that contains a trigger word.** The self-eat message is
   `"Line 1: {name} cannot eat itself."`, and `second_line` does substring matching, so it returns the wrong
   sentence for a name that contains "mixes", "no arrow", "empty name" or "run in a circle" (all four rendered):
   `no arrow -> no arrow` → "Nothing is drawn until every line is a link." (the line *is* a link);
   `mixes -> mixes` → "…every line uses one direction."; `empty name -> empty name` → "…every name has letters.";
   `run in a circle -> run in a circle` → "…the links form a one-way web." The first line is correct in every case;
   only the second is wrong. Fix: dispatch on an enum/error kind, not on message text.
3. **LOW (new) — the name-cap second line is the first line repeated.** `More than 24 names (line 13): split the
   web into two pictures.` is followed by `Nothing is drawn: split the web into two pictures.` — the same
   instruction twice on a 420 x 126 canvas (rendered). The mixed-arrow line is a paraphrase of its first line too.
   The prior false sentence is gone, but the new one adds nothing; a second line such as "Nothing is drawn until
   the web fits in one picture." would not duplicate.
4. **LOW (new) — the empty-name second line gives false advice.** "Nothing is drawn until every name has letters."
   suggests every name must contain a letter, but the parser accepts numeric names (`1 -> 2` is a valid web);
   the actual rule is that no name may be blank. It is also redundant with "put a name on both sides of every
   arrow." on the first line.

Not ranked as defects, recorded for round 3 decisions:

- The head is now upright for straight links whose shaft meets it at an angle (fan-in, `omni`); this is a design
  consequence of "one head", readable as "flow to the eater", and at Small the outermost head's base sits 0.8 px
  from the box edge (flush but not crossing; the base is 9 px below the box).
- The heads do not scale with label size (9 px at Small/Medium/Large, inherited from `svg::arrow`'s absolute 9 px
  floor). At Large they read small against the 2.1 px stroke and 18.2 px text; still legible and consistent with
  every other figure, so not a new defect.
- A routed arrival run passes ~1–2 px under a straight arrival head's base and crosses its slanted shaft in `omni`
  (canvas 420 x 252, run y=68, head base y=66). Round 1 had `y1 = qy+8 = 65`, exactly at the old head base
  (`hl=8` at Medium), so round 2's `y1 = qy+11` is a small improvement, not a regression.
- CJK names still run off the 420 px error canvas (51-char `樹` lines measure 371 px in `text_w`, ~663 px on screen);
  recorded `text_w` leftover, not worse than round 1.
- The 20–24-name strip is unchanged in width by the widening in the fan-in case (the apex row is not the widest
  row); recorded leftover.

## Verified as correct (independently, from the emitted SVG)

Slot geometry: `n = 1..8` at Small/Medium/Large gives spacing >= the 7.56 px head base in every case, slots
symmetric about the box centre and strictly inside the drawn rect; straight and routed attachments never leave
their box. Route geometry: no routed segment enters any box's strict interior in any rendered case (Liang–Barsky
against every box), and every lane stays inside the canvas (1..6 long links; sixlong2's rightmost lane 552.56 <
578.56, deep-skip's 302.74 < 420). Stubs: departure 8 px above the prey row, arrival 11 px below the predator row,
inside a 47 px inter-row gap, so neither reaches the next row; every routed head's apex is on the target box bottom,
pointing up. Widened boxes still centre their text (<=0.005 px) and do not collide with level names (level text ends
10 px left of `left`; boxes start at or right of `left`). Narrow webs stay centred including the lane reserve
(default-levels-off body 110.28…283.72 with the lane at 303.72). One-link 420x176, chain4 420x328, default at
Large 504.84x440.8 all sane. `wrap` battery as in claim 4. An old saved link carrying `style=Print` loads through
`params_from_json` and renders (no panic, unknown key ignored). Valid render is hue-free (0 reddish pixels); the
error canvas is red (#c0392b, 1268 reddish pixels), luma ~96, which prints black in a 66 % bitonal copy — acceptable
for an error state, and the figure itself is monochrome.

## Not checked

The egui Figures-tab UI itself (I read `notes_for`/`params_from_json` and rendered through `Params`, but did not
screenshot the app). A real `vershion://figure/...` share URL end-to-end. PDF/DOCX export. A release-build suite
(this was debug). The 66 % grey copy of every case (I counted hue pixels and computed the red luma). Every error
path at Large (checked the six templates at Medium and a long-name/cycle at the default size). The recorded
20–24-name strip and CJK under-measure as *fixed* (confirmed still present, not worse).

## Census note

Only `food-web__defaults` and `food-web__start` moved (737309558c9fe0e2 → a1214e52b36e5243), the expected effect of
the `(k+0.5)/n` slots and the changed straight-link head; widths are unchanged for the default web (its widest
in/out count is 2, below the text width). The `census_baseline` test passed, and an independent FNV-1a 64 of both
renders reproduces `a1214e52b36e5243`. No other figure's hash moved. Full suite in my copy: **1907 passed, 0
failed, 5 ignored** (commit's 1905 + two harness tests).
