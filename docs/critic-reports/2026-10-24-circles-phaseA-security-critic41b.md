# Critic 41b: circles phase A after the Round 33 fix round, security / ASVS / abuse / privacy lens

Scope: the uncommitted circles phase A backend in `server/worker` (migration 0108, `circles.ts`, `circlesend.ts`,
`circlecheck.ts`, `circleadmin.ts`, the changed pool files, README Round 32/33) against `docs/circles-plan.md` and the
reports `...-moodle-critic40a.md` and `...-security-critic40b.md`. Baseline: `cd server/worker && npm test` gives
**543 pass, 0 fail** (run first, before any of my scripts; no repo file was changed except this report).
My throwaway scripts (outside the repo, run with `node --experimental-strip-types --no-warnings --test` from `server/worker`
on `test/pool/harness.ts`) are in
`/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/c41/`
(`v41.test.ts` F1-F6, `v41b.test.ts` G1-G3, `v41c.test.ts` T1-T5, `v41d.test.ts` G4, `v41e.test.ts` fuzz).

## Verdict

**No new blockers found.** Every blocker from critic40a and critic40b is fixed, and I could not reopen any of them with my own
variants. I found eight variants (F1 to F8 below) that the README does not list and that I think the owner should see before the pilot; the two closest to a blocker are F5 (the new owner purge route) and F1 (the publish check still
names hidden banks). Each is a trusted-member or narrow-precondition issue with a small fix.

## Step 1: each earlier blocker, re-run

| Earlier finding | Script and result now |
|---|---|
| 40a B1 / 40b B1 rejoin after removal with the same code | `crit1` S2, `atk1` S1: `422 circle_invite_invalid`. Own variant: preview with the old code is the same 422 (T1). A code issued after the removal lets the member back (33-1 test and mine). Demote-then-leave-then-old-editor-code returns `viewer` (`crit4` D1). A member who simply left with an old code gets the same role (viewer stays viewer). |
| 40a B2 codes of a former owner | `crit1` S4, `crit4` D2: stranger with the departed owner's code gets 422 after transfer and after leave. Own variant: erase, remove, transfer-and-back: revoked rows stay revoked (revocation is permanent, a returning owner needs new codes). |
| 40a B3 circles_owned through a transfer | `crit1` S1: `422 circles_limit circles_owned 5/5`. Cap is in the step-down statement (`circles.ts:773-775`), so a race cannot pass it (their 33-3 interleaves every statement boundary). |
| 40a B4 hold vs purge | `crit2` P1 now shows the bank kept with its hold after 15 days. Own T3: removed bank with an open report, or with an admin hold, answers `409` on the new purge route; a bank purged and undeleted in parallel ends purged, 0 versions, 0 R2 keys. |
| 40b B2 owner-only restore | `atk5` S8: author undelete of the owner's removal is `403`. |
| 40b B3 publish check bounded | G1: user with exactly 100 membership rows, all holding records, public publish of a 1,000-question bank: 201, 134 statements (control user with none: same order of time). Joins and creates stop at 100 rows (`F4`: `circle_memberships_total`). A bank is capped at 1,000 questions (`too_many`), so the check is at most 2 chunks of 1,000 hashes x 100 circles. |
| 40b B4 send memory | `atk3`: dense 760 KB payload now reserves 37,244,690 bytes at the share-core call (was 12.2 MB); `circlesend.ts:143` is the publish formula (`publish.ts:808`). |
| 40b V1 activity titles | `atk1` S2: `bank_title: null` for a held bank and for a removed bank, to a viewer. |
| 40b V2 prune vs hold/report | `atk2` S3: version 1 stays `previous` after 21 more sends on a held, reported bank. |
| 40b V3 cap flood | `atk2` S4/S5 still `circles_limit` until purged, but the owner purge route exists and works (T3, F5). |

Other things I attacked and that held: removed and former members got `404` on every one of 10 circle/bank routes (read, HEAD,
blob, report, leave, undelete, delete, patch), on `send`, and `unavailable` from `heads` (T1); cross-circle bank ids on
get/purge/delete/report/PUT all `404` (T1); concurrent double use of a single-use code: one 200, one 422, `uses = 1` (F6);
754 malformed bodies and cursors over 12 write routes and 2 list routes (`fuzz`): no 5xx, still exactly one owner;
circle responses carry `Cache-Control: no-store`, `Vary: Authorization, Cookie, Origin`, `nosniff` (T5); the only inserts into
`circle_members` are `createCircle` and `joinCircle` (grep), so nobody can be put into a circle, or have a membership row
created, without their own request.

