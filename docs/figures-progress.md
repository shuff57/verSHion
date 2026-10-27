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
- Thinning of tick numbers follows the spacing rule under "Tick numbers"; a plain Colour window keeps its legacy numbers.

## Tick numbers (round 11: one simple rule)

In a figure with a conic or implicit row, or in Print (a plain Colour figure keeps its earlier layout, for census identity):

- Every number sits at its own tick in the standard row (x numbers under the x-axis, y numbers left of the y-axis). Nothing is moved,
  flipped across an axis, or led back to its tick; there are no leader lines for numbers.
- A number whose box touches a curve gets a thin 2 px white halo (`data-plate="over"`), so the curve is only interrupted by the halo.
  No white plate covers a curve. Numbers clear of curves keep the white plate under the curves that clears the gridlines.
- Thinning comes from spacing only: the x axis shows every Nth number when two neighbours would be less than 4 px apart (text widths
  measured, minus signs included), the y axis when rows are under 16 px apart; N from 2, 5, 10, 20 ... , the same either side of 0,
  each axis on its own. One more case: in a squeezed window the x "-1" can land on the y "-1" under the y axis (about 20 px or less per
  unit on both axes); then the x axis is thinned by the same list. That can drop an x intercept (circle r = 5 at x +-5 loses the x "5").
- The editor notes "Some axis numbers were thinned ..." exactly when an axis shows fewer numbers than the step asked for.
- Origin zero: below left of the origin, else below right, left out only if neither fits.
- Major gridlines closer than 6 px are drawn every Nth line (N divides the number stride, so every numbered tick has a line); minor
  lines under 4 px are left out.
- The digit boxes are estimates (about 0.75 px of side bearing), so the tests allow 3 px of overlap between the boxes of two numbers.

## Phase 1.1 final state (round 12)

- Plain Colour figures (no conic row, not Print) are identical to the figures before the conic work: same numbers, same gridlines
  (every line, every minor line), in any window. The 2100-line safety cap and thinning of numbers that literally touch are the only
  differences.
- Print and conic figures: numbers on their own ticks, a thin halo over curves, thinning from number spacing only (gap under 1 px
  between measured text boxes), and an x number at the x-row/y-column corner dropped below its row (clear of the y numbers) when
  a spot exists. Where the y rows are under about 21 px apart (Square windows taller or wider than about 13 by 13) no spot exists
  between the y numbers, and the x axis is thinned evenly instead (editor note); a circle of radius 5 then loses its x intercepts.
- Print grid is a hairline-weight 1.4 px grey: it drops out of a bitonal copy below 66%; the Bold option is the remedy. Windows of
  +-50 and +-1000 are drawn with every Nth line (6 px minimum) in Print and with conics; plain Colour draws every line.
- The `data-axis` attribute on tick numbers appears in the exported SVG (harmless).

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
- Plain Colour windows (no conic, not Print) still use the legacy number places, so an origin 0 can touch a neighbour there in some windows.
- In a tall narrow window the equation label can sit over the y numbers.
