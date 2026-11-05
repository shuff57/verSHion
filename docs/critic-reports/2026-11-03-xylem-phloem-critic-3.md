# Xylem and phloem, critic 3, commit c27a3e8

Verdict: PASS. 0 blocker, 0 major, 0 MED, 5 LOW, 6 minor. (Condensed; tools in scratchpad/xylem-crit3/.)
Method: 2,304 SVGs with resvg ink boxes, own analysers (an3 pointers and arrow-blank last legs, an4 applies every note's advice over 1,152 Colour states), 22 own mutants, PNGs (opening key/student, Print 300 dpi and 66% grey, lengthwise pointers, five examples). Full figures:: stopped at 45 min under load: 863 passed 0 failed of ~1,900.

## LOW (left open on purpose)
1. The new arrow-distance test checks only the default label set: mutant M5 (Xylem vessel pointer back to 4.8 px from a water arrowhead, only when show_layers is off) survives all xylem tests, start tests and census_baseline. Figure is fine today (min over 1,152 states, end to shaft / end to head: vessel 13.0/10.4 px, sieve tube 9.24/19.72, sieve plate 10.46/11.66, companion cell 17.87/15.24). Fix: loop the test over all 32 masks.
2. "Nothing is labelled" note (Stem, lengthwise off, layers and xylem/phloem off, cells/arrows/root extras on) never says the switches already on wait for the lengthwise view or a root; removing the two hidden-panel notes is otherwise sound (360 + 120 silent-wait states, no loops) but hints show on hover only and "Transport arrows" does not say it needs the lengthwise view. Fix: name Longitudinal view/Cross-section in that note; consider the label "Transport arrows (lengthwise view)".
3. Example 3 ("Which way do water and sugar move? Student copy", 6.2 pt): its only note advice (switch off Longitudinal view, ~9.6 pt) removes the whole task. Do not offer a step that removes every arrow blank.
4. Reserved label columns cost print size (corrects critic 2 LOW 3): in 98 of 1,152 states width-limited, an empty band of 150-245 px costs 0.3-0.9 pt (Stem+lengthwise masks 02/06 student Large 6.72 pt, ~7.6 without; Stem alone student Large 8.42, ~9.3 without; example 3 6.18, ~6.4). Fix: size each panel's label column to the widest name or disc actually drawn.
5. Small-print filter no_new_note compares note text, so in 50 states already showing "Nothing is labelled" a step that only changes that note's wording is not offered. Compare which notes fire, not their text.

## minor
M16 (printed size without ceil) equivalent; M17 (step order) no promised order; M1 (vessel line ends on a ring thickening) caught only by the census hash; "the lengthwise view goes" reads oddly; "~11 px" claim is really 13.0 px shaft / 10.4 px head; vessel line ends 2 px into the wall (a student might write "cell wall"); Both-to-Stem step does not say root labels go; opening bracket test bound lowered 3.0 to 2.0 pt (measured 2.16).

## Verified
All pointers in 1,152 Colour states end in the part they name; Print geometry identical; no collisions; arrow blanks 0.60 px off their own shaft halfway along, next arrow 36.8 px; Sucrose last leg horizontal 6.2 px. Printed sizes recomputed match builder and FAQ table (all three panels 4.81/3.94/3.39; Stem+lengthwise 5.25/4.66/4.20; Stem alone 8.42/7.17/5.94; Root alone 8.70/7.76/6.51; opening 5.25 key / 5.39 student). All 2,486 offered note steps applied: none loops, none raises a new note, each size step >= 0.5 pt bigger within 0.05 pt, lengthwise-off steps say what they remove. Opening state: 12 arrows, one note offering the stem alone at 8.4 pt; student copy 16 blanks, direction shown, Source/Sink discs, answerable. Examples all Large; stem and root student copies 8.42/8.91 pt note-free. No biology wording changed; OpenStax 30.5 agrees. 0 leaks in 1,152 student SVGs. Regression: xylem module + census + start + faq 39 passed 3 ignored; handles 5 passed; census differs from 244e33a only in the two xylem lines.

## Biggest gap vs textbook
Lengthwise view is two schematic tubes with no link to the cross-section; no xylem supply to the leaf, no transpiration exit.

## Not checked
Live egui, real PDF/DOCX, other fonts, photocopier, Campbell/Raven, the rest of the full figures:: run.
