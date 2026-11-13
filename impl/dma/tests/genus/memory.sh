#!/usr/bin/env bash
set -euo pipefail
main() {
    root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)
    source "$root/tools/rtl-flow/environment.sh"
    source "$root/tools/rtl-flow/process.sh"
    pdma_process_pid=
    trap pdma_stop_process EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM
    snapshot=$(realpath "${1:?Usage: memory.sh DUT_SNAPSHOT NEW_TEST [normal|minimum]}")
    run=$(realpath -m "${2:?Supply a fresh output directory}")
    case "${3:-normal}" in
        normal) profile=memory-profile.tcl ;;
        minimum) profile=memory-minimum-profile.tcl ;;
        *) printf 'Unknown memory test profile\n' >&2; exit 2 ;;
    esac
    pdma_environment synthesis
    (cd "$snapshot" && sha256sum --check --status snapshot.sha256)
    pdma_new_run "$run"
    mkdir "$run/snapshot"
    cp "$root/impl/dma/tests/genus/memory.sv" "$run/snapshot/"
    python3 "$root/tools/rtl-flow/ip/bundle.py" copy "$snapshot" "$run/snapshot"
    cat "$snapshot/sources.f" > "$run/snapshot/sources.f"
    printf '%s\n' "$run/snapshot/memory.sv" >> "$run/snapshot/sources.f"
    mapfile -t files < "$run/snapshot/sources.f"
    (cd "$run/snapshot" && {
        sha256sum sources.f ip-manifest.tcl ip-manifest.json "${files[@]}"
        find ip -type f -print0 | sort -z | xargs -0 -r sha256sum
    } > snapshot.sha256)
    pdma_execute 0 "$run/map-launch.log" env PDMA_MEMORY_MODE=control-budget \
        PDMA_DESIGN_PROFILE="$root/impl/dma/tests/genus/$profile" \
        bash "$root/impl/dma/flow/genus.sh" "$run/snapshot" "$run/map"
    mkdir "$run/audit"
    cp "$run/map/"{checks.tcl,library-policy.tcl,config.tcl,technology.tcl,profile.tcl,memory-model.tcl,memory.tcl,memory-report.tcl,design-profile.tcl} "$run/audit/"
    cp "$root/impl/dma/tests/genus/memory-audit.tcl" "$run/audit/"
    export PDMA_MAPPED_DATABASE="$run/map/copy.db"
    cd "$run/audit"
    sha256sum *.tcl "$PDMA_MAPPED_DATABASE" > inputs.sha256
    pdma_execute 180 console.log "${PDMA_GENUS_TOOL:-genus}" -no_gui -batch \
        -abort_on_error -files memory-audit.tcl -log audit
    pdma_check_log audit.log PDMA_MEMORY_TEST_PASS
    sha256sum --check --status inputs.sha256
    (cd "$run/map" && sha256sum --check --status artifacts.sha256)
    printf 'PDMA_MEMORY_TEST_PASS\n'
}
{ main "$@"; exit $?; }
