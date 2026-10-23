# share-core round 21: robustness critic (critic38b)

Tree `shared-pool-phase1` working tree, `server/share-core`. wasm under test: `pkg/share_core_bg.wasm`, 1,289,686 B, mtime Oct 23 05:11, not rebuilt. Scripts: `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/tmp.NAlW9xaN4B` (`$S`, `node $S/<name>.mjs`, loader `$S/h.mjs`). No repo file edited except this report; nothing committed, pushed or deployed; port 8799 unused; owner client files untouched. CPU = `process.cpuUsage`, min of 3; load average 2-6 on 24 cores.

## Baseline (run alone)

| check | result |
|---|---|
| `cargo test --release` | 309 passed, 0 failed, 1 ignored |
| `cargo clippy --release --all-targets` / `--target wasm32-unknown-unknown` | clean / clean |
| `bash scripts/check-wasm-exports.sh` | `ok: pkg has no test-only exports`; pkg exports are exactly `memory argon2id formatting_findings full_hash health label_findings match_hash memory_bytes publish_prepare schema_versions strip strip_author_data validate validate_for_publish validate_meta` |
| `node tests/node/check.mjs` | 506/506 |
| root `cargo test --release --test share_core_fixtures` | 8/8 |

## (a) Previous findings re-run

| finding | now | evidence |
|---|---|---|
| 37b B1 invisible direction mark inside a key word | **FIXED** | `y1.mjs`, `q5.mjs`: "Answer: Pa<LRM>ris", "Key: 1. Pa<LRM>ris", Mc fixed "Qu<LRM>artz", TF "Tr<LRM>ue", Blank "osmo<LRM>sis", bank key list with a mark in every word (with and without a lead), pair listing with a mark in the right: all refused (`answer_line`, `tf_value`, `bank_key_list`, `pairs_listed`) |
| 37b B2 `Ans.` + number row; lead ending a line after prose + row on the next line | **FIXED** | `r2.mjs` 15/15 refused (`Ans. 1 B`, `Ans. (1) B`, `ans. 1 B`, `Ans.\n1 B`, TF forms); `r3.mjs` 12/12 refused (`Capital of France? Key:\n1. Paris`, `Correct:\n1) Paris`, `Kunci:\n1 Paris`, `(Key:\n1. Paris)`, `** Key:\n1. Paris`); `m2.mjs` 200 forms: 0 published (the 60 previously unread weak leads now read) |
| 37b weaker leads | read for a numbered row, low risk, ackable (see (c)) | `o1.mjs`: bare "Keys: Paris" etc. still publish (documented) |

