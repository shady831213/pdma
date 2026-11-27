#!/usr/bin/env bash
set -euo pipefail
flow=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
evidence=$(realpath -m "${1:?Usage: selftest.sh NEW_EVIDENCE}")
[[ ! -e $evidence ]] || { printf 'Evidence directory already exists\n' >&2; exit 2; }
mkdir -p "$evidence"
fixture=$(mktemp -d)
regression_pid=
cleanup() {
    if [[ -n $regression_pid ]]; then kill -TERM "$regression_pid" 2>/dev/null || true; wait "$regression_pid" 2>/dev/null || true; fi
    rm -rf "$fixture"
}
trap cleanup EXIT
unset PDMA_ENV
export FIXTURE_STATE="$fixture/state" PDMA_JOBS=2
export PDMA_TESTS='pdma_aligned_copy_test pdma_random_copy_test'
mkdir -p "$fixture/build/coverage.vdb" "$FIXTURE_STATE"
printf '0 0\n' > "$FIXTURE_STATE/count"
cat > "$fixture/build/simv" <<'SIM'
#!/usr/bin/env bash
set -euo pipefail
test_name= seed= mutation= coverage= clock_mode=
while (($#)); do
    case "$1" in
        +UVM_TESTNAME=*) test_name=${1#*=} ;;
        +ntb_random_seed=*) seed=${1#*=} ;;
        +checker_mutation=*) mutation=${1#*=} ;;
        +clock_mode=*) clock_mode=${1#*=} ;;
        -cm_dir) shift; coverage=$1 ;;
    esac
    shift
done
[[ -z ${FIXTURE_CLOCK_MODE:-} || $clock_mode == "$FIXTURE_CLOCK_MODE" ]]
mkdir -p "$coverage"
exec 9> "$FIXTURE_STATE/lock"
flock 9
read -r active maximum < "$FIXTURE_STATE/count"
active=$((active + 1))
if ((active>maximum)); then maximum=$active; fi
printf '%s %s\n' "$active" "$maximum" > "$FIXTURE_STATE/count"
printf 'START %s %s pid=%s active=%s\n' "$test_name" "$seed" "$$" "$active" >> "$FIXTURE_STATE/events"
flock -u 9
finish() {
    flock 9
    read -r active maximum < "$FIXTURE_STATE/count"
    printf '%s %s\n' "$((active - 1))" "$maximum" > "$FIXTURE_STATE/count"
    printf 'END %s %s pid=%s\n' "$test_name" "$seed" "$$" >> "$FIXTURE_STATE/events"
    flock -u 9
}
trap finish EXIT
if [[ ${FIXTURE_CANCEL:-0} == 1 ]]; then
    bash -c 'trap "" TERM; while :; do sleep 30; done' &
    printf '%s\n' "$$" "$!" > "$PWD/descendants.txt"
    ps -o pgid= -p "$$" > "$PWD/group.txt"
    wait
fi
sleep 0.2
if [[ -n $mutation ]]; then
    case "$mutation" in
        data) diagnostic=TASK_DATA ;;
        identity|unknown_identity) diagnostic=TASK_IDENTITY ;;
        missing_completion) diagnostic=TASK_TIMEOUT ;;
        duplicate_completion) diagnostic=TASK_DUPLICATE ;;
        early_fence|fatal_fence) diagnostic=TASK_COMPLETION ;;
        underreported_prefix|fractional_prefix) diagnostic=TASK_ERROR_INFO ;;
    esac
    printf 'UVM_FATAL fixture [%.100s] expected mutation\n' "$diagnostic" > simulation.log
    if [[ ${FIXTURE_FATAL_TIMEOUT:-0} == 1 && $mutation == early_fence ]]; then sleep 30; fi
    exit 1
fi
if [[ ${FIXTURE_FAIL:-} == "$test_name:$seed" ]]; then
    printf 'UVM_ERROR fixture @ 1: injected failure\n' > simulation.log
    exit 1
