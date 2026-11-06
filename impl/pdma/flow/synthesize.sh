#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
source "$root/tools/rtl-flow/environment.sh"
snapshot=$(realpath "${1:?Usage: synthesize.sh SNAPSHOT NEW_RUN}")
run=$(realpath -m "${2:?Usage: synthesize.sh SNAPSHOT NEW_RUN}")
pdma_environment synthesis
: "${PDMA_TECH_CONFIG:?Set PDMA_TECH_CONFIG to a private technology Tcl file}"
export PDMA_TECH_CONFIG=$(realpath "$PDMA_TECH_CONFIG")
export PDMA_FILELIST="$snapshot/sources.f"
export PDMA_CONSTRAINTS="$root/impl/pdma/constraints/timing.tcl"
pdma_new_run "$run"
(cd "$snapshot" && sha256sum --check --status snapshot.sha256)
cd "$run"
cp "$root/impl/pdma/flow/synthesize.tcl" .
sha256sum synthesize.tcl "$PDMA_TECH_CONFIG" "$PDMA_CONSTRAINTS" "$snapshot/snapshot.sha256" > inputs.sha256
timeout "${PDMA_SYNTH_TIMEOUT:-1800}" "${PDMA_SYNTH_TOOL:-dc_shell}" \
    -f synthesize.tcl -output_log_file synthesis.log > console.log 2>&1
pdma_check_log synthesis.log PDMA_SYNTHESIS_COMPLETE
sha256sum --check --status inputs.sha256
(cd "$snapshot" && sha256sum --check --status snapshot.sha256)
sha256sum ./*.rpt applied.sdc copy.v copy.ddc > artifacts.sha256
printf 'PDMA_SYNTHESIS_COMPLETE\n'
