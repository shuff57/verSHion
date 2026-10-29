# HANDOFF 2026-10-27 (late evening): circles phase D1 — critics 101/102 ran, scoped fixes in, awaiting the re-check

Repo `verSHion`, branch `shared-pool-phase1`, last commit is the round 19–20 checkpoint. This file was rewritten after the round-19 takeover session (`vershion-fd`, `240c6322`) lost its builder to the weekly API limit. The limit was due to reset **2026-10-28 16:00 PT**; the owner lifted that gate early, so the critic pair ran on the evening of 2026-10-27 (see the scoped fix round below).

## Phase D2 backend: per-question change lists (2026-10-28, commit 21d66aa)

The route the plan defers to D2 is in: `GET /circles/:cid/banks/:bid/changes` (a member who may read the bank)
returns the kept, readable versions newest first, each compared with the next one below it, from the per-version R2
indexes the server already keeps — no migration, no share-core change (`circleversions.ts` `changes()`, the index
reader exported from `circlesend.ts`). Rules pinned by `test/pool/round54.test.ts` (5 cases, each killed by a
mutation: the withheld filter, the hiding-bank refusal, the 40-index cap, pruned versions as a base):

- withheld and pruned versions are absent and never a `compared_to`; the comparison skips to the next readable kept
  version;
- at most 40 indexes are read (two in memory), so the oldest entry in the window is left out — it has no base inside
  the window, and reading one more would break the cap;
- a missing index is `{version, compared_to, gap: true}`, never an error, and the entry that would compare with it is
  a gap too;
- a viewer of a bank that hides its keys gets `{per_question: false, reason: "keys_hidden"}` and nothing is read or
  counted for that answer (their compare is the stripped read, Q4);
- `ETag` over the head version id and the readable kept versions; a matching `If-None-Match` is `304` and costs
  nothing; a served read counts one `item_reads` and one `circle_change_reads` (new metric, 200 a day — the plan's
  proposal, still an unconfirmed number).

Measured locally (MemR2, 990-question bank, median of 20): **19.9 ms at 60 versions, 17.0 ms at 1,000**, 19 entries
both. `EXPLAIN QUERY PLAN` is `SEARCH circle_versions USING INDEX sqlite_autoindex_circle_versions_3 (bank_id=?)` with
no temp B-tree, so the unique `(bank_id, version)` index supplies the order and `LIMIT 40` stops the scan. The D1
`meta.rows_read` figure the plan asks for cannot be taken on the local harness; the local plan bounds it at ~40 index
rows, and it should be re-taken on staging when D2 closes. Worker `npm test` **704/704**, typecheck clean.

**Still to do for D2 (client, 2-3 rounds):** the per-question history panel built from these lists plus the existing
history lines; side-by-side compare of two versions of one question (viewers of a hiding bank: the stripped reads,
Matching word banks compared sorted); "Restore this question to version N" as a local edit that writes a restore point
first and then counts as one unsent change; hook H3 (`question_menu`) needs the owner's approval before it is touched.

## Phase D2 client: per-question history, compare, restore (2026-10-28, overnight loop)

Owner approved hook H3 on 2026-10-28 and asked for the rest to be finished overnight; the previous session closed, this one took over.
Loop rules: local commits only, never push; critic pair (product vs Moodle, security vs ASVS) per round until both are clean on the same build; stop and leave a note here on a new class of critic finding, another protected file, staging/deploy, or privacy/terms wording.

- **r1 `1ec39ff`** models, no UI: `Pool::circle_changes(_if)`, timeline, compare, body cache, `restore_question` (restore point first, one unsent change, edit links only). Share suite 1425.
- **r2 `dce14de`** UI: `question_history_ui.rs` page drawn in place of the grid (new `EditView::Question`, additive edits in `circle_edit.rs`, `circle_edit_ui.rs`, `bank_links_ui.rs`), the restore applied from `CircleEditModel::sync`, and hook H3 (one line in `bankui.rs` `draw_card`). Share suite 1439, 7 mutations killed. Frames: `d2frames/` in the session scratchpad, rendered by the ignored test `d2u_frames` (`D2FRAMES=<dir> cargo test --lib d2u_frames -- --ignored`).
- **Critic round 1** (product vs Moodle, security vs ASVS): 1 blocker (cached full bodies and lines surviving the server hiding keys or withholding a version), the H3 line committed in the wrong branch of `draw_card` (a zero-context patch against shifted lines; fixed in its own commit, now guarded by a test that reads `bankui.rs`), plus 10 important findings. **Fix round `r3`** fixed them (cache keyed by account/role/hide state and dropped on any change, caps on the `/changes` reply, restore warns about unsent edits and offers Undo, removed-question restore, Multipart/picture/figure compare, confirmation under its row, 22 frames at 420 and 1280). Share suite 1467, 20 mutations killed. M6 (bank written before link) kept deliberately; reason in the comment at the sync hook in `circle_edit.rs`.
- **Critic round 2** (fresh pair on `79a0baa`): no strict blocker, but near-blockers around the new Undo restore (stale whole-link write-back, overwriting later edits), an open inline-editor draft overwriting a restore, a restore the strip cannot count, a possible cross-account read race, and about 10 important product items (no preview, stale compare, Multipart/figure compare). **Fix round `r4`** fixed them: Undo patches only its own link entries and refuses when edited since, the page refuses to open while the bank has an open editor draft, read round counter dropped on reset, figure-only restores refused with a plain message, "Show" preview per version, compare reuses merge's `Sig`. Share suite 1487, 33 mutations killed. Skipped: picture thumbnails (label only), one rig-data mismatch in frame 19.
- **Critic round 3** (fresh pair on `51c4e83`): a narrow blocker-class hole (a restore could slip past the figure-only refusal and leave the strip saying "up to date"), a per-frame re-read loop on a failing version, the open-editor-draft guard only stopping the page opening (not a restore), Undo failure/retry/dismiss problems, Show spinning forever or showing a keys-stripped body as complete, an unbounded history reply, and a flaky thread test. **Fix round `r5`** fixed all of them. Share suite 1504 (stable over repeated runs), 44 mutations killed.
- **Owed outside `src/share/` (not done, needs the owner's yes):** `src/qedit.rs` `commit_draft` should refuse (or ask) when the question's `modified` in the store is newer than when the draft was opened (one new `opened_at` field on `Draft`). This is a second layer for writers outside the history page; the history page itself now refuses restores under an open draft.
- **Open layout deviation:** plan 4.8 wants two lines per version at 420 px and a stacked compare as "Version A block above Version B block"; rows take three lines and the compare is one block per field (round 3 asked for the field name not to repeat). Owner's call.
- **Critic round 4** (fresh pair on `80dd3d1`): no blocker; important: (N1) NotCountable still bypassed for a locally DELETED question and falsely refusing a restore of the base version over a stem+figure edit; (N2) a circle picture the store re-encodes (PNG/JPEG -> WebP, new file name) reads as "a different picture" on a teammate's copy, so compare, the header and "already the same" are wrong (the strip is right); (N3) a confirmed restore can still be applied unseen in the first frame after leaving the page (sync also runs from the Shared page and other banks' strips), losing Done/Undo; (N4) Undo dropped on a role/hide change while still on the page; (N5) the difficulty pass can overwrite a restored question's difficulty. Probes: scratchpad `copy6/` (`zz_critic_*`) and `copy7/` (`zcrit_*`) under `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/63f2b0d4-500a-4288-83db-19bdb8133e9a/scratchpad/` (may be gone; re-derive from the descriptions).
- **Fix round r6 was NOT done.** The builder agent hit the 5-hour API limit (reset 11:10am PT) before saving anything; the tree is clean at `80dd3d1`, share suite 1504 passing. Next step: send the N1-N5 list (plus the "should fix" items: Show disabled while a restore is asked, cap `by`/`message` length at parse, no same side on both compare pickers, dedupe restore points, report a keyboard route for H3) to a builder, test first, then a fifth critic pair. Loop exit: a product and a security critic both clean on the same build.
- **Owner decisions parked:** `src/qedit.rs` `commit_draft` refusing when the question moved since the draft opened (one `opened_at` field on `Draft`); plan 4.8 layout at 420 px (rows three lines not two; compare one block per field); staging re-measure of `/changes`; owner's look on a real screen (frames are rasterized, light theme only); picture thumbnails in Show; focus after Yes/Undo/Show.
- Not yet seen on a real screen; the owner's look is still due before D2 is called done. Staging re-measure of the `/changes` route (`meta.rows_read`) is still owed and needs the owner's yes.

