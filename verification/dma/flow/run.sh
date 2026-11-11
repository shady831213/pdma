#!/usr/bin/env bash
set -euo pipefail
main() {
    root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
    source "$root/tools/rtl-flow/environment.sh"
    source "$root/tools/rtl-flow/process.sh"
    pdma_process_pid=
    trap pdma_stop_process EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM
    build=$(realpath "${1:?Usage: run.sh BUILD NEW_RUN TEST SEED [SIM_ARGS...]}")
    run=$(realpath -m "${2:?Usage: run.sh BUILD NEW_RUN TEST SEED [SIM_ARGS...]}")
    test_name=${3:?Usage: run.sh BUILD NEW_RUN TEST SEED [SIM_ARGS...]}
    seed=${4:?Usage: run.sh BUILD NEW_RUN TEST SEED [SIM_ARGS...]}
    [[ $test_name =~ ^[a-zA-Z_][a-zA-Z0-9_]*$ ]] || { printf 'Test must be a safe identifier\n' >&2; exit 2; }
    [[ $seed =~ ^[1-9][0-9]{0,8}$ ]] || { printf 'Seed must be a positive integer below one billion\n' >&2; exit 2; }
    shift 4
    for argument in "$@"; do
        case "$argument" in
            +UVM_TESTNAME*|+ntb_random_seed*) printf 'Use the explicit test/seed arguments\n' >&2; exit 2 ;;
        esac
    done
    pdma_environment verification
    pdma_new_run "$run"
    (cd "$build" && sha256sum --check --status executable.sha256)
    cd "$run"
    printf '%s\n' "$seed" > seed.txt
    printf '%s\n' "$test_name" > test.txt
    mkdir tmp
    export TMPDIR="$run/tmp"
    command=("$build/simv" "+ntb_random_seed=$seed" "+UVM_TESTNAME=$test_name"
        +UVM_VERBOSITY=UVM_LOW -cm line+cond+tgl+branch -cm_dir "$run/coverage.vdb"
        -l simulation.log "$@")
    printf '%q ' "${command[@]}" > invocation.txt
    printf '\n' >> invocation.txt
    setsid timeout --kill-after=5s "${PDMA_RUN_TIMEOUT:-180}" "${command[@]}" > console.log 2>&1 &
    pdma_process_pid=$!
    status=0
    wait "$pdma_process_pid" || status=$?
    printf 'simulation_exit=%s\n' "$status" > status.txt
    pdma_stop_process
    if [[ $status != 0 ]] || ! pdma_check_log simulation.log "PDMA_VIP_PASS test=$test_name"; then
        printf 'PDMA_VIP_RUN_FAILED test=%s seed=%s\n' "$test_name" "$seed" >&2
        exit 1
    fi
    sha256sum simulation.log seed.txt test.txt invocation.txt status.txt > artifacts.sha256
    printf 'PDMA_VIP_RUN_COMPLETE test=%s seed=%s\n' "$test_name" "$seed"
}
{ main "$@"; exit $?; }
