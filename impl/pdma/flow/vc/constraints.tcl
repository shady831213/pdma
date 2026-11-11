# All external signal ownership comes from the selected design profile.
source constraints-common.tcl
set report [open clock-reset.rpt w]
set covered 0
set groups {}
foreach spec [profile::ports $design_profile] {
    lassign $spec name width direction clock
    set ports [get_ports $name]
    if {[sizeof_collection $ports] != $width} {error "PDMA_BOUNDARY_MISMATCH: $name width"}
    foreach_in_collection port $ports {
        if {[get_attribute $port direction] ne [dict get {input in output out} $direction]} {
            error "PDMA_BOUNDARY_MISMATCH: $name direction"
        }
    }
    incr covered $width
    dict lappend groups $clock $name
    puts $report "$name bits=$width direction=$direction domain=$clock"
}
if {$covered != [sizeof_collection [get_ports *]]} {error "PDMA_BOUNDARY_MISMATCH: unclassified ports"}
design_constraints::clocks $design_profile
dict for {clock settings} [dict get $design_profile clocks] {
    set reset [dict get $settings reset]
    create_reset -name $reset -async -value low [get_ports $reset]
}
# RTL CDC has no mapped register D/Q timing graph. Keep the shared transport
# budget visible, but apply/check physical Q-to-D exceptions only in STA.
if {[dict exists $design_profile mailboxes]} {
    dict for {scope spec} [dict get $design_profile mailboxes] {
        puts $report "mailbox=$scope source=[dict get $spec source] destination=[dict get $spec destination] max_delay_ns=[dict get $spec max_delay] physical_check=STA"
    }
}
if {[dict exists $design_profile reset_groups]} {
    dict for {name resets} [dict get $design_profile reset_groups] {
        # This constrains assertion only. Release remains independently clocked.
        set_reset_groups -name $name -group $resets
        puts $report "joint_assertion=$name resets=$resets"
    }
}
set_constraints_scope -module [dict get $design_profile top]
dict for {clock patterns} $groups {
    set reset [dict get $design_profile clocks $clock reset]
    define_attribute -name $clock
    set_clock_attribute $clock -clocks $clock
    set_reset_attribute $clock -resets $reset
    # Exclude clock/reset ports from ordinary data ownership.
    set data [lsearch -all -inline -not -exact $patterns $clock]
    set data [lsearch -all -inline -not -exact $data $reset]
    if {[llength $data]} {apply_attribute $clock -objects $data}
    define_attribute -name ${clock}_release
    set_clock_attribute ${clock}_release -clocks $clock
    apply_attribute ${clock}_release -objects $reset
}
end_constraints_scope
puts $report "classified_bits=$covered"
close $report
puts PDMA_STATIC_BOUNDARY_PASS
