# Critic 46b: circles phase B (rounds 37-38), security / privacy / abuse lens

Fresh reviewer. Scope: uncommitted phase B on `shared-pool-phase1`. `cd server/worker && npm test`: 584 pass, 0 fail (ran it).
Scripts (all run, `node --experimental-strip-types --no-warnings --test-reporter=spec --test <file>`):
`/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/c46/{lib,w1,w2,w3,o1,p1,r1}.test.ts`.
No repo file was changed except this report.

## Step 1: critic 45 blockers, re-run

| Repro | Result now |
|---|---|
| 45 a1 (dirty v2, clean v3, member reads `versions/2`) | `404 version_withheld`; history `kept:false, withheld:true`; flag still open. Fixed. |
| 45 s1 (same, product lens) | Fixed (`E reads v2: 404`). |
| 45 s5 (byte-cap deadlock) | Rollback at the cap: 200, kept 20. Fixed. |
| 45 s8/s3 (duplicate `published.qid`) | Both copies keep the origin, nothing counted as removed. Fixed. |
| 45 a3 (origin laundering by omitting the qid) | Origin kept (match hash), author relicense 422. Fixed. |
| 45 a2 (authz matrix), s2 (oldest_kept), s4 (paging) | Same answers as before / `pinned_versions` present. (s2's S6 step fails only because that old script calls a PATCH members route that does not exist.) |

(a) Withheld version, paths I tried with the dirty text/images/title: head read, `versions/:n` (author, editor, viewer, owner),
ETag/If-None-Match, history, merge-initiate `between`/`base_kept`, rollback target, blob route, `circles/heads`, activity,
export (`versions_sent` is the sender's own message only), the admin routes (only the audited `basis=privacy` open), after:
clean send, rollback to a clean version, bank-wide release, generic `resolve-flag`, erasing the committer, 20+ later sends
(pruning). The text, images and message are never served. The commit batch marks `withheld_at` from the holds that exist
after its own hold statements (`circlesend.ts` ~772-778), and no later statement clears it; `circle_holds` PK is
`(bank_id, reason, source)` so another hold cannot swallow the privacy insert. One path serves a withheld version's
**title** (blocker N1).

(b) Cap/trim authz and races: viewer 403, non-author editor 403, non-member 404 (also with the caller's own circle id and a
foreign bank id), bad `keep` (0, 20, 1.5, "3", -1, null) 400, held bank 409 `under_moderation` (author) / 404 (owner), an open
report pins its version through `trim keep=1` (`pinned_versions:[9]`), trim twice is idempotent, send+trim+rollback in
parallel gave 200/200/409 with a consistent state (w3). The cap's `PRUNABLE_SQL` and the prune statement use the same
predicates (status, `version <= n-20`, no hold, no open report on that version) and both run before this commit's own hold
changes, so no parity gap that I could construct. Clean.

(c) Duplicate qid / origin-by-text: the 45 repros are fixed, but see variants V1 and V2: a third party can now attach or
impose an origin.

## Blockers (new class)

### N1. A withheld version's title, subject, course and grade stay live in the listing, the head read and the activity feed -- MEDIUM, privacy

- Where: `circlesend.ts:729` (`title = COALESCE(?5, title)` ... written by every send, withheld or not); `circlesend.ts:1015`
  (`meta: null` for a rollback, so the bank keeps the title the withheld version wrote); read side `circles.ts:1136`
  (`bank.title` in the head read), the circle detail's banks list, `activity()` `bank_title`.
- Class: round 38 made the student-data property per VERSION, but the bank row's listing fields are written by the latest
  version and are never tied to it. Nothing resets them when that version is withheld.
- Repro (w2.test.ts, ran): admin confirms a student sentence; editor Eli sends v2 with the sentence and title
  "Jenny Smith P3 scores" (`held:true, withheld:true`; hidden from members). The AUTHOR does the documented fix, a rollback
  to v1 (`200 held:false`). Viewer Vic then sees `GET /circles/:cid` -> `banks[0].title = "Jenny Smith P3 scores"`, the head
  read `bank.title = "Jenny Smith P3 scores"`, and the activity feed `bank_title` on three lines. Same through the other exit
  (w1.test.ts): an admin bank-wide release (hold lifted, v2 stays withheld) lists the title/subject at once.
- Impact: the text-match scan does not look at titles, so the title is exactly the part of the withheld send that nothing
  holds back, and the rollback (the fix path the product tells the author to use) is what publishes it to all members (and
  the owner's removed-bank view). The task's own list says "text, images or titles".
- Fix direction: store title/subject/course/grade per version and let a rollback restore the target's; or blank the bank's
  listing fields whenever the version that wrote them is withheld and the head is not that version.

### N2. A lost commit whose R2 delete fails leaves permanent, unrecorded orphans (log says "retried hourly") -- MEDIUM-LOW, privacy retention

- Where: `circlesend.ts:815-817` (lost CAS: `DELETE FROM circle_versions ... staging` then `deleteKeys`) and `circlesend.ts:281-283`
  (abandoned-attempt replay). `deleteKeys` (`moderation.ts:635`) only forgets keys that are already in `r2_orphans`; these two
  paths never insert them (the cron path `discardCircleStaging`, `circles.ts:1569`, does, in the same batch as the row delete).
- Repro (r1.test.ts, ran): Eli's send loses the compare-and-set (head moved) while `r2.delete` throws. Result: `409 conflict`,
  `console.error "R2 delete failed, retried hourly"`, version row gone, `r2_orphans` empty, both objects
  (`circles/<cid>/<bid>/<vid>.json` and `.idx.json`) still in R2 forever. Nothing in the hourly cron can find them.
- Impact: the body is up to 4 MB of circle content, and the losing sender can be an erased account or a send that held
  confirmed student data. Rare (needs an R2 delete error) but permanent, and the code claims otherwise.
- Fix: insert both keys into `r2_orphans` in the same batch that deletes the staging row, as `discardCircleStaging` does.

No other blocker found in the areas I attacked (merge token, version/rollback/trim authz, commit_id replay, erasure races,
public-path regression); details below.

## Variants and limits

V1. **An editor can graft or impose an origin on the author's bank (o1.test.ts, ran).** (a) Resend the origin-bearing qid twice
and change the copy's content: `200`, `origins` = positions 1 and 2, position 2 is Eli's original text credited to Dee
(`circlesend.ts:435`/`carry`, round 38 B3). (b) An editor sends any question with `origin:{item_id: <any visible public
item>}`: accepted (`circlesend.ts:490`; the server checks declared origins only), and the author's `PATCH license:
circle-only` is `422 license_incompatible` until she drops that question or rolls back, even after she dropped the real
credited copy. Phase A let only the author declare her own; phase B lets a third party lock another person's licence and put a
false credit on public view. Griefing by a trusted editor, recoverable by the author. Suggest: origins declared by a
non-author are ignored (or marked `declared_by` and dismissable by the author).

V2. Origin-by-text (`byHash`, `circlesend.ts:519`) only works from the head's recorded origins and the key-free skeleton
(stem and options, not the key). I could not make it attach an origin to a question that is not that skeleton, and an author
who drops the origin question frees the bound (an editor cannot re-attach it later, a rollback to it is refused). One
character of reword plus no qid still sheds the origin (documented).

V3. **A confirmed block is not retroactive.** `withheld_at` is set only at a commit. Text already in a kept version when an
admin purges the matching public item stays readable (the admin's `PRIVACY_MATCH_SQL` lets the admin find it, nothing holds it).
A rollback copy of such a version IS withheld while the readable original is not. Plan 13.2 says "on sends", so this is a
limit, but README should say it.

V4. **Rows read grow with a bank's pruned history.** `circlesend.ts:760` and `circleversions.ts:189`
(`DELETE FROM circle_version_blobs WHERE version_id IN (SELECT id ... status = 'pruned')`) read every pruned version of the bank on
every send and every trim (EXPLAIN: `SEARCH circle_versions USING INDEX circle_versions_bank (bank_id=? AND status=?)`, then a PK
probe per row; history lines are kept forever). 41 pruned rows after 60 versions in p1.test.ts; at 5,000 versions that is
about 10,000 extra reads per send. Restrict it to the versions this commit pruned.

V5. **Held banks never prune** (documented), so `BYTES_SQL` (circle-wide SUM, `circlesend.ts:83`) and `keptInfo` (loads every kept
version of the bank into a Set) read O(unpruned versions) on every send/history/merge in that circle. An author who can hold her
own bank (resend the confirmed text) can grow this to the circle byte cap (200 MB / a few KB per tiny version). Cost
amplification only.

V6. **Stale/refunded sends are unmetered heavy work.** `REFUNDED = {409, 428, 503}` gives the day's count back, but the body is read
and the one heavy slot taken before the base is compared (`circlesend.ts:206` then `sendOnce` -> `conflict`). The only bound is
the general 300 requests / 5 min. Compare the base (or the cheap 409) before `acquireHeavy`. With the public publishes sharing
that slot (queue 4), five editors can make public publishes see `503 busy` (45b variant 5, unchanged).

V7. `PATCH .../banks/:bid {license}` reads the head's origins and then updates the licence with no compare on the head
(`circles.ts` ~1296-1308); an editor's send that adds an origin in between leaves a licence the head's origins do not allow.
The window is two awaits; add `AND head_version_id = ?` to the UPDATE.

V8. `pruneCircleVersions` deletes at most 500 version bodies a day (`circles.ts` PRUNE_BATCH, daily stage). More than 500 prunes a
day across the service (including trims) builds a backlog of bodies, withheld ones included, in R2.

V9. Rollback does not extend `pending_blobs` for the images it copies (a send does). If the target and every other holder of an
image are pruned or trimmed while a rollback waits for the heavy slot (up to 20 s) and the blob GC runs then, the new version
points at a deleted image. Very narrow.

V10. `PRIVACY_MATCH_SQL` (admin basis=privacy) never expires: a resolved/dismissed `privacy_text_match` flag or an old
`circle_qkeys` match still lets the admin open any version of that bank. Audited, so a nitpick on D1's "only on a basis".

V11. A withheld version's line still tells every member who sent it, when, its question counts and licence (`by`, `at`,
`question_count`). Intended; say so.

## Checked and clean

- Merge token: wrong user, forged exp with the real MAC (45 a2), other bank, demoted holder, removed holder: all 422/403/404.
  MAC compared in constant time before the JSON is parsed. `merged_from` remains informational (45b variant 2).
- commit_id: cross-user reuse 409 without detail; replays run after the membership/role/readable checks, so a removed or
  demoted account gets 404/403, not the stored answer; abandoned staging row is discarded and the request reruns.
- 404/403 uniformity: non-member, foreign circle id, foreign bank id, held bank for a non-author: 404 `not_found` on history,
  version, merge, rollback, trim, blob. Roles are answered 403 only to members who can already see the bank.
- Erasure: staging insert, CAS, trim, PATCH, pending-blob upsert all carry `ALIVE_SQL`; the committer's message is nulled.
- Reservation parity: a rollback reserves `max(size*15, values*96*2)` like a send (38-11); heavy slot released on every
  path (`finally`).
- Public publish regression: `privacynear.ts` is a line-for-line move (same band query/limit, `cut` fail-closed, exact set
  fetched only when band rows exist, empty-bands early return); `acquireHeavy`'s default slot is the same module object;
  full suite including rounds 12-22 passes.
- Mail: circle notices use their own `circle_privacy_match` channel (5/hour), contain ids only.

## Nitpicks

- The `console.error` in `deleteKeys` promises an hourly retry that circle paths do not arm (N2).
- `circle-release {version}` writes its audit row and may delete the privacy holds even when the version is not withheld (the
  `409 not_withheld` is returned after the batch); harmless (the hold deletion needs the head and a non-withheld row).
- `GET .../versions/0` is routable and answers 404 `not_found` after a counted miss; fine.
