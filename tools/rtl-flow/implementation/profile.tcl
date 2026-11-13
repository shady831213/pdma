# Tool-independent validation and expansion; tool adapters resolve real ports.
namespace eval profile {}
proc profile::read {path} {
    set result [source $path]
    dict get $result top
    if {[dict exists $result parameters]} {
        dict for {name value} [dict get $result parameters] {
            if {![regexp {^[A-Za-z_][A-Za-z0-9_]*$} $name] || ![string is integer -strict $value]} {
                error "PDMA_PROFILE: invalid integer parameter"
            }
        }
    }
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
    if {[dict exists $result crossings]} {
        dict for {instance spec} [dict get $result crossings] {
            set source [dict get $spec source]
            set destination [dict get $spec destination]
            set budget [dict get $spec max_delay]
            if {$instance eq "" || ![dict exists $clocks $source] || ![dict exists $clocks $destination] ||
                $source eq $destination} {error "PDMA_PROFILE: invalid crossing scope or clocks"}
            set limit [expr {min([dict get $clocks $source period],[dict get $clocks $destination period])}]
            if {![string is double -strict $budget] || !($budget > 0 && $budget <= $limit)} {
                error "PDMA_PROFILE: crossing budget must fit within both clock periods"
            }
            if {[catch {rtl_ip::invoke [dict get $spec provider] validate $spec} reason]} {
                error "PDMA_PROFILE: $reason"
            }
        }
    }
    memories $result
    return $result
}
proc profile::memories {profile} {
    if {![dict exists $profile memories]} {return {}}
    set result [dict get $profile memories]
    set instances {}
    set providers {}
    dict for {role spec} $result {
        if {![regexp {^[A-Za-z_][A-Za-z0-9_]*$} $role]} {error "PDMA_PROFILE: invalid memory role"}
        set instance [dict get $spec instance]
        set provider [dict get $spec provider]
        set kind [dict get $spec kind]
        if {![regexp {^[A-Za-z_][A-Za-z0-9_./]*$} $instance] || $instance in $instances} {
            error "PDMA_PROFILE: invalid or duplicate memory instance"
        }
        lappend instances $instance
        if {![regexp {^[A-Za-z_][A-Za-z0-9_]*$} $provider] || $kind ni {Ram1RW Ram1R1W}} {
            error "PDMA_PROFILE: invalid memory provider contract"
        }
        if {[dict exists $providers $provider] && [dict get $providers $provider] ne $kind} {
            error "PDMA_PROFILE: inconsistent memory provider contract"
        }
        dict set providers $provider $kind
        foreach field {depth width} {
            set value [dict get $spec $field]
            if {![string is integer -strict $value] || $value < 1} {error "PDMA_PROFILE: invalid memory geometry"}
        }
        if {![dict exists $profile clocks [dict get $spec clock]]} {error "PDMA_PROFILE: unknown memory clock"}
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
