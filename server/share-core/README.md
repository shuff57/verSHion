# share-core

The shared pool's payload rules, in one Rust crate used natively (tests, fixtures, the app's
fixture test) and as wasm by the Worker (`pkg/`, built by `scripts/build-wasm.sh`, which
`server/worker/wrangler*.toml` run as their `[build]` step; the Worker refuses to load a build
with test-only exports, `server/worker/src/share-core-guard.ts`).

The module docs are the normative text; this file is the map, and the list of spec text that
still has to move into `docs/shared-pool.md` (which this crate's builder may not edit).

| module | what it decides |
|---|---|
| `json` | strict, schema-directed reading: duplicate keys where read, unknown keys skipped unread, arrays capped while reading, a value / object / allocation budget, depth |
| `text` | the character table (Block / Line classes), line-break and BOM mapping, Word Symbol-font mapping, tag sequences, variation selectors, the mark-run cap, NFC, the raw-size cap |
| `validate` | well-formedness, key completeness, the keyless rule, publish-only checks (`validate_for_publish`) |
| `labels` | letter labels (finding, Strip / Keep), sibling references (~25 languages, table-driven) |
| `format` | keys carried by formatting: the served text, the closed set of marker features, risks |
| `strip` | the stripped (served) form, order acknowledgements, the author-only served orders |
| `hash` | the canonical writer, `match_hash`, `match_hash_with`, `full_hash` |
| `import` | the import contract: `localize`, `right_column`, `rekey` |
| `limits` | every limit, and which ones a caller may only lower (`Limits::sane`) |
| `wasm` | the byte-oriented API the Worker calls |

## wasm API

Every payload function takes the payload as **bytes** (`Uint8Array`; anything else is
`expected_bytes`) and an options JSON string; it returns a JSON string with `ok`.

| export | returns |
|---|---|
| `validate(payload, options)` | `{ok, payload, dropped}` or `{ok:false, errors}` |
| `validate_for_publish(payload, options)` | the same, plus key completeness, the keyless rule, label decisions, formatting keys and locked leaks (`hide_key`), `placeholder_key` |
| `strip(payload, options)` | `validate_for_publish` with `hide_key`, then the served form: `{ok, payload, needs_order_ack, order_reasons, labels_removed, label_findings, images_kept, images_removed, figures_removed, dropped, match_hashes, stripped_hashes}`; `ok:false` with `order_ack_required` and `needs_order_ack` until `order_acks` covers every frozen order. **No author data** (round 6). |
| `publish_prepare(payload, options)` | the Worker's whole publish in one call (one read and validation): `{ok, payload, dropped, match_hashes, full_hashes? (with options.hash_key), stripped? (when options.hide_key: exactly strip's success response, `dropped` included)}`. A refusal of a payload that parsed: `{ok:false, errors, label_findings? (when a label decision is involved), formatting_findings, formatting_findings_total, order_preview}` (round 8: no second call); one that did not parse: `{ok:false, errors}`. Strip-time refusals (`order_ack_required`) as `strip` |
| `strip_author_data()` | the last successful `strip`'s `{ok, served_orders}`, once; then `no_author_data`. **Every other export clears it first** (round 8): take it right after `strip` / `publish_prepare` |
| `label_findings(payload, options)` | `{ok, label_findings (with previews, capped), label_findings_total, order_preview}` |
| `formatting_findings(payload, options)` | `{ok, formatting_findings: [{question, code, path, reason, ack_allowed, served}], formatting_findings_total, order_preview}` (labels as in `options.labels`, Keep where undecided) |
| `match_hash` | `{ok, hashes}` |
| `full_hash` | `{ok, hashes}`: HMAC-SHA256 keyed with `options.hash_key` (hex, 32+ bytes); `hash_key_required` without it |
| `validate_meta`, `schema_versions`, `argon2id`, `memory_bytes` | unchanged |

Options: `limits` (memory- and work-sizing limits can only be lowered), `figures`, `image_ok`,
`hide_key`, `labels` (`[{question, part?, choice: "strip"|"keep"}]`), `order_acks`
(`[{question, part?}]`), `ack_formatting` (`[question index]`: low-risk formatting findings
are content), `drop_client_provenance` (`strip`: remove client-sent `origin`), `hash_key`
(`full_hash`). `strip_with_seed` exists only in builds with the `test-seed` feature.

`match_hashes` at the top level of `publish_prepare` are plain `match_hash` of the author's body
(no label decisions applied). With `hide_key`, **the Worker must store `stripped.match_hashes`**:
those are `match_hash_with` the author's label decisions, i.e. the hash of what students are
served; they differ from the top-level ones whenever a label was stripped. `stripped.stripped_hashes`
are `served_sha256` of the served bodies.

**What students will see** (the mandatory confirmation): render `stripped.payload` of a successful
`publish_prepare` with `hide_key`. It is exactly the served form: for each question `stem`, `topics`,
the Mc `answers[].text` in served order (all `correct: false`), Matching `pairs[].left` in served
order and the word bank `distractors`, Multipart part stems and choices, the bank `name` and
`details.description`; every text already in the served normalisation. There is no separate
`student_view` field: nothing else reaches students.

Every `key_in_formatting` / `key_in_stem` error carries `hint` (English: what the author can do).
`served_orders[k]` of a Matching question has `left_order` too (round 8: the left items are served
shuffled unless `keep_order`): served left `i` is the author's `pairs[left_order[i]]`.

`order_preview` (in `label_findings`, `formatting_findings` and `publish_prepare` refusals): per Mc question or part and
Matching bank, `{question, part?, matching?, movable: [indices], frozen?: reason,
frozen_if_keep?}`, so the app can show what will move **before** publishing. To keep an order
whatever this says, the app sets `keep_order: true` on the question (schema v2).

`strip` already runs `validate_for_publish`: a caller that strips needs no separate
`validate_for_publish` call for the same bytes and options.

**`strip_author_data` must reach the publishing author and nobody else** (their publish
response): never a listing, a non-author's response, a log, an audit record or a report. With
the author's full question, `served_orders[k].order` maps each served choice back to its
`correct` flag; for a Matching bank, `order[k]` indexes the author's right column, so an entry
`order[k] < pairs` **is** the pairing. Natively, `strip::AuthorOnly` is not `Serialize` and its
`Debug` redacts; `author_json()` is the only way out. Errors that name the key's choice
(`key_in_formatting`, `locked_key`) carry `author_only: true`: show them to the author only.

`origin` and `published` are the server's: the Worker must overwrite them, or strip with
`drop_client_provenance: true`.

Native callers: `strip::strip` returns `needs_order_ack` and leaves enforcing acknowledgements to
the caller; the only publishing caller is the Worker through wasm `strip`, which enforces them.

## Costs (measured by the tests)

* Memory: every exported payload function peaks at most 4x the input bytes plus 1 MB natively
  (`tests/memory.rs`, `field_shapes_have_a_per_byte_cost_bound`), and its wasm linear memory grows
  by at most 7x the input plus 2 MB and under 40 MB on the field shapes (the B1 lead shape of round 16 grows by about 20 MB, 5x) (`tests/node/check.mjs`). One key-scan-heavy shape is above
  both per-byte bounds: the 4 MB Blank bank with 170,000 distinct variants (170 x 50 x 20) peaks at
  about 27 MB natively (7.4x) and grows wasm memory by 36.3 MB for the critic's 4,172,388-byte
  payload (8.7x), under the 40 MB ceiling (`tests/memory.rs::blank_bank_with_170000_variants_stays_under_the_ceiling`
  and the `blank170` mode of `tests/node/mem.mjs`). A text field longer
  than 4x its limit is refused before any character of it is read; character checks are one
  allocation-free pass with error offsets computed in the same pass.
* Time: the label, sibling-reference and formatting checks are linear in the text
  (`tests/round5.rs`, `formatting_checks_are_linear`); every exported call on the hostile and
  field shapes stays under 3 s in Node (`tests/node/check.mjs`).

## Spec text to move into docs/shared-pool.md

**Round 21 changes** (critic37a Moodle, critic37b robustness; `tests/round21.rs`; the last planned
round: the layouts of the key-lead channel that remain are documented limits):
* **Invisible characters in a key word** (critic37b B1, systemic). One U+200E (LRM), U+200F, U+061C
  (ALM), U+2061-2064 or U+180E inside a key word ("Answer: Pa<LRM>ris") defeated every key scan,
  while the student sees "Paris". `fold_key` (the copy every key, hint, pair and list scan reads;
  the served text is not touched) now drops every Default_Ignorable_Code_Point: the table
  `format::DEFAULT_IGNORABLE` (U+00AD, 034F, 061C, 115F-1160, 17B4-17B5, 180B-180F, 200B-200F,
  202A-202E, 2060-206F, 3164, FE00-FE0F, FEFF, FFA0, FFF0-FFF8, 1BCA0-1BCA3, 1D173-1D17A,
  E0000-E0FFF). A test walks every code point of general category Cf and fails for one that is in
  neither that table nor `VISIBLE_FORMAT` (number signs and the like, which show something), so a
  class added by a Unicode update is caught; a second test puts every point of the table at four
  places inside "Paris" in an answer line; a third puts a sample of each range inside the key of a
  fixed-order Mc, True/False, Blank, a bank key list, a listed pair and a rights list, and requires
  a refusal; honest emoji with ZWJ and variation selectors publish and their stems are served
  unchanged.
* **`Ans.` and a lead at the end of a line** (critic37b B2, critic37a B37a-1). "Ans. 1 B", "Ans. (1)
  True", "Ans.\n1 B" (the full stop is skipped for the one lead it may end) and a lead that ends a
  line AFTER PROSE with the numbered row on the next line ("Capital of France? Key:\n1. Paris",
  "Mars is a planet. Answer:\n1 True", "Hardest? Answer:\n1. B") are read like a lead that opens
  its line: the row's exact key is `answer_line`, the key among the first words of a longer row is
  `key_heading_row` (low risk, ackable). Prose that merely ends in "answer:" or "key:" and is
  followed by the student's tasks ("Write your answer:\n1. Explain why ...") publishes, or at worst
  is low risk; the dichotomous key and "Key terms/points/stage/vocabulary" are unchanged.
* **Weaker lead words** ("Keys", "Marking scheme", "Marking guide", "Rubric", "Memo", "Marks") are
  read for a numbered row only, as LOW RISK (`key_heading_row`, acknowledged with
  `ack_formatting`) even when the row is exactly the key: "Marks: 3 of 4" and "Memo: bring a
  ruler" are everyday text, so they are never a hard refusal. They are not leads for any other
  reading. Documented limit (critics 38a/38b, found after this round): in ANOTHER question's stem
  or a description of a bank these six weak leads are NOT refused (a bank scan has no
  acknowledgement, and a hard refusal of "Marks: ..." would refuse honest banks), so "Keys:\n1. Paris"
  there publishes; the same text in the question's own stem is a low-risk acknowledgement. An earlier
  version of this paragraph said they were `answer_line` in a bank; that was wrong.
* Documented, not fixed: the contrived honest shape that gets `scan_budget` (100 Matching questions
  whose left items quote 10-word phrases of one shared 8 KB passage that is also the stem of
  150 questions: the pair walks of one text pass 50,000 steps; split the passage or shorten the
  items); exotic row labels after a lead (`[1]`, `#1`, `No. 1`, `Q.1`, `1.-`, capital `A.`, `i.`,
  `Question 1:`); decorated leads (`**Key:** 1. Paris`, `<b>Key:</b>`, backticks); look-alike leads
  of another script; lead words not in the tables ("Answer sheet", "Right answer", "Teacher key");
  a key of one alphanumeric character after a symbol in the key-list scan ("°C", "∠A"); the
  remaining layouts of the key-lead channel.

