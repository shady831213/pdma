# Tool-independent validation and expansion; tool adapters resolve real ports.
namespace eval profile {}
proc profile::read {path} {
    set result [source $path]
    dict get $result top
    set clocks [dict get $result clocks]
    if {![dict size $clocks]} {error "PDMA_PROFILE: no clocks"}
    dict for {clock settings} $clocks {
        set period [dict get $settings period]
        dict get $settings reset
        if {![string is double -strict $period] || !($period > 0 && $period < Inf)} {
            error "PDMA_PROFILE: invalid clock period"
        }
    }
    set seen {}
    foreach item [ports $result] {
        lassign $item name width direction clock
        if {[dict exists $seen $name]} {error "PDMA_PROFILE: duplicate port $name"}
        if {![dict exists $clocks $clock]} {error "PDMA_PROFILE: unknown clock $clock"}
        if {$direction ni {input output}} {error "PDMA_PROFILE: invalid direction"}
        if {![string is integer -strict $width] || $width < 1} {error "PDMA_PROFILE: invalid width"}
        dict set seen $name 1
    }
    if {[dict exists $result reset_groups]} {
        set grouped {}
        set resets {}
        dict for {clock settings} $clocks {lappend resets [dict get $settings reset]}
        dict for {group members} [dict get $result reset_groups] {
            if {[llength $members] < 2} {error "PDMA_PROFILE: reset group needs multiple resets"}
            foreach reset $members {
                if {$reset ni $resets || $reset in $grouped} {error "PDMA_PROFILE: invalid reset group member"}
                lappend grouped $reset
            }
        }
    }
    foreach family {mailboxes async_queues} {
        if {![dict exists $result $family]} {continue}
        dict for {instance spec} [dict get $result $family] {
            set source [dict get $spec source]
            set destination [dict get $spec destination]
            set budget [dict get $spec max_delay]
            if {$instance eq "" || ![dict exists $clocks $source] || ![dict exists $clocks $destination] ||
                $source eq $destination} {error "PDMA_PROFILE: invalid crossing scope or clocks"}
            set limit [expr {min([dict get $clocks $source period],[dict get $clocks $destination period])}]
            if {![string is double -strict $budget] || !($budget > 0 && $budget <= $limit)} {
                error "PDMA_PROFILE: crossing budget must fit within both clock periods"
            }
            if {$family eq "async_queues"} {
                set depth [dict get $spec depth]
                if {![string is integer -strict $depth] || $depth < 4 || ($depth & ($depth-1))!=0} {
                    error "PDMA_PROFILE: asynchronous queue depth must be a power of two >=4"
                }
            }
        }
    }
    return $result
}
proc profile::ports {profile} {
    set result {}
    dict for {clock settings} [dict get $profile clocks] {
        lappend result [list $clock 1 input $clock]
        lappend result [list [dict get $settings reset] 1 input $clock]
    }
    dict for {clock groups} [dict get $profile ports] {
        dict for {direction members} $groups {
            dict for {name width} $members {lappend result [list $name $width $direction $clock]}
        }
    }
    return $result
}
