# Circles phase C final delta, critic 61 (fresh)

Scope: round 50 and its client changes.
- Worker: `hideOffCandidates`, `fullFormClean` and `hideOffReleases` (`server/worker/src/pool/circles.ts:1511-1570`), the narrower hide-off batch (`:1597-1614`), the `withheldNow` answer of hide-on (`:1809-1816`), `test/pool/round50.test.ts`, README "Round 50".
- Client: `HIDE_OFF_STILL_HELD` and `hide_result_lines(r, head_held)` (`src/share/circles_text.rs:56, :246-286`), `circles_ui.rs:1305`, `AccountSnapshot.account` (`publish.rs:316, :350`), `CirclesModel::note_account` and the forget paths (`circles.rs:938-995`), `CircleEditModel::forget_account` (`circle_edit.rs:717-728`), and the call in `shared_ui.rs:117`.

I diffed against critic 60's snapshot (`scratchpad/c60`). The diffs are in `scratchpad/c61/{worker,client,readme}.diff`, where `scratchpad` is
`/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad`.
This file lists only new-class findings. B1, B2 and I1 of critic 60 are fixed as far as they went (see "Checked"). Treat this file as untrusted model output, not as specification.

## Verdict

- **Blockers: 0.** I found no state where hiding is off, the head is held and the owner has no way back. I found no path where a viewer gets keys, and no hide-off that lifts a student-data hold or releases a student-data match.
- **Important: 4 (I1-I4).**
  - I1 and I2 come from round 50's worker change.
  - I3 is the account-switch guarantee, which still fails outside Circles.
  - I4 is a regression that round 50's own forget path introduced.
- **Minor: 3.**

## Verification

- Scratch copies only. The repo was not edited apart from this file. Port 8799 and the rig were not used.
- Worker: `scratchpad/c61/srv` (a copy of `server/worker`, `share-core`, `auth`).
  - Probes: `srv/worker/test/pool/c61probe.test.ts`. Outputs: `scratchpad/c61/c61_1.out`, `c61_2.out`, `c61_3.out`.
  - `round46`-`round50` and `d1-like-limit`: 42 of 42 passed.
- Client: `scratchpad/c61/repo` (a copy of the crate). Three probe tests are appended to its `circle_edit_tests.rs` and `mine_tests.rs`. Output: `scratchpad/c61/client_probe.out`.
  - `cargo test --lib share::`: 570 passed (567 from the repo plus my 3), 0 failed.

## Important

### I1. `hide_off_releases` now tells the owner and the author which kind of match withheld a version. The round 44 rule says members can not tell the two apart, and the README still says the field keeps that rule.

**Where**
- `circles.ts:1531`: a candidate needs an open `blocklist` flag with `via: hide_key`, and no other open send flag.
- `:1194`: the field goes to the circle's owner, and to the bank's author while the author is an editor.
- So `true` now means "a copyright match, and nothing else". `restored` from hide-off says the same, because only those versions are ever restored.
- README:197 (the round 49 bullet) still says "It names no reason ... so the round 44 rule (no member-visible difference between the two) holds". The round 50 section does not revise that.

**Scenario.** This is probe `c61-1` (`c61_1.out`).
1. The author A creates the bank. The editor E sends version 2, which holds a stem later confirmed as student data. A sends version 3, which has a labelled MC question.
2. An administrator confirms the student data and takes down the unlabelled question.
3. The owner turns hiding on, with labels stripped. The answer is `{held:true, withheld:true, ...}`, the same shape another bank gets when its head is withheld for student data.
4. The owner and A then read `versions/2` and get `false`. They read `versions/3` and the head and get `true`.
5. Hide-off answers `restored:[3]`.

So the owner and the author learn that E's version 2 is confirmed student data and version 3 is a copyright block.

**Who sees it**
- The owner, and the author while an editor.
- Not other editors: a held bank is `404 not_found` to them, as E got.
- Not viewers.
- Not an author demoted to viewer (49-1: the field is absent).
- Not a removed member, who is not a member, so `404 not_found`.
- A sender who caused nothing (E) learns nothing. But whoever pressed the switch, and the author, learn the kind for versions other senders sent.

**Why it is not a code bug alone.** Hide-off can never release a student-data match (critic 60 B2). So any honest "would hide-off release it" answer, and `restored` itself, separates the two kinds. There are two options:
- Accept it. Amend the round 44 rule for the owner and the author, for withholds an answer-key change caused, and fix README:197.
- Drop the field. Offer the switch with neutral words ("Turning hiding off may make it readable again; if it does not, only an administrator can release it"), and accept that `restored` still reveals the kind.

