# Staging runbook: Rust API on the VPS, app on Cloudflare Pages

Written for you to run. Nothing here has been run, and none of it touches production, the Worker, or live Stripe.
Replace `example.org` with your domain. The API and the page must be under ONE registrable domain (the refresh cookie
is `SameSite=Strict; Secure`):

| Piece | Where | Name |
|---|---|---|
| API | VPS, Caddy + systemd | `staging-api.example.org` |
| App (web build) | Cloudflare Pages | `staging-app.example.org` |

The API hostname is compiled into the web build as `VERSHION_POOL_URL`, so the builds you test against staging point at
staging. Production builds are made later with the production hostname.

## 1. DNS (Cloudflare)

- `staging-api` A/AAAA record to the VPS, **DNS only (grey cloud)**. The server needs the visitor's address from
  Caddy (see the Caddyfile comment); the orange cloud would hide it.
- `staging-app` is added by Pages as a custom domain on the Pages project (step 5).

## 2. Build the server (on a build machine; the VPS needs no Rust)

```
cargo build --release --manifest-path server/api/Cargo.toml
# -> server/api/target/release/vershion-api
```

Copy `vershion-api` and the `server/worker/migrations/` directory to the VPS:

```
scp server/api/target/release/vershion-api  vps:/opt/vershion-staging/vershion-api
scp -r server/worker/migrations             vps:/opt/vershion-staging/migrations
```

## 3. Server setup on the VPS (once)

```
useradd --system --home /var/lib/vershion-staging --shell /usr/sbin/nologin vershion
install -d -o vershion -g vershion /var/lib/vershion-staging /opt/vershion-staging
install -d -m 700 /etc/vershion-staging
```

- Copy `api.env.example` to `/etc/vershion-staging/api.env`, `chmod 600`, and fill it in **on the server**: fresh
  `AUTH_SECRET` and `POOL_HASH_KEY` (32+ characters; `head -c48 /dev/urandom | base64 | tr -d '/+=\n'`), real SMTP
  settings, your OpenRouter key, Stripe **test** keys.
- Install `vershion-api.service` as `/etc/systemd/system/vershion-api-staging.service`, then
  `systemctl daemon-reload && systemctl enable --now vershion-api-staging`.
- Install the Caddyfile (as the staging site block) and `systemctl reload caddy`.
- Firewall: 80/443 and SSH only. The API listens on `127.0.0.1:8787`, so it is not reachable from outside anyway.

Check: `journalctl -u vershion-api-staging -n 30` shows the migrations applied and no "not set" warnings you did not
expect; `curl -s https://staging-api.example.org/health` answers `{"ok":true,...}`.

## 4. Stripe test endpoint

In the Stripe dashboard, **test mode**: add the endpoint `https://staging-api.example.org/billing/webhook` with the
same events as production's, copy its signing secret into `STRIPE_WEBHOOK_SECRET`, and
`systemctl restart vershion-api-staging`. Send a test event from the dashboard and look for it in the journal.

## 5. The app on Pages

Build with the staging API address compiled in (a release build without `VERSHION_POOL_URL` shows "Sharing is not set
up in this build"; a typed address works only in debug builds):

```
VERSHION_POOL_URL=https://staging-api.example.org \
VERSHION_LEGAL_URL=https://staging-app.example.org/legal \
trunk build --release
# -> dist/web   (the wasm is about 20 MiB; Pages refuses single files over 25 MiB)
```

Deploy `dist/web` as a Pages project (`wrangler pages deploy dist/web --project-name vershion-staging`, or connect it in
the dashboard) and add the custom domain `staging-app.example.org`. Copy `server/web/legal` into the deployed
directory first if you want the legal links to resolve, and `server/web/account` for the credits page (set its
`<meta name="api-base">` to `https://staging-api.example.org`). The mailed links open `/account/verify#token=...`; Pages
serves `index.html` for paths it does not have when the project has no `404.html`, which is what lets the app read the
link. Check that the first time.

`ALLOWED_ORIGINS` and `APP_URL` in `api.env` must already be `https://staging-app.example.org`.

## 6. Test like a user

1. Open `https://staging-app.example.org`, go to the Account screen (no server address to type: it is built in).
2. Sign up with a real address you control, open the mailed link, confirm: the app must post the password and
   profile with the token. Log in, reload, and check you stay signed in.
3. Approve your own account from the admin queue (an admin account is needed: set `trust='admin'` in the staging
   database once, with the service stopped; the admin routes also need a known device and a sudo token).
4. Free tier: use the AI until the weekly allowance is spent. Paid tier: buy a pack with `4242 4242 4242 4242`;
   check the credits grow and the webhook shows in the journal.
5. Pool: publish, browse, create a circle, send a bank, import from the pool.
6. Browser console: no CORS, CSP or mixed-content errors.
7. `server/api/smoke/smoke.sh` can also be pointed at staging (`BASE=https://staging-api.example.org`) for the API-only
   steps; it needs no database access there, so its approve and pool steps skip.

## 7. Tear down / reset

Staging data is disposable: stop the service, delete `/var/lib/vershion-staging`, start it again (migrations rerun on an
empty file). Never copy production data here without scrubbing it: it holds teachers' email addresses.
