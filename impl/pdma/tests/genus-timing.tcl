source checks.tcl
proc run {} {
    read_db $::env(PDMA_MAPPED_DATABASE)
    check_errors restore
    check_timing_intent -verbose > timing-coverage.rpt
    if {![timing_status]} {error "Baseline setup target already failing"}
    file rename timing-status.json baseline.json
    # Deliberately tighten a real mapped path; no library or RTL substitution.
    set_units -time 1ns -capacitance 1pF
    create_clock -name core_clk -period 0.9 [get_ports core_clk]
    set_clock_uncertainty 0.05 [get_clocks core_clk]
    check_timing_intent -verbose > timing-coverage.rpt
    if {[timing_status]} {error "Negative setup slack was accepted"}
    check_errors timing_probe
    puts PDMA_TIMING_REJECTION_TEST_PASS
}
if {[catch {run} detail]} {
    puts stderr "PDMA_TIMING_TEST_FAILED: $detail"
    exit 1
}
exit 0