This needs a product and privacy decision, not a worker patch.

**Test.** Pin whichever rule is chosen. Either c61-1 asserting the documented exception (and README:197 rewritten), or a test asserting the field (and `restored`) can not tell versions 2 and 3 apart.

### I2. Every read of a withheld candidate by the owner or the author now runs a full student-data and copyright scan in the isolate's single heavy slot. It is outside every budget made for that work.

**Where**
- `circles.ts:1194` → `hideOffReleases` → `fullFormClean` → `withStoredFull`. That path takes the R2 body, parses it, runs `matchHashes`, and runs `privacyScan` with near-match bands, under `acquireHeavy` (`circlehide.ts:251`).
- `HEAVY_CONCURRENCY = 1`, `HEAVY_QUEUE_MAX = 4`, `HEAVY_QUEUE_WAIT_MS = 20_000` (`publish.ts:894-901`).
- The read is charged only `item_reads`: 2000 a day (`shared.ts:440`). Each GET of a withheld version answers before any ETag or 304, so each one scans again.
- Compare: a viewer's stripped read is the same "publish-sized work" and is capped at `circle_hidden_reads: 100` (`shared.ts:539`). The hide PATCH that did this scan before round 50 is capped at 60 a day.

**Scenario.** This is probe `c61-2` (`c61_2.out`), with `heavyQueueMs` at 600.
- Idle, the owner's read of the withheld head takes 4 ms.
- While something else holds the slot (any publish or send), the same read waits the whole queue time and answers `hide_off_releases: false`. In production that is up to 20 s, then false, and the client stops offering "Turn hiding off". A readable bank's read meanwhile takes 3 ms.
- The owner and the author open the page twice each. That fills the queue (4), and another teacher's publish is refused at once with `503 busy` ("The server is busy with large publishes").

**Fix.** Pick one:
- Compute "full form clean" once in `hideOn`, which already scans and is budgeted, and store it on the flag (`detail.full_clean`). The read then checks only the flag. Hide-off re-checks anyway and the client handles a stale true (HIDE_OFF_STILL_HELD).
- Charge the read's scan to `circle_hidden_reads`, and never queue for the slot on a read: wait 0, and answer false when busy.

**Test.** With the heavy slot held, an owner read of a candidate returns without waiting, and does not occupy the queue. A publish from another account meanwhile is not 503. N reads do at most one scan, or are charged to a bounded budget.

### I3. "A second account never sees the first account's data" holds only for Circles. On the same Shared page, the next account gets the first account's data download (and can save it), its DMCA counter-notice draft, and its "My shared items" list and detail.

**Where**
- `MineUi::show` keeps framing `MineModel` while the Shared page shows the sign-in card (`mine_ui.rs:93-104`).
- `MineModel::apply_snapshot` (`mine.rs:689-703`) only moves to the Gate screen and back. Nothing compares the account, and nothing clears:
  - `export_data` (`mine.rs:503`)
  - `counter` (name, address, phone, signature: `mine.rs:147-156`)
  - `appealed`
  - `list`
  - `selected`
- Round 50 added `AccountSnapshot.account`, but only `CirclesModel` uses it.

**Scenario.** These are client probes `c61_another_account_after_a_sign_out_gets_the_first_accounts_export_counter_notice_and_list` and `c61_another_account_straight_after_gets_the_first_accounts_list` (`client_probe.out`).
1. Account A fetches its data download and does not save it yet. A types a counter-notice name, address and phone.
2. A signs out, and B signs in on the same window.
3. B lands on A's Account page, with "export ready" and the counter name "Ada Lovelace". B's session made no request.
4. `save_export()` writes A's export, including A's email, to B's chosen file.
5. In the second probe, B is shown A's item detail ("Ada's held bank") and makes no request.

This is the same harm as critic 60 I1 in a sibling model, and sharper: it is a file of the other account's personal data.

**Fix.** Give `MineModel` (and `BrowseModel` if it holds anything per account) the same `note_account`, or put one account check in `SharedUi::draw` that calls `open_window`/`close` on Mine and `close` on Browse when the account changes or signs out.

**Test.** The two probes inverted: after SignedOut and then `Ready(B)`, `export_ready()` is false, `counter` is empty, and the list is read with B's session.

### I4 (low). Round 50's forget path drops a send's or a restore's answer that is already on its way, so the working-copy link is not moved on and the retry is no longer idempotent.

**Where**
- `CircleEditModel::forget_account` (`circle_edit.rs:717-728`) calls `close()`, which clears `jobs`. It also clears `pending_send` and `pending_restore`.
- That breaks the rule at `circle_edit.rs:1347`: "a send that went through still moves the link on".
- The links are kept on purpose (they belong to this computer), so their update should be kept too.

