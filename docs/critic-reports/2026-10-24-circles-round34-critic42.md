# Critic 42: circles Round 34 (uncommitted backend, server/worker)

Scope: migration 0109, circles.ts, circlecheck.ts, cron.ts, index.ts, round34 tests, README Round 34. `npm test`: 550 pass.
Scratch repros (not in repo): `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/c42/s1.test.ts`, `s2.test.ts` (harness.ts; run with `node --experimental-strip-types --no-warnings --test --test-reporter=spec <file>`).
No security break found (no authz hole, no forced ownership, no way to resurrect removed content, no cap bypass beyond the documented succession one). Two functional/robustness findings below.

## Blockers (new class)

### B1. AUTHOR_OK hides REMOVED banks from the owner: the owner loses the list for erased and suspended authors (medium, functional regression of round 33 V3)
`circles.ts:389` (`if (b.author_ok !== 1) return false;` runs before the `removed_at !== null -> mine || owner` rule) with `AUTHOR_OK_SQL` at `circles.ts:130`.
- Erased author who chose `keep_on_delete: false`: banks are `removed_reason='account_deleted'`, `anonymized=0`, so `author_ok = 0`. Repro S1: after the erase, the owner's `GET /circles/:cid` returns `banks: []` (before round 34 the owner saw removed banks). `POST .../purge` by id still returns 200, but the owner cannot discover the id; the bank still counts toward the 100-bank / 200 MB / question caps for 14 days. The purge-now route (round 33 V3) documents "by its author's erasure" as a target.
- Suspended author: repro S1b: a bank the author removed is listed to the owner (1) and vanishes after the suspension (0). The owner cannot see, undelete or purge it while suspended.
- Fix: apply `author_ok` only to non-removed banks (`if (b.removed_at !== null) return mine || owner;` first, then the `author_ok`/held rules), or show removed rows to the owner regardless of author standing (their title is already visible to the owner). Same ordering question in `activity` titles (`circles.ts` ~947): an owner no longer gets the title of a removed bank of a suspended/erased author.

### B2. Offer/cancel have no daily cap: unbounded audit rows (medium, new write path)
`offerTransfer` (`circles.ts:795`) and `cancelTransfer` (`:873`) each write a `pool_audit` row per call with no `requireDaily` (every other owner write that audits has one: invites 50/day, sends 100/day, creates 5/day). Repro S2: 140 offer + 140 cancel in one run, 280 audit rows, all succeeded. Only the account rate limit applies (300 requests / 5 min, about 86k rows a day from one owner, forever, one row per request, even for an identical re-offer).
`circle.transfer_offer` and `circle.transfer_cancel` are not in `ACTIVITY`, so they are exactly the "non-activity audit rows between events" the README already lists as a read-cost limit (F8): `activity()` walks `pool_audit_item (item_id, at)` newest first filtering by action, so an owner can make every member's activity call scan tens of thousands of rows (D1 rows-read cost, latency) until 51 activity rows are found. Before round 34 the same flood needed a capped action.
- Fix: a daily counter for offers (for example 10 a day, `requireDaily` + `giveDaily` on failure like invites), and do not audit an identical re-offer (same target, offer still open); cancel is bounded by offers.

## Variants / limits to document

