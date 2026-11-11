#!/usr/bin/env bash
set -euo pipefail
main() {
    root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
    source "$root/tools/rtl-flow/environment.sh"
    source "$root/tools/rtl-flow/process.sh"
    snapshot=$(realpath "${1:?Usage: genus-cdc.sh DUT_SNAPSHOT NEW_TEST}")
    run=$(realpath -m "${2:?Supply a fresh output directory}")
    (cd "$snapshot" && sha256sum --check --status snapshot.sha256)
    pdma_environment synthesis
    pdma_new_run "$run"
    mkdir "$run/snapshot"
    cp "$root/impl/pdma/tests/cdc-integration.sv" "$run/snapshot/"
    cat "$snapshot/sources.f" > "$run/snapshot/sources.f"
    printf '%s\n' "$run/snapshot/cdc-integration.sv" >> "$run/snapshot/sources.f"
    mapfile -t files < "$run/snapshot/sources.f"
    (cd "$run/snapshot" && sha256sum sources.f "${files[@]}" > snapshot.sha256)
    PDMA_DESIGN_PROFILE="$root/impl/pdma/tests/cdc-integration-profile.tcl" \
        bash "$root/impl/pdma/flow/genus.sh" "$run/snapshot" "$run/map"
    mkdir "$run/audit"
    cp "$run/map/"{checks.tcl,cdc.tcl,profile.tcl,constraints-common.tcl,duplex-mailbox.tcl,design-profile.tcl} "$run/audit/"
    cp "$root/impl/pdma/tests/genus-cdc.tcl" "$run/audit/"
    export PDMA_MAPPED_DATABASE="$run/map/copy.db"
    cd "$run/audit"
    sha256sum *.tcl "$PDMA_MAPPED_DATABASE" > inputs.sha256
    pdma_process_pid=
    trap pdma_stop_process EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM
    setsid timeout --kill-after=10s 180 "${PDMA_GENUS_TOOL:-genus}" -no_gui -batch \
        -abort_on_error -files genus-cdc.tcl -log audit > console.log 2>&1 &
    pdma_process_pid=$!
    wait "$pdma_process_pid"
    pdma_stop_process
    pdma_check_log audit.log PDMA_CDC_INTEGRATION_TESTS_PASS
    sha256sum --check --status inputs.sha256
    (cd "$run/map" && sha256sum --check --status artifacts.sha256)
    printf 'PDMA_CDC_INTEGRATION_TESTS_PASS\n'
}
{ main "$@"; exit $?; }
