# Critic 41a: circles phase A after the Round 33 fixes, product/functional parity

Scope: the uncommitted circles phase A in `server/worker` (migration 0108, `circles.ts`, `circlesend.ts`, `circlecheck.ts`,
`circleadmin.ts`, the changed pool files) after the Round 33 fixes, against `docs/circles-plan.md` (D1-D19) and the earlier
reports `...-moodle-critic40a.md` and `...-security-critic40b.md`.
Baseline: `cd server/worker && npm test` = 543 pass, 0 fail; `npm run typecheck` clean. No repo file changed except this report.
Scenario scripts (not in the repo), run with
`node --experimental-strip-types --no-warnings --test --test-reporter=spec <file>` from `server/worker`:
`/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/` `crit1-4.test.ts`,
`atk1.ts`, `atk2.ts`, `atk3.ts`, `atk5.ts` (earlier), and new `crit5.test.ts`, `crit6.test.ts`.

## Verdict: no new blockers found. All earlier blockers are fixed.

## Step 1: earlier blockers re-run (all fixed)

| Earlier finding | Re-run result |
|---|---|
| 40a B1 / 40b B1 removed member rejoins with same code | crit1 S2, atk1 S1: `422 circle_invite_invalid`, no role row |
| 40a D1 demoted member leaves and rejoins as editor | crit4 D1: rejoin `200 viewer` |
| 40a B2 ex-owner's code works after transfer / leave | crit1 S4, crit4 D2: `422`; list now has `created_by`, `created_by_name` |
| 40a B3 `circles_owned` bypass through transfer | crit1 S1: `422 circles_limit circles_owned used 5 limit 5`; Y stays at 5 |
| 40a B4 hold does not delay purge | crit2 P1: after 15 days the bank is not purged and the hold row is still there |
| 40b B2 author undoes owner's removal | atk5 S8 and crit6 V3: author `403 circle_role_required`; owner undelete works (200) |
| 40b V1 activity leaks titles | atk1 S2: `bank_title: null` for held and removed banks |
| 40b B4 send reserves less than publish | atk3: reserved 37,244,690 at the share-core call (was 12.2 MB) |
| 40b V2/V3 | atk2: reported/held bank keeps v1; owner purge route exists (below) |

My own variants (all behave as intended):
* Owner purge route (`purgeBankNow`): a held bank answers `409 under_moderation`, bank and hold row survive (crit6 V3). A flood of
  100 junk banks from a removed editor is cleared by 100 deletes plus 100 purge calls: `circles.qkeys` back to 0, the owner's next
  send is `201` (crit6 V4; before the fix `422 circles_limit 100`).
* Owner-only restore of an owner-removed bank: author `403`, owner `200`; for a held bank the owner's undelete returns 200 but the
  bank stays hidden (404 on read), which is right.
* Rejoin after removal needs a code created after the removal; after leaving, an older code gives at most the role held (viewer stays
  viewer); a code created after leaving gives its own role (crit6 V5: removed E rejoined with a code issued after the removal, `200`, which is the rule;
  demoted F, who left after the code was issued, came back as viewer; a code issued after F left would give editor, see variant 5).
* Codes after transfer, leave and erasure: revoked in the same batch; join also refuses any code whose issuer is not the active owner.
* `circles_owned` under races: the cap is inside the createCircle INSERT and the transfer step-down statement (no read-then-write gap).
* D13 check is bounded as documented; `my` is limited to `circle_memberships_total`, which equals the join cap, so the window never
  silently drops a circle for an ordinary account.

## Blockers (new class)

None found. I looked for regressions and interactions of the fixes (rejoin rules x code revocation x succession; holds x owner purge x
version pruning; the 100-row cap x leave/rejoin/create; erasure x codes/members/banks; export fields) and found no mechanism that
loses data, lets a removed or demoted member come back without a new owner decision, bypasses a hold, or fails a cap open in a way
the plan did not already accept.

## Variants and limits to document (not blockers)

1. **The 100 membership-row cap (`circle_memberships_total`) can falsely refuse an honest teacher, and nothing in the app lets her
   recover.** Rows of circles she left stay while the circle exists. Repro (crit5 V1): one teacher joins and leaves 100 different
   circles; join 101 is `422 circles_limit metric circle_memberships_total used 100 limit 100`, and so is creating her own circle.
   The message says "You can join another once some of them close", which she does not control. Realistic only for a heavy joiner,
   and an admin can raise the per-account limit, so not a blocker. Suggest: say "ask an administrator" in the message, or let a
   leaver with no bank/record in a circle drop the row (`left_reason = 'left'` and the circle holds none of her records).
2. **The `circles_owned` message tells a teacher at 5 circles to "Close one", but a closed circle still counts until it is purged.**
   Repro (crit5 V2): 5 owned, close one, create: `422` with the same "Close one" text; works after 15 days. Fix the wording or let
   `OWNED_SQL` ignore owner-closed circles. Simplest: reword ("closing frees the slot after 14 days").
3. **Succession still ignores `circles_owned`.** Repro (crit6 V6): seven different owners each create a circle, invite Y as editor
   and leave; Y owns 7 (cap 5) and Y's own create is refused. Cooperating accounts can therefore park more than 5 circles x 200 MB on
   one account; the real bound is Y's 20 active memberships (and each leaver's 100 rows), about 4x the intended storage per account.
   Needs two approved accounts. Documented in the README as accepted; keep it in the pilot risk list.
4. **Closing then leaving still strands a circle (crit6 V7).** Owner closes, then leaves: `200`; the remaining editor sees the circle
   closed with an undo window but `reopen` is `403 circle_role_required`, the circle's own banks answer `409` to everyone, and all
   content is purged at day 14. This is the most likely real data loss in phase A and the cheapest to remove (refuse `leave` for the
   owner of an owner-closed circle with `409 circle_closed`, or run succession for owner-closed circles). The README lists it as a
   known limit; I would fix it before the pilot.
5. **A new team code undoes a demotion or a removal for whoever the owner hands it to.** By design (a code issued after the
   departure gives its own role), but an owner who demotes E to viewer and later issues a fresh multi-use editor code to the group
   restores E if E leaves and rejoins with it. Document next to D6/D17 so the owner knows a demotion lasts only until the next
   editor code is shared and the member re-enters.
6. **Owner purge claim is not atomic with the purge (read from code, not run).** `purgeBankNow` sets `purge_after = now` and then
   runs `purgeBank`; if the second step fails (R2 error), undelete is already refused (`purge_after > now` fails) and the daily cron
   finishes the purge. Acceptable (the owner asked for it), but the 500 hides that the bank is now unrecoverable.
7. All variants listed in critic40a (leave-then-claim independence, first-sender after purge, no copy tracking, retention of
   resolved reports and tombstones, legal pages, junk-flood now clearable, report-pinned purges, strict `OPEN_REPORT` without age
   limit) remain as documented in README Round 33 "Known limits"; I found nothing that changes their severity.

## Nitpicks

* README Round 33 says `round33.test.ts` has 10 tests; `grep -c "^test("` gives 10. Correct.
* `circle.member_erased` is shown as "left" in the activity list, as documented.
* `purgeBankNow` for a bank removed with reason `circle_closed` is accepted by the claim statement but cannot occur in practice
  (that reason is only written by the purge itself).
