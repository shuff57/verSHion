###TASK 2026-10-22T15:46:09.573Z
I found **4 blockers**: 2 hidden-key leaks where a cap or window fails open, and 2 shapes that run far past the time budget. Separately, a classifier stopped one of my earlier responses mid-task, so a few checks (listed at the end) were not run.

Everything ran in wasm on a copy of `pkg/` (built 07:35), in Node, inside the scratchpad folder `critic31b`. No repo files were edited and port 8799 was not used. The machine load average was 20 to 60 on 24 cores, so times below are the minimum CPU time of 3 runs. As a reference point, the plain `mc.choice "correct"` shape took 1.45 s at 4 MB under the same load.

## Blockers (all ran)

**B1. Hidden-key leak: the 200-character window after an answer lead cuts the key off.** A key that starts inside the window but ends past it is never matched, because the trie walk stops at the window's end without reaching the key's end (`src/format.rs` around 3569 and 3703). This breaks the README's promise of "any word start within 200 characters", and any key longer than about 190 characters is missed after even one filler word.
- Mc question, correct choice K (228 characters: "Because the light from the Sun is scattered by the molecules of the atmosphere, and the shorter blue wavelengths are scattered much more strongly than the longer red ones, so blue reaches our eyes from every direction of the sky"). Stem `"Why is the sky blue?\nAnswer: probably " + K` gives `publish_prepare {hide_key:true}` → `ok:true`, with the key served verbatim. The same happens with the prefix `"it is: "`. With no prefix it is refused (`answer_line`).
- Short key: correct choice `"Olympus Mons"`, stem `"Which is the tallest volcano in the solar system?\nAnswer: " + &lt;filler&gt; + "Olympus Mons"`, where the filler is the repeating words "as we said in class the thing is ". With 187 filler characters it is refused; with 188 or more it is `ok:true`.
- The same gap applies to another question's key in a bank stem and to the bank description: filler of 195 gives `ok:true`, filler of 20 is refused.
- Repro: `t_window.mjs`, `t_window2.mjs`.
- Likely fix: walk the trie on the folded text up to `min(line end, p + KEY_WINDOW)`, not inside the window string `tc`.

**B2. Hidden-key leak: the 4,096-event cap in `bank_key_list_errors` fails open.** In `src/strip.rs:1043`, once `ev.len() &gt;= 4096` the rest of the text's words are silently skipped. There is also `ev.truncate(4096)` at line 1110. Neither sets `over`, so the text gets no `scan_budget` refusal.
- Bank of four questions:
  - q0: Blank whose accepted answers are `"zq"`, `"zq zq"`, … up to 682 copies, spread over 35 blanks of 20 accepts each.
  - q1: True/False, true.
  - q2: True/False, false.
  - q3: Mc.
- Description `"zq "×95 + "2 T 3 F"` → `ok:true`. With `"zq "×85` (3,655 events, under the cap) it is refused (`bank_key_list`). The forms `"Results 2 T, 3 F"` and `"Q2 (T) Q3 (F)"` behave the same.
- Repro: `t_evcap.mjs`.
- Fix: past the cap, set `over = true` (that is, refuse with `scan_budget`).

**B3. Time: Matching left items filled with the Amharic lead `መልስ:`.** A 4 MB bank takes 8.7 s; at 1 / 2 / 4 MB it takes 2.4 / 5.1 / 8.7 s. The first sweep's figures (1.6 / 3.8 / 11.7 s) gave an exponent of about 1.4; this standalone rerun is roughly linear but still far over budget. It is refused with `scan_budget`, but only after the full cost. Shape: 649 Matching questions, each with 3 pairs whose left is `"መልስ:"` repeated to 2,040 bytes plus `p+"l"+i`, right `"Right p i"`, one distractor. Command: `node repro_time.mjs matchleft 649`.

**B4. Time: Multipart part stems filled with the label `2)`.** A 4 MB bank takes 11.4 s, scaling linearly (2.6 / 5.7 / 11.4 s at 1 / 2 / 4 MB). Shape: 326 Multipart questions; part 0 stem is `"2)"` repeated to 8 KB (Mc, key `"2) 2) 2) zq"+i`), part 1 stem is `"2)"` repeated to 4 KB (True/False), part 2 is Short. Command: `node repro_time.mjs partstem 326`.

