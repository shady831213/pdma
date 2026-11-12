# Veryl package entry: definitions only, no design mutation or inherited waiver.
foreach file {objects.tcl duplex_mailbox.tcl async_queue.tcl} {
    source [file join [file dirname [info script]] $file]
}
proc common_cell_cdc::validate {spec} {
    switch -- [dict get $spec cell] {
        DuplexMailbox {}
        AsyncQueue {
            set depth [dict get $spec depth]
            if {![string is integer -strict $depth] || $depth < 4 || ($depth & ($depth-1))!=0} {
                error "CDC_QUEUE_DEPTH: require a power of two >=4"
            }
        }
        default {error "IP_RESOURCE: unknown common-cell constraint contract"}
    }
}
proc common_cell_cdc::describe {scope source destination spec} {
    validate $spec
    set budget [dict get $spec max_delay]
    switch -- [dict get $spec cell] {
        DuplexMailbox {
            set paths [duplex_paths $scope $source $destination $budget]
            set chains [duplex_chains $scope $source $destination]
        }
        AsyncQueue {
            set paths [async_queue_paths $scope $source $destination $budget]
            set chains [async_queue_chains $scope $source $destination [dict get $spec depth]]
        }
    }
    return [dict create paths $paths chains $chains]
}
return [dict create \
    validate [namespace current]::common_cell_cdc::validate \
    describe [namespace current]::common_cell_cdc::describe \
    synchronizer_cells [namespace current]::common_cell_cdc::synchronizer_cells]