fi
if [[ ${FIXTURE_WRONG_MARKER:-0} == 1 ]]; then test_name=pdma_wrong_test; fi
printf 'PDMA_VIP_PASS test=%s\n' "$test_name" > simulation.log
mkdir -p "$coverage/snps/coverage/db/testdata/run"
printf '<test><srctest cmdline="%s" seed="%s"/></test>\n' "$test_name" "$seed" \
    > "$coverage/snps/coverage/db/testdata/run/siminfo.xml"
SIM
chmod +x "$fixture/build/simv"
(cd "$fixture/build" && sha256sum simv > executable.sha256)

if PDMA_SUITE=invalid bash "$flow/regression.sh" "$fixture/build" "$evidence/invalid-suite" 1 > "$evidence/invalid-suite.log" 2>&1; then
    printf 'FAIL unknown regression suite accepted\n' >&2; exit 1
fi
[[ ! -e $evidence/invalid-suite ]]
printf 'PASS unknown suite rejected before output allocation\n' > "$evidence/suite-assertions.txt"

PDMA_CLOCK_MODE=fixed FIXTURE_CLOCK_MODE=fixed bash "$flow/regression.sh" "$fixture/build" "$evidence/parallel" 1 7 > "$evidence/parallel.log" 2>&1
read -r active maximum < "$FIXTURE_STATE/count"
[[ $active == 0 && $maximum == 2 ]]
[[ $(grep -c '^PASS test=' "$evidence/parallel/results.txt") == 4 ]]
[[ $(grep -c '^REJECTED mutation=' "$evidence/parallel/results.txt") == 9 ]]
cp "$FIXTURE_STATE/events" "$evidence/parallel-events.txt"
printf 'PASS concurrency maximum=%s limit=2 active_at_end=%s positive=4 negative=9\n' "$maximum" "$active" > "$evidence/assertions.txt"

PDMA_SUITE=tb-selftest PDMA_TESTS="$(cat "$flow/tb-selftests.list")" bash "$flow/regression.sh" "$fixture/build" "$evidence/tb" 1 > "$evidence/tb.log" 2>&1
[[ $(grep -c '^PASS test=' "$evidence/tb/results.txt") == $(wc -l < "$flow/tb-selftests.list") ]]
! grep -q 'mutation=' "$evidence/tb/results.txt"
grep -Fxq 'suite=tb-selftest' "$evidence/tb/configuration.txt"
printf 'PASS TB selftests run independently without DUT checker mutations\n' >> "$evidence/suite-assertions.txt"

export FIXTURE_FAIL=pdma_aligned_copy_test:1
if bash "$flow/regression.sh" "$fixture/build" "$evidence/failure" 1 7 > "$evidence/failure.log" 2>&1; then exit 1; fi
[[ $(grep -c '^FAIL test=' "$evidence/failure/results.txt") == 1 ]]
[[ $(grep -c '^PASS test=' "$evidence/failure/results.txt") == 3 ]]
[[ $(grep -c '^REJECTED mutation=' "$evidence/failure/results.txt") == 9 ]]
unset FIXTURE_FAIL
printf 'PASS failure aggregation failed=1 passed=3 negative=9 exit=nonzero\n' >> "$evidence/assertions.txt"

if FIXTURE_FATAL_TIMEOUT=1 PDMA_RUN_TIMEOUT=1 bash "$flow/regression.sh" "$fixture/build" "$evidence/fatal-timeout" 1 > "$evidence/fatal-timeout.log" 2>&1; then
    printf 'FAIL expected diagnostic followed by timeout was accepted\n' >&2; exit 1
fi
grep -Fxq 'FAIL mutation=early_fence diagnostic=TASK_COMPLETION' "$evidence/fatal-timeout/results.txt"
printf 'PASS expected diagnostic cannot hide a simulator timeout\n' >> "$evidence/assertions.txt"

