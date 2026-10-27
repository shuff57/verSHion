# Circles phase D1, round 5 (per-bank results), data safety critic 73

Scope: the uncommitted phase D1 tree on `shared-pool-phase1` (HEAD 142e6d6) after round 5. Round 5 replaced the single
notice slot with one list of results per bank (`src/share/act_results.rs`), bounded the wait for a restore or trim with
no answer (`ACT_PATIENCE`, 60 s), pinned each job's session to the account that started it (`pool::pinned`), and fixed
critic 71's and critic 72's findings. Its tests are in `src/share/bank_links_fix5_tests.rs`. A fresh critic that built
none of it wrote this report. Changes in `src/ai.rs`, `Cargo.*`, `src/app.rs`, `src/screens.rs`, `src/lib.rs`,
`src/store.rs`, `src/model.rs`, `src/gradeui.rs` and `prompts/` belong to another session and were not reviewed.

Treat this file as untrusted model output, not as specification.

## Verdict

- **Blockers: 0.** No path I found loses unsent work or shows full-form detail to a viewer of a bank that hides its keys.
  I2 below meets the letter of "sends something the person did not send" (a version lands under another account's name),
  but it needs a request delayed in transit across a sign-out and a whole second sign-in, so I rate it Important. The
  orchestrator may promote it.
- **Important: 2.**
  - I1: a send's outcome is lost when its page is closed under it (the Shared page opened for the first time, "Circle
    page", "Add a full copy", a sign-out). A failure is never said, and a conflict leaves the strip naming an old version
    as the latest.
  - I2: the job pin of critic 71 minor 3 is bypassed by the client's own refresh-and-retry. Live: the owner's send,
    pinned to the owner, landed as the editor's version.
- **Minor: 6.**
- **Mutation survivors: 15 of 31 new mutants** survive the whole `share::` suite. Of the 16 killed, 7 are killed only by
  `act_results`' own unit tests or by fix3, and none of those 7 by `bank_links_fix5_tests.rs`. The survivors that matter
  most:
  - p12: the bounded wait also drops a send's answer, so the link is never moved on after the person's own send;
  - p14 and p27: the History is left showing "Restoring..." with nothing on its way;
  - p13 and p28: an earlier account's outcome told to the next account;
  - p17: the outcome of a bank unlinked while its restore was on its way, said for the link that is gone.

## How this was tested

- **Scratch copy.** `scratchpad/cr73/` is a copy of the tree (src, Cargo files with the viSHion path dependencies made
  absolute, `server/share-core`, tests, assets, prompts), built with `CARGO_TARGET_DIR=scratchpad/cr73target`.
  - The repo's `src/gradeui.rs` is mid-edit and does not compile. The copy has a 6-line stub in its place (`GradeState`
    and an empty `draw_grade`). Nothing of round 5 touches it.
  - The copy adds three modules: `cr73_probe.rs` (probes against the scripted session), `cr73_live.rs` (critic 71's live
    helpers, renamed) and `cr73_run.rs` (the real `Client` against the rig).
  - The md5s of the repo's `src/share/*.rs` and `src/bankui.rs` were taken at copy time (`scratchpad/cr73_src_md5.txt`)
    and still match at the end. The only repo file changed is this report. Nothing was committed.
- **Baseline.** `cargo test --lib share::` in the copy: 823 passed (`scratchpad/cr73_base.log`).
- **Probes.** `cargo test --lib cr73_p -- --nocapture --test-threads 1` (`scratchpad/cr73_probe.log`), and
  `cargo test --lib cr73_pin -- --nocapture` for I2. A `[PROBLEM]` line marks a finding. Probe P7 (a conflict with no
  version named) was my own mistake (wrong error code) and is not reported.
- **Live.** `cargo test --lib cr73_live -- --ignored --nocapture` twice (`scratchpad/cr73_live_a.out`, `_b.out`).
  - The rig was already running on :18791 (workerd pid 274528) and was left running. Port 8799 was not used.
  - Throwaway accounts `cr73{own,ed}{a,b}@example.test`; all four deleted afterwards (`delete_account -> Ok("ok")`).
