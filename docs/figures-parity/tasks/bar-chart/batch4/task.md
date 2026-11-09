# Interaction task: `bar-chart`, batch 4

## The task

"From the opening state, make a stacked bar graph of how Year 7, 8 and 9 travel (Walk, Bus, Car),
with each part's value written inside it and the key below the graph."

Data: Year 7 = 12, 10, 8; Year 8 = 15, 8, 6; Year 9 = 9, 14, 5.

## Reference (mathfigures.com/bar-graph, headless, no login)

1. Type the table cell by cell: Bus and Car headings in the + column (Walk replaced the first
   heading), then each category name and its three values: 17 typed cells. A spreadsheet paste
   into the table would be 2 actions (click a cell, paste); it was not done headless.
2. Open Bars, click Stacked, click Inside (values on the bars): 3 clicks.
3. Open Look, click Below (key): 2 clicks.

Screenshots: `ref/00-opening.png`, `ref/01-table-typed.png`, `ref/02-done.png`.

## Ours before (main at 6d3a640)

1. Click the data box, select all, paste the four lines `Year, Walk, Bus, Car` ... (3 actions).
2. Refused: the figure took at most two counts per category (`ours-before/01-paste.png`). Not
   finished: no third group, no stacking, no values inside, no key position.

## Ours after

Under Xvfb with the app's QA hooks (`VERSHION_SCRIPT`); the alpha notice's "Got it" click is the
environment, not a teacher action, and scrolling the settings panel is not counted.

1. Click the data box, Ctrl+A, paste the table (3 actions). Three groups appear side by side,
   named by the headings line (`ours-after/01-paste.png`).
2. Open Bars, click Stacked under "Several groups" (2): `ours-after/02-stacked.png` (stack
   totals 30, 29, 28 written at the ends).
3. Click Inside under "Values sit" (1): the parts' values on white chips (`ours-after/03-inside.png`).
4. Open Look, click Below under "Key" (2): `ours-after/04-key-below.png`.

## Result

| | reference | ours before | ours after |
|---|---:|---:|---:|
| actions to finish | 22 typed (or 2 + 5 with a paste) | stopped after 3 | 8 |
| finished? | yes | no: three counts refused | yes |
| dead ends | 0 | the paste (refused) | 0 |

How it differs:
- Input: the reference is a spreadsheet-like grid; ours is a text box that takes the same table
  typed or pasted (tab, comma or semicolon separated), so a paste is one action either way.
- Every step is a panel control on both sides; neither acts on the bars directly for this task.
- Ours writes values on white chips (verSHion's look) where the reference writes white or black
  text on the fill.
- Student copy: on ours the values, the bars (with a table to draw from) or the scale numbers can
  be left out; titles can be blank lines.

Verdict: finished, about the same actions with a paste (8 against 7); far fewer than typing the
reference's grid cell by cell (22).
