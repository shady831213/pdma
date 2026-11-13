set root [file normalize [file join [file dirname [info script]] ../../..]]
source [file join $root tools/rtl-flow/ip/runtime.tcl]
set snapshot [lindex $argv 1]
if {$snapshot eq ""} {error "Supply evidence directory and resource snapshot"}
rtl_ip::load [file join $snapshot ip-manifest.tcl]
source [file join $root tools/rtl-flow/implementation/profile.tcl]
set profile [profile::read [file join $root impl/dma/profiles/copy.tcl]]
set directory [lindex $argv 0]
if {$directory eq "" || [file exists $directory]} {error "Supply a new evidence directory"}
file mkdir $directory
set variants [list \
    [dict replace $profile clocks {}] \
    [dict set profile clocks reg_clk period 0] \
]
# Build each mutation from the actual profile, preserving other dimensions.
set base [profile::read [file join $root impl/dma/profiles/copy.tcl]]
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
    dict set changed crossings csr/crossing $field $value
    lappend variants $changed
}
foreach {name value} {CHANNEL_COUNT nope bad-name 2 CHANNEL_COUNT {2;exit}} {
    set changed $base
    dict set changed parameters $name $value
    lappend variants $changed
}
set index 0
foreach {field value} {depth 3 depth 0 max_delay 0 max_delay 2 destination unknown} {
    set changed $base
    dict set changed crossings request_crossing $field $value
    lappend variants $changed
}
foreach {field value} {provider absent cell UnknownCell} {
    set changed $base
    dict set changed crossings request_crossing $field $value
    lappend variants $changed
}
foreach {field value} {depth 0 width -1 clock unknown kind RamAsync provider bad-name instance */ram} {
    set changed $base
    dict set changed memories copy_buffer $field $value
    lappend variants $changed
}
set changed $base
dict set changed memories duplicate [dict get $base memories copy_buffer]
lappend variants $changed
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

# Every supported count changes real storage/ports and the elaboration input.
for {set count 1} {$count <= 8} {incr count} {
    set ::env(PDMA_CHANNEL_COUNT) $count
    set current [profile::read [file join $root impl/dma/profiles/copy.tcl]]
    if {[dict get $current parameters CHANNEL_COUNT] != $count ||
        [dict get $current memories copy_buffer depth] != 8*$count ||
        [dict get $current ports core_clk input irq_ready] != $count ||
        [dict get $current ports core_clk output irq] != $count} {error "Incorrect channel profile $count"}
}
foreach invalid {0 9 -1 2.0 02 nope} {
    set ::env(PDMA_CHANNEL_COUNT) $invalid
    if {![catch {profile::read [file join $root impl/dma/profiles/copy.tcl]} reason] ||
        ![string match {PDMA_PROFILE:*} $reason]} {error "Invalid channel count accepted"}
}
unset ::env(PDMA_CHANNEL_COUNT)
puts "PDMA_CHANNEL_PROFILES_PASS counts=1..8 invalid=6"
