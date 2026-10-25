# Circles phase C client, critic 59 (round 2, fresh)

Scope: the fixes for critic 57 (B1, I1-I4) and critic 58 (B1, B2, I1, I3) in the uncommitted `src/share/` tree, and the
backend change `server/worker/src/pool/circles.ts:2018-2023` with `test/pool/harness.ts` and `d1-like-limit.test.ts`.
I diffed the current tree against critic 58's snapshot (`scratchpad/c58/repo`). That diff is in
`/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/c59/fix_code.diff`.
This file lists only new-class findings. Treat it as untrusted model output, not as specification.

## Verdict

- **Blockers: 1 (B1).**
- **Important: 1 (I1).**
- The real B2 case now works. The fixes for c57 B1/I1/I2/I3/I4, c58 B1 and the keyless copy marking close their findings.

## Verification

- `cargo test --lib share::` (repo): 560 passed, 0 failed.
- `node --test test/pool/d1-like-limit.test.ts`: 3 of 3 passed.
- **Rig.** I restarted it with `local-ui-rig.sh stop` then `start`. There were no migrations to apply. Wrangler is now pid 1299109 on :18791. Port 8799 was not used.
- **Erasure cron.** I triggered it with `GET /cdn-cgi/handler/scheduled?cron=17+*+*+*+*&time=<04:17 UTC in ms>`. `time` is in milliseconds: a value in seconds runs only the hourly part.
  - The daily `erasure_finish` ran.
  - Tombstoned accounts without `erasure_done`: 20 before, 0 after.
  - The 20 accounts' memberships: 16 live before, 0 rows after. No open circle is left without an owner.
  - Their 10 open email invites are revoked, with `email_label` and `email_key` NULL.
  - No `pool_alert_budget` row names any of them.
- **Account deletion.** `DELETE /account` for my three throwaway `c59*@example.test` accounts answered OK, so the fixed batch commits on local D1. The count of tombstoned accounts without `erasure_done` stayed at 0.
- **Live scratch run.**
  - Code: `scratchpad/c59/repo/src/share/c59_live.rs` and `c59_run.rs`, a copy of the crate. src/ was not edited.
  - Output: `scratchpad/c59/run_a.out`.
  - Passwords were made in-process and never printed.
  - Every block row I added (`by = 'critic59'`) was removed.
- **Local data left behind.** The test banks of the erased c59 accounts remain. One has an open `privacy_text_match` flag. I cleared its `withheld_at` and hold by hand for the S6 step below.

## The real B2 case (live, through `CirclesModel`): PASS

1. The author hides the keys from the page. The viewers' copy matches a block on a viewers'-form-only hash. The answer is `Done{held, withheld}`.
2. After the reload:
   - the load fails with `version_withheld`;
   - `hide_off_offered()` is true;
   - the page draws `HIDE_HELD_HEAD`, the switch and "Turn hiding off";
   - the "Only an administrator" box is not drawn.
3. Pressing "Close" on the result keeps the offer, from the circle list's `hide_key`.
4. Fresh models for the owner (not the author) and for the author also offer it. The viewer's list does not show the bank.
5. "Turn hiding off" gives `restored:[1]`. The page reads again (Ready), and the viewer reads the head with keys again.

## Blockers

### B1. "Turn hiding off" is offered, and said to help, for every withheld head of a hiding bank. It only helps when the answer-key PATCH did the withholding. When a send withheld the head, pressing it removes the protection, releases nothing, and leaves no way to turn hiding back on.

**Where**
- `src/share/circles.rs:2417-2421`: `hide_off_offered` = withheld head + bank hides keys + may change the setting.
- `circles_ui.rs:1106-1111` and `:1324`.
- The words, `circles_text.rs:53`: "if the version was held back because of the copy viewers get, turning hiding off makes it readable again."

**Server behaviour**
- `hideOff` (`circles.ts:1512-1519`, `ONLY_HIDE_FLAGS` `:1505`) restores a version only if every open flag on it says `via: 'hide_key'`.
- A send's flags carry no `via` (`circlesend.ts:1128-1140`).
- The send also scans the viewers' copy, so "held back because of the copy viewers get" is exactly the send case. The sentence is false there.

**Live repro (section S)**
1. A bank already hides its keys, cleanly.
2. The author sends v2. Its viewers' copy matches a viewers'-form-only block. The send answers `held:true, withheld:true`, and its flag is `privacy_text_match` with `via: null`.
3. The author's page shows `HIDE_HELD_HEAD` and "Turn hiding off". The author follows it: `Done{hide_key:false, restored:[]}`. The only line drawn is "Viewers see the answer keys again, in every version of this bank."
4. Reload: still `version_withheld`. Now `hide_key_on=false`, `hide_off_offered=false`, `hide_allowed=false`. The page shows only "Only an administrator can release it." The switch is gone, so hiding cannot be turned back on from the app.
5. The owner's list says `hide_key:false, held:true`.
6. After a (simulated) admin release, the viewer's read of the head has 3 `correct:true`. The author had asked for those keys to be hidden.

**Impact**
- The app's own advice removes the author's answer-key protection for nothing.
- The keys reach viewers as soon as the hold is lifted.
- The author cannot restore the setting until the release.
- `version_withheld` says nothing about the reason (`circles.ts:1286`), so the client cannot tell the two cases apart.

**Fix**
- Server: add a flag to the `version_withheld` answer for the owner and the author (e.g. `hide_release: true`). Set it when the bank hides keys and every open send flag on the head is `via: hide_key`.
- Client: offer "Turn hiding off" and `HIDE_HELD_HEAD` only when the flag is true. Until then, at most offer it right after this session's own `Done{withheld}`.
- Client: keep turning hiding back **on** reachable on a withheld head. `hideOn` handles `!headReadable`.
- Client: when a turn-off restored nothing and the head is still withheld, say so, not "Viewers see the answer keys again".

