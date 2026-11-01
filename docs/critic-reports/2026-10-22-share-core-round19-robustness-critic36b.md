# share-core round 19: robustness critic (critic36b)

Tree `shared-pool-phase1` working tree, `server/share-core`. wasm under test: `pkg/share_core_bg.wasm`, 1,264,436 B, mtime 20:44, not rebuilt. Scratch dir (all scripts): `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/tmp.NAlW9xaN4B` (`$S`); `node $S/<script>.mjs` against `pkg/`; `$S/p` is a native probe crate (path dependency, own target dir). No repo file edited except this report; nothing committed or deployed; port 8799 unused; owner client files untouched. Times are process CPU (`process.cpuUsage`), min of 3, load average 3-6 on 24 cores. Payload builders in `$S/h.mjs`.

## Baseline (run alone)

| check | result |
|---|---|
| `cargo test --release` | 294 passed, 0 failed, 1 ignored |
| `cargo clippy --release --all-targets` | clean |
| `cargo clippy --release --target wasm32-unknown-unknown` | clean |
| `bash scripts/check-wasm-exports.sh` | `ok: pkg has no test-only exports` |
| `node tests/node/check.mjs` | 506/506 (`blank170 ... +29.4 MB`) |
| root `cargo test --release --test share_core_fixtures` | 8/8 |

## (a) Previous findings re-run

| finding | now | evidence |
|---|---|---|
| 35b B1 label-path CPU, 950 q x 26 choices "x - x - ...", `publish_prepare` with 950 keep decisions | **FIXED** | `node a1.mjs "x - " 950 110`: 3,923,599 B, cpu [2274,1300,1344] min **1.30 s** (was 10.2 s); `label_findings` min 0.18 s. Variants (min of 3): "x. " 1.67 s, "x) " 1.48 s, "x, " 1.46 s, "i - " 1.40 s, 1000 q x 70 B 1.67 s. (150 B choices exceed 4 MB.) |
| 35a B1 Matching rights-order CPU, 480 keep_order sharing Yes/No/Maybe + 480 carrier stems | **FIXED** | `node e1.mjs`: 4,041,369 B, min **2.01 s** in wasm (was 24.8 s), `ok:true`; control with `keep_order` off 1.92 s, so no scanner overhead left. 480 distinct columns all holding "Yes": `scan_budget` in 0.68 s (fail closed, as documented) |
| 35a B2 GIFT `{TRUE#fb}` | **FIXED** | `node h1.mjs`: refused `gift` for `{TRUE#..}`, 0-200 spaces, 1-20 newlines/tabs/NBSP before or after, ZWSP, fullwidth, `##`, empty feedback, 5 KB feedback. Only an unterminated `{TRUE#fb` publishes (not GIFT). The 6-character letter bound is not reachable by whitespace |
| 35b B2 numbered row under `Key`/`Answer key` heading alone on its line | **FIXED for the heading-alone form; STILL REPRODUCES on one line** (see B1 below) | `node a2.mjs`: all 6 headings x 11 row forms refuse for Short and Mc text keys; `Key`, `Answer key` heading refuse for TF |

## (b) New-class blockers

### B1. Hint channel: a numbered row after a lead on the SAME line (or `Answers`/`Answer` + number with a letter or True/False key) is read by nothing (incomplete fix of 35b B2)

**Mechanism.** `heading_row` (`src/format.rs`, ~4290) requires the lead to stand alone on its line (`fc[e] == '\n'` after spaces and colons). `key`/`correct` leads never compare key text (`names_key(.., texts=false)`), and the numbered-letter rule needs two values. So the same row on one line is not read. Controls: the bare form is refused.

**Inputs and outputs** (`publish_prepare`, `{"hide_key":true}`; Mc rows also `order_acks:[{question:0}]`; stem is `"Capital of France?\n" + row` unless stated). Scripts `m1.mjs`, `m2.mjs`, `b1.mjs`, `a2c.mjs`:

