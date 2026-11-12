#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
source "$root/tools/rtl-flow/environment.sh"
source "$root/tools/rtl-flow/implementation/run.sh"
export PDMA_DESIGN_PROFILE="$root/impl/dma/profiles/copy.tcl"
out=$(realpath -m "${1:?Supply a fresh test evidence directory}")
pdma_new_run "$out"
mkdir "$out/source with spaces"
printf 'fixture\n' > "$out/source with spaces/design.sv"
printf '%s\n' "$out/source with spaces/design.sv" > "$out/source with spaces/sources.f"
(cd "$out/source with spaces" && sha256sum design.sv sources.f > snapshot.sha256)
for mode in success tool-failure timeout changed-input; do
    status=0
    (
        pdma_impl_prepare "$out/source with spaces" "$out/$mode"
        pdma_impl_capture adapter=test "case=$mode"
        case "$mode" in
            tool-failure) pdma_impl_execute 10 bash -c 'exit 7' || exit $? ;;
            timeout) pdma_impl_execute 1 sleep 30 || exit $? ;;
            changed-input) printf 'changed\n' >> design-profile.tcl ;;
        esac
    ) > "$out/$mode.log" 2>&1 || status=$?
    case "$mode" in success) expected=0 ;; tool-failure) expected=7 ;; timeout) expected=124 ;; changed-input) expected=2 ;; esac
    [[ $status == "$expected" ]] || { printf 'Unexpected status: %s %s\n' "$mode" "$status" >&2; exit 1; }
    grep -Fxq "{\"flow_exit_status\":$expected}" "$out/$mode/result.json"
    (cd "$out/$mode" && sha256sum --check --status artifacts.sha256)
done
if (pdma_impl_prepare "$out/source with spaces" "$out/success") > "$out/reuse.log" 2>&1; then
    printf 'Stale run accepted\n' >&2; exit 1
fi
(cd "$out/success" && sha256sum --check --status artifacts.sha256)
printf '{}\n' > "$out/source with spaces/simulation-only.json"
if (pdma_impl_prepare "$out/source with spaces" "$out/instrumented") > "$out/instrumented.log" 2>&1; then
    printf 'Simulation-only snapshot accepted for implementation\n' >&2; exit 1
fi
grep -Fxq 'Simulation-only instrumentation cannot qualify implementation' "$out/instrumented.log"
printf 'PDMA_IMPLEMENTATION_RUN_TEST_PASS\n'
