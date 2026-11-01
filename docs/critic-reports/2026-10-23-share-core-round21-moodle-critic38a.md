# share-core review, round 21 (critic38a): Moodle lens

Verdict: 37a B37a-1 is FIXED, and the builder's claims hold on a matrix of inputs I chose. I hunted the new code (Default_Ignorable fold, weaker
leads, mid-line lead rule) hard and found **no class-(B) blocker**. I report **6 class-(A) variants** (same families as rounds 17-21), none of which is a
new class; one of them (weak leads in another question's stem) contradicts the README sentence that describes it.

Classification used: (A) = a variant of the key-lead / numbered-row / pair-list / label-path families; (B) = a genuinely different new class
(new hint channel, honest hard refusal with no workable path, wrong freeze exposing the key by position, wasm trap, fail-open cap, CPU > 5 s min of 3 on 4 MB).

## Build, environment, baseline

wasm copy of `server/share-core/pkg`, never rebuilt: sha256 `ecc10fb6...2c17`, 1,289,686 B, mtime 05:11.
Scripts in `$D` = `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/tmp.n8SpPC8vFx`
(`h.mjs` loader and builders; `cd $D && node <script>`). CPU = `process.cpuUsage()` around `publish_prepare(bytes, '{"hide_key":true}')`, min of 3.

| baseline (run alone) | result |
|---|---|
| `cargo test --release` (share-core) | 309 passed, 0 failed, 1 ignored |
| `cargo clippy --release --all-targets` | clean |
| `node tests/node/check.mjs` | 506/506 |
| `cargo test --release --test share_core_fixtures` (root) | 8/8 |

## (a) Previous findings

| finding | command | result |
|---|---|---|
| 37a B37a-1, lead ending a line after prose + numbered row on the next line (TF, fixed-order Mc, Short, shuffled Mc, part stem, another question's stem) | `node gap2.mjs`, `node gap3.mjs` | **FIXED**: all 13 leads x 7 rows refused (`answer_line`); TF and fixed order 10 of 10 leads refused; served stem not published |
| independent matrix, newline form: 6 prefixes ("Which gas do plants absorb? ", "Solve for x. (3 marks) ", "Read the text above and decide. ", "Q4) Name the largest planet. ", Spanish "¿Cuál es la capital? ", "Check your work. Then answer. ") x 16 leads (Key, Correct, Kunci, Answer, Answers, Answer key, Solution, Solutions, Ans, **Ans.**, Mark scheme, Model answer, Acceptable answer, Sol, Keys, Correct answer) x 6 row forms (1. / 1) / (1) / Q1: / 1 - / a)) x 6 kinds | `node ind.mjs` | 3,456 cases: **0 published** for Short, shuffled Mc, fixed Mc, TF, part stem; 20 published in "another question's stem of a bank", all with the weak lead `Keys` (A1 below) |
| same matrix, same-line form (`Lead: row` on one line) | `node ind.mjs` | 3,456 cases, the same 20 (weak lead in another stem) published, 0 others |
| `Ans.` lead, row on same line or next | `ind.mjs` (lead `Ans.` in the matrix) | refused in every kind |
| 36a/37a earlier repros (pair marks inside items, symbol-start items, label-path CPU, same-line rows) | `hy2.mjs`, `sym.mjs`, `wo2.mjs`, `up3.mjs`, `rm4.mjs` family | all still fixed (not re-run individually beyond `timings21.txt`; the CPU shapes are in the table below) |

Independent check of the Default_Ignorable fold: `node ig2.mjs` puts 11 ignorable characters (ZWSP, ZWJ, LRM, soft hyphen, word joiner, VS16, CGJ, U+180E,
invisible separator, BOM, ALM) at every inner position of 19 leak templates (marker word in the correct choice, wrong words on wrong choices, answer line, TF restatement,
GIFT braces, Cloze, GIFT TF feedback, Aiken letter, topic = key, bank key list, pairs listed, rights order, heading + row, same-line numbered row, mid-line lead + row,
blank hint, part states key, left names right): **2,278 insertions, 0 published** where the plain text is refused (all refused with the ignorable inside).

## (b) New-class blockers (B)

None. How hard I hunted (all in `$D`, outputs quoted in the sections below):

