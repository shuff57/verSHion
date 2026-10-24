# Critic 40a: circles phase A, product/functional parity (Moodle-style shared banks and the plan)

Scope: uncommitted circles phase A in `server/worker` against `docs/circles-plan.md` (D1-D19 and owner decisions),
`docs/shared-pool.md` and the Round 32 README. Method: read all of `circles.ts`, `circlesend.ts`, `circlecheck.ts`,
`circleadmin.ts`, `0108`, the diffs of the shared files, then ran throwaway scenarios on the real harness
(`test/pool/harness.ts`). Baseline `npm test`: 533 pass, 0 fail. No repo file was changed except this report.
Scenario files (not in the repo): `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/crit1.test.ts` to `crit4.test.ts`
(run with `cd server/worker && node --experimental-strip-types --no-warnings --test --test-reporter=spec <file>`).

Verdict: 4 blockers of new classes (all reproduced). No problem found in the core CAS/ownership/erasure mechanics
(single owner invariant, succession order editor then viewer by `join_seq`, owner leaving with only viewers, role
gates 403/404, codes single use / 7-30 days / revoke / race-safe use count, D13 check for unlisted and former
members, export fields, blob GC).

## Blockers (new class)

### B1. A removal or demotion is undone by leaving and rejoining with the same still-valid code (role/permission semantics, code lifecycle)
`circles.ts:581-614` (`joinCircle`): the membership upsert is `ON CONFLICT (circle_id, user_id) DO UPDATE SET role = excluded.role, left_at = NULL, ...`
with no look at `left_reason` or at whether the code is older than the departure. A code is only spent by `uses`, so any
multi-use code (the normal way to invite a team of 5-10) keeps working for the people the owner just removed or demoted.
- Scenario 1 (removal): A creates `max_uses: 5` editor code, B joins, A `POST /members/remove {B}` (200), B `POST /circles/join` with the same code -> `200 {ok:true, role:"editor"}`. B is back, with author rights on their banks again.
- Scenario 2 (demotion): same code, A sets B to viewer, B `POST /leave`, B joins with the same code -> `200`, role `editor`. Output: `D1 E role now viewer` then `D1 rejoin: 200 editor`.
- Contradicts D6 ("Re-joining needs a new code") and README line 184 (`members/remove`: "re-joining needs a new code"), and makes "Only the owner removes / D17 roles" unenforceable.
- Severity: high (permission semantics; removal for a student-data or copyright reason is the case the owner needs most). Repro: `crit4.test.ts` D1, `crit1.test.ts` S2.
- Fix direction: on upsert refuse (same `circle_invite_invalid`) when the existing row has `left_reason = 'removed'` and `code.created_at <= left_at`, and never raise the role of a former member above the role they held when they left unless the code is newer than their departure.

### B2. Invite authority outlives the owner role (permission semantics)
`createInvite` is owner-only, but nothing revokes or even attributes a code when its issuer stops being owner (`leaveCircle` 641-655, `transferOwnership` 716-749; only erasure revokes, `circleErasureStatements`). `GET /invites` omits the creator.
- Scenario: A (owner) makes `max_uses 50`, 30-day editor code, then `POST /transfer {B}` (A becomes editor). A hands the code to stranger S: `POST /circles/join` -> `200 role editor`. Same after A `leave`s: output `S4 stranger joins with departed owner's code: 200`.
- The new owner cannot tell which codes the old owner issued (the list has no `created_by`), so cannot reason about them; a former owner who left angry keeps a working key to the circle for up to 30 days.
- Severity: medium-high. Repro: `crit4.test.ts` D2, `crit1.test.ts` S4.
- Fix direction: revoke (or flag `inherited`) the departing/demoted owner's open codes in the same batch as leave/transfer, and show `created_by` name in the list.

### B3. `circles_owned` (D2: 5 per account) fails open through transfer (cap that fails open, a blocker class in plan section 12)
`transferOwnership` (`circles.ts:728-742`) checks only that the target is an active member. D2 and the README ("circles_owned 5") are enforced only in `createCircle`.
- Scenario: Y owns 5 circles; X owns one and Y is a member; X `POST /transfer {Y}` -> `200`; Y now owns 6 (`S1 Y owns 6`). Repeating, one account can end up owning any number, and X can create 5/day and hand them off indefinitely, so the "5 circles owned bounds what one account can create" argument of the plan fails.
- Succession (erasure/leave) also ignores the cap; that one cannot refuse, so it should at least be documented. The explicit transfer can and should return `422 circles_limit {metric:"circles_owned"}`.
- Severity: medium. Repro: `crit1.test.ts` S1.

