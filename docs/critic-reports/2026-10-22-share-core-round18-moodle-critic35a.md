# share-core review, round 18 (critic35a): Moodle lens

**Verdict: the round-17 blockers (34a B1 Multipart part answers, 34a B2 Matching pairings and rights) and the
critic31a ones all stay fixed. I found two blockers: one is CPU (a quadratic loop in the new Matching
rights-order scan, 25-29 s on 4 MB), one is a GIFT True/False production that leaks (borderline family, see
the call).**

## Build, environment and baseline (read first)

All wasm repros use a copy of `server/share-core/pkg` taken at 19:13, never rebuilt:
`sha256 4ab34bf1fbc4cbb5a50a26ef8200fa07a6ab40d7823107ca60d8223173b94da7`, 1,248,397 bytes, mtime 19:04
(identical to the live `pkg/share_core_bg.wasm`). Scripts live in
`/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/critic35a.Xdhdvl/`
(`$D` below); run each as `cd $D && node <script> [args]`. `h.mjs` is the shared loader and payload builders.
CPU is `process.cpuUsage()` around `publish_prepare(bytes, '{"hide_key":true}')`, min of 3 (the first of three
runs is regularly 2-3x slower; the min is quoted). Load average was 3.7-6 when timing (other sessions on the
box); the baseline ran first, alone as far as this session goes, with load from other users at 5-19.

| baseline command | result |
|---|---|
| `cd server/share-core && cargo test --release` | **PASS, exit 0**: 285 passed, 0 failed, 1 ignored (sum of the per-binary `test result` lines). No load flake. |
| `cd server/share-core && cargo clippy --release --all-targets` | exit 0, 0 warnings |
| `cd server/share-core && node tests/node/check.mjs` | **506/506**, exit 0 (`blank170 validate_for_publish 3,690,848 B +29.4 MB 1026 ms ok`) |
| `cargo test --release --test share_core_fixtures` (repo root) | 8/8, exit 0 |

## (a) Previous findings

`node prev34.mjs` (all rows below are its lines; verdict = `publish_prepare` with `hide_key`).

