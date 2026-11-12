# Native mapped-path qualification. Merely emitting an SDC command is not proof
# that a path remains timed or that a global exception did not mask its budget.
proc cdc_preserve {profile} {
    set count 0
    foreach chain [design_constraints::synchronizer_chains $profile] {
        lassign $chain scope clock expected
        set cells [common_cell_cdc::synchronizer_cells [list $chain]]
        set_dont_touch $cells
        incr count [sizeof_collection $cells]
    }
    puts "PDMA_CDC_SYNCHRONIZERS_PRESERVED count=$count"
}
proc cdc_path_status {from to budget file} {
    set paths [report_timing -from $from -to $to -max_paths 1 -collection]
    if {[sizeof_collection $paths] != 1} {error "PDMA_CDC_UNTIMED: $file"}
    report_timing -from $from -to $to -max_paths 1 > $file
    set fd [open $file r]
    set report [read $fd]
    close $fd
    # Native report uses ps; SDC API/budget uses ns. A max-delay exception appears
    # as Path Delay, whereas ordinary same-clock setup uses Capture Clock.
    if {$budget ne ""} {
        if {![regexp {Path Delay:\+\s+([0-9.]+)} $report -> actual] || abs($actual-1000*$budget)>0.51} {
            error "PDMA_CDC_BUDGET_MASKED: $file"
        }
    } elseif {[regexp {Path Delay:\+} $report]} {
        error "PDMA_CDC_LOCAL_EXCEPTION: $file"
    }
    set slack [get_db $paths .slack]
    if {![string is double -strict $slack] || !($slack > -Inf && $slack < Inf)} {
        error "PDMA_CDC_NONFINITE_SLACK: $file"
    }
    return $slack
}
proc cdc_status {profile} {
    set index 0
    set payloads 0
    set stages 0
    set passed 1
    set fd [open cdc-timing.tsv w]
    puts $fd "kind\tscope\tmax_delay_ns\tworst_slack_ps"
    # Resolve after mapping without reapplying constraints: this checks that the
    # original exceptions survived synthesis and remain effective.
    foreach path [design_constraints::paths $profile] {
        set scope [dict get $path scope]
        set budget [dict get $path max_delay]
        set from [dict get $path from]
        set to [dict get $path to]
        set slack [cdc_path_status $from $to $budget cdc-path-$index.rpt]
        puts $fd "[dict get $path kind]\t$scope\t$budget\t$slack"
        if {$slack < 0} {set passed 0}
        if {[dict get $path kind] in {payload fifo-data-and-pointer}} {
            set data_name [expr {[dict get $path kind] eq "payload" ? "latched_data" : "ram"}]
            set data ""
            foreach_in_collection cell $from {
                if {[string first "$scope/$data_name" [get_object_name $cell]] == 0} {
                    set data [add_to_collection $data $cell]
                }
            }
            if {[sizeof_collection $data] == 0} {error "PDMA_CDC_PAYLOAD_MISSING: $scope"}
            set slack [cdc_path_status $data $to $budget cdc-payload-$payloads.rpt]
            puts $fd "held-data\t$scope\t$budget\t$slack"
            if {$slack < 0} {set passed 0}
            incr payloads
        }
        incr index
    }
    foreach chain [design_constraints::synchronizer_chains $profile] {
        lassign $chain scope clock expected
        set cells [common_cell_cdc::synchronizer_cells [list $chain]]
        set slack [cdc_path_status $cells $cells "" cdc-stage-$stages.rpt]
        puts $fd "local-stage\t$scope\t-\t$slack"
        if {$slack < 0} {set passed 0}
        incr stages
    }
    close $fd
    set fd [open cdc-timing-status.json w]
    puts $fd "{\"crossings\":$index,\"payloads\":$payloads,\"synchronizer_chains\":$stages,\"passed\":[expr {$passed ? "true" : "false"}]}"
    close $fd
    return $passed
}
