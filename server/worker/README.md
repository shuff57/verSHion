# vershion-pool Worker

The verSHion shared-pool API (docs/shared-pool.md). Accounts, sessions and the approval workflow come
from the generic auth package in `../auth`. This Worker adds the verSHion parts: the share-core
Argon2id wasm, the trust levels `unverified | pending | teacher | admin`, the sign-up profile, mail
text, staff-domain hints and the cron.

## Setup

```sh
wrangler d1 create vershion-pool                 # put the id in wrangler.toml
wrangler secret put AUTH_SECRET                  # 32+ random characters
wrangler secret put RESEND_API_KEY
wrangler secret put POOL_HASH_KEY                # 32+ random characters; never rotate casually (see below)
# vars: ALLOWED_ORIGINS, APP_URL, MAIL_FROM, ADMIN_EMAIL (comma list),
#       ESCALATION_EMAIL (comma list; someone who is NOT the admin), APPROVAL_DIGEST=1 (optional)
scripts/sync-migrations.sh                       # copy the auth package's migrations here
wrangler d1 migrations apply vershion-pool --remote
```

The auth package owns migrations `0001`-`0099` and the Worker owns `0100+` (staff-domain hints).
**A migration is never edited once committed** (staging may have applied it): `0102_pool.sql` is
exactly as first committed (1d311ff), and everything added since lives in new numbered files
(`0103_pool_rounds_6_11.sql`: the columns, tables, indexes, CHECK and key changes of rounds 6-11; the
evidence table of the committed 0102 is rebuilt and its rows discarded, so accounts re-record them on
their next reads; round 12 added a SQL backfill of sequence numbers, first publishes and ASCII folds
for rows that predate it) and `0104_pool_round12.sql` (round 12: content-based authorship records,
block keys, hold releases, privacy bands, search indexes, `items.fold_rev`). What SQL cannot derive
(exact Unicode folds, search words, normalised block keys, body hashes) the hourly cron recomputes
from the stored bodies for every item with `fold_rev = 0`, ten items a run (`backfill`).
`0105_pool_round24.sql` is the round 24-26 migration, `0106_pool_round28.sql` round 28's (retention
trigger and indexes, invite codes) and `0107_pool_round30.sql` round 30's (one index; round 31 needed
none). Add `0108_...` for the next change.

`POOL_HASH_KEY` keys share-core's `full_hash` (HMAC-SHA256; an unkeyed hash of a hidden-key question
is a guessable oracle of its key). The Worker derives the key per purpose by HKDF-SHA256 (salt
`vershion-pool/v1`, info `pool-hash-key:full-hash`, 32 bytes, hex), so the secret itself is never
handed to the wasm. Without the secret (or shorter than 32 characters) every `/items` and `/blobs`
route answers 503 `pool_not_configured` and the adapter refuses to hash; auth keeps working. Stored
`full_sha256` values depend on it: rotating it makes every stored full hash stale (a re-hash
migration would be needed), so do not rotate it casually.
wrangler reads one directory, so the package's files are copied into `migrations/`. `npm test` fails
if a copy drifts from the package.

## The first admin (and any later one)

**There is no API route that creates an admin.** To promote an account:

1. The person signs up, confirms the address and sets a password (so the account is `pending` or
   `teacher`).
2. You verify who they are **out of band**: in person, or through the school's published phone
   number. Never through a contact they supplied.
3. Run:

```sh
scripts/make-admin.sh person@school.example.org --remote
```

The script refuses anything that is not a plain email address, and generates its SQL from the auth
package's `makeAdminStatements` (the same statements the tests run). It:
- promotes only a confirmed, active, unsuspended account, **or brings back a suspended admin** (the
  way in again if every admin has ended up suspended);
- closes the account's open application with `decided_by = 'bootstrap'`;
- writes an `auth_audit` row with actor `bootstrap`.

Running it twice changes nothing.

`scripts/demote-admin.sh EMAIL --remote` demotes an admin to teacher (sessions and device tokens
revoked, audited as `bootstrap`). It never demotes the last active admin.

> **Warning.** An admin can approve teachers, and approved teachers can read every answer key an
> author chose to share (items published without "hide key"); hidden keys stay with their authors. Promote only someone you have verified yourself. Keep at least two admins: the last active
> admin can never be suspended, and the cron mails `ESCALATION_EMAIL` when applications wait more than
> 48 hours, so one admin going silent is noticed.

## Using the admin routes

Every admin route needs the admin's live session (bearer token) from a **known device**: the device
cookie set at login, or `X-Device-Token` for a native client.

Every change (approve, reject, revoke-teacher, suspend, restore, staff-domain edits) also needs a
**sudo token**:
1. `POST /auth/reauth` with `{ "password": ... }` returns `{ sudo_token, expires_in: 300 }`.
2. Send it as `X-Sudo-Token` for the next 5 minutes.

It only works for the same session and device epoch, so a stolen access token cannot approve anyone.

| Route | Body |
|---|---|
| `GET /admin/queue?status=open` | Applications, oldest first, with the email domain, the staff-domain hint, the domain's history, and the queue's age. |
| `POST /admin/approve` | `user_id`, `request_id` (from the queue), `evidence` (`directory` or `school_confirmed`), `evidence_note`, `note` |
| `POST /admin/reject` | `user_id`, `request_id`, `reason` (shown to the applicant), `note` (never shown) |
| `POST /admin/reject-bulk` | `items: [{ user_id, request_id }]` (at most 50), `reason`, `note`: for a wave of spam sign-ups |
| `POST /admin/revoke-teacher` | `user_id`, `reason`, `note` |
| `POST /admin/suspend` / `restore` | `user_id`, `reason` (for suspend), `note`. Suspending an **admin** only proposes it. |
| `POST /admin/confirm` / `cancel`, `GET /admin/proposals` | `proposal_id`. Another admin confirms; anyone can cancel. With only two admins, the proposer may confirm alone after 24 h. |
| `GET /admin/users/:id`, `GET /admin/audit` | Account detail with its audit trail; the audit log. |
| `GET`/`PUT`/`DELETE /admin/staff-domains` | `domain`, `note`: a label in the queue only. It **never** grants trust, because students often get addresses on the staff domain. |

Every action against an admin (a suspension proposal, its cancellation, the suspension, an admin
deleting their account) mails **every admin and `ESCALATION_EMAIL` at once**. With `APPROVAL_DIGEST=1`
the daily digest goes to both as well, with the last day's approvals and rejections per admin.

"New application" mails to the admins are limited to 5 an hour. The hourly cron rolls the rest into
one summary, so many sign-ups cannot flood the admins' inboxes. Use bulk reject to clear a spam wave.

The queue keeps what the applicant typed (`applicant`) apart from what the server adds
(`server_labels`: the staff-domain hint, the domain history, `profile_changed`). Render applicant
fields as plain text.

**Approval rule** (docs/shared-pool.md): approve only when the **verified address itself** is on the
school's staff directory (`directory`), or the school office confirmed the person through a contact
**you** initiated (`school_confirmed`). A name on a staff list is never enough.

## The shared pool (phase 2, `src/pool/`)

`unlisted` means private: published (so it can be republished and kept in sync) but readable only by
its author, as in docs/shared-pool.md "Sharing model". Non-authors see a public version number that
does not move for changes they are not served (a hidden key correction).

R2 bucket binding `POOL` (`wrangler r2 bucket create vershion-pool`; private, never public or
presigned). Migrations `0102_pool.sql` and `0103_pool_rounds_6_11.sql`. Every route needs `teacher` or
`admin` (and, round 28, a teacher also needs a redeemed invite code); anything an account may not see is a 404. share-core is reached only through
`src/pool/core.ts`.

**share-core adapter (round 11, share-core round 6).** `core.ts` is the one place that knows the
wasm API. Assumptions, all checked by tests: `full_hash(payload, {hash_key})` needs the hex key (at
least 32 bytes) and otherwise answers `hash_key_required`; `strip` no longer returns the author-only
data, `strip_author_data()` hands the last successful strip's `{served_orders}` out ONCE from a
per-instance global, so the adapter calls it synchronously in the same function right after `strip`
(no `await` between them can exist; a test runs two publishes interleaved at every D1 call and checks
each gets its own orders); strip's `stripped_hashes` are unkeyed `full_hash` of the served form
(public data); `locked_key` (only the correct choice locked) and `key_in_formatting` are payload
errors with messages. Round 12 (final share-core round 6): `ack_formatting` (meta field, question
indices) is passed to share-core; formatting refusals keep `reason`, `ack_allowed` and
`author_only` and carry `formatting_findings` and `order_preview` (from share-core's
`formatting_findings`), all in the publish response, which reaches the publishing author only;
`strip` runs with `drop_client_provenance: true`, so a client-sent `origin` is never served.

**POOL_HASH_KEY is permanent (round 23, critic30a B1).** Every identity hash in the pool is made
under it: share-core's keyed content hashes, every Worker fingerprint (contents, key-free contents, near
hashes, stems, block keys, `u:` unit keys, authorship records, licence-history keys) and the sketches.
Rotating it would make every stored hash meaningless (copies would stop binding, authors would be
credited to their copiers), so it is NOT supported: the pool records the key's id (`pool_kv`
`pool_key_id`, an HMAC under its own subkey) on first use, and any request that writes (every non-GET
pool route and admin pool route) or cron run under another key fails closed with `503
pool_key_mismatch`, changing nothing, with one admin alert. Plain reads continue. `GET /admin/pool/key`
shows the ids. A real rotation would need an offline migration that re-hashes every stored row from
the full bodies (impossible for purged items); none is provided. Treat the key as permanent: keep it as
a wrangler secret AND an offline backup; losing it loses the pool's identity data.

**Circles phase C, safe parts (rounds 42-45; `docs/circles-plan.md` 13.3, D5, D8, D12).** Owner-approved scope: email
invites, hiding keys from viewers, copyright re-upload checks on sends, the activity list. NOT built (owner: lawyer
review first): the public re-share route with origin credit, the DMCA takedown machine for circle banks, any new
privacy, terms or moderation wording beyond the fixed invitation mail. Migrations `0111`-`0115` (new, nothing applied
anywhere yet, `0111`, `0113` and `0114` edited in place in rounds 46 and 48; the next is `0116`). Tests:
`test/pool/round42.test.ts` (7), `round43.test.ts` (8), `round44.test.ts` (9), `round45.test.ts` (4), `round46.test.ts`
(12), `round47.test.ts` (10), `round48.test.ts` (10), `round49.test.ts` (3), `round50.test.ts` (4), `round51.test.ts`
(4), `round52.test.ts` (4); every file failed on the code before its round (45-3 is a guard that already passes after
round 44: it fails on round 41's code; 51-4 pins a rule round 50's code already keeps). Suite 680 (with
`d1-like-limit.test.ts`). Rounds 46-52 changed parts of rounds 42-51: where a line below says otherwise, the later round
is what the code does.

