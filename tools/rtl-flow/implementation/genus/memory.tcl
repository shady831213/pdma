# Bind explicitly inventoried native RAM providers to opaque accounting leaves.
# Adapted from pcie_modeling; no DMA hierarchy or clock is owned by this adapter.
namespace eval genus_memory {variable specs {}}
proc genus_memory::write_table {path header rows} {
    set fd [open $path w]
    try {
        puts $fd [join $header \t]
        foreach row $rows {puts $fd [join $row \t]}
    } finally {close $fd}
}
proc genus_memory::configure {profile mode} {
    variable specs
    if {$mode ni {generic control-budget}} {error "MEMORY_GUARD: unknown mode"}
    set specs [expr {$mode eq "generic" ? {} : [profile::memories $profile]}]
}
proc genus_memory::library {} {
    variable specs
    if {![dict size $specs]} {return {}}
    memory_model::write_library memory-budget.lib $specs $::output_load
    return [list [file normalize memory-budget.lib]]
}
proc genus_memory::blackbox {} {
    variable specs
    if {![dict size $specs]} {set_db hdl_error_on_blackbox true; return}
    set providers {}
    dict for {role spec} $specs {lappend providers [dict get $spec provider]}
    foreach provider [lsort -unique $providers] {
        set arch [vfind / -hdl_architecture $provider]
        if {[llength $arch] != 1} {error "MEMORY_GUARD: missing native RAM provider"}
        set_db $arch .blackbox true
    }
}
proc genus_memory::pin_contract {spec} {
    set result [dict create clk in]
    foreach {name count direction shape} [memory_model::signals [dict get $spec kind] [dict get $spec width] \
        [memory_model::address_width [dict get $spec depth]]] {
        for {set i 0} {$i < $count} {incr i} {
            set bit $name
            if {$shape eq "vector"} {append bit \[$i\]}
            dict set result $bit [dict get {input in output out} $direction]
        }
    }
    return $result
}
proc genus_memory::check_pins {spec pins} {
    set expected [pin_contract $spec]
    if {[llength $pins] != [dict size $expected]} {error "MEMORY_GUARD: pin count"}
    foreach pin $pins {
        set name [file tail [get_db $pin .name]]
        if {![dict exists $expected $name] || [get_db $pin .direction] ne [dict get $expected $name]} {
            error "MEMORY_GUARD: pin contract mismatch"
        }
        dict unset expected $name
    }
    if {[dict size $expected]} {error "MEMORY_GUARD: missing pins"}
}
proc genus_memory::bind {} {
    variable specs
    dict for {role spec} $specs {
        set path [dict get $spec instance]
        set h [get_db hinsts $path]
        if {[llength $h] != 1 || [get_db $h .name] ne $path} {error "MEMORY_GUARD: missing exact instance $role"}
        set name [get_db [get_db $h .module] .base_name]
        set expected "[dict get $spec provider]_DEPTH[dict get $spec depth]_WIDTH[dict get $spec width]_*"
        if {![string match $expected $name]} {error "MEMORY_GUARD: unexpected geometry $role"}
        if {[llength [get_db insts $path/*]] || [llength [get_db hinsts $path/*]]} {
            error "MEMORY_GUARD: provider body survived $role"
        }
        check_pins $spec [get_db $h .hpins]
        set lib [get_db lib_cells */rtl_memory_budget/[memory_model::reference $role $spec]]
        if {[llength $lib] != 1} {error "MEMORY_GUARD: missing boundary library cell"}
        change_link -instances $h -lib_cell $lib
    }
}
proc genus_memory::bound {role} {
    variable specs
    set matches {}
    foreach box [get_db insts -if {.is_black_box == true}] {
        set lib [lsort -unique [get_db $box .lib_cells]]
        if {[llength $lib] == 1 && [get_db $lib .base_name] eq [memory_model::reference $role [dict get $specs $role]] &&
            [get_db [get_db $lib .library] .base_name] eq "rtl_memory_budget"} {
            lappend matches $box
        }
    }
    if {[llength $matches] != 1} {error "MEMORY_GUARD: missing or ambiguous bound role $role"}
    return [lindex $matches 0]
}
proc genus_memory::inventory {} {
    variable specs
    if {[llength [get_db hinsts -if {.unresolved == true}]]} {error "MEMORY_GUARD: unresolved hierarchy"}
    if {[llength [get_db insts -if {.is_black_box == true}]] != [dict size $specs]} {
        error "MEMORY_GUARD: unexpected blackbox count"
    }
    set rows {}
    set boxes {}
    dict for {role spec} $specs {
        set box [bound $role]
        set pins [get_db $box .pins]
        check_pins $spec $pins
        foreach pin $pins {
            if {[file tail [get_db $pin .name]] eq "clk" &&
                [get_db [get_db $pin .clocks] .name] ne [dict get $spec clock]} {
                error "MEMORY_GUARD: provider clock mismatch"
            }
        }
        set depth [dict get $spec depth]; set width [dict get $spec width]
        lappend rows [list $role [get_db $box .name] [memory_model::reference $role $spec] $depth $width \
            [expr {$depth*$width}] [dict get $spec clock] [dict get $spec instance]]
        lappend boxes $box
    }
    write_table memory-inventory.tsv {role instance reference depth width bits clock rtl_anchor} $rows
    puts "PDMA_MEMORY_INVENTORY_PASS count=[llength $boxes]"
    return $boxes
}
proc genus_memory::prepare {} {
    variable specs
    set boxes [inventory]
    dict for {role spec} $specs {
        set box [bound $role]
        set_db $box .preserve true
        foreach pin [get_db $box .pins] {
            if {[file tail [get_db $pin .name]] eq "clk"} {continue}
            set command set_output_delay
            if {[get_db $pin .direction] eq "out"} {set command set_input_delay}
            $command -clock [dict get $spec clock] -max 0 $pin
            $command -clock [dict get $spec clock] -min 0 $pin
        }
    }
    check_caps $boxes pre
}
