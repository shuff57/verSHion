# verSHion shared pool: legal pages (DRAFT)

Static pages: `index.html`, `terms.html`, `privacy.html`, `copyright.html`, `acceptable-use.html`, plus
`style.css`. No scripts, no external fonts, light/dark by `prefers-color-scheme`.

**These are DRAFTS pending legal review. They are not legal advice.** Every page carries one
`<div class="draft-banner">` right after `<body>`. To remove it later, delete that line from each page
(or `grep -l draft-banner *.html`).

## Resolved owner decisions (applied)

1. 3 upheld strikes in a rolling 12 months = flagged for admin review; "we review the account and may suspend or close it" (copyright.html).
2. After account deletion the name on others' copies shows "deleted teacher" (privacy.html, terms.html); conflict note removed.
3. No circles at launch: all circle / circle-only mentions removed. Items are shared to the public pool; "unlisted" (visible only to the author) is still described in privacy.html because the worker README has it.
4. Launch limits confirmed: 200 MB storage and 5,000 shared questions per account (terms.html); the [CONFIRM] is gone.

5. Account export: self-service Export button in the app (account settings) producing JSON of profile fields, items/publishes, imports list, reports filed, notices, attestations; or email us (privacy.html). Shipped: `GET /me/export` (Worker) and the app's "Download my data" button in My shared items; [CONFIRM] the exact field list against the privacy page before launch.
6. Audit and takedown records kept 3 years, pseudonymous ids only after erasure (privacy.html).
7. Terms changes take effect 30 days after we email you (terms.html).
8. Liability capped at one hundred US dollars; no indemnity clause (terms.html).

## Placeholders to fill (search for `[` in the HTML)

Styled `.ph` (to fill) and `.confirm` (to decide/verify) spans mark every one.

- `[OPERATOR NAME]`: the individual/entity operating the pool (all pages)
- `[CONTACT EMAIL]`: general contact (all pages)
- `[AGENT NAME]`, `[ADDRESS]`, `[EMAIL]`, `[PHONE]`: DMCA designated agent (copyright.html)
- `[CONFIRM when registered]`: Copyright Office agent registration number (copyright.html)
- `[GOVERNING LAW: state/country]`, `[VENUE: county/state]` (terms.html)
- Remaining `[CONFIRM ...]` items (grep `CONFIRM`): governing law with counsel (terms); mail provider, Cloudflare logs and retention, data location, state-law rights language, account export "[CONFIRM when shipped]" (privacy); complainant notification and day counting, overall counsel review (copyright).
- Mail provider: `[CONFIRM: Resend or Cloudflare Email]` (privacy.html)

Also update the "Last updated" date on every page when text changes.

## Deploy on Cloudflare Pages

The pages must be served at `/legal/` on the same site as `/account/*`.

1. Put the folder in the Pages project's output directory as `legal/` (for example the project's build
   output is `server/web/` and these files are `server/web/legal/*`), or copy it into the output at build
   time.
2. `wrangler pages deploy server/web --project-name <project>` (or connect the repo and set the build
   output directory to `server/web`).
3. Pages serves `/legal/` from `index.html`, and `/legal/terms` as well as `/legal/terms.html`. All links
   in these pages are relative (`terms.html`), so they work either way.
4. Link to them from the app's sign-up screen, the sharing panel and the footer of `/account/*`. The Terms
   say the copyright page is linked from the app (docs/shared-pool.md "Takedown", designated agent).
5. If the Pages project and the Worker share a route, make sure `/legal/*` is not captured by the Worker.

## Claims that depend on system behaviour (verify before launch)

Sources: **WR** = `server/worker/README.md`, **SP** = `docs/shared-pool.md`, **AU** = `server/auth/README.md`,
**Code** = `server/worker/src/pool/*.ts`. "Section" names the heading or table.