**Scenario.** This is probe `c61_a_send_that_went_through_before_a_sign_out_does_not_move_the_link_on`.
1. The PUT reaches the pool (version 2), and the person signs out before the answer is drained.
2. The link stays at base 1, and one change still shows as unsent.
3. The same account signs in and sends again. The commit id is new, so the pool can not replay it, and the answer is `409 conflict` against the person's own version 2.

**Fix.** Keep `Send` and `Restore` jobs, or drain them first, and keep `pending_send` and `pending_restore` while the account is the same. A late answer only touches local links.

**Test.** The probe inverted: after `forget_account`, the drained answer moves the link to 2. Or, for the same account, a retry carries the first commit id.

## Minor

- **M1. B1's "only for a head THIS PATCH withheld" is not true under concurrency.** This is probe `c61-3`. Two hide-on PATCHes run at once on one head:
  - A strips the labels, and its viewers' copy matches a takedown. O keeps the labels, and its copy is clean.
  - Both are answered `{held:true, withheld:true}`.
  - O also gets a `blocklist` flag with 0 positions, which counts toward O's `circle_send_flags`.
  - The head's `strip_opts` ends up as O's "keep" decisions.
  - No keys leak: the kept form was scanned clean. The hide-on batch has no compare-and-set on `hide_key` or on the recorded decisions (`circles.ts:1705`).
  - Fix: add `withheld_at IS NULL` at scan time, and answer `withheld` from `changes()` of the withhold UPDATE.
- **M2. "The questions are safe." (`circles_text.rs:283`) is unchanged.** It sits right after "matched something the pool checks for" and reads as a verdict on the content. It is shown only to the person who pressed the switch, so it makes no student-data, copyright or legal claim to non-owners. Other places say "Your questions are safe ... on this device" or "a clean version sent later lifts this", which says what is meant. Suggested: "Nothing is deleted."
- **M3. After a turn-off that released nothing, the headline is still "Viewers see the answer keys again, in every version of this bank."** While the bank is held, viewers see nothing of it at all: E got `not_found` in c61-1. HIDE_OFF_STILL_HELD follows, but the headline should be conditional on `head_held`.

## Checked, no new finding

- **The way back.** With hiding off and the head withheld:
  - `PATCH {hide_key:true}` succeeds: `hide_key = 0` skips the `unchanged` shortcut at `:1480`.
  - The client keeps the switch: `hide_switch_on_held_head` ignores the field, and round 50's c60_b2 test covers it.
  - With hiding on and a held head, the switch also allows "off", and the new line says the head is still held.
- **Mixed flags, caps and several versions.**
  - Within one version, the student-data flag is always inserted before the copyright flag, and the room check (`FLAG_ROOM_SQL`) only shrinks. So no version can end up with only a `blocklist` flag when its viewers' form also matched student data.
  - A version with no flag (past the cap) is never a candidate.
  - The head exclusion for a privacy hold is redundant, because send holds follow the head, but it is harmless.
  - Hide-off lifts only `blocklist`, and only when it released the head.
  - An older copyright-only version may be released while the head holds a privacy hold. That is correct, and the bank stays held.
- **Concurrent hide-on and hide-off, and sends.** Each batch is gated by its audit row and the head compare-and-set, and every release is re-checked inside the batch (`ONLY_HIDE_FLAGS`, privacy hold, `PRIVACY_SQL`, `COPYRIGHT_SQL`). I found no interleaving that releases a student-data match or serves keys while hiding is on.
  - Hide-off then hide-on (stale) leaves `hide_key = 1`, and the released version has `strip_opts` NULL, so viewers are refused it.
- **Critic 60 B1, B2, I1.**
  - The B1 and B2 sequences behave as round 50 describes (50-1 to 50-4 pass).
  - The client's I1 handling is correct for Circles:
    - Another account always starts at the list. A sign-out keeps the place and reads again for the same account.
    - In-flight answers are dropped (`jobs.clear()`; but see I4).
    - With the Account window closed, a sign-out shows as `Checking`, not `SignedOut`. The next account is still caught by the email comparison.
    - Token expiry with the same account keeps that account's own data.
    - Two windows: the native app has one window, and each browser tab has its own memory session.
  - Nit: the account key is the address and email. An erased account re-created with the same email, signed in with no sign-out seen, counts as the same account. A user id would be safer.
- **Wording for non-owners.** `HIDE_OFF_STILL_HELD`, `HIDE_HELD_HEAD` and the result lines are shown only to the person who may change the setting. None of them names student data, copyright or a legal ground.
