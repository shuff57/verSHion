# share-core review, round 19 (critic36a): Moodle lens

Verdict: the round-18 blockers (35a quadratic Matching rights scan, GIFT `{TRUE#fb}`) are FIXED. I found three new-class
blockers: one CPU (labels shape, 11-28 s), two fail-open holes in the Matching pair-list scanners.

Environment. wasm copy of `server/share-core/pkg`, never rebuilt: sha256 `22dc2c89...386c0e`, 1,264,436 B, mtime 20:44.
Scripts in `$D` = `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/tmp.n8SpPC8vFx`
(`h.mjs` loader/builders; run `cd $D && node <script>`). CPU = `process.cpuUsage()`, min of 3.

Baseline (run alone): `cargo test --release` 294 passed, 0 failed, 1 ignored; `cargo clippy --release --all-targets` clean;
`node tests/node/check.mjs` 506/506 (blank170 +29.4 MB); root `cargo test --release --test share_core_fixtures` 8/8.

## (a) Previous findings

| finding | command | result |
|---|---|---|
| 35a B1 CPU, 480 keep_order Matching + 480 "Yes, No, Maybe, " stems, 4,041,369 B | `node cpu1.mjs` | **FIXED**: keep 1.76 s (2.00/1.79/1.76), control 1.19 s, `ok:true` |
| 35a B2 GIFT True/False feedback, 11 forms x (Tf stem, Tf part stem) | `node prev.mjs` | **FIXED**: all 22 `gift` |
| 35b B1 "x - " 950x26 labels, 3.97 MB | `node lab.mjs` | **FIXED** with decisions 1.52 s; without decisions 3.9 s (see B36a-1 for the sibling shape that is not) |
| 35b B2 heading + numbered row | `node hd.mjs` | **FIXED** for plain headings (Answer key, Key, KEY, Answers, Solution, Mark scheme, ...; 7 row styles). Decorated headings: see (c) |
| 35a/earlier shapes (480 shared-column Matching, Key rows, Key cross-bank, TF cross, letters) | `node cpu2.mjs`, `node cpu3.mjs` | all under 2.3 s; pair-scan hostile shape ends `scan_budget` (fail closed) |
| honest 4 MB Mc banks (900-1000 q x 26), labels none/paren/dot, with Keep | `node hon4.mjs` | no false `scan_budget` (1.3-2.6 s) |
| honest map/circuit legends under `Key` | `node leg.mjs` | refused only when a row states the question's answer or first choice correct (documented) |

## (b) New-class blockers

### B36a-1. CPU 11-28 s on 3.8 MB: single capital-letter units after capital labels

Class: CPU over 5 s (min of 3) on a payload under 4 MB.
Input (`up.mjs upper 1000`): bank of 1000 Mc questions, 26 choices each, choice k = `"<A..Z>. " + "X - " repeated to 95 B`
(`"A. X - X - X - ..."`), correct = choice 0, stem "s". 3,815,209 B. `publish_prepare(bytes, '{"hide_key":true}')`.
Output (`node up.mjs upper 1000`): 13.64 13.50 14.11, **min 13.50 s**, result `labels_need_decision` (a plain refusal path).
`node up2.mjs` (per export, min of 3): `formatting_findings` **18.18 s**, `label_findings` **8.60 s**, `publish_prepare` no decisions **11.46 s**,
`publish_prepare` with every label "keep" **27.84 s** (`scan_budget`), with "strip"/auto 1.07 s / 27.74 s; `validate` 0.08, `strip` 1.13,
`match_hash` 0.13.
Controls (`node up3.mjs`, same size 3,815,209 B): lowercase unit `A. x - ...`: formatting_findings 1.54, label_findings 0.58, publish_prepare 2.90 s;
lowercase label `a. X - ...`: 1.57 / 0.57 / 2.94 s. Also `upper_paren` ("A) X - ") 12.34 s; `noLabel_upperX` 4.51 s.
Only capital label + capital "X" unit differ, so the cost is in the sibling/label reading of capital single-letter tokens, the round-19
`x - ` family with a capital letter (the memo and budget charge fixed `x - `, not this). Not a quadratic in questions; per-question cost ~13 ms.
Why it counts: the author-facing `label_findings`/`formatting_findings` calls (8-18 s) and the first `publish_prepare` run well over the 5 s line.
The honest-content likelihood is low; the line is CPU > 5 s.

### B36a-2. Bank pair-list scan skips any item that contains a pair-mark character (fail-open)

Class: fail-open scanner (text normalisation mismatch). Mechanism (`src/strip.rs`): the text is folded with `pair_mark_form`
(every `-`, `>`, `:`, `=`, `,` becomes one mark and spaces around it are dropped) but the lexicon entries are `fold_key(left) + mark + fold_key(right)`
built with the raw item. A left or right that itself holds `-`, `,`, `:`, `=`, `>` (en dash is folded to `-`) can never equal the text form.
The single-question stem rule (`strip.rs` ~line 813) has the same comparison. The rights-order scan has the same hole: `word_bank_lists` splits
on `,`/`;`, so a right such as "Paris, France" is never an item.
Exact inputs and outputs (`node hy2.mjs`, `node hy3.mjs`, `node wo.mjs`, `node wo2.mjs`): 4-pair Matching question, the two pairs whose items hold the character
listed as `Left - Right; ...` either in the question's own stem or in the bank description; control = two plain pairs listed the same way.

