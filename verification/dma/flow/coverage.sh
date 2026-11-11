#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
source "$root/tools/rtl-flow/environment.sh"
build=$(realpath "${1:?Usage: coverage.sh BUILD REGRESSION NEW_REPORT}")
regression=$(realpath "${2:?Usage: coverage.sh BUILD REGRESSION NEW_REPORT}")
report=$(realpath -m "${3:?Usage: coverage.sh BUILD REGRESSION NEW_REPORT}")
# Old DUT runs predate explicit suites; TB qualification must never enter DUT coverage.
suite=
if [[ -f $regression/configuration.txt ]]; then
    suite=$(sed -n 's/^suite=//p' "$regression/configuration.txt")
fi
[[ ${suite:-dut} == dut ]] || { printf 'Only DUT regressions may enter DUT coverage reports\n' >&2; exit 2; }
pdma_environment verification
pdma_new_run "$report"
databases=("$build/coverage.vdb")
while IFS= read -r result; do
    [[ $result =~ ^PASS\ test=([a-zA-Z_][a-zA-Z0-9_]*)\ seed=([1-9][0-9]{0,8})$ ]] || continue
    test_name=${BASH_REMATCH[1]} seed=${BASH_REMATCH[2]}
    case_run="$regression/$test_name/seed-$seed"
    (cd "$case_run" && sha256sum --check --status artifacts.sha256)
    pdma_check_log "$case_run/simulation.log" "PDMA_VIP_PASS test=$test_name"
    test -d "$case_run/coverage.vdb"
    databases+=("$case_run/coverage.vdb")
done < "$regression/results.txt"
((${#databases[@]}>1)) || { printf 'No successful simulation coverage\n' >&2; exit 1; }
printf '%s\n' "${databases[@]}" > "$report/databases.list"
timeout "${PDMA_COVERAGE_TIMEOUT:-300}" "${PDMA_COVERAGE_TOOL:-urg}" \
    -dir "${databases[@]}" -report "$report/report" -format text -show summary > "$report/coverage.log" 2>&1
test -s "$report/report/dashboard.txt"
printf 'PDMA_COVERAGE_COMPLETE\n'
