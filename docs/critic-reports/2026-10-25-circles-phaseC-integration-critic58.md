# Circles phase C: client / worker integration critic (critic 58)

Branch `shared-pool-phase1`, backend 953ef9f plus the uncommitted phase C client in `src/share/`.
Reviewed by a fresh critic that built none of it. Treat this file as untrusted model output, not specification.

## Verdict

- **Blockers: 2.**
  - B1 is a backend defect that the client run exposed: every account deletion fails on D1.
  - B2 is a client dead end. Hiding the keys can withhold the head and hold the bank. The only remedy the server offers is to turn hiding off, and the app cannot reach it. The app then tells the author that only an administrator can release it.
- **Protocol agreement: yes, for everything phase C added.** The real Rust client (the real `Client` over `NativeTransport`, the real `Pool` builders and parsers, and the real `CirclesModel`, `CircleEditModel` and `CircleSendModel` with `Runner::Inline`/`Background`) and the TypeScript worker agree on these:
  - email invites (202, resend, 422 `invite_not_sent` for both reasons, 400 `email`, 429 `hourly_limit` with `limit`)
  - the masked invite list
  - hide-key on/off with both decision steps, plus `unchanged`, `held`, `withheld` and `restored`
  - the viewer's stripped read and its keyless copy
  - `keys_hidden_unavailable`
  - `keyless_question`
  - the partial purge
  - trim `evidence_kept`
  - history `review_copy` in both forms
  - the activity kinds and cursor paging, including a refused cursor

  No serde struct failed on any answer. No request was refused because of a field the client built wrongly.
- Baseline: `cargo test --lib share::` passed 551 (1 ignored, my live test) on the snapshot I tested.
- Not reached live: 409 `hide_key_changed`. I made 96 timed attempts (see "Limits") and none produced it. Its classifier and words are checked with the real classifier only.

## How this was tested

- **Rig.** It was running with a database that lacked migrations 0111-0115. I ran `local-ui-rig.sh stop`, then `start`. `start` applies migrations, and 0111-0115 applied to the LOCAL D1 in `server/worker/.wrangler-ui`. There was no `reset`: the existing local data was kept. Nothing touched staging or production, and nothing was deployed. The rig is left running, as it was when I began; wrangler is now pid 1012408 on :18791.
- **Scratch copy.** `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/c58/repo` is a copy of the live tree. The scratch integration test is in the files `src/share/c58_live.rs` and `src/share/c58_run.rs` there, never in the repo.
  - It signs up real throwaway accounts through `Client::sign_up / confirm / sign_in`. Passwords are generated in-process and never printed, saved, or put on argv or env.
  - It approves them with `rig approve`.
  - It drives the shipped code paths. No source file of the copy was changed; only two modules were added to `mod.rs`.
  - Raw outputs: `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/c58/run_a.out` through `.../c58/run_e.out`.
