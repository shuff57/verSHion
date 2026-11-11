# Interaction task: `coordinate-grid`, batch 7

**Task:** "Graph y = x² − 4 as a dashed curve with arrows at both ends, and finish the right end of the x-axis with a circle."
(cap-03b a row's line, cap-03d its arrows, cap-03i set per row, cap-06c an axis end as a circle, cap-06d each end on its own.)

## Reference (mathfigures.com/coordinate-grid, headless, no login)

| # | action | shot |
|---|---|---|
| 1 | type `y = x^2 - 4` in Equation 1 | ref/01-type.png |
| 2 | click the row's style button (Customize equation 1) | |
| 3 | click Dashed under Line (Arrows is already Both ends: the reference's rows have arrows by default) | ref/02-dashed.png |
| 4 | open the x-axis section | |
| 5 | open the Right end chooser | ref/04-end-menu.png |
| 6 | click Circle | ref/05-circle.png |

## Ours before (main 47e760a)

The only way to a dashed curve was to type the style word: `y = x^2 - 4 dashed` (1 action,
ours-before/01-typed-dashed.png; that render is main's drawing, which the frozen-figures test pins byte for byte).
No row could have arrows, and the axes had one switch for all four ends ("Arrowheads on axes": triangles or nothing),
so the task stopped there: **not finished**.

## Ours after (figtest build under Xvfb, nothing seeded)

The alpha notice is closed with Escape first (the environment, not counted). Steps 1 to 4 at 1280x800; the window
is 1280x2000 for step 5 because the script hook cannot scroll the panel (scrolling is not counted).

| # | action | shot |
|---|---|---|
| 1 | click the equation box, type `y = x^2 - 4`, Enter (one typed run) | |
| 2 | click the row's line button (left of the box): colours, then Line, Thickness and Arrows | ours-after/01-type-and-open-menu.png |
| 3 | click Dashed | |
| 4 | click Both ends (the menu stays open between choices; a click outside closes it) | ours-after/02-dashed-both-ends.png |
| 5 | open the x-axis section | ours-after/03-open-x-axis.png |
| 6 | click Circle under Right end (each end's choices are one-click pills) | ours-after/04-circle-figure.png, 04-circle-panel.png |

## Result

| | reference | ours before | ours after |
|---|---:|---:|---:|
| actions to finish | 6 | stopped after 1 | 6 |
| finished? | yes | no: no row arrows, no per-end axis finish | yes |
| dead ends | 0 | 0 | 0 |

What differs in how:
- Same place for the row's style: a button beside each equation opens colour, line, thickness and arrows (ours for the
  first four rows, as with colour; a typed style word still works on any row).
- The reference's rows start with arrows at both ends; ours start with none (every grid saved before batch 7 must draw
  as it did), so arrows cost us one click the reference does not need, and its end chooser costs it one click we do
  not need (our ends are pills, its are a drop-down).
- Our axis-end circle is open (the app's own circle, as on the chemistry graphs); the reference's is a filled dot.
- The student copy (Show answers off) keeps the dashes, arrows and end marks: they are how the graph looks, not answers.

Verdict: **fewer-or-equal actions and finished.**
