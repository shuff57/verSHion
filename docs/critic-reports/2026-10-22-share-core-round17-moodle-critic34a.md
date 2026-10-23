# share-core review, round 17 (critic34a): Moodle lens

**Verdict: the six round-16 blockers are fixed; two new-class blockers remain.** Both are served bank
texts that give away keys the round-17 "every served field is read" scan does not enumerate:
Multipart part answers and Matching pairings/rights. Nothing else I tried produced a new-class
finding.

## Build, environment and baseline (read first)

All wasm repros use a copy of `server/share-core/pkg` taken at 15:30 and never rebuilt:
`sha256 84e6abc4ba3cab597ba3a30955b8c906e760f05417f7cdf452befb7593a76253`, 1,241,087 bytes,
mtime 13:05:36 (byte-identical to the live `pkg/share_core_bg.wasm` throughout, checked at 15:40).
`src/strip.rs` (13:03:31) and `src/format.rs` (12:44:57) are older than that build, so the code under
test is the built code. `src/hash.rs` (14:10), `src/import.rs` (14:07) and `src/limits.rs` (14:17)
have later mtimes; the scanners that decide these findings (`strip.rs`, `validate.rs`, `format.rs`,
all older than the build) are unchanged, and the omission quoted in (b) is visible in the current
`strip.rs`. Other agents shared the machine: load
average 10–24 on 24 cores (`wasm-bindgen`, several node servers and, at the start, a second
`cargo test --release` from another session). Timing is process CPU (min of 3 for `cpu.mjs`, min of 7
for `mem3.mjs`) unless stated; wall-clock failures are called out as such.

| baseline command | result |
|---|---|
| `cd server/share-core && cargo test --release -- --test-threads=4` | **PASS, exit 0** (282 listed = 281 pass + 1 ignored). One earlier run, while a second full `cargo test` and `node check.mjs` ran, failed `round6::costs_are_linear` at 3.091 s; the other session's concurrent run failed the same test at 3.014 s. Its bound is an absolute 3.0 s (`loaded()` only relaxes under `SHARE_CORE_MUTATION`), and the second run of the same command passed with exit 0: the documented load flake, not a race or data loss. |
| `cd server/share-core && cargo clippy --release --all-targets` | exit 0, no warnings |
| `cd server/share-core && node tests/node/check.mjs` | **505/506**. The one failure is `f_many_small/strip: 3442 ms >= 3000 ms`; min of 3 separate runs 2849 / 2982 / 3100 ms (2.85 s). Not the >5 s CPU blocker rule; see (c). All trap, memory (≤6x) and correctness checks passed, including `blank170` (+29.4 MB, ok). |
| `cd . && cargo test --release --test share_core_fixtures` | 8/8, exit 0 |

Repro scripts are in `/tmp/critic34a.CkZeg6/`; each command below is run from that directory with
`node <script>` and imports the copied `pkg/` (`h.mjs`).

## (a) Previous findings (critic31a, round 16)

