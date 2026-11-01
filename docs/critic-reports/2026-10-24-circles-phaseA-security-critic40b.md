# Critic 40b: circles phase A, security / abuse / privacy lens

Scope: the uncommitted circles phase A backend in `server/worker` (migration 0108, `circles.ts`, `circlesend.ts`,
`circlecheck.ts`, `circleadmin.ts`, and the changed pool files) against `docs/circles-plan.md` (D1-D19).
Baseline: `cd server/worker && npm test` gives 533 pass, 0 fail. Attack scripts (throwaway, outside the repo):
`/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/atk1.ts` to `atk5.ts`,
run with `node --experimental-strip-types --no-warnings --test <file>` from `server/worker`, on `test/pool/harness.ts`.

What held up (no finding): invite codes (120 bits, SHA-256 with a `circle:` prefix, single atomic `uses` bump,
all refusals one 422, 10 failed tries a day, malformed codes reveal nothing but their format); 404-for-non-member
on every route including the cross-circle bank and blob IDOR paths (every query binds `circle_id` with the bank id);
viewer/editor role checks; the owner-succession batch and the one-owner partial index; ALIVE_SQL on every write
I read; no LIKE, no string-built SQL from request data (ids are regex-checked before they reach `won`/`json_each`);
activity never lists codes, reports or admin opens; circle images are not reachable through public `GET /blobs`;
circle content never reaches `served`, search, export bodies or imports; admin content opens are audited before the
body streams.

## Blockers (new class)

### B1. A removed member rejoins at once with the same multi-use code (D6 and README violated)
`src/pool/circles.ts:599-614` (`joinCircle`), `:670` (`removeMember`).
The join takes a use on any live code and `ON CONFLICT (circle_id, user_id) DO UPDATE ... left_at = NULL` revives the
former row. Nothing compares the code with the removal. D6 says "re-joining needs a new code", and README line
"`members/remove` ... re-joining needs a new code" says the same; neither holds for any code with `max_uses > 1`
that is still usable (the normal way to invite a group; 7 days by default).
Repro (`atk1.ts` S1): owner issues one editor code with `max_uses: 10`; X and Y join; owner removes X (X now gets
404); X posts the same code to `/circles/join` and gets `200 {"ok":true,"role":"editor"}` and full access again.
Impact: removal (the owner's only tool against a member who posted student data or copied material) is undone by
the person removed, until the owner notices and revokes every open code. Severity: high for the moderation model.
Fix direction: in the join UPDATE add `AND NOT EXISTS (SELECT 1 FROM circle_members r WHERE r.circle_id = ?2 AND r.user_id = ?4 AND r.left_reason = 'removed' AND r.joined_at >= (SELECT created_at FROM circle_invites WHERE id = ?1))` (a removed member needs a code created after the removal), or revoke/ignore codes older than the removal for that user.

### B2. The author undoes the owner's removal of their bank
`src/pool/circles.ts:1073` and `:1080` (`undeleteBank`).
The permission test is `author_id = me OR owner` for any `removed_reason IN ('author_removed','owner_removed')`.
Repro (`atk5.ts` S8): editor shares a bank, owner deletes it (`removed_reason = owner_removed`), the editor calls
`POST .../undelete` and gets 200; `removed_at` is NULL again, the bank is visible to everyone.
Impact: the owner cannot remove an active member's bank (inappropriate, student data the owner noticed, copied
test); the author reverses it inside the 14 days, silently except for an activity line. Severity: medium-high.
Fix direction: an `owner_removed` removal is undone by the owner only; `author_removed` by the author or owner.

### B3. The public-publish circle check returns one row per (hash x membership) and has no cap on former memberships
`src/pool/circlecheck.ts:44-49` (`CIRCLE_HITS_SQL`), `:68` (`.all()`), `circle_members` rows kept after leaving
(`migrations/0108_pool_round32.sql` comment, "Rows of members who left ... stay until the circle is purged").
The query is `h CROSS JOIN my` and selects a row for every pair, hit or not, so the Worker materialises
hashes x memberships objects before it looks at any `hit`. No reservation (the round-29 `lender`/`reserveBytes`
discipline for D1 results) covers it, and `my` counts former memberships without a bound (plan 9.1 marks this
UNCONFIRMED).
Measured (`atk4.ts`, node:sqlite, 1000 hashes, one chunk): 20 memberships gives 18,020 rows in 16 ms; 500 gives
450,500 rows in 440 ms; 2000 gives 1,802,000 rows in 3.3 s. A bank of 1000 questions sends 2000 hashes (two chunks).
Reaching hundreds of former memberships is cheap for two or three cooperating approved accounts: one account creates
5 circles a day, the other joins and leaves them, and each circle (and the former row) lives 14 days after it closes
or indefinitely if a third member stays. The victim is the shared isolate: millions of result objects against a 72 MB
budget, 128 MB isolate (an OOM restarts every in-flight publish and read), plus D1 time per publish and dry run.
Severity: medium (needs several approved accounts, but the damage is shared and the fix is small).
Fix direction: `SELECT ... FROM (SELECT ... ) WHERE hit IS NOT NULL` so only real hits come back; aggregate per
hash in SQL (MIN by first_seq); cap or age out the former memberships the check reads (for example the newest
200, or only circles that still hold qkeys); add a test at 500 former memberships.

