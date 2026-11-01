# What to gather before starting staging

Do this list top to bottom, then follow `README.md`. Tick a line only when you have the thing in hand. Secrets go
straight into `/etc/vershion-staging/api.env` on the server (or your password manager); never into chat, git, a ticket
or a screenshot.

## 1. Decisions (nothing to buy, but everything below depends on them)

- [ ] **The domain** both hosts will live under, e.g. `example.org`. The API and the page must share one registrable
      domain (the refresh cookie is `SameSite=Strict; Secure`).
- [ ] **The two names:** API `staging-api.<domain>` and app `staging-app.<domain>` (or your own pair under the same
      domain). Replace `example.org` in `Caddyfile`, `api.env.example` and `README.md` with them.
- [ ] **Whether staging's API name will later become production's.** The API address is compiled into every build, so
      a production build made later for a different name needs a new release. Decide now if that matters.
- [ ] **A daily AI spend cap** for staging (`AI_DAILY_USD`, e.g. 1) and, if you want a smaller free allowance than the
      default, `AI_WEEKLY_CREDITS`.

## 2. Accounts and access

- [ ] **Cloudflare:** edit rights on the DNS zone of the domain, and rights to create a Pages project and add it a
      custom domain.
- [ ] **The VPS:** SSH as a user with sudo; a public IPv4 (IPv6 optional); ports 80 and 443 open and reachable.
      Note its OS and version (the runbook uses `useradd`, `systemd` and Caddy, which fit Debian/Ubuntu/Fedora).
- [ ] **Caddy** installed on the VPS (a package from Caddy's own repository is fine), or permission to install it.
- [ ] **Stripe, test mode:** access to the dashboard. You need three things from it, all test-mode: a secret key
      (`sk_test_...`), a publishable key (`pk_test_...`), and, after the endpoint exists, the endpoint's signing
      secret (`whsec_...`). Not live keys.
- [ ] **OpenRouter:** an API key for staging (a separate key from production's, with a spending limit set in
      OpenRouter), and the model name if it is not the default.
- [ ] **A sending mailbox** the server can log in to over SMTP: host, port, username, password, whether the port uses
      implicit TLS (`SMTP_SECURE`), and the From address. `HANDOFF.md` mentions a planned Stalwart mailbox
      (`vershion@lefthanddev.com`) with DMARC `p=reject`: if you use that domain, check its SPF and DKIM first or
      the first mails will land in spam.
- [ ] **An address you can read** for sign-up tests, and a second one if you want to test "someone else's address".
- [ ] **An admin address** for `ADMIN_EMAIL` (receives the pool's alerts, takedown and cron failures).

## 3. Tools on the machine that builds

- [ ] Rust (the workspace builds with the toolchain in use for the app), to run
      `cargo build --release --manifest-path server/api/Cargo.toml`.
- [ ] The wasm target and `trunk`, to run `trunk build --release` (`Trunk.toml` pins `wasm-opt`; trunk downloads it).
- [ ] `node` and `wrangler` logged in to Cloudflare, if you deploy Pages from the command line (or use the
      dashboard's upload instead).
- [ ] `scp` or `rsync` to the VPS.

## 4. Made on the server, never sent anywhere

- [ ] `AUTH_SECRET`: 32+ random characters, new for staging.
- [ ] `POOL_HASH_KEY`: 32+ random characters, new for staging. It can never be rotated once staging holds data you
      keep; store an offline copy.
      Generate with `head -c48 /dev/urandom | base64 | tr -d '/+=\n'` on the server and put it directly in the env file.

## 5. Checks before you call it ready

- [ ] The two DNS names resolve (`staging-api` to the VPS, grey cloud; `staging-app` to Pages).
- [ ] `curl -s https://staging-api.<domain>/health` answers `{"ok":true,...}`.
- [ ] The service journal shows no unexpected "not set" warning (a missing `OPENROUTER_API_KEY`,
      `STRIPE_WEBHOOK_SECRET` or `AUTH_SECRET` is logged at start).
- [ ] The Stripe dashboard's test event reaches the webhook and the journal shows it accepted.
- [ ] A sign-up mail arrives at your own address.
- [ ] An admin account exists in the staging database (set by hand with the service stopped), so you can approve test
      accounts.

## What I need back from you to continue

Only the two names (and the domain) from section 1: with those I can fill them into the staging files and re-commit.
Do not send me any key, password or secret. If something fails, send the journal line or the browser console line.
