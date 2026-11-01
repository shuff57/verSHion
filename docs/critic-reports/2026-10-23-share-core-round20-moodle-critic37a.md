# share-core review, round 20 (critic37a): Moodle lens

Verdict: my three round-19 blockers (36a) and the robustness critic's two (36b B1, B2) are FIXED, and the swap, the
pair-walk limit and the label caches held up under differential tests. I found **one** new blocker: a numbered key row
on the line after a lead that sits mid-line is read by nothing (TF and fixed-order Mc: 20 of 20 leads published).

## Build, environment, baseline

wasm copy of `server/share-core/pkg`, never rebuilt: sha256 `0bd3ef56...6bb6223`, 1,288,848 B, mtime 01:45.
Scripts in `$D` = `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/tmp.n8SpPC8vFx`
(`h.mjs` loader and builders; run `cd $D && node <script>`). CPU = `process.cpuUsage()` around `publish_prepare(bytes, '{"hide_key":true}')`, min of 3.

| baseline (run alone) | result |
|---|---|
| `cargo test --release` (share-core) | 302 passed, 0 failed, 1 ignored |
| `cargo clippy --release --all-targets` | clean |
| `node tests/node/check.mjs` | 506/506 |
| `cargo test --release --test share_core_fixtures` (root) | 8/8 |

## (a) Previous findings (re-run on the new pkg)

| finding | command | result |
|---|---|---|
| 36a B36a-1 CPU, "A. X - X - ..." 1000 x 26, 3,815,209 B | `node up3.mjs`, `node up.mjs upper 1000` | **FIXED**: publish_prepare 1.28 s (was 11.5), formatting_findings 1.49 (was 17.8), label_findings 0.43 (was 8.6); `upper` min 1.41 (was 13.5), `upper_paren` 1.74 (was 12.3). Controls 1.2 / 0.95 / 0.26 |
| 36a B36a-2 pair marks inside items | `node hy2.mjs`, `hy3.mjs`, `wo.mjs`, `wo2.mjs` | **FIXED**: every `- , : = > –` x left/right case is `pairs_listed` in the stem and in the bank description; "Key:" + hyphenated pairs refused; `Paris, France; Rome, Italy; Madrid, Spain` over a `keep_order` column is `word_bank_order` (was published) |
| 36a B36a-3 symbol-start items | `node sym.mjs` | **FIXED**: 16 of 16 sets refused in the bank description, the stem and the arrow list (was 5 of 16 in the description) |
| 36b B1 same-line numbered row | `node rm4.mjs` (my matrix, below) and `rm2.mjs`, `rm3.mjs` | **FIXED for the lead table and the row forms the README names**; see the independent matrix and gaps below |
| 36b B2 = my B36a-2 | as above | **FIXED** |

**Independent check of the builder's "440/440 lead x row-form" claim** (different inputs: 14 table leads, 7 separators incl. `:\n`, tab, en dash, double space,
10 row forms, line start / mid line, 5 question kinds; `node rm4.mjs`): 9,800 cases, 461 published. Of those, the row `Key: • Paris` (bullet, no number) and the
"lead mid-line, row on the NEXT line" shape (B37a-1) are all of them for Short/Mc/part; for fixed-order Mc and TF the published ones are the same newline shape. With the lead and row
on one line, or the lead at the start of its line, the table leads are refused in every kind. Row labels outside the README list
publish (non-blocker, see (c)): `Key: A. Paris`, `i.`, `[1]`, `No. 1:`, `Question 1:`, `#1`, `Q1 Paris`.
**Different inputs, the 36a pair claim "567/567":** random Matching items (96 atoms: symbols, marks, CJK, diacritics, scripts), 2-5 pairs, listing 2+ pairs with 20 separators and
wrappers, in the description, the name, or a stem: `node pf.mjs 5..8 4000`, 16,000 cases, **0 not refused**.

## (b) New-class blockers

### B37a-1. A key row on the line after a MID-LINE lead is read by nothing (TF and fixed order: 20 of 20 leads published)

