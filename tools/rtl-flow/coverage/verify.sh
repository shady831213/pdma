#!/usr/bin/env bash
# Native flat/tree equivalence over a caller-selected diagnostic subset, not a
# substitute for merging the complete qualified regression.
set -euo pipefail
main() {
    root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
    source "$root/tools/rtl-flow/environment.sh"
    source "$root/tools/rtl-flow/process.sh"
    pdma_process_pid=
    trap pdma_stop_process EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM
    design=$(realpath "${1:?Usage: verify.sh DESIGN_VDB INPUT_LIST NEW_RUN}")
    inputs=$(realpath "${2:?Supply 9..32 simulation databases}")
    out=$(realpath -m "${3:?Supply a fresh evidence directory}")
    count=$(awk 'NF {count++} END {print count+0}' "$inputs")
    ((count>=9 && count<=32)) || { printf 'Equivalence fixture needs 9..32 inputs\n' >&2; exit 2; }
    pdma_environment verification
    pdma_new_run "$out"
    for mode in flat tree; do
        batch=32; [[ $mode == tree ]] && batch=8
        pdma_execute 900 "$out/$mode.log" python3 "$root/tools/rtl-flow/coverage/merge.py" \
            --tool "${PDMA_COVERAGE_TOOL:-urg}" --design "$design" --inputs "$inputs" \
            --output "$out/$mode" --batch-size "$batch"
    done
    python3 - "$out" <<'PY'
from pathlib import Path
import json,re,sys
root=Path(sys.argv[1])
def metrics(mode):
    lines=(root/mode/'report/dashboard.txt').read_text().splitlines()
    return [line.split() for line in lines if re.match(r'^\s*\d+(?:\.\d+)?\s',line)]
a,b=metrics('flat'),metrics('tree')
assert a and a==b,'Flat/tree native metric numerators, denominators or scores differ'
left=json.loads((root/'flat/merge-result.json').read_text())
right=json.loads((root/'tree/merge-result.json').read_text())
assert left['source_records']==right['source_records']
(root/'equivalence.json').write_text(json.dumps({'metrics_equal':True,'source_records':left['source_records'],'metrics':a},indent=2)+'\n')
PY
    printf 'RTL_COVERAGE_EQUIVALENCE_PASS\n'
}
{ main "$@"; exit $?; }
