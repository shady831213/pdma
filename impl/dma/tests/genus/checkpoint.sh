#!/usr/bin/env bash
# A diagnostic mapped checkpoint must survive independently of final optimization.
set -euo pipefail
main() {
    root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)
    source "$root/tools/rtl-flow/environment.sh"
    source "$root/tools/rtl-flow/process.sh"
    pdma_process_pid=
    trap pdma_stop_process EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM
    snapshot=$(realpath "${1:?Usage: checkpoint.sh DUT_SNAPSHOT NEW_RUN}")
    out=$(realpath -m "${2:?Supply a fresh output directory}")
    pdma_new_run "$out"
    pdma_environment synthesis
    pdma_execute 0 "$out/full.log" env PDMA_SYNTH_STAGE=map \
        bash "$root/impl/dma/tests/genus/memory.sh" "$snapshot" "$out/full"
    (cd "$out/full/map" && sha256sum --check --status mapped.sha256)
    pdma_execute 0 "$out/mapped.log" env PDMA_SYNTH_STAGE=mapped PDMA_MEMORY_MODE=control-budget \
        PDMA_DESIGN_PROFILE="$root/impl/dma/tests/genus/memory-profile.tcl" \
        bash "$root/tools/rtl-flow/implementation/genus.sh" "$out/full/snapshot" "$out/mapped"
    test ! -e "$out/mapped/copy.db"
    if grep -Fxq PDMA_GENUS_REPORTS_COMPLETE "$out/mapped/genus.log"; then
        printf 'Mapped-only diagnostic claimed final qualification\n' >&2; exit 1
    fi
    mkdir "$out/restored"
    cp "$out/mapped/"{checks.tcl,library-policy.tcl,config.tcl,technology.tcl,profile.tcl,memory-model.tcl,memory.tcl,memory-report.tcl,design-profile.tcl} "$out/restored/"
    cp "$root/impl/dma/tests/genus/memory-audit.tcl" "$out/restored/"
    export PDMA_MAPPED_DATABASE="$out/mapped/mapped.db"
    cd "$out/restored"
    pdma_execute 180 console.log "${PDMA_GENUS_TOOL:-genus}" -no_gui -batch \
        -abort_on_error -files memory-audit.tcl -log audit
    pdma_check_log audit.log PDMA_MEMORY_TEST_PASS
    (cd "$out/mapped" && sha256sum --check --status mapped.sha256 && sha256sum --check --status artifacts.sha256)
    printf 'PDMA_MAPPED_CHECKPOINT_TEST_PASS\n'
}
{ main "$@"; exit $?; }
