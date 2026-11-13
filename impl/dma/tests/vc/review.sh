#!/usr/bin/env bash
# Exercise review rejection in the native tool, using actual unchanged DUT reports.
set -euo pipefail
main() {
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)
source "$root/tools/rtl-flow/environment.sh"
source "$root/tools/rtl-flow/process.sh"
pdma_process_pid=
trap pdma_stop_process EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
snapshot=$(realpath "${1:?Usage: review.sh SNAPSHOT NEW_RUN}")
out=$(realpath -m "${2:?Supply a fresh output directory}")
pdma_new_run "$out"
policy="${PDMA_STATIC_REVIEW:-$root/impl/dma/reviews/copy.tcl}"
tclsh /dev/stdin "$policy" "$out" <<'TCL'
lassign $argv source out
set original [source $source]
foreach mode {stale mismatch omitted bad_group bad_count missing_sink} {
    set policy $original
    if {$mode eq "stale"} {
        dict set policy identity rtl stale
    } elseif {$mode eq "mismatch"} {
        set entries [dict get $policy findings]
        set entry [lindex $entries 0]
        dict set entry fields ConvergencePoint {nonexistent/endpoint}
        lset entries 0 $entry
        dict set policy findings $entries
    } elseif {$mode eq "omitted"} {
        # The first entry is a single CDC finding; the last may be RDC-only.
        dict set policy findings [lrange [dict get $policy findings] 1 end]
    } else {
        set entries [dict get $policy findings]
        set changed 0
        for {set i 0} {$i<[llength $entries]} {incr i} {
            set entry [lindex $entries $i]
            if {![dict exists $entry expected_count]} {continue}
            if {$mode eq "bad_group"} {
                dict set entry fields DestObjectType flop
            } elseif {$mode eq "bad_count"} {
                dict incr entry expected_count
            } else {
                dict unset entry fields DestObject
            }
            lset entries $i $entry
            set changed 1
            break
        }
        if {!$changed} {error "No grouped sink entry to exercise"}
        dict set policy findings $entries
    }
    set file [open [file join $out $mode.tcl] w]
    puts $file [list return $policy]
    close $file
}
TCL
for mode in accepted stale mismatch omitted bad_group bad_count missing_sink; do
    selected=$policy expected=0 marker='PDMA_STATIC_REVIEWED check=cdc' check=cdc
    case "$mode" in
        stale) expected=2; marker=PDMA_REVIEW_STALE ;;
        mismatch) expected=2; marker=PDMA_REVIEW_MISMATCH ;;
        omitted) expected=3; marker='PDMA_STATIC_FINDINGS check=cdc count=1' ;;
        bad_group|missing_sink) expected=2; marker=PDMA_REVIEW_GROUP; check=rdc ;;
        bad_count) expected=2; marker=PDMA_REVIEW_MISMATCH; check=rdc ;;
    esac
    [[ $mode == accepted ]] || selected="$out/$mode.tcl"
    status=0
    pdma_execute 0 "$out/$mode.log" env PDMA_STATIC_REVIEW="$selected" bash "$root/impl/dma/flow/crossings.sh" \
        "$snapshot" "$out/$mode" "$check" || status=$?
    [[ $status == "$expected" ]]
    if ((expected==2)); then
        grep -Eq "^PDMA_STATIC_FAILED: $marker(:| )" "$out/$mode/analysis.log" "$out/$mode/console.log"
    else
        grep -Fxq "$marker" "$out/$mode/analysis.log" "$out/$mode/console.log"
    fi
    if [[ $mode == omitted ]]; then
        read -r raw _ < "$out/accepted/finding-counts.txt"
        raw=${raw#raw=}
        [[ $raw =~ ^[0-9]+$ ]]
        grep -Fxq "raw=$raw reviewed=$((raw-1)) unresolved=1" "$out/$mode/finding-counts.txt"
    fi
    printf 'PDMA_REVIEW_TEST_PASS %s\n' "$mode"
done
printf 'PDMA_REVIEW_TESTS_PASS\n'
}
{ main "$@"; exit $?; }
