#!/usr/bin/env bash
# First-run smoke test for vershion-api against a LOCAL server (never point it at production).
#
# What it does: health, sign-up, verify (token read from the server's dev-mail log), approve (local database only),
# native login, /me, credits, one chat call, one import job, the billing packs and a PaymentIntent, a circle, the export.
# Each step prints PASS, FAIL or SKIP with the reason. Nothing is deleted. Passwords and tokens are never printed.
#
# Needs: bash, curl, jq, and sqlite3 or python3 (to approve the test account in the local database file).
#
# Start the server first, with MAIL_MODE=log and its output going to a file the script can read:
#   set -a; . server/api/smoke/env.local; set +a
#   cargo run --release --manifest-path server/api/Cargo.toml 2>&1 | tee /tmp/vershion-api.log
#
# Then:
#   LOG=/tmp/vershion-api.log DB=./data/pool.db server/api/smoke/smoke.sh
#
# Variables:
#   BASE   server address (default http://127.0.0.1:8787; with Caddy use https://localhost:8443 and add CURL_OPTS=-k)
#   LOG    the server's log file (needed to read the verification token; without it pass TOKEN=...)
#   DB     the SQLite file the server uses (needed to approve the account and join the pool; without it those steps SKIP)
#   EMAIL  test address (default smoke-<random>@example.org)
#   CURL_OPTS  extra curl options
set -u

BASE="${BASE:-http://127.0.0.1:8787}"
LOG="${LOG:-}"
DB="${DB:-}"
EMAIL="${EMAIL:-smoke-$(head -c6 /dev/urandom | od -An -tx1 | tr -d ' \n')@example.org}"
# A throwaway password for this run only (never printed, never stored).
PASSWORD="$(head -c18 /dev/urandom | base64 | tr -d '/+=\n')Aa1!"
# shellcheck disable=SC2206
CURL=(curl -sS --max-time 60 ${CURL_OPTS:-} -H 'X-Forwarded-For: 127.0.0.1' -H 'X-Client: native')

pass=0 fail=0 skip=0
ok() { echo "PASS  $1"; pass=$((pass + 1)); }
bad() { echo "FAIL  $1"; fail=$((fail + 1)); }
no() { echo "SKIP  $1"; skip=$((skip + 1)); }

command -v jq >/dev/null || { echo "jq is required"; exit 2; }

# The local database file, through sqlite3 or (without it) python3. Only ever the local test file named by DB.
have_sql() { [ -n "$DB" ] && [ -f "$DB" ] && { command -v sqlite3 >/dev/null || command -v python3 >/dev/null; }; }
sql() {
  if command -v sqlite3 >/dev/null; then
    sqlite3 "$DB" "$1"
  else
    python3 -I -c 'import sqlite3, sys
c = sqlite3.connect(sys.argv[1], timeout=10)
for r in c.execute(sys.argv[2]).fetchall():
    print(r[0])
c.commit()' "$DB" "$1"
  fi
}

# req METHOD PATH [JSON] [extra curl args...]  -> sets STATUS and BODY
req() {
  local method="$1" path="$2" data="${3:-}"
  shift 3 2>/dev/null || shift $#
  local out
  if [ -n "$data" ]; then
    out="$("${CURL[@]}" -X "$method" -H 'Content-Type: application/json' ${AUTH:+-H "Authorization: Bearer $AUTH"} "$@" -d "$data" -w '\n%{http_code}' "$BASE$path")"
  else
    out="$("${CURL[@]}" -X "$method" ${AUTH:+-H "Authorization: Bearer $AUTH"} "$@" -w '\n%{http_code}' "$BASE$path")"
  fi
  STATUS="${out##*$'\n'}"
  BODY="${out%$'\n'*}"
}
AUTH=""

echo "server: $BASE   account: $EMAIL"

# 0. health -------------------------------------------------------------------------------------------------------
req GET /health
[ "$STATUS" = 200 ] && [ "$(jq -r .ok <<<"$BODY")" = true ] && ok "health" || { bad "health ($STATUS)"; echo "server not reachable; stopping"; exit 1; }

# 1. sign up (always 202, whatever the address) -------------------------------------------------------------------
req POST /auth/signup "$(jq -n --arg e "$EMAIL" --arg p "$PASSWORD" '{email:$e,password:$p,display_name:"Smoke Test",school:"Smoke School",adult_school_employee:true}')"
[ "$STATUS" = 202 ] && ok "sign-up accepted" || bad "sign-up ($STATUS) $BODY"

