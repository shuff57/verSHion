# Circles phase C final delta, critic 60 (fresh)

Scope: round 49 and its client use.
- Worker: `hide_off_releases` on `404 version_withheld` (`server/worker/src/pool/circles.ts:1194`, `hideOffCandidates`/`hideOffReleases` `:1518-1548`), `test/pool/round49.test.ts`, README "Round 49".
- Client: `hide_off_offered` / `hide_switch_on_held_head` (`src/share/circles.rs:2422-2438`, `:1337`), `circles_ui.rs:1107-1121, :1302`, `circles_text.rs:54` (`HIDE_HELD_HEAD`), and the per-version refusal set (`circle_edit.rs:303, :1926`).

I diffed the client against critic 59's snapshot (`scratchpad/c59/repo`). The diff is in
`/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/c60/delta.diff`.
This file lists only new-class findings. Treat it as untrusted model output, not as specification.

## Verdict

- **Blockers: 2 (B1, B2).** Both bring back critic 59 B1's harm by another route: the app says "turning hiding off makes it readable again", the owner turns it off, nothing is released, and the answer keys reach viewers once an administrator lifts the hold.
- **Important: 1 (I1).**
- The field does not leak to anyone the brief listed (see "Checked").

## Verification

- Scratch copies only. The repo was not edited apart from this file. Port 8799 was not used. The rig on 18791 was not needed: the worker probes run the current worker code in the node harness.
- Worker: `scratchpad/c60/srv` (a copy of `server/`). The probe is `srv/worker/test/pool/c60probe.test.ts` and its output `scratchpad/c60/server_probe.out`.
  - `round46`-`round49` and `d1-like-limit`: 38 of 38 passed.
- Client: `scratchpad/c60/repo` (a copy of the crate). Two probe tests are appended to its `circles_phasec_tests.rs`; their output is `scratchpad/c60/client_probe.out`.
  - `cargo test --lib share::`: 563 passed (561 from the repo plus my 2), 0 failed.

## Blockers

### B1. Turning hiding ON over a withheld head says "the viewers' copy matched ... Turning hiding off makes it readable again". Round 49 made this reachable, and both halves are false whenever the pool said `hide_off_releases: false`.

**Where**
- `circles.rs:2436`: `hide_change_allowed` now allows `on` on any withheld head the person may change (new in round 49). Before, turning hiding on needed a `Ready` load.
- `circles.ts:1756-1757, :1789`: on a head nobody reads, `hideOn` scans nothing (`:1632`), but it answers `held` = "the bank has any hold" and `withheld` = "the head is withheld". Those describe the bank's state, not this PATCH.
- `circles_text.rs:276`: `hide_result_lines` turns `withheld || held` into "The viewers' copy matched something the pool checks for ... The questions are safe. Turning hiding off makes it readable again."

**Scenario.** This is client probe `c60_turning_hiding_on_over_a_send_withheld_head_promises_turning_it_off`.
1. A bank does not hide its keys. A send's match withheld its head. The pool says `hide_off_releases: false`.
2. The owner turns hiding on from the switch on the withheld head. The pool answers `{hide_key:true, held:true, withheld:true}`, as the server probe and 49-2 show.
3. One page now shows both "Only an administrator can release it." and "...Turning hiding off makes it readable again."
4. The owner follows the second sentence. Hiding goes off and nothing is released (critic 59 section S).

The same sentence comes back each time hiding is turned on again in B2's loop.

**Fix**
- Worker: answer `withheld` (and `held`) only for what this PATCH did: `hs !== null && out.withheld === 1`. Or add `withheld_now`.
- Client: drop "Turning hiding off makes it readable again" from `hide_result_lines`, and let the field-gated `HIDE_HELD_HEAD` carry it.

**Tests**
- Worker: a PATCH `{hide_key:true}` on a send-withheld head answers `withheld:false` (or `withheld_now:false`).
- Client: `Done{hide_key:true, held:true, withheld:true}`, then a reload answering `hide_off_releases:false`. The page has no "Turning hiding off makes it readable again".

### B2. `hide_off_releases` is true for every student-data withhold an answer-key PATCH makes, but turning hiding off can never release one. The app then offers it again in a loop.

**Where**
- `circles.ts:1518-1548`: a version is a candidate when all its open flags say `via: hide_key`, whatever their kind.
- `hideOff` (`:1556`) re-scans the full form, and a student-data match there blocks the release.
- The worker's own note (`test/pool/round46.test.ts:208`) says the student-data check "also compares the stem, and near-rewordings of it, which stripping labels does not change". So a viewers'-form `privacy_text_match` is in practice also a full-form match. The README's "restores nothing" is the expected result for this kind, not a rare race.