### Round 9 critic pair → r11 (2026-10-29)

- **Round 9 pair** on `aac85dd` (r10): product FINDINGS (2 important, 1 minor), security FINDINGS (2 important,
  2 minors). Reports: `docs/critic-reports/2026-10-29-d2-r10-critic-product.md` and `-security.md`.
- **r11** fixed them, test-first: (1) a merge records the new base version's figure and sets `figure_recorded`,
  including a pre-r10 link's `Unchanged` rows; (2) after a whole-bank send the picture map is cleared, a partial send
  judges the keep from the composed records, and a merge from the full merged base, so a stale name never overrides
  the sent file's own; `picture_files_kept` is one pass (was about 127 M pair checks on a 990-question bank);
  (3) one unreadable link field no longer drops the link (`viewers_form` reads the unsafe `true`); (4) a gone link
  offers no History item and `open_question_history` refuses it.
- Suite on r11: **share 1559/0/3; full lib 2254/0/6**. Both critics' probe sets replay with the defect
  demonstrations inverted and every finding probe green.
- Next: a fresh round-10 pair (product + security) on r11; the loop exits when both are clean on the same build.

### Round 10 critic pair → r12 (2026-10-29)

- **Round 10 pair** on `9d926bf` (r11): product FINDINGS (1 important, 2 minors), security FINDINGS (1 important,
  2 minors). Reports: `docs/critic-reports/2026-10-29-d2-r11-critic-product.md` and `-security.md`.
- **r12** fixed them, test-first: (1) a partial send carries the held-back question's old base-record
  `local_image`/`picture_now` into the rebuilt record (falling back to the link map when the base was bounded away),
  so a re-encoded picture no longer reads as an unsent change or "a different picture"; (2) the merge's picture-name
  keep is judged against the base that is actually STORED, not the full merged base that `bounded` then discards
  (r11 had regressed this on large banks); (3) an unreadable `seen` reads the safe `hide_key = true`, while a missing
  `seen` stays "nothing known"; (4) a distinct `RestoreRefused::FigureNotRecorded` gives the truthful reason when an
  earlier build's link has no figure record.
- Suite on r12: **share 1561/0/3; full lib 2256/0/6**. Replayed probe sets: the product's P1 is green; its P2/P3
  probes invert (they documented the old wording/default); security's merge-order probe fails only because it
  hardcodes the old r11 sequence, while its pre-r11-order twin (now the code's order) passes.
- **Open minor, recorded:** an unreadable non-scalar link field (`base_version`, `linked_at`, `is_author`,
  `author_known`, `difficulty_read`, `title`, `base`, `base[].local_id`, `base[].snap`, `base[].snap.points`) can
  still drop that link. Same privilege as editing the banks; fix if the leniency is ever extended.
- Next: a fresh round-11 pair (product + security) on r12; the loop exits when both are clean on the same build.

### Round 11 critic pair → r13 (2026-10-29)

- **Round 11 pair** on `6777002` (r12): product FINDINGS (2 important A/B, 1 minor D), security FINDINGS (1 important
  I1, 1 minor M1). Reports: `docs/critic-reports/2026-10-29-d2-r12-critic-product.md` and `-security.md`.
- **r13** fixed them, test-first: (1) the send summary and composer compare a picture by its circle identity, not
  the local file name; a fetched picture kept here re-encoded is not listed as a change, and it is sent under the
  base body's picture identity so the server does not rewrite the circle's picture; `after_partial_send` records the
  identity the send used; (2) `reencoded_pairs` keeps `(file, circle sha)` when `bounded` discards the base, in
  `from_head` and `finish_merge`, so a large added bank's fetched pictures still have a name; (3) the partial-send keep
  is judged against the records actually stored — when bounding discards the composed base, only questions the send
  carried as this bank's own silence the map; (4) the carry only reuses a record of the same base picture.
- Suite on r13: **share 1567/0/3; full lib 2262/0/6**. Replayed probe sets: the product's A/B probes are green (B's
  probe fails only on its old “an Add starts with no map” precondition), the security I1 probe is green, and the
  demonstration probes for the old behaviour invert as intended.
