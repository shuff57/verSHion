# Interaction task: `ph-reading`, batch 7 (readings track)

## The task

"An analog pH meter at a random pH, the magnifier in place of the dial, with the answer key."

## Reference (https://www.chemistryfigures.com/ph-reading, headless, no login)

| # | action | shot |
|---|---|---|
| 0 | opening state | ref/00-open.png |
| 1 | Instrument: Analog meter (its magnifier comes on beside the dial) | ref/01-analog.png |
| 2 | Random (beside the pH box, already open) | ref/02-random.png |
| 3 | open the Magnifier section | |
| 4 | Magnifier only | ref/03-magnifier-only.png |
| 5 | open Title and answer key | |
| 6 | tick Answer key | ref/04-answer-key.png |

## Ours before (main at 47e760a)

The opening state is the same as ours-after/00-open.png (batch 7 does not change it). Choosing Analog meter worked, but the pH
section had a typed box with no Random button, and the analog meter had no magnifier. The task could not be finished; it stopped
at step 2.

## Ours after (figtest build under Xvfb, 1280x800, nothing seeded)

The alpha notice is closed with Escape first, and the panel is scrolled with the wheel twice. Neither is counted as an action.

| # | action | shot |
|---|---|---|
| 0 | opening state | ours-after/00-open.png |
| 1 | Instrument: Analog meter | ours-after/01-analog.png |
| 2 | open the pH section | |
| 3 | Random (the needle moves; the box shows the picked pH, here 6.34) | ours-after/03-random.png |
| 4 | open the Magnifier section | |
| 5 | Magnifier only | ours-after/05-magnifier-only.png |
| 6 | open More options | |
| 7 | tick Answer key ("pH: 13.29" under the lens; each run picks its own random pH) | ours-after/07-answer-key.png |

## Result

| | reference | ours before | ours after |
|---|---:|---:|---:|
| actions to finish | 6 | - | 7 |
| finished? | yes | no: no Random, no magnifier | yes |
| dead ends | 0 | 0 | 0 |

How the two differ:
- Opening the pH section is one extra action on ours. The reference opens that section once Analog meter is picked.
- On the reference, picking Analog meter turns the magnifier on beside the dial. Ours keeps the meter alone until the teacher asks
  for the lens, so old figures and links draw as before.
- The reference's answer key is under "Title and answer key". Ours is under "More options" (that layout is older than this batch).
- The reference lets you choose how many numbered marks the lens shows (1 to 6). Ours shows a fixed window of about three
  numbers.
- On both, the picked pH is a value with 2 places and the key prints it. On ours the value is stored, so a saved figure or a
  share link redraws the same pH.

Verdict: finished, more actions: one extra click to open the pH section.
