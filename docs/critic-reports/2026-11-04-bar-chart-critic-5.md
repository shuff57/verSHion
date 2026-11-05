# Bar chart, critic 5, 374b712 (`figures-gapfill`, frozen at /tmp/figures-crit/bar5)

Verdict: **FAIL** — 0 blocker, 0 major, 1 MED, 5 LOW, 4 minor. External probe (scratchpad/probe) over `render`/`notes_for`/`answer_pair` plus all 38 bar tests; no image input.

## BLOCKER
None. 3,000 random + 2,000 structured tables × options: no panic; max canvas 1210×1577; 38/38 bar tests pass (1 ignored).

## MAJOR
None. The three quote cases keep their counts and names, draw with no note; applying every “change line” rewrite over 40+ refusal/note states found none that reduces a line's typed counts.

## MED
M1. The heading classifier still swallows a real first data row when a mistyped count carries a word and its numeric token is not plain ASCII digits. `A, l23 students\nB, 5` (also `A, 1O students`, `A, — students`, `A, several`, `A, lots`) → “Line 1 … was read as headings”, one bar (B, 5); the A row and its observation are gone, only the editor note says so. The FAQ promises “a count … with a word after it (12 students) is refused with the line named”; it is not. The test only covers `N students`, where line 2 is also unreadable, so it never notices line 1 was eaten.

## LOW
L1. Paired literal quotes are silently deleted from names: `5'2" - 5'4", 6` then `5'4" - 5'6", 9` draws the labels `5'2 - 5'4` and `5'4 - 5'6` (counts 6/9, no note); `a"b"c` → `abc`. Counts survive; labels do not.
L2. A merged line whose count is an ambiguous thousands group gets a broken fix: `A, 1200` then `A, 3,400` offers `change line 2 to “, 3400”`; applying it yields “Line 2 has a count but no category name”. The second note (“A 2, 3400”) works; the first does not, contradicting “a fix that works when applied”.
L3. The >2 message still normalises counts: `A, 1 200, 3, 4` quotes “1,200” (typed “1 200”); plain `1200` also becomes “1,200”. “Quotes the count as typed” holds only for the comma spelling tested.
L4. The too-large unit is unpinned: a mutant that always advises “thousands” survives the bar suite (M10), though the code correctly picks millions for 1e15.
L5. `Year, 2019, 2020` is still drawn as a category with bars 2,019/2,020; the note offers only “delete line 1 and type the group names”, no route that keeps it (carried from critic 4).

## minor
- `Way, "Boys, grade 6", "Girls, grade 7"` is refused: a quoted heading cell with a comma is not a name, though quoted names are the figure's own convention.
- `A, 3"` silently reads as count 3 (the quote is trimmed from a count cell).
- `<title>/<desc>` say “Bar Chart”, not the typed title; the student table's first column is always “Category”; “Other” is sorted by size in Pareto, not held last.
- Print's two groups differ sharply in weight: solid #7a beside a 1 px hatch (18–25 % ink at 58–75 %); survives, but group 2 reads much lighter.

## verified
- Print: #8c 3 px grid, #7a bars. 1× render + Lanczos at 58/66/75 %, 4/10/30 categories × Small/Medium/Large: every gridline min 117–144, 0 dead; bars are drawn after the grid, so no gridline crosses a bar interior (sample at a gridline's y inside a bar is 122); 30-cat bars 18.3 px; no hue. Hatch bars 18–25 % ink. The 18/255 tone gap is enough only because the 1.4 px black outline separates the bar edge; it does.
- Heading: `A, —`, full-width `A, １２`, `A, N/A`, `Smith; Jones, 3`, `Grade, 1st, 2nd`, `Quarter, Q1, Q2`, `Illness, COVID-19, Flu`, `Grade, K, 1` all refused at line 1 with the bad cell named and both routes, no rewrite; `A, B1`/`A, 5B` refused as not a number; `Boys and girls` headings, `Grade 9`, `9th grade`, `None` accepted; line 1 headings + unreadable line 2 refuses line 2.
- Quotes: `Don"t know, 5`, `55" TV, 12`, `a"b, 3` names/counts exact, no note; `"Train, fast", 2` exact.
- Numbers: `1,200` / `1 200` / `12 345 678` round-trip through labels; 3/19→16 %, 1/8→13 %; table cells = bar labels = question 3's answer (44.0 %/28.6 %), Total = cell sum (100.0/99.9), question 2 25/28, question 1 tie Cat/None; Pareto ends at 100 % at the top with a 0–100 right axis; gaps 0.250/0.400/0.600; sorting tests pass.
- Student copy: 960 state pairs, same size, key minus exactly the answers (test passes; M12 killed); census `bar-chart__defaults aee0af800ed41b11`, `__start 208a340577ea2008` match the fixture.

## mutants
17 valid, 16 killed, 1 survivor: **M10 too-large always “thousands”**. Killed: quote branch; grid 3→2.4 px; grid #8c→#cccccc; bar #7a→#d9d9d9; heading count_like off; slash allowed; >2 rewrite; mismatch “, 0”; Wide 0.6→0.9; half-up→floor; student ticks; MAX_CATS 31; Pareto 25 % ticks; ambiguity note off; >2 digits normalised; thousands not joined. (A float-percent-order mutant was neutralised by quantising and is not counted.)

## not checked
Visual/PNG appearance (no image input: I measured SVG boxes and pixels, I did not see a rendered figure); CJK/RTL live rendering; PDF/DOCX export; other figures' census ids and the rest of the lib suite.

## biggest gap
Still the key: a frequency/relative-frequency table beside the graph is reachable only by setting “Student copy leaves out” to “The bars”; the default key is graph-only, and the table's first column says “Category” whatever the axis.
