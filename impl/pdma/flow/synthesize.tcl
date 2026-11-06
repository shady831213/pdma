set_app_var sh_continue_on_error false
set_app_var sh_script_stop_severity E
set_app_var high_fanout_net_threshold 0
source $env(PDMA_TECH_CONFIG)
set_app_var target_library $target_libraries
set_app_var link_library [concat * $target_libraries]
set hashes [open technology.sha256 w]
foreach library $target_libraries {
    puts $hashes [exec sha256sum $library]
    read_db $library
}
close $hashes
file mkdir work
define_design_lib WORK -path work
set fd [open $env(PDMA_FILELIST) r]
set files [split [string trim [read $fd]] "\n"]
close $fd
if {![analyze -format sverilog -define SYNTHESIS $files]} {exit 2}
if {[elaborate pdma_rtl_DmaCopy] eq ""} {exit 3}
if {![link]} {exit 4}
set_operating_conditions -library $operating_library $operating_condition
redirect units.rpt {report_units}
set fd [open units.rpt r]
set units [read $fd]
close $fd
foreach {label expected} [list Time_unit $time_unit Capacitive_load_unit $capacitance_unit] {
    if {![regexp "$label\\s*:\\s*(\\S+)" $units -> actual] || $actual != $expected} {
        error "Library units mismatch"
    }
}
source $env(PDMA_CONSTRAINTS)
redirect check-design-pre.rpt {check_design}
if {![check_design]} {exit 5}
set_host_options -max_cores 4
if {![compile_ultra -no_autoungroup -no_boundary_optimization -no_seq_output_inversion]} {exit 6}
if {![get_attribute [current_design] is_mapped]} {error "Unmapped design"}
redirect check-design.rpt {check_design}
if {![check_design]} {exit 7}
redirect area.rpt {report_area -hierarchy}
redirect timing.rpt {report_timing -delay_type max -max_paths 10 -nets}
redirect hold.rpt {report_timing -delay_type min -max_paths 10}
redirect coverage.rpt {check_timing}
redirect qor.rpt {report_qor}
redirect power.rpt {report_power}
redirect references.rpt {report_reference -hierarchy}
write_sdc applied.sdc
write -format ddc -hierarchy -output copy.ddc
write -format verilog -hierarchy -output copy.v
exec sha256sum --check --status technology.sha256
puts PDMA_SYNTHESIS_COMPLETE
exit
