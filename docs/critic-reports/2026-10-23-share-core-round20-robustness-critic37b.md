# share-core round 20: robustness critic (critic37b)

Tree `shared-pool-phase1` working tree, `server/share-core`. wasm under test: `pkg/share_core_bg.wasm`, 1,288,848 B, mtime Oct 23 01:45, not rebuilt. Scripts: `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/tmp.NAlW9xaN4B` (`$S`; `node $S/<name>.mjs`, loader `$S/h.mjs`; native probes in `$S/p/src/bin`, own target dir). No repo file edited except this report; nothing committed/pushed/deployed; port 8799 unused; owner client files untouched. CPU = `process.cpuUsage`, min of 3 unless noted; load average 3-6 on 24 cores.

## Baseline (run alone, before the sweeps)

| check | result |
|---|---|
| `cargo test --release` | 302 passed, 0 failed, 1 ignored |
| `cargo clippy --release --all-targets` / `--target wasm32-unknown-unknown` | clean / clean |
| `bash scripts/check-wasm-exports.sh` | `ok: pkg has no test-only exports` |
| `node tests/node/check.mjs` | 506/506 |
| root `cargo test --release --test share_core_fixtures` | 8/8 |

## (a) Previous findings re-run on the new pkg

| finding | now | evidence |
|---|---|---|
| 36b B1 same-line numbered row after a lead | **FIXED** for the table leads | `m1.mjs` 18/18 refused (`Key: 1. Paris`, `Mark scheme 1 Paris`, `Correct: 1) Paris`, bank stems, fixed-order letter, TF); `b1.mjs` all 19 forms refused; `m2.mjs` matrix 200 forms: 60 publish, all 60 are leads not in the table even bare (`Keys`, `Marking scheme`, `Marking guide`, `Rubric`, `Memo`, `Marks`; `o1.mjs`: "Keys: Paris" itself publishes), documented |
| 36b B2 pair-mark characters inside pair items | **FIXED** | `c1.mjs` 14/14 refused; 2,304 new cases (`q1.mjs`: 23 item sets incl. hyphenated names, `Washington, D.C.`, scripture, date ranges, ratios, `x = 2`, `A -> B`, emoji, symbol starts x 6 separator forms x 2 joins x keep_order x 4 places) **0 published** |
| 36a B36a-1 label CPU "A. X - X -", 1000 x 26, 3.8 MB | **FIXED** | `p1.mjs`: 6 label styles x 5 units, six exports each: worst `publish_prepare` 1.97 s, `formatting_findings` 1.48 s, `label_findings` 0.44 s, with keep/strip decisions 1.1-1.5 s (was 11.5/18.2/8.6/27.8 s) |
| 36a B36a-2 pair-mark items in bank text, rights order | **FIXED** | `q8.mjs`: rights `Paris, France`, scripture, `Mr. Brown`, long rights, hyphen rights: comma, semicolon, slash and "Word bank:" lists refused in description and stem; only " and " and one-per-line wrappers publish (documented) |
| 36a B36a-3 symbol-start left items | **FIXED** | `q7.mjs`: 30 symbol-start sets (`√9`, `°C`, `$5`, `(a)`, `#include`, `+`, quotes, `/etc`, `~`, emoji, `¿Qué?`, `[1]`, `{a}`, `<b>`, `@home`, `%`, `§1`, `→x`, `-1`, `=x`, `:a`, `,x`, `>x`, `- a`, `1.5`, `½`, `(−3)`, `∞` ...) x keep_order x 6 separators x 3 places = 1,080 cases, 0 published |

Independent re-derivation of the builder's counts with different inputs: `r1.mjs`, 21 leads (incl. KEY, Ans., Acceptable answers, Correct answer, Answer key) x 20 row forms x 6 kinds (Short, Mc text shuffled, Mc letter fixed, Mc text fixed, TF true, TF false) x 4 prefixes = 10,080 cases. The mainstream forms (`1.`, `1)`, `(1)`, `Q1:`, `1:`, `1 -`, `a)`, `- 1.`) are all refused except the cases in B2 below. Published rows are otherwise exotic labels (`[1]`, `#1`, `No. 1`, `Q.1`, `1.-`), see (c).