**Round 20 changes** (critic36a Moodle, critic36b robustness; `tests/round20.rs`):
* **A numbered row after a lead on the same line** (critic36b B1; round 19 read only a heading alone
  on its line). "Key: 1. Paris", "Correct: 1) Paris", "Answers 1. Paris", "Answer key 1. Paris",
  "Mark scheme 1 Paris", "Model answer 1. Paris", "Solution: 1. B" (fixed order), "Answer:\n1 True",
  "Key - 1. Paris", "Q1: Paris", "(1) Paris" are read for the leads key, correct, kunci, answer(s),
  ans, solution(s), sol, mark scheme, model answer, acceptable answer(s), in stems, part stems and
  other questions' stems, after the lead anywhere on the line. The row's value that IS the key
  (the whole rest of the line: key text, letter, True/False value) is `answer_line` (high risk).
  The key among the first words of a longer row is `key_heading_row` (low risk, ackable), only
  where the lead opens its line (at most one qualifier word before it). A label that is only a
  number and a space followed by another number ("Answer: 2 3") is data and is not read as a
  row. Unchanged: "Key terms", "Key points", "Key stage", "Key vocabulary" and dichotomous keys
  ("1a Has six legs", every position of the correct choice) publish; the 400-form matrix of the
  test (20 leads x 10 row forms x line start or mid line) refuses all.
* **Pair-mark characters inside pair items** (critic36a B36a-2, critic36b B2: fail open). The
  bank text is read with every `- > : = ,` made a mark, but a pair item holding one of them
  ("Jean-Paul Sartre", "Washington, D.C.", "Genesis 1:1", "1914-1918", "Ratio 1:2", "a > b") kept
  it raw, so the entry was never found. Now the entry is made by the SAME normalisation
  (`pair_entry`: the folded left, " - ", the folded right, through `pair_mark_form`) in the stem
  rule and the bank pair scan (the key-list scan matches the raw key text in the raw text and was
  not affected). A right that cannot be one item of a word-bank
  list ("Paris, France", "Genesis 1:1", "Mr. Brown", "(a) first", more than six words) is swapped
  for a plain one-word name in the text and in the keys (`format::Neutral`: the longest text at
  each word boundary; positions of a swapped text are mapped back), in the bank-wide rights-order
  scan and in the single-question `bank_order`, so a list of them is a list of items. The swap is
  charged to the work budget; over budget it is `scan_budget`.
* **The pair scan starts at any character that is not a space** (critic36a B36a-3). A left item
  may open with a symbol ("√9", "°C", "$5", "(a)", "#include", "+", quotes, "/etc", "~"); the
  walk now starts at every non-space character, as the stem rule's substring search does. Each
  text has its own limit of 50,000 walk steps over all its starts, beyond which the text is
  refused with `scan_budget` (a left item repeated to the field limit is read from every start;
  never read in part). The cost is bounded: `round10::repeated_tokens_stay_linear` passes.
* **Label and formatting paths** (critic36a B36a-1: 1000 questions x 26 choices "A. X - X - ...",
  3.8 MB, `publish_prepare` 11.5 s, `formatting_findings` 18.2 s, `label_findings` 8.6 s, with Keep
  decisions 27.8 s; the lowercase control 2.9 s). Profiled: the cost was `labels::sibling_ref`
  (called per answer from the order plan, outside any memo) at about 10 microseconds a token:
  every token ran two linear scans of the 300-word joiner table and a capital letter was a label
  token that read the Greek, Cyrillic, Hebrew and kana tables. Fixes: (1) the joiner and
  qualifier tables are hash sets and a lone ASCII letter or digit is a label with no table read;
  (2) `sibling_ref` is a function of the first 200 bytes of its text, so inside a work scope its
  answer is kept per distinct text (memory bounded by the payload, forgotten with the outermost
  scope); (3) the refusal's own findings and order preview (`wasm::refusal`) now run inside a
  work scope, which they did not before. 9 shapes ("A.", "A)", "(A)", "a.", "A -", "1.", unique
  texts, Cyrillic, numbers) are linear at 4x the size; each is now about 0.2 s natively for 1.1 MB.
* Documented limits (not fixed): wrapper variants of the rights list in bank texts (quoted,
  parenthesised, bold, tab-separated, arrow chains, " or ", "and" before the last item, numbered or
  lettered lines, reversed); decorated headings ("**Answer key**", "# Answer key", "<b>Answer
  key</b>", "Answer key.", "Answer-key", "Scoring key", "Rubric", "Memo", "Marking guide");
  Matching separators not read (`~ / | tab`, leaders, "matches", "is", `<->`, `<=>`, `≈`, `»`);
  honest numeric tables in numeric-answer banks (an S1-style id works around it); feedback phrases
  after the correct choice ("Spot on"); GIFT True/False feedback in English only; a rights list
  repeated three times with five or more questions sharing the column; the swap of list items
  applies to rights and key texts of word banks, not to a left item that is a separator character
  alone.

**Round 19 changes** (critic35a Moodle, critic35b robustness; `tests/round19.rs`):
* **The Matching rights-order scan is linear and budgeted** (critic35a B1: 480 fixed-order Matching
  questions sharing "Yes / No / Maybe" and 480 stems of "Yes, No, Maybe, ..." took 24.8 s min of 3
  on 4 MB, `ok:true`). Round 18 searched a per-question list for each list item. Now the rights
  columns are grouped (identical columns are one group: 480 questions sharing a column are one),
  each right indexes the (group, position) pairs holding it, the state of a group is a dense slot
  (reset through a touched list) and its longest increasing run is kept online (patience tails).
  The pairs scan counts per question in a dense array the same way. Every step is charged to the
  publish's work budget and to the text's own 100,000-step budget (`TEXT_STEPS`): a text over
  either is refused with `scan_budget` (never skipped), e.g. 480 distinct columns that all hold
  "Yes" read against a stem listing "Yes" 1,500 times. The critic's shape now takes about 0.8 s
  natively, the same as the same bytes with `keep_order` off. A bank of one question is no longer
  skipped by the bank-text pair and order scans (only the key-list automaton needs two).
* **GIFT True/False with feedback** (critic35a B2): `{TRUE#well done}`, `{FALSE#Not quite}`,
  `{T#good}`, `{F#no}`, `{TRUE#if wrong#if right}`, `{ T # fb }`, `{TRUE\n#fb}`: the value, spaces,
  then "#", with a closing brace later in the text, is `gift` like the bare form, in stems and
  part stems. The choice-feedback forms (`{=x#fb ~y#fb}`, `{#4}`, the Cloze types, `{=a -> b}`) were
  already read and are in the test. Limit: a musical or set-notation brace that opens with a
  lone T or F and "#" ("{F# minor}") reads as GIFT: reword it.
* **Single-letter label shape** (critic35b B1: 950 questions x 26 choices of "x - x - ..." took
  9.1-10.3 s in `publish_prepare` once the author's label decisions were supplied, against 2.3 s for
  "xy - "). Profiled: no quadratic loop; `labels::refers_to_siblings` ran twelve times per
  question on the same choices (the label findings, the order plan, each key scan), and a text of
  repeated one-letter labels is the most expensive text it reads (every token was a label and a
  joiner, each re-derived in every loop). Fixes: (1) inside a publish's work scope the answer of
  `refers_to_siblings` is kept under the SHA-256 of the exact choices and lock flags (exact: no two
  different questions share an entry; the memory is 32 bytes a question; the entries go with the
  outermost scope); `label_findings` and `formatting_findings` open a scope for the call; (2)
  `sibling_ref` computes each token's bare form, joiner flag and label flag once, and
  `sib_tokens` searches a word only for a splitting character; (3) every `sibling_ref` reading
  is charged to the work budget (a quarter step a byte), so a budget spent there turns the next
  key scan into `scan_budget`. Natively: `publish_prepare` 5.5 s to 0.7 s, `label_findings` 2.3 s
  to 0.17 s; the control shape went from 1.4 s to 0.75 s.
* **A numbered row under a `Key` / `Answer key` heading** (critic35b B2): "Answer key\n1. Paris",
  "Key\n1 Quartz", "Key:\n1) Paris", "Key\n(1) Paris", "Key\nQ1: Paris", "Key\na) Paris",
  "Key\n• Paris", "Key\nQ1 - Paris", "Answer key\n1. B" (fixed order), True/False rows, and another
  question's key in a bank. The heading is `key`, `correct` or `kunci` alone on its line (at most
  one qualifier word before it: answer, answers, ans, correct, solution(s), marking, model,
  teacher; nothing but spaces and ":" after it); the row opens with a label (a bullet, "1.", "1)",
  "1 -", "1 ", "(1)", "Q1:", "a)"). The row's value that IS the key (the whole rest of the line: the
  key text, its letter, its True/False value) is `answer_line`, high risk. The key text among the
  first words of a longer row is `key_heading_row`, **low risk, acknowledged with
  `ack_formatting`** (an honest legend row may hold the word); in a bank's cross-question scan,
  which has no acknowledgement, both are `answer_line`. A letter glued to its number ("1a  Has six
  legs", the dichotomous key) is no row label, so round 18's worksheet still publishes for every
  position of the correct choice; "Key terms", "Key points", "Key stage" and "Use the key" are not
  headings (the lead is not alone on its line). Pre-existing, documented: a `Key` heading whose
  first row is "1. ..." refuses when the question's first choice is correct (the number reads
  as "choice 1"; the "Legend" heading, or numbering the rows from 2, avoids it).
