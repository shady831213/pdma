#!/usr/bin/env bash
# Exercise review rejection in the native tool, using actual unchanged DUT reports.
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
source "$root/tools/rtl-flow/environment.sh"
snapshot=$(realpath "${1:?Usage: review.sh SNAPSHOT NEW_RUN}")
out=$(realpath -m "${2:?Supply a fresh output directory}")
pdma_new_run "$out"
policy="$root/impl/pdma/constraints/dma-copy-review.tcl"
tclsh /dev/stdin "$policy" "$out" <<'TCL'
lassign $argv source out
set original [source $source]
foreach mode {stale mismatch omitted} {
    set policy $original
    if {$mode eq "stale"} {
        dict set policy identity rtl stale
    } elseif {$mode eq "mismatch"} {
        set entries [dict get $policy findings]
        set entry [lindex $entries 0]
        dict set entry fields ConvergencePoint {nonexistent/endpoint}
        lset entries 0 $entry
        dict set policy findings $entries
    } else {
        dict set policy findings [lrange [dict get $policy findings] 0 end-1]
    }
    set file [open [file join $out $mode.tcl] w]
    puts $file [list return $policy]
    close $file
}
TCL
for mode in accepted stale mismatch omitted; do
    selected=$policy expected=0 marker='PDMA_STATIC_REVIEWED check=cdc'
    case "$mode" in
        stale) expected=2; marker=PDMA_REVIEW_STALE ;;
        mismatch) expected=2; marker=PDMA_REVIEW_MISMATCH ;;
        omitted) expected=3; marker='PDMA_STATIC_FINDINGS check=cdc count=1' ;;
    esac
    [[ $mode == accepted ]] || selected="$out/$mode.tcl"
    status=0
    PDMA_STATIC_REVIEW="$selected" bash "$root/impl/pdma/flow/crossings.sh" \
        "$snapshot" "$out/$mode" cdc > "$out/$mode.log" 2>&1 || status=$?
    [[ $status == "$expected" ]]
    grep -Fq "$marker" "$out/$mode/analysis.log" "$out/$mode/console.log"
    if [[ $mode == omitted ]]; then
        grep -Fxq 'raw=15 reviewed=14 unresolved=1' "$out/$mode/finding-counts.txt"
    fi
    printf 'PDMA_REVIEW_TEST_PASS %s\n' "$mode"
done
printf 'PDMA_REVIEW_TESTS_PASS\n'