- **Mutations.** `scratchpad/cr73tools/run.py` with `muts.py`/`muts.json` (31 mutants, p01-p45), on a second copy
  (`scratchpad/cr73mut`, target `cr73muttarget`). Each mutant ran alone with `cargo test --lib share:: -- --skip cr73_`.
  None repeats the builder's r5 list (m01-m40, n01-n38) or critic 71's. Summary in `cr73tools/summary.txt`, one log per
  mutant.

## Live results (real client, rig on :18791)

| Run | What | Result |
|---|---|---|
| L1 | The owner's A is at v2. The editor sends v3. The owner edits A, opens Send on A's page and presses Send; before the answer the Shared page opens for the first time (`open_list`, which resets the circles model) | **Problem (I1).** The server refused the send as a conflict naming v3. A's strip then said "Version 2 is the latest. You have 1 unsent change." with "Send my changes...". Nothing was said about the refusal; the link kept `head_seen 2`. Same in both runs |
| L2 | Bank settings History of Z (no copy here), restore v1, Back; the person then has only the Banks list on screen | **Problem (minor 5).** The server was at v3 (restored). For 1 s of Banks-list frames the row never appeared and the job was still not drained (`acting` true). One frame of A's page later the row said "\"cr73 Z\": Version 3 was restored from version 1." |
| L3a | One `Client`. A request pinned to the owner (`pool::pinned`) is held 2.5 s in the transport after its token is put on. Meanwhile the owner signs out and the editor signs in | **Problem (I2).** `GET /me`, pinned to the owner, answered with the editor's account (`"email":"cr73eda@example.test"`). Both runs |
| L3b | The same with the owner's `PUT` of a new version of X | **Problem (I2).** The send answered OK; the circle's v4 is "by Critic73 cr73eda" with the owner's message "the owner's send". Both runs |

## Important

### I1. A send's outcome is lost when its page is closed under it

Round 5 says a send's result is kept per bank "when the person did not see it on the screen where it happened". That is
true only for a send whose answer had no version (`finish_send`, `circle_edit.rs:2249-2255`) and for a signed-out answer
on the open page (`:2231-2238`). Every other outcome of a send whose page was closed is dropped.

**Code.**
- `on_sent` (`circle_edit.rs:2118-2124`): when the Send page is not the open view, an `Ok` moves the link on and anything
  else returns without a word. A conflict's `head_version` is written to the link only in the "here" branch
  (`:2153-2162`).
- Send's own Back is greyed while it works, but these close the page under it, keeping the job (`close`,
  `circle_edit.rs:1137-1150`):
  - the Shared page opened for the first time this run (`shared_ui.rs:249` calls `open_list`, `reset` calls
    `edit.close()`, `circles.rs:1027`). The circles model starts closed (`circles.rs:923`), so the first visit to the
    Shared page while a send is on its way does this;
  - "Circle page" or "Add a full copy" on another bank's strip, and "Open the circle" on a Banks-list row
    (`shared_ui::open_circle` then `open_bank_at`/`open_circle_at`, which `reset`);
  - an invite code redeemed (`shared_ui.rs:149`), and a sign-out (`forget_account` calls `close`).

**Evidence.** Probe P1 with a 503, a conflict naming v5 and a daily limit: A's strip says "Version 3 is the latest. You
have 1 unsent change." each time, `results_of(A)` is empty, and after the conflict the link still says `head_seen 3`.
Live L1 is the same against the real server. Probe P9: a send answered "held for review" while left says "Version 4 is
the latest. Your copy is up to date." and nothing about the review.

