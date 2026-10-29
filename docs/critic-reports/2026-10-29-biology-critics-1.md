# Biology batch critic, 2026-10-29

One combined accuracy + teacher + regression critic over `hardy-weinberg` (`bd58e90`), `chi-square-fit` (`d30ece5`)
and `enzyme-kinetics` (`4f89e17`), read-only, in its own `git archive` copy of `4f89e17`. Verdict: **FAIL — 2
blockers, 9 majors, 13 minors.**

The arithmetic was right in all three: the critic recomputed Hardy-Weinberg's p²/2pq/q² and p′ = p/(1−q²) over six
generations and its counts at awkward population sizes, chi-square's expected counts, statistic and df against the
3.841 / 7.815 / 11.070 tables and Mendel's 0.4700 with p = 0.9254, and enzyme kinetics against v = Vmax[S]/(Km+[S])
with both inhibition modes. Student copies leaked nothing; Print and the 66 % bitonal copies survived. Every failure
was in what the figures did with *awkward input*, which is the same pattern as every previous round.

## Blockers

| # | Finding | State |
|---|---------|-------|
| B1 | A zero ratio part with a zero count stayed in the test: `obs 10, 0` against `ratio 1, 0` printed "df = 1 (2 categories − 1)", "p = 1.00" and "do not reject … consistent with the ratio" — a confident pass on a test that never ran. | **fixed**: a category with no expected count and no observed count carries no degree of freedom and is dropped; with one category left there is no test and the decision says so. |
| B2 | A zero expected count with something observed printed a finite statistic: `obs 10, 5` against `ratio 1, 0` printed "χ² = 1.67" although the true statistic is infinite. | **fixed**: the statistic is shown as ∞, the p-value as "—", and the decision explains that a category the ratio says cannot occur has counts in it. |

## Majors

| # | Finding | State |
|---|---------|-------|
| M1 | The enzyme figure never drew the uninhibited curve, so the comparison its caption promised was not on the page; noncompetitive squeezed its curve into the bottom sixth and the labels overprinted. | **fixed**: the uninhibited curve is drawn dashed underneath, which both shows the comparison and fills the space. |
| M2 | The marked point's rate label was always end-anchored, so at low [S] it ran off the left canvas. | **fixed**: start-anchored unless that would cross the right edge. |
| M3 | The givens line was clipped at Large with an inhibitor (fixed 620 canvas). | **fixed**: the canvas grows with the label size, the line is shorter and is wrapped. |
| M4 | A long category name printed through the three number columns. | **fixed**: names are shortened with an ellipsis to the column width. |
| M5 | Huge counts mixed "1 × 10^12" with "999999999999.56" and overran the columns. | **fixed**: one compact format for the whole table when the counts are large. |
| M6 | Non-numeric tokens were dropped silently, quietly changing the test. | **fixed**: the ignored tokens are named in the Notes. |
| M7 | The printed contributions did not add up to the printed total (0.64+0.07+0.07+0.64 against "1.43"). | **fixed**: the total is the sum of the printed contributions, so the column adds up. |
| M8 | Empty input drew a table of zeros and "df = 0 (0 categories − 1)". | **fixed**: it says what is missing. |
| M9 | At one decimal place Hardy-Weinberg printed "0.4 + 0.5 + 0.2 = 1.1" beside a check that said 1. | **fixed**: the fractions are rounded as a set that adds to exactly 1, at a precision raised until no real frequency prints as zero. |

## Minors

Recorded, not fixed: the Hardy-Weinberg student copy's empty bars have no divisions, so drawing p²/2pq/q² is
guesswork; the enzyme axis is unitless; the cladogram has no branch lengths.

## Regression

- Census: exactly 6 ids added (three figures × defaults/start), none removed, none of the 158 pre-existing ids
  changed at the time of the critic.
- Tests: `#[test]` 1826 → 1857 (+31), nothing deleted or weakened.
- Full suite run alone: 1851 passed, 0 failed, 5 ignored.

## What was fixed after it

Everything above, plus the four new tests it implied: a category the ratio cannot produce is not a pass; a category
expected to be empty but counted makes the statistic infinite; the printed contributions add up to the printed
total; and a long name or huge counts do not overprint the columns. Census after the fixes: `chi-square-fit` and
`enzyme-kinetics` re-hashed (their default renders changed), `cladogram` added.
