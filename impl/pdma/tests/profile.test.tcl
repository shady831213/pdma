set root [file normalize [file join [file dirname [info script]] ../../..]]
source [file join $root impl/pdma/flow/profile.tcl]
set profile [profile::read [file join $root impl/pdma/constraints/dma-copy.tcl]]
set directory [lindex $argv 0]
if {$directory eq "" || [file exists $directory]} {error "Supply a new evidence directory"}
file mkdir $directory
set variants [list \
    [dict replace $profile clocks {}] \
    [dict set profile clocks reg_clk period 0] \
]
# Build each mutation from the actual profile, preserving other dimensions.
set base [profile::read [file join $root impl/pdma/constraints/dma-copy.tcl]]
set changed $base
dict set changed ports core_clk input irq_ready 0
lappend variants $changed
set changed $base
dict set changed ports unknown input data 1
lappend variants $changed
set changed $base
dict set changed reset_groups cold {reg_rst_n nonexistent}
lappend variants $changed
foreach {field value} {source unknown destination reg_clk max_delay 0 max_delay 2} {
    set changed $base
    dict set changed mailboxes csr/crossing $field $value
    lappend variants $changed
}
set index 0
foreach variant $variants {
    set path [file join $directory invalid-$index.tcl]
    set fd [open $path w]
    puts $fd [list return $variant]
    close $fd
    if {![catch {profile::read $path} reason] || ![string match {PDMA_PROFILE:*} $reason]} {
        error "Invalid profile accepted: $index $reason"
    }
    incr index
}
puts "PDMA_PROFILE_TEST_PASS negatives=$index"
