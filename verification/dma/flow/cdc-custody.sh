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
    snapshot=$(realpath "${1:?Usage: cdc-custody.sh SNAPSHOT NEW_RUN}")
    run=$(realpath -m "${2:?Supply a fresh output directory}")
    pdma_environment verification
    pdma_new_run "$run"
    (cd "$snapshot" && sha256sum --check --status snapshot.sha256)
    cd "$run"
    sha256sum "$root/verification/dma/tb/cdc_assertions.sv" \
        "$root/verification/dma/tests/cdc_custody.sv" > inputs.sha256
    setsid timeout --kill-after=5s 180 "${PDMA_SIM_COMPILER:-vcs}" -full64 -sverilog \
        -timescale=1ns/1ps -f "$snapshot/sources.f" \
        "$root/verification/dma/tb/cdc_assertions.sv" \
        "$root/verification/dma/tests/cdc_custody.sv" -top CdcCustodyTest \
        -o "$run/simv" > compile.log 2>&1 &
    pdma_process_pid=$!
    wait "$pdma_process_pid"
    pdma_stop_process
    : > results.txt
    for mode in positive data duplicate overwrite backpressure; do
        set --
        [[ $mode == positive ]] || set -- "+mutation=$mode"
        setsid timeout --kill-after=5s 30 "$run/simv" "$@" > "$mode.log" 2>&1 &
        pdma_process_pid=$!
        status=0
        wait "$pdma_process_pid" || status=$?
        pdma_stop_process
        printf 'simulation_exit=%s\n' "$status" > "$mode.status"
        if [[ $mode == positive ]]; then
            ((status==0))
            pdma_check_log "$mode.log" PDMA_CDC_CUSTODY_PASS
            printf 'PASS positive\n' >> results.txt
        else
            case "$mode" in
                data) diagnostic=CDC_CUSTODY_DATA ;;
                duplicate) diagnostic=CDC_CUSTODY_UNOWNED ;;
                overwrite) diagnostic=CDC_CUSTODY_OVERWRITE ;;
                backpressure) diagnostic=CDC_CUSTODY_BACKPRESSURE ;;
            esac
            # This simulator may return zero for $fatal. Require the exact fatal
            # diagnostic and absence of success; timeout/signal exits never qualify.
            ((status==0 || status==1))
            grep -q '^Fatal:' "$mode.log"
            grep -Fxq "$diagnostic" "$mode.log"
            ! grep -Fq PDMA_CDC_CUSTODY_PASS "$mode.log"
            printf 'REJECTED %s\n' "$mode" >> results.txt
        fi
    done
    sha256sum --check --status inputs.sha256
    (cd "$snapshot" && sha256sum --check --status snapshot.sha256)
    printf 'PDMA_CDC_CUSTODY_TESTS_PASS\n'
}
{ main "$@"; exit $?; }