### B4. Circle send reserves less isolate memory than a public publish for the same payload (round-16 class reopened)
`src/pool/circlesend.ts:138-139`, against `src/pool/publish.ts:806-809`.
`publish` reserves `max(declared * 15, scan.values * PARSE_BYTES_PER_VALUE * 2)` for the heavy phase (critic21d 1:
many tiny values cost far more to parse than their bytes). `sendVersion` reserves only `declared * 15` and never
calls the value count in `scanJson` for the reservation.
Repro (`atk3.ts`): a 760,261-byte bank with one question holding 190,000 short `topics` strings; the circle send
reserves 12,167,456 bytes at the share-core call (760,261 x 16). The publish formula for the same payload is
about 36.5 MB (190,000 x 96 x 2), so the send under-reserves by roughly 24 MB. (The public publish in my script was
refused earlier for a malformed meta, so the 36.5 MB is the formula, not a measurement.)
Impact: any circle editor (100 sends a day) can run the heavy phase with about 25 MB of real heap that the 72 MB
budget does not know about, next to other reservations already at the limit. The heavy slot is single, so one at a
time, but the budget's headroom was set with that exact error closed. Severity: medium.
Fix direction: reuse the `scan.values` term (one `scanJson` call already exists at `circlesend.ts:197`; reserve
before it, or reserve the difference after it).

## Variants and limits to document

V1. **Activity list leaks titles of hidden banks** (`circles.ts:798-802`, `:813`). The title lookup ignores holds and
removals. Repro (`atk1.ts` S2): a bank held for student data is absent from the viewer's circle page, yet the viewer's
activity shows `bank_shared ... bank_title: "Secret title Jane Doe roster"`; after the author removes the bank the
title is still listed with `bank_removed`. A student-data hold exists to hide exactly this text (titles are free text
up to 200 characters). Fix: return `bank_title: null` for held banks (except to the author) and for removed banks
(except to the owner and author), as `getCircle` filters them.

V2. **A held or reported bank's evidence can be pushed out by its author** (`circles.ts` D16 prune, `circlesend.ts:418`,
`sendVersion` has no hold check). Repro (`atk2.ts` S3): reporter files a student-data report on version 1 (hold set);
the author sends 21 more versions (allowed, 100 a day); version 1 becomes `pruned`, and after the daily run its
`full_key` is `''`. The report row still says version 1, and `circle-bank` answers `version_not_retained`.
Fix: refuse sends (or skip pruning) while the bank has an open report or a hold; or keep the reported version.

V3. **Insider can fill a circle's caps, and the owner cannot free them for 14 days** (`circlesend.ts:350-399`,
`circles.ts:1296`). Repro (`atk2.ts` S4, S5, with the caps lowered by `pool_limits`): an editor fills the bank cap
(100) with throwaway banks; the owner's own send gets `422 circles_limit`; after the owner deletes them all it is
still `circles_limit` (removed banks count until purged) and works only after 15 days and the daily purge. The
record cap (`circle_records`, 50,000 distinct question hashes ever held) is worse: an editor fills it with fresh
questions across versions (about 50 sends of 1000 questions, inside the 100/day limit); every new unique question by
anyone then gets `413 circle_records_full`, also after the bank is removed. Needs an invited editor, so a
trusted-member abuse, not an outsider one. Fix: "purge now" for removed banks (owner), or exclude removed banks from
the caps and qkeys; removing a member could offer to purge their banks.

V4. **Activity query scans non-activity rows** (`circles.ts:781-788`). The `action IN (...)` filter cannot use the
`(item_id, at)` index, so rows for `circle.invite_create`, `circle.invite_revoke`, `circle.report` and
`circle.admin_*` lying between real events are read on every page. An owner can write about 100 such rows a day
(10 open codes, create and revoke, 50 creates a day). Retention is 3 years. Bounded by members' own requests, but it
is the unbounded-scan shape rounds 30-31 removed. Fix: index `(item_id, action, at)` or skip the junk with a
`pool_audit` column for visibility.

