# Figures gap-fill progress

## Phase 1.1: conics and implicit curves on coordinate-grid

Equations in x and y that are not straight lines (circles, ellipses, hyperbolas, x = y^2, xy = 4, chained and system inequalities) are
sampled by marching squares and drawn as smooth polylines, with a single tint (a hatch in Print) for the common solution of several
inequalities, dashes only for excluded boundaries, asymptotes recognised by geometry and drawn fine and dotted, curve labels placed
with leaders, tick numbers kept clear of curves, and editor-side notes for rows that cannot be drawn as typed. Print is built for
photocopies (dark grid at 1.4 px, hatch survives a bitonal copy, rows at least 1.8 times the grid). Wide windows thin their numbers and
grid. Existing census files for every other figure, and for the older coordinate-grid cases, are byte-identical.

## Known limits

- A dashed closed curve can show a short seam where its dash pattern restarts (for instance where a dashed circle meets a dashed line);
  fixing it means scaling the dash array, which would change the literal `9 6` that several tests and exports rely on.
- Where no free spot exists for a tick number beside a curve, the number stays and its white plate is painted over the curve for that
  digit alone (a marked, tiny break in the curve).
- tan(x) at +-1000 has a period under 2 px and is aliased by any sampler; it is checked only to +-200.
- A wide window with Square cells on is only warned about, not relaxed.
- Plain Colour windows (no conic, not Print) keep the number placement of the earlier figures, so the origin 0 can sit 2 px from a
  neighbouring -1; changing it would alter existing census files. Colour/Print and conic windows resolve every collision.
- The Colour grid is near-black for a plain figure and a grey-blue beside a conic, for the same census reason.
- Windows squeezed by Square cells (wider or taller than 6 to 1) keep every number and gridline; the editor says so.

## Tick numbers (round 8)

- A number keeps its own tick along its axis (at most a 3 px nudge, and at most 15% of the number spacing). To clear a curve or another
  number it moves only perpendicular to the axis: further out in 4 px steps (up to 16) or across the axis. A number that left its row
  by more than 8 px gets a 0.8 px grey leader from the tick.
- When no spot is free of everything, the nearest spot clear of other numbers is used and its white plate is painted over the curve for
  that digit (`data-plate="over"`). There is no cap on these. They show up where a curve runs along an axis for a long stretch (the top
  and bottom of a circle beside the y numbers, hyperbola vertices on the x-axis), as a visible local break in the curve.
- `data-plate="over"` can appear in any figure drawn in Print (and any figure with a conic), not only conics: the census has them only in
  the new conic examples ex11 to ex17; no earlier census file changed.
- The number search is local (64 px cells) with a budget of 2 million checks, past which numbers keep their standard places. Square
  windows with thousands of numbers (x +-1000 by y +-1, 40 circles at +-100000 by +-1, x +-1e6 by y +-10) render in well under 2 s of CPU;
  they took 7 to 19 s after round 7.
- Major gridlines use a stride that divides the number stride, so every numbered tick has a line. Minor lines are left out when they
  would be closer than 3 px, whatever the major stride.
- Origin zero: below and left of the origin when free, else a row lower, else below right. In hyperbolas with the vertex at 1 (ex12,
  ex16) the y "-1" fills the lower left, so the 0 sits below right with about 12 px before the x "1": it reads "0  1", not "10".
- Plain Colour figures without a conic or Print keep the legacy places (census), so an origin 0 can still touch a neighbouring number
  there in some small windows. Only Print and conic figures are rearranged.

## Leftover minors

- Strict double-root inequalities (y^2 > 0, abs(x) > 0, (y-1)^2 > 0) tint the whole window without the dashed excluded line or an axis
  cut, and `x^2 > 0; y^2 > 0` says "No part of this curve is inside the window" while still tinting.
- (x-y)^2 = 0 and x^2 + 2xy + y^2 = 0 stop about 1.2 px short of the window edge.
- White number plates leave small notches in the tint near the origin.
- Notes written on the figure use a different font from the figure's numbers.
- A Print-only default for equation labels when several curves are drawn was not added (rows differ by weight and tone only).
- A circle or ellipse with a vertex on an axis number row (x^2 + y^2 = 25 at +-10) loses a short stretch of curve under the y "5" and "-5" plates.
- A number moved by the free-spot search while its over-plate fallback is used gets no leader.
- Plain Colour windows still use the legacy number places, so the new displacement tests cover Colour only where a conic is present.
