# Adapted from pcie_modeling's Genus policy. Approvals are exact library names.
namespace eval library_policy {}
proc library_policy::require_non_ulvt {identity} {
    if {[string match -nocase *ulvt* $identity]} {error "PDMA_LIBRARY_POLICY: ULVT forbidden"}
}
proc library_policy::attribute {object key} {
    set attributes [get_db $object .liberty_attributes]
    if {[dict exists $attributes $key]} {return [dict get $attributes $key]}
    return {}
}
proc library_policy::identity {cell} {
    set lib [get_db $cell .library]
    return [list [get_db $lib .base_name] [get_db $cell .name] \
        [attribute $lib default_threshold_voltage_group] [attribute $cell threshold_voltage_group]]
}
proc library_policy::prepare {approved} {
    if {![llength $approved]} {error "PDMA_LIBRARY_POLICY: no approved libraries"}
    foreach name $approved {
        require_non_ulvt $name
        set matches [lsort -unique [get_db libraries -if [list .base_name == $name]]]
        if {[llength $matches] != 1} {error "PDMA_LIBRARY_POLICY: library missing or ambiguous"}
        require_non_ulvt [attribute $matches default_threshold_voltage_group]
    }
    set blocked 0
    foreach cell [get_db lib_cells] {
        lassign [identity $cell] library name default_vt cell_vt
        if {$library ni $approved || [string match -nocase *ulvt* "$name $default_vt $cell_vt"]} {
            set_db $cell .dont_use true
            if {![get_db $cell .dont_use]} {error "PDMA_LIBRARY_POLICY: exclusion failed"}
            incr blocked
        }
    }
    puts "PDMA_LIBRARY_POLICY_PREPARED blocked=$blocked"
}
proc library_policy::audit {approved} {
    set instances [get_db insts]
    if {![llength $instances]} {error "PDMA_LIBRARY_POLICY: no mapped instances"}
    set rows {}
    foreach inst $instances {
        set cells [lsort -unique [get_db $inst .lib_cells]]
        if {[llength $cells] != 1} {error "PDMA_LIBRARY_POLICY: unresolved mapped reference"}
        set identity [identity $cells]
        require_non_ulvt $identity
        if {[lindex $identity 0] ni $approved} {error "PDMA_LIBRARY_POLICY: unapproved mapped reference"}
        dict set rows $identity 1
    }
    set fd [open library-policy.tsv w]
    puts $fd "library\tcell\tdefault_vt\tcell_vt"
    foreach identity [lsort [dict keys $rows]] {puts $fd [join $identity \t]}
    close $fd
    puts "PDMA_LIBRARY_POLICY_PASS references=[dict size $rows] instances=[llength $instances]"
}
