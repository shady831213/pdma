#!/usr/bin/env bash
set -euo pipefail
flow=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
source "$flow/process.sh"
fixture=$(mktemp -d)
owner=
cleanup() {
    if [[ -n $owner ]]; then kill -TERM "$owner" 2>/dev/null || true; wait "$owner" 2>/dev/null || true; fi
    rm -rf "$fixture"
}
trap cleanup EXIT
pdma_process_pid=
pdma_execute 5 "$fixture/success.log" bash -c 'printf "owned output\n"'
grep -Fxq 'owned output' "$fixture/success.log"
status=0
pdma_execute 5 "$fixture/failure.log" bash -c 'exit 7' || status=$?
[[ $status == 7 ]]
cat > "$fixture/child.sh" <<'SH'
#!/usr/bin/env bash
trap '' TERM
printf '%s\n' "$$" > "$1/child.pid"
sleep 30 &
printf '%s\n' "$!" >> "$1/child.pid"
wait
SH
cat > "$fixture/owner.sh" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
source "$1/process.sh"
pdma_process_pid=
trap pdma_stop_process EXIT
trap 'exit 143' TERM
pdma_execute 30 "$2/child.log" bash "$2/child.sh" "$2"
SH
bash "$fixture/owner.sh" "$flow" "$fixture" & owner=$!
deadline=$((SECONDS+5))
until [[ -s $fixture/child.pid ]] && (($(wc -l < "$fixture/child.pid") == 2)); do
    ((SECONDS < deadline)) || { printf 'Child did not start\n' >&2; exit 1; }
    sleep 0.05
done
kill -TERM "$owner"
status=0
wait "$owner" || status=$?
owner=
[[ $status == 143 ]]
while read -r pid; do
    state=$(ps -o stat= -p "$pid" 2>/dev/null || true)
    [[ -z $state || $state == Z* ]] || { printf 'Owned descendant survived cancellation\n' >&2; exit 1; }
done < "$fixture/child.pid"
printf 'PDMA_PROCESS_TEST_PASS output_status_and_cancelled_descendants\n'