## Blockers (new class)

None.

## Variants and limits to document (not in the README's known-limits list)

### F5. The new owner purge route lets an owner reset "first sender" at once and take a member's questions public (medium)
`circles.ts:1158-1181` (`purgeBankNow`), `:1381` (`purgeBank` deletes the bank's `circle_qkeys`).
Authz is right: editor/viewer `403`, outsider and admin `404`, not-removed `409`, held or reported `409`, other circle's bank `404`
(F5 script). The problem is what it does with D13. Repro (`v41.test.ts` F5): editor E shares bank B; owner O reads it (as any member
may), deletes it (`owner_removed`; since round 33 only O can undo that), calls `POST .../purge` (200), sends the same text as O's own
bank (201, O is now "first sender"); E's public publish of her own text is then `422 circle_material`, O's is `201`.
Before round 33 this needed 14 days (documented by critic40a as variant 4); now it is one request, the author has no window, and the
purge is not in the activity list (`circle.bank_purge` is outside `ACTIVITY`; members see only "bank removed").
Why not a blocker: needs a hostile owner (a trust-model case), copying was already possible by hand, every step is audited.
Fix direction: keep a hash-only tombstone (`circle_id, match_hash, first_by, first_seq`) when a bank is purged by anyone but the
author's own erasure, or let the immediate purge apply only to banks whose author has left or been erased, or give the author 24 hours.

### F1. The public publish check still hands out titles of hidden banks, and to removed members (medium-low)
`circlecheck.ts:158-172` (names query has no hold/removal/membership-time test), `:53-54` (`my` includes every former membership).
Repro (F1): X is removed from a circle; later the owner shares "Jane Doe IEP roster", a reporter holds it for student data; X (removed)
and M (active, cannot see the bank on the circle page: `banks: []`) each publish the same question text publicly:
both get `422 circle_material` with `circle_name`, `bank_id` and `"bank_title": "Jane Doe IEP roster"`. This is the same class as 40b V1
(a hold exists to hide that title), on a channel round 33 did not touch. Precondition: the caller must already hold the exact text, so it
confirms and names, it does not reveal content. The same check also keeps a removed member as an existence oracle for the circle's
whole later content for as long as the circle lives.
Fix direction: return `bank_title`/`circle_name` only for banks the caller may see now (member, not held, not removed), and for former
members count only material first sent while they were members (`first_seq` up to their leaving; that is also the only material they could
have copied, so it removes false refusals for them too).

### F2. Standing: a suspended or revoked account keeps its circles open and its banks served (medium-low)
`cleanup.ts:11-13` (suspend only writes the public pool's `author_suspended` audit; public reads join the author's standing), nothing in `circles.ts` does.
Repro (F2): owner suspended; a stranger joins with the owner's still-valid 30-day code (`200`); a member reads the suspended author's bank (`200`).
`CODE_USABLE_SQL` (`:132-134`) only asks that the issuer is the active owner, `successionStatements` (`:159-166`) and `transferOwnership`
do not look at `suspended_at`, so an owner who cannot act (suspended accounts cannot authenticate) still blocks every owner action and
a succession can pick a suspended or tombstoned account. The admin has `circle-close` and `circle-remove` by hand; the plan does not decide it.
Fix direction: decide the policy (revoke the suspended issuer's open codes in `onSuspend`, skip suspended accounts in succession,
treat a suspended author's circle banks like held ones), or document that the admin must act.

### F3. Ownership is forced on the receiver (low-medium)
`circles.ts:755-797`: `transfer` needs no acceptance; a succession also lands without consent. Repro (F3): attacker invites victim as a viewer to
five circles, transfers each to the victim: victim owns five, `POST /circles` answers `422 circles_limit circles_owned`. The victim can leave to shed
each one, but then succession gives it back to the attacker (it ignores the cap). The victim is also the owner the admin sees for content they never chose.
Not a membership-slot burn (that needs the victim to join every circle themselves). Fix: a pending transfer the target accepts, or at least show it
in the activity list / a notice.

### F4. Membership rows are permanent while the circle lives and the member cannot drop them (low)
`circles.ts:126, 305, 630`. One account alone, with no helper, reaches 100 rows in 20 days (create 5 a day, leave each; F4 output: refused at 100,
`circle_memberships_total`) and is locked out of create and join until those circles purge 14 days after closing (201 after 15 days). Not usable
against someone else (no non-consensual row creation), but a teacher who joins and leaves circles that stay open will meet it with no way to clear it.
Document, or offer "forget this circle" (note: it would shrink the D13 check for that user).

### F6. The owner's "delete" of a bank its author already removed is a silent no-op (low-medium)
`circles.ts:1106-1108`. Repro (T4): author deletes own bank; owner calls `DELETE` (200, `undo_until` shown); `removed_reason` stays `author_removed`;
the author undeletes (200). The B2 fix assumes the reason records who decided last. Fix: upgrade `author_removed` to `owner_removed` when the owner deletes, or return
`409` pointing to purge.

### F7. Insider hold griefing and the shared instant-hold budget (low)
`circles.ts:1211-1227`, `shared.ts:444` (`instant_holds_global` 50). A member in good standing (account 7 days, member 1 day) hides any other member's bank at once with
one `student_data` report, 3 a day; the global 50-a-day instant-hold budget is shared with the public pool, so about 17 cooperating approved accounts can use it up each day and
silently turn real public student-data reports into queue-only. The mail budget (`privacy_report`, 20 an hour) is shared the same way (40b V10, still open). Needs approved accounts
(invite gate), so low; document.

### F8. Suppliers of unbounded growth (low)
* `exportCircles` (`circles.ts:1442-1454`) has no LIMIT and no `reserveBytes`: `versions_sent` grows 100 rows a day for one account (history rows of pruned versions are never deleted
  while the bank lives, message up to 500 characters); after about a year of maximum use ~36,000 rows (~20 MB of JSON, more as objects) go into one response inside the isolate.
* Purged banks leave a tombstone row forever (`circles.ts:1385-1387`); `GET /admin/pool/circle` (`circleadmin.ts:98-106`) lists all of them and all membership rows with no LIMIT.
* Activity junk scan (40b V4, README-documented): measured 40,000 non-activity audit rows newer than the real events: one page read all of them (9 ms in SQLite, 40,000 `rows_read`).
  Rates make this a year of an owner writing ~100 rows a day; still the unbounded-scan shape.

### Smaller disclosures
* `transferOwnership` answers `422 circles_limit {used, limit}` with the target's count of owned circles (`circles.ts:791-793`): an owner learns how many circles another member owns.
* `heads` (`circles.ts:910`) tells any member `removed` plus `until` for a bank the circle page hides from them (T1: viewer on an owner-removed bank).
* `exportCircles.memberships` gives former members the circle's current name.
* Mail: ids and an enumerated reason only, no injection (`poolmail.ts`); `circle_closes` cap alert is deduplicated per user per day. Nothing found.

## Nitpicks

N1. `transferOwnership` has no ALIVE test on the target (`circles.ts:773-775`). A target whose account is tombstoned but whose content hook has not run becomes owner (G2: `200`); the
hook then hands the circle on (it did: back to the old owner). Between those two the circle has no usable owner. Same window for any succession pick.
N2. Code-versus-removal ordering is wall-clock (`left_at >= created_at`, `circles.ts:133`), computed before each batch commits. Two owner requests overlapping by a few ms can
order the other way round; a code issued a moment before a removal then counts as "after". Not reachable by the removed member. `join_seq` would be exact.
N3. `purgeBank` deletes holds (`circles.ts:1383`); an admin hold placed after the cron selected a bank but before `purgeBank` runs is lost. Window of milliseconds.
N4. `circle-release` dismisses the reports it releases (`circleadmin.ts:206`), which starts the 30-day instant-hold throttle on honest reporters.
N5. Client note for phase A's app: joining must stay a button after `join/preview` (a deep link that auto-joins would let a stranger put a row into someone's 20-circle cap and 100-row cap).
N6. `closeCircle` counts closes through the actor's whole audit index (`circles.ts:431-434`) every time (carried from 40b N1).
N7. D13 coverage is correct at the cap, not free: at most 2 x 1,000 hashes x 100 circles = 200,000 index seeks per publish or preview (previews 200 a day). Bounded; mention it in the operator cost notes.

## Areas checked with no finding

Invite codes (120 bits, one 422 for every refusal, 10 failed tries a day, preview refunded on success only); erasure statements are idempotent and guarded by `ALIVE_SQL` on every write
I read, including the new purge route; successor selection (smallest `join_seq`, re-join resets it); one-owner index; no string-built SQL from request data (the interpolated ids are
`USER_RE`/route-regex/`newId` values); `/circles` bodies skipping the 8 KB pre-read (`preread.ts`, `index.ts`): unauthenticated and pending callers are refused by the pool before any body is
read, an unmatched `/circles/...` path is not read and answers 404; circle images are not reachable by `/blobs`; circle content never reaches `served`, search or the public export.