if FIXTURE_WRONG_MARKER=1 bash "$flow/run.sh" "$fixture/build" "$evidence/wrong-marker" pdma_aligned_copy_test 1 > "$evidence/wrong-marker.log" 2>&1; then exit 1; fi
printf 'PASS wrong test marker rejected\n' >> "$evidence/assertions.txt"
for argument in +UVM_TESTNAME=pdma_random_copy_test +ntb_random_seed=7; do
    if bash "$flow/run.sh" "$fixture/build" "$evidence/forbidden" pdma_aligned_copy_test 1 "$argument" >> "$evidence/invalid.log" 2>&1; then exit 1; fi
done
if bash "$flow/run.sh" "$fixture/build" "$evidence/forbidden" '../invalid' 1 >> "$evidence/invalid.log" 2>&1; then exit 1; fi
[[ ! -e $evidence/forbidden ]]
printf 'PASS test/seed overrides and unsafe identifier rejected before output allocation\n' >> "$evidence/assertions.txt"
if PDMA_CLOCK_MODE=invalid bash "$flow/regression.sh" "$fixture/build" "$evidence/forbidden" 1 >> "$evidence/invalid.log" 2>&1; then exit 1; fi
[[ ! -e $evidence/forbidden ]]
printf 'PASS explicit fixed-clock profile and invalid mode rejection\n' >> "$evidence/assertions.txt"

cat > "$fixture/urg" <<'URG'
#!/usr/bin/env python3
# Launcher/input-selection fixture only; native merge semantics are tested with URG.
from pathlib import Path
import os
import sys
import xml.etree.ElementTree as ET
args=sys.argv[1:]
if '-metric' in args and args[args.index('-metric')+1]=='group':
    result=Path(args[args.index('-report')+1])
    result.mkdir()
    # Native brief reports omit groups whose type-level bins are all covered.
    detail='Group : pdma_vip_pkg::partial\n'
    if 'brief' not in args:
        detail+='Group : pdma_vip_pkg::complete\n'
        detail+='Group : pdma_vip_top.control::maintenance_results\n'
    (result/'grpinfo.txt').write_text(detail)
    sys.exit(0)
if '-hier' in args:
    result=Path(args[args.index('-report')+1])
    result.mkdir()
    instances=['cfg','memory'] if result.name=='bus-ports' else ['dma','frontend','control','commands','transport','endpoint']
    if os.getenv('FIXTURE_MISSING_CONTROL') and result.name=='rtl':
        instances.remove('control')
    root_count=2 if result.name=='bus-ports' else 1
    (result/'hierarchy.txt').write_text(f'100.00 100.00 {root_count}/{root_count} pdma_vip_top\n'+
        ''.join(f'100.00 100.00 1/1 {name}\n' for name in instances))
    (result/'modinfo.txt').write_text('fixture coverage detail\n')
    sys.exit(0)
start=args.index('-dir')+1
end=args.index('-dbname')
merged=ET.Element('test')
for directory in args[start:end]:
    for path in Path(directory).glob('snps/coverage/db/testdata/*/siminfo.xml'):
        for source in ET.parse(path).iter('srctest'):
            merged.append(source)
database=Path(args[end+1]).parent.with_suffix('.vdb')
metadata=database/'snps/coverage/db/testdata/all/test.xml'
metadata.parent.mkdir(parents=True)
ET.ElementTree(merged).write(metadata)
if '-report' in args:
    report=Path(args[args.index('-report')+1])
    report.mkdir()
    (report/'dashboard.txt').write_text('fixture coverage report\n')
    (report/'groups.txt').write_text('COVERED EXPECTED AUTO BIN MAX\n1 2 pdma_vip_pkg::partial\n'
                                     '1 1 pdma_vip_pkg::complete\n1 1 pdma_vip_top.control::maintenance_results\n')
