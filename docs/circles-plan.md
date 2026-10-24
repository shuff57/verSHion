# Circles: planning document

Written 2026-10-23. **Plan only: nothing here is built, migrated or decided beyond section 2.** It builds on
`docs/shared-pool.md` (the current spec), `server/worker/README.md`, `server/worker/src/pool/` (read for every claim
about existing code below), `server/auth`, `src/share/` and `docs/handoff-shared-pool.md`. Anything not checked in
code or not decided by the owner is marked UNCONFIRMED. Sizes and row counts are estimates until measured.

---

## 1. Summary

A **circle** is a small private group of approved teachers. Any approved teacher can create one and invite other
approved teachers (by code in this plan; email invites are open decision D5). Members see the circle's **banks**, take
private copies, and (in phase B) edit the same bank together.

Shared editing works like version control, not like a shared document. The server keeps **one official version** of
each circle bank and a short history of earlier versions. Each member edits a copy on their own device and, when
ready, **sends** it as the next version. If someone else sent a version first, the server refuses and the member
must **press "Get latest"**, go through the existing three-way merge review (`src/share/merge.rs`: base, yours, theirs;
conflicts decided per question; restore point first), and send again. Nothing is merged automatically and nobody's
version is overwritten without them having seen it in a merge. There is no live co-editing.

Circles live entirely in the Worker's pool database (new tables, migration `0108`), keyed by the auth package's user
ids. Nothing goes into the auth package or its `users` table. Circle banks are stored apart from public items, so the
public pool's visibility, search and licence code (hardened over 31 review rounds) does not change, except for one
added check at public publish time that stops circle material being re-shared without the author's permission
(section 7).

Delivery in three phases: **A** circles, invites, sharing a bank into a circle, view and copy; **B** shared editing
with versions, initiated merge and rollback; **C** polish (email invites, hiding keys from viewers, re-share path,
copyright-takedown machinery for circle banks). The five biggest risks are in section 14.

---

## 2. Decisions made (owner, via multiple choice; they stand)