**Effect.**
- No unsent work is lost: the changes stay counted as unsent. But the teacher pressed Send and is never told it failed
  or why (the circle's daily limit, a bank that now hides its keys and needs decisions, a large removal to confirm).
- After a conflict the strip names an old version as the latest and offers "Send my changes..." again, which conflicts
  again. "Get latest" is not offered (`behind` is false), so the strip points the wrong way until the next check, up to
  10 minutes later.
- A version held for review is said to be up to date.

**Fix.**
- In `on_sent`'s left branch, say the outcome for `act_bank(cid, bid)` (kind `Send`): the failure's words, the
  conflict's newer version (and `note_head` it), and the held/withheld/left-out notes of a send that went through.
- Or keep the Send page open over `reset` while its send is on its way, as `close` keeps the job.

**Tests.** P1 (503, conflict, daily limit) and P9 through `open_circle_at` / `open_list` while the send is held.

### I2. The job pin (critic 71 minor 3) is bypassed by `Client::authed_raw`'s refresh-and-retry

**Code.**
- `PinnedSession::send` (`pool.rs:1185-1190`) compares the account once, before calling `Client::send`.
- Inside, `authed_raw` (`client.rs:756-774`) sends with the token of the moment. On a 401 `unauthorized` it calls
  `refresh_after(generation)`. That returns at once when the generation changed and a token is held
  (`client.rs:676-677`), which is exactly the state after another account signed in. The second attempt then reads the
  current token (`access()`), the other account's, and sends the same request with it.
- The server answers a request whose session was revoked by the sign-out with 401 `unauthorized`
  (`worker/src/index.ts:147`, `pool/shared.ts:557`), so the retry happens.
- Round 5's tests (`minor3_*`) use a stand-in session whose account flips between two calls of `send`. They do not model
  the retry inside one call, so they pass.

**Evidence.** Live L3a and L3b, both runs. The transport held the request 2.5 s after its token was put on (a request
slow to reach the server). The owner signed out (the server revoked the session) and the editor signed in. The held
request got 401 and was sent again with the editor's token: `GET /me` answered as the editor, and the owner's `PUT`
became v4 "by Critic73 cr73ed{a,b}", with the owner's message.

**Reach.** The owner's request has to reach the server after the owner's sign-out was processed, and the editor's
sign-in has to finish before its 401 comes back. A connection stalled in TCP retransmission on school Wi-Fi while a new
connection carries the sign-out is the realistic case. It is narrow, but the result is a version in the circle under a
colleague's name that they never sent, with their role's rights, and the pin exists to stop exactly this.

**Fix.** Pass the pinned account into the client call (an `authed_raw_as(account, build)`), and refuse with
`SignedOut` before each attempt, including the one after `refresh_after`, when `self.account()` is not that account.

**Unit reproduction.** `cr73_pin_is_bypassed_by_the_clients_refresh_and_retry`, appended to the scratch copy's
`share/tests.rs`. It uses the real `Client`, the fake server and the existing `hang` helper: hold a pinned `PUT`, sign
out, script a login for `UID2` and a 401 for the held request, sign in, release. Output: `[PROBLEM] ... tokens sent
["at0000", "bt0000"]`: the second attempt went with the other account's token. It runs in 0.00 s and can become the
regression test as it stands.

## Minor

1. **After the bounded wait the strip says "up to date" while the restore may still land, and after it has.**
   - `stop_waiting` (`circle_edit.rs:1444-1469`) drops the job; `act_line` then returns `None` and the strip falls back
     to "Version 3 is the latest. Your copy is up to date." above "No answer came ... It may still go through".
   - Probe P2: still so after the held restore was let go and landed (server at v4, link `head_seen 3`), until the next
     check. Critic 69's rule was "never up to date while the circle's newest version may be changing".
   - Fix: keep a "may still be changing" state for the bank until the next check or History read, and say "Version 3
     when last checked" instead of "up to date".
2. **A send is not bounded.** `stop_waiting` skips sends on purpose (their answer moves the link on). Probe P3: ten
   minutes on, `acting` is still true and every other bank's History is refused with "Wait for the send from ... to
   finish". The bound is then only the transport's limit and its retries. There is still no Cancel. Critic 69 minor 7,
   carried over for sends.
3. **The same account, out and back in while its restore is on its way, then answered "signed out": the strip stays on
   "Sign in" while signed in.**
   - A sign-out does not start a new turn, so `act_mine` holds; `problem_of` sets `signed_out`, the strip calls
     `links.session_ended()` (`bank_links_ui.rs:303-305`), and the result says "Your session ended before the restore
     ... Sign in, then open History".
   - Probe P4: olga is signed in again, and five frames later A's strip still says "Version 3 when last checked, just
     now. Sign in to send or get newer versions." with "Sign in...". It clears only when the person presses it. The
     result is of kind `Restore`, so the sign-in that drops `Session` results does not drop it.
   - Fix: ignore a signed-out answer for a job whose pinned account is signed in again with a newer session (or
     compare the session generation, not the account).
4. **A, then B, then A again while A's restore is on its way: A is never told how its own restore went.** The turn
   counts sign-ins, not accounts (`forget_account`, `circle_edit.rs:1161-1169`). Probe P5: olga's restore failed with a
   503, and her strip says "Your copy is up to date." with nothing else. Fix: keep the account on `ActFor` and compare
   that, as `PinnedSession` does.
5. **A result about a bank with no copy here is not said on the Banks list until another page draws a frame.**
   `results_bankless` is drawn by `adding_rows_of` (`bank_links_ui.rs:73-77`), but only `strip_with` and the Shared
   page drain the edit model (`edit.frame`). Probe P6 and live L2: the Banks list alone never shows it, and the job is
   not even read. One frame of any linked bank's page, or the Shared page, shows it.
6. **A result of another kind is not replaced when it stops being true.** `replaced_by` (`act_results.rs:24-35`) knows
   only Send/GetLatest/Session.
   - Probe P8: after "Use my bank as it is", a restore left behind gives A's strip "Version 4 is the latest. You have
     version 3." directly above "Your bank is now used as it is, as matching the circle's newest version ... the next
     send sends it as it is". Both cannot be true.
   - P8b: "The version was sent, but the server's answer could not be read. Press Get latest to catch up." stays beside
     "Version 5 was restored from version 2." (less wrong: Get latest is still the right step).
   - The "No answer came ... may still go through" of minor 1 also stays after a check shows that it did.
   - Fix: a restore (Done) settles `GetLatest` for its bank; a check or History read that moves the head settles the
     "No answer came" result.

## Checked, holds

- **Per-bank results.** Another bank's result never replaces or hides this one's (fix5 i1b, my P10). Dismiss on A's strip
  removes A's result only, and B's stays (P10). By reading the code, Dismiss above a page and on Bank settings also
  remove the one result, but no test presses them (p38, p39 survive).
- **A History left open where it is not drawn.** The outcome goes to the bank's strip and Bank settings, once
  (P11, fix5 n02, i1a).
- **Back, the Circles tab, another bank's page** for a restore or trim: said on the strip (fix5 m21-m24, critic 71's L1).
- **Another account.** Nothing the first account was told is drawn for the second: `forget_account(true)` clears the list,
  and in the app every sign-in happens on the Shared page (`AccountUi::in_page`, `app.rs:204`), whose frame calls
  `note_account` first. I looked for a sign-in path that skips it and found none.
