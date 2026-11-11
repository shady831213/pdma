# Standalone design wrapper. Integration should call the reusable common-cell
# procedures directly, using its own clocks, hierarchy and timing budgets.
source duplex-mailbox.tcl
namespace eval design_constraints {}
proc design_constraints::clocks {profile} {
    set file [open clocks.sdc w]
    set groups {set_clock_groups -name standalone_domains -asynchronous -allow_paths}
    dict for {clock settings} [dict get $profile clocks] {
        puts $file [format {create_clock -name %s -period %s [get_ports %s]} \
            [list $clock] [dict get $settings period] [list $clock]]
        append groups [format { -group [get_clocks %s]} [list $clock]]
    }
    # Keep the asynchronous relationship without cutting the physical max paths.
    puts $file $groups
    close $file
    read_sdc clocks.sdc
}
proc design_constraints::mailbox_paths {profile {prefix ""}} {
    set paths {}
    if {![dict exists $profile mailboxes]} {return $paths}
    dict for {instance spec} [dict get $profile mailboxes] {
        set scope [expr {$prefix eq "" ? $instance : "[string trimright $prefix /]/$instance"}]
        set source [get_clocks [dict get $spec source]]
        set destination [get_clocks [dict get $spec destination]]
        lappend paths {*}[common_cell_cdc::duplex_paths $scope $source $destination [dict get $spec max_delay]]
    }
    return $paths
}
proc design_constraints::mailboxes {profile {prefix ""}} {
    set paths [mailbox_paths $profile $prefix]
    common_cell_cdc::apply_paths $paths
    return $paths
}
