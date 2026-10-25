# Circles phase C (rounds 42-45), product/functional critic 53a

Scope: uncommitted phase C backend on `shared-pool-phase1` (migrations 0111-0113, `circlemail.ts`, `circlehide.ts`, edits in
`circles.ts`, `circlesend.ts`, `circleversions.ts`, `circleadmin.ts`, `poolmail.ts`, `cron.ts`). Lens: would honest teachers and the
later client work correctly, and does it agree with `docs/circles-plan.md` (D5, D12, D13, 13.3).

`cd server/worker && npm test`: 630 pass, 0 fail (240 s). My scenario tests (not in the repo) are in
`/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/c53/`
(`s1.test.ts`, `s1b.test.ts`, `s3.test.ts`, `s4.test.ts`, `s5.test.ts`, `s6.test.ts`); run each with
`cd server/worker && node --experimental-strip-types --no-warnings --test <path>`.

## Blockers (new class)

### B1. A keyless copy sent to a hide_key bank silently replaces the keys with blanks (major, data loss, recoverable only by rollback)

`server/worker/src/pool/circlesend.ts:646-651` (`publishPrepare` called with `hideKey: hide`), reached via a normal PUT.

Scenario (repro: `s4.test.ts`): Ann (author) hides keys on her bank. Vic (viewer) takes a copy (keyless, `keyless: true`, every `correct`
false, `Short.sample` ""). The owner promotes Vic to editor (a routine request). Vic edits a stem and presses "Send my changes".
Result:

- `PUT` returns 200, `version: 2`, `changed: 2`, `keys_hidden: true`. No warning, no `confirm_large_change`.
- Ann and the owner now read v2 with every Mc `correct:false` and `sample:""`. The bank's only full-key text is gone from the head.
- With `hide_key` OFF the same body is refused (`422 keyless_needs_hide_key`, "set the correct answers yourself before publishing"),
  so the server already knows a keyless question is not a valid full text. With `hide_key` ON, `publishPrepare` accepts it
  because the public path's rule is "a keyless copy may be published hidden". In a circle that rule is wrong: D12 says every
  editor and the owner read the full bank and "editors cannot edit without" the keys. The stored full body is the only copy of the keys.
- Recovery is a rollback to v1 by an editor, only if someone notices, and only inside the 20-version window. The history line
  looks like any edit.

Fix direction: for circle sends, refuse any question with `keyless: true` (new error, say so plainly), whatever `hide_key` is.
Also worth a test: the same send by a former editor demoted and re-promoted.

No other new-class blocker found in email invites, copyright holds, cursor or migrations.

## Variants and limits

### Email invites (round 42)

Ran: `s1.test.ts`, `s1b.test.ts`, `s6.test.ts`.

1. **Mail silently not sent past 3 per address per day (as documented), but an honest owner trips it by resending.** Repro: five
   `invites/email` to `slow@x.example` in one day give 202, 202, 202, 202, 202; `mails total` for that address is 3; the list shows
   five identical `active` rows. An owner who resends because the first mail "did not arrive" (spam folder) gets no mail from the
   4th try and no sign of it. Because the per-address count spans circles and inviters, **anyone can use it to suppress a
   colleague's invitations**: three invites from any approved teacher to the target's address use up the day, and the real
   department chair's invite is accepted (202) and never mailed. Low-cost, silent, targeted. Suggest: tell the inviter
   "not sent today, try tomorrow" only in the shape `mailed: false`, which leaks nothing about other senders, since the count is
   already per day; or count only mails sent by distinct (circle, inviter) differently. If the silence stays, the client must
   not say "mail sent".
2. **The owner cannot tell email invites apart.** An email invite lists `kind: "email"`, `note: ""`, no address or label, by design.
   With four pending invites the owner cannot say which to revoke if one was mistyped. Client can keep a local label; document that.
