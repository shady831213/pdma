#!/usr/bin/env bash
set -euo pipefail
# Parse the complete entry before starting a long-running external process.
main() {
    root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
    source "$root/tools/rtl-flow/environment.sh"
    source "$root/tools/rtl-flow/implementation/run.sh"
    snapshot=$(realpath "${1:?Usage: crossings.sh SNAPSHOT NEW_RUN cdc|cdc-formal|rdc}")
    run=$(realpath -m "${2:?Supply a fresh output directory}")
    export PDMA_STATIC_CHECK=${3:?Choose cdc, cdc-formal or rdc}
    case "$PDMA_STATIC_CHECK" in cdc|cdc-formal|rdc) ;; *) exit 2 ;; esac
    pdma_environment static
    pdma_impl_prepare "$snapshot" "$run"
    cp "$root/tools/rtl-flow/implementation/vc/"*.tcl "$run/"
    "${PDMA_STATIC_TOOL:-vc_static_shell}" -id > "$run/tool-version.txt" 2>&1
    marker=PDMA_STATIC_CLEAN
    if [[ -n ${PDMA_STATIC_REVIEW:-} ]]; then
        cp "$PDMA_STATIC_REVIEW" "$run/static-review.tcl"
        # Relative emitted filenames make this identity independent of run paths.
        rtl_hash=$(cd "$snapshot" && sha256sum ./*.sv | sha256sum)
        printf 'set review_identity [dict create rtl %s' "${rtl_hash%% *}" > "$run/review-inputs.tcl"
        for item in "design:$snapshot/design.sha256" "profile:$run/design-profile.tcl" \
            "constraints:$run/constraints.tcl"; do
            hash=$(sha256sum "${item#*:}")
            printf ' %s %s' "${item%%:*}" "${hash%% *}" >> "$run/review-inputs.tcl"
        done
        shared_hash=$(cd "$run" && {
            sha256sum constraints-common.tcl profile.tcl ip.tcl ip-manifest.tcl
            find ip -type f -print0 | sort -z | xargs -0 -r sha256sum
        } | sha256sum)
        printf ' common %s' "${shared_hash%% *}" >> "$run/review-inputs.tcl"
        analysis_hash=$(cd "$run" && sha256sum check.tcl review.tcl | sha256sum)
        printf ' analysis %s' "${analysis_hash%% *}" >> "$run/review-inputs.tcl"
        for item in "tool:$run/tool-version.txt"; do
            hash=$(sha256sum "${item#*:}")
            printf ' %s %s' "${item%%:*}" "${hash%% *}" >> "$run/review-inputs.tcl"
        done
        printf ']\n' >> "$run/review-inputs.tcl"
        marker=PDMA_STATIC_REVIEWED
    fi
    pdma_impl_capture adapter=vc "check=$PDMA_STATIC_CHECK" "timeout=${PDMA_STATIC_TIMEOUT:-600}" "gate=$marker"
    pdma_impl_execute "${PDMA_STATIC_TIMEOUT:-600}" \
        "${PDMA_STATIC_TOOL:-vc_static_shell}" -batch -no_init -no_restore -no_color \
        -f check.tcl -output_log_file analysis.log
    pdma_check_log analysis.log "$marker check=$PDMA_STATIC_CHECK"
    test -s raw.rpt
    test -s clock-reset.rpt
    printf '%s check=%s\n' "$marker" "$PDMA_STATIC_CHECK"
}
{ main "$@"; exit $?; }
