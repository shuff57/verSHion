#!/bin/bash
# Five critics in parallel, one .jsonl each into $UC_BLIND/out/. Override the panel with
# UC_CRITICS="provider/model:slug ..." (round 8 moved from the retired ollama-cloud provider to ollama/).
# Round 6 swapped minimax and mistral (wrong-screen and made-up readings) for qwen3.8-max and gpt-6-luna.
cd "${UC_PROJECT:-.}" || exit 9
FULL="ollama/deepseek-v4.1-flash:deepseek ollama/glm-5.3-flash:glm ollama/kimi-k3:kimi cheaper-inference/qwen3.8-max-0902:qwen cheaper-inference/gpt-6-luna:gpt"
# UC_PANEL=steer: the three ungated critics only (no 4-item chunk restarts), about 60% of a full round's
# tokens. Use it to steer the builder between rounds; the bar is only claimed on a full-panel round, and
# a steer round's means are never compared with a full round's (a changed panel resets the scale).
STEER="ollama/deepseek-v4.1-flash:deepseek ollama/glm-5.3-flash:glm ollama/kimi-k3:kimi"
if [ "${UC_PANEL:-full}" = steer ]; then CRITICS=${UC_CRITICS:-$STEER}; else CRITICS=${UC_CRITICS:-$FULL}; fi
echo "panel: ${UC_PANEL:-full} -> $CRITICS"
# Critics behind a gateway with a request-size cap (cheaper-inference: ~4.5 MB, HTTP 413) keep every
# image they read in the conversation, so they get the manifest in chunks of $UC_CHUNK items, one fresh
# session each, appending to the same file. Ids already in the file are skipped, so a rerun resumes.
start() { sed -e "s#OUTFILE#$2#g" -e "s#MANIFEST#$3#g" "${UC_CRITIC:-critic.md}" | t3 thread start "${UC_T3_PROJECT:-.}" - --title "ui critic: $4" --provider opencode --model "$1" --wait --json > "${UC_LOGS:-.}"/critic-$4.json 2> "${UC_LOGS:-.}"/critic-$4.err; }
run() { m=$1; s=$2; B="${UC_BLIND:-blind}"; o=$B/out/$s.jsonl
  case "$m" in
    ${UC_CHUNKED:-cheaper-inference/*})
      touch "$o"; mkdir -p "$B/chunks"; rm -f "$B/chunks/$s"-*.txt
      python3 - "$B/manifest.txt" "$o" "$B/chunks/$s" "${UC_CHUNK:-4}" <<'PY'
import json, sys
man, out, pre, n = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4])
done = set()
for l in open(out):
    try: done.add(json.loads(l)['id'])
    except Exception: pass
todo = [l for l in open(man) if l.strip() and l.split('\t')[0] not in done]
for i in range(0, len(todo), n):
    open(f'{pre}-{i // n:02d}.txt', 'w').writelines(todo[i:i + n])
PY
      for c in "$B/chunks/$s"-*.txt; do [ -e "$c" ] || continue; k=$(basename "$c" .txt); start "$m" "$o" "$c" "$k"; echo "$k exit=$?"; done ;;
    *) : > "$o"; start "$m" "$o" "$B/manifest.txt" "$s"; echo "$s exit=$?" ;;
  esac; }
for c in $CRITICS; do run "${c%%:*}" "${c##*:}" & done
wait
echo ALLDONE
