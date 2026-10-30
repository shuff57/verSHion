# Verification critic on ecd71c8: the food web's first fix round

A verification pass over `ecd71c8` ("food web critic round 1: skipping links routed, distinct attachments"), read-only,
in its own `git archive` copy, with its own geometry harness (SVG parsed independently: `<line>`, `<polyline>`,
`<polygon>`, `<text>` x/y) and its own PNG renders looked at with the image reader. Its verdict: **PASS** — the blocker
is genuinely fixed across its whole family, the major is fixed for the reported case and mostly across its family, every
minor is fixed except "the unused `mut` warning is gone" (false) and the long-name wrap (true for Latin, not for CJK).
No new blocker or major; one MED and five LOW residuals.

## Claim table

| # | Claim | Verdict | Evidence |
|---|---|---|---|
| B1 | A link that skips a level is routed outside the rows through its own lane, so every typed link is drawn | **FIXED** | The omnivore now renders three links: `Grass -> Fox` leaves Grass's top-right stub, runs right, up a lane at x=317, left below Fox's row, and enters Fox's bottom beside the straight `Rabbit -> Fox`. Family checked (8 prey, 3-level skip A->D, two long links from one prey sharing a departure y, two long links into one predator sharing an arrival y, levels off, Small/Large): every routed segment is outside every box interior (0 crossings in a 200-sample-per-segment sweep) and every lane x < canvas width. Routed arrival heads point up (apex at the box bottom), confirmed on every triangle. |
| M1 | Every link gets distinct attachment points (top for outgoing, bottom for incoming); a many-prey apex no longer stacks on one point | **FIXED, PARTLY for a short-named apex** | `Apex` with 8 prey: 8 arrival x's at 426.7…466.5 (bot_frac k/9), all inside the box [421.0..472.2], heads separable in a 5× zoom. **But** an apex named `X` (box 29.36 px wide) gives 3.26 px spacing against a ~7 px head base, so the 8 heads overlap into a dense band (see MED-1). Attachments never leave the box width in any case (fractions are strictly in (0,1)). |
| Minor | A narrow web is centred in the 420 px minimum | **FIXED** | Measured body extent (boxes + links + level names, excluding the count line) vs canvas: `A -> B` L=138/R=132; chain L=97.5/R=91.5; `many_long` L=87.3/R=87.3; omnivore L=R=102.7. Off-centre ≤ 6 px in every narrow case; the 420 minimum is used, not exceeded. |
| Minor | The inert Style control is removed (the figure is already monochrome) | **FIXED, rationale untrue** | No `style` param remains. But the figure is **not** monochrome: the error canvas inks the reason in red `#c0392b` (467 reddish pixels in the cycle PNG; 0 hue pixels in the valid default). Removing Style removed the only grey error render (`hint_col` used to be `#555555` in Print). See LOW-3. |
| Minor | A long unbroken name breaks so it stays inside the error canvas | **PARTLY** | Latin/W true: 300 `x` wrap to 52-char chunks, worst right edge 402.6 on 420; 200 `W` chunk right edge 406.9; every wrapped line start x=24. **CJK false**: `樹` is measured at 0.56 em by `text_w` but renders near 1 em, so a 200-`樹` name wraps on 51-char lines whose real width is ~676 px and runs far off the 420 canvas. This is the recorded `text_w` under-measure, **not worse than before** (before, nothing wrapped). |
| Minor | `←` (arrow key) is read as `<-` | **FIXED** | `Fox ← Rabbit` → names `[Rabbit, Fox]`, edge Rabbit->Fox. `A -> B <- C` is still refused ("mixes"). Note (LOW-6): a `←`/`→` *inside* a name is parsed as an arrow, so `Grass←Field -> Fox` errors "mixes". |
| Minor | Runs of whitespace inside a name are tidied to one space | **FIXED** | `Arctic\u{00A0}\u{00A0}fox -> Wolf` and `Arctic\tfox -> Wolf` both draw `Arctic fox`; leading/trailing trimmed; a whitespace-only name (`A ->   \t`) gives "empty name"; an indented `#` line is still skipped as a note. |
| Minor | A parse error shows in the Figures tab's editor note area (`notes_for`) | **FIXED** | `mod.rs` maps `"food-web" => biology::foodweb::warnings`. `warnings` and `notes_for` both return the message on a parse error and on a cycle, and both return empty for a valid (non-empty) web. |
| Minor | The cycle refusal's second line is reworded ("until the links form a one-way web") | **FIXED for cycles; false for the name cap** | Cycle canvas: accurate. But the same line is drawn for **every** parse error; for >24 names it says "Nothing is drawn until the links form a one-way web" when the links *do* form a one-way web and the real cause is the 24-name cap (see LOW-4). |
| Minor | An unused `mut` warning is gone | **NOT FIXED** | The build still emits `warning: variable does not need to be mutable --> src/figures/biology/foodweb.rs:78:13` (`let mut parts`), unchanged from `39ae4f6`. Also `warning: unused variable: l --> foodweb.rs:331:18` (the box_x placement loop). |
| Tests | Routed link distinct starts/ends; many-prey distinct arrivals; `←` and whitespace; long name inside canvas; editor parse error; PNG `fanin` case | **PRESENT, one weak** | All five tests and the `fanin` PNG case exist. The long-name test asserts only `x < w` for each `<text>`, and every error line is start-anchored at x=24, so it would pass even if text ran off the right edge — it does not test the claim (see LOW-5). |
| Census | Only `food-web__defaults` and `food-web__start` moved | **CLEAN** | `git diff 39ae4f6 ecd71c8 -- tests/fixtures/figure_census.txt` changes exactly those two hashes and nothing else. |
| Protected | The commit touches no protected file | **CONFIRMED** | Diff names only `docs/figures-progress.md`, `src/figures/biology/foodweb.rs`, `src/figures/faq.rs`, `src/figures/mod.rs`, `tests/fixtures/figure_census.txt`. No test was deleted and no bound loosened (no removed line contains test/assert/bound/tol). |

