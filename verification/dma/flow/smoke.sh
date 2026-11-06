#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
source "$root/tools/rtl-flow/environment.sh"
snapshot=$(realpath "${1:?Usage: smoke.sh SNAPSHOT NEW_RUN}")
run=$(realpath -m "${2:?Usage: smoke.sh SNAPSHOT NEW_RUN}")
pdma_environment simulation
pdma_new_run "$run"
(cd "$snapshot" && sha256sum --check --status snapshot.sha256)
cd "$run"
cp "$root/verification/dma/tests/minimal_copy.sv" bench.sv
sha256sum bench.sv "$snapshot/snapshot.sha256" > inputs.sha256
timeout "${PDMA_COMPILE_TIMEOUT:-600}" "${PDMA_SIM_COMPILER:-vcs}" \
    -full64 -sverilog -timescale=1ns/1ps -f "$snapshot/sources.f" bench.sv \
    -top MinimalCopyTest -o simv -l compile.log > compiler-console.log 2>&1
timeout "${PDMA_RUN_TIMEOUT:-120}" ./simv -l simulation.log > simulation-console.log 2>&1
pdma_check_log simulation.log PDMA_MINIMAL_SV_PASS
sha256sum --check --status inputs.sha256
(cd "$snapshot" && sha256sum --check --status snapshot.sha256)
sha256sum simv compile.log simulation.log > artifacts.sha256
printf 'PDMA_SMOKE_COMPLETE\n'
