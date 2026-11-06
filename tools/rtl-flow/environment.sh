#!/usr/bin/env bash
# Deployment files are trusted shell code and must stay outside version control.
pdma_environment() {
    if [[ -n ${PDMA_ENV:-} ]]; then
        source "$(realpath "$PDMA_ENV")"
    fi
    if declare -F pdma_setup >/dev/null; then
        pdma_setup "$1"
    fi
}

pdma_new_run() {
    local directory=$1
    if [[ -e "$directory" ]]; then
        printf 'Refusing to reuse an existing run directory: %s\n' "$directory" >&2
        return 2
    fi
    mkdir -p "$directory"
}

pdma_check_log() {
    local log=$1 marker=$2
    grep -Fxq "$marker" "$log" || return 1
    if grep -Eq '^Error[: -]|^Fatal:|^UVM_(ERROR|FATAL)[[:space:]].*@|UVM_(ERROR|FATAL)[[:space:]]*:[[:space:]]*[1-9]|Assertion.*failed' "$log"; then
        return 1
    fi
}
