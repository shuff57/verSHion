# Verification critic on 25045dc: the food web's per-gap run slots

A verification pass over `25045dc` ("figures phase 4: food web verification round 3 (one run height per gap)"),
read-only, in its own `git archive` copy (`/tmp/fwv4`), with its own SVG parser (rects, lines, polylines, arrowheads,
text) and my own PNG renders looked at with the image reader. Scope is the commit's own diff, `bab13f8..25045dc`
(the parent is another session's commit and none of its changes are counted here). **Verdict: PASS as a fix (0
blocker, 0 major, 0 new MED)** — the round-3 MED is fixed: the two runs that sat 1.00 px apart and overlapped
120.5 px now sit **6.25 px** apart at Medium (10.85 Large, 3.90 Small), with 0 near-height/overlapping pairs
anywhere in the repro. **The "no regression" half does not hold:** reserving `drop` at both ends of the gap band
shrinks the slot spacing from `(clear-drop)/(n+1)` to `(clear-2drop)/(n+1)`, so the recorded run-packing LOW is
materially worse — at Medium the stroke-merge threshold falls from n≈22 to n≈15, and a 15-run gap is reachable
with **11 names on a 693 px canvas**, not the recorded 20–24-name "wide strip".

Counts: **0 blocker, 0 major, 0 new MED; 1 LOW materially worse (the recorded packing class), 1 LOW (the recorded
limit is not honest across label sizes), 1 minor (new test tolerance).**

## Claim table

| # | Claim | Verdict | Evidence |
|---|---|---|---|
| 1 | Per gap `g`, every run in that gap (departures from row `g`, arrivals into row `g+1`) takes its own slot: `clear = LEVEL_GAP*lab - box_h`, `drop = max(8*lab, normal_hl+2)`, `height = max(clear-2*drop,0)`, `top = cy(g)-box_h/2 - clear + drop`, `y(s) = top + (s+1)*height/(n+1)` | **FIXED; implemented exactly** | `foodweb.rs` 377–396 is literally that formula. The round-3 repro `P0 -> P1 / P1 -> P2 / P2 -> P3 / X0 -> P2 / X1 -> P2 / X2 -> P3 / P1 -> P3` now has gap 1 runs at y **150.25 / 156.50 / 162.75** (Medium), i.e. the old 150.00/151.00 pair is **6.25 px** apart against a 1.5 px stroke; Small 3.90 px, Large 10.85 px. Every slot height is distinct and every run is inside its band (below). |
| 2 | One new test `a_departure_and_an_arrival_never_share_a_run_height` walking every pair of runs; PNG `gap-clash` case | **PRESENT; non-vacuous, but weak** | Test present (19 tests in `foodweb.rs`, 18 before), PNG case present and rendered (`foodweb-gap-clash.png`). It fails on the parent geometry: reconstructing the old runs from the unchanged box geometry gives the arrival at 150.00 and the departure at 151.00, `|dy| = 1.0 < 2.0` over 120.5 px of x, so the assertion trips. Weaknesses in defect 3. |
| 3 | Census only food-web moved; suite 1919 / 0 / 5 on a clean copy | **CONFIRMED** | `git diff bab13f8 25045dc -- tests/fixtures/figure_census.txt` changes only `food-web__defaults` and `food-web__start` (`9775549ac600dadf` → **`bbc126be1f01f841`**); my own FNV-1a 64 of `FigureDef::svg` for the default render reproduces **bbc126be1f01f841**. No other id added, removed or changed, so enzyme/chisquarefit/cladogram/hardyweinberg are untouched. Full `cargo test --lib -j2` in my copy (own target dir, 8 GB scope): **1919 passed, 0 failed, 5 ignored** (the new test and `census_baseline` are in it). |
| 4 | "Recorded limit: past roughly 14 long links through one gap the slots sit closer than the 1.5 px stroke; such a web is already a wide strip." | **PARTLY TRUE, misleading** | At Medium the stroke threshold is n = 16, so "past 14" is defensible; at Small it is n = 13 and at Large n = 20. A ≥0.5 px visible gap ends at n = 11 (Medium), 8 (Small), 15 (Large). The "wide strip" justification is false: n = 15 needs 11 names and renders on a 693 px canvas. Details in defect 2. |
| 5 | No protected file, no test deleted or loosened | **CONFIRMED** | The commit touches only `foodweb.rs`, `figure_census.txt`, `figures-progress.md` and the round-3 report; `screens.rs`, `theme.rs`, `fonts.rs`, `main.rs`, `examples/` are not in the diff. The only removed lines are the old comment/`step` code; tests go 18 → 19. |

