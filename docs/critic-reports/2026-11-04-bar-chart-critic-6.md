# Bar chart, critic 6, d026eec (`figures-gapfill`, frozen at /tmp/figures-crit/bar6)

Verdict: **FAIL** — 0 blocker, 0 major, 2 MED, 5 LOW, 4 minor. 42/42 bar tests and `census_baseline` pass at this commit; external probe over the public `svg`/`notes_for`/`answer_pair`, plus 4,000 fuzz renders, 900 percent comparisons and 13 mutants; no image input.

## BLOCKER
None. 4,000 random tables × random options: no panic, no NaN/Inf in any attribute, 0.22 s for the sweep; refusals draw the note and never a chart; the 1,440-render collision audit passes.

## MAJOR
None.

## MED
M1. The round-5 first-line fix is still incomplete for the comma-free form the figure accepts. With no tab/comma/semicolon the line is one cell, `cells[1..]` is empty and `name_cell` is never applied, so a data row with a bad count becomes the headings and leaves the chart: `Walk 12 students\nCar, 9` (also `Walk l23`, `Walk 1O`, `Walk twelve`, `Walk dozen`) → note only “Line 1 (“Walk 12 students”) was read as headings, not as a category.”, one bar (Car, 9). The count 12 and the name are gone from the printed figure, though the FAQ promises “a count … with a word after it (12 students) is refused with the line named”. The comma form (`A, l23 students`) is refused, but there the first route (“type the group names in Group 1 name and Group 2 name”) names controls `relevant()` hides whenever the data does not parse, so only one route exists.
M2. A quoted name holding a semicolon loses the count and gets a fix that invents one and never settles. `A, 5\n"a;b", 3` → “Line 2 (“"a;b", 3”) has no count … change line 2 to “"'a;b', 3", 0”.” A `;` anywhere makes `;` the only separator, so the comma after the quoted cell is not one and the whole line becomes the name; the rewrite appends `, 0` and re-quotes. Applied three times it re-refuses each time with another `, 0` and a longer name (verified); same for `"a;b", 3, 4`. This breaks the rule the suite pins elsewhere: no refusal may invent a count, and its fix must work when applied.

## LOW
L1. A thousands group inside a name is joined with no note: `Bus 12,300 riders, 4` draws the name “Bus 12300 riders” (count 4 intact); `join_thousands` runs over the whole line before splitting.
L2. A note's fix renames the category: `Don"t know, 1,200` → “change line 1 to “Don't know, 1200”” (double quote replaced by an apostrophe); the merge note does the same (“Don't know 2”).
L3. A merge fix can merge: a 40-character name typed twice plus an existing 39-character name equal to the truncated fresh name → the fix “to keep them apart” adds line 2 to that other category (labels 8/7 become 5/10).
L4. `Grade, 1st, 2nd` (a headings line) is refused, the message calling `“1st” … neither a number I can read nor a clear group name`, though the round-5 rule says a number with an ordinal suffix is a group name; `Grade, K, 1` blames K, which is a fine name.
L5. `Year, 2019, 2020` is still a category with bars 2,019 and 2,020; the editor-only note offers no route that keeps the line (carried).

## minor
- The percent axis stops at the tallest bar's nice step (0–20 % for five equal categories, 0–40 % for three), not 100 %, unless Pareto.
- Quoted heading cells with a comma (`"Boys, grade 6", "Girls, grade 7"`) are refused although quoted names are the data convention (carried).
- `Walk, 1,200 / Car, 2,500,000 / Bus, 3,000` notes “Lines 1 and 3 give a count in thousands” though line 2 does too (its other reading has four cells, so it is not listed).
- Print's two groups differ 4× in weight: solid #7a is 100 % ink, the 1 px hatch 24–25 % at 66 %; the 1.4 px black outline keeps it legible.

## verified
- The five typo cells (`l23 students`, `1O students`, `— students`, `several`, `lots`) are refused at line 1 with no chart; `A, 12 students` is data with the fix “A, 12”; `Grade, 9th grade, 10th grade` still reads as headings.
- Quotes are kept in the drawn name (`5'2" - 5'4"`, `a"b"c`); the merged ambiguous fix keeps its name (“A, 3400”); `1 200` is quoted as typed in the >2 message; the too-large unit is right (5e9→thousands, 1e15→millions, 1e17→billions, 1e22→limit).
- Percents: 900 table/decimals comparisons against an independent half-up give 0 mismatches; table cell = bar label = answer; Total row = the cells' sum (25/28, 100.0/99.9); 1/8→13 %, 2/3→67 %.
- Pareto: points are cum/total, the line ends on the axis top, the right axis is 0–100 % at the exact percents, the student copy has no line; gaps 0.250/0.400/0.600; ties keep typed order.
- Thousands round-trip (“1,200”, “2,500,000”); 12 student/key combinations show the student text = key text less exactly the values/table cells/ticks/answers plus three “Answer:” placeholders; Print has no non-grey fill or stroke (defs included) and the 66 % grid survives.
- Fuzz: 4,000 renders, no panic, 0 NaN/Inf attributes, all notes end in a period; census `bar-chart__defaults aee0af800ed41b11` and `__start 208a340577ea2008` match.

## mutants
13 run, 11 killed (M1 label_token, M2 “wordy” off — killed only by the broader first-line test, M3 quotes, M5 >2 typed quote, M6 thousands-only unit, M7 floor, M8 forced 100 % total, M9 grid 2.4 px, M10 MAX_CATS 31, M11 student ticks, M13 combine largest). Survivors, each re-run against two targeted tests that both pass: **M4** a fix that keeps a name's `"` (no test covers a quote inside a fix — the gap behind L2); **M12** Q3 asks the first typed category instead of the first drawn (no test runs Questions with a sorted order; today's code is correct).

## not checked
No image input: I measured SVG geometry and pixels but did not look at any PNG; no live egui editor, browser/wasm fonts, CJK/RTL in resvg, PDF/DOCX export, or the other 2,676 lib tests.

## biggest gap
The figure still cannot tell a one-cell title line from a comma-free data row, so a bad count in the style it also accepts silently costs a category; the default key still has no frequency table beside the graph (carried).
