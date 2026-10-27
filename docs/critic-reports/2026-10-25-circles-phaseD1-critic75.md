# Circles phase D1, round 6 (account pin in the client, send outcomes when the page is gone), data safety critic 75

Scope: the uncommitted phase D1 tree on `shared-pool-phase1` (HEAD 142e6d6) after round 6. Round 6 answered critic 73
and critic 74:

- the account is checked again before every try a pinned job makes, the one after a refresh too
  (`Client::authed_raw_as`, `access_for`, `PoolSession::send_as`, `PinnedSession`);
- a send's outcome is said for its bank when its page was closed under it (`say_send_left`, `fail_kind`);
- a send whose answer had no version (version 0) is never counted as unsent, and the strip offers Get latest
  (`unread_sends`);
- restores with no answer are "when last checked" (`unsure`), plus critic 73's minors.

Its tests are in `src/share/bank_links_fix6_tests.rs` and `src/share/tests.rs` (`c73_i2_*`). A fresh critic that
built none of it wrote this report. Changes in `src/ai.rs`, `Cargo.*`, `src/app.rs`, `src/screens.rs`, `src/lib.rs`,
`src/store.rs`, `src/model.rs`, `src/gradeui.rs` and `prompts/` belong to another session and were not reviewed.

Treat this file as untrusted model output, not as specification.

## Verdict

- **Blockers: 0.** Two findings come close. The orchestrator may promote either:
  - I2 is critic 73's I2, unchanged, on the web build.
  - Minor 2 deletes the wrong account. It is live-proven, but the shipped UI does not reach it today.