## New defects (ranked)

1. **LOW (regression in the recorded run-packing class; materially worse) — the per-gap band costs a third of the
   slot spacing, and a compact 11-name web is a solid band.** Round 3 spaced departures along
   `36/(n+1)` at Medium (one `drop` at the near end; uncapped for n > 5, which is every row of the table); round 4
   reserves `drop` at both ends, so the spacing is `25/(n+1)`. The table is for all n long links in one gap (the
   cross web), measured from the emitted SVG:

   | n per gap | round 3 spacing (visible) | round 4 spacing (visible), Medium | round 4, Small | round 4, Large |
   |---|---|---|---|---|
   | 10 | 3.273 (1.773) | 2.273 (**0.773**) | 1.418 (0.218) | 3.945 (1.845) |
   | 14 | 2.400 (0.900) | 1.667 (**0.167**) | 1.042 (**−0.158**) | 2.893 (0.793) |
   | 15 | 2.250 (0.750) | 1.5625 (**0.0625**) | 0.975 (**−0.225**) | 2.713 (0.613) |
   | 20 | 1.714 (0.214) | 1.190 (**−0.310**) | 0.743 (−0.457) | 2.067 (**−0.033**) |
   | 25 | 1.385 (−0.115) | 0.962 (−0.538) | 0.600 (−0.600) | 1.669 (−0.431) |

   (visible = spacing − stroke; stroke 1.5 Medium, 1.2 Small, 2.1 Large.) Repro for the reachable case with only
   **11 names, 25 links, canvas 693 px wide**: 5 producers `P0..P4`, one primary `M0` (`Pi -> M0`, `M0 -> Sj`), and
   the first 15 of the 25 `Pi -> Sj` links (the other ten short links keep the levels). 15 departures in gap 0 and
   15 arrivals in gap 1 at **1.5625 px** spacing, 1.5 px stroke: at the 1.3× preview the 15 runs rasterise as one
   solid band (my best-column count collapses 15 runs to 4–6 bands, while at n = 10 it still resolves ~10 lines; the
   4× render shows the individual 1.5 px lines only when magnified). At n = 14 Medium the visible gap is 0.17 px and
   the 4× image is already a band; at Small the strokes **overlap** from n = 14 (13 adjacent near pairs) and n = 15
   (14). The pre-fix commit drew the same web with 0.75 px (n = 15 Medium) and 0.46 px (Small) of visible gap. This is
   the same class the round-3 report recorded as LOW, so I do not call it a new MED — but it is materially worse, and
   the "already a wide strip" qualifier is false for this web. A cheaper use of the band would be first and last slot
   at the edges (`height/(n-1)`), which recovers only ~0.2 px at n = 15; the real limit is the 25 px band.
2. **LOW (claim honesty) — "past about 14" is a Medium-only stroke number, and the visible limit is lower.** Measured
   minimum slot spacing for the two named cases: **n = 14 Medium 1.6667 px** (0.1667 above the 1.5 px stroke),
   **n = 15 Medium 1.5625 px** (0.0625 above). But at Small (stroke 1.2) n = 14 is **1.042** (0.158 *below*) and
   n = 15 is **0.975** (0.225 below); at Large (stroke 2.1) n = 20 is **2.067** (0.033 below). The exact stroke
   thresholds are n = 15 Medium, 11 Small, 19 Large; the ≥0.5 px visible thresholds are 11 Medium, 8 Small, 15 Large.
   So "past about 14" is honest only for "Medium, and only for touching"; it is not a size-independent limit, and the
   note should say so (and drop the wide-strip excuse).
3. **MINOR — the new test's tolerance is wider than the stroke and it checks one Medium web.**
   `a_departure_and_an_arrival_never_share_a_run_height` only fails when `|dy| < 2.0`; the stroke is 1.5 (Small 1.2),
   so a 1.7 px pair that still reads as merged passes. It renders the one repro at Medium only, so a Small-only
   collapse is not caught, and it does not assert that any runs were found (a render with no routed links would pass
   vacuously). It is non-vacuous on the parent (see claim 2).

## Verified as correct (independently, from the emitted SVG and PNGs)

- **The formula is implemented exactly** as claimed (`clear`, `drop`, `height`, `top`, `y(s)`), and the gap lists
  contain departures from row `g` and arrivals into `g+1` for every routed link (`long`), with no out-of-range gap
  index (a routed link always has `level[predator] ≥ level[prey]+2`, so `level[predator]-1 ≥ level[prey]+1`).
