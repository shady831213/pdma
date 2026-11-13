package require tcltest
namespace import ::tcltest::*
set root [file normalize [file join [file dirname [info script]] ../../../..]]
source [file join $root tools/rtl-flow/implementation/memory-model.tcl]
source [file join $root tools/rtl-flow/implementation/genus/memory.tcl]
source [file join $root tools/rtl-flow/implementation/genus/memory-report.tcl]

# Independent small fixtures exercise the library interface, not a DMA layout.
set memories {
    first {instance block/a provider tech_cell_generic_Ram1R1W kind Ram1R1W depth 8 width 13 clock clk}
    second {instance block/b provider tech_cell_generic_Ram1R1W kind Ram1R1W depth 8 width 13 clock clk}
    shared {instance block/c provider tech_cell_generic_Ram1RW kind Ram1RW depth 16 width 5 clock clk}
}
proc render {specs cap} {
    set path [makeFile {} boundary.lib]
    memory_model::write_library $path $specs $cap
    set fd [open $path r]; set text [read $fd]; close $fd
    removeFile boundary.lib
    return $text
}
test role-identity {Equal shapes retain separate owner references} -body {
    set text [render $memories 0.023]
    list [regexp -all {cell \(} $text] \
        [string match *MemoryBoundary_first* $text] [string match *MemoryBoundary_second* $text]
} -result {3 1 1}
test pins {Read/write masks and addresses retain exact shape and direction} -body {
    set text [render [dict filter $memories key first] 0.023]
    list [regexp {bus \("write_port_mask"\) \{ bus_type : bits_13; direction : input;} $text] \
        [regexp {bus \("read_port_address"\) \{ bus_type : bits_3; direction : input;} $text] \
        [regexp {bus \("read_port_data"\) \{ bus_type : bits_13; direction : output;} $text]
} -result {1 1 1}
test accounting {No fabricated function/timing or physical SRAM area} -body {
    set text [render $memories 0.023]
    list [regexp {capacitance : 0\.023;} $text] [regexp {timing\s*\(|function\s*:} $text] \
        [regexp -all {area : 0; dont_use : true; is_macro_cell : true;} $text]
} -result {1 0 3}
test empty {Empty inventory has no blackbox cell} -body {
    regexp -all {cell \(} [render {} 0.01]
} -result 0
test non-power-two {Address width is derived from full depth} -body {
    memory_model::address_width 9
} -result 4
test depth-one {Single row still has a one-bit address port} -body {
    memory_model::address_width 1
} -result 1
test one-bit-vector {A one-bit address/data vector is distinct from a scalar enable} -body {
    set one [dict get $memories first]
    dict set one width 1
    dict set one depth 1
    set text [render [dict create one $one] 0.023]
    list [regexp {bus \("read_port_data"\) \{ bus_type : bits_1;} $text] \
        [regexp {pin \("read_port_address\[0\]"\)} $text] \
        [regexp {pin \("read_port_enable"\)} $text]
} -result {1 1 1}
foreach invalid {-1 Inf NaN} {
    test load-$invalid {Invalid electrical model rejected} -body {
        render $memories $invalid
    } -returnCodes error -match glob -result {MEMORY_MODEL:*}
}
test unsupported {Unsupported port contract fails explicitly} -body {
    memory_model::signals Unknown 13 3
} -returnCodes error -match glob -result {MEMORY_MODEL:*}
test failed-query-resources {A failed native query must not retain report handles} -body {
    set directory [makeDirectory report-failure]
    set cwd [pwd]; cd $directory
    proc get_db {args} {error "injected native query failure"}
    set before [lsort [chan names]]
    catch {genus_memory::check_caps {missing} failed} reason
    set after [lsort [chan names]]
    # Cleanup remains mandatory when the assertion fails on the old behavior.
    foreach channel $after {if {$channel ni $before} {close $channel}}
    rename get_db {}
    cd $cwd
    removeDirectory report-failure
    list [expr {$reason eq "injected native query failure"}] [expr {$before eq $after}]
} -result {1 1}
set failures $::tcltest::numTests(Failed)
set skipped $::tcltest::numTests(Skipped)
set total $::tcltest::numTests(Total)
cleanupTests
if {$failures || $skipped || $total != 12} {exit 1}
