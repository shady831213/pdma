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
    snapshot=$(realpath "${1:?Usage: smoke.sh SNAPSHOT NEW_RUN}")
    run=$(realpath -m "${2:?Usage: smoke.sh SNAPSHOT NEW_RUN}")
    pdma_environment simulation
    pdma_new_run "$run"
    (cd "$snapshot" && sha256sum --check --status snapshot.sha256)
    cd "$run"
    cp "$root/verification/dma/tests/minimal_copy.sv" bench.sv
    cp "$root/verification/dma/tb/generated/contract_records.sv" records.sv
    sha256sum bench.sv records.sv "$snapshot/snapshot.sha256" > inputs.sha256
    pdma_execute "${PDMA_COMPILE_TIMEOUT:-600}" compiler-console.log "${PDMA_SIM_COMPILER:-vcs}" \
        -full64 -sverilog -timescale=1ns/1ps -f "$snapshot/sources.f" records.sv bench.sv \
        -top MinimalCopyTest -o simv -l compile.log
    pdma_execute "${PDMA_RUN_TIMEOUT:-120}" simulation-console.log ./simv -l simulation.log
    pdma_check_log simulation.log PDMA_MINIMAL_SV_PASS
    sha256sum --check --status inputs.sha256
    (cd "$snapshot" && sha256sum --check --status snapshot.sha256)
    sha256sum simv compile.log simulation.log > artifacts.sha256
    printf 'PDMA_SMOKE_COMPLETE\n'
}
{ main "$@"; exit $?; }
