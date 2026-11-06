#!/usr/bin/env bash
set -euo pipefail
flow=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
evidence=$(realpath -m "${1:?Usage: selftest.sh NEW_EVIDENCE}")
[[ ! -e $evidence ]] || { printf 'Evidence directory already exists\n' >&2; exit 2; }
mkdir -p "$evidence"
fixture=$(mktemp -d)
regression_pid=
cleanup() {
    if [[ -n $regression_pid ]]; then kill -TERM "$regression_pid" 2>/dev/null || true; wait "$regression_pid" 2>/dev/null || true; fi
    rm -rf "$fixture"
}
trap cleanup EXIT
unset PDMA_ENV
export FIXTURE_STATE="$fixture/state" PDMA_JOBS=2
export PDMA_TESTS='pdma_aligned_copy_test pdma_random_copy_test'
mkdir -p "$fixture/build/coverage.vdb" "$FIXTURE_STATE"
printf '0 0\n' > "$FIXTURE_STATE/count"
cat > "$fixture/build/simv" <<'SIM'
#!/usr/bin/env bash
set -euo pipefail
test_name= seed= mutation= coverage=
while (($#)); do
    case "$1" in
        +UVM_TESTNAME=*) test_name=${1#*=} ;;
        +ntb_random_seed=*) seed=${1#*=} ;;
        +checker_mutation=*) mutation=${1#*=} ;;
        -cm_dir) shift; coverage=$1 ;;
    esac
    shift
done
mkdir -p "$coverage"
exec 9> "$FIXTURE_STATE/lock"
flock 9
read -r active maximum < "$FIXTURE_STATE/count"
active=$((active + 1))
if ((active>maximum)); then maximum=$active; fi
printf '%s %s\n' "$active" "$maximum" > "$FIXTURE_STATE/count"
printf 'START %s %s pid=%s active=%s\n' "$test_name" "$seed" "$$" "$active" >> "$FIXTURE_STATE/events"
flock -u 9
finish() {
    flock 9
    read -r active maximum < "$FIXTURE_STATE/count"
    printf '%s %s\n' "$((active - 1))" "$maximum" > "$FIXTURE_STATE/count"
    printf 'END %s %s pid=%s\n' "$test_name" "$seed" "$$" >> "$FIXTURE_STATE/events"
    flock -u 9
}
trap finish EXIT
if [[ ${FIXTURE_CANCEL:-0} == 1 ]]; then
    bash -c 'trap "" TERM; while :; do sleep 30; done' &
    printf '%s\n' "$$" "$!" > "$PWD/descendants.txt"
    ps -o pgid= -p "$$" > "$PWD/group.txt"
    wait
fi
sleep 0.2
if [[ -n $mutation ]]; then
    case "$mutation" in
        data) diagnostic=TASK_DATA ;;
        identity|unknown_identity) diagnostic=TASK_IDENTITY ;;
        missing_completion) diagnostic=TASK_TIMEOUT ;;
        duplicate_completion) diagnostic=TASK_DUPLICATE ;;
    esac
    printf 'UVM_FATAL fixture [%.100s] expected mutation\n' "$diagnostic" > simulation.log
    exit 1
fi
if [[ ${FIXTURE_FAIL:-} == "$test_name:$seed" ]]; then
    printf 'UVM_ERROR fixture @ 1: injected failure\n' > simulation.log
    exit 1
fi
if [[ ${FIXTURE_WRONG_MARKER:-0} == 1 ]]; then test_name=pdma_wrong_test; fi
printf 'PDMA_VIP_PASS test=%s\n' "$test_name" > simulation.log
SIM
chmod +x "$fixture/build/simv"
(cd "$fixture/build" && sha256sum simv > executable.sha256)

bash "$flow/regression.sh" "$fixture/build" "$evidence/parallel" 1 7 > "$evidence/parallel.log" 2>&1
read -r active maximum < "$FIXTURE_STATE/count"
[[ $active == 0 && $maximum == 2 ]]
[[ $(grep -c '^PASS test=' "$evidence/parallel/results.txt") == 4 ]]
[[ $(grep -c '^REJECTED mutation=' "$evidence/parallel/results.txt") == 5 ]]
cp "$FIXTURE_STATE/events" "$evidence/parallel-events.txt"
printf 'PASS concurrency maximum=%s limit=2 active_at_end=%s positive=4 negative=5\n' "$maximum" "$active" > "$evidence/assertions.txt"

