# Verification critic on e76d968: arrivals carry the head margin, a crowded web is refused

A read-only verification pass over `e76d968` ("figures phase 4: food web verification round 4 (arrivals carry the
head margin, a crowded web is refused)"), run on its own `git archive` copy with its own target dir, its own SVG
parser (rects, lines, polylines, polygons, text) and its own PNG renders at 1.3x, which I looked at with the image
reader. The scope is this commit's diff, `68a5ed7..e76d968`. The parent `68a5ed7` is another session's commit
(chisquarefit/cladogram/enzyme/hardyweinberg), and none of its changes are counted here.

**Verdict: FAIL as a complete change (0 blocker, 0 major, 1 MED, 1 LOW, 4 minor).** All five claims hold.
The packing formula is implemented exactly, arrivals sit above departures in every gap, nothing crosses a box or
touches an arrowhead, and I rendered the refusal boundaries myself: they are exactly 10 (Small), 12 (Medium) and 15
(Large). The `refusal()` refactor gives byte-identical output for 42 parse-refusal cases. The census and the suite
are clean. The one MED is new: **the density refusal never reaches the editor.** `warnings()` and so
`notes_for("food-web", p)` return `[]` for a refused dense web, while every parse refusal does reach the note area.
The fix is small, because `warnings()` only has to run the same gap test.

## Claim table

| # | Claim | Verdict | Evidence |
|---|---|---|---|
| 1 | `clear = 76lab - box_h`, `arrival_drop = max(normal_hl+2, 6lab)`, `departure_drop = 4lab`, `height = max(clear - both, 0)`; arrivals sorted first and take the top slots; `y(s) = top + (s+1)h/(n+1)`, `top = cy(g) - box_h/2 - clear + arrival_drop` | **FIXED; implemented exactly** | `foodweb.rs` 381–411. Measured in the round-3 repro at Medium: gap 1 has arrivals at **152, 160** and the departure P1->P3 at **168** (top 133+11 = 144, spacing 32/4 = 8), and gap 0 has departures at 228/236/244 with the last one 12 px above the producer tops (256). Arrival stubs span [qy, y1] and departure stubs span [y0, py], so these intervals never meet and no arrival run crosses a departure stub. My geometry sweep covered the repro, `mixed(5,5)`, `cross(8)`, a span-3 web, the default and all six bundled examples × Small/Medium/Large × levels on/off. It found **0** route segments entering a box (0.3 px inset), **0** horizontal runs within stroke/2 + 0.5 px of any arrowhead's bbox, **0** runs within 3·lab of a box edge they pass, and **0** same-gap pairs closer than stroke+1. The only "problem" the sweep counted was `mixed(5,5)` at Small, which is correctly **refused** (10 runs in gap 1). At Large, X0's departure run passes under P1's head 14.3 px below the head base (290.4 vs 304.7); it only looks close at 1.3x. |
| 2 | Refuse if any gap has `height/(n+1) < stroke+1`; texts "could not be drawn:" / "Too many links skip…" / "Nothing is drawn until…"; canvas shared via `refusal()` | **FIXED** (but see MED 1) | Texts exact. Refused canvas: 420 px wide at every size, one `<rect>` (the background), header `#c0392b`, body `#111111`. Wrapping: Medium and Small keep each sentence on one line (361.3 / 289.0 px of 372); Large wraps 2+2 (widest line 368.2 of 372). Heights 97.2 / 108 / 180 px; at Large the last baseline is 170.8 px for a 16.8 px font, so the descenders fit. The refactor is **identical**: I pasted the 25045dc inline code into a harness and compared it with `build()` for 7 parse-refusal webs (no arrow, mixed arrows, self-eat, cycle, empty name, >24 names, 200-char names) × 3 sizes × levels on/off: **42/42 byte-identical**. |
| 3 | First refused at 10 Small / 12 Medium / 15 Large; n-1 draws | **CONFIRMED by rendering** | I swept `cross(n)` (`A_i -> B`, `A_i -> C`, `B -> C`) for n = 1..24. The first refusal is at n = **10 / 12 / 15**, and every n up to 22 after that is also refused (monotonic). Largest web that still draws, measured from the SVG: Small n=9 min spacing **2.34** (visible **1.14** px over the 1.2 stroke); Medium n=11 **2.66** (1.16 over 1.5); Large n=14 **3.28** (1.18 over 2.1). This matches the progress doc's band `47lab - 11 - 4lab` = 23.4 / 32 / 49.2 px. Mixed gaps follow the same count: arrivals plus departures in gap 1 of `mixed(k,m)` refuse at k+m = 10 (Small), 12 (Medium, 6+6), and k+m = 14 still draws at Large (3.28 spacing). |
| 4 | Run-height test sweeps S/M/L at the true stroke and asserts 8 runs | **PRESENT; loosened at Small/Medium** (minor 1) | The sweep and `assert_eq!(runs.len(), 8)` are there. The pair threshold went from `|dy| < 2.0` (25045dc) to `|dy| < stroke - 0.01`, which is **1.19 / 1.49 / 2.09**. That is looser than before at Small and Medium, and below the stroke+1 the figure now ships to. |
| 5 | Census: only food-web's two hashes moved (bbc126be1f01f841 -> b541d249b4277a37) | **CONFIRMED** | `git diff 68a5ed7 e76d968 -- tests/fixtures/figure_census.txt` changes only `food-web__defaults` and `food-web__start`. `census_baseline` passes on the real e76d968 tree. |

Diff scope: `foodweb.rs`, `figure_census.txt`, `figures-progress.md` and the round-4 report only. No protected file
(`screens.rs`, `theme.rs`, `fonts.rs`, `main.rs`, `examples/`) is touched, no test is deleted, and foodweb tests go
from 20 to 22.