### B4. An admin hold does not delay the purge (D7 bent; evidence destroyed)
D7: "An open hold, appeal or takedown delays the purge as it does for items." `purgeCircles` (`circles.ts:1309-1343`) only skips banks/circles with an open `circle_reports` row (`OPEN_REPORT_*`); `circle_holds` is never consulted. `purgeBank` then deletes the bank's `circle_holds` rows and R2 bodies.
- Scenario: B shares a bank; admin `POST /admin/pool/circle-hold {bank_id, note}` (200, suspected copied publisher material, no member report exists); the owner (or author) `DELETE`s the bank; after 15 simulated days: `circle_banks.purged_at` set, `circle_holds` empty (`P1 bank after 15d: [{"p":1}] holds: []`). The author of suspect material can delete the held bank and the admin loses the content before the 14 days are out of their sight, and the hold itself vanishes.
- The same applies to a closed circle containing a manually held bank. README documents only "never one with an open report", so the gap is silent.
- Severity: medium (moderation/evidence). Repro: `crit2.test.ts` P1.
- Fix direction: skip purge while `EXISTS circle_holds` (bank, or any bank of the circle), as the item purge does.

## Variants and limits to document (not blockers)

1. **Closing then leaving strands the circle.** Owner `DELETE /circles/:cid` then `POST /leave`: succession requires `closed_at IS NULL`, so the circle stays closed with no owner; remaining members get `403` on reopen and everything is purged at day 14 (`S3`: B reopen 403, 15 days later circle and bank purged). Same if the owner is erased while the circle is in its undo window. Consider blocking leave of an owner-closed circle or running succession on `owner_closed` circles until the window ends.
2. **Junk flood cannot be cleared for 14 days.** An editor creates 100 one-question banks in a day (cap 100, `circle_commits` 100); the owner removes the editor and deletes all 100; the next bank by the owner is `422 circles_limit circle_banks 100` (`D3 owner new bank right after cleanup: 422 circles_limit 100`), because removed banks count until purged and there is no owner-side "purge now". The same holds for the 200 MB and 5,000-question caps. Document, or give the owner an immediate purge for banks of removed members.
3. **Leave-then-claim defeats the independence rule.** A member's `circle_independent_of` is refused (`may_claim_independence:false`), but after `POST /leave` the same publish passes (`E1 B left then claim: 201`), because `READABLE_OTHERS_SQL` looks only at active memberships. It is audited and flagged `independent_claim`, as the plan accepts for former members, but it means the strongest protection is one click away.
4. **A purge hands "first sender" to the next copier.** `circle_qkeys` rows go with the bank (plan 5.1/8.3). A shares Q, B reads it, A deletes her bank, 14 days pass, B sends Q to a new bank: B is now the first sender; A's own public publish of Q is refused `circle_material` and B's passes (`E5 A: 422, B: 201`). Plan-conformant, but it is a false first and a false refusal of the true author. Document.
5. **Author who left cannot withdraw their bank** (`E6`: author DELETE 404, bank still readable by members); only the owner can. D15 covers account erasure only. Likewise an author demoted to viewer can still `PATCH` the bank licence (200) but not send (`P5`).
6. **Any single open report blocks the purge of a bank and of the whole closed circle** (`OPEN_REPORT_*`), with no age limit; a lone member can keep a closed circle's content alive until an admin acts. The plan's delay triggers are hold, appeal, takedown, not any report.
7. **No copy tracking.** "Get a copy" is the same read as view (by plan), so after a student-data hold there is no list of holders to notify, unlike the public pool's `copy_held`. Provenance of a copy is only the stored `origins` of the version.
8. **Retention.** Resolved `circle_reports` (with `note`, `reporter_id`) and purged-circle tombstones are never deleted (plan says UNCONFIRMED 3 years; the public `reports` table is likewise not retained-out).
9. **Legal pages.** D9 asked for the privacy/terms page changes to be drafted in phase A; `server/web/legal/{privacy,terms,acceptable-use}.html` contain no mention of circles (disclosed in `docs/shared-pool.md`), so the pilot is the only safe use.
10. Phase A has no way for anyone but the author to send to an author's bank; an erased author's kept (anonymised) bank can never get a new version until phase B.
11. `purgeCircles` runs up to 50 circles x 100 banks x several D1 calls in one invocation; a large backlog can exceed subrequest limits (resumes next day, since each step is idempotent).

## Plan items checked and fine

Owner/editor/viewer gates (viewer POST 403, editor deleting another's bank 403, editor PUT another's bank 403, viewer/editor invite, remove, transfer 403); owner leave with only viewers (longest-standing viewer owns, can send); erased owner (editor with smallest `join_seq` owns, author's bank anonymised, shown "deleted teacher"); codes: 7-day default, 31 days -> 400, expired -> 422, single use second user -> 422, used-up code cannot be reused by a leaver, list states `active/used_up/expired`; D13: plain member refused with hit details, unlisted refused, a publisher who is first sender passes; blob GC respects `circle_version_blobs`; export contains memberships, owned, invites without codes, versions without bodies, banks, reports; admin basis rules and audits as described; routes of plan 6.2 (phase A rows) and 6.4 all exist.

## Nitpicks

- README says erasure "adds 15 statements" and "Tests ... (26)"; `circleErasureStatements` returns 14 (counted by running it), and `round32.test.ts` has 17 top-level `test(` (more via the 32-10 loop).
- README line 184 and D6 wording ("re-joining needs a new code") should be restated once B1 is fixed.
- `circle.member_erased` is shown to members as "left"; fine, but the activity list cannot tell an erased member from a voluntary leave.
