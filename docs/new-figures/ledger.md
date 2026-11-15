# Figures parity ledger

One row per round, written by `ledger.py close` from `score.json`. A round whose
capability did not pass, or whose score did not move, is a **miss** and stays.

| round | date | figure | capability | figure score | pooled | probe | checker | result | evidence | next |
|---:|---|---|---|---:|---:|---|---|---|---|---|
| 1 | 2026-11-13 | `conic-sections` | cap-01: New figure conic-sections: all 46 checklist entries built and accepted on the fourth independent check | 0.0 → 100.0 | 0.0 → 12.9 | pass | accept on the fourth independent check (NAS, aaf9497): all 46 entries pass; 2,694 swept figures with nothing missing, on text or off the page; side list only on named tiny conics; cosmetic faults listed | miss | conic and census tests 48 passed, 0 failed | tangent-secant-lines and riemann-sums (second checks running); derivative-graphs (building) |
| 2 | 2026-11-14 | `limits-discontinuities` | cap-01: New figure limits-discontinuities: all 34 checklist entries built and accepted on the sixth independent check | 0.0 → 100.0 | 12.9 → 22.7 | pass | accept on the sixth independent check (NAS, b845ce8): all 34 entries pass; 104 new ordinary functions on three windows, 13 epsilon-delta cases, 5 point graphs, 30 spellings: nothing false written as established; 7,972 renders under 0.19 s; known issues: 6 of 832 answers cannot determine, some spellings refused with a note, Large label placement | miss | limits suite 124 passed, 0 failed | derivative-graphs, tangent-secant-lines, riemann-sums, sequences-series, complex-plane |
