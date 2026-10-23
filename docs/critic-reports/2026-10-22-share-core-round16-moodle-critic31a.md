###TASK 2026-10-22T15:33:57.143Z
# share-core review, round 16 (critic31a): 6 blockers

I counted six blockers. One of them (B6, a 166 MB memory spike) is only a blocker if you treat running out of memory on a Worker as equivalent to a wasm trap. If you don't, the count is 5. No wasm traps or panics occurred in any run. The pkg/ copy was used as built at 07:35; no repo files were changed. All work is in `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/a8a6fadd-a053-441c-8fa2-cd39f6730aa8/scratchpad/critic31a/`.

## Honest rates

**Corpus.** 627 new items: 140 base items written fresh (about 60 subjects and 36 languages not in tests/data), each rendered in several of 22 paste styles. The corpus is in `honest_corpus.json` and the results in `results.json`.

| | clean | ack | hard | freeze |
|---|---|---|---|---|
| Items (627) | 617 (98.4%) | 5 (0.8%) | 5 (0.8%) | 0 |

- **mc:** 420 clean, 5 ack, 5 hard (430 items). Every other kind was 100% clean: tf 104, match 36, blank 27, short 21, multi 9.
- **The 5 ack items** are one phlebotomy question in five styles. The correct choice "Blood culture bottles" is the only one without a parenthetical like "Lavender (EDTA)", so it is flagged `outlier` at low risk.
- **The 5 hard items** are one Quechua question in five styles: `["Killa","*Rupay / sol","Para","Wayra"]` is refused with `key_word` (high), because "sol" after the slash is read as a solution marker.

**Banks.** 17 banks plus one bank of all 627 items (`banks2.mjs`): 15 clean, 1 ack, 2 hard, 0 freeze. Both hard banks are refused only because they contain that Quechua question.

**Long texts and `scan_budget`.**
- **Original prose:** none of seven hand-written passages was refused (crypto 6.9 KB, music theory 6.6 KB, titration 3.4 KB, exam front page 2.9 KB, homeostasis 3.1 KB, keyboarding 2.6 KB). They are in `texts/`.
- **Same density stretched towards 8 KB:** music was refused at 6.2 KB and keyboarding at 7.7 KB. Titration, crypto, exam and homeostasis stayed clean up to 7–7.9 KB.
- **Honest 3–4 MB banks (7 shapes, 6,262 questions):** `scan_budget` hit only the survey-spreadsheet bank, where every question (1000 of 1000) was refused (B5). In the same run, the long-stem True/False bank also produced 100 `answer_line` false positives; the cause is listed under non-blockers.

**Shuffle fairness** (4,800 runs per test): every chi-square is below its 5% critical value. 4 choices: 22.9 over 23 degrees of freedom; key position 0.28 over 3; 26 choices: 19.8 over 25; Matching joint: 15.3 over 19; independence between bank questions: 10.5, and between Multipart parts: 9.3 (8 each). Locked choices stayed in place 4800 of 4800 times.

## Blockers

**B1. Fail-open cap in the bank answer-key scan: 64 entries per value** (`strip.rs:991`, `if e.len() &lt; 64`).
- In a True/False bank with more than 64 questions of each value, the questions past the 64th lose their "T"/"F" entries, so their numbered key is never seen.
- Input: 140 True/False questions (True when the index is even), with description `"Section D: 135 T, 136 F, 137 T, 138 F, 139 T, 140 F"`. It publishes **clean**, as do the same key with "135. True; …" and the same key in a stem.
- The same text naming questions 115–120 of a 120-question bank is refused (`bank_key_list`).
- Repro: `p1.mjs`.

**B2. Fail-open cap: 4,096 events per text** (`strip.rs:1043` `continue`, and `ev.truncate(4096)` at 1110).
- Input: a stem of `"Readings: "` + `"7 "` repeated 2,060 times + `"\n\nPage 2: 31 F, 32 T, 33 F, 34 T"`, in a bank where one Blank accepts "7" and one Short sample is "7 7". It publishes **clean**.
- With 2,040 repeats the same stem is refused (`bank_key_list`).
- It takes a lot of repetition to reach, but it is fail-open by the rule. Repro: `p3.mjs`.

