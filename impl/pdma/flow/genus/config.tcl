source technology.tcl
foreach name {target_libraries operating_library operating_condition time_unit capacitance_unit clock_uncertainty transition io_min io_max output_load} {
    if {![info exists $name] || [string trim [set $name]] eq ""} {error "Missing technology parameter: $name"}
}
if {$time_unit != 1e-9 || $capacitance_unit != 1e-12} {
    error "PDMA_GENUS_UNITS: technology budgets must use ns/pF"
}
foreach name {clock_uncertainty transition io_min io_max output_load} {
    if {![string is double -strict [set $name]] || !([set $name] >= 0 && [set $name] < Inf)} {
        error "Invalid technology parameter: $name"
    }
}
if {$io_min > $io_max} {error "I/O minimum exceeds maximum"}
if {![info exists approved_standard_cell_libraries]} {
    set approved_standard_cell_libraries [list $operating_library]
}
if {$operating_library ni $approved_standard_cell_libraries} {error "Operating library not approved"}
foreach library $target_libraries {
    if {![file readable $library] || [file size $library] == 0} {error "Unreadable technology library"}
    library_policy::require_non_ulvt [file tail $library]
}