Class: fail-open (a rule that silently skips a layout of the channel it was added for). Round 19 reads a `Key`/`Answer` heading that stands ALONE on its line
followed by a numbered row; round 20 reads a numbered row on the SAME line after a lead. The layout in between, text then lead
on one line and the numbered row on the NEXT line ("Which is hardest? Answer:" newline "1. B"), falls into neither: the lead is not alone on its line, and the row is not on the lead's line.
Controls: the same text with the lead at the start of its line is refused; the un-numbered row ("Answer:" newline "True") is refused.

Exact input (`node gap2.mjs`, `node gap3.mjs`; `publish_prepare` with `{"hide_key":true,"order_acks":[{"question":0}]}`; stem is the quoted text, `\n` a line break):

```
TF, correct = true      "Mars is a planet. <lead>\n1 True"          PUBLISHED for all 10 leads
                        control "Mars is a planet.\n<lead>\n1 True"       answer_line (10 of 10)
                        control "Mars is a planet. Answer:\nTrue"          tf_value
fixed-order Mc [Talc, Quartz, Diamond, Gypsum], correct B (keep_order, order acked)
                        "Hardest? <lead>\n1. B"                     PUBLISHED for all 10 leads
                        control "Hardest?\n<lead>\n1. B"                  answer_line (10 of 10)
                        control "Hardest? Answer:\nB"                      answer_line
   leads: Key:, Correct:, Kunci:, Answer:, Answers:, Answer key:, Solution:, Ans:, Mark scheme:, Model answer:
Short "Paris" / shuffled Mc (key text Quartz) / part stem / another question's stem in a bank:
                        "Capital of France? Key:\n1. Paris"          PUBLISHED (also Correct:, Kunci:, and Key / Correct without a colon)
                        rows 1. / 1) / (1) / Q1: / 1 / a) all PUBLISHED; the row "• Paris" is refused
                        control "Capital of France?\nKey:\n1. Paris"      answer_line
   with Answer: / Answers: / Answer key: / Solution: / Ans: / Mark scheme: / Model answer: the Short and shuffled-Mc rows ARE refused (another rule reads "Answer:" then the key text), TF and fixed order are not.
```
The served stem keeps the text: `stripped.payload.question.stem = "Capital of France? Key:\n1. Paris"`. For TF the value and for the fixed order the letter (by position) is the key.
Prefix text does not matter: five different prefixes ("Capital of France? ", "Read the passage. ", "Name the capital. (2 marks) ", "Q1. ...", two spaces) give the same 9 of 40 (Short) published.
Why counted: it is the README's own example ("Answer:\n1 True") with one sentence before the lead on the same line; the honest control (a legend "Key terms", "Key points") is unaffected by a fix that reads the next-line numbered row after any lead word that ends its line (the whole row = key).
Judgment call: if the convergence judge treats a layout of an already-fixed family as a same-class variant, the count is 0; I count it because the fix claims the layout is read, for 20 of 20 leads with TF and fixed order the leak is complete, and the same text at line start is refused.

## (c) Non-blockers (terse)

