# Circles phase C (rounds 42-46), product/functional critic 54a

Scope: uncommitted phase C backend on `shared-pool-phase1` after round 46. Lens: honest teachers.

`cd server/worker && npm test`: 642 pass, 0 fail. The 53 scripts `s4`, `s1`, `e1`, `c1`, `c2`, `s3`, `h1` were rerun and the earlier
blockers hold in the form the scripts test them (flagged keyless copy refused, dots and googlemail folded, stripped form held).
My scenarios (not in the repo) are in
`/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/c54/`
(`t1`-`t6.test.ts`). Run one with `cd server/worker && node --experimental-strip-types --no-warnings <path>` (it prints with
`console.log`; `node --test` hides it).

## Blockers (new class)

### B1. The round 46 keyless guard checks a field the real app does not send, so a promoted viewer's copy still replaces the keys (major, data loss)

`server/worker/src/pool/circlesend.ts:534` (`(q as {keyless?}).keyless === true`).

share-core's own definition of a keyless question is `q.keyless || explanation === KEYLESS_SENTINEL`
(`server/share-core/src/validate.rs:1102`). `wire.rs:21-26` and `import.rs:17` say why: the importing app stores the sentinel
explanation because "older builds drop `keyless` on re-save". The app today has no `keyless` field at all
(`src/model.rs:100-105`: the marker is `is_keyless(q)` = sentinel explanation). So a viewer's copy that the app re-saves and
sends carries the sentinel and no `keyless` flag, and the new guard does not see it.

Repro (`t3.test.ts`, first test): Ann hides keys on a bank (Mercury is the correct answer). Vic (viewer) reads the stripped
head; the owner promotes Vic to editor. Vic sends the same questions as the app would hold them (`keyless` absent, `explanation`
= the sentinel). Result: `200`, version 2, no warning. The owner reads v2: every `correct` is false, Mercury is no longer the
answer. The same body with `hide_key` OFF is refused (`422 keyless_needs_hide_key`), so only banks that hide their keys are exposed,
which is the case 53a B1 was about. 46-1 and the 53a `s4` script pass only because they send `keyless: true`.

Severity: major. It is the same data loss 53a B1 reported (recoverable only by a rollback inside the 20-version window, history
line looks like any edit). Fix: the guard must use share-core's own rule (flag, sentinel explanation, and `origin.form ===
"stripped"` with `rekeyed` not true), ideally by asking `c.core` rather than a second copy of the rule. Add a 46-1 variant that
sends the sentinel-only form.

### B2. A refused hide_key PATCH is charged to the 20 a day cap, so a teacher hits it on the 11th bank (or the 7th)

`server/worker/src/pool/circles.ts:1465` (charge) and `:1617` (refund only for 503 and 409).

The normal app flow is: PATCH, `422 labels_need_decision` (charged), PATCH with decisions (charged), and `422 order_ack_required`
adds a third charge. Repro (`t5.test.ts`): 12 banks, each with one labelled Mc. Banks 0-9 succeed, each using 2 charges;
bank 10 and 11 get `429 daily_limit` "You have changed answer-key settings as many times as you may today." A teacher who
turns hiding on across a semester of banks in one sitting is locked out until 00:00 UTC (5 pm Pacific). With an order
acknowledgement step per bank the lockout comes at 7 banks. The README says only "every change counts" and "same setting again
costs nothing"; it does not say refusals cost. Fix: give the charge back on any refusal (stripRefusal, 422), or charge only when
the batch wins.

## Variants and limits

1. **A PATCH that turns hiding on can withhold the whole bank, and turning it off does not undo that** (`t2.test.ts`). With a
   takedown whose block holds only the unlabelled (served) form, Ann's clean labelled bank (v1, her only version) is PATCHed on:
   `200 {held: true, withheld: true}`; v1 is now `version_withheld` for Ann, the owner and every editor, and the circle page
   shows nothing to others. `PATCH hide_key:false` answers 200 but v1 stays withheld. Recovery: a new send of the same text,
   but the diff against a withheld head reads "removes 2 of the bank's 2 questions" (`428 confirm_large_required`), so the sender
   must confirm a message that sounds destructive; with hide off that send is clean. Two steps, then the keys cannot be hidden
   for that text. The block is correct to hold; the surprise is that the old, readable version is gone. If the head has older
   versions, a rollback also works. Suggest: when the PATCH would withhold the head, refuse it (nothing changes) and say so,
   or let the off toggle re-check and lift a withhold the PATCH itself made. `/admin/dmca/unblock` with
   `release_circle_versions: true` does release both versions and resolve both flags (checked).
2. **Invites in bulk** (`t1.test.ts`). One owner, 15 colleagues at one school: invites 1-10 go; 11-15 are `429 hourly_limit`
   (clear message). An hour later the 11th is `422 circles_limit` "This circle has 10 open codes. Withdraw one first." because
   the 10 mails stay open for 7 days; the owner must revoke someone or wait for redemptions. "Withdraw" reads as un-inviting a
   person. 20 a day, 3 a day per address are fine. Acceptable for a pilot; document "10 colleagues at a time".
3. **A free-mail domain is a site-wide shared pool of 30 an hour.** Three teachers each inviting 10 gmail addresses use it;
   the fourth teacher's first gmail invite is `503 busy` "Too many invitations are being sent right now" (`t1b`), which reads as
   a site outage. Three approved accounts can block every gmail, outlook or yahoo invitation for an hour. The privacy trade
   for a domain count is understood; use a different message and consider exempting the big free-mail domains from the per-domain
   cap (they are already bounded by the per-mailbox cap).
4. **Masked labels collide for colleagues** (`t1`): `j***@pvhs.example` three times for jsmith, jjones, jlee. The owner cannot
   tell which to revoke. Show the first two characters, or the local part's length, or keep a label the owner types.
5. **`address_limit` is an answer about other people's activity**: `422 invite_not_sent` tells any owner that three invites went
   to that mailbox today from anyone. No account oracle, but a small cross-circle leak. The owner's way out ("give the
   person a code yourself") works but the three mails already use 3 of the 10 open codes until revoked.
6. **The mail**: understandable and safe (no names, no free text; says an approved account and a redeemed pool code are needed;
   expiry to the minute). The recipient is not told who invited them (deliberate), so a mail from a sender they do not know
   has no way to be checked except by the app's preview. The link `APP_URL/circles/join#code=` has no route in the repo and
   `wrangler.toml:41` still has `APP_URL = "https://app.example.org"`: staging mail would carry a dead placeholder link. Keep the
   link out until the client route exists, or fix APP_URL before any staging mail.
