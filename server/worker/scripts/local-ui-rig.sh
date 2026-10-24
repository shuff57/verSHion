#!/usr/bin/env bash
# A throwaway LOCAL pool server for clicking through the app's sharing windows. Nothing here talks
# to staging or production: wrangler runs with --local, D1 and R2 live in server/worker/.wrangler-ui
# (git-ignored), and mail is printed to a log file instead of sent. See scripts/LOCAL-UI.md.
#
#   scripts/local-ui-rig.sh start                 # migrations, then `wrangler dev` on http://localhost:18791
#   scripts/local-ui-rig.sh status
#   scripts/local-ui-rig.sh links [EMAIL]         # newest confirm / reset / device links from the mail log
#   scripts/local-ui-rig.sh approve EMAIL         # trust -> teacher, plus a pre-gate invite redemption
#   scripts/local-ui-rig.sh approve EMAIL --teacher-only   # trust -> teacher, the invite still needed
#   scripts/local-ui-rig.sh invite                # prints one fresh single-use invite code
#   scripts/local-ui-rig.sh seed                  # 3 sample shared items from a throwaway b@example.test
#   scripts/local-ui-rig.sh log                   # follow the dev server output
#   scripts/local-ui-rig.sh stop
#   scripts/local-ui-rig.sh reset                 # stop, then delete server/worker/.wrangler-ui
#
# Port 18791 only (devtools on 18792). It refuses to start if either is taken and never stops any
# process it did not start. Passwords never pass through this script: they are typed in the app
# (seed's throwaway password is made and kept inside its node process, see local-ui-seed.mjs).
set -euo pipefail

PORT=18791
INSPECTOR_PORT=18792
BASE="http://localhost:$PORT"
DB=vershion-pool
WRANGLER=${WRANGLER:-wrangler}

here=$(cd "$(dirname "$0")/.." && pwd) # server/worker
dir="$here/.wrangler-ui"
log="$dir/dev.log"
pidfile="$dir/dev.pid"

die() { echo "local-ui-rig: $*" >&2; exit 1; }

port_busy() {
  if command -v ss >/dev/null 2>&1; then
    [ -n "$(ss -ltnH "sport = :$1" 2>/dev/null)" ]
  else
    (exec 3<>"/dev/tcp/127.0.0.1/$1") 2>/dev/null
  fi
}

# The pid in the pid file, only if that process is still our wrangler dev on our port.
rig_pid() {
  [ -f "$pidfile" ] || return 1
  local pid
  pid=$(cat "$pidfile")
  [[ "$pid" =~ ^[0-9]+$ ]] || return 1
  [ -r "/proc/$pid/cmdline" ] || return 1
  local cmd
  cmd=$(tr '\0' ' ' <"/proc/$pid/cmdline")
  case "$cmd" in *wrangler*"--port $PORT"*) echo "$pid" ;; *) return 1 ;; esac
}

need_dir() { [ -d "$dir" ] || die "nothing here yet: run '$0 start' first"; }

d1() { (cd "$here" && "$WRANGLER" d1 "$@" --local --persist-to "$dir"); }

valid_email() {
  printf '%s' "$1" | grep -Eq '^[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}$'
}

cmd_start() {
  if pid=$(rig_pid); then die "already running (pid $pid) on $BASE"; fi
  port_busy "$PORT" && die "port $PORT is in use by something else; not starting (nothing was stopped)"
  port_busy "$INSPECTOR_PORT" && die "port $INSPECTOR_PORT (devtools) is in use; not starting"
  mkdir -p "$dir"
  chmod 700 "$dir"

  # Throwaway secrets, made once per rig (reset makes new ones). Never printed. They must stay the
  # same between restarts: stored pool hashes are keyed by POOL_HASH_KEY.
  if [ ! -s "$dir/secrets.env" ]; then
    (
      umask 077
      {
        printf 'AUTH_SECRET=%s\n' "$(od -An -tx1 -N32 /dev/urandom | tr -d ' \n')"
        printf 'POOL_HASH_KEY=%s\n' "$(od -An -tx1 -N32 /dev/urandom | tr -d ' \n')"
      } >"$dir/secrets.env"
    )
  fi
  # Non-secret settings. Log-mode mail needs a localhost APP_URL (src/mail.ts); the native app
  # sends no Origin, so ALLOWED_ORIGINS only has to be a valid localhost value.
  cat >"$dir/vars.env" <<EOF
MAIL_MODE=log
APP_URL=$BASE
ALLOWED_ORIGINS=$BASE
EOF

  echo "Applying migrations to the local D1 in $dir ..."
  d1 migrations apply "$DB" </dev/null

  echo "Starting wrangler dev on $BASE (log: $log) ..."
   # Appended, so mail links from before a restart are still there for 'links'.
  ( umask 077; echo "==== local-ui-rig start $(date -Is) ====" >>"$log" )
  (
    cd "$here"
    exec setsid "$WRANGLER" dev --local --ip localhost --port "$PORT" --inspector-port "$INSPECTOR_PORT" \
      --persist-to "$dir" --env-file "$dir/secrets.env" --env-file "$dir/vars.env" \
      --show-interactive-dev-session=false
  ) >>"$log" 2>&1 </dev/null &
  local pid=$!
  echo "$pid" >"$pidfile"

  # The first start builds share-core's wasm (wasm-pack), which can take a few minutes.
  local i
  for i in $(seq 1 300); do
    if ! kill -0 "$pid" 2>/dev/null; then
      tail -n 40 "$log" >&2
      die "wrangler dev exited; the log above is from $log"
    fi
    if curl -s -o /dev/null -m 2 "$BASE/" 2>/dev/null; then
      echo "Ready: $BASE  (pid $(cat "$pidfile"))"
      return 0
    fi
    sleep 1
  done
  die "not answering on $BASE after 300 s; see $log (still running: '$0 stop' to end it)"
}

