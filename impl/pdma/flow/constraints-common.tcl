# Standalone design wrapper. Integration should call the reusable common-cell
# procedures directly, using its own clocks, hierarchy and timing budgets.
source duplex-mailbox.tcl
source async-queue.tcl
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
proc design_constraints::paths {profile {prefix ""}} {
    set paths {}
    foreach {family builder} {mailboxes duplex_paths async_queues async_queue_paths} {
        if {![dict exists $profile $family]} {continue}
        dict for {instance spec} [dict get $profile $family] {
            set scope [expr {$prefix eq "" ? $instance : "[string trimright $prefix /]/$instance"}]
            set source [get_clocks [dict get $spec source]]
            set destination [get_clocks [dict get $spec destination]]
            lappend paths {*}[common_cell_cdc::$builder $scope $source $destination [dict get $spec max_delay]]
        }
    }
    return $paths
}
proc design_constraints::crossings {profile {prefix ""}} {
    set paths [paths $profile $prefix]
    common_cell_cdc::apply_paths $paths
    return $paths
}
proc design_constraints::synchronizer_chains {profile} {
    set result {}
    foreach family {mailboxes async_queues} {
        if {![dict exists $profile $family]} {continue}
        dict for {scope spec} [dict get $profile $family] {
            set source [get_clocks [dict get $spec source]]
            set destination [get_clocks [dict get $spec destination]]
            if {$family eq "mailboxes"} {
                lappend result {*}[common_cell_cdc::duplex_chains $scope $source $destination]
            } else {
                lappend result {*}[common_cell_cdc::async_queue_chains $scope $source $destination [dict get $spec depth]]
            }
        }
    }
    return $result
}