## The generic sweep (A)
- **Stage 1:** 823 tokens × 21 fields × spaced/glued, at about 384 KB: 34,734 of 35,700 jobs finished. There were 0 traps, and the largest memory growth was 3.25 MB.
- **Stage 2 (1 / 2 / 4 MB):**
  - Matching right items with verdict words: `തെറ്റ്` 5.5 s (spaced) and `ತಪ್ಪು` 5.3 s (glued) in the sweep; my standalone rerun of the Malayalam shape gave 3.9 s, and both publish `ok:true`. These are over 3.5 s, but I don't count them as blockers because they are close to load noise.
  - Earlier, at 4 MB with `Answer:`: `mc.choice` 3.3 s and `mp.partchoice` 3.4 s, with about 31 MB growth.
- **Not finished:** only about 10 of the top stage-1 candidates got stage 2. Fill-fraction (within-field) scaling was not done, and the label and formatting findings exports were not timed. The other 4 MB shapes stayed under 40 MB of memory growth.

## Information flow (B, ran)
- **Property test:** 123,450 key-moved variants from 24,690 base payloads, covering Mc, True/False, Blank, Short, Matching and Multipart, with leads, word banks, bank lists and the bank-key forms. There were **0 differences** in anything not marked `author_only`. That covers non-author errors, `order_preview`, `label_findings`, `needs_order_ack`, the served payload (order sorted) and `stripped.match_hashes`. Long `scan_budget` texts were not in this generator.
- **`scan_budget`:** every `scan_budget` I saw carries `author_only:true`. Whether it fires does depend on the key, but only the publishing author, who already has the key, receives it, so there is no oracle for anyone else.
- **Non-blocker:** `labels_keep_refused` / `labels_strip_refused` and the Matching `order_revealing` / `strip_allowed` flags depend on the pairing and are not marked `author_only`. They also appear in strip's success response, which the README says has "No author data". This is documented (round 5) and I see little practical leak.
- **Non-blocker:** `formatting_findings` items name the key's choice but carry no `author_only` flag, unlike the matching errors.

## Shuffle uniformity (C, ran)
All tests pass (p ≥ 0.02 on first runs; the borderline ones went to 0.15 to 0.36 with 60k to 150k samples). Covered:
- Mc with 2, 3, 4, 5, 8 and 26 choices: position matrices, full permutations, and consecutive-call correlation.
- Locked choices: always stay in place, and the rest are uniform.
- Matching: word bank, left order, and their independence; the aligned rate was 0.1668 against 1/6 expected.
- Multipart with 50 parts: each part uniform, and parts independent of each other.

## Wasm traps (D, ran)
220,000 structure-aware payloads went through all 9 exports plus a deep strip/publish path, about 2.62M calls. There were 0 traps, 0 failed `health()` checks and 0 responses without `ok`. Inputs included 26 choices, 50 parts and 50 blanks, 1,000-question banks, huge and malformed numbers, Unicode edge cases, broken JSON and option fuzzing. About 27% of payloads validated, and about 29k reached the deep path. The native debug/overflow-check build and the source audit of slicing patterns were **not run**.

## Fail-open audit (E)
- **Ran:** B1 and B2 above.
- **Read only:** I checked that the `leads_seen &gt; 256` and `leads_read &gt; 64` limits, the global/local work budget and the Blank 4,096-places budget all refuse (fail closed). The `anki_cloze` `.take(4096)` cap needs 8,192 bytes of `{{` before a cloze, which is at or over the stem limit, so it can't be reached. The `walked` start-skip map looks safe.
- **Not run:** crafting colliding keys for `PairHasher`.

## Invariants (F, partly ran)
About 29k served payloads revalidated to themselves with 0 failures. A dedicated test of `served_text` idempotence, hash parity and ASCII-versus-slow-path parity was not run.

## Files
Scripts are in `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/a8a6fadd-a053-441c-8fa2-cd39f6730aa8/scratchpad/critic31b/js/`, and results (stage 1, stage 2, property test, fuzz) in `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/a8a6fadd-a053-441c-8fa2-cd39f6730aa8/scratchpad/critic31b/out/`.

BLOCKERS: 4</result>
<usage><subagent_tokens>211879</subagent_tokens><tool_uses>70</tool_uses><duration_ms>2537158</duration_ms></usage>
</task-notification>
###END