- **Open minor, recorded:** D — an unreadable `seen` (hide_key = true) overrides a fresh `per_question: true` reply
  and the panel states the hiding as fact; suggested follow-up is to let the fresh reply win or hedge the sentence.
- Next: a fresh round-12 pair (product + security) on r13; the loop exits when both are clean on the same build.

### Round 12 critic pair → r14 (2026-10-29)

- **Round 12 pair** on `494db7c` (r13): product FINDINGS (1 blocker, 1 important, 1 minor), security FINDINGS
  (1 important, 4 minors). Reports: `docs/critic-reports/2026-10-29-d2-r13-critic-product.md` and `-security.md`.
- **r14** fixed them, test-first: (1) the composer carries `picture_on_circle`, the identity it actually sent;
  `after_partial_send` records that as `snap.image` and only a question sent under its own file hash may silence the
  link map (a bounded base keeps held-back and substituted names); (2) a substituted question records the base
  body's file in `base_image`, so the send page fetches the circle's file the payload names (no `MissingMedia`); (3)
  `same_picture_on_circle` lets the lost-answer matcher recognise a picture sent under the circle's identity; (4)
  `from_head` caps the map to `PICTURE_FILES_MAX`, newest kept; (5) a removed picture records none; a `picture_now`
  record now names this store's file directly.
- Suite on r14: **share 1572/0/3; full lib 2267/0/6**.
- **Open minors, recorded:** the map is keyed by file, so one local picture with two circle names reads the second
  as changed (a per-question name field is the real fix); an unreadable `seen` over-hides and states it as fact; an
  unreadable non-scalar link field can still drop that link.
- Next: a fresh round-13 pair (product + security) on r14; the loop exits when both are clean on the same build.

### D2 client loop CONVERGED (2026-10-29)

- **Round 13 pair** on `83469e3` (r14): product **CLEAN** (0 blockers, 0 importants, 3 minors), security **CLEAN**
  (0 blockers, 0 importants, 0 new minors). The loop's exit condition — a product and a security critic clean on the
  same build — is met. Reports: `docs/critic-reports/2026-10-29-d2-r14-critic-product.md` and `-security.md`.
- **Leftovers recorded, not gating:** (1) the failed-fetch sentence says "a change you are not sending", false for a
  ticked question whose picture was substituted (`pool.rs:133`); (2) a base picture the circle will never serve (a
  purged/blocked blob) is offered only as Try again, with no way forward after a 404; (3) every partial send
  re-downloads and re-uploads every untouched re-encoded picture — keeping the fetched original under its circle name
  would avoid it; plus the earlier open minors: the file-keyed map collision (two circle names for one local picture),
  an unreadable `seen` over-hiding and stating it as fact, and unreadable non-scalar link fields dropping that link.
  The strip still compares pictures by local file name (`merge::ours_delta`), a recorded residual.
- **Still owed before D2 is called done:** the owner's look on a real screen (frames are rasterized, light theme only);
  the staging re-measure of `/changes` (`meta.rows_read`) and the partial-send picture round-trip — both need the
  owner's yes; D3's remaining pieces and its staging check per the plan.
- The figures gap-fill work remains paused in `/home/shuff57/Documents/GitHub/verSHion-figures` (phase 4, fixes staged
  but uncommitted, verification critic not yet run) — see its own HANDOFF.md.

### The three recorded minors fixed (r15, 2026-10-29)

- **(1) Wording.** `BaseMediaUnavailable` no longer says "a change you are not sending" (false since r14's fetch also
  runs for a ticked, substituted question): it now says a picture this send needs could not be fetched.
- **(2) A 404 has a way forward.** A new `LocalProblem::BaseMediaGone { file }` (no retry) names the picture and says
  it may no longer be kept, or the person may no longer be able to read it, and points at Get latest or removing it.
  Transient failures keep the plain line and Try again.
- **(3) No re-download every send.** `MediaSource::keep` (`store::restore_image`) stores the fetched circle original
  under the exact name the payload uses, so later sends find it locally. Disk cost: one original per distinct circle
  picture the bank references.
- Suite on r15: **share 1573/0/3; full lib 2268/0/6**. A verification pair runs on r15; the loop stays converged if
  both are clean.

### Round-14 verification of r15 → r16 (2026-10-29)

- **Product and security verification** on `01b976e` both found the same important: the `BaseMediaGone` arm was
  unreachable. `Pool::call` turns a real non-2xx answer into `PoolError::Refused`, not `ApiError::Http`; the r15 test
  injected the impossible shape while the older real-shape test still asserted Try again. Reports:
  `docs/critic-reports/2026-10-29-r15-verify-product.md` and `-security.md`.
- **r16** matches `PoolError::Refused(r) if r.detail.status == 404` inside the blob-fetch loop only (sign-out first,
  429/5xx transient), names the picture and the question's stem from the payload, and drops the unreachable "get the
  latest" advice ("Remove or replace that picture, then send again"). The tests now drive a real 404 `Response`; the
  transient retry test uses a 500.
- **r15's "keep the fetched original" minor is DECLINED and reverted (security M1):** keeping the circle author's
  verbatim PNG/JPEG/GIF/WebP bytes under `media/<sha>.<ext>` preserves EXIF/GPS, ICC, comments and trailing bytes on
  every member's disk and in backups; the store used to keep only a stripped WebP. The per-send re-download (and the
  re-upload) residual is accepted and recorded.
- Suite on r16: **share 1573/0/3; full lib 2268/0/6**.
- **Open hardening, recorded:** the fetch loop's `media.read(file)` runs before the name is validated, so a crafted
  `image.file` is an existence/timing oracle (its bytes cannot be uploaded).