```
Short "Paris":
  "Key: Paris"                   REFUSED answer_line      <- control
  "Answer: 1. Paris"             REFUSED answer_line      <- control
  "Key:\n1. Paris"               REFUSED answer_line      <- round-19 fix
  "Key: 1. Paris"                PUBLISHED
  "Key: 1) Paris" / "Key: (1) Paris" / "Key: Q1: Paris" / "Key 1 Paris"   PUBLISHED
  "Correct: 1. Paris" / "Correct: 1) Paris"      PUBLISHED
  "Mark scheme 1 Paris"          PUBLISHED
  "Answers 1. Paris", "Answer key 1. Paris", "Solution 1) Paris", "Ans 1. Paris", "Model answer 1. Paris"   PUBLISHED
Mc [Talc,Quartz,Diamond,Gypsum], correct Quartz, SHUFFLED: "Key: 1. Quartz"   PUBLISHED (key text served)
Mc same, keep_order (after order ack):  "Answer: B" REFUSED (control); "Answer: 1 B" REFUSED;
  "Answer: 1. B", "Answer:\n1. B", "Answer 1) B", "Solution: 1. B", "Key: 1 B", "Key: 1. B"   PUBLISHED
  shuffled control "Answer: 1. B" PUBLISHED (letters name nothing there, correct)
TF true: "Answer: True" REFUSED (control); "Answer:\n1 True", "Solution:\n1 True", "Key: 1 True"   PUBLISHED
Bank (two Short keyed Paris, Madrid): third question stem "Key: 1. Paris" PUBLISHED; "Correct: 1. Paris" PUBLISHED
   (the same text in the bank description is refused: answer_line)
```
Matrix `m2.mjs`: 20 leads x 10 same-line forms for Short "Paris": **117 of 200 published**. Served stem keeps the text (`stripped.payload...stem` echoes `"Which is hardest?\nAnswer: 1. B"`). For the fixed-order Mc and TF cases the leak is the key by position or value; for Short and shuffled Mc it is the key text.

**Why new-class.** Same family as 35b B2 (single numbered row), a syntax the round-19 rule does not cover; the README documents only `Key`-alone-on-a-line headings. Honest controls (`Key terms`, `Key points`, `Key stage 3`) are unaffected, so a fix reading the same-line row after a lead with the existing exactness test (whole rest of line is the key text, letter or value) would not add false refusals there.

### B2. Fail-open: Matching pairs whose item contains a separator character are never found by `pairs_listed`

**Mechanism.** The text is folded by `pair_mark_form` (`src/strip.rs:530`: each of `- > : = ,` becomes `\u{1}`), but the pair entries are built as `format!("{l}\u{1}{r}")` from `fold_key(left)`/`fold_key(right)` with the raw characters kept. An entry with `,`, `-`, `:`, `=` or `>` inside the left or right can never equal the mark-formed text, so a bank text listing those pairs completes none. Round 19's skip and budget are not the cause (the walk is simply never matched); it is a silent no-match, i.e. a fail-open of the round-18 rule.

