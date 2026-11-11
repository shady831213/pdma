source checks.tcl
source constraints-common.tcl
source cdc.tcl
proc run {} {
    read_db $::env(PDMA_MAPPED_DATABASE)
    set_units -time 1ns -capacitance 1pF
    source profile.tcl
    set profile [profile::read design-profile.tcl]
    # This database was constrained before mapping. Re-query, do not reapply.
    if {![cdc_status $profile]} {error "Expected constrained baseline to pass"}
    file rename cdc-timing-status.json baseline.json
    set source [get_clocks s_clk]
    set destination [get_clocks d_clk]
    set peer_from [common_cell_cdc::registers ip01/mailbox/requests $source cells]
    set peer_to [common_cell_cdc::registers ip01/mailbox/requests $destination cells]
    cdc_path_status $peer_from $peer_to 0.7 untouched-peer.rpt
    puts PDMA_CDC_SCOPE_ISOLATED
    if {![catch {common_cell_cdc::duplex_mailbox missing/mailbox $source $destination 0.5} reason] ||
        ![string match {CDC_BINDING_EMPTY:*} $reason]} {error "Missing instance was accepted"}
    puts PDMA_CDC_MISSING_SCOPE_REJECTED
    # Tighten the real mapped crossing, without changing RTL or the clock profile.
    dict set profile mailboxes ip0/mailbox max_delay 0.001
    design_constraints::mailboxes $profile
    if {[cdc_status $profile]} {error "Impossible crossing budget was accepted"}
    file rename cdc-timing-status.json negative-budget.json
    puts PDMA_CDC_NEGATIVE_BUDGET_REJECTED
    # Model a top-level integration false-path accidentally masking the IP budget.
    set path [lindex [design_constraints::mailbox_paths $profile] 0]
    set_false_path -from [dict get $path from] -to [dict get $path to]
    if {![catch {cdc_status $profile} reason] || ![string match {PDMA_CDC_UNTIMED:*} $reason]} {
        error "Masked crossing was accepted: $reason"
    }
    puts PDMA_CDC_MASKED_PATH_REJECTED
    check_errors qualification
    puts PDMA_CDC_INTEGRATION_TESTS_PASS
}
if {[catch {run} detail]} {puts stderr "PDMA_CDC_TEST_FAILED: $detail";exit 1}
exit