## New defects (ranked)

1. **MED — a short-named apex re-creates the head cluster the fix was for.** Reproducer: `Grass -> Prey1` … `Grass -> Prey8`
   plus `Prey1 -> X` … `Prey8 -> X`. The `X` box is 29.36 px wide, so the eight arrivals sit 3.26 px apart while a head
   base is ~7 px wide; in a 6× zoom the heads merge into a solid band. The reported `Apex` (51 px box, 6.3 px spacing)
   is separable, so the fix is right for the named case and incomplete for the case beside it. Spacing is `width/(n+1)`;
   nothing shrinks the head with prey count.

2. **LOW — routed and straight arrowheads differ in size, and the gap flips sign with label size.** The new `head`
   helper uses `hl = max(6·lab, 8·lab)`; `svg::arrow` uses `hl = max(6·lab, 9)` (an absolute 9 px floor). At Small
   (lab 0.8) a routed head is 6.4 vs a straight 9 (−29 %); at Large (lab 1.4) it is 11.2 vs 9 (+24 %). In one picture
   (`omni`, Small) the routed arrival head is visibly smaller than the straight one; at Large it is larger.

3. **LOW — the figure is not monochrome in its error state, and the removal of Style took the grey variant with it.**
   `ERR = #c0392b` is forced for `hint_col` regardless of state; the cycle PNG carries 467 red pixels. `style` is now a
   no-op (`colour` and `print` PNGs are byte-identical for defaults, cycle, empty, fanin), so `food_web_pngs`'s "print"
   variants are duplicates and the BW66 pass is the only monochrome evidence left. `print_style_is_grey_only` also no
   longer checks this figure at all (it skips any figure without a `style=Print` choice).

4. **LOW — the generic second error line is false for the name cap.** With 25 names (`N1 -> M1` … `N25 -> M25`) the canvas
   reads "More than 24 names (line 13): split the web into two pictures." then "Nothing is drawn until the links form a
   one-way web." — true for a cycle, false here (the links already form a one-way web). Also misleading for the
   "mixes -> and <-" error.

5. **LOW — the new long-name test does not test what it claims.** It asserts each `<text>`'s `x < w`; every error line is
   start-anchored at x = 24, so the assertion is trivially true and would pass with text overflowing the right edge.
   Underneath, `wrap`'s broken chunks are the first prefix that *exceeds* `max` (up to one glyph over), so lines run to
   ~407 px on a 420 canvas — safe for Latin only because of the 48 px reserve, and never checked after the glyph.

6. **LOW (design decision, not a bug) — `←`/`→` inside a name is an arrow, not literal.** `Grass←Field -> Fox` becomes
   "mixes -> and <-" and is refused. Acceptable, but worth stating in the FAQ if a name may ever hold those glyphs.

## Verified as correct (independently)

Routed geometry: every polyline segment clears every box interior; the departure/arrival stubs stay outside the box
edges; lanes never exceed the canvas width for 1 or 6 long links; horizontal runs at `py−8·lab` / `qy+8·lab` sit in the
inter-row gaps at Small, Medium and Large. Span-1 straight arrows never enter a box (they live entirely in the gap), so
there is no remaining "hidden behind a box" case for short links. Arrowheads for routed arrivals point up (apex at the
box bottom) in every rendered case. Attachment points stay strictly inside each box's left..left+width (fractions
k/(n+1)). Centring measured symmetric within 6 px on 1-, 2- and many-node webs. Tidy handles spaces, tabs, NBSP,
leading/trailing and whitespace-only input; `#` notes (even indented) are skipped. `←`/`→` read; mixed arrows refused.
Old links carrying `style=Print` still load: `params_from_json` keeps the key and `normalize` ignores unknown keys, so
nothing panics — the key is simply unread now (behavioural change: a previously-Print link renders the colour output).

## Not checked

The share-link/URL decoder end-to-end (I read `params_from_json`/`normalize` and rendered through `Params` directly, but
did not fetch a real `vershion://figure/...` URL). The Figures-tab UI wiring visually (I checked the `notes_for` arm in
`mod.rs` only). The 20–24-name "very wide strip" and the `text_w` CJK under-measure as *fixed* (both recorded leftovers;
CJK confirmed still overflowing but not worse). Print BW66 of every case (I checked the red error pixels and that the
valid render has none; red luma is 87, so it prints black rather than vanishing).

## Census note

`git diff 39ae4f6 ecd71c8` moves only `food-web__defaults` and `food-web__start` (43b8b660… → 73730955…), the expected
consequence of the attachment spread and centring; no figure was added or removed. Full `cargo test --lib -j2` in this
copy (which adds one harness test): **1904 passed, 0 failed, 5 ignored** — the builder's 1903 + the harness, so 1903/0/5
is confirmed.