| finding | repro | result |
|---|---|---|
| 34a B1 Multipart part answers in bank texts | three Multipart questions, part (a) Mc keyed Quartz/Basalt/Mohs, description `1 Quartz 2 Basalt 3 Mohs` | **FIXED**: `HARD bank_key_list@bank.details.description` |
| same, the list in a 4th question's stem (`Check your work. 1 Quartz 2 Basalt 3 Mohs`) | | **FIXED**: `bank_key_list@bank.questions[3].stem` |
| same with part Short samples; with Tf parts `1 T 2 F 3 T` | | **FIXED**: both `bank_key_list` |
| same with fixed-order Mc parts, `1 C 2 B 3 D`, before and after the order ack | | **FIXED**: both `bank_key_list` |
| 34a B2 Matching pairs in bank texts | `Bauxite - Aluminium, Chalk - Soft, Slate - Roof`; the GIFT block `=Bauxite -> Aluminium\n=Chalk -> Soft\n=Slate -> Roof`; `Bauxite: Aluminium; ...`; `Bauxite = Aluminium, ...` | **FIXED**: all four `pairs_listed@bank.details.description` |
| 34a B2 rights in pair order, `keep_order` | `Aluminium, Soft, Roof` | **FIXED**: `word_bank_order`. Control (shuffled question): `CLEAN`, correct. |
| 31a B1 64-entry cap | 70/120/140/200 True/False questions, description `Answers: <last five keys>` and `<first 20 keys>` | **FIXED**: all 8 `bank_key_list` |
| 31a B2 4,096-event cap | `Readings: 7 7 ... ` x 0/500 then `1 Quartz 2 Basalt 3 Mohs` | **FIXED**: `bank_key_list`; at 3000 repeats the description is over 8 KB: `too_long` (ordinary limit) |
| 31a B3 fields not read | `1 Quartz 2 Basalt 3 Mohs` in part stem, left, topic, right, distractor, choice, name, author, licence name | **FIXED**: all 9 refused (`bank_key_list`, plus `topic_is_key`/`outlier` where those also apply) |
| 31a B4 frozen order | three `keep_order` Mc, description `Key: C B D`, before and after ack | **FIXED**: `bank_key_list` + `answer_line`, both |
| 31a B5 false `scan_budget` | 63-row `Pupil,Key Stage,Hours` CSV in the description | **FIXED**: `CLEAN` |
| 31a B6 memory | `check.mjs` `blank170` (3,690,848 B) | **FIXED**: +29.4 MB, under the 40 MB ceiling (the critic's exact 4,172,388-byte `mem3.mjs` script no longer exists; not re-run) |
| 34b B1 prefilter | `node prefilter1.mjs`: `mark  scheme:`, `mark\tscheme:`, ` \t`, 3 spaces in stem and choice | **FIXED**: `answer_line`. NBSP, U+2009, U+3000, VT between the words are also refused (the text is non-ASCII or VT is a line break, so it takes the full scan). |
| 34b B2 dichotomous key | `node dk1.mjs`, `1a  ` couplets, every correct position | **FIXED** for the layouts it names (see (c) for a separator it does not) |

Exports: `validate_for_publish`, `strip` and `publish_prepare` give the same refusal for both new bank repros
(`node exports1.mjs`).

## (b) New-class blockers

### B35a-1. CPU 25-29 s (min of 3) on 4 MB: the Matching rights-order scan is quadratic and unmetered

**Class:** CPU over 5 s on a 4 MB payload.

**Mechanism.** The round-18 block in `strip::bank_key_list_errors` (`src/strip.rs`, "if !matching_orders.is_empty()")
reads every list of every bank text (`format::word_bank_lists`). For each list item it looks up
`matching_order_of[item]`, which holds one `(question, position)` per `keep_order` Matching question whose
right column holds that text, and for each of those it does `seqs.iter_mut().find(|x| x.0 == mi)`, a linear
search of the per-question slots. The cost per list item is (questions sharing the right) x (distinct questions
seen so far), and it is not charged to `format::spend_work`, so neither the 100,000-step text budget nor the
20 M phase budget sees it. The result is multiplicative in matching questions x carrier texts x list length.
A right text shared by many questions is ordinary for Matching ("True / False / Not Given", "Yes / No").

**Exact input** (`cpu.mjs`, `mo_param`): a bank of M Matching questions, each `keep_order: true`, three pairs
`Statement i a/b/c` -> rights `Yes`, `No`, `Maybe`, no distractors; plus C Short questions whose stem is
`"Yes, No, Maybe, "` repeated to T-5 bytes (T = 8000 gives 7,995 bytes). `publish_prepare(payload, '{"hide_key":true}')`.

**Exact outputs** (min of 3, process CPU, seconds; every one of these returns `ok: true`, no error, no
`scan_budget`):

| command (`cd $D`) | payload | min CPU (3 runs) |
|---|---|---|
| `node cpu.mjs mo_param 480 480 8000 keep` | 4,041,369 B | **24.80 s** (67.35, 24.80, 25.40) |
| `node cpu.mjs mo_shared` (same shape) | 4,038,969 B | **28.62 s** (72.63, 29.56, 28.62) |
| `node cpu.mjs mo_param 480 480 8000 shuffled` (control: same bytes, `keep_order` false) | 4,041,849 B | **1.23 s** |
| `node cpu.mjs mo_param 20 480 8000 keep` (few matching questions) | 3,904,609 B | 1.07 s |
| `node cpu.mjs mo_param 480 1 8000 keep` (one carrier) | 150,931 B | 0.15 s |

Scaling (all `keep`, T = 8000 unless stated): M=240, C=480: 12.74 s; M=480, C=240: 15.68 s; T=4000 (2.1 MB):
12.90 s; T=2000 (1.2 MB): 7.61 s; T=1000 (0.68 MB): 4.16 s. It is the product M x C x T, so a 1.2 MB payload
already costs 7.6 s. The control isolates the new block: the only difference between 1.23 s and 24.8 s is
`keep_order: true` on the Matching questions, which is the condition that fills `matching_orders`.
(Why the matching questions may share rights: the bank scans do not need any list to be a leak; the scan
reads the list first and decides afterwards.)

Honest shape, for scale: 1000 `keep_order` True/False/Not Given Matching questions with a normal stem
(`node cpu.mjs tfng 1000 keep`): 0.38 s, so the honest case is not near the line; the cost needs long lists
in many texts, which is hostile but within the limits, and the rule counts it.

**Fix direction.** Index the slots by question (a `Vec` indexed by `mi`, or sort the candidate list once),
and charge `spend_work` for `cands.len()` per item.

### B35a-2. GIFT True/False with feedback (`{TRUE#...}`, `{FALSE#...}`, `{T#...}`) is not read (borderline family: counted)

**Class:** a hint syntax the gate promises to read. README "Documented limits (round 9)": "the formatting checks
protect against ... Moodle/GIFT/Aiken/Cloze/XML syntax". `{TRUE}`, `{T}`, `{F}`, `{FALSE}` are refused (`gift`).

**Mechanism.** `format::stem_key` brace loop (`src/format.rs`, near "if let Some(e) = head.find('}')"):
the True/False brace is recognised only when the whole text up to `}` is exactly `true`, `false`, `t` or `f`.
GIFT's True/False production with feedback (`{TRUE#feedback}`, `{TRUE#feedback if wrong#feedback if right}`) has
`#...` before the `}`, so it falls through. The `#` branch below it accepts only a numeric head (`{#4}`), so it
does not catch it either. The Moodle GIFT forms with `=`/`~`/`#=` feedback are all read; only this production
is missed.

**Exact inputs and outputs** (`node giftb.mjs`, `node gift2.mjs`; `publish_prepare` with `hide_key`, question
`Mars is a planet. <form>` with `Tf.correct` equal to the value in the braces):

```
CLEAN   {TRUE#well done}                    (key true)
CLEAN   {FALSE#Not quite}                   (key false)
CLEAN   {T#good}
CLEAN   {TRUE#Great job!#Check your notes}
CLEAN   {TRUE#}   {true#Yes}   {F#no}   { T # fb }   {TRUE #well done}   {TRUE\n#well done}
HARD    {TRUE}                              gift@question.stem        <- control
```

The served payload keeps the text: `stripped.payload.question.stem = "Mars is a planet. {TRUE#well done}"`,
`kind.Tf.correct = false` (placeholder), so students read the answer. Same CLEAN result in a Tf **part** stem
(`{TRUE#well done}`, `{FALSE#Not quite}`, `{T#good}`: all CLEAN). The full GIFT matrix of `gift2.mjs` (38
productions on an Mc stem: `{=x}`, `{~a =b}`, `%` weights, `#` feedback on choices, `{#4}`, `{#4:0.1}`,
`{#3..5}`, escapes, `[html]`, `$CATEGORY`, Cloze types, matching `{=a -> b}`) misses only: these six True/False
forms, and `{}` / `{=}` / `{#}` (empty braces, no key) and `{TRUE:}` (not GIFT).

**Differential.** 6 of 6 `#`-feedback True/False forms in a Tf stem and 3 of 3 in a Tf part stem are published;
the bare forms are refused in both places.

**Why counted, honestly.** It is the same brace family as `{TRUE}`, so by the "same-class variant" rule it can
be argued either way. I count it because it is a separate production of the GIFT grammar (Moodle's own export
of a True/False question with general feedback writes `{TRUE#...}`), the matcher is a whole-string equality
that cannot match it, the leak is complete (key and no hint it is there), and the README names GIFT as
in scope. If you judge it the same channel, the count below is 1, not 2. The fix is one condition in the same
line (a head starting `true`/`false`/`t`/`f` followed by optional spaces and `#`).

## (c) Non-blockers (terse)

**Letter followed by prose is not an answer line (largest remaining leak, same channel as the fixed
`Answer: B`).** With a fixed order (`keep_order` + ack), `node letter1.mjs`, `node letter3.mjs`:
`Answer: B because it interlocks`, `The answer is B because it interlocks`, `Answer: B is right`, `Answer: B
and nothing else`, and for a two-answer question (correct B, D) `Answer: B and D`, `Answer: B & D`, `Answer:
B or D`, `Answer: BD`, `The correct answers are B and D`, `Answers: B and D because they interlock` are all
`CLEAN`; `Answer: B`, `Answer: B.`, `Answer: B, D`, `Answers: B, D`, `Answer: B (second choice)`, `Answer: B; ...`
are `answer_line`. A numbered key with a rationale in a lone question
(`Answers: 1B because it interlocks. 2C because ...`, `1 B because ...`) is `CLEAN` too (a bank catches the same text
through `bank_key_list`, `couplet1.mjs`). Text-based forms are refused (`The correct answers are: Dovetail
joint and Mitre joint`, 13 of 14 forms). By design a letter must "stand alone" (round 9, to keep "Answer: A tree
..." honest), so I treat it as the documented letter rule, not a new channel. Without a fixed order a letter
names nothing. Worth a decision because multi-answer Moodle questions are the natural place for "B and D".

**Matching pair separators that the pair-list rule does not read** (same list, other separators; the Matching
stem rule has the same holes, `match1.mjs`): pipe, tab-separated or dotted-leader lines (`Bauxite ........
Aluminium`), `matches`, reversed order `Aluminium - Bauxite`, `=Aluminium <- Bauxite`, two parallel lists
(lefts line, rights line), grid, HTML/Markdown tables, `;`-joined, `=` without spaces. Rights in pair order on
separate lines, numbered (`1. Aluminium`) or lettered (`a) Aluminium`), `and`-joined or in reverse order are
not `word_bank_order` (comma, semicolon, slash, pipe, bullets, colon, bracket table and a rotated list are).
All are separator variants of a refused list or documented (tables).

**Moodle essay "Information for graders" and friends are not leads** (`moodle1.mjs`, key `Mars` in an Mc stem):
`Grader info:`, `Information for graders:`, `Grading notes:`, `Marker's note:`, `Note to grader:`, `Teacher
note:`, `Teacher's notes:`, `Instructor note:`, `Tutor note:`, `Rubric:`, `Scoring guide:`, `Scoring:`,
`Grading:`, `Memo:`, `Sample response:`, `Expected response:`, `<correctfeedback>Mars</correctfeedback>`,
`<generalfeedback><text>Mars</text></generalfeedback>` are CLEAN (with `{TRUE#well done}` and "The answer is the second one", 20 of 85); 65 are refused (all Cloze types and
abbreviations, GIFT, `<answer fraction>`, Aiken, `The right answer is:`, `Model solution:`, `Expected answer:`,
`General feedback:` ...) are refused. Lead-word variants.

**Honest numeric table refused (workaround exists).** A 40-row `id  x  y` table with plain numeric ids in the
description of a 200-question numeric bank is `bank_key_list` in 12 of 40 random trials with Short answers and
20 of 40 with two-part Multipart answers (`honest2.mjs`, `honest4.mjs`): "a value right after its own question's
number" fires on two rows by chance. `S1`-style ids: 0 of 40. `Trial 1` and `1,x,y` rows: 20 of 40. The hint
says "remove the answer", which is not what to do. Same family as the round-17 residual false positives.

**Honest dichotomous key with `1a - ` labels** is refused when the correct choice is the first leaf: 12 of 96
layouts (`dk1.mjs`; only the ` - ` separator, with every header). `1a  `, `1a. `, `1a) `, `1a: ` are fine.

**Honest Matching option list in a `keep_order` stem** (`True, False, Not Given`) is `word_bank_pair` (low,
ackable) for one of the six orders (`tfng2.mjs`); in a 12-question bank 3 of 6 orders; fine with `ack_formatting`.

**Not read:** `details.license_url` is served and unscanned (`url1.mjs`: `https://example.com/answers/1-Quartz-2-Basalt-3-Mohs`
is published and served). A teacher does not type a key into a URL field; I did not count it, but the round-17
sentence "every served field of a bank is read" omits it.

**Digit-glued lead** (`7answer: Zebra`, `1key: Zebra`, `q1answer:`) is not a lead, also in a lone question
(consistent with the prefilter, not a skip).

**Checked and sound:** shuffle fairness over 6,000 runs (`stats1.mjs`): Matching 4 pairs + 2 distractors,
distractor bank positions chi2 9.1 (5 df), left-order chi2 18.4 (15 df), pair left-slot x bank position chi2
32.5 (23 df, p about 0.09); Mc with a locked last choice, correct position chi2 4.6 (2 df); Multipart two-correct
part positions chi2 2.0 (4 df). Matrices with no miss: 5 syntaxes (Cloze, GIFT, Aiken, `Answer:`, Anki cloze,
`The correct answer is:`, `{TRUE}`, `<answer fraction>`) x bank name, description, author, licence name, and x
stem, part stem, main stem, Matching left, blank stem (`matrix1.mjs`; the single-question "misses" there are
a wrong choice containing answer syntax naming the key, which I did not count). CPU shapes, min of 3, all under
5 s: 480 shared-pair Matching questions + 480 stems of `Yes - Y;` 0.55 s; distinct pairs listed once 0.81 s;
rights shared by 480 non-`keep_order` Matching, Mc, Multipart + 480 carrier stems 1.73 / 1.19 / 1.18 s; 300
Multipart with keep_order letters + 300 carriers of `a b ` 0.70 s (`scan_budget`, fails closed); 1000 questions
x 26 choices with double spaces 1.01 s; 4 MB of double-spaced stems 1.43 s. Honest banks: 150 Matching
questions x 30 pairs 0.19 s CLEAN; 200 four-part numeric Multipart questions CLEAN (plus a small data table
stem, CLEAN). Fuzz: `fuzz1.mjs` over 5 exports, random banks mixing every kind, pair separators, İ ẞ ß ſ,
combining marks, ZWJ/ZWSP, NBSP, ideographic space, CJK/Arabic/Devanagari/Thai/Georgian/Hangul: 246,000
banks, 1.23 M calls, 0 traps.

## (d) Not run

* The 627-item honest corpus and the 158-marker corpus of critics 31a/34a (their scripts are gone from `/tmp`).
* `mem3.mjs` (the critic's exact 4,172,388-byte blank170); `check.mjs`'s `blank170` stands in.
* The `--ignored` amplification sweep and the mutation runner; `cargo test` ran once, not 3 times.
* An old-binary differential (I did not rebuild); whether the glued-letter couplet rule changed the
  `1B because ...` result in a lone question is therefore not measured.
* The Worker end to end, Argon2id, the app's Anki/GIFT import code, and the Moodle sources.

BLOCKERS: 2