## (b) New-class blockers

### B1. Fail-open: one invisible direction mark inside a key word defeats every key scan

**Mechanism.** The key scans fold the stem for matching, but `served_text`/`fold_key` keeps U+200E / U+200F / U+061C / U+2061-2064 / U+180E (they are not removed, unlike ZWSP, ZWJ, soft hyphen, BOM which are removed), so "Pa<LRM>ris" is a different string from "Paris" in every scan, while students are served the text unchanged and see "Paris". The lead words tolerate the mark ("Ans<LRM>wer: Paris" is refused), the key text does not. README line 715 treats these characters as high risk only inside choices; stems, descriptions and pair listings are not covered.

**Exact inputs and outputs** (`q5.mjs`, `q4.mjs`, `y1.mjs`, `q6.mjs`; `publish_prepare`, `{"hide_key":true}`; `<LRM>` is U+200E):
```
lone Short "Paris":
  "Capital?\nAnswer: Paris"                       REFUSED answer_line      <- control
  "Capital?\nAnswer: Pa<LRM>ris"                  PUBLISHED   served stem "Capital?\nAnswer: Pa‎ris" (bytes 50 61 200e 72 69 73)
  "Capital? The answer is Pa<LRM>ris"             PUBLISHED   (control without the mark: answer_line)
Mc fixed order, correct Quartz:  "Answer: Qu<LRM>artz" PUBLISHED   (control "Answer: B" and "Answer: Quartz" refused)
TF true: "Answer: Tr<LRM>ue" PUBLISHED   (control "Answer: True": tf_value)
Blank accept "osmosis": "... Answer: osmo<LRM>sis" PUBLISHED
Bank of three Short (Paris, Madrid, Rome), description:
  "Answers: 1 Paris 2 Madrid 3 Rome"              REFUSED bank_key_list,answer_line   <- control
  "Answers: 1 Pa<LRM>ris 2 Ma<LRM>drid 3 Ro<LRM>me"   PUBLISHED
  "1 Pa<LRM>ris 2 Ma<LRM>drid 3 Ro<LRM>me"            PUBLISHED   (control without marks: bank_key_list)
Matching pairs listed "Bauxite - Alu<LRM>minium; Chalk - So<LRM>ft; Slate - Ro<LRM>of"  PUBLISHED  (control: pairs_listed)
Pair listing with the mark inside a left or the separator zone: left-mid, left-end, right-mid, around " - ": PUBLISHED for LRM, RLM, ALM, U+2063, U+2062, U+2061 (`q3.mjs`); ZWSP, ZWJ, ZWNJ, WJ, soft hyphen, BOM, CGJ, MVS, combining marks are folded and refused.
```
Sweep (`q6.mjs`): all 6,033 code points that are Cf, Default_Ignorable, spaces or Mn/Me, inserted in "Pa?ris" after "Answer:": 4,201 are `forbidden_char`; the ones that publish and are invisible: U+061C, U+180E, U+200E, U+200F, U+2061-U+2064 (plus the visible Arabic and Kaithi number signs, spaces which really split the word, and a few new-script Mn). The mark is not stripped from the served text, so students still read the whole key.

**Class.** A normalisation mismatch that skips a check silently (a fold gap), one character of effort, applying to the stem rule, the bank key list, Blank, pairs and the True/False scan. It is the same mismatch family the brief named (zero-width inside items), not a new exotic marker syntax.

### B2. Hint channel: `Ans.` with a number row, and a `Key`/`Correct`/`Kunci` lead mid-line followed by a newline and a row

