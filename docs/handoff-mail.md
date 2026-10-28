# Handoff: real mail for the pool (2026-10-28)

**Status: working.** The owner created `vershion@lefthanddev.com` and wrote
`server/worker/.wrangler-ui/real-mail.env`; the rig restarted into `Mail: real, through SMTP`, and a
sign-up through the worker sent a real message that Stalwart authenticated (465, AUTH as
`vershion@lefthanddev.com`), queued and delivered to `gmail-smtp-in.l.google.com` with `250 OK`.
SPF aligns (`v=spf1 mx -all` covers the VPS), so DMARC `p=reject` passes on SPF alignment even
though the outbound message is not DKIM-signed yet — signing is worth doing before real use.
The sections below are kept because they record what was built, what was ruled out, and the
staging/legal steps that are still outstanding.

For a fresh session picking this up. Written at the point where the code was finished and tested but
**no authenticated send had happened yet**, because there was no mail credential on this machine.

Repo `~/Documents/GitHub/verSHion`, branch **`shared-pool-phase1`**, last commits
`2dc6aed` (rig: opt-in real mail) and `c689b42` (mail: SMTP transport + spam note). **Nothing is
pushed.** Two sessions work on this tree; this one owns `src/share/` and the circles docs. The owner
explicitly authorised this session to edit `server/worker/src/mail.ts`, `src/mail_smtp.ts`,
`test/worker.test.ts`, `scripts/local-ui-rig.sh`, `scripts/LOCAL-UI.md` and the comment lines of
`wrangler.toml`/`wrangler.staging.toml`. Do not widen that into the rest of `server/` without asking;
`src/{app,model,store,screens,lib,ai,tidy,bankui,figuresui,gradeui}.rs`, `Cargo.toml` and `Cargo.lock`
belong to the other session.

## What was built

The worker previously knew only Resend. It now sends through **SMTP**, in the shape raSHio already
uses (`raSHio/functions/utils/smtp.js`): `cloudflare:sockets`, greeting, EHLO, TLS, AUTH, MAIL FROM,
RCPT TO, DATA.

- `server/worker/src/mail_smtp.ts` (new)
  - 465 = implicit TLS; any other port = STARTTLS (`SMTP_SECURE` overrides).
  - AUTH LOGIN, with AUTH PLAIN as the fallback when LOGIN is refused.
  - Stricter than raSHio's original on three points: CR/LF are stripped from header values **before**
    any RFC 2047 encoding (raSHio encodes the raw value, and a client can decode the line break back
    into the header block); the body is normalised to CRLF and dot-stuffed; a 20 s deadline closes the
    socket instead of hanging the request.
  - `cloudflare:sockets` is imported **dynamically**, because the worker's tests run on plain Node
    (no such module) and `mail.ts` must stay importable there.
- `server/worker/src/mail.ts` — `MailEnv` gained `SMTP_HOSTNAME`, `SMTP_PORT`, `SMTP_USERNAME`,
  `SMTP_PASSWORD`, `SMTP_FROM_NAME`, `SMTP_SECURE`. Order in `sendRaw`: `MAIL_MODE=log` → SMTP when
  all three settings are present → Resend. The confirm mail text now says to check spam.
- `src/share/ui.rs` — the "Check your email" page adds "If it does not arrive in a few minutes, look
  in your spam or junk folder."
- `server/worker/scripts/local-ui-rig.sh` + `LOCAL-UI.md` — if `server/worker/.wrangler-ui/real-mail.env`
  exists (mode 600, written by hand, git-ignored), `rig start` leaves `MAIL_MODE` out of `vars.env`
  and passes the file with `--env-file`, printing `Mail: real, through SMTP`. Without the file
  nothing changes. In real mode `links` warns and `seed` refuses (it confirms its throwaway account
  from the log).

Verification at handoff: worker `npm test` **699/699** (5 new cases in `test/worker.test.ts`),
`npm run typecheck` clean; `cargo test --lib` **2108 passed, 0 failed, 5 ignored**; `cargo build --bins`
and the wasm32 check green.

## The blocker (read this before anything else)

**There is no credential anywhere on this machine.** Checked: no `.dev.vars` in verSHion or raSHio
(raSHio only has `.dev.vars.example`; its Gmail app password is gone), bookSHelf's `RESEND_API_KEY`
is a Cloudflare Pages secret and not on disk, the GNOME keyring has no mail entries, `bw status` is
`locked` (shuff57@gmail.com), no `msmtp`/`swaks`/`sendmail` installed locally or on the VPS.

The owner's mail server is `15.204.118.73`, SSH host **`vps`** (user `debian`, key
`~/.ssh/id_ed25519_vps`), Stalwart v0.16 in Docker, publishing 25/465/993 and 8443 (587 is not
published externally), Pangolin + traefik in front.

- Submission refuses everything without a login, **even from the machine itself**:
  `503 5.5.1 You must authenticate first.`
- Stalwart's admin UI answers on the VPS itself: `curl -sk https://127.0.0.1:8443/admin` → **302**
  (a login page). From outside it is **503**, and `https://smtp.huffpalmer.fyi/` failed TLS
  (`curl` exit 60) at the time of writing.