7. **Withheld versions with an open flag are pinned without any limit; the byte cap can fill with them** (`t4.test.ts`, circle
   bytes set to 60,000): one author sending 14 versions that match a takedown (each `200 held`) pins all 14. The 15th send
   and a clean 4-question fix are `413 quota_exceeded` whose text says the owner can trim. Owner trim prunes only 1 (the
   readable one); `pinned_versions` stays empty in the answer (it lists readable versions only); purge of a deleted bank is
   `409 pending_review`. So the only way out is the administrator deciding 14 flags. At the default 200 MB it needs large
   banks, but a hostile editor, or an honest author retrying a false positive with images, can do it. Suggest a per-bank limit on
   open evidence versions (for example refuse the next held send past 5 until the admin decides), and say in the 413 that
   held versions need the administrator.
8. **Viewer hidden reads are charged before the strip and not given back for a 409** (`circlehide.ts:224`; only 503 refunds). A
   viewer polling a bank whose head lacks recorded decisions (after a rollback to a pre-flag version) spends the 100 a day on
   a bank she cannot open. The author's single PATCH repairs the head (`t6.test.ts`: rollback to v1, viewer 409, PATCH with
   decisions 200, viewer 200); the owner can do it too. Older versions that never got decisions stay 409 for viewers.
9. **Editors' decisions replace the author's** for the version they send and are not readable back (`t6`: Eli's send needs
   `labels` and then `order_acks`, two 422 rounds). Known, documented; the client must compute them every time.
10. **A promoted viewer cannot send her copy at all** (even after B1 is fixed) unless the app clears the keyless marker when she
    re-keys; the worker cannot tell a re-keyed question from the sentinel except by the sentinel being gone
    (`src/qedit.rs:452-455` drops it on re-key). Say in the client: "get a full copy from an editor" is the only path for an
    unkeyed question.
11. **Activity cursor and migrations**: the rule `<at>:<audit id>` and the 400 on malformed cursors read right in code and in
    45-1..4; I did not find a new problem. 0111 and 0112 are `ADD COLUMN` with defaults; 0113 copies every `circle_holds` row,
    drops, renames and recreates the index; no foreign key or view names the table. Each file is atomic in D1; numbering is
    consecutive after 0110. Not re-runnable by hand (normal).

## Nitpicks

- `409 conflict`/`428` wording on a withheld head ("removes 2 of 2") will frighten an honest author.
- A refused PATCH (`labels_need_decision`) is a normal answer yet counts like a change, README should say.
- README "Round 42" bullets still describe the round 42 wording in places (they are marked superseded by Round 46).
- Viewer 429 for hidden reads now has a message; good.

## Verdict

Two new-class blockers: B1 (the keyless guard misses the app's real marker, so a promoted viewer can still overwrite the keys)
and B2 (refused PATCHes spend the daily cap, so normal bulk use is locked out in half a day). The rest are limits.
