# reaction-rate-graph, critic 1 (science and look together)

Verdict: FAIL on one MAJOR, no blocker. The major is fixed in the commit that adds this file.

Checked: an independent Python recomputation of every printed key value over 792 cases (3 modes, 3 orders, both given modes, k 1e-6..1e6, a0 1e-6..1000, dec 1..4, tmax short/long/clamped, tangent times to 1.2e8); about 60 renders (Colour/Print, Large/Medium/Small, student/key); two 300-case student-copy leak sweeps; chemistry wording. Not checked: drag handles and live UI, cargo tests, export paths, time units other than seconds.

1. MAJOR (fixed): Zero order, Curve given, student copy named the end of the reaction in a legend entry, a blank line and the screen-reader description, which gives the order away. Now gated on the key or Order-and-k-given; the open circle stays. The leak test forbids "end of the reaction" and "reaction is over" on that copy.
2. MINOR: Rate vs concentration, Zero order: the open circle at [A] = 0 has no legend entry, no closed dot at (0, 0).
3. MINOR: values written at an axis use scientific form ("5.00 x 10^-2") beside decimal ticks.
4. MINOR: written-in axis values use `dec` decimals beside bare ticks.
5. MINOR: a tick's mark stays when its number is dropped for a written value; one case loses its last tick number (Rate, Zero, k 3, a0 0.4, a_rate 0.39).
6. MINOR: Zero order key, product on, Large: the "[A]" label is 29 px from [A] and 36 px from [P].
7. MINOR: Rate, Second order, k 1e6, a0 1000, a_rate 0.001 prints "1.00 x 10^-12" beside a working that says 1.00 M/s (consistent, reads oddly).
8. MINOR: half-life markers do nothing in Linearised and Rate modes; "Time shown" does nothing in Rate mode; not stated in the hints.
9. MINOR: the tangent point can hide the [A]0/2 dot or the end circle (t_tan 19.99, k 0.05).