Round 20's same-line rule reads "Ans" but not "Ans." (the full stop is special-cased as a lead terminator), and its heading-row form still needs the lead at the start of a line. Inputs and outputs (`r2.mjs`, `r3.mjs`; `publish_prepare`):
```
Mc keep_order, correct B, after the order ack (hint "Hardest?\n" + line):
  "Ans. B"  REFUSED  "Ans: B" REFUSED  "Ans: 1. B" REFUSED  "Ans 1. B" REFUSED      <- controls
  "Ans. 1 B"  "Ans. 1. B"  "Ans. (1) B"  "Ans. Q1: B"  "ans. 1 B"  "Ans.\n1 B"        PUBLISHED
TF true ("Mars is a planet.\n" + line):
  "Ans. True" REFUSED (tf_value)   "Ans. 1. True" REFUSED   "Ans. 1 True"  "Ans. (1) True"  "ans. 1 True"  "Ans.\n1 True"   PUBLISHED
Short "Paris", stem "Capital of France? " + line (lead mid-line, row on the next line):
  "Key: Paris" REFUSED   "Key:\nParis" REFUSED   "Answer:\n1. Paris" REFUSED   "Answer key:\n1. Paris" REFUSED   <- controls
  "Key:\n1. Paris"  "Key\n1. Paris"  "Correct:\n1) Paris"  "Kunci:\n1 Paris"  "(Key:\n1. Paris)"  PUBLISHED
```
The served stem keeps the line, so the fixed-order letter, the True/False value or the key text is shown. (Short answers after "Ans. 1" are refused, the gap is the letter and True/False value, and `Key`/`Correct`/`Kunci` mid-line + newline for text keys.) The same row at line start is refused, so the author sees a differential.

## (c) Non-blockers (terse)