**B3. Part stems, left items and topics are not read against other questions' keys in a bank.**
- Bank: four True/False questions, a Multiple-choice question with key "Quartz", and a Blank with "Mohs".
- Add a Multipart whose part stem is `"Explain. Answer: Quartz"`: **clean**. The same text as a Short stem is refused (`answer_line`, high).
- Part stems `"Answers: 1 T, 2 F, 3 T, 4 F"`, `"Q5 Quartz, Q6 Mohs"` and `"5) Quartz 6) Mohs"`: all **clean**.
- A left item `"Spores (Q1: Gills)"` and a topic `"Q1: Gills"`: **clean**.
- The README says "every stem" (rounds 12 and 15); only question stems, the bank name and the description are actually scanned. Repro: `p4.mjs`, `p6.mjs`.

**B4. A frozen order exposes the key by position.**
- Three Multiple-choice questions with `keep_order: true` and keys C, B, D (kept labels behave the same), with description `"1 C 2 B 3 D"`.
- Result: `order_ack_required`; after the author acknowledges the order it publishes. The served description is `"1 C 2 B 3 D"` and the served choices are `["Jurassic","Triassic","Cretaceous","Permian"]` in author order.
- The same happens with `"Answers: 1 C, 2 B, 3 D"`, `"1) C   2) B   3) D"`, `"C, B, D"`, `"Q1 C · Q2 B · Q3 D"`, `"Antworten: 1 C, 2 B, 3 D"`, `"答案：1 C 2 B 3 D"`, and with that text in the last question's stem or in a part stem.
- Only the glued and dashed forms ("Answers: 1C 2B 3D", "Answer key: 1-C …") are refused. Without `keep_order` the letters say nothing, so there is no leak.
- Moodle comparison: with shuffle off, Moodle letters answers by `answernumbering` (`question/type/multichoice/renderer.php:109-111`, 199-216; shuffle happens only at `question.php:64-65`). But Moodle's category info is teacher-only (`question/bank/managecategories/classes/question_category_list_item.php:100`), whereas here the bank description is served to students. Repro: `p22.mjs`, `p23.mjs`.

**B5. False `scan_budget` hard refusal on short honest data.**
- Input: `"Pupil,Key Stage,Hours of sleep\nP1,Key Stage 3,6.5\n…"` with 63 rows (1,281 bytes), as a Multiple-choice stem or a Multipart main stem.
- Result: `key_in_stem/scan_budget` (high). The hint reads: "…shorten it, or split the question or bank."
- Each "Key Stage" uses up one of the 64 lead readings allowed per text (`format.rs:3823-3826`). The word filter lets "stage", "site", "sheet" and "of" through.
- Also refused the same way:
  - a 64-row key log `"L1, Key 200, Lab"`
  - a 65-entry glossary using "Definition:"
  - 65 notes such as "see answer sheet" or "hint of rain" (1.5–2 KB)
- Renaming the column to "KS3" works, but the hint never names the word, and shortening or splitting a dataset breaks the question. Repro: `p19.mjs`, `p20.mjs`, `p21.mjs`.

**B6. wasm memory reaches 166 MB on a valid 4 MB payload.**
- Input: `blank170.json`, 4,172,388 bytes: 170 Blank questions × 50 blanks × 20 four-word variants (e.g. "ka ka ka ka").
- `validate_for_publish` and `publish_prepare` with `hide_key` grow wasm memory to 165.8 MB. Smaller cuts: 0.9 MB → 48 MB, 2 MB → 93 MB. Without `hide_key` it stays at 24 MB.
- That is about 40× the input, against the README's "at most 7× the input + 2 MB and under 40 MB", and above a Worker's 128 MB limit.
- CPU is not the problem here: 4.13 s minimum over 7 runs. Repro: `mem2.mjs`, `mem3.mjs`.

