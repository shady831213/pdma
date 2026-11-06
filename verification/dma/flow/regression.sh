#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
source "$root/tools/rtl-flow/environment.sh"
build=$(realpath "${1:?Usage: regression.sh BUILD NEW_RUN [SEEDS...]}")
run=$(realpath -m "${2:?Usage: regression.sh BUILD NEW_RUN [SEEDS...]}")
shift 2
jobs=${PDMA_JOBS:-4}
[[ $jobs =~ ^[1-9][0-9]{0,3}$ ]] || { printf 'PDMA_JOBS must be in 1..9999\n' >&2; exit 2; }
if (($#==0)); then set -- 1 7 19 41 97 211 503 997; fi
seeds=("$@")
tests=()
if [[ ${PDMA_TESTS+x} ]]; then
    read -r -a tests <<< "${PDMA_TESTS//$'\n'/ }"
else
    mapfile -t tests < "$root/verification/dma/flow/tests.list"
fi
((${#tests[@]})) || { printf 'No tests selected\n' >&2; exit 2; }
for test_name in "${tests[@]}"; do
    [[ $test_name =~ ^[a-zA-Z_][a-zA-Z0-9_]*$ ]] || exit 2
    grep -Fxq "$test_name" "$root/verification/dma/flow/tests.list" || { printf 'Unknown test: %s\n' "$test_name" >&2; exit 2; }
done
for seed in "${seeds[@]}"; do [[ $seed =~ ^[1-9][0-9]{0,8}$ ]] || exit 2; done
[[ $(printf '%s\n' "${tests[@]}" | sort -u | wc -l) == ${#tests[@]} ]] || { printf 'Duplicate test selection\n' >&2; exit 2; }
[[ $(printf '%s\n' "${seeds[@]}" | sort -u | wc -l) == ${#seeds[@]} ]] || { printf 'Duplicate seed selection\n' >&2; exit 2; }
pdma_new_run "$run"
printf '%s\n' "${tests[@]}" > "$run/tests.list"
printf '%s\n' "${seeds[@]}" > "$run/seeds.list"
printf 'jobs=%s\n' "$jobs" > "$run/configuration.txt"
: > "$run/results.txt"
pids=() names=() case_seeds=() directories=() diagnostics=() mutations=()
head=0 tail=0 failed=0
cleanup() {
    local index
    for ((index=head; index<tail; index++)); do kill -TERM "${pids[index]}" 2>/dev/null || true; done
    for ((index=head; index<tail; index++)); do wait "${pids[index]}" 2>/dev/null || true; done
}
trap cleanup EXIT
trap 'printf "PDMA_REGRESSION_INTERRUPTED\n" >> "$run/results.txt"; exit 130' INT
trap 'printf "PDMA_REGRESSION_INTERRUPTED\n" >> "$run/results.txt"; exit 143' TERM
collect() {
    local index=$head status=0 verdict=FAIL log
    wait "${pids[index]}" || status=$?
    log=${directories[index]}/simulation.log
    if [[ -z ${diagnostics[index]} ]]; then
        if ((status==0)); then verdict=PASS; fi
        printf '%s test=%s seed=%s\n' "$verdict" "${names[index]}" "${case_seeds[index]}" >> "$run/results.txt"
    else
        if ((status!=0)) && [[ -f $log ]] &&
            grep -Eq "^UVM_FATAL .*\[${diagnostics[index]}\]" "$log" &&
            ! grep -Eq '^PDMA_VIP_PASS([[:space:]]|$)' "$log"; then verdict=REJECTED; fi
        printf '%s mutation=%s diagnostic=%s\n' "$verdict" "${mutations[index]}" "${diagnostics[index]}" >> "$run/results.txt"
    fi
    if [[ $verdict == FAIL ]]; then failed=1; fi
    head=$((head + 1))
}
launch() {
    local test_name=$1 seed=$2 directory=$3 diagnostic=$4 mutation=$5
    set --
    if ((tail-head >= jobs)); then collect; fi
    if [[ -n $mutation ]]; then set -- "+checker_mutation=$mutation"; fi
    bash "$root/verification/dma/flow/run.sh" "$build" "$directory" "$test_name" "$seed" "$@" \
        > "$run/case-$tail.log" 2>&1 &
    pids[tail]=$! names[tail]=$test_name case_seeds[tail]=$seed directories[tail]=$directory
    diagnostics[tail]=$diagnostic mutations[tail]=$mutation
    tail=$((tail + 1))
}
for test_name in "${tests[@]}"; do
    for seed in "${seeds[@]}"; do launch "$test_name" "$seed" "$run/$test_name/seed-$seed" '' ''; done
done
for mutation in data identity unknown_identity missing_completion duplicate_completion; do
    case "$mutation" in
        data) diagnostic=TASK_DATA ;;
        identity|unknown_identity) diagnostic=TASK_IDENTITY ;;
        missing_completion) diagnostic=TASK_TIMEOUT ;;
        duplicate_completion) diagnostic=TASK_DUPLICATE ;;
    esac
    launch pdma_aligned_copy_test 1 "$run/negative-$mutation" "$diagnostic" "$mutation"
done
while ((head<tail)); do collect; done
if ((failed)); then printf 'PDMA_REGRESSION_FAILED\n'; exit 1; fi
printf 'PDMA_REGRESSION_COMPLETE\n'
