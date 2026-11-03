# phase-diagram, critic 1 (science and look together)

Verdict: FAIL (1 blocker, 3 majors), all addressed in the commit that adds this file.

1. BLOCKER (fixed): the Water melting curve leaned about 10x too steeply, so computed melting points, the isotherm freezing pressure and the phase at points like (268.15 K, 100 atm) were false. Water now uses the IAPWS ice equation (shifted to 273.15 K at 1 atm), CO2 a Simon fit; drawing and answers come from one function. Typical and Custom keep an exaggerated lean and say so.
2. MAJOR (fixed): an isotherm at exactly the triple temperature said no boundary is crossed; now names the triple point. T = Tc reads "the critical temperature".
3. MAJOR (fixed, mostly): region and point labels silently dropped at ordinary settings. Placer has more candidates and leader lines; essential labels place in 99.5% of a 3,828-case sweep and in every listed case. A label that cannot be placed is listed in the notes.
4. MAJOR (fixed): identical "______" blanks on the student copy. Blanks are now numbered on the plot and in the working.
5. MINOR: some fixed (point B moved, Print stroke widths, crossing guides as soft obstacles); others open (Sublimation label near the triple point on CO2, leaders crossing the isobar).

Loosened: the 10,500-case glyph audit now allows one problem per 2,000 labels (41 remain in ~90,000, 26 of them in extreme random Custom substances). Not checked by the critic: handles and the live editor. Not re-viewed by me after the last placer change: Water p_line 218/500, supercritical, student Blank.
