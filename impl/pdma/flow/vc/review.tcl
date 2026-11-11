# Apply reviewed exceptions only to the exact source/configuration identity and
# exact native finding fields. Never infer approval from a tag or finding count.
namespace eval static_review {}
proc static_review::apply {check} {
    source review-inputs.tcl
    set policy [source static-review.tcl]
    if {[dict get $policy identity] ne $review_identity} {
        error "PDMA_REVIEW_STALE: source, tool or constraints changed; review again"
    }
    set selected {}
    set index 0
    foreach entry [dict get $policy findings] {
        if {$check ni [dict get $entry checks]} {continue}
        set tag [dict get $entry tag]
        set matches {}
        foreach id [report_$check -severity {fatal error warning} -tag $tag -id_list] {
            set match 1
            dict for {field expected} [dict get $entry fields] {
                set actual [get_violation_field -quiet -all $id $field $check]
                if {[lsort $actual] ne [lsort $expected]} {set match 0; break}
            }
            if {$match} {lappend matches $id}
        }
        if {[llength $matches] != 1 || [lindex $matches 0] in $selected} {
            error "PDMA_REVIEW_MISMATCH: entry $index must match exactly one distinct finding"
        }
        lappend selected {*}$matches
        incr index
    }
    if {![llength $selected]} {error "PDMA_REVIEW_EMPTY: no reviewed findings"}
    # Validate every entry before applying any exception. IDs come from this run,
    # after field matching; no unstable native report ID is stored in the policy.
    set index 0
    foreach entry [dict get $policy findings] {
        if {$check ni [dict get $entry checks]} {continue}
        waive_$check -add pdma_review_$index -id [lindex $selected $index] \
            -comment [dict get $entry reason]
        incr index
    }
    report_$check -only_waived -verbose -limit 0 -file reviewed.rpt
    puts "PDMA_STATIC_REVIEW_APPLIED check=$check findings=[llength $selected]"
    return [llength $selected]
}