* Documented limits (not fixed): with a fixed order "Answer: B because ..." and "Answer: B and D"
  are not refused (a letter must stand alone; "Answer: A tree ..." stays honest). Lead words
  not in the table are not leads ("Information for graders:", "Grader info:", "Teacher note:",
  "Scoring guide:", "Sample response:"). Matching separators the pair rule does not read: pipe,
  tab, dotted leaders, "matches", reversed order, parallel lists, tables; rights in pair order on
  separate numbered or lettered lines. Honest numeric data tables in a numeric-answer bank are
  refused about half the time (a value right after its own question's number fires on two rows
  by chance; an `S1`-style id works around it). A description listing answer values is refused
  often, with a workable path (remove the numbers or the list, or reorder it). `details.license_url`
  is served and not read for keys.

**Round 18 changes** (critic34a Moodle, critic34b robustness; `tests/round18.rs`):
* **The bank prefilter folds whitespace** (critic34b B1). `format::may_have_answer_line` compared
  the raw text with the spaced leads, while `stem_key` reads the served, folded text where a tab or
  a run of spaces is one space: "mark  scheme: <key>" or "mark\tscheme: <key>" in another
  question's stem, choice, left, right, distractor, part stem or part choice skipped the whole
  scan (42 of 48 lead x separator x field combinations leaked). Now any text with a tab or two
  consecutive spaces takes the full scan (conservative: it can only add reading, never skip).
  Line breaks were never joined into a lead by the scanner, so they are unchanged. The matrix is
  `b1_whitespace_in_a_lead_does_not_disable_the_bank_scan` (2 leads x 4 separators x 7 fields).
* **Couplet labels are not a numbered key** (critic34b B2). In the numbered-letter rule a letter
  glued to its number and followed by a word ("1a  Has six legs ... Spider") is a dichotomous
  key's couplet label, not "question 1 = A", so the canonical `Key` worksheet publishes whatever
  position the correct choice holds. Separated or run forms still match ("1 A 2 B", "1. A",
  "1) A", "1 - A", "1B 2T", "1C 2B 3D", "[Key: 4B]"); two or more numbered values are a key as
  before. Documented edge: a single glued letter followed directly by a word ("1B then ...")
  is read as a label.
