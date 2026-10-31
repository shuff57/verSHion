# solubility-curves, critic 1 (science and look together)

Verdict: FAIL (1 blocker, 4 majors), all addressed in the commit that adds this file. Data and arithmetic were clean: about 4,000 printed values across four modes, ten substances, dec 0-2 matched an independent implementation; salt data matches the CRC table, CO2 and O2 match Lange.

1. BLOCKER (fixed): classification mode said a supersaturated gas "can crystallise". Gas wording now in all modes; a test scans every output for crystallis* next to a gas.
2. MAJOR (fixed): saturation-temperature student copy stopped the guide at the curve with a dot at the answer (and ran to the edge when none existed). Student geometry is now identical either way.
3. MAJOR (fixed): default six curves, labels misattributed ("KCl" nearer NaCl/NH4Cl). Labels coloured like their curves, ordered with spacing, leaders used consistently, Print labels carry the marker glyph; audit requires a label to be nearer its own curve or have a leader ending on it.
4. MAJOR (fixed): ten-curve crystallisation, T2 leader ended on the Pb(NO3)2 marker. T1/T2 now on the axis; audit checks leaders passing through other curves' markers.
5. MAJOR (fixed): NaCl saturation temperature inverted a rounded given (35.66 g printed 8 C, curve gives ~1 C). Now inverts the typed given and says "almost flat, cannot be read reliably" where the slope is too small.
6-9. MINORS: fixed (guides as obstacles, ticks kept, full-precision saturation test, wording, gas units per 100 mL stated, hydrate line, off-scale marker, distinct Print dashes). Open: a last-resort point letter can have a leader that crosses a curve.

Not checked: live editor and handle dragging; crossing-curve label placement over every ten-curve combination.
