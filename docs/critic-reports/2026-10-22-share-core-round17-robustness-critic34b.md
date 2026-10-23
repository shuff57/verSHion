# share-core round 17 — robustness critic (critic34b)

Date: 2026-10-22. Tree: `shared-pool-phase1`, working tree (round-17 changes uncommitted).
Component: `server/share-core` (Rust→wasm publish gate). All work in
`/tmp/critic34b.O71257/js/` (scripts) and `/tmp/critic34b.O71257/out/`; no repo file was edited
except this report, `pkg/` was not rebuilt, port 8799 was not used, and the owner's client files
were not touched.

Everything below ran against the repo's own `pkg/` (production build, file time 13:05, after the
last source edits at 13:03). `bash scripts/check-wasm-exports.sh` is clean (no test-only exports,
`strip_with_seed` absent, `reinit` present); source and `pkg/share_core.d.ts` export the same set.

Commands below assume `cd /tmp/critic34b.O71257/js`. `common.mjs` loads
`/home/shuff57/Documents/GitHub/verSHion/server/share-core/pkg/share_core.js` once.

## Baseline (run first)

| check | result |
|---|---|
| `cargo test --release` (share-core) | **281 pass, 1 ignored, 0 failed when the binaries are counted together**, but no single uninterrupted run got there: a competing `cargo test` from another process held the machine (load 20-62, 24 cores) and two *timing* assertions flaked — `round6::costs_are_linear` ("ps.rs Worker pattern: 3.014 s" against a 3.0 s bound, twice) and `round7::adversarial_shapes_are_cheap`. Both pass alone (`cargo test --release --test round6 costs_are_linear` → ok; `cargo test --release --test round7` → 15/15). All other binaries passed in the full run; `round7..round9`, `wasm_api`, `unicode_canary` were run separately and pass (15/15, 11/11, 8/8, 4/4). These are load flakes, not findings. |
| `cargo clippy --release --all-targets` | clean, exit 0 |
| `cargo clippy --release --target wasm32-unknown-unknown` | clean |
| `bash scripts/check-wasm-exports.sh` | `ok: pkg has no test-only exports` |
| `node tests/node/check.mjs` | **506/506**; the first run under load gave 505/506 (one timing-sensitive check), the rerun 506/506 |
| `cargo test --release --test share_core_fixtures` (repo root) | 8/8 |

I did not rebuild `pkg/`; its mtime is after every source file, its exports match, and the round-17
behaviours below are present in the wasm, so the binary is the round-17 build.

## (a) critic31b's four blockers, re-run

| # | finding | result now | repro |
|---|---|---|---|
| B1 | 200-character window cut a key off | **FIXED.** Long key (228 chars) after `Answer:` / `Answer: it is:` / `Answer: probably` / `Answer: B.` → `answer_line`; "Olympus Mons" at 180/185/190/195 chars of filler → `answer_line`; 205 → `ok:true` (the documented window). Also refused in a bank stem, a bank description and a Multipart part. Only a key starting at *exactly* 200 characters after the lead is not read (starts `< 200`); that is the documented window edge, reported below as a non-blocker. | `node b1_window.mjs` |
| B2 | 4,096-event cap in the bank key scan failed open | **FIXED.** `"zq "×{0,85,95,300}` + `2 T 3 F` / `Results 2 T, 3 F` / `Q2 (T) Q3 (F)` → `bank_key_list` in all 12 cases; a control with the same decoys and no key list → `ok:true` (the refusal is the key, not the size); the word index finds a part's key after 70 decoys (`part_states_key`). | `node b2_evcap.mjs` |
| B3 | Amharic-lead Matching 8.7 s | **FIXED per the rule; near the limit under load.** 4,151,744 B, `መልስ:` lefts. min CPU of 3: **2.29 s** at load 24; earlier under load 62: 3.30, 3.36 and 4.91 s. Never >5 s in any min-of-3. | `node time_shapes.mjs matchleft 649` |
| B4 | Multipart `2)` stems 11.4 s | **FIXED per the rule; near the limit under load.** 4,185,012 B (325 questions; 326 gives 4,197,890 B > the 4 MB body limit). min CPU of 3: **2.88 s** at load 24, 4.13 s under load 62. Never >5 s. | `node time_shapes.mjs partstem 325` |

critic31b's slow Malayalam verdict shape also stays under: 2.54 MB, 1.81 s (`sweep_cpu3.mjs`).

## (b) New-class blockers

### B1. Fail-open prefilter: a tab or a run of spaces inside a multi-word lead skips the whole cross-question scan of a bank

**Class:** a cap/filter that silently skips a check, leaking the key (and a hint channel).

