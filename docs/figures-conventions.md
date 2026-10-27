# Conventions for later figures

Rules learned on figure 1.1 (conics on coordinate-grid). Apply them to every new figure.

- Solid means an included boundary or a plain curve; dashed (`9 6`) means an excluded (strict) boundary only.
- An asymptote is a fine dashed grey line, lighter than the curve: Print 1.6 px, dash `7 5`, `#666666`; Colour dotted, at least 1.4 px.
- Print is neutral grey only (no hue); rows differ by weight and tone, and a curve is at least 2.4 px at its darkest.
- The Print grid and hatch must survive a bitonal copy at 66% (test at 2x, at 58, 66 and 75%); the thin default grid breaks below that.
- A label never erases a curve: a number or text over a curve gets a thin (2 px) white halo, never a plate over the curve.
- Tick numbers sit on their own tick; do not dodge them sideways or flip them, and do not add leader lines for numbers.
- Thin numbers by measured spacing only (text widths from the same estimator in code and tests), evenly and symmetric about 0.
- Every numbered tick keeps a gridline; the gridline stride divides the number stride.
- The student copy omits labels and shading; the key shows them.
- The Colour grid must not outweigh 2 px curves (a grey-blue grid beside a conic, near-black for a plain figure).
- Label spacing is normalised (`tidy_spacing`) so equivalent typed rows give the same figure.
- Every loop has a hard cap (lines, rows, points, handles, curves); work is bounded by a budget and tested in CPU time, not wall time
  (debug builds get a larger bound).
- Cap rows, handles and points, and say so in an editor note when something was not drawn.
- Sweep tests: run absurd values (huge, tiny, NaN-like, long odd strings) through `params_from_json` and render, asserting a
  non-trivial SVG within the CPU bound (`every_figure_survives_extreme_numbers_through_the_load_path`).
- Sweep window shapes (half-widths 2 to 30 on both axes, Square on and off) for layout rules, not just the default window.
- Census byte-identity: existing census files must not change. Dump with
  `VERSHION_CENSUS_DIR=<dir> cargo test --release --lib census_dump -- --ignored`, diff against the previous dump, and list only
  the files you meant to change. Bless an intended change with `VERSHION_CENSUS_BLESS`.
- Gate any new layout on what the figure draws (for example conic or Print) when the old figures must stay identical, and say so in
  the progress doc; if a general fix changes wide windows, record exactly which.
- Always LOOK at the rendered PNGs (Colour, Print, student, 66% bitonal) before reporting; tests cannot see everything.
