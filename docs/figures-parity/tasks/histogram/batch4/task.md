# Interaction task: `histogram`, batch 4

## The task

"Draw a histogram from this frequency table with unequal bins (0 to 10: 4, 10 to 15: 8, 15 to 30: 6), put a frequency
polygon over the bars, and write each bin's frequency above its bar."

Capabilities: cap-02a (frequency table), cap-02b (unequal bin widths), cap-05a and cap-05c (polygon over the bars, closed to
the axis), with frequency density on the vertical axis (cap-03c's statistics for unequal bins).

## Reference (mathfigures.com/histogram, headless playwright, 1280x720)

The reference opens on empty axes, 0 to 10 both ways, with a Data set box (ref/00-open.png).

| # | action | shot |
|---|---|---|
| 1 | Frequency table (a toggle over the data box): a First bin starts at box and an Up to / Frequency grid | ref/01-frequency-table.png |
| 2 | type 0 in First bin starts at | |
| 3 | type 10 in Add a bin | |
| 4 | type its frequency, 4 | |
| 5 | type 15 | |
| 6 | type 8 | |
| 7 | type 30 | |
| 8 | type 6 | ref/02-type-start-and-three-rows.png |
| 9 | open the Bars card | |
| 10 | Frequency polygon: Over the bars (Close the polygon down to the axis appears, ticked) | |
| 11 | tick Frequencies above the bars | ref/03-polygon-over-bars-frequencies.png |

The axis switches to "Frequency density" by itself once the widths differ, the polygon's feet sit at the middle of an empty
bin of the same width each side (-5 and 37.5), and the numbers written above the bars are the frequencies (4, 8, 6), not the
heights. A summary under the grid lists each bin, its frequency and density. Seen on the way: Horizontal axis: Every step
numbers -5 to 40 by 5, "fits the bins" (ref/04-seen-every-step.png).

## Ours before (main at 6d3a640)

The histogram takes a list of values only: no frequency table, so no unequal bins, no polygon (ours-before/01-open-no-table.png).
Not finished.

## Ours after (debug build under Xvfb, 1280x800)

| # | action | shot |
|---|---|---|
| 1 | Data: Frequency table (the first control in the Data card). The table opens filled with the data set's own bins (50 up to 100 in tens), so the bars do not change until something is typed | ours-after/01-frequency-table.png |
| 2 | type 0 in First bin starts at | |
| 3 | type the three rows into Up to, frequency (pasted here as one run: 10, 4 / 15, 8 / 30, 6) | ours-after/02-type-start-paste-rows.png |
| 4 | open the Bars card (the Bins card is hidden in table mode) | ours-after/03-open-bars.png |
| 5 | Frequency polygon: Over the bars (Close the polygon down to the axis appears, ticked) | ours-after/04-polygon-over-bars.png |
| 6 | tick Write the value above each bar | ours-after/05-values-above.png |

Every step is a real click, key or paste in the running app; nothing seeded. The first-run alpha notice was dismissed first and
the side panel scrolled twice with the wheel (not counted, as on the reference).

## Result

| | reference | ours before | ours after |
|---|---:|---:|---:|
| actions to finish | 11 | - | 6 |
| finished? | yes | no | yes |
| dead ends | 0 | - | 0 |

How it differs: the reference types each row into a two-column grid (one box per number); ours takes the whole table in one text
box, one bin per line, so a table copied from a worksheet (`140 < h ≤ 150	7`, `10-19, 4`) goes in with one paste; class limits
like 10-19 are drawn at the boundaries 9.5 and 19.5. Both switch the axis to Frequency density by themselves, both close the polygon
at the middle of an empty bin each side, both write the frequencies (not the heights) above density bars. Ours keeps those numbers
clear of the polygon's lines (the 8 moves to the side of its vertex). The reference lists bin, frequency and density under the grid;
ours shows only what it could not read (red notes). Neither has direct manipulation on this figure. Student copy: ours hides the
bars, polygon and numbers and keeps the axes, scale and titles.

Verdict: fewer-or-equal actions and finished.
