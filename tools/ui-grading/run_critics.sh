#!/bin/bash
# Five critics in parallel, one .jsonl each into $UC_BLIND/out/. Override the panel with
# UC_CRITICS="provider/model:slug ..." (round 8 moved from the retired ollama-cloud provider to ollama/).
cd "${UC_PROJECT:-.}" || exit 9
CRITICS=${UC_CRITICS:-"ollama/deepseek-v4.1-flash:deepseek ollama/glm-5.3-flash:glm ollama/kimi-k3:kimi ollama/minimax-m3.1-flash:minimax ollama/mistral-large-4:mistral"}
run() { m=$1; s=$2; o="${UC_BLIND:-blind}"/out/$s.jsonl; : > "$o"; sed -e "s#OUTFILE#$o#g" -e "s#MANIFEST#${UC_BLIND:-blind}/manifest.txt#g" "${UC_CRITIC:-critic.md}" | t3 thread start "${UC_T3_PROJECT:-.}" - --title "ui critic: $s" --provider opencode --model "$m" --wait --json > "${UC_LOGS:-.}"/critic-$s.json 2> "${UC_LOGS:-.}"/critic-$s.err; echo "$s exit=$?"; }
for c in $CRITICS; do run "${c%%:*}" "${c##*:}" & done
wait
echo ALLDONE