1. **Membership.** Any approved teacher (trust `teacher`, and past the pool's invite-code gate) can create a circle
   and invite other approved teachers by email or link/code. No school tie.
2. **Permissions.** Shared editing of the same bank by several members, beyond view and import. The owner, on
   updates: "I want to be like a version control, merges have to be initiated." So: build on the existing
   user-initiated three-way merge (base snapshot + ours + theirs, restore point), not live co-editing.
3. **Owner leaves or deletes their account.** Ownership passes to the longest-standing editor. If there is no other
   member, the circle closes and its items are removed.
4. **Licence.** Members may re-share a circle item publicly only if the item's author allows it (a per-item flag).
5. **Storage.** `circles`, `circle_members` (role `owner | editor | viewer`) and `circle_invites` tables in the Worker
   pool database, keyed by user id, separate from `users`.

Earlier owner decisions that still bind circles (`docs/shared-pool.md` section 10): no circles at the *first*
launch; 200 MB and 5,000 shared questions per teacher; 3 upheld strikes in 12 months is a flag, not a suspension;
"deleted teacher" on others' copies; audit and takedown records kept 3 years; lawyer review before any public launch.

---

## 3. Decisions needed

Each has a recommendation. None is decided.

| # | Question | Recommendation |
|---|---|---|
| D1 | **Moderation of private circle content**: what the operator can see, reports, DMCA path, strikes | Admins open circle content only when a member reports it, a copyright notice names it, or a student-data block matches it; every admin open is audited. Members can report (student data, copyright, inappropriate, other); one student-data report from an established member hides the bank from the circle at once, as in the public pool. DMCA: phase A/B handled by the admin by hand (remove, mail, counter-notice by hand); phase C extends the existing takedown machine to circle banks. A strike goes to the account the admin names (default: whoever sent the first version containing the material, from the history). The own-work attestation is ticked on every send, because private circles are where copied publisher test banks would hide. |
| D2 | **Circle size limits** | 50 members a circle; 5 circles owned and 20 memberships per account; 100 banks a circle; 1,000 questions a bank (share-core's existing limit); 10 open invite codes a circle. All admin-adjustable through the existing `PUT /admin/pool/limits`. |
| D3 | **Storage quota: whose 200 MB** | A separate quota per circle (200 MB: every kept version's body plus its images), not charged to any person. Charging the sender fills an editor's quota with others' work after a merge; charging the owner lets editors fill it. The 5-circles-owned cap bounds what one account can create. |
| D4 | **Do circle questions count toward the 5,000-question cap** | No. A per-circle cap instead: 5,000 questions across the circle's current versions. |
| D5 | **Invite by email or by code** | Code (and a pasteable link holding the code) only in phases A and B. Email invites later if wanted: sent by the server from a fixed text, the same answer whether or not the address has an account, 20 a day, only an HMAC of the address kept. A code needs no look-up of anyone's email (no account-exists oracle) and cannot be used to mail strangers. |
| D6 | **Removing a member; what removed members keep** | Only the owner removes. Removal is immediate. Removed members keep the copies already on their device (these cannot be recalled, as with public imports). Versions they sent stay in the history under their name. The re-share restriction still applies to them (section 7). Re-joining needs a new code. |
| D7 | **Deleting a circle** | Owner only. Members see "closed by the owner" at once; 14 days to undo, then the banks, versions and codes are purged (same timing as an item deletion). Members' local copies stay. An open hold, appeal or takedown delays the purge as it does for items. |
| D8 | **Audit trail** | Every circle action writes a `pool_audit` row (kept 3 years, then the existing `retention` stage deletes it). Members see an activity list inside the circle (joins, leaves, removals, role changes, transfers, versions sent, rollbacks), with names but no technical data. |
| D9 | **Privacy page, terms, lawyer** | Draft the page changes in phase A (section 11). Counsel reviews them, the contributor terms for shared editing and the moderation policy before circles are used by anyone outside the invite-only pilot. |
| D10 | **Rate limits** | Per account a day: 5 circles created, 50 invite codes, 10 failed join codes, 100 versions sent, 20 rollbacks. Circle reads count toward the existing `item_reads` (2,000 a day), so there is one read budget. Bursts use the existing per-account and per-address limits. |
| D11 | **Abuse (invite spam)** | With codes only, nobody can be added without typing a code, and nothing is mailed to strangers. Codes are single use by default, expire in 7 days (30 at most), are revocable, and the caps above apply. Circle names are plain text with a length cap. An admin can close a circle. Accounts that create and close many circles are flagged. |
| D12 | **Answer keys inside a circle (`hide_key`)** | Every member sees the full bank, keys included: editors cannot edit without them. The app says so plainly when a bank is shared into a circle. Optional per-bank "hide keys from viewers" in phase C, using share-core's existing strip and key-hint gate for the viewers' copy only. |
| D13 | **Provenance / near-copy system and circle items** | Circle content stays out of the public licence engine (no `served` rows, no authorship records, no licence history). One new exact-match check at public publish, for current and former members: a question passes if the publisher first sent it to the circle, if it is a short statement, or with an audited independence statement; otherwise it is refused. Members' re-sharing of CC-licensed circle banks waits for a phase C server route that credits the first sender (its own critic loop); allowing it earlier means the public pool credits the member instead. Public-pool material sent into a circle bank must carry a licence its origin allows. Details in section 7. |
| D14 | **Owner leaves and only viewers remain** (decision 3 names only editors) | The longest-standing viewer becomes owner. Closing would delete banks that the remaining people still use. |
| D15 | **Banks a deleted account created, in a circle that survives** | Kept, shown as by "deleted teacher", unless the person chose "do not keep my shared items" (`keep_on_delete` = no), in which case they are removed. Other members' edits are in those banks too. |
| D16 | **How much history** | The last 20 versions of each bank are kept in full; older versions keep only their history line (who, when, message, counts). Rollback and merge bases work only within the kept 20. |
| D17 | **Who may roll back or delete a bank** | The owner and the bank's author. Editors may send versions but not roll back, because a rollback takes others' later work out of the current version (it stays in the history). |
| D18 | **"Longest-standing"** | Counted from when the person joined the circle, not from when they became an editor. |
| D19 | **How decision 4's per-item flag is stored** | As the bank's licence: `circle-only` (the existing wire value, "All rights reserved") means "members may not re-share"; one of the five Creative Commons licences means "members may re-share under this licence". One field, so the flag and the licence can never disagree. The app shows it as "Members may share this publicly: No / Yes, under CC BY 4.0". |

---

## 4. How a shared circle bank works

### 4.1 The model

* **One official version on the server.** Each circle bank has a head version number (1, 2, 3 ...). Versions are
  immutable and linear: no branches.
* **Each member works on a local copy.** "Edit with the circle" makes a normal bank on the member's device, linked to
  the circle bank and to the version it came from (its **base**). The member edits it with the app's existing bank
  editor, offline if they like.
* **Sending a version.** "Send my changes" uploads the whole local bank with `base_version`. The server accepts it as
  the next version only if the head is still `base_version` (compare-and-set, the same pattern as a public republish's
  `base_publish_id`, which answers `409 conflict`). The person's base becomes the new version.
* **Someone sent first.** The server answers `409 conflict` with the current head, who sent it and when. Nothing is
  stored. The app says "The circle bank changed since you got it. Get the latest changes and merge them, then send
  again."
* **Merging is always started by the person.** "Get latest and merge..." fetches two versions from the server, the
  person's base and the head, and runs the existing three-way merge (`merge.rs`): BASE = their base version,
  OURS = their local bank, THEIRS = the head. The existing review categories apply per question: unchanged; changed
  only in the circle (take, the default); changed only by you (keep); **conflict** (keep mine / use the circle's /
  keep both, no default, Apply stays disabled until each is decided); new in the circle (add); removed from the
  circle (keep or remove); deleted by you (stays deleted). Apply writes the existing restore point
  (`backups/banks-before-merge-<date>.json`) and replaces the local bank once. Then the person sends again, based on
  the head they merged.
* **Nothing to send.** "Get latest" with no local changes is a fast-forward: the app shows what changed and replaces
  the copy after a confirm (restore point first).
* **Who may send.** Owner and editors. Viewers can read and copy only. In phase A, only a bank's author sends new
  versions of it (it is "share to circle" plus republish).
* **Who may overwrite.** Nobody silently: every accepted version was based on the head at that moment, so whatever
  it replaced, the sender saw in a merge. Two guards for mistakes: a version that drops more than half the bank's
  questions needs `confirm_large_change` (`428 confirm_large_required`, the code the admin purge already uses), and
  any kept version can be rolled back to.
* **Rollback** makes a *new* version equal to an older kept one (history is never rewritten). It is a send with the
  same compare-and-set on the head. Owner and bank author only (D17).
* **Question identity.** Every question in a circle bank has a server id (`qid`) that stays the same across versions,
  as in the public pool. The local copy remembers which local question is which `qid`. A question the member adds
  gets a new `qid` when the version is accepted; the answer returns them.
* **Merge base too old.** If the person's base version is no longer kept (D16), there is no BASE. The app then shows
  every question that differs between yours and the circle's as a conflict with no default (a two-way review), which
  is slower but never loses work. The app says why.

### 4.2 Deliberately not included

* **Real-time co-editing:** no live cursors, no presence, no typing into the same bank at once, no auto-save to the
  circle, no CRDT or operational transforms.
* **Automatic merging:** the server never merges, rebases or resolves anything. A merge happens only on the
  person's device, only when they press it.
* **Locks:** no per-question or per-bank checkout. Two people editing the same question produce a conflict for the
  second sender, decided by a person.
* **Branches:** history is one line.
* **Field- or word-level merge:** the review shows which fields differ, but the decision is per question.
* **Comments, threads, notifications by email per version:** not planned. Members see "newer version" badges from an
  update check.

---

## 5. Data model

All new tables are in the Worker's pool database. The next migration is **`0108`** (`0107_pool_round30.sql` is the
last; a committed migration is never edited; the auth package keeps `0001-0099`). Phase A's `0108_circles.sql` should
already hold the version columns phase B needs, so phase B adds a `0109` only if something was missed. Times are epoch
milliseconds, ids are `newId()` hex, as elsewhere. Display names are read from `users` at read time (as `loadVisible`
does), so "deleted teacher" follows the account's state without rewriting circle rows.

### 5.1 Tables

**`circles`**

| Column | Notes |
|---|---|
| `id` | primary key |
| `name` | 1 to 80 characters, cleaned like other user text; shown as plain text |
| `description` | up to 1,000 characters, optional |
| `created_by`, `created_at`, `updated_at` | `created_by` is history only; the owner is the `circle_members` row with role `owner` |
| `closed_at`, `close_reason` | `owner_closed`, `last_member_gone`, `admin`; both NULL or both set (CHECK) |
| `undo_until`, `purge_after`, `purged_at` | 14-day undo for an owner close; the row stays as a tombstone after purge, its name and description nulled |

Index: `circles_purge (purge_after) WHERE purge_after IS NOT NULL AND purged_at IS NULL`.

**`circle_members`**

| Column | Notes |
|---|---|
| `circle_id`, `user_id` | primary key |
| `role` | `owner`, `editor`, `viewer` |
| `joined_at`, `join_seq` | `join_seq` comes from the existing `pool_seq` counter, so "longest-standing" has no clock ties (D18) |
| `left_at`, `left_reason` | `left`, `removed`, `circle_closed`, `account_deleted` (the last only inside the erasure batch, which then deletes the row); active means `left_at IS NULL` |
| `invite_id` | the code used; NULL for the creator |

Indexes: a **partial unique index** on `(circle_id) WHERE role = 'owner' AND left_at IS NULL` (at most one active owner,
enforced by the database, so two racing transfers cannot both win); `(user_id, left_at)` for "my circles";
`(circle_id, role, join_seq) WHERE left_at IS NULL` for the transfer pick and member counts. Rows of members who left
are kept until the circle is purged, because the re-share check (section 7) needs former membership. Re-joining reuses
the row (new `joined_at`, `join_seq`, role). Account erasure deletes the account's rows outright.

**`circle_invites`** (mirrors `pool_invites` from round 28)

| Column | Notes |
|---|---|
| `id`, `circle_id` | |
| `code_hash` | UNIQUE; SHA-256 of the normalised code, as `pool_invites`; the code is shown once and never stored or audited |
| `role` | `editor` or `viewer`: what the code grants |
| `created_by`, `created_at`, `max_uses` (1 to 50, default 1), `uses`, `expires_at` (NOT NULL: default 7 days, at most 30) | |
| `revoked_at`, `revoked_by`, `note` (up to 200 characters) | revoking stops further joins, never undoes one |

Index: `(circle_id, created_at)`. Proposal: circle codes carry a visible prefix (for example `C-`) so the app can tell a
circle code from a pool invite code typed into the wrong box (UNCONFIRMED; a wording/format choice for the owner).

**`circle_banks`**

| Column | Notes |
|---|---|
| `id`, `circle_id` | |
| `author_id` | the member who first shared it: "the item's author" for decision 4 and D17 |
| `title`, `subject`, `course`, `grade` | copies of the head version's, as `items` does |
| `licence` | `circle-only` or one of the five CC licences (D19); only the author changes it |
| `head_version`, `head_version_id`, `question_count`, `has_images` | written by the same compare-and-set UPDATE that accepts a version |
| `anonymized` | author erased, bank kept (D15) |
| `created_at`, `updated_at`, `removed_at`, `removed_reason`, `purge_after`, `purged_at` | reasons: `author_removed`, `owner_removed`, `circle_closed`, `account_deleted`, `admin`, `privacy`, `takedown` |

Indexes: `(circle_id, removed_at)`, `(author_id)`.

**`circle_versions`**

| Column | Notes |
|---|---|
| `id`, `bank_id`, `circle_id`, `version` | UNIQUE `(bank_id, version)` |
| `commit_id` | client-chosen, 22 to 64 characters, UNIQUE: the idempotency key (a retry with the same id and body returns the first answer), like `publish_id` |
| `request_sha256`, `result_json` | as `item_publishes`, for the retry replay |
| `committer_id`, `base_version`, `merged_from`, `rollback_of` | `merged_from`: the base the sender merged from; `rollback_of`: the version copied |
| `seq` | from the existing `pool_seq` counter: orders "who sent this text first" across banks and circles without clock ties |
| `message` | up to 500 characters; nulled when the sender's account is erased |
| `licence` | per version: a CC grant is made per version and is irrevocable for it (as in the public pool) |
| `status` | `staging`, `current`, `previous`, `pruned` |
| `full_key`, `index_key` | R2 keys under `circles/<circle>/<bank>/`: the body and a per-question index (`qid` and a keyed content hash per question) used to count added/changed/removed |
| `bytes` | body + index + the size of each image the version references (charged per version, see 8.1) |
| `question_count`, `added`, `changed`, `removed`, `created_at`, `committed_at`, `pruned_at` | |

Indexes: `(circle_id, status)` for the quota sum, `(committer_id)` for erasure.

**`circle_version_blobs`** `(version_id, sha256)` primary key, index on `sha256`. The images a version uses. Images
themselves stay in the existing content-addressed `blobs` table and `blobs/` in R2, uploaded with the existing
`PUT /blobs/:sha256`.

**`circle_qkeys`** `(circle_id, match_hash, bank_id, first_version, first_seq, first_by)`, primary key
`(circle_id, match_hash, bank_id)`, WITHOUT ROWID. The key-free `match_hash` (share-core's keyed hash) of every
question any version of the bank has held, with the version, sequence number and sender that first held it; kept
until the bank is purged. Used only by the public re-share check (section 7). Written with one
`INSERT OR IGNORE ... SELECT FROM json_each(?)` per version (only new hashes write a row, so the first sender is never
overwritten). Cap: 50,000 per circle; a version past it is refused plainly (UNCONFIRMED number). `first_by` of an
erased account stays as an opaque id (it can never match a publisher again).

**`circle_reports`** `(id, circle_id, bank_id, version, reporter_id, reason, note, at, resolved_at, resolved_by,
action)`; reasons `student_data`, `copyright`, `inappropriate`, `other`. Kept apart from `reports`, whose readers assume a
public item. **`circle_holds`** `(bank_id, reason, source, at, by)`, primary key `(bank_id, reason, source)`, mirroring
`item_holds`.

### 5.2 Existing tables reused, and what that costs

* `pool_audit`: circle actions are audited there (`item_id` = the circle bank id or circle id, `detail` names the
  circle). The table is append-only with the round-28 retention trigger; nothing changes in it.
* `usage_daily` and `pool_limits`: new metric names (`circle_creates`, `circle_invites`, `circle_join_tries`,
  `circle_commits`, `circle_rollbacks`, `circles_owned`, `circle_memberships`, `circle_members`, `circle_banks`,
  `circle_bytes`, `circle_questions`) are added to `DEFAULT_LIMITS`; `PUT /admin/pool/limits` already accepts any name
  in `METRICS`. No schema change.
* `admin_flags.kind` is a CHECK list (rebuilt once already, in `0103`). A new flag kind needs the table rebuilt in a
  migration. Phase A should avoid that: open `circle_reports` rows are the admin's queue for circle cases, and cap
  alerts reuse the existing `cap` kind (UNCONFIRMED whether that is enough; if not, `0108` rebuilds `admin_flags`
  the way `0103` did).
* `blobs`, `pending_blobs`: shared with circles. The hourly blob GC (`gcBlobs`, `moderation.ts`) deletes a blob that
  no `item_blobs` row and no live `pending_blobs` row references. **It must also respect `circle_version_blobs`**, or
  images used only by circle banks are deleted within the hour. The same applies to `reconcile`, `privacy_bytes`
  (`usedUnserved`) and any purge that releases images.
* `pool_seq` (the single-row counter publishes already use): `join_seq` and each version's `seq`.

### 5.3 R2 layout

`circles/<circle_id>/<bank_id>/<version_id>.json` (body) and `.idx.json` (index). Images in the existing `blobs/`.
Admin quarantine for a circle takedown (phase C) under the existing `quarantine/` prefix.

### 5.4 Client records

`share-circles.json`, one record per linked local bank: circle id, circle bank id, local bank id, base version, the
`qid` of each local question, last check, and (offline fallback only) a base snapshot with the existing caps of 500
questions and 1.5 MB (`merge.rs` `BASE_MAX_QUESTIONS`, `BASE_MAX_BYTES`). Normally the base is fetched from the
server's kept versions, so banks of up to 1,000 questions can be merged. Copies taken from a circle without linking
(phase A "Get a copy") are recorded like public imports, with a "From the circle ..." credit line.

---

## 6. API routes

### 6.1 Rules for every circle route

* They sit in the pool's existing route table (`src/pool/index.ts` `ROUTES`), so they inherit everything in front of
  it: a live session, trust `teacher` or `admin`, the round-28 invite-code gate (`403 invite_required`), the
  per-account and per-address rate limits, and the pool-key check on writes (`503 pool_key_mismatch`).
* A circle, bank or version the caller is not an **active member** of answers `404 not_found`, never 403 (the pool's
  existing rule: no existence oracle). Admins are not members by being admins; they use admin routes (6.4), audited.
* Inside a circle the caller belongs to, a missing role answers `403 circle_role_required {needed}`.
* Every write the caller's request makes is guarded in the same statement or batch with `ALIVE_SQL` (round 27), so a
  request that was authenticated just before an erasure writes nothing and answers `401 unauthorized`.
* Errors use the existing envelope: `{"error": "<code>", ...extra}` with `message` where the app shows words;
  `400 invalid_request {field}`; `429 daily_limit {metric}` with `Retry-After`; `413 quota_exceeded {used, needed,
  limit}` (with `scope: "circle"`); `409 conflict` on a lost compare-and-set.
* Request bodies use the existing single body reader (`Content-Length` required and exact, deadlines, per-account
  in-flight caps). A version body uses the publish format, `application/vnd.vershion.publish` (`<meta JSON>\n<payload>`).
* Circle and bank ids are 32 hex characters and the route patterns say so, so `/circles/join` and `/circles/heads`
  can never be read as a circle id.

### 6.2 Routes

| Route | Who | What | Phase |
|---|---|---|---|
| `POST /circles` | approved teacher | `{name, description?}` -> `201 {circle}`; creator becomes owner. Caps: owned, memberships, daily creates | A |
| `GET /circles` | teacher | my active circles: name, my role, member and bank counts | A |
| `GET /circles/:cid` | member | name, description, my role, members (display name, role, joined; "deleted teacher"), banks (title, author, licence, head version, question count, updated) | A |
| `PATCH /circles/:cid` | owner | `{name?, description?}` | A |
| `DELETE /circles/:cid`, `POST /circles/:cid/reopen` | owner | close with a 14-day undo (D7); reopen inside it | A |
| `POST /circles/:cid/invites` | owner | `{role, max_uses?, expires_in_days?, note?}` -> `201 {invite_id, code, shown_once: true}` | A |
| `GET /circles/:cid/invites`, `POST /circles/:cid/invites/revoke` | owner | list with state (`active`, `expired`, `used_up`, `revoked`); revoke `{invite_id}` (`409 already_revoked`) | A |
| `POST /circles/join/preview` | teacher | `{code}` -> circle name, owner's display name, member count, role offered. Uses nothing; counts a try | A |
| `POST /circles/join` | teacher | `{code}` -> `{circle_id, role}`, or `{ok, already: true}`. Every bad code is the same `422 circle_invite_invalid` | A |
| `POST /circles/:cid/leave` | member | an owner leaving triggers the transfer of decision 3 (6.3) | A |
| `POST /circles/:cid/members/remove` | owner | `{user_id}`; not yourself (use leave) | A |
| `POST /circles/:cid/members/role` | owner | `{user_id, role: editor or viewer}` | A |
| `POST /circles/:cid/transfer` | owner | `{user_id}` of an active member; they become owner, the old owner becomes an editor | A |
| `POST /circles/:cid/banks` | editor, owner | first version of a new bank (publish body; meta: `commit_id`, title and listing fields, `licence`, `attest {own_work, terms_version}`, `message`) -> `201 {bank_id, version: 1, qids}` | A |
| `GET`/`HEAD /circles/:cid/banks/:bid` | member | the head version, streamed, `ETag` = version id, `If-None-Match` gives 304. Viewing and copying are the same read; the copy is made on the device | A |
| `GET /circles/:cid/banks/:bid/blobs/:sha256` | member | an image of a kept version of the bank | A |
| `POST /circles/heads` | teacher | `{bank_ids}` (up to 200) -> per id: head version, when, who; `removed`; or `unavailable` (left, removed, closed) | A |
| `PATCH /circles/:cid/banks/:bid` | bank author | `{licence}`: the re-share setting (D19). Earlier versions' CC grants stand | A |
| `DELETE /circles/:cid/banks/:bid`, `.../undelete` | owner, bank author | 14-day undo, then purge | A |
| `POST /circles/:cid/banks/:bid/report` | member, not the author | `{reason, note}` -> always "thanks" | A |
| `PUT /circles/:cid/banks/:bid` | bank author (A); editor, owner (B) | **send a version**: meta adds `base_version`, optional `merged_from`, `merge_token`, `confirm_large_change`. -> `200 {version, version_id, qids}`; `409 conflict {head_version, head_at, head_by}`; `428 confirm_large_required {removed, total}` | A (author), B |
| `GET /circles/:cid/banks/:bid/versions` | member | history, newest first, 50 a page (`?before=`): version, sender, when, message, added/changed/removed, `rollback_of`, `merged_from`, kept or not | B |
| `GET /circles/:cid/banks/:bid/versions/:n` | member | a kept version's body (the merge base); `404 version_not_retained {oldest_kept}` | B |
| `POST /circles/:cid/banks/:bid/merge` | editor, owner | **merge-initiate** `{base_version}` -> head, whether the base is still kept, the versions in between (who, when, message, counts) and a `merge_token` (signed, 30 minutes, binds account, bank, base and head). Informational: no lock, nothing stored; the token only lets the next send record "merged v7 into v9" in the history | B |
| `POST /circles/:cid/banks/:bid/rollback` | owner, bank author | `{to_version, base_version, commit_id, message}`: a new version equal to a kept one, same compare-and-set | B |

Error codes new to circles: `circle_role_required` (403), `circle_invite_invalid` (422), `circle_full` (409, `limit`),
`circles_limit` (422, `metric`, `used`, `limit`), `circle_closed` (409, `undo_until`), `bank_removed` (409),
`version_not_retained` (404), `circle_questions_limit` (422), `circle_records_full` (413), `circle_material` (422, on a
public publish, section 7). Existing codes reused: `not_found`, `invalid_request`, `daily_limit`, `quota_exceeded`,
`conflict`, `confirm_large_required`, `publish_in_progress`, `busy`, `attestation_required`, `invalid_payload`,
`license_incompatible`, `unauthorized`.

### 6.3 Ownership transfer (decision 3), in one statement

When the owner leaves, is erased, or (admin) is removed, one batch does three things in order: the old owner's row
stops being active (left; or becomes an editor on an explicit transfer); one UPDATE makes the active editor with the
smallest `join_seq` the owner, in each such circle that now has no active owner; if there is none, the active viewer
with the smallest `join_seq` (D14, UNCONFIRMED); if there is no other active member, the circle closes
(`last_member_gone`) and its banks are removed and purged after 14 days. Each pick and its update are one statement,
"circles with no active owner" is the condition (so a rerun or the daily resweep finds exactly what is left to do),
and the partial unique index allows one active owner, so concurrent leaves cannot produce two owners. The circle is
never left without an owner while it has an active member (an invariant the oracle tests check, section 12).

### 6.4 Admin routes

Behind the auth package's admin guard, exact paths, ids in body or query, a sudo token for every write, as today:
`GET /admin/pool/circle-reports`; `GET /admin/pool/circle?id=` (metadata, members, banks: audited);
`GET /admin/pool/circle-bank?id=&version=` (content: audited, D1); `POST /admin/pool/circle-hold|circle-release|
circle-remove|circle-close` with a reason of 10+ characters. No admin route makes an admin a member.

### 6.5 Existing routes that change

* `POST /items`, `PUT /items/:id` and their previews: the circle-material check (section 7).
* `GET /me/export`: gains a `circles` section: memberships (circle, role, joined, left), circles owned, invites
  created (no codes), versions sent (bank, version, when, message; not bodies), reports filed.
* Account deletion and the cron: section 8.

---

## 7. Licences, re-sharing and the provenance engine

**Why circle content stays out of the public engine.** The licence and provenance code (`provenance.ts`, `visible.ts`,
`publish.ts`) decides who was first, what a copy may be licensed as and whom to credit, from rows that assume one
author per item and content visible to "others" (`VISIBLE_TO_OTHERS`, `visibleTo`, `served`, `author_contents`,
`content_licences`). A circle bank has several senders and is visible to a group. Feeding it in would touch every one
of those queries. So in phases A and B circle reads write no `served` rows, circle versions write no authorship
records, and circle banks never appear in browse, search, `bound_by` or credits.

**What could go wrong without a check.** A member copies a circle question into a public publish. The engine has no
record of it, so the member would be treated as its original author: the material leaks against the author's wish,
and the member is falsely credited as first (a "false credit/first" finding is a blocker class under the existing
convergence rule).

**The check (phase A).** At public publish (and its dry run), after share-core has produced the publish's key-free
`match_hash` values: take the DISTINCT hashes, look them up in `circle_qkeys` for every circle the publisher is or was
a member of (`circle_members` rows, active or left), and for each hash keep the earliest hit (smallest `first_seq`
across those circles). A position passes when that earliest hit was **first sent by the publisher** (`first_by`): her
own words, even inside a shared bank. Otherwise it is refused with `422 circle_material {positions, circle, bank}`,
whatever the bank's licence, until phase C's route exists. The circle and bank names go only to the publisher, who is
or was a member. Three ways through, so honest teachers are not locked out (a wrongly refused honest teacher is a
blocker class); the second and third reuse the public pool's existing rules:

* **Already hers in the pool.** A question the publisher had published in the public pool before the circle's first
  sender sent it passes: `author_firsts (author_id, match_hash, first_seq)` already records her first publish on the
  same `pool_seq` scale, one key lookup per distinct hash. Without this, a colleague copying her public question into
  a circle would block her own republish.
* **Short statements** pass, by the public engine's existing definition (a True/False stem of at most 8 words, an Mc
  stem and choices of at most 12, and so on; `docs/shared-pool.md` section 4). "Is 7 prime?" in someone's circle bank
  must not stop anyone publishing it.
* **An independence statement** (the shape of the existing `independent_of`: the circle bank named, a statement of 10+
  characters) lets a position through; it is audited and flagged for the admin, and the third in 30 days flags the
  account, as today. It never covers an exact copy of a question the publisher can still read in a `circle-only` bank
  first sent by someone else (UNCONFIRMED: this mirrors the public rule that a claim never covers an exact copy of
  something served; the owner may prefer a softer rule for circles).

Checking per question by first sender, rather than "the bank's author may publish her bank", is what makes the rule
work with shared editing: a bank holds several people's questions, and only the person who first sent a question's
exact text is treated as its author.

**Allowed re-sharing (phase C).** For a circle bank under a CC licence, a member re-shares through a server route
(`POST /circles/:cid/banks/:bid/share-publicly`, proposed) that runs the normal public publish and records the first
sender of each question as its origin, so the public engine credits them and knows they came first. This changes the
licence engine and gets its own critic loop. Until it exists, decision 4's "allowed" case is served only by the
author herself (her own questions pass); members are refused with a message saying public sharing of this bank is not
available yet. The owner may instead choose to let members re-share CC circle banks from phase A, accepting that the
public pool would then credit the member, not the author (D13).

**Public material inside circles.** A member may copy a public CC item into a circle bank. The circle bank's licence
must be one the origin allows (the existing licence table; `circle-only` is not in any CC origin's allowed list
except CC0's "any"). Phase A: the app warns and asks for a compatible licence, using the origins it already records in
`share-imported.json`; the server checks declared origins only. Whether sharing to a private group of up to 50 counts
as "Share" under CC 4.0 (which would make attribution and ShareAlike apply) is a legal question (UNCONFIRMED; D9).

**Documented limits of the check.** Exact key-free match only: rewording defeats it, as it defeats the public pool's
matching (owner decision, `docs/shared-pool.md` 8.2). An editor who changes one word of someone else's question in a
shared bank becomes the first sender of the changed text and may publish it; a key-only change keeps the original
sender (the hash is key-free). A person can always retype or screenshot what they can read. The licence, the terms
and reports are the control.

**Contributor terms.** With shared editing, a bank holds several people's work. Proposed rule (needs counsel, D9):
by sending a version you license your additions to the circle's members under the bank's licence, and you agree that
the bank's author sets that licence (including re-share). Without it, an editor could argue joint authorship and the
right to re-share regardless of the flag (UNCONFIRMED; US joint-work law needs a lawyer).

---

## 8. Erasure and retention

### 8.1 Account deletion (`deleteContentStatements`, run by the auth package's `onDelete` after the tombstone)

Added statements, in this order inside the same batch (the code already relies on a D1 batch being atomic), each
set-based and bounded by the account's own rows:

1. **Step down.** The account's memberships are marked left (`left_reason = 'account_deleted'`). This must come
   first: the partial unique index allows one active owner, so the old owner's row has to stop being active before a
   new owner is named in the same batch.
2. **Transfer** (6.3): in every circle the account owned that now has no active owner, one UPDATE names the active
   editor with the smallest `join_seq`; a second names the longest-standing viewer where there is still none (D14).
   At most 5 owned circles, each choosing among at most 50 members.
3. **Close** the circles the account owned that still have no active owner (nobody else was left); remove their
   banks (`circle_closed`, purge in 14 days).
4. **Banks the account authored** in surviving circles: per D15 (`anonymized = 1`, or removed when `keep_on_delete`
   is no). With no choice made: kept.
5. **Free text** the account wrote goes, as in the public pool: version messages, report notes. Circle names and
   descriptions stay when the circle survives (they are the circle's, not the person's; UNCONFIRMED whether counsel
   agrees).
6. **Invites** the account created are revoked.
7. **Memberships** of the account are deleted (steps 2 to 4 needed them to find what the account owned and wrote).
8. Versions the account sent keep their `committer_id` (an opaque id) and their content; the history shows
   "deleted teacher".

Email invites (if D5 brings them) cannot be matched by address after the tombstone (the auth package has nulled the
email by then); they expire within 7 days.

### 8.2 `erasure_finish` and `erasure_resweep` (daily cron)

* `erasure_finish` reruns the whole batch for an account without an `erasure_done` row. Every added statement must
  match only what is not yet clean (a second run after a success changes nothing).
* `erasure_resweep` is extended to every circle table a request already in flight could have written under the
  erased account's id: `circle_members` (a join or a transfer that slipped past: such rows step down, then the
  transfer and close steps run again for open circles with no active owner), `circle_invites` (revoke), `circle_versions.message` (null), `circle_reports`
  (note and reporter nulled), and `circles` created by the account whose only member is erased (close). Same
  pattern as today: `... WHERE user_id IN (SELECT user_id FROM erasure_done)`, indexed, bounded (50 circles a run).

### 8.3 Retention and purges

* Closed circles and removed banks: purged by the existing daily `purges` stage pattern (batches, backoff, never with
  an open appeal): R2 bodies and indexes, `circle_versions` rows, `circle_version_blobs`, `circle_qkeys`, invites, and
  then member rows; the `circles` row stays as a tombstone with name and description nulled.
* Pruned versions (D16): a daily stage deletes the R2 body and index of versions past the 20 kept (at most 500 a run);
  the history row stays.
* `pool_audit` rows for circles go after 1,096 days through the existing `retention` stage. Its "keep while the item
  still has a hold or an open appeal" rule looks at `item_holds` and `admin_flags` by `item_id`; it must also look at
  `circle_holds` and open `circle_reports`, or a circle case's trail could go while the case is open.
* Tombstones of purged circles go after 3 years with the audit rows that name them (UNCONFIRMED; same rule as audit).

---

## 9. Memory and D1 cost

The pool just had five publish-time statements and the hourly licence re-key rewritten because they read rows growing
with the square of the bank (rounds 30 and 31; one is still open: `evidenceChunk` when a bank repeats one question).
Every circle statement must be linear in its inputs, use an index chosen on purpose, and be checked the same way:
`EXPLAIN QUERY PLAN` plus `meta.rows_read` on workerd's SQLite at 250 and 990 questions.

### 9.1 Expected rows read per operation (estimates, with the D2 caps)

| Operation | Rows read | Rows written | Notes |
|---|---|---|---|
| create circle | about 25 (my memberships, at most 20) | 4 | |
| issue / revoke code | about 15 | 2 | |
| join preview / join | under 100 (code by unique hash; members at most 50; my memberships) | 3 | |
| list my circles | up to about 3,000 (20 circles x members and banks by index) | 0 | counted, not stored, so they cannot drift |
| circle page | about 250 (members joined to `users` by key, banks) | 0 | |
| read the head / a version | 3 lookups by key, then R2 | 0 (plus the daily counter) | streamed |
| history page | at most 50 | 0 | |
| send a version | about 2,000 for the quota sum (kept versions of the circle: 100 banks x 20) + up to 1,000 key probes for `circle_qkeys` | new keys only (up to 1,000), images, about 5 more | plus one share-core run and one R2 read of the previous index |
| merge-initiate | at most 20 | 0 | |
| rollback | about 2,000 | about 5 | no share-core run; the body is copied |
| public publish: circle check | DISTINCT hashes (at most 1,000) x the publisher's circles, plus actual matches (earliest per hash) | 0 | DISTINCT first: a bank repeating one question costs one probe per circle, not 1,000 (the round-31 lesson); former memberships grow over the years, so their count needs a cap or a bound in the test (UNCONFIRMED) |
| erasure (added statements) | at most about 250 + the account's own rows | same | |
| update check (`/circles/heads`) | at most 200 banks + membership checks | 0 | |

**Quota without a quadratic sum.** Images are charged per version (each kept version pays for the images it
references), not deduplicated across the circle. The sum is then one indexed `SUM(bytes)` over the circle's kept
versions (at most 2,000 rows). A deduplicated sum would join versions to images (100 banks x 20 versions x up to
hundreds of images) on every send. The cost: shared images are over-counted against the circle's 200 MB. Honest and
conservative.

### 9.2 Memory

* A send is a publish-sized request: the body (up to 4 MB) is reserved 16x through the heavy phase, it waits for the
  isolate's single heavy slot (4 may queue up to 20 seconds, then `503 busy`), and it holds the account's publish lock
  (`pool_locks`), so a teacher cannot publish and send at once (`409 publish_in_progress`). The 72 MB isolate budget
  is unchanged. Sends compete with public publishes for the heavy slot (a risk under load, section 14).
* share-core runs `publish_prepare` once per send for validation and the key-free hashes; no strip unless phase C's
  "hide keys from viewers" is on. Measured cost of a 3.8 MB bank in local workerd: about 3 seconds (Worker README,
  share-core round 7), so heavy circle editing costs about what heavy publishing costs.
* Reads stream with the existing read reservation (1 MB plus 3x the JSON head).
* Rollback copies the old body from R2 to a new key. The Workers R2 binding is assumed to have no server-side copy, so
  it streams get to put with a known length (UNCONFIRMED; measure).
* Client: a merge holds the base, the head and the local bank (up to three 4 MB banks plus parsed trees). Not measured.

---

## 10. Client UI plan (proposal; layout and wording for the owner to confirm)

The brief settles: a "Circles" tab on the Shared page, the existing full-page style (one scroll column of
`pageshell::card`s capped at 960 px, the same pills and step strip), guided empty states and plain English. The
placement of the tab among the existing five (`src/share/shared.rs` `Tab`) and the exact labels below are proposals,
not decisions.

* **Gate.** The Shared page's existing guided steps apply unchanged: a person sees the Circles tab only at step 5
  ("Ready"), so sign-in, email, approval and the pool invite code are already handled.
* **Circles tab, list.** One card per circle: name, my role, members, banks, "n banks have newer versions". Empty
  state: "You are not in any circle yet. A circle is a small private group of teachers who share banks with each
  other. Create one, or enter a code someone gave you." Buttons: "Create a circle...", "Join with a code...".
* **Create.** Name and optional description; then the circle page with a hint to make a code.
* **Codes.** Owner only: choose "can edit" or "can only view", how many people, how long; the code is shown once with
  "Copy" and the sentence "Anyone with this code can join until it expires or is used up. It is shown only now."
  The app has no registered link handler (only pasted-text parsing, as for figure links in `src/figures/share.rs`;
  UNCONFIRMED that nothing else exists), so a "link" is text holding the code, recognised when pasted.
* **Join.** Paste a code (or a link holding it); the app shows the preview ("Join 'Algebra team' as an editor? Owner:
  A. Teacher, 6 members.") before anything is used.
* **Circle page.** Cards: banks (title, author, licence line, version, updated), members (owner can change roles,
  remove, transfer), codes (owner), activity, and "Leave this circle" / "Close this circle" with plain consequences
  ("Everyone loses access in 14 days. Copies on their computers stay.").
* **Bank in a circle.** Phase A: "Look at it", "Get a copy" (a private copy, like importing). Phase B adds "Edit with
  the circle" (a linked copy in My banks). The linked bank's page shows one quiet line: "In circle 'Algebra team',
  version 7. Version 9 is the latest." with "Get latest and merge..." and "Send my changes...". Editing happens in the
  existing bank editor; hooks are needed in the bank page and `app.rs` (tell the owner before touching their files).
* **Send.** A short summary (added, changed, removed questions), a message box, the own-work tick, the licence line,
  then Send. On `409`: "Someone sent changes first (B. Teacher, 10 minutes ago). Get the latest changes, check them,
  then send again." with the merge button.
* **Merge review.** The existing review screen from the pool update, with "the circle's version" in place of "the
  pool's", the restore point sentence, and the two-way note when the base is gone.
* **History.** A list of versions (number, who, when, message, counts); "Look at this version"; "Roll back to this
  version..." for the owner and bank author, explaining that later versions stay in the history.
* **Answer keys.** When sharing into a circle: "Everyone in this circle will see the answer keys."
* **Public publish refused for circle material.** The existing Fix step of the publish window gets one more refusal:
  "Questions 3 and 7 were first shared by another member, in the circle bank 'Unit 2 quiz' (Algebra team). They can
  not be shared publicly from here." with the independence statement as the way out where it applies (section 7).
* Not run until built: a visual check by the owner on a real screen (as for every earlier window).

---

## 11. Legal and privacy impacts

The legal pages are drafts and not legal advice (`server/web/legal/`). Circles add:

* **Privacy page (`privacy.html` section 3, "Who can see what")**: members of a circle see each other's display names
  and roles (never email or school, proposed), the circle's banks **with answer keys**, and who sent each version and
  when; the circle owner sees the codes' state, never who typed a wrong code. New data stored: circle names and
  descriptions, memberships and roles with dates, invite codes as hashes, version history with the sender's id and
  message. The administrator's access rule (D1). Retention: closed circles purged after 14 days; old versions pruned;
  audit kept 3 years. Deletion: ownership passes on (decision 3); sent versions stay in the circle as "deleted
  teacher"; messages removed; banks you created kept or removed per D15. Export gains the circles section (6.5).
* **Terms (`terms.html` section 3)**: today they say items "are shared to the public pool" under the five CC licences.
  They need: circles and the "All rights reserved" (`circle-only`) option; the contributor terms (section 7); that a
  CC licence on a circle bank lets members re-share publicly and cannot be withdrawn for versions already received;
  that removed or leaving members keep their copies; owner responsibilities; the attestation applies to every send.
* **Acceptable use**: circles are not for publisher test banks, paid worksheets or student data, even privately.
* **Counsel review needed** (before anyone outside the pilot uses circles): contributor terms and joint authorship;
  whether private-group sharing is "Share" under CC 4.0; the DMCA 512 process for content the public cannot see
  (notice, expeditious removal, counter-notice; phases A and B run it by hand); the operator's access to private
  content and how the privacy page describes it; the counter-notice wording already pending.
* **Not affected**: no student accounts, no student data by design (the same controls as the public pool, plus the
  circle report).

---

## 12. Test plan

Same method as the public pool: a builder per side, then fresh critics each round, the owner's convergence rule (a
finding is a blocker only if it is a new class), and the orchestrator re-running every suite after each builder report.

**Server tests** (`server/worker/test/pool/circles-*.test.ts`, harness `test/pool/harness.ts`):

* **Oracle tests.** A small in-test model of circles (members, roles, owner, codes, banks, heads, kept versions) is
  run against the Worker for random sequences of create, code, join, leave, remove, role change, transfer, close,
  reopen, send, rollback, erase and cron runs, comparing state after every step. Invariants: at most one active owner,
  and exactly one while any active member exists; "longest-standing" picks the smallest `join_seq`; versions are
  linear and gap-free; every accepted version's body equals what its sender sent; every refused send got `409` with
  the true head; a rollback's body equals the version it names; nothing a non-member can reach (every route answers
  404 to non-members, including former members).
* **Lost-update tests.** N editors send against the same head at every interleaving of their D1 calls (the existing
  "interleaved at every D1 call" pattern): exactly one wins each round; retries with the same `commit_id` replay;
  a different body under the same `commit_id` is refused.
* **Erasure race tests.** Erasure against each in-flight write (join, send, code, report, transfer, role change) at
  every D1 call; `erasure_finish` reruns; `erasure_resweep` idempotence; owner erased with 0, 1 or many editors, only
  viewers, or ties on join time; the last member erased while a valid code is being used (the join must fail).
* **Capacity tests.** 50 members x 20 circles; 100 banks x 1,000 questions; 20 kept versions; a send of round 29's 3.81 MB
  Matching bank (990 questions x 50 pairs) within the 72 MB budget; `EXPLAIN QUERY PLAN` of every new statement (no table scan,
  intended index); `meta.rows_read` at 250 and 990 questions, growing at most about 4.5x for 4x the bank; the circle
  check with a bank repeating one question 990 times and with a hash present in every bank of a circle.
* **Shared-code regressions.** Blob GC, reconcile and privacy-bytes never delete an image a circle version uses;
  the public suites (507 tests at round 31) unchanged; the circle check, for current and former members, refuses a
  question first sent by someone else and passes the publisher's own, short statements and an audited independence
  statement; it never refuses a non-member; first sender is decided by `seq`, ties and rejoins included; nothing about
  circles appears in browse, search, `bound_by`, credits or `/items/versions`.
* **Migration test** for `0108` on a database with data (as for `0104` and `0106`).

**Client tests** (`src/share/circles*.rs` with scripted sessions, as `publish_tests.rs` and `browse_tests.rs`): the
push flow (send, `409`, merge, send again); base not kept -> two-way review with no defaults; conflict has no default
and Apply stays disabled; a typed key is never replaced by the placeholder (the existing guard); restore point written
before any local change; codes cleared from memory after use; plain messages for every new error code.

**Critics each round:**

* **Moodle-parity critic.** Compares against Moodle's question bank versioning and sharing (a sparse Moodle clone, as
  for earlier critics): how versions, history, "latest version" and permissions to view, use and edit work there, and
  whether a teacher used to Moodle would be surprised or could lose work here. Moodle's exact behaviour is for the
  critic to establish from the code (UNCONFIRMED here).