cmd_stop() {
  local pid
  if ! pid=$(rig_pid); then
    rm -f "$pidfile"
    echo "Not running."
    return 0
  fi
  # setsid made the pid its own process group: this ends wrangler and its workerd child, nothing else.
  kill -TERM -- "-$pid" 2>/dev/null || kill -TERM "$pid" 2>/dev/null || true
  local i
  for i in $(seq 1 20); do
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.5
  done
  kill -0 "$pid" 2>/dev/null && kill -KILL -- "-$pid" 2>/dev/null || true
  rm -f "$pidfile"
  echo "Stopped."
}

cmd_status() {
  if pid=$(rig_pid); then echo "Running: $BASE (pid $pid)"; else echo "Not running."; fi
}

cmd_reset() {
  cmd_stop
  # Only ever this one git-ignored directory.
  [ "$(basename "$dir")" = ".wrangler-ui" ] || die "unexpected persist dir: $dir"
  rm -rf -- "$dir"
  echo "Deleted $dir (local accounts, pool items, secrets and logs)."
}

cmd_links() {
  need_dir
  [ -f "$log" ] || die "no mail log yet"
  local want="${1:-}"
  want=$(printf '%s' "$want" | tr '[:upper:]' '[:lower:]')
  # Each log-mode mail is "[dev mail] to=ADDR subject=..." followed by its text; the link line has
  # "#token=". Only the newest link of each kind per address is printed.
  sed 's/\x1b\[[0-9;]*m//g' "$log" | awk -v want="$want" '
    /\[dev mail\] to=/ {
      to = $0; sub(/.*\[dev mail\] to=/, "", to); sub(/ subject=.*/, "", to); to = tolower(to); next
    }
    /#token=/ && to != "" {
      url = $0; sub(/^[^h]*/, "", url); sub(/[ \t\r]+$/, "", url)
      kind = (url ~ /\/account\/verify/) ? "confirm" : (url ~ /\/account\/reset/) ? "reset" : (url ~ /\/account\/enrol/) ? "device" : "other"
      if (want == "" || want == to) { key = to " " kind; if (!(key in seen)) order[++n] = key; seen[key] = 1; last[key] = url }
    }
    END {
      if (n == 0) { print "No links in the mail log" (want == "" ? "" : " for " want) " yet."; exit }
      for (i = 1; i <= n; i++) { split(order[i], k, " "); printf "%-8s %s\n         %s\n", k[2], k[1], last[order[i]] }
    }'
}

