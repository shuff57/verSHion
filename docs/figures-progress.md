# Figures gap-fill progress

## Phase 1.1: conics and implicit curves on coordinate-grid

Equations in x and y that are not straight lines (circles, ellipses, hyperbolas, x = y^2, xy = 4, chained and system inequalities) are
sampled by marching squares and drawn as smooth polylines, with a single tint (a hatch in Print) for the common solution of several
inequalities, dashes only for excluded boundaries, asymptotes recognised by geometry and drawn fine and dotted, curve labels placed
with leaders, tick numbers on their own ticks, and editor-side notes for rows that cannot be drawn as typed. Print is built for
photocopies (dark grid at 1.4 px, hatch survives a bitonal copy, rows at least 1.8 times the grid). Wide windows thin their numbers and
grid. Existing census files for every other figure, and for the older coordinate-grid cases, are byte-identical.

## Known limits

- A tick number beside a curve stays on its tick and is drawn over the curve with a thin 2 px white halo (`data-plate="over"`).
- tan(x) at +-1000 has a period under 2 px and is aliased by any sampler; it is checked only to +-200.
- A wide window with Square cells on is only warned about, not relaxed.
- Plain Colour windows (no conic, not Print) keep the number placement of the earlier figures, so the origin 0 can sit 2 px from a
  neighbouring -1; changing it would alter existing census files. Colour/Print and conic windows resolve every collision.
- The Colour grid is near-black for a plain figure and a grey-blue beside a conic, for the same census reason.
- Thinning of tick numbers follows the spacing rule under "Tick numbers".

## Tick numbers

In a figure with a conic or implicit row, or in Print:

- Every number sits at its own tick in the standard row (x numbers under the x-axis, y numbers left of the y-axis). Nothing is moved
  sideways, flipped across an axis, or led back to its tick; there are no leader lines for numbers.
- A number whose box touches a curve gets a thin 2 px white halo (`data-plate="over"`), so the curve is only interrupted by the halo.
  No white plate covers a curve. Numbers clear of curves keep the white plate under the curves that clears the gridlines.
- Thinning comes from measured spacing: an axis shows every Nth number (N from 2, 5, 10, 20 ..., the same either side of 0, each axis
  on its own) when two neighbouring text boxes (measured widths, minus signs included) would be less than 1 px apart; the y axis when
  rows are under 16 px apart. The tests use zero overlap tolerance and the same width estimator as the code.
- An x number whose box meets a y number (the x-row / y-column corner) drops 13 to 26 px below its row, centred on its tick, to the
  first spot clear of the y numbers. That only works when the y rows are at least about 21 px apart. Where they are closer, nothing
  fits between the y numbers and the x axis is thinned by the same step list.
- The editor notes "Some axis numbers were thinned ..." exactly when an axis shows fewer numbers than the step asked for.
- Origin zero: below left of the origin, else below right, left out only if neither fits.
- Major gridlines closer than 6 px are drawn every Nth line (N divides the number stride, so every numbered tick has a line); minor
  lines under 4 px are left out. This is for Print and conic figures only.

### Tall windows

At x +-5 or +-8 with y +-15 and Square cells on, the y rows are 16 px apart, so the x row is thinned: x +-5 shows -4, -2, 0, 2, 4 and
x +-8 shows -8, -6, -4, -2, 0, 2, 4, 6, 8 (the real renders). A circle of radius 5 therefore has no x-intercept label at x +-5
(the gridline and the curve edge still mark the point) and the editor note says numbers were thinned. An audit of window and conic
combinations found 161 of 3468 that lose an intercept label, all in Colour and none in Print.

## Phase 1.1 final state

- Plain Colour figures (no conic row, not Print) are identical to 0c1a8a8 (before the conic work) up to windows of roughly 12 by 12
  and in every census window. In wider windows (about +-30 and up, also 30 by 5 and 5 by 30 at "Every line" or "Every 2nd line") tick
  numbers that would literally touch are thinned, because the old output printed overlapping labels (for example -999, -998 0.24 px
  apart); +-30 keeps 25 of 121 numbers. +-1000 with minor lines stops at the 2100-line safety cap. Saved links still load; only the
  number thinning differs, and it is an improvement over overlapping labels. Gridlines of a plain Colour figure are otherwise unchanged.
- Print and conic figures: own-tick numbers with a halo over curves, spacing-only thinning, corner x numbers dropped below the row,
  coarser grid under 6 px per line.
- The `data-axis` attribute on tick numbers appears in the exported SVG (harmless).

## Known limits (summary)

- Corner x numbers drop below the row only where the y rows are at least about 21 px apart; otherwise the x axis is thinned.
- The hairline Print grid breaks up at 50 to 58% bitonal (the Bold option is the remedy); asymptotes and curves survive.
- +-50 and +-1000 grids are dense (every Nth line, 6 px minimum).
- Only 4 curves are drawn per figure.
- An equation label can sit over x numbers in very wide windows, and over the y numbers in tall narrow ones.
- Digit boxes are estimated from character widths, not measured from the font.

## Leftover minors

- Only 4 curves are drawn per figure (IMPLICIT_MAX); the cap was not raised because existing tests assert the "Only 4 curves" warning
  and each curve costs a full sample.
- Numbers on a tick that sits on a curve (x = 1 on the unit circle) are drawn over it with a thin halo; they are not moved outside a small circle.
- Numbers in a very tall or wide window are thinned evenly (note in the editor); only the origin 0 can still be left out alone.
- Print asymptotes are 1.6 px, dash 7 5, #666666; they survive a 58 to 75% bitonal copy. The thin default Print grid drops out below
  66% bitonal; the Bold grid option is the remedy.
- An equation label can sit over an x number in a very wide window.
- The annulus inner dashed circle blurs into the hatch at 50 to 58% bitonal.
- Equal-scale (Square) wide windows are only warned about.
- Strict double-root inequalities (y^2 > 0, abs(x) > 0, (y-1)^2 > 0) tint the whole window without the dashed excluded line or an axis
  cut, and `x^2 > 0; y^2 > 0` says "No part of this curve is inside the window" while still tinting.
- (x-y)^2 = 0 and x^2 + 2xy + y^2 = 0 stop about 1.2 px short of the window edge.
- Notes written on the figure use a different font from the figure's numbers.
- A Print-only default for equation labels when several curves are drawn was not added (rows differ by weight and tone only).
- Plain Colour windows (no conic, not Print) use the earlier number places, so an origin 0 can touch a neighbour there in some windows.
- In a tall narrow window the equation label can sit over the y numbers.

## Phase 1.2: circle

New module `src/figures/circle.rs` (id `circle`, subject Math), drawn with `svg::Doc` at a fixed size (the radius is a number to read, not a scale). Toggles: centre, radius, diameter, chord, perpendicular bisector, tangent at A (with the right angle), secant, central angle, inscribed angle, minor/major arc, minor/major sector (each follows its own choice; arc and sector are independent since round 2). Points A, B and C sit at angles in degrees and are draggable handles (reached through `math::handles`, so the circle is not in `NO_HANDLES`); a handle exists only for a point that is drawn. Dragging A moves by whole degrees (so 28 to 32 are reachable); dragging B moves the central angle (the gap from A), which snaps to 30/45/60/90/... within 3 degrees; typed values are exact. The central angle can also be set by number (`Set the central angle by number` and `Central angle`, B = A + that; off by default, so existing figures are unchanged); over 180 degrees it is a reflex angle when the arc or sector is Major, and the readout then shows the reflex value. The four point letters and the midpoint are text params (3 characters, never empty), and the control labels, rows, labels and notes use them.

Given / Ask / Hidden: each of radius, diameter, circumference, area, central angle, arc length, sector area, chord, OM (distance from the centre to the chord, r cos(theta/2)), AM (half chord, r sin(theta/2)), inscribed angle and the tangent-radius angle has a state in the Measures section. Given is printed on the figure and on the student copy, Ask is a blank (`_____`, wide enough for `000` and the degree sign, one style for the figure and the card) on the student copy and the value on the key (through `answers_param()`), Hidden is not shown. Defaults: radius and central angle Given, OM, AM and the tangent-radius angle Hidden, the rest Ask (the readout rows are separate toggles). A Given value that is neither listed nor drawn is not shown on any sheet: the Notes box says so when a value asked for needs it. A check (`sheet_values`) finds the Given and Ask values on a sheet; a test asserts every shipped example's student sheet shows at least one Given value whenever it asks for something. The card shows `lhs = value` (an angle row shows just the value, its name says which angle). `Show formula hints` (off by default; saved links from before it lose the formula text that round 1 printed on the key) adds the formula: degree form (`s = theta/360 x 2 pi r`, `A = theta/360 x pi r^2`) or radian form (`s = r theta`, `A = 1/2 r^2 theta`), and `half the central angle` for the inscribed angle. The key also gets a worked `Check` line (`OM^2 + AM^2 = r^2`) when OM is listed. Right-angle marks (tangent, chord midpoint, angle in a semicircle) are Show always / Key only / Hide; with Key only the student copy can still ask for the angle through the tangent-radius angle (a blank at the contact point; the key shows 90).

Layout: labels are placed by a candidate search (8 directions and the four corners, three distances each, then rings with a leader line). A spot that touches another label, a point, a line of the drawing, the readout or the edge is rejected; in Print the sector's hatch region is avoided too (a label that can only go on it gets a white plate, never over a curve); a leader line is drawn when a label sits more than 14 px from what it names, and a spot whose leader would cross no drawn line (the circle's own outline excepted) is preferred. A small angle (under 25 degrees) puts its angle, chord and midpoint labels outside the circle with leaders; M is dropped when it lies on O (AB a diameter). The readout is laid out from measured text widths (`tw`, the same estimator in code and tests): the label column is the widest label, long rows go on two lines, and the canvas grows in width and height (Large also widens it), so Small, Medium and Large never overlap. The label size is applied inside the module (not by `with_label_size`), so the geometry is unchanged. Numbers are grouped in thousands from four digits up to 1e12 (then `m x 10^n`, with the rounding carried into the exponent) and honour the decimal places.

Print: exactly neutral (`#111111` ink, greys only; the test uses spread 0), a hatch fill (only the patterns in use are defined), the circle at 2.6 px, inscribed lines, angle arcs and right-angle marks at 2 px, auxiliary lines (bisector, secant extensions) at 1.6 px dash `7 5` `#666666`, the readout rule `#888888`. The bitonal check is automated at 66% (2x render; the outline, chord, dashed bisector and the readout rule, found from the layout, must stay dark) and `circle_pngs` also writes `-print-bw66.png` for every example for a manual look. Census: new `circle__*` lines only, no existing line changed (`svg::esc` now drops characters XML 1.0 cannot carry and `Kind::Text` params are cut at 500 characters, which changes no other figure). Editor notes (the Notes box, gated to `circle` in `figuresui.rs` through `figures::notes_for`): radius out of range (a loaded link is clamped first, so `params_from_json` leaves a `clamped_radius` marker; other loaders that clamp without it cannot show the note), A and B in the same place, C on A or B, the inscribed row without C, two points with the same letter, a Given value that is not shown. Examples: ten (radius and diameter; chord and bisector with r and AB given and OM asked; tangent with the angle asked and the marks Key only; central vs inscribed; arc and sector; radians; tangent and secant at A; semicircle with the radius given; chord 8 and OM from r = 5; OM = 3, find the chord). FAQ arm `CIRCLE` has six questions.