* **Security / ASVS critic.** Access control (circle and bank ids, former members, role checks, admin access),
  business-logic races (codes, joins, transfers, sends), anti-automation (code guessing, invite caps), data protection
  (keys and content reachable only by members), logging (audit without codes or content), erasure. ASVS requirement
  ids to be cited by the critic from the version it uses.
* **Blocker classes for circles** (proposed, extending the owner's rule): content or keys reachable by a non-member;
  a lost update or silent overwrite; data loss or resurrection around erasure, close or purge; a circle with no owner
  or two; circle material reaching the public pool past the check for exact copies; a quadratic statement; a cap
  that fails open.

---

## 13. Phased delivery

Sizes are rough guesses for planning, not measurements.

### 13.1 Phase A: circles, codes, view and copy

* **Server:** migration `0108_circles.sql` (all tables in section 5); routes marked A; the bank author's own sends
  (republish); ownership transfer; reports and holds with admin routes; erasure, resweep, purge and retention
  extensions; blob GC and reconcile respecting circle images; the public circle-material check; export section;
  limits. About 2,000 to 3,000 lines of TypeScript and 1,500 of tests.
* **Client:** Circles tab (section 10), create, codes, join, circle page, bank view, "Get a copy" (recorded like an
  import), "Share to circle" as a destination in the existing publish window, update badges, plain messages for every
  new code. About 2,500 lines of Rust plus tests.
* **Rounds:** expect 3 to 5 builder-critic rounds (the public pool needed 31 on the backend, but most of that was the
  licence engine, which circles avoid).
* **Risks:** the circle check's false "first" edge cases; blob GC; erasure transfer races; the legal pages.

### 13.2 Phase B: shared editing

* **Server:** editors send; history and kept-version routes; merge-initiate token; rollback; large-change confirm;
  version pruning; the confirmed-student-data check on sends (the publish path's privacy-block check reused), because
  private circles are where a class list would be pasted.
* **Client:** linked working copies (`share-circles.json`), "Send my changes", "Get latest and merge" using
  `merge.rs` with circle wording, the two-way fallback, history screen, rollback. `merge.rs` today is written for the
  pull direction and the pool's wording; its logic is reused, its text and the base source change.
* **Size:** about 1,500 lines server, 2,500 client, plus tests. 3 to 5 rounds.
* **Risks:** lost updates; merge correctness at 1,000 questions; heavy-slot contention with publishes.

### 13.3 Phase C: polish

* Email invites (if D5 says so); "hide keys from viewers" (share-core strip and key-hint gate for viewers);
  copyright re-upload block checks on sends; the allowed re-share route with origin credit (licence engine change, own
  critic loop); the DMCA takedown machine extended to circle banks (notice, quarantine, counter-notice window,
  strikes); activity list polish. Size unknown until A and B land.

---

## 14. Biggest risks

1. **Circle material and the public licence engine.** Keeping circles out of the engine avoids touching its hardest
   code, but the boundary check is new: a re-share it misses leaks private material and falsely makes the copier
   "first" publicly; allowed re-sharing (phase C) has to change the engine itself. Public CC material copied into
   `circle-only` banks may break the licence terms (legal question open).
2. **Erasure and ownership transfer.** Transfer, close and removal run inside the erasure batch, `erasure_finish`
   reruns and the daily resweep, with requests in flight. A mistake leaves a circle with no owner or two, resurrects a
   deleted member, or deletes banks other people were editing.
3. **Lost or silently overwritten work.** The model depends on the compare-and-set, idempotent sends, kept merge
   bases and a correct push-direction merge in the client. `merge.rs` was written and tested for pull-only updates
   with a 500-question base cap; circle banks reach 1,000 questions and the base comes from the server instead.
4. **Moderation and law.** Private circles are exactly where copied publisher test banks and student data would sit,
   out of sight of the public reporters. The operator's access rule, a by-hand DMCA process for phases A and B,
   contributor terms and joint authorship all need counsel before circles leave the pilot.
5. **Shared infrastructure.** Circle images live in the shared `blobs` store (the GC, reconcile and privacy stages
   delete what they do not know is used); sends compete with publishes for the heavy slot and the 72 MB budget (whole
   4 MB bodies per send, even for a one-word change); new statements could reintroduce the quadratic D1 reads rounds
   30 and 31 removed; a new admin flag kind needs a table rebuild migration.

---

## 15. Honest limits

* Not real-time. Two people changing the same question always produce a conflict for whoever sends second, and a
  person decides it. Busy banks mean more merges.
* Every send uploads the whole bank (up to 4 MB) and runs share-core, even for a one-word change.
* Only 20 versions are kept in full; past that a merge falls back to a two-way review and rollback is not possible.
* Members see answer keys (D12). Anyone who can read a bank can copy it, photograph it or retype it; leaving or
  removal does not take copies back. The licence, the terms and reports are the control.
* The re-share check matches exact (key-free) questions only. Rewording or translating defeats it, as in the public
  pool. Short statements are exempt by design, so a member can publish someone's short circle question; an
  independence statement lets other questions through on the member's word (audited and flagged, not verified).
* Until phase C, members cannot re-share a circle bank publicly even when its author chose a CC licence; only each
  question's first sender can publish it.
* The operator can read circle content when it is reported, named in a notice or matches a student-data block.
  Circles are private from other teachers, not from the operator.
* No per-question permissions or locks: an editor can change any question in a bank. History and rollback are the
  safety net.
* Phases A and B have no automatic copyright takedown process for circle banks; the admin handles notices by hand.
* Merge, history and send screens have not been designed against a real screen yet, and nothing in this plan has been
  measured: the row counts, sizes and memory figures above are estimates.
