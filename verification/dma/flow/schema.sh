#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
source "$root/tools/rtl-flow/environment.sh"
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
timeout "${PDMA_COMPILE_TIMEOUT:-300}" "${PDMA_SIM_COMPILER:-vcs}" \
    -full64 -sverilog -ntb_opts uvm-1.2 -timescale=1ns/1ps -top "$top" \
    "${sources[@]}" -Mdir="$out/csrc" -o "$out/simv" \
    -l "$out/compile.log" > "$out/console.log" 2>&1
timeout "${PDMA_RUN_TIMEOUT:-120}" "$out/simv" -l "$out/simulation.log" > "$out/run.log" 2>&1
grep -Eq "^${marker}([[:space:]]|$)" "$out/simulation.log"
if grep -Eq '^UVM_(ERROR|FATAL)([[:space:]]+[^[:space:]:]|[[:space:]]*:[[:space:]]*[1-9])' "$out/simulation.log"; then exit 1; fi
printf 'PDMA_SCHEMA_SIMULATION_COMPLETE top=%s\n' "$top"
