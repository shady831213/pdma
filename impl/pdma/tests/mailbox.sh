#!/usr/bin/env bash
# Isolate the real mailbox/type instantiation without abstracting its state.
set -euo pipefail
main() {
    root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
    source "$root/tools/rtl-flow/environment.sh"
    snapshot=$(realpath "${1:?Usage: mailbox.sh DUT_SNAPSHOT NEW_PROOF}")
    out=$(realpath -m "${2:?Supply a fresh proof output directory}")
    (cd "$snapshot" && sha256sum --check --status snapshot.sha256)
    pdma_new_run "$out"
    mkdir "$out/snapshot"
    cp "$root/impl/pdma/tests/mailbox.sv" "$out/snapshot/"
    cat "$snapshot/sources.f" > "$out/snapshot/sources.f"
    printf '%s\n' "$out/snapshot/mailbox.sv" >> "$out/snapshot/sources.f"
    mapfile -t sources < "$out/snapshot/sources.f"
    (cd "$out/snapshot" && sha256sum sources.f "${sources[@]}" > snapshot.sha256)
    export PDMA_DESIGN_PROFILE="$root/impl/pdma/tests/mailbox-profile.tcl"
    # Preserve the raw structural verdict: proof diagnostics are not a waiver.
    bash "$root/impl/pdma/flow/crossings.sh" "$out/snapshot" "$out/check" cdc-formal
}
{ main "$@"; exit $?; }
