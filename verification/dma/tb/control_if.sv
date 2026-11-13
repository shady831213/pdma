`ifndef PDMA_CHANNEL_COUNT
`define PDMA_CHANNEL_COUNT 1
`endif
interface pdma_control_if #(parameter int CHANNELS=`PDMA_CHANNEL_COUNT)(input logic reg_clk, core_clk, bus_clk);
    wire reg_rst_n, core_rst_n, bus_rst_n;
    logic [CHANNELS-1:0] irq, irq_ready='0;
    logic fatal, fatal_irq, fatal_irq_ready=0;
    logic [5:0] awatop;
    clocking observer @(posedge core_clk);
        input irq, irq_ready, fatal, fatal_irq, fatal_irq_ready, core_rst_n;
    endclocking
endinterface
