#!/usr/bin/env bash
set -euo pipefail
# Parse the complete entry before starting a long-running external process.
main() {
    root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
    source "$root/tools/rtl-flow/environment.sh"
    source "$root/tools/rtl-flow/implementation/run.sh"
    export PDMA_SYNTH_STAGE=${PDMA_SYNTH_STAGE:-map}
    case "$PDMA_SYNTH_STAGE" in prepare|map) ;; *) printf 'Invalid synthesis stage\n' >&2; exit 2 ;; esac
    export PDMA_MEMORY_MODE=${PDMA_MEMORY_MODE:-control-budget}
    case "$PDMA_MEMORY_MODE" in generic|control-budget) ;; *) printf 'Invalid memory mode\n' >&2; exit 2 ;; esac
    snapshot=$(realpath "${1:?Usage: genus.sh SNAPSHOT NEW_RUN}")
    run=$(realpath -m "${2:?Supply a fresh output directory}")
    pdma_environment synthesis
    : "${PDMA_TECH_CONFIG:?Supply a private technology Tcl file}"
    pdma_impl_prepare "$snapshot" "$run"
    cp "$PDMA_TECH_CONFIG" "$run/technology.tcl"
    cp "$root/tools/rtl-flow/implementation/genus/"*.tcl "$run/"
    cp "$root/tools/rtl-flow/implementation/memory-model.tcl" "$run/"
    pdma_impl_capture adapter=genus "stage=$PDMA_SYNTH_STAGE" "memory=$PDMA_MEMORY_MODE" "timeout=${PDMA_SYNTH_TIMEOUT:-1800}"
    pdma_impl_execute "${PDMA_SYNTH_TIMEOUT:-1800}" \
        "${PDMA_GENUS_TOOL:-genus}" -no_gui -batch -abort_on_error \
        -files synth.tcl -log genus
    marker=PDMA_GENUS_REPORTS_COMPLETE
    reports=(area.rpt timing.rpt clocks.rpt timing-coverage.rpt timing-status.json cdc-timing.tsv cdc-timing-status.json power.rpt library-policy.tsv references.rpt design-rules.rpt copy.v copy.db memory-budgets.tsv memory-static-pins.tsv memory-load-post.tsv memory-qualification.txt)
    if [[ $PDMA_SYNTH_STAGE == prepare ]]; then
        marker=PDMA_GENUS_PREPARE_COMPLETE
        reports=(prepared.db)
    fi
    pdma_check_log genus.log "$marker"
    for report in "${reports[@]}" units.rpt applied.sdc constraint-coverage.rpt memory-inventory.tsv memory-load-pre.tsv; do
        test -f "$report" && test -s "$report"
    done
    printf '%s\n' "$marker"
}
{ main "$@"; exit $?; }
