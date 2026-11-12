#!/usr/bin/env bash
set -euo pipefail
main() {
    root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
    source "$root/tools/rtl-flow/environment.sh"
    source "$root/tools/rtl-flow/process.sh"
    pdma_process_pid=
    trap pdma_stop_process EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM
    if (($# < 4)); then
        printf 'Usage: schema.sh NEW_RUN TOP PASS_MARKER SOURCE...\n' >&2
        exit 2
    fi
    out=$(realpath -m "$1")
    top=$2
    marker=$3
    shift 3
    [[ $top =~ ^[A-Za-z_][A-Za-z0-9_]*$ && $marker =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]] || exit 2
    sources=()
    for source in "$@"; do sources+=("$(realpath -e "$source")"); done
    pdma_environment verification
    pdma_new_run "$out"
    cd "$out"
    pdma_execute "${PDMA_COMPILE_TIMEOUT:-300}" "$out/console.log" "${PDMA_SIM_COMPILER:-vcs}" \
        -full64 -sverilog -ntb_opts uvm-1.2 -timescale=1ns/1ps -top "$top" \
        "${sources[@]}" -Mdir="$out/csrc" -o "$out/simv" \
        -l "$out/compile.log"
    pdma_execute "${PDMA_RUN_TIMEOUT:-120}" "$out/run.log" "$out/simv" -l "$out/simulation.log"
    grep -Eq "^${marker}([[:space:]]|$)" "$out/simulation.log"
    if pdma_log_has_errors "$out/simulation.log"; then exit 1; fi
    printf 'PDMA_SCHEMA_SIMULATION_COMPLETE top=%s\n' "$top"
}
{ main "$@"; exit $?; }