| Claim in the pages | Page | Source |
|---|---|---|
| Invite-only; admin approves every account; approval needs staff-directory email or school-confirmed contact; no auto-promotion | terms, index | SP "Trust levels"; WR "The first admin" (Approval rule) |
| Adult-school-employee statement at sign-up; students have no accounts or data | terms, privacy | SP "Accounts and auth", "Non-goals" |
| Rejected applicants may re-apply after 7 days; unconfirmed accounts purged after 7 days | terms, privacy | SP "Trust levels" (decisions, last para.) |
| Suspension/revocation hides items reversibly, never deletes | terms, aup | SP "Trust levels" ("Content follows the author's standing") |
| Licence list: CC-BY-4.0 (suggested, nothing preselected), CC0-1.0, CC-BY-SA-4.0, CC-BY-NC-4.0, CC-BY-NC-SA-4.0 (circle-only dropped: no circles at launch) | terms, aup | SP "Sharing model"; Code `license.ts` |
| Licence is per version and irrevocable for copies already made | terms | SP "Sharing model" (Per publish) |
| Attestation wording and that acceptance + wording revision is recorded | terms, aup | task text; Code `license.ts` (`ATTEST_TEXT_REVS = 2026-10-19`), `publish.ts` |
| hide-key: non-authors get stripped form (no correct answers, explanations, figures; shuffled); authors get full; admins can view full | terms, privacy | SP "The stripped (keyless) form"; WR "The first admin" warning, `GET /admin/pool/item` |
| Items without hide key show answers to all approved teachers | terms, privacy | WR "The first admin" warning |
| Display name is copied into imported copies and chains; email and school never shared | terms, privacy | SP "Publish-time review" text; "Safety and privacy" |
| Imports are private copies, with provenance chain and licence; "licence check" is only consistency with recorded sources | terms | SP "Import"; WR "Reader-visible licence check" |
| Importers cannot be recalled; told "source removed" and why; asked to delete on student data | terms, privacy | SP "Non-goals", "Import" (update notices) |
| Keyless imports have no key | terms | SP "Import" / stripped form |
| Limits: 200 MB storage, 5,000 shared questions, daily caps | terms, aup | WR "Limits"; SP "Limits" |
| Image rules: still PNG/JPEG/GIF/WebP, 2 MB, 4096 px, metadata stripped, animated -> first frame, no virus scan, private R2, served only to allowed viewers | terms, privacy | SP "Images", "Blob rules" |
| One report on `student_data` hides at once; reporter needs 7-day-old account with pool history; confirmed = immediate purge, hashes blocked, importers asked to delete; 14-day author appeal | terms, privacy, aup | SP "Quality signals" (Reports); WR "Limits" (Reports), "Known limits" (Privacy purges) |
| Student-data text matching known data is held at publish ("held for review") | privacy | WR "Known limits" (Student-data text is held at publish) |
| 3 distinct teachers' reports hide an item pending review (non-privacy reasons) | aup | SP "Quality signals"; WR "Limits" (Reports) |
| Report reasons: wrong answer, copyright, student data, key leak, inappropriate, other | aup | Code `moderation.ts` `REPORT_REASONS` |
| Reporter told only a thank-you | aup, privacy | WR route table (`POST /items/:id/report` -> always "thanks") |
| Author delete: hidden at once, 14-day undo, then purged | terms, privacy | WR route table; SP "Takedown, DMCA and moderation" |
| Appeals exist for admin-removed, held, privacy-purged items; privacy removals never auto-restored | terms | WR route table (`/appeal`), "Known limits" |
| Takedown: item disabled, content quarantined (admin-only), author emailed, counter-notice form | copyright, privacy | SP "Takedown" table; WR round 22/23 notes |
| 30-day counter-notice window; no counter-notice -> upheld and purged; erased author cannot counter, takedown stands after 30 days | copyright, privacy | SP "No counter-notice" row; WR route table (`/counter-notice` "refused after the 30-day quarantine window"), round 22 notes; Code `QUARANTINE_MS` |
| Complete counter-notice forwarded at once; restored >= 10 and <= 14 US business days after receipt; court-action notice keeps it disabled | copyright | SP "Forward and put back"; Code `moderation.ts` (`addBusinessDays(now,10/14)`), `bizdays.ts` |
| Put-back cancels strike and lifts blocks/holds | copyright | WR round 23 (Counter-notices) |
| Counter-notice fields (512(g)(3)) collected by the form; incomplete refused | copyright | SP "Counter-notice" row |
| Re-upload holds of questions/images of upheld takedowns; DMCA blocks skip content used by 3+ other authors; blocks expire after 60 days by default | copyright | WR "Limits" (Blocks) |
| Strikes: only upheld ones count; 3 within rolling 12 months | copyright, aup | SP "Repeat infringers"; Code `moderation.ts` (`STRIKES_TO_SUSPEND = 3`, `STRIKE_WINDOW_MS`) |
| Strike outcome: admin flag + mail at 3 upheld strikes, then review (matches code) | copyright | Code `repeatInfringerCheck` |
| Passwords: Argon2id hash; tokens and invite codes hashed; sessions store user-agent | privacy | SP "Accounts and auth"; AU; `auth.ts` sessions insert |
| Access token 15 min, refresh token 30 days; HttpOnly cookie for web, device cookie | privacy | SP "Accounts and auth"; AU "Known devices" |
| Rate-limit counters are HMAC-keyed (no raw IP/email), account counters deleted on account deletion | privacy | AU rate-limit bullets |
| Data kept about activity: served/import records ("evidence"), import counts, usage counters, authorship fingerprints (keyed hashes) | privacy | WR "Round 19" (served rows), "licence rule", "Account deletion, more" |
| Account deletion removes: PII, sessions, ratings/favourites, free-text notes, counter-notice personal fields, preferences | privacy | SP "Account deletion"; WR "Known limits" (Account deletion) |
| `keep_on_delete` choice; default keeps imported items anonymised, removes and purges (14 days) the rest | privacy | WR "Known limits" (Account deletion), Spec text round 22 |
| Email HMAC kept 14 days; no re-link of removed items in phase 2 | privacy | SP "Accounts and auth"; WR "Spec text (round 22)" |
| Name becomes "deleted teacher" in chains (batched) | privacy, terms | WR "Known limits" (Account deletion, more). The app publish-review text (SP) still says the name stays; update it |
| Import records kept under an anonymous id; audit rows keep opaque id, notes nulled | privacy | WR "Account deletion, more", "Audit notes" |
| Copyright/moderation records: complainant and counter-notifier contact kept, admin-only | privacy | SP "Safety and privacy" |
| 30-day import-image hold; D1 Time Travel 30 days | privacy | WR "Limits" (Images); WR "Migration 0104" |
| Account export Export button with the listed contents | privacy | Implemented: `GET /me/export` + the app's Download my data button (src/share/mine*). Verify the field list matches the privacy page before launch |
| Cloudflare (Workers, D1, R2) hosts; mail via Resend (default) or Cloudflare Email | privacy | SP "Stack", "Open questions"; `mail.ts` |
| No ads, no sale, no AI training, no analytics/third-party scripts on the web build | privacy | SP "Accounts and auth" (web build loads no third-party scripts). **"No sale", "no ads", "no AI training" are operator commitments, not system facts** |
| Not a child service; sign-up statement | privacy | SP "Accounts and auth" |

## Things the pages do NOT say (on purpose)

- No claim of DMCA safe-harbour status; the copyright page says "we respond to notices that follow this
  policy".
- No statutes cited other than 17 U.S.C. section 512 (subsections (c)(3), (g)(3), (f)). FERPA is mentioned
  by name only.
- No statement about COPPA/SOPIPA beyond "adults only, no student accounts".
