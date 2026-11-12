#!/usr/bin/env bash
# Shared run ownership for implementation adapters. Source after environment.sh.
source "$root/tools/rtl-flow/process.sh"
pdma_impl_prepare() {
    pdma_snapshot=$(realpath "$1") || return
    if [[ -f $pdma_snapshot/simulation-only.json ]]; then
        printf 'Simulation-only instrumentation cannot qualify implementation\n' >&2
        return 2
    fi
    pdma_output=$(realpath -m "$2") || return
    (cd "$pdma_snapshot" && sha256sum --check --status snapshot.sha256) || return
    pdma_new_run "$pdma_output" || return
    export PDMA_FILELIST="$pdma_snapshot/sources.f"
    cp "$root/tools/rtl-flow/implementation/profile.tcl" "$pdma_output/" || return
    cp "$root/tools/rtl-flow/implementation/constraints-common.tcl" "$pdma_output/" || return
    cp "$root/tools/rtl-flow/ip/runtime.tcl" "$pdma_output/ip.tcl" || return
    if [[ -f $pdma_snapshot/ip-manifest.tcl ]]; then
        cp "$pdma_snapshot/ip-manifest.tcl" "$pdma_output/" || return
        cp -R "$pdma_snapshot/ip" "$pdma_output/ip" || return
    else
        printf 'return {}\n' > "$pdma_output/ip-manifest.tcl" || return
        mkdir "$pdma_output/ip" || return
    fi
    cp "${PDMA_DESIGN_PROFILE:?Supply the integrating design profile}" "$pdma_output/design-profile.tcl" || return
    cp "${BASH_SOURCE[1]}" "$pdma_output/launcher.sh" || return
    cp "$root/tools/rtl-flow/implementation/run.sh" "$pdma_output/flow-run.sh" || return
    cp "$root/tools/rtl-flow/environment.sh" "$pdma_output/flow-environment.sh" || return
    cp "$root/tools/rtl-flow/process.sh" "$pdma_output/flow-process.sh" || return
}

pdma_impl_capture() {
    (($#)) || return 2
    cd "$pdma_output" || return
    printf '%s\n' "$@" > settings.txt || return
    sha256sum ./*.tcl ./*.sh settings.txt "$pdma_snapshot/snapshot.sha256" > inputs.sha256 || return
    find ip -type f -print0 | sort -z | xargs -0 -r sha256sum >> inputs.sha256 || return
    pdma_process_pid=
    trap pdma_impl_finish EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM
}

pdma_impl_execute() {
    local seconds=$1 status=0
    shift
    printf '%q ' "$@" > invocation.txt
    printf '\n' >> invocation.txt
    pdma_execute "$seconds" console.log "$@" || status=$?
    return "$status"
}

pdma_impl_finish() {
    local status=$?
    trap - EXIT
    pdma_stop_process || status=2
    # Preserve provenance and failure status for unsuccessful runs too.
    sha256sum --check --status inputs.sha256 || status=2
    (cd "$pdma_snapshot" && sha256sum --check --status snapshot.sha256) || status=2
    find . -maxdepth 1 -type f ! -name artifacts.sha256 ! -name result.json \
        -print0 | sort -z | xargs -0 sha256sum > artifacts.sha256 || status=2
    find ip -type f -print0 | sort -z | xargs -0 -r sha256sum >> artifacts.sha256 || status=2
    printf '{"flow_exit_status":%s}\n' "$status" > result.json
    exit "$status"
}