## CPU on adversarial 4 MB shapes (`publish_prepare` with `hide_key`, minimum of 3–7 runs)

None exceeds 5 s.

| shape | min CPU | peak memory | result |
|---|---|---|---|
| blank170 (B6) | 4.13 s | 166 MB | ok |
| trie_long_key | 2.60 s | 20 MB | scan_budget |
| leads_64 | 2.64 s | 52 MB | ok |
| tf_numbered_events | 2.18 s | 34 MB | ok |
| lists_shared4 | 2.15 s | 26 MB | ok |
| mp_cross_keys | 2.13 s | 33 MB | ok |
| matching_pointer_lists | 2.11 s | 28 MB | ok |
| trie_near_miss | 1.45 s | 20 MB | scan_budget |
| html_tags | 1.45 s | 32 MB | ok |
| cjk_runs | 1.10 s | 31 MB | ok |
| topics_many_keys (2.2 MB) | 3.23 s | 58 MB | topic_is_key |

## Marker corpus

158 new key hints (`markers.mjs`): 90 hard, 18 low, 50 passed. Three of the 50 are not leaks:
- padding that is trimmed when served;
- Matching numbers that are removed when labels are stripped;
- choice letters on choices that are shuffled anyway.

The leaking misses, besides B3, are variants of known mechanisms or documented limits:
- **True/False bank keys in other layouts:** `"1T 2F 3T 4F"`, `"1True 2False"`, `"Q1T Q2F"`, a two-row grid `"1 2 3 4\nT F T F"`, a pipe table, a tab grid. The README's round 15 explicitly claims coverage of the grid and the pipe table; for True/False values that claim is false.
- **Answer leads in languages missing from the tables:** 24 of 28 tested publish clean, e.g. Punjabi ਉੱਤਰ, Sinhala පිළිතුර, Burmese အဖြေ, Lao ຄຳຕອບ, Pashto ځواب, Somali Jawaab, Māori Whakautu, Malagasy Valiny, Gaelic Freagairt, Yiddish ענטפֿער, Armenian Պատասխան, Georgian პასუხი, Tajik Ҷавоб, and Albanian "Përgjigja".
- **LaTeX exam.cls forms:** `\answerline[Gills]`, `solutionordottedlines`, `\ifprintanswers … \fi`.
- **Marks on the correct choice:** "Gills v", "Gills /" and "Gills J" (OCR'd ticks), "Gills A1", "Gills[KO1]", "Gills %100%", and "Gills\t1" with no flag on the wrong choices.
- **True/False endings:** "(This statement is true.)", "(Patiesi)", "(Үнэн)", "— Fìor", and an OCR'd worksheet "(T)  F  statement".
- **Other:** Matching "Chanterelle → false gills"; another question's key inside a topic ("Unit 4: Quartz"); another question's key given as data in a stem.

## Non-blocker false positives (honest content refused, but rewording fixes it)

- The article "A" is read as choice letter A by the clause rule: `"A correct answer earns one mark."` and `"Answer all questions in Section A."` are refused when the key is A.
- `'Their first answer was "yes"'` in a True stem gives `answer_line`.
- A dichotomous key in the stem (`"Key\n1a  Has six legs … go to 2"`) is refused only when the correct choice is A.
- In a data log with an "Answer sheet NNN" column, a row whose score equals the key is refused.

Moodle has no key-leak checks of this kind. Aiken `ANSWER:` (`question/format/aiken/format.php:96`), GIFT `=`/`~` (`question/format/gift/format.php:304-308`) and XML `fraction` (`question/format/xml/format.php:307`) are all caught. Matching shuffles the same way Moodle does (`question/type/match/question.php:63-69`).

BLOCKERS: 6</result>
<usage><subagent_tokens>238245</subagent_tokens><tool_uses>57</tool_uses><duration_ms>1813816</duration_ms></usage>
</task-notification>
###END

