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
    for(genvar ch=0;ch<`PDMA_CHANNEL_COUNT;ch++) begin : channel_irqs
        jvs_int_if pins();
        assign pins.clk=core_clk;
        assign pins.interrupt=`JVS_MAX_INT_PIN_NUM'(control.irq[ch]) << control.irq_vectors[ch];
        initial uvm_config_db#(virtual jvs_int_if)::set(null,
            $sformatf("uvm_test_top.env.channel_%0d.irq.jvs_int_agent",ch),"jvs_int_if",pins);
    end
    jvs_int_if fatal_irq();
    assign fatal_irq.clk=core_clk;
    assign fatal_irq.interrupt=`JVS_MAX_INT_PIN_NUM'(control.fatal_irq) << control.device_irq_vector;
    initial begin
        uvm_config_db#(virtual jvs_int_if)::set(null,"uvm_test_top.env.device_fatal_irq.jvs_int_agent","jvs_int_if",fatal_irq);
        uvm_config_db#(svt_axi_vif)::set(null,"uvm_test_top.env.host_bus","vif",host_bus);
        uvm_config_db#(svt_axi_vif)::set(null,"uvm_test_top.env.memory_bus","vif",memory_bus);
        uvm_config_db#(virtual pdma_control_if)::set(null,"uvm_test_top.env","control",control);
        run_test();
    end
    initial begin #20000000; $fatal(1,"PDMA_VIP_WATCHDOG"); end
endmodule
