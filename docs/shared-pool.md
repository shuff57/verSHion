# verSHion shared pool: current specification

Written 2026-10-23 from the code and the builder READMEs, not from the old spec (kept as
`docs/shared-pool-old-spec.md`; where they differ, this file wins). Nothing is committed, deployed or
launched; the pool runs only in tests, local workerd and an unused staging Worker.

Sources, in order of authority: the code (`server/worker/src/pool/`, `server/auth/src/`,
`server/share-core/src/`, `src/share/`), the three READMEs (`server/worker`, `server/share-core`,
`server/auth`), the draft legal pages (`server/web/legal/`), `docs/handoff-shared-pool.md`, and the
owner's decisions note. Where a README and the code disagree, this spec describes the code and lists the
disagreement in the appendix. "Round N" means a builder-critic review round recorded in the READMEs.
Anything not confirmed is marked UNCONFIRMED.

---

## 1. What it is and what stays local

Approved teachers publish banks (sets of questions) or single questions to a server; other approved
teachers browse them and import **private copies** that keep attribution and the licence. There is no live
co-editing, no student account and no student data.

Always local: banks, exams, editing, printing and export work offline and signed out. The share module
does nothing (no network, no keychain) until the Account window is opened. Imported copies live on the
importer's device and cannot be recalled; the server can only say the source changed or was removed, and why.

A release build without `VERSHION_POOL_URL` has sharing switched off; a release build with it talks only to
that server. Debug builds, or the never-to-ship feature `pool-custom-server`, let a developer type an address
(`src/share/mod.rs`).

At launch there are no circles, ratings, favourites or separate search service: only **public** items and
**unlisted** items. Unlisted means private: published (so it can be republished) but readable only by its author.

**Circles (backend phase A, round 32; not at the first launch).** The Worker now has the server side of
circles as planned in `docs/circles-plan.md` (decisions D1-D19): small private groups of approved teachers
joined by code, banks shared into a circle for members to view and copy, the author's own republish as
versions, caps, erasure and succession, purges, the admin's audited access, and one check at public publish
that keeps material first sent into a circle by someone else out of the public pool. Circle content stays out
of this document's licence engine (no served rows, no authorship records). Routes, decisions and limits:
`server/worker/README.md` "Round 32", "Round 33" (fixes after two critics: re-joining after a removal,
codes that outlive their issuer's owner role, caps through transfers, holds and purges) "Round 34" (ownership is
an offer the member accepts, succession of owner-closed circles, accounts not in good standing) "Round 35" (removed
banks stay findable by the owner, offers capped a day) and "Round 36" (no null text in circle responses, `closed_by`;
the documented limits). **Phase B backend (round 37):** shared editing on the server: editors send versions against a
base (409 with the true head, nothing stored), a history and the last 20 versions kept in full, merge-initiate with a
30-minute token that records "merged from", rollback by the owner or the bank's author (D17), a confirm for versions
that drop more than half a bank, and the public publish's confirmed-student-data check on every send and rollback
(a match holds the bank, never refuses); `server/worker/README.md` "Round 37". Round 38 (after critics 45a/45b): a
version that matched confirmed student data stays withheld from every member until an administrator releases that
version; the byte cap counts what a commit prunes and an owner can trim a bank's older versions; a repeated question
keeps its origin; README "Round 38". Round 39 (critics 46a/46b): a bank held for student data can still be fixed at
the byte cap, a withheld version's title never lists the bank, lost sends record their R2 orphans, and an editor cannot
graft or impose a public item's credit; README "Round 39". Round 40 (critic 47a): an author's adapted copy of a credited
question keeps the credit wherever it sits, the owner can repair or purge a bank held for student data, and the admin
mail says to release the matching version; README "Round 40". Round 41 (critic 48): a bank with an open student-data
flag is not purged until the admin decides; README "Round 41".
There is no client yet; the owner's "no circles at the first launch"
stands until the app ships them.

Stack: a Cloudflare Worker (TypeScript) with D1 (`vershion-pool`), a private R2 bucket `POOL` (bodies under
`items/`, takedown quarantine under `quarantine/`, images under `blobs/`), and the Rust crate `server/share-core`
compiled to wasm for the publish gate, content hashes and Argon2id.

---

## 2. Roles and accounts

| Level | Who | Can do |
|---|---|---|
| `unverified` | Email not confirmed, or password and profile not yet set by the mailbox owner | Nothing; cannot log in. Purged after 7 days. |
| `pending` | Email confirmed, awaiting approval (or approval withdrawn) | See status and any rejection reason, re-apply (7-day cooldown), log out, delete the account, `GET /me/export` (profile only). Pool routes answer `403 account_not_approved`. |
| `teacher` | Admin-approved (and, to use the pool, holding a redeemed invite code; round 28) | Browse, publish, import, report, appeal, counter-notice, export own data. |
| `admin` | Made only by the bootstrap script | All of the above plus queue, suspension, moderation and DMCA routes. |

(`src/index.ts` `TRUST`; `src/pool/index.ts`.)