* Fold: 2,278 insertions above; honest corpora (18a 551, 20a 595, 23a 614, 24a 727 questions) re-run with 1-2 random ZWJ/ZWNJ/LRM/WJ/SHY per text (`corp3.mjs`): **0 newly published, 0 newly refused for a key rule** (the 24 newly refused are my sprinkle landing inside the `___` blank underscore runs: `too_few` / `blank_count_mismatch`). Offset check: the fold changes only the scan copy; the served stem is unchanged (`ig2`, `corp3`, `lbl.mjs`: served texts keep their characters).
* Scripts that need ZWJ/ZWNJ (`node lang.mjs`): Persian, Hindi, Tamil, Malayalam, Sinhala, Khmer (ZWSP word breaks), Bengali, Arabic, Urdu, Burmese; 8 question shapes each (Mc, Short, TF, Matching, Blank, a 3-question bank with name/description, "Key terms 1." Mc, a stem with "Answer 1. ... Marks: 3 of 4. Memo: ..."): 80 cases, each also run with the joiners removed. Refusals: 3 with joiners, 1 without. The differences: Khmer and Burmese `forbidden_char` (see A6), and the one refusal in both versions is my own test: Hindi Blank stem holding "क्‍ष" (ZWJ) while the accepted answer is "क्‌ष" (ZWNJ), which fold to the same string, so `answer_line` (a stem that shows the key; honest orthography questions that print both forms of one conjunct would need the form removed). No false refusal of an ordinary text in any of the ten scripts.
* Weaker leads, honest rate (`weak3.mjs`, 13 honest stems): "Marks: 3 of 4", "Marks: 1 mark each", "Memo: bring a ruler", "Rubric" headings with essay criteria, "Marking scheme: 1. Method (2) 2. Answer (1)", "Keys 1 to 3 are white", "Marks 2. Name the capital of France" all publish. Refused (low risk, `key_heading_row`, publishes after `ack_formatting`): "Memo\n1. Bring a ruler ...", "Keys: 1. House key 2. Car key", "Rubric: 1. Paris", "Marks: 1) Paris 2) Rome 3) Madrid" where the row is literally the key. Honest false-refusal rate on that list: 5 of 13 low-risk, 0 hard. Acceptable.
* Mid-line lead rule (`lay.mjs`, 18 layouts x TF / fixed / Short x Key / Answer): refused for lead then newline, trailing spaces, blank line, indent, CRLF, tab, U+2028, VT, NBSP before the lead, lead glued to prose, bullet before the lead, "Teacher use only: Key:" prefix, a second row. Published layouts are all class A (A3).
* CPU, all known 4 MB shapes: table below, worst 2.41 s. New ignorable-heavy shapes (`cpu5.mjs`: 400 stems of 8 KB where every character is followed by ZWJ or ZWSP; key rows, key list, pairs, rights, mid-line lead, labels, only ignorables) 0.07-0.71 s.
* Traps: `fz2.mjs`, 3 seeds x 4,000 banks x 5 exports = 60,000 calls over random banks with ignorable characters, new leads (`Keys:`, `Marks:`, `Memo:`, `Rubric`, `Ans.`, `Prose. Key:\n`, `Answer:\n1 `), marks, symbols and scripts: 0 traps, 0 internal errors.
* Fail-open caps: README-documented `scan_budget` shape not re-attacked; previous boundary probe (`lim.mjs`) still `pairs_listed` below the limit and `scan_budget` above it, never CLEAN.

BLOCKERS(B): 0

## (c) Variants (A): same families, one-line repro each (6 new)

All repros are `publish_prepare` with `{"hide_key":true}` (plus `order_acks` for fixed order); `\n` is a line break.

1. **A1, weak leads in ANOTHER question's stem of a bank are published** (README round 21 says `answer_line`). Bank: Q1 "Largest planet?" Short "Jupiter"; Q2 stem `Memo: 1. Jupiter\nFiller.` Short "zz": CLEAN. Same for `Keys:`, `Marks:`, `Rubric:`, `Marking scheme:`, `Marking guide:`, own-line or same-line, mid-line (`weak.mjs`, `weak2.mjs`); the bank description is refused (`answer_line`) and the strong lead `Key:` in the other stem is refused. The leak is the Short key text of a sibling question. Family: key-lead / numbered row (weak leads).
2. **A2, a bullet after a lead**: stem `Capital?\nKey: • Paris`, Short "Paris": CLEAN (bare `Key: Paris` is refused). Documented family (exotic row labels).
3. **A3, three more key-lead layouts**: `Capital? Key -\n1. Paris` (dash after the lead) CLEAN; `Mars is a planet. (Answer)\n1 True` (TF true) CLEAN; `Hardest?\n| Answer | 1. B |` (fixed order, correct B) CLEAN (`fin.mjs`, `lay.mjs`; `Key:` in the table cell also CLEAN for TF). The README says remaining layouts are documented limits.
4. **A4, an ignorable character inside a choice label defeats label detection** (label path, not the key scans): choices `a​) Mars`, `b​) Venus`, `c​) Earth`, `d​) Jupiter` (U+200B after the letter) give no label finding and publish shuffled with the letters scrambled (`lbl.mjs`, `lbl2.mjs`: ZWSP, ZWJ, ZWNJ, LRM, SHY, WJ, ALM, U+180E, invisible separator, CGJ in "a) X", "(a) X", "A. X", "1) X", "a. X"; "X) a" keeps the finding). With the plain labels the same bank is `labels_need_decision`. The correct choice lands uniformly (50,48,50,52 of 200), so no position leak, but the author's label decision is bypassed.
5. **A5, an ignorable character inside a sibling reference is not frozen**: `All​ of the above` (ZWSP), `All of the‍ above` (ZWJ), `Non­e of the above` (SHY), `All of the above​` (trailing ZWSP) publish shuffled with the reference choice landing in every position (28,20,22,30 of 100); the plain forms need an order acknowledgement and stay last (`ref2.mjs`). `Both A​ and B` is still frozen; `Both‎ A and B` is refused as `invisible`. Not a key exposure (a reference choice that moves is a broken question, not a leak); same label/sibling-reference path as A4.
6. **A6, Khmer / Burmese bank NAME with ZWSP word separators is `forbidden_char`**: `bank(name = "ខ្ញុំ​ស្រលាញ់")` refused, while the same text in a stem or the description publishes (`fin.mjs`, `lang.mjs`). Pre-existing, workable (remove the separators from the name), not a round 21 change.

