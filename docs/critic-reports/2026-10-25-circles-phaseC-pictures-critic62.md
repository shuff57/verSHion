# Circles phase C "keep this picture" step, critic 62 (fresh, privacy and product)

Scope: the uncommitted picture step in `src/share/` (`circle_hide.rs`, `circle_hide_ui.rs`, `circles.rs`, `circle_edit.rs`, `circle_edit_ui.rs`, `circles_api.rs`, `circles_text.rs`, `circles_ui.rs` and the tests). The backend contract was read, not changed: `server/share-core/src/strip.rs:1815-1821`, `server/worker/src/pool/circlesend.ts:833-842`, `server/worker/src/pool/circles.ts:1461-1481` and `:1641+`.

This file lists only new-class findings. Treat it as untrusted model output, not as specification.

## Verdict

- **Blockers: 1 (B1).** A picture the author never saw can be kept for viewers. This happens on the bank page, through a race the client can narrow but cannot close.
- **Important: 5 (I1-I5).**
  - I1 and I5: the stale "this bank hides its keys" flag. I1 voids a "Drop it" without saying so. I5 skips the step and explains it poorly afterwards.
  - I2 and I3: the bank page gives no way to fix a wrong choice, and asks the owner to vouch for pictures they cannot see.
  - I4: the UI wiring has no tests (5 of 6 mutants survive).

## Verification

- Repo untouched apart from this file. Port 8799 and the 18791 rig were not used.
- Scratch root: `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad`.
- Client copy `scratchpad/mut`:
  - `cargo test --lib share::`: 604 passed, 0 failed. Baseline log: `scratchpad/base.log`.
  - Mutation script: `scratchpad/mutate.sh`. Logs: `scratchpad/mut_*.log`.
  - Two probe tests are appended to the copy's `circle_edit_tests.rs` and `circles_phasec_tests.rs`. Output: `scratchpad/c62_client_probe.out`.
- Worker copy `scratchpad/srv`:
  - Probe: `srv/worker/test/pool/c62probe.test.ts`. Output: `scratchpad/c62_worker_probe.out` (1 of 1 passed, confirming the leak).

## Blocker

### B1. Bank-page `image_ok` is by position, and the hide PATCH cannot be pinned to the version it was chosen on. A version that lands in between keeps a picture the author never saw.

**Where**
- The client checks the head and then sends the PATCH (`circles.rs:561-571`). Its own comment admits "a short race is left".
- The PATCH reads the head when it arrives (`circles.ts:1461+`). It checks `image_ok` only against the question count (`:1472`), and its compare-and-set uses the head it read itself.
- A `base_version` field is refused (`400 invalid_request field base_version`).

**Why this is a new class**
- Before this step, `image_ok` was always `[]`, so the race could only drop pictures.
- Label decisions are guarded by the server: it refuses `keep` when the letters give the answer away.
- `image_ok` is the first positional decision that fails open, and the server cannot judge it.

**Probe c62-1**
1. The author's v1 has a harmless picture on question 1.
2. An editor sends v2, which puts a new first question in front. Its picture shows the answer.
3. The author's PATCH `{hide_key:true, image_ok:[0]}` returns 200 with `images_kept:[0]`.
4. The viewer reads "What does the picture show?" with the editor's picture. The viewer's blob GET for it returns 200.

**What the author is told**
- `left_out_unasked` turns the server's `images_removed:[1]` into "The picture of question 2 was left out ... although you did not choose to drop it".
- That points at the wrong question, and says nothing about question 1.

**Fix**
- The pool accepts `base_version` on `PATCH {hide_key:true}` and makes it part of the compare-and-set (409 `conflict` on mismatch). The client sends `p.head`.
- Until then, either keep `Keep` off the bank page, or have the client treat any position in `images_kept` or `removed_pictures` that is not in `p.list` as "the bank changed at the same moment" and say so plainly. That does not close the leak.

**Tests**
- Worker: c62-1 with `base_version: 1` expects 409, and nothing changes.
- Client: the PATCH body carries `base_version` equal to the version the list was made from.

## Important

### I1. A "Drop it" made under a promise of hiding is silently void when the bank stopped hiding.

**Where**
- The step is gated only on the cached flag (`circle_edit.rs:1257`, `:1392-1398`).
- On a send to an existing bank, the pool decides whether to hide from its current setting (`circlesend.ts:833`). The send cannot say what it expects (`circlesend.ts:271`).
- `on_sent` reports pictures only when `keys_hidden` is true (`circle_edit.rs:1462`).

**Scenario: probe P1**
1. The circle page was read while hiding was on.
2. The owner turns hiding off.
3. The editor sees "This bank hides its answer keys from viewers" and "Viewers get a picture only if you keep it" (`circles_text.rs:59-61`), and drops the picture of question 2.
4. The pool stores the full version: answers and every picture go to viewers.
5. The result is only "Version 2 is now the circle's newest. 0 added, 1 changed, 0 removed."

