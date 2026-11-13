#!/usr/bin/env bash
set -euo pipefail
main() {
    root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)
    source "$root/tools/rtl-flow/environment.sh"
    source "$root/tools/rtl-flow/process.sh"
    mapped=$(realpath "${1:?Usage: genus-timing.sh MAPPED_RUN NEW_TEST}")
    run=$(realpath -m "${2:?Supply a fresh test output directory}")
    (cd "$mapped" && sha256sum --check --status artifacts.sha256)
    pdma_environment synthesis
    pdma_new_run "$run"
    cp "$root/tools/rtl-flow/implementation/genus/checks.tcl" "$root/impl/dma/tests/genus/genus-timing.tcl" "$run/"
    cp "$mapped/"{profile.tcl,memory-model.tcl,memory.tcl,memory-report.tcl,design-profile.tcl,ip.tcl,ip-manifest.tcl} "$run/"
    cp -R "$mapped/ip" "$run/"
    export PDMA_MEMORY_MODE=$(sed -n 's/^memory=//p' "$mapped/settings.txt")
    export PDMA_MAPPED_DATABASE="$mapped/copy.db"
    cd "$run"
    sha256sum *.tcl "$PDMA_MAPPED_DATABASE" > inputs.sha256
    find ip -type f -print0 | sort -z | xargs -0 -r sha256sum >> inputs.sha256
    pdma_process_pid=
    trap pdma_stop_process EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM
    status=0
    pdma_execute 180 console.log "${PDMA_GENUS_TOOL:-genus}" -no_gui -batch \
        -abort_on_error -files genus-timing.tcl -log timing || status=$?
    ((status==0)) || exit "$status"
    pdma_check_log timing.log PDMA_TIMING_REJECTION_TEST_PASS
    sha256sum --check --status inputs.sha256
    (cd "$mapped" && sha256sum --check --status artifacts.sha256)
    printf 'PDMA_TIMING_REJECTION_TEST_PASS\n'
}
{ main "$@"; exit $?; }
