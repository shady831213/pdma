# Reusable IP constraints. Source ip.tcl, then pass an instance path, actual
# clock collections and a maximum delay in the active SDC time unit.
# No clocks, global clock groups, IO delays or reset assumptions are created here.
namespace eval common_cell_cdc {}
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
        set path [dict create scope $scope kind $kind source $launch destination $capture \
            from $from to $to max_delay $budget]
        if {$kind eq "payload"} {dict set path payload [payload_registers $from $scope/latched_data]}
        lappend paths $path
    }
    return $paths
}
proc common_cell_cdc::duplex_paths {scope source destination budget} {
    set prefix [expr {$scope eq "" ? "" : "[string trimright $scope /]/"}]
    return [concat [handshake_paths ${prefix}requests $source $destination $budget] \
        [handshake_paths ${prefix}responses $destination $source $budget]]
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
proc common_cell_cdc::synchronizers {scope source destination} {
    return [synchronizer_cells [duplex_chains $scope $source $destination]]
}
