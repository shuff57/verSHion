# Figures parity ledger

One row per round, written by `ledger.py close` from `score.json`. A round whose
capability did not pass, or whose score did not move, is a **miss** and stays.

| round | date | figure | capability | figure score | pooled | probe | checker | result | evidence | next |
|---:|---|---|---|---:|---:|---|---|---|---|---|
| 1 | 2026-11-13 | `conic-sections` | cap-01: New figure conic-sections: all 46 checklist entries built and accepted on the fourth independent check | 0.0 → 100.0 | 0.0 → 12.9 | pass | accept on the fourth independent check (NAS, aaf9497): all 46 entries pass; 2,694 swept figures with nothing missing, on text or off the page; side list only on named tiny conics; cosmetic faults listed | miss | conic and census tests 48 passed, 0 failed | tangent-secant-lines and riemann-sums (second checks running); derivative-graphs (building) |
