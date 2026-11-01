# Staging runbook: one host, two containers on the existing VPS

`staging-vershion.lefthanddev.com` serves the API and the app's web build. On the VPS (`ssh vps`, Debian 12, Docker) two
containers run from `/opt/vershion-staging/compose.yml`:

- `vershion-staging-api`: the Rust server, no published ports, state in `/var/lib/vershion-staging`.
- `vershion-staging-caddy`: routes the API paths to the server and serves the web build from
  `/var/www/vershion-staging` (`index.html` fallback, so a mailed `/account/verify#token=...` link opens the app).
  It is on the existing `pangolin_frontend` network so the Cloudflare tunnel can reach it as
  `http://vershion-staging-caddy:80`.

The VPS's ports 80/443 belong to Pangolin (gerbil/Traefik) and Stalwart owns the mail ports, so nothing here binds a
host port. The other containers are not touched.

Page and API are one origin: the `SameSite=Strict; Secure` refresh cookie is sent and CORS never applies.

## 1. Build (on the dev machine)

Server, in a Debian 12 container so it runs on the VPS's glibc:

```
docker run --rm -v "$PWD/server:/w/server" -v vershion-target:/target -e CARGO_TARGET_DIR=/target \
  -w /w/server/api rust:1-bookworm cargo build --release
```

App, with the staging address compiled in (a release build without `VERSHION_POOL_URL` shows "Sharing is not set up in
this build"):

```
VERSHION_POOL_URL=https://staging-vershion.lefthanddev.com \
VERSHION_LEGAL_URL=https://staging-vershion.lefthanddev.com/legal \
trunk build --release --dist dist-web
```

Web root (credits and legal pages are plain files in `server/web`):

```
cp -r server/web/account dist-web/account && cp -r server/web/legal dist-web/legal
sed -i 's#https://api.example.org#https://staging-vershion.lefthanddev.com#' dist-web/account/credits.html
```

## 2. Install on the VPS

```
sudo install -d -o debian -g debian /opt/vershion-staging /var/www/vershion-staging
sudo install -d -o 10001 -g 10001 /var/lib/vershion-staging
sudo install -d -m 700 /etc/vershion-staging
rsync -a staging/{compose.yml,Dockerfile,Caddyfile} vps:/opt/vershion-staging/
scp vershion-api vps:/opt/vershion-staging/vershion-api
rsync -a server/worker/migrations/ vps:/opt/vershion-staging/migrations/
rsync -a --delete dist-web/ vps:/var/www/vershion-staging/
```

Env file `/etc/vershion-staging/api.env` (root, mode 600): copy `api.env.example`, generate `AUTH_SECRET` and
`POOL_HASH_KEY` on the server (`head -c48 /dev/urandom | base64 | tr -d '/+=\n'`), then add the SMTP, OpenRouter and
Stripe **test** values yourself.

```
cd /opt/vershion-staging && docker compose up -d --build
docker compose logs api | tail -30
docker compose exec caddy wget -qO- http://vershion-staging-api:8787/health
```

## 3. Front door (needs you)

The tunnel config (`/home/debian/.cloudflared/config.yml`) has one rule, `mom.huffpalmer.fyi`. Add above the 404 rule:

```
  - hostname: staging-vershion.lefthanddev.com
    service: http://vershion-staging-caddy:80
```

then `docker restart cloudflared` (about a second of downtime for the other host), and in Cloudflare DNS add a proxied
CNAME `staging-vershion` to `<tunnel-id>.cfargotunnel.com` (needs `lefthanddev.com` in the same Cloudflare account as the
tunnel; otherwise use a Pangolin resource instead). Traffic arrives with `CF-Connecting-IP`, which the staging Caddy
turns into the single `X-Forwarded-For` entry the server requires.

## 4. Stripe test endpoint

Test mode: add `https://staging-vershion.lefthanddev.com/billing/webhook`, copy its signing secret to
`STRIPE_WEBHOOK_SECRET`, `docker compose up -d`, send a test event, look for it in `docker compose logs api`.

## 5. Test like a user

1. Open the host, sign up with a real address, open the mailed link (the app posts the password and profile with the
   token), log in, reload, stay signed in.
2. Approve your account: set `trust='admin'` for yours in `/var/lib/vershion-staging/pool.db` with the api container
   stopped; admin routes also need a known device and a sudo token.
3. Free tier: spend the weekly allowance. Paid tier: buy a pack with `4242 4242 4242 4242`; credits grow; webhook in logs.
4. Pool: publish, browse, circle, send a bank, import.
5. Browser console: no CORS, CSP or mixed-content errors.
6. `BASE=https://staging-vershion.lefthanddev.com server/api/smoke/smoke.sh` for the API-only steps.

## 6. Reset / tear down

`docker compose down`, delete `/var/lib/vershion-staging`, `up -d` (migrations rerun on an empty file). Never copy
production data here unscrubbed: it holds teachers' email addresses.

## Not done here

- A Content-Security-Policy (the Pages `_headers` file does not apply); add one to the Caddyfile before production.
- Backups: staging is disposable.
