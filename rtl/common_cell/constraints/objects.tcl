# Physical object selection shared by common-cell constraint providers.
namespace eval common_cell_cdc {variable source_names {}}
# A tool adapter may supply native RTL-origin names after hierarchy optimization.
# Standalone SDC consumers keep ordinary object-name lookup by default.
proc common_cell_cdc::set_source_names {resolver} {
    variable source_names
    set source_names $resolver
}
proc common_cell_cdc::matches {object prefix} {
    variable source_names
    set names [list [get_object_name $object]]
    if {[llength $source_names]} {lappend names {*}[{*}$source_names $object]}
    foreach name $names {if {[string first $prefix $name] == 0} {return 1}}
    return 0
}
proc common_cell_cdc::payload_registers {registers prefix} {
    set result ""
    foreach_in_collection cell $registers {
        if {[matches $cell $prefix]} {
            set result [add_to_collection $result $cell]
        }
    }
    if {[sizeof_collection $result] == 0} {error "CDC_PAYLOAD_BINDING: $prefix"}
    return $result
}
proc common_cell_cdc::registers {scope clock kind} {
    if {[sizeof_collection $clock] != 1} {error "CDC_CLOCK: require one clock"}
    set all [all_registers -clock $clock -$kind]
    set selected ""
    foreach_in_collection object $all {
        # Literal hierarchy boundary: chip/block0 must not select chip/block01.
        if {[matches $object "${scope}/"]} {
            set selected [add_to_collection $selected $object]
        }
    }
    if {[sizeof_collection $selected] == 0} {error "CDC_BINDING_EMPTY: $scope $kind"}
    return $selected
}
proc common_cell_cdc::apply_paths {paths} {
    foreach path $paths {
        # Bound only this cell's cross-domain register paths, including held payload
        # and request/ACK controls. Same-clock synchronizer stages stay timed.
        set from [dict get $path from]
        set to [dict get $path to]
        set_max_delay [dict get $path max_delay] -ignore_clock_latency -from $from -to $to
        set_false_path -hold -from $from -to $to
    }
}
proc common_cell_cdc::synchronizer_cells {chains} {
    set result ""
    foreach chain $chains {
        lassign $chain scope clock expected
        set cells [registers $scope $clock cells]
        if {[sizeof_collection $cells]!=$expected} {error "CDC_SYNC_SHAPE: $scope"}
        set result [add_to_collection $result $cells]
    }
    return $result
}