- Still owed before D2 is called done: the owner's real-screen look; the staging re-measure of `/changes` and the
  partial-send picture round-trip (owner's yes); D3's remaining pieces.

## The 5 known failures are fixed (2026-10-28)

The owner asked to fix the five `share_core::wasm::publish_prepare_json` failures. Cause: the viSHion path deps in
`Cargo.toml` turn on `serde_json`'s `arbitrary_precision` through feature unification, so `serde_json` hands a number
to `deserialize_any` as a one-entry map (`$serde_json::private::Number`); share-core's `Read::visit_map` skipped it at
a `Scalar` shape, so `points: 1.0` became `Skipped` and the validator said `expected_number`. Fix in
`server/share-core/src/json.rs`: a `Shape::Scalar` map that is that private-number form is read back into `Node::Num`
(the text charged to the budget); inert when the feature is off, so the worker's own build is unchanged. The owner
authorised this one edit outside `src/share/`.

`cargo test --lib`: **2099 passed, 0 failed, 4 ignored** — the five are fixed (later rounds: 2108/0/4). share-core's
own behaviour tests pass; its `critic24b_scans_are_linear` (an absolute <2.0 s bound) fails on this loaded box at ~3.0 s
both with and without this fix, so that one is environmental, not a regression. `cargo build --bins` and the wasm32
check green.

## Confirmed accounts may join circles (owner decision, 2026-10-28)

The owner decided: a **confirmed** (email-verified) account still awaiting its first approval decision may join a
circle with a circle code and act there **by its role** (viewer reads/copies; editor sends/merges; owner
invites/owns). The **public pool stays approval- and invite-gated**, and creating a circle stays behind approval.
Rejected applicants and revoked teachers are refused — their trust is also `pending`, so the gate checks the latest
approval decision (`awaitingFirstDecision`). A confirmed pending account may also upload the images a circle send
needs (`PUT /blobs/<sha256>`); `/items/*` stays gated.

Implementation: `server/worker/src/pool/index.ts` (`pendingCirclePath`, `pendingBlobPath`, `awaitingFirstDecision`,
the invite-gate exemption on circle paths for every account — a circle code, not a pool invite, admits to a circle);
`circles.ts` `IN_STANDING_SQL`/`AUTHOR_OK_SQL` count `pending`; the student-data instant hide still needs an approved
reporter. Client: a pending account sees only the Circles and Settings tabs, may start a circle send (never a public
publish), the linked-bank strip treats it as signed in, and background circle work runs for it; "Create a circle…" is
hidden. Legal pages (index, terms, privacy) updated to separate the public pool from circles — they remain **drafts for
review, not legal advice**.

Tests: worker `npm test` 694/694, `npm run typecheck` clean; Rust `cargo test --lib` 2108 passed, 0 failed, 5 ignored.
The `pending_circles.test.ts` file's TypeScript diagnostics are the IDE's (tests are outside `tsconfig.include`; the
suite runs and typecheck passes).

### Staging round-trip (2026-10-28)

The changed worker is deployed to `vershion-pool-staging` (version `05465e46`, `workers_dev = false`, no public URL).
Against a `wrangler dev --remote` session on the staging bindings (localhost:18793), the round-trip passed: a
throwaway account (confirmed, approved in the staging D1 as teacher with pool access) created a circle and a
3-question bank (v1), made two changes, and sent all-but-one-unticked -> the server answered version 2 with **added 0,
changed 1, removed 0**, and the unticked question is identical to version 1. The test is
`partial_send_staging_round_trip` (ignored; refuses any base but 18793). Cleanup: the D1 rows (account, circle, bank,
versions, qkeys, members) and the four R2 objects (`circles/<cid>/<bid>/<vid>{,.idx}.json`) were deleted and verified;
the remote dev session was stopped. The local rig on 18791 is still running for the demo. `cargo test --lib`: 2108
passed, 0 failed, 5 ignored.

Still unproven: production, real-network behaviour, and partial sends with pictures, additions or removals (the pure
tests cover those; this round-trip covered a changed raw-base question).

### Real mail through the owner's VPS (2026-10-28)

The owner asked to test an actual email flow and to route it through their own VPS (`lefthanddev.com`), the way raSHio
does: `cloudflare:sockets` + SMTP. That VPS is `15.204.118.73` (SSH host `vps`), Stalwart v0.16 in Docker, publishing
25/465 (587 is not published externally) plus IMAP 993 and JMAP/HTTPS on 8443; Pangolin and traefik sit in front.
`lefthanddev.com` has MX `smtp.huffpalmer.fyi`, SPF `v=spf1 mx -all`, DMARC `p=reject`, so mail must be submitted
authenticated to Stalwart — no third-party sender involved.

What changed (all inside `server/worker/`, which the other session does not have open; the owner directed this round):

- **`src/mail_smtp.ts` (new)** — the transport, modelled on raSHio's `functions/utils/smtp.js`: connect, greeting,
  EHLO, STARTTLS on any port but 465 (465 is implicit TLS), AUTH LOGIN with an AUTH PLAIN fallback, MAIL FROM, RCPT TO,
  DATA. It fixes three things the raSHio client leaves loose: header values are stripped of CR/LF **before** any RFC
  2047 encoding (encoding the raw value would let a client decode the line break back into the header block), the body
  is normalised to CRLF and dot-stuffed, and a 20 s deadline closes the socket rather than hanging the request.
  `cloudflare:sockets` is imported dynamically so the Node test run can import the module.
- **`src/mail.ts`** — `MailEnv` gained `SMTP_HOSTNAME`, `SMTP_PORT`, `SMTP_USERNAME`, `SMTP_PASSWORD`,
  `SMTP_FROM_NAME`, `SMTP_SECURE`. Transport order: `MAIL_MODE=log` → SMTP when all three settings are present → Resend
  (unchanged). The confirm mail now says to look in spam if it does not arrive.
- **`src/share/ui.rs`** — the "Check your email" page adds "If it does not arrive in a few minutes, look in your spam
  or junk folder."
- **`scripts/local-ui-rig.sh` + `LOCAL-UI.md`** — opt-in real mail: if `.wrangler-ui/real-mail.env` exists (mode 600,
  written by hand), `start` leaves `MAIL_MODE` out of `vars.env` and passes the file with `--env-file`, reporting
  `Mail: real, through SMTP`. With no such file nothing changes. In real mode `links` warns and `seed` refuses (it
  confirms itself from the log). `wrangler.toml`/`wrangler.staging.toml` comments name the SMTP secrets.

Tests: worker `npm test` **699/699** (5 new in `test/worker.test.ts`: configuration, sender, port/TLS choice, header
injection + RFC 2047, CRLF/dot-stuffing); `npm run typecheck` clean. Rust: `cargo test --lib` **2108 passed, 0 failed,
5 ignored**; `cargo build --bins` and the wasm32 check green.

Still needed from the owner before the live test: a mailbox on Stalwart (planned sender `verSHion
<vershion@lefthanddev.com>`) and its password, written by them into `.wrangler-ui/real-mail.env` — never through this
session. Expected sharp edges: DMARC is `p=reject`, so the first mail may land in spam until Stalwart's DKIM/SPF for
`lefthanddev.com` is checked (the app copy now says so). The mail's link points at `/account/verify#token=…`, which
this repo does not serve (there is no web app), so opening it in a browser 404s; the app's "Link or code from the
email" box takes the whole link or the long code, which is the path that works.

D1 rounds 19 and 20 are **committed locally** (the round 19–20 checkpoint, 45 files: 35 under `src/share/` plus this file and the nine critic reports 92-100). **Nothing is pushed.** The rest of the dirty tree belongs to the other sessions (see "Do not touch").

## Where D1 stands

Round 18 (committed in `0d17a83` as WIP) was failed by its critic pair; round 19 was scoped to the two important findings plus cheap minors, with the brief "this is the last scoped round". **All 7 round-19 items are now in the tree, with tests, and pass.** The previous version of this file said item 7 was only partly done; four more pin tests were added after it was written.

| # | Item | State |
|---|------|-------|
| 1 | c100 I2: a save failure that loses her edits warns, survives Dismiss, gives advice | done |
| 2 | c100 I1: Check now says "Checking…" then "Checked just now: still the same" | done |
| 3 | c99 m1/m2: held retry saves the links file first and holds on failure | done |
| 4 | c99 m4/m7/m5/m3: persist held Undo, hold Get latest, one safety copy per model, move sent times | done |
| 5 | c99 m6/m8/m9 | done |
| 6 | c100 minors 1, 2, 5, 7, 9, 10, 12 | done |
| 7 | Pin the mutation gaps | **done**: `s19_m12`, `s19_m17`, `s19_m21`, `s19_m24`, `s19_m61`, `s19_m80` in `bank_links_fix19_tests.rs` |

## Round 20 (2026-10-27): the leftover minors

The owner asked for the leftovers from critic 100 to be worked: **minors 3, 4, 6, 8, 11 and 12 are now done**.

| Minor | Change | Test |
|-------|--------|------|
| 3 | A row with no "Put mine back" says why and what she can do: `Kept` rows say the copy moved on and "set it by hand if you want yours"; `Changed` rows add the same tip | `c100_m3_…` |
| 4 | A put-back row names the circle's value ("…the circle's is Hard"); "Put all of mine back" **asks first** ("Put yours back on N questions?" / "Yes, put all back" / "Keep them") and changes nothing until the yes | `c100_m4_…` ×2 |
| 6 | The button column sits just past the widest line that offers one (was at the far edge, ~700 px away at 1280); a row with no button uses the whole width; at a narrow width the stem is cut at 28 chars, so a row is 3 lines, not 4, and "Easy -> Hard" stays whole | `c100_m6_…` ×3 |
| 8 | "Use my bank as it is" and "Get latest" name the same bank: the circle's title, never her bank's own name on one page and the circle's on the next (`merge_title`) | `c100_m8_…` |
| 11 | The news part of the badge has a short form ("difficulty"); "removed from the circle" is said as removed even when `gone()` is also true; `WAITING_NOTE` is "circle not checked yet", not "waiting for the circle check"; the "..." hint says "circle page", the menu's own words | `c100_m11_…` ×3 |
| 12 | The review-plus-not-kept strip is three sentences (`review_not_kept_line`), not five saying the same thing twice; the held line names verSHion once, not four times; the Account window drops "(UTC)". The rest of minor 12 (no "merge", no "REPLACES", no "copy that file over banks.json", no "redeemed") was already pinned by round 19's `c100_m12_leftover_words` | `c100_m12_…` ×3 |

New file: `src/share/bank_links_fix20_tests.rs` (13 tests). Five existing tests changed **deliberately**, each because round 20 reverses what it pinned: `fix18` `c98_m11` (as-is now names the circle's title), `c98_i2` (the news part is now shortened), `c98_m6` (Put all now asks), `fix15`/`fix17`/`fix18` waiting-note assertions (now the constant), `fix18` hint wording.

**Round 20's nine behavioural tests are mutation-checked, 9 of 9 killed** (scratch tree `/tmp/d1mut`, own target `/tmp/d1mut-target`, script `/tmp/d1mut-scripts/mut20.py`, logs `/tmp/d1mut20-*.log`):

| Mutant | What it breaks | Test that failed |
|--------|----------------|------------------|
| ma | "Put all" acts on the first press without asking | `c100_m4_put_all_…` |
| mb | a row with no button is held to the button column | `c100_m6_a_row_with_no_button_…` |
| mc | the stem is always cut at 60, whatever the width | `c100_m6_a_narrow_…` |
| md | `gone()` is asked before `seen.removed` (removed said as "left") | `c100_m11_removed_…` |
| me | a put-back row drops the circle's value | `c100_m4_a_put_back_…` |
| mf | a row whose button is gone gives no reason or tip | `c100_m3_…` |
| mr1 | the review line goes back to the five-sentence wall | `c100_m12_the_review_…` |
| mr2 | "verSHion tries again every minute" comes back (four names in the strip) | `c100_m12_the_held_…` |
| mr3 | the Account window says "(UTC)" again | `c100_m12_the_account_…` |

## Scoped fix round, critics 101/102 (2026-10-27 late evening)

The owner lifted the agent-quota gate early, so the pair ran on 2026-10-27 evening rather than after 2026-10-28 16:00 PT. Both were read-only on `a02c12d`, exactly per `docs/critic-reports/2026-10-28-round101-102-brief.md`. Both returned **important problem(s)**; per the brief the lead stopped and told the owner, and the owner then authorised one scoped fix round.

Reports: `docs/critic-reports/2026-10-27-circles-phaseD1-critic101.md` and `-critic102.md`. Five fixes, each with a test in `src/share/bank_links_fix21_tests.rs` (5 tests), each mutation-checked (5 of 5 killed):

| # | Finding | Fix |
|---|---------|-----|
| 1 | c101 I1: `finish_merge` wrote the links file before the merged bank, through an unchecked caller save. A failed banks write left the link advanced, the bank stale and her merge choices lost | the model writes all banks, **checked**, before `replace_link`; on failure the bank is restored and the page says nothing was changed, with Apply changes again. `banks_saved()` clears a stale "edits aren't saved" warning after the write |
| 2 | c102 I1: an under-review head returned before the `Learning::Held` arm, so a failed save was drawn as a calm "You can keep working here" with no Try again, while the badge said "couldn't save" | the `held` warning is applied after the state is chosen (as round 19 did for `unsaved`): where the line would mislead it is replaced, elsewhere the warning is added after it; warning colour, Try again first, a still-useful action (sign in) kept. The normal `Held` arm is untouched |
| 3 | c101 m1: "Put all of mine back" emptied `inherited_before`, so a restart re-ran the pass over her put-back | the round-19 guard now fires on `ran` (the pass's banks write is known to have happened). A held pass drops `pass_times`, so only both files unwritable at once could mask it |
| 4 | c102 m1: "Dismiss hides this list", but Dismiss deletes it for good | `NEWS_DISMISS` says it removes the list for good, with the offer to put hers back |
| 5 | c102 m2: the badge's "newer version" hover promised "Get latest … brings it in" where the page cannot offer it | the hover says the bank's page says what she can do |

One existing test changed **deliberately**: `bank_links_fix18_tests::c98_m6_the_news_list_layout` pinned the old Dismiss words; it now pins the new ones.

### Verification (2026-10-27 late evening, live tree)

- `cargo test --lib`: **2007 passed, 5 failed, 3 ignored** (2001 before + the 6 new; the 5 are the known share-core/vishion ones).
- `cargo build --bins`: EXIT 0. `cargo check --lib --target wasm32-unknown-unknown`: EXIT 0 (23 pre-existing warnings).
- Mutation: scratch `/tmp/d1mut21` (HEAD + live `src/share/`, own target `/tmp/d1mut21-target`, script `/tmp/mut21.py`). m0 pristine: 6/6 pass. m1–m6 each fail their pinning test. Logs `/tmp/mut21-m*.log`.

### Re-check of the fix diff (focused, same evening)

One critic re-checked only `git diff a02c12d -- src/share/`: **fixes hold**, no blocker or important, two minors — both fixed in the same round. Report: `2026-10-27-circles-phaseD1-recheck.md`.

- the held warning replaced every state's line, losing a removal's bring-back deadline and the sent-unread explanation and pushing History where there is no menu for it: it now replaces only where the line would mislead ("You can keep working here", "works as usual") and appends elsewhere;
- `pass_times` alone did not prove the pass's banks write: `ran` now also requires `bank.modified >= after`, so a crash or a failed safety copy cannot record a pass that never wrote the banks. New test `c101_m1_a_pass_times_write_…`; six mutants killed (m1–m6).

### Screenshots (2026-10-27 late evening, `/tmp/d1shots/`)

- `news21-{light,dark}-{1280,760,420}.png` — the news list with the new Dismiss words. `news21-dark-420.png` replaces the stale `news20-dark-420.png` (which still showed the 60-char stem from before round 20); the new dark frame shows the 28-char stem and three-line rows, matching light.
- `held21-{light,dark}-{1280,760,420}.png` — the Held save failure drawn over the signed-out strip: red panel, "your own edits aren't saved either", Try again first, the sign-in button kept. Fixtures rebuilt by `/tmp/fixture21.py` from the news fixture (difficulty pass pending, `banks.json.tmp` a directory). Recaptured after the re-check narrowing: the signed-out state keeps its own line and the warning follows it.
- **Not captured**: the under-review variant of that strip (needs a signed-in check, i.e. a live read) and the merge save-failure page (a merge needs a circle read). Both are pinned by `c102_i1_…` and `c101_i1_…`.

## Verification of rounds 19-20 (before the scoped fix round)

- `cargo test --lib`: **2001 passed, 5 failed, 3 ignored** (2009 total). Log: `/tmp/d1-m12-final.log`.
- `cargo build --bins`: EXIT 0.
- `cargo check --lib --target wasm32-unknown-unknown`: EXIT 0 (23 warnings, pre-existing).

The 5 failures are **not round 19's**: `bank_links_fix10_tests` ×3, `fix11` ×1, `fix12` ×1. They call `share_core::wasm::publish_prepare_json` and get `{"errors":[{"code":"expected_number","path":"bank.questions[0].points"}]}` on `"points": 1.0`. Cause: the **figures session** added `vishion-core` / `vishion-net` path deps to `Cargo.toml`; their `serde_json` features (`arbitrary_precision`, `preserve_order`) change how share-core's `Node::Num` sees `1.0`, so `as_f64()` returns `None`. Proven not D1's: in a HEAD-only scratch tree (`git archive HEAD` + live `src/share/`) those tests pass. `server/share-core` is untouched and its own suite passes. **The owner has ruled this belongs to the other session; do not edit share-core or the Cargo files as part of D1.**

## Mutation checks on the six new pin tests (2026-10-27 evening)

Method: `git archive HEAD | tar -x -C /tmp/d1mut`, `cp -a src/share/. /tmp/d1mut/src/share/`, `CARGO_TARGET_DIR=/tmp/d1mut-target` (own target, so the shared build lock is untouched), one mutant at a time with the pristine files restored after each.

| Mutant | Where | Result |
|--------|-------|--------|
| m12 | `circle_edit.rs` success path: drop `add_news(l, version, &already);` | **killed** |
| m17 | `(have.len() > known).then_some(have)` → `{ let _ = known; Some(have) }` | survives — **equivalent**, as documented in the test's doc comment (the caller only waits for questions the bank still has) |
| m21 | drop `!r.by.is_empty() &&` from the send-record guard | **killed** |
| m61 | `offered >= 2` → `offered >= 1` for "Put all of mine back" | **killed** |
| m80 | drop `&& !l.as_is` from the partial-count line | **killed** |
| m24 | `settle_all_news`: do not call `self.save()` | **killed** (re-confirmed) |

Scratch tree and logs: `/tmp/d1mut`, `/tmp/d1mut-<mutant>.log`, summary `/tmp/d1mut-summary.log`.

## Screenshots (2026-10-27 evening)

Twelve real-app frames, taken with the QA hooks: `VERSHION_STORE_DIR` (a fixture store), `VERSHION_SCREEN=bank`, `VERSHION_WINSIZE`, `VERSHION_SCRIPT` (`tap:x,y` in egui points), `VERSHION_SHOT`, `VERSHION_SHOT_FRAME=200`, under `xvfb-run`. Window scale is 1.7, so a 1280-point window shoots 2176 px wide; 420 points does work despite `min_inner_size([760,520])`.

In `/tmp/d1shots/` (11 MB, survives until the box reboots):

- `news-{light,dark}-{1280,760,420}.png` — the difficulty news list: three rows, per-row "Put mine back", "Put all of mine back", Dismiss, signed-out strip.
- `news20-{light,dark}-{1280,420}.png` — the same after round 20: the button column beside the rows, and the 420 view with the stem cut at 28 chars.
- `putall20-light-1280.png` — the "Put all of mine back" question (round 20): "Put yours back on 3 questions?…", "Yes, put all back" / "Keep them".
- `warn-{light,dark}-{1280,760,420}.png` — the round-19 I2 warning strip: red panel, "Your difficulty couldn't be put back, and your own edits aren't saved either…", Try again / Sign in… / "…", the news list kept beneath it.

The save failure was produced without code changes by making the banks write fail: `mkdir <store>/banks.json.tmp` (the backend writes `<name>.tmp` then renames, so this fails only `banks.json`). `share-circles.json.tmp` as a directory fails only the links file and gives the other path (m2), also captured once.

**Not captured: the Check-now success feedback** ("Checking…" then "Checked just now: still the same."). It needs a live signed-in circle read; the app is signed out in a fixture store and cannot reach that state offline. The wording and geometry are pinned by `c100_i1_check_now_*` in `bank_links_fix19_tests.rs`. If the owner wants a picture of it, that is a staging run and needs their yes.

Fixtures were made by a temporary `#[ignore]` test appended to `bank_links_fix19_tests.rs` (it ran `many_news(3)` from `bank_links_fix18_tests` and wrote `banks.json`, `share-circles.json`, `settings.json`). It has been removed; re-add it if the shots need regenerating.

## Phase D3 (partial send), piece 1 (2026-10-27 late evening)

The owner chose **D3 before D2**. D2's backend route stays with the db/sharing session; `server/` is untouched.

**Piece 1, committed: the pure composer.** `src/share/partial_send.rs`:
`compose_partial(link, bank, base_body, changed, ticked)` returns the send payload. `changed` is authoritative,
computed by the caller's summary against the base body over every field the server hashes (so a change to `figure`
counts); the composer never calls `merge::ours_changed`. An unticked changed question is sent as the base's raw
JSON plus a `published` block; an unticked addition is omitted; an unticked removal is restored after its base
predecessor, read from `link.qids` (the full pairs survive `bounded` emptying `link.base`). 18 tests in
`partial_send_tests.rs`; four mutants killed in `/tmp/d1mut21` (p1 removals from `link.base`, p2 ignore `changed`,
p3 anchor at the front, p4 send an unticked addition). `cargo test --lib`: 2026 passed, 5 known failed, 3 ignored.

Critic round: 2 blockers + 1 important, all fixed before the commit — a figure-only change sent unticked (the
snapshot cannot see `figure`; the server hashes it, `share-core/src/hash.rs:394`), removals missed on a bounded
link, and the same wrong predicate for an unknown difficulty. The composer now trusts the caller's change set, so
the body-based summary is the next piece and must include `figure` and the difficulty.

**Piece 2, committed: the body-based change list.** `changes_against_body(link, bank, base_body)` returns
`changed` / `added` / `removed`, comparing every field the server hashes: `merge::same_question` (stem, choices and
key, explanation, points, topics, difficulty, figure) plus the picture `{file,width,height}` and the keyless
placeholder. Removals come from `link.qids`, so a bounded link still lists them. `base_index` is shared with the
composer. 31 tests in the module; six mutants killed (figure ignored, difficulty ignored, picture ignored,
removals from `link.base`, the re-added strict check, the empty-bank refusal).

Piece-2 critic: **holds**, two minors, both fixed — a removal the base body lacks is now still listed (only an
unticked one makes the composer refuse), and an empty bank is refused with the composer's own words so the summary
cannot offer a send that can never compose.

**Piece 3, committed: the bookkeeping.** `CircleLink.held_back` (serde default, lenient reader; cleared by a
whole-bank send); `ComposedQuestion` / `compose_partial_parts` keep each composed question's local id (a restored
removal's raw carries the circle qid as its `id`); `after_partial_send` sets the base and pairs from the COMPOSED
questions (a ticked removal's pair dropped, an unticked one kept), stores `held_back`, marks the send-start bank
(so an edit during the send is not stale and shows as unsent), and clears the inherit state. A composed question's
snapshot is optional: a restored question this build cannot read still composes and keeps its pair, with no base
entry (the count then says not fully known). The server's returned qid wins where the answer's list aligns; a short
list is ignored whole, so no question borrows another's qid. 42 tests in the module; five mutants killed
(`held_back` not stored, `mark_bank` on the composed bank, `c.qid` over the answer, the length guard, the
unreadable-removal refusal).

Piece-3 critic: one important (an unreadable restored question refused the payload, so the caller's whole-bank
fallback would send the unticked removal) and three minors, all fixed.

**Piece 4, committed: the send summary model.** `PartialSend` (`Reading` / `Ready { base, body, print, changes, ticked }` /
`Unavailable` / `ReadFailed`), one base-version read per page (`Work::SendBase`), all changes ticked except
`link.held_back`, `toggle_change`/`tick_all`, the composed send through the existing path, the keyless check on the
composed questions (refusals mapped back to bank question numbers), and `after_partial_send` with the as-sent bank;
`held_back` = the unticked ids still differing. Critic blockers fixed in the same pass:
- the summary re-checks the bank's print at Send; a bank changed since the body arrived sends nothing, is re-listed
  against the same body, and keeps ticks by local id (new changes start ticked), sending over the body's own
  `base_version`;
- a 409 carries the unticked set across Get latest and the merge (`send_unticked`), so the rebuilt list starts them
  unticked;
- a transient base read is `ReadFailed` with Try again, never "the circle no longer keeps it" and never a silent
  whole-bank send; only 404/withheld does that;
- an unread answer (version 0) keeps the selection in `RetrySend` and reuses the commit id;
- send-on-top goes as the whole bank instead of composing against another base.
57 partial-send tests (15 page tests); full `cargo test --lib`: 2065 passed, 5 known failed, 3 ignored. The send-test
harness now answers an unscripted version read with 404 (keeps the ~130 whole-bank tests on today's path); two tests
that used the lost-job branch now script 503.

Deferred: the missing-picture fetch for an unticked question; narrowing the harness 404 fallback (the `JobKind::Base`
disconnected branch lost its test); decision/label refusal positions (keyless is mapped).

**Piece 5, committed: the visible summary.** The Ready page draws the plan 4.7 heading, the intro, one row per change
(kind, the bank's question number, the stem cut to one line at 1280 and a second line at 420), a tick box whose
accessible name carries the question and stem, `N of M ticked`, Tick all/Untick all, and `Send N change(s)` — disabled
with nothing ticked, with "Tick at least one change". Reading shows the spinner and waits; ReadFailed shows its stored
reason (session ended, daily limit, network) with Try again; Unavailable shows the state's line with **Get latest** and
Send everything. The notice is drawn above the rows; an in-step copy says there is nothing to send and has no Send;
after a partial send the result says "Sent version N with X of your Y changes. Z stay on this computer, not sent."
72 partial-send tests; full `cargo test --lib`: 2080 passed, 5 known failed, 3 ignored. No screenshots: the Ready state
needs a live circle read (the local rig on 18791 now makes it reachable; no GUI automation was run).

**Piece 6, committed: the picture fetch.** A partial send whose unticked, base-raw question names a picture not on this
computer fetches it from the circle with `circle_blob` (hash checked) and uses the fetched bytes under the base's own
file name (`FetchedMedia`, each picture moved out on its one read), so the question hashes as unchanged; the store
keeps a local copy best-effort. A failed fetch sends nothing, and the page's "Try again" re-composes and re-fetches.
`pool::media_sha` is shared with the uploader so the name rule cannot diverge. 82 partial-send tests; full
`cargo test --lib`: 2090 passed, 5 known failed, 3 ignored. A missing picture on the person's own question keeps
today's refusal and fetches nothing; whole-bank sends fetch nothing; the blob GET costs no item/change read.

**Minor round, committed.** The test harness's unscripted-version 404 is now opt-in (`Script::send_base_404`), so
Compare/History version reads are strict again; the lost `JobKind::Base` disconnected branch is pinned by a new test.
`Pictures`/`Decisions` carry a display-only map from payload positions to the bank's question numbers
(`display_question`), while the data and the request keep payload positions — covered for a restored removal before a
question, a picture row and a decision refusal, with the submitted `image_ok`/`labels` still in payload positions.
85 partial-send tests; full `cargo test --lib`: 2094 passed, 5 known failed, 3 ignored.

**Round-trip confirmed on the local rig.** `src/share/partial_send_rig_tests.rs` (ignored; refuses unless the base is
`http://localhost:18791`) signs up a throwaway account with an in-memory random password (never printed or stored),
approves it with the rig script, creates a circle and a 3-question bank (version 1), changes two questions, sends
all-but-one-unticked, and asserts the server answers `version 2, added 0, changed 1, removed 0` with the unticked
question byte-identical to version 1. Run it while the rig is up with
`cargo test --lib partial_send_rig_round_trip -- --ignored --nocapture`. It passed.

This confirms the plan's one unproven assumption locally: the raw base JSON round-trips as unchanged and the server
counts exactly the ticked change. Still unproven: staging/production, real-network behaviour, and partial sends with
pictures, additions or removals (the pure D3 tests cover those; this round-trip does not). Full
`cargo test --lib`: 2094 passed, 5 known failed, 4 ignored.

Next: staging (needs the owner's yes); otherwise D3 is done.

## What is left (in order)

1. **D1 is closed** (commits `fb1b7e9`, `cc49eed`; Tidy Undo `7956fe7`). **D3 piece 1 is committed**; the next piece is the tick-list model and send summary, which computes `changed` against the base body (including `figure` and the difficulty). Then `after_partial_send`, the send page, pictures, and the staging round-trip (owner's yes).
2. Local commits only; never push. See memory `circles-loop-authorisation`.

## Decisions taken by the owner (2026-10-27)

- **Keep "Check now" as the big button** when the circle's version can't be read. Round 19's visible feedback stands.
- **The `vishion-core`/`vishion-net` dep and the 5 share-core test failures belong to the other session** (the figures one). D1 does not touch them; a short bug note with the evidence is enough.
- Two other sessions have the tree dirty: one on figures (it added the vishion deps), one on the db/sharing work.

## Do not touch (other sessions' uncommitted work)

`Cargo.toml`, `Cargo.lock`, `src/ai.rs`, `src/app.rs`, `src/screens.rs`, `src/store.rs`, `src/lib.rs`, `src/model.rs`, `src/figures/`, `src/gradeui.rs`, `src/figuresui.rs`, `src/tidy.rs`, `src/bankui.rs`, `server/`. Final D1 edits go only to live `src/share/`. Naming rules from the brief: "difficulty", never "ratings" or "pool"; "safety copy" is one word.

## Pitfalls

- **Shared cargo target dir.** Other sessions hold the build lock; a `cargo test` can sit at "Blocking waiting for file lock" for minutes. This is not a hang. Use an own `CARGO_TARGET_DIR` for scratch work, as above.
- A full `cargo test --lib` takes about 84 s warm (150 s cold) once it has the lock.
- **Backgrounded `cargo ... | tail` prints nothing until exit.** Redirect to a file and read that.
- Task `.output` files under `/tmp/claude-1000/.../tasks/` vanish when a session restarts, and the per-session scratchpad dirs went with them. Use `/tmp/<name>.log` for long output.
- `xvfb-run` screenshots need the app built first (`target/debug/vershion`); the window scale is 1.7, so divide screenshot pixels by 1.7 to get the egui points a `VERSHION_SCRIPT` click expects.
- `docs/critic-reports/` has many untracked files from this loop (92 to 100, plus the `grade-p*` files from the other session). Do not delete them.

## Related

- `docs/handoff-shared-pool.md` is the standing project handoff (what is being built, standing rules).
- Memory: `circles-phaseD-decisions`, `circles-loop-authorisation`, `shared-pool-accounts`.
- Round 18 condensed reports: `docs/critic-reports/2026-10-27-circles-phaseD1-critic99.md` and `-critic100.md`.

## First commands

```
cd ~/Documents/GitHub/verSHion
git status --short src/share            # nothing: D1 is committed
cargo test --lib                        # expect 2001 pass, 5 fail (the share-core ones), 3 ignored
```
