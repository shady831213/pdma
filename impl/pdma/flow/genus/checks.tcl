# Native diagnostic and timing gates, shared by mapping and database audits.
proc check_errors {stage} {
    set errors [get_db messages -if {.severity == Error && .count > 0}]
    if {[llength $errors]} {error "PDMA_GENUS_ERRORS after $stage: [get_db $errors .name]"}
}
proc timing_status {} {
    set fd [open timing-coverage.rpt r]
    set coverage [read $fd]
    close $fd
    if {![regexp {Total:\s+([0-9]+)} $coverage -> count] || $count != 0} {
        error "PDMA_TIMING_COVERAGE: inspect timing-coverage.rpt"
    }
    set paths [report_timing -max_paths 1 -collection]
    if {[sizeof_collection $paths] != 1} {error "PDMA_TIMING_COVERAGE: missing critical path"}
    set slack [get_db $paths .slack]
    if {![string is double -strict $slack] || !($slack > -Inf && $slack < Inf)} {
        error "PDMA_TIMING_COVERAGE: nonfinite slack"
    }
    set fd [open timing-status.json w]
    puts $fd "{\"worst_setup_slack_ps\":$slack,\"setup_target_met\":[expr {$slack >= 0 ? "true" : "false"}]}"
    close $fd
    return [expr {$slack >= 0}]
}
