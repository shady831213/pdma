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
    set groups {}
    set index 0
    foreach entry [dict get $policy findings] {
        if {$check ni [dict get $entry checks]} {continue}
        set tag [dict get $entry tag]
        set expected_count 1
        if {[dict exists $entry expected_count]} {
            set expected_count [dict get $entry expected_count]
            # Only proven dead sinks may group multiple reset-source paths. A
            # register, output or ordinary CDC waiver must remain one-to-one.
            set fields [dict get $entry fields]
            if {![string is integer -strict $expected_count] || $expected_count<1 ||
                $check ne "rdc" || $tag ne "RDC_CORRUPT_OBSERVED" ||
                [dict get $fields DestObjectType] ne "unconnected" ||
                [dict get $fields DestClockInfoList:DestClockInfo:ClockName] ne "'no-clock'" ||
                ![dict exists $fields DestObject]} {
                error "PDMA_REVIEW_GROUP: only an exact unconnected sink may group source paths"
            }
        }
        set matches {}
        foreach id [report_$check -severity {fatal error warning} -tag $tag -id_list] {
            set match 1
            dict for {field expected} [dict get $entry fields] {
                set actual [get_violation_field -quiet -all $id $field $check]
                if {[lsort $actual] ne [lsort $expected]} {set match 0; break}
            }
            if {$match} {lappend matches $id}
        }
        if {[llength $matches] != $expected_count} {
            error "PDMA_REVIEW_MISMATCH: entry $index requires $expected_count distinct findings"
        }
        foreach id $matches {
            if {$id in $selected} {error "PDMA_REVIEW_MISMATCH: overlapping review entries"}
        }
        lappend groups $matches
        lappend selected {*}$matches
        incr index
    }
    if {![llength $selected]} {error "PDMA_REVIEW_EMPTY: no reviewed findings"}
    # Validate every entry before applying any exception. IDs come from this run,
    # after field matching; no unstable native report ID is stored in the policy.
    set index 0
    foreach entry [dict get $policy findings] {
        if {$check ni [dict get $entry checks]} {continue}
        set subindex 0
        foreach id [lindex $groups $index] {
            waive_$check -add pdma_review_${index}_$subindex -id $id -comment [dict get $entry reason]
            incr subindex
        }
        incr index
    }
    report_$check -only_waived -verbose -limit 0 -file reviewed.rpt
    puts "PDMA_STATIC_REVIEW_APPLIED check=$check findings=[llength $selected]"
    return [llength $selected]
}