# 2. verify: the token is in the dev-mail log as ...#token=<token> ---------------------------------------------------
sleep 1
TOKEN="${TOKEN:-}"
if [ -z "$TOKEN" ] && [ -n "$LOG" ] && [ -r "$LOG" ]; then
  TOKEN="$(grep -F -A6 "to=$EMAIL" "$LOG" | grep -o '#token=[A-Za-z0-9_-]*' | tail -1 | cut -d= -f2)"
fi
if [ -z "$TOKEN" ]; then
  no "verify (no token: set LOG=<server log> with MAIL_MODE=log, or TOKEN=...)"
else
  # Confirming an address discards the sign-up password and profile (whoever registered may not own the mailbox), so the
  # person who clicks the link sends the password and profile again with the token. That activates the account.
  req POST /auth/verify "$(jq -n --arg t "$TOKEN" --arg p "$PASSWORD" '{token:$t,password:$p,display_name:"Smoke Test",school:"Smoke School",adult_school_employee:true}')"
  { [ "$STATUS" = 200 ] || [ "$STATUS" = 204 ]; } && ok "address verified" || bad "verify ($STATUS) $BODY"
fi

# 3. approve in the LOCAL database (what an admin's approval does), so the pool and AI treat the account as a teacher
if have_sql; then
  sql "UPDATE users SET trust='teacher' WHERE email='$EMAIL' AND trust IN ('unverified','pending');" \
    && ok "account approved as teacher in $DB (local test only)" || bad "could not update $DB"
else
  no "approve (set DB=<database file>; needs sqlite3 or python3). Login needs an approved account."
fi

# 4. login (native mode returns the tokens in the body) -------------------------------------------------------------
req POST /auth/login "$(jq -n --arg e "$EMAIL" --arg p "$PASSWORD" '{email:$e,password:$p}')"
if [ "$STATUS" = 200 ]; then
  AUTH="$(jq -r .access_token <<<"$BODY")"
  REFRESH="$(jq -r .refresh_token <<<"$BODY")"
  ok "login (trust: $(jq -r .user.trust <<<"$BODY"))"
else
  bad "login ($STATUS) $(jq -r .error <<<"$BODY" 2>/dev/null) (the address must be verified first)"
fi

if [ -n "$AUTH" ]; then
  req GET /me
  [ "$STATUS" = 200 ] && ok "/me answers for the signed-in account" || bad "/me ($STATUS)"
  req POST /auth/refresh "$(jq -n --arg r "$REFRESH" '{refresh_token:$r}')"
  if [ "$STATUS" = 200 ]; then
    AUTH="$(jq -r .access_token <<<"$BODY")"
    ok "refresh rotates the session"
  else
    bad "refresh ($STATUS)"
  fi
fi

# 5. free tier: credits and one chat call ---------------------------------------------------------------------------
if [ -n "$AUTH" ]; then
  req GET /ai/credits
  if [ "$STATUS" = 200 ]; then
    ok "credits: $(jq -c . <<<"$BODY" 2>/dev/null)"
  else
    bad "credits ($STATUS) $BODY"
  fi
  req POST /ai/chat/completions '{"messages":[{"role":"user","content":"Reply with the single word OK."}]}'
  case "$STATUS" in
    200) ok "chat answered by the upstream" ;;
    503) no "chat: OPENROUTER_API_KEY is not set in the server's environment" ;;
    *) bad "chat ($STATUS) $BODY" ;;
  esac
  # A 1x1 PNG, enough to start an import job.
  PNG='iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg=='
  req POST /ai/import "$(jq -n --arg b "$PNG" '{hint:"smoke",pages:[{mime:"image/png",b64:$b}]}')"
  case "$STATUS" in
    202 | 200)
      JOB="$(jq -r '.job_id // empty' <<<"$BODY")"
      ok "import job started ($JOB)"
      S=""
      for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15; do
        sleep 2
        req GET "/ai/import/$JOB"
        S="$(jq -r '.status // empty' <<<"$BODY")"
        if [ "$S" = done ] || [ "$S" = failed ]; then break; fi
      done
      if [ "$S" = done ]; then
        ok "import job finished"
      else
        no "import job ended as '${S:-unknown}': $(jq -r '.error // empty' <<<"$BODY") (a 1x1 image yields no text, so 'failed: No page could be read' is expected)"
      fi
      ;;
    503) no "import: OPENROUTER_API_KEY is not set" ;;
    *) bad "import ($STATUS) $BODY" ;;
  esac
