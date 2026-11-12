# Package resources define APIs only. Loading a dependency applies no constraints
# and no structural waiver. A caller explicitly invokes the selected provider.
namespace eval rtl_ip {variable providers {}}
proc rtl_ip::load {manifest} {
    variable providers
    if {[dict size $providers]} {error "IP_RESOURCE: providers already loaded"}
    set base [file dirname [file normalize $manifest]]
    set index 0
    foreach record [source $manifest] {
        set name [dict get $record name]
        if {[dict exists $providers $name]} {error "IP_RESOURCE: duplicate provider"}
        set space ::rtl_ip::package_$index
        namespace eval $space {}
        set exports [namespace eval $space [list source [file join $base [dict get $record entry]]]]
        dict for {method command} $exports {
            if {[string first ${space}:: [lindex $command 0]] != 0 ||
                [namespace which -command [lindex $command 0]] eq ""} {
                error "IP_RESOURCE: export must name a provider-owned command"
            }
        }
        dict set providers $name $exports
        incr index
    }
}
proc rtl_ip::invoke {provider method args} {
    variable providers
    if {![dict exists $providers $provider $method]} {
        error "IP_RESOURCE: missing provider method $provider/$method"
    }
    return [uplevel #0 [list {*}[dict get $providers $provider $method] {*}$args]]
}
