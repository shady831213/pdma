# Reusable IP constraints. Source this file, then pass an instance path, actual
# clock collections and a maximum delay in the active SDC time unit.
# No clocks, global clock groups, IO delays or reset assumptions are created here.
namespace eval common_cell_cdc {}
proc common_cell_cdc::registers {scope clock kind} {
    if {[sizeof_collection $clock] != 1} {error "CDC_CLOCK: require one clock"}
    set all [all_registers -clock $clock -$kind]
    set selected ""
    foreach_in_collection object $all {
        # Literal hierarchy boundary: chip/dma0 must not select chip/dma01.
        if {[string first "${scope}/" [get_object_name $object]] == 0} {
            set selected [add_to_collection $selected $object]
        }
    }
    if {[sizeof_collection $selected] == 0} {error "CDC_BINDING_EMPTY: $scope $kind"}
    return $selected
}
proc common_cell_cdc::handshake_paths {scope source destination budget} {
    if {[get_object_name $source] eq [get_object_name $destination]} {
        error "CDC_CLOCK: handshake clocks must differ"
    }
    if {![string is double -strict $budget] || !($budget > 0 && $budget < Inf)} {
        error "CDC_BUDGET: require a finite positive maximum delay"
    }
    set paths {}
    foreach pair [list [list $source $destination payload] [list $destination $source ack]] {
        lassign $pair launch capture kind
        set from [registers $scope $launch cells]
        set to [registers $scope $capture cells]
        lappend paths [dict create scope $scope kind $kind source $launch destination $capture \
            from $from to $to max_delay $budget]
    }
    return $paths
}
proc common_cell_cdc::duplex_paths {scope source destination budget} {
    set prefix [expr {$scope eq "" ? "" : "[string trimright $scope /]/"}]
    return [concat [handshake_paths ${prefix}requests $source $destination $budget] \
        [handshake_paths ${prefix}responses $destination $source $budget]]
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
proc common_cell_cdc::duplex_mailbox {scope source destination budget} {
    set paths [duplex_paths $scope $source $destination $budget]
    apply_paths $paths
    return $paths
}
proc common_cell_cdc::duplex_chains {scope source destination} {
    set result {}
    set prefix [expr {$scope eq "" ? "" : "[string trimright $scope /]/"}]
    foreach {direction launch capture} [list requests $source $destination responses $destination $source] {
        foreach {name clock} [list u_s_to_d $capture u_d_to_s $launch] {
            lappend result [list $prefix$direction/$name $clock 2]
        }
    }
    return $result
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
proc common_cell_cdc::synchronizers {scope source destination} {
    return [synchronizer_cells [duplex_chains $scope $source $destination]]
}