- `lefthanddev.com`: MX `smtp.huffpalmer.fyi`, SPF `v=spf1 mx -all`, DMARC `p=reject`. No DKIM
  selector could be found by guessing (`default`, `dkim`, `mail`, `stalwart`, `s1`, `k1`,
  `selector1`); Stalwart may sign with a random selector name, so this proves nothing either way.

## Next actions, in order

1. **Get one mailbox credential.** Any of these unblocks everything:
   - the owner writes `server/worker/.wrangler-ui/real-mail.env` (mode 600) with
     `SMTP_HOSTNAME=smtp.huffpalmer.fyi`, `SMTP_PORT=465`, `SMTP_USERNAME=<mailbox>`,
     `SMTP_PASSWORD=<password>`, `SMTP_FROM_NAME="verSHion"` (and optionally
     `MAIL_FROM="verSHion <mailbox>"` when the sender differs from the login); or
   - the owner leaves Stalwart admin credentials in a 0600 file on the VPS and this session creates
     `vershion@lefthanddev.com` + an app password through the admin API, then deletes that file; or
   - the owner unlocks Bitwarden and copies the item into the file themselves.
   The password must never be typed into a chat message or a command that is logged.
2. `cd server/worker && scripts/local-ui-rig.sh stop && scripts/local-ui-rig.sh start` — expect
   `Mail: real, through SMTP (real-mail.env)`. The D1 data (accounts, the seeded circle) survives a
   stop/start; `reset` would delete it.
3. Trigger a real message through the worker, not through the app (no clicking needed). Sign-up
   always sends a verify mail and answers 202 either way:
   ```sh
   PW=$(openssl rand -base64 24)
   curl -s -o /dev/null -w '%{http_code}\n' -X POST http://localhost:18791/auth/signup \
     -H 'Content-Type: application/json' \
     -d "{\"email\":\"shuff57@gmail.com\",\"password\":\"$PW\",\"display_name\":\"Mail test\",
          \"school\":\"Mail test\",\"adult_school_employee\":true}"
   ```
   Use a generated password: the policy rejects short or common ones, and a rejection looks like a
   mail failure if you are not careful. `display_name` and `school` must be plain text and
   `adult_school_employee` must be `true`.
4. Read the server's own verdict:
   ```sh
   ssh vps 'docker logs --since 5m stalwart | grep -iE "shuff57|gmail|dkim|queue|bounce" | tail -30'
   ```
   What to look for: an authentication success for the mailbox, a DKIM-signed outbound message, and
   no `spf.fail`/`dmarc.fail` line. Then check the Gmail inbox **and spam**.
5. If Gmail rejects or junks it: `lefthanddev.com` is `p=reject`, so alignment matters. Check
   Stalwart's DKIM for the domain (its admin UI on the VPS), and whether the published selector
   matches what it signs with. Fix there, then repeat step 4. The app copy already warns about spam.
6. Only then consider **staging** (needs the owner's explicit yes, per phase): add `SMTP_HOSTNAME`,
   `SMTP_PORT`, `SMTP_USERNAME`, `SMTP_FROM_NAME`, `MAIL_FROM` as vars and `SMTP_PASSWORD` as a
   secret (`wrangler secret put --config wrangler.staging.toml`), drop `MAIL_MODE = "log"`, point
   `APP_URL`/`ALLOWED_ORIGINS` at the real URL, and deploy.
   The legal pages still carry `[CONFIRM: Resend or Cloudflare Email]`; changing that wording is a
   privacy/terms change — **stop and ask the owner** first.

## Known limitations, so nobody re-discovers them

- The mail's link is `APP_URL + /account/verify#token=…`, and this repo serves no such page (there is
  no web app; `server/web` is the legal pages only). Opening the link in a browser 404s **on the
  local rig**; the app's "Link or code from the email" box accepts the whole link or the long code,
  which is the path that works. The app copy on that page already explains it.
- In real mode `rig links` cannot help and `rig seed` refuses by design (it confirms its throwaway
  account from the mail log).
- The SMTP client has been exercised against a live handshake only (`220 … Stalwart ESMTP`,
  `250-AUTH PLAIN LOGIN`, `8BITMIME` — checked with `openssl s_client`); no authenticated send has
  happened yet. Treat the first send as the real test.
- `critic24b_scans_are_linear` in share-core's suite is environmental (an absolute <2.0 s bound on a
  loaded box), not a regression; documented in `HANDOFF.md`.

## Local state at handoff

- Local rig: `http://localhost:18791`, running, seeded (`rig seed`): three public items from
  `b@example.test`, the circle "Maths department" with two banks (one at version 3) and two unused
  invite codes (editor `C-KDBC-…`, viewer `C-5NDB-…` — re-run `rig seed` for fresh ones).
- The app is running against it (`VERSHION_POOL_URL=http://localhost:18791 cargo run --bin vershion`).
- `.wrangler-ui/` is git-ignored (`server/.gitignore` line 6); `real-mail.env` belongs there and
  nowhere else.
- Do not run Stalwart's store console (`stalwart -o`) against the live store to look for accounts: it
  is the running server's database.
