# SDC inputs use ns/pF; Genus database/report values use ps/fF.
# Genus flattens interfaces and packed fields into port bits. Restrict expansion
# to this exact base, so a payload vector never also matches payload_valid.
proc genus_ports {name} {
    set name [string map {. _} $name]
    set result [get_ports $name*]
    foreach_in_collection port $result {
        regexp {^[^\[]+} [get_object_name $port] base
        if {$base ne $name} {set result [remove_from_collection $result $port]}
    }
    return $result
}
set_units -time 1ns -capacitance 1pF
source constraints-common.tcl
set covered 0
set report [open constraint-coverage.rpt w]
foreach spec [profile::ports $design_profile] {
    lassign $spec name width direction clock
    set ports [genus_ports $name]
    if {[sizeof_collection $ports] != $width} {error "PDMA_BOUNDARY_MISMATCH: $name width"}
    foreach_in_collection port $ports {
        set actual [get_db $port .direction]
        if {$actual ne [dict get {input in output out} $direction]} {error "PDMA_BOUNDARY_MISMATCH: $name direction"}
    }
    incr covered $width
    puts $report "$name bits=$width direction=$direction domain=$clock"
}
if {$covered != [sizeof_collection [get_ports *]]} {error "PDMA_BOUNDARY_MISMATCH: unclassified ports"}
design_constraints::clocks $design_profile
dict for {clock settings} [dict get $design_profile clocks] {
    # Assertion is asynchronous. Release must be locally synchronous at integration.
    set_false_path -fall_from [get_ports [dict get $settings reset]]
}
set_clock_uncertainty $clock_uncertainty [all_clocks]
set_clock_transition $transition [all_clocks]
set cdc_paths [design_constraints::mailboxes $design_profile]
foreach spec [profile::ports $design_profile] {
    lassign $spec name width direction clock
    if {$name eq $clock} {continue}
    set ports [genus_ports $name]
    if {$direction eq "input"} {
        set_input_delay -min $io_min -clock $clock $ports
        set_input_delay -max $io_max -clock $clock $ports
        set_input_transition -min $transition $ports
        set_input_transition -max $transition $ports
    } else {
        set_output_delay -min $io_min -clock $clock $ports
        set_output_delay -max $io_max -clock $clock $ports
        set_load $output_load $ports
    }
}
dict for {clock settings} [dict get $design_profile clocks] {
    if {[get_db [get_db clocks $clock] .period] != 1000*[dict get $settings period]} {error "PDMA_CLOCK_UNITS_MISMATCH"}
}
puts $report "classified_bits=$covered; SDC=ns/pF; database=ps/fF"
close $report
puts PDMA_CLOCK_TARGETS_PASS
