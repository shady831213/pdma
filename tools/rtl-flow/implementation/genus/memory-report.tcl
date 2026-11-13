# Actual library-pin loads and timing paths, independent of RTL hierarchy names.
proc genus_memory::clock_pins {} {
    variable specs
    set result {}
    dict for {role spec} $specs {
        foreach pin [get_db [bound $role] .pins] {
            if {[file tail [get_db $pin .name]] ne "clk"} {continue}
            if {[get_db [get_db $pin .clocks] .name] ne [dict get $spec clock]} {
                error "MEMORY_GUARD: provider clock mismatch"
            }
            lappend result $pin
        }
    }
    if {[llength $result] != [dict size $specs]} {error "MEMORY_GUARD: missing clock pins"}
    return $result
}
proc genus_memory::check_delays {} {
    variable specs
    dict for {role spec} $specs {
        set clock [get_db clocks [dict get $spec clock]]
        foreach pin [get_db [bound $role] .pins] {
            if {[file tail [get_db $pin .name]] eq "clk"} {continue}
            set input [expr {[get_db $pin .direction] eq "out"}]
            set seen {0 0 0 0}
            foreach delay [get_db $pin .external_delays] {
                if {[get_db $delay .clock] ne $clock || ([get_db $delay .input_delay] ? 1 : 0) != $input ||
                    ![get_db $delay .clock_rise]} {error "MEMORY_DELAY: wrong boundary clock or direction"}
                set values [get_db $delay .delay]
                if {[llength $values] != 4} {error "MEMORY_DELAY: malformed delay"}
                for {set i 0} {$i < 4} {incr i} {
                    set value [lindex $values $i]
                    if {$value eq "no_value"} {continue}
                    if {![string is double -strict $value] || abs($value)>1e-5} {error "MEMORY_DELAY: changed accounting origin"}
                    lset seen $i [expr {[lindex $seen $i]+1}]
                }
            }
            if {$seen ne {1 1 1 1}} {error "MEMORY_DELAY: missing or duplicate boundary delay"}
        }
    }
}
proc genus_memory::check_caps {boxes label} {
    set rows {}
    foreach box $boxes {
        foreach pin [get_db $box .pins] {
            if {[get_db $pin .direction] ne "in" || [file tail [get_db $pin .name]] eq "clk"} {continue}
            set libpin [get_db $pin .lib_pins]
            if {[llength $libpin] != 1} {error "MEMORY_CAP: missing library pin"}
            set lib [get_db [get_db $libpin .lib_cell] .library]
            set scale [get_db $lib .cap_scale_in_ff]
            foreach attr {capacitance_max_rise capacitance_max_fall capacitance_min_rise capacitance_min_fall} {
                set cap [expr {[get_db $libpin .$attr]*$scale}]
                if {![string is double -strict $cap] || abs($cap-1000*$::output_load)>1e-5} {
                    error "MEMORY_CAP: wrong library pin load"
                }
            }
            set rise [get_db $pin .capacitance_max_rise]; set fall [get_db $pin .capacitance_max_fall]
            foreach load [list $rise $fall] {
                if {![string is double -strict $load] || $load+1e-7<$cap} {error "MEMORY_CAP: missing instance load"}
            }
            lappend rows [list [get_db $pin .name] $cap $rise $fall]
        }
    }
    write_table memory-load-$label.tsv {pin library_pin_ff net_max_rise_ff net_max_fall_ff} $rows
}
proc genus_memory::report {} {
    variable specs
    set boxes [inventory]
    check_caps $boxes post
    check_delays
    set rows {}
    set static {}
    foreach box $boxes {
        foreach pin [get_db $box .pins] {
            set name [get_db $pin .name]
            if {[file tail $name] eq "clk"} {continue}
            set direction -to
            if {[get_db $pin .direction] eq "out"} {set direction -from}
            if {$direction eq "-to"} {
                set value [get_db [get_db $pin .net] .constant]
                if {$value in {0 1}} {lappend static [list $name $value]; continue}
            }
            set paths [report_timing $direction $pin -max_paths 1 -collection]
            if {[sizeof_collection $paths] != 1} {error "MEMORY_TIMING: missing boundary path"}
            set slack [get_db $paths .slack]
            if {![string is double -strict $slack] || !($slack > -Inf && $slack < Inf)} {
                error "MEMORY_TIMING: nonfinite boundary slack"
            }
            lappend rows [list $direction $name $slack]
        }
    }
    write_table memory-budgets.tsv {direction pin max_accounting_slack_ps} $rows
    write_table memory-static-pins.tsv {pin constant_value} $static
    set bits 0
    dict for {role spec} $specs {incr bits [expr {[dict get $spec depth]*[dict get $spec width]}]}
    set fd [open memory-qualification.txt w]
    try {
        puts $fd "mode=$::env(PDMA_MEMORY_MODE) opaque_providers=[llength $boxes] excluded_bits=$bits"
        puts $fd "Opaque provider area/power and real SRAM timing are unknown/excluded."
        puts $fd "Zero boundary delays are accounting origins, not SRAM tCO/setup/hold."
        puts $fd "Other storage remains synthesized standard-cell logic."
    } finally {close $fd}
    set cells {}
    foreach box $boxes {lappend cells {*}[get_db $box .lib_cells]}
    puts PDMA_MEMORY_BUDGET_REPORT_COMPLETE
    return [lsort -unique $cells]
}
