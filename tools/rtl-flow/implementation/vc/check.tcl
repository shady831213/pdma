# VC SpyGlass flow; reference command sequence: pcie_modeling's mailbox probe.
# No design waiver or reset assumption is inherited from that probe.
proc run {} {
    global env
    set mode $env(PDMA_STATIC_CHECK)
    if {$mode ni {cdc cdc-formal rdc}} {error "Invalid static check"}
    set check [expr {$mode eq "cdc-formal" ? "cdc" : $mode}]
    set_app_var enable_$check true
    source ip.tcl
    rtl_ip::load ip-manifest.tcl
    source profile.tcl
    set design_profile [profile::read design-profile.tcl]
    analyze -format sverilog -vcs "-f $env(PDMA_FILELIST) +define+SYNTHESIS"
    set parameters {}
    if {[dict exists $design_profile parameters]} {
        dict for {name value} [dict get $design_profile parameters] {lappend parameters "$name=$value"}
    }
    if {[llength $parameters]} {elaborate [dict get $design_profile top] -parameters [join $parameters ,]} \
    else {elaborate [dict get $design_profile top]}
    source constraints.tcl
    if {$check eq "rdc"} {
        # Follow resetless storage to observable consumers, including primary
        # outputs and disconnected sinks. Do not stop at an invalid payload row.
        configure_rdc_corrupt -report_rdc_on_cdc true -skip_resetless_flops true \
            -report_unconnected_destination true -report_port_destination_thru_seq true \
            -report_combo_port_destination true
    }
    check_$check
    if {$mode eq "cdc-formal"} {
        configure_cdc_formal -datahold true -dataloss all -msgmode all \
            -single_source false -atime 60
        check_cdc -type formal
    }
    report_$check -verbose -include_compressed -limit 0 -file raw.rpt
    set raw_count [llength [report_$check -severity {fatal error warning} -id_list]]
    set reviewed 0
    if {[file exists static-review.tcl]} {
        source review.tcl
        set reviewed [static_review::apply $check]
    }
    report_$check -severity {fatal error warning} -verbose -limit 0 -file unresolved.rpt
    set findings [report_$check -severity {fatal error warning} -id_list]
    set counts [open finding-counts.txt w]
    puts $counts "raw=$raw_count reviewed=$reviewed unresolved=[llength $findings]"
    close $counts
    if {[llength $findings]} {
        puts "PDMA_STATIC_FINDINGS check=$mode count=[llength $findings]"
        return 3
    }
    if {$reviewed} {
        puts "PDMA_STATIC_REVIEWED check=$mode"
    } else {
        puts "PDMA_STATIC_CLEAN check=$mode"
    }
    return 0
}
if {[catch {run} result]} {
    puts stderr "PDMA_STATIC_FAILED: $result"
    exit 2
}
exit $result
