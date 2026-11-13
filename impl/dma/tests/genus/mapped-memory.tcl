# Re-audit an immutable mapped candidate with the current helper implementation.
proc run {} {
    global output_load
    source checks.tcl
    source library-policy.tcl
    source config.tcl
    source ip.tcl
    rtl_ip::load ip-manifest.tcl
    source profile.tcl
    source memory-model.tcl
    source memory.tcl
    source memory-report.tcl
    source constraints-common.tcl
    source cdc.tcl
    set profile [profile::read design-profile.tcl]
    genus_memory::configure $profile $::env(PDMA_MEMORY_MODE)
    if {[dict size $::genus_memory::specs]} {
        memory_model::write_library current-memory.lib $::genus_memory::specs $output_load
        set fd [open current-memory.lib r]; set current [read $fd]; close $fd
        set fd [open reference-memory.lib r]; set original [read $fd]; close $fd
        if {$current ne $original} {error "MEMORY_GUARD: current emitter changes mapped library; fresh mapping required"}
    }
    read_db $::env(PDMA_MAPPED_DATABASE)
    set_units -time 1ns -capacitance 1pF
    if {[get_db auto_ungroup] ne "both" || [get_db [get_db current_design] .retime]} {
        error "PDMA_BOUNDARY_POLICY: restored optimization policy mismatch"
    }
    set allowed [genus_memory::report]
    library_policy::audit $approved_standard_cell_libraries $allowed
    check_timing_intent -verbose > timing-coverage.rpt
    if {![timing_status [genus_memory::clock_pins]] || ![cdc_status $profile]} {
        error "PDMA_MAPPED_MEMORY: timing/crossing budget failed"
    }
    check_errors mapped_memory
    puts PDMA_MAPPED_MEMORY_TEST_PASS
}
if {[catch {run} result]} {puts stderr "PDMA_MAPPED_MEMORY_FAILED: $result"; exit 1}
exit 0