* **Row labels beyond the README list** with a table lead and the key on the same line (`rm3.mjs`): `Key: A. Paris`, `i.`, `[1]`, `No. 1:`, `Question 1:`, `#1`, `1 )`, `Q1 Paris` (Short, shuffled Mc). The lettered `a)` is read, the capital `A.` is not. Same family (label shape).
* **Decorated leads**: `**Key:** 1. Paris`, `_Key_: 1. Paris`, `<b>Key:</b> 1. Paris`, `` `Key:` 1. Paris `` publish for Key and Correct (bare `**Key:** Paris` is refused); `**Answer:** 1. Paris` is refused (`bold.mjs`). Documented decorated-heading family.
* **Lead words not in the table** (documented): Marking scheme, Memo, Answer sheet, Right answer, Teacher key publish with a numbered row.
* **Look-alike lead**: Cyrillic `Кey: 1. Paris` publishes (documented look-alike limit); fullwidth, NBSP and zero-width forms of the lead are refused.
* **Key-list scan skips keys with one alphanumeric character after a symbol**: `°C`, `∠A`, `×2`, `¬p`, `∑x`, `∫x`, `∞` listed as `1 °C 2 °F 3 °R` (3 questions) publish (`kl6.mjs`); `±2 ±3 ±5` numbered publish; `Cu Zn Fe`, `∠ABC`, `±12`, `−12` (U+2212) refuse. This is the documented single-letter-key guard (alphanumeric length 1), not a symbol-start skip.
* **A rights list repeated 3 times** with five or more questions sharing the column: CLEAN (documented). Un-bulleted first item glued to a lead (`Terms: Granite\n• Quartz\n• Roof`, 3 rights) is CLEAN: the first item reads as "terms: granite".
* **Honest false refusals measured**: 25 honest stems with a lead word and a number ("Answer 1. Which gas ...", "Solution 2 contains 10 g ...", "Key: 1 cm = 10 mm", "Answer: 2 3 4 ...", "Kunci 1. ...") all CLEAN except "Key 1: Forest, Key 2: Lake. ..." which is `key_heading_row` (low, ackable) (`hon5.mjs`). Honest corpora of tests/data (18a 551, 20a 595, 23a 614, 24a 727 questions): refusals only in the pre-existing categories (edge, outlier, choices_pasted, names_left, word_bank_pair, pairing_token); no `answer_line`, `key_heading_row` or `pairs_listed` among them (`corp.mjs`). A glossary stem with 400 "Key N:" mentions in one 8 KB stem is `scan_budget` (not an honest text); 100-200 mentions are CLEAN.
* **Random partial rights lists** are refused about 42% of the time whether the items are plain or swapped (design: a 3-run of 60% of the found items).
* GIFT True/False feedback scanner reads English values only (documented).

### Attacks that found nothing (hard hunt)

* **The swap of non-representable rights is lossless** (`sw2.mjs`, `sw5.mjs`): 900 in-order lists (n = 4-6, 18 weird items: commas, colons, hyphens, "Mr.", brackets, 8-word, "e.g.", "Ph.D.", "U.S.A.", years; 6 separators) were refused 900 of 900 at all three weird fractions; paired plain-vs-weird columns under 2,400 random partial and permuted lists (`sw5.mjs 11`, `12`) gave the **same verdict 2,400 of 2,400** (503/503 and 497/497 refused). A weird right that contains a plain right of another question ("Soft, White" vs rights Soft / White) still refuses the list (`sw3.mjs`, `sw4.mjs`). No hidden order, no false refusal found.
* **Pair-walk 50,000-step limit** (`lim.mjs`): a pair at the END of a text that needs growing walks is `pairs_listed` up to 400 repeats and `scan_budget` from 500 (never CLEAN); at the start it is found at every size. Honest bank of 900 Matching x 50 dictionary pairs (45,000 pairs) with 100 8 KB passages: no `scan_budget` (`walk2.mjs`, 1.7 s).
* **Label caches** (`cache.mjs`): 1,500 random banks (labels, references, locks, long "Both xxxx and B") run in a long-lived instance against a fresh instance per publish: `publish_prepare`, `label_findings`, `formatting_findings` identical, **0 diffs**. The 200-byte rule: "Both A and B"/"All of the above" + 0..1000 bytes of filler give a verdict that does not change across 190-205 bytes (`ref.mjs`). Unique-text shape memory (`mem.mjs`): +17.9 MB for the 3.8 MB unique-"X - " bank, under the 40 MB ceiling.
* **Wasm traps** (`fz.mjs`): 75,000 calls over `publish_prepare` (two option sets), `label_findings`, `formatting_findings`, `strip` on random banks mixing every kind with marks, symbols, scripts, `{TRUE#`, leads, bullets, NBSP and ZWJ: 0 traps, 0 internal errors.
* **CPU shapes of the new code** (`cpu4.mjs`, min of 3): same-line rows on 440 stems 0.30 s; 26-choice Mc 1.29 s; TF 1.42 s; fixed letters 1.19 s; weird pairs 0.20 s; swap shapes 0.46 / 0.25 s. Hostile repeated-lead shapes end `scan_budget` (fail closed).
* **Pair/GIFT/lead matrices**: Moodle GIFT, Cloze, XML, Aiken and weights syntaxes (50 forms, `moo.mjs`) behave as in rounds 18-19 (the unrefused ones are bare weights, `<correctfeedback>`, `[html]`, `//comment`, `Paris ✓` in a stem: documented lead/marker vocabulary).

