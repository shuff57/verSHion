# Round 9, r1 — did the first fixes help?

What changed (commit `e2ede29`, live as `vershion-web-33e82b49c88e7d00`): welcome's dead band split
between a lead space above the hero and a capped gap before "How it works"; a footer rule holding
the floor on settings and account; a stated reason beside the disabled Save; one height for the
three feature cards.

Method: a *steer* panel — three critics (deepseek, glm, kimi), the same critic prompt, and a blind
set of **before/after pairs** (the r0 frames against the fixed frames, same build otherwise), plus
two same-image controls and three swapped-side mirrors. 11 items, 33 verdicts.

## Honesty

- Controls: 6 of 6 tied, **0.00 error** on every judge.
- Mirrors: **8 of 9** agreed under a side swap (glm flipped once on welcome desktop;
  deepseek 3/3, kimi 3/3).

A first attempt with one codex judge (`gpt-6-luna`) is discarded: it flipped **0 of 3** mirrors and
its scores swapped sides exactly, i.e. a left-side bias, not a preference. Its 5-of-6 "after wins"
was therefore not evidence.

## Result

| screen | view | after | before | delta |
|---|---|---|---|---|
| account | desktop | 6.92 | 6.25 | **+0.67** |
| account | narrow | 6.50 | 6.50 | 0.00 |
| settings | desktop | 6.75 | 6.33 | **+0.42** |
| settings | narrow | 6.75 | 6.25 | **+0.50** |
| welcome | desktop | 6.58 | 6.17 | **+0.42** |
| welcome | narrow | 6.83 | 6.50 | **+0.33** |

- Pair votes: **16 for the fixed build, 2 for the old one**; all six screens preferred after.
- Mean delta **+0.39**.
- The one screen the single luna judge called worse (settings narrow, −1.50) is **+0.50** here, with
  none of the three judges preferring the old build.

## What this does and does not say

- It says the four fixes are a real, if modest, improvement: ~0.4 on a 10-point scale, consistent in
  direction across three judges and both widths.
- It does **not** say the screens pass. On this panel the fixed screens still sit at 6.5–6.9, and the
  bar is 8.0 with no dimension below 7.0. The spacing floor is still the thing to break.
- It is a steer round: means are not comparable with the five-judge r0 baseline, and this round may
  not claim the bar.
- `figure-editor` was not touched in this batch and was not re-scored; the alpha-dialog slicing on
  home-empty is likewise untouched.
