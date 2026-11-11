# Interaction task: `number-line`, batch 7

## The task

"Plot the sequence aₙ = nπ/4 for n = 1 to 6 on a number line from 0 to 2π counted by π/2, with the terms' values written
above the line."

Uses cap-01f (a sequence row), cap-05 (which terms: n from 1 to 6), cap-02c (values above the line) and cap-03d (the range
typed in π, the ticks numbered in π).

## Reference (mathfigures.com/number-line, headless, no login)

The reference's equation box is a math field. Driven by playwright it would not take a typed subscript: `a_n` became an
empty subscript box followed by `n` (an automation limit; a teacher at a keyboard types `a_n` and it works, as its own
help says). So its steps are counted from its controls, and its end state was opened through its own link parameters
(`?from=0&to=2pi&step=pi/2&eq=a_{n}=npi/4`), after which the last n was set in its box like a teacher would.

| # | action | shot |
|---|---|---|
| 0 | open the page (−2 < x ≤ 5 on −10 to 10) | ref/00-open.png |
| 1 | type `a_n = n pi/4` in Equation 1 (the n boxes 1 to 5 appear under the row) | |
| 2 | last n: 6 | |
| 3 | open the Line card | |
| 4 | From: 0 | |
| 5 | To: 2pi | |
| 6 | Count by: pi/2 | ref/05-final.png |

Values above the line are Write by default there, so no action. Result: dots at π/4 ... 3π/2; π/4, 3π/4 and 5π/4 written
above their dots as stacked fractions; the dots on numbered ticks (π/2, π, 3π/2) have no extra value; the ticks are
numbered 0, π/2, π, 3π/2, 2π.

## Ours before (number-line code as at main 47e760a; binary from the main checkout)

Same opening (ours-before/00-open.png). Typing `a_n = n pi/4` in the equation row drew nothing and said nothing
(ours-before/01-type-sequence.png): a dead end. The To box takes plain numbers only, so 2π could only be typed as
6.2832. Count by as a fraction `pi/2` numbered the ticks as halves of one, wrongly: 3/2 at π/2, 3 at π, 9/2 at 3π/2
(ours-before/05-pi-count-by-figure.png, the figure as drawn with those values and the terms typed as points). Not finished.

## Ours after (figtest build under Xvfb, 1280 x 1400 so the Line card fits; nothing seeded)

The alpha notice is closed with Escape first (the environment, not counted).

| # | action | shot |
|---|---|---|
| 0 | open (blank line) | ours-after/00-open.png |
| 1 | click the equation row and type `a_n = n pi/4`, Enter: five dots, a Graphs card appears | ours-after/01-type-sequence.png |
| 2 | open the Line card | |
| 3 | From: 0 | |
| 4 | To, typed: `2pi` | |
| 5 | Count by as a fraction: `pi/2` (ticks now 0, π/2, π, 3π/2, 2π) | ours-after/05-range-in-pi.png |
| 6 | close the Line card | |
| 7 | open the Graphs card | ours-after/07-graphs-card.png |
| 8 | First graph: values above the line, Write | |
| 9 | First graph: last n, 6 | ours-after/09-final.png |

Result: the same figure as the reference: six dots, π/4, 3π/4, 5π/4 written over theirs, nothing extra over π/2, π and
3π/2, ticks numbered in π.

## Result

| | reference | ours before | ours after |
|---|---:|---:|---:|
| actions to finish | 6 | - | 9 |
| finished? | yes | no: the sequence draws nothing, π ticks numbered as 3/2, 3, 9/2 | yes |
| dead ends | 0 | 1 (the sequence row) | 0 |

How it differs:
- Sequence: both type it in the equation row. The reference puts the n boxes right under the row; ours puts first n and
  last n in the Graphs card (one more card to open, and closing the long Line card first).
- Values above the line: the reference writes them by default; ours defaults to None, because an old figure with an
  inequality ending between numbered ticks must draw as it did (its saved spec and share link carry no such key, and the
  share-link version is shared by every figure). One more action.
- Range: the reference has one From / To / Count by box each, taking π. Ours keeps its number boxes and adds From, typed
  and To, typed beside Count by as a fraction (the existing number boxes are shared app code that reads plain numbers).
  Same number of actions, but two boxes per end.

Verdict: finished, more actions (9 against 6): the values default (kept for old figures) and the Graphs card being
separate from the row.
