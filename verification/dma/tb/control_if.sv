`include "configuration.svh"
interface pdma_control_if #(parameter int CHANNELS=`PDMA_CHANNEL_COUNT)(input logic reg_clk, core_clk, bus_clk);
    wire reg_rst_n, core_rst_n, bus_rst_n;
    wire [CHANNELS-1:0] irq;
    // Platform timebase, in core cycles, independent of DMA reset.
    bit timer_available=1;
    bit [63:0] timer_value=0;
    always @(posedge core_clk) timer_value <= timer_value + 1;
    wire fatal, fatal_irq;
    logic [5:0] awatop;
endinterface
