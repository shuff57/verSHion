# Interaction task: `venn-diagram`, batch 3

## The task

"Two sets, Even and Prime: put 4 6 8 in Even only, 2 in both, 3 5 7 in Prime only and 1 9 outside, and shade Prime only and the
overlap."

Capabilities: cap-i01 (click a region to type in it), cap-i02 (click a region to shade it), cap-05b (shade any chosen regions),
cap-03 (names), cap-01a (region text).

## Reference (mathfigures.com/venn-diagram, headless playwright, 1280x800)

Clicking a region opens a pop-up with the region's Text box (focused) and a Shaded checkbox; clicking the next region moves the
pop-up there.

| # | action | shot |
|---|---|---|
| 1-2 | click Name of set A, type Even | |
| 3 | click the A only region (pop-up, text focused) | ref/01-click-region-popup.png |
| 4 | type 4 6 8 | |
| 5 | click the overlap (pop-up moves) | |
| 6 | type 2 | |
| 7 | tick Shaded | ref/02-overlap-typed-and-shaded.png |
| 8 | click B only | |
| 9 | type 3 5 7 | |
| 10 | tick Shaded (in headless the pop-up's box timed out; the same tick in the panel's region table was used) | |
| 11 | click Neither | |
| 12 | type 1 9 | |
| 13 | Escape | |
| 14-15 | click Name of set B, type Prime | ref/03-done.png |

## Ours before (main at bd2ea88)

No handles: a click or double-click on a region does nothing (ours-before/01-click-region-nothing.png). The task can be finished in
the panel: Set A (click, select, type), Set B (3), Region values `4 6 8, 3 5 7, 2, 1 9` (3), Shade: open the list, pick B (2), 11
actions. Only a fixed list of shadings was possible (Prime only and the overlap happens to be "B"; "Even only and Prime only" was not).

## Ours after (this branch, figtest build under Xvfb, 1280x800)

One click area over the box: a click in a region shades it (again clears it); a double-click opens a small box just under the
pointer with the region's text selected, so typing replaces it; double-clicking the next region keeps what was typed and opens that
one (the double-click's first click, which shaded, is taken back). Hover outlines the region and names it with the sets' names.
Each set's name is renamed by a double-click on it.

| # | action | shot |
|---|---|---|
| 1-4 | double-click Music, Ctrl+A, type Even, Enter | |
| 5-8 | double-click Sport, Ctrl+A, type Prime, Enter | |
| 9 | double-click Even only (box opens under the pointer, 12 selected) | ours-after/01-double-click-region-edit-box.png |
| 10 | type 4 6 8 | |
| 11-12 | double-click the overlap, type 2 | |
| 13-14 | double-click Prime only, type 3 5 7 | |
| 15-17 | double-click outside the circles, type 1 9, Enter | ours-after/02-regions-typed.png |
| 18 | click the overlap: shaded | |
| 19 | click Prime only: shaded (Shaded regions now reads "B") | ours-after/03-click-two-regions-shaded.png |

ours-after/04-hover-outline.png: the pointer in Even only outlines that crescent and the tip reads "Even only: click to shade it
(again to clear it), double-click to type what goes in it".

The panel route after the change is the same 11 actions as before, and any combination can now be typed (`A only, B only`,
`(A or B) and not C`) or clicked.

Steps were real input in the running app (VERSHION_SCRIPT). The QA hook's `dclick` now sends press, release, press, release on four
frames in a row (all in one frame read as a single click). Frames under Xvfb run about 10 ms apart, so the script idles between
actions: egui reads a click and a double-click within 0.6 s as a triple-click.

## Result

| | reference | ours before | ours after |
|---|---:|---:|---:|
| actions to finish (on the figure) | 15 | - (no figure route) | 19 |
| actions to finish (panel) | 15 | 11 | 11 |
| finished? | yes | yes, in the panel only | yes, on the figure or in the panel |
| dead ends | 0 (1 automation timeout on the pop-up's checkbox) | 2 clicks on the figure did nothing | 0 |

How it differs:
- Typing in a region: the reference opens a pop-up on a single click; ours opens a box on a double-click. A single click is kept
  for shading, so shading a region is 1 action here against 2 there (click, tick).
- Names: the reference renames a set in the panel (click the box, type: 2 actions). On our figure a name's inline box opens with
  the old name unselected, as every figure's label box does, so it takes Ctrl+A and Enter too: 4 actions a name. That is most of the difference in the figure
  route; the panel route is shorter than the reference's.
- Student copy: both leave the regions empty; ours can also give each empty region a blank line (Empty regions: A blank line).

Verdict: finished, more actions on the figure route (19 against 15: renaming a set on the figure needs Ctrl+A and Enter); fewer in
the panel (11). Shading is one click a region.