```
char  side   stem          bank description   control (plain pairs)
 -    left   CLEAN         CLEAN              pairs_listed
 ,    left   CLEAN         CLEAN              pairs_listed
 :    left   CLEAN         CLEAN              pairs_listed
 =    left   CLEAN         CLEAN              pairs_listed
 >    left   CLEAN         CLEAN              pairs_listed
 en-dash (1914-1918)       CLEAN  CLEAN       pairs_listed       (same for right-hand items)
 /    left   pairs_listed  pairs_listed       (not a mark: read)
```
Realistic: stem "Match each thinker to the idea. Key: Jean-Paul Sartre - Existentialism, Jean-Jacques Rousseau - Social contract" with
`Jean-Paul Sartre`/`Existentialism` etc.: `ok:true`, and `stripped.payload.question.stem` still contains the two pairs. With "Key:" and " - " or an en dash it is CLEAN;
"Answers:", "Answer key:", "Mark scheme:" lead words are refused by `answer_line` (a different rule), so the pair rule is the only guard for `Key:`, no lead, "Teacher note:".
Rights order: `keep_order` Matching, rights `Paris, France` / `Rome, Italy` / `Madrid, Spain`, lefts `Landmark 1..3` served in order, stem
"Locations: Paris, France; Rome, Italy; Madrid, Spain": CLEAN, published (`wo2.mjs`); with rights `France/Italy/Spain` the same text is `word_bank_order`.
Same for colon rights ("Genesis 1:1", "John 3:16") listed with comma/semicolon: CLEAN, while hyphen rights ("Jean-Paul Sartre") ARE read (a list split only on `,` `;`).
Differential: plain items 100% refused, items with a mark 0 of 10 sampled combinations refused.
Honest content: hyphenated names, "Washington, D.C.", scripture references, date ranges are ordinary Matching items.

### B36a-3. Bank pair-list scan starts only at a letter or digit (fail-open)

Class: fail-open (a prefilter that silently skips). In `strip::bank_key_list_errors` a pair is searched only at a position where `c.fast_alnum() && !prev_alnum`.
A left item that begins with a symbol is never searched. `node sym.mjs` (4-pair... 3-pair Matching, all pairs listed in the bank description as `L - R; ...`;
the single-question stem form of the same text is refused in all 16 sets):

```
set (left items)               bank description ' - '     stem
"√9" "√16" "√25"               CLEAN                      pairs_listed
"°C" "°F" "K"                  CLEAN                      pairs_listed
"$5" "€5" "£5"                 CLEAN                      pairs_listed
"(a)" "(b)" "(c)"              CLEAN                      pairs_listed
"#include" "#define" "#if"     CLEAN                      pairs_listed
"+" "×" "÷"                    CLEAN                      pairs_listed
'"hello"' "'a'" "`x`"          CLEAN                      pairs_listed
emoji, "*args" "**kwargs", ".py" ".js" ".rs", "/etc" "/var" "/usr", "~" "&&" "||"   CLEAN   pairs_listed
controls: "π" "Σ" "Δ", "1" "2" "3", CJK, Arabic, plain words      pairs_listed
```
11 of 16 sets pass in the bank text (also another question's stem, since those are bank texts). Moodle matching of units, currency, brackets,
code symbols is ordinary. The same text in the question's own stem is refused, so the author sees a differential. The right-order scan reads these items (only the pure-symbol set `+ × ÷` passed).

## (c) Non-blockers (terse)

* Near miss, CPU: first-call `publish_prepare` of the "x - " 1000 x 26 shape without decisions 3.75-3.99 s (5.65 s on the first run); with decisions 1.5 s (`lab3.mjs`, `lab4.mjs`).
* Matching pair-list separators still unread (documented family): `~`, `/`, `|`, tab, leaders, "matches", "is", `<->`, `<=>`, `≈`, `»` (`sep.mjs`). `<->` is a plain two-char arrow.
* Rights list wrappers in bank texts only (same list in the question's own stem is refused by `answer_line`): quoted, parenthesised, bold `**x**`, bracket tags, tab-separated, arrow chain, " or ", "and" before last, numbered/lettered lines, reversed (`wl.mjs`). The "and" and numbered forms are documented.
* A rights list repeated 3 times ("Aluminium, Soft, Roof, " x3) with 5 or more questions sharing the column is CLEAN: the longest run is 3 of 9 found (< 60%) and the shared-word suppression hides the "word bank three" rule (`wb2.mjs`). Contrived.
* Decorated `Key` headings with a numbered row are CLEAN: `**Answer key**`, `# Answer key`, `<b>Answer key</b>`, `Answer key.`, `Answer key (hidden)`, `Answer-key`, `Answer sheet`, `Scoring key`, `Grading key`, `Rubric`, `Memo`, `Marking guide` (`hd.mjs`; rows `1. Paris` etc.). Marker/heading variants of the fixed family.
* Feedback phrases after the correct choice ("Venus. Excellent", "Nope", "Spot on") are not read; only vocabulary (`fbk.mjs`). Documented word-table limit.
* Honest `Key` legends: refused when the first choice is correct (documented) or when the row states the answer (correct).
* Over-budget honest-looking shape: 700 keep_order Matching + 300 8 KB stems that are comma-separated common words (1 word per comma) gives `scan_budget`; prose with a comma every 2-5 words is CLEAN (`fb.mjs`). Not an honest text.
* GIFT TF-feedback scanner reads only English true/false/t/f and a `{` followed by at most 64 spaces (language variants).

## (d) Not run

* The 627-item honest corpus, the 158-marker corpus, `--ignored` amplification sweep, mutation runner, fuzz over exports.
* Native differential of the SHA-256 sibling memo (only `b3_sibling_memo_is_exact` in the baseline run; no fail-open found by inspection).
* Worker end to end, shuffle fairness statistics (round 19 did not touch the shuffle), Argon2id, importers.
* B36a-1: I did not profile the Rust source for the root cause; the controls isolate the shape only.

BLOCKERS: 3