| # | repro now | result |
|---|---|---|
| B1 fail-open 64-entry cap in the bank answer-key scan | `node p1.mjs` (120/140/200 T/F questions, key as questions n-5…n in the description, `True`/`False` forms, and in a stem) | **FIXED** — all 16 lines `hard key_in_stem/bank_key_list(high)`; no size is clean. |
| B2 fail-open 4,096-event cap | `node p2.mjs`, `node p3.mjs` (`Readings: 7 7 …` × 0…3000 + a page-2 key) | **FIXED** — the numbered key is refused at every repeat count (`bank_key_list`, and the control with no repeats is refused too). |
| B3 part stems / left items / topics / rights / distractors / choices / name / author / licence not read against other questions' keys | `node p4.mjs`, `node p6.mjs` | **FIXED** — every one of those fields is now `hard bank_key_list`; `Answer: Quartz` in a part stem/left/topic is `answer_line`; the main stem too. (`bank.questions[i].id` is still not read against keys; an id is `[A-Za-z0-9_-]` only and carries no teacher text.) |
| B4 frozen order exposes the key by position | `node p22.mjs`, `node p23.mjs` (7 layouts × keep/strip + 12 keep_order layouts) | **FIXED** — all keep-order/kept-label layouts are `hard bank_key_list` **before** any order ack; the stripped/no-fixed-order controls are clean, which is correct (shuffled letters name nothing). |
| B5 false `scan_budget` on honest short data | `node p19.mjs`, `node p20.mjs`, `node p21.mjs`, `node b5_glossary.mjs` | **FIXED** — 63-row `Pupil,Key Stage,Hours` clean (the 80-row CSV with the same header clean too; the 80-row TSV/pipe/space/HTML variants with a `KS` header clean); the same table in a Multipart main stem, 64/65/120 rows: clean; 64–300-row caretaker key log, field notes, `see answer sheet`, `hint of rain`, `Definition:` glossary (65/120 entries): clean. Rows past the 8 KB stem limit now give the ordinary `too_long`, not `scan_budget`. |
| B6 166 MB wasm memory on a valid 4 MB payload | `node mem3.mjs` (4,172,388-byte blank170) | **FIXED** — `publish_prepare+hide_key` is `ok`, wasm memory 37.6 MB (~36 MB growth, under the 40 MB ceiling), CPU min 1.31 s over 7 runs (was 165.8 MB / 4.13 s). Scaling 0.90 MB→10 MB, 1.96 MB→19 MB, 2.78 MB→26 MB. |

The round-17 tests in `tests/round17.rs` (`a1`…`a6`, `b1`…`b5`) pass in the full run.

## (b) New-class blockers

### B34a-1 — Multipart part answers are not in the bank key-list scan (fail-open enumeration)

