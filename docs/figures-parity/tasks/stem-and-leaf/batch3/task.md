# Interaction task: `stem-and-leaf`, batch 3

## The task

"Plot Class A's and Class B's quiz scores back to back, name the two sides, and put the unit (points) on the key."

Capabilities: cap-01b (two sets back to back), cap-01c (name each set), cap-07c (a measurement unit on the key's value).
Class A: 52, 58, 61, 63, 65, 66, 68, 70, 71, 72, 74, 75, 75, 77, 79, 82, 84, 85, 88, 93 (our opening data).
Class B: 55, 59, 60, 62, 64, 67, 69, 71, 73, 73, 76, 78, 80, 81, 86, 90 (our default second set).

## Reference (mathfigures.com/stem-and-leaf, headless playwright, 1280x720)

The reference opens on an empty plot with Stem and Leaf headings and the key "3 | 4 = 34" (ref/00-open.png).

| # | action | shot |
|---|---|---|
| 1 | Back-to-back (a toggle over the data box) | ref/01-back-to-back.png |
| 2 | type the left name, Class A | |
| 3 | type the left data | |
| 4 | type the right name, Class B | |
| 5 | type the right data | ref/02-type-both-sets.png |
| 6 | open the Key card | |
| 7 | type points in Unit | ref/03-key-unit.png |

Seen on the way: the Look card's Leaves in boxes adds a "Boxes in each row, at least" box, and Key: Blank line draws "Key:" and a
rule (ref/04-look-card.png, ref/05-boxes-blank-key.png).

## Ours before (main at bd2ea88)

One data set only: no second set, no names, and the key always reads "Key: 5 | 2 means 52" with no unit
(ours-before/01-open-no-second-set.png). Not finished.

## Ours after (debug build under Xvfb, 1280x800)

| # | action | shot |
|---|---|---|
| 1 | Back-to-back (the first control in the Data card): the opening data goes on the left, a default second set on the right, headed Set A and Set B, and the key reads both ways | ours-after/01-back-to-back.png |
| 2 | type the left name, Class A | (seeded) |
| 3 | type the left data (already the opening data here; counted to match the reference) | |
| 4 | type the right name, Class B | (seeded) |
| 5 | type the right data (already the default second set) | |
| 6 | open the Key card | |
| 7 | type points in "Unit on the key's value" | ours-after/02-named-key-unit-seeded.png |

Seeded: the names and the unit went in through VERSHION_FIG_PARAMS because the QA hook cannot scroll the side panel to those text
boxes; step 1 is a real click in the running app. ours-after/00-open.png is the opening state with the Plot toggle above Numbers.

## Result

| | reference | ours before | ours after |
|---|---:|---:|---:|
| actions to finish | 7 | - | 7 |
| finished? | yes | no | yes |
| dead ends | 0 | - | 0 |

How it differs: both put the one-set/back-to-back choice above the data and the names beside each set's data; both write the
key as left leaf | stem | right leaf with each side's value and name. Ours opens on a filled plot (the reference opens empty),
so the second set appears filled at once; our key wraps to a second line when it is wider than the plot. Neither has direct
manipulation on this figure. Student copy: ours hides every leaf on both sides and keeps stems, names and key; with boxes on,
every row gets the same number of empty boxes.

Verdict: fewer-or-equal actions and finished.