VARIANTS(A): 6 new in this round (plus the documented carry-overs from 37a that I did not re-measure: row labels `A.`, `i.`, `[1]`, `No. 1:`, `Question 1:`, decorated leads `**Key:** 1. Paris`, look-alike lead letters, lead words not in the tables).

## Timings of the known 4 MB shapes (node on pkg/, min of 3, CPU seconds; `timings21.txt`)

| shape (script) | bytes | min |
|---|---|---|
| 35a: 480 keep_order Matching + 480 "Yes, No, Maybe, " stems (`cpu1.mjs`) | 4,041,369 | 1.28 (control 1.21) |
| 35b "x - " 1000x26 / 950x26 / 600 / 300 / 150 q / 1000x12 / 1000x4 (`lab3.mjs`) | 3.66-4.08 M | 1.47 / 0.99 / 0.89 / 0.78 / 0.76 / 0.83 / 0.95 |
| "x -- ", "a - ", "x - x ", "1 - " (scan_budget), "x -" 950x26 | 3.9-4.0 M | 1.08 / 0.93 / 1.02 / 1.12 / 1.13 |
| 36a "A. X - " 1000x26: upper / upper_lower / lowerlab_upperx / noLabel_upperX / upper_paren / upper_Y (`up.mjs`) | 3.76-3.82 M | 1.25 / 1.24 / 1.29 / 1.35 / 1.28 / 1.21 |
| per export, upper shape (`up2.mjs`): validate / validate_for_publish / strip / formatting_findings / label_findings / match_hash / publish_prepare none, keep, strip, auto | 3,815,209 | 0.08 / 1.08 / 1.11 / 1.03 / 0.31 / 0.13 / 1.24, 1.47, 1.17, 1.48 |
| refusal shapes (`lab5.mjs`, 9 shapes) | 2.7-4.1 M | 0.76-1.58 |
| key rows / answer lines / key bank cross / tf cross / letters (`cpu2.mjs`) | 3.1-4.2 M | 1.47 / 1.62 / 0.43 / 1.33 / 0.43 / 2.41 (cross bank) / 0.17 / 1.66 |
| pair scan hostile "a " / "ab " 1 and 20 Matching (controls 0.56 / 0.62) (`cpu3.mjs`) | 3.4-3.5 M | 0.85 / 1.48 / 0.86 / 1.47, all `scan_budget` |
| round 20 shapes (`cpu4.mjs`): same-line short / mc26 / leads / leads_nums / keyterms / tf / fixed / pairs_weird / swap x2 | 1.3-3.6 M | 0.30 / 1.55 / 0.83 / 2.11 (scan_budget) / 0.48 (scan_budget) / 1.44 / 1.21 / 0.20 / 0.45 / 0.25 |
| round 21 ignorable-heavy (`cpu5.mjs`): key rows / key list / pairs / rights / mid-line lead / labels / only ignorables, ZWJ and ZWSP | 1.1-2.0 M | 0.14 / 0.24 / 0.27 / 0.43 / 0.31 / 0.71 / 0.07 |
| `blank170` (`check.mjs`) | 3,690,848 | +29.4 MB, ok |

Worst case: 2.41 s. Nothing near 5 s.

## (d) Not run

* The 627-item and 158-marker corpora of critics 31a/34a (scripts gone); `--ignored` amplification sweep, mutation runner; `cargo test` once, not three times.
* The `tests/round21.rs` Cf-canary was not re-derived (it runs in the baseline, 309 pass); I did not walk Unicode Cf myself.
* An old-binary differential (the round-20 pkg is gone), shuffle fairness statistics (round 21 did not touch the shuffle), the Worker end to end, Argon2id, importers.
* Persian/Hindi/Tamil/Malayalam/Sinhala/Khmer texts were written by me (short sentences with the joiners placed where the scripts use them); no native-speaker review, so a spelling in the samples may be unnatural. No source review of the fold code.

BLOCKERS(B): 0
VARIANTS(A): 6
