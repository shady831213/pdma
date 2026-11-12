#!/usr/bin/env bash
set -euo pipefail
# Parse the complete entry before starting a long-running external process.
main() {
    root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)
    source "$root/tools/rtl-flow/environment.sh"
    source "$root/tools/rtl-flow/process.sh"
    pdma_process_pid=
    trap pdma_stop_process EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM
    run=$(realpath -m "${1:?Supply a fresh test output directory}")
    pdma_environment synthesis
    pdma_new_run "$run"
    cp "$root/impl/dma/tests/genus/"{genus-policy.tcl,policy-cells.lib,policy-voltage.lib} "$run/"
    cp "$root/tools/rtl-flow/implementation/genus/library-policy.tcl" "$run/"
    cd "$run"
    sha256sum ./*.tcl ./*.lib > inputs.sha256
    pdma_execute 180 console.log "${PDMA_GENUS_TOOL:-genus}" -no_gui -batch \
        -abort_on_error -files genus-policy.tcl -log policy
    pdma_check_log policy.log PDMA_GENUS_POLICY_TEST_PASS
    sha256sum --check --status inputs.sha256
    printf 'PDMA_GENUS_POLICY_TEST_PASS\n'
}
{ main "$@"; exit $?; }