- **All slot heights are distinct in every gap** for n = 2, 3, 5, 10, 14, 15, 20, 25 at Small/Medium/Large, levels
  on and off (the `{:.2}` output cannot collapse two slots until n > 3000, far past the 24-name cap).
- **Every run is inside the usable band**: lowest slot = upper box bottom + `drop` + spacing, highest = lower box
  top − `drop` − spacing. Measured minimum clearance **11.6 px** (Small, n = 25) against `drop` 11.0; the tightest
  Medium case is 11.96 px. `height = 15.6 / 25.0 / 43.4 px` at Small/Medium/Large, so **height ≤ 0 cannot occur at
  a supported label size**.
- **No run, stub or lane enters a box** (segment-vs-rect interior test against every white box, 0 hits across all
  115 renders); **no drawn point is off-canvas** (polylines, `<line>` endpoints and arrowhead polygons, 0); every lane
  is right of every box by at least 20 px (and the rightmost lane is ≥20 px inside the canvas edge).
- **Bands of adjacent gaps are disjoint by more than a box**: the closest runs in neighbouring gaps are **≥ 46.4 px**
  apart (n = 25 Small) against `box_h` 23.2 and the theoretical minimum `box_h + 2·drop` = 45.2; the same link's
  departure and arrival are always in different gaps and never share a height.
- **No run enters an arrowhead**: every arrival run sits `drop`+spacing below the row's box bottom (≥ 11.6 px), while
  a head extends 9 px below it, so the nearest approach is ~2.6 px (n = 25 Small); 0 bbox hits.
- **Shared prey, shared predator, 3-level skip and 5-level skip**: 2 or 1 runs in their gaps, 5.2–14.5 px apart,
  0 near pairs; the repro at Large has 3 runs at 10.85 px in its busiest gap.
- **PNGs read as the typed pairs**: `gap-clash` shows three distinct runs where the round-3 bug merged two (the two
  arrivals into P2 and the P1→P3 departure are separate), `two-omni` reads as two disjoint pairs, `fanin-narrow`
  keeps eight distinct heads, `default` and `large` read correctly (Rabbit→Fox alone on its route), and the omnivore
  example shows Fox eating Grass and Rabbit. Crossings are plain X crossings (a run crossing a stub or a short
  diagonal), not junctions; no run touches a head.
- **Old `style=Print` link still loads**: byte-identical 2625-byte output to the same links without the key, no panic.
- **A valid render is hue-free** (0 colours with RGB spread > 12; the palette is #000/#111/#333/#fff); **an error
  canvas's heading is `#c0392b`** (luma 95.8, so it prints black at the 66 % threshold), and the red is absent from
  valid webs.
- **No protected file** is touched, and no test was deleted or loosened (only the old `step` code was removed).

## Not checked

The egui Figures-tab UI itself (I drove `Params`, `warnings`, `notes_for` and the renderers, but did not screenshot
the app). A real `vershion://figure/...` share URL end-to-end. PDF/DOCX export. A release-build suite (this was
debug). Every error path at Large. The 24-name / 64-long-link extreme (the recorded wide-strip case). The recorded
leftovers as fixed — the 20–24-name wide strip, "Level 5" past "Quaternary consumers", the always-drawn count line,
and the wide/CJK `text_w` under-measure (the other session improved CJK measuring in `bab13f8`, and food web uses
`cladogram::text_w`, so it benefits; not re-raised) — I did not re-measure them this round and none is materially
worse here.

## Census note

Only `food-web__defaults` and `food-web__start` moved (`9775549ac600dadf` → **`bbc126be1f01f841`**), the expected
effect of moving the one routed link in the default meadow from `py−drop` / `qy+drop` to the middle of its gap
(and of the new per-gap slots in general). Both cases render the same default params here (`start::overrides` has no
food-web entry), so the two hashes are equal. I reproduced `bbc126be1f01f841` with my own FNV-1a 64 over
`FigureDef::svg`, and `census_baseline` passed in the suite. No other figure id appears in the fixture diff, so the
enzyme/chisquarefit/cladogram/hardyweinberg hashes are unchanged from `bab13f8`.

Full suite in my copy: **1919 passed, 0 failed, 5 ignored** (`cargo test --lib -j2`, own target dir, 8 GB scope; the
known `absurd_windows_cannot_run_away` flake passed this run).
