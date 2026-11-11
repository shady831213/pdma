proc rejected {script pattern} {
    if {![catch {uplevel 1 $script} reason] || ![string match $pattern $reason]} {
        error "Expected $pattern, received $reason"
    }
    puts "PDMA_POLICY_NEGATIVE_PASS $reason"
}
proc run {} {
    source library-policy.tcl
    read_libs {policy-cells.lib policy-voltage.lib}
    library_policy::prepare {policy_cells}
    foreach path {*/policy_cells/BUF_ULVT */policy_cells/BUF_METADATA */policy_voltage/VBUF} {
        set cell [get_db lib_cells $path]
        if {[llength $cell] != 1 || ![get_db $cell .dont_use]} {error "Forbidden cell not disabled"}
    }
    set design [create_design -name policy_test]
    set good [get_db lib_cells */policy_cells/GOOD]
    create_inst -name good -base_cell [get_db $good .base_cell] -parent $design
    library_policy::audit {policy_cells}
    rejected {library_policy::audit {unknown}} {PDMA_LIBRARY_POLICY: unapproved mapped reference}
    foreach path {*/policy_cells/BUF_ULVT */policy_cells/BUF_METADATA */policy_voltage/VBUF} {
        set cell [get_db lib_cells $path]
        set bad [create_inst -name forbidden -base_cell [get_db $cell .base_cell] -parent $design]
        rejected {library_policy::audit {policy_cells}} {PDMA_LIBRARY_POLICY: ULVT forbidden}
        delete_obj $bad
    }
    rejected {library_policy::prepare {policy_voltage}} {PDMA_LIBRARY_POLICY: ULVT forbidden}
    rejected {library_policy::prepare {missing}} {PDMA_LIBRARY_POLICY: library missing or ambiguous}
    rejected {library_policy::prepare {}} {PDMA_LIBRARY_POLICY: no approved libraries}
    library_policy::audit {policy_cells}
    delete_obj [get_db insts]
    rejected {library_policy::audit {policy_cells}} {PDMA_LIBRARY_POLICY: no mapped instances}
    set errors [get_db messages -if {.severity == Error && .count > 0}]
    if {[llength $errors]} {error "Unexpected tool errors: [get_db $errors .name]"}
    puts PDMA_GENUS_POLICY_TEST_PASS
}
if {[catch {run} result]} {
    puts stderr "PDMA_GENUS_POLICY_TEST_FAILED: $result"
    exit 1
}
exit 0
