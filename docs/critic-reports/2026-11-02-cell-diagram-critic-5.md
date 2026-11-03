# Cell diagram, critic 5 (verification), commit e485cc8

Verdict: PASS. 0 blocker, 0 major, 0 MED, 6 LOW, 7 minor.
(The critic could not write its report file; condensed from its final answer. Probe, scripts, dumps and zoom crops: scratchpad/cd-crit5/.)

The stud, pointer and leader problem that failed rounds 1-4 is fixed and guarded by picture tests. 3,326 renders (cell types x plasmodesmata x 31 group sets x captions x 3 sizes x key/student x Colour/Print, plus defaults, opening state and 6 examples at every combination), 26,523 leaders.

## Invariants measured independently
(a) exactly 4 drawn studs within 12 px of all 1,953 Rough ER pointers, never 3 or 5, at every size/captions/copy/style incl. examples (the 4 at 10.6 and 11.19-11.255 px, so the 4th is 0.745 px inside 12; the 5th is 15.93 px away; none within 10 px); (b) 0 studs within 12 px or 15 px of a pointer left out; I1 0 violations (min clearance 1.03 px beyond radius + halo); clearance from the Rough ER's own leader at least 6.15 px from any stud centre; (c) no stud overlaps anything outside its own sac (nearest other ink: plant Smooth ER 2.84 px, animal 4.48 px; nearest other stud 1.03 px); (e) Rough ER leader crossings 0 of 1,953; 0 intruders within 6 px of a drawn end; leader-leader 0 crossings, min 7.005 px; 0 text overlaps; crossings 81 of 5,712 grid leaders (1.42 %), none over a ribosome; leader lengths as claimed (sets 256 px without captions, 259/299/336 with; grid longest 406 px with captions at Large); longest boundary run 39 px. 0 leaks in 1,658 student copies; all 1,668 keys correct. Tests 35 to 35 names, assertion lines 239 to 242, census only the two cell-diagram ids, module 35/35, faq 15/15, start 5/5, handle 23/23, full figures:: without the module 1,827 passed 0 failed.

## LOW
1. Crossings in the defaults, opening state and examples at other sizes with captions (the test checks them as-is only): plant opening state, plant defaults and example 1 at Large with captions, the 164 px Smooth ER leader cuts three Golgi cisternae (visible in Colour and Print); example 1 at Medium with captions, a 299 px Ribosome leader skims the vacuole top; animal defaults and examples 0 and 4 at Large with captions, the Nuclear envelope leader's halo grazes a cytoskeleton fibre end (invisible). The FAQ "for the defaults, the opening state and the examples, none does" is false. Fix: run the must-be-clean check over these states at every size x captions, or correct the FAQ.
2. 406 px leader unchanged: animal masks 9/12/13, Large with captions, the Nuclear envelope leader runs diagonally to a slot about 280 px below the cell while the left column holds 2 labels. Fix: penalise slots beyond the cell's extent or prefer the emptier column.
3. Latent panic in `compute`: `if let Some((slots, st, 0, x)) = chosen.take()` discards the best layout whenever it has a violation, then line 1305 indexes an empty vector ("index out of bounds: the len is 0"). Cannot happen with today's finite swept parameter space (0 violations everywhere) but it is one geometry change away (mutant L04, plant left only its 99 degree pointer place, panics 23 of 35 tests); the code predates this round (b587bcb) but the new gate makes it easier to hit; it breaks "render never panics". Fix: match on a reference/clone; add a test that forces a violation.
4. The 20 s speed bound still flakes under load (mutant L01, which only moves a plasmodesma, failed with the animal scene at 26.5 s; own baseline 9.5 s at load about 80) and cannot tell a real slowdown from load (mutant L08, 7x raster work). Fix: CPU time or count the work.
5. The Smooth ER does not join the Rough ER: 1.17 px (plant) and 1.52 px (animal) white gap between outlines, yet the FAQ says "smooth ER continuing from it" and the code comment says it leaves the end of the outer rough sac; a plant stud sits 2.84 px from the Smooth ER tube. Fix: overlap the tube with the sac's end like the envelope join and keep studs at least 4 px from the Smooth ER.
6. Plant proportions: the plant nucleus shrank from r 36 to r 30 (60 px across), narrower than the Golgi stack (76 px tall) and near a chloroplast (52 px); the plant Rough ER is about 60 x 52 px with 21 studs, still smaller than the Golgi; the studs-on-sac test needs at least 20 and the plant has 21 (one stud of slack).

## minor
1. One plant Rough ER pointer place carries 1,148 of 1,152 plant pointers (the 99 degree place is used 4 times).
2. The FAQ says the Rough ER leader leaves "straight out"; measured exit up to 22.1 degrees off.
3. Plant mask 29, every size: the Nucleus leader skims the left tips of all four sacs, parallel to the Rough ER's own leader.
4. The 66 % grey wall test now skips a dark run at the start of each scan line (a small loosening).
5. The moved lysosome is 2.5 px from the animal membrane; the new plant fibre ends 10 px above the bottom plasmodesma and looks as if it feeds into it.
6. Free ribosomes left out: as few as 14/18 (animal), 15/18 (plant), 14/18 (plant with plasmodesmata), 24/28 (bacterium).
7. The unit assertion "a kept stud costs two structures" was removed but the path is live; held only by the picture test (mutant P01 caught by it).

## Mutation (22 own mutants)
17 caught by picture tests (gate removed/tightened/on every leader, ROSE 12, stud on the outline, narrow gap, no outer-face studs, no bare zone, studs on one side only, stud r 2.1, thin sacs, protected studs free for other leaders, all studs unprotected, protection radius 10.9, narrower omission radius, cost on the centre line only, no outline redraw); sparse studs only by the 20-stud count; L04 by the panic (LOW 3); L08 by the speed test (which load alone can also trip). Harmless survivors: "never leave a stud out" (no leader comes within 5.7 px of a stud; I1 would catch it), moving the plasmodesma back to the left wall (caught only by the pinned anchor hash and a speed flake).

## Teacher verdict and biggest gap
All three cells can go into an exam at the as-is size. The animal Rough ER now looks like a textbook drawing (three studded concentric sacs joined to the envelope); the plant Rough ER clearly reads as rough rather than smooth. Biggest gap vs OpenStax Fig 4.8 / Campbell: the plant endomembrane system is still token-sized next to its own Golgi, under a small nucleus, and the Smooth ER is not continuous with the Rough ER. Check the plant opening state at Large with captions before relying on it.

## Not checked
Release-build timing; live egui editor; PDF/DOCX; browser font metrics; a real photocopier; reference figures were compared from memory; about 25 PNGs plus zooms looked at.