* **Multipart part answers in the bank scans** (critic34a B1 and critic34b's named gap). The
  key-list scan reads every part's correct Mc texts (with and without labels), Short samples
  and True/False values, and each part's correct letters and numbers when students see its order
  (the part's own label decision is used, `decision(question, part)`). `bank_order_errors` has one
  slot per part, in part order (a Tf part has no shareable text, so it is a slot with no item).
  "1 Quartz 2 Basalt 3 Mohs" over three Multipart questions, "1 T 2 F 3 T" over Tf parts and
  "1 C 2 B 3 D" over fixed-order Mc parts (before and after the order acknowledgement) are refused.
* **Matching pairings and rights in bank texts** (critic34a B2 and the named gap). Each Matching
  pair is a `left<mark>right` string; two pairs of one question in a bank text, in the same
  separator forms as the stem rule (dash, arrow, colon, "=", comma; the GIFT "=Bauxite -> Aluminium"
  block) are `pairs_listed`. A Matching question with `keep_order` and three or more pairs lists
  its rights in pair order: a bank text holding three or more of them in that order (60% of those
  found) is `word_bank_order`. Paired rights are also values of the key-list scan, and slots of
  the order scan when `keep_order` (a shuffled left column makes the rights order name nothing).
  All four are high risk, never ackable, author only, and fail closed (`scan_budget`).
* Documented limits: single-letter keys stay out of the bank key-list scan (a deliberate
  ambiguity guard); a pairing given only by the lefts in shuffled order has no list to read;
  Matching pairs written in a table or QTI stay as in round 13.

**Round 17 changes** (critic31a Moodle, critic31b robustness):
* The bank key scan reads **every entry**: the True/False value lists have no 64-question cap
  (`strip.rs`, critic31a B1); a Short sample of any length is read (80 and 200 character caps
  removed); every key is folded once into a sorted arena. The 4,096-event cap of the answer-key
  scan (both critics B2) became a one-pass sequence automaton: `1 C 2 B 3 D`, `1T 2F 3T 4F`,
  `Q1T Q2F`, a two-row grid, a pipe table, `Answers: 1 C, 2 B, 3 D`, `C, B, D`, `Q1 C · Q2 B ·
  Q3 D`, German and CJK labels are all read, whatever the number of words before them.
* Every **served field** of a bank is read against every question's key (critic31a B3): stems,
  part stems, main stems, Matching left and right items, distractors, topics, choices, the bank's
  name, description, author and licence name. A choice is read even after a lead with no separator
  ("Answer Mars"). Another question's key standing in a topic as a token ("Unit 4: Quartz", four
  characters or more) is `topic_is_key`.
* A key list in a bank's name, description or any stem is refused **before** any order
  acknowledgement when the questions' order students see makes the letters name the key
  (`keep_order`, kept labels; critic31a B4). Without a fixed order the letters name nothing.
* **No silent caps** (critic31b B2). Each cap that could hide a key is gone or proves unreachable
  by the enforced limits; `tests/round17.rs::b2_caps_cannot_hide_a_key` records the arithmetic:

  | cap | now |
  |---|---|
  | part cross-key candidates (4,096) | removed: at most 50 parts x (26 x 2 forms + 1 sample) = 2,650 |
  | part True/False references (4,096) | removed: an 8 KB stem holds at most 2,048 of "(a)t" |
  | `anki_cloze` "{{" occurrences (4,096) | removed: an 8 KB field holds at most 4,096, all of them read |
  | word index places per word (64) | removed: every place kept (a sorted vector, not a hash map) |
  | `holds_token` occurrences (256) | removed: linear until the first whole token |
  | the answer-key scan's event list (4,096) | gone with the automaton; the work budget refuses |
  | "wrong choice" lists (64) | removed: at most 26 choices / 50 pairs |
* The 200-character window after an answer lead limits **where a key starts, not where it ends**
  (critic31b B1): the line is read to the window plus the longest key. A 228-character key after
  "Answer: probably" and "Olympus Mons" behind 187-195 characters of filler are refused (the key
  at 205 is outside the documented window).
* Repeated leads and labels are skipped cheaply (critic31b B3/B4): the lead window is a slice of
  the folded text (no copy), word starts are iterated rather than characters (bytes for ASCII),
  a window inside one alnum run with no long key is not read, a lead immediately followed by
  another lead checks only its own unit, a covered range of word starts is not walked twice, the
  no-space leads are found in one pass (13 leads x the text before), and Ethiopic folds at one push
  per character (no NFKD table). The lead charge is the work done, not a fraction, so a repeated
  lead reaches the work budget after a bounded reading. Measured in wasm (process CPU, best of 3,
  final build): the critic's Amharic-lead Matching bank 4 MB 1.4-1.8 s (was 8.7 s), the `2)`
  Multipart bank 4 MB 1.7-2.0 s (was 11.4 s), the Malayalam verdict-word bank 3.98 MB 2.5-3.0 s
  (was 5.5 s); the round-10 amplification sweep passes.
* The **work budget** counts reading, not lead occurrences (critic31a B5): a 63-row data table
  ("Key Stage", a caretaker's key log, field notes, a dichotomous key) is not refused; only a text
  that really reads more than 100,000 steps is `scan_budget` (fail closed).
* **Memory** (critic31a B6): the trie of keys and the pair trie became sorted arenas (`format::Lex`,
  about 12 bytes per text beyond its bytes), with a fixed 64 KB direct-mapped transition cache for
  the walk (integer keys: the shared prefix of thousands of numbered keys is walked once, not per
  start). The word index and the attacker-keyed maps of the bank scans are sorted vectors or
  ordered maps, so no attacker-chosen text reaches a hasher (the old `PairHasher` is gone). The
  4 MB Blank bank (170 x 50 x 20 distinct variants) that took 166 MB in wasm grows 36.3 MB for
  the critic's 4,172,388-byte payload (under the 40 MB ceiling; the `blank170` node measurement is
  in the Costs section); natively it is above the 4x+1 MB bound that the other field shapes meet
  (about 27 MB for 3.7 MB, measured by
  `tests/memory.rs::blank_bank_with_170000_variants_stays_under_the_ceiling`).
* Honest corpus (critic31a A6): 627 items, 0 hard refusals, 5 acknowledgements (0.8%, one
  parenthetical outlier in five paste styles; `tests/round17.rs::a6_honest_corpus`). The critic's
  bank set contains the same Quechua item, now clean. The Quechua "Rupay / sol" was refused because
  a bracket-only word ("sol") after a "/" counted as a verdict; a "/" or "|" is a gloss's separator,
  so a bracket-only word there is content. Answer leads added: Punjabi ਉੱਤਰ, Sinhala පිළිතුර,
  Burmese အဖြေ, Lao ຄຳຕອບ, Pashto ځواب, Somali "jawaab", Māori "whakautu", Malagasy "valiny",
  Gaelic "freagairt", Yiddish ענטפֿער, Armenian պատասխան, Georgian პასუხი, Tajik ҷавоб, Albanian
  "përgjigja" (the scripts without spaces also in the no-space lead list). The critic's marker
  corpus adds Punjabi ਜਵਾਬ, Kurdish "bersiv", Quechua "kutichiy", Samoan "tali", Javanese
  "wangsulan", Kinyarwanda "igisubizo", Shona "mhinduro", Luxembourgish "äntwert", Breton
  "respont", Kyrgyz жооп; the True/False values "patiesi", "үнэн", "fìor" and a trailing bracket
  stating the value ("(This statement is true.)"); and the exam.cls LaTeX forms `\answerline[...]`,
  `\begin{solutionordottedlines}` and `\ifprintanswers`. A "Q1: key" value in a left item is
  `bank_key_list`.
* Every formatting finding carries `author_only: true` (critic31b): it names the key's choice.
  A formatting finding is not a listing's or another author's response.
* Limits (documented, unchanged): the 200-character window (a key starting past it is not read);
  a key of three characters or fewer after a lead counts only as the first word (so a stop word in
  the sentence after a pasted "Save Answer" button is no answer line); the label helpers'
  160/200-byte reads decide sibling references (order freezes), not key hints;
  the 4 MB Blank bank's 7.4x native peak. The critic31a marker-corpus misses that remain are
  variants of documented mechanisms: a worksheet with one True/False value circled ("(T)  F
  statement"), a list of partial key words ("gills, cuckoo" for keys "Gills", "Common cuckoo"), a
  key given as data in another question's stem, and OCR'd marks; round 15's documented limits
  cover them. The LaTeX exam.cls forms (`\answerline[...]`, `solutionordottedlines`,
  `\ifprintanswers`), the missing lead languages and the True/False endings added in this round are
  now read.

**Round 16 changes** (critic28b):
* Work budget: the lead scan walks a trie of the keys from each word start (each start once per
  text, at most to the window's end), counting steps; past 100,000 steps in one text or 20 M in
  one publish phase (`format::WorkScope`, opened by `publish_errors`, `strip`, the findings), the
  text is refused, `scan_budget`, high (hint: shorten or split). critic28b's 99 key lengths x 880
  stems took 31.8 s; now refused in 1.3 s (wasm). The bank answer-key scan walks a byte trie the
  same way, budgeted, and reads True/False symbols once per text (a search per question was
  quadratic).
* Lists in order (bank and Multipart lists, Blank word banks): a longest non-decreasing chain of
  (list position, question) pairs; a text two questions share is one item for both (a tie), a
  True/False word is not; four or more in order and at least 60% of those found: high (three in a
  Blank bank); repeated values ("Results: True, False, False, True") follow the parts' sequence. A
  value of more than four questions ("alpha" accepted everywhere) says nothing of the order; in
  the answer-key scan a value several questions share counts only next to its question number.
* No caps in the word-bank scanner: banks, sentences, line lengths, bracket groups, bulleted runs
  are all read, linear in the text; a list keeps its good items (three at least, half of them).
* Topics: every key is read against the set of the bank's topics (no key cap); the bank's name and
  description fail closed (`scan_budget`) when the bank's keys pass 65,536 and the text has an
  answer line.
* Served form: a topic is stored in its served form (invisible joiners dropped) and deduplicated,
  so the served payload validates to itself; the fuzz test checks that for the full and the
  stripped payload of every published case.
* Remaining limits removed: pasted choice lists past 30 lines, Matching stem lists past 128 items
  and numbers past 99, the twelve-word window after a lead (any word start in the window, and a
  key may start after the first word), part references past "z" (numbers 1-50 and letters, read
  once per part stem).
* Worker contract additions: `reinit()` zeroes the old instance's memory before dropping it.
  `health()` cannot see a leak from exceptions thrown inside imported JS functions (the stack
  pointer); the contract stays "reinit after ANY exception from ANY export", which covers it.

**Round 15 changes** (critic28a):
* Anki cloze (`{{c1::x}}`, `{{c1::x::hint}}`) and the field references `{{type:...}}`,
  `{{cloze:...}}` are answer syntax in every served text field: stems, part stems, choices, left
  and right items, distractors, topics, the id, the bank's name, description, author and licence
  name (`cloze`, high risk, keyless questions too). Client note: the app's Anki import
  (`src/formats.rs`) should turn `{{cN::x}}` into a blank with `x` accepted; share-core refuses it
  either way.
* The bank's `details.author` and `details.license_name` are served and are now read like the
  description (they were served unscanned).
* Answer lines need structure: after a lead with no separator, a True/False word counts only as
  the question's own value, at the end of its line, and not after an article ("Chọn đáp án
  đúng.", "Pick the answer that is true.", "Elige la respuesta verdadera." are instructions); a
  copula in the first sentence counts only before a colon ("The correct answer is: X").
* An answer key of any layout in the bank's name, description or any stem (`bank_key_list`, high
  risk): three key values of different questions in question order within 200 characters, each
  at its own place, or two each next to its own question's number ("2 T, 5 F", "Q1 (X) Q2 (Y)",
  "Key — 1 X; 2 Y", a two-row grid, a pipe table, "解答 1 ... 2 ○"); a lone "true"/"false"/"T"
  counts only next to its number.
* Kahoot: the filled quartet ▲ ◆ ● ■, one each, is a label scheme, not marks; a filled shape on
  the correct choices among outlined ones (△ ◇ ○ □) is a mark.
* HTML whose tags alone differ (the correct choices' formatting elements and attributes unlike
  every wrong choice's, which are alike: `<strong>`, `style="font-weight:700"`, a colour) is
  `markup`, high risk, judged apart from the visible text; notation tags (`<sub>`, `<sup>`) and
  code (`<stdio.h>`, `<!-- -->`, a choice that is a tag) are content.
* Variants: leads "Hint", "Rationale", "Explanation", "Definition", "Keywords", "Tip", Zulu
  "Impendulo", Amharic "መልስ"; a key text ending the stem decorated (`**Mars**`, `{Mars}`,
  `‘Mars’`, `→ Mars`, `= Mars`; not notation, "f(2)"); a verdict before the content or a praise
  after it on the key choice ("Correct! Mars", "Correct Answer Mars", "Mars OK", "Mars Well
  done!"); verdict words of Basque, Yoruba, Zulu, Xhosa and Swahili; an Anki or spreadsheet row
  in a stem, the key after a tab; distractors tagged as unused ("(not used)", "– extra",
  "(distractor)") while no paired right is: `marked_distractors`, now high risk; a True/False
  symbol ending the statement ("... 👍").
* Limits (documented, not read): initial-letter hints, underscore counts, upside-down text,
  pairs in an HTML table, references to another question by number ("question 1 (carbon
  dioxide)"), a main stem giving a part's key as data, "Because Mars ...", "Remember: Mars ...",
  a teacher note in brackets, hidden spans, footnotes.

**Round 14 changes** (critic26a, critic26b):
* No trap on case-changing text: the copula tail searched a full lowercase copy and cut the
  original at its offsets ("İ" lowercases to three bytes, "ẞ" to two). ASCII lowercase keeps the
  offsets; no other offset found in a transformed copy indexes a different string (audited; the
  fuzz test now draws İ, ẞ, ǅ, ŉ, ß, ſ, K, Å, ΐ, ﬃ, Georgian Mtavruli, ı, ς and combining
  sequences, natively in `tests/fuzz13.rs` and in wasm in `tests/node/check.mjs`).
* **Worker contract** (a trap poisons a wasm instance; wasm cannot unwind): after ANY exception
  from ANY share-core export, call `reinit()` (exported by `pkg/share_core.js`, added by
  `scripts/build-wasm.sh`; the export guard fails without it) before the next call; it gives a
  fresh instance and memory from the same compiled module, `argon2id` included, so no separate
  argon2 module is needed. `health()` returns 1 on a working instance (it validates a payload and
  runs a tiny Argon2id); a throw or any other value means `reinit()`. Answer the request that
  threw with an error; do not retry it on the new instance.
* Every Multipart part is read against every other part's key (all 50; parts 16 and up were not),
  with every key text and both True/False values; the main stem too, and a main stem listing the
  parts' keys in part order ("Results: 3 m/s², True, ...") is refused like a bank list.
* Caps on a key-hint path (each fails closed or is unreachable, with a test in
  `tests/round14.rs` where it fails closed):

  | cap | now |
  |---|---|
  | parts read for cross-part keys (16) | removed: all 50 |
  | key texts per question (64 in `KeyInfo::folded`, 64 Blank answers) | removed: all (at most 26 x 2, 50 x 20, 50) |
  | answer leads read per text (32) | 64 readings, 256 lead words: more is `scan_budget`, high (fails closed) |
  | CJK lead occurrences (64 per lead) | counted in the 64 readings: fails closed |
  | Blank occurrences of answers (512) | 4,096 places through a word index: more is `scan_budget` (fails closed) |
  | bank keys (4,096) | 65,536 distinct: more makes any stem with an answer line `scan_budget` (fails closed) |
  | key length after a lead (60 characters, 4 words) | round 16: any word start within 200 characters, walked down a trie of the keys under the work budget (fails closed); a longer key is matched by its first 200 characters |
  | Matching left item (first 512 bytes) | removed: the whole item |
  | pasted choice lines (2 KB, skipped) | each line read in its first 2 KB (a choice's limit: its mark is there) |
  | part-key candidates (256) | 4,096: unreachable, by enforced limits: 50 parts x 26 choices x 2 forms + 50 samples = 2,650 |
  | word index places per word (64) | unbounded in part stems and Blank stems |
  | word banks (32 per text, 64 items) | round 16: **no cap** (the round-14 "unreachable" was wrong: banks, sentences and line lengths in a text are not limited by the validator); every region read once, linear in the text; an odd item is skipped, not the list |
  | stem lists for Matching pointers (256 lines, 64 items) | round 16: **no cap**; numbers to 999 |
  | label helpers (200 / 160 bytes) | unchanged: they decide sibling references (order freezes), not key hints |
* A bank's lists in question order (critic26a B1): a list of three items or more in the bank's
  name, description or any stem holding the questions' keys (Mc keys, Blank answers, short
  samples) with four or more in question order is `bank_word_bank_order`, high risk (served
  banks keep their order); three in a stem is `bank_word_bank_three`, low risk; out of order is
  allowed ("Key vocabulary: diffusion, osmosis, ...").
* Topics: against every Multipart part's key and True/False value, a key of four characters or
  more standing in the topic as a token ("Unit 4: Mars"), and every other question's key in a
  bank.
* Honest forms: both True/False values before the statement ("T   F   Larches ...", "V  F",
  "True / False") are a worksheet's answer blank, not a value; a sentence's own punctuation is no
  pairing token ("¿Qué hora es?" / "What time is it?"); a score on every choice, each its own
  ("Bogey (+1)", "Birdie (−1)"), is content; every value of a wrong choice's kind is compared
  ("- [ ] Elm (U)" has two boxes); choices in one same HTML element with the same opening tag are
  judged by their content (Atto's `<p dir="ltr">`); a bracket glued to a word is notation
  ("Poisson(λ)"); Turkish "ı" in Turkish text is no look-alike.
* Variants: LaTeX ticks and crosses (`\checkmark`, `\ding{51}`, `\cmark`; `\xmark`, `\ding{55}`)
  are marks; mark-scheme leads ("Mark scheme:", "Accept:", "Indicative content:", "Model
  answer:"); exam.cls `\begin{solution}`, `\fillin[...]`, `\answer{}` in stems.
* Cost: keys after a lead are looked up only at the keys' own lengths; Cyrillic, Arabic, Thai
  and Georgian letters are letters by range; critic26a's multi-script lead shapes went from 3.91 s
  to at most 1.43 s in wasm, and two of them are in the amplification test.
* Limits: a poet named in his own book's title next to the blank stays refused (it shows the
  answer); `[[n]]` gap markers and "the last one" with a fixed order; three in order on a bank's
  name or description (no question index acknowledges it); Wingdings degraded to "ü"/"P".

**Round 13 changes** (critic24a, critic24b):
* No input panics: a trailing key word after a digit ("1Answer", "100% correct") read the word
  before it out of range, a wasm trap on honest text. `tests/fuzz13.rs` runs 20,000 random texts
  (digits, punctuation, labels, answer words, marks, scripts) through every scanner and 4,000
  random questions and banks through every export with both label decisions.
* Cost: the lead scan looks a key up by each candidate end (one hash lookup per word end within
  60 characters), never scanning the keys that share a first character (4,096 keys starting "q"
  took 82 s); Multipart parts fold each key once, test the main stem once, and find keys through
  one word index per part stem (at most 256 candidate keys per question; 64.7 s before). Both
  shapes are in `tests/round13.rs` (two sizes, linear) and `tests/node/check.mjs` (4 MB, wasm).
* LaTeX quiz packages: the correct choices' leading command differing from the wrong ones'
  (exam `\CorrectChoice` / `\choice`, moodle.sty `\item*` / `\item`, AMC `\bonne` / `\mauvaise`,
  `\correctchoice` / `\wrongchoice`) is `latex_key`, high risk; such an environment pasted into a
  stem is `key_in_stem/latex_key`. Mathematics commands (`\frac`, `\sqrt`) are not read.
  Export attributes on the correct choices only (`fraction="100"`, `checked`, `class="correct"`)
  are `export_key`; a spreadsheet flag column glued on ("E7018,1" / "E6010,0") is a tail token.
* Matching: a left item that points to its right through a labelled list in the stem (on lines,
  two spaces apart, or "A. Tin B. Mercury" one space apart) by a letter or number in a blank, after
  a dash, colon or arrow, or in brackets ("Bauxite ___e___", "Bauxite — E", "HTTP (5)", "Bauxite
  (Answer: E)"), or whose blank holds its right ("Bauxite ___Aluminium___"), is
  `left_points_to_right`, high risk. A right that holds its own left ("Cotton" / "Seed hairs of
  the cotton plant", "Cat" / "Cat") is `names_left`, now low risk.
* The bank's name and description are read against every question's key (texts, True/False
  values, a choice letter or number, an answer key of letters "Key: ABT", "Answers: A B T") for
  answer lines; a copula then a colon ("The correct answer is: X") counts in the first sentence.
* Numbers read like letters: a choice of numbers and joining words ("1 and 3") names choices only
  if no such choice holds a number out of range; a number with a unit or noun ("400 to 700 nm",
  "pH 7 only") is content. Labels are cut first ("3. Both 1 and 2").
* A letter names a position only where students see one: the order does not move, or labels are
  kept. Labels stripped from shuffled choices name nothing; a stem's own label ("(a) How long...",
  "a) Which...") is its numbering, not a letter.
* Word banks: three answers or more in blank order are `word_bank_order` (high); two are
  `word_bank_pair` (low risk: one honest bank in two).
* Honest forms: ";" after every choice and "." after the last is a list's punctuation; marks after
  letters of scripts from Hebrew on are orthography (Sinhala, Khmer); a fraction before words is a
  quantity ("1/4 inch per foot").
* A topic with nothing visible is dropped at validation; a refusal whose errors are all for the
  author carries `author_only_refusal: true`.
* Limits: "Quiz 3 — key AB T" (no lead and colon); a left item numbered in pair order with no
  list in the stem ("HTTP (1)"); GIFT `::title::` and `####` feedback naming the key; QTI and
  tab-separated pairs in stems; "1. Modular" / "1" (a numbered left whose right is its number)
  stays low risk.

**Round 12 changes** (critic23a, critic23b):
* Another part's or question's key. Each Multipart part stem, and the main stem, is also read
  against every other part's key (correct texts with and without labels, short samples of up to
  40 characters, True/False values) for answer lines only: "Answer to (a): 4.0 m/s²", "ANSWER:
  False", "Respuesta (a): X", and a part reference with its value, "(a) is False", are high risk
  (`answer_line`). A part stem holding another part's key text as a whole token, a number ("use
  4.0 m/s²") or a word in a sentence that refers to that part ("from (a) (Mg)"), is low risk
  (`part_states_key`, acknowledged with `ack_formatting`: given data as often as a leak), unless
  the main stem holds it too or it is one of the part's own choices. In a bank, every stem is read
  against the keys of all questions (at most 4096 texts, looked up once for the bank) for answer
  lines: question 2 opening "ANSWER: <question 1's key>" is high risk. A number is named whole:
  "2" is not named by "2.5" or "2,5".
* Word banks are judged by order: students see the stem as written. A bank is three or more short
  items after a colon, on a line, in a bracket group, or on bulleted lines (or lines after a line
  ending with a colon). When two or more blanks' answers stand in it in the order of the blanks it
  is `word_bank_order` (high risk; the hint asks to reorder the bank); in any other order it is
  allowed and its items are not read as answers restated near a blank. One blank whose answer is
  the bank's first item is `word_bank_first` (low risk). With `keep_order`, a Matching stem that
  lists the rights in pair order is `word_bank_order` too.
* Labels: sibling references are read on the choices without their labels when the choices are
  labelled ("I. 2", "II. 3" are the numbers 2 and 3); a choice that is only a number and a full
  stop ("2.") is content, not a label.
* Locked choices: a choice that names two or more of the others by their text ("Both mitochondria
  and chloroplasts") is a reference, like "Both of the above"; locking it is no `locked_key`.
* Pasted forms: ExamView rows ("ANS: B  PTS: 1"), a numbered answer key ("1B 2T", "1-B, 2-True",
  "[Key: 4B]", also in the bank description), a verdict line of its own before the key choice
  ("Correct!\n...", "Correct Answer\n...", "Correcta\n..."), the key on its own line after an
  arrow or bullet ("→ Dovetail joint"), "Correct answer(s): 2" and "Answer to (a):" (a bracketed
  qualifier after a lead), "Correct Answer: f/1.8" (the whole key before any "A/B" reading), an
  ordinal with an order students see ("Answer: the second option"). Both True/False values on
  lines of their own at the end of a True/False stem ("Select one:\nTrue\nFalse", a pasted Moodle
  preview) is `choices_pasted`, low risk.
* `key_in_stem` reasons that can be acknowledged: `blank_hint`, `word_bank_first`,
  `choices_pasted`, `part_states_key` (`format::LOW_STEM_REASONS`); the rest are high risk.
* Cost: the served text is computed once per `stem_key` call (it was up to seven times in the
  True/False checks); blank ends are ASCII "___" runs, as validation counts them (compatibility
  underscores "＿", "﹍" no longer fold to "_"), found by binary search; lines of a pasted choice
  list longer than 2 KB are not read as choices; True/False edge tokens are folded only when short;
  a Matching left item is read for answer lines in its first 512 bytes; a run of bulleted lines is
  read once. The amplification test adds label-led lines of 4 KB and fifty blanks over a bulleted
  word bank.
* Cost of many small questions: printable ASCII skips the per-character normalisation work in
  validation (and NFC where the whole text is ASCII); other characters' results are cached per
  call stream; the formatting check reads equal choice texts once and skips the sibling-reference
  reading when every choice has the same feature kinds (no outlier is possible); Hangul, CJK and
  kana are letters by range, not by table search; wrong choices are folded only for stems of more
  than one line. `f_many_small` strip in wasm: 2.18 s (round 11) to 1.33 s at the same load. The
  amplification test's bound against plain text is 6x (plain ASCII got about twice as fast).

**Round 11 changes**:
* Cost: the Blank scanner counts list separators once per stem (a suffix array) and reads at most
  512 occurrences of the distinct accepted answers; key texts are looked up at word starts by
  their first character (no search per key and occurrence); "pairs listed in the stem" is one
  search per pair; `noun_before_letters` and `letter_list` read at most 200 / 160 bytes. The
  amplification test (`tests/round10.rs`) covers Blank (one blank, fifty blanks), key texts equal
  to the repeated token (Mc, Short), and stems with and without full stops and new lines.
* The generic net: a trailing annotation (after " - ", " – ", " — ", " <- ", " ← ", " = ", or in
  spaced brackets at the end, up to six words with a letter) or a copula tail ("... is correct",
  "... is the correct answer") on one choice: high risk when it holds an answer or verdict word
  (inflected forms included: "richtige", "corretta"), otherwise low risk (`s_annotation`).
  Consequently "Venus (hottest planet)" and "Mitosis (cell division)" on the key alone are low
  risk (acknowledged by the author).
* Scores: units in about 15 languages ("1 mark", "(1 punto)", "5 Punkte", "%100", "=100", "(full
  marks)", "(score: 1)", "(100 %)").
* Stems: key texts are compared with and without the choices' labels ("B. Nitrogen" is named
  "Nitrogen"); label letters of other alphabets name positions (Devanagari क ख, Bengali, Arabic
  abjad, Persian "الف", Hebrew, iroha and aiueo, Hangul, heavenly stems and Chinese numerals);
  a choice list pasted into the stem with exactly one line marked (✓, a leading "*", bold, a key
  word) is `marked_list`; with an order students see (keep_order, kept labels, sibling
  references), a decorated letter naming a correct position at the stem's start or end or on its
  own line ("[B]", "✓B", "→ B", "**B**", "{B}") is refused.
* True/False: a bracketed token of up to three capitals or one symbol ending the statement
  ("(V)", "(C)", "(Đ)", "(O)", "（√）", "(Да)", "(D)", "(T)."), a value alone on the last line, a
  value opening the statement with a delimiter ("TRUE: ...", "True - ...", "T. ..."), either
  polarity; symbols and the value letters of about 15 languages. A capital letter in brackets
  ending a True/False statement is therefore refused even when it is content ("temperature (T)").
* Blank: an accepted answer restated within 40 characters after a blank, or emphasised, quoted
  or ticked anywhere, is refused (a word bank of three or more items may hold it).
* Labels: initials ("B. F. Skinner", "A. P. J. Abdul Kalam") and genus abbreviations in no order
  ("E. coli", "S. aureus") are no labels (no decision needed); a choice's own label is no box.
  A symbol and its name ("✓ (check)", "Backspace (←)") is low risk.

**Round 10 changes**:
* Cost: every scanner is linear in its field and cheap per occurrence of a token it starts on.
  The CJK answer-lead scan reads at most 8 characters after a lead and skips leads glued to a word
  that no key, True/False value or "Option" word starts; at most 32 spaced and 64 CJK lead
  occurrences are read per text; key texts are folded once per question (not per stem, topic and
  part); keys longer than the 60 characters read after a lead are not searched there; the
  clause rule trims the stem once; Hangul stays composed and long compatibility expansions ("ﷺ")
  stay one character when folding. `tests/round10.rs` repeats every table token and syntax mark
  to every field's byte limit (6 question shapes, half and full size) and refuses quadratic growth
  or a cost above 4x plain text; the full sweep runs with `--ignored`. Measured in wasm: 4 MB
  bodies of the slowest scripts (Thai, Georgian, Arabic, Hangul) take at most about 2.3 s.
* Annotations on any content, numbers included ("6 (correct)", "6 - correct", "1945 (correct)",
  "$40 (correct)", "(wrong)" on every wrong year); words run on through combining marks
  ("उत्तर", "মঙ্গল"); more key/wrong words (esatta, gabarito, 옳음, incorreta, kunci, đáp án,
  الإجابة الصحيحة).
* Scores: bracketed points or a signed score next to any content ("6 (1 pt)", "6 +1"), after a
  delimiter ("Mars = 1 point", "Mars — 5 pts", "(worth 1 point)"); not an unspaced index ("xs[-1]").
* GIFT feedback glued with "#" ("English#Sorry…", "Mars#Good job"); a choice opening with an
  answer lead and a separator ("Correct answer: Mars", "正确答案:", "정답:", "उत्तर:"); words that are
  content as often as leads ("Solution:", "key:") count only with a qualifier.
* Stems: answer lines name **this question's** key (a correct choice's letter, its True/False
  value, the key text): "Answer A, B, C or D", "Answer true or false" are instructions; inline
  leads without a separator ("Answer B", "Key B", "ans Mars"); "(B)" ending an Mc stem; more leads
  (Clave, Rpta, Resp., Gabarito, Corrigé, Trả lời, Feedback). True/False: a value after the
  statement's last full stop ("TRUE", "[T]", "*T"), after an arrow or "=", opening a bracket that
  explains it ("(FALSE – it reflects light)"), a value letter opening the statement ("T The Sun…");
  part stems too. Matching: the stem listing two pairs or more ("Key: France–Paris, Japan–Tokyo"),
  a left item with "-> right". Topics: "Mars (correct)", "True" for a True question.
* Blank: a run of three underscores or more is one blank ("__________"); a word bank (a list of
  three or more items) may hold the key; the accepted word in brackets right after its blank ("I
  ___ (eat)") is `blank_hint`, low risk (acknowledged with `ack_formatting`).
* Not hard: unit symbols in brackets ("ohm (Ω)", "degree (°)", "pi (π)"), "O negative".
* Locks: "d) All of the above", "• None of the above" (label or bullet first) are references.
* Sibling references: "Both"/"Neither" followed by content ("Both make gametes") is content; a
  noun before numbers or letters ("Shade 14 only") is content; a bullet every choice starts with
  ("o I only") is no label.

**Documented limits** (round 12): a hint that gives the key's initial ("Hint: it starts with
“D”"); a Kahoot spreadsheet row pasted as text with a bare index ("... | 20 | 2"); a True/False
value in an answer line of another *question* of a bank (parts of one Multipart are read); a
word bank whose items are longer than six words or 80 bytes, or more than 64; the first 64
lines of a bulleted run; ordinals in English only ("the second option"); a part's key word
restated without a reference to the part (a statement, "Its acceleration at the top is zero").
Two blanks have a one-in-two chance of standing in bank order by accident: the author reorders.

**Documented limits** (round 10, in addition to round 9's): four-letter capitals, lowercase
against capitalised choices, brackets or quotes around the key alone (low risk, acknowledged),
"The answer is true." as the whole True/False stem, "The even numbers are 2 and 4.", markers on
the key that a scanner reads only beyond the first 32 answer leads of one text.

**Round 9 changes**:
* `served_text` is idempotent: NFC first, the invisible characters judged on the NFC text, NFC
  again, repeated until nothing changes (a removal can compose a new letter, "ا͏ٓ" → "آ", that
  changes a joiner's neighbours). So `match_hash` of the author's body and of the served form
  agree for every input (property test over joiners, marks, Arabic, Indic, Tibetan, Kaithi,
  Chakma, Hangul, emoji).
* Every served text goes through `served_text`: stems, part stems, topics, left items, the bank's
  name and description (besides choices and the word bank).
* `key_in_stem` reads `fold_key` of the served text: NFKC, combining marks dropped, lowercase,
  Cyrillic/Greek look-alikes as Latin, any space as a space, dashes as "-", arrows as ">".
  Leads in about 60 languages ("Answer", "Respuesta", "Réponse", "Ответ", "答案", "答", ...), up to
  two qualifier words before a colon ("Respuesta correcta:"), a qualifier before the lead
  ("Bonne réponse :"), a copula after a first sentence ("The answer is B"), CJK leads glued to a
  copula or bracket ("答案是B", "【答案】B"). The key after it: a choice letter standing alone
  ("B", "B)", "‘B’", "**B**", "Option B"), a True/False value, or a key text in its first four
  words (exactly the key after "Key:"/"Correct:"). GIFT braces after a word ("red{=Mars ~Venus}",
  "star{T}"), with spaces or new lines inside, `{#=3.14}`; LaTeX only after `\cmd`, `^`, `_`,
  `$`, `}`. Cloze in any case and spacing.
* Also `key_in_stem`: a clause of at most six words that starts or ends with a key text (or a
  choice letter, a True/False value) and holds an answer/verdict word ("(Nairobi is correct)",
  "(A is correct)"), unless it is the whole stem; a stem line that is a correct choice when no
  line is a wrong one; for True/False, lettered options ("A) True / B) False / ANSWER: A"), a
  value after a dash ("— True"), "(F)" after the statement's full stop, a check or cross mark at
  the end; for Blank, an accepted answer in brackets or after an arrow or colon ("____ (Au)",
  "→ Au"). A topic or the id that the scanner refuses, or a topic that is a correct choice, is
  high risk; the bank name and description are scanned for answer syntax (high risk).
* New marker features: score tokens ("(100%)", "[1 pt]", "+1"/"−1"; bare percent and fractions
  low risk, signed numbers in maths ignored); a whole-choice wrapper of the Unicode open/close or
  initial/final punctuation classes other than ( [ { and quotes ("【Nairobi】", "⟨Nairobi⟩"), or
  "|...|", "/.../"; "Key:" first (capitalised high, "key:" low); "KEY Nairobi". Best effort (see
  the limits): letters spaced one by one, an emoticon at an end, a rare under/through mark on one
  letter of an ASCII word, a trailing "…".
* Matching: a lone Latin letter or short Roman numeral at the end after a content word ("Peru
  b" / "Lima b", "Japan iii"), a letter of another script glued to a word ("Kenyaα"); a right
  item in a script without spaces that contains its left item ("ケニアの首都ナイロビ"); a left item
  that starts with its right item and a copula ("Nairobi is in Kenya"); every right item starting
  with its left item's first letter (three pairs or more, words of three letters or more: low).
* Not markers: "(right)" alone or "(right-hand)" (low), a Roman numeral after a name ("Vagus (X)",
  low), a grade star ("A*", "A**", low), "x"/"o" next to joiners only ("A and O"), German nouns
  for "error" only as a bracket's whole content.
* Letter references: a letter list after a noun ("Type A", "Types A and B equally", "Column A and
  C", "Hepatitis A and B") is content. "All / none of the above" in about 60 languages are
  references (a locked correct one is not a locked key).

**Documented limits** (owner decision, round 9): the formatting checks protect against markers a
careful-but-hurried author plausibly types: Moodle/GIFT/Aiken/Cloze/XML syntax, check/cross/star/
arrow/emoji marks, verdict and answer words in the languages of the tables, score tokens, wrapper
styles, answer lines and restatements in stems and topics, Matching tags and names, and the
structural outlier net. They do not promise to catch deliberate obfuscation: letters spaced,
dotted or hyphenated one by one; look-alike letters from scripts other than Cyrillic, Greek and
Armenian (and the blocks listed in round 8); a diacritic or "…" on the key only; exotic wrapper
brackets beyond the Unicode bracket classes; emoticons; length differences; words of languages
beyond the tables; plural or stem forms in Matching ("Frenchmen" / "(French)"). Several of these
are caught on a best-effort basis; misses there are not blockers.

**Round 8 changes**:
* `locked_key`, exactly: among the answer choices (not references such as "All/None/Both of the
  above", "A and B", nor "I don't know"-type choices), with at least one locked, one movable and
  one correct choice and a shuffle that happens, refused when the locked set or the movable set
  contains every correct choice and at most two others and is not every choice. So all wrongs
  locked around a multi-answer key, one correct among two or three movable choices, and the
  correct choice locked with up to two others are refused; a lock leaving three or more movable
  wrong choices beside the key is not.
* Marks of any kind: when every wrong choice carries some marker (check, cross, star, mark, emoji,
  key/wrong word, GIFT, a token box, an edge token, markup, a line mark, a "wrong" score
  `0 F N - ×`) and the correct one none, `marked_wrongs` (high); the correct choice alone with two
  or more kinds of mark, `marker_combination` (high); "marked at all" is one feature of the outlier
  net. In code/notation these are low risk.
* Look-alike letters inside a Latin word: letters of Cyrillic, Greek, Armenian, Georgian, Cherokee,
  Lisu, Coptic, Roman-numeral, letterlike, fullwidth, small-capital and Latin Extended-D blocks,
  high risk (`mixed_script`); dotless ı, long s, click letters, IPA, low risk.
* Matching tags are compared after folding superscript, subscript, circled, parenthesised,
  dingbat and fullwidth digits to ASCII and case; bracketed short words ("(FR)", "[red]"), a lone
  non-Latin letter ("α"), a 1-3 letter suffix after a hyphen ("-a") count; an exponent ("x²") and
  a Latin variable ("sin x") do not. One left and one right sharing a tag: `pairing_token` (high;
  low for numbers or single characters in maths). Right items starting or ending with the left's
  initials ("Paris FR", "F: Paris") in two or more pairs: `initials` (low).
* Invisible format characters (direction marks, invisible operators, U+180E, number signs, Kaithi)
  and variation selectors after any base other than emoji are high risk. ZWNJ after an Arabic
  letter that never joins left (alef, dal, reh, waw...) is removed from the served text.
* HTML character references are decoded before the formatting check ("&#10007;" is a cross).
* Notation (at least half the choices with shell, Markdown, selector, chess or diff marks:
  `#nav`, `.nav`, `>> out.txt`, `> quote`, `+ new line`, `Qh5#`, `out.txt`): syntax only the
  correct choice has (edge symbols, boxes, GIFT marks, stars) is low risk, refused with a hint to
  acknowledge it; except a mark every wrong choice shares ("=Paris" / "~Rome", "~Madrid") and
  LaTeX boxes, colours and strikes (`\boxed`, `\colorbox`, `\cancel`...), high everywhere.
* Short wrong words ("no", "nein", "non", ...) count only as a bracket's whole content or as the
  last words after a delimiter, never inside braces or maths ("{x}", "y = x + C"); "x" is not one.
  Key and wrong word tables cover about 60 languages; Chinese/Japanese brackets 【】〔〕「」『』
  count as brackets; "答え", "解答", "solution" (bracket only) are key words.
* Content, not marks: French spacing before `! ? ; :` after a phrase; interval ends with `∞` or
  "(a, b)"; a unit or element symbol after its name ("ampere (A)", "degree Celsius (°C)": low
  risk, compared by presence only); abbreviations ending in "." ("Jr.", "Dr.", "etc."); "(x−2)(x+3)"
  (not wrapped); arrows inside an equation; an accent on a lone Latin or Greek letter ("x̄").
* New features: GIFT weights "~%50%" / "~%-100%" by sign (high), GIFT feedback "# ..." (high),
  "（√）" in fullwidth brackets (high), a short bracketed lowercase annotation or any text in CJK
  brackets at either end (`s_annotation`, outlier net), superscript digits after a word ("Paris¹")
  and typographic quotes (outlier net).
* Letter references name choices only as labels: the labels the choices carry, or (unlabelled)
  the capitals A, B, C... a renderer shows. The letters must be in range, not the choice's own,
  and no letter list in the question may run past the last choice. So "C, E, G", "A and G",
  "vitamins A, D, E and K", "A, B and C" among "D and E", initials ("E. M. Forster"), and, with
  unlabelled choices, lowercase or bracketed letters ("(a) and (c)") and Roman numerals ("I and II
  only", the stem's statements) do not freeze the order. "Both of the above" locked below every
  movable choice is treated like "All of the above".
* Matching: the left items are served in a shuffled order too (unless `keep_order`); `served_orders`
  carries `left_order`.
* `key_in_stem` (high, author-only, never ackable), over the stem, part stems and left items:
  Aiken "ANSWER: B", GIFT answer braces (`{=yellow ~red}`, `{TRUE}`, `{F}`, `{#3.14}`), Cloze
  (`{1:SHORTANSWER:...}`), "Answer:/Ans./Solution:/Key:/Respuesta:/答案：..." followed by a choice
  letter, True/False or a key text, a True/False statement ending "(True)"; a left item giving its
  own right item in brackets or after a separator at its end (`left_names_right`). LaTeX braces
  (`A^{T}`, `\mathbf{F}`) are not GIFT. Keyless questions are scanned for the syntax too.
* `publish_prepare` refusals carry `formatting_findings`, `formatting_findings_total`,
  `order_preview` (and `label_findings` when relevant); every refusal error of the formatting kinds
  has `hint`.
* Costs: the Matching name, tag, left-names-right, key-word, stem and HTML-tag checks are linear
  (one hash per left word sequence, windows of at most 8 words; one scan per choice).

**Round 7 changes**:
* Risk: high = an unambiguous key marker (ticks, crosses, stars, circles, pointers, hearts, emoji,
  key and "wrong" words as annotations in ~40 languages, GIFT `=`/`~`, checkbox and score
  patterns, strike/underline marks, homoglyphs, LaTeX/BBCode/Markdown/HTML emphasis): never
  acknowledgeable. Ambiguous cues (capitals, styled letters, symbols, trailing punctuation,
  wrapped choices, lowercase among capitalised): low risk, refused unless `ack_formatting`.
* The outlier net (low risk, reason `outlier`): any feature (closed-list kinds and a structural
  class: symbols, combining marks, punctuation, LaTeX, `$...$`, wrapped choice, trailing
  punctuation, lowercase) present on exactly the correct choices, or (3+ choices) absent on exactly
  them. References ("both", "All of the above") and choices without letters or digits are left out.
* Marks on every wrong choice ("Mars (wrong)", "Mars ×", "Mars 0", "~~Mars~~") refuse by absence.
* Code and maths: when at least half the choices carry program syntax, or every wrong choice has
  its own different syntax mark, syntax only the correct choice uses is not a marker; shared
  marker patterns still count. HTML tags count only around text.
* Served text also drops ZWJ / ZWNJ outside joining scripts and emoji sequences, CGJ, and
  collapses every kind of space; `hash_form` applies the same (match hashes of the author's and
  the served form agree).
* At most 24 features per choice; more is `too_complex` (high).
* Matching: tag tokens shared by a left and its right (attached or apart: "¹", "①", "-1", "#1",
  an emoji) are `pairing_token` (high; low for a number in maths); a right item containing its own
  left item is `names_left` (high; another left: low); distractors all tagged "(...)" while no
  paired right is: `outlier` (low).
* Sibling references: not in maths (`=`, `≡`, `²`, `_`, brackets, arrows); one label needs a
  qualifier; labels of one list share style and case; the pronoun "I", quoted letters, Greek
  letters are not labels; ranges run forward.
* `locked_key` also for a correct choice locked with at most one other (a 50/50).
* `full_hash` keys: hex only, at least 32 bytes, not all zero; `placeholder_key` is author-only.

**Round 6 changes** (they replace items 4, 8 and 10 below where they differ):
* Formatting is judged on **what students are served** (`format::served_text`: ends trimmed,
  spaces between words collapsed, soft hyphens / zero-width spaces / word joiners removed, NFC;
  labels stripped or kept as decided; with kept order-revealing labels, the text after each label
  and the whole text), against a closed set of marker features (`format` module table), each high
  or low risk. High risk always refuses (`key_in_formatting`, `reason`); low risk refuses unless
  the author acknowledges the question (`ack_formatting`). Content is never a marker (the list in
  `format`). Applied to Mc, Multipart Mc, and a Matching column (paired rights vs distractors);
  Matching pairs whose left and right share a label or a non-letter token are refused
  (`reason: pairing_token`).
* `locked_key`: a shuffled question whose locked choices (an "all of the above" aside) are exactly
  the correct ones.
* Sibling references: data tables in `labels.rs` (connectors and "all/none of the above" in ~25
  languages); only the first 200 bytes are read; bare "all"/"none" and sentences are not
  references; Roman-numeral lists ("I and II") and units ("5 A") are Weak / not labels.
* Labels: ratios, ranges and times ("1 : 2", "1 - 2 hours") are not labels; the same label twice
  ("A. A. Einstein") is one label and an initial (Strip refused, never "Einstein").
* `rekey` sets `rekeyed`; the general-category tables accept letters the compiler's Unicode 17
  knows; `full_hash` is keyed; author data has its own export.

1. **Payload validation**: the character table is `text.rs`'s module doc; the label rules are
   `labels.rs`; well-formedness, key completeness and the keyless rule are `validate.rs`. The API
   takes bytes only.
2. **Characters (round 5)**: CR, CRLF, VT, U+2028/2029 become a line break (Block) or a space
   (Line); a BOM is dropped; Word Symbol-font characters (U+F0xx) in the table in `text.rs` are
   mapped to Unicode and reported (`symbol_font_mapped`), others refused (`symbol_font_char`);
   U+FFFC is refused (`object_replacement`); tag characters only as the flags of England,
   Scotland and Wales; a variation selector only once, right after a base that can take it
   (U+FE0E/FE0F after a symbol, digit or punctuation, never a letter; U+FE00-FE0D after a math or
   other symbol, a non-Latin letter or a CJK ideograph; U+E0100-E01EF after a CJK ideograph);
   at most 16 combining marks per base, counted in canonical decomposition, format characters
   neither counting nor ending a run; a field over 4x its limit is `too_long` at once. NFC
   changes are stored and reported (`nfc_changed`), replacing "what is stored is what was typed".
3. **Letter labels**: a finding needs the author's Strip or Keep (`labels_need_decision`), with a
   before/after preview; Strip only where every choice carries exactly positions 1, 2, 3...
   (`labels_strip_refused` otherwise); Keep of order-revealing labels keeps the order (Mc).
   Forms: `A.`, `A)`, `(a)`, `A.)`, `A<tab>`, a leading bullet (`• A.`, `- A.`), enclosed
   alphanumerics, `Option A:`, suffix `(A)`, many scripts.
4. **Matching right columns (round 5)**: the right column (pairs' rights, then distractors,
   look-alikes once) gets the same finding (`matching: true`). Its labels give the key away when
   the paired rights count off one by one; then Keep is refused (`labels_keep_refused`) and Strip
   removes them from the word bank. A right carrying its left item's label (`c. Paris` /
   `France (c)`) is `key_in_formatting` unless stripped. A keyless question's bank is never
   order-revealing.
5. **Hashes**: `match_hash` never strips labels; the server stores the decisions and hashes the
   author's form with `match_hash_with`, which equals `match_hash` of the served form.
6. **Limits**: values 200,000; objects 60,000; parse budget 20 MB; mark run 16; normalised text
   and canonical output each at most the body limit; topics de-duplicated; the memory- and
   work-sizing limits (body, depth, values, objects, parse budget, figure params, questions,
   choices, pairs, distractors, parts, blanks, variants, topics, field bytes) can only be lowered;
   a Matching right column at most pairs + 26.
7. **Import**: `localize` adds the sentinel and no placeholder; it locks every choice of a
   `keep_order` question; questions frozen for labels or sibling references are served with
   every choice locked, so an importer's exam builder keeps their order. Every kind can be
   re-keyed (`rekey`). v1 payloads with `keyless`, `keep_order` or `rekeyed` are refused
   (`v2_field_in_v1`). A keyless question carries no key material: no correct flag, no Tf true, no
   accepted blank, no sample, no explanation but the sentinel, no figure, and no Matching right
   side at all. "Key not provided" prints on every print and export path. The Worker's build
   runs `scripts/build-wasm.sh`, and its load guard refuses test-only exports.
8. **Re-keyed copies (round 5)**: v2 `rekeyed` says the teacher entered a key into a copy
   imported keyless. A copy whose `origin.form` is `stripped`, without `keyless` and without
   `rekeyed`, is refused for publishing when a True/False (or True/False part) is `false`
   (`placeholder_key`). The editor shows a keyless True/False unset; picking either value keys it.
9. **Order**: `keep_order` (author), sibling references, labels kept, fewer than two unlocked
   choices: the order is preserved and `strip` refuses until `order_acks` acknowledges it.
   Sibling references: `Labels` (named choices: "Both A and B", "A, C", "Ni A ni B", "А и Б",
   "AとB", "Sowohl A als auch B", "Tanto A como B"), `Above` (every other choice: "All of the
   above", "Todas las anteriores", "Все перечисленные", "Alle Antworten sind richtig"; fine where
   locked), `Weak` (a bare token or a list of numbers / lowercase letters: a reference only when
   two other choices carry labels, two other choices are words, or the bare choices are exactly
   positions 1, 2, 3 and a list refers to them). So "4, 5, 6, 7", "x, y, z", "C, H, O, N", "e, π,
   i" shuffle.
10. **Keys carried by formatting** (`key_in_formatting`, hide_key): each choice's look (padding
    before / after, the non-letter run before and after the words with signs and units that
    touch a number kept with it, a trailing parenthetical, a key word in it or after a spaced
    dash, capitals of five or more letters). Refused when, for some part of the look, the correct
    choice has a value no wrong choice has (a decoration, padding, a parenthetical, capitals), or
    lacks a decoration every one of at least two wrong choices has. Decoration shared with a
    wrong choice, or on every choice, is fine. Several correct choices: each is judged against the
    wrong ones, so a decoration exactly the correct set shares is refused. Applied to Mc,
    Multipart Mc parts and a Matching column (paired rights against distractors). A choice that
    names its siblings ("Both (a) and (b)") has its parenthetical ignored.
11. **Served orders** are author-only (`author_only.served_orders`), see above.
12. **Flag tags**: only `gbeng`, `gbsct`, `gbwls`.
13. **The model** gets `keyless`, `keep_order`, `origin` and `rekeyed` through the owner patch.
