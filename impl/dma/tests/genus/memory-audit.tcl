proc rejected {script pattern} {
    if {![catch {uplevel 1 $script} reason] || ![string match $pattern $reason]} {
        error "Expected $pattern, received $reason"
    }
    puts "PDMA_MEMORY_NEGATIVE_PASS $reason"
}
proc run {} {
    global output_load
    set ::env(PDMA_MEMORY_MODE) control-budget
    source checks.tcl
    source library-policy.tcl
    source config.tcl
    source profile.tcl
    source memory-model.tcl
    source memory.tcl
    source memory-report.tcl
    read_db $::env(PDMA_MAPPED_DATABASE)
    genus_memory::configure [profile::read design-profile.tcl] control-budget
    set boxes [genus_memory::inventory]
    if {[llength $boxes] != 2} {error "Test requires two equal-shaped distinct owners"}
    check_timing_intent -verbose > timing-coverage.rpt
    set clocks [genus_memory::clock_pins]
    if {![timing_status $clocks]} {error "Expected baseline boundary timing to pass"}
    rejected {timing_status} {PDMA_TIMING_COVERAGE:*}
    set wrong $clocks
    lset wrong 0 pin:MemoryBoundaryTest/unrelated/clk
    rejected {timing_status $wrong} {PDMA_TIMING_COVERAGE:*}
    genus_memory::check_caps $boxes positive
    genus_memory::check_delays
    set first [genus_memory::bound first]
    set pin [get_db pins [get_db $first .name]/read_port_enable]
    set_output_delay -clock clk -max 0.01 $pin
    rejected {genus_memory::check_delays} {MEMORY_DELAY: changed accounting origin}
    set_output_delay -clock clk -max 0 $pin
    delete_obj [get_db $pin .external_delays]
    rejected {genus_memory::check_delays} {MEMORY_DELAY: missing or duplicate boundary delay}
    set_output_delay -clock clk -max 0 $pin
    set_output_delay -clock clk -min 0 $pin
    set expected $::output_load
    set ::output_load [expr {$expected*2}]
    rejected {genus_memory::check_caps $boxes wrong_load} {MEMORY_CAP: wrong library pin load}
    set ::output_load $expected
    set saved $::genus_memory::specs
    dict set ::genus_memory::specs first depth 7
    rejected {genus_memory::inventory} {MEMORY_GUARD: missing or ambiguous bound role*}
    set ::genus_memory::specs $saved
    dict set ::genus_memory::specs first width 12
    rejected {genus_memory::inventory} {MEMORY_GUARD: missing or ambiguous bound role*}
    set ::genus_memory::specs $saved
    dict unset ::genus_memory::specs second
    rejected {genus_memory::inventory} {MEMORY_GUARD: unexpected blackbox count}
    set ::genus_memory::specs $saved
    dict set ::genus_memory::specs first clock absent
    rejected {genus_memory::inventory} {MEMORY_GUARD: provider clock mismatch}
    set ::genus_memory::specs $saved
    # A library exception must name precisely the inventory-approved leaves.
    rejected {library_policy::audit $approved_standard_cell_libraries} {PDMA_LIBRARY_POLICY: unapproved mapped reference}
    set allowed [genus_memory::report]
    library_policy::audit $approved_standard_cell_libraries $allowed
    file copy -force memory-inventory.tsv memory-inventory-original.tsv
    if {[llength [get_db hinsts]]} {ungroup -all}
    genus_memory::report
    puts PDMA_MEMORY_HIERARCHY_PASS
    set module [create_module -name unresolved_control]
    set injected [create_hinst -name unresolved_control -module $module -parent [get_db current_design]]
    set_db $injected .unresolved true
    rejected {genus_memory::inventory} {MEMORY_GUARD: unresolved hierarchy}
    check_errors memory_tests
    puts PDMA_MEMORY_TEST_PASS
}
if {[catch {run} result]} {puts stderr "PDMA_MEMORY_TEST_FAILED: $result"; exit 1}
exit 0
