# share-core round 18: robustness critic (critic35b)

Date: 2026-10-22. Tree: `shared-pool-phase1`, working tree. Component: `server/share-core`.
All scripts live in the session scratchpad
`/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/`
(`p/src/bin/*.rs` native probes against the crate by path, built with `CARGO_TARGET_DIR=$S/tgt`;
`node/*.mjs` against the repo's own `pkg/` wasm, 1,248,397 B, mtime 19:04, not rebuilt). `S` below is that
directory. No repo file was edited except this report; nothing committed, pushed or deployed; port 8799 unused;
owner client files untouched. The two earlier critic reports were used as mechanisms only.

Machine note: the box was shared (load average 4.5 early, 12-16 later, 24 cores). Times quoted are CPU time
(`process.cpuUsage`) where marked, wall otherwise; they agreed within about 10%.

## Baseline

| check | result |
|---|---|
| `cd server/share-core && cargo test --release` | 285 passed, 0 failed, 1 ignored (summed over every `test result:` line) |
| `cargo clippy --release --all-targets` | clean |
| `cargo clippy --release --target wasm32-unknown-unknown` | clean |
| `bash scripts/check-wasm-exports.sh` | `ok: pkg has no test-only exports` |
| `node tests/node/check.mjs` | first run 505/506 under load 15, rerun **506/506** (load flake) |
| root `cargo test --release --test share_core_fixtures` | 8/8 |

## (a) Previous findings re-run

| finding | now | evidence |
|---|---|---|
| 34b B1: whitespace-split lead skips the bank scan | **FIXED** for every ASCII and Unicode separator I could construct | below |
| 34b B2: dichotomous key `1a`/`1b` refused when the correct choice is first | **FIXED** | `round18::b2_*` pass; every position of the correct choice publishes |
| 31b B1 (200-char window) | FIXED: `"Answer: " + "x "*90 + "Olympus Mons"` -> `answer_line` | `node perf.mjs window 1` |
| 31b B2 (event cap) | FIXED (covered by `round17::b2_caps_cannot_hide_a_key`, passes) | cargo test |
| 31b B3 Amharic Matching lefts | FIXED: 4,182,277 B, CPU min of 3 = 1.61 s | `LIM=4190000 node perf.mjs amharic 3` |
| 31b B4 Multipart `2)` part stems | FIXED: 3,712,709 B (300 q x 50 parts), CPU min of 3 = 1.37 s | `node perf.mjs partstem 3` |

**34b B1 hunt (how hard).** (i) The prefilter is `may_have_answer_line` (`src/format.rs:3315`): any non-ASCII text, any
`{{`, any tab or double space always scans; otherwise ASCII words are matched in place. The only remaining
mismatch candidates are therefore ASCII whitespace the fold maps to a space but the prefilter does not:
VT (0x0B), CR, FF, and the other controls. Probe (`p/src/bin/b1.rs`, `cargo run --release --bin b1`):
`Notes.\nmark<S>scheme: Zebra` after a Zebra-keyed question in a bank:

```
space  -> answer_line     (all 5 fields: stem, choice, left, right, distractor)
CR     -> ok=true  (bank)  and ok=true (lone question too)
VT     -> ok=true  (bank)  and ok=true (lone question too)
FF, FS, GS, RS, US, NUL, DEL, BS -> forbidden_char
```
CR and VT look like leaks but are not: `text.rs` maps every line-break variant to `\n` before any scan, and the
served payload printed by the same program is `"...\nmark\nscheme: Zebra"`, a line break the scanner never joins
into a lead (documented, unchanged since round 17). The lone question (no prefilter) behaves identically, which is
the control. (ii) A format-level fuzz of the prefilter against `stem_key` itself (3,000,000 ASCII texts built from
32 leads, 15 tails, random control and punctuation separators; `p/src/bin/fz.rs`) found 395 "mismatches", every
one a CR/VT/LF-normalised text (that harness bypasses `text.rs`); none with only printable ASCII or tab. (iii) The
end-to-end differential (`p/src/bin/diff.rs`): 60,000 random lead texts (11 leads, 31 separators including NBSP,
U+2000, ZWSP, ZWJ, ZWNJ, soft hyphen, WJ, BOM, bidi LRM/RLM/RLO/PDF, LS/PS, ideographic space, CGJ, VS16, CRLF,
fullwidth letters, zero-width characters inside words, `:` U+FF1A and NBSP-colon), same text as a bank stem after a
Zebra-keyed question versus as a lone question (no prefilter), with a non-lead control to remove "= key" noise:
`lone refuses, bank passes, control does not` = **0** of 60,000 (`n=60000 ... distinct=0`). (Without the control
982 mismatches appeared; all were texts such as `zzz = Zebra` that the lone path refuses as "its own key after a
separator", so they are not lead leaks.) No new prefilter class found.

## (b) New-class blockers

### B1. CPU: single-letter label shape, 26 choices x about 950 questions, `publish_prepare` with the author's label decisions takes 10 s (min of 3), 4 MB

**Class:** CPU > 5 s (min of 3) on a 4 MB payload. This is outside the round-18 scanners (the single-space texts do
not go through the new prefilter path, and a same-size control is fast), so it is an older nonlinear path that no
shape in `check.mjs`, `mem.mjs` or the critic reports exercises. README "Costs" claims every exported call on the
hostile and field shapes stays under 3 s in Node.

**Input.** 950 Mc questions, each 26 choices; choice j is `"x - " * 28 + <letter j>` (110 bytes + letter), so each
text starts with a run of repeated single-letter labels. Payload 3,972,159 B (< 4 MB). The author keeps every
label (the normal flow: `label_findings`, then `publish_prepare` with `labels: [{question, choice:"keep"}...]`,
950 decisions).

**Commands and output** (`$S/node/phases.mjs`, `$S/node/b_cpu.mjs`):

```
$ node phases.mjs "x - " 950 110
bytes 3972159
validate                           154 ms
label_findings                     3964 ms
findings 950 ok true
validate_for_publish(labels keep)  7160 ms
strip(labels keep)                 10206 ms
strip ok true
publish_prepare(labels keep)       10281 ms
pp ok true  label_findings listed: 950

$ node b_cpu.mjs        (load average 15)
bytes 3972159 ... ok true
publish_prepare cpu ms x3 [ 11569, 12314, 12312 ] min 11569
```
Earlier at load ~4.8 the same shape (`ULEN=110 LABELS=1 UNIT='"x - "' LIM=4100000 node perf.mjs unit 3`, 4,097,589 B):
`cpu:[11724,10194,10350]` min 10,194 ms. The result is `ok:true`: the bank publishes after 10 s of CPU. Other
unit strings on the same layout (`$S/node/sweep.mjs`, CPU ms of `publish_prepare`): `"x - "` 15.8 s at 3.1 MB
(1000 q x 70 B), `"x. "` 9.6 s, `"x) "` 9.4 s, `"x - "` 150 B 9.3 s.

**Control.** Same layout and size with a two-letter unit `"xy - "` (not a label): `cpu:[2877,2328,2366]` min 2,328 ms
(`ULEN=110 LABELS=1 UNIT='"xy - "' LIM=4100000 node perf.mjs unit 3`). Plain `"a  "` double-space choices at 4.1 MB
take 4.19 s (below). The label cost is 4 to 5 times the control. It scales with the number of texts times the
labels per text (ULEN 8 -> 1.9 s, 28 -> 3.6 s, 56 -> 5.2 s at 2.7 MB, 110 -> 10 s), and falls again when texts
become so long that few fit (ULEN 2000 -> 2.4 s), so the worst point is about 100 to 150 B choices at the 1,000-
question cap.

**Impact.** A Worker publish of a hostile or merely odd (`x - x - x -` repeated markers, e.g. a pasted scheme) 4 MB
bank holds the isolate for 10 s of CPU, twice the gauntlet bound and 3.3 times the documented 3 s. Memory is fine
(35 MB). Fail-open is not involved; the answer is correct, only slow.

**Not run:** which function dominates (no profiler), and a native (non-wasm) timing.

### B2. Hint channel: a numbered row under a `Key` / `Answer key` heading line is not read, so a lone question or a bank publishes its own key

**Class:** a syntax family that leaks the key; inconsistent with the same words on one line or any other heading.
I could not date it (HEAD is an early snapshot, no pre-round-18 binary), so it may predate round 18; neither the
round-16 nor round-17 critic reports mention it, and the round-18 couplet exemption is not the cause (the rows
below contain no glued letter).

**Mechanism (source).** `names_key` at `src/format.rs:4282` is called with `texts = false` for the leads
`key`, `correct`, `kunci`, so the key text after them is never compared; the `Answer key\n1. Paris` form reaches
that path (the lead `answer` is followed by the word `key`, not a separator). The numbered-letter rule reads only
the same line and needs two values or a glued letter, so one `1. Paris` row is nothing. Honest dichotomous keys
(`Key\n1a ... Spider`) depend on the same tolerance, so the fix has to separate the two shapes.

**Inputs and outputs** (`p/src/bin/b6.rs`, `b7.rs`; `publish_prepare` with `{"hide_key":true}`; the Mc
`keep_order` row also carries `order_acks:[{question:0}]`):

```
Short sample "Paris":
  "Answer key\n1. Paris\nCapital of France?"        PUBLISHED  (lone question and bank of 1)
  "Key\n1. Paris\nWhat is the capital of France?"    PUBLISHED  (served stem is exactly that text)
  "Answer key\n1. Paris"                              PUBLISHED
Mc [Talc, Quartz, Diamond, Gypsum], correct Quartz:
  "Answer key\n1 Quartz\nWhich is hardest?"          PUBLISHED
Mc keep_order (position B, students see the order), after the order acknowledgement:
  "Answer key\n1. B\nWhich is hardest?"              PUBLISHED   (also "1) B", "(1) B", "1 B", "1B  Quartz")
Controls, same Short key, same question:
  "Answers\n1. Paris"          answer_line
  "Solution\n1. Paris"         answer_line
  "Answer key: 1. Paris"       answer_line
  "Answer key\nParis"          answer_line
  "Key\nParis"                 answer_line
  "Key: Paris"                 answer_line
```
A matrix of 6 headings (Answer key, Answers, Key, Solution, Mark scheme, Model answer) x 9 row forms (`1 K`, `1. K`,
`1) K`, `(1) K`, `Q1: K`, `a) K`, `• K`, `1. K` plus a second row, `Q1 - K`) leaks 16 of 54 for Short and for Mc (all
and only the `Answer key` and `Key` headings, 8 row forms each), and 30 of 54 for True/False (`p/src/bin/b5.rs`).
Banks of three or more questions with a full numbered list are refused (`bank_key_list`; 16 forms tested in
`b3.rs`, all refused, so the bank-wide rule is not broken); the hole is the single numbered row for the question's
own key (lone, bank of 1, or a bank where only this question's row is present).

**Why it matters.** "Answer key" over a numbered row is the usual pasted-worksheet shape. The served stem is the
text itself.

**Possible fix:** read key texts after `key` / `answer key` when the next line starts with a number or letter
label and the row's value is this question's key, keeping `Key` followed by `1a/1b` couplets and legends.

## (c) Non-blockers (terse)

* **Round-18 prefilter cost.** Texts with a tab or a double space always take the full scan. Measured
  (`node perf.mjs unit`, `LIM=4100000`, CPU/wall min of 3): `"a  "` x choices 4.19 s (4,096,448 B); `"x:  y:  "`
  4.12 s (first run 4.81 s at load 15); `"x:  "` 3.25 s. Under 5 s but with thin headroom and about 40x the
  single-space cost (0.07 s). Matching with double-space texts 2.4 s (`mt_dbl`, 3.9 MB). If B1's label cost is
  fixed, this is the next shape to watch. Per-text base cost is about 23 microseconds, per byte about 1 microsecond.
* **New scanners at 4 MB** (min of 3 wall ms, `perf.mjs`): Matching keep_order with rights in desc 1.37 s; Matching
  rights repeated in 3.9 MB of stems 1.73 s; `pairs_listed` stems refuse in 0.5 s; Multipart key-list (7,500 part
  keys + numbered stems) refuses `bank_key_list` in 0.69 s; the same with `Answers:` leads -> `scan_budget` 0.70 s
  (adversarial, fail-closed); dichotomous couplet stems 1.40 s; Matching rights as stem tokens 1.27 s (publishes: no
  numbers, fewer than three in order, as designed). Description is capped at 2 KB, so no description shape is large.
* **Memory:** 24 cycling publishes of three 3.5 MB double-space banks in one instance: growth flat at 23.9 MB from
  the first call to the 24th (`$S/node/grow.mjs`); worst single call growth about 35 MB (< 64 MB).
* **Fuzz / traps:** 20,000 mutations x 3 exports (60,000 calls) of a Multipart + Matching bank with tab, double space,
  NUL, NBSP, ZWSP, RLO, BOM, combining mark, astral, escapes, lone surrogates and bracket mutations: 0 exceptions
  or traps, instance alive after, 25.3 MB at the end. Codes spread over 12 reasons including `key_in_stem`,
  `too_deep`, `forbidden_char`.
* **Fail-closed caps:** `scan_budget` fired in the adversarial `"key  "`x24-per-choice shape (2.4 s) and the
  `Answers:`-numbered Multipart shape; both refuse, never skip. No honest case produced `scan_budget`.
* **Honest bank corpus (round-18 rules).** `p/src/bin/honest.rs <n>`: banks of 8 to 27 questions from seven lexicons
  (chemistry symbols, capitals, organelles, dates, rocks, CS types, arithmetic) over Mc, Tf, Short, Matching and
  3-part Multipart; each bank first published with a neutral description (control: **0 hard refusals** in 800 of each
  corpus) and then with each description style. "Hard" = an error with `ack_allowed != true` (label and order
  acknowledgements are workable and excluded). 800 banks per row; hard refusals:

  | description style | all kinds (n=600) | Matching only | Multipart only | no Matching, no Multipart |
  |---|---|---|---|---|
  | plain / "correct answer ... hint cards" sentence | 0% | 0% | 0% | 0% |
  | `1a 1b 2a 2b` outline heading | 0% | 0% | 0% | 0% |
  | dichotomous key note (`Key\n1a ...`) | 0% | 0% | 0% | 0% |
  | topics (the stems' terms) named, comma list | 0.2% | 1.0% | 0.2% | 0.0% |
  | numbered outline of three of the stems' terms | 0.0% | 0.4% | 0.1% | 0.0% |
  | "Covers a, b, c." answers, shuffled, no lead | 3.3% | 0.6% | 1.4% | 2.6% |
  | "Vocabulary: a, b, c, d." answers, shuffled | 29.5% | 6.6% | 19.4% | 33.6% |
  | "Sections: 1. a 2. b 3. c" answers, shuffled | 10.3% | 44.4% | 15.9% | 7.9% |

  The `1a/1b` outline and the dichotomous key never refuse. The high rows are descriptions that list answer values:
  the refusals are `bank_key_list` / `bank_word_bank_order`, always on `bank.details.description`, `ack_allowed:false`,
  hint "remove the answer from it". Pure Mc/Tf/Short banks refuse at the same or higher rate (33.6% for the
  "Vocabulary" list), so round 18's Matching/Multipart additions are not the main source; the new part is the
  numbered list over Matching rights (44.4% in my small lexicons, where answers repeat heavily). The corpus
  over-represents repeated answers (7 to 12 distinct values), so treat the high rows as an upper bound. A workable
  path exists in each case: `Sections: Igneous, Sedimentary, Metamorphic` publishes, `Sections: 1. Igneous
  2. Sedimentary 3. Metamorphic` is refused (`p/src/bin/b8.rs`: `bank.details.description`, `ack_allowed=false`),
  and removing the numbers or the list clears it. Not a blocker; the hint is generic and does not say that
  numbering and listing the answer classes is what triggers it.
* A single glued letter followed directly by a word (`1B then ...`) is read as a label: documented in README.

## (d) Not run

* A profile of which function owns B1's cost; native-vs-wasm timing of B1.
* An uncommitted-vs-round-17 comparison for B2 and B1 (no earlier binary; HEAD is far older than round 17).
* Multipart shapes at the full 4 MB with 50 parts (limited by the 60,000-object and 200,000-value caps, so the
  largest valid Multipart payloads are 3.0 to 3.9 MB and run in under 0.8 s).
* Non-ASCII separators inside the prefilter's ASCII path are unreachable by construction (any non-ASCII text scans),
  so no further test.

BLOCKERS: 2
