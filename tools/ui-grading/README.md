# UI grading

The blind A/B grader this app's UI was scored with, kept here so a UI round can be reproduced
instead of remembered. It drives a built binary as a black box: screenshot it under Xvfb, assemble
a blind set, have independent critic models grade it, print the numbers.

Nothing here is part of the app. No `src/` file imports it, and `cargo` never sees it.

## What it answers

"Is this interface better than the reference, and by how much?" — with a number that survives the
objection that the person who built the UI graded it themselves.

## Run order

```bash
cargo build                                   # one build for every frame, see "one build per round"
python3 tools/ui-grading/capture-vershion.py shots

export UC_SHOTS=shots UC_BLIND=blind UC_REFS=refs/desktop UC_REFS_NARROW=refs/narrow
export UC_KEY=~/.local/state/ui-hill-climb/key.json
python3 tools/ui-grading/make_blind.py
python3 tools/ui-grading/verify_blind.py      # must print OK; do not grade otherwise
bash tools/ui-grading/run_critics.sh          # five critics, one .jsonl each into blind/out/
python3 tools/ui-grading/analyze.py > rN-analysis.txt
```

`UC_*` are all optional; each script also takes its directory as an argument. Editing the screen
inventory in `capture-vershion.py` and the `PAIRS` / `SOLO_*` lists in `make_blind.py` is what
retargets it at a different build of the app.

## The three things that make the number trustworthy

1. **Controls.** One or two items are the same image in both slots. A judge that splits them is
   measuring itself. `analyze.py` prints the per-judge control error before any score; identical
   pixels scoring differently means the harness is broken, not the panel.
2. **Mirrors.** A few pairs are shown twice with the sides swapped. A judge that keeps picking the
   left image is only visible here. Round 6's mistral agreed with itself 2/5, so its votes were
   discounted and the mean moved 0.3.
3. **`verify_blind.py`.** Rebuilds every blind image from `key.json` and compares hashes. This exists
   because two runs of an earlier assembler raced: 6 of 21 pairs were graded against the wrong
   screen and the "same image twice" control was two different images. That round read as a
   regression and was actually a broken set.

## Reading a round

- The **vote count** is the firm signal. Means drift ±0.3–0.5 between rounds on unchanged screens.
- Quote a range, not a decimal.
- Fix what the critics *name*; each names one located defect per image. Two rounds on the same
  screen is a real defect, one mention is taste.

## What is not in the repo

The screenshots (45 MB), the assembled blind set (22 MB) and the reference captures (2.8 MB) are
generated, not vendored. Regenerate them with `capture-vershion.py`, and point `UC_REFS` at your own
reference captures.

Round evidence kept: `docs/ui-grading/r4…r7-analysis.txt` and `rounds.html`.

## Cutting the token cost of a round

A full round is every frame times five critics, plus controls and mirrors. Two measures cut that
without touching what a judge sees, so the verdicts stay comparable:

1. **Grade only frames that changed.** Capture is deterministic: an untouched screen comes back
   byte-identical. `changed.py` compares the new shots with the last graded shots and writes a
   `UC_ROUND` file with only the differing frames (controls and mirrors are drawn from them).
   Score with `scoreboard.py NEW --carry OLD`; carried screens are marked, never silently re-used.
   (One real r5→r6 pair of shot sets: 6 of 30 frames identical. A builder that touches most
   screens saves little here; a builder that fixes two screens saves most of the round.)

   ```bash
   python3 tools/ui-grading/changed.py ui-r8/round-r6.json ui-r8/shots-r8-r5 ui-r8/shots-r8-after ui-r8/round-r7.json
   UC_ROUND=ui-r8/round-r7.json python3 tools/ui-grading/make_blind.py ...
   ```
2. **Steer with three judges, claim with five.** `UC_PANEL=steer bash run_critics.sh` runs the three
   critics that are not behind the 4.5 MB gateway, so there are no chunk restarts. Use it between
   builder runs. The 7.5 bar is only claimed on a full-panel round, and steer-round means are never
   compared with full-round means (rule 3: a changed panel resets the scale).

Not changed on purpose: the critic prompt, the four-dimension scores on both sides of a pair, the
controls, and `verify_blind.py`. Those are what the number's honesty rests on.