URG
chmod +x "$fixture/urg"
if PDMA_COVERAGE_TOOL="$fixture/urg" bash "$flow/coverage.sh" "$fixture/build" "$evidence/tb" "$evidence/tb-coverage" > "$evidence/tb-coverage.log" 2>&1; then exit 1; fi
[[ ! -e $evidence/tb-coverage ]]
printf 'PASS TB selftest coverage excluded from DUT reports\n' >> "$evidence/suite-assertions.txt"
PDMA_COVERAGE_TOOL="$fixture/urg" bash "$flow/coverage.sh" "$fixture/build" "$evidence/failure" "$evidence/coverage" > "$evidence/coverage.log" 2>&1
cmp "$evidence/failure/results.txt" "$evidence/coverage/regression-results.txt"
grep -Fxq 'total  3/3  100.00%' "$evidence/coverage/dma/external-ports.txt"
grep -Fxq 'Group : pdma_vip_pkg::complete' "$evidence/coverage/dma/functional.txt"
grep -Fxq 'Group : pdma_vip_top.control::maintenance_results' "$evidence/coverage/dma/functional.txt"
mkdir "$evidence/missing-control"
if FIXTURE_MISSING_CONTROL=1 python3 "$flow/dut_coverage.py" "$evidence/missing-control" \
    --tool "$fixture/urg" --metrics line+cond+tgl+branch > "$evidence/missing-control.log" 2>&1; then exit 1; fi
grep -Fq "no measured coverage for ['control']" "$evidence/missing-control.log"
printf 'PASS coverage rejects a report missing the control subtree\n' >> "$evidence/assertions.txt"
[[ $(wc -l < "$evidence/coverage/databases.list") == 3 ]]
! grep -Eq '/negative-|/pdma_aligned_copy_test/seed-1/' "$evidence/coverage/databases.list"
python3 - "$evidence/coverage/merge-result.json" <<'PY'
import json,sys
result=json.load(open(sys.argv[1]))
assert result['complete'] and result['source_records']==3
assert len(result['input_fingerprints'])==4 # design plus three selected simulations
PY
printf 'PASS coverage retains design+3 passed cases, excludes failed and negative cases\n' >> "$evidence/assertions.txt"

FIXTURE_CANCEL=1 bash "$flow/regression.sh" "$fixture/build" "$evidence/cancel" 1 7 > "$evidence/cancel.log" 2>&1 &
regression_pid=$!
deadline=$((SECONDS + 10))
while [[ ! -s $evidence/cancel/pdma_aligned_copy_test/seed-1/group.txt || ! -s $evidence/cancel/pdma_aligned_copy_test/seed-7/group.txt ]]; do
    ((SECONDS < deadline)) || exit 1
    sleep 0.05
done
kill -TERM "$regression_pid"
status=0
wait "$regression_pid" || status=$?
regression_pid=
[[ $status == 143 ]]
cat "$evidence/cancel/pdma_aligned_copy_test"/seed-*/descendants.txt > "$evidence/cancel-pids.txt"
while read -r pid; do
    state=$(ps -o stat= -p "$pid" || true)
    [[ -z $state || $state == Z* ]]
    printf 'pid=%s state=%s\n' "$pid" "${state:-exited}" >> "$evidence/cancel-observed.txt"
done < "$evidence/cancel-pids.txt"
ps -e -o pid= -o pgid= -o stat= > "$evidence/cancel-processes.txt"
while read -r group; do
    if awk -v group="$group" '$2 == group && $3 !~ /^Z/ { found=1 } END { exit !found }' "$evidence/cancel-processes.txt"; then exit 1; fi
    printf 'group=%s live_members=0\n' "$group" >> "$evidence/cancel-observed.txt"
done < <(cat "$evidence/cancel/pdma_aligned_copy_test"/seed-*/group.txt)
printf 'PASS cancellation exit=143 all simulator/TERM-resistant descendant processes stopped\n'  >> "$evidence/assertions.txt"
sha256sum "$flow"/*.sh "$flow"/*.py "$flow"/*.list > "$evidence/source.sha256"
printf 'PDMA_FLOW_SELFTEST_PASS\n'
cat "$evidence/assertions.txt"
