# Integration owns clocks and instance bindings. Package providers own their
# private hierarchy and protocol-specific paths; this adapter does not infer it.
namespace eval design_constraints {}
proc design_constraints::clocks {profile} {
    set file [open clocks.sdc w]
    set groups {set_clock_groups -name standalone_domains -asynchronous -allow_paths}
    dict for {clock settings} [dict get $profile clocks] {
        puts $file [format {create_clock -name %s -period %s [get_ports %s]} \
            [list $clock] [dict get $settings period] [list $clock]]
        append groups [format { -group [get_clocks %s]} [list $clock]]
    }
    puts $file $groups
    close $file
    read_sdc clocks.sdc
}
proc design_constraints::describe {profile {prefix ""}} {
    set result {}
    if {![dict exists $profile crossings]} {return $result}
    dict for {instance spec} [dict get $profile crossings] {
        set scope [expr {$prefix eq "" ? $instance : "[string trimright $prefix /]/$instance"}]
        set source [get_clocks [dict get $spec source]]
        set destination [get_clocks [dict get $spec destination]]
        set provider [dict get $spec provider]
        set description [rtl_ip::invoke $provider describe $scope $source $destination $spec]
        dict set description provider $provider
        lappend result $description
    }
    return $result
}
proc design_constraints::paths {profile {prefix ""}} {
    set result {}
    foreach description [describe $profile $prefix] {lappend result {*}[dict get $description paths]}
    return $result
}
proc design_constraints::crossings {profile {prefix ""}} {
    set result [paths $profile $prefix]
    foreach path $result {
        set from [dict get $path from]
        set to [dict get $path to]
        set_max_delay [dict get $path max_delay] -ignore_clock_latency -from $from -to $to
        set_false_path -hold -from $from -to $to
    }
    return $result
}
proc design_constraints::synchronizer_chains {profile} {
    set result {}
    foreach description [describe $profile] {
        foreach chain [dict get $description chains] {
            lappend result [dict create provider [dict get $description provider] chain $chain]
        }
    }
    return $result
}
proc design_constraints::synchronizer_cells {binding} {
    return [rtl_ip::invoke [dict get $binding provider] synchronizer_cells [list [dict get $binding chain]]]
}
