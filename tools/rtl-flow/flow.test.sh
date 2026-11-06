#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/environment.sh"
fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT
printf 'PASS\n' > "$fixture/good.log"
printf 'PASS\nUVM_ERROR @ 10: observer [DATA] corrupted\n' > "$fixture/bad.log"
printf 'PASS\nUVM_ERROR scoreboard.svh(31) @ 10: observer [DATA] corrupted\n' > "$fixture/location.log"
printf 'PASS\nUVM_FATAL : 1\n' > "$fixture/summary.log"
printf 'PASS\nError-[DUT] assertion failed\n' > "$fixture/compiler.log"
pdma_check_log "$fixture/good.log" PASS
for name in bad location summary compiler; do
    if pdma_check_log "$fixture/$name.log" PASS; then
        printf 'Misleading success accepted: %s\n' "$name" >&2; exit 1
    fi
done
pdma_new_run "$fixture/run"
if pdma_new_run "$fixture/run" 2>/dev/null; then
    printf 'Stale output accepted\n' >&2; exit 1
fi
printf 'PDMA_FLOW_TEST_PASS\n'