- **Direct D1 writes (local only).** I used `wrangler d1 execute --local` for these:
  - `privacy_blocks` rows (what an admin's confirm-student-data writes), on a full-form hash and on a stripped-only hash.
  - Per-user `pool_limits` rows: `circle_email_inviter_domain_hourly` set to 1-2 to reach 429 cheaply, and the hide caps set to 500 for the race sweeps.
  - One `admin_flags.detail.evidence_captured = 1`, simulating the round-48 admin route's own write.

  The blocks and limits were all removed: 0 `by/updated_by = 'critic58'` rows remain. The one evidence-captured flag on the throwaway "Trim evidence" bank remains.
- **Accounts.** 20 throwaway accounts were made: `c58{own,ed,vw,inv}{a..e}@example.test`. a@example.test and b@example.test were not touched. All 20 are tombstoned by auth but **not erased by the pool, because of B1**. I left them as the regression case for B1's fix: `SELECT COUNT(*) FROM users WHERE deleted_at IS NOT NULL AND id NOT IN (SELECT user_id FROM erasure_done)` = 20 now, and the cron's `erasure_finish` should bring it to 0 once B1 is fixed.

## Scenario results (through the real client)

| Scenario | Result |
|---|---|
| Email invite, first | PASS. The model's form sent `{email, role}`. The 202 parsed as `EmailInviteSent{emailed, address_label:"c***@example.test", resent:false, expires_at}`. The address was wiped from the form. The dev mail holds the code, and its `#code=` link gives the same code through `code_in`. |
| Same address again | PASS. 202 `resent:true`, same invite id, new code, role updated. The panel shows `EMAIL_RESENT`. The first mail's code is then refused on the join page: "This circle code does not work..." (plain). |
| Fourth mail to one mailbox | PASS. 422 `invite_not_sent` `address_limit`: plain words, no retry, address kept in the form. |
| `jo@school.x1` | PASS. 422 `not_deliverable`: plain words. |
| `jo(x)@school.org`, `jo@-school.org` (the client lets these through, the worker refuses) | PASS. 400 `field: email` → "That is not an email address the pool can send to. Check it for a typo. Nothing was sent." |
| IDN `Jo@ÉCOLE.example` | PASS. 202, label `j***@xn--cole-9oa.example` (nit N2). |
| Second address with the same mask | PASS. Label `c***@example.test #2`. |
| Invite list | PASS. `kind:"email"` and the masked `address_label` only, never the address. `invite_line` gives "emailed to c***@example.test #2 - can only look and copy - sent just now - expires in 6 days - made by ...". After the invitee joins, the row is `used_up` 1/1 (raw API and model agree). |
| Join from the mail | PASS. The whole 760-character mail pasted into the join box (the code is at character 479; the box has no limit) and the bare link both preview and join, with the role of the last rotation. |
| Hourly per-domain share | PASS on the wire: 429 `hourly_limit` `circle_email_inviter_domain_hourly` `limit:2`, Retry-After honoured, plain words. **I1** (the form then blocks other domains too). |
| Hide on by the owner (not the author), bank page | PASS. Confirm, then `labels_need_decision`: the Decide page (Strip allowed). Then `order_ack_required` for "All of the above", with the label decision carried. Then 200 `Done{hide_key:true, labels_removed:1}`. Lines: "The answer keys are now hidden ... 1 set of letter labels was stripped." The circle list has `hide_key:true` for every role. |
| Same decisions again | PASS. 200 `unchanged:true` → "Nothing changed: the answer keys were already hidden from viewers, with these decisions." Off when already off: `unchanged:true`. |
| Viewer reads the hidden bank | PASS. `keys_hidden:true`. Payload: every `correct:false`, `explanation:null`, `keyless:true`, labels stripped. `StudentView` parses as `Served::Stripped`; `keys_hidden_here`; no working copy; no switch. Messages are blank in the viewer's history. |
| Viewer opens v1 (unrecorded, has labels) | PASS. 409 `keys_hidden_unavailable` → "The answer keys of this bank are hidden from viewers, and this version can not be shown without them. Ask the bank's author or the circle's owner." See I3. |
| Viewer's copy | PASS. `copy_bank`: `imp.keyless` true and every question carries the keyless sentinel. |
| `keyless_question` | PASS. The viewer, promoted to editor, shares its keyless copy as a new bank through `CircleSendModel`. 422 `keyless_question {positions:[0,1,2], kinds:[Mc,Mc,Mc]}` → `Failed`: "In this copy, questions 1, 2 and 3 have no answer keys ... or mark the correct answer of each of them." A keyless question in a working copy is stopped locally (`SendStage::Keyless`, nothing sent). |
| Author (editor) sends to a hidden-key bank | PASS. `SendStage::Decide` (labels), then Decide (order), then `Done v3`. The viewer then reads v3 stripped. |
| Author turns it off from the page | PASS. `Done` → "Viewers see the answer keys again". The viewer reads the full text. |
| Non-author editor PATCH | PASS. 403 `owner_or_author` → "Only the circle's owner, or the bank's author while an editor, can do that." |
| Decision for question 9 of 3 | PASS. 400 `field: labels` (the bank page maps it to "The bank changed meanwhile ... start again"). |
| Hide on where the viewers' form matches a block | PASS on the wire: `held:true, withheld:true` parsed. **FAIL in the app: B2.** |
| Hide off after that (direct API: the page cannot) | PASS. `restored:[1]` → "Version 1 had been held back when the keys were hidden; it can be read again." The viewer reads the head again. |
| Purge-now with an open review | PASS. 200 `purged_partial`, `evidence_kept:1` → notice "Mostly deleted. 1 version is kept while an administrator reviews them; ... Undo is no longer possible ...". `purge_claimed` set; restore blocked locally. A direct undelete gives 409 `bank_removed`. See I2 for a fresh start. |
| Trim with a withheld version | PASS. `Trimmed{pruned:3, pinned_versions:[2], evidence_kept:1}` → "Done. The content of 3 older versions was deleted." / "1 version (version 2) is kept while an administrator reviews it ...". |
| `review_copy` | PASS. A withheld kept version is `"full"` ("Hidden while it is reviewed"). After evidence was captured and a trim: `"hashes_only"` → flags "Hidden while it is reviewed", "Kept for review as fingerprints only". GET of it: 404 `version_not_retained {withheld, review_copy:"hashes_only"}` → the special words. A pruned version that was never withheld gets the ordinary words. |
| Activity | PASS. 85 lines: 50 + 35, `next_before` passed back opaque, no duplicates, for owner and viewer. `email_invited` → "X emailed an invitation to join as viewer." `hide_key_changed` → "hid the answer keys of \"Race\" from viewers." or "showed ... again" (by `hide_key`). `rolled_back` → "restored an older version of \"Rollback line\" (version 1)." `versions_trimmed` is worded. No "does not know about" line. A cursor the pool refuses (400 `before`) quietly restarts the list (85 → 50 lines, no error). |
| 30 synthetic phase C bodies through `RefusalKind::classify` and `message()` | PASS. Every new code has plain words. The one "RAW" hit was `busy`, a false match on the word; the email form overrides it. |

## Blockers

### B1. Account deletion fails on D1 for every account (backend, round 48)

- Observed: all 20 `Client::delete_account` calls returned **500 `server_error`**.
- The local workerd trace (`/cdn-cgi/local/explorer/api/local/observability/query`, the `d1_batch` span of `DELETE /account`) says: `LIKE or GLOB pattern too complex: SQLITE_ERROR`, on the 47-statement onDelete batch (`deleteContentStatements`).
- Cause: `server/worker/src/pool/circles.ts:2019`, `DELETE FROM pool_alert_budget WHERE channel LIKE 'circle_email_domain:%:' || ?1`.
  - The pattern is a 22-byte prefix plus a 36-character user id, 58 bytes. D1's documented limit for a LIKE/GLOB pattern is **50 bytes** (developers.cloudflare.com/d1/platform/limits).
  - `wrangler d1 execute --local "SELECT 'x' LIKE ('circle_email_domain:%:' || '<36-char id>')"` gives the same error.
  - The same 47 statements run cleanly in plain SQLite (Python, limit 50000).
  - `npm test` cannot see it: `test/pool/harness.ts:34` wraps `node:sqlite`, which uses SQLite's default limit.
- Effect:
  - `server/auth/src/auth.ts:1405-1417` commits the tombstone first, then runs the onDelete batch, which rolls back as a whole.
  - The account is gone, but none of the pool or circle erasure happens. In the local DB:
    - memberships stay active, and an erased owner stays the owner with no succession;
    - its email invites stay unrevoked, with `email_label` and `email_key`;
    - messages, report notes and items are not handled.
  - `erasure_finish` in `src/pool/cron.ts:171-175` re-runs the same batch, so it fails every day.
  - The person who deleted their account is shown an error (a retry then gets 401).
  - Any deploy of 953ef9f would do this on staging/production.
- Fix: no concatenated pattern. For example `channel LIKE 'circle_email_domain:%' AND substr(channel, -(length(?1) + 1)) = ':' || ?1` (21-byte pattern). Also make the test harness's `d1()` shim refuse a bound LIKE/GLOB pattern over 50 bytes, so this class is caught. The resweep lines `circles.ts:2062-2063` use constant patterns of 22/23 bytes and are fine.

### B2. A hide-key change that withholds the head leaves the author no way out, and wrong words

- Path: the author (or the owner) hides the keys. The viewers' form matches a confirmed block (the server checks it: round 46 B2). The PATCH answers `held:true, withheld:true`.
- What the app does:
  - `circles.rs:1674-1679` sets `HideStage::Done` and reloads the bank at the same time (`b.load = BankLoad::Loading`).
  - The head read is 404 `version_withheld`, so the page renders `BankLoad::Failed` and returns before `hide_section` (`circles_ui.rs:1081-1094`).
  - The Done lines are never drawn, including `hide_result_lines`' held paragraph (`circles_text.rs:272`).
  - `hide_allowed()` (`circles.rs:2317`) needs `BankLoad::Ready`, so the "Hide answer keys from viewers" switch is gone too.
  - The author and the owner see only `pool.rs:531`: "This version is hidden while it is reviewed. **Only an administrator can release it.**"
- Observed live: the author's page and the owner's page both show exactly that and `hide_allowed=false`. Meanwhile the bank is listed as held to the owner and the author, and hidden from viewers.
- The server's remedy is turning hiding off (round 47, critic 54a variant 1). Done through the API, it gave `restored:[1]`, released the hold, and viewers read the bank again. The app cannot send it.
- The Done line it would have shown ("Sending a clean version lifts the hold") is also misleading. The full text is clean; sending it again to a bank that hides its keys strips and matches again.
- Suggested fixes:
  - Keep the hide section, with the switch, on the failed page when the failure is `version_withheld` and the person may change the setting.
  - Or do not reload on a held/withheld Done.
  - And word it: "turning hiding off makes this version readable again".

## Important issues

- **I1. One domain's hourly limit blocks inviting anyone (email form).**
  - After 429 `hourly_limit` `circle_email_inviter_domain_hourly` (or `circle_email_domain_hourly`), `f.problem` keeps its `retry_at` when the address is edited.
  - The form disables "Send the invitation" while it is ahead (`circles_ui.rs:745-760`, `waiting`).
  - Observed: after typing an address at another domain, `email_blocker()` is None but the problem and `retry_at` are kept. The page would keep Send greyed for about 48 minutes, although the words say "The others can go later" and another domain would be accepted. Only Cancel and reopening clears it.
  - Fix: tie the wait to the domain (or clear the problem when the address's domain changes); keep the global wait only for `circle_email_invites_hourly`.
- **I2. The partial purge is remembered in memory only.**
  - It lives in `CirclesModel::purge_claimed` (`circles.rs:693`). The server's list entry carries nothing to tell a claimed purge apart: `removed.undo_until` is just the claim time (`circles.ts:448`).
  - After a restart or on another device, the card says "Removed from the circle. The time to undo has passed." and offers "Delete it for good" again (`circles_ui.rs:522`). `purge_pending_line` is never shown.
  - Pressing it again repeats the partial purge, with the same "Mostly deleted" notice: harmless, but the honest state is lost.
  - Fix: have the server add `purge_pending`/`evidence_kept` to `removed`.
- **I3. Viewers are offered versions they cannot open.**
  - For a bank that hides its keys, a history line has no field saying whether viewers can read it (the server knows: `strip_opts IS NULL` gives `circleversions.ts` `lineOf`).
  - The client offers `view_version` for every kept, not-withheld line (`circle_edit.rs:1866-1871`).
  - Observed: viewer history v1 has flags `[]`, and opening it gives 409 `keys_hidden_unavailable`. The words are plain, but the link is a dead end.

## Nits

- **N1.** `invite_line` says "expires in 6 days" right after a 7-day invitation (`circles_ui.rs:959`, `text::until` rounds down). The sent panel deliberately shows the date instead.
- **N2.** An IDN address's label is shown in punycode (`j***@xn--cole-9oa.example`). The server masks the ASCII form.
- **N3.** "emailed an invitation to join as viewer." (`circles_text.rs:161`) has no article, like the older "joined as editor." / "made X viewer.".
- **N4.** `BankHead` still does not read `message`. The server sends it, blank for viewers; harmless (as critic 50 noted).
- **N5.** "Kept for a review" still labels a version that is pinned only because a withheld version broke the run (`circle_edit.rs:2314`; seen as v1 in T1). This is critic 50 variant 5, still open.
- **N6.** `HELD_BANK` says "Only you can see it" to the owner of another author's held bank (pre-existing).
- **N7.** `server/worker/src/pool/shared.ts:512-513` still says that past `circle_email_address` "the invite is made ... but not mailed". Since round 46 the code answers 422 `invite_not_sent` `address_limit`. Comment only.

## Limits and what was not done

- **409 `hide_key_changed` was not reached live.** I made 96 attempts in four sweeps:
  - send first then PATCH;
  - PATCH first then send;
  - delays from 0 to 500 ms;
  - 400-question banks, trimmed to one version;
  - the edit model kept turning on a background runner.

  The isolate's one heavy slot serialises the PATCH's strip and the send's validation. The PATCH always committed either before the send's `freshBank` read (the send was then checked as hidden: Done) or after its commit (no conflict). So the guard is effectively unreachable locally.
  - The real classifier and words were checked: `hide_key:true` → "...Send it again; the pool may ask how the viewers' copy should look.", `false` → "...Send it again.".
  - `circle_edit.rs` has the send and restore arms.
  - A worker test that injects the PATCH between the read and the commit is the way to cover it.
- `circle_email_invites_hourly`, the service-wide 503 `busy` and `circle_open_email_invites` (40) were checked through the classifier only.
- Images in a hidden-key bank (`image_ok`, `images_kept`) and copyright-block matches on a send were not exercised.
- The admin release and evidence-captured routes were simulated with their own writes, not called.

## What was not a problem (checked)

- The request shapes match what the worker validates:
  - `{email, role}`;
  - `{hide_key, labels:[{question, part?, choice}], order_acks, ack_formatting, image_ok}`, with decisions only with `on`;
  - send meta decisions and `keep`;
  - the activity `before`, percent-encoded.
- Decisions from an earlier step go again with the next one, and only listed findings are decided.
- The email address never appears in a list, a `Debug` or a log line of the client. The mail is a fixed text; the join page reads a code from the whole mail or the bare link.
- Every new answer field is read by a matching client field:
  - `resent`, `address_label`, `kind`;
  - `hide_key`, `keys_hidden`;
  - `unchanged`, `held`, `withheld`, `labels_removed`, `images_*`, `figures_removed`, `restored`;
  - `purged_partial` / `pending_review`, `evidence_kept`;
  - `review_copy`;
  - the activity's `hide_key` and `rollback_of`.

  Nothing the client reads is missing on the server.