**Approval.** Every teacher is admin-approved; there is no promotion by email domain (students often hold
staff-domain addresses). A "staff domain" is only a label in the queue. The rule: approve only when the verified
address itself is on the school's staff directory (`directory`), or the school office confirmed the person through
a contact **the admin** started (`school_confirmed`). A name on a staff list is never enough (an unconfirmed
assumption in the owner's note). Sign-up collects email, password, display name, school and an adult-school-employee
statement.

**Invite codes (round 28).** The pool is invite-only: besides admin approval, an account must redeem an invite code an
admin issued (`POST /admin/pool/invites`; 24 characters shown once, stored only as a SHA-256 hash; single use by default
or up to 1,000 uses, optional expiry up to 365 days, revocable; issue, redemption and revocation are audited, never with
the code). The gate sits in the pool, not in sign-up: sign-up stays open and answers the same for every address (the
auth package is unchanged, and a code consumed at sign-up could be burned by whoever typed someone else's address). An
account redeems with `POST /me/invite {code}`, also while `pending`, so the admin can check it before approving
(`GET /admin/pool/invites?user_id=`). Until then every pool route but `GET /me/export` answers `403 invite_required`;
admins are exempt. Every account already `teacher` or `admin` when migration 0106 ran is grandfathered. A bad, expired,
revoked or used-up code is `422 invite_invalid`; 10 tries a day. Revoking a code stops further redemptions only.
Erasure deletes the account's redemption.

A rejection has a reason (shown to the applicant) and a note (never shown); an approval records evidence kind and
notes. Admins can bulk-reject up to 50, revoke a teacher back to `pending`, or suspend. Application mails to admins
are limited to 5 an hour, then summarised; applications waiting over 48 hours are escalated by the daily cron to
`ESCALATION_EMAIL`.

**Admin safety.** Admin routes need a live session, an admin level and a known device; every admin write also needs a
5-minute sudo token from `POST /auth/reauth`. An admin cannot act on their own account; the last active admin can
never be suspended or deleted. Suspending an admin is a proposal another admin must confirm (the proposer alone after
24 hours; proposals lapse after 72 hours), and every action against an admin is mailed to all admins and the
escalation contact. No route creates an admin: the operator runs `server/worker/scripts/make-admin.sh EMAIL --remote`
after verifying the person out of band (`demote-admin.sh` reverses it, never for the last admin).

**Content follows standing.** Others see an item only while its author is `teacher`/`admin`, not suspended and not
deleted (`STANDING` in `shared.ts`). Suspension, revocation and restore hide and show items at once, reversibly;
nothing is deleted.

**Sign-in, devices, sessions.** Passwords: at least 10 characters, NFKC, common-password checks, Argon2id m=19456 KiB
t=2 p=1 (pinned in `src/index.ts`). Login gives a 15-minute access token and a rotating refresh token (30 days
sliding, 90-day family maximum; reuse within 10 seconds returns the same successor, later reuse revokes the family).
Web keeps the refresh token only in an `__Host-refresh` cookie; the native app sends `X-Client: native` and keeps
tokens in the OS keychain. A 90-day "known device" token skips the per-IP and per-network failure limits and the
account lockdown, so students guessing at a shared school address cannot lock a teacher out; otherwise 5 failures per
15 minutes per account-and-network, and a lockdown past 30 an hour or 100 a day. A reset or activation mails a
15-minute enrolment code for the native app. Sign-up, resend and forgot answer the same whether or not the address
exists. Web build and API must share one registrable domain (cookie auth; unconfirmed assumption). (Auth README.)

---

## 3. What is shared and how

An **item** is a bank or a question with a stable id and a sequence of immutable **publishes**. Per item: `visibility`
(`public`/`unlisted`, changed by `PATCH /items/:id`). Per publish: `hide_key`, `license`, attestation, metadata (title,
subject, course, grade, tags, standards) and a client-made `publish_id` (22 to 64 characters; also the idempotency key).

**Flow.** The app builds the payload, uploads pictures privately (`PUT /blobs/:sha256`), runs the dry run
`POST /items/preview` (or `/items/:id/preview`), the author reviews what students will get, then `POST /items` or
`PUT /items/:id` (with `base_publish_id`). The Worker makes **one** share-core call, `publish_prepare`, and stores only
its validated output. Only the server strips; a client never supplies a stripped form. The body is
`application/vnd.vershion.publish` (`<meta JSON>\n<payload>`) with an exact `Content-Length` (411 without, 400
`length_mismatch`); retrying the same `publish_id` and body returns the first result; a lost compare-and-set is
`409 conflict`.

### Hiding the key

With `hide_key`, non-authors get the **stripped form**; the author always gets the full form. Stripping (share-core
README, rounds 6 to 8):

| Kind | Stripped form |
|---|---|
| Mc | `correct` all false; letter labels stripped or kept per decision; unlocked choices shuffled once at publish; locked ones stay put |
| True/False | key removed, question marked keyless |
| Matching | no paired rights; rights and extra words in one shuffled word bank; left items shuffled too unless `keep_order` |
| Blank | accepted answers emptied (blank count kept) |
| Short | model answer removed |
| Multipart | rules applied per part |
| any | explanation removed; figures always removed; a picture kept only if the author ticks "does not reveal the answer" |

The stripped form is its own R2 object. Shuffles are drawn once, so repeated reads cannot be averaged. Only the
publishing author receives `served_orders`; it must never reach a listing, log, report or other account. A hidden
republish that changes nothing students see keeps the stripped body and public version. Without `hide_key`, **every
approved teacher who can see the item sees the answers.** Admins can view the full form of any item. Stems, topics,
name and description are served intact (normalised only), so they can leak a key; the gate below is for that.

### The key-hint gate (share-core)

It protects against hints a careless teacher plausibly types, not deliberate obfuscation (owner decision 2026-10-21;
see section 8).

| Class | Examples | Author action |
|---|---|---|
| **Hard** | ticks, crosses, stars, arrows, emoji marks; key/"wrong" words in about 40 languages; GIFT `=`/`~`, Aiken `ANSWER: B`, Moodle and Anki cloze; check boxes, score tokens; bold/underline/strike or LaTeX quiz commands on the key; export attributes; look-alike letters in a Latin word; answer lines in a stem or topic; another question's key in a bank description or stem; word banks or Matching lists in answer order; `locked_key`; placeholder True/False key in a re-keyed copy | Never acknowledgeable; fix the text. Each refusal has a plain-English `hint`. |
| **Low risk** | capitals, styled letters, trailing punctuation, wrapped choices, a feature on exactly the correct choices (`outlier`), a blank hint like `I ___ (eat)`, a pasted choice list, "Memo:"/"Marks:" rows | Refused unless acknowledged (`ack_formatting`, question indices). |
| **Labels** | "A.", "(a)", "Option A:" on choices | Author chooses Strip or Keep per finding with a preview. Strip only where labels are exactly positions 1, 2, 3...; order-revealing labels cannot be kept on shuffled choices. |
| **Order** | fixed order, kept labels, sibling references ("Both A and B", "All of the above"), fewer than two unlocked choices | Refused until `order_acks` covers each; `order_preview` shows what will move; `keep_order: true` forces an order. |

Notation (shell, Markdown, chess, code, maths) is judged separately so honest content is not refused. Errors that name
the key's choice carry `author_only: true`. In a bank, every served field is read against **every** question's key
(round 17). Scans have work budgets; over budget the text is refused `scan_budget`, never skipped.

**Student-view preview.** For a hidden-key publish the dry run returns `student_view`: the exact stripped payload (stems,
topics, Mc texts in served order all `correct:false`, Matching left items and word bank, part stems and choices, bank name
and description), plus `notes`, `label_findings`, `labels_removed`, `images_removed`, `figures_removed`, `order_reasons`,
`license_check` and `chains`. It never reveals holds or blocks or served orders. The app keeps the final button disabled
until the attestation is ticked, the preview is current and clean, and "I looked at what students will see" is ticked
(`publish_blocker`).

**Attestation.** Every publish and republish carries `attest: {own_work: true, terms_version}`; without it, or with a
terms version other than the server's (`2026-10-22`), the answer is `422 attestation_required` with the statement. Dry
runs do not need it. Wording: "I confirm this content is my own original work, or I have the right to share it under the
licence I chose. It is not copied from a published source (textbooks, publisher test banks, exam boards, paid worksheets)
without permission." It is kept per publish (when, version, who; admin-visible, pseudonymous after erasure). **The server
cannot verify it** and it changes no flag or hold. The Worker also still requires the older
`attestation {accepted, text_rev}` (appendix 1).

**Images.** One still PNG, JPEG, GIF or WebP, 1 to 4096 px a side, at most 2 MB, structure-checked by the Worker (chunk and
segment allow-lists, no metadata or animation, PNG at most 16 Mpix and 64 MB decoded). Every refusal is the same
`422 image_refused {reason:"not_accepted"}`, so a privacy-blocked image looks like a bad one. The app stores pictures as
lossless WebP and keeps only an animated image's first frame; large photos may exceed 2 MB. Server-side decode and
re-encode is not built. Images are served only through the Worker, only in the caller's served form.

### Limits

**Payload** (share-core `Limits::default`; sizing limits can be lowered, never raised): payload 4 MB; 1,000 questions per
bank; stem and explanation 8 KB; answer/pair/distractor/sample/accept entry 2 KB; 26 Mc answers; 50 Matching pairs, 26 extra
distractors; 50 blanks, 20 variants each; 50 parts; 20 topics per question; 30 tags and 30 standards per item (64 characters
each); title/name/author 200 characters; description 2 KB; figure params 16 KB, 256 params, 4 KB per text; points 0 to 1,000;
ids 64 characters; 32 chain links; JSON depth 16, 200,000 values, 60,000 objects, parse budget 20 MB; 16 combining marks per
base; subject/course/grade 100 characters. The Worker caps distinct tags plus topics at 200 per item. Duplicate JSON keys, a
byte-order mark or a second top-level value are refused before parsing.

**Per account, per day unless noted** (`DEFAULT_LIMITS`; admin-adjustable by `PUT /admin/pool/limits`): storage 200 MB
(removed-not-yet-purged bytes another 200 MB); 5,000 shared questions; 50 publishes, 300 refused publishes, 200 dry runs;
100 imports (20,000 distinct items ever; 200,000 distinct publishes ever read); 2,000 item reads, 500 browse pages; 500
uploads and 200 MB; 100 blob misses, 100 id misses; 20 reports, 10 appeals, 3 exports; 20 held publishes; 10 invite-code
tries; 500,000 authorship records. Bursts: 300 requests per 5 minutes per account; 6,000 per 5 minutes per address; 50,000 reads a day per
address, at most 10,000 from one account. One heavy publish parse per isolate (4 may queue up to 20 seconds, then
`503 busy`) and one per account (`409 publish_in_progress`); 72 MB committed to requests per isolate (UNCONFIRMED in real
workerd). A publish reserves 16x its body to the end; its evidence and fuzzy lookups run after its payload trees are freed and
borrow the part of that reservation it no longer uses (round 29: a 3.81 MB hidden-key Matching bank peaks at 61 MB, was refused at
76.6 MB on an idle isolate). Publish-time statements read rows linear in the bank (round 30: five read rows growing with its
square, up to 9.9 million rows and 10 s each at 990 questions on workerd's SQLite; round 31: the earlier-publication lookup when
many questions share a near hash, 26.5 million, and the hourly licence re-key), except two when a bank repeats one question many
times: the evidence and served-units queries (36 and 1 million rows at 990; Worker README "Round 31"). Bodies: `Content-Length` required, read within 15 seconds, at most 4 bodies/8 MB at once per account; JSON 64 KB,
auth 8 KB, admin 256 KB. Caps always refuse; none silently skips a check.

---

## 4. Licences and provenance

**Offered:** `CC-BY-4.0` (suggested, listed first), `CC0-1.0`, `CC-BY-SA-4.0`, `CC-BY-NC-4.0`, `CC-BY-NC-SA-4.0`.
**Nothing is preselected**; Publish stays disabled until chosen; a republish preselects the current licence. The wire value
`circle-only` still exists and is offered as "All rights reserved" for unlisted items only (never public, never re-shared by
importers; appendix 3).

| Origin licence | A republish may use |
|---|---|
| CC0-1.0 | any |
| CC-BY-4.0 | CC-BY-4.0, CC-BY-SA-4.0, CC-BY-NC-4.0, CC-BY-NC-SA-4.0 |
| CC-BY-SA-4.0 | CC-BY-SA-4.0 only |
| CC-BY-NC-4.0 | CC-BY-NC-4.0, CC-BY-NC-SA-4.0 |
| CC-BY-NC-SA-4.0 | CC-BY-NC-SA-4.0 only |

A bank mixing incompatible origins gets `per_question` in its 422; there is no per-question licence.

**Licence windows.** A CC grant is made with a version and is irrevocable for it. Every read or import writes one `served`
row per (reader, publish); a later publish is matched against the **source publish's own** key set (questions, texts, parts,
images by bytes and pixels). The licence history (`content_licences`) counts a licence only while the item **offers** that
content publicly (public, live, not held). A relicence to more permissive terms reaches earlier readers when the public
version serves the same content ("same" = kind, stem, answers as a multiset, explanation after NFC/case/punctuation folding;
never points, difficulty, topics, order, key, ids, dates). A received CC0 stays CC0.

**What binds a publish** (worker README "The licence rule"; `test/pool/licence-cases.test.ts`):
- *Hard* sources can refuse and are named in `bound_by`: the same question whether read or imported; an import the publish
  adapted (a stem reworded by a word or two, Jaccard 0.8 or more, counts as adapted); a declared origin. *Soft* sources (a
  changed copy of something only read) are credited and flagged, never a refusal.
- A **copy**: the earliest-published identical source is the original and decides. An **adaptation**: sources in publication
  order, each narrowing the earlier; one leaving nothing is skipped (`license_conflict`). So import order never matters and a
  contradiction never locks anyone out.
- A republish never refuses a question the author did not touch (`license_grandfathered`). **First** is decided on content,
  never a bare stem; the author's durable stem record (`author_stems`) is consulted so a copier's edit cannot displace her.
- **Stock** never binds: a unit is stock when 3 or more authors published it independently and at least 2 did so before the
  earliest source the publisher was served. Short statements count as common (Tf stem at most 8 words; Short stem at most 8
  and sample at most 12; Blank stem at most 8 and answers at most 12; Mc stem plus choices at most 12; never Matching,
  Multipart or a hidden-key Short/Blank).
- **Earlier publication.** An exact or near copy of something earlier that the publisher was never served credits the earlier
  author (`earlier_match`, badge `unverified_earlier_match`) and flags the admin; credit, not a refusal.
- **Independence claim** `independent_of {items (1 to 50), accepted, statement (10+ characters)}`: the way out for common or
  short questions, changed copies and the author's own question. It never covers an exact copy of anything the account was
  served, nor an imported text or image unless the statement carries attribution words (public domain, CC BY/CC0, Creative
  Commons, OpenStax, Wikimedia, Wikipedia). It is audited; the third claim in 30 days flags the account.
- **Units** (round 17): an explanation or Short answer of 25+ words, an image, or a question wrapped as a Multipart part bind
  on their own, only from a version the account was served, never against their own author, never when stock.
- **Ghosts.** When a publish anyone was served is deleted, retired or purged, a key-only ghost (hashes, ids, licence, credits)
  keeps the licence binding. An erased author's credit reads "deleted teacher".
- Hidden keys never change what others observe (`visible.ts` computes it from key-free hashes). A hidden explanation equal to
  material the author was served under an incompatible licence refuses her publish (`hidden_material`), to her alone.

**Chains and checks.** A copy's chain has one link per item, built by the server. Stored credits: up to 64 contributors; past
that the first 8 and the most recent, with "and N contributors between". Every read, import and browse result has
`license_check {status, basis:"recorded_sources"}` with status `ok`, `mismatch`, `unverified`, `claimed_independent` or
`unverified_earlier_match`. It says only that the licence is **consistent with recorded sources**: a convenience, not legal advice.

**Import is a private copy.** `POST /items/:id/import` (recorded; the author's own import is not) returns the served payload.
The app makes a copy with fresh local ids, keeping source, author, licence and chain. A hidden-key item arrives keyless: no key,
a sentinel explanation, and "Key not provided" on every print and export path.

---

## 5. Pool use

**Browse.** `GET /items?scope=public|mine&subject&kind&tag&standard&license&q&cursor&limit`: newest created first, signed cursor,
20 by default, 50 at most, no totals. `q` matches the start of any word of title, subject, a tag or the question text **as
served to non-authors** (a word index in the main database; at most 5,000 candidates). `GET /items/:id` streams the served form
with provenance and `ETag`/`If-None-Match`; HEAD and 304 record nothing. An item the caller may not see is 404, never 403. The
app has a "Search the pool" tab and a "What I imported" tab; the grade box only narrows the loaded page.

**Import.** Into the bank the person came from (default), another bank, or a new bank (which gets author, licence and a credit
line). Pictures are fetched, hash-checked and stored; a lost picture does not fail the import. The served images are held for
the importer 30 days. `share-imported.json` records item, version, licence, question ids, fingerprints, last check and merge base.

**Update checks.** `POST /items/versions` (up to 200 ids) answers `current`, `newer` (only for an imported copy), `under_review`
(undo window, admin removal, takedown quarantine, any hold, or student data within its grace: keep your copy), `removed` (only
once purged; reason deleted/takedown/privacy/admin) or `unavailable`. The app checks once per window opening when a record was
last checked over 6 hours ago, and on a button. Choices: **Add as a new bank** (safe default) or **Merge**.

**Merge** (`src/share/merge.rs`): starts only when the person presses it. Three-way: BASE (as imported or last merged; at most
500 questions and 1.5 MB per record, else none), OURS (bank question by stored local id), THEIRS (new version, matched by pool
question id). Categories: unchanged; pool-only change (take, default); your-only change (keep); **conflict** (keep mine / use
pool's / keep both; no default, Apply disabled until decided); new in pool (add); removed from pool (keep or remove); deleted by
you (stays deleted); licence changed (credit line changes only on Apply). Apply re-fetches the version, checks it matches what was
reviewed, writes a **restore point** (`backups/banks-before-merge-<date>.json`; restore by closing the app and copying it over
`banks.json`; no in-app restore), applies to a copy, replaces the bank once and advances the base. Any failure leaves everything
unchanged. A typed key is never replaced by the placeholder. Questions added locally or copied from another bank, and records with
no base, can only be updated as a new bank.

**Publish in the app** (`src/share/publish.rs`): Gate (signed in, approved), Options (licence chosen by the person; visibility
default public; `hide_key` default on; metadata; own-work tick; optional independence claim), Checking (dry run), Review,
Publishing, Done; refusals land in Fix (hard findings, acknowledgements, label and order decisions, licence refusal with
`bound_by`) or Failed (limits with `Retry-After`, busy, offline, not approved). An acknowledgement can only be given for something
the server listed. A lost publish keeps its `publish_id`. The last publish of a bank is remembered in `share-published.json`.

**My shared items** (`src/share/mine.rs`, `actions_for`): live public (make private, delete, share again); live unlisted (make
public, delete, share again); deleted within 14 days (undo); taken down (counter-notice while `counter_notice: available`, which
the server now says only inside the 30-day window, with `counter_notice_by`; round 28);
takedown pending (counter-notice until the window ends, once); removed/privacy-removed (appeal while offered); needs re-upload or
key-leak review (share again, delete); held (appeal when offered; share again when a hold names questions). Also the
counter-notice form (nothing prefilled; cleared on send; signature must repeat the typed name, enforced by the app only),
"Download my data" (cap 24 MB in memory; parts for large exports), "Delete my account", and links to the four draft legal pages.

**Reports.** Reasons the Worker takes: `wrong_answer`, `copyright`, `student_data`, `inappropriate`, `other`, `key_leak`. Authors
cannot report their own items; the reporter is told only "thanks". The app's "answer key visible" button currently files `other`
with a text prefix (appendix 2).

---

## 6. Moderation and legal flow

This describes the system. The legal pages are **drafts, not legal advice**; counsel must review them and this flow before a
public launch.

**Reports and holds.** *Student data:* one report from an established account (7+ days old with an import or a published item)
holds the item at once and mails admins; at most 3 instant holds a day per reporter, 50 a day site-wide, and past half that cap
two reporters are needed; younger accounts wait for the admin. *Other reasons:* three distinct teachers' open reports hold it
(`report_threshold`); after an admin release the same reporters cannot re-hide it for 30 days. *Key leak:* hidden-key publishes
only; counts only from an account approved 7+ days that was served that publish a day earlier (1 per item per 30 days, 5 a day);
a hold needs two such reporters on the current publish; it ends when a republish changes the served form or shows the key.

A hold hides the item from others reversibly. The author sees "held for review" with no reporter and, for content checks, no
detail. Admins release with the `publish_id` they reviewed and a reason of 10+ characters. **Blocks** (re-upload holds) cover only
what an admin designated after a preview: a privacy block selects nothing by default; a DMCA block skips material 3+ other authors
use and expires after 60 days by default. A publish matching a block is **held, never refused** (20 held publishes a day); the
rightful author and those served the rightful item are exempt.

**Student-data purge.** Confirmed: hidden, quarantined 14 days (author sees `removed_privacy`, may appeal), then purged; images and
text blocked from re-upload; importers see `under_review`, then removed; copies held. An open appeal stops the purge. Privacy
removals and student-data/DMCA holds are **never restored by default**: past 60 days the appeal escalates and the item stays
quarantined until an admin decides. Other removals auto-restore after 60 unanswered days (flagged). One appeal per removal.

**Copyright takedown lifecycle.**
1. *Notice:* the admin records it (`POST /admin/dmca/notices`) with a tick per 512(c)(3) element; an incomplete notice is `deficient`.
2. *Preview and confirm:* `takedown-preview`, then `takedown` with a one-use `confirm_preview` token (10 minutes).
3. *Disable:* the item leaves browse and search; bodies move to `quarantine/` (admin-only); the author is mailed (outbox, retried
   hourly) and sees "taken down after a copyright notice" with a counter-notice form. An item already deleted and purged follows the
   same machine from its ghost (`takedown_pending`). Importers keep their copy and are told the source was removed. A strike is
   recorded as **pending**.
4. *30-day window* from the author being notified. A counter-notice (`POST /items/:id/counter-notice`: signature, material
   description, name, address, phone, three ticks: perjury statement, consent to jurisdiction, accept service) is taken only while
   the action is open; incomplete is `400`, late `409 window_closed`. `GET /items/mine` offers it (`counter_notice:
   "available"`, with the window's end in `counter_notice_by`) by the same rule, so from the instant the route refuses the
   item says `closed`, before the daily cron has upheld anything (round 28; a ghost takedown carries the same field).
5. *Counter-notice received:* forwarded to the complainant at once (mail if the contact is an email, else the admin forwards it).
   The daily cron restores the item (bodies back, blocks and holds lifted, strike voided) no sooner than the start of the 10th US
   business day after receipt (weekends and federal holidays skipped; the 14th is the stored outer bound), unless the admin records
   a court action (`court-action`), which keeps it disabled until `resolve {restore|remove}`.
6. *No counter-notice:* after 30 days the daily cron sets the purge date and the takedown is **upheld**: item purged, strike upheld,
   the infringing item's evidence voided (links stand for a named rightful item; with none named, copies are held for review),
   re-upload blocks stay, repeat-infringer check runs. An **erased author** cannot counter; the takedown stands after 30 days but,
   with no rightful item named, its evidence is not voided, so the licence keeps binding copies.
7. *Reversal:* `resolve {outcome: put_back}` or `withdraw` voids the strike and lifts blocks and holds; purged content stays purged.
   A put-back is never a strike.
8. *Hold-copies:* admins can hold copies in other items (`hold-copies-preview`, `hold-copies`, paged). A held copy is released only
   if its `license_check` is `ok` after the void and its author is exempt (`exempt_authors`).

**Strikes and repeat infringers.** A strike counts only once upheld. At **3 upheld strikes within 365 days** the system raises a
`repeat_infringer` flag and mails the admins; it does **not** suspend automatically (owner decision). The API exposes no strike
count to authors.

**Appeals.** `POST /items/:id/appeal {note}` for an admin-removed, held or privacy-removed item raises an admin flag; the admin
decides with `appeal-decision {restore|purge}`; weekly reminders while open. Mail contains ids only, never item text.

---

## 7. Privacy and erasure

**Stored:** account (email, name, school, adult statement time, Argon2id hash, trust, approval records), sessions (hashed tokens,
times, user agent), HMAC-keyed rate-limit counters (no raw IP or email); items and retained publishes (current, previous, and the
one non-authors are served) with full and stripped bodies and images in R2; served rows, import counts, usage counters, keyed
authorship fingerprints (never readable text), attestations; reports, flags, DMCA notices and counter-notices (names, addresses,
phones, signatures; admin-only), strikes, appeals, and append-only `pool_audit`/`auth_audit`. Mail goes through Resend or a log
mode (final provider UNCONFIRMED). The web build loads no third-party scripts and sets two cookies.

**Export.** `GET /me/export` returns a JSON attachment of the account's own data: profile, items (every retained version, full
current body with hidden keys), imports and reads, reports filed, takedowns and notices received, attestations, counter-notices,
appeals, sessions. Never another user's personal data, admin notes, flags or hashes. 3 a day; the session must have logged in within
24 hours (`403 reauth_required`). Above 8 MB it returns a manifest (`export_too_large`) and `?part=n` returns bodies in parts of at
most 4 MB. Pending accounts get their profile only.

**Account deletion.** The account stops working at once (password re-entered). PII is nulled (a schema `CHECK`); sessions and device
tokens revoked; the user row stays as a tombstone with an HMAC of the email for **14 days**. The pool has **no re-link route**.
- *Items:* with no choice (`keep_on_delete` null) items other teachers imported are kept anonymised and the rest are removed and
  purged **14 days** later; `PUT /items/prefs {keep_on_delete}` records an explicit yes or no. Items under an open DMCA action follow
  the same date; a deleted account cannot counter-notice.
- *Name:* the author's name on others' copies and chains reads **"deleted teacher"** (shown at once; stored chains rewritten by the
  cron in batches). Licence and credit still apply. The app's publish-review text still says otherwise (open item).
- *Removed at once:* report notes, statements and appeal notes; counter-notice name, address, phone, signature, description;
  preferences; pending uploads, counters, limits, served rows, authorship records; open appeals close (`author_erased`); audit notes
  the account wrote (an admin's note is evidence and stays).
- *Kept:* the bare tombstone; import counts under one anonymous key; audit rows with opaque ids; DMCA and moderation records; kept
  anonymised items; backups until they expire (D1 Time Travel, 30 days). A name typed into a kept item's title or tags is content and
  not scrubbed.
- *Retention (round 28):* audit and takedown records are kept **3 years** (1,096 days), pseudonymous after erasure, then the daily
  cron's `retention` stage deletes them, oldest first and bounded (100 takedown actions and 5,000 rows per table a run). A takedown
  action goes with its counter-notice, strike, exempt authors, block rows and upstream snapshot once it is closed (put back,
  withdrawn, reversed or upheld) and nothing about it is newer than 3 years, unless something still needs it: a re-upload block in
  force (and with it the upstream original's exemption), a copy it holds, an open appeal, or an unresolved counter-notice or
  copies flag on its item. A notice goes once no action names it (an `open` notice stays until withdrawn). A `pool_audit` row goes
  unless its item still has a kept takedown record, a hold or an open appeal, or it names a kept action or notice; the table
  stays append-only for its first 1,096 days by the database's own clock. Evidence voids (`void_items`) are licensing state and
  stay. Not purged: the auth package's `auth_audit` (it would need an auth package change).
- *In-flight-request guard (round 27):* a request authenticated just before an erasure keeps running, so every write it could make
  under the caller's id is conditioned in the same statement or batch on the account not being erased (`ALIVE_SQL`); a skipped write
  answers 401. The daily `erasure_resweep` re-cleans every table such a request could touch, and `erasure_finish` reruns a failed
  erasure batch until `erasure_done` records it.

**Student data.** Pool content is questions, answers, explanations and figures only. Controls: the publish review, the one-report
hold, confirmed purge with re-upload blocks, and a publish-time check: text matching confirmed student data is held (the publisher
sees only "held for review"); a near-rewording is flagged but stays live.

---

## 8. Limits and documented gaps

### 8.1 Share-core (owner decision: ship with these)

The gate does not promise to catch deliberate obfuscation. Grouped from the README's "Documented limits" (rounds 9 to 21):
- **Key leads:** lead words not in the tables ("Answer sheet", "Right answer", "Teacher key", "Scoring guide:"); decorated or qualified
  leads (`**Key:** 1. Paris`, `<b>Key:</b>`, `Key (hidden): Paris`, `Key -> 1. Paris`); look-alike leads of another script; table-cell
  form; exotic row labels (`[1]`, `#1`, `No. 1`, `Q.1`); a key starting past the 200-character window; weak leads (Keys, Marks, Rubric,
  Memo, Marking scheme) in another question's stem or a bank description (low-risk acknowledgement in the question's own stem).
- **Invisible, spacing, script tricks:** an invisible character inside a choice label or "All of the above"; visible spacing marks
  inside a key word; letters spaced or dotted one by one; a diacritic or "..." on the key only; rare look-alike scripts; emoticons;
  length differences; languages beyond the tables; plural or stem forms in Matching.
- **Lists and layouts:** wrapper variants of the rights list in bank texts; Matching separators not read (pipe, tab, leaders,
  "matches", `<->`, `~`); pairs in HTML tables or QTI; a pairing given only by shuffled lefts; word banks over six words/80 bytes or
  over 64 items; a worksheet with one True/False value circled; a key given as data in another question's stem; OCR'd marks.
- **Not read:** initial-letter hints, underscore counts, upside-down text, hidden spans, footnotes, `[[n]]` gaps, references to another
  question by number.
- **Honest-teacher cost:** numeric data tables in numeric-answer banks and descriptions listing answer values are refused often, with a
  workable path (an `S1`-style id; reorder the numbers); a capital letter in brackets ending a True/False statement ("temperature (T)")
  and GIFT-looking musical text (`{F# minor}`) are refused; a contrived Matching shape hits the 50,000-step pair-walk limit
  (`scan_budget`; split the passage). Corpus results: round 17, 627 items, 0 hard refusals, 5 acknowledgements (0.8%); round 21 critics,
  about 1.2 to 3.2 percent hard refusals, each with a workable path (appendix 13).
- **Cost:** a 4 MB Blank bank with 170,000 variants peaks near 27 MB natively (7.4x) and grows wasm memory by 36.3 MB, under the 40 MB ceiling.

### 8.2 Backend

- **Rewording and translation defeat matching** (owner decision); `origin_suspect` flags, reports and takedowns are the control.
  Stem-edit laundering stays an admin flag. A re-upload that adds or drops a distractor or turns the question into True/False is a
  different question to the blocks.
- Several identical sources are all credited, oldest first. A sock's copy of an accomplice's infringing item can shield the accomplice
  from hold-copies; the admin then needs a second notice.
- A determined prober can learn whether a guess matches a block from its own item's held state; the bound is the held cap and admin
  visibility. A short-question copier can claim independence (public and flagged). An author who deletes, purges and re-adds her
  original as a NEW item has no derivation record (the exempt-authors list is the path).
- The licence-history re-key can widen a window across a gap between two offers; a never-served hidden reshuffle no longer retained is
  unrecoverable for pre-0105 rows. An edited word in a copied explanation, an explanation split across fields, a re-encoded non-PNG
  image, or a True/False rewrite with the answer in the stem are not chased.
- A populated pool whose retained bodies are all unreadable cannot prove the hash key and is left unpinned. Memory was measured in node
  only, not workerd.

### 8.3 What a determined teacher can still slip past the gate

1. A key hint share-core does not read (8.1): an exotic marker, an unlisted lead word, a decorated lead, spaced letters, an invisible
   character in a label, a diacritic on the key, an unusual Matching separator. The mandatory student-view preview and the key-leak
   report are the second line.
2. An answer in a picture the author ticked as safe, in a figure, a title or a tag: the server does not read pictures.
3. Someone else's work reworded or translated; the attestation is an unchecked promise.
4. A claim of independence for short, common or read-only material (public and flagged, not verified).
5. Several accounts: admin approval is the only guard (the owner chose no account-age check).
6. Student data not matched by an existing block, until a teacher reports it.

---

## 9. Operations

**Components:** Worker (`server/worker`: pool routes, admin routes, cron, mail, migrations `0100+`); auth package (`server/auth`:
generic `d1-auth`, MIT, migrations `0001-0099`; `package.json` is still `private`); share-core (`server/share-core`, Rust to wasm);
client (`src/share/`); legal drafts (`server/web/legal/`). Staging: Worker and D1 `vershion-pool-staging` (`wrangler.staging.toml`,
`workers_dev = false`, log-mode mail). Never use or kill port 8799 (the owner's own `wrangler dev`); local workerd uses 18791/18792.

**Config.** Secrets: `AUTH_SECRET` (32+), `RESEND_API_KEY`, `POOL_HASH_KEY` (32+). Vars: `ALLOWED_ORIGINS`, `APP_URL`, `MAIL_FROM`,
`ADMIN_EMAIL`, `ESCALATION_EMAIL` (comma lists), optional `APPROVAL_DIGEST=1`, `MAIL_MODE=log` (local only). Bindings: D1 `DB`,
private R2 `POOL`. The Worker is served only on a custom-domain route; `wrangler.toml` still holds a placeholder database id and
origins. Argon2id needs the paid Workers plan. Client release builds need `VERSHION_POOL_URL=https://api.<domain>` (`build.rs`
rejects anything else); never ship `pool-custom-server`. Mail goes through a small adapter (Resend over HTTPS, or log); an
SMTP-capable adapter for the owner's own server is wished for and **not built**.

**`POOL_HASH_KEY` is permanent.** Every identity hash (content hashes, fingerprints, sketches) is made under keys derived from it
(HKDF-SHA256, salt `vershion-pool/v1`). Rotation is unsupported: the pool pins the key's id on first use and any write or cron under
another key fails closed with `503 pool_key_mismatch` and one admin alert (reads continue). Without the secret, `/items` and `/blobs`
answer `503 pool_not_configured`. Keep it as a wrangler secret and an offline copy. `GET /admin/pool/key` shows the ids.

**Migrations.** `0001-0005` (auth, copied by `scripts/sync-migrations.sh`; `npm test` fails on drift), `0100_staff_domains`,
`0101_notice_budget`, `0102_pool`, `0103_pool_rounds_6_11`, `0104_pool_round12`, `0105_pool_round24`, `0106_pool_round28` (retention
trigger and indexes, invite codes; applying it starts the invite gate and grandfathers every account approved then),
`0107_pool_round30` (index `author_stems_qhash`; no data change; round 31 needed none). A committed migration is never edited;
the next is `0108`. Before applying `0104` to a database with data, export it (D1 Time Travel keeps 30 days); it has no down
migration.

**Cron** (`17 * * * *`; the 04:xx UTC run adds the daily stages). Hourly: `staged`, `pending`, `gc`, `reconcile`, `r2_orphans`,
`outbox`, `privacy_backlog`, `deletion_rewrites`, `quarantine`, `backfill`, `stem_backfill`, `licence_rekey`, `offer_sync`, `d1_size`,
`wasm_traps`, plus the sign-up notice summary. Daily (also auth `purgeExpired` and `reviewReminders`): `putbacks`, `ghost_upholds`,
`restored_holds`, `content_licences_gc`, `putback_finish`, `repeat_infringers`, `quarantine_expiry`, `erasure_finish`,
`erasure_resweep`, `account_deleted`, `appeals`, `privacy_bytes`, `ghost_gc`, `purges` (batches of 200, up to 50 batches or 20 s),
`retention` (round 28), `cleanup`, plus the hourly stages again. Stages are isolated; a failure is flagged and retried.

**Verification.** Run from the component directory.

| Component | Commands | Result (re-run 2026-10-23) |
|---|---|---|
| share-core | `cargo test --release` (the amplification sweep can flake under load: rerun alone); `cargo clippy --release --all-targets`, also with `--target wasm32-unknown-unknown`; `bash scripts/check-wasm-exports.sh`; `node tests/node/check.mjs`; at the repo root `cargo test --release --test share_core_fixtures` | cargo 309 passed, 0 failed, 1 ignored; node check 506/506; fixtures 8/8. clippy and export check not re-run (UNCONFIRMED) |
| Worker | `npm test`, `npm run typecheck` (editor diagnostics on test files are stale) | `npm test` 507/507, 0 failed (round 28: 477 plus `round28.test.ts`'s 12; round 29: plus `round29.test.ts`'s 7; round 30: plus `round30.test.ts`'s 5; round 31: plus `round31.test.ts`'s 6); typecheck clean |
| Auth | `npm test`, `tsc` | 224/224 across 18 files. tsc not re-run |
| Client | `cargo test --lib share::` and the library suite | not re-run; brief says 1,038 library tests, with two figuresui tests failing in the owner's files (UNCONFIRMED, appendix 18) |

The Worker harness imports `../../share-core/pkg/share_core_bg.wasm`: rerun the Worker suite after any share-core rebuild.
`POOL_STRICT_TIMING=1` restores tight CPU bounds (never judge timing under load).

**Wasm sizes.** share-core `pkg/share_core_bg.wasm`: 1,289,686 bytes (built 2026-10-23 05:11). Web app wasm after
`trunk build --release`: 15,600,951 bytes (14.88 MiB) as of 2026-10-20 (`docs/progress.md`), against Cloudflare Pages' 25 MiB file
limit (not re-measured; UNCONFIRMED now).

---

## 10. Owner decisions

Verbatim from `docs/handoff-shared-pool.md` ("My decisions"):

> Cloudflare Workers + D1 + R2; email+password; every teacher needs admin approval (invite-only pilot first; DMCA agent registration ($6, renew every 3 years) + lawyer review before any public launch); no circles at first launch (public pool only); limits 200 MB storage + 5,000 shared questions per teacher; 3 upheld strikes/12 months = admin flag + mail (no auto-suspend); deleted teacher's name shows "deleted teacher" on others' copies; audit/takedown records kept 3 years; terms changes 30 days' notice by email; liability US$100, no indemnity; publish attestation (own work, not from a published source) with mandatory checkbox in the client; self-service data export (GET /me/export + app button); API at api.<my domain> (domain not chosen yet), /account/* on Cloudflare Pages same domain; mail via Cloudflare email OR my Stalwart server on my VPS (keep behind an SMTP-capable adapter, undecided); auth package to be published open source under a permissive licence (MIT/Apache-2.0, needs README + security contact); POOL_HASH_KEY permanent, back up in password manager + one offline copy; images stored as lossless WebP (large for photos; may revisit); stem-edit laundering stays an admin flag; rewording/translation defeats matching (documented limit).

Also recorded (owner's note and handoff): Argon2id m=19456 t=2 p=1 via RustCrypto in share-core; the keyless form is stripped
server-side only; CC-BY suggested but nothing preselected; the common-question rule relies on admin approval of every teacher (no
account-age check); the key-hint gate protects against plausible markers only, with a mandatory student-view confirm and a key-leak
report (exit rule: zero misses on plausible markers, at most 2% acknowledgements, 0 hard refusals on honest corpora); a critic finding
is a blocker only if it is a new class; on 2026-10-23 the owner approved one more narrow round and then shipping with documented limits.

---

## 11. Open items before a public launch

1. **Domain:** choose it; API at `api.<domain>`, `/account/*` and `/legal/` on Cloudflare Pages under the same registrable domain; set
   `VERSHION_POOL_URL`; add the custom-domain route and real D1 id and origins to `wrangler.toml`.
2. **Mail provider:** Cloudflare email or the owner's Stalwart server; build an SMTP adapter if Stalwart; confirm sender setup.
3. **DMCA agent and a lawyer:** register the agent ($6, renew every 3 years), publish its name and address, and have counsel review the
   Terms, Privacy, Copyright and Acceptable Use pages and section 6.
4. **Legal pages hosting:** serve `server/web/legal/` at `/legal/`, keep the Worker from capturing `/legal/*`, fill the placeholders
   (operator, contact, agent, governing law, venue), link the pages from sign-up, the sharing panel and the account footer. The app
   builds its links as `https://<configured server>/legal/{terms,privacy,copyright,acceptable-use}.html`, which assumes the API host
   serves them; the Worker does not.
5. **Counter-notice wording review:** the statement text shown by the app is app-supplied draft text (the Worker takes only three
   booleans); counsel should review it and whether the signature must equal the typed name.
6. **Auth package as open source:** MIT `LICENSE` exists; `package.json` is still `private`; add a security contact and finish the README
   for third parties.
7. **Key backup:** back up `POOL_HASH_KEY` in a password manager plus one offline copy before the first publish.
8. **Staging memory measurement:** deploy to staging (owner's OK first), set `POOL_HASH_KEY` there, measure real workerd memory against
   the 72 MB budget and Argon2id CPU on the edge.
9. **Client items in the handoff (UNCONFIRMED whether done):** model fields `keyless`/`origin`/`rekeyed` (12-file owner patch, tell the
   owner first); the `"___"` blank-count mismatch between `src/ditto.rs` and share-core; Anki `{{cN::x}}` import as a blank; the
   publish-review text for "deleted teacher"; a "Shared" badge on bank pages; a picture preview before import; image decode and
   re-encode in share-core; a visual check of every window (only headless frames were run).
10. **Policy gaps found while writing:** a strike count for authors (decision pending). (The 3-year retention job and the
    counter-notice window in `/items/mine` were added in round 28; the auth package's `auth_audit` is still kept indefinitely.)
    **Client (round 28):** the app needs an invite-code entry (`POST /me/invite`, also while pending) and a reading of
    `403 invite_required`; it may show `counter_notice_by` for a taken-down item.
11. A whole-phase critic pass after the above, then a final run of all suites on the final build.

---

## Appendix: doc/code discrepancies found

Each item names the disagreeing sources and the one this spec follows (the code in every case).

1. **Attestation fields.** Worker README route table: `attestation {accepted, text_rev}`; its Round 23 text: `attest {own_work,
   terms_version}`. Code (`server/worker/src/pool/publish.ts`) requires **both**: `attestation.accepted` with `text_rev` `2026-10-19`
   (else `400 invalid_request`) and `attest` with `terms_version` `2026-10-22` (else `422 attestation_required`). The client sends both.
2. **Key-leak report reason.** The client files "answer key visible" as `other` with a note starting `Answer key visible: `
   (`src/share/pool.rs`, comment "the Worker has no such reason"). The Worker has `key_leak` in `REPORT_REASONS` with its own
   qualification, flag and two-reporter hold (rounds 17/18, `moderation.ts`). The app's button never reaches that path; an `other`
   report needs three teachers. The client needs a change.
3. **Circles and `circle-only`.** Owner: no circles at launch; the draft Terms list five licences. Code keeps the wire licence
   `circle-only` (offered as "All rights reserved" for unlisted items; since round 32 also a circle bank's "members may not
   re-share", D19) and the approval mail says the person can "join circles" (`server/worker/src/mail.ts`). The circles backend
   (phase A) exists since round 32; the Terms and Privacy drafts do not describe circles yet (plan section 11, D9).
4. **3-year retention.** Owner decision and the draft Privacy page say audit and takedown records are kept 3 years. Resolved in
   round 28 for the pool's records (the `retention` cron stage, section 7); the auth package's `auth_audit` is still kept
   indefinitely.
5. **Ratings, favourites, search service.** The old spec decided a separate FTS5 database, circles, ratings and favourites; none exist
   (search is a word index in the main database). The draft Privacy page still mentions "favourited or rated" and deleting "ratings and
   favourites"; the draft needs trimming.
6. **Imports per day.** Old spec 200; code and worker README 100.
7. **Strike outcome.** Old spec: three upheld strikes suspend the account. Code: a `repeat_infringer` flag and mail; no suspension. The
   365-day window is counted on `strikes.at` (when recorded), not `upheld_at`.
8. **Account deletion.** Old spec: the email HMAC lets an admin re-link items and the display name stays; `cleanup.ts` has a TODO. Code:
   no re-link route, name becomes "deleted teacher", `deleteContentStatements` does the work.
9. **Export.** The legal README says the full export is "not yet implemented as described". `GET /me/export` and the app's "Download my
   data" exist; the Privacy draft still has "[CONFIRM when shipped]".
10. **Handoff status lines.** The handoff's "Rust client" bullet and "Remaining work 1" say the UI is not started (and "180 tests"); its
    later sections describe the publish, browse, merge and My-shared-items slices as built. It also gives 464, 466 and 477 for the
    backend in different places. The 477 matches the run below.
11. **Next migration.** Worker README "Setup": "Add `0105_...` for the next change", but `0105_pool_round24.sql` exists. Fixed in
    round 28: `0106_pool_round28.sql` exists and the next is `0107`.
12. **Publish retention.** Old spec: current plus one previous. Code (README round 12 A1): current, previous, and the publish non-authors
    are served (`public_publish_id`).
13. **Honest-teacher exit rule.** Owner rule: 0 hard refusals on honest corpora; final critics measured about 1.2 to 3.2 percent honest
    hard refusals, each with a workable path, and the owner then approved shipping with documented limits.
14. **Counter-notice availability.** Resolved in round 28: `GET /items/mine` says `available` (with `counter_notice_by`) only inside
    the 30-day window, by the rule the `POST` refuses with (`409 window_closed`).
15. **Section names cited elsewhere.** Both READMEs, code comments and the legal README cite old-spec sections by name ("Sharing model",
    "Blob rules", "Trust levels", "Limits", "Takedown"). Those now point at the history copy. A citation sweep is pending.
16. **Refresh-token lifetime.** Legal drafts say 30 days; auth is 30 days sliding with a 90-day family maximum.
17. **Tag limits.** share-core: 30 tags and 30 standards per item, 20 topics per question; Worker: 200 distinct tags plus topics per item.
    Both apply, at different layers.
18. **Client test count.** The owner brief says 1,038 library tests; a recorded run in the session scratchpad listed 341 `share::` tests with
    700 filtered out (1,041). Not re-run; UNCONFIRMED which is current.

### Verification run

Re-run while writing, 2026-10-23, on the working tree as found (uncommitted changes included): worker `npm test` 477 pass, 0 fail, 0 skipped;
auth `npm test` 18 files, 224 pass; share-core `cargo test --release` 309 pass, 0 fail, 1 ignored; `node tests/node/check.mjs` 506/506 checks.
Not re-run: clippy, `check-wasm-exports.sh`, typecheck/tsc, the client library suite, the web wasm size.