3. **Invite to a full circle is accepted and mailed** (`circle_members` 2, two members: 202, mail sent). The invitee then gets
   `422 circle_full` ("This circle has 2 members, the most allowed."), a clear message. The owner gets no warning at invite time.
4. **Existing member invited** gets a mail; their code stays `active` (counts toward the 10 open codes) for 7 days; using it answers
   `already: true` and uses nothing. Fine, but it eats an open-code slot until revoked.
5. **Owner leaves / ownership changes:** all of that owner's pending email codes are revoked (round 33 rule). The invitee holding
   the mail gets "Ask the circle's owner for a new one", and the mail named no inviter, so a new owner does not know who was meant.
6. **Hourly site budget 100** (503 `busy`, Retry-After 3600): five accounts at 20 a day can exhaust an hour and block every other
   owner's invitations for up to an hour. The message ("Too many invitations are being sent right now") is honest. Acceptable for
   an invite-only pilot; note for later.
7. **The mail is a relay for 80 characters of the sender's text** (`s6.test.ts`): a circle named
   `Your account is locked: verify at https://evil.example/login now"` is mailed verbatim, quoted, from the service's address, to
   any address, 3 a day each, 20 a day per inviter. README lists this as a decision for the owner; I rate it a real, if small,
   phishing path. Cheapest fix: refuse `://`, `www.` and `@` in circle names, or drop the name from the mail and rely on the
   preview after the code is typed.
8. **Pool is invite-only, the mail does not say so.** An invitee with a normal approved account but no pool invite redemption
   gets `403 invite_required` ("Enter the invite code an admin gave you") on join; the code is not used. The mail only says
   "approved verSHion account". Repro: `s1.test.ts` (`uninvited join`). `account_not_approved` is handled with a clear message.
9. The mailed link `APP_URL/circles/join#code=` has no route in the repo (no web app route; grep finds only the API calls in
   `src/share/circles_api.rs`). README says so. The client heading is "Join a circle with a code", the mail says
   `"Join with a code"`. `wrangler.toml` still has `APP_URL = "https://app.example.org"`.
10. Address rule is strict ASCII (typed `Ünï@c.example` gives 400 `field: email`, "This is not an email address we can send to.").
    Acceptable. `+tag` is collapsed for the cap (`kim+x@` = `kim@`), which also hits plus-addressed shared mailboxes.

Exact mail text (verified by running `circleInviteText`; subject `verSHion: you are invited to a circle`):

> You are invited to join the circle "Algebra team" on verSHion as an editor. A circle is a small private group of teachers who share question banks.
>
> To join, you need an approved verSHion account. In verSHion, open Shared, then Circles, then "Join with a code", and enter this code:
>
> C-31RG-T66Y-7JQB-FDAY-F7FD-SGW0
>
> Or open this link:
> https://app.example.org/circles/join#code=C-31RG-T66Y-7JQB-FDAY-F7FD-SGW0
>
> The code works once and expires on 2026-10-27 (UTC). If you did not expect this invitation, ignore this mail: nothing happens unless the code is used.

Neutral and short. Missing: who invited you (deliberate), the pool invite code requirement, what to do if you have no account.
"Expires on <date> (UTC)" is really 12:00 UTC of that day for a mail sent at noon; "at the end of" would be wrong either way; say
"in 7 days" or give the time.

Covered and fine: no account-exists oracle (identical statements and answer for member, stranger, own address; test 42-2),
editor cannot invite (`403 circle_role_required`), bad role 400, closed circle `circle_closed`, revoked and expired codes give the
usual `circle_invite_invalid`, used-up code used by the same member is `already: true`, pending/uninvited joiners are refused before
the code is spent, 20/day (`429 daily_limit`) and 10 open codes (`422 circles_limit`, all counts given back), invites list and
export carry `kind`.

### Hide keys (round 43)

Ran: `s3.test.ts`, `s4.test.ts`.