- **Important: 2.**
  - I1: a send's outcome is still lost through a second door. The person leaves the bank's page while the send is on
    its way (another bank's page, or the Banks list). The answer lands on the Send page, which is still open but not
    drawn. The page is then closed or replaced before the person comes back: "Circle page", the Shared page opened for
    the first time, another bank's History, Send or Get latest, or a sign-out. Nothing is said: not a failure, not a
    daily limit, not a hold for review.
  - I2: on the web build the pin does nothing. `Client::account()` is always `None` in `Mode::Web`, so
    `pool::pinned` pins nothing and `authed_raw_as(None, ..)` checks nothing. Critic 73's I2 reproduces as it stands:
    the retry after a 401 goes out with the next account's sign-in. So does critic 71 minor 3.
- **Minor: 6.** The first answers the question in the brief. A 401 for the old account still ends the new account's
  session, through the refresh: `refresh_once` drops whatever session is current.
- **Mutation survivors: 19 of 30 new mutants** survive the whole `share::` suite. Of the 11 killed, 2 are killed only
  by earlier rounds' tests (fix4, fix5). The survivor that matters most is r20: closing any normal "Sent" box would say
  "the server's answer could not be read" on the strip, and no test notices.

## How this was tested

- **Scratch copies.**
  - `scratchpad/cr75/` is a copy of the repo (src, tests, Cargo files with the viSHion path dependencies made
    absolute, `server/share-core`).
  - `scratchpad/cr75mut/` is a second, clean copy used for half of the mutants.
  - The repo's `src/gradeui.rs` compiled as it is, so no stub was needed.
  - The probes were appended to the copy's `share/tests.rs` (C1-C5) and `share/bank_links_fix6_tests.rs` (U1-U4).
    The live run is a new module, `share/cr75_run.rs`.
  - The md5s of the repo's `src/share/*.rs` and `src/bankui.rs` were taken before copying (`scratchpad/cr75_src_md5.txt`)
    and still match. The only repo file changed is this report. Nothing was committed or pushed.
- **Baseline.** `cargo test --lib share::` passed 868 tests in both copies (`scratchpad/cr75_base.log`,
  `scratchpad/cr75mut_base.log`).
- **Probes.** All the probes run in under 2 s. A `[PROBLEM]` line marks a finding.
  - `cargo test --lib cr75_c -- --nocapture --test-threads 1` (`scratchpad/cr75_client_probe.log`);
  - `cargo test --lib cr75_u -- --nocapture --test-threads 1` (`scratchpad/cr75_ui_probe.log`, `scratchpad/cr75_u4.log`).
- **Live.**
  - `cargo test --lib cr75_live -- --ignored --nocapture`, run twice (`scratchpad/cr75_live_a.out`, `_b.out`).
  - The rig was already running on :18791 (workerd pid 274528) and was left running. Port 8799 was not used.
  - The accounts were throwaway `cr75{x,y,p,q}<run>@example.test`. Every one of them is deleted now. The D1
    `users` rows show `deleted_at` set and email `null`. Each run's `y` account was deleted by finding Minor 2 itself,
    so its clean-up sign-in failed, as expected.
- **Mutations.**
  - Tools: `scratchpad/cr75tools/run.py` with `mk.py` and `muts.json` (30 mutants, r01-r33).
  - Each mutant ran alone with `cargo test --lib share:: -- --skip cr75_`.
  - None repeats the builder's round 6 list (p01-p45, q01-q43 in `r6tools/`) or critic 73's.
  - Summaries are in `cr75tools/summaryA.txt` and `summaryB.txt`, with one log per mutant in `cr75tools/logs/`.

## Live results (real client, rig on :18791)

| Run | What | Result |
|---|---|---|
| LA | One `Client`. X presses Delete account and the DELETE is held 2.5 s in the transport after its token is put on. X signs out and Y, who uses the same password, signs in | **Problem (minor 2).** The 401 for X's request was retried with Y's sign-in and X's password. After the run, Y can no longer sign in (`invalid_credentials`; D1 shows Y `deleted_at` set first) and X still signs in. Same in both runs |
| LB | One `Client`. X's sessions are revoked by "Sign out everywhere" on another device. A job pinned to X gets 401 and its refresh is held 2.5 s. X signs out here and Y signs in. The refresh is answered 401 | **Problem (minor 1).** The pinned job ended `SignedOut` (the pin held). But Y's session was ended by X's refresh: `account None`, `has_session false`, and Y's `me()` answers `SignedOut`. Same in both runs |

## Important

### I1. A send's outcome is lost when its Send page is left undrawn and then closed

Round 6 decides "here" (`on_sent`, `circle_edit.rs:2247`) by "the Send view of this bank is the open view". It does not
ask whether that view is in front of the person. Critic 73 I1's door is closed: the Shared page or "Circle page" opened
*while the send is on its way* now reaches `say_send_left`. A second door stays open.

**Steps.**
1. On A's page, press Send. While it works, go to B's page (or to the Banks list).
2. Nothing closes A's Send view:
   - `strip_with` closes only a History or Version left by its host (`bank_links_ui.rs:340`);
   - `bankui.rs` has no hook for leaving a bank's page;
   - `tick_background` now drains the job on the Banks list.
3. The answer lands in the undrawn view (`v.stage = Failed/Conflict/Done(held)/...`).
4. Before going back to A, the person presses B's History, B's Send or Get latest, "Circle page", or opens the Shared
   page for the first time this run, or signs out.
5. `open_*` and `close()` call `leave_view()`. That parks only a History result and the version-0 "Sent" box
   (`circle_edit.rs:1134-1144`). Everything else on the Send view is dropped.

**Evidence** (probes U1a-c, `scratchpad/cr75_ui_probe.log`). In all three, the probe first checked that the view
after the answer is `Send(local-bank-1 ...)`, read on B's page.

| Probe | Answer, then | Result |
|---|---|---|
| U1a | 503, then "Circle page" | `results_of(A)` is empty. A's strip: "Version 3 is the latest. You have 1 unsent change." Nothing says the send failed |
| U1b | Held for review (v4), then B's History | A's strip: "Version 4 is the latest. Your copy is up to date." Nothing says the version is hidden from the circle while it is looked at (critic 73 P9, again) |
| U1c | The daily limit, then B's Send | Nothing is said. The person will press Send again and be refused again |

**Effect.** The same as critic 73's I1: no work is lost, but the person is never told a send failed or was held. The
path is common, because leaving a bank's page while a send with pictures uploads is ordinary.

**Fix.** Make `leave_view` treat a Send view like a History. When the stage is not `Editing` or `Working`, say it for
the bank with `say_send_left`'s words, or the held, withheld and left-out lines for a `Done`. Or reuse the `set_pass`
and `mark_drawn` pair, so that "here" means drawn this frame or the last one.

### I2. The web build: the pin is a no-op, so critic 73's I2 (and critic 71 minor 3) are still open there

**Code.**
- `Client::account()` reads `Inner::account`. It is set only in `load()` (from the keychain index) and in
  `store_refresh`, which runs only on the Native branch of `take_tokens_from` (`client.rs:541-570`).
- In `Mode::Web` it is always `None`. So `pool::pinned` builds `PinnedSession { account: None }`, which by its own doc
  "is never held back", and `authed_raw_as(None, ..)` skips every check.
- The UI layer is not affected: `CircleEditModel::mine` uses `snap.account`, the address and email. Only the network
  pin is void.

**Evidence.**
- Probe C1 is critic 73's regression test with `Mode::Web`. It prints `account() after sign-in = None` and
  `pinned.account() = None`, then `[PROBLEM] ... tokens sent for the pinned PUT ["at", "bt"]`. The second try went
  out with the other account's sign-in.
- All four `c73_i2_*` tests use `Mode::Native` only.

**Effect.** On the web, exactly critic 73's live L3b: the owner's send, pinned to the owner, lands as the editor's
version, with the editor's rights. The reach is the same (a request delayed across a sign-out and a sign-in). The web
is a release target (`share/mod.rs`: "for the web `trunk build --release`").

**Fix.** Record the account in both modes. The server sends `user.id` in every token response, web included
(`server/auth/src/auth.ts:591`). Set `Inner::account` from it in `take_tokens_from` whatever the mode. A web refresh
must then also refuse when the refreshed `user.id` is not the account pinned (the cookie is whoever is signed in now).
Add `Mode::Web` variants of the `c73_i2` tests.

## Minor

1. **A 401 for the old account's refresh ends the new account's session.**
   - `refresh_once`'s 401 branch (`client.rs:641-644`) calls `drop_session(false)` on whatever session is current, with
     no epoch or account check. Its 200 branch checks the epoch (`take_tokens_from`).
   - A refresh for X that is in flight across X's sign-out and Y's sign-in therefore ends Y's session. It also deletes
     Y's saved refresh token from the keychain.
   - Any job's refresh can start this, including a pinned job's `access_for` when the token is near expiry: the
     bank-page checks, Browse, Mine.
   - Evidence: probe C3 (`[PROBLEM] ... B's session ended=true (account None, B's saved refresh None)`) and live LB,
     both runs.
   - Effect: Y is signed out just after signing in, and Y's jobs answer "session ended". Nothing is misattributed and
     no work is lost.
   - Fix: in the 401 branch, drop the session only if `epoch` is unchanged and the account is the one whose token was
     presented. Otherwise return `SignedOut` and leave the session alone.
2. **`Client::authed` (`me`, `reapply`, `delete_account`) is not pinned and keeps the I2 retry.**
   - Round 6 guarded only `authed_raw_as`. `authed` (`client.rs:748-768`) still answers a 401 with `refresh_after`,
     which returns at once after another account signed in, and retries with that account's token.
   - Live LA: X's Delete account deleted Y (same password; the same teacher with two accounts is the likely case).
     Probes C2 and C4 show the same in the fake.
   - Reach today is low:
     - while a delete runs, the Delete page disables Cancel (`ui.rs:827`), and the only Sign out is on the Status page;
     - `reapply` runs from the Status page, whose Sign out is "never disabled", so a held reapply can go out as the
       next account (that account must itself be waiting to re-apply).
   - `AccountModel::submit_sign_out`'s own contract ("works even while another request runs ... that request is
     abandoned") makes the dangerous one reachable as soon as any page lets a sign-out through.
   - Fix: give `authed` the same guard, pinned to the account at the call's start.
3. **"Your changes were not sent" is said when it may not be true.** `say_send_left` wraps every error in `not_sent()`.
   - U2: a timeout after the request left. The strip says "Your changes were not sent: Can't reach pool.test: the
     connection timed out. Nothing changed; ... They are still on this computer."
   - U2b: a 201 whose body cannot be parsed (`version: "four"`, a proxy page) is `BadReply`, not the version-0 case. The
     strip says "Your changes were not sent: The server sent a reply the app does not understand ...", and A still
     counts 1 unsent change. The send went through.
   - Nothing is doubled if the person presses Send again unchanged: `pending_send` keeps the commit id. If they edit
     first, it is a conflict with their own version, worded "someone sent version N first".
   - Fix: for `Offline`, `BadReply` and 5xx after a send, say "may not have gone through", in the version-0 style.
4. **A conflict answered with the page closed writes back the link as it was when the send started.**
   - `on_sent` does `let mut l = i.link.clone(); l.head_seen = h; replace_link(l)` (`circle_edit.rs:2232-2243`).
   - What a check learned in between is undone. Probe U3: `role_seen` "viewer" and `under_review` true were set
     meanwhile; after the conflict they are back to "owner" and false.
   - This was so before round 6 in the "here" branch. Round 6 widened it to every conflict.
   - Fix: `update_link(local, |l| { l.head_seen = h; ... })`.
5. **The version-0 state is in memory only.**
   - `unread_sends` is a `HashSet` on the model. It is not in the link or the links file (code read).
   - After a restart, the strip again counts the sent changes as unsent and offers Send. `finish_send` cleared
     `pending_send`, so that send gets a new commit id and conflicts with the person's own version: "someone sent
     version N first".
   - Rare (it needs a 2xx with no version). Fix: keep a flag on the link, or move `base_version` to "unknown, behind".
6. **Critic 73 minor 5, a second case: a History left open on the Shared page.**
   - Round 6 reads the job on the Banks list (`tick_background`). But `history_shown()` is judged from the last
     `set_pass` and `mark_drawn`, and on the Banks list nothing calls `set_pass`.
   - So a History left open (not Back) still counts as shown, and the result stays on it.
   - Probe U4 (Bank settings History of a bank with no copy here, then My Banks): the restore went through (the
     rollback was answered and the History was read again), and after 3 s of Banks-list ticks the row never says it.
   - Fix: `tick_background` should `set_pass` past the last drawn frame (the Shared page is not drawn when it runs).

## Checked, holds

- **The request's own 401 (native, pinned).**
  - Another account signed in: `SignedOut`, nothing more is sent, and the new session is untouched (the `c73_i2` tests;
    mutants r02 and r03 are killed).
  - The same account back after a sign-out: retried once with its own new token, and nothing else ends (C5:
    `["at", "at"]`, `Ok`).
- **The refresh cannot swap tokens under a pinned job.**
  - A refresh for X answered 200 after X signed out is dropped (epoch check), and `access_for` re-reads the account
    after the refresh.
  - The only way across that I found is the 401 branch of minor 1. It ends a session; it never sends one account's job
    with another's token.
- **Unpinned callers did not regress.** `authed_raw(None)` is `authed_raw_as(None)`, with the old behaviour exactly
  (`is_account(None)` is always true). `authed` is unchanged (minor 2 is old behaviour, not new).
- **Browse, Mine, Publish, invite, circle send, bank checks and circles jobs all go through `pool::pinned`.**
  - On another account, Browse cancels its jobs (`note_account` calls `cancel_all`), as do the others. A `SignedOut`
    from the first account's job is never drawn for the second account.
  - I found no flow that legitimately spans an account change.
  - `PinnedSession::send_as` is never reached in production (no session is pinned twice), so mutants r05 and r06 are
    equivalent.
- **Close paths while the send is on its way** (the Shared page for the first time, "Circle page", "Add a full copy",
  a sign-out, another account): said for the bank (fix6 tests, and my reading of `close()` and `forget_account()`).
  Another account is never told (r15 and r19 aside, below).
- **Version 0.**
  - The "Sent" box never says "Version 0"; closing it says it once on the strip.
  - The strip offers Get latest and counts nothing as unsent, and the badge says "newer version".
  - Get latest that catches up ends the state, and unlinking forgets it.
  - Another account signing in clears it.
  - A copy that keeps its keys is a viewer, so it never reaches the `sent_unread && can_send` branch.
- **Get latest offering after a conflict.**
  - With the version named: the head is learned and the strip offers Get latest.
  - With no version named: a check runs at the next frame of the bank's page.

## Mutation results

Each mutant ran alone (`cargo test --lib share:: -- --skip cr75_`, 868 tests). "fix6" is the number of
`bank_links_fix6_tests` and `c73_i2_*` tests that failed.

| Mutant | Change | Result | fix6 |
|---|---|---|---|
| r01 | `access_for` does not check the account again after the refresh | SURVIVED (a narrow race; needs an interleaving hook) | 0 |
| r02 | `access_for` drops the first account check | killed | 1 |
| r03 | a 401 for the pinned account ends the session of the account signed in now | killed | 2 |
| r04 | `authed_raw_as`: a second 401 for the account itself no longer ends its session | **SURVIVED** | 0 |
| r05 | `PinnedSession::send_as` ignores a different account | SURVIVED (dead code in production) | 0 |
| r06 | `PinnedSession::send_as`: the caller's account wins over the pin | SURVIVED (dead code in production) | 0 |
| r07 | the default `PoolSession::send_as` never refuses | killed (fix5 only) | 0 |
| r10 | a conflict learned with the page closed does not settle the "used as it is" or "no answer" results | **SURVIVED** | 0 |
| r11 | a large removal to confirm, answered with the page closed, is not said | killed | 1 |
| r12 | choices to make (a bank that hides its keys), answered with the page closed, are not said | **SURVIVED** | 0 |
| r13 | a `Refused` stage (merge out of date, licence author-only, key setting changed), page closed, is not said | **SURVIVED** | 0 |
| r14 | the page-closed "session ended" is kept as a Send result (a sign-in again does not take it away) | **SURVIVED** | 0 |
| r15 | the held or withheld notes of a send whose page was closed are told to another account | **SURVIVED** | 0 |
| r16 | a held send's note always says the sender can fix it | SURVIVED | 0 |
| r17 | a plain send with the page closed adds a result with the counts | killed | 1 |
| r18 | a dead send job while another bank's Send page is open is said nowhere | SURVIVED | 0 |
| r19 | a dead send job is told to an account that signed in after it | **SURVIVED** | 0 |
| r20 | closing any "Sent" box says "the server's answer could not be read" on the strip | **SURVIVED** | 0 |
| r21 | `ended_earlier` ignores whether anyone is signed in | SURVIVED | 0 |
| r22 | `ended_earlier` ignores the sign-out count | killed | 1 |
| r23 | a job whose account is not known is always this account's | killed (fix4 only) | 0 |
| r24 | a later send whose answer is read does not end the version-0 state | **SURVIVED** | 0 |
| r25 | a newer version found does not take away "no answer came ..." | killed | 1 |
| r26 | another account's restore with no answer is not marked unsure (this account's strip says "up to date") | SURVIVED | 0 |
| r27 | an earlier session's signed-out restore answer is not marked unsure | killed | 1 |
| r28 | "They are still on this computer." added even when the reason says so | killed | 1 |
| r30 | a check read for another account settles unsure and results | SURVIVED | 0 |
| r31 | the badge counts an unread send's changes as unsent | killed | 1 |
| r32 | a copy that can no longer send gets "Your changes were sent ... Get latest" | SURVIVED | 0 |
| r33 | the Banks list reads a job's answer without the account rule (`note_account` dropped) | SURVIVED | 0 |

The survivors that matter:

- **r20.** No test closes a normal "Sent" box and checks that nothing is added. The mutant tells every successful
  sender that the answer could not be read and to press Get latest.
- **r24.** `c74_i1_a_later_send_that_is_read_ends_the_sent_state_and_unlinking_forgets_it` tests only the unlink half.
  Without the line, a bank stays in "Your changes were sent. Get the latest ..." after a later send that was read.
- **r15 and r19.** The another-account rule is pinned for a failed send only (503). Neither a held send nor a dead job
  is pinned.
- **r12, r13 and r14.** Of `say_send_left`'s branches, only Conflict, Large, Failed (503 and daily limit) and Session
  are pressed. The Decide, Payload, Keyless and Pictures branch, the `Refused` branch and the Session kind are not.
- **r10.** The conflict's `head_moved` is not pinned. "Used as it is, as matching the newest version" would stay
  beside "someone sent version 5 first".
- **r04.** No test sends a pool request whose retry after a refresh is also answered 401 for the same account.
- I1, I2 and minors 1-3 and 6 have no test at all. Probes C1, C3 and U1a-c can become regression tests as they stand
  (the fake and helpers already exist).