cmd_approve() {
  need_dir
  local email mode
  email=$(printf '%s' "${1:-}" | tr '[:upper:]' '[:lower:]')
  mode="${2:-}"
  valid_email "$email" || die "usage: $0 approve EMAIL [--teacher-only]"
  case "$mode" in ""|--teacher-only) ;; *) die "unknown option: $mode" ;; esac
  local now="CAST(strftime('%s', 'now') AS INTEGER) * 1000"
  # Same eligibility as scripts/make-admin.sh: confirmed, password set, not deleted, not suspended.
  local eligible="email = '$email' AND email_verified_at IS NOT NULL AND pw_hash IS NOT NULL AND deleted_at IS NULL AND suspended_at IS NULL AND trust IN ('pending', 'teacher')"
  local sql="$dir/approve.sql"
  {
    echo "INSERT INTO auth_audit (id, at, actor_id, actor_kind, action, target_id, prev_trust, new_trust, note)
      SELECT lower(hex(randomblob(16))), $now, 'bootstrap', 'bootstrap', 'approve', id, trust, 'teacher', 'local UI rig'
      FROM users WHERE $eligible AND trust = 'pending';"
    echo "UPDATE approval_requests SET decided_at = $now, decided_by = 'bootstrap', decision = 'bootstrap'
      WHERE decided_at IS NULL AND user_id IN (SELECT id FROM users WHERE $eligible);"
    echo "UPDATE users SET trust = 'teacher' WHERE $eligible;"
    if [ "$mode" != "--teacher-only" ]; then
      # As migration 0106 did for accounts approved before the invite gate: invite_id NULL.
      echo "INSERT OR IGNORE INTO pool_invite_redemptions (user_id, invite_id, at)
        SELECT id, NULL, $now FROM users WHERE $eligible AND trust = 'teacher';"
    fi
  } >"$sql"
  d1 execute "$DB" --file "$sql" >/dev/null
  local out
  out=$(d1 execute "$DB" --json --command "SELECT u.email, u.trust, u.email_verified_at IS NOT NULL AS confirmed, r.user_id IS NOT NULL AS has_invite FROM users u LEFT JOIN pool_invite_redemptions r ON r.user_id = u.id WHERE u.email = '$email'")
  local row
  row=$(printf '%s' "$out" | jq -c '.[0].results[0] // empty')
  [ -n "$row" ] || die "no account with the address $email (sign up in the app first)"
  echo "$row"
  [ "$(printf '%s' "$row" | jq -r .trust)" = "teacher" ] || die "not approved: the address must be confirmed (use 'links'), and the account not suspended or admin"
  if [ "$mode" = "--teacher-only" ] && [ "$(printf '%s' "$row" | jq -r .has_invite)" != "1" ]; then
    echo "Approved as teacher. The pool still answers invite_required until an invite code is redeemed ('$0 invite')."
  else
    echo "Approved as teacher, with pool access."
  fi
}

cmd_invite() {
  need_dir
  local sql="$dir/invite.sql" code
  # The code is made and hashed by the worker's own src/pool/invites.ts; only the hash is stored.
  code=$(cd "$here" && SQL_OUT="$sql" node --experimental-strip-types --no-warnings --input-type=module - <<'JS'
import { writeFileSync } from "node:fs";
import { pathToFileURL } from "node:url";
const m = await import(pathToFileURL(process.cwd() + "/src/pool/invites.ts").href);
const code = m.newInviteCode();
const hash = await m.inviteHash(m.normaliseInviteCode(code));
const id = [...crypto.getRandomValues(new Uint8Array(16))].map((x) => x.toString(16).padStart(2, "0")).join("");
writeFileSync(
  process.env.SQL_OUT,
  "INSERT INTO pool_invites (id, code_hash, created_at, created_by, max_uses, uses, expires_at, note) " +
    `VALUES ('${id}', '${hash}', ${Date.now()}, 'local-ui-rig', 1, 0, NULL, 'local UI rig');\n`,
);
process.stdout.write(code);
JS
  )
  d1 execute "$DB" --file "$sql" >/dev/null
  rm -f "$sql"
  echo "Single-use invite code: $code"
}

cmd_seed() {
  need_dir
  rig_pid >/dev/null || die "not running: '$0 start' first"
  local seed_email=b@example.test out n
  # Idempotent: an existing b@example.test (seeded before, or made by hand) is left alone.
  out=$(d1 execute "$DB" --json --command "SELECT COUNT(*) AS n FROM users WHERE email = '$seed_email'")
  n=$(printf '%s' "$out" | jq -r '.[0].results[0].n')
  [[ "$n" =~ ^[0-9]+$ ]] || die "could not read the local D1"
  if [ "$n" != "0" ]; then
    out=$(d1 execute "$DB" --json --command "SELECT COUNT(*) AS n FROM items i JOIN users u ON u.id = i.author_id WHERE u.email = '$seed_email' AND i.removed_at IS NULL")
    echo "$seed_email already exists ($(printf '%s' "$out" | jq -r '.[0].results[0].n') items): already seeded, nothing done. ('$0 reset' starts over.)"
    return 0
  fi
  node "$here/scripts/local-ui-seed.mjs"
}

cmd_log() {
  need_dir
  exec tail -n 50 -f "$log"
}

case "${1:-}" in
  start) cmd_start ;;
  stop) cmd_stop ;;
  status) cmd_status ;;
  reset) cmd_reset ;;
  links) cmd_links "${2:-}" ;;
  approve) cmd_approve "${2:-}" "${3:-}" ;;
  invite) cmd_invite ;;
  seed) cmd_seed ;;
  log) cmd_log ;;
  *)
    sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'
    exit 2
    ;;
esac
