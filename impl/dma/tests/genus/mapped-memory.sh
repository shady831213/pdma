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
    mapped=$(realpath "${1:?Usage: mapped-memory.sh MAPPED_RUN NEW_TEST}")
    run=$(realpath -m "${2:?Supply a fresh output directory}")
    (cd "$mapped" && sha256sum --check --status artifacts.sha256)
    pdma_environment synthesis
    pdma_new_run "$run"
    # Reuse the producer's subject/configuration/IP resources, not current IP.
    cp "$mapped/"{technology.tcl,design-profile.tcl,ip-manifest.tcl} "$run/"
    cp -R "$mapped/ip" "$run/"
    if [[ -f $mapped/memory-budget.lib ]]; then cp "$mapped/memory-budget.lib" "$run/reference-memory.lib"; fi
    cp "$root/tools/rtl-flow/implementation/"{profile.tcl,memory-model.tcl,constraints-common.tcl} "$run/"
    cp "$root/tools/rtl-flow/ip/runtime.tcl" "$run/ip.tcl"
    cp "$root/tools/rtl-flow/implementation/genus/"{checks.tcl,library-policy.tcl,config.tcl,memory.tcl,memory-report.tcl,cdc.tcl} "$run/"
    cp "$root/impl/dma/tests/genus/mapped-memory.tcl" "$run/"
    export PDMA_MAPPED_DATABASE="$mapped/copy.db"
    export PDMA_MEMORY_MODE=$(sed -n 's/^memory=//p' "$mapped/settings.txt")
    cd "$run"
    sha256sum *.tcl "$PDMA_MAPPED_DATABASE" > inputs.sha256
    if [[ -f reference-memory.lib ]]; then sha256sum reference-memory.lib >> inputs.sha256; fi
    find ip -type f -print0 | sort -z | xargs -0 -r sha256sum >> inputs.sha256
    pdma_execute 600 console.log "${PDMA_GENUS_TOOL:-genus}" -no_gui -batch \
        -abort_on_error -files mapped-memory.tcl -log audit
    pdma_check_log audit.log PDMA_MAPPED_MEMORY_TEST_PASS
    sha256sum --check --status inputs.sha256
    (cd "$mapped" && sha256sum --check --status artifacts.sha256)
    printf 'PDMA_MAPPED_MEMORY_TEST_PASS\n'
}
{ main "$@"; exit $?; }