export FIXTURE_FAIL=pdma_aligned_copy_test:1
if bash "$flow/regression.sh" "$fixture/build" "$evidence/failure" 1 7 > "$evidence/failure.log" 2>&1; then exit 1; fi
[[ $(grep -c '^FAIL test=' "$evidence/failure/results.txt") == 1 ]]
[[ $(grep -c '^PASS test=' "$evidence/failure/results.txt") == 3 ]]
[[ $(grep -c '^REJECTED mutation=' "$evidence/failure/results.txt") == 5 ]]
unset FIXTURE_FAIL
printf 'PASS failure aggregation failed=1 passed=3 negative=5 exit=nonzero\n' >> "$evidence/assertions.txt"

if FIXTURE_WRONG_MARKER=1 bash "$flow/run.sh" "$fixture/build" "$evidence/wrong-marker" pdma_aligned_copy_test 1 > "$evidence/wrong-marker.log" 2>&1; then exit 1; fi
printf 'PASS wrong test marker rejected\n' >> "$evidence/assertions.txt"
for argument in +UVM_TESTNAME=pdma_random_copy_test +ntb_random_seed=7; do
    if bash "$flow/run.sh" "$fixture/build" "$evidence/forbidden" pdma_aligned_copy_test 1 "$argument" >> "$evidence/invalid.log" 2>&1; then exit 1; fi
done
if bash "$flow/run.sh" "$fixture/build" "$evidence/forbidden" '../invalid' 1 >> "$evidence/invalid.log" 2>&1; then exit 1; fi
[[ ! -e $evidence/forbidden ]]
printf 'PASS test/seed overrides and unsafe identifier rejected before output allocation\n' >> "$evidence/assertions.txt"

cat > "$fixture/urg" <<'URG'
#!/usr/bin/env bash
set -euo pipefail
while (($#)); do
    if [[ $1 == -report ]]; then shift; mkdir -p "$1"; printf 'fixture coverage report\n' > "$1/dashboard.txt"; fi
    shift
done
URG
chmod +x "$fixture/urg"
PDMA_COVERAGE_TOOL="$fixture/urg" bash "$flow/coverage.sh" "$fixture/build" "$evidence/failure" "$evidence/coverage" > "$evidence/coverage.log" 2>&1
[[ $(wc -l < "$evidence/coverage/databases.list") == 4 ]]
! grep -Eq '/negative-|/pdma_aligned_copy_test/seed-1/' "$evidence/coverage/databases.list"
printf 'PASS coverage includes build+3 passed cases, excludes failed and negative cases\n' >> "$evidence/assertions.txt"

FIXTURE_CANCEL=1 bash "$flow/regression.sh" "$fixture/build" "$evidence/cancel" 1 7 > "$evidence/cancel.log" 2>&1 &
regression_pid=$!
deadline=$((SECONDS + 10))
while [[ ! -s $evidence/cancel/pdma_aligned_copy_test/seed-1/group.txt || ! -s $evidence/cancel/pdma_aligned_copy_test/seed-7/group.txt ]]; do
    ((SECONDS < deadline)) || exit 1
    sleep 0.05
done
kill -TERM "$regression_pid"
status=0
wait "$regression_pid" || status=$?
regression_pid=
[[ $status == 143 ]]
cat "$evidence/cancel/pdma_aligned_copy_test"/seed-*/descendants.txt > "$evidence/cancel-pids.txt"
while read -r pid; do
    state=$(ps -o stat= -p "$pid" || true)
    [[ -z $state || $state == Z* ]]
    printf 'pid=%s state=%s\n' "$pid" "${state:-exited}" >> "$evidence/cancel-observed.txt"
done < "$evidence/cancel-pids.txt"
ps -eo pid=,pgid=,stat= > "$evidence/cancel-processes.txt"
while read -r group; do
    if awk -v group="$group" '$2 == group && $3 !~ /^Z/ { found=1 } END { exit !found }' "$evidence/cancel-processes.txt"; then exit 1; fi
    printf 'group=%s live_members=0\n' "$group" >> "$evidence/cancel-observed.txt"
done < <(cat "$evidence/cancel/pdma_aligned_copy_test"/seed-*/group.txt)
printf 'PASS cancellation exit=143 all simulator/TERM-resistant descendant processes stopped\n'  >> "$evidence/assertions.txt"
sha256sum "$flow"/*.sh "$flow/tests.list" > "$evidence/source.sha256"
printf 'PDMA_FLOW_SELFTEST_PASS\n'
cat "$evidence/assertions.txt"