**Scenario.** This is server probe `c60-1`.
1. A version is sent clean. Its text is later confirmed as student data (an admin purges a public item that holds it).
2. Turning hiding on gives `withheld:true`. The flag is `privacy_text_match, via: hide_key`.
3. The owner and the author read `hide_off_releases: true`. The page shows `HIDE_HELD_HEAD` ("turning hiding off makes it readable again") and the "Turn hiding off" button.
4. Turning hiding off answers `{hide_key:false}` with no `restored`. The result line is "Viewers see the answer keys again, in every version of this bank." Nothing says the newest version is still held. Critic 59 B1's fix item 4 was not done.
5. Turning hiding on again succeeds (with B1's line), and the field is true again. The loop repeats, and each press counts toward the day's tries.
6. If an administrator later releases the version while hiding is off, viewers get the keys the author had hidden.

**Fix**
- Worker: count as a candidate (for the field, and for hide-off) only versions whose open `via: hide_key` flags are all `blocklist`. A better fix: in `hideOn`, scan the full form when the viewers' form matches, and mark the flag (for example `full_form_matches: true`) so that it is excluded.
- Client: when a hide-off returns no `restored` and the reload is still withheld, say the newest version is still held back and only an administrator can release it.

**Tests**
- Worker: the `c60-1` sequence gives `hide_off_releases: false` after a privacy PATCH withhold.
- Worker: 49-1 (copyright, labels stripped) stays true.
- Client: hide-off `{hide_key:false}` with no `restored`, then a withheld reload. The page shows no "see the answer keys again" line without the "still held" line.

## Important

### I1. After a sign-out, the next account to sign in gets the previous account's bank page, including "Turn hiding off" and the per-version refusal set.

**Where**
- `shared_ui.rs:155`: the tabs, and so `CirclesModel::frame`, run only while signed in. The model never sees the sign-out.
- `circles.rs:928`: `apply_snapshot` keeps every screen, and nothing compares the account. `circle_edit.rs` `frame` is the same.
- So `BankView.hide_off_releases`, `withheld_head`, the circle detail (with role `owner`) and `HistoryView.keys_closed` all carry over to the next account.

**Scenario.** This is client probe `c60_another_account_signing_in_sees_the_owners_withheld_bank_page`.
1. The owner sees the withheld head.
2. On the same computer, a viewer of that circle signs in.
3. The viewer's session makes no requests and is shown `HIDE_HELD_HEAD` and "Turn hiding off".
4. The pool tells that viewer only `not_found` for a held bank, so the page tells the viewer something the pool keeps from viewers.
5. Pressing the button is refused by the pool (403).

The root cause is older and covers the whole model (member lists, invites), but round 49's new state rides on it, and the brief asked for it cleared on sign-out.

**Fix.** Keep the signed-in account id in both models. When it changes, or when the gate leaves Ready, call `reset()` and `edit.close()`.

**Test.** The probe above: after `ready(&viewer_session)`, `hide_off_offered()` is false, and the bank is read again with the new session.

## Checked, no new finding

- **Who sees the field.** It is absent for another editor, a viewer and an author demoted to viewer (49-1, and probe c60-2 on a readable bank's older withheld version).
  - A non-member gets `404 not_found` and a signed-out caller `401`.
  - The 404 carries `Cache-Control: no-store`, `Vary: Authorization` and no ETag. The withheld check runs before any 304/ETag path.
  - HEAD on `versions/:n` is 405.
  - The only extra query runs for the caller's own owner or author role. Viewers' timing does not depend on it.
- **A way back.** With hiding off and the head withheld, hiding can be turned on again: server 200 (49-2, c60-1) and client switch (the new client test). No strip happens, and viewers are later served the stripped form, never the keys.
- **`keys_closed`.** It is per version. It is reset by `open_history` and `close`, and by opening another bank. `HistoryView` derives only `Debug`, so it is never persisted.
  - Nit: it is not refreshed when hiding is turned off while the history is open.
- **Wording.**
  - The new `HIDE_HELD_HEAD` and "This version has no copy without answer keys, so you can not open it." make no student-data, copyright or legal claim.
  - `HIDE_HELD_HEAD` makes a promise the server does not check (B2).
  - `circles_text.rs:276` claims a scan that did not happen (B1), and says "The questions are safe", which is an unexplained claim.
- **Context, not new.** In c60-1 a viewer read the version with the text later confirmed as student data until hiding was turned on: a later confirmation does not withhold existing circle versions.