## Timings of the known 4 MB shapes (node on pkg/, min of 3, CPU seconds)

| shape (script) | bytes | min |
|---|---|---|
| 35a: 480 keep_order Matching + 480 "Yes, No, Maybe, " stems (`cpu1.mjs`) | 4,041,369 | 1.28 (control 1.22) |
| 35b: "x - " 1000 x 26, no decisions / 950 x 26 / 600 / 300 / 150 q (`lab3.mjs`) | 3.66-4.08 M | 1.46 / 1.04 / 0.92 / 0.80 / 0.76 |
| "x - " 950 x 26 with decisions keep / strip (`lab2.mjs`) | 3,970,259 | 1.34 / 1.32 (none 1.00) |
| 36a: "A. X - " 1000 x 26 (`up.mjs`): upper / upper_paren / upper_lower / lowerlab_upperx / noLabel_upperX / upper_Y | 3.76-3.82 M | 1.41 / 1.74 / 1.66 / 1.24 / 0.99 / 1.29 |
| same, per export (`up2.mjs`): validate / validate_for_publish / strip / formatting_findings / label_findings / match_hash / publish_prepare none, keep, strip, auto | 3,815,209 | 0.08 / 1.02 / 1.06 / 1.01 / 0.31 / 0.13 / 1.20, 1.41, 1.10, 1.39 |
| 26 label/ref unit shapes incl. Greek, Cyrillic, kana, Arabic, Hebrew, Indic, CJK, fullwidth, circled, combining marks (`lab6.mjs`, `lab7.mjs`) | 2.0-4.1 M | all under 1.9 (max 1.70 s; `x́ - ` 2.77 s only in the duplicate-choice refusal at 3.7 MB, `lab6`) |
| key rows / answer lines / key bank cross / TF cross / letters (`cpu2.mjs`) | 3.1-4.2 M | 1.37 / 1.54 / 0.40 / 1.24 / 0.41 / 2.29 / 0.16 / 1.61 |
| pair scan hostile ("a " units, 1 / 20 Matching; control) (`cpu3.mjs`) | 3.4-3.5 M | 0.95 / 1.42 (0.53), `scan_budget` |
| bank refusal shapes `lab5.mjs` (9 shapes) | 2.7-4.1 M | 0.75-1.58 |
| `blank170` (`check.mjs`) | 3,690,848 | +29.4 MB, ok |

Worst case I found: 2.77 s (a duplicate-choice refusal path), 2.29 s for the Key cross-bank shape. Nothing near 5 s.

## (d) Not run

* The 627-item and 158-marker corpora of critics 31a/34a (scripts gone); `--ignored` amplification sweep and mutation runner; `cargo test` once, not 3 times.
* An old-binary differential (the round-19 pkg is gone): the claim "FIXED" rests on the same inputs as my round-19 report, which reproduced there.
* Worker end to end, shuffle fairness statistics (round 20 did not touch the shuffle), Argon2id, the app's importers.
* The Rust source of `Neutral`, `pair_entry` and the lead table was read only through behaviour (differential tests); no source review of the new 50,000-step counter beyond the boundary probe in `lim.mjs` (a text just under the limit is read fully at the end; the step threshold itself was not located exactly).
* A fix-direction for B37a-1 was not tested.

BLOCKERS: 1