Round 4: `Define the chord by` (Points, Angle, Chord length, OM distance) sets B from a typed chord (theta = 2 asin(c/2r), clamped to 0 < c <= 2r) or OM (theta = 2 acos(d/r), 0 <= d < r), with a note when clamped; the defaults (Points) leave every existing render unchanged, and dragging B sets the chosen length. Presets: `Chord 8 and OM from r = 5` (3-4-5) and `OM = 3: find the chord`; tests include 5-12-13. Blanks and answers are measured at the wider of the two forms, so the key's value lands where the blank was, and the inscribed and tangent-angle labels always carry a short leader (a narrow angle's leader starts on one of its two lines, so it crosses no line). Leaders: a leader is used only for a spot more than 18 px from what it names (or a forced one); a spot needing none beats one that does, and one that crosses no drawn line beats one that does (the circle's outline excepted, a label outside has to cross it); the default figure has no leader, and a test asserts no leader crosses a drawn line in every example, the 5/10/20/30 degree cases, a renamed case and the coordinate-label cases (see the Round 5 limits for what is not guaranteed). Fitting: if a label would overlap another or leave the canvas the figure is rebuilt without the unit on the figure labels (the readout keeps it; a figure whose readout rows are off loses the unit that way), then with up to 150 px of room added on the left, right and top (the drawing is shifted and handles follow), and as a last resort the labels that still do not fit are listed in the readout. A randomized test (2200 layouts, long units, radius up to 10000, all sizes, all states) asserts zero label-label overlaps and zero labels outside the canvas; no more than 30% of them may need the readout. Plates never cover any drawn line. Mixed reflex: a typed angle over 180 is shown as reflex only when every arc/sector chosen is Major; otherwise it is shown as the smaller one with a note, and chord, OM and AM rows say `minor angle` when the central row shows the reflex one. Notes now cover a Given value that is not shown for every value (circumference, area, diameter, chord, arc, sector, OM, AM; a Given chord is no size source when the radius is hidden), the answer-leaking right-angle mark with the tangent angle asked, same-letter names including M. The out-of-range-radius marker (`clamped_radius`) holds the value the radius was clamped to, so it stops once the radius is edited; `params_from_json` and the saved-link loader (through `migrate`) both set it, with a circle-specific branch in shared code (a generic marker was not worth the churn). Units over 20 characters are cut at 20; text controls over 500 characters are cut at 500 (shorter text is untouched). The exponent form honours 0 decimals; the OM check line says `(rounded)`. An automated test renders Print at 2x and checks the outline, chord, dashed bisector and readout rule at a 66% bitonal threshold.

Known limits: at Large with every part on, leaders to labels moved outside run long (they cross no line, but pass over the figure); at 5 and 10 degrees A, B and M still crowd the circle and rely on leaders; with a very long unit and everything on, figure labels may sit on a tangent or secant line (never on another label); the text-width estimate is generous but is not the real font.

Not in this figure (follow-up figure `circle theorems`): an external point P with two tangents (equal tangent lengths), tangent-secant (the power of a point), a second inscribed point C2 on the same arc (angles on the same arc), the cyclic quadrilateral (opposite angles), the alternate segment theorem, equal chords and their distances from the centre, and intersecting chords.

Round 5: (1) A reflex central angle no longer leaks on the student copy: the Chord, OM and AM row names say `(minor angle X)` only when the central angle is Given (always on the key); a sweep over theta 181 to 359, every Arc and Sector choice and Central Given, Ask and Hidden checks the student text has no `minor angle` and no number derived from 360 minus theta unless Central is Given. (2) Angle blanks carry a unit (`_____°`, or `_____ rad`); the tangent row is `Angle between tangent and radius OA` and the tangent example turns it on; a figure label for the central, inscribed or tangent angle names itself (`∠ACB = _____°`, `angle at A = _____°`) when its row is off. A seeded sweep asserts every blank is named by a label or by its row and every angle blank has its unit. (3) The tangent line and the secant extensions are solid obstacles at every fitting level; a label that would sit on one moves into the readout at the last resort (asserted in the randomized layout test). Area and sector units are not squared twice (a unit starting with `square` or `sq`, ending in `squared`, or containing `²` or `^2` is used as typed). (4) Plates are drawn before all leaders, and a plate is not used where an earlier leader runs; a sweep asserts no plate covers a leader. (5) The Check line is left out when Radius or OM is Hidden, or AM and the chord both are (with AM Hidden it uses half the chord); `visible()` needs the part for the Inscribed and Tangent rows; a Given size that follows from a Given angle plus a Given arc, sector, chord or distance counts as shown (no false `not shown` note); the same-letter note needs both letters drawn; a semicircle with the inscribed angle asked and the marks on Show always gets the leak note; grabbing a handle without moving it (pointer within 1 px) changes nothing and dragging B in Chord or OM mode uses the shorter gap (the central-angle control now has step 0.01 so the drag snap keeps a typed 106.26). Also in this round, beyond the minimum: leaders may start from the nearest of several points of their object and end at the best point of their label, a leader never passes through another label or over another leader (the figure grows, then the label moves to the readout), and the right-angle marks reserve room on both copies so the key-only copy keeps the blank and the answer within 6 px.

Known limits (Round 5), not work items: leaders still cross drawn lines in dense or everything-on layouts. Measured on a seeded sweep of 3200 random heavy layouts (45% of parts and rows on, every state, size and style) about one layout in three that has a leader still has a crossing leader after the routing above (the earlier measurement was about 76%); there are no crossings in the shipped examples, the 5/10/20/30 degree cases, the renamed case and the coordinate-label cases except Large with the central angle, the inscribed angle and the centre coordinates all on (the `O (0, 0)` leader crosses the chord AC). Everything-on and 5 and 10 degrees crowd the figure; leaders to labels moved outside run long at Large; in heavy configurations the unit is dropped from figure labels (the readout keeps it) and the editor shows no note for that; the chord and OM controls step in 0.1, so a dragged chord or OM at a radius of 1 or less lands coarsely; typing the clamped radius value back keeps the out-of-range note (the marker matches by value); `Kind::Text` parameters of every figure are cut at 500 characters (a shared change from Round 1). The FAQ stays at six entries.

Figure 1.2 closed (combined critic on 4f5715d: PASS, no blocker or major; full suite 1615 passed, census only 107 new `circle__*` files). Minors left open: units such as `sqm`, `sq.ft`, `cm sq`, `ft2`, `m*m` still get a second `²`; the bitonal test comment says 58/66/75% but the loop runs 66% only; the reflex sweep steps theta by 2; two unused-variable warnings in `circle.rs` (`ident`, `rep`); the `∠AOB = …` label renders in a non-bold fallback font in resvg PNGs; the tangent blank sits about 50 px from its angle with a faint leader (its row names it); the key's Check line widens the canvas on examples 1, 8 and 9 (23 to 57 px) and prints half the chord even when the chord row is off.

## Phase 1.3: right-triangle trig

New figure `right-triangle-trig` (`src/figures/trig.rs`, Math), a new id rather than a mode on `triangle`, so every existing census file is unchanged (52 new `right-triangle-trig__*` files only). Right angle at C, theta marked at A or B (`Theta is at vertex` swaps which acute angle is theta). Sides are labelled opposite / adjacent / hypotenuse or a / b / c (`Name the sides with`), vertex letters and the right-angle mark are switches. The triangle is set by `define`: two legs, leg and hypotenuse, or the angle and the opposite, adjacent or hypotenuse side; the drawing is fitted to a fixed box and kept to scale, and theta is held between 10 and 80 degrees (the sides adjust, a Notes line says so). Each of the three sides, theta, sin, cos and tan is Given / Ask / Hidden: Given is printed on both copies, Ask is `_____` on the student copy (`answers_param()`) and the value on the key; the readout shows sin, cos, tan as `3/5 = 0.6` (fraction of the side lengths, then the decimal) and theta in degrees (the key names arctan(opp/adj) when the hint is on). `Show the SOH CAH TOA hint` is off by default. Decimals 0 to 4 round theta; lengths print with at least two decimals and ratios are worked out from the printed lengths. Six examples: 3-4-5, 5-12-13 (theta at B), 30-60-90 with hypotenuse 10, find the missing side from an angle and a side, find the angle from two sides. Labels (vertex letters, theta, sides) come from a candidate search that rejects spots touching another label or a line, with a 2 px halo and no plates; the canvas grows to hold them and the student and key copies lay out identically. Print is exactly neutral (ink #111111, sides 2.6 px), the label size control is applied inside the module, one handle (drag the far vertex) sets the angle, the Notes box warns about a clamped angle or Given values that do not fix the triangle, the FAQ arm has six entries. Tests: values (3-4-5, 5-12-13, 30-60-90, missing side, every definition), a student-vs-key leak test, exact Print neutrality, a 1920-layout overlap sweep, odd and extreme values through the load path, drag, a 66% bitonal test. Not done: radians, a worked method line for a missing side, the other acute angle as 90 - theta, a second handle for the size.

Backlog (1.3): radians mode; key-only worked line for a missing side (adjacent = hypotenuse x cos theta); optional label for the other acute angle; a size handle; out-of-range typed lengths are clamped silently (no marker note like the circle's radius); at 10 degrees theta sits outside the triangle below the base beside A.

Round 2 (1.3): a Given sin, cos or tan prints its fraction of the lengths on the student copy only when every side it uses is Given; otherwise the student copy shows the decimal only (the key keeps the fraction) and the Notes box says so (a sweep over 243 side-state combinations asserts it, and the leak test no longer skips tokens that match a Given value). New sixth example "Given sin" (opposite 7, sin 0.28 given; hypotenuse, other leg, angle, cos, tan asked). Theta's arc has a 22 px minimum radius; theta is placed first, inside the angle only when within about 24 px of the arc, otherwise just outside either ray beside the arc's end (a short leader is drawn only if a gap over 3 px is left); a test at 10, 12, 15, 20 degrees (theta at A and B, three label sizes) asserts theta is within 24 px of the arc and no other label is nearer. At 10 degrees the arc is still short (about 6.6 px of chord) because the angle is that thin. The drag handle keeps full precision (no rounding, so dragging to 10 or 80 raises no clamp note; the clamp flag has a 1e-6 tolerance). Ratios always print at least two decimals. In Letters mode the SOH CAH TOA hint uses a, b, c. A Notes line says when theta is at its 10 or 80 limit in an Angle setup (a typed value outside is held there; it cannot be told apart from a typed 10 or 80). The FAQ example matches the shipped example (arctan(7/10) = 35 degrees).

Round 3 (1.3): the critic's one major on the round-2 commit. Lengths and fraction operands print with at least two decimals (`dec.max(2)`) whatever Decimal places says for theta, and the printed decimal is worked out from the lengths as printed, so `sin theta = 8.66/10 = 0.87` is true as printed and a 5-9-10 "right" triangle can no longer appear; theta keeps its own `dec`. The SOH hint's arctan operands are lengths too. A sweep over the five define modes, whole and non-integer sides, dec 0 to 4, and key and student asserts every printed `x/y = z` holds at the printed precision, every printed side triple is a right triangle, and every printed length is the solved one at its precision. FAQ: Hidden keeps the side's name on the drawing but shows no value; the arctan example says the hint has to be on and lets Decimal places set the rounding; the rounding note now says ratios follow the printed lengths. Doc: the 10 degree chord is about 6.6 px (was 4), 52 census files, six examples.

## Phase 1.4: sine and cosine

New figure `sine-cosine` (`src/figures/sincos.rs`, Math). The wave is y = A sin(B(x - C)) + D with the x axis in degrees, so B = 360 / period. Where the wave starts is a preset (`The wave starts at the`): midline going up is sin, a peak is cos, the other two are their negatives; the phase shift C is a typed number of degrees and a shift of a whole cycle or more is folded into one cycle (the Notes box says so). Amplitude 0.1 to 10, period 36 to 1080 degrees, midline -5 to 5, decimal places 0 to 2. Three markers are drawn: the midline as a dashed line, the amplitude as a double arrow from the midline to the first peak, and the period as a double arrow between neighbouring peaks; each carries its value, and on the student copy it carries a blank instead when that value is Asked. Four readout rows (amplitude, period, midline, equation) share the trig figure's Given / Ask / Hidden states, and the equation row prints the form that matches the drawing rather than the parameters. The grid uses the shared `nice_step` and `ticks`; Print is neutral grey only and a Print render at a 66% bitonal threshold is exported by the test. Labels carry a 2 px white halo, never a plate. The figure has no handles and is listed in `NO_HANDLES`. FAQ arm has four entries. Five examples: y = sin x, y = 3 sin 2x, y = cos x, y = 2 cos x - 1, and writing the equation from a 90 degree shift. Tests: defaults and their markers, the drawn wave against the printed equation over every preset and three parameter sets, two peaks always inside the axis, student/key blanks (rows and marker labels), Hidden values absent, the folded shift and its note, Print neutrality, and the PNG export. Census: only `sine-cosine__defaults` and `sine-cosine__start` added.

## Phase 1.5: unit circle

New figure `unit-circle` (`src/figures/unitcircle.rs`, Math). A circle of radius 1 about the origin with the point P at angle theta, the radius drawn to it, a dashed line dropped from P to each axis, and the angle marked at the centre. The angle is typed in whole degrees (0 to 360) or dragged: P is a handle, so this figure is not in `NO_HANDLES`. The readout is the coordinate triple: the angle, cos theta (the x coordinate), sin theta (the y coordinate) and P itself, each Given / Ask / Hidden, and the labels on the figure follow the same states (a blank where the value would be). `Show the angle in` switches to radians, written as a fraction of pi when the angle has one with a denominator up to 12 (30 degrees is pi/6, 270 is 3pi/2, 360 is 2pi) and as a decimal number of radians otherwise. Labels are placed by a candidate search that keeps each one inside the canvas and off every label already placed, including the axis numbers; a sweep over every second angle asserts no label overlaps another and none leaves the canvas (which found two real collisions: the x value on the -1 tick and the P label running off the left edge in the third quadrant). Print is neutral grey and the 66% bitonal copy keeps the circle, the drops, the arc and every label. FAQ arm has four entries. Five examples: 30 degrees in radians, 45 degrees both ways, 150 degrees (cos negative), 270 degrees (all Given), and reading the angle from the coordinates. Tests: the defaults, pi fractions for eight angles, the radians row and label, student/key blanks, the point's position at seven angles, the drag handle, Hidden values, the label sweep, Print neutrality, and the PNG export. Census: only `unit-circle__defaults` and `unit-circle__start` added.

## Phase 2: transformations

New figure `transformations` (`src/figures/transform.rs`, Math). A fixed pre-image triangle A(1, 1), B(4, 1), C(1, 3) on a square-unit grid, and its image after one transformation: translate by (dx, dy), reflect in the x-axis, the y-axis, y = x or y = -x, turn 90, 180 or 270 degrees anticlockwise about the origin, or enlarge by a scale factor from 0.5 to 3 about the origin. The rule is written under the grid, `(x, y) -> ...`, and the image's vertices are named A', B', C' and printed with their coordinates. The image is the answer, so it is drawn and printed only while Show answers is on; the student copy keeps the pre-image, the rule and three blanks. The grid is square (the same pixels per unit on both axes) and grows to take in both shapes, so a translation of 8 units or an enlargement by 3 still fits. No handles, so the figure is listed in `NO_HANDLES`. Print is neutral grey (the dashed pre-image and the solid image differ by line style, not hue), and the 66% bitonal copy keeps both. FAQ arm has four entries. Five examples: translate by (3, -2), reflect in the x-axis, reflect in y = x, turn 180 degrees, enlarge by 2. Tests: the defaults, all four reflections (including a point on each mirror line), the three turns, enlargements, the student/key split, the grid covering both shapes for every operation, Print neutrality, and the PNG export. Census: only `transformations__defaults` and `transformations__start` added.

Fraction and number models: new figure `fraction-models` (`src/figures/fraction.rs`, Math). One figure with a `Show a` choice: a fraction bar, a fraction circle, a tape diagram (numbered cut points) or a ten-frame. Shaded parts and total parts are typed (0 to 12; the ten-frame always has ten cells), the shaded count is held at the total, and the readout prints the fraction, its decimal and its percentage. Those three are the answer, so they are printed only while Show answers is on and are blanks on the student copy; the shape is always drawn. Print is neutral grey (shaded cells dark, unshaded white, outlines ink) and the 66% bitonal copy keeps every cell. FAQ arm has four entries. Five examples: three quarters as a bar and as a circle, five eighths as a tape diagram, seven tenths in a ten-frame, and one whole shaded. Tests: the defaults, the shading never exceeding the parts (every model, five fractions), every model drawing as many parts as it says, the student/key split, a whole and none, Print neutrality, and the PNG export. Census: only `fraction-models__defaults` and `fraction-models__start` added.

Triangle centres: new figure `triangle-centres` (`src/figures/triangle_centres.rs`, Math). A fixed acute triangle A(1, 1), B(9, 2), C(3, 8) on a grid, with a `Show the` choice of circumcentre, incentre, centroid or orthocentre. Each draws its own construction (two perpendicular bisectors and the circle through the vertices; two angle bisectors and the incircle; two medians; two altitudes), the centre as a marked point named in words, and a readout giving its coordinates and, where there is one, its radius. Construction lines are clipped to the plot so none runs off the canvas. The centres are computed, not stored: the circumcentre solves the two perpendicular-bisector equations, the incentre weights each vertex by the opposite side, the centroid is the mean, and the orthocentre follows from the Euler relation. Tests check all four against their definitions (equal distance to the vertices, equal distance to the sides, the mean, perpendicular to each opposite side), the Euler line G = (2O + H)/3, the radii, the construction switch, Print neutrality, and the PNG export. No handles, so the figure joins `NO_HANDLES`. FAQ arm has four entries. Five examples: one per centre and the centroid with the construction hidden. Census: only `triangle-centres__defaults` and `triangle-centres__start` added.

Slope triangle: new figure `slope-triangle` (`src/figures/slope.rs`, Math). The line y = mx + c across a fixed window with a right-angled slope triangle on it: a flat leg of `Triangle run` units and an upright leg of gradient x run, with the right angle marked and `run` and `rise` written beside the legs. The readout gives the gradient, the rise for that run, and the equation with the right signs (no `+` when the intercept is zero, a true minus otherwise). The gradient, the rise, the equation and the two leg labels are the answer, so they are blanks while Show answers is off, with the line and the triangle still drawn. An axis number is left out where it would sit on one of the triangle's labels, which is what the first render showed: `run 1` sat on the -4. No handles, so the figure joins `NO_HANDLES`. FAQ arm has four entries. Five examples: gradient 2, a half, a negative gradient, through the origin, and a gentle quarter. Tests: the defaults, the rise equalling gradient x run over five settings with the triangle ends on the line, the equation's signs, the student/key split (readout and legs), Print neutrality, and the PNG export. Census: only `slope-triangle__defaults` and `slope-triangle__start` added.

Critic round, 2026-10-28 (two combined critics over all seven new figures, both FAIL; full disposition in
`docs/critic-reports/2026-10-28-figures-critics-2.md`). Fixed: the sine/cosine equation printed B rounded to the
readout precision, so it described a different wave (B now 4 dp, A and D at one decimal at least, with a round-trip
test); composite leaks on the student copy (a Given equation spelling out an Asked amplitude/midline, a Given point
spelling out an Asked cos/sin — both blanked unless their parts are Given); the slope line not clipped to the plot
(it ran through the readout); slope and transformations printing rounded values that contradicted the drawing at
`dec` 0 (default is now 2); the transformations student grid not containing the image at all (the exercise was
impossible, now the grid always covers both shapes); the mirror line for y = x and y = -x never drawn; axis numbers
painted over by an image side; Large label sizes cutting the readout off the canvas (the canvas widens with the
size); triangle centres having no Show answers at all (a student copy was impossible) and no axis numbers; its
altitudes stopping at the orthocentre instead of reaching the sides; the slope triangle able to leave the plot (the
anchor now moves to fit) and its degenerate m = 0 right angle; fraction printing 0.62 beside 62.5% (the percentage
now follows the printed decimal); unit-circle value labels colliding with axis numbers at Large (a ladder of
candidate positions, swept at all three sizes); its coordinate labels indistinguishable from ticks in Print (now
prefixed `x =` / `y =`); 360 degrees folding to 0 making 2pi unreachable; the sincos Notes never being displayed;
the sincos FAQ claiming a 90 degree right shift gives cosine (it gives minus cosine); and the Print grid vanishing
in a 66% bitonal copy in five figures (now `#9a9a9a`). Refraction: new figure `refraction` (`src/figures/refraction.rs`, Physics). An interface, the normal, an incident
ray at a typed angle and the refracted ray from Snell's law; past the critical angle the reflected ray is drawn and
the readout says "Total internal reflection: no refracted ray". The refracted angle, the critical angle (only when
light goes from the denser medium outwards) and the speed in each medium are the answers and blank on the student
copy. Labels sit beside their rays with a halo; the canvas widens with the label size and the readout gap scales with
it, and the PNG export includes a Large case (both lessons from the circular-motion round). Tests: Snell's law across
four index pairs, the bending direction and the absence of a critical angle going inwards, glass to air at 41.81
degrees with the steep just-below case, no bending down the normal, the student/key split, TIR said in words, and
Print neutrality. Census: only `refraction__defaults` and `refraction__start` added.

Electric field: new figure `electric-field` (`src/figures/electric_field.rs`, Physics). Eight radial field lines whose
arrows reverse with the sign of the charge, three equipotential circles at r, r/2 and r/3 (labelled V, 2V, 3V inward —
equal potential steps are equal steps of 1/r, which is why they bunch near the charge), the charge marked + or −, and
a test charge at a chosen distance with the force on it. Field strength, potential and force are the answers and blank
on the student copy; the force arrow is key-only and fixed-length so it encodes no magnitude. Tests: the textbook
formulas at the defaults, 1/r² for the field against 1/r for the potential, the signs following the charges, the test
charge on the canvas at any distance, the student/key split, a small value never printing zero (falling through to
exponential form), and Print neutrality. Census: only `electric-field__defaults` and `electric-field__start` added.

Magnetic field: new figure `magnetic-field` (`src/figures/magnetic.rs`, Physics). The field round a long straight wire
seen end-on: a dot for current out of the page, a cross for into it, three concentric field-line circles with arrows,
a point P at a chosen distance, and B = mu0 I / (2 pi r) printed in tesla and microtesla. The render caught a reversed
direction — on screen anticlockwise means the top of a circle moves left, and a current out of the page gives
anticlockwise lines, so the arrows had been drawn backwards; both senses are now right and the readout says which.
Tests: the formula at the defaults (8 uT), doubling the distance halving the field, doubling the current doubling it,
the direction following the sense, P inside the outer circle at every distance, the student/key split, a small field
never printing zero, and Print neutrality. Census: only `magnetic-field__defaults` and `magnetic-field__start` added.

Standing waves: new figure `standing-wave` (`src/figures/standing.rs`, Physics). A string fixed at both ends with a
snapshot of the standing wave, the envelope it swings between (dashed), the nodes as dots and the antinodes between
them, and the wavelength 2L/n, the frequency nv/2L and the two counts, all blanked on the student copy. Tests: the
fundamental as half a wavelength, all six harmonics scaling as n and 1/n with f lambda = v, the nodes and antinodes
where the drawn wave crosses and peaks, a short string never printing zero, the student/key split, and Print
neutrality. Census: only `standing-wave__defaults` and `standing-wave__start` added.

PV diagrams: new figure `pv-diagram` (`src/figures/pv_diagram.rs`, Physics). A pressure-volume diagram with the path
drawn and the work shaded as the area under it — an isothermal hyperbola P = nRT/V or an isobaric line — with the start
and end pressures and the work in the readout. The Print grid had been hard-coded to a blue-grey and failed the
figure's own neutrality test; it is a neutral grey now. The first run also failed the two FAQ tests because the figure
was registered without a FAQ arm; four entries were added. Tests: isothermal work = nRT ln(V2/V1) with PV constant, isobaric
work = P ΔV at constant pressure, compression doing negative work, the equal-volumes note, the student/key split, and
Print neutrality. Census: only `pv-diagram__defaults` and `pv-diagram__start` added.

## Phase 3: physics

Circular motion: new figure `circular-motion` (`src/figures/circular.rs`, Physics). A body on a circle at a chosen
angle, with its velocity along the tangent, its centripetal acceleration and force towards the centre (through the
radius line, so the direction is unambiguous), and a readout of the acceleration v²/r, the force mv²/r, the angular
speed and the period. The givens (mass, radius, speed) always print; the four derived values blank on the student
copy. Tests: the textbook formulas at the defaults, F = ma and T·ω = 2π over four settings, doubling the speed
quadrupling the acceleration, the body on the rim at six angles, the student/key split, and Print neutrality (fills
as well as strokes). No handles, so the figure joins `NO_HANDLES`. FAQ arm has four entries. Five examples: the
defaults, a shorter string, twice the speed, a heavy mass on a long string, and a slow small circle. Census: only
`circular-motion__defaults` and `circular-motion__start` added.

Collisions: new figure `collision-momentum` (`src/figures/momentum.rs`, Physics). Two carts on a track before and
after a one-dimensional collision, each with a velocity arrow scaled to the largest speed in the picture, cart size
following mass, and a readout of the momentum before and after (always equal), the speeds after and the kinetic
energy on both sides. `Collision` chooses elastic (the standard solution) or perfectly inelastic (both at the common
speed). The given masses and speeds always print; the momenta, speeds and energies are blanks while Show answers is
off. Tests: momentum conserved in both kinds over four settings, kinetic energy conserved for elastic collisions,
the inelastic loss equalling the kinetic energy of the relative motion exactly, equal masses swapping speeds, the
student/key split, and Print neutrality (fills as well as strokes). No handles, so the figure joins `NO_HANDLES`.
FAQ arm has four entries. Five examples: equal masses, sticking together, a heavier cart, a head-on with both moving,
and a light cart into a heavy one. Census: only `collision-momentum__defaults` and `collision-momentum__start` added.

Energy bar charts: new figure `energy-bar-chart` (`src/figures/energy.rs`, Physics). A body dropped from one height
to another, with its gravitational, kinetic and thermal energy drawn as two stacks of three bars on one scale, so the
group heights can be seen to match: m g h empties, and kinetic plus thermal fill by exactly that (the thermal part
is the share lost to air resistance). The bars are the answer, so with Show answers off the chart keeps its axes,
its names and its blanks and draws no bars at all. Short store names (KE, GPE, thermal) with a header saying what
they stand for, because "gravitational" is wider than the 52 px column. A final height above the start is noted and
held. No handles, so the figure joins `NO_HANDLES`. FAQ arm has four entries. Five examples: a 2 kg ball dropped
5 m, the same with a fifth lost, falling part of the way, a heavier ball, and a small drop. Tests: the default
leaving all 98 J as kinetic, the total never changing over five settings, the bars drawn to one scale, the
student/key split, the height note, and Print neutrality (fills as well as strokes). Census: only
`energy-bar-chart__defaults` and `energy-bar-chart__start` added.

Nets: new figure `solid-net` (`src/figures/nets.rs`, Math). The net of a cube or cuboid: four side rectangles in a
row (a by c, b by c alternating) with the top and bottom (a by b) above and below the first, fitted into a fixed box
and drawn with a light fill. The readout gives the edges, the surface area 2(ab + ac + bc) and the volume abc; the
area and volume are blanks while Show answers is off, but the edges stay: the net is scaled to fit its box, so two
similar boxes draw the same picture and the edges cannot be measured back from it. Every face is labelled a x c,
b x c or a x b, in a size that shrinks to fit the face. Choosing Cube forces all three edges equal, so the
Width and Height controls are ignored. No handles, so the figure joins `NO_HANDLES`. FAQ arm has four entries. Five
examples: a cube of edge 3, a 4 x 3 x 2 cuboid, a tall thin one, a flat one, and a cube of edge 1. Tests: the
defaults' area and volume, a cube forcing its edges, the surface area equalling the six drawn rectangles, six faces
inside the canvas, the student/key split, Print neutrality and the PNG export. Census: only `solid-net__defaults`
and `solid-net__start` added. **This closes the phase-2 list.**

Base-ten blocks: new figure `base-ten-blocks` (`src/figures/baseten.rs`, Math). A three-digit number drawn as
hundreds flats (ten by ten, three to a row), tens rods (one by ten) and ones cubes, each group labelled, with the
place-value expansion written under it. A missing place draws nothing, and every place is named in both copies —
the key writes a zero place as 0 (305 is "3 hundreds + 0 tens + 5 ones") and the student copy gets the same three
labelled blanks, so the two line up term for term. The expansion is the answer, so it is blank while Show answers is
off, with the blocks and the number still drawn. The canvas is sized from the blocks actually used, so a
single flat does not leave a page of air, and the column label sits under the rows drawn rather than under a fixed
three. No handles, so the figure joins `NO_HANDLES`. FAQ arm has four entries. Five examples: 247, 100, 305, 60 and
999. Tests: the default, the places summing back to the number across seven values, a missing place left out of the
expansion, the student/key split, the columns not overlapping, Print neutrality, and the PNG export. Census: only
`base-ten-blocks__defaults` and `base-ten-blocks__start` added.

Area model added to `fraction-models` as a fifth `Show a` choice: a rectangle cut into as many equal cells as there
are parts, laid out as the factorisation closest to square (twelve parts is 3 x 4, a prime falls back to one row),
shaded in reading order. Same readout as the other models, one new example (five twelfths) and a test that the
layout is the expected grid and that the shaded cell count matches the numerator.

Protractor: new figure `protractor` (`src/figures/protractor.rs`, Math). A half-disc graduated every 5 degrees
with longer ticks every 10 and numbers every 30 on two scales (outer 0 at the right, inner 0 at the left), a ray at
a typed angle from 0 to 180, and a readout giving the reading and what the other scale shows (180 minus it). The
ray can be switched off for a sheet where the student places the arm. The reading is the answer, so it is blank
while Show answers is off. The end numbers are lifted off the baseline and the second scale is set further in,
which is what the first render showed needed doing: at 0 and 180 both scales printed on the baseline and on each
other. No handles, so the figure joins `NO_HANDLES`. FAQ arm has four entries. Five examples: 55 degrees, a right
angle, 120 degrees, a ray to read, and the bare protractor. Tests: the default reading, both scales graduated and
summing to 180, the ray's tip on the rim at five angles, the student/key split, the ray switch, Print neutrality,
and the PNG export. Census: only `protractor__defaults` and `protractor__start` added.

Phase-3 critic round (five figures at `cfd89f0`: electric field, magnetic field, standing waves, PV diagrams,
refraction): FAIL — 2 blockers, 6 majors, 9 minors. Both blockers were leaks through drawn geometry on the student
copy, and neither was visible to the tests. The PV diagram printed the two asked pressures as y-axis numbers and shaded
the area under the path, which *is* the work, while its readout said `_____ Pa` and `_____ J`; the y numbers and the
shade are now key-only. The standing wave drew the node dots and antinode rings beside the blanked counts, so the
counts could be answered by counting rather than by reading the wave; the markers are key-only. Majors: two of the
magnetic field's three arrows were not on their field lines (fixed chord and two arrows pushed down the canvas), so each
arrow now sits at the top of its own circle with a chord proportional to the radius; the electric force arrow vanished
for attraction at r <= 1.1 m including the default, so the test charge sits at least 40 px out; the PV path had no
direction and an unsigned area, so expansion and compression drew identically while the work flips sign — the key now
carries a direction arrow and both copies number the ends; the PV givens said `T = 300 K` for an isobaric change, which
is only the initial temperature (now `T1`); the PV title collided with the y tick labels at Large (the left margin now
grows with the label size); and the PV shade at `#e8e8e8` vanished in the 66 % bitonal copy, leaving an isobaric figure
as a bare line (Print now hatches it). Minors fixed in the commit after: the standing antinode rings were on the
envelope side opposite the peak; refraction's doc and FAQ said "the rays stay" although the refracted ray is key-only;
refraction gained the never-zero printer (a hand-edited 0.04 degree link printed `0.0` for a real 0.013); the PV
worksheet FAQ still promised the shaded area; and the magnetic givens use mu-zero and 4pi rather than `mu0` and `4 pi`.
Full disposition in `docs/critic-reports/2026-10-28-figures-critics-8.md`. Still open and recorded there: the electric
equipotential labels speckle at 66 % at 1.3-1.5x, the electric bottom arrowhead's halo nicks the givens line, the
magnetic field prints the direction on the student copy (deliberate), and the PV area in L.Pa needs the 10^-3
conversion that the FAQ now states.

Spring scale: the reading figures can now be printed as a worksheet. The scale's pointer and spring are the reading, so
nothing on the instrument may print the number; the answer key line under the figure already existed, but there was no
way to caption the figure without it, so a student copy could only be blank. A new `answer_text` control (blank by
default, as on the micropipette) replaces the printed reading when typed: `Force: ________ N` gives the student a
prompt and a place to write, with the pointer left where it is (that is the exercise), while leaving it empty keeps the
reading under the figure for the key. Test: the key prints `Force: 3.47 N`, a typed caption wins, and the reading does
not appear anywhere in the worksheet SVG. Census unchanged, because the new control defaults to empty.

Verification of that round (`8f40aff`, frozen copy, own target dir): seven of the eight fixes hold exactly — the PV
student copy has no polygons and no y numbers, the standing markers count 3/5/…/13 on the key and zero on the student
copy, the magnetic arrows sit at 145 − r on each circle, the electric force arrow is drawn in all 100 distance × sign
renders, expansion and compression differ by 219 plot pixels, the givens say T1, and the Print hatch survives 66 % at
0.7x, 1.3x and 2.0x. One held only partly: the PV title was still crossed by a seven-digit pressure label, which the
slider minimum V1 = 1 L reaches. The cause was a left margin that grew 34 px per label step while the labels grew
about 37; the margin is now measured from the widest label the axis prints, with the title's band added, and the two
worst layouts are exported as PNG cases. The verification also caught two regressions the fixes themselves introduced:
the electric 40 px distance floor made 0.1 m and 1.2 m draw identically (floor now 16 px, and the force arrow is drawn
beside the test charge so it needs no room in the gap), and the magnetic inner arrowhead was longer than its chord
(chord now 0.34 r clamped to 12–26). Report: `docs/critic-reports/2026-10-28-figures-verify-8f40aff.md`.

Capacitor circuits on `circuit-diagram`: a new `components` choice (default `Resistors and bulbs`, so every saved
figure is untouched) turns all four slots into capacitors — two plates across the wire, named C or C_1..C_4 — and
switches the worked-out block to capacitance: `C_eq` and `Q`, then each part's `V_k` and `Q_k`. Series adds
reciprocals (1/C = Σ1/Cᵢ, the same charge on each, voltages adding to the battery), parallel adds capacitances (the
same voltage on each, charges adding). Values are typed in farads — 10 µF, 470 nF, 0.5 mF — and `parse_qty` gained the
farad unit and the nano and pico prefixes for them. There is no steady current in a capacitor circuit, so the ammeter
and the voltmeter are forced off in the layout (not just skipped in the drawing), and the figure says so in its FAQ;
the meters' switches stay where they are and simply have no effect. A value still marked in ohms is **not** read as a
number of farads — that would be wrong by a factor of a million and silent — so it counts as unknown, prints as `?`,
and the editor's Notes box says why. An open switch leaves them uncharged — zero charge, zero volts — while the
equivalent capacitance is still known, which is the one place a capacitor circuit behaves differently from a resistor
one. The first render also showed the battery's value label ("12 V") running into the first branch's value ("10 µF")
in parallel: the parallel layout now starts its branches beyond both labels, measured from the texts rather than at a
fixed 240 px, which was already marginal for a long resistance such as 4.7 kΩ. Tests: 10 µF + 10 µF in series on 12 V
gives 5 µF, 60 µC and 6 V each; in parallel 2 × 10 µF on 10 V gives 20 µF and 100 µC each; an unknown part leaves the
equivalent unknown but not the rest; a value in ohms is refused; an open switch charges nothing; both meter controls
render an identical figure in capacitor mode (the ammeter and all four voltmeter settings); and the series and parallel
12 V renders are exported as key and student copies to be looked at. Two examples (series and parallel) were added, and the FAQ explains how
capacitors add the other way round. Census: `circuit-diagram__defaults` and `__start` only, from the blurb now naming
capacitors.

Combination circuits on `circuit-diagram`: a third layout, `In series and parallel`, puts the first component on the top
wire and the rest as branches hanging off it between the same two rails, with junction dots where the branches meet
them. It needs two components or more (with one there is nothing to parallel), and falls back to the series drawing
otherwise. The solver is the textbook mix — the parallel group first, added to the series part — so 2 Ω in series with
4 Ω ∥ 4 Ω on 12 V gives R_eq = 4 Ω, I = 3 A, 6 V across the series part and 6 V across each branch at 1.5 A each. It
works in capacitor mode too, where the same shape means 10 µF in series with two 10 µF in parallel: 20/3 µF, 80 µC on
the series capacitor at 8 V, and 40 µC at 4 V on each branch. The handles follow the drawing (the series part toggles
and scrolls above and below the top wire, the branches beside them), the voltmeter uses the series placement for the
first part and the branch placement for the rest, and the ammeter and switch stay on the left wire. Tests: the
resistor and capacitor figures above, the layout's own geometry (first part on the top wire, the rest below it), and
PNG exports of the key, the student copy and the capacitor combination, all looked at. Census unchanged: the default
layout is still series.

Circuit diagram, critic round: FAIL on `9addc16` — 2 blockers, 7 majors, 11 minors — with every number in the figures
verified correct, so none of it was arithmetic. Both blockers were mine: the combination layout put a voltmeter at
y = −2 (its "above the wire" placement only holds for the series layout, whose top wire sits 70 px lower), and adding
`F` to the shared quantity parser made resistor mode read `470 µF` as 470 µΩ — "1.41 mV across a 470 µF resistor" where
the base build printed `?`. The parser now takes its allowed units as a parameter (`parse_ohm`, `parse_cap`, and
`parse_qty` for the battery), so a farad is never a resistance nor a volt a capacitance, and the uppercase `N`/`P`
prefixes are gone because `N` is newtons. Majors fixed: the branch pitch is measured from the widest value and name
(so "R₂4.7 kΩ" cannot happen), the series canvas grows when four long values plus a switch would merge, a solved
voltmeter gets the wider pitch and a 160 px right margin, a 0 Ω branch in parallel now leaves 0 V everywhere instead of
12 V (a short across the battery), and a capacitor is no longer labelled "2 Ω" — a value that is not a capacitance
draws a blank line. Also: a battery the figure does not print can no longer produce answers (it would give the hidden
voltage away), `fmt_qty` reaches pico, and the resistor/bulb toggle is left off in capacitor mode where it did nothing.
Open and recorded: `fmt_qty`'s 3 s.f. can disagree with the printed inputs at a .5 rounding, mirror mode mirrors the
results table's text, "Fill meter readings" is inert in capacitor mode, and this figure has no label-size control so
the Large sweep is unreachable. Report: `docs/critic-reports/2026-10-29-circuit-diagram-critic.md`.

Ray diagram: new figure `ray-diagram` (`src/figures/raydiagram.rs`, Physics). A thin lens on its axis with the object
arrow, the principal rays and the image they form, for a converging or a diverging lens, and the lens equation worked
underneath: image distance v = uf/(u − f) (signed, so a diverging lens has f < 0), magnification m = −v/u and image
height mh, with a sentence saying whether the image is real or virtual and upright or inverted. The three rays are the
textbook ones — parallel then through F′, straight through the centre, through F then parallel — with the third drawn
only when the ray exists (a converging lens with the object beyond that focus); for a diverging lens the first ray
leaves as though it came from the near focal point. A virtual image is drawn dashed, with its two backward extensions.
The object is a drag handle: sideways sets the object distance, up and down its height, so the figure is not in
`NO_HANDLES`. The image distance, magnification and height blank on the student copy; the axis, lens, foci, object and
rays stay, because the answers are read off the diagram. Tests: the lens equation at the defaults (v = 250/15, m = −2/3,
and 1/u + 1/v = 1/f), the four textbook cases (u = 2f same size, inside the focus virtual and magnified, diverging always
virtual and smaller, on the focus no image), the sign of v over three distances, every nature sentence against its own
numbers, the student/key split, Print neutrality and the PNG export. The first render caught two real defects: the
centre ray was drawn horizontally (the extension helper walked the line the wrong way) and the third ray's parallel
segment was at the wrong height (its base was the axis instead of the object tip); both were rebuilt from the line
through two points. Census: only `ray-diagram__defaults` and `ray-diagram__start` added.

Motion map: new figure `motion-map` (`src/figures/motionmap.rs`, Physics). A strobe record of a moving object: one dot
per equal time interval on a track, a velocity arrow at each dot drawn to a common scale, and the flash number under
each. The three motions are the ones the diagram teaches — constant velocity (equal gaps, equal arrows), speeding up
(gaps growing by a·dt² each interval) and slowing down (gaps shrinking, and the object stopping rather than reversing:
the flashes past the stop sit on the stopping point with v = 0, and a note says where and when it stops). The readout
lists the velocity at every flash, the gaps, and either "Equal gaps: constant velocity" or the acceleration; all of it
blanks on the student copy while the dots, arrows and interval stay. The last flash is a drag handle: it sets the first
velocity by solving the position for v0, so the whole map stretches or compresses — the figure is not in `NO_HANDLES`.
Tests: equal gaps at constant velocity, the gaps growing by exactly a·dt² and the velocities by a·dt, a slowing object
stopping at v0²/2a with the later flashes on that point and the note raised, the student/key split, Print neutrality
and the PNG export. The renders caught two defects: the "track" label printed into the first flash number (the label
is gone — the line reads for itself), and a slowing object's last dots crowd together so their numbers collided (the
flash numbers now sit on two staggered rows). Five examples and four FAQ entries. Census: only
`motion-map__defaults` and `motion-map__start` added. **This closes the phase-3 list.**

Ray diagram and motion map, critic round: FAIL on `1e9451f` — 4 blockers, 11 majors, 12 minors — with the physics right
everywhere and the *printing* and the *extremes* wrong. Blockers: u = 2f said "smaller" at m = −1, in the shipped
example whose point is that the image is the same size (a magnification within 2 % of 1 is now "the same size"); the
printed magnification and image height disagreed with the printed object height ("Magnification −0.7, image height
−2.7 cm" beside h = 4, where −0.7 × 4 = −2.8) — `printed()` now computes each number from the previous printed one; the
motion map printed `a = 2` in the givens and "Acceleration −2" in the readout for the same figure (the givens print the
signed value); and a constant-velocity map printed "0.1, 0.1, 0.1, 5.00e−2, 5.00e−2" under "Equal gaps: constant
velocity", which was float noise in a gap computed by subtracting positions — gaps now come from the mean velocity
times dt where that is exact, and from the positions only where the stop clamps them. Majors: the ray diagram's scale
clamp broke the fit near the focal point (u = 10.5 put the image at x = 675 on a 640 canvas) and its canvas height
ignored a tall image (f = 10, u = 6, h = 10 was clipped at y = −166) — one uniform scale now fits both directions and
the axis is centred in the band; the motion map's clamp drew fewer dots than its readout listed (v₀ = 10, a = 5, dt = 1,
n = 8 put three dots off the canvas); both student copies leaked (the ray diagram printed the real/virtual sentence, the
motion map the acceleration and "v throughout") and both now blank them; the virtual image arrow was solid although the
FAQ promised dashed; **the lens symbols were swapped** (the converging lens had its heads pointing inward); the motion
handle inverted an unclamped parabola while the flashes clamp at the stop (it is not offered where it cannot be
inverted); two example names were false ("from rest" starting at 0.5 m/s, "until it stops" never stopping); and the FAQ
credited the readout with the stopping time and distance, which is the editor's Notes box. Report:
`docs/critic-reports/2026-10-29-ray-motion-critic.md`.

Left open: axis ink `#111827` is grey but not exactly
neutral; trig's key prints a Hidden side inside a fraction; two-label y axes on tall waves; some coordinate labels
sit over the drop lines; two example names overstate what they show.

## Phase 4: biology

Hardy-Weinberg: new figure `hardy-weinberg` (`src/figures/biology/hardyweinberg.rs`, Biology). The gene pool as one bar
split by p and q, and below it one stacked bar per generation showing the genotypes random mating makes from that
pool. With no selection the bars repeat exactly — that is the equilibrium — and the readout gives p², 2pq and q² with
a sum check; the optional counts are rounded by largest remainder so they add up to N exactly (1 + 4 + 5 of 10, never
9 or 11). A "Selection" choice removes every recessive homozygote before it reproduces, and the next generation's
allele frequency follows p′ = p / (1 − q²), so the bars shift and the point of the figure becomes what breaks the
equilibrium. The allele boundary in the gene pool is a drag handle and inverts exactly. Two defects found by looking
at the exports rather than by the tests: a real AA frequency of 0.0001 printed as "AA p² = 0" beside a segment that was
really there (a never-zero formatter now goes to six decimals before exponential form), and the Print copy's labels
were lost in the hatching (they now sit on a white plate; a `paint-order` halo is not used because the PNG renderer
paints the stroke over the glyphs and turns the label white). Tests: p²/2pq/q² and their sum at the defaults, every
generation identical without selection over five allele frequencies, p′ = p/(1 − q²) under selection with p rising
every generation, counts summing to N at four population sizes, a segment too narrow for its label drawn without one,
the student copy free of genotype bars and numbers, the never-zero prints, the drag's exact inverse, Print neutrality
(fills as well as strokes), and the PNG export. Census: only `hardy-weinberg__defaults` and `hardy-weinberg__start`
added.

Verification of the ray/motion fixes (`99d3e43`): a read-only pass in its own archive copy confirmed all four blockers
and seven of the majors fixed with evidence, and found the other two only partly fixed — the image still left the
canvas at long focal lengths (f = 40, u = 40.5 → x = 1332 on a 640 canvas, 2,120 settings) and labels still collided
("F′image" at f = 10, u = 60). It also caught three defects the fix itself had introduced: the vertical fit ignored
the lens, so a tall image pushed it off the top or across the readout; the nature sentence used the exact
magnification while the readout used the rounded one ("Magnification −1" beside "smaller"); and unclipped rays printed
through the readout. All fixed: one collision-aware placer for every drawing label, the lens reserved in the vertical
fit, rays clipped to the drawing band, and an off-canvas image named at the edge (with the distance blanked on the
student copy) instead of silently missing. New tests parse the drawn lens polygons and every rendered label over 96
settings, so a collision or an off-canvas label now fails a test rather than a critic. Report:
`docs/critic-reports/2026-10-29-ray-motion-verify-99d3e43.md`. Census: `hardy-weinberg__defaults` and
`hardy-weinberg__start` added; `ray-diagram__defaults` and `ray-diagram__start` re-hashed for the new geometry.

Chi-square goodness of fit: new figure `chi-square-fit` (`src/figures/biology/chisquarefit.rs`, Biology). This is the
genetics test — do 315 : 101 : 108 : 32 peas fit 9 : 3 : 3 : 1? — and not the test of independence that
`chi-square-table` already draws, where the expected counts come from the row and column totals and cannot be typed.
The table carries the observed counts, the expected counts from the ratio, each category's (O − E)² / E and the totals,
below it the statistic, the degrees of freedom, the p-value and the decision in words. The observed counts and the
ratio are the question and always print; the expected counts, the contributions and the whole statistics block are
the answer and blank on the student copy. The p-value comes from the existing `stats::dist::chisq_cdf` (a one-word
change made `dist` visible to the rest of the crate), so no new special functions were added. Defects caught by
looking at the exports: the decision sentence ran off the right edge (now wrapped to the canvas and the canvas grown
to fit); at Large the title printed into the header and the givens into the statistics block (every vertical offset
now scales with the label size, and the columns are fractions of the width so they spread); "p = < 0.001" read as a
typo (now "p < 0.001"); and a column of trimmed decimals (0.1 beside 0.02) became fixed-decimal columns that fall back
to the never-zero printer rather than showing a real 0.0002 as "0.00". Tests: Mendel's 556 peas giving χ² = 0.470 and
p = 0.925, the statistic equalling the sum of the contributions and the expected counts summing to the observations
over four settings, a perfect fit giving 0 and p = 1, a wide mismatch being rejected and a close one not, a ratio
part of zero with something observed there refusing to produce a statistic, mismatched lists reported, the empty
input, the student/key split, the never-zero prints, Print neutrality and the PNG export. No handles, so it joins
`NO_HANDLES`. Census: only `chi-square-fit__defaults` and `chi-square-fit__start` added.

Enzyme kinetics: new figure `enzyme-kinetics` (`src/figures/biology/enzyme.rs`, Biology). The Michaelis-Menten curve
v = Vmax[S]/(Km + [S]) against the axis, with Vmax drawn as the dashed asymptote, Km marked by the two dotted lines
that meet at half Vmax, and a draggable point reading the rate at a chosen concentration. An inhibitor choice moves
exactly one of the constants: competitive raises Km by (1 + [I]/Ki) and leaves Vmax, noncompetitive divides Vmax by
(1 + [I]/Ki) and leaves Km. The curve, the two constants and the marked rate are the answer and absent from the
student copy, which keeps the axes, their numbers and the givens so the student sketches the curve and marks the
constants. Defects the exports caught: the givens line printed into the readout (8 px apart); the readout repeated
the typed Vmax and Km, which are already given (it now shows only the derived pair under inhibition and the marked
rate); **the noncompetitive case inverted the mapping and printed "Without it: Vmax = 2, Km = 10"**, the shifted
pair rather than the typed one (a test now pins the "Without it" line to the typed values for both inhibitors); and
the marked point's label printed into the Vmax asymptote when the point sat just under it (it drops below the point
there). Census: only `enzyme-kinetics__defaults` and `enzyme-kinetics__start` added.

Cladogram: new figure `cladogram` (`src/figures/biology/cladogram.rs`, Biology). Newick-shaped text in, tree out:
`(((Human, Chimp), Gorilla), Orangutan)` draws either a rectangular tree (horizontal branches with a vertical bar at
each branching point) or a slanted one (diagonals), with the leaves in the order typed and a line under the picture
repeating that order, so a misplaced bracket is visible without counting them. A name after a bracket labels that
ancestor; malformed text is refused with the reason ("a bracket is never closed", "a pair of brackets with nothing
inside them is not a group") instead of drawing nothing. Defects the exports caught: the slanted shape drew every
diagonal flat, because each branch was recorded with the *child's* height at both ends — the parent's height is only
known once its children are walked, so the edges are now recorded after that. Tests: the parser on nested, labelled,
whitespace-and-semicolon trees and six malformed ones; the layout's leaf order, depth and branch direction; one bar
per branching point; every leaf labelled; the two shapes drawing differently by counting segments; the single-name
note; Print neutrality; the PNG export. No handles, so it joins `NO_HANDLES`. Census: only `cladogram__defaults` and
`cladogram__start` added.

Biology batch critic (three figures, `4f89e17`): **FAIL, 2 blockers, 9 majors, 13 minors** — the arithmetic was
verified correct in all three (Hardy-Weinberg over six generations, chi-square against the 3.841 / 7.815 / 11.070
tables and Mendel's 0.4700, enzyme against v = Vmax[S]/(Km+[S]) and both inhibition modes), and the failures were in
what the figures did with awkward input. Fixed: (B1) a zero ratio part with a zero count left a category in the
degrees of freedom, so a test that never ran reported "df = 1 … do not reject" — such categories are now dropped and
the test refuses to run with one category left; (B2) a zero expected count with something observed printed a finite
statistic — it is now ∞ and the test refuses; (M1) the enzyme figure never drew the uninhibited curve, so the
comparison its caption promised was not on the page — it is now dashed underneath, which also fills the space a
squashed noncompetitive curve left; (M2) the rate label was always end-anchored and ran off the left canvas at low
[S]; (M3) the givens line was clipped at Large with an inhibitor (canvas now grows and the line is wrapped and
shorter); (M4) a long category name printed through the number columns (names are shortened with an ellipsis);
(M5) huge counts mixed "1 × 10^12" with "999999999999.56" (one compact format for the whole table); (M6) non-numeric
tokens were dropped silently (reported in the Notes); (M7) the printed contributions did not add up to the printed
total (the total is now the sum of what is printed); (M8) empty input drew a table of zeros (it now says what is
missing); (M9) at one decimal place Hardy-Weinberg printed "0.4 + 0.5 + 0.2 = 1.1" beside a check that said 1 (the
printed fractions are now rounded as a set that adds to exactly 1, at a precision raised until no real frequency
prints as zero). Thirteen minors recorded. Census: `chi-square-fit` and `enzyme-kinetics` re-hashed for the changed
default renders, `cladogram` added.

Cladogram, round 2 (2026-10-29), from looking at the exported PNGs rather than at the tests. Three defects, one a
blocker: (1) the rectangular shape drew every branch along the PARENT's row instead of the child's, so `(A, B, C, D, E)`
— the simplest tree a student would type — came out as one vertical bar with a single horizontal stub at the middle
row, and four leaves with no branch reaching them; every nested tree was subtly wrong the same way (a leaf's branch at
its sibling's row). Branches now run along each child's row and meet the parent's bar, which is pinned by a test that
counts the distinct branch rows of a five-name star. (2) The parser dropped ALL whitespace, so a name with a space
became one word: `Homo sapiens` printed as `Homosapiens` and the "Great apes" example lost its space. Whitespace is now
skipped only around brackets, commas and the semicolon, and a name may contain spaces. (3) A name typed after a bracket
was parsed and never drawn, although the hint, the blurb, the FAQ and the "A labelled ancestor" example all promised it;
it is now drawn just above its branching point, with the left margin and column width grown to fit the longest ancestor
name. Also: the count line now reads "1 branching point, 1 level" rather than "1 branching points, 1 levels", and the FAQ
no longer says an ancestor label is not printed. The same pass fixed a contradiction the biology critic's M7 fix had
left: the table's own Total row still printed the exact statistic (1.43) beside the line under it that now prints the
sum of the printed contributions (1.42). Census re-blessed: only `cladogram`'s two ids move.

Verification critic on `9a5ae46` (2026-10-29): **FAIL** — it confirmed both blockers and most majors fixed, and found
six majors the first round had missed or introduced. Fixed here, all seen in a PNG afterwards:
(1) the cladogram's "Left to right" line wrapped at a fixed 74 characters whatever the canvas, so on the 360 px floor
the last names were cut off — it wraps to the canvas width now, and the continuation lines are indented (SVG collapses
leading spaces, so they were not); (2) malformed-tree reasons were cut mid-word for the same reason, and now wrap to
the width and print "Nothing is drawn…" below them instead of over them; (3) the enzyme's rate label at [S] = 0 landed
on the "Km = n" label (now lifted clear when the two boxes overlap); (4) the rate label was drawn straight through the
curve with no halo (it has one, drawn as an outline first because the PNG renderer ignores `paint-order`); (5) the
chi-square total was rounded to two decimals while a contribution could print four, so 0.0033 + 0.01 totalled 0.01 —
the total now sums the printed strings at the precision they print at (0.0133); (6) at one decimal place Hardy-Weinberg
printed p = 0.99 as "1" beside q = 0.01 — the page's precision now also rises until p and q print honestly.
Minors fixed too: the Km label clips at the right edge near the axis end; "1 categories" and "2 categories with an
expected count" when one expected count is 0; "p = 0.050 is below α = 0.05" when the printed p equals α; the Expected
column did not group thousands beside a grouped Observed column; "(A)" reported a branching point it does not have;
and with an inhibitor the plot's "Vmax = 2" now reads "Vmax′ = 2" (Km′ likewise), which is the apparent constant the
givens' "Vmax = 10" does not contradict. Census: only `chi-square-fit` and `enzyme-kinetics` re-hashed.

Second verification critic on `a073352` (2026-10-29): **FAIL** — twelve claimed fixes checked, most real, five majors left,
three of them incomplete versions of the previous round's fixes. Fixed here, each seen in a PNG:
(1) the enzyme's rate label was drawn over the "Vmax/2" label when the marked [S] is Km — the lift only knew about the Km
label; it now tests both boxes and takes the first clear row; (2) the inhibited half-rate label still said "Vmax/2" beside
a "Vmax′ = 2" asymptote and givens of 10 — it says "Vmax′/2" now; (3) Hardy-Weinberg's "p in each generation" line still
used the typed precision, so p = 0.99 printed a column of 1s under selection — it uses the page's precision, and so does
the editor note that explains the rise; (4) the cladogram's closing sentence ("Nothing is drawn…") was the one line still
unwrapped and ran off the canvas at Large — it wraps now, and the canvas takes the count line's width into account at
Large; (5) a chi-square category expected to be empty but counted printed a contribution of 0.00 beside a Total of ∞ — the
row prints ∞. Two minors fixed too: the half-rate label is haloed like the rate label, and the enzyme's Print grid is
`#787878` like every other figure, because `#e2e2e2` vanished in a 66% bitonal copy. Census: only `enzyme-kinetics`.

Third verification critic on `8d64904` (2026-10-29): **FAIL** — no blocker; 4 majors and 6 minors, and again three of the
majors were one case over from the fix before. Fixed here, each seen in a PNG:
(1) the half-rate label was never checked against the Km label, so a strong noncompetitive inhibitor printed
"Km′=Vmax′/2 = 0.1" on one line — it takes the first of three rows that clears the Km label (or its beyond-the-axis note);
(2) that note was outside every collision test when Km sits past the axis, and a point marked at the axis end printed
through it — the note is in the box list now; (3) a long category name was shortened without reserving the width of the
observed number, so a grouped count printed under the name — the room is what the Observed column leaves once its widest
number is drawn; (4) Hardy-Weinberg's rounding moved the whole difference into the largest part, printing a 1:2:1 cross
as 0.3/0.4/0.3 (a full unit low on the heterozygote against a bar that is plainly half) — the printed set is now the
largest-remainder rounding, which is the nearest set that adds to 1.
Minors fixed too: the chi-square givens line wraps to the canvas (it ran off at Large); an overflowing statistic prints ∞
rather than "inf", and its Total and χ² line agree; a huge contribution from a tiny expected count is written in one
compact form instead of across two columns; the contribution column is thousands-grouped like the counts; the decision
line wraps to the canvas (70 characters overflowed at Large); and the cladogram measures text by width per glyph rather
than by character count, so a run of Ws is no longer clipped at either edge. Census: only `cladogram`.

Fourth verification critic on `cef0949` (2026-10-29): **FAIL** — no blocker; 2 majors, 2 minors, both majors in the
enzyme's label placement. Fixed here, seen in a PNG:
(1) the Vmax/Vmax′ asymptote label was in no collision box at all, so a noncompetitive inhibitor — which drops the
asymptote onto the row where the Km label (or its beyond-the-axis note) lives — printed the two through each other; the
label now moves up its column until it clears them, and the note clears it in turn; (2) when every candidate row for the
rate label failed, the unchecked fallback could be the row below the point, which at the axis is the x-axis tick row —
the candidates are now bounded inside the plot and the fallback is the first row that is inside it, never a tick row.
Minors: a ratio part that is not a whole number keeps its decimals ("1.5 : 2.5 : 1.5", not "2 : 2 : 2"), and a single
name too long to break now sizes the canvas so the "Left to right:" line fits. The two statistic lines that carry a
reason were shortened so they keep a margin at Large.

Food web (2026-10-29): new figure `food-web` (`src/figures/biology/foodweb.rs`, Biology). Typed links, one per line
(`Grass -> Rabbit`), with whole chains on one line (`Grass -> Rabbit -> Fox`), `<-` for the predator-first order, the
unicode arrow, `#` notes, and spaces in names. Producers sit at the bottom (nothing they eat is typed); a predator is
one level above its highest prey, so an omnivore sits with the secondary consumers; the levels are named on the left
and can be turned off. Arrows run from the food to the eater, behind the white-filled boxes. Refused with a reason,
never a panic: a line without an arrow (with its line number), an empty name, a mixed arrow line, a name eating
itself, more than 24 names, and a cycle, whose reason names the loop (`A -> B -> A`). Empty input says what to type.
Nine module tests (parser, levels, every name and link drawn, level names on/off, empty and bad input, canvas fit,
Print neutrality, PNG export), six examples. Census: only `food-web__defaults` and `food-web__start` added. Awaiting
its first critic.

Food-web critic round 1 (2026-10-29): **FAIL** — one blocker, two majors. The blocker: a link that skips a trophic
level was drawn straight behind the boxes and, when the boxes shared an x, exactly on the short links — the bundled
"An omnivore" example (Grass -> Rabbit, Grass -> Fox, Rabbit -> Fox) rendered as a plain chain while the count line
said three links. Fixed, seen in a PNG: every link gets its own attachment points along the box edges (outgoing
spread on the top, incoming on the bottom), and a link that skips a level is routed outside the rows — out above its
prey's row, up a lane right of every box, in below its predator's row — so it can never be hidden. The fan-in major
(eight arrowheads stacked on one point under an apex) is fixed by the same attachments. Minors fixed: a narrow web is
centred in the 420 px minimum; the inert Style control (a monochrome figure) is gone; a long unbroken name breaks in
the error text; `←` is read; runs of whitespace inside a name are tidied to one; the editor note area shows the
parse error; the cycle's second line says "until the links form a one-way web"; it also claimed the unused `mut`
warning was gone — verification round 1 found it still there, and round 2 removed it. Two tests pin that the
skipping link is routed with distinct attachment points and that a many-prey apex has distinct arrivals; the PNG
export gained a fan-in case. Census: only food-web's two hashes moved. Remaining, recorded: a
20–24-name web is a very wide strip (the cap's rationale is not fully met at the top of its range); "Level 5"
replaces the named levels after "Quaternary consumers"; the count line is always drawn (no Show-answers switch);
wide/CJK glyph widths are under-measured by `text_w`.

Food-web verification round 1 (2026-10-29): **PASS** — blocker 1/1 and the reported major confirmed fixed, no new
blocker or major; one MED and five LOW, all addressed. The MED: on a box narrower than its own attachment slots (an
apex named `X` is 29 px), eight full-size heads still merged into a band; the fix widens such a box to hold its
slots rather than shrinking the heads, so every head is one size — which also removes the routed head's ±24–29 %
mismatch with the straight one (both now use `svg::arrow`'s size). The second error line now names the actual
refusal (the name cap says "split the web into two pictures", mixed arrows say "use one direction") instead of the
cycle sentence in every case. `wrap`'s broken chunks are the last prefix that fits, not the first that overflows;
the long-name test asserts the wrapped width, not a start x that is always 24; the unused `mut` and the unused loop
variable are gone. Two tests pin a narrow apex's heads apart and that the second line matches the refusal; the PNG
export gained a `fanin-narrow` case and dropped the duplicate "print" outputs (the Style control is gone, so
`style=Print` is an ignored key — old links still load). The valid render is asserted hue-free, so the figure prints
as it stands. Census: only food-web's two hashes moved again.

Food-web verification round 2 (2026-10-29): **PASS** — all seven round-2 claims held (the narrow apex's box widens
29.36 → 76.48 px, heads are a uniform 9 px in straight and routed links, slots are symmetric and strictly inside,
`wrap` never overflows its reserve, no warnings, census clean). One MED remained, pre-existing: two routed links from
different prey on the same level pair ran their horizontal segments along one y and overlapped into a single bus, so
a six-link picture with two separate pairs could read as all six relations. Round 3 fixes it: each routed link gets
its own run height inside the inter-row gap (staggered by its lane) as well as its own lane. The three LOWs from the
second line's text matching are gone at the root: `parse` now returns `Refusal { why, then }`, so the sentence under
an error is the refusal's own and a name such as `mixes` can no longer change it. Tests pin disjoint routed runs
apart and a self-eat named `mixes` / `run in a circle`. Census: only food-web's two hashes moved. (The worktree also
held another session's uncommitted biology edits; their census hashes were left at the committed values and are not
in this commit.)

Fifth verification critic on `57a8fe0` (2026-10-29): **FAIL** — two blockers, three majors and five minors, all
reproduced and all fixed here, each seen in a PNG. Hardy–Weinberg printed a fraction beside a count it contradicted
(`AA p² = 0.12 (116 of 1000)`, and `aa 0.43` where 435/1000 rounds to 0.44): the page now raises its precision until
each printed fraction × N gives the count on its line (`0.116 (116 of 1000)`), and a row or column carries one
precision. Chi-square ran a test on zero observations and reported `p = 1.00 … consistent with the ratio`: it now says
there are no observations to test, prints `—` for the statistic and no decision (a single category likewise has
none). The HW bars grow with the label size, so `aa 0.16` no longer vanishes at Large; the enzyme's Km and Vmax labels
carry the same halo as the other plot labels, so the dotted marker and curves stop striking through them; the
cladogram measures CJK and other wide glyphs honestly, so a Japanese name stays inside the canvas. The student copies
no longer leak the enzyme's `1 + [I]/Ki` factor or the chi-square Expected Total; a ratio part too large for an integer
prints in scientific notation instead of a saturated i64; a tree cut by the box's 500-character cap says it is too
long instead of blaming the brackets. Eleven new tests; figures 1400/0/4 and figuresui 82/0/0 in the worktree.
Census: only enzyme-kinetics' two hashes moved. Awaiting a sixth verification critic.

Food-web verification round 3 (2026-10-29): **FAIL as a fix** — the stagger's four claims all held, but it
introduced a new MED of the same class. One global index let a departure run and a *different* route's arrival run
land on the same height in one gap (repro: `P0->P1 / P1->P2 / P2->P3 / X0->P2 / X1->P2 / X2->P3 / P1->P3`, where
X1->P2's arrival and P1->P3's departure sat 1 px apart and overlapped 120 px, against 28 px apart before). Round 4
slots every run per gap: departures and arrivals in a gap alike take their own height inside that gap's usable band,
so no two runs can share a height and overlap into a false link. A test walks every pair of runs and fails on any
near-equal height with overlapping x. Recorded limit: past a certain number of runs through one gap the slots sit
closer than the stroke — the exact counts are in the round-4 note below. Census: only food-web's two hashes moved.

Food-web verification round 4 (2026-10-29): **PASS as a fix** — the per-gap slots fixed the MED (the old 1 px pair is
6.25 px apart at Medium), the census and suite were confirmed independently, and no blocker or major was found. Two
cheap corrections from its LOWs. First, the packing reserved an arrowhead's clearance at *both* ends of every gap;
only an arrival has to clear a head, so arrivals now carry that margin and departures only a small box margin,
recovering part of each gap (the usable band is 47·lab − 11 − 4·lab px, against 47·lab − 22 before). Second, the
limit recorded in the round-3 note was Medium-only and its "already a wide strip" defence was wrong: the critic
measured the old packing against the real stroke (1.5 px Medium, 2.1 Large, 1.2 Small) and found it banded from about
15 runs per gap at Medium, 11 at Small and 19 at Large, and a dense 11-name web can put 25 long links through one
gap — the limit is one-dimensional packing, not the canvas. (Those counts are the critic's, for the old margins; the
new margins' limits are worked from the formula below, not re-measured.) The test now sweeps all three label sizes at
the true stroke and asserts the run count. Census: only food-web's two hashes moved.

Food-web density decision (2026-10-29, owner): a web whose routed runs cannot keep a stroke plus 1 px apart in any
gap is **refused**, not drawn. The canvas says "The food web could not be drawn: Too many links skip a trophic level
to keep their runs apart." and asks for fewer level-skipping links — the same policy as the 24-name cap, so an
ambiguous picture never ships. From the formula (band = 47·lab − 11 − 4·lab, refuse when band / (n + 1) < stroke + 1) a
gap is refused from 10 runs at Small, 12 at Medium and 15 at Large, and draws up to 9, 11 and 14; an ordinary web is
far below it. The refusal canvas is shared with the parse refusals (`refusal()`); a test pins that twelve
long links are refused with no boxes drawn while eight still draw, and the PNG export gains a `dense-refused` case.

Food-web verification round 5 (2026-10-30): **FAIL as a complete change** — no blocker or major, one MED. All five
claims held (packing exact, the refusal first fires at 10 / 12 / 15 runs per gap at Small / Medium / Large as rendered
from n = 1 to 24, 42 of 42 parse refusals byte-identical after the `refusal()` refactor, census clean, 1924 passed
on the real tree). The MED: the density refusal drew its canvas but never reached the editor's note area, which
every parse refusal does. The test now lives in one place (`crowded()`, fed by `band()` and `gap_runs()`), so the
canvas and `warnings()` cannot disagree, and the note adds that a larger label size leaves more room — a web with
10–11 runs in one gap draws at Medium and is refused at Small, which the critic flagged as surprising. The minors:
the refusal header's baseline now scales with the label size (it sat 11 px above the reason at Small and 50 px at
Large); the run-height test holds to a stroke plus a visible pixel, the figure's own bar, instead of the looser bare
stroke; `MIN_W` is the one width; the dead `6·lab` arrival term is gone; a test comment no longer calls
a 0.96 px gap a "band". A test pins the note both ways at all three label sizes (a refused web has one, the web one
run below does not). Census unchanged: the valid default render did not move.

Food-web verification round 6 (2026-10-30): **PASS** — no blocker, major or MED; one LOW, two minors, one nit. All six
claims held: 524 webs at each size (1,572 cases) gave zero disagreements between the canvas, `warnings()` and
`notes_for`; 3,144 SVGs from each commit differ in 232 places, all of them the Small and Large refusal canvases (the
header's y), so no drawn web moved; `gap_runs` matches the layout's own lists gap by gap. The LOW: the refusal counts
runs through `gap_runs` while the layout counts them from its own lists, and deleting the arrival count from
`gap_runs` left every test green. Fixed: `build()` now `debug_assert`s the two agree, and a new test puts six
arrivals and five or six departures through one gap (11 draws, 12 is refused, the editor note agrees). That mutant now
fails nine tests. Minor: nothing measured the spacing at the limit; the boundary test now measures the closest runs
of the web just under each limit against stroke plus a pixel (2.34 / 2.66 / 3.28 px against bars of 2.2 / 2.5 / 3.1).
Nit: `band()` no longer returns the departure margin, which no caller read. Recorded, not changed: the refusal
canvas's bottom margin is 26.8 / 20 / 9.2 px at Small / Medium / Large (it was already 9.2 at Large before this round;
only the top was fixed), and its last line does not mention the label size though the editor note does. The food web
is converged. Census unchanged.

Energy pyramid (2026-10-30): new figure `energy-pyramid` (`src/figures/biology/energypyramid.rs`, Biology). Typed levels,
one `Name: value` per line, producers first, drawn as a centred stack of bars; in `Proportional` (the default) a bar's
width is its energy, so the shape is honest, and `Equal steps` draws the conventional stepped pyramid (each bar 70 % of
the one below) and says "Not to scale" on the picture and in its description. Each level is named to the left of its bar
and valued with its unit (kcal, kJ or J per m² per year, or none) inside the bar, or, when the bar is too narrow to hold
its own number, beside it on a short leader, so a sliver never clips its label; a bar under 3 px is solid black so it is
still seen. The percent passed up from one level to the next is computed from the typed values, so it is an answer: drawn
only under Show answers (`answers_param()`), with a rule and a "%" where it goes in a student copy, and absent from the
description. Values print as typed, grouped in threes (`1.5e6` prints `1,500,000`; only below 0.001 or from 10^15 up
is `1.5e21` used); the percent is three significant figures with no trailing zeros, rederived in a test from the two
values as printed. Refusals, as a structured `Refusal { why, then }` on a 420 px canvas with a red header whose baseline
scales with the label size, and each one also the editor's note (`warnings()` through `notes_for`; both come from one
`read()`, so they cannot disagree): fewer than 2 or more than 6 levels, a line with no colon, two colons, no name or no
value, a duplicate name (case-insensitive), a value that is not a number, has misplaced thousands commas, is not above
zero or overflows, a level with more energy than the one below (energy cannot grow up a food chain: a decision the
brief did not make), and, in Proportional only, a largest value more than 1,000 times the smallest (the top bar would be
under half a pixel; Equal steps has no limit). Six examples, an FAQ arm, no handles. Tests (21): the parser and every
refusal with its second line; the refusal canvas at three label sizes; the canvas and the note agree over 28 inputs at both scales and three label sizes;
bar widths within 0.5 px of the typed values at 2 to 6 levels and ratios 1 to 1,000; a sweep of 1,920 renders (2 to 6
levels, ratios 1 / 2 / 10 / 1,000, four name sets including 60-character, CJK and unbroken-W names, both scales, three
label sizes, transfer on and off, key and student) asserting every text inside the canvas, no two texts overlapping, no
text across a bar edge, every value printed; the sliver's label and leader; the student copy leaking none of the
percents (rects, canvas size and values identical to the key); number round-trips; hostile input rendering to a valid
image; no hue; a PNG export (`VERSHION_PNG_DIR`). Census: `energy-pyramid__defaults` and `energy-pyramid__start` added,
nothing else moved. The food-web module's `wrap` and `text_el` became `pub(super)` to be shared. Awaiting its first critic.

Energy pyramid round 2 (2026-10-30): the first critic **FAIL** (2 MED, 5 LOW, 5 minor; bar widths, the 16,000-input
agreement of canvas and note, and the student copy all held) and four owner decisions, fixed in one commit.
**Owner decisions (2026-10-30):** (1) a chain over the 1,000:1 limit, such as a 10 % chain of five levels, stays
refused in Proportional, and the refusal's second line now says plainly "set Bar widths to Equal steps to draw this
chain (it is marked Not to scale)"; nothing switches by itself. (2) Equal values (a 100 % transfer) stay allowed. (3) The
99.9 rule stays (two unequal bars never print 100 %) and is one short FAQ answer. (4) A value below 0.001 or from 10^15
up is written `1.5 × 10^21`, drawn in the SVG as a smaller `<tspan>` raised by `dy` and a second `dy` back to the
baseline (no superscript glyphs; a test pins the two `dy`s summing to zero and the 0.7 size); the width estimate counts
the exponent, and the label, the percent and the editor note use the same number (the note and the description keep
the plain `^`, since they are not drawn). **MED 1:** at 1000:1 the 0.40 px top bar sat on the arrows' x. The arrows now
run in a column of their own beyond every bar and label, from just above one row to just below the next, and a bar
under 6 px is solid with a 7 px cap at each end (the widths stay exactly proportional; the caption says so). A test
renders the default at all three sizes at 1x and 1.3x and finds a print-copy-dark pixel on both caps. **MED 2:** the
percent rounded exact ties to even (8000 over 2500 printed 31.2); it now rounds half up (31.3), and the test that
checked `fmt_pct` against itself is replaced by an oracle in exact integer arithmetic over every b/a with a up to 400
(more than 20 exact ties) plus a table (31.25, 56.25, 3.125, 81.25...). **LOW 3:** the ratio reason now says "thinner
than 0.4 px", true on both sides of the limit. **LOW 5:** a leading U+FEFF is dropped. **LOW 7:** the student's blank is
48 px long (scaled) with the whole column free above it, tested. **Minors:** values are read to 15 significant figures
(10000.000000000001 is 10,000; stated in the FAQ); `1.5 X 10^6`, `1.5 × 10⁶`, and a bare `10⁶` or `10^6` are read; the
names in the "more energy" and duplicate-name refusals are quoted; the leak test derives the hidden strings from the
computed percents of four chains and also searches the note. **Recorded, not fixed:** duplicate names are folded with
`to_lowercase`, not full case folding, so `ß` and `SS`, or `İ` and `i`, count as different levels. **Also noted:** the
percent's tie test uses a 1e-7 tolerance on the last printed digit (a ratio within 3e-10 of a tie rounds up); the
refusal canvas writes a huge value with the plain `^`, since it is wrapped text and not a figure. The mutants run
against the new tests (half-even rounding, no caps, arrows back on the axis, no BOM strip, no 15-digit read, a width
estimate that ignores the exponent, a 30 px blank, unquoted names, the old reason, no `dy` return, no superscript or
capital X read, the key always drawn) are each killed. Census: only `energy-pyramid__*` ids move. Verified by the third critic (below).

Energy pyramid round 3 (2026-10-30): the second critic **FAIL** (1 MED, 3 LOW, 5 minor; the half-up percent, the arrow
column, the 6 px cap rule, the ratio refusal and the student copy all held). **Owner decision (2026-10-30):** a bar
thinner than 1.5 px is drawn 1.5 px wide, solid black, so it survives the 66 % grey print copy at 1x (a 0.4 px black
bar fades to light grey there, which round 2's cap-only test missed); bars of 1.5 px and wider stay exactly
proportional, and Equal steps is untouched. The caption says so only when it happens: "...its width is to scale, except
that a bar under 1.5 px is drawn 1.5 px wide", one sentence with the 6 px caps rule. `the_top_level_survives_the_print_copy`
now rasterizes the 66 % copy at 1x and 1.3x at all three sizes and needs a dark pixel in the bar's own columns on every
row between its caps (a mutant drawing 0.4 px is killed by it and by six more tests). **LOW 1:** the transfer-off
assertion no longer ends in a clause the caption always satisfies; it renders the chain in Equal steps and asserts no
arrowhead, no vertical shaft, no percent, no arrow sentence and a narrower canvas (a mutant drawing the column with the
transfer off is killed). **LOW 2:** a sliver's name now stands 16 px from its caps, so a cap level with a wrapped name
line cannot read as punctuation (checked in the sweep against a literal 16, so the constant cannot move the test with it;
the six 50-character names at Large are in the PNG set). **LOW 3:** a value with a raised exponent carries an
`aria-label` with the plain `1 × 10^21 kcal/m²/yr` (a test asserts it equals the drawn text with the caret and that
the rasterized picture is unchanged). Copying the drawn text still gives `1 × 1021`: a known limit, not fixed, and an FAQ
answer. **Minors fixed:** `1.5 × 10 ^ 6` and `10 ⁶` (a space before the exponent) are read; `1e-400` is refused as
smaller than the figure can read, not as "not above zero". **Recorded limits, not fixed:** (1) the 1e-7 tie window:
`1000000000 / 312499999.9` prints 31.3 where the exact value is 31.2; (2) the 15-digit read moves `999999999999999.9`
across the notation threshold, so it prints `1 × 10^15`; (3) with the transfer on the pyramid sits up to 179 px left of
centre, and at Large the fixed percent column leaves up to 83 px of empty right margin (the student blank needs that
room). Tests: 27 in the module; mutants for each fix killed (the width mutant by three tests; the builder's note said seven, the third critic counted three). Census: only
`energy-pyramid__*` ids move. Verified by the third critic (below).

Energy pyramid, third critic on `e29ba45` (2026-10-30): **PASS** — no blocker, major or MED; two LOW, six minor. Round 2's
MED is closed: bars under 1.5 px draw exactly 1.5 px (242 boundary renders, no jump at the line, centred on the axis),
the caption sentence appears exactly when a bar is widened and never in Equal steps, and the 66 % print copy keeps ink
on every bar row, line, arrowhead and label in all 78 renders checked (a mutant drawing the 1.5 px bar grey is killed,
so the test does look at the bar and not only its caps). The transfer-off check is real again, a sliver's name stays
16 px from its caps over 10,608 renders, the `aria-label` is on exactly the labels with a raised exponent and every
render parses as XML even with hostile names, 10 of 10 mutants were killed, canvas and editor note agreed on 12,000
random inputs plus the sweep, the census moved only the two energy-pyramid ids, 1975 passed on the critic's copy.
Recorded, not fixed (the loop converges on minors): two unequal levels can now draw identical bars (`Grass: 1000 /
Mice: 3 / Owls: 1` gives 1.2 px and 0.4 px to scale, both drawn 1.5 px) and the caption says a bar was widened without
saying which; the default figure always has a 0.4 px level, so the commonest render carries the long caption (3 lines at
Medium, 4 at Large; nothing collides, it is clutter); `B: -1e-400` is refused as "smaller than the figure can read" when
it is negative; spaces between digits are now joined silently (`1 5 × 10 ^ 6` reads as 15,000,000; `1.5 × 10⁶ 7`
reading as 1.5 × 10^67 is older); a bar that computes to 1.4999999999999998 can be called widened (985 of 19,999
exact-1.5 inputs); the new message names 5 × 10^-324 as the smallest readable value but `3.125e-321` prints
`3.127 × 10^-321`; a sliver's name sits further left than the next, wider level's. If the caption clutter bothers a
teacher, shortening it to one sentence is the cheap fix. Phase 4 next: action potential, cell cycle, cell diagrams.

Action potential (2026-10-30): new figure `action-potential` (`src/figures/biology/actionpotential.rs`, Biology). A neuron's
membrane potential (mV) against time (ms), the standard exam trace. The curve is analytic, not a polyline: a flat rest,
`smoothstep(u^1.5)` up to the peak (zero slope at the top), a mirror smoothstep down to the lowest point of the undershoot
(zero slope again), then a critically damped exponential `(1 + kt)e^-kt` back to rest, so every piece is monotone and the
trace cannot wiggle. Resting, peak and lowest point are hit exactly, the threshold is crossed once up and once down, and the
path is sampled until no segment is longer than 0.9 px (the peak, trough, stimulus and threshold times are always sampled).
The spike is a fixed 0.8 ms rise, 1.4 ms fall and about 2.5 ms settle, so it needs 4.7 ms after the stimulus. Controls: six
typed numbers (resting, threshold, peak, undershoot in mV; stimulus time and total time in ms; unit text, a minus sign of
any kind and a decimal comma are read; `1,000` is refused as ambiguous), a Failed initiation switch (a bump that peaks 60 %
of the way to threshold and returns to rest), five show switches (threshold line, stimulus marker, phase pointers, ion-channel
bands, refractory bands), Show answers, Style and Label size. Decisions: the NAMES are the exercise. Numbered discs point at the
stimulus, the resting level, the threshold line (a disc in the right margin) and each phase; lettered discs sit under
Na+/K+ and absolute/relative refractory brackets; every name lives in a two-column key under the figure, which has a blank
line for each in a student copy, so both copies are the same size and the curve, discs and brackets are byte-identical. The
typed resting, threshold, peak and undershoot numbers are never printed as text in either copy (they are read off the axes);
the word "Threshold" is therefore a key entry, not a label on the line. A disc is found by search (several places along the
curve, eight directions, four lengths; first clear of the curve, guide lines, discs, other leaders and the stimulus arrow, with
its leader kept off the curve, the arrow, other leaders and other dots) and a trace with no clear place is refused with that
reason rather than drawn crowded; none of the 6,528 sweep cases, 1,500+ grid cases or the examples hits it.
Refused with a true reason and a second line: a value that is not a number, outside -100..+70 mV (resting -100..-14,
threshold -100..-4), a threshold at or above 0 mV or within 4 mV of it, a threshold under 10 mV above rest, a peak under 20 mV above
the threshold, an undershoot above rest or under 3 mV below it (rest itself means no undershoot), a stimulus before 0.5 ms, a
total that is not positive, over 12 ms (the upstroke would be under 35 px) or too short to hold the spike. Disclosures (a failed
initiation with a band switch on; no undershoot with a dependent item on) are in the EDITOR NOTE ONLY (`notes_for`); the canvas, both copies, the
title, the description and the aria text stay silent (round 2). Round-1 tests (25, in the module): the extremes and the
crossing count on a 13-set family, the derivative sign pattern over 80,000 samples, the path read back through the
picture's own ticks (peak and trough rows within 0.06 px, no chord more than 0.25 px from the true curve), an independent
audit (own Arial-based text widths, literal 1-2 px gaps) over 4,992 renders (13 trace sets x 3 sizes x 32 switch sets x
failed x answers) for text/text, text/curve, text/line, disc/disc, disc/curve, disc/line, canvas edge, plus the expected disc
count so nothing is dropped; a 1,500+ case grid of valid traces never refused; the student copy carries no name and none of the typed numbers
over 256 states; note and canvas agreement over 10,000 random inputs (BOM, CRLF, tabs, decimal commas, unit text, NaN, inf, huge,
tiny); Print grey-only. 17 of 17 mutants were killed (peak, trough, failure past threshold, wiggle, names always drawn, a name in the
description, a typed number printed, note dropped, caption not drawn, disc clearance, x title into the ticks, coarse path, Na+
band too long, no blank lines, a red Print line, refusal's second line dropped). Known limits: the shape and the time course are schematic and
fixed, not fitted to a neuron type; the channel and refractory brackets are placed from the curve, not measured; a total over
12 ms is refused; the refusal for "no clear place for a pointer" and for a stimulus pointer touching the dip are never reached by any
input I found (they guard the search); the controls are text boxes, not sliders; the egui editor itself was not driven; text
widths in the audit are an estimate of Noto Sans, not the rendered face.


Action potential, round 2 (2026-10-30, after critic 1: 0 blocker, 3 major, 4 MED, 5 LOW, 6 minor). Owner decisions, dated
2026-10-30: (1) disclosures live in the editor note only, so the canvas caption is gone and the description and title never say
what was left out; the agreement test now asserts the canvas carries NO disclosure and the note carries it, over 10,000 random
inputs; (2) the K+ bracket and the relative refractory bracket end where the curve is back at rest, taken as within 1 mV of the
resting level after the undershoot (`t_settled`, 1.5 ms after the lowest point for a 10 mV dip, never more than 2.5 ms for any
dip), and the "return to resting potential" pointer sits after them; Na+ runs from threshold to the peak and the absolute
refractory period from threshold to where the falling curve crosses rest, which is how OpenStax A&P 12.4 and Campbell 48.3 draw
them (I checked the extents against those two descriptions, not against a printed page); (3) a threshold at or above 0 mV is
refused with a reason, and so is one within 4 mV of it, so the resting potential tops out at -14 mV (the 10 mV gap to threshold
plus 4 mV): ranges, refusal text, FAQ, hints and census follow; (4) the editor keeps text boxes, not sliders. KNOWN DIFFERENCE:
enzyme kinetics and Hardy-Weinberg use sliders, this figure's six numbers are text boxes, so it has parse refusals they do not;
a value typed on a limit is now accepted exactly. Fixes: minor ticks (4 or 5 divisions per labelled step) and a light grid at
every tick level; the range ends on a minor tick, so Large no longer draws -200..150 for -100/-90/70 (its span is under 290 mV, asserted);
with no undershoot the "return" pointer is dropped with the hyperpolarisation pointer (it would point at the same flat line as
"resting potential"), and the resting pointer is always before the stimulus, so no two blanks share an answer; a stimulus very
early in a long trace widens the plot so the resting pointer has room; leaders are kept off the curve, the stimulus arrow,
other leaders and other dots, in the search and in the audit (a leader used to cross the arrow in 16 renders); row labels
Channels and Refractory period; in Print the four brackets differ by dash (solid, long dashes, dash-dot, dotted), the raised
plus is bold at 0.95 of the key size, the pointer dots have a white ring, and the Print test reads the curve ELEMENT and every
stroke against the 66 % cut; numbers are kept exactly (billionths) so a one-decimal value on a limit is never refused and a
message quotes the digits typed; the timing refusal offers only what works (total, stimulus, both, or neither alone); dead
code, `b2f` and the private `dist_seg` removed (the public `composite::seg_dist` is used). Mutants: 46 run, 43 killed; survivors are the leader-crosses-leader and leader-touches-another-dot guards and the disc-clearance guard, none of which changes any render in the 6,528-case sweep (redundant with the other checks; the audit still measures all three). Limits: the grid is light and
vanishes in a 66 % black-and-white copy, where the minor ticks on the axis remain; the widened plot for a very early stimulus
can be over 1,200 px at Large; a level reads to about half a division (2 to 5 mV), not better; the extents of the bands were
checked against two textbook descriptions, not a printed figure; the live editor was not driven.
### Phase 4 biology: verification rounds 6-11 (hardy-weinberg, chi-square-fit, enzyme-kinetics, cladogram)
Six more critic/builder rounds after the fifth pass, each on a frozen `git archive` with its own target dir (reports
`2026-10-29-phase4-verify5.md` .. `2026-10-30-phase4-verify11.md`). Blockers/majors/minors: 6th 1/2/1, 7th 1/5/10, 8th 1/2/8,
9th 0/3/6, 10th 1/0/7, **11th 0/0/3 (PASS, chi-square only)**. Commits `68a5ed7`, `dfcbd79`, `df9c7c2`, `48d7701`, `db37b61`.
What changed: Hardy-Weinberg derives everything from one six-decimal p as an exact fraction; chi-square rounds each count once
to six decimals and computes everything from that, prints plain only up to 15 digits, else scientific; the enzyme labels are
placed by a collision box with thin-line penalties (half-rate label inside the plot, above the axis, off the Km row; givens wrap
grows the readout offset); the cladogram lays mixed-script names out by real glyph advances (new data-only
`noto_adv.rs`, generated from NotoSans-Regular.ttf) and sends what Noto Sans lacks to the numbered list. The repeating defect
was a fix right for the reported case and wrong for its neighbour; two of the ninth round's three majors were regressions from
the eighth round's fixes.
Known residuals (recorded, not fixed): Hangul draws in the bundled font's faint face and at 0.92 em (gaps wider than a space in
Han+Hangul names); rare CJK-adjacent blocks overlap; Devanagari mixed with Latin shows boxes; the enzyme half-rate fallback row
can sit ~40 px above its line and the curve crosses haloed labels with a strong inhibitor; a 1-2 px touch of Km' against the
marked point in rare seeds; ~3 of 2,000 dense slanted trees keep one struck ancestor label; chi-square Expected cells of 13-15
digits can be wrong in the last digit in about 0.9% of very large tables (double-precision rounding; the sweep test tolerance
is too loose to see it); the six-decimal note names counts the test does not use; chi-square chi-square printed at mixed
precision when contributions are tiny. Not checked in any round: the live egui editor, the web renderer (no system Noto Sans
there, so the width table may differ), PDF/DOCX export.
