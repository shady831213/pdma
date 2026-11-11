#!/usr/bin/env bash
set -euo pipefail
# Parse the complete entry before starting a long-running external process.
main() {
    root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
    source "$root/tools/rtl-flow/environment.sh"
    run=$(realpath -m "${1:?Supply a fresh test output directory}")
    pdma_environment synthesis
    pdma_new_run "$run"
    cp "$root/impl/pdma/tests/"{genus-policy.tcl,policy-cells.lib,policy-voltage.lib} "$run/"
    cp "$root/impl/pdma/flow/genus/library-policy.tcl" "$run/"
    cd "$run"
    sha256sum ./*.tcl ./*.lib > inputs.sha256
    timeout --kill-after=10s 180 "${PDMA_GENUS_TOOL:-genus}" -no_gui -batch \
        -abort_on_error -files genus-policy.tcl -log policy > console.log 2>&1
    pdma_check_log policy.log PDMA_GENUS_POLICY_TEST_PASS
    sha256sum --check --status inputs.sha256
    printf 'PDMA_GENUS_POLICY_TEST_PASS\n'
}
{ main "$@"; exit $?; }
