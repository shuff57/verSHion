#!/bin/bash
cd "${UC_PROJECT:-.}" || exit 9
run() { m=$1; s=$2; o="${UC_BLIND:-blind}"/out/$s.jsonl; : > "$o"; sed "s#OUTFILE#$o#g" "${UC_CRITIC:-critic.md}" | t3 thread start . - --title "ui critic: $s" --provider opencode --model "$m" --wait --json > critic-$s.json 2> critic-$s.err; echo "$s exit=$?"; }
run ollama-cloud/deepseek-v4.1-flash deepseek &
run ollama-cloud/glm-5.3-flash glm &
run ollama-cloud/kimi-k3 kimi &
run ollama-cloud/minimax-m3.1-flash minimax &
run ollama-cloud/mistral-large-4 mistral &
wait
echo ALLDONE
