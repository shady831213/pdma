#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
output=$(realpath -m "${1:?Usage: snapshot.sh NEW_OUTPUT_DIRECTORY [PROFILE_JSON]}")
project="${output}.project"
args=()
if [[ $# -ge 2 ]]; then args=(--profile "$2"); fi
python3 "$root/rtl/flow/configuration.py" "${args[@]}" --project "$project"
bash "$root/tools/rtl-flow/snapshot.sh" "$output" "$project"
cp "$project/"configuration.{json,tcl,svh} "$output/"
python3 "$root/rtl/flow/verification.py" "$output"
(cd "$output" && sha256sum configuration.json configuration.tcl configuration.svh dut_binding.svh >> snapshot.sha256)
printf 'PDMA_CONFIGURED_SNAPSHOT_COMPLETE\n'