**Round 52 (the app's picture step, critic 62, `docs/critic-reports/2026-10-25-circles-phaseC-pictures-critic62.md`).**
No migration. Tests: `test/pool/round52.test.ts` (4, each failed on round 51's code: 52-1 to 52-3 on the refused
`base_version`, 52-4 on the missing field). No earlier test pinned the old `400 invalid_request field base_version`.

* **62 B1, the answer-key PATCH is pinned to a version.** `PATCH .../banks/:bid` takes `base_version` (an integer
  version, as a send's) with `hide_key` (true or false; refused with `license` or alone: `400 field base_version`). A
  head that is not that version is `409 conflict {head_version, message}`, the shape a lost batch already answers,
  before any decision is checked (a position past the head's count is the conflict, not a `400` for that field) and
  before anything is charged (no try, no change). Hiding on's batch compares and sets the head that was checked, so a
  version arriving after the check is the same conflict; hiding off strips nothing and is pinned by the check only.
  Without `base_version` nothing changed. This closes critic 62 c62-1 (a version sent between the author's look and
  the PATCH got her `image_ok` positions and kept a picture she never saw) and is critic 57's suggestion. 52-1, 52-2.
* **62 I2, choosing again which pictures viewers get.** Already accepted (round 43/46): `hide_key: true` again with
  other decisions re-strips the head, records them (`circle.bank_strip_decisions`, no activity line), and counts toward
  `circle_hide_changes` (20 a day; the same decisions again are `unchanged` and free; past the cap `429 daily_limit`).
  Now pinned too. 52-3.
* **62 I5, `POST /circles/heads` says `hide_key`.** Each bank it reports as `current` carries `hide_key` (boolean),
  for every member who reads it there (the circle page and the bank's read already tell every member); a `removed`,
  `under_review` or `unavailable` entry does not (a member who left reads only `unavailable`). 52-4.
* The app (client, same round): the bank page's hide PATCH sends `base_version` (the version the decisions and picture
  choices were made on; none for turning hiding off); a `409 conflict` drops every choice, pictures included, says so,
  and the bank is read again for the step to start over. The send page notes `hide_key` from its own `heads` check
  before the picture step, and the result line says plainly when the person was not asked about a picture (and that
  sending again lets them choose), or when a send made on the picture step went to a bank that no longer hides its
  keys (viewers get it in full, the dropped pictures too). While the keys are hidden, the owner or author can
  "Change picture choices" (every picture starts as dropped; pinned; counts as an answer-key change).

**Round 51 (phase C final critic 61, `docs/critic-reports/2026-10-25-circles-phaseC-final-critic61.md`).** No
migration. Tests: `test/pool/round51.test.ts` (4; 51-1 and 51-2 failed on round 50's code every run; 51-3 failed
when both PATCHes scanned before either batch ran, as they did when it ran on its own, and can pass on round 50's code
when they run one after the other; 51-4 pins the decided rule below and passes on round 50's code).

* **61 I1, decided by the owner: allowed, for the owner and the author-editor only.** `hide_off_releases` and
  hide-off's `restored` stay as round 50 made them, and so they tell the circle's owner and the bank's author while an
  editor which kind of match withheld a version an answer-key change withheld (`true`, and a version in `restored`,
  mean a copyright match and nothing else). That is an accepted exception to the round 44 rule for those two people
  only. Nobody else is told or can tell the two apart: another editor (the one who sent the version too), a
  viewer, an author demoted to viewer and a removed member get no field and the same answer for both kinds (a held
  bank is `404 not_found` to them). 51-4. The round 49 and round 44 bullets below say so.
* **61 I2, the owner's read no longer takes the heavy slot unbounded.** The full-form check behind `hide_off_releases`
  (round 50) runs only when the isolate's heavy slot is free at once (`tryHeavy`, `withStoredFull(..., {now: true})`):
  a read never waits for the slot and never takes a place in its queue, so it answers at once and can not fill the
  queue and turn another teacher's publish away with `503 busy`. A busy slot says false (nothing is promised; the app
  still keeps the switch on a held head). Each check that runs is one of the reader's `circle_hide_checks` today (new
  limit, 60; one that could not run is given back); past it the field says false without a check. Reads that check
  nothing (not a candidate, a bank that does not hide its keys, any other member) are not charged. Hide-off's own
  check is unchanged (it waits for the slot as before, on the hide budgets). 51-1, 51-2.
* **61 M1, part: of two hide-on PATCHes at once, only one says it withheld the head.** `withheld` (and `held`, and
  the held-publish charge) now also need the batch's own withhold UPDATE to have changed the row; the PATCH whose batch
  ran second answers `withheld: false, held: false`. Not changed (noted): its batch may still add a `blocklist` flag
  with no positions (counted toward its sender's `circle_send_flags`) and record its decisions on the withheld head
  (its viewers' form was scanned clean, so no key is served from them). 51-3.
* The app (client, same round): critic 61 I3, the account-switch rule of critic 60 I1 now holds on the whole Shared
  page and in the publish window, not only in Circles: "My shared items" (the list, the item open, the data download,
  the counter-notice and appeal drafts) drops everything on a sign-out and keeps the place for the same account;
  another account starts at the list. The browser, the publish window (its check, done screen, pictures sent and
  circles) and the invite step drop what the last account was told when another signs in. Critic 61 I4: a send
  already on its way when the account signs out (or the page is left) still moves this computer's link on when it
  answers, and the same account signed in again retries a lost send or restore with the same commit id. Critic 61 M3:
  after a turn-off that left the bank held, the headline is "Turning hiding off did not release the newest version
  ...", not "Viewers see the answer keys again".

**Round 50 (phase C final critic 60, `docs/critic-reports/2026-10-25-circles-phaseC-final-critic60.md`).** No
migration. Tests: `test/pool/round50.test.ts` (4, each failed on round 49's code); 49-3's last line now expects false
(it asserted the false promise of 60 B2: its version's full text is the taken-down text, and hide-off releases nothing).

* **60 B2, only what turning hiding off can release counts.** `hideOffCandidates` (the versions hide-off checks, and so
  `hide_off_releases`) now takes a version only when its open send flags are all copyright matches an answer-key PATCH
  raised (`blocklist`, `via: "hide_key"`), and never the head while the bank has a student-data hold. A student-data
  match never counts, even one an answer-key PATCH raised: turning hiding off does not change what the full form holds.
  The field is also true only when the version's full form is clean now, checked as hide-off checks it (the version's
  own sender's exemptions); a check that can not run (busy) says false. Hide-off's batch re-checks the same rule and
  lifts only the `blocklist` hold, never a student-data one. So "the field says true, hiding goes off, nothing is
  released, hiding goes on again, the field says true again" no longer happens. 50-1 (critic 60's c60-1), 50-2, 50-3.
* **60 B1, the hide-on answer says what that PATCH did.** `PATCH {hide_key: true}` answers `withheld: true` (and
  `held: true`) only when this PATCH withheld the head (a head members could read, scanned by it); a head withheld
  before it (a send's match, an earlier PATCH) answers `held: false, withheld: false`. Whether turning hiding off
  would release it is the read's `hide_off_releases`. 50-4.
* The app (client, same round): its hide-on result no longer says "Turning hiding off makes it readable again" (the
  bank page says it only when the pool's field is true); after a turn-off whose bank read still finds the newest
  version held back, it says so ("Turning hiding off did not release the newest version. It is still held back, and
  only an administrator can release it.") and keeps the switch to turn hiding on again. Critic 60 I1: the Circles
  pages drop everything read for an account when it signs out (where the person was is kept and read again for the
  same account) or when another account signs in (which starts at the list).

**Round 49 (phase C client critic 59, `docs/critic-reports/2026-10-25-circles-phaseC-client-critic59.md`).** No
migration. Tests: `test/pool/round49.test.ts` (3, each failed on round 48's code at the new field).

* **59 B1, the app can tell whether turning hiding off releases a withheld version.** A read of a withheld version
  (`GET .../banks/:bid` and `GET .../versions/:n`) by the circle's owner or the bank's author while an editor (who may
  change the answer-key setting) answers `404 version_withheld {hide_off_releases, message}`. `hide_off_releases` is
  true only when the bank hides its keys and the version is one hide-off checks (`hideOffCandidates`: at least one open
  send flag on it says `via: "hide_key"`, every open send flag on it does, kept in full, within `HIDE_OLDER_MAX + 1`
  and `HIDE_OLDER_BYTES`, newest first). A send's withhold (its flags have no `via`), a version withheld past the day's
  flags (no flag), and any bank that does not hide its keys say false. Nobody else is told (another editor, a viewer,
  an author demoted to viewer: the field is absent). The full form is not re-scanned on the read: hide-off still checks
  it and `restored` says what it released. It names no reason in words. Round 50 then narrowed the field to copyright
  matches an answer-key change raised (60 B2), so since then it (and `restored`) does tell the owner and the bank's
  author while an editor which kind withheld such a version; round 51 (critic 61 I1, the owner's decision) accepts
  that as the one exception to the round 44 rule, for those two only: every other member still gets no field and the
  same answer for both kinds. 49-1, 49-3, 51-4.
* **Hiding can be turned on again on a withheld head.** The critic's scenario (a send to a hiding bank withheld by a
  copyright match, hiding turned off: `restored` absent) leaves the head withheld with `hide_off_releases: false`; a
  PATCH `{hide_key: true}` then succeeds (no strip of a head nobody reads; the head keeps the decisions its send
  recorded), and once an administrator releases the head a viewer is served its viewers' form, never the keys. 49-2.
  The app keeps the switch on a withheld head both ways and offers "Turn hiding off" only when the field is true.

**Round 48 (phase C after critics 55a and 55b, `docs/critic-reports/2026-10-25-circles-phaseC-{product-critic55a,security-critic55b}.md`).**
Migration `0115_pool_round48.sql` (`circle_invites.email_key` and its partial index); `0114` edited in place (not applied
anywhere): `IF NOT EXISTS`, the index is now `(circle_id, status) WHERE withheld_at IS NOT NULL AND full_key <> ''` (55b
V6: pruned rows leave it); `0111` and `0113` got `IF NOT EXISTS` on their `CREATE TABLE`/`CREATE INDEX` (an `ADD COLUMN`
cannot have it). Tests: `test/pool/round48.test.ts` (10, each failed on round 47's code; the critics' `h1`, `a2`, `a3`,
`u1`, `r1`, `e1`, `i1` scripts were rerun and show the fixes; `a3`, `u1`, `e1` stop later on routes or expectations of
their own). Earlier tests changed where the behaviour changed on purpose: 40-3, 41-1, 44-7, 47-3 (purge-now that waits
for a review is `200 {purged_partial, pending_review}`), 42-3 and 46-2 (a second invite to one mailbox in one circle
rotates the first: one invite row), 47-6 (the inviter's own share of a domain has its own metric; "mail domain"), 47-10
(`409 hide_key_changed`), 32-1x and 37-x (role hint `owner_or_author` on the bank PATCH, trim and rollback).

* **55b B1, answer-key attempts are counted.** Every PATCH `{hide_key}` that does work (a strip of the head, checks of
  older versions, a restore) counts toward `circle_hide_attempts` (60 a day), refused ones included, and is not given
  back (a `503 busy` aside); past it `429 daily_limit {metric: "circle_hide_attempts"}` "You have tried to change
  answer-key settings as many times as you may today. Try again tomorrow." The day's 20 changes (`circle_hide_changes`)
  still count only what changed something. Refused before any work and not counted: a member without the right, the
  same setting again, a malformed decision (400), a decision for a question the head does not have (`400
  invalid_request {field}`). The app's two or three steps per bank fit (twelve banks are 36). 48-1.
* **55b B2, the evidence bound follows the hold rules and ranks per sender.** `evidenceBound` never prunes (nor counts)
  a version of a bank a report or an administrator holds, nor one an open report names (48-2: the administrator's hold
  keeps version 1 through a 7th flagged send). Heads are outside it. Each sender's newest 2 flagged versions in a bank
  and newest 5 in the circle are protected from everyone else's; the rest fill what is left of 5 a bank and 20 a
  circle, newest first, within a quarter of `circle_bytes` (48-3: one account's 40 flagged versions across five banks
  leave another member's older evidence in place, the circle keeps at most 20).
* **The owner's remedy, and the rule that cannot destroy what a pending flag needs.** A flag pins its version's body
  until the administrator decides it OR marks its evidence captured: `POST /admin/pool/circle-evidence-captured
  {bank_id, version?, note}` (sudo, audited; answers `{ok, flags}`). A captured flag stays open for the decision with
  its positions, hashes and takedowns (`evidence_captured`, `_at`, `_by` in its detail) and pins nothing: trim, the
  window, the bound and purge-now treat its version as any other, and the daily purge no longer waits for it. Purge-now
  of a removed bank whose only obstacle is an uncaptured flag is answered as the work it does (55a V5, 55b V10): `200
  {ok, purged_partial: true, pending_review: true, evidence_kept, message}` (every version but the pinned evidence is
  pruned, the purge is claimed, undelete is `409 bank_removed`); once every flag is decided or captured, purge-now
  purges the bank (`200 {ok}`). So a circle full of flagged withheld heads is freed by the owner's remove and
  purge-now as soon as the administrator has captured the evidence, without waiting for the decision. 48-4.
* **Limits fixed.**
  * The keyless message fits the questions it names (55a V1): `422 keyless_question {positions, kinds, message}`;
    `kinds` is each position's question kind; a `Short` (short answer or essay) is asked for a sample answer, others
    for the correct answer, and every message says to get the full bank again now that the sender can edit. A keyless
    `Short` is NOT accepted: a stripped copy cannot tell an essay from a keyed short answer (both `sample: ""`) and its
    explanation is the sentinel, so accepting it would lose the author's text (share-core has no essay kind). 48-5.
  * A send or rollback that loses only because `hide_key` changed meanwhile is `409 hide_key_changed {hide_key,
    head_version, message}` (send again; with decisions when hiding is now on), not a merge `conflict`. The PATCH's own
    lost race is `409 conflict {head_version, message}` (55a V2). 48-6.
  * Versions withheld past the sender's `circle_send_flags` are named on a bank-level `cap` flag (55a V3, 55b V3): one
    per bank, sender and day (`item_id` the bank, `user_id` the sender), its detail `{metric: "circle_send_flags",
    circle_id, bank_id, versions: [{version, version_id, actions, student_data}]}`; the admin mail goes when it is made.
    `/admin/dmca/unblock {release_circle_versions}` releases those versions by the same rule as flagged ones (every
    takedown of the entry lifted or expired, no student data, the body re-checked) and marks the cap flag
    `block_lifted`. The author's answer is the same `held`/`withheld` as for a flagged withhold. 48-7.
  * Pruned evidence is said as such (55a V4): a history line of a withheld version has `review_copy: "full"` (its text
    is still kept) or `"hashes_only"` (only the record of what matched is kept, for the administrator); reading such a
    version is `404 version_not_retained {withheld: true, review_copy: "hashes_only", message}`. Trim's
    `pinned_versions` now lists the evidence it keeps too, as many as `evidence_kept`. 48-8.
  * An email invite to an address that has an open (unused, unrevoked, unexpired) email invite in the circle rotates it
    (55a V6): a new code is mailed, the old one dies, the invite keeps its id and label, takes the new role and expiry:
    `202 {..., resent: true, message}`. Found by `email_key`, an HMAC of the circle and the mailbox (its own HKDF purpose),
    nulled with the label (inviter's erasure, the daily cron). A rotation counts as a mail (the day's and the hour's
    counts, the mailbox's 3) but not as a new code. 48-9.
  * Neutral words for a mail domain's hour: the inviter's own share is `429 hourly_limit {metric:
    "circle_email_inviter_domain_hourly", limit}` "You have sent as many invitations to addresses at this mail domain
    as you may in an hour (10). Try again in an hour."; the domain's hour is `{metric: "circle_email_domain_hourly"}`
    "There have been too many invitations to addresses at this mail domain in the last hour. Try again in an hour."
  * Erasure deletes the inviter's hourly domain counters (`circle_email_domain:<hmac>:<user id>`), in the erasure and
    the resweep (55b V4). 48-10.
  * One role hint (55a V7): `403 circle_role_required {needed: "owner_or_author"}` on the bank PATCH (before the body
    is read), the answer-key PATCH, trim and rollback. The licence change itself stays `needed: "author"` for an owner
    who is not the author. 48-10.
  * Hide-off re-checks a version with its own sender's exemptions (55b V7).
* **Documented, not changed.** 30 refused PATCHes still take 15 s of the heavy slot (55b B1's measure); the attempts cap
  bounds it at 60 a day per account (about 30 s), not per request. A withheld HEAD is never pruned by the bound; the
  owner's way out is a clean version, a rollback, or remove plus purge-now (the head kept until the flag is decided or
  captured). The per-sender protection lets several hostile accounts keep 2 a bank and 5 a circle each (bounded by the
  circle's members and their daily flags). A version withheld past the day's flags is not evidence (no flag pins it):
  trim and the window prune it; its record is the cap flag's entry and `circle_qkeys`. The cap flag's entries list grows
  with a sender's sends of a day (at most `circle_commits`). An unblock still checks at most 50 versions. The keyless
  guard's extra validation (55b V5) and a promoted viewer's need to re-read the full bank stay as they are; the client
  should re-import the full head when a viewer is promoted. A hide-on PATCH charges the PATCHer's held-publish count and
  flags under the PATCHer's id for older versions others sent (55b V11). The address limit is answered before anything is
  charged (55b V9, known). `APP_URL` in `wrangler.toml` is still the placeholder (55a V6): no staging mail before the
  client's join route and a real `APP_URL` exist.

**Round 47 (phase C after critics 54a and 54b, `docs/critic-reports/2026-10-25-circles-phaseC-{product-critic54a,security-critic54b}.md`).**
Migration `0114_pool_round47.sql` (one partial index, `circle_versions_withheld`). Tests: `test/pool/round47.test.ts`
(10; each failed on round 46's code, rechecked by putting the old line back for 47-5 and 47-10; the critics' `t1`-`t6`,
`pin`, `rel`, `m1`, `kl`, `kl2`, `rb`, `cpu` scripts were rerun and show the fixes). Earlier tests changed where the
behaviour changed on purpose: 42-3 (the account's own daily limit set to 20 for its 20-a-day part, the default is 40
now; the open-codes part counts email invites by their own cap), 46-3 (the account's own hourly limit set to 10, the
default is 30 now; a full domain is `429 hourly_limit`, no longer `503 busy`), 44-1 (a flag's detail also has
`hashes`).

* **54a B1, the keyless guard asks share-core.** A send (new bank or next version, `hide_key` on or off) is refused
  with `422 keyless_question {positions, message}` when share-core's own publish check without `hide_key`
  (`validate_for_publish`) names a question `keyless_needs_hide_key` (its `keyless` flag OR the sentinel explanation the
  app stores in place of it, `KEYLESS_SENTINEL`, wire.rs) or `placeholder_key` (a stripped copy's True/False `false`,
  `origin.form = "stripped"` and not `rekeyed`). It runs on the payload as sent (the client's `origin` is still on it);
  when that payload is refused for something else (a field the send drops, such as a malformed `origin`), it runs again
  on the form the send stores, so nothing hides the sentinel. No share-core change: the rule is share-core's, not a copy
  of it; the test reads the sentinel from `wire.rs` (it fails if the two drift). 47-1: a viewer promoted to editor sends
  her copy as the app holds it (sentinel, no flag): refused, the keys stay.
* **54a B2, refused answer-key PATCHes are free.** The day's count (`circle_hide_changes`, 20) is taken before the work,
  so concurrent PATCHes cannot pass the cap, and given back for every answer but a 200 that changed something: a
  `labels_need_decision` or `order_ack_required` step, any 4xx/5xx, a lost compare-and-set. 47-2: twelve banks through
  the app's three steps count twelve. At the cap a PATCH is `429` even if it would have been refused for its decisions.
* **54b N1, the evidence a circle keeps is bounded, and the owner can always free the rest.**
  * Of the withheld versions whose flag is open (the administrator's evidence, round 46), only the newest 5 of a bank
    and 20 of a circle, within a quarter of `circle_bytes` (the newest always), keep their bodies (`evidenceBound`, in
    every commit and every answer-key PATCH that can add evidence). An older one is pruned like a version past the 20
    (body, index and images go, the history line stays) and its flags get `evidence_pruned: true` (and
    `evidence_pruned_at`); they keep what they recorded: the positions, the takedowns (`actions`) and, new, the block
    hashes that matched (`hashes`, at most 20). A head is never pruned by the bound (see below). The admin's open of a
    pruned version (`GET /admin/pool/circle-bank`) is `404 version_not_retained` with `evidence_pruned: true`, a message
    and the flags' details.
  * One account raises at most `circle_send_flags` (5) student-data or copyright flags a day in one circle (sends,
    rollbacks, answer-key PATCHes; the flag's `user_id` is the sender). Past it the version is withheld and the bank held
    all the same, without a flag (so it is not kept as evidence and the window prunes it), the matches still count
    toward the held-publish budget, and the administrator gets one `cap` flag a day (`metric: circle_send_flags`).
  * Trim answers `evidence_kept` (versions it keeps for the administrator). Purge-now of a removed bank whose only
    obstacle is an open flag still answers `409 pending_review` (round 48: `200 {purged_partial, pending_review}`), and
    now also claims the purge (the bank can no longer be
    restored; the daily purge takes it once the review is done) and prunes every version but the evidence at once
    (`evidence_kept` in the answer). So the evidence a circle holds is at most a quarter of its bytes plus its withheld
    heads, and everything else is the owner's to free. 47-3: 61 blocked sends by one editor keep 5 flags and at most 25
    versions, a clean fix fits, trim keeps the head and 5, purge-now leaves only the evidence; 47-4: 5 banks x 6 flags
    keep at most 5 a bank and 20 in the circle, the rest marked.
  * The round-38/41 guarantees stand: a withheld version is never served to members; within the bound the admin opens
    it (47-3), trim keeps it, purge-now and the daily purge wait for its flag (41-1, 44-7).
* **54b N2, an unblock releases only what the takedown alone withheld.** With `release_circle_versions`, a version is
  released only when its body, checked again now as a send's is (student data exact and near, every active copyright
  block), is clean, no report or administrator holds the bank, no report is open on it, and every open send flag on it
  is a `blocklist` flag whose takedowns are all lifted or expired; else it keeps its `block_lifted` flag for the
  administrator's own `circle-release`. At most 50 versions are checked per unblock. 47-5.
* **Variants.**
  * Bulk invites (54a variant 2): `circle_email_invites` 40 a day (was 20), `circle_email_invites_hourly` 30 (was 10);
    open email invites have their own cap, `circle_open_email_invites` 40 a circle (`422 circles_limit {metric:
    "circle_open_email_invites"}`, a message that says invitations free up as people join or as they expire), apart
    from the circle's 10 plain codes (`circle_open_invites` now counts plain codes only); 3 mails a day per address stays.
  * A domain's hour (54a variant 3, 54b V1): one inviter has at most `circle_email_inviter_domain_hourly` (10) of a
    domain's 30, and the domain's last fifth goes only to inviters who mailed that domain fewer than 3 times this hour.
    Past either: `429 hourly_limit {metric: "circle_email_domain_hourly"}` "There have been too many invitations to this
    mail provider in the last hour. Try again in an hour." (Retry-After; it was a `503 busy` that read as an outage). So
    a teacher inviting 15 colleagues at one school sends 10 this hour and 5 the next; three heavy inviters on gmail.com
    leave a light one room (47-6).
  * Masked labels (54a variant 4, 54b V5): a one-character local part is not shown (`***@pvhs.example`); a label the
    circle already has gets ` #2`, ` #3`... (stored, listed and answered as made).
  * A viewer's hidden-read that serves nothing (409 `keys_hidden_unavailable`, 404, an error) gives its count back (54a
    variant 8).
  * Turning hiding off undoes what turning it on withheld (54a variant 1): a version withheld only by an answer-key PATCH
    (every open flag on it has `via: "hide_key"`) whose full form is clean now (checked as a send's, and again inside the
    batch) is readable again, its recorded decisions dropped (turning hiding on again checks it afresh), its flags
    resolved (`resolved_via: "hide_key_off"`, `resolved_by` the PATCHer), and when it is the head the bank's send holds
    go. The answer adds `restored: [versions]` when there are any. Bounded like the PATCH's older pass (newest first, 20
    versions, 4 MB). A version withheld past the day's flags (no flag) is not found this way (47-9).
  * A rollback compares `hide_key` at its commit, as a send does: turned on meanwhile, `409 conflict`, nothing stored
    (54b V9, 47-10).
* **Documented, not changed.** The answer-key PATCH's CPU (54b V8): the head and older versions within 4 MB of bodies are
  stripped and checked; the critics' 1,000-question bank with 10 versions (6.5 MB) took 1.4 s (4.7 s on the critic's
  run) of wall time in node, under the isolate's heavy slot, at most 20 changes an account a day; most of it is older
  versions that need label decisions and stay unrecorded. Not bounded further. A withheld HEAD is kept while it is the
  head, whatever the bound (one per bank; the owner sends a clean version, rolls back, or removes and purges the bank).
  An unblock's student-data check of a body is exact keys, images and near rewordings; a bank's other versions are not
  looked at (54b V6's 50-takedown truncation of `actions` stays). A purge-now answered `pending_review` cannot be undone.
  The pinned versions' `full_key` stays until the daily stage deletes their objects, as for every prune (the cap counts
  only kept versions at once).

**Round 46 (phase C after critics 53a and 53b, `docs/critic-reports/2026-10-25-circles-phaseC-{product-critic53a,security-critic53b}.md`).**
Migration `0111` edited in place (not applied anywhere): `circle_invites.email_label` and an index for its clean-up.
Tests: `test/pool/round46.test.ts` (12, each failed on round 45's code; the critics' `s4`, `e1`, `e3`, `c1`, `c2`, `s3`,
`p2` repros were rerun and now show the fixes). Earlier tests changed where the behaviour changed on purpose: 42-1 (the
answer's keys, the masked label is stored), 42-2 (the label differs per address), 42-3 (the fourth mail to a mailbox is
`422 invite_not_sent`, nothing made; the inviter's hourly cap raised for its 20-a-day part), 42-4 (a domain in another
script is an IDN, mailed in punycode), 42-6 (no circle name in the mail), 38-1 (a withheld version with an open flag
is pruned only after the flag is decided).

* **53a B1, keyless copies never become circle versions.** Any question with `keyless: true` (a copy made without
  the answer keys, such as a viewer's copy of a bank that hides its keys) is refused on every send, new bank or next
  version, whatever `hide_key` is: `422 keyless_question {positions, message}` ("These questions were copied without
  their answer keys, so they cannot be sent to the circle: the circle keeps the full questions with their keys. Set
  the correct answers first, or get a full copy from an editor."), nothing stored. (Round 43 let one through on a
  bank that hides its keys and the head lost its keys.)
* **53b B1, one mailbox is one address, and the hour cannot be spent by one inviter.** The address is read in NFKC
  (full-width letters are ASCII), an IDN domain in punycode, trailing dots dropped. The per-address cap counts the
  MAILBOX (`emailCapKey`): a `+tag` dropped (from the first `+`), Gmail's dots ignored and googlemail.com read as
  gmail.com. The hour's mail budget: `circle_email_invites_hourly` 10 per inviter (`429 hourly_limit {metric,
  message}`, Retry-After to the hour), `circle_email_domain_hourly` 30 per domain from everyone (an HMAC of the domain
  as the counter's key), `circle_email_hourly` 100 for the service of which the last fifth goes only to inviters who
  sent fewer than 3 this hour (`503 busy`). Only mails that go use the hour. 46-3: five accounts x 20 invites to
  dot-variants of one Gmail mailbox mail it 3 times; a legitimate teacher's invite in that hour goes.
* **53b B2, viewers' forms are checked.** For a bank that hides its keys the student-data and copyright checks run
  on the form its VIEWERS are served too (each position's keys and stems are the union of the full and the stripped
  form's), on every send, every rollback (the target stripped with its recorded decisions) and the PATCH that hides the
  keys (the head stripped with the decisions sent). A match withholds and holds exactly as a send's match (round 37/44):
  the PATCH answers `held` and `withheld`, flags (`via: "hide_key"` in the detail) and mails the admin, and counts toward
  the held-publish budget. The PATCH also strips and checks OLDER kept versions with no decisions recorded (newest first,
  at most 19 and 4 MB of stored bodies): one that passes records its (empty) decisions, one that matches is withheld and
  flagged (no bank hold: it is not the head). A viewer now reads ONLY versions with recorded decisions (whose served form
  was checked): any other is `409 keys_hidden_unavailable` before anything is stripped (a HEAD and a 304 too).
* **The invitation mail** (53a variant 7, 53b V1) carries no text any person chose: no circle name, no inviter (the
  app's join preview shows both). It says what joining needs and gives the expiry to the minute. **Exact text, for the
  owner to approve** (subject `verSHion: you are invited to a circle`):

  > You are invited to join a circle on verSHion as an editor. A circle is a small private group of teachers who share
  > question banks. The app shows you the circle's name and its owner before you join.
  >
  > To join, you need a verSHion account that an administrator has approved and that has redeemed an invite code for
  > the shared pool. If you do not have one, this code cannot be used yet.
  >
  > In verSHion, open Shared, then Circles, then "Join a circle with a code", and enter this code:
  >
  > C-ABCD-EFGH-JKMN-PQRS-TVWX-YZ01
  >
  > Or open this link:
  > https://app.example.org/circles/join#code=C-ABCD-EFGH-JKMN-PQRS-TVWX-YZ01
  >
  > The code works once and expires on 2026-10-31 12:00 UTC. If you did not expect this invitation, ignore this mail:
  > nothing happens unless the code is used.

* **The inviter is told whether a mail went** (53a variant 1, 53b V2). `202 {..., emailed: true, address_label}`, or
  `422 invite_not_sent {emailed: false, reason, message}` with nothing made and the day's counts given back: `reason:
  "address_limit"` (that mailbox had its 3 mails today, from anyone) or `"not_deliverable"` (an address this service's
  mailer does not send to: a top-level domain that is not letters). Both depend only on the address and today's mails
  to it, never on accounts. A provider failure after the 202 is not known to the request (logged; the owner can revoke
  and send a code by hand).
* **Smaller fixes.** The owner's list (`GET .../invites`) and the export show `address_label` ("j***@gmail.com",
  made at invite time; nulled with the inviter's erasure and by the daily cron 30 days after the invite expired) (53a
  variant 2). An invite to a full circle is `409 circle_full` before anything is made (53a variant 3). A failed write
  gives the day's counts and the hour's slots back (53b nitpick). The hide_key PATCH: a member who is neither the
  circle's owner nor the bank's author is refused before the body is read (53b nitpick); the same setting again (for
  on: the decisions the head already recorded, or a withheld head) is `200 {unchanged: true}` with no audit line and no
  charge; every change counts toward `circle_hide_changes` (20 a day, `429 daily_limit` with a message) (53b V4). The
  hidden-read cap's 429 has a message (53a nitpick). A viewer's ETag also covers the version's recorded decisions, so
  re-recorded decisions change it (53a variant 3, 53b V6). A withheld version whose student-data or copyright flag is
  open is no longer pruned by a commit (nor counted as room the commit frees: `PINNED_EVIDENCE_SQL` in the prune and in
  `PRUNABLE_SQL` alike); once the flag is decided, the next commit prunes it (53b V7). The admin's `basis=copyright`
  ignores expired blocks, as a send does (53b V8; the sender's exemptions are not applied there: a bank has several
  senders, and the basis only lets an admin open it, audited). `/admin/dmca/unblock` marks the open `blocklist` flags of
  circle versions that only lifted or expired blocks held `block_lifted: true` and, with `release_circle_versions:
  true`, releases those versions in the same call (readable again, flag resolved, a head's copyright hold lifted,
  listing refreshed; never a version whose student-data flag is open); the answer adds `circle_flags_lifted` and
  `circle_versions_released` (53a copyright variant 2). Each `blocklist` flag now names the takedowns that matched
  (`actions`). The activity cursor must be a line of this circle at the time it names, else `400` (53b nitpick).
* **Documented, not changed.** A held sender is not told which positions matched: the public path tells a publisher
  only "held for review" for a content check's hold (positions only for an administrator's hold-copies), so circles do
  the same; the admin's flag has them (53a copyright variant 1). An existing member's invitation is mailed and its code
  stays open until used or revoked (53a variant 4). An owner who leaves or passes the circle on revokes her pending
  email codes (round 33) and the mail names nobody, so the new owner re-invites (53a variant 5). A viewer's served order
  is a fresh shuffle at each read under one ETag: a client compares versions by ETag, never by the bytes (53a hide-key
  2). Recorded decisions cannot be read back; a send to a bank that hides its keys supplies its own each time, and an
  editor's decisions replace the author's for that version (positional; 53a hide-key 4). Older versions past the PATCH's
  bound, or refused without decisions, stay refused to viewers; a PATCH that would record what the head already has is
  a no-op and checks nothing older (53a hide-key 5). Many viewers of one circle can keep an isolate's heavy slot busy
  (each bounded per account; 53b V5). A local part like `a..b` passes the address rule (the provider may bounce it).
  A version release (`circle-release {version}`) clears every send hold and flag of that version, student-data and
  copyright alike (53b V8). An address whose local part starts with `+` counts as the domain's "empty" mailbox (one
  cap for all such addresses). The HMAC rows and the masked labels can outlive their clean-up in D1 backups (plan 8.1).

**Round 45 (phase C: the activity list, D8).** No migration.

* **Kinds for the new events.** `email_invited` (`by` the inviter, `member: null`, `role`; never an address or an id)
  and `hide_key_changed` (`by`, `bank_id`, `bank_title`, `hide_key: true|false`). Recording new strip decisions for a
  bank that already hides its keys is audited as `circle.bank_strip_decisions`, not an activity line.
* **Stable pages.** The cursor is now `<at>:<audit row id>` (the row's PERMANENT id; round 32's was `<at>:<rowid>`,
  a service-wide row number every member could read). The written order within one millisecond is kept: the row's
  `rowid` is looked up by its id, by key. A later page seeks its start in the `(item_id, at)` index (rows read do not
  grow with the page's depth). Every line that existed at the first page comes exactly once, whatever is written
  meanwhile; a malformed `before` is `400 invalid_request {field: "before"}` (it used to be page one again). The app
  treats `next_before` as opaque, so nothing changes for it.
* **No withheld title, erased actors.** Lines carry the bank's listing, which never comes from a withheld version
  (student data, round 39; copyright, round 44); an erased actor or member is "deleted teacher" on every line.

**Round 44 (phase C: copyright re-upload block checks on sends, plan 13.3).** Migration `0113_pool_round44.sql`
rebuilds `circle_holds` to add the reason `copyright` (its CHECK list; rows copied as they are, index made again).

* **The check.** Every send and rollback checks the version's block keys (the same keys as the student-data check:
  key-free match hash, normalised skeleton, stem, full content, explanation, answer, each Multipart part's) and its
  images against the public pool's re-upload blocks (`blocked_hashes`), by the public path's rules: an active
  (not lifted, not expired) block of a takedown; never one the admin exempted the sender from (`dmca_exempt`); never
  the rightful item's own material for its author, or for an account it was served to while visible (an import
  record; `served.ts rightfulExempts`, with no publish provenance since circle versions have none). Scanned before the
  commit (positions for the flag) and decided again inside the commit batch (a block confirmed meanwhile still
  holds: no gap).
* **A match never refuses (mirrors the public path and the student-data check).** The version is withheld from every
  member (`withheld_at`), the bank is held while it is the head (`circle_holds` reason `copyright`, source
  `blocklist`; a non-blocking hold like a student-data one: the owner and the author still see and repair the bank,
  prune and trim go on), a later version without the material lifts the hold, only the administrator releases the
  version. The send answers `held: true, withheld: true`, exactly as for student data.
* **No oracle beyond the student-data one.** Every member-visible response is identical for a copyright match and a
  student-data match (the send's answer, the 404, the history line, the circle page, `heads`, the purge refusal;
  44-2 compares them). Round 51 (the owner's decision on critic 61 I1): one exception, for the circle's owner and the
  bank's author while an editor only: `hide_off_releases` and hide-off's `restored` tell them whether an answer-key
  change's withhold was a copyright match (see "Round 49" and "Round 51"); nobody else can tell the two apart (51-4). The two member-facing messages that named student data now name no reason:
  `version_withheld` "This version is withheld until an administrator has reviewed it. Only an administrator can
  release it." and `pending_review` "An administrator is reviewing a version of this bank; it can be purged once that
  review is done." (wording for the owner to approve). The public path tells a publisher only "held for review" after
  the fact; a circle sender learns the same, at once (the round 37 `held` oracle, documented in round 39, now also
  covers the copyright block list, no richer).
* **The probe budget.** The matching questions count toward the account's `held_publishes` (the public path's daily
  budget, shared by publishes and sends); past `HELD_PUBLISHES_PER_DAY` (20) the admin gets a `cap` flag.
* **The admin.** One `blocklist` flag per matching version (`item_id` the bank, `publish_id` the version, detail
  `{circle_id, version, positions, rollback_of?}`, no text) and a mail (`circle_copyright_match`, its own hourly budget,
  ids only, the open and release calls). `GET /admin/pool/circle-bank?...&basis=copyright` (a `blocklist` hold or flag,
  or an active block on one of its key-free hashes). `circle-release {bank_id, version}`, the bank-wide release,
  `circle-remove` and `circle-close` treat the `blocklist` flag as they treat `privacy_text_match`; the purge (owner's
  and daily) and the trim wait for it the same way (round 41).
* **Documented, not changed.** Not retroactive: a version sent before a takedown stays readable (a rollback copy of
  it is withheld). An unblock (`/admin/dmca/unblock`) does not release circle versions or holds (the public path
  releases orphaned item holds; a circle version keeps no block keys to re-check): the flag stays in the admin's queue.
  The public path's 90-day release memory is not applied to circles (as for student data). Past the held-publish
  budget the public path puts a persisting `probe_cap` hold on the publisher's own item; a circle bank may be someone
  else's, so circles only flag the admin. The exemptions' "served" branch reads import records only.

**Round 43 (phase C: hide keys from viewers, D12).** Migration `0112_pool_round43.sql`: `circle_banks.hide_key`
(0/1) and `circle_versions.strip_opts` (JSON `{labels, order_acks, ack_formatting, image_ok, image_shas}`).

* **Who sets it.** `PATCH /circles/:cid/banks/:bid {hide_key: true|false, labels?, order_acks?, ack_formatting?,
  image_ok?}` by the bank's author while an editor or the owner, or by the circle's owner (else `403
  circle_role_required {needed: "owner"}`; a non-member 404). One setting per request (`license` and `hide_key`
  together: 400; decisions only with `hide_key: true`). A new bank may also say `hide_key: true` (with its decisions)
  in its first send's meta; a PUT's meta may not carry `hide_key` (400). The licence stays the author's.
* **What viewers get.** A VIEWER (the role at read time; a demoted author too) reads EVERY version of such a bank (the
  head, `versions/:n`, a copy) as share-core's stripped form: the public pool's strip and key-hint gate, never a rule
  of the Worker's (every `correct` false, True/False false, blanks' accepted answers emptied, samples blank, Matching
  rights in a shuffled word bank, explanations and figures removed, images only where `image_ok` keeps them, labels as
  decided, `keyless: true`; the qids stay). Owner and editors read the full text; the stored bodies never change.
  `bank.hide_key` (every member) and `bank.keys_hidden: true` (a stripped answer) in the head; `hide_key` per bank on
  the circle page. Version messages are blank to viewers of such a bank (head and history: editors write them and
  nothing checks them). The bank's image route serves a viewer only images a kept, readable version's stripped form
  keeps (`strip_opts.image_shas`).
* **Decisions, per version.** Turning it on strips the HEAD with the decisions sent (refusals exactly as a hidden-key
  publish's, to the caller: `422 invalid_payload | labels_need_decision | order_ack_required` with `errors`,
  `label_findings`, `needs_order_ack`, `order_reasons`, `order_preview`, `formatting_findings`; nothing changes) and
  records them with it; `200 {ok, hide_key: true, labels_removed, images_kept, images_removed, figures_removed,
  order_reasons}` (a withheld head is not stripped: `{ok, hide_key: true}`). Turning it on again records new
  decisions for the head. Every send to such a bank passes the gate with its own meta decisions (the same field names
  as a publish's) or stores nothing; it answers `keys_hidden: true`. A rollback copies its target's decisions. What is
  recorded is what the strip used (a decision per label finding, the frozen orders, the images kept, acknowledgements
  of existing questions), so it is bounded by the bank, not by the request.
* **Fail closed.** At each viewer read the version is stripped with the decisions it recorded (round 46: a version with
  none recorded is refused to viewers, and the PATCH records them for older versions it can check). A version
  the gate refuses that way (sent before the flag with an undecided label or a key in its text; a rollback to one) is
  `409 keys_hidden_unavailable {message}` to viewers, no text, no findings; editors still read it; the author or owner
  re-records decisions for the head with the PATCH.
* **Cost.** A viewer read is publish-sized work: the isolate's heavy slot (shared; 503 busy past its queue, the read
  is given back), a send's reservation for the stored body (`max(15 x bytes, values x 96 x 2)`), the read's own
  reservation of the stripped bytes until they are streamed, `item_reads` and a new daily cap `circle_hidden_reads`
  (100). If-None-Match strips nothing and is not counted; a viewer's HEAD has no `Content-Length` (it is not known
  without stripping); the ETag differs from an editor's. Nothing is cached: each read strips again (a fresh uniform
  shuffle, share-core's own seed; the author-only served orders are dropped at once). Measured in node: a
  990-question 450 KB bank, 150-200 ms and 10 statements a viewer read; a send of it with the gate 506 ms.
* **D13.** Where a label stripped for viewers changes a question's key-free match hash, that served-form hash joins
  `circle_qkeys` under the FIRST sender of the full text (copied from its record; one primary-key probe per position),
  counted in `circle_records`: a viewer's copy published publicly is refused `circle_material`, its first sender
  passes. (Without it a viewer's labels-stripped copy published as new: 43-7.)
* **Documented, not changed.** The bank's listing title is the author's (`meta.title`), not run through the key-hint
  gate, as a public item's title is not. The circle's history counts (`added`/`changed`/`removed`) still tell a viewer
  that some question changed. A rollback to a version without decisions leaves viewers refused until the PATCH. There
  is no "what viewers will see" preview route (the PATCH answers the strip's report only).

**Round 42 (phase C: email invites, D5).** Migration `0111_pool_round42.sql`: `circle_invites.kind` (`code` |
`email`) and `circle_email_sends (addr, day, n)`.

* **Route.** `POST /circles/:cid/invites/email {email, role: editor|viewer}` (the owner: only owners issue codes, and a
  code dies with its issuer's owner role, round 33; there is no editor invite right to give). `202 {ok, invite_id,
  kind: "email", role, max_uses: 1, expires_at}`, never the code or the address. The server makes an ordinary
  single-use circle code (7 days; only its hash stored, as any code) and mails it; the join is the usual `POST
  /circles/join` with every gate. Any other body field is `400 {field}` (the inviter adds no text to the mail); a bad
  address is `400 {field: "email"}` (printable ASCII only, trimmed, lower-cased, the auth package's address rule:
  no line break, space, comma list or look-alike letter can reach a header).
* **No oracle.** Nobody's address is ever looked up: the answer, the statements run and the mail are the same for any
  address (42-2 compares them).
* **Caps.** `circle_email_invites` 20 a day per inviter (`429 daily_limit`), and the code caps it also counts toward
  (`circle_invites` 50 a day, the circle's 10 open codes `422 circles_limit`); `circle_email_hourly` 100 mails an hour
  for the whole service (`503 busy` with Retry-After to the hour, nothing kept, the day's counts given back);
  `circle_email_address` 3 mails a day per address across every circle and inviter. (Round 46: past it the inviter is
  told, `422 invite_not_sent {reason: "address_limit"}`, nothing made; the cap counts the mailbox; per-inviter and
  per-domain hourly caps; see "Round 46". Round 47: 40 a day, 30 an hour, 40 open email invites a circle apart from its
  10 codes, one inviter's share of a domain; see "Round 47".)
* **Only an HMAC.** The per-address count keeps `HMAC(circle-invite-email/v1:<cap key>)` under its own HKDF purpose of
  AUTH_SECRET (`circle-invite-email`), one row per address and day, deleted by the daily cron after two days. The
  address and the code exist only in the in-memory notice handed to the mailer (no outbox, no audit, no log line);
  42-1 sweeps every table for them.
* **The mail** (`poolmail.ts circleInviteText`, MAIL_MODE log in 42-6): a fixed subject, one recipient, nothing about
  the inviter, the code and a link holding it in the fragment (`APP_URL/circles/join#code=...`, left out without
  APP_URL). A bad address or code shape is never mailed. Round 42's text quoted the circle name; **round 46 replaced it
  (no circle name at all): the text to approve is in "Round 46".** Round 42's text, for the record:

  > You are invited to join the circle "Algebra team" on verSHion as an editor. A circle is a small private group of
  > teachers who share question banks.
  >
  > To join, you need an approved verSHion account. In verSHion, open Shared, then Circles, then "Join with a code",
  > and enter this code:
  >
  > C-ABCD-EFGH-JKMN-PQRS-TVWX-YZ01
  >
  > Or open this link:
  > https://app.example.org/circles/join#code=C-ABCD-EFGH-JKMN-PQRS-TVWX-YZ01
  >
  > The code works once and expires on 2026-10-31 (UTC). If you did not expect this invitation, ignore this mail:
  > nothing happens unless the code is used.

* **Erasure (plan 8.1).** No mail for an erased or suspended inviter: a suspended account is not let in, and the write
  is guarded by ALIVE_SQL and the inviter's standing in the same statement (a request in flight at the erasure writes
  and mails nothing, 42-5). The inviter's erasure revokes its email invites with its other codes. Nothing can match an
  invite to an address afterwards (nothing holds one).
* **The owner's list.** `GET /circles/:cid/invites` adds `kind` per code; an email invite is revoked with the usual
  `POST .../invites/revoke`. The export's `invites_created` adds `kind`. Activity: `email_invited` (round 45).
* **Decisions for the owner.** The code is not bound to the address (a forwarded mail works, like a forwarded code).
  There is no opt-out list (it would keep addresses). The circle name in the mail can carry a URL-like text the owner
  typed (80 characters, plain text). The web app must recognise `/circles/join#code=` (or the link line can go). The
  silent per-address suppression means an inviter is not told a mail was not sent.

**Round 41 (circles phase B after critic 48, no blocker; `docs/critic-reports/2026-10-24-circles-round40-critic48.md`).**
Migration `0110_pool_round38.sql` edited in place again (still uncommitted). Tests: `test/pool/round41.test.ts` (4,
each failed on round 40's code).

* **48 V1, the admin's evidence is not purged before the admin looks.** While a `privacy_text_match` flag on a bank is
  open (the admin has not released, resolved or removed it), the owner's `POST .../purge` answers `409 pending_review
  {message}` ("An administrator is reviewing a version of this bank for student data; it can be purged once that review
  is done."), and the daily purge of a removed bank, or of a closed circle holding such a bank, waits like it does for
  a hold or an open report. The owner can still roll back, send a clean version and trim (a trim keeps the flagged
  version), so the bank is usable meanwhile. The admin's version release, the generic flag resolve, a circle-remove and
  (new) an admin circle-close resolve the flags and let the purge follow; the mail says the bank cannot be purged until
  the admin decides.
* **48 nitpick, a notice without a version.** A student-data notice queued before round 40 (no `version`) renders no
  `version=` link and no `"version": 0`; it points the admin at the flag, which names the version.
* **48 V4, 0110 on a database with a withheld head.** The backfill records a listing only for a current version that
  is not withheld, and a bank whose current version is withheld gets its listing blanked (on a fresh database neither
  case exists; it covers a database that ran an earlier edit of 0110). 41-4 runs 0110 on a populated 0109 database and
  on one with a withheld head.
* **Documented, not changed.** The owner of a bank held for student data learns who sent the withheld version, when,
  its counts and that it was withheld (as members do once the hold lifts; no text) (48 V2). A sender who edited a
  question in place can be probed once by an exact guess (`changed: 0` versus `1`): it needs the whole question and
  each probe is a committed version compared against the guess afterwards (48 V3). A declared origin on a NEW question
  is not checked against the public item's text: an editor can credit any visible public item (and so bind the licence)
  until the author removes that question (48 V5; round 39's rule ignores it only on questions the head already holds).

**Round 40 (circles phase B after critic 47a; critic 47b found no blocker; `docs/critic-reports/2026-10-24-circles-phaseB-product-critic47a.md`).**
No new migration (`0110` unchanged since round 39). Tests: `test/pool/round40.test.ts` (6, each failed on round 39's
code; the critic's `or`, `left` and `mig` scripts were rerun).

* **47a B1, a credit no longer depends on where a copy sits.** When a qid is sent more than once, the occurrence that
  keeps it is the one whose content is unchanged from the head (keyed content hash; the first occurrence if none is),
  not simply the first in the list. The bank's AUTHOR's copies of a credited question, exact or adapted, all carry its
  recorded origin, so "duplicate, adapt the copy, drop the original" keeps the credit and the licence bound in either
  order (an adaptation edited in place keeps it too). A non-author's copy carries it only as an exact copy (round 39),
  now in either order: an editor's adapted copy is a new, uncredited question, so once the author drops the original
  the bank is free of that origin (documented: an editor's adaptation of a credited question is credited only if the
  author re-declares it; dropping a credited question and re-adding it later is a new question too).
* **47a V1, the owner can recover a bank held for student data.** A bank whose only holds are student-data holds a
  send raised is no longer hidden from the circle's OWNER: it is listed to the owner (with its listing, which never
  comes from a withheld version), and the owner reads its readable versions, rolls it back, sends a clean version or
  trims it, as its author can (the withheld version stays unreadable to both; the update check and the activity list
  follow). Such a hold no longer keeps a removed bank from its purge (the owner's purge-now and the daily purge): the
  purge deletes the withheld text, which is what that hold is for; the admin's flag stays (ids, positions). This
  covers an author who left, was suspended (remove and purge) or was erased with the bank kept. A report's or an
  administrator's hold still hides the bank from the owner and still stops trim and purge.
* **47a V2, the admin mail names the version.** The circle student-data mail says which version matched, links
  `circle-bank?...&version=n&basis=privacy`, and says to release THAT version with `{bank_id, version, note}`, noting
  that a bank-wide release leaves the version withheld (`PoolNotice` `privacy_text_match` gains `version`).
* **47a V3, a rollback to a version from before 0110.** Such a version has no recorded listing; its title is now its
  own bank name from its stored body (the bank's current subject, course and grade are kept). The app sends the bank
  name as `bank.name`; if a client's `meta.title` differed from it, that older title is not recoverable (it was never
  stored per version).
* **47b V2, the listing refresh reads the history once.** One row-value `UPDATE ... SET (title, subject, course, grade)`
  with a single search of the bank's versions instead of four.
* **README.** The round 37 and round 38 lines that said a rollback keeps the bank's listing now say it restores the
  target's (round 39).
* **Documented, not changed.** A title is last-writer-wins on every send (an editor merging from an old copy can rename
  the bank back; a rollback restores it) (47a V4). The bank's author (while an editor) or the owner can trim away every
  co-editor's merge base with `keep: 1` (recorded as `versions_trimmed`) (47a V5). A rollback to a version sent before
  a block was confirmed makes a new, withheld version (47a V6, 46b V3). The account export lists versions sent without
  their bodies, as the plan says; with phase B a circle can be the only place that content lives (for counsel, D9)
  (47a V8). The nitpicks (a version release of a pruned version answers 200; the 413 names the owner's trim even to an
  editor who cannot trim that bank; a retry of a `commit_id` from before a purge re-creates the bank) stand.

**Round 39 (circles phase B after critics 46a and 46b, `docs/critic-reports/2026-10-24-circles-phaseB-{product-critic46a,security-critic46b}.md`).**
Migration `0110_pool_round38.sql` (uncommitted, so edited rather than a new `0111`) now also adds
`circle_versions.listing` (JSON `{title, subject, course, grade}`; NULL for versions sent before it) and fills it for
each bank's current version from the bank row. Tests: `test/pool/round39.test.ts` (8, each failed on round 38's
code; the critics' scripts were rerun); round 38's 38-6 now sends an EXACT copy (a changed copy no longer carries
the credit, 39-4).

* **46a M1, a held bank at the byte cap can be repaired.** A student-data hold a send raised (`circle_holds` source
  `privacy` / `privacy_near`) no longer freezes pruning: the matching version is withheld, which is what protects the
  data. Only a report's or an administrator's hold (any other source: evidence for the admin) still stops the
  commit's prune, the cap's `PRUNABLE_SQL` (the same predicate, `BLOCKING_HOLD_SQL`) and a trim (`409
  under_moderation`). So at the cap the author's clean version of the same size, a rollback and a trim all go
  through on a held bank. (A fix that is larger than what the commit prunes is still `413`, and the trim the message
  names now works on such a bank.)
* **46b N1, a withheld version never names the bank.** Every version records its own listing fields; in the same
  commit batch the bank's `title`, `subject`, `course` and `grade` are set from its NEWEST VERSION THAT IS NOT WITHHELD
  (blank when there is none), and an admin's version release refreshes them the same way. A rollback records its
  target's listing (a version from before round 39: the bank's current one), so a rollback to v1 lists v1's title. The
  circle page, the head read, the activity list and the update check never show a withheld version's title, after a
  rollback, a clean version or a bank-wide release.
* **46b N2, lost commits record their orphans.** A commit that loses its compare-and-set, and an abandoned attempt a
  retry with the same `commit_id` discards, insert their two R2 keys into `r2_orphans` in the same batch that deletes
  the staging row (as the hourly `discardCircleStaging` does), so a failed R2 delete is retried hourly.
* **46b V1, no grafted or imposed credit.** A repeated `published.qid` carries a recorded origin only as an EXACT copy
  (the same key-free match hash; a changed copy is a new question with no credit). An origin a NON-author declares on a
  question the head already holds (its qid kept) is ignored, so an editor cannot put a public item's credit and
  licence bound on the author's own question; an editor's declared origin on a NEW question is recorded (credit and
  bound). The first occurrence of a qid keeps its recorded origin even when edited (an adaptation keeps its
  attribution); an editor who rewrites a credited question completely while keeping its qid leaves the credit on it
  (the author removes the question or rolls back).
* **Variants fixed.** A trim keeps a withheld version while its `privacy_text_match` flag is open (the admin's
  evidence; the commit's own prune still drops it past the 20); once the flag is resolved (version release or the
  generic resolve) the next trim prunes it, and resolving a flag never makes the version readable (46a v3).
  `oldest_kept` and `pinned_versions` count readable versions only: the run starts at the newest version that is kept
  and not withheld, a withheld version breaks it, and `pinned_versions` lists readable versions older than the run
  (kept by a report or a hold, or beyond a withheld version) (46a v8). A send's meta, its `commit_id` replay, the head
  and licence check and the merge token are handled before the heavy slot and its reservation, so a stale send or a
  replay costs a lookup (the head is read again after the slot) (46b V6). The licence change is a compare-and-set on
  the head whose origins it checked (`409 conflict` if a version landed meanwhile) (46b V7). A commit or trim deletes
  the image rows of the versions IT pruned only (by the prune index), not of every pruned version of the bank (46b V4).
  A rollback extends the account's pending hold on the images it copies, as a send does (46b V9).
* **Documented, not changed.** A withheld head still hides the author's whole bank from every other member (owner
  included) until a clean version or an admin acts, and the sender cannot read or fix it (46a v1; 45a 4). After a
  bank-wide release the bank is listed while its head answers `version_withheld`; the admin releases the version with
  `{version}` (46a v2). A released text is not an allow-list: sending it again withholds the new version (46a v4).
  Editors cannot merge with a withheld head (their send needs `base_version` = the head, from the history or a 409)
  (46a v5). `held`/`withheld` in a send's answer tell its sender that the text matched (as a public publish's "held for
  review"; capped by D10) (46a v6). `rollback_of` may name a version the same commit pruned (46a v7). A block confirmed
  after a version was sent is not retroactive: that version stays readable (the admin finds it by `basis=privacy`); a
  rollback copy of it is withheld (46b V3). A held bank never prunes, so `BYTES_SQL` and `keptInfo` read its unpruned
  versions (46b V5). The daily prune stage deletes at most 500 bodies a day (46b V8). `basis=privacy` does not expire
  (46b V10). A withheld line still shows who sent it, when, its counts and licence (46b V11). The nitpicks (trim
  answers 404 to a non-author owner of a held bank and `pruned: 0` when nothing qualifies; a copy placed before its
  original takes the qid) stand.

**Round 38 (circles phase B after critics 45a and 45b, `docs/critic-reports/2026-10-24-circles-phaseB-{product-critic45a,security-critic45b}.md`).**
Migration `0110_pool_round38.sql`: one column, `circle_versions.withheld_at` (`ALTER TABLE ... ADD COLUMN`, no rebuild,
no existing row changes; the next is `0111`). A column rather than the admin flag because flags are resolved by
other paths (the generic flag resolve, an erased author's flags), which must never re-open student data. Tests:
`test/pool/round38.test.ts` (12, each failed on round 37's code; the critics' repro scripts were rerun after the
fix); round 37's 37-5, 37-7, 37-9 and 37-13 follow the new rules (merge order, withheld versions, the kept-versions
query).

* **B1, student data is withheld per version (both critics).** Round 37 held the BANK while its head matched confirmed
  student data, and the next clean version lifted that hold, after which `GET .../versions/:n` served the matching
  version to every member. Now the commit that matches (exactly or as a near-rewording) also marks THAT version
  `withheld_at`, and no later version clears it. A withheld version is served to no member, its author included,
  whichever version is the head: `GET .../versions/:n` and the head read answer `404 version_withheld {message}`
  (round 49: and `hide_off_releases`); a rollback to it is `404 version_withheld`; a merge from it has `base_kept: false`; the bank's image route serves no
  image only it uses; its history (and merge `between`) line is `withheld: true, kept: false, message: ""`. The bank
  hold works as before (hidden while the head matches; a clean version or a rollback lifts it). A withheld version
  is pruned like any other once 20 newer ones exist (D16; the trim below too). Only an administrator releases it:
  `POST /admin/pool/circle-release {bank_id, version, note}` (audited `circle.admin_version_release`; its
  `privacy_text_match` flag is resolved; if it is the head, the bank's privacy hold goes too; `409 not_withheld`
  otherwise). A bank-wide release (`{bank_id, note}`) lifts holds and dismisses reports but releases no version and
  leaves those versions' flags open; `circle-remove` hides everything. The admin still opens a withheld version on
  `basis=privacy`.
* **B2, the byte cap counts what the commit prunes (45a).** The compare-and-set checked `circle_bytes` before the
  same batch pruned the version past the 20, so at steady state (sum of 20 versions near the cap; one bank with five
  2 MB images is 10 MB a version) the 21st version and every rollback were refused for good. The cap is now checked
  net of the versions that commit prunes (`PRUNABLE_SQL`: past the 20, not held, not named by an open report), and
  the prune runs before the commit's own holds so it prunes exactly that. A circle that is really full gets room
  back with a new route: `POST /circles/:cid/banks/:bid/trim {keep: 1-19}` (the owner, or the bank's author while an
  editor, D17's rights): versions older than the newest `keep` are pruned now (history lines stay, bodies stop
  counting at once, the daily stage deletes them; never while the bank is held, never one an open report names;
  `409 under_moderation` on a held bank) -> `{ok, pruned, oldest_kept, pinned_versions}`; activity
  `versions_trimmed`. The `413 quota_exceeded` message now says that ("every kept version of every bank counts,
  with its images. The owner (or a bank's author) can trim a bank's older versions, or remove a bank and purge it")
  and its `used` is net of what the commit would prune. Images are charged per kept version (D3), so a circle's
  editing room is about `circle_bytes / 20` of distinct content.
* **B3, a repeated `published.qid` is a copy (45a).** Both occurrences used to get new qids (the original counted as
  removed, its recorded origin and the licence bound lost). Now the first keeps the qid (and its origin); each later
  one is a new question that carries the same recorded origin. The large-change guard counts by the kept qids.
* **Variant 1 (45b), origins by text.** An origin the head recorded also follows a question sent WITHOUT its qid when
  its key-free match hash equals that question's (the head index's `m`, versions from round 37 on), so an editor
  re-sending the text without its qid no longer frees the author to relicense it. A reworded copy is not matched
  (the server checks declared and recorded origins only, plan 7).
* **Variant 1 (45a), what is kept.** Every response that said `oldest_kept` (history, merge-initiate, `404
  version_not_retained`, trim, the admin's) now also has `pinned_versions`: `oldest_kept` is where the contiguous run
  of kept versions that ends at the head starts, `pinned_versions` lists older versions a report or a hold keeps (at
  most 50). A line's `kept` (and merge's `base_kept`) stay the per-version truth.
* **Smaller fixes.** Merge-initiate's `between` is the FIRST 50 versions after the base, oldest first (45a variant 6). A
  demoted author no longer changes the licence: `PATCH .../banks/:bid` needs the author while an editor or owner
  (`403 circle_role_required {needed: "editor"}`, 45a variant 3). A rollback reserves memory by the body's value
  count when that is more than its bytes, as a send does (45b variant 4). A circle's student-data mail has its own
  hourly budget (`circle_privacy_match`), so it and the public pool's publish alerts cannot starve each other (45b
  variant 3).
* **Documented, not changed.**
  * The licence stays the author's: once the author is removed or erased, nobody changes it (45a 2). An editor's
    student-data version hides the author's whole bank while it is the head and only the admin is mailed (45a 4;
    45b 6: an editor holding text that matches a confirmed block can hold a bank). While a bank is held nothing is
    pruned or trimmed, so its bytes grow (45a 5). A send whose head index is missing in R2 skips the large-change guard
    and carries no origins (45a 7). A rollback restores the body, qids, images and origins (and, since round 39, its
    target's title, subject, course and grade) (45a 8). A replay answers the first answer's `held`/`withheld` (45a 9). The daily caps
    (100 sends, 20 rollbacks) are per account across all circles (45a 10). Dropping a public question frees the
    licence bound; a rollback to a version holding it is refused (`license_incompatible`) if the licence changed
    meanwhile (45a 11).
  * `merged_from` is the sender's claim (the token proves a merge was started at that head, not what was merged), and a
    token can be used for more sends at the same head until it moves or expires (45b 2). Circle sends and rollbacks
    share the isolate's one heavy slot (queue 4) with public publishes (45b 5). A rollback charges a full copy, net of
    what it prunes (45b 7). `origins` read the public item's author name live whatever the item's moderation state
    (45b 8). History and merge-initiate are metered by the rate limiter, not `item_reads` (45b 9).
  * Nitpicks: a `commit_id` of another account answers `409 commit_id_conflict` (ids are random); merge-initiate from
    the head is `400`; a version number past the head counts as an id miss; `oldest_kept` is computed on every history
    page (about 20 rows).

**Round 37 (circles, phase B: shared editing; `docs/circles-plan.md` 4, 6.2, 13.2; D16, D17).** No migration (the
phase B columns `base_version`, `merged_from`, `rollback_of` and the `circle_holds` source were already in `0108`; the
next is still `0110`). Code: `src/pool/circleversions.ts` (history, kept versions, merge-initiate and its token),
`circlesend.ts` (editors' sends, the large-change confirm, the student-data check, rollback; one commit shared by a
send and a rollback), `privacynear.ts` (the near-rewording check, moved unchanged out of `publish.ts` so both use it).
Tests: `test/pool/round37.test.ts` (17, each failed on round 36's code); `round32`/`round33`'s send helpers now send
`confirm_large_change` (their phase A tests replace whole banks), and 32-4 checks a viewer instead of a non-author.

* **What phase A already had** (`circlesend.ts`, round 32): POST (a new bank) and PUT (the next version) in the publish
  format, the compare-and-set on `base_version` inside the commit batch with every circle cap (`409 conflict
  {head_version, head_at, head_by}`, nothing stored), idempotency by `commit_id` (replay / `commit_id_conflict` /
  `publish_in_progress`), qids kept through `published.qid`, `added`/`changed`/`removed` per qid, images, declared
  public origins and the licence they allow, the account lock, the heavy slot and the byte reservation, staging + R2 +
  one batch, the D16 prune (never while held or a report names the version) and the hourly/daily cleanup. Phase B
  extends that path rather than adding another.
* **Editors send (plan 4.1).** PUT is open to the owner and every editor for any bank they may read (a held bank, or
  one whose author is not in good standing: its author only, others `404`; a removed bank `409 bank_removed`), checked
  again in the compare-and-set. The licence stays the author's (D19): an editor sends with the bank's licence, any other
  is `422 license_author_only {license}` (also in the commit, so a licence change racing the send cannot be
  bypassed). A send that waited for the heavy slot re-reads the head first.
* **Large changes.** A version that drops more than half of the head's questions (by qid; questions sent without
  their `published.qid` are new ones) needs `confirm_large_change: true`, else `428 confirm_large_required {removed,
  total, message}` (exactly half passes). Also on rollback.
* **History and kept versions.** `GET /circles/:cid/banks/:bid/versions?before=<n>` (a member who may read the bank):
  `{bank_id, head_version, oldest_kept, versions: [{version, version_id, by, at, message, added, changed, removed,
  question_count, license, kept, rollback_of?, merged_from?}], next_before}`, newest first, 50 a page (`?before=`
  the last line's version; `400` on a bad cursor). `by` is a display name or "deleted teacher"; `message` "" when none.
  `GET .../versions/:n` streams a KEPT version exactly like the head read (same shape; `bank.version` is `n`,
  `bank.head_version` the head, plus `bank.message`, `bank.rollback_of?`, `bank.merged_from?`; counts toward
  `item_reads`; ETag/304); a pruned one is `404 version_not_retained {oldest_kept, message}`, one that never existed
  (0, past the head) `404 not_found`. The head read gains the same `head_version`, `message`, `rollback_of?`,
  `merged_from?`. Fields that do not apply are absent, never null.
* **Merge-initiate.** `POST .../merge {base_version}` (owner, editor; `1 <= base < head`, else `400 {field,
  head_version}`) -> `{bank_id, base_version, base_kept, oldest_kept, head_version, head_version_id, head_at, head_by,
  between: [history lines base+1..head, at most 50; round 38: the FIRST 50 after the base, oldest first], between_total, merge_token,
  merge_token_expires_at}`. Nothing is stored or locked. The token is an HMAC (its own HKDF purpose, `circle-merge`)
  over `[expiry (30 min), account, bank, base, head]`. A PUT with `merge_token` must be based on that head, by that
  account, for that bank, unexpired, else `422 merge_token_invalid {message}`; the version then records
  `merged_from: base` (history, activity, result). A bare `merged_from` without the token is `400`; with it, it must
  match. When `base_kept` is false the app falls back to its two-way review (plan 4.1).
* **Rollback (D17).** `POST .../rollback {to_version, base_version, commit_id, message?, confirm_large_change?}` (the
  owner, or the bank's author while an editor; others `403 circle_role_required {needed: "owner"}`): a NEW version
  `base_version + 1` whose body, index, images and recorded origins are copied from kept version `to_version`
  (`rollback_of`), with the same compare-and-set and caps. `to_version >= base_version` is `400 {field: to_version}`;
  a stale base `409 conflict`; a pruned target `404 version_not_retained {oldest_kept}`; idempotent by `commit_id`
  (replay / `commit_id_conflict`); the bank keeps its licence (the copied origins must allow it, else `422
  license_incompatible`); its listing fields are the target's since round 39 (round 37 kept the bank's). 20 a day per account (`circle_rollbacks`, D10; not counted in `circle_commits`);
  a replay, a conflict, a 428 or a 503 gives the day's count back (sends too). Answer `200 {circle_id, bank_id,
  version, version_id, qids, added, changed, removed, rollback_of, held}`; activity `rolled_back {version,
  rollback_of}`. It is publish-sized work (account lock, heavy slot, 16x the body, and since round 38 its value count as a send's), because the copied body is
  checked for student data (below).
* **The confirmed-student-data check on every send and rollback (plan 13.2).** The public publish's check, on the
  version's questions (a circle bank is never stripped, so the served form is the full one): the same block keys
  (key-free match hash, normalised skeleton, stem, full content, explanation, answer, each Multipart part's) and
  images against `privacy_blocks`, and the same near-rewording check (`privacynear.ts`). A match never refuses (that
  would confirm the block): the commit batch re-checks the exact keys against the blocks as they are then (no gap)
  and holds the bank (`circle_holds` `student_data` / `privacy`, or `privacy_near` for a rewording), so it is hidden
  from every member but its author; a later version without it lifts that hold (an admin's or a report's hold stays). Round 38: the matching VERSION stays withheld from everyone until an administrator releases it (below).
  One `privacy_text_match` flag per matching version (`item_id` the bank, `publish_id` the version, detail circle,
  version, positions and counts only) and an admin mail (circle wording); `POST /admin/pool/circle-release` and
  `circle-remove` resolve those flags; the admin's `basis=privacy` now also accepts the bank's privacy hold or flag.
  Every send and rollback answers `held` (the bank is hidden from members now). The index of each version keeps the
  key-free match hashes (`m`) so a rollback needs no share-core run (an older version's are computed again).
* **Public material across editors.** An origin recorded for a question (declared at a send, checked then) now
  follows its qid into every later version (an editor's copy has no origin fields) and into a rollback, so the
  licence bound and the credits survive editors' versions. Versions no longer store the public author's name;
  `origins[].author_name` is read at each read ("deleted teacher" once that author is erased).
* **Tombstones (brief item 1).** Already blank since round 32: a purge sets `title`, `subject`, `course`, `grade` to
  "" (the columns are `NOT NULL`, so "" rather than NULL; nulling them would need a table rebuild for no gain) and
  deletes every version, image row, record, hold and R2 object; 37-8 checks no table keeps the text.
* **Cost (workerd, miniflare `meta.rows_read`, a circle at the caps: 100 banks x 20 kept versions).** History page
  131 rows (the same with 60 or 1,000 versions in the bank), head read 11, kept-version read 12, a pruned version's
  404 34, merge-initiate 37-130. The oldest-kept lookup is forced onto `circle_versions_bank` (`INDEXED BY`): on the
  `(bank_id, version)` index it walked every pruned version first (37-13 failed on it; the admin's circle-bank route
  used the same statement and now calls the same helper). A send of 990 questions reads about 21,600 rows (10,600
  in reads, 11,000 in the batch; 250 questions: 7,000), linear: the quota sum (2,100), the record probe (1,000) and
  the student-data keys (about 8 rows a question, twice: the scan and the in-commit re-check). A rollback of the same
  bank about 19,800. 37-13 checks every new read by index with its plan and flat timing from 60 to 1,000 versions;
  37-10 sends, reads, merges and rolls back a 1,000-question 3.8 MB bank with nothing left reserved.
* **Concurrency.** 37-2 runs a second send in a SECOND simulated isolate (its own heavy slot, `PoolDeps.heavySlots`,
  tests only) at every statement boundary of the first: exactly one wins each time, the loser gets the true head and
  leaves no row or object. 37-12: seven accounts sending at once in one isolate get one heavy slot, four waiting and
  the rest `503 busy` (Retry-After, the day's send given back, nothing staged, locked or reserved); five editors
  sending version 2 of one bank at once: exactly one 200. 37-9 runs two seeded sequences of 110 sends, stale sends,
  rollbacks, merges, prunes and an erasure against a model (linear versions, bodies, 409 heads, D17 rights, the 20
  kept). 37-11 erases the sender before every statement boundary of an editor's send, a merged send, an author's
  rollback and an owner's rollback (nothing but a normal erasure's residue, no staging row, no lock, versions linear).
* **Known limits.** A merge token can be replayed until it expires by the same account against the same head, which
  only records the same `merged_from` once (the head moves). An admin's release of a privacy hold does not stop the
  next version holding the same text again (the public pool's 90-day release memory is not applied to circles; the
  admin releases again or removes it). A send whose head index is missing in R2 skips the large-change guard (it
  cannot tell which questions were kept). `between` lists at most 50 versions (`between_total` says how many).

**Round 36 (circles after integration critic 44, `docs/critic-reports/2026-10-24-circles-phaseA-integration-critic44.md`).**
No migration. Tests: `test/pool/round36.test.ts` (2, each failed on round 35's code).

* **No null text in circle responses (B1).** A circle created or edited without a description stored and answered
  `description: null`; the app reads it as a plain string, so nobody could open that circle. Storage is unchanged
  (NULL still means "none"); every circle response now answers missing text as `""`: the circle's `description` and
  `name` (create, `GET /circles`, `GET /circles/:cid`, `PATCH`), a code's `note` (create and list), and in the export
  `circle_name`, a code's `note`, a version's `message` and a report's `note`. `POST /circles/heads` answers `by`
  "deleted teacher" rather than null. The create answer now also has `close_reason`, `closed_by` and `undo_until` (all
  null), as the list has. The test walks the JSON of every circle route a member, owner or code holder calls (create,
  detail, list, edit, codes, preview, join, send, read, heads, report, offer, activity, the export's circles section,
  a closed circle) and allows null only where it is documented:
  * timestamps and states: `closed_at`, `close_reason`, `closed_by`, `undo_until`, `revoked_at`, `removed_at`,
    `left_at`, `left_reason`, `resolved_at`, a report's `action`, a removed bank's `until`, `ownership_offered_until`;
  * `pending_transfer` (null: no open offer) and the paging cursors `next_cursor` / `next_before` (null: no more pages);
  * an activity line's `by` (a system action: a succession, a close by the last member), `member` (a circle-wide
    action) and `bank_title` (a bank the viewer may not see);
  * a hidden bank's `title`, `subject`, `course`, `grade` and `author_name` (`hidden: true`, round 35);
  * the publish check's `circle_material` hits: `circle_id`, `bank_id` (a held or suspended author's bank),
    `circle_name`, `bank_title` (a bank the publisher cannot see now), round 34.
* **Who closed a circle (critic 44 variant 2).** `GET /circles` and `GET /circles/:cid` add `closed_by`: `"owner"`,
  `"admin"` or `"last_member"` (null while open; the same fact as `close_reason`, as words). Only `"owner"` inside
  `undo_until` can be reopened, by the owner.
* **Variants not fixed here (client side or documented):** the client's silent parse fallbacks (variant 1), circle
  sends never declaring an origin (3), the keyless refusal's hide-key advice on a circle send (4), share-core codes
  without wording (5), short statements passing the publish check by design (6), the memberships-total and offer-limit
  wording (7, 8), the purge button on a held removed bank (10), the hidden-bank wording for an erased author (11), and
  no idempotency on create (13) are the app's or already documented; the README route table now matches the worker on
  variant 12's points: an offer to a non-member is `404 not_found`, restoring a purged bank is `404`, and `GET
  /circles/:cid` of a closed circle is `200` with its closed state.

**Round 35 (circles after critic 42, `docs/critic-reports/2026-10-24-circles-round34-critic42.md`).** No migration.
Tests: `test/pool/round35.test.ts` (3, each failed on round 34's code).

* **The owner finds removed banks whatever their author's standing (B1).** Round 34's standing rule hid an erased
  author's removed banks (`keep_on_delete: false`) and a suspended author's removed banks from the owner's circle page,
  while they still counted toward the circle's caps for 14 days. A REMOVED bank is now listed to the owner and its
  author whatever the author's standing; a live bank still needs its author in good standing (and no hold, but to its
  author). A removed bank that is held, or whose author is not in good standing, is listed without its text (`title`,
  `subject`, `course`, `grade`, `author_name` null, `hidden: true`, as F1's generic hit): the owner gets the id to
  purge (`POST .../purge`) or restore it, nothing a hold hides. The activity list follows the same rule.
* **Ownership offers are capped (B2).** 10 offers a day per account (`circle_transfer_offers`, `429 daily_limit`); the
  same open offer to the same member again changes nothing, writes nothing and is not charged; a cancel writes only
  when it removes an offer (so cancels are bounded by offers). Offer and cancel audit rows are keyed by the account
  (`item_id` NULL, the circle in `detail.circle_id`), not by the circle, so they never enter the activity list's scan
  of the circle's rows (the test counts that range before and after a day's capped offers and cancels: unchanged; an
  accepted transfer adds its one activity row).
* **Nitpicks.** No SQL comment inside a fragment other statements embed (`CODE_USABLE_SQL`); the accept binds the
  taker's id instead of writing it into the SQL; the export's `transfer_offers` lists open offers only.
* **Documented, not fixed (critic 42 variants).** (1) A succession in an owner-closed circle ignores `circles_owned`
  like an open one: a member owning five can hold a sixth for up to 14 days (leaving hands it on to the next member).
  (2) When nobody in good standing is left, an open circle closes (`last_member_gone`) and is purged on day 14 with its
  banks; a suspended member restored inside those days cannot reopen it (an owner-closed circle does come back: the
  daily net hands it on after the restore). (3) Members can infer a suspension: every bank of one author vanishes at
  once (`heads` says `under_review`, the publish check's hit is generic), which a hold of one bank does not show.
  (4) The owner learns a member's standing: an offer to a suspended or no longer approved member is `409
  member_unavailable`, and a succession silently skips them. (5) A member who holds a question's text can tell a held or
  suspended author's bank (generic hit, null ids) from a removed bank, a closed circle or a former membership (ids, no
  names); the refusal is the same. (6) An offer made before the owner's suspension stays acceptable for its 7 days (a
  suspended owner can still pass the circle on, also to an accomplice); there is no way for a member to block offers (a
  declined offer can be made again, now within the daily 10). (7) Demoting the member offered the circle to viewer
  does not drop the offer (an accept makes them owner); the owner can cancel. (8) An offer whose offerer's erasure is
  in flight can still be accepted; the erasure then leaves one owner and no offer row (correct outcome). (9) The daily
  net's `OWNERLESS_CLOSED_SQL` walks every unpurged closed circle by the purge index before its `owner_closed` filter;
  bounded by the closes of 14 days. The activity line of an accepted transfer has the offerer as its actor
  ("ownership_transferred" by the old owner to the new one).

**Round 34 (circles phase A after critics 41a and 41b, `docs/critic-reports/2026-10-24-circles-phaseA-*-critic41*`).**
Migration `0109_pool_round34.sql` (one new table, `circle_transfers`: a pending ownership offer per circle; nothing
else changes). Tests: `test/pool/round34.test.ts` (6, each failed on round 33's code), plus an erasure-race scenario
for the accept in round 32 (32-10).

* **An owner who closes a circle, then leaves or is erased, hands it on (critic41a 4).** Succession now also runs for
  an owner-closed circle inside its undo window; the circle stays closed and the successor may reopen it before the
  14 days end. The owner invariant the tests check covers such circles too.
* **Standing (critic41b F2).** The simplest safe rule, the public pool's: content and authority follow the account's
  standing (approved `teacher`/`admin`, not suspended, not erased). Succession picks only members in good standing
  (an open circle with no owner and nobody in good standing left closes, `last_member_gone`); an offer goes only to
  a member in good standing (`409 member_unavailable`); a code admits nobody while its issuer is not in good
  standing (it works again on restore); a bank whose author is not in good standing (suspended, or no longer
  approved) is not served: hidden from every member like a held bank (circle page, read 404, image 404, `heads`
  `under_review`, activity title, publish-check names) until the account is restored. An erased author's bank kept
  anonymised (D15) is served as before. A suspended OWNER stays the owner (a suspension is reversible): nothing is
  handed on; the circle keeps working for its members, no new member joins by the owner's codes, and the admin can
  close it. 
* **The publish check names only what the caller can see now (critic41b F1).** A hit names the bank and circle only
  for an active member of the open circle and a live, unheld bank whose author is in good standing; a held bank (or
  a suspended author's) gives a generic hit (`circle_id`, `circle_name`, `bank_id`, `bank_title` all null,
  `may_claim_independence: false`); otherwise (a former member, a removed bank, a closed circle) the ids, to name in an
  independence statement, without names. The position is refused all the same.
* **The owner's delete of a bank its author removed (critic41b F6)** makes it the owner's removal (`owner_removed`):
  the author can no longer undo it; the undo window is not extended.
* **Ownership is offered, never forced (critic41b F3).** `POST /circles/:cid/transfer` makes an offer (a new one
  replaces the pending one); the member accepts with `POST /circles/:cid/transfer/accept` within 7 days, while the
  offerer is still the active owner, the circle is open and the member is still in it. The taker's `circles_owned`
  cap is checked in the accept's step-down statement (two accepts at once cannot pass it; tested at every statement
  boundary). Either side drops it with `POST /circles/:cid/transfer/cancel`. An offer goes when it is accepted,
  declined, cancelled or replaced, when the offerer or the target leaves (or the target is removed), when either
  account is erased (and the daily resweep), when the circle is purged, and daily once expired. `GET /circles/:cid`
  shows `pending_transfer {to_user_id, to_name, expires_at}` to the owner and the member offered it (null to everyone
  else); `GET /circles` shows `ownership_offered_until` per circle to the member offered it. Audited
  `circle.transfer_offer`, `circle.transfer` (the accept, with the offerer as actor) and `circle.transfer_cancel`
  (round 35: offers and cancels keyed by the account, not the circle);
  members' activity shows the accepted transfer. The export lists open offers made or received (`transfer_offers`).
* **Wording (critic41a 2).** The `circles_owned` refusal says a closed circle still counts for 14 days.
* **Still documented, not fixed.** (critic41b F4) Membership rows of circles one left stay while the circle exists,
  and an account alone can reach the 100-row cap in about 20 days (create five a day, leave each), then cannot
  create or join until those circles are purged; there is no "forget this circle" (it would shrink the publish check
  for that account); an admin can raise `circle_memberships_total` per account. (F5) The owner's immediate purge of a
  removed bank deletes its first-sender records at once: an owner can remove and purge a member's bank, resend its
  text as their own, and become its "first sender" (the member's own public publish is then refused); before round 33
  this took 14 days; every step is audited (the purge is not in the members' activity list). (F7) A member in good
  standing hides any other member's bank at once with one student-data report (3 a day), and the site-wide 50 a day
  instant-hold budget and the 20 an hour privacy mail budget are shared with the public pool (about 17 cooperating
  approved accounts could use them up for a day). (F8) The account export's circle section has no LIMIT or
  reservation (one account's `versions_sent` grows up to 100 rows a day); purged banks leave tombstone rows forever;
  `GET /admin/pool/circle` lists every tombstone and membership row of a circle without a LIMIT; the activity page
  still reads non-activity audit rows between events. (critic41a) Succession ignores `circles_owned` (it cannot refuse):
  cooperating accounts can park more than five circles on one member (each leaver's 100 rows and the member's 20
  memberships bound it). A demotion or a removal lasts until the owner shares a code issued after it with that member.
  The owner purge's claim and its purge are two steps: if the purge fails, the bank can no longer be restored and the
  daily cron finishes it (the request answers 500). (critic41b smaller) A failed offer accept answers `circles_limit`
  with the member's own count (the round-33 transfer told the OWNER another member's count: gone with the offer);
  `heads` tells a member `removed` with `until` for a bank the circle page hides from them; the export gives former
  members the circle's current name; code-versus-removal order is by wall clock (`left_at >= created_at`), so two owner
  requests a few milliseconds apart may order the other way; a hold placed in the milliseconds between the purge's
  selection and its run is lost; `circle-release` dismisses the reports it releases (which starts the reporter's 30-day
  instant-hold throttle); the app must keep joining a button after the preview (a deep link that auto-joins would let
  anyone fill someone's membership caps); closes are counted through the actor's audit index; the publish check is at
  most 2 x 1,000 hashes x 100 circles = 200,000 index seeks per publish or preview.

**Round 33 (circles phase A after critics 40a and 40b, `docs/critic-reports/2026-10-24-circles-phaseA-*`).** No migration
change (one new limit, `circle_memberships_total`). Every fix has a test in
`test/pool/round33.test.ts` (10) that failed on round 32's code.

* **Re-joining (B1, both critics).** A member the owner REMOVED can come back only with a code issued after the
  removal (`code.created_at > left_at`); an older code is the same `422 circle_invite_invalid` as any bad code
  (preview too). A member who LEFT (or was demoted, then left) rejoins with a code issued before leaving at most
  in the role held when leaving (a viewer stays a viewer); a code issued after leaving gives its own role.
  Checked in the join's own UPDATE (`CODE_USABLE_SQL`, `JOIN_ROLE_SQL`, `circles.ts`).
* **Codes die with their issuer's owner role (B2).** A leave, a transfer and a removal revoke the account's open
  codes of that circle in the same batch (an erasure already did); and the join refuses any code whose issuer is
  not the circle's active owner now. The owner's list names each code's issuer (`created_by`, `created_by_name`).
* **circles_owned through a transfer (B3).** The target's cap is in the transfer's step-down statement:
  `422 circles_limit {metric: "circles_owned", used, limit}`; two transfers to one member at once cannot both pass
  (tested at every statement boundary). A succession (an owner leaving or erased) cannot refuse and ignores the cap.
* **A hold delays the purge (B4).** A bank with a hold, or a closed circle with a held bank, is not purged until
  the admin releases it (as an open report already did). An admin `circle-remove` deletes that bank's holds and
  `circle-close` those of the circle's banks: the admin's decision lets the 14 days run.
* **Undelete (B5).** The owner's removal is undone by the owner only; the author's own by the author or the owner.
* **The publish check is bounded (B6).** One result row per hash that has a hit (the earliest sender, chosen in
  SQL by a window), not one per (hash, membership); the circles considered are the publisher's memberships that
  hold records, active first then the most recently left, at most `circle_memberships_total` (100), which joins
  and creates now enforce on membership ROWS, former ones included (a former member re-joining reuses its row:
  never refused by it). Each statement's results are reserved first (`CIRCLE_CHECK_ROW_BYTES` = 512 a hash).
  Rows read on workerd, 1,000 distinct hashes, every circle holding records: 20 memberships 43,061; 500 and
  2,000 memberships 208,501 and 213,001 (round 32's form: 507,500 and 2,024,000 rows and as many result rows).
* **Send memory (B7).** A send reserves `max(15 x body, values x 96 x 2)`, the publish's formula (a dense 760 KB
  payload of 190,000 values now reserves 36.5 MB, was 12.2 MB).
* **Fixed variants.** V1: the activity list shows a bank's title only to who may see the bank (held: its author;
  removed: the owner and its author). V2: no version is pruned while its bank is held, nor one an open report
  names (the next send after the case prunes it). V3: `POST .../banks/:bid/purge` (owner) purges a removed bank at
  once, so an insider's flood stops counting toward the caps and records the same day (removed banks still count
  until purged: excluding them would let deleted bodies pile up in R2 for 14 days without any cap).
* **Known limits of circles (documented, not fixed).** (critic40a) [Fixed in round 34: an owner who closed a
  circle and then left or was erased left it ownerless.] A member's independence statement is refused while she can read the bank, but passes
  once she has left (audited and flagged, as for former members). A purge deletes the bank's first-sender
  records, so a later copier becomes the "first sender" of that text and the true author's public publish may be
  refused. An author who left cannot withdraw her bank (the owner can); an author demoted to viewer may still
  change its licence but not send. An open report or a hold keeps a bank and its closed circle without time
  limit (also a report whose reporter was erased): the admin queue must be worked. "Get a copy" is the view
  read: there is no list of who holds a copy to notify after a hold. Resolved circle reports and purged-circle
  tombstones are not retention-deleted. The legal pages do not describe circles yet (D9). An erased author's
  kept bank gets no new version until phase B. A purge backlog resumes the next day (each step idempotent). The
  activity list shows an erased member as "left". (critic40b) The activity query reads non-activity audit rows
  (codes, reports, admin opens) between events, bounded by what members write. Student-data blocks do not hold or
  purge circle banks automatically: they give the admin a basis to open, and the image route refuses blocked
  images. The admin's `basis` is checked for existence (any report ever filed on the bank, any recorded notice),
  not for relevance: accountability (every open audited), not access control. On erasure with no choice made,
  circle banks are kept anonymised (D15), the opposite of the public pool's default for items nobody imported:
  confirm with the owner. A member may publish another member's question from a CC-licensed circle bank with an
  independence statement (audited, flagged; the credited route is phase C). Circle student-data report mails
  share the public pool's hourly privacy budget (20). One account may own 5 circles of 200 MB each (D3: not
  charged to anyone). Every member sees members' account ids (the routes take them). Closes are counted from the
  audit by actor; a removed member's in-flight leave answers 404; a valid code's preview is not counted; `heads`
  counts no misses; a version's message is stored and exported but not shown in phase A.

**Round 32 (circles, phase A: `docs/circles-plan.md`, decisions D1-D19).** Migration `0108_pool_round32.sql`
(new tables and indexes only; no existing table, index, trigger or row changes; the next is `0109`). Code:
`src/pool/circles.ts` (circles, members, codes, reads, reports, erasure, cron stages, export),
`circlesend.ts` (sending a version), `circlecheck.ts` (the public publish check, D13), `circleadmin.ts`
(admin routes). Phase A: circles, codes, banks shared into a circle for members to view and copy, and the
bank AUTHOR's own republish as versions; editors' shared editing, history pages, merge and rollback are
phase B. No admin flag kind was added (no `admin_flags` rebuild): circle cases are open `circle_reports`
rows, cap crossings use `cap`, circle independence statements `independent_claim` / `independent_repeat`.

* **Rules every circle route keeps.** They sit in the pool's route table, so a live session, `teacher` /
  `admin` trust, the round-28 invite gate, the rate limits and (writes) the pool-key check apply. A circle
  or bank the caller is not an ACTIVE member of is a counted `404 not_found` (former members included;
  admins are not members). Inside a circle a missing role is `403 circle_role_required {needed, role}`.
  Every write is guarded in the same statement or batch by `ALIVE_SQL`. Ids in the patterns are 32 hex
  characters. `/circles` bodies skip the auth package's 8 KB pre-read (`preread.ts`, `src/index.ts`), like
  `/items` and `/blobs`.
* **Routes** (JSON unless said; errors in the usual envelope):

  | Route | Who | Body / answer |
  |---|---|---|
  | `POST /circles` | teacher | `{name (1-80), description? (<=1000)}` -> `201 {circle}`; caps: `circles_owned` 5, `circle_memberships` 20 (`422 circles_limit {metric, used, limit}`), `circle_creates` 5 a day |
  | `GET /circles` | teacher | `{circles: [{id, name, role, joined_at, closed_at, close_reason, closed_by, undo_until, updated_at, members, banks, ownership_offered_until}]}` (active memberships; closed ones until purged or left) |
  | `GET /circles/:cid` | member (a closed circle too: `200` with its closed state) | `{circle: {id, name, description ("" when none), role, joined_at, created_at, updated_at, closed_at, close_reason, closed_by, undo_until}, members: [{user_id, display_name, role, joined_at, is_me}], banks: [{id, title, subject, course, grade, license, author_name, is_author, head_version, question_count, has_images, created_at, updated_at, held?, removed?: {reason, undo_until}}]}`; removed banks only to the owner and author, held ones only to the author |
  | `PATCH /circles/:cid` | owner | `{name?, description?}` |
  | `DELETE /circles/:cid`, `POST .../reopen` | owner | close: `{ok, undo_until}` (14 days; members get `409 circle_closed {reason, undo_until}` at once); reopen inside it |
  | `GET /circles/:cid/activity?before=` | member | `{activity: [{at, what, by, member, role?, bank_id?, bank_title?, version?, license?}], next_before}`, 50 a page; `what` is `created, joined, left, removed, role_changed, ownership_transferred, ownership_passed_on, closed, reopened, renamed, bank_shared, version_sent, rolled_back, bank_removed, bank_restored, license_changed` (round 37: `rollback_of?`, `merged_from?`; round 45: `email_invited`, `hide_key_changed` with `hide_key`; the cursor is `<at>:<id>`, a bad one 400); never codes, reports or admin opens |
  | `POST /circles/:cid/invites` | owner | `{role: editor\|viewer, max_uses? 1-50 (1), expires_in_days? 1-30 (7), note? (<=200)}` -> `201 {invite_id, code, role, max_uses, expires_at, note, shown_once}`; 10 open codes a circle (`circle_open_invites`; round 47: plain codes only, email invites have `circle_open_email_invites` 40), 50 a day (`circle_invites`) |
  | `GET /circles/:cid/invites?after=` | owner | `{invites: [{invite_id, role, created_at, created_by, created_by_name, max_uses, uses, expires_at, revoked_at, note, state}], next_cursor}`; never the code, never who typed a wrong one |
  | `POST /circles/:cid/invites/revoke` | owner | `{invite_id}`; `409 already_revoked` |
  | `POST /circles/:cid/invites/email` | owner | `{email, role}` -> `202 {ok, invite_id, kind: "email", role, max_uses: 1, expires_at, emailed: true, address_label}` or `422 invite_not_sent {emailed: false, reason: address_limit\|not_deliverable}` (nothing made); `429 hourly_limit`; `409 circle_full`; the server mails a fixed text holding a single-use code (rounds 42, 46) |
  | `POST /circles/join/preview` | teacher | `{code}` -> `{circle: {id, name, owner_name, members}, role, already_member}`; uses nothing |
  | `POST /circles/join` | teacher | `{code}` -> `{ok, circle_id, role}` or `{ok, already: true, ...}`; every bad code `422 circle_invite_invalid`; `409 circle_full {limit}`; 10 FAILED codes a day (`circle_join_tries`, 429) |
  | `POST /circles/:cid/leave` | member | an owner leaving hands the circle on (below) |
  | `POST /circles/:cid/members/remove` | owner | `{user_id}` (not yourself: 400); immediate; re-joining needs a code issued AFTER the removal (round 33) |
  | `POST /circles/:cid/members/role` | owner | `{user_id, role: editor\|viewer}` -> `{ok, changed}` |
  | `POST /circles/:cid/transfer` | owner | `{user_id}` of an active member in good standing: an OFFER (round 34) -> `202 {ok, pending: {to_user_id, to_name, expires_at}}`; a new offer replaces the pending one, the same open offer again changes nothing; `409 member_unavailable`; `404 not_found` for a non-member; 10 offers a day (`429 daily_limit {metric: "circle_transfer_offers"}`, round 35) |
  | `POST /circles/:cid/transfer/accept` | the member offered it | within 7 days, while the offerer is still the active owner -> `200 {ok, role: "owner"}` (the old owner becomes an editor, their codes are revoked); `409 no_pending_transfer`; `422 circles_limit {metric: "circles_owned"}` |
  | `POST /circles/:cid/transfer/cancel` | owner, or the member offered it | withdraws / declines -> `200 {ok, cancelled}` |
  | `POST /circles/:cid/banks` | owner, editor | publish format (`application/vnd.vershion.publish`, meta below) -> `201 {circle_id, bank_id, version: 1, version_id, qids, added, changed, removed}` |
  | `PUT /circles/:cid/banks/:bid` | owner, editor (round 37; phase A: the bank's author) | the next version: meta `base_version`, `merge_token?`, `confirm_large_change?` -> `200 {..., held, merged_from?}`; `409 conflict {head_version, head_at, head_by}` (nothing stored); `428 confirm_large_required {removed, total}`; `422 license_author_only {license}`; `422 merge_token_invalid` |
  | `GET /circles/:cid/banks/:bid/versions?before=` | member who may read the bank | the history, 50 a page (round 37) |
  | `GET /circles/:cid/banks/:bid/versions/:n` | member who may read the bank | a kept version, streamed like the head; `404 version_not_retained {oldest_kept}` (round 37); a withheld one (or head) `404 version_withheld {message}`, with `hide_off_releases` for the owner and the bank's author while an editor (round 49) |
  | `POST /circles/:cid/banks/:bid/merge` | owner, editor | `{base_version}` -> head, `base_kept`, versions between, `merge_token` (30 min) (round 37) |
  | `POST /circles/:cid/banks/:bid/rollback` | owner; the bank's author while an editor (D17) | `{to_version, base_version, commit_id, message?, confirm_large_change?}` -> `200 {..., rollback_of, held, withheld}`; 20 a day (round 37); `404 version_withheld` (round 38) |
  | `POST /circles/:cid/banks/:bid/trim` | owner; the bank's author while an editor | `{keep: 1-19}` -> `{ok, pruned, oldest_kept, pinned_versions}` (round 38; round 39: a student-data hold does not stop it, a withheld version with an open flag is kept) |
  | `GET` / `HEAD /circles/:cid/banks/:bid` | member (held: author only) | streamed `{circle, bank: {id, title, subject, course, grade, license, version_license, author_name, is_author, version, version_id, question_count, has_images, committed_at, committed_by, held?}, origins, payload}`; `ETag`, `If-None-Match` -> 304; viewing and copying are this one read; counts toward `item_reads`; `409 bank_removed`, `409 circle_closed` |
  | `GET /circles/:cid/banks/:bid/blobs/:sha256` | member who may read the bank | an image a kept version uses (never a student-data-blocked one) |
  | `POST /circles/heads` | teacher | `{bank_ids}` (<= 200) -> `{banks: [{bank_id, status: current, version, version_id, license, at, by, hide_key (round 52)} \| {status: removed, until?} \| {status: under_review} \| {status: unavailable}]}`; one `item_reads` |
  | `PATCH /circles/:cid/banks/:bid` | the bank's author | `{license}` (D19: `circle-only` = members may not re-share); bounded by the head's declared public origins (`422 license_incompatible {allowed}`); earlier versions keep theirs. Round 43: or `{hide_key, labels?, order_acks?, ack_formatting?, image_ok?}` (the author while an editor or the owner, or the circle's owner): viewers read the stripped form; round 52: `base_version?` pins it (`409 conflict {head_version}` on another head, nothing changed) |
  | `DELETE .../banks/:bid`, `POST .../undelete` | owner, the bank's author (D17) | 14 days to undo, then purged; the owner's removal is undone by the owner only, the author's own by either (round 33) |
  | `POST .../banks/:bid/purge` | owner | a removed bank (by its author, the owner or an erasure) purged now, so it stops counting toward the circle's caps; never under an admin removal, a hold or an open report (`409 under_moderation`; `409 bank_not_removed`); not undoable (round 33); round 48: while only an administrator's review of a flagged version is pending, `200 {ok, purged_partial: true, pending_review: true, evidence_kept, message}` (the rest pruned, the purge claimed, finished once the flags are decided or their evidence captured) |
  | `POST .../banks/:bid/report` | member, not the author | `{reason: student_data\|copyright\|inappropriate\|other, note?}` -> `202` always |

* **Sending a version.** Meta: `commit_id` (22-64 base64url characters: the idempotency key; a retry with
  the same id and body replays the first answer with `replayed: true`, another body or bank is `409
  commit_id_conflict`), `license` (required, any of the six), `attest {own_work: true, terms_version}` on
  every send (`422 attestation_required`, D1), `message?` (<= 500), `meta {title, subject, course, grade,
  tags, standards}` (share-core's metadata check), `base_version` (PUT only). The payload must be a bank
  (1-1,000 questions: share-core). A question keeps its qid when it carries `published.qid` of the head
  (the index), otherwise it gets a new one; `added` / `changed` / `removed` are per qid (keyed content
  hashes). Images: verified, and the sender's (pending, in one of her publishes) or already in the bank's
  kept versions; the sender's pending hold is extended to the commit. A question whose DECLARED `origin`
  names a public item the sender can see binds the bank's licence to what that origin allows
  (`422 license_incompatible {positions, allowed}`; `circle-only` only from CC0); the checked origins are
  stored with the version and served as `origins` (credits), the body's own `origin` is dropped. The send
  holds the account's publish lock (`409 publish_in_progress` against a publish), the heavy slot and 16x its
  body. Order: staging row (ALIVE-guarded), R2 body and index under `circles/<cid>/<bid>/<vid>`, then one
  batch: the compare-and-set (the circle open, the role, the author, the head, and every circle cap in the
  same statement), then everything else conditioned on "this version is the head". A lost commit deletes its
  staging row and objects; an abandoned one is collected hourly. Caps (site-wide `'*'` values, admin
  adjustable): `circle_banks` 100 (removed banks count until purged), `circle_bytes` 200 MB (every kept
  version's body + index + its images, charged per version, never to a person: `413 quota_exceeded {scope:
  "circle", used, needed, limit}`), `circle_questions` 5,000 across current versions (`422
  circle_questions_limit`; not counted toward `shared_questions`), `circle_records` 50,000 key-free
  question records (`413 circle_records_full`), `circle_commits` 100 sends a day. D16: the last 20 versions
  of a bank are kept; older ones become `pruned` at the commit (their image rows go then, their R2 objects
  with the daily `circle_prune` stage), their history rows stay.
* **The public publish check (D13, `circlecheck.ts`).** On every `/items` publish and preview (public or
  unlisted: an unlisted item can be made public later without a new publish), after share-core: the
  DISTINCT key-free hashes (full and served form) are looked up in `circle_qkeys` for every circle the
  publisher is or was in, one covering-index seek per (hash, circle) for its earliest sender. A position
  passes when that sender is the publisher, when her own public publish of the text came first
  (`author_firsts`), when it is a short statement (`isShort`), or when her meta's `circle_independent_of
  {banks, accepted: true, statement (10+)}` names the bank and no circle-only bank she can still read holds
  the text first sent by someone else (audited in the publish's audit detail, flagged `independent_claim`;
  with public claims, the third in 30 days flags `independent_repeat`). Otherwise `422 circle_material
  {positions, hits: [{pos, circle_id, circle_name, bank_id, bank_title, may_claim_independence}] (first 100),
  message}`. A publisher who was never in a circle costs one indexed probe.
* **Ownership (decision 3, D14, D18).** An owner leaving, erased, or found missing by the resweep: in each
  OPEN circle with no active owner, the active editor with the smallest `join_seq` (pool_seq at joining)
  becomes owner, else the smallest-`join_seq` viewer; with no active member left the circle closes
  (`last_member_gone`, purged after 14 days, no undo). Each step is one statement whose condition is "no
  active owner"; a partial unique index allows one active owner. Audited as `circle.owner_succession`.
* **Reports and holds (D1).** A student-data report from an established member (approved 7+ days, in the
  circle 1+ day, no dismissed student-data report in 30 days, at most 3 such holds a day, under the
  site-wide `instant_holds_global` shared with the public pool) hides the bank from every member but its
  author at once; other reasons hide it at `report_threshold` (3) distinct members. The admin is mailed ids
  only (`circle_report`).
* **Erasure (plan 8.1/8.2).** `deleteContentStatements` adds 16 statements since round 34 (before `pool_prefs` goes): the
  account's active memberships step down (audited), succession and closes, banks it authored are kept
  anonymised ("deleted teacher") unless it chose `keep_on_delete: false` (removed, purged after 14 days),
  version messages and report notes go (reports lose their reporter), codes it created are revoked (notes
  go), its membership rows are deleted. Versions it sent keep their opaque committer id. The daily
  `erasure_resweep` repeats all of it for anything a request in flight wrote, and repairs up to 50 open
  circles without an owner (one indexed walk of the open circles a day).
* **Cron.** Hourly `circle_staged` (sends that never committed). Daily `circle_prune` (pruned versions'
  R2 objects, 500 a run) and `circle_purges` (removed banks and closed circles past `purge_after`, never one
  with an open report: R2 bodies and indexes, versions, image rows, records, holds, codes, members; bank and
  circle rows stay as tombstones without typed text). The blob GC never deletes an image a kept circle
  version lists (`circle_version_blobs`). `retention` keeps a circle's audit trail while one of its banks is
  held or a report about it is open.
* **Admin (D1, plan 6.4).** `GET /admin/pool/circle-reports?after=` (the queue), `GET /admin/pool/circles?after=&user_id=`,
  `GET /admin/pool/circle?id=` (metadata, members past and present, banks with holds and reports, codes'
  states: audited `circle.admin_open`), `GET /admin/pool/circle-bank?id=&version=&basis=report|notice|privacy[&notice_id=]`
  (a kept version's body, streamed; only on a basis: a report on the bank, a recorded DMCA notice, or a
  student-data block matching one of its questions or images, else `403 basis_required`; audited
  `circle.admin_view` in both audits), and writes with a reason of 10+ characters: `circle-hold {bank_id,
  note}`, `circle-release` (all holds go, open reports dismissed), `circle-remove` (purged after 14 days),
  `circle-close {circle_id, note}` (no owner undo).
* **Export.** `GET /me/export` gains `circles {memberships, owned, invites_created (no codes),
  versions_sent (no bodies), banks_authored, reports}`.
* **Choices made in this round where the plan left one (UNCONFIRMED items).** Circle codes are `C-` and 24
  Crockford characters (120 bits), SHA-256 of the normalised form with a `circle:` prefix; "10 failed join
  codes" counts only failures (a try is charged first and given back for a good code); a member typing a
  spent code of her own circle is told `already`. An owner-closed circle counts toward the owner's
  `circles_owned` and every member's `circle_memberships` until purged or left (caps never fail open); a
  reopen does not re-check members' caps. A removed bank counts toward the circle's caps until purged (so an
  undelete needs no cap check). Per-circle caps use the site-wide value only. "Established member" (D1)
  = approved 7+ days and in the circle 1+ day. D11: the fifth owner close in 30 days raises a `cap` flag
  (`circle_closes`). An open circle report (any reason) delays the circle's and bank's purge. The
  independence statement never covers a text a circle-only bank she can still READ holds (the plan's
  UNCONFIRMED "exact copy" rule taken as written: the check is exact on the key-free hash). Circle names
  and descriptions stay when their creator is erased (they are the circle's). The wire field is `license`
  (as everywhere else), not `licence`. ETag of a bank read is a hash of the head (it changes with the
  version and with the licence or author name), not the bare version id. The privacy byte purge deletes a
  confirmed student-data image even when a circle uses it (members are "others"); the circle's blob route
  refuses such images. Version pruning (D16) is done at the author's republish already (phase A sends can
  otherwise fill the circle's quota with old versions).
* **Rows read** (workerd SQLite, miniflare D1 `meta.rows_read`, statements captured from the node harness and
  replayed on a copy of the database; plan 9.1's estimates in brackets): my circles list 214 -> 3,099 for 5
  -> 20 circles of 13 -> 50 members and 25 -> 100 banks [about 3,000]; the circle page 76 + 40 -> 301 + 151
  [about 250]; `heads` of 200 banks 1,200; a bank read 6 rows whatever its size [3 lookups]; a send's quota sum
  1,981 with 99 banks x 20 kept versions [about 2,000] and record probe 2 per distinct question (500 -> 1,980);
  the public check with 20 circles and one circle's 100 banks all holding every hash: 10,540 -> 41,620 rows
  for 250 -> 990 distinct questions (2 per seek; a join on the primary key would read 29,750 -> 117,810), 82
  for a bank repeating one question 990 times, its "readable bank" probe at most 18,120 (bounded to claimed
  and reported positions); the erasure batch of an account owning 20 full circles 2,695 rows in all.
* Tests: `test/pool/round32.test.ts` (17 `test()` calls, 25 tests: the 32-10 loop registers 9): create, caps and 404s on every circle route (32-1); codes (32-2);
  members, roles, transfer, succession (32-3); banks, reads, ETag, republish CAS, replay, `commit_id`
  conflicts, activity, nothing in browse or `/items/versions` (32-4); images, the GC, pruning (32-5); every
  cap in the commit (32-6); close, undo, purge, reports, holds, licence, origins, admin basis and audit,
  export (32-7); the publish check (32-8); erasure and succession, reruns, resweep (32-9); round-27 style
  erasure races before every statement boundary of nine requests, checked BEFORE the cron (the guard in the
  write) and after (32-10), and the last member erased while a code is used (32-10b); a random-sequence
  oracle against a model, 450 steps (32-11); plans and time at the caps (32-12a-c); 0108 on a populated
  database (32-13); pre-read, blocked images (32-14); admin list and close, the D11 flag, retention (32-15).
  Three earlier tests changed: round 11's erasure statement bound (< 60: the circle steps add a fixed 15),
  and rounds 16 and 20's migration lists now apply 0107 and 0108 (a publish needs the circle tables).

**Round 31 (shared near hashes: the earlier-publication lookup and the licence re-key).** No migration
(the existing near-hash and primary-key indexes suffice); the next is still `0108`. Staging's D1 insights
after round 30: the earlier-publication lookup (`publish.ts`, "duplicate / near-copy") averaged 26.5 s and
21.0 million rows a run (27 runs), with 500 `D1_ERROR`s on concurrent big publishes, when two teachers
published the same 990-question bank. Reproduced on workerd's SQLite (miniflare D1) with two near-identical
990-question banks by different accounts (plus two other banks, 3,960 questions in all): its plan was
always the index plan (MULTI-INDEX OR over the five near-hash indexes, with and without `ANALYZE` /
`PRAGMA optimize` statistics, node and workerd alike), so the cost was not a lost index but FAN-OUT. Every
position followed each of its near hashes to every question holding it; a hash many questions share
made it positions x questions. Rows read at 990 (old: the round-30 form; the pre-round-30 form read as
much or more, 15.7 million already with each question ten times):

  | Two near-identical banks of 990 | Round 30 | Round 31 (firsts + lookup) |
  |---|---|---|
  | every question different | 30,678 (15 ms) | 89,046 + 11,871 (24 ms) |
  | 990 copies of one question | 22,549,230 (6.9 s) | 56,463 + 10,890 (19 ms) |
  | one stem and choices, 990 explanations | 26,465,670 (9.3 s) | 71,301 + 23,747 (29 ms) |

  (If staging's insights entry still shows `iq.publish_id IN (SELECT x.publish_id ...`, it is the
  pre-round-30 statement, 9.9-41 million rows for a copy of a public bank; `iq.rowid IN (SELECT x.rowid
  ...` is round 30's.)
* **Two statements per chunk of 1,000 positions.** `EARLIER_FIRSTS_SQL` takes the chunk's DISTINCT near
  hashes and, for each, the questions it finds in the lookup's own sense (one column at a time, each by its
  index, forced with `INDEXED BY`; `near_full` and `near_bare_full` only on a shown publish), on an item
  visible to others by another author (`VISIBLE_TO_OTHERS`, the same joins), and returns, per hash, the
  FIRST in the lookup's order (`seq, publish_id, lineage_seq, rowid`), and the first among those carrying
  each explanation key and each answer key a position of the chunk has. `earlierCandidates` gives each
  position those few rowids (`$.c`), and `EARLIER_SQL` runs the old test, unchanged, on them only.
* **Why the answer is the same.** A source counts when its `seq` is below her first, or, when it lacks what
  she added (explanation or answer key), below her looser first; her looser first is never later than her
  first (`minFirst`), NULL counts as "no bound". So: the first question of all her hashes is the answer if it
  counts; if it does not, no later question lacking her additions can count either (none has a smaller
  seq), and the answer is the first one carrying her added explanation or answer key, if it counts. Both
  are among the candidates; the position-independent conditions (the hash test, visibility, another
  author) are the same in both statements, so "first" means the same thing. Ties keep round 30's order.
* The plain case reads about three times round 30's rows (each hash's questions are read once with their
  item, publish and author, where round 30 read only the matching rowids), still linear.
* **`licence_rekey` (hourly).** Its batch was selected by testing EVERY `q:` row of `content_licences`
  and `item_offer` against every question of its item, every hour, though at rest none is mappable. It now
  finds the rows from the questions' side: each retained (and ghost) question's served content is looked up
  as the row's key (`'q:'` or `'Q:'` + content, which is what `LIKE 'q:%'` and `substr(ckey, 3)` matched)
  on the primary keys, which lead with `(item_id, ckey)`. Same rows, same batch order, same mapping
  (the per-row `map` lookup is unchanged). Rows read per run with one republished 990-question bank and a
  hidden one (workerd): at rest 4,914,361 + 4,914,361 + 2,950,202 + 2,958,121 (6.4 s) -> 11,885 + 11,885 +
  11,885 + 19,804 (27 ms). While old-style rows remain (5,940 licence rows, 1,980 offer rows): 5.6 + 5.4 +
  6.0 + 5.6 million ->
  0.53 + 0.37 + 3.0 + 2.0 million: the unchanged per-row `map` walks its item's questions for each of the
  (at most `LICENCE_REKEY_ROWS`) rows a run takes, only until the catch-up is done.
* **Swept, NOT changed (reported).** With banks that hold one question many times, two more statements
  read rows growing with the square of the bank (workerd, 250 -> 990): the evidence query
  (`evidenceChunk`, `provenance.ts`), 2,327,358 -> 36,299,724 rows (15.1 s over its 32 chunks) when a
  teacher imports such a bank (`mayRepublishAs`) and again when she publishes her copy, 3.9 million on the
  author's own republish; and the served-units query (`publish.ts`, `servedOuter`), 64,260 -> 987,040
  (75 ms). Every position reads every served row with its match hash or unit key. Nothing else on the
  publish path grows past 4.5x for 4x the bank but two probes under 8,000 rows; the band-capped fuzzy
  lookups remain the heaviest linear ones (about 2.1 million rows).
* Tests: `test/pool/round31.test.ts` (6); the old statements are in `test/pool/oracles.ts` (round 30's
  test now takes the old lookup from there). (a1) every lookup of random pools (few templates, banks
  holding a question twice, hidden and shown, republishes, imports, erased authors with anonymised kept
  items, holds) answers as the round-30 and round-29 forms, compared when it runs (83 lookups, 320
  positions with her own first, 79 with a looser first); (a2) a directed case where the answer is a source
  behind the first one (it carries her added explanation); (a3) the licence re-key against the old
  statements over four runs (live and ghost content, hidden and shown, `Q:` keys, unmappable and canonical
  rows, past the 1,000-row batch); (a4) 80 adversarial rounds of random near hashes across all five
  columns, keys, seq and lineage ties, own, held and anonymised rows, and every first / looser-first /
  added-key combination; (b) two teachers' near-identical banks at 250 and 990 (every explanation
  different, and all one question): rows visited 1,998 -> 7,918 and 1,750 -> 6,930 (round 30's 250,499 ->
  3,922,379 and 374,252 -> 5,877,632), each near-hash column by its index, each candidate by rowid,
  answers as round 30's, time about 4x; (b2) the re-key's plan reads no `q:` row it does not look up by
  key (the old statements scanned them). Mutations of the new code (no explanation-key candidates, no
  author or visibility filter, no shown-only rule, another tie order, no `Q:` keys, no ghosts) each fail
  a test.

**Round 30 (publish-time rows read: linear in the bank).** Migration `0107_pool_round30.sql` (one index;
next is `0108`). Staging's D1 insights showed max-size publishes (about 990 questions) taking 20-60 s, and
two at once failing with `500 D1_ERROR`: two statements read rows growing with the SQUARE of the bank.
Every publish-time statement was then checked the same way, on workerd's own SQLite (miniflare D1,
`meta.rows_read`) with the staging repro's banks, 990 x 50-pair Matching and 1,000 x 26-choice Mc, at 250
and 990 questions, published, republished, copied by a teacher who never read it, and backfilled. Five
statements had the shape; behaviour is unchanged (the old statements are the oracle in `round30.test.ts`).
Rows read at 250 -> 990 questions, old vs new (workerd SQLite, the same database for both):

  | Statement | Old | New |
  |---|---|---|
  | `author_stems` INSERT at publish (`publish.ts`) | 506,251 -> 7,865,551 (2.1 s) | 4,501 -> 17,821 (74 ms) |
  | `author_stems` INSERT in `stemBackfillAuthor` | 502,252 -> 7,849,712 | 2,252 -> 8,912 |
  | `stemBackfillAuthor` / `stemBackfill` candidate SELECT | 68,429 -> 1,158,989 | 1,007 -> 3,966 |
  | `ownFuzzyFirst` durable stem lookup (a republish) | 452,262 -> 5,699,585 (9.8 s) | 31,289 -> 40,800 (47 ms) |
  | earlier-publication lookup (`publish.ts`, a copy) | 638,416 -> 9,852,396 (4.5 s) | 12,948 -> 50,439 (22 ms) |

* **The stem record INSERTs** joined every question and band with EVERY entry of the bound array (the
  stem record `?4`, or the backfill's explanation keys `?3`) and matched the position afterwards
  (990 x 8 bands x 990). They now read the question's own entry by position (`?4 ->> '$[pos].e'`; the
  record's entry i is `{p: i, e}`, and the `p` check is kept). The loop order (question, then band) and
  the GROUP BY are unchanged, so rows reach the upsert in the same order. The path lookup walks the
  array's entries (JSONB has no index), a few milliseconds at the 1,000-question limit; not rows read.
* **The backfill's candidate SELECT** probed `author_stems` by `(author_id, qhash)`, which the primary key
  `(author_id, band, qhash, ...)` cannot seek: every probe walked all of the author's records. Index
  `author_stems_qhash (author_id, qhash, bk_expl_full)`, covering: without `bk_expl_full` SQLite keeps the
  primary key's `author_id` prefix rather than look rows up in a WITHOUT ROWID table. Same SQL.
* **`ownFuzzyFirst`'s durable lookup** joins each position's keyed bands with her `author_stems` and keeps
  the first `FUZZY_PAIRS_MAX` (2,000) rows in (position, content) order. A bank of like stems ("Match each
  term...") republished joined every position with every record of the common bands. `ownStemsWindow`
  (`provenance.ts`) first counts each band's records (capped at 2,000, one query): a position has at least
  as many rows as its busiest band has records, so once those counts add up to 2,000 no later position can
  reach the result, and only the positions up to there are joined. Same SQL (`OWN_STEMS_SQL`), same rows
  in the same order; below the cut every position is joined as before.
* **The earlier-publication lookup** (a near copy of a VISIBLE question by another author, published
  before hers) took as candidates every question of every publish holding a match (`iq.publish_id IN`),
  so a copy of a 990-question public bank walked each matching publish per question (41.2 million rows,
  17.6 s, with four publishes holding the bank; the table's 9.9 million is with two). Its candidates are
  now the matching questions themselves (`iq.rowid IN`, by the near-hash indexes): a question passing its
  test matches by one of those hashes, so the set is the same. Ties on `seq` (a bank holding a question twice) were broken by the old scan
  order, publish then `item_questions_publish_lineage`; the ORDER BY now spells that out (`seq, publish_id,
  lineage_seq, rowid`), so the same question is named (checked on node and on workerd: identical plans
  for the old form, identical answers).
* Checked and left as they are (every statement of the session above, as it ran, on workerd's SQLite): the
  evidence query (`evidenceChunk`, linear: 26,898 -> 107,094 rows for a copier who imported); `fuzzyQuery`
  and `publicNearEarlier`, bounded per band by `FUZZY_BAND_MAX` and `PUB_BAND_MAX` (a bank of like stems:
  at most about 2.2 million rows at either size, a band past its cap skipped; under 0.15 s), now the heaviest
  publish-time statements; nothing else grows past 4.5x for 4x the bank but the privacy band probe (2,440
  rows at 990: one seek per distinct band). Rows read per phase at 990 after this round: first publish
  205,095; republish 4.0 million; a teacher's copy 2.1 million (43.2 million before).
* **Not publish-time, NOT changed (reported):** the hourly `licence_rekey` (`licenceRekey`, round 25)
  tests every `q:%` row of `content_licences` and `item_offer` for mappability by walking all the questions
  of its item, every run, whether or not the row is already canonical: four statements of about 5.9
  million rows each (2.3-2.7 s each) per hour with one 990-question bank in the pool, growing with the
  square of the bank.
* Tests: `test/pool/round30.test.ts` (5). (a1) the old and new INSERTs give the same `author_stems` on
  random banks (every kind, duplicates, a reordered and edited republish, aged records, the legacy
  backfill at the hourly cron); (a2) adversarial item rows and arguments (duplicate contents and bands,
  NULL and '' keys, short and long arrays, an item that is not current, an erased author); (a3) her durable
  lookup over the kept positions against the full join, hot, sparse and tight band regimes, with the cut
  reached and not; (a4) the earlier-publication lookup against the publish-driven form, ties included;
  (b) the 990 x 50 Matching and 1,000 x 26 Mc session at 250 and 990: no statement walks a whole bound
  array, an author's whole stem record, or (over the bank's positions) a whole publish per row
  (EXPLAIN QUERY PLAN of every statement; the old forms are caught), the index changes no answer, her
  durable lookup visits 16,394 -> 26,396 rows (the full join 431,620 -> 5,662,944), and each rewritten
  statement's time grows about 1.3-7x for 4x the bank (best of 5; 12x allowed, 8x with
  `POOL_STRICT_TIMING=1`; a statement under 25 ms at 990 passes).

**Round 29 (publish memory: the fuzzy lookup inside the publish's own reservation).** No migration, no
limit changed (`ISOLATE_BYTES_BUDGET` stays 72 MB, `PUBLISH_COST_PER_BYTE` 16, the lookups' worst cases
as before).
* **The bug (measured on deployed staging).** A hidden-key Matching bank of 3.81 MB (990 questions x 50
  pairs) was refused `503 busy` on an IDLE isolate after about 8 s of CPU. The publish holds its body
  (1x, readBody) and 15x for the heavy phase (16x: 61 MB for this bank) until it ends; its evidence query
  (8 MB) and its fuzzy lookups (`FUZZY_PAIRS_MAX` 2,000 rows x `FUZZY_ROW_BYTES` 8 KB = 15.6 MB, reserved
  only when a stem has 4+ words, so it has MinHash bands) were reserved ON TOP: 61 + 15.6 = 76.6 MB > 72 MB.
* **Why not simply run the lookups inside the 16x.** Measured (node, `--expose-gc`, live heap plus
  buffers at the lookups): both payload trees (the full and the stripped form) were still alive there,
  so 47 MB of the 61 MB was in use (with the test harness's 3.8 MB copy of the request), 13.5 MB to
  spare against the lookup's 15.6 MB worst case. Running the lookup inside the reservation as it was would
  have under-reserved. Nor may the reservation simply shrink after the heavy phase: staging builds the
  large insert statements again (measured in node: live memory rose by 15 MB at the question inserts of
  a served-body-keeping republish of this bank), so the 16x is kept until the end.
* **The fix: serialise, then lend.** (1) The payload trees, the full form's question list and the sketch
  memo (a pure cache) are dropped BEFORE the evidence and fuzzy lookups (the few facts staging needs are
  taken first: kinds, images, tags, bank name; `src/pool/publish.ts`). The previous stripped body of a
  hidden republish that keeps its served form is now read in its own function (`reuseServed`), so its
  text and parsed tree are gone once it returns, and the share-core adapter no longer keeps a decoded copy
  of the stripped JSON (`core.ts`: the spread of `validated` called its `json` getter). (2) The four
  lookups (evidence, own fuzzy, fuzzy, public near) take a `reserve` function: in a publish it is
  `lender(heavy - postHeavyBytes(...))` (`src/pool/shared.ts`), which borrows first from the part of the
  heavy reservation the publish provably no longer uses and reserves only the rest from the isolate (503
  `busy` as before). A borrow never frees isolate budget to anyone else (the heavy reservation stays
  whole) and is returned when its lookup ends; a refused reservation borrows nothing.
* **`postHeavyBytes`, the upper bound of what a publish holds during its lookups:** the bodies it will
  store (full + stripped + a reused stripped body, exact bytes; the request body keeps its own
  reservation) + 16x the meta's bytes + 12 KB a question + 224 bytes a stem/choice/part-stem word
  (`stemTokens`, `bodyTokens`; at most 64 a text) + 8 bytes a character of those words (two-byte strings
  included) + 2 KB a Multipart part. Fitted on node measurements (no pointer compression, so above
  workerd) of eleven publishes, fresh and served-body-keeping republishes: Matching 990 x 50, Mc 1,000 x
  26 with every word distinct, 440 x 64 long words, 1,000 tiny Tf, 1,000 Short, 180 x 50 Multipart parts.
  Every measured holding is at least 20% below the bound (the closest: 1,000 tiny Tf republished, 10.9
  MB live against 14.2 MB; the Matching republish 38 against 46.5 MB). When the bound is above the heavy
  reservation (many small questions, many distinct words), nothing is lent and the lookups reserve from
  the isolate exactly as before.
* **The repro bank now** (3,996,278 bytes, 74,250 words): heavy 57.2 MB, bound 38.8 MB, so 18.4 MB to
  lend, which covers the fuzzy worst case (15.6 MB) and an evidence query (8 MB) in turn. Peak reserved
  61.0 MB (the request body + the heavy 15x) instead of 76.6; measured live at its lookups 26-28 MB
  (node, with the request body). A served-body-keeping republish of it peaks at 62.8 MB reserved.
* **True peak against the 72 MB.** The budget's arithmetic is unchanged: 128 MB - 30 MB wasm - 20 MB
  baseline leaves 78 MB, 72 committed. Every byte a publish uses during its lookups is still covered by
  a reservation (its own, measured at least 20% above in node) and the lookups' worst cases are still
  reserved in full (borrowed or new); the borrowed part is memory the dropped trees actually freed, not
  headroom. What is still an estimate, as before round 29: the heavy phase's 16x (measured 62 MB on a
  3.9 MB Mc bank) and node-only numbers (workerd reports no isolate memory). Measured in node, a publish
  of 1,000 Mc questions whose every word is distinct holds more after its heavy phase (38 MB at 2.96 MB of
  body) than 16x its body, before and after this round; its bound is above its reservation, so nothing
  is lent to it. A 180-question bank of 50 Multipart parts each, every part stem 63 distinct words (3.8
  MB), is still refused `503 busy` on an idle isolate, as before (its bound lends nothing).
* Tests: `test/pool/round29.test.ts` (7): the 3.81 MB Matching bank is accepted on an idle isolate with
  its fuzzy lookups run and nothing reserved past 16x its body (it failed with 503 with lending disabled);
  two such publishes at once: one 201, the other shed `503 busy` by the byte budget (not the heavy queue),
  the budget never passed, zero after; with the rest of the budget taken by another request mid-publish it
  still completes on its own reservation; a publish with nothing to lend (1,000 small questions) is still
  refused 503 when the isolate is full, and accepted when idle; a refusal after the lookups (too many
  tags at staging) releases everything; the lender's arithmetic; the adapter keeps no decoded copy.

**Round 28 (owner decisions).** Three changes; migration `0106_pool_round28.sql` (next is `0107`).
* **Retention: audit and takedown records are kept 3 years** (`retention`, a daily cron stage,
  src/pool/cron.ts). 3 years is `RETENTION_MS` = 1,096 days (never less than three calendar years).
  Bounded and idempotent, oldest first: at most `RETENTION_ACTIONS` (100) takedown actions and
  `RETENTION_ROWS` (5,000) rows per table a run; the rest resume the next day. In order:
  * A takedown action goes, with its counter-notice, strike, exempt authors, hold releases, re-upload
    block rows and upstream snapshot (`dmca_upstream_keys`), when it is CLOSED (put back, withdrawn,
    reversed, or upheld and purged; no uphold or put-back still finishing), NOTHING about it is newer
    than the cutoff (its own dates, counter-notice, strike, exemptions, hold releases, the `dmca.*` audit
    rows of its item), and nothing still DEPENDS on it: no re-upload block of it in force (a 10-year
    `block_days` block keeps its action and upstream snapshot, so the original's exemption holds for as
    long as the block does), no copy it holds, no open appeal and no unresolved `counter_notice` or
    `dmca_copy` flag on its item (resolve the flag to let retention proceed). Each delete re-checks this
    inside one batch, so an action an admin reverses meanwhile is left whole.
  * A DMCA notice received before the cutoff goes once no action names it, unless it is still `open`
    (it can still be acted on; withdraw it to let it go).
  * A `pool_audit` row older than the cutoff goes, except the trail of an item that still has a
    takedown record kept, a hold (under review) or an open appeal, and rows naming a kept action or
    notice. The `pool_audit_no_delete` trigger now allows a DELETE only of a row older than 1,096 days
    by the DATABASE's clock (`strftime('%s','now')`), and the stage's DELETE carries the same floor:
    a skewed application clock can neither delete a younger row nor fail the stage. Updates stay
    refused as before.
  * Never touched: `void_items` (an upheld takedown's evidence void is live licensing state, not a
    record), admin flags, the outbox, and `auth_audit` (the auth package's table; NOT purged at 3 years:
    that needs a change in the auth package, not made here). The erasure resweep is unchanged and runs
    as before (it scrubs a kept record's counter-notice of an erased author; retention later deletes it).
  * Documented limits: the independence-claim "already claimed" check reads the account's audit rows
    (src/pool/publish.ts), so a claim re-made after its first is past retention counts as fresh for the
    30-day `independent_repeat` count; the audit stage re-reads kept old rows (held items, kept records)
    each day, bounded by the `at` index range.
* **Invite codes: pool use needs a redeemed admin-issued code** (src/pool/invites.ts). Integration
  point, chosen as the least invasive: the gate sits in the POOL, beside the approval gate, not in
  sign-up. The auth package's sign-up answers the same whether or not an address exists and keeps
  nothing a registrant typed past confirmation (the mailbox owner re-enters the profile), so a code
  checked or used up at sign-up could be burned by whoever typed someone else's address, and a refusal
  for a bad code would be a new differing answer; the auth package is generic and stays unchanged. So
  sign-up, confirmation and approval work exactly as before, and an account needs BOTH the admin's
  approval (trust `teacher`) and an invite redemption. Every pool route answers `403 invite_required`
  (with a message) to a teacher without one; admins are exempt; `GET /me/export` stays open (the
  account's own data); `POST /me/invite {code}` is open to `pending` accounts too, so an applicant
  redeems right after confirming and the admin can check before approving (`GET /admin/pool/invites?user_id=`;
  the auth package's queue does not show it). Approving an account without a code is still possible; it
  then cannot use the pool until it redeems one.
  * Codes: 120 random bits as 24 Crockford base32 characters, shown as `XXXX-XXXX-XXXX-XXXX-XXXX-XXXX`
    ONCE in the issue answer; only the SHA-256 of the normalised form is stored (`pool_invites.code_hash`;
    neither audit row has the code). Typed codes ignore case, spaces and hyphens and read I/L as 1 and
    O as 0. Single use by default, `max_uses` 1 to 1,000, optional `expires_in_days` 1 to 365,
    revocable; a revocation stops further redemptions and never undoes one (use revoke-teacher or
    suspend for an account). One redemption per account (`pool_invite_redemptions`); a second answers
    `{ok, already: true}` and uses nothing. Every refusal of a code (unknown, expired, revoked, used up)
    is the same `422 invite_invalid`; a malformed code is `400 invalid_request`. Attempts are a daily
    allowance (`invite_tries`, 10). The use is taken in one batch with the redemption and its audit,
    only while the code is good, the account holds no redemption and stands (`ALIVE_SQL`), so a race for
    a last use has one winner and an erased account writes nothing (401).
  * Existing approved accounts keep working: migration 0106 gives every account that is `teacher` or
    `admin` (not deleted) when it runs a redemption with `invite_id` NULL ("grandfathered"). Applying 0106
    is when the gate starts: accounts pending then, and accounts approved later, need a code. An admin
    later demoted to teacher keeps a grandfathered row only if they had one.
  * Erasure deletes the account's redemption (the invite keeps its use count); the erasure resweep does
    too. A request still authenticated after the erasure is a 401 at the gate, never a 403.
  * Admin routes (audited in `pool_audit` and `auth_audit`, never with the code): `POST /admin/pool/invites
    {max_uses?, expires_in_days?, note?}` -> `201 {invite_id, code, max_uses, expires_at, note,
    shown_once}`; `GET /admin/pool/invites` (newest first, `?after=` cursor, each with `state`
    `active | expired | used_up | revoked` and `uses`), `?id=` (one invite and who redeemed it), `?user_id=`
    (that account's redemption, `grandfathered` when it predates the gate, or null); `POST
    /admin/pool/invites/revoke {invite_id, note?}` (409 `already_revoked`, 404).
* **Counter-notice window in `GET /items/mine`** (spec appendix 14). The counter-notice route already
  refused after the window (`409 window_closed`, `author_notified_at > now - 30 days` in the same
  statement that takes the notice); `/items/mine` offered `counter_notice: "available"` for any open
  action. It now uses the route's own rule (`counterWindowOpen`, src/pool/moderation.ts): a taken-down
  item says `available` only inside the window, with `counter_notice_by` (the window's end, exclusive),
  and `closed` from that instant on, whether or not the daily cron has upheld it yet; a ghost takedown
  (`takedown_pending`) also carries `counter_notice` (`available | received | closed`) besides
  `counter_notice_by` and `counter_notice_sent`.
* Tests: `test/pool/round28.test.ts` (12). Retention: closed records of every kind (withdrawn, upheld,
  put back after a counter-notice) and their audit trail go after 3 years while a block in force, an
  open counter-notice flag, a hold, an open notice and everything younger stay; the upstream original's
  republish stays exempt while its block is in force (and a stranger's re-upload held); the bounds and
  the order; the trigger by the database's clock and a skewed application clock; retention together
  with the erasure resweep. Invites: the gate, hashed storage (every table scanned for the code), loose
  typing, audit, single use / cap / expiry / revocation, the race for a last use, the daily allowance,
  pending accounts, the 0106 grandfathering, erasure and the round-27 401. Window: both boundaries for a
  live and a ghost takedown, with no cron run between. `test/pool/harness.ts`'s `user()` gives each
  account a grandfathered redemption (`invited: false` for a new one); the migration tests apply 0106;
  the LIMIT lint covers invites.ts and the retention batches.

**Round 26 (critic moodle-33c).** Three blockers in the round-25 work, and one starvation fix.
* The upstream exemption covers only what the upstream CARRIED when the action was made (B-1). The
  round-25 exemption was item-wide and had no time bound, so the item a taken-down copy came from
  could ADD the taken-down material after the block: it was never held, and its readers were served
  the blocked question (80/80 differential runs). Migration 0105 adds
  `dmca_upstream_keys` and `dmca_actions.upstream_snap`; the action's batch records, per derivation
  item and per SELECTED block key, whether that item carried it then (a retained publish or a ghost).
  `upstreamCarried` (src/pool/served.ts) replaces the plain `upstreamOf` test in the publish re-upload
  hold, the held-publish charge, `recheckBlocks` and hold-copies. An ordinary republish of unchanged
  material stays exempt; material added after the action is held like any re-upload. An action made
  before round 26 (`upstream_snap = 0`) falls back to "in a publish committed by the action's time".
  Documented limit: the no-edit form, where a sock's copy of an accomplice's infringing item shields
  the accomplice's item from hold-copies, remains (the admin needs a second notice).
* Her earlier version predates a source that lacks what she added (B-2). `ownFuzzyFirst` discarded her
  own earlier version when her new one added an explanation or answer, so after she fixed a typo AND
  wrote the explanation, a copier (or an independent author) who published the typo fix first was
  credited as the earlier publication (public "Another author published these questions earlier"),
  and her readers were refused under the copier's licence. It now also returns the earliest own
  version with the same body ignoring that addition; a candidate source that itself lacks what she
  added (`lacksAdded`) is judged against it. Used only against such a source, never for binding.
  The test is PER FIELD (critic 34c): only a field her later version ADDED to her earlier one must
  be absent from the source, so an unchanged Short sample or Blank accepted answer no longer keeps
  the copier's typo fix as "earlier" (the first form of the fix required the source to differ in
  both fields and still credited the copier for those kinds).
* No cron re-creates an erased account's records (B-3). `stemBackfill` and `stemBackfillAuthor` skip
  authors whose `users.deleted_at` is set: the round-25 backfill re-INSERTed the `author_stems` rows
  that erasure deletes (8 rows back within the first hourly run, for kept and purged items alike).
  The guard is repeated in the INSERT's own SELECT (critic 34d): a hidden body is read from R2
  between the candidate SELECT and the write, and an erasure committing in that window would
  otherwise be followed by the INSERT (the daily resweep removed the rows again, up to ~24 h later).
  What is true now: the two backfill paths and the publish routes carry the guard in the write; the
  other cron jobs write no row keyed to an account; and the daily `erasure_resweep` is the backstop for
  every table a user request can write (next bullet), so no path depends on "each skips erased ones".
* A user request already authenticated when its account is erased (critic 35d B-1, round 27). Erasure
  is two batches (the auth tombstone, then `deleteContentStatements`), and a request authenticated
  before the tombstone keeps running: its writes landed AFTER the erasure and nothing removed them
  (`imports` and `served` rows for the erased id from `GET`/`import`, `pool_prefs`, a `reports` row
  with the reporter id and note plus its audit, a counter-notice with the real name, address, phone and
  signature, and an OPEN appeal flag that blocked the purge of the erased author's removed item and its
  R2 body for about 60 days, its note text staying in the flag). Two parts, both required:
  * The guard lives IN the write (`ALIVE_SQL`, src/pool/shared.ts: `NOT EXISTS (SELECT 1 FROM users WHERE
    id = ? AND deleted_at IS NOT NULL)`), in the same statement, or chained by `changes() = 1` inside the
    same atomic D1 batch. Guarded: `recordServed` (`imports`, `served`) and `holdImportedImages`
    (src/pool/read.ts), `putPrefs` (src/pool/index.ts), `report` (its `reports` row, the instant
    student_data hold, the "3 reporters" hold, the key_leak hold and flag, and its `pool_audit` row),
    `appeal` (the flag, its audit by `changes()`), `counterNotice` (the `dmca_actions` UPDATE that takes
    the notice, so the notice INSERT, its audit and the admin flag, all chained by `changes()`, write
    nothing), `putBlob` (`pending_blobs`, `blob_uploads`), `takeDaily` and `countMiss` (`usage_daily`, so
    every route's counters), the publish route's `pending_blobs` extension, `held_publishes` charge and
    account lock (`pool_locks`), and the admin `PUT /admin/pool/limits` (`pool_limits` of an erased
    user; `*` is exempt). A route whose write was skipped for this reason answers 401, exactly what an
    unauthenticated request gets (`throwIfErased`), never a success and never a 404 or a cap alert.
    Not guarded, by design: the rate-limit store (keys are HMACs of the id; the auth package purges them
    at deletion), and writes keyed to an item or an admin rather than to the caller.
  * `erasure_resweep` (src/pool/cron.ts, daily, idempotent) now covers every table those writers touch,
    for accounts in `erasure_done`: `imports` (re-keyed to one fresh `deleted:` key per account, at
    most 50 accounts a run), `pool_prefs`, `reports` (note, then reporter id, NULL), `admin_flags`
    (`$.statement` and `$.note` removed, the key_leak `$.served` quote removed, and an open appeal of an
    erased account or on its items resolved `author_erased`, so the 14-day purge proceeds),
    `dmca_counter_notices` (name, address, phone, signature, description scrubbed for the account's own
    notices), `pool_audit` (notes NULL; the report audits unlinked), `usage_daily`, `pool_limits`,
    `pending_blobs`, `pool_locks`, `served`, and the author records it already deleted. It never touches what
    an erasure keeps: kept anonymised items, the users tombstone, `erasure_done`, audit rows without
    notes, `retired_publish_ids`, attestations. `deleteContentStatements` already scrubbed all of these
    for rows existing at erasure time; round 27 adds only `pool_locks` there.
  * Documented limit (critic 35c/35d): a name typed into the title or tags of an author's KEPT item
    (the author chose to keep it, anonymised) is content and is not scrubbed. A re-keyed straggler
    `imports` row gets its own anonymous key, so such an account can count as one more importer on an
    item than the batch's single key would give (pseudonymous, no PII).
* Stem-backfill starvation (V4). A retained body that cannot be read three times is skipped
  (`stem_bf_skip:<publish_id>`) with one `cron_failure` admin flag, so an unreadable publish cannot sit
  at the front of the bounded author window and starve other authors' backfill (`STEM_BACKFILL_FAILS`).
* The round-21 preview CPU assertion is tolerant under load (`POOL_STRICT_TIMING=1` restores the tight
  bound); the structural assertion (the planted text is matched from the reader's serves, never
  enumerated) is what proves the property.
* Tests: `test/pool/round26.test.ts` (8): the two B-1 directions (added material held; what was
  carried stays exempt, plus the legacy fallback), B-2 for a copier and an independent author over
  three republish paths plus the per-field Short/Blank case, B-3 over all four kept/hidden
  combinations for 30 hourly and 16 daily runs plus the erasure that lands during the R2 body read,
  V4, and an 8-seed differential. `test/pool/round27.test.ts` (11): for each of the writing routes
  (`GET`, `import`, `PUT /items/prefs`, `report` twice, `appeal`, `counter-notice`, blob upload, publish)
  the account is erased before EVERY statement boundary of one request, 40+ days of hourly and daily
  crons are run, and every table is scanned for the erased id and for the account's name, phone and
  address (only the residue a normal erasure leaves may remain; a request after the erasure is 401;
  the appeal case checks the item is purged and its R2 body gone, the counter-notice case that no PII
  survives); the resweep alone over rows planted after an erasure; and every guard through a second pool
  whose authentication still trusts the erased account.

**Round 25 (critics asvs-24 / moodle-24).** Three blockers in the round-24 work.
* The upstream-original exemption no longer lapses on an ordinary republish (asvs B1). The item the
  taken-down item came from is exempt from the publish-time re-upload hold BY THE DERIVATION (its
  origin / derivation chain link), not by a publish timestamp: the round-24 form still required the
  immediately previous publish (`base_publish_id`) to predate the block, so the SECOND ordinary
  republish of the original was held as a re-upload (and vanished from view). The timestamp rule
  stays only for the named rightful author's pre-existing material (round 14 B4), as before. The
  same exemption is applied to the held-publish charge, so the cap (`held_publishes`) cannot put a
  `probe_cap` hold on an exempt original either.
* The upstream exemption is not forgeable (asvs B2). `upstreamOf` (src/pool/served.ts) no longer has
  the "the taken-down item's author imported the candidate" branch: a plain GET is recorded as an
  `imports` row too, so one read of an accomplice's copy shielded it from hold-copies (and from the
  publish hold and `recheckBlocks`). Only the server-recorded origin and derivation chain links
  count, never an `earlier_match` credit or a claimed-independent possible source. The same rule is
  used in `lineageCopies`, the publish re-upload hold, `recheckBlocks` and the held-publish charge.
* The derivation survives the copy's purge with no serve (moodle B1). A taken-down copy that nobody
  was served leaves NO ghost, so `upstreamOf`'s live/ghost chain read lost the derivation after its
  uphold; the author's own original was then held on her next republish. `dmca_actions.derivation_json`
  now records the item ids the taken-down publish derives from when the action is made (live publish
  or ghost), and `upstreamOf` reads it first. An action made before this round has no such record;
  the live/ghost chain is still read for it.
* The licence-history re-key loses no recoverable grant (asvs B3). Migration 0105 maps old
  exact-content keys (`q:<content hash>`) to the canonical key through RETAINED publishes AND
  GHOSTS, so a version retired before the upgrade keeps its window. Rows whose near hash was still
  NULL at migration time (fold_rev = 0, waiting for the backfill) are finished by a bounded,
  idempotent hourly/daily `licence_rekey` stage that runs right after the backfill; it reads ghosts
  too. The batch selects MAPPABLE rows only, so an old row whose content is gone can never sit at the
  front and hide a mappable row behind it; such a row stays at rest (no view looks its key up).
  Several exact contents can map to one canonical key: their windows are MERGED (earliest `from_at`;
  open if any is open, otherwise the latest `until_at`), so no grant is lost. Documented limits: the
  merge can widen a window across a gap between two offers of the same canonical question (the old
  exact-keyed rows could not express that history; a CC grant the author already made is
  irrevocable), and a window whose exact content was a hidden reshuffle that was never served and is
  no longer retained has no content left to map — unrecoverable for pre-0105 rows.
* Stem-record backfill (moodle B2). `author_stems.bk_expl_full` is added by 0105 but a database
  populated before round 24 has no value for it (and one before round 23 may have no stem record at
  all), so the round-24 false-credit defect would return for hidden versions first published before
  the upgrade. A bounded, idempotent `stem_backfill` (a few authors/publishes a run) computes the
  full form's explanation key from the retained full body in R2 (a shown publish's served key is
  already in D1) and rebuilds missing records; it also runs at the start of a real publish for the
  publishing author, so the fix does not wait for the cron. An unreadable body leaves the key NULL
  ("unknown"), never a reason to credit a copier. No writes in a preview.
* Hourly licence reconcile (moodle new6): `offer_sync` now runs hourly as well as daily, so a licence
  re-opened by an admin hold release is not refused for up to a day. Key pinning (new4) samples up to
  five retained bodies, not one: a missing body no longer answers "unknown" when another readable
  body would prove a mismatch. The durable stem lookup's own allowance is flagged when reached
  (a cut can only keep a binding). Documented limit: an author who deletes and purges her original
  and re-adds the question as a NEW item has no derivation record, so the original's re-add can be
  held by a takedown of a copy; the admin's exempt-authors list is the path.
* Export (round-24 non-blocker): `version_bodies` now carries every retained version EXCEPT the
  current publish, whose body is already `bodies[item_id]`; the part byte accounting and the wire
  payload now agree.

**Round 24 (critics 31a / 31b).** Migration 0105 re-keys the licence history and recreates the
`import_questions_*` views; it adds `author_stems.bk_expl_full` (round 25: and
`dmca_actions.derivation_json`).
* The earliest author is never displaced by a copier's edit (31a B-1). The author's durable stem
  record (`author_stems`) is now ALWAYS consulted and the EARLIEST of it and her retained versions
  decides her first publication of a question; before, a retained version newer than the copier hid
  her older durable record. So a copier who makes the obvious next edit (a typo fix, a wrong key, a
  dropped distractor) first is not credited as an "earlier publication" of her own question when she
  makes the same fix, republishes, or uses it in a second bank, and her downstream copies never bind
  to his link. The durable record also keeps the block key of her FULL form's explanation (a keyed
  HMAC, server-side only, never a block for anyone), so a later SHOWN version (or second bank) still
  sees that her earlier HIDDEN version had that explanation: switching hide_key neither invents an
  addition of hers nor hides her priority. The ANSWER is deliberately not kept from the full form: a
  hidden key must never change an observable outcome (the H1 property holds her own flags to that
  too), so a hidden answer is never a reason her earlier version counts.
* A publicly relaxed licence reaches an earlier reader after an unrelated edit (31a B-2). The
  licence-history key (`content_licences.ckey` `q:…`) and the "the source's licence now" comparison
  are the CANONICAL SERVED CONTENT: the licence-sense near hash of the form the item serves
  (`near_full`, or `near_stripped` for a hidden-key publish) — its kind, stem, answer choices and
  Matching columns (order-free) and explanation. Points, difficulty, topics and the shuffle order are
  NOT in it, so changing a point value, a topic, or reverting an edit keeps the reader's window open
  and the relicence applies. (The exact keyed content hash before included all of those and, hidden,
  the shuffled choice order.) Canonical form rule: **the same question is the same kind, stem and
  answers (as a multiset), with the same explanation, after NFC/case/punctuation folding; never
  points, difficulty, topics, answer order, the key, ids or dates.** Documented limit: a reworded
  stem, a rewritten explanation, or a hidden→shown change of the served explanation is a different
  canonical content (rewordings still defeat matching by design).
* An unlisted, held or removed item's licence now grants NOTHING (31a V1, README's offer rule): its
  current licence no longer counts in the question licence union, the unit licence union or a ghost's
  frozen `now_licenses`; only its licence-history windows, as before (a window that was public after
  the serve still grants). The licence history itself is unchanged.
* A takedown of a copy never holds the author's own original (31a V2). The item the taken-down item
  came FROM (its publish names it as an origin or carries it as a derivation chain link — never an
  `earlier_match` credit or a claimed-independent possible source) is exempt from the publish-time
  re-upload hold, from `recheckBlocks` (switching back to public) and from hold-copies. An unlisted
  original cannot be named as the rightful item (that needs a visible item), so this is decided by
  the derivation itself. (Round 25: no import/read branch — a GET is an import row and was forgeable;
  the exemption is by derivation only, and it never lapses on a republish.)
* Account export (31a (d)/(e)): an import shows the item's title only while the item is visible to
  others (unlisted, held, removed or a privacy removal → `title: null`; the importer still sees the
  id, licence and time, never the author's private title). Every RETAINED version's full body is
  exported, not only the current one: `version_bodies` maps each retained `publish_id` to its full
  body (the author's own hidden keys included), in the whole export and in each part; the part size
  accounting covers all retained bodies. (Round 25: the current publish's body stays
  `bodies[item_id]` and is not repeated in `version_bodies`, so the accounting matches the wire.)
* Pinning the hash key (31a f). A pool populated before round 23 has identity data but no recorded
  key id. The first write now VERIFIES the key before pinning it: one retained publish's keyed
  content hashes are recomputed from its R2 body and compared with the stored rows (only for an item
  the current worker hashed, `fold_rev = 1`). A mismatch is `503 pool_key_mismatch`, one alert, and
  NOTHING is pinned, so restoring the right key still works. An empty pool pins on first use as
  before. Documented limit: a populated pool whose retained bodies are all unreadable or pre-round-12
  cannot prove the key; it is then left UNPINNED (accepted, unverified) rather than pinning a guess —
  an unpinned key locks nobody out, a wrongly pinned one locks out the real key for good.
  `POST /items/versions` is a bulk read (item metadata, never a hash) and continues under a mismatch
  like a GET; every other non-GET route still fails closed.

**Round 23 (critics 30a / 30b).**
* Ghost takedowns (30b A2): a takedown of a purged item is planned from its ghost (the keyed block keys
  its last publish kept: positions, skeleton, stem and full keys, part keys) by the live rules: the
  admin's selection and scopes, block_common_k, exempt authors, the same preview. With no selection
  NOTHING is selected (`selection_required`, the takedown 409s): the admin picks positions, or sends
  `questions: "all"` for the live default (introduced, distinctive questions).
* Upholds (30b A1, F1): one claim-and-finish for a ghost takedown's uphold, at the end of its window or
  on a court decision (`resolve remove`); the finish (evidence void, strike, copies held, repeat
  check) is resumed by the cron until done (`uphold_pending`). A resolve records its decision only
  once it took effect.
* Erased authors (30a B2): on EVERY path (live, anonymised, within the purge window, ghost) an upheld
  takedown of an erased author's item voids its evidence only when the admin named the rightful item;
  otherwise its licence keeps binding copies. An erased account's item inside its 14-day window can be
  taken down (it returns there on a put-back).
* Counter-notices (30b A3) are taken only while the action is open, in the same statement: an uphold
  that committed first wins and the author gets `409 window_closed`. An admin can reverse an upheld
  action with `resolve {outcome: "put_back"}` (strike voided, evidence restored, blocks and holds
  lifted; purged content stays purged).
* The author's takedown notice (30b F2/F3) is written to `pool_outbox` with the action and sent until
  sent (hourly). /items/mine lists a ghost takedown awaiting her counter-notice (`takedown_pending`,
  action id, window end; no content), and the mail names the item and action.
* Authorship by order (30a B3): the author's own earlier version of a question counts for her later one
  whenever the later one adds nothing to it (same kind and choices; no explanation or answer the earlier
  lacked): removing her explanation never makes her copier "earlier". Her keyed record of every stem she
  published (`author_stems`) keeps this after retention, deletion and purge.
* Licence history (30a B4, 30b W1): a licence counts only while the item OFFERS that content publicly
  (public, live, not held): unlisted ("not offered"), held or removed windows grant nothing. One row
  per (item, content, licence), re-opened when offered again; reconciled hourly; rows no serve can reach
  are collected; past 5,000,000 rows the admin is flagged. A unit's window counts from the reader's
  first serve of the ITEM (documented).
* Finishing (30b F4, G1): the account_deleted stage waits for the erasure batch (which applies
  keep_on_delete); a withdrawal's retry and the cron release blocklist holds of put-back actions; a
  ghost put-back flags admin-held copies for release as a live one does.
* Publishing attestation (owner): every publish and republish carries `attest: {own_work: true,
  terms_version}`; without it, or with another terms version than the server's `POOL_TERMS_VERSION`
  (`src/pool/license.ts`, now "2026-10-22"), it is refused `422 attestation_required` with the current
  `terms_version` and the `statement` the app shows: "I confirm this content is my own original work, or
  I have the right to share it under the licence I chose. It is not copied from a published source
  (textbooks, publisher test banks, exam boards, paid worksheets) without permission." A preview does
  not need it. Each publish's attestation (when, which version, by whom) is kept in `attestations` while
  the publish or its ghost exists, shown only in the admin item view and DMCA actions; after erasure the
  user id stays as the pseudonymous tombstone id. **The attestation is the teacher's affirmation; the
  server does not and cannot verify it.** It changes no flag or hold.
* Account export (owner): `GET /me/export` returns a JSON attachment (`Content-Disposition:
  attachment`, `no-store`, `nosniff`) of the account's own data: its profile (email, name, profile,
  approval state, dates), its items (title, subject, licence, visibility, state, every retained
  version with its time, and the FULL current body, its own hidden keys included), its imports and
  reads (item id, title, the licence it was shown, the time; no one else's content), the reports it
  filed, the takedowns of its items and the notices sent to it, its attestations, counter-notices and
  appeals, and its sessions (times and user agent; no ids, tokens or hashes). Never: another user's
  personal data (reporters, complainants, emails), admin notes, flags, hashes or sketches. Teachers
  and admins get everything; a pending account its profile only (others 403). Three a day
  (`exports`); the session must have logged in within 24 hours (`403 reauth_required`); an erased
  account cannot authenticate (401). Each export is audited (`account.export`, counts only). Above 8 MB
  of item bodies it returns everything but the bodies with `export_too_large: true` and a manifest;
  `GET /me/export?part=n` returns part n's bodies (at most 4 MB each by stored size; parts do not count
  against the daily allowance).
* Audit notes (30b T1): erasure nulls the notes the account WROTE; an admin's note is evidence and stays,
  the subject's display name and email replaced by its id when the note is written (auth_audit keeps its
  own copy). Admins should reference ids, not names.

**Round 22 (critics 29a / 29b).**
* Takedown of an item purged before its notice (29a B1): the SAME state machine as a live item. The
  author is told (`takedown_author`; an erased account is told nothing, the admin sees it), the 30-day
  counter-notice window opens (the counter-notice route takes the purged item: the tombstone says
  `takedown`), the strike is PENDING, and re-uploads of its question hashes are held. Inside the window
  nothing is voided: copies keep the author's credit and licence. A counter-notice put-back or a
  withdrawal voids the strike, lifts the blocks and holds it caused, and returns the tombstone to what it
  was. With none, after 30 days (cron `ghost_upholds`) it is upheld as an upheld purge is: evidence
  void, strike upheld, every lineage copy held (paged, idempotent) when no rightful item is named, the
  repeat-infringer check. **An erased author** cannot counter: the takedown stands after 30 days, but
  with no rightful item named its evidence is NOT voided, so the ghost's licence keeps binding the
  copies (nobody launders the work by the takedown); naming the rightful item (`replaced_by`) is the
  existing design for crediting the real author.
* Sketches (29a B2, 29b H1): every token sketch and band of purged, erased or private content is
  HMAC-SHA256 under an HKDF-style subkey of POOL_HASH_KEY (`vershion-pool/fingerprint/sketch-v1`, never
  AUTH_SECRET), truncated to 64 bits, stored as an UNORDERED, deduplicated, sorted set (no order, no
  positions). MinHash bands are keyed the same way (HMAC of the band tuple). Live public questions keep
  their unkeyed bands in `stem_bands` only while they are live; delete-to-ghost, purge and erasure
  remove them, and a ghost is found by its keyed bands (`ghost_bands`) only. Ghost and privacy rows
  hold no token list, ordered hash or unkeyed value. Matching is unchanged (sets keep Jaccard).
  (Round 22 described a sketch re-key job for a key rotation; round 23 removed it: see "POOL_HASH_KEY
  is permanent" below.)
* Licence history (29a V-a): `content_licences` records every licence an item had at each served
  content (question form, unit key) from when it was published with it until a publish without it or
  its purge. A serve counts every licence in force at that content since the serve, however many
  publishes (NC, CC0, NC with no republish in between still grants CC0).
* Takedown responses (29a V-b) list the copies hold-copies would hold (the first page, `copies_more`);
  `/admin/dmca/copies` pages its held and candidate lists on their own cursors (`held_after`,
  `candidates_after`; `next_held`, `next_candidates`); hold-copies of a put-back or withdrawn action is
  `409 action_closed`.
* Text units (29b B1) have their OWN seq (`item_questions.unit_seqs`): a republish carries a text's seq
  only when that very text was in the previous publish of the qid; a text it adds is dated by the new
  publish (the question's match_hash leaves explanations and samples out, so its seq never dates them).
* Stock (29b B2): a unit is stock only if 3 or more authors published it independently AND at least 2 of
  them did so BEFORE the earliest source the publisher was served (or, served none, before its first
  holder). Text that first appeared in the pool with one author and was later copied by unrelated
  accounts, socks or not, however many, stays that author's.
* Hot keys (29b): a key past 64 occurrences is counted again up to the publisher's own served rows (at
  most 4,096, within 262,144 index steps a publish); only a key with more copies than that goes to the
  served-driven path, so planted copies alone cannot move an author onto it.
* Finishing what fails part way (29b H2, H3, mediums): a privacy purge records its backlog in the same
  batch as its claim (copies past the list, the bodies' move), the cron or the admin's retry (200)
  finishes it, copies held before it is marked done; a put-back that failed after its claim is finished
  by the cron (`putback_finish`); an erasure whose batch failed is redone daily until `erasure_done`
  has the account; erasure closes the account's open appeals (`author_erased`) so its removed items
  purge on time, and nulls the notes of audit rows it wrote or about its items; a repeat-infringer
  check that failed is redone from the strikes; a body copied to R2 is recorded as an orphan until the
  row naming it commits; a failed import gives its import allowance back; a takedown marks its notice
  `actioned`; share-core trap counts leave with the request that trapped (login included). Migration
  0103's seq backfill is computed once and joined (linear: well under a second at 40k publishes).

**Spec text (round 22, accurate statements; doc drift to fix in docs/shared-pool.md).**
* docs/shared-pool.md:128-130 ("Undo restores content only ... the admin re-links the held items") and
  :136-137 ("Implemented by `onDeleteContent` in `server/worker/src/cleanup.ts`, which today only logs a
  `PHASE 2 TODO`") have drifted. Accurate: account deletion runs the pool's erasure batch
  (`deleteContentStatements`, src/pool/index.ts) from the auth package's delete hook; if that batch
  fails, the daily cron reruns it until it commits (`erasure_done`). The account's items are
  anonymised when it chose to keep them (or, no choice made, when another teacher imported them) and
  otherwise removed (`account_deleted`) and purged after 14 days; its open appeals close; its report
  notes, statements, appeal notes, counter-notice personal fields, served rows, authorship records and
  bands, uploads and audit notes go. The auth package keeps the email HMAC for 14 days
  (`findTombstoneByEmail`), but the pool has no re-link route in phase 2: nothing re-attaches the
  removed items to a new account, and once purged they cannot be restored.
* Items under an open DMCA action of a deleted account follow the same purge date; a takedown of one
  purged first is still taken down (above), and since nobody can counter, it stands after 30 days
  while its licence keeps binding copies unless the admin names the rightful item.

**Round 21 (critics 27a / 27b).**
* Units (B-1, B-5, M-B2, V4): each key's occurrences are counted only up to 64; a key past that (a stock
  phrase or planted copies) is matched from the reader's OWN served content, the rest by key; per
  question and key only the earliest 8 served sources (one material: its earliest decides) and the
  earliest 8 stored occurrences (stock and relays: listed items or ones the reader was served, earlier
  than the reader's own first use) are looked at. The allowance is the publisher's own distinct units
  (60,000: 422 `too_many_units`); nothing anyone else publishes can make an author refuse or pay more,
  and an item the caller cannot see never changes an answer. Dry runs pay per unit.
* Near candidates (B-2, M-B1): counted only when admissible (not the publisher's own or derived items,
  not attested independent, published before the publisher's own first, a served version with another
  match_hash; questions with a source matched on themselves last); earliest first, at most 200 a
  question examined and 32 admissible kept. Sources left out (there, or past the exact evidence
  allowance with no copy kept) never decide by being left out: the publish is refused (422
  `license_incompatible`, bound_by naming the first forbidding one) only if one of their licences
  forbids the licence chosen; otherwise it is published with a placeholder link carrying their
  licences, and flagged. `too_many_sources` is gone.
* Unit stock (M-B3) counts only authors who published the unit independently: a listed item, never
  served an earlier holder of it, no lineage from one (an `earlier_match` credit is not lineage).
* Earlier publications (B-4) are ranked earliest first before any cut; a cut is flagged.
* Takedowns (B-3, D2, G1): copies are compared with the infringing item's current publish (read before a
  purge's claim), never vacuously; every copy is held, a page at a time (hold-copies says `more`; the
  upheld purge holds all); a takedown that failed part way resumes on a retry of the same notice and in
  the hourly sweep; a takedown of an item its author deleted and the cron purged first is upheld at once
  (strike, evidence void; superseded in round 22: the live state machine, pending first).
* Purges (G1) that failed after their claim are finished by the next cron run; purges and erasure (E1)
  leave no typed text: a tombstone's title is `[removed]`, key_leak flags lose the served text (purge)
  and a reporter's quote (erasure), student-data bands keep a keyed sketch (0104 drops the plaintext
  tokens; a band without one matches on the band).
* Ghosts (V1-V3): `now_licenses` gathers every licence the item had for the very content until it goes;
  units keep when they were first published; questions keep a keyed token sketch (no words) and their
  public bands stay (round 22: keyed bands instead), so a near-rewording of a publish someone was served still binds after it is gone.
  (Rows published before 0104 have no sketch: their ghosts are matched exactly only.)
* Migration 0104 (M-B4): a void covers only the serves that existed when it was made.
* Base headers on pre-read errors and fallbacks; /admin/pool/reports and /items/mine are paged by a
  cursor; a lint test lists every LIMIT, slice and take on these paths with why each is safe.

**Round 20 (critics 25a / 25b).**
* A stored link records the licence that ALLOWED the use (E1): per content, the most permissive licence
  available to the publisher (what it received, or the source's licence now for the very same content)
  that permits the publish; across contents, the most restrictive of those. Downstream copies are bound
  by exactly that, so a copy made under a permissive relicence credits it, and a received CC0 stays CC0
  however the source is relicensed later. A unit's received licences are those of the serves whose
  publish held it. The source's licence now counts whether or not it is still listed (a void cuts it),
  and a ghost freezes it (`now_license`), so no delete, purge or erasure changes an outcome.
* Caps only refuse (F1): past `UNIT_ROWS_MAX` (25,000) unit matches a publish is refused with 422
  `too_many_units`; a question whose sources exceed the allowance with no copy among them, or whose
  IMPORTED near candidates were cut, is refused with 422 `too_many_sources` (an untouched question of a
  republish keeps its outcome). Copies are kept earliest first, so the cut never changes a copy's
  outcome. Near candidates are filtered for admissibility before anything is counted (an identical or
  later copy never takes a place). Read candidates and hot bands are soft (flagged). Privacy near
  checks past their allowance flag every question sharing a band; a rightful check past 1,000 shared
  questions fails; a hold of more than 500 copies is refused (an upheld takedown holds 500 and says
  more remain); the soft check filters before it lists. A test lists every LIMIT on these paths.
* A serve is recorded only while the publish read still exists, in the same batch (F2): a read that
  raced two republishes is a 404 with nothing recorded.
* Erasure (F3) deletes the account's author records; a key_leak flag keeps the report id, never its
  note; 0104 creates no served rows for anonymised (erased) readers.
* The ghost GC deletes at most 20,000 rows of each table per run (F4); dry runs count their unit
  lookups against `preview_units` (2,000,000 a day).
* A re-read of an item already read is never refused by `reads_max`; a fix of a held item is exempt
  from `author_records` up to 1,000 new records; the `relicensed` hint reads the served rows.
* Migration 0104 carries voids made before it (E2): see "Migration 0104" below.

**Round 19 (critics 24c / 24d).** Replaces round 18's evidence caps, eviction and expiry (V4).
* Binding evidence (C1) is one small row per (reader, publish read) in `served` (item, publish, kind
  import/view, every licence shown, first and last serve). A later publish is matched against the
  SOURCE publish's own key set (its questions, texts, part and question units, images by bytes and
  pixels), never against per-reader copies; `import_questions` is now a view (`import_questions_live`
  and `import_questions_ghost`, read through hash indexes). Nothing is ever evicted.
* Reads and imports are bounded separately (C2): `reads_max` (200,000 distinct publishes read) refuses
  a read of a publish not read before with 413 `reads_quota`; `imports_max` (20,000 distinct items)
  refuses a new import with 413 `imports_quota`; `imports` stays 100 a day. A read never blocks an
  import and an import never uses up reads; neither ever refuses a publish.
* Gone sources (C3): a publish anyone was served leaves a key-only ghost when it is deleted, retired or
  purged (`ghost_publishes`, `ghost_questions`, `ghost_units`: hashes, ids, licence, credits; no text,
  no tokens). Credits name the author from the account, so an erased author reads "deleted teacher" and
  the licence still binds. A ghost goes when no served row references it; served rows go only with
  their reader's account (no expiry). The daily evidence_expiry stage is gone (critic24d D4).
* Author records (C4): only the records a publish would newly write count against `author_records`
  (500,000), exactly (D6); a republish adding nothing and any publish of a held item (a fix) are never
  refused; the refusal says republishing still works and that an admin can raise the limit. (Records
  stay durable through deletes: they are what keeps the author first.)
* A relicence to more permissive terms covers earlier readers: the item's current licence counts when
  its public version serves the very same content (a). The `hidden_material` refusal lists the items
  the author was shown the material in, with their titles and licences (b). A key_leak hold is carried
  through deleting the item and publishing the same content again (c).
* D1: a serve whose publish's units are unknown (a pre-0104 serve of a publish gone since; its 0104
  ghost kept question hashes only, `units_known` 0) binds through the item's retained publishes with
  the licences that serve holds. A serve whose units are known is examined exactly, so a version never
  served still does not bind (round 17 M1).
* D2: dismissing a key_leak report lifts the hold when fewer than two QUALIFYING open reports remain
  (`keyLeakQualifies`, the predicate that placed it). D3: the unit and evidence queries drive from
  their hash indexes (no scan of item_publishes, served or the question tables; tested by EXPLAIN).
  D5: duplicate keys in any pool JSON body or publish meta are refused (400 `invalid_json`,
  `duplicate_key`); the auth package's JSON bodies too. D7: the database size alert is its own notice
  (`d1_size`, its own mail subject).

**Round 18 (critics 23c / 23d).**
* Unit evidence is durable and per reader (`unit_evidence`, U1/U2; round 19 replaces the per-reader
  copies with `served` rows matched against the source publish): every serve (a full read or an
  import) records the units of the publish served (texts of the question and its parts, images by
  bytes and pixels, question units) with the licence shown then; a later publish is bound by what the
  account RECEIVED, whatever the source's retention or relicensing did since. Material the account was
  never served is found in retained publishes only (earlier publication, stock).
* Hidden keys (U3): text units are built from the SERVED form only. A hidden publish's hidden-only
  explanation or model answer changes nothing anyone else observes (no link, badge, flag or licence
  record); when it equals material its author was served under an incompatible licence, her publish is
  refused (`hidden_material`), to her alone.
* `key_leak` (U4): only for a hidden-key publish; a report counts only from an account approved 7+ days
  that was served that publish a day or more before; one per item per 30 days, 5 a day; a hold needs two
  such reporters on the current publish, at most 3 holds a day per reporter, never from reporters with a
  dismissed key_leak report on the item in 90 days. The hold belongs to the publish: a republish whose
  served form changed (or that shows the key) ends it and resolves the reports as `fixed`; a dismissal
  that leaves fewer than two open reports lifts it; the default `release` treats it as human. The
  author sees `key_leak_review` with what to fix; key_leak notices have their own hourly budget.
* Stem-only question units (U5: Tf / Short / Blank prompts) read are soft (credited, flagged, never
  binding), imported they bind and a claim may cover them; units with a body follow the question rule
  (a claim covers a read short one, never an import).
* Part model answers (U6) are texts both ways. (Multipart parts have no explanation field: share-core
  drops one sent there.)
* Claims never cover an IMPORTED text or image; an attribution needs attribution words (public domain,
  CC BY / CC0, Creative Commons, OpenStax, Wikimedia, Wikipedia): a URL alone is none.
* Migration window (V3): non-author full reads and imports also answer 503 `pool_migrating`; the
  backfill repairs any evidence row of an item still without hashes. An item whose bodies can never fit
  the isolate's budget is skipped (`fold_rev` 2, `backfill_skipped` flag) so the window closes; a window
  open more than a day raises `migration_stalled` and alerts the admin.
* Storage (V4; its caps, eviction and expiry are replaced by round 19): reads no longer write per-reader band rows (the fuzzy lookup uses the public band index,
  restricted to items the account has evidence of; near-copies of versions no longer retained are lost
  to it); an account keeps at most `evidence_rows` (10,000) evidence rows per table: past it a read
  evicts the oldest READ rows and a new import is refused (`evidence_quota_exceeded`); `imports` is 100 a
  day; evidence of a source purged more than 90 days ago goes unless it binds a copy the account
  published; an account's durable authorship records are capped (`author_records`, 100,000:
  `author_records_exceeded`); the cron alerts the admin at 60% / 80% of D1's 10 GB (`d1_size`).
* JSON responses carry `Content-Security-Policy: default-src 'none'; frame-ancestors 'none'`; unexpected
  errors are logged by kind (never a parser message quoting the input).

**Round 17 (critics 22c / 22e, client additions).**
* Licence units (M1-M6): what a question uses besides itself binds on its own: an explanation or Short
  model answer of 25+ words ("text", either field), its image ("image": the exact bytes, or the same
  decoded pixels of a PNG, so a re-encode is the same image), and a question wrapped as a Multipart
  part or a part published alone ("part": key-free unit keys of the kind, stem and choices, both ways;
  a whole question against a whole question stays the question rule's).
  - Only from a version the account was SERVED (an import or a full read of that publish; a later
    version it never saw does not count; a browse list is no serve).
  - Never against its own author: every unit an account publishes is recorded durably
    (`author_contents` kind `unit`, kept after a delete or purge), and her own earlier use exempts it.
  - Stock material (3+ distinct authors' items) never binds.
  - An independence claim never covers a unit, except a text or image the statement attributes to a
    public source (public domain, CC BY / CC0, OpenStax, Wikimedia, a URL), or an image the claimant
    uploaded before the other item was first published (`blob_uploads`): then a possible source, flagged.
  - Material equal to an EARLIER public item the account was never served (a relay) is credited as an
    earlier publication (`unverified_earlier_match`, `license_soft`), never a refusal.
  - Links are typed in chains (`kind`: "image", "text", "part"): an image is "also used by", never an
    unmodified copy of the question; the same material from several sources is one material (any grant
    of either applies).
* Near stems (M3): a fuzzy (near-stem) source counts only with the same KEY-FREE body (kind, choices,
  Matching columns, part stems: token Jaccard >= 0.5; a Tf / bare Short is its stem); otherwise the
  admin is told (`origin_suspect`) and nothing else: no credit, binding, label or refusal.
* Rewording limit (M5, documented, not chased): an edited word in a copied explanation, an
  explanation split across fields, a non-PNG image re-encoded, a question turned into True/False with
  its answer in the stem.
* Raw-bytes scan: a byte-order mark, tokens other than numbers / true / false / null, and a second
  top-level value are refused before the Worker parses anything.
* Blocked images (22e A1): the bytes of a privacy-blocked image are never required at publish (the
  publish is held, exactly like a control's would be published); a purge removes only the purged item's
  author's pending uploads of its images, never another account's.
* Migration window (22e A2): while 0102-era items wait for the backfill, publish, republish and import
  answer 503 `pool_migrating`; the backfill (200 items a run, one at a time, memory reserved, progress
  logged) also writes their authors' durable records (`author_contents`, `author_firsts`, units), so an
  original 0102 author stays first. 0103 aggregates the committed schema's evidence per served content
  (an import wins over a read; every licence shown is kept).
* Derived rows (22e A3): a retired publish takes its public and own stem bands along; a publish writes at
  most 4000 part / unit key rows; an account keeps at most 3 evidence rows (and 24 band rows) per question
  of an item it reads; a second read writes nothing.
* Storage (22e): images only deleted items keep count toward the removed-items cap, and uploads are
  refused past it. A blocked upload's same-size stand-in is removed within a day.
* `POST /items/preview` (and `POST /items/:id/preview` for a republish): the publish body and checks,
  author-only, counted against `previews` (200 a day; never the publish allowance, the refusal counter
  or the held cap), memory reserved like a publish. It stores nothing (no rows, bodies, blobs, counters
  but `previews`) and returns a publish's refusal unchanged, or `{ok, dry_run, hide_key, student_view
  (the exact stripped payload of a hidden-key publish), notes, label_findings, labels_removed,
  images_removed, figures_removed, order_reasons, license_check, chains}`. It never says anything about
  holds or blocks, and never returns served orders (a real publish draws its shuffle again).
* Reports: reason `key_leak` (an importer saw a visible hint of a hidden key). The admin queue gets a
  `key_leak` flag with the served text (stems and choices, at most 4 KB); the author is told
  (`key_leak_report` notice, budgeted, the reporter never named); two or more distinct reporters hold a
  hidden-key item for review (`reports` / `key_leak`) until the author fixes it or an admin releases it.

**Round 16 (critics 21c / 21d).**
* Takedown exemption (B1): a rightful item exempts its own author, and an account it was served to
  by a recorded import or a chain link that is derivation evidence; never an "earlier identical
  publication" credit or a claimed-independent possible source, and never the author of the
  taken-down copy. Reordered choices are the same question (order-free keys) and held too.
* Earlier publications (B2): compared in every served form (this publish's served full or stripped
  content, its key-free content and its content without the explanation, against a source's shown
  full, stripped, key-free and bare forms; never through a hidden key), and by a near-rewording of a
  served stem of the same kind (token Jaccard >= 0.8 over the public stem-band index, user "pub").
  Any match credits the earlier author (`earlier_match`; `possible_source` for an item the publisher
  attests independence of), reads `unverified_earlier_match`, flags `license_soft` and binds
  downstream copies. "Earlier" means before the publisher's own first publish of the question
  (its match_hash, content, lineage or own near-rewordings).
* The original author (B3): a copy she was served after her own first publish of the question
  (key-free: its served form, a hidden key or not) never binds her, whatever it added; copying its
  explanation or model answer is a lift (below). A Short's model answer is a work: a bare stem never
  makes anyone first.
* Report holds (B4) follow the question's normalised skeleton and content as well as its exact text,
  and outlive the held item: when it is deleted, its keys (hash-only, `carried_keys`) keep holding the
  same content until its reports are resolved or 90 days pass. Only a hold an admin placed or that two
  or more distinct reporters stand behind is carried beyond the reported item (critic21d 4), never for
  a short or common question.
* Multipart parts (B5): each part's stem, explanation and answers are block keys of their own
  (`item_part_keys`), matched against standalone questions and other Multiparts' parts, existing
  copies included; near-rewordings of a part stem are held (`privacy_near`). Scopes: the default and
  "stem" cover every part stem; "explanation" / "answer" the parts' too; `part:<k>` all of part k.
* Lifts (B6): an explanation or Short model answer of 25+ normalised words equal to one the account
  was served (read or imported; as an explanation or a model answer, either way) is a copy of that
  item: credited and bound (an adaptation), whatever the stem; an independence claim never covers it.
* Images (B7): the exact bytes a publish serves that the account was served from another author's
  item bind and credit that item (a verbatim copy, any claim notwithstanding), unless the publisher
  served them first or three or more independent authors' visible items already serve them (stock).
  Takedown previews, hold-copies and the copies list also find items by the blocked image's bytes.
* Migration (B8): 0103 rebuilds the committed schema's evidence rows (import_questions) from imports
  and the served publish instead of dropping them; the cron backfill then fills in every hash rounds
  6-16 added (near, key-free, bare, content, stem words and bands, part keys, public bands) and the
  evidence rows' hashes (rev 0 -> 1). Tested on a database populated by the committed worker
  (test/pool/fixtures/m7_0102_*). Rows the backfill has not reached bind as adaptations (match_hash).
* Raw bytes first (21d 1): share-core's structural limits (depth 16, 200,000 values, 60,000 objects,
  no duplicate keys) are applied to the raw publish bytes before the Worker parses them; a failing
  payload gets share-core's own refusal. The parse is reserved by its value count when that is more.
* Tag browse (21d 2) shortlists like the word search: shown publishes only, newest first.
* Privacy preview (21d 3): `suggested` only where this item introduced the content; `first` names the
  item and author that had it first.
* Blocked images (21d 5): a well-formed image is accepted alike whether or not it is privacy-blocked
  (a blocked one's bytes are never stored: a same-size stand-in goes to a quarantine key); a publish
  serving it is held (student_data), never refused.
* PNG (21d 6): at most 16 Mpix and 64 MB of decoded pixel data, checked at the header.
* `release-hold` (21d 7) needs the `publish_id` the admin reviewed (409 `publish_changed` otherwise).
* Pending accounts (21d 8) are rate-limited like everyone before being told they are not approved.
* Independence claims: `independent_repeat` counts distinct items claimed in 30 days; making the
  same claim again (bank maintenance) is not a new one. A question with an exact source no longer
  credits a near-rewording beside it (flagged `origin_suspect`). A licence refusal caused by a source
  relicensed after the account received it says so (`relicensed`, `message`); `keyless_needs_hide_key`
  has an author message.
* Documented limits (c6): a re-upload after a takedown that adds or drops a distractor, adds a
  sentence to the stem or turns the question into True/False is a different question to the blocks
  (it is credited and bound by the licence rules when the account was served the original).

**share-core round 8 in the Worker.**
* `publish_prepare` refusals carry `formatting_findings`, `formatting_findings_total` and
  `order_preview` themselves; the Worker passes them, and each error's `reason`, `ack_allowed`,
  `author_only` and `hint`, to the publishing author only (no second share-core call). `key_in_stem`
  (reasons `answer_line`, `gift`, `cloze`, `tf_value`, `left_names_right`; never acknowledgeable)
  refuses a hidden-key publish whose question text gives the answer away.
* `strip_author_data()` is the very next share-core export after `publish_prepare`, in the same
  synchronous adapter call (every export clears it; the adapter test records every export).
* Matching left items are served shuffled unless `keep_order`; the author's `served_orders` entry
  has `left_order` (served left `i` = the author's `pairs[left_order[i]]`) and `order` (the bank).
  Whether a hidden republish changed anything served compares a shuffled left column as a
  multiset, a kept one in order. When the previous stripped body is reused, the author's
  `served_orders` are remapped onto that body (they describe what is actually served).
* `dropped` notes (named by `reason`) carry the author-facing message for hidden-key publishes too.

**Rounds 13-14 (critics 19a/19b).**
* Privacy blocks: the default scope ("default") is the question's skeleton (exact and normalised),
  its normalised STEM alone (`s:` key) and its normalised full content: student data may be in the
  stem, so every question with that stem is held whatever its answer or explanation, existing copies
  included (listed in the preview's `affected` and per question in `affects`). The admin narrows a
  question to "explanation", "answer" or "full" when the data is only there. A stem shared by 3 or
  more other authors' visible items is `generic_stem` (with `suggest_scope`, and not `suggested`). A
  near-rewording of blocked student-data text is HELD (source `privacy_near`) and flagged. Scopes:
  default, skeleton (DMCA's default), stem, full, explanation, answer.
* Block keys are computed from the SERVED form (the stripped form of a hidden-key publish), so no block
  acts on, and no hold reveals, a hidden answer or explanation (a hidden Short has no "answer" key).
* Every Worker fingerprint of question content (licence-sense hashes, key-free facts, block keys,
  loose stem hashes) is HMAC-SHA256 under a per-purpose subkey of the pool hash key.
* Held-publish cap (B1): never a refusal. It is charged per question that a block would hold, never
  for what an exemption covers (the rightful author and those served the rightful item, exempt
  authors, an admin's release); past 20 a day the account's matching publishes are also held
  (`probe_cap`) and the admin is alerted; unrelated publishes always go through. A determined prober
  can still learn whether a guess matches from its own item's held state (positives are held, the
  admin sees the account); that is the bound, not a count of guesses.
* Identity (B2): licence-sense hashes treat Mc choices (locked ones keep their place), Matching pairs
  and distractors, and Multipart parts as multisets unless `keep_order` is set.
* Earlier identical publication (B3): a question crediting nobody that equals what another author
  published visibly earlier is credited as `earlier_match` and reads `unverified_earlier_match`; the
  link binds downstream copies; short statements and common questions are left alone.
* Takedowns (B4): copies are found by content keys across all publishes (unlisted ones too) as well
  as by lineage; `visible` in the copies list is visibility to others; a PATCH to public re-runs the
  block checks; what an item carried before a block is exempt only for the rightful author.
* Generic reads (B5): an exact copy of something only READ may be attested independent when the
  question is 40 words or fewer as served, or 2+ independent authors have it: it binds nothing, the
  earlier author stays credited (`possible_source`), the badge is `claimed_independent`, the admin is
  flagged. Imports never. The tradeoff: a determined copier of short questions can claim
  independence; the claim is public and flagged.
* Lineage (B6): a qid's lineage also needs each stem within token Jaccard 0.6 of the lineage's root
  stem; a served source it would cover is not credited or binding but flags `origin_suspect` and
  `license_soft` (admin only).
* Soft relabels (B8): a publish whose only restriction came from a soft (read, changed) source reads
  `unverified`; a downstream refusal names the upstream restriction (author, licence, `via` the relabel).
* `license_check.status`: ok, mismatch, unverified, claimed_independent, unverified_earlier_match.
* Rightful items are judged per blocked hash (one merged question never disqualifies the rest).
* Search shortlist: taken among shown publishes only (public, live, unheld), newest first, from at most
  50,000 word rows. Admin releases (`release-hold`) cover 90 days and never a block placed after them.
  The backfill takes one item at a time with its memory reserved, by id; a failing item is marked
  (`fold_rev = 2`) and flagged once. On account deletion `author_contents` stay under the anonymous id.

**share-core round 7.** A publish makes ONE share-core call, `publish_prepare` (validation, the
stripped form when the key is hidden, match hashes and keyed full hashes, with `hide_key`, `labels`,
`order_acks`, `ack_formatting`, `image_ok`, `drop_client_provenance: true` and the derived hash key);
`strip_author_data()` is still called synchronously right after it, in the same adapter call. What the
server rewrites before that call (question ids, dates, its own provenance, image references) is read
from the author's JSON, type-checked field by field; anything malformed is handed to share-core
unrewritten and its refusal is the answer; only share-core's validated output is stored. Content
hashes ("the same content", ids and dates aside) are HMAC-SHA256 with the pool hash key over a
canonical JSON, computed by the Worker (`core.contentHashes`), not by more share-core passes.
Measured on a 3.8 MB hidden-key bank (1,000 Mc questions of 10 choices): 2.7 s in node, 2.9-3.2 s per
publish in local workerd (round 12 with three share-core passes: 7-8.5 s). The licence-sense
normalisation (`normText`) also drops invisible format characters (ZWJ, ZWNJ, zero-width spaces,
word joiner, soft hyphen, BOM) and the combining grapheme joiner and folds every space variant, like
share-core's round-7 `hash_form` (stricter: it never keeps a ZWJ). Match hashes changed in share-core
round 7 for such texts; every stored hash so far was produced by tests, so no migration re-derives
them (the cron backfill recomputes block keys and body hashes only for `fold_rev = 0` rows).

| Route | Who | What |
|---|---|---|
| `POST /items` | teacher | new item. Body `Content-Type: application/vnd.vershion.publish`: `<meta JSON>\n<payload bytes>` (the payload goes to share-core unparsed), exactly `Content-Length` bytes (411 without one, 400 `length_mismatch` if the body is longer or shorter). Meta: `publish_id` (client, >=128 bits, the idempotency key), `hide_key`, `license` (no default), `attestation {accepted, text_rev}`, `labels`, `order_acks`, `image_ok`, `visibility`, `meta {title, subject, course, grade, tags, standards}`, optional `independent_of {items, accepted: true}` (see Evidence) |
| `PUT /items/:id` | author | republish; meta adds `base_publish_id`; 409 `conflict` on a lost CAS |
| `PATCH /items/:id` | author | `{visibility}`; `public` refused while the licence is `circle-only` |
| `DELETE /items/:id`, `POST /items/:id/undelete` | author | hidden at once, undo for 14 days, then purged |
| `GET /items?scope=&subject=&kind=&tag=&standard=&license=&q=&cursor=&limit=` | teacher | newest created first (a republish never moves an item across a cursor), subject and `q` case-insensitive in every script (stored NFKC + lower-case folds): (round 12) every word of `q` starts a word of the title, the subject, a tag or the question text **as served to non-authors** (`served_words`, built from the stripped form when the key is hidden and keyed on the public publish; the full form is never indexed). A `q` or `tag` walk starts from its index (the longest query word; at most 5,000 candidate publishes, `SEARCH_CANDIDATES`), never from the whole pool; `license=` filters by licence; each result carries `license_check`; signed cursor, max 50, no totals |
| `GET /items/mine` | teacher | own items with state (`live`, `held_for_review`, `needs_reupload`, `deleted`, `taken_down`, `removed_privacy` (listed with a notice and `appeal` for 14 days), ...) and storage used; a takedown's `counter_notice` is `available` (with `counter_notice_by`) only inside its 30-day window (round 28) |
| `POST /me/invite` | pending or teacher | `{code}`: redeem an admin's invite code (round 28); every other pool route but `GET /me/export` answers a teacher without one `403 invite_required` |
| `GET /items/:id`, `HEAD /items/:id` | visible | served form + per-question provenance (`served_sha256`, `match_hash`, `chain`), streamed, with `Content-Length` and an `ETag` (a hash of the head, which names only the public publish id: a hidden-only republish does not change it); `If-None-Match` gives 304; HEAD and 304 record nothing |
| `POST /items/:id/import` | visible | same, recorded (author: own full copy, not recorded) |
| `POST /items/versions` | teacher | `{ids}` (<=200) -> `current` / `newer` (with `license` and `license_version`, which moves on any licence change) / `under_review` (student data within its 14-day grace: keep the copy) / `removed` / `unavailable` |
| `PUT /blobs/:sha256` | teacher | image upload (PNG/JPEG/GIF/WebP still, <=2 MB, <=4096 px); every refusal is the same `422 image_refused {reason: "not_accepted"}` (a privacy-blocked image is indistinguishable from a malformed one; the app explains format problems with the same checks before uploading) |
| `GET /items/:id/blobs/:sha256` | visible | only blobs listed under the caller's served form of the current publish |
| `POST /items/:id/report` | teacher (not the author) | `{reason, note}` -> always "thanks" |
| `POST /items/:id/counter-notice` | author of a taken-down item | 512(g)(3) fields; refused after the 30-day quarantine window |
| `POST /items/:id/appeal` | author of an admin-removed, held, or privacy-purged (14 days) item | `{note}`: raises an admin flag |

Admin routes (behind the admin guard; ids in the body or query, because the guard matches exact
paths): `GET /admin/pool/reports`, `GET /admin/pool/flags`, `GET /admin/pool/item?id=`,
`POST /admin/pool/hold|release|dismiss|remove|unremove|purge-preview|purge|privacy-restore|purge-held-copies|appeal-decision|unblock|evidence|resolve-flag`, `PUT /admin/pool/limits`, `GET /admin/dmca/notices|actions|counter-notices`,
`POST /admin/dmca/notices|takedown-preview|takedown|court-action|resolve|hold-copies-preview|hold-copies|release-copies|unblock|exempt|withdraw`, `GET /admin/dmca/copies?action_id=` (`resolve {action_id, outcome:
"restore"|"remove"}` ends a court action), and (round 28) `GET|POST /admin/pool/invites`,
`POST /admin/pool/invites/revoke`.

- **Confirms:** purge, takedown and hold-copies quote the `confirm_preview` token of their preview. A
  token binds the admin, the selection, the affected items and the action's parameters (`notice_id`,
  `block_days`, `exempt_authors`), lasts 10 minutes and works once. A privacy purge whose affected
  items exceed the listed 500 (`affected_total`, `large: true`) needs `confirm_large: true`; the rest
  are held in batches and an unfinished batch is a `privacy_backlog` flag the daily cron completes.
- **Release** (`{item_id, reason}`) lifts only holds people placed (reports, a manual hold, holds
  carried to an evasion copy). Privacy, DMCA and needs_reupload holds go with their own routes
  (unblock, withdraw/put-back, release-copies, a republish); lifting one here needs its exact
  `source` and `force: true` and is audited as `item.release_forced`. The answer lists what remains.
- **Takedown** `exempt_authors` (and `POST /admin/dmca/exempt {action_id, user_id, action: add|remove}`):
  the rightful authors, whose publishes the action's re-upload blocks never hold (adding one releases
  the action's holds on their items). Re-upload blocks also never hold what an item already carried
  before the block (a pre-existing copy is a hold-copies decision). hold-copies drops designated
  questions that K or more authors without lineage to the item use, unless `include_common`.
- **Strikes** count only once upheld (no counter-notice by the end of the quarantine, or a court
  decision for the complainant); the repeat-infringer flag counts upheld strikes only.

Errors: `pending`/`unverified` accounts get `403 account_not_approved` with a message; `quota_exceeded`
and `shared_questions_limit` carry `used`, `needed`, `limit`; a refused payload does not use the day's
publish allowance; a purged item answers `409 item_removed {purged: true}` (publish it as new).

### Migration 0104 (run once)

D1 records an applied migration in `d1_migrations` and never runs it twice. Applied by hand a second
time, 0104 stops at its first statement (`table pool_migration_0104_already_applied already exists`)
before changing anything. Before applying it to a database with data:

1. Export it: `wrangler d1 export <db> --remote --output pre-0104.sql` (and note the time: D1 Time Travel
   can restore to any minute of the last 30 days: `wrangler d1 time-travel restore <db> --timestamp=...`).
2. On a staging copy, count the evidence voids 0104 must carry:
   `SELECT COUNT(*) FROM import_questions WHERE void_at IS NOT NULL;` and
   `SELECT COUNT(*) FROM pool_audit WHERE action = 'evidence.void';`
3. Apply, then check the voids arrived: `SELECT COUNT(*) FROM served WHERE void_at IS NOT NULL;` and
   `SELECT COUNT(*) FROM void_items;` (an item whose last evidence audit is a void is in void_items and
   every serve of it is void; a reader void voids the reader's serves; serves whose old rows were all
   void stay void). Zero voided serves when step 2 found voids means stop and restore.

4. (Round 22) 0104 also creates `content_licences` (seeded from the retained publishes, questions
   only), `ghost_bands` and `erasure_done` (every account already deleted counts as erased), adds the
   keyed-band and unit-seq columns (filled for new publishes; older rows: ghosts of them match exactly
   only, their texts dated by the question), and replaces the audit trigger so erasure may null notes
   of audit rows about the account's items. After it, the daily cron records the sketch key id.

Rollback: restore the export or Time Travel; 0104 has no down migration.

### Limits (defaults; an admin changes them with `PUT /admin/pool/limits`)

| Limit | Default |
|---|---|
| Publish body | 4 MB payload + 1 MB meta (`maxPublishBytes` in the pool deps); publishes are parsed one at a time per isolate (4 more may wait at most 20 s, then 503 `busy`) and one at a time per account (409 `publish_in_progress`) |
| Memory per isolate | 72 MB committed to requests at once (`ISOLATE_BYTES_BUDGET`; round 9: 128 MB - 30 MB for the share-core wasm memory - about 20 MB of runtime baseline leaves 78 MB with no headroom, so 72 MB until a staging measurement on the final share-core build). The wasm memory is one instance shared by publishes and Argon2 login hashing and never shrinks: measured 20.7 MB after a 3.9 MB Mc bank, 21.4 MB after a 3.8 MB Matching bank, 20.4 MB after one Argon2 hash (a publish after it reuses that memory); the critic measured 29.4 MB on a Matching bank, so 30 MB is budgeted. A publish reserves 16x its body (1x as it arrives, 15x for the heavy phase; measured: about 62 MB of JS heap and buffers, wasm excluded, server-side for a 3.9 MB hidden-key bank) and holds it to the end; its evidence (8 MB a query) and fuzzy (up to 15.6 MB) lookups run after its payload trees are freed and first borrow the part of that reservation it no longer uses (round 29: the heavy 15x minus `postHeavyBytes`), reserving only the rest (a 3.81 MB Matching bank now peaks at 61 MB, was 76.6); a read reserves 1 MB plus 3x its JSON head; beyond it, 503 `busy` |
| Request bodies | One reader for every route: the pool's publish, upload and JSON routes, and every body the auth package reads (account routes and admin routes, pre-read in `src/pool/preread.ts`). `Content-Length` required (411) and exact (400 `length_mismatch`); bytes are reserved as they ARRIVE (a stalled body holds only what it sent); read within 15 s (408); at most 4 bodies and 8 MB at once per account (429 `too_many_uploads`). A request with a valid bearer token of a teacher or admin is authenticated first and read under its account: it is never refused because of unauthenticated load (if authentication itself fails, the body is read under the isolate budget, not the unauthenticated pools). Any other body (no token, a bad one, a pending or unverified account) is read under the unauthenticated budgets (round 9 K2): 2 MB in all, in three pools (sign-in: login, refresh, reauth, 1 MB, bodies at most 2 KB; sign-up 512 KB; everything else 1 MB), within 2 s (so 8 KB must arrive at that rate), at most 2 at once per IPv6 /64, 4 per /56, 8 per /48 and 16 per /32, or 4 per IPv4 address and 8 per IPv4 /24 (round 10). A client that goes away mid-body gets a clean 400 `body_aborted`. JSON bodies at most 64 KB, auth bodies 8 KB, an admin's bodies on admin routes 256 KB (round 10 M-8: read after authentication and, in the auth package, after its admin guard: a generic per-route `maxBodyBytes`), images 2 MB (the day's uploads and bytes are charged before the body is read). The auth package's own reader also stops at 8 KB without buffering more. A test fails on any direct request-body read outside the two readers |
| Streamed reads | Isolate-local caps (each isolate counts its own): 4 unfinished per account, 2 per account on one address, 8 per address (429 `too_many_open_reads`, "retry in a moment"); a stream nobody pulls from for 10 s is cancelled; at 60 s a stream still moving at 20 KB/s or more continues up to 300 s, a slower one is cut |
| Storage per account | 200 MB (`storage_bytes`); deleted-but-not-purged bytes have their own 200 MB (`removed_bytes`) |
| Shared questions per account | 5,000 (`shared_questions`), in-flight publishes included |
| Daily per account | 50 publishes, 300 refused publishes (reserved before the work), 100 imports, 2,000 item reads, 500 browse pages, 500 uploads and 200 MB, 100 id misses, 100 blob misses, 20 reports, 10 appeals, 10 invite-code tries (round 28) |
| Per IP | 6,000 requests / 5 min (20 account-shares: a school's teachers behind one NAT), counted only for requests that passed their account's own limit (300 / 5 min); 50,000 reads / day, at most 10,000 of them (20%) from one account; refused requests are given back |
| Reports | 3 distinct teachers hold an item; a student-data report holds at once, at most 3 a day per reporter, 50 a day site-wide (`instant_holds_global`), never from accounts younger than 7 days nor from accounts with no pool history (an import or a published item); the admin's report list shows each reporter's history (reports, dismissed, imports, items, since) |
| Blocks | privacy and DMCA blocks cover only the questions and images the admin designated after a preview. A privacy block selects NOTHING by default (round 10): the admin ticks what holds student data (400 `selection_required` otherwise); the preview marks `suggested` what no other author has on their own (any item, any state; copies that trace to the item do not count); a DMCA block skips a question or image used by 3+ other authors (`block_common_k`); either can be selected explicitly; DMCA blocks expire after `block_days` (default 60); both can be lifted (`/admin/pool/unblock` with `hash` or `sha256`, `/admin/dmca/unblock`) |
| Block keys (round 12 M4/M6) | a block names a question by KEYS (`shared.ts` `QKEY_COLS`): its exact match_hash and its normalised skeleton (`n:`; case, punctuation incl. quotes and dashes, whitespace), its normalised full content (`f:`), explanation (`e:`) or model / accepted answers (`a:`). Per question the admin picks a scope (`scopes: {pos: "stem" \| "full" \| "explanation" \| "answer"}`); the default is `stem`, except a PRIVACY block of a Tf, Short or Blank question (whose skeleton is the bare stem), which defaults to `full`. The preview lists, per selected question, its `keys`, `scope`, `stem_wide` (a stem block of a stem-only kind: it holds every question with that stem, whatever its answer) and the first 20 items it alone would hold (`affects`). Unblock by a key or by the question's match_hash (every key of it goes). `POST /admin/pool/release-hold {item_id, source, note}` releases one hold of one item (any source, a reason of 10+ characters required, audited `item.release_hold`) and records the keys it used then (`hold_releases`): its later publishes are not held again by those keys. A near-rewording (stem token Jaccard >= 0.8, MinHash bands of the blocked stems in `privacy_bands`) of confirmed student-data text is flagged for the admin (`privacy_text_match` with `near`), never held |
| Admin flags | one per kind per publish (a count and the first 5). Per account, kind and day (round 9 K3): 20 rows, then the kind folds into ONE aggregated row with a count (never dropped); integrity kinds (independence claims, licence flags, blocklist, privacy matches, origin suspicions) are never folded, and `origin_unverified` always folds; no burst of one kind can silence another. Publishes that end up held by a content block: at most 20 per account a day (429 `daily_limit` `held_publishes`); the queue (`GET /admin/pool/flags?limit&cursor`) is most severe first, then newest, paged; student-data report mails have their own hourly budget (20), re-publishes of blocked text another (5), everything else another (5) |
| Tags | 200 distinct tags and topics per item (more is an error) |
| Images | PNG, JPEG, GIF, WebP; one still image, 1..4096 px a side, 2 MB; an import holds the images it served for the importer for 30 days |
| Circles (rounds 32-35) | per account: 5 owned (an accepted transfer counts, a closed circle counts for its 14 days; a succession does not), 10 ownership offers a day, 20 memberships, 100 membership rows former ones included (`circle_memberships_total`: they live until their circle is purged; the bound of the publish check), 5 created / 50 codes / 10 failed codes / 100 versions sent a day; per circle (site-wide values): 50 members, 10 open codes, 100 banks, 200 MB (kept versions, images per version), 5,000 questions across current versions, 50,000 question records; codes 1-50 uses, 1-30 days (7); 20 versions kept a bank |

**The licence rule (rounds 9 and 10).** Critics pulled opposite ways (never lock an honest teacher
out; never let a copy carry a wrong public badge; more imports must never widen what is allowed).
One rule serves them; `test/pool/licence-cases.test.ts` holds every repro since round 7 with its
expected result, a bank-size and order property, a monotonicity property and a version-history
property.

* *Hard* sources (they can refuse, naming the items in `bound_by`): a source the subject is the same
  question as in the licence sense (`nearSql`), whether READ or imported; an import the subject
  adapted (also one found by a near-rewording of its stem, below); an origin declared now. *Soft*: a
  changed copy of something only read (and the server's own record of such a source at a
  republish): credited, flagged `license_soft` when its terms disagree, never a refusal.
* "The same question" (round 10 M-3): the content with points, difficulty and topics left out, text
  normalised (compatibility forms, case, punctuation and spacing, NBSP included), a dropped
  explanation and a key added to a stripped serve not counting as changes (round 11: an explanation
  of one's own added to it does, for a publish that shows it; see Known limits). A stem reworded by a word or two (token Jaccard >= 0.8 against what the
  account was served, found through MinHash bands) is an ADAPTATION of that source (hard for an
  import, soft for a read) and flagged `origin_suspect`. Determined rewording defeats any exact
  system; the admin flags and takedowns are the control.
* What one source allows: its own licences (for a copy, any of those shown for the copied content;
  for an adaptation, any for each content it served: identical content across versions of one item
  is ONE material, its licences united, round 10 M-4), narrowed by its chain; if those contradict, the
  older terms (the chain) win.
* A COPY (round 10 M-1): the earliest-published identical source is the original and decides; later
  identical relabels neither widen nor narrow it (a CC0 original beside an NC-SA relabel still allows
  CC0; five identical sources under five licences: the first one's). Adaptations beside a copy do not
  narrow it.
* An ADAPTATION: materials (sources that served the same content) in publication order, earliest
  first; each narrows what the earlier ones allow, one that would leave nothing is skipped (flag
  `license_conflict`). So a later-published import or read never widens the set, the outcome never
  depends on import order, and a contradiction never locks anyone out (the earliest material's terms
  bound it). Nothing at all only for circle-only material.
* A republish never refuses a question its author did not touch (round 9 J2): same qid, same
  content (key-free whenever either version hides its key) and the same licence keep the outcome they
  had; the admin is told (`license_grandfathered`). A touched question is checked.
* "Published first" is decided on CONTENT (round 12 M1), never on a stem or match_hash: a source
  counts against a publisher unless the publisher served THAT content (licence sense: stem, choices,
  shown key, explanation, model answer; normalised) before the source did (`author_contents` kind
  `nshown`), or its key-free form, except when the key is itself a work (a Short's model answer).
  Publishing a bare stem early ("What is photosynthesis?") therefore makes no one first for a richer
  later question; that question's added material stays protected (a verbatim copy, a points or
  difficulty change, a hidden-key copy, an adaptation of an import all bind). For fuzzy sources and
  the `origin_suspect` flag only, the original author is also first through (b) the qid's own
  lineage (`item_questions.lineage_seq`, carried while each edit changes ONLY the stem: the same body,
  `body_hash`, and the same question or a stem with token Jaccard >= 0.8) and (c) their own retained
  questions with the same body and a near-reworded stem (their own MinHash bands, user `own:<id>`).
  So an original author's case edit, punctuation fix, reword or unchanged republish never credits,
  binds or flags against a later copier, in the same item or a new one; a stem-only question turned
  into a rich one starts a new lineage. A translation is a different question (no match, no flag).
* The original author (round 10 M-5): a later source counts against its author only for what that
  source ADDED; it never binds an author who published the question first when the source added
  only the key (key-free comparison, so no hidden key decides it). The author's own contents are
  durable (`author_contents`: shown full contents and key-free contents, never removed by retention,
  item deletion or purge; on ACCOUNT deletion they are kept under the account's anonymous id, so they
  no longer name it), so a purged or retained-away original stays the author's own. Taking a
  copier's explanation still binds (round 12: also when only points or difficulty were changed),
  unless the author served that very content first.
* `independent_of {items, accepted, statement}` (round 12 M5: reads and imports alike) is the way out
  for common questions, short statements, CHANGED copies (an adaptation of an import, a reworded
  read), and the original author's own question; it never covers an exact copy (licence sense) of
  anything the account was served, read or imported: such a publish is refused naming the source in
  `bound_by`. A statement is required, audited and flagged; a near-rewording stays flagged
  (`origin_suspect`).
* "Common" (rounds 10 M-2, 11 P7, 12 M2): for a given serve, K (3) or more INDEPENDENT roots (the
  root of each copy's lineage, so honest copies of one source never count) other than the source's
  own that served the SAME content (licence sense, as above) before the source's first publish of it.
  Three teachers with their own answers to a generic stem never make a fourth teacher's model answer
  and explanation common; sock puppets publishing it after the source never count; a question three
  teachers had, in that form, before it is common.
* Short statements (rounds 10 M-5, 11 P6) are treated as common: a read is not evidence and an import
  may be attested independent. Decided on the SERVED form (so a hidden key decides nothing): no
  explanation, image, figure or keyless marker, and Tf: a stem of at most 8 words; Short: a stem of at
  most 8 words AND a served sample of at most 12 words; Blank: a stem of at most 8 words AND served
  accepted answers of at most 12 words in all; Mc: the stem and all choices together at most 12 words;
  Matching and Multipart: never; a hidden-key Short or Blank (its key not served): never. "Define
  osmosis in your own words." with a 40-word model answer is NOT short.

**Monotonicity: what can and cannot be promised.** "Adding any import never enlarges the allowed set"
cannot hold together with "a contradiction never locks anyone out" and "the outcome does not depend on
import order": with two sources X and Y that allow disjoint sets, monotonicity would force
f({X,Y}) to lie in f({X}) and in f({Y}), which is empty. The rule therefore guarantees: (1) adding a
source published LATER than every source already present never widens the set; (2) no licence is
ever allowed that no present source grants by itself; (3) adding the source the subject is a COPY of,
or one published earlier than all present, can change the set to that original's terms.

Before anything is ranked or limited, SQL drops every source that cannot count: the publisher's own
items and sources derived from them, sources first published after the publisher's own first publish
of the question (unless the publisher copies their additions verbatim), void items and lineage
through them, items the publisher attests independence of, and reads of common questions. Sources
are counted per ITEM (a bank repeating a question counts once), at most 32 per question whatever the
size of the bank, ranked by origin, sameness and age, never by import order: a question's outcome is
the same in a bank of 1 or 1,000 and in any import order (a property test). Past the allowance
nothing restricts (flag `evidence_overflow`).

**Credits (rounds 9 J6, 10 M-7).** A question's credits are merged in SQL from the chains of the
sources a chain follows (the identical ones if any, else all), so none drops out of a big bank. The
stored credit list keeps up to 64 contributors (`CREDITS_MAX`): past it, the first 8 (the
originators) and the most recent ones, with one placeholder "and N contributors between" carrying
the licences of those left out; the direct source is always present; downstream copies inherit the
list. A read shows the first 8, one placeholder "and N contributors between" and the most recent 24
(round 11); N always counts the contributors it stands for, a placeholder inherited from a source
counting its own number (so a stored "and 10" is never shown as "and 2").

**Reader-visible licence check (rounds 10 M-6, 11 P5).** Every read, import and browse result carries
`license_check: {status: "ok" | "mismatch", basis: "recorded_sources", positions?, reason?}`: whether
the item's licence is **consistent with the recorded sources**. Each question stores, at publish, the
licences the licence rule allowed and the items that decided it (`item_questions.allowed_json`); the
check reads that record, so it can never disagree with what the rule let the author publish. The one
change since publish: a deciding source later voided and tied to a rightful item narrows the set to
what the rightful item's licence allows (verbatim or modified as the chain records). Questions
published before the record existed fall back to their displayed chain. A held copy is released at an
upheld takedown only if this check is `ok` (recomputed after the void); otherwise it stays held and
the admin is flagged (`license_mismatch`). A relabel the rule allowed is never accused publicly; the
admin flags (`license_soft`, `license_conflict`, `independent_claim`) carry suspicion.

Evidence is stored as one durable row per (source item, qid, served content): repeated reads write
nothing, new licences and versions are added to the row, and the earliest per-content sequence number
is kept. A publish aggregates it in SQL; only up to 32 source items per question reach the Worker,
with their chain licences reduced to a set in SQL.
Sequence numbers come from one counter (`pool_seq`), never reused; an author's first publish of a
question is kept in `author_firsts`, which retention and purge never lower. The own-work rule sees
only the author's SHOWN content: a hidden key of the author's other items never decides anything
others see. A licence refusal names, per position, the items that bind it (`bound_by`: id, title,
author); an independence claim (`independent_of {items, accepted, statement}`) needs the author's
own statement, is audited and flagged, and the third claim in 30 days flags the account
(`independent_repeat`). An upheld takedown may name the rightful author's item (`rightful_item_id`):
links to the infringing item then stand for it (its licence at void time and its author, whose name
is resolved at read; not modified when the copy was verbatim; a neutral "the rightful author" with no
id when that item is not visible to others); without one, copies of the infringing item are held for
review (flag `dmca_copy`). hold-copies never holds an exempt author, the rightful item or its
author's items. The rightful item (round 11 P4; the same check for `/admin/pool/evidence`
`replaced_by`) must be another author's live item, visible to others, not derived from the item taken
down (no chain or origin naming it; its author was not served it BEFORE the rightful item's first
publish, round 12 M3: the victim opening the infringing copy later, which is how she found it, does
not count), and contain EVERY blocked hash
(question and image) in a publish committed before the taken-down item's first publish containing it
(per-content sequence numbers, never the item's age). The takedown preview shows `rightful_check`
(per blocked hash, the earliest sequence in each item); a confirm that fails it is 409
`rightful_item_invalid` with `reasons` (`same_author`, `rightful_not_visible`,
`rightful_derived_from_item`, `not_earlier_in_rightful`, `no_shared_material`) and the failing
hashes. The takedown then selects the infringing copies of its material by default. Its exemption (rounds 9 K1, 10 L2): a blocked hash the rightful item itself contains never
holds a publish by the rightful item's author, nor by someone who was served the rightful item (an
import or read record, an origin or chain naming it) while it is visible to others; anyone else
re-uploading the content is held, and nothing is exempted through an unlisted, removed or held
rightful item. When upheld, a hold of that action is lifted only if every blocked hash the held item
uses is exempt for its author in that sense (a lineage hold using none: if its questions come from
the rightful item) AND its licence check is `ok`. `bound_by` names only items visible to others.

Evidence for licences and attribution: a question the account imported or read counts only if it is not
the account's own work (nor derived from it) and was first published before the account first published
such a question; a verbatim copy takes its grants from the identical sources only. "First published" is
per content: a kept qid given new content (another `match_hash`) gets a new sequence number, and an
image counts from the first publish of that item that served it, so an old qid or item never fakes
having come first. A plain READ is evidence only for a question fewer than 3 other authors' visible
items use (`block_common_k`) or one declared as an origin; `independent_of {items, accepted: true}` on
a publish attests that the listed items had no part in it (their reads always; their imports when the
question is common or not a verbatim copy; audited, the statement kept in the audit note, and flagged
`independent_claim` for the admin). Material voided as evidence (an upheld takedown, or
`/admin/pool/evidence`) is dropped everywhere: links to it are not shown and grant nothing, a kept
question's provenance through it is cleared at the author's next republish, and a source whose lineage
runs through it binds only a verbatim copy of its own additions. Chains hold one link per item (the
earliest version, `modified` if any path modified it) and never the publisher's own items. A copy that
shows its key is compared with what its source served: adding only the key to a stripped serve is no
change (a copy), adding one's own explanation is (an adaptation, round 11). A mixed-licence bank
that cannot be published gets `per_question` in its 422 (split the bank, or pick a licence they all allow;
there is no per-question licence). An author may choose (`PUT /items/prefs {keep_on_delete}`) to keep
their shared items, shown as by "deleted teacher", when they delete their account, or not; with no
choice the items others imported are kept and the rest removed and purged after 14 days.

### What a hidden key can and cannot influence

Everything a non-author can observe that is derived from comparing questions (which sources a chain
credits, `modified`, the licences a copy may carry, `may_republish_as`, whether a source counts as
evidence) is computed in one place, `src/pool/visible.ts`. For a publish that hides its key, that code
is given only key-free hashes; it has nothing key-bearing to compare. A property test publishes key
variants of a hidden question against public sources with different keys and licences and checks that
every non-author-visible field is identical.

Images follow the same rule (`src/pool/served.ts`): every hold, block, unblock, needs_reupload list,
orphan release, unremove re-hold, block plan and affected list looks only at the images an item SERVES
(the stripped form, i.e. the ticked images, of a hidden publish; the full form otherwise). An image
only a hidden publish's full form carries changes nothing anyone else can see. When a privacy block's
grace window ends, the image is gone for every served use (until uploaded again after an unblock); its
bytes are kept only while an author still uses them unserved, and only that author can still read or
republish that use. A property test runs random admin and attacker sequences (reports, purges,
unblocks, removals, releases, takedowns, the cron) with the victim's full-only image equal to the
attacker's, different, or absent, and checks that everything a non-author observes is identical.

### Known limits (by design, documented)

- **Rewording:** trivial edits stay copies and a stem reworded by a word or two is an adaptation
  (round 10, above), but a determined rewording (a different stem, different choices) defeats any
  exact or fuzzy comparison. The `origin_suspect` flag, reports and takedowns are the control; the
  publisher's attestation and the ToS carry the responsibility. No claim otherwise is made.
- **Over-attribution:** when several sources are identical (key-free for a hidden publish), all of them
  are credited, oldest first, rather than guessing which one was copied.
- **Topics vs tags:** a question's `topics` are merged into the item's searchable tags (lower-cased,
  de-duplicated); there is no separate topic filter.
- **Privacy purges** hide the item at once and quarantine it for 14 days (its author sees
  `removed_privacy` and may appeal; importers' `versions` answer is `under_review`, so clients keep
  their copy). Unblocking its last block, or `POST /admin/pool/privacy-restore` (an upheld appeal),
  within that window puts the same item back (id, versions, chains). An OPEN APPEAL stops the purge
  (round 9 J5): the cron neither purges the item nor its images' bytes until the admin decides
  (`POST /admin/pool/appeal-decision {item_id, decision: restore | purge}`, also for a held copy:
  restore releases its holds, purge removes it like the original); the admin is reminded weekly
  (`appeal_overdue`: ONE flag per open appeal, its week count and date kept current, round 11). Round
  10 L1 / round 11 P8: a privacy removal, any `student_data` hold (whatever placed it: a report, a
  purge, a publish) or a DMCA hold is NEVER restored by default; past 60 days its appeal is escalated (`appeal_escalated`, the most urgent flag) and the
  item stays quarantined with its bytes until an admin decides; restoring it is always an explicit
  admin action. Only other removals and holds (an admin's policy removal, reports) are restored after
  60 unanswered days, flagged `appeal_auto_restored`. One appeal per removal or hold (never one per
  UTC day); once the admin has decided, that removal cannot be appealed again (409 `decided`).
  After the purge, the cron removes the item and the images' bytes; copies are held
  (`needs_reupload` for images) and `versions` says `under_review` for them until they are purged.
  The copies a confirmed purge holds can be listed and removed in bulk
  (`POST /admin/pool/purge-held-copies {item_id, confirm}`), each quarantined for the grace window.
  A purge whose R2 delete fails keeps the keys (`r2_orphans`) for the hourly retry; one whose
  database step fails waits 1, 2, 4 ... days (at most 7) and is flagged; the oldest due go first.
- **Student-data text is held at publish** (round 8, reversing round 7): the hold tells the publisher
  only that text they hold verbatim and are publishing is confirmed student data; their item shows
  the same "held for review" as any other hold, with no detail. One `privacy_text_match` flag per
  publish goes to the admin, on its own mail budget. A republish whose content no longer uses a
  privacy block lifts the hold its earlier content caused.
- **Held items in `versions`** (round 12): a held item answers `under_review` with reason `held` to
  everyone, its author included, whatever the hold: privacy, copyright and report holds look alike
  through another account (no kind oracle), and the author's held item is never `current`.
- **Reversible states:** `versions` answers `under_review` (with `until` when a date applies) for an
  author deletion within its undo window, an admin removal, a takedown in quarantine, student data
  within its grace window or under appeal, and a held item whatever the hold (round 9 J4); `removed`
  only once purged. A late counter-notice gets
  409 `window_closed`.
- **Account deletion:** free text the account wrote goes with it (independence statements and appeal
  notes in flags, report notes; audit notes are nulled), and (round 9) the name, address, phone,
  signature and description of its counter-notices (ids and dates stay), its name in others' stored
  chains and evidence rows, and its preferences. Audit rows keep the account's opaque id as actor
  (security evidence; the account row itself holds no personal data any more). With no choice made
  (`GET /items/prefs` answers `keep_on_delete: null`), the items other teachers imported are kept,
  anonymised, and the rest are deleted; an explicit choice wins.
- **Display text vs search:** titles, subjects, courses, grades and tags show the author's own text
  (NFC; round 9). share-core's metadata cleaner folds NFKC (x² to x2, ½ to 1⁄2); the Worker keeps the
  author's NFC form whenever share-core changed nothing but that fold, and folds only the search
  column: compatibility forms, accents optional, lower case, ß = ss, œ = oe, æ = ae, ü = ue as well as
  u, punctuation and hyphens as spaces, and hyphenated words joined too (co-operative = cooperative).
  `q` matches the start of any word of the title, the subject or a tag (round 10); `tag=` matches the
  tag folded the same way (case, accents, decomposed forms). `If-None-Match` uses the weak comparison.
  On a route whose body is optional, no body is no body: `Content-Length: 0` whatever the type, or a
  bare POST with neither a length nor a type (415/411 only when a body is really sent).
- **Held copies** (rounds 10, 11 P3): a hold an admin placed on an existing item (a confirmed
  student-data purge, a takedown's hold-copies) tells its author the class (`hold_class`) and what to
  do, and the question positions from a SNAPSHOT taken at the admin action (`item_holds.detail`), only
  while the item still has the content the admin saw: a republish is never re-evaluated against the
  blocks for the notice, so a held item cannot probe which texts are blocked. Every publish that ends up
  held (a new hold or a republish of an item still held, any hold) counts toward 20 a day
  (`usage_daily` `held_publishes`, 429 `daily_limit`). The author is told by mail (`copy_held`, ids
  only, its own hourly budget of 200). Round 12: the cap is charged PER QUESTION that matches a block
  (privacy or active copyright, by any key, plus near-rewordings of blocked student-data text),
  whatever the outcome (held, released, exempt). A copyright hold whose action's blocked material a republish no
  longer contains is cleared, flagged `hold_cleared`; one that still contains it stays. A hold placed
  by a publish's own content check keeps the neutral "held for review".
- **Admin records** (round 10): `GET /admin/dmca/notices`, `/admin/dmca/actions` and
  `/admin/dmca/counter-notices` (newest first, `?limit&cursor`); a counter-notice raises a
  `counter_notice` flag; `GET /admin/pool/item` lists the item's DMCA actions and hold sources.
- **Hidden republish** (round 10 M-9): a republish that changes nothing served (a key or an
  explanation of a hidden item) keeps the stripped body and public version: unlocked choices are
  compared as a multiset (strip shuffles them), locked and acknowledged orders exactly.
- **Account deletion, more** (rounds 10, 11): its import records stay counted under ONE anonymous id
  per deleted account (so import counts do not inflate); its report audits lose the actor and report
  id. Its name in others' stored chains is rewritten by the cron in bounded batches
  (`deletion_rewrites`, 500 rows by rowid per statement, 20 statements per run, resumed from a
  cursor), never thousands of statements in the deletion request; reads already show "deleted
  teacher" from the account's state meanwhile. Round 12: every chain is stored through `anonChain`
  (a link to a deleted account's item gets "deleted teacher"), so no read or publish during or after
  the rewrite stores the name again.
- **Alert budgets** (round 12): one hourly budget per notice kind (student-data reports 20, held-copy
  notices 200, every other kind 5 each), so a flood of one kind never silences another. The
  student-data match mail says whether the item was held (an exact or normalised match) or is live
  (a near-rewording).
- **Retention** (round 12 A1): a publish is kept while it is current, the previous one, or the one
  non-authors are served (`public_publish_id`): hidden-only republishes never retire the served
  publish, its search words or its rows.
- **Memory:** publish bodies must declare a Content-Length and match it, are read within 15 s before
  any heavy work, and reads stream (the admin item view too); one isolate commits at most 72 MB to
  requests (503 `busy` beyond). Large D1 reads reserve their worst case first (round 11): the
  evidence query 8 MB per query (31 positions at a time), the fuzzy lookup 8 KB per candidate pair (at
  most 16 MB); in a publish they first borrow what its heavy reservation no longer covers (round 29).
  Object-dense banks (26-choice Mc, 4x8 Multipart, 50-pair Matching, 1,000 questions)
  peak at 13-28 MB of live JS heap against 23-32 MB reserved (critic eK, node, `--expose-gc`).
  Measured in node only (workerd does not report isolate memory).
- **Fuzzy lookup bounds** (round 11 P2): one aggregated SQL statement per publish; a MinHash band
  shared by more than 32 served stems (a stock phrase) is skipped; at most 8 candidate (item, qid)
  pairs per question and 2,000 in all; at most 20,000 evidence rows examined, aggregated per pair in
  SQL (so a source republished many times returns one row). Past a cap the publish is flagged
  `evidence_overflow` (conservative; nothing is refused). Measured (node, loaded machine): 1,000
  questions x 50 contents each, fuzzy 0.3-0.5 s and 400 rows; the same bank where every stem is a
  candidate, 0.3 s, capped and flagged. Evidence rows are deduplicated at insert (one per served
  content), so re-reading never grows them.
- **Common questions:** a question 3+ INDEPENDENT roots published before the source is not evidence
  when only read (round 11 P7); puppets publishing it later never count. Puppets who published it
  BEFORE the source would (approval is the guard there).
- **Hidden-key sources** (round 11): adding the key to a stripped serve is still a copy (the stem and
  choices are verbatim); adding the key AND one's own explanation is an adaptation. Only for a
  publish that SHOWS its explanation: a hidden publish compares key-free and explanation-free, so the
  show/hide choice can change the outcome in exactly this case (the explanation is hidden material).
- **circle-only adaptations:** the licence table does not let an importer adapt CC BY or BY-NC
  material into a `circle-only` (all rights reserved) item. CC 4.0 would allow an adapter's own licence
  on their additions, but the spec keeps circle-only for the author's own material; phase 3 decides.

### Which queries may read key-bearing columns (round 7 audit)

`full_content`, `full_sha256` and a non-stripped `content_hash` carry the key; `hide_key` decides who
may compare them. Every read of them in `src/pool/`, and why it is allowed:

| Where | Reads | Why it cannot reach a non-author's view of a hidden key |
|---|---|---|
| `visible.ts` `sameSql` | `import_questions.content_hash` vs the subject's `full` | The subject's `full` is NULL for a hidden publish (`subjectFor`), so only `keyfree_hash` is compared |
| `provenance.ts` `evidence` | `import_questions` (what the account was served) | Served content only: the stripped content of a hidden source; compared only through `sameSql` |
| `provenance.ts` `authorContext` | the publisher's own `full_content` with `hide_key` | `ownShown` drops every hidden publish's content (H1); the type `OwnShown` can only be built by it |
| `read.ts` `questionsFor` | `full_sha256` | Sent only for a full serve (author, or a publish that shows its key); a stripped serve gets `stripped_sha256` |
| `read.ts` `mayRepublishAs` | served form's content (`stripped_content` for a stripped serve) | The Subject of a verbatim copy of what was served: hidden when the serve was stripped |
| `read.ts` `recordServed` | one `served` row per publish read (round 19); `import_questions` is a view over the served publish's own rows | What this account was served, nothing more |
| `read.ts` `getBlob` | `hide_key` | The served-form rule (served.ts); an unserved image only for its own author |
| `publish.ts` previous-publish carry | the item's own `full_content` / `keyfree_content` | Compared key-free whenever either version hides its key; result is the author's own chain flag |
| `publish.ts` hidden republish reuse | the item's own `stripped_content` | Served bytes only |
| `publish.ts` image gate | `hide_key` | Served-ness of the publisher's own images (served.ts) |
| `served.ts` | `hide_key` | Decides the served form; never compares content |
| `admin.ts` `/admin/pool/item` | `hide_key`, the full body | Admin only |

### Client requirements (for the Rust client piece)

The current `src/share` transport cannot talk to these routes as they are: it uses `ureq` without
`PUT`/`PATCH` (republish is `PUT /items/:id`, visibility is `PATCH`), forces `Content-Type:
application/json` (a publish is `application/vnd.vershion.publish` with an exact `Content-Length`), and
reads at most 64 KB natively (an item GET or import is up to ~5 MB, streamed). The client piece must
add those methods, send the publish content type and length, stream large bodies, honour `ETag` /
`If-None-Match` and `Retry-After` (409 `publish_in_progress`, 429, 503 `busy`), and finish or cancel
each download (at most 4 open per account).

The hourly cron runs the pool GC (abandoned staged publishes, expired pending uploads, unreferenced
blobs); the daily one runs DMCA put-backs, quarantine expiry and purges.

## Tests

```sh
npm test            # node --test: mail, alerts, notice budget, make/demote-admin on SQLite, domain validation, migrations,
                    # the pool (test/pool: licence-cases.test.ts is the licence table, round*.test.ts the critics' repros)
npm run typecheck
```

The auth flows themselves are tested in `../auth` (`npm test` there).

**D1's LIKE/GLOB pattern limit (critic 58 B1).** D1 refuses a LIKE or GLOB pattern longer than 50 bytes
("LIKE or GLOB pattern too complex"), each time the operator is evaluated on a row, whether the pattern is a literal,
a bound parameter or built with `||`. node:sqlite allows 50000, so the account-deletion batch's
`channel LIKE 'circle_email_domain:%:' || ?1` (58 bytes with a 36-character id) passed every test and failed on D1
for every account (and in the cron's `erasure_finish` each day). That statement now compares
`substr(channel, 1, 20)` and the id after the hmac exactly, with no LIKE. The harness's `d1()` overrides `like()` and
`glob()` so a pattern over 50 bytes inside a D1 statement throws D1's error (the test's own `w.q` queries are not
limited), and `test/pool/d1-like-limit.test.ts` scans `src/` and `migrations/`: every LIKE/GLOB pattern must be one
SQL string literal of at most 50 bytes. Match a prefix of a value you bind with `substr(col, 1, n) = ?` or an
equality or range comparison, not `LIKE ? || '%'`.
