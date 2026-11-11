#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
source "$root/tools/rtl-flow/environment.sh"
out=$(realpath -m "${1:?Supply a fresh selftest directory}")
pdma_new_run "$out"
export PDMA_DESIGN_PROFILE="$root/impl/pdma/tests/static-profile.tcl"
for mode in synchronized unsynchronized resetless; do
    snapshot="$out/$mode"
    mkdir "$snapshot"
    case "$mode" in
        unsynchronized) printf '`define PDMA_UNSYNCHRONIZED\n' ;;
        resetless) printf '`define PDMA_RESETLESS\n' ;;
    esac > "$snapshot/probe.sv"
    cat "$root/impl/pdma/tests/static-probe.sv" >> "$snapshot/probe.sv"
    printf '%s\n' "$snapshot/probe.sv" > "$snapshot/sources.f"
    (cd "$snapshot" && sha256sum probe.sv sources.f > snapshot.sha256)
done
run_case() {
    local name=$1 mode=$2 check=$3 expected=$4 tag=$5 status=0
    bash "$root/impl/pdma/flow/crossings.sh" "$out/$mode" "$out/$name" "$check" > "$out/$name.log" 2>&1 || status=$?
    if (( status != expected )); then
        printf 'Unexpected status %s: %s (expected %s)\n' "$status" "$name" "$expected" >&2
        return 1
    fi
    grep -q "$tag" "$out/$name/analysis.log" "$out/$name/raw.rpt" 2>/dev/null
    printf 'PDMA_STATIC_TEST_PASS %s\n' "$name"
}
run_case cdc-clean synchronized cdc 0 'PDMA_STATIC_CLEAN check=cdc'
run_case cdc-unsafe unsynchronized cdc 3 'CDC_UNSYNC'
# Independently reset source/destination is deliberately unsafe in this fixture.
run_case rdc-unsafe synchronized rdc 3 'RDC_CORRUPT_OBSERVED'
run_case rdc-resetless resetless rdc 3 'RDC_CORRUPT_OBSERVED'
sed 's/s_data/missing_input/g' "$PDMA_DESIGN_PROFILE" > "$out/bad-profile.tcl"
export PDMA_DESIGN_PROFILE="$out/bad-profile.tcl"
run_case bad-profile synchronized cdc 2 'PDMA_BOUNDARY_MISMATCH'
printf 'PDMA_STATIC_TESTS_PASS\n'