**Inputs and outputs** (`c1.mjs`; bank = Short filler + Matching keep_order false, 3 pairs; the text is in `details.description` and, separately, in a third question's stem; `publish_prepare`):

```
rights Aluminium / Soft / Roof (control)        "Bauxite - Aluminium, Chalk - Soft, Slate - Roof"           REFUSED pairs_listed (description and stem)
colon control                                  "Bauxite: Aluminium; Chalk: Soft; Slate: Roof"              REFUSED pairs_listed
rights "Aluminium, metal" "Soft, white" "Roof, grey"
                                               "Bauxite: Aluminium, metal; Chalk: Soft, white; Slate: Roof, grey"    PUBLISHED (both)
same rights, " - " separator                                                                            PUBLISHED (both)
rights "Ore-rich" "Well-known" "Ever-green"    "Bauxite = Ore-rich; Chalk = Well-known; ..."               PUBLISHED (both)
lefts "Iron-ore" "Co-balt" "Sea-salt"          "Iron-ore = Aluminium; Co-balt = Soft; Sea-salt = Roof"      PUBLISHED (both)
rights "Ratio 1:2" "Ratio 3:4" "Ratio 5:6"     "Bauxite = Ratio 1:2; ..."                                  PUBLISHED (both)
```
Every pairing of the bank is readable and nothing else in the gate catches it (key-list scan values are the paired rights, but the list is not read either). Hyphenated and comma-containing items ("Well-known", "Paris, France") are ordinary, not exotic. Same bytes with plain items are refused, so the bank-text rule works; only the item characters differ.

## (c) Non-blockers (terse)

* **CPU, new scanners at 4 MB (min of 3, `publish_prepare`)**: heading rows `Key\n1. a\n` x 480 stems 3.36 s (control `Kex\n1. a\n` 2.01 s; `Key terms` 2.09 s; `Answer key\n1. a\n` 2.50 s; `Key\n1 x\n` 2.24 s; long-row 0.86 s; `scan_budget` fires fast on `Key\n1 w0 w1...` against a 300-word key, 1.3 s). Thin headroom (3.4 of 5 s), under the bound. Pair-scan trie walk, thrash-tuned (random a/b words, 15-30k pairs, 200-300 8 KB stems with a trailing mark; `d1.mjs`): refuses `scan_budget` at 1.1-2.5 s, or publishes at 2.7-2.9 s; max seen 2.85 s (the 20 M global budget bounds it). Double-space/label shapes (`k1.mjs`, `k2.mjs`, 950 x 26 and 1000 x 4, 2 KB x 70): 0.9-2.4 s; `"1 and 2 and 3 "` 1,000 x 4 refuses `scan_budget` in 1.84 s; `"Key  "` x 26 `scan_budget` 1.76 s (both adversarial, fail closed).
* **Memory** (`l1.mjs`, fresh instance, wasm `memory_bytes` growth): labels950x26 +31.8 MB, matching480 +28.8 MB, pair-walk shape +16.8 MB; six further calls each add 0 bytes (flat). Under 64 MB; no growth across repeated calls.
* **SHA-256 memo**: native differential (`p/src/bin/memo.rs`, `cargo run --release --bin memo`): 60,000 random label-heavy choice sets, 3 passes inside one scope, a nested scope, and a second scope: cached vs uncached answer **0 mismatches in 180,000+ calls** (true fraction 0.48, so both outcomes covered). The key hashes count, every text length and the lock flag, so no collision between questions; entries die with the outermost scope. Wasm cross-call differential (`g2.mjs`, 400 random banks x 4 exports, a fresh instance per case vs one long-lived instance with unrelated publishes between): **0 differences** (ok, errors, label and formatting findings, order reasons, preview, hashes). No stale-memo or cross-publish leak found. (A trap would leave the depth counter and memo stale, but the memo is a pure function of its key, so a stale entry is still correct.)
* **100,000-step cap on honest content**: honest 3.8 MB prose bank, 1,000 questions with 3.5 KB stems, 200 keep_order Matching (`n1.mjs`): publishes, min 0.83 s. The order scan charges only list items that equal a right (2 per candidate group) and the pair walk charges per matched entry, so a cap hit needs over 25 list items each in over 2,000 groups; a long honest stem does not reach it. Fail closed in every case seen: no skip.
* **Pair-scan skip lossless**: the skip tests the same mark-formed string the entries are matched against and an entry must contain its own mark, so a start after the last mark cannot complete one. Folded full-width marks (`：`, `＝`, `－`, `＞`, `，`, `、`) and arrows (`→`, `⇒`, `⟶`, `->`, `=>`) are refused. The separators that are not read (61 tested, 19 publish: Arabic comma, ideographic comma, pipe, tilde, tab, `꞉ ∶ ⁚ ≡ ➝ ▶ ... … ·· ・ ／ / \`) are the README's exotic/documented set; a slash is the only one that is not very exotic.
* **Fuzz/traps** (`g1.mjs`): 30,000 mutated Multipart + Matching + Mc banks (tab, double space, NBSP, ZWSP, RLO, BOM, combining mark, astral, GIFT, heading, arrow and label fragments) x 6 exports = 180,000 calls: **0 exceptions or traps**, memory flat at 1.6 MB, 14 refusal codes. Deep-path (depth 12 Multipart > parts > Mc) is in every mutated bank.
* **Honest false-refusal rate** (`f1.mjs`, `f3.mjs`, `f4.mjs`): 54 honest scaffolding stems x 6 kinds (Mc shuffled and fixed, TF true and false, Short, Blank) = 324 items: 3 refused, 0.9% hard; two of those three are the documented `Key learning: 1. ...` / `Key\n1. Wall` legend with the first choice correct, one is my fixture (a stray blank). Heading honest set (11 headings incl. Key terms, Key points, Key stage 3, Key vocabulary, Key facts, Use the key, bare Key; 7 kinds with numbered rows holding answer words) = 77 items: 11 refused, 4 hard. All 4 hard are `Key terms:` (with a colon) or `Use the key` + `1.` row when the question's FIRST choice is correct (shuffled too): `answer_line`; the README documents this pre-existing "1 reads as choice 1" rule for `Key`; workable path exists (reorder choices, number from 2, say Legend), though the hint does not name it. `Key terms`, `Key points`, `Key stage`, `Key vocabulary`, `Key facts` without a colon never refuse. Seven bare-`Key` rows are `key_heading_row`, low risk, ackable. Combined honest hard-refusal rate: about 1.0% (7 of 401), all with a workable path.
* GIFT: `{F# minor}`-style braces read as GIFT (README limit).

## (d) Not run

* A profile of the 3.4 s heading-row shape (which part of the extra 1.3 s over control is the heading rule).
* Non-English honest corpus, and honest QTI/Aiken paste styles.
* Native (non-wasm) timings of the new scanners; no pre-round-19 binary for a before/after of B1 or B2.
* B1 matrix for non-English leads and Multipart part stems (the same-line gap was shown for lone questions and a bank stem).

BLOCKERS: 2
