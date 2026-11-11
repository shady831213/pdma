// Tool-sensitivity fixture, never part of the DMA source snapshot.
module StaticProbe (
    input logic s_clk, s_rst_n, d_clk, d_rst_n, s_data,
    output logic d_data
);
    logic source, first;
    always_ff @(posedge s_clk or negedge s_rst_n)
        if (!s_rst_n) source <= 0;
        else source <= s_data;
`ifdef PDMA_RESETLESS
    always_ff @(posedge d_clk) first <= source;
`else
    always_ff @(posedge d_clk or negedge d_rst_n)
        if (!d_rst_n) first <= 0;
        else first <= source;
`endif
    always_ff @(posedge d_clk or negedge d_rst_n)
        if (!d_rst_n) d_data <= 0;
`ifdef PDMA_UNSYNCHRONIZED
        else d_data <= source;
`else
        else d_data <= first;
`endif
endmodule
