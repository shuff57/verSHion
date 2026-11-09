# Interaction task: `two-way-table`, batch 3

## The task

"Make a two-way table of Year 10 and Year 11 by School lunch and Packed lunch (46, 34 / 38, 41)
with its totals, leave only Year 11's Packed lunch count blank for students, then show it as
row percents."

## Reference side (mathfigures.com/two-way-table, headless, no login)

| step | action | screenshot |
|---:|---|---|
| 1-4 | type School lunch and Packed lunch into the column headings, Year 10 and Year 11 into the row names (4 boxes) | `ref/01-type-names.png` |
| 5-8 | type the four counts (4 boxes) | `ref/02-type-counts.png` |
| 9 | Work out totals | `ref/03-work-out-totals.png` |
| 10 | empty Year 11, Packed lunch (the panel says it can be worked out: 41) | `ref/04-empty-cell.png` |
| 11 | In the cells: Relative frequencies | |
| 12-14 | open the Relative frequencies section, Each row, Percent | `ref/05-row-percents.png` |

The reference opens on an empty grid and its figure is the student copy (no key).

## Ours before (commit bd2ea88)

| step | action | screenshot |
|---:|---|---|
| 1-3 | type Row names, Column names, Counts (3 boxes) | `ours-before/01-type-table.png` |
| - | no way to blank one chosen box: Blank for students only picks cells along the diagonals (Some cells, 1 blanks Year 10 School lunch; 2 blanks that and Year 11 Packed lunch) | |
| - | no relative frequencies | |

## Ours after

| step | action | screenshot |
|---:|---|---|
| 1-3 | type Row names, Column names, Counts (3 boxes) | `ours-after/01-type-table.png` |
| 4-6 | open Student copy, Blank for students: Boxes I choose (a dropdown, 2 clicks) | |
| 7 | click Year 11's Packed lunch box on the table (the counts become `38, [41]`) | |
| 8 | Show answers off: the student copy, totals given, one box blank | `ours-after/02-blank-one-box-student.png` |
| 9-12 | open In the cells, In the cells: Relative frequency of each row (dropdown, 2 clicks), Written as: Percent | `ours-after/03-row-percents.png` |

The panel was scrolled once with the wheel to reach the cards below Counts (not counted). The
alpha notice's Got it is dismissed first on both of our runs (not counted).

Same numbers as the reference: 57.50%, 42.50%, 48.10%, blank, 52.83%, 47.17%, 100.00%
(34 / 80 = 0.425 rounds up to 42.50%, as on paper).

## Result

| | reference | ours before | ours after |
|---|---:|---:|---:|
| actions to finish | 14 | - | 12 |
| finished? | yes | no (stopped at blanking one chosen box) | yes |
| dead ends | 0 | 1 (Some cells blanks the wrong box) | 0 |

What differs in how:
- Input: the reference types into a spreadsheet-like grid, one box per action; ours types the whole
  table as text (one action per box of names or counts), and takes a spreadsheet paste with its
  headings and totals in the Counts box.
- Choosing the blank box: the reference empties the box in its grid (and then needs Work out
  totals first, or the totals are lost with it); ours clicks the box on the figure itself, which
  keeps its value for the key.
- Student copy: the reference's figure is the student copy and lists the missing values in the
  panel; ours has a key (Show answers on) with the blanks filled in bold, and a note only when a
  blank can't be worked out from what is shown.

Verdict: fewer-or-equal actions and finished.
