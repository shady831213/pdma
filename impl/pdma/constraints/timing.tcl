foreach clock_port {reg_clk core_clk bus_clk} {
    create_clock -name $clock_port -period $clock_period [get_ports $clock_port]
}
set_clock_uncertainty $clock_uncertainty [all_clocks]
set_clock_transition $transition [all_clocks]
# Synchronous-path estimate only; CDC max delays and RDC require separate signoff.
set_clock_groups -asynchronous -group reg_clk -group core_clk -group bus_clk
set_false_path -from [get_ports {*rst_n}]
foreach {reset_port owner} {reg_rst_n reg_clk core_rst_n core_clk bus_rst_n bus_clk} {
    set_input_delay -min 0 -clock $owner [get_ports $reset_port]
    set_input_delay -max 0 -clock $owner [get_ports $reset_port]
}
set inputs [remove_from_collection [all_inputs] [get_ports {*clk *rst_n}]]
foreach_in_collection port $inputs {
    set name [get_object_name $port]
    set owner core_clk
    if {[string match cfg* $name]} {set owner reg_clk}
    if {[string match memory* $name]} {set owner bus_clk}
    set_input_delay -max $io_max -clock $owner $port
    set_input_delay -min $io_min -clock $owner $port
}
foreach_in_collection port [all_outputs] {
    set name [get_object_name $port]
    set owner core_clk
    if {[string match cfg* $name]} {set owner reg_clk}
    if {[string match memory* $name] || $name eq "awatop"} {set owner bus_clk}
    set_output_delay -max $io_max -clock $owner $port
    set_output_delay -min $io_min -clock $owner $port
}
set_input_transition $transition $inputs
set_load $output_load [all_outputs]
# Vectorless activity is an estimate, not workload-measured power.
set_switching_activity -static_probability 0.5 -toggle_rate 0.1 $inputs
