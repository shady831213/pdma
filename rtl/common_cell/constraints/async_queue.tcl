# AsyncQueue uses registered Gray pointers and stable FIFO rows. Bound both pointer
# directions and the stored-data crossing within the faster clock's period. This
# also bounds Gray bus skew; no asynchronous setup/hold promise is inferred.
# Source ip.tcl for this package's complete constraint API.
proc common_cell_cdc::async_queue_paths {scope source destination budget} {
    if {[get_object_name $source] eq [get_object_name $destination]} {
        error "CDC_CLOCK: FIFO clocks must differ"
    }
    if {![string is double -strict $budget] || !($budget > 0 && $budget < Inf)} {
        error "CDC_BUDGET: require a finite positive maximum delay"
    }
    set scope [string trimright $scope /]/queue
    set paths {}
    foreach pair [list [list $source $destination fifo-data-and-pointer] [list $destination $source fifo-credit]] {
        lassign $pair launch capture kind
        set path [dict create scope $scope kind $kind source $launch destination $capture \
            from [registers $scope $launch cells] to [registers $scope $capture cells] max_delay $budget]
        if {$kind eq "fifo-data-and-pointer"} {
            dict set path payload [payload_registers [dict get $path from] $scope/ram]
        }
        lappend paths $path
    }
    return $paths
}
proc common_cell_cdc::async_queue {scope source destination budget} {
    set paths [async_queue_paths $scope $source $destination $budget]
    apply_paths $paths
    return $paths
}
proc common_cell_cdc::async_queue_chains {scope source destination depth} {
    if {![string is integer -strict $depth] || $depth<4 || ($depth & ($depth-1))!=0} {
        error "CDC_QUEUE_DEPTH: require a power of two >=4"
    }
    set width 1
    for {set n $depth} {$n>1} {set n [expr {$n/2}]} {incr width}
    set result {}
    foreach {name clock} [list u_synchronizer_wp $destination u_synchronizer_rp $source] {
        lappend result [list [string trimright $scope /]/queue/$name $clock [expr {2*$width}]]
    }
    return $result
}