**Tests**
- Worker: a send that withholds a hiding bank's head answers `version_withheld` without the flag. A PATCH that withholds it answers with it.
- Client: `version_withheld` without the flag gives `hide_off_offered() == false`, and the page has no "Turn hiding off".
- Client: after a turn-off with `restored: []` while the reload still fails as withheld, the switch to turn hiding on is offered.
- Live: section S of `c59_run.rs`.

## Important

### I1. One refused older version hides every older one from the viewer, and the history labels readable versions as unopenable

**Where**
- `src/share/circle_edit.rs:313-314`: `closed_to_me` is `version <= keys_closed_from`.
- `:1926`: set from one `keys_hidden_unavailable` refusal.
- `circle_edit_ui.rs:800`: the line drawn.
- `circle_edit_tests.rs:2168` asserts this model ("version 2 and the older ones have no viewers' copy").

**Server behaviour.** Availability is not ordered by version. When hiding is turned on, `hideOn` (`circles.ts:1616-1637`) strips each older version with no decisions. A version refused that way (letter labels, an order acknowledgement) stays unrecorded (`continue`). Older and newer ones that pass are recorded and served.

**Live repro (section V)**
- v1: two plain questions. v2: adds "A. 4 / B. 7 / C. 9". v3: drops it. Then hide on.
- DB: v1 recorded, v2 not, v3 recorded.
- The viewer's GETs: v1 200 (keys hidden), v2 409, v3 200.
- In the viewer's history model, after opening v2: v1 is `can_look=false, closed_to_me=true`. It is no longer offered, and it carries "This version can not be shown without its answer keys, so viewers can not open it." That is false: the pool serves v1 to this viewer.

**Fix.** Record the refused versions as a set (exact numbers, not a threshold). Better: have the server mark each history line for a keys-hidden read (`strip_opts IS NOT NULL`), as critic 58 I3 suggested.

**Test.** History [3, 2, 1] and a 409 on 2: `can_look(1)` stays true and no line is drawn on 1. Turn around the in-repo test at `circle_edit_tests.rs:2168`.

## The three new wordings

1. `HIDE_HELD_HEAD` (`circles_text.rs:53`):
   - Inaccurate in the send case (B1).
   - It also asks the reader to know why the version was held, which the app never says.
   - No student-data, copyright or legal claim.
2. The per-domain hint (`circles_ui.rs:771`, `circles.rs` `email_blocker`): "Addresses at that mail domain can be invited again in about N minutes." / "... You can invite someone with an address at another mail domain now."
   - Accurate for both per-domain metrics.
   - Plain.
3. "This version can not be shown without its answer keys, so viewers can not open it." (`circle_edit_ui.rs:800`):
   - Accurate only for the refused version itself (I1).
   - It is said to a viewer in the third person, and "can not be shown without its answer keys" is roundabout.
   - Wording is the orchestrator's call. One candidate: "This version has no copy without answer keys, so you can not open it."

## Nits

- **N1** `circles.rs:1515`: a restarted Decide page keeps `head = base`. A restart now means the head moved after the client's check, so its findings are for the new head. Its send then always stops with "your decisions were dropped", after one counted 422. On `restarted`, re-read and drop the page, as `HideHeadMoved` does.
- **N2** `circles.rs:559-561`, `:1731-1735`:
  - Any head entry that is not `Current` at `base` reads as "A new version of this bank arrived meanwhile". That includes `removed`, `under_review` and `unavailable`, for example when the owner removed the bank or closed the circle while the author decided.
  - The pre-check (`POST /circles/heads`) charges `item_reads` (`circles.ts:1066`). A person at that cap cannot hide keys and is told about reads.
- **N3** A lost answer to a hide that withheld the head is retried and answered `unchanged`. The page then says "Nothing changed: the answer keys were already hidden..." (`circles_text.rs:246`), though that lost press made the change.
- **N4** `mail_domain` (`circles.rs:166`) uses `to_ascii_lowercase`. "ÉCOLE.example" and "école.example" count as two domains, while the pool counts one (punycode), so one extra 429 is possible.

## Checked, no new finding

- **c57 B1.**
  - Carried decisions are dropped on a restart.
  - The pool's echoed `decision` is never used.
  - The head is re-read before every PATCH with decisions and before the first PATCH to turn hiding on. A moved head sends nothing and drops the decisions.
  - What remains is a short race until the PATCH takes a `base_version`; the code comment says so.
- **c57 I1.** The decisions go only with the payload sha they were made for, and `back_to_note` clears them.
- **c57 I2.** `Failed` keeps the decisions page except on a conflict or invalid-field answer. "Try again" re-sends it through the head check. An `Offline` answer says it is not known whether the setting changed.
- **c57 I3.** The text edit ids are absolute (`.id(Id::new(..))`), so the removal matches. It runs before the Shared page draws and after the Circles tab draws.
- **c57 I4.** The wait is per domain and lives in the model (`email_blocker`). A non-domain wait still holds.
- **Keyless copy.** `circle_theirs_as(.., keys_hidden)` marks every question. Merges read the per-question `keyless` the pool sets.
- **c58 B1.**
  - The new statement matches `circle_email_domain:<hmac>:<id>` only, never the domain total.
  - The harness's `like()`/`glob()` shim answers with SQLite's own operator.
  - The scan covers `src/` and `migrations/`; `server/auth/src` has no LIKE/GLOB.