V5. **Open reports hold the purge indefinitely** (`circles.ts:1305-1324`). A member who files a report just before
the owner closes the circle (or deletes a bank) stops the 14-day purge until an admin decides; the data the owner
asked to delete stays. Intended as evidence preservation; document that the admin queue must be worked, and that an
erased reporter's report keeps blocking the purge.

V6. **Student-data blocks are not applied to circle banks.** `privacy_blocks` (text matches and images) do not
hold or purge circle banks automatically; they only give an admin a basis to open (`circleadmin.ts` `PRIVACY_MATCH_SQL`)
and the image route refuses blocked images. A teacher can still share previously blocked text into a circle, and
a block confirmed later leaves it readable to members until someone reports it. Document as a gap against the
public pool's behaviour.

V7. **Admin `basis` is a label, not a check** (`circleadmin.ts:132-139`). `basis=report` accepts any report ever
filed on the bank (resolved or dismissed); `basis=notice` accepts any existing `dmca_notices` id without checking it
names this bank. All opens are audited, so this is accountability, not access control. Say so in the privacy page,
or require an unresolved report and a notice that names the bank.

V8. **Default on erasure keeps circle banks**, unlike public items (`circles.ts:1187-1189`). With no `pool_prefs` row
(the default) the erased teacher's circle banks are kept anonymised; the public pool's default deletes everything not
imported. D15 says this, but the default is the opposite of the public one, the bank text can still carry the
teacher's name or student data, and `circle_versions.committer_id` / `circle_qkeys.first_by` keep the opaque id.
Confirm the owner meant a different default for people who never opened the setting.

V9. **Independence statement over CC-licensed circle banks** (`circlecheck.ts:52-56`, `READABLE_OTHERS_SQL` filters
`license = 'circle-only'`). A member can publish an exact copy of another member's question from a CC-licensed bank
publicly, with an independence statement, as "first" and without credit. Audited and flagged (`independent_claim`),
not refused; phase C is meant to add the credited route. Document until then.

V10. **Alert channel sharing** (`shared.ts:709`). Circle student-data reports use the public `privacy_report` budget
(20 mails an hour). One member can file up to 20 reports a day (`reports` limit) on other people's banks, so a
two-account circle can use up the hour's budget and silence public student-data mails for it (the queue still holds
them). All other circle reports share one `circle_report` channel of 5 an hour. Give circle reports their own channel.

V11. **Per-account storage ceiling.** `circle_bytes` is 200 MB per circle and not charged to anyone (D3), so one
account can own 5 circles (5 x 200 MB = 1 GB) plus whatever circles it edits, against 200 MB for public items.
Intended by D3; state it in the operator limits section.

V12. **Member ids are shown to every member.** `GET /circles/:cid` returns each member's auth `user_id` to viewers
too (`circles.ts:362`). Only the owner needs them. Harmless today (ids are not secret), but an id is what the
remove/role/transfer routes and the admin routes take.

## Nitpicks

N1. `closeCircle` counts the caller's `circle.close` audit rows by `actor_id` (`circles.ts:409`), reading all of the
account's audit rows on each close (index `pool_audit_actor` only). Small (5 closes a day), but add an `action` to the index or count from `circles`.
N2. `retention` (`cron.ts` diff) probes `circle_banks JOIN circle_holds` once per audit row of a circle (up to 100 banks); bounded by `RETENTION_ROWS`, but only after the cheaper clauses, so fine; mention it in the cost notes.
N3. `explain()` after a lost guarded write re-reads state and can turn a concurrent change into `409 conflict` with a generic text; fine, but `leaveCircle` answers 404 when the account was removed mid-request, which reads oddly.
N4. `joinPreview` refunds the failed-try charge on success, so a holder of a valid code can preview without limit; harmless (valid code), but the preview shows owner display name and member count to anyone who holds the code, including a leaked one.
N5. The `heads` route skips `countMiss` for unknown ids (ids are 128 bits, so no enumeration), unlike the bank routes; consistent with the public `versions` route, only worth a comment.
N6. A message of up to 500 characters (`circle_versions.message`) is stored and exported but not shown anywhere in phase A; keep the cap at the validation edge when phase B shows it (cleanText already drops control and format characters).

## Summary for the owner

Four blockers: B1 (removed members rejoin on a multi-use code; contradicts D6 and the README), B2 (author reverses an
owner's removal), B3 (public-publish check materialises hashes x memberships with no bound on former memberships),
B4 (circle send under-reserves isolate memory for dense payloads). Each has a repro that ran. Everything else is
document-or-fix variants; V1 (activity title leak for held banks) and V2 (evidence pruned on a held bank) are the two
I would also fix before a pilot because they defeat the student-data hold the plan relies on.