1. Succession in an owner-closed circle ignores `circles_owned` (S4: a member who owns 5 now owns 6 for up to 14 days). Same class as the documented open-circle succession; the README says "a succession does not [count]" but the closed case adds a way to dump a dying circle on a member. They can leave again (it hops on to the next member). Document next to the existing line.
2. Nobody in good standing left: the circle closes `last_member_gone` and is purged on day 14 with its banks. A restored suspended member cannot reopen it (S3: `reopen` 403 `circle_role_required`; purged at day 14, banks gone). A suspension is advertised as reversible, but this path destroys content on a timer. Contrast the owner-closed case, which does come back (T5: restore, daily net picks the member, reopen 200). Document, or let the daily net also reopen/hand a `last_member_gone` circle on restore inside the window.
3. Suspension is observable by members: a suspended author's banks all vanish from the circle page, `heads` says `under_review`, publish-check hit is the generic shape (T4). A hold and a suspension look the same per bank, but "every bank of one person vanished at once" tells members that person was sanctioned.
4. The owner learns standing of members: `POST /transfer` answers 409 `member_unavailable` vs 202 (suspended / not approved member), and succession silently skips. Minor privacy leak to the owner only.
5. The publish check's generic hit (null ids) vs an ids-only hit lets a member who holds the text tell "held or suspended-author bank" from "removed / former member / closed". She already has the text; refusal is unchanged. Acceptable, say so.
6. An offer made before a suspension stays acceptable (S5: owner suspended, offer to a member accepted, 200). Arguably desirable; note that a suspended abusive owner can still pass the circle to an accomplice for 7 days. There is no "block offers" for a member: a declined offer can be re-made at once (with B2 uncapped).
7. `setRole` demoting the offered member to viewer does not drop the offer (accept makes a viewer owner). The owner can cancel; only a note.
8. A tombstoned (erase in flight) offerer's offer can still be accepted (T2: accept 200, then the erasure leaves one owner and `circle_transfers` empty). Correct outcome; no ownerless state. Offer to a tombstoned target is refused by `IN_STANDING_SQL`; accept by it is refused by `ALIVE_SQL` (checked by reading, the harness cannot tombstone without clearing PII, so T3 is not run).
9. The new `OWNERLESS_CLOSED_SQL` scans every unpurged closed circle by `circles_purge` (all close reasons) before the `owner_closed` filter; LIMIT 50 applies after the filter. Bounded by closes in 14 days; fine, document.

## Verified OK (tried to break, held)

- (1) Closed-circle succession: leave and erase both hand on, stay closed, reopen inside 14 days works, purge after (test 34-1); the boundary uses `undo_until > now` in succession and `purge_after <= now` in purge, so no gap. Closed + only suspended members: stays ownerless, daily net hands it on after a restore (T5). Open circle with an owner is untouched; a suspended owner stays owner.
- (2) Suspended owner's codes: preview/join give the same 422 as any bad code, and work again after restore (T6, 34-2); `CODE_USABLE_SQL` is used in both lookup and the join UPDATE. Succession skips suspended editors/viewers. Suspension/restore changes no rows, so nothing is lost on restore except B1/limit 2. Removed content stays removed after restore; undeleting a suspended author's bank leaves it hidden (author_ok) until restore.
- (3) Publish check: held and suspended-author banks give `circle_id/name/bank_id/title` all null and `may_claim_independence: false` (34-3, T4); the position is still refused. Former member / removed bank: ids only.
- (4) Owner delete of an author-removed bank: switches to `owner_removed`, window unchanged, a second owner delete is idempotent with no extra audit, the author's undelete is 403, the owner's works.
- (5) Offers: only the target accepts (others get `no_pending_transfer`, outsiders the counted 404), only the owner offers, offer is replaced by upsert, expiry checked in the accept statement, double accept refused, accept refused when closed (409 `circle_closed`), offer survives close/reopen (T1), cap checked in the step-down statement (race test at every boundary passes), offerer leave/target leave/removal/erasure/purge/expiry drop the row, erased offerer or target leaves no row (T2). `changes()` chain in the accept batch is right (promotion then audit).
- (6) Message: `ownedMessage` is used in both circle creation and accept.
- Export: `transfer_offers` has no other party's id.

## Nitpicks

- `circles.ts:143`: a `--` SQL comment inside the `CODE_USABLE_SQL` template; it works only because the newline survives. Any caller that flattens whitespace would comment out the standing check. Use a JS comment.
- `circles.ts:842`: `isOwner` interpolates `me` into the SQL string (a validated UUID today, but bind it).
- Export `transfer_offers` (`circles.ts:1573`) says "open" but has no `expires_at > now` filter (expired rows show until the daily purge).
- README says the `circle.transfer` audit has the offerer as actor; the activity line therefore reads "A transferred ownership to B" even though B did the accepting; fine, but the label `ownership_transferred` has no hint of the acceptance.