1. **What a viewer gets (usable):** stems, points, Mc choices (shuffled), Tf (value false, hence no key), Blank with `accept:[[]]`,
   Matching with lefts and a shuffled word bank in `distractors` (rights ""), Short `sample:""`, no explanation, `keyless: true`,
   qids kept. Printable as a worksheet; not auto-gradable (every answer is wrong against the blanks). Viewers lose: keys, explanations,
   figures, commit messages (blank), and (per README) images unless `image_ok`. The client must say this plainly on the "get a copy"
   screen; the server gives it `bank.keys_hidden: true`, `question.keyless: true`.
2. **Shuffle is fresh each read, ETag is not content-based.** Two consecutive viewer reads gave different Mc and Matching orders under
   the same ETag. So a viewer's "get latest" diff between two reads of the same version will show phantom answer-order changes unless
   the client compares only by version/ETag. A conditional read after the ETag matches returns 304 (no strip, not counted), good.
3. **ETag stale after the author re-records decisions.** `s3`: Ann re-records with `labels: keep`; Vic's `If-None-Match` with the old
   ETag still gets 304, and the new ETag is identical, though the served labels changed (`Red/Blue/Green` became `A. Red/B. Blue/C. Green`).
   The head JSON hashed for the ETag does not include `strip_opts`. Low severity (decisions rarely change), but a viewer keeps the
   older form until the version changes. Fix: add a short hash of `strip_opts` to the ETag input.
4. **Decisions cannot be read back.** There is no route that returns the recorded decisions (README: no preview). Every later send to
   a hide_key bank must supply `labels`, `order_acks`, `image_ok` again or get `422 labels_need_decision`; the client must recompute
   them. In `s3`, a non-author editor (Eli) answered the label question differently from the author (he kept `A./B./C.` on Ann's
   question 5) and that changed what viewers see, with no notice to Ann. Decisions are positional (question index), so any
   reorder in a merge changes which question they refer to. UX works (the 422 body carries `label_findings`, `needs_order_ack`), but
   it is an annoying loop for the client; consider carrying the previous version's decisions forward by qid when a send gives none.
5. **Older versions sent before the flag:** viewers get `409 keys_hidden_unavailable` for any version whose recorded decisions the
   gate refuses, while `history` still lists it `kept: true` for them. The author's PATCH repairs only the head. Not a dead end for
   the head; a permanent 409 for old non-head versions with undecided labels. Acceptable; the client should hide the open button on 409.
6. **Rollback copies the target's decisions** (works; viewer reads after rollback 200). A rollback to a pre-flag version with an
   undecided label gives viewers 409 until the PATCH (documented).
7. **Merge/history interplay:** viewer `merge` is `403 circle_role_required` (editor needed); editors merge from full text; no
   leak found; viewer history messages are blank. An editor merging from a hide_key bank must send decisions (see 4).
8. **Daily cap:** `circle_hidden_reads` 100/day gives `429 {"error":"daily_limit","metric":"circle_hidden_reads"}` with Retry-After
   to UTC midnight (5 pm Pacific) and no `message`, like every other daily_limit. 100 is fine for browsing; a client that re-reads
   every linked hidden bank at each launch (15 banks x 8 launches) would exhaust it, so it must use `/circles/heads` first.
9. Viewer `HEAD` has no `Content-Length`, as documented (editor HEAD has one). `keys_hidden` appears only to viewers of a hide_key
   bank; `hide_key` to every member. Consistent.
10. A viewer demoted from editor keeps any full copy she already took (unavoidable; not stated in README).

### Copyright block on sends (round 44)

Ran: `s5.test.ts`.

1. **Honest sender with one false positive among many has no way to find it.** A 30-question first send with one blocked question:
   201, `held: true, withheld: true`, no positions. The sender then reads `404/version_withheld` for her own bank, the circle page
   lists it with `title: ""`, and history shows `kept: false, withheld: true`. Her only exits are: delete the bank and resend a
   guess, or wait for the administrator. This mirrors the public path ("held for review") and was chosen to avoid an oracle, but in
   a 1,000-question bank it is a real dead end. Suggest: allow the sender (not other members) to see the positions count or,
   better, a sender-only `GET .../versions/:n/held` that returns positions only, rate-limited by the existing `held_publishes` budget.