* **New per-text 50,000-step pair-walk limit.** Honest 3.8 MB prose bank with 200 keep_order Matching and 1,000 prose stems: publishes (`n1.mjs`, 0.83 s). An 8 KB passage repeated as the stem of 150-250 questions with 100 Matching questions whose lefts are 10-word phrases quoted from it (1,000 phrases) refuses `scan_budget` (`t1.mjs`: 100q x 10 pairs x 10-word, 150 stems, 2.2 MB, 0.92 s; 50q x 50 pairs x 12-word 0.76 s; 200q x 20 pairs x 8-word 1.45 s); 100 x 10 pairs of 6-word phrases publishes (0.87 s), as do 30 x 8. Contrived, fail closed, workable (split the passage or shorten the lefts). Not run: a text just under the limit read fully (not measured), only the refuse side.
* **CPU of the new limit across many near-limit texts (attacker shapes, 4 MB)**: `d1.mjs`/`d2.mjs` (15-30k random pair lefts of 5-40 words or 10-30 chars, 200-400 stems of 6-8 KB ending in a mark, thrash-tuned): refuse `scan_budget` or publish in 0.8-1.7 s; the 20 M global budget bounds it. No shape exceeded 2 s.
* **labels.rs caches**: `sibling_ref` memo keyed by `(first 200 bytes, long flag)` is exact (no collision: it stores the text). Native differential `p/src/bin/sib.rs`: 89,993 texts (labels, joiners, multibyte straddling the 200-byte cut, 200+ byte tails), scoped vs unscoped, two passes: **0 mismatches**. Wasm cross-call differential (`g2.mjs`): 400 random banks (Latin, Greek, Cyrillic, roman, locked choices) x 4 exports, fresh instance vs one long-lived instance with unrelated publishes between: **0 differences**. Memory over 30 mixed calls (`v1.mjs`; three distinct-text banks 2.5-3.7 MB, five exports): +19.3 MB at the first call, flat to the 30th. `SIB_REF_MEMO` is a std `HashMap` keyed by attacker text (round 17 had moved the other bank scans off hashers): hash-flooding probe (SipHash-1-3 verified against `DefaultHasher`, 12,000 same-bucket keys for k0 = 0..20 and k1 = 2, `p/src/bin/sip.rs`, `u1.mjs`): no slowdown (0.19-0.26 s against 0.18 s control), because `RandomState` bumps k0 for every map created in the instance; not exploitable as built.
* **Pair normalisation (fold) matrix** (`q2.mjs`, 4,332 cases: 19 transforms on lefts and rights (upper case, NBSP, ZWSP, soft hyphen, ZWJ, NFC/NFD, full-width, double space, tab, combining, ligatures, Turkish dotted I, circled letters, small caps, bidi wrappers), items transformed vs listing transformed, 3 separators, 2 places): all refused except the bidi marks (B1) and a Latin small-capital E, which NFKC does not fold (exotic).
* **Swap of a non-representable right** (`x1.mjs`, `s1.mjs`): rights "Paris, capital city" etc. listed in pair order are refused; reversed order, two only, prose mentions publish (no false refusal found among 168 honest bank-text cases over 7 sets: lefts-only lists, reversed rights, one pair mention, topic prose). A list of the rights' first words only ("Paris, Rome, Madrid, Berlin") publishes (paraphrase, not read). Leftover: 24 refusals in my "cap" set are the question's own `names_left` (a right "Paris, France" holding the left "France"), unrelated to the swap.
* **Heading rows and other scanners at 4 MB** (`j2.mjs`, min of 3): `Key\n1. a\n` x 480 stems 3.4 s (control 2.0 s), well under 5 s but thin; `x - ` labels 1.4 s; double-space shapes 0.9-2.4 s.
* **Exotic row labels** still publish after the lead: `Key: [1] Paris`, `Key #1 Paris`, `Key: No. 1 Paris`, `Key: Q.1 Paris`, `Key: 1.- Paris`, `Mark scheme #1 Paris`; and `Key:\n1 Paris` with the lead mid-line is B2.
* **Label-path CPU, other scripts** (`p2.mjs`, 1,000 x 26, 3.7-4.1 MB, six entry points, min of 2 on a loaded box): Cyrillic, Greek, roman, `(i)`, kana, `A:`, `A、`, circled, `[A]`, `A.-`, none x units `Х - `, `I - `, `A, B, C, `, `ア - `, `A. B. `: worst `publish_prepare` 3.14 s (first, cold shape; the same family 1.6-2.2 s), `formatting_findings` 2.04 s, `label_findings` 0.43 s, `publish_prepare` with every label kept 2.55 s. Latin styles (`p1.mjs`) at most 1.97 s. All under 5 s.
* **Fuzz**: only the 1,600-call fresh-vs-long-lived differential (0 differences) on this pkg; the 180,000-call mutation fuzz was not repeated.
* **Honest false-refusal rate** (new rules): 54 scaffolding stems x 6 kinds (324): 3 refused (2 documented `Key learning: 1.` / `Key\n1. Wall` with first choice correct, 1 fixture artefact); 11 heading variants x 7 kinds (77): 4 hard (`Key terms:` / `Use the key` + `1.` row, first choice correct, documented); 36 honest lead+number lines (`Answer 3 questions`, `Answers must be in cm`, `Key: 1 = shaded`, `Key 3 of the diagram`, `Solution 1 is shown`, `Mark scheme: 2 marks`, `Model answer 2 is below`, `Ans. in 1 line`...) x 7 kinds x 3 prefixes (756): 8 refused, all hard: `Key: 1. forest 2. desert` and `Answers 1-5 refer to the map.` with the first choice correct in a fixed order (the number reads as choice 1), and `Correct: 3 of 4 marks needed.` on any True/False question (`tf_value`). Combined 1,157 honest items: **15 hard refusals, 1.3%** (about 0.5% if the documented first-choice numbering rule is excluded), each with a workable rewrite.
* Row label and heading scope of the table (README documented): `Keys`, `Marking scheme`, `Marking guide`, `Rubric`, `Memo`, `Marks` are not leads even bare.

## (d) Not run

* A repeat of the 180,000-call mutation fuzz and of the multi-call memory growth for the heading-row and pair-walk shapes on this pkg.
* A text read just under the 50,000-step limit (only the refuse side was measured); a Matching bank of 50 pairs in 1,000 questions of honest vocabulary against honest prose stems.
* Full 4 MB near-limit text corpora of honest passages beyond the shapes above; non-English honest corpus.

BLOCKERS: 2
