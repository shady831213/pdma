`timescale 1ns/1ps
module pdma_vip_top;
    import uvm_pkg::*;
    import svt_axi_uvm_pkg::*;
    import pdma_vip_pkg::*;
    import pdma_tests_pkg::*;
    wire reg_clk,core_clk,bus_clk;
    pdma_control_if control(reg_clk,core_clk,bus_clk);
    pdma_clock_domain #(.NAME("reg")) reg_domain(.clock(reg_clk),.reset_n(control.reg_rst_n),.rst_process());
    pdma_clock_domain #(.NAME("core")) core_domain(.clock(core_clk),.reset_n(control.core_rst_n),.rst_process());
    pdma_clock_domain #(.NAME("bus")) bus_domain(.clock(bus_clk),.reset_n(control.bus_rst_n),.rst_process());
    svt_axi_if host_bus(), memory_bus();
    assign host_bus.common_aclk=reg_clk;
    assign memory_bus.common_aclk=bus_clk;
    assign host_bus.master_if[0].aresetn=control.reg_rst_n;
    assign memory_bus.slave_if[0].aresetn=control.bus_rst_n;
    pdma_vip_dut dut(host_bus,memory_bus,control);
    initial begin
        uvm_config_db#(svt_axi_vif)::set(null,"uvm_test_top.env.host_bus","vif",host_bus);
        uvm_config_db#(svt_axi_vif)::set(null,"uvm_test_top.env.memory_bus","vif",memory_bus);
        uvm_config_db#(virtual pdma_control_if)::set(null,"uvm_test_top.env","control",control);
        run_test();
    end
    initial begin #20000000; $fatal(1,"PDMA_VIP_WATCHDOG"); end
endmodule