2. **Unblock does not free circle versions.** After `/admin/dmca/unblock` (complainant withdrew): author still reads
   `version_withheld`; the `blocklist` flag stays unresolved; `circle_holds` still has `copyright/blocklist`; reader 404. A new
   clean version lifts the hold, but the withheld version stays until an admin releases it. This is documented, but it means a
   withdrawn takedown leaves honest teachers locked out until an administrator walks the flag queue; the flag detail does not say the
   block is now lifted. Suggest: on unblock, resolve (or mark) `blocklist` circle flags whose only matching hashes were that action's,
   or at least show "block lifted" on the admin flag.
3. The release path works as designed (`circle-release {bank_id, version}`, 44-1 asserts it). Consistency with student data holds:
   same hold source set, same member-visible answers, same purge refusal wording; the two member messages no longer name a reason.
   Not retroactive (documented).
4. Block keys mirror the public path (match, normalised skeleton, stem, near, explanation, answer, parts). The public path also
   uses `unitKeysOf` for licence evidence, not for blocking; I found no gap.

### Activity cursor (round 45)

Ran: `s5.test.ts`.

- `before` values `abc`, `1`, `12345:`, `:x`, `-1:aa`, `1:2:3`, `99999999999999999999:zz`, `" "`, repeated `before` all give 400
  `{field: "before"}`. A round-32 cursor `<at>:<rowid>` also gives 400, so a client paging across the deploy gets one error and must
  restart; an old client that stored a cursor across sessions gets 400 instead of page one. Acceptable (opaque, short-lived).
- If the cursor's audit row has since been deleted, the `rowid <` subquery is NULL and the page drops same-millisecond lines
  silently (benign).
- New kinds `email_invited`, `hide_key_changed` have no address, id or key material; erased actors show as the deleted-teacher name.

### Migrations, caps, README

- **0113 rebuild of `circle_holds` does not lose data:** same columns and PK as 0108, `INSERT ... SELECT` of every row, same
  `circle_holds_at` index recreated; no triggers or views reference the table. D1 runs a file atomically; a failed apply leaves no
  `_r44` table. 0111 and 0112 add defaulted columns only. Numbering 0111-0113 is consecutive after 0110. Not re-runnable by hand
  (`CREATE TABLE circle_holds_r44` would fail on a second run), normal for numbered migrations.
- Cron deletes `circle_email_sends` for days before yesterday (kept at most two days), matching the README.
- Caps: 20 email invites/day, 3/address/day, 100/hour site, 100 hidden reads/day, 10 open codes. Reasonable for a pilot, with the
  caveats in 1 and 6 above (silent suppression, shared hourly pool).
- README claims checked against code: route and fields, 202 shape, caps, kind in list and export, hide_key semantics, 409 text,
  HEAD, 304, cursor, copyright flow. I found no false claim. One omission: README's hide_key section does not say that a keyless
  copy sent back is accepted (B1).
- Plan items still open (as intended for "safe parts"): the allowed re-share route with origin credit, the DMCA takedown machine for
  circle banks (notice, quarantine, counter-notice, strikes). Nothing else from 13.3 is dropped.

## Nitpicks

- Mail says "Join with a code"; client heading is "Join a circle with a code".
- Mail does not mention the pool invite requirement or what to do without an account.
- `circle_hidden_reads` 429 has no `message`.
- A withheld-first-send bank lists with an empty title for its own author.
- Existing-member invitation burns an open-code slot with no way to know which one.
- `README` "Round 42" says the inviter "is not told a mail was not sent"; fine, but the client needs a sentence for it.
