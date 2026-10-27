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

New module `src/figures/circle.rs` (id `circle`, subject Math), drawn with `svg::Doc` at a fixed size (the radius is a number to read, not a scale). Toggles: centre, radius, diameter, chord, perpendicular bisector, tangent at A (with the right angle), secant, central angle, inscribed angle, minor/major arc, minor/major sector (each follows its own choice). Points A, B and C sit at angles in degrees and are draggable handles (reached through `math::handles`, so the circle is not in `NO_HANDLES`); a handle exists only for a point that is drawn, and a drag snaps to whole degrees and to 30/45/60/90/... within 3 degrees (typed values are exact). The four point letters and the midpoint are text params (3 characters, never empty) and every label, row and note uses them.

Given / Ask / Hidden: each of radius, diameter, circumference, area, central angle, arc length, sector area, chord and inscribed angle has a state in the Measures section. Given is printed on the figure and on the student copy, Ask is a blank (`____`, one style for the figure and the card) on the student copy and the value on the key (through `answers_param()`), Hidden is not shown. Defaults: radius and central angle Given, the rest Ask (the readout rows themselves are separate toggles). The card shows `lhs = value`; `Show formula hints` (off by default) adds the formula. A row for the inscribed angle says `C is not shown` or `C must differ from A and B` instead of a value when it has none. Right-angle marks (tangent, chord midpoint, angle in a semicircle) are Show always / Key only / Hide.

Layout: labels are placed by a candidate search (8 directions and the four corners, three distances each, then a ring with a leader line); a spot that touches another label, a point, a line of the drawing, the readout or the edge is rejected, and a small angle (under 25 degrees) puts its angle, chord and midpoint labels outside the circle with leaders. The readout is laid out from measured text widths (`tw`, the same estimator in code and tests): the label column is the widest label, long rows go on two lines, and the canvas grows in width and height, so Small, Medium and Large never overlap. The label size is applied inside the module (not by `with_label_size`), so the geometry is unchanged. Numbers are grouped in thousands up to 1e12 and honour the decimal places.

Print: exactly neutral (`#111111` ink, greys only; the test uses spread 0), a hatch fill, the circle at 2.6 px, inscribed lines, angle arcs and right-angle marks at 2 px, auxiliary lines (bisector, secant extensions) at 1.6 px dash `7 5` `#666666`, checked at a 66% bitonal threshold. Census: new `circle__*` lines only, no existing line changed. Editor notes (the Notes box, gated to `circle` in `figuresui.rs` through `figures::notes_for`): radius out of range, A and B in the same place, C on A or B, the inscribed row without C. Examples: eight (radius and diameter, chord and bisector, tangent, central vs inscribed, arc and sector, radians, tangent and secant at A, semicircle). FAQ arm `CIRCLE` has six questions.

Not in this figure (follow-up figure `circle theorems`): an external point P with two tangents (equal tangent lengths), tangent-secant (the power of a point), a second inscribed point C2 on the same arc (angles on the same arc), the cyclic quadrilateral (opposite angles), the alternate segment theorem, equal chords and their distances from the centre, and intersecting chords.