## New defects (ranked)

1. **MED: the density refusal never reaches the editor's note area.** `warnings()` (`foodweb.rs` 214–220) only
   runs `parse()`, and `parse()` succeeds for a dense web. With `links = cross(16)` (16 × `A_i -> B`,
   16 × `A_i -> C`, `B -> C`), every size gives `warnings(p) == []` and `notes_for("food-web", p) == []`, while the
   canvas says "The food web could not be drawn:". Every parse refusal does reach the note area; a test pins this
   (`the_editor_reports_a_parse_error`). So the new refusal is the only one the editor stays silent about. It also
   depends on the label size (a Medium web with 10–11 runs in one gap draws, and the same web at Small is refused),
   so a teacher who only changes the label size gets a refused figure and no note saying why.
   Fix: have `warnings()` call the same gap count and threshold (factor it out of `build()`), and add a test like
   `the_editor_reports_a_parse_error` for density.
2. **LOW (pre-existing, now inherited by the new refusal): the refusal header's baseline is not scaled with the
   label size.** The header sits at a fixed y = 40, and the body starts at `64·lab`. At Small that is a 11.2 px
   baseline step under a 10.4 px header, so the header nearly sits on the reason line (`refused-Small.png`). At
   Large there is a 49.6 px gap under an 18.2 px header (`refused-Large.png`). Medium (24 px) is fine. This is the
   25045dc layout carried over unchanged (the refactor is byte-identical), so it is not a regression. The new
   refusal is the most likely one to be seen at Small, though, because Small refuses first.
3. **minor 1: the run-height test is looser than before and looser than the policy.** See claim 4. It passes
   runs 1.49 px apart at Medium, which is touching strokes. The figure itself refuses below stroke+1 = 2.5.
   Asserting `|dy| >= stroke + 1` would pin what ships.
4. **minor 2: `MIN_W` and the local `min_w` are duplicates.** The new const `MIN_W = 420.0` is used only by
   `refusal()`. `build()` still declares `let min_w: f64 = 420.0;` (line 295) and uses it for the empty-web canvas
   and the drawn width. Neither produces a warning, but the doc comment "a refusal never widens it" sits on one
   copy while the other copy is the one that sets the drawn width.
5. **minor 3: the `6.0 * lab` branch of `arrival_drop` never binds.** `normal_hl = max(6lab, 9)` is 9 at all three
   sizes, so `arrival_drop = max(11, 6lab)` is 11 everywhere (6·1.4 = 8.4). The arithmetic is still right; the
   branch is just dead at the shipped sizes.
6. **minor 4: one test comment overclaims.** `a_web_too_dense_to_draw_is_refused_not_banded` says twelve long
   links "would merge their runs into a band". At Medium, 12 runs would sit 2.46 px apart, which is 0.96 px of
   visible gap. That is below the owner's threshold, not a band.

## Legibility of the largest drawn webs (looked at)

At 1.3x I counted the dark runs down one column through the bundles. Small n=9 resolves 7+7 runs with fully white
(255) gaps. Medium n=11 resolves 9+9 with white or light-grey (204) gaps. Large n=14 resolves 13+13 with white or
204 gaps. The 4x nearest-neighbour zoom of Medium n=11 shows every line separate, with some gaps only one grey
pixel. So these webs are **separable but thin**: about 1.1–1.2 px of white at 1x and 1.5 px at 1.3x. The
threshold is honest. It is not generous.

## Things for the owner

- **The refusal depends on the label size.** A web with 10–11 runs in one gap draws at Medium and is refused at
  Small; 12–14 draws only at Large. The message does not say that a larger label size would fit it. Decide whether
  the reason should say so, or whether that is acceptable as it is. MED 1 makes this worse until the note area
  carries the refusal.

## Verified

- Formula, slot order and stub/run disjointness, from the emitted SVG of the repro at S/M/L.
- Box, arrowhead and run-spacing sweep (cases above) × 3 sizes × levels on/off.
- Refusal boundaries by rendering, including monotonicity up to n = 22 and mixed gaps.
- `refusal()` against the old inline code (42/42 identical).
- The density refusal's texts, colours, width, wrapping and heights at S/M/L; PNGs `refused-{Small,Medium,Large}`
  and `foodweb-dense-refused(-bw66)`. The red header prints as black at 66% grey.
- `warnings`/`notes_for` for a dense web (empty: MED 1).
- An old link carrying `style=Print` still loads and renders (6 rects). Its colours are only `#000 #111111 #333333
  #fff #ffffff none`, so it is hue-free.
- Bundled examples and the default: none refused, 0 geometry problems.
- No foodweb compiler warnings in the full build.
- Full suite on the real e76d968 tree (own target dir, 8 GB scope, `cargo test --lib -j2`): **1924 passed,
  0 failed, 5 ignored**. The commit reported 1921 because it ran without 68a5ed7; the difference comes from the
  other session's tests. `census_baseline` passes.

## Not checked

- Webs above ~25 names (capped by the parser) and runs crossing lane verticals: crossings are inherent to the
  right-side lanes and were accepted earlier.
- 1x-scale PNG legibility (I looked at 1.3x only).
- The UI in a running app: the editor's note area was checked through `notes_for`, not on screen.
- CJK/wide-text measuring (recorded and accepted).

## Census note

Only `food-web__defaults` and `food-web__start` moved (`bbc126be1f01f841` -> `b541d249b4277a37`). No other id was
added, removed or changed by this commit. `census_baseline` passes on the e76d968 tree.