fi

# 6. paid tier: packs, then a PaymentIntent (needs BILLING_ENABLED=1 and Stripe TEST keys) -----------------------------
req GET /billing/packs
if [ "$STATUS" = 404 ]; then
  no "billing is off (BILLING_ENABLED is not 1)"
elif [ "$STATUS" = 200 ]; then
  ok "packs listed"
  PACK="$(jq -r '(.packs // .)[0].id // empty' <<<"$BODY")"
  if [ -n "$AUTH" ] && [ -n "$PACK" ]; then
    req POST /billing/intent "$(jq -n --arg p "$PACK" '{pack:$p}')"
    if [ "$STATUS" = 200 ]; then
      SECRET="$(jq -r .client_secret <<<"$BODY")"
      PI="${SECRET%%_secret_*}"
      ok "PaymentIntent $PI created (test mode)"
      echo "      confirm it with a test card:  stripe payment_intents confirm $PI --payment-method pm_card_visa"
      echo "      (needs: stripe listen --forward-to $BASE/billing/webhook   and its whsec_ value in STRIPE_WEBHOOK_SECRET)"
      echo "      then re-run:  curl -H 'X-Client: native' -H 'X-Forwarded-For: 127.0.0.1' -H 'Authorization: Bearer <token>' $BASE/ai/credits"
    else
      bad "intent ($STATUS) $BODY"
    fi
  else
    no "intent (no pack id in the list, or not signed in)"
  fi
else
  bad "packs ($STATUS) $BODY"
fi

# 7. pool: needs an approved account that has redeemed an invite ----------------------------------------------------
if [ -n "$AUTH" ] && have_sql; then
  UID_="$(sql "SELECT id FROM users WHERE email='$EMAIL';")"
  sql "INSERT OR IGNORE INTO pool_invite_redemptions (user_id, invite_id, at) VALUES ('$UID_', 'smoke', $(date +%s)000);" \
    && ok "pool invite recorded in the local database"
  req POST /circles '{"name":"Smoke circle"}'
  if [ "$STATUS" = 201 ] || [ "$STATUS" = 200 ]; then
    ok "circle created ($(jq -r '.id // .circle.id // empty' <<<"$BODY"))"
  else
    bad "circle create ($STATUS) $BODY"
  fi
  req GET /circles
  [ "$STATUS" = 200 ] && ok "circles listed" || bad "circles list ($STATUS)"
  req GET /items
  [ "$STATUS" = 200 ] && ok "pool listing" || bad "pool listing ($STATUS) $BODY"
  req GET /me/export
  [ "$STATUS" = 200 ] && ok "account export" || bad "export ($STATUS) $BODY"
else
  no "pool (needs a login plus DB=<database file>; sqlite3 or python3)"
fi

# 8. cross-origin: a browser's preflight on a pool route --------------------------------------------------------------
ORIGIN="${ORIGIN:-}"
if [ -n "$ORIGIN" ]; then
  P="$(curl -sS ${CURL_OPTS:-} -o /dev/null -w '%{http_code}' -X OPTIONS -H "Origin: $ORIGIN" -H 'X-Forwarded-For: 127.0.0.1' "$BASE/items")"
  [ "$P" = 204 ] && ok "preflight for $ORIGIN" || bad "preflight for $ORIGIN ($P): is it in ALLOWED_ORIGINS?"
else
  no "preflight (set ORIGIN=http://localhost:8080, one of ALLOWED_ORIGINS)"
fi

# 9. logout ------------------------------------------------------------------------------------------------------------
if [ -n "$AUTH" ]; then
  req POST /auth/logout "$(jq -n --arg r "$REFRESH" '{refresh_token:$r}')"
  { [ "$STATUS" = 204 ] || [ "$STATUS" = 200 ]; } && ok "logout" || bad "logout ($STATUS)"
fi

echo
echo "pass=$pass fail=$fail skip=$skip"
[ "$fail" = 0 ]