**Mechanism.** Round 17 added `may_have_answer_line` (`src/format.rs:3303`) as a cheap necessary
condition before the bank-wide answer-line scan (`src/strip.rs:949`). It compares the **raw** text
byte-for-byte with the spaced leads, including `"mark scheme"` and `"indicative content"`. The
scanner it gates (`stem_key`) compares the **served, NFC'd, case-folded** text, where
`served_text`/`fold_key` collapse every run of whitespace to one space. So `mark␠␠scheme:` or
`mark\tscheme:` makes `may_have_answer_line` return `false`; `bank_key_errors` then `continue`s
past the whole text and no key is looked for. The two multi-word ASCII leads that are not also a
single-word lead are exactly the affected ones (`model answer`/`acceptable answer` contain the
lead `answer`, so the prefilter still catches those).

The single-question path does not use this filter, so the same text is refused as a lone question
and published inside a bank. The leak is cross-question: question q1's served stem, choice, left
item, right item, distractor, part stem or part choice shows q0's answer.

**Exact input** (`node b3_prefilter_leak.mjs` builds it; this is the minimal one — two spaces
between `mark` and `scheme`):

```json
{"schema_version":2,"bank":{"id":"b00000001","name":"B","created":0,"modified":0,
 "details":{"author":"T","description":"","license_name":"CC BY 4.0","license_url":"https://example.com/licence"},
 "questions":[
  {"id":"q00000001","stem":"Name it.","kind":{"Mc":{"answers":[{"text":"Zebra","correct":true,"locked":false},{"text":"Lion","correct":false,"locked":false}]}},"points":1,"difficulty":null,"topics":[],"created":0,"modified":0},
  {"id":"q00000002","stem":"Notes.\nmark  scheme: Zebra","kind":{"Tf":{"correct":true}},"points":1,"difficulty":null,"topics":[],"created":0,"modified":0}]}}
```

**Exact output** (`publish_prepare(payload, {"hide_key":true})`):

```
ok: true
stripped.payload.bank.questions[1].stem = "Notes.\nmark scheme: Zebra"
```

The served form normalises the whitespace, so students read the answer line plainly; q1's own key
is not `Zebra`. `strip(payload, {})` returns the same (`node mem_calls.mjs` prints
`strip two spaces: ok=true q1 served holds Zebra=true`).

**Controls (all in the same script).**