**Independent Default_Ignorable sweep** (different inputs from the builder's: `z1.mjs`, `z2.mjs`). Candidates: every code point 0x80-0x10FFFF that is Cf, Default_Ignorable, Mn, Me or Mc (6,300+); 2,285 pass character validation (the rest are `forbidden_char`), of which 21 are Default_Ignorable (U+00AD 034F 061C 17B4 17B5 180B-180F 200B-200F 2060-2064 FEFF), 25 Cf, 2,260 Mn/Me/Mc. Each inserted at four positions (start, middle, end, between every character) of the key word in 11 scans, 9,140 calls per scan, 100,540 calls total, 0 exceptions:

| scan | control | Default_Ignorable leaks | other-category leaks (distinct code points) |
|---|---|---|---|
| Short "Answer: Paris" | refused | **0** | 497 |
| Mc fixed order "Answer: Quartz" | refused | **0** | 497 |
| Mc fixed order letter "Answer: B" (before/after) | refused | **0** | 453 |
| True/False "Answer: True" | refused | **0** | 497 |
| Blank "Answer: osmosis" | refused | **0** | 497 |
| bank key list, no lead / with "Answers:" lead | refused | **0 / 0** | 497 / 497 |
| pair listing in bank description / in the question's own stem | refused | **0 / 0** | 497 / 497 |
| rights list (keep_order) in description / in another stem | refused | **0 / 0** | 497 / 497 |

The "other-category" leaks are visible marks, not invisible ones: Arabic and Kaithi number signs (U+0600-0605, 06DD, 070F, 08E2, 110BD, 110CD, Cf: they draw a sign), and spacing combining vowel signs (Mc: U+0903, 093E-0940, 0982, 09BE ...) and a few new-script Mn that draw glyphs next to the letter. The common combining diacritics U+0300-036F are folded and refused. They change what students see ("Pa" + visible vowel sign + "ris"), so they are spelling variants of the key (family A below), not an invisible bypass.

Bidi, tag and selector characters: LRE/RLE/PDF/LRI/PDI/LRO/RLO, tags U+E0000-E007F, variation selectors U+FE00-FE0F and U+E0100-E01EF, Hangul fillers are `forbidden_char` at validation; the allowed ones (LRM, RLM, ALM, invisible operators, MVS, Mongolian FVS, ZWJ, ZWNJ, ZWSP, WJ, BOM, soft hyphen, CGJ, Khmer inherent vowels) are all folded.

## (b) New-class blockers (B)

**None found.** How hard I hunted, with the classification of what I did find below (all (A)):

* Fold index/offset mismatch: `z4.mjs`, `z7.mjs`, fuzz. The fold only feeds the key, hint, pair and list scans; the served text is untouched (an honest stem with LRM, ZWJ and a soft hyphen is served byte for byte). Texts entirely ignorable: stem of 50 LRM -> `blank_text`; Mc choices, Blank accept, Matching left or right that are only LRM -> `blank_text`; Short sample of only LRM/ZWJ/SHY publishes (an empty key; nothing to leak); bank name only LRM publishes; bank description of 1,500 LRM -> `too_long` (3 bytes each); "Answers: 1 2 3" with Short samples that are all LRM publishes. No panic, no wrong refusal. `label_findings` `order_preview` (movable / frozen reason) with a Default_Ignorable character inserted at six positions of "Both A and B", "All of the above", "Talc": identical to the control in 126 of 126 cases (no wrong freeze).
* 4 MB of ignorable characters (`z8.mjs`, fresh instance, min of 3, memory = wasm growth): stems of "a"+LRM (2.6 MB) 0.47 s +20.5 MB; "Answer: "+LRM runs 0.54 s +17.5 MB; "Pa"+LRM+"ris" repeated 0.44 s +13.3 MB; "Key"+LRM+": "+LRM+"1. " rows 0.77 s +14.4 MB; one letter + 8,000 LRM per stem x 480 (3.8 MB) 0.54 s +27.1 MB; 26,000 choices of "x"+LRM x 60 (4.08 MB) 1.22 s +32.1 MB; 20k Matching pairs with LRM + stems with LRM 0.57 s +15.2 MB; Hindi "क"+virama+ZWJ+"ष" repeated 0.78 s +28.2 MB. Nothing above 1.3 s or 33 MB.
* Honest scripts that depend on ZWJ/ZWNJ (`z5.mjs`, `z6.mjs`): Persian (ZWNJ in "می‌خواهم", "کتاب‌ها"), Hindi (conjunct with ZWJ and ZWNJ), Tamil, Malayalam, Sinhala (yansaya), Khmer (U+17B4/17B5), Myanmar, Arabic (lam-alef with ZWJ), each as Mc shuffled and fixed, Short, TF, Matching, Blank, and a bank description: 56 items, **0 refusals**. Served text unchanged except two things that predate round 21: a ZWSP inside Myanmar text and a word-final ZWJ before a space in Malayalam (chillu) are removed by the served-text rule (they are not the fold). No false match found: the fold is applied to both the key and the text, so "کتاب‌ها" matches "کتابها" as the same word (a leak either way).
* Weaker leads and the mid-line rule, honest corpus: see (c).
* Argon2id and the other exports under bad parameters: `aa.mjs`: `m=0`, `t=0`, `p=0`, `p=1e9`, salt of 0 or 1 byte, `m=4G` all return a JS error, no trap. Not capped inside the wasm: `argon2id(pw, salt, 1<<20, 1, 1)` (1 GiB) succeeded in 0.93 s and keeps wasm memory at that size; the parameters are the Worker's constants, not payload data, so this is a note for the Worker, not a gate blocker.
* Hash flooding of the `SIB_REF_MEMO` std `HashMap` (round 20 hunt) not repeated; unchanged code.

**Differential and cross-call checks** (round-20 harnesses on this pkg): `g2.mjs` (400 random banks x 4 exports, fresh instance vs one long-lived instance with noise between) 0 differences; memory over 30 mixed calls (`v1.mjs`) flat at +19.3 MB from the first call, `l1.mjs` shapes +31.8 / +28.8 / +16.8 MB flat across repeated calls.

BLOCKERS(B): 0

## (c) Non-blockers, measured

**Fuzz (0 traps required): 498,000+ mutated calls, 0 exceptions, instance alive and `validate` still `ok` after each run.**
* `g3.mjs` (mutated bank of Matching, Mc fixed and shuffled with "Both A and B", Short, TF, Blank, Multipart, with Default_Ignorable, combining, lead, row, arrow, GIFT, full-width and pair-mark fragments, five options sets incl. `ack_formatting`, `order_acks`, `labels`, plus single-question calls): seeds 111, 222, 333 x 14,000 iterations = 378,000 calls, 0 exceptions, memory 1.6 MB.
* `g4.mjs` (all five kinds, random words and Default_Ignorable characters in stems, samples, accepts, lefts, rights, bank name and description; 4 exports): 120,000 calls, 0 exceptions, memory 1.6 MB; 63,128 `forbidden_char` and the rest spread over `invisible`, `outlier`, `variation`, `answer_line`, `emoji`, `order_ack_required`, `mixed_script`, `key_word`, `tf_value`.

**CPU, all known 4 MB shapes re-timed (min of 3, node on pkg/, publish_prepare unless noted):**

| shape | bytes | min |
|---|---|---|
| 950 x 26 "x - " choices with 950 keep decisions (35b B1) | 3,923,599 | 1.66 s (`label_findings` 0.19 s) |
| Matching rights order, 480 shared columns + 480 carrier stems (35a B1) | 4,041,369 | 1.29 s (control, keep_order off, 1.22 s); 480 distinct columns holding "Yes": `scan_budget` 0.62 s |
| heading rows `Key\n1. a\n` x 480 stems | 4,121,969 | **3.24 s** (control `Kex\n1. a\n` 1.90 s), `Answer\n1. a\n` 2.55 s, `Key a\n1. a\n` 2.19 s; first-run 3.6-4.5 s; the widest margin on the list |
| `Ans. 1 a\n`, `Memo\n1. a\n`, `Marks: 1. a\n`, `Rubric: 1 a\n` stems | 3.9-4.0 MB | 2.25 s, 2.11 s, 1.06 s, 1.07 s |
| mid-line lead `Answer: x. Key:\n1. Paris` / LRM key rows / `Ans.\n1 B` | 3.9-4.1 MB | 0.85 s / 1.39 s / 0.37 s (refused) |
| label styles: double-space, `x:  y:  `, `Key  `, `x - `, `a.  b.  `, `A) B) C) `, `a, b, c, `, roman | 3.95 MB | 0.88-1.45 s (`Key  ` and `"1 and 2 and 3 "` refuse `scan_budget`, 1.45 s and 1.65 s, fail closed) |
| label styles in other scripts (round-20 sweep) | 3.7-4.1 MB | worst 3.14 s, mostly 1.1-2.2 s (previous round) |
| pair walk, random-word and random-char lefts, 15-30k pairs, 200-400 stems (`d1.mjs`, `d2.mjs`) | 3.0-4.0 MB | 0.85-2.11 s (refuse `scan_budget` or publish) |
| honest 3.8 MB prose bank with 200 keep_order Matching | 3,805,180 | 0.86 s, publishes |
| shared 8 KB passage + quoted phrases (`t1.mjs`) | 1.1-3.2 MB | 0.34-1.12 s; the documented contrived shape refuses `scan_budget` |
| `j1.mjs` heading/row family | 3.9-4.1 MB | 0.79-3.06 s |

No shape above 3.3 s. Memory over 30 calls flat; the worst single call grows wasm memory by 32.1 MB (26,000 ignorable-laced choices), under the 64 MB bound.

**Honest false-refusal rate, round-21 rules (1,715 honest items, deliberately full of lead words):**
* 54 scaffolding stems x 6 kinds (324): 3 refused, hard, two of them the documented first-choice numbering rule (`Key learning: 1. ...`, `Key\n1. Wall` with the first choice correct), one is my fixture (a stray blank).
* 11 headings ("Key terms", "Key points", "Key stage 3", "Key vocabulary", "Key facts", "Use the key", bare "Key"...) x 7 kinds (77): 16 refused, **14 low risk and ackable** (bare `Key` and `Use the key` followed by numbered rows that hold the key text; `Use the key` is new in this round: the mid-line rule), 2 hard (`Key terms:` + `1.` row, first choice correct, documented).
* 36 lead + number lines x 7 kinds x 3 prefixes (756): 8 refused, all hard: `Key: 1. forest 2. desert` and `Answers 1-5 refer to the map.` with a fixed order and the first choice correct (the number reads as choice 1, documented), and `Correct: 3 of 4 marks needed.` on any True/False question (`tf_value`, 6 of the 8).
* Weaker leads and prose-then-colon (`w2.mjs`; 31 lines x 6 kinds x 3 prefixes = 558): "Marks: 3 of 4", "Marks: 4", "Marks 4", "(2 marks)", "Memo: bring a ruler", "Memo 1. bring a ruler", "Memo: 4 pencils", "Rubric: 4", "Rubric 4 points", "Rubric: 1. Clarity 2. Evidence", "Marking guide: 2 marks", "Keys: 1 cm = 10 km", "Keys: A = apple", "Write your answer: 1. Explain why." and the like publish for Short, shuffled Mc and TF (6 refusals only where the row IS the key: `Marking scheme\n1. Paris`, `Tick the correct answer:\n1. Paris is in France` with Short "Paris": `key_heading_row`, ackable, and `ack_formatting` publishes it: tested). Hard refusals: 24 for a fixed order with the first choice correct and a numbered task list under "Write your answer:", "Your answer:", "Answer:", "Tick the correct answer:" (documented numbering rule), and 18 `marked_list` refusals (a pre-existing rule: "Marks / Notes / Rubric / Memo / Steps\n1. Method\n2. Answer", the second row's word "Answer"; "Method / Working" publishes).
* Combined: 55 hard (3.2%) and 20 low-risk ackable (1.2%) of 1,715; excluding the documented first-choice numbering rule and my fixture, 20 hard (1.2%). The corpus is adversarial (every line contains a lead word); a normal teacher bank would be well under 1%. Honest bank-text cases (168: lefts-only lists, reversed rights, one-pair mentions, topic prose over 7 pair sets): 0 refusals except a set whose own right names its left (`names_left`, unrelated).

**Other**
* `output_too_large` refuses a payload that fits the 4 MB input limit but expands past it once defaults are written (inputs of 4.15-4.18 MB); documented, fail closed.
* Short sample of only invisible characters and a bank name of only LRM publish (nothing to leak).

## (d) Variants (A): families already reported in rounds 17-21

Each is a layout or spelling of the key-lead / numbered-row / pair-list / fold families; one repro each (Short key "Paris", stem "Capital of France?\n" + line unless stated; every one publishes). The first four are explicitly in the README's documented limits ("the remaining layouts of the key-lead channel", exotic row labels, decorated leads, leads not in the tables).

1. Parenthetical or bracketed qualifier between the lead and the separator: `Key (hidden): Paris`, `Answer (hidden): Paris`, `Answer Key (Teacher): Paris`, `Key [teacher only]: Paris`, `Answer (teacher): 1. Paris`. (This is the most likely honest-looking paste of the list; the bare `Key: Paris` and `Answer: Paris` are refused.)
2. Decorated leads (documented): `**Key:** 1. Paris`, `<b>Key:</b> 1. Paris`, `` `Key:` 1. Paris ``.
3. Exotic row labels after a lead (documented): `Key: i. Paris`, `Key: A. Paris`, `Key: Question 1: Paris`, `Key: [1] Paris`, `Key: #1 Paris`, `Key: No. 1 Paris`.
4. Lead words not in the table (documented): `Model response: Paris`, `Number 1: Paris`, `Key for Q1: Paris`, `Answer sheet: 1. Paris`, `Teacher key: 1. Paris`; the numbered form `Correct response: 1. Paris` publishes while `Correct response: Paris` is refused.
5. Arrow after the lead with a numbered row: `Key -> 1. Paris` (the bare `Key -> Paris` is refused).
6. Visible spacing marks inside a key word (fold family, visible not invisible): `Answer: Pa` + U+0903 + `ris` (497 code points publish at some position; U+0300-036F and every invisible character are refused).
7. Pair listing separators not read (documented): ` / `, `|`, tab, `~`, `...`, `・`, ` ، `, `∶`, `≡`, `▶` between left and right (`i1.mjs`, 19 of 61 separators); rights as a list wrapped in " and " or one per line (documented); the first words of a multi-word right as a list ("Paris, Rome, Madrid" for rights "Paris, capital city", ...).

VARIANTS(A): 7

BLOCKERS(B): 0
