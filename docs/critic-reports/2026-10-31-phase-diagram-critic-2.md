# phase-diagram, critic 2 (recheck)

Verdict: FAIL (1 blocker, 2 majors), all addressed in the commit that adds this file. The first round's melting-curve fix itself checked out (water and CO2 melting temperatures and point classification matched the critic's own derivation).

1. BLOCKER (fixed): Water's melting curve ran past the validity of ice Ih (about 2070 atm, 251 K), printing e.g. "solid -> liquid at -36 C" at 3000 atm. Water is now modelled only to 2070 atm; the axis stops there; isobars/points above it and isotherms below ~251 K are refused with a note. CO2 gets an extrapolation note above 7,900 atm.
2. MAJOR (fixed): the student description and data-f ids named the hidden phases and features. With any group Blank the description is generic and ids are opaque (f1, f2, ...).
3. MAJOR (fixed): in a contrived CO2 case (isotherm on Tt, isobar on Pt) blank tag 5 read as the triple point. Leaders now start and end on their own feature; the audit checks leader ends (markers within 18 px of the leader's own feature are exempt, since they coincide).
4. MINOR (fixed): pressure readings now round to 2 significant figures. 5. MINOR (fixed): water vaporisation follows the steam-table equation (within 1.5% at 100/200/300 C).
6. MINOR (open): crowded bottom-left labels on linear axes. Also open: a refused isobar/isotherm shows only in the notes; the label audit tolerance stays at one problem per 2,000 labels (52 in ~113,000).
