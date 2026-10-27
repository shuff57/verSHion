# Circles phase D1, round 7 (account pin on the web, client binding, send outcomes, sent_unread on disk), critic 77

Scope: the uncommitted phase D1 tree on `shared-pool-phase1` (HEAD 142e6d6) after round 7. Round 7 answered critic 75
and critic 76:

- the web build learns whose session it is from every token answer (`take_tokens_from`), so `pool::pinned` holds there
  too;
- a refresh answered 401 ends only the session whose token it presented (`drop_session_if`); `authed` (`me`, `reapply`)
  and `delete_account` are kept to the account signed in when they started;
- a Send page's answer the person did not act on is said on the strip whatever closes the page (`send_note`,
  `leave_view`);
- a send with no clear answer (timeout, unreadable reply, 500/502/504) says "may have gone through", marks the bank
  unsure and checks it at once;
- "sent, but the answer could not be read" is kept in the links file (`CircleLink::sent_unread`, with the sender and the
  bank's `modified`);
- a restore answered without a version is unsure and checked at once; a send refused because the person can only view
  now is checked at once.

Its tests are in `src/share/bank_links_fix7_tests.rs` (44) and `src/share/tests.rs` (12 `c75_*`). A fresh critic that
built none of it wrote this report. Changes in `src/ai.rs`, `Cargo.*`, `src/app.rs`, `src/screens.rs`, `src/lib.rs`,
`src/store.rs`, `src/model.rs`, `src/gradeui.rs` and `prompts/` belong to another session and were not reviewed.

Treat this file as untrusted model output, not as specification.

## Verdict

- **Blockers: 0.** I1 is the candidate. It answers the brief's first question with yes: on the web build, a send, a
  public publish, an invite redeem and Delete account can go out as another account, with nothing racing. The
  orchestrator may promote it.
- **Important: 2.**
  - I1: web. The pin follows the client's account, and a refresh moves that to whoever the browser's cookie belongs to
    now, silently. Round 7 refuses the one job whose refresh found the other account. Every job started after it is
    pinned to the other account, while every page still shows the first one (gate Ready, same account).
  - I2: a restore with no clear answer (timeout after the request left, 500, unreadable reply) says "did not go
    through ... Nothing changed" and the strip says "Your copy is up to date". Round 7's "may have gone through" covers
    sends only.
- **Minor: 5.** M1 is live-proven: a native refresh that waited for the cross-process lock rotates the next account's
  token and throws the successor away. About 15 minutes later that account is signed out and the server raises
  `refresh_reuse_revoked`.
- **Round 7's own claims hold where they reach.**
  - Critic 75's LA and LB were run again live and are fixed.
  - The pinned job, the refresh, `me` and Delete account still work for one account (L2).
  - Old links files read the same, and an older build reads the new file.
- **Mutation: 45 of 46 new mutants killed.**
  - The one survivor (e03) is equivalent.
  - Two were killed only by earlier rounds' tests (c13 by `c73_i2`, e15 by fix6).
  - Every other kill includes a round 7 test.
  - The round 7 tests pin what round 7 changed. What they miss is everything outside it: I1, I2 and M1-M4 have no test.

## How this was tested

- **Scratch copies** (all under the session scratchpad).
  - `repo/` is a copy of the tree. `Cargo.toml`, `Cargo.lock`, `src/ai.rs`, `app.rs`, `screens.rs`, `lib.rs`,
    `store.rs` and `model.rs` were put back to HEAD, and `src/gradeui.rs` was removed. Without that the copy does not
    build: the other session's `Cargo.toml` names `../viSHion` path crates, and `gradeui` is mid-edit.
  - `cr77mA/` and `cr77mB/` were clean copies of the same tree, used for the mutants and removed afterwards. `mk.py`
    and `run.py` rebuild them.
  - The md5s of the repo's `src/share/*.rs` and `src/bankui.rs` were taken first (`cr77_src_md5.txt`) and still match
    at the end. The only repo file changed is this report. Nothing was committed.
- **Baseline** (`repo/`):
  - `cargo test --lib`: 1621 passed, 0 failed, 3 ignored (`test1.log`);
  - `cargo build --bins`: ok (`bins.log`);
  - `cargo check --lib --target wasm32-unknown-unknown`: ok, 5 warnings (`wasm.log`);
  - `cargo test --lib share::` in `cr77mA/`: 924 passed (`cr77mA_base.log`).
- **Probes.** They were appended to the copy's `share/tests.rs` (P1, P1b, P2, P3) and `share/bank_links_fix7_tests.rs`
  (U1-U3). Run them with `cargo test --lib cr77_ -- --nocapture --test-threads 1` (`cr77_probes.log`). A
  `[PROBLEM]` line marks a finding.
- **Live.**
  - `share/cr77_run.rs` (critic 75's scaffolding, new runs) uses the real client and `NativeTransport` against the rig
    on :18791. The rig (workerd pid 274528, rig pid 273219) was already running and was left running. Port 8799 was
    not used.
  - Run: `cargo test --lib cr77_live -- --ignored --nocapture` (`cr77_live_a.out`).
  - The accounts were throwaway `cr77{a..g}081e58@example.test`. All seven are deleted: six by the clean-up, one (g) by
    the test itself.
- **Mutations.** `cr77tools/mk.py` makes `muts.json` (46 mutants) and `cr77tools/run.py` runs them. Each mutant ran
  alone with `cargo test --lib share::`, two copies in parallel. The summaries are `cr77tools/summaryA.txt` and
  `summaryB.txt`, with one log per mutant in `cr77tools/logs/`.

## Live results (real client, rig on :18791)

| Run | What | Result |
|---|---|---|
| LA | Critic 75 LA again. X's sessions are revoked elsewhere, so X's DELETE /account is answered 401. It is held 2.5 s in transit while X signs out here and Y (same password) signs in | **Holds.** `delete_account -> Err(SignedOut)`. Y's session stays, and X and Y can both still sign in |
| LB | Critic 75 LB again. A pinned X job gets 401, and its refresh is held 2.5 s. X signs out and Y signs in. The refresh is answered 401 | **Holds.** The job ends `SignedOut`. Y keeps the session, and `me()` answers Y |
| L1 | X's pinned job needs a refresh, which waits for the cross-process lock. X signs out and Y signs in. The lock is released | **Problem (M1).** The refresh presented **Y's** refresh token, the server rotated it (200), and the client threw the successor away (sign-out count changed). 12 s later Y's next refresh replays the rotated token: 401, Y is signed out, and the rig log shows `auth alert: refresh_reuse_revoked user=<Y> family=...` |
| L2 | One account: pinned GET /circles, then the access token's expiry (refresh), `me()`, then Delete account | **Holds.** 200, 200, me ok, delete ok; signing in afterwards is refused |

## Important

### I1. Web: after a refresh comes back for another account, every later job is pinned to it, and the pages never notice

**Path (web build, two tabs of the app in one browser).**
1. Tab 1 shows X. In tab 2, X signs out and Y signs in, so the refresh cookie is now Y's.
2. In tab 1, X's next request is answered 401 (X's session was ended), or X's access token runs out within 15 minutes.
   Either way, tab 1 refreshes with the cookie and the answer names Y.
3. `take_tokens_from` sets `Inner::account = Y` (`client.rs:592-597`). The job that asked is refused (`access_for`
   checks again, `client.rs:761-764`). Round 7 stops there.
4. Nothing tells the Account window. `snapshot_of` (`publish.rs:373-398`) builds the gate and the account from `m.me`,
   which is still X. `AccountModel::frame` does nothing while the window is closed (`model.rs:742`), so `/me` is not
   read again. The gate stays `Ready` and `snap.account` stays X's address, so no page's `note_account` sees a change.
5. The next job on any page calls `pool::pinned(session)`, which reads `client.account()` at that moment
   (`pool.rs:1188-1191`): Y. It goes out with Y's sign-in and Y's rights. Pages that do this include a circle Send,
   Get latest, restore, Publish to the public pool, Mine, Browse imports, and redeeming an invite code. The most natural
   trigger is the refused job itself when it is a send. `on_sent` puts the page back at the note ("Your session ended.
   Sign in again; where you were is kept", `circle_edit.rs:2431-2439`) with Send still on it. Pressing Send again
   starts a new job pinned to Y (by code).
6. Delete account takes `self.account()` when pressed (`client.rs:1051`), which is also Y. If Y has the same password
   (one teacher with two accounts), Y is deleted from a page that says X.

**Evidence.**
- P1, client level, `Mode::Web`:
  - the first account's pinned job is refused (`Err(SignedOut)`), and `client.account()` is then Y;
  - the next `pool::pinned` job is pinned to Y and goes out with bearer `bt` (Y's), answered 201;
  - `delete_account` goes out with `bt`, `Ok(())`.
- P1b, through the real `AccountModel` (web harness):
  - after the flip, `snapshot_of` still says gate Ready and account `https://pool.test\nteacher@school.example.org`;
  - `client.account()` is UID2, and a job started then is pinned to UID2.
- Only one sign-of-life reaches the person. The refused job's `SignedOut` sets `links.out`, so bank strips say "Sign
  in...". Browse, Mine, Publish and the Send page are not gated by it.

**Why it is the brief's question.** No timing is needed after step 2: from then on, each new job in tab 1 lands as Y
while tab 1 says X. This is critic 73's I2 and critic 75's I2 by a door round 7 did not close. The pin holds a job to
the client's account at the moment the job starts. It does not hold it to the account the person is looking at.

**Fix (sketch).**
- Keep, with `me`, the account id the client had when `/me` answered.
- In `snapshot_of`, when `client.account()` differs from that id, the gate is `Checking`, `account` is `None` (so the
  pages' account watch fires) and a quiet `/me` is asked for.
- Pin jobs to that id, not to `client.account()` at spawn: hand the pages a session already pinned by the Account
  window.
- `delete_account` should take the account to delete from the page and refuse when it is not the client's.

Tests:
- P1 and P1b as they stand;
- a web delete after the flip sends nothing;
- a page's `note_account` sees `Another` after the flip.

### I2. A restore with no clear answer says "did not go through ... Nothing changed" and "up to date"

**Code.**
- `unclear_why` is used only in `on_sent` (`circle_edit.rs:2299`).
- `on_rolled` maps a timeout, a 500/502/504 or an unreadable reply to `self.problem_of(&e)`, then
  `RestoreStage::Failed` (`circle_edit.rs:3273`).
- `unsure` is set only for a signed-out answer (`3278`). No check is asked.
- `restore_note` words a `Failed` stage as "The restore of version N did not go through: ..." (`3652`).

**Evidence** (probe U1, History left, read on the strip):

| Answer | Result line | Strip line |
|---|---|---|
| timeout after the request left | "The restore of version 1 did not go through: Can't reach pool.test: the connection timed out. Nothing changed; your work on this device is unaffected. Open History to try again." | "Version 3 is the latest. Your copy is up to date." (`unsure=false`, no check) |
| 500 `server_error` | "...did not go through: verSHion's server had a problem of its own. Nothing was changed; try again in a minute..." | the same "up to date" |
| 200 with `version: "four"` | "...did not go through: The server sent a reply the app does not understand..." | the same "up to date" |

**Why it matters.**
- The restore may have made version 4 in the circle. The person is told it did not, and that the copy is up to date.
  That is the claim critics 69, 72, 74 and 76 (I2) held the line on, and it holds until the 10-minute check.
- Nothing is doubled: `pending_restore` is kept, so Restore again replays the same commit.
- A trim answered the same way says "did not go through" too. That claim is weaker, because a trim does not move the
  head.

**Fix.** Do what round 7 did for sends:
- in `on_rolled` and `on_trimmed`, check `unclear_why(&e)` first;
- for an unclear answer, say "may have gone through", insert into `unsure` and push `check_asks`;
- keep "did not go through" for refusals.

## Minor

1. **A native refresh delayed by the cross-process lock rotates the next account's token and throws it away
   (live-proven, L1 and P2).**
   - `refresh_once` reads the sign-out count before `self.lock.acquire().await` (`client.rs:620-621`). It reads the
     account and token after it (`624-651`).
   - If X signs out and Y signs in while the lock is held (another copy of verSHion refreshing, or a keychain prompt),
     the refresh presents Y's token. The server rotates it. `take_tokens` then drops the answer, because the sign-out
     count changed (`553-555`).
   - Y keeps a rotated-away token. Y's next refresh after the 10 s grace is a replay: the family is revoked, Y is
     signed out, and the server's theft alert fires.
   - The `Ok(None)` branch at `634-637` likewise clears `account` and `refresh` with no check of either.
   - Fix: read the count under the same lock as the account and token, after `acquire`. Or refuse before sending when
     it moved.
2. **A late refresh answer for the old account, after a sign-in over it, swaps the client back (P3).**
   - With no sign-out between (the Enrol page while signed in signs in over the session: `submit_enrol`,
     `model.rs:1123-1124`), X's refresh answered 200 after Y signed in runs `take_tokens_from`'s native branch.
   - Y is `previous`, so `clear_refresh(Y)` deletes Y's saved sign-in, and the client becomes X again (`client.rs:567-575`).
   - The window shows Y, so the next jobs are pinned to X: I1's shape, native, needing that race.
   - Fix: in the 200 branch, keep the answer only if the account is still the one whose token was presented (`whose`).
3. **After a restart, the Banks list badge counts a send whose answer was unread as unsent (U2).**
   - `sent_unread_link` hides a flag whose `by` is not the account the edit model knows (`circle_edit.rs:992-996`).
   - `CircleEditModel::account` is set only in `frame` while Ready (`1554-1557`). On the Banks list, `tick_background`
     calls `edit.frame` only while acting (`circles.rs:3509-3512`).
   - So after a restart, olga signed in, the badge says "1 unsent change". Once A's page is drawn, the strip says
     "Your changes were sent. Get the latest..." and the badge then says "newer version".
   - Critic 75 minor 5's fix holds on the strip only. Fix: let the badge path learn the account (`frame` on the Banks
     list when Ready, or pass `snap.account` to `badge`).
4. **A version-0 answer that lands after another account signed in is not kept for the account that sent it (U3).**
   - `finish_send` records `sent_unread` only `if self.act_mine()` (`circle_edit.rs:2528`), although `by` already
     names the sender.
   - Olga, back, sees "You have 1 unsent change. Send my changes...". `pending_send` went with the account switch, so
     the send is a new commit, refused as a conflict with her own version ("someone sent version N first").
   - Rare (version 0 and an account switch while sending). Fix: record it whatever account is signed in now, tagged
     with the sender.
5. **The web no-account case and the tests' account.**
   - `take_tokens_from`'s comment says an answer naming nobody leaves the account unknown, "a pinned job then sends
     nothing". But `pinned` with `None` "is never held back" (`pool.rs:1182`), so such jobs go unpinned. The server
     always names the user, so this is a wrong comment, not a hole today.
   - The fix7 tests' `ready()` snapshot has `account: None`, so every `sent_unread` they write has an empty `by`, which
     is shown to anyone. Only `r7_sent_but_not_read_is_kept_for_the_account_that_sent_it` exercises the account rule,
     and nothing exercises it across a restart (M3 went unnoticed).

## Checked, holds

- **Native pin, round 7 paths.** Live LA and LB are fixed:
  - Delete account answered 401 after another account signed in is not retried;
  - the old account's refresh 401 leaves the new session alone;
  - `me` and `reapply` are kept to their account (`c75_m2_*`).

  The same account signing out and in again keeps its new session (`c75_m1_..._same_account_...`).
- **Send outcomes on every page-close path.** The callers of `leave_view` are `close`, `open_send`, `open_merge`,
  `open_history` and `back`. With `forget_account` (through `close`) they cover every way the Send view is replaced. I
  found no path that drops `send_note`: `cancel_merge` only closes a Merge view, and `open_version` is entered from a
  History only. An answer the person acted on (back to the note, Get latest on the page's own conflict) is not said
  again.
- **"May have gone through" for sends.** Timeout, unreadable reply and 500/502/504 are never "not sent". 503 `busy` and
  refusals are still "not sent", correctly. The bank is unsure until checked, and checked at once. Try again replays
  the same commit.
- **`sent_unread` file compatibility.**
  - Forward: a file without the field reads as `None` (`c75_m5_the_link_file_without_...`).
  - Backward: no `deny_unknown_fields` on `CircleLink` or `LinkFile`, in HEAD or now, so an older build reads the new
    file. It drops the field on its next write, so the strip again counts the sent changes as unsent: acceptable.
  - A malformed `sent_unread` fails the whole link entry. The file is kept as `.bad`, as for any field (existing
    design).
- **`sent_unread` lifecycle.** Cleared by a send that is read, by Get latest that catches up or applies (`caught_up`
  from `finish_merge`), and for an unknown sender when another account signs in. Later edits are not called sent
  (`modified` compared). It is never shown to another account.
- **Restore with version 0, viewer-refused send.** Both are unsure (restore) and checked at once. The words are right:
  "send changes", not "add banks"; no "Version 0".
- **Public pool, Mine, Browse, Publish.** Every caller still goes through `pool::pinned`. `authed_raw(None)` is
  unchanged, and the 924 `share::` tests pass. L2 live: one account's pinned job, refresh, `me` and delete work.
  `knows_device`, `after_link` and enrol behave the same on the web with the account now known.
- **Stuck states.** None found that needs a restart:
  - a web flip leaves the strips at "Sign in...", and the Account window's restore then shows Y;
  - an unclear send's unsure state ends at the next answered check;
  - `sent_unread` with Get latest refused (the head held for review) keeps offering Get latest until the hold lifts.
    It is not stuck, but it loops.

## Mutation results

Each mutant ran alone (`cargo test --lib share::`, 924 tests). "r7" is the number of round 7 tests that failed: the
`bank_links_fix7_tests` and the `c75_*` tests in `tests.rs`. None of these mutants repeats critic 75's r01-r33, except
the four re-checks marked "again".

| Mutant | Change | Result | r7 |
|---|---|---|---|
| c01 | web: the token answer's user id not kept | killed | 4 |
| c02 | web: a refresh for another account keeps the old id | killed | 1 |
| c03 | refresh 401 ends whatever session is current | killed | 3 |
| c04 | refresh 401: the account is not compared | killed | 1 |
| c05 | refresh 401: the sign-out count is not compared | killed | 1 |
| c06 | `me`/`reapply` not kept to the account | killed | 1 |
| c07 | delete ends the session of whoever is signed in now | killed | 1 |
| c08 | delete answered after its account signed out keeps its device token | killed | 1 |
| c09 | delete not kept to the account | killed | 1 |
| c10 | refresh: whose token was presented not known | killed | 1 |
| c11 | (critic 75 r01 again) token after a refresh not checked against the pin | killed | 1 |
| c12 | (critic 75 r04 again) second 401 for the account keeps its session | killed | 1 |
| c13 | a 401 after another account signed in goes on to refresh-and-retry | killed (c73_i2 only) | 0 |
| e01 | a Send page left undrawn, then closed, says nothing | killed | 7 |
| e02 | the page's note is said even when the person is back at the note | killed | 1 |
| e03 | a new answer on the page does not first drop the last note | **SURVIVED** (equivalent: `do_send` clears `send_note` at every start, and every here-branch assigns it) | 0 |
| e04-e06 | 500 / unreadable / timeout called "not sent" | killed | 1-2 |
| e07, e08 | an unclear send not unsure / not checked at once | killed | 1-2 |
| e09, e10 | viewer-refused send not checked / generic "add banks" words | killed | 1 |
| e11 | `sent_unread` forgets the bank's `modified` | killed | 3 |
| e12, e13 | `sent_unread` without its sender / shown to every account | killed | 1 |
| e14 | Get latest that catches up leaves `sent_unread` in the file | killed | 1 |
| e15 | another account: an unread send of unknown sender kept | killed (fix6 only) | 0 |
| e16-e18 | restore v0: not unsure / not checked / "Version 0 was restored" | killed | 1-2 |
| e19-e22 | edits after an unread send taken as sent (link, strip, badge x2) | killed | 1 |
| e23 | (critic 75 minor 4 again) conflict writes back the link as at the send's start | killed | 1 |
| e24 | a send that went writes back the link as at the send's start | killed | 1 |
| e25 | (critic 75 r12 again) "needs choices" not said with the page closed | killed | 1 |
| e26 | (critic 75 minor 6 again) the Banks list does not set the frame | killed | 1 |
| e27 | Get latest on the page's own conflict still says it on the strip | killed | 1 |
| e28 | `sent_unread: null` written into old-format links | killed | 1 |
| e29 | the strip calls an unclear send "not sent" | killed | 3 |
| e30 | (critic 75 r20 again) every "Sent" box said on the strip when closed | killed | 2 |
| e31 | (critic 75 r10 again) conflict does not settle "no answer came" | killed | 1 |
| e32 | a version-0 send not recorded at all | killed | 6 |
| e33 | (critic 75 r24 again) a later read send keeps `sent_unread` | killed | 1 |

**Survivors:** e03 only, and it is equivalent. The gaps are untested behaviour, not weak assertions. All of these have
no test:
- the client's account moving under a Ready gate (I1; P1, P1b);
- a restore or trim with no clear answer (I2; U1);
- the refresh that crosses the lock (M1; P2, L1) or a sign-in over the session (M2; P3);
- the badge after a restart with a real account (M3; U2);
- a version-0 answer after an account switch (M4; U3).

Also, the fix7 `sent_unread` tests use an account-less snapshot (minor 5). Each probe can become a regression test as
it stands.