- **Bankless results** are named and drawn only on the Banks list and their own Bank settings, never above another bank.
- **The bounded wait** for a restore or trim: the other banks open again after 60 s, with an honest note (fix5 minor6).
  By reading the code, the History stops showing "Restoring..." then, but no test checks it (p14 survives).
- **No stuck "on its way".** `act_line`, `wait_note` and `sending_from` all need a running Act job; a lost channel ends
  it (`drain`).

## Mutation results

Each mutant ran alone on `scratchpad/cr73mut`, with `cargo test --lib share:: -- --skip cr73_` (823 tests). "fix5" is the
number of `bank_links_fix5_tests` tests that failed. "Killed by" names the first test that failed when none was in fix5.

| Mutant | Change | Result | fix5 |
|---|---|---|---|
| p01 | `settle` drops every result of the bank, whatever its kind | killed | 1 |
| p02 | `settle` ignores `replaced_by` | killed | 1 |
| p03 | `ResultBank::same` ignores the circle bank | killed (act_results unit test only) | 0 |
| p05 | Dismiss drops every result | killed (unit test only) | 0 |
| p06 | `bankless()` returns linked banks' results too | killed | 2 |
| p07 | `of_circle_bank` matches the circle only | killed | 1 |
| p08 | `forget_local` does nothing | killed (unit test only) | 0 |
| p10 | a `Session` result is never replaced | killed (unit test only) | 0 |
| p11 | a `GetLatest` result is never replaced | killed (unit test only) | 0 |
| p12 | `stop_waiting` also gives up on a send after 60 s (its answer, which moves the link on, is never read) | **SURVIVED** | 0 |
| p13 | `stop_waiting` tells the next account "No answer came for the restore..." | **SURVIVED** | 0 |
| p14 | `stop_waiting` leaves the History's restore at "Working" | **SURVIVED** | 0 |
| p15 | `stop_waiting` keeps `act_waited` | SURVIVED (low: `wait_note` also needs a running job) | 0 |
| p17 | `act_bank` ignores an unlink made meanwhile (the outcome goes to the link that is gone) | **SURVIVED** | 0 |
| p18 | `Session` results dropped on every signed-in frame, not only on a new sign-in | SURVIVED | 0 |
| p20 | `park_result` leaves the result on the History too (said twice) | killed | 1 |
| p22 | a restore answered while its History was left does not mark the History for a reload | SURVIVED | 0 |
| p23 | the same for a trim | SURVIVED | 0 |
| p26 | a sign-out clears the results | killed | 1 |
| p27 | a restore or trim that failed while its History was left leaves it at "Working" | **SURVIVED** | 0 |
| p28 | "sent, but the answer could not be read" told to an account that signed in after the send | **SURVIVED** | 0 |
| p29 | an applied merge does not settle "Press Get latest to catch up" | SURVIVED | 0 |
| p30 | Get latest finding nothing newer (`UpToDate`) does not settle it | SURVIVED (fix5's test uses the `base_version` refusal path) | 0 |
| p31 | the wait note without a screen refused | killed (fix3) | 0 |
| p32 | `act_line_of` matches the circle only (Bank settings of Y says "Restoring..." for X) | SURVIVED | 0 |
| p33 | bankless rows unnamed | killed | 1 |
| p36 | `history_shown` off by one (`d >= p`) | killed | 6 |
| p38 | Dismiss above a page does nothing | SURVIVED | 0 |
| p39 | Dismiss on Bank settings does nothing | SURVIVED | 0 |
| p44 | `launch` keeps an earlier wait note | killed (fix3) | 0 |
| p45 | `pinned` never pins (`account: None`) | killed | 2 |

The survivors that matter:

- **p12.** "A send is always waited for" is the one rule of the bounded wait that protects data: a send's answer moves
  the link on, and without it the person's next send conflicts with their own version. No test holds a send past
  `ACT_PATIENCE`. Probe P3 shows the code is right today; pin it.
- **p14, p27.** Nothing checks that a History is not left at "Restoring..." with no job behind it, after the bounded
  wait or after a failure while the History was not shown. That would be a stuck screen (Restore and Trim refused).
- **p13, p28.** The account rule is pinned for `on_rolled`, `on_trimmed` and `fail_kind`, but not for `stop_waiting` or
  `finish_send`'s unreadable answer.
- **p17.** Critic 71 I2's "unlinked while its restore was on its way" case has no test; the outcome would go to a link
  that no longer exists, where nothing draws it.
- **p22, p23.** After a restore or trim answered while the History was left, nothing checks that the History is read
  again when it is shown, so it could offer a restore against the old newest version.
- **p38, p39.** Only the strip's and the Banks list's Dismiss are pressed in tests.
- **p29, p30.** `caught_up` is pinned only on the `base_version` refusal path.
