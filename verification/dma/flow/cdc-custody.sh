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
    snapshot=$(realpath "${1:?Usage: cdc-custody.sh SNAPSHOT NEW_RUN [mailbox|queue]}")
    run=$(realpath -m "${2:?Supply a fresh output directory}")
    kind=${3:-mailbox}
    case "$kind" in mailbox) top=CdcCustodyTest ;; queue) top=CdcQueueCustodyTest ;; *) exit 2 ;; esac
    pdma_environment verification
    pdma_new_run "$run"
    (cd "$snapshot" && sha256sum --check --status snapshot.sha256)
    custody=$(python3 "$root/tools/rtl-flow/ip/bundle.py" locate "$snapshot" common_cell verification/cdc_custody.sv)
    cd "$run"
    sha256sum "$custody" "$root/verification/dma/tb/bindings.sv" \
        "$root/verification/dma/tests/cdc_custody.sv" > inputs.sha256
    pdma_execute 180 compile.log "${PDMA_SIM_COMPILER:-vcs}" -full64 -sverilog \
        -timescale=1ns/1ps -f "$snapshot/sources.f" \
        "$custody" "$root/verification/dma/tb/bindings.sv" \
        "$root/verification/dma/tests/cdc_custody.sv" -top "$top" \
        -o "$run/simv"
    : > results.txt
    for mode in positive data duplicate overwrite backpressure; do
        set --
        [[ $mode == positive ]] || set -- "+mutation=$mode"
        status=0
        pdma_execute 30 "$mode.log" "$run/simv" "$@" || status=$?
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
            if [[ $kind == queue ]]; then
                case "$mode" in
                    data) diagnostic=CDC_QUEUE_DATA ;;
                    duplicate) diagnostic=CDC_QUEUE_UNOWNED ;;
                    overwrite) diagnostic=CDC_QUEUE_OVERFLOW ;;
                    backpressure) diagnostic=CDC_QUEUE_BACKPRESSURE ;;
                esac
            fi
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
