source checks.tcl

proc run {} {
    global env target_libraries approved_standard_cell_libraries operating_library operating_condition
    global time_unit capacitance_unit clock_uncertainty transition io_min io_max output_load
    source library-policy.tcl
    source config.tcl
    source memory-model.tcl
    source memory.tcl
    source memory-report.tcl
    source ip.tcl
    rtl_ip::load ip-manifest.tcl
    source profile.tcl
    set design_profile [profile::read design-profile.tcl]
    genus_memory::configure $design_profile $env(PDMA_MEMORY_MODE)
    set_db max_cpus_per_server 4
    set_db auto_ungroup both
    set_db syn_generic_effort medium
    set_db syn_map_effort high
    set_db syn_opt_effort high
    set_db hdl_error_on_latch true
    set_db limit_lbr_messages false
    set fd [open technology.sha256 w]
    foreach library $target_libraries {puts $fd [exec sha256sum $library]}
    close $fd
    read_libs [concat $target_libraries [genus_memory::library]]
    check_errors read_library
    library_policy::prepare $approved_standard_cell_libraries
    set library [get_db libraries -if [list .base_name == $operating_library]]
    if {abs([get_db $library .time_scale_in_ps]*1e-12-$time_unit)>1e-20 ||
        abs([get_db $library .cap_scale_in_ff]*1e-15-$capacitance_unit)>1e-23} {error "Library units mismatch"}
    set_db operating_conditions $operating_condition
    report_units $library > units.rpt
    set fd [open $env(PDMA_FILELIST) r]
    set files [split [string trim [read $fd]] \n]
    close $fd
    read_hdl -language sv -define SYNTHESIS $files
    genus_memory::blackbox
    set parameters {}
    if {[dict exists $design_profile parameters]} {
        dict for {name value} [dict get $design_profile parameters] {lappend parameters [list $name $value]}
    }
    if {[llength $parameters]} {elaborate -parameters $parameters [dict get $design_profile top]} \
    else {elaborate [dict get $design_profile top]}
    check_errors elaborate
    genus_memory::bind
    set_db [get_db modules] .boundary_opto true
    set_db [get_db current_design] .retime false
    set checked 0
    foreach module [get_db modules] {
        if {[get_db $module .arch_filename] in $files} {
            if {[get_db $module .boundary_opto] ne "true"} {error "PDMA_BOUNDARY_POLICY: module opt disabled"}
            incr checked
        }
    }
    # A top containing only directly bound RAM leaves has no remaining RTL
    # submodules; there are then no hierarchical boundaries to audit.
    if {[get_db auto_ungroup] ne "both" || [get_db [get_db current_design] .retime]} {
        error "PDMA_BOUNDARY_POLICY: ungroup/retiming configuration"
    }
    puts "PDMA_BOUNDARY_POLICY_PASS native_modules=$checked auto_ungroup=both boundary_opto=true retime=false"
    source timing.tcl
    source cdc.tcl
    genus_memory::prepare
    check_errors constraints
    write_sdc > applied.sdc
    if {$env(PDMA_SYNTH_STAGE) eq "prepare"} {
        write_db -all_root_attributes prepared.db
        check_errors prepared_database
        exec sha256sum --check --status technology.sha256
        puts PDMA_GENUS_PREPARE_COMPLETE
        return
    }
    set timings [open stage-wall.tsv w]
    puts $timings "stage\telapsed_seconds"
    fconfigure $timings -buffering line
    foreach command {syn_generic syn_map syn_opt} {
        set start [clock milliseconds]
        $command
        check_errors $command
        if {$command eq "syn_map"} {cdc_preserve $design_profile}
        puts $timings "$command\t[expr {([clock milliseconds]-$start)/1000.0}]"
    }
    close $timings
    write_db -all_root_attributes copy.db
    set memory_cells [genus_memory::report]
    library_policy::audit $approved_standard_cell_libraries $memory_cells
    check_timing_intent -verbose > timing-coverage.rpt
    report_area > area.rpt
    report_qor > qor.rpt
    report_gates > references.rpt
    report_design_rules > design-rules.rpt
    report_timing -max_paths 20 > timing.rpt
    report_clocks > clocks.rpt
    set cdc_pass [cdc_status $design_profile]
    # Vectorless exploration only. Do not call this measured workload power.
    set_db lp_default_toggle_rate 0.02
    set_db lp_default_probability 0.5
    report_power -unit mW > power.rpt
    write_hdl > copy.v
    write_sdc > applied.sdc
    check_errors reports
    exec sha256sum --check --status technology.sha256
    puts PDMA_GENUS_REPORTS_COMPLETE
    if {![timing_status [genus_memory::clock_pins]] || !$cdc_pass} {
        puts PDMA_GENUS_SETUP_TARGET_NOT_MET
        return 3
    }
    puts PDMA_GENUS_SETUP_TARGET_MET
    return 0
}
if {[catch {run} detail]} {
    puts stderr "PDMA_GENUS_FAILED: $detail"
    exit 1
}
if {$detail eq ""} {set detail 0}
exit $detail