* same bank text with **one** space → `answer_line` (`key_in_stem`, not acknowledgeable);
* the lead with a tab and a non-key word (`mark\tscheme: Walrus`, q0's key `Zebra`) → `ok:true`
  (no false positive);
* the same text as a **lone question whose own key is Zebra** → `answer_line` (the scanner sees
  the lead; only the bank prefilter misses it);
* the bank description with the same double-space lead → refused (that path has no prefilter).

**Differential (N runs).** 48 combinations (2 leads × 3 separators × 8 served field kinds):
**42 leak**. Separators that leak: two spaces, three spaces, tab, and space+tab; `\n`, `\r`, `\r\n`
and VT do not (the scanner itself does not fold a line break into the lead, so those are not
leaks). Field kinds that leak: stem, Mc choice, Matching left, Matching right, distractor, part
stem and part choice. The topic case is caught by a different scan (`topic_is_key`, the key stands
in the topic as a token), so it is not evidence about this filter. Deterministic: 20/20 runs of
one leaking payload → `ok:true`.

**Fix direction.** Fold the whitespace in the prefilter (compare the lead with whitespace runs
collapsed), or simply return `true` for any text containing a tab or two consecutive spaces, so
the text gets the full scan (such texts are rare and the per-text/global budget already bounds the
cost).

### B2. Honest hard refusal: a dichotomous key labelled `1a`/`1b` after the header `Key` is read as a numbered answer key when the correct choice is the first one

**Class:** an honest-teacher hard refusal with no workable path (the suggested action cannot be
performed without destroying the content). The round-17 A4 honest-table test uses this exact
shape but asserts only `!scan_budget`, so it does not catch it.

**Mechanism.** The lead `Key` (`ANSWER_LEADS`, `src/format.rs:3041`) followed by a newline opens
the first line after it, `1a  Has six legs … go to 2`. The numbered-values block
(`src/format.rs:3955-3975`) reads the first token `1a` as "question 1, answer a"; a correct choice
at index 0 puts `a` into `key.letters`, so `answer_line` is returned (high risk,
`ack_allowed:false`).

**Exact input** (`node b_dichotomous.mjs`; the full stem is in the script):

```
Use the key to identify the invertebrate.

Key
1a  Has six legs ................ go to 2
…
3a  Eight legs .................. Spider
3b  Many legs ................... Centipede

An animal has eight legs. What is it?
```

with choices `["Spider","Fly","Springtail","Centipede"]` and correct = index 0.

**Exact output:**

```json
{"ok":false,"author_only_refusal":true,"errors":[{"code":"key_in_stem","path":"question.stem",
 "reason":"answer_line","ack_allowed":false,"author_only":true,
 "hint":"Students see this text: remove the answer from it (answer lines, GIFT or Cloze answer syntax, the paired item)."}]}
```

**Controls (same script).**

* the identical question with correct = B, C or D → `ok:true` (the refusal depends on where the
  correct choice happens to sit; the same worksheet publishes or not depending on choice order);
* the same key with `Step 1a` as the label → `ok:true`;
* the same key without the `Key` header → `ok:true`;
* a plain `1a`/`1b` list with no lead at all → `ok:true`;
* 20/20 runs (correct = A) → refused. Deterministic.

This is a canonical biology worksheet form (`1a/1b` couplets under a `Key` heading), named as an
honest shape in the round-17 changes. The hint asks the teacher to remove the answer, which is
the content of the key. The only workarounds are rewording the couplet labels or reordering the
choices so the key is not at position 1. The rule predates round 17 (round 12's numbered-letter
answer keys), but it contradicts the A4 claim and was not previously reported.

## (c) Non-blockers / variants

* **Same key-list channel, kinds not covered (not counted — same class as the round-17 B4 fix).**
  `node gaps.mjs`:
  * Multipart: bank questions with `keep_order` and Mc parts, description `1 C 2 B 3 D` (each
    question's part 0 key is C/B/D). Before the ack: `order_ack_required` only; **after the ack
    the bank publishes `ok:true`**, the description is served verbatim and part 0's choices are
    served in author order (`["alpha","beta","gamma","delta"]`), so `1 C 2 B 3 D` names every
    answer. The `letters` table in `bank_key_list_errors` (`src/strip.rs:1118`) collects only
    top-level `Kind::Mc`; a top-level-Mc control with the same description is `bank_key_list`.
  * Matching: description `Key: Mammal, Fish, Amphibian, Reptile` with `keep_order` Matching
    questions → `ok:true`; the served lefts are in pair order, so the list gives the pairing.
    `bank_order_errors` groups only Mc/Blank/Short, and `bank_key_list_errors` has no Matching
    arm. Controls: the same list over Short samples is `bank_key_list`, and the answer-line form
    `Answer: Mammal, Fish, Amphibian` **is** caught (`answer_line`).
* **Window boundary.** A key starting exactly 200 characters after the lead (40 × `"word "`) is
  not read; the documented window is starts `< 200`. Not a leak beyond the documented limit.
* **Timing headroom.** The former slow shapes are under 5 s but not with much margin under load
  (B3 up to 4.91 s min-of-3 at load 62, B4 4.13 s). Not blockers by the rule; worth watching.
* **Memory.** 4 MB shapes grow wasm memory by at most 32 MB (blank170 29.5 MB, 1000×26 choices
  31.9 MB, CJK choices 31.7 MB), well under 64 MB; 12 repeated calls in one instance (2.5-3.7 MB
  shapes, alternating) plateau at 30.8 MB (no per-call growth); `limits` higher than the defaults
  is refused (`bad_options`), so a caller cannot raise the memory budget. (`sweep_cpu*.mjs`,
  `mem_calls.mjs`.)
* **No traps.** 60,135 calls (structured + raw-byte + hostile options) across all 9 exports
  (`fuzz_traps.mjs`), and 30,000 valid-shaped payloads of which 8,937 passed validation and went
  through `publish_prepare` + `strip` + both findings exports (`fuzz_deep.mjs`): 0 exceptions,
  0 responses without `ok`, 0 served payloads that failed to revalidate, 0 `"correct":true` in a
  served payload.
* **Shuffle.** Key position uniform for 2/3/4/5/8/26 choices (χ² 0.1-32.1, df 1-25) and for the
  Matching left column and word bank (χ² 29.4 df 29, 41.6 df 45); no positional leak
  (`shuffle_check.mjs`).
* **Honest corpus.** 627 items through `publish_prepare` with the round-17 driver: 0 hard
  refusals, 5 acknowledgements, as documented (`honest_corpus.mjs`). The other A4 honest shapes
  (63-row sleep table, caretaker's `Key` log, field notes log) are `ok:true`
  (`honest_shapes.mjs`); only the dichotomous key above is refused.
* **pkg vs source.** Same export set, `reinit` present, no `strip_with_seed`; not rebuilt.

## (d) What I did not run

* A single uninterrupted `cargo test --release` with no competing build: another process ran
  `cargo test` on the same target dir for the whole session (load 20-62), so the full run flaked
  two timing assertions; I ran every binary and re-ran the flakes alone (all pass) but did not get
  one clean 281/1 process exit.
* The `--ignored` amplification sweep (`repeated_tokens_stay_linear_full`), and the
  `test-seed` wasm build.
* Debug/overflow-check native builds, and a source audit of every slicing site (my trap evidence
  is the fuzz above).
* The previous critic's 2.6M-call structural fuzz; mine is 90k calls.
* `tests/node/mem.mjs` standalone (its modes are exercised by `check.mjs`, which passed 506/506).

BLOCKERS: 2
