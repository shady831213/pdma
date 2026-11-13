# Capacitance-only RAM boundaries, adapted from pcie_modeling's public flow.
# No SRAM function, timing arcs, physical area or power is characterized here.
namespace eval memory_model {}
proc memory_model::address_width {depth} {
    if {![string is integer -strict $depth] || $depth < 1} {error "MEMORY_MODEL: invalid depth"}
    set width 1
    for {set n 2} {$n < $depth} {set n [expr {$n*2}]} {incr width}
    return $width
}
proc memory_model::signals {kind width address} {
    # Name, width, direction, shape. A one-bit address/data bus still has a
    # [0] pin; it must not collapse into the scalar enable-pin convention.
    switch -- $kind {
        Ram1RW {
            return [list port_enable 1 input scalar port_write_enable 1 input scalar port_address $address input vector \
                port_write_data $width input vector port_write_mask $width input vector port_read_data $width output vector]
        }
        Ram1R1W {
            return [list read_port_enable 1 input scalar read_port_address $address input vector read_port_data $width output vector \
                write_port_enable 1 input scalar write_port_address $address input vector write_port_data $width input vector write_port_mask $width input vector]
        }
        default {error "MEMORY_MODEL: unsupported RAM port contract"}
    }
}
proc memory_model::reference {role spec} {
    if {![regexp {^[A-Za-z_][A-Za-z0-9_]*$} $role]} {error "MEMORY_MODEL: invalid role name"}
    return MemoryBoundary_${role}_[dict get $spec kind]_D[dict get $spec depth]_W[dict get $spec width]
}
proc memory_model::write_library {path memories cap} {
    if {![string is double -strict $cap] || !($cap >= 0 && $cap < Inf)} {
        error "MEMORY_MODEL: invalid pin capacitance"
    }
    set widths {}
    dict for {role spec} $memories {
        reference $role $spec
        set address [address_width [dict get $spec depth]]
        foreach {name count direction shape} [signals [dict get $spec kind] [dict get $spec width] $address] {
            if {$shape eq "vector"} {lappend widths $count}
        }
    }
    set fd [open $path w]
    try {
        puts $fd "library (rtl_memory_budget) \{"
        puts $fd {delay_model : table_lookup;
time_unit : "1ns";
voltage_unit : "1V";
current_unit : "1mA";
pulling_resistance_unit : "1kohm";
leakage_power_unit : "1nW";
capacitive_load_unit (1, pf);
bus_naming_style : "%s[%d]";
nom_process : 1;
nom_voltage : 1;
nom_temperature : 25;
default_fanout_load : 1;
default_input_pin_cap : 0;
default_inout_pin_cap : 0;
default_output_pin_cap : 0;
operating_conditions (accounting) { process : 1; voltage : 1; temperature : 25; }
default_operating_conditions : accounting;
slew_lower_threshold_pct_rise : 20;
slew_upper_threshold_pct_rise : 80;
slew_lower_threshold_pct_fall : 20;
slew_upper_threshold_pct_fall : 80;
input_threshold_pct_rise : 50;
input_threshold_pct_fall : 50;
output_threshold_pct_rise : 50;
output_threshold_pct_fall : 50;
slew_derate_from_library : 1;}
        foreach width [lsort -integer -unique $widths] {
            puts $fd "type (bits_$width) \{ base_type : array; data_type : bit; bit_width : $width; bit_from : [expr {$width-1}]; bit_to : 0; downto : true; \}"
        }
        dict for {role spec} $memories {
            puts $fd "cell (\"[reference $role $spec]\") \{ area : 0; dont_use : true; is_macro_cell : true;"
            puts $fd {pin (clk) { direction : input; clock : true; capacitance : 0; }}
            foreach {name count direction shape} [signals [dict get $spec kind] [dict get $spec width] [address_width [dict get $spec depth]]] {
                set attributes "direction : $direction;"
                if {$direction eq "input"} {append attributes " capacitance : $cap;"}
                if {$shape eq "scalar"} {
                    puts $fd "pin (\"$name\") \{ $attributes \}"
                } else {
                    puts $fd "bus (\"$name\") \{ bus_type : bits_$count; $attributes"
                    for {set i 0} {$i < $count} {incr i} {puts $fd "pin (\"$name\[$i\]\") \{ $attributes \}"}
                    puts $fd "\}"
                }
            }
            puts $fd "\}"
        }
        puts $fd "\}"
    } finally {close $fd}
}