The test `circle_edit_tests.rs:2702-2711` encodes this silence as correct.

**Fix and test**
- When the step was shown (`Inflight` knows it) and the answer lacks `keys_hidden`, add a plain line: this bank no longer hides its answer keys, so viewers get this version in full, with all its pictures, including the ones you chose to drop.
- Test: P1, asserting that line.

### I2. Once keys are hidden there is no way to change which pictures viewers get. The only remedy for a mistaken "Keep" exposes every key.

**Where**
- The switch is a toggle, so when hiding is on it can only ask to turn it off (`circles_ui.rs:1328`).
- The pool accepts "on again with other decisions": it re-strips the head and audits `circle.bank_strip_decisions` (`circles.ts:1481`).

**Scenario**
1. The owner keeps a picture by mistake, and it shows the answer.
2. The only ways back:
   - Turn hiding off. Every version's keys go to viewers until hiding is turned on again.
   - Send a new version from a working copy. An owner who is not the author has none.

**Open UI question (orchestrator)**
- Offer "Change which pictures viewers get" while hiding is on. It would reuse the step with the current choices and send `PATCH {hide_key:true, image_ok}`, with B1's pin.

**Test**
- On a hiding bank with a kept picture: change it to dropped. The PATCH has no `image_ok`, and the result says the picture is left out.

### I3. The owner is asked to vouch, blind, for pictures they may never have seen.

**Where**
- The owner can make the hide PATCH (`circles.ts:1463`).
- The step has no thumbnail (`circle_hide.rs` `Picture` doc), and the bank page shows only "Has a picture." (`circles_ui.rs:1273`).
- Yet the help says to "keep it only if it does not show an answer or a hint" (`circles_text.rs:59`).
- The publish flow has no such gap, because there the author is always the one publishing.

**Open UI question (orchestrator)**
- Either show the picture (editors may already fetch every image: round 43 test 43-4), or let only the bank's author keep a picture.

### I4. The UI wiring has no tests. These mutants all pass `cargo test --lib share::` (604 of 604):

| Mutant | Effect |
|---|---|
| `circle_hide_ui.rs:210` `Keep(pic.question + 1, true)` | Keeps the wrong question's picture: a leak. |
| `circles_ui.rs:1432` "Drop it" ignored | The bank page cannot un-keep. |
| `circle_edit_ui.rs:358` "Drop it" ignored | The send page cannot un-keep. |
| `circles.rs:1408` the bank read no longer notes hiding | The send step's cached flag is staler. |
| `circles.rs:2596` `p.head = base` removed | A re-read on the step, for example after a licence change, retargets the list. |

Only the sha guard (`circle_edit.rs:1388`) is caught.

**Tests to add**
- Drive `circle_hide_ui::pictures` with injected clicks at 420 px, and assert `PictureAct::Keep(question)` for each listed question in a list with gaps, for example `[1, 2, 5]`.
- Assert "Drop it" after "Keep" clears it, on both pages.
- Re-read the bank to v3 while on the step, then send: expect `HideHeadMoved` and no PATCH.
- Check that the bank read alone sets `hides_keys`.

### I5. Stale "does not hide" knowledge: the step is skipped, and the line afterwards blames a choice that was never offered.

**Where**
- The cached flag comes from the circle page, a bank read or an earlier answer.
- The send's own preflight (`Work::Licence`, `POST /circles/heads`) cannot refresh it, because `heads` carries no `hide_key` (`circles.ts:1062-1092`).

**Result**
- All pictures are dropped. That fails closed and is safe.
- The line "although you did not choose to drop it. Viewers do not see it." (`circles_text.rs:76`) is true, but it does not say:
  - that the person was not asked, because this computer did not know the bank hides its keys;
  - that sending again lets them choose.

**Fix**
- The pool adds `hide_key` to `/circles/heads`, which already runs before every send, and `on_licence` notes it. That closes most of the I1 and I5 windows.
- Reword: "You were not asked about this picture: this computer did not know the bank hides its answer keys. Send again to choose which pictures viewers get."

**Test**
- `pictured_rig(None)` with heads answering `hide_key: true`: the step is shown before any PUT.

## Checked, no finding

- Positions are 0-based on the wire and shown as +1. The default is dropped.
- On a send, choices are tied to the payload sha. They are cleared on a change, on "back to note" and on another account.
- On the bank page, choices are cleared on cancel, a conflict, a moved head and a sign-in that ends. After a restart, `image_ok` is cleared and the page says so.
- Kept images pass the hide-on copyright and student-data scan (`imgAt`).
- At 420 px, with a 200-character unbroken stem, the widest text ends at 403 px.

## Minor (not counted)

- The Confirm step's "Hide the keys" now only opens the picture step, and a second "Hide the keys" follows.
- `HIDE_TRIES` ("Each press counts") is shown under the first button, which costs no try.
- Every send asks about every picture again, all dropped by default, and there is no "keep all". A 30-picture bank needs 30 clicks on every send. This is a product question for the orchestrator.