**Mechanism.** `strip::bank_key_list_errors` (the round-17 "an answer key of any layout in any served
text of a bank" scan, `src/strip.rs:1084-1156`) builds its values from `Kind::Mc`, `Kind::Blank`,
`Kind::Short` and top-level `Kind::Tf` only (`src/strip.rs:1109-1133`); `Kind::Multipart` and
`Kind::Matching` fall through `_ => {}`. `strip::bank_order_errors` (`src/strip.rs:963-992`, the
word-bank-in-question-order scan) has the same three-kind list. The cross-question answer-line scan
(`bank_key_errors`) *does* include part keys via `each_key`, but only reads answer lines
(`lines_only`), so a numbered list with no lead is not read. Consequently a bank text that is served
to students can name each Multipart question's part answers, and the round-17 promise ("Every served
field of a bank is read against every question's key… A key list in a bank's name, description or any
stem is refused") does not hold for the Multipart kind — Moodle's multianswer/cloze-shaped question.

**Exact input** (`blocker_a.mjs`, first two lines): bank `b00101a2b3c`, three questions, each
`Multipart` with part (a) an Mc whose correct text is `Quartz` / `Basalt` / `Mohs` and part (b) a
Short; `bank.details.description = "1 Quartz 2 Basalt 3 Mohs"`; `publish_prepare` with
`{"hide_key":true}`.

**Exact output:** `{"ok":true}`. The served payload keeps
`"details":{"description":"1 Quartz 2 Basalt 3 Mohs",…}` and serves the three parts' choices with
`correct:false` and the same texts in a fresh shuffle (one run:
`[["Talc","Gypsum","Quartz"],["Talc","Basalt","Gypsum"],["Talc","Mohs","Gypsum"]]`); the questions are
served in this order.

**Controls ruling out the trivial explanation.** The identical description with top-level `Short`
samples (`Quartz`/`Basalt`/`Mohs`) is `key_in_stem/bank_key_list(high)@bank.details.description`; so is
the same with `Mc` keys and with `Blank` keys. The same text with Multipart **Tf** parts
(`"1 T 2 F 3 T"`) publishes clean while three top-level `Tf` questions are refused
`bank_key_list`. Putting `"Check your work. 1 Quartz 2 Basalt 3 Mohs"` in a fourth question's stem is
also clean, so the gap is the key kind, not the field.

**Repro:** `cd /tmp/critic34a.CkZeg6 && node blocker_a.mjs`

### B34a-2 — Matching pairings and rights in served bank texts (new hint channel)

**Mechanism.** The left→right pairing is checked only in the Matching question's **own** stem:
`pairs_listed` lives in `stem_key_errors`' `Kind::Matching` arm (`src/strip.rs:787-815`) and is never
run on `bank_texts`. The bank key-list automaton does not model a left–right pair at all, and its
value set (B34a-1) omits Matching rights too, so `bank_order_errors` cannot see a rights list either.
Moodle matching (`=left -> right` GIFT, a pasted answer key) in a bank's description, name or another
question's stem is therefore served.

**Exact inputs / outputs.**
1. `blocker_a.mjs`: Matching pairs `Bauxite→Aluminium, Chalk→Soft, Slate→Roof` (+ a Short question);
   `description = "Bauxite - Aluminium, Chalk - Soft, Slate - Roof"` → `{"ok":true}`; the served
   description is unchanged and the question is served with its lefts and word bank shuffled
   (observed in one run `["Slate","Chalk","Bauxite"]`, `["Soft","Aluminium","Roof","Granite"]`; the
   order changes per run). The **control** — the same text in the Matching stem — is
   `key_in_stem/pairs_listed(high)@bank.questions[0].stem`.
2. Moodle GIFT form: `description = "=Bauxite -> Aluminium\n=Chalk -> Soft\n=Slate -> Roof"` →
   `{"ok":true}`; in the Matching stem the same block is `answer_line(high)` **and**
   `pairs_listed(high)`.
3. `probe_rights.mjs`: Matching with `keep_order:true`, `description = "Aluminium, Soft, Roof"`
   (the rights in pair order) → `{"ok":true}`, and the served lefts are in author order
   `["Bauxite","Chalk","Slate"]`, so the description plus the served left column is the whole
   pairing. **Control:** the same list in the Matching stem is
   `key_in_stem/word_bank_order(high)@bank.questions[0].stem`.

Question order is served (banks are not shuffled), and the bank `description` is part of
`stripped.payload`, which the Worker returns/stores as `student_view`; the README's "What students
will see" names the bank description explicitly.

**Repro:** `cd /tmp/critic34a.CkZeg6 && node blocker_a.mjs && node probe_rights.mjs`

**Why these are new class, not variants.** They are not spelling/language variants and not
documented limits: the round-17 text claims coverage of exactly these fields and key kinds, and the
same strings are refused when they are a higher-level key or when they sit in the Matching stem. The
fix is a silent skip of key material (the spirit of the fail-open-cap rule), and in case 3 a
`keep_order` freeze is combined with a served list, which is "a wrong freeze/shuffle that exposes the
key by position" in effect (the freeze is the author's, the leak is the list).

## (c) Non-blockers, variants, and one baseline regression

* **Honest corpora (round-17 fixes hold).** `node banks2.mjs`: 18 banks / 1,254 questions
  (the 627-item corpus), `{clean: 16, ack: 2}, 0 hard, 0 freeze`; the ALL bank is the same
  parenthetical-outlier acknowledgement as before; the Quechua item is clean. `node markers.mjs`:
  158 markers, **125 hard, 18 low, 15 miss**. The 15 misses are the set critic31a already classified
  as documented/best-effort or as non-leaks: OCR ticks (`Gills v`, `/`, `J`), `Gills A1`,
  `Gills[KO1]`, `Gills %100%`, `Gills\t1`, trailing-space padding, `(T) F` OCR, `Chanterelle →
  false gills` (partial right), matching numbers removed by Strip, the letters in a name/description
  whose choices are shuffled (`Quiz 3 — A B C T F`, `Field biology (1A 2B 3C)`), and a key given as
  data in another stem (`Spores come from the gills.`). `Revision words: gills, cuckoo, Cretaceous,
  index fossil.` is a partial restatement (only two exact keys) — the documented order threshold,
  not a new channel.
* **Residual honest false positives (not new; rewording fixes them).** A TSV table whose **last cell
  of a row equals the key** gives `answer_line` (the round-15 spreadsheet-row rule; the CSV form is
  clean): `header + "P24\tKS3\t7.6"` with key `7.6`. `Key 1a` (a dichotomous couplet) gives
  `answer_line` (the numbered-answer-key rule). A `Correct?`-column QA log gives `answer_line` at
  ≥7 rows. A dichotomous key pasted in the stem is still refused when the correct choice's leaf is
  the answer (p20 "key first"), as critic31a recorded. Renaming (`KS3`, `Couplet`, `OK`, moving the
  key to an image) works.
* **Single-character keys are not in the bank key-list scan** (`1 A 2 B 3 C` with Short samples
  `A/B/C` is clean, while `1 Au 2 Ag 3 Cu` is refused). This is the deliberate one-letter ambiguity
  guard in `bank_key_list_errors` (`v.chars().count() >= 2 || all digits`), a variant of the class
  above, not a new channel; one-letter answers are rare and indistinguishable from choice labels.
* **Baseline regression, not a blocker by the stated rule.** `node tests/node/check.mjs` is 505/506:
  `f_many_small/strip` took 3442 ms against its 3000 ms bound; min of three separate runs 2849 / 2982
  / 3100 ms. The round-17 bank-wide scans (every served text folded and read by the key-list
  automaton, ~26,000 choice texts here) are the added work; the README's "under 3 s in Node" is now
  marginal for this shape and one run crosses it. Below the >5 s blocker threshold.
* **CPU on 4 MB adversarial shapes (min of 3, `node cpu.mjs`)** — all under 5 s: trie_long_key 1.79 s
  (ok; was 2.60 s `scan_budget` in round 16), trie_near_miss 3.12 s (`scan_budget`), lists_shared4
  1.35 s, mp_cross_keys 1.90 s, topics_many_keys 2.53 s (2.2 MB), tf_numbered_events 0.47 s,
  leads_64 2.46 s, matching_pointer_lists 2.14 s, html_tags 1.43 s, cjk_runs 1.37 s.
* **Documented limits seen and not counted:** the 200-character start window; short keys after a
  lead; `Key:` naming a key text only by letter/value (the `Match key:` variant of B34a-2 escapes
  for that reason too — the bare/GIFT/numbered forms above do not depend on it); >2 qualifier words;
  `::title::`; `####` feedback; pairs in an HTML table; another question's key as data.

## (d) What I did not run

* A clean second `cargo test --release` on an idle machine: the passing run had load 10–24 from
  other sessions; the earlier 3.09 s / 3.01 s `round6` failures are attributed to that load (its
  bound does not adapt to it).
* The `--ignored` full amplification sweep (`repeated_tokens_stay_linear_full`) and the mutation
  runner.
* Moodle itself: no sparse clone was needed; the Moodle comparisons are the ones already in the
  round-16 report and the README.
* Shuffle-fairness statistics: round 17's change list and the code I read keep the round-16
  `strip_mc`/`word_bank` shuffle algorithm (Fisher–Yates on the unlocked slots, left column and
  word bank), so critic31a's 4,800-run fairness result stands unmeasured by me.
* Argon2id parameters, the Worker end-to-end publish path, and the app's own fixture/import code
  beyond `tests/share_core_fixtures`.
* A dedicated wasm fuzz run: the full `check.mjs` trap/fuzz block passed (only the timing check
  failed).

BLOCKERS: 2
